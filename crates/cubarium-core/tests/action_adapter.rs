//! Workstream X: **the turn deadband alone** — a second action adapter that differs from
//! `cub-act-1` in exactly one constant, the `TURN` band
//! (`design/handoffs/ecology-v1-turn-deadband-opus-2026-09-16.md`).
//!
//! Written from the brief's definitions **before** the implementation, in the order U and W
//! used. (A pinned whole-state hash of a default world once lived here too; it was dropped at
//! the ring-world merge, since a world's bytes are not something this project promises.)
//!
//! The one variable is `ActionAdapter`. `cub-act-1` is the shipped adapter and the default:
//! `DEADBAND = 0.05` on both `THRUST` and `TURN`. `cub-act-2` is the same adapter with the
//! `TURN` band at 0.0 and **nothing else changed** — the thrust band, the intake bands, the
//! `LEVEL` triggers, the capability masks, the shared-mouth normalisation and the order they
//! run in are all `cub-act-1`'s, arithmetic for arithmetic.
//!
//! Selection is a `World` transient beside `motor_model`: never persisted, never hashed, never
//! a `WorldConfig` field. The profile text — and therefore the schema digest — carries the
//! adapter's name, so a policy trained under one adapter is refused **by name** under the
//! other rather than silently reinterpreted.

use cubarium_core::neural::Policy;
use cubarium_core::neural::action::{
    ACT_LEN, ActionAdapter, Action7, ATTACK, Capability, DEADBAND, FRUIT, GRAZE, LEVEL,
    MOUTH_CHANNELS, REPRODUCE, SCAVENGE, THRUST, TURN,
};
use cubarium_core::neural::gru::{Gru32, HIDDEN, INPUT, N, R, Z};
use cubarium_core::neural::obs::FOOD_NEAR;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{World, WorldConfig, decode_snapshot, encode_snapshot};


fn grazer() -> Capability {
    Capability::ordinary(1.0, 0.0, true, true)
}

// -------------------------------------------------------------------------------------------
// 1. The one constant that differs.
// -------------------------------------------------------------------------------------------

/// The brief's own test: a raw turn head of ±0.03 is zero under `cub-act-1` and ±0.03 under
/// `cub-act-2`, **at the same thrust**.
///
/// `tanh(0.03) = 0.029991…`, inside the 0.05 band and outside a band of 0.0, so the squashed
/// value under `cub-act-2` is `tanh(0.03)` itself — the adapter releases the channel, it does
/// not rescale it.
#[test]
fn a_raw_turn_head_of_three_hundredths_is_released_only_by_the_second_adapter() {
    let cap = grazer();
    for sign in [1.0f64, -1.0] {
        let head = [0.0, sign * 0.03, 0.0, 0.0, 0.0, 0.0, 0.0];
        let one = Action7::squash_in(&head, &cap, ActionAdapter::CubAct1);
        let two = Action7::squash_in(&head, &cap, ActionAdapter::CubAct2);
        assert_eq!(one.0[TURN], 0.0, "cub-act-1 bands a turn head of {}", sign * 0.03);
        assert!(
            (two.0[TURN] - sign * 0.03_f64.tanh()).abs() < 1e-15,
            "cub-act-2 passes tanh(head) through: {}",
            two.0[TURN]
        );
        assert_eq!(two.0[TURN].signum(), sign, "and keeps the sign");
        // The same thrust in both arms: this is a turn-only change.
        assert_eq!(one.0[THRUST], two.0[THRUST]);
        assert_eq!(one.0[THRUST], 0.5, "sigmoid(0) is above the 0.05 thrust band");
    }
}

/// `squash` without an adapter is `cub-act-1`, and `ActionAdapter::default()` is `cub-act-1`.
#[test]
fn the_shipped_adapter_is_the_default_and_the_unqualified_call() {
    assert_eq!(ActionAdapter::default(), ActionAdapter::CubAct1);
    let cap = grazer();
    let head = [0.4, 0.03, 0.9, -0.2, 0.1, 0.3, 0.7];
    assert_eq!(
        Action7::squash(&head, &cap),
        Action7::squash_in(&head, &cap, ActionAdapter::CubAct1)
    );
}

/// The **thrust** band is unchanged: the same 0.05, on both sides of it, under both adapters.
#[test]
fn the_thrust_band_is_unchanged_under_both_adapters() {
    let cap = grazer();
    // `sigmoid(h) = 0.05` at `h = ln(0.05/0.95) = −2.9444…`. Just inside and just outside.
    let inside = (DEADBAND / (1.0 - DEADBAND)).ln() - 1e-6;
    let outside = (DEADBAND / (1.0 - DEADBAND)).ln() + 1e-6;
    for adapter in [ActionAdapter::CubAct1, ActionAdapter::CubAct2] {
        let a = Action7::squash_in(&[inside, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], &cap, adapter);
        assert_eq!(a.0[THRUST], 0.0, "{adapter:?} must still band a thrust below 0.05");
        let b = Action7::squash_in(&[outside, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], &cap, adapter);
        assert!(b.0[THRUST] > 0.0, "{adapter:?} must still pass a thrust above 0.05");
        assert!((b.0[THRUST] - DEADBAND).abs() < 1e-6);
    }
}

/// The three intake bands, the two `LEVEL` triggers, the capability masks and the shared-mouth
/// normalisation are the same under both adapters, for every head in a small spanning set.
#[test]
fn the_masks_the_intake_bands_and_the_normalisation_are_unchanged() {
    let heads: [[f64; ACT_LEN]; 6] = [
        [0.0; ACT_LEN],
        [4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0],
        [-4.0, -4.0, -4.0, -4.0, -4.0, -4.0, -4.0],
        [0.0, 0.03, 6.0, 6.0, 6.0, 6.0, 6.0],
        [-3.0, -0.02, -2.95, -2.94, 0.2, 0.4, 0.6],
        [1.0, 1.0, -2.95, 3.0, -2.94, -0.5, 0.5],
    ];
    let caps = [
        Capability::ordinary(1.0, 0.0, true, true),
        Capability::ordinary(0.0, 1.0, true, true),
        Capability::ordinary(1.0, 1.0, false, false),
        Capability::ordinary(1.0, 1.0, true, true),
    ];
    for head in &heads {
        for cap in &caps {
            let one = Action7::squash_in(head, cap, ActionAdapter::CubAct1);
            let two = Action7::squash_in(head, cap, ActionAdapter::CubAct2);
            for channel in [GRAZE, FRUIT, SCAVENGE, ATTACK, REPRODUCE, THRUST] {
                assert_eq!(
                    one.0[channel], two.0[channel],
                    "channel {channel} moved between the adapters on head {head:?} cap {cap:?}"
                );
            }
            // The mouth is still one mouth under the released turn band.
            let sum: f64 = MOUTH_CHANNELS.iter().map(|i| two.0[*i]).sum();
            assert!(sum <= 1.0 + 1e-12, "the mouth over-committed: {sum}");
            assert_eq!(one.reproduce(), two.reproduce());
            assert_eq!(LEVEL, 0.5, "the level trigger is untouched");
        }
    }
}

/// `cub-act-2` releases the turn band and **only** the turn band: a turn head outside the old
/// band decodes identically under both.
#[test]
fn a_turn_outside_the_old_band_is_identical_under_both_adapters() {
    let cap = grazer();
    for head_turn in [0.2_f64, -0.2, 1.5, -3.0] {
        let head = [0.4, head_turn, 0.0, 0.0, 0.0, 0.0, 0.0];
        let one = Action7::squash_in(&head, &cap, ActionAdapter::CubAct1);
        let two = Action7::squash_in(&head, &cap, ActionAdapter::CubAct2);
        assert_eq!(one.0, two.0, "head turn {head_turn} decoded differently");
    }
}

// -------------------------------------------------------------------------------------------
// 2. The profile text and the digest.
// -------------------------------------------------------------------------------------------

/// The two profile texts differ in **exactly** that one token, and the digests differ with
/// them. `cub-act-1`'s text — and so the digest every retained policy carries — is unchanged.
#[test]
fn the_profile_texts_differ_in_exactly_one_token() {
    let one = cubarium_core::neural::profile_text(ActionAdapter::CubAct1);
    let two = cubarium_core::neural::profile_text(ActionAdapter::CubAct2);
    assert_eq!(one, cubarium_core::neural::PROFILE_TEXT, "cub-act-1 keeps the shipped text");
    assert_ne!(one, two);
    assert_eq!(
        one.replace("cub-act-1", "cub-act-2"),
        two,
        "the only difference must be the adapter token"
    );
    assert!(one.contains("|cub-act-1|") && two.contains("|cub-act-2|"));

    assert_eq!(
        cubarium_core::neural::schema_digest(),
        cubarium_core::neural::schema_digest_in(ActionAdapter::CubAct1),
        "the unqualified digest is cub-act-1's"
    );
    assert_ne!(
        cubarium_core::neural::schema_digest_in(ActionAdapter::CubAct1),
        cubarium_core::neural::schema_digest_in(ActionAdapter::CubAct2)
    );
}

/// The name round-trips, and an unknown name is refused rather than defaulted.
#[test]
fn the_adapter_parses_and_prints_by_name() {
    for adapter in [ActionAdapter::CubAct1, ActionAdapter::CubAct2] {
        assert_eq!(ActionAdapter::parse(adapter.name()), Ok(adapter));
    }
    assert_eq!(ActionAdapter::CubAct1.name(), "cub-act-1");
    assert_eq!(ActionAdapter::CubAct2.name(), "cub-act-2");
    let err = ActionAdapter::parse("cub-act-3").expect_err("an unknown adapter is refused");
    assert!(err.contains("cub-act-3"), "{err}");
}

// -------------------------------------------------------------------------------------------
// 3. The world transient.
// -------------------------------------------------------------------------------------------

fn remembering_policy() -> Gru32 {
    let mut w = Gru32::zeros();
    w.b_i[R] = 4.0;
    w.b_i[Z] = 2.0;
    w.w_i[N * INPUT + FOOD_NEAR] = 6.0;
    w.w_h[N * HIDDEN] = 1.0;
    w.b_i[N] = -2.0;
    w.w_o[0] = 5.0;
    w.w_o[HIDDEN] = -5.0;
    w
}

/// A bias-only head whose raw turn is `0.03` on every controller tick: inside the `cub-act-1`
/// band, outside a released one. Thrust is `sigmoid(0) = 0.5`, well clear of its own band, so
/// the body moves in both arms and the **only** difference between them is whether it turns.
fn small_turn_policy() -> Gru32 {
    let mut w = Gru32::zeros();
    w.b_o[THRUST] = 0.0;
    w.b_o[TURN] = 0.03;
    w
}

/// Build the fixture: a default world, 1,000 ticks of ordinary life, eight of its bodies
/// dispatched to `weights`, then 3,000 further ticks. The adapter is named on the world before
/// the first neural tick, exactly as `set_motor_model` is.
fn fixture(weights: &Gru32, adapter: ActionAdapter) -> World {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    for _ in 0..1_000 {
        world.step();
        world.drain_events();
    }
    if adapter != ActionAdapter::CubAct1 {
        world.set_action_adapter(adapter).expect("no policy is attached yet");
    }
    let ids: Vec<_> = world.state.organisms.iter().map(|(id, _)| id).collect();
    let mut attached = 0;
    for id in ids {
        if attached >= 8 {
            break;
        }
        if world
            .attach_neural_policy(id, Policy::new_in(weights.clone(), adapter))
            .is_ok()
        {
            attached += 1;
        }
    }
    assert_eq!(attached, 8, "the fixture needs eight neural animals");
    for _ in 0..3_000 {
        world.step();
        world.drain_events();
    }
    world.check_invariants().expect("the fixture world stays consistent");
    world
}


/// The switch is live: the same fixture under `cub-act-2` is a **different** world from the one
/// under `cub-act-1`, because a turn head of 0.03 is now a turn.
#[test]
fn naming_the_second_adapter_changes_what_the_body_does() {
    let under_act1 = state_hash(&fixture(&small_turn_policy(), ActionAdapter::CubAct1).state);
    let under_act2 = state_hash(&fixture(&small_turn_policy(), ActionAdapter::CubAct2).state);
    assert_ne!(
        under_act1, under_act2,
        "releasing the turn band must change a world whose turn head sits inside it"
    );
}

/// The transient is not in the state: a snapshot round trip of a `cub-act-2` world comes back
/// `cub-act-1`, exactly as `motor_model` does, and the encoded bytes are the same either way.
#[test]
fn the_adapter_is_transient_and_never_persisted() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    assert_eq!(world.action_adapter(), ActionAdapter::CubAct1, "the default");
    for _ in 0..50 {
        world.step();
    }
    let before = encode_snapshot(&world.state, "test-build");
    world.set_action_adapter(ActionAdapter::CubAct2).expect("no policy is attached");
    assert_eq!(world.action_adapter(), ActionAdapter::CubAct2);
    let after = encode_snapshot(&world.state, "test-build");
    assert_eq!(before, after, "naming an adapter must not reach the snapshot");

    let state = decode_snapshot(&after).expect("round trip").1;
    let resumed = World::from_state(state).expect("valid");
    assert_eq!(
        resumed.action_adapter(),
        ActionAdapter::CubAct1,
        "a resumed world runs the shipped adapter until it is told otherwise"
    );
}

/// The same contract at the live boundary (Astra, round-5 review P1): once a policy is
/// attached, the world cannot be switched to an adapter that policy was not authored for. The
/// refusal names both adapters and leaves the world unchanged; a world whose policies match
/// the requested adapter, or that has none, may still change. Both directions.
#[test]
fn a_live_adapter_change_that_mismatches_an_attached_policy_is_refused_and_atomic() {
    let mut world = fixture(&remembering_policy(), ActionAdapter::CubAct1);
    let before = encode_snapshot(&world.state, "test-build");
    let err = match world.set_action_adapter(ActionAdapter::CubAct2) {
        Ok(()) => panic!("a cub-act-1 policy must not be decoded under cub-act-2"),
        Err(e) => e,
    };
    assert!(err.contains("cub-act-2") && err.contains("cub-act-1"), "{err}");
    assert_eq!(world.action_adapter(), ActionAdapter::CubAct1, "unchanged");
    assert_eq!(encode_snapshot(&world.state, "test-build"), before, "untouched");
    world.set_action_adapter(ActionAdapter::CubAct1).expect("a matching request is a no-op");

    let mut world = fixture(&remembering_policy(), ActionAdapter::CubAct2);
    let err = match world.set_action_adapter(ActionAdapter::CubAct1) {
        Ok(()) => panic!("a cub-act-2 policy must not be decoded under cub-act-1"),
        Err(e) => e,
    };
    assert!(err.contains("cub-act-1") && err.contains("cub-act-2"), "{err}");
    assert_eq!(world.action_adapter(), ActionAdapter::CubAct2, "unchanged");
    world.set_action_adapter(ActionAdapter::CubAct2).expect("a matching request is a no-op");
}

/// A snapshot holding a `cub-act-2` policy cannot be resumed: the adapter is a transient the
/// bytes do not carry, a resumed world runs the shipped adapter, and decoding those weights
/// under it would be silent and wrong. Refused by name at `World::from_state`, which is the
/// one door a snapshot comes through (Fable, at X's integration). A `cub-act-1` world with a
/// policy round-trips as before.
#[test]
fn a_snapshot_with_a_second_adapter_policy_is_refused_on_resume_by_name() {
    let world = fixture(&remembering_policy(), ActionAdapter::CubAct2);
    let bytes = encode_snapshot(&world.state, "test-build");
    let state = decode_snapshot(&bytes).expect("the bytes decode").1;
    let err = match World::from_state(state) {
        Ok(_) => panic!("a cub-act-2 policy must not resume under the shipped adapter"),
        Err(e) => e,
    };
    assert!(err.contains("cub-act-2") && err.contains("cub-act-1"), "{err}");

    let world = fixture(&remembering_policy(), ActionAdapter::CubAct1);
    let bytes = encode_snapshot(&world.state, "test-build");
    let state = decode_snapshot(&bytes).expect("the bytes decode").1;
    let resumed = World::from_state(state).expect("a shipped-adapter policy resumes");
    assert_eq!(resumed.action_adapter(), ActionAdapter::CubAct1);
}

/// A policy stamped for one adapter is refused **by name** by a world running the other, at the
/// one explicit door into the extension, and the world is left as it was found.
#[test]
fn a_policy_from_the_other_adapter_is_refused_by_name_at_attachment() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    for _ in 0..200 {
        world.step();
        world.drain_events();
    }
    let id = world.state.organisms.iter().map(|(i, _)| i).next().expect("a body");

    // A `cub-act-2` policy into the shipped world.
    let foreign = Policy::new_in(remembering_policy(), ActionAdapter::CubAct2);
    let err = world.attach_neural_policy(id, foreign).expect_err("refused");
    assert!(err.contains("schema_digest"), "{err}");
    assert!(
        err.contains(&format!(
            "{:#018x}",
            cubarium_core::neural::schema_digest_in(ActionAdapter::CubAct2)
        )),
        "the refusal names the policy's digest: {err}"
    );
    assert!(world.neural().get(id).is_none(), "nothing was attached");

    // And the other direction: the shipped policy into a `cub-act-2` world.
    world.set_action_adapter(ActionAdapter::CubAct2).expect("nothing was attached");
    let shipped = Policy::new(remembering_policy());
    let err = world.attach_neural_policy(id, shipped).expect_err("refused");
    assert!(err.contains("schema_digest"), "{err}");
    assert!(world.neural().get(id).is_none(), "nothing was attached");

    // Its own adapter's policy is accepted, and the world stays valid.
    world
        .attach_neural_policy(id, Policy::new_in(remembering_policy(), ActionAdapter::CubAct2))
        .expect("its own adapter attaches");
    for _ in 0..100 {
        world.step();
        world.drain_events();
    }
    world.check_invariants().expect("consistent");
}
