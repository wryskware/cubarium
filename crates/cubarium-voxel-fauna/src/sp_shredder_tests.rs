//! Independent SP-SHREDDER mechanism fixtures; no census or tuned-default assertions.

#[allow(dead_code)]
#[path = "predation_fixture.rs"]
mod fixture;

use crate::{Activity, Fauna, FaunaConfig, Food, Founder};
use cubarium_voxel::{Command as WorldCommand, Material, World};
use cubarium_voxel_flora::{Face, FaceDir, Flora, FloraConfig, Site, SpurPhase, VineSeed};
use fixture::*;

const SP: Founder = Founder::Seedporter;
const SH: Founder = Founder::Blind;

fn scene(on: bool) -> (World, Flora, Fauna, u64, u64, f64) {
    let world = at_hour(12.0, 861);
    let flora = Flora::new(FloraConfig::default());
    let mut config = FaunaConfig::default();
    config.perception.remote_hz = f64::from(crate::TICK_HZ);
    let phys = &mut config.founders[SP.index()];
    phys.active = Activity::Always;
    phys.catch_bellwings = on;
    phys.catch_gain_min = 0.0;
    phys.catch_ceiling_m = 0.15;
    let hunter = phys.hunter.as_mut().expect("seedporter opportunist table");
    // Preserve the shipped diet membership; make just this contact deterministic.
    hunter.strike_power = 8.0;
    hunter.strike_reach_length_fraction = 0.8;
    hunter.jaw_forward_length_fraction = 0.5;
    hunter.jaw_band_height_fraction = 4.0;
    hunter.windup_s = 0.1;
    hunter.lunge_s = 0.15;
    hunter.recovery_s = 0.5;
    hunter.strike_cost_s = 0.1;
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let sp = introduce(&mut fauna, &world, SP, HUNTING);
    place(&mut fauna, sp, 3.0, LANE_Z, EAST);
    animal_mut(&mut fauna, sp).pose.y_m = SURFACE_M;
    animal_mut(&mut fauna, sp).eating = true;
    drive(&mut fauna, sp, 0.0, true);
    let phys = fauna.config.founder(SP);
    let geometry = phys.body_at(animal(&fauna, sp).body);
    let hunter = phys.hunter.unwrap();
    let ahead = (hunter.jaw_forward_length_fraction + 0.5 * hunter.strike_reach_length_fraction)
        * geometry.length_m;
    let sh = introduce(&mut fauna, &world, SH, HUNTING);
    place(&mut fauna, sh, 3.0 + ahead, LANE_Z, EAST);
    animal_mut(&mut fauna, sh).pose.y_m = SURFACE_M;
    drive(&mut fauna, sh, 0.0, false);
    (world, flora, fauna, sp, sh, ahead)
}

fn target(world: &World, fauna: &Fauna, sp: u64) -> Option<u64> {
    let i = fauna.animals.iter().position(|a| a.id == sp).unwrap();
    let phys = fauna.config.founder(SP);
    let geometry = phys.body_at(fauna.animals[i].body);
    crate::step::prey_in_capsule(fauna, i, &world.view(), &geometry, &phys.hunter.unwrap())
}

#[test]
fn seedporter_catches_and_eats_a_reachable_littershredder() {
    let (world, mut flora, mut fauna, sp, sh, _) = scene(true);
    assert_eq!(
        target(&world, &fauna, sp),
        Some(sh),
        "reachable prey is selected"
    );
    for _ in 0..160 {
        fauna.step_with(&world, &mut flora, 1);
        if fauna.ledger.bites_by_founder_food[SP.index()][Food::Carrion.index()] > 0 {
            break;
        }
    }
    assert!(
        fauna.ledger.strikes_by_founder[SP.index()] > 0,
        "seedporter strikes"
    );
    assert!(
        fauna.ledger.hits_by_founder[SP.index()] > 0,
        "strike connects"
    );
    assert!(fauna.view().animal(sh).is_none(), "shredder is killed");
    assert!(
        fauna.ledger.bites_by_founder_food[SP.index()][Food::Carrion.index()] > 0,
        "shredder supplies an actual meal"
    );
    assert_closes(&fauna, "seedporter shredder catch");
}

#[test]
fn seedporter_catch_switch_off_spares_a_reachable_littershredder() {
    let (world, mut flora, mut fauna, sp, sh, _) = scene(false);
    assert_eq!(target(&world, &fauna, sp), None);
    for _ in 0..80 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert_eq!(fauna.ledger.strikes_by_founder[SP.index()], 0);
    assert!(
        fauna.view().animal(sh).is_some(),
        "disabled catch spares prey"
    );
    assert_closes(&fauna, "disabled seedporter shredder catch");
}

#[test]
fn seedporter_rejects_littershredders_outside_reach_or_above_ceiling() {
    let (world, _, mut fauna, sp, sh, ahead) = scene(true);
    assert_eq!(
        target(&world, &fauna, sp),
        Some(sh),
        "fixture starts reachable"
    );
    animal_mut(&mut fauna, sh).pose.x = animal(&fauna, sp).pose.x + 2.0;
    assert_eq!(
        target(&world, &fauna, sp),
        None,
        "distant shredder is not taken"
    );
    animal_mut(&mut fauna, sh).pose.x = animal(&fauna, sp).pose.x + ahead;
    let ceiling = fauna.config.founder(SP).catch_ceiling_m;
    animal_mut(&mut fauna, sh).pose.y_m = SURFACE_M + ceiling + 0.025;
    assert_eq!(
        target(&world, &fauna, sp),
        None,
        "ceiling independently rejects prey"
    );
}

#[test]
fn seedporter_prefers_reachable_fruit_over_a_reachable_littershredder() {
    let (mut world, _, mut fauna, sp, sh, _) = scene(true);
    let face = Face::new(12, 3, 7, FaceDir::NegZ);
    let rock = WorldCommand::SetMaterial {
        x: 12,
        y: 3,
        z: 7,
        material: Material::Rock,
    };
    world.apply(rock);
    let geometry = fauna.config.founder(SP).body_at(animal(&fauna, sp).body);
    let gap = geometry.footprint_radius() + 0.5 * geometry.mouth_reach_m;
    let z = 7.0 * VOXEL_M - gap;
    animal_mut(&mut fauna, sp).pose.z = z;
    animal_mut(&mut fauna, sh).pose.z = z;
    let mut config = FloraConfig::default();
    config.latticevine.spur_reserve_min = 0.1;
    config.latticevine.spur_share = 1.0;
    config.latticevine.bud_s = 0.5;
    config.latticevine.flower_s = 0.5;
    config.latticevine.fruit_s = 60.0;
    config.latticevine.spent_s = 0.5;
    config.latticevine.nectar_per_face_s = 0.01 / 61.5;
    config.latticevine.fruit_per_face_s = 0.02 / 61.5;
    config.latticevine.spread_cost = 1e12;
    config.latticevine.sister_cost = 1e12;
    let mut flora = Flora::new(config);
    flora
        .seed_vine(
            &world,
            VineSeed {
                root: Site {
                    x: 12,
                    y: SURFACE_Y,
                    z: 6,
                },
                first: face,
                reserve: 100.0,
                lineage: None,
            },
        )
        .expect("fixture vine roots next to the mouth");
    // One shower pays the spur's water cost; its geometry matches the dry scene.
    let mut wet_config = world.config().clone();
    wet_config.rain_m_per_s = 1e-6;
    let mut wet = World::empty(wet_config);
    for z in [6, 7] {
        for y in 1..=SURFACE_Y {
            wet.apply(WorldCommand::SetMaterial {
                x: 12,
                y,
                z,
                material: Material::Soil,
            });
        }
    }
    wet.apply(WorldCommand::SetMaterial {
        x: 12,
        y: 3,
        z: 7,
        material: Material::Rock,
    });
    assert!(wet.view().is_raining(), "fixture shower supplies water");
    flora.step(&mut wet);
    for _ in 0..160 {
        if flora
            .view()
            .cover
            .face(face)
            .is_some_and(|c| c.spur == SpurPhase::Fruit && c.fruit > 0.0)
        {
            break;
        }
        flora.step(&mut world);
    }
    assert!(
        flora
            .view()
            .cover
            .face(face)
            .is_some_and(|c| c.spur == SpurPhase::Fruit && c.fruit > 0.0),
        "fixture hangs fruit at ground height"
    );
    let i = fauna.animals.iter().position(|a| a.id == sp).unwrap();
    assert!(
        crate::step::glider_fruit_in_reach(
            &world.view(),
            &flora.view(),
            &animal(&fauna, sp),
            fauna.config.founder(SP)
        )
        .is_some(),
        "fruit physically reaches the mouth"
    );
    assert_eq!(
        target(&world, &fauna, sp),
        Some(sh),
        "prey also physically reachable"
    );
    assert!(
        crate::hunt::catch_step(&mut fauna, i, &world.view(), &flora.view()).is_none(),
        "catch steering leaves reachable fruit to normal feeding"
    );
    for _ in 0..40 {
        fauna.step_with(&world, &mut flora, 1);
        if fauna.ledger.bites_by_founder_food[SP.index()][Food::Fruit.index()] > 0 {
            break;
        }
    }
    assert!(
        fauna.ledger.bites_by_founder_food[SP.index()][Food::Fruit.index()] > 0,
        "seedporter takes fruit with both foods at hand"
    );
    assert_eq!(
        fauna.ledger.strikes_by_founder[SP.index()],
        0,
        "fruit takes priority over striking"
    );
    assert!(
        fauna.view().animal(sh).is_some(),
        "supplementary prey survives the fruit meal"
    );
    assert_closes(&fauna, "seedporter fruit priority");
}
