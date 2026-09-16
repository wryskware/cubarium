//! Workstream X: the protocol, the provenance and the refusals the second action adapter adds
//! (`design/handoffs/ecology-v1-turn-deadband-opus-2026-09-16.md`).
//!
//! Written from the brief's definitions before the implementation. The brief asks for the
//! host-side refusal to be tested **here** rather than in `crates/cubarium` — another worker
//! owns that crate this round — so "the host" is exercised as what it actually is: a `World`
//! that never names an adapter, which is `cub-act-1`.
//!
//! The `cub-act-1` arm of every check below is a **pin**: the protocol hash, the layout hashes
//! and the policy digest the retained `runs/es-eco-v1-fastleaf` run carries must all survive
//! this workstream unchanged, or the retained rows stop being comparable with anything.

use cubarium_core::neural::{ActionAdapter, Policy, gru::Gru32};
use cubarium_core::{MotorModel, World, WorldConfig};
use cubarium_search::es::export::PolicyFile;
use cubarium_search::es::turnband::{ALPHA, sign_test, two_sided_binomial};
use cubarium_search::es::{fixture, tensor, trainer};

/// The protocol hash of the retained `fast-leaf` training run, from its own `train.log`
/// (`runs/es-eco-v1-fastleaf/train.log`: `protocol hash 0x8e51a1a9b1e2742b`). It is pinned on
/// the **shipped** adapter, so this is the check that `cub-act-1` is still the task it was.
const RETAINED_PROTOCOL_HASH: u64 = 0x8e51_a1a9_b1e2_742b;
/// And that run's ecology, from the same log.
const FAST_LEAF_CONFIG_HASH: u64 = 0x09e2_4439_2ec9_1768;

fn fast_leaf() -> Option<fixture::Ecology> {
    // `runs/` is git-ignored: the retained configuration lives in the main checkout, and a
    // clone without it must skip rather than fail.
    let path = std::path::Path::new("../../runs/ecology-v1-calibration/selected/fast-leaf.toml");
    let path = if path.exists() {
        path.to_path_buf()
    } else {
        let alt = std::path::Path::new("runs/ecology-v1-calibration/selected/fast-leaf.toml");
        if !alt.exists() {
            return None;
        }
        alt.to_path_buf()
    };
    fixture::Ecology::load(&path).ok()
}

// -------------------------------------------------------------------------------------------
// The protocol.
// -------------------------------------------------------------------------------------------

/// A `cub-act-1` protocol keeps the hash it has always had; a `cub-act-2` protocol is a
/// different task with a different hash **and** a different policy digest, and the two differ in
/// that field alone.
#[test]
fn the_shipped_protocol_keeps_its_hash_and_the_second_adapter_moves_it() {
    let one = trainer::Protocol::new(16, 36_000, 20_260_915, &fixture::training_layouts());
    assert_eq!(one.adapter, ActionAdapter::CubAct1, "the default is the shipped adapter");
    assert_eq!(
        one.policy_digest,
        cubarium_core::neural::schema_digest(),
        "and its digest is the build's own"
    );

    let two_layouts: Vec<_> = fixture::training_layouts()
        .into_iter()
        .map(|l| l.with_adapter(ActionAdapter::CubAct2))
        .collect();
    let two = trainer::Protocol::new(16, 36_000, 20_260_915, &two_layouts);
    assert_eq!(two.adapter, ActionAdapter::CubAct2);
    assert_ne!(one.hash(), two.hash(), "a released turn band is a different task");
    assert_ne!(one.policy_digest, two.policy_digest);

    // Everything else is identical: the two protocols differ in the adapter and the digest it
    // implies, and in nothing else.
    let mut same = two.clone();
    same.adapter = ActionAdapter::CubAct1;
    same.policy_digest = one.policy_digest;
    assert_eq!(same, one, "the adapter must not move any other protocol field");

    // The layout hashes do not move: the adapter is not part of a layout's geometry.
    assert_eq!(one.layout_hashes, two.layout_hashes);
}

/// The retained run's own protocol hash, reproduced exactly on the shipped adapter.
#[test]
fn the_retained_runs_protocol_hash_is_unchanged() {
    let Some(eco) = fast_leaf() else {
        eprintln!("skipped: runs/ecology-v1-calibration/selected/fast-leaf.toml is not here");
        return;
    };
    assert_eq!(eco.hash, FAST_LEAF_CONFIG_HASH, "this is the run's own ecology");
    let protocol =
        trainer::Protocol::new(16, 36_000, 20_260_915, &fixture::training_layouts_on(&eco));
    assert_eq!(
        protocol.hash(),
        RETAINED_PROTOCOL_HASH,
        "workstream X must not move the retained run's protocol hash"
    );
}

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
    let pos = cubarium_surface::CellId::new(cubarium_surface::Face::Top, 8, 8).center();
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

/// A `cub-act-2` **policy file** is refused by the host's loader too, before the weights are
/// rebuilt: `PolicyFile::policy` checks the digest against the adapter the file names, and the
/// digest it names is not the one a `cub-act-1` world runs.
#[test]
fn a_cub_act_2_policy_file_does_not_load_into_a_shipped_world() {
    let theta = tensor::initial_center(3);
    let file = PolicyFile::new_in(
        &theta,
        "test",
        0,
        0,
        "default",
        0xc0ff_ee,
        MotorModel::Sweep,
        ActionAdapter::CubAct2,
    )
    .expect("exportable");
    let policy = file.policy().expect("its own adapter's file loads");
    let mut world = World::new(WorldConfig::default()).expect("valid");
    for _ in 0..50 {
        world.step();
        world.drain_events();
    }
    let id = world.state.organisms.iter().map(|(i, _)| i).next().expect("a body");
    assert!(
        world.attach_neural_policy(id, policy).is_err(),
        "the shipped world must refuse it"
    );

    // A file whose recorded digest and recorded adapter disagree is refused by name rather than
    // reinterpreted under either.
    let mut tampered = file.clone();
    tampered.adapter = Some("cub-act-1".into());
    let err = tampered.policy().expect_err("refused");
    assert!(err.contains("cub-act-1"), "{err}");
    assert!(err.contains("cannot"), "{err}");
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

/// A trajectory trace changes nothing about the episode it measures: the traced and untraced
/// rollouts of one policy on one layout are equal field for field.
#[test]
fn a_traced_episode_is_the_same_episode() {
    use cubarium_search::es::episode::{self, Driver, Limits};
    use std::sync::atomic::AtomicBool;

    let layout = &fixture::training_layouts()[0];
    let policy = tensor::policy(&tensor::initial_center(20_260_915)).expect("a policy");
    let driver = Driver::Policy(Box::new(policy));
    let cancel = AtomicBool::new(false);
    let plain = episode::run(layout, &driver, 3_000, Limits::new(&cancel), "plain").expect("ran");
    let (traced, trace) =
        episode::run_traced(layout, &driver, 3_000, Limits::new(&cancel), "traced").expect("ran");
    assert_eq!(plain, traced, "switching the trace on must not change the episode");
    assert_eq!(trace.trace_rows_dropped, 0, "the drain cadence must keep the recorder empty");
    assert!(trace.traced_ticks > 0, "the trace covered the life");
    assert!(
        trace.turn_measured_ticks <= plain.ticks,
        "the turn cannot be measured on more ticks than were simulated"
    );
    assert!(
        (trace.turn_abs_rad - plain.turn_sweep_rad).abs() < 1e-12,
        "the trace's turn sum is the episode's: {} vs {}",
        trace.turn_abs_rad,
        plain.turn_sweep_rad
    );
    assert_eq!(
        trace.dwell_bout_ticks, trace.on_food_ticks,
        "every on-food tick belongs to exactly one bout"
    );
    assert!(trace.dwell_bouts as u64 <= trace.on_food_ticks);
}
