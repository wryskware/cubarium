//! **Cloning the rules into both layers** (§10.2, §11.3).
//!
//! A [`TrajectoryRecord`] carries, per controller sample, the observation, the intent
//! mask, the candidate rows, the teacher's **proposal before its hold** (the top's label),
//! the intent and subject held after arbitration (the bottom's conditioning), and the
//! teacher's actions. [`fit`] fits the whole vector teacher-forced:
//!
//! - **the top**: cross-entropy on the proposed intent over the valid intents, plus the
//!   subject pointer's cross-entropy, conditioned on the proposed intent;
//! - **the bottom**: today's action-imitation loss (squared error on the transferred
//!   action, straight through the deadband as in [`crate::es::voxel::imitate`]), plus
//!   the discrete heads' cross-entropies, teacher-forced on the held intent and subject;
//! - the top's losses flow into the GRU's hidden state, so the strategic loss trains the
//!   shared representation.
//!
//! # The order, and why no label leaks
//!
//! Exactly the runtime's ([`super::brain`]): the bottom at sample `t` reads the intent and
//! subject held **after sample `t − 1`'s** arbitration, with that intent's time held; the
//! top at `t` reads the GRU state that update produced, the same held intent and time, and
//! is scored against the proposal at `t`. Nothing derived from sample `t`'s proposal or
//! hold is in any input at `t` ([`bottom_inputs`] is the one place the inputs are built).
//!
//! The teacher acts at `t` under its own hold at `t`, while the bottom acts at `t` under the
//! hold from `t − 1` (the runtime's order: a settled intent conditions the bottom from the
//! next sample), so at a switch the action label leads the conditioning by one sample.
//!
//! # Room for the second half
//!
//! [`RecordSource`] names the alternative-mode examples (staged scenes invoking a teacher
//! skill in a mode it would not have chosen: `proposal` is `None`, `held` the invoked
//! mode) and the closed-loop correction pass (the learner's own holds as `held`, the
//! teacher's labels as `proposal` and actions). Producing them needs the fauna side.

use serde::{Deserialize, Serialize};

use super::arbitration::{EarlyEnd, IntentArbitration};
use super::brain::{CandidateRow, DecisionSample, SubjectRef, TwoLayerNet};
use super::net::{
    SubjectKind, TopForward, bottom_input, none_score, pointer_query, pointer_row, pointer_score,
    squash_time, top_forward,
};
use super::shape::{TYPE_TAGS, TwoLayerLayout, TwoLayerShape, initial_center};
use crate::es::optimizer::Adam;

/// Where a trajectory's samples came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordSource {
    /// The teacher's own choices.
    Teacher,
    /// A staged scene invoking a teacher skill in a mode it would not have chosen
    /// (§11.3): `held` is the invoked mode; `proposal` is `None` unless the teacher's own
    /// proposal is wanted as a label too.
    AlternativeMode,
    /// The closed-loop correction pass: the learner picked the held intents, the teacher
    /// labelled the states it reached.
    ClosedLoop,
}

/// One candidate row as recorded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordedCandidate {
    pub key: u64,
    pub kind: SubjectKind,
    pub features: Vec<f64>,
    /// Valid as the subject of each named intent (menu long).
    pub valid_for: Vec<bool>,
}

/// An intent and its subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedChoice {
    pub intent: usize,
    pub subject: Option<SubjectRef>,
}

/// One controller sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordedSample {
    pub observation: Vec<f64>,
    /// Which named intents were valid (menu long).
    pub intent_valid: Vec<bool>,
    pub candidates: Vec<RecordedCandidate>,
    /// The fauna's early-end flag at this sample, replayed by [`evaluate`].
    #[serde(default)]
    pub early_end: Option<EarlyEnd>,
    /// The teacher's proposal **before its hold**, at a decision sample: the top's label.
    /// `None` where the top gets no label (between decisions, invoked modes).
    pub proposal: Option<RecordedChoice>,
    /// The intent and subject held after this sample's arbitration (the teacher's, or the
    /// learner's in a closed-loop pass). The bottom is conditioned on it from the **next**
    /// sample.
    pub held: RecordedChoice,
    /// The teacher's continuous actions, adapted, in manifest order.
    pub actions: Vec<f64>,
    #[serde(default)]
    pub kick: Option<bool>,
    /// `Some(None)`: silence; `Some(Some(k))`: call kind `k`; `None`: no label.
    #[serde(default)]
    pub call: Option<Option<usize>>,
    #[serde(default)]
    pub rest: Option<bool>,
}

/// One body's stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryRecord {
    pub source: RecordSource,
    /// The lineage's controller period, seconds.
    pub sample_period_s: f64,
    pub samples: Vec<RecordedSample>,
}

/// The bottom's input at every sample of `record`, in the runtime's order: the
/// observation, the hold from the previous sample, that hold's subject row as it appears
/// in this sample, and the squashed time in the held intent. Cloning reads nothing else.
pub fn bottom_inputs(
    shape: &TwoLayerShape,
    arbitration: &IntentArbitration,
    record: &TrajectoryRecord,
) -> Vec<Vec<f64>> {
    conditioning(shape, arbitration, record)
        .into_iter()
        .map(|c| c.x)
        .collect()
}

struct Conditioning {
    x: Vec<f64>,
    held: Option<usize>,
    time: f64,
}

fn conditioning(
    shape: &TwoLayerShape,
    arbitration: &IntentArbitration,
    record: &TrajectoryRecord,
) -> Vec<Conditioning> {
    let mut out = Vec::with_capacity(record.samples.len());
    let mut before: Option<RecordedChoice> = None;
    let mut age = 0.0;
    for sample in &record.samples {
        let held = before.filter(|_| shape.has_top());
        let row = held.and_then(|h| h.subject).and_then(|s| {
            sample
                .candidates
                .iter()
                .find(|c| c.key == s.key && c.kind == s.kind)
                .map(|c| (c.features.as_slice(), c.kind))
        });
        let time = squash_time(age, arbitration.intent_time_scale_s);
        let mut x = vec![0.0; shape.bottom_inputs()];
        bottom_input(
            shape,
            &sample.observation,
            held.map(|h| h.intent),
            row,
            time,
            &mut x,
        );
        out.push(Conditioning {
            x,
            held: held.map(|h| h.intent),
            time,
        });
        if before.is_none_or(|b| b.intent != sample.held.intent) {
            age = 0.0;
        }
        age += record.sample_period_s;
        before = Some(sample.held);
    }
    out
}

/// How the clone is fitted (the fit's numbers, not the world's).
#[derive(Clone, Copy, Debug)]
pub struct CloneSpec {
    /// Adam steps over the full record set.
    pub updates: u32,
    pub learning_rate: f64,
    /// Truncated-BPTT window in controller samples; the hidden state carries across it.
    pub chunk: usize,
    /// Global gradient-norm clip.
    pub clip: f64,
    /// Seed of the starting vector, when none is given.
    pub seed: u64,
    /// Threads over records; the reduction is in record order, so the fit is
    /// deterministic whatever this is.
    pub workers: usize,
}

impl Default for CloneSpec {
    fn default() -> CloneSpec {
        CloneSpec {
            updates: 1_500,
            learning_rate: 0.01,
            chunk: 120,
            clip: 5.0,
            seed: crate::es::voxel::task::TRAINING_SEED,
            workers: crate::es::voxel::task::episode_worker_limit(),
        }
    }
}

/// Teacher-forced losses, each the mean over its own labels, and the top's agreement.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CloneLosses {
    pub intent_ce: f64,
    /// Fraction of intent labels the top's masked argmax matches.
    pub intent_agreement: f64,
    pub subject_ce: f64,
    pub subject_agreement: f64,
    /// Mean squared error per continuous action value.
    pub action_mse: f64,
    pub kick_bce: f64,
    pub call_ce: f64,
    pub rest_bce: f64,
    pub labels: LabelCounts,
}

/// How many labels of each kind the records gave, and how many were unusable (a label
/// on a masked intent, or a subject not among the candidates or invalid for its intent).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelCounts {
    pub samples: usize,
    pub intents: usize,
    pub subjects: usize,
    pub kicks: usize,
    pub calls: usize,
    pub rests: usize,
    pub unusable: usize,
}

/// What a finished fit reports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloneReport {
    pub updates: u32,
    pub initial: CloneLosses,
    pub last: CloneLosses,
    /// The weighted total every 25 updates and at the end.
    pub loss_history: Vec<f64>,
}

/// One sample's labels, resolved against the shape once.
struct Labels {
    valid: Vec<bool>,
    intent: Option<usize>,
    /// `(intent, rows (K × (F+2)), valid (K+1), label in 0..=K)`.
    pointer: Option<(usize, Vec<f64>, Vec<bool>, usize)>,
    call: Option<usize>,
}

/// A record prepared for the fit: conditioning and labels, theta-independent.
struct Prepared {
    cond: Vec<Conditioning>,
    labels: Vec<Labels>,
}

fn prepare(
    shape: &TwoLayerShape,
    arbitration: &IntentArbitration,
    record: &TrajectoryRecord,
    counts: &mut LabelCounts,
) -> Result<Prepared, String> {
    let cond = conditioning(shape, arbitration, record);
    let (k, f) = (shape.candidates, shape.row_width + TYPE_TAGS);
    let mut labels = Vec::with_capacity(record.samples.len());
    for (t, s) in record.samples.iter().enumerate() {
        if s.actions.len() != shape.actions.len() {
            return Err(format!(
                "sample {t}: {} actions, the shape has {}",
                s.actions.len(),
                shape.actions.len()
            ));
        }
        if s.candidates.len() > k {
            return Err(format!("sample {t}: {} candidates, K is {k}", s.candidates.len()));
        }
        counts.samples += 1;
        let mut valid = vec![false; shape.intents()];
        if shape.has_top() {
            valid[..shape.menu].copy_from_slice(&s.intent_valid);
        }
        let mut intent = None;
        let mut pointer = None;
        if let Some(p) = s.proposal.filter(|_| shape.has_top()) {
            if valid.get(p.intent).copied().unwrap_or(false) {
                intent = Some(p.intent);
                counts.intents += 1;
            } else {
                counts.unusable += 1;
            }
            if k > 0 && intent.is_some() {
                let mut rows = vec![0.0; k * f];
                let mut ok = vec![false; k + 1];
                ok[k] = true;
                for (j, c) in s.candidates.iter().enumerate() {
                    if c.valid_for.get(p.intent).copied().unwrap_or(false) {
                        pointer_row(shape, &c.features, c.kind, &mut rows[j * f..(j + 1) * f]);
                        ok[j] = true;
                    }
                }
                let label = match p.subject {
                    None => Some(k),
                    Some(r) => s
                        .candidates
                        .iter()
                        .position(|c| c.key == r.key && c.kind == r.kind)
                        .filter(|&j| ok[j]),
                };
                match label {
                    Some(l) => {
                        pointer = Some((p.intent, rows, ok, l));
                        counts.subjects += 1;
                    }
                    None => counts.unusable += 1,
                }
            }
        }
        let call = match s.call.filter(|_| shape.call_kinds > 0) {
            Some(None) => Some(0),
            Some(Some(c)) if c < shape.call_kinds => Some(c + 1),
            Some(Some(_)) => {
                counts.unusable += 1;
                None
            }
            None => None,
        };
        counts.calls += usize::from(call.is_some());
        counts.kicks += usize::from(s.kick.is_some() && shape.kick);
        counts.rests += usize::from(s.rest.is_some() && shape.rest);
        labels.push(Labels {
            valid,
            intent,
            pointer,
            call,
        });
    }
    Ok(Prepared { cond, labels })
}

/// Per-term weights: one over each term's label count, so every term is a mean.
#[derive(Clone, Copy)]
struct Weights {
    intent: f64,
    subject: f64,
    action: f64,
    kick: f64,
    call: f64,
    rest: f64,
}

impl Weights {
    fn new(c: &LabelCounts, actions: usize) -> Weights {
        let inv = |n: usize| if n > 0 { 1.0 / n as f64 } else { 0.0 };
        Weights {
            intent: inv(c.intents),
            subject: inv(c.subjects),
            action: inv(c.samples * actions),
            kick: inv(c.kicks),
            call: inv(c.calls),
            rest: inv(c.rests),
        }
    }
}

/// Raw sums for [`CloneLosses`].
#[derive(Clone, Copy, Default)]
struct Sums {
    intent_ce: f64,
    intent_hits: usize,
    subject_ce: f64,
    subject_hits: usize,
    action_se: f64,
    kick: f64,
    call: f64,
    rest: f64,
    weighted: f64,
}

impl Sums {
    fn add(&mut self, o: &Sums) {
        self.intent_ce += o.intent_ce;
        self.intent_hits += o.intent_hits;
        self.subject_ce += o.subject_ce;
        self.subject_hits += o.subject_hits;
        self.action_se += o.action_se;
        self.kick += o.kick;
        self.call += o.call;
        self.rest += o.rest;
        self.weighted += o.weighted;
    }

    fn losses(&self, c: &LabelCounts, actions: usize) -> CloneLosses {
        let mean = |x: f64, n: usize| if n > 0 { x / n as f64 } else { 0.0 };
        CloneLosses {
            intent_ce: mean(self.intent_ce, c.intents),
            intent_agreement: mean(self.intent_hits as f64, c.intents),
            subject_ce: mean(self.subject_ce, c.subjects),
            subject_agreement: mean(self.subject_hits as f64, c.subjects),
            action_mse: mean(self.action_se, c.samples * actions),
            kick_bce: mean(self.kick, c.kicks),
            call_ce: mean(self.call, c.calls),
            rest_bce: mean(self.rest, c.rests),
            labels: *c,
        }
    }
}

fn sigmoid(v: f64) -> f64 {
    1.0 / (1.0 + (-v).exp())
}

/// `ln Σ exp` over the valid entries, and the softmax into `p`.
fn log_softmax(logits: &[f64], valid: &[bool], p: &mut [f64]) -> f64 {
    let peak = logits
        .iter()
        .zip(valid)
        .filter(|(_, v)| **v)
        .map(|(l, _)| *l)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut sum = 0.0;
    for ((pi, l), v) in p.iter_mut().zip(logits).zip(valid) {
        *pi = if *v { (l - peak).exp() } else { 0.0 };
        sum += *pi;
    }
    for pi in p.iter_mut() {
        *pi /= sum;
    }
    peak + sum.ln()
}

fn argmax_valid(values: &[f64], valid: &[bool]) -> Option<usize> {
    super::net::masked_argmax(values, valid)
}

/// One record's pass: teacher-forced forward over truncated windows, losses into `sums`,
/// and — with `grad` — the backward pass accumulated into it.
fn record_pass(
    shape: &TwoLayerShape,
    layout: &TwoLayerLayout,
    theta: &[f64],
    prep: &Prepared,
    record: &TrajectoryRecord,
    chunk: usize,
    w: Weights,
    sums: &mut Sums,
    mut grad: Option<&mut [f64]>,
) {
    let hw = shape.hidden;
    let mut hidden = vec![0.0; hw];
    let mut start = 0;
    let n = record.samples.len();
    while start < n {
        let end = (start + chunk.max(1)).min(n);
        window(
            shape,
            layout,
            theta,
            prep,
            record,
            start..end,
            &mut hidden,
            w,
            sums,
            grad.as_deref_mut(),
        );
        start = end;
    }
}

/// The function the finite-difference test checks: one truncated window.
#[allow(clippy::too_many_arguments)]
fn window(
    shape: &TwoLayerShape,
    layout: &TwoLayerLayout,
    theta: &[f64],
    prep: &Prepared,
    record: &TrajectoryRecord,
    range: std::ops::Range<usize>,
    hidden: &mut [f64],
    w: Weights,
    sums: &mut Sums,
    mut grad: Option<&mut [f64]>,
) {
    let b = &layout.bottom;
    let (hw, inputs, outputs) = (shape.hidden, shape.bottom_inputs(), shape.bottom_outputs());
    let gates = 3 * hw;
    let steps = range.len();
    let heads = shape.heads();
    let h0 = hidden.to_vec();
    let mut gh = vec![0.0; steps * gates];
    let mut rr = vec![0.0; steps * hw];
    let mut zz = vec![0.0; steps * hw];
    let mut nn = vec![0.0; steps * hw];
    let mut hh = vec![0.0; steps * hw];
    let mut dy = vec![0.0; steps * outputs];
    let mut dh_top = vec![0.0; steps * hw];
    let mut gi = vec![0.0; gates];
    let mut y = vec![0.0; outputs];
    let mut top = TopForward::new(shape);
    let (m, th) = (shape.intents(), shape.top_hidden);
    let mut p_int = vec![0.0; m];
    let (k, f, pw) = (shape.candidates, shape.row_width + TYPE_TAGS, shape.pointer_width);
    let mut q = vec![0.0; pw];
    let mut g_rows = vec![0.0; k * pw];
    let mut scores = vec![0.0; k + 1];
    let mut p_ptr = vec![0.0; k + 1];
    let mut call_p = vec![0.0; shape.call_logits()];

    for (i, t) in range.clone().enumerate() {
        let x = &prep.cond[t].x;
        let hp: Vec<f64> = if i == 0 {
            h0.clone()
        } else {
            hh[(i - 1) * hw..i * hw].to_vec()
        };
        for row in 0..gates {
            let mut a = theta[b.b_i + row];
            let wi = &theta[b.w_i + row * inputs..b.w_i + (row + 1) * inputs];
            for (wv, xv) in wi.iter().zip(x) {
                a += wv * xv;
            }
            gi[row] = a;
            let mut c = theta[b.b_h + row];
            let whr = &theta[b.w_h + row * hw..b.w_h + (row + 1) * hw];
            for (wv, hv) in whr.iter().zip(&hp) {
                c += wv * hv;
            }
            gh[i * gates + row] = c;
        }
        for j in 0..hw {
            let g = &gh[i * gates..(i + 1) * gates];
            let r = sigmoid(gi[j] + g[j]);
            let z = sigmoid(gi[hw + j] + g[hw + j]);
            let nv = (gi[2 * hw + j] + r * g[2 * hw + j]).tanh();
            rr[i * hw + j] = r;
            zz[i * hw + j] = z;
            nn[i * hw + j] = nv;
            hh[i * hw + j] = z * hp[j] + (1.0 - z) * nv;
        }
        let h = hh[i * hw..(i + 1) * hw].to_vec();
        for (o, yo) in y.iter_mut().enumerate() {
            let mut a = theta[b.b_o + o];
            let wo = &theta[b.w_o + o * hw..b.w_o + (o + 1) * hw];
            for (wv, hv) in wo.iter().zip(&h) {
                a += wv * hv;
            }
            *yo = a;
        }

        // The bottom's losses.
        let s = &record.samples[t];
        let d = &mut dy[i * outputs..(i + 1) * outputs];
        for (a, tr) in heads.continuous.clone().zip(&shape.actions) {
            let v = tr.apply(y[a]);
            let e = v - s.actions[a];
            sums.action_se += e * e;
            sums.weighted += w.action * e * e;
            d[a] = 2.0 * w.action * e * tr.derivative(y[a], v);
        }
        for (head, label, wt, slot) in [
            (heads.kick, s.kick, w.kick, 0),
            (heads.rest, s.rest, w.rest, 1),
        ] {
            if let (Some(o), Some(lbl)) = (head, label) {
                let pz = sigmoid(y[o]);
                let target = f64::from(u8::from(lbl));
                let bce = -(target * pz.max(1e-300).ln() + (1.0 - target) * (1.0 - pz).max(1e-300).ln());
                if slot == 0 {
                    sums.kick += bce;
                } else {
                    sums.rest += bce;
                }
                sums.weighted += wt * bce;
                d[o] = wt * (pz - target);
            }
        }
        if let (Some(r), Some(lbl)) = (heads.call.clone(), prep.labels[t].call) {
            let all = vec![true; r.len()];
            let lse = log_softmax(&y[r.clone()], &all, &mut call_p);
            let ce = lse - y[r.start + lbl];
            sums.call += ce;
            sums.weighted += w.call * ce;
            for (c, o) in r.enumerate() {
                d[o] = w.call * (call_p[c] - f64::from(u8::from(c == lbl)));
            }
        }

        // The top's losses, on the state this update produced.
        let lab = &prep.labels[t];
        if let Some(to) = layout.top.filter(|_| lab.intent.is_some() || lab.pointer.is_some()) {
            let c = &prep.cond[t];
            top_forward(shape, &to, theta, &h, c.held, c.time, &mut top);
            let mut dz = vec![0.0; th];
            let mut gtop = grad.as_deref_mut();
            if let Some(label) = lab.intent {
                let lse = log_softmax(&top.logits, &lab.valid, &mut p_int);
                let ce = lse - top.logits[label];
                sums.intent_ce += ce;
                sums.weighted += w.intent * ce;
                if argmax_valid(&top.logits, &lab.valid) == Some(label) {
                    sums.intent_hits += 1;
                }
                if let Some(gr) = gtop.as_deref_mut() {
                    for kk in 0..m {
                        let dl = w.intent * (p_int[kk] - f64::from(u8::from(kk == label)));
                        if dl == 0.0 {
                            continue;
                        }
                        gr[to.b2 + kk] += dl;
                        for j in 0..th {
                            gr[to.w2 + kk * th + j] += dl * top.z[j];
                            dz[j] += dl * theta[to.w2 + kk * th + j];
                        }
                    }
                }
            }
            if let Some((intent, rows, ok, label)) = &lab.pointer {
                pointer_query(shape, &to, theta, &top.z, *intent, &mut q);
                for j in 0..k {
                    scores[j] = if ok[j] {
                        pointer_score(
                            shape,
                            &to,
                            theta,
                            &rows[j * f..(j + 1) * f],
                            &q,
                            &mut g_rows[j * pw..(j + 1) * pw],
                        )
                    } else {
                        0.0
                    };
                }
                scores[k] = none_score(&to, theta, *intent);
                let lse = log_softmax(&scores, ok, &mut p_ptr);
                let ce = lse - scores[*label];
                sums.subject_ce += ce;
                sums.weighted += w.subject * ce;
                if argmax_valid(&scores, ok) == Some(*label) {
                    sums.subject_hits += 1;
                }
                if let Some(gr) = gtop.as_deref_mut() {
                    let mut dq = vec![0.0; pw];
                    for j in 0..=k {
                        if !ok[j] {
                            continue;
                        }
                        let ds = w.subject * (p_ptr[j] - f64::from(u8::from(j == *label)));
                        if j == k {
                            gr[to.none + intent] += ds;
                            continue;
                        }
                        let gj = &g_rows[j * pw..(j + 1) * pw];
                        let xj = &rows[j * f..(j + 1) * f];
                        for u in 0..pw {
                            gr[to.v + u] += ds * gj[u];
                            let da = ds * theta[to.v + u] * (1.0 - gj[u] * gj[u]);
                            dq[u] += da;
                            for (c, xv) in xj.iter().enumerate() {
                                gr[to.wr + u * f + c] += da * xv;
                            }
                        }
                    }
                    for u in 0..pw {
                        gr[to.bg + u] += dq[u];
                        gr[to.e + u * m + intent] += dq[u];
                        for j in 0..th {
                            gr[to.wq + u * th + j] += dq[u] * top.z[j];
                            dz[j] += dq[u] * theta[to.wq + u * th + j];
                        }
                    }
                }
            }
            if let Some(gr) = gtop {
                let uw = shape.top_inputs();
                let dht = &mut dh_top[i * hw..(i + 1) * hw];
                for j in 0..th {
                    let da = dz[j] * (1.0 - top.z[j] * top.z[j]);
                    if da == 0.0 {
                        continue;
                    }
                    gr[to.b1 + j] += da;
                    for c in 0..uw {
                        gr[to.w1 + j * uw + c] += da * top.u[c];
                    }
                    for c in 0..hw {
                        dht[c] += da * theta[to.w1 + j * uw + c];
                    }
                }
            }
        }
    }
    hidden.copy_from_slice(&hh[(steps - 1) * hw..steps * hw]);

    let Some(grad) = grad else {
        return;
    };
    // Backward through the GRU, as `imitate::gradient_of_chunk_options`, with the top's
    // gradient joining the hidden state at each sample.
    let mut dh_next = vec![0.0; hw];
    let mut dh = vec![0.0; hw];
    let mut dhp = vec![0.0; hw];
    let mut dgi = vec![0.0; gates];
    let mut dgh = vec![0.0; gates];
    for i in (0..steps).rev() {
        let t = range.start + i;
        let x = &prep.cond[t].x;
        let hp: &[f64] = if i == 0 {
            &h0
        } else {
            &hh[(i - 1) * hw..i * hw]
        };
        for j in 0..hw {
            dh[j] = dh_next[j] + dh_top[i * hw + j];
        }
        for o in 0..outputs {
            let d = dy[i * outputs + o];
            if d == 0.0 {
                continue;
            }
            grad[b.b_o + o] += d;
            for j in 0..hw {
                grad[b.w_o + o * hw + j] += d * hh[i * hw + j];
                dh[j] += d * theta[b.w_o + o * hw + j];
            }
        }
        dhp.fill(0.0);
        for j in 0..hw {
            let z = zz[i * hw + j];
            let nv = nn[i * hw + j];
            let r = rr[i * hw + j];
            let gh_n = gh[i * gates + 2 * hw + j];
            let dz = dh[j] * (hp[j] - nv);
            let dn = dh[j] * (1.0 - z);
            dhp[j] += dh[j] * z;
            let da_n = dn * (1.0 - nv * nv);
            dgi[2 * hw + j] = da_n;
            dgh[2 * hw + j] = da_n * r;
            let da_z = dz * z * (1.0 - z);
            dgi[hw + j] = da_z;
            dgh[hw + j] = da_z;
            let da_r = da_n * gh_n * r * (1.0 - r);
            dgi[j] = da_r;
            dgh[j] = da_r;
        }
        for row in 0..gates {
            let (a, c) = (dgi[row], dgh[row]);
            grad[b.b_i + row] += a;
            grad[b.b_h + row] += c;
            if a != 0.0 {
                for (col, xv) in x.iter().enumerate() {
                    grad[b.w_i + row * inputs + col] += a * xv;
                }
            }
            for col in 0..hw {
                grad[b.w_h + row * hw + col] += c * hp[col];
                dhp[col] += c * theta[b.w_h + row * hw + col];
            }
        }
        dh_next.copy_from_slice(&dhp);
    }
}

/// Prepared records with their label counts and weights.
pub struct CloneSet<'a> {
    shape: TwoLayerShape,
    layout: TwoLayerLayout,
    records: &'a [TrajectoryRecord],
    prepared: Vec<Prepared>,
    counts: LabelCounts,
    weights: Weights,
}

impl<'a> CloneSet<'a> {
    /// Prepare `records` for `shape` under the lineage's `arbitration` (its time scale
    /// builds the time-in-intent input). Refuses a record of another width by name.
    pub fn new(
        shape: &TwoLayerShape,
        arbitration: &IntentArbitration,
        records: &'a [TrajectoryRecord],
    ) -> Result<CloneSet<'a>, String> {
        shape.validate()?;
        let mut counts = LabelCounts::default();
        let prepared = records
            .iter()
            .enumerate()
            .map(|(i, r)| {
                prepare(shape, arbitration, r, &mut counts).map_err(|e| format!("record {i}: {e}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CloneSet {
            shape: shape.clone(),
            layout: TwoLayerLayout::new(shape),
            records,
            weights: Weights::new(&counts, shape.actions.len()),
            prepared,
            counts,
        })
    }

    pub fn counts(&self) -> LabelCounts {
        self.counts
    }

    /// The weighted loss and (with `grad`) its gradient at `theta`, deterministic: records
    /// are split statically over `workers` and reduced in record order.
    pub fn loss(&self, theta: &[f64], chunk: usize, workers: usize, grad: Option<&mut [f64]>) -> CloneLosses {
        self.pass(theta, chunk, workers, grad).0
    }

    fn pass(
        &self,
        theta: &[f64],
        chunk: usize,
        workers: usize,
        grad: Option<&mut [f64]>,
    ) -> (CloneLosses, f64) {
        assert_eq!(theta.len(), self.layout.params);
        let want_grad = grad.is_some();
        let n = self.records.len();
        let workers = workers.clamp(1, n.max(1));
        let per = n.div_ceil(workers).max(1);
        let parts: Vec<(Sums, Vec<f64>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|wk| {
                    scope.spawn(move || {
                        let mut sums = Sums::default();
                        let mut g = if want_grad { vec![0.0; theta.len()] } else { Vec::new() };
                        for i in wk * per..((wk + 1) * per).min(n) {
                            record_pass(
                                &self.shape,
                                &self.layout,
                                theta,
                                &self.prepared[i],
                                &self.records[i],
                                chunk,
                                self.weights,
                                &mut sums,
                                want_grad.then_some(g.as_mut_slice()),
                            );
                        }
                        (sums, g)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("clone worker")).collect()
        });
        let mut sums = Sums::default();
        if let Some(grad) = grad {
            grad.fill(0.0);
            for (s, g) in &parts {
                sums.add(s);
                for (a, b) in grad.iter_mut().zip(g) {
                    *a += b;
                }
            }
        } else {
            for (s, _) in &parts {
                sums.add(s);
            }
        }
        (sums.losses(&self.counts, self.shape.actions.len()), sums.weighted)
    }
}

/// Fit both layers to `records`, teacher-forced, from `start` (or a seeded centre).
pub fn fit(
    shape: &TwoLayerShape,
    arbitration: &IntentArbitration,
    records: &[TrajectoryRecord],
    start: Option<&[f64]>,
    spec: &CloneSpec,
) -> Result<(Vec<f64>, CloneReport), String> {
    let set = CloneSet::new(shape, arbitration, records)?;
    let mut theta = match start {
        Some(s) => {
            TwoLayerNet::new(shape.clone(), s.to_vec())?;
            s.to_vec()
        }
        None => initial_center(shape, spec.seed),
    };
    let mut adam = Adam::new(theta.len());
    adam.learning_rate = spec.learning_rate;
    let mut grad = vec![0.0; theta.len()];
    let (initial, _) = set.pass(&theta, spec.chunk, spec.workers, None);
    let mut loss_history = Vec::new();
    for update in 0..spec.updates {
        let (_, loss) = set.pass(&theta, spec.chunk, spec.workers, Some(&mut grad));
        let norm = grad.iter().map(|g| g * g).sum::<f64>().sqrt();
        let factor = if norm > spec.clip && norm.is_finite() {
            spec.clip / norm
        } else {
            1.0
        };
        let descent: Vec<f64> = grad.iter().map(|g| -g * factor).collect();
        adam.ascend(&mut theta, &descent);
        if update % 25 == 0 || update + 1 == spec.updates {
            loss_history.push(loss);
        }
    }
    let (last, _) = set.pass(&theta, spec.chunk, spec.workers, None);
    Ok((
        theta,
        CloneReport {
            updates: spec.updates,
            initial,
            last,
            loss_history,
        },
    ))
}

/// A closed-loop replay: the brain runs on a record's observations, masks, candidates and
/// early ends, choosing its own intents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClosedLoopReport {
    pub samples: usize,
    /// Fraction of samples whose held intent is the record's.
    pub intent_agreement: f64,
    /// Fraction of samples whose held subject is the record's.
    pub subject_agreement: f64,
    /// Mean squared error of the transferred continuous actions against the record's.
    pub action_mse: f64,
}

/// Run a fresh brain over `records` closed-loop and compare with what they hold.
pub fn evaluate(
    net: &std::sync::Arc<TwoLayerNet>,
    arbitration: IntentArbitration,
    records: &[TrajectoryRecord],
) -> ClosedLoopReport {
    let shape = net.shape();
    let (mut n, mut hits, mut subject_hits, mut se) = (0usize, 0usize, 0usize, 0.0);
    for record in records {
        let mut brain = net.brain(arbitration, record.sample_period_s);
        for s in &record.samples {
            let rows: Vec<CandidateRow<'_>> = s
                .candidates
                .iter()
                .map(|c| CandidateRow {
                    key: c.key,
                    kind: c.kind,
                    features: &c.features,
                    valid_for: &c.valid_for,
                })
                .collect();
            let mask: Vec<bool> = if shape.has_top() {
                s.intent_valid.clone()
            } else {
                vec![true; shape.menu]
            };
            let a = brain.step(&DecisionSample {
                observation: &s.observation,
                intent_valid: &mask,
                candidates: &rows,
                early_end: s.early_end,
            });
            n += 1;
            hits += usize::from(a.intent == s.held.intent);
            subject_hits += usize::from(a.subject_ref == s.held.subject);
            for (k, tr) in shape.actions.iter().enumerate() {
                let e = tr.apply(a.action_logits[k]) - s.actions[k];
                se += e * e;
            }
        }
    }
    let nn = n.max(1) as f64;
    ClosedLoopReport {
        samples: n,
        intent_agreement: hits as f64 / nn,
        subject_agreement: subject_hits as f64 / nn,
        action_mse: se / (nn * shape.actions.len() as f64),
    }
}

#[cfg(test)]
pub(super) fn window_for_test(
    shape: &TwoLayerShape,
    arbitration: &IntentArbitration,
    records: &[TrajectoryRecord],
    theta: &[f64],
    chunk: usize,
    grad: Option<&mut [f64]>,
) -> f64 {
    let set = CloneSet::new(shape, arbitration, records).expect("set");
    set.pass(theta, chunk, 1, grad).1
}
