//! Consumers on the voxel strip. One species — the **frondgrazer**, a ground browser that
//! stands on a support face, crops the foliage it can reach, walks toward the best food it
//! can sense, and pays for every unit of it — living as [`Animal`]s beside a
//! [`cubarium_voxel_flora::Flora`] on a [`cubarium_voxel::World`].
//!
//! This crate never draws, never reads the clock and never touches the world's stores: it
//! reads the world through [`cubarium_voxel::VoxelView`] (support faces, heights, water
//! depth, the `x` wrap) and changes the plant layer only through round 5a's bounded
//! transfers — [`cubarium_voxel_flora::Flora::take_foliage`] and
//! [`cubarium_voxel_flora::Flora::deposit`], with
//! [`cubarium_voxel_flora::FloraView::reachable_foliage`] as the geometry of what a
//! browser standing somewhere can actually get at.
//!
//! # What this layer is, and is not
//!
//! It is a **heuristic** animal, not a policy and not the cube's hunter: there is no
//! network, no learning, no drive system and no game hook. Every quantity is paid out of a
//! stock this layer holds, and every unit that leaves it is a named boundary flow of
//! [`FaunaLedger`], so the plant layer's `consumed_*` and `deposited_*` and this layer's
//! `eaten_*` and `deposited_*` are the same transfers seen from two sides and the two
//! ledgers close together.
//!
//! It says **nothing** about populations, carrying capacity or viability. Astra's round 7
//! left those unclaimed on purpose and this round does not claim them: what a run can show
//! is that the flows are paid and observable.
//!
//! # Stated assumptions of this round
//!
//! 1. **No mating system.** An adult that has the reserve pays the whole `birth_cost`
//!    itself and a newborn appears beside it. One specimen is therefore a lineage here,
//!    which `design/theoretical-biosphere-2026-09-16.md` says a population is not; a
//!    two-parent contract (encounter, pairing, gestation) is a later round and not a
//!    missing line of code.
//! 2. **Growth is unrated and unpriced.** Assimilated matter fills the body up to
//!    `body_max` and then the reserve up to `reserve_cap · body`; there is no growth rate
//!    and no construction cost, because [`SpeciesConfig::yield_fraction`] already carries
//!    the loss between what was eaten and what was built.
//! 3. **Dung is mineral only.** The undigested fraction of a bite is *respired*, per the
//!    round's brief, and what is excreted is the mineral a bite carried in excess of the
//!    tissue it built — as a litter deposit, because litter is where this round puts dung.
//! 4. **No sub-tick motion.** An animal is on one support face per tick. How a body moves
//!    between faces is the art thread's question (`design/voxel-art-direction.md`, when it
//!    lands), and `pace-in-body-lengths` applies to it then, not here.
//! 5. **The interim look is not the look.** The presenter draws a 2×1×2 block in a
//!    placeholder colour (`crates/cubarium/src/voxel/animal.rs`), named interim in the
//!    code, until the art direction says what a consumer looks like.
//!
//! Every number in [`SpeciesConfig::frondgrazer`] is an untuned **placeholder** listed in
//! `design/backlog.md` §1. Nothing here is tuned and no result in this round depends on a
//! value.
//!
//! # The phase-one founder path (P1-B)
//!
//! A body carrying a [`Founder`] marker is not run by the heuristic at all. It runs the
//! phase-one body model instead: a [`Controller`] is sampled at the tick's controller
//! stage ([`step`]'s order), its three bounded actions are held, and the body resolves
//! paid heading motion, contact/taste receptors and local feeding through real
//! transfers — [`body`]'s and [`controller`]'s docs carry the rules. The live heuristic
//! path above is untouched by that: `founder: None` is the frondgrazer, unchanged, and
//! a founder with no controller installed rests exactly as it did in P1-A.

#![forbid(unsafe_code)]

mod body;
mod controller;
mod manifest;
mod pose;
mod senses;
mod snapshot;
mod step;

use cubarium_voxel::{VoxelView, World};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraView, Site, Taken};
use serde::{Deserialize, Serialize};

pub use body::{
    FounderPhysiology, effective_config, has_headroom, headroom_voxels, mouth_reach_up_voxels,
};
pub use controller::{
    Actions, BlindForager, BrowserForager, Controller, ControllerFactory, FounderControllers,
    FounderFactories, Response, Scripted, resolve_actions,
};
pub use cubarium_voxel::{DT, TICK_HZ};
pub use cubarium_voxel_flora::Reach;
pub use manifest::{
    ACTION_DEADBAND, Action, BROWSER_RAY_PITCH_OFFSETS_DEG, BROWSER_RAY_YAW_OFFSETS_DEG,
    BROWSER_SECTOR_CENTRES_DEG, BROWSER_VISIBLE_CLASSES, Founder, HIDDEN, Manifest, Module,
    SCHEMA_VERSION, Transfer, Tunings, gru32_parameter_count,
};
pub use pose::Pose;
pub use senses::{Senses, UPDATE_TICKS};
pub use snapshot::SCHEMA;

/// The browser's three cone sectors as `(foliage_fraction, foliage_proximity)` pairs.
/// This is a read-only diagnostic projection of the same ray result the founder
/// controller receives; it exists for autopsies and does not participate in stepping.
pub fn browser_cone_readings(
    view: &VoxelView<'_>,
    flora: &FloraView<'_>,
    fauna: &FaunaView<'_>,
    animal: &Animal,
) -> Option<[(f64, f64); 3]> {
    if animal.founder != Some(Founder::Browser) {
        return None;
    }
    let occupancy = senses::cone_occupancy(view, flora, fauna);
    let reading = senses::cone_readings(
        view,
        &occupancy,
        animal.id,
        &animal.pose,
        animal.site.y,
        &Founder::Browser.manifest(),
    );
    reading.valid.then_some(
        reading
            .sectors
            .map(|sector| (sector.foliage_fraction, sector.foliage_proximity)),
    )
}

/// What one cone ray struck first, at the resolution the policy is **not** given.
///
/// [`browser_cone_readings`] reports a sector's `foliage_fraction`; everything that is
/// not foliage and not a body collapses into one occluding class, so a cone that reads
/// no foliage is indistinguishable from a cone facing a slope, a puddle, a shower film,
/// a crown someone already stripped, or a litter pool. This is that same first hit named
/// (`design/handoffs/voxel-cone-autopsy-2026-09-21.md`, deliverable 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConeHit {
    /// Nothing within the cone's range, or the ray left the world.
    Clear,
    /// A solid voxel.
    Terrain,
    /// A cell holding free water.
    Water,
    /// A crown cell of a stand whose foliage is zero.
    StrippedCrown,
    /// A crown cell with foliage standing in it — the only hit the policy reads as food.
    FoliageCrown,
    /// The cell over a ground site holding litter, carrion or dead wood.
    GroundPool,
    /// Another animal's body.
    Body,
}

impl ConeHit {
    /// Its name, for a CSV column.
    pub const fn name(self) -> &'static str {
        match self {
            ConeHit::Clear => "clear",
            ConeHit::Terrain => "terrain",
            ConeHit::Water => "water",
            ConeHit::StrippedCrown => "stripped",
            ConeHit::FoliageCrown => "foliage",
            ConeHit::GroundPool => "pool",
            ConeHit::Body => "body",
        }
    }

    /// Every variant, in the order a census row prints them.
    pub const ALL: [ConeHit; 7] = [
        ConeHit::Clear,
        ConeHit::Terrain,
        ConeHit::Water,
        ConeHit::StrippedCrown,
        ConeHit::FoliageCrown,
        ConeHit::GroundPool,
        ConeHit::Body,
    ];

    /// Its index in [`ConeHit::ALL`].
    pub const fn index(self) -> usize {
        match self {
            ConeHit::Clear => 0,
            ConeHit::Terrain => 1,
            ConeHit::Water => 2,
            ConeHit::StrippedCrown => 3,
            ConeHit::FoliageCrown => 4,
            ConeHit::GroundPool => 5,
            ConeHit::Body => 6,
        }
    }
}

/// One ray of the census: where it was aimed and what it found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConeRay {
    /// Sector index in manifest order, or [`ConeRay::PROBE`] for a caller's bearing.
    pub sector: usize,
    /// Yaw from the body's heading, degrees — sector centre plus the ray's own offset
    /// for a fan ray, the bearing itself for a probe.
    pub yaw_deg: f64,
    /// Pitch from horizontal, degrees.
    pub pitch_deg: f64,
    pub hit: ConeHit,
    /// Distance in metres, or [`f64::INFINITY`] for [`ConeHit::Clear`].
    pub distance_m: f64,
}

impl ConeRay {
    /// The `sector` of a ray the caller aimed rather than one of the fan's.
    pub const PROBE: usize = usize::MAX;
}

/// The browser's cone, ray by ray, at the fine resolution.
#[derive(Clone, Debug, PartialEq)]
pub struct ConeCensus {
    /// The eye, in metres: the body's column, one and a half **voxels** over its
    /// standing face.
    pub origin_m: (f64, f64, f64),
    pub heading_rad: f64,
    pub range_m: f64,
    /// The manifest's own fan, in `cone_readings`' accumulation order: sector, then yaw
    /// offset, then pitch offset.
    pub rays: Vec<ConeRay>,
    /// One ray per bearing the caller asked for, in the order asked.
    pub probes: Vec<ConeRay>,
}

impl ConeCensus {
    /// How many fan rays of one sector struck each class, indexed by
    /// [`ConeHit::index`].
    pub fn counts(&self, sector: usize) -> [u32; 7] {
        let mut out = [0u32; 7];
        for ray in self.rays.iter().filter(|r| r.sector == sector) {
            out[ray.hit.index()] += 1;
        }
        out
    }

    /// The mean distance of this sector's [`ConeHit::FoliageCrown`] hits, or `None` when
    /// it has none.
    pub fn nearest_foliage_mean_m(&self, sector: usize) -> Option<f64> {
        let hits: Vec<f64> = self
            .rays
            .iter()
            .filter(|r| r.sector == sector && r.hit == ConeHit::FoliageCrown)
            .map(|r| r.distance_m)
            .collect();
        (!hits.is_empty()).then(|| hits.iter().sum::<f64>() / hits.len() as f64)
    }
}

/// Run the browser's **exact** cone from its exact eye and name every first hit.
///
/// It is the same occupancy map, the same origin, the same ray directions, the same
/// sub-step and the same step cap the controller's own observation uses — the geometry
/// lives in one place (`senses::cone_origin`, `senses::ray_direction`,
/// `senses::ray_first_hit`) and both callers read it — so a census row explains the
/// reading the policy was given rather than a second opinion about it. What it adds is
/// only resolution: `senses::Fine::coarse` is the one mapping back, and a test asserts
/// the census reproduces [`browser_cone_readings`] exactly.
///
/// `probes` are extra bearings in radians, `(yaw_from_heading, pitch)`, for asking
/// "is the crown over there blocked, and by what" — they are marched with the same ray
/// and returned separately, and they change nothing.
///
/// `None` if the animal is not a browser founder or its eye is invalid where it stands.
pub fn browser_cone_census(
    view: &VoxelView<'_>,
    flora: &FloraView<'_>,
    fauna: &FaunaView<'_>,
    animal: &Animal,
    probes: &[(f64, f64)],
) -> Option<ConeCensus> {
    if animal.founder != Some(Founder::Browser) {
        return None;
    }
    let manifest = Founder::Browser.manifest();
    if !senses::cone_valid(view, &animal.pose, animal.site.y, &manifest) {
        return None;
    }
    let occupancy = senses::cone_occupancy(view, flora, fauna);
    let origin = senses::cone_origin(view, &animal.pose, animal.site.y);
    let heading = animal.pose.heading_rad;
    let range = manifest.cone_range_m;
    let march = |sector: usize, yaw_deg: f64, pitch_deg: f64, dir: (f64, f64, f64)| {
        let (hit, distance_m) =
            match senses::ray_first_hit(view, &occupancy, animal.id, origin, dir, range) {
                None => (ConeHit::Clear, f64::INFINITY),
                Some((d, fine)) => (
                    match fine {
                        senses::Fine::Terrain => ConeHit::Terrain,
                        senses::Fine::Water => ConeHit::Water,
                        senses::Fine::StrippedCrown => ConeHit::StrippedCrown,
                        senses::Fine::FoliageCrown => ConeHit::FoliageCrown,
                        senses::Fine::GroundPool => ConeHit::GroundPool,
                        senses::Fine::Body => ConeHit::Body,
                    },
                    d,
                ),
            };
        ConeRay {
            sector,
            yaw_deg,
            pitch_deg,
            hit,
            distance_m,
        }
    };
    let mut rays = Vec::with_capacity(
        manifest.sector_centres_deg.len()
            * manifest.ray_yaw_offsets_deg.len()
            * manifest.ray_pitch_offsets_deg.len(),
    );
    for (si, &centre_deg) in manifest.sector_centres_deg.iter().enumerate() {
        for &oyaw in manifest.ray_yaw_offsets_deg {
            for &opitch in manifest.ray_pitch_offsets_deg {
                let dir = senses::ray_direction(heading, centre_deg, oyaw, opitch);
                rays.push(march(si, centre_deg + oyaw, opitch, dir));
            }
        }
    }
    let probes = probes
        .iter()
        .map(|&(yaw, pitch)| {
            let dir = senses::ray_direction(heading, yaw.to_degrees(), 0.0, pitch.to_degrees());
            march(ConeRay::PROBE, yaw.to_degrees(), pitch.to_degrees(), dir)
        })
        .collect();
    Some(ConeCensus {
        origin_m: origin,
        heading_rad: heading,
        range_m: range,
        rays,
        probes,
    })
}

/// The stand selected by the browser's actual mouth geometry, if any. Read-only
/// diagnostic access for explaining a bite or a starvation event.
pub fn browser_mouth_foliage(
    view: &VoxelView<'_>,
    flora: &FloraView<'_>,
    animal: &Animal,
) -> Option<(Site, f64)> {
    if animal.founder != Some(Founder::Browser) {
        return None;
    }
    let manifest = Founder::Browser.manifest();
    let cols = body::mouth_columns(view, &animal.pose, &manifest);
    body::mouth_foliage_stand(flora, view, &cols, animal.site.y, &manifest)
}

/// Every stand whose crown touches the browser's actual mouth probe columns.
/// Read-only diagnostic access for distinguishing a nearby crown from edible contact.
pub fn browser_mouth_candidates(
    view: &VoxelView<'_>,
    flora: &FloraView<'_>,
    animal: &Animal,
) -> Option<Vec<(Site, f64)>> {
    if animal.founder != Some(Founder::Browser) {
        return None;
    }
    let manifest = Founder::Browser.manifest();
    let cols = body::mouth_columns(view, &animal.pose, &manifest);
    Some(body::mouth_foliage_stands(
        flora,
        view,
        &cols,
        animal.site.y,
        &manifest,
    ))
}

/// The consumers of the voxel ecology. One, so far: see
/// [`SpeciesConfig::frondgrazer`] for the sentence of ecology its numbers encode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Species {
    /// The paid ground browser of the grazed meadow: crops reachable foliage, walks to
    /// better food, drowns in standing water it cannot wade.
    Frondgrazer,
}

impl Species {
    pub const ALL: [Species; 1] = [Species::Frondgrazer];
    pub const COUNT: usize = Species::ALL.len();

    /// Index into a per-species array. Stable, like the flora's.
    pub fn index(self) -> usize {
        match self {
            Species::Frondgrazer => 0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Species::Frondgrazer => "frondgrazer",
        }
    }

    /// The name a command line or a stdin line gives, or `None`.
    pub fn parse(s: &str) -> Option<Species> {
        Species::ALL
            .into_iter()
            .find(|sp| sp.name().eq_ignore_ascii_case(s))
    }
}

/// What an animal did on the tick just stepped. A record of the act, not a mode it is put
/// into: the next tick reads its stocks and its surroundings again from scratch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    /// It had at least one bite within reach and took it.
    Cropping,
    /// It had nothing in reach, something in sense, and a face strictly closer to it that
    /// it could climb and wade. Also the state of an animal that wanted to walk and was
    /// not due a step this tick.
    Walking,
    /// Nothing in reach and nothing in sense, or nowhere closer it could stand.
    Resting,
}

/// One animal: an identity, the support face it stands on, and the three currencies it
/// holds.
///
/// `body` is structure and `reserve` is what maintenance is paid out of; `mineral` and
/// `energy` are the stocks that came in with its food. Energy is a **stock and not
/// `energy_density · organic`** (the flora's dead pools are kept the same way and for the
/// same reason): a bite carries the energy the plant layer handed over at *its* density,
/// so holding that number is what makes the energy residual close for any food, instead of
/// requiring this species' own density to be no greater than its food's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animal {
    /// This animal's identity, from [`FaunaLedger::births`]: every animal this layer has
    /// ever created has its own, introduced ones included, and it never changes or
    /// repeats.
    pub id: u64,
    pub species: Species,
    /// The support face it stands on. Its body is drawn in the void above this.
    pub site: Site,
    /// The continuous place and heading on that support layer. The live heuristic
    /// browser treats `site` as authoritative and re-anchors the pose to each face it
    /// steps to; a **founder body is the other way round** — its pose is authoritative
    /// and `site` follows the pose's column at the standing layer ([`step`]'s founder
    /// path moves the pose and re-derives the site).
    pub pose: Pose,
    /// `Some` for a phase-one founder: the lineage whose manifest its controller will be
    /// authored against. P1-B runs the founder's local actions: held controller actions,
    /// paid heading motion, contacts and local feeding. `None` is the live frondgrazer
    /// heuristic path, unchanged.
    pub founder: Option<Founder>,
    /// The founder's held actions and prior-interval feedback. All-default for a
    /// heuristic body (`founder: None`), which never reads it.
    pub founder_state: FounderState,
    /// Where this body is in its own reproductive cycle: the surplus it has held, the
    /// refractory left after its last offspring, and the escrow it is filling.
    ///
    /// **On the animal and not in [`FounderState`]**, which the reproduction brief
    /// suggested, because the rule is not a founder's: a body with no lineage runs the
    /// same eligibility, hold and gestation ([`Reproduction::LIVE_BIRTH_PLACEHOLDER`]),
    /// and `FounderState` is documented as the controller's interval state that a
    /// heuristic body never reads.
    pub reproduction: ReproductionState,
    /// Organic structure. Below `body_min` it is dead.
    pub body: f64,
    /// Organic matter maintenance is paid from before the body is.
    pub reserve: f64,
    /// The mineral in its tissue: an inventory, moved by the same fraction rule the plant
    /// layer uses for every outflow, never created and never respired.
    pub mineral: f64,
    /// The energy in its tissue, as its food handed it over.
    pub energy: f64,
    /// Ticks since it was born or introduced. Also its step phase: an animal steps on the
    /// ticks where `age_ticks` is a multiple of its species' step period, so two animals
    /// born on different ticks do not step in lockstep.
    pub age_ticks: u64,
    pub state: State,
}

impl Animal {
    /// Organic matter in the animal: structure plus reserve. What a corpse deposits —
    /// a gestation is always resolved before a body dies, so its escrow is back in the
    /// reserve by the time this is read for a corpse.
    pub fn organic(&self) -> f64 {
        self.body + self.reserve
    }

    /// Organic matter this body **holds**, the escrowed offspring included: what the
    /// layer's stored total counts, and what leaves with a body an
    /// [`Command::Remove`] or a collapsing floor takes out of the world.
    ///
    /// The escrow is not the parent's to spend — no rule reads it as reserve, and
    /// maintenance cannot touch it — but it is still in the parent until it is born,
    /// so a conservation check that ignored it would see matter vanish at the first
    /// instalment.
    pub fn stored_organic(&self) -> f64 {
        self.organic() + self.reproduction.escrow.map_or(0.0, |e| e.organic)
    }

    /// Mineral in this body, the escrowed offspring's included.
    pub fn stored_mineral(&self) -> f64 {
        self.mineral + self.reproduction.escrow.map_or(0.0, |e| e.mineral)
    }

    /// Energy in this body, the escrowed offspring's included.
    pub fn stored_energy(&self) -> f64 {
        self.energy + self.reproduction.escrow.map_or(0.0, |e| e.energy)
    }
}

/// Where a body is in its own reproductive cycle ([`step`]'s step 7).
///
/// Three pieces of state, one per clause of the rule: how long the surplus has stood,
/// how long the body must wait after its last offspring, and the package it is
/// currently paying for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReproductionState {
    /// Consecutive ticks the body has been **eligible** — structure at `birth_body` and
    /// a reserve at or above the offspring package plus `surplus_floor`. Any tick below
    /// resets it to zero, which is what makes the hold a *sustained* surplus and not a
    /// momentary one.
    pub surplus_ticks: u64,
    /// Ticks of refractory still owed after the last birth or laying. Nothing about
    /// eligibility advances while this is above zero.
    pub refractory_ticks: u64,
    /// The offspring package being filled, for a gestating body. `None` is a body that
    /// is not gestating; an egg-laying body never has one.
    pub escrow: Option<Escrow>,
}

/// An offspring package the parent has already paid for and cannot spend.
///
/// The three currencies leave the parent's reserve in equal instalments over
/// `gestation_s` and sit here until the birth builds a body out of them. The escrow
/// pays no upkeep of its own and the parent's maintenance is unchanged: it is not a
/// second animal yet, and it is not the parent's reserve any more.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Escrow {
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
    /// Instalments paid so far. The birth is at `gestation_ticks`.
    pub ticks: u64,
}

/// A clutch of eggs: a **stationary paid package in the world**, on the support face
/// the parent laid it on.
///
/// It is not an animal — it has no body, no controller, no pose and no upkeep — and it
/// is not a plant-layer pool either: the material is still the animal layer's, and the
/// layer's stored totals count it, until the eggs hatch into bodies or the site takes
/// them. Nothing eats a clutch yet; the record carries its site and its three
/// currencies so that a consumer can be added later without changing what an egg *is*.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clutch {
    /// The support face the eggs lie on.
    pub site: Site,
    /// Eggs still in the clutch. They hatch together.
    pub count: u32,
    /// The clutch's organic matter, all eggs together. One egg is `organic / count`.
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
    /// The tick the clutch was laid; incubation is counted from it.
    pub laid_tick: u64,
    /// The lineage that laid it, which is the lineage its hatchlings carry and the
    /// factory their controllers come from.
    pub lineage: Founder,
}

impl Clutch {
    /// What one egg holds: the clutch split `count` ways.
    pub fn egg(&self) -> Taken {
        let n = f64::from(self.count.max(1));
        Taken {
            organic: self.organic / n,
            mineral: self.mineral / n,
            energy: self.energy / n,
        }
    }
}

/// How a lineage puts its offspring into the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BirthMode {
    /// Live birth out of a gestation escrow: one offspring at a time, at the parent's
    /// pose, at term.
    Gestation,
    /// A clutch of eggs laid on the litter the parent lives in, hatching where they lie.
    Eggs,
}

/// A lineage's reproduction: **every number here is an untuned placeholder**
/// (`design/backlog.md` §1, `design/handoffs/voxel-reproduction-2026-09-21.md`), chosen
/// to encode "births after a sustained surplus, one at a time, with an interval" at the
/// scale the frozen physiology already sets. Nothing measured any of them and there was
/// no tuning loop.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reproduction {
    pub mode: BirthMode,
    /// Reserve a body must hold **above** the offspring package's price before it counts
    /// as being in surplus.
    pub surplus_floor: f64,
    /// Seconds the surplus must stand, unbroken, before anything begins.
    pub surplus_hold_s: f64,
    /// Seconds a gestation takes, over which the escrow is filled in equal instalments.
    /// [`BirthMode::Eggs`] does not read it.
    pub gestation_s: f64,
    /// The share of an interrupted gestation's escrowed **organic matter** that is
    /// respired; the rest returns to the parent, and the mineral all returns, because
    /// respiration in this layer never takes mineral.
    pub gestation_loss_fraction: f64,
    /// Seconds of refractory after a birth or a laying, before a surplus can start
    /// accumulating again.
    pub birth_interval_s: f64,
    /// Eggs in one clutch. [`BirthMode::Gestation`] does not read it.
    pub clutch_size: u32,
    /// Organic matter in **one** egg, which must be at least the lineage's `body_min`:
    /// a hatchling is a `body_min` juvenile with what is left over as its reserve.
    pub egg_organic: f64,
    /// Seconds a clutch incubates before it hatches.
    pub incubation_s: f64,
}

impl Reproduction {
    /// The frondgrazer's gestation placeholders, and the table a body with **no**
    /// lineage runs: the browser founder is the frondgrazer, so the live heuristic
    /// species and the browser lineage reproduce the same way.
    ///
    /// Against the frondgrazer's frozen physiology (`birth_cost` 0.01, a reserve
    /// ceiling of 0.025 at `body_max`): eligibility at 0.015 is 60 % of a full reserve,
    /// so it is a real surplus and not a full tank; the escrow fills at
    /// `0.01 / 180 s` = 5.6e-5 /s against a basal upkeep of 5e-5 /s at `body_max`, so
    /// gestating roughly doubles what the parent is paying out; and hold + gestation +
    /// interval put at least 10 minutes between one parent's offspring.
    pub const LIVE_BIRTH_PLACEHOLDER: Reproduction = Reproduction {
        mode: BirthMode::Gestation,
        surplus_floor: 0.005,
        surplus_hold_s: 120.0,
        gestation_s: 180.0,
        gestation_loss_fraction: 0.25,
        birth_interval_s: 300.0,
        clutch_size: 0,
        egg_organic: 0.0,
        incubation_s: 0.0,
    };

    /// The littershredder's egg placeholders.
    ///
    /// `clutch_size` is **1** and that is a consequence, not a preference: the blind
    /// founder's frozen physiology puts `body_min` at 0.003125 and caps its reserve at
    /// `reserve_cap · body_max` = 0.00625, so a viable egg costs at least a body_min
    /// plus something to hatch with and two of them cannot be paid for out of one
    /// reserve at all. Raising it needs the blind body's reserve ceiling or its
    /// `body_min` revisited, which is a physiology decision and not this round's
    /// (`design/backlog.md` §1). The rule itself is written for any count.
    pub const EGGS_PLACEHOLDER: Reproduction = Reproduction {
        mode: BirthMode::Eggs,
        surplus_floor: 0.000_625,
        surplus_hold_s: 120.0,
        gestation_s: 0.0,
        gestation_loss_fraction: 0.25,
        birth_interval_s: 300.0,
        clutch_size: 1,
        egg_organic: 0.004,
        incubation_s: 300.0,
    };

    /// What one offspring package costs the parent's reserve: a gestation's `birth_cost`
    /// or a whole clutch.
    pub fn package_cost(&self, sc: &SpeciesConfig) -> f64 {
        match self.mode {
            BirthMode::Gestation => sc.birth_cost,
            BirthMode::Eggs => f64::from(self.clutch_size) * self.egg_organic,
        }
    }

    /// The reserve a body has to hold to be in surplus: the package plus the floor.
    pub fn surplus_reserve(&self, sc: &SpeciesConfig) -> f64 {
        self.package_cost(sc) + self.surplus_floor
    }

    /// Whole ticks, never below one, for a duration in seconds.
    fn ticks(seconds: f64) -> u64 {
        let t = (seconds * f64::from(TICK_HZ)).round();
        if !t.is_finite() || t < 1.0 {
            1
        } else {
            t as u64
        }
    }

    pub fn hold_ticks(&self) -> u64 {
        Reproduction::ticks(self.surplus_hold_s)
    }

    pub fn gestation_ticks(&self) -> u64 {
        Reproduction::ticks(self.gestation_s)
    }

    pub fn interval_ticks(&self) -> u64 {
        Reproduction::ticks(self.birth_interval_s)
    }

    pub fn incubation_ticks(&self) -> u64 {
        Reproduction::ticks(self.incubation_s)
    }

    /// Structural refusals only, in [`SpeciesConfig::validate`]'s shape: finite
    /// nonnegative numbers, a loss fraction that is a fraction, and — for an egg layer —
    /// a clutch of at least one egg each of which can build a `body_min` hatchling.
    /// **Not** a plausibility check on the placeholders themselves.
    pub fn validate(&self, name: &str, sc: &SpeciesConfig) -> Result<(), String> {
        let fields: [(&str, f64); 6] = [
            ("surplus_floor", self.surplus_floor),
            ("surplus_hold_s", self.surplus_hold_s),
            ("gestation_s", self.gestation_s),
            ("gestation_loss_fraction", self.gestation_loss_fraction),
            ("birth_interval_s", self.birth_interval_s),
            ("incubation_s", self.incubation_s),
        ];
        for (field, v) in fields {
            if !v.is_finite() || v < 0.0 {
                return Err(format!(
                    "{name}.{field} must be finite and nonnegative, not {v}"
                ));
            }
        }
        if !(self.gestation_loss_fraction <= 1.0) {
            return Err(format!(
                "{name}.gestation_loss_fraction must be at most 1, not {}",
                self.gestation_loss_fraction
            ));
        }
        if self.mode == BirthMode::Eggs {
            if self.clutch_size == 0 {
                return Err(format!("{name}.clutch_size must be at least one egg"));
            }
            if !(self.egg_organic.is_finite() && self.egg_organic >= sc.body_min) {
                return Err(format!(
                    "{name}.egg_organic {} cannot build a hatchling of body_min {}",
                    self.egg_organic, sc.body_min
                ));
            }
        }
        Ok(())
    }
}

/// A phase-one founder's held actions and prior-interval feedback. The heuristic bodies
/// carry it all-default and never read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FounderState {
    /// The actions held from the last due controller sampling, already through the
    /// shared adapter — bounded and deadbanded. Rest until the first sampling: an
    /// unsampled founder holds no action.
    pub held: Actions,
    /// What the last completed interval actually did, consumed and zeroed once at the
    /// next sampling. The `Self` channels are built from this; it never carries a
    /// prediction of success.
    pub feedback: IntervalFeedback,
}

/// What one controller interval actually did, from the body's own ledger events and
/// resolved motion — the prior-interval feedback the observation's `Self` channels are
/// built from, and zeroed once when it is consumed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IntervalFeedback {
    /// Organic matter assimilated into body and reserve over the interval.
    pub intake: f64,
    /// Internal tissue lost over the interval: the share of upkeep and motor
    /// respiration that came out of the body itself once the reserve was empty.
    pub structural_loss: f64,
    /// Requested equivalent displacement over the interval, `|v| + r·|yaw rate|`
    /// summed over the ticks it was held. A wall-constrained attempt is attempted in
    /// full.
    pub attempted_equivalent: f64,
    /// Delivered equivalent displacement over the interval: what the sweep actually
    /// covered plus the turn the body actually made.
    pub delivered_equivalent: f64,
    /// Metres actually covered along the heading over the interval.
    pub delivered_forward: f64,
    /// Radians actually turned over the interval, signed.
    pub delivered_turn: f64,
    /// Organic matter the interval's paid motion respired, reserve and body together —
    /// the per-interval motor-respiration counter, alongside the ledger's cumulative
    /// `respired_motor_out`. Zero for a resting interval.
    pub motor_respiration: f64,
}

/// One species' numbers. **Every one of them is an untuned placeholder**; see
/// [`SpeciesConfig::frondgrazer`] and `design/backlog.md` §1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeciesConfig {
    /// Organic matter respired per unit of body per second, standing still. Paid from the
    /// reserve, then from the body.
    pub maintenance_per_s: f64,
    /// Organic matter a mouth can take per second, spread over the stands in reach in site
    /// order. The bound on what a bite *asks* for; the plant layer bounds what it gets.
    pub bite_per_s: f64,
    /// The fraction of a bite that becomes tissue. The rest is respired, with the energy
    /// that was in it leaving as heat and the mineral staying behind.
    pub yield_fraction: f64,
    /// Mineral per unit of tissue built — the density of what it builds, and therefore the
    /// **budget** its growth fits inside: a bite can build at most `mineral / n_tissue` of
    /// tissue, whatever `yield_fraction` would otherwise have assimilated, and the
    /// unfunded organic matter is respired with its energy (Astra R9.1). Mineral a bite
    /// carries in excess of what was built is excreted as a litter deposit.
    ///
    /// There is **no mineral reserve**: [`Animal::mineral`] is an inventory of the tissue's
    /// own content and growth never draws on it, so a mineral-free bite builds nothing
    /// however much mineral the animal is carrying. At the placeholders the budget binds
    /// on every bite — a plant's foliage holds 0.02 against this 0.05 — so a frondgrazer
    /// turns 40 % of what it eats into tissue and not the 50 % `yield_fraction` names.
    pub n_tissue: f64,
    /// Structure it grows to and no further.
    pub body_max: f64,
    /// Structure below which it is dead. Also a newborn's body.
    pub body_min: f64,
    /// Structure at which it can pay for a birth.
    pub birth_body: f64,
    /// Organic matter a birth costs its parent, all of it out of the reserve. At least
    /// `body_min`, because that is the newborn's body; the remainder is the newborn's
    /// reserve.
    pub birth_cost: f64,
    /// The ceiling on **new intake** into the reserve, per unit of body: assimilated
    /// matter stops going to the reserve at `reserve_cap · body` and is respired instead
    /// ([`step`]'s assimilation). It is deliberately **not** a universal storage bound
    /// (Astra R9.4).
    ///
    /// **The one exception, named.** A paid birth endowment may sit above it. A newborn is
    /// `body_min` of body and `birth_cost − body_min` of reserve, which at the
    /// placeholders is `0.005` against a cap of `reserve_cap · body_min` = `0.0025`: the
    /// parcel is what the parent actually paid for out of its own reserve, so clamping it
    /// would either destroy organic matter or need a conserving birth allocation, which is
    /// a life-history decision and not a repair. While a reserve is above the ceiling no
    /// intake raises it — `room` is zero and the surplus is respired — and maintenance
    /// spends it exactly as it spends any other reserve, so the excess is transient and
    /// never grows.
    pub reserve_cap: f64,
    /// How far it can get at food from the face it stands on.
    pub reach: Reach,
    /// The largest height difference, in voxels, between the face it stands on and a face
    /// it can step onto.
    pub climb: u32,
    /// The deepest standing water, in metres, it will step into.
    pub wade_depth_m: f64,
    /// The depth of standing water on its own face that kills it.
    pub drown_depth_m: f64,
    /// Seconds between steps. Quantised to whole ticks, never below one.
    pub step_period_s: f64,
    /// How far, in voxels of wrapped `x` and of `z` each measured on its own, a **face it
    /// could stand on** may be and still be a candidate for the walk. The domain is the
    /// candidate face's and not the food's (Astra R9.5): a crown is found by scoring the
    /// faces in this box with [`cubarium_voxel_flora::FloraView::reachable_foliage`], so a
    /// stand outside the box whose feeding face is inside it *is* found, and a stand inside
    /// it whose only feeding face is outside is not.
    pub sense_radius: u32,
    /// Energy per unit of organic matter an **introduced** animal arrives with. Used for
    /// nothing else: every other unit of energy in this layer came in with a bite at the
    /// food's own density, and a newborn takes its parent's pro rata.
    pub energy_density: f64,
    /// How a body of this kind puts offspring into the world: the gestation escrow or a
    /// clutch of eggs, with the thresholds and intervals both rules run on
    /// ([`Reproduction`], `design/handoffs/voxel-reproduction-2026-09-21.md`).
    ///
    /// It rides here, inside the founder's own physiology and inside the placeholder
    /// species', because [`effective_config`] is already the one lookup every rule uses
    /// to ask what numbers a body runs on, and reproduction is not a different kind of
    /// question. A body with no lineage therefore reproduces by its species' table, and
    /// the browser founder — which *is* the frondgrazer — shares that table with it.
    pub reproduction: Reproduction,
}

impl SpeciesConfig {
    /// The paid ground browser of the biosphere's grazed meadow
    /// (`design/theoretical-biosphere-2026-09-16.md` §5, branch 1): a small body that
    /// eats low foliage it can stand under, walks to the next patch when its own is bare,
    /// cannot climb a step taller than itself, wades a film and drowns in a pool.
    ///
    /// **Every number is a placeholder and nothing measured any of them.** They are
    /// chosen to encode that sentence at the scale the round-4 presets already set: a
    /// springturf founder carries `α · W` = 0.06 of foliage, so a body of 0.05 and a
    /// mouth of 0.002 /s put one animal and one turf in the same order of magnitude, and
    /// `bite_per_s` is deliberately the same rate as round 5a's harvest probe so that the
    /// two are comparable.
    pub fn frondgrazer() -> SpeciesConfig {
        SpeciesConfig {
            maintenance_per_s: 0.001,
            bite_per_s: 0.002,
            yield_fraction: 0.5,
            n_tissue: 0.05,
            body_max: 0.05,
            body_min: 0.005,
            birth_body: 0.03,
            birth_cost: 0.01,
            reserve_cap: 0.5,
            reach: Reach {
                horizontal: 1,
                up: 1,
            },
            climb: 1,
            wade_depth_m: 0.05,
            drown_depth_m: 0.2,
            step_period_s: 1.0,
            sense_radius: 8,
            energy_density: 2.0,
            reproduction: Reproduction::LIVE_BIRTH_PLACEHOLDER,
        }
    }

    /// Whole ticks between steps, never zero.
    pub fn step_period_ticks(&self) -> u64 {
        let t = (self.step_period_s * f64::from(TICK_HZ)).round();
        if !t.is_finite() || t < 1.0 {
            1
        } else {
            t as u64
        }
    }

    /// The reserve intake into a body of this size stops at: `reserve_cap · body`. A
    /// ceiling on new intake and not a storage bound — a newborn's paid endowment starts
    /// above it (see [`SpeciesConfig::reserve_cap`]) — so a caller must not read this as
    /// "what it is holding" or clamp to it.
    pub fn reserve_of(&self, body: f64) -> f64 {
        self.reserve_cap * body
    }

    /// Structural refusals only, in the shape of [`cubarium_voxel_flora::FloraConfig`]'s:
    /// finite nonnegative numbers, and the four body thresholds in an order that can
    /// build a living animal. **Not** a plausibility check — the placeholders above are
    /// the values known to be arbitrary, and refusing them here would be tuning in a
    /// validator's clothes.
    pub fn validate(&self, name: &str) -> Result<(), String> {
        let fields: [(&str, f64); 12] = [
            ("maintenance_per_s", self.maintenance_per_s),
            ("bite_per_s", self.bite_per_s),
            ("yield_fraction", self.yield_fraction),
            ("n_tissue", self.n_tissue),
            ("body_max", self.body_max),
            ("body_min", self.body_min),
            ("birth_body", self.birth_body),
            ("birth_cost", self.birth_cost),
            ("reserve_cap", self.reserve_cap),
            ("wade_depth_m", self.wade_depth_m),
            ("drown_depth_m", self.drown_depth_m),
            ("energy_density", self.energy_density),
        ];
        for (field, v) in fields {
            if !v.is_finite() || v < 0.0 {
                return Err(format!(
                    "{name}.{field} must be finite and nonnegative, not {v}"
                ));
            }
        }
        if !(self.yield_fraction <= 1.0) {
            return Err(format!(
                "{name}.yield_fraction must be at most 1, not {}",
                self.yield_fraction
            ));
        }
        if !(self.step_period_s.is_finite() && self.step_period_s > 0.0) {
            return Err(format!(
                "{name}.step_period_s must be positive, not {}",
                self.step_period_s
            ));
        }
        if !(self.body_min > 0.0) {
            return Err(format!(
                "{name}.body_min must be positive, not {}",
                self.body_min
            ));
        }
        if self.body_min > self.birth_body || self.birth_body > self.body_max {
            return Err(format!(
                "{name} needs body_min <= birth_body <= body_max, not {} <= {} <= {}",
                self.body_min, self.birth_body, self.body_max
            ));
        }
        if self.birth_cost < self.body_min {
            return Err(format!(
                "{name}.birth_cost {} cannot build a newborn of body_min {}",
                self.birth_cost, self.body_min
            ));
        }
        self.reproduction.validate(name, self)?;
        Ok(())
    }
}

/// The whole layer's numbers: one [`SpeciesConfig`] per species, and nothing else. There is
/// no global constant in this layer — no decomposition, no cap, no lottery — because every
/// rule it has belongs to an animal.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaunaConfig {
    pub frondgrazer: SpeciesConfig,
    /// The two phase-one founders' own physiology and cost settings
    /// ([`FounderPhysiology`], frozen for the first pilots). A body carrying a
    /// [`Founder`] marker runs its founder's numbers instead of its placeholder
    /// species'; [`effective_config`] is the one lookup every rule uses.
    pub founders: [FounderPhysiology; Founder::COUNT],
}

impl Default for FaunaConfig {
    fn default() -> FaunaConfig {
        FaunaConfig {
            frondgrazer: SpeciesConfig::frondgrazer(),
            founders: [
                FounderPhysiology::frozen(Founder::Blind),
                FounderPhysiology::frozen(Founder::Browser),
            ],
        }
    }
}

impl FaunaConfig {
    pub fn species(&self, s: Species) -> &SpeciesConfig {
        match s {
            Species::Frondgrazer => &self.frondgrazer,
        }
    }

    pub fn species_mut(&mut self, s: Species) -> &mut SpeciesConfig {
        match s {
            Species::Frondgrazer => &mut self.frondgrazer,
        }
    }

    /// One founder's physiology, by lineage.
    pub fn founder(&self, founder: Founder) -> &FounderPhysiology {
        &self.founders[founder.index()]
    }

    pub fn validate(&self) -> Result<(), String> {
        for s in Species::ALL {
            self.species(s).validate(s.name())?;
        }
        for founder in Founder::ALL {
            let phys = self.founder(founder);
            phys.core.validate(founder.name())?;
            if !phys.motor_respiration_per_s.is_finite() || phys.motor_respiration_per_s < 0.0 {
                return Err(format!(
                    "{}.motor_respiration_per_s must be finite and nonnegative, not {}",
                    founder.name(),
                    phys.motor_respiration_per_s
                ));
            }
            if !phys.organ_structure_fraction.is_finite()
                || !(0.0..1.0).contains(&phys.organ_structure_fraction)
            {
                return Err(format!(
                    "{}.organ_structure_fraction must be finite and below 1, not {}",
                    founder.name(),
                    phys.organ_structure_fraction
                ));
            }
        }
        Ok(())
    }
}

/// Why a body left the world, as [`FaunaLedger`] books it.
///
/// Two of the three are **deaths** under [`step`]'s step 8 and leave a corpse; the third
/// is not a death at all and leaves none. Keeping all three in one enum is what lets a
/// census say "sixty-five bodies left, and here is the split" without a second bookkeeping
/// path: the two death causes sum to [`FaunaLedger::deaths`], and `Removed` is stated
/// beside them rather than hidden in the difference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Departure {
    /// `body` fell below the effective `body_min`: the reserve ran out and dieback took
    /// the structure past the floor. A corpse.
    Starved,
    /// Standing water deeper than the effective `drown_depth_m` on its own face. A
    /// corpse.
    Drowned,
    /// No corpse: a [`Command::Remove`], or a body whose support face stopped being one
    /// ([`step`]'s step 1). Booked `removed_*_out` and **not** counted in
    /// [`FaunaLedger::deaths`].
    Removed,
}

impl Departure {
    pub const ALL: [Departure; 3] = [Departure::Starved, Departure::Drowned, Departure::Removed];
    pub const COUNT: usize = Departure::ALL.len();

    /// Index into a per-cause array, in [`Departure::ALL`] order.
    pub fn index(self) -> usize {
        match self {
            Departure::Starved => 0,
            Departure::Drowned => 1,
            Departure::Removed => 2,
        }
    }

    /// Whether this departure left a corpse and is counted in [`FaunaLedger::deaths`].
    pub fn is_death(self) -> bool {
        !matches!(self, Departure::Removed)
    }

    pub fn name(self) -> &'static str {
        match self {
            Departure::Starved => "starved",
            Departure::Drowned => "drowned",
            Departure::Removed => "removed",
        }
    }
}

/// Every unit of organic matter, mineral and energy that has crossed this layer's
/// boundary, and the counters of what happened.
///
/// The three currencies are the plant layer's ([`cubarium_voxel_flora::FloraLedger`]) and
/// mean the same thing. Internal moves — a bite becoming tissue, a reserve paying
/// maintenance, a parent paying for a newborn — never appear here.
///
/// **Two pairs of terms the brief did not name are here because the residuals do not close
/// without them.** An [`Command::Introduce`] creates an animal out of nothing, exactly as
/// the plant layer's `Seed` creates a founder stand, so it is booked `introduced_*_in`
/// the way that is booked `seeded_*_in`; and [`Command::Remove`] takes one out of the
/// world without a corpse, so it is booked `removed_*_out`. Both are named boundary flows
/// and neither is hidden.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FaunaLedger {
    /// Organic matter, mineral and energy taken out of the plant layer by
    /// [`cubarium_voxel_flora::Flora::take_foliage`], cumulative over every bite. The
    /// sum of the [`cubarium_voxel_flora::Taken`]s the withdrawals returned, to the bit,
    /// so these three **equal** the plant layer's `consumed_*_out` for a run in which
    /// this layer is the only consumer.
    pub eaten_organic_in: f64,
    pub eaten_mineral_in: f64,
    pub eaten_energy_in: f64,
    /// Organic matter respired: maintenance, the undigested fraction of a bite, the
    /// founder's paid motor respiration, and anything assimilated that a full body and a
    /// full reserve could not hold. Mineral never crosses this boundary.
    ///
    /// The four split counters below sum to this **to the bit** — every booking adds the
    /// same addend to the total and to one split at the same point — so a driver reads
    /// an interval's or an episode's upkeep, motor, digestion and failed-gestation
    /// charges apart without a second ledger. The ES pilot's score needs the motor term
    /// apart from upkeep (`design/voxel-senses-phase1-tests.md` §2).
    pub respired_out: f64,
    /// The `respired_out` share that was basal upkeep.
    pub respired_maintenance_out: f64,
    /// The `respired_out` share that was the founder's paid motion.
    pub respired_motor_out: f64,
    /// The `respired_out` share that was digestion: the undigested fraction of a bite and
    /// assimilated matter neither the body nor the reserve could hold.
    pub respired_digestion_out: f64,
    /// The `respired_out` share that was an **interrupted gestation**:
    /// `gestation_loss_fraction` of the escrow of a parent that could not pay its next
    /// instalment or died carrying one. The fourth split, and the only one that is not
    /// something a living body spent on itself.
    pub respired_gestation_out: f64,
    /// The energy that left with it, at the density of the stock it left.
    pub heat_out: f64,
    /// Organic matter, mineral and energy handed back to the plant layer by
    /// [`cubarium_voxel_flora::Flora::deposit`]: a corpse as carrion, and excreted mineral
    /// as litter. Equal to the plant layer's `deposited_*_in` to the bit, for the same
    /// reason the `eaten_*` are.
    pub deposited_organic_out: f64,
    pub deposited_mineral_out: f64,
    pub deposited_energy_out: f64,
    /// What an [`Command::Introduce`] created out of nothing.
    pub introduced_organic_in: f64,
    pub introduced_mineral_in: f64,
    pub introduced_energy_in: f64,
    /// What a [`Command::Remove`] — or an animal whose support face the terrain took away
    /// — removed without a corpse.
    pub removed_organic_out: f64,
    pub removed_mineral_out: f64,
    pub removed_energy_out: f64,
    /// Every animal this layer has created, introduced ones included: a counter, not a
    /// flux, and the source of [`Animal::id`]. `births − introduced` is how many were
    /// born here.
    pub births: u64,
    /// Animals born of a parent's reserve: a live birth out of a gestation escrow, and
    /// a hatchling out of an egg the parent paid for. `births − introduced == born`.
    pub born: u64,
    /// The `born` share that came out of an egg.
    pub hatched: u64,
    /// Gestations begun, and the ones that ended without a birth because the parent
    /// could not pay an instalment or died carrying the escrow. `opened − failed` is
    /// the number that reached term, once no gestation is still running.
    pub gestations_opened: u64,
    pub gestations_failed: u64,
    /// Clutches laid, and the eggs in them.
    pub clutches_laid: u64,
    pub eggs_laid: u64,
    /// Eggs lost with their site: the face was taken away or drowned, and the clutch
    /// went to carrion where it lay.
    pub eggs_lost: u64,
    /// Animals an [`Command::Introduce`] put into the world.
    pub introduced: u64,
    /// Animals that died: starved below `body_min`, or drowned.
    pub deaths: u64,
    /// The **cause** each of those deaths was booked under, by [`Departure::index`]:
    /// `Starved`, `Drowned`, then `Removed`.
    ///
    /// Instrumentation, not a rule: nothing here changes when a body dies, only what the
    /// ledger can be asked afterwards. `deaths_by_cause[Starved] +
    /// deaths_by_cause[Drowned]` is exactly [`FaunaLedger::deaths`]
    /// ([`FaunaLedger::deaths_accounted`] asserts it), because every death books both at
    /// the same point. `deaths_by_cause[Removed]` is **not** in `deaths` and never was: a
    /// body an [`Command::Remove`] took, or one whose support face the terrain took away,
    /// left the world without dying and without a corpse ([`step`]'s step 1), and the
    /// census could not tell that apart from a death before this counter existed.
    ///
    /// A body that is at once below `body_min` and under drowning water is booked
    /// `Starved`: the starvation clause is the one the death rule reads first, and a
    /// cause counter must name the clause that fired, not adjudicate between two.
    pub deaths_by_cause: [u64; Departure::COUNT],
    /// The same three counters restricted to bodies that carry a lineage, indexed
    /// `[founder][cause]` in [`Founder::index`] and [`Departure::index`] order. A
    /// heuristic body carries no lineage and appears only in the totals above.
    pub deaths_by_founder_cause: [[u64; Departure::COUNT]; Founder::COUNT],
    /// Withdrawals that returned something.
    pub bites: u64,
    /// Bites and organic matter **by the plant species they came off**, indexed by
    /// [`cubarium_voxel_flora::Species::index`].
    ///
    /// The plant layer's `consumed_*` have no species in them and a reach box does not
    /// choose one (round 5a's note), and Astra's round 8 asks a consumer's report to say
    /// which species it actually ate: which stands a browser can reach depends on the
    /// face it stands on, so "what it ate" is a measurement and never a property of a
    /// species. This is that measurement, taken where the bite happens.
    pub bites_by_plant: [u64; cubarium_voxel_flora::Species::COUNT],
    pub eaten_by_plant: [f64; cubarium_voxel_flora::Species::COUNT],
    /// Local founder bites and the organic matter they actually **placed** into body
    /// and reserve, by lineage, indexed by [`Founder::index`].
    ///
    /// The same kind of measurement as `bites_by_plant`, taken at the other end of the
    /// transfer: what a lineage got, not what a species lost. `assimilated_by_founder`
    /// is the placed figure and not the bite's gross organic matter, because that is
    /// what became tissue — it is the same number the body's own `Self`
    /// `assimilated_intake` channel reports. A heuristic body carries no lineage and
    /// appears in neither.
    pub bites_by_founder: [u64; Founder::COUNT],
    pub assimilated_by_founder: [f64; Founder::COUNT],
    /// Steps taken, one voxel each.
    pub steps: u64,
}

impl FaunaLedger {
    /// How many bodies left this way, whatever lineage they carried.
    pub fn departed(&self, cause: Departure) -> u64 {
        self.deaths_by_cause[cause.index()]
    }

    /// How many bodies of one lineage left this way.
    pub fn departed_founder(&self, founder: Founder, cause: Departure) -> u64 {
        self.deaths_by_founder_cause[founder.index()][cause.index()]
    }

    /// The two death causes, summed: equal to [`FaunaLedger::deaths`] for any ledger this
    /// layer produced, which is the instrumentation's own invariant.
    pub fn deaths_accounted(&self) -> u64 {
        self.departed(Departure::Starved) + self.departed(Departure::Drowned)
    }

    pub fn expected_organic(&self) -> f64 {
        self.introduced_organic_in + self.eaten_organic_in
            - self.respired_out
            - self.deposited_organic_out
            - self.removed_organic_out
    }

    pub fn expected_mineral(&self) -> f64 {
        self.introduced_mineral_in + self.eaten_mineral_in
            - self.deposited_mineral_out
            - self.removed_mineral_out
    }

    pub fn expected_energy(&self) -> f64 {
        self.introduced_energy_in + self.eaten_energy_in
            - self.heat_out
            - self.deposited_energy_out
            - self.removed_energy_out
    }
}

/// Read-only access for drawing, inspection and a harness.
#[derive(Clone, Copy, Debug)]
pub struct FaunaView<'a> {
    pub config: &'a FaunaConfig,
    pub tick: u64,
    /// Sorted by [`Animal::id`], which never changes: an animal keeps its place in this
    /// slice for its whole life, whatever it walks over.
    pub animals: &'a [Animal],
    /// The clutches standing in the world, in the order they were laid. Not animals:
    /// stationary paid packages this layer still owns until they hatch.
    pub clutches: &'a [Clutch],
    pub ledger: &'a FaunaLedger,
}

impl<'a> FaunaView<'a> {
    /// The animal with this identity, if it is still alive.
    pub fn animal(&self, id: u64) -> Option<&'a Animal> {
        self.animals
            .binary_search_by_key(&id, |a| a.id)
            .ok()
            .map(|i| &self.animals[i])
    }

    /// Every animal standing on this face, in id order. More than one is allowed: a
    /// newborn appears on its parent's face.
    pub fn animals_at(&self, site: Site) -> impl Iterator<Item = &'a Animal> {
        self.animals.iter().filter(move |a| a.site == site)
    }

    /// Organic matter this layer holds: every animal's structure and reserve, the
    /// escrowed offspring inside gestating parents, and the eggs standing in the world.
    /// This is the stock the ledger's `expected_organic` is checked against, so every
    /// package the layer owns has to be in it or a paid offspring would read as matter
    /// created or destroyed.
    pub fn organic(&self) -> f64 {
        self.animals.iter().map(Animal::stored_organic).sum::<f64>()
            + self.clutches.iter().map(|c| c.organic).sum::<f64>()
    }

    /// Mineral in every animal's tissue, its escrow and every standing clutch.
    pub fn mineral(&self) -> f64 {
        self.animals.iter().map(Animal::stored_mineral).sum::<f64>()
            + self.clutches.iter().map(|c| c.mineral).sum::<f64>()
    }

    /// Energy in every animal's tissue, its escrow and every standing clutch.
    pub fn energy(&self) -> f64 {
        self.animals.iter().map(Animal::stored_energy).sum::<f64>()
            + self.clutches.iter().map(|c| c.energy).sum::<f64>()
    }

    /// Eggs of one lineage standing in the world, over every clutch.
    pub fn eggs_by_founder(&self, founder: Founder) -> u64 {
        self.clutches
            .iter()
            .filter(|c| c.lineage == founder)
            .map(|c| u64::from(c.count))
            .sum()
    }

    /// Bodies of one lineage currently carrying a gestation escrow.
    pub fn gestating_by_founder(&self, founder: Founder) -> u64 {
        self.animals
            .iter()
            .filter(|a| a.founder == Some(founder) && a.reproduction.escrow.is_some())
            .count() as u64
    }

    /// Every clutch on this face, in the order they were laid.
    pub fn clutches_at(&self, site: Site) -> impl Iterator<Item = &'a Clutch> {
        self.clutches.iter().filter(move |c| c.site == site)
    }
}

/// A frontend, a harness or a test changes this layer only through these.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Put an animal of `body` on the **highest** support face of column `(x, z)`, with a
    /// full reserve, its species' `n_tissue` of mineral and its `energy_density` of
    /// energy — a body grown outside the system, booked `introduced_*_in`. Refused if the
    /// column has no support face or `body` is below the species' `body_min`.
    ///
    /// More than one animal may stand on a face, so this is never refused for being
    /// occupied.
    Introduce {
        x: i64,
        z: u32,
        species: Species,
        body: f64,
    },
    /// Put a **phase-one founder** body of `body` organic matter on the highest support
    /// face of column `(x, z)`, at that face's centre and the given `heading_rad`. Booked
    /// exactly as [`Command::Introduce`]. A body that is not finite, a heading that is not
    /// finite, a column with no support, or a body below the founder's own `body_min` is
    /// refused.
    ///
    /// The founder marker selects which manifest's controller the body belongs to and
    /// which physiology it runs (P1-B): the litter feeder's digestive configuration is its
    /// own, set deliberately from the real `Taken` composition of litter. The species is
    /// still the shared phase-one placeholder (`Species::Frondgrazer`); every rule that
    /// reads a founder's numbers goes through [`effective_config`], which substitutes the
    /// founder table for the placeholder.
    IntroduceFounder {
        x: i64,
        z: u32,
        founder: Founder,
        /// How full the body arrives ([`StartingStores`]). Unlike [`Command::Introduce`],
        /// which always grows a full body outside the system, a founder may be placed
        /// hungry — which is what makes eating worth anything to it.
        stores: StartingStores,
        heading_rad: f64,
    },
    /// Take every animal off the highest support face of column `(x, z)`, booking their
    /// material as `removed_*_out`. No corpse: this is a frontend's undo, not a death.
    /// Refused if there is none.
    Remove { x: i64, z: u32 },
}

/// How full a founder body arrives, as fractions of what its physiology lets it hold.
///
/// Phase one introduced every founder at [`StartingStores::FULL`] — body at `body_max`
/// and a full reserve — and P2-B measured what that costs: with no headroom, settled
/// intake can only replace the upkeep the body already burned, so the whole score is
/// capped at `0.25 + maintenance/reference` and standing still is within a hair of the
/// best attainable episode. A body placed at [`StartingStores::HUNGRY`] has somewhere to
/// put what it eats.
///
/// The manifest's `body_reference` is **not** this: it is the schema's fixed normaliser
/// and does not move with the start state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StartingStores {
    /// Structure at introduction, as a fraction of the founder's `body_max`.
    pub body: f64,
    /// Reserve at introduction, as a fraction of the full reserve that much structure
    /// carries (`reserve_cap · body`).
    pub reserve: f64,
}

impl StartingStores {
    /// Everything the body can hold: what [`Command::Introduce`] does, and what the
    /// live schedule's founders keep.
    pub const FULL: StartingStores = StartingStores {
        body: 1.0,
        reserve: 1.0,
    };

    /// The arenas' hungry founder (P2-C): half its structure and no reserve, so a whole
    /// reference body of headroom is there to be eaten into.
    pub const HUNGRY: StartingStores = StartingStores {
        body: 0.5,
        reserve: 0.0,
    };

    /// The absolute `(body, reserve)` these fractions mean for `sc`, or `None` when they
    /// are not finite, not within `[0, 1]`, or the structure they ask for is below the
    /// physiology's `body_min` — a body that cannot live is refused, not clamped.
    pub fn resolve(&self, sc: &SpeciesConfig) -> Option<(f64, f64)> {
        if !(self.body.is_finite() && (0.0..=1.0).contains(&self.body)) {
            return None;
        }
        if !(self.reserve.is_finite() && (0.0..=1.0).contains(&self.reserve)) {
            return None;
        }
        let body = self.body * sc.body_max;
        if !(body >= sc.body_min) {
            return None;
        }
        Some((body, self.reserve * sc.reserve_of(body)))
    }
}

/// The animal layer. Owns its animals; borrows the world and the plant layer per call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fauna {
    config: FaunaConfig,
    tick: u64,
    /// Sorted by id, so every pass over them is in one stable order however they move.
    /// A `Vec` with a binary search, never a `HashMap`: this layer iterates nothing
    /// unordered.
    animals: Vec<Animal>,
    /// The clutches standing in the world, in the order they were laid — which is
    /// parent id order within a tick and tick order between ticks, so every pass over
    /// them is the same order however the world runs.
    clutches: Vec<Clutch>,
    ledger: FaunaLedger,
    /// Whether paid births run in [`step`]. Defaults to **true**, which is the live world's
    /// rule and changes nothing. An isolated arena sets it false
    /// (`design/voxel-senses-phase1-plan.md`, "Frozen arena contract"); a fresh episode
    /// sets it back.
    births_enabled: bool,
    /// The controllers the founder bodies are driven by, keyed by animal id. Driver-owned
    /// session state, not world state: skipped by the snapshot and not cloned
    /// ([`FounderControllers`]).
    #[serde(skip)]
    controllers: FounderControllers,
    /// How to build a fresh controller for a founder **born** here, one recipe per
    /// lineage. Skipped by the snapshot for the same reason the controllers are, but
    /// cloned with the layer: see [`FounderFactories`].
    #[serde(skip)]
    factories: FounderFactories,
    /// Which lineages were driven by a **saved policy** rather than their own
    /// heuristic, by [`Founder::index`]. This one **is** world state and the snapshot
    /// carries it.
    ///
    /// The controllers themselves are not saved — a `dyn Controller` does not serialize
    /// and a hidden state is session state — so on its own a loaded layer cannot tell a
    /// trained lineage from a heuristic one, and a loader would put the heuristic back
    /// without saying anything. That is a different world wearing the same bytes. This
    /// flag is the one thing a loader cannot re-derive, so it is recorded: a driver
    /// reading it either supplies the policy again or refuses the load.
    policy_driven: [bool; Founder::COUNT],
    /// The `weights_fnv1a` digest of the centre driving this lineage, by
    /// [`Founder::index`], or `0` for a heuristic or an untouched lineage. Recorded
    /// beside [`Fauna::policy_driven`] for the same reason and by the same installer:
    /// the bool alone can tell a trained lineage from a heuristic one, but not one
    /// trained centre from another, so a loaded world whose lineage was driven by one
    /// centre and is handed a *different* one could not be refused — only demoted or
    /// silently swapped, both of which `always-fresh-never-migrate` forbids. This is
    /// the identity a loader compares against.
    policy_digest: [u64; Founder::COUNT],
}

impl Fauna {
    /// An animal layer on a **valid** config. Panics with the offending field named if
    /// [`FaunaConfig::validate`] refuses it, exactly as
    /// [`cubarium_voxel_flora::Flora::new`] does and for the same reason: a preset that
    /// cannot build a living animal is a programming error in the preset.
    pub fn new(config: FaunaConfig) -> Fauna {
        match Fauna::try_new(config) {
            Ok(fauna) => fauna,
            Err(e) => panic!("invalid FaunaConfig — {e}"),
        }
    }

    /// [`Fauna::new`] without the panic.
    pub fn try_new(config: FaunaConfig) -> Result<Fauna, String> {
        config.validate()?;
        Ok(Fauna {
            config,
            tick: 0,
            animals: Vec::new(),
            clutches: Vec::new(),
            ledger: FaunaLedger::default(),
            births_enabled: true,
            controllers: FounderControllers::default(),
            factories: FounderFactories::default(),
            policy_driven: [false; Founder::COUNT],
            policy_digest: [0; Founder::COUNT],
        })
    }

    /// Give the founder body `id` a controller: the diagnostic heuristic and the GRU
    /// policy are interchangeable behind [`Controller`], and this table is what the
    /// tick's controller stage drives. Accepted only for a body that exists; `true`
    /// when installed. A fresh episode clears a controller's own memory through
    /// [`Controller::reset`] — reinstalling is not what resets hidden state.
    pub fn set_controller(&mut self, animal_id: u64, controller: Box<dyn Controller>) -> bool {
        if self.view().animal(animal_id).is_none() {
            return false;
        }
        self.controllers.set(animal_id, controller);
        true
    }

    /// Take a body's controller back, if it has one: a driver that wants to replace or
    /// drop one between episodes.
    pub fn take_controller(&mut self, animal_id: u64) -> Option<Box<dyn Controller>> {
        self.controllers.take(animal_id)
    }

    /// Register how to build a controller for a founder **born** in this layer.
    ///
    /// A newborn founder inherits its parent's lineage and pays for its own body, but it
    /// inherits no mind: [`step`]'s birth rule asks this factory for a fresh controller
    /// of the parent's kind, so a lineage that breeds keeps being driven instead of
    /// filling the world with resting bodies that still pay upkeep. One factory per
    /// lineage, never a shared controller instance — two bodies sharing one controller
    /// would share its memory.
    ///
    /// Introduced bodies are **not** touched by this: a driver installs their controllers
    /// itself with [`Fauna::set_controller`], which is also how it chooses something
    /// other than the lineage default for a particular body.
    pub fn set_founder_factory(
        &mut self,
        founder: Founder,
        factory: std::sync::Arc<dyn ControllerFactory>,
    ) {
        self.factories.set(founder, factory);
    }

    /// Whether this lineage's bodies are driven by a saved policy rather than their own
    /// heuristic. Snapshot state: see the field's own note for why it is recorded.
    pub fn policy_driven(&self, founder: Founder) -> bool {
        self.policy_driven[founder.index()]
    }

    /// Record that this lineage is (or is no longer) driven by a saved policy. A driver
    /// sets it when it installs one; a loader reads it to decide whether it may run this
    /// world at all.
    pub fn set_policy_driven(&mut self, founder: Founder, driven: bool) {
        self.policy_driven[founder.index()] = driven;
    }

    /// The `weights_fnv1a` digest of the centre this lineage is recorded as running, or
    /// `0` for a heuristic or an untouched lineage. Snapshot state: see the field's own
    /// note for why it is recorded beside [`Fauna::policy_driven`].
    pub fn policy_digest(&self, founder: Founder) -> u64 {
        self.policy_digest[founder.index()]
    }

    /// Record the `weights_fnv1a` digest of the centre now driving this lineage. A
    /// driver sets it whenever it sets [`Fauna::set_policy_driven`]; a loader reads it
    /// to decide whether the centre it was handed is the one this world remembers.
    pub fn set_policy_digest(&mut self, founder: Founder, digest: u64) {
        self.policy_digest[founder.index()] = digest;
    }

    /// Forget a lineage's factory. Founders of that kind born afterwards rest.
    pub fn clear_founder_factory(&mut self, founder: Founder) {
        self.factories.clear(founder);
    }

    /// Whether a lineage has a birth factory registered.
    pub fn has_founder_factory(&self, founder: Founder) -> bool {
        self.factories.has(founder)
    }

    /// A fresh controller of a lineage's registered kind, installed on an existing body.
    /// `false` when the body does not exist or the lineage has no factory. This is how a
    /// driver re-installs controllers over a whole layer — after a load, or after
    /// changing what a lineage is driven by.
    pub fn install_founder_controller(&mut self, animal_id: u64, founder: Founder) -> bool {
        let Some(controller) = self.factories.make(founder) else {
            return false;
        };
        self.set_controller(animal_id, controller)
    }

    pub fn config(&self) -> &FaunaConfig {
        &self.config
    }
    /// Whether paid births run. Live-world default: `true`.
    pub fn births_enabled(&self) -> bool {
        self.births_enabled
    }

    /// Turn births on or off. An isolated arena turns them off; the live world never does.
    pub fn set_births_enabled(&mut self, enabled: bool) {
        self.births_enabled = enabled;
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn view(&self) -> FaunaView<'_> {
        FaunaView {
            config: &self.config,
            tick: self.tick,
            animals: &self.animals,
            clutches: &self.clutches,
            ledger: &self.ledger,
        }
    }

    /// Advance one tick against the world and the plant layer as they stand. Call **after**
    /// `World::step` and after `Flora::step`: the plant layer reads the water the core has
    /// just moved and grows, and then the animals eat what is there. The order within the
    /// tick is [`step`]'s module doc, and it is written down in exactly one place.
    pub fn step(&mut self, world: &World, flora: &mut Flora) {
        step::step(self, world, flora, cubarium_voxel::default_threads());
    }

    /// [`Fauna::step`] with a thread count for the one phase that splits: `sense`, which
    /// only reads. [`Fauna::step`] is this with [`cubarium_voxel::default_threads`];
    /// `1` runs it on this thread.
    ///
    /// **Execution, never a rule.** Every animal's plan is a pure function of the world,
    /// the plant layer and that animal, the plans are gathered in animal order whatever
    /// order the chunks finish in, and `act` still applies them serially in id order. So
    /// the thread count cannot reach a result, and with the `parallel` feature off it is
    /// ignored altogether (`design/handoffs/voxel-schedule-brief-2026-09-18.md`).
    pub fn step_with(&mut self, world: &World, flora: &mut Flora, threads: usize) {
        step::step(self, world, flora, threads);
    }

    /// The **static arena's** stepping: [`Fauna::step_with`] with the arena's per-arena
    /// [`Senses`] handle threaded into the tick, so the controller stage reads the settled
    /// litter field its caller prepared (`design/voxel-senses-phase1-plan.md`, "Initial cue
    /// field settings"). The field updates itself inside the tick at its own cadence
    /// ([`UPDATE_TICKS`]); a caller never calls this for the live world. [`Fauna::step`] and
    /// [`Fauna::step_with`] keep the live signature and stay senses-free.
    pub fn step_with_senses(
        &mut self,
        world: &World,
        flora: &mut Flora,
        threads: usize,
        senses: &mut Senses,
    ) {
        step::step_with_senses(self, world, flora, threads, senses);
    }

    /// Apply a command now, between ticks, the way the plant layer's are applied. Returns
    /// whether it was accepted.
    pub fn apply(&mut self, world: &World, command: Command) -> bool {
        match command {
            Command::Introduce {
                x,
                z,
                species,
                body,
            } => self.introduce(world, x, z, species, None, body, 0.0),
            Command::IntroduceFounder {
                x,
                z,
                founder,
                stores,
                heading_rad,
            } => {
                let core = self.config.founder(founder).core;
                let Some((body, reserve)) = stores.resolve(&core) else {
                    return false;
                };
                self.introduce_body(
                    world,
                    x,
                    z,
                    Species::Frondgrazer,
                    Some(founder),
                    body,
                    Some(reserve),
                    heading_rad,
                )
            }
            Command::Remove { x, z } => {
                let view = world.view();
                let Some(site) = cubarium_voxel_flora::highest_support(&view, x, z) else {
                    return false;
                };
                let before = self.animals.len();
                let mut removed = Vec::new();
                self.animals.retain(|a| {
                    if a.site == site {
                        removed.push(*a);
                        false
                    } else {
                        true
                    }
                });
                for a in removed {
                    // The body is gone, so its controller is too: ids are never reused,
                    // and a table that only grows is a leak in a world that breeds.
                    self.controllers.take(a.id);
                    self.book_removed(&a);
                }
                self.animals.len() != before
            }
        }
    }

    /// One body placed on a column's highest support face: shared by [`Command::Introduce`]
    /// and [`Command::IntroduceFounder`], so the booking is one path. `founder` `None` is
    /// the live heuristic browser.
    fn introduce(
        &mut self,
        world: &World,
        x: i64,
        z: u32,
        species: Species,
        founder: Option<Founder>,
        body: f64,
        heading_rad: f64,
    ) -> bool {
        self.introduce_body(world, x, z, species, founder, body, None, heading_rad)
    }

    /// The placement itself. `reserve` `None` is the full reserve `body` can carry — what
    /// every live introduction does and did. `Some(r)` is an explicit starting reserve,
    /// which only a founder introduction asks for.
    #[allow(clippy::too_many_arguments)]
    fn introduce_body(
        &mut self,
        world: &World,
        x: i64,
        z: u32,
        species: Species,
        founder: Option<Founder>,
        body: f64,
        reserve: Option<f64>,
        heading_rad: f64,
    ) -> bool {
        let view = world.view();
        let Some(site) = cubarium_voxel_flora::highest_support(&view, x, z) else {
            return false;
        };
        // A founder body is introduced under its **own** physiology: thresholds,
        // densities and tissue mineral content come from the founder table, not from
        // the placeholder species it shares a [`Species`] tag with.
        let sc = match founder {
            Some(f) => self.config.founder(f).core,
            None => *self.config.species(species),
        };
        if !(body.is_finite() && body >= sc.body_min) {
            return false;
        }
        if !heading_rad.is_finite() {
            return false;
        }
        let reserve = reserve.unwrap_or_else(|| sc.reserve_of(body));
        if !(reserve.is_finite() && (0.0..=sc.reserve_of(body)).contains(&reserve)) {
            return false;
        }
        let organic = body + reserve;
        let pose = Pose {
            heading_rad,
            ..Pose::at_site(site, view.config.voxel_m)
        };
        let animal = Animal {
            id: self.ledger.births,
            species,
            site,
            pose,
            founder,
            founder_state: FounderState::default(),
            reproduction: ReproductionState::default(),
            body,
            reserve,
            mineral: sc.n_tissue * organic,
            energy: sc.energy_density * organic,
            age_ticks: 0,
            state: State::Resting,
        };
        self.ledger.births += 1;
        self.ledger.introduced += 1;
        self.ledger.introduced_organic_in += organic;
        self.ledger.introduced_mineral_in += animal.mineral;
        self.ledger.introduced_energy_in += animal.energy;
        self.insert(animal);
        true
    }

    /// Save as postcard bytes behind this layer's own schema tag. A different tag is
    /// refused by [`Fauna::load`]; there is no migration, ever
    /// (`always-fresh-never-migrate`).
    pub fn save(&self) -> Vec<u8> {
        snapshot::encode(self)
    }

    /// Read a layer back. Refuses another schema tag, corrupt bytes, and a config no
    /// animal could live under.
    pub fn load(bytes: &[u8]) -> anyhow::Result<Fauna> {
        snapshot::decode(bytes)
    }

    /// Insert keeping the id order. An id is never reused, so this is always an insert.
    fn insert(&mut self, animal: Animal) {
        match self.animals.binary_search_by_key(&animal.id, |a| a.id) {
            Ok(_) => unreachable!("an animal id is never reused"),
            Err(at) => self.animals.insert(at, animal),
        }
    }

    /// One withdrawal on the boundary: the three `eaten_*` flows, and the same
    /// [`Taken`] back, so what this ledger books and what the animal received are one
    /// value and cannot drift from the plant layer's `consumed_*`.
    fn book_eaten(&mut self, taken: Taken) -> Taken {
        self.ledger.eaten_organic_in += taken.organic;
        self.ledger.eaten_mineral_in += taken.mineral;
        self.ledger.eaten_energy_in += taken.energy;
        self.ledger.bites += 1;
        taken
    }

    /// Material out of this layer and into the plant layer's pools, booked whichever way
    /// it went.
    ///
    /// A triplet whose three numbers are all zero is not a deposit and the plant layer
    /// refuses it; there is nothing to lose and nothing is booked. A triplet it refuses
    /// for any *other* reason would otherwise strand material on a dead animal, so it is
    /// booked out as `removed_*` instead — this layer never keeps an inert remainder.
    fn book_deposit(&mut self, flora: &mut Flora, site: Site, kind: DepositKind, d: Taken) {
        if d.organic <= 0.0 && d.mineral <= 0.0 && d.energy <= 0.0 {
            return;
        }
        let deposit = Deposit {
            kind,
            organic: d.organic.max(0.0),
            mineral: d.mineral.max(0.0),
            energy: d.energy.max(0.0),
        };
        if flora.deposit(site, deposit) {
            self.ledger.deposited_organic_out += deposit.organic;
            self.ledger.deposited_mineral_out += deposit.mineral;
            self.ledger.deposited_energy_out += deposit.energy;
        } else {
            self.ledger.removed_organic_out += deposit.organic;
            self.ledger.removed_mineral_out += deposit.mineral;
            self.ledger.removed_energy_out += deposit.energy;
        }
    }

    /// An animal leaving the world without a corpse takes **everything it holds**, the
    /// escrowed offspring included: an unborn package is not left behind on an empty
    /// face, and it is not a death, so it is not carrion either.
    fn book_removed(&mut self, a: &Animal) {
        self.ledger.removed_organic_out += a.stored_organic();
        self.ledger.removed_mineral_out += a.stored_mineral();
        self.ledger.removed_energy_out += a.stored_energy();
        self.book_departure(a, Departure::Removed);
    }

    /// The one place a departure's cause is counted, so the totals and the per-lineage
    /// split cannot drift: the same body books the same addend into both at the same
    /// point, exactly as [`FaunaLedger::respired_out`]'s three splits do.
    pub(crate) fn book_departure(&mut self, a: &Animal, cause: Departure) {
        self.ledger.deaths_by_cause[cause.index()] += 1;
        if let Some(founder) = a.founder {
            self.ledger.deaths_by_founder_cause[founder.index()][cause.index()] += 1;
        }
    }
}

/// Every support face of column `(x, z)` an animal on `from` could step onto, ascending in
/// `y`: within `climb` of `from.y`, with no more than `wade_depth_m` of standing water on
/// it, and with `headroom` voxels of void over it — a body does not stand in a slot it
/// does not fit in ([`crate::body::headroom_voxels`]).
///
/// Public because the geometry is worth stating without a tick in the way — a test, or a
/// harness choosing where to introduce an animal, asks exactly this question.
pub fn steppable(
    view: &VoxelView<'_>,
    from: Site,
    x: i64,
    z: u32,
    sc: &SpeciesConfig,
    headroom: u32,
) -> Vec<Site> {
    let c = view.config;
    if z >= c.depth {
        return Vec::new();
    }
    let climb = i64::from(sc.climb);
    let mut out = Vec::new();
    for y in 0..c.height {
        if !view.is_support(x, y, z) {
            continue;
        }
        if !crate::body::has_headroom(view, x, y, z, headroom) {
            continue;
        }
        if (i64::from(y) - i64::from(from.y)).abs() > climb {
            continue;
        }
        if view.water_depth_m(x, y, z) > sc.wade_depth_m {
            continue;
        }
        out.push(Site {
            x: x.rem_euclid(i64::from(c.width)) as u32,
            y,
            z,
        });
    }
    out
}

#[cfg(test)]
mod starting_stores_tests {
    use super::*;

    /// The fractions mean what they say against a founder's own physiology, and a body
    /// that could not live is refused rather than clamped up to `body_min`.
    #[test]
    fn starting_stores_resolve_against_the_founders_own_physiology() {
        for founder in Founder::ALL {
            let sc = FounderPhysiology::frozen(founder).core;

            let (body, reserve) = StartingStores::FULL.resolve(&sc).expect("full is valid");
            assert_eq!(body, sc.body_max);
            assert_eq!(reserve, sc.reserve_of(sc.body_max), "a full reserve");

            let (body, reserve) = StartingStores::HUNGRY
                .resolve(&sc)
                .expect("hungry is valid");
            assert_eq!(body, 0.5 * sc.body_max);
            assert_eq!(reserve, 0.0, "hungry means no reserve at all");
            // The headroom that buys: a whole body_max of organic matter to eat into.
            let full = sc.body_max + sc.reserve_of(sc.body_max);
            assert!(
                ((full - body) - sc.body_max).abs() < 1e-15,
                "{founder:?}: headroom {} is not one body_max",
                full - body
            );

            // Below body_min, out of range, and non-finite are all refusals.
            let too_small = StartingStores {
                body: 0.5 * sc.body_min / sc.body_max,
                reserve: 0.0,
            };
            assert!(too_small.resolve(&sc).is_none(), "a body under body_min");
            for bad in [
                StartingStores {
                    body: 1.5,
                    reserve: 0.0,
                },
                StartingStores {
                    body: -0.1,
                    reserve: 0.0,
                },
                StartingStores {
                    body: 1.0,
                    reserve: 1.5,
                },
                StartingStores {
                    body: 1.0,
                    reserve: -0.5,
                },
                StartingStores {
                    body: f64::NAN,
                    reserve: 0.0,
                },
                StartingStores {
                    body: 1.0,
                    reserve: f64::INFINITY,
                },
            ] {
                assert!(bad.resolve(&sc).is_none(), "{bad:?} must be refused");
            }
        }
    }
}

#[cfg(test)]
mod cone_census_tests {
    use super::*;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material};
    use cubarium_voxel_flora::{Command as FloraCommand, FloraConfig, Species as Plant};

    /// A 16 × 8 × 8 world at 0.25 m with soil to y = 2, a rock step, a pool of free
    /// water and four stands — so the census has a terrain hit, a water hit, a stripped
    /// crown and a standing crown to tell apart in the same run.
    fn wall_and_water_world() -> (World, Flora, Fauna) {
        let mut world = World::empty(VoxelConfig {
            width: 16,
            height: 8,
            depth: 8,
            voxel_m: 0.25,
            ..VoxelConfig::default()
        });
        for z in 0..8 {
            for x in 0..16 {
                for y in 1..=2 {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        // A rock step two columns east of the middle: a terrain first hit.
        for z in 4..=6 {
            world.apply(WorldCommand::SetMaterial {
                x: 11,
                y: 3,
                z,
                material: Material::Rock,
            });
        }
        // A puddle two columns west: a water first hit at the eye's own layer is what a
        // shower film looks like to the ray.
        for z in 2..=4u32 {
            world.apply(WorldCommand::AddWater {
                x: 5,
                y: 3,
                z,
                volume_m3: 0.01,
            });
        }
        let mut flora = Flora::new(FloraConfig::default());
        for (x, z, species) in [
            (9u32, 2u32, Plant::Springturf),
            (8, 6, Plant::Springturf),
            (12, 3, Plant::Stonecushion),
            (6, 5, Plant::Stonecushion),
        ] {
            let wood = 0.5 * flora.config().species(species).wood_max;
            flora.apply(
                &world,
                FloraCommand::Seed {
                    x: i64::from(x),
                    z,
                    species,
                    wood,
                },
            );
        }
        let mut fauna = Fauna::new(FaunaConfig::default());
        for (x, z, heading) in [
            (8u32, 4u32, 0.0),
            (9, 4, std::f64::consts::FRAC_PI_2),
            (7, 3, std::f64::consts::PI),
        ] {
            assert!(fauna.apply(
                &world,
                Command::IntroduceFounder {
                    x: i64::from(x),
                    z,
                    founder: Founder::Browser,
                    stores: StartingStores::FULL,
                    heading_rad: heading,
                },
            ));
        }
        (world, flora, fauna)
    }

    /// **The census is the reading.** Deliverable 2's contract: aggregating the fine
    /// per-ray classes back through `Fine::coarse` reproduces the policy's own
    /// `foliage_fraction` and `foliage_proximity` bit for bit, on a world that is
    /// changing under the bodies — stands cropped, water moving, three browsers walking
    /// into and out of each other's cones.
    #[test]
    fn the_fine_census_maps_onto_the_coarse_reading_exactly() {
        let (mut world, mut flora, mut fauna) = wall_and_water_world();
        let mut checked = 0usize;
        let mut saw = [0usize; 7];
        for tick in 0..=200u64 {
            if tick > 0 {
                world.step();
                fauna.step(&world, &mut flora);
            }
            if tick % 20 != 0 {
                continue;
            }
            let view = world.view();
            let fv = flora.view();
            let av = fauna.view();
            for animal in av.animals {
                let Some(census) = browser_cone_census(&view, &fv, &av, animal, &[]) else {
                    continue;
                };
                let reading = browser_cone_readings(&view, &fv, &av, animal).expect("a valid cone");
                let rays_per_sector = (Founder::Browser.manifest().ray_yaw_offsets_deg.len()
                    * Founder::Browser.manifest().ray_pitch_offsets_deg.len())
                    as f64;
                for (sector, (fraction, proximity)) in reading.into_iter().enumerate() {
                    let counts = census.counts(sector);
                    for (i, n) in counts.iter().enumerate() {
                        saw[i] += *n as usize;
                    }
                    let foliage: Vec<f64> = census
                        .rays
                        .iter()
                        .filter(|r| r.sector == sector && r.hit == ConeHit::FoliageCrown)
                        .map(|r| (1.0 - r.distance_m / census.range_m).clamp(0.0, 1.0))
                        .collect();
                    assert_eq!(
                        f64::from(counts[ConeHit::FoliageCrown.index()]) / rays_per_sector,
                        fraction,
                        "tick {tick} body {} sector {sector}: the census's foliage count \
                         is not the reading's fraction",
                        animal.id,
                    );
                    let want = if foliage.is_empty() {
                        0.0
                    } else {
                        foliage.iter().sum::<f64>() / foliage.len() as f64
                    };
                    assert_eq!(
                        want, proximity,
                        "tick {tick} body {} sector {sector}: the census's foliage \
                         distances are not the reading's proximity",
                        animal.id,
                    );
                    assert_eq!(
                        counts.iter().sum::<u32>() as f64,
                        rays_per_sector,
                        "every ray of the sector is classed exactly once"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "the test never sampled a cone");
        // The fixture has to actually exercise the classes the coarse reading hides, or
        // the assertion above is vacuous for them.
        for hit in [
            ConeHit::Clear,
            ConeHit::Terrain,
            ConeHit::Water,
            ConeHit::FoliageCrown,
        ] {
            assert!(
                saw[hit.index()] > 0,
                "the fixture never produced a {} first hit",
                hit.name()
            );
        }
    }

    /// A probe bearing is marched by the same ray as the fan: aimed down the heading
    /// with no pitch, it agrees with the fan's own centre-sector, centre-offset ray.
    #[test]
    fn a_probe_bearing_is_the_same_ray_as_the_fan() {
        let (world, flora, fauna) = wall_and_water_world();
        let view = world.view();
        let fv = flora.view();
        let av = fauna.view();
        let animal = av.animals.first().expect("a founder was introduced");
        let census =
            browser_cone_census(&view, &fv, &av, animal, &[(0.0, 0.0)]).expect("a valid cone");
        let straight = census
            .rays
            .iter()
            .find(|r| r.sector == 1 && r.yaw_deg == 0.0 && r.pitch_deg == 0.0)
            .expect("the fan has a ray dead ahead");
        let probe = census.probes.first().expect("one probe was asked for");
        assert_eq!(
            (probe.hit, probe.distance_m),
            (straight.hit, straight.distance_m)
        );
    }
}
