//! Independent boundary probes for the shared-prey cleanup repair (512ee52).
use cubarium_core::{HunterMember, HunterPhase, HunterState, OrganismId, decode_snapshot};

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

/// **Retired by ecology v1** (`design/ecology-v1-contract.md` §15.1). This was an exact
/// artifact replay from the schema 12 seed-6 capture: load `post-initialization.cubw`, step
/// 26,771 ticks, and compare every field against the recorded closing state. Schema 16
/// refuses every older snapshot by name, so the artifact can no longer be loaded at all and
/// the replay is not re-anchorable from this repository — the capture is a schema 12 world
/// and this build cannot make one.
///
/// What is left is the refusal itself, checked against the real bytes when the retained
/// capture is present: the loader must name schema 12 rather than reinterpret it. The
/// mechanism the retired test probed — that a paid strike settles as `TargetLost` and an
/// unpaid pursuit is cleaned up — is covered without any artifact by
/// [`invalidation_is_full_id_scoped_and_preserves_paid_episode`] above and by
/// `tests/hunter.rs`.
#[test]
fn the_retained_seed6_artifacts_are_refused_by_name() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../captures/hunter-charge-candidate-two-hour-3b06596/seed-6/facultative_on");
    if !dir.exists() {
        eprintln!("retained seed-6 artifact absent; the refusal is checked on the fixtures instead");
        return;
    }
    for name in ["post-initialization.cubw", "closing.cubw"] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let schema = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert!(schema < 16, "{name} is schema {schema}");
        assert_eq!(
            decode_snapshot(&bytes),
            Err(cubarium_core::SnapshotError::UnsupportedSchema(schema)),
            "{name}: an old world is refused by name, never migrated"
        );
    }
}
