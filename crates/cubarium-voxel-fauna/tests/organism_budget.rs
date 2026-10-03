//! **The organism budget kernel, through the animal layer's public step**: a feeding
//! juvenile (feeding is income, stored in the reserve; structure built out of the reserve;
//! upkeep from the reserve) and a starving adult (upkeep from the reserve until it is
//! spent, then the unpaid rest out of the structure), a few hundred ticks, both layers'
//! ledgers closed on every tick. Fixtures follow `grazing.rs`; no pinned numbers.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Fauna, FaunaConfig, Founder, Scripted, StartingStores,
};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species as Plant};

/// 8 × 6 × 6 voxels at 0.25 m, soil 1..=2, the support face at y = 2 everywhere.
fn flat_world() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 6,
        depth: 6,
        voxel_m: 0.25,
        day: cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..8 {
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
    world
}

fn browser(
    fauna: &mut Fauna,
    world: &World,
    x: i64,
    z: u32,
    stores: StartingStores,
    feed: f64,
) -> u64 {
    let id = fauna.view().ledger.births;
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x,
            z,
            founder: Founder::Browser,
            stores,
            heading_rad: 0.0,
        },
    ));
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 0.0,
            turn: 0.0,
            feed,
        }])),
    ));
    id
}

fn assert_closed(fauna: &Fauna, flora: &Flora, when: &str) {
    let av = fauna.view();
    let fv = flora.view();
    for (name, got, want) in [
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
    ] {
        assert!(
            (got - want).abs() <= 1e-9 * got.abs().max(1.0),
            "{when}: the {name} ledger is {got:e} against {want:e}"
        );
    }
}

#[test]
fn animals_close_both_ledgers_through_feeding_growth_upkeep_and_starvation() {
    let mut world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    for z in 2..=4 {
        let wood = flora.config().species(Plant::Springturf).wood_max;
        assert!(flora.apply(
            &world,
            FloraCommand::Seed {
                x: 2,
                z,
                species: Plant::Springturf,
                wood,
            },
        ));
    }
    // A growth rate that shows within the run, and a bite big enough to keep up with it.
    let mut config = FaunaConfig::placeholder();
    {
        let core = &mut config.founders[Founder::Browser.index()].core;
        core.growth_max_per_s = (core.birth_body - core.body_min) / (100.0 * DT);
        // The pre-gut upkeep, so starvation shows within the run's 300 ticks (the gut's
        // per-day upkeep would take hours).
        core.maintenance_per_s = 0.001;
        // The minimal gut: a mouth and a digestion fast enough to keep up with growth.
        let gut = &mut config.founders[Founder::Browser.index()].gut;
        gut.bite_per_body_s = 50.0;
        gut.digestion_s = [0.05; cubarium_voxel_fauna::Food::COUNT];
    }
    let sc = config.founder(Founder::Browser).core;
    assert!(sc.maintenance_per_s > 0.0, "the fixture needs an upkeep");
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let feeder = browser(
        &mut fauna,
        &world,
        2,
        2,
        StartingStores {
            body: sc.body_min / sc.body_max,
            reserve: 0.5,
        },
        1.0,
    );
    // Far from the turf, not feeding, a sliver of reserve over a full structure.
    let starver = browser(
        &mut fauna,
        &world,
        6,
        0,
        StartingStores {
            body: 1.0,
            reserve: 0.02,
        },
        0.0,
    );
    assert_closed(&fauna, &flora, "introduced");

    let feeder_body0 = fauna.view().animal(feeder).unwrap().body;
    let starver_body0 = fauna.view().animal(starver).unwrap().body;
    let mut starver_spent = false;
    for tick in 1..=300 {
        let before = fauna.view().animal(starver).copied();
        world.step();
        flora.step(&mut world);
        fauna.step(&world, &mut flora);
        let when = format!("tick {tick}");
        assert_closed(&fauna, &flora, &when);
        for a in fauna.view().animals.iter() {
            assert!(
                a.reserve.is_finite() && a.reserve >= -1e-12,
                "{when}: animal {} reserve {:e}",
                a.id,
                a.reserve
            );
            assert!(
                a.body.is_finite() && a.body >= 0.0,
                "{when}: body {:e}",
                a.body
            );
        }
        // Upkeep comes out of the reserve before the structure: while any reserve is left
        // at the end of a tick, the starver's structure did not shrink in it.
        if let (Some(b), Some(a)) = (before, fauna.view().animal(starver)) {
            if a.reserve > 0.0 {
                assert!(
                    a.body >= b.body,
                    "{when}: structure {:e} -> {:e} with {:e} reserve left",
                    b.body,
                    a.body,
                    a.reserve
                );
            } else {
                starver_spent = true;
            }
        }
    }
    let v = fauna.view();
    assert!(v.ledger.eaten_organic_in > 0.0, "the feeder never ate");
    assert!(
        v.ledger.respired_maintenance_out > 0.0,
        "no upkeep was respired"
    );
    let feeder_body = v.animal(feeder).expect("the feeder starved").body;
    assert!(
        feeder_body > feeder_body0,
        "the fed juvenile built no structure: {feeder_body0} -> {feeder_body}"
    );
    assert!(starver_spent, "the starver never spent its reserve");
    assert!(
        v.animal(starver).is_none_or(|a| a.body < starver_body0),
        "unpaid upkeep took no structure from the starver"
    );
}
