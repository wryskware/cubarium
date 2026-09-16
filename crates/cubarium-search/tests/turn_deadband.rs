//! Workstream X: the protocol, the provenance and the refusals the second action adapter adds
//! (`design/handoffs/ecology-v1-turn-deadband-opus-2026-09-16.md`).
//!
//! Written from the brief's definitions before the implementation. The brief asks for the
//! host-side refusal to be tested **here** rather than in `crates/cubarium` — another worker
//! owns that crate this round — so "the host" is exercised as what it actually is: a `World`
//! that never names an adapter, which is `cub-act-1`.
//!
//! The `cub-act-1` arm of every check below is the shipped adapter: a file that names no
//! adapter reads as `cub-act-1`, and a `cub-act-2` file is refused by name.

use cubarium_core::neural::{ActionAdapter, Policy, gru::Gru32};
use cubarium_core::{MotorModel, World, WorldConfig};
use cubarium_search::es::export::PolicyFile;
use cubarium_search::es::turnband::{ALPHA, sign_test, two_sided_binomial};
use cubarium_search::es::{fixture, tensor};

/// An arbitrary ecology config hash: the policy file carries it verbatim, so any value does.
const FAST_LEAF_CONFIG_HASH: u64 = 0x09e2_4439_2ec9_1768;

// -------------------------------------------------------------------------------------------
// The policy file's provenance.
// -------------------------------------------------------------------------------------------

/// A policy file records the adapter by name; a file that records none reads as `cub-act-1`,
/// which is what every file written before the switch in fact ran.
#[test]
fn a_policy_file_records_the_adapter_and_a_missing_one_is_the_shipped_adapter() {
    let theta = tensor::initial_center(20_260_915);
    let two = PolicyFile::new_in(
        &theta,
        "test",
        0xdead_beef,
        9,
        "fast-leaf",
        FAST_LEAF_CONFIG_HASH,
        MotorModel::Sweep,
        ActionAdapter::CubAct2,
    )
    .expect("exportable");
    assert_eq!(two.adapter.as_deref(), Some("cub-act-2"));
    assert_eq!(two.adapter().expect("known"), ActionAdapter::CubAct2);
    assert_eq!(
        two.policy_digest,
        cubarium_core::neural::schema_digest_in(ActionAdapter::CubAct2)
    );
    two.check_adapter(ActionAdapter::CubAct2).expect("its own adapter");
    let err = two.check_adapter(ActionAdapter::CubAct1).expect_err("the other one is refused");
    assert!(err.contains("cub-act-2"), "the refusal names the file's adapter: {err}");
    assert!(err.contains("cub-act-1"), "and the evaluation's: {err}");
    assert!(err.contains("not the same task"), "{err}");

    // `PolicyFile::new` is the shipped adapter, and the round trip keeps the field.
    let one = PolicyFile::new(
        &theta,
        "test",
        0xdead_beef,
        9,
        "fast-leaf",
        FAST_LEAF_CONFIG_HASH,
        MotorModel::Sweep,
    )
    .expect("exportable");
    assert_eq!(one.adapter.as_deref(), Some("cub-act-1"));
    assert_eq!(one.policy_digest, cubarium_core::neural::schema_digest());
    let back: PolicyFile =
        serde_json::from_str(&serde_json::to_string(&two).expect("write")).expect("read");
    assert_eq!(back, two);

    // A file written before the field existed: absent from the JSON, so `None`, which is
    // `cub-act-1` — not "unknown".
    let mut value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&one).expect("write")).expect("read");
    value.as_object_mut().expect("object").remove("adapter");
    let old: PolicyFile = serde_json::from_value(value).expect("an older file still parses");
    assert_eq!(old.adapter, None);
    assert_eq!(old.adapter().expect("read as the shipped adapter"), ActionAdapter::CubAct1);
    old.check_adapter(ActionAdapter::CubAct1).expect("accepted");
    assert!(old.check_adapter(ActionAdapter::CubAct2).is_err());
    assert!(old.policy().is_ok(), "the weights themselves are still loadable");

    // And a `cub-act-2` file rebuilds into a policy stamped for `cub-act-2`.
    let policy = two.policy().expect("loadable");
    assert_eq!(policy.adapter(), Some(ActionAdapter::CubAct2));
}

/// The weights are untouched by the adapter: restamping a theta for the other adapter changes
/// the digest and nothing else, bit for bit. This is what makes the paired replay a replay.
#[test]
fn restamping_a_theta_for_the_other_adapter_moves_no_weight() {
    let theta = tensor::initial_center(7);
    let one = tensor::policy(&theta).expect("a policy");
    let two = tensor::policy_in(&theta, ActionAdapter::CubAct2).expect("a policy");
    assert_ne!(one.schema_digest, two.schema_digest);
    for (a, b) in tensor::flatten(&one.weights).iter().zip(tensor::flatten(&two.weights)) {
        assert_eq!(a.to_bits(), b.to_bits());
    }
    assert_eq!(one.weights, two.weights);
}

// -------------------------------------------------------------------------------------------
// The host's refusal.
// -------------------------------------------------------------------------------------------

/// **The host stays on `cub-act-1`.** A world that never names an adapter — which is every
/// world the display builds — refuses a `cub-act-2` policy by name, at both doors into the
/// extension, and is left exactly as it was found.
#[test]
fn the_host_refuses_a_cub_act_2_policy_by_name() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    assert_eq!(world.action_adapter(), ActionAdapter::CubAct1, "the host names no adapter");
    for _ in 0..200 {
        world.step();
        world.drain_events();
    }
    let before = world.population();

    let foreign = Policy::new_in(Gru32::zeros(), ActionAdapter::CubAct2);
    let id = world.state.organisms.iter().map(|(i, _)| i).next().expect("a body");
    let err = world.attach_neural_policy(id, foreign.clone()).expect_err("refused");
    assert!(err.contains("cub-act-2"), "the refusal names the policy's adapter: {err}");
    assert!(err.contains("cub-act-1"), "and the world's: {err}");
    assert!(world.neural().get(id).is_none());

    // `found_neural_animal` is the other door, and it refuses before the body exists.
    let pos = cubarium_surface::CellId::new(cubarium_surface::Topology::Cube, cubarium_surface::Scale::ONE, cubarium_surface::Face::Top, 8, 8).center(cubarium_surface::Topology::Cube, cubarium_surface::Scale::ONE);
    let heading = cubarium_surface::Vec2::new(1.0, 0.0);
    let err = world.found_neural_animal(pos, heading, foreign).expect_err("refused");
    assert!(err.contains("cub-act-2"), "{err}");
    assert_eq!(world.population(), before, "a refused seed founds nothing");

    // And the shipped policy still attaches, so the refusal is about the adapter and not about
    // the door.
    world
        .attach_neural_policy(id, Policy::new(Gru32::zeros()))
        .expect("the shipped adapter attaches");
}

/// A fixture set built for `cub-act-2` builds worlds that are on `cub-act-2`, and a set built
/// without naming one is on the shipped adapter.
#[test]
fn a_layout_puts_its_world_on_its_own_adapter() {
    let plain = &fixture::training_layouts()[0];
    let (world, _) = plain.build().expect("builds");
    assert_eq!(world.action_adapter(), ActionAdapter::CubAct1);

    let released = plain.clone().with_adapter(ActionAdapter::CubAct2);
    let (world, id) = released.build().expect("builds");
    assert_eq!(world.action_adapter(), ActionAdapter::CubAct2);
    // And the layout hash does not move with it.
    assert_eq!(plain.hash(&plain.config()), released.hash(&released.config()));

    // A `cub-act-2` policy runs in it, for ordinary core inference.
    let mut world = world;
    let policy =
        tensor::policy_in(&tensor::initial_center(11), ActionAdapter::CubAct2).expect("a policy");
    world.attach_neural_policy(id, policy).expect("its own adapter attaches");
    for _ in 0..200 {
        world.step();
        world.drain_events();
    }
    world.check_invariants().expect("the world stays consistent");
}

// -------------------------------------------------------------------------------------------
// The decision rule's arithmetic.
// -------------------------------------------------------------------------------------------

/// The exact binomial the rule is read through, against values computed by hand, and the
/// threshold the rule uses.
#[test]
fn the_decision_rules_sign_test_is_exact() {
    assert_eq!(ALPHA, 0.1);
    // C(4,k)/16: two-sided at k = 4 is 2·(1/16) = 0.125.
    assert!((two_sided_binomial(4, 4) - 0.125).abs() < 1e-12);
    // k = 2 of 4 is the centre: 2·min(11/16, 11/16) = 1.375, capped at 1.
    assert!((two_sided_binomial(2, 4) - 1.0).abs() < 1e-12);
    // 100 of 132: far below 0.1 and finite, which is what the log-space terms are for.
    let mut d = vec![1.0; 100];
    d.extend(vec![-1.0; 32]);
    let t = sign_test(&d);
    assert!(t.p_value > 0.0 && t.p_value < 1e-8, "p = {}", t.p_value);
    assert!(t.rises());
}
