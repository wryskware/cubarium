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
