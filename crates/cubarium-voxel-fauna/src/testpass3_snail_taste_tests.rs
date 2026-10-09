//! Independent SNAIL-LAND, SNAIL-LAND-2 and TASTE contracts:
//! Shore eggs require land moisture and nearby water within horizontal/climb limits.
//! Consecutive wet/dry exposure kills only past its configured duration, resets when safe,
//! and returns the whole clutch to litter. Film-site eggs do not acquire shore risks.
//! Giving-up uses edible film, searches only its configured radius, respects roam hours,
//! crowding and worth, and is disabled at zero. Moving between patches conserves stores.
//! Food perception offers only what a mouth can remove, including shore film and caps.

use super::{Sky, graze_reflex, graze_target, shore_eggs};
use crate::testpass3_support::{V, close, introduce, ledgers, plain};
use crate::{Activity, Clutch, EggSite, Fauna, FaunaConfig, Founder};
use cubarium_voxel::{Command as WC, DT, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const SN: Founder = Founder::RippleSnail;

fn pore(w: &mut World, s: Site, fill: f64) {
    let volume =
        fill * w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
    w.apply(WC::WithdrawPore {
        x: i64::from(s.x),
        y: s.y,
        z: s.z,
        volume_m3: 1e6,
    });
    w.apply(WC::SetMaterial {
        x: i64::from(s.x),
        y: s.y,
        z: s.z,
        material: Material::Air,
    });
    w.apply(WC::AddWater {
        x: i64::from(s.x),
        y: s.y,
        z: s.z,
        volume_m3: volume,
    });
    w.apply(WC::SetMaterial {
        x: i64::from(s.x),
        y: s.y,
        z: s.z,
        material: Material::Soil,
    });
}

fn site(x: u32) -> Site {
    Site {
        x,
        y: if x < 6 {
            2
        } else if x == 7 {
            5
        } else {
            3
        },
        z: 2,
    }
}

fn shore() -> World {
    let mut w = plain(16, 6, 12);
    for z in 0..6 {
        for x in 6..16 {
            let top = if x == 7 { 5 } else { 3 };
            for y in 3..=top {
                w.apply(WC::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
            pore(
                &mut w,
                Site {
                    x: x as u32,
                    y: top,
                    z,
                },
                0.5,
            );
        }
        for x in 0..6 {
            let cell = w.config().voxel_volume();
            w.apply(WC::AddWater {
                x,
                y: 3,
                z,
                volume_m3: cell,
            });
        }
    }
    w
}

fn cfg() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    let p = &mut c.founders[SN.index()];
    p.active = Activity::Always;
    p.egg_site = EggSite::Shore;
    p.cruise_body_lengths_per_s = 20.0;
    let am = p.amphibious.as_mut().unwrap();
    am.egg_water_max_m = V;
    am.egg_climb_max_m = 0.0;
    am.egg_moisture_min = 0.5;
    am.egg_dry_moisture = 0.4;
    am.egg_drown_s = 3.0 * DT;
    am.egg_dry_s = 3.0 * DT;
    am.graze_give_up = 0.3;
    am.graze_worth = 0.5;
    am.graze_search_m = 3.0 * V;
    am.roam_search_m = 2.0 * V;
    am.step_moisture_min = 0.4;
    am.on_land = true;
    c
}

fn film(w: &World) -> Flora {
    let fc = FloraConfig {
        voxel_m: V,
        film_capacity_per_m2: 2.0,
        film_wet_min_depth_m: 0.01,
        film_max_depth_m: 2.0 * V,
        film_seed_per_m2: 1.5,
        film_graze_refuge: 0.25,
        film_land_moisture_min: 0.9,
        film_shore_moisture_min: 0.4,
        film_shore_band_m: 2.0 * V,
        film_land_capacity_per_m2: 1.0,
        film_land_capacity_wet_per_m2: 2.0,
        ..Default::default()
    };
    seeded_film(w, fc)
}

// Seeding is an underwater-only founding API. Seed during a temporary flood; the test
// then reads/grows the same film on the unflooded bank, without stepping a water world.
fn seeded_film(w: &World, fc: FloraConfig) -> Flora {
    let mut flooded = w.clone();
    for x in 6..16 {
        for z in 0..6 {
            let y = if x == 7 { 6 } else { 4 };
            let cell = flooded.config().voxel_volume();
            flooded.apply(WC::AddWater {
                x,
                y,
                z,
                volume_m3: cell,
            });
        }
    }
    let mut flora = Flora::new(fc);
    assert!(flora.seed_film(&flooded) > 0);
    flora
}

fn clutch(c: FaunaConfig, s: Site) -> Fauna {
    let mut f = Fauna::new(c);
    let egg = Clutch {
        site: s,
        count: 3,
        organic: 0.7,
        mineral: 0.05,
        energy: 1.3,
        laid_tick: 0,
        lineage: SN,
        dormancy: None,
        wet_ticks: 0,
        dry_ticks: 0,
        face: None,
    };
    f.ledger.introduced_organic_in += egg.organic;
    f.ledger.introduced_mineral_in += egg.mineral;
    f.ledger.introduced_energy_in += egg.energy;
    f.clutches.push(egg);
    f
}

#[test]
fn shore_horizontal_limit_includes_exactly_the_neighbour_column() {
    let w = shore();
    let mut am = cfg().founder(SN).amphibious.unwrap();
    assert!(crate::shore_egg_site(&w.view(), &am, site(6)));
    am.egg_water_max_m -= 1e-5;
    assert!(!crate::shore_egg_site(&w.view(), &am, site(6)));
}

#[test]
fn shore_search_wraps_x_at_the_exact_water_limit() {
    let w = shore();
    let mut am = cfg().founder(SN).amphibious.unwrap();
    assert!(
        crate::shore_egg_site(&w.view(), &am, site(15)),
        "water at x=0 across seam"
    );
    am.egg_water_max_m -= 1e-5;
    assert!(!crate::shore_egg_site(&w.view(), &am, site(15)));
}

#[test]
fn shore_climb_limit_includes_water_surface_gap() {
    let w = shore();
    let mut am = cfg().founder(SN).amphibious.unwrap();
    am.egg_water_max_m = 2.0 * V;
    am.egg_climb_max_m = 2.0 * V;
    assert!(crate::shore_egg_site(&w.view(), &am, site(7)));
    am.egg_climb_max_m -= 1e-5;
    assert!(!crate::shore_egg_site(&w.view(), &am, site(7)));
}

#[test]
fn shore_moisture_limit_is_inclusive_and_standing_water_is_rejected() {
    let mut w = shore();
    let am = cfg().founder(SN).amphibious.unwrap();
    close(
        crate::body::ground_moisture(&w.view(), 6, 3, 2),
        am.egg_moisture_min,
    );
    assert!(crate::shore_egg_site(&w.view(), &am, site(6)));
    pore(&mut w, site(6), am.egg_moisture_min - 0.01);
    assert!(!crate::shore_egg_site(&w.view(), &am, site(6)));
    assert!(!crate::shore_egg_site(&w.view(), &am, site(5)));
}

#[test]
fn drowning_is_strictly_past_the_configured_duration_and_closes_all_books() {
    let w = shore();
    let mut f = clutch(cfg(), site(5));
    let egg = f.clutches[0];
    let mut flora = Flora::new(FloraConfig {
        initial_mineral: 0.0,
        ..Default::default()
    });
    let limit = (f.config.founder(SN).amphibious.unwrap().egg_drown_s / DT).round() as u32;
    for _ in 0..limit {
        shore_eggs(&mut f, &w.view(), &mut flora);
    }
    assert_eq!(f.clutches[0].wet_ticks, limit);
    assert_eq!(f.ledger.eggs_died, 0);
    shore_eggs(&mut f, &w.view(), &mut flora);
    assert!(f.clutches.is_empty());
    assert_eq!(
        f.ledger.eggs_drowned_by_founder[SN.index()],
        u64::from(egg.count)
    );
    assert_eq!(f.ledger.eggs_dried_by_founder[SN.index()], 0);
    close(flora.view().organic(), egg.organic);
    close(flora.view().mineral(), egg.mineral);
    close(flora.view().energy(), egg.energy);
    ledgers(&f, &flora);
}

#[test]
fn drying_is_strictly_past_the_configured_duration_and_closes_all_books() {
    let mut w = shore();
    pore(&mut w, site(6), 0.1);
    let mut f = clutch(cfg(), site(6));
    let egg = f.clutches[0];
    let mut flora = Flora::new(FloraConfig {
        initial_mineral: 0.0,
        ..Default::default()
    });
    let limit = (f.config.founder(SN).amphibious.unwrap().egg_dry_s / DT).round() as u32;
    for _ in 0..limit {
        shore_eggs(&mut f, &w.view(), &mut flora);
    }
    assert_eq!(f.clutches[0].dry_ticks, limit);
    shore_eggs(&mut f, &w.view(), &mut flora);
    assert!(f.clutches.is_empty());
    assert_eq!(
        f.ledger.eggs_dried_by_founder[SN.index()],
        u64::from(egg.count)
    );
    assert_eq!(f.ledger.eggs_drowned_by_founder[SN.index()], 0);
    close(flora.view().organic(), egg.organic);
    close(flora.view().mineral(), egg.mineral);
    close(flora.view().energy(), egg.energy);
    ledgers(&f, &flora);
}

#[test]
fn a_safe_tick_resets_both_consecutive_egg_clocks() {
    let w = shore();
    let mut f = clutch(cfg(), site(6));
    f.clutches[0].wet_ticks = 3;
    f.clutches[0].dry_ticks = 3;
    let mut flora = Flora::new(FloraConfig::default());
    shore_eggs(&mut f, &w.view(), &mut flora);
    assert_eq!((f.clutches[0].wet_ticks, f.clutches[0].dry_ticks), (0, 0));
    ledgers(&f, &flora);
}

#[test]
fn wet_exposure_clears_dry_clock_and_cannot_count_as_both() {
    let w = shore();
    let mut f = clutch(cfg(), site(5));
    f.config.founders[SN.index()]
        .amphibious
        .as_mut()
        .unwrap()
        .egg_dry_moisture = 1.0;
    f.clutches[0].dry_ticks = 3;
    let mut flora = Flora::new(FloraConfig::default());
    shore_eggs(&mut f, &w.view(), &mut flora);
    assert_eq!((f.clutches[0].wet_ticks, f.clutches[0].dry_ticks), (1, 0));
}

#[test]
fn disabling_shore_egg_site_restores_no_shore_mortality() {
    let w = shore();
    let mut c = cfg();
    c.founders[SN.index()].egg_site = EggSite::Film;
    let mut f = clutch(c, site(5));
    let mut flora = Flora::new(FloraConfig::default());
    for _ in 0..8 {
        shore_eggs(&mut f, &w.view(), &mut flora);
    }
    assert_eq!(f.clutches.len(), 1);
    assert_eq!(f.clutches[0].wet_ticks, 0);
    ledgers(&f, &flora);
}

fn grazer(w: &World) -> Fauna {
    let mut f = Fauna::new(cfg());
    introduce(&mut f, w, SN, 5, 2);
    f.animals[0].eating = true;
    f.animals[0].state = crate::State::Walking;
    f
}

fn sky() -> Sky {
    Sky {
        clock_on: false,
        daylight: 1.0,
        rain_m_per_s: 0.0,
        downpour: false,
    }
}

#[test]
fn graze_search_and_worth_include_their_exact_boundaries() {
    let w = shore();
    let f = grazer(&w);
    let mut am = f.config.founder(SN).amphibious.unwrap();
    am.graze_search_m = V;
    let k = 0.7;
    let offer = |s| {
        if s == site(6) {
            am.graze_worth * k
        } else {
            0.0
        }
    };
    assert_eq!(
        graze_target(&w.view(), &am, &f.animals[0], true, k, &offer, &[]),
        Some((V, 0.0))
    );
    let short = crate::Amphibious {
        graze_search_m: V - 1e-5,
        ..am
    };
    assert!(graze_target(&w.view(), &short, &f.animals[0], true, k, &offer, &[]).is_none());
    let poor = |s| {
        if s == site(6) {
            am.graze_worth * k - 1e-5
        } else {
            0.0
        }
    };
    assert!(graze_target(&w.view(), &am, &f.animals[0], true, k, &poor, &[]).is_none());
}

#[test]
fn crowding_at_worth_boundary_and_equal_current_stock_are_distinct() {
    let w = shore();
    let f = grazer(&w);
    let am = f.config.founder(SN).amphibious.unwrap();
    let offer = |s| {
        if s == site(6) {
            2.0 * am.graze_worth
        } else {
            0.0
        }
    };
    assert!(graze_target(&w.view(), &am, &f.animals[0], true, 1.0, &offer, &[site(6)]).is_some());
    assert!(
        graze_target(
            &w.view(),
            &am,
            &f.animals[0],
            true,
            1.0,
            &offer,
            &[site(6), site(6)]
        )
        .is_none()
    );
    let equal = |s| {
        if s == site(6) || s == site(5) {
            am.graze_worth
        } else {
            0.0
        }
    };
    assert!(graze_target(&w.view(), &am, &f.animals[0], true, 1.0, &equal, &[]).is_none());
}

#[test]
fn graze_give_up_zero_disables_the_reflex_on_a_refuge_only_shore() {
    let w = shore();
    let mut f = grazer(&w);
    crate::testpass3_support::place(&mut f, 0, site(6), 0.0);
    let mut flora = film(&w);
    assert!(flora.take_film(site(6), 1e6).is_some());
    assert_eq!(flora.view().edible_film_at(site(6)), 0.0);
    f.config.founders[SN.index()]
        .amphibious
        .as_mut()
        .unwrap()
        .graze_give_up = 0.0;
    assert!(graze_reflex(&f, 0, &w.view(), &flora.view(), &sky()).is_none());
    f.config.founders[SN.index()]
        .amphibious
        .as_mut()
        .unwrap()
        .graze_give_up = 0.3;
    assert!(graze_reflex(&f, 0, &w.view(), &flora.view(), &sky()).is_some());
}

#[test]
fn edible_density_at_give_up_stays_and_just_below_leaves() {
    let w = shore();
    let f = grazer(&w);
    let mut flora = film(&w);
    let k = flora.config().film_capacity_per_m2 * V * V;
    let threshold = f.config.founder(SN).amphibious.unwrap().graze_give_up * k;
    let now = flora.view().edible_film_at(site(5));
    assert!(now > threshold);
    flora.take_film(site(5), now - threshold).unwrap();
    close(flora.view().edible_film_at(site(5)), threshold);
    assert!(graze_reflex(&f, 0, &w.view(), &flora.view(), &sky()).is_none());
    flora.take_film(site(5), threshold * 0.01).unwrap();
    assert!(graze_reflex(&f, 0, &w.view(), &flora.view(), &sky()).is_some());
}

#[test]
fn shore_film_scent_and_habitat_cue_read_the_remaining_partial_bite() {
    let w = shore();
    let mut flora = film(&w);
    let s = site(6);
    assert!(flora.view().film_at(s) > 0.0, "shore soil was seeded");
    let full = flora.view().edible_film_at(s);
    flora.take_film(s, full * 0.4).unwrap();
    let edible = flora.view().edible_film_at(s);
    let capacity = flora.config().film_capacity_per_m2 * V * V;
    let emission = crate::senses::film_emission(&w.view(), &flora.view(), s).expect("food remains");
    close(emission, edible / capacity);
    let ground = flora.view().ground_at(s).unwrap().clone();
    let fauna = Fauna::new(cfg());
    close(
        crate::hunt::ground_cue(&flora.view(), &fauna, &ground, crate::Food::Film, V * V),
        edible / capacity,
    );
    close(flora.take_film(s, 1e6).unwrap().organic, edible);
    assert_eq!(
        crate::senses::film_emission(&w.view(), &flora.view(), s),
        None
    );
}

#[test]
fn a_snail_leaves_a_refuge_patch_with_both_layers_books_closed() {
    let w = shore();
    let mut f = grazer(&w);
    f.set_births_enabled(false);
    let id = f.animals[0].id;
    let mut flora = film(&w);
    for x in 3..=5 {
        for z in 0..6 {
            flora.take_film(Site { x, y: 2, z }, 1e6);
        }
    }
    let old = f.animals[0].site;
    assert_eq!(flora.view().edible_film_at(old), 0.0);
    assert!(f.set_controller(id, crate::controller::heuristic(SN, f.config.teachers)));
    let net0 = flora.view().organic();
    for _ in 0..120 {
        f.step(&w, &mut flora);
    }
    let a = f.view().animal(id).expect("alive");
    assert_ne!(a.site, old, "configured giving-up search moves to food");
    ledgers(&f, &flora);
    close(
        flora.view().organic() - net0,
        f.ledger.deposited_organic_out - f.ledger.eaten_organic_in,
    );
}

#[test]
fn shore_soil_grows_land_film_only_inside_the_configured_band() {
    let mut w = shore();
    let base = film(&w).config().clone();
    let mut fc = FloraConfig {
        film_shore_band_m: V,
        film_seed_per_m2: 0.25,
        film_land_growth_per_s: 0.3,
        film_maintenance_per_s: 0.0,
        film_spread_per_s: 0.0,
        film_dry_death_per_s: 0.2,
        step_every: 1,
        ..base
    };
    let initial = fc.film_seed_per_m2 * V * V;
    let mut on = seeded_film(&w, fc.clone());
    for _ in 0..40 {
        on.step(&mut w);
    }
    assert!(
        on.view().film_at(site(6)) > initial,
        "grows at exact shore-band boundary"
    );
    fc.film_shore_band_m -= 1e-5;
    let mut outside = seeded_film(&w, fc.clone());
    for _ in 0..40 {
        outside.step(&mut w);
    }
    assert!(outside.view().film_at(site(6)) < initial);
    fc.film_shore_band_m = 0.0;
    let mut off = seeded_film(&w, fc);
    for _ in 0..40 {
        off.step(&mut w);
    }
    assert!(off.view().film_at(site(6)) < initial);
}

#[test]
fn partly_cropped_caps_offer_only_the_remaining_removable_tissue() {
    use cubarium_voxel_flora::{Command as FC, Species};
    let w = plain(48, 16, 24);
    let mut c = FloraConfig {
        voxel_m: V,
        ..Default::default()
    };
    c.glowcap.graze_refuge = 0.4;
    let wood = c.glowcap.wood_max;
    let mut flora = Flora::new(c);
    assert!(flora.apply(
        &w,
        FC::Seed {
            x: 10,
            z: 6,
            species: Species::Glowcap,
            wood
        }
    ));
    let s = Site { x: 10, y: 2, z: 6 };
    let st = flora.view().stands_on(s)[0].clone();
    let full = flora.view().edible_foliage(&st);
    assert!(full > 0.0);
    let part = flora
        .take_foliage_of(s, st.place, full * 0.25, &(i64::MIN..=i64::MAX))
        .unwrap()
        .taken
        .organic;
    let cropped = flora.view().stand_in(s, st.place).unwrap().clone();
    let offer = flora.view().edible_foliage(&cropped);
    close(offer, full - part);
    assert_eq!(
        crate::hunt::stand_cue(&flora.view(), &cropped, crate::Food::CapTissue),
        1.0
    );
    close(
        flora
            .take_foliage_of(s, st.place, 1e6, &(i64::MIN..=i64::MAX))
            .unwrap()
            .taken
            .organic,
        offer,
    );
    let empty = flora.view().stand_in(s, st.place).unwrap().clone();
    assert!(empty.foliage > 0.0);
    assert_eq!(
        crate::hunt::stand_cue(&flora.view(), &empty, crate::Food::CapTissue),
        0.0
    );
}

#[test]
fn places_film_and_shore_egg_point_at_the_reflexes_faces() {
    let w = shore();
    let mut f = grazer(&w);
    let mut flora = film(&w);
    // From a damp bank, the neighbouring film face is exposed above water, and
    // the current bank is an eligible shore egg face. Underwater film is hidden
    // by the water surface, just as it is for the body's other ray senses.
    crate::testpass3_support::place(&mut f, 0, site(6), 0.0);
    let target = Site { x: 6, y: 3, z: 1 };
    for x in 0..16 {
        for z in 0..6 {
            let s = Site { z, ..site(x) };
            if s != target { flora.take_film(s, 1e6); }
        }
    }
    let p = &mut f.config.founders[SN.index()];
    p.senses.sight_m = 4.0;
    p.senses.sight_night_m = 4.0;
    p.senses.sight_still_fraction = 1.0;
    p.senses.sight_half_angle_deg = 180.0;
    p.senses.still_gain = 1.0;
    p.senses.feeding_sight_fraction = 1.0;
    f.start_tap();
    f.tick = f.animals[0].id % f.config.perception.stagger_ticks();
    let sky = sky();
    crate::places::refresh(&mut f, &w.view(), &flora.view(), &sky);
    let a = f.animals[0];
    let am = f.config.founder(SN).amphibious.unwrap();
    let v = w.view();
    let fv = flora.view();
    let frame = crate::observe_v3::Frame::of(&a, f64::from(v.config.width) * V, f.tick);
    let k = fv.config.film_capacity_per_m2 * V * V;
    let film_face = super::film_target(&v, &am, &a, true, k, &|s| fv.edible_film_at(s), &[], |_| true).expect("film to walk to");
    let egg_face = super::shore_target(&f, 0, &v, &sky, am.egg_search_m, |_| true).expect("shore to lay on");
    for (kind, face) in [("film", film_face), ("shore_egg", egg_face)] {
        let b = crate::observe_v3::place(&f, &a, kind, &frame, V);
        let (x, z) = crate::pack::centre(face, V);
        let (sin, cos, _) = frame.toward(x, z);
        assert_eq!(b[0], 1.0, "{kind}");
        assert!((b[1] - sin).abs() < 1e-12 && (b[2] - cos).abs() < 1e-12, "{kind}: {b:?}");
    }
}
