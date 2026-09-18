//! Component metrics for one evaluated world, and the scalar rank derived from them.
//!
//! The components are the deliverable; the scalar exists only to pick parents. Every row
//! carries the whole component vector, and [`crate::search`] also reports the nondominated
//! set over [`Components::objectives`], so a convenient scalar cannot hide why a candidate won.
//!
//! Deliberate non-goals, from the handoff: maximum population is not a component, a world of
//! immortal unfed bodies scores zero on `activity`, a single dominant form scores low on
//! `variety`, and apex *dormancy* is counted separately from apex *life*.

use cubarium_core::IntakeDiagnostics;
use serde::{Deserialize, Serialize};

/// Distinct heritable `form` values a cubarium world can contain: the four founder kinds plus
/// the apex rig. Mutation never touches `form`, so this is a closed set, and normalizing the
/// form entropy by `ln(5)` gives an evenness that is 1 only for five equally common forms.
pub const FORMS_POSSIBLE: f64 = 5.0;

/// The three **guilds**, decided from the decoded phenotype and nothing else
/// (`design/ecology-v1-contract.md` §6.1): `cap_foliage` and `cap_detrital` are `φ(diet)` and
/// `φ(1 − diet)` gated at `θ`, and `θ ≤ 0.5` guarantees at least one of them is positive, so
/// the three cases below are exhaustive.
///
/// - `Herbivore`: `cap_foliage > 0`, `cap_detrital = 0` — leaf and fruit only.
/// - `Detritivore`: `cap_detrital > 0`, `cap_foliage = 0` — litter and remains only.
/// - `Generalist`: both positive — all four foods, each at a reduced yield.
///
/// A body's guild is fixed for life: `diet` mutates at conception, never afterwards, so a
/// guild census changes only through birth and death and `births = deaths + Δpopulation`
/// holds per guild. The visual founder kinds are counted separately, by `form`, because a
/// grazer's descendants can be born into any guild while staying the same creature on screen.
pub const GUILDS: [&str; 3] = ["herbivore", "detritivore", "generalist"];

/// Index into the per-guild arrays. Apex members are excluded from every guild count: they
/// are the imported cohort, counted apart.
pub fn guild_of(cap_foliage: f64, cap_detrital: f64) -> usize {
    match (cap_foliage > 0.0, cap_detrital > 0.0) {
        (true, false) => 0,
        (false, true) => 1,
        _ => 2,
    }
}

/// Death causes in the order `Components` reports them: starvation, age, collapse, predation.
/// Exactly the four `cubarium_core::organism::DeathCause` distinguishes.
pub const DEATH_CAUSES: [&str; 4] = ["starvation", "age", "collapse", "predation"];

/// Apex event counters in the order they are carried in `Sample::cum_apex`.
pub const APEX_EVENTS: [&str; 7] = [
    "attacks",
    "captures",
    "births",
    "deaths",
    "matings",
    "emergences",
    "exhausted",
];

/// One periodic observation of the world. Cheap: field sums plus one pass over the organisms.
///
/// Every `cum_*` field is **cumulative since the run started**, so the flow over any window is
/// the difference between the window's first and last sample and no second accumulator is
/// needed. [`IntakeDiagnostics`] is cumulative from the moment the `World` was constructed,
/// which for this harness is the run's first tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub tick: u64,
    pub population: u32,
    pub prey: u32,
    pub apex_active: u32,
    pub apex_dormant: u32,
    pub producer: f64,
    pub detritus: f64,
    pub fruit: f64,
    pub nutrient: f64,
    pub water: f64,
    pub form_evenness: f64,
    pub forms_present: u32,
    pub feeding: u32,
    pub seeking: u32,
    pub resting: u32,
    pub mean_hunger: f64,
    pub mass_residual: f64,
    pub water_residual: f64,
    pub energy_residual: f64,

    // --- ecology v1 stocks (§3.1), summed over all 1,280 cells ----------------------
    /// `W`, `Q`, `Wd`, `C` and `Ce`: living wood, plant reserve, dead wood, animal remains
    /// and the energy those remains carry. `producer` above is `Σ P`, foliage alone.
    pub wood: f64,
    pub plant_reserve: f64,
    pub dead_wood: f64,
    pub carrion: f64,
    pub carrion_energy: f64,

    // --- cell classes, from the current wood (§3.1) ---------------------------------
    pub alive_cells: u32,
    pub establishing_cells: u32,
    pub bare_cells: u32,
    /// Cells currently below the depletion threshold (see [`Components::depletion_events`]).
    pub depleted_cells: u32,

    // --- who is alive -------------------------------------------------------------
    /// Live prey by guild, in [`GUILDS`] order. Apex members are excluded.
    pub guild: [u32; 3],
    /// Live bodies by `form`, apex included at index 4, in the pack's creature order
    /// (lantern 0 = grazer, sail 1 = glider, mossback 2 = burrower, skimmer 3, apex rig 4).
    pub forms: [u32; 5],

    // --- cumulative counters as of this tick --------------------------------------
    pub cum_prey_births: u64,
    pub cum_prey_births_guild: [u64; 3],
    pub cum_prey_deaths: u64,
    pub cum_prey_deaths_guild: [u64; 3],
    /// All deaths by cause, apex included, in [`DEATH_CAUSES`] order.
    pub cum_deaths_cause: [u64; 4],
    /// In [`APEX_EVENTS`] order.
    pub cum_apex: [u64; 7],
    /// The world's own counters (§3.3).
    pub cum_plant_deaths: u64,
    pub cum_recolonisations: u64,
    /// Foliage depletion and recovery crossings counted so far.
    pub cum_depletion_events: u64,
    pub cum_recovery_events: u64,
    /// The world's own intake ledger, cumulative (`cubarium_core::IntakeDiagnostics`).
    pub intake: IntakeDiagnostics,
}

/// Everything one completed evaluation measured. Serialized in full into each result row.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Components {
    // --- horizon and terminal state -------------------------------------------------
    pub horizon_ticks: u64,
    /// Ticks actually simulated. Below the horizon only on a terminal collapse.
    pub ticks_run: u64,
    /// Ticks before the world held no organisms at all; the horizon when it never collapsed.
    pub survived_ticks: u64,
    pub collapsed: bool,
    pub final_population: u32,
    pub final_prey: u32,
    pub peak_population: u32,
    pub mean_population: f64,
    pub min_population: u32,

    // --- producers and recycling ----------------------------------------------------
    pub mean_producer: f64,
    pub min_producer: f64,
    pub final_producer: f64,
    pub mean_detritus: f64,
    pub mean_nutrient: f64,
    pub mean_fruit: f64,
    /// Gross primary production over the run, in material units (audited light income
    /// divided by the producer's energy density), expressed per simulated hour.
    pub gross_production_per_hour: f64,
    /// A cell's carrying capacity times the cell count: the scale `mean_producer` is read against.
    pub producer_capacity: f64,

    // --- prey turnover, lineages, maturation ----------------------------------------
    pub prey_births: u64,
    pub prey_deaths: u64,
    pub deaths_starvation: u64,
    pub deaths_age: u64,
    pub deaths_collapse: u64,
    pub deaths_predation: u64,
    /// Prey born during the run that were observed at full adult structure.
    pub descendants_matured: u64,
    /// Prey born during the run that later became a parent themselves.
    pub descendants_reproduced: u64,
    /// Longest parent chain built entirely from births inside the run.
    pub max_lineage_depth: u32,
    /// Distinct founders with a living descendant (or still alive themselves) at the end.
    pub founder_lineages_alive: u32,
    pub distinct_parents: u64,

    // --- variety --------------------------------------------------------------------
    /// Mean over samples of the form entropy normalized by `ln(FORMS_POSSIBLE)`.
    pub mean_form_evenness: f64,
    pub min_forms_present: u32,
    pub final_forms_present: u32,

    // --- active life, not immortal unfed bodies -------------------------------------
    /// Mean fraction of the live population in `Feeding`.
    pub feeding_fraction: f64,
    pub seeking_fraction: f64,
    pub resting_fraction: f64,
    /// Mean of `1 - R/R_max` over the live population: a world of full, idle bodies reads low,
    /// a world of starving ones reads high.
    pub mean_hunger: f64,

    // --- apex -----------------------------------------------------------------------
    pub apex_introduced: u32,
    /// Mean count of apex members that are alive **and not dormant**.
    pub apex_active_mean: f64,
    pub apex_dormant_mean: f64,
    /// Fraction of samples with at least one active apex. Episodic occupancy is expected;
    /// permanent occupancy is not the target.
    pub apex_active_sample_fraction: f64,
    pub apex_alive_final: u32,
    pub apex_attacks: u64,
    pub apex_captures: u64,
    pub apex_births: u64,
    pub apex_deaths: u64,
    pub apex_matings: u64,
    pub apex_emergences: u64,
    pub apex_exhausted: u64,

    // --- recovery after predator pressure -------------------------------------------
    /// Whether any predation death happened, so the recovery numbers mean anything.
    pub predation_occurred: bool,
    pub prey_before_first_predation: u32,
    pub prey_min_after_first_predation: u32,
    /// Final prey over the post-predation minimum; 0 when no predation happened.
    pub prey_recovery_ratio: f64,

    // --- conservation ---------------------------------------------------------------
    pub max_abs_mass_residual: f64,
    pub max_abs_water_residual: f64,
    pub max_abs_energy_residual: f64,

    // --- identity -------------------------------------------------------------------
    pub final_ecology_hash: u64,
    pub final_state_hash: u64,

    // --- ecology v1 -----------------------------------------------------------------
    /// The whole run, from tick 0 to the terminal sample.
    pub eco: EcoMeasures,
    /// The **late window**: the last 20 % of the declared horizon. `None` when the world
    /// collapsed before the window opened, so a censored late window is never reported as a
    /// window of zeros.
    pub late: Option<EcoMeasures>,
    /// Foliage per cell at tick 0, summed: the reference the depletion threshold is read
    /// against, and the "initial stock" half of `intake ≤ production + initial stocks`.
    pub opening_foliage: f64,
    pub opening_wood: f64,
    pub opening_litter: f64,
    pub opening_alive_cells: u32,
    /// Cells that started with foliage, so the depletion counts have a denominator.
    pub depletion_cells_watched: u32,
    /// Mean distinct cells one prey body visited inside one spatial window, over every
    /// window and over the last window alone. A body counts only if it was alive for the
    /// whole window, so the number is a foraging range and not a lifespan.
    pub cells_per_body_window: f64,
    pub cells_per_body_window_late: f64,
    pub spatial_windows_observed: u64,
    pub spatial_bodies_observed: u64,
}

/// Every ecology v1 measurement over one window of a run: the stocks as means and terminal
/// values, the flows as differences of the cumulative ledgers, and the census by guild.
///
/// Stocks are `Σ` over all 1,280 cells, in material units `m`. Flows are totals **over the
/// window**, not rates; `seconds` is there so a reader can divide. Nothing here is scaled by
/// a reference: the trade-off table is the deliverable and a scalar would hide it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EcoMeasures {
    pub start_tick: u64,
    pub end_tick: u64,
    pub samples: u64,
    pub seconds: f64,

    // --- stocks ---------------------------------------------------------------------
    pub foliage_mean: f64,
    pub foliage_final: f64,
    pub foliage_min: f64,
    pub wood_mean: f64,
    pub wood_final: f64,
    pub dead_wood_mean: f64,
    pub dead_wood_final: f64,
    pub plant_reserve_mean: f64,
    pub plant_reserve_final: f64,
    pub litter_mean: f64,
    pub litter_final: f64,
    pub remains_mean: f64,
    pub remains_final: f64,
    pub fruit_mean: f64,
    pub fruit_final: f64,
    pub nutrient_mean: f64,
    pub nutrient_final: f64,

    // --- cells ----------------------------------------------------------------------
    pub alive_cells_mean: f64,
    pub alive_cells_final: u32,
    pub establishing_cells_mean: f64,
    pub establishing_cells_final: u32,
    pub bare_cells_mean: f64,
    pub bare_cells_final: u32,
    pub depleted_cells_final: u32,
    /// Stand deaths and crossings of `W_min` from a propagule, inside the window (§3.3).
    pub plant_deaths: u64,
    pub recolonisations: u64,
    /// Cells whose foliage fell below a quarter of their opening value, and cells that later
    /// climbed back above half of it, inside the window.
    pub depletion_events: u64,
    pub recovery_events: u64,

    // --- plant flows (§4) -------------------------------------------------------------
    /// `Σ A`: material plant income actually took out of `N`, before maintenance and
    /// construction respiration send most of it back.
    pub plant_income: f64,
    /// `Σ ΔP`: **gross foliage grown**, the edible leaf replacement an intake is read against.
    pub foliage_grown: f64,
    /// `Σ unpaid`: maintenance neither income nor reserve could cover, which is what becomes
    /// dieback.
    pub plant_maintenance_unpaid: f64,
    /// `Σ s`: reserve material donors sent to establishing or bare neighbours.
    pub propagule_sent: f64,

    // --- intake by food (§6.4), served material ---------------------------------------
    pub leaf_eaten: f64,
    pub fruit_eaten: f64,
    pub litter_eaten: f64,
    pub remains_eaten: f64,
    /// Feces: what a mouth returned to `D` because its machinery could not digest it.
    pub undigested: f64,
    /// What the mouths asked for before the per-cell proportional share.
    pub requested: f64,
    pub request_ticks: u64,
    pub reserve_saturated_ticks: u64,

    // --- the animals' actual bill (§7) -------------------------------------------------
    pub body_bill_total: f64,
    pub body_bill_paid: f64,
    pub body_bill_upkeep: f64,

    // --- census ------------------------------------------------------------------------
    pub population_mean: f64,
    pub population_final: u32,
    pub prey_final: u32,
    /// Live prey by guild, in [`GUILDS`] order.
    pub guild_mean: [f64; 3],
    pub guild_final: [u32; 3],
    /// Live bodies by visual `form`, apex at index 4.
    pub form_mean: [f64; 5],
    pub form_final: [u32; 5],

    // --- turnover ------------------------------------------------------------------------
    pub prey_births: u64,
    pub prey_births_guild: [u64; 3],
    pub prey_deaths: u64,
    pub prey_deaths_guild: [u64; 3],
    /// All deaths by cause, apex included, in [`DEATH_CAUSES`] order.
    pub deaths_cause: [u64; 4],

    // --- apex ------------------------------------------------------------------------------
    /// In [`APEX_EVENTS`] order: attacks, captures, births, deaths, matings, emergences,
    /// exhausted.
    pub apex_events: [u64; 7],
    pub apex_active_mean: f64,
    pub apex_dormant_mean: f64,
    pub apex_active_sample_fraction: f64,
    pub apex_alive_final: u32,

    // --- activity ----------------------------------------------------------------------------
    pub feeding_fraction: f64,
    pub seeking_fraction: f64,
    pub resting_fraction: f64,
    pub mean_hunger: f64,
    pub form_evenness_mean: f64,
}

impl EcoMeasures {
    /// Measure one window from the samples inside it. Flows are the difference between the
    /// window's **first** and **last** sample, which is exact because every `cum_*` field is
    /// cumulative from the run's first tick.
    ///
    /// Returns `None` for an empty slice, and a window of one sample carries zero flow — a
    /// window is a difference, so a single observation cannot state one.
    pub fn from_samples(window: &[Sample]) -> Option<EcoMeasures> {
        let first = window.first()?;
        let last = window.last()?;
        let n = window.len() as f64;
        let mean = |f: fn(&Sample) -> f64| window.iter().map(f).sum::<f64>() / n;
        let flow = |f: fn(&Sample) -> u64| f(last).saturating_sub(f(first));
        let fflow = |f: fn(&Sample) -> f64| f(last) - f(first);
        let mut guild_mean = [0.0; 3];
        for (g, slot) in guild_mean.iter_mut().enumerate() {
            *slot = window.iter().map(|s| f64::from(s.guild[g])).sum::<f64>() / n;
        }
        let mut form_mean = [0.0; 5];
        for (k, slot) in form_mean.iter_mut().enumerate() {
            *slot = window.iter().map(|s| f64::from(s.forms[k])).sum::<f64>() / n;
        }
        let mut prey_births_guild = [0u64; 3];
        let mut prey_deaths_guild = [0u64; 3];
        for g in 0..3 {
            prey_births_guild[g] =
                last.cum_prey_births_guild[g].saturating_sub(first.cum_prey_births_guild[g]);
            prey_deaths_guild[g] =
                last.cum_prey_deaths_guild[g].saturating_sub(first.cum_prey_deaths_guild[g]);
        }
        let mut deaths_cause = [0u64; 4];
        for (c, slot) in deaths_cause.iter_mut().enumerate() {
            *slot = last.cum_deaths_cause[c].saturating_sub(first.cum_deaths_cause[c]);
        }
        let mut apex_events = [0u64; 7];
        for (e, slot) in apex_events.iter_mut().enumerate() {
            *slot = last.cum_apex[e].saturating_sub(first.cum_apex[e]);
        }
        let with_apex = window.iter().filter(|s| s.apex_active > 0).count() as f64;

        Some(EcoMeasures {
            start_tick: first.tick,
            end_tick: last.tick,
            samples: window.len() as u64,
            seconds: (last.tick.saturating_sub(first.tick)) as f64 * cubarium_core::DT,

            foliage_mean: mean(|s| s.producer),
            foliage_final: last.producer,
            foliage_min: window
                .iter()
                .map(|s| s.producer)
                .fold(f64::INFINITY, f64::min),
            wood_mean: mean(|s| s.wood),
            wood_final: last.wood,
            dead_wood_mean: mean(|s| s.dead_wood),
            dead_wood_final: last.dead_wood,
            plant_reserve_mean: mean(|s| s.plant_reserve),
            plant_reserve_final: last.plant_reserve,
            litter_mean: mean(|s| s.detritus),
            litter_final: last.detritus,
            remains_mean: mean(|s| s.carrion),
            remains_final: last.carrion,
            fruit_mean: mean(|s| s.fruit),
            fruit_final: last.fruit,
            nutrient_mean: mean(|s| s.nutrient),
            nutrient_final: last.nutrient,

            alive_cells_mean: mean(|s| f64::from(s.alive_cells)),
            alive_cells_final: last.alive_cells,
            establishing_cells_mean: mean(|s| f64::from(s.establishing_cells)),
            establishing_cells_final: last.establishing_cells,
            bare_cells_mean: mean(|s| f64::from(s.bare_cells)),
            bare_cells_final: last.bare_cells,
            depleted_cells_final: last.depleted_cells,
            plant_deaths: flow(|s| s.cum_plant_deaths),
            recolonisations: flow(|s| s.cum_recolonisations),
            depletion_events: flow(|s| s.cum_depletion_events),
            recovery_events: flow(|s| s.cum_recovery_events),

            plant_income: fflow(|s| s.intake.plant_income),
            foliage_grown: fflow(|s| s.intake.producer_growth),
            plant_maintenance_unpaid: fflow(|s| s.intake.plant_maintenance_unpaid),
            propagule_sent: fflow(|s| s.intake.propagule_sent),

            leaf_eaten: fflow(|s| s.intake.producer_eaten),
            fruit_eaten: fflow(|s| s.intake.fruit_eaten),
            litter_eaten: fflow(|s| s.intake.litter_eaten),
            remains_eaten: fflow(|s| s.intake.carrion_eaten),
            undigested: fflow(|s| s.intake.undigested),
            requested: fflow(|s| s.intake.requested),
            request_ticks: flow(|s| s.intake.request_ticks),
            reserve_saturated_ticks: flow(|s| s.intake.reserve_saturated_ticks),

            body_bill_total: fflow(|s| s.intake.body_bill_total),
            body_bill_paid: fflow(|s| s.intake.body_bill_paid),
            body_bill_upkeep: fflow(|s| s.intake.body_bill_upkeep),

            population_mean: mean(|s| f64::from(s.population)),
            population_final: last.population,
            prey_final: last.prey,
            guild_mean,
            guild_final: last.guild,
            form_mean,
            form_final: last.forms,

            prey_births: flow(|s| s.cum_prey_births),
            prey_births_guild,
            prey_deaths: flow(|s| s.cum_prey_deaths),
            prey_deaths_guild,
            deaths_cause,

            apex_events,
            apex_active_mean: mean(|s| f64::from(s.apex_active)),
            apex_dormant_mean: mean(|s| f64::from(s.apex_dormant)),
            apex_active_sample_fraction: with_apex / n,
            apex_alive_final: last.apex_active + last.apex_dormant,

            feeding_fraction: mean(|s| {
                if s.population > 0 {
                    f64::from(s.feeding) / f64::from(s.population)
                } else {
                    0.0
                }
            }),
            seeking_fraction: mean(|s| {
                if s.population > 0 {
                    f64::from(s.seeking) / f64::from(s.population)
                } else {
                    0.0
                }
            }),
            resting_fraction: mean(|s| {
                if s.population > 0 {
                    f64::from(s.resting) / f64::from(s.population)
                } else {
                    0.0
                }
            }),
            mean_hunger: mean(|s| s.mean_hunger),
            form_evenness_mean: mean(|s| s.form_evenness),
        })
    }

    /// `births − deaths` per guild. The census identity Fable's review re-derives is
    /// `births = deaths + Δpopulation` per guild, and this is its left-hand side.
    pub fn guild_net(&self) -> [i64; 3] {
        let mut net = [0i64; 3];
        for g in 0..3 {
            net[g] = self.prey_births_guild[g] as i64 - self.prey_deaths_guild[g] as i64;
        }
        net
    }

    /// Everything a mouth took, in material units.
    pub fn total_eaten(&self) -> f64 {
        self.leaf_eaten + self.fruit_eaten + self.litter_eaten + self.remains_eaten
    }
}

/// The reference scales the component scores are read against.
///
/// These are *declared hypotheses*, calibrated once against the measured default-parameter
/// baseline at a 120 000-tick (100 simulated minute) horizon, where the ecology is actually
/// running. They are not retuned per search. Every rate is per simulated hour, so a component
/// means the same thing at any horizon; the two `_fraction` references are already
/// dimensionless. Kept in one struct so a follow-up can change them explicitly and say so.
///
/// Measured default baseline (seed 1001, 120 000 ticks, 2 apex founders), for reference:
/// standing crop 0.164 of capacity, gross production 1.34 capacities/hour, 223 prey
/// births/hour, 171 prey deaths/hour, 67% of births reaching adult structure, 51% of them
/// reproducing, feeding fraction 0.44, 3.6 apex captures/hour, apex present in 13% of samples,
/// and **no apex alive at the end** - the inadequacy this search exists to move.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Scoring {
    /// Mean standing crop, as a fraction of total carrying capacity, that scores 1.
    pub producer_stock_ref: f64,
    /// Gross primary production per simulated hour, in units of total carrying capacity.
    pub production_ref: f64,
    /// Prey births per simulated hour that score 1.
    pub prey_births_per_hour_ref: f64,
    /// Prey deaths per simulated hour that score 1. Below the birth reference on purpose: a
    /// world is allowed to be growing.
    pub prey_deaths_per_hour_ref: f64,
    /// In-run descendants reaching adult structure per simulated hour.
    pub matured_per_hour_ref: f64,
    /// Fraction of in-run births that reach adult structure.
    pub matured_fraction_ref: f64,
    /// In-run descendants that themselves reproduce, per simulated hour.
    pub reproduced_per_hour_ref: f64,
    /// Fraction of in-run births that themselves reproduce.
    pub reproduced_fraction_ref: f64,
    /// Mean feeding fraction at which the world counts as fully active. This is a **gate**,
    /// not something to maximize: the point is to score a world of immortal unfed bodies at
    /// zero, not to reward frantic feeding.
    pub feeding_ref: f64,
    /// Apex captures per simulated hour that score 1.
    pub apex_captures_per_hour_ref: f64,
    /// Fraction of samples with an active apex that scores 1. Well below 1, because episodic
    /// occupancy is the target and permanent occupancy earns nothing further.
    pub apex_occupancy_ref: f64,
    /// Floor applied to every term before a geometric mean, so one zero dominates a rank
    /// without collapsing every failing candidate into an untied zero.
    pub component_floor: f64,
}

impl Default for Scoring {
    fn default() -> Self {
        Scoring {
            producer_stock_ref: 0.20,
            production_ref: 1.5,
            prey_births_per_hour_ref: 250.0,
            prey_deaths_per_hour_ref: 200.0,
            matured_per_hour_ref: 150.0,
            matured_fraction_ref: 0.60,
            reproduced_per_hour_ref: 120.0,
            reproduced_fraction_ref: 0.40,
            feeding_ref: 0.25,
            apex_captures_per_hour_ref: 4.0,
            apex_occupancy_ref: 0.5,
            component_floor: 1e-3,
        }
    }
}

fn clamp01(x: f64) -> f64 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Geometric mean with every term floored, so a single zero dominates without erasing the
/// gradient the other terms still carry.
fn geometric(terms: &[f64], floor: f64) -> f64 {
    let sum: f64 = terms.iter().map(|t| t.max(floor).ln()).sum();
    (sum / terms.len() as f64).exp()
}

/// The seven scores the scalar rank and the Pareto front are both built from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Objectives {
    pub persistence: f64,
    pub plants: f64,
    pub prey_turnover: f64,
    pub maturation: f64,
    pub lineage: f64,
    pub variety: f64,
    pub apex: f64,
}

impl Objectives {
    pub const NAMES: [&'static str; 7] = [
        "persistence",
        "plants",
        "prey_turnover",
        "maturation",
        "lineage",
        "variety",
        "apex",
    ];

    pub fn as_array(&self) -> [f64; 7] {
        [
            self.persistence,
            self.plants,
            self.prey_turnover,
            self.maturation,
            self.lineage,
            self.variety,
            self.apex,
        ]
    }

    /// `self` is at least as good on every objective and strictly better on one.
    pub fn dominates(&self, other: &Objectives) -> bool {
        let (a, b) = (self.as_array(), other.as_array());
        a.iter().zip(b).all(|(x, y)| *x >= y) && a.iter().zip(b).any(|(x, y)| *x > y)
    }
}

impl Components {
    /// Simulated hours actually run.
    pub fn hours(&self) -> f64 {
        (self.ticks_run as f64 * cubarium_core::DT / 3600.0).max(1e-9)
    }

    /// In-run births that were seen at full adult structure.
    pub fn matured_fraction(&self) -> f64 {
        if self.prey_births == 0 {
            0.0
        } else {
            self.descendants_matured as f64 / self.prey_births as f64
        }
    }

    /// In-run births that went on to be a parent themselves.
    pub fn reproduced_fraction(&self) -> f64 {
        if self.prey_births == 0 {
            0.0
        } else {
            self.descendants_reproduced as f64 / self.prey_births as f64
        }
    }

    /// Apex lineage continuity: members still alive at the end, plus every apex born in the
    /// run, against the cohort that was introduced. Zero is the current shipped behaviour.
    pub fn apex_lineage(&self) -> f64 {
        let introduced = f64::from(self.apex_introduced.max(1));
        clamp01((f64::from(self.apex_alive_final) + self.apex_births as f64) / introduced)
    }

    pub fn objectives(&self, s: &Scoring) -> Objectives {
        let floor = s.component_floor;
        let hours = self.hours();
        let horizon = self.horizon_ticks.max(1) as f64;
        let persistence = clamp01(self.survived_ticks as f64 / horizon);

        let capacity = self.producer_capacity.max(1e-9);
        // Standing crop *and* the gross production that keeps replacing it: a world can hold
        // biomass without producing any, and a world can produce without anything standing.
        let plants = geometric(
            &[
                clamp01(self.mean_producer / (s.producer_stock_ref * capacity)),
                clamp01(self.gross_production_per_hour / (s.production_ref * capacity)),
            ],
            floor,
        );

        // Turnover needs both halves: a world that only births, or only dies, is not turning over.
        let prey_turnover = geometric(
            &[
                clamp01(self.prey_births as f64 / hours / s.prey_births_per_hour_ref),
                clamp01(self.prey_deaths as f64 / hours / s.prey_deaths_per_hour_ref),
            ],
            floor,
        );

        // A rate and a fraction together: many births of which none mature is not maturation,
        // and one birth that happens to mature is not an ecology.
        let maturation = geometric(
            &[
                clamp01(self.matured_fraction() / s.matured_fraction_ref),
                clamp01(self.descendants_matured as f64 / hours / s.matured_per_hour_ref),
            ],
            floor,
        );
        let lineage = geometric(
            &[
                clamp01(self.reproduced_fraction() / s.reproduced_fraction_ref),
                clamp01(self.descendants_reproduced as f64 / hours / s.reproduced_per_hour_ref),
            ],
            floor,
        );

        // Evenness, held down by whether the world ever lost a form outright.
        let variety = clamp01(self.mean_form_evenness)
            * clamp01(f64::from(self.min_forms_present) / FORMS_POSSIBLE).max(0.25);

        // Present, eating, and still there: an apex that only ever perches scores nothing, one
        // that never eats scores nothing, and one whose whole cohort dies scores nearly
        // nothing however busy it was.
        let apex = geometric(
            &[
                clamp01(self.apex_active_sample_fraction / s.apex_occupancy_ref),
                clamp01(self.apex_captures as f64 / hours / s.apex_captures_per_hour_ref),
                self.apex_lineage(),
            ],
            floor,
        );

        Objectives {
            persistence,
            plants,
            prey_turnover,
            maturation,
            lineage,
            variety,
            apex,
        }
    }

    /// The scalar rank: the geometric mean of the seven objectives, gated by activity.
    ///
    /// A world whose bodies never feed is multiplied out however many of them there are, which
    /// is what keeps "maximum population" and "immortal unfed bodies" from ranking.
    pub fn fitness(&self, s: &Scoring) -> f64 {
        let objectives = self.objectives(s).as_array();
        let activity = clamp01(self.feeding_fraction / s.feeding_ref);
        geometric(&objectives, s.component_floor) * activity.max(s.component_floor)
    }
}
