//! Workstream S: a **fixed-age preconditioned opening**, measured and compared with the
//! status quo.
//!
//! Workstream M measured that today's §11 seeding puts 9–15 % of watched cells above what the
//! plant step alone sustains, and that the depletion crossings which follow are opening-stock
//! declines with zero consumer withdrawal. Its option A — run the plants alone for a while,
//! then found the animals — is what this module turns into a declared, deterministic,
//! seed- and config-bound procedure, and then measures against leaving §11 alone.
//!
//! Two stages, one command:
//!
//! - [`run_field`] — the **preconditioning operator**. For each (candidate, seed) it builds
//!   the ordinary world with an empty roster, advances the ordinary §4 dynamics with nothing
//!   eating (`evaluate::precondition`, the same function a comparison arm uses), saves the
//!   whole-field state at every declared age, and reports at 6,000-tick resolution how much
//!   the field is still moving: moving-window changes in total and per-cell `P`, `W`, `Q` and
//!   `N`, the exact plant income and loss over the window, the depletion crossings, and the
//!   spatial spread of the foliage. That is what makes "how settled is the field at age T" a
//!   measurement rather than an assumption.
//! - [`run_compare`] — the **comparison**. At each declared age the ordinary roster is founded
//!   through [`cubarium_core::World::found_roster`] and the world is run the ordinary 180,000
//!   ticks with the ledger, the plant record and the depletion split on. Age 0 is the status
//!   quo: the door founds at tick 0 into a world built with an empty roster, which the core's
//!   own test proves is the constructor's world bit for bit, so those rows must reproduce
//!   workstream M's retained present-arm `final_state_hash`es — and that reproduction is the
//!   campaign's own check on the door.
//!
//! **Nothing here changes §11, `producer.initial_fraction`, or any equation.** A preconditioned
//! opening is a *procedure applied before founding*, not a new seeding rule, and this module
//! measures it so the owner can accept or refuse it.
//!
//! The age is called an age, never an equilibrium: `P` at 180,000 plant-only ticks is the
//! state the plant step has reached by then, and the readings below are exactly the evidence
//! for how close to standing still that is.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_core::organism::DeathCause;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{LifeEvent, OrganismId, World};
use serde::{Deserialize, Serialize};

use crate::calibrate::{Candidate, SeedSet, candidate};
use crate::evaluate::{
    BUILD_ID, DEPLETION_FRACTION, Evaluation, OPENING_TRAJECTORY_TICKS, OpeningPoint,
    PRECONDITION_CHECK_EVERY, PROBE_EVERY, RECOVERY_FRACTION, RunOptions, WINDOWS, evaluate_with,
    precondition,
};
use crate::metrics::{FORMS_POSSIBLE, guild_of};
use crate::movement::{CensusKey, CrossingCounter, CrossingKind, Crossings, FounderBroods};
use crate::plant_budget::{CellSample, PlantBudgetTracker};

/// The ages the comparison declares, in ticks. `0` is the status quo; 48,000 is just below
/// workstream M's *earliest* plant-only crossing (tick 42,520–54,980, so 48,000 sits inside
/// the band where the first cells are only beginning to go); 96,000 is about two thirds of the
/// way to M's median crossing (141,220 / 147,060); 180,000 is the whole horizon every result
/// so far was measured over, and therefore the longest age that can be compared without
/// inventing a new one.
pub const DEFAULT_AGES: [u64; 4] = [0, 48_000, 96_000, 180_000];

/// The moving window every settling measure is taken over: the same 6,000 ticks workstream M's
/// crossing budgets use, so "still moving" and "was moving before it crossed" are the same
/// interval.
pub const WINDOW_TICKS: u64 = 6_000;

/// Boundary spacing of the ring the window is differenced across (the same 600 ticks M's
/// tracker keeps), and the spacing of the readings.
pub const BOUNDARY_EVERY: u64 = 600;

/// How often a reading is emitted: one per window length, so consecutive readings tile the
/// run rather than overlapping it.
pub const READING_EVERY: u64 = 6_000;

/// A cell is counted as "still moving" in a window when its own stock changed by more than
/// this fraction of what it currently holds. One per cent over ten simulated minutes.
pub const MOVING_FRACTION: f64 = 0.01;

// ---------------------------------------------------------------------------------------
// the readings
// ---------------------------------------------------------------------------------------

/// One stock's settling over the moving window: how much the whole field moved, and how much
/// the cells moved individually (a field can be dead still in total while its cells trade
/// stock with each other, and that is a different kind of settled).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StockReading {
    /// `Σ x` over every cell now.
    pub total: f64,
    /// `Σ x` now minus `Σ x` one window ago.
    pub delta_total: f64,
    /// `delta_total / total`, the whole field's fractional drift per window.
    pub relative_total: f64,
    /// `Σ |Δx_i| / Σ x_i`: the per-cell movement the total can cancel out.
    pub cell_change_rate: f64,
    /// Cells whose own stock moved by more than [`MOVING_FRACTION`] of what they hold.
    pub cells_moving: u32,
}

/// The field at one tick of a plant-only run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldReading {
    pub tick: u64,
    /// The interval every window measure below was taken over: [`WINDOW_TICKS`] once the run
    /// is that old, the run's whole length before then, and `0` on the opening reading, which
    /// has no window behind it at all. Reported rather than assumed, so a shortened window is
    /// never read as a full one.
    pub window_ticks: u64,
    pub foliage: StockReading,
    pub wood: StockReading,
    pub plant_reserve: StockReading,
    pub nutrient: StockReading,
    /// Exact §4 flows over the window, summed over every cell: the gross income the stands
    /// drew from `N`, what they put into foliage, what they took back out, and what mouths
    /// withdrew — which is zero in a plant-only world, and is reported so that it is *seen*
    /// to be zero rather than assumed.
    pub income: f64,
    pub foliage_in: f64,
    pub foliage_out: f64,
    pub withdrawal: f64,
    /// Depletion crossings of the §11 opening reference, by the same counter every other
    /// workstream reports: cumulative, and inside this window.
    pub crossings_total: u64,
    pub crossings_in_window: u64,
    pub recoveries_total: u64,
    pub depleted_now: u32,
    /// The spatial spread of the foliage over the watched cells — the opening distribution a
    /// founding at this age would meet.
    pub foliage_median: f64,
    pub foliage_p10: f64,
    pub foliage_p90: f64,
    pub foliage_cv: f64,
    /// Cells still holding living wood above `plant.alive_min`.
    pub alive_cells: u32,
}

/// One declared age of one plant-only run: the state saved there, and the world a founding
/// there would open on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgeState {
    pub age: u64,
    /// `state_hash` of the plant-only world at this age.
    pub plant_state_hash: u64,
    /// `state_hash` the instant after the roster is founded into it. This is the opening the
    /// comparison arm of the same age must report in `founding_state_hash`.
    pub founding_state_hash: u64,
    /// The saved whole-field state, relative to the stage directory.
    pub snapshot: String,
    pub snapshot_bytes: u64,
    /// `state_hash` of the state decoded back off disk: the saved file is checked to be the
    /// state it claims to be, not assumed to be.
    pub snapshot_state_hash: u64,
}

/// One plant-only run of the operator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldRun {
    pub candidate: String,
    pub seed: u64,
    pub build_id: String,
    pub ticks: u64,
    pub watched_cells: u32,
    /// The core plant record's own closing identity residual at the horizon. A run whose
    /// accounting did not close is not a measurement.
    pub max_identity_residual: f64,
    pub elapsed_ms: u64,
    pub readings: Vec<FieldReading>,
    pub ages: Vec<AgeState>,
}

/// One arm of the comparison: a founding at `age` and the ordinary horizon after it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompareRow {
    pub candidate: String,
    pub seed: u64,
    pub age: u64,
    pub build_id: String,
    pub evaluation: Evaluation,
}

/// What a whole stage ran under.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StagePlan {
    pub stage: String,
    pub build_id: String,
    pub candidates: Vec<String>,
    pub seed_set: String,
    pub seeds: Vec<u64>,
    pub ages: Vec<u64>,
    pub horizon_ticks: u64,
    pub sample_every: u64,
    pub trials: usize,
    pub workers: usize,
    pub wall_seconds_cap: u64,
    /// The pursuit stopping rule every arm of this stage ran under, by name. Absent on S's
    /// retained plan (`runs/ecology-v1-precondition`), which ran the half-space; Z's retained
    /// plan (`runs/ecology-v1-grazed-opening`) was written without the field under the reach
    /// envelope and carries it explicitly since integration, so no artifact is re-read through
    /// a default it did not run (Astra, round-5 review P3).
    #[serde(default = "half_space_name")]
    pub pursuit_stop: String,
    /// The motor contract every arm ran under, by name. Absent means `sweep`, which every
    /// retained precondition artifact ran.
    #[serde(default = "sweep_name")]
    pub motor: String,
}

fn half_space_name() -> String {
    cubarium_core::hunter::PursuitStop::ForwardHalfSpace
        .as_str()
        .to_string()
}

fn sweep_name() -> String {
    cubarium_core::MotorModel::Sweep.name().to_string()
}

/// A completed stage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StageReport {
    pub plan: StagePlan,
    pub wall_seconds: f64,
    pub completed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub simulated_ticks: u64,
    pub ticks_per_second: f64,
    pub peak_rss_mib: f64,
}

// ---------------------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------------------

fn peak_rss_mib() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| {
                l.split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse::<f64>().ok())
            })
        })
        .map_or(0.0, |kib| kib / 1024.0)
}

/// `p` at the given quantile of an already-sorted slice, by nearest rank.
fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1;
    sorted[i]
}

/// Total, median, p10, p90 and coefficient of variation of a set of values.
fn spread(values: &[f64]) -> (f64, f64, f64, f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0, 0.0, 0.0, 0.0);
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite foliage"));
    let total: f64 = sorted.iter().sum();
    let n = sorted.len() as f64;
    let mean = total / n;
    let var = sorted.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
    let cv = if mean.abs() > 1e-12 {
        var.sqrt() / mean
    } else {
        0.0
    };
    (
        total,
        quantile(&sorted, 0.5),
        quantile(&sorted, 0.1),
        quantile(&sorted, 0.9),
        cv,
    )
}

/// One stock's settling between two whole-field vectors.
fn stock(now: &[f64], then: Option<&Vec<f64>>) -> StockReading {
    let total: f64 = now.iter().sum();
    let Some(then) = then else {
        return StockReading {
            total,
            ..StockReading::default()
        };
    };
    let before: f64 = then.iter().sum();
    let moved: f64 = now.iter().zip(then).map(|(a, b)| (a - b).abs()).sum();
    let cells_moving = now
        .iter()
        .zip(then)
        .filter(|(a, b)| (**a - **b).abs() > MOVING_FRACTION * a.abs().max(1e-12))
        .count() as u32;
    StockReading {
        total,
        delta_total: total - before,
        relative_total: if total.abs() > 1e-12 {
            (total - before) / total
        } else {
            0.0
        },
        cell_change_rate: if total.abs() > 1e-12 {
            moved / total
        } else {
            0.0
        },
        cells_moving,
    }
}

/// One 600-tick boundary of the plant-only run, kept in a ring so a reading can be differenced
/// against the state exactly one window earlier.
#[derive(Clone, Debug)]
struct Boundary {
    tick: u64,
    p: Vec<f64>,
    w: Vec<f64>,
    q: Vec<f64>,
    n: Vec<f64>,
    cells: Vec<CellSample>,
}

fn boundary_of(world: &World) -> Boundary {
    let cells = world.plant_budget().map_or_else(Vec::new, |rec| {
        rec.cells
            .iter()
            .map(|c| CellSample {
                foliage_in: c.foliage_in(),
                foliage_out: c.foliage_out(),
                withdrawal: c.withdrawal_foliage,
                income: c.income,
                maintenance_unpaid: c.maintenance_unpaid,
                death_foliage: c.death_foliage,
                wood_sum: c.w_sum,
                light_sum: c.light_effective_sum,
                nutrient_sum: c.nutrient_sum,
                ticks_alive: c.ticks_alive,
            })
            .collect()
    });
    Boundary {
        tick: world.tick(),
        p: world.state.fields.p.clone(),
        w: world.state.ecology.wood.clone(),
        q: world.state.ecology.plant_reserve.clone(),
        n: world.state.fields.n.clone(),
        cells,
    }
}

fn sum_cells(cells: &[CellSample], of: fn(&CellSample) -> f64) -> f64 {
    cells.iter().map(of).sum()
}

/// The ordinary config of a candidate at a seed, with the founder roster emptied so the
/// constructor places nobody — and the roster it emptied, to write back before founding.
fn plant_only_config(
    c: &Candidate,
    seed: u64,
) -> Result<
    (
        cubarium_core::WorldConfig,
        cubarium_core::config::FounderConfig,
    ),
    String,
> {
    let mut config = c.config(seed)?;
    let roster = config.founders.clone();
    config.founders.kinds.clear();
    config.founders.count = 0;
    config
        .validate()
        .map_err(|e| format!("config rejected: {e}"))?;
    Ok((config, roster))
}

// ---------------------------------------------------------------------------------------
// stage 1: the operator
// ---------------------------------------------------------------------------------------

/// One plant-only run to the last declared age, with a reading every [`READING_EVERY`] ticks
/// and the whole field saved at every declared age.
pub fn field_run(c: &Candidate, seed: u64, ages: &[u64], out: &Path) -> Result<FieldRun, String> {
    let start = Instant::now();
    let (config, roster) = plant_only_config(c, seed)?;
    let mut world = World::new(config).map_err(|e| format!("world creation refused: {e}"))?;
    // Opened before the first step, so `P₀` is the world's own §11 seeding and the closing
    // identity spans the whole run — the same point in the run `evaluate::run` opens it at.
    world.record_plant_budgets(true);

    let p_ref: Vec<f64> = world.state.fields.p.clone();
    let alive_min = world.config().plant.alive_min;
    let mut crossings = CrossingCounter::new(&p_ref, DEPLETION_FRACTION, RECOVERY_FRACTION);
    let watched_cells = crossings.watched();
    let watched: Vec<usize> = (0..p_ref.len()).filter(|i| p_ref[*i] > 1e-9).collect();

    let slots = (WINDOW_TICKS / BOUNDARY_EVERY) as usize + 1;
    let mut ring: Vec<Boundary> = Vec::with_capacity(slots);
    let mut readings: Vec<FieldReading> = Vec::new();
    let mut ages_out: Vec<AgeState> = Vec::new();
    let mut last_crossings = 0u64;

    let horizon = ages.iter().copied().max().unwrap_or(0);
    let reading =
        |world: &World, ring: &[Boundary], crossings: &CrossingCounter, last: &mut u64| {
            let now = boundary_of(world);
            // The newest boundary a whole window back, or — before the run is a window old — the
            // oldest boundary there is, with the window's *actual* length reported beside it.
            // Workstream M's tracker reports a shortened window the same way rather than
            // pretending the interval was the nominal one.
            let then = ring
                .iter()
                .filter(|b| b.tick < now.tick)
                .filter(|b| b.tick + WINDOW_TICKS >= now.tick)
                .min_by_key(|b| b.tick)
                .or_else(|| {
                    ring.iter()
                        .filter(|b| b.tick < now.tick)
                        .max_by_key(|b| b.tick)
                });
            let window_ticks = then.map_or(0, |b| now.tick - b.tick);
            let flows = |of: fn(&CellSample) -> f64| match then {
                Some(b) => sum_cells(&now.cells, of) - sum_cells(&b.cells, of),
                None => 0.0,
            };
            let foliage: Vec<f64> = watched.iter().map(|i| now.p[*i]).collect();
            let (_, median, p10, p90, cv) = spread(&foliage);
            let total = crossings.depletions();
            let row = FieldReading {
                tick: now.tick,
                window_ticks,
                foliage: stock(&now.p, then.map(|b| &b.p)),
                wood: stock(&now.w, then.map(|b| &b.w)),
                plant_reserve: stock(&now.q, then.map(|b| &b.q)),
                nutrient: stock(&now.n, then.map(|b| &b.n)),
                income: flows(|c| c.income),
                foliage_in: flows(|c| c.foliage_in),
                foliage_out: flows(|c| c.foliage_out),
                withdrawal: flows(|c| c.withdrawal),
                crossings_total: total,
                crossings_in_window: total - *last,
                recoveries_total: crossings.recoveries(),
                depleted_now: crossings.depleted_now(),
                foliage_median: median,
                foliage_p10: p10,
                foliage_p90: p90,
                foliage_cv: cv,
                alive_cells: world
                    .state
                    .ecology
                    .wood
                    .iter()
                    .filter(|w| {
                        cubarium_core::fields::CellClass::of(**w, alive_min)
                            == cubarium_core::fields::CellClass::Alive
                    })
                    .count() as u32,
            };
            *last = total;
            row
        };

    // The opening reading, before a single tick: this is the §11 seeding itself, and the
    // opening distribution the status-quo arm founds into.
    readings.push(reading(&world, &ring, &crossings, &mut last_crossings));
    ring.push(boundary_of(&world));
    if ages.contains(&0) {
        ages_out.push(save_age(&world, &roster, 0, c, seed, out)?);
    }

    let mut tick = 0u64;
    while tick < horizon {
        // Advanced in probe-length chunks through the *same* function a comparison arm
        // preconditions with, so "the world at age T" is one object and not two.
        let step = PROBE_EVERY.min(horizon - tick);
        precondition(&mut world, step, PRECONDITION_CHECK_EVERY)?;
        tick = world.tick();
        crossings.observe(&world.state.fields.p);
        if tick % BOUNDARY_EVERY == 0 {
            if ring.len() == slots {
                ring.remove(0);
            }
            ring.push(boundary_of(&world));
        }
        if tick % READING_EVERY == 0 || ages.contains(&tick) {
            readings.push(reading(&world, &ring, &crossings, &mut last_crossings));
        }
        if ages.contains(&tick) {
            ages_out.push(save_age(&world, &roster, tick, c, seed, out)?);
        }
    }

    Ok(FieldRun {
        candidate: c.name.to_string(),
        seed,
        build_id: BUILD_ID.to_string(),
        ticks: world.tick(),
        watched_cells,
        max_identity_residual: world.plant_budget_residual(),
        elapsed_ms: start.elapsed().as_millis() as u64,
        readings,
        ages: ages_out,
    })
}

/// Save the plant-only state at `age`, and report the opening a founding there would produce.
///
/// The founding is done on a **clone** of the state and thrown away: the run itself must carry
/// on plant-only to the next age, and a world that had founded once would refuse to found
/// again. The hash it produces is the one the comparison arm of the same age must report.
fn save_age(
    world: &World,
    roster: &cubarium_core::config::FounderConfig,
    age: u64,
    c: &Candidate,
    seed: u64,
    out: &Path,
) -> Result<AgeState, String> {
    let plant_state_hash = state_hash(&world.state);

    let mut clone = World::from_state(world.state.clone())
        .map_err(|e| format!("rebuilding the state at age {age}: {e}"))?;
    clone.state.config.founders = roster.clone();
    clone
        .found_roster()
        .map_err(|e| format!("founding at age {age}: {e}"))?;
    let founding_state_hash = state_hash(&clone.state);

    let dir = out.join("states");
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let name = format!("{}-seed{seed}-age{age}.cube", c.name);
    let path = dir.join(&name);
    let bytes = cubarium_core::snapshot::encode_snapshot(&world.state, BUILD_ID);
    std::fs::write(&path, &bytes).map_err(|e| format!("writing {}: {e}", path.display()))?;

    // The saved file is checked to be the state it claims to be rather than assumed to be:
    // decoded straight back off disk and hashed.
    let read = std::fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let (_, decoded) = cubarium_core::snapshot::decode_snapshot(&read)
        .map_err(|e| format!("decoding {}: {e:?}", path.display()))?;
    let snapshot_state_hash = state_hash(&decoded);

    Ok(AgeState {
        age,
        plant_state_hash,
        founding_state_hash,
        snapshot: format!("states/{name}"),
        snapshot_bytes: bytes.len() as u64,
        snapshot_state_hash,
    })
}

/// Stage 1 over the whole matrix.
#[allow(clippy::too_many_arguments)]
pub fn run_field(
    names: &[String],
    seed_set: SeedSet,
    seed_count: usize,
    ages: &[u64],
    workers: usize,
    wall_seconds: u64,
    out: &Path,
) -> Result<StageReport, String> {
    let (chosen, seeds, ages) = plan_inputs(names, seed_set, seed_count, ages, workers)?;
    let jobs: Vec<(&'static Candidate, u64)> = chosen
        .iter()
        .flat_map(|c| seeds.iter().map(move |s| (*c, *s)))
        .collect();

    std::fs::create_dir_all(out).map_err(|e| format!("creating {}: {e}", out.display()))?;
    let rows_path = out.join("field.jsonl");
    let file = std::fs::File::create(&rows_path)
        .map_err(|e| format!("creating {}: {e}", rows_path.display()))?;
    let writer = Mutex::new(std::io::BufWriter::new(file));

    let start = Instant::now();
    let deadline = start + std::time::Duration::from_secs(wall_seconds);
    let cursor = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let failed: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let done = AtomicUsize::new(0);
    let ticks = AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= jobs.len() {
                        return;
                    }
                    if Instant::now() >= deadline {
                        skipped.fetch_add(1, Ordering::SeqCst);
                        continue;
                    }
                    let (c, seed) = jobs[i];
                    match field_run(c, seed, &ages, out) {
                        Ok(run) => {
                            ticks.fetch_add(run.ticks as usize, Ordering::SeqCst);
                            done.fetch_add(1, Ordering::SeqCst);
                            if let Ok(mut w) = writer.lock()
                                && let Ok(text) = serde_json::to_string(&run)
                            {
                                let _ = writeln!(w, "{text}");
                            }
                        }
                        Err(e) => {
                            failed
                                .lock()
                                .expect("failure list")
                                .push(format!("{} seed {seed}: {e}", c.name));
                        }
                    }
                }
            });
        }
    });

    if let Ok(mut w) = writer.lock() {
        let _ = w.flush();
    }
    let failures = failed
        .into_inner()
        .map_err(|e| format!("failure mutex: {e}"))?;
    for f in &failures {
        eprintln!("field run failed — {f}");
    }
    let wall = start.elapsed().as_secs_f64();
    let simulated = ticks.load(Ordering::SeqCst) as u64;
    let report = StageReport {
        plan: StagePlan {
            stage: "field".into(),
            build_id: BUILD_ID.to_string(),
            candidates: chosen.iter().map(|c| c.name.to_string()).collect(),
            seed_set: seed_set.label().to_string(),
            seeds,
            ages,
            horizon_ticks: 0,
            sample_every: READING_EVERY,
            trials: jobs.len(),
            workers,
            wall_seconds_cap: wall_seconds,
            pursuit_stop: cubarium_core::hunter::PursuitStop::default()
                .as_str()
                .to_string(),
            motor: sweep_name(),
        },
        wall_seconds: wall,
        completed: done.load(Ordering::SeqCst),
        skipped: skipped.load(Ordering::SeqCst),
        failed: failures.len(),
        simulated_ticks: simulated,
        ticks_per_second: if wall > 0.0 {
            simulated as f64 / wall
        } else {
            0.0
        },
        peak_rss_mib: peak_rss_mib(),
    };
    write_report(out, &report)?;
    Ok(report)
}

// ---------------------------------------------------------------------------------------
// stage 2: the comparison
// ---------------------------------------------------------------------------------------

/// Stage 2 over the whole matrix: one run per (candidate, seed, age).
#[allow(clippy::too_many_arguments)]
pub fn run_compare(
    names: &[String],
    seed_set: SeedSet,
    seed_count: usize,
    ages: &[u64],
    horizon_ticks: u64,
    sample_every: u64,
    pursuit_stop: cubarium_core::hunter::PursuitStop,
    workers: usize,
    wall_seconds: u64,
    out: &Path,
) -> Result<StageReport, String> {
    let (chosen, seeds, ages) = plan_inputs(names, seed_set, seed_count, ages, workers)?;
    struct Job {
        candidate: &'static Candidate,
        values: Vec<f64>,
        seed: u64,
        age: u64,
    }
    let mut jobs = Vec::new();
    for c in &chosen {
        let values = c.vector()?;
        for seed in &seeds {
            for age in &ages {
                jobs.push(Job {
                    candidate: c,
                    values: values.clone(),
                    seed: *seed,
                    age: *age,
                });
            }
        }
    }
    // Longest first: a 180,000-tick precondition in front of the horizon is the job that sets
    // the wall time, so it must not be the one a worker picks up last.
    jobs.sort_by_key(|j| std::cmp::Reverse(j.age));

    std::fs::create_dir_all(out).map_err(|e| format!("creating {}: {e}", out.display()))?;
    let rows_path = out.join("compare.jsonl");
    let file = std::fs::File::create(&rows_path)
        .map_err(|e| format!("creating {}: {e}", rows_path.display()))?;
    let writer = Mutex::new(std::io::BufWriter::new(file));

    let start = Instant::now();
    let deadline = start + std::time::Duration::from_secs(wall_seconds);
    let cursor = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    let ticks = AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= jobs.len() {
                        return;
                    }
                    if Instant::now() >= deadline {
                        skipped.fetch_add(1, Ordering::SeqCst);
                        continue;
                    }
                    let job = &jobs[i];
                    let protocol = crate::evaluate::Protocol {
                        horizon_ticks,
                        sample_every,
                        apex_founders: 0,
                        apex_introduce_tick: 0,
                    };
                    let options = RunOptions {
                        ledger: true,
                        plant_record: true,
                        no_animals: false,
                        precondition: Some(job.age),
                        motor: cubarium_core::MotorModel::Sweep,
                        pursuit_stop,
                    };
                    let evaluation = evaluate_with(&job.values, job.seed, protocol, options);
                    if evaluation.status == crate::evaluate::Status::Completed {
                        done.fetch_add(1, Ordering::SeqCst);
                    } else {
                        failed.fetch_add(1, Ordering::SeqCst);
                        eprintln!(
                            "compare arm {} seed {} age {} — {:?}: {}",
                            job.candidate.name,
                            job.seed,
                            job.age,
                            evaluation.status,
                            evaluation.reason.clone().unwrap_or_default()
                        );
                    }
                    ticks.fetch_add(
                        evaluation.metrics.as_ref().map_or(0, |m| m.ticks_run) as usize
                            + job.age as usize,
                        Ordering::SeqCst,
                    );
                    let row = CompareRow {
                        candidate: job.candidate.name.to_string(),
                        seed: job.seed,
                        age: job.age,
                        build_id: BUILD_ID.to_string(),
                        evaluation,
                    };
                    if let Ok(mut w) = writer.lock()
                        && let Ok(text) = serde_json::to_string(&row)
                    {
                        let _ = writeln!(w, "{text}");
                    }
                }
            });
        }
    });

    if let Ok(mut w) = writer.lock() {
        let _ = w.flush();
    }
    let wall = start.elapsed().as_secs_f64();
    let simulated = ticks.load(Ordering::SeqCst) as u64;
    let report = StageReport {
        plan: StagePlan {
            stage: "compare".into(),
            build_id: BUILD_ID.to_string(),
            candidates: chosen.iter().map(|c| c.name.to_string()).collect(),
            seed_set: seed_set.label().to_string(),
            seeds,
            ages,
            horizon_ticks,
            sample_every,
            trials: jobs.len(),
            workers,
            wall_seconds_cap: wall_seconds,
            pursuit_stop: pursuit_stop.as_str().to_string(),
            motor: sweep_name(),
        },
        wall_seconds: wall,
        completed: done.load(Ordering::SeqCst),
        skipped: skipped.load(Ordering::SeqCst),
        failed: failed.load(Ordering::SeqCst),
        simulated_ticks: simulated,
        ticks_per_second: if wall > 0.0 {
            simulated as f64 / wall
        } else {
            0.0
        },
        peak_rss_mib: peak_rss_mib(),
    };
    write_report(out, &report)?;
    Ok(report)
}

// ---------------------------------------------------------------------------------------
// shared plumbing
// ---------------------------------------------------------------------------------------

type PlanInputs = (Vec<&'static Candidate>, Vec<u64>, Vec<u64>);

fn plan_inputs(
    names: &[String],
    seed_set: SeedSet,
    seed_count: usize,
    ages: &[u64],
    workers: usize,
) -> Result<PlanInputs, String> {
    if workers == 0 || workers > 8 {
        return Err("--workers must be in 1..=8 for this brief's compute cap".into());
    }
    if ages.is_empty() {
        return Err("--ages named no age; 0 is the status quo and must be one of them".into());
    }
    for age in ages {
        if age % PROBE_EVERY != 0 {
            return Err(format!(
                "age {age} is not a multiple of the {PROBE_EVERY}-tick probe cadence, so the \
                 plant-only run could not be read at it"
            ));
        }
    }
    let mut sorted = ages.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let seeds = seed_set.seeds(seed_count)?;
    let chosen: Vec<&'static Candidate> = names
        .iter()
        .map(|n| candidate(n).ok_or_else(|| format!("{n} is not a declared candidate")))
        .collect::<Result<_, String>>()?;
    if chosen.is_empty() {
        return Err("--candidates named none".into());
    }
    Ok((chosen, seeds, sorted))
}

fn write_report(out: &Path, report: &StageReport) -> Result<(), String> {
    let path: PathBuf = out.join("summary.json");
    let text = serde_json::to_string_pretty(report).map_err(|e| format!("encoding report: {e}"))?;
    std::fs::write(&path, text).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// A short human summary of a finished stage.
pub fn print_report(report: &StageReport) {
    println!(
        "stage {} — {} of {} trials completed ({} skipped at the cap, {} failed) in {:.1} s",
        report.plan.stage,
        report.completed,
        report.plan.trials,
        report.skipped,
        report.failed,
        report.wall_seconds
    );
    println!(
        "  build {}, {} workers, ages {:?}, seeds {:?}",
        report.plan.build_id, report.plan.workers, report.plan.ages, report.plan.seeds
    );
    println!(
        "  {} simulated ticks at {:.0} ticks/s, peak RSS {:.0} MiB",
        report.simulated_ticks, report.ticks_per_second, report.peak_rss_mib
    );
}

/// The whole-field state that `--no-animals` produces is the thing every comparison arm is
/// founded from, so the opening trajectory is worth naming in one place: it is
/// [`OpeningPoint`], recorded by the run itself.
pub type Opening = Vec<OpeningPoint>;

// =========================================================================================
// stage 3: the coupled grazed opening (workstream Z)
// =========================================================================================
//
// Workstream S §7 "Neither": a plant-only prefix settles the plants against **no grazing**,
// and every arm then converges on a *grazed* standing crop of ΣP ≈ 215–230 — about half the
// ungrazed 390–450 the prefix produced. So the prefix is long enough to reach the plants' own
// state and therefore long enough to reach the wrong one, and the thing that actually settles
// the field is the founding. The honest target for a §11 change, on that evidence, is the
// grazed standing crop.
//
// This stage measures the opening that reading points at and S did not run: burn a field in
// with an **ordinary coupled world**, take that burn-in population out through
// [`cubarium_core::World::remove_all_animals`], found the identical fresh roster into the
// field it grazed, and run the ordinary 180,000 ticks after it.
//
// Age **0 is the status-quo arm** and is deliberately *not* burnt in or emptied: at tick 0 the
// burn-in population is the fresh roster, so a remove-and-refound there would be the identity
// in every number that matters and would still change the world — `Slots` hands a reused slot
// a higher generation, which enters `state_hash` and the organism-keyed draws. The status-quo
// arm therefore runs the ordinary constructor's world untouched, which is what makes it a
// reproduction of S's retained age-0 rows rather than a near-miss.
//
// **Row shape.** These rows are not [`Evaluation`]s. `evaluate::run` is private, builds its
// own world from a config and a seed and cannot be entered with a world that has already been
// burnt in and re-founded; `evaluate::precondition`, the only public preconditioning entry, is
// plant-only and refuses a populated world. So this stage carries [`GrazedRow`], measuring the
// pre-registered list and nothing else. Every measure that already has a shared public
// definition **is** that definition — [`FounderBroods`], [`CrossingCounter`], [`CensusKey`],
// [`guild_of`], [`OpeningPoint`], [`PlantBudgetTracker`] and the core's own plant record — so
// what is new here is the loop that drives them, and the age-0 arm is checked against S's
// retained rows measure by measure as well as by `state_hash`
// (`design/7_Research/ecology-v1-grazed-opening-preregistration-2026-09-16.md` §8).

/// The ages the grazed comparison declares. `0` is the status quo and is run without a
/// burn-in; the other three are S's plant-only ages, so the coupled and plant-only openings
/// are compared at the same age.
pub const GRAZED_AGES: [u64; 4] = [0, 48_000, 96_000, 180_000];

/// What the burn-in population was, and what left the world with it. Absent on the status-quo
/// arm, which has no burn-in at all.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BurnIn {
    /// Ticks of ordinary coupled world before the population was removed.
    pub ticks: u64,
    /// `state_hash` of the coupled world the instant **before** the removal.
    pub coupled_state_hash: u64,
    /// `state_hash` the instant **after** the removal and before the founding: the field on
    /// its own, with the books already closed on the bodies that left.
    pub emptied_state_hash: u64,
    pub population: u32,
    /// The burn-in population by visual form and by guild at the instant of removal.
    pub population_by_form: [u32; 5],
    pub population_by_guild: [u32; 3],
    /// Bodies whose parent chain roots at one of the burn-in founders, and how many of the 24
    /// founders were still alive.
    pub founders_alive: u32,
    /// `Σ material()` — structure, reserve and escrow — that left the world, and the stored
    /// energy that left with it. The material is booked out of `external_material_in`; the
    /// energy is booked nowhere and is reported here instead.
    pub material_removed: f64,
    pub energy_removed: f64,
    /// The world's own audits at the removal instant.
    pub mass_residual: f64,
    pub water_residual: f64,
    pub births_total: u64,
    pub deaths_total: [u64; 3],
}

/// The field a founding opens on, measured at the founding instant.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GrazedOpening {
    /// `state_hash` the instant after the roster is placed. On a burnt-in arm this is what
    /// the burn-in stage's saved state must independently reproduce.
    pub founding_state_hash: u64,
    pub foliage: f64,
    pub wood: f64,
    pub plant_reserve: f64,
    pub nutrient: f64,
    pub water: f64,
    /// The per-cell spread over the watched cells — the opening distribution the founders meet.
    pub foliage_median: f64,
    pub foliage_p10: f64,
    pub foliage_p90: f64,
    pub foliage_cv: f64,
    /// Cells already below ¼ and ½ of their **§11 seeding** when the roster is founded.
    pub below_quarter_of_seeding: u32,
    pub below_half_of_seeding: u32,
    pub alive_cells: u32,
    pub watched_cells: u32,
}

/// M's depletion split, as counts only: the per-crossing rows and the all-cell table are not
/// carried, because 48 arms of them is the 28 MiB S's `compare.jsonl` cost and this workstream
/// makes no claim that needs them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DepletionSplit {
    pub total_crossings: u64,
    pub with_withdrawal: u64,
    pub without_withdrawal: u64,
    pub ever_visited: u64,
    pub never_visited: u64,
    pub crossings_dropped: u64,
    /// The core plant record's own closing identity residual: a run whose accounting did not
    /// close is not a measurement.
    pub max_identity_residual: f64,
}

/// The 180,000 ticks after the founding, on the pre-registered measures and nothing else.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GrazedHorizon {
    pub ticks_run: u64,
    pub collapsed: bool,
    pub final_state_hash: u64,
    pub final_ecology_hash: u64,
    pub final_population: u32,
    pub final_forms_present: u32,
    pub min_forms_present: u32,
    pub mean_form_evenness: f64,
    pub founder_lineages_alive: u32,
    pub prey_births: u64,
    pub prey_deaths: u64,
    pub deaths_by_cause: [u64; 4],
    pub guild_final: [u32; 3],
    /// Distinct founders of each form that completed at least one brood, and the first brood
    /// tick per form, relative to the founding.
    pub founder_broods: FounderBroods,
    /// The crossing counter on the **arm's own** opening reference, the way every workstream
    /// reports it.
    pub crossings: Crossings,
    /// The same counter on the **common §11 reference** — the tick-0 foliage of the ordinary
    /// world of this (candidate, seed). This is the column that is comparable across openings.
    pub crossings_common: Crossings,
    pub depletion_split: DepletionSplit,
    /// Terminal starved cells: at the horizon, cells below ¼ and ½ of each reference.
    pub below_quarter_own: u32,
    pub below_half_own: u32,
    pub below_quarter_common: u32,
    pub below_half_common: u32,
    /// Median of `P_final / P_§11` over the watched cells.
    pub median_final_over_seeding: f64,
    pub final_foliage: f64,
    /// Mean ΣP over the late window — the last fifth of the declared horizon, S's window.
    /// **This is "the grazed standing crop the arm converges to".**
    pub late_foliage_mean: f64,
    /// The relative form, whose denominator is this arm's own opening. Reported so the
    /// declared `FOLIAGE_FLOOR` gate can be read, and **not** comparable across openings.
    pub late_foliage_over_opening: f64,
    /// Exact §4 flows over the **first simulated hour** after founding, summed over every
    /// cell: gross income, what went into foliage, what came back out, what mouths withdrew.
    pub income_first_hour: f64,
    pub foliage_in_first_hour: f64,
    pub foliage_out_first_hour: f64,
    pub withdrawal_first_hour: f64,
    pub max_abs_mass_residual: f64,
    pub max_abs_water_residual: f64,
}

/// One arm of the grazed comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrazedRow {
    pub candidate: String,
    pub seed: u64,
    /// The pursuit stopping rule and motor contract this arm ran under, by name (Astra,
    /// round-5 review P3). Z's retained rows were written without the fields under the reach
    /// envelope and Sweep and carry them explicitly since integration; the serde defaults
    /// name what a row without them ran only where that is its actual history.
    #[serde(default = "half_space_name")]
    pub pursuit_stop: String,
    #[serde(default = "sweep_name")]
    pub motor: String,
    /// Ticks of coupled burn-in before the founding. `0` is the status-quo arm.
    pub age: u64,
    pub build_id: String,
    pub elapsed_ms: u64,
    /// `true` when the arm ran to its horizon with every audit intact. A refused or broken arm
    /// carries `reason` and no horizon, and is never silently dropped.
    pub completed: bool,
    pub reason: Option<String>,
    pub burn_in: Option<BurnIn>,
    pub opening: GrazedOpening,
    pub horizon: Option<GrazedHorizon>,
    /// The whole-field trajectory of the first simulated hour after founding, every
    /// [`BOUNDARY_EVERY`] ticks.
    pub trajectory: Vec<OpeningPoint>,
}

/// One coupled burn-in run: the states it saved and how settled the field it produced is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrazedBurnInRun {
    pub candidate: String,
    pub seed: u64,
    pub build_id: String,
    pub ticks: u64,
    pub elapsed_ms: u64,
    pub readings: Vec<FieldReading>,
    pub ages: Vec<AgeState>,
    /// What each declared age's removal took out of the world.
    pub removals: Vec<BurnIn>,
}

/// Advance a world with its **animals in it** — the ordinary coupled tick loop, nothing
/// staged and nothing suppressed.
///
/// This is deliberately not [`precondition`]: that one is plant-only and refuses a populated
/// world, which is exactly the difference this stage exists to measure. `check_every` ticks
/// the world's own invariants are checked, and a violation stops the burn-in by name rather
/// than founding into a world that has already gone wrong. A burn-in whose population dies
/// out is reported by name too: an emptied world has nothing to remove and the arm it would
/// found is not the arm the comparison declared.
pub fn burn_in(world: &mut World, ticks: u64, check_every: u64) -> Result<(), String> {
    if ticks > 0 && world.population() == 0 {
        return Err(
            "a coupled burn-in needs the ordinary roster in the world, and this one \
                    holds nobody"
                .to_string(),
        );
    }
    let every = check_every.max(1);
    for _ in 0..ticks {
        world.step();
        world.drain_events();
        world.drain_hunter_events();
        world.drain_apex_dormancy_events();
        world.drain_apex_encounter_events();
        world.drain_quiet_events();
        if world.population() == 0 {
            return Err(format!(
                "the burn-in population died out at tick {}; there is no grazed field to found \
                 into",
                world.tick()
            ));
        }
        if world.tick() % every == 0 {
            world.check_invariants().map_err(|e| {
                format!(
                    "invariant violated while burning in at tick {}: {e}",
                    world.tick()
                )
            })?;
        }
    }
    Ok(())
}

/// The ordinary world of a candidate at a seed, with its roster founded at tick 0 by the
/// constructor and the run's two transients set before the first tick.
fn coupled_world(
    c: &Candidate,
    seed: u64,
    pursuit_stop: cubarium_core::hunter::PursuitStop,
) -> Result<World, String> {
    let config = c.config(seed)?;
    let mut world = World::new(config).map_err(|e| format!("world creation refused: {e}"))?;
    world.set_motor_model(cubarium_core::MotorModel::Sweep);
    world.set_pursuit_stop(pursuit_stop);
    Ok(world)
}

/// Cells below `fraction` of their reference, over the cells the reference watches.
fn below(p: &[f64], reference: &[f64], fraction: f64) -> u32 {
    p.iter()
        .zip(reference)
        .filter(|(_, r)| **r > 1e-9)
        .filter(|(v, r)| **v < fraction * **r)
        .count() as u32
}

/// The composition of a world's population by form and by guild, and how many of `founders`
/// are still alive in it.
fn composition(world: &World, founders: &BTreeSet<OrganismId>) -> ([u32; 5], [u32; 3], u32) {
    let mut by_form = [0u32; 5];
    let mut by_guild = [0u32; 3];
    let mut alive = 0u32;
    for (id, o) in world.state.organisms.iter() {
        by_form[(o.phenotype.form as usize).min(4)] += 1;
        by_guild[guild_of(o.phenotype.cap_foliage, o.phenotype.cap_detrital)] += 1;
        if founders.contains(&id) {
            alive += 1;
        }
    }
    (by_form, by_guild, alive)
}

/// The measures of one arm's horizon, on the pre-registered list and nothing else.
///
/// Every counter here is a shared public definition driven by this loop, never a second
/// implementation of one: the crossings are [`CrossingCounter`] on A's thresholds and A's
/// hysteresis, the broods are [`FounderBroods`], the split is [`PlantBudgetTracker`], the
/// classification is [`CensusKey`] and [`guild_of`], and the evenness is the same normalized
/// form entropy `evaluate::Recorder::sample` computes, written out here because that recorder
/// is private.
struct ArmRecorder {
    origin_tick: u64,
    sample_every: u64,
    /// The bodies the door placed. A lineage is alive when a live body's parent chain roots at
    /// one of these.
    founders: BTreeSet<OrganismId>,
    founder_parents: BTreeSet<OrganismId>,
    parent_of: BTreeMap<OrganismId, OrganismId>,
    key_of_id: BTreeMap<OrganismId, CensusKey>,
    broods: FounderBroods,
    crossings: CrossingCounter,
    crossings_common: CrossingCounter,
    /// The arm's **own** opening foliage, kept because the counter does not hand its reference
    /// back and the terminal starved-cell count has to be read against both references.
    p_ref: Vec<f64>,
    plant: PlantBudgetTracker,
    plant_scratch: Vec<CellSample>,
    plant_occupied: Vec<u16>,
    prey_births: u64,
    prey_deaths: u64,
    deaths_by_cause: [u64; 4],
    evenness_sum: f64,
    evenness_n: u64,
    min_forms_present: u32,
    late_start: u64,
    late_sum: f64,
    late_n: u64,
    max_mass: f64,
    max_water: f64,
    trajectory: Vec<OpeningPoint>,
    collapsed_at: Option<u64>,
}

impl ArmRecorder {
    /// Opened **after** the founding and before the first step, so the opening reference is
    /// the field the founders meet and every interval is measured from the founding.
    fn new(world: &World, seeding: &[f64], horizon_ticks: u64, sample_every: u64) -> ArmRecorder {
        let p_ref: Vec<f64> = world.state.fields.p.clone();
        let cfg = world.config().clone();
        let habitat = cubarium_core::habitat::Habitat::new(
            &cfg.habitat,
            cfg.seed,
            cfg.topology,
            cfg.world_scale,
        );
        let l_mu: Vec<f64> = habitat
            .light_base
            .iter()
            .zip(habitat.moisture_base.iter())
            .map(|(l, m)| l * m)
            .collect();
        let mut key_of_id = BTreeMap::new();
        let mut broods = FounderBroods::default();
        let mut founders = BTreeSet::new();
        for (id, o) in world.state.organisms.iter() {
            let key = CensusKey {
                form: o.phenotype.form.min(4),
                diet_bin: crate::movement::diet_bin(o.phenotype.diet) as u8,
                guild: guild_of(o.phenotype.cap_foliage, o.phenotype.cap_detrital) as u8,
            };
            key_of_id.insert(id, key);
            broods.found(key.form);
            founders.insert(id);
        }
        ArmRecorder {
            origin_tick: world.tick(),
            sample_every,
            founders,
            founder_parents: BTreeSet::new(),
            parent_of: BTreeMap::new(),
            key_of_id,
            broods,
            crossings: CrossingCounter::new(&p_ref, DEPLETION_FRACTION, RECOVERY_FRACTION),
            crossings_common: CrossingCounter::new(seeding, DEPLETION_FRACTION, RECOVERY_FRACTION),
            plant: PlantBudgetTracker::new(&p_ref, &l_mu, world.tick()),
            p_ref,
            plant_scratch: Vec::new(),
            plant_occupied: Vec::new(),
            prey_births: 0,
            prey_deaths: 0,
            deaths_by_cause: [0; 4],
            evenness_sum: 0.0,
            evenness_n: 0,
            min_forms_present: u32::MAX,
            // S's late window: the last fifth of the **declared** horizon, measured from the
            // founding, so a collapsed arm's death throes are not read as a standing crop.
            late_start: world.tick() + horizon_ticks.saturating_sub(horizon_ticks / WINDOWS),
            late_sum: 0.0,
            late_n: 0,
            max_mass: 0.0,
            max_water: 0.0,
            trajectory: Vec::new(),
            collapsed_at: None,
        }
    }

    fn root(&self, mut id: OrganismId) -> OrganismId {
        for _ in 0..self.parent_of.len() + 1 {
            match self.parent_of.get(&id) {
                Some(parent) => id = *parent,
                None => break,
            }
        }
        id
    }

    /// Every tick: the life events, and — every [`PROBE_EVERY`] ticks — both crossing counters
    /// and the per-cell record, in the fixed order `PlantBudgetTracker` documents (cumulative
    /// samples first, then this probe's crossings).
    fn absorb(&mut self, world: &mut World) {
        for event in world.drain_events() {
            match event {
                LifeEvent::Birth {
                    tick, id, parent, ..
                } => {
                    self.parent_of.insert(id, parent);
                    self.prey_births += 1;
                    let key = world
                        .state
                        .organisms
                        .get(id)
                        .map(|o| CensusKey {
                            form: o.phenotype.form.min(4),
                            diet_bin: crate::movement::diet_bin(o.phenotype.diet) as u8,
                            guild: guild_of(o.phenotype.cap_foliage, o.phenotype.cap_detrital)
                                as u8,
                        })
                        .unwrap_or(CensusKey {
                            form: 0,
                            diet_bin: 1,
                            guild: 2,
                        });
                    self.key_of_id.insert(id, key);
                    // A `Birth` is emitted when gestation completes and the child is
                    // committed, so this **is** a completed brood. The parent's own form is
                    // used, exactly as the ladder's gate uses it.
                    if self.founders.contains(&parent) {
                        let form = self.key_of_id.get(&parent).map_or(0, |k| k.form);
                        let first = self.founder_parents.insert(parent);
                        self.broods
                            .brood(form, tick.saturating_sub(self.origin_tick), first);
                    }
                }
                LifeEvent::Death { cause, .. } => {
                    self.prey_deaths += 1;
                    self.deaths_by_cause[match cause {
                        DeathCause::Starvation => 0,
                        DeathCause::Age => 1,
                        DeathCause::Collapse => 2,
                        DeathCause::Predation => 3,
                    }] += 1;
                }
            }
        }
        world.drain_hunter_events();
        world.drain_apex_dormancy_events();
        world.drain_apex_encounter_events();
        world.drain_quiet_events();

        if world.tick() % PROBE_EVERY == 0 {
            self.probe(world);
        }
    }

    fn probe(&mut self, world: &World) {
        let tick = world.tick();
        let p = &world.state.fields.p;
        let mut occupancy: BTreeSet<u16> = BTreeSet::new();
        for (_, o) in world.state.organisms.iter() {
            occupancy.insert(
                cubarium_surface::cell_of(world.topology(), world.scale(), &o.pos).index() as u16,
            );
        }
        if let Some(rec) = world.plant_budget() {
            self.plant_scratch.clear();
            self.plant_scratch
                .extend(rec.cells.iter().map(|c| CellSample {
                    foliage_in: c.foliage_in(),
                    foliage_out: c.foliage_out(),
                    withdrawal: c.withdrawal_foliage,
                    income: c.income,
                    maintenance_unpaid: c.maintenance_unpaid,
                    death_foliage: c.death_foliage,
                    wood_sum: c.w_sum,
                    light_sum: c.light_effective_sum,
                    nutrient_sum: c.nutrient_sum,
                    ticks_alive: c.ticks_alive,
                }));
            self.plant_occupied.clear();
            self.plant_occupied.extend(occupancy.iter().copied());
            self.plant
                .observe(tick, &self.plant_scratch, &self.plant_occupied);
        }
        let plant = &mut self.plant;
        self.crossings
            .observe_reporting(p, |cell, kind| match kind {
                CrossingKind::Depleted => {
                    plant.depleted(cell, tick, p.get(cell).copied().unwrap_or(0.0))
                }
                CrossingKind::Recovered => plant.recovered(cell, tick),
            });
        // The common §11 reference is a second reading of the same foliage against a fixed
        // reference; it drives no per-cell record of its own, because the record's crossing
        // rows belong to the counter the rest of the campaign reports.
        self.crossings_common.observe(p);
    }

    /// Every `sample_every` ticks: the audits, the form evenness, the late window, and — for
    /// the first simulated hour — the whole-field trajectory.
    fn sample(&mut self, world: &World) {
        let tick = world.tick();
        let since = tick - self.origin_tick;
        let state = &world.state;
        self.max_mass = self.max_mass.max(world.mass_residual().abs());
        self.max_water = self.max_water.max(world.water_residual().abs());

        let mut forms = [0u32; 8];
        let mut population = 0u32;
        for (_, o) in state.organisms.iter() {
            population += 1;
            forms[(o.phenotype.form as usize).min(7)] += 1;
        }
        let present = forms.iter().filter(|c| **c > 0).count() as u32;
        let entropy = if population > 0 {
            let total = f64::from(population);
            -forms
                .iter()
                .filter(|c| **c > 0)
                .map(|c| {
                    let p = f64::from(*c) / total;
                    p * p.ln()
                })
                .sum::<f64>()
        } else {
            0.0
        };
        self.evenness_sum += (entropy / FORMS_POSSIBLE.ln()).clamp(0.0, 1.0);
        self.evenness_n += 1;
        self.min_forms_present = self.min_forms_present.min(present);

        let foliage: f64 = state.fields.p.iter().sum();
        if tick >= self.late_start {
            self.late_sum += foliage;
            self.late_n += 1;
        }
        if since <= OPENING_TRAJECTORY_TICKS
            && self.trajectory.last().map(|o| o.ticks_since_founding) != Some(since)
        {
            let mut point = OpeningPoint {
                ticks_since_founding: since,
                foliage,
                wood: state.ecology.wood.iter().sum(),
                plant_reserve: state.ecology.plant_reserve.iter().sum(),
                nutrient: state.fields.n.iter().sum(),
                population,
                ..OpeningPoint::default()
            };
            if let Some(rec) = world.plant_budget() {
                for c in &rec.cells {
                    point.foliage_in += c.foliage_in();
                    point.foliage_out += c.foliage_out();
                    point.income += c.income;
                    point.withdrawal += c.withdrawal_foliage;
                }
            }
            self.trajectory.push(point);
        }
    }

    fn finish(self, world: &World, seeding: &[f64], opening_foliage: f64) -> GrazedHorizon {
        let p = &world.state.fields.p;
        let p_ref = &self.p_ref;
        let live: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
        let lineages: BTreeSet<OrganismId> = live
            .iter()
            .map(|id| self.root(*id))
            .filter(|root| self.founders.contains(root))
            .collect();
        let mut forms = [0u32; 8];
        let mut guild_final = [0u32; 3];
        for (_, o) in world.state.organisms.iter() {
            forms[(o.phenotype.form as usize).min(7)] += 1;
            guild_final[guild_of(o.phenotype.cap_foliage, o.phenotype.cap_detrital)] += 1;
        }
        let mut ratios: Vec<f64> = p
            .iter()
            .zip(seeding)
            .filter(|(_, r)| **r > 1e-9)
            .map(|(v, r)| v / r)
            .collect();
        ratios.sort_by(|a, b| a.partial_cmp(b).expect("finite foliage"));
        let residual = world.plant_budget_residual();
        let (samples, ticks) = match world.plant_budget() {
            Some(rec) => (
                rec.cells
                    .iter()
                    .map(|c| CellSample {
                        foliage_in: c.foliage_in(),
                        foliage_out: c.foliage_out(),
                        withdrawal: c.withdrawal_foliage,
                        income: c.income,
                        maintenance_unpaid: c.maintenance_unpaid,
                        death_foliage: c.death_foliage,
                        wood_sum: c.w_sum,
                        light_sum: c.light_effective_sum,
                        nutrient_sum: c.nutrient_sum,
                        ticks_alive: c.ticks_alive,
                    })
                    .collect::<Vec<_>>(),
                rec.ticks,
            ),
            None => (Vec::new(), 0),
        };
        let split = self.plant.finish(&samples, p, residual, ticks);
        let hour = self
            .trajectory
            .iter()
            .filter(|t| t.ticks_since_founding <= OPENING_TRAJECTORY_TICKS)
            .next_back()
            .copied()
            .unwrap_or_default();
        let final_foliage: f64 = p.iter().sum();
        let late_mean = if self.late_n > 0 {
            self.late_sum / self.late_n as f64
        } else {
            final_foliage
        };
        GrazedHorizon {
            ticks_run: world.tick() - self.origin_tick,
            collapsed: self.collapsed_at.is_some(),
            final_state_hash: state_hash(&world.state),
            final_ecology_hash: cubarium_core::snapshot::ecology_hash(&world.state),
            final_population: live.len() as u32,
            final_forms_present: forms.iter().filter(|c| **c > 0).count() as u32,
            min_forms_present: if self.min_forms_present == u32::MAX {
                0
            } else {
                self.min_forms_present
            },
            mean_form_evenness: if self.evenness_n > 0 {
                self.evenness_sum / self.evenness_n as f64
            } else {
                0.0
            },
            founder_lineages_alive: lineages.len() as u32,
            prey_births: self.prey_births,
            prey_deaths: self.prey_deaths,
            deaths_by_cause: self.deaths_by_cause,
            guild_final,
            founder_broods: self.broods,
            crossings: self.crossings.summary(),
            crossings_common: self.crossings_common.summary(),
            depletion_split: DepletionSplit {
                total_crossings: split.total_crossings,
                with_withdrawal: split.with_withdrawal,
                without_withdrawal: split.without_withdrawal,
                ever_visited: split.ever_visited,
                never_visited: split.never_visited,
                crossings_dropped: split.crossings_dropped,
                max_identity_residual: split.max_identity_residual,
            },
            below_quarter_own: below(p, p_ref, DEPLETION_FRACTION),
            below_half_own: below(p, p_ref, 0.5),
            below_quarter_common: below(p, seeding, DEPLETION_FRACTION),
            below_half_common: below(p, seeding, 0.5),
            median_final_over_seeding: quantile(&ratios, 0.5),
            final_foliage,
            late_foliage_mean: late_mean,
            late_foliage_over_opening: if opening_foliage.abs() > 1e-12 {
                late_mean / opening_foliage
            } else {
                0.0
            },
            income_first_hour: hour.income,
            foliage_in_first_hour: hour.foliage_in,
            foliage_out_first_hour: hour.foliage_out,
            withdrawal_first_hour: hour.withdrawal,
            max_abs_mass_residual: self.max_mass,
            max_abs_water_residual: self.max_water,
        }
    }
}

/// The §11 seeding of the ordinary world of this (candidate, seed): the tick-0 foliage the
/// constructor lays down. This is the **common reference** every arm's terminal starved cells
/// and second crossing count are read against, so the reference does not move with the
/// opening (S §4.2 is the reason it has to be fixed).
fn seeding_of(c: &Candidate, seed: u64) -> Result<Vec<f64>, String> {
    let config = c.config(seed)?;
    let world = World::new(config).map_err(|e| format!("world creation refused: {e}"))?;
    Ok(world.state.fields.p.clone())
}

/// One arm: burn in, remove, found, and run the horizon. `age == 0` is the status quo and is
/// neither burnt in nor emptied.
pub fn grazed_run(
    c: &Candidate,
    seed: u64,
    age: u64,
    horizon_ticks: u64,
    sample_every: u64,
    pursuit_stop: cubarium_core::hunter::PursuitStop,
) -> GrazedRow {
    let start = Instant::now();
    let refused = |reason: String, elapsed: u64| GrazedRow {
        candidate: c.name.to_string(),
        seed,
        pursuit_stop: pursuit_stop.as_str().to_string(),
        motor: sweep_name(),
        age,
        build_id: BUILD_ID.to_string(),
        elapsed_ms: elapsed,
        completed: false,
        reason: Some(reason),
        burn_in: None,
        opening: GrazedOpening::default(),
        horizon: None,
        trajectory: Vec::new(),
    };
    let ms = |s: Instant| s.elapsed().as_millis() as u64;

    let seeding = match seeding_of(c, seed) {
        Ok(s) => s,
        Err(e) => return refused(e, ms(start)),
    };
    let mut world = match coupled_world(c, seed, pursuit_stop) {
        Ok(w) => w,
        Err(e) => return refused(e, ms(start)),
    };
    let founders0: BTreeSet<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();

    // --- the burn-in, the removal and the second founding --------------------------------
    let mut burn = None;
    if age > 0 {
        if let Err(e) = burn_in(&mut world, age, PRECONDITION_CHECK_EVERY) {
            return refused(e, ms(start));
        }
        let coupled_state_hash = state_hash(&world.state);
        let (by_form, by_guild, founders_alive) = composition(&world, &founders0);
        let population = world.population() as u32;
        let (births_total, deaths_total) = (world.state.births_total, world.state.deaths_total);
        let removed = match world.remove_all_animals() {
            Ok(r) => r,
            Err(e) => return refused(format!("removing the burn-in population: {e}"), ms(start)),
        };
        let e_r = world.config().organism.reserve_energy_density;
        burn = Some(BurnIn {
            ticks: age,
            coupled_state_hash,
            emptied_state_hash: state_hash(&world.state),
            population,
            population_by_form: by_form,
            population_by_guild: by_guild,
            founders_alive,
            material_removed: removed.iter().map(|o| o.material()).sum(),
            energy_removed: removed
                .iter()
                .map(|o| {
                    o.energy
                        + e_r * o.reserve
                        + o.escrow
                            .as_ref()
                            .map_or(0.0, |e| e_r * (e.structure + e.reserve) + e.energy)
                })
                .sum(),
            mass_residual: world.mass_residual(),
            water_residual: world.water_residual(),
            births_total,
            deaths_total,
        });
        if let Err(e) = world.found_roster() {
            return refused(format!("founding the fresh roster: {e}"), ms(start));
        }
    }

    // --- the opening the founders meet ----------------------------------------------------
    let alive_min = world.config().plant.alive_min;
    let watched: Vec<usize> = (0..seeding.len()).filter(|i| seeding[*i] > 1e-9).collect();
    let foliage: Vec<f64> = watched.iter().map(|i| world.state.fields.p[*i]).collect();
    let (opening_foliage, median, p10, p90, cv) = spread(&foliage);
    let opening = GrazedOpening {
        founding_state_hash: state_hash(&world.state),
        foliage: world.state.fields.p.iter().sum(),
        wood: world.state.ecology.wood.iter().sum(),
        plant_reserve: world.state.ecology.plant_reserve.iter().sum(),
        nutrient: world.state.fields.n.iter().sum(),
        water: world.state.fields.w.iter().sum(),
        foliage_median: median,
        foliage_p10: p10,
        foliage_p90: p90,
        foliage_cv: cv,
        below_quarter_of_seeding: below(&world.state.fields.p, &seeding, DEPLETION_FRACTION),
        below_half_of_seeding: below(&world.state.fields.p, &seeding, 0.5),
        alive_cells: world
            .state
            .ecology
            .wood
            .iter()
            .filter(|w| {
                cubarium_core::fields::CellClass::of(**w, alive_min)
                    == cubarium_core::fields::CellClass::Alive
            })
            .count() as u32,
        watched_cells: watched.len() as u32,
    };

    // --- the horizon ----------------------------------------------------------------------
    // Both recorders are opened after the founding and before the first step, exactly where
    // `evaluate::run` opens them, so the ledger's and the record's `P₀` is the field the
    // founders meet and the closing identity spans the whole horizon.
    world.record_body_budgets(true);
    world.record_plant_budgets(true);
    let mut rec = ArmRecorder::new(&world, &seeding, horizon_ticks, sample_every);
    world.drain_events();
    world.drain_hunter_events();
    world.drain_apex_dormancy_events();
    world.drain_apex_encounter_events();
    world.drain_quiet_events();
    rec.sample(&world);

    for _ in 0..horizon_ticks {
        world.step();
        rec.absorb(&mut world);
        if (world.tick() - rec.origin_tick) % rec.sample_every == 0 {
            if let Err(e) = world.check_invariants() {
                return refused(
                    format!("invariant violated at tick {}: {e}", world.tick()),
                    ms(start),
                );
            }
            rec.sample(&world);
        }
        if world.population() == 0 {
            // Nothing is ever created from nothing: an empty world cannot recover, so the
            // horizon stops here and the collapse tick is the honest survival time.
            rec.collapsed_at = Some(world.tick() - rec.origin_tick);
            break;
        }
    }
    let trajectory = rec.trajectory.clone();
    let horizon = rec.finish(&world, &seeding, opening_foliage);
    GrazedRow {
        candidate: c.name.to_string(),
        seed,
        pursuit_stop: pursuit_stop.as_str().to_string(),
        motor: sweep_name(),
        age,
        build_id: BUILD_ID.to_string(),
        elapsed_ms: ms(start),
        completed: true,
        reason: None,
        burn_in: burn,
        opening,
        horizon: Some(horizon),
        trajectory,
    }
}

// --- phase 1: the coupled burn-in, saved and accounted -----------------------------------

/// One coupled burn-in run to the last declared age, with a reading every [`READING_EVERY`]
/// ticks and the **emptied, re-founded** field saved at every declared age.
///
/// The state saved at an age is the opening a founding there produces, so it is the
/// conservation-accounted snapshot of the full coupled field Astra's item 6 asks for: the
/// removal's books are already closed in it. The arm of the same age re-runs its own burn-in
/// rather than loading this file — the measured chain stays snapshot-free, exactly as S's
/// arms do — and the two are then checked against each other by `founding_state_hash`, which
/// is what makes the agreement evidence rather than an assumption.
pub fn grazed_burn_in_run(
    c: &Candidate,
    seed: u64,
    ages: &[u64],
    pursuit_stop: cubarium_core::hunter::PursuitStop,
    out: &Path,
) -> Result<GrazedBurnInRun, String> {
    let start = Instant::now();
    let mut world = coupled_world(c, seed, pursuit_stop)?;
    world.record_plant_budgets(true);
    let founders0: BTreeSet<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();

    let p_ref: Vec<f64> = world.state.fields.p.clone();
    let alive_min = world.config().plant.alive_min;
    let mut crossings = CrossingCounter::new(&p_ref, DEPLETION_FRACTION, RECOVERY_FRACTION);
    let watched: Vec<usize> = (0..p_ref.len()).filter(|i| p_ref[*i] > 1e-9).collect();

    let slots = (WINDOW_TICKS / BOUNDARY_EVERY) as usize + 1;
    let mut ring: Vec<Boundary> = Vec::with_capacity(slots);
    let mut readings: Vec<FieldReading> = Vec::new();
    let mut ages_out: Vec<AgeState> = Vec::new();
    let mut removals: Vec<BurnIn> = Vec::new();
    let mut last_crossings = 0u64;
    let horizon = ages.iter().copied().max().unwrap_or(0);

    let reading =
        |world: &World, ring: &[Boundary], crossings: &CrossingCounter, last: &mut u64| {
            let now = boundary_of(world);
            let then = ring
                .iter()
                .filter(|b| b.tick < now.tick)
                .filter(|b| b.tick + WINDOW_TICKS >= now.tick)
                .min_by_key(|b| b.tick)
                .or_else(|| {
                    ring.iter()
                        .filter(|b| b.tick < now.tick)
                        .max_by_key(|b| b.tick)
                });
            let window_ticks = then.map_or(0, |b| now.tick - b.tick);
            let flows = |of: fn(&CellSample) -> f64| match then {
                Some(b) => sum_cells(&now.cells, of) - sum_cells(&b.cells, of),
                None => 0.0,
            };
            let foliage: Vec<f64> = watched.iter().map(|i| now.p[*i]).collect();
            let (_, median, p10, p90, cv) = spread(&foliage);
            let total = crossings.depletions();
            let row = FieldReading {
                tick: now.tick,
                window_ticks,
                foliage: stock(&now.p, then.map(|b| &b.p)),
                wood: stock(&now.w, then.map(|b| &b.w)),
                plant_reserve: stock(&now.q, then.map(|b| &b.q)),
                nutrient: stock(&now.n, then.map(|b| &b.n)),
                income: flows(|c| c.income),
                foliage_in: flows(|c| c.foliage_in),
                foliage_out: flows(|c| c.foliage_out),
                withdrawal: flows(|c| c.withdrawal),
                crossings_total: total,
                crossings_in_window: total - *last,
                recoveries_total: crossings.recoveries(),
                depleted_now: crossings.depleted_now(),
                foliage_median: median,
                foliage_p10: p10,
                foliage_p90: p90,
                foliage_cv: cv,
                alive_cells: world
                    .state
                    .ecology
                    .wood
                    .iter()
                    .filter(|w| {
                        cubarium_core::fields::CellClass::of(**w, alive_min)
                            == cubarium_core::fields::CellClass::Alive
                    })
                    .count() as u32,
            };
            *last = total;
            row
        };

    readings.push(reading(&world, &ring, &crossings, &mut last_crossings));
    ring.push(boundary_of(&world));

    let mut tick = 0u64;
    while tick < horizon {
        let step = PROBE_EVERY.min(horizon - tick);
        burn_in(&mut world, step, PRECONDITION_CHECK_EVERY)?;
        tick = world.tick();
        crossings.observe(&world.state.fields.p);
        if tick % BOUNDARY_EVERY == 0 {
            if ring.len() == slots {
                ring.remove(0);
            }
            ring.push(boundary_of(&world));
        }
        if tick % READING_EVERY == 0 || ages.contains(&tick) {
            readings.push(reading(&world, &ring, &crossings, &mut last_crossings));
        }
        if ages.contains(&tick) {
            let (age_state, removal) = save_grazed_age(&world, &founders0, tick, c, seed, out)?;
            ages_out.push(age_state);
            removals.push(removal);
        }
    }

    Ok(GrazedBurnInRun {
        candidate: c.name.to_string(),
        seed,
        build_id: BUILD_ID.to_string(),
        ticks: world.tick(),
        elapsed_ms: start.elapsed().as_millis() as u64,
        readings,
        ages: ages_out,
        removals,
    })
}

/// Empty and re-found a **clone** of the burn-in world at `age`, save the opening it produces,
/// and report what the removal took out.
///
/// The clone is thrown away: the burn-in itself must carry on coupled to the next age, and a
/// world that has already been emptied once has no population to remove at the next one. The
/// hash it produces is the one the arm of the same age must report in `founding_state_hash`.
fn save_grazed_age(
    world: &World,
    founders0: &BTreeSet<OrganismId>,
    age: u64,
    c: &Candidate,
    seed: u64,
    out: &Path,
) -> Result<(AgeState, BurnIn), String> {
    let plant_state_hash = state_hash(&world.state);
    let mut clone = World::from_state(world.state.clone())
        .map_err(|e| format!("rebuilding the state at age {age}: {e}"))?;
    let (by_form, by_guild, founders_alive) = composition(&clone, founders0);
    let population = clone.population() as u32;
    let (births_total, deaths_total) = (clone.state.births_total, clone.state.deaths_total);
    let removed = clone
        .remove_all_animals()
        .map_err(|e| format!("removing the burn-in population at age {age}: {e}"))?;
    let e_r = clone.config().organism.reserve_energy_density;
    let removal = BurnIn {
        ticks: age,
        coupled_state_hash: plant_state_hash,
        emptied_state_hash: state_hash(&clone.state),
        population,
        population_by_form: by_form,
        population_by_guild: by_guild,
        founders_alive,
        material_removed: removed.iter().map(|o| o.material()).sum(),
        energy_removed: removed
            .iter()
            .map(|o| {
                o.energy
                    + e_r * o.reserve
                    + o.escrow
                        .as_ref()
                        .map_or(0.0, |e| e_r * (e.structure + e.reserve) + e.energy)
            })
            .sum(),
        // `from_state` re-derives the residual baseline so it reads zero at the rebuild; what
        // this number checks is that the *removal* did not move it.
        mass_residual: clone.mass_residual(),
        water_residual: clone.water_residual(),
        births_total,
        deaths_total,
    };
    clone
        .found_roster()
        .map_err(|e| format!("founding at age {age}: {e}"))?;
    let founding_state_hash = state_hash(&clone.state);

    let dir = out.join("states");
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let name = format!("{}-seed{seed}-age{age}.cube", c.name);
    let path = dir.join(&name);
    let bytes = cubarium_core::snapshot::encode_snapshot(&clone.state, BUILD_ID);
    std::fs::write(&path, &bytes).map_err(|e| format!("writing {}: {e}", path.display()))?;

    // Checked to be the state it claims to be rather than assumed to be: decoded straight back
    // off disk and hashed.
    let read = std::fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let (_, decoded) = cubarium_core::snapshot::decode_snapshot(&read)
        .map_err(|e| format!("decoding {}: {e:?}", path.display()))?;
    let snapshot_state_hash = state_hash(&decoded);

    Ok((
        AgeState {
            age,
            plant_state_hash,
            founding_state_hash,
            snapshot: format!("states/{name}"),
            snapshot_bytes: bytes.len() as u64,
            snapshot_state_hash,
        },
        removal,
    ))
}

// --- the stage ----------------------------------------------------------------------------

/// The whole grazed campaign: the coupled burn-ins and their saved openings, then one arm per
/// (candidate, seed, age). Age 0 is the status-quo arm and is run without a burn-in; it is the
/// reproduction target for S's retained age-0 rows and the comparison's own control.
#[allow(clippy::too_many_arguments)]
pub fn run_grazed(
    names: &[String],
    seed_set: SeedSet,
    seed_count: usize,
    ages: &[u64],
    horizon_ticks: u64,
    sample_every: u64,
    pursuit_stop: cubarium_core::hunter::PursuitStop,
    workers: usize,
    wall_seconds: u64,
    out: &Path,
) -> Result<StageReport, String> {
    let (chosen, seeds, ages) = plan_inputs(names, seed_set, seed_count, ages, workers)?;
    let burn_ages: Vec<u64> = ages.iter().copied().filter(|a| *a > 0).collect();
    std::fs::create_dir_all(out).map_err(|e| format!("creating {}: {e}", out.display()))?;

    let start = Instant::now();
    let deadline = start + std::time::Duration::from_secs(wall_seconds);
    let skipped = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    let ticks = AtomicUsize::new(0);

    // --- phase 1: the burn-ins ------------------------------------------------------------
    if !burn_ages.is_empty() {
        let jobs: Vec<(&'static Candidate, u64)> = chosen
            .iter()
            .flat_map(|c| seeds.iter().map(move |s| (*c, *s)))
            .collect();
        let path = out.join("burn-in.jsonl");
        let file = std::fs::File::create(&path)
            .map_err(|e| format!("creating {}: {e}", path.display()))?;
        let writer = Mutex::new(std::io::BufWriter::new(file));
        let cursor = AtomicUsize::new(0);
        let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..workers.min(jobs.len().max(1)) {
                scope.spawn(|| {
                    loop {
                        let i = cursor.fetch_add(1, Ordering::SeqCst);
                        if i >= jobs.len() {
                            return;
                        }
                        if Instant::now() >= deadline {
                            skipped.fetch_add(1, Ordering::SeqCst);
                            continue;
                        }
                        let (c, seed) = jobs[i];
                        match grazed_burn_in_run(c, seed, &burn_ages, pursuit_stop, out) {
                            Ok(run) => {
                                ticks.fetch_add(run.ticks as usize, Ordering::SeqCst);
                                if let Ok(mut w) = writer.lock()
                                    && let Ok(text) = serde_json::to_string(&run)
                                {
                                    let _ = writeln!(w, "{text}");
                                }
                            }
                            Err(e) => {
                                failed.fetch_add(1, Ordering::SeqCst);
                                failures
                                    .lock()
                                    .expect("failure list")
                                    .push(format!("{} seed {seed}: {e}", c.name));
                            }
                        }
                    }
                });
            }
        });
        if let Ok(mut w) = writer.lock() {
            let _ = w.flush();
        }
        for f in failures
            .into_inner()
            .map_err(|e| format!("failure mutex: {e}"))?
        {
            eprintln!("grazed burn-in failed — {f}");
        }
    }

    // --- phase 2: the arms ----------------------------------------------------------------
    struct Job {
        candidate: &'static Candidate,
        seed: u64,
        age: u64,
    }
    let mut jobs = Vec::new();
    for c in &chosen {
        for seed in &seeds {
            for age in &ages {
                jobs.push(Job {
                    candidate: c,
                    seed: *seed,
                    age: *age,
                });
            }
        }
    }
    // Longest first: a 180,000-tick burn-in in front of the horizon sets the wall time, so it
    // must not be the job a worker picks up last.
    jobs.sort_by_key(|j| std::cmp::Reverse(j.age));

    let rows_path = out.join("grazed.jsonl");
    let file = std::fs::File::create(&rows_path)
        .map_err(|e| format!("creating {}: {e}", rows_path.display()))?;
    let writer = Mutex::new(std::io::BufWriter::new(file));
    let cursor = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= jobs.len() {
                        return;
                    }
                    if Instant::now() >= deadline {
                        skipped.fetch_add(1, Ordering::SeqCst);
                        continue;
                    }
                    let job = &jobs[i];
                    let row = grazed_run(
                        job.candidate,
                        job.seed,
                        job.age,
                        horizon_ticks,
                        sample_every,
                        pursuit_stop,
                    );
                    if row.completed {
                        done.fetch_add(1, Ordering::SeqCst);
                    } else {
                        failed.fetch_add(1, Ordering::SeqCst);
                        eprintln!(
                            "grazed arm {} seed {} age {} — {}",
                            job.candidate.name,
                            job.seed,
                            job.age,
                            row.reason.clone().unwrap_or_default()
                        );
                    }
                    ticks.fetch_add(
                        row.horizon.as_ref().map_or(0, |h| h.ticks_run) as usize + job.age as usize,
                        Ordering::SeqCst,
                    );
                    if let Ok(mut w) = writer.lock()
                        && let Ok(text) = serde_json::to_string(&row)
                    {
                        let _ = writeln!(w, "{text}");
                    }
                }
            });
        }
    });
    if let Ok(mut w) = writer.lock() {
        let _ = w.flush();
    }

    let wall = start.elapsed().as_secs_f64();
    let simulated = ticks.load(Ordering::SeqCst) as u64;
    let report = StageReport {
        plan: StagePlan {
            stage: "grazed".into(),
            build_id: BUILD_ID.to_string(),
            candidates: chosen.iter().map(|c| c.name.to_string()).collect(),
            seed_set: seed_set.label().to_string(),
            seeds,
            ages,
            horizon_ticks,
            sample_every,
            trials: jobs.len(),
            workers,
            wall_seconds_cap: wall_seconds,
            pursuit_stop: pursuit_stop.as_str().to_string(),
            motor: sweep_name(),
        },
        wall_seconds: wall,
        completed: done.load(Ordering::SeqCst),
        skipped: skipped.load(Ordering::SeqCst),
        failed: failed.load(Ordering::SeqCst),
        simulated_ticks: simulated,
        ticks_per_second: if wall > 0.0 {
            simulated as f64 / wall
        } else {
            0.0
        },
        peak_rss_mib: peak_rss_mib(),
    };
    write_report(out, &report)?;
    Ok(report)
}
