//! The transient diagnostic intent seam (`cubarium_core::diagnostic`), added for R0b's
//! scripted-probe fixture (`examples/mobile_grazing.rs`).
//!
//! Three claims, because a seam that could quietly alter the world would make every
//! measurement taken through it worthless:
//!
//! 1. **Inert when empty.** A world with no overrides steps bit for bit as it did before the
//!    seam existed, and nothing about it is persisted.
//! 2. **An intent, not a result.** A scripted heading is answered by the same resolver as any
//!    other request, under `|v| + r · |ω| ≤ u` — it cannot spin a body for free.
//! 3. **It opens the behavioural gate and nothing else.** A scripted `graze_effort` feeds on a
//!    cell the controller's `feed_min` gate would refuse, but still takes only what the cell
//!    holds, through the world's own type-II term and reserve headroom.

use cubarium_core::diagnostic::ScriptedIntent;
use cubarium_core::genome::{Genome, decode};
use cubarium_core::ids::OrganismId;
use cubarium_core::motor;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::{DT, World, WorldConfig, encode_snapshot};
use cubarium_surface::{CellId, Face, SurfacePoint, Vec2, cell_of};

fn calm() -> WorldConfig {
    let mut c = WorldConfig::default();
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    c
}

/// The signed shortest angle from `a` to `b`, radians in `(−π, π]`.
fn signed_turn(a: Vec2, b: Vec2) -> f64 {
    use std::f64::consts::{PI, TAU};
    let d = b.screen_angle() - a.screen_angle();
    let d = (d + PI).rem_euclid(TAU) - PI;
    if d.is_finite() { d } else { 0.0 }
}

#[test]
fn an_empty_override_list_is_the_unchanged_trajectory() {
    let mut plain = World::new(calm()).expect("valid");
    let mut seamed = World::new(calm()).expect("valid");
    // Setting an empty list, and clearing one, are both no-ops.
    seamed.set_scripted_intents(Vec::new());
    seamed.clear_scripted_intents();
    for tick in 0..400 {
        plain.step();
        plain.drain_events();
        seamed.step();
        seamed.drain_events();
        assert_eq!(seamed.state, plain.state, "the seam moved the world at tick {tick}");
    }
    // And nothing about it reaches a snapshot.
    assert_eq!(
        encode_snapshot(&seamed.state, "seam"),
        encode_snapshot(&plain.state, "seam"),
        "the override list leaked into the persisted state"
    );
}

/// A probe standing alone on an emptied world, so nothing but the script drives it.
fn probe(config: WorldConfig, cell: CellId) -> (World, OrganismId) {
    let mut world = World::new(config).expect("valid");
    for c in CellId::all() {
        let i = c.index();
        let f = &mut world.state.fields;
        world.state.external_material_in -= f.p[i] + f.f[i] + f.d[i];
        f.p[i] = 0.0;
        f.f[i] = 0.0;
        f.d[i] = 0.0;
        f.de[i] = 0.0;
    }
    let cfg = world.config().clone();
    let genome = Genome::founder(0.5, &cfg.drives);
    let phenotype = decode(&genome, &cfg.organism);
    let pos = cell.center();
    let structure = phenotype.structure_adult;
    let reserve = 0.5 * phenotype.reserve_max;
    let id = world.state.organisms.insert(Organism {
        pos,
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure,
        reserve,
        energy: 0.75 * phenotype.energy_max,
        born_tick: 0,
        hunger_memory: 1.0,
        mode: Mode::Seeking,
        escrow: None,
        births: 0,
        genome,
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    });
    world.state.external_material_in += structure + reserve;
    let world = World::from_state(world.state).expect("the staged state is valid");
    world.check_invariants().expect("staged world is consistent");
    (world, id)
}

#[test]
fn a_scripted_heading_is_a_request_the_resolver_still_bounds() {
    let mut config = calm();
    config.founders.kinds.clear();
    config.founders.count = 0;
    let (mut world, id) = probe(config, CellId::new(Face::Top, 8, 8));
    let (extent, speed_max) = {
        let o = world.state.organisms.get(id).expect("placed");
        (o.phenotype.extent, o.phenotype.speed_max)
    };
    // The loosest possible per-tick turn: the whole unboosted budget spent on rotation.
    let ceiling = speed_max / extent * DT;
    assert!(
        ceiling < WorldConfig::default().drives.turn_rate_max_deg.to_radians() * DT,
        "this fixture only means something while the budget binds before the genome does"
    );

    // Ask, every tick, to face exactly backwards — the largest turn there is.
    let mut total = 0.0;
    for _ in 0..200 {
        let before = world.state.organisms.get(id).expect("alive").heading;
        world.set_scripted_intents(vec![(
            id,
            ScriptedIntent {
                heading: Some(before * -1.0),
                effort: Some(1.0),
                bud: Some(false),
                ..ScriptedIntent::default()
            },
        )]);
        world.step();
        world.drain_events();
        let after = world.state.organisms.get(id).expect("alive").heading;
        let turn = signed_turn(before, after).abs();
        assert!(
            turn <= ceiling * (1.0 + 1e-9),
            "a script turned the body {turn} rad in one tick, past the {ceiling} its budget buys"
        );
        total += turn;
    }
    assert!(total > 0.0, "the script never turned the body at all");
    // The body really is bound by the budget rather than merely refusing to turn. It sits a
    // few percent under the pure-pivot ceiling because the script also asks for full effort,
    // and the resolver's one common factor gives that request its proportional sliver of the
    // same budget — which is the shared budget doing exactly what it is for.
    let mean = total / 200.0;
    assert!(
        mean > ceiling * 0.9 && mean <= ceiling * (1.0 + 1e-9),
        "the script should sit just under the budget: mean {mean} against {ceiling}"
    );
}

#[test]
fn a_scripted_stillness_is_stillness_and_a_scripted_effort_is_paid() {
    let mut config = calm();
    config.founders.kinds.clear();
    config.founders.count = 0;
    config.organism.oxidation_rate = 0.0;
    let (mut world, id) = probe(config, CellId::new(Face::Top, 8, 8));
    let (pos, heading, bill) = {
        let o = world.state.organisms.get(id).expect("placed");
        (o.pos, o.heading, motor::MotorBill::of(o, world.config()))
    };
    let energy_before = world.state.organisms.get(id).expect("placed").energy;
    world.set_scripted_intents(vec![(id, ScriptedIntent::still(heading))]);
    world.step();
    world.drain_events();
    let o = world.state.organisms.get(id).expect("alive");
    assert_eq!(o.pos, pos, "a still script moved the body");
    assert_eq!(o.heading, heading, "a still script turned the body");
    assert!(
        ((energy_before - o.energy) - bill.upkeep(DT)).abs() < 1e-12,
        "a still body paid {} rather than its upkeep {}",
        energy_before - o.energy,
        bill.upkeep(DT)
    );
}

#[test]
fn a_scripted_graze_opens_the_gate_but_not_the_cell() {
    let mut config = calm();
    config.founders.kinds.clear();
    config.founders.count = 0;
    let feed_min = config.drives.feed_min;
    let cell = CellId::new(Face::Top, 8, 8);

    // A cell holding far less than the controller's gate would open on.
    let stock = feed_min * 0.25;
    let mut eaten = Vec::new();
    for scripted in [false, true] {
        let (mut world, id) = probe(config.clone(), cell);
        world.state.fields.p[cell.index()] = stock;
        world.state.external_material_in -= stock;
        let mut world = World::from_state(world.state).expect("valid");
        for _ in 0..200 {
            let heading = world.state.organisms.get(id).expect("alive").heading;
            let mut intent = ScriptedIntent::still(heading);
            if scripted {
                intent.graze_effort = Some(1.0);
            }
            world.set_scripted_intents(vec![(id, intent)]);
            world.step();
            world.drain_events();
            assert_eq!(
                cell_of(&world.state.organisms.get(id).expect("alive").pos),
                cell,
                "the probe left its cell"
            );
        }
        world.check_invariants().expect("consistent");
        eaten.push((world.intake_diagnostics().producer_eaten, world.state.fields.p[cell.index()]));
    }
    let (gated, _) = eaten[0];
    let (open, left) = eaten[1];
    assert_eq!(gated, 0.0, "the legacy gate must refuse a cell below feed_min");
    assert!(open > 0.0, "the scripted request must reach a cell the gate refuses");
    // But the world's own law still binds: nothing was taken that the cell did not hold, and
    // the type-II term still throttles a poor cell rather than handing over the lot.
    assert!(left >= 0.0, "the cell went negative: {left}");
    assert!(
        open <= stock + 1e-12,
        "the script took {open} from a cell holding {stock}"
    );
}

#[test]
fn a_scripted_intent_cannot_place_a_body_off_the_surface() {
    let mut config = calm();
    config.founders.kinds.clear();
    config.founders.count = 0;
    let (mut world, id) = probe(config, CellId::new(Face::Top, 1, 1));
    for _ in 0..600 {
        world.set_scripted_intents(vec![(
            id,
            ScriptedIntent {
                // A degenerate request: not a unit vector, not finite in one component.
                heading: Some(Vec2::new(f64::NAN, 7.0)),
                effort: Some(5.0),
                graze_effort: Some(9.0),
                bud: Some(false),
                ..ScriptedIntent::default()
            },
        )]);
        world.step();
        world.drain_events();
    }
    world.check_invariants().expect("a degenerate script broke the world");
    let o = world.state.organisms.get(id).expect("alive");
    assert!(o.pos.is_canonical());
    assert!((o.heading.length() - 1.0).abs() < 1e-6);
    let _ = SurfacePoint::new(Face::Top, 0.0, 0.0);
}
