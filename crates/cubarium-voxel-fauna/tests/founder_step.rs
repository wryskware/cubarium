//! A founder that steps a terrace conserves matter and energy
//! (`design/handoffs/voxel-founder-step-2026-09-22.md`, deliverable 3, last case).
//!
//! The step rule moves a body's standing layer. Nothing about that is a transfer, so
//! the fauna ledger's residual across the step must be exactly what it was standing
//! still: the body only ever pays motor respiration and upkeep. Written from the brief
//! before the implementation; 120 ticks.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig};

use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Fauna, FaunaConfig, Founder, Scripted, StartingStores,
};

/// A terraced strip at 0.25 m: soil in `1..=2` everywhere, one voxel more in columns
/// 5..8, so a body walking east off column 3 meets a 0.25 m riser and then a 0.25 m
/// drop back.
fn terraced_world() -> World {
    let config = VoxelConfig {
        width: 16,
        height: 10,
        depth: 6,
        voxel_m: 0.25,
        ..VoxelConfig::default()
    };
    let mut world = World::empty(config);
    for z in 0..6 {
        for x in 0..16 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    for z in 0..6 {
        for x in 5..8 {
            world.apply(WorldCommand::SetMaterial {
                x,
                y: 3,
                z,
                material: Material::Soil,
            });
        }
    }
    world
}

#[test]
fn a_founder_that_steps_a_terrace_creates_no_matter() {
    let world = terraced_world();
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 3,
            z: 2,
            founder: Founder::Browser,
            stores: StartingStores::FULL,
            heading_rad: std::f64::consts::FRAC_PI_2,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 1.0,
            turn: 0.0,
            feed: 0.0,
        }])),
    ));
    let start_y = fauna.view().animal(id).expect("alive").site.y;

    let mut climbed = false;
    let mut descended = false;
    for _ in 0..120 {
        fauna.step(&world, &mut flora);
        if let Some(a) = fauna.view().animal(id) {
            if a.site.y > start_y {
                climbed = true;
            } else if climbed && a.site.y == start_y {
                descended = true;
            }
        }
    }
    assert!(climbed, "the body never stepped onto the terrace");
    assert!(descended, "the body never stepped back off it");

    // Conservation. Every gram this layer holds came in through `introduced_*` or
    // `eaten_*` and left through `respired_out` / `deposited_*` / `removed_*`; a change
    // of standing layer is not a transfer and must not appear anywhere.
    let v = fauna.view();
    let l = v.ledger;
    let held_organic: f64 = v.animals.iter().map(|a| a.body + a.reserve).sum();
    let held_mineral: f64 = v.animals.iter().map(|a| a.mineral).sum();
    let held_energy: f64 = v.animals.iter().map(|a| a.energy).sum();
    let organic_residual = l.introduced_organic_in + l.eaten_organic_in
        - l.respired_out
        - l.deposited_organic_out
        - l.removed_organic_out
        - held_organic;
    let mineral_residual = l.introduced_mineral_in + l.eaten_mineral_in
        - l.deposited_mineral_out
        - l.removed_mineral_out
        - held_mineral;
    let energy_residual = l.introduced_energy_in + l.eaten_energy_in
        - l.heat_out
        - l.deposited_energy_out
        - l.removed_energy_out
        - held_energy;
    for (name, r) in [
        ("organic", organic_residual),
        ("mineral", mineral_residual),
        ("energy", energy_residual),
    ] {
        assert!(r.abs() < 1e-12, "{name} residual across the step is {r}");
    }
    assert_eq!(l.eaten_organic_in, 0.0, "nothing was eaten on this walk");
    assert!(
        l.respired_motor_out > 0.0,
        "the walk was paid for, so the step is not free"
    );
}
