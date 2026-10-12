//! The TWO-LAYER core's checks: mechanisms on hand-built fixtures, never a tuned number.

use std::sync::Arc;

use cubarium_voxel_fauna::{Controller, Founder, TeacherConfig};

use super::arbitration::{
    EarlyEnd, HoldChange, HoldView, IntentArbitration, ProposalView, Settle, registry, settle,
};
use super::brain::{CandidateRow, DecisionSample, SubjectRef, TwoLayerNet};
use super::clone::{
    self, CloneSpec, RecordSource, RecordedCandidate, RecordedChoice, RecordedSample,
    TrajectoryRecord,
};
use super::file::{BrainFile, TwoLayerPolicyFile, plain_net};
use super::net::{SubjectKind, masked_softmax};
use super::shape::{ActionTransfer, Layer, TwoLayerLayout, TwoLayerShape, initial_center};
use crate::es::tensor;
use crate::es::voxel::store::{test_centre, test_centre_path};

/// A small deterministic stream for fixtures.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo)
    }
}

/// A brain with every part: a menu, spares, subjects and all three discrete heads.
fn full_shape() -> TwoLayerShape {
    TwoLayerShape {
        observation: 5,
        hidden: 6,
        actions: vec![ActionTransfer::Sigmoid, ActionTransfer::Tanh],
        legacy_options: 0,
        menu: 3,
        spare: 2,
        candidates: 3,
        row_width: 2,
        kick: true,
        call_kinds: 2,
        rest: true,
        top_hidden: 5,
        pointer_width: 4,
    }
}

fn test_arbitration(hold_s: f64, switch_margin: f64) -> IntentArbitration {
    IntentArbitration {
        intent_hz: 4.0,
        hold_s,
        switch_margin,
        temperature: 1.0,
        intent_time_scale_s: 5.0,
    }
}

// ---------------------------------------------------------------------------------------
// A plain brain is today's controller.
// ---------------------------------------------------------------------------------------

/// Every founder's plain policy file, loaded unchanged, drives identically to today's
/// runtime GRU over a fixed observation sequence — bit for bit, through the same seam.
#[test]
fn a_plain_policy_file_gives_todays_outputs_exactly() {
    for founder in Founder::ALL {
        let path = test_centre_path(founder, "two-layer-plain");
        let file = match BrainFile::load(&path).expect("a plain file loads") {
            BrainFile::Plain(f) => f,
            BrainFile::TwoLayer(_) => panic!("a plain file is not a two-layer file"),
        };
        let manifest = founder.manifest();
        let net = plain_net(&file).expect("plain net");
        assert!(net.shape().is_plain());
        assert_eq!(net.theta(), &file.theta[..], "the vector is the file's, unchanged");
        assert_eq!(net.layout().range(Layer::Top).len(), 0, "no top");

        let mut today = file.driver().expect("driver").fresh(TeacherConfig::default());
        let mut brain = net.brain(IntentArbitration::default(), manifest.controller_period_s);
        let mut rng = Rng(0x5eed ^ founder.index() as u64);
        for t in 0..64 {
            let obs: Vec<f64> = (0..manifest.inputs()).map(|_| rng.unit()).collect();
            assert_eq!(
                today.drive(&obs),
                brain.drive(&obs),
                "{} sample {t}: the plain brain differs from today's",
                founder.name()
            );
        }
        // And after a reset, both start over identically.
        today.reset();
        brain.reset();
        let obs = vec![0.3; manifest.inputs()];
        assert_eq!(today.drive(&obs), brain.drive(&obs));
        let _ = std::fs::remove_file(path);
    }
}

/// A plain shape starts where today's GRU starts.
#[test]
fn a_plain_shape_initialises_as_todays_gru() {
    let m = Founder::Browser.manifest();
    let shape = TwoLayerShape::plain_from_manifest(&m);
    assert_eq!(
        initial_center(&shape, 9),
        tensor::initial_center_runtime(m.inputs(), m.outputs(), m.hidden, 9)
    );
    assert_eq!(TwoLayerLayout::new(&shape).params, m.parameter_count());
}

// ---------------------------------------------------------------------------------------
// ES layout.
// ---------------------------------------------------------------------------------------

/// The two layers partition the vector, the named tensors tile it in order, and a
/// top-only restriction leaves the bottom untouched.
#[test]
fn the_layout_names_top_and_bottom_and_restricts_to_either() {
    let shape = full_shape();
    let layout = TwoLayerLayout::new(&shape);
    let (bottom, top) = (layout.range(Layer::Bottom), layout.range(Layer::Top));
    assert_eq!(bottom.start, 0);
    assert_eq!(bottom.end, top.start);
    assert_eq!(top.end, layout.params);
    assert!(!top.is_empty());
    let mut at = 0;
    for (name, layer, r) in layout.named_ranges() {
        assert_eq!(r.start, at, "{name} starts where the last ended");
        assert!(layout.range(layer).contains(&r.start), "{name} is in its layer");
        at = r.end;
    }
    assert_eq!(at, layout.params);

    let mut eps = vec![1.0; layout.params];
    layout.restrict(Layer::Top, &mut eps);
    assert!(eps[bottom.clone()].iter().all(|v| *v == 0.0));
    assert!(eps[top.clone()].iter().all(|v| *v == 1.0));
    let mask = layout.mask(Layer::Bottom);
    assert_eq!(mask.iter().filter(|m| **m).count(), bottom.len());

    // Perturbing only the top changes the brain's choice, and leaves a fresh brain's
    // actions at its first sample alone: the GRU has no hold to read yet, and the
    // same-sample readout starts at zero.
    let theta = initial_center(&shape, 3);
    let mut moved = theta.clone();
    for i in top {
        moved[i] += 0.5 * ((i % 7) as f64 - 3.0);
    }
    let a = Arc::new(TwoLayerNet::new(shape.clone(), theta).unwrap());
    let b = Arc::new(TwoLayerNet::new(shape.clone(), moved).unwrap());
    let cfg = test_arbitration(0.0, 0.0);
    let (mut ba, mut bb) = (a.brain(cfg, 0.1), b.brain(cfg, 0.1));
    let mask = [true; 3];
    let valid = [true, true, true];
    let feats = [[0.2, 0.9], [0.7, 0.1], [0.5, 0.5]];
    let rows: Vec<_> = (0..3)
        .map(|k| CandidateRow {
            key: k as u64,
            kind: SubjectKind::Body,
            features: &feats[k],
            valid_for: &valid,
        })
        .collect();
    let s = DecisionSample {
        observation: &[0.1, 0.2, 0.3, 0.4, 0.5],
        intent_valid: &mask,
        candidates: &rows,
        early_end: None,
    };
    let (x, y) = (ba.step(&s).clone(), bb.step(&s).clone());
    assert_eq!(x.action_logits, y.action_logits, "the bottom is the same");
    assert_ne!(x.intent_probs, y.intent_probs, "the top moved");
}

// ---------------------------------------------------------------------------------------
// Arbitration.
// ---------------------------------------------------------------------------------------

fn view<'a>(probs: &'a [f64], subject_probs: &'a [f64]) -> ProposalView<'a> {
    let intent = (0..probs.len())
        .max_by(|a, b| probs[*a].total_cmp(&probs[*b]))
        .unwrap();
    let subject = if subject_probs.is_empty() {
        None
    } else {
        let k = (0..subject_probs.len())
            .max_by(|a, b| subject_probs[*a].total_cmp(&subject_probs[*b]))
            .unwrap();
        (k + 1 < subject_probs.len()).then_some(k)
    };
    ProposalView {
        intent_probs: probs,
        intent,
        subject_probs,
        subject,
    }
}

/// A proposal that beats the held intent by less than the margin does not switch; one
/// that beats it by more does. Read against the configured margin, whatever it is.
#[test]
fn a_proposal_below_the_margin_does_not_switch() {
    let cfg = test_arbitration(0.5, 0.2);
    let hold = Some(HoldView {
        intent: 0,
        subject: None,
        hold_age_s: cfg.hold_s * 2.0,
    });
    let held_p = 0.3;
    let near = [held_p, held_p + cfg.switch_margin * 0.5, 0.0];
    let p = ProposalView {
        intent: 1,
        ..view(&near, &[])
    };
    assert_eq!(settle(&cfg, hold, None, &p), Settle::Keep);
    let far = [held_p, held_p + cfg.switch_margin * 1.5, 0.0];
    let p = ProposalView {
        intent: 1,
        ..view(&far, &[])
    };
    assert_eq!(settle(&cfg, hold, None, &p), Settle::Take(HoldChange::Outscored));

    // The same for a subject under the held intent.
    let hold = Some(HoldView {
        intent: 0,
        subject: Some(0),
        hold_age_s: cfg.hold_s,
    });
    let probs = [0.9, 0.1, 0.0];
    let near = [0.3, 0.3 + cfg.switch_margin * 0.5, 0.0, 0.4 - cfg.switch_margin * 0.5];
    let p = ProposalView {
        subject: Some(1),
        ..view(&probs, &near)
    };
    assert_eq!(settle(&cfg, hold, None, &p), Settle::Keep);
    let far = [0.2, 0.2 + cfg.switch_margin * 1.5, 0.0, 0.6 - cfg.switch_margin * 1.5];
    let p = ProposalView {
        subject: Some(1),
        ..view(&probs, &far)
    };
    assert_eq!(settle(&cfg, hold, None, &p), Settle::Take(HoldChange::SubjectOutscored));
}

/// Nothing switches on the top's own account before `hold_s`, however strong the
/// proposal; at `hold_s` it may. And through a running brain whose top flips its
/// preference every sample, holds last at least `hold_s` between own-account switches.
#[test]
fn nothing_switches_before_hold_s() {
    let cfg = test_arbitration(0.7, 0.05);
    let strong = [0.0, 1.0, 0.0];
    let p = ProposalView {
        intent: 1,
        ..view(&strong, &[])
    };
    let young = HoldView {
        intent: 0,
        subject: None,
        hold_age_s: cfg.hold_s * 0.99,
    };
    assert_eq!(settle(&cfg, Some(young), None, &p), Settle::Keep);
    let grown = HoldView {
        hold_age_s: cfg.hold_s,
        ..young
    };
    assert_eq!(settle(&cfg, Some(grown), None, &p), Settle::Take(HoldChange::Outscored));

    // A running brain: the top reads only the first observation channel, which flips.
    let shape = TwoLayerShape {
        candidates: 0,
        row_width: 0,
        pointer_width: 0,
        ..full_shape()
    };
    let net = flip_net(&shape);
    let period = 0.1;
    let cfg = IntentArbitration {
        intent_hz: 1.0 / period,
        ..cfg
    };
    let mut brain = net.brain(cfg, period);
    let mask = [true; 3];
    let mut last_switch: Option<usize> = None;
    let mut switches = 0;
    for t in 0..80 {
        let mut obs = [0.0; 5];
        obs[0] = if (t / 2) % 2 == 0 { 1.0 } else { -1.0 };
        let a = brain.step(&DecisionSample {
            observation: &obs,
            intent_valid: &mask,
            candidates: &[],
            early_end: None,
        });
        if a.change == Some(HoldChange::Outscored) {
            if let Some(prev) = last_switch {
                let held = (t - prev) as f64 * period;
                assert!(held + 1e-9 >= cfg.hold_s, "switched after {held} s < hold_s");
            }
            last_switch = Some(t);
            switches += 1;
        }
    }
    assert!(switches > 2, "the fixture does switch ({switches})");
}

/// A net whose top prefers intent 0 when observation channel 0 is positive and intent 1
/// when negative, strongly: every weight zero but the path obs[0] → h → top.
fn flip_net(shape: &TwoLayerShape) -> Arc<TwoLayerNet> {
    let layout = TwoLayerLayout::new(shape);
    let mut theta = vec![0.0; layout.params];
    let b = layout.bottom;
    let (i, h) = (shape.bottom_inputs(), shape.hidden);
    // Candidate gate n of unit 0 reads obs[0]; z gate bias strongly negative so h follows.
    theta[b.w_i + 2 * h * i] = 3.0;
    theta[b.b_i + h] = -20.0;
    let t = layout.top.unwrap();
    theta[t.w1] = 3.0; // z1[0] reads h[0]
    theta[t.w2] = 10.0; // intent 0 ← z1[0]
    theta[t.w2 + shape.top_hidden] = -10.0; // intent 1 ← −z1[0]
    Arc::new(TwoLayerNet::new(shape.clone(), theta).unwrap())
}

/// An early end forces a fresh choice at once: at age 0, between scheduled decisions,
/// and below the margin.
#[test]
fn an_early_end_forces_a_fresh_choice() {
    let cfg = test_arbitration(100.0, 0.9);
    let probs = [0.5, 0.5, 0.0];
    let p = ProposalView {
        intent: 1,
        ..view(&probs, &[])
    };
    let hold = Some(HoldView {
        intent: 0,
        subject: None,
        hold_age_s: 0.0,
    });
    for reason in [EarlyEnd::SubjectLost, EarlyEnd::IntentMasked, EarlyEnd::Emergency] {
        assert_eq!(
            settle(&cfg, hold, Some(HoldChange::Told(reason)), &p),
            Settle::Take(HoldChange::Told(reason))
        );
    }

    // Through a brain: a long hold, a slow top; the flag decides at the very next sample.
    let shape = full_shape();
    let net = Arc::new(TwoLayerNet::new(shape.clone(), initial_center(&shape, 4)).unwrap());
    let mut brain = net.brain(IntentArbitration { intent_hz: 0.01, ..cfg }, 0.1);
    let mask = [true; 3];
    let obs = [0.2; 5];
    let mut sample = DecisionSample {
        observation: &obs,
        intent_valid: &mask,
        candidates: &[],
        early_end: None,
    };
    assert_eq!(brain.step(&sample).change, Some(HoldChange::Start));
    for _ in 0..5 {
        assert!(!brain.step(&sample).decided, "no scheduled decision yet");
    }
    sample.early_end = Some(EarlyEnd::Emergency);
    let a = brain.step(&sample);
    assert!(a.decided);
    assert_eq!(a.change, Some(HoldChange::Told(EarlyEnd::Emergency)));
}

/// Over random masks and random weights, the held intent is never a masked one, and a
/// held subject is never a row invalid for the held intent.
#[test]
fn a_masked_intent_or_invalid_row_is_never_held() {
    let shape = full_shape();
    let mut rng = Rng(77);
    for seed in 0..6 {
        let net =
            Arc::new(TwoLayerNet::new(shape.clone(), initial_center(&shape, seed)).unwrap());
        let mut brain = net.brain(test_arbitration(0.2, 0.05), 0.1);
        for _ in 0..200 {
            let mask: Vec<bool> = (0..shape.menu).map(|_| rng.unit() < 0.6).collect();
            let feats: Vec<[f64; 2]> = (0..3).map(|_| [rng.unit(), rng.unit()]).collect();
            let valid: Vec<Vec<bool>> = (0..3)
                .map(|_| (0..shape.menu).map(|_| rng.unit() < 0.5).collect())
                .collect();
            let n = rng.range(0, 4) as usize;
            let rows: Vec<_> = (0..n)
                .map(|k| CandidateRow {
                    key: k as u64,
                    kind: SubjectKind::Body,
                    features: &feats[k],
                    valid_for: &valid[k],
                })
                .collect();
            let obs: Vec<f64> = (0..5).map(|_| rng.unit()).collect();
            let a = brain.step(&DecisionSample {
                observation: &obs,
                intent_valid: &mask,
                candidates: &rows,
                early_end: None,
            });
            if mask.iter().any(|m| *m) {
                assert!(mask[a.intent], "held a masked intent");
            } else {
                assert_eq!(a.intent, 0, "an empty mask falls back to the first intent");
            }
            if let Some(k) = a.subject {
                assert!(valid[k][a.intent], "held a subject from an invalid row");
            }
            assert!(
                a.subject_ref.is_none() || a.subject.is_some(),
                "a held subject is always one of this sample's rows"
            );
        }
    }
}

/// The margin is read on probabilities at the fixed temperature, so the logit scale
/// matters only through them: adding a constant to every logit changes nothing, and a
/// logit scale matched by the temperature changes nothing. (The mechanism: [`settle`]
/// takes probabilities and never sees a logit.)
#[test]
fn a_logit_scale_change_does_not_switch_on_its_own_account() {
    let cfg = test_arbitration(0.0, 0.15);
    let valid = [true, true, true];
    let hold = Some(HoldView {
        intent: 0,
        subject: None,
        hold_age_s: 1.0,
    });
    let base = [0.4, 0.9, -0.3];
    let decide = |logits: &[f64], temperature: f64| {
        let mut p = [0.0; 3];
        masked_softmax(logits, &valid, temperature, &mut p);
        let v = view(&p, &[]);
        (settle(&cfg, hold, None, &v), p)
    };
    let (d0, p0) = decide(&base, cfg.temperature);
    for shift in [-50.0, -1.0, 3.0, 400.0] {
        let shifted: Vec<f64> = base.iter().map(|l| l + shift).collect();
        let (d, p) = decide(&shifted, cfg.temperature);
        assert_eq!(d, d0, "a common shift moved the decision");
        for (a, b) in p.iter().zip(&p0) {
            assert!((a - b).abs() < 1e-12);
        }
    }
    for scale in [0.1, 3.0, 40.0] {
        let scaled: Vec<f64> = base.iter().map(|l| l * scale).collect();
        let (d, _) = decide(&scaled, cfg.temperature * scale);
        assert_eq!(d, d0, "a scale matched by the temperature moved the decision");
    }
}

// ---------------------------------------------------------------------------------------
// Order and saves.
// ---------------------------------------------------------------------------------------

fn record_from(samples: Vec<RecordedSample>) -> TrajectoryRecord {
    TrajectoryRecord {
        source: RecordSource::Teacher,
        sample_period_s: 0.1,
        samples,
    }
}

fn fixture_sample(rng: &mut Rng, shape: &TwoLayerShape, intent: usize, proposal: usize) -> RecordedSample {
    let candidates = (0..shape.candidates)
        .map(|k| RecordedCandidate {
            key: 10 + k as u64,
            kind: if k == 0 { SubjectKind::Place } else { SubjectKind::Body },
            features: (0..shape.row_width).map(|_| rng.unit()).collect(),
            valid_for: vec![true; shape.menu],
        })
        .collect();
    let subject = |k: u64| {
        Some(SubjectRef {
            key: 10 + k,
            kind: if k == 0 { SubjectKind::Place } else { SubjectKind::Body },
        })
    };
    RecordedSample {
        observation: (0..shape.observation).map(|_| rng.unit()).collect(),
        intent_valid: vec![true; shape.menu],
        candidates,
        early_end: None,
        proposal: Some(RecordedChoice {
            intent: proposal,
            subject: subject(proposal as u64 % 3),
        }),
        held: RecordedChoice {
            intent,
            subject: subject(intent as u64 % 3),
        },
        actions: (0..shape.actions.len()).map(|_| rng.unit()).collect(),
        kick: Some(rng.unit() < 0.5),
        call: Some(if rng.unit() < 0.5 { None } else { Some(1) }),
        rest: Some(rng.unit() < 0.5),
    }
}

/// **The order** (§11.3, amended 2026-10-12), with the teacher's hold fed as conditioning:
/// the top's label at sample t (its proposal, and the hold that follows from it) never
/// reaches the **top's** input at t, nor the GRU's update at t. It reaches the bottom's
/// action readout at t (the same-sample readout) and the GRU's update from t + 1.
#[test]
fn the_label_at_t_never_reaches_the_tops_input_at_t() {
    let shape = full_shape();
    let cfg = test_arbitration(0.5, 0.1);
    let theta = initial_center(&shape, 4);
    let mut rng = Rng(5);
    let samples: Vec<_> = (0..8)
        .map(|t| fixture_sample(&mut rng, &shape, t % 2, (t + 1) % 3))
        .collect();
    let rec = record_from(samples.clone());
    let (base_x, base_u, base_c) = (
        clone::bottom_inputs(&shape, &cfg, &rec),
        clone::top_inputs(&shape, &cfg, &theta, &rec),
        clone::readout_conditioning(&shape, &cfg, &rec),
    );
    for t in 0..7 {
        let mut changed = samples.clone();
        let other = (samples[t].held.intent + 1) % shape.menu;
        changed[t].proposal = Some(RecordedChoice {
            intent: other,
            subject: None,
        });
        changed[t].held = RecordedChoice {
            intent: other,
            subject: None,
        };
        let rec = record_from(changed);
        let (x, u, c) = (
            clone::bottom_inputs(&shape, &cfg, &rec),
            clone::top_inputs(&shape, &cfg, &theta, &rec),
            clone::readout_conditioning(&shape, &cfg, &rec),
        );
        for s in 0..=t {
            assert_eq!(u[s], base_u[s], "sample {t}'s label reached the top's input at {s}");
            assert_eq!(x[s], base_x[s], "sample {t}'s label reached the GRU's update at {s}");
        }
        assert_ne!(c[t], base_c[t], "the settled hold conditions the same sample's readout");
        assert_ne!(x[t + 1], base_x[t + 1], "the hold conditions the next sample's update");
        assert_ne!(u[t + 1], base_u[t + 1], "and so the next sample's top");
    }
    // The first sample's update and top read no hold at all.
    let block = shape.observation..shape.observation + shape.intent_block();
    assert!(base_x[0][block].iter().all(|v| *v == 0.0));
    let h = shape.hidden;
    assert!(base_u[0][h..h + shape.intents()].iter().all(|v| *v == 0.0));
}

/// **The same-sample readout at runtime**: at the sample the hold switches, the actions
/// already read the newly settled intent (a readout weight on that intent moves them at
/// once), while the hidden state the next decision reads is the same as without that
/// weight: the readout never feeds back into the recurrent update.
#[test]
fn a_switch_moves_the_actions_in_the_same_sample_and_not_the_state() {
    let shape = TwoLayerShape {
        candidates: 0,
        row_width: 0,
        pointer_width: 0,
        ..full_shape()
    };
    let layout = TwoLayerLayout::new(&shape);
    let theta = initial_center(&shape, 21);
    // A readout weight on intent 1 for action 0 only.
    let mut lifted = theta.clone();
    let cw = shape.conditioning();
    lifted[layout.readout.start + 1] = 3.0;
    let (a, b) = (
        Arc::new(TwoLayerNet::new(shape.clone(), theta).unwrap()),
        Arc::new(TwoLayerNet::new(shape.clone(), lifted).unwrap()),
    );
    let cfg = test_arbitration(0.0, 0.0);
    let (mut ba, mut bb) = (a.brain(cfg, 0.1), b.brain(cfg, 0.1));
    let mut rng = Rng(77);
    let mut saw_intent_one = false;
    for _ in 0..30 {
        // Mask everything but one intent per sample, so the hold changes as the mask does.
        let pick = (rng.next() % 3) as usize;
        let mask: Vec<bool> = (0..shape.menu).map(|k| k == pick).collect();
        let obs: Vec<f64> = (0..shape.observation).map(|_| rng.unit()).collect();
        let s = DecisionSample {
            observation: &obs,
            intent_valid: &mask,
            candidates: &[],
            early_end: None,
        };
        let (x, y) = (ba.step(&s).clone(), bb.step(&s).clone());
        assert_eq!(x.intent, pick, "a lone valid intent is held at once");
        assert_eq!(y.intent, pick);
        assert_eq!(ba.state().hidden, bb.state().hidden, "the readout never reaches the state");
        let d = y.action_logits[0] - x.action_logits[0];
        if pick == 1 {
            saw_intent_one = true;
            assert!((d - 3.0).abs() < 1e-12, "intent 1 settled at this sample moves action 0 now");
        } else {
            assert_eq!(d, 0.0);
        }
        assert_eq!(x.action_logits[1], y.action_logits[1]);
    }
    assert!(saw_intent_one && cw > 0);
}

/// Serialise a brain mid-hold, restore it into a fresh one, and the next outputs are
/// identical to the original's.
#[test]
fn a_restored_brain_continues_identically() {
    let shape = full_shape();
    let net = Arc::new(TwoLayerNet::new(shape.clone(), initial_center(&shape, 12)).unwrap());
    let cfg = test_arbitration(0.6, 0.05);
    let mut rng = Rng(9);
    let samples: Vec<_> = (0..40).map(|_| fixture_sample(&mut rng, &shape, 0, 0)).collect();
    let run = |brain: &mut super::TwoLayerBrain, s: &RecordedSample| {
        let rows: Vec<_> = s
            .candidates
            .iter()
            .map(|c| CandidateRow {
                key: c.key,
                kind: c.kind,
                features: &c.features,
                valid_for: &c.valid_for,
            })
            .collect();
        brain
            .step(&DecisionSample {
                observation: &s.observation,
                intent_valid: &s.intent_valid,
                candidates: &rows,
                early_end: None,
            })
            .clone()
    };
    let mut original = net.brain(cfg, 0.1);
    for s in &samples[..17] {
        run(&mut original, s);
    }
    let state = original.state();
    assert!(state.held.is_some() && state.hold_age_s > 0.0, "mid-hold");
    let text = serde_json::to_string(&state).expect("serialise");
    let mut restored = net.brain(cfg, 0.1);
    restored
        .restore_state(serde_json::from_str(&text).expect("deserialise"))
        .expect("restore");
    for s in &samples[17..] {
        assert_eq!(run(&mut original, s), run(&mut restored, s));
    }
}

// ---------------------------------------------------------------------------------------
// Files and knobs.
// ---------------------------------------------------------------------------------------

/// A two-layer file round-trips exactly and loads by its token; a plain shape, a wrong
/// length and a bottom not matching the founder's manifest are refused by name.
#[test]
fn a_two_layer_file_round_trips_and_refuses_by_name() {
    let founder = Founder::Loftstrider;
    let m = founder.manifest();
    let shape = TwoLayerShape {
        menu: 4,
        spare: 2,
        candidates: 3,
        row_width: 5,
        kick: true,
        call_kinds: 2,
        rest: true,
        top_hidden: 8,
        pointer_width: 6,
        ..TwoLayerShape::plain_from_manifest(&m)
    };
    let net = TwoLayerNet::new(shape.clone(), initial_center(&shape, 2)).unwrap();
    let file = TwoLayerPolicyFile::new(founder, &net, 7, "test");
    let path = std::env::temp_dir().join(format!("two-layer-{}.json", std::process::id()));
    file.write(&path).unwrap();
    match BrainFile::load(&path).expect("loads") {
        BrainFile::TwoLayer(f) => assert_eq!(f, file),
        BrainFile::Plain(_) => panic!("wrong format"),
    }
    let _ = std::fs::remove_file(&path);

    let mut plain = file.clone();
    plain.shape = TwoLayerShape::plain_from_manifest(&m);
    plain.theta = test_centre(founder, 1).theta;
    assert!(plain.validate("p").unwrap_err().contains("plain brain keeps"));
    let mut short = file.clone();
    short.theta.pop();
    assert!(short.validate("p").unwrap_err().contains("theta"));
    let mut wrong = file.clone();
    wrong.shape.observation += 1;
    assert!(wrong.validate("p").unwrap_err().contains("manifest"));
}

/// The holding knobs have registry rows with ranges, and the defaults sit inside them.
#[test]
fn the_holding_knobs_have_registry_rows() {
    registry().check(&IntentArbitration::default()).expect("defaults in range");
    let bad = IntentArbitration {
        switch_margin: 1.5,
        ..IntentArbitration::default()
    };
    assert!(bad.validate().is_err());
}

// ---------------------------------------------------------------------------------------
// Cloning.
// ---------------------------------------------------------------------------------------

/// The analytic gradient of the clone objective (every loss term, the top flowing into
/// the GRU) matches central finite differences on a tiny shape.
#[test]
fn the_clone_gradient_matches_finite_differences() {
    let shape = TwoLayerShape {
        observation: 2,
        hidden: 3,
        legacy_options: 1,
        top_hidden: 3,
        pointer_width: 2,
        ..full_shape()
    };
    let cfg = test_arbitration(0.0, 0.0);
    let mut rng = Rng(31);
    let records: Vec<_> = (0..2)
        .map(|_| {
            record_from(
                (0..6)
                    .map(|t| fixture_sample(&mut rng, &shape, t % 3, (t * 2) % 3))
                    .collect(),
            )
        })
        .collect();
    let theta = initial_center(&shape, 17);
    let mut grad = vec![0.0; theta.len()];
    clone::window_for_test(&shape, &cfg, &records, &theta, 100, Some(&mut grad));
    let eps = 1e-6;
    let mut worst: f64 = 0.0;
    for i in 0..theta.len() {
        let mut up = theta.clone();
        up[i] += eps;
        let mut dn = theta.clone();
        dn[i] -= eps;
        let num = (clone::window_for_test(&shape, &cfg, &records, &up, 100, None)
            - clone::window_for_test(&shape, &cfg, &records, &dn, 100, None))
            / (2.0 * eps);
        let err = (num - grad[i]).abs() / (1e-4 + num.abs() + grad[i].abs());
        worst = worst.max(err);
        assert!(err < 1e-4, "param {i}: analytic {} numeric {num}", grad[i]);
    }
    eprintln!("worst relative gradient error {worst:e} over {} params", theta.len());
}


/// Samples after a cue during which the synthetic teacher's actions do not yet depend on
/// its mode (it turns to face, say, before it moves). Longer than the clones' BPTT
/// window, as a real mode outlasts the 120-sample window by minutes.
const NEUTRAL: usize = 15;
/// The clones' truncated-BPTT window in the comparison.
const WINDOW: usize = 10;

/// The synthetic teacher: two modes set by brief cues and held between them. For
/// [`NEUTRAL`] samples after a cue both modes act alike; after that each mode has its own
/// actions. So the right intent depends on the input (the cue), the actions depend on the
/// intent, and which mode is in force is memory, not something in the present
/// observation.
fn mode_teacher(seed: u64, samples: usize, neutral: usize) -> TrajectoryRecord {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut mode = 0;
    let mut next_cue = 0;
    let mut last_cue = 0;
    let mut out = Vec::with_capacity(samples);
    for t in 0..samples {
        let u = rng.unit();
        let mut obs = vec![0.0, 0.0, u];
        if t == next_cue {
            mode = (rng.next() % 2) as usize;
            obs[mode] = 1.0;
            last_cue = t;
            next_cue = t + rng.range(40, 90) as usize;
        }
        let jitter = 0.3 * (u - 0.5);
        let actions = if t - last_cue < neutral {
            vec![0.5, jitter]
        } else if mode == 0 {
            vec![0.8, -0.5 + jitter]
        } else {
            vec![0.2, 0.6 + jitter]
        };
        let choice = RecordedChoice {
            intent: mode,
            subject: None,
        };
        out.push(RecordedSample {
            observation: obs,
            intent_valid: vec![true, true],
            candidates: Vec::new(),
            early_end: None,
            proposal: Some(choice),
            held: choice,
            actions,
            kick: None,
            call: None,
            rest: None,
        });
    }
    record_from(out)
}

/// What one synthetic cloning comparison measured on held-out streams.
struct ModeComparison {
    fit_intent_agreement: f64,
    mode_two: f64,
    mode_blind: f64,
    mse_two: f64,
    mse_blind: f64,
}

/// Clone the two-layer net and an intent-blind net (today's plain GRU) the same way on the
/// same synthetic streams (`neutral` samples after each cue where both modes act alike),
/// then run both closed loop on held-out streams. Mode is scored where the teacher's
/// actions show it (past the neutral stretch); the blind clone's mode is read off its
/// forward action, which the two modes set apart (0.8 against 0.2).
fn compare_mode_clones(neutral: usize) -> ModeComparison {
    let period = 0.1;
    let train: Vec<_> = (0..12).map(|s| mode_teacher(s, 400, neutral)).collect();
    let held_out: Vec<_> = (100..104).map(|s| mode_teacher(s, 400, neutral)).collect();
    let actions = vec![ActionTransfer::Sigmoid, ActionTransfer::Tanh];
    let two = TwoLayerShape {
        observation: 3,
        hidden: 12,
        actions: actions.clone(),
        legacy_options: 0,
        menu: 2,
        spare: 2,
        candidates: 0,
        row_width: 0,
        kick: false,
        call_kinds: 0,
        rest: false,
        top_hidden: 12,
        pointer_width: 0,
    };
    let blind = TwoLayerShape::plain(3, 12, actions, 0);
    let cfg = IntentArbitration {
        intent_hz: 1.0 / period,
        hold_s: 0.0,
        switch_margin: 0.05,
        temperature: 1.0,
        intent_time_scale_s: 2.0,
    };
    let spec = CloneSpec {
        updates: 400,
        learning_rate: 0.02,
        chunk: WINDOW,
        clip: 5.0,
        seed: 3,
        workers: 4,
    };
    let (theta2, report2) = clone::fit(&two, &cfg, &train, None, &spec).expect("fit");
    let (theta_b, _) = clone::fit(&blind, &cfg, &train, None, &spec).expect("fit");
    let net2 = Arc::new(TwoLayerNet::new(two, theta2).unwrap());
    let net_b = Arc::new(TwoLayerNet::new(blind, theta_b).unwrap());

    let (mut n, mut two_hits, mut blind_hits, mut se2, mut seb) = (0, 0, 0, 0.0, 0.0);
    for record in &held_out {
        let mut b2 = net2.brain(cfg, period);
        let mut bb = net_b.brain(cfg, period);
        let mut last_cue = 0;
        for (t, s) in record.samples.iter().enumerate() {
            if s.observation[0] > 0.5 || s.observation[1] > 0.5 {
                last_cue = t;
            }
            let a2 = b2
                .step(&DecisionSample {
                    observation: &s.observation,
                    intent_valid: &s.intent_valid,
                    candidates: &[],
                    early_end: None,
                })
                .clone();
            let cubarium_voxel_fauna::Response::Logits(lb) = bb.drive(&s.observation) else {
                panic!("logits")
            };
            for (k, tr) in [ActionTransfer::Sigmoid, ActionTransfer::Tanh].iter().enumerate() {
                se2 += (tr.apply(a2.action_logits[k]) - s.actions[k]).powi(2);
                seb += (tr.apply(lb[k]) - s.actions[k]).powi(2);
            }
            if t - last_cue >= neutral {
                n += 1;
                two_hits += usize::from(a2.intent == s.held.intent);
                let blind_mode = usize::from(ActionTransfer::Sigmoid.apply(lb[0]) < 0.5);
                blind_hits += usize::from(blind_mode == s.held.intent);
            }
        }
    }
    let samples = (held_out.len() * 400 * 2) as f64;
    let c = ModeComparison {
        fit_intent_agreement: report2.last.intent_agreement,
        mode_two: two_hits as f64 / n as f64,
        mode_blind: blind_hits as f64 / n as f64,
        mse_two: se2 / samples,
        mse_blind: seb / samples,
    };
    eprintln!(
        "neutral {neutral}: two-layer teacher-forced intent agreement {:.3}, held-out mode \
         {:.3}, action mse {:.6}; intent-blind held-out mode {:.3}, action mse {:.6}",
        c.fit_intent_agreement, c.mode_two, c.mse_two, c.mode_blind, c.mse_blind
    );
    c
}

/// Cloned from the synthetic teacher, the two-layer net holds the teacher's mode and
/// copies each mode's actions on held-out streams far better than an intent-blind clone
/// (today's plain GRU) fitted the same way on the same streams: the held intent and its
/// time carry the mode past the BPTT window, where action error alone cannot teach the
/// blind clone to remember the cue. The comparison is asserted, not an accuracy.
#[test]
fn a_two_layer_clone_beats_an_intent_blind_clone_on_held_out_streams() {
    let c = compare_mode_clones(NEUTRAL);
    assert!(
        c.mode_two > c.mode_blind,
        "the two-layer clone holds the teacher's mode better ({} vs {})",
        c.mode_two,
        c.mode_blind
    );
    assert!(
        c.mse_two * 2.0 < c.mse_blind,
        "the two-layer clone copies each mode's actions far better ({} vs {})",
        c.mse_two,
        c.mse_blind
    );
}

/// **Modes that act at once** (no neutral stretch): the variant the plain GRU won while the
/// whole bottom was conditioned only from the next sample (§11.3's amendment: mode 0.993,
/// action error 0.0023 against 0.0146). With the same-sample readout a mode change acts
/// in the sample it is settled, so the two-layer clone copies the actions at least as well
/// as the blind one. Printed beside the comparison above; the comparison is asserted.
#[test]
fn with_modes_acting_at_once_the_two_layer_clone_is_not_one_sample_late() {
    let c = compare_mode_clones(0);
    assert!(
        c.mse_two <= c.mse_blind,
        "the same-sample readout copies at-once modes no worse ({} vs {})",
        c.mse_two,
        c.mse_blind
    );
}

/// The pointer learns which candidate the teacher means: on held-out samples its
/// teacher-forced subject choice beats the fresh net's and a uniform pick among the valid
/// rows.
#[test]
fn the_cloned_pointer_picks_the_teachers_subject() {
    let shape = TwoLayerShape {
        observation: 2,
        hidden: 8,
        actions: vec![ActionTransfer::Tanh],
        legacy_options: 0,
        menu: 2,
        spare: 2,
        candidates: 3,
        row_width: 2,
        kick: false,
        call_kinds: 0,
        rest: false,
        top_hidden: 8,
        pointer_width: 6,
    };
    let record = |seed: u64| {
        let mut rng = Rng(seed * 7919 + 13);
        let samples = (0..200)
            .map(|_| {
                let feats: Vec<Vec<f64>> =
                    (0..3).map(|_| vec![rng.unit(), rng.unit() * 2.0 - 1.0]).collect();
                let intent = usize::from(rng.unit() < 0.7);
                let best = (0..3)
                    .max_by(|a, b| feats[*a][0].total_cmp(&feats[*b][0]))
                    .unwrap();
                let subject = (intent == 1).then_some(SubjectRef {
                    key: best as u64,
                    kind: SubjectKind::Body,
                });
                let choice = RecordedChoice { intent, subject };
                RecordedSample {
                    observation: vec![f64::from(u8::from(intent == 1)), rng.unit()],
                    intent_valid: vec![true, true],
                    candidates: feats
                        .iter()
                        .enumerate()
                        .map(|(k, f)| RecordedCandidate {
                            key: k as u64,
                            kind: SubjectKind::Body,
                            features: f.clone(),
                            valid_for: vec![false, true],
                        })
                        .collect(),
                    early_end: None,
                    proposal: Some(choice),
                    held: choice,
                    actions: vec![if intent == 1 { feats[best][1] * 0.8 } else { 0.0 }],
                    kick: None,
                    call: None,
                    rest: None,
                }
            })
            .collect();
        record_from(samples)
    };
    let train: Vec<_> = (0..8).map(record).collect();
    let held_out: Vec<_> = (50..53).map(record).collect();
    let cfg = test_arbitration(0.0, 0.0);
    let spec = CloneSpec {
        updates: 300,
        learning_rate: 0.02,
        chunk: 20,
        clip: 5.0,
        seed: 1,
        workers: 4,
    };
    let (theta, _) = clone::fit(&shape, &cfg, &train, None, &spec).expect("fit");
    let set = clone::CloneSet::new(&shape, &cfg, &held_out).unwrap();
    let after = set.loss(&theta, spec.chunk, 1, None);
    let before = set.loss(&initial_center(&shape, spec.seed), spec.chunk, 1, None);
    // Subject labels here: three valid rows plus "none" for intent 1, so a uniform pick
    // is right a quarter of the time.
    let uniform = 0.25;
    eprintln!(
        "pointer: held-out subject agreement {:.3} (fresh {:.3}), ce {:.3} (fresh {:.3})",
        after.subject_agreement, before.subject_agreement, after.subject_ce, before.subject_ce
    );
    assert!(after.subject_ce < before.subject_ce);
    assert!(after.subject_agreement > before.subject_agreement.max(uniform));
}

// ---------------------------------------------------------------------------------------
// Cost.
// ---------------------------------------------------------------------------------------

/// One decision sample's cost (bottom + top + pointer) beside today's GRU step, on the
/// loftstrider's manifest with an illustrative menu. Printed, not asserted.
#[test]
fn report_a_decision_samples_cost() {
    let founder = Founder::Loftstrider;
    let m = founder.manifest();
    let shape = TwoLayerShape {
        menu: 6,
        spare: 2,
        candidates: 8,
        row_width: 16,
        kick: true,
        call_kinds: 3,
        rest: true,
        top_hidden: 32,
        pointer_width: 16,
        ..TwoLayerShape::plain_from_manifest(&m)
    };
    let net = Arc::new(TwoLayerNet::new(shape.clone(), initial_center(&shape, 1)).unwrap());
    let mut rng = Rng(3);
    let obs: Vec<Vec<f64>> = (0..64)
        .map(|_| (0..m.inputs()).map(|_| rng.unit()).collect())
        .collect();
    let feats: Vec<Vec<f64>> = (0..8).map(|_| (0..16).map(|_| rng.unit()).collect()).collect();
    let valid = vec![true; shape.menu];
    let rows: Vec<_> = (0..8)
        .map(|k| CandidateRow {
            key: k as u64,
            kind: SubjectKind::Body,
            features: &feats[k],
            valid_for: &valid,
        })
        .collect();
    let brain = |hz: f64| {
        net.brain(
            IntentArbitration {
                intent_hz: hz,
                ..IntentArbitration::default()
            },
            m.controller_period_s,
        )
    };
    // A decision at every sample, and (for the bottom alone) one decision in the run.
    let mut every = brain(1.0 / m.controller_period_s);
    let mut bottom = brain(1e-9);
    let mut today = test_centre(founder, 1)
        .driver()
        .unwrap()
        .fresh(TeacherConfig::default());
    // The machine is shared: interleave the three and keep each one's fastest round.
    let (n, rounds) = (4_000, 7);
    let mut best = [f64::INFINITY; 3];
    let mut decided = 0;
    for _ in 0..rounds {
        let start = std::time::Instant::now();
        for i in 0..n {
            let _ = today.drive(&obs[i % 64]);
        }
        best[0] = best[0].min(start.elapsed().as_nanos() as f64 / n as f64);
        for (slot, b) in [(1, &mut every), (2, &mut bottom)] {
            let start = std::time::Instant::now();
            for i in 0..n {
                let a = b.step(&DecisionSample {
                    observation: &obs[i % 64],
                    intent_valid: &valid,
                    candidates: &rows,
                    early_end: None,
                });
                decided += usize::from(slot == 1 && a.decided);
            }
            best[slot] = best[slot].min(start.elapsed().as_nanos() as f64 / n as f64);
        }
    }
    assert_eq!(decided, n * rounds, "the timed brain decided at every sample");
    // Multiply-adds, exactly: GRU 3H(I+H) + O·H; top T·U + M'·T; pointer P·T + K·P·(F+3).
    let (h, t, p, k, f) = (
        shape.hidden,
        shape.top_hidden,
        shape.pointer_width,
        shape.candidates,
        shape.row_width + 2,
    );
    let gru = |i: usize, o: usize| 3 * h * (i + h) + o * h;
    let today_macs = gru(m.inputs(), m.outputs());
    let bottom_macs = gru(shape.bottom_inputs(), shape.bottom_outputs());
    let top_macs = t * shape.top_inputs() + shape.intents() * t + p * t + k * p * (f + 1);
    let layout = TwoLayerLayout::new(&shape);
    eprintln!(
        "cost, loftstrider I={} H={} (period {} s), menu 6+2, K=8, F=16, kick, 3 calls, rest, \
         T=32, P=16: today's GRU step {:.0} ns / {today_macs} MACs; two-layer bottom alone \
         {:.0} ns / {bottom_macs} MACs; bottom + top + pointer {:.0} ns / {} MACs; params \
         bottom {} top {} (fastest of {rounds} interleaved rounds of {n})",
        m.inputs(),
        m.hidden,
        m.controller_period_s,
        best[0],
        best[2],
        best[1],
        bottom_macs + top_macs,
        layout.range(Layer::Bottom).len(),
        layout.range(Layer::Top).len()
    );
}
