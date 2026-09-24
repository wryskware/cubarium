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
use cubarium_voxel_sim::{Arena, ArenaGrid, Site, SuccessorBand};

use super::landscape::PreparedLandscape;

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

/// How full this build's episodes introduce an acting body: D11's draw, structure
/// uniform between the lineage's newborn `body_min` and its adult `body_max`, no reserve
/// ([`super::driver::sampled_stores`]). P2-C's `p2c-half-body-no-reserve` (half the body,
/// no reserve) is a different task; a policy trained under it is refused rather than
/// reinterpreted. Moved by P5-C before its training runs, so the centres it trains carry
/// the string of the task they were trained on (P5-B held it back only so the host would
/// still start; the retrain branch's host falls back to the heuristics instead).
pub const STARTING_STORES_PROTOCOL: &str = "p5-sampled-body-no-reserve";

/// Stage B's landed fixture revision. P3-A halves only the initial edible stock while
/// retaining the successor stock and the browser crown. A Stage-B centre from the
/// earlier equal-patch task is refused rather than silently called a policy for this one.
///
/// P3-B did **not** re-anchor this string: the near band is scoped so that a landed
/// layout is built from exactly the draws P3-A made, so a P3-A landed centre is still a
/// policy for this task and re-anchoring would only destroy a usable artefact. The band
/// is disclosed through [`STAGE_B_NEAR_ARENA_PROTOCOL`] instead, which is a different
/// string and therefore a different protocol hash.
pub const STAGE_B_LANDED_ARENA_PROTOCOL: &str = "p3a-half-initial-patch-1";

/// Stage B's near rung (P3-B step 2): the successor sits 4 to under 6 columns from the
/// initial patch, inside both founders' sensed radii. That is a different task from the
/// landed one — the founder can smell or see where to go next — so its centres carry
/// their own protocol string and an unqualified landed evaluation refuses them.
pub const STAGE_B_NEAR_ARENA_PROTOCOL: &str = "p3b-near-successor-1";

/// The browser's crown-height revision of every arena it trains on
/// (`design/handoffs/voxel-browser-reach-2026-09-21.md`, step 2): its stands' crowns are
/// laid at the head layer, one voxel above it and — Stage A only, where the tile is not
/// a scored patch — two voxels above it, out of the vertical mouth reach. The stock per
/// stand, the resource sites, the pond and the start placement are exactly what they
/// were; only the crown heights moved. A browser centre trained on the flat all-at-head
/// geometry is a policy for a different task and is refused.
///
/// The **blind** founder's litter arenas are untouched, so its protocol strings are
/// untouched and its P3-C centre still qualifies. Versioning per founder is the honest
/// statement: one lineage's task changed and the other's did not.
pub const BROWSER_CROWN_HEIGHT_REVISION: &str = "p3d-crown-heights-1";

/// The Stage-B arena protocol for one separation band, for the blind founder's litter
/// patches. The browser's own revision is appended by [`arena_protocol`].
pub fn stage_b_arena_protocol(band: Band) -> &'static str {
    match band {
        Band::Landed => STAGE_B_LANDED_ARENA_PROTOCOL,
        Band::Near => STAGE_B_NEAR_ARENA_PROTOCOL,
    }
}

/// The arena protocol recorded for a trained founder, stage and band. Stage A has no
/// successor, so its band is ignored; the browser carries
/// [`BROWSER_CROWN_HEIGHT_REVISION`] on top of the stage's own string.
pub fn arena_protocol(founder: Founder, stage: Stage, band: Band) -> String {
    let base = match stage {
        Stage::A => STAGE_A_ARENA_PROTOCOL,
        Stage::B => stage_b_arena_protocol(band),
    };
    let base = format!("{base}+{P5_ARENA_REVISION}");
    match founder {
        Founder::Blind => base,
        Founder::Browser => format!("{base}+{BROWSER_CROWN_HEIGHT_REVISION}"),
    }
}

/// P5-B's arena revision, both founders (`design/handoffs/voxel-retrain-2026-09-22.md`,
/// item 3): every patch in the mouth band (seedlings, basal rosettes, a stripped upper
/// crown as a Stage-A distractor; litter, carrion and glowcap caps for the shredder) and
/// two bystander bodies of the other lineage. Neither founder's earlier centre is a
/// policy for this task.
pub const P5_ARENA_REVISION: &str = "p5-arena-1";

/// Stage A's fixture revision, shared by both founders.
pub const STAGE_A_ARENA_PROTOCOL: &str = "p2b-stage-a-1";

/// Stage B's successor separation band, the P3-B curriculum rung.
pub type Band = SuccessorBand;

/// A band from a command-line name.
pub fn parse_band(s: &str) -> Result<Band, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "near" => Ok(Band::Near),
        "landed" | "far" => Ok(Band::Landed),
        other => Err(format!("unknown --band `{other}`; use `near` or `landed`")),
    }
}

/// How far a blind founder can sense a Stage-B litter patch, in metres.
///
/// Measured, not chosen (integration note 4, measurement 1): the settled field response
/// `q = cue/(cue+1)` of one 0.015 successor patch on a flat support falls 0.17 at 0 m,
/// 1.5e-2 at 0.5 m, 8.3e-4 at 1.0 m, 4.2e-5 at 1.5 m, and is **exactly zero** from
/// 1.75 m out, where the cue drops below the field's 1e-5 discard. 1.5 m is therefore
/// the last distance at which a blind founder reads the successor at all.
///
/// Fixture-side only: this constant sizes an evaluator's counter. No observation, no
/// reward and no controller sees it.
pub const BLIND_CUE_REACH_M: f64 = 1.5;

/// The radius within which `founder` can sense a Stage-B successor patch at all.
///
/// Blind: [`BLIND_CUE_REACH_M`]. Browser: its manifest's own `cone_range_m` — the ray
/// fan simply stops there — so the number is read from the manifest rather than copied.
pub fn sensed_radius_m(founder: Founder) -> f64 {
    match founder {
        Founder::Blind => BLIND_CUE_REACH_M,
        Founder::Browser => founder.manifest().cone_range_m,
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

/// The voxel trainer's cap on episode workers in one process. It was sixteen, a
/// saturation point measured before the P5 landscapes; the cache study (2026-09-24,
/// `design/handoffs/voxel-cache-and-pinning-2026-09-24.md`) measured browser landscapes
/// at 258-265k ticks/s on 16 workers spread over both chiplets and 297-310k on 32, so
/// the cap is now the desktop's thread count and the ten-percent reserve and the
/// affinity mask (`taskset`) are what bound a run.
pub const MAX_EPISODE_WORKERS: usize = 32;

/// The available voxel episode workers: obey the machine-wide CPU reserve and the
/// workload-specific saturation cap. Every episode itself stays single-threaded.
///
/// The reserve is the **machine's** (ten percent of its online CPUs, at least one); the
/// result is then capped at the CPUs this process may use. Under
/// `taskset -c 0-7,16-23` on the 32-thread desktop that is all sixteen of them — the
/// other chiplet is the reserve — where reserving ten percent of the mask as well left
/// two of the sixteen idle.
pub fn episode_worker_limit() -> usize {
    let mask = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let machine = std::fs::read_to_string("/sys/devices/system/cpu/online")
        .ok()
        .and_then(|s| super::pin::parse_cpu_list(&s).ok())
        .map_or(mask, |cpus| cpus.len());
    worker_limit_within(machine, mask).min(MAX_EPISODE_WORKERS)
}

/// [`worker_limit_for`] the machine, capped at the process's own CPUs.
fn worker_limit_within(machine: usize, mask: usize) -> usize {
    worker_limit_for(machine.max(mask)).min(mask.max(1))
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

/// The face centre of a site on `grid`, in metres.
fn site_center_m(site: Site, grid: ArenaGrid) -> (f64, f64) {
    (
        (f64::from(site.x) + 0.5) * grid.voxel_m(),
        (f64::from(site.z) + 0.5) * grid.voxel_m(),
    )
}

/// Metres from a point to a site's face centre, across the wrapped strip.
///
/// **Fixture-side only**, like [`StartGeometry`]: the Stage-B evaluator reads it after
/// a tick has already been simulated, and no observation or reward term can reach it.
pub fn distance_to_site_m(x: f64, z: f64, site: Site, grid: ArenaGrid) -> f64 {
    let (cx, cz) = site_center_m(site, grid);
    wrapped_dx_m(x, cx, grid).hypot(cz - z)
}

/// Metres between two sites' face centres, across the wrapped strip. Fixture-side.
pub fn site_separation_m(a: Site, b: Site, grid: ArenaGrid) -> f64 {
    let (ax, az) = site_center_m(a, grid);
    distance_to_site_m(ax, az, b, grid)
}

/// The shortest signed x displacement across the wrapped strip, in metres.
fn wrapped_dx_m(from_x: f64, to_x: f64, grid: ArenaGrid) -> f64 {
    let width = f64::from(grid.width()) * grid.voxel_m();
    let raw = to_x - from_x;
    [raw, raw - width, raw + width]
        .into_iter()
        .min_by(|a, b| a.abs().partial_cmp(&b.abs()).expect("finite"))
        .expect("three wrapped displacements")
}

/// One immutable prepared arena: the terrain, the pond, the finite resources and the
/// placed founder, exactly as [`Arena::build`] left them.
#[derive(Clone)]
pub struct PreparedArena {
    pub founder: Founder,
    pub layout_seed: u64,
    pub stage: Stage,
    /// The cells the arena is laid on (D11's 0.125 m variant).
    pub grid: ArenaGrid,
    /// Stage B's successor separation band. Stage A carries the default and ignores it.
    pub band: Band,
    /// Stage B's two patch sites, `(initial, successor)`. **Evaluator-only**: the
    /// driver's epilogue reads their stocks to report reacquisition, and nothing on the
    /// observation path ever sees them.
    patches: Option<(Site, Site)>,
    arena: Arena,
    /// The arena's settled cue field. Every episode clones it into its private simulator;
    /// field settlement belongs to fixture preparation, never to every candidate rollout.
    senses: Senses,
}

impl PreparedArena {
    /// Build the prepared Stage-A layout: one [`Arena::build`], kept immutable.
    pub fn build(founder: Founder, layout_seed: u64) -> PreparedArena {
        PreparedArena::build_stage(founder, layout_seed, Stage::A)
    }

    /// Build the prepared layout for `stage` in the landed band, kept immutable.
    pub fn build_stage(founder: Founder, layout_seed: u64, stage: Stage) -> PreparedArena {
        PreparedArena::build_stage_in(founder, layout_seed, stage, Band::Landed)
    }

    /// Build the prepared layout for `stage` with the Stage-B successor in `band`.
    pub fn build_stage_in(
        founder: Founder,
        layout_seed: u64,
        stage: Stage,
        band: Band,
    ) -> PreparedArena {
        PreparedArena::build_stage_on(founder, layout_seed, stage, band, ArenaGrid::Standard)
    }

    /// [`PreparedArena::build_stage_in`] laid on `grid`.
    pub fn build_stage_on(
        founder: Founder,
        layout_seed: u64,
        stage: Stage,
        band: Band,
        grid: ArenaGrid,
    ) -> PreparedArena {
        let (arena, patches) = match stage {
            Stage::A => (Arena::build_on(founder, layout_seed, grid), None),
            Stage::B => {
                let (arena, initial, successor) =
                    Arena::build_reacquisition_on(founder, layout_seed, band, grid).into_parts();
                (arena, Some((initial, successor)))
            }
        };
        PreparedArena {
            founder,
            layout_seed,
            stage,
            grid,
            band,
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
    ) -> PreparedArena {
        let (arena, initial, successor) =
            Arena::build_reacquisition_with_start_turn(founder, layout_seed, turn_to_initial_rad)
                .into_parts();
        PreparedArena {
            founder,
            layout_seed,
            stage: Stage::B,
            grid: ArenaGrid::Standard,
            band: Band::Landed,
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
                    let (cx, cz) = site_center_m(*s, self.grid);
                    let dx = wrapped_dx_m(pose.x, cx, self.grid);
                    let dz = cz - pose.z;
                    dx * dx + dz * dz
                };
                d(a).partial_cmp(&d(b)).expect("finite")
            })?,
        };
        let (cx, cz) = site_center_m(target, self.grid);
        let dx = wrapped_dx_m(pose.x, cx, self.grid);
        let dz = cz - pose.z;
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
        self.arena.clone()
    }

    /// A fresh copy of the settled cue field for one private episode simulator.
    pub fn episode_senses(&self) -> Senses {
        self.senses.clone()
    }

    /// A fresh arena rebuilt from the seed — the other setup path, measured beside the
    /// clone path by the bench. Both must prepare the identical arena.
    pub fn rebuilt(&self) -> Arena {
        match self.stage {
            Stage::A => Arena::build_on(self.founder, self.layout_seed, self.grid),
            Stage::B => {
                Arena::build_reacquisition_on(self.founder, self.layout_seed, self.band, self.grid)
                    .into_arena()
            }
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

impl std::fmt::Debug for PreparedArena {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedArena")
            .field("founder", &self.arena.founder.name())
            .field("stage", &self.stage.as_str())
            .field("grid", &self.grid.as_str())
            .field("band", &self.band.as_str())
            .field("layout_seed", &self.layout_seed)
            .field("animal_id", &self.arena.animal_id)
            .field("resources", &self.arena.resources.len())
            .finish()
    }
}

/// One immutable episode fixture (P5-B item 2): a frozen arena, or a frozen landscape.
#[derive(Clone, Debug)]
pub enum Prepared {
    Arena(PreparedArena),
    Landscape(Box<PreparedLandscape>),
}

impl From<PreparedArena> for Prepared {
    fn from(p: PreparedArena) -> Prepared {
        Prepared::Arena(p)
    }
}

impl From<PreparedLandscape> for Prepared {
    fn from(p: PreparedLandscape) -> Prepared {
        Prepared::Landscape(Box::new(p))
    }
}

impl Prepared {
    /// [`PreparedArena::build`], as a fixture.
    pub fn build(founder: Founder, layout_seed: u64) -> Prepared {
        PreparedArena::build(founder, layout_seed).into()
    }

    /// [`PreparedArena::build_stage`], as a fixture.
    pub fn build_stage(founder: Founder, layout_seed: u64, stage: Stage) -> Prepared {
        PreparedArena::build_stage(founder, layout_seed, stage).into()
    }

    /// [`PreparedArena::build_stage_in`], as a fixture.
    pub fn build_stage_in(
        founder: Founder,
        layout_seed: u64,
        stage: Stage,
        band: Band,
    ) -> Prepared {
        PreparedArena::build_stage_in(founder, layout_seed, stage, band).into()
    }

    /// [`PreparedArena::build_reacquisition_with_start_turn`], as a fixture.
    pub fn build_reacquisition_with_start_turn(
        founder: Founder,
        layout_seed: u64,
        turn_to_initial_rad: f64,
    ) -> Prepared {
        PreparedArena::build_reacquisition_with_start_turn(
            founder,
            layout_seed,
            turn_to_initial_rad,
        )
        .into()
    }

    /// The lineage whose body — or bodies — run the candidate.
    pub fn founder(&self) -> Founder {
        match self {
            Prepared::Arena(a) => a.founder,
            Prepared::Landscape(l) => l.founder,
        }
    }

    /// The fixture's seed: an arena's layout seed, a landscape's seed base.
    pub fn layout_seed(&self) -> u64 {
        match self {
            Prepared::Arena(a) => a.layout_seed,
            Prepared::Landscape(l) => l.seed_base,
        }
    }

    /// The horizon this fixture fixes for itself, if any: a landscape's
    /// [`super::landscape::LANDSCAPE_HORIZON_TICKS`] (D8). An arena runs the caller's.
    pub fn horizon(&self) -> Option<u64> {
        match self {
            Prepared::Arena(_) => None,
            Prepared::Landscape(l) => Some(l.horizon()),
        }
    }

    /// The arena, if this is one.
    pub fn arena(&self) -> Option<&PreparedArena> {
        match self {
            Prepared::Arena(a) => Some(a),
            Prepared::Landscape(_) => None,
        }
    }

    /// The landscape, if this is one.
    pub fn landscape(&self) -> Option<&PreparedLandscape> {
        match self {
            Prepared::Arena(_) => None,
            Prepared::Landscape(l) => Some(l),
        }
    }

    /// The immutable arena ([`PreparedArena::fixture_arena`]). **Panics on a
    /// landscape**: it is the arena fixture's validation surface, for commands and tests
    /// that built an arena.
    pub fn fixture_arena(&self) -> &Arena {
        self.arena()
            .expect("fixture_arena is an arena fixture's surface")
            .fixture_arena()
    }

    /// An arena's start geometry ([`PreparedArena::start_geometry`]); `None` on a
    /// landscape.
    pub fn start_geometry(&self) -> Option<StartGeometry> {
        self.arena().and_then(PreparedArena::start_geometry)
    }

    /// An arena's Stage-B patches; `None` on Stage A and on a landscape.
    pub fn patches(&self) -> Option<(Site, Site)> {
        self.arena().and_then(PreparedArena::patches)
    }

    /// A short label for job names and reports.
    pub fn label(&self) -> String {
        match self {
            Prepared::Arena(a) => format!(
                "{}{}",
                a.layout_seed,
                match a.grid {
                    ArenaGrid::Standard => "",
                    ArenaGrid::Fine => "f",
                }
            ),
            Prepared::Landscape(l) => l.label(),
        }
    }
}

/// The training layout seeds, as [`Prepared`] layouts of `stage`, in the frozen order.
pub fn training_layouts(founder: Founder, stage: Stage, band: Band) -> Vec<Prepared> {
    TRAINING_LAYOUT_SEEDS
        .map(|seed| Prepared::build_stage_in(founder, seed, stage, band))
        .into()
}

/// P5-C's arena half (C1): the sixteen training seeds as Stage-`stage` layouts in
/// `band`, the first eight on the standard 0.25 m grid and the last eight on the
/// 0.125 m grid (D11's variant) — sixteen distinct layouts, both grids equally.
pub fn p5_arena_layouts(founder: Founder, stage: Stage, band: Band) -> Vec<Prepared> {
    arena_half(founder, stage, band, true, TRAINING_LAYOUT_SEEDS.len())
}

/// The first `n` training arenas a run trains on, built and nothing more: P5-C's
/// ([`p5_arena_layouts`]) when `p5`, the standard grid's ([`training_layouts`])
/// otherwise. Layout `i` is the same fixture whether or not the ones after it are built,
/// which is what lets a remote worker build exactly a run's arena half.
pub fn arena_half(founder: Founder, stage: Stage, band: Band, p5: bool, n: usize) -> Vec<Prepared> {
    TRAINING_LAYOUT_SEEDS
        .iter()
        .take(n)
        .enumerate()
        .map(|(i, &seed)| {
            let grid = if p5 && i >= TRAINING_LAYOUT_SEEDS.len() / 2 {
                ArenaGrid::Fine
            } else {
                ArenaGrid::Standard
            };
            PreparedArena::build_stage_on(founder, seed, stage, band, grid).into()
        })
        .collect()
}

/// The evaluation layout seeds, as [`Prepared`] layouts of `stage`, in the frozen order.
pub fn evaluation_layouts(founder: Founder, stage: Stage, band: Band) -> Vec<Prepared> {
    EVALUATION_LAYOUT_SEEDS
        .map(|seed| Prepared::build_stage_in(founder, seed, stage, band))
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
    use cubarium_voxel_sim::{ARENA_VOXEL_M, ARENA_WIDTH};

    #[test]
    fn the_blind_stage_b_holdout_is_balanced_by_side_and_range() {
        let mut bins = [0usize; 4];
        for seed in EVALUATION_LAYOUT_SEEDS {
            let turn = PreparedArena::build_stage(Founder::Blind, seed, Stage::B)
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
            assert_eq!(prepared.founder(), Founder::Blind);
            assert_eq!(prepared.arena().expect("arena").stage, Stage::B);
            assert_eq!(prepared.layout_seed(), OFFSET_SWEEP_LAYOUT_SEED);
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

    /// Every training and held-out layout of the near rung actually satisfies its band
    /// and still starts the founder off food.
    ///
    /// Proved from the arena's own wrapped column arithmetic rather than by running
    /// anything: the successor lands 4 to 5 columns from the initial patch (1.0 m to
    /// 1.49 m, inside the blind founder's 1.5 m cue reach and the browser's 2.0 m cone),
    /// the start is a distinct site 2 to 4 columns from the initial patch, and it is at
    /// least 2 columns from **both** patches, so the body is never introduced in feeding
    /// contact with the patch it is meant to have to find.
    #[test]
    fn every_near_layout_sits_in_its_band_and_starts_off_food() {
        let columns = |site: Site, other: Site| -> i64 {
            let width = i64::from(ARENA_WIDTH);
            let raw = i64::from(other.x) - i64::from(site.x);
            let dx = [raw, raw - width, raw + width]
                .into_iter()
                .min_by_key(|d| d.abs())
                .expect("three wrapped displacements");
            let dz = i64::from(other.z) - i64::from(site.z);
            dx * dx + dz * dz
        };
        for seed in TRAINING_LAYOUT_SEEDS
            .into_iter()
            .chain(EVALUATION_LAYOUT_SEEDS)
        {
            for founder in Founder::ALL {
                let p = PreparedArena::build_stage_in(founder, seed, Stage::B, Band::Near);
                let (initial, successor) = p.patches().expect("Stage B names its patches");
                let separation = columns(initial, successor);
                assert!(
                    Band::Near.contains(separation),
                    "{founder:?} seed {seed}: successor {separation} squared columns off"
                );
                let metres = site_separation_m(initial, successor, ArenaGrid::Standard);
                assert!(
                    (1.0..1.5).contains(&metres),
                    "{founder:?} seed {seed}: {metres} m apart"
                );
                assert!(metres <= sensed_radius_m(founder));

                let arena = p.fixture_arena();
                let pose = arena
                    .animal_pose()
                    .expect("the near layout places a founder");
                let start = Site {
                    x: (pose.x / ARENA_VOXEL_M).floor() as u32,
                    y: initial.y,
                    z: (pose.z / ARENA_VOXEL_M).floor() as u32,
                };
                assert!(
                    !arena.resources.contains(&start),
                    "{founder:?} seed {seed}: the founder was started on a patch"
                );
                assert!(
                    (4..=16).contains(&columns(start, initial)),
                    "{founder:?} seed {seed}: start {} off the initial patch",
                    columns(start, initial)
                );
                for patch in [initial, successor] {
                    assert!(
                        columns(start, patch) >= 4,
                        "{founder:?} seed {seed}: start is {} squared columns from {patch:?}",
                        columns(start, patch)
                    );
                }
                assert!(
                    arena.patch_stock(initial) > 0.0 && arena.patch_stock(successor) > 0.0,
                    "{founder:?} seed {seed}: both near patches are stocked"
                );
            }
        }
    }

    /// P5-B's layouts on the frozen seeds: every Stage-A layout lays each kind twice at
    /// the arena's stock, and every Stage-B pair is two kinds the band reaches, the
    /// initial one at half the successor. A stripped crown is Stage A's only.
    #[test]
    fn every_frozen_layout_lays_its_kinds_at_the_arenas_stock() {
        use cubarium_voxel_sim::{BROWSER_FOLIAGE_PER_STAND, PatchKind};

        for founder in Founder::ALL {
            for seed in TRAINING_LAYOUT_SEEDS
                .into_iter()
                .chain(EVALUATION_LAYOUT_SEEDS)
            {
                let a = PreparedArena::build_stage(founder, seed, Stage::A);
                let arena = a.fixture_arena();
                let mut kinds = arena.resource_kinds.clone();
                kinds.sort_by_key(|k| k.as_str());
                kinds.dedup();
                assert_eq!(
                    kinds.len(),
                    3,
                    "{founder:?} seed {seed}: every kind in Stage A"
                );
                for (&site, &kind) in arena.resources.iter().zip(&arena.resource_kinds) {
                    let want = match (founder, kind) {
                        (_, PatchKind::Stripped) => 0.0,
                        (Founder::Browser, _) => BROWSER_FOLIAGE_PER_STAND,
                        (Founder::Blind, _) => 0.2,
                    };
                    assert!(
                        (arena.patch_stock(site) - want).abs() < 1e-12,
                        "{founder:?} seed {seed}: {kind:?} at {site:?} holds {}",
                        arena.patch_stock(site)
                    );
                }
                let b = PreparedArena::build_stage_in(founder, seed, Stage::B, Band::Landed);
                let arena = b.fixture_arena();
                let (initial, successor) = b.patches().expect("Stage B names its patches");
                assert!(
                    !arena.resource_kinds.contains(&PatchKind::Stripped),
                    "{founder:?} seed {seed}: a scored patch nobody can eat"
                );
                let (i, s) = (arena.patch_stock(initial), arena.patch_stock(successor));
                assert!(
                    s > 0.0 && (i / s - 0.5).abs() < 1e-12,
                    "{founder:?} seed {seed}"
                );
            }
        }
    }

    /// The landed band is untouched by the near rung: every landed layout already keeps
    /// the start two columns clear of both patches, which is why the near band's extra
    /// start filter is scoped to `Near` and the landed candidate pool — and so every
    /// landed draw — is exactly what P3-A built.
    #[test]
    fn the_landed_band_already_satisfies_the_near_rungs_start_rule() {
        for seed in TRAINING_LAYOUT_SEEDS
            .into_iter()
            .chain(EVALUATION_LAYOUT_SEEDS)
        {
            for founder in Founder::ALL {
                let p = PreparedArena::build_stage_in(founder, seed, Stage::B, Band::Landed);
                let (initial, successor) = p.patches().expect("patches");
                assert!(site_separation_m(initial, successor, ArenaGrid::Standard) >= 2.0);
                let pose = p.fixture_arena().animal_pose().expect("founder");
                for patch in [initial, successor] {
                    let d = distance_to_site_m(pose.x, pose.z, patch, ArenaGrid::Standard);
                    assert!(
                        d >= 0.5,
                        "{founder:?} seed {seed}: start {d} m from {patch:?}"
                    );
                }
            }
        }
    }

    /// The two bands are different tasks and say so: different protocol strings, and
    /// disjoint squared-column ranges with no value satisfying both.
    #[test]
    fn the_two_bands_are_disjoint_and_separately_named() {
        assert_ne!(
            stage_b_arena_protocol(Band::Near),
            stage_b_arena_protocol(Band::Landed)
        );
        assert_eq!(
            arena_protocol(Founder::Blind, Stage::A, Band::Near),
            "p2b-stage-a-1+p5-arena-1"
        );
        assert_eq!(
            arena_protocol(Founder::Blind, Stage::A, Band::Landed),
            "p2b-stage-a-1+p5-arena-1"
        );
        // The browser's crown-height revision rides on every one of its own strings and
        // on none of the blind founder's.
        for stage in [Stage::A, Stage::B] {
            for band in [Band::Near, Band::Landed] {
                let blind = arena_protocol(Founder::Blind, stage, band);
                let browser = arena_protocol(Founder::Browser, stage, band);
                assert!(!blind.contains(BROWSER_CROWN_HEIGHT_REVISION));
                assert_eq!(browser, format!("{blind}+{BROWSER_CROWN_HEIGHT_REVISION}"));
            }
        }
        for d2 in 0..200i64 {
            assert!(
                !(Band::Near.contains(d2) && Band::Landed.contains(d2)),
                "{d2}"
            );
        }
        assert!(Band::Near.contains(16) && Band::Near.contains(35));
        assert!(!Band::Near.contains(15) && !Band::Near.contains(36));
        assert!(Band::Landed.contains(64) && !Band::Landed.contains(63));
        assert_eq!(parse_band("Near").expect("parsed"), Band::Near);
        assert_eq!(parse_band(" landed ").expect("parsed"), Band::Landed);
        assert!(parse_band("middling").is_err());
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
                    let p = PreparedArena::build_stage(founder, seed, stage);
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

    /// The two sensed radii are the measurements they claim to be: the browser's is its
    /// manifest's own ray-fan reach, not a copy of it, and the blind founder's is the
    /// last distance at which integration note 4's field measurement is nonzero.
    #[test]
    fn the_sensed_radii_come_from_the_manifest_and_the_field_measurement() {
        assert_eq!(
            sensed_radius_m(Founder::Browser),
            Founder::Browser.manifest().cone_range_m
        );
        assert_eq!(sensed_radius_m(Founder::Browser), 2.0);
        assert_eq!(sensed_radius_m(Founder::Blind), BLIND_CUE_REACH_M);
        assert_eq!(BLIND_CUE_REACH_M, 1.5);
        // The blind manifest has no cone at all, so reading it would say 0 m.
        assert_eq!(Founder::Blind.manifest().cone_range_m, 0.0);
    }

    /// The fixture's wrapped metre arithmetic: a straight run along z, a wrap across the
    /// seam that takes the short way round, and the symmetry of a separation.
    #[test]
    fn wrapped_site_distances_take_the_short_way_round() {
        let at = |x, z| Site { x, y: 4, z };
        // Four columns of z apart, 0.25 m each.
        assert!((site_separation_m(at(5, 2), at(5, 6), ArenaGrid::Standard) - 1.0).abs() < 1e-12);
        // Two columns apart across the seam of a 32-wide strip, not thirty.
        let seam = site_separation_m(at(31, 3), at(1, 3), ArenaGrid::Standard);
        assert!((seam - 0.5).abs() < 1e-12, "{seam}");
        assert_eq!(
            seam,
            site_separation_m(at(1, 3), at(31, 3), ArenaGrid::Standard)
        );
        // A point reading agrees with the site-to-site reading it is built from.
        let (cx, cz) = site_center_m(at(31, 3), ArenaGrid::Standard);
        assert_eq!(
            distance_to_site_m(cx, cz, at(1, 3), ArenaGrid::Standard),
            seam
        );
        assert_eq!(
            distance_to_site_m(cx, cz, at(31, 3), ArenaGrid::Standard),
            0.0
        );
    }

    #[test]
    fn worker_limit_reserves_ten_percent_then_stops_at_the_cap() {
        assert_eq!(worker_limit_for(1), 1);
        assert_eq!(worker_limit_for(2), 1);
        assert_eq!(worker_limit_for(10), 9);
        assert_eq!(worker_limit_for(32).min(MAX_EPISODE_WORKERS), 28);
        assert_eq!(worker_limit_for(128).min(MAX_EPISODE_WORKERS), 32);
    }

    /// The reserve is the machine's, the cap the process's own CPUs (cache study B).
    #[test]
    fn the_reserve_is_the_machines_and_the_affinity_mask_caps_it() {
        assert_eq!(
            worker_limit_within(32, 16),
            16,
            "`taskset -c 0-7,16-23` on the 32-thread desktop: the other chiplet is the reserve"
        );
        assert_eq!(worker_limit_within(32, 32).min(MAX_EPISODE_WORKERS), 28);
        assert_eq!(worker_limit_within(32, 4), 4);
        assert_eq!(
            worker_limit_within(12, 12),
            10,
            "eidolon, all of it: still reserves"
        );
        assert_eq!(worker_limit_within(16, 16), 14);
        assert_eq!(
            worker_limit_within(8, 16),
            14,
            "a mask wider than a stale count"
        );
        assert_eq!(worker_limit_within(1, 1), 1);
    }

    /// The two setup paths prepare the identical arena: cloning the immutable layout and
    /// rebuilding from the seed give the same placed animal, the same stock and the same
    /// world material.
    #[test]
    fn clone_and_rebuild_prepare_identical_arenas() {
        for founder in Founder::ALL {
            let p = PreparedArena::build(founder, TRAINING_LAYOUT_SEEDS[2]);
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
                    let p = PreparedArena::build_stage(founder, seed, stage);
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
        let p = PreparedArena::build(Founder::Blind, TRAINING_LAYOUT_SEEDS[0]);
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
