//! The starvation predicate: a body dies when it cannot pay for being alive.
//!
//! Before this change the world asked `energy <= 0 && reserve <= 0` **after** the feeding
//! settlement. Reserve decays geometrically and oxidation never empties it exactly, so a broke
//! body standing on a nearly bare cell stayed alive indefinitely on an infinitesimal bite —
//! R0b recorded live bodies at `energy = 5.5e-57`. The rule is now evaluated *before* intake
//! settles, and asks the only question that has a physical answer: can the energy this body
//! can raise *this* tick — what it holds plus everything one tick of oxidation can convert out
//! of its reserve — cover this tick's mandatory upkeep (`MotorBill::upkeep`)?

use cubarium_surface::{Scale, Topology};
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
    cell_of(Topology::Cube, Scale::ONE, &world.state.organisms.get(id).expect("the founder").pos).index()
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


// --------------------------------------------------------------- solvency is also settlement

/// What a body actually paid this tick, reconstructed from the transaction rather than from
/// the closing energy: stored energy spent, plus the usable energy the reserve it burned was
/// worth. Every case below asserts *this*, not merely that the body is still breathing.
fn paid(before: &cubarium_core::organism::Organism, after: &cubarium_core::organism::Organism, cfg: &WorldConfig) -> f64 {
    let oxidised = (before.reserve - after.reserve).max(0.0)
        * cfg.organism.reserve_energy_density
        * cfg.organism.oxidation_efficiency;
    before.energy + oxidised - after.energy
}

/// A solo body that cannot feed: no mouth, no food underfoot, nothing to move toward.
fn sealed(world: &mut World, id: cubarium_core::OrganismId) -> usize {
    let here = park(world, id);
    for cell in cubarium_surface::CellId::all(Topology::Cube, Scale::ONE) {
        world.state.fields.p[cell.index()] = 0.0;
        world.state.fields.f[cell.index()] = 0.0;
        world.state.fields.d[cell.index()] = 0.0;
        world.state.fields.de[cell.index()] = 0.0;
    }
    here
}

/// **Zero stored energy, adequate reserve.** Solvency admits the body on energy the reserve
/// can supply, so the settlement must actually take it. Before this repair the bill was
/// forgiven and the whole oxidation credit was handed over anyway: the body ended the tick
/// with `8.0e-4 e` having paid `7.6e-19`.
#[test]
fn a_body_with_no_stored_energy_pays_its_upkeep_out_of_the_reserve() {
    let mut world = World::new(solo(1.0)).expect("valid");
    let id = only_id(&world);
    sealed(&mut world, id);
    let cfg = world.config().clone();
    {
        let o = world.state.organisms.get_mut(id).expect("the founder");
        o.energy = 0.0;
        o.reserve = 10.0 * cfg.organism.oxidation_rate * DT;
    }
    let before = world.state.organisms.get(id).expect("alive").clone();
    let upkeep = MotorBill::of(&before, &cfg).upkeep(DT);
    assert!(before.raisable_energy(&cfg.organism, DT) >= upkeep, "the fixture is solvent");

    world.step();

    let after = world.state.organisms.get(id).expect("it survived").clone();
    let settled = paid(&before, &after, &cfg);
    assert!(
        (settled - upkeep).abs() < 1e-15,
        "owed {upkeep:e}, paid {settled:e}, reserve {:e} -> {:e}, energy {:e} -> {:e}",
        before.reserve,
        after.reserve,
        before.energy,
        after.energy
    );
    assert!(after.reserve < before.reserve, "the reserve is what paid");
    world.check_invariants().unwrap();
}

/// **Partial stored energy.** The battery goes first and the reserve gives up exactly the
/// shortfall — no more.
///
/// `oxidation_threshold` is zeroed so the physiology pass's own top-up never runs: the only
/// oxidation in the tick is the settlement's, and the reserve delta therefore measures it
/// directly instead of being confounded by a legitimate later recharge.
#[test]
fn a_body_with_partial_stored_energy_oxidises_only_the_shortfall() {
    let mut config = solo(1.0);
    config.organism.oxidation_threshold = 0.0;
    let mut world = World::new(config).expect("valid");
    let id = only_id(&world);
    sealed(&mut world, id);
    let cfg = world.config().clone();
    let upkeep = {
        let o = world.state.organisms.get(id).expect("the founder");
        MotorBill::of(o, &cfg).upkeep(DT)
    };
    {
        let o = world.state.organisms.get_mut(id).expect("the founder");
        o.energy = 0.25 * upkeep;
        o.reserve = 10.0 * cfg.organism.oxidation_rate * DT;
    }
    let before = world.state.organisms.get(id).expect("alive").clone();

    world.step();

    let after = world.state.organisms.get(id).expect("it survived").clone();
    let settled = paid(&before, &after, &cfg);
    assert!((settled - upkeep).abs() < 1e-15, "owed {upkeep:e}, paid {settled:e}");
    // Three quarters of the upkeep was missing; that, and nothing more, came out of the
    // reserve, leaving most of the per-tick oxidation allowance unspent.
    let burned = before.reserve - after.reserve;
    let want =
        0.75 * upkeep / (cfg.organism.reserve_energy_density * cfg.organism.oxidation_efficiency);
    assert!(
        (burned - want).abs() < 1e-15,
        "burned {burned:e} of reserve, expected the shortfall's {want:e}"
    );
    assert!(
        burned < cfg.organism.oxidation_rate * DT,
        "and it did not reach for the whole allowance"
    );
    assert!(after.energy.abs() < 1e-15, "the battery was emptied into the bill first");
    world.check_invariants().unwrap();
}

/// **Inadequate reserve.** The body pays everything it can raise and dies the same tick; the
/// shortfall is not silently forgiven and no energy is created.
#[test]
fn a_body_whose_reserve_cannot_cover_the_shortfall_pays_what_it_has_and_dies() {
    let mut world = World::new(solo(1.0)).expect("valid");
    let id = only_id(&world);
    sealed(&mut world, id);
    let cfg = world.config().clone();
    let upkeep = {
        let o = world.state.organisms.get(id).expect("the founder");
        MotorBill::of(o, &cfg).upkeep(DT)
    };
    {
        let o = world.state.organisms.get_mut(id).expect("the founder");
        o.energy = 0.1 * upkeep;
        // A hair of reserve: real, oxidisable, and nowhere near enough.
        o.reserve = 0.01 * upkeep / (cfg.organism.reserve_energy_density * cfg.organism.oxidation_efficiency);
    }
    let before = world.state.organisms.get(id).expect("alive").clone();
    assert!(before.raisable_energy(&cfg.organism, DT) < upkeep, "the fixture is insolvent");

    world.step();

    let died = world
        .drain_events()
        .into_iter()
        .any(|e| matches!(e, LifeEvent::Death { id: d, cause, .. } if d == id && cause == DeathCause::Starvation));
    assert!(died, "an insolvent body starves");
    assert!(world.state.organisms.get(id).is_none());
    world.check_invariants().unwrap();
}

/// **Intake arriving later in the tick does not retroactively fund the bill.** The settlement
/// runs in the motor stage, before the feeding pass, so a body pays out of the resources it
/// held *then*.
///
/// The measurement is a comparison: the identical body on a rich cell and on a bare one burns
/// exactly the same reserve for its upkeep. If the later bite were being allowed to settle any
/// part of the bill, the fed arm would have burned less. `oxidation_threshold` is zeroed in
/// both arms so the only oxidation is the settlement's.
#[test]
fn intake_later_in_the_tick_does_not_pay_this_ticks_bill() {
    fn arm(fed: bool) -> (f64, f64, f64) {
        let mut config = solo(1.0);
        config.organism.oxidation_threshold = 0.0;
        // Producer dynamics off, so the cell's stock changes by the bite and by nothing else
        // and the reconstruction below is exact rather than approximate.
        config.producer.growth = 0.0;
        config.producer.mortality = 0.0;
        config.detritus.decomposition = 0.0;
        // Growth also draws on the reserve, and it fires only in the arm whose bite refilled
        // it. Switch it off so the two arms differ in exactly one thing: whether a bite landed.
        config.organism.growth_rate = 0.0;
        let mut world = World::new(config).expect("valid");
        let id = only_id(&world);
        let here = sealed(&mut world, id);
        if fed {
            world.state.fields.p[here] = world.state.config.producer.max;
        }
        let cfg = world.config().clone();
        {
            let o = world.state.organisms.get_mut(id).expect("the founder");
            o.energy = 0.0;
            o.reserve = 10.0 * cfg.organism.oxidation_rate * DT;
            // Hungry, and on a rich cell in the fed arm, so the legacy mouth actually opens.
            o.hunger_memory = 1.0;
            o.mode = cubarium_core::organism::Mode::Feeding;
        }
        let before = world.state.organisms.get(id).expect("alive").clone();
        let upkeep = MotorBill::of(&before, &cfg).upkeep(DT);
        let _ = here;
        world.step();
        let after = world.state.organisms.get(id).expect("it survived").clone();
        world.check_invariants().unwrap();
        // Reserve *gained* from the bite and *lost* to the settlement in the same tick, so the
        // burn is recovered by removing the material the mouth put in. The bite comes from the
        // world's own intake diagnostic, not from a cell delta: the settlement happens after
        // movement, so the cell that was grazed is not necessarily the one the body started on.
        let eaten = world.intake_diagnostics().producer_eaten;
        // Ecology v1 §6.4: a served bite `q` credits the reserve with `η_m · cap_h · q`, not
        // `η_m · q` — the indigestible share never enters the body at all.
        let assimilated =
            cfg.organism.assimilation_material * before.phenotype.cap_foliage * eaten;
        let burned = before.reserve + assimilated - after.reserve;
        (burned, upkeep, eaten)
    }

    let (bare_burn, upkeep, bare_eaten) = arm(false);
    let (fed_burn, _, fed_eaten) = arm(true);
    assert_eq!(bare_eaten, 0.0, "the control arm must not feed");
    assert!(fed_eaten > 0.0, "the fed arm must actually feed, or this tests nothing");
    let want = upkeep / (cfg_density() * cfg_efficiency());
    assert!(
        (bare_burn - want).abs() < 1e-12 && (fed_burn - want).abs() < 1e-12,
        "upkeep {upkeep:e} should cost {want:e} of reserve in both arms; bare {bare_burn:e}, \
         fed {fed_burn:e}"
    );
}

fn cfg_density() -> f64 {
    WorldConfig::default().organism.reserve_energy_density
}

fn cfg_efficiency() -> f64 {
    WorldConfig::default().organism.oxidation_efficiency
}

/// The per-tick oxidation limit bounds the **tick**, not each pass: a body that oxidised in
/// the settlement cannot oxidise a second full allowance in the physiology pass.
#[test]
fn the_settlement_and_the_physiology_pass_share_one_oxidation_allowance() {
    let mut world = World::new(solo(1.0)).expect("valid");
    let id = only_id(&world);
    sealed(&mut world, id);
    let cfg = world.config().clone();
    {
        let o = world.state.organisms.get_mut(id).expect("the founder");
        o.energy = 0.0;
        o.reserve = o.phenotype.reserve_max;
    }
    let before = world.state.organisms.get(id).expect("alive").clone();

    world.step();

    let after = world.state.organisms.get(id).expect("it survived").clone();
    let burned = before.reserve - after.reserve;
    let allowance = cfg.organism.oxidation_rate * DT;
    assert!(
        burned <= allowance + 1e-15,
        "burned {burned:e} of reserve against a per-tick allowance of {allowance:e}"
    );
    world.check_invariants().unwrap();
}
