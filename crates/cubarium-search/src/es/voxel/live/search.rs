//! **The ecology half**: a world-level search over a search genome
//! (`design/handoffs/voxel-ecology-search-2026-09-24.md`; R3 and targets v2,
//! `design/handoffs/cotrain-stage0-2026-10-01.md` §2–3).
//!
//! A candidate is a [`Genome`] (a search file's genes, `config/training/search/`), searched
//! in log space by CMA-ES ([`crate::es::cma`]). Each candidate is first **screened**
//! ([`super::screen`]): one whose R, S or F falls outside the bands takes a fixed penalty
//! without a run. The rest are scored on **freshly founded** worlds — the `--world` (the
//! training preset by default), coarse water — run for a horizon with the brains held at
//! fixed centres (a lineage given none runs its heuristic), and the score is a
//! [`penalty`]: zero inside the band, growing with the normalised distance outside it.
//!
//! **The band** (targets v2, Wrysk 2026-10-01: persist, breed, bounded):
//! - every lineage the world founded (the lanternjaw, founded as a dormant egg bank,
//!   included: its eggs count as its living) and every plant species the default world
//!   keeps on that seed persists;
//! - every founded lineage has births in the second half;
//! - animals per world stay in `[lo, hi]` over the second half and never above `peak`:
//!   by default 1x, 3x and 5x the world's founding animal count ([`BandSpec`]);
//! - browsers move at least `moving_min` of their minutes;
//! - lanternjaws are 5–15 % of the animals over the second half.
//!
//! Common random numbers: every candidate of a generation (and the default genome,
//! evaluated beside them as the persistence reference) runs on the same founding seeds;
//! the seeds change from one generation to the next.
//!
//! A world that booms past [`EvalSpec::abort_animals`] or loses a lineage stops early;
//! its trace carries its last count to the horizon, which is the trend the penalty reads.
//!
//! **Unified mode** (`--brain-pairs > 0`): the same worlds also carry antithetic brain
//! variants for every lineage with a centre, and each of those takes one ES step per
//! generation from the pooled body scores, as the live trainer does.

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use cubarium_voxel::genome::Genome;
use cubarium_voxel_fauna::Founder;
use cubarium_voxel_flora::Species;
use serde::Serialize;

use super::curiosity::{Curiosity, Memory};
use super::ecology::EcologyParams;
use super::readout::{LineageStats, Scan};
use super::screen::{RosterScreen, Rsf, ScreenSpec, Verdict, roster, rsf};
use super::targets::{LineageOutcome, StageSpec, judge};
use super::threads::{ThreadBudget, parallel_shared};
use super::trainer::{ascend, parallel, pooled_scores};
use super::accounting::{Presence, Recruitment, RequiredRoster, retain_losses};
use super::variants::VariantTable;
use super::window::{self, BodyScore, TICKS_PER_MIN, Window};
use super::world;
use crate::es::cma::Cma;
use crate::es::optimizer::Adam;
use crate::es::voxel::EpisodeDriver;
use crate::es::voxel::store::VoxelPolicyFile;

/// The band's settings, as the command line gives them.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct BandSpec {
    /// Animals per world over the second half, at least this × the founding count.
    pub lo_x: f64,
    /// At most this × the founding count.
    pub hi_x: f64,
    /// Never above this × the founding count.
    pub peak_x: f64,
    /// Browser moving share (body-minutes over 0.1 BL/s), second half.
    pub moving_min: f64,
    /// Per lineage or plant species lost, and per lineage silent in the second half.
    pub missing_cost: f64,
    /// Penalty per absent declared member, independent of paired/readout exclusions.
    pub required_loss_weight: f64,
    /// Disable absolute penalties only for diagnostic maps with deliberate omissions.
    pub absolute_persistence: bool,
    /// Experimental reward uses body + reserve + escrow + digestible stomach.
    /// False preserves the current reward; both deltas are always recorded.
    pub stomach_aware_reward: bool,
    /// The lanternjaws' share of the animals over the second half.
    pub lanternjaw_share: [f64; 2],
    /// Lineages held to persistence alone (by [`Founder::index`]): no late-breeding target
    /// and, for the lanternjaw, no share target; living bodies plus viable eggs persist.
    /// Stage 1 holds the lanternjaw so, whose egg bank first hatches at the first dusk
    /// (~60 min on the terrarium's 2 h day). The search file's `[targets] persist_only`.
    pub persist_only: [bool; Founder::COUNT],
    /// The second-half readiness ([`body_readiness`]) at which a lineage with no late
    /// birth stops paying for it: `missing_cost × (1 − min(1, readiness / this))`. The
    /// search file's `[targets] ready_target`.
    pub ready_target: f64,
    /// Lineages exempt from the breeding term alone (persistence and share still apply):
    /// the lanternjaw, whose own eggs cannot hatch before the next dusk. `[targets]
    /// breed_exempt`.
    pub breed_exempt: [bool; Founder::COUNT],
    /// The weight on the moving-share term (`lively`, 0 to 1 before it), so a still
    /// browser costs on the scale of a lineage that does not breed. `[targets]
    /// lively_weight`.
    pub lively_weight: f64,
    /// **Targets v3** (`[targets] version = 3`, [`super::targets`]): per lineage and life
    /// stage. Replaces the breeding term when set.
    pub stage: Option<StageSpec>,
}

impl BandSpec {
    /// The search file's `[targets]` table: `persist_only = ["lanternjaw", ...]`.
    pub fn with_targets(mut self, rest: &toml::Table) -> Result<BandSpec, String> {
        let Some(t) = rest.get("targets") else {
            return Ok(self);
        };
        let t = t.as_table().ok_or("[targets] is a table")?;
        self.stage = StageSpec::from_targets(t)?;
        if let Some(s) = &self.stage {
            self.ready_target = s.ready_target;
        }
        for (k, v) in t {
            match k.as_str() {
                "version" | "birth_required" | "progress" | "replenish" | "birth_credit_max"
                | "lanternjaw_progress" | "chorister_pup_survival" | "replenish_weight"
                | "share_weight_unestablished" | "balance_share" | "readout_only" | "balance"
                | "balance_weight" | "counts_readout"
                    if self.stage.is_some() => {}
                "required_loss_weight" => {
                    self.required_loss_weight = v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
                        .filter(|x| *x >= 0.0 && x.is_finite()).ok_or("required_loss_weight is a number >= 0")?;
                }
                "absolute_persistence" => {
                    self.absolute_persistence = v.as_bool().ok_or("absolute_persistence is a boolean")?;
                }
                "stomach_aware_reward" => {
                    self.stomach_aware_reward = v.as_bool().ok_or("stomach_aware_reward is a boolean")?;
                }
                "persist_only" => {
                    let names = v.as_array().ok_or("persist_only is a list of lineages")?;
                    self.persist_only = [false; Founder::COUNT];
                    for n in names {
                        let n = n.as_str().ok_or("persist_only names lineages")?;
                        let f = Founder::ALL
                            .into_iter()
                            .find(|f| f.name() == n)
                            .ok_or_else(|| format!("[targets] persist_only: no lineage {n:?}"))?;
                        self.persist_only[f.index()] = true;
                    }
                }
                "breed_exempt" => {
                    let names = v.as_array().ok_or("breed_exempt is a list of lineages")?;
                    self.breed_exempt = [false; Founder::COUNT];
                    for n in names {
                        let n = n.as_str().ok_or("breed_exempt names lineages")?;
                        let f = Founder::ALL
                            .into_iter()
                            .find(|f| f.name() == n)
                            .ok_or_else(|| format!("[targets] breed_exempt: no lineage {n:?}"))?;
                        self.breed_exempt[f.index()] = true;
                    }
                }
                "lively_weight" => {
                    self.lively_weight = v
                        .as_float()
                        .or_else(|| v.as_integer().map(|i| i as f64))
                        .filter(|x| *x >= 0.0 && x.is_finite())
                        .ok_or("lively_weight is a number >= 0")?;
                }
                "ready_target" => {
                    let v = v
                        .as_float()
                        .or_else(|| v.as_integer().map(|i| i as f64))
                        .filter(|x| *x > 0.0 && x.is_finite())
                        .ok_or("ready_target is a positive number")?;
                    self.ready_target = v;
                }
                other => return Err(format!("unknown key `{other}` in [targets]")),
            }
        }
        Ok(self)
    }
}

impl Default for BandSpec {
    fn default() -> BandSpec {
        BandSpec {
            lo_x: 1.0,
            hi_x: 3.0,
            peak_x: 5.0,
            moving_min: 0.30,
            missing_cost: 10.0,
            required_loss_weight: 100.0,
            absolute_persistence: true,
            stomach_aware_reward: false,
            lanternjaw_share: [0.05, 0.15],
            persist_only: [false; Founder::COUNT],
            ready_target: 0.5,
            breed_exempt: [false; Founder::COUNT],
            lively_weight: 10.0,
            stage: None,
        }
    }
}

/// The band one world is scored against.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Band {
    pub lo: f64,
    pub hi: f64,
    pub peak: f64,
    pub moving_min: f64,
    pub missing_cost: f64,
    /// `None` for a world founded without the lanternjaw.
    pub lanternjaw_share: Option<[f64; 2]>,
    /// The weight on the moving-share term ([`BandSpec::lively_weight`]).
    pub lively_weight: f64,
}

impl Band {
    /// The band of a world founded with `animals` bodies (and the lanternjaw's egg bank,
    /// if `lanternjaw`).
    pub fn for_founding(animals: usize, lanternjaw: bool, spec: &BandSpec) -> Band {
        let n = animals.max(1) as f64;
        Band {
            lo: spec.lo_x * n,
            hi: spec.hi_x * n,
            peak: spec.peak_x * n,
            moving_min: spec.moving_min,
            missing_cost: spec.missing_cost,
            lanternjaw_share: (lanternjaw && !spec.persist_only[Founder::Lanternjaw.index()])
                .then_some(spec.lanternjaw_share),
            lively_weight: spec.lively_weight,
        }
    }
}

/// What a candidate world did, as the penalty reads it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Trace {
    pub horizon_min: f64,
    /// Sample times, simulated minutes (evenly spaced).
    pub minutes: Vec<f64>,
    /// Animals in the world at each sample.
    pub animals: Vec<usize>,
    /// Lanternjaw bodies at each sample.
    #[serde(default)]
    pub lanternjaws: Vec<usize>,
    /// Browser moving share over the second half (or all it ran, if it stopped before).
    pub browser_moving: f64,
    /// Lineages and plant species (present at the start) absent at the end.
    pub missing: usize,
    /// Absolute losses of the declared pre-founding/pre-burn-in roster.
    #[serde(default)]
    pub lost_required: usize,
    #[serde(default)]
    pub lost_required_names: Vec<String>,
    /// Effective weight (zero in diagnostic mode); always report the losses.
    #[serde(default)]
    pub required_loss_weight: f64,
    /// The breeding shortfall, summed over the lineages held to breeding
    /// ([`breeding_shortfall`]): 1 for a lineage lost or with no late birth and no
    /// readiness, graded below that by its readiness, 0 with late births. Costs
    /// `missing_cost` per unit.
    #[serde(default)]
    pub silent_lineages: f64,
    /// Targets v3: the stage shortfalls summed over the lineages ([`super::targets::judge`]),
    /// costing `missing_cost` per unit, in place of `silent_lineages`.
    #[serde(default)]
    pub stage: f64,
    /// Targets v3: the replenishment term, already weighted.
    #[serde(default)]
    pub replenish: f64,
    /// Targets v3: the lanternjaw share term is discounted by this (1 − its weight while no
    /// lanternjaw lives free).
    #[serde(default)]
    pub share_discount: f64,
    /// The balance term ([`super::balance::shortfall`] summed over the lineages scored on
    /// it), already weighted.
    #[serde(default)]
    pub balance: f64,
    /// Alive counts are readouts: the band, its peak and the lanternjaw share are reported
    /// in the penalty but left out of its total ([`StageSpec::counts_readout`]).
    #[serde(default)]
    pub counts_readout: bool,
}

/// The penalty and its parts.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Penalty {
    pub bound: f64,
    pub peak: f64,
    pub lively: f64,
    /// The lanternjaw share.
    pub share: f64,
    pub persist: f64,
    /// Targets v3: the stage part of `persist`, and the replenishment term.
    #[serde(default)]
    pub stage: f64,
    #[serde(default)]
    pub replenish: f64,
    /// The balance term ([`Trace::balance`]).
    #[serde(default)]
    pub balance: f64,
    /// Weighted absolute persistence, never forgiven by the reference.
    #[serde(default)]
    pub absolute_persist: f64,
    pub total: f64,
}

/// Normalised distance of `x` outside `[lo, hi]`.
fn outside(x: f64, lo: f64, hi: f64) -> f64 {
    (lo - x).max(0.0) / lo.max(1e-12) + (x - hi).max(0.0) / hi.max(1e-12)
}

/// Zero inside the band; outside it the normalised distance: the mean over the second
/// half's samples of `(lo − n)/lo` below and `(n − hi)/hi` above, plus `(peak_n −
/// peak)/peak`, plus `(moving_min − m)/moving_min`, plus the lanternjaw share's mean
/// distance outside its band, plus `missing_cost` per thing lost or silent. A trace that
/// stopped early is carried at its last sample to the horizon.
pub fn penalty(t: &Trace, band: &Band) -> Penalty {
    let step = if t.minutes.len() >= 2 {
        t.minutes[1] - t.minutes[0]
    } else {
        1.0
    };
    let lj_at = |i: usize| t.lanternjaws.get(i).or(t.lanternjaws.last()).copied().unwrap_or(0);
    let mut samples: Vec<(f64, f64, f64)> = t
        .minutes
        .iter()
        .zip(&t.animals)
        .enumerate()
        .map(|(i, (m, n))| (*m, *n as f64, lj_at(i) as f64))
        .collect();
    if let Some(&(mut m, n, lj)) = samples.last() {
        while m + step <= t.horizon_min + 1e-9 {
            m += step;
            samples.push((m, n, lj));
        }
    }
    let half = t.horizon_min / 2.0;
    let late: Vec<&(f64, f64, f64)> = samples.iter().filter(|(m, _, _)| *m > half).collect();
    let mean = |f: &dyn Fn(&(f64, f64, f64)) -> f64| {
        if late.is_empty() {
            0.0
        } else {
            late.iter().map(|s| f(s)).sum::<f64>() / late.len() as f64
        }
    };
    let bound = mean(&|s| outside(s.1, band.lo, band.hi));
    let share = match band.lanternjaw_share {
        Some([lo, hi]) => mean(&|s| outside(if s.1 > 0.0 { s.2 / s.1 } else { 0.0 }, lo, hi)),
        None => 0.0,
    };
    let peak_n = samples.iter().map(|s| s.1).fold(0.0, f64::max);
    let peak = (peak_n - band.peak).max(0.0) / band.peak;
    let lively =
        band.lively_weight * (band.moving_min - t.browser_moving).max(0.0) / band.moving_min;
    let share = share * (1.0 - t.share_discount).clamp(0.0, 1.0);
    let stage = band.missing_cost * t.stage;
    let persist = band.missing_cost * (t.missing as f64 + t.silent_lineages) + stage;
    let counts = if t.counts_readout { 0.0 } else { bound + peak + share };
    let absolute_persist = t.required_loss_weight * t.lost_required as f64;
    Penalty {
        absolute_persist,
        bound,
        peak,
        lively,
        share,
        persist,
        stage,
        replenish: t.replenish,
        balance: t.balance,
        total: counts + lively + persist + t.replenish + t.balance + absolute_persist,
    }
}

/// How one candidate world is run.
#[derive(Clone, Debug, Serialize)]
pub struct EvalSpec {
    /// The world every candidate is founded on.
    #[serde(skip)]
    pub world: cubarium_voxel::Config,
    /// Founders per lineage, in [`Founder::ALL`] order (the lanternjaw's are clutches).
    pub founders: [usize; Founder::COUNT],
    /// Fixed declared roster across candidates; None derives it before placement.
    pub required_roster: Option<RequiredRoster>,
    pub horizon_ticks: u64,
    pub window_ticks: u64,
    pub water_every: u32,
    /// Stop a world that holds more animals than this.
    pub abort_animals: usize,
    /// Stop a world once a founded lineage is lost (the search's early stop; the exhibit
    /// and finalist runs keep going to show every lineage's whole horizon).
    pub stop_on_loss: bool,
    pub band: BandSpec,
    /// The horizon the band is scored on, ticks. Past what was run (`horizon_ticks`, or
    /// an early stop) the animal count is projected on its last hour's trend
    /// ([`project`]). Equal to `horizon_ticks` is no projection.
    pub score_horizon_ticks: u64,
    /// Each lineage's curiosity in the brains' score (`--curiosity`; weight 0 without).
    /// A training knob, not an ecology gene: the CMA never sees it.
    pub curiosity: Curiosity,
    /// File/control labels accompanying the installed driver digests.
    pub driver_sources: Vec<(Founder, String)>,
    /// Found exactly the world `base` names (`Seeds::Asked`), not the first accepted from
    /// it: the grid keeps one world per seed across its cells.
    pub asked_seed: bool,
    /// Where every candidate starts ([`super::start`]): today's founding (E0) or a start
    /// library.
    #[serde(skip)]
    pub start: super::start::StartSet,
    /// The burn-in before scoring.
    pub burn_in: super::start::BurnIn,
    /// Runtime-only staged run protocol.
    #[serde(skip)]
    pub staging: Option<super::staging::Segment>,
}

impl EvalSpec {
    /// `horizon_ticks` of `world` in windows of `window_ticks`, founded as the live world
    /// is (the egg bank included), the default band, the experiment's coarse water, no
    /// projection.
    pub fn new(world: cubarium_voxel::Config, horizon_ticks: u64, window_ticks: u64) -> EvalSpec {
        EvalSpec {
            world,
            // The shipped founding table: a search's candidates share it (no count is a
            // gene), and a lineage at 0 founds nothing and has no target or score term.
            founders: cubarium_voxel_fauna::FaunaConfig::default().founder_counts(),
            required_roster: None,
            horizon_ticks,
            window_ticks,
            water_every: cubarium_voxel_sim::EXPERIMENT_WATER_EVERY,
            abort_animals: 240,
            stop_on_loss: true,
            band: BandSpec::default(),
            score_horizon_ticks: horizon_ticks,
            curiosity: Curiosity::default(),
            driver_sources: Vec::new(),
            asked_seed: false,
            start: super::start::StartSet::E0,
            burn_in: super::start::BurnIn::default(),
            staging: None,
        }
    }
}

/// Extend a trace to `to_min` on the exponential trend of its last hour: rate
/// `ln(n_end / n_end−60) / 60` per minute, clamped to ±0.05, the count capped at 2000.
/// Returns the minute the projection starts from.
pub fn project(t: &mut Trace, to_min: f64) -> f64 {
    let Some(&last_min) = t.minutes.last() else {
        return 0.0;
    };
    let n_end = *t.animals.last().expect("paired") as f64;
    let back = t
        .minutes
        .iter()
        .position(|m| *m >= last_min - 60.0)
        .unwrap_or(0);
    let span = (last_min - t.minutes[back]).max(1.0);
    let rate = if span >= 30.0 {
        ((n_end.max(1.0)) / (t.animals[back] as f64).max(1.0)).ln() / span
    } else {
        0.0
    };
    let rate = rate.clamp(-0.05, 0.05);
    let step = if t.minutes.len() >= 2 {
        t.minutes[1] - t.minutes[0]
    } else {
        1.0
    };
    let lj_share = match (t.lanternjaws.last(), t.animals.last()) {
        (Some(&lj), Some(&n)) if n > 0 => lj as f64 / n as f64,
        _ => 0.0,
    };
    let mut m = last_min;
    while m + step <= to_min + 1e-9 {
        m += step;
        t.minutes.push(m);
        let n = (n_end * (rate * (m - last_min)).exp()).min(2000.0).round() as usize;
        t.animals.push(n);
        if !t.lanternjaws.is_empty() {
            t.lanternjaws.push((lj_share * n as f64).round() as usize);
        }
    }
    t.horizon_min = to_min;
    last_min
}

/// One candidate world's result.
#[derive(Clone, Debug, Serialize)]
pub struct Evaluation {
    pub base_seed: u64,
    pub world_seed: u64,
    pub trace: Trace,
    pub penalty: Penalty,
    /// The band it was scored against (from its founding count).
    pub band: Band,
    /// Why it stopped early, if it did.
    pub stopped: Option<String>,
    /// The minute the scored trace turns from measured to projected, if it does.
    pub projected_from_min: Option<f64>,
    /// Every readout factor, averaged over the windows it ran.
    pub readout_mean: Vec<(String, f64)>,
    /// And as the last window left it.
    pub readout_end: Vec<(String, f64)>,
    pub species_start: Vec<usize>,
    pub species_end: Vec<usize>,
    /// Bodies per lineage at the end.
    pub lineages_end: Vec<usize>,
    /// Eggs (clutches' eggs) per lineage at the end.
    pub eggs_end: Vec<usize>,
    /// Founders per lineage (the lanternjaw's are clutches).
    pub founded: Vec<usize>,
    /// Births per lineage in the second half of what was run.
    pub late_births: Vec<u64>,
    /// Second-half mean breeding readiness per lineage ([`body_readiness`]).
    pub readiness: Vec<f64>,
    /// **Per lineage and life stage** (targets v3, [`super::targets`]): what each founded
    /// lineage did and its verdict. Judged under the search's `[targets]` when it is v3,
    /// else under every lineage's own default rule as a readout.
    #[serde(default)]
    pub lineages: Vec<LineageOutcome>,
    /// **Predator balance**, measured (a readout; Astra A6): per founded hunter.
    #[serde(default)]
    pub balance: Vec<BalanceReadout>,
    /// The readout-only lineages it was scored without (Astra F1).
    #[serde(default)]
    pub readout_only: Vec<bool>,
    pub ticks: u64,
    pub wall_s: f64,
    pub body_scores: Vec<BodyScore>,
    pub required_roster: RequiredRoster,
    pub presence_end: Presence,
    pub lost_required: usize,
    pub lost_required_names: Vec<String>,
    pub recruitment: Recruitment,
    /// How it started ([`super::start::StartRecord`]).
    #[serde(default)]
    pub start: super::start::StartRecord,
    /// The ledger baselines: the burn-in's end and every window boundary.
    #[serde(default)]
    pub baselines: Vec<super::start::Baseline>,
}

/// One hunter's measured balance (Astra A6's conditional ceiling, on this world's own
/// births): `share` of the prey lineages' births per world-day, each valued as an eventual
/// full adult (`body_max × (1 + reserve_cap)` at the hunter's carrion yield), over one
/// hunter's basal day; and the carrion its mouths actually took per hunter-day.
#[derive(Clone, Debug, Serialize)]
pub struct BalanceReadout {
    pub predator: &'static str,
    pub prey_births_per_day: f64,
    pub supported: f64,
    pub carrion_per_predator_day: f64,
}

/// What the targets read off the fauna's ledger, per lineage, at one moment.
#[derive(Clone, Debug, Default)]
struct Snap {
    recruitment: Recruitment,
    assimilated: [f64; Founder::COUNT],
    hatched: [u64; Founder::COUNT],
    laid: [u64; Founder::COUNT],
    hits: [u64; Founder::COUNT],
    carrion: [f64; Founder::COUNT],
    riders_attached: u64,
    rider_dropoffs: u64,
    riders_freed: u64,
    litters: u64,
    pups_born: u64,
    provision_received: f64,
    pack_kills: u64,
    /// Body, reserve and open escrow over each lineage's living bodies.
    biomass: [f64; Founder::COUNT],
    /// The whole ledger, for the balance measures' books and kills.
    ledger: Box<cubarium_voxel_fauna::FaunaLedger>,
    /// Their stomachs' digestible organic ([`super::targets::digestible_stomach`]).
    stomach: [f64; Founder::COUNT],
}

impl Snap {
    fn of(sim: &cubarium_voxel_sim::Sim) -> Snap {
        let view = sim.fauna().view();
        let l = view.ledger;
        let mut biomass = [0.0; Founder::COUNT];
        let mut stomach = [0.0; Founder::COUNT];
        for a in view.animals.iter() {
            let Some(f) = a.founder else { continue };
            biomass[f.index()] +=
                a.body + a.reserve + a.reproduction.escrow.map_or(0.0, |e| e.organic);
            stomach[f.index()] +=
                super::targets::digestible_stomach(a, sim.fauna().config().founder(f));
        }
        Snap {
            recruitment: Recruitment::of(sim.fauna()),
            assimilated: l.assimilated_by_founder,
            hatched: l.eggs_hatched_by_founder,
            laid: l.eggs_laid_by_founder,
            hits: l.hits_by_founder,
            carrion: l.eaten_carrion_by_founder,
            riders_attached: l.riders_attached,
            rider_dropoffs: l.rider_dropoffs,
            riders_freed: l.riders_freed,
            litters: l.litters,
            pups_born: l.pups_born,
            provision_received: l.provision_received_organic,
            pack_kills: l.pack_chase_kills,
            biomass,
            stomach,
            ledger: Box::new(*l),
        }
    }
}

/// Eggs per lineage now.
fn eggs(sim: &cubarium_voxel_sim::Sim) -> Vec<usize> {
    let v = sim.fauna().view();
    Founder::ALL
        .iter()
        .map(|f| v.eggs_by_founder(*f) as usize)
        .collect()
}

/// What each lineage was founded with: founder bodies plus founder eggs (the lanternjaw is
/// founded as an egg bank, and its seeding counts no bodies).
pub fn founded_lineages(founders: &[usize], eggs: &[usize]) -> Vec<usize> {
    founders.iter().zip(eggs).map(|(b, e)| b + e).collect()
}

/// [`lost_lineages`] without the readout-only ones (Astra F1): what the search scores
/// and stops on.
pub fn scored_lost(founded: &[usize], bodies: &[usize], eggs: &[usize], readout: &[bool; Founder::COUNT]) -> usize {
    (0..Founder::COUNT)
        .filter(|&i| !readout[i] && founded[i] > 0 && bodies[i] + eggs[i] == 0)
        .count()
}

/// The animals the population band counts: every lineage's bodies but the readout-only
/// ones' (their endowments neither help nor hurt the band; Astra F1).
pub fn scored_animals(pop: &[usize; Founder::COUNT], readout: &[bool; Founder::COUNT]) -> usize {
    (0..Founder::COUNT).filter(|&i| !readout[i]).map(|i| pop[i]).sum()
}

/// The lineages a world founded (bodies or clutches) that hold neither now.
pub fn lost_lineages(founded: &[usize], bodies: &[usize], eggs: &[usize]) -> usize {
    (0..Founder::COUNT)
        .filter(|&i| founded[i] > 0 && bodies[i] + eggs[i] == 0)
        .count()
}

/// **A body's breeding progress**, 0 to 1: 1 while it gestates (its escrow open);
/// otherwise half its reserve toward the surplus line (`min(1, reserve /
/// surplus_reserve)`) plus half its hold (`reproduction.surplus_ticks`, what
/// `advance_eligibility` keeps from the window, over the ticks the hold requires to
/// begin, [`cubarium_voxel_fauna::Reproduction::hold_required`], capped at 1). A body below
/// `birth_body` has only its reserve half: it cannot hold surplus yet.
pub fn body_readiness(
    a: &cubarium_voxel_fauna::Animal,
    sc: &cubarium_voxel_fauna::SpeciesConfig,
) -> f64 {
    if a.reproduction.escrow.is_some() {
        return 1.0;
    }
    let line = sc.reproduction.surplus_reserve(sc);
    let reserve = if line > 0.0 { (a.reserve / line).clamp(0.0, 1.0) } else { 1.0 };
    if a.body < sc.birth_body {
        return 0.5 * reserve;
    }
    let need = sc.reproduction.hold_required().max(1);
    let hold = (a.reproduction.surplus_ticks as f64 / need as f64).min(1.0);
    0.5 * reserve + 0.5 * hold
}

/// **The breeding term**, summed over the founded lineages held to breeding (not
/// `persist_only`): 0 for one with late births; 1 for one `lost`, or for every one when the
/// world stopped before its second half; otherwise `1 − min(1, readiness / ready_target)`
/// with `readiness` its second-half mean ([`body_readiness`]).
pub fn breeding_shortfall(
    founded: &[usize],
    late_births: &[u64],
    lost: &[bool],
    readiness: &[f64],
    spec: &BandSpec,
    reached_second_half: bool,
) -> f64 {
    (0..Founder::COUNT)
        .filter(|&i| founded[i] > 0 && !spec.persist_only[i] && !spec.breed_exempt[i])
        .map(|i| {
            if !reached_second_half || lost[i] {
                1.0
            } else if late_births[i] > 0 {
                0.0
            } else {
                1.0 - (readiness[i] / spec.ready_target).min(1.0)
            }
        })
        .sum()
}

/// **One candidate world in progress**: founded, then stepped one window at a time
/// ([`EvalRun::window`]), so a unified generation can hold all its worlds and take a brain
/// step between windows; [`EvalRun::finish`] scores it. [`evaluate`] is the three in a row.
pub struct EvalRun {
    spec: EvalSpec,
    base: u64,
    started: Instant,
    sim: cubarium_voxel_sim::Sim,
    info: world::FoundInfo,
    founded: Vec<usize>,
    band: Band,
    species_start: Vec<usize>,
    trace: Trace,
    moving: [u64; 4],
    /// Second-half readiness: its sum and samples per lineage.
    readiness: Vec<(f64, u64)>,
    readouts: Vec<Vec<(String, super::readout::Pool, f64)>>,
    body_scores: Vec<BodyScore>,
    stopped: Option<String>,
    elapsed: u64,
    /// The centre key ([`centre_key`]) each window ran, in order.
    pub keys: Vec<u64>,
    /// Its bodies' familiarity, carried from window to window.
    memory: Memory,
    /// What the last window counted per lineage.
    last: [LineageStats; Founder::COUNT],
    /// Targets v3's bookkeeping: the ledger at founding and one day-cycle later, the
    /// cycle's length, exact recruitment event baselines, births per
    /// lineage over the run and in its second half (by tick), the founders' ids, and
    /// body-minutes per lineage.
    snap_start: Snap,
    snap_cycle: Option<Snap>,
    cycle_ticks: u64,
    required_roster: RequiredRoster,
    event_half: Option<Recruitment>,
    first_id: u64,
    births_all: [u64; Founder::COUNT],
    births_late: [u64; Founder::COUNT],
    body_minutes: [u64; Founder::COUNT],
    /// Adult body-ticks per lineage over the run (the balance measures' adult-days).
    adult_ticks: [u64; Founder::COUNT],
    /// The shared thread budget this world re-reads every simulated minute, if any.
    budget: Option<std::sync::Arc<super::threads::ThreadBudget>>,
    /// How it started, and its baselines ([`super::start`]).
    record: super::start::StartRecord,
    baselines: Vec<super::start::Baseline>,
}

impl EvalRun {
    /// Found a world under `params` from seed base `base`, the lineages with a centre
    /// driven by `drivers` (the rest run their heuristic).
    pub fn begin(
        params: &EcologyParams,
        base: u64,
        drivers: &[(Founder, EpisodeDriver)],
        spec: &EvalSpec,
    ) -> EvalRun {
        use super::start::{self, Residuals, StartRecord, StartSet};
        let started = Instant::now();
        let mut spec = spec.clone();
        let declared_flora = params.flora(spec.world.voxel_m);
        let mut required_roster = spec.required_roster.clone().unwrap_or_else(|| RequiredRoster::declared(&spec.founders, &declared_flora));
        let (mut sim, info, mut record) = match &spec.start {
            StartSet::E0 => {
                let seeds = if spec.asked_seed {
                    cubarium_voxel_sim::found::Seeds::Asked(base)
                } else {
                    cubarium_voxel_sim::found::Seeds::from(base)
                };
                let (mut sim, info) = world::found_world_from(&spec.world, seeds, spec.founders, params);
                super::install_centres(sim.fauna_mut(), drivers);
                let (w, fl, fa) = sim.layers();
                let hash = start::params_hash(&start::layers_of(fl, fa));
                let record = StartRecord {
                    parameter_changes: start::parameter_changes(&EcologyParams::defaults().layers(w.config().voxel_m),
                        &params.layers(w.config().voxel_m), &start::layers_of(fl, fa)),
                    set: spec.start.label(),
                    seed: info.seed,
                    start_tick: w.tick(),
                    start_params: hash.clone(),
                    candidate_params: hash.clone(),
                    installed_params: hash,
                    residuals: vec![("found".into(), Residuals::of(w, fl, fa))],
                    ..StartRecord::default()
                };
                (sim, info, record)
            }
            StartSet::Library(lib) => {
                // **The initializer**: load and check, the candidate's rates over the
                // loaded state, the result installed in every layer, fresh controllers.
                let s = lib
                    .load(base)
                    .unwrap_or_else(|e| panic!("start {} seed {base}: {e}", lib.dir.display()));
                let mut residuals = vec![("load".to_string(), Residuals::of(&s.world, &s.flora, &s.fauna))];
                let start_layers = start::layers_of(&s.flora, &s.fauna);
                let candidate = params.layers(s.world.config().voxel_m);
                let o = start::rate_override(&start_layers, &candidate)
                    .unwrap_or_else(|e| panic!("start {} seed {base}: the override: {e}", lib.dir.display()));
                let (world, mut flora, mut fauna, manifest) = (s.world, s.flora, s.fauna, s.manifest);
                start::install(&mut flora, &mut fauna, &o.installed)
                    .unwrap_or_else(|e| panic!("start {} seed {base}: installing: {e}", lib.dir.display()));
                residuals.push(("override".into(), Residuals::of(&world, &flora, &fauna)));
                // Controller memory is not saved: every body gets a fresh controller.
                cubarium_voxel_sim::habitat::reinstall_heuristics(&mut fauna);
                let start_tick = world.tick();
                let mut sim = cubarium_voxel_sim::Sim::ambient(
                    world,
                    flora,
                    fauna,
                    cubarium_voxel_sim::SimConfig { threads: 1, overlap: false, water_every: 1 },
                );
                super::install_centres(sim.fauna_mut(), drivers);
                let pop = window::population(&sim);
                let info = world::FoundInfo {
                    seed: manifest.seed,
                    accepted: manifest.accepted,
                    lake_rejected: 0,
                    habitat_rejected: 0,
                    founders: pop,
                    stands: sim.flora().view().stands.len(),
                };
                // The established setting scores every lineage (R1).
                if let Some(stage) = &mut spec.band.stage {
                    stage.readout_only = [false; Founder::COUNT];
                }
                let record = StartRecord {
                    parameter_changes: start::parameter_changes(&start_layers, &candidate, &o.installed),
                    start_identity: Some(manifest.identity.clone()),
                    set: spec.start.label(),
                    seed: manifest.seed,
                    start_tick,
                    start_params: manifest.params.clone(),
                    candidate_params: start::params_hash(&candidate),
                    installed_params: start::params_hash(&o.installed),
                    changed: o.changed.len(),
                    frozen: o.frozen,
                    residuals,
                    ..StartRecord::default()
                };
                (sim, info, record)
            }
        };
        if spec.start.established() { required_roster.include_present(&Presence::of(&sim)); }
        let mut lost_required_names = required_roster.losses(&Presence::of(&sim));
        let founding_losses = lost_required_names.clone();
        record.drivers = Founder::ALL.iter().map(|f| {
            let what = drivers.iter().find(|(lineage, _)| lineage == f).map_or_else(
                || "heuristic".to_string(),
                |(_, d)| format!("{} weights={:016x}", d.name(), d.digest()));
            let source = spec.driver_sources.iter().find(|(lineage, _)| lineage == f)
                .map_or("in-memory", |(_, label)| label.as_str());
            format!("{}={what} source={}", f.name(), if drivers.iter().any(|(lineage, _)| lineage == f) { source } else { "heuristic" })
        }).collect();
        eprintln!("arm_provenance {}", serde_json::to_string(&record).expect("provenance"));
        sim.world_mut().set_water_every(spec.water_every);
        // **The burn-in**: fixed per start, the same for every candidate, ending on the
        // day clock's window grid. A failure in it is recorded, never excluded.
        let present: Vec<bool> = {
            let pop = window::population(&sim);
            let e = eggs(&sim);
            (0..Founder::COUNT).map(|i| pop[i] + e[i] > 0).collect()
        };
        let burn = start::burn_in_ticks(
            record.start_tick,
            spec.burn_in.min_ticks,
            spec.burn_in.align_ticks,
            &sim.world().config().day,
        );
        if let Some(stage) = &spec.staging {
            if let Err(why) = stage.run.check().and_then(|()| stage.check_world(&sim, info.seed, None).map(|_| ())) {
                record.burn_in_stopped = Some(why);
            }
        }
        let mut burn_check = spec.staging.as_ref().filter(|s| s.full).map(|_| cubarium_voxel_sim::staged::Window::open(&sim));
        let mut burnt = 0u64;
        while burnt < burn && record.burn_in_stopped.is_none() {
            if let Some(stage) = &spec.staging {
                if let Err(why) = stage.run.check() { record.burn_in_stopped = Some(why); break; }
            }
            sim.step();
            retain_losses(&mut lost_required_names, required_roster.losses(&Presence::of(&sim)));
            burnt += 1;
            if let Some(check) = &mut burn_check { check.watch(&sim); }
            if burnt.is_multiple_of(TICKS_PER_MIN) {
                if let Some(stage) = &spec.staging {
                    if let Err(why) = stage.check_world(&sim, info.seed, burn_check.as_ref()).map(|_| ()) {
                        record.burn_in_stopped = Some(why); break;
                    }
                    burn_check = None;
                }
                let total = sim.fauna().view().animals.len();
                if total > spec.abort_animals {
                    record.burn_in_stopped = Some(format!(
                        "burn-in: {total} animals at minute {}",
                        burnt / TICKS_PER_MIN
                    ));
                    break;
                }
            }
        }
        record.burn_in_ticks = burnt;
        {
            let pop = window::population(&sim);
            let e = eggs(&sim);
            record.burn_in_lost = Founder::ALL
                .iter()
                .filter(|f| present[f.index()] && pop[f.index()] + e[f.index()] == 0)
                .map(|f| f.name().to_string())
                .collect();
        }
        retain_losses(&mut record.burn_in_lost, lost_required_names.iter().filter(|name| !founding_losses.contains(name)).cloned().collect());
        let (w, fl, fa) = sim.layers();
        record.residuals.push(("burn-in".into(), Residuals::of(w, fl, fa)));
        record.scoring_tick = w.tick();
        record.scoring_phase = w.config().day.cycle_phase(w.tick());
        let baselines = vec![start::Baseline::of(w, fl, fa)];
        let stopped = record.burn_in_stopped.clone();
        let founders = if spec.start.established() { window::population(&sim) } else { info.founders };
        let founded = founded_lineages(&founders, &eggs(&sim));
        let lj = Founder::Lanternjaw.index();
        let readout = spec.band.stage.map_or([false; Founder::COUNT], |s| s.readout_only);
        let animals0: usize =
            (0..Founder::COUNT).filter(|&i| i != lj && !readout[i]).map(|i| founded[i]).sum();
        let band = Band::for_founding(animals0, founded[lj] > 0, &spec.band);
        let species_start = Scan::of(&sim).stands.to_vec();
        let max_id = sim.fauna().view().animals.iter().map(|a| a.id).max().unwrap_or(0);
        let day = &spec.world.day;
        let cycle_ticks = match day.mode {
            cubarium_voxel::weather::DayMode::Cycle => {
                (day.length_min * 60.0 * f64::from(cubarium_voxel_fauna::TICK_HZ)).round() as u64
            }
            _ => spec.horizon_ticks,
        };
        let snap_start = Snap::of(&sim);
        let trace = Trace {
            lost_required: lost_required_names.len(),
            lost_required_names,
            required_loss_weight: if spec.band.absolute_persistence { spec.band.required_loss_weight } else { 0.0 },
            horizon_min: spec.horizon_ticks as f64 / TICKS_PER_MIN as f64,
            ..Trace::default()
        };
        EvalRun {
            spec,
            base,
            started,
            sim,
            info,
            founded,
            band,
            species_start,
            trace,
            moving: [0; 4],
            readiness: vec![(0.0, 0); Founder::COUNT],
            readouts: Vec::new(),
            body_scores: Vec::new(),
            stopped,
            elapsed: 0,
            keys: Vec::new(),
            memory: Memory::default(),
            last: Default::default(),
            snap_start,
            snap_cycle: None,
            cycle_ticks,
            required_roster,
            event_half: None,
            first_id: max_id,
            births_all: [0; Founder::COUNT],
            births_late: [0; Founder::COUNT],
            body_minutes: [0; Founder::COUNT],
            adult_ticks: [0; Founder::COUNT],
            budget: None,
            record,
            baselines,
        }
    }

    /// How this run started ([`super::start::StartRecord`]).
    pub fn start_record(&self) -> &super::start::StartRecord {
        &self.record
    }

    /// The ledger baselines: the burn-in's end and every window boundary since.
    pub fn baselines(&self) -> &[super::start::Baseline] {
        &self.baselines
    }

    /// What the last window counted per lineage (all zero before the first).
    pub fn last_lineages(&self) -> &[LineageStats; Founder::COUNT] {
        &self.last
    }

    /// The world as it stands.
    pub fn sim(&self) -> &cubarium_voxel_sim::Sim {
        &self.sim
    }

    /// Step this world on `threads` workers from the next tick on, and keep it there
    /// (no shared budget). Execution only.
    pub fn set_threads(&mut self, threads: usize) {
        self.budget = None;
        self.sim.set_water_pool(None);
        self.sim.set_threads(threads);
    }

    /// Take this world's thread count from `budget` now and again every simulated minute
    /// ([`super::threads`]): one thread while jobs are queued, the machine shared among
    /// the worlds still running once the queue is empty. Its water, once it is parallel,
    /// runs on the one pool of all the budget's cores the other worlds share, so worlds at
    /// the same count do not crowd one small pool. Execution only.
    pub fn share_threads(&mut self, budget: std::sync::Arc<super::threads::ThreadBudget>) {
        self.sim.set_water_pool(Some(budget.cores()));
        self.sim.set_threads(budget.current());
        self.budget = Some(budget);
    }

    /// Ticks run so far.
    pub fn elapsed(&self) -> u64 {
        self.elapsed
    }

    /// What the founding loop kept.
    pub fn info(&self) -> &world::FoundInfo {
        &self.info
    }

    /// Whether the horizon is run or the world stopped early.
    pub fn done(&self) -> bool {
        self.stopped.is_some() || self.elapsed >= self.spec.horizon_ticks
    }

    /// Bodies per lineage now.
    pub fn population(&self) -> [usize; Founder::COUNT] {
        window::population(&self.sim)
    }

    /// Run the next window under `tables` (`None` per untrained lineage), the twins'
    /// placement starting at `first_pair`; `key` records which centres they came from.
    /// Returns the window's body scores (none once done).
    pub fn window(
        &mut self,
        tables: &[Option<VariantTable>],
        first_pair: &[usize],
        key: u64,
    ) -> Vec<BodyScore> {
        if self.done() {
            return Vec::new();
        }
        let spec = &self.spec;
        let lj = Founder::Lanternjaw.index();
        let half_ticks = spec.horizon_ticks / 2;
        let ticks = spec.window_ticks.min(spec.horizon_ticks - self.elapsed);
        let window_start = self.elapsed;
        let memory = std::mem::take(&mut self.memory);
        let mut w =
            Window::begin_with(&mut self.sim, tables, first_pair, ticks, &spec.curiosity, memory);
        w.stomach_aware_reward = spec.band.stomach_aware_reward;
        let mut halt = None;
        let mut check = spec.staging.as_ref().filter(|s| s.full && self.elapsed == 0)
            .map(|_| cubarium_voxel_sim::staged::Window::open(&self.sim));
        while !w.done() {
            if let Some(stage) = &spec.staging {
                if let Err(why) = stage.run.check() { halt = Some(why); break; }
            }
            w.step(&mut self.sim);
            if let Some(check) = &mut check { check.watch(&self.sim); }
            self.elapsed += 1;
            retain_losses(&mut self.trace.lost_required_names, self.required_roster.losses(&Presence::of(&self.sim)));
            self.trace.lost_required = self.trace.lost_required_names.len();
            let events = Recruitment::of(self.sim.fauna());
            self.births_all = events.since(&self.snap_start.recruitment).births;
            if self.elapsed == half_ticks { self.event_half = Some(events); }
            if self.elapsed > half_ticks {
                self.births_late = events.since(self.event_half.as_ref().unwrap_or(&self.snap_start.recruitment)).births;
            }
            if self.elapsed.is_multiple_of(TICKS_PER_MIN) {
                if let Some(b) = &self.budget {
                    self.sim.set_threads(b.current());
                }
                let pop = window::population(&self.sim);
                let total = self.sim.fauna().view().animals.len();
                let readout = spec.band.stage.map_or([false; Founder::COUNT], |s| s.readout_only);
                self.trace.minutes.push((self.elapsed / TICKS_PER_MIN) as f64);
                // The band's count: readout-only lineages out (all of them without v3).
                self.trace.animals.push(total - (pop.iter().sum::<usize>() - scored_animals(&pop, &readout)));
                self.trace.lanternjaws.push(pop[lj]);
                if self.elapsed > half_ticks {
                    let fauna = self.sim.fauna();
                    for a in fauna.view().animals.iter() {
                        let Some(f) = a.founder else { continue };
                        let r = &mut self.readiness[f.index()];
                        r.0 += body_readiness(a, &fauna.config().founder(f).core);
                        r.1 += 1;
                    }
                }
                if self.snap_cycle.is_none() && self.elapsed >= self.cycle_ticks {
                    self.snap_cycle = Some(Snap::of(&self.sim));
                }
                if total > spec.abort_animals {
                    halt = Some(format!(
                        "{total} animals at minute {}",
                        self.elapsed / TICKS_PER_MIN
                    ));
                } else if spec.stop_on_loss
                    && (scored_lost(&self.founded, &pop, &eggs(&self.sim), &readout) > 0
                        || (spec.band.absolute_persistence && self.trace.lost_required > 0))
                {
                    halt = Some("a required member or scored lineage was lost".to_string());
                }
                if halt.is_some() {
                    break;
                }
            }
        }
        let mut r = w.finish(&self.sim, self.info.seed as usize);
        self.memory = std::mem::take(&mut r.memory);
        self.last = r.lineages.clone();
        let b = &r.lineages[Founder::Browser.index()];
        self.moving[2] += b.moving_minutes;
        self.moving[3] += b.body_minutes;
        if window_start >= half_ticks {
            self.moving[0] += b.moving_minutes;
            self.moving[1] += b.body_minutes;
        }
        for f in Founder::ALL {
            self.body_minutes[f.index()] += r.lineages[f.index()].body_minutes;
            self.adult_ticks[f.index()] += r.lineages[f.index()].adult_ticks;
        }
        self.body_scores.extend(r.scores.iter().copied());
        self.readouts.push(r.readout);
        self.keys.push(key);
        if let Some(stage) = &spec.staging {
            let checked = if halt.as_ref().is_some_and(|why| why.starts_with("wall budget:")) {
                Ok("deadline; partial window saved, boundary will be replayed".into())
            } else { stage.check_world(&self.sim, self.info.seed, check.as_ref()) };
            match checked {
                Ok(note) => {
                    let dir = stage.world_dir(self.info.seed);
                    if let Err(why) = super::staging::save_world(&dir, &self.sim)
                        .and_then(|()| stage.run.status(stage.generation, self.keys.len() as u64,
                            self.elapsed as f64 / self.started.elapsed().as_secs_f64().max(1e-9), &note, "RUNNING")) {
                        stage.run.fail(format!("world {} seed {}: checkpoint/status failed: {why}", dir.display(), self.info.seed));
                        halt = Some(why);
                    }
                }
                Err(why) => halt = Some(why),
            }
        }
        self.stopped = halt;
        let (w, fl, fa) = self.sim.layers();
        self.baselines.push(super::start::Baseline::of(w, fl, fa));
        r.scores
    }

    /// Score the world as it stands: the trace to the horizon, persistence, the band.
    pub fn finish(self) -> Evaluation {
        let EvalRun {
            spec,
            base,
            started,
            sim,
            info,
            founded,
            band,
            species_start,
            mut trace,
            moving,
            readiness,
            readouts,
            body_scores,
            stopped,
            elapsed,
            snap_start,
            snap_cycle,
            first_id,
            births_all,
            births_late,
            body_minutes,
            adult_ticks,
            record,
            baselines,
            required_roster,
            ..
        } = self;
        let late_births = births_late.to_vec();
        let presence_end = Presence::of(&sim);
        retain_losses(&mut trace.lost_required_names, required_roster.losses(&presence_end));
        trace.lost_required = trace.lost_required_names.len();
        let recruitment = Recruitment::of(sim.fauna()).since(&snap_start.recruitment);
        let readiness: Vec<f64> = readiness
            .iter()
            .map(|(sum, n)| if *n > 0 { sum / *n as f64 } else { 0.0 })
            .collect();
        let [late_moving, late_minutes, all_moving, all_minutes] = moving;
        trace.browser_moving = if late_minutes > 0 {
            late_moving as f64 / late_minutes as f64
        } else if all_minutes > 0 {
            all_moving as f64 / all_minutes as f64
        } else {
            0.0
        };
        let pop = window::population(&sim);
        let eggs_end = eggs(&sim);
        let species_end = Scan::of(&sim).stands.to_vec();
        let readout = spec.band.stage.map_or([false; Founder::COUNT], |s| s.readout_only);
        trace.missing = scored_lost(&founded, &pop, &eggs_end, &readout)
            + species_start
                .iter()
                .zip(&species_end)
                .filter(|(s, e)| **s > 0 && **e == 0)
                .count();
        let score_min =
            spec.score_horizon_ticks.max(spec.horizon_ticks) as f64 / TICKS_PER_MIN as f64;
        let projected_from_min = (trace.minutes.last().copied().unwrap_or(0.0) < score_min)
            .then(|| project(&mut trace, score_min));
        let lost: Vec<bool> = (0..Founder::COUNT)
            .map(|i| founded[i] > 0 && pop[i] + eggs_end[i] == 0)
            .collect();
        let reached_half = elapsed > spec.horizon_ticks / 2;
        let stage = spec.band.stage.unwrap_or_else(StageSpec::readout_default);
        trace.silent_lineages = breeding_shortfall(
            &founded,
            &late_births,
            &lost,
            &readiness,
            &spec.band,
            reached_half,
        );
        let lineages = outcomes(
            &sim,
            &founded,
            &eggs_end,
            &readiness,
            &snap_start,
            snap_cycle.as_ref(),
            first_id,
            &births_all,
            &births_late,
            &stage,
            reached_half,
            &adult_ticks,
            elapsed,
        );
        if let Some(stage) = &spec.band.stage {
            trace.silent_lineages = 0.0;
            trace.stage = lineages.iter().map(|o| o.shortfall).sum();
            trace.replenish =
                stage.replenish_weight * lineages.iter().map(|o| o.replenish_shortfall).sum::<f64>();
            // Established means a free adult or a hunter, not a free juvenile (Astra F2).
            let lj_free = lineages.iter().any(|o| {
                o.founder == Founder::Lanternjaw && (o.free_adults > 0 || (o.free_living > 0 && o.hits > 0))
            });
            trace.share_discount =
                if lj_free { 0.0 } else { 1.0 - stage.share_weight_unestablished.clamp(0.0, 1.0) };
            trace.balance =
                stage.balance_weight * lineages.iter().map(|o| o.balance_shortfall).sum::<f64>();
            trace.counts_readout = stage.counts_readout;
        }
        let balance = balance_readout(
            &sim,
            &founded,
            &births_all,
            &snap_start,
            &body_minutes,
            elapsed,
            stage.balance_share,
        );
        let penalty = penalty(&trace, &band);
        println!("world base_seed={base} accepted_seed={} lost_required={} [{}] absolute_persist={}",
            info.seed, trace.lost_required, trace.lost_required_names.join(";"), penalty.absolute_persist);
        let readout_mean = super::readout::pool(&readouts)
            .into_iter()
            .map(|(name, v)| {
                // `pool` sums `Sum` factors over the list; here the list is one world's
                // windows, so every factor is averaged instead.
                let vals: Vec<f64> = readouts
                    .iter()
                    .filter_map(|r| r.iter().find(|x| x.0 == name).map(|x| x.2))
                    .filter(|x| x.is_finite())
                    .collect();
                let m = if vals.is_empty() {
                    v
                } else {
                    vals.iter().sum::<f64>() / vals.len() as f64
                };
                (name, m)
            })
            .collect();
        let readout_end = readouts
            .last()
            .map(|r| r.iter().map(|(n, _, v)| (n.clone(), *v)).collect())
            .unwrap_or_default();
        Evaluation {
            lost_required: trace.lost_required,
            lost_required_names: trace.lost_required_names.clone(),
            required_roster,
            presence_end,
            recruitment,
            base_seed: base,
            world_seed: info.seed,
            trace,
            penalty,
            band,
            stopped,
            projected_from_min,
            readout_mean,
            readout_end,
            species_start,
            species_end,
            lineages_end: pop.to_vec(),
            eggs_end,
            founded,
            late_births,
            readiness,
            lineages,
            balance,
            readout_only: readout.to_vec(),
            ticks: elapsed,
            wall_s: started.elapsed().as_secs_f64(),
            body_scores,
            start: record,
            baselines,
        }
    }
}

/// Every founded lineage's [`LineageOutcome`], judged under `stage`.
#[allow(clippy::too_many_arguments)]
fn outcomes(
    sim: &cubarium_voxel_sim::Sim,
    founded: &[usize],
    eggs_end: &[usize],
    readiness: &[f64],
    start: &Snap,
    cycle: Option<&Snap>,
    first_id: u64,
    births_all: &[u64; Founder::COUNT],
    births_late: &[u64; Founder::COUNT],
    stage: &StageSpec,
    reached_half: bool,
    adult_ticks: &[u64; Founder::COUNT],
    elapsed: u64,
) -> Vec<LineageOutcome> {
    let now = Snap::of(sim);
    let run_days = super::balance::days(
        elapsed as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ),
        sim.fauna().config(),
    );
    let fauna = sim.fauna();
    let animals = &fauna.view().animals;
    let mut out = Vec::new();
    for f in Founder::ALL {
        let i = f.index();
        if founded[i] == 0 {
            continue;
        }
        let mut o = LineageOutcome::new(f, founded[i]);
        let cfg = fauna.config();
        let core = &cfg.founder(f).core;
        let detach = cfg.parasite_detach_body_fraction * core.body_max;
        let mut growth = 0.0;
        let mut progress = 0.0;
        let mine = animals.iter().filter(|a| a.founder == Some(f));
        for a in mine {
            o.alive_end += 1;
            if a.attached.is_some() {
                o.riders_now += 1;
                let hours = a.age_ticks as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ) / 3600.0;
                let grown = (a.body - core.body_min).max(0.0);
                if hours > 0.0 {
                    growth += grown / hours;
                }
                if detach > core.body_min {
                    progress += (grown / (detach - core.body_min)).min(1.0);
                }
            } else {
                o.free_living += 1;
                if a.body >= core.birth_body {
                    o.free_adults += 1;
                } else if core.birth_body > core.body_min {
                    let p = (a.body - core.body_min).max(0.0) / (core.birth_body - core.body_min);
                    o.free_progress = o.free_progress.max(p.min(1.0));
                }
            }
            if let Some(m) = a.pack {
                if m.pup {
                    o.pups_alive += 1;
                } else if a.id > first_id {
                    o.pups_independent += 1;
                }
            }
        }
        if o.riders_now > 0 {
            o.rider_growth_per_h = growth / o.riders_now as f64;
            o.rider_progress = progress / o.riders_now as f64;
        }
        o.eggs_end = eggs_end[i];
        o.births = births_all[i];
        o.late_births = births_late[i];
        o.readiness = readiness[i];
        let bank = fauna.config().founder(f).founding == cubarium_voxel_fauna::Founding::Clutches;
        if bank {
            o.bank_hatches = now.hatched[i] - start.hatched[i];
        }
        let recruitment = now.recruitment.since(&start.recruitment);
        o.live_births = recruitment.live_births[i];
        o.hatchings = recruitment.hatchings[i];
        o.maturations = recruitment.maturations[i];
        o.eggs_laid = now.laid[i] - start.laid[i];
        o.hits = now.hits[i] - start.hits[i];
        o.intake_run = now.assimilated[i] - start.assimilated[i];
        o.carrion_eaten = now.carrion[i] - start.carrion[i];
        if f == Founder::Lanternjaw {
            o.riders_attached = now.riders_attached - start.riders_attached;
            o.rider_dropoffs = now.rider_dropoffs - start.rider_dropoffs;
            o.riders_freed = now.riders_freed - start.riders_freed;
        }
        if fauna.config().founder(f).pack.is_some() {
            o.litters = now.litters - start.litters;
            o.pups_born = now.pups_born - start.pups_born;
            o.provision_received = now.provision_received - start.provision_received;
            o.pack_kills = now.pack_kills - start.pack_kills;
        }
        o.biomass_start = start.biomass[i] + start.stomach[i];
        if let Some(c) = cycle {
            o.cycle_complete = true;
            (o.intake_cycle, o.biomass_start, o.biomass_cycle) = super::targets::cycle_books(
                c.assimilated[i] - start.assimilated[i],
                start.biomass[i],
                c.biomass[i],
                start.stomach[i],
                c.stomach[i],
            );
        }
        // The balance measures over the run (the search rebase's addendum).
        let adult_days = super::balance::days(
            adult_ticks[i] as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ),
            cfg,
        );
        o.books = super::balance::Books::of(&now.ledger, f).since(&super::balance::Books::of(&start.ledger, f));
        let ctx = super::balance::context(animals, cfg, f, adult_days, run_days);
        o.balance = super::balance::measures(&o.books, &ctx);
        if let Some(h) = cfg.founder(f).hunter {
            let k = super::balance::kills(&start.ledger, &now.ledger, f) as f64;
            let prey: u64 = Founder::ALL.iter().filter(|q| h.prey[q.index()]).map(|q| births_all[q.index()]).sum();
            o.kills_per_adult_day = (adult_days > 0.0).then(|| k / adult_days);
            o.prey_births_per_day = (run_days > 0.0).then(|| prey as f64 / run_days);
        }
        judge(&mut o, stage, reached_half);
        out.push(o);
    }
    out
}

/// The measured predator balance of every founded hunter ([`BalanceReadout`]).
fn balance_readout(
    sim: &cubarium_voxel_sim::Sim,
    founded: &[usize],
    births_all: &[u64; Founder::COUNT],
    start: &Snap,
    body_minutes: &[u64; Founder::COUNT],
    elapsed: u64,
    share: f64,
) -> Vec<BalanceReadout> {
    let cfg = sim.fauna().config();
    let now = Snap::of(sim);
    let day_s = sim.world().config().day.length_min * 60.0;
    let run_s = elapsed as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ);
    if run_s <= 0.0 {
        return Vec::new();
    }
    let life = cfg.life;
    let mut out = Vec::new();
    for f in Founder::ALL {
        let p = cfg.founder(f);
        let Some(h) = p.hunter else { continue };
        if founded[f.index()] == 0 {
            continue;
        }
        let y = p.yield_by_food[cubarium_voxel_fauna::Food::Carrion.index()];
        let mut births = 0.0;
        let mut valued = 0.0;
        for q in Founder::ALL.into_iter().filter(|q| h.prey[q.index()]) {
            let qc = &cfg.founder(q).core;
            let b = births_all[q.index()] as f64 * day_s / run_s;
            births += b;
            valued += b * qc.body_max * (1.0 + qc.reserve_cap) * y;
        }
        let asleep = super::screen::founder_sleep(&sim.world().config().day, p, &life).asleep_s;
        let one = p.core.maintenance_per_s
            * p.core.body_max
            * ((day_s - asleep) + p.sleep_upkeep * asleep);
        let pred_days = body_minutes[f.index()] as f64 * 60.0 / day_s;
        let carrion = now.carrion[f.index()] - start.carrion[f.index()];
        out.push(BalanceReadout {
            predator: f.name(),
            prey_births_per_day: births,
            supported: if one > 0.0 { share * valued / one } else { f64::INFINITY },
            carrion_per_predator_day: if pred_days > 0.0 { carrion / pred_days } else { 0.0 },
        });
    }
    out
}

/// Found a world under `params` from seed base `base`, drive the lineages with a centre
/// by `drivers` (the rest run their heuristic), and run it for the horizon under one set
/// of `tables` ([`EvalRun`] begin, every window, finish).
pub fn evaluate(
    params: &EcologyParams,
    base: u64,
    drivers: &[(Founder, EpisodeDriver)],
    tables: &[Option<VariantTable>],
    spec: &EvalSpec,
) -> Evaluation {
    evaluate_shared(params, base, drivers, tables, spec, None)
}

/// [`evaluate`] with the world's thread count taken from a runner's shared `budget`
/// ([`super::threads`]) every simulated minute; one thread throughout without.
pub fn evaluate_shared(
    params: &EcologyParams,
    base: u64,
    drivers: &[(Founder, EpisodeDriver)],
    tables: &[Option<VariantTable>],
    spec: &EvalSpec,
    budget: Option<&std::sync::Arc<ThreadBudget>>,
) -> Evaluation {
    let mut run = EvalRun::begin(params, base, drivers, spec);
    if let Some(b) = budget {
        run.share_threads(b.clone());
    }
    let zeros = vec![0usize; Founder::COUNT];
    while !run.done() {
        run.window(tables, &zeros, 0);
    }
    run.finish()
}

/// One search run.
#[derive(Clone, Debug)]
pub struct SearchSpec {
    /// Lineages with a trained centre; the rest run their heuristic and take no step.
    pub centres: Vec<(Founder, PathBuf)>,
    /// The search: its genes at their reference values (the default candidate every
    /// generation runs beside the draws, the persistence reference).
    pub genome: Genome,
    /// Where the CMA mean starts: the search genome with these genes' values ([`start_from`]);
    /// the reference itself when `None`.
    pub start: Option<Genome>,
    pub screen: ScreenSpec,
    pub lambda: usize,
    pub seeds_per_generation: usize,
    pub generations: u32,
    pub wall_seconds: f64,
    /// Calibration deadline for the first boundary, wall seconds including setup.
    pub first_generation_seconds: f64,
    pub sigma0: f64,
    pub threads: usize,
    /// Cores a generation's worlds share once none is left queued ([`super::threads`]): a
    /// generation waits for its slowest world, so its last worlds spread onto the cores the
    /// finished ones freed. `1` keeps every world on one thread throughout.
    pub tail_cores: usize,
    /// Generation `g`'s seeds are `seed_base + (g·seeds_per_generation + s)·1000`.
    pub seed_base: u64,
    pub cma_seed: u64,
    pub eval: EvalSpec,
    /// Unified mode: brain antithetic pairs per lineage (0 = brains held, the default).
    pub brain_pairs: usize,
    pub brain_sigma: f64,
    pub brain_learning_rate: f64,
    pub out: PathBuf,
    /// Continue from the checkpoint ([`Checkpoint`]) in this directory: the CMA state, the
    /// centres and their Adam state, the brain step, the best so far.
    pub resume: Option<PathBuf>,
}

/// One candidate's record in a generation.
#[derive(Clone, Debug, Serialize)]
pub struct CandidateRecord {
    /// `c0..` a CMA draw, `default` the search file at its starting values.
    pub label: String,
    pub multipliers: Vec<(String, f64)>,
    /// Mean penalty over the seeds plus the box penalty (what CMA was told); the screen's
    /// fixed penalty for a candidate it refused.
    pub fitness: f64,
    pub box_penalty: f64,
    /// Its stay-or-go numbers, and what the screen did: ran inside the bands, ran in the
    /// soft margin (paying `soft` on top of its runs), or refused (unrun; `screened_out`).
    pub rsf: [f64; 3],
    /// The roster screens' named refusals (night endurance, food access), and their rows
    /// with the predator balance readout.
    #[serde(default)]
    pub refusals: Vec<String>,
    #[serde(default)]
    pub roster: RosterScreen,
    pub screen: Verdict,
    pub soft: f64,
    pub screened_out: bool,
    pub evaluations: Vec<Evaluation>,
    pub lost_required: usize,
    pub lost_required_names: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchRecord {
    pub generation: u32,
    pub seeds: Vec<u64>,
    /// The CMA mean and σ the generation drew from, and both after its step.
    pub mean_start: Vec<f64>,
    pub sigma_start: f64,
    pub mean: Vec<f64>,
    pub sigma: f64,
    /// Unified mode: one entry per window, each a brain step.
    pub windows: Vec<WindowLog>,
    pub wall_s: f64,
    pub elapsed_s: f64,
    pub candidates: Vec<CandidateRecord>,
}

/// One window of a unified generation: every running world stepped, one ES step per
/// trained lineage.
#[derive(Clone, Debug, Serialize)]
pub struct WindowLog {
    pub generation: u32,
    pub window: u32,
    /// The ES step index (perturbation stream and Adam step) this window used.
    pub brain_step: u32,
    pub wall_s: f64,
    /// The centre key ([`centre_key`], all lineages XORed) each running world was handed.
    pub world_keys: Vec<u64>,
    pub body_scores: Vec<BodyScore>,
    pub lineages: Vec<LineageWindow>,
}

/// One trained lineage in one window.
#[derive(Clone, Debug, Serialize)]
pub struct LineageWindow {
    pub lineage: String,
    /// Bodies scored, over every world.
    pub bodies: usize,
    /// **The signal check**, over the pooled (within-world z) candidate scores: the mean
    /// |plus − minus| of a pair (the perturbation's effect plus placement noise) against
    /// the mean |pair mean − overall mean| (placement noise alone: a pair's mean cancels
    /// its perturbation to first order). At brain σ = 0 both are noise.
    pub twin_diff: f64,
    pub pair_spread: f64,
    /// The step's update rms (0: no step).
    pub update_rms: f64,
    /// Over the scored bodies: mean novelty and mean curiosity term (`weight × novelty`),
    /// and the within-world spreads (sd about each world's mean, pooled over worlds) of
    /// `delta + given` and of novelty, the curiosity weight's calibration numbers.
    pub mean_novelty: f64,
    pub mean_curiosity: f64,
    pub sd_energy: f64,
    pub mean_delta_body: f64,
    pub mean_delta_stomach_aware: f64,
    pub sd_novelty: f64,
    /// Moving body-minutes over body-minutes, every body of the lineage in the window.
    pub moving_share: f64,
    /// The same spreads over the bodies that neither died nor gave in the window (the
    /// ones that simply lived through it), and the shares of scored bodies that died and
    /// that gave.
    pub sd_energy_lived: f64,
    pub sd_novelty_lived: f64,
    pub died_share: f64,
    pub gave_share: f64,
    /// The centre this window ran, before its step.
    pub centre_key: u64,
}

/// A centre's identity: FNV-1a 64 over its weights' bits.
pub fn centre_key(theta: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in theta {
        for b in x.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// The signal check of [`LineageWindow`]: `(mean |twin difference|, mean |pair mean −
/// overall mean|)` over antithetic candidate scores `[p0+, p0−, p1+, …]`.
pub fn signal(scores: &[f64]) -> (f64, f64) {
    let pairs = scores.len() / 2;
    if pairs == 0 {
        return (0.0, 0.0);
    }
    let m = scores.iter().sum::<f64>() / scores.len() as f64;
    let (mut twin, mut pair) = (0.0, 0.0);
    for p in 0..pairs {
        let (a, b) = (scores[2 * p], scores[2 * p + 1]);
        twin += (a - b).abs();
        pair += (0.5 * (a + b) - m).abs();
    }
    (twin / pairs as f64, pair / pairs as f64)
}

/// The within-world standard deviation: each value about its own world's mean, pooled
/// over the worlds (as [`pooled_scores`] normalises). 0 with no values.
pub fn within_world_sd(per_world: &[Vec<f64>]) -> f64 {
    let (mut ss, mut n) = (0.0, 0usize);
    for xs in per_world.iter().filter(|xs| !xs.is_empty()) {
        let m = xs.iter().sum::<f64>() / xs.len() as f64;
        ss += xs.iter().map(|x| (x - m).powi(2)).sum::<f64>();
        n += xs.len();
    }
    if n > 0 { (ss / n as f64).sqrt() } else { 0.0 }
}

/// One trained centre's state in a [`Checkpoint`].
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct CentreState {
    pub lineage: String,
    pub theta: Vec<f64>,
    pub adam: Adam,
}

/// What a search needs to carry on after a generation (`checkpoint.json`, rewritten every
/// generation; `--resume <dir>` reads it).
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct Checkpoint {
    pub next_generation: u32,
    pub brain_step: u32,
    pub cma: Cma,
    pub centres: Vec<CentreState>,
    /// The best so far: its in-box penalty, its genome file, its generation.
    pub best: Option<(f64, String, u32)>,
}

impl Checkpoint {
    pub const FILE: &'static str = "checkpoint.json";

    /// Write it beside the run's output, atomically (a temporary file renamed).
    pub fn save(&self, dir: &std::path::Path) -> Result<(), String> {
        let tmp = dir.join(format!("{}.tmp", Checkpoint::FILE));
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, json).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, dir.join(Checkpoint::FILE))
            .map_err(|e| format!("cannot replace the checkpoint: {e}"))
    }

    pub fn load(dir: &std::path::Path) -> Result<Checkpoint, String> {
        let path = dir.join(Checkpoint::FILE);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// A unified generation in lock step: found every running world and hold them; then,
/// window by window, step every world one window in parallel, pool the body scores
/// (normalised within each world), take one ES step per trained lineage and hand out fresh
/// variants for the next window. Returns each world's evaluation at the horizon.
#[allow(clippy::too_many_arguments)]
fn lockstep(
    spec: &SearchSpec,
    params: &[EcologyParams],
    jobs: &[(usize, u64)],
    drivers: &[(Founder, EpisodeDriver)],
    thetas: &mut [(Founder, Vec<f64>, Adam)],
    brain_step: &mut u32,
    generation: u32,
    logs: &mut Vec<WindowLog>,
) -> Result<Vec<Evaluation>, String> {
    let mut runs: Vec<EvalRun> = parallel(jobs.to_vec(), spec.threads, |(k, seed)| {
        let mut eval = spec.eval.clone();
        if let Some(s) = &mut eval.staging { s.arm = format!("candidate{k}"); }
        EvalRun::begin(&params[k], seed, drivers, &eval)
    });
    if let Some(s) = &spec.eval.staging { s.run.check()?; }
    let candidates = 2 * spec.brain_pairs;
    let budget = ThreadBudget::new(spec.tail_cores);
    let mut window = 0u32;
    while runs.iter().any(|r| !r.done()) {
        let started = Instant::now();
        let mut tables: Vec<Option<VariantTable>> = vec![None; Founder::COUNT];
        let mut keys = Vec::new();
        for (f, theta, _) in thetas.iter() {
            tables[f.index()] = Some(VariantTable::antithetic(
                *f,
                theta,
                spec.cma_seed,
                spec.brain_sigma,
                *brain_step,
                spec.brain_pairs,
            )?);
            keys.push(centre_key(theta));
        }
        let key = keys.iter().fold(0u64, |a, k| a ^ k);
        // Pair numbering runs on from one world to the next, as the live trainer's does.
        let mut first: Vec<Vec<usize>> = Vec::with_capacity(runs.len());
        let mut cursor = vec![0usize; Founder::COUNT];
        for r in &runs {
            first.push(cursor.clone());
            let pop = r.population();
            for f in Founder::ALL {
                cursor[f.index()] = (cursor[f.index()] + pop[f.index()].div_ceil(2)) % spec.brain_pairs;
            }
        }
        let ran: Vec<usize> = (0..runs.len()).filter(|&i| !runs[i].done()).collect();
        let live: Vec<(&mut EvalRun, Vec<usize>)> = runs
            .iter_mut()
            .zip(first)
            .filter(|(r, _)| !r.done())
            .collect();
        let tables_ref = &tables;
        let results: Vec<(Vec<BodyScore>, [LineageStats; Founder::COUNT])> =
            parallel_shared(live, spec.threads, &budget, |(r, first), b| {
                r.share_threads(b.clone());
                let scores = r.window(tables_ref, &first, key);
                (scores, r.last_lineages().clone())
            });
        if let Some(s) = &spec.eval.staging { s.run.check()?; }
        let mut lineages = Vec::new();
        for ((f, theta, adam), k) in thetas.iter_mut().zip(&keys) {
            let per_world: Vec<Vec<(usize, f64)>> = results
                .iter()
                .map(|(sc, _)| {
                    sc.iter()
                        .filter(|b| b.founder == *f)
                        .map(|b| (b.candidate, b.score.score))
                        .collect()
                })
                .collect();
            let part = |pick: fn(&BodyScore) -> f64| -> Vec<Vec<f64>> {
                results
                    .iter()
                    .map(|(sc, _)| sc.iter().filter(|b| b.founder == *f).map(pick).collect())
                    .collect()
            };
            let energy = part(|b| b.score.delta_normalized + b.score.given_normalized);
            let lived = |pick: fn(&BodyScore) -> f64| -> Vec<Vec<f64>> {
                results
                    .iter()
                    .map(|(sc, _)| {
                        sc.iter()
                            .filter(|b| {
                                b.founder == *f && !b.score.died && b.score.given_normalized == 0.0
                            })
                            .map(pick)
                            .collect()
                    })
                    .collect()
            };
            let mine = || results.iter().flat_map(|(sc, _)| sc.iter()).filter(|b| b.founder == *f);
            let n_mine = mine().count().max(1) as f64;
            let died_share = mine().filter(|b| b.score.died).count() as f64 / n_mine;
            let gave_share = mine().filter(|b| b.score.given_normalized > 0.0).count() as f64 / n_mine;
            let novelty = part(|b| b.score.novelty);
            let curiosity = part(|b| b.score.curiosity);
            let mean = |xs: &[Vec<f64>]| {
                let n: usize = xs.iter().map(Vec::len).sum();
                xs.iter().flatten().sum::<f64>() / n.max(1) as f64
            };
            let (moving, minutes) = results.iter().fold((0u64, 0u64), |a, (_, l)| {
                let l = &l[f.index()];
                (a.0 + l.moving_minutes, a.1 + l.body_minutes)
            });
            let bodies: usize = per_world.iter().map(Vec::len).sum();
            let (scores, _) = pooled_scores(&per_world, candidates);
            let (twin_diff, pair_spread) = signal(&scores);
            // σ = 0 is the placement-noise control: no direction, no step.
            let (_, update_rms) = if bodies >= 2 && spec.brain_sigma > 0.0 {
                ascend(theta, adam, &scores, spec.brain_sigma, spec.cma_seed, *brain_step)
            } else {
                (0.0, 0.0)
            };
            lineages.push(LineageWindow {
                lineage: f.name().into(),
                bodies,
                twin_diff,
                pair_spread,
                update_rms,
                mean_novelty: mean(&novelty),
                mean_curiosity: mean(&curiosity),
                sd_energy: within_world_sd(&energy),
                mean_delta_body: mean(&part(|b| b.delta_body_normalized)),
                mean_delta_stomach_aware: mean(&part(|b| b.delta_stomach_aware_normalized)),
                sd_novelty: within_world_sd(&novelty),
                moving_share: if minutes > 0 { moving as f64 / minutes as f64 } else { f64::NAN },
                sd_energy_lived: within_world_sd(&lived(|b| {
                    b.score.delta_normalized + b.score.given_normalized
                })),
                sd_novelty_lived: within_world_sd(&lived(|b| b.score.novelty)),
                died_share,
                gave_share,
                centre_key: *k,
            });
        }
        let log = WindowLog {
            generation,
            window,
            brain_step: *brain_step,
            wall_s: started.elapsed().as_secs_f64(),
            body_scores: results.iter().flat_map(|(scores, _)| scores.iter().copied()).collect(),
            world_keys: ran
                .iter()
                .map(|&i| *runs[i].keys.last().expect("it ran a window"))
                .collect(),
            lineages,
        };
        println!("{}", window_line(&log));
        logs.push(log);
        *brain_step += 1;
        window += 1;
    }
    Ok(runs.into_iter().map(EvalRun::finish).collect())
}

/// One line per window: the worlds stepped, the wall time, and per lineage the bodies,
/// the signal check (twin |Δ| against the pair-mean spread) and the step's size.
pub fn window_line(w: &WindowLog) -> String {
    let mut s = format!(
        "  w {}.{} step {} {:.0}s worlds {}",
        w.generation,
        w.window,
        w.brain_step,
        w.wall_s,
        w.world_keys.len()
    );
    for l in &w.lineages {
        s.push_str(&format!(
            " | {} n={} twin={:.3} pair={:.3} ratio={:.2} rms={:.2e} nov={:.3} cur={:.3} \
             deltaE={:.3} deltaEstar={:.3} sdE={:.3} sdN={:.3} mov={:.2} lived:sdE={:.3} sdN={:.4} died={:.2} gave={:.2}",
            l.lineage,
            l.bodies,
            l.twin_diff,
            l.pair_spread,
            l.twin_diff / l.pair_spread.max(1e-12),
            l.update_rms,
            l.mean_novelty,
            l.mean_curiosity,
            l.mean_delta_body,
            l.mean_delta_stomach_aware,
            l.sd_energy,
            l.sd_novelty,
            l.moving_share,
            l.sd_energy_lived,
            l.sd_novelty_lived,
            l.died_share,
            l.gave_share
        ));
    }
    s
}

fn multipliers(g: &Genome) -> Vec<(String, f64)> {
    g.genes.iter().map(|x| (x.name.clone(), x.value)).collect()
}

/// Whether a plant species counts as **lost** in a candidate world against the default
/// world on the same seed: the candidate started with it, the default kept it, and the
/// candidate either lost it or — where the default holds four stands or more — kept
/// under a quarter of the default's stands of it. The quarter catches the near-losses
/// held-out seeds turn into losses (siphonreed, es3's best, 2026-09-28).
pub fn species_lost(start: usize, end: usize, default_end: usize) -> bool {
    if start == 0 || default_end == 0 {
        return false;
    }
    end == 0 || (default_end >= 4 && 4 * end < default_end)
}

/// Re-score `e` with persistence judged against `reference`, the **default** genome's
/// world on the same seed: a species counts as lost only if the default world kept it
/// (and `e`'s own world started with it), since some worlds lose a species whatever the
/// genome (siphonreed, 2026-09-25). Lineages lost always count.
pub fn rescore(e: &mut Evaluation, reference: &Evaluation) {
    let mut readout = [false; Founder::COUNT];
    for (i, r) in e.readout_only.iter().enumerate().take(Founder::COUNT) {
        readout[i] = *r;
    }
    let lineages = scored_lost(&e.founded, &e.lineages_end, &e.eggs_end, &readout);
    let species = e
        .species_start
        .iter()
        .zip(&e.species_end)
        .zip(&reference.species_end)
        .filter(|((start, end), kept)| species_lost(**start, **end, **kept))
        .count();
    e.trace.missing = lineages + species;
    e.penalty = penalty(&e.trace, &e.band);
}

/// The box penalty for a search point outside the genome's box: squared distance.
fn box_penalty(bounds: &[(f64, f64)], x: &[f64]) -> f64 {
    bounds
        .iter()
        .zip(x)
        .map(|((lo, hi), v)| (lo - v).max(0.0).powi(2) + (v - hi).max(0.0).powi(2))
        .sum::<f64>()
        * 10.0
}

/// A candidate's R, S and F on `world`.
pub fn screen_of(e: &EcologyParams, world: &cubarium_voxel::Config, spec: &ScreenSpec) -> Rsf {
    let voxel_m = world.voxel_m;
    let layers = e.layers(voxel_m);
    rsf(&layers.flora, &layers.fauna, voxel_m, spec)
}

/// A candidate's roster screens on `world` ([`roster`]): night endurance, food access and
/// the predator balance readout (each off unless the search file's `[screen]` names it).
pub fn roster_of(e: &EcologyParams, world: &cubarium_voxel::Config, spec: &ScreenSpec) -> RosterScreen {
    let layers = e.layers(world.voxel_m);
    roster(&layers.flora, &layers.fauna, world, spec)
}

/// One lineage's brain for a check arm: `heuristic` (the fauna's own observation-only
/// forager), `parker` (the same forager, standing still while it feeds,
/// [`crate::es::voxel::controller::Parker`]), `herding` (the browser's forager with cohesion,
/// [`crate::es::voxel::controller::HerdingBrowser`]), or a centre file validated against
/// this build and the lineage.
pub fn brain_driver(founder: Founder, spec: &str) -> Result<EpisodeDriver, String> {
    use crate::es::voxel::controller::VoxelControl;
    match spec {
        "heuristic" => Ok(EpisodeDriver::control(VoxelControl::Heuristic, founder)),
        "parker" => Ok(EpisodeDriver::control(VoxelControl::Parker, founder)),
        "herding" if founder == Founder::Browser => {
            Ok(EpisodeDriver::control(VoxelControl::Herding, founder))
        }
        "herding" => Err(format!(
            "herding reads the browser's cone; the {} lineage has none",
            founder.name()
        )),
        path => {
            let file = VoxelPolicyFile::load_or_transfer(std::path::Path::new(path))?;
            if file.founder()? != founder {
                return Err(format!("{path} is not a {} centre", founder.name()));
            }
            file.driver()
        }
    }
}

fn load_centres(centres: &[(Founder, PathBuf)]) -> Result<Vec<(Founder, VoxelPolicyFile)>, String> {
    let mut out = Vec::new();
    for (f, path) in centres {
        let file = VoxelPolicyFile::load_or_transfer(path)?;
        if file.founder()? != *f {
            return Err(format!("{} is not a {} centre", path.display(), f.name()));
        }
        file.driver()?;
        out.push((*f, file));
    }
    out.sort_by_key(|(f, _)| f.index());
    Ok(out)
}

/// The search genome `search` with the values `start` gives its genes, matched by name;
/// a gene `search` lacks, or a value outside its range there, is an error.
pub fn start_from(search: &Genome, start: &Genome) -> Result<Genome, String> {
    let mut g = search.clone();
    for gene in &start.genes {
        g.set(&gene.name, gene.value)
            .map_err(|e| format!("--start: {e}"))?;
    }
    Ok(g)
}

/// Run the search. Writes `generations.jsonl`, `best.toml` (the lowest-fitness genome
/// evaluated so far), `mean.toml` (the search mean), and each as a resolved parameter
/// file (`best-params.toml`, `mean-params.toml`) under `out`.
pub fn search(spec: &SearchSpec) -> Result<Vec<SearchRecord>, String> {
    super::staging::execute(&spec.out, spec.resume.is_some(), spec.wall_seconds, spec.first_generation_seconds, |run| search_inner(spec, run))
}

fn search_inner(spec: &SearchSpec, run: &std::sync::Arc<super::staging::Run>) -> Result<Vec<SearchRecord>, String> {
    let mut configured = spec.clone();
    configured.eval.required_roster = Some(spec.eval.required_roster.clone().unwrap_or_else(||
        RequiredRoster::declared(&spec.eval.founders, &EcologyParams::Genome(spec.genome.clone()).flora(spec.eval.world.voxel_m))));
    configured.eval.driver_sources = spec.centres.iter().map(|(f, p)| (*f, p.display().to_string())).collect();
    cubarium_voxel::world::set_thread_override(1);
    configured.threads = spec.threads.max(1);
    configured.tail_cores = spec.tail_cores.max(1).min(configured.threads);
    let spec = &mut configured;
    println!("# pool limits: workers={} tail-cores={} world-threads=1 while queued", spec.threads, spec.tail_cores);
    let run_started = Instant::now();
    std::fs::create_dir_all(&spec.out)
        .map_err(|e| format!("cannot create {}: {e}", spec.out.display()))?;
    let files = load_centres(&spec.centres)?;
    let mut thetas: Vec<(Founder, Vec<f64>, Adam)> = files
        .iter()
        .map(|(f, file)| {
            let mut adam = Adam::new(file.theta.len());
            adam.learning_rate = spec.brain_learning_rate;
            (*f, file.theta.clone(), adam)
        })
        .collect();
    let start = spec.genome.clone();
    if let super::start::StartSet::Library(lib) = &spec.eval.start {
        // Established starts take rate-only genes; a gene on any other path is frozen at
        // the start's value there (recorded per evaluation as `start.frozen`).
        let frozen: Vec<&str> = start
            .genes
            .iter()
            .filter(|g| !g.frozen && g.paths.iter().chain(&g.inverse).any(|p| !super::start::is_rate_path(p)))
            .map(|g| g.name.as_str())
            .collect();
        println!(
            "# starts: {} — every candidate on seeds {:?}; genes frozen at the start's values there: {}",
            lib.label(),
            lib.seeds,
            if frozen.is_empty() { "none".to_string() } else { frozen.join(", ") }
        );
    }
    let bounds = start.search_bounds();
    let mean0 = match &spec.start {
        Some(s) => start_from(&start, s)?.to_search(),
        None => start.to_search(),
    };
    let mut cma = Cma::new(mean0, spec.sigma0, spec.lambda, spec.cma_seed);
    let mut best: Option<(f64, Genome, u32)> = None;
    let mut first_generation = 0u32;
    let mut brain_step = 0u32;
    if let Some(dir) = &spec.resume {
        let cp = Checkpoint::load(dir)?;
        if cp.cma.mean().len() != start.searched() {
            return Err(format!(
                "the checkpoint searches {} genes, the search file {}",
                cp.cma.mean().len(),
                start.searched()
            ));
        }
        cma = cp.cma;
        for (f, theta, adam) in thetas.iter_mut() {
            let c = cp
                .centres
                .iter()
                .find(|c| c.lineage == f.name())
                .ok_or_else(|| format!("the checkpoint has no {} centre", f.name()))?;
            if c.theta.len() != theta.len() {
                return Err(format!("the checkpoint's {} centre is another shape", f.name()));
            }
            *theta = c.theta.clone();
            *adam = c.adam.clone();
        }
        first_generation = cp.next_generation;
        brain_step = cp.brain_step;
        best = match cp.best {
            Some((pen, text, g)) => match EcologyParams::parse(&text)? {
                EcologyParams::Genome(genome) => Some((pen, genome, g)),
                _ => return Err("the checkpoint's best is not a genome".into()),
            },
            None => None,
        };
        println!(
            "# resumed from {}: generation {first_generation}, brain step {brain_step}, σ {:.4}",
            dir.display(),
            cma.sigma()
        );
    }
    if spec.resume.is_some() {
        super::staging::repair_log(&spec.out.join("generations.jsonl"), |r| r["generation"].as_u64().is_some_and(|g| g < u64::from(first_generation)))?;
    }
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(spec.resume.is_some())
        .write(true)
        .truncate(spec.resume.is_none())
        .open(spec.out.join("generations.jsonl"))
        .map_err(|e| format!("cannot write the log: {e}"))?;
    let mut records = Vec::new();
    let mut last_gen_wall = 0.0;
    save_checkpoint(&spec.out, first_generation, brain_step, &cma, &thetas, &best)?;
    for generation in first_generation..spec.generations {
        if run.boundary(generation, last_gen_wall) { break; }
        run.check()?;
        run.status(generation, 0, 0.0, "founding", "RUNNING")?;
        spec.eval.staging = Some(super::staging::Segment {
            run: run.clone(), generation, full: generation == first_generation, arm: String::new(),
        });
        let started = Instant::now();
        let seeds: Vec<u64> = (0..spec.seeds_per_generation)
            .map(|s| {
                let k = u64::from(generation) * spec.seeds_per_generation as u64 + s as u64;
                match &spec.eval.start {
                    // A library's starts in turn: every candidate on the same ones.
                    super::start::StartSet::Library(lib) => lib.seeds[(k % lib.seeds.len() as u64) as usize],
                    super::start::StartSet::E0 => spec.seed_base + k * 1000,
                }
            })
            .collect();
        let (mean_start, sigma_start) = (cma.mean().to_vec(), cma.sigma());
        let xs = cma.ask();
        let mut candidates: Vec<(String, Vec<f64>)> = xs
            .iter()
            .enumerate()
            .map(|(k, x)| (format!("c{k}"), x.clone()))
            .collect();
        // The search file's starting genome on the same seeds: the persistence reference
        // and the baseline.
        candidates.push(("default".into(), start.to_search()));
        let genomes: Vec<Genome> = candidates.iter().map(|(_, x)| start.from_search(x)).collect();
        let params: Vec<EcologyParams> = genomes.iter().cloned().map(EcologyParams::Genome).collect();
        let screens: Vec<Rsf> = params
            .iter()
            .map(|p| screen_of(p, &spec.eval.world, &spec.screen))
            .collect();
        let rosters: Vec<RosterScreen> = params
            .iter()
            .map(|p| roster_of(p, &spec.eval.world, &spec.screen))
            .collect();
        let reference_k = candidates.len() - 1;
        // Inside the bands or within the soft margin runs; the default always runs (it is
        // the persistence reference). Any roster refusal refuses.
        let verdicts: Vec<Verdict> = screens
            .iter()
            .zip(&rosters)
            .map(|(s, r)| spec.screen.verdict_with(s, r))
            .collect();
        let run_candidates: Vec<bool> = verdicts
            .iter()
            .enumerate()
            .map(|(k, v)| k == reference_k || *v != Verdict::Refused)
            .collect();
        let drivers: Vec<(Founder, EpisodeDriver)> = thetas
            .iter()
            .map(|(f, t, _)| (*f, EpisodeDriver::gru(t, *f).expect("a validated centre")))
            .collect();
        let jobs: Vec<(usize, u64)> = (0..candidates.len())
            .filter(|&k| run_candidates[k])
            .flat_map(|k| seeds.iter().map(move |&s| (k, s)))
            .collect();
        println!("# pools: workers={} tail-cores={} compute-pool={} water-pool={} max-world-threads={}",
            jobs.len().min(spec.threads), spec.tail_cores, if spec.tail_cores > 1 { spec.tail_cores } else { 0 },
            if spec.tail_cores > 1 { spec.tail_cores } else { 0 }, spec.tail_cores);
        let mut windows = Vec::new();
        let mut evals: Vec<Evaluation> = if spec.brain_pairs > 0 {
            lockstep(
                spec,
                &params,
                &jobs,
                &drivers,
                &mut thetas,
                &mut brain_step,
                generation,
                &mut windows,
            )?
        } else {
            let tables: Vec<Option<VariantTable>> = vec![None; Founder::COUNT];
            let budget = ThreadBudget::new(spec.tail_cores);
            parallel_shared(jobs.clone(), spec.threads, &budget, |(k, seed), b| {
                let mut eval = spec.eval.clone();
                if let Some(s) = &mut eval.staging { s.arm = format!("candidate{k}"); }
                evaluate_shared(&params[k], seed, &drivers, &tables, &eval, Some(b))
            })
        };
        run.check()?;
        let references: Vec<Evaluation> = jobs
            .iter()
            .zip(&evals)
            .filter(|((k, _), _)| *k == reference_k)
            .map(|(_, e)| e.clone())
            .collect();
        for ((_, seed), e) in jobs.iter().zip(evals.iter_mut()) {
            let r = references
                .iter()
                .find(|r| r.base_seed == *seed)
                .expect("a reference per seed");
            rescore(e, r);
        }
        // The runs' fitness first (their penalty plus the soft term): a refused candidate
        // scores above the worst of them.
        let ran: Vec<Option<f64>> = (0..candidates.len())
            .map(|k| {
                run_candidates[k].then(|| {
                    let pens: Vec<f64> = jobs
                        .iter()
                        .zip(&evals)
                        .filter(|((j, _), _)| *j == k)
                        .map(|(_, e)| e.penalty.total)
                        .collect();
                    pens.iter().sum::<f64>() / pens.len().max(1) as f64
                        + spec.screen.soft_with(&screens[k], &rosters[k])
                })
            })
            .collect();
        let worst_run = ran.iter().flatten().copied().fold(f64::NEG_INFINITY, f64::max);
        let mut recs: Vec<CandidateRecord> = Vec::new();
        for (k, (label, x)) in candidates.iter().enumerate() {
            let mine: Vec<Evaluation> = jobs
                .iter()
                .zip(&evals)
                .filter(|((j, _), _)| *j == k)
                .map(|(_, e)| e.clone())
                .collect();
            let bp = box_penalty(&bounds, x);
            let screened_out = verdicts[k] == Verdict::Refused && k != reference_k;
            let fitness = match ran[k] {
                Some(p) => p + bp,
                None => spec.screen.graded_with(&screens[k], &rosters[k], worst_run) + bp,
            };
            recs.push(CandidateRecord {
                label: label.clone(),
                multipliers: multipliers(&genomes[k]),
                fitness,
                box_penalty: bp,
                rsf: [screens[k].r, screens[k].s, screens[k].f],
                refusals: rosters[k].refusals(),
                roster: rosters[k].clone(),
                screen: verdicts[k],
                soft: if run_candidates[k] { spec.screen.soft_with(&screens[k], &rosters[k]) } else { 0.0 },
                screened_out,
                lost_required: mine.iter().map(|e| e.lost_required).sum(),
                lost_required_names: {
                    let mut names = Vec::new();
                    for e in &mine { retain_losses(&mut names, e.lost_required_names.clone()); }
                    names
                },
                evaluations: mine,
            });
        }
        let fitness: Vec<f64> = recs[..xs.len()].iter().map(|r| r.fitness).collect();
        // Best so far, by the in-box fitness (run penalty plus soft term), over the draws
        // and the default, among those the screen let run (inside or in the margin).
        for (k, r) in recs.iter().enumerate() {
            if r.screen == Verdict::Refused {
                continue;
            }
            let pen = r.fitness - r.box_penalty;
            if best.as_ref().is_none_or(|b| pen < b.0) {
                best = Some((pen, genomes[k].clone(), generation));
            }
        }
        cma.tell(&xs, &fitness);
        let record = SearchRecord {
            generation,
            seeds,
            mean_start,
            sigma_start,
            mean: cma.mean().to_vec(),
            sigma: cma.sigma(),
            windows,
            wall_s: started.elapsed().as_secs_f64(),
            elapsed_s: run_started.elapsed().as_secs_f64(),
            candidates: recs,
        };
        println!("{}", summary(&record, best.as_ref().map(|b| b.0)));
        writeln!(log, "{}", serde_json::to_string(&record).map_err(|e| e.to_string())?)
            .map_err(|e| format!("generation log: {e}"))?;
        log.flush().map_err(|e| format!("generation log flush: {e}"))?;
        let mean = EcologyParams::Genome(start.from_search(cma.mean()));
        let note = format!("Ecology search mean after generation {generation}. Training only.");
        mean.save(&spec.out.join("mean.toml"), &note)?;
        mean.params()?.write_beside(&spec.out.join("mean-params"))?;
        if let Some((pen, g, gen_)) = &best {
            let e = EcologyParams::Genome(g.clone());
            e.save(
                &spec.out.join("best.toml"),
                &format!(
                    "Lowest-penalty genome so far ({pen:.4}, generation {gen_}). Training only;\nthe live defaults are unchanged."
                ),
            )?;
            e.params()?.write_beside(&spec.out.join("best-params"))?;
        }
        if spec.brain_pairs > 0 {
            for ((f, theta, _), (_, file)) in thetas.iter().zip(&files) {
                let out = VoxelPolicyFile {
                    theta: theta.clone(),
                    generation: Some(u64::from(generation + 1)),
                    ..file.clone()
                };
                out.write(&spec.out.join(format!("centre-{}.json", f.name())))?;
            }
        }
        save_checkpoint(&spec.out, generation + 1, brain_step, &cma, &thetas, &best)?;
        records.push(record);
        last_gen_wall = started.elapsed().as_secs_f64();
        if (generation + 1 < spec.generations || run.dir.join("STOP").exists()) && run.boundary(generation + 1, last_gen_wall) { break; }
        // Only the current generation's world checkpoints are retained.
        for entry in std::fs::read_dir(spec.out.join("worlds")).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with(&format!("g{generation}-")) { std::fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?; }
        }
    }
    Ok(records)
}

fn save_checkpoint(out: &std::path::Path, next_generation: u32, brain_step: u32,
    cma: &Cma, thetas: &[(Founder, Vec<f64>, Adam)], best: &Option<(f64, Genome, u32)>) -> Result<(), String> {
    Checkpoint { next_generation, brain_step, cma: cma.clone(),
        centres: thetas.iter().map(|(f, theta, adam)| CentreState { lineage: f.name().into(), theta: theta.clone(), adam: adam.clone() }).collect(),
        best: best.as_ref().map(|(pen, g, generation)| (*pen, g.to_toml("best so far"), *generation)),
    }.save(out)
}

fn summary(r: &SearchRecord, best: Option<f64>) -> String {
    let mut f: Vec<f64> = r.candidates.iter().map(|c| c.fitness).collect();
    f.sort_by(f64::total_cmp);
    let screened = r.candidates.iter().filter(|c| c.screened_out).count();
    let mean = r.candidates.iter().find(|c| c.label == "default");
    let mut s = format!(
        "gen {:>3} {:>6.0}s σ={:.3} fit min={:.3} med={:.3} screened={screened} | best so far {:.3}",
        r.generation,
        r.wall_s,
        r.sigma,
        f[0],
        f[f.len() / 2],
        best.unwrap_or(f64::NAN)
    );
    if let Some(m) = mean {
        let end: Vec<String> = m
            .evaluations
            .iter()
            .map(|e| {
                format!(
                    "{}{}",
                    e.trace.animals.last().copied().unwrap_or(0),
                    if e.stopped.is_some() { "!" } else { "" }
                )
            })
            .collect();
        s.push_str(&format!(
            " | default fit={:.3} rsf=[{:.2} {:.2} {:.1}] animals_end={}",
            m.fitness,
            m.rsf[0],
            m.rsf[1],
            m.rsf[2],
            end.join(","),
        ));
    }
    for c in &r.candidates {
        s.push_str(&format!(" | {} lost_required={} [{}]", c.label, c.lost_required, c.lost_required_names.join(";")));
    }
    s
}

/// The held-out check: `arms` (a name and a vector each) × `seeds`, `hours` of the
/// training world with the given water cadence, one CSV per world under `out` with one
/// row per window.
/// The world a check founds: the `[world]` of a host's config file, read as the app reads
/// it (its own water, not stilled), or the training preset.
pub fn check_world_config(world: Option<&std::path::Path>) -> Result<cubarium_voxel::Config, String> {
    match world {
        Some(path) => cubarium_voxel::world_config_from_file(path),
        None => Ok(world::training_config()),
    }
}

/// Refuse a trained centre in a world whose lineage is another mass than its manifest's:
/// the policy's senses were read at that mass (the manifest's `mass_scale`, in the
/// digest). A heuristic reads no mass and runs anywhere.
pub fn check_mass(drivers: &[(Founder, EpisodeDriver)], e: &EcologyParams) -> Result<(), String> {
    let fauna = e.fauna();
    for (f, d) in drivers {
        if !matches!(d.kind(), crate::es::voxel::controller::EpisodeKind::Gru(_)) {
            continue;
        }
        let (world, trained) = (fauna.founder(*f).mass_scale, f.manifest().mass_scale);
        if (world / trained - 1.0).abs() > 1e-9 {
            return Err(format!(
                "the {} centre was trained at mass x{trained} and this world's {} is x{world}",
                f.name(),
                f.name()
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn check(
    world_cfg: &cubarium_voxel::Config,
    arms: &[(String, EcologyParams)],
    seeds: &[u64],
    hours: f64,
    window_min: f64,
    water_every: u32,
    centres: &[(Founder, PathBuf)],
    founders: [usize; Founder::COUNT],
    threads: usize,
    curiosity: &Curiosity,
    out: &std::path::Path,
) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let drivers: Vec<(Founder, EpisodeDriver)> = centres
        .iter()
        .map(|(f, p)| Ok((*f, brain_driver(*f, &p.display().to_string())?)))
        .collect::<Result<_, String>>()?;
    for (_, params) in arms {
        check_mass(&drivers, params)?;
    }
    let jobs: Vec<(usize, u64)> = (0..arms.len())
        .flat_map(|a| seeds.iter().map(move |&s| (a, s)))
        .collect();
    let horizon = (hours * 3600.0 * f64::from(cubarium_voxel_fauna::TICK_HZ)).round() as u64;
    let window_ticks = (window_min * TICKS_PER_MIN as f64).round() as u64;
    let results: Vec<Result<(), String>> = parallel(jobs, threads, |(a, seed)| {
        let (name, params) = &arms[a];
        let cfg = world_cfg.clone();
        let (mut sim, info) = world::found_world(&cfg, seed, founders, params);
        super::install_centres(sim.fauna_mut(), &drivers);
        sim.world_mut().set_water_every(water_every);
        let path = out.join(format!("{name}__seed{seed}.csv"));
        let mut f = std::fs::File::create(&path)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        let (w, fl, fa) = sim.layers();
        let installed = super::start::layers_of(fl, fa);
        let provenance = super::start::StartRecord {
            set: "E0".into(), seed: info.seed, start_tick: w.tick(),
            parameter_changes: super::start::parameter_changes(&EcologyParams::defaults().layers(w.config().voxel_m),
                &params.layers(w.config().voxel_m), &installed),
            drivers: Founder::ALL.iter().map(|f| drivers.iter().find(|(lineage, _)| lineage == f)
                .map_or_else(|| format!("{}=heuristic", f.name()), |(_, d)| {
                    let source = centres.iter().find(|(lineage, _)| lineage == f).map(|(_, p)| p.display().to_string()).unwrap_or_default();
                    format!("{}={} source={source} weights={:016x}", f.name(), d.name(), d.digest())
                })).collect(),
            ..super::start::StartRecord::default()
        };
        std::fs::write(path.with_extension("provenance.json"), serde_json::to_vec_pretty(&provenance).map_err(|e| e.to_string())?)
            .map_err(|e| format!("cannot write provenance: {e}"))?;
        let required = RequiredRoster::declared(&founders, &params.flora(w.config().voxel_m));
        let mut lost_required = required.losses(&Presence::of(&sim));
        let mut header = vec!["minute".to_string(), "world_seed".into(), "animals".into(), "lost_required".into(), "lost_required_names".into()];
        for l in Founder::ALL {
            header.push(format!("{}.population", l.name()));
            header.push(format!("{}.eggs", l.name()));
            header.push(format!("{}.births", l.name()));
            for field in ["live_births", "hatchings", "eggs_laid", "maturations", "r_hat_surplus_extrapolated"] {
                header.push(format!("{}.{field}", l.name()));
            }
            for d in cubarium_voxel_fauna::Departure::ALL {
                header.push(format!("{}.{}", l.name(), d.name()));
            }
            header.push(format!("{}.moving_share", l.name()));
            header.push(format!("{}.mean_novelty", l.name()));
            header.push(format!("{}.readiness", l.name()));
            header.push(format!("{}.reserve_fraction", l.name()));
            header.push(format!("{}.pace_bl_per_s", l.name()));
            for c in ["intake_per_body_h", "walked_m_per_body_h", "nn_m"] {
                header.push(format!("{}.{c}", l.name()));
            }
        }
        for s in Species::ALL {
            header.push(format!("stands.{}", s.name()));
            header.push(format!("dormant.{}", s.name()));
            header.push(format!("seed_banks.{}", s.name()));
        }
        header.extend(
            [
                "foliage",
                "litter",
                "lake_level_m",
                "soil_moisture",
                "tick_ms",
            ]
            .map(String::from),
        );
        let _ = writeln!(f, "{}", header.join(","));
        let tables = vec![None; Founder::COUNT];
        let zeros = vec![0usize; Founder::COUNT];
        let mut elapsed = 0u64;
        // Novelty is a measurement here: the same cells and memory as training.
        let mut memory = Memory::default();
        while elapsed < horizon {
            let ticks = window_ticks.min(horizon - elapsed);
            let mut w = Window::begin_with(&mut sim, &tables, &zeros, ticks, curiosity, memory);
            while !w.done() {
                w.step(&mut sim);
                retain_losses(&mut lost_required, required.losses(&Presence::of(&sim)));
            }
            elapsed += ticks;
            let mut r = w.finish(&sim, 0);
            memory = std::mem::take(&mut r.memory);
            let pop = window::population(&sim);
            let get = |n: &str| {
                r.readout
                    .iter()
                    .find(|x| x.0 == n)
                    .map_or(f64::NAN, |x| x.2)
            };
            let mut row = vec![
                format!("{:.0}", elapsed as f64 / TICKS_PER_MIN as f64),
                info.seed.to_string(),
                sim.fauna().view().animals.len().to_string(),
                lost_required.len().to_string(),
                lost_required.join(";"),
            ];
            for l in Founder::ALL {
                let s = &r.lineages[l.index()];
                row.push(pop[l.index()].to_string());
                row.push(sim.fauna().view().eggs_by_founder(l).to_string());
                row.push(s.births.to_string());
                for count in [s.live_births, s.hatchings, s.eggs_laid, s.maturations] { row.push(count.to_string()); }
                row.push(s.balance.r_hat.to_string());
                for d in s.deaths {
                    row.push(d.to_string());
                }
                row.push(format!(
                    "{:.4}",
                    if s.body_minutes > 0 {
                        s.moving_minutes as f64 / s.body_minutes as f64
                    } else {
                        f64::NAN
                    }
                ));
                row.push(format!("{:.4}", s.mean_novelty()));
                // Breeding readiness and reserve over cap, means over the living bodies
                // now.
                let fauna = sim.fauna();
                let core = fauna.config().founder(l).core;
                let (mut rd, mut rf, mut n) = (0.0, 0.0, 0.0);
                for a in fauna.view().animals.iter().filter(|a| a.founder == Some(l)) {
                    rd += body_readiness(a, &core);
                    rf += a.reserve / core.reserve_of(a.body).max(1e-300);
                    n += 1.0;
                }
                row.push(format!("{:.4}", if n > 0.0 { rd / n } else { f64::NAN }));
                row.push(format!("{:.4}", if n > 0.0 { rf / n } else { f64::NAN }));
                row.push(format!(
                    "{:.4}",
                    get(&format!("{}.pace_bl_per_s", l.name()))
                ));
                for c in ["intake_per_body_h", "walked_m_per_body_h", "nn_m"] {
                    row.push(format!("{:.6}", get(&format!("{}.{c}", l.name()))));
                }
            }
            let scan = Scan::of(&sim);
            let presence = Presence::of(&sim);
            for s in Species::ALL {
                row.push(scan.stands[s.index()].to_string());
                row.push(presence.dormant_plants[s.index()].to_string());
                row.push(presence.seed_banks[s.index()].to_string());
            }
            row.push(format!("{:.3}", scan.foliage));
            row.push(format!("{:.3}", scan.litter));
            row.push(format!("{:.3}", scan.lake_level_m));
            row.push(format!("{:.4}", scan.soil_moisture));
            row.push(format!("{:.3}", get("cost.tick_ms")));
            let _ = writeln!(f, "{}", row.join(","));
            let _ = f.flush();
        }
        println!("{name} seed {seed} done");
        Ok(())
    });
    results.into_iter().collect()
}

/// **The finalist horizon**, hours: `from` plus the longest of the next night after it
/// (from `from` through the following dawn), the pack's pup independence and its litter
/// interval (a founded pack's; none without one). About 8 h on the terrarium.
pub fn finalist_hours(world: &cubarium_voxel::Config, fauna: &cubarium_voxel_fauna::FaunaConfig, from: f64) -> f64 {
    let day = &world.day;
    let mut wait_s: f64 = 0.0;
    if day.mode == cubarium_voxel::weather::DayMode::Cycle {
        let len = day.length_min * 60.0;
        let (rise, set) = day.cycle_rise_set();
        let tick = (from * 3600.0 * f64::from(cubarium_voxel_fauna::TICK_HZ)).round() as u64;
        let p = day.cycle_phase(tick);
        let to_dusk = (set - p).rem_euclid(1.0);
        let night = (rise - set).rem_euclid(1.0);
        wait_s = (to_dusk + night) * len;
    }
    for f in Founder::ALL {
        let p = fauna.founder(f);
        if let (Some(k), true) = (p.pack, p.founding_count > 0) {
            wait_s = wait_s.max(k.independence_age_s).max(k.litter_interval_s);
        }
    }
    from + wait_s / 3600.0
}

/// **Finalist validation** (proposal Part 3; Astra A10): each arm × seed founded as the
/// search founds it and run to [`finalist_hours`], judged by the search's own evaluation
/// under the arm's `[targets]` (v3, else the readout rules) with the lanternjaw held to an
/// established hunter (`lanternjaw_progress` 1). Worlds are refounded on their seeds, not
/// restored from a save. Writes `<arm>__seed<s>.json` (the [`Evaluation`]) under `out` and
/// prints each lineage's targets met per arm.
///
/// With `hours` given (`voxel-ecology-check --targets`), every world runs that long
/// instead and is judged under the arm's own `[targets]` unchanged: the search's
/// evaluation of one candidate, as the exhibit run uses it.
#[allow(clippy::too_many_arguments)]
pub fn finalist(
    world_cfg: &cubarium_voxel::Config,
    arms: &[(String, EcologyParams)],
    seeds: &[u64],
    hours: Option<f64>,
    from_hours: f64,
    window_min: f64,
    water_every: u32,
    centres: &[(Founder, PathBuf)],
    founders: [usize; Founder::COUNT],
    threads: usize,
    tail_cores: usize,
    out: &std::path::Path,
) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let drivers: Vec<(Founder, EpisodeDriver)> = centres
        .iter()
        .map(|(f, p)| Ok((*f, brain_driver(*f, &p.display().to_string())?)))
        .collect::<Result<_, String>>()?;
    let hz = f64::from(cubarium_voxel_fauna::TICK_HZ);
    let mut specs = Vec::new();
    for (name, params) in arms {
        check_mass(&drivers, params)?;
        let hours = hours.unwrap_or_else(|| finalist_hours(world_cfg, &params.fauna(), from_hours));
        let validating = hours > from_hours;
        let horizon = (hours * 3600.0 * hz).round() as u64;
        let mut eval = EvalSpec::new(world_cfg.clone(), horizon, (window_min * 60.0 * hz).round() as u64);
        eval.founders = founders;
        eval.driver_sources = centres.iter().map(|(f, p)| (*f, p.display().to_string())).collect();
        eval.water_every = water_every;
        eval.stop_on_loss = false;
        let rest = params.genome().map(|g| g.rest.clone()).unwrap_or_default();
        eval.band = BandSpec::default().with_targets(&rest)?;
        let mut stage = eval.band.stage.unwrap_or_else(StageSpec::readout_default);
        if validating {
            stage.lanternjaw_progress = 1.0;
            stage.chorister_kill_required = true;
        }
        eval.band.stage = Some(stage);
        println!("# {name}: {hours:.2} h per world");
        specs.push(eval);
    }
    let jobs: Vec<(usize, u64)> = (0..arms.len())
        .flat_map(|a| seeds.iter().map(move |&s| (a, s)))
        .collect();
    let tables: Vec<Option<VariantTable>> = vec![None; Founder::COUNT];
    let budget = ThreadBudget::new(tail_cores);
    let evals: Vec<Evaluation> = parallel_shared(jobs.clone(), threads, &budget, |(a, seed), b| {
        evaluate_shared(&arms[a].1, seed, &drivers, &tables, &specs[a], Some(b))
    });
    for ((a, seed), e) in jobs.iter().zip(&evals) {
        let path = out.join(format!("{}__seed{seed}.json", arms[*a].0));
        let json = serde_json::to_string(e).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    for (a, (name, _)) in arms.iter().enumerate() {
        println!("{name}: lineage  met/worlds  alive_end  births  (score)");
        for f in Founder::ALL {
            let rows: Vec<&LineageOutcome> = jobs
                .iter()
                .zip(&evals)
                .filter(|((k, _), _)| *k == a)
                .filter_map(|(_, e)| e.lineages.iter().find(|o| o.founder == f))
                .collect();
            if rows.is_empty() {
                continue;
            }
            let met = rows.iter().filter(|o| o.met).count();
            let stage_met = rows.iter().filter(|o| o.stage_met).count();
            let repl_met = rows.iter().filter(|o| o.replenishment_met).count();
            let alive: usize = rows.iter().map(|o| o.alive_end).sum();
            let births: u64 = rows.iter().map(|o| o.births).sum();
            let score = rows.iter().map(|o| o.score).sum::<f64>() / rows.len() as f64;
            let late: u64 = rows.iter().map(|o| o.late_births).sum();
            let repl: Vec<String> = rows.iter().map(|o| format!("{:.2}", o.replenishment)).collect();
            println!(
                "  {:<14}{} met {met}/{} (stage {stage_met}, replenishment {repl_met})  alive {alive:>4}  births {births:>4} (late {late:>3})  score {score:.2}  replenish [{}]",
                f.name(),
                if rows.iter().any(|o| o.readout) { " (readout)" } else { "" },
                rows.len(),
                repl.join(" ")
            );
        }
        for ((k, seed), e) in jobs.iter().zip(&evals) {
            if *k != a {
                continue;
            }
            println!(
                "  seed {seed}: penalty {:.3} (bound {:.3} lively {:.3} share {:.3} persist {:.3} stage {:.3} replenish {:.3}) animals_end {} stopped {:?} wall {:.0}s",
                e.penalty.total,
                e.penalty.bound,
                e.penalty.lively,
                e.penalty.share,
                e.penalty.persist,
                e.penalty.stage,
                e.penalty.replenish,
                e.trace.animals.last().copied().unwrap_or(0),
                e.stopped,
                e.wall_s
            );
        }
    }
    Ok(())
}
