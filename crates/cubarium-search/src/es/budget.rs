//! The matched feasibility experiment: **can this body pay for itself on these patches, and
//! which of the body, the controller or the need to relocate is what stops it?**
//!
//! The training note compared 0.6436 m of served material against a 2.4740 e upkeep bill and
//! concluded that the body's energy budget, not the search, ends the run. Those are different
//! units, and a served bite reaches the battery only after capability, energy density,
//! material assimilation and oxidation have each taken a share
//! (`design/ecology-v1-contract.md` §6.4). The review therefore left body infeasibility and
//! controller failure confounded and asked for one matched comparison
//! (`design/7_Research/ecology-v1-next-review-2026-09-15.md`, finding 1 and next step 1).
//!
//! This command is that comparison: the same ecology, the same twelve layouts, the same
//! horizon, four drivers, and — for the first time — the world's own per-organism ledger
//! ([`cubarium_core::BodyBudget`]) instead of a reconstruction.
//!
//! # The two numbers everything turns on
//!
//! **Energy income** is what one tick of eating is worth to the battery *in the long run*:
//!
//! ```text
//! income = Σ_channels battery_credit  +  η_ox · e_r · Σ_channels reserve_credit
//! ```
//!
//! The first term is the charge a bite delivered straight into `E`. The second is what the
//! material it put into `R` is worth once oxidation converts it: `e_r` energy per unit of
//! reserve, of which `η_ox` reaches the battery and the rest is heat (§7). Both come from the
//! ledger, at the sites the world performs them.
//!
//! **Energy bill** is what the body owed over the same ticks:
//!
//! ```text
//! bill = bill_total + other_energy_paid + growth_energy + reproduction_energy
//! ```
//!
//! `bill_total` is `MotorBill::total_cost` — maintenance, sensing and both halves of the motor
//! charge. The other three are zero for this fixture (no combat, no handling, an adult body,
//! `bud = false`), and are summed anyway so the ratio means the same thing in a world where
//! they are not.
//!
//! **Why not the realised credit.** Counting the oxidation the body actually performed instead
//! of what its intake is worth would make the ratio ≈ 1 for any body in steady state — a body
//! keeps its battery roughly level by definition, right up until it cannot. That number
//! measures nothing. The ratio above is above 1 exactly when intake could pay the bill
//! indefinitely, which is the question.
//!
//! # The window
//!
//! A ratio over a whole life is dominated by its opening stores. Two windowed ratios are
//! reported, both over **2,000 ticks (100 s)** of cumulative income and bill:
//!
//! - `ratio_last_window`: the last 2,000 ticks the body was alive. What it was living on at
//!   the end.
//! - `ratio_best_window`: the largest such ratio over **any** 2,000-tick window of the life.
//!   This is the charitable reading — the body's best sustained stretch — and it is the one
//!   Astra's branch test names ("its best sustained credit/bill ratio stays below 1").
//!
//! An episode shorter than 2,000 ticks reports its whole life as the window and says so.
//!
//! # What this cannot establish
//!
//! Nothing here says the layouts are a fair sample of the world, that 36,000 ticks is long
//! enough for a policy that behaves differently later, or that a fifth driver would not beat
//! all four. It measures four named drivers on twelve frozen layouts under one ecology.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cubarium_core::{BodyBudget, World};
use serde::{Deserialize, Serialize};

use super::episode::{self, Control, Driver, Episode, Limits};
use super::export::PolicyFile;
use super::fixture::{self, Ecology};
use super::tensor;
use crate::evaluate::BUILD_ID;

type Boxed = Box<dyn std::error::Error>;

/// What one measured episode yields: the episode the fixture would have scored, the world's own
/// ledger for its body, and the per-tick cumulative `(income, bill)` trace the windows are
/// differenced from.
pub type Measurement = (Episode, BodyBudget, Vec<(f64, f64)>);

/// The window, in ticks, both sustained ratios are taken over. 2,000 ticks is 100 s at 20 Hz —
/// long enough to average over a walk between patches, short enough that a 36,000-tick life
/// holds seventeen disjoint ones.
pub const WINDOW_TICKS: u64 = 2_000;

/// One `(driver, layout)` result: the episode, the whole ledger, and the derived ratios.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetRow {
    pub driver: String,
    pub layout: String,
    pub layout_hash: u64,
    /// The episode the fixture scores, unchanged: lifetime, terminal stores, distinct cells,
    /// route foliage, and the reconstructed prices the ES protocol already reported.
    pub episode: Episode,
    /// The world's own per-organism ledger for this body, at the end of the episode.
    pub budget: BodyBudget,
    /// `Σ credits − Σ debits − Δ(stores)` for this body, in each currency. Both are zero for a
    /// correctly recorded life and are carried on the row so a reader can check rather than
    /// trust (`cubarium_core::world::budget`).
    pub material_residual: f64,
    pub energy_residual: f64,

    /// Energy income and bill over the whole recorded life (e), as the module docs define them.
    pub income_total: f64,
    pub bill_total: f64,
    /// `income_total / bill_total`, dominated by the opening stores for a short life.
    pub ratio_lifetime: f64,
    /// The last [`WINDOW_TICKS`] the body was alive, and the best such window of its life.
    pub ratio_last_window: f64,
    pub ratio_best_window: f64,
    /// How many ticks each window actually covers: less than [`WINDOW_TICKS`] only for a life
    /// shorter than one window.
    pub window_ticks: u64,

    /// Usable stores `E + e_r·R` at the first and last recorded tick.
    pub usable_start: f64,
    pub usable_end: f64,
}

/// One driver of the four, named for the record.
#[derive(Clone, Debug)]
pub struct NamedDriver {
    pub name: String,
    pub driver: Driver,
}

/// The whole experiment's record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetReport {
    pub build: String,
    pub config: String,
    pub config_hash: String,
    pub horizon_ticks: u64,
    pub window_ticks: u64,
    pub drivers: Vec<String>,
    pub layouts: Vec<String>,
    pub reserve_energy_density: f64,
    pub oxidation_efficiency: f64,
    pub wall_seconds: f64,
    pub rows: Vec<BudgetRow>,
}

/// Run one episode with the world's per-body budget recorder on, and keep the per-tick
/// cumulative income and bill so a window can be taken over any stretch of the life.
///
/// Public so the regression can compare this measurement against the episode's own
/// independently reconstructed columns instead of against a copy of itself.
pub fn measure(
    layout: &fixture::Layout,
    named: &NamedDriver,
    horizon: u64,
    limits: Limits<'_>,
    e_r: f64,
    eta_ox: f64,
) -> Result<Measurement, Boxed> {
    // `(cumulative income, cumulative bill)` after each tick. One entry per simulated tick.
    let trace: Mutex<Vec<(f64, f64)>> = Mutex::new(Vec::with_capacity(horizon as usize));
    // The final ledger, captured at the tick the body was last alive — after the body is
    // removed, the record has been closed and moved out of the live table.
    let last: Mutex<Option<BodyBudget>> = Mutex::new(None);

    let prepare = |w: &mut World| w.record_body_budgets(true);
    let watch = |w: &mut World, _tick: u64| {
        // Take the closed record first: on the tick the body dies, its ledger has already been
        // moved out of the live table with the terminal stores and the cause the world booked.
        // Reading the live table first would silently drop that last tick's bill — which is
        // precisely the bill it could not pay.
        let closed = w.drain_body_budgets().0;
        let record = match closed.into_iter().next_back() {
            Some(b) => Some(b),
            // The fixture holds exactly one body; `Layout::build` asserts it and `bud = false`
            // keeps it that way.
            None => w
                .state
                .organisms
                .iter()
                .next()
                .map(|(id, _)| id)
                .and_then(|id| w.body_budget(id).copied()),
        };
        let Some(b) = record else { return };
        let income = b.battery_credit_total()
            + b.gut_battery_credit
            + eta_ox * e_r * (b.reserve_credit_total() + b.gut_reserve_credit);
        let bill = b.bill_total + b.other_energy_paid + b.growth_energy + b.reproduction_energy;
        trace.lock().expect("trace").push((income, bill));
        *last.lock().expect("last") = Some(b);
    };

    let job = format!("budget/{}/{}", named.name, layout.name);
    let episode = episode::run_prepared(
        layout,
        &named.driver,
        horizon,
        limits,
        &job,
        Some(&prepare),
        Some(&watch),
    )?;
    let trace = trace.into_inner().expect("trace");
    let budget = last.into_inner().expect("last").ok_or_else(|| {
        Boxed::from(format!("{job}: the body was never recorded, so there is no ledger"))
    })?;
    Ok((episode, budget, trace))
}

/// `income/bill` over the trailing `window` ticks of a cumulative trace, and over the best such
/// window anywhere in it. A zero bill over a window yields `f64::INFINITY` when income is
/// positive and `0.0` when it is not, so a divide never silently becomes a number.
pub fn ratios(trace: &[(f64, f64)], window: usize) -> (f64, f64, u64) {
    if trace.is_empty() {
        return (0.0, 0.0, 0);
    }
    let w = window.min(trace.len());
    let ratio = |a: usize, b: usize| {
        let (i0, b0) = if a == 0 { (0.0, 0.0) } else { trace[a - 1] };
        let (i1, b1) = trace[b];
        let (di, db) = (i1 - i0, b1 - b0);
        if db > 0.0 {
            di / db
        } else if di > 0.0 {
            f64::INFINITY
        } else {
            0.0
        }
    };
    let last = ratio(trace.len() - w, trace.len() - 1);
    let mut best = f64::NEG_INFINITY;
    for end in (w - 1)..trace.len() {
        best = best.max(ratio(end + 1 - w, end));
    }
    (last, best, w as u64)
}

/// Build the four drivers the experiment compares.
fn drivers(policy_file: &Path, initial_seed: u64, eco: &Ecology) -> Result<Vec<NamedDriver>, Boxed> {
    let file: PolicyFile = serde_json::from_str(&fs::read_to_string(policy_file)?)?;
    // A policy from another ecology would be answering a different question in a world it
    // never saw: refuse it by name, exactly as `es-evaluate` does.
    file.check_ecology(eco)?;
    let trained = file.policy()?;
    let initial = tensor::policy(&tensor::initial_center(initial_seed))?;
    Ok(vec![
        NamedDriver {
            name: "stationary-grazing".into(),
            driver: Driver::Control(Control::StationaryGrazing),
        },
        NamedDriver {
            name: "mobile-script".into(),
            driver: Driver::Control(Control::MobileScript),
        },
        NamedDriver {
            name: format!("initial-center-{initial_seed}"),
            driver: Driver::Policy(Box::new(initial)),
        },
        NamedDriver {
            name: format!("generation-{}", file.generation),
            driver: Driver::Policy(Box::new(trained)),
        },
    ])
}

/// Run the matched feasibility experiment and write its record.
#[allow(clippy::too_many_arguments)]
pub fn run(
    policy_file: PathBuf,
    config: PathBuf,
    horizon: u64,
    initial_seed: u64,
    workers: usize,
    wall_seconds: u64,
    out: PathBuf,
) -> Result<(), Boxed> {
    let eco = Ecology::load(&config)?;
    let drivers = drivers(&policy_file, initial_seed, &eco)?;
    let mut layouts = fixture::training_layouts_on(&eco);
    layouts.extend(fixture::holdout_layouts_on(&eco));
    let e_r = eco.base.organism.reserve_energy_density;
    let eta_ox = eco.base.organism.oxidation_efficiency;

    println!("# matched feasibility experiment on {} (hash {})", eco.label, eco.hex());
    println!("# build {BUILD_ID}, {horizon} ticks per episode, window {WINDOW_TICKS} ticks");
    println!("# {} drivers x {} layouts, {workers} workers", drivers.len(), layouts.len());
    println!("# income = battery credit + {eta_ox} * {e_r} * reserve credit; bill = the whole MotorBill");

    let jobs: Vec<(usize, usize)> = (0..drivers.len())
        .flat_map(|d| (0..layouts.len()).map(move |l| (d, l)))
        .collect();
    let cursor = AtomicUsize::new(0);
    let rows: Mutex<Vec<BudgetRow>> = Mutex::new(Vec::new());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let limits = Limits::until(&cancel, started + Duration::from_secs(wall_seconds));

    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    let Some(&(d, l)) = jobs.get(i) else { return };
                    let named = &drivers[d];
                    let layout = &layouts[l];
                    match measure(layout, named, horizon, limits, e_r, eta_ox) {
                        Ok((episode, budget, trace)) => {
                            let (last, best, window) = ratios(&trace, WINDOW_TICKS as usize);
                            let (income_total, bill_total) =
                                trace.last().copied().unwrap_or((0.0, 0.0));
                            rows.lock().expect("rows").push(BudgetRow {
                                driver: named.name.clone(),
                                layout: layout.name.clone(),
                                layout_hash: layout.hash(&layout.config()),
                                material_residual: budget.material_residual(),
                                energy_residual: budget.energy_residual(),
                                usable_start: budget.usable_start(e_r),
                                usable_end: budget.usable_end(e_r),
                                income_total,
                                bill_total,
                                ratio_lifetime: if bill_total > 0.0 {
                                    income_total / bill_total
                                } else {
                                    0.0
                                },
                                ratio_last_window: last,
                                ratio_best_window: best,
                                window_ticks: window,
                                episode,
                                budget,
                            });
                        }
                        Err(e) => failures
                            .lock()
                            .expect("failures")
                            .push(format!("{}/{}: {e}", named.name, layout.name)),
                    }
                }
            });
        }
    });

    let failures = failures.into_inner().expect("failures");
    if !failures.is_empty() {
        for f in &failures {
            println!("FAILED {f}");
        }
        return Err(format!("{} of {} episodes did not produce a row", failures.len(), jobs.len())
            .into());
    }
    let mut rows = rows.into_inner().expect("rows");
    rows.sort_by(|a, b| a.driver.cmp(&b.driver).then(a.layout.cmp(&b.layout)));

    println!();
    println!(
        "{:<22} {:<14} {:>6} {:>5} {:>8} {:>8} {:>8} {:>7} {:>6} {:>8} {:>8}",
        "driver",
        "layout",
        "ticks",
        "alive",
        "ratio/w",
        "best/w",
        "life",
        "served",
        "cells",
        "P start",
        "P end",
    );
    for r in &rows {
        println!(
            "{:<22} {:<14} {:>6} {:>5} {:>8.3} {:>8.3} {:>8.3} {:>7.3} {:>6} {:>8.3} {:>8.3}",
            r.driver,
            r.layout,
            r.episode.ticks,
            r.episode.alive,
            r.ratio_last_window,
            r.ratio_best_window,
            r.ratio_lifetime,
            r.budget.served_total(),
            r.episode.distinct_cells,
            r.episode.route_p_start,
            r.episode.route_p_end,
        );
    }

    println!();
    println!("{:<22} {:>8} {:>9} {:>9} {:>9} {:>11}", "driver", "survived", "mean tick", "mean w", "best w", "max |resid|");
    for d in &drivers {
        let mine: Vec<&BudgetRow> = rows.iter().filter(|r| r.driver == d.name).collect();
        if mine.is_empty() {
            continue;
        }
        let n = mine.len() as f64;
        let alive = mine.iter().filter(|r| r.episode.alive).count();
        let mean_ticks = mine.iter().map(|r| r.episode.ticks as f64).sum::<f64>() / n;
        let mean_w = mine.iter().map(|r| r.ratio_last_window).sum::<f64>() / n;
        let best_w = mine.iter().map(|r| r.ratio_best_window).fold(f64::MIN, f64::max);
        let resid = mine
            .iter()
            .map(|r| r.material_residual.abs().max(r.energy_residual.abs()))
            .fold(0.0, f64::max);
        println!(
            "{:<22} {alive:>4} / {:<3} {mean_ticks:>9.0} {mean_w:>9.3} {best_w:>9.3} {resid:>11.2e}",
            d.name,
            mine.len()
        );
    }

    let report = BudgetReport {
        build: BUILD_ID.to_string(),
        config: eco.label.clone(),
        config_hash: eco.hex(),
        horizon_ticks: horizon,
        window_ticks: WINDOW_TICKS,
        drivers: drivers.iter().map(|d| d.name.clone()).collect(),
        layouts: layouts.iter().map(|l| l.name.clone()).collect(),
        reserve_energy_density: e_r,
        oxidation_efficiency: eta_ox,
        wall_seconds: started.elapsed().as_secs_f64(),
        rows,
    };
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, serde_json::to_string(&report)?)?;
    println!();
    println!("wall {:.1} s of {wall_seconds}; wrote {}", report.wall_seconds, out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window arithmetic, on a trace whose answer is known by construction: income rises
    /// by 2 per tick and the bill by 1, except for a lean stretch where income does not rise
    /// at all. The trailing window must see the lean stretch and the best window must not.
    #[test]
    fn the_windowed_ratio_is_the_difference_of_the_cumulative_trace() {
        let mut trace = Vec::new();
        let (mut income, mut bill) = (0.0, 0.0);
        for tick in 0..30 {
            income += if tick >= 20 { 0.0 } else { 2.0 };
            bill += 1.0;
            trace.push((income, bill));
        }
        let (last, best, window) = ratios(&trace, 10);
        assert_eq!(window, 10);
        assert!((last - 0.0).abs() < 1e-12, "trailing window {last}");
        assert!((best - 2.0).abs() < 1e-12, "best window {best}");
    }

    /// A life shorter than one window reports the whole life, and says so through
    /// `window_ticks`.
    #[test]
    fn a_life_shorter_than_the_window_reports_the_whole_life() {
        let trace: Vec<(f64, f64)> = (1..=5).map(|i| (3.0 * i as f64, 1.0 * i as f64)).collect();
        let (last, best, window) = ratios(&trace, 2_000);
        assert_eq!(window, 5);
        assert!((last - 3.0).abs() < 1e-12);
        assert!((best - 3.0).abs() < 1e-12);
    }

    /// A window that owed nothing is not silently a number: positive income over a zero bill
    /// is infinite, and no income over no bill is zero.
    #[test]
    fn a_zero_bill_window_is_infinite_or_zero_never_a_quiet_nan() {
        let free = vec![(0.0, 0.0), (1.0, 0.0)];
        let (last, _, _) = ratios(&free, 2);
        assert!(last.is_infinite() && last > 0.0);
        let idle = vec![(0.0, 0.0), (0.0, 0.0)];
        let (last, _, _) = ratios(&idle, 2);
        assert_eq!(last, 0.0);
    }
}
