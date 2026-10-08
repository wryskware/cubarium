//! Independent, short mechanism fixtures for den rest and a browser's birth wait.

use super::{advance_eligibility, circadian, gestate};
use crate::{
    Actions, Activity, Command, DenChoice, Fauna, FaunaConfig, Founder, Reproduction, Rest,
    StartingStores, State,
};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site, Species};
use std::cmp::Ordering;
use std::f64::consts::FRAC_PI_2;

const C: Founder = Founder::Chorister;
const V: f64 = 0.5;
const FLOOR: u32 = 2;

fn site(x: u32, z: u32) -> Site {
    Site { x, y: FLOOR, z }
}

fn plain() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 40,
        height: 16,
        depth: 12,
        voxel_m: V,
        seed: 901,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    for z in 0..12 {
        for x in 0..40 {
            for y in 1..=FLOOR {
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

fn roof(world: &mut World, xs: std::ops::RangeInclusive<u32>, zs: std::ops::RangeInclusive<u32>) {
    for z in zs {
        for x in xs.clone() {
            world.apply(WorldCommand::SetMaterial {
                x: i64::from(x),
                y: 8,
                z,
                material: Material::Soil,
            });
        }
    }
}

fn wall(world: &mut World, x: i64) {
    for z in 0..12 {
        for y in FLOOR + 1..16 {
            world.apply(WorldCommand::SetMaterial {
                x,
                y,
                z,
                material: Material::Soil,
            });
        }
    }
}

fn pack_config() -> FaunaConfig {
    let mut config = FaunaConfig::default();
    config.founding.clearance_margin_m = 0.0;
    let phys = &mut config.founders[C.index()];
    phys.active = Activity::Nocturnal;
    phys.core.reserve_cap = 10.0;
    phys.core.maintenance_per_s = 0.001;
    phys.motor_respiration_per_s = 0.001;
    let pack = phys.pack.as_mut().expect("pack lineage");
    pack.pack_size = 1;
    pack.den_search_m = 12.0;
    pack.den_cover_m = 3.0;
    pack.den_radius_m = 0.5;
    pack.rest_radius_m = 0.25;
    pack.home_reserve_share = 1.0;
    config
}

fn found(world: &World, den: Site) -> (Fauna, u64) {
    let mut fauna = Fauna::new(pack_config());
    fauna.set_births_enabled(false);
    let pack = fauna
        .found_pack_sized(world, den, C, StartingStores::FULL, FRAC_PI_2, 1)
        .expect("one adult");
    let id = fauna.animals[0].id;
    let teachers = fauna.config.teachers;
    fauna
        .controllers
        .set(id, crate::controller::heuristic(C, teachers));
    (fauna, pack)
}

fn place(fauna: &mut Fauna, site: Site) {
    let a = &mut fauna.animals[0];
    a.site = site;
    (a.pose.x, a.pose.z) = crate::pack::centre(site, V);
    a.pose.y_m = crate::surface_m(site.y, V);
    a.pose.heading_rad = FRAC_PI_2;
    a.eating = false;
    a.emergency = false;
}

#[test]
fn a_dark_cave_beats_an_overhang_and_any_roof_beats_open_ground() {
    let mut world = plain();
    roof(&mut world, 9..=17, 2..=9);
    roof(&mut world, 25..=25, 5..=5);
    let (mut fauna, _) = found(&world, site(13, 6));
    let flora = Flora::new(FloraConfig::default());
    let glowcap = flora.config().species(Species::Glowcap);
    let pack = fauna.config.founder(C).pack.unwrap();
    let view = world.view();
    let cave = crate::den_quality(&view, glowcap, site(13, 6), &pack);
    let overhang = crate::den_quality(&view, glowcap, site(25, 5), &pack);
    let open = crate::den_quality(&view, glowcap, site(32, 5), &pack);
    assert!(
        cave.sky < overhang.sky,
        "fixture: cave has less sky than the narrow overhang"
    );
    assert!(cave.darkness >= overhang.darkness);
    assert_eq!(cave.compare(overhang), Ordering::Less);
    assert_eq!(overhang.compare(open), Ordering::Less);
    assert!(!open.roofed());
    let graph = fauna.pack_routes(&view, C);
    let component = graph.components[graph.face(site(13, 6)).expect("dry cave face")];
    for s in [site(25, 5), site(32, 5)] {
        assert_eq!(
            graph.components[graph.face(s).expect("walkable face")],
            component
        );
        assert_eq!(view.water_depth_m(i64::from(s.x), s.y, s.z), 0.0);
    }
}

#[test]
fn a_fed_adult_walks_home_then_rests_inside_the_configured_radius() {
    let world = plain();
    let den = site(10, 6);
    let (mut fauna, _) = found(&world, den);
    let flora = Flora::new(FloraConfig::default());
    place(&mut fauna, site(7, 6));
    let act = crate::pack::steer(
        &mut fauna,
        0,
        &world.view(),
        &flora.view(),
        false,
        1.0,
        Actions::REST,
        None,
    )
    .expect("pack tactic");
    assert!(
        act.forward > 0.0,
        "the fed adult starts its trip rather than resting in the field"
    );
    assert_eq!(act.feed, 0.0);
    place(&mut fauna, den);
    let act = crate::pack::steer(
        &mut fauna,
        0,
        &world.view(),
        &flora.view(),
        false,
        1.0,
        Actions::REST,
        None,
    )
    .expect("pack tactic");
    assert_eq!((act.forward, act.feed), (0.0, 0.0));
    assert!(!crate::pack::must_walk_home(&mut fauna, 0, &world.view()));
    let radius = fauna.config.founder(C).pack.unwrap().rest_radius_m;
    fauna.animals[0].pose.x += 0.9 * radius;
    assert!(!crate::pack::must_walk_home(&mut fauna, 0, &world.view()));
    fauna.animals[0].pose.x += 0.2 * radius;
    assert!(
        crate::pack::must_walk_home(&mut fauna, 0, &world.view()),
        "the rest radius is the arrival rule"
    );
}

#[test]
fn sleep_waits_for_home_but_an_unaffordable_trip_sleeps_in_the_field() {
    let world = plain();
    let den = site(10, 6);
    let (mut fauna, _) = found(&world, den);
    place(&mut fauna, site(7, 6));
    circadian(&mut fauna, &world.view(), &Flora::new(FloraConfig::default()).view(), true, 1.0);
    assert_ne!(fauna.animals[0].state, State::Asleep);
    assert_eq!(fauna.animals[0].rest, None);
    place(&mut fauna, den);
    circadian(&mut fauna, &world.view(), &Flora::new(FloraConfig::default()).view(), true, 1.0);
    assert_eq!(fauna.animals[0].state, State::Asleep);
    assert_eq!(fauna.animals[0].rest, Some(Rest::Sleep));
    place(&mut fauna, site(7, 6));
    fauna.config.founders[C.index()]
        .pack
        .as_mut()
        .unwrap()
        .home_reserve_share = 0.0;
    circadian(&mut fauna, &world.view(), &Flora::new(FloraConfig::default()).view(), true, 1.0);
    assert_eq!(fauna.animals[0].state, State::Asleep);
    assert!(
        !fauna.animals[0].emergency,
        "field sleep comes from the trip budget"
    );
}

#[test]
fn the_home_budget_uses_current_reserves_above_the_wake_floor() {
    let world = plain();
    let (mut fauna, _) = found(&world, site(10, 6));
    place(&mut fauna, site(7, 6));
    assert!(crate::pack::must_walk_home(&mut fauna, 0, &world.view()));
    let phys = *fauna.config.founder(C);
    fauna.animals[0].reserve =
        (1.0 - phys.wake_hunger) * phys.core.reserve_of(fauna.animals[0].body);
    assert!(
        !crate::pack::must_walk_home(&mut fauna, 0, &world.view()),
        "the trip cannot spend the emergency wake reserve"
    );
}

#[test]
fn a_home_trip_is_budgeted_for_its_detour_and_has_no_elapsed_time_cutoff() {
    let mut world = plain();
    for z in 0..8 {
        for y in FLOOR + 1..16 {
            world.apply(WorldCommand::SetMaterial {
                x: 9,
                y,
                z,
                material: Material::Soil,
            });
        }
    }
    let den = site(11, 4);
    let (mut fauna, pack) = found(&world, den);
    place(&mut fauna, site(7, 4));
    let view = world.view();
    let graph = fauna.pack_routes(&view, C);
    let from = graph.face(fauna.animals[0].site).expect("start face");
    let to = graph.face(den).expect("home face");
    let (_, field) = fauna.den_path(&view, C, pack, to);
    let route_m = f64::from(field[&from]) * V;
    let straight_m = 4.0 * V;
    assert!(route_m > straight_m, "fixture: the wall forces a detour");
    let phys = *fauna.config.founder(C);
    let a = fauna.animals[0];
    let cruise = phys.cruise_body_lengths_per_s * phys.body_at(a.body).length_m;
    let spend_per_m =
        a.body * (phys.core.maintenance_per_s + phys.motor_respiration_per_s) / cruise;
    let wake_floor = (1.0 - phys.wake_hunger) * phys.core.reserve_of(a.body);
    fauna.animals[0].reserve = wake_floor + 0.5 * (straight_m + route_m) * spend_per_m;
    assert!(
        !crate::pack::must_walk_home(&mut fauna, 0, &view),
        "reserves fund the straight line but not the actual walk"
    );
    fauna.animals[0].reserve = phys.core.reserve_of(a.body);
    fauna.animals[0].homing_ticks = 1_000_000;
    assert!(
        crate::pack::must_walk_home(&mut fauna, 0, &view),
        "elapsed homing time does not excuse an affordable trip"
    );
}

#[test]
fn a_fed_pregnant_breeder_rests_at_home_even_with_a_held_feed_action() {
    let world = plain();
    let den = site(10, 6);
    let (mut fauna, _) = found(&world, den);
    place(&mut fauna, den);
    fauna.config.founders[C.index()]
        .pack
        .as_mut()
        .unwrap()
        .whelp_at_den = true;
    fauna.animals[0].reproduction.escrow = Some(crate::Escrow {
        young: 1,
        ..crate::Escrow::default()
    });
    let flora = Flora::new(FloraConfig::default());
    let held = Actions {
        forward: 0.0,
        turn: 0.0,
        feed: 1.0,
    };
    let act = crate::pack::steer(
        &mut fauna,
        0,
        &world.view(),
        &flora.view(),
        false,
        1.0,
        held,
        None,
    )
    .expect("pack tactic");
    assert_eq!(
        (act.forward, act.feed),
        (0.0, 0.0),
        "a finished meal rests even while gestating"
    );
    place(&mut fauna, site(7, 6));
    fauna.config.founders[C.index()]
        .pack
        .as_mut()
        .unwrap()
        .home_reserve_share = 0.0;
    let act = crate::pack::steer(
        &mut fauna,
        0,
        &world.view(),
        &flora.view(),
        false,
        1.0,
        held,
        None,
    )
    .unwrap();
    assert_eq!(
        (act.forward, act.feed),
        (0.0, 0.0),
        "a pregnant fed adult also respects the trip reserve budget"
    );
}

#[test]
fn an_unreachable_den_allows_field_sleep_and_is_reselected_on_the_same_component() {
    let mut world = plain();
    roof(&mut world, 16..=24, 2..=9);
    roof(&mut world, 7..=7, 6..=6);
    let den = site(20, 6);
    let (mut fauna, pack) = found(&world, den);
    place(&mut fauna, site(7, 6));
    wall(&mut world, 12);
    wall(&mut world, 30);
    assert!(!crate::pack::must_walk_home(&mut fauna, 0, &world.view()));
    circadian(&mut fauna, &world.view(), &Flora::new(FloraConfig::default()).view(), true, 1.0);
    assert_eq!(fauna.animals[0].state, State::Asleep);
    let flora = Flora::new(FloraConfig::default());
    crate::pack::renew_dens(
        &mut fauna,
        &world.view(),
        flora.config().species(Species::Glowcap),
    );
    let new_den = fauna.pack(pack).unwrap().den;
    assert_ne!(new_den, den);
    let graph = fauna.pack_routes(&world.view(), C);
    assert_eq!(
        graph.components[graph.face(new_den).unwrap()],
        graph.components[graph.face(fauna.animals[0].site).unwrap()]
    );
    assert!(
        crate::den_quality(
            &world.view(),
            flora.config().species(Species::Glowcap),
            new_den,
            &fauna.config.founder(C).pack.unwrap()
        )
        .roofed()
    );
}

#[test]
fn a_flooded_den_moves_to_a_dry_roof_and_a_usable_den_stays_put() {
    let mut world = plain();
    roof(&mut world, 9..=17, 2..=9);
    let old = site(6, 6);
    let (mut fauna, pack) = found(&world, old);
    place(&mut fauna, site(7, 6));
    let flora = Flora::new(FloraConfig::default());
    let glowcap = flora.config().species(Species::Glowcap);
    crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
    assert_eq!(
        fauna.pack(pack).unwrap().den,
        old,
        "usable dens are stable even with a better cave nearby"
    );
    assert!(
        world.apply(WorldCommand::AddWater {
            x: i64::from(old.x),
            y: old.y + 1,
            z: old.z,
            volume_m3: world.config().voxel_volume()
        }) > 0.0
    );
    crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
    let den = fauna.pack(pack).unwrap().den;
    assert_ne!(den, old);
    assert_eq!(
        world.view().water_depth_m(i64::from(den.x), den.y, den.z),
        0.0
    );
    assert_eq!(
        fauna.pack(pack).unwrap().den_choice,
        Some(DenChoice::Covered)
    );
}

#[test]
fn an_off_graph_pack_with_a_wet_den_keeps_the_route_refresh_cadence() {
    use std::sync::Arc;
    let mut world = plain();
    let den = site(6, 6);
    let (mut fauna, pack) = found(&world, den);
    // A body in transit above every support face cannot anchor a den search.
    place(&mut fauna, Site {
        y: world.config().height - 2,
        ..site(7, 6)
    });
    world.apply(WorldCommand::AddWater {
        x: i64::from(den.x),
        y: den.y + 1,
        z: den.z,
        volume_m3: world.config().voxel_volume(),
    });
    let flora = Flora::new(FloraConfig::default());
    let glowcap = flora.config().species(Species::Glowcap);
    let graph = fauna.pack_routes(&world.view(), C);
    assert!(graph.face(fauna.animals[0].site).is_none());
    for _ in 0..2 {
        crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
        assert!(Arc::ptr_eq(&graph, &fauna.pack_routes(&world.view(), C)));
        assert_eq!(fauna.pack(pack).unwrap().den, den);
        fauna.tick += 1;
    }
    let every = fauna.config.founder(C).pack.unwrap().route_refresh_s;
    fauna.tick = cubarium_voxel::ticks_at_least_one(every);
    crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
    assert!(!Arc::ptr_eq(&graph, &fauna.pack_routes(&world.view(), C)));
    // Returning to the graph restores immediate relocation, even within the cache's
    // refresh interval: the wet den is replaced by a dry reachable face.
    place(&mut fauna, site(7, 6));
    crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
    let next = fauna.pack(pack).unwrap().den;
    assert_ne!(next, den);
    assert_eq!(world.view().water_depth_m(i64::from(next.x), next.y, next.z), 0.0);
}

#[test]
fn a_loftstrider_is_not_gravid_at_introduction_and_has_one_calf_before_its_birth_wait() {
    let world = plain();
    let f = Founder::Loftstrider;
    let mut config = FaunaConfig::default();
    let core = &mut config.founders[f.index()].core;
    core.reserve_cap = 10.0;
    core.reproduction = Reproduction {
        surplus_floor: 0.0,
        surplus_hold_s: 0.1,
        surplus_share: 1.0,
        gestation_s: 0.15,
        birth_interval_s: 0.4,
        litters_per_year: 0,
        ..Reproduction::LIVE_BIRTH_PLACEHOLDER
    };
    let sc = *core;
    let rule = sc.reproduction;
    let mut fauna = Fauna::new(config);
    assert!(fauna.apply(
        &world,
        Command::IntroduceFounderOnFace {
            site: site(8, 6),
            founder: f,
            stores: StartingStores::FULL,
            heading_rad: 0.0
        }
    ));
    assert!(fauna.animals[0].reproduction.escrow.is_none());
    let mut newborns = Vec::new();
    let first_birth = rule.hold_ticks() + rule.gestation_ticks() - 1;
    for tick in 1..=first_birth {
        fauna.tick = tick;
        advance_eligibility(&mut fauna, 0, &sc, &rule);
        gestate(
            &mut fauna,
            0,
            &sc,
            &rule,
            (world.config().seed, None),
            &mut newborns,
        );
    }
    assert_eq!(
        newborns.len(),
        1,
        "one completed nonpack gestation gives one calf"
    );
    assert_eq!(newborns[0].founder, Some(f));
    assert_eq!(
        fauna.animals[0].reproduction.refractory_ticks,
        rule.interval_ticks()
    );
    for tick in 1..=rule.interval_ticks() {
        fauna.tick = first_birth + tick;
        advance_eligibility(&mut fauna, 0, &sc, &rule);
        gestate(
            &mut fauna,
            0,
            &sc,
            &rule,
            (world.config().seed, None),
            &mut newborns,
        );
        assert!(
            fauna.animals[0].reproduction.escrow.is_none(),
            "no conception during the configured postbirth wait"
        );
        assert_eq!(newborns.len(), 1);
    }
    for tick in 1..=first_birth {
        fauna.tick += 1;
        advance_eligibility(&mut fauna, 0, &sc, &rule);
        gestate(
            &mut fauna,
            0,
            &sc,
            &rule,
            (world.config().seed, None),
            &mut newborns,
        );
        if tick < first_birth {
            assert_eq!(
                newborns.len(),
                1,
                "the fresh surplus hold and gestation follow the wait"
            );
        }
    }
    assert_eq!(newborns.len(), 2);
}
