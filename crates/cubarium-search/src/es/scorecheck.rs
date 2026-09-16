//! **Two falsification checks before any score change**
//! (`design/handoffs/ecology-v1-score-checks-opus-2026-09-16.md`).
//!
//! The per-tick intake diagnostic ([`super::intake`]) showed that generation 9 stands on food
//! on 8.3 % of its ticks against the surviving mobile script's 95.2 %, with its mouth open on
//! every one of those ticks and nothing clamping the bite
//! (`design/7_Research/ecology-v1-intake-2026-09-16.md`). The leading hypothesis is that the
//! trainer's score `t_min + 0.25 · stores` pays nothing for staying on a cell. That is an
//! inference from three measured refusals, not a measurement, so it gets two chances to be
//! wrong before it becomes a design change:
//!
//! - **(a) the controller's response to food.** Take real observations off generation 9's own
//!   trajectory, vary **only** the local food scalar `v[0]` or **only** the ring food sectors,
//!   hold every other scalar fixed, and run the frozen weights forward — once from a zero
//!   hidden state and once from the hidden state the trajectory actually had at that tick.
//!   If the movement head turns toward food or slows on it while residence still fails, the
//!   binding constraint is cadence or recurrent dynamics and the score hypothesis is weakened.
//! - **(b) the current score's dwell gradient.** Run a scripted control family that differs
//!   from the disclosed mobile script in exactly one rule — leave a food cell after `d` ticks
//!   instead of when it falls below `feed_min` — and score each rung with the **current**
//!   `t_min + 0.25 · stores`, exactly as the trainer would. If the current score is already
//!   strongly and monotonically increasing in dwell, the missing-gradient diagnosis is wrong.
//!
//! Neither check trains anything, and **nothing here is wired into the trainer's score**. The
//! auxiliary below is computed and reported so a proposal can be argued from numbers; the
//! trainer still optimises [`super::trainer::score`] and this module never touches it.
//!
//! # The auxiliary's constants are fixed here, before any outcome
//!
//! Astra's proposal is `A = (1/T) Σ_t clip((E_credited,t − E_billed,t)/b_ref, −1, 1)` with the
//! ticks after death scored −1, and `S = t_min + λ·A`. `T`, `b_ref`, the clip and `λ` are
//! [`AUX_HORIZON`], [`b_ref`], [`AUX_CLIP`] and [`AUX_LAMBDA`], and they are compiled into this
//! file so that the commit that defines them precedes the commit that records any ladder run
//! under them. Their reasoning is on each constant.

// Every loop this lint flags walks the five fixed action channels and indexes two or three
// parallel `[f64; CH]` arrays at once. The index is the subject of the loop, and an iterator
// over one of them would only hide which array the others are being read at.
#![allow(clippy::needless_range_loop)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cubarium_core::motor::MotorBill;
use cubarium_core::neural::action::{GRAZE, SCAVENGE, THRUST, TURN};
use cubarium_core::neural::obs::{FOOD_FAR, FOOD_NEAR, OBS_LEN};
use cubarium_core::neural::{Action7, Capability, Gru32, HIDDEN, Policy};
use cubarium_core::{BodyBudget, DT, IntakeTick, World};
use serde::{Deserialize, Serialize};

use super::budget::NamedDriver;
use super::episode::{self, Episode, Limits};
use super::export::PolicyFile;
use super::fixture::{self, Ecology, HORIZON_TICKS};
use super::tensor;
use crate::evaluate::BUILD_ID;

type Boxed = Box<dyn std::error::Error>;

// ---------------------------------------------------------------------------------------
// The auxiliary's constants. Fixed before any outcome; see the module header.
// ---------------------------------------------------------------------------------------

/// `T`: the fixed horizon the auxiliary averages over. The fixture's own episode horizon, so a
/// surviving body's `A` covers exactly its life and a dead one's covers exactly the ticks it
/// did not live. Choosing anything else would make `A` depend on a second, invented clock.
pub const AUX_HORIZON: u64 = HORIZON_TICKS;

/// The symmetric per-tick clip. One tick's margin cannot count for more than one `b_ref`
/// whichever way it goes, so a single very rich bite cannot buy a life of deficit and a single
/// catastrophic tick cannot erase a life of surplus.
pub const AUX_CLIP: f64 = 1.0;

/// What a tick after death contributes. Astra's rule: without it, dying early avoids future
/// bills and a corpse outscores a body that kept paying for itself.
pub const AUX_DEAD_TICK: f64 = -1.0;

/// `λ`: how many ticks of survival the whole auxiliary is worth.
///
/// `A ∈ [−1, 1]`, so `λ · A` moves a score by at most `±λ` ticks and the full range is `2λ`.
/// At `λ = 100` that range is 200 ticks — 10 s of simulated time, 0.56 % of the horizon and
/// 2.4 % of generation 9's current `t_min ≈ 8,400`. **A survival difference of more than 10 s
/// is therefore preserved strictly**, and one of exactly 10 s can at worst be tied by the two
/// extremes of `A`; nothing larger can be inverted. That is the constraint Astra named, and
/// `the_auxiliary_cannot_reorder_a_material_survival_difference` sweeps it. It is also 800×
/// the existing stores tiebreak, whose whole range is 0.25 ticks and which therefore cannot
/// reorder two candidates that differ by a single tick of survival; the auxiliary is meant to
/// be a gradient, not a tiebreak, so it has to be larger than that by a wide margin.
pub const AUX_LAMBDA: f64 = 100.0;

/// `b_ref`: the reference bill one tick of the auxiliary is measured against — the body's own
/// **upkeep price for one tick** at the start of the episode.
///
/// It is a property of the fixture's body and its config, not of any run's outcome: the
/// layouts all found the same mature grazer, so the same number comes back on every one. It
/// makes `A = +1` read as "earned at least one tick of upkeep more than it was billed" and
/// `A = −1` as "fell at least one tick of upkeep short", which is the scale a body's own
/// standing-still cost sets rather than one this file invents.
pub fn b_ref(world: &World) -> f64 {
    let cfg = world.config().clone();
    let o = world
        .state
        .organisms
        .iter()
        .next()
        .map(|(_, o)| o)
        .expect("a layout holds exactly one body");
    MotorBill::of(o, &cfg).upkeep(DT)
}

/// One tick's clipped margin.
pub fn margin(credited: f64, billed: f64, b_ref: f64) -> f64 {
    if !(b_ref.is_finite() && b_ref > 0.0) {
        return 0.0;
    }
    ((credited - billed) / b_ref).clamp(-AUX_CLIP, AUX_CLIP)
}

/// `A` from a life: the clipped margins it lived, plus [`AUX_DEAD_TICK`] for every tick of the
/// horizon it did not.
pub fn auxiliary(summed_margin: f64, lived_ticks: u64, horizon: u64) -> f64 {
    let lived = lived_ticks.min(horizon);
    let dead = horizon.saturating_sub(lived) as f64;
    (summed_margin + dead * AUX_DEAD_TICK) / horizon as f64
}

/// The proposed score for a candidate's set of episodes: the trainer's own `t_min`, plus the
/// auxiliary averaged over the layouts exactly as the stores tiebreak is.
///
/// **Reported only.** Nothing in the trainer calls this.
pub fn proposed_score(t_min: u64, mean_auxiliary: f64) -> f64 {
    t_min as f64 + AUX_LAMBDA * mean_auxiliary
}

// ---------------------------------------------------------------------------------------
// Check (a): the frozen controller's response to food.
// ---------------------------------------------------------------------------------------

/// The action channels this check reads: the two movement channels and the three mouths.
pub const CH: usize = 5;
pub const CHANNEL_NAMES: [&str; CH] = ["thrust", "turn", "graze", "fruit", "scavenge"];
const CHANNEL_INDEX: [usize; CH] = [THRUST, TURN, GRAZE, cubarium_core::neural::action::FRUIT, SCAVENGE];

/// The levels `v[0]` is driven to. `v[0] = P_here / P_max`, so these are "bare", "a quarter
/// stand" … "a full stand under the body", and nothing else in the vector moves with them.
pub const V0_LEVELS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

/// How the ring food sectors are overwritten. `AsRecorded` is the control arm: it is the
/// observation the trajectory actually produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ring {
    AsRecorded,
    /// Every near and far food sector zero: nothing in sensing range.
    Zero,
    /// A full foliage reading in sector 0 of both rings, everything else zero. Sector 0 is the
    /// body-frame forward window (`neural::obs`), so this is "food dead ahead".
    Ahead,
    /// The same in sector 3, the window 180° from forward: "food behind".
    Behind,
}

impl Ring {
    pub fn name(self) -> &'static str {
        match self {
            Ring::AsRecorded => "as-recorded",
            Ring::Zero => "none",
            Ring::Ahead => "ahead",
            Ring::Behind => "behind",
        }
    }
}

/// Which hidden state the forward pass starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Start {
    /// Zeroed, as at birth: what the weights alone say about this observation.
    Reset,
    /// The hidden state the real trajectory held at this tick: what the policy would actually
    /// have done had the world handed it this observation instead.
    Carried,
}

impl Start {
    pub fn name(self) -> &'static str {
        match self {
            Start::Reset => "reset",
            Start::Carried => "carried",
        }
    }
}

/// One observation with the check's single edit applied. Everything not named is copied.
pub fn variant(base: &[f64; OBS_LEN], v0: Option<f64>, ring: Ring) -> [f64; OBS_LEN] {
    let mut x = *base;
    if let Some(v) = v0 {
        x[0] = v;
    }
    match ring {
        Ring::AsRecorded => {}
        Ring::Zero | Ring::Ahead | Ring::Behind => {
            for v in &mut x[FOOD_NEAR..FOOD_NEAR + 36] {
                *v = 0.0;
            }
            let sector = match ring {
                Ring::Ahead => Some(0usize),
                Ring::Behind => Some(3usize),
                _ => None,
            };
            if let Some(k) = sector {
                // channel 0 of the sector triple is foliage.
                x[FOOD_NEAR + 3 * k] = 1.0;
                x[FOOD_FAR + 3 * k] = 1.0;
            }
        }
    }
    x
}

/// One forward pass through the **core's own** GRU and action adapter: no copy of either lives
/// here. Returns the five read channels of the action in force.
pub fn respond(
    weights: &Gru32,
    cap: &Capability,
    x: &[f64; OBS_LEN],
    hidden: &[f64; HIDDEN],
) -> [f64; CH] {
    let mut h = *hidden;
    let head = weights.forward(x, &mut h);
    let held = Action7::squash(&head, cap).0;
    let mut out = [0.0; CH];
    for (i, slot) in CHANNEL_INDEX.iter().enumerate() {
        out[i] = held[*slot];
    }
    out
}

/// One sampled tick of a real trajectory: the observation the world built, the hidden state the
/// animal held going into the next controller update, and whether the body was on food.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub tick: u64,
    pub on_food: bool,
    pub obs: Vec<f64>,
    pub hidden: Vec<f64>,
}

/// Running moments of the five channels over a trajectory's own actions: the **natural**
/// variation an effect size is measured against.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Moments {
    pub n: u64,
    pub sum: [f64; CH],
    pub sumsq: [f64; CH],
    pub min: [f64; CH],
    pub max: [f64; CH],
}

impl Default for Moments {
    fn default() -> Self {
        Moments {
            n: 0,
            sum: [0.0; CH],
            sumsq: [0.0; CH],
            min: [f64::INFINITY; CH],
            max: [f64::NEG_INFINITY; CH],
        }
    }
}

impl Moments {
    pub fn push(&mut self, v: &[f64; CH]) {
        self.n += 1;
        for i in 0..CH {
            self.sum[i] += v[i];
            self.sumsq[i] += v[i] * v[i];
            self.min[i] = self.min[i].min(v[i]);
            self.max[i] = self.max[i].max(v[i]);
        }
    }

    pub fn merge(&mut self, other: &Moments) {
        self.n += other.n;
        for i in 0..CH {
            self.sum[i] += other.sum[i];
            self.sumsq[i] += other.sumsq[i];
            self.min[i] = self.min[i].min(other.min[i]);
            self.max[i] = self.max[i].max(other.max[i]);
        }
    }

    pub fn mean(&self) -> [f64; CH] {
        let mut m = [0.0; CH];
        if self.n > 0 {
            for i in 0..CH {
                m[i] = self.sum[i] / self.n as f64;
            }
        }
        m
    }

    /// Population standard deviation, floored at zero against float noise.
    pub fn sd(&self) -> [f64; CH] {
        let mut s = [0.0; CH];
        if self.n > 0 {
            let n = self.n as f64;
            for i in 0..CH {
                let mean = self.sum[i] / n;
                s[i] = (self.sumsq[i] / n - mean * mean).max(0.0).sqrt();
            }
        }
        s
    }
}

/// What one layout's trajectory produced for the sweep.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recording {
    pub layout: String,
    pub episode: Episode,
    /// Moments of the action the policy actually held, over the update ticks it lived.
    pub natural: Moments,
    pub on_food_ticks: u64,
    pub update_ticks: u64,
    /// The largest absolute disagreement, over every checked update tick and all five
    /// channels, between the action this module reconstructs from the recorded pair and the
    /// action the world went on to hold. It is **not zero**, and the reason is stated once
    /// here: the world's stages run field reactions, light, water and the pair pass *before*
    /// it observes and decides (`world/step.rs`, stages 2–5), so an observation sampled at the
    /// close of tick `t` is one tick of field dynamics older than the one the controller reads
    /// at `t + 1`. The baseline is still a real observation the world's own sampler built on
    /// this trajectory; this column says how far it is from the exact input, so every effect
    /// the sweep reports can be read against it.
    pub max_reconstruction_residual: f64,
    pub reconstruction_checks: u64,
    pub samples: Vec<Sample>,
}

/// Record a policy's trajectory on one layout and keep a bounded, evenly spaced sample of the
/// ticks the controller was about to update on.
///
/// The observation is the **world's own** (`World::neural_observation`) and the hidden state is
/// the animal's own; [`Recording::max_reconstruction_residual`] measures how close the pair is
/// to the exact controller input.
pub fn record(
    layout: &fixture::Layout,
    policy: &Policy,
    cap: &Capability,
    horizon: u64,
    limits: Limits<'_>,
    per_stratum: usize,
    job: &str,
) -> Result<Recording, Boxed> {
    let cfg = layout.config();
    let p_max = cfg.producer.max;
    let feed = cfg.drives.feed_min;
    let held_floor = feed / p_max;

    let all: Mutex<Vec<Sample>> = Mutex::new(Vec::new());
    let natural: Mutex<Moments> = Mutex::new(Moments::default());
    // (on-food ticks, update ticks, residual, checks, pending pair)
    type Pending = Option<([f64; OBS_LEN], [f64; HIDDEN])>;
    let acc: Mutex<(u64, u64, f64, u64, Pending)> = Mutex::new((0, 0, 0.0, 0, None));

    let watch = |w: &mut World, tick: u64| {
        let Some((id, _)) = w.state.organisms.iter().next() else { return };
        let Some(animal) = w.state.neural.get(id) else { return };
        if animal.updates_on(tick) {
            let held = animal.held;
            let mut v = [0.0; CH];
            for (i, slot) in CHANNEL_INDEX.iter().enumerate() {
                v[i] = held[*slot];
            }
            natural.lock().expect("natural").push(&v);
            let mut a = acc.lock().expect("acc");
            a.1 += 1;
            if let Some((x, h)) = a.4.take() {
                let got = respond(&policy.weights, cap, &x, &h);
                for (i, slot) in CHANNEL_INDEX.iter().enumerate() {
                    a.2 = a.2.max((got[i] - held[*slot]).abs());
                }
                a.3 += 1;
            }
        }
        // The state the *next* update will read, kept only when there is one.
        if !animal.updates_on(tick + 1) {
            return;
        }
        let mut hidden = [0.0f64; HIDDEN];
        hidden.copy_from_slice(&animal.hidden);
        let Some(obs) = w.neural_observation(id) else { return };
        let obs = obs.0;
        let on_food = obs[0] >= held_floor || obs[1] >= held_floor || obs[2] >= held_floor;
        let mut a = acc.lock().expect("acc");
        if on_food {
            a.0 += 1;
        }
        a.4 = Some((obs, hidden));
        drop(a);
        all.lock().expect("all").push(Sample {
            tick: tick + 1,
            on_food,
            obs: obs.to_vec(),
            hidden: hidden.to_vec(),
        });
    };

    let driver = episode::Driver::Policy(Box::new(policy.clone()));
    let episode =
        episode::run_with_fault(layout, &driver, horizon, limits, job, Some(&watch))?;

    let all = all.into_inner().expect("all");
    let (on_food_ticks, update_ticks, residual, checks, _) = acc.into_inner().expect("acc");
    let samples = stratified(&all, per_stratum);
    Ok(Recording {
        layout: layout.name.clone(),
        episode,
        natural: natural.into_inner().expect("natural"),
        on_food_ticks,
        update_ticks,
        max_reconstruction_residual: residual,
        reconstruction_checks: checks,
        samples,
    })
}

/// Up to `per_stratum` on-food and `per_stratum` off-food samples, evenly spaced through the
/// life in each stratum. Deterministic: no draw, no clock.
pub fn stratified(all: &[Sample], per_stratum: usize) -> Vec<Sample> {
    let mut out = Vec::new();
    for want_food in [true, false] {
        let pool: Vec<&Sample> = all.iter().filter(|s| s.on_food == want_food).collect();
        if pool.is_empty() || per_stratum == 0 {
            continue;
        }
        let take = per_stratum.min(pool.len());
        for i in 0..take {
            // Evenly spaced including both ends when `take > 1`.
            let idx = if take == 1 {
                pool.len() / 2
            } else {
                (i * (pool.len() - 1)) / (take - 1)
            };
            out.push(pool[idx].clone());
        }
    }
    out.sort_by_key(|s| s.tick);
    out
}

/// One `(driver, layout, stratum, hidden start)` cell of the sweep.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SweepRow {
    pub driver: String,
    pub layout: String,
    /// `"on-food"`, `"off-food"` or `"all"`.
    pub stratum: String,
    pub start: String,
    pub samples: usize,
    /// Mean of each channel at each `v[0]` level, `[level][channel]`.
    pub v0_mean: Vec<[f64; CH]>,
    /// Mean of each channel under each ring arm, in [`RING_ARMS`] order.
    pub ring_mean: Vec<[f64; CH]>,
    /// Mean over samples of `channel(v0 = 1) − channel(v0 = 0)`, per sample and then averaged:
    /// the signed within-tick response to the food scalar.
    pub v0_delta: [f64; CH],
    /// Mean over samples of `max_level − min_level` within the same tick: the whole span the
    /// food scalar can move this channel without anything else changing.
    pub v0_span: [f64; CH],
    /// The same span for the ring arms: `max_arm − min_arm` within a tick, over the three
    /// overwritten arms.
    pub ring_span: [f64; CH],
    /// Mean and standard deviation of the action the trajectory actually held.
    pub natural_mean: [f64; CH],
    pub natural_sd: [f64; CH],
    /// `|v0_delta| / natural_sd`, and `v0_span / natural_sd`: the effect sizes.
    pub v0_delta_over_sd: [f64; CH],
    pub v0_span_over_sd: [f64; CH],
    pub ring_span_over_sd: [f64; CH],
}

/// The ring arms, in the order [`SweepRow::ring_mean`] reports them.
pub const RING_ARMS: [Ring; 4] = [Ring::AsRecorded, Ring::Zero, Ring::Ahead, Ring::Behind];

/// Reduce one recording to the sweep's rows: one per stratum per hidden start.
pub fn sweep(
    driver: &str,
    layout: &str,
    samples: &[Sample],
    natural: &Moments,
    weights: &Gru32,
    cap: &Capability,
) -> Vec<SweepRow> {
    let mut rows = Vec::new();
    let natural_mean = natural.mean();
    let natural_sd = natural.sd();
    let zero = [0.0f64; HIDDEN];

    for (stratum, want) in [("on-food", Some(true)), ("off-food", Some(false)), ("all", None)] {
        let pool: Vec<&Sample> =
            samples.iter().filter(|s| want.is_none_or(|w| s.on_food == w)).collect();
        if pool.is_empty() {
            continue;
        }
        for start in [Start::Reset, Start::Carried] {
            let mut v0_mean = vec![[0.0f64; CH]; V0_LEVELS.len()];
            let mut ring_mean = vec![[0.0f64; CH]; RING_ARMS.len()];
            let mut v0_delta = [0.0f64; CH];
            let mut v0_span = [0.0f64; CH];
            let mut ring_span = [0.0f64; CH];

            for s in &pool {
                let x0: [f64; OBS_LEN] =
                    s.obs.clone().try_into().expect("a recorded observation is 70 wide");
                let mut h = zero;
                if start == Start::Carried {
                    h.copy_from_slice(&s.hidden);
                }
                let mut per_level = Vec::with_capacity(V0_LEVELS.len());
                for (li, level) in V0_LEVELS.iter().enumerate() {
                    let out = respond(weights, cap, &variant(&x0, Some(*level), Ring::AsRecorded), &h);
                    for c in 0..CH {
                        v0_mean[li][c] += out[c];
                    }
                    per_level.push(out);
                }
                let mut per_arm = Vec::with_capacity(RING_ARMS.len());
                for (ai, arm) in RING_ARMS.iter().enumerate() {
                    let out = respond(weights, cap, &variant(&x0, None, *arm), &h);
                    for c in 0..CH {
                        ring_mean[ai][c] += out[c];
                    }
                    per_arm.push(out);
                }
                for c in 0..CH {
                    v0_delta[c] += per_level[V0_LEVELS.len() - 1][c] - per_level[0][c];
                    let hi = per_level.iter().map(|p| p[c]).fold(f64::NEG_INFINITY, f64::max);
                    let lo = per_level.iter().map(|p| p[c]).fold(f64::INFINITY, f64::min);
                    v0_span[c] += hi - lo;
                    // The ring span excludes the as-recorded control arm: it compares the three
                    // *constructed* ring states with one another.
                    let hi = per_arm[1..].iter().map(|p| p[c]).fold(f64::NEG_INFINITY, f64::max);
                    let lo = per_arm[1..].iter().map(|p| p[c]).fold(f64::INFINITY, f64::min);
                    ring_span[c] += hi - lo;
                }
            }

            let n = pool.len() as f64;
            for row in &mut v0_mean {
                for c in 0..CH {
                    row[c] /= n;
                }
            }
            for row in &mut ring_mean {
                for c in 0..CH {
                    row[c] /= n;
                }
            }
            let mut d_sd = [0.0f64; CH];
            let mut s_sd = [0.0f64; CH];
            let mut r_sd = [0.0f64; CH];
            for c in 0..CH {
                v0_delta[c] /= n;
                v0_span[c] /= n;
                ring_span[c] /= n;
                let sd = natural_sd[c];
                let ratio = |x: f64| if sd > 0.0 { x / sd } else { f64::INFINITY };
                d_sd[c] = if sd > 0.0 { v0_delta[c].abs() / sd } else { f64::INFINITY };
                s_sd[c] = ratio(v0_span[c]);
                r_sd[c] = ratio(ring_span[c]);
            }

            rows.push(SweepRow {
                driver: driver.to_string(),
                layout: layout.to_string(),
                stratum: stratum.to_string(),
                start: start.name().to_string(),
                samples: pool.len(),
                v0_mean,
                ring_mean,
                v0_delta,
                v0_span,
                ring_span,
                natural_mean,
                natural_sd,
                v0_delta_over_sd: d_sd,
                v0_span_over_sd: s_sd,
                ring_span_over_sd: r_sd,
            });
        }
    }
    rows
}

/// The capability mask the world would build for this layout's body, read from the body and
/// the config rather than assumed.
pub fn capability_of(layout: &fixture::Layout) -> Result<Capability, Boxed> {
    let (world, id) = layout.build()?;
    let cfg = world.config().clone();
    let o = world.state.organisms.get(id).ok_or_else(|| Boxed::from("no body"))?;
    Ok(Capability::ordinary(
        o.phenotype.cap_foliage,
        o.phenotype.cap_detrital,
        cfg.mechanisms.grazing,
        cfg.mechanisms.scavenging,
    ))
}

// ---------------------------------------------------------------------------------------
// Check (b): the dwell ladder.
// ---------------------------------------------------------------------------------------

/// One rung's episode on one layout, with everything both scores need.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LadderEpisode {
    pub rung: String,
    pub layout: String,
    pub episode: Episode,
    /// The world's own ledger for the life.
    pub budget: BodyBudget,
    /// `b_ref`, recorded so the auxiliary can be recomputed from the columns.
    pub b_ref: f64,
    /// `Σ_t clip(...)` over the ticks the body lived.
    pub summed_margin: f64,
    /// The auxiliary itself, ticks after death included at [`AUX_DEAD_TICK`].
    pub auxiliary: f64,
    /// Usable energy credited by food over the life (e), and what the motor bill asked for.
    pub credited: f64,
    pub billed: f64,
    /// Ticks the body stood on a cell holding any stock at or above `drives.feed_min`, from
    /// the world's own per-tick trace — H's definition, unchanged.
    pub on_food_ticks: u64,
    pub traced_ticks: u64,
    /// Material that actually left the fields through this body's mouths (m).
    pub served_total: f64,
}

/// Run one rung on one layout with the ledger and the per-tick trace on, and reduce it.
pub fn measure_rung(
    layout: &fixture::Layout,
    named: &NamedDriver,
    horizon: u64,
    limits: Limits<'_>,
    e_r: f64,
    eta_ox: f64,
) -> Result<LadderEpisode, Boxed> {
    let reference: Mutex<f64> = Mutex::new(0.0);
    let prepare = |w: &mut World| {
        let id = w.state.organisms.iter().next().map(|(id, _)| id);
        *reference.lock().expect("b_ref") = b_ref(w);
        w.record_body_budgets(true);
        w.trace_intake(id);
    };

    // Cumulative ledger readings, differenced tick by tick.
    let acc: Mutex<(f64, f64, f64, u64, u64, Option<BodyBudget>)> =
        Mutex::new((0.0, 0.0, 0.0, 0, 0, None));
    let watch = |w: &mut World, _tick: u64| {
        let (rows, dropped) = w.drain_intake_trace();
        assert_eq!(dropped, 0, "a per-tick drain can never overflow the trace cap");
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
        let mut a = acc.lock().expect("acc");
        for row in &rows {
            a.4 += 1;
            if IntakeTick::on_food(row) {
                a.3 += 1;
            }
        }
        let Some(b) = record else { return };
        let credited = b.battery_credit_total() + eta_ox * e_r * b.reserve_credit_total();
        let billed = b.bill_total;
        let b_ref = *reference.lock().expect("b_ref");
        a.0 += margin(credited - a.1, billed - a.2, b_ref);
        a.1 = credited;
        a.2 = billed;
        a.5 = Some(b);
    };

    let job = format!("ladder/{}/{}", named.name, layout.name);
    let episode = episode::run_prepared(
        layout,
        &named.driver,
        horizon,
        limits,
        &job,
        Some(&prepare),
        Some(&watch),
    )?;

    let (summed_margin, credited, billed, on_food_ticks, traced_ticks, budget) =
        acc.into_inner().expect("acc");
    let budget = budget.ok_or_else(|| Boxed::from(format!("{job}: no ledger record")))?;
    Ok(LadderEpisode {
        rung: named.name.clone(),
        layout: layout.name.clone(),
        auxiliary: auxiliary(summed_margin, episode.ticks, AUX_HORIZON),
        b_ref: *reference.lock().expect("b_ref"),
        summed_margin,
        credited,
        billed,
        on_food_ticks,
        traced_ticks,
        served_total: budget.served_total(),
        budget,
        episode,
    })
}

/// A rung reduced over a set of layouts, under both scores.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RungScore {
    pub layouts: usize,
    pub survivors: usize,
    pub t_min: u64,
    pub t_mean: f64,
    pub mean_stores: f64,
    /// The trainer's own score, computed by [`super::trainer::score`] on these episodes.
    pub current_score: f64,
    pub mean_auxiliary: f64,
    /// [`proposed_score`]. Reported only; the trainer does not use it.
    pub proposed_score: f64,
    pub mean_on_food_fraction: f64,
    pub mean_served: f64,
}

pub fn reduce(rows: &[&LadderEpisode]) -> RungScore {
    let episodes: Vec<Episode> = rows.iter().map(|r| r.episode.clone()).collect();
    let n = rows.len() as f64;
    let t_min = episodes.iter().map(|e| e.ticks).min().expect("nonempty");
    let mean_aux = rows.iter().map(|r| r.auxiliary).sum::<f64>() / n;
    RungScore {
        layouts: rows.len(),
        survivors: episodes.iter().filter(|e| e.alive).count(),
        t_min,
        t_mean: episodes.iter().map(|e| e.ticks as f64).sum::<f64>() / n,
        mean_stores: episodes.iter().map(Episode::normalized_stores).sum::<f64>() / n,
        current_score: super::trainer::score(&episodes),
        mean_auxiliary: mean_aux,
        proposed_score: proposed_score(t_min, mean_aux),
        mean_on_food_fraction: rows
            .iter()
            .map(|r| {
                if r.traced_ticks == 0 {
                    0.0
                } else {
                    r.on_food_ticks as f64 / r.traced_ticks as f64
                }
            })
            .sum::<f64>()
            / n,
        mean_served: rows.iter().map(|r| r.served_total).sum::<f64>() / n,
    }
}

/// One completed stay on a cell, measured from the body's own per-tick cell and travel.
///
/// A route-following control stops dead the tick after it finishes a tick on its goal cell
/// (`effort = 0` is a genuine request for stillness, pinned by
/// `a_still_body_with_no_intake_eats_nothing_and_keeps_its_stores_falling`), so a maximal run
/// of zero-travel ticks is exactly a stay, and the moving tick before it is the arrival.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stay {
    pub cell: u16,
    /// The tick the body first finished on this cell.
    pub arrival_tick: u64,
    /// Ticks finished on this cell from the arrival tick through the end of the still run —
    /// every tick of the stay on which the cell was fed from, the arrival included.
    pub ticks_on_cell: u64,
    /// Ticks of the stay on which the body did not move at all.
    pub still_ticks: u64,
    /// Whether the run ended inside the horizon rather than being cut off by it.
    pub completed: bool,
}

/// Every stay a driver made, from the world's own post-step cell and resolved travel.
pub fn stays(
    layout: &fixture::Layout,
    driver: &episode::Driver,
    horizon: u64,
) -> Result<Vec<Stay>, Boxed> {
    let cancel = AtomicBool::new(false);
    let track: Mutex<Vec<(u16, f64)>> = Mutex::new(Vec::new());
    let watch = |w: &mut World, _tick: u64| {
        let Some((id, o)) = w.state.organisms.iter().next() else { return };
        let here = cubarium_surface::cell_of(cubarium_surface::Topology::Cube, cubarium_surface::Scale::ONE, &o.pos).0;
        let travelled: f64 = w
            .moved_segments(id)
            .iter()
            .map(cubarium_surface::PathSegment::length)
            .sum();
        track.lock().expect("track").push((here, travelled));
    };
    episode::run_with_fault(layout, driver, horizon, Limits::new(&cancel), "stays", Some(&watch))?;
    let track = track.into_inner().expect("track");

    let mut out = Vec::new();
    let mut i = 1usize; // a stay needs an arrival tick before it
    while i < track.len() {
        if track[i].1 != 0.0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < track.len() && track[i].1 == 0.0 && track[i].0 == track[start].0 {
            i += 1;
        }
        let still = (i - start) as u64;
        // The arrival: the moving tick immediately before, on the same cell.
        if track[start - 1].0 != track[start].0 || track[start - 1].1 == 0.0 {
            continue;
        }
        out.push(Stay {
            cell: track[start].0,
            arrival_tick: (start - 1) as u64,
            ticks_on_cell: still + 1,
            still_ticks: still,
            completed: i < track.len(),
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// The two experiments.
// ---------------------------------------------------------------------------------------

/// Everything check (a) produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SweepReport {
    pub build: String,
    pub config: String,
    pub config_hash: String,
    pub policy: String,
    pub horizon_ticks: u64,
    pub per_stratum: usize,
    pub v0_levels: Vec<f64>,
    pub ring_arms: Vec<String>,
    pub channel_names: Vec<String>,
    pub drivers: Vec<String>,
    pub layouts: Vec<String>,
    pub wall_seconds: f64,
    /// One line per driver and layout: the trajectory the sweep was sampled from.
    pub trajectories: Vec<TrajectoryLine>,
    /// Per driver, per layout, per stratum, per hidden start; plus a `pooled` layout row that
    /// merges every layout's samples for that driver.
    pub rows: Vec<SweepRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrajectoryLine {
    pub driver: String,
    pub layout: String,
    pub ticks: u64,
    pub alive: bool,
    pub update_ticks: u64,
    pub on_food_update_ticks: u64,
    pub samples: usize,
    pub on_food_samples: usize,
    pub max_reconstruction_residual: f64,
    pub reconstruction_checks: u64,
    pub natural_mean: [f64; CH],
    pub natural_sd: [f64; CH],
}

/// Check (a): record generation 9 (and the untrained centre, as a contrast) on the twelve
/// layouts, then sweep the frozen weights over the recorded observations.
#[allow(clippy::too_many_arguments)]
pub fn run_sweep(
    policy_file: PathBuf,
    config: PathBuf,
    horizon: u64,
    initial_seed: u64,
    workers: usize,
    wall_seconds: u64,
    per_stratum: usize,
    out: PathBuf,
) -> Result<(), Boxed> {
    let eco = Ecology::load(&config)?;
    let file: PolicyFile = serde_json::from_str(&fs::read_to_string(&policy_file)?)?;
    file.check_ecology(&eco)?;
    let trained = file.policy()?;
    let initial = tensor::policy(&tensor::initial_center(initial_seed))?;
    let named: Vec<(String, Policy)> = vec![
        (format!("generation-{}", file.generation), trained),
        (format!("initial-center-{initial_seed}"), initial),
    ];
    let mut layouts = fixture::training_layouts_on(&eco);
    layouts.extend(fixture::holdout_layouts_on(&eco));

    println!("# check (a): the frozen controller's response to food, on {} ({})", eco.label, eco.hex());
    println!("# build {BUILD_ID}, {horizon} ticks, {} drivers x {} layouts", named.len(), layouts.len());
    println!("# v[0] levels {V0_LEVELS:?}, ring arms as-recorded/none/ahead/behind, hidden reset and carried");

    let jobs: Vec<(usize, usize)> =
        (0..named.len()).flat_map(|d| (0..layouts.len()).map(move |l| (d, l))).collect();
    let cursor = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Recording)>> = Mutex::new(Vec::new());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let limits = Limits::until(&cancel, started + Duration::from_secs(wall_seconds));

    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let next = cursor.fetch_add(1, Ordering::Relaxed);
                    let Some(&(d, l)) = jobs.get(next) else { return };
                    let (name, policy) = &named[d];
                    let layout = &layouts[l];
                    let cap = match capability_of(layout) {
                        Ok(c) => c,
                        Err(e) => {
                            failures.lock().expect("f").push(e.to_string());
                            continue;
                        }
                    };
                    let job = format!("sweep/{name}/{}", layout.name);
                    match record(layout, policy, &cap, horizon, limits, per_stratum, &job) {
                        Ok(rec) => done.lock().expect("done").push((d, rec)),
                        Err(e) => failures.lock().expect("f").push(e.to_string()),
                    }
                }
            });
        }
    });

    let failures = failures.into_inner().expect("f");
    if !failures.is_empty() {
        return Err(Boxed::from(format!("{} episode(s) failed: {}", failures.len(), failures.join("; "))));
    }
    let mut done = done.into_inner().expect("done");
    done.sort_by(|a, b| (a.0, &a.1.layout).cmp(&(b.0, &b.1.layout)));

    let cap = capability_of(&layouts[0])?;
    let mut rows = Vec::new();
    let mut trajectories = Vec::new();
    for (d, (name, policy)) in named.iter().enumerate() {
        let mine: Vec<&Recording> = done.iter().filter(|(i, _)| *i == d).map(|(_, r)| r).collect();
        let mut pooled_samples: Vec<Sample> = Vec::new();
        let mut pooled_natural = Moments::default();
        for rec in &mine {
            trajectories.push(TrajectoryLine {
                driver: name.clone(),
                layout: rec.layout.clone(),
                ticks: rec.episode.ticks,
                alive: rec.episode.alive,
                update_ticks: rec.update_ticks,
                on_food_update_ticks: rec.on_food_ticks,
                samples: rec.samples.len(),
                on_food_samples: rec.samples.iter().filter(|s| s.on_food).count(),
                max_reconstruction_residual: rec.max_reconstruction_residual,
                reconstruction_checks: rec.reconstruction_checks,
                natural_mean: rec.natural.mean(),
                natural_sd: rec.natural.sd(),
            });
            rows.extend(sweep(name, &rec.layout, &rec.samples, &rec.natural, &policy.weights, &cap));
            pooled_samples.extend(rec.samples.iter().cloned());
            pooled_natural.merge(&rec.natural);
        }
        rows.extend(sweep(name, "pooled", &pooled_samples, &pooled_natural, &policy.weights, &cap));
    }

    let report = SweepReport {
        build: BUILD_ID.to_string(),
        config: config.display().to_string(),
        config_hash: eco.hex(),
        policy: policy_file.display().to_string(),
        horizon_ticks: horizon,
        per_stratum,
        v0_levels: V0_LEVELS.to_vec(),
        ring_arms: RING_ARMS.iter().map(|r| r.name().to_string()).collect(),
        channel_names: CHANNEL_NAMES.iter().map(|s| s.to_string()).collect(),
        drivers: named.iter().map(|(n, _)| n.clone()).collect(),
        layouts: layouts.iter().map(|l| l.name.clone()).collect(),
        wall_seconds: started.elapsed().as_secs_f64(),
        trajectories,
        rows,
    };
    write_json(&out, &report)?;
    print_sweep(&report);
    Ok(())
}

/// Everything check (b) produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LadderReport {
    pub build: String,
    pub config: String,
    pub config_hash: String,
    pub horizon_ticks: u64,
    pub aux_horizon: u64,
    pub aux_lambda: f64,
    pub aux_clip: f64,
    pub aux_dead_tick: f64,
    pub b_ref: f64,
    pub rungs: Vec<String>,
    pub training_layouts: Vec<String>,
    pub all_layouts: Vec<String>,
    pub wall_seconds: f64,
    pub episodes: Vec<LadderEpisode>,
    /// `(rung, over training layouts, over all twelve)`.
    pub scores: Vec<(String, RungScore, RungScore)>,
}

/// Check (b): the dwell ladder under the **current** score, with the proposed auxiliary
/// computed alongside from the same episodes.
#[allow(clippy::too_many_arguments)]
pub fn run_ladder(
    policy_file: Option<PathBuf>,
    config: PathBuf,
    horizon: u64,
    dwells: &[u32],
    workers: usize,
    wall_seconds: u64,
    out: PathBuf,
) -> Result<(), Boxed> {
    let eco = Ecology::load(&config)?;
    let e_r = eco.base.organism.reserve_energy_density;
    let eta_ox = eco.base.organism.oxidation_efficiency;
    let training = fixture::training_layouts_on(&eco);
    let mut layouts = training.clone();
    layouts.extend(fixture::holdout_layouts_on(&eco));

    let mut rungs: Vec<NamedDriver> = dwells
        .iter()
        .map(|d| NamedDriver {
            name: episode::Control::Dwell(*d).name(),
            driver: episode::Driver::Control(episode::Control::Dwell(*d)),
        })
        .collect();
    // The top rung of the ladder is the disclosed control itself: "stay until the cell falls
    // below `feed_min`" is exactly `Control::MobileScript`, so the ladder ends on the driver H
    // already measured rather than on a reimplementation of it.
    for c in [
        episode::Control::MobileScript,
        episode::Control::StationaryGrazing,
        episode::Control::NoIntake,
    ] {
        rungs.push(NamedDriver { name: c.name(), driver: episode::Driver::Control(c) });
    }
    if let Some(path) = &policy_file {
        let file: PolicyFile = serde_json::from_str(&fs::read_to_string(path)?)?;
        file.check_ecology(&eco)?;
        rungs.push(NamedDriver {
            name: format!("generation-{}", file.generation),
            driver: episode::Driver::Policy(Box::new(file.policy()?)),
        });
    }

    println!("# check (b): the dwell ladder under t_min + 0.25*stores, on {} ({})", eco.label, eco.hex());
    println!("# build {BUILD_ID}, {horizon} ticks, {} rungs x {} layouts", rungs.len(), layouts.len());
    println!("# the auxiliary is reported alongside: T={AUX_HORIZON}, lambda={AUX_LAMBDA}, clip={AUX_CLIP}, dead tick {AUX_DEAD_TICK}");

    let jobs: Vec<(usize, usize)> =
        (0..rungs.len()).flat_map(|r| (0..layouts.len()).map(move |l| (r, l))).collect();
    let cursor = AtomicUsize::new(0);
    let out_rows: Mutex<Vec<LadderEpisode>> = Mutex::new(Vec::new());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let limits = Limits::until(&cancel, started + Duration::from_secs(wall_seconds));

    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let next = cursor.fetch_add(1, Ordering::Relaxed);
                    let Some(&(r, l)) = jobs.get(next) else { return };
                    match measure_rung(&layouts[l], &rungs[r], horizon, limits, e_r, eta_ox) {
                        Ok(row) => out_rows.lock().expect("rows").push(row),
                        Err(e) => failures.lock().expect("f").push(e.to_string()),
                    }
                }
            });
        }
    });

    let failures = failures.into_inner().expect("f");
    if !failures.is_empty() {
        return Err(Boxed::from(format!("{} episode(s) failed: {}", failures.len(), failures.join("; "))));
    }
    let mut episodes = out_rows.into_inner().expect("rows");
    episodes.sort_by(|a, b| (&a.rung, &a.layout).cmp(&(&b.rung, &b.layout)));

    let training_names: Vec<String> = training.iter().map(|l| l.name.clone()).collect();
    let mut scores = Vec::new();
    for rung in &rungs {
        let mine: Vec<&LadderEpisode> = episodes.iter().filter(|e| e.rung == rung.name).collect();
        if mine.is_empty() {
            continue;
        }
        let train: Vec<&LadderEpisode> =
            mine.iter().filter(|e| training_names.contains(&e.layout)).copied().collect();
        scores.push((rung.name.clone(), reduce(&train), reduce(&mine)));
    }

    let b_ref = episodes.first().map_or(0.0, |e| e.b_ref);
    let report = LadderReport {
        build: BUILD_ID.to_string(),
        config: config.display().to_string(),
        config_hash: eco.hex(),
        horizon_ticks: horizon,
        aux_horizon: AUX_HORIZON,
        aux_lambda: AUX_LAMBDA,
        aux_clip: AUX_CLIP,
        aux_dead_tick: AUX_DEAD_TICK,
        b_ref,
        rungs: rungs.iter().map(|r| r.name.clone()).collect(),
        training_layouts: training_names,
        all_layouts: layouts.iter().map(|l| l.name.clone()).collect(),
        wall_seconds: started.elapsed().as_secs_f64(),
        episodes,
        scores,
    };
    write_json(&out, &report)?;
    print_ladder(&report);
    Ok(())
}

fn write_json<T: Serialize>(out: &Path, value: &T) -> Result<(), Boxed> {
    if let Some(dir) = out.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut f = fs::File::create(out)?;
    f.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    f.write_all(b"\n")?;
    println!("# wrote {}", out.display());
    Ok(())
}

fn print_sweep(r: &SweepReport) {
    println!();
    println!("driver                layout  stratum   start     n   channel   v0=0    v0=1    span   span/sd   natural sd");
    for row in r.rows.iter().filter(|row| row.layout == "pooled") {
        for c in 0..CH {
            println!(
                "{:<20}  {:<6}  {:<8}  {:<7} {:>4}  {:<8} {:>7.4} {:>7.4} {:>7.4} {:>8.3} {:>11.5}",
                row.driver,
                row.layout,
                row.stratum,
                row.start,
                row.samples,
                CHANNEL_NAMES[c],
                row.v0_mean[0][c],
                row.v0_mean[V0_LEVELS.len() - 1][c],
                row.v0_span[c],
                row.v0_span_over_sd[c],
                row.natural_sd[c],
            );
        }
    }
    println!();
    println!("ring arms (pooled, all strata):");
    for row in r.rows.iter().filter(|row| row.layout == "pooled" && row.stratum == "all") {
        for c in 0..CH {
            print!("{:<20} {:<7} {:<8}", row.driver, row.start, CHANNEL_NAMES[c]);
            for (a, arm) in RING_ARMS.iter().enumerate() {
                print!("  {}={:.4}", arm.name(), row.ring_mean[a][c]);
            }
            println!("  span/sd={:.3}", row.ring_span_over_sd[c]);
        }
    }
    println!();
    for t in &r.trajectories {
        println!(
            "# {:<20} {:<12} ticks {:>6} alive {:<5} updates {:>6} on-food {:>6} samples {:>3} residual {:.3e}",
            t.driver, t.layout, t.ticks, t.alive, t.update_ticks, t.on_food_update_ticks,
            t.samples, t.max_reconstruction_residual
        );
    }
    println!("# wall {:.1} s", r.wall_seconds);
}

fn print_ladder(r: &LadderReport) {
    println!();
    println!("b_ref = {:.6e} e/tick", r.b_ref);
    println!("rung                 set        t_min  t_mean   stores  CURRENT   on-food  served      A    PROPOSED");
    for (name, train, all) in &r.scores {
        for (label, s) in [("train-4", train), ("all-12", all)] {
            println!(
                "{:<20} {:<9} {:>6} {:>7.0} {:>8.4} {:>9.3} {:>8.4} {:>7.3} {:>7.4} {:>10.2}",
                name, label, s.t_min, s.t_mean, s.mean_stores, s.current_score,
                s.mean_on_food_fraction, s.mean_served, s.mean_auxiliary, s.proposed_score
            );
        }
    }
    println!("# wall {:.1} s", r.wall_seconds);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::neural::gru::{N, Z};
    use cubarium_core::neural::obs::OBS_LEN;

    fn layout0() -> fixture::Layout {
        fixture::training_layouts_on(&fixture::Ecology::defaults()).remove(0)
    }

    /// The brief's own test for check (a): a hand-built observation whose **only** difference
    /// is `v[0]`, pushed through a hand-built weight set whose response is known in closed
    /// form, must produce exactly the documented action.
    ///
    /// The weights make the network a one-layer function of `v[0]`: the update gate is driven
    /// to zero (`b_iz = −40`, so `z = σ(−40) ≈ 0` and `h' = n`), there is no recurrent term
    /// from a zero hidden state, so `h'_0 = tanh(2·v[0])` and the thrust head is exactly that.
    /// Thrust is the sigmoid of the head, above the adapter's 0.05 deadband throughout.
    #[test]
    fn a_hand_built_weight_set_responds_to_v0_exactly_as_computed() {
        let mut w = Gru32::zeros();
        w.b_i[Z] = -40.0; // z ≈ 0 for unit 0
        w.w_i[N * OBS_LEN] = 2.0; // W_in[unit 0, v[0]] = 2
        w.w_o[THRUST * HIDDEN] = 1.0; // y_thrust = h'_0
        let cap = Capability::ordinary(1.0, 1.0, true, true);
        let base = [0.0f64; OBS_LEN];
        let h = [0.0f64; HIDDEN];

        for level in V0_LEVELS {
            let out = respond(&w, &cap, &variant(&base, Some(level), Ring::AsRecorded), &h);
            let head = (2.0f64 * level).tanh();
            let want = 1.0 / (1.0 + (-head).exp());
            assert!(want > cubarium_core::neural::action::DEADBAND, "the band must not bite");
            assert!(
                (out[0] - want).abs() < 1e-12,
                "v0 {level}: thrust {} is not the computed {want}",
                out[0]
            );
        }
        // Documented difference, end to end: σ(tanh 2) − σ(0) = 0.7 − 0.5.
        let lo = respond(&w, &cap, &variant(&base, Some(0.0), Ring::AsRecorded), &h);
        let hi = respond(&w, &cap, &variant(&base, Some(1.0), Ring::AsRecorded), &h);
        let want = 1.0 / (1.0 + (-(2.0f64).tanh()).exp()) - 0.5;
        assert!((hi[0] - lo[0] - want).abs() < 1e-12, "{} vs {want}", hi[0] - lo[0]);
        assert!((want - 0.223_927_5).abs() < 1e-6, "the documented value moved: {want}");
        // And a weight set that reads nothing from v[0] must not move at all.
        let inert = Gru32::zeros();
        let a = respond(&inert, &cap, &variant(&base, Some(0.0), Ring::AsRecorded), &h);
        let b = respond(&inert, &cap, &variant(&base, Some(1.0), Ring::AsRecorded), &h);
        assert_eq!(a, b, "a policy that ignores v[0] must give the same action");
    }

    /// The variant builder edits exactly what it names and copies everything else.
    #[test]
    fn a_variant_differs_from_its_base_in_exactly_the_named_scalars() {
        let mut base = [0.0f64; OBS_LEN];
        for (i, v) in base.iter_mut().enumerate() {
            *v = (i as f64 + 1.0) / 100.0;
        }
        let only_v0 = variant(&base, Some(0.75), Ring::AsRecorded);
        assert_eq!(only_v0[0], 0.75);
        for i in 1..OBS_LEN {
            assert_eq!(only_v0[i], base[i], "scalar {i} moved and should not have");
        }

        let ahead = variant(&base, None, Ring::Ahead);
        assert_eq!(ahead[0], base[0], "the ring arms leave v[0] alone");
        for i in 0..3 {
            assert_eq!(ahead[i], base[i]);
        }
        for i in FOOD_NEAR + 36..OBS_LEN {
            assert_eq!(ahead[i], base[i], "scalar {i} is outside both rings");
        }
        assert_eq!(ahead[FOOD_NEAR], 1.0, "near sector 0 foliage");
        assert_eq!(ahead[FOOD_FAR], 1.0, "far sector 0 foliage");
        let behind = variant(&base, None, Ring::Behind);
        assert_eq!(behind[FOOD_NEAR + 9], 1.0, "near sector 3 foliage");
        assert_eq!(behind[FOOD_NEAR], 0.0);
        let none = variant(&base, None, Ring::Zero);
        assert!(none[FOOD_NEAR..FOOD_NEAR + 36].iter().all(|v| *v == 0.0));
    }

    /// The brief's own test for check (b): the dwell control leaves a cell after exactly `d`
    /// ticks on it.
    ///
    /// Measured from the world's own post-step cell and resolved travel, not from the rule's
    /// own bookkeeping: a stay is a maximal run of zero-travel ticks, its arrival is the moving
    /// tick before it on the same cell, and `ticks_on_cell` is therefore the whole residence —
    /// one arrival tick that already feeds from the cell (the feeding settlement runs after the
    /// move) plus `d − 1` ticks standing still on it.
    #[test]
    fn the_dwell_control_leaves_a_cell_after_exactly_d_ticks_on_it() {
        let layout = layout0();
        for d in [20u32, 60] {
            let driver = episode::Driver::Control(episode::Control::Dwell(d));
            let stays = stays(&layout, &driver, 600).expect("ok");
            let done: Vec<&Stay> = stays.iter().filter(|s| s.completed).collect();
            assert!(done.len() >= 3, "d {d}: the ladder must complete several stays: {stays:?}");
            for s in &done {
                assert_eq!(
                    s.ticks_on_cell,
                    u64::from(d),
                    "d {d}: cell {} was held for {} ticks from arrival, not {d}",
                    s.cell,
                    s.ticks_on_cell
                );
                assert_eq!(s.still_ticks, u64::from(d) - 1);
            }
            // And every stay is on a cell of the route, not on bare ground.
            let route: Vec<u16> = layout.route().iter().map(|c| c.0).collect();
            for s in &done {
                assert!(route.contains(&s.cell), "d {d}: stayed on {} which is not food", s.cell);
            }
        }
        // The mobile script's own stays are its own rule's, and much longer than any rung here:
        // it leaves on `P < feed_min`, not on a counter.
        let script = stays(
            &layout,
            &episode::Driver::Control(episode::Control::MobileScript),
            600,
        )
        .expect("ok");
        assert!(
            script.iter().filter(|s| s.completed).all(|s| s.ticks_on_cell > 400),
            "the script leaves on depletion, not on a counter: {script:?}"
        );
    }

    /// `Dwell` differs from the mobile script in the departure rule and in nothing else: a
    /// dwell long enough that the cell falls below `feed_min` first still crops it the same
    /// way, and a dwell of exactly the script's own residence reproduces its route order.
    #[test]
    fn the_dwell_control_is_the_mobile_script_with_one_rule_replaced() {
        let layout = layout0();
        let cancel = AtomicBool::new(false);
        let limits = Limits::new(&cancel);
        // Over a window shorter than the script's first departure, the two are identical.
        let script = episode::run(
            &layout,
            &episode::Driver::Control(episode::Control::MobileScript),
            400,
            limits,
            "t",
        )
        .expect("ok");
        let dwell = episode::run(
            &layout,
            &episode::Driver::Control(episode::Control::Dwell(100_000)),
            400,
            limits,
            "t",
        )
        .expect("ok");
        assert_eq!(script.travelled_px, dwell.travelled_px);
        assert_eq!(script.intake_producer, dwell.intake_producer);
        assert_eq!(script.terminal_stores, dwell.terminal_stores);
        // A short dwell leaves earlier and therefore visits more cells.
        let short = episode::run(
            &layout,
            &episode::Driver::Control(episode::Control::Dwell(20)),
            600,
            limits,
            "t",
        )
        .expect("ok");
        let long = episode::run(
            &layout,
            &episode::Driver::Control(episode::Control::Dwell(400)),
            600,
            limits,
            "t",
        )
        .expect("ok");
        assert!(
            short.distinct_cells > long.distinct_cells,
            "{} vs {}",
            short.distinct_cells,
            long.distinct_cells
        );
    }

    /// The auxiliary's arithmetic, on numbers chosen by hand: a life that ends early is
    /// charged −1 for every tick of the horizon it did not live, and the clip is symmetric.
    #[test]
    fn the_auxiliary_charges_the_ticks_after_death_and_clips_both_ways() {
        assert_eq!(margin(3.0, 1.0, 1.0), 1.0, "clipped above");
        assert_eq!(margin(0.0, 9.0, 1.0), -1.0, "clipped below");
        assert!((margin(1.5, 1.0, 2.0) - 0.25).abs() < 1e-15);
        // A body that lived a quarter of the horizon at exactly +1 per tick.
        let t = AUX_HORIZON / 4;
        let a = auxiliary(t as f64, t, AUX_HORIZON);
        assert!((a - (0.25 - 0.75)).abs() < 1e-12, "{a}");
        // The same margin, surviving the horizon.
        assert!((auxiliary(AUX_HORIZON as f64, AUX_HORIZON, AUX_HORIZON) - 1.0).abs() < 1e-12);
        // λ cannot erase a survival difference of 200 ticks.
        assert!(proposed_score(1_000, -1.0) < proposed_score(1_200, 1.0));
        assert!(proposed_score(1_000, 1.0) >= proposed_score(1_199, -1.0));
    }

}
