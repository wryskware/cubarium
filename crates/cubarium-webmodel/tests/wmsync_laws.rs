//! Small, config-driven checks of WMSYNC mechanisms. No ecology trajectories or tuned defaults.

use cubarium_voxel_fauna::{Amphibious, Founder, FounderPhysiology, LightConfig, VigilMode};
use cubarium_voxel_flora::FloraConfig;
use cubarium_webmodel::fauna::{behavior, laws};
use cubarium_webmodel::flora::film;

fn grazer() -> FounderPhysiology {
    let mut p = FounderPhysiology::frozen(Founder::Browser);
    p.vigilance.mode = VigilMode::Herd;
    p.vigilance.flee_start_m = 3.0;
    p.vigilance.flee_stop_m = 5.0;
    p.vigilance.group_min = 1;
    p.vigilance.calm_s = 4.0;
    p.hunker.enabled = true;
    p.hunker.enter_m = 2.0;
    p.hunker.leave_m = 4.0;
    p.hunker.calm_s = 4.0;
    p.hunker.min_condition = 0.25;
    p.hunker.hunger_calm_exp = 1.0;
    p.hunker.shield = 0.75;
    p.hunker.shield_growth_exp = 2.0;
    p.hunker.pry_per_member_s = 0.05;
    p.hunker.resist_per_s = 0.1;
    p.hunker.opened_s = 2.0;
    p.hunker.ring_resist_per_kin = 0.5;
    p.hunker.ring_kin_max = 2;
    p
}

fn body(p: &FounderPhysiology, condition: f64) -> laws::Body {
    laws::Body {
        body: p.core.birth_body,
        reserve: condition * p.core.reserve_of(p.core.birth_body),
        ..laws::Body::default()
    }
}

fn flyer() -> FounderPhysiology {
    let mut p = FounderPhysiology::frozen(Founder::Bellwing);
    p.senses.live = true;
    p.senses.nir = true;
    p.senses.light_m = 4.0;
    p.senses.light_min = 0.01;
    p
}

fn light() -> LightConfig {
    LightConfig {
        lure_emission: 1.0,
        lure_nir: 1.0,
        lure_scent: 0.0,
        flower_reflectance: 1.0,
        glowcap_emission: 0.5,
        background_floor: 0.1,
        half_m: 1.0,
        visit_s: 2.0,
        learn: false,
        ..LightConfig::default()
    }
}

#[test]
fn hunker_share_requires_pressing_threats_and_responds_to_the_enter_radius() {
    let p = grazer();
    let b = body(&p, 1.0);
    let response = |p: &FounderPhysiology, density, speed| {
        let mut s = behavior::ResponseState::default();
        behavior::update_response(p, &b, &mut s, density, speed, 0.5);
        s
    };
    assert_eq!(response(&p, 0.0, 1.0).hunker, 0.0);
    assert_eq!(response(&p, 0.01, 0.0).hunker, 0.0);
    let low = response(&p, 0.01, 1.0);
    let high = response(&p, 0.02, 1.0);
    assert!(low.hunker > 0.0 && high.hunker > low.hunker);
    let mut wider = p;
    wider.hunker.enter_m *= 2.0;
    assert!(response(&wider, 0.01, 1.0).hunker > low.hunker);
}

#[test]
fn hungry_and_disabled_hunkers_use_the_alternative_flight_response() {
    let p = grazer();
    let response = |p: &FounderPhysiology, condition| {
        let mut s = behavior::ResponseState::default();
        behavior::update_response(p, &body(p, condition), &mut s, 0.1, 1.0, 0.5);
        s
    };
    let fed = response(&p, 1.0);
    let hungry = response(&p, 0.5 * p.hunker.min_condition);
    assert!(fed.hunker > hungry.hunker);
    assert_eq!(hungry.hunker, 0.0);
    assert!(hungry.flight > fed.flight);
    let mut off = p;
    off.hunker.enabled = false;
    let old_response = response(&off, 1.0);
    assert_eq!(old_response.hunker, 0.0);
    assert!(old_response.flight > fed.flight);
}

#[test]
fn longer_calm_holds_more_of_an_existing_hunker() {
    let p = grazer();
    let b = body(&p, 1.0);
    let mut short = behavior::ResponseState {
        hunker: 0.5,
        flight: 0.0,
    };
    let mut long = short;
    let mut slower = p;
    slower.hunker.calm_s *= 2.0;
    behavior::update_response(&p, &b, &mut short, 0.0, 0.0, 0.5);
    behavior::update_response(&slower, &b, &mut long, 0.0, 0.0, 0.5);
    assert!(short.hunker < 0.5 && long.hunker > short.hunker);
}

#[test]
fn prying_requires_net_work_and_respects_youth_and_ringed_resistance() {
    let p = grazer();
    let adult = p.core.birth_body;
    let share =
        |p: &FounderPhysiology, mass, members, kin| behavior::pry_open_share(p, mass, members, kin);
    let equal = p.hunker.resist_per_s / p.hunker.pry_per_member_s;
    assert_eq!(share(&p, adult, equal, 0.0), 0.0);
    let alone = share(&p, adult, equal + 1.0, 0.0);
    assert!(alone > 0.0);
    assert!(share(&p, adult, equal + 2.0, 0.0) > alone);
    assert!(share(&p, adult, equal + 1.0, 1.0) < alone);
    assert_eq!(
        share(&p, adult, equal + 3.0, f64::from(p.hunker.ring_kin_max)),
        share(
            &p,
            adult,
            equal + 3.0,
            f64::from(p.hunker.ring_kin_max) + 10.0
        ),
    );
    assert!(share(&p, 0.5 * adult, equal, 0.0) > share(&p, adult, equal, 0.0));
    let mut tougher = p;
    tougher.hunker.resist_per_s *= 2.0;
    assert!(share(&tougher, adult, equal + 1.0, 0.0) < alone);
    let mut longer = p;
    longer.hunker.opened_s *= 2.0;
    assert!(share(&longer, adult, equal + 1.0, 0.0) > alone);
}

#[test]
fn grown_shields_reduce_hits_and_pried_prey_lose_shield_and_armour() {
    let mut hunter = FounderPhysiology::frozen(Founder::Chorister);
    hunter.hunter.as_mut().unwrap().strike_power = 0.1;
    let mut prey = grazer();
    prey.armour = 0.25;
    let adult = prey.core.birth_body;
    let hit = |prey: &FounderPhysiology, mass, hunker, opened| {
        behavior::guarded_strike_share(&hunter, adult, prey, mass, hunker, opened)
    };
    let unguarded = hit(&prey, adult, 0.0, 0.0);
    let guarded = hit(&prey, adult, 1.0, 0.0);
    assert!(guarded < unguarded);
    let young = 0.5 * adult;
    assert!(hit(&prey, young, 1.0, 0.0) / hit(&prey, young, 0.0, 0.0) > guarded / unguarded);
    let mut softer = prey;
    softer.hunker.shield *= 0.5;
    assert!(hit(&softer, adult, 1.0, 0.0) > guarded);
    let opened = hit(&prey, adult, 1.0, 1.0);
    let mut bared = prey;
    bared.armour = 0.0;
    assert_eq!(opened, laws::strike_share(&hunter, adult, &bared, adult));
    assert!(opened > unguarded);
}

#[test]
fn lure_visits_need_awake_flyers_a_lit_signal_and_lose_to_competing_beacons() {
    let cfg = light();
    let p = flyer();
    let visits = |cfg: &LightConfig, awake, lit, flowers, caps, daylight| {
        behavior::lure_visits_per_s(
            cfg,
            &p,
            p.core.birth_body,
            awake,
            lit,
            flowers,
            caps,
            daylight,
            100.0,
        )
    };
    let night = visits(&cfg, 1.0, 1.0, 0.0, 0.0, 0.0);
    assert!(night > 0.0);
    assert_eq!(visits(&cfg, 0.0, 1.0, 0.0, 0.0, 0.0), 0.0);
    assert_eq!(visits(&cfg, 1.0, 0.0, 0.0, 0.0, 0.0), 0.0);
    assert!(visits(&cfg, 0.5, 1.0, 0.0, 0.0, 0.0) < night);
    assert!(visits(&cfg, 1.0, 0.5, 0.0, 0.0, 0.0) < night);
    assert!(visits(&cfg, 1.0, 1.0, 0.0, 100.0, 0.0) < night);
    let day = visits(&cfg, 1.0, 1.0, 0.0, 0.0, 1.0);
    assert!(visits(&cfg, 1.0, 1.0, 100.0, 0.0, 1.0) < day);
    let mut dark = cfg;
    dark.lure_emission = 0.0;
    dark.lure_nir = 0.0;
    dark.lure_scent = 0.0;
    assert_eq!(visits(&dark, 1.0, 1.0, 0.0, 0.0, 0.0), 0.0);
}

#[test]
fn nectar_response_tracks_stock_flower_density_and_visit_handling() {
    let cfg = light();
    let p = flyer();
    let response = |cfg: &LightConfig, nectar, flowers, area| {
        behavior::nectar_response(cfg, &p, p.core.birth_body, nectar, flowers, 0.0, 1.0, area)
    };
    assert_eq!(response(&cfg, 0.0, 100.0, 100.0), 0.0);
    assert_eq!(response(&cfg, 1.0, 0.0, 100.0), 0.0);
    let scarce = response(&cfg, 0.001, 100.0, 100.0);
    assert!(scarce > 0.0 && response(&cfg, 0.002, 100.0, 100.0) > scarce);
    assert!(response(&cfg, 100.0, 100.0, 100_000.0) < response(&cfg, 100.0, 100.0, 10_000.0));
    let mut slow = cfg;
    slow.visit_s *= 100.0;
    assert!(response(&slow, 100.0, 100.0, 10_000.0) < response(&cfg, 100.0, 100.0, 10_000.0));
}

#[test]
fn giving_up_and_per_snail_worth_control_departure_but_a_refuge_is_not_food() {
    let mut am = Amphibious::RIPPLE_SNAIL;
    am.graze_give_up = 0.3;
    am.graze_worth = 0.5;
    am.graze_search_m = 2.0;
    let depart = |am: &Amphibious, edible, rich, reachable, neighbors| {
        behavior::film_departure(am, edible, rich, reachable, neighbors)
    };
    let leaving = depart(&am, 0.2, 0.8, 20.0, 1.0);
    assert!(leaving > 0.0);
    assert_eq!(depart(&am, 0.4, 0.8, 20.0, 1.0), 0.0);
    assert_eq!(depart(&am, 0.2, 0.4, 20.0, 1.0), 0.0);
    assert_eq!(depart(&am, 0.2, 0.8, 0.0, 1.0), 0.0);
    assert!(depart(&am, 0.2, 0.8, 20.0, 10.0) < leaving);
    let mut off = am;
    off.graze_give_up = 0.0;
    assert_eq!(depart(&off, 0.2, 0.8, 20.0, 1.0), 0.0);
    assert!(depart(&off, 0.0, 0.8, 20.0, 1.0) > 0.0);
    let mut stricter = am;
    stricter.graze_worth = 0.9;
    assert!(depart(&stricter, 0.2, 0.8, 20.0, 1.0) < leaving);
}

#[test]
fn shore_eggs_use_the_moisture_distance_and_climb_knobs_and_stay_above_water() {
    let mut am = Amphibious::RIPPLE_SNAIL;
    am.egg_moisture_min = 0.6;
    am.egg_water_max_m = 2.0;
    am.egg_climb_max_m = 1.0;
    let eligible = |am: &Amphibious, depth, moisture, distance, height| {
        behavior::shore_eligible(am, depth, moisture, distance, height)
    };
    assert!(eligible(
        &am,
        0.0,
        am.egg_moisture_min,
        am.egg_water_max_m,
        am.egg_climb_max_m
    ));
    assert!(!eligible(&am, 0.01, 1.0, 1.0, 0.5));
    assert!(!eligible(&am, 0.0, 0.5 * am.egg_moisture_min, 1.0, 0.5));
    assert!(!eligible(&am, 0.0, 1.0, 1.5 * am.egg_water_max_m, 0.5));
    assert!(!eligible(&am, 0.0, 1.0, 1.0, 1.5 * am.egg_climb_max_m));
    let mut wider = am;
    wider.egg_water_max_m *= 2.0;
    wider.egg_climb_max_m *= 2.0;
    assert!(eligible(
        &wider,
        0.0,
        1.0,
        1.5 * am.egg_water_max_m,
        1.5 * am.egg_climb_max_m
    ));
}

#[test]
fn egg_losses_require_consecutive_exposure_past_the_configured_clock() {
    let mut am = Amphibious::RIPPLE_SNAIL;
    am.egg_drown_s = 4.0;
    am.egg_dry_s = 6.0;
    am.egg_dry_moisture = 0.4;
    let (mut wet, mut dry) = (0.0, 0.0);
    assert_eq!(
        behavior::egg_loss(&am, &mut wet, &mut dry, true, 0.0, am.egg_drown_s),
        None
    );
    assert_eq!(dry, 0.0, "standing water never advances the dry clock");
    assert_eq!(
        behavior::egg_loss(&am, &mut wet, &mut dry, false, 1.0, 0.5),
        None
    );
    assert_eq!(wet, 0.0, "a break resets the wet clock");
    assert_eq!(
        behavior::egg_loss(&am, &mut wet, &mut dry, true, 1.0, am.egg_drown_s + 0.5),
        Some(true)
    );
    wet = 0.0;
    assert_eq!(
        behavior::egg_loss(
            &am,
            &mut wet,
            &mut dry,
            false,
            0.5 * am.egg_dry_moisture,
            am.egg_dry_s
        ),
        None
    );
    assert_eq!(
        behavior::egg_loss(&am, &mut wet, &mut dry, false, am.egg_dry_moisture, 0.5),
        None
    );
    assert_eq!(dry, 0.0, "at the moisture floor is a break in dry exposure");
    assert_eq!(
        behavior::egg_loss(&am, &mut wet, &mut dry, false, 0.0, am.egg_dry_s + 0.5),
        Some(false)
    );
}

#[test]
fn land_film_capacity_uses_the_shore_floor_and_moisture_dependent_capacity() {
    let mut c = FloraConfig::default();
    c.film_land_moisture_min = 0.8;
    c.film_shore_moisture_min = 0.4;
    c.film_shore_band_m = 2.0;
    c.film_land_capacity_per_m2 = 0.2;
    c.film_land_capacity_wet_per_m2 = 0.6;
    c.film_wet_min_depth_m = 0.1;
    c.film_max_depth_m = 2.0;
    c.film_capacity_per_m2 = 0.9;
    assert_eq!(film::capacity(&c, 0.0, 0.6, 2.0 * c.film_shore_band_m), 0.0);
    assert_eq!(
        film::capacity(&c, 0.0, 0.5 * c.film_shore_moisture_min, 0.0),
        0.0
    );
    let shore = film::capacity(
        &c,
        0.0,
        c.film_shore_moisture_min,
        0.5 * c.film_shore_band_m,
    );
    assert_eq!(shore, c.film_land_capacity_per_m2);
    assert!(film::capacity(&c, 0.0, 0.6, 0.5 * c.film_shore_band_m) > shore);
    assert_eq!(
        film::capacity(&c, 0.0, 1.0, 0.0),
        c.film_land_capacity_wet_per_m2
    );
    let mut off = c.clone();
    off.film_shore_band_m = 0.0;
    assert_eq!(film::capacity(&off, 0.0, 0.6, 0.1), 0.0);
    assert_eq!(
        film::capacity(&c, c.film_wet_min_depth_m, 0.0, 100.0),
        c.film_capacity_per_m2
    );
    assert_eq!(film::capacity(&c, 2.0 * c.film_max_depth_m, 1.0, 0.0), 0.0);
}

#[test]
fn edible_film_leaves_the_shared_refuge_floor_on_every_face() {
    let mut c = FloraConfig::default();
    c.voxel_m = 0.5;
    c.film_capacity_per_m2 = 2.0;
    c.film_land_capacity_per_m2 = 0.4;
    c.film_land_capacity_wet_per_m2 = 0.8;
    c.film_graze_refuge = 0.25;
    let faces = 3.0;
    let refuge = faces * c.film_refuge_per_site();
    assert_eq!(film::edible(&c, refuge, faces), 0.0);
    assert_eq!(film::edible(&c, 0.5 * refuge, faces), 0.0);
    assert_eq!(film::edible(&c, 2.0 * refuge, faces), refuge);
    let mut no_refuge = c.clone();
    no_refuge.film_graze_refuge = 0.0;
    assert_eq!(film::edible(&no_refuge, refuge, faces), refuge);
    let mut different_land = c;
    different_land.film_land_capacity_per_m2 *= 2.0;
    different_land.film_land_capacity_wet_per_m2 *= 2.0;
    assert_eq!(film::edible(&different_land, 2.0 * refuge, faces), refuge);
}

#[test]
fn film_growth_uses_separate_land_rate_and_shared_upkeep_without_growing_above_capacity() {
    let mut c = FloraConfig::default();
    c.film_growth_per_s = 0.04;
    c.film_land_growth_per_s = 0.01;
    c.film_maintenance_per_s = 0.002;
    let (stock, capacity, light, dt) = (0.25, 1.0, 0.5, 0.5);
    let land = film::growth(&c, stock, capacity, light, true, dt);
    let underwater = film::growth(&c, stock, capacity, light, false, dt);
    assert!(land.0 > 0.0 && underwater.0 > land.0);
    assert_eq!(land.1, c.film_maintenance_per_s * stock * dt);
    assert_eq!(underwater.1, land.1);
    let mut faster = c.clone();
    faster.film_land_growth_per_s *= 2.0;
    assert!(film::growth(&faster, stock, capacity, light, true, dt).0 > land.0);
    assert_eq!(
        film::growth(&faster, stock, capacity, light, false, dt).0,
        underwater.0
    );
    assert_eq!(film::growth(&c, stock, capacity, 0.0, true, dt).0, 0.0);
    assert_eq!(film::growth(&c, capacity, capacity, light, true, dt).0, 0.0);
    assert_eq!(
        film::growth(&c, 2.0 * capacity, capacity, light, true, dt).0,
        0.0
    );
}
