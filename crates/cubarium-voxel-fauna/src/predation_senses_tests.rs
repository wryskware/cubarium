//! Predation senses: the body kinds per observer, the still-threat rule, the tremor, and
//! bodies with volume in the cone occupancy (`design/predation-and-flight-2026-09-28.md`
//! §2.2).

use super::predation_fixture::*;
use crate::senses::{Fine, predation_occupancy, ray_first_hit};
use crate::{BodyKind, DT, Fauna, FaunaConfig, Founder, StartingStores, kind_of};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig};

/// Case 11, the lookup: prey, threat and other across the three lineages, the size
/// window, and the still-threat rule.
#[test]
fn kind_of_reads_prey_threat_and_other_per_observer() {
    let world = clock_off(11);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    let lj2 = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    let young = introduce(
        &mut fauna,
        &world,
        Founder::Lanternjaw,
        StartingStores {
            body: 0.6,
            reserve: 1.0,
        },
    );
    let grazer = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    let grazer2 = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    let shredder = introduce(&mut fauna, &world, Founder::Blind, StartingStores::FULL);
    let c = fauna.config;
    let [lj, lj2, young, grazer, grazer2, shredder] =
        [lj, lj2, young, grazer, grazer2, shredder].map(|id| animal(&fauna, id));
    let moving = |mut a: crate::Animal| {
        a.still_ticks = 0;
        a
    };
    let still = |mut a: crate::Animal| {
        a.still_ticks = (3.0 / DT) as u64;
        a
    };
    let recently = |mut a: crate::Animal| {
        a.still_ticks = (1.0 / DT) as u64;
        a
    };
    use BodyKind::{Other, Prey, Threat};

    // The hunter's view: both prey lineages, never its own.
    assert_eq!(kind_of(&c, &lj, &grazer, 1.0), Prey);
    assert_eq!(kind_of(&c, &lj, &shredder, 1.0), Prey);
    assert_eq!(kind_of(&c, &lj, &moving(lj2), 1.0), Other);
    // The prey's view of a moving hunter, and of one that moved a second ago.
    assert_eq!(kind_of(&c, &grazer, &moving(lj), 1.0), Threat);
    assert_eq!(kind_of(&c, &shredder, &moving(lj), 1.0), Threat);
    assert_eq!(kind_of(&c, &grazer, &recently(lj), 1.0), Threat);
    // A still hunter hides beyond 0.5 m and not inside it.
    assert_eq!(kind_of(&c, &grazer, &still(lj), 1.0), Other);
    assert_eq!(kind_of(&c, &grazer, &still(lj), 0.6), Other);
    assert_eq!(kind_of(&c, &grazer, &still(lj), 0.4), Threat);
    // Grazers and shredders are nothing to each other.
    assert_eq!(kind_of(&c, &grazer, &moving(shredder), 1.0), Other);
    assert_eq!(kind_of(&c, &shredder, &moving(grazer), 1.0), Other);
    assert_eq!(kind_of(&c, &grazer, &moving(grazer2), 1.0), Other);
    // The window follows the hunter's own body: a young one (0.021, window 0.0315) takes
    // a shredder (0.0125) and not an adult grazer (0.05), and is no threat to the grazer.
    assert_eq!(kind_of(&c, &young, &shredder, 1.0), Prey);
    assert_eq!(kind_of(&c, &young, &grazer, 1.0), Other);
    assert_eq!(kind_of(&c, &grazer, &moving(young), 1.0), Other);
    assert_eq!(kind_of(&c, &shredder, &moving(young), 1.0), Threat);
}

/// Distance between two bodies' centres on the plain (no case here crosses the seam).
fn apart(fauna: &Fauna, a: u64, b: u64) -> f64 {
    let (p, q) = (animal(fauna, a).pose, animal(fauna, b).pose);
    ((p.x - q.x).powi(2) + (p.z - q.z).powi(2)).sqrt()
}

/// Case 11, in the world: a walking lanternjaw 1 m from a shredder makes tremor and
/// reads as a threat; a still one makes none and, after 2 s of stillness beyond 0.5 m,
/// reads as other; a walking one 2 m off is outside the 1.5 m tremor radius.
#[test]
fn tremor_rises_for_a_walking_body_and_not_for_a_still_one() {
    let run = |walking: bool, offset_z: f64| -> (Fauna, World, u64, u64) {
        let world = clock_off(12);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(hunting_config());
        let shredder = introduce(&mut fauna, &world, Founder::Blind, StartingStores::FULL);
        place(&mut fauna, shredder, 4.0, 0.5, EAST);
        let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, HUNTING);
        place(&mut fauna, lj, 3.5, 0.5 + offset_z, EAST);
        if walking {
            drive(&mut fauna, lj, 1.0, false);
        }
        for _ in 0..45 {
            fauna.step_with(&world, &mut flora, 1);
        }
        (fauna, world, shredder, lj)
    };

    let (fauna, world, shredder, lj) = run(true, 1.0);
    let d = apart(&fauna, shredder, lj);
    assert!(d < 1.5, "the walker is {d} m off");
    assert!(animal(&fauna, lj).pose.x > 4.0, "the lanternjaw walked");
    assert_eq!(animal(&fauna, lj).still_ticks, 0);
    let felt = fauna.view().tremor(&world.view(), shredder);
    assert!(felt[0] + felt[1] > 0.0, "a walking body 1 m off makes no tremor: {felt:?}");
    let c = fauna.config;
    let (s, l) = (animal(&fauna, shredder), animal(&fauna, lj));
    assert_eq!(kind_of(&c, &s, &l, d), BodyKind::Threat);

    let (fauna, world, shredder, lj) = run(false, 1.0);
    let felt = fauna.view().tremor(&world.view(), shredder);
    assert_eq!(felt, [0.0, 0.0], "a still body makes tremor");
    let (s, l) = (animal(&fauna, shredder), animal(&fauna, lj));
    assert!(l.still_ticks >= 40, "still for {} ticks", l.still_ticks);
    assert_eq!(kind_of(&c, &s, &l, apart(&fauna, shredder, lj)), BodyKind::Other);

    let (fauna, world, shredder, lj) = run(true, 2.1);
    assert!(apart(&fauna, shredder, lj) > 1.5);
    let felt = fauna.view().tremor(&world.view(), shredder);
    assert_eq!(felt, [0.0, 0.0], "tremor from beyond 1.5 m");
}

/// Case 12: bodies have volume. At 0.125 m a 0.45 m lanternjaw fills four cells over its
/// face, so a ray at the height of its back hits it and a ray above it passes; the
/// 0.375 m grazer is three cells tall, so the same back-height ray passes over it.
#[test]
fn a_ray_at_the_height_of_a_lanternjaws_back_hits_it() {
    const V: f64 = 0.125;
    let mut world = World::empty(VoxelConfig {
        width: 64,
        height: 16,
        depth: 16,
        voxel_m: V,
        seed: 12,
        ..VoxelConfig::default()
    });
    for z in 0..16 {
        for x in 0..64 {
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
    let surface = 3.0 * V;
    let flora = Flora::new(FloraConfig::for_voxel_size(V));
    let mut fauna = Fauna::new(FaunaConfig::default());
    // Each body at a cell centre, in its own row.
    let mut put = |founder, row: u32| {
        assert!(fauna.apply(
            &world,
            crate::Command::IntroduceFounder {
                x: 32,
                z: row,
                founder,
                stores: StartingStores::FULL,
                heading_rad: std::f64::consts::FRAC_PI_2,
            }
        ));
        fauna.animals.last().unwrap().id
    };
    let lj = put(Founder::Lanternjaw, 4);
    let grazer = put(Founder::Browser, 11);
    let view = world.view();
    // Re-pointed at the predation occupancy: bodies have volume in the predation queries
    // until contract v3; the v2 cone keeps one cell per body (the coordinator's call).
    let occupancy = predation_occupancy(&view, &flora.view(), &fauna.view());
    let ray = |id: u64, above: f64| {
        let p = animal(&fauna, id).pose;
        ray_first_hit(
            &view,
            &occupancy,
            u64::MAX,
            (p.x - 1.5, surface + above, p.z),
            (1.0, 0.0, 0.0),
            3.0,
        )
        .map(|(_, fine)| fine)
    };
    assert_eq!(ray(lj, 0.40), Some(Fine::Body), "the lanternjaw's back");
    assert_eq!(ray(lj, 0.20), Some(Fine::Body), "its flank");
    assert_eq!(ray(lj, 0.55), None, "over its back");
    assert_eq!(ray(grazer, 0.30), Some(Fine::Body), "the grazer's back");
    assert_eq!(ray(grazer, 0.40), None, "over the grazer's back");
}
