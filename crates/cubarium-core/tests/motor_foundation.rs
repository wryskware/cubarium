//! Milestone R0a: body-scaled turn limits and paid physical motion, exercised through the
//! world rather than through `motor::resolve` alone.
//!
//! `crate::motor`'s own unit tests pin the envelope arithmetic — pure pivot, pure translation,
//! simultaneous scaling, degenerate inputs, the apex radius, the bill. What is checked here is
//! that **every** writer of heading and movement effort goes through it: the ordinary
//! controller, apex pursuit, escape and the encounter retreat alike, with no path left that
//! turns a body for free.

use cubarium_core::config::FounderKind;
use cubarium_core::hunter::ContactGeometry;
use cubarium_core::ids::OrganismId;
use cubarium_core::motor::{self, REFERENCE_RADIUS_PX};
use cubarium_core::organism::Mode;
use cubarium_core::{
    DT, FixedHunterProfile, HunterTarget, World, WorldConfig, decode_snapshot, encode_snapshot,
};
use cubarium_surface::{Face, SurfacePoint, Vec2};

/// A world with no hunters and no weather, so a body's only motion is its own.
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

/// Did this organism's published path leave the face it started the tick on? A heading that
/// crossed a seam is expressed in a different chart afterwards, so comparing it with the
/// pre-tick heading measures a change of coordinates, not a physical turn.
fn crossed_a_seam(world: &World, id: OrganismId) -> bool {
    let segments = world.moved_segments(id);
    segments
        .first()
        .is_some_and(|first| segments.iter().any(|s| s.face != first.face))
}

/// How far the published path actually went, in pixels.
fn travelled(world: &World, id: OrganismId) -> f64 {
    world
        .moved_segments(id)
        .iter()
        .map(cubarium_surface::PathSegment::length)
        .sum()
}

// ---------------------------------------------------------------- the envelope holds

/// Every body, every tick, in an ordinary world of mixed sizes: `|v| + r · |ω|` stays inside
/// the capability its own geometry allows. No boost exists here, so the bound checked is the
/// tight one — `speed_max / wading + REFERENCE_RADIUS_PX · turn_rate_max`, read from each
/// organism's own phenotype rather than from a constant.
#[test]
fn no_body_in_an_ordinary_world_ever_leaves_its_motor_envelope() {
    let mut config = calm();
    // Founders across the size range, so small, reference and wide bodies all run: a body at
    // exactly `REFERENCE_RADIUS_PX` is the one the envelope is calibrated on, and the wide ones
    // are the ones that have to trade.
    config.founders.kinds = vec![
        FounderKind { count: 12, ..kind(0.6) },
        FounderKind { count: 12, ..kind(1.0) },
        FounderKind { count: 12, ..kind(2.0) },
    ];
    let mut world = World::new(config).expect("valid");

    let mut widest_pivot_rate = 0.0f64;
    let mut widest_ceiling = 0.0f64;
    let mut traded = false;
    for _ in 0..600 {
        let before: Vec<(OrganismId, Vec2, f64, f64, f64)> = world
            .state
            .organisms
            .iter()
            .map(|(id, o)| {
                (
                    id,
                    o.heading,
                    o.phenotype.extent,
                    o.phenotype.speed_max,
                    f64::from(o.phenotype.drives.turn_rate_max_deg).to_radians(),
                )
            })
            .collect();
        world.step();
        world.drain_events();
        for (id, heading, extent, speed_max, turn_rate_max) in before {
            let Some(o) = world.state.organisms.get(id) else {
                continue; // died this tick
            };
            if crossed_a_seam(&world, id) {
                continue; // transport, measured separately below
            }
            let distance = travelled(&world, id);
            let turn = signed_turn(heading, o.heading);
            let swept = distance / DT + extent * turn.abs() / DT;
            // Wading only ever divides the translation ceiling, so this is the loosest the
            // capability can be, and the assertion is therefore the strictest.
            let capability = speed_max + REFERENCE_RADIUS_PX * turn_rate_max;
            assert!(
                swept <= capability * (1.0 + 1e-9),
                "{id:?}: swept {swept} past its capability {capability} \
                 (extent {extent}, moved {distance}, turned {turn})"
            );
            if extent > REFERENCE_RADIUS_PX && turn != 0.0 {
                widest_pivot_rate = widest_pivot_rate.max(turn.abs() / DT);
                widest_ceiling = widest_ceiling.max(turn_rate_max);
                // The trade the envelope is for: this body is turning *and* the envelope is
                // what stopped it going any faster, not its genome's angular ceiling.
                if swept > capability * 0.999 && turn.abs() / DT < turn_rate_max * 0.999 {
                    traded = true;
                }
            }
        }
    }
    assert!(widest_pivot_rate > 0.0, "no wide body ever turned");
    assert!(
        widest_pivot_rate < widest_ceiling,
        "a body wider than the reference radius reached its genome's full {widest_ceiling} rad/s          ({widest_pivot_rate}); the envelope never bound it"
    );
    assert!(traded, "no wide body ever gave up speed to turn");
}

/// A founder kind of one body size, everything else the world's own defaults.
fn kind(size: f32) -> FounderKind {
    FounderKind {
        name: format!("size-{size}"),
        count: 1,
        size: Some(size),
        ..FounderKind::default()
    }
}

// ---------------------------------------------------------------- the named regression

/// **Regression (R0a).** An apex member's pursuit override used to assign the heading directly,
/// late in the step and after the mode had already been relabelled. The body therefore faced
/// any bearing it liked within one tick — a lanternjaw reaching 14.8 px from root to claw tip
/// spun like a point — and paid nothing for it, because only translation was ever billed.
///
/// Now the override states an intent and the shared resolver answers it: the turn is at most
/// `u / r` with `r` the member's real grasp radius, and the energy for it is charged.
#[test]
fn an_apex_late_override_cannot_spin_a_body_for_free() {
    let mut world = World::new(calm()).expect("valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let spot = SurfacePoint::new(Face::Top, 32.0, 32.0);
    let hunter = world
        .start_hunter_trial(profile.clone(), HunterTarget { face: 4, u: 32.0, v: 32.0 })
        .expect("started")
        .id;

    // Face the member one way and put a hungry member's prey behind it, so the pursuit
    // override asks for the largest turn there is.
    {
        let o = world.state.organisms.get_mut(hunter).expect("alive");
        o.heading = Vec2::new(1.0, 0.0);
        let before = o.reserve;
        o.reserve = 0.1 * o.phenotype.reserve_max;
        world.state.external_material_in += o.reserve - before;
        assert_eq!(o.pos, spot, "the trial placed the member where this fixture expects");
    }

    let (radius, ceiling, capability) = {
        let o = world.state.organisms.get(hunter).expect("alive");
        let geometry = ContactGeometry::of(&profile, o);
        let radius = motor::turn_radius_px(o, Some(&geometry));
        let ceiling = f64::from(o.phenotype.drives.turn_rate_max_deg).to_radians();
        // Pursuit runs at full effort, which is the largest translation ceiling an unboosted
        // member has, and therefore the largest capability.
        (radius, ceiling, o.phenotype.speed_max + REFERENCE_RADIUS_PX * ceiling)
    };
    assert!(
        radius > 3.0 * REFERENCE_RADIUS_PX,
        "this fixture needs a body much wider than the reference one: {radius}"
    );
    let pivot_ceiling = capability / radius;
    assert!(
        pivot_ceiling < ceiling * 0.25,
        "a body this wide must be held well under its genome's {ceiling} rad/s, not {pivot_ceiling}"
    );

    let mut largest_turn_rate = 0.0f64;
    for _ in 0..400 {
        let heading = world.state.organisms.get(hunter).expect("alive").heading;
        world.step();
        world.drain_hunter_events();
        world.drain_events();
        let Some(o) = world.state.organisms.get(hunter) else {
            break;
        };
        if crossed_a_seam(&world, hunter) {
            continue;
        }
        let rate = signed_turn(heading, o.heading).abs() / DT;
        assert!(
            rate <= pivot_ceiling * (1.0 + 1e-9),
            "the member turned at {rate} rad/s, past the {pivot_ceiling} its body allows"
        );
        largest_turn_rate = largest_turn_rate.max(rate);
    }

    assert!(
        largest_turn_rate > 0.5 * pivot_ceiling,
        "the fixture never made the member turn hard: {largest_turn_rate}"
    );
    // What that rotation *costs* is billed in `a_turning_body_pays_for_the_distance_it_sweeps`,
    // on a fixture where no strike, meal or charging transaction shares the energy ledger.
}

/// The other half of the regression: a turn is not only bounded, it is **paid for**, at the
/// world's one `move_cost` and on the same `|v| + r · |omega|` the envelope bounds.
///
/// The fixture is quiet on purpose — no feeding, no oxidation, no hunters — so a tick's whole
/// energy delta is the motor bill and nothing else.
#[test]
fn a_turning_body_pays_for_the_distance_it_sweeps() {
    let mut config = calm();
    config.founders.kinds = vec![FounderKind { count: 8, ..kind(2.0) }];
    config.mechanisms.grazing = false;
    config.mechanisms.scavenging = false;
    // Oxidation would top the battery up in the same tick and hide the charge.
    config.organism.oxidation_rate = 0.0;
    let mut world = World::new(config).expect("valid");
    let cfg = world.config().organism.clone();

    let mut billed = 0u32;
    let mut turning = 0u32;
    for _ in 0..400 {
        let before: Vec<(OrganismId, Vec2, f64, f64, f64, f64)> = world
            .state
            .organisms
            .iter()
            .map(|(id, o)| {
                (id, o.heading, o.energy, o.structure, o.phenotype.extent, o.phenotype.maintenance)
            })
            .collect();
        world.step();
        world.drain_events();
        for (id, heading, energy, structure, extent, maintenance) in before {
            let Some(o) = world.state.organisms.get(id) else {
                continue;
            };
            // Gestation and exhaustion both move the same ledger inside one tick.
            if crossed_a_seam(&world, id) || o.escrow.is_some() || o.energy <= 0.0 {
                continue;
            }
            let rate = signed_turn(heading, o.heading).abs() / DT;
            let speed = travelled(&world, id) / DT;
            let bill = motor::MotorBill {
                structure,
                maintenance,
                sense_radius: o.phenotype.sense_radius,
                move_cost: cfg.move_cost,
                sense_cost: cfg.sense_cost,
            };
            let paid = energy - o.energy;
            assert!(
                (paid - bill.total_cost(speed, extent * rate, DT)).abs() < 1e-12,
                "{id:?} paid {paid}, not the bill for travelling {speed} px/s and sweeping {} px/s",
                extent * rate
            );
            billed += 1;
            if rate > 0.0 {
                turning += 1;
                assert!(
                    paid > bill.total_cost(speed, 0.0, DT),
                    "{id:?} turned at {rate} rad/s for free"
                );
            }
        }
    }
    assert!(billed > 1_000, "too few clean ticks to mean anything: {billed}");
    assert!(turning > 0, "no body turned at all");
}

// ---------------------------------------------------------------- transport is not a turn

/// A seam crossing re-expresses a heading in the next face's chart. That is coordinate
/// transport, and it must cost nothing: the energy a body spends on a tick that crosses a seam
/// is the energy that tick's *physical* motion costs, whatever the chart did to the numbers.
#[test]
fn a_seam_crossing_is_transport_and_never_a_paid_turn() {
    let mut config = calm();
    config.founders.kinds.clear();
    config.founders.count = 0;
    let mut world = World::new(config).expect("valid");

    // One body walking straight at a seam, resting so that it requests no turn at all: every
    // radian that appears in its heading across the boundary is the chart's doing.
    let id = world
        .state
        .organisms
        .insert(founder_at(&world, SurfacePoint::new(Face::Front, 63.99, 32.0), Vec2::new(1.0, 0.0)));
    {
        let o = world.state.organisms.get(id).expect("alive");
        world.state.external_material_in += o.structure + o.reserve;
    }
    let mut world = World::from_state(world.state).expect("valid");

    let mut crossings = 0;
    for _ in 0..400 {
        let (heading, energy, structure, sense_radius, maintenance) = {
            let o = world.state.organisms.get(id).expect("alive");
            (o.heading, o.energy, o.structure, o.phenotype.sense_radius, o.phenotype.maintenance)
        };
        world.step();
        world.drain_events();
        let Some(o) = world.state.organisms.get(id) else {
            break;
        };
        if !crossed_a_seam(&world, id) {
            continue;
        }
        crossings += 1;
        // The chart really did change the numbers.
        let apparent = signed_turn(heading, o.heading).abs();
        // And the charge is the one the physical motion earns, with nothing added for the
        // chart: the body was resting, so its sweep is its travel.
        let cfg = world.config().organism.clone();
        let distance = travelled(&world, id);
        let expected = (maintenance * structure
            + cfg.move_cost * structure * (distance / DT)
            + cfg.sense_cost * sense_radius)
            * DT;
        let paid = energy - o.energy;
        assert!(
            (paid - expected).abs() < 1e-12,
            "a seam tick paid {paid}, not the {expected} its {distance} px of travel costs \
             (the chart moved the heading by {apparent} rad)"
        );
    }
    assert!(crossings > 0, "the fixture never crossed a seam");
}

fn founder_at(world: &World, pos: SurfacePoint, heading: Vec2) -> cubarium_core::organism::Organism {
    use cubarium_core::genome::{Genome, decode};
    use cubarium_core::organism::{Organism, Origin};
    use cubarium_core::rng::Counter;

    let cfg = world.config();
    let genome = Genome::founder(0.5, &cfg.drives);
    let phenotype = decode(&genome, &cfg.organism);
    Organism {
        pos,
        heading,
        ou: Vec2::ZERO,
        structure: phenotype.structure_adult,
        // Full, so it rests rather than seeking: a resting body requests no turn at all.
        reserve: phenotype.reserve_max,
        energy: phenotype.energy_max,
        born_tick: 0,
        hunger_memory: 0.0,
        mode: Mode::Resting,
        escrow: None,
        births: 0,
        genome,
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    }
}

// ---------------------------------------------------------------- no energy, no motion

/// A body whose energy covers its upkeep and nothing more neither moves nor turns. Before R0a
/// the world moved it first and truncated the bill at whatever energy was left, so an exhausted
/// organism kept travelling for free.
#[test]
fn a_body_with_no_movement_energy_holds_still() {
    let mut config = calm();
    config.founders.kinds.clear();
    config.founders.count = 0;
    // Nothing to eat and nothing to burn: an intake or an oxidation would refill the battery
    // later in the same tick and hide what movement was allowed to do with it.
    config.mechanisms.grazing = false;
    config.mechanisms.scavenging = false;
    config.organism.oxidation_rate = 0.0;
    let mut world = World::new(config).expect("valid");
    let id = world
        .state
        .organisms
        .insert(founder_at(&world, SurfacePoint::new(Face::Top, 20.0, 20.0), Vec2::new(1.0, 0.0)));
    {
        let o = world.state.organisms.get_mut(id).expect("alive");
        // Hungry enough to want to go somewhere, with nothing to go on. The sliver of reserve
        // is what keeps it alive past this tick — starvation needs both stores empty — and it
        // cannot reach the battery in time, because oxidation runs after movement.
        o.reserve = 1e-6 * o.phenotype.reserve_max;
        o.hunger_memory = 1.0;
        o.mode = Mode::Seeking;
    }
    let (pos, heading, upkeep) = {
        let o = world.state.organisms.get_mut(id).expect("alive");
        let bill = motor::MotorBill {
            structure: o.structure,
            maintenance: o.phenotype.maintenance,
            sense_radius: o.phenotype.sense_radius,
            move_cost: 0.006,
            sense_cost: 0.0002,
        };
        let upkeep = bill.upkeep(DT);
        o.energy = upkeep;
        (o.pos, o.heading, upkeep)
    };
    assert!(upkeep > 0.0);
    {
        let o = world.state.organisms.get(id).expect("alive");
        world.state.external_material_in += o.structure + o.reserve;
    }
    let mut world = World::from_state(world.state).expect("valid");

    world.step();
    let o = world.state.organisms.get(id).expect("it starves later, not this tick");
    assert_eq!(o.pos, pos, "an organism with no movement energy travelled");
    assert_eq!(o.heading, heading, "an organism with no movement energy turned");
    assert!(world.moved_segments(id).is_empty(), "it published a path anyway");
    assert_eq!(o.energy, 0.0, "upkeep is still unavoidable");
}

// ---------------------------------------------------------------- overrides share the path

/// Escape bursts and encounter retreats are requests like any other. A threatened prey may turn
/// faster than its ordinary ceiling — that is the profile's escape rate — but never faster than
/// its own body allows, and the world charges it.
#[test]
fn an_escaping_prey_turns_within_its_body_and_pays_for_it() {
    let mut world = World::new(calm()).expect("valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let escape_ceiling = profile.escape_turn_rate_deg.to_radians();
    let hunter = world
        .start_hunter_trial(profile, HunterTarget { face: 4, u: 32.0, v: 32.0 })
        .expect("started")
        .id;
    {
        let o = world.state.organisms.get_mut(hunter).expect("alive");
        let before = o.reserve;
        o.reserve = 0.1 * o.phenotype.reserve_max;
        world.state.external_material_in += o.reserve - before;
    }

    let mut fastest: Vec<(OrganismId, f64, f64, f64)> = Vec::new();
    for _ in 0..600 {
        let before: Vec<(OrganismId, Vec2, f64, f64)> = world
            .state
            .organisms
            .iter()
            .filter(|(id, _)| *id != hunter)
            .map(|(id, o)| (id, o.heading, o.phenotype.extent, o.phenotype.speed_max))
            .collect();
        world.step();
        world.drain_hunter_events();
        world.drain_events();
        for (id, heading, extent, speed_max) in before {
            let Some(o) = world.state.organisms.get(id) else {
                continue;
            };
            if crossed_a_seam(&world, id) {
                continue;
            }
            let turn = signed_turn(heading, o.heading).abs();
            let swept = travelled(&world, id) / DT + extent * turn / DT;
            // The loosest envelope any override can open: the escape angular ceiling, and the
            // escape speed multiple on the translation ceiling.
            let capability =
                profile_escape_speed(speed_max) + REFERENCE_RADIUS_PX * escape_ceiling;
            assert!(
                swept <= capability * (1.0 + 1e-9),
                "{id:?}: a threatened body swept {swept} past {capability}"
            );
            fastest.push((id, turn / DT, extent, swept));
        }
    }
    let ordinary = WorldConfig::default().drives.turn_rate_max_deg.to_radians();
    let hurried = fastest.iter().any(|(_, rate, _, _)| *rate > ordinary * 1.01);
    assert!(
        hurried,
        "no prey ever used the escape rate, so the override was never exercised"
    );
}

fn profile_escape_speed(speed_max: f64) -> f64 {
    2.0 * speed_max
}

// ---------------------------------------------------------------- continuation

/// Same-build continuation across a tick that pivots: saving and reloading in the middle of a
/// run reproduces the rest of it exactly. The resolver is memoryless and reads only persisted
/// state, so nothing about it needs migrating — this is the check that says so.
#[test]
fn a_saved_world_resumes_identically_through_paid_pivots() {
    let mut config = calm();
    config.founders.kinds = vec![FounderKind { count: 24, ..kind(2.0) }];
    let mut world = World::new(config).expect("valid");
    for _ in 0..200 {
        world.step();
        world.drain_events();
    }

    let bytes = encode_snapshot(&world.state, "r0a-continuation");
    let (_, reloaded) = decode_snapshot(&bytes).expect("it decodes");
    assert_eq!(reloaded, world.state, "the round trip is not lossless");
    let mut resumed = World::from_state(reloaded).expect("valid");

    for tick in 0..300 {
        world.step();
        world.drain_events();
        resumed.step();
        resumed.drain_events();
        assert_eq!(
            resumed.state, world.state,
            "the resumed world diverged {tick} ticks after the save"
        );
    }
}
