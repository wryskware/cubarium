//! The opt-in art image for the live world (`cubarium run --art <dir>`).
//!
//! Same contract as [`crate::present::Presenter`] — [`ArtPresenter::observe`] once per
//! completed tick, [`ArtPresenter::draw`] once per rendered frame — but the bodies are
//! the authored sprite clips of an [`ArtPack`] instead of procedural discs, and the
//! fields grow the authored **plants** of `art/PLANTS.md` where they are rich.
//!
//! What this presenter keeps from the decided M2 image above the horizon: the night
//! floor, the producer ramp with its saturation point and squared brightness, and the
//! detritus flecks, drawn with exactly the arguments [`crate::present::Presenter::draw`]
//! uses. What it drops: trails, disc lobes, and the warm feeding flash. The clips carry
//! that expression, and stacking both reads as two creatures on top of each other.
//!
//! What it adds is the **stratification** of `design/stratified-world.md`: the cube is
//! drawn as three places rather than one top-down field. By the embedded height `h` of a
//! point ([`SurfacePoint::embed`]`()[1]`, Top = 1, the open rim = −1):
//!
//! | Band | Where | Ground | Plants (by hash, per cell) | Driven by |
//! | --- | --- | --- | --- | --- |
//! | Soil | `h < `[`SOIL_TOP`] | detritus ramp, dark plum → violet-mauve, no flecks | glowcap / rootveil, dim | detritus `D` |
//! | Foliage | [`SOIL_TOP`]` ≤ h < 1` | the decided producer ramp and flecks | lanternstalk / tendrilfan | producer `P` |
//! | Canopy | the top face (`h = 1`) | the decided producer ramp and flecks | umbrellafrond / bloomcrown, eager | producer `P` |
//! | Water | any cell deeper than [`REED_DEPTH`] | (the band's own ground) | reedspire | water depth |
//!
//! Each cell owns one plant slot at a hashed placement. The plant grows through three
//! authored stages as its field rises ([`stage_thresholds`]), with hysteresis so an
//! oscillating field does not flicker ([`next_stage`]), and a hashed **rank** caps how
//! far each slot may grow ([`rank_cap_of`]) so a rich patch shows a few full plants,
//! more mid ones and many sprouts rather than a wall of 16-pixel sprites. On the side
//! faces stalks stand *up* toward the canopy; on the top face plants are radial and face
//! wherever their hash says. A full-grown plant with a `fruit` clip plays it while the
//! cell holds fruit ([`fruit_stage`]); until the world publishes fruit, it never does.
//!
//! The horizon between soil and foliage is a *per-pixel* blend ([`w_soil`], half-width
//! [`HORIZON`] in `h`), evaluated at each pixel center's own height rather than at its
//! cell's, so it reads as one horizontal line all the way round the cube instead of a
//! staircase of cell edges. Plant choice, which is per cell, uses the cell center's band
//! with no blending.
//!
//! **The simulation does not know about bands or plants.** [`SOIL_TOP`] lives here and
//! nowhere else; the world publishes the same `P`, `D` and `w` it always did, and the
//! bands are a consequence of the height-dependent light, the downhill detritus
//! transport and the water's flow.
//!
//! Nothing here is wall-clock driven. Presentation time is [`present_seconds`]`(tick, f)`
//! — the *simulated* instant the frame shows, `(tick − 1 + f) · DT`, which is the same
//! interval the bodies are interpolated along — and every looping clip, the water shimmer,
//! the ground breath and the rain read it. So `--speed 8` animates eight times faster, a
//! paused world holds its pose, and the motion is continuous across a tick boundary
//! instead of stepping once per tick. The bud clip still advances on the organism's own
//! gestation progress.
//!
//! Growth is *paced*, and paced only in [`ArtPresenter::observe`]: one call per completed
//! tick moves each cell's [`Growth`] and each column's [`TallGrowth`] toward the target
//! its field warrants ([`advance_growth`], [`advance_tall`]), by the simulated time the
//! tick took. The first view, or a view whose tick went backwards, *snaps* instead, so a
//! mature world is not replayed from bare ground. [`ArtPresenter::draw`] is then a pure
//! function of (presenter state, view, `f`): it snap-initializes if the presenter has
//! never been observed and after that mutates nothing, so two draws of the same inputs
//! give the same image and a thousand draws advance nothing.

use std::sync::LazyLock;

use cubarium_core::OrganismId;
use cubarium_core::hunter::{FixedHunterProfile, HunterEvent, HunterPhase, HunterView};
use cubarium_core::organism::Mode;
use cubarium_core::view::{OrganismView, RenderView};
use cubarium_render::{
    Bend, Canvas, Mask, Pose, draw_field, stamp_layers, stamp_layers_bent, stamp_pose,
};
use cubarium_surface::{
    CELL_COUNT, CELLS_PER_FACE_EDGE, CellId, Edge, PixelImage, ScalarField, SurfacePoint, Vec2,
    cell_of, pixel_neighbor,
};
use cube_proto::{FACE_SIZE, Face};

pub use crate::art::Band;
use crate::art::{ArtPack, Clip, GroundTile, Plant, TallPlant};
use crate::clock::DT;
use crate::hunter_present::{HunterFrame, HunterMemory, validate_profile, validate_view};
use crate::lanternjaw::{Lanternjaw, Part};
use crate::meal_present::{MealMemory, Meals};
use crate::present::{
    self, DETRITUS_SCALE, DETRITUS_THRESHOLD, JUVENILE_SCALE, PALETTE, PRODUCER_SATURATION,
};
use crate::rng::SplitMix64;

#[cfg(test)]
#[path = "../corner_cap_present_tests.rs"]
mod corner_cap_present_tests;
#[cfg(test)]
#[path = "../vine_strips_present_tests.rs"]
mod vine_strips_present_tests;

mod environment;
mod growth;
mod habitat;
mod tall;
mod wind;

pub use environment::*;
pub use growth::*;
pub use habitat::*;
pub use tall::*;
pub use wind::*;

use environment::filtered_at;
use habitat::{FACE_PIXELS, PHASE_SEED, SOIL_RAMP, SOIL_WEIGHT, weight_index};
use tall::{TallSpecies, draw_column, layers_extent, stage_layers, stage_pose};
use wind::{budget_in, hermite};

/// The pack's plants resolved to indices once, per band and pick. A pack without a plant
/// (a v1 pack, or a repaint that dropped one) simply grows nothing in that slot.
#[derive(Clone, Copy, Debug, Default)]
struct Species {
    soil: [Option<usize>; 2],
    foliage: [Option<usize>; 2],
    canopy: [Option<usize>; 2],
    water: Option<usize>,
}

impl Species {
    fn resolve(pack: &ArtPack) -> Species {
        let find = |name: &str| pack.plants.iter().position(|p| p.name == name);
        Species {
            soil: [find(SOIL_PLANTS[0]), find(SOIL_PLANTS[1])],
            foliage: [find(FOLIAGE_PLANTS[0]), find(FOLIAGE_PLANTS[1])],
            canopy: [find(CANOPY_PLANTS[0]), find(CANOPY_PLANTS[1])],
            water: find(WATER_PLANT),
        }
    }

    fn index(&self, band: Band, pick: usize) -> Option<usize> {
        match band {
            Band::Soil => self.soil[pick],
            Band::Foliage => self.foliage[pick],
            Band::Canopy => self.canopy[pick],
            Band::Water => self.water,
        }
    }
}

/// What the presenter remembers about one organism's body, so a state change can cross-fade
/// instead of cutting — and a change that arrives *during* a fade starts from the blend
/// actually on screen rather than from the pose it was fading toward.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyMemory {
    /// The state it is in now ([`state_of`]).
    pub state: usize,
    /// What the body was showing the instant `state` began: at most two earlier states
    /// with weights summing to 1 (an unused entry has weight 0). A body that has never
    /// changed, or entered on a snap, holds `[(state, 1), (state, 0)]`.
    pub prev: [(usize, f32); 2],
    /// The presentation seconds at which `state` began; `−∞` for a body that entered on a
    /// snap, which therefore draws with no fade.
    pub switched_at: f64,
    /// The unit heading the body was drawn with at the end of the last observed tick
    /// ([`present::interpolate`] at `f = 1`): what the next tick's turn is measured from.
    pub end_heading: Vec2,
    /// The signed turn, in radians in `(−π, π]`, from `end_heading` of the previous tick to
    /// the heading this tick *starts* with. The world turns a body in one step at the
    /// tick; the presenter spends that step over the tick's frames ([`turn_heading`]), so
    /// a turning body rotates continuously instead of popping. 0 for a body just entered.
    pub turn: f64,
}

impl BodyMemory {
    /// A body first seen in `state`, drawn with no fade and no pending turn.
    pub fn entered(state: usize) -> BodyMemory {
        BodyMemory {
            state,
            prev: [(state, 1.0), (state, 0.0)],
            switched_at: f64::NEG_INFINITY,
            end_heading: Vec2::new(1.0, 0.0),
            turn: 0.0,
        }
    }

    /// How far the fade into `state` has got at presentation seconds `s`, in `[0, 1]`.
    pub fn fade_at(&self, s: f64) -> f32 {
        let w = (s - self.switched_at) / BODY_FADE_SECONDS;
        if w.is_nan() {
            1.0
        } else {
            w.clamp(0.0, 1.0) as f32
        }
    }

    /// The states on screen at presentation seconds `s` with their weights (summing to 1):
    /// the earlier states at `prev · (1 − fade)` and `state` at `fade`, zero-weight entries
    /// dropped and equal states merged. Once the fade is complete this is `[(state, 1)]`.
    pub fn layers_at(&self, s: f64) -> Vec<(usize, f32)> {
        let w = self.fade_at(s);
        let mut out: Vec<(usize, f32)> = Vec::with_capacity(3);
        let mut push = |state: usize, weight: f32| {
            if weight <= 0.0 {
                return;
            }
            match out.iter_mut().find(|(st, _)| *st == state) {
                Some((_, acc)) => *acc += weight,
                None => out.push((state, weight)),
            }
        };
        for (state, weight) in self.prev {
            push(state, weight * (1.0 - w));
        }
        push(self.state, w);
        out
    }

    /// Record the headings of a newly observed tick: `start` is the heading the tick's
    /// path begins with, `end` the one it leaves the body with. The turn from the previous
    /// tick's `end_heading` to `start` is what the frames of this tick spend.
    pub fn observe_heading(&mut self, start: Vec2, end: Vec2) {
        self.turn = signed_turn(self.end_heading, start);
        self.end_heading = end;
    }

    /// Record a change to `state` at presentation seconds `now`.
    ///
    /// **Normative**: the new `prev` is the blend on screen at `now` ([`layers_at`]), which
    /// is what the next fade starts from; with three distinct states in it, the lightest is
    /// dropped and the other two renormalized (a step of at most that weight, which cannot
    /// exceed one third, on a body that changed state twice inside one fade). A change back
    /// to the state being faded out therefore resumes from exactly the current blend.
    pub fn switch_to(&mut self, state: usize, now: f64) {
        let mut layers = self.layers_at(now);
        layers.sort_by(|a, b| b.1.total_cmp(&a.1));
        layers.truncate(2);
        let total: f32 = layers.iter().map(|(_, w)| w).sum();
        let mut prev = [(state, 0.0f32); 2];
        for (slot, (st, w)) in prev.iter_mut().zip(layers) {
            *slot = (st, if total > 0.0 { w / total } else { 0.0 });
        }
        if prev.iter().all(|(_, w)| *w <= 0.0) {
            prev = [(self.state, 1.0), (self.state, 0.0)];
        }
        *self = BodyMemory {
            state,
            prev,
            switched_at: now,
            ..*self
        };
    }
}

/// The signed angle, in `(−π, π]`, that turns unit vector `from` onto `to` the short way;
/// 0 when either cannot be normalized.
pub fn signed_turn(from: Vec2, to: Vec2) -> f64 {
    let (Some(a), Some(b)) = (from.normalized(), to.normalized()) else {
        return 0.0;
    };
    let d = b.screen_angle() - a.screen_angle();
    let d = (d + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    if d.is_finite() { d } else { 0.0 }
}

/// The heading a body is drawn with a fraction `f` into its tick: the path's own direction
/// `dir` (from [`present::interpolate`], which already follows the chart across a seam)
/// turned back by the unspent part `(1 − f) · turn` of the turn the tick began with.
///
/// **Normative**: at `f = 0` this is the previous tick's end heading, at `f = 1` it is
/// `dir` itself, and in between it rotates the short way — so a body that the world turned
/// by 30° at a tick boundary sweeps those 30° over the tick's frames. `turn = 0` leaves
/// `dir` unchanged. A non-finite `f` reads as 0.
pub fn turn_heading(dir: Vec2, turn: f64, f: f64) -> Vec2 {
    let f = if f.is_finite() {
        f.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if turn == 0.0 || !turn.is_finite() {
        return dir;
    }
    Vec2::from_screen_angle(dir.screen_angle() - (1.0 - f) * turn)
}

/// Outgoing presentation of an observed target that disappeared at this boundary.
/// Kept only for a hunter's retained-prey interpolation tick, never as dead-id history.
#[derive(Clone, Copy)]
struct OutgoingPrey {
    tick: u64,
    body: Option<BodyMemory>,
    meal: Option<MealMemory>,
}

/// The live world drawn with the baked art. Holds the pack, the scratch buffers the
/// field and sprite paths need, the fixed per-cell slots, and the renderer-side history the
/// art image keeps: how far each cell's plant, each tall column and each body has got
/// toward what the world now says.
pub struct ArtPresenter {
    pack: ArtPack,
    species: Species,
    producer: ScalarField,
    detritus: ScalarField,
    /// The raw detritus field, which the soil ground ramps against. Separate from
    /// `detritus`, which the flecks threshold: the soil is a wash, not flecks.
    soil: ScalarField,
    /// One layer of the foliage/canopy ground, drawn on its own so it can be faded out
    /// per pixel by the horizon before it reaches the image.
    layer: Canvas,
    scratch: Vec<PixelImage>,
    /// One slot per field cell, in `CellId` index order. Placement never changes.
    slots: Vec<Slot>,
    /// The band each cell was last drawn in; water can move a cell in and out of
    /// [`Band::Water`], and a cell that changes band starts over from bare ground.
    bands: Vec<Band>,
    /// How far each cell's plant has got toward the stage its field warrants: the target
    /// [`next_stage`] decides, the step in flight toward it, and the fruit blend. Advanced
    /// once per completed tick in [`ArtPresenter::observe`] and nowhere else;
    /// [`ArtPresenter::draw`] only reads it (a presenter that has never been observed snaps
    /// it to the view on its first draw).
    growth: Vec<Growth>,
    /// `growth` as the previous `observe` left it, so a frame at fraction `f` of the tick
    /// draws [`growth_between`] the two: growth is then continuous at the render rate, not
    /// stepped at 20 Hz. Equal to `growth` after a snap.
    growth_prev: Vec<Growth>,
    /// The water field, seam-filtered per pixel when drawn.
    water: ScalarField,
    /// The pack's tall species resolved once.
    tall_species: TallSpecies,
    /// The tall columns the hash selected, side faces in `Face::ALL` order.
    columns: Vec<TallColumn>,
    /// How tall each column has got, in segments, toward the target [`next_tall`] decides;
    /// advanced in `observe` exactly like `growth`.
    tall: Vec<TallGrowth>,
    /// `tall` as the previous `observe` left it, for [`tall_between`].
    tall_prev: Vec<TallGrowth>,
    /// The state each live body is in and when it last changed, for the cross-clip fade.
    /// Bodies the view no longer carries are dropped in `observe`.
    bodies: std::collections::HashMap<OrganismId, BodyMemory>,
    /// The measured bend budget of every plant and every tall family in the pack, by asset
    /// name, in pack order: plants first, then tall families. Measured once here
    /// ([`plant_bend_budget`], [`tall_bend_budget`]) and never per frame, so a pose that is
    /// wide only in one frame of one clip still cannot clip its footprint when the wind
    /// blows.
    budgets: Vec<(String, f64)>,
    /// The tick of the last [`ArtPresenter::observe`], `None` until the first one: what
    /// tells `observe` how much simulated time to advance, and what makes the first view a
    /// snap instead of a replay of a mature world's whole growth.
    last_tick: Option<u64>,
    /// The selected megafauna body, rasterized per frame for every hunter member.
    lanternjaw: Lanternjaw,
    /// The hunters the world's observer lists, by full generation-bearing id, in id order
    /// so two hunters always composite in the same order. Bounded by the membership list
    /// handed to [`ArtPresenter::observe_hunters`]; an id the list no longer carries is
    /// forgotten there. Never derived from `genome.form`.
    hunters: std::collections::BTreeMap<OrganismId, HunterMemory>,
    /// Scratch for the rig's parts, reused across frames.
    hunter_parts: Vec<Part>,
    /// The meal memory of every ordinary organism ([`crate::meal_present`]): where the feed
    /// clip is read from once an actual intake bout has begun.
    meals: Meals,
    /// At most one entry per previously observed hunter target, pruned to actual retained
    /// prey after hunter observation and cleared at the next ordinary tick or snap.
    outgoing_prey: std::collections::BTreeMap<OrganismId, OutgoingPrey>,
    /// Whether the meal onset is drawn at all; off, bodies use their original shared clip
    /// clock (a review switch for paired captures and tests, not a setting).
    meal_onset: bool,
}

impl ArtPresenter {
    /// Build the presenter and lay out every cell's slot once.
    pub fn new(pack: ArtPack) -> ArtPresenter {
        let species = Species::resolve(&pack);
        let slots: Vec<Slot> = CellId::all().map(slot_of).collect();
        let bands = CellId::all().map(band_of).collect();
        let tall_species = TallSpecies::resolve(&pack);
        let columns = tall_columns();
        let tall = vec![
            TallGrowth {
                height: 0.0,
                target: 0
            };
            columns.len()
        ];
        // The amplitude budgets of the shipped pack, measured once from its own pixels.
        let budgets: Vec<(String, f64)> = pack
            .plants
            .iter()
            .map(|p| (p.name.clone(), plant_bend_budget(p)))
            .chain(
                pack.tall
                    .iter()
                    .map(|p| (p.name.clone(), tall_bend_budget(p))),
            )
            .collect();
        ArtPresenter {
            pack,
            species,
            water: ScalarField::zeros(),
            tall_species,
            columns,
            tall_prev: tall.clone(),
            tall,
            producer: ScalarField::zeros(),
            detritus: ScalarField::zeros(),
            soil: ScalarField::zeros(),
            layer: Canvas::new(),
            scratch: Vec::new(),
            slots,
            bands,
            growth: vec![Growth::snapped(None, false); CELL_COUNT],
            growth_prev: vec![Growth::snapped(None, false); CELL_COUNT],
            bodies: std::collections::HashMap::new(),
            budgets,
            last_tick: None,
            lanternjaw: Lanternjaw::new(),
            hunters: std::collections::BTreeMap::new(),
            hunter_parts: Vec::new(),
            meals: Meals::new(),
            outgoing_prey: std::collections::BTreeMap::new(),
            meal_onset: true,
        }
    }

    /// This presenter with the meal onset ([`crate::meal_present`]) switched off, so ordinary
    /// bodies use their original shared clip clock: the "before" half of a paired capture.
    /// Both halves retain an outgoing prey's body fade through capture. Observation still
    /// tracks meals, so the switch can be compared frame for frame.
    pub fn without_meal_onset(mut self) -> ArtPresenter {
        self.meal_onset = false;
        self
    }

    /// The meal memory of an ordinary organism, if it is tracked.
    pub fn meal_of(&self, id: OrganismId) -> Option<MealMemory> {
        self.meals.memory_of(id)
    }

    /// How many organisms the meal memory tracks (bounded by the living population).
    pub fn meals_tracked(&self) -> usize {
        self.meals.len()
    }

    /// Whether this presenter can draw hunters of `profile`: [`validate_profile`], the
    /// renderer's capability stated before a world is stepped or drawn. A host asks this at
    /// load, with the saved world's own profile, and fails by name rather than clamping,
    /// substituting an ordinary body, or panicking mid-frame later.
    pub fn validate_hunter_profile(&self, profile: &FixedHunterProfile) -> Result<(), String> {
        validate_profile(profile)
    }

    /// Record this tick's hunter membership from the world's own observer, after
    /// [`ArtPresenter::observe`] of the same view, together with the hunter events the world
    /// committed at this tick.
    ///
    /// **Normative.** A member is a hunter by its full id in `hunters` and nothing else: an
    /// ordinary organism with the same `genome.form` is not one, and an entry whose id does
    /// not resolve to an organism of `view` (a stale generation) is ignored. A member whose
    /// published scale or contact geometry the rig cannot honour ([`validate_view`]) is an
    /// error, named: nothing is clamped and no ordinary body is substituted — a host that
    /// validated the profile at load never sees this. Every other member gets a
    /// [`HunterMemory`] — entered from its persisted `entered_from` when first seen, advanced
    /// by [`HunterMemory::observe`] after — and is drawn once, as the Lanternjaw, never also as
    /// its atelier rig. Memory of an id the list no longer carries is dropped, so the map is
    /// bounded by the membership. A target that leaves the view at the same boundary its
    /// hunter enters `Handling` is kept for that one tick's frames ([`HunterMemory::prey`]),
    /// carried to the `Capture` event's settlement position when the event names it, so a
    /// captured prey is not shown vanishing before the claws close; nothing is kept longer,
    /// and nothing is regenerated.
    ///
    /// **Idempotent and rewind-safe**: the same completed view observed again changes no
    /// memory and no frame; a view of an earlier tick (a rewind or a replaced world — the
    /// same rule [`ArtPresenter::observe`] snaps on) starts every memory over as first seen,
    /// so a later phase's reach never leaks into an earlier tick. With an empty list this
    /// changes nothing: the image is bit for bit the one a presenter never told about hunters
    /// draws.
    pub fn observe_hunters(
        &mut self,
        view: &RenderView,
        hunters: &[HunterView],
        events: &[HunterEvent],
    ) -> Result<(), String> {
        if self.hunters.values().any(|m| view.tick < m.cur.tick) {
            self.hunters.clear();
            self.outgoing_prey.clear();
        }
        let live: std::collections::HashSet<OrganismId> =
            view.organisms.iter().map(|o| o.id).collect();
        let mut listed = Vec::with_capacity(hunters.len());
        for h in hunters {
            if !live.contains(&h.id) {
                continue;
            }
            validate_view(h)?;
            listed.push(h.id);
            let frame = HunterFrame::of(h, view.tick);
            let target_view = frame
                .target
                .and_then(|t| view.organisms.iter().find(|o| o.id == t))
                .cloned();
            match self.hunters.get_mut(&h.id) {
                Some(memory) if memory.cur.tick == view.tick => {
                    // The same completed view again: one observation, nothing moves.
                }
                Some(memory) => {
                    memory.observe(frame);
                    let captured = memory.prev.as_ref().is_some_and(|p| {
                        p.target.is_some_and(|t| !live.contains(&t))
                            && frame.phase == HunterPhase::Handling
                            && frame.started == view.tick
                    });
                    memory.prey = if captured {
                        memory.target_view.take()
                    } else {
                        None
                    };
                    memory.prey_at = None;
                    memory.target_view = target_view;
                }
                None => {
                    let mut memory = HunterMemory::enter(frame);
                    memory.target_view = target_view;
                    self.hunters.insert(h.id, memory);
                }
            }
        }
        self.hunters.retain(|id, _| listed.contains(id));
        for id in listed {
            self.meals.forget(id);
        }
        for event in events {
            if let HunterEvent::Capture {
                tick,
                hunter,
                prey,
                evidence,
                ..
            } = event
                && *tick == view.tick
                && let Some(memory) = self.hunters.get_mut(hunter)
            {
                memory.note_capture(*prey, evidence.prey_pos);
            }
        }
        // Only a prey the hunter adapter actually retains may keep this one-tick pose.
        // A vanished target without Handling, an empty membership list, and a later tick
        // all discard the provisional copy. Repeating this boundary preserves the copy.
        self.outgoing_prey.retain(|id, outgoing| {
            outgoing.tick == view.tick
                && self
                    .hunters
                    .values()
                    .any(|m| m.prey.as_ref().is_some_and(|p| p.id == *id))
        });
        Ok(())
    }

    /// The adapter's memory of a hunter, if it is drawn as the Lanternjaw.
    pub fn hunter_of(&self, id: OrganismId) -> Option<&HunterMemory> {
        self.hunters.get(&id)
    }

    /// The hunters currently drawn as the Lanternjaw, in composite order.
    pub fn hunter_ids(&self) -> Vec<OrganismId> {
        self.hunters.keys().copied().collect()
    }

    /// The measured amplitude budget of an asset, in tile pixels: the largest `|amplitude|`
    /// every frame of every clip of that plant or tall family can be bent by without any
    /// painted texel leaving the nine-pixel stamp footprint ([`cubarium_render::Sprite::bend_headroom`],
    /// [`plant_bend_budget`], [`tall_bend_budget`]).
    ///
    /// **Normative**: measured once when the presenter is built and constant for its life;
    /// [`f64::INFINITY`] for an asset nothing bounds, and **0** for a name this pack does not
    /// carry, so an unknown asset stands still rather than moving on an unmeasured budget.
    pub fn bend_budget(&self, name: &str) -> f64 {
        budget_in(&self.budgets, name)
    }

    /// Every measured budget, in pack order (plants, then tall families): the table a review
    /// session reads to see which species the pack's own art is holding back.
    pub fn bend_budgets(&self) -> &[(String, f64)] {
        &self.budgets
    }

    /// The one budget a whole tall column shares: the smaller of its own family's and, when
    /// it carries a vine, the vine's. Every part of the column bends by the same amplitude,
    /// so the vine's art bounds the tree's motion as much as the tree's own does.
    pub fn column_budget(&self, column: &TallColumn) -> f64 {
        let own = self.bend_budget(TALL_PLANTS[column.pick]);
        if column.vine {
            own.min(self.bend_budget(VINE_PLANT))
        } else {
            own
        }
    }

    /// The growth of a cell's plant as the previous `observe` left it (equal to
    /// [`ArtPresenter::growth_of`] after a snap): what a frame at fraction `f` interpolates
    /// from.
    pub fn growth_prev_of(&self, cell: CellId) -> Growth {
        self.growth_prev[cell.index()]
    }

    /// What the presenter remembers of a body, if it has seen it.
    pub fn body_of(&self, id: OrganismId) -> Option<BodyMemory> {
        self.bodies.get(&id).copied()
    }

    /// The loaded pack, for tests that want to compare a drawn body or plant against its
    /// sprite.
    pub fn pack(&self) -> &ArtPack {
        &self.pack
    }

    /// The stage a cell's plant is *headed for* after the last `observe`: what the field
    /// warrants once [`next_stage`]'s hysteresis has had its say, `None` for bare ground.
    /// The visual may still be part-way there — see [`ArtPresenter::growth_of`].
    pub fn stage_of(&self, cell: CellId) -> Option<u8> {
        self.growth[cell.index()].target
    }

    /// The whole paced growth of a cell's plant: where the visual is, where it is going and
    /// how far along it is.
    pub fn growth_of(&self, cell: CellId) -> Growth {
        self.growth[cell.index()]
    }

    /// The band a cell was last drawn in.
    pub fn band_at(&self, cell: CellId) -> Band {
        self.bands[cell.index()]
    }

    /// The plant a cell grows in a band, if the pack has it.
    pub fn plant_for(&self, band: Band, cell: CellId) -> Option<&Plant> {
        let pick = self.slots[cell.index()].pick;
        self.species.index(band, pick).map(|i| &self.pack.plants[i])
    }

    /// Record one completed tick: retarget every cell's plant and every column to what its
    /// field now warrants ([`next_stage`] and [`next_tall`], with their hysteresis), and
    /// move the visuals that much closer to it. This is the only place the art image's
    /// history advances.
    ///
    /// **Normative**: the step is `dt = (view.tick − last_tick) · DT` clamped into `[0,
    /// `[`MAX_STEP_SECONDS`]`]`. The presenter **snaps** instead — every visual set to its
    /// target, every body entered with no fade, the previous-state copies made equal — when
    /// this is the first view (a mature world must not replay its growth from bare ground)
    /// or when the tick went backwards (a replaced or rewound world). A snap derives its
    /// targets from the same empty baseline a new presenter would (`next_stage(None, ..)`,
    /// `next_tall(0, ..)`), so the old world's hysteresis does not leak into the new one and
    /// restoring a view into a used presenter draws what a fresh presenter would. A cell
    /// whose band changed starts over from bare ground and then grows paced.
    pub fn observe(&mut self, view: &RenderView) {
        self.observe_with_fruit(view, Some(&view.fruit));
    }

    /// [`ArtPresenter::observe`] against a fruit field other than the view's own, the way
    /// [`ArtPresenter::draw_with_fruit`] draws against one: the fruit accent is paced here,
    /// not at draw time, so a test that wants to see a plant in fruit must observe the
    /// fruit. `None` is "the world publishes no fruit yet".
    pub fn observe_with_fruit(&mut self, view: &RenderView, fruit: Option<&[f64]>) {
        let snap = match self.last_tick {
            None => true,
            Some(last) => view.tick < last,
        };
        if snap {
            // A rewind or a replaced world: the hunters' memory starts over with everything
            // else, so no later phase's reach can leak back in time.
            self.hunters.clear();
            self.outgoing_prey.clear();
        }
        let dt = match self.last_tick {
            Some(last) if view.tick >= last => {
                (((view.tick - last) as f64) * DT).clamp(0.0, MAX_STEP_SECONDS)
            }
            _ => 0.0,
        };
        // The previous-state copies are "as the previous *tick* left it": a repeated
        // observe of the same tick retargets but must not fold them forward, or a frame
        // drawn mid-tick would stop interpolating from the tick before.
        let new_tick = self.last_tick != Some(view.tick);
        if new_tick {
            self.outgoing_prey.clear();
            // Save the last published target's outgoing pose before live-body/meal pruning.
            // The hunter observer follows this call and decides whether a capture actually
            // retained that target. No targets are fabricated on a first view or rewind.
            for hunter in self.hunters.values() {
                if let Some(prey) = &hunter.target_view
                    && !view.organisms.iter().any(|o| o.id == prey.id)
                {
                    self.outgoing_prey.insert(
                        prey.id,
                        OutgoingPrey {
                            tick: view.tick,
                            body: self.bodies.get(&prey.id).copied(),
                            meal: self.meals.memory_of(prey.id),
                        },
                    );
                }
            }
        }
        for (index, cell) in CellId::all().enumerate() {
            let band = cell_band(cell, view.water.get(index).copied());
            if band != self.bands[index] {
                self.bands[index] = band;
                // The new band's plant is a different plant: it starts from bare ground.
                self.growth[index] = Growth::snapped(None, false);
            }
            let t = plant_density(view, index, band);
            let from_target = if snap {
                None
            } else {
                self.growth[index].target
            };
            let target = plant_cap(band, cell)
                .and_then(|cap| next_stage(from_target, t, &stage_thresholds(band), cap));
            let in_fruit = fruit_stage(fruit.and_then(|f| f.get(index).copied()));
            if new_tick {
                self.growth_prev[index] = self.growth[index];
            }
            self.growth[index] = if snap {
                Growth::snapped(target, in_fruit)
            } else {
                advance_growth(self.growth[index], target, in_fruit, dt)
            };
            if snap {
                self.growth_prev[index] = self.growth[index];
            }
        }
        for (i, column) in self.columns.iter().enumerate() {
            let t_col = column_density(view, column.face, column.cx);
            let from_target = if snap { 0 } else { self.tall[i].target };
            let target = next_tall(from_target, t_col);
            if new_tick {
                self.tall_prev[i] = self.tall[i];
            }
            self.tall[i] = if snap {
                TallGrowth {
                    height: f64::from(target),
                    target,
                }
            } else {
                advance_tall(self.tall[i], target, dt)
            };
            if snap {
                self.tall_prev[i] = self.tall[i];
            }
        }
        self.observe_bodies(view, snap);
        // Meals: the hunter adapter's members are its own; every other body is tracked.
        let hunters = &self.hunters;
        self.meals
            .observe(view, snap, &|id| hunters.contains_key(&id));
        self.last_tick = Some(view.tick);
    }

    /// The body memory: a state change records when it happened and the blend it started
    /// from ([`BodyMemory::switch_to`]), an unknown id enters with no fade, and an id the
    /// view no longer carries is forgotten.
    fn observe_bodies(&mut self, view: &RenderView, snap: bool) {
        if snap {
            self.bodies.clear();
        }
        let now = present_seconds(view.tick, 0.0);
        for o in &view.organisms {
            let state = state_of(o);
            let start = present::interpolate(&o.moved, o.pos, o.heading, 0.0).1;
            let end = present::interpolate(&o.moved, o.pos, o.heading, 1.0).1;
            match self.bodies.get_mut(&o.id) {
                Some(memory) => {
                    if memory.state != state {
                        memory.switch_to(state, now);
                    }
                    memory.observe_heading(start, end);
                }
                None => {
                    let mut memory = BodyMemory::entered(state);
                    // A body just seen has no previous heading to turn from.
                    memory.end_heading = end;
                    self.bodies.insert(o.id, memory);
                }
            }
        }
        if self.bodies.len() > view.organisms.len() {
            let live: std::collections::HashSet<OrganismId> =
                view.organisms.iter().map(|o| o.id).collect();
            self.bodies.retain(|id, _| live.contains(id));
        }
    }

    /// The trunk segments a tall column is *headed for* after the last `observe` (0 when
    /// nothing tall stands there), by its index in [`tall_columns`]. The column may still be
    /// growing toward it — see [`ArtPresenter::tall_growth_of`].
    pub fn segments_of(&self, column: usize) -> u8 {
        self.tall.get(column).map_or(0, |g| g.target)
    }

    /// The paced height of a tall column: how tall it is drawn now and what it is headed
    /// for.
    pub fn tall_growth_of(&self, column: usize) -> TallGrowth {
        self.tall.get(column).copied().unwrap_or(TallGrowth {
            height: 0.0,
            target: 0,
        })
    }

    /// The tall columns this presenter draws, in the order [`segments_of`] indexes.
    pub fn columns(&self) -> &[TallColumn] {
        &self.columns
    }

    /// Draw the art image: floor, foliage/canopy ground, soil ground, ground cover,
    /// water, plants, tall plants, rain, bodies, in that order. `f` is the clock's interpolation fraction, used exactly as
    /// [`crate::present::Presenter::draw`] uses it. Full-grown plants with a `fruit` clip
    /// play it where the view's fruit field says the cell is in fruit; see
    /// [`ArtPresenter::draw_with_fruit`] for drawing with a different fruit field.
    ///
    /// The two grounds are complementary per pixel — the foliage ground is scaled by
    /// `1 − `[`w_soil`] and the soil ground by [`w_soil`] — so above the horizon the
    /// image is exactly the decided one, below it there is no producer lawn and no
    /// fleck, and across the horizon the two cross-fade over about a cell.
    pub fn draw(&mut self, view: &RenderView, f: f64, canvas: &mut Canvas) {
        self.draw_with_fruit(view, f, canvas, Some(&view.fruit));
    }

    /// [`ArtPresenter::draw`] with the world's fruit field (one value per cell, the same
    /// order as the other fields): a full-grown plant whose species has a `fruit` clip
    /// plays it where [`fruit_stage`] says the cell is in fruit. [`ArtPresenter::draw`]
    /// passes the view's own `fruit`; tests pass `None` or a synthetic field.
    ///
    /// The fruit field passed here does two things: it snap-initializes a presenter that
    /// has never been observed, and it **gates** the accent at draw time — a cell the field
    /// says is not in fruit ([`fruit_stage`]) draws no fruit whatever the paced
    /// [`Growth::fruit`] says, so `None` always suppresses the accent and the picture never
    /// shows food the field does not hold. The fade *in* is the paced value
    /// [`ArtPresenter::observe_with_fruit`] advanced.
    ///
    /// # A stage step in flight
    ///
    /// **Normative**, and the whole of the growth pilot. A cell whose [`Growth`] is in flight
    /// this frame is decomposed by [`growth_step`] into its `lower` stage, its `upper` stage
    /// and the upper stage's progress `t` — one number that runs 0 → 1 up the step whichever
    /// way the step is travelling. Then, with `plant` the cell's species:
    ///
    /// * When `lower` is a stage and [`Plant::transition`]`(lower, upper)` is `Some(clip)` —
    ///   an **authored growth clip**, pack v5 — the step is drawn as **one**
    ///   [`cubarium_render::stamp_layers_bent`] of three layers at the weights
    ///   [`growth_weights`]`(t)` gives: `[(stage_pose(lower), w_from), (clip.sample(t ·
    ///   clip.seconds), w_grow), (stage_pose(upper), w_to)]` — `stage_pose` being the stage's
    ///   own looping sway clip at the slot's phase, exactly the pose an idle plant shows at
    ///   that instant — with [`Mask::None`], opacity
    ///   `opacity_of(lower) + (opacity_of(upper) − opacity_of(lower)) · t`
    ///   ([`stage_opacity`] at the cell's own density), and the slot's wind — the same
    ///   `(`[`cubarium_render::Bend`]`, heading)` [`slot_wind`] gives every other stamp of
    ///   that slot this frame, so the breeze carries on right through the growth. Nothing
    ///   here is masked: the clip's own art says what a half-grown plant looks like. Top-face
    ///   (radial) slots with a clip follow exactly the same rule.
    /// * Otherwise — every pair the pack has no clip for, the bare-ground step (`None ↔ 0`,
    ///   which has no lower stage to blend from), and every plant of a pack before v5 — the
    ///   step keeps the **reveal masks**: the lower stage stamped whole at `opacity_of(lower)
    ///   · (1 − t)` and the upper stage over it at `opacity_of(upper)` through
    ///   [`Mask::Axial`]`{ reveal: t · `[`PLANT_REVEAL_PX`]` }` on a side face or
    ///   [`Mask::Radial`]`{ reveal: t · (extent + 0.5) }` outward from the pivot on the top
    ///   face, `extent` being the largest [`cubarium_render::Pose::extent`] of the upper
    ///   stage's layers — so `t = 1` is exactly [`Mask::None`] for that pose.
    ///
    /// The clip is a pure function of `t` and is therefore never restarted, resumed or
    /// advanced by anything a frame does: a repeated draw of the same (state, view, `f`) is
    /// the same image, a reversal replays the same `t` backwards ([`growth_step`]), a wind
    /// packet arriving mid-step changes only the bend, and pausing holds the pose. The fruit
    /// accent takes no part in an authored step: [`advance_growth`] holds it at 0 while a
    /// plant is in flight and the three-layer stamp has no fruit layer, so an authored
    /// growth stamp never reads the `fruit` clip. On the *mask* path a full-grown plant in
    /// fruit that turns round still carries the accent [`growth_between`] interpolates from
    /// the previous tick for the first frames of the step (about two frames at 60 fps, only
    /// for a fruiting species whose 1 → 2 step has no clip — none on the shipped pack since
    /// the canopy species were authored on 2026-09-13; a v1–v4 pack's bloomcrown).
    pub fn draw_with_fruit(
        &mut self,
        view: &RenderView,
        f: f64,
        canvas: &mut Canvas,
        fruit: Option<&[f64]>,
    ) {
        // A presenter that has never seen a tick snaps to this view, exactly as the first
        // `observe` would: a mature world is drawn as it is, not replayed from bare ground.
        // After that `draw` mutates nothing, so a thousand draws advance nothing.
        if self.last_tick.is_none() {
            self.observe_with_fruit(view, fruit);
        }
        let seconds = present_seconds(view.tick, f);

        canvas.clear();
        present::draw_floor(canvas);

        // The decided producer ramp, with the same arguments the M2 presenter passes,
        // drawn into a scratch layer and then faded out through the horizon. Drawing it
        // into a layer rather than straight onto the canvas is what lets the weight be
        // per pixel while `present::draw_ramp_field` stays untouched — and at weight 1
        // the multiply is exact, so a foliage pixel is bit-for-bit the decided image.
        present::copy_field(&mut self.producer, &view.producer);
        let saturation = view.producer_max * PRODUCER_SATURATION;
        self.layer.clear();
        present::draw_ramp_field(
            &mut self.layer,
            &self.producer,
            saturation,
            PALETTE.producer_low,
            PALETTE.producer_high,
            true,
        );
        add_above_horizon(canvas, &self.layer);

        // Detritus flecks, exactly as the M2 presenter draws them — and likewise only
        // above the horizon. In the soil, detritus is the ground itself, not a fleck.
        present::threshold_field(&mut self.detritus, &view.detritus, DETRITUS_THRESHOLD);
        self.layer.clear();
        draw_field(
            &mut self.layer,
            &self.detritus,
            DETRITUS_SCALE,
            PALETTE.detritus,
            false,
        );
        add_above_horizon(canvas, &self.layer);

        // The soil ground: dark plum to violet-mauve by raw detritus, seam-filtered like
        // every other field layer, faded in through the same horizon.
        present::copy_field(&mut self.soil, &view.detritus);
        draw_soil_ground(canvas, &self.soil);

        // Ground cover: each band's tileable texture on the 8-px lattice, fading in with
        // the density that grows the band's plants and cross-fading through the horizon.
        {
            let pack = &self.pack;
            let scratch = &mut self.scratch;
            for face in Face::ALL {
                for (face, x, y) in ground_points(face) {
                    let point = SurfacePoint::pixel_center(face, x, y);
                    let cell = cell_of(&point);
                    let band = band_of(cell);
                    let Some(tile) = pack.ground_for(band) else {
                        continue;
                    };
                    let t = plant_density(view, cell.index(), band);
                    let opacity = ground_opacity(t, band) * ground_weight(face, x, y, band);
                    if opacity <= 0.0 {
                        continue;
                    }
                    let pose =
                        ground_pose(tile, seconds + ground_phase_of(face, x, y, tile.seconds));
                    stamp_pose(
                        canvas,
                        point,
                        Vec2::new(1.0, 0.0),
                        pose,
                        1.0,
                        opacity,
                        Mask::None,
                        scratch,
                    );
                }
            }
        }

        // Water: pools and streams source-over the ground, under the plants.
        if !view.water.is_empty() {
            present::copy_field(&mut self.water, &view.water);
            let saturation = view.producer_max * PRODUCER_SATURATION;
            draw_water(canvas, &self.water, &self.producer, saturation, seconds);
        }

        // Plants: scenery that follows the fields. These are not organisms — nothing in
        // the world knows about them, they never move, and they are not eaten. They are
        // how a rich cell reads as overgrown rather than as merely brighter.
        let pack = &self.pack;
        let species = &self.species;
        let budgets = &self.budgets;
        let scratch = &mut self.scratch;
        for (index, cell) in CellId::all().enumerate() {
            // The growth this frame shows: between the last two observed states, at `f`.
            let growth = growth_between(self.growth_prev[index], self.growth[index], f);
            if growth.from.is_none() && growth.to.is_none() {
                continue;
            }
            let band = self.bands[index];
            let slot = &self.slots[index];
            let Some(plant) = species.index(band, slot.pick).map(|i| &pack.plants[i]) else {
                continue;
            };
            let t = plant_density(view, index, band);
            let thresholds = stage_thresholds(band);
            let ceiling = band_opacity(band);
            let opacity_of = |stage: u8| stage_opacity(stage, t, &thresholds, ceiling);
            // The accent is gated by the field this frame is drawn against: the paced value
            // only ever fades it *in*.
            let fruit_now = if fruit_stage(fruit.and_then(|f| f.get(index).copied())) {
                growth.fruit
            } else {
                0.0
            };
            // The shared breeze, once for this slot this frame: a side-face plant bends
            // along its own tile's horizontal axis, a radial top-face plant turns in place,
            // and a species with no response takes neither.
            let (bend, heading) =
                slot_wind(slot, &plant.name, budget_in(budgets, &plant.name), seconds);
            if growth.from == growth.to {
                // Idle: one stage, whole, with the fruit accent blended in where it holds.
                let stage = growth.to.expect("an idle bare slot was skipped above");
                let opacity = opacity_of(stage);
                if opacity > 0.0 {
                    let layers = stage_layers(plant, stage, cell, seconds, fruit_now);
                    stamp_layers_bent(
                        canvas,
                        slot.at,
                        heading,
                        &layers,
                        1.0,
                        opacity,
                        Mask::None,
                        bend,
                        scratch,
                    );
                }
                continue;
            }
            // In flight. Where the pack carries an authored growth clip for this step that
            // clip *is* the picture, blended into the two idle stage clips at its ends;
            // otherwise the lower stage fades out under the higher one, which is revealed
            // along the stalk on a side face and outward from its centre on the top face.
            let Some(GrowthStep {
                lower,
                upper,
                t: gu,
            }) = growth_step(growth)
            else {
                continue;
            };
            if let Some((low, clip)) =
                lower.and_then(|low| plant.transition(low, upper).map(|clip| (low, clip)))
            {
                let under = opacity_of(low);
                let opacity = under + (opacity_of(upper) - under) * gu as f32;
                if opacity > 0.0 {
                    let [w_from, w_grow, w_to] = growth_weights(gu);
                    // One stamp: the step's own art, held between the two idle clips. A
                    // layer at weight 0 is not sampled, so each end of the step reads one
                    // clip and costs one.
                    let layers = [
                        (stage_pose(plant, low, cell, seconds), w_from),
                        (clip.sample(gu * clip.seconds), w_grow),
                        (stage_pose(plant, upper, cell, seconds), w_to),
                    ];
                    stamp_layers_bent(
                        canvas,
                        slot.at,
                        heading,
                        &layers,
                        1.0,
                        opacity,
                        Mask::None,
                        bend,
                        scratch,
                    );
                }
                continue;
            }
            if let Some(stage) = lower {
                let opacity = opacity_of(stage) * (1.0 - gu) as f32;
                if opacity > 0.0 {
                    let layers = stage_layers(plant, stage, cell, seconds, fruit_now);
                    stamp_layers_bent(
                        canvas,
                        slot.at,
                        heading,
                        &layers,
                        1.0,
                        opacity,
                        Mask::None,
                        bend,
                        scratch,
                    );
                }
            }
            {
                let stage = upper;
                let opacity = opacity_of(stage);
                if opacity > 0.0 {
                    let layers = stage_layers(plant, stage, cell, seconds, fruit_now);
                    let mask = match up_of(cell) {
                        // A stalk stands on the tile's bottom edge and grows upward.
                        Some(_) => Mask::Axial {
                            reveal: gu * PLANT_REVEAL_PX,
                        },
                        // A radial top-face plant opens from its centre.
                        None => Mask::Radial {
                            reveal: gu * (layers_extent(&layers) + 0.5),
                        },
                    };
                    stamp_layers_bent(
                        canvas, slot.at, heading, &layers, 1.0, opacity, mask, bend, scratch,
                    );
                }
            }
        }

        // Tall plants: columns of base, trunks and crown up the side faces, the crown of a
        // full column carried onto the top face by the shared surface.
        {
            let tall = &self.tall_species;
            for (i, column) in self.columns.iter().enumerate() {
                let Some(plant) = tall.plants[column.pick].map(|p| &pack.tall[p]) else {
                    continue;
                };
                let vine = tall.vine.map(|v| &pack.tall[v]);
                let height = tall_between(self.tall_prev[i], self.tall[i], f).height;
                // One wind sample at the column's base anchor, one amplitude for every part
                // of it, bounded by the family's budget and the vine's together.
                let budget = {
                    let own = budget_in(budgets, TALL_PLANTS[column.pick]);
                    if column.vine {
                        own.min(budget_in(budgets, VINE_PLANT))
                    } else {
                        own
                    }
                };
                let amplitude = tall_amplitude(column, budget, seconds);
                draw_column(
                    canvas, column, height, plant, vine, seconds, amplitude, scratch,
                );
            }
        }

        // Rain: the streaks at this frame's own instant, so they fall continuously.
        if !view.rain.is_empty() {
            draw_rain(canvas, &view.rain, seconds);
        }

        // Bodies: the organism's real state, cross-faded for `BODY_FADE_SECONDS` after a
        // change so a body does not cut from one clip to another. Every clip in the fade
        // keeps its own temporal blend ([`Clip::sample`]); the layers are mixed in one stamp.
        // A hunter member drawn as the Lanternjaw is skipped here: each body is drawn once.
        for o in &view.organisms {
            if self.hunters.contains_key(&o.id) {
                continue;
            }
            let meal = if self.meal_onset {
                self.meals.bout(o.id, seconds, f)
            } else {
                None
            };
            stamp_creature(
                &self.pack,
                &mut self.scratch,
                o,
                self.bodies.get(&o.id),
                meal,
                seconds,
                f,
                canvas,
            );
        }

        // A prey the world removed at this tick's boundary while its hunter entered
        // `Handling`: still on screen for the frames before that boundary, carried from its
        // last published pose to the settlement position the `Capture` event names
        // ([`HunterMemory::retained_prey_pose`]), gone at `f = 1`.
        if f < 1.0 {
            for memory in self.hunters.values() {
                if let (Some(prey), Some((pos, heading))) =
                    (&memory.prey, memory.retained_prey_pose(f))
                {
                    let held = OrganismView {
                        pos,
                        heading,
                        moved: Vec::new(),
                        ..prey.clone()
                    };
                    let outgoing = self
                        .outgoing_prey
                        .get(&prey.id)
                        .filter(|p| p.tick == view.tick);
                    let meal = if self.meal_onset {
                        outgoing.and_then(|p| p.meal).and_then(|m| {
                            // The previous completed tick's weight, not its interpolation
                            // start. No new intake or meal fade is inferred for removed prey.
                            let weight = m.weight_at(1.0);
                            if weight > 0.0 {
                                m.bout_seconds(seconds).map(|t| (t, weight))
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };
                    stamp_creature(
                        &self.pack,
                        &mut self.scratch,
                        &held,
                        outgoing.and_then(|p| p.body.as_ref()),
                        meal,
                        seconds,
                        1.0,
                        canvas,
                    );
                }
            }
        }

        // The hunters, in id order, over the ordinary bodies: the living pose from the
        // adapter's memory of the world's own phases, the root along the same interpolated
        // path and turn every body uses, the whole rig at the authoritative scale.
        for (id, memory) in &self.hunters {
            let Some(o) = view.organisms.iter().find(|o| o.id == *id) else {
                continue;
            };
            let (anchor, dir) = present::interpolate(&o.moved, o.pos, o.heading, f);
            let heading = turn_heading(dir, self.bodies.get(id).map_or(0.0, |m| m.turn), f);
            let (pose, scale) = memory.living_pose(view.tick, f, &o.moved);
            self.lanternjaw.draw_living(
                canvas,
                anchor,
                heading,
                &pose,
                scale,
                1.0,
                &mut self.hunter_parts,
                &mut self.scratch,
            );
        }
    }
}

/// One ordinary creature at fraction `f` of its tick: exactly the stamp the body loop has
/// always made, with `memory` supplying the cross-fade and the turn (none for a body with
/// no memory) and `meal` the bout-time reading of the feed clip ([`Meals::bout`]: seconds
/// into the bout and its weight `w`). With a meal, the mode-driven layers (the state and its
/// cross-fade, exactly as without one) are scaled by `1 − w` and the feed clip read at bout
/// time is added at `w`, so a bout holds the feed loop through the one-tick `Seeking` gaps
/// between nibbles and hands back to the mode's own clip as `w` falls; the path, heading,
/// turn and scale are untouched. Gestation immediately fades out any outgoing meal;
/// no new meal starts while an organism holds an escrow.
fn stamp_creature(
    pack: &ArtPack,
    scratch: &mut Vec<PixelImage>,
    o: &OrganismView,
    memory: Option<&BodyMemory>,
    meal: Option<(f64, f32)>,
    seconds: f64,
    f: f64,
    canvas: &mut Canvas,
) {
    let form = rig_of(o.form, o.hue, pack.creature_count());
    let state = state_of(o);
    let pose_of = |st: usize| {
        let clip = &pack.clips[form * 4 + st];
        // The bud clip's last frame *is* the birth, so a body that has just stopped
        // budding fades out of that frame rather than out of a half-grown bud.
        let gestation = if st == 3 && st != state {
            Some(1.0)
        } else {
            o.gestation
        };
        clip.sample(clip_time(
            clip,
            seconds,
            phase_of(o.id, clip.seconds),
            gestation,
        ))
    };
    let (anchor, dir) = present::interpolate(&o.moved, o.pos, o.heading, f);
    // The turn the tick began with is spent over the tick's frames.
    let heading = turn_heading(dir, memory.map_or(0.0, |m| m.turn), f);
    let scale = if o.juvenile { JUVENILE_SCALE } else { 1.0 };
    let states = match memory {
        Some(memory) if memory.fade_at(seconds) < 1.0 => memory.layers_at(seconds),
        _ => vec![(state, 1.0)],
    };
    let mut layers: Vec<(Pose<'_>, f32)> = Vec::with_capacity(states.len() + 1);
    let meal = match meal {
        Some((bout, mw)) if mw > 0.0 => Some((bout, mw.min(1.0))),
        _ => None,
    };
    let mode_weight = meal.map_or(1.0, |(_, mw)| 1.0 - mw);
    if mode_weight > 0.0 {
        for &(st, w) in &states {
            layers.push((pose_of(st), w * mode_weight));
        }
    }
    if let Some((bout, mw)) = meal {
        layers.push((pack.clips[form * 4 + FEED_STATE].sample(bout), mw));
    }
    stamp_layers(
        canvas,
        anchor,
        heading,
        &layers,
        scale,
        1.0,
        Mask::None,
        scratch,
    );
}

/// The clip index [`state_of`] gives a `Feeding` organism: the authored `feed` loop.
pub const FEED_STATE: usize = 2;

/// Add one ground layer to the image, each pixel scaled by `1 − `[`w_soil`].
///
/// At a pixel wholly above the horizon the scale is exactly 1, so the multiply and the
/// add reproduce the decided image bit for bit; at a pixel wholly in the soil the layer
/// is skipped entirely.
fn add_above_horizon(canvas: &mut Canvas, layer: &Canvas) {
    let weight = &*SOIL_WEIGHT;
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let k = 1.0 - weight[weight_index(face, x, y)];
                if k <= 0.0 {
                    continue;
                }
                let c = layer.get(face, x, y);
                if c == [0.0; 3] {
                    continue;
                }
                canvas.add(face, x, y, [c[0] * k, c[1] * k, c[2] * k]);
            }
        }
    }
}

/// The soil ground: for every pixel with any soil in it, the detritus field ramped from
/// [`SOIL_LOW_SRGB`] to [`SOIL_HIGH_SRGB`] against [`SOIL_SCALE`], at brightness
/// `SOIL_MIN_BRIGHTNESS + (SOIL_MAX_BRIGHTNESS − SOIL_MIN_BRIGHTNESS) · t`, scaled by
/// [`w_soil`].
///
/// Two things differ from [`crate::present::draw_ramp_field`] on purpose. The brightness
/// is linear in `t` rather than squared, because soil is ground rather than a highlight
/// that should only appear when rich. And `t = 0` still paints: bare soil is the dark
/// plum at [`SOIL_MIN_BRIGHTNESS`], which is what makes the band read as a *place* below
/// the horizon instead of as an unlit strip. The seam-aware one-pixel box filter is
/// exactly `cubarium_render::draw_field`'s, rim normalization included.
fn draw_soil_ground(canvas: &mut Canvas, detritus: &ScalarField) {
    let (low, high) = *SOIL_RAMP;
    let weight = &*SOIL_WEIGHT;
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let w = weight[weight_index(face, x, y)];
                if w <= 0.0 {
                    continue;
                }
                let t = filtered_at(detritus, face, x, y) / SOIL_SCALE;
                // A `NaN` cell paints bare soil rather than nothing or a panic.
                let t = if t.is_nan() {
                    0.0
                } else {
                    t.clamp(0.0, 1.0) as f32
                };
                let c = present::mix(low, high, t);
                let b = (SOIL_MIN_BRIGHTNESS + (SOIL_MAX_BRIGHTNESS - SOIL_MIN_BRIGHTNESS) * t) * w;
                canvas.add(face, x, y, [c[0] * b, c[1] * b, c[2] * b]);
            }
        }
    }
}

/// Compile-time reminder that the slot table is one entry per field cell.
const _: () = assert!(CELL_COUNT == 1280);

#[cfg(test)]
mod tests;
