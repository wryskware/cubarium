// R2a review at 80bf718: three regressions fail on the reviewed implementation.
// Copy to crates/cubarium-search/tests/review_r2a_temporary.rs and run
// cargo test -p cubarium-search --release --test review_r2a_temporary
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use cubarium_search::es::{self, export::PolicyFile, tensor, trainer};

#[test]
fn foreign_policy_digest_must_be_rejected() {
    let mut file = PolicyFile::new(&tensor::initial_center(1), "review", 0, 0).unwrap();
    file.policy_digest ^= 1;
    assert!(file.policy().is_err(), "foreign digest was silently replaced by the current digest");
}

#[test]
fn deadline_during_last_batch_must_cancel_without_update() {
    let layouts = es::training_layouts();
    let protocol = es::Protocol::new(1, 4_000, 20_260_915, &layouts[..1]);
    let mut theta = tensor::initial_center(protocol.train_seed);
    let original = theta.clone();
    let mut adam = es::Adam::new(es::PARAMS);
    let cancel = AtomicBool::new(false);
    let start = Instant::now();
    let plan = es::Plan {
        layouts: &layouts[..1], horizon: 4_000, workers: 2,
        detail: es::Detail::Score, evaluate_center: false,
        deadline: Some(start + Duration::from_millis(50)),
    };
    let result = trainer::run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel);
    assert!(result.is_err(), "completed and updated after {:?} despite 50ms deadline; Adam step {}", start.elapsed(), adam.step);
    assert_eq!(theta, original);
    assert_eq!(adam.step, 0);
}

#[test]
fn score_detail_must_not_report_unmeasured_paid_costs_as_zero() {
    let layouts = es::training_layouts();
    let policy = tensor::policy(&tensor::initial_center(20_260_915)).unwrap();
    let driver = es::Driver::Policy(Box::new(policy));
    let cancel = AtomicBool::new(false);
    let full = es::episode::run(&layouts[0], &driver, 40, es::Detail::Full, &cancel).unwrap();
    let score = es::episode::run(&layouts[0], &driver, 40, es::Detail::Score, &cancel).unwrap();
    assert_eq!(full.terminal_stores, score.terminal_stores);
    assert!(full.upkeep_paid > 0.0);
    assert!(full.motion_paid > 0.0);
    assert_eq!(score.upkeep_paid, full.upkeep_paid, "identical episode falsely records zero paid upkeep");
    assert_eq!(score.motion_paid, full.motion_paid);
}
