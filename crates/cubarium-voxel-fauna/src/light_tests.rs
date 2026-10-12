//! **S3 LIGHT AND FLIGHT** (`design/7_Research/senses-review-2026-10-06.md` §1, §2, §5 S3;
//! the brief's test list): the bellwing's still-attracts / moving-repels rule, smell leading
//! and brightness steering, the looming shadow, the failed-visit mark, the dark-active
//! lanternjaw, the capgnawer's cyan (D2), strike eligibility for a hovering bellwing, and
//! `live` off restoring S1. A child of `step`, sharing the predation fixture.
//!
//! Every case builds its scene by hand and checks a mechanism against the config it sets;
//! no default, ratio or population is asserted. The light readings are taken every call
//! (remote senses at tick rate).

use super::predation_fixture::*;
use crate::light::{self, BeaconKind};
use crate::{Actions, Activity, Command, Fauna, FaunaConfig, Founder, MobilityMode, Pose, StartingStores, State};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Material, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species};

const BW: Founder = Founder::Bellwing;
const LJ: Founder = Founder::Lanternjaw;
const CAP: Founder = Founder::Capgnawer;
/// Facing `+z`: a body to its `+x` side is to its right (bearing +90°).
const NORTH: f64 = 0.0;
/// The bellwing's heuristic's wander, as the steer sees it: on, no feed, no lift.
const WANDER: Actions = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };

/// The default tables with remote senses (and so the beacons) read every tick.
fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c
}

/// A hungry bellwing hovering `up_m` over the plain at `(x, z)` facing `heading`.
fn bellwing(fauna: &mut Fauna, world: &World, x: f64, z: f64, up_m: f64, heading: f64) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft { pose: Pose { x, z, heading_rad: heading, y_m: SURFACE_M + up_m }, founder: BW, stores: StartingStores::FULL },
    ));
    let id = fauna.view().ledger.births - 1;
    let a = animal_mut(fauna, id);
    a.eating = true;
    assert_eq!(a.mobility.mode, MobilityMode::Fly, "fixture: aloft");
    id
}

/// A lanternjaw at `(x, z)` facing `heading`, awake, its lure at `lure`, still or moving.
fn lanternjaw(fauna: &mut Fauna, world: &World, x: f64, z: f64, heading: f64, lure: f64, moving: bool) -> u64 {
    let id = introduce(fauna, world, LJ, StartingStores::FULL);
    place(fauna, id, x, z, heading);
    drive(fauna, id, 0.0, false);
    let a = animal_mut(fauna, id);
    a.pose.y_m = SURFACE_M;
    a.state = State::Resting;
    a.rest = None;
    a.founder_state.lure = lure;
    a.still_ticks = if moving { 0 } else { u64::MAX / 2 };
    a.speed_m_s = if moving { 0.2 } else { 0.0 };
    id
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("alive")
}

/// The bellwing's steer this tick (the tick advanced first).
fn steer(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, daylight: f64) -> Option<(Actions, f64)> {
    fauna.tick += 1;
    let i = index(fauna, id);
    light::steer_flyer(fauna, i, &world.view(), &flora.view(), daylight, WANDER, 0.0)
}

fn light_of(fauna: &Fauna, id: u64) -> crate::perception::LightReading {
    fauna.percept(id).expect("a record").light
}

/// The plain `rows` tall at `hour`.
fn plain_at(hour: f64, rows: u32, seed: u64) -> World {
    plain_of_height(DayClock { start_hour: hour, ..DayClock::default() }, seed, rows)
}

// ------------------------------------------------------- the still-attracts / moving-repels rule

/// **The same bellwing** faced with a still lit lure, a moving lit lure and an unlit hunter,
/// each 2–3 m to its right in the dark: it turns toward the first, away from the second, and
/// takes no notice of the third (its heuristic's answer stands).
#[test]
fn a_bellwing_approaches_a_still_lit_lure_only() {
    let world = clock_off(3);
    let flora = Flora::new(FloraConfig::default());
    let arm = |lure: f64, moving: bool| {
        let mut fauna = Fauna::new(config());
        let bw = bellwing(&mut fauna, &world, 2.0, LANE_Z, 0.3, NORTH);
        lanternjaw(&mut fauna, &world, 5.0, LANE_Z, WEST, lure, moving);
        let got = steer(&mut fauna, &world, &flora, bw, 0.0);
        (got, light_of(&fauna, bw), fauna.ledger)
    };
    let (still, l, ledger) = arm(1.0, false);
    assert!(l.beacon && l.beacon_kind == BeaconKind::Lure.index() as u8, "the still lit lure is its beacon: {l:?}");
    let (act, _) = still.expect("a still light attracts");
    assert!(act.turn > 0.0, "toward it, on its right: {act:?}");
    assert!(ledger.beacon_samples[BW.index()] > 0 && ledger.repel_samples[BW.index()] == 0);

    let (moving, l, ledger) = arm(1.0, true);
    assert!(!l.beacon && l.repel_level > 0.0, "the moving lure is a repellent, not a beacon: {l:?}");
    let (act, _) = moving.expect("a moving light repels");
    assert!(act.turn < 0.0, "away from it, to the left: {act:?}");
    assert!(ledger.repel_samples[BW.index()] > 0 && ledger.beacon_samples[BW.index()] == 0);

    let (unlit, l, _) = arm(0.0, false);
    assert!(!l.beacon && l.repel_level == 0.0, "an unlit hunter is no light: {l:?}");
    assert!(unlit.is_none(), "its heuristic's answer stands: {unlit:?}");
}

// ------------------------------------------------------- smell leads, brightness steers

/// A bloomcrown in flower on the plain at noon, beyond the bellwing's light range: with the
/// flower giving off its nectar plume it turns up the smell toward the flower; with the flower
/// giving none ("light alone") it does not — yet light alone finds the same flower once it is
/// within its light range. So a flower with scent and light is found from farther.
#[test]
fn smell_leads_and_brightness_steers() {
    let mut world = plain_at(12.0, 24, 5);
    let mut fc = FloraConfig::default();
    fc.bloomcrown.nectar_per_s = 1e-3;
    fc.bloomcrown.nectar_cap = 2e-3;
    let mut flora = Flora::new(fc);
    let (fx, fz) = (32i64, 6u32);
    let wood = flora.config().species(Species::Bloomcrown).wood_max;
    assert!(flora.apply(&world, FloraCommand::Seed { x: fx, z: fz, species: Species::Bloomcrown, wood }));
    for _ in 0..60 {
        flora.step(&mut world);
    }
    let flower_x = (fx as f64 + 0.5) * VOXEL_M;
    assert!(flora.view().stands.iter().any(|s| s.nectar > 0.0), "fixture: the flower holds nectar");
    let light_m = FaunaConfig::default().founder(BW).senses.light_m;
    let daylight = world.life_daylight();
    let run = |nectar_per_s: f64, x: f64| {
        let mut c = config();
        c.scent.nectar_per_s = nectar_per_s;
        c.scent.nectar_full = flora.config().bloomcrown.nectar_cap;
        let mut fauna = Fauna::new(c);
        let bw = bellwing(&mut fauna, &world, x, (f64::from(fz) + 0.5) * VOXEL_M, 0.5, NORTH);
        let got = steer(&mut fauna, &world, &flora, bw, daylight);
        (got, light_of(&fauna, bw), fauna.ledger)
    };
    // Farther than its light reaches (the short way round is +x).
    let far = flower_x - light_m - 0.5;
    let (smelled, l, ledger) = run(2.25, far);
    assert!(!l.beacon, "fixture: the flower is beyond its light: {l:?}");
    let (act, _) = smelled.expect("smell leads it");
    assert!(act.turn > 0.0, "toward the flower, on its right: {act:?}");
    assert!(ledger.scent_led_samples[BW.index()] > 0);
    let (unsmelled, _, _) = run(0.0, far);
    assert!(unsmelled.is_none(), "light alone does not find it from there: {unsmelled:?}");
    // Light alone, within its light range.
    let (seen, l, _) = run(0.0, flower_x - 0.5 * light_m);
    assert!(l.beacon && l.beacon_kind == BeaconKind::Flower.index() as u8, "the flower is its beacon near: {l:?}");
    assert!(seen.is_some_and(|(a, _)| a.turn > 0.0), "and brightness steers it there: {seen:?}");
}

// ------------------------------------------------------- the looming shadow

/// A body of another lineage moving within the bellwing's `loom_m`, on its right, coming at
/// it (S3b: closing in within `loom_tau_s`): it turns away and climbs. The same body still,
/// or moving just beyond `loom_m`, is no shadow.
#[test]
fn a_looming_shadow_repels() {
    let world = clock_off(7);
    let flora = Flora::new(FloraConfig::default());
    let loom_m = FaunaConfig::default().founder(BW).senses.loom_m;
    let tau = config().light.loom_tau_s;
    let arm = |dx: f64, moving: bool| {
        let mut fauna = Fauna::new(config());
        let bw = bellwing(&mut fauna, &world, 2.0, LANE_Z, 0.1, NORTH);
        let g = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
        place(&mut fauna, g, 2.0 + dx, LANE_Z, WEST);
        drive(&mut fauna, g, 0.0, false);
        let a = animal_mut(&mut fauna, g);
        a.pose.y_m = SURFACE_M;
        a.still_ticks = if moving { 0 } else { u64::MAX / 2 };
        a.speed_m_s = if moving && tau > 0.0 { 2.0 * (dx + 1.0) / tau } else { 0.0 };
        let got = steer(&mut fauna, &world, &flora, bw, 1.0);
        (got, fauna.ledger.loom_samples[BW.index()])
    };
    let (near, n) = arm(0.5 * loom_m, true);
    let (act, lift) = near.expect("a looming shadow repels");
    assert!(act.turn < 0.0 && lift > 0.0, "away to the left and up: {act:?} lift {lift}");
    assert!(n > 0);
    let (still, n) = arm(0.5 * loom_m, false);
    assert!(still.is_none() && n == 0, "a still body casts no looming shadow: {still:?}");
    let (beyond, n) = arm(loom_m + 0.5, true);
    assert!(beyond.is_none() && n == 0, "a moving body beyond loom_m does not loom: {beyond:?}");
}

// ------------------------------------------------------- the failed-visit mark

/// A bellwing inspecting a still lit lure (no nectar there): after `visit_s` without a sip
/// the visit fails and the place is marked; while the mark lasts it does not approach the
/// lure again; once `mark_s` has passed, it does.
#[test]
fn a_failed_visit_mark_stops_a_repeat_visit_and_decays() {
    let world = clock_off(9);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.light.visit_s = 1.0;
    c.light.mark_s = 5.0;
    let visit = cubarium_voxel::ticks_at_least_one(c.light.visit_s);
    let mark = cubarium_voxel::ticks_at_least_one(c.light.mark_s);
    let mut fauna = Fauna::new(c);
    let lj = lanternjaw(&mut fauna, &world, 5.0, LANE_Z, WEST, 1.0, false);
    let (lx, lz, ly) = light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).expect("a lure");
    let bw = bellwing(&mut fauna, &world, lx, lz, ly - SURFACE_M, NORTH);
    let lure = BeaconKind::Lure.index();
    assert!(steer(&mut fauna, &world, &flora, bw, 0.0).is_some(), "at the lure: inspecting");
    assert_eq!(fauna.ledger.beacon_visits[BW.index()][lure], 1, "one inspection begun");
    fauna.tick += visit;
    assert!(steer(&mut fauna, &world, &flora, bw, 0.0).is_none(), "the inspection fails: back to its search");
    assert_eq!(fauna.ledger.failed_visits[BW.index()][lure], 1, "and is counted");
    for _ in 0..3 {
        let again = steer(&mut fauna, &world, &flora, bw, 0.0);
        assert!(again.is_none() && !light_of(&fauna, bw).beacon, "marked: no repeat visit ({again:?})");
    }
    assert_eq!(fauna.ledger.beacon_visits[BW.index()][lure], 1, "no new inspection while marked");
    fauna.tick += mark;
    assert!(steer(&mut fauna, &world, &flora, bw, 0.0).is_some(), "the mark decayed: the lure draws it again");
    assert!(light_of(&fauna, bw).beacon);
}

// ------------------------------------------------------- the dark-active lanternjaw (D1)

/// The default lanternjaw at noon: awake under a rock roof (a dark face), asleep in the open
/// (a lit face), by the config's own day line against its face's sky × daylight.
#[test]
fn a_lanternjaw_is_active_in_a_dark_cave_by_day_and_not_in_open_daylight() {
    let mut world = plain_at(12.0, 8, 33);
    let depth = world.config().depth;
    for z in 0..depth {
        for x in 0..20 {
            world.apply(WorldCommand::SetMaterial { x, y: SURFACE_Y + 3, z, material: Material::Rock });
        }
    }
    let life = FaunaConfig::default().life;
    let state_at = |x_m: f64| {
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = FaunaConfig::default();
        c.founders[LJ.index()].lure_daylight = false;
        let mut fauna = Fauna::new(c);
        let lj = introduce(&mut fauna, &world, LJ, StartingStores::FULL);
        place(&mut fauna, lj, x_m, LANE_Z, EAST);
        drive(&mut fauna, lj, 0.0, false);
        fauna.step_with(&world, &mut flora, 1);
        let a = animal(&fauna, lj);
        let sky = world.view().sky_visibility((x_m / VOXEL_M).floor() as i64, a.sense_layer(VOXEL_M), a.site.z);
        (a.state, sky * world.life_daylight())
    };
    let (st, light) = state_at(2.5);
    assert!(light < life.day_line, "fixture: the face under the roof is dark ({light})");
    assert_ne!(st, State::Asleep, "under the roof at noon it is awake");
    let (st, light) = state_at(8.0);
    assert!(light >= life.day_line, "fixture: the open face is lit ({light})");
    assert_eq!(st, State::Asleep, "in the open at noon it sleeps");
}

// ------------------------------------------------------- the capgnawer's cyan (D2)

/// A hungry capgnawer a metre from a still lit lure: with no fungal scent (its chemistry
/// reads none, or under the gate) it walks to the lure; with the fungal scent at the gate or
/// over it, it doesn't; fed, it doesn't.
#[test]
fn a_hungry_capgnawer_without_fungal_scent_visits_a_lit_lure() {
    let world = clock_off(11);
    let flora = Flora::new(FloraConfig::default());
    let gate = config().light.glow_scent_gate;
    let arm = |fungal: Option<f64>, hungry: bool| {
        let mut fauna = Fauna::new(config());
        let lj = lanternjaw(&mut fauna, &world, 5.0, LANE_Z, WEST, 1.0, false);
        let (lx, lz, _) = light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).expect("a lure");
        let cap = introduce(&mut fauna, &world, CAP, StartingStores::FULL);
        place(&mut fauna, cap, lx, lz - 1.0, NORTH);
        drive(&mut fauna, cap, 0.0, false);
        animal_mut(&mut fauna, cap).eating = hungry;
        fauna.tick += 1;
        let i = index(&fauna, cap);
        let got = light::steer_glow(&mut fauna, i, &world.view(), &flora.view(), 0.0, Actions { forward: 1.0, turn: 0.0, feed: 0.0 }, fungal);
        (got, fauna.percept(cap).map(|p| p.light))
    };
    for fungal in [None, Some(0.5 * gate)] {
        let (got, l) = arm(fungal, true);
        let l = l.expect("a record");
        assert!(l.beacon && l.beacon_kind == BeaconKind::Lure.index() as u8, "the lure is its beacon: {l:?}");
        let act = got.unwrap_or_else(|| panic!("fungal {fungal:?}: the cyan draws it"));
        assert!(act.forward > 0.0 && act.turn.abs() < 0.5, "straight ahead to it: {act:?}");
    }
    let (got, _) = arm(Some(gate), true);
    assert!(got.is_none(), "fungal scent at the gate: its chemistry leads ({got:?})");
    let (got, _) = arm(None, false);
    assert!(got.is_none(), "fed: it does not look ({got:?})");
}

// ------------------------------------------------------- strike eligibility

/// `is_prey_of`: a bellwing hovering with its body inside the lanternjaw's jaw band over the
/// lanternjaw's standing surface is takeable; one hovering above the band is not; one
/// dropping never is.
#[test]
fn a_bellwing_hovering_in_reach_is_takeable_and_one_above_reach_is_not() {
    let world = clock_off(13);
    let mut fauna = Fauna::new(config());
    let lj = lanternjaw(&mut fauna, &world, 5.0, LANE_Z, WEST, 0.0, false);
    let c = *fauna.config;
    let h = c.founder(LJ).hunter.expect("a hunter");
    let band = h.jaw_band_height_fraction * crate::body_of(&c, &animal(&fauna, lj)).expect("a body").height_m;
    let bw = bellwing(&mut fauna, &world, 4.2, LANE_Z, 0.25 * band, WEST);
    let (hunter, prey) = (animal(&fauna, lj), animal(&fauna, bw));
    assert!(crate::is_prey_of(&c, &hunter, &prey), "hovering in the band: takeable");
    animal_mut(&mut fauna, bw).pose.y_m = SURFACE_M + 2.0 * band;
    assert!(!crate::is_prey_of(&c, &hunter, &animal(&fauna, bw)), "hovering above the band: not");
    let a = animal_mut(&mut fauna, bw);
    a.pose.y_m = SURFACE_M + 0.25 * band;
    a.mobility.falling = true;
    assert!(!crate::is_prey_of(&c, &hunter, &animal(&fauna, bw)), "dropping: not");
}

// ------------------------------------------------------- live off

/// With the bellwing's and the capgnawer's tables not live, light does nothing: the steers
/// stand back, and a stepped bellwing beside a still lit lure holds exactly what it holds
/// beside the same lanternjaw unlit.
#[test]
fn live_off_restores_s1() {
    let world = clock_off(17);
    let flora0 = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[BW.index()].senses.live = false;
    c.founders[CAP.index()].senses.live = false;
    c.founders[LJ.index()].active = Activity::Always;
    // The steers stand back.
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world, 2.0, LANE_Z, 0.3, NORTH);
    lanternjaw(&mut fauna, &world, 5.0, LANE_Z, WEST, 1.0, false);
    assert!(steer(&mut fauna, &world, &flora0, bw, 0.0).is_none(), "the bellwing's steer stands back");
    let cap = introduce(&mut fauna, &world, CAP, StartingStores::FULL);
    place(&mut fauna, cap, 4.0, LANE_Z - 1.0, NORTH);
    animal_mut(&mut fauna, cap).eating = true;
    let i = index(&fauna, cap);
    assert!(light::steer_glow(&mut fauna, i, &world.view(), &flora0.view(), 0.0, WANDER, None).is_none(), "the capgnawer's too");
    // Stepped: the lure changes nothing; with the tables live (the control) it does.
    let held = |c: FaunaConfig, lure: f64| {
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(c);
        let bw = bellwing(&mut fauna, &world, 2.0, LANE_Z, 0.3, NORTH);
        let teachers = fauna.config.teachers;
        fauna.controllers.set(bw, crate::controller::heuristic(BW, teachers));
        assert!(fauna.controllers.layer(bw) == crate::Layer::Heuristic, "fixture: driven by its own heuristic");
        let lj = lanternjaw(&mut fauna, &world, 5.0, LANE_Z, WEST, lure, false);
        let mut out = Vec::new();
        for _ in 0..40 {
            animal_mut(&mut fauna, lj).founder_state.lure = lure;
            animal_mut(&mut fauna, bw).eating = true;
            fauna.step_with(&world, &mut flora, 1);
            let a = animal(&fauna, bw);
            out.push((a.founder_state.held, a.founder_state.lift, a.pose));
        }
        (out, fauna.ledger.beacon_samples[BW.index()] + fauna.ledger.repel_samples[BW.index()] + fauna.ledger.loom_samples[BW.index()])
    };
    let (lit, n) = held(c, 1.0);
    let (dark, _) = held(c, 0.0);
    assert_eq!(n, 0, "no light steering while not live");
    let mut on = c;
    on.founders[BW.index()].senses.live = true;
    let (lit_on, n_on) = held(on, 1.0);
    let (dark_on, _) = held(on, 0.0);
    assert!(n_on > 0 && lit_on != dark_on, "control: live, the lit lure steers it ({n_on} samples)");
    assert_eq!(lit, dark, "the lit lure changes nothing");
}

// ------------------------------------------------------- LEADS-PAY: a lead that never pays

/// A config whose lead budget runs out on the bellwing's `samples`-th controller sample of
/// leading (half a sample short of it, so rounding never decides).
fn lead_config(samples: f64) -> FaunaConfig {
    let mut c = config();
    let period_s = c.manifest(BW).cadence_ticks().max(1) as f64 * cubarium_voxel::DT;
    c.teachers.lead_give_up_bl = (samples - 0.5) * period_s * c.founder(BW).cruise_body_lengths_per_s;
    c
}

/// A bloomcrown in flower on the plain at noon giving off its nectar plume, and a hungry
/// bellwing beyond its light range within the plume: `(world, flora, fauna,
/// bellwing, the flower's (x, z))`.
fn scent_scene(c: FaunaConfig) -> (World, Flora, Fauna, u64, (f64, f64)) {
    let mut world = plain_at(12.0, 24, 5);
    let mut fc = FloraConfig::default();
    fc.bloomcrown.nectar_per_s = 1e-3;
    fc.bloomcrown.nectar_cap = 2e-3;
    let mut flora = Flora::new(fc);
    let (fx, fz) = (32i64, 6u32);
    let wood = flora.config().species(Species::Bloomcrown).wood_max;
    assert!(flora.apply(&world, FloraCommand::Seed { x: fx, z: fz, species: Species::Bloomcrown, wood }));
    for _ in 0..60 {
        flora.step(&mut world);
    }
    assert!(flora.view().stands.iter().any(|s| s.nectar > 0.0), "fixture: the flower holds nectar");
    let flower = ((fx as f64 + 0.5) * VOXEL_M, (f64::from(fz) + 0.5) * VOXEL_M);
    let light_m = c.founder(BW).senses.light_m;
    let mut c = c;
    c.scent.nectar_per_s = 2.25;
    c.scent.nectar_full = flora.config().bloomcrown.nectar_cap;
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world, flower.0 - light_m - 0.5, flower.1, 0.5, NORTH);
    (world, flora, fauna, bw, flower)
}

/// The bellwing's steer this tick with its heuristic asking `asked` (the tick advanced
/// first).
fn steer_asked(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, asked: Actions) -> Option<(Actions, f64)> {
    fauna.tick += 1;
    let i = index(fauna, id);
    let daylight = world.life_daylight();
    light::steer_flyer(fauna, i, &world.view(), &flora.view(), daylight, asked, 0.0)
}

/// The hover-sip's answer at a flower: still, feeding.
const SIP: Actions = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };

/// **A scent lead that never brings a sip is given up**: a hungry bellwing led up the nectar
/// scent, sample after sample, without a sip, follows it for its `lead_give_up_bl` budget;
/// then it gives the lead up — the flower the scent comes from is marked "no reward here",
/// the give-up counted — and the scent from it is passed by while the mark lasts.
#[test]
fn a_bellwing_whose_scent_lead_never_brings_a_sip_gives_it_up() {
    let budget = 3.0;
    let (world, flora, mut fauna, bw, flower) = scent_scene(lead_config(budget));
    let ring = f64::from(world.view().config.width) * VOXEL_M;
    let r = fauna.config.light.mark_radius_m;
    for k in 0..(budget as usize - 1) {
        let led = steer_asked(&mut fauna, &world, &flora, bw, WANDER);
        assert!(led.is_some(), "sample {k}: within its budget the scent leads it");
    }
    assert!(!fauna.percept(bw).unwrap().marked(flower.0, flower.1, fauna.tick, r, ring), "not marked yet");
    let gave_up = steer_asked(&mut fauna, &world, &flora, bw, WANDER);
    assert!(gave_up.is_none(), "its budget spent: its heuristic's answer stands: {gave_up:?}");
    assert_eq!(fauna.lead_give_ups()[BW.index()], 1, "the give-up is counted");
    assert!(fauna.percept(bw).unwrap().marked(flower.0, flower.1, fauna.tick, r, ring), "the scent's source is marked");
    for _ in 0..3 {
        assert!(steer_asked(&mut fauna, &world, &flora, bw, WANDER).is_none(), "the scent from the marked flower is passed by");
    }
}

/// **One that sips keeps feeding**: the same lead, a sip between its stretches: each sip
/// starts the count over, so the bellwing is never turned off the flower's scent, and its
/// sips stand.
#[test]
fn a_bellwing_that_sips_keeps_its_lead() {
    let budget = 3.0;
    let (world, flora, mut fauna, bw, flower) = scent_scene(lead_config(budget));
    let ring = f64::from(world.view().config.width) * VOXEL_M;
    for round in 0..4 {
        for k in 0..(budget as usize - 1) {
            assert!(steer_asked(&mut fauna, &world, &flora, bw, WANDER).is_some(), "round {round} sample {k}: led");
        }
        assert!(steer_asked(&mut fauna, &world, &flora, bw, SIP).is_none(), "round {round}: the sip stands");
    }
    assert_eq!(fauna.lead_give_ups()[BW.index()], 0, "never given up");
    assert!(!fauna.percept(bw).unwrap().marked(flower.0, flower.1, fauna.tick, fauna.config.light.mark_radius_m, ring), "and nothing marked");
}

/// **An approach that keeps moving cannot hold a light forever** (the S3b stall needs a body
/// that stands still): a hungry bellwing steering for a still lit lure short of it, moving
/// every sample, gives the light up at its budget: the lure is marked, not as a stall.
#[test]
fn a_moving_approach_to_a_still_light_is_given_up_at_its_budget() {
    let world = clock_off(3);
    let flora = Flora::new(FloraConfig::default());
    let budget = 3.0;
    let mut fauna = Fauna::new(lead_config(budget));
    let bw = bellwing(&mut fauna, &world, 2.0, LANE_Z, 0.3, NORTH);
    let lj = lanternjaw(&mut fauna, &world, 5.0, LANE_Z, WEST, 1.0, false);
    let (lx, lz, _) = light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).expect("a lure");
    let ring = f64::from(world.view().config.width) * VOXEL_M;
    let r = fauna.config.light.mark_radius_m;
    let sample = |fauna: &mut Fauna| {
        animal_mut(fauna, bw).still_ticks = 0;
        steer(fauna, &world, &flora, bw, 0.0)
    };
    for k in 0..(budget as usize - 1) {
        assert!(sample(&mut fauna).is_some(), "sample {k}: it approaches the still light");
    }
    assert!(sample(&mut fauna).is_none(), "its budget spent: the light is given up");
    assert!(fauna.percept(bw).unwrap().marked(lx, lz, fauna.tick, r, ring), "the lure is marked");
    assert_eq!(fauna.ledger.stalled_visits[BW.index()], 0, "a give-up, not a stall");
    assert_eq!(fauna.lead_give_ups()[BW.index()], 1);
}

/// `lead_give_up_bl` 0: the scent leads it on however long.
#[test]
fn a_zero_lead_budget_never_gives_a_lead_up() {
    let mut c = config();
    c.teachers.lead_give_up_bl = 0.0;
    let (world, flora, mut fauna, bw, _) = scent_scene(c);
    for k in 0..20 {
        assert!(steer_asked(&mut fauna, &world, &flora, bw, WANDER).is_some(), "sample {k}: led");
    }
    assert_eq!(fauna.lead_give_ups()[BW.index()], 0);
}
