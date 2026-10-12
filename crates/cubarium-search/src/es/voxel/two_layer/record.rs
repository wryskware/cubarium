//! **Teacher streams for cloning** (TWO-LAYER half 2, step 5; §10.2, §11.3 "Cloning data"):
//! the fauna's decision tap ([`cubarium_voxel_fauna::ActionTap::decisions`]) joined with its
//! executed-action tap into the core's [`TrajectoryRecord`]s, the shadow teacher beside a
//! brain-driven body, and the per-mode readouts the cloning report gives.
//!
//! - **Teacher records** ([`records_from_tap`]): per body, every decision sample's
//!   observation, the named part of its intent mask, all `K` candidate rows (keyed, an empty
//!   row valid for no intent, as the adapter hands them to the brain), the label **before
//!   the hold** (`proposal`, its subject row's key), what the teacher held after it, and the
//!   executed actions. Where a physical way out (DROP, WADE-OUT, the shallows) set the
//!   actions, the label is the controller's own answer: a brain's body takes those ways out
//!   by reflex, so the brain is not taught to imitate them.
//! - **Alternative-mode records**: stretches where a staged scene held the body to its own
//!   answer ([`cubarium_voxel_fauna::Fauna::force_own`]) become
//!   [`RecordSource::AlternativeMode`] records: no top label (`proposal: None`), the bottom
//!   conditioned on the forced base mode, the teacher's actions in it.
//! - **The shadow teacher** ([`ShadowTeacher`]): a brain drives the body; beside it the
//!   lineage's heuristic controller reads the same observation and its own answer is the
//!   action label for the state the learner reached ([`RecordSource::ClosedLoop`]). The
//!   brain's own held intent and subject are the conditioning. The heuristic **layer's**
//!   intent label (flight, roost, patrol: the step's overrides) runs only for heuristic
//!   bodies, so these records carry no top label.

use std::sync::{Arc, Mutex};

use cubarium_voxel_fauna::subjects::RowKind;
use cubarium_voxel_fauna::{
    ActionTap, BrainMemory, Controller, Decision, DecisionInput, Founder, Held, Manifest, Response,
    Steer, TapDecision, TapSample,
};
use serde::{Deserialize, Serialize};

use super::arbitration::{EarlyEnd, IntentArbitration};
use super::brain::{SubjectRef, TwoLayerBrain};
use super::clone::{
    RecordSource, RecordedCandidate, RecordedChoice, RecordedSample, TrajectoryRecord,
};
use super::net::{SubjectKind, readout, top_forward};
use super::shape::{TwoLayerLayout, TwoLayerShape};
use crate::es::voxel::imitate::held_actions;

fn early_end(e: cubarium_voxel_fauna::EarlyEnd) -> EarlyEnd {
    match e {
        cubarium_voxel_fauna::EarlyEnd::SubjectDead | cubarium_voxel_fauna::EarlyEnd::SubjectLost => {
            EarlyEnd::SubjectLost
        }
        cubarium_voxel_fauna::EarlyEnd::Masked => EarlyEnd::IntentMasked,
        cubarium_voxel_fauna::EarlyEnd::Emergency => EarlyEnd::Emergency,
    }
}

/// The fauna's candidate rows as the core records them (all `K`, in the fauna's order).
fn recorded_rows(input: &DecisionInput<'_>, named: usize) -> Vec<RecordedCandidate> {
    (0..input.rows())
        .map(|r| RecordedCandidate {
            key: input.keys[r],
            kind: if input.kinds[r] == RowKind::Place { SubjectKind::Place } else { SubjectKind::Body },
            features: input.candidates[r * input.row_width..(r + 1) * input.row_width].to_vec(),
            valid_for: if input.kinds[r] == RowKind::Empty {
                vec![false; named]
            } else {
                input.candidate_mask[r * input.menu..r * input.menu + named].to_vec()
            },
        })
        .collect()
}

fn choice(intent: usize, row: Option<usize>, rows: &[RecordedCandidate]) -> RecordedChoice {
    RecordedChoice {
        intent,
        subject: row.and_then(|r| rows.get(r)).map(|c| SubjectRef { key: c.key, kind: c.kind }),
    }
}

/// The action label of one tap sample: executed, unless a physical way out set it.
fn label(manifest: &Manifest, s: &TapSample) -> Vec<f64> {
    let held: &Held = match s.steer {
        Steer::Drop | Steer::Wade | Steer::Shallows => &s.own,
        _ => &s.executed,
    };
    held_actions(manifest, held)
}

/// One decision sample joined with its tap sample, as recorded.
fn sample(
    manifest: &Manifest,
    shape: &TwoLayerShape,
    d: &TapDecision,
    s: &TapSample,
    forced: bool,
) -> RecordedSample {
    let input = d.prep.input();
    let rows = recorded_rows(&input, shape.menu);
    let proposal = choice(usize::from(d.proposal), d.proposal_row, &rows);
    let held = if forced { proposal } else { choice(usize::from(d.held), d.held_row, &rows) };
    let discrete = |on: bool| on.then_some(());
    RecordedSample {
        observation: s.observation.clone(),
        intent_valid: d.prep.intent_mask[..shape.menu].to_vec(),
        early_end: d.prep.early_end.map(early_end),
        proposal: (!forced).then_some(proposal),
        held,
        actions: label(manifest, s),
        kick: discrete(shape.kick).map(|()| d.executed.kick),
        call: discrete(shape.call_kinds > 0)
            .map(|()| (d.executed.call > 0).then(|| usize::from(d.executed.call) - 1)),
        rest: discrete(shape.rest).map(|()| d.executed.rest == cubarium_voxel_fauna::RestAsk::Rest),
        candidates: rows,
    }
}

/// **The tap's teacher records for `founder`** under `shape`: per body in id order, its
/// decision samples joined by tick with its tap samples, cut into one record per stretch
/// of the same kind — [`RecordSource::Teacher`] where the body chose, and
/// [`RecordSource::AlternativeMode`] where `forced(id, tick)` held it to its own answer.
/// A stretch shorter than `min_samples` is dropped.
pub fn records_from_tap(
    tap: &ActionTap,
    founder: Founder,
    shape: &TwoLayerShape,
    forced: &dyn Fn(u64, u64) -> bool,
    min_samples: usize,
) -> Vec<(u64, TrajectoryRecord)> {
    let manifest = founder.manifest();
    let mut out = Vec::new();
    for (id, f, stream) in tap.streams() {
        if f != founder {
            continue;
        }
        let Some(decisions) = tap.decisions(id) else { continue };
        let by_tick: std::collections::HashMap<u64, &TapSample> =
            stream.iter().map(|s| (s.tick, s)).collect();
        let mut current: Option<(bool, Vec<RecordedSample>)> = None;
        let flush = |cur: Option<(bool, Vec<RecordedSample>)>, out: &mut Vec<(u64, TrajectoryRecord)>| {
            if let Some((was_forced, samples)) = cur
                && samples.len() >= min_samples.max(1)
            {
                out.push((
                    id,
                    TrajectoryRecord {
                        source: if was_forced { RecordSource::AlternativeMode } else { RecordSource::Teacher },
                        sample_period_s: manifest.controller_period_s,
                        samples,
                    },
                ));
            }
        };
        for d in decisions {
            let Some(s) = by_tick.get(&d.tick) else { continue };
            if s.observation.len() != shape.observation {
                continue;
            }
            let is_forced = forced(id, d.tick);
            if current.as_ref().is_some_and(|(k, _)| *k != is_forced) {
                flush(current.take(), &mut out);
            }
            current
                .get_or_insert_with(|| (is_forced, Vec::new()))
                .1
                .push(sample(&manifest, shape, d, s, is_forced));
        }
        flush(current, &mut out);
    }
    out
}

/// **The shadow teacher** (§11.3's closed-loop correction): the brain drives; the lineage's
/// heuristic controller reads the same observation beside it and its own answer labels the
/// state the learner reached. The teacher's answer never reaches the body.
pub struct ShadowTeacher {
    brain: TwoLayerBrain,
    teacher: Box<dyn Controller>,
    manifest: Manifest,
    named: usize,
    log: Arc<Mutex<Vec<RecordedSample>>>,
}

impl ShadowTeacher {
    /// A shadow over `brain` with `teacher` (the lineage's heuristic), logging into `log`.
    pub fn new(
        brain: TwoLayerBrain,
        teacher: Box<dyn Controller>,
        founder: Founder,
        log: Arc<Mutex<Vec<RecordedSample>>>,
    ) -> ShadowTeacher {
        let named = brain.net().shape().menu;
        ShadowTeacher { brain, teacher, manifest: founder.manifest(), named, log }
    }
}

impl Controller for ShadowTeacher {
    fn drive(&mut self, o: &[f64]) -> Response {
        let _ = self.teacher.drive(o);
        self.brain.drive(o)
    }
    fn reset(&mut self) {
        self.brain.reset();
        self.teacher.reset();
    }
    fn decide(&mut self, o: &[f64], input: &DecisionInput<'_>) -> (Response, Option<Decision>) {
        let (r, d) = self.brain.decide(o, input);
        let taught = cubarium_voxel_fauna::resolve(self.teacher.drive(o), &self.manifest);
        if let Some(d) = d {
            let rows = recorded_rows(input, self.named);
            let held = choice(d.intent, d.subject, &rows);
            self.log.lock().expect("shadow log").push(RecordedSample {
                observation: o.to_vec(),
                intent_valid: input.intent_mask[..self.named].to_vec(),
                early_end: input.early_end.map(early_end),
                proposal: None,
                held,
                actions: held_actions(&self.manifest, &Held { actions: taught.actions, body: taught.body }),
                kick: None,
                call: None,
                rest: None,
                candidates: rows,
            });
        }
        (r, d)
    }
    fn memory(&self) -> Option<BrainMemory> {
        self.brain.memory()
    }
    fn restore(&mut self, m: &BrainMemory) -> bool {
        self.brain.restore(m)
    }
}

/// Per-mode readouts on held-out records, teacher-forced in the runtime's order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PerMode {
    /// Per named intent: top labels with that intent, and how many the top's masked argmax
    /// matched.
    pub intent_labels: Vec<usize>,
    pub intent_hits: Vec<usize>,
    /// Per held intent (the mode the bottom acts in): samples and summed squared action
    /// error per action value.
    pub mode_samples: Vec<usize>,
    pub mode_se: Vec<f64>,
}

impl PerMode {
    /// `(agreement per intent, mean squared action error per mode)`, `NaN` where empty.
    pub fn rates(&self, actions: usize) -> (Vec<f64>, Vec<f64>) {
        let agree = self
            .intent_labels
            .iter()
            .zip(&self.intent_hits)
            .map(|(n, h)| if *n > 0 { *h as f64 / *n as f64 } else { f64::NAN })
            .collect();
        let err = self
            .mode_samples
            .iter()
            .zip(&self.mode_se)
            .map(|(n, se)| if *n > 0 { se / (*n * actions) as f64 } else { f64::NAN })
            .collect();
        (agree, err)
    }
}

/// **Per-intent agreement and per-mode action error** of `theta` over `records`,
/// teacher-forced exactly as cloning runs: the GRU on the hold before each sample, the top
/// against the proposal, the readout on the hold at the sample. `menu` sizes the tables
/// (a plain shape is scored per the records' held intents, its top absent).
pub fn per_mode(
    shape: &TwoLayerShape,
    arbitration: &IntentArbitration,
    theta: &[f64],
    records: &[TrajectoryRecord],
    menu: usize,
) -> PerMode {
    let layout = TwoLayerLayout::new(shape);
    let mut m = PerMode {
        intent_labels: vec![0; menu],
        intent_hits: vec![0; menu],
        mode_samples: vec![0; menu],
        mode_se: vec![0.0; menu],
    };
    let heads = shape.heads();
    for record in records {
        let xs = super::clone::bottom_inputs(shape, arbitration, record);
        let cs = super::clone::readout_conditioning(shape, arbitration, record);
        let mut hidden = vec![0.0; shape.hidden];
        let mut next = vec![0.0; shape.hidden];
        let mut y = vec![0.0; shape.bottom_outputs()];
        let mut top = super::net::TopForward::new(shape);
        let mut before: Option<usize> = None;
        let mut age = 0.0;
        for (t, s) in record.samples.iter().enumerate() {
            crate::es::tensor::gru_view(theta, &layout.bottom).forward(&xs[t], &mut hidden, &mut next, &mut y);
            if let (Some(to), Some(p)) = (layout.top, s.proposal) {
                let time = super::net::squash_time(age, arbitration.intent_time_scale_s);
                top_forward(shape, &to, theta, &hidden, before, time, &mut top);
                let mut valid = vec![false; shape.intents()];
                valid[..shape.menu].copy_from_slice(&s.intent_valid);
                if valid.get(p.intent).copied().unwrap_or(false) && p.intent < menu {
                    m.intent_labels[p.intent] += 1;
                    if super::net::masked_argmax(&top.logits, &valid) == Some(p.intent) {
                        m.intent_hits[p.intent] += 1;
                    }
                }
            }
            readout(&layout, theta, &cs[t], &mut y);
            let mode = s.held.intent.min(menu.saturating_sub(1));
            m.mode_samples[mode] += 1;
            for (a, tr) in heads.continuous.clone().zip(&shape.actions) {
                let e = tr.apply(y[a]) - s.actions[a];
                m.mode_se[mode] += e * e;
            }
            if before != Some(s.held.intent) {
                age = 0.0;
            }
            age += record.sample_period_s;
            before = Some(s.held.intent);
        }
    }
    m
}
