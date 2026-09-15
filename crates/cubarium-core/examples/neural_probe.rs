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

use std::time::Instant;

use cubarium_core::config::FounderKind;
use cubarium_core::ids::OrganismId;
use cubarium_core::motor::MotorBill;
use cubarium_core::neural::gru::{Gru32, OUTPUT};
use cubarium_core::neural::Policy;
use cubarium_core::{DT, World, WorldConfig};
use cubarium_surface::{Face, SurfacePoint, cell_of};

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
    for cell in cubarium_surface::CellId::all() {
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
        "| segment | s | px travelled | net turn (deg) | energy spent (e) | upkeep (e) | \
         motor (e) | P eaten (m) | reserve gained (m) |"
    );
    println!("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    for segment in segments {
        let (mut world, id) = probe_world();
        world
            .attach_neural_policy(id, Policy::new(tape_policy(segment.head)))
            .expect("attach");
        let start = world.state.organisms.get(id).expect("alive").clone();
        let bill = MotorBill::of(&start, world.config());
        let mut travelled = 0.0f64;
        let mut turned = 0.0f64;
        let mut eaten = 0.0f64;
        let mut heading = start.heading;
        for _ in 0..segment.ticks {
            let here = cell_of(&world.state.organisms.get(id).expect("alive").pos).index();
            let before_p = world.state.fields.p[here];
            world.step();
            world.drain_events();
            let Some(o) = world.state.organisms.get(id) else {
                break;
            };
            travelled += world
                .moved_segments(id)
                .iter()
                .map(cubarium_surface::PathSegment::length)
                .sum::<f64>();
            // Only count a turn that stayed on one face: a seam crossing changes the chart.
            if world
                .moved_segments(id)
                .first()
                .is_none_or(|f| world.moved_segments(id).iter().all(|s| s.face == f.face))
            {
                let d = (o.heading.screen_angle() - heading.screen_angle()).rem_euclid(
                    std::f64::consts::TAU,
                );
                let d = if d > std::f64::consts::PI {
                    d - std::f64::consts::TAU
                } else {
                    d
                };
                // Clockwise in the body frame is the negative screen angle.
                turned += -d;
            }
            heading = o.heading;
            eaten += (before_p - world.state.fields.p[here]).max(0.0);
        }
        let end = world.state.organisms.get(id).expect("alive");
        let spent = start.energy - end.energy;
        let upkeep = bill.upkeep(DT) * segment.ticks as f64;
        println!(
            "| {} | {:.1} | {:.2} | {:.1} | {:.5} | {:.5} | {:.5} | {:.4} | {:.4} |",
            segment.label,
            segment.ticks as f64 * DT,
            travelled,
            turned.to_degrees(),
            spent,
            upkeep,
            (spent - upkeep).max(0.0),
            eaten,
            end.reserve - start.reserve,
        );
    }
    println!(
        "\nRest spends upkeep and nothing else; pure pivot turns and pays for the sweep with no \
         travel; the split trades pixels for radians at exactly `r` px per radian; grazing in \
         place fills the reserve while travel-and-graze does both out of one mouth. The two \
         grazing rows spend zero *net* energy because assimilation credits the battery faster \
         than upkeep drains it and the battery is already at its ceiling: the upkeep column is \
         still the bill that was charged."
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
fn arm(bodies: u32, ticks: u64, neural: bool) -> (f64, usize) {
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
    let start = Instant::now();
    for _ in 0..ticks {
        world.step();
        world.drain_events();
    }
    let seconds = start.elapsed().as_secs_f64();
    (seconds, world.population())
}

fn throughput() {
    println!("\n# R1 throughput screen\n");
    println!(
        "| bodies | ticks | legacy s | legacy ticks/s | neural s | neural ticks/s | \
         sampler+inference µs/body-tick | live at end |"
    );
    println!("| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    let mut total = 0.0;
    for (bodies, ticks) in [(32u32, 2_000u64), (128, 2_000), (512, 200)] {
        let (legacy_s, _) = arm(bodies, ticks, false);
        let (neural_s, live) = arm(bodies, ticks, true);
        total += legacy_s + neural_s;
        let per_body_tick =
            (neural_s - legacy_s) / (f64::from(bodies) * ticks as f64) * 1.0e6;
        println!(
            "| {bodies} | {ticks} | {legacy_s:.3} | {:.0} | {neural_s:.3} | {:.0} | {:.3} | {live} |",
            ticks as f64 / legacy_s,
            ticks as f64 / neural_s,
            per_body_tick,
        );
    }
    println!(
        "\nThe neural column is the whole world tick with every body neural; the difference \
         column is the sampler, the GRU and the adapter together, amortised over every \
         body-tick (inference itself runs on half of them). Wall time for the whole screen: \
         {total:.1} s."
    );
}

fn main() {
    action_tape();
    throughput();
}
