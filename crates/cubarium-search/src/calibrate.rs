//! The ecology v1 calibration: a **declared matrix** of candidate configurations × seeds ×
//! matched apex arms, run on the real `World` with reproduction and the ordinary lifecycle on.
//!
//! This is not a genetic search. [`crate::search`] explores a box; this module evaluates a
//! hand-designed list of joint hypotheses ([`CANDIDATES`]), each of which moves two or more of
//! the four axes together, on the same seeds and the same three apex arms, and writes one
//! JSONL row per run. Every row carries `param_bits`, so `cubarium-search replay --record
//! <file> --index <i>` re-derives it bit for bit exactly as it does for a search row.
//!
//! **Matched arms.** Every arm runs the identical world, from the identical seed, with the
//! identical parameter vector, and introduces `0`, `1` or `2` apex adults at the **same tick**
//! through the same `World::introduce_hunters` the interactive control uses. The apex profile
//! is `FixedHunterProfile::lanternjaw_trial` in every arm and is never searched, so an arm
//! difference is the predator's presence and nothing else. The cohort is never restocked, and
//! the material and energy it imported are recorded per row (`apex_material_in`,
//! `apex_energy_in`), so the imported stock is on the record rather than free.
//!
//! **Nothing here protects anything.** There is no culling, no quota, no replenishment and no
//! controller rule that shelters plants. A candidate that eats its world is recorded eating
//! its world.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_core::WorldConfig;
use cubarium_core::hunter::FixedHunterProfile;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::evaluate::{BUILD_ID, Evaluation, Protocol, RunOptions, Status, evaluate_with};
use crate::params;
use crate::search::{HELDOUT_SEEDS, TRAINING_SEEDS};

/// The three matched apex arms, in the order the summary reports them.
pub const ARMS: [u32; 3] = [0, 1, 2];

/// Workstream I's ladder: the shipped price, the three equal 0.0003 steps that fit between it
/// and workstream F's lowest raised level, and that level. Declared here so the campaign's
/// command is checkable against the note rather than retyped from it.
pub const LADDER_PRICES: [f64; 5] = [0.00036, 0.0006, 0.0009, 0.0012, 0.0018];

/// The shipped `organism.move_cost`, in e per unit of structure per pixel travelled. A stage
/// that names no price runs at this one, which is exactly what the calibration screen ran.
pub const DEFAULT_MOVE_COST: f64 = 0.00036;

fn default_move_cost() -> f64 {
    DEFAULT_MOVE_COST
}

/// One declared candidate: a name, the hypothesis it encodes, and the searched names it moves
/// away from the shipped defaults. Everything it does not name keeps the default value.
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub name: &'static str,
    /// Which axes it moves, for the trade-off table: `P` production and foliage turnover,
    /// `R` the plant's reserve policy, `A` animal intake and upkeep, `D` demography,
    /// `C` recycling.
    pub axes: &'static str,
    pub hypothesis: &'static str,
    pub overrides: &'static [(&'static str, f64)],
}

/// The designed screen: the shipped defaults plus fourteen joint candidates.
///
/// Each one is a hypothesis about **why** the accepted ecology v1 baseline fails, taken from
/// the measurements in `design/7_Research/ecology-v1-implementation-2026-09-15.md` Run 3:
///
/// - B6b: the prey population doubles in 150 s while the stand it eats needs 823 s to recover
///   half its foliage, and the measured income/upkeep ratio falls from 3.52 to 0.03.
/// - B1b: one mobile grazer took a bright 5 × 5 region from 11.97 m of foliage to 0.02 m and
///   killed 17 of its 25 stands before starving.
/// - B0: the average light band settles at `P = 0.0977` against a hand value of 0.50 — it sits
///   just above `(L·μ)_crit = 2.439·m_p/g = 0.305`, the light below which foliage cannot
///   persist at all (see [`crate::params`]).
///
/// The list deliberately includes one candidate in the *opposite* direction (`cheap-fast-fauna`)
/// so the screen brackets the default rather than only pushing past it.
pub const CANDIDATES: &[Candidate] = &[
    Candidate {
        name: "baseline",
        axes: "-",
        hypothesis: "the shipped ecology v1 provisional defaults (§11), unchanged",
        overrides: &[],
    },
    Candidate {
        name: "lit-world",
        axes: "P",
        hypothesis: "the world is dark, not overgrazed: raising `g` and halving `m_p` moves \
                     the critical light from 0.305 to 0.091, which should give the average \
                     band real foliage instead of the measured 0.098",
        overrides: &[("producer.growth", 0.016), ("producer.mortality", 0.0006)],
    },
    Candidate {
        name: "lit-world-slow-breeders",
        axes: "P+D",
        hypothesis: "the same brighter world, with prey that cannot double inside a plant \
                     recovery time: generation 300 s against B3's 823 s half-recovery",
        overrides: &[
            ("producer.growth", 0.016),
            ("producer.mortality", 0.0006),
            ("drives.bud_min_age_seconds", 300.0),
            ("drives.bud_reserve", 0.80),
        ],
    },
    Candidate {
        name: "fast-leaf",
        axes: "P+R",
        hypothesis: "production is adequate and *recovery* is the bottleneck: triple the leaf \
                     cap, halve the cost of standing structure, bank more of the surplus",
        overrides: &[
            ("plant.foliage_rate", 0.006),
            ("plant.maintenance", 0.0001),
            ("plant.reserve_share", 0.35),
        ],
    },
    Candidate {
        name: "fast-leaf-slow-breeders",
        axes: "P+R+D",
        hypothesis: "faster recovery is not enough on its own unless the fauna also stops \
                     outrunning it; slower maturation as well as a later first brood",
        overrides: &[
            ("plant.foliage_rate", 0.006),
            ("plant.maintenance", 0.0001),
            ("plant.reserve_share", 0.35),
            ("drives.bud_min_age_seconds", 300.0),
            ("organism.growth_rate", 0.006),
        ],
    },
    Candidate {
        name: "small-mouths",
        axes: "A",
        hypothesis: "the mouth is the problem: §11 puts one bite at ~40× a bright stand's \
                     sustainable yield. A fifth of the mouth, a cheaper body and a much \
                     higher `K_P` so a thinned cell stops paying its occupant",
        overrides: &[
            ("organism.mouth_rate", 0.020),
            ("organism.intake_half_saturation", 0.80),
            ("organism.maintenance", 0.0035),
        ],
    },
    Candidate {
        name: "small-mouths-lit",
        axes: "A+P",
        hypothesis: "gentler mouths on a brighter world: the two independent fixes together, \
                     to see whether they add or whether one of them is doing all the work",
        overrides: &[
            ("organism.mouth_rate", 0.020),
            ("organism.intake_half_saturation", 0.80),
            ("organism.maintenance", 0.0035),
            ("producer.growth", 0.014),
            ("producer.mortality", 0.0007),
        ],
    },
    Candidate {
        name: "joint-moderate",
        axes: "P+R+A+D",
        hypothesis: "every axis moved a little rather than one moved a lot: the cheapest \
                     plausible world, and the reference for whether the strong version is \
                     paying for itself",
        overrides: &[
            ("producer.growth", 0.012),
            ("producer.mortality", 0.0007),
            ("plant.foliage_rate", 0.004),
            ("organism.mouth_rate", 0.030),
            ("organism.intake_half_saturation", 0.65),
            ("drives.bud_min_age_seconds", 240.0),
        ],
    },
    Candidate {
        name: "joint-strong",
        axes: "P+R+A+D",
        hypothesis: "every axis moved hard: if a persistent, vegetated, turning-over world \
                     exists anywhere in this box it should exist here, and what it costs in \
                     turnover and variety is the measurement",
        overrides: &[
            ("producer.growth", 0.018),
            ("producer.mortality", 0.0005),
            ("plant.foliage_rate", 0.008),
            ("plant.maintenance", 0.0001),
            ("organism.mouth_rate", 0.020),
            ("organism.intake_half_saturation", 0.90),
            ("organism.growth_rate", 0.006),
            ("drives.bud_min_age_seconds", 360.0),
            ("drives.bud_reserve", 0.85),
        ],
    },
    Candidate {
        name: "reflush-refuge",
        axes: "R+A",
        hypothesis: "give the plant a behavioural response and the grazer a reason to leave: \
                     reflush opens at 70 % of the cap instead of 25 %, the bank is deep, and a \
                     thinned cell stops feeding well before it is stripped",
        overrides: &[
            ("plant.reflush_below", 0.70),
            ("plant.reserve_share", 0.40),
            ("organism.intake_half_saturation", 0.90),
        ],
    },
    Candidate {
        name: "recycle-fast",
        axes: "C+P",
        hypothesis: "the loop is nutrient-limited: return litter to `N` three times faster on \
                     a brighter world. The cost is the detritivore's larder, which is the \
                     trade-off being measured",
        overrides: &[
            ("detritus.decomposition", 0.006),
            ("producer.growth", 0.012),
            ("producer.mortality", 0.0007),
        ],
    },
    Candidate {
        name: "recycle-slow",
        axes: "C+A",
        hypothesis: "the opposite: litter persists as a standing larder for detritivores, and \
                     the animals are cheaper and slower-mouthed so they can live on it",
        overrides: &[
            ("detritus.decomposition", 0.0008),
            ("organism.mouth_rate", 0.030),
            ("organism.maintenance", 0.0035),
        ],
    },
    Candidate {
        name: "cheap-fast-fauna",
        axes: "A+D",
        hypothesis: "the bracket in the other direction: cheaper bodies that mature fast and \
                     breed early. Expected to fail, and included so the screen measures a \
                     direction rather than only a distance from the default",
        overrides: &[
            ("organism.maintenance", 0.0028),
            ("organism.growth_rate", 0.020),
            ("drives.bud_min_age_seconds", 90.0),
        ],
    },
    Candidate {
        name: "plant-first",
        axes: "P+R",
        hypothesis: "move only the plants, hard, and leave every animal constant alone: the \
                     control for whether any fauna change is needed at all",
        overrides: &[
            ("producer.growth", 0.016),
            ("producer.mortality", 0.0005),
            ("plant.foliage_rate", 0.006),
            ("plant.maintenance", 0.0001),
            ("plant.reflush_below", 0.50),
            ("plant.reserve_share", 0.30),
        ],
    },
    Candidate {
        name: "demography-only",
        axes: "D+A",
        hypothesis: "the mirror control: move only the fauna's timing and its willingness to \
                     strip a thin cell, and leave every plant constant alone",
        overrides: &[
            ("drives.bud_min_age_seconds", 480.0),
            ("drives.bud_reserve", 0.88),
            ("organism.growth_rate", 0.005),
            ("organism.intake_half_saturation", 0.80),
        ],
    },
];

impl Candidate {
    /// The full parameter vector: the defaults with this candidate's overrides written in.
    /// An override that names something the vector does not search is an **error**, so a
    /// renamed parameter cannot silently become a no-op candidate.
    pub fn vector(&self) -> Result<Vec<f64>, String> {
        let mut values = params::defaults();
        for (name, value) in self.overrides {
            let k = params::index_of(name)
                .ok_or_else(|| format!("candidate {}: {name} is not a searched parameter", self.name))?;
            values[k] = *value;
        }
        Ok(values)
    }

    /// The `WorldConfig` this candidate describes, at a given seed.
    pub fn config(&self, seed: u64) -> Result<WorldConfig, String> {
        let mut config = crate::evaluate::base_config(seed);
        let mut profile = FixedHunterProfile::lanternjaw_trial(&config);
        params::apply(&self.vector()?, &mut config, &mut profile)?;
        Ok(config)
    }
}

/// Look a candidate up by name.
pub fn candidate(name: &str) -> Option<&'static Candidate> {
    CANDIDATES.iter().find(|c| c.name == name)
}

/// Which seed set a stage draws from. The held-out set is never touched before the final
/// validation stage; that is the whole point of it being a separate name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeedSet {
    Training,
    Heldout,
}

impl SeedSet {
    pub fn parse(text: &str) -> Result<SeedSet, String> {
        match text {
            "training" => Ok(SeedSet::Training),
            "holdout" | "heldout" | "held-out" => Ok(SeedSet::Heldout),
            other => Err(format!("seed set {other:?} is not `training` or `holdout`")),
        }
    }

    pub fn seeds(&self, count: usize) -> Result<Vec<u64>, String> {
        let all: &[u64] = match self {
            SeedSet::Training => &TRAINING_SEEDS,
            SeedSet::Heldout => &HELDOUT_SEEDS,
        };
        if count == 0 || count > all.len() {
            return Err(format!("seed count {count} is outside 1..={}", all.len()));
        }
        Ok(all[..count].to_vec())
    }

    pub fn label(&self) -> &'static str {
        match self {
            SeedSet::Training => "training",
            SeedSet::Heldout => "holdout",
        }
    }
}

/// One `(candidate, seed, arm)` row. The `param_bits`, `param_fingerprint`, `seed`,
/// `protocol`, `status` and `metrics` keys are deliberately the same shape a
/// [`crate::search::Row`] carries, so `replay` reads either file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalibrationRow {
    pub candidate: String,
    pub axes: String,
    /// Apex adults introduced in this arm: 0, 1 or 2.
    pub arm: u32,
    /// The per-structure-pixel motor price this row was charged. It is also inside `params`
    /// and `param_bits`; it is lifted to the top level so a price arm is filterable without
    /// decoding the vector. `0.00036` is the shipped default.
    #[serde(default = "default_move_cost")]
    pub move_cost: f64,
    pub seed_set: String,
    pub stage: String,
    pub params: serde_json::Map<String, Value>,
    pub param_bits: Vec<String>,
    pub param_fingerprint: u64,
    #[serde(flatten)]
    pub evaluation: Evaluation,
}

/// Everything one stage of the calibration ran under.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StagePlan {
    pub stage: String,
    /// The `organism.move_cost` levels this stage crossed the matrix with. A single-element
    /// list at the shipped default is the calibration screen's own matrix, unchanged.
    #[serde(default)]
    pub prices: Vec<f64>,
    /// Whether this stage ran with workstream E's per-body budget ledger on (workstream I).
    /// Absent on every stage before it, which is what `false` means there.
    #[serde(default)]
    pub ledger: bool,
    /// Whether this stage ran with workstream M's per-cell plant budget on, and whether it
    /// founded no animals at all. Absent on every stage before M.
    #[serde(default)]
    pub plant_record: bool,
    #[serde(default)]
    pub no_animals: bool,
    /// The motor contract every run of this stage used (`cubarium_core::MotorModel`), by name.
    /// Absent on every stage before workstream T, which is `sweep` — the one contract there
    /// was. A transient on the world, so it does not enter [`config_hash`] and a `sweep` stage
    /// reproduces the retained rows bit for bit.
    #[serde(default = "sweep_name")]
    pub motor: String,
    /// The pursuit stopping rule every run of this stage used
    /// (`cubarium_core::hunter::PursuitStop`), by name. Absent on every stage written before
    /// this field existed, and what that absence means is **`forward_half_space`** — the rule
    /// this workspace shipped until 2026-09-16, which is what those runs in fact evaluated.
    /// It is deliberately not `PursuitStop::default()`, which is the *envelope* now: reading a
    /// retained plan under today's default would relabel it as a stage it never ran.
    ///
    /// A transient on the world, so it does not enter [`config_hash`]; a `forward_half_space`
    /// stage reproduces the retained rows bit for bit, which is the check this workstream ran.
    #[serde(default = "half_space_name")]
    pub pursuit_stop: String,
    pub build_id: String,
    pub horizon_ticks: u64,
    pub sample_every: u64,
    pub apex_introduce_tick: u64,
    pub seed_set: String,
    pub seeds: Vec<u64>,
    pub arms: Vec<u32>,
    pub candidates: Vec<String>,
    pub trials: usize,
    pub workers: usize,
    pub wall_seconds_cap: u64,
}

/// What a completed stage produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageReport {
    pub plan: StagePlan,
    pub wall_seconds: f64,
    pub trials_run: usize,
    pub trials_skipped: usize,
    pub ticks_simulated: u64,
    pub ticks_per_second: f64,
    pub peak_rss_mib: f64,
    /// One entry per `(candidate, arm)`, aggregated over the stage's seeds.
    pub cells: Vec<CellSummary>,
}

/// One `(candidate, arm)` aggregated over the seeds it ran on.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CellSummary {
    pub candidate: String,
    pub axes: String,
    pub arm: u32,
    #[serde(default = "default_move_cost")]
    pub move_cost: f64,
    pub seeds: usize,
    pub completed: usize,
    pub invalid: usize,
    pub failed: usize,
    /// First refusal reason, so a region the core rejects is legible without opening the rows.
    pub reason: Option<String>,
    /// Seeds whose world held no organisms at the end.
    pub extinctions: usize,
    /// Worst (largest) conservation residual over the seeds, so a broken accounting cannot
    /// hide inside a mean.
    pub worst_mass_residual: f64,
    pub worst_energy_residual: f64,
    /// The mean over completed seeds of every numeric field of [`Components`], as JSON. Taken
    /// generically so a component added later is averaged without touching this code.
    pub mean: Value,
    /// Per-seed foliage retention, `late-window mean ΣP / opening ΣP`, so the spread behind
    /// the mean is visible.
    pub foliage_retention: Vec<f64>,
    /// The declared plausibility gates, evaluated on this cell.
    pub gates: Gates,
}

/// The plausibility gates, **declared before the screen ran** and applied identically to every
/// candidate and every arm. They are ecological statements, not a score: a candidate passes
/// or it does not, and the trade-off table reports the components either way.
///
/// Old `Scoring` reference scales are not used anywhere in this module. They were calibrated
/// on the pre-ecology-v1 world and are not evidence here.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Gates {
    /// Every seed completed with its audits intact — no panic, no invariant failure, no
    /// refused configuration.
    pub sound: bool,
    /// No seed ended with an empty world.
    pub persists: bool,
    /// The late window still holds at least [`FOLIAGE_FLOOR`] of the foliage the world opened
    /// with, on **every** seed. This is the vegetation Wrysk reported losing.
    pub vegetated: bool,
    /// The late window still has births **and** deaths: a world that has stopped turning over
    /// is not an ecology, however much biomass it holds.
    pub turning_over: bool,
    /// Both prey guilds — herbivore and detritivore — are still present at the end of every
    /// seed. A monoculture passes nothing.
    pub guilds_intact: bool,
    /// The late window's alive cells are at least [`ALIVE_CELL_FLOOR`] of the opening count:
    /// the stands themselves are still there, not merely some foliage somewhere.
    pub stands_intact: bool,
    /// All of the above.
    pub plausible: bool,
}

/// Late-window foliage, as a fraction of the world's opening foliage, below which a candidate
/// is not called plausible. Half: the baseline's own B1b measurement lost 99.8 % of a bright
/// region's foliage, so this is a low bar deliberately set where the failure is unambiguous.
pub const FOLIAGE_FLOOR: f64 = 0.5;
/// Late-window alive cells as a fraction of the opening count, below which the stands
/// themselves are counted as lost. B1b killed 17 of 25 stands, i.e. retained 0.32.
pub const ALIVE_CELL_FLOOR: f64 = 0.8;

fn gates_for(rows: &[&CalibrationRow]) -> (Gates, Vec<f64>, usize, f64, f64) {
    let mut gates = Gates::default();
    let mut retention = Vec::new();
    let mut extinctions = 0;
    let mut worst_mass: f64 = 0.0;
    let mut worst_energy: f64 = 0.0;
    if rows.is_empty() {
        return (gates, retention, 0, 0.0, 0.0);
    }
    gates.sound = rows.iter().all(|r| r.evaluation.status == Status::Completed);
    let mut persists = true;
    let mut vegetated = true;
    let mut turning_over = true;
    let mut guilds_intact = true;
    let mut stands_intact = true;
    for row in rows {
        let Some(m) = row.evaluation.metrics.as_ref() else {
            persists = false;
            vegetated = false;
            turning_over = false;
            guilds_intact = false;
            stands_intact = false;
            continue;
        };
        worst_mass = worst_mass.max(m.max_abs_mass_residual);
        worst_energy = worst_energy.max(m.max_abs_energy_residual);
        if m.final_population == 0 {
            extinctions += 1;
            persists = false;
        }
        let opening = m.opening_foliage.max(1e-12);
        match m.late.as_ref() {
            Some(late) => {
                retention.push(late.foliage_mean / opening);
                if late.foliage_mean / opening < FOLIAGE_FLOOR {
                    vegetated = false;
                }
                if late.prey_births == 0 || late.prey_deaths == 0 {
                    turning_over = false;
                }
                if late.guild_final[0] == 0 || late.guild_final[1] == 0 {
                    guilds_intact = false;
                }
                if f64::from(late.alive_cells_final)
                    < ALIVE_CELL_FLOOR * f64::from(m.opening_alive_cells.max(1))
                {
                    stands_intact = false;
                }
            }
            None => {
                // No late window means the world did not reach it. Censored, and censored is
                // not a pass.
                retention.push(0.0);
                vegetated = false;
                turning_over = false;
                guilds_intact = false;
                stands_intact = false;
            }
        }
    }
    gates.persists = persists;
    gates.vegetated = vegetated;
    gates.turning_over = turning_over;
    gates.guilds_intact = guilds_intact;
    gates.stands_intact = stands_intact;
    gates.plausible = gates.sound
        && persists
        && vegetated
        && turning_over
        && guilds_intact
        && stands_intact;
    (gates, retention, extinctions, worst_mass, worst_energy)
}

/// Element-wise mean of a set of JSON values with identical shape. Numbers average, booleans
/// and strings take the first value, `null` stays `null`, arrays and objects recurse. Used so
/// a component added to [`Components`] is aggregated without editing this function.
fn mean_value(values: &[Value]) -> Value {
    let Some(first) = values.first() else {
        return Value::Null;
    };
    match first {
        Value::Number(_) => {
            let mut sum = 0.0;
            let mut n = 0.0;
            for v in values {
                if let Some(x) = v.as_f64() {
                    sum += x;
                    n += 1.0;
                }
            }
            if n == 0.0 {
                Value::Null
            } else {
                serde_json::Number::from_f64(sum / n).map_or(Value::Null, Value::Number)
            }
        }
        Value::Array(a) => Value::Array(
            (0..a.len())
                .map(|i| {
                    let column: Vec<Value> =
                        values.iter().filter_map(|v| v.get(i).cloned()).collect();
                    mean_value(&column)
                })
                .collect(),
        ),
        Value::Object(o) => {
            let mut out = serde_json::Map::new();
            for key in o.keys() {
                let column: Vec<Value> =
                    values.iter().filter_map(|v| v.get(key).cloned()).collect();
                out.insert(key.clone(), mean_value(&column));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Linux peak resident set size of this process, in MiB.
fn peak_rss_mib() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|k| k.parse::<f64>().ok()))
        })
        .map(|kib| kib / 1024.0)
        .unwrap_or(f64::NAN)
}

/// Run one stage of the calibration matrix.
#[allow(clippy::too_many_arguments)]
pub fn run_stage(
    stage: &str,
    names: &[String],
    seed_set: SeedSet,
    seed_count: usize,
    arms: &[u32],
    // One or more `organism.move_cost` levels. `&[DEFAULT_MOVE_COST]` is the screen's matrix.
    prices: &[f64],
    // What this stage records beside its ordinary metrics, and whether it founds animals at
    // all. `RunOptions::default()` is every stage before workstream I.
    options: RunOptions,
    horizon_ticks: u64,
    sample_every: u64,
    apex_introduce_tick: u64,
    workers: usize,
    wall_seconds: u64,
    out: &Path,
) -> Result<StageReport, String> {
    if workers == 0 || workers > 8 {
        return Err("--workers must be in 1..=8 for this brief's compute cap".into());
    }
    let seeds = seed_set.seeds(seed_count)?;
    let chosen: Vec<&'static Candidate> = names
        .iter()
        .map(|n| {
            candidate(n).ok_or_else(|| {
                format!(
                    "{n} is not a declared candidate; known: {}",
                    CANDIDATES.iter().map(|c| c.name).collect::<Vec<_>>().join(", ")
                )
            })
        })
        .collect::<Result<_, String>>()?;
    for arm in arms {
        if !ARMS.contains(arm) {
            return Err(format!("apex arm {arm} is not one of {ARMS:?}"));
        }
    }
    if prices.is_empty() {
        return Err("--prices named no movement price".into());
    }
    let price_index = params::index_of("organism.move_cost")
        .ok_or("organism.move_cost is not in the parameter box")?;
    let (lo, hi) = (params::PARAMS[price_index].lo, params::PARAMS[price_index].hi);
    for price in prices {
        if !price.is_finite() || *price <= 0.0 {
            return Err(format!("movement price {price} is not a positive finite number"));
        }
        if *price < lo || *price > hi {
            return Err(format!(
                "movement price {price} is outside the declared box [{lo}, {hi}]; widen the box                  deliberately rather than running outside it"
            ));
        }
    }

    // The job list, in a fixed order, so the same matrix always produces the same rows in the
    // same places whatever order the workers finish in.
    struct Job {
        candidate: &'static Candidate,
        values: Vec<f64>,
        move_cost: f64,
        seed: u64,
        arm: u32,
    }
    let mut jobs = Vec::new();
    for price in prices {
        for c in &chosen {
            let mut values = c.vector()?;
            values[price_index] = *price;
            for seed in &seeds {
                for arm in arms {
                    jobs.push(Job {
                        candidate: c,
                        values: values.clone(),
                        move_cost: *price,
                        seed: *seed,
                        arm: *arm,
                    });
                }
            }
        }
    }

    let plan = StagePlan {
        stage: stage.to_string(),
        prices: prices.to_vec(),
        ledger: options.ledger,
        plant_record: options.plant_record,
        no_animals: options.no_animals,
        motor: options.motor.name().to_string(),
        pursuit_stop: options.pursuit_stop.as_str().to_string(),
        build_id: BUILD_ID.to_string(),
        horizon_ticks,
        sample_every,
        apex_introduce_tick,
        seed_set: seed_set.label().to_string(),
        seeds: seeds.clone(),
        arms: arms.to_vec(),
        candidates: chosen.iter().map(|c| c.name.to_string()).collect(),
        trials: jobs.len(),
        workers,
        wall_seconds_cap: wall_seconds,
    };

    std::fs::create_dir_all(out).map_err(|e| format!("creating {}: {e}", out.display()))?;
    let rows_path = out.join("evals.jsonl");
    let file = std::fs::File::create(&rows_path)
        .map_err(|e| format!("creating {}: {e}", rows_path.display()))?;
    let writer = Mutex::new(std::io::BufWriter::new(file));

    let start = Instant::now();
    let deadline = start + std::time::Duration::from_secs(wall_seconds);
    let cursor = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, CalibrationRow)>> = Mutex::new(Vec::new());
    let skipped = AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= jobs.len() {
                        return;
                    }
                    if Instant::now() >= deadline {
                        // Stop at the cap. The trial is *not* run and is counted as skipped;
                        // nothing is extended and nothing is silently shortened.
                        skipped.fetch_add(1, Ordering::SeqCst);
                        continue;
                    }
                    let job = &jobs[i];
                    let protocol = Protocol {
                        horizon_ticks,
                        sample_every,
                        apex_founders: job.arm,
                        apex_introduce_tick,
                    };
                    let evaluation =
                        evaluate_with(&job.values, job.seed, protocol, options);
                    let row = CalibrationRow {
                        candidate: job.candidate.name.to_string(),
                        axes: job.candidate.axes.to_string(),
                        arm: job.arm,
                        move_cost: job.move_cost,
                        seed_set: seed_set.label().to_string(),
                        stage: stage.to_string(),
                        params: params::labelled(&job.values),
                        param_bits: params::bit_labels(&job.values),
                        param_fingerprint: params::fingerprint(&job.values),
                        evaluation,
                    };
                    if let Ok(mut w) = writer.lock()
                        && let Ok(text) = serde_json::to_string(&row)
                    {
                        let _ = writeln!(w, "{text}");
                    }
                    results.lock().expect("calibration results mutex").push((i, row));
                }
            });
        }
    });

    let wall = start.elapsed().as_secs_f64();
    if let Ok(mut w) = writer.lock() {
        let _ = w.flush();
    }
    let mut results = results.into_inner().map_err(|e| format!("results mutex: {e}"))?;
    results.sort_by_key(|(i, _)| *i);
    let rows: Vec<CalibrationRow> = results.into_iter().map(|(_, r)| r).collect();
    let ticks: u64 = rows
        .iter()
        .map(|r| r.evaluation.metrics.as_ref().map_or(0, |m| m.ticks_run))
        .sum();

    // Aggregate per (candidate, arm).
    let mut cells = Vec::new();
    for price in prices {
        for c in &chosen {
        for arm in arms {
            let mine: Vec<&CalibrationRow> = rows
                .iter()
                .filter(|r| {
                    r.candidate == c.name
                        && r.arm == *arm
                        && r.move_cost.to_bits() == price.to_bits()
                })
                .collect();
            if mine.is_empty() {
                continue;
            }
            let (gates, foliage_retention, extinctions, worst_mass, worst_energy) =
                gates_for(&mine);
            let completed_metrics: Vec<Value> = mine
                .iter()
                .filter(|r| r.evaluation.status == Status::Completed)
                .filter_map(|r| r.evaluation.metrics.as_ref())
                .map(|m| serde_json::to_value(m).unwrap_or(Value::Null))
                .collect();
            cells.push(CellSummary {
                candidate: c.name.to_string(),
                axes: c.axes.to_string(),
                arm: *arm,
                move_cost: *price,
                seeds: mine.len(),
                completed: mine
                    .iter()
                    .filter(|r| r.evaluation.status == Status::Completed)
                    .count(),
                invalid: mine.iter().filter(|r| r.evaluation.status == Status::Invalid).count(),
                failed: mine.iter().filter(|r| r.evaluation.status == Status::Failed).count(),
                reason: mine.iter().find_map(|r| r.evaluation.reason.clone()),
                extinctions,
                worst_mass_residual: worst_mass,
                worst_energy_residual: worst_energy,
                mean: mean_value(&completed_metrics),
                foliage_retention,
                gates,
            });
        }
        }
    }

    let report = StageReport {
        plan,
        wall_seconds: wall,
        trials_run: rows.len(),
        trials_skipped: skipped.load(Ordering::SeqCst),
        ticks_simulated: ticks,
        ticks_per_second: if wall > 0.0 { ticks as f64 / wall } else { 0.0 },
        peak_rss_mib: peak_rss_mib(),
        cells,
    };
    let summary_path = out.join("summary.json");
    std::fs::write(
        &summary_path,
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("writing {}: {e}", summary_path.display()))?;
    Ok(report)
}

/// A stable 64-bit fingerprint of a configuration: FNV-1a over its canonical JSON encoding.
/// Deterministic for a given build because `serde_json` writes a struct's fields in
/// declaration order, and it covers **every** field, not only the searched ones — so a config
/// that differs anywhere has a different hash.
/// The motor contract a stage written before workstream T implies: there was one.
fn sweep_name() -> String {
    cubarium_core::MotorModel::Sweep.name().to_string()
}

/// The pursuit stopping rule a stage written before 2026-09-16 implies: the forward
/// half-space, which was the only rule the workspace had. Named rather than taken from
/// `PursuitStop::default()` so that adopting a new shipped rule cannot relabel a retained plan.
fn half_space_name() -> String {
    cubarium_core::hunter::PursuitStop::ForwardHalfSpace.as_str().to_string()
}

pub fn config_hash(config: &WorldConfig) -> u64 {
    let text = serde_json::to_string(config).unwrap_or_default();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// Export one candidate as a complete `WorldConfig` TOML that `cubarium run --config` accepts,
/// beside a JSON note recording what it is and how to reproduce it.
pub fn export(name: &str, seed: u64, out: &Path, selected: bool, why: &str) -> Result<PathBuf, String> {
    let c = candidate(name).ok_or_else(|| format!("{name} is not a declared candidate"))?;
    let config = c.config(seed)?;
    config
        .validate()
        .map_err(|e| format!("the exported config is not one the core accepts: {e}"))?;
    std::fs::create_dir_all(out).map_err(|e| format!("creating {}: {e}", out.display()))?;

    let toml_path = out.join(format!("{name}.toml"));
    let text = toml::to_string_pretty(&config)
        .map_err(|e| format!("serialising the config as TOML: {e}"))?;
    // A config that cannot be read back is not an export.
    let round_trip: WorldConfig =
        toml::from_str(&text).map_err(|e| format!("the exported TOML does not parse back: {e}"))?;
    if config_hash(&round_trip) != config_hash(&config) {
        return Err("the exported TOML does not round-trip to the same configuration".into());
    }
    std::fs::write(&toml_path, &text).map_err(|e| format!("writing {}: {e}", toml_path.display()))?;

    let values = c.vector()?;
    let note = serde_json::json!({
        "candidate": c.name,
        "axes": c.axes,
        "hypothesis": c.hypothesis,
        "selected": selected,
        "why": why,
        "build_id": BUILD_ID,
        "config_hash": format!("{:016x}", config_hash(&config)),
        "config_seed": seed,
        "param_bits": params::bit_labels(&values),
        "param_fingerprint": params::fingerprint(&values),
        "params": params::labelled(&values),
        "overrides": c.overrides.iter().map(|(k, v)| serde_json::json!({"name": k, "value": v}))
            .collect::<Vec<_>>(),
        "apex_profile": "FixedHunterProfile::lanternjaw_trial, unsearched and unchanged",
        "load_with": format!("cubarium run --config {}", toml_path.display()),
    });
    let note_path = out.join(format!("{name}.json"));
    std::fs::write(&note_path, serde_json::to_string_pretty(&note).map_err(|e| e.to_string())?)
        .map_err(|e| format!("writing {}: {e}", note_path.display()))?;
    Ok(toml_path)
}

/// A compact per-cell line for the terminal, so a stage is readable without opening the JSON.
pub fn print_report(report: &StageReport) {
    println!(
        "stage {} — {} trial(s) in {:.1}s on {} worker(s), {} skipped at the cap, {:.0} ticks/s, peak RSS {:.0} MiB",
        report.plan.stage,
        report.trials_run,
        report.wall_seconds,
        report.plan.workers,
        report.trials_skipped,
        report.ticks_per_second,
        report.peak_rss_mib,
    );
    println!(
        "horizon {} ticks ({:.0} sim min), sample every {}, apex at tick {}, seeds {:?} ({})",
        report.plan.horizon_ticks,
        report.plan.horizon_ticks as f64 * cubarium_core::DT / 60.0,
        report.plan.sample_every,
        report.plan.apex_introduce_tick,
        report.plan.seeds,
        report.plan.seed_set,
    );
    println!("movement prices {:?}", report.plan.prices);
    println!("per-body ledger  {}", if report.plan.ledger { "on" } else { "off" });
    println!("plant record     {}", if report.plan.plant_record { "on" } else { "off" });
    println!("founders         {}", if report.plan.no_animals { "none (plant-only)" } else { "ordinary" });
    println!(
        "\n{:<24} {:>9} {:>3} {:>4} {:>6} {:>9} {:>9} {:>8} {:>8} {:>7} {:>7}  gates",
        "candidate", "move_cost", "arm", "ok", "extinc", "foliage/0", "wood", "pop", "births",
        "deaths", "apexA",
    );
    for cell in &report.cells {
        let get = |path: &[&str]| -> f64 {
            let mut v = &cell.mean;
            for key in path {
                match v.get(key) {
                    Some(next) => v = next,
                    None => return f64::NAN,
                }
            }
            v.as_f64().unwrap_or(f64::NAN)
        };
        let opening = get(&["opening_foliage"]).max(1e-12);
        let flag = |on: bool, ch: char| if on { ch } else { '.' };
        println!(
            "{:<24} {:>9.5} {:>3} {:>4} {:>6} {:>9.3} {:>9.1} {:>8.1} {:>8.1} {:>7.1} {:>7.2}  {}{}{}{}{}{} {}",
            cell.candidate,
            cell.move_cost,
            cell.arm,
            cell.completed,
            cell.extinctions,
            get(&["late", "foliage_mean"]) / opening,
            get(&["late", "wood_mean"]),
            get(&["late", "population_mean"]),
            get(&["late", "prey_births"]),
            get(&["late", "prey_deaths"]),
            get(&["late", "apex_active_mean"]),
            flag(cell.gates.sound, 'S'),
            flag(cell.gates.persists, 'P'),
            flag(cell.gates.vegetated, 'V'),
            flag(cell.gates.turning_over, 'T'),
            flag(cell.gates.guilds_intact, 'G'),
            flag(cell.gates.stands_intact, 'C'),
            if cell.gates.plausible { "PLAUSIBLE" } else { "" },
        );
    }
    println!(
        "\ngates: S sound  P persists  V vegetated (late ΣP ≥ {:.0}% of opening)  T turning over  \
         G both prey guilds alive  C stands intact (≥ {:.0}% of opening alive cells)",
        FOLIAGE_FLOOR * 100.0,
        ALIVE_CELL_FLOOR * 100.0,
    );
}

/// Every declared candidate and what it moves.
pub fn print_candidates() {
    println!("build {BUILD_ID}\n{} declared candidates\n", CANDIDATES.len());
    for c in CANDIDATES {
        println!("{:<24} [{}]  {}", c.name, c.axes, c.hypothesis);
        if c.overrides.is_empty() {
            println!("{:<24}   (shipped defaults)", "");
        }
        for (name, value) in c.overrides {
            let k = params::index_of(name).expect("a declared candidate names a searched parameter");
            println!("{:<24}   {:<34} {} (default {})", "", name, value, params::PARAMS[k].default);
        }
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for c in CANDIDATES {
        for (name, _) in c.overrides {
            *counts.entry(name).or_default() += 1;
        }
    }
    println!("\nhow often each searched name is moved by the screen");
    for p in params::PARAMS {
        println!("  {:<34} {}", p.name, counts.get(p.name).copied().unwrap_or(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every declared candidate must name real parameters and build a world the core
    /// accepts.** A candidate that the core refuses is a legitimate finding when the *search*
    /// proposes it; one that is hand-written into this list is a typo, and this test is what
    /// tells them apart before an hour of compute is spent.
    #[test]
    fn every_candidate_names_searched_parameters_and_validates() {
        for c in CANDIDATES {
            let values = c.vector().unwrap_or_else(|e| panic!("{}: {e}", c.name));
            assert_eq!(values.len(), params::PARAMS.len());
            for (name, value) in c.overrides {
                let k = params::index_of(name).expect("checked by `vector`");
                let spec = params::PARAMS[k];
                assert!(
                    spec.lo <= *value && *value <= spec.hi,
                    "{}: {name} = {value} is outside the declared box [{}, {}]",
                    c.name,
                    spec.lo,
                    spec.hi
                );
                assert_ne!(
                    value.to_bits(),
                    spec.default.to_bits(),
                    "{}: {name} is overridden to its own default, which is not a candidate",
                    c.name
                );
            }
            let config = c.config(TRAINING_SEEDS[0]).unwrap_or_else(|e| panic!("{}: {e}", c.name));
            config
                .validate()
                .unwrap_or_else(|e| panic!("{} builds a config the core refuses: {e}", c.name));
            assert!(!c.hypothesis.is_empty(), "{} has no hypothesis", c.name);
            assert!(!c.axes.is_empty(), "{} names no axes", c.name);
        }
    }

    /// The baseline candidate must be the shipped defaults, bit for bit: the whole screen is
    /// read against it.
    #[test]
    fn the_baseline_candidate_is_the_shipped_default_vector() {
        let baseline = candidate("baseline").expect("the baseline is declared");
        assert!(baseline.overrides.is_empty());
        let values = baseline.vector().expect("the baseline builds");
        for (v, p) in values.iter().zip(params::PARAMS) {
            assert_eq!(v.to_bits(), p.default.to_bits(), "{} moved", p.name);
        }
        let config = baseline.config(7).expect("the baseline config builds");
        assert_eq!(config_hash(&config), config_hash(&crate::evaluate::base_config(7)));
    }

    /// Candidate names are unique, and every candidate but the baseline moves at least two
    /// searched names — the brief asks for a *joint* screen, not a one-at-a-time sweep.
    #[test]
    fn candidates_are_unique_and_joint() {
        let mut seen = std::collections::BTreeSet::new();
        for c in CANDIDATES {
            assert!(seen.insert(c.name), "{} is declared twice", c.name);
            if c.name != "baseline" {
                assert!(
                    c.overrides.len() >= 2,
                    "{} moves only {} parameter(s); the screen is a joint one",
                    c.name,
                    c.overrides.len()
                );
            }
        }
    }

    /// Exporting a candidate produces a TOML the core reads back to the same configuration.
    #[test]
    fn an_exported_candidate_round_trips_through_toml() {
        let dir = std::env::temp_dir().join(format!("cubarium-calibrate-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = export("joint-strong", 1_001, &dir, false, "test").expect("export");
        let text = std::fs::read_to_string(&path).expect("the export is readable");
        let back: WorldConfig = toml::from_str(&text).expect("the export parses");
        back.validate().expect("the export is a valid config");
        let direct = candidate("joint-strong").unwrap().config(1_001).unwrap();
        assert_eq!(config_hash(&back), config_hash(&direct));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The three movement prices this workstream runs must all be inside the declared box, and
    /// the control price must be the value the world actually ships — otherwise the "control"
    /// arm is not the screen's world and the reproduction check could not be met.
    #[test]
    fn the_declared_movement_prices_are_the_shipped_default_and_two_inside_the_box() {
        let k = params::index_of("organism.move_cost").expect("the price is in the box");
        let spec = params::PARAMS[k];
        assert_eq!(
            spec.default.to_bits(),
            DEFAULT_MOVE_COST.to_bits(),
            "the declared default price is not the shipped one"
        );
        assert_eq!(
            WorldConfig::default().organism.move_cost.to_bits(),
            DEFAULT_MOVE_COST.to_bits(),
            "the shipped config no longer carries the price this module calls the default"
        );
        for price in [0.00036, 0.0018, 0.006] {
            assert!(
                spec.lo <= price && price <= spec.hi,
                "{price} is outside the declared box [{}, {}]",
                spec.lo,
                spec.hi
            );
        }
    }

    /// Workstream I's ladder is the brief's five levels, every rung is inside the declared box,
    /// the three interior rungs are 0.0003 apart, and the two rungs it shares with workstream F
    /// are bit-for-bit F's — which is what makes the state-hash row check possible.
    ///
    /// The end intervals are **not** 0.0003 (0.00024 at the bottom, 0.0006 at the top) because
    /// the two end rungs are F's, not this campaign's to choose. The note's pre-registration
    /// calls the ladder "three equal steps of 0.0003"; that sentence is wrong about the end
    /// intervals and the error is disclosed below its rule rather than edited out of it.
    #[test]
    fn the_ladder_is_the_declared_five_levels_inside_the_box_sharing_two_rungs_with_the_matrix() {
        let k = params::index_of("organism.move_cost").expect("the price is in the box");
        let spec = params::PARAMS[k];
        for price in LADDER_PRICES {
            assert!(
                spec.lo <= price && price <= spec.hi,
                "{price} is outside the declared box [{}, {}]",
                spec.lo,
                spec.hi
            );
        }
        assert_eq!(
            LADDER_PRICES[0].to_bits(),
            DEFAULT_MOVE_COST.to_bits(),
            "the ladder's control rung is not the shipped price"
        );
        assert_eq!(LADDER_PRICES[4].to_bits(), 0.0018f64.to_bits(), "F's lowest raised level");
        assert_eq!(
            LADDER_PRICES,
            [0.00036, 0.0006, 0.0009, 0.0012, 0.0018],
            "the ladder is not the brief's five levels"
        );
        for pair in LADDER_PRICES[1..4].windows(2) {
            assert!(
                (pair[1] - pair[0] - 0.0003).abs() < 1e-12,
                "the interior rungs are not 0.0003 apart: {pair:?}"
            );
        }
        assert!(
            LADDER_PRICES.windows(2).all(|p| p[1] > p[0]),
            "the ladder must be strictly increasing"
        );
    }

    /// Writing the price at its shipped default must change **no bit** of the configuration,
    /// so the movement campaign's control arm is the calibration screen's world exactly.
    #[test]
    fn writing_the_default_price_leaves_every_candidate_config_bit_for_bit_unchanged() {
        let k = params::index_of("organism.move_cost").expect("the price is in the box");
        for name in ["baseline", "fast-leaf"] {
            let c = candidate(name).expect("declared");
            let mut values = c.vector().expect("builds");
            let with_default = {
                let mut config = crate::evaluate::base_config(1);
                let mut profile = FixedHunterProfile::lanternjaw_trial(&config);
                params::apply(&values, &mut config, &mut profile).expect("applies");
                config_hash(&config)
            };
            assert_eq!(with_default, config_hash(&c.config(1).expect("builds")));
            // And a raised price must change it, or the knob is not connected.
            values[k] = 0.006;
            let mut config = crate::evaluate::base_config(1);
            let mut profile = FixedHunterProfile::lanternjaw_trial(&config);
            params::apply(&values, &mut config, &mut profile).expect("applies");
            assert_ne!(
                config_hash(&config),
                with_default,
                "{name}: raising the movement price changed no configuration bit"
            );
        }
    }

    /// A price outside the declared box is refused before any compute is spent, rather than
    /// quietly clamped into it.
    #[test]
    fn a_movement_price_outside_the_box_is_refused() {
        let dir = std::env::temp_dir().join(format!("cubarium-price-box-{}", std::process::id()));
        let err = run_stage(
            "test",
            &["baseline".to_string()],
            SeedSet::Training,
            1,
            &[0],
            &[0.5],
            RunOptions::default(),
            100,
            50,
            10,
            1,
            1,
            &dir,
        )
        .expect_err("a price outside the box is refused");
        assert!(err.contains("outside the declared box"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The gates are conjunctive and censoring is not a pass: a cell with no late window
    /// fails every ecological gate rather than defaulting to true.
    #[test]
    fn a_censored_cell_is_not_plausible() {
        let row = CalibrationRow {
            candidate: "baseline".into(),
            axes: "-".into(),
            arm: 0,
            move_cost: DEFAULT_MOVE_COST,
            seed_set: "training".into(),
            stage: "test".into(),
            params: params::labelled(&params::defaults()),
            param_bits: params::bit_labels(&params::defaults()),
            param_fingerprint: params::fingerprint(&params::defaults()),
            evaluation: Evaluation {
                status: Status::Completed,
                reason: None,
                seed: 1_001,
                protocol: Protocol::default(),
                build_id: BUILD_ID.to_string(),
                elapsed_ms: 0,
                apex_material_in: 0.0,
                apex_energy_in: 0.0,
                metrics: Some(crate::metrics::Components {
                    opening_foliage: 10.0,
                    ..crate::metrics::Components::default()
                }),
                movement: None,
                opening: None,
                founding_state_hash: None,
            },
        };
        let (gates, retention, extinctions, _, _) = gates_for(&[&row]);
        assert!(gates.sound, "the run itself completed");
        assert!(!gates.vegetated, "a censored late window is not vegetation");
        assert!(!gates.plausible);
        assert_eq!(retention, vec![0.0]);
        assert_eq!(extinctions, 1, "a world with no organisms is an extinction");
    }
}
