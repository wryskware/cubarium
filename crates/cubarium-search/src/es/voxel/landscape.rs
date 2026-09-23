//! **Training landscapes** (P5-B item 4, `design/handoffs/voxel-retrain-2026-09-22.md`):
//! whole worlds founded by the ambient run's own founding loop, frozen, with every
//! founder the seeder placed acting.
//!
//! - **D5, whole worlds, not crops.** A [`Landscape`] is what the moved founding loop
//!   ([`cubarium_voxel_sim::found::found_a_landscape`]) returns for a preset and a seed —
//!   the live lake gate, pre-roll, seeding and acceptance, redrawn on a refusal — so the
//!   water, the cue and the walking boundaries are the world's own.
//! - **D6, frozen water, two states.** The world the seeder planted on (after the
//!   opening shower drained: [`WaterState::Drained`]) and the world half way through
//!   that shower ([`WaterState::MidShower`]), with the same seeded plants and founders.
//!   An episode runs the **static** schedule — no water leg and no plant leg — so
//!   neither moves; rising water is judged only live.
//! - **D7, who acts.** Every founder the seeder placed of the candidate's lineage runs
//!   the candidate; the other lineage's founders are there too, on their own
//!   heuristics. Births are off. The score is the acting bodies' mean.
//! - **D8, horizon** [`LANDSCAPE_HORIZON_TICKS`]; **D10, seeds**
//!   [`TRAINING_LANDSCAPE_SEEDS`] / [`HELD_OUT_LANDSCAPE_SEEDS`] on
//!   [`LANDSCAPE_PRESETS`]; **D11**, each acting body's size is drawn per episode
//!   ([`super::driver`]).
//!
//! The seed of a landscape is where its founding **starts**: the loop draws `base` first
//! and keeps it when it passes, which on every preset is most of the time. A refused
//! draw is redrawn from `base + 1000`, `base + 2000`, … ([`LANDSCAPE_REDRAW_STRIDE`])
//! rather than from `base + 1` as the live loop does, because consecutive training bases
//! would otherwise keep each other's worlds: measured on the training set, 13 of the 48
//! foundings kept a world another base had already kept (`small` 109–112 all kept 112).
//! The kept world is [`Landscape::world_seed`]; the gate, the pre-roll, the seeding and
//! the verdict are the live loop's.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use cubarium_voxel::World;
use cubarium_voxel_fauna::{Fauna, FaunaConfig, Founder, RouteMap, Senses};
use cubarium_voxel_flora::Site;
use cubarium_voxel_flora::{Flora, FloraConfig};
use cubarium_voxel_sim::Placement;
use cubarium_voxel_sim::found::{self, Founded};
use cubarium_voxel_sim::habitat;

/// The landscape episode protocol: whole frozen worlds, two water states, every placed
/// founder acting, bodies sampled per episode, 4,800 ticks.
pub const LANDSCAPE_PROTOCOL: &str = "p5-landscape-1";

/// D8: four simulated minutes.
pub const LANDSCAPE_HORIZON_TICKS: u64 = 4_800;

/// D10: the sixteen training seed bases.
pub const TRAINING_LANDSCAPE_SEEDS: [u64; 16] = [
    101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
];

/// D10: the eight held-out seed bases, never used to choose anything.
pub const HELD_OUT_LANDSCAPE_SEEDS: [u64; 8] = [201, 202, 203, 204, 205, 206, 207, 208];

/// How far a refused landscape draw jumps: `base`, then `base + 1000`, `base + 2000`, …
/// Two bases that differ modulo this never draw the same world, so every training and
/// held-out landscape is its own world. A fixture choice, not a rate.
pub const LANDSCAPE_REDRAW_STRIDE: u64 = 1_000;

/// D10: the presets a landscape set is founded on.
pub const LANDSCAPE_PRESETS: [&str; 3] = ["small", "default", "wide"];

/// Which frozen water a landscape episode runs in (D6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WaterState {
    /// After the opening shower drained: the world the seeder planted on.
    Drained,
    /// Half way through the opening shower ([`habitat::pre_roll_capturing`]).
    MidShower,
}

impl WaterState {
    pub const ALL: [WaterState; 2] = [WaterState::Drained, WaterState::MidShower];

    pub fn as_str(self) -> &'static str {
        match self {
            WaterState::Drained => "drained",
            WaterState::MidShower => "mid-shower",
        }
    }
}

/// One founded world, both of its water states, its plants and where the seeder put
/// every founder. Founder-agnostic: [`Landscape::prepare`] makes one lineage's episode
/// fixture out of it.
#[derive(Clone)]
pub struct Landscape {
    pub preset: String,
    /// The seed base the founding loop started from.
    pub seed_base: u64,
    /// The world seed it kept.
    pub world_seed: u64,
    /// Whether the kept world passed the acceptance check.
    pub accepted: bool,
    drained: World,
    mid_shower: Option<World>,
    flora: Flora,
    placements: Vec<Placement>,
    /// Per lineage ([`Founder::index`]): the standable faces of the walkable components
    /// the seeder's acceptance judged habitable, on the drained world — where acting
    /// bodies start ([`start_faces_of`]).
    start_faces: [Vec<Site>; Founder::COUNT],
    /// Per lineage: the faces a body of it can stand on at all in the mid-shower world.
    mid_shower_standable: Option<[Vec<Site>; Founder::COUNT]>,
}

impl Landscape {
    /// Found `preset` from `seed_base` through the live founding loop
    /// ([`found::found_a_landscape`]: lake gate, pre-roll, seeding and acceptance with
    /// the live bounds, plant layer and animal config), drawing `seed_base` first and
    /// [`LANDSCAPE_REDRAW_STRIDE`] apart after a refusal.
    pub fn found(preset: &str, seed_base: u64) -> Result<Landscape, String> {
        let cfg = cubarium_voxel::Preset::find(preset)
            .ok_or_else(|| format!("no landform preset is called {preset:?}"))?
            .config();
        let mut next = seed_base;
        let founded = found::found_a_landscape(
            &cfg,
            None,
            found::LAKE_SEED_TRIES,
            found::HABITAT_TRIES,
            move || {
                let seed = next;
                next = next.wrapping_add(LANDSCAPE_REDRAW_STRIDE);
                seed
            },
            |w: &World| {
                (
                    Flora::new(FloraConfig::for_voxel_size(w.config().voxel_m)),
                    Fauna::new(FaunaConfig::default()),
                )
            },
            habitat::FOUNDER_COUNTS,
            |s: &habitat::Seeded| s.acceptance.accepted,
        );
        Ok(Landscape::from_founded(preset, seed_base, founded))
    }

    /// A landscape out of a founding the caller ran itself (a test's small ring, say).
    /// The founders' faces and headings are read off the seeded animal layer.
    pub fn from_founded(preset: &str, seed_base: u64, founded: Founded) -> Landscape {
        let cfg = FaunaConfig::default();
        let start_faces = Founder::ALL.map(|f| start_faces_of(&founded, &cfg, f));
        let mid_shower_standable = founded.mid_shower.as_ref().map(|w| {
            Founder::ALL.map(|f| {
                let mut faces = RouteMap::for_founder(&w.view(), cfg.founder(f)).faces;
                faces.sort_unstable_by_key(|s| (s.x, s.y, s.z));
                faces
            })
        });
        let placements = founded
            .fauna
            .view()
            .animals
            .iter()
            .filter_map(|a| {
                a.founder.map(|founder| Placement {
                    founder,
                    site: a.site,
                    heading_rad: a.pose.heading_rad,
                })
            })
            .collect();
        Landscape {
            preset: preset.to_string(),
            seed_base,
            world_seed: founded.seed,
            accepted: founded.accepted,
            drained: founded.world,
            mid_shower: founded.mid_shower,
            flora: founded.flora,
            placements,
            start_faces,
            mid_shower_standable,
        }
    }

    /// The faces `founder`'s acting bodies start on in the drained world.
    pub fn start_faces(&self, founder: Founder) -> &[Site] {
        &self.start_faces[founder.index()]
    }

    /// Every founder the seeder placed, both lineages, in id order.
    pub fn placements(&self) -> &[Placement] {
        &self.placements
    }

    /// Whether the world rained, so that a mid-shower state exists.
    pub fn has_mid_shower(&self) -> bool {
        self.mid_shower.is_some()
    }

    /// One lineage's episode fixture in one water state, with its cue field settled
    /// once. `None` for [`WaterState::MidShower`] on a world that did not rain.
    pub fn prepare(&self, founder: Founder, water: WaterState) -> Option<PreparedLandscape> {
        let world = match water {
            WaterState::Drained => self.drained.clone(),
            WaterState::MidShower => self.mid_shower.clone()?,
        };
        // Mid-shower, a start must still be a face the body can stand on in that water.
        let start_faces: Vec<Site> = match (water, &self.mid_shower_standable) {
            (WaterState::MidShower, Some(standable)) => self.start_faces[founder.index()]
                .iter()
                .copied()
                .filter(|s| {
                    standable[founder.index()]
                        .binary_search_by_key(&(s.x, s.y, s.z), |f| (f.x, f.y, f.z))
                        .is_ok()
                })
                .collect(),
            _ => self.start_faces[founder.index()].clone(),
        };
        let mut senses = Senses::new();
        senses.settle(&world.view(), &self.flora.view());
        Some(PreparedLandscape {
            founder,
            preset: self.preset.clone(),
            seed_base: self.seed_base,
            world_seed: self.world_seed,
            water_state: water,
            world,
            flora: self.flora.clone(),
            placements: self.placements.clone(),
            start_faces,
            senses,
            horizon: LANDSCAPE_HORIZON_TICKS,
        })
    }
}

/// One immutable landscape episode fixture: a frozen world, its plants, the placements
/// and the settled cue field. An episode clones the layers into its own static
/// simulator and builds a fresh animal layer from the placements, so nothing one
/// episode does reaches another.
#[derive(Clone)]
pub struct PreparedLandscape {
    /// The lineage whose founders act.
    pub founder: Founder,
    pub preset: String,
    pub seed_base: u64,
    pub world_seed: u64,
    pub water_state: WaterState,
    world: World,
    flora: Flora,
    placements: Vec<Placement>,
    /// Where this lineage's acting bodies may start: the drained world's accepted
    /// components' standable faces, still standable in this water state.
    start_faces: Vec<Site>,
    senses: Senses,
    /// The episode horizon this fixture fixes: [`LANDSCAPE_HORIZON_TICKS`] (D8), or a
    /// test's shorter one ([`PreparedLandscape::with_horizon`]).
    horizon: u64,
}

impl PreparedLandscape {
    /// The horizon every episode on this fixture runs.
    pub fn horizon(&self) -> u64 {
        self.horizon
    }

    /// The same fixture with a shorter horizon, for a test that must stay within a few
    /// hundred ticks. Training and evaluation always run D8's.
    pub fn with_horizon(mut self, ticks: u64) -> PreparedLandscape {
        self.horizon = ticks;
        self
    }

    /// The frozen world, as every episode starts it.
    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn flora(&self) -> &Flora {
        &self.flora
    }

    /// Every placement, both lineages.
    pub fn placements(&self) -> &[Placement] {
        &self.placements
    }

    /// The seeder's placements of this fixture's lineage. Acting bodies do **not**
    /// start here ([`PreparedLandscape::acting_starts`]); their count is the episode's.
    pub fn acting(&self) -> impl Iterator<Item = &Placement> {
        self.placements.iter().filter(|p| p.founder == self.founder)
    }

    /// Where this lineage's acting bodies may start.
    pub fn start_faces(&self) -> &[Site] {
        &self.start_faces
    }

    /// Where one episode's acting bodies start (P5-C, coordinator's decision): as many
    /// bodies as the seeder placed of this lineage, each on a face drawn uniformly from
    /// [`PreparedLandscape::start_faces`] — distinct while the pool lasts — with a
    /// heading drawn uniformly, both a pure function of `episode_seed`. Not the seeder's
    /// founder faces: live, only the eight t0 founders ever stand on a starter tile, and
    /// every later body has to forage. With no start face at all the seeder's own
    /// placements are kept.
    pub fn acting_starts(&self, episode_seed: u64) -> Vec<Placement> {
        let seeder: Vec<Placement> = self.acting().copied().collect();
        if self.start_faces.is_empty() {
            return seeder;
        }
        let mut state = episode_seed ^ self.seed_base.rotate_left(17) ^ START_SALT;
        let mut next = move || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        let mut pool: Vec<Site> = self.start_faces.clone();
        let mut out = Vec::with_capacity(seeder.len());
        for _ in 0..seeder.len() {
            if pool.is_empty() {
                pool = self.start_faces.clone();
            }
            let i = (next() % pool.len() as u64) as usize;
            let site = pool.swap_remove(i);
            let heading_rad = (next() >> 11) as f64 / (1u64 << 53) as f64 * std::f64::consts::TAU;
            out.push(Placement {
                founder: self.founder,
                site,
                heading_rad,
            });
        }
        out
    }

    /// The other lineage's placements, which run their own heuristics.
    pub fn bystanders(&self) -> impl Iterator<Item = &Placement> {
        self.placements.iter().filter(|p| p.founder != self.founder)
    }

    /// A fresh copy of the settled cue field.
    pub fn episode_senses(&self) -> Senses {
        self.senses.clone()
    }

    /// A short label: `preset/base/water`.
    pub fn label(&self) -> String {
        format!(
            "{}/{}/{}",
            self.preset,
            self.seed_base,
            self.water_state.as_str()
        )
    }
}

impl std::fmt::Debug for PreparedLandscape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedLandscape")
            .field("founder", &self.founder.name())
            .field("preset", &self.preset)
            .field("seed_base", &self.seed_base)
            .field("world_seed", &self.world_seed)
            .field("water_state", &self.water_state.as_str())
            .field("acting", &self.acting().count())
            .finish()
    }
}

/// The start draw's own salt.
const START_SALT: u64 = 0x_57A2_7F0C_E5D1_0000;

/// The standable faces of `founder`'s walkable components that the seeder's acceptance
/// judged **habitable**, on the founded (drained) world, sorted. The route map is the
/// acceptance's own ([`RouteMap::for_founder`] on the same world and physiology), so its
/// component indices are the verdict's. A world whose verdict holds no habitable
/// component for the lineage (a test ring accepted unconditionally) falls back to the
/// components the lineage's founders were placed in.
pub fn start_faces_of(founded: &Founded, cfg: &FaunaConfig, founder: Founder) -> Vec<Site> {
    let map = RouteMap::for_founder(&founded.world.view(), cfg.founder(founder));
    let verdict = &founded.seeded.acceptance.lineages[founder.index()];
    let mut keep: Vec<usize> = verdict
        .components
        .iter()
        .filter(|c| c.habitable)
        .map(|c| c.component)
        .collect();
    if keep.is_empty() {
        keep = verdict.components.iter().map(|c| c.component).collect();
    }
    let mut faces: Vec<Site> = map
        .faces
        .iter()
        .zip(&map.components)
        .filter(|(_, c)| keep.contains(c))
        .map(|(f, _)| *f)
        .collect();
    faces.sort_unstable_by_key(|s| (s.x, s.y, s.z));
    faces
}

/// A set of landscapes to train or evaluate on: one preset, its seed bases, one water
/// state (P5-B item 4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandscapeSet {
    pub preset: String,
    pub seeds: Vec<u64>,
    pub water_state: WaterState,
}

impl LandscapeSet {
    /// The training sets: every preset × both water states over the sixteen training
    /// bases — 16 × 3 × 2 episode fixtures per lineage, from 48 foundings.
    pub fn training() -> Vec<LandscapeSet> {
        LandscapeSet::over(&TRAINING_LANDSCAPE_SEEDS)
    }

    /// The held-out sets, over the eight held-out bases.
    pub fn held_out() -> Vec<LandscapeSet> {
        LandscapeSet::over(&HELD_OUT_LANDSCAPE_SEEDS)
    }

    fn over(seeds: &[u64]) -> Vec<LandscapeSet> {
        LANDSCAPE_PRESETS
            .iter()
            .flat_map(|preset| {
                WaterState::ALL.map(|water_state| LandscapeSet {
                    preset: (*preset).to_string(),
                    seeds: seeds.to_vec(),
                    water_state,
                })
            })
            .collect()
    }
}

/// Found every `(preset, seed base)` the sets name, **once each** whatever the water
/// states, on up to `workers` threads. Returned in `(preset, base)` order of first
/// appearance. A founding is single-threaded apart from the world's own water solver.
///
/// Two bases that kept the same world — which [`LANDSCAPE_REDRAW_STRIDE`] rules out —
/// would be reported on stderr and kept, because dropping one would change the set's
/// size under the caller.
pub fn found_landscapes(sets: &[LandscapeSet], workers: usize) -> Result<Vec<Landscape>, String> {
    let mut keys: Vec<(String, u64)> = Vec::new();
    for set in sets {
        for &seed in &set.seeds {
            let key = (set.preset.clone(), seed);
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    for (preset, _) in &keys {
        if cubarium_voxel::Preset::find(preset).is_none() {
            return Err(format!("no landform preset is called {preset:?}"));
        }
    }
    let slots: Mutex<Vec<Option<Landscape>>> = Mutex::new(vec![None; keys.len()]);
    let cursor = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1).min(keys.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    let Some((preset, seed)) = keys.get(i) else {
                        return;
                    };
                    let land = Landscape::found(preset, *seed).expect("the preset was checked");
                    slots.lock().expect("slots")[i] = Some(land);
                }
            });
        }
    });
    let lands: Vec<Landscape> = slots
        .into_inner()
        .expect("slots")
        .into_iter()
        .map(|l| l.expect("every founding ran"))
        .collect();
    for (i, a) in lands.iter().enumerate() {
        for b in &lands[..i] {
            if a.preset == b.preset && a.world_seed == b.world_seed {
                eprintln!(
                    "landscapes: {} seed bases {} and {} both kept world seed {} — the set \
                     holds that world twice",
                    a.preset, b.seed_base, a.seed_base, a.world_seed
                );
            }
        }
        if !a.accepted {
            eprintln!(
                "landscapes: {} seed base {} kept world {} although it failed the \
                 acceptance check",
                a.preset, a.seed_base, a.world_seed
            );
        }
    }
    Ok(lands)
}

/// The episode fixtures `sets` name for `founder`, in set order and then seed order,
/// from landscapes founded once ([`found_landscapes`]). A mid-shower fixture on a world
/// that did not rain is skipped with a line on stderr.
pub fn prepare_sets(
    lands: &[Landscape],
    sets: &[LandscapeSet],
    founder: Founder,
) -> Vec<PreparedLandscape> {
    let mut out = Vec::new();
    for set in sets {
        for &seed in &set.seeds {
            let Some(land) = lands
                .iter()
                .find(|l| l.preset == set.preset && l.seed_base == seed)
            else {
                eprintln!(
                    "landscapes: {} seed base {seed} was not founded",
                    set.preset
                );
                continue;
            };
            match land.prepare(founder, set.water_state) {
                Some(p) => out.push(p),
                None => eprintln!(
                    "landscapes: {} seed base {seed} never rained; no {} fixture",
                    set.preset,
                    set.water_state.as_str()
                ),
            }
        }
    }
    out
}

/// P5-C (C1): every training landscape fixture of `founder` — the sixteen training bases
/// × three presets × both water states, 96 when every world rained — as trainer
/// fixtures, founded on up to `workers` threads.
pub fn training_pool(
    founder: Founder,
    workers: usize,
) -> Result<Vec<super::task::Prepared>, String> {
    let sets = LandscapeSet::training();
    let lands = found_landscapes(&sets, workers)?;
    Ok(prepare_sets(&lands, &sets, founder)
        .into_iter()
        .map(super::task::Prepared::from)
        .collect())
}

/// P5-C (C2): the held-out landscape fixtures of `founder` — the eight held-out bases ×
/// three presets, **drained** only, 24 — as trainer fixtures. Never trained on and never
/// drawn from; the ship candidate is chosen on these.
pub fn held_out_pool(
    founder: Founder,
    workers: usize,
) -> Result<Vec<super::task::Prepared>, String> {
    let sets: Vec<LandscapeSet> = LandscapeSet::held_out()
        .into_iter()
        .filter(|s| s.water_state == WaterState::Drained)
        .collect();
    let lands = found_landscapes(&sets, workers)?;
    Ok(prepare_sets(&lands, &sets, founder)
        .into_iter()
        .map(super::task::Prepared::from)
        .collect())
}

/// C1's per-generation draw: `k` distinct indices into a pool of `pool` fixtures, a pure
/// function of `(train_seed, generation)` in the trainer's own stream — the same for
/// every candidate of that generation (both signs of a pair and the centre see the same
/// worlds) and a fresh draw the next. A partial Fisher–Yates over splitmix64; `k` is
/// clamped to the pool.
pub fn generation_draw(train_seed: u64, generation: u32, pool: usize, k: usize) -> Vec<usize> {
    let mut state =
        train_seed ^ u64::from(generation).wrapping_mul(0xD1B5_4A32_D192_ED03) ^ DRAW_SALT;
    let mut next = move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let mut order: Vec<usize> = (0..pool).collect();
    let k = k.min(pool);
    for i in 0..k {
        let j = i + (next() % (pool - i) as u64) as usize;
        order.swap(i, j);
    }
    order.truncate(k);
    order
}

/// The landscape draw's own salt, so it never shares a stream with a perturbation or a
/// body-size draw.
const DRAW_SALT: u64 = 0x_1A4D_5CA9_E0C1_0000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generation_draw_is_distinct_repeatable_and_moves_between_generations() {
        let a = generation_draw(7, 3, 96, 16);
        assert_eq!(a.len(), 16);
        let mut sorted = a.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 16, "sixteen distinct worlds");
        assert!(a.iter().all(|&i| i < 96));
        assert_eq!(
            a,
            generation_draw(7, 3, 96, 16),
            "a pure function of its seed"
        );
        assert_ne!(
            a,
            generation_draw(7, 4, 96, 16),
            "a fresh draw each generation"
        );
        assert_ne!(a, generation_draw(8, 3, 96, 16), "and each run seed");
        assert_eq!(generation_draw(7, 3, 5, 16).len(), 5, "clamped to the pool");
        // Over many generations every world is drawn.
        let mut seen = [false; 96];
        for g in 0..64 {
            for i in generation_draw(7, g, 96, 16) {
                seen[i] = true;
            }
        }
        assert!(seen.iter().all(|s| *s));
    }
}
