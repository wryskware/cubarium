//! Hand-built Places mechanisms: no ecological run or default tuning assertions.
#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use super::*;
use crate::{FaunaConfig, Founder, Perch, PerchKind, StartingStores};
use cubarium_voxel::{Command as WCommand, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig};
use fixture::*;

fn sky() -> Sky {
    Sky {
        clock_on: false,
        daylight: 1.0,
        downpour: false,
        rain_m_per_s: 0.0,
    }
}

fn config() -> FaunaConfig {
    let mut c = hunting_config();
    c.perception.remote_hz = 4.0;
    c.perception.place_memory_fade_s = 8.0 * crate::DT;
    for p in &mut c.founders {
        p.senses.sight_m = 4.0;
        p.senses.sight_night_m = 4.0;
        p.senses.sight_still_fraction = 1.0;
        p.senses.sight_half_angle_deg = 180.0;
        p.senses.still_gain = 1.0;
        p.senses.feeding_sight_fraction = 1.0;
    }
    c
}

fn body(f: &mut Fauna, w: &World, founder: Founder) -> u64 {
    let id = introduce(f, w, founder, StartingStores::FULL);
    place(f, id, 2.125, 1.125, 0.0);
    id
}

fn water(w: &mut World, site: Site) {
    w.apply(WCommand::AddWater {
        x: i64::from(site.x),
        y: site.y + 1,
        z: site.z,
        volume_m3: w.config().voxel_volume(),
    });
}

fn sense(f: &mut Fauna, w: &World, flora: &Flora, id: u64) {
    let n = f.config.perception.stagger_ticks();
    f.tick += (id % n + n - f.tick % n) % n;
    refresh(f, &w.view(), &flora.view(), &sky());
}

fn block(f: &Fauna, w: &World, id: u64, kind: &str) -> [f64; 5] {
    let a = animal(f, id);
    let frame = crate::observe_v3::Frame::of(&a, f64::from(w.config().width) * VOXEL_M, f.tick);
    encode(f, &a, kind, &frame, VOXEL_M)
}

#[test]
fn water_bearing_is_the_reflex_target_and_queries_require_a_reader() {
    let mut w = clock_off(11);
    let site = Site {
        x: 12,
        y: SURFACE_Y,
        z: 4,
    };
    water(&mut w, site);
    let mut f = Fauna::new(config());
    let id = body(&mut f, &w, Founder::RippleSnail);
    let flora = Flora::new(FloraConfig::default());
    sense(&mut f, &w, &flora, id);
    assert!(f.place_cache.is_empty(), "heuristics do not query the tail");
    f.start_tap();
    sense(&mut f, &w, &flora, id);
    let b = block(&f, &w, id, "water");
    assert_eq!(b[0], 1.0);
    assert!(
        (b[1] - 1.0).abs() < 1e-12 && b[2].abs() < 1e-12,
        "right: {b:?}"
    );
    let i = f.animals.iter().position(|a| a.id == id).unwrap();
    let am = f.config.founder(Founder::RippleSnail).amphibious.unwrap();
    assert_eq!(
        crate::step::water_target(&f, i, &w.view(), am.water_search_m, |_| true),
        Some(site)
    );
    let go = crate::step::walk_to_place(&f.animals[i], &w.view(), site, 0.0);
    assert!(go.turn > 0.0, "the reflex walks right too");
    let range = f.place_cache[&id].range_m;
    assert!((b[3] - (1.0 - VOXEL_M * 4.0 / range)).abs() < 1e-12);
    assert!(f.walk_pending.is_empty(), "observing sets no walking goal");
    assert!(
        f.pack_routes.is_empty(),
        "observing moves no heuristic cache clock"
    );
}

#[test]
fn water_is_empty_past_range_out_of_field_or_behind_terrain() {
    let mut w = clock_off(12);
    let site = Site {
        x: 12,
        y: SURFACE_Y,
        z: 4,
    };
    water(&mut w, site);
    let mut f = Fauna::new(config());
    let id = body(&mut f, &w, Founder::RippleSnail);
    let flora = Flora::new(FloraConfig::default());
    f.start_tap();
    // A deep pond still has a visible surface, even though its bed is hidden.
    w.apply(WCommand::AddWater {
        x: i64::from(site.x),
        y: site.y + 2,
        z: site.z,
        volume_m3: w.config().voxel_volume(),
    });
    sense(&mut f, &w, &flora, id);
    assert_eq!(block(&f, &w, id, "water")[0], 1.0, "it sees the surface");
    let p = &mut f.config.founders[Founder::RippleSnail.index()];
    p.senses.sight_m = 0.5;
    sense(&mut f, &w, &flora, id);
    assert_eq!(block(&f, &w, id, "water"), [0.0; 5]);
    f.config.founders[Founder::RippleSnail.index()]
        .senses
        .sight_m = 4.0;
    f.config.founders[Founder::RippleSnail.index()]
        .senses
        .sight_half_angle_deg = 30.0;
    f.tick += f.config.perception.stagger_ticks();
    sense(&mut f, &w, &flora, id);
    assert_eq!(block(&f, &w, id, "water"), [0.0; 5]);
    f.config.founders[Founder::RippleSnail.index()]
        .senses
        .sight_half_angle_deg = 180.0;
    w.apply(WCommand::SetMaterial {
        x: 10,
        y: SURFACE_Y + 1,
        z: 4,
        material: Material::Rock,
    });
    f.tick += f.config.perception.stagger_ticks();
    sense(&mut f, &w, &flora, id);
    assert_eq!(block(&f, &w, id, "water"), [0.0; 5]);
}

#[test]
fn sensed_places_refresh_on_remote_ticks_and_encode_the_current_heading() {
    let mut w = clock_off(13);
    let site = Site {
        x: 12,
        y: SURFACE_Y,
        z: 4,
    };
    water(&mut w, site);
    let mut f = Fauna::new(config());
    let id = body(&mut f, &w, Founder::RippleSnail);
    let flora = Flora::new(FloraConfig::default());
    f.start_tap();
    sense(&mut f, &w, &flora, id);
    let at = f.place_cache[&id].tick;
    let period = f.config.perception.stagger_ticks();
    assert!(period > 1);
    w = clock_off(13); // The source disappears between remote samples.
    animal_mut(&mut f, id).pose.heading_rad = EAST;
    for _ in 1..period {
        f.tick += 1;
        refresh(&mut f, &w.view(), &flora.view(), &sky());
        assert_eq!(f.place_cache[&id].tick, at);
        let b = block(&f, &w, id, "water");
        assert_eq!(b[0], 1.0, "the old sample is held");
        assert!(
            b[1].abs() < 1e-12 && (b[2] - 1.0).abs() < 1e-12,
            "now ahead: {b:?}"
        );
    }
    f.tick += 1;
    refresh(&mut f, &w.view(), &flora.view(), &sky());
    assert_eq!(f.place_cache[&id].tick, at + period);
    assert_eq!(block(&f, &w, id, "water"), [0.0; 5]);
}

#[test]
fn a_brain_queries_places_with_the_tap_off() {
    struct Brain;
    impl crate::Controller for Brain {
        fn drive(&mut self, _: &[f64]) -> crate::Response {
            crate::Response::Bounded(crate::Actions {
                forward: 0.0,
                turn: 0.0,
                feed: 0.0,
            })
        }
        fn reset(&mut self) {}
        fn reads_tail(&self) -> bool {
            true
        }
        fn layer(&self) -> crate::Layer {
            crate::Layer::Brain
        }
    }
    let mut w = clock_off(14);
    water(
        &mut w,
        Site {
            x: 12,
            y: SURFACE_Y,
            z: 4,
        },
    );
    let mut f = Fauna::new(config());
    let id = body(&mut f, &w, Founder::RippleSnail);
    assert!(f.set_controller(id, Box::new(Brain)));
    sense(&mut f, &w, &Flora::new(FloraConfig::default()), id);
    assert_eq!(block(&f, &w, id, "water")[0], 1.0);
}

#[test]
fn physical_home_visits_refresh_memory_without_a_tap_and_recall_fades() {
    let mut w = clock_off(15);
    let mut f = Fauna::new(config());
    let roost = body(&mut f, &w, Founder::Seedporter);
    let wet = body(&mut f, &w, Founder::RippleSnail);
    let nest = body(&mut f, &w, Founder::Bellwing);
    let site = animal(&f, wet).site;
    water(&mut w, site);
    animal_mut(&mut f, roost).mobility.mode = MobilityMode::Perched;
    animal_mut(&mut f, roost).mobility.perch = Some(Perch {
        kind: PerchKind::CrownTop,
        anchor: PerchAnchor::Stand { site, layer: 0 },
    });
    animal_mut(&mut f, nest).mobility.mode = MobilityMode::Perched;
    animal_mut(&mut f, nest).mobility.perch = Some(Perch {
        kind: PerchKind::VineFace,
        anchor: PerchAnchor::Vine(cubarium_voxel_flora::Face::new(
            site.x,
            site.y + 1,
            site.z,
            cubarium_voxel_flora::FaceDir::NegX,
        )),
    });
    let flora = Flora::new(FloraConfig::default());
    refresh(&mut f, &w.view(), &flora.view(), &sky());
    assert!(f.place_cache.is_empty());
    for (id, kind) in [(roost, "roost"), (wet, "water"), (nest, "nest")] {
        assert_eq!(block(&f, &w, id, kind)[4], 1.0, "used {kind}");
        animal_mut(&mut f, id).mobility.mode = MobilityMode::Ground;
        place(&mut f, id, 7.0, 1.125, 0.0);
    }
    f.tick += 2;
    refresh(&mut f, &w.view(), &flora.view(), &sky());
    for (id, kind) in [(roost, "roost"), (wet, "water"), (nest, "nest")] {
        let b = block(&f, &w, id, kind);
        assert_eq!(b[0], 1.0, "remembered beyond sight");
        assert!(b[4] > 0.0 && b[4] < 1.0);
    }
    place(&mut f, wet, 2.125, 1.125, 0.0);
    refresh(&mut f, &w.view(), &flora.view(), &sky());
    assert_eq!(block(&f, &w, wet, "water")[4], 1.0, "a visit restores it");
    place(&mut f, wet, 7.0, 1.125, 0.0);
    f.tick += cubarium_voxel::ticks_at_least_one(f.config.perception.place_memory_fade_s);
    refresh(&mut f, &w.view(), &flora.view(), &sky());
    for (id, kind) in [(roost, "roost"), (wet, "water"), (nest, "nest")] {
        assert_eq!(block(&f, &w, id, kind), [0.0; 5], "{kind} forgotten");
    }
}

#[test]
fn a_save_keeps_used_places_and_drops_the_derived_cache() {
    let mut w = clock_off(16);
    let mut f = Fauna::new(config());
    let id = body(&mut f, &w, Founder::RippleSnail);
    water(&mut w, animal(&f, id).site);
    f.start_tap();
    sense(&mut f, &w, &Flora::new(FloraConfig::default()), id);
    assert!(!f.place_cache.is_empty());
    let bytes = crate::snapshot::encode(&f);
    let loaded = crate::snapshot::decode(&bytes).unwrap();
    assert_eq!(loaded.place_memory, f.place_memory);
    assert!(loaded.place_cache.is_empty());
}

#[test]
fn roost_and_fruit_use_the_reflexes_climbable_tree_queries() {
    let w = plain_of_height(
        cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
        17,
        32,
    );
    let mut cfg = config();
    let g = cfg.founders[Founder::Seedporter.index()]
        .glider
        .as_mut()
        .unwrap();
    g.roost_reserve_share = 1.0;
    g.roost_budget_m = 4.0;
    let safe = crate::step::refuge_height_m(&cfg);
    let climb = g_climb(&cfg);
    let mut fc = FloraConfig::default();
    fc.vaulttree.crown_height_m = [2.0 * safe.max(climb), 2.0 * safe.max(climb)];
    let wood = fc.vaulttree.wood_max;
    let mut flora = Flora::new(fc);
    assert!(flora.apply(
        &w,
        cubarium_voxel_flora::Command::Seed {
            x: 12,
            z: 4,
            species: cubarium_voxel_flora::Species::Vaulttree,
            wood
        }
    ));
    let site = flora.view().stands[0].site;
    let mut f = Fauna::new(cfg);
    let id = body(&mut f, &w, Founder::Seedporter);
    let i = f.animals.iter().position(|a| a.id == id).unwrap();
    assert!(
        matches!(crate::step::better_roost(&mut f, i, &w.view(), &flora.view()), Some(glider::Roost::Crown(_, s)) if s == site)
    );
    f.start_tap();
    sense(&mut f, &w, &flora, id);
    assert_eq!(
        f.place_cache[&id].sensed[4],
        Some(Point::site(&w.view(), site))
    );
    assert_eq!(block(&f, &w, id, "roost")[0], 1.0);
    // Fruit uses the same visible candidates and read-only route selection as its reflex.
    // Put a full ripe stand in view; no maturation run is needed.
    // Flora owns stocks: use its serialized state to construct the small fixture.
    let mut doc = serde_json::to_value(&flora).unwrap();
    doc["stands"][0]["fruit"] = serde_json::json!(1.0);
    flora = serde_json::from_value(doc).unwrap();
    f.config.founders[Founder::Seedporter.index()]
        .senses
        .sight_m = 8.0;
    f.tick += f.config.perception.stagger_ticks();
    sense(&mut f, &w, &flora, id);
    let seen =
        glider::fruit_candidates(&f, i, &w.view(), &flora.view(), f.place_cache[&id].range_m)
            .unwrap();
    let cands: Vec<_> = seen.iter().map(|s| (s.0, s.1, s.2)).collect();
    let (_, k) = glider::walk_target(&mut f, i, &w.view(), &flora.view(), &cands, 8.0, 1.0)
        .expect("fruit is reachable");
    assert_eq!(
        f.place_cache[&id].sensed[5],
        Some(Point::site(&w.view(), cands[k].2))
    );
}

fn g_climb(c: &FaunaConfig) -> f64 {
    c.founder(Founder::Seedporter)
        .glider
        .unwrap()
        .climb_min_stand_m
}

#[test]
fn a_vine_nest_bearing_points_where_the_nest_reflex_flies() {
    use cubarium_voxel_flora::{Face, FaceDir, VineSeed};
    let mut w = plain_of_height(
        cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
        18,
        24,
    );
    for z in 0..w.config().depth {
        for y in 1..=12 {
            w.apply(WCommand::SetMaterial {
                x: 12,
                y,
                z,
                material: Material::Rock,
            });
        }
    }
    let mut flora = Flora::new(FloraConfig::default());
    let first = Face::new(12, 3, 4, FaceDir::NegX);
    let vine = flora
        .seed_vine(
            &w,
            VineSeed {
                root: Site { x: 11, y: 2, z: 4 },
                first,
                reserve: 100.0,
                lineage: None,
            },
        )
        .unwrap();
    for y in 4..=10 {
        assert!(flora.cover_face(&w, vine, Face::new(12, y, 4, FaceDir::NegX), 1.0));
    }
    let mut cfg = config();
    cfg.founders[Founder::Bellwing.index()]
        .core
        .reproduction
        .surplus_hold_s = 0.0;
    cfg.founders[Founder::Bellwing.index()]
        .core
        .reproduction
        .clutch_hold_max_s = 0.0;
    cfg.founders[Founder::Bellwing.index()]
        .core
        .reproduction
        .nest_radius_m = 0.0;
    cfg.founders[Founder::Bellwing.index()].senses.sight_m = 0.0;
    cfg.founders[Founder::Bellwing.index()].senses.sight_night_m = 0.0;
    let mut f = Fauna::new(cfg);
    assert!(f.apply(
        &w,
        crate::Command::IntroduceAloft {
            pose: crate::Pose {
                x: 2.0,
                z: 1.125,
                y_m: 1.55,
                heading_rad: 0.0
            },
            founder: Founder::Bellwing,
            stores: StartingStores::FULL
        }
    ));
    let id = f.animals[0].id;
    f.animals[0].reproduction.eligible_now = true;
    f.animals[0].reproduction.surplus_ticks = f
        .config
        .founder(Founder::Bellwing)
        .core
        .reproduction
        .hold_required();
    f.start_tap();
    sense(&mut f, &w, &flora, id);
    let flight = f.config.founder(Founder::Bellwing).flight.unwrap();
    let t = crate::step::nest_target(
        &mut f,
        0,
        &w.view(),
        &flora.view(),
        flight.nest_search_m,
        |_| true,
    )
    .expect("a fitting vine nest");
    let (go, _) =
        crate::step::nest_reflex(&mut f, 0, &w.view(), &flora).expect("flies to the nest");
    let b = block(&f, &w, id, "nest");
    let frame =
        crate::observe_v3::Frame::of(&f.animals[0], f64::from(w.config().width) * VOXEL_M, f.tick);
    let (sin, cos, _) = frame.toward(t.x, t.z);
    assert_eq!(b[0], 1.0);
    assert!((b[1] - sin).abs() < 1e-12 && (b[2] - cos).abs() < 1e-12);
    assert!(b[1] > 0.0 && go.turn > 0.0, "both point at the vine face");
    f.config.founders[Founder::Bellwing.index()].senses.light_m = 0.2;
    f.tick += f.config.perception.stagger_ticks();
    sense(&mut f, &w, &flora, id);
    assert_eq!(block(&f, &w, id, "nest"), [0.0; 5]);
}

#[test]
fn shore_uses_the_shallows_reflex_face_without_changing_its_cache_clock() {
    let mut w = plain_of_height(
        cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
        19,
        24,
    );
    let mut f = Fauna::new(config());
    let id = body(&mut f, &w, Founder::Loftstrider);
    f.config.founders[Founder::Loftstrider.index()]
        .senses
        .sight_m = 20.0;
    f.config.founders[Founder::Loftstrider.index()]
        .senses
        .sight_night_m = 20.0;
    // A thin film beyond this fixture body's wade, with its surface below the
    // exposed bank point. The shoreline is actually visible above that water.
    f.config.founders[Founder::Loftstrider.index()].wade_height_fraction = 0.0;
    let rows = 1;
    for z in 0..w.config().depth {
        for x in 4..=12 {
            for y in 3..3 + rows {
                w.apply(WCommand::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: 0.1 * w.config().voxel_volume(),
                });
            }
        }
    }
    f.tick = 7;
    let reach = f.config.life.shallows_search_m;
    let face =
        crate::step::shallows_target(&mut f, 0, &w.view(), reach, |_| true).expect("a way out");
    let before: Vec<_> = f
        .pack_routes
        .iter()
        .map(|(&k, (t, g))| (k, *t, std::sync::Arc::clone(g)))
        .collect();
    let flora = Flora::new(FloraConfig::default());
    f.start_tap();
    sense(&mut f, &w, &flora, id);
    for (k, tick, graph) in before {
        assert_eq!(f.pack_routes[&k].0, tick);
        assert!(std::sync::Arc::ptr_eq(&f.pack_routes[&k].1, &graph));
    }
    let b = block(&f, &w, id, "shore");
    let frame =
        crate::observe_v3::Frame::of(&f.animals[0], f64::from(w.config().width) * VOXEL_M, f.tick);
    let p = Point::site(&w.view(), face);
    let (sin, cos, _) = frame.toward(p.x, p.z);
    assert_eq!(b[0], 1.0);
    assert!(
        (b[1] - sin).abs() < 1e-12 && (b[2] - cos).abs() < 1e-12,
        "same face: {b:?}"
    );
    f.config.founders[Founder::Loftstrider.index()]
        .senses
        .sight_m = 0.1;
    f.tick += f.config.perception.stagger_ticks();
    sense(&mut f, &w, &flora, id);
    assert_eq!(block(&f, &w, id, "shore"), [0.0; 5]);
}

#[test]
fn an_eyeless_shore_is_contact_local_and_cannot_borrow_tremor_range() {
    let mut w = clock_off(20);
    let mut cfg = config();
    cfg.founders[Founder::Blind.index()].senses.sight_m = 0.0;
    cfg.founders[Founder::Blind.index()].senses.sight_night_m = 0.0;
    let mut f = Fauna::new(cfg);
    let id = body(&mut f, &w, Founder::Blind);
    let flora = Flora::new(FloraConfig::default());
    f.start_tap();
    sense(&mut f, &w, &flora, id);
    assert_eq!(
        block(&f, &w, id, "shore")[0],
        1.0,
        "it feels its own support"
    );
    let r = f
        .config
        .founder(Founder::Blind)
        .body_at(f.animals[0].body)
        .footprint_radius();
    assert!(f.place_cache[&id].range_m <= r + std::f64::consts::SQRT_2 * 0.5 * VOXEL_M);
    f.config.founders[Founder::Blind.index()].wade_height_fraction = 0.0;
    water(&mut w, f.animals[0].site);
    f.tick += f.config.perception.stagger_ticks();
    sense(&mut f, &w, &flora, id);
    assert_eq!(
        block(&f, &w, id, "shore"),
        [0.0; 5],
        "dry faces beyond its contact are unknown"
    );
}

#[test]
fn the_current_den_is_always_known_and_a_used_former_den_fades() {
    let w = clock_off(21);
    let mut f = Fauna::new(config());
    let den = Site {
        x: 8,
        y: SURFACE_Y,
        z: 4,
    };
    f.found_pack_sized(&w, den, Founder::Chorister, StartingStores::FULL, 0.0, 1)
        .expect("a pack at its den");
    let id = f.animals[0].id;
    let member = f.animals[0].pack;
    let flora = Flora::new(FloraConfig::default());
    refresh(&mut f, &w.view(), &flora.view(), &sky());
    assert!(
        f.place_memory[&id].get("den").is_some(),
        "a physical den visit is learned"
    );
    place(&mut f, id, 7.0, 1.125, 0.0);
    f.tick += 2;
    let b = block(&f, &w, id, "den");
    assert_eq!(b[0], 1.0, "known beyond sight and without a remote cache");
    assert_eq!(b[4], 1.0, "the current home remains known");
    animal_mut(&mut f, id).pack = None;
    let remembered = block(&f, &w, id, "den");
    assert_eq!(remembered[0], 1.0);
    assert!(
        remembered[4] > 0.0 && remembered[4] < 1.0,
        "the former den fades"
    );
    animal_mut(&mut f, id).pack = member;
    place(&mut f, id, 2.125, 1.125, 0.0);
    refresh(&mut f, &w.view(), &flora.view(), &sky());
    assert_eq!(
        f.place_memory[&id].get("den").unwrap().visited,
        f.tick,
        "returning refreshes it"
    );
    animal_mut(&mut f, id).pack = None;
    place(&mut f, id, 7.0, 1.125, 0.0);
    f.tick += cubarium_voxel::ticks_at_least_one(f.config.perception.place_memory_fade_s);
    assert_eq!(block(&f, &w, id, "den"), [0.0; 5]);
}

fn shore_search_fixture(seed: u64) -> (World, Fauna) {
    let w = clock_off(seed);
    let mut cfg = config();
    let phys = &mut cfg.founders[Founder::Loftstrider.index()];
    phys.adult_length_m = VOXEL_M;
    phys.adult_width_m = 0.5 * VOXEL_M;
    phys.adult_height_m = 0.5 * VOXEL_M;
    phys.step_up_m = 0.0;
    phys.step_down_m = 0.0;
    cfg.founding.clearance_margin_m = 0.0;
    let mut f = Fauna::new(cfg);
    body(&mut f, &w, Founder::Loftstrider);
    (w, f)
}

#[test]
fn a_seeded_shore_search_preserves_ties_when_the_square_wraps_repeatedly() {
    let (w, mut f) = shore_search_fixture(23);
    let id = f.animals[0].id;
    place(&mut f, id, 0.5 * VOXEL_M, 4.5 * VOXEL_M, 0.0);
    let first = Site { x: 1, y: SURFACE_Y, z: 4 };
    let last = Site { x: w.config().width - 1, ..first };
    let reach = f64::from(w.config().width) * VOXEL_M;
    // Both are equally near. At k=width the original -k..=k scan meets x=1
    // before x=width-1, although the local seed meets the latter first.
    assert_eq!(
        crate::step::shallows_target(&mut f, 0, &w.view(), reach, |s| s == first || s == last),
        Some(first)
    );
}

#[test]
fn an_unseen_local_shore_does_not_truncate_the_remaining_search() {
    let (w, mut f) = shore_search_fixture(24);
    let a = f.animals[0];
    let farther = Site { x: a.site.x + 4, ..a.site };
    assert_eq!(
        crate::step::shallows_target(&mut f, 0, &w.view(), 5.0 * VOXEL_M, |s| s == farther),
        Some(farther)
    );
}

#[test]
fn a_bounded_shore_rescan_stays_inside_the_original_site_square() {
    let (w, mut f) = shore_search_fixture(25);
    let site = f.animals[0].site;
    // Deliberately separate pose and site to exercise both bounds. A candidate
    // near the pose can still lie outside the original square about the site.
    f.animals[0].pose.x += 4.0 * VOXEL_M;
    let outside = Site { x: site.x + 5, ..site };
    assert_eq!(
        crate::step::shallows_target(&mut f, 0, &w.view(), 4.0 * VOXEL_M, |s| s == site || s == outside),
        Some(site)
    );
}
