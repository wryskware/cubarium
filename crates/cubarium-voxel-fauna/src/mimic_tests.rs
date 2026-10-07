//! MIMIC mechanism cases, authored independently before implementation.
//! Scenes set their switches, geometry, costs and timing explicitly; no tuned default
//! or population outcome is asserted. All world cases stay within 200 fauna ticks.

use super::predation_fixture::*;
use crate::light::{self, Beacon, BeaconKind, UV};
use crate::{
    Actions, Activity, Command, Diet, Fauna, FaunaConfig, Food, Founder, HuntMode, Hunter,
    MobilityMode, Pose, StartingStores,
};
use cubarium_voxel::{World, weather::DayClock};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species as Plant};

const LJ: Founder = Founder::Lanternjaw;
const SP: Founder = Founder::Seedporter;
const BW: Founder = Founder::Bellwing;

fn noon(seed: u64) -> World {
    plain_of_height(
        DayClock {
            start_hour: 12.0,
            ..DayClock::default()
        },
        seed,
        24,
    )
}

fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.hunt.patrol = true;
    c.hunt.meal_first = true;
    c.hunt.strike_chosen_only = false;
    c.hunt.cue_scan_m = 8.0;
    c.hunt.patch_wait_s = 1e6;
    c.light.lure_uv = 1.0;
    c.light.lure_scent = 1.0;
    c.light.learn = true;
    c.light.uv_innate = 0.7;
    c.light.cyan_innate = 0.4;
    c.light.expect_min = 0.05;
    c.light.learn_rate = 0.4;
    c.light.forget_s = 1e9;
    c.light.visit_s = 0.5;
    c.light.mark_s = 10.0;
    c.founders[LJ.index()].lure_daylight = true;
    c.founders[LJ.index()].flower_ambush_m = 0.7;
    c.founders[LJ.index()].senses.light_m = 8.0;
    c.founders[BW.index()].active = Activity::Always;
    c.founders[BW.index()].senses.live = true;
    c.founders[BW.index()].senses.uv = true;
    c.founders[BW.index()].senses.light_m = 8.0;
    c.founders[BW.index()].senses.light_min = 0.0001;
    // A complete explicit catch table; the test does not depend on its shipped values.
    let mut prey = [false; Founder::COUNT];
    prey[BW.index()] = true;
    c.founders[SP.index()].hunter = Some(Hunter {
        prey,
        strike_power: 8.0,
        prey_max_ratio: 4.0,
        strike_reach_length_fraction: 0.8,
        jaw_band_height_fraction: 2.0,
        jaw_forward_length_fraction: 0.5,
        windup_s: 0.1,
        lunge_s: 0.15,
        recovery_s: 0.5,
        strike_cost_s: 0.1,
        lure_power: 0.0,
    });
    c.founders[SP.index()].catch_bellwings = true;
    c.founders[SP.index()].catch_ceiling_m = 0.3;
    c.founders[SP.index()].catch_gain_min = 0.0;
    c.founders[SP.index()].active = Activity::Always;
    c.founders[SP.index()].diet = Diet::of(&[Food::Fruit, Food::Carrion]);
    c
}

fn heuristic(fauna: &mut Fauna, id: u64, f: Founder) {
    fauna
        .controllers
        .set(id, crate::controller::heuristic(f, fauna.config.teachers));
}

fn flower(x: f64, y_m: f64) -> Beacon {
    Beacon {
        kind: BeaconKind::Flower,
        x,
        z: LANE_Z,
        y_m,
        sky: 1.0,
        power: 1.0,
        moving: false,
        body: 0,
    }
}

fn waiting_lanternjaw(fauna: &mut Fauna, world: &World, x: f64) -> u64 {
    let id = introduce(fauna, world, LJ, HUNTING);
    place(fauna, id, x, LANE_Z, EAST);
    heuristic(fauna, id, LJ);
    let a = animal_mut(fauna, id);
    a.pose.y_m = SURFACE_M;
    a.eating = true;
    a.hunt.mode = HuntMode::Wait;
    a.hunt.patch = (x, LANE_Z);
    a.still_ticks = u64::MAX / 2;
    id
}

fn bellwing(fauna: &mut Fauna, world: &World, x: f64, lift: f64) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft {
            pose: Pose {
                x,
                z: LANE_Z,
                heading_rad: WEST,
                y_m: SURFACE_M + lift
            },
            founder: BW,
            stores: StartingStores::FULL,
        }
    ));
    let id = fauna.animals.last().unwrap().id;
    drive(fauna, id, 0.0, false);
    animal_mut(fauna, id).eating = true;
    animal_mut(fauna, id).still_ticks = 0;
    id
}

#[test]
fn daylight_lure_obeys_the_lineage_switch() {
    for on in [true, false] {
        let world = noon(501);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config();
        // Isolate the light switch from the independent flower-location decision.
        c.founders[LJ.index()].active = Activity::Always;
        c.founders[LJ.index()].lure_daylight = on;
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let lj = waiting_lanternjaw(&mut fauna, &world, 3.0);
        fauna.beacons.fixed(
            &world.view(),
            &flora.view(),
            vec![flower(3.2, SURFACE_M + 0.2)],
        );
        for _ in 0..12 {
            fauna.step_with(&world, &mut flora, 1);
        }
        assert_eq!(
            animal(&fauna, lj).founder_state.lure > 0.0,
            on,
            "daylight lure follows its switch"
        );
        assert_closes(&fauna, "daylight light switch");
    }
}

#[test]
fn nonpatrol_lure_obeys_the_same_daylight_switch() {
    struct Lit;
    impl crate::controller::Controller for Lit {
        fn drive(&mut self, _: &[f64]) -> crate::controller::Response {
            crate::controller::Response::Bounded(Actions::REST)
        }
        fn lure(&self) -> f64 {
            1.0
        }
        fn reset(&mut self) {}
    }
    for on in [true, false] {
        let world = noon(510);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config();
        c.hunt.patrol = false;
        c.founders[LJ.index()].active = Activity::Always;
        c.founders[LJ.index()].lure_daylight = on;
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let lj = waiting_lanternjaw(&mut fauna, &world, 3.0);
        fauna.controllers.set(lj, Box::new(Lit));
        for _ in 0..12 {
            fauna.step_with(&world, &mut flora, 1);
        }
        assert_eq!(
            animal(&fauna, lj).founder_state.lure > 0.0,
            on,
            "daylight gate applies to any lure-producing controller"
        );
        assert_closes(&fauna, "nonpatrol light switch");
    }
}

#[test]
fn daytime_lure_is_approached_inspected_and_teaches_the_flower_class() {
    let world = noon(502);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config());
    let lj = waiting_lanternjaw(&mut fauna, &world, 4.0);
    animal_mut(&mut fauna, lj).founder_state.lure = 1.0;
    let bw = bellwing(&mut fauna, &world, 2.0, 0.2);
    animal_mut(&mut fauna, bw).pose.heading_rad = 0.0;
    fauna.beacons.fixed(
        &world.view(),
        &flora.view(),
        vec![flower(7.0, SURFACE_M + 0.2)],
    );
    let steer = |fauna: &mut Fauna| {
        fauna.tick += 1;
        let i = fauna.animals.iter().position(|a| a.id == bw).unwrap();
        light::steer_flyer(
            fauna,
            i,
            &world.view(),
            &flora.view(),
            1.0,
            Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0,
            },
            0.0,
        )
    };
    let (act, _) = steer(&mut fauna).expect("the daylight flower mimic attracts");
    let p = fauna.percept(bw).unwrap();
    assert!(p.light.beacon && p.light.beacon_kind == BeaconKind::Lure.index() as u8);
    assert!(act.turn > 0.0, "turns toward the lure on its right");
    assert_eq!(BeaconKind::Lure.class_seen(&fauna.config.light, true), UV);
    let (x, z, y) = light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).unwrap();
    let a = animal_mut(&mut fauna, bw);
    a.pose.x = x;
    a.pose.z = z;
    a.pose.y_m = y;
    a.still_ticks = 0;
    assert!(steer(&mut fauna).is_some());
    assert!(
        fauna.percept(bw).unwrap().visit.active,
        "inspects the apparent flower"
    );
    fauna.tick += cubarium_voxel::ticks_at_least_one(fauna.config.light.visit_s);
    animal_mut(&mut fauna, bw).still_ticks = 0;
    let _ = steer(&mut fauna);
    assert_eq!(
        fauna.ledger.failed_visits[BW.index()][BeaconKind::Lure.index()],
        1
    );
    let learned = light::expectation_of(
        &fauna.config.light,
        fauna.percept(bw).unwrap(),
        UV,
        fauna.tick,
    );
    assert!(
        learned < fauna.config.light.uv_innate,
        "no nectar lowers flower expectation"
    );
}

#[test]
fn a_daytime_lure_inspection_can_be_struck() {
    let world = noon(503);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[LJ.index()].active = Activity::Always;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let lj = waiting_lanternjaw(&mut fauna, &world, 3.0);
    fauna.beacons.fixed(
        &world.view(),
        &flora.view(),
        vec![flower(3.2, SURFACE_M + 0.2)],
    );
    for _ in 0..12 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert!(
        animal(&fauna, lj).founder_state.lure > 0.0,
        "fixture: lure has sampled its controller"
    );
    let (x, _, y) = light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).unwrap();
    let bw = bellwing(&mut fauna, &world, x, y - SURFACE_M);
    fauna
        .beacons
        .fixed(&world.view(), &flora.view(), vec![flower(6.0, y)]);
    let i = fauna.animals.iter().position(|a| a.id == bw).unwrap();
    let _ = light::steer_flyer(
        &mut fauna,
        i,
        &world.view(),
        &flora.view(),
        1.0,
        Actions {
            forward: 1.0,
            turn: 0.0,
            feed: 0.0,
        },
        0.0,
    );
    assert!(
        fauna.percept(bw).unwrap().visit.active,
        "at its lure inspection"
    );
    for _ in 0..30 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert!(
        fauna.ledger.strikes_on_by_founder[BW.index()] > 0,
        "the visitor is inside the existing strike capsule"
    );
    assert_closes(&fauna, "daytime lure strike");
}

#[test]
fn a_daytime_lanternjaw_moves_to_and_holds_a_flower_ambush() {
    for offset in [0.2, 2.0] {
        let world = noon(504);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(config());
        fauna.set_births_enabled(false);
        // Keep DarkActive: the flower rule itself must make the daytime exception.
        fauna.config.founders[LJ.index()].active = Activity::DarkActive;
        let lj = waiting_lanternjaw(&mut fauna, &world, 3.0);
        let target = 3.0 + offset;
        fauna.beacons.fixed(
            &world.view(),
            &flora.view(),
            vec![flower(target, SURFACE_M + 0.2)],
        );
        for _ in 0..180 {
            fauna.step_with(&world, &mut flora, 1);
        }
        let p = animal(&fauna, lj).pose;
        let distance = (p.x - target).hypot(p.z - LANE_Z);
        if offset <= fauna.config.founder(LJ).flower_ambush_m {
            assert!(
                distance <= fauna.config.founder(LJ).flower_ambush_m,
                "holds inside its configured flower ambush radius: {distance}"
            );
            assert!(
                animal(&fauna, lj).founder_state.lure > 0.0,
                "holding its lit ambush by day"
            );
        } else {
            assert!(
                distance < offset,
                "moves toward its daytime flower ambush: {distance} < {offset}"
            );
        }
        assert_closes(&fauna, "flower ambush");
    }
}

#[test]
fn flower_mimic_switch_off_preserves_dark_active_daytime_sleep() {
    let world = noon(507);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[LJ.index()].active = Activity::DarkActive;
    c.founders[LJ.index()].wake_hunger = 0.95;
    c.founders[LJ.index()].lure_daylight = false;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let lj = waiting_lanternjaw(&mut fauna, &world, 3.0);
    fauna.beacons.fixed(
        &world.view(),
        &flora.view(),
        vec![flower(3.2, SURFACE_M + 0.2)],
    );
    for _ in 0..12 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert_eq!(
        animal(&fauna, lj).state,
        crate::State::Asleep,
        "a flower alone does not override the configured dark-active rhythm with the switch off"
    );
    assert_eq!(animal(&fauna, lj).founder_state.lure, 0.0);
    assert_closes(&fauna, "flower mimic off");
}

/// Freeze the prey's flight command and position to isolate strike/carcass intake from
/// the already tested looming escape. The seedporter uses its own live heuristic.
fn catch_case(on: bool, ahead_scale: f64, lift: f64, min_gain: f64) -> (Fauna, u64) {
    let world = noon(505);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[SP.index()].catch_bellwings = on;
    c.founders[SP.index()].catch_gain_min = min_gain;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let sp = introduce(&mut fauna, &world, SP, HUNTING);
    place(&mut fauna, sp, 3.0, LANE_Z, EAST);
    animal_mut(&mut fauna, sp).pose.y_m = SURFACE_M;
    animal_mut(&mut fauna, sp).eating = true;
    heuristic(&mut fauna, sp, SP);
    let phys = fauna.config.founder(SP);
    let h = phys.hunter.unwrap();
    let g = phys.body_at(animal(&fauna, sp).body);
    let ahead = ahead_scale
        * (h.jaw_forward_length_fraction + 0.5 * h.strike_reach_length_fraction)
        * g.length_m;
    let bw = bellwing(&mut fauna, &world, 3.0 + ahead, lift);
    for _ in 0..200 {
        if ahead_scale > 1.0 {
            // Hold the original distance: ordinary fruit-search wandering can otherwise
            // bring a distant insect into genuine catch reach during this small run.
            let heading = animal(&fauna, sp).pose.heading_rad;
            place(&mut fauna, sp, 3.0, LANE_Z, heading);
            animal_mut(&mut fauna, sp).pose.y_m = SURFACE_M;
        }
        if let Some(a) = fauna.animals.iter_mut().find(|a| a.id == bw) {
            a.pose.x = 3.0 + ahead;
            a.pose.z = LANE_Z;
            a.pose.y_m = SURFACE_M + lift;
            a.mobility.mode = if lift == 0.0 {
                MobilityMode::Perched
            } else {
                MobilityMode::Fly
            };
            a.mobility.falling = false;
        }
        fauna.step_with(&world, &mut flora, 1);
        if fauna.ledger.bites_by_founder_food[SP.index()][Food::Carrion.index()] > 0 {
            break;
        }
    }
    assert_closes(&fauna, "seedporter catch");
    (fauna, bw)
}

#[test]
fn a_seedporter_strikes_kills_and_eats_a_perched_or_low_flying_bellwing() {
    let ceiling = config().founder(SP).catch_ceiling_m;
    for lift in [0.0, 0.25 * ceiling] {
        let (fauna, bw) = catch_case(true, 1.0, lift, 0.0);
        assert!(
            fauna.ledger.strikes_by_founder[SP.index()] > 0,
            "catch at lift {lift}: hunter pose {:?}; prey pose {:?}",
            fauna
                .animals
                .iter()
                .find(|a| a.founder == Some(SP))
                .map(|a| a.pose),
            fauna.view().animal(bw).map(|a| a.pose)
        );
        assert!(
            fauna.ledger.hits_by_founder[SP.index()] > 0,
            "strike contacts its prey"
        );
        assert!(fauna.view().animal(bw).is_none(), "bellwing killed");
        assert!(
            fauna.ledger.bites_by_founder_food[SP.index()][Food::Carrion.index()] > 0,
            "the killed bellwing becomes an actual meal"
        );
    }
}

#[test]
fn seedporter_catch_rejects_distant_high_disabled_or_unprofitable_bellwings() {
    let ceiling = config().founder(SP).catch_ceiling_m;
    for (on, range, lift, gain) in [
        (true, 12.0, 0.0, 0.0),
        // Just above the configured ceiling, still inside this fixture's physical
        // jaw band: this checks the independent catch ceiling, not only geometry.
        (true, 1.0, ceiling + 0.025, 0.0),
        (false, 1.0, 0.0, 0.0),
        (true, 1.0, 0.0, 1.0),
    ] {
        let (fauna, bw) = catch_case(on, range, lift, gain);
        assert_eq!(
            fauna.ledger.strikes_by_founder[SP.index()],
            0,
            "no strike: switch {on}, range multiplier {range}, lift {lift}, gain floor {gain}"
        );
        assert!(fauna.view().animal(bw).is_some(), "rejected prey survives");
    }
}

#[test]
fn seedporter_opportunistic_catches_are_day_only_even_when_awake_and_feed_driven() {
    let clock = DayClock::default();
    let (rise, set) = clock.cycle_rise_set();
    let day_hour = 12.0 * (rise + set);
    let night_hour = (24.0 * set + 12.0 * (1.0 + rise - set)).rem_euclid(24.0);
    for custom_feed in [false, true] {
        for (hour, day) in [(day_hour, true), (night_hour, false)] {
            let world = plain_of_height(
                DayClock {
                    start_hour: hour,
                    ..clock
                },
                511,
                24,
            );
            let mut flora = Flora::new(FloraConfig::default());
            let mut c = config();
            c.founders[SP.index()].active = Activity::Always;
            c.founders[SP.index()].wake_hunger = 0.01;
            let mut fauna = Fauna::new(c);
            fauna.set_births_enabled(false);
            let sp = introduce(
                &mut fauna,
                &world,
                SP,
                StartingStores {
                    body: 1.0,
                    reserve: 0.2,
                },
            );
            place(&mut fauna, sp, 3.0, LANE_Z, EAST);
            animal_mut(&mut fauna, sp).pose.y_m = SURFACE_M;
            animal_mut(&mut fauna, sp).eating = true;
            if custom_feed {
                drive(&mut fauna, sp, 0.0, true);
            } else {
                heuristic(&mut fauna, sp, SP);
            }
            let phys = fauna.config.founder(SP);
            let h = phys.hunter.unwrap();
            let g = phys.body_at(animal(&fauna, sp).body);
            let x = 3.0
                + (h.jaw_forward_length_fraction + 0.5 * h.strike_reach_length_fraction)
                    * g.length_m;
            let bw = bellwing(&mut fauna, &world, x, 0.0);
            assert!(
                crate::is_prey_of(&fauna.config, &animal(&fauna, sp), &animal(&fauna, bw)),
                "fixture: an otherwise eligible prey in each arm"
            );
            for _ in 0..100 {
                if let Some(a) = fauna.animals.iter_mut().find(|a| a.id == bw) {
                    a.pose.x = x;
                    a.pose.z = LANE_Z;
                    a.pose.y_m = SURFACE_M;
                    a.mobility.mode = MobilityMode::Perched;
                    a.mobility.falling = false;
                }
                fauna.step_with(&world, &mut flora, 1);
            }
            assert_eq!(
                fauna.night.since_set_h.is_infinite(),
                day,
                "fixture: paired day and night"
            );
            assert_ne!(
                animal(&fauna, sp).state,
                crate::State::Asleep,
                "night refusal is exercised by an awake seedporter"
            );
            if day {
                assert!(
                    fauna.ledger.strikes_by_founder[SP.index()] > 0,
                    "day catch, custom feed {custom_feed}"
                );
                assert!(fauna.ledger.hits_by_founder[SP.index()] > 0);
                assert!(fauna.view().animal(bw).is_none(), "day prey is caught");
            } else {
                assert_eq!(
                    fauna.ledger.strikes_by_founder[SP.index()],
                    0,
                    "no night catch, custom feed {custom_feed}"
                );
                assert!(
                    fauna.view().animal(bw).is_some(),
                    "night prey remains alive"
                );
            }
            assert_closes(&fauna, "day-only opportunistic catch");
        }
    }
}

#[test]
fn a_bellwing_rising_above_the_catch_ceiling_during_windup_is_not_hit() {
    let world = noon(508);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[SP.index()].hunter.as_mut().unwrap().windup_s = 0.5;
    c.founders[SP.index()].hunter.as_mut().unwrap().recovery_s = 2.0;
    let ceiling = c.founder(SP).catch_ceiling_m;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let sp = introduce(&mut fauna, &world, SP, HUNTING);
    place(&mut fauna, sp, 3.0, LANE_Z, EAST);
    animal_mut(&mut fauna, sp).pose.y_m = SURFACE_M;
    animal_mut(&mut fauna, sp).eating = true;
    heuristic(&mut fauna, sp, SP);
    let phys = fauna.config.founder(SP);
    let h = phys.hunter.unwrap();
    let g = phys.body_at(animal(&fauna, sp).body);
    let high = ceiling + 0.025;
    assert!(
        high < h.jaw_band_height_fraction * g.height_m,
        "fixture: rising prey remains in the strike geometry, above only the catch ceiling"
    );
    let x =
        3.0 + (h.jaw_forward_length_fraction + 0.5 * h.strike_reach_length_fraction) * g.length_m;
    let bw = bellwing(&mut fauna, &world, x, 0.25 * ceiling);
    let mut raised = false;
    for _ in 0..50 {
        if phase(&fauna, sp) == Some(crate::StrikePhase::Windup) {
            raised = true;
        }
        let Some(a) = fauna.animals.iter_mut().find(|a| a.id == bw) else {
            break;
        };
        a.pose.x = x;
        a.pose.z = LANE_Z;
        a.pose.y_m = SURFACE_M + if raised { high } else { 0.25 * ceiling };
        a.mobility.mode = MobilityMode::Fly;
        a.mobility.falling = false;
        fauna.step_with(&world, &mut flora, 1);
    }
    assert!(
        raised && fauna.ledger.strikes_by_founder[SP.index()] > 0,
        "a live strike began while the bellwing was low"
    );
    assert_eq!(
        fauna.ledger.hits_by_founder[SP.index()],
        0,
        "catch ceiling is checked again at contact after prey rises"
    );
    assert_eq!(animal(&fauna, bw).wound, 0.0);
    assert_closes(&fauna, "rising bellwing");
}

#[test]
fn a_seedporter_can_strike_a_low_bellwing_from_its_crown_perch() {
    let world = noon(509);
    let mut fc = FloraConfig::default();
    fc.lanternberry.fruit_per_s = 0.0;
    let wood = fc.lanternberry.wood_max;
    let mut flora = Flora::new(fc);
    assert!(flora.apply(
        &world,
        FloraCommand::Seed {
            x: 20,
            z: 6,
            species: Plant::Lanternberry,
            wood,
        }
    ));
    let site = flora.view().stands[0].site;
    let top = (flora
        .view()
        .layers_at(site)
        .filter(|l| l.foliage_index.is_some())
        .map(|l| l.cell)
        .max()
        .unwrap()
        + 1) as f64
        * VOXEL_M;
    let mut fauna = Fauna::new(config());
    fauna.set_births_enabled(false);
    let x = 20.5 * VOXEL_M;
    let z = 6.5 * VOXEL_M;
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft {
            pose: Pose {
                x,
                z,
                heading_rad: EAST,
                y_m: top
            },
            founder: SP,
            stores: HUNTING,
        }
    ));
    let sp = fauna.animals.last().unwrap().id;
    drive(&mut fauna, sp, 0.0, false);
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(
        animal(&fauna, sp).mobility.mode,
        MobilityMode::Perched,
        "fixture: crown resolves a perch"
    );
    heuristic(&mut fauna, sp, SP);
    animal_mut(&mut fauna, sp).eating = true;
    let phys = fauna.config.founder(SP);
    let h = phys.hunter.unwrap();
    let g = phys.body_at(animal(&fauna, sp).body);
    let prey_x =
        x + (h.jaw_forward_length_fraction + 0.5 * h.strike_reach_length_fraction) * g.length_m;
    let prey_y = top + 0.25 * phys.catch_ceiling_m;
    let bw = bellwing(&mut fauna, &world, prey_x, prey_y - SURFACE_M);
    for _ in 0..40 {
        if let Some(a) = fauna.animals.iter_mut().find(|a| a.id == bw) {
            a.pose.x = prey_x;
            a.pose.z = z;
            a.pose.y_m = prey_y;
            a.mobility.mode = MobilityMode::Fly;
            a.mobility.falling = false;
        }
        fauna.step_with(&world, &mut flora, 1);
        if fauna.ledger.hits_by_founder[SP.index()] > 0 {
            break;
        }
    }
    assert!(
        fauna.ledger.strikes_by_founder[SP.index()] > 0,
        "crown hunter starts its catch"
    );
    assert!(
        fauna.ledger.hits_by_founder[SP.index()] > 0,
        "crown catch reaches the low flyer"
    );
    assert_eq!(
        animal(&fauna, sp).mobility.mode,
        MobilityMode::Perched,
        "strike preserves its perch"
    );
    assert_closes(&fauna, "crown perch catch");
}

#[test]
fn a_seedporter_above_ground_carrion_requests_descent_instead_of_feeding_through_air() {
    let world = noon(512);
    let mut fc = FloraConfig::default();
    fc.lanternberry.fruit_per_s = 0.0;
    let wood = fc.lanternberry.wood_max;
    let mut flora = Flora::new(fc);
    assert!(flora.apply(
        &world,
        FloraCommand::Seed {
            x: 20,
            z: 6,
            species: Plant::Lanternberry,
            wood,
        }
    ));
    let plant = flora.view().stands[0].site;
    let top = (flora
        .view()
        .layers_at(plant)
        .filter(|l| l.foliage_index.is_some())
        .map(|l| l.cell)
        .max()
        .unwrap()
        + 1) as f64
        * VOXEL_M;
    let mut c = config();
    c.hunt.meal_first = true;
    c.hunt.carcass_scan_m = 2.0;
    c.teachers.stalk = 0.5;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft {
            pose: Pose {
                x: 20.5 * VOXEL_M,
                z: 6.5 * VOXEL_M,
                heading_rad: EAST,
                y_m: top
            },
            founder: SP,
            stores: HUNTING,
        }
    ));
    let sp = fauna.animals.last().unwrap().id;
    drive(&mut fauna, sp, 0.0, false);
    fauna.step_with(&world, &mut flora, 1);
    animal_mut(&mut fauna, sp).eating = true;
    // The closest ground cell is directly beneath the perch (bearing zero). Align
    // with it so the descent command is a walk rather than a turn-before-walk.
    animal_mut(&mut fauna, sp).pose.heading_rad = 0.0;
    let a = animal(&fauna, sp);
    let g = fauna.config.founder(SP).body_at(a.body);
    assert_eq!(
        a.mobility.mode,
        MobilityMode::Perched,
        "fixture: resolved crown perch"
    );
    assert!(
        a.pose.y_m > SURFACE_M + g.mouth_ceiling_m,
        "fixture: carrion below mouth reach"
    );
    for (x, z) in crate::body::mouth_columns(&world.view(), &a.pose, &g) {
        let site = cubarium_voxel_flora::Site {
            x: x.rem_euclid(i64::from(world.view().config.width)) as u32,
            y: a.site.y,
            z,
        };
        assert!(flora.deposit(
            site,
            cubarium_voxel_flora::Deposit {
                kind: cubarium_voxel_flora::DepositKind::Carrion,
                organic: 0.1,
                mineral: 0.01,
                energy: 0.2,
            }
        ));
        assert!(flora.lay_carcass(
            site,
            cubarium_voxel_flora::Carcass {
                lineage: BW.index() as u8,
                heading_rad: EAST,
                organic: 0.1,
            }
        ));
    }
    assert!(
        crate::hunt::mouth_on_carcass(&flora.view(), &world.view(), &a, &g),
        "fixture: old horizontal check would report a meal at the mouth"
    );
    let i = fauna.animals.iter().position(|a| a.id == sp).unwrap();
    let act = crate::hunt::meal_step(&mut fauna, i, &world.view(), &flora.view())
        .expect("it knows the carrion underneath and requests a descent");
    assert!(
        act.forward > 0.0,
        "moves off its perch toward the ground meal: {act:?}"
    );
    assert_eq!(
        act.feed, 0.0,
        "cannot feed at ground carrion from the high crown"
    );
    assert_closes(&fauna, "crown carcass routing");
}

#[test]
fn catching_enabled_seedporter_still_eats_lanternberry_fruit() {
    let mut world = noon(506);
    let mut fc = FloraConfig::default();
    fc.lanternberry.fruit_per_s = 0.001;
    fc.lanternberry.fruit_cap = 0.02;
    fc.lanternberry.fruit_drop_s = 1e9;
    fc.lanternberry.propagule_rate = 0.0;
    let wood = fc.lanternberry.wood_max;
    let mut flora = Flora::new(fc);
    assert!(flora.apply(
        &world,
        FloraCommand::Seed {
            x: 20,
            z: 6,
            species: Plant::Lanternberry,
            wood
        }
    ));
    for _ in 0..60 {
        flora.step(&mut world);
    }
    let site = flora.view().stands[0].site;
    let crown_top = (flora
        .view()
        .layers_at(site)
        .filter(|l| l.foliage_index.is_some())
        .map(|l| l.cell)
        .max()
        .unwrap()
        + 1) as f64
        * VOXEL_M;
    assert!(
        flora.view().stand_at(site).unwrap().fruit > 0.0,
        "fixture has fruit"
    );
    let mut c = config();
    c.founders[SP.index()].catch_foray_m = 5.0;
    c.founders[SP.index()].catch_foray_min = 2.0;
    c.founders[SP.index()].catch_foray_s = 1.0;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft {
            pose: Pose {
                x: 20.5 * VOXEL_M,
                z: 6.5 * VOXEL_M,
                heading_rad: EAST,
                y_m: crown_top
            },
            founder: SP,
            stores: HUNTING,
        }
    ));
    let sp = fauna.animals.last().unwrap().id;
    fauna.step_with(&world, &mut flora, 1);
    drive(&mut fauna, sp, 0.0, true);
    for _ in 0..40 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert!(
        fauna.ledger.bites_by_founder_food[SP.index()][Food::Fruit.index()] > 0,
        "fruit remains an ordinary seedporter meal with catching enabled"
    );
    assert_closes(&fauna, "seedporter fruit");
}

// A walker can circle outside visit_m because its turn radius/body keeps its centre
// from reaching the light. Near-light inspection must still expire and mark the place.
#[test]
fn a_capgnawer_circling_a_day_lure_gives_up_and_resumes_foraging() {
    let world = noon(906);
    let flora = Flora::new(FloraConfig::default());
    let cap = Founder::Capgnawer;
    let mut c = config();
    c.light.visit_m = 0.2;
    c.light.near_m = 1.2;
    c.light.visit_s = 0.5;
    c.light.mark_s = 6.0;
    c.founders[cap.index()].senses.live = true;
    c.founders[cap.index()].senses.uv = false;
    c.founders[cap.index()].senses.light_m = 8.0;
    c.founders[cap.index()].senses.light_min = 0.00001;
    let mut fauna = Fauna::new(c);
    let lj = waiting_lanternjaw(&mut fauna, &world, 5.0);
    animal_mut(&mut fauna, lj).founder_state.lure = 1.0;
    let (lx, lz, _) = light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).unwrap();
    let id = introduce(&mut fauna, &world, cap, StartingStores::FULL);
    let d = (c.light.visit_m + c.light.near_m) / 2.0;
    place(&mut fauna, id, lx - d, lz, EAST);
    drive(&mut fauna, id, 0.0, false);
    animal_mut(&mut fauna, id).eating = true;
    fauna.beacons.fixed(&world.view(), &flora.view(), vec![]);
    let asked = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
    let steer = |fauna: &mut Fauna| {
        fauna.tick += 1;
        let i = fauna.animals.iter().position(|a| a.id == id).unwrap();
        light::steer_glow(fauna, i, &world.view(), &flora.view(), 1.0, asked, None)
    };
    assert!(steer(&mut fauna).is_some(), "initially drawn toward the lure");
    fauna.tick += cubarium_voxel::ticks_at_least_one(c.light.visit_s);
    assert!(steer(&mut fauna).is_none(), "a near approach with no bite expires, even outside visit_m");
    let ring = f64::from(world.view().config.width) * VOXEL_M;
    let p = fauna.percept(id).unwrap();
    assert!(p.marked(lx, lz, fauna.tick, c.light.mark_radius_m, ring));
    assert_eq!(fauna.ledger.failed_visits[cap.index()][BeaconKind::Lure.index()], 1);
    assert_eq!(p.expect, [0.0; light::CLASSES], "marking a place does not teach the UV/cyan expectation");
    for _ in 0..3 {
        assert!(steer(&mut fauna).is_none(), "the mark leaves the ordinary forager in control");
    }
    fauna.tick += cubarium_voxel::ticks_at_least_one(c.light.mark_s);
    assert!(steer(&mut fauna).is_some(), "the place can be tried again after its mark expires");
}

fn foray_scene(count: usize, lift: f64) -> (World, Flora, Fauna, u64) {
    let world = noon(907);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.hunt.reachable_only = false;
    c.founders[SP.index()].catch_foray_m = 5.0;
    c.founders[SP.index()].catch_foray_min = 2.0;
    c.founders[SP.index()].catch_foray_s = 1.0;
    let mut fauna = Fauna::new(c);
    let sp = introduce(&mut fauna, &world, SP, HUNTING);
    place(&mut fauna, sp, 3.0, LANE_Z, EAST);
    animal_mut(&mut fauna, sp).pose.y_m = SURFACE_M;
    animal_mut(&mut fauna, sp).eating = true;
    fauna.tick = 10;
    let tracks: Vec<_> = (0..count).map(|j| {
        let x = 5.0 + j as f64 * 0.1;
        let bw = bellwing(&mut fauna, &world, x, lift);
        crate::Track { known: true, id: bw, lineage: BW.index() as u8,
            modes: crate::perception::SIGHT, x, z: LANE_Z, y_m: SURFACE_M + lift,
            tick: fauna.tick, ..Default::default() }
    }).collect();
    let p = fauna.percepts.entry(sp).or_default();
    p.n = tracks.len() as u8;
    p.tracks[..tracks.len()].copy_from_slice(&tracks);
    (world, flora, fauna, sp)
}

#[test]
fn a_seedporter_forays_toward_a_sensed_low_bellwing_cluster_then_gives_up() {
    let (world, flora, mut fauna, sp) = foray_scene(2, 0.0);
    let i = fauna.animals.iter().position(|a| a.id == sp).unwrap();
    let go = crate::hunt::catch_step(&mut fauna, i, &world.view(), &flora.view()).expect("a nearby cluster invites a foray beyond strike reach");
    assert!(go.forward > 0.0 && go.turn.abs() < 1e-9, "walks toward the flowers' low visitors");
    let cadence_s = SP.manifest().cadence_ticks() as f64 / f64::from(crate::TICK_HZ);
    let budget = (fauna.config.founder(SP).catch_foray_s / cadence_s).ceil() as usize;
    for _ in 0..budget {
        let _ = crate::hunt::catch_step(&mut fauna, i, &world.view(), &flora.view());
    }
    assert!(crate::hunt::catch_step(&mut fauna, i, &world.view(), &flora.view()).is_none(), "a foray without a catch returns to fruit search");
}

#[test]
fn seedporter_forays_require_dense_seen_profitable_low_visitors() {
    let ceiling = config().founder(SP).catch_ceiling_m;
    for (count, lift, seen, enabled, gain) in [
        (1, 0.0, true, true, 0.0),
        (2, 2.0 * ceiling, true, true, 0.0),
        (2, 0.0, false, true, 0.0),
        (2, 0.0, true, false, 0.0),
        (2, 0.0, true, true, 1e9),
    ] {
        let (world, flora, mut fauna, sp) = foray_scene(count, lift);
        fauna.config.founders[SP.index()].catch_bellwings = enabled;
        fauna.config.founders[SP.index()].catch_gain_min = gain;
        if !seen { fauna.percepts.get_mut(&sp).unwrap().n = 0; }
        let i = fauna.animals.iter().position(|a| a.id == sp).unwrap();
        assert!(crate::hunt::catch_step(&mut fauna, i, &world.view(), &flora.view()).is_none());
    }
}

#[test]
fn a_seedporter_foray_uses_a_reachable_ground_route() {
    let (world, flora, mut fauna, sp) = foray_scene(2, 0.0);
    fauna.config.hunt.reachable_only = true;
    let i = fauna.animals.iter().position(|a| a.id == sp).unwrap();
    let go = crate::hunt::catch_step(&mut fauna, i, &world.view(), &flora.view()).expect("the low cluster has a reachable catch face");
    assert!(go.forward > 0.0);
}

#[test]
fn a_capgnawer_circling_outside_the_near_band_marks_a_failed_approach() {
    let world = noon(908);
    let flora = Flora::new(FloraConfig::default());
    let cap = Founder::Capgnawer;
    let mut c = config();
    c.light.visit_m = 0.1;
    c.light.near_m = 0.5;
    c.light.visit_s = 0.5;
    c.light.mark_s = 6.0;
    c.founders[cap.index()].senses.live = true;
    c.founders[cap.index()].senses.uv = false;
    c.founders[cap.index()].senses.light_m = 2.0;
    c.founders[cap.index()].senses.light_min = 0.00001;
    c.founders[cap.index()].cruise_body_lengths_per_s = 5.0;
    let manifest = c.manifest(cap);
    let search_s = c.light.visit_s + c.founder(cap).senses.light_m / manifest.cruise_m_per_s;
    let cadence_s = manifest.cadence_ticks() as f64 / f64::from(crate::TICK_HZ);
    let budget = (search_s / cadence_s).ceil() as usize;
    let mut fauna = Fauna::new(c);
    let lj = waiting_lanternjaw(&mut fauna, &world, 5.0);
    animal_mut(&mut fauna, lj).founder_state.lure = 1.0;
    let (lx, lz, _) = light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).unwrap();
    let id = introduce(&mut fauna, &world, cap, StartingStores::FULL);
    let d = (c.light.near_m + c.founder(cap).senses.light_m) / 2.0;
    place(&mut fauna, id, lx - d, lz, EAST);
    drive(&mut fauna, id, 0.0, false);
    animal_mut(&mut fauna, id).eating = true;
    fauna.beacons.fixed(&world.view(), &flora.view(), vec![]);
    let steer = |fauna: &mut Fauna| {
        fauna.tick += manifest.cadence_ticks();
        let i = fauna.animals.iter().position(|a| a.id == id).unwrap();
        light::steer_glow(fauna, i, &world.view(), &flora.view(), 1.0,
            Actions { forward: 1.0, turn: 0.0, feed: 0.0 }, None)
    };
    assert!(steer(&mut fauna).is_some(), "the beacon may initially draw it");
    for _ in 0..budget { let _ = steer(&mut fauna); }
    let ring = f64::from(world.view().config.width) * VOXEL_M;
    assert!(fauna.percept(id).unwrap().marked(lx, lz, fauna.tick, c.light.mark_radius_m, ring), "an orbit cannot renew an unrewarded distant approach forever");
    assert!(steer(&mut fauna).is_none(), "the mark returns control to its forager");
    assert_eq!(fauna.ledger.stalled_visits[cap.index()], 1);
}
