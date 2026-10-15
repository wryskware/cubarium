//! PG-PILOT mechanism tests, on hand-built fixtures (written before the learner). None of
//! them reads a tuned default: knobs are set by the fixture or read back from the config.
//!
//! - the surrogate's gradient is its finite difference (clipped and unclipped, entropy on);
//! - a positive advantage raises its intent, a negative one lowers it;
//! - on a bandit where one intent pays, generations of updates raise its probability;
//! - the value baseline's regression lowers its error;
//! - a masked intent never gets probability: not in the policy, not in an exploring
//!   brain's samples, not through the gradient;
//! - GAE is the discounted return with a zero baseline and a full trace, and the one-step
//!   TD error with no trace; a death ends the return;
//! - the reward interface: own energy sums to the window's net energy, a death costs the
//!   body's energy, a relative's change counts at its share;
//! - the split readout agrees with itself on identical worlds;
//! - the update moves only the intent head.

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use super::knobs::PgKnobs;
use super::learner::{self, Policy, PgState, Step, TopInput, Trajectory, ValueNet};
use super::reward::{self, KinEnergy, OwnEnergy, PgBody, PgWorld, RewardTerm};
use crate::es::optimizer::Adam;
use crate::es::voxel::kin::{KinRecord, KinShares, Roster};
use crate::es::voxel::two_layer::{
    ActionTransfer, CandidateRow, DecisionSample, Explore, IntentArbitration, Layer, TopDecision,
    TwoLayerLayout, TwoLayerNet, TwoLayerShape, initial_center,
};
use cubarium_voxel_fauna::Founder;

fn shape() -> TwoLayerShape {
    TwoLayerShape {
        observation: 3,
        hidden: 4,
        actions: vec![ActionTransfer::Sigmoid],
        legacy_options: 0,
        menu: 4,
        spare: 2,
        candidates: 0,
        row_width: 0,
        kick: false,
        call_kinds: 0,
        rest: false,
        top_hidden: 5,
        pointer_width: 0,
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn sym(&mut self) -> f64 {
        2.0 * self.next() - 1.0
    }
}

/// The named intents valid except `masked`; spares always masked.
fn mask(s: &TwoLayerShape, masked: &[usize]) -> Vec<bool> {
    (0..s.intents()).map(|i| i < s.menu && !masked.contains(&i)).collect()
}

fn input(s: &TwoLayerShape, rng: &mut Rng, masked: &[usize]) -> TopInput {
    TopInput {
        hidden: (0..s.hidden).map(|_| rng.sym()).collect(),
        held: Some((rng.next() * s.menu as f64) as usize % s.menu),
        time: rng.next(),
        valid: mask(s, masked),
    }
}

fn policy<'a>(s: &'a TwoLayerShape, l: &'a TwoLayerLayout, temperature: f64) -> Policy<'a> {
    Policy { shape: s, layout: l, temperature }
}

/// A draw from `p`.
fn draw(p: &[f64], rng: &mut Rng) -> usize {
    let x = rng.next();
    let mut acc = 0.0;
    let mut last = 0;
    for (i, &pi) in p.iter().enumerate() {
        if pi > 0.0 {
            acc += pi;
            last = i;
            if x < acc {
                return i;
            }
        }
    }
    last
}

fn step_of(pol: &Policy<'_>, theta: &[f64], input: TopInput, action: usize, reward: f64) -> Step {
    let logp = pol.log_prob(theta, &input, action);
    Step { input, action, logp, dt_s: 0.25, reward }
}

/// A theta whose top carries real weight (the seeded centre's are small).
fn theta(s: &TwoLayerShape, seed: u64) -> Vec<f64> {
    let mut t = initial_center(s, seed);
    let l = TwoLayerLayout::new(s);
    let mut rng = Rng(seed ^ 0xabc);
    for i in l.range(Layer::Top) {
        t[i] += 0.5 * rng.sym();
    }
    t
}

#[test]
fn the_surrogate_gradient_matches_finite_differences() {
    let s = shape();
    let l = TwoLayerLayout::new(&s);
    for (temperature, clip, nudge) in [(1.0, 0.9, 0.02), (0.7, 0.05, 0.4)] {
        let pol = policy(&s, &l, temperature);
        let old = theta(&s, 5);
        let mut rng = Rng(9);
        let steps: Vec<Step> = (0..12)
            .map(|k| {
                let x = input(&s, &mut rng, if k % 3 == 0 { &[1] } else { &[] });
                let mut p = vec![0.0; s.intents()];
                pol.probs(&old, &x, &mut p);
                let a = draw(&p, &mut rng);
                step_of(&pol, &old, x, a, 0.0)
            })
            .collect();
        let refs: Vec<&Step> = steps.iter().collect();
        let adv: Vec<f64> = (0..steps.len()).map(|_| rng.sym()).collect();
        // Away from the sampling policy, so some ratios differ from 1 (and, with the small
        // clip, sit outside it).
        let mut at = old.clone();
        for i in pol.trained() {
            at[i] += nudge * rng.sym();
        }
        let entropy = 0.3;
        let mut grad = vec![0.0; at.len()];
        let sur = pol.surrogate(&at, &refs, &adv, clip, entropy, Some(&mut grad));
        assert!(sur.objective.is_finite());
        let h = 1e-6;
        let mut worst: f64 = 0.0;
        for i in 0..at.len() {
            let mut up = at.clone();
            up[i] += h;
            let mut dn = at.clone();
            dn[i] -= h;
            let num = (pol.surrogate(&up, &refs, &adv, clip, entropy, None).objective
                - pol.surrogate(&dn, &refs, &adv, clip, entropy, None).objective)
                / (2.0 * h);
            if !pol.trained().contains(&i) {
                assert_eq!(grad[i], 0.0, "parameter {i} is outside the intent head");
            }
            worst = worst.max((num - grad[i]).abs() / (1e-6 + num.abs().max(grad[i].abs())));
        }
        assert!(worst < 1e-4, "worst relative error {worst} (temperature {temperature}, clip {clip})");
    }
}

#[test]
fn a_positive_advantage_raises_its_intent_and_a_negative_one_lowers_it() {
    let s = shape();
    let l = TwoLayerLayout::new(&s);
    let pol = policy(&s, &l, 1.0);
    let t = theta(&s, 3);
    let mut rng = Rng(4);
    let x = input(&s, &mut rng, &[]);
    for a in 0..s.menu {
        for sign in [1.0, -1.0] {
            let st = step_of(&pol, &t, x.clone(), a, 0.0);
            let mut grad = vec![0.0; t.len()];
            pol.surrogate(&t, &[&st], &[sign], 0.2, 0.0, Some(&mut grad));
            let moved: Vec<f64> = t.iter().zip(&grad).map(|(v, g)| v + 1e-3 * g).collect();
            let before = pol.log_prob(&t, &x, a);
            let after = pol.log_prob(&moved, &x, a);
            if sign > 0.0 {
                assert!(after > before, "intent {a}: +A must raise it ({before} -> {after})");
            } else {
                assert!(after < before, "intent {a}: -A must lower it ({before} -> {after})");
            }
        }
    }
}

/// One generation of the bandit: `n` one-decision bodies on one state, each paid 1 for
/// `best` and 0 otherwise, the episode ending at the decision.
fn bandit_generation(pol: &Policy<'_>, theta: &[f64], x: &TopInput, best: usize, n: usize, rng: &mut Rng) -> Vec<Trajectory> {
    let mut p = vec![0.0; pol.shape.intents()];
    pol.probs(theta, x, &mut p);
    (0..n)
        .map(|k| {
            let a = draw(&p, rng);
            let st = step_of(pol, theta, x.clone(), a, if a == best { 1.0 } else { 0.0 });
            Trajectory { world: k % 4, body: k as u64, steps: vec![st], bootstrap: None }
        })
        .collect()
}

#[test]
fn on_a_bandit_the_intent_that_pays_gains_probability() {
    let s = shape();
    let l = TwoLayerLayout::new(&s);
    let pol = policy(&s, &l, 1.0);
    let mut t = theta(&s, 11);
    let mut rng = Rng(12);
    let x = input(&s, &mut rng, &[3]);
    let mut p0 = vec![0.0; s.intents()];
    pol.probs(&t, &x, &mut p0);
    // The paying intent is the least likely valid one, so learning has to find it.
    let best = (0..s.menu).filter(|&i| i != 3).min_by(|&a, &b| p0[a].total_cmp(&p0[b])).unwrap();
    let knobs = PgKnobs { policy_lr: 0.05, ..PgKnobs::default() };
    let mut adam = Adam::new(t.len());
    let mut state = PgState::new(&s, &knobs, 1);
    for g in 0..8 {
        let trajs = bandit_generation(&pol, &t, &x, best, 256, &mut rng);
        let r = learner::update(&pol, &mut t, &mut adam, &mut state, &trajs, &knobs, g, 2);
        assert_eq!(r.decisions, 256);
    }
    let mut p1 = vec![0.0; s.intents()];
    pol.probs(&t, &x, &mut p1);
    assert!(p1[best] > p0[best], "the paying intent {best}: {} -> {}", p0[best], p1[best]);
    assert_eq!(p1[3], 0.0, "the masked intent stays at zero");
    assert!(p1[s.menu..].iter().all(|&q| q == 0.0), "spares stay masked");
}

#[test]
fn the_value_regression_lowers_its_error() {
    let s = shape();
    let mut rng = Rng(21);
    let inputs: Vec<Vec<f64>> = (0..64).map(|_| learner::features(&s, &input(&s, &mut rng, &[]))).collect();
    let targets: Vec<f64> = inputs.iter().map(|u| u[0] - 0.5 * u[1] + 0.3).collect();
    let refs: Vec<&[f64]> = inputs.iter().map(Vec::as_slice).collect();
    let mut v = ValueNet::new(inputs[0].len(), 8, 3);
    let mut adam = Adam::new(v.phi.len());
    adam.learning_rate = 0.01;
    let before = v.loss(&refs, &targets, None);
    for _ in 0..200 {
        let mut g = vec![0.0; v.phi.len()];
        v.loss(&refs, &targets, Some(&mut g));
        for x in &mut g {
            *x = -*x;
        }
        adam.ascend(&mut v.phi, &g);
    }
    let after = v.loss(&refs, &targets, None);
    assert!(after < before, "{before} -> {after}");
    // And its gradient is its finite difference.
    let mut g = vec![0.0; v.phi.len()];
    v.loss(&refs, &targets, Some(&mut g));
    for i in 0..v.phi.len() {
        let mut up = v.clone();
        up.phi[i] += 1e-6;
        let mut dn = v.clone();
        dn.phi[i] -= 1e-6;
        let num = (up.loss(&refs, &targets, None) - dn.loss(&refs, &targets, None)) / 2e-6;
        assert!((num - g[i]).abs() <= 1e-5 * (1.0 + num.abs()), "value parameter {i}: {num} vs {}", g[i]);
    }
}

#[test]
fn a_masked_intent_never_gets_probability() {
    let s = shape();
    let l = TwoLayerLayout::new(&s);
    let pol = policy(&s, &l, 1.0);
    let t = theta(&s, 31);
    let mut rng = Rng(32);
    // The policy: zero on the masked and the spares, a distribution on the rest.
    let x = input(&s, &mut rng, &[0, 2]);
    let mut p = vec![0.0; s.intents()];
    pol.probs(&t, &x, &mut p);
    assert_eq!((p[0], p[2]), (0.0, 0.0));
    assert!(p[s.menu..].iter().all(|&q| q == 0.0));
    assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    // The gradient never touches a masked intent's own logit row.
    let st = step_of(&pol, &t, x.clone(), 1, 0.0);
    let mut grad = vec![0.0; t.len()];
    pol.surrogate(&t, &[&st], &[1.0], 0.2, 0.5, Some(&mut grad));
    let top = l.top.unwrap();
    for k in [0, 2, 4, 5] {
        assert_eq!(grad[top.b2 + k], 0.0, "masked intent {k}'s bias");
        assert!(grad[top.w2 + k * s.top_hidden..top.w2 + (k + 1) * s.top_hidden].iter().all(|g| *g == 0.0));
    }
    // An exploring brain samples only valid intents, logs what its top read, and logs the
    // log-probability of what it sampled.
    let arb = IntentArbitration { intent_hz: 4.0, hold_s: 0.0, switch_margin: 0.0, temperature: 1.0, intent_time_scale_s: 5.0 };
    let net = Arc::new(TwoLayerNet::new(s.clone(), t.clone()).unwrap());
    let mut brain = net.brain(arb, 0.25);
    let clock = Arc::new(AtomicU64::new(0));
    let log: Arc<Mutex<Vec<TopDecision>>> = Arc::new(Mutex::new(Vec::new()));
    brain.explore_with(Explore::new(77, Arc::clone(&clock), Arc::clone(&log)));
    let valid = [true, false, true, false];
    let none: [CandidateRow<'_>; 0] = [];
    for k in 0..400u64 {
        clock.store(k, std::sync::atomic::Ordering::Relaxed);
        let obs: Vec<f64> = (0..s.observation).map(|_| rng.sym()).collect();
        brain.step(&DecisionSample { observation: &obs, intent_valid: &valid, candidates: &none, early_end: None });
    }
    let log = log.lock().unwrap();
    assert!(log.len() > 100, "it decided ({})", log.len());
    let mut seen = [0usize; 4];
    for d in log.iter() {
        assert!(valid[d.intent], "sampled masked intent {}", d.intent);
        seen[d.intent] += 1;
        let x = TopInput { hidden: d.hidden.clone(), held: d.held, time: d.time, valid: d.valid.clone() };
        assert!((pol.log_prob(&t, &x, d.intent) - d.logp).abs() < 1e-12, "the logged log-probability is the policy's");
    }
    assert!(seen[0] > 0 && seen[2] > 0, "both valid intents were sampled: {seen:?}");
    assert!(log.windows(2).all(|w| w[0].tick < w[1].tick), "stamped by the clock");
}

fn traj(rewards: &[f64], dts: &[f64], bootstrap: bool) -> Trajectory {
    let s = shape();
    let mut rng = Rng(41);
    Trajectory {
        world: 0,
        body: 1,
        steps: rewards
            .iter()
            .zip(dts)
            .map(|(&reward, &dt_s)| Step { input: input(&s, &mut rng, &[]), action: 0, logp: 0.0, dt_s, reward })
            .collect(),
        bootstrap: bootstrap.then(|| input(&s, &mut rng, &[])),
    }
}

#[test]
fn gae_is_the_discounted_return_or_the_td_error_at_its_ends() {
    let rewards = [0.3, -0.1, 0.0, 0.7];
    let dts = [0.25, 1.0, 0.25, 3.0];
    // A full trace and a zero baseline: the discounted return, plus the bootstrap's
    // discounted value when the body lived on.
    let knobs = PgKnobs { lambda_horizon_s: 1e15, discount_horizon_s: 2.0, ..PgKnobs::default() };
    for (alive, boot) in [(true, 0.9), (false, 0.9)] {
        let t = traj(&rewards, &dts, alive);
        let a = learner::advantages(&t, &[0.0; 4], boot, &knobs);
        for i in 0..4 {
            let mut ret = 0.0;
            let mut disc = 1.0;
            for k in i..4 {
                ret += disc * rewards[k];
                disc *= knobs.factors(dts[k]).0;
            }
            if alive {
                ret += disc * boot;
            }
            assert!((a[i] - ret).abs() < 1e-9, "step {i} alive {alive}: {} vs {ret}", a[i]);
        }
    }
    // No trace: the one-step TD error against the baseline.
    let knobs = PgKnobs { lambda_horizon_s: 1e-12, discount_horizon_s: 2.0, ..PgKnobs::default() };
    let values = [0.2, -0.4, 0.1, 0.5];
    let t = traj(&rewards, &dts, true);
    let a = learner::advantages(&t, &values, 0.9, &knobs);
    for i in 0..4 {
        let next = if i + 1 < 4 { values[i + 1] } else { 0.9 };
        let td = rewards[i] + knobs.factors(dts[i]).0 * next - values[i];
        assert!((a[i] - td).abs() < 1e-9, "step {i}: {} vs {td}", a[i]);
    }
}

fn decision(tick: u64) -> TopDecision {
    TopDecision { tick, hidden: vec![0.0; 4], held: None, time: 0.0, valid: vec![true; 6], intent: 1, logp: -1.0 }
}

#[test]
fn the_rewards_are_net_energy_with_death_and_kin() {
    // A mother alive the whole window (12 ticks) and a calf that dies at tick 9.
    let mother = PgBody {
        id: 10,
        start: 0,
        series: (0..=12).map(|t| 1.0 + 0.1 * t as f64).collect(),
        died: false,
        decisions: [1, 4, 8, 11].map(decision).to_vec(),
        ..PgBody::default()
    };
    let mut calf_series: Vec<f64> = (2..=8).map(|t| 0.5 - 0.02 * t as f64).collect();
    calf_series.push(0.0); // dead: E = 0, it gave nothing
    let calf = PgBody { id: 11, start: 2, series: calf_series, died: true, decisions: [3, 5, 7].map(decision).to_vec(), ..PgBody::default() };
    let w = PgWorld { founder: Founder::Lanternjaw, bodies: vec![mother.clone(), calf.clone()] };
    let roster = Roster::fixed(&[
        KinRecord { id: 10, mother: None, pack: None },
        KinRecord { id: 11, mother: Some(10), pack: None },
    ]);
    let shares = KinShares { calf: 0.25, mother: 0.5, ..KinShares::default() };
    let own = OwnEnergy;
    let kin = KinEnergy::new(&w, &roster, &shares);
    let terms: [&dyn RewardTerm; 2] = [&own, &kin];
    let dt = 0.05;
    let (trajs, totals) = reward::trajectories(&w, 3, &terms, dt);
    assert_eq!(trajs.len(), 2);
    let m = trajs.iter().find(|t| t.body == 10).unwrap();
    let c = trajs.iter().find(|t| t.body == 11).unwrap();
    assert_eq!(m.world, 3);
    // The mother lived on: her last decision is the bootstrap, the rest are trained.
    assert_eq!(m.steps.len(), 3);
    assert!(m.bootstrap.is_some());
    assert!((m.steps[1].dt_s - 4.0 * dt).abs() < 1e-12);
    // The calf died: every decision trained, the last one's reward runs to its death.
    assert_eq!(c.steps.len(), 3);
    assert!(c.bootstrap.is_none());
    // Own energy over the calf's trained span: its series from its first decision to the
    // end, the death costing what it held.
    let calf_own: f64 = (0..3).map(|i| own.reward(&w, 1, [3, 5, 7][i], [5, 7, calf.end()][i])).sum();
    assert!((calf_own - (0.0 - calf.s(3))).abs() < 1e-12, "{calf_own}");
    // Each decision's reward is its own change plus the relative's at its share.
    for (i, (from, to)) in [(1u64, 4u64), (4, 8), (8, 11)].into_iter().enumerate() {
        let expect = (mother.s(to) - mother.s(from)) + 0.25 * (calf.s(to) - calf.s(from));
        assert!((m.steps[i].reward - expect).abs() < 1e-12, "mother step {i}");
    }
    for (i, (from, to)) in [(3u64, 5u64), (5, 7), (7, calf.end())].into_iter().enumerate() {
        let expect = (calf.s(to) - calf.s(from)) + 0.5 * (mother.s(to) - mother.s(from));
        assert!((c.steps[i].reward - expect).abs() < 1e-12, "calf step {i}");
    }
    let total: f64 = trajs.iter().flat_map(|t| t.steps.iter().map(|s| s.reward)).sum();
    assert!((totals.0.iter().map(|x| x.1).sum::<f64>() - total).abs() < 1e-12, "the terms add up to the rewards");
}

#[test]
fn the_split_readout_agrees_on_identical_worlds_and_only_the_intent_head_moves() {
    let s = shape();
    let l = TwoLayerLayout::new(&s);
    let pol = policy(&s, &l, 1.0);
    let mut t = theta(&s, 51);
    let mut rng = Rng(52);
    let x = input(&s, &mut rng, &[]);
    let one = bandit_generation(&pol, &t, &x, 2, 64, &mut rng);
    // Two worlds holding the same bodies: every half-split puts one in each half.
    let mut trajs = Vec::new();
    for world in 0..2 {
        for (k, tr) in one.iter().enumerate() {
            trajs.push(Trajectory { world, body: (world * 1000 + k) as u64, ..tr.clone() });
        }
    }
    let knobs = PgKnobs::default();
    let mut adam = Adam::new(t.len());
    let mut state = PgState::new(&s, &knobs, 2);
    let before = t.clone();
    let r = learner::update(&pol, &mut t, &mut adam, &mut state, &trajs, &knobs, 0, 2);
    assert!(r.split_worlds > 0.999, "identical worlds give the same step: {}", r.split_worlds);
    assert!(r.gradient_norm > 0.0 && r.update_rms > 0.0);
    assert!(r.next.is_nan(), "no previous generation");
    for (i, (a, b)) in t.iter().zip(&before).enumerate() {
        if !pol.trained().contains(&i) {
            assert_eq!(a.to_bits(), b.to_bits(), "parameter {i} is outside the intent head");
        }
    }
    let r2 = learner::update(&pol, &mut t, &mut adam, &mut state, &trajs, &knobs, 1, 2);
    assert!(r2.next.is_finite());
}

#[test]
fn the_shipped_pilot_kin_file_parses_to_the_policy_gradient() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/pg-pilot-2026-10-14.toml");
    let k = crate::es::voxel::kin::KinConfig::load(&path).unwrap();
    let lj = k.of(Founder::Lanternjaw);
    assert_eq!(lj.comparison, crate::es::voxel::kin::Comparison::PolicyGradient);
    assert_eq!(lj.stage, crate::es::voxel::kin::BrainStage::Top);
    // Every knob it names is in the registry's range, and an out-of-range one is refused.
    assert!(crate::es::voxel::kin::KinConfig::parse("[lanternjaw]\ncomparison = \"policy_gradient\"\n[lanternjaw.pg]\nclip = 0.0\n").is_err());
    assert!(crate::es::voxel::kin::KinConfig::parse("[lanternjaw]\ncomparison = \"policy_gradient\"\n[lanternjaw.pg]\nbogus = 1.0\n").is_err());
}
