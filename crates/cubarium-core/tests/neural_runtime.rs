//! The recurrent runtime through the real world (R1a, `design/recurrent-interface-contract.md`
//! §9): per-animal dispatch, the memory the interface can carry, and exact persistence.
//!
//! None of this is a claim about learning. A fixed hand-authored weight set proves that the
//! *interface* carries state and that the world reads it; what a trained policy would do is
//! not measured anywhere here.

use cubarium_core::config::FounderKind;
use cubarium_core::ids::OrganismId;
use cubarium_core::neural::gru::{Gru32, HIDDEN, INPUT, N, OUTPUT, R, Z};
use cubarium_core::neural::obs::{FOOD_NEAR, OBS_LEN};
use cubarium_core::neural::{AnimalState, NeuralState, Policy};
use cubarium_core::organism::Mode;
use cubarium_core::{
    LifeEvent, SCHEMA_V14, World, WorldConfig, decode_snapshot, encode_snapshot,
    snapshot::state_hash,
};
use cubarium_surface::{Face, SurfacePoint};

const BUILD: &str = "r1a-neural";

fn calm(founders: usize) -> WorldConfig {
    let mut c = WorldConfig::default();
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    c.founders.kinds = vec![FounderKind {
        name: "size-1".into(),
        count: founders as u32,
        size: Some(1.0),
        ..FounderKind::default()
    }];
    c.founders.count = founders as u32;
    c
}

fn ids(world: &World) -> Vec<OrganismId> {
    world.state.organisms.iter().map(|(id, _)| id).collect()
}

/// A hand-authored policy that **remembers**: unit 0 integrates the own-cell producer channel
/// with a slow update gate, and the head turns that memory into thrust. Nothing about it is
/// trained, and nothing here claims it is good at anything.
fn remembering_policy() -> Gru32 {
    let mut w = Gru32::zeros();
    // r: wide open, so the recurrent term is not gated away.
    w.b_i[R] = 4.0;
    // z: mostly retain. σ(2.0) ≈ 0.88 of the old state survives each update.
    w.b_i[Z] = 2.0;
    // n: driven by the first near-ring producer sector, and by the unit's own history.
    w.w_i[N * INPUT + FOOD_NEAR] = 6.0;
    w.w_h[N * HIDDEN] = 1.0;
    w.b_i[N] = -2.0;
    // Head: thrust and turn both read the memory, so a difference in `hidden` shows in the
    // held action rather than only inside the network.
    w.w_o[0] = 5.0;
    w.w_o[HIDDEN + 0] = -5.0;
    w
}

/// Run one policy over a fixed sequence of observation vectors and return the held action the
/// adapter would latch after the last one.
fn run_sequence(weights: &Gru32, sequence: &[[f64; OBS_LEN]]) -> [f64; OUTPUT] {
    let mut hidden = [0.0f64; HIDDEN];
    let mut y = [0.0f64; OUTPUT];
    for x in sequence {
        y = weights.forward(x, &mut hidden);
    }
    y
}

fn vector(p_near: f64) -> [f64; OBS_LEN] {
    let mut v = [0.0f64; OBS_LEN];
    v[FOOD_NEAR] = p_near;
    v[69] = 1.0;
    v
}

// ------------------------------------------------------------------ the memory check

/// Two histories, one final observation. The interface carries the difference into the held
/// action; a hidden reset removes it. This proves the *interface* can carry memory, and
/// nothing about learning.
#[test]
fn two_histories_ending_in_the_same_observation_give_different_actions() {
    let w = remembering_policy();
    let rich: Vec<[f64; OBS_LEN]> = vec![vector(1.0), vector(1.0), vector(1.0), vector(0.4)];
    let bare: Vec<[f64; OBS_LEN]> = vec![vector(0.0), vector(0.0), vector(0.0), vector(0.4)];
    assert_eq!(
        rich.last(),
        bare.last(),
        "the two sequences must end on the identical observation"
    );

    let a = run_sequence(&w, &rich);
    let b = run_sequence(&w, &bare);
    let gap = (a[0] - b[0]).abs();
    assert!(
        gap > 0.5,
        "the same final input after different histories gave {a:?} and {b:?}: gap {gap}"
    );

    // The control: with the hidden state reset, only the final observation remains, and the
    // two are identical.
    let reset_a = run_sequence(&w, &rich[3..]);
    let reset_b = run_sequence(&w, &bare[3..]);
    assert_eq!(
        reset_a, reset_b,
        "after a hidden reset the history cannot matter"
    );
}

// ------------------------------------------------------------------ dispatch

/// A neural animal is dispatched away from the legacy controller: its mode label follows its
/// own activation, it moves under the contract's own request, and its hidden state changes.
#[test]
fn a_neural_animal_runs_its_own_controller_and_moves() {
    let mut world = World::new(calm(1)).expect("valid");
    let id = ids(&world)[0];
    // Full thrust, no turning, mouths shut: a bias-only head, so the action never varies.
    let mut w = Gru32::zeros();
    w.b_o[0] = 8.0; // thrust: σ(8) ≈ 1
    w.b_o[2] = -8.0;
    w.b_o[3] = -8.0;
    w.b_o[4] = -8.0;
    world
        .attach_neural_policy(id, Policy::new(w))
        .expect("attach");

    let before = world.state.organisms.get(id).expect("alive").pos;
    for _ in 0..40 {
        world.step();
        world.drain_events();
    }
    let o = world.state.organisms.get(id).expect("alive");
    assert_eq!(o.mode, Mode::Seeking, "an active neural body is not resting");
    assert!(
        o.pos != before,
        "a neural body asking for full thrust did not move"
    );
    assert!(world.neural().contains(id));
    world.check_invariants().unwrap();
}

/// The zero policy is a real request: every channel is at its squash midpoint, so thrust and
/// turn are inside the deadband and the body is still.
#[test]
fn a_zero_policy_holds_the_body_exactly_still() {
    let mut world = World::new(calm(1)).expect("valid");
    let id = ids(&world)[0];
    world
        .attach_neural_policy(id, Policy::new(Gru32::zeros()))
        .expect("attach");
    let before = world.state.organisms.get(id).expect("alive").pos;
    let heading = world.state.organisms.get(id).expect("alive").heading;
    for _ in 0..20 {
        world.step();
        world.drain_events();
    }
    let o = world.state.organisms.get(id).expect("alive");
    // σ(0) = 0.5 for thrust — above the deadband — but tanh(0) = 0 for turn, so the heading
    // must be untouched and the travel must be along it.
    assert_eq!(o.heading, heading, "a zero turn channel never turns the body");
    assert!(o.pos != before, "σ(0) thrust is a real half-speed request");
    world.check_invariants().unwrap();
}

/// The cadence: every animal updates on every second tick, and the two phases split the load.
#[test]
fn the_controller_runs_on_every_second_tick_for_each_animal() {
    let mut world = World::new(calm(2)).expect("valid");
    let list = ids(&world);
    for id in &list {
        world
            .attach_neural_policy(*id, Policy::new(remembering_policy()))
            .expect("attach");
    }
    // Both founders are born on tick 0, so both carry phase 0 here; the phase is the birth
    // tick's parity, which is what splits a *growing* population across the two ticks.
    for id in &list {
        let a = world.neural().get(*id).expect("attached");
        assert_eq!(a.phase, 0);
    }
    let mut updates = 0;
    let mut previous = world.neural().get(list[0]).expect("attached").hidden.clone();
    for _ in 0..8 {
        world.step();
        world.drain_events();
        let now = world.neural().get(list[0]).expect("attached").hidden.clone();
        if now != previous {
            updates += 1;
        }
        previous = now;
    }
    assert!(
        (3..=4).contains(&updates),
        "8 ticks should carry 4 controller updates, saw {updates}"
    );
}

// ------------------------------------------------------------------ persistence

/// Uninterrupted and save-then-resume agree on `state_hash` at every later tick, for a world
/// whose animals sit on **both** cadence phases and which crosses a seam.
#[test]
fn a_resumed_neural_world_agrees_with_an_uninterrupted_one_across_both_phases_and_a_seam() {
    let mut world = World::new(calm(6)).expect("valid");
    let list = ids(&world);
    for id in &list {
        world
            .attach_neural_policy(*id, Policy::new(remembering_policy()))
            .expect("attach");
    }
    // Force one animal onto the other phase and park another on a seam, so the resume has to
    // carry both cases.
    if let Some(a) = world.state.neural.get_mut(list[0]) {
        a.phase = 1;
    }
    if let Some(o) = world.state.organisms.get_mut(list[1]) {
        o.pos = SurfacePoint {
            face: Face::Top,
            u: 63.6,
            v: 32.0,
        };
    }
    world.state.validate().expect("still a valid world");
    let mut world = World::from_state(world.state).expect("valid");

    for _ in 0..60 {
        world.step();
        world.drain_events();
    }
    let bytes = encode_snapshot(&world.state, BUILD);
    let (meta, reloaded) = decode_snapshot(&bytes).expect("it decodes");
    assert_eq!(meta.schema, cubarium_core::SCHEMA_VERSION);
    assert_eq!(reloaded, world.state, "the round trip is not lossless");
    assert!(!reloaded.neural.animals.is_empty(), "the animals persisted");
    let mut resumed = World::from_state(reloaded).expect("valid");

    for tick in 0..120 {
        world.step();
        world.drain_events();
        resumed.step();
        resumed.drain_events();
        assert_eq!(
            state_hash(&world.state),
            state_hash(&resumed.state),
            "the resumed world diverged {} tick(s) after the save",
            tick + 1
        );
    }
}

/// A schema-14 world loads with every organism legacy-controlled and steps identically to one
/// that never had the extension. No silent replacement, no reset.
#[test]
fn a_schema_fourteen_world_loads_all_legacy_and_steps_identically() {
    let mut reference = World::new(calm(8)).expect("valid");
    for _ in 0..40 {
        reference.step();
        reference.drain_events();
    }
    // Build a schema-14 payload by hand from the projection, exactly as the old build wrote it.
    let projected =
        cubarium_core::snapshot::v14::project(&reference.state).expect("no neural animal exists");
    let payload = postcard::to_allocvec(&projected).expect("encodes");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"CUBW");
    bytes.extend_from_slice(&SCHEMA_V14.to_le_bytes());
    bytes.extend_from_slice(&(BUILD.len() as u16).to_le_bytes());
    bytes.extend_from_slice(BUILD.as_bytes());
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    bytes.extend_from_slice(&payload);

    let (meta, migrated) = decode_snapshot(&bytes).expect("a schema 14 world still loads");
    assert_eq!(meta.schema, SCHEMA_V14, "the meta reports what was read");
    assert_eq!(
        migrated.neural,
        NeuralState::default(),
        "every organism is legacy-controlled after the migration"
    );
    assert_eq!(state_hash(&migrated), state_hash(&reference.state));

    let mut migrated = World::from_state(migrated).expect("valid");
    for tick in 0..200 {
        reference.step();
        reference.drain_events();
        migrated.step();
        migrated.drain_events();
        assert_eq!(
            state_hash(&reference.state),
            state_hash(&migrated.state),
            "the migrated world diverged at tick {tick}"
        );
    }
}

/// A legacy world with no neural animal runs the pre-extension tick: the three inert bytes are
/// the whole difference, and every legacy body still uses the legacy controller.
#[test]
fn a_world_with_no_neural_animal_never_enters_a_neural_code_path() {
    let mut world = World::new(calm(12)).expect("valid");
    for _ in 0..300 {
        world.step();
        world.drain_events();
    }
    assert!(world.neural().is_empty());
    assert!(
        world
            .state
            .organisms
            .iter()
            .any(|(_, o)| o.hunger_memory > 0.0),
        "legacy bodies still run the hunger memory the contract removes for neural ones"
    );
    world.check_invariants().unwrap();
}

/// A death removes the entry, and the slot a later body reuses starts from zero hidden state.
#[test]
fn a_reused_slot_starts_from_zero_hidden_state() {
    let mut world = World::new(calm(2)).expect("valid");
    let list = ids(&world);
    let doomed = list[0];
    world
        .attach_neural_policy(doomed, Policy::new(remembering_policy()))
        .expect("attach");
    for _ in 0..6 {
        world.step();
        world.drain_events();
    }
    assert!(
        world.neural().get(doomed).expect("attached").hidden.iter().any(|h| *h != 0.0),
        "the fixture needs a nonzero hidden state to prove it does not survive"
    );

    // Starve it: it cannot pay this tick's upkeep, so it dies before intake settles.
    {
        let o = world.state.organisms.get_mut(doomed).expect("alive");
        o.energy = 0.0;
        o.reserve = 0.0;
    }
    world.step();
    let deaths: Vec<OrganismId> = world
        .drain_events()
        .into_iter()
        .filter_map(|e| match e {
            LifeEvent::Death { id, .. } => Some(id),
            _ => None,
        })
        .collect();
    assert!(deaths.contains(&doomed), "the fixture body died: {deaths:?}");
    assert!(
        !world.neural().contains(doomed),
        "the private state went at the same boundary the body did"
    );

    // The same slot, a later generation: a different key, and nothing to inherit.
    let reused = OrganismId {
        slot: doomed.slot,
        generation: doomed.generation + 1,
    };
    assert!(!world.neural().contains(reused));
    let mut state = NeuralState::default();
    let p = state.intern(Policy::new(Gru32::zeros()));
    state.insert(reused, AnimalState::fresh(world.tick(), p));
    assert!(
        state.get(reused).expect("inserted").hidden.iter().all(|h| *h == 0.0),
        "a reused slot starts from zero"
    );
    world.check_invariants().unwrap();
}

/// A funded ordinary birth by a neural parent produces a neural child with the parent's policy
/// and **fresh** private state.
#[test]
fn a_funded_ordinary_birth_gives_the_child_the_parents_policy_and_fresh_state() {
    let mut config = calm(1);
    config.organism.gestation_seconds = 0.5;
    let mut world = World::new(config).expect("valid");
    let parent = ids(&world)[0];
    // Reproduce held high, mouths shut, still: a standing request the world funds when it can.
    let mut w = Gru32::zeros();
    w.b_o[0] = -8.0;
    w.b_o[2] = -8.0;
    w.b_o[3] = -8.0;
    w.b_o[4] = -8.0;
    w.b_o[6] = 8.0;
    world
        .attach_neural_policy(parent, Policy::new(w))
        .expect("attach");
    {
        // Fund it: adult, full stores, old enough for the world's own maturity gate.
        let o = world.state.organisms.get_mut(parent).expect("alive");
        let before = o.reserve;
        o.structure = o.phenotype.structure_adult;
        o.reserve = o.phenotype.reserve_max;
        o.energy = o.phenotype.energy_max;
        let added = o.reserve - before;
        world.state.external_material_in += added;
        world.state.organisms.get_mut(parent).expect("alive").born_tick = 0;
    }
    let mut world = World::from_state(world.state).expect("valid");

    let mut child = None;
    for _ in 0..4200 {
        world.step();
        for e in world.drain_events() {
            if let LifeEvent::Birth { id, .. } = e
                && id != parent
            {
                child = Some(id);
            }
        }
        if child.is_some() {
            break;
        }
    }
    let child = child.expect("the neural parent funded and delivered a child");
    let parent_policy = world.neural().get(parent).expect("still neural").policy;
    let born = world.neural().get(child).expect("the child is neural");
    assert_eq!(born.policy, parent_policy, "the child copies the policy");
    assert!(
        born.hidden.iter().all(|h| *h == 0.0),
        "and starts from a fresh private state"
    );
    assert_eq!(born.feedback, cubarium_core::neural::Feedback::default());
    world.check_invariants().unwrap();
}

/// The refusals the contract names, through the public door.
#[test]
fn unsupported_combinations_and_foreign_policies_are_refused_by_name() {
    let mut world = World::new(calm(1)).expect("valid");
    let id = ids(&world)[0];

    let mut foreign = Policy::new(Gru32::zeros());
    foreign.schema_digest ^= 0x5eed;
    let err = world.attach_neural_policy(id, foreign).unwrap_err();
    assert!(err.contains("schema_digest"), "{err}");

    let missing = OrganismId {
        slot: 900,
        generation: 0,
    };
    let err = world
        .attach_neural_policy(missing, Policy::new(Gru32::zeros()))
        .unwrap_err();
    assert!(err.contains("not alive"), "{err}");

    // Quiet and neural together have no contract.
    let mut quiet = World::new(calm(1)).expect("valid");
    let qid = ids(&quiet)[0];
    quiet.state.quiet.policy = cubarium_core::quiet::QuietPolicy::PostBirthPauseV1;
    let err = quiet
        .attach_neural_policy(qid, Policy::new(Gru32::zeros()))
        .unwrap_err();
    assert!(err.contains("quiet"), "{err}");
}
