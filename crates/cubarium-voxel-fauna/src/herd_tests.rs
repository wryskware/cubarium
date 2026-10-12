//! HERD mechanism checks, authored before the implementation. Every scene sets its
//! own response knobs; changing shipped ecological values must not break these tests.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use crate::{
    Actions, Activity, Fauna, FaunaConfig, Founder, Hunker, SenseTable, StartingStores, State,
    Vigil, VigilMode, Vigilance,
};
use cubarium_voxel::{Command as WorldCommand, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig};
use fixture::*;

const GRAZER: Founder = Founder::Browser;
const VH: Founder = Founder::Chorister;
const CROP: Actions = Actions {
    forward: 0.0,
    turn: 0.0,
    feed: 1.0,
};

fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
        c.founders[f.index()].vigilance = Vigilance::default();
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let grazer = &mut c.founders[GRAZER.index()];
    grazer.senses = SenseTable {
        live: true,
        sight_m: 6.0,
        sight_night_m: 6.0,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: 180.0,
        kin_tracks: 4,
        ..SenseTable::default()
    };
    grazer.vigilance = Vigilance {
        mode: VigilMode::Herd,
        flee_start_m: 4.0,
        flee_stop_m: 5.0,
        calm_s: 0.5,
        lost_calm_s: 1.0,
        group_min: 2,
        watch_calm: false,
        orient_s: 0.0,
        run_noise: 1.0,
        alarm_bolt_s: 1.0,
        alarm_refractory_s: 2.0,
        ..Vigilance::default()
    };
    grazer.hunker = Hunker {
        enabled: true,
        herd_enabled: true,
        enter_m: 1.0,
        leave_m: 3.0,
        calm_s: 0.5,
        hunger_calm_exp: 0.0,
        min_condition: 0.0,
        ring_m: 0.8,
        run_start_m: 4.0,
        catch_m: 0.0,
        escape_probe_m: 0.7,
        shelter_seek_m: 3.0,
        shelter_margin_m: 0.1,
        shelter_exposure: 0.25,
        ..crate::vigil::hunker_of(GRAZER)
    };
    c
}

fn body(f: &mut Fauna, w: &World, kind: Founder, x: f64, z: f64, heading: f64, adult: bool) -> u64 {
    let id = introduce(f, w, kind, StartingStores::FULL);
    let mass = f.config.founder(kind).core.birth_body;
    place(f, id, x, z, heading);
    drive(f, id, 0.0, false);
    let a = animal_mut(f, id);
    a.body = if adult { mass } else { 0.4 * mass };
    a.state = State::Cropping;
    a.speed_m_s = 0.0;
    a.still_ticks = u64::MAX / 2;
    id
}

fn hunter(f: &mut Fauna, w: &World, x: f64, z: f64) -> u64 {
    let id = body(f, w, VH, x, z, WEST, true);
    let cruise = f.config.manifest(VH).cruise_m_per_s;
    let a = animal_mut(f, id);
    a.speed_m_s = cruise;
    a.still_ticks = 0;
    a.state = State::Walking;
    id
}

fn sense(f: &mut Fauna, w: &World) {
    f.tick += 1;
    crate::perception::sense_pass(f, &w.view(), false, 1.0, 1);
}

fn respond(f: &mut Fauna, w: &World, flora: &Flora, id: u64) -> Actions {
    let i = f.animals.iter().position(|a| a.id == id).expect("alive");
    crate::perception::refresh(
        f,
        i,
        &w.view(),
        &flora.view(),
        None,
        crate::perception::Want {
            tremor: true,
            ..Default::default()
        },
    );
    crate::vigil::respond(f, i, &w.view(), &flora.view(), CROP).unwrap_or(CROP)
}

fn vigil(f: &Fauna, id: u64) -> Vigil {
    f.percept(id).map(|p| p.vigil).unwrap_or_default()
}

fn hunker(f: &mut Fauna, id: u64) {
    animal_mut(f, id).hunkered = true;
    let now = f.tick;
    let v = &mut f.percepts.entry(id).or_default().vigil;
    v.hunkered = true;
    v.hunker_close = now;
}

fn ring(w: &World) -> f64 {
    f64::from(w.view().config.width) * w.view().config.voxel_m
}

fn distance(f: &Fauna, w: &World, a: u64, b: u64) -> f64 {
    let (a, b) = (animal(f, a), animal(f, b));
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, b.pose.x, b.pose.z, ring(w));
    dx.hypot(dz)
}

fn herd(w: &World) -> (Fauna, [u64; 2], [u64; 2]) {
    let mut f = Fauna::new(config());
    f.set_births_enabled(false);
    let grazers = [
        body(&mut f, w, GRAZER, 3.0, 1.2, WEST, true),
        body(&mut f, w, GRAZER, 3.0, 1.8, WEST, true),
    ];
    let hk = f.config.founder(GRAZER).hunker;
    let vg = f.config.founder(GRAZER).vigilance;
    let threat_d = (hk.enter_m + vg.flee_start_m) / 2.0;
    let pack = [
        hunter(&mut f, w, 3.0 + threat_d, 1.2),
        hunter(&mut f, w, 3.0 + threat_d, 1.8),
    ];
    (f, grazers, pack)
}

#[test]
fn a_herd_runs_from_a_pack_beyond_hunker_range_then_hunkers_when_it_closes() {
    let w = clock_off(301);
    let flora = Flora::new(FloraConfig::default());
    let (mut f, grazers, pack) = herd(&w);
    sense(&mut f, &w);
    for id in grazers {
        let act = respond(&mut f, &w, &flora, id);
        assert!(
            vigil(&f, id).fleeing && !animal(&f, id).hunkered,
            "a known pressing pack beyond enter_m sends the herd running"
        );
        assert!(
            act.forward > 0.0 && act.feed == 0.0,
            "running moves and stops feeding"
        );
    }
    let enter = f.config.founder(GRAZER).hunker.enter_m;
    for (id, grazer) in pack.into_iter().zip(grazers) {
        let z = animal(&f, grazer).pose.z;
        place(&mut f, id, 3.0 + 0.5 * enter, z, WEST);
    }
    sense(&mut f, &w);
    for id in grazers {
        let act = respond(&mut f, &w, &flora, id);
        assert!(
            animal(&f, id).hunkered && vigil(&f, id).hunkered && !vigil(&f, id).fleeing,
            "contact inside enter_m replaces running with a hunker"
        );
        assert_eq!((act.forward, act.feed), (0.0, 0.0));
    }
}

#[test]
fn a_cornered_herd_hunkers_before_the_pack_enters_hunker_range() {
    let mut w = clock_off(302);
    let flora = Flora::new(FloraConfig::default());
    let (mut f, grazers, _) = herd(&w);
    let a = animal(&f, grazers[0]);
    let phys = *f.config.founder(GRAZER);
    let geometry = phys.body_at(a.body);
    let vm = w.view().config.voxel_m;
    let wall_x = ((a.pose.x - geometry.footprint_radius()) / vm).floor() as i64 - 1;
    for z in 0..w.view().config.depth {
        for y in (SURFACE_Y + 1)..w.view().config.height {
            w.apply(WorldCommand::SetMaterial {
                x: wall_x,
                y,
                z,
                material: Material::Soil,
            });
        }
    }
    let probe = phys.hunker.escape_probe_m;
    assert!(
        crate::body::motor_walk(
            &w.view(),
            (a.pose.x, a.pose.z),
            (a.pose.x - probe, a.pose.z),
            a.site.y,
            geometry.footprint_radius(),
            phys.wade_depth_m(&geometry),
            geometry.headroom_voxels(vm),
            phys.step_limits(vm)
        )
        .is_none(),
        "fixture: the intended escape route is blocked by terrain"
    );
    sense(&mut f, &w);
    for id in grazers {
        let act = respond(&mut f, &w, &flora, id);
        assert!(
            animal(&f, id).hunkered,
            "blocked escape calls for the shield before contact"
        );
        assert_eq!((act.forward, act.feed), (0.0, 0.0));
    }
}

#[test]
fn a_faster_close_pack_catches_an_adult_but_a_distant_pack_does_not() {
    let w = clock_off(303);
    let flora = Flora::new(FloraConfig::default());
    let (mut f, grazers, pack) = herd(&w);
    let mut c = f.config.clone();
    c.founders[GRAZER.index()].hunker.catch_m =
        (c.founder(GRAZER).hunker.enter_m + c.founder(GRAZER).vigilance.flee_start_m) / 2.0;
    c.founders[VH.index()].cruise_body_lengths_per_s =
        4.0 * c.founder(GRAZER).cruise_body_lengths_per_s;
    f.replace_config(c).unwrap();
    let hk = f.config.founder(GRAZER).hunker;
    let distant = (hk.catch_m + hk.run_start_m) / 2.0;
    for (id, g) in pack.into_iter().zip(grazers) {
        let z = animal(&f, g).pose.z;
        place(&mut f, id, 3.0 + distant, z, WEST);
    }
    sense(&mut f, &w);
    for id in grazers {
        respond(&mut f, &w, &flora, id);
        assert!(
            vigil(&f, id).fleeing && !animal(&f, id).hunkered,
            "speed alone does not catch a herd outside catch_m"
        );
    }
    let d = (hk.catch_m + hk.enter_m) / 2.0;
    for (id, g) in pack.into_iter().zip(grazers) {
        let z = animal(&f, g).pose.z;
        place(&mut f, id, 3.0 + d, z, WEST);
    }
    sense(&mut f, &w);
    for id in grazers {
        respond(&mut f, &w, &flora, id);
        assert!(
            animal(&f, id).hunkered,
            "a faster known pack inside catch_m makes further escape unprofitable"
        );
    }
}

#[test]
fn a_juvenile_in_reach_moves_toward_a_hunkered_adult_without_hunkering() {
    let w = clock_off(304);
    let flora = Flora::new(FloraConfig::default());
    let mut f = Fauna::new(config());
    f.set_births_enabled(false);
    let adult = body(&mut f, &w, GRAZER, 4.5, LANE_Z, EAST, true);
    hunker(&mut f, adult);
    let calf = body(&mut f, &w, GRAZER, 3.2, LANE_Z, EAST, false);
    hunter(&mut f, &w, 2.8, LANE_Z);
    let before = distance(&f, &w, calf, adult);
    assert!(before < f.config.founder(GRAZER).hunker.shelter_seek_m);
    sense(&mut f, &w);
    let act = respond(&mut f, &w, &flora, calf);
    assert!(
        !animal(&f, calf).hunkered && act.forward > 0.0 && act.feed == 0.0,
        "a calf runs into the bunch instead of offering its soft shield"
    );
    let a = animal(&f, calf);
    let phys = *f.config.founder(GRAZER);
    let geometry = phys.body_at(a.body);
    let mut pose = a.pose;
    let mut standing_y = a.site.y;
    crate::body::resolve_motion(
        &w.view(),
        &mut pose,
        &mut standing_y,
        &f.config.manifest(GRAZER),
        &geometry,
        phys.wade_depth_m(&geometry),
        phys.step_limits(VOXEL_M),
        f.config.life.wall_friction,
        act,
    );
    animal_mut(&mut f, calf).pose = pose;
    assert!(
        distance(&f, &w, calf, adult) < before,
        "the chosen action actually closes the adult gap"
    );
}

#[test]
fn a_lone_juvenile_runs_even_inside_the_adult_hunker_range() {
    let w = clock_off(305);
    let flora = Flora::new(FloraConfig::default());
    let mut f = Fauna::new(config());
    let calf = body(&mut f, &w, GRAZER, 3.0, LANE_Z, WEST, false);
    let enter = f.config.founder(GRAZER).hunker.enter_m;
    hunter(&mut f, &w, 3.0 + 0.5 * enter, LANE_Z);
    sense(&mut f, &w);
    let act = respond(&mut f, &w, &flora, calf);
    assert!(
        vigil(&f, calf).fleeing && !vigil(&f, calf).hunkered && !animal(&f, calf).hunkered,
        "without a reachable adult a juvenile keeps running"
    );
    assert!(act.forward > 0.0 && act.feed == 0.0);
}

fn cover_scene(exposure: f64) -> (World, Fauna, u64, u64, u64) {
    let w = clock_off(306);
    let mut c = config();
    c.founders[GRAZER.index()].hunker.shelter_exposure = exposure;
    let mut f = Fauna::new(c);
    let calf = body(
        &mut f,
        &w,
        GRAZER,
        STRIKER_X + IN_CAPSULE_M,
        LANE_Z,
        WEST,
        false,
    );
    let adult = body(&mut f, &w, GRAZER, 4.0, LANE_Z, WEST, true);
    let gap = 0.75
        * (crate::body_of(&f.config, &animal(&f, adult))
            .unwrap()
            .footprint_radius()
            + crate::body_of(&f.config, &animal(&f, calf))
                .unwrap()
                .footprint_radius());
    place(&mut f, adult, STRIKER_X + IN_CAPSULE_M - gap, LANE_Z, WEST);
    hunker(&mut f, adult);
    let striker = body(
        &mut f,
        &w,
        Founder::Lanternjaw,
        STRIKER_X,
        LANE_Z,
        EAST,
        true,
    );
    (w, f, calf, adult, striker)
}

#[test]
fn physical_cover_reduces_juvenile_strike_exposure_by_the_configured_knob() {
    for exposure in [0.0, 0.25, 1.0] {
        let (w, f, calf, _, striker) = cover_scene(exposure);
        let calf = animal(&f, calf);
        assert!(crate::vigil::sheltered(&f, &calf, ring(&w), VOXEL_M));
        assert_eq!(
            crate::vigil::shelter_exposure(&f, &calf, &animal(&f, striker), ring(&w), VOXEL_M),
            f.config.founder(GRAZER).hunker.shelter_exposure
        );
    }
}

#[test]
fn shelter_requires_a_closed_adult_at_the_juveniles_height() {
    let (w, mut f, calf, adult, striker) = cover_scene(0.25);
    let prey = animal(&f, calf);
    let assert_exposed = |f: &Fauna| {
        assert!(!crate::vigil::sheltered(f, &prey, ring(&w), VOXEL_M));
        assert_eq!(
            crate::vigil::shelter_exposure(f, &prey, &animal(f, striker), ring(&w), VOXEL_M),
            1.0
        );
    };
    let height_gap = (f.config.founder(GRAZER).hunker.shelter_margin_m / VOXEL_M).ceil() as u32 + 2;
    animal_mut(&mut f, adult).site.y += height_gap;
    assert_exposed(&f);
    animal_mut(&mut f, adult).site.y = prey.site.y;
    let open_until = f.tick + 10;
    f.percepts.entry(adult).or_default().vigil.opened_until = open_until;
    assert_exposed(&f);
    f.percepts.entry(adult).or_default().vigil.opened_until = 0;
    let soft_body = 0.5 * f.config.founder(GRAZER).core.birth_body;
    animal_mut(&mut f, adult).body = soft_body;
    assert_exposed(&f);
}

#[test]
fn an_adult_on_the_wrong_side_cannot_block_a_strike() {
    let (w, mut f, calf, adult, striker) = cover_scene(0.0);
    let p = animal(&f, calf);
    let d = distance(&f, &w, calf, adult);
    place(&mut f, adult, p.pose.x + d, p.pose.z, WEST);
    assert!(
        crate::vigil::sheltered(&f, &p, ring(&w), VOXEL_M),
        "the calf remains beside an adult"
    );
    assert_eq!(
        crate::vigil::shelter_exposure(&f, &p, &animal(&f, striker), ring(&w), VOXEL_M),
        1.0,
        "an adult behind the calf is outside the striker-to-calf segment"
    );
}

#[test]
fn a_real_lunge_uses_the_shelter_knob_at_contact() {
    let harm = |exposure| {
        let (w, mut f, calf, adult, striker) = cover_scene(exposure);
        f.set_births_enabled(false);
        let mut c = f.config.clone();
        c.founders[GRAZER.index()].vigilance.mode = VigilMode::None;
        c.founders[GRAZER.index()].senses.live = false;
        c.founders[GRAZER.index()].agility = 0.0;
        c.founders[GRAZER.index()].armour = 0.0;
        f.replace_config(c).unwrap();
        let a = animal(&f, striker);
        let geometry = crate::body_of(&f.config, &a).unwrap();
        let h = f.config.founder(Founder::Lanternjaw).hunter.unwrap();
        // This injects the last tick of a lunge, so place the calf in that tick's jaw
        // sweep, not at the reach of a complete lunge on a larger starter body.
        let jaw_offset = geometry.length_m * h.jaw_forward_length_fraction;
        let lunge_step = geometry.length_m * h.strike_reach_length_fraction
            / f64::from(crate::Hunter::ticks(h.lunge_s));
        let prey_x = a.pose.x + jaw_offset + 0.5 * lunge_step;
        let cover_gap = distance(&f, &w, calf, adult);
        place(&mut f, calf, prey_x, a.pose.z, WEST);
        place(&mut f, adult, prey_x - cover_gap, a.pose.z, WEST);
        assert!(
            crate::vigil::sheltered(&f, &animal(&f, calf), ring(&w), VOXEL_M),
            "fixture: the adult still covers the relocated calf"
        );
        animal_mut(&mut f, striker).strike = Some(crate::Strike {
            phase: crate::StrikePhase::Lunge,
            ticks_left: 1,
            prey: calf,
            from_x: a.pose.x,
            from_z: a.pose.z,
            lure: false,
        });
        let mut flora = Flora::new(FloraConfig::default());
        f.step_with(&w, &mut flora, 1);
        assert_eq!(
            phase(&f, striker),
            Some(crate::StrikePhase::Recovery),
            "the lunge resolved"
        );
        if exposure == 1.0 {
            assert!(
                hits(&f) > 0,
                "fixture: an exposed calf receives a real contact hit"
            );
        }
        f.view().animal(calf).map_or(1.0, |a| a.wound)
    };
    let (covered, exposed) = (harm(0.0), harm(1.0));
    assert!(
        covered < exposed,
        "actual contact must apply physical cover: {covered} < {exposed}"
    );
}

#[test]
fn a_herd_unhunkers_after_the_pack_leaves_and_does_not_hold_a_lost_flight_forever() {
    let w = clock_off(307);
    let flora = Flora::new(FloraConfig::default());
    let (mut f, grazers, pack) = herd(&w);
    let enter = f.config.founder(GRAZER).hunker.enter_m;
    for (id, g) in pack.into_iter().zip(grazers) {
        let z = animal(&f, g).pose.z;
        place(&mut f, id, 3.0 + 0.5 * enter, z, WEST);
    }
    sense(&mut f, &w);
    for id in grazers {
        respond(&mut f, &w, &flora, id);
        assert!(animal(&f, id).hunkered);
    }
    for id in pack {
        f.animals.retain(|a| a.id != id);
    }
    let c = f.config.founder(GRAZER);
    let horizon = cubarium_voxel::ticks_at_least_one(c.hunker.calm_s + c.vigilance.lost_calm_s)
        + 2 * f.config.perception.stagger_ticks()
        + 2;
    for _ in 0..horizon {
        sense(&mut f, &w);
        for id in grazers {
            respond(&mut f, &w, &flora, id);
        }
    }
    for id in grazers {
        assert!(
            !animal(&f, id).hunkered && !vigil(&f, id).hunkered && !vigil(&f, id).fleeing,
            "calm and lost-threat timers let the herd resume after the pack goes"
        );
    }
}

#[test]
fn an_alarm_beside_kin_still_runs_along_the_runners_escape_heading() {
    let w = clock_off(308);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    let ring_m = c.founder(GRAZER).hunker.ring_m;
    c.founders[GRAZER.index()].senses = SenseTable {
        live: true,
        hearing_m: 4.0,
        hearing_min: 1e-9,
        kin_tracks: 4,
        ..SenseTable::default()
    };
    c.founders[GRAZER.index()].vigilance.alarm_emission_factor = 2.0;
    c.founders[GRAZER.index()].vigilance.run_noise = 3.0;
    let mut f = Fauna::new(c);
    let listener = body(&mut f, &w, GRAZER, 3.0, LANE_Z, WEST, true);
    let runner = body(&mut f, &w, GRAZER, 3.0 + 0.5 * ring_m, LANE_Z, WEST, true);
    let speed = f.config.manifest(GRAZER).cruise_m_per_s;
    animal_mut(&mut f, runner).speed_m_s = speed;
    animal_mut(&mut f, runner).still_ticks = 0;
    f.percepts.entry(runner).or_default().vigil.bolt_until = u64::MAX / 2;
    sense(&mut f, &w);
    let act = respond(&mut f, &w, &flora, listener);
    let v = vigil(&f, listener);
    assert!(
        v.bolt_until > f.tick && act.forward > 0.0 && !animal(&f, listener).hunkered,
        "a heard escape keeps moving even though kin are already beside the listener"
    );
    let off = (v.bolt_heading - WEST).rem_euclid(std::f64::consts::TAU);
    assert!(
        off.min(std::f64::consts::TAU - off) < 1e-6,
        "the listener follows the runner west, rather than bunching east toward it"
    );
}

#[test]
fn a_herd_flight_ends_after_the_pack_goes_unseen_for_its_lost_calm() {
    let w = clock_off(309);
    let flora = Flora::new(FloraConfig::default());
    let (mut f, grazers, pack) = herd(&w);
    sense(&mut f, &w);
    for id in grazers {
        respond(&mut f, &w, &flora, id);
        assert!(vigil(&f, id).fleeing, "fixture: the herd starts a flight");
    }
    for id in pack {
        f.animals.retain(|a| a.id != id);
    }
    let vg = f.config.founder(GRAZER).vigilance;
    let horizon = cubarium_voxel::ticks_at_least_one(vg.lost_calm_s + vg.calm_s)
        + 2 * f.config.perception.stagger_ticks()
        + 2;
    for _ in 0..horizon {
        sense(&mut f, &w);
        for id in grazers {
            respond(&mut f, &w, &flora, id);
        }
    }
    for id in grazers {
        assert!(
            !vigil(&f, id).fleeing && !animal(&f, id).hunkered,
            "FEAR-2 ends a herd flight even within flee_stop_m of the last sighting"
        );
    }
}

#[test]
fn idle_hunters_do_no_pry_work_one_hungry_hunter_cannot_open_and_a_hungry_pack_can() {
    let w = clock_off(310);
    let mut c = config();
    c.founders[GRAZER.index()].hunker.pry_per_member_s = 1.0;
    c.founders[GRAZER.index()].hunker.resist_per_s = 1.5;
    c.founders[GRAZER.index()].hunker.ring_resist_per_kin = 0.0;
    let hk = c.founder(GRAZER).hunker;
    let enough = (hk.resist_per_s / hk.pry_per_member_s).floor() as usize + 1;
    let net = enough as f64 * hk.pry_per_member_s - hk.resist_per_s;
    let bound = cubarium_voxel::ticks_at_least_one(1.0 / net) + 1;
    let mut f = Fauna::new(c);
    let grazer = body(&mut f, &w, GRAZER, 3.0, LANE_Z, WEST, true);
    hunker(&mut f, grazer);
    let mut pack = Vec::new();
    for n in 0..enough {
        let theta = n as f64 * std::f64::consts::TAU / enough as f64;
        let id = hunter(
            &mut f,
            &w,
            3.0 + 0.25 * theta.cos(),
            LANE_Z + 0.25 * theta.sin(),
        );
        animal_mut(&mut f, id).eating = false;
        animal_mut(&mut f, id).emergency = false;
        pack.push(id);
    }
    for _ in 0..bound {
        f.tick += 1;
        crate::vigil::pry(&mut f, ring(&w));
    }
    assert_eq!(
        vigil(&f, grazer).pry,
        0.0,
        "sated or idle bodies contribute no handling effort"
    );
    assert!(!crate::vigil::opened(&f, grazer));
    assert_eq!(f.ledger.vigil.worker_ticks[GRAZER.index()], 0);
    animal_mut(&mut f, pack[0]).eating = true;
    for _ in 0..bound {
        f.tick += 1;
        crate::vigil::pry(&mut f, ring(&w));
    }
    assert_eq!(
        vigil(&f, grazer).pry,
        0.0,
        "one hungry hunter loses to configured resistance"
    );
    assert!(!crate::vigil::opened(&f, grazer));
    for id in pack {
        animal_mut(&mut f, id).eating = true;
    }
    let mut opened = false;
    for _ in 0..bound {
        f.tick += 1;
        crate::vigil::pry(&mut f, ring(&w));
        if crate::vigil::opened(&f, grazer) {
            opened = true;
            break;
        }
    }
    assert!(
        opened,
        "enough hungry contact members still open an adult by the configured net-work bound"
    );
}

#[test]
fn a_hungry_pack_only_pries_the_grazer_it_is_actually_targeting() {
    let w = clock_off(311);
    let mut c = config();
    c.founders[GRAZER.index()].hunker.pry_per_member_s = 1.0;
    c.founders[GRAZER.index()].hunker.resist_per_s = 1.5;
    c.founders[GRAZER.index()].hunker.ring_resist_per_kin = 0.0;
    let hk = c.founder(GRAZER).hunker;
    let enough = (hk.resist_per_s / hk.pry_per_member_s).floor() as usize + 1;
    let net = enough as f64 * hk.pry_per_member_s - hk.resist_per_s;
    let bound = cubarium_voxel::ticks_at_least_one(1.0 / net) + 1;
    let mut f = Fauna::new(c);
    let grazer = body(&mut f, &w, GRAZER, 3.0, LANE_Z, WEST, true);
    hunker(&mut f, grazer);
    let other_prey = body(&mut f, &w, Founder::Loftstrider, 9.0, LANE_Z, WEST, true);
    let pack_id = 17;
    let mut members = Vec::new();
    for n in 0..enough {
        let theta = n as f64 * std::f64::consts::TAU / enough as f64;
        let id = hunter(
            &mut f,
            &w,
            3.0 + 0.25 * theta.cos(),
            LANE_Z + 0.25 * theta.sin(),
        );
        let a = animal_mut(&mut f, id);
        a.eating = true;
        a.emergency = false;
        a.pack = Some(crate::PackMember {
            pack: pack_id,
            pup: false,
            calling_ticks: 0,
        });
        members.push(id);
    }
    f.packs.push(crate::PackRecord {
        id: pack_id,
        lineage: VH,
        den: animal(&f, grazer).site,
        breeder: Some(members[0]),
        den_choice: None,
        last_litter_tick: None,
        target: None,
        chase_since_tick: 0,
        chase_progress_tick: 0,
        chase_best_m: f64::INFINITY,
        chase_work: 0.0,
        target_site: None,
        feast: None,
        feast_eaten: 0.0,
        abandoned: None,
        shunned: [(0, 0); 4],
        drawn: None,
        drawn_scent: false,
        season: Default::default(),
    });
    // VH-TACTICS (§11.6): each hound's own attack state, its tactics memory's target.
    let aim = |f: &mut Fauna, target: Option<u64>| {
        f.pack_mut(pack_id).unwrap().target = target;
        for &id in &members {
            f.hounds.entry(id).or_default().target = target;
        }
    };
    for target in [None, Some(other_prey)] {
        aim(&mut f, target);
        for _ in 0..bound {
            f.tick += 1;
            crate::vigil::pry(&mut f, ring(&w));
        }
        assert_eq!(
            vigil(&f, grazer).pry,
            0.0,
            "a hungry contact pack targeting {target:?} does no work on this grazer"
        );
        assert!(
            !crate::vigil::opened(&f, grazer),
            "proximity alone must not open an unrelated grazer"
        );
        assert_eq!(
            f.ledger.vigil.worker_ticks[GRAZER.index()],
            0,
            "only hunters working this target count as pry workers"
        );
    }
    aim(&mut f, Some(grazer));
    let mut opened = false;
    for _ in 0..bound {
        f.tick += 1;
        crate::vigil::pry(&mut f, ring(&w));
        if crate::vigil::opened(&f, grazer) {
            opened = true;
            break;
        }
    }
    assert!(
        opened,
        "the same hungry full pack still opens its selected grazer by the configured bound"
    );
}
