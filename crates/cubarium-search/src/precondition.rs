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

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_core::World;
use cubarium_core::snapshot::state_hash;
use serde::{Deserialize, Serialize};

use crate::calibrate::{Candidate, SeedSet, candidate};
use crate::evaluate::{
    BUILD_ID, DEPLETION_FRACTION, Evaluation, OpeningPoint, PRECONDITION_CHECK_EVERY, PROBE_EVERY,
    RECOVERY_FRACTION, RunOptions, evaluate_with, precondition,
};
use crate::movement::CrossingCounter;
use crate::plant_budget::CellSample;

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
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|v| v.parse::<f64>().ok()))
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
    let cv = if mean.abs() > 1e-12 { var.sqrt() / mean } else { 0.0 };
    (total, quantile(&sorted, 0.5), quantile(&sorted, 0.1), quantile(&sorted, 0.9), cv)
}

/// One stock's settling between two whole-field vectors.
fn stock(now: &[f64], then: Option<&Vec<f64>>) -> StockReading {
    let total: f64 = now.iter().sum();
    let Some(then) = then else {
        return StockReading { total, ..StockReading::default() };
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
        relative_total: if total.abs() > 1e-12 { (total - before) / total } else { 0.0 },
        cell_change_rate: if total.abs() > 1e-12 { moved / total } else { 0.0 },
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
) -> Result<(cubarium_core::WorldConfig, cubarium_core::config::FounderConfig), String> {
    let mut config = c.config(seed)?;
    let roster = config.founders.clone();
    config.founders.kinds.clear();
    config.founders.count = 0;
    config.validate().map_err(|e| format!("config rejected: {e}"))?;
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
    let reading = |world: &World, ring: &[Boundary], crossings: &CrossingCounter, last: &mut u64| {
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
            .or_else(|| ring.iter().filter(|b| b.tick < now.tick).max_by_key(|b| b.tick));
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
                .filter(|w| cubarium_core::fields::CellClass::of(**w, alive_min)
                    == cubarium_core::fields::CellClass::Alive)
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
    clone.found_roster().map_err(|e| format!("founding at age {age}: {e}"))?;
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
    let jobs: Vec<(&'static Candidate, u64)> =
        chosen.iter().flat_map(|c| seeds.iter().map(move |s| (*c, *s))).collect();

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
    let failures = failed.into_inner().map_err(|e| format!("failure mutex: {e}"))?;
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
        },
        wall_seconds: wall,
        completed: done.load(Ordering::SeqCst),
        skipped: skipped.load(Ordering::SeqCst),
        failed: failures.len(),
        simulated_ticks: simulated,
        ticks_per_second: if wall > 0.0 { simulated as f64 / wall } else { 0.0 },
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
                jobs.push(Job { candidate: c, values: values.clone(), seed: *seed, age: *age });
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
                        evaluation.metrics.as_ref().map_or(0, |m| m.ticks_run) as usize + job.age as usize,
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
        },
        wall_seconds: wall,
        completed: done.load(Ordering::SeqCst),
        skipped: skipped.load(Ordering::SeqCst),
        failed: failed.load(Ordering::SeqCst),
        simulated_ticks: simulated,
        ticks_per_second: if wall > 0.0 { simulated as f64 / wall } else { 0.0 },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declared_ages_are_sorted_distinct_and_open_at_the_status_quo() {
        assert_eq!(DEFAULT_AGES[0], 0, "age 0 is the status quo arm");
        let mut sorted = DEFAULT_AGES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, DEFAULT_AGES.to_vec(), "declared ascending and distinct");
        for age in DEFAULT_AGES {
            assert_eq!(age % PROBE_EVERY, 0, "every age falls on a probe");
            assert_eq!(age % BOUNDARY_EVERY, 0, "and on a window boundary");
        }
    }

    /// The settling numbers are read off hand-built vectors, so what each one means is fixed
    /// here rather than inferred from a campaign that happens to produce small values.
    #[test]
    fn a_stock_that_does_not_move_reads_as_settled_and_one_that_trades_does_not() {
        let now = vec![1.0, 2.0, 3.0];
        let still = stock(&now, Some(&now.clone()));
        assert_eq!(still.total, 6.0);
        assert_eq!(still.delta_total, 0.0);
        assert_eq!(still.relative_total, 0.0);
        assert_eq!(still.cell_change_rate, 0.0);
        assert_eq!(still.cells_moving, 0);

        // The total is unchanged, but two cells traded a whole unit: a field can be flat in
        // aggregate and still be rearranging itself, which is a different kind of unsettled.
        let traded = stock(&now, Some(&vec![2.0, 1.0, 3.0]));
        assert_eq!(traded.delta_total, 0.0, "the total did not move");
        assert_eq!(traded.relative_total, 0.0);
        assert!((traded.cell_change_rate - 2.0 / 6.0).abs() < 1e-12, "but the cells did");
        assert_eq!(traded.cells_moving, 2);

        let shrunk = stock(&now, Some(&vec![2.0, 2.0, 3.0]));
        assert_eq!(shrunk.delta_total, -1.0);
        assert!((shrunk.relative_total + 1.0 / 6.0).abs() < 1e-12);
    }

    /// With no earlier boundary there is no window, and a reading must say so rather than
    /// report a difference against zero.
    #[test]
    fn a_stock_with_no_window_behind_it_reports_only_its_total() {
        let s = stock(&[1.0, 2.0], None);
        assert_eq!(s.total, 3.0);
        assert_eq!(s.delta_total, 0.0);
        assert_eq!(s.cell_change_rate, 0.0);
        assert_eq!(s.cells_moving, 0);
    }

    #[test]
    fn the_spread_is_by_nearest_rank_and_its_cv_is_scale_free() {
        let values: Vec<f64> = (1..=10).map(f64::from).collect();
        let (total, median, p10, p90, cv) = spread(&values);
        assert_eq!(total, 55.0);
        assert_eq!(median, 5.0, "nearest rank on ten values");
        assert_eq!(p10, 1.0);
        assert_eq!(p90, 9.0);
        let doubled: Vec<f64> = values.iter().map(|v| v * 2.0).collect();
        let (_, _, _, _, cv2) = spread(&doubled);
        assert!((cv - cv2).abs() < 1e-12, "the coefficient of variation does not carry units");
        assert_eq!(spread(&[]).0, 0.0, "an empty set has no spread rather than a panic");
    }

    #[test]
    fn the_window_tiles_the_ring_exactly() {
        assert_eq!(WINDOW_TICKS % BOUNDARY_EVERY, 0, "the ring can hold a whole window");
        assert_eq!(READING_EVERY, WINDOW_TICKS, "consecutive readings tile, not overlap");
    }
}
