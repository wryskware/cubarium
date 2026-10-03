//! **Targets v3: per lineage and life stage** (`design/ecology-timescales-2026-10-02.md`
//! Part 3; Astra A10; `design/handoffs/roster-search-groundwork-2026-10-02.md` §3). A search
//! file opts in with `[targets] version = 3`; without it the v2 breeding term stands.
//!
//! Two hours cannot hold a replacement generation for every lineage, so each is held to
//! what fits:
//! - **Birth required** (`birth_required`): an actual birth in the second half. Readiness
//!   earns at most `birth_credit_max` of the term as a gradient and never meets it.
//! - **Readout-only** (`readout_only`): judged and reported, never scored (the pilot's
//!   broken feeders, Astra F1).
//! - **The lanternjaw's graded objective** (`progress`; [`lanternjaw_score`] is current,
//!   the list below its first version): 0.25 hatched from the founding egg bank,
//!   0.5 a rider attached (up to 0.75 as the furthest rider grows toward its drop-off
//!   line), 0.75 a juvenile off its host (dropped off or freed) and living
//!   free, 1 an established hunter (a free lanternjaw's hit landed) or eggs laid by an
//!   adult. Met at `lanternjaw_progress`. Bank hatches, riders and free-living hunters are
//!   counted apart.
//! - **The chorister's pups** (`progress`): a litter (in either half: an early one is a
//!   success, the next is six hours off), pups provisioned, pup survival at least
//!   `chorister_pup_survival`, and the pack's own intake (a kill or carcass feeding; a
//!   pack kill in finalist validation). A quarter each; met at all four.
//! - **Replenishment** (`replenish`): over one whole day-cycle, intake over the cycle's
//!   measured spending, `intake / (intake − Δstock)` with the stock every living body's
//!   structure, reserve, open escrow and **digestible stomach** (the minimal gut,
//!   [`digestible_stomach`]), and intake the assimilation plus the stomach's digestible
//!   change ([`cycle_books`]): food moved from stomach to reserve is not income. At least
//!   1; a rise after one meal cannot hide a negative day.

use cubarium_voxel_fauna::{Animal, Food, Founder, FounderPhysiology};
use serde::Serialize;

/// The v3 targets as a search file's `[targets]` sets them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct StageSpec {
    pub birth_required: [bool; Founder::COUNT],
    /// The lanternjaw and the chorister: their ladders instead of a birth.
    pub progress: [bool; Founder::COUNT],
    pub replenish: [bool; Founder::COUNT],
    /// **Readout-only** (`readout_only`, the pilot's broken feeders; Astra F1): judged and
    /// reported, but no stage, replenishment or loss penalty, no early stop on their loss,
    /// and out of the population band.
    pub readout_only: [bool; Founder::COUNT],
    /// The most of a birth term readiness can earn back (never all of it).
    pub birth_credit_max: f64,
    /// Readiness at which that credit is full.
    pub ready_target: f64,
    pub lanternjaw_progress: f64,
    pub chorister_pup_survival: f64,
    /// The replenishment term's weight per unit of shortfall.
    pub replenish_weight: f64,
    /// The lanternjaw share term's weight while no lanternjaw lives free.
    pub share_weight_unestablished: f64,
    /// Finalist validation: the chorister's intake criterion asks a pack kill, not carcass
    /// feeding alone.
    pub chorister_kill_required: bool,
    /// The measured predator balance's share of prey births per hunter lineage (a
    /// readout, Astra A6's 50 %).
    pub balance_share: f64,
}

impl Default for StageSpec {
    fn default() -> StageSpec {
        StageSpec {
            birth_required: [false; Founder::COUNT],
            progress: [false; Founder::COUNT],
            replenish: [false; Founder::COUNT],
            readout_only: [false; Founder::COUNT],
            birth_credit_max: 0.5,
            ready_target: 0.5,
            lanternjaw_progress: 0.5,
            chorister_pup_survival: 0.5,
            replenish_weight: 5.0,
            share_weight_unestablished: 1.0,
            balance_share: 0.5,
            chorister_kill_required: false,
        }
    }
}

fn lineages(v: &toml::Value, key: &str) -> Result<[bool; Founder::COUNT], String> {
    let names = v.as_array().ok_or_else(|| format!("{key} is a list of lineages"))?;
    let mut out = [false; Founder::COUNT];
    for n in names {
        let n = n.as_str().ok_or_else(|| format!("{key} names lineages"))?;
        let f = Founder::ALL
            .into_iter()
            .find(|f| f.name() == n)
            .ok_or_else(|| format!("[targets] {key}: no lineage {n:?}"))?;
        out[f.index()] = true;
    }
    Ok(out)
}

fn number(v: &toml::Value, key: &str) -> Result<f64, String> {
    v.as_float()
        .or_else(|| v.as_integer().map(|i| i as f64))
        .filter(|x| x.is_finite() && *x >= 0.0)
        .ok_or_else(|| format!("[targets] {key} is a number >= 0"))
}

impl StageSpec {
    /// The rules a v2 search's readout judges by (no penalty reads them): a birth for every
    /// lineage but the lanternjaw and the chorister, which climb their ladders, and
    /// replenishment for every lineage but the lanternjaw.
    pub fn readout_default() -> StageSpec {
        let mut s = StageSpec::default();
        for f in Founder::ALL {
            let ladder = matches!(f, Founder::Lanternjaw | Founder::Chorister);
            s.birth_required[f.index()] = !ladder;
            s.progress[f.index()] = ladder;
            s.replenish[f.index()] = f != Founder::Lanternjaw;
        }
        s
    }

    /// The v3 keys of a `[targets]` table: `Some` when it says `version = 3`. Keys of v2
    /// (and the band's) are left to their reader.
    pub fn from_targets(t: &toml::Table) -> Result<Option<StageSpec>, String> {
        match t.get("version").and_then(|v| v.as_integer()) {
            Some(3) => {}
            Some(v) if v != 2 => return Err(format!("[targets] version {v}: 2 or 3")),
            _ => return Ok(None),
        }
        let mut s = StageSpec::default();
        for (k, v) in t {
            match k.as_str() {
                "birth_required" => s.birth_required = lineages(v, k)?,
                "progress" => s.progress = lineages(v, k)?,
                "replenish" => s.replenish = lineages(v, k)?,
                "readout_only" => s.readout_only = lineages(v, k)?,
                "birth_credit_max" => s.birth_credit_max = number(v, k)?.min(0.99),
                "ready_target" => s.ready_target = number(v, k)?.max(1e-9),
                "lanternjaw_progress" => s.lanternjaw_progress = number(v, k)?,
                "chorister_pup_survival" => s.chorister_pup_survival = number(v, k)?,
                "replenish_weight" => s.replenish_weight = number(v, k)?,
                "share_weight_unestablished" => s.share_weight_unestablished = number(v, k)?,
                "balance_share" => s.balance_share = number(v, k)?,
                _ => {}
            }
        }
        Ok(Some(s))
    }
}

/// Which rule a lineage is held to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Rule {
    Birth,
    LanternjawProgress,
    ChoristerProgress,
    /// Persistence alone (and replenishment, if listed).
    Persist,
}

/// **One lineage's outcome** in one world: what it did, and the verdict ([`judge`]).
#[derive(Clone, Debug, Serialize)]
pub struct LineageOutcome {
    pub lineage: &'static str,
    pub founder: Founder,
    /// Founders (bodies, or clutches for an egg bank).
    pub founded: usize,
    /// Living bodies at the end (riders included) and eggs.
    pub alive_end: usize,
    pub eggs_end: usize,
    /// Bodies born in the second half, and in the whole run (by id, minute by minute).
    pub late_births: u64,
    pub births: u64,
    /// Second-half mean breeding readiness.
    pub readiness: f64,
    /// Hatches out of the founding egg bank (an egg-bank lineage), and eggs adults laid.
    pub bank_hatches: u64,
    pub eggs_laid: u64,
    /// Riders now, and attached, dropped off and freed over the run.
    pub riders_now: usize,
    pub riders_attached: u64,
    pub rider_dropoffs: u64,
    pub riders_freed: u64,
    /// Bodies living free (not riding) at the end.
    pub free_living: usize,
    /// The riders now: their mean growth since hatching, body per hour, and their mean
    /// progress from hatch size to the drop-off line by actual size gain, 0 to 1.
    pub rider_growth_per_h: f64,
    pub rider_progress: f64,
    /// Free-living lanternjaws at adult size (`birth_body`) or more, and the furthest free
    /// juvenile's progress from hatch size to it, 0 to 1.
    pub free_adults: usize,
    pub free_progress: f64,
    /// Strikes that hit, and carrion its mouths took (gross organic).
    pub hits: u64,
    pub carrion_eaten: f64,
    /// Pack chases that ended in a kill (a pack lineage). Not strict attribution: the
    /// fauna counts a target gone with carrion at its site (Astra F5).
    pub pack_kills: u64,
    /// Assimilated intake over the whole run (the ledger's), organic.
    pub intake_run: f64,
    /// A pack's litters, pups born, pups alive (still pups) and grown past independence at
    /// the end, and what pups received from provisioning (organic).
    pub litters: u64,
    pub pups_born: u64,
    pub pups_alive: usize,
    pub pups_independent: usize,
    pub provision_received: f64,
    /// One whole day-cycle from founding ([`cycle_books`]): intake (assimilated plus the
    /// stomach's digestible change), and the lineage's stock (biomass plus digestible
    /// stomach) at its start and its end. `cycle_complete` false when the world stopped
    /// first.
    pub intake_cycle: f64,
    pub biomass_start: f64,
    pub biomass_cycle: f64,
    pub cycle_complete: bool,
    pub replenishment: f64,
    /// Readout-only: judged, never scored.
    pub readout: bool,
    /// The verdict.
    pub rule: Rule,
    /// The ladder's score (progress rules), 0 to 1.
    pub score: f64,
    /// The stage term, 0 (met) to 1.
    pub shortfall: f64,
    /// **Three verdicts** (Astra F4): the stage target met; replenishment met (a lineage
    /// held to it: a complete cycle at 1 or more; otherwise true); and both.
    pub stage_met: bool,
    pub replenishment_met: bool,
    pub met: bool,
    /// The replenishment term, 0 to 1 (0 for a lineage not held to it).
    pub replenish_shortfall: f64,
}

impl LineageOutcome {
    /// A lineage founded with `founded`, nothing yet observed.
    pub fn new(founder: Founder, founded: usize) -> LineageOutcome {
        LineageOutcome {
            lineage: founder.name(),
            founder,
            founded,
            alive_end: 0,
            eggs_end: 0,
            late_births: 0,
            births: 0,
            readiness: 0.0,
            bank_hatches: 0,
            eggs_laid: 0,
            riders_now: 0,
            riders_attached: 0,
            rider_dropoffs: 0,
            riders_freed: 0,
            free_living: 0,
            rider_growth_per_h: 0.0,
            rider_progress: 0.0,
            free_adults: 0,
            free_progress: 0.0,
            hits: 0,
            carrion_eaten: 0.0,
            pack_kills: 0,
            intake_run: 0.0,
            litters: 0,
            pups_born: 0,
            pups_alive: 0,
            pups_independent: 0,
            provision_received: 0.0,
            intake_cycle: 0.0,
            biomass_start: 0.0,
            biomass_cycle: 0.0,
            cycle_complete: false,
            replenishment: 0.0,
            readout: false,
            rule: Rule::Persist,
            score: 0.0,
            shortfall: 0.0,
            stage_met: true,
            replenishment_met: true,
            met: true,
            replenish_shortfall: 0.0,
        }
    }

    /// Neither a body nor an egg left.
    pub fn lost(&self) -> bool {
        self.founded > 0 && self.alive_end + self.eggs_end == 0
    }
}

/// **What a body's stomach is worth to its reserve** (the minimal gut), organic units: each
/// food class's raw organic times the yield it digests at, mineral-limited as digestion
/// limits it (`min(yield, mineral / (n_tissue · organic))`). Zero for a body with no
/// lineage.
pub fn digestible_stomach(a: &Animal, p: &FounderPhysiology) -> f64 {
    let n = p.core.n_tissue;
    Food::ALL
        .iter()
        .map(|&f| {
            let pool = a.stomach.pools[f.index()];
            if !(pool.organic > 0.0) {
                return 0.0;
            }
            let funded = if n > 0.0 { pool.mineral / (n * pool.organic) } else { f64::INFINITY };
            pool.organic * p.yield_for(f).min(funded).max(0.0)
        })
        .sum()
}

/// **The cycle's books under the minimal gut**: `(intake, stock_start, stock_end)` from the
/// lineage's assimilation over the cycle, its biomass at both ends and its digestible
/// stomach at both ends. The stomach is counted as stock, and food entering it as intake,
/// so digesting what was already held moves nothing: spending (`intake − Δstock`) is
/// `assimilated − Δbiomass` either way.
pub fn cycle_books(
    assimilated: f64,
    biomass_start: f64,
    biomass_end: f64,
    stomach_start: f64,
    stomach_end: f64,
) -> (f64, f64, f64) {
    (
        assimilated + (stomach_end - stomach_start),
        biomass_start + stomach_start,
        biomass_end + stomach_end,
    )
}

/// **Replenishment**: intake over spending, `intake / (intake − (end − start))`; 0 with no
/// intake, capped at 10 when the lineage gained more than it spent.
pub fn replenishment(intake: f64, biomass_start: f64, biomass_end: f64) -> f64 {
    if !(intake > 0.0) {
        return 0.0;
    }
    let spent = intake - (biomass_end - biomass_start);
    if spent <= intake / 10.0 { 10.0 } else { intake / spent }
}

/// **The lanternjaw's graded objective** (Astra F2), 0 to 1, of **living** bodies only:
/// - 1: a free adult (at `birth_body`), a free lanternjaw's hit landed, or eggs laid;
/// - 0.75 + 0.2 x the free juvenile's progress to adult size: a healthy drop-off living
///   free (a host-death release earns no juvenile credit);
/// - 0.5 + 0.25 x rider survival (live riders over attached) x their mean progress to the
///   drop-off line by actual size gain: a live rider (attach-and-die earns nothing);
/// - 0.25: any other living lanternjaw (a hatchling, a released rider);
/// - 0: none living, whatever eggs remain.
fn lanternjaw_score(o: &LineageOutcome) -> f64 {
    if o.free_adults > 0 || (o.free_living > 0 && o.hits > 0) || o.eggs_laid > 0 {
        1.0
    } else if o.free_living > 0 && o.rider_dropoffs > 0 {
        0.75 + 0.2 * o.free_progress.clamp(0.0, 1.0)
    } else if o.riders_now > 0 {
        let survival = (o.riders_now as f64 / o.riders_attached.max(1) as f64).min(1.0);
        0.5 + 0.25 * survival * o.rider_progress.clamp(0.0, 1.0)
    } else if o.alive_end > 0 {
        0.25
    } else {
        0.0
    }
}

/// The chorister's four quarters: a litter, pups provisioned, pup survival, and the
/// pack's own intake (a kill or carcass feeding; a pack kill when `kill_required`).
fn chorister_score(o: &LineageOutcome, survival_min: f64, kill_required: bool) -> f64 {
    let litter = o.litters > 0;
    let fed = o.provision_received > 0.0;
    let pups_now = (o.pups_alive + o.pups_independent) as f64;
    let survived = o.pups_born > 0 && pups_now / o.pups_born as f64 >= survival_min;
    // Real assimilated intake, never a hit or a bite alone (Astra F5); at finalist stage a
    // pack kill as well.
    let ate = o.intake_run > 0.0 && (!kill_required || o.pack_kills > 0);
    [litter, fed, survived, ate].iter().filter(|x| **x).count() as f64 / 4.0
}

/// **Judge one lineage** under `spec`: its rule, score, stage shortfall, whether met, and
/// its replenishment. `reached_second_half` false (the world stopped first) fails a birth.
pub fn judge(o: &mut LineageOutcome, spec: &StageSpec, reached_second_half: bool) {
    let i = o.founder.index();
    let lost = o.lost();
    o.rule = if spec.progress[i] && o.founder == Founder::Lanternjaw {
        Rule::LanternjawProgress
    } else if spec.progress[i] && o.founder == Founder::Chorister {
        Rule::ChoristerProgress
    } else if spec.birth_required[i] {
        Rule::Birth
    } else {
        Rule::Persist
    };
    let (score, shortfall, met) = match o.rule {
        Rule::Birth => {
            if lost || !reached_second_half {
                (0.0, 1.0, false)
            } else if o.late_births > 0 {
                (1.0, 0.0, true)
            } else {
                let credit = spec.birth_credit_max * (o.readiness / spec.ready_target).min(1.0);
                (0.0, 1.0 - credit, false)
            }
        }
        Rule::LanternjawProgress => {
            // Graded above the pass line: the term is 1 − score, met at the line.
            let s = lanternjaw_score(o);
            (s, 1.0 - s, s >= spec.lanternjaw_progress)
        }
        Rule::ChoristerProgress => {
            let s = if lost { 0.0 } else { chorister_score(o, spec.chorister_pup_survival, spec.chorister_kill_required) };
            (s, 1.0 - s, s >= 1.0 - 1e-12)
        }
        Rule::Persist => (if lost { 0.0 } else { 1.0 }, 0.0, !lost),
    };
    o.score = score;
    o.shortfall = shortfall;
    o.stage_met = met;
    o.replenishment = replenishment(o.intake_cycle, o.biomass_start, o.biomass_cycle);
    o.readout = spec.readout_only[i];
    o.replenish_shortfall = if !spec.replenish[i] || o.founded == 0 {
        0.0
    } else if !o.cycle_complete {
        1.0
    } else {
        (1.0 - o.replenishment).clamp(0.0, 1.0)
    };
    o.replenishment_met =
        !spec.replenish[i] || o.founded == 0 || (o.cycle_complete && o.replenishment >= 1.0);
    o.met = o.stage_met && o.replenishment_met;
    if o.readout {
        o.shortfall = 0.0;
        o.replenish_shortfall = 0.0;
    }
}
