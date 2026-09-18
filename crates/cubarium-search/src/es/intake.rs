//! **Why generation 9 does not eat**: the per-tick intake diagnostic
//! (`design/handoffs/ecology-v1-intake-opus-2026-09-16.md`).
//!
//! The feasibility experiment ([`super::budget`]) localised the trained controller's failure to
//! *intake* rather than travel: generation 9 visits five times as many cells as the surviving
//! mobile script, travels 1.6 times as far, and takes in fifteen times less material
//! (`design/7_Research/ecology-v1-budget-2026-09-16.md`). It could not say why, because a life
//! total cannot tell a shut mouth from an unfed one.
//!
//! Three explanations were left standing, and they imply three different next moves:
//!
//! - **(a) the mouths are off when the body is on food** — an action or score problem, and the
//!   fix is in what the search rewards or in how the head is decoded;
//! - **(b) the body is rarely on edible food at all** — an observation or navigation problem,
//!   and the fix is in what the policy is shown;
//! - **(c) the mouths are on, the food is there, and the served bite is clamped** — an adapter
//!   or settlement problem, and the fix is in the clamp that binds.
//!
//! The world's own per-tick trace ([`cubarium_core::IntakeTick`]) records all three at the site
//! that decides them. This module runs the three drivers over the twelve `fast-leaf` layouts
//! with that trace on, and reduces each episode to the counts that separate the branches.
//!
//! # What each column means
//!
//! - `on_food`: ticks the body stood on a cell holding **any** of the four stocks at or above
//!   the world's own `drives.feed_min`. This is the threshold the legacy controller and the
//!   disclosed mobile script both use to decide a cell is worth cropping; it gates nothing in
//!   the settlement, which serves whatever is there.
//! - `open_on_food`: of those, the ticks on which the mouth that serves one of those stocks was
//!   open — above the adapter's own `DEADBAND` (0.05), and above a half (0.5).
//! - `open_off_food`: ticks with a mouth open above the deadband and no stock above the
//!   threshold anywhere in the cell. A mouth spent on bare ground.
//! - `served / requested`: over the ticks where both are positive, `Σ served / Σ requested` —
//!   how much of what the mouth asked the cell for it actually got.
//! - the limiting-term histogram: [`cubarium_core::IntakeLimit`], per mouth, over every traced
//!   tick and again over just the ticks where that mouth's own food was above the threshold.
//!
//! # What it cannot establish
//!
//! Twelve frozen single-body layouts under one ecology, three named drivers, one horizon.
//! It says what happened to these bodies on these patches. It does not say what a different
//! score, a different observation or a fifth driver would do, and it measures nothing about a
//! whole world's foraging.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cubarium_core::{
    BodyBudget, CARRION, CHANNELS, FOLIAGE, FRUIT, IntakeLimit, IntakeTick, LITTER, MOUTH_GRAZE,
    MOUTH_NAMES, MOUTHS, World,
};
use serde::{Deserialize, Serialize};

use super::budget::NamedDriver;
use super::episode::{self, Control, Driver, Episode, Limits};
use super::export::PolicyFile;
use super::fixture::{self, Ecology};
use super::tensor;
use crate::evaluate::BUILD_ID;

type Boxed = Box<dyn std::error::Error>;

/// The adapter's own deadband, and the level a "clearly open" mouth has to clear. Both are read
/// from the action contract rather than chosen here: `DEADBAND` is the floor below which the
/// adapter zeroes a channel outright, so an effort above it is a mouth the world will act on.
pub const OPEN: f64 = cubarium_core::neural::action::DEADBAND;
/// The second, stricter floor the brief asks for.
pub const WIDE_OPEN: f64 = 0.5;

/// How many ticks one aggregation window covers. 100 ticks is 5 s: short enough that a walk
/// between patches is several windows, long enough that a window is not one bite.
pub const WINDOW_TICKS: u64 = 100;

/// The six limiting terms, in a fixed order, so a histogram reads the same everywhere.
pub const LIMITS: [IntakeLimit; 6] = [
    IntakeLimit::CapabilityZero,
    IntakeLimit::EffortZero,
    IntakeLimit::StockBelowThreshold,
    IntakeLimit::ReserveHeadroom,
    IntakeLimit::StockShare,
    IntakeLimit::MouthRate,
];
/// Their short names, in the same order.
pub const LIMIT_NAMES: [&str; 6] = [
    "capability",
    "effort_zero",
    "below_threshold",
    "reserve_room",
    "stock_share",
    "mouth_rate",
];

fn limit_index(l: IntakeLimit) -> usize {
    LIMITS
        .iter()
        .position(|x| *x == l)
        .expect("every term is in the table")
}

/// One 100-tick window of a life, for a coarse time series that costs nothing to keep.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Window {
    pub start_tick: u64,
    pub ticks: u64,
    pub on_food: u64,
    pub open_on_food: u64,
    pub open_off_food: u64,
    pub served: f64,
    pub requested: f64,
    pub income: f64,
    pub bill: f64,
}

/// One `(driver, layout)` episode, reduced to the counts that separate the three branches.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntakeRow {
    pub driver: String,
    pub layout: String,
    pub layout_hash: u64,
    /// The episode the fixture would have scored, unchanged.
    pub episode: Episode,
    /// The world's own per-organism ledger at the end, so the trace can be checked against it.
    pub budget: BodyBudget,
    /// Ticks the trace actually recorded — one per tick the body was alive to feed.
    pub ticks: u64,

    /// Ticks on a cell with any stock at or above `feed_min`, and per channel.
    pub on_food: u64,
    pub on_food_by_channel: [u64; CHANNELS],
    /// Of the `on_food` ticks, those where the mouth serving one of the above-threshold stocks
    /// was open above `OPEN` / `WIDE_OPEN`.
    pub open_on_food: u64,
    pub wide_open_on_food: u64,
    /// Ticks with a mouth open above `OPEN` and nothing above the threshold in the cell.
    pub open_off_food: u64,
    /// Ticks with any mouth open above `OPEN` / `WIDE_OPEN`, wherever the body stood.
    pub open_ticks: u64,
    pub wide_open_ticks: u64,
    /// Per-mouth mean effort over every traced tick, and over the ticks that mouth's own food
    /// was above the threshold.
    pub mean_effort: [f64; MOUTHS],
    pub mean_effort_on_its_food: [f64; MOUTHS],

    /// `Σ requested` and `Σ served` per mouth, and the two restricted to the ticks where both
    /// were positive — the ratio the brief asks for.
    pub requested_total: [f64; MOUTHS],
    pub served_total: [f64; CHANNELS],
    pub requested_when_both: f64,
    pub served_when_both: f64,
    pub ticks_both_positive: u64,
    /// Ticks that asked for something and were served nothing at all.
    pub ticks_asked_got_nothing: u64,

    /// The limiting term, per mouth, over every traced tick and over the ticks that mouth's own
    /// food was above the threshold.
    pub limits: [[u64; 6]; MOUTHS],
    pub limits_on_its_food: [[u64; 6]; MOUTHS],

    /// The energy the intake is worth and the energy the tick cost, on the same definitions the
    /// feasibility experiment used: `income = Σ battery_credit + η_ox·e_r·Σ reserve_credit`.
    pub income_total: f64,
    pub bill_total: f64,
    pub credit_over_bill: f64,
    /// Charge the `E_max` clamp sent to heat instead of the battery (e).
    pub battery_rejected: f64,

    /// The mean type-II factor the grazing mouth saw on the ticks it stood on above-threshold
    /// foliage: how much of the mouth-rate ceiling the cell's own thinness cost.
    pub mean_graze_saturation_on_food: f64,
    /// The mean above-threshold foliage stock under the body, over the same ticks.
    pub mean_foliage_on_food: f64,

    /// The coarse time series.
    pub windows: Vec<Window>,
}

impl IntakeRow {
    fn fraction(a: u64, b: u64) -> f64 {
        if b == 0 { 0.0 } else { a as f64 / b as f64 }
    }

    /// Fraction of traced ticks spent on a cell with something edible in it.
    pub fn on_food_fraction(&self) -> f64 {
        IntakeRow::fraction(self.on_food, self.ticks)
    }

    /// Of the ticks on food, the fraction with the matching mouth open above `OPEN`.
    pub fn open_given_food(&self) -> f64 {
        IntakeRow::fraction(self.open_on_food, self.on_food)
    }

    /// Of the ticks on food, the fraction with the matching mouth open above `WIDE_OPEN`.
    pub fn wide_open_given_food(&self) -> f64 {
        IntakeRow::fraction(self.wide_open_on_food, self.on_food)
    }

    /// Fraction of traced ticks that spent an open mouth on bare ground.
    pub fn open_off_food_fraction(&self) -> f64 {
        IntakeRow::fraction(self.open_off_food, self.ticks)
    }

    /// `Σ served / Σ requested` over the ticks where both were positive.
    pub fn served_over_requested(&self) -> f64 {
        if self.requested_when_both > 0.0 {
            self.served_when_both / self.requested_when_both
        } else {
            0.0
        }
    }
}

/// The accumulator the per-tick hook folds rows into. Never allocates per tick beyond the
/// window it is filling.
#[derive(Default)]
struct Fold {
    ticks: u64,
    on_food: u64,
    on_food_by_channel: [u64; CHANNELS],
    open_on_food: u64,
    wide_open_on_food: u64,
    open_off_food: u64,
    open_ticks: u64,
    wide_open_ticks: u64,
    effort_sum: [f64; MOUTHS],
    effort_sum_on_its_food: [f64; MOUTHS],
    its_food_ticks: [u64; MOUTHS],
    requested_total: [f64; MOUTHS],
    served_total: [f64; CHANNELS],
    requested_when_both: f64,
    served_when_both: f64,
    ticks_both_positive: u64,
    ticks_asked_got_nothing: u64,
    limits: [[u64; 6]; MOUTHS],
    limits_on_its_food: [[u64; 6]; MOUTHS],
    income_total: f64,
    bill_total: f64,
    battery_rejected: f64,
    graze_saturation_sum: f64,
    foliage_sum: f64,
    graze_on_food_ticks: u64,
    windows: Vec<Window>,
    /// Kept only when this episode is one of the two the per-tick file is written for.
    keep: Option<Vec<IntakeTick>>,
}

impl Fold {
    fn push(&mut self, row: &IntakeTick, e_r: f64, eta_ox: f64) {
        self.ticks += 1;
        let on_food = row.on_food();
        self.on_food += u64::from(on_food);
        for c in 0..CHANNELS {
            self.on_food_by_channel[c] += u64::from(row.above_threshold[c]);
        }
        // "The matching effort": the mouth that serves a stock this cell actually holds.
        let matched =
            |floor: f64| (0..CHANNELS).any(|c| row.above_threshold[c] && row.mouth_open(c, floor));
        if on_food {
            self.open_on_food += u64::from(matched(OPEN));
            self.wide_open_on_food += u64::from(matched(WIDE_OPEN));
        }
        let any_open = |floor: f64| row.effort.iter().any(|e| *e > floor);
        self.open_ticks += u64::from(any_open(OPEN));
        self.wide_open_ticks += u64::from(any_open(WIDE_OPEN));
        if !on_food && any_open(OPEN) {
            self.open_off_food += 1;
        }

        let mut its_food = [false; MOUTHS];
        for c in 0..CHANNELS {
            if row.above_threshold[c] {
                its_food[IntakeTick::mouth_of(c)] = true;
            }
        }
        for (m, on_its_food) in its_food.into_iter().enumerate() {
            self.effort_sum[m] += row.effort[m];
            self.requested_total[m] += row.requested[m];
            self.limits[m][limit_index(row.limit[m])] += 1;
            if on_its_food {
                self.its_food_ticks[m] += 1;
                self.effort_sum_on_its_food[m] += row.effort[m];
                self.limits_on_its_food[m][limit_index(row.limit[m])] += 1;
            }
        }
        if its_food[MOUTH_GRAZE] {
            self.graze_on_food_ticks += 1;
            self.graze_saturation_sum += row.saturation[MOUTH_GRAZE];
            self.foliage_sum += row.stock[FOLIAGE];
        }

        let requested: f64 = row.requested.iter().sum();
        let served: f64 = row.served.iter().sum();
        for c in 0..CHANNELS {
            self.served_total[c] += row.served[c];
            self.battery_rejected += row.battery_rejected[c];
        }
        if requested > 0.0 && served > 0.0 {
            self.ticks_both_positive += 1;
            self.requested_when_both += requested;
            self.served_when_both += served;
        } else if requested > 0.0 {
            self.ticks_asked_got_nothing += 1;
        }

        let income: f64 = (0..CHANNELS)
            .map(|c| row.battery_credit[c] + eta_ox * e_r * row.reserve_credit[c])
            .sum();
        self.income_total += income;
        self.bill_total += row.bill_total;

        let start = row.tick - row.tick % WINDOW_TICKS;
        if self.windows.last().is_none_or(|w| w.start_tick != start) {
            self.windows.push(Window {
                start_tick: start,
                ..Window::default()
            });
        }
        let w = self.windows.last_mut().expect("just pushed");
        w.ticks += 1;
        w.on_food += u64::from(on_food);
        w.open_on_food += u64::from(on_food && matched(OPEN));
        w.open_off_food += u64::from(!on_food && any_open(OPEN));
        w.served += served;
        w.requested += requested;
        w.income += income;
        w.bill += row.bill_total;

        if let Some(keep) = self.keep.as_mut() {
            keep.push(*row);
        }
    }

    fn finish(
        self,
        driver: &str,
        layout: &fixture::Layout,
        layout_hash: u64,
        episode: Episode,
        budget: BodyBudget,
    ) -> (IntakeRow, Option<Vec<IntakeTick>>) {
        let mean = |sum: f64, n: u64| if n == 0 { 0.0 } else { sum / n as f64 };
        let row = IntakeRow {
            driver: driver.to_string(),
            layout: layout.name.clone(),
            layout_hash,
            episode,
            budget,
            ticks: self.ticks,
            on_food: self.on_food,
            on_food_by_channel: self.on_food_by_channel,
            open_on_food: self.open_on_food,
            wide_open_on_food: self.wide_open_on_food,
            open_off_food: self.open_off_food,
            open_ticks: self.open_ticks,
            wide_open_ticks: self.wide_open_ticks,
            mean_effort: std::array::from_fn(|m| mean(self.effort_sum[m], self.ticks)),
            mean_effort_on_its_food: std::array::from_fn(|m| {
                mean(self.effort_sum_on_its_food[m], self.its_food_ticks[m])
            }),
            requested_total: self.requested_total,
            served_total: self.served_total,
            requested_when_both: self.requested_when_both,
            served_when_both: self.served_when_both,
            ticks_both_positive: self.ticks_both_positive,
            ticks_asked_got_nothing: self.ticks_asked_got_nothing,
            limits: self.limits,
            limits_on_its_food: self.limits_on_its_food,
            income_total: self.income_total,
            bill_total: self.bill_total,
            credit_over_bill: if self.bill_total > 0.0 {
                self.income_total / self.bill_total
            } else {
                0.0
            },
            battery_rejected: self.battery_rejected,
            mean_graze_saturation_on_food: mean(
                self.graze_saturation_sum,
                self.graze_on_food_ticks,
            ),
            mean_foliage_on_food: mean(self.foliage_sum, self.graze_on_food_ticks),
            windows: self.windows,
        };
        (row, self.keep)
    }
}

/// Run one episode with the world's per-tick intake trace and per-body ledger on, and reduce it
/// as it goes.
///
/// Public so the regression can compare a traced episode against the untraced one the trainer
/// would have scored, rather than against a copy of itself.
pub fn measure(
    layout: &fixture::Layout,
    named: &NamedDriver,
    horizon: u64,
    limits: Limits<'_>,
    e_r: f64,
    eta_ox: f64,
    keep_per_tick: bool,
) -> Result<(Episode, IntakeRow, Option<Vec<IntakeTick>>), Boxed> {
    let fold: Mutex<Fold> = Mutex::new(Fold {
        keep: keep_per_tick.then(|| Vec::with_capacity(horizon as usize)),
        ..Fold::default()
    });
    let last: Mutex<Option<BodyBudget>> = Mutex::new(None);

    let prepare = |w: &mut World| {
        // The fixture holds exactly one body; `Layout::build` asserts it and `bud = false`
        // keeps it that way, so "the body" is unambiguous here.
        let id = w.state.organisms.iter().next().map(|(id, _)| id);
        w.record_body_budgets(true);
        w.trace_intake(id);
    };
    let watch = |w: &mut World, _tick: u64| {
        let (rows, dropped) = w.drain_intake_trace();
        assert_eq!(
            dropped, 0,
            "a per-tick drain can never overflow the trace cap"
        );
        let mut fold = fold.lock().expect("fold");
        for row in &rows {
            fold.push(row, e_r, eta_ox);
        }
        // Same order as the feasibility experiment: the closed record first, because on the
        // tick a body dies its ledger has already left the live table carrying the bill it
        // could not pay.
        let closed = w.drain_body_budgets().0;
        let record = match closed.into_iter().next_back() {
            Some(b) => Some(b),
            None => w
                .state
                .organisms
                .iter()
                .next()
                .map(|(id, _)| id)
                .and_then(|id| w.body_budget(id).copied()),
        };
        if let Some(b) = record {
            *last.lock().expect("last") = Some(b);
        }
    };

    let job = format!("intake/{}/{}", named.name, layout.name);
    let episode = episode::run_prepared(
        layout,
        &named.driver,
        horizon,
        limits,
        &job,
        Some(&prepare),
        Some(&watch),
    )?;
    let budget = last.into_inner().expect("last").ok_or_else(|| {
        Boxed::from(format!(
            "{job}: the body was never recorded, so there is no ledger"
        ))
    })?;
    let layout_hash = layout.hash(&layout.config());
    let (row, keep) = fold.into_inner().expect("fold").finish(
        &named.name,
        layout,
        layout_hash,
        episode.clone(),
        budget,
    );
    Ok((episode, row, keep))
}

/// The whole experiment's record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntakeReport {
    pub build: String,
    pub config: String,
    pub config_hash: String,
    pub horizon_ticks: u64,
    pub window_ticks: u64,
    pub open_floor: f64,
    pub wide_open_floor: f64,
    pub feed_threshold: f64,
    pub reserve_energy_density: f64,
    pub oxidation_efficiency: f64,
    pub mouth_names: [String; MOUTHS],
    pub limit_names: [String; 6],
    pub drivers: Vec<String>,
    pub layouts: Vec<String>,
    /// The layouts whose per-tick rows were written out in full.
    pub per_tick_layouts: Vec<String>,
    pub wall_seconds: f64,
    pub rows: Vec<IntakeRow>,
}

/// The three drivers this experiment compares: the disclosed mobile control that survives, the
/// untrained centre, and generation 9.
fn drivers(
    policy_file: &Path,
    initial_seed: u64,
    eco: &Ecology,
) -> Result<Vec<NamedDriver>, Boxed> {
    let file: PolicyFile = serde_json::from_str(&fs::read_to_string(policy_file)?)?;
    // A policy from another ecology would be answering a different question in a world it never
    // saw: refuse it by name, exactly as `es-evaluate` and `es-budget` do.
    file.check_ecology(eco)?;
    let trained = file.policy()?;
    let initial = tensor::policy(&tensor::initial_center(initial_seed))?;
    Ok(vec![
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

/// One per-tick row as a CSV line. Compact on purpose: a 36,000-tick episode is about 4 MiB.
fn csv_line(row: &IntakeTick, out: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(out, "{},{}", row.tick, row.cell);
    for c in 0..CHANNELS {
        let _ = write!(out, ",{:.6}", row.stock[c]);
    }
    let _ = write!(
        out,
        ",{}",
        (0..CHANNELS)
            .map(|c| u8::from(row.above_threshold[c]) << c)
            .sum::<u8>()
    );
    for m in 0..MOUTHS {
        let _ = write!(out, ",{:.6}", row.effort[m]);
    }
    for m in 0..MOUTHS {
        let _ = write!(out, ",{:.3e}", row.requested[m]);
    }
    for c in 0..CHANNELS {
        let _ = write!(out, ",{:.3e}", row.served[c]);
    }
    for m in 0..MOUTHS {
        let _ = write!(out, ",{}", LIMIT_NAMES[limit_index(row.limit[m])]);
    }
    let _ = writeln!(
        out,
        ",{:.6},{:.6},{:.3e},{:.3e}",
        row.reserve, row.energy, row.bill_total, row.reserve_headroom
    );
}

const CSV_HEADER: &str = "tick,cell,p,f,d_eff,c_eff,above_mask,e_graze,e_fruit,e_scav,\
req_graze,req_fruit,req_scav,served_p,served_f,served_d,served_c,\
limit_graze,limit_fruit,limit_scav,reserve,energy,bill,reserve_room\n";

/// Run the intake diagnostic and write its record.
#[allow(clippy::too_many_arguments)]
pub fn run(
    policy_file: PathBuf,
    config: PathBuf,
    horizon: u64,
    initial_seed: u64,
    workers: usize,
    wall_seconds: u64,
    out: PathBuf,
    per_tick_layouts: &[String],
) -> Result<(), Boxed> {
    let eco = Ecology::load(&config)?;
    let drivers = drivers(&policy_file, initial_seed, &eco)?;
    let mut layouts = fixture::training_layouts_on(&eco);
    layouts.extend(fixture::holdout_layouts_on(&eco));
    let e_r = eco.base.organism.reserve_energy_density;
    let eta_ox = eco.base.organism.oxidation_efficiency;
    let feed_threshold = eco.base.drives.feed_min;

    println!(
        "# the per-tick intake diagnostic on {} (hash {})",
        eco.label,
        eco.hex()
    );
    println!("# build {BUILD_ID}, {horizon} ticks per episode, window {WINDOW_TICKS} ticks");
    println!(
        "# {} drivers x {} layouts, {workers} workers",
        drivers.len(),
        layouts.len()
    );
    println!("# a mouth is open above {OPEN}, wide open above {WIDE_OPEN}");
    println!("# a stock is food at or above drives.feed_min = {feed_threshold}");

    let per_tick_dir = out.parent().unwrap_or(Path::new(".")).join("per-tick");
    let jobs: Vec<(usize, usize)> = (0..drivers.len())
        .flat_map(|d| (0..layouts.len()).map(move |l| (d, l)))
        .collect();
    let cursor = AtomicUsize::new(0);
    let rows: Mutex<Vec<IntakeRow>> = Mutex::new(Vec::new());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let limits = Limits::until(&cancel, started + Duration::from_secs(wall_seconds));

    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let next = cursor.fetch_add(1, Ordering::Relaxed);
                    let Some(&(d, l)) = jobs.get(next) else {
                        return;
                    };
                    let (named, layout) = (&drivers[d], &layouts[l]);
                    let keep = per_tick_layouts.contains(&layout.name);
                    match measure(layout, named, horizon, limits, e_r, eta_ox, keep) {
                        Ok((_, row, kept)) => {
                            if let Some(kept) = kept {
                                let mut text = String::with_capacity(kept.len() * 160);
                                text.push_str(CSV_HEADER);
                                for r in &kept {
                                    csv_line(r, &mut text);
                                }
                                let path = per_tick_dir
                                    .join(format!("{}--{}.csv", named.name, layout.name));
                                if let Err(e) = fs::create_dir_all(&per_tick_dir)
                                    .and_then(|()| fs::write(&path, text))
                                {
                                    failures
                                        .lock()
                                        .expect("failures")
                                        .push(format!("{}: {e}", path.display()));
                                }
                            }
                            rows.lock().expect("rows").push(row);
                        }
                        Err(e) => failures.lock().expect("failures").push(e.to_string()),
                    }
                }
            });
        }
    });

    let failures = failures.into_inner().expect("failures");
    if !failures.is_empty() {
        return Err(Boxed::from(format!(
            "{} episodes failed:\n{}",
            failures.len(),
            failures.join("\n")
        )));
    }
    let mut rows = rows.into_inner().expect("rows");
    rows.sort_by(|a, b| (&a.driver, &a.layout).cmp(&(&b.driver, &b.layout)));

    let report = IntakeReport {
        build: BUILD_ID.to_string(),
        config: config.display().to_string(),
        config_hash: eco.hex(),
        horizon_ticks: horizon,
        window_ticks: WINDOW_TICKS,
        open_floor: OPEN,
        wide_open_floor: WIDE_OPEN,
        feed_threshold,
        reserve_energy_density: e_r,
        oxidation_efficiency: eta_ox,
        mouth_names: MOUTH_NAMES.map(String::from),
        limit_names: LIMIT_NAMES.map(String::from),
        drivers: drivers.iter().map(|d| d.name.clone()).collect(),
        layouts: layouts.iter().map(|l| l.name.clone()).collect(),
        per_tick_layouts: per_tick_layouts.to_vec(),
        wall_seconds: started.elapsed().as_secs_f64(),
        rows,
    };

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(&out)?;
    file.write_all(serde_json::to_string_pretty(&report)?.as_bytes())?;
    println!(
        "# wrote {} ({} rows, {:.1} s)",
        out.display(),
        report.rows.len(),
        report.wall_seconds
    );
    summarise(&report);
    Ok(())
}

/// Print the per-driver table the note carries, as medians over the twelve layouts.
pub fn summarise(report: &IntakeReport) {
    fn median(mut v: Vec<f64>) -> f64 {
        if v.is_empty() {
            return f64::NAN;
        }
        v.sort_by(f64::total_cmp);
        let n = v.len();
        if n % 2 == 1 {
            v[n / 2]
        } else {
            0.5 * (v[n / 2 - 1] + v[n / 2])
        }
    }
    println!();
    println!(
        "{:<24} {:>7} {:>8} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "driver",
        "ticks",
        "on_food",
        "open|food",
        ">0.5|food",
        "open_bare",
        "served/req",
        "cred/bill"
    );
    for driver in &report.drivers {
        let rows: Vec<&IntakeRow> = report.rows.iter().filter(|r| r.driver == *driver).collect();
        println!(
            "{:<24} {:>7.0} {:>8.3} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9.3}",
            driver,
            median(rows.iter().map(|r| r.ticks as f64).collect()),
            median(rows.iter().map(|r| r.on_food_fraction()).collect()),
            median(rows.iter().map(|r| r.open_given_food()).collect()),
            median(rows.iter().map(|r| r.wide_open_given_food()).collect()),
            median(rows.iter().map(|r| r.open_off_food_fraction()).collect()),
            median(rows.iter().map(|r| r.served_over_requested()).collect()),
            median(rows.iter().map(|r| r.credit_over_bill).collect()),
        );
    }
    println!();
    println!("limiting term for the grazing mouth, summed over the twelve layouts:");
    print!("{:<24}", "driver");
    for name in LIMIT_NAMES {
        print!(" {name:>16}");
    }
    println!();
    for driver in &report.drivers {
        let rows: Vec<&IntakeRow> = report.rows.iter().filter(|r| r.driver == *driver).collect();
        let total: u64 = rows
            .iter()
            .map(|r| r.limits[MOUTH_GRAZE].iter().sum::<u64>())
            .sum();
        print!("{driver:<24}");
        for t in 0..6 {
            let n: u64 = rows.iter().map(|r| r.limits[MOUTH_GRAZE][t]).sum();
            print!(
                " {:>16}",
                format!("{n} ({:.1}%)", 100.0 * n as f64 / total.max(1) as f64)
            );
        }
        println!();
    }
    println!();
    println!("the same, restricted to ticks the grazing mouth's own food was above threshold:");
    print!("{:<24}", "driver");
    for name in LIMIT_NAMES {
        print!(" {name:>16}");
    }
    println!();
    for driver in &report.drivers {
        let rows: Vec<&IntakeRow> = report.rows.iter().filter(|r| r.driver == *driver).collect();
        let total: u64 = rows
            .iter()
            .map(|r| r.limits_on_its_food[MOUTH_GRAZE].iter().sum::<u64>())
            .sum();
        print!("{driver:<24}");
        for t in 0..6 {
            let n: u64 = rows
                .iter()
                .map(|r| r.limits_on_its_food[MOUTH_GRAZE][t])
                .sum();
            print!(
                " {:>16}",
                format!("{n} ({:.1}%)", 100.0 * n as f64 / total.max(1) as f64)
            );
        }
        println!();
    }
    println!();
    println!("served material by channel, summed over the twelve layouts (m):");
    println!(
        "{:<24} {:>10} {:>10} {:>10} {:>10}",
        "driver", "foliage", "fruit", "litter", "carrion"
    );
    for driver in &report.drivers {
        let rows: Vec<&IntakeRow> = report.rows.iter().filter(|r| r.driver == *driver).collect();
        let s = |c: usize| rows.iter().map(|r| r.served_total[c]).sum::<f64>();
        println!(
            "{:<24} {:>10.4} {:>10.4} {:>10.4} {:>10.4}",
            driver,
            s(FOLIAGE),
            s(FRUIT),
            s(LITTER),
            s(CARRION)
        );
    }
}
