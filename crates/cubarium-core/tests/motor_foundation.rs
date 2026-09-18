//! Milestone R0a: body-scaled turn limits and paid physical motion, exercised through the
//! world rather than through `motor::resolve` alone.
//!
//! `crate::motor`'s own unit tests pin the envelope arithmetic — pure pivot, pure translation,
//! simultaneous scaling, degenerate inputs, the apex radius, the bill. What is checked here is
//! that **every** writer of heading and movement effort goes through it: the ordinary
//! controller, apex pursuit, escape and the encounter retreat alike, with no path left that
//! turns a body for free.

use cubarium_core::config::FounderKind;
use cubarium_core::hunter::{ContactGeometry, HunterPhase};
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
/// the capability its own geometry allows.
///
/// **R0b.** The capability is now the translation ceiling alone. No boost exists in this
/// world and effort and wading only ever *divide* that ceiling, so the loosest bound any body
/// here can have is its own `speed_max` — and that is the assertion. Before R0b the same
/// assertion carried `+ REFERENCE_RADIUS_PX · turn_rate_max`, an addend worth 3.93 px/s
/// against a 0.3 px/s ceiling, which is why it never bound anything.
///
/// The second thing checked is that the shared budget, not the genome's angular ceiling, is
/// what limits turning: under the corrected envelope *every* body in this world is
/// budget-bound, because `speed_max / extent` is far below `turn_rate_max` at every size the
/// decoder produces.
#[test]
fn no_body_in_an_ordinary_world_ever_leaves_its_motor_envelope() {
    let mut config = calm();
    // Founders across the size range, so small, reference and wide bodies all run: a body at
    // exactly `REFERENCE_RADIUS_PX` is the one the envelope is calibrated on, and the wide ones
    // are the ones that have to trade.
    config.founders.kinds = vec![
        FounderKind {
            count: 12,
            ..kind(0.6)
        },
        FounderKind {
            count: 12,
            ..kind(1.0)
        },
        FounderKind {
            count: 12,
            ..kind(2.0)
        },
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
            // Effort and wading only ever divide the translation ceiling, so this is the
            // loosest the capability can be, and the assertion is therefore the strictest.
            let capability = speed_max;
            assert!(
                swept <= capability * (1.0 + 1e-9),
                "{id:?}: swept {swept} past its capability {capability} \
                 (extent {extent}, moved {distance}, turned {turn})"
            );
            if turn != 0.0 {
                // The trade the envelope is for: a turning body is spending budget it could
                // have spent on travel, and its rate is set by `u / r`, not by its genome.
                assert!(
                    turn.abs() / DT <= capability / extent * (1.0 + 1e-9),
                    "{id:?}: turned at {} rad/s, past the {} its {extent} px body allows",
                    turn.abs() / DT,
                    capability / extent
                );
                if distance > 0.0 {
                    traded = true;
                }
            }
            if extent > REFERENCE_RADIUS_PX && turn != 0.0 {
                widest_pivot_rate = widest_pivot_rate.max(turn.abs() / DT);
                widest_ceiling = widest_ceiling.max(turn_rate_max);
            }
        }
    }
    assert!(widest_pivot_rate > 0.0, "no wide body ever turned");
    assert!(
        widest_pivot_rate < widest_ceiling,
        "a body wider than the reference radius reached its genome's full {widest_ceiling} rad/s          ({widest_pivot_rate}); the envelope never bound it"
    );
    assert!(
        traded,
        "no body ever turned while travelling, so nothing was traded"
    );
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
        .start_hunter_trial(
            profile.clone(),
            HunterTarget {
                face: 4,
                u: 32.0,
                v: 32.0,
            },
        )
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
        assert_eq!(
            o.pos, spot,
            "the trial placed the member where this fixture expects"
        );
    }

    let (radius, ceiling, capability) = {
        let o = world.state.organisms.get(hunter).expect("alive");
        let geometry = ContactGeometry::of(&profile, o);
        let radius = motor::turn_radius_px(o, Some(&geometry));
        let ceiling = f64::from(o.phenotype.drives.turn_rate_max_deg).to_radians();
        // Pursuit runs at full effort, which is the largest translation ceiling an unboosted
        // member has — and since R0b that ceiling *is* the whole capability.
        (radius, ceiling, o.phenotype.speed_max)
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

    // **R0d.** A strike is a *paid* burst that lifts the whole shared budget, so during the
    // Strike phase the envelope is the profile's strike speed, not cruise — exactly as the
    // escaping-prey fixture bounds a threatened body by its escape multiple. Before the pace
    // calibration this never showed up here, because a 1 px/s strike was barely above the
    // 0.25 px/s the member cruised at; at 16.667 px/s it is the binding envelope.
    let strike_pivot = profile.strike_speed_px_s / radius;
    let mut largest_turn_rate = 0.0f64;
    let mut largest_budget = capability;
    let in_strike = |w: &World| {
        w.hunters()
            .members
            .iter()
            .any(|m| m.id == hunter && m.phase == HunterPhase::Strike)
    };
    for _ in 0..400 {
        let heading = world.state.organisms.get(hunter).expect("alive").heading;
        // **The phase at either end of the tick.** The budget a tick spends is the phase the
        // step resolved under, and the boundary tick — Windup before, Strike after — spends the
        // strike budget. Reading only the phase before the step was safe while the shipped
        // pursuit rule held this member at `rest_effort` through its whole burst; since the
        // reach envelope was adopted the burst is delivered, and the boundary tick is a real
        // strike tick that the pre-step reading called cruise
        // (`crates/cubarium-core/tests/pursuit_predicate_adoption.rs`).
        let was_striking = in_strike(&world);
        world.step();
        world.drain_hunter_events();
        world.drain_events();
        let striking = was_striking || in_strike(&world);
        let Some(o) = world.state.organisms.get(hunter) else {
            break;
        };
        if crossed_a_seam(&world, hunter) {
            continue;
        }
        let allowed = if striking {
            strike_pivot
        } else {
            pivot_ceiling
        };
        if striking {
            largest_budget = largest_budget.max(profile.strike_speed_px_s);
        }
        let rate = signed_turn(heading, o.heading).abs() / DT;
        assert!(
            rate <= allowed * (1.0 + 1e-9),
            "the member turned at {rate} rad/s, past the {allowed} its body allows"
        );
        largest_turn_rate = largest_turn_rate.max(rate);
    }

    // A floor on what the fixture exercised, stated against the smallest budget any tick of it
    // could have had: the rest-effort share of the cruise ceiling, which is what a member the
    // pursuit rule *holds* can reach. Since R0b effort throttles turning as well as travel.
    // Before R0b it was neither: the capability carried `REFERENCE_RADIUS_PX · turn_rate_max`
    // on top, and a *resting* member spun at 0.28 rad/s. That is the allowance this milestone
    // removes, and the per-tick assertion above is where it is actually enforced.
    let holding_ceiling = pivot_ceiling * WorldConfig::default().drives.rest_effort;
    assert!(
        largest_turn_rate > 0.5 * holding_ceiling,
        "the fixture never made the member turn as hard as its budget allowed: \
         {largest_turn_rate} against {holding_ceiling}"
    );
    // The R0b regression guard, re-anchored at R0d. At 0.06 BL/s the addend R0b removed
    // (`REFERENCE_RADIUS_PX · turn_rate_max` = 3.93 px/s) was thirteen times the whole cruise
    // budget, so "unreachable by an order of magnitude" was an available claim. At 1.0 BL/s
    // cruise is 4.2 px/s and the addend is still 3.93, so no such margin exists arithmetically
    // and the 10× form would assert nothing about this milestone. What is still asserted, every
    // tick above, is what R0b actually established: the rate never exceeds what the member's own
    // budget buys in the phase it is in. Here that budget is named again, with the addend added
    // back, to show the old envelope is strictly the larger one.
    let pre_r0b = (largest_budget + REFERENCE_RADIUS_PX * ceiling) / radius;
    assert!(
        largest_turn_rate < pre_r0b,
        "the member still turns like the pre-R0b envelope allowed ({largest_turn_rate} \
         against {pre_r0b} rad/s)"
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
    config.founders.kinds = vec![FounderKind {
        count: 8,
        ..kind(2.0)
    }];
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
                (
                    id,
                    o.heading,
                    o.energy,
                    o.structure,
                    o.phenotype.extent,
                    o.phenotype.maintenance,
                )
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
    assert!(
        billed > 1_000,
        "too few clean ticks to mean anything: {billed}"
    );
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
    let id = world.state.organisms.insert(founder_at(
        &world,
        SurfacePoint::new(Face::Front, 63.99, 32.0),
        Vec2::new(1.0, 0.0),
    ));
    {
        let o = world.state.organisms.get(id).expect("alive");
        world.state.external_material_in += o.structure + o.reserve;
    }
    let mut world = World::from_state(world.state).expect("valid");

    let mut crossings = 0;
    for _ in 0..400 {
        let (heading, energy, structure, sense_radius, maintenance) = {
            let o = world.state.organisms.get(id).expect("alive");
            (
                o.heading,
                o.energy,
                o.structure,
                o.phenotype.sense_radius,
                o.phenotype.maintenance,
            )
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

fn founder_at(
    world: &World,
    pos: SurfacePoint,
    heading: Vec2,
) -> cubarium_core::organism::Organism {
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
    let id = world.state.organisms.insert(founder_at(
        &world,
        SurfacePoint::new(Face::Top, 20.0, 20.0),
        Vec2::new(1.0, 0.0),
    ));
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
    let o = world
        .state
        .organisms
        .get(id)
        .expect("it starves later, not this tick");
    assert_eq!(o.pos, pos, "an organism with no movement energy travelled");
    assert_eq!(
        o.heading, heading,
        "an organism with no movement energy turned"
    );
    assert!(
        world.moved_segments(id).is_empty(),
        "it published a path anyway"
    );
    assert_eq!(o.energy, 0.0, "upkeep is still unavoidable");
}

// ---------------------------------------------------------------- overrides share the path

/// Escape bursts and encounter retreats are requests like any other: they raise ceilings and
/// the shared envelope still answers them.
///
/// **R0b.** Before the correction the escape override raised `turn_rate_max` to 240°/s *and*
/// the capability by `REFERENCE_RADIUS_PX · 240°/s` = 10.5 px/s, so a threatened prey really
/// did spin at 240°/s — two body lengths per second of rim sweep — and the test asserted that
/// it did. Under the shared budget a raised angular ceiling buys nothing: the prey's sweep is
/// bounded by its *speed* ceiling, which the escape multiple raises to `2 · speed_max`. What
/// is checked now is that the override is still real and still bounded — the escape speed
/// multiple lifts the budget above the ordinary one (so no Mode label suppresses a legitimate
/// escape), the envelope holds at the raised ceiling, and a prey turning hard gives up travel
/// to do it rather than getting the turn free.
#[test]
fn an_escaping_prey_turns_within_its_body_and_pays_for_it() {
    let mut world = World::new(calm()).expect("valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let escape_ceiling = profile.escape_turn_rate_deg.to_radians();
    let hunter = world
        .start_hunter_trial(
            profile,
            HunterTarget {
                face: 4,
                u: 32.0,
                v: 32.0,
            },
        )
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
        let before: Vec<(OrganismId, Vec2, f64, f64, Face)> = world
            .state
            .organisms
            .iter()
            .filter(|(id, _)| *id != hunter)
            .map(|(id, o)| {
                (
                    id,
                    o.heading,
                    o.phenotype.extent,
                    o.phenotype.speed_max,
                    o.pos.face,
                )
            })
            .collect();
        world.step();
        world.drain_hunter_events();
        world.drain_events();
        for (id, heading, extent, speed_max, face) in before {
            let Some(o) = world.state.organisms.get(id) else {
                continue;
            };
            // **R0d.** Seam transport is unpaid and is excluded from this envelope, which is
            // what `crossed_a_seam` is for — but that helper only sees a *published path* that
            // straddles two faces. At 1.0 BL/s a threatened body can land its whole tick on
            // the far side of a seam, publishing one segment on the new face and defeating the
            // check; the residual then reads as ~47 px/s of sweep on a 9.2 px/s envelope. A
            // change of face is the same event and is excluded the same way.
            if crossed_a_seam(&world, id)
                || o.pos.face != face
                || world.moved_segments(id).len() > 1
            {
                continue;
            }
            let turn = signed_turn(heading, o.heading).abs();
            let swept = travelled(&world, id) / DT + extent * turn / DT;
            // The loosest envelope any override can open: the escape speed multiple on the
            // translation ceiling, which since R0b is the whole capability. The escape
            // angular ceiling is a ceiling and adds nothing.
            let capability = profile_escape_speed(speed_max);
            assert!(
                swept <= capability * (1.0 + 1e-9),
                "{id:?}: a threatened body swept {swept} past {capability}"
            );
            fastest.push((id, turn / DT, extent, swept));
        }
    }
    assert!(!fastest.is_empty(), "the fixture observed no prey at all");
    // The escape override is still real: some prey spent more motor magnitude than an
    // unboosted body of its size ever could, which is the speed multiple arriving through the
    // shared budget. A Resting label does not take that away.
    let ordinary_cap = WorldConfig::default().organism.speed_max;
    let boosted = fastest
        .iter()
        .any(|(_, _, _, swept)| *swept > ordinary_cap * 1.01);
    assert!(
        boosted,
        "no prey ever exceeded the ordinary {ordinary_cap} px/s budget, so the escape \
         override never reached the envelope"
    );
    // And a threatened prey that turns is bounded by its own body, not by the 240°/s ceiling
    // the profile raises: `u / r` with `u` at most the boosted ceiling.
    let escape_ceiling_is_slack = fastest.iter().all(|(_, rate, extent, _)| {
        *rate <= profile_escape_speed(ordinary_cap) / extent * (1.0 + 1e-9)
    });
    assert!(
        escape_ceiling_is_slack,
        "a prey turned faster than its body's share of the escape budget"
    );
    let _ = escape_ceiling;
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
    config.founders.kinds = vec![FounderKind {
        count: 24,
        ..kind(2.0)
    }];
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
