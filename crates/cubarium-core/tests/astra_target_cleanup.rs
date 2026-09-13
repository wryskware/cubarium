//! Independent boundary probes for the shared-prey cleanup repair (512ee52).
use cubarium_core::{
    HunterEvent, HunterMember, HunterPhase, HunterState, OrganismId, World, WorldState,
    WorldStateV12, decode_snapshot, encode_snapshot,
};

/// FNV-1a 64 over a payload, the hash `snapshot::state_hash` computes. Written out here so the
/// recorded schema 12 replay hash below can stay the number that was verified: schema 13
/// appends the inert ordinary-quiet extension, so the live `state_hash` of the same world is a
/// different — and equally correct — number. This one is a property of the frozen payload.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

#[test]
fn invalidation_is_full_id_scoped_and_preserves_paid_episode() {
    let prey = OrganismId {
        slot: 99,
        generation: 7,
    };
    let mut state = HunterState::default();
    for (slot, phase) in [
        HunterPhase::Stalking,
        HunterPhase::Windup,
        HunterPhase::Strike,
    ]
    .into_iter()
    .enumerate()
    {
        let mut m = HunterMember::new(
            OrganismId {
                slot: slot as u32,
                generation: 3,
            },
            10,
        );
        m.enter(
            phase,
            10,
            if phase == HunterPhase::Stalking {
                10
            } else {
                30
            },
            12,
        );
        m.attack_counter = 12;
        m.target = Some(prey);
        state.insert_member(m);
    }
    let initial = state.clone();
    state.forget_target(
        OrganismId {
            generation: 8,
            ..prey
        },
        20,
    );
    assert_eq!(
        state, initial,
        "same slot but another generation must be inert"
    );
    state.forget_target(prey, 20);
    for i in 0..2 {
        let m = state.members[i];
        assert_eq!(m.phase, HunterPhase::Perched);
        assert_eq!(m.entered_from, initial.members[i].phase);
        assert_eq!(
            (m.phase_started_tick, m.phase_ends_tick, m.episode),
            (20, 20, 0)
        );
        assert_eq!(m.attack_counter, 12);
        assert_eq!(m.target, None);
    }
    let mut expected_paid = initial.members[2];
    expected_paid.target = None;
    assert_eq!(
        state.members[2], expected_paid,
        "paid phase, counter, timing and history survive"
    );
    let once = state.clone();
    state.forget_target(prey, 21);
    assert_eq!(state, once, "repeat cleanup cannot restart a pause");
}

#[test]
fn exact_seed6_boundary_changes_only_unpaid_parent_phase_and_restarts() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../captures/hunter-charge-candidate-two-hour-3b06596/seed-6/facultative_on");
    if !dir.exists() {
        eprintln!("retained seed-6 artifact absent; exact artifact replay not executed");
        return;
    }
    let opening = std::fs::read(dir.join("post-initialization.cubw")).unwrap();
    let old_closing = std::fs::read(dir.join("closing.cubw")).unwrap();
    // Public decoder checks the old envelope/CRC before rejecting the semantic defect.
    assert!(
        decode_snapshot(&old_closing)
            .unwrap_err()
            .to_string()
            .contains("stalking no one")
    );
    assert_eq!(
        u32::from_le_bytes(old_closing[4..8].try_into().unwrap()),
        12
    );
    let build_len = u16::from_le_bytes(old_closing[8..10].try_into().unwrap()) as usize;
    // The payload is schema 12, so it is read through its frozen mirror; the live `WorldState`
    // grew the ordinary-quiet extension in schema 13 and can no longer decode these bytes
    // directly (`snapshot::v12`).
    let old: WorldStateV12 = postcard::from_bytes(&old_closing[22 + build_len..]).unwrap();
    assert_eq!(
        fnv1a(&postcard::to_allocvec(&old).unwrap()),
        9140998897574537509
    );
    let mut expected: WorldState = old.into();
    // Migration adds nothing but an inert Off extension.
    assert_eq!(expected.quiet, cubarium_core::quiet::QuietState::default());
    let parent = OrganismId {
        slot: 29,
        generation: 6,
    };
    let child = OrganismId {
        slot: 74,
        generation: 5,
    };
    let prey = OrganismId {
        slot: 63,
        generation: 5,
    };
    let m = expected.hunters.member_mut(parent).unwrap();
    assert_eq!(m.phase, HunterPhase::Stalking);
    assert_eq!(m.target, None);
    m.enter(HunterPhase::Perched, 170772, 170772, 0);
    expected.validate().unwrap();

    let mut world = World::from_state(decode_snapshot(&opening).unwrap().1).unwrap();
    for _ in 0..26771 {
        world.step();
        world.drain_events();
        world.drain_hunter_events();
    }
    // The recorded number is this world's schema 12 replay hash, so it is read off the schema
    // 12 projection — which is every field the recording covered. Schema 13's inert Off
    // extension moves the live hash and changes nothing it describes.
    assert_eq!(
        fnv1a(
            &postcard::to_allocvec(
                &cubarium_core::snapshot::v12::project(&world.state).expect("an Off world projects")
            )
            .unwrap()
        ),
        2258426608805215987,
        "every state field remains identical through the prior boundary"
    );
    // A separate fixture exports this prey before the paid strike settles. This probes lost
    // target semantics, not the accounting of an environmental removal or a biological trial.
    let mut missing = World::from_state(world.state.clone()).unwrap();
    let exported = missing.state.organisms.remove(prey).unwrap();
    missing.state.external_material_in -= exported.material();
    missing.state.hunters.forget_target(prey, 170771);
    let paid = *missing.hunters().member(child).unwrap();
    assert_eq!(paid.phase, HunterPhase::Strike);
    let attempted = missing.hunters().attacks_total;
    let bytes = encode_snapshot(&missing.state, "paid-target-gone");
    let mut missing_resumed = World::from_state(decode_snapshot(&bytes).unwrap().1).unwrap();
    missing.step();
    missing_resumed.step();
    assert_eq!(missing.state, missing_resumed.state);
    let lost = missing.drain_hunter_events();
    assert_eq!(lost, missing_resumed.drain_hunter_events());
    assert_eq!(lost.len(), 1);
    assert!(matches!(lost[0], HunterEvent::Attempt {
        hunter, outcome: cubarium_core::AttemptOutcome::TargetLost,
        energy_paid: 0.08, attack_counter: Some(1), ..
    } if hunter == child));
    assert_eq!(
        missing.hunters().attacks_total,
        attempted,
        "no new charge or retry"
    );
    let settled = missing.hunters().member(child).unwrap();
    assert_eq!(settled.phase, HunterPhase::Recovering);
    assert_eq!(settled.entered_from, HunterPhase::Strike);
    assert_eq!(settled.episode, paid.episode);
    world.step();
    assert_eq!(
        world.state, expected,
        "only the unpaid parent's phase history changes"
    );
    let events = world.drain_hunter_events();
    assert_eq!(events.len(), 2);
    assert!(
        matches!(events[0], HunterEvent::Attempt { hunter, energy_paid: 0.08, attack_counter: Some(1), .. } if hunter == child)
    );
    assert!(
        matches!(events[1], HunterEvent::Capture { hunter, prey: p, material, energy, .. }
        if hunter == child && p == prey && material == 0.6035863309232207 && energy == 1.0122994782514116)
    );
    world.drain_events();
    let encoded = encode_snapshot(&world.state, "astra-target-cleanup");
    let mut resumed = World::from_state(decode_snapshot(&encoded).unwrap().1).unwrap();
    for _ in 0..300 {
        world.step();
        resumed.step();
        world.check_invariants().unwrap();
        resumed.check_invariants().unwrap();
        assert_eq!(world.state, resumed.state);
        assert_eq!(world.drain_events(), resumed.drain_events());
        assert_eq!(world.drain_hunter_events(), resumed.drain_hunter_events());
    }
}
