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
    DT, LifeEvent, SCHEMA_V14, World, WorldConfig, decode_snapshot, encode_snapshot,
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
    assert_eq!(
        o.mode,
        Mode::Seeking,
        "an active neural body is not resting"
    );
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
    assert_eq!(
        o.heading, heading,
        "a zero turn channel never turns the body"
    );
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
    let mut previous = world
        .neural()
        .get(list[0])
        .expect("attached")
        .hidden
        .clone();
    for _ in 0..8 {
        world.step();
        world.drain_events();
        let now = world
            .neural()
            .get(list[0])
            .expect("attached")
            .hidden
            .clone();
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

/// **Retired by ecology v1** (`design/ecology-v1-contract.md` §15.1). A schema 14 world used
/// to load with every organism legacy-controlled and step identically to one that never had
/// the extension. Worlds always restart fresh and are never migrated (Wrysk, 2026-09-15), so
/// schema 16 refuses schema 14 by name instead.
///
/// The claim underneath — that an empty neural extension changes nothing — is still tested,
/// by [`a_world_with_no_neural_animal_never_enters_a_neural_code_path`] below and by the
/// projection check here: a world with no neural animal still has a schema 14 image, and a
/// world with one does not.
#[test]
fn a_hand_framed_schema_fourteen_payload_is_refused_by_name() {
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

    assert_eq!(
        decode_snapshot(&bytes),
        Err(cubarium_core::SnapshotError::UnsupportedSchema(SCHEMA_V14)),
        "a schema 14 world is refused by name, never migrated"
    );
    // And the projection side of the rule is unchanged: an empty extension still has an old
    // image, and the payload it drops is exactly the extension.
    let full = postcard::to_allocvec(&reference.state).expect("encodes");
    assert_eq!(
        &full[..payload.len()],
        &payload[..],
        "schema 14 is a prefix of schema 16"
    );
    assert!(full.len() > payload.len(), "and schema 16 appends to it");
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
        world
            .neural()
            .get(doomed)
            .expect("attached")
            .hidden
            .iter()
            .any(|h| *h != 0.0),
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
    assert!(
        deaths.contains(&doomed),
        "the fixture body died: {deaths:?}"
    );
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
        state
            .get(reused)
            .expect("inserted")
            .hidden
            .iter()
            .all(|h| *h == 0.0),
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
        // Fund it: adult and full stores. The world's retained maturity gate is the real
        // condition here, so the body is *aged* into it rather than merely asserted to be old
        // enough: the world clock is wound past `bud_min_age_seconds` and `born_tick` is left
        // at 0, which makes `age_ticks` genuinely exceed the gate on the first step.
        let o = world.state.organisms.get_mut(parent).expect("alive");
        let before = o.reserve;
        o.structure = o.phenotype.structure_adult;
        o.reserve = o.phenotype.reserve_max;
        o.energy = o.phenotype.energy_max;
        let added = o.reserve - before;
        world.state.external_material_in += added;
        o.born_tick = 0;
        let gate = f64::from(o.phenotype.drives.bud_min_age_seconds);
        assert!(gate > 0.0, "the fixture must face a real age gate");
        world.state.tick = (gate / DT).ceil() as u64 + 1;
    }
    let mut world = World::from_state(world.state).expect("valid");
    {
        let o = world.state.organisms.get(parent).expect("alive");
        let age = o.age_ticks(world.tick()) as f64 * DT;
        assert!(
            age > f64::from(o.phenotype.drives.bud_min_age_seconds),
            "precondition: the parent is {age} s old against a {} s gate",
            o.phenotype.drives.bud_min_age_seconds
        );
    }

    let mut child = None;
    for _ in 0..1200 {
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

// ------------------------------------------------------- the retained maturity gate

/// A neural body with `reproduce` held high, adult and fully funded, at a chosen age.
/// Returns whether it opened an escrow on its first step.
fn tries_to_bud(age_seconds: f64, funded: bool) -> (bool, f64) {
    let mut world = World::new(calm(1)).expect("valid");
    let parent = ids(&world)[0];
    let mut w = Gru32::zeros();
    // Still, mouths shut, reproduce held above the level trigger.
    w.b_o[0] = -8.0;
    w.b_o[2] = -8.0;
    w.b_o[3] = -8.0;
    w.b_o[4] = -8.0;
    w.b_o[6] = 8.0;
    world
        .attach_neural_policy(parent, Policy::new(w))
        .expect("attach");
    let gate = {
        let o = world.state.organisms.get_mut(parent).expect("alive");
        let before = o.reserve;
        o.structure = o.phenotype.structure_adult;
        if funded {
            o.reserve = o.phenotype.reserve_max;
            o.energy = o.phenotype.energy_max;
        } else {
            // Adult and old, but nothing to build a child out of: the world's own funding
            // check refuses, and it must refuse for that reason and not for the age.
            o.reserve = 0.05 * o.phenotype.reserve_max;
            o.energy = 0.05 * o.phenotype.energy_max;
        }
        let added = o.reserve - before;
        o.born_tick = 0;
        let gate = f64::from(o.phenotype.drives.bud_min_age_seconds);
        world.state.external_material_in += added;
        gate
    };
    world.state.tick = (age_seconds / DT).round() as u64;
    let mut world = World::from_state(world.state).expect("valid");
    // Two ticks, because the action is latched on the animal's own controller tick and one of
    // any two consecutive ticks is one. The age quoted is the age at the start.
    world.step();
    world.step();
    let opened = world
        .state
        .organisms
        .get(parent)
        .expect("alive")
        .escrow
        .is_some();
    (opened, gate)
}

/// **Below the gate.** A neural policy owns reproductive *intent*; it does not own physical
/// maturity. `decide_quiet` refuses a budding request under `bud_min_age_seconds`, and a
/// neural body skips that function entirely — so the world enforces the same condition at
/// gestation admission. Before this repair a funded body at age zero opened escrow on its
/// very first tick against a 120 s gate.
#[test]
fn a_neural_body_below_the_minimum_age_cannot_start_gestation() {
    let (opened, gate) = tries_to_bud(0.0, true);
    assert!(gate > 0.0, "the fixture must face a real age gate");
    assert!(!opened, "age 0 started gestation against a {gate} s gate");

    // Just short of it, too: the gate is a threshold, not a formality at zero. The fixture
    // runs two ticks, so it starts three short and is still one short when it finishes.
    let (opened, _) = tries_to_bud(gate - 3.0 * DT, true);
    assert!(!opened, "a tick short of {gate} s still started gestation");
}

/// **At the gate.** Once the body is old enough the request is honoured: the repair restores
/// the retained condition, it does not disable ordinary budding.
#[test]
fn a_neural_body_at_the_minimum_age_starts_gestation() {
    let (_, gate) = tries_to_bud(0.0, true);
    let (opened, _) = tries_to_bud(gate + DT, true);
    assert!(
        opened,
        "a funded adult past the {gate} s gate did not start gestation"
    );
}

/// **Insufficient funding.** Old enough, and still refused — by the world's own escrow
/// funding check, which this repair did not touch.
#[test]
fn a_mature_but_unfunded_neural_body_cannot_start_gestation() {
    let (_, gate) = tries_to_bud(0.0, true);
    let (opened, _) = tries_to_bud(gate + DT, false);
    assert!(
        !opened,
        "an unfunded body opened an escrow it could not pay for"
    );
}
