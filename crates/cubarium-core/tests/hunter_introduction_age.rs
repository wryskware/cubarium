//! The introduction door's **optional age**: one founder placed as if it had already lived
//! for a stated number of seconds.
//!
//! The apex opportunity audit measured that no two introduced adults were ever simultaneously
//! able to reproduce, because every one of them died at 43–59 % of the
//! `reproduce_min_age_seconds` its own profile demands
//! (`design/7_Research/ecology-v1-budget-2026-09-16.md`, "The apex opportunity audit").
//! Separating "the age gate is what binds" from "something after the age gate also binds"
//! needs a founder that is already past it, and nothing else changed.
//!
//! These tests fix what that door may and may not do:
//!
//! - age zero is the current door, byte for byte, now and after the world runs on;
//! - a founder given an age at least `reproduce_min_age_seconds` passes `may_reproduce`'s age
//!   term on its first tick, and every other term it passes or fails is the one the age-zero
//!   founder passes or fails;
//! - the only value the age moves on the placed body is `born_tick`;
//! - an age the world cannot represent is refused before anything changes.

use cubarium_core::hunter::{self, FixedHunterProfile, HunterTarget};
use cubarium_core::organism::Organism;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{DT, OrganismId, World, WorldConfig};

/// The trial profile with a short maturity, so a test can step a world past it cheaply. Only
/// `reproduce_min_age_seconds` moves; the door under test reads no other profile term.
fn quick_maturity(world: &World, seconds: f64) -> FixedHunterProfile {
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
    profile.reproduce_min_age_seconds = seconds;
    profile
        .validate()
        .expect("the trial profile with a short maturity is valid");
    profile
}

fn target(face: u8, u: f64, v: f64) -> HunterTarget {
    HunterTarget { face, u, v }
}

fn pair() -> [HunterTarget; 2] {
    [target(1, 23.0, 31.0), target(4, 55.0, 3.0)]
}

fn stepped(ticks: u64) -> World {
    let mut world = World::new(WorldConfig::default()).expect("default world");
    for _ in 0..ticks {
        world.step();
    }
    world
}

/// Top a placed founder's stores up to the two stock fractions `may_reproduce` tests, so the
/// age term is the only one left that can decide it. This mutates a test world's organism
/// directly; it books nothing, because these tests read a predicate, not a budget.
fn fill_stores(world: &mut World, id: OrganismId, profile: &FixedHunterProfile) {
    let o = world
        .state
        .organisms
        .get_mut(id)
        .expect("the founder is placed");
    o.reserve = profile.reproduce_reserve_fraction * o.phenotype.reserve_max;
    o.energy = profile.reproduce_energy_fraction * o.phenotype.energy_max;
}

fn ready(world: &World, id: OrganismId, profile: &FixedHunterProfile) -> bool {
    let o = world
        .state
        .organisms
        .get(id)
        .expect("the founder is placed");
    let member = world.hunters().member(id).expect("the founder is a member");
    hunter::may_reproduce(profile, o, member, world.state.tick, DT)
}

/// Every term of `may_reproduce` except the age one, so a test can say which term decided.
fn non_age_terms_hold(world: &World, id: OrganismId, profile: &FixedHunterProfile) -> bool {
    let o = world
        .state
        .organisms
        .get(id)
        .expect("the founder is placed");
    let member = world.hunters().member(id).expect("the founder is a member");
    o.escrow.is_none()
        && !member.carrying()
        && member.target.is_none()
        && !member.phase.hunting()
        && o.structure >= o.phenotype.structure_adult - 1e-9
        && o.reserve >= profile.reproduce_reserve_fraction * o.phenotype.reserve_max
        && o.energy >= profile.reproduce_energy_fraction * o.phenotype.energy_max
        && world.state.tick >= member.next_reproduction_tick
}

fn body(world: &World, id: OrganismId) -> Organism {
    world
        .state
        .organisms
        .get(id)
        .expect("the founder is placed")
        .clone()
}

/// Age zero is the door that exists today: the same state hash at introduction, and the same
/// state hash after the world runs on, so nothing downstream reads a different value either.
#[test]
fn introduction_at_age_zero_is_byte_identical_to_the_current_door() {
    let mut old = stepped(120);
    let mut new = stepped(120);
    assert_eq!(state_hash(&old.state), state_hash(&new.state));

    let profile = quick_maturity(&old, 10.0);
    let old_receipts = old
        .introduce_hunters(profile.clone(), &pair())
        .expect("old door");
    let new_receipts = new
        .introduce_hunters_with_age(profile.clone(), &pair(), 0.0)
        .expect("new door at age zero");

    assert_eq!(state_hash(&old.state), state_hash(&new.state));
    assert_eq!(old_receipts.len(), new_receipts.len());
    for (a, b) in old_receipts.iter().zip(&new_receipts) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.tick, b.tick);
        assert_eq!(a.structure, b.structure);
        assert_eq!(a.reserve, b.reserve);
        assert_eq!(a.energy, b.energy);
        assert_eq!(a.material_in, b.material_in);
        assert_eq!(a.energy_in, b.energy_in);
    }

    for _ in 0..300 {
        old.step();
        new.step();
    }
    assert_eq!(state_hash(&old.state), state_hash(&new.state));
}

/// The one thing the age is for: a founder introduced at `reproduce_min_age_seconds` passes
/// the age term of `may_reproduce` on its first tick, where the age-zero founder does not, and
/// every other term of the predicate reads the same on both.
#[test]
fn an_aged_founder_passes_the_age_term_and_fails_nothing_else_a_young_one_passes() {
    let maturity_seconds = 10.0;
    let maturity_ticks = (maturity_seconds / DT).round() as u64;
    let mut young = stepped(maturity_ticks + 20);
    let mut aged = stepped(maturity_ticks + 20);
    let profile = quick_maturity(&young, maturity_seconds);

    let y = young
        .introduce_hunters(profile.clone(), &[target(1, 23.0, 31.0)])
        .expect("young")[0]
        .id;
    let a = aged
        .introduce_hunters_with_age(profile.clone(), &[target(1, 23.0, 31.0)], maturity_seconds)
        .expect("aged")[0]
        .id;
    fill_stores(&mut young, y, &profile);
    fill_stores(&mut aged, a, &profile);

    let now = young.state.tick;
    assert_eq!(now, aged.state.tick);
    assert_eq!(body(&young, y).age_ticks(now), 0);
    assert_eq!(body(&aged, a).age_ticks(now), maturity_ticks);

    // The age term itself, read directly.
    assert!((body(&young, y).age_ticks(now) as f64) * DT < profile.reproduce_min_age_seconds);
    assert!((body(&aged, a).age_ticks(now) as f64) * DT >= profile.reproduce_min_age_seconds);

    // Every other term reads the same on both, and holds.
    assert!(non_age_terms_hold(&young, y, &profile));
    assert!(non_age_terms_hold(&aged, a, &profile));

    // So the predicate is decided by the age alone.
    assert!(
        !ready(&young, y, &profile),
        "the age-zero founder must not be ready"
    );
    assert!(ready(&aged, a, &profile), "the aged founder must be ready");
}

/// The age moves `born_tick` and nothing else on the placed body, and books the same import.
#[test]
fn the_age_moves_the_birth_tick_and_no_other_value() {
    let mut young = stepped(600);
    let mut aged = stepped(600);
    let profile = quick_maturity(&young, 10.0);
    let age_seconds = 20.0;
    let age_ticks = (age_seconds / DT).round() as u64;

    let y = young
        .introduce_hunters(profile.clone(), &pair())
        .expect("young");
    let a = aged
        .introduce_hunters_with_age(profile.clone(), &pair(), age_seconds)
        .expect("aged");

    for (ry, ra) in y.iter().zip(&a) {
        let mut by = body(&young, ry.id);
        let ba = body(&aged, ra.id);
        assert_eq!(by.born_tick, young.state.tick);
        assert_eq!(ba.born_tick, aged.state.tick - age_ticks);
        // Rewrite only the field under test; everything else must already be equal.
        by.born_tick = ba.born_tick;
        assert_eq!(by, ba, "the age must move born_tick and nothing else");
        assert_eq!(
            young.hunters().member(ry.id).expect("member"),
            aged.hunters().member(ra.id).expect("member"),
        );
    }
    assert_eq!(
        young.hunters().founder_material_in,
        aged.hunters().founder_material_in
    );
    assert_eq!(
        young.hunters().founder_energy_in,
        aged.hunters().founder_energy_in
    );
    assert!(aged.mass_residual().abs() < 1e-9);
    aged.check_invariants()
        .expect("an aged founder keeps the world's own invariants");
}

/// An age the profile, the world's lifespan or the world's own clock cannot carry is refused
/// before any value changes, exactly like an unplaceable target.
#[test]
fn an_impossible_age_is_refused_without_placing_anything() {
    let mut world = stepped(200);
    let profile = quick_maturity(&world, 10.0);
    let before = state_hash(&world.state);
    let lifespan = world.config().organism.max_age_seconds;

    for (age, needle) in [
        (-1.0, "negative"),
        (f64::NAN, "negative"),
        (f64::INFINITY, "negative"),
        (lifespan + 1.0, "lifespan"),
        // 200 ticks (10 s) have passed, and 1,000 s is well inside the lifespan, so this asks
        // for 20,000 ticks of world that never happened.
        (1_000.0, "before this world began"),
    ] {
        let err = world
            .introduce_hunters_with_age(profile.clone(), &pair(), age)
            .expect_err("an impossible age must be refused");
        assert!(err.contains(needle), "age {age}: {err}");
        assert_eq!(state_hash(&world.state), before);
        assert_eq!(world.hunters().founders_placed, 0);
        assert!(world.hunters().profile.is_none());
    }
}
