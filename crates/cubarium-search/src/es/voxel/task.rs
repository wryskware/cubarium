//! The frozen fixture: founders, layout seeds, episode bounds, and the immutable prepared
//! layout one episode clones its private mutable copy from.
//!
//! Everything here is data the tests plan fixes ("Use four training layout seeds and eight
//! untouched evaluation seeds per archetype"; "Begin with 1,200 ticks"; the §3 bounds
//! table). A layout seed varies geometry, resource placement and the arena's starting
//! heading within the task — never the sensory contract, which the manifest owns.
//!
//! # Prepared layouts
//!
//! [`Prepared`] is one [`cubarium_voxel_sim::Arena`] built once from
//! `(founder, layout_seed)` and kept **immutable**. An episode takes
//! [`Prepared::episode_arena`], which clones the three layers into the episode's own
//! private mutable copy — that clone *is* the fresh-episode reset of every mutable arena
//! datum (stocks, the fauna's ledger, the placed animal), and it is also what makes the
//! two signs of an antithetic pair bit-identical worlds, because [`Arena::build`] is a pure
//! function of its seed. Rebuilding from the seed stays available
//! ([`Prepared::rebuilt`]) so the bench can measure the two setup paths against each other.

use cubarium_voxel_fauna::{Founder, Senses};
use cubarium_voxel_sim::{ARENA_VOXEL_M, ARENA_WIDTH, Arena, Site};

/// The phase-one pilot's episode horizon: 1,200 ticks is 60 simulated seconds at 20 Hz
/// (`design/voxel-senses-phase1-tests.md` §2). "Ample for multiple body lengths without
/// inheriting the old 36,000-tick campaign."
pub const HORIZON_TICKS: u64 = 1_200;

/// Stage B's horizon: 2,400 ticks, 120 simulated seconds.
///
/// Sized from the two legs the task actually has, at the plan's 1 BL/s cruise:
///
/// - **Deplete.** Stage B's initial patch is about 15 s of full-effort feeding; its
///   successor remains 30 s. This leaves the learned controller time for the second leg.
/// - **Reacquire.** The successor stays at its landed 2 m minimum; the arena's widest
///   separation is 4.85 m (16 wrapped columns by 11, at 0.25 m). At 1 BL/s that is
///   16 s (blind, 0.125 m body) to 39 s in the worst layout, and 8-19 s for the
///   browser's 0.25 m body. After the approach (about 10 s) and 15 s of feeding,
///   95 s remain: two to five times the straight-line cost, which is the room for
///   searching. At the Stage-A horizon of 1,200 ticks the worst layout would leave
///   15 s against a 39 s walk — not a task, a lottery.
pub const STAGE_B_HORIZON_TICKS: u64 = 2_400;

/// Which arena task a run uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Stage {
    /// Acquire: the six-tile Stage-A layout, one off-food start inside the signal.
    #[default]
    A,
    /// Persist and leave: two finite patches, the first small enough to run out
    /// (`Arena::build_reacquisition`).
    B,
}

impl Stage {
    /// The stage's own default horizon.
    pub fn horizon(self) -> u64 {
        match self {
            Stage::A => HORIZON_TICKS,
            Stage::B => STAGE_B_HORIZON_TICKS,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Stage::A => "a",
            Stage::B => "b",
        }
    }
}

/// A stage from a command-line name.
pub fn parse_stage(s: &str) -> Result<Stage, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "a" | "acquire" | "stage-a" => Ok(Stage::A),
        "b" | "reacquire" | "reacquisition" | "stage-b" => Ok(Stage::B),
        other => Err(format!("unknown --stage `{other}`; use `a` or `b`")),
    }
}

/// When the Stage-B evaluator calls the initial patch depleted: its stock has fallen
/// below this fraction of what it started with. A tenth, not zero: the last crumbs of a
/// patch are below one bite and a founder that keeps chewing them is no longer being fed
/// by it.
pub const DEPLETION_FRACTION: f64 = 0.1;

/// The sixteen training layout seeds (P2-D).
///
/// Four layouts estimated the gradient against an objective whose between-layout spread
/// is ten times the per-generation perturbation spread, so most of what the ES ranked
/// was which layouts a candidate happened to land food on. Sixteen is the same protocol
/// with four times the sample per candidate.
///
/// Chosen so **none** is a seam layout (`layout_seed % 4 == 3` is the arena's deliberate
/// seam test, and the plan's default layouts keep food away from the seam), and all
/// sixteen are disjoint from [`EVALUATION_LAYOUT_SEEDS`] — and from P2-C's four, so the
/// held-out comparison is against a genuinely fresh training set.
pub const TRAINING_LAYOUT_SEEDS: [u64; 16] = [
    20, 21, 22, 24, 25, 26, 28, 29, 30, 32, 33, 34, 36, 37, 38, 40,
];

/// Eight held-out Phase-3 evaluation seeds, never used to choose parameters or the best
/// generation. For blind Stage B their signed start offsets occupy all four sign/range
/// bins equally: negative/positive × near/far (two layouts in each). This prevents a
/// one-handed opening arc from passing because one side happened to dominate the set.
pub const EVALUATION_LAYOUT_SEEDS: [u64; 8] = [0, 1, 4, 5, 9, 41, 60, 61];

/// The Stage-A start-heading convention this build's arena places founders under
/// (P2-B step 2): blind uniform over the circle, browser uniform within +/-90 degrees of
/// the bearing. Phase one aimed both founders at their food with a +/-5 degree jitter,
/// which is a different task; a policy trained under it is refused rather than
/// reinterpreted, both through [`super::trainer::VoxelProtocol::hash`] and through the
/// policy file's own field.
pub const START_HEADING_PROTOCOL: &str = "p2b-varied-1";

/// How full this build's arenas introduce a founder (P2-C):
/// `cubarium_voxel_sim::FOUNDER_START`, half the body and no reserve. A policy trained
/// against a full start was trained where intake could only repay upkeep, which is a
/// different task; it is refused rather than reinterpreted.
pub const STARTING_STORES_PROTOCOL: &str = "p2c-half-body-no-reserve";

/// Stage B's fixture revision. P3-A halves only the initial edible stock while retaining
/// the successor stock and the browser crown. A Stage-B centre from the earlier equal-
/// patch task is refused rather than silently called a policy for this one.
pub const STAGE_B_ARENA_PROTOCOL: &str = "p3a-half-initial-patch-1";

/// The arena protocol recorded for a trained stage.
pub fn arena_protocol(stage: Stage) -> &'static str {
    match stage {
        Stage::A => "p2b-stage-a-1",
        Stage::B => STAGE_B_ARENA_PROTOCOL,
    }
}

/// One fixed Stage-B layout used for the diagnostic blind heading sweep.
pub const OFFSET_SWEEP_LAYOUT_SEED: u64 = 6;
/// Inclusive signed offsets, in degrees: -180, -165, ..., +180.
pub const OFFSET_SWEEP_DEGREES: [i16; 25] = [
    -180, -165, -150, -135, -120, -105, -90, -75, -60, -45, -30, -15, 0, 15, 30, 45, 60, 75, 90,
    105, 120, 135, 150, 165, 180,
];

/// The trainer's default train seed, in the trainer's own stream — its randomness never
/// touches a world draw ([`super::trainer`]).
pub const TRAINING_SEED: u64 = 20_260_918;

/// Antithetic pairs per generation. The plan's eight became thirty-two in P2-D: the
/// gradient was being estimated from eight pairs against layout noise an order of
/// magnitude larger than the signal.
pub const DEFAULT_PAIRS: usize = 32;

/// Centre evaluations per generation: the unperturbed centre on **every** training layout.
pub const CENTER_EVALUATIONS: usize = TRAINING_LAYOUT_SEEDS.len();

/// Updates per archetype the pilot bounds itself to (plan §3).
/// Updates per archetype a run bounds itself to. The plan's 32 became 64 in P2-B and 512
/// in P2-D: every P2-C run selected a centre at or within five of its cap (browser A and
/// B both at generation 63 of 63) while spending 1.6-3.7 s of a 900 s budget. The wall
/// cap, not this, is what should stop a run.
pub const MAX_UPDATES: u32 = 512;

/// The voxel trainer's measured saturation point. The machine-wide policy also reserves
/// ten percent of logical CPUs, but this workload gains little beyond sixteen workers.
pub const MAX_EPISODE_WORKERS: usize = 16;

/// The available voxel episode workers: obey the machine-wide CPU reserve and the
/// workload-specific saturation cap. Every episode itself stays single-threaded.
pub fn episode_worker_limit() -> usize {
    worker_limit_for(std::thread::available_parallelism().map_or(1, std::num::NonZero::get))
        .min(MAX_EPISODE_WORKERS)
}

fn worker_limit_for(cpus: usize) -> usize {
    let cpus = cpus.max(1);
    let reserved = ((cpus + 9) / 10).max(1);
    cpus.saturating_sub(reserved).max(1)
}

/// Wall seconds one archetype's training is allotted. Fifteen minutes since P2-B.
pub const DEFAULT_TRAIN_WALL_SECONDS: u64 = 900;

/// The episode count one archetype's full bounded training needs: the initial centre
/// evaluation (one per training layout) plus `MAX_UPDATES × (2·pairs + 1) × layouts`.
/// At P2-D's 32 pairs, 512 updates and 16 layouts that is 532,496 — the limit is a
/// backstop against a runaway run, not the thing that ends a healthy one (the 900 s
/// wall cap is).
pub const DEFAULT_EPISODE_LIMIT: u64 = CENTER_EVALUATIONS as u64
    + MAX_UPDATES as u64 * (2 * DEFAULT_PAIRS as u64 + 1) * TRAINING_LAYOUT_SEEDS.len() as u64;

/// Where the founder starts relative to the patch the arena placed it in signal of.
///
/// **Fixture-side only.** This is the geometry `Arena::build` used to choose the start
/// and the evaluator recomputes afterwards to read a result; it is not an observation,
/// not a reward term, and no controller can reach it. P2-D added it to ask what
/// separates the layouts a policy feeds on from the ones it wanders.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StartGeometry {
    /// The patch in question: Stage B's initial patch, or — on Stage A, where the
    /// placement aimed at whichever resource was nearest — that nearest resource.
    pub target: Site,
    /// Metres from the start pose to that patch's face centre, across the wrapped strip.
    pub distance_m: f64,
    /// The signed turn the body would have to make to face it at the first tick:
    /// `bearing − heading`, wrapped to `(−pi, pi]`. Positive is a positive yaw, the
    /// sign of the turn action that would close it.
    pub turn_to_target_rad: f64,
}

impl StartGeometry {
    /// `+1` when the patch lies to the positive-yaw side of the start heading, `-1`
    /// otherwise: the "patch side" of the bimodality reading.
    pub fn side(&self) -> i8 {
        if self.turn_to_target_rad >= 0.0 {
            1
        } else {
            -1
        }
    }
}

/// The shortest signed x displacement across the wrapped strip, in metres.
fn wrapped_dx_m(from_x: f64, to_x: f64) -> f64 {
    let width = f64::from(ARENA_WIDTH) * ARENA_VOXEL_M;
    let raw = to_x - from_x;
    [raw, raw - width, raw + width]
        .into_iter()
        .min_by(|a, b| a.abs().partial_cmp(&b.abs()).expect("finite"))
        .expect("three wrapped displacements")
}

/// One immutable prepared arena: the terrain, the pond, the finite resources and the
/// placed founder, exactly as [`Arena::build`] left them.
pub struct Prepared {
    pub founder: Founder,
    pub layout_seed: u64,
    pub stage: Stage,
    /// Stage B's two patch sites, `(initial, successor)`. **Evaluator-only**: the
    /// driver's epilogue reads their stocks to report reacquisition, and nothing on the
    /// observation path ever sees them.
    patches: Option<(Site, Site)>,
    arena: Arena,
    /// The arena's settled cue field. Every episode clones it into its private simulator;
    /// field settlement belongs to fixture preparation, never to every candidate rollout.
    senses: Senses,
}

impl Prepared {
    /// Build the prepared Stage-A layout: one [`Arena::build`], kept immutable.
    pub fn build(founder: Founder, layout_seed: u64) -> Prepared {
        Prepared::build_stage(founder, layout_seed, Stage::A)
    }

    /// Build the prepared layout for `stage`, kept immutable.
    pub fn build_stage(founder: Founder, layout_seed: u64, stage: Stage) -> Prepared {
        let (arena, patches) = match stage {
            Stage::A => (Arena::build(founder, layout_seed), None),
            Stage::B => {
                let (arena, initial, successor) =
                    Arena::build_reacquisition(founder, layout_seed).into_parts();
                (arena, Some((initial, successor)))
            }
        };
        Prepared {
            founder,
            layout_seed,
            stage,
            patches,
            senses: arena.prepare_senses(),
            arena,
        }
    }

    /// Build one Stage-B diagnostic case with an exact fixture-side signed start turn.
    /// The target and angle remain outside the controller boundary.
    pub fn build_reacquisition_with_start_turn(
        founder: Founder,
        layout_seed: u64,
        turn_to_initial_rad: f64,
    ) -> Prepared {
        let (arena, initial, successor) =
            Arena::build_reacquisition_with_start_turn(founder, layout_seed, turn_to_initial_rad)
                .into_parts();
        Prepared {
            founder,
            layout_seed,
            stage: Stage::B,
            patches: Some((initial, successor)),
            senses: arena.prepare_senses(),
            arena,
        }
    }

    /// Stage B's `(initial, successor)` patch sites, for the evaluator's accounting.
    pub fn patches(&self) -> Option<(Site, Site)> {
        self.patches
    }

    /// The start geometry of this layout ([`StartGeometry`]), or `None` when the arena
    /// placed no body. Fixture-side; the driver never calls it.
    pub fn start_geometry(&self) -> Option<StartGeometry> {
        let pose = self.arena.animal_pose()?;
        let target = match self.patches {
            Some((initial, _)) => initial,
            None => *self.arena.resources.iter().min_by(|a, b| {
                let d = |s: &Site| {
                    let dx = wrapped_dx_m(pose.x, (f64::from(s.x) + 0.5) * ARENA_VOXEL_M);
                    let dz = (f64::from(s.z) + 0.5) * ARENA_VOXEL_M - pose.z;
                    dx * dx + dz * dz
                };
                d(a).partial_cmp(&d(b)).expect("finite")
            })?,
        };
        let dx = wrapped_dx_m(pose.x, (f64::from(target.x) + 0.5) * ARENA_VOXEL_M);
        let dz = (f64::from(target.z) + 0.5) * ARENA_VOXEL_M - pose.z;
        // The arena's own heading convention: atan2(dx, dz).
        let bearing = dx.atan2(dz);
        let delta = bearing - pose.heading_rad;
        Some(StartGeometry {
            target,
            distance_m: dx.hypot(dz),
            turn_to_target_rad: delta.sin().atan2(delta.cos()),
        })
    }

    /// A fresh **private mutable copy** of the arena for one episode: every mutable datum
    /// — stocks, ledger, the placed animal — starts at the prepared state, and nothing one
    /// episode does can reach the prepared layout or another episode.
    ///
    /// The clone carries `births = false` (the arena set it at build) and the placed
    /// founder at its starting pose.
    pub fn episode_arena(&self) -> Arena {
        Arena {
            founder: self.arena.founder,
            layout_seed: self.arena.layout_seed,
            world: self.arena.world.clone(),
            flora: self.arena.flora.clone(),
            fauna: self.arena.fauna.clone(),
            resources: self.arena.resources.clone(),
            animal_id: self.arena.animal_id,
        }
    }

    /// A fresh copy of the settled cue field for one private episode simulator.
    pub fn episode_senses(&self) -> Senses {
        self.senses.clone()
    }

    /// A fresh arena rebuilt from the seed — the other setup path, measured beside the
    /// clone path by the bench. Both must prepare the identical arena.
    pub fn rebuilt(&self) -> Arena {
        match self.stage {
            Stage::A => Arena::build(self.founder, self.layout_seed),
            Stage::B => Arena::build_reacquisition(self.founder, self.layout_seed).into_arena(),
        }
    }

    /// The placed founder's id, if the arena landed one.
    pub fn animal_id(&self) -> Option<u64> {
        self.arena.animal_id
    }

    /// The immutable prepared arena, for the fixture's own validation surface. **The
    /// episode driver and every observation source must not read its `resources` or its
    /// stock** (`super`'s boundaries): this accessor exists for `voxel-check` and tests.
    pub fn fixture_arena(&self) -> &Arena {
        &self.arena
    }
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared")
            .field("founder", &self.arena.founder.name())
            .field("stage", &self.stage.as_str())
            .field("layout_seed", &self.layout_seed)
            .field("animal_id", &self.arena.animal_id)
            .field("resources", &self.arena.resources.len())
            .finish()
    }
}

/// The training layout seeds, as [`Prepared`] layouts of `stage`, in the frozen order.
pub fn training_layouts(founder: Founder, stage: Stage) -> Vec<Prepared> {
    TRAINING_LAYOUT_SEEDS
        .map(|seed| Prepared::build_stage(founder, seed, stage))
        .into()
}

/// The evaluation layout seeds, as [`Prepared`] layouts of `stage`, in the frozen order.
pub fn evaluation_layouts(founder: Founder, stage: Stage) -> Vec<Prepared> {
    EVALUATION_LAYOUT_SEEDS
        .map(|seed| Prepared::build_stage(founder, seed, stage))
        .into()
}

/// The blind Stage-B diagnostic sweep: one terrain/resource/start position, with only
/// the start heading changed in balanced 15-degree increments.
pub fn offset_sweep_layouts() -> Vec<(String, Prepared)> {
    OFFSET_SWEEP_DEGREES
        .into_iter()
        .map(|degrees| {
            (
                format!("{degrees:+}deg"),
                Prepared::build_reacquisition_with_start_turn(
                    Founder::Blind,
                    OFFSET_SWEEP_LAYOUT_SEED,
                    f64::from(degrees).to_radians(),
                ),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blind_stage_b_holdout_is_balanced_by_side_and_range() {
        let mut bins = [0usize; 4];
        for seed in EVALUATION_LAYOUT_SEEDS {
            let turn = Prepared::build_stage(Founder::Blind, seed, Stage::B)
                .start_geometry()
                .expect("the held-out layout places a founder")
                .turn_to_target_rad
                .to_degrees();
            let side = usize::from(turn >= 0.0);
            let range = usize::from(turn.abs() > 90.0);
            bins[side * 2 + range] += 1;
        }
        assert_eq!(bins, [2, 2, 2, 2], "negative/positive x near/far");
    }

    #[test]
    fn the_offset_sweep_changes_only_the_blind_start_heading() {
        let cases = offset_sweep_layouts();
        assert_eq!(cases.len(), OFFSET_SWEEP_DEGREES.len());
        let first = &cases[0].1;
        let first_pose = first.fixture_arena().animal_pose().expect("founder");
        let first_geometry = first.start_geometry().expect("geometry");
        for ((label, prepared), expected) in cases.iter().zip(OFFSET_SWEEP_DEGREES) {
            let pose = prepared.fixture_arena().animal_pose().expect("founder");
            let geometry = prepared.start_geometry().expect("geometry");
            assert_eq!(prepared.founder, Founder::Blind);
            assert_eq!(prepared.stage, Stage::B);
            assert_eq!(prepared.layout_seed, OFFSET_SWEEP_LAYOUT_SEED);
            assert_eq!(pose.x, first_pose.x, "{label}");
            assert_eq!(pose.z, first_pose.z, "{label}");
            assert_eq!(geometry.target, first_geometry.target, "{label}");
            assert_eq!(geometry.distance_m, first_geometry.distance_m, "{label}");
            let error = geometry.turn_to_target_rad - f64::from(expected).to_radians();
            assert!(
                error.sin().atan2(error.cos()).abs() < 1e-12,
                "{label}: got {} degrees",
                geometry.turn_to_target_rad.to_degrees()
            );
        }
    }

    /// No training or evaluation seed is the arena's seam layout, so the default fixtures
    /// keep food away from the wrapped seam as the plan says.
    #[test]
    fn the_frozen_seeds_avoid_the_seam_layout() {
        for seed in TRAINING_LAYOUT_SEEDS
            .into_iter()
            .chain(EVALUATION_LAYOUT_SEEDS)
        {
            assert_ne!(
                seed % 4,
                3,
                "seed {seed} is the arena's deliberate seam layout"
            );
        }
    }

    /// The two sets are disjoint and internally unique — the held-out eight must never
    /// have been trained on — and every seed on both lists actually builds a usable
    /// layout for both founders and both stages: a placed founder and a stocked patch.
    /// A seed that quietly placed no body would shrink a generation without saying so.
    #[test]
    fn every_frozen_seed_builds_a_usable_layout_for_both_founders_and_stages() {
        let mut all: Vec<u64> = TRAINING_LAYOUT_SEEDS
            .into_iter()
            .chain(EVALUATION_LAYOUT_SEEDS)
            .collect();
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(
            all.len(),
            n,
            "a seed appears on both lists, or twice on one"
        );

        for seed in TRAINING_LAYOUT_SEEDS
            .into_iter()
            .chain(EVALUATION_LAYOUT_SEEDS)
        {
            for founder in Founder::ALL {
                for stage in [Stage::A, Stage::B] {
                    let p = Prepared::build_stage(founder, seed, stage);
                    assert!(
                        p.animal_id().is_some(),
                        "{founder:?} seed {seed} stage {}: no founder was placed",
                        stage.as_str()
                    );
                    let stock = p.fixture_arena().resource_stock();
                    assert!(
                        stock.is_finite() && stock > 0.0,
                        "{founder:?} seed {seed} stage {}: stock {stock}",
                        stage.as_str()
                    );
                    if stage == Stage::B {
                        let (initial, successor) = p.patches().expect("Stage B names patches");
                        assert_ne!(initial, successor, "seed {seed}");
                    }
                }
            }
        }
    }

    #[test]
    fn worker_limit_reserves_ten_percent_then_stops_at_measured_saturation() {
        assert_eq!(worker_limit_for(1), 1);
        assert_eq!(worker_limit_for(2), 1);
        assert_eq!(worker_limit_for(10), 9);
        assert_eq!(worker_limit_for(32).min(MAX_EPISODE_WORKERS), 16);
        assert_eq!(worker_limit_for(128).min(MAX_EPISODE_WORKERS), 16);
    }

    /// The two setup paths prepare the identical arena: cloning the immutable layout and
    /// rebuilding from the seed give the same placed animal, the same stock and the same
    /// world material.
    #[test]
    fn clone_and_rebuild_prepare_identical_arenas() {
        for founder in Founder::ALL {
            let p = Prepared::build(founder, TRAINING_LAYOUT_SEEDS[2]);
            let cloned = p.episode_arena();
            let rebuilt = p.rebuilt();
            assert_eq!(cloned.animal_id, rebuilt.animal_id);
            assert_eq!(cloned.animal_pose(), rebuilt.animal_pose());
            assert_eq!(
                format!("{:?}", cloned.world),
                format!("{:?}", rebuilt.world),
                "the cloned world equals the rebuilt one"
            );
        }
    }

    /// The start geometry is the geometry the arena actually used: the target is within
    /// the placement's 0.5-1 m band, and the turn that would close the heading is inside
    /// the band each founder's start convention allows — a half-turn either way for the
    /// blind founder, at most 90 degrees for the browser, whose stand has to be
    /// somewhere its three sectors cover.
    #[test]
    fn the_start_geometry_is_the_placement_the_arena_made() {
        for seed in EVALUATION_LAYOUT_SEEDS {
            for founder in Founder::ALL {
                for stage in [Stage::A, Stage::B] {
                    let p = Prepared::build_stage(founder, seed, stage);
                    let g = p.start_geometry().expect("a placed body has a geometry");
                    assert!(
                        (0.5..=1.02).contains(&g.distance_m),
                        "{founder:?} seed {seed} stage {}: start {} m from the patch",
                        stage.as_str(),
                        g.distance_m
                    );
                    assert!(g.turn_to_target_rad.abs() <= std::f64::consts::PI + 1e-12);
                    if founder == Founder::Browser {
                        assert!(
                            g.turn_to_target_rad.abs() <= std::f64::consts::FRAC_PI_2 + 1e-9,
                            "seed {seed}: the browser's stand is outside its sector fan \
                             by {} degrees",
                            g.turn_to_target_rad.to_degrees()
                        );
                    }
                    assert!(g.side() == 1 || g.side() == -1);
                    if stage == Stage::B {
                        assert_eq!(g.target, p.patches().expect("patches").0);
                    }
                }
            }
        }
    }

    /// The fresh-episode reset: mutable arena data starts at the prepared state, and what
    /// one episode's private copy does stays in that copy. The fixture itself is allowed
    /// to read the stock — this is exactly what the arena built the accessor for — while
    /// the driver never does.
    #[test]
    fn an_episode_leaves_the_prepared_layout_and_other_copies_untouched() {
        let p = Prepared::build(Founder::Blind, TRAINING_LAYOUT_SEEDS[0]);
        let before = p.fixture_arena().resource_stock();
        assert!(
            before.is_finite() && before > 0.0,
            "a finite prepared stock"
        );
        let site = p.fixture_arena().resources[0];

        // One episode copy consumes part of the stock through the fixture's settlement API.
        let mut episode = p.episode_arena();
        let taken = episode.take(site, 0.05).expect("the stock is takeable");
        assert!(taken.organic > 0.0);
        assert!(episode.resource_stock() < before);

        // The prepared layout and a second fresh copy are untouched by it.
        assert_eq!(p.fixture_arena().resource_stock(), before);
        let again = p.episode_arena();
        assert_eq!(again.resource_stock(), before);
        assert_eq!(again.animal_pose(), p.episode_arena().animal_pose());
    }
}
