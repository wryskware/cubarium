//! **The per-decision reward interface** (PG-PILOT; SCORE-V2). A window's record per body
//! is its decisions, its energy every tick ([`PgBody::series`]), its drives every tick
//! ([`PgBody::drives`]) and its life events (its stage when it died, its mother when it was
//! born); a [`RewardTerm`] reads a body's reward over any tick span, and a decision's reward
//! is every term's sum over the span until the body's next decision. The learner sees only
//! the sums ([`super::Trajectory`]), so a term joins here without touching it.
//!
//! **The score is split** (SCORE-V2, Wrysk's "Score topic 1"):
//!
//! - **The judge**, outcomes only, what every comparison and check reads ([`JUDGE_TERMS`]):
//!   - [`OwnEnergy`] (`own`): the body's own condition, `(E + E_given)/body_ref` at the
//!     span's end minus at its start, with `E = 0` from its death on: over a whole window
//!     this is the ES score's `(E_end − E_start + E_given)/body_ref` exactly, the loss of
//!     its stored energy at death included;
//!   - [`LifeValue`] (`life`): the life value lost at its death, at the stage it died in,
//!     and a young's added at each own young's birth ([`super::knobs::JudgeKnobs`]);
//!   - [`KinEnergy`] (`kin`) and the kin life term (`kin_life`): `r ×` each relative's own
//!     condition change and life events over the same span (§10.3; the window's roster,
//!     newborns joined at birth), each relative at its own `body_ref`.
//! - **The teacher**, what the learner trains on: the judge per step plus each drive in
//!   potential form ([`Drive`]: `weight × (drive before − drive after)`), so over a window a
//!   drive term telescopes to `weight × (start − end)` and cannot raise the return by a loop
//!   ([`super::knobs::TeacherKnobs`]).
//!
//! A term at weight 0 is left out ([`teacher_terms`]), so a kin file with every SCORE-V2 knob
//! off sums exactly the two terms pilot v1 summed.
//!
//! The curiosity term (`config/training/curiosity.toml`) is not wired: a novelty term would
//! be one more [`RewardTerm`].

use cubarium_voxel_fauna::Founder;
use rustc_hash::FxHashMap;

use super::judge::{DRIVE_NAMES, DRIVES, LifeStage};
use super::knobs::PgKnobs;
use super::learner::{Step, TopInput, Trajectory};
use crate::es::voxel::kin::{KinShares, Roster};
use crate::es::voxel::two_layer::TopDecision;

/// One body's window: its energy every tick and its decisions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PgBody {
    pub id: u64,
    /// The window tick `series[0]` was read at (0, or a newborn's first tick).
    pub start: u64,
    /// `(E + E_given − E_given at start)/body_ref` after each tick from `start` on; from a
    /// death on, `E = 0` and the series ends.
    pub series: Vec<f64>,
    pub died: bool,
    /// Its decisions in tick order.
    pub decisions: Vec<TopDecision>,
    /// Its drives after each tick, aligned with `series` ([`super::judge::drives`]): from a
    /// death on, the last reading (a death neither pays nor relieves a drive).
    pub drives: Vec<[f64; DRIVES]>,
    /// Its life stage at its last reading: for a body that died, the stage it died in.
    pub stage: LifeStage,
    /// Its mother's id when it was born in the window (a newborn of a recorded mother);
    /// `None` for a body standing at the start, introduced, or hatched from an egg (a clutch
    /// records no parent).
    pub born_to: Option<u64>,
}

impl PgBody {
    /// The series at window tick `t`: before its first reading the first, past its last
    /// reading (a death, the window's end) the last.
    pub fn s(&self, t: u64) -> f64 {
        if self.series.is_empty() {
            return 0.0;
        }
        let i = t.saturating_sub(self.start) as usize;
        self.series[i.min(self.series.len() - 1)]
    }

    /// The window tick of its last reading.
    pub fn end(&self) -> u64 {
        self.start + self.series.len().saturating_sub(1) as u64
    }

    /// Drive `k` at window tick `t`, read as [`PgBody::s`] reads the series (0 with no
    /// reading).
    pub fn drive(&self, t: u64, k: usize) -> f64 {
        if self.drives.is_empty() {
            return 0.0;
        }
        let i = t.saturating_sub(self.start) as usize;
        self.drives[i.min(self.drives.len() - 1)][k]
    }
}

/// One world's window for one lineage.
#[derive(Clone, Debug, PartialEq)]
pub struct PgWorld {
    pub founder: Founder,
    pub bodies: Vec<PgBody>,
}

/// One term of a decision's reward.
pub trait RewardTerm: Send + Sync {
    fn name(&self) -> &'static str;
    /// Body `b`'s (an index into `w.bodies`) reward over window ticks `from..to`.
    fn reward(&self, w: &PgWorld, b: usize, from: u64, to: u64) -> f64;
}

/// The body's own net energy, the loss at death included.
#[derive(Clone, Copy, Debug, Default)]
pub struct OwnEnergy;

impl RewardTerm for OwnEnergy {
    fn name(&self) -> &'static str {
        "own"
    }
    fn reward(&self, w: &PgWorld, b: usize, from: u64, to: u64) -> f64 {
        let x = &w.bodies[b];
        x.s(to) - x.s(from)
    }
}

/// `r ×` each relative's `own` term over the span (§10.3): [`KinEnergy`] over
/// [`OwnEnergy`], the kin life term over [`LifeValue`].
#[derive(Clone, Debug, Default)]
pub struct Kin<T> {
    /// Per body, its relatives on the record: `(index, share)`.
    relatives: Vec<Vec<(usize, f64)>>,
    own: T,
    name: &'static str,
}

/// `r ×` each relative's own net energy over the span (§10.3).
pub type KinEnergy = Kin<OwnEnergy>;

impl<T: RewardTerm> Kin<T> {
    /// The relatives of every recorded body, by the window's `roster`, at `shares`, each
    /// read by `own`. A relative not recorded (another lineage, or never alive in the
    /// window) adds nothing.
    pub fn of(w: &PgWorld, roster: &Roster, shares: &KinShares, own: T, name: &'static str) -> Kin<T> {
        let index: FxHashMap<u64, usize> = w.bodies.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
        let relatives = w
            .bodies
            .iter()
            .map(|b| {
                roster
                    .relatives(b.id)
                    .iter()
                    .filter_map(|(other, rel)| {
                        let r = shares.of(*rel);
                        index.get(other).filter(|_| r != 0.0).map(|&i| (i, r))
                    })
                    .collect()
            })
            .collect();
        Kin { relatives, own, name }
    }
}

impl KinEnergy {
    /// The kin energy term (`kin`), pilot v1's.
    pub fn new(w: &PgWorld, roster: &Roster, shares: &KinShares) -> KinEnergy {
        Kin::of(w, roster, shares, OwnEnergy, "kin")
    }
}

impl<T: RewardTerm> RewardTerm for Kin<T> {
    fn name(&self) -> &'static str {
        self.name
    }
    fn reward(&self, w: &PgWorld, b: usize, from: u64, to: u64) -> f64 {
        self.relatives[b].iter().map(|&(i, r)| r * self.own.reward(w, i, from, to)).sum()
    }
}

/// **The life value** (the judge; SCORE-V2 "Score topic 2"): per body, `−life(stage)` at its
/// death (the stage it died in) and `+life(young)` at the birth of each young whose
/// mother it is, each counted in the span `(from, to]` that holds its tick.
#[derive(Clone, Debug, Default)]
pub struct LifeValue {
    /// Per body, its events: `(window tick, body references)`.
    events: Vec<Vec<(u64, f64)>>,
}

impl LifeValue {
    pub fn new(w: &PgWorld, judge: &super::knobs::JudgeKnobs) -> LifeValue {
        let index: FxHashMap<u64, usize> = w.bodies.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
        let mut events: Vec<Vec<(u64, f64)>> = vec![Vec::new(); w.bodies.len()];
        for (i, b) in w.bodies.iter().enumerate() {
            if b.died {
                events[i].push((b.end(), -judge.life(b.stage)));
            }
            if let Some(&m) = b.born_to.as_ref().and_then(|m| index.get(m)) {
                events[m].push((b.start, judge.life(LifeStage::Young)));
            }
        }
        LifeValue { events }
    }
}

impl RewardTerm for LifeValue {
    fn name(&self) -> &'static str {
        "life"
    }
    fn reward(&self, _w: &PgWorld, b: usize, from: u64, to: u64) -> f64 {
        self.events[b].iter().filter(|&&(t, _)| t > from && t <= to).map(|&(_, v)| v).sum()
    }
}

/// **A drive in potential form** (the teacher; SCORE-V2 "Score topic 3"):
/// `weight × (drive at from − drive at to)`.
#[derive(Clone, Copy, Debug)]
pub struct Drive {
    /// The drive's index ([`super::judge::HUNGER`], `WOUND`, `SLEEP`).
    pub k: usize,
    pub weight: f64,
}

impl RewardTerm for Drive {
    fn name(&self) -> &'static str {
        DRIVE_NAMES[self.k]
    }
    fn reward(&self, w: &PgWorld, b: usize, from: u64, to: u64) -> f64 {
        let x = &w.bodies[b];
        self.weight * (x.drive(from, self.k) - x.drive(to, self.k))
    }
}

/// The judge's terms by name: a decision's judge reward is these terms' sum.
pub const JUDGE_TERMS: [&str; 4] = ["own", "kin", "life", "kin_life"];

/// **The teacher's terms for one world's window**: the judge (`own` and `kin` always, as
/// pilot v1; `life` and `kin_life` when a life value is above 0) and each drive whose
/// weight is above 0.
pub fn teacher_terms(w: &PgWorld, roster: &Roster, shares: &KinShares, knobs: &PgKnobs) -> Vec<Box<dyn RewardTerm>> {
    let mut terms: Vec<Box<dyn RewardTerm>> = vec![Box::new(OwnEnergy), Box::new(KinEnergy::new(w, roster, shares))];
    if !knobs.judge.is_off() {
        terms.push(Box::new(LifeValue::new(w, &knobs.judge)));
        terms.push(Box::new(Kin::of(w, roster, shares, LifeValue::new(w, &knobs.judge), "kin_life")));
    }
    for (k, weight) in knobs.teacher.weights().into_iter().enumerate() {
        if weight != 0.0 {
            terms.push(Box::new(Drive { k, weight }));
        }
    }
    terms
}

/// Each term's total over the trained decisions, summed over bodies.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TermTotals(pub Vec<(String, f64)>);

/// **The trajectories of one world's window**: per body, its decisions with each one's
/// reward until the next (every term summed), `dt` in seconds at `seconds_per_tick`.
///
/// A body that died trains every decision, the last one's reward running to its death,
/// and has no bootstrap. A body alive at the window's end trains all but its last decision,
/// which is its bootstrap state. A body with nothing to train is left out.
pub fn trajectories(
    w: &PgWorld,
    world: usize,
    terms: &[&dyn RewardTerm],
    seconds_per_tick: f64,
) -> (Vec<Trajectory>, TermTotals) {
    let mut totals: Vec<(String, f64)> = terms.iter().map(|t| (t.name().to_string(), 0.0)).collect();
    let mut out = Vec::new();
    for (b, body) in w.bodies.iter().enumerate() {
        let d = &body.decisions;
        if d.is_empty() {
            continue;
        }
        let trained = if body.died { d.len() } else { d.len() - 1 };
        if trained == 0 {
            continue;
        }
        let input = |x: &TopDecision| TopInput {
            hidden: x.hidden.clone(),
            held: x.held,
            time: x.time,
            valid: x.valid.clone(),
        };
        let steps = (0..trained)
            .map(|i| {
                let from = d[i].tick;
                let to = if i + 1 < d.len() { d[i + 1].tick } else { body.end().max(from) };
                let mut reward = 0.0;
                for (k, t) in terms.iter().enumerate() {
                    let r = t.reward(w, b, from, to);
                    totals[k].1 += r;
                    reward += r;
                }
                Step {
                    input: input(&d[i]),
                    action: d[i].intent,
                    logp: d[i].logp,
                    dt_s: (to - from) as f64 * seconds_per_tick,
                    reward,
                }
            })
            .collect();
        out.push(Trajectory {
            world,
            body: body.id,
            steps,
            bootstrap: (!body.died).then(|| input(&d[d.len() - 1])),
        });
    }
    (out, TermTotals(totals))
}
