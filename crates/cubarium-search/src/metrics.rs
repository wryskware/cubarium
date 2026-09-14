//! Component metrics for one evaluated world, and the scalar rank derived from them.
//!
//! The components are the deliverable; the scalar exists only to pick parents. Every row
//! carries the whole component vector, and [`crate::search`] also reports the nondominated
//! set over [`Components::objectives`], so a convenient scalar cannot hide why a candidate won.
//!
//! Deliberate non-goals, from the handoff: maximum population is not a component, a world of
//! immortal unfed bodies scores zero on `activity`, a single dominant form scores low on
//! `variety`, and apex *dormancy* is counted separately from apex *life*.

use serde::{Deserialize, Serialize};

/// Distinct heritable `form` values a cubarium world can contain: the four founder kinds plus
/// the apex rig. Mutation never touches `form`, so this is a closed set, and normalizing the
/// form entropy by `ln(5)` gives an evenness that is 1 only for five equally common forms.
pub const FORMS_POSSIBLE: f64 = 5.0;

/// One periodic observation of the world. Cheap: field sums plus one pass over the organisms.
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
    if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 }
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

        Objectives { persistence, plants, prey_turnover, maturation, lineage, variety, apex }
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
