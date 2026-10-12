//! The arithmetic both the runtime and cloning share: the bottom's input assembly and
//! the top's forward pass. One implementation, so a clone is fitted on exactly the
//! inputs the runtime will build.

use super::shape::{TYPE_TAGS, TopOffsets, TwoLayerShape};

/// What a subject is: a body (sensed or remembered) or a place (§11.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SubjectKind {
    Body,
    Place,
}

impl SubjectKind {
    /// The type tag's one-hot index.
    pub fn tag(self) -> usize {
        match self {
            SubjectKind::Body => 0,
            SubjectKind::Place => 1,
        }
    }
}

/// `1 − exp(−t/τ)`: time in the current intent, squashed into `[0, 1)`.
pub fn squash_time(age_s: f64, scale_s: f64) -> f64 {
    if scale_s > 0.0 {
        1.0 - (-age_s.max(0.0) / scale_s).exp()
    } else {
        1.0
    }
}

/// Fill the bottom's input `x` (width [`TwoLayerShape::bottom_inputs`]):
/// `[observation | held intent one-hot | subject features + tag | squashed time]`.
///
/// `held` is the intent held **before** this sample's decision, `subject` that hold's
/// subject row as it appears in this sample (`None`: no subject, or not found), `time`
/// the squashed time that intent has been held.
pub fn bottom_input(
    shape: &TwoLayerShape,
    observation: &[f64],
    held: Option<usize>,
    subject: Option<(&[f64], SubjectKind)>,
    time: f64,
    x: &mut [f64],
) {
    assert_eq!(observation.len(), shape.observation, "observation width");
    assert_eq!(x.len(), shape.bottom_inputs(), "bottom input width");
    let mut at = shape.observation;
    x[..at].copy_from_slice(observation);
    let block = shape.intent_block();
    x[at..at + block].fill(0.0);
    if let Some(i) = held.filter(|_| block > 0) {
        x[at + i] = 1.0;
    }
    at += block;
    let block = shape.subject_block();
    if block > 0 {
        x[at..at + block].fill(0.0);
        if let Some((features, kind)) = subject {
            x[at..at + shape.row_width].copy_from_slice(features);
            x[at + shape.row_width + kind.tag()] = 1.0;
        }
        at += block;
    }
    if shape.time_block() > 0 {
        x[at] = time;
    }
}

/// The top's hidden layer and intent logits, kept for the pointer and for a backward
/// pass.
#[derive(Clone, Debug, Default)]
pub struct TopForward {
    /// `u = [h | held one-hot | time]`.
    pub u: Vec<f64>,
    /// `z = tanh(w1 u + b1)`.
    pub z: Vec<f64>,
    /// `w2 z + b2`, unmasked.
    pub logits: Vec<f64>,
}

impl TopForward {
    pub fn new(shape: &TwoLayerShape) -> TopForward {
        TopForward {
            u: vec![0.0; shape.top_inputs()],
            z: vec![0.0; shape.top_hidden],
            logits: vec![0.0; shape.intents()],
        }
    }
}

/// The top's forward pass over the GRU state `h`, the previously held intent and the
/// squashed time held.
pub fn top_forward(
    shape: &TwoLayerShape,
    t: &TopOffsets,
    theta: &[f64],
    h: &[f64],
    held: Option<usize>,
    time: f64,
    out: &mut TopForward,
) {
    let (hw, m, th) = (shape.hidden, shape.intents(), shape.top_hidden);
    let uw = shape.top_inputs();
    out.u[..hw].copy_from_slice(h);
    out.u[hw..hw + m].fill(0.0);
    if let Some(i) = held {
        out.u[hw + i] = 1.0;
    }
    out.u[hw + m] = time;
    for j in 0..th {
        let mut a = theta[t.b1 + j];
        let row = &theta[t.w1 + j * uw..t.w1 + (j + 1) * uw];
        for (w, u) in row.iter().zip(&out.u) {
            a += w * u;
        }
        out.z[j] = a.tanh();
    }
    for k in 0..m {
        let mut a = theta[t.b2 + k];
        let row = &theta[t.w2 + k * th..t.w2 + (k + 1) * th];
        for (w, z) in row.iter().zip(&out.z) {
            a += w * z;
        }
        out.logits[k] = a;
    }
}

/// One candidate row as the pointer reads it: features and type tag.
pub fn pointer_row(shape: &TwoLayerShape, features: &[f64], kind: SubjectKind, x: &mut [f64]) {
    assert_eq!(features.len(), shape.row_width, "candidate row width");
    x[..shape.row_width].copy_from_slice(features);
    x[shape.row_width..shape.row_width + TYPE_TAGS].fill(0.0);
    x[shape.row_width + kind.tag()] = 1.0;
}

/// The pointer's pre-activation shared by every row for one proposed intent:
/// `q = wq z + e[:, intent] + bg`.
pub fn pointer_query(
    shape: &TwoLayerShape,
    t: &TopOffsets,
    theta: &[f64],
    z: &[f64],
    intent: usize,
    q: &mut [f64],
) {
    let (p, th, m) = (shape.pointer_width, shape.top_hidden, shape.intents());
    for j in 0..p {
        let mut a = theta[t.bg + j] + theta[t.e + j * m + intent];
        let row = &theta[t.wq + j * th..t.wq + (j + 1) * th];
        for (w, zz) in row.iter().zip(z) {
            a += w * zz;
        }
        q[j] = a;
    }
}

/// One row's pointer score `v · tanh(wr x + q)`, leaving `g = tanh(…)` in `g`.
pub fn pointer_score(
    shape: &TwoLayerShape,
    t: &TopOffsets,
    theta: &[f64],
    x: &[f64],
    q: &[f64],
    g: &mut [f64],
) -> f64 {
    let (p, f) = (shape.pointer_width, shape.row_width + TYPE_TAGS);
    let mut s = 0.0;
    for j in 0..p {
        let mut a = q[j];
        let row = &theta[t.wr + j * f..t.wr + (j + 1) * f];
        for (w, xx) in row.iter().zip(x) {
            a += w * xx;
        }
        g[j] = a.tanh();
        s += theta[t.v + j] * g[j];
    }
    s
}

/// The "no subject" score for a proposed intent.
pub fn none_score(t: &TopOffsets, theta: &[f64], intent: usize) -> f64 {
    theta[t.none + intent]
}

/// Masked softmax of `logits / temperature` into `p` (masked entries 0). Returns `false`
/// when nothing is valid.
pub fn masked_softmax(logits: &[f64], valid: &[bool], temperature: f64, p: &mut [f64]) -> bool {
    let tau = if temperature > 0.0 { temperature } else { 1.0 };
    let peak = logits
        .iter()
        .zip(valid)
        .filter(|(_, v)| **v)
        .map(|(l, _)| l / tau)
        .fold(f64::NEG_INFINITY, f64::max);
    if !peak.is_finite() {
        p.fill(0.0);
        return false;
    }
    let mut sum = 0.0;
    for ((pi, l), v) in p.iter_mut().zip(logits).zip(valid) {
        *pi = if *v { (l / tau - peak).exp() } else { 0.0 };
        sum += *pi;
    }
    for pi in p.iter_mut() {
        *pi /= sum;
    }
    true
}

/// The index of the largest valid entry, lowest index on ties.
pub fn masked_argmax(values: &[f64], valid: &[bool]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, (x, v)) in values.iter().zip(valid).enumerate() {
        if *v && best.is_none_or(|b| *x > values[b]) {
            best = Some(i);
        }
    }
    best
}
