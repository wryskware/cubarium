// Diagnostic fixtures for the copied observer with SHA256
// 34c6869ef92c40454398533613a11d4310bce9458126db03d3c9d8a048434c39.
// Reverified unchanged outcomes against committed f4c410a, source SHA256
// 2d0b8ce0a233c754d7bdb1862d643d35fc8aa9e985b238635cb9975d46a0fd6c.
// Include inside that file's existing #[cfg(test)] mod tests, in an isolated copy.
// Assertions demonstrate the reviewed defects; they are not desired regressions.

fn astra_blocked(tick: u64, parent: OrganismId) -> HunterEvent {
    HunterEvent::Reproduction {
        tick,
        hunter: parent,
        record: Reproduction::NotFunded { parent, reason: FundingBlocked::Stocks },
    }
}

fn astra_fund_refund(world: &World, parent: OrganismId) -> Vec<HunterEvent> {
    let cfg = &world.config().organism;
    let p = &world.state.organisms.get(parent).unwrap().phenotype;
    let structure = cfg.child_structure_fraction * p.structure_adult;
    let reserve = cfg.child_reserve_fraction * p.reserve_max;
    let energy = cfg.child_energy_fraction * p.energy_max;
    let build = cfg.build_cost * structure;
    let key = EscrowKey { parent, started_tick: world.tick() - 1 };
    vec![
        HunterEvent::Reproduction {
            tick: world.tick(), hunter: parent,
            record: Reproduction::Funded {
                key, parent_reserve_before: structure + reserve, parent_reserve_after: 0.0,
                parent_energy_before: energy + build, parent_energy_after: 0.0,
                escrow_structure: structure, escrow_reserve: reserve, escrow_energy: energy,
                build_heat: build,
            },
        },
        HunterEvent::Reproduction {
            tick: world.tick(), hunter: parent,
            record: Reproduction::Refunded {
                key, refunded_structure: structure, refunded_reserve: reserve, refunded_energy: energy,
                parent_reserve_before: 0.0, parent_reserve_after: structure + reserve,
                parent_energy_before: 0.0, parent_energy_after: energy,
            },
        },
    ]
}

fn astra_staged_same_tick_loss() -> Staged {
    let mut cfg = WorldConfig::default();
    cfg.organism.max_age_seconds = 2.0;
    let mut world = quiet(cfg);
    let profile = breeder(&world);
    let parent = found(&mut world, profile);
    feed_to_full(&mut world, parent);
    world.state.hunters.member_mut(parent).unwrap().next_reproduction_tick = 40;
    let mut audit = ReproductionAudit::new(&world.state).unwrap();
    for _ in 0..80 {
        world.step();
        let hunter = world.drain_hunter_events();
        let life = world.drain_events();
        if saw(&hunter, "miscarried") {
            return Staged { world, audit, hunter, life };
        }
        audit.observe(&hunter, &life, &world.state).unwrap();
    }
    panic!("no loss");
}

#[test]
fn astra_unknown_refusal_is_accepted() {
    let mut world = quiet(WorldConfig::default());
    let mut audit = ReproductionAudit::new(&world.state).unwrap();
    world.step();
    let id = OrganismId { slot: 12345, generation: 999 };
    audit.observe(&[astra_blocked(world.tick(), id)], &[], &world.state).unwrap();
    assert_eq!(count(&audit.summary(), "not_funded_stocks"), 1);
}

#[test]
fn astra_duplicate_refusal_is_counted_twice() {
    let mut world = quiet(WorldConfig::default());
    let profile = breeder(&world);
    let parent = found(&mut world, profile);
    let mut audit = ReproductionAudit::new(&world.state).unwrap();
    world.step();
    let event = astra_blocked(world.tick(), parent);
    audit.observe(&[event.clone(), event], &[], &world.state).unwrap();
    assert_eq!(count(&audit.summary(), "not_funded_stocks"), 2);
}

#[test]
fn astra_ordinary_parent_funding_and_refund_are_accepted() {
    let mut world = quiet(WorldConfig::default());
    let parent = bystander(&mut world, SPOT);
    let mut audit = ReproductionAudit::new(&world.state).unwrap();
    world.step();
    assert!(!world.hunters().contains(parent));
    let events = astra_fund_refund(&world, parent);
    audit.observe(&events, &[], &world.state).unwrap();
    assert_eq!(count(&audit.summary(), "funded"), 1);
    assert_eq!(count(&audit.summary(), "refunded"), 1);
}

#[test]
fn astra_closed_same_tick_key_can_be_reopened_and_counted_twice() {
    let mut s = astra_staged_same_tick_loss();
    let copied: Vec<_> = s.hunter.iter().filter(|e| matches!(e, HunterEvent::Reproduction { .. })).cloned().collect();
    assert_eq!(copied.len(), 2);
    s.hunter.extend(copied);
    s.audit.observe(&s.hunter, &s.life, &s.world.state).unwrap();
    assert_eq!(count(&s.audit.summary(), "funded"), 2);
    assert_eq!(count(&s.audit.summary(), "miscarried"), 2);
}

#[test]
fn astra_missing_hunter_death_is_accepted() {
    let mut s = astra_staged_same_tick_loss();
    let before = s.hunter.len();
    s.hunter.retain(|e| !matches!(e, HunterEvent::Death { .. }));
    assert_eq!(before, s.hunter.len() + 1);
    s.audit.observe(&s.hunter, &s.life, &s.world.state).unwrap();
    assert_eq!(count(&s.audit.summary(), "miscarried"), 1);
}

#[test]
fn astra_actual_miscarriage_can_be_relabelled_as_refund_to_dead_parent() {
    let mut s = astra_staged_same_tick_loss();
    let (key, structure, reserve, energy) = s.hunter.iter().find_map(|event| match event {
        HunterEvent::Reproduction { record: Reproduction::Funded {
            key, escrow_structure, escrow_reserve, escrow_energy, ..
        }, .. } => Some((*key, *escrow_structure, *escrow_reserve, *escrow_energy)),
        _ => None,
    }).unwrap();
    assert!(s.world.state.organisms.get(key.parent).is_none());
    for event in &mut s.hunter {
        if let HunterEvent::Reproduction { record, .. } = event {
            if matches!(record, Reproduction::Miscarried { .. }) {
                *record = Reproduction::Refunded {
                    key, refunded_structure: structure, refunded_reserve: reserve, refunded_energy: energy,
                    parent_reserve_before: 0.0, parent_reserve_after: structure + reserve,
                    parent_energy_before: 0.0, parent_energy_after: energy,
                };
            }
        }
    }
    s.audit.observe(&s.hunter, &s.life, &s.world.state).unwrap();
    assert_eq!(count(&s.audit.summary(), "refunded"), 1);
    assert_eq!(count(&s.audit.summary(), "miscarried"), 0);
}

#[test]
fn astra_membership_addition_without_birth_is_accepted_by_module() {
    let mut world = quiet(WorldConfig::default());
    let mut audit = ReproductionAudit::new(&world.state).unwrap();
    let profile = breeder(&world);
    let parent = found(&mut world, profile);
    world.step();
    audit.observe(&[], &[], &world.state).unwrap();
    assert!(audit.sizes.contains_key(&parent));
    assert_eq!(count(&audit.summary(), "born"), 0);
}

#[test]
fn astra_maximum_key_tick_panics_instead_of_returning_error() {
    let mut world = quiet(WorldConfig::default());
    let profile = breeder(&world);
    let parent = found(&mut world, profile);
    let mut audit = ReproductionAudit::new(&world.state).unwrap();
    world.step();
    let mut events = astra_fund_refund(&world, parent);
    if let HunterEvent::Reproduction { record: Reproduction::Funded { key, .. }, .. } = &mut events[0] {
        key.started_tick = u64::MAX;
    }
    let before = audit.summary();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        audit.observe(&events, &[], &world.state)
    }));
    assert!(caught.is_err(), "the reviewed debug build panics rather than returning Result::Err");
    assert_eq!(audit.summary(), before);
}
