//! **The per-decision reward interface** (PG-PILOT). A window's record per body is its
//! decisions and its energy every tick ([`PgBody::series`]); a [`RewardTerm`] reads a
//! body's reward over any tick span, and a decision's reward is every term's sum over the
//! span until the body's next decision. The learner sees only the sums
//! ([`super::Trajectory`]), so a term joins here without touching it.
//!
//! Today's score is two terms, both outcomes (energy, death, kin), never a behaviour:
//!
//! - [`OwnEnergy`]: the body's own net energy, `(E + E_given)/body_ref` at the span's end
//!   minus at its start, with `E = 0` from its death on: over a whole window this is the ES
//!   score's `(E_end − E_start + E_given)/body_ref` exactly, the loss at death included;
//! - [`KinEnergy`]: `r ×` each relative's own change over the same span (§10.3; the
//!   window's roster, newborns joined at birth), each relative at its own `body_ref`.
//!
//! The curiosity term (`config/training/curiosity.toml`) is not wired: a novelty term would
//! be one more [`RewardTerm`].

use cubarium_voxel_fauna::Founder;
use rustc_hash::FxHashMap;

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

/// `r ×` each relative's own net energy over the span (§10.3).
#[derive(Clone, Debug, Default)]
pub struct KinEnergy {
    /// Per body, its relatives on the record: `(index, share)`.
    relatives: Vec<Vec<(usize, f64)>>,
}

impl KinEnergy {
    /// The relatives of every recorded body, by the window's `roster`, at `shares`. A
    /// relative not recorded (another lineage, or never alive in the window) adds nothing.
    pub fn new(w: &PgWorld, roster: &Roster, shares: &KinShares) -> KinEnergy {
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
        KinEnergy { relatives }
    }
}

impl RewardTerm for KinEnergy {
    fn name(&self) -> &'static str {
        "kin"
    }
    fn reward(&self, w: &PgWorld, b: usize, from: u64, to: u64) -> f64 {
        self.relatives[b]
            .iter()
            .map(|&(i, r)| r * (w.bodies[i].s(to) - w.bodies[i].s(from)))
            .sum()
    }
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
