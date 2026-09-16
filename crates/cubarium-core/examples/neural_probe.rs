//! R1a development probe: a fixed hand-authored policy through the **real** world step, plus
//! the R1 throughput screen.
//!
//! Two parts, both deterministic and both on their own isolated fixture:
//!
//! 1. **Action tape.** One neural body on a fed patch, driven by a bias-only head that is
//!    rewritten between segments, so the probe walks the contract's §4 rows — rest, travel,
//!    pure pivot, a combined split, and continuous grazing — and prints the resolved motion,
//!    the bills and the intake the world actually produced.
//! 2. **Throughput.** 32 and 128 bodies for 2,000 ticks and 512 for 200, each arm run twice
//!    (all-legacy and all-neural) so the sampler-and-inference cost is the difference.
//!
//! Nothing here attaches a policy to the display world, and nothing here trains.

use cubarium_surface::{Scale, Topology};
use std::time::Instant;

use cubarium_core::config::FounderKind;
use cubarium_core::ids::OrganismId;
use cubarium_core::motor::MotorBill;
use cubarium_core::neural::gru::{Gru32, OUTPUT};
use cubarium_core::neural::Policy;
use cubarium_core::{DT, NeuralTiming, World, WorldConfig};
use cubarium_surface::{Face, SurfacePoint};

/// A bias-only head: the action is `squash(b_o)` on every controller tick, whatever the world
/// looks like. That is what makes the tape a *tape* — the world's response is the only thing
/// that varies.
fn tape_policy(head: [f64; OUTPUT]) -> Gru32 {
    let mut w = Gru32::zeros();
    w.b_o.copy_from_slice(&head);
    w
}

fn probe_world() -> (World, OrganismId) {
    let mut c = WorldConfig::default();
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    c.producer.growth = 0.0;
    c.producer.mortality = 0.0;
    c.founders.kinds = vec![FounderKind {
        name: "probe".into(),
        count: 1,
        size: Some(1.0),
        ..FounderKind::default()
    }];
    c.founders.count = 1;
    let mut world = World::new(c).expect("valid probe world");
    let id = world
        .state
        .organisms
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("one founder");
    {
        let o = world.state.organisms.get_mut(id).expect("the founder");
        o.pos = SurfacePoint {
            face: Face::Top,
            u: 26.0,
            v: 26.0,
        };
        o.heading = cubarium_surface::Vec2::new(1.0, 0.0);
        o.structure = o.phenotype.structure_adult;
        let before = o.reserve;
        // Deliberately *not* full: a reserve at its ceiling has no headroom, and the intake
        // law would refuse every bite for a reason that has nothing to do with the policy.
        o.reserve = 0.2 * o.phenotype.reserve_max;
        o.energy = o.phenotype.energy_max;
        world.state.external_material_in += world
            .state
            .organisms
            .get(id)
            .expect("the founder")
            .reserve
            - before;
    }
    // A well-fed patch so grazing has something to bite.
    for cell in cubarium_surface::CellId::all(Topology::Cube, Scale::ONE) {
        world.state.fields.p[cell.index()] = world.state.config.producer.max;
    }
    let world = World::from_state(world.state).expect("still valid");
    (world, id)
}

struct Segment {
    label: &'static str,
    head: [f64; OUTPUT],
    ticks: u64,
}

fn action_tape() {
    println!("# R1a probe: a fixed action tape through the real world step\n");
    println!(
        "One unit adult, {} px/s cruise, radius {:.2} px, on a patch held at `P_max`. Each row \
         is what the **world** did over the segment, not what was asked for.\n",
        WorldConfig::default().organism.speed_max,
        2.5
    );

    let segments = [
        Segment {
            label: "rest (all channels at the floor)",
            head: [-8.0, 0.0, -8.0, -8.0, -8.0, -8.0, -8.0],
            ticks: 200,
        },
        Segment {
            label: "travel (thrust 1, no turn)",
            head: [8.0, 0.0, -8.0, -8.0, -8.0, -8.0, -8.0],
            ticks: 200,
        },
        Segment {
            label: "pure pivot (thrust 0, turn +1)",
            head: [-8.0, 8.0, -8.0, -8.0, -8.0, -8.0, -8.0],
            ticks: 200,
        },
        Segment {
            label: "split (thrust ~0.5, turn ~+0.5)",
            head: [0.0, 0.55, -8.0, -8.0, -8.0, -8.0, -8.0],
            ticks: 200,
        },
        Segment {
            label: "graze in place (thrust 0, graze 1)",
            head: [-8.0, 0.0, 8.0, -8.0, -8.0, -8.0, -8.0],
            ticks: 200,
        },
        Segment {
            label: "travel and graze together",
            head: [8.0, 0.0, 8.0, -8.0, -8.0, -8.0, -8.0],
            ticks: 200,
        },
    ];

    println!(
        "| segment | px travelled | net turn (deg) | paid upkeep (e) | paid motion (e) | \
         eaten P/F/D (m) | Δreserve (m) | Δenergy (e) |"
    );
    println!("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    let mut rows = Vec::new();
    for segment in segments {
        let (mut world, id) = probe_world();
        world
            .attach_neural_policy(id, Policy::new(tape_policy(segment.head)))
            .expect("attach");
        let start = world.state.organisms.get(id).expect("alive").clone();
        let bill = MotorBill::of(&start, world.config());
        let radius = start.phenotype.extent;
        let intake_before = world.intake_diagnostics();

        let mut travelled = 0.0f64;
        let mut turned = 0.0f64;
        // The bill the world actually charged, rebuilt from the motion it actually resolved:
        // upkeep is mandatory and now genuinely collected every tick, and the motion half is
        // priced from this tick's published path and heading change rather than inferred from
        // an energy delta that also contains assimilation and oxidation credits.
        let mut paid_upkeep = 0.0f64;
        let mut paid_motion = 0.0f64;
        let mut heading = start.heading;
        for _ in 0..segment.ticks {
            world.step();
            world.drain_events();
            let Some(o) = world.state.organisms.get(id) else {
                break;
            };
            paid_upkeep += bill.upkeep(DT);
            let path: f64 = world
                .moved_segments(id)
                .iter()
                .map(cubarium_surface::PathSegment::length)
                .sum();
            travelled += path;
            // A seam crossing changes the chart, so the heading difference across one is a
            // change of coordinates and not a turn. Those ticks contribute travel but no sweep.
            let one_face = world
                .moved_segments(id)
                .first()
                .is_none_or(|f| world.moved_segments(id).iter().all(|s| s.face == f.face));
            let turn = if one_face {
                let d = (o.heading.screen_angle() - heading.screen_angle())
                    .rem_euclid(std::f64::consts::TAU);
                let d = if d > std::f64::consts::PI {
                    d - std::f64::consts::TAU
                } else {
                    d
                };
                // Clockwise in the body frame is the negative screen angle.
                -d
            } else {
                0.0
            };
            turned += turn;
            paid_motion += bill.motor_cost(path / DT, radius * turn.abs() / DT, DT);
            heading = o.heading;
        }

        let end = world.state.organisms.get(id).expect("alive");
        let intake = world.intake_diagnostics();
        let eaten = (
            intake.producer_eaten - intake_before.producer_eaten,
            intake.fruit_eaten - intake_before.fruit_eaten,
            (intake.litter_eaten + intake.carrion_eaten)
                - (intake_before.litter_eaten + intake_before.carrion_eaten),
        );
        println!(
            "| {} | {:.2} | {:.1} | {:.5} | {:.5} | {:.4}/{:.4}/{:.4} | {:+.4} | {:+.5} |",
            segment.label,
            travelled,
            turned.to_degrees(),
            paid_upkeep,
            paid_motion,
            eaten.0,
            eaten.1,
            eaten.2,
            end.reserve - start.reserve,
            end.energy - start.energy,
        );
        rows.push((segment.label, travelled, paid_motion, eaten.0));
    }

    println!(
        "\nIntake is the world's own `IntakeDiagnostics`: material that actually left a field \
         through a mouth, after the per-cell share and every clamp. It is **not** a cell-stock \
         delta — the settlement runs after movement, so the cell that gets grazed is not always \
         the one the body started the tick on, and a stock delta also contains growth, \
         mortality and decomposition. The motion column is the resolved motion priced through \
         `MotorBill::motor_cost`, not an energy difference: a feeding body's battery is being \
         credited by assimilation at the same time, which is why `Δenergy` can be positive on a \
         tick that paid for travel."
    );

    // The two checks the review asked for, stated as checks rather than left to the reader.
    let rest = rows.iter().find(|r| r.0.starts_with("rest")).expect("the rest row");
    assert_eq!(rest.3, 0.0, "a closed mouth must record exactly zero intake");
    let both = rows
        .iter()
        .find(|r| r.0.starts_with("travel and graze"))
        .expect("the travel-and-graze row");
    assert!(
        both.2 > 0.0 && both.3 > 0.0,
        "travelling while feeding must record both a motor debit and an intake: {both:?}"
    );
    println!(
        "\nChecked: the resting row records **zero** intake ({:.4} m), and the \
         travel-and-graze row records both a motor debit ({:.5} e) and an intake ({:.4} m).",
        rest.3, both.2, both.3
    );
}

fn crowd(bodies: u32) -> WorldConfig {
    let mut c = WorldConfig::default();
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    c.capacity.max_organisms = c.capacity.max_organisms.max(bodies);
    c.founders.kinds = vec![FounderKind {
        name: "screen".into(),
        count: bodies,
        size: Some(1.0),
        ..FounderKind::default()
    }];
    c.founders.count = bodies;
    // Births off: the screen measures a fixed active-body count, not a growing one.
    c.mechanisms.mutation = false;
    c
}

/// One arm: `bodies` bodies for `ticks` ticks, all legacy or all neural.
///
/// Returns the wall seconds, the live population at the end, and — for a neural arm — the
/// world's own timing of the sampler, the GRU forward pass and the action adapter, measured at
/// those three calls rather than inferred from the difference between two worlds.
fn arm(bodies: u32, ticks: u64, neural: bool) -> (f64, usize, NeuralTiming) {
    let mut world = World::new(crowd(bodies)).expect("valid");
    if neural {
        let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
        let policy = Policy::new(tape_policy([0.0, 0.3, 0.0, 0.0, 0.0, -8.0, -8.0]));
        for id in ids {
            world
                .attach_neural_policy(id, policy.clone())
                .expect("attach");
        }
    }
    let before = world.neural_timing();
    let start = Instant::now();
    for _ in 0..ticks {
        world.step();
        world.drain_events();
    }
    let seconds = start.elapsed().as_secs_f64();
    let timing = world.neural_timing().since(&before);
    (seconds, world.population(), timing)
}

fn throughput() {
    println!("\n# R1 throughput screen\n");
    println!(
        "| bodies | ticks | legacy s | legacy ticks/s | neural s | neural ticks/s | \
         world difference µs/body-tick | live at end |"
    );
    println!("| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    let mut total = 0.0;
    let mut components = Vec::new();
    for (bodies, ticks) in [(32u32, 2_000u64), (128, 2_000), (512, 200)] {
        let (legacy_s, _, _) = arm(bodies, ticks, false);
        let (neural_s, live, timing) = arm(bodies, ticks, true);
        total += legacy_s + neural_s;
        let body_ticks = f64::from(bodies) * ticks as f64;
        println!(
            "| {bodies} | {ticks} | {legacy_s:.3} | {:.0} | {neural_s:.3} | {:.0} | {:.3} | {live} |",
            ticks as f64 / legacy_s,
            ticks as f64 / neural_s,
            (neural_s - legacy_s) / body_ticks * 1.0e6,
        );
        components.push((bodies, ticks, body_ticks, timing));
    }

    println!(
        "\n**Component cost, measured at the calls.** The world difference above is not an \
         isolated measurement of the recurrent stage: the two arms also run different \
         controllers and follow different trajectories, so it contains the legacy controller \
         work that was *removed* as well as the neural work that was added. These three \
         columns are timed inside `world/step.rs` at the sampler, at `Gru32::forward` and at \
         the action adapter."
    );
    println!(
        "\n| bodies | sampler µs/call | inference µs/call | adapter µs/call | \
         sampler+inference+adapter µs/body-tick | calls (sample/infer/adapt) |"
    );
    println!("| ---: | ---: | ---: | ---: | ---: | --- |");
    for (bodies, _, body_ticks, t) in &components {
        let per = |nanos: u64, calls: u64| {
            if calls == 0 {
                0.0
            } else {
                nanos as f64 / calls as f64 / 1000.0
            }
        };
        let summed = (t.sampler_nanos + t.inference_nanos + t.adapter_nanos) as f64
            / body_ticks
            / 1000.0;
        println!(
            "| {bodies} | {:.3} | {:.3} | {:.3} | {summed:.3} | {}/{}/{} |",
            per(t.sampler_nanos, t.sampler_calls),
            per(t.inference_nanos, t.inference_calls),
            per(t.adapter_nanos, t.adapter_calls),
            t.sampler_calls,
            t.inference_calls,
            t.adapter_calls,
        );
    }
    println!(
        "\nThe sampler and the GRU run on the animal's controller tick — half the body-ticks, \
         which is what the call counts show — and the adapter runs on every one, because a \
         held action is rebuilt from the current transported heading each tick. Each timed \
         region is bracketed by two `Instant::now()` calls, worth a few tens of nanoseconds \
         together, so a sub-microsecond column carries a few percent of its own instrument."
    );
    println!("\nWall time for the whole screen: {total:.1} s.");
}

fn main() {
    action_tape();
    throughput();
}
