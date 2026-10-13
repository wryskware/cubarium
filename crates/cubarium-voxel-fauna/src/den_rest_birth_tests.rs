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

/// Standing water of `depth_m` on `s` (added to the empty cell above it).
fn flood(world: &mut World, s: Site, depth_m: f64) {
    let v = world.config().voxel_m;
    world.apply(WorldCommand::AddWater { x: i64::from(s.x), y: s.y + 1, z: s.z, volume_m3: depth_m * v * v });
}

/// The depth over which a den of the pack lineage counts as flooded, as the config says.
fn flood_limit(fauna: &Fauna) -> f64 {
    let phys = fauna.config.founder(C);
    phys.pack.unwrap().den_flood_wade_fraction * phys.wade_depth_m(&phys.adult_body())
}

#[test]
fn a_den_floods_only_above_the_configured_share_of_the_wade_depth() {
    let mut world = plain();
    roof(&mut world, 9..=17, 2..=9);
    let old = site(6, 6);
    let (mut fauna, pack) = found(&world, old);
    place(&mut fauna, site(7, 6));
    let limit = flood_limit(&fauna);
    assert!(limit > 0.0, "fixture: the default knob leaves a margin");
    let flora = Flora::new(FloraConfig::default());
    let glowcap = flora.config().species(Species::Glowcap);
    // Float dust, and a film under the knob: the den stays and is still walked home to.
    for depth in [1e-18, 0.5 * limit] {
        let now = world.view().water_depth_m(i64::from(old.x), old.y, old.z);
        flood(&mut world, old, depth - now);
        let d = world.view().water_depth_m(i64::from(old.x), old.y, old.z);
        assert!(d > 0.0 && d <= limit, "fixture: wet but under the knob ({d})");
        let (phys, t) = (*fauna.config.founder(C), fauna.config.founder(C).pack.unwrap());
        assert!(!crate::pack::flooded(&world.view(), &phys, &t, old));
        crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
        assert_eq!(fauna.pack(pack).unwrap().den, old);
        assert!(crate::pack::must_walk_home(&mut fauna, 0, &world.view()));
    }
    // Over the knob: flooded, left for a site the knob calls dry.
    flood(&mut world, old, limit);
    assert!(world.view().water_depth_m(i64::from(old.x), old.y, old.z) > limit);
    assert!(!crate::pack::must_walk_home(&mut fauna, 0, &world.view()));
    crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
    let den = fauna.pack(pack).unwrap().den;
    assert_ne!(den, old);
    assert!(world.view().water_depth_m(i64::from(den.x), den.y, den.z) <= limit);
    // The knob decides: raised over the water's depth, the same den is no flood.
    let deep = world.view().water_depth_m(i64::from(old.x), old.y, old.z);
    let wade = { let p = fauna.config.founder(C); p.wade_depth_m(&p.adult_body()) };
    fauna.config.founders[C.index()].pack.as_mut().unwrap().den_flood_wade_fraction = 2.0 * deep / wade;
    let (phys, t) = (*fauna.config.founder(C), fauna.config.founder(C).pack.unwrap());
    assert!(!crate::pack::flooded(&world.view(), &phys, &t, old));
}

#[test]
fn an_adult_against_a_wall_anchors_its_packs_den_search() {
    let mut world = plain();
    wall(&mut world, 12);
    let old = site(6, 6);
    let (mut fauna, pack) = found(&world, old);
    // The founding's clearance margin keeps the disc's graph off the face against the wall;
    // the body itself (its own footprint) stands there.
    let r = { let p = fauna.config.founder(C); p.adult_body().footprint_radius() };
    fauna.config.founding.clearance_margin_m = V;
    place(&mut fauna, site(11, 6));
    let graph = fauna.pack_routes(&world.view(), C);
    assert!(graph.face(site(11, 6)).is_none(), "fixture: the face against the wall is off the graph");
    assert!(graph.face(site(10, 6)).is_some() || graph.face(site(9, 6)).is_some(), "fixture: the graph runs beside it");
    assert!(r <= 0.5 * V, "fixture: the body fits its own face");
    flood(&mut world, old, V);
    // Rebuild: the flood changes what is standable.
    fauna.pack_routes.clear();
    let flora = Flora::new(FloraConfig::default());
    crate::pack::renew_dens(&mut fauna, &world.view(), flora.config().species(Species::Glowcap));
    let den = fauna.pack(pack).unwrap().den;
    assert_ne!(den, old, "the adult beside the graph anchors the search for a dry den");
    let (phys, t) = (*fauna.config.founder(C), fauna.config.founder(C).pack.unwrap());
    assert!(!crate::pack::flooded(&world.view(), &phys, &t, den));
}

#[test]
fn a_pack_on_the_roof_above_its_cave_den_walks_the_route_down() {
    let mut world = plain();
    // A slab roof over a cave, reached by a stair on its east side.
    roof(&mut world, 9..=17, 2..=9);
    for (x, top) in [(18i64, 7u32), (19, 6), (20, 5), (21, 4), (22, 3)] {
        for z in 2..=9 {
            for y in FLOOR + 1..=top {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: cubarium_voxel::Material::Soil });
            }
        }
    }
    let den = site(13, 6);
    let (mut fauna, _) = found(&world, den);
    let above = Site { y: 8, ..den };
    place(&mut fauna, above);
    let view = world.view();
    let graph = fauna.pack_routes(&view, C);
    let (from, to) = (graph.face(above).expect("fixture: the roof is walkable"), graph.face(den).expect("fixture: the cave floor is walkable"));
    assert_eq!(graph.components[from], graph.components[to], "fixture: the stair joins roof and cave");
    let xz = (fauna.animals[0].pose.x, fauna.animals[0].pose.z);
    assert!(graph.walk_line(from, xz, graph.centre(to)), "fixture: the column-only line test passes on the wrong layer");
    assert!(!graph.walk_line_to(from, xz, graph.centre(to), to));
    assert!(crate::pack::must_walk_home(&mut fauna, 0, &view));
    let a = fauna.animals[0];
    let phys = *fauna.config.founder(C);
    let geometry = phys.body_at(a.body);
    let h = phys.active_hunter().unwrap();
    let ((x, z), routed) = crate::pack::way_home(&mut fauna, &view, C, &a, &geometry, &h, den);
    assert!(routed);
    assert!((x - xz.0).hypot(z - xz.1) > 0.0, "it steers along its route, not at its own column");
}

// Birth fixtures exercise only the reproduction functions: no food, upkeep or movement.
fn nonpack_birth_fixture(young: Option<u32>) -> (Fauna, World, crate::SpeciesConfig) {
    let world = plain();
    let f = Founder::Browser;
    let mut config = FaunaConfig::default();
    let sc = &mut config.founders[f.index()].core;
    sc.reserve_cap = 10.0;
    sc.reproduction.surplus_hold_s = crate::DT;
    sc.reproduction.surplus_share = 1.0;
    sc.reproduction.gestation_s = 4.0 * crate::DT;
    if let Some(n) = young {
        sc.reproduction.young_per_birth = n;
    }
    let sc = *sc;
    let mut fauna = Fauna::new(config);
    assert!(fauna.apply(
        &world,
        Command::IntroduceFounderOnFace {
            site: site(8, 6),
            founder: f,
            stores: StartingStores::FULL,
            heading_rad: 0.0,
        }
    ));
    (fauna, world, sc)
}

fn nonpack_birth_conserves(young: Option<u32>, expected: usize) {
    let (mut fauna, world, sc) = nonpack_birth_fixture(young);
    let rule = sc.reproduction;
    let before = fauna.animals[0].clone();
    let mut newborns = Vec::new();
    for tick in 1..=rule.gestation_ticks() {
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
    assert_eq!(newborns.len(), expected);
    let after = &fauna.animals[0];
    let paid = before.reserve - after.reserve;
    assert!((paid - sc.birth_cost * expected as f64).abs() < 1e-14);
    let organic: f64 = newborns.iter().map(|a| a.body + a.reserve).sum();
    let mineral: f64 = newborns.iter().map(|a| a.mineral).sum();
    let energy: f64 = newborns.iter().map(|a| a.energy).sum();
    assert!((organic - paid).abs() < 1e-14);
    assert!((mineral - (before.mineral - after.mineral)).abs() < 1e-14);
    assert!((energy - (before.energy - after.energy)).abs() < 1e-14);
    assert!(after.reproduction.escrow.is_none());
}

#[test]
fn nonpack_twins_split_the_paid_parcel_without_loss() {
    nonpack_birth_conserves(Some(2), 2);
}

#[test]
fn default_nonpack_live_birth_still_has_one_newborn() {
    nonpack_birth_conserves(None, 1);
    let mut doc = serde_json::to_value(Reproduction::LIVE_BIRTH_PLACEHOLDER).unwrap();
    doc.as_object_mut().unwrap().remove("young_per_birth");
    let old: Reproduction = serde_json::from_value(doc).unwrap();
    nonpack_birth_conserves(Some(old.young_per_birth), 1);
}

#[test]
fn pack_litter_rule_ignores_young_per_birth() {
    let world = plain();
    let (mut fauna, pack) = found(&world, site(8, 6));
    let id = fauna.animals[0].id;
    fauna.pack_mut(pack).unwrap().breeder = Some(id);
    let sc = fauna.config.founder(C).core;
    let mut rule = sc.reproduction;
    let mut counts = Vec::new();
    for knob in [1, 2, u32::MAX] {
        rule.young_per_birth = knob;
        let super::Litter::Of(n) = super::litter_size(&fauna, 0, &sc, &rule, world.config().seed)
        else {
            panic!("fixture breeder must qualify for a pack litter");
        };
        counts.push(n);
    }
    assert!(counts[0] > 0);
    assert!(counts.iter().all(|&n| n == counts[0]));
}

#[test]
fn an_unpaid_twin_instalment_returns_escrow_and_books_the_loss() {
    let (mut fauna, world, sc) = nonpack_birth_fixture(Some(2));
    let rule = sc.reproduction;
    let mut newborns = Vec::new();
    advance_eligibility(&mut fauna, 0, &sc, &rule);
    gestate(
        &mut fauna,
        0,
        &sc,
        &rule,
        (world.config().seed, None),
        &mut newborns,
    );
    let escrow = fauna.animals[0].reproduction.escrow.unwrap();
    assert_eq!(escrow.young, 2);
    assert!(escrow.organic > 0.0);
    fauna.animals[0].reserve = 0.0;
    let before = fauna.animals[0].clone();
    let heat_before = fauna.ledger.heat_out;
    let respired_before = fauna.ledger.respired_gestation_out;
    gestate(
        &mut fauna,
        0,
        &sc,
        &rule,
        (world.config().seed, None),
        &mut newborns,
    );
    let after = &fauna.animals[0];
    let lost = rule.gestation_loss_fraction;
    assert!(newborns.is_empty());
    assert!(after.reproduction.escrow.is_none());
    assert_eq!(fauna.ledger.gestations_failed, 1);
    assert!((after.reserve - escrow.organic * (1.0 - lost)).abs() < 1e-14);
    assert!(
        (after.mineral + after.gut.mineral - before.mineral - before.gut.mineral - escrow.mineral)
            .abs()
            < 1e-14
    );
    assert!(
        (after.energy - before.energy + fauna.ledger.heat_out - heat_before - escrow.energy).abs()
            < 1e-14
    );
    assert!(
        (fauna.ledger.respired_gestation_out - respired_before - escrow.organic * lost).abs()
            < 1e-14
    );
}

// Materialize a constant-surplus history at a boundary, rather than simulate a year.
fn surplus_history(ticks: u64, hold: u64) -> crate::SurplusWindow {
    let bucket = hold.div_ceil(crate::SURPLUS_BUCKETS as u64).max(1);
    let full = ticks / bucket;
    let mut w = crate::SurplusWindow::default();
    for k in 0..full as usize {
        w.buckets[k] = bucket as u32;
    }
    w.filled = full as u32;
    w.pos = full as u32;
    w.cur_ticks = (ticks % bucket) as u32;
    w.cur_eligible = w.cur_ticks;
    assert_eq!(w.count_for(hold), ticks);
    w
}

#[test]
fn seedporter_birth_spacing_includes_the_wait_hold_and_gestation() {
    let (mut fauna, world, _) = nonpack_birth_fixture(None);
    let sc = FaunaConfig::default().founder(Founder::Seedporter).core;
    fauna.animals[0].founder = Some(Founder::Seedporter);
    fauna.animals[0].body = sc.body_max;
    fauna.animals[0].reserve = 10.0 * sc.birth_cost;
    let rule = sc.reproduction;
    let mut newborns = Vec::new();
    let mut births = Vec::new();
    for birth in 0..2 {
        if birth > 0 {
            assert_eq!(
                fauna.animals[0].reproduction.refractory_ticks,
                rule.interval_ticks()
            );
            // Jump to the last refractory tick and verify it still clears surplus.
            fauna.tick += rule.interval_ticks();
            fauna.animals[0].reproduction.refractory_ticks = 1;
            advance_eligibility(&mut fauna, 0, &sc, &rule);
            gestate(
                &mut fauna,
                0,
                &sc,
                &rule,
                (world.config().seed, None),
                &mut newborns,
            );
            assert!(fauna.animals[0].reproduction.escrow.is_none());
            assert_eq!(fauna.animals[0].reproduction.surplus_ticks, 0);
        }
        fauna.tick += rule.hold_required() - 1;
        fauna.animals[0].reproduction.window =
            surplus_history(rule.hold_required() - 1, rule.hold_ticks());
        fauna.animals[0].reproduction.surplus_ticks = rule.hold_required() - 1;
        fauna.animals[0].reproduction.eligible_now = true;
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
            "hold not yet met"
        );
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
        assert!(fauna.animals[0].reproduction.escrow.is_some());
        // The cumulative target lets the final tick pay the skipped instalments.
        fauna.tick += rule.gestation_ticks() - 2;
        fauna.animals[0].reproduction.escrow.as_mut().unwrap().ticks = rule.gestation_ticks() - 1;
        fauna.tick += 1;
        gestate(
            &mut fauna,
            0,
            &sc,
            &rule,
            (world.config().seed, None),
            &mut newborns,
        );
        assert_eq!(
            newborns.len(),
            (birth + 1) * rule.young_per_birth.max(1) as usize
        );
        births.push(fauna.tick);
    }
    // Conception pays instalment 1 on the hold's last tick: the spans share that tick.
    assert_eq!(
        births[1] - births[0] + 1,
        rule.interval_ticks() + rule.hold_required() + rule.gestation_ticks()
    );
}

/// DEN-REACH fixture: a ring split by two full walls into a west range (x 31..=11, roofed
/// over x 3..=9) and an east range (x 13..=29, roofed over x 16..=24), a pack of `size`
/// grown members founded on a den in the east roof. Returns the world, the pack, its den,
/// and the breeder's index.
fn split_pack(size: usize) -> (World, Fauna, u64, Site, usize) {
    let mut world = plain();
    roof(&mut world, 16..=24, 2..=9);
    roof(&mut world, 3..=9, 2..=9);
    wall(&mut world, 12);
    wall(&mut world, 30);
    let den = site(20, 6);
    let mut fauna = Fauna::new(pack_config());
    fauna.set_births_enabled(false);
    let pack = fauna
        .found_pack_sized(&world, den, C, StartingStores::FULL, FRAC_PI_2, size)
        .expect("a pack");
    let breeder = fauna.pack(pack).unwrap().breeder.expect("a breeder");
    let b = fauna.animals.iter().position(|a| a.id == breeder).unwrap();
    (world, fauna, pack, den, b)
}

fn place_at(fauna: &mut Fauna, i: usize, s: Site) {
    let a = &mut fauna.animals[i];
    a.site = s;
    (a.pose.x, a.pose.z) = crate::pack::centre(s, V);
    a.pose.y_m = crate::surface_m(s.y, V);
}

fn component(fauna: &mut Fauna, world: &World, s: Site) -> usize {
    let graph = fauna.pack_routes(&world.view(), C);
    graph.components[graph.face(s).expect("on the graph")]
}

#[test]
fn a_den_only_the_breeder_walks_to_moves_to_where_the_rest_of_the_pack_stands() {
    let (world, mut fauna, pack, den, b) = split_pack(3);
    let others: Vec<usize> = (0..fauna.animals.len()).filter(|&i| i != b).collect();
    place_at(&mut fauna, b, site(20, 6));
    for (k, &i) in others.iter().enumerate() {
        place_at(&mut fauna, i, site(5 + 2 * k as u32, 6));
    }
    let flora = Flora::new(FloraConfig::default());
    let glowcap = flora.config().species(Species::Glowcap);
    // The rule before DEN-REACH (the breeder's reach alone) keeps the pocket den.
    fauna.config.founders[C.index()].pack.as_mut().unwrap().den_reach_share = 0.0;
    crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
    assert_eq!(fauna.pack(pack).unwrap().den, den);
    // Judged by every member on the graph, it moves to the range the others stand on.
    fauna.config.founders[C.index()].pack.as_mut().unwrap().den_reach_share = 1.0;
    crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
    let new_den = fauna.pack(pack).unwrap().den;
    assert_ne!(new_den, den);
    let s0 = fauna.animals[others[0]].site;
    let west = component(&mut fauna, &world, s0);
    assert_eq!(component(&mut fauna, &world, new_den), west);
    for &i in &others {
        let s = fauna.animals[i].site;
        assert_eq!(component(&mut fauna, &world, s), west);
    }
}

#[test]
fn a_den_every_member_walks_to_is_kept_and_an_even_split_keeps_its_den() {
    let (world, mut fauna, pack, den, b) = split_pack(2);
    let flora = Flora::new(FloraConfig::default());
    let glowcap = flora.config().species(Species::Glowcap);
    fauna.config.founders[C.index()].pack.as_mut().unwrap().den_reach_share = 1.0;
    // Both on the den's range: kept, though the other range has a roof too.
    place_at(&mut fauna, 0, site(18, 6));
    place_at(&mut fauna, 1, site(22, 6));
    crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
    assert_eq!(fauna.pack(pack).unwrap().den, den);
    // One on each range: no range holds more of the pack than the den's, so it stays.
    let other = 1 - b;
    place_at(&mut fauna, other, site(6, 6));
    crate::pack::renew_dens(&mut fauna, &world.view(), glowcap);
    assert_eq!(fauna.pack(pack).unwrap().den, den);
}
