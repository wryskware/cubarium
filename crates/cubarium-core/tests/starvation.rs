//! The starvation predicate: a body dies when it cannot pay for being alive.
//!
//! Before this change the world asked `energy <= 0 && reserve <= 0` **after** the feeding
//! settlement. Reserve decays geometrically and oxidation never empties it exactly, so a broke
//! body standing on a nearly bare cell stayed alive indefinitely on an infinitesimal bite —
//! R0b recorded live bodies at `energy = 5.5e-57`. The rule is now evaluated *before* intake
//! settles, and asks the only question that has a physical answer: can the energy this body
//! can raise *this* tick — what it holds plus everything one tick of oxidation can convert out
//! of its reserve — cover this tick's mandatory upkeep (`MotorBill::upkeep`)?

use cubarium_core::config::FounderKind;
use cubarium_core::motor::MotorBill;
use cubarium_core::organism::DeathCause;
use cubarium_core::{DT, LifeEvent, World, WorldConfig};
use cubarium_surface::{Face, SurfacePoint, cell_of};

/// One founder, no weather, no rain, no hunters: the only thing that happens is physiology.
fn solo(size: f32) -> WorldConfig {
    let mut c = WorldConfig::default();
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    c.founders.kinds = vec![FounderKind {
        name: format!("size-{size}"),
        count: 1,
        size: Some(size),
        ..FounderKind::default()
    }];
    c.founders.count = 1;
    c
}

fn only_id(world: &World) -> cubarium_core::OrganismId {
    let ids: Vec<_> = world.state.organisms.iter().map(|(id, _)| id).collect();
    assert_eq!(ids.len(), 1, "the fixture places exactly one body");
    ids[0]
}

/// Park the body on the middle of the Top face, in the cell whose stocks the test controls.
fn park(world: &mut World, id: cubarium_core::OrganismId) -> usize {
    let o = world.state.organisms.get_mut(id).expect("the founder");
    o.pos = SurfacePoint {
        face: Face::Top,
        u: 26.0,
        v: 26.0,
    };
    cell_of(&world.state.organisms.get(id).expect("the founder").pos).index()
}

/// A body whose cell still carries food, but not enough to matter, dies on the tick it first
/// cannot pay — it does not linger for thousands of ticks taking infinitesimal bites.
///
/// The "epsilon stock" here is exactly the R0b failure mode: `P` is positive, so the old
/// predicate's `reserve <= 0` half was still false after the settlement topped the reserve up
/// by ~1e-9 m, and the body never died.
#[test]
fn a_body_that_cannot_pay_upkeep_dies_within_one_tick_even_with_food_underfoot() {
    let mut world = World::new(solo(1.0)).expect("valid");
    let id = only_id(&world);
    let here = park(&mut world, id);
    // An epsilon standing crop: edible, and worth nothing.
    world.state.fields.p[here] = 1e-9;
    world.state.fields.f[here] = 0.0;
    world.state.fields.d[here] = 0.0;
    world.state.fields.de[here] = 0.0;
    // Growth would refill the cell from nothing; keep the stock exactly as placed.
    let cfg = world.config().clone();
    let (energy, reserve, upkeep) = {
        let o = world.state.organisms.get_mut(id).expect("the founder");
        // Strip the stores to a hair under one tick of upkeep, oxidation included.
        let bill = MotorBill::of(o, &cfg);
        let upkeep = bill.upkeep(DT);
        o.reserve = 0.0;
        o.energy = 0.5 * upkeep;
        (o.energy, o.reserve, upkeep)
    };
    assert!(upkeep > 0.0, "the default world charges upkeep");
    assert!(
        energy > 0.0 && reserve <= 0.0,
        "the old predicate would not have fired: energy {energy:e}, reserve {reserve:e}"
    );

    world.step();

    let deaths: Vec<_> = world
        .drain_events()
        .into_iter()
        .filter_map(|e| match e {
            LifeEvent::Death { id, cause, .. } => Some((id, cause)),
            _ => None,
        })
        .collect();
    assert_eq!(
        deaths,
        vec![(id, DeathCause::Starvation)],
        "the body could raise {energy:e} e against an upkeep of {upkeep:e} e and should have \
         starved on this tick"
    );
    assert!(world.state.organisms.get(id).is_none());
    world.check_invariants().unwrap();
}

/// The converse, and the reason the predicate is not simply `energy < upkeep`: a body with
/// **no** energy at all survives as long as one tick of oxidation can convert enough reserve
/// to cover the upkeep. Charging is physiology, not a rescue.
#[test]
fn a_body_with_no_energy_but_oxidisable_reserve_survives() {
    let mut world = World::new(solo(1.0)).expect("valid");
    let id = only_id(&world);
    let here = park(&mut world, id);
    world.state.fields.p[here] = 0.0;
    world.state.fields.f[here] = 0.0;
    world.state.fields.d[here] = 0.0;
    world.state.fields.de[here] = 0.0;

    let cfg = world.config().clone();
    let (raisable, upkeep) = {
        let o = world.state.organisms.get_mut(id).expect("the founder");
        o.energy = 0.0;
        // One tick of oxidation at the configured rate, worth `e_r · η_ox` per unit burned.
        o.reserve = 10.0 * cfg.organism.oxidation_rate * DT;
        let bill = MotorBill::of(o, &cfg);
        (o.raisable_energy(&cfg.organism, DT), bill.upkeep(DT))
    };
    assert!(
        raisable > upkeep,
        "the fixture must leave the body solvent: raisable {raisable:e} against upkeep {upkeep:e}"
    );

    world.step();

    let died = world
        .drain_events()
        .into_iter()
        .any(|e| matches!(e, LifeEvent::Death { id: d, .. } if d == id));
    assert!(
        !died,
        "a body with zero energy and oxidisable reserve pays its upkeep out of the reserve"
    );
    let o = world.state.organisms.get(id).expect("it is still alive");
    assert!(
        o.energy >= 0.0 && o.energy.is_finite(),
        "energy after the oxidation transaction: {:e}",
        o.energy
    );
    world.check_invariants().unwrap();
}

