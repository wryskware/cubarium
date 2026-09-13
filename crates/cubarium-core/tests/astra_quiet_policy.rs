//! Independent mechanism/compatibility probes; synthetic stocks are fixtures, not ecology.
use cubarium_core::controller::{Observation, TurnGate, decide, decide_quiet};
use cubarium_core::organism::Mode;
use cubarium_core::quiet::{Budget, QuietEvent, QuietOverride, QuietPolicy, QuietReason};
use cubarium_core::{OrganismId, World, WorldConfig, decode_snapshot, encode_snapshot};
use cubarium_surface::Vec2;

fn breeder() -> World {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 1;
    cfg.founders.initial_reserve_fraction = 1.0;
    cfg.founders.initial_energy_fraction = 1.0;
    cfg.drives.bud_min_age_seconds = 0.0;
    cfg.organism.gestation_seconds = 0.1;
    let mut world = World::new(cfg).unwrap();
    // Valid historical memory makes this an active parent, not a founder's initial rest pose.
    for (_, o) in world.state.organisms.iter_mut() {
        o.mode = Mode::Seeking;
        o.hunger_memory = 0.5;
    }
    world
}

/// Until the first admitted paid birth, enabling the policy must change no legacy byte.
fn admitted() -> (World, OrganismId, OrganismId, u64) {
    let mut off = breeder();
    let mut state = off.state.clone();
    state.quiet.policy = QuietPolicy::PostBirthPauseV1;
    let mut on = World::from_state(state).unwrap();
    for _ in 0..100 {
        off.step();
        on.step();
        let mut projected = on.state.clone();
        projected.quiet = Default::default();
        assert_eq!(
            projected, off.state,
            "admission cannot debit stocks, change the paid child, or consume extra RNG"
        );
        assert_eq!(on.drain_events(), off.drain_events());
        for e in on.drain_quiet_events() {
            if let QuietEvent::Begin {
                tick,
                parent,
                child,
                end_tick,
                ..
            } = e
            {
                assert_eq!(end_tick, tick + 40);
                let o = on.state.organisms.get(parent).unwrap();
                assert!(o.births > 0);
                assert_ne!(o.mode, Mode::Resting, "birth boundary is not repainted");
                assert_eq!(on.state.organisms.get(child).unwrap().parent, Some(parent));
                assert!(on.state.organisms.get(child).unwrap().escrow.is_none());
                assert_eq!(
                    on.quiet().pauses.len(),
                    1,
                    "newborn is not offered its own pause"
                );
                return (on, parent, child, tick);
            }
        }
    }
    panic!("synthetic funded parent did not admit a pause in 100 ticks");
}

#[test]
fn successful_paid_birth_holds_exactly_forty_intervals_and_restarts_at_the_last_boundary() {
    let (mut world, parent, _, birth) = admitted();
    let first_counter = world.state.organisms.get(parent).unwrap().turn_counter;
    let (_, state) = decode_snapshot(&encode_snapshot(&world.state, "astra")).unwrap();
    let mut replay = World::from_state(state).unwrap();
    for completed in 1..=40 {
        world.step();
        replay.step();
        assert_eq!(world.state, replay.state);
        let o = world.state.organisms.get(parent).unwrap();
        assert_eq!(o.mode, Mode::Resting, "completed interval {completed}");
        assert!(!o.fed_this_tick);
        assert!(o.escrow.is_none(), "no new budding while held");
        let mut expected_counter = first_counter;
        for _ in 0..4 * completed {
            expected_counter.take();
        }
        assert_eq!(
            o.turn_counter, expected_counter,
            "ordinary four draws per tick"
        );
        world
            .check_invariants()
            .unwrap_or_else(|e| panic!("B+{completed}: {e}"));
        let (_, state) = decode_snapshot(&encode_snapshot(&world.state, "astra")).unwrap();
        if completed == 40 {
            replay = World::from_state(state).unwrap();
        }
    }
    assert_eq!(world.tick(), birth + 40);
    // This is the first ordinary decision, not a forty-first imposed resting interval.
    world.step();
    replay.step();
    assert_eq!(world.state, replay.state);
    assert!(world.quiet().find(parent).is_none());
    let ends: Vec<_> = world
        .drain_quiet_events()
        .into_iter()
        .filter(|e| {
            matches!(e,
        QuietEvent::End{parent:p,tick,completed_ticks:40,..} if *p==parent && *tick==birth+40)
        })
        .collect();
    assert_eq!(ends.len(), 1);
}

#[test]
fn affordability_is_strict_finite_and_read_only_with_the_exact_individual_budget() {
    let world = breeder();
    let (_, original) = world.state.organisms.iter().next().unwrap();
    let mut o = original.clone();
    o.structure = o.phenotype.structure_adult * 0.9;
    let cfg = &world.config().organism;
    let h = 41.0 * cubarium_core::DT;
    let b = Budget::of(&o, cfg, h).unwrap();
    let sb = o.structure.max(o.phenotype.structure_adult);
    let g = (cfg.growth_rate * h).min((o.phenotype.structure_adult - o.structure).max(0.0));
    let rate = o.phenotype.maintenance * sb
        + cfg.move_cost * sb * (f64::from(o.phenotype.drives.rest_effort) * o.phenotype.speed_max)
        + cfg.sense_cost * o.phenotype.sense_radius;
    assert_eq!(b.energy, rate * h + cfg.build_cost * g);
    assert_eq!(b.material, cfg.oxidation_rate * h + g);
    o.energy = b.energy;
    o.reserve = b.material + 1.0;
    assert!(!b.affordable(&o));
    o.energy = b.energy + 1.0;
    o.reserve = b.material;
    assert!(!b.affordable(&o));
    o.reserve = b.material + 1.0;
    let before = o.clone();
    assert!(b.affordable(&o));
    assert_eq!(before, o);
    o.energy = f64::INFINITY;
    assert!(!b.affordable(&o));
    assert!(Budget::of(&o, cfg, f64::NAN).is_none());
}

#[test]
fn release_uses_live_underlying_hysteresis_not_the_imposed_mode() {
    let world = breeder();
    let (_, base) = world.state.organisms.iter().next().unwrap();
    let mut o = base.clone();
    // This band is deliberately between seek_off and seek_on, not biological satiation.
    let memory =
        (f64::from(o.phenotype.drives.seek_off) + f64::from(o.phenotype.drives.seek_on)) * 0.5;
    o.reserve = (1.0 - memory) * o.phenotype.reserve_max;
    o.hunger_memory = memory;
    o.mode = Mode::Resting;
    o.ou = Vec2::new(0.3, 0.2);
    let obs = Observation {
        p_here: 1.0,
        f_here: 1.0,
        d_here: 1.0,
        noise: Vec2::new(2.0, -1.0),
        ..Default::default()
    };
    let args = (10000, cubarium_core::DT, true, true, TurnGate::default());
    let held = decide_quiet(
        &o,
        &obs,
        args.0,
        args.1,
        args.2,
        args.3,
        args.4,
        Some(QuietOverride {
            underlying: Mode::Seeking,
            hold: true,
        }),
    );
    assert_eq!(held.mode, Mode::Resting);
    assert_eq!(held.underlying_mode, Mode::Feeding);
    assert_eq!(
        (held.fruit_effort, held.graze_effort, held.scavenge_effort),
        (0.0, 0.0, 0.0)
    );
    assert!(!held.bud);
    let mut ordinary = o.clone();
    ordinary.mode = held.underlying_mode;
    let released = decide_quiet(
        &o,
        &obs,
        args.0,
        args.1,
        args.2,
        args.3,
        args.4,
        Some(QuietOverride {
            underlying: held.underlying_mode,
            hold: false,
        }),
    );
    assert_eq!(
        released,
        decide(&ordinary, &obs, args.0, args.1, args.2, args.3, args.4)
    );
    assert_eq!(released.mode, Mode::Feeding);
    assert_eq!(
        held.hunger_memory, released.hunger_memory,
        "memory evaluated only once"
    );
}

#[test]
fn depletion_aborts_before_the_same_tick_controller_and_cannot_be_rearmed_by_draining() {
    let (mut world, parent, _, birth) = admitted();
    world.state.organisms.get_mut(parent).unwrap().reserve = 0.0;
    let mut expected = world.state.clone();
    let underlying = expected.quiet.find(parent).unwrap().underlying;
    expected.quiet = Default::default();
    expected.organisms.get_mut(parent).unwrap().mode = underlying;
    let mut ordinary = World::from_state(expected).unwrap();
    world.step();
    ordinary.step();
    let mut projected = world.state.clone();
    projected.quiet = Default::default();
    assert_eq!(projected, ordinary.state);
    assert!(world.drain_quiet_events().iter().any(|e| matches!(e,
        QuietEvent::Abort{tick,parent:p,completed_ticks:0,reason:QuietReason::UnaffordableRemaining,..} if *tick==birth && *p==parent)));
    let bytes = encode_snapshot(&world.state, "astra");
    assert!(world.drain_quiet_events().is_empty());
    assert_eq!(encode_snapshot(&world.state, "astra"), bytes);
}

#[test]
fn genuine_schema12_off_plain_and_care_continue_as_recorded() {
    for (start, end) in [
        (
            &include_bytes!("fixtures/quiet-v12-plain-3000.cubw")[..],
            &include_bytes!("fixtures/quiet-v12-plain-3000-plus600.cubw")[..],
        ),
        (
            &include_bytes!("fixtures/quiet-v12-care-3000.cubw")[..],
            &include_bytes!("fixtures/quiet-v12-care-3000-plus600.cubw")[..],
        ),
    ] {
        let (m, s) = decode_snapshot(start).unwrap();
        assert_eq!(m.schema, 12);
        assert_eq!(s.quiet.policy, QuietPolicy::Off);
        let (_, expected) = decode_snapshot(end).unwrap();
        let mut w = World::from_state(s).unwrap();
        for _ in 0..600 {
            w.step();
        }
        assert_eq!(
            postcard::to_allocvec(&cubarium_core::snapshot::v12::project(&w.state).unwrap())
                .unwrap(),
            postcard::to_allocvec(&cubarium_core::snapshot::v12::project(&expected).unwrap())
                .unwrap()
        );
        assert!(w.drain_quiet_events().is_empty());
        w.state.quiet.policy = QuietPolicy::PostBirthPauseV1;
        assert!(cubarium_core::snapshot::v12::project(&w.state).is_none());
    }
}

#[test]
fn age_death_cleans_full_parent_identity_and_reused_slot_cannot_inherit_the_pause() {
    let (world, parent, child, birth) = admitted();
    let mut state = world.state;
    let pause = *state.quiet.find(parent).unwrap();
    state.config.organism.max_age_seconds = birth as f64 * cubarium_core::DT;
    let mut world = World::from_state(state).unwrap();
    world.step();
    assert!(world.state.organisms.get(parent).is_none());
    assert!(world.quiet().find(parent).is_none());
    assert!(world.state.organisms.get(child).is_some());
    let events = world.drain_quiet_events();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e,QuietEvent::Abort {
        parent:p,tick,completed_ticks:1,reason:QuietReason::ParentGone,..}
        if *p==parent && *tick==birth+1))
            .count(),
        1
    );
    decode_snapshot(&encode_snapshot(&world.state, "astra")).unwrap();
    let duplicate = world.state.organisms.get(child).unwrap().clone();
    let replacement = world.state.organisms.insert(duplicate);
    assert_eq!(replacement.slot, parent.slot);
    assert_ne!(replacement.generation, parent.generation);
    let mut stale = world.quiet().clone();
    stale.pauses.push(pause);
    assert!(
        stale
            .validate(
                world.tick(),
                world.config().capacity.max_organisms as usize,
                false,
                |id| world.state.organisms.get(id).is_some()
            )
            .is_err()
    );
}

#[test]
fn persisted_pause_mutations_and_hunter_combination_are_refused() {
    let (world, parent, _, _) = admitted();
    let state = &world.state;
    let validate = |q: &cubarium_core::quiet::QuietState| {
        q.validate(
            state.tick,
            state.config.capacity.max_organisms as usize,
            false,
            |id| state.organisms.get(id).is_some(),
        )
    };
    let mut q = state.quiet.clone();
    q.version += 1;
    assert!(validate(&q).is_err());
    let mut q = state.quiet.clone();
    q.policy = QuietPolicy::Off;
    assert!(validate(&q).is_err());
    let mut q = state.quiet.clone();
    q.pauses.push(q.pauses[0]);
    assert!(validate(&q).is_err());
    let mut q = state.quiet.clone();
    q.pauses[0].parent.generation += 1;
    assert!(validate(&q).is_err());
    let mut q = state.quiet.clone();
    q.pauses[0].child = parent;
    assert!(validate(&q).is_err());
    let mut q = state.quiet.clone();
    q.pauses[0].end_tick += 1;
    assert!(validate(&q).is_err());
    let mut q = state.quiet.clone();
    q.pauses[0].start_tick = u64::MAX - 1;
    q.pauses[0].end_tick = u64::MAX;
    assert!(validate(&q).is_err());
    let q = &state.quiet;
    assert!(
        q.validate(
            state.tick,
            state.config.capacity.max_organisms as usize,
            true,
            |_| true
        )
        .is_err()
    );
    assert!(q.validate(state.tick, 0, false, |_| true).is_err());
}

#[test]
fn off_ecology_hash_remains_the_bare_legacy_payload_not_an_option_wrapper() {
    let (_, s) = decode_snapshot(include_bytes!("fixtures/quiet-v12-plain-3000.cubw")).unwrap();
    let legacy = cubarium_core::snapshot::v7::project(&s);
    let bytes = postcard::to_allocvec(&legacy).unwrap();
    let expected = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3)
    });
    assert_eq!(
        cubarium_core::snapshot::ecology_hash(&s),
        expected,
        "changing a projection API to Option must not add its tag to legacy ecology hashing"
    );
}

#[test]
fn current_schema_rejects_a_crc_valid_unconsumed_payload_tail() {
    let state = breeder().state;
    let mut bytes = encode_snapshot(&state, "astra");
    let at = 10 + u16::from_le_bytes(bytes[8..10].try_into().unwrap()) as usize;
    bytes.push(0x7f);
    let length = (bytes.len() - at - 12) as u64;
    bytes[at..at + 8].copy_from_slice(&length.to_le_bytes());
    let crc = crc32fast::hash(&bytes[at + 12..]);
    bytes[at + 8..at + 12].copy_from_slice(&crc.to_le_bytes());
    assert!(
        decode_snapshot(&bytes).is_err(),
        "a correct envelope cannot legitimize an unconsumed current-schema payload tail"
    );
}
