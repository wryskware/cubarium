//! The two-layer brain at runtime: one body's controller over plain inputs.
//!
//! # Order at every controller sample (§11.3)
//!
//! 1. The brain finds its held subject among this sample's candidates by its opaque key
//!    (the key never enters a network) and notes any early end: the fauna's flag, the
//!    subject gone or invalid for the held intent, the held intent masked.
//! 2. **The GRU updates once** on the observation plus the intent held *before* this
//!    sample's decision, that hold's subject row and the squashed time in the intent.
//! 3. **At a decision** (the top's rate is a credit of `period × intent_hz` per sample; an
//!    early end decides at once) the top reads that GRU state, the held intent and the
//!    time held, and proposes an intent and, conditioned on it, a subject.
//! 4. **Arbitration** ([`super::arbitration::settle`]) settles the held intent and subject.
//! 5. **The same-sample readout** (§11.3, amended 2026-10-12): this sample's actions read
//!    the new hidden state *and* the intent and subject just settled
//!    ([`super::net::readout`]), so a mode change acts at once. The recurrent update reads
//!    the settled hold from the next sample (step 2), so the top never reads its own
//!    choice through the state it chooses from.
//!
//! A plain brain has no top: steps 1, 3, 4 and 5 do nothing and step 2 is today's GRU
//! step, with today's input, on today's weights.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use cubarium_voxel_fauna::Response;

use super::arbitration::{
    EarlyEnd, HoldChange, HoldView, IntentArbitration, ProposalView, Settle, settle,
};
use super::net::{
    SubjectKind, TopForward, bottom_input, conditioning, masked_argmax, masked_softmax,
    none_score, pointer_query, pointer_row, pointer_score, readout, squash_time, top_forward,
};
use super::shape::{TYPE_TAGS, TwoLayerLayout, TwoLayerShape};
use crate::es::tensor;

/// One lineage's validated two-layer weights, shared by every body that runs them.
#[derive(Clone, Debug, PartialEq)]
pub struct TwoLayerNet {
    shape: TwoLayerShape,
    layout: TwoLayerLayout,
    theta: Vec<f64>,
}

impl TwoLayerNet {
    /// Validate `theta` against `shape`: a consistent shape, the layout's length, finite
    /// values. Refused by name, never reinterpreted.
    pub fn new(shape: TwoLayerShape, theta: Vec<f64>) -> Result<TwoLayerNet, String> {
        shape.validate()?;
        let layout = TwoLayerLayout::new(&shape);
        if theta.len() != layout.params {
            return Err(format!(
                "two-layer theta has {} values, expected {} (bottom {}, top {})",
                theta.len(),
                layout.params,
                layout.bottom.params,
                layout.params - layout.bottom.params
            ));
        }
        if let Some(i) = theta.iter().position(|x| !x.is_finite()) {
            return Err(format!("two-layer theta[{i}] is not finite"));
        }
        Ok(TwoLayerNet {
            shape,
            layout,
            theta,
        })
    }

    pub fn shape(&self) -> &TwoLayerShape {
        &self.shape
    }
    pub fn layout(&self) -> &TwoLayerLayout {
        &self.layout
    }
    pub fn theta(&self) -> &[f64] {
        &self.theta
    }

    /// A fresh body's brain: shared weights, its own memory.
    ///
    /// `sample_period_s` is the lineage's controller period (the manifest's
    /// `controller_period_s`): hold ages and the top's rate count in it.
    pub fn brain(
        self: &Arc<Self>,
        arbitration: IntentArbitration,
        sample_period_s: f64,
    ) -> TwoLayerBrain {
        let s = &self.shape;
        TwoLayerBrain {
            net: Arc::clone(self),
            arbitration,
            sample_period_s,
            state: BrainState::fresh(s),
            x: vec![0.0; s.bottom_inputs()],
            c: vec![0.0; s.conditioning()],
            next: vec![0.0; s.hidden],
            y: vec![0.0; s.bottom_outputs()],
            valid: vec![false; s.intents()],
            top: TopForward::new(s),
            q: vec![0.0; s.pointer_width],
            g: vec![0.0; s.pointer_width],
            row: vec![0.0; s.row_width + TYPE_TAGS],
            scores: vec![0.0; s.candidates + 1],
            row_valid: vec![false; s.candidates + 1],
            all_valid: vec![true; s.menu],
            answer: BrainAnswer::new(s),
        }
    }
}

/// One subject candidate, as the fauna hands it over.
#[derive(Clone, Copy, Debug)]
pub struct CandidateRow<'a> {
    /// An opaque identity, stable across samples for the same body or place (the fauna's
    /// id, or a place's own key). The brain holds its subject by it and re-finds the
    /// subject's row each sample; it never enters a network (§11.1).
    pub key: u64,
    pub kind: SubjectKind,
    /// `F` features, the lineage's fixed row width.
    pub features: &'a [f64],
    /// Whether this row may be the subject of each named intent (menu long).
    pub valid_for: &'a [bool],
}

/// What the fauna gives the brain at one controller sample.
#[derive(Clone, Copy, Debug)]
pub struct DecisionSample<'a> {
    /// The observation vector, in manifest order.
    pub observation: &'a [f64],
    /// Which named intents may be held now (menu long). Spare intents are always masked.
    pub intent_valid: &'a [bool],
    /// At most `K` candidate rows.
    pub candidates: &'a [CandidateRow<'a>],
    /// The fauna's early-end flag, with its reason.
    pub early_end: Option<EarlyEnd>,
}

/// A held or proposed subject: its opaque key and kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubjectRef {
    pub key: u64,
    pub kind: SubjectKind,
}

/// A held intent and its subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldChoice {
    pub intent: usize,
    pub subject: Option<SubjectRef>,
}

/// The top's last proposal, as probabilities at the fixed temperature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProposalRecord {
    pub intent: usize,
    pub subject: Option<SubjectRef>,
    /// `M'` long, masked entries 0.
    pub intent_probs: Vec<f64>,
    /// `K` rows then "none"; empty without candidates.
    pub subject_probs: Vec<f64>,
}

/// Everything a body's brain carries between samples: what a save or a matched training
/// start keeps so the brain continues rather than cold-starts (§11.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrainState {
    /// The GRU's hidden state.
    pub hidden: Vec<f64>,
    /// The held intent and subject; `None` before the first decision.
    pub held: Option<HeldChoice>,
    /// Seconds since the hold (intent or subject) last changed.
    pub hold_age_s: f64,
    /// Seconds in the current intent (resets only when the intent changes).
    pub intent_age_s: f64,
    /// The top's decision credit; a scheduled decision falls due at 1.
    pub decision_credit: f64,
    /// The last proposal.
    pub proposal: Option<ProposalRecord>,
}

impl BrainState {
    pub fn fresh(shape: &TwoLayerShape) -> BrainState {
        BrainState {
            hidden: vec![0.0; shape.hidden],
            held: None,
            hold_age_s: 0.0,
            intent_age_s: 0.0,
            decision_credit: 1.0,
            proposal: None,
        }
    }
}

/// The brain's answer at one sample.
#[derive(Clone, Debug, PartialEq)]
pub struct BrainAnswer {
    /// Today's continuous action logits (plus any legacy option rows), raw: the fauna's
    /// shared adapter transfers and deadbands them exactly as today.
    pub action_logits: Vec<f64>,
    /// The kick trigger, where the lineage has one.
    pub kick: Option<bool>,
    /// The call, where the lineage has a call head: `Some(None)` is silence,
    /// `Some(Some(k))` call kind `k`.
    pub call: Option<Option<usize>>,
    /// The rest request, where the lineage has one.
    pub rest: Option<bool>,
    /// The held intent after this sample's arbitration (0 on a plain brain).
    pub intent: usize,
    /// The held subject's row in this sample's candidates, after arbitration.
    pub subject: Option<usize>,
    /// The held subject's key.
    pub subject_ref: Option<SubjectRef>,
    /// Whether the top ran at this sample. Between decisions the proposal probabilities
    /// below are the last decision's.
    pub decided: bool,
    /// What changed the hold at this sample, if anything did.
    pub change: Option<HoldChange>,
    /// The top's proposal probabilities over intents (`M'` long), for logging.
    pub intent_probs: Vec<f64>,
    /// The pointer's probabilities for the proposed intent (`K` rows, then "none").
    pub subject_probs: Vec<f64>,
}

impl BrainAnswer {
    fn new(s: &TwoLayerShape) -> BrainAnswer {
        BrainAnswer {
            action_logits: vec![0.0; s.heads().passthrough.len()],
            kick: None,
            call: None,
            rest: None,
            intent: 0,
            subject: None,
            subject_ref: None,
            decided: false,
            change: None,
            intent_probs: vec![0.0; if s.has_top() { s.intents() } else { 0 }],
            subject_probs: vec![0.0; if s.candidates > 0 { s.candidates + 1 } else { 0 }],
        }
    }
}

/// One body's two-layer brain.
#[derive(Clone, Debug)]
pub struct TwoLayerBrain {
    net: Arc<TwoLayerNet>,
    arbitration: IntentArbitration,
    sample_period_s: f64,
    state: BrainState,
    x: Vec<f64>,
    c: Vec<f64>,
    next: Vec<f64>,
    y: Vec<f64>,
    valid: Vec<bool>,
    top: TopForward,
    q: Vec<f64>,
    g: Vec<f64>,
    row: Vec<f64>,
    scores: Vec<f64>,
    row_valid: Vec<bool>,
    all_valid: Vec<bool>,
    answer: BrainAnswer,
}

impl TwoLayerBrain {
    pub fn net(&self) -> &Arc<TwoLayerNet> {
        &self.net
    }
    pub fn arbitration(&self) -> &IntentArbitration {
        &self.arbitration
    }

    /// A copy of the brain's state, for a save.
    pub fn state(&self) -> BrainState {
        self.state.clone()
    }

    /// Continue from a saved state. Refuses a state of another shape by name.
    pub fn restore_state(&mut self, state: BrainState) -> Result<(), String> {
        let s = &self.net.shape;
        if state.hidden.len() != s.hidden {
            return Err(format!(
                "brain state has {} hidden values, the shape {}",
                state.hidden.len(),
                s.hidden
            ));
        }
        if let Some(h) = &state.held
            && (h.intent >= s.intents().max(1) || (h.subject.is_some() && s.candidates == 0))
        {
            return Err(format!("brain state holds intent {} outside the shape", h.intent));
        }
        if let Some(i) = state
            .hidden
            .iter()
            .chain([&state.hold_age_s, &state.intent_age_s, &state.decision_credit])
            .position(|x| !x.is_finite())
        {
            return Err(format!("brain state value {i} is not finite"));
        }
        if let Some(p) = &state.proposal
            && (p.intent_probs.len() != self.answer.intent_probs.len()
                || p.subject_probs.len() != self.answer.subject_probs.len())
        {
            return Err("brain state's proposal does not match the shape".into());
        }
        // The answer between decisions shows the last proposal: restore it with the rest.
        match &state.proposal {
            Some(p) => {
                self.answer.intent_probs.copy_from_slice(&p.intent_probs);
                self.answer.subject_probs.copy_from_slice(&p.subject_probs);
            }
            None => {
                self.answer.intent_probs.fill(0.0);
                self.answer.subject_probs.fill(0.0);
            }
        }
        self.state = state;
        Ok(())
    }

    /// Fresh memory: as a newly built brain.
    pub fn reset_state(&mut self) {
        self.state = BrainState::fresh(&self.net.shape);
        self.answer = BrainAnswer::new(&self.net.shape);
    }

    /// One controller sample, in the order of the module doc.
    pub fn step(&mut self, sample: &DecisionSample<'_>) -> &BrainAnswer {
        let net = Arc::clone(&self.net);
        let (s, theta) = (&net.shape, &net.theta[..]);
        assert!(sample.candidates.len() <= s.candidates, "more candidates than K");

        // 1. The held subject in this sample, and any early end.
        let has_top = s.has_top();
        self.valid.fill(false);
        if has_top {
            assert_eq!(sample.intent_valid.len(), s.menu, "intent mask is menu long");
            self.valid[..s.menu].copy_from_slice(sample.intent_valid);
            if !self.valid.iter().any(|v| *v) {
                // The fauna sends at least one valid intent; if it ever does not, the
                // first named intent stands in rather than nothing.
                self.valid[0] = true;
            }
        }
        let mut forced = sample.early_end.map(HoldChange::Told);
        let mut subject_row = None;
        if let Some(held) = self.state.held.filter(|_| has_top) {
            if !self.valid[held.intent] {
                forced.get_or_insert(HoldChange::IntentMasked);
            }
            if let Some(subject) = held.subject {
                match sample
                    .candidates
                    .iter()
                    .position(|c| c.key == subject.key && c.kind == subject.kind)
                {
                    None => {
                        forced.get_or_insert(HoldChange::SubjectGone);
                    }
                    Some(k) => {
                        if !sample.candidates[k].valid_for.get(held.intent).copied().unwrap_or(false)
                        {
                            forced.get_or_insert(HoldChange::SubjectInvalid);
                        }
                        subject_row = Some(k);
                    }
                }
            }
        }

        // 2. The GRU, conditioned on the hold from before this sample.
        let held_intent = self.state.held.map(|h| h.intent);
        let time = squash_time(
            self.state.intent_age_s,
            self.arbitration.intent_time_scale_s,
        );
        bottom_input(
            s,
            sample.observation,
            held_intent,
            subject_row.map(|k| (sample.candidates[k].features, sample.candidates[k].kind)),
            time,
            &mut self.x,
        );
        tensor::gru_view(theta, &net.layout.bottom).forward(
            &self.x,
            &mut self.state.hidden,
            &mut self.next,
            &mut self.y,
        );

        // 3–4. The top and arbitration, at a decision.
        let scheduled = self.state.decision_credit >= 1.0 - 1e-9;
        let due = has_top && (scheduled || forced.is_some() || self.state.held.is_none());
        self.answer.decided = due;
        self.answer.change = None;
        if due {
            if scheduled {
                self.state.decision_credit -= 1.0;
            }
            self.decide(sample, forced, subject_row);
        }

        // Subject's row in this sample, after arbitration.
        self.answer.intent = self.state.held.map_or(0, |h| h.intent);
        self.answer.subject_ref = self.state.held.and_then(|h| h.subject);
        self.answer.subject = self.answer.subject_ref.and_then(|r| {
            sample
                .candidates
                .iter()
                .position(|c| c.key == r.key && c.kind == r.kind)
        });

        // 5. The same-sample readout on the settled hold.
        if has_top {
            let time = squash_time(
                self.state.intent_age_s,
                self.arbitration.intent_time_scale_s,
            );
            conditioning(
                s,
                self.state.held.map(|h| h.intent),
                self.answer
                    .subject
                    .map(|k| (sample.candidates[k].features, sample.candidates[k].kind)),
                time,
                &mut self.c,
            );
            readout(&net.layout, theta, &self.c, &mut self.y);
        }
        let heads = s.heads();
        self.answer
            .action_logits
            .copy_from_slice(&self.y[heads.passthrough.clone()]);
        self.answer.kick = heads.kick.map(|k| self.y[k] > 0.0);
        self.answer.call = heads.call.clone().map(|r| {
            let calls = &self.y[r];
            let best = (1..calls.len()).fold(0, |b, k| if calls[k] > calls[b] { k } else { b });
            (best > 0).then(|| best - 1)
        });
        self.answer.rest = heads.rest.map(|k| self.y[k] > 0.0);

        // Time passes.
        let dt = self.sample_period_s;
        self.state.hold_age_s += dt;
        self.state.intent_age_s += dt;
        self.state.decision_credit =
            self.state.decision_credit.min(1.0) + dt * self.arbitration.intent_hz;
        &self.answer
    }

    fn decide(
        &mut self,
        sample: &DecisionSample<'_>,
        forced: Option<HoldChange>,
        subject_row: Option<usize>,
    ) {
        let net = Arc::clone(&self.net);
        let (s, theta) = (&net.shape, &net.theta[..]);
        let t = net.layout.top.expect("a brain with a top has top offsets");
        let cfg = self.arbitration;
        let held = self.state.held;
        let time = squash_time(self.state.intent_age_s, cfg.intent_time_scale_s);
        top_forward(
            s,
            &t,
            theta,
            &self.state.hidden,
            held.map(|h| h.intent),
            time,
            &mut self.top,
        );
        masked_softmax(
            &self.top.logits,
            &self.valid,
            cfg.temperature,
            &mut self.answer.intent_probs,
        );
        let intent = masked_argmax(&self.top.logits, &self.valid).unwrap_or(0);

        let mut subject = None;
        if s.candidates > 0 {
            pointer_query(s, &t, theta, &self.top.z, intent, &mut self.q);
            self.row_valid.fill(false);
            for (k, c) in sample.candidates.iter().enumerate() {
                if c.valid_for.get(intent).copied().unwrap_or(false) {
                    pointer_row(s, c.features, c.kind, &mut self.row);
                    self.scores[k] = pointer_score(s, &t, theta, &self.row, &self.q, &mut self.g);
                    self.row_valid[k] = true;
                } else {
                    self.scores[k] = 0.0;
                }
            }
            for k in sample.candidates.len()..s.candidates {
                self.scores[k] = 0.0;
            }
            self.scores[s.candidates] = none_score(&t, theta, intent);
            self.row_valid[s.candidates] = true;
            masked_softmax(
                &self.scores,
                &self.row_valid,
                cfg.temperature,
                &mut self.answer.subject_probs,
            );
            subject = masked_argmax(&self.scores, &self.row_valid).filter(|&k| k < s.candidates);
        }
        let subject_ref = subject.map(|k| SubjectRef {
            key: sample.candidates[k].key,
            kind: sample.candidates[k].kind,
        });
        // Reuse the last record's buffers: no allocation per decision.
        match &mut self.state.proposal {
            Some(p) => {
                p.intent = intent;
                p.subject = subject_ref;
                p.intent_probs.copy_from_slice(&self.answer.intent_probs);
                p.subject_probs.copy_from_slice(&self.answer.subject_probs);
            }
            None => {
                self.state.proposal = Some(ProposalRecord {
                    intent,
                    subject: subject_ref,
                    intent_probs: self.answer.intent_probs.clone(),
                    subject_probs: self.answer.subject_probs.clone(),
                })
            }
        }

        let view = ProposalView {
            intent_probs: &self.answer.intent_probs,
            intent,
            subject_probs: &self.answer.subject_probs,
            subject,
        };
        let hold = held.map(|h| HoldView {
            intent: h.intent,
            subject: subject_row,
            hold_age_s: self.state.hold_age_s,
        });
        if let Settle::Take(reason) = settle(&cfg, hold, forced, &view) {
            if held.is_none_or(|h| h.intent != intent) {
                self.state.intent_age_s = 0.0;
            }
            self.state.held = Some(HeldChoice {
                intent,
                subject: subject_ref,
            });
            self.state.hold_age_s = 0.0;
            self.answer.change = Some(reason);
        }
    }
}

impl TwoLayerBrain {
    /// The observation-only step: every named intent valid, no candidates, no early end.
    /// For a plain brain this is today's GRU step exactly.
    pub fn drive_plain(&mut self, observation: &[f64]) -> Response {
        let all = std::mem::take(&mut self.all_valid);
        self.step(&DecisionSample {
            observation,
            intent_valid: &all,
            candidates: &[],
            early_end: None,
        });
        self.all_valid = all;
        Response::logits(&self.answer.action_logits)
    }

    /// The held subject, if any.
    pub fn held_subject(&self) -> Option<SubjectRef> {
        self.state.held.and_then(|h| h.subject)
    }

    /// Re-mark the held subject to `subject` (the fauna re-marked it to another row's key,
    /// §11.1). Nothing without a held subject; the hold's age is kept.
    pub fn remark_subject(&mut self, subject: SubjectRef) {
        if let Some(h) = self.state.held.as_mut()
            && h.subject.is_some()
        {
            h.subject = Some(subject);
        }
    }
}
