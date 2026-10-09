//! **Terrain first, then founding** (`design/handoffs/hybrid-renderer-2026-10-07.md`,
//! "Terrain-first new-world picking"): a picker's candidates are bare terrains, drawn in
//! seconds and checked for room for the founders ([`terrain_candidate`]); only the picked
//! one is founded ([`found_picked`]), on its own terrain, with its progress told through
//! [`FoundingInfo`] and stoppable at every tick.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cubarium_voxel_sim::found::{self, LAKE_SEED_TRIES, LayerConfig, NoTerrain, TerrainFounding, TerrainRejection};
use cubarium_voxel_sim::habitat::{self, FoundingPhase, FoundingWatch};
use serde::{Deserialize, Serialize};

use crate::create::{self, Populate, StartLayers};
use crate::founders::founder_recipes;
use crate::params::{SimParams, WorldSpec};
use crate::session::Prepared;

/// Terrain draws a candidate makes before it gives up (the lake gate's own bound).
pub const TERRAIN_TRIES: usize = LAKE_SEED_TRIES;

/// What a picked candidate's founding is doing, read without waiting on it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FoundingInfo {
    /// `idle`, `founding`, `done`, `rejected`, `failed` or `cancelled`.
    pub state: String,
    /// The candidate being (or last) founded.
    pub candidate: Option<usize>,
    /// Its terrain seed, and its terrain's hash ([`cubarium_voxel::World::terrain_hash`],
    /// hex): the same before and after founding.
    pub seed: Option<u64>,
    pub terrain: Option<String>,
    /// The stage now: `settling`, `shower`, `draining`, `lake_rest`, `stream_watch`,
    /// `placing`, `starting` (drivers installed), or "" before the first.
    pub phase: String,
    /// What the stage is, in words for a progress label.
    pub phase_label: String,
    /// 0–1 through the current placement try ([`progress_of`]).
    pub progress: f64,
    /// Placement try, 1-based, of `attempts`, and the seed its weather and draws run on.
    pub attempt: usize,
    pub attempts: usize,
    pub placement_seed: Option<u64>,
    /// Seconds since the founding started (to the end, once it has ended).
    pub elapsed_s: f64,
    /// Why it failed, in plain words.
    pub error: Option<String>,
    /// The failed terrain check, with its plant or lineage where applicable. A rejected
    /// terrain stays selected; the app can read `state` and this object for feedback.
    pub terrain_rejection: Option<TerrainRejection>,
    /// The founded world's label, once done.
    pub label: Option<String>,
}

/// The words a progress label shows for a stage.
pub fn phase_label(phase: &str) -> &'static str {
    match phase {
        "settling" => "Settling the water",
        "shower" => "The opening shower",
        "draining" => "Draining",
        "lake_rest" => "The lake coming to rest",
        "stream_watch" => "Watching the stream",
        "placing" => "Placing the founders",
        "starting" => "Starting",
        _ => "",
    }
}

/// **The progress bar's position** within one placement try: each stage owns a span of
/// the bar in proportion to the wall time it took founding the wide desktop terrarium
/// (2026-10-08 measurement), and moves through it by its ticks against the ticks it
/// typically takes — `typical`, never past 95 % of its span until the stage ends, so a
/// stage that runs long slows down instead of overrunning the next. Stages end early
/// when their water settles, so the bar jumps forward then; it never moves back within a
/// try.
pub fn progress_of(phase: FoundingPhase, ticks: u32, shower_typical: u32) -> f64 {
    let (start, span, typical) = match phase {
        FoundingPhase::Settling => (0.0, 0.06, FoundingPhase::Settling.cap_ticks()),
        FoundingPhase::Shower => (0.06, 0.30, shower_typical.max(1)),
        FoundingPhase::Draining => (0.36, 0.16, FoundingPhase::Draining.cap_ticks()),
        FoundingPhase::LakeRest => (0.52, 0.34, 4 * 1200),
        FoundingPhase::StreamWatch => (0.86, 0.04, FoundingPhase::StreamWatch.cap_ticks()),
        FoundingPhase::Placing => (0.90, 0.10, 1),
    };
    let f = (f64::from(ticks) / f64::from(typical.max(1))).min(0.95);
    start + span * f
}

/// Counts a session's worker threads (candidates and foundings) while they run.
pub(crate) struct Busy(Arc<AtomicUsize>);

impl Busy {
    pub(crate) fn start(count: &Arc<AtomicUsize>) -> Busy {
        count.fetch_add(1, Ordering::SeqCst);
        Busy(count.clone())
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// What a terrain candidate's draw said, beside the world itself.
pub(crate) struct TerrainNote {
    pub(crate) seed: u64,
    /// What the feasibility check said; `None` for a bare spec, which founds no one.
    pub(crate) feasibility: Option<habitat::Acceptance>,
    pub(crate) warnings: Vec<String>,
}

/// **One terrain candidate**: `spec`'s world drawn ([`found::draw_terrain`]), with the
/// feasibility check unless the spec is bare, and returned bare — empty layers, so a view
/// and a renderer read it as they read any world. `go_on` stops the draws.
pub(crate) fn terrain_candidate(
    params: &SimParams,
    spec: &WorldSpec,
    tries: usize,
    go_on: &dyn Fn() -> bool,
) -> Result<(Prepared, TerrainNote), String> {
    let p = params.resolve_sets(&spec.sets).map_err(|e| format!("{e:#}"))?;
    let layers = LayerConfig::new(&p, p.world.voxel_m);
    let counts = p.fauna.founder_counts();
    let check = (!spec.empty).then_some((&layers, counts));
    let found::Terrain { world, seed, feasibility, warnings, .. } =
        found::draw_terrain(&p.world, spec.seed, tries, create::random_seed, check, go_on).map_err(|e| match e {
            NoTerrain::Cancelled => "cancelled".to_string(),
            NoTerrain::Refused(why) => why,
        })?;
    let (flora, fauna) = layers.layers();
    let label = format!("terrain (seed {seed})");
    Ok((Prepared { world, flora, fauna, label, exec: params.clone() }, TerrainNote { seed, feasibility, warnings }))
}

/// A founding's shared state: what readers see, and the stop flag.
pub(crate) struct Job {
    pub(crate) info: Mutex<FoundingInfo>,
    pub(crate) cancel: AtomicBool,
    pub(crate) started: Instant,
    /// The shower's typical length in ticks: the climate's guaranteed minimum, twice.
    pub(crate) shower_typical: u32,
    /// Test only: the acceptance verdict in place of the habitat's own.
    pub(crate) accept: Option<fn(&habitat::Seeded) -> bool>,
}

impl Job {
    pub(crate) fn update(&self, f: impl FnOnce(&mut FoundingInfo)) {
        if let Ok(mut info) = self.info.lock() {
            f(&mut info);
            info.elapsed_s = self.started.elapsed().as_secs_f64();
        }
    }
}

impl FoundingWatch for Job {
    fn attempt(&self, k: usize, of: usize, placement_seed: u64) {
        self.update(|i| {
            i.attempt = k + 1;
            i.attempts = of;
            i.placement_seed = Some(placement_seed);
            i.progress = 0.0;
        });
    }

    fn progress(&self, phase: FoundingPhase, ticks: u32) {
        let at = progress_of(phase, ticks, self.shower_typical);
        self.update(|i| {
            if i.phase != phase.name() {
                i.phase = phase.name().to_string();
                i.phase_label = phase_label(phase.name()).to_string();
            }
            i.progress = i.progress.max(at);
        });
    }

    fn go_on(&self) -> bool {
        !self.cancel.load(Ordering::Relaxed)
    }
}

/// How a founding ended.
pub(crate) enum Outcome {
    Founded(Box<Prepared>),
    Rejected(TerrainRejection),
    Refused(String),
    Cancelled,
    Failed(String),
}

/// **Found a picked terrain**, keeping it: [`found::found_terrain`] with `tries` placement
/// tries, then the lineages' drivers installed ([`create::populate`]). A bare spec founds
/// no one and only installs the drivers, as a bare world always has.
pub(crate) fn found_picked(
    world: cubarium_voxel::World,
    seed: u64,
    params: &SimParams,
    spec: &WorldSpec,
    tries: usize,
    job: &Job,
) -> Outcome {
    let run = || -> anyhow::Result<Outcome> {
        let p = params.resolve_sets(&spec.sets)?;
        let recipes = founder_recipes(&[], &[])?;
        let founder_counts = p.fauna.founder_counts();
        let layers = LayerConfig::new(&p, p.world.voxel_m);
        let (world, label, founded) = if spec.empty {
            (world, format!("generated (seed {seed})"), None)
        } else {
            let made = match job.accept {
                Some(accept) => found::found_terrain_judged(world, seed, &layers, founder_counts, tries, job, accept),
                None => found::found_terrain(world, seed, &layers, founder_counts, tries, job),
            };
            match made {
                TerrainFounding::Cancelled => return Ok(Outcome::Cancelled),
                TerrainFounding::Rejected(why) => return Ok(Outcome::Rejected(why)),
                TerrainFounding::Refused(why) => {
                    let last = why.last().cloned().unwrap_or_default();
                    return Ok(Outcome::Refused(format!(
                        "The founders couldn't all be placed on this terrain ({} tries; the last: {last}). \
                         Pick another terrain, or reroll.",
                        why.len()
                    )));
                }
                TerrainFounding::Founded(f) => {
                    let label = if f.refused.is_empty() {
                        format!("generated (seed {seed})")
                    } else {
                        format!(
                            "generated (seed {seed}, placement seed {} after {} refused)",
                            f.placement_seed,
                            f.refused.len()
                        )
                    };
                    let f = *f;
                    (f.world, label, Some(StartLayers::Founded((f.flora, f.fauna, f.seeded))))
                }
            }
        };
        if !job.go_on() {
            return Ok(Outcome::Cancelled);
        }
        job.update(|i| {
            i.phase = "starting".into();
            i.phase_label = phase_label("starting").into();
        });
        let (world, flora, fauna) = create::populate(
            world,
            false,
            founded,
            &p,
            &Populate {
                recipes: &recipes,
                introductions: &[],
                empty: spec.empty,
                founder_counts,
                no_egg_bank: false,
            },
        )?;
        Ok(Outcome::Founded(Box::new(Prepared { world, flora, fauna, label, exec: params.clone() })))
    };
    run().unwrap_or_else(|e| Outcome::Failed(format!("{e:#}")))
}

/// The shower's typical length for `params`' world, in ticks.
pub(crate) fn shower_typical(params: &SimParams) -> u32 {
    (params.world.climate.opening_min_s * 2.0 * f64::from(cubarium_voxel::TICK_HZ)).round().max(1.0) as u32
}
