// Include inside the actual reproduction.rs test module of a mechanical review copy.
// Correctness assertions: these must pass before the corresponding audit gaps are closed.

#[test]
fn astra_birth_cannot_also_report_a_refused_budding_branch() {
    let mut s = staged_birth();
    let parent = s.hunter.iter().find_map(|event| match event {
        HunterEvent::Reproduction { record: Reproduction::Born { key, .. }, .. } => Some(key.parent),
        _ => None,
    }).expect("genuine birth");
    let before = s.audit.summary();
    let mut malformed = s.hunter.clone();
    malformed.push(blocked(s.world.tick(), parent));
    assert!(s.audit.observe(&malformed, &s.life, &s.world.state).is_err(),
        "a due-birth parent cannot also enter the mutually exclusive budding branch");
    assert_eq!(s.audit.summary(), before);
    s.audit.observe(&s.hunter, &s.life, &s.world.state).expect("genuine birth remains valid");
}

#[test]
fn astra_member_cannot_fund_and_refund_in_the_same_physiology_pass() {
    let mut world = quiet(WorldConfig::default());
    let profile = breeder(&world);
    let parent = found(&mut world, profile);
    let mut audit = ReproductionAudit::new(&world.state).expect("fresh opening");
    world.step();
    let hunter = world.drain_hunter_events();
    let life = world.drain_events();
    assert!(!saw(&hunter, "funded"), "fixture has no actual funding");
    let before = audit.summary();
    assert!(audit.observe(&fund_refund(&world, parent), &life, &world.state).is_err(),
        "a newly funded escrow cannot already be due for refund in the same pass");
    assert_eq!(audit.summary(), before);
    audit.observe(&hunter, &life, &world.state).expect("genuine quiet tick remains valid");
}

#[test]
fn astra_observer_at_maximum_tick_returns_an_atomic_error() {
    let mut world = quiet(WorldConfig::default());
    world.state.tick = u64::MAX;
    let mut audit = ReproductionAudit::new(&world.state).expect("empty opening");
    let before = audit.summary();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        audit.observe(&[], &[], &world.state)
    })).expect("checked next-tick arithmetic must not panic");
    assert!(result.is_err());
    assert_eq!(audit.summary(), before);
    assert_eq!(audit.last_complete_tick(), u64::MAX);
}

#[test]
fn astra_newborn_cannot_report_a_pre_step_funding_decision() {
    let mut s = staged_birth();
    let child = s.hunter.iter().find_map(|event| match event {
        HunterEvent::Reproduction { record: Reproduction::Born { child, .. }, .. } => Some(*child),
        _ => None,
    }).expect("genuine newborn");
    let before = s.audit.summary();
    let mut malformed = s.hunter.clone();
    malformed.push(blocked(s.world.tick(), child));
    assert!(s.audit.observe(&malformed, &s.life, &s.world.state).is_err());
    assert_eq!(s.audit.summary(), before);
    s.audit.observe(&s.hunter, &s.life, &s.world.state).expect("genuine birth remains valid");
}

#[test]
fn astra_missing_and_duplicate_ordinary_death_are_atomic_rejections() {
    for duplicate in [false, true] {
        let mut s = staged_same_tick_loss();
        let before = s.audit.summary();
        let mut malformed = s.life.clone();
        let death = malformed.iter().find(|event| matches!(event, LifeEvent::Death { .. }))
            .expect("genuine ordinary death").clone();
        if duplicate {
            malformed.push(death);
        } else {
            malformed.retain(|event| !matches!(event, LifeEvent::Death { .. }));
        }
        assert!(s.audit.observe(&s.hunter, &malformed, &s.world.state).is_err());
        assert_eq!(s.audit.summary(), before);
        s.audit.observe(&s.hunter, &s.life, &s.world.state).expect("genuine loss remains valid");
    }
}
