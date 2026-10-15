//! SCORE-V2 mechanism tests, on hand-built fixtures; knobs are set by the fixture or read
//! back from the config, never a tuned default.
//!
//! - a death costs the life value of the stage it died in, and a birth adds a young's to its
//!   mother (and, at the kin share, to her relatives);
//! - a drive term over a window telescopes to the drive's start − end, and a death neither
//!   pays nor relieves a drive;
//! - the clone pull's gradient is its finite difference, is zero at the clone, and pulls
//!   toward it (in the surrogate and through a whole update);
//! - pilot v1's kin file pins every SCORE-V2 term off and reproduces its own reward exactly;
//! - a body's life stage and drives read its own state against its lineage's physiology.

use super::judge::{self, LifeStage};
use super::knobs::{JudgeKnobs, PgKnobs, TeacherKnobs};
use super::learner::{self, PgState, Policy, Step, TopInput, Trajectory};
use super::reward::{self, Drive, KinEnergy, LifeValue, OwnEnergy, PgBody, PgWorld, RewardTerm};
use crate::es::optimizer::Adam;
use crate::es::voxel::kin::{KinConfig, KinRecord, KinShares, Roster};
use crate::es::voxel::two_layer::{ActionTransfer, Layer, TopDecision, TwoLayerLayout, TwoLayerShape, initial_center};
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

fn input(s: &TwoLayerShape, rng: &mut Rng, masked: &[usize]) -> TopInput {
    TopInput {
        hidden: (0..s.hidden).map(|_| rng.sym()).collect(),
        held: Some((rng.next() * s.menu as f64) as usize % s.menu),
        time: rng.next(),
        valid: (0..s.intents()).map(|i| i < s.menu && !masked.contains(&i)).collect(),
    }
}

/// A theta whose top carries real weight.
fn theta(s: &TwoLayerShape, seed: u64) -> Vec<f64> {
    let mut t = initial_center(s, seed);
    let l = TwoLayerLayout::new(s);
    let mut rng = Rng(seed ^ 0xabc);
    for i in l.range(Layer::Top) {
        t[i] += 0.5 * rng.sym();
    }
    t
}

fn decision(tick: u64) -> TopDecision {
    TopDecision { tick, hidden: vec![0.0; 4], held: None, time: 0.0, valid: vec![true; 6], intent: 1, logp: -1.0 }
}

/// Drives that change every tick, differently per drive and body.
fn drive_series(len: usize, seed: u64) -> Vec<[f64; judge::DRIVES]> {
    let mut rng = Rng(seed);
    (0..len).map(|_| [rng.next(), rng.next(), rng.next()]).collect()
}

/// A mother (alive, breeding), her calf born at tick 5 that dies young at tick 9, and an
/// unrelated adult that dies at tick 7; energy and drives move every tick.
fn family() -> (PgWorld, Roster) {
    let mother = PgBody {
        id: 10,
        start: 0,
        series: (0..=12).map(|t| 1.0 + 0.1 * t as f64).collect(),
        died: false,
        decisions: [1, 4, 8, 11].map(decision).to_vec(),
        drives: drive_series(13, 1),
        stage: LifeStage::Breeding,
        born_to: None,
    };
    let mut calf_series: Vec<f64> = (5..9).map(|t| 0.3 - 0.02 * t as f64).collect();
    calf_series.push(0.0);
    let mut calf_drives = drive_series(4, 2);
    calf_drives.push(calf_drives[3]);
    let calf = PgBody {
        id: 11,
        start: 5,
        series: calf_series,
        died: true,
        decisions: [6, 8].map(decision).to_vec(),
        drives: calf_drives,
        stage: LifeStage::Young,
        born_to: Some(10),
    };
    let mut other_series: Vec<f64> = (0..7).map(|t| 0.9 + 0.01 * t as f64).collect();
    other_series.push(0.0);
    let mut other_drives = drive_series(7, 3);
    other_drives.push(other_drives[6]);
    let other = PgBody {
        id: 12,
        start: 0,
        series: other_series,
        died: true,
        decisions: [0, 3, 5].map(decision).to_vec(),
        drives: other_drives,
        stage: LifeStage::Adult,
        born_to: None,
    };
    let w = PgWorld { founder: Founder::Lanternjaw, bodies: vec![mother, calf, other] };
    let roster = Roster::fixed(&[
        KinRecord { id: 10, mother: None, pack: None },
        KinRecord { id: 11, mother: Some(10), pack: None },
        KinRecord { id: 12, mother: None, pack: None },
    ]);
    (w, roster)
}

/// The sum of a term over body `b`'s trained spans, as [`reward::trajectories`] cuts them.
fn over_spans(t: &dyn RewardTerm, w: &PgWorld, b: usize) -> f64 {
    let body = &w.bodies[b];
    let d = &body.decisions;
    let trained = if body.died { d.len() } else { d.len() - 1 };
    (0..trained)
        .map(|i| {
            let to = if i + 1 < d.len() { d[i + 1].tick } else { body.end() };
            t.reward(w, b, d[i].tick, to)
        })
        .sum()
}

#[test]
fn a_death_costs_its_stages_life_value_and_a_birth_adds_a_youngs_to_its_mother() {
    let (w, roster) = family();
    let judge = JudgeKnobs { life_young: 0.7, life_adult: 0.3, life_breeding: 1.1 };
    let life = LifeValue::new(&w, &judge);
    // The mother lives on: her only event is her calf's birth, inside her trained spans.
    assert!((over_spans(&life, &w, 0) - judge.life_young).abs() < 1e-12);
    assert_eq!(life.reward(&w, 0, 1, 4), 0.0, "before the birth");
    assert_eq!(life.reward(&w, 0, 4, 8), judge.life_young, "the span holding the birth");
    // The calf dies young; the unrelated adult dies an adult: each loses its stage's value
    // in the span that ends at its death.
    assert!((over_spans(&life, &w, 1) + judge.life_young).abs() < 1e-12);
    assert!((over_spans(&life, &w, 2) + judge.life_adult).abs() < 1e-12);
    assert_eq!(life.reward(&w, 2, 5, w.bodies[2].end()), -judge.life_adult);
    // Stage is the key: the same death as a breeding adult costs the breeding value.
    let mut w2 = w.clone();
    w2.bodies[2].stage = LifeStage::Breeding;
    assert!((over_spans(&LifeValue::new(&w2, &judge), &w2, 2) + judge.life_breeding).abs() < 1e-12);
    // Through the teacher's terms: the mother's reward is her own change, her calf's birth,
    // and her kin share of the calf's energy and of its death.
    let shares = KinShares { calf: 0.25, mother: 0.5, ..KinShares::default() };
    let knobs = PgKnobs { judge, teacher: TeacherKnobs::off(), ..PgKnobs::default() };
    let boxed = reward::teacher_terms(&w, &roster, &shares, &knobs);
    let names: Vec<&str> = boxed.iter().map(|t| t.name()).collect();
    assert_eq!(names, ["own", "kin", "life", "kin_life"]);
    let list: Vec<&dyn RewardTerm> = boxed.iter().map(|t| t.as_ref()).collect();
    let (trajs, _) = reward::trajectories(&w, 0, &list, 0.05);
    let m = trajs.iter().find(|t| t.body == 10).unwrap();
    let total: f64 = m.steps.iter().map(|s| s.reward).sum();
    let (mother, calf) = (&w.bodies[0], &w.bodies[1]);
    let expect = (mother.s(11) - mother.s(1)) + judge.life_young + 0.25 * ((calf.s(11) - calf.s(1)) - judge.life_young);
    assert!((total - expect).abs() < 1e-12, "{total} vs {expect}");
}

#[test]
fn a_drive_term_over_a_window_telescopes_to_its_start_minus_end() {
    let (w, _) = family();
    for k in 0..judge::DRIVES {
        let term = Drive { k, weight: 0.37 };
        for b in 0..w.bodies.len() {
            let body = &w.bodies[b];
            let first = body.decisions[0].tick;
            let last = if body.died { body.end() } else { body.decisions.last().unwrap().tick };
            let sum = over_spans(&term, &w, b);
            let expect = 0.37 * (body.drive(first, k) - body.drive(last, k));
            assert!((sum - expect).abs() < 1e-12, "drive {k} body {b}: {sum} vs {expect}");
            // Any cut of a span adds up the same.
            let (a, c) = (first, last);
            let mid = (a + c) / 2;
            let cut = term.reward(&w, b, a, mid) + term.reward(&w, b, mid, c);
            assert!((cut - term.reward(&w, b, a, c)).abs() < 1e-12);
        }
        // A death neither pays nor relieves a drive: past the last reading it holds.
        let calf = &w.bodies[1];
        assert_eq!(term.reward(&w, 1, calf.end() - 1, calf.end() + 50), 0.0);
    }
}

fn pull_fixture() -> (TwoLayerShape, Vec<f64>, Vec<Step>) {
    let s = shape();
    let clone = theta(&s, 61);
    let mut rng = Rng(62);
    let l = TwoLayerLayout::new(&s);
    let pol = Policy { shape: &s, layout: &l, temperature: 0.8 };
    let steps = (0..10)
        .map(|k| {
            let x = input(&s, &mut rng, if k % 4 == 0 { &[2] } else { &[] });
            let logp = pol.log_prob(&clone, &x, 0);
            Step { input: x, action: 0, logp, dt_s: 0.25, reward: 0.0 }
        })
        .collect();
    (s, clone, steps)
}

fn clone_probs(pol: &Policy<'_>, clone: &[f64], steps: &[Step]) -> Vec<Vec<f64>> {
    steps
        .iter()
        .map(|st| {
            let mut p = vec![0.0; pol.shape.intents()];
            pol.probs(clone, &st.input, &mut p);
            p
        })
        .collect()
}

#[test]
fn the_clone_pull_gradient_matches_finite_differences_is_zero_at_the_clone_and_pulls_toward_it() {
    let (s, clone, steps) = pull_fixture();
    let l = TwoLayerLayout::new(&s);
    let pol = Policy { shape: &s, layout: &l, temperature: 0.8 };
    let refs: Vec<&Step> = steps.iter().collect();
    let c = clone_probs(&pol, &clone, &steps);
    let beta = 0.7;
    let mut rng = Rng(63);
    let mut at = clone.clone();
    for i in pol.trained() {
        at[i] += 0.3 * rng.sym();
    }
    // 1. Finite differences, with the advantage and entropy terms on as well.
    let adv: Vec<f64> = (0..steps.len()).map(|_| rng.sym()).collect();
    let mut grad = vec![0.0; at.len()];
    let sur = pol.surrogate_pulled(&at, &refs, &adv, 0.2, 0.1, Some((beta, &c)), Some(&mut grad));
    assert!(sur.clone_kl > 0.0, "away from the clone the KL is positive");
    let h = 1e-6;
    let mut worst: f64 = 0.0;
    for i in pol.trained() {
        let (mut up, mut dn) = (at.clone(), at.clone());
        up[i] += h;
        dn[i] -= h;
        let num = (pol.surrogate_pulled(&up, &refs, &adv, 0.2, 0.1, Some((beta, &c)), None).objective
            - pol.surrogate_pulled(&dn, &refs, &adv, 0.2, 0.1, Some((beta, &c)), None).objective)
            / (2.0 * h);
        worst = worst.max((num - grad[i]).abs() / (1e-6 + num.abs().max(grad[i].abs())));
    }
    assert!(worst < 1e-4, "worst relative error {worst}");
    // 2. At the clone, the pull alone (no advantage, no entropy) has no gradient and no KL.
    let zero = vec![0.0; steps.len()];
    let mut g0 = vec![0.0; clone.len()];
    let s0 = pol.surrogate_pulled(&clone, &refs, &zero, 0.2, 0.0, Some((beta, &c)), Some(&mut g0));
    assert!(s0.clone_kl.abs() < 1e-12, "{}", s0.clone_kl);
    assert!(g0.iter().all(|g| g.abs() < 1e-12), "the pull is still at the clone");
    // 3. Away from it, a step up the pull's gradient lowers the KL.
    let mut g = vec![0.0; at.len()];
    let before = pol.surrogate_pulled(&at, &refs, &zero, 0.2, 0.0, Some((beta, &c)), Some(&mut g)).clone_kl;
    let moved: Vec<f64> = at.iter().zip(&g).map(|(x, d)| x + 1e-2 * d).collect();
    let after = pol.surrogate_pulled(&moved, &refs, &zero, 0.2, 0.0, Some((beta, &c)), None).clone_kl;
    assert!(after < before, "{before} -> {after}");
    // 4. Without a pull the surrogate is the plain one exactly.
    let plain = pol.surrogate(&at, &refs, &adv, 0.2, 0.1, None);
    let none = pol.surrogate_pulled(&at, &refs, &adv, 0.2, 0.1, None, None);
    assert_eq!(plain.objective.to_bits(), none.objective.to_bits());
}

#[test]
fn an_update_with_nothing_to_learn_moves_toward_the_clone_and_without_an_anchor_stays() {
    let (s, clone, steps) = pull_fixture();
    let l = TwoLayerLayout::new(&s);
    let pol = Policy { shape: &s, layout: &l, temperature: 0.8 };
    let mut rng = Rng(64);
    let mut start = clone.clone();
    for i in pol.trained() {
        start[i] += 0.3 * rng.sym();
    }
    // Every reward zero and a fresh baseline: every advantage is zero.
    let trajs: Vec<Trajectory> = steps
        .iter()
        .enumerate()
        .map(|(k, st)| Trajectory { world: k % 2, body: k as u64, steps: vec![st.clone()], bootstrap: None })
        .collect();
    let c = clone_probs(&pol, &clone, &steps);
    let refs: Vec<&Step> = steps.iter().collect();
    let zero = vec![0.0; steps.len()];
    let kl = |t: &[f64]| pol.surrogate_pulled(t, &refs, &zero, 0.2, 0.0, Some((1.0, &c)), None).clone_kl;
    let knobs = PgKnobs { entropy: 0.0, clone_kl: 0.5, policy_lr: 0.01, ..PgKnobs::default() };
    let mut t = start.clone();
    let mut state = PgState::new(&s, &knobs, 5);
    state.anchor = clone.clone();
    let mut adam = Adam::new(t.len());
    let r = learner::update(&pol, &mut t, &mut adam, &mut state, &trajs, &knobs, 1, 2);
    assert!(kl(&t) < kl(&start), "{} -> {}", kl(&start), kl(&t));
    assert!(r.clone_kl_weight > 0.0 && r.clone_kl.is_finite() && r.clone_entropy.is_finite());
    // No anchor: no pull, and nothing else to learn, so the centre stays.
    let mut t2 = start.clone();
    let mut state2 = PgState::new(&s, &knobs, 5);
    let mut adam2 = Adam::new(t2.len());
    let r2 = learner::update(&pol, &mut t2, &mut adam2, &mut state2, &trajs, &knobs, 1, 2);
    assert_eq!(r2.clone_kl_weight, 0.0);
    assert!(t2.iter().zip(&start).all(|(a, b)| a.to_bits() == b.to_bits()));
    // The fade: weight at generation g is clone_kl x (1 - g/N), 0 past N, constant at N = 0.
    let faded = PgKnobs { clone_kl: 0.4, clone_kl_fade_generations: 10.0, ..PgKnobs::default() };
    assert!((faded.clone_kl_at(5) - 0.2).abs() < 1e-12);
    assert_eq!(faded.clone_kl_at(20), 0.0);
    let constant = PgKnobs { clone_kl: 0.4, clone_kl_fade_generations: 0.0, ..PgKnobs::default() };
    assert_eq!(constant.clone_kl_at(1_000), 0.4);
}

#[test]
fn pilot_v1s_kin_file_pins_every_score_v2_term_off_and_reproduces_its_reward() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/pg-pilot-2026-10-14.toml");
    let k = KinConfig::load(&path).unwrap();
    let pg = k.of(Founder::Lanternjaw).pg;
    assert!(pg.judge.is_off());
    assert_eq!(pg.teacher, TeacherKnobs::off());
    assert_eq!(pg.clone_kl_at(0), 0.0);
    // On a window with births, deaths and moving drives, its teacher is v1's two terms,
    // reward for reward.
    let (w, roster) = family();
    let shares = k.of(Founder::Lanternjaw).shares;
    let boxed = reward::teacher_terms(&w, &roster, &shares, &pg);
    let list: Vec<&dyn RewardTerm> = boxed.iter().map(|t| t.as_ref()).collect();
    let (now, totals) = reward::trajectories(&w, 0, &list, 0.05);
    let (own, kin) = (OwnEnergy, KinEnergy::new(&w, &roster, &shares));
    let v1: [&dyn RewardTerm; 2] = [&own, &kin];
    let (then, v1_totals) = reward::trajectories(&w, 0, &v1, 0.05);
    assert_eq!(now, then);
    assert_eq!(totals, v1_totals);
    // And the v2 file parses to the policy gradient on the lanternjaw.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/score-v2-2026-10-15.toml");
    let v2 = KinConfig::load(&path).unwrap();
    assert_eq!(v2.of(Founder::Lanternjaw).comparison, crate::es::voxel::kin::Comparison::PolicyGradient);
}

#[test]
fn a_bodys_stage_and_drives_read_its_own_state_against_its_physiology() {
    use cubarium_voxel_sim::mode_scenes::{self, ModeScene};
    let f = Founder::Lanternjaw;
    let sim = mode_scenes::found(&mode_scenes::Config::default(), ModeScene::Feed, 1).unwrap().sim;
    let base = *sim.fauna().view().animals.iter().find(|a| a.founder == Some(f)).expect("a lanternjaw");
    let p = *sim.fauna().config().founder(f);
    // Stage: below breeding size young; at it, adult, or breeding with a brood held.
    let mut a = base;
    a.attached = None;
    a.reproduction.escrow = None;
    a.reproduction.brood = 0.0;
    a.body = 0.5 * p.core.birth_body;
    assert_eq!(judge::life_stage(&a, &p), LifeStage::Young);
    a.body = p.core.birth_body;
    assert_eq!(judge::life_stage(&a, &p), LifeStage::Adult);
    a.reserve = a.reserve.max(1e-3);
    a.reproduction.brood = 0.5 * a.reserve;
    assert_eq!(judge::life_stage(&a, &p), LifeStage::Breeding);
    // Hunger is 1 - S/K; wound is the wound stock.
    let mut a = base;
    a.stomach = Default::default();
    a.wound = 0.3;
    let now = 1_000;
    let d = judge::drives(&a, &p, now);
    let k = p.gut.capacity(a.body);
    assert_eq!(d[judge::HUNGER], if k > 0.0 { 1.0 } else { 0.0 });
    assert_eq!(d[judge::WOUND], 0.3);
    if k > 0.0 {
        a.stomach.pools[0].organic = 0.25 * k;
        assert!((judge::drives(&a, &p, now)[judge::HUNGER] - 0.75).abs() < 1e-12);
    }
    // Sleep debt: with light rest and a ceiling, time since the last deep bout over the
    // ceiling; none while deep asleep; none for a lineage without a ceiling.
    let mut q = p;
    q.rest.light = true;
    q.rest.need_s = 100.0;
    let mut a = base;
    a.state = cubarium_voxel_fauna::State::Walking;
    a.doze.last_deep = 400;
    let debt = judge::drives(&a, &q, 1_400)[judge::SLEEP];
    assert!((debt - 1_000.0 * cubarium_voxel::DT / 100.0).abs() < 1e-12, "{debt}");
    a.state = cubarium_voxel_fauna::State::Asleep;
    a.doze.light = false;
    assert_eq!(judge::drives(&a, &q, 1_400)[judge::SLEEP], 0.0);
    q.rest.need_s = 0.0;
    a.state = cubarium_voxel_fauna::State::Walking;
    assert_eq!(judge::drives(&a, &q, 1_400)[judge::SLEEP], 0.0);
}
