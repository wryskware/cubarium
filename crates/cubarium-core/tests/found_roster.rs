//! The fourth founding door: [`World::found_roster`]
//! (`design/handoffs/ecology-v1-precondition-opus-2026-09-16.md`, deliverable 1).
//!
//! Workstream M's option A wants the world's plants advanced on their own for a declared
//! number of ticks and *then* the ordinary roster founded into whatever field that produced.
//! No existing door can do it. `World::new` places the roster only at creation, before a
//! single plant tick has run; `found_training_animal` and `found_animal_with_genome` place one
//! named body with the *training* stores (`TRAINING_START_RESERVE`, `Mode::Seeking`), not a
//! founder with `founders.initial_reserve_fraction` and `Mode::Resting`. A preconditioned
//! opening founded through either of those would be a different cohort from the one every
//! result so far was measured in, and the comparison it exists for would be worthless.
//!
//! So the door is tested from its definition, not from what it happens to produce:
//!
//! 1. **It is the constructor's own founding.** Founding into a fresh tick-0 world whose
//!    roster was cleared before construction — with the roster written back into the config
//!    first — yields a world whose `state_hash` equals the constructor's. That is the whole
//!    claim in one number: same genomes, same positions, same headings, same stores, same
//!    material book, same config.
//! 2. **It survives a plant-only prefix.** After 1,000 ticks with no animals the same
//!    genomes land at the same positions: the draws are keyed on the seed and the founder
//!    index, so the field the world has grown in the meantime cannot move them.
//! 3. **It refuses by name** — animals already present, and a config that declares no
//!    roster — and a refused call leaves the world exactly as it found it.
//! 4. **The material box closes**: `structure + reserve` for every founder is booked into
//!    `external_material_in`, and the world's own audits pass afterwards.

use cubarium_core::organism::{Mode, Origin};
use cubarium_core::snapshot::state_hash;
use cubarium_core::{World, WorldConfig};

fn full(seed: u64) -> WorldConfig {
    WorldConfig { seed, ..WorldConfig::default() }
}

/// The shipped config with its founder roster emptied — workstream M's `--no-animals` arm
/// (`world/lifecycle.rs:52-63`): no kind, no count, so the constructor places nobody.
fn bare(seed: u64) -> WorldConfig {
    let mut c = full(seed);
    c.founders.kinds.clear();
    c.founders.count = 0;
    c
}

/// A plant-only world at tick 0, with the roster written back into its config so the door has
/// one to found. Writing it back is what makes the founded world's *config* — and therefore
/// its hash — the ordinary one; the door reads the roster from the world it is founding into.
fn preconditioned(seed: u64, ticks: u64) -> World {
    let mut w = World::new(bare(seed)).expect("a world with no founders is an ordinary world");
    for _ in 0..ticks {
        w.step();
        w.drain_events();
    }
    w.state.config.founders = full(seed).founders;
    w
}

// --- 1. the door is the constructor's own founding --------------------------------------

/// The one number the whole door rests on. If these two hashes differ, a preconditioned
/// opening is not comparable with the status quo it is being compared against.
#[test]
fn founding_into_a_fresh_world_reproduces_the_constructor_exactly() {
    for seed in [1u64, 1001, 4242] {
        let mut w = preconditioned(seed, 0);
        assert_eq!(w.tick(), 0, "nothing was stepped");
        let ids = w.found_roster().expect("a fresh world accepts the roster");
        let reference = World::new(full(seed)).expect("the shipped defaults build a world");

        assert_eq!(
            ids.len(),
            reference.state.organisms.len(),
            "seed {seed}: the door places every founder the constructor places"
        );
        assert_eq!(
            state_hash(&w.state),
            state_hash(&reference.state),
            "seed {seed}: the founded world must be the constructor's world, bit for bit"
        );
    }
}

/// The founders the door places are founders: adult, `Origin::Founder`, `Mode::Resting`, and
/// born at tick 0 here because that is the tick the world is at. Stated separately from the
/// hash so a future change to the hash's encoding cannot quietly take this with it.
#[test]
fn the_founded_bodies_are_the_constructors_founders_field_for_field() {
    let mut w = preconditioned(7, 0);
    w.found_roster().expect("a fresh world accepts the roster");
    let reference = World::new(full(7)).expect("the shipped defaults build a world");

    let mine: Vec<_> = w.state.organisms.iter().map(|(_, o)| o.clone()).collect();
    let theirs: Vec<_> = reference.state.organisms.iter().map(|(_, o)| o.clone()).collect();
    assert_eq!(mine.len(), theirs.len(), "the same number of founders");
    assert!(!mine.is_empty(), "the shipped roster is not empty");
    for (a, b) in mine.iter().zip(&theirs) {
        assert_eq!(a.pos, b.pos, "same cell");
        assert_eq!(a.heading, b.heading, "same heading");
        assert_eq!(a.genome, b.genome, "same genome");
        assert_eq!(a.structure, b.structure, "same structure");
        assert_eq!(a.reserve, b.reserve, "same reserve");
        assert_eq!(a.energy, b.energy, "same energy");
        assert_eq!(a.hunger_memory, b.hunger_memory, "same hunger memory");
        assert_eq!(a.mode, Mode::Resting, "a founder opens resting");
        assert_eq!(b.mode, Mode::Resting, "so does the constructor's");
        assert_eq!(a.origin, Origin::Founder, "a founder is a founder");
        assert_eq!(a.born_tick, 0, "founded at tick 0");
    }
}

// --- 2. a plant-only prefix does not move the roster -------------------------------------

/// The founder draws are keyed on `(seed, Stream::Founders, index)` and nothing else, so a
/// thousand ticks of plant dynamics in between cannot move a founder to another cell or give
/// it another genome. Only `born_tick` follows the clock, which is the one thing that should.
#[test]
fn a_plant_only_prefix_places_the_same_genomes_at_the_same_places() {
    const PREFIX: u64 = 1_000;
    let mut warm = preconditioned(1001, PREFIX);
    assert_eq!(warm.tick(), PREFIX, "the prefix really ran");
    assert_eq!(warm.population(), 0, "and ran with nobody in the world");
    warm.found_roster().expect("a plant-only world accepts the roster");

    let cold = World::new(full(1001)).expect("the shipped defaults build a world");
    let mine: Vec<_> = warm.state.organisms.iter().map(|(_, o)| o.clone()).collect();
    let theirs: Vec<_> = cold.state.organisms.iter().map(|(_, o)| o.clone()).collect();
    assert_eq!(mine.len(), theirs.len(), "the same roster");
    for (a, b) in mine.iter().zip(&theirs) {
        assert_eq!(a.genome, b.genome, "the genome is drawn from the seed, not the field");
        assert_eq!(a.pos, b.pos, "and so is the position");
        assert_eq!(a.heading, b.heading, "and the heading");
        assert_eq!(a.phenotype.form, b.phenotype.form, "and therefore the form");
        assert_eq!(a.born_tick, PREFIX, "born when it was founded, not at tick 0");
        assert_eq!(b.born_tick, 0, "the constructor's founders were born at tick 0");
    }
    // The field is genuinely a different one: this is not the same world with a later clock.
    assert_ne!(
        warm.state.fields.p, cold.state.fields.p,
        "a thousand plant-only ticks moved the foliage"
    );
}

/// The whole point of the door: the world it founds into keeps its own grown field, and the
/// run that follows is an ordinary run. A thousand more ticks with the roster present step
/// without tripping an audit.
#[test]
fn a_preconditioned_world_steps_on_normally_after_founding() {
    let mut w = preconditioned(1002, 1_000);
    w.found_roster().expect("a plant-only world accepts the roster");
    assert!(w.population() > 0, "there are animals now");
    for _ in 0..1_000 {
        w.step();
        w.drain_events();
    }
    w.check_invariants().expect("a preconditioned world obeys the same invariants");
}

// --- 3. the refusals ---------------------------------------------------------------------

/// Founding a second roster on top of the first would double the cohort and double-book the
/// material. The door refuses by name, and the refusal changes nothing.
#[test]
fn a_world_that_already_holds_animals_refuses_by_name() {
    let mut w = World::new(full(3)).expect("the shipped defaults build a world");
    let before = state_hash(&w.state);
    let refusal = w.found_roster().expect_err("a populated world must refuse");
    assert!(
        refusal.contains("already holds"),
        "the refusal must say why, not just fail: {refusal}"
    );
    assert_eq!(state_hash(&w.state), before, "a refused founding touches nothing");
}

/// A caller that cleared the roster to run plant-only and forgot to write it back would
/// otherwise get a silent empty founding — a status-quo arm with no animals in it, reported
/// as if it had 24. That is the failure this refusal exists to make loud.
#[test]
fn a_config_with_no_roster_refuses_by_name() {
    let mut w = World::new(bare(4)).expect("a world with no founders is an ordinary world");
    let before = state_hash(&w.state);
    let refusal = w.found_roster().expect_err("an empty roster must refuse");
    assert!(
        refusal.contains("no founders"),
        "the refusal must name the empty roster: {refusal}"
    );
    assert_eq!(state_hash(&w.state), before, "a refused founding touches nothing");
    assert_eq!(w.population(), 0, "and founds nobody");
}

// --- 4. the material box ------------------------------------------------------------------

/// The founders arrive from outside the world's material box, so every one of them is booked
/// in `external_material_in` exactly as the constructor books them — and the world's own
/// conservation audit, which is what would catch a mis-booking, passes.
#[test]
fn the_founders_material_is_booked_as_arriving_from_outside() {
    let mut w = preconditioned(1003, 600);
    let before = w.state.external_material_in;
    assert_eq!(before, 0.0, "a plant-only world has imported nothing");
    w.found_roster().expect("a plant-only world accepts the roster");
    let imported: f64 = w.state.organisms.iter().map(|(_, o)| o.structure + o.reserve).sum();
    assert!(imported > 0.0, "the cohort is made of something");
    assert!(
        (w.state.external_material_in - imported).abs() < 1e-12,
        "every founder's structure and reserve is booked: {} against {imported}",
        w.state.external_material_in
    );
    w.check_invariants().expect("the material box still closes");
}

// --- 5. the mirror door: removing the population -------------------------------------------
//
// Workstream Z's coupled grazed opening (`design/handoffs/
// ecology-v1-grazed-opening-opus-2026-09-16.md`, deliverable 1) needs the *other* half of the
// door: burn a field in with an ordinary roster, take that burn-in population out again, and
// found the identical fresh roster into the field it grazed. `World::remove_all_animals` is
// that operator, and it is tested from its definition:
//
// 1. it leaves **nothing** alive and hands back exactly what it took;
// 2. the field, the water, the weather and every pool are **byte-identical** across it;
// 3. the material it removed is booked out of the world's net external-material counter, so
//    a burn-in-then-found world's conservation identity still closes;
// 4. `found_roster` succeeds after it, with `born_tick` at the removal tick;
// 5. the removed-then-founded world steps on and keeps its invariants over 2,000 ticks;
// 6. removal on an empty world is a no-op that books zero;
// 7. it refuses, by name and without touching the world, when an extension holds state keyed
//    to the bodies it would remove.

/// An ordinary coupled world, burnt in for `ticks` with its own roster present. This is the
/// burn-in workstream Z's stage runs; a plant-only prefix is a different world entirely.
fn burnt_in(seed: u64, ticks: u64) -> World {
    let mut w = World::new(full(seed)).expect("the shipped defaults build a world");
    for _ in 0..ticks {
        w.step();
        w.drain_events();
    }
    w
}

/// Every pool the operator promises not to touch, as plain numbers.
fn pools(w: &World) -> Vec<Vec<f64>> {
    vec![
        w.state.fields.p.to_vec(),
        w.state.fields.n.to_vec(),
        w.state.fields.d.to_vec(),
        w.state.fields.de.to_vec(),
        w.state.fields.f.to_vec(),
        w.state.fields.w.to_vec(),
        w.state.ecology.wood.to_vec(),
        w.state.ecology.plant_reserve.to_vec(),
        w.state.ecology.dead_wood.to_vec(),
        w.state.ecology.carrion.to_vec(),
        w.state.ecology.carrion_energy.to_vec(),
    ]
}

/// The core claim, in two numbers: nobody is left, and what left the world is exactly the
/// material the bodies held — escrow included, because a gestating body's escrow is part of
/// the material the conservation identity sums over it.
#[test]
fn removal_leaves_nobody_and_books_their_material_to_the_digit() {
    let mut w = burnt_in(1001, 4_000);
    let before = w.population();
    assert!(before > 0, "a coupled burn-in has animals in it");
    let held: f64 = w.state.organisms.iter().map(|(_, o)| o.material()).sum();
    let booked_in = w.state.external_material_in;

    let removed = w.remove_all_animals().expect("an ordinary world accepts the removal");

    assert_eq!(w.population(), 0, "nobody is left");
    assert_eq!(removed.len(), before as usize, "every body is handed back");
    let exported: f64 = removed.iter().map(|o| o.material()).sum();
    assert_eq!(exported, held, "what was handed back is what was held");
    assert_eq!(
        w.state.external_material_in,
        booked_in - held,
        "the removal is the exact mirror of the founding's booking"
    );
    assert!(
        w.mass_residual().abs() < 1e-9,
        "the material box still closes: residual {}",
        w.mass_residual()
    );
    w.check_invariants().expect("an emptied world obeys the same invariants");
}

/// The operator is about the animals and nothing else. Every field, every ecology v1 pool and
/// the weather are the same values afterwards — not close, the same.
#[test]
fn the_field_is_byte_identical_across_the_removal() {
    let mut w = burnt_in(1002, 4_000);
    let before = pools(&w);
    let weather = w.state.weather.clone();
    let tick = w.tick();
    let (rain_in, evap_out) = (w.state.rain_in_total, w.state.evap_out_total);

    w.remove_all_animals().expect("an ordinary world accepts the removal");

    assert_eq!(pools(&w), before, "every cell of every pool is untouched");
    assert_eq!(w.state.weather, weather, "the weather is untouched");
    assert_eq!(w.tick(), tick, "the clock does not move");
    assert_eq!(w.state.rain_in_total, rain_in, "the water ledgers are untouched");
    assert_eq!(w.state.evap_out_total, evap_out, "both of them");
    assert!(w.water_residual().abs() < 1e-9, "and the water budget still closes");
}

/// The door workstream Z actually walks through: burn in, remove, found again. The fresh
/// roster is the *identical* roster — the same genomes at the same cells the constructor
/// would place — and it is born at the removal tick, not at tick 0.
#[test]
fn the_identical_roster_can_be_founded_after_a_removal() {
    const BURN_IN: u64 = 4_000;
    let mut w = burnt_in(1003, BURN_IN);
    w.remove_all_animals().expect("an ordinary world accepts the removal");
    let ids = w.found_roster().expect("an emptied world accepts the roster");

    let cold = World::new(full(1003)).expect("the shipped defaults build a world");
    let mine: Vec<_> = w.state.organisms.iter().map(|(_, o)| o.clone()).collect();
    let theirs: Vec<_> = cold.state.organisms.iter().map(|(_, o)| o.clone()).collect();
    assert_eq!(ids.len(), theirs.len(), "the same roster, body for body");
    assert_eq!(mine.len(), theirs.len(), "and the world holds exactly it");
    for (a, b) in mine.iter().zip(&theirs) {
        assert_eq!(a.genome, b.genome, "the genome is drawn from the seed, not the world");
        assert_eq!(a.pos, b.pos, "and so is the position");
        assert_eq!(a.heading, b.heading, "and the heading");
        assert_eq!(a.structure, b.structure, "adult at founding, as the constructor founds");
        assert_eq!(a.reserve, b.reserve, "with the configured opening reserve");
        assert_eq!(a.mode, Mode::Resting, "a founder opens resting");
        assert_eq!(a.origin, Origin::Founder, "a founder is a founder");
        assert_eq!(a.born_tick, BURN_IN, "born at the removal tick, not at tick 0");
    }
    // The field is the grazed one, not a fresh seeding: that is the entire point.
    assert_ne!(w.state.fields.p, cold.state.fields.p, "the field is the one the burn-in left");
    assert!(w.mass_residual().abs() < 1e-9, "and the material box closes across both doors");
}

/// A world that has been emptied and re-founded is an ordinary world. Two thousand ticks is
/// the brief's span; the invariants are checked every tick in a debug build, which is what
/// the test profile runs, and the audits again at the end.
#[test]
fn a_removed_then_founded_world_steps_on_and_keeps_its_invariants() {
    let mut w = burnt_in(1004, 3_000);
    w.remove_all_animals().expect("an ordinary world accepts the removal");
    w.found_roster().expect("an emptied world accepts the roster");
    for _ in 0..2_000 {
        w.step();
        w.drain_events();
    }
    w.check_invariants().expect("a removed-then-founded world obeys the same invariants");
    assert!(
        w.mass_residual().abs() < 1e-9,
        "and its material residual stays inside the acceptance: {}",
        w.mass_residual()
    );
    assert!(w.water_residual().abs() < 1e-9, "as does the water budget");
}

/// Nothing to remove is not an error, and it must not invent an export. A world that founded
/// nobody is the case this actually happens in.
#[test]
fn removal_on_an_empty_world_is_a_no_op_that_books_zero() {
    let mut w = World::new(bare(5)).expect("a world with no founders is an ordinary world");
    for _ in 0..600 {
        w.step();
        w.drain_events();
    }
    let before = state_hash(&w.state);
    let booked = w.state.external_material_in;

    let removed = w.remove_all_animals().expect("an empty world accepts the removal");

    assert!(removed.is_empty(), "nothing was removed");
    assert_eq!(w.state.external_material_in, booked, "and nothing was booked");
    assert_eq!(state_hash(&w.state), before, "the world is the same world");
}

/// The refusal. The operator empties the ordinary roster and its descendants; it has no
/// accounted policy for a hunter's carried carcass, an open quiet pause, a dormant apex or a
/// paired gestation, and dropping one silently would corrupt exactly the ledger the extension
/// exists to keep. A refused call leaves the world untouched.
#[test]
fn a_world_whose_extensions_hold_bodies_refuses_by_name() {
    use cubarium_core::organism::Mode as OMode;
    use cubarium_core::quiet::{QuietPause, QuietPolicy};

    let mut w = burnt_in(1005, 600);
    let victim = w.state.organisms.iter().map(|(id, _)| id).next().expect("somebody is alive");
    w.state.quiet.policy = QuietPolicy::PostBirthPauseV1;
    w.state.quiet.pauses.push(QuietPause {
        parent: victim,
        child: victim,
        start_tick: w.tick(),
        end_tick: w.tick() + 10,
        underlying: OMode::Seeking,
    });
    let before = state_hash(&w.state);
    let population = w.population();

    let refusal = w.remove_all_animals().expect_err("an extension holding bodies must refuse");
    assert!(
        refusal.contains("quiet"),
        "the refusal must name which extension holds them: {refusal}"
    );
    assert_eq!(state_hash(&w.state), before, "a refused removal touches nothing");
    assert_eq!(w.population(), population, "and removes nobody");
}

/// The removal door also refuses transient body-keyed state (Astra, round-5 review P2): a
/// scripted intent, a recording body-budget ledger, an intake trace target or an undrained
/// life event. Each is refused by name and the world is left as it was; clearing it lets the
/// removal proceed.
#[test]
fn removal_refuses_transient_body_keyed_state_by_name_and_leaves_the_world_as_it_was() {
    use cubarium_core::diagnostic::ScriptedIntent;
    let mut world = World::new(WorldConfig::default()).expect("valid");
    for _ in 0..200 {
        world.step();
    }
    let id = world.state.organisms.iter().map(|(id, _)| id).next().expect("a founder");

    // Undrained events: step until the world has queued one (a birth or a death), then the
    // removal must refuse by name and leave the world exactly as it was.
    let mut refused_for_events = false;
    for _ in 0..12_000 {
        world.step();
        if world.pending_events() == 0 {
            continue;
        }
        let before = state_hash(&world.state);
        let err = world.remove_all_animals().err().expect("undrained events are refused");
        assert!(err.contains("undrained"), "{err}");
        assert_eq!(state_hash(&world.state), before, "refusal must not move the world");
        refused_for_events = true;
        break;
    }
    assert!(refused_for_events, "twelve thousand ticks queued no life event");
    world.drain_events();
    let before = state_hash(&world.state);

    world.set_scripted_intents(vec![(id, ScriptedIntent::default())]);
    let err = world.remove_all_animals().err().expect("a scripted intent is refused");
    assert!(err.contains("scripted"), "{err}");
    assert_eq!(state_hash(&world.state), before);
    assert_eq!(world.scripted_intents().len(), 1, "the intent itself is left in place");
    world.set_scripted_intents(Vec::new());

    world.record_body_budgets(true);
    let err = world.remove_all_animals().err().expect("a recording ledger is refused");
    assert!(err.contains("ledger"), "{err}");
    assert_eq!(state_hash(&world.state), before);
    assert!(world.body_budgets_recording(), "the ledger is left recording");
    world.record_body_budgets(false);

    world.trace_intake(Some(id));
    let err = world.remove_all_animals().err().expect("an intake trace is refused");
    assert!(err.contains("intake trace"), "{err}");
    assert_eq!(state_hash(&world.state), before);
    assert_eq!(world.intake_trace_target(), Some(id), "the trace target is left in place");
    world.trace_intake(None);

    let removed = world.remove_all_animals().expect("nothing holds a body now");
    assert!(!removed.is_empty());
    assert!(world.state.organisms.iter().next().is_none());
}

/// An extension's event queue holds the door too (Astra, round-5 review, finding 11): a world
/// with hunters introduced has hunter events queued (and members present); the removal names
/// the queue, and the queue is left exactly as long as it was.
#[test]
fn removal_refuses_an_undrained_extension_event_queue_and_leaves_it_queued() {
    use cubarium_core::hunter::{FixedHunterProfile, HunterTarget};
    let mut world = World::new(WorldConfig::default()).expect("valid");
    for _ in 0..1_000 {
        world.step();
    }
    world.drain_events();
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    world
        .introduce_hunters(profile, &[HunterTarget { face: 1, u: 23.0, v: 31.0 }])
        .expect("one adult is placed");
    // Hunter events are emitted by the hunt itself, not by the introduction: step until the
    // adult has attempted something, draining only the ordinary life events meanwhile.
    for _ in 0..30_000 {
        if world.pending_extension_events()[0] > 0 {
            break;
        }
        world.step();
        world.drain_events();
    }
    let queued = world.pending_extension_events();
    assert!(queued[0] > 0, "a hunting adult queues hunter events within 30,000 ticks: {queued:?}");
    let err = world.remove_all_animals().err().expect("refused");
    assert!(err.contains("hunter events are queued"), "{err}");
    assert_eq!(world.pending_extension_events(), queued, "the queue is left as it was");
}
