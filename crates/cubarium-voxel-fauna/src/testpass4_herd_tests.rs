//! **TEST-PASS-4 (independent), HERD**: the run at exactly `run_start_m` and not a hair
//! beyond; the switch off restoring HUNKER's response (no run; a calf hunkers inside
//! `enter_m`); the catch at exactly `catch_m`, never by a slower hunter, and none at
//! `catch_m = 0`; no cornering at `escape_probe_m = 0`; a run ending on a ledge's drop;
//! the calf's seek at exactly `shelter_seek_m`; shelter gone with the switch off; a
//! sheltered calf's flight readout; the pry with sated and hungry hunters side by side,
//! the emergency counting as hunger, and pups never prying; FEAR-2 ending a run whose
//! hunter is seen still before its lost calm; the run-to-hunker readout; and a postcard
//! round trip of the new config and readouts.
//!
//! Every scene sets its own response knobs; no shipped number is asserted.

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
const CROP: Actions = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };

/// One grazer reads one pressing hunter: `group_min` 1, enter 1 m, run 5 m, flee 4 m, stop
/// 6 m; catch and the escape probe off unless a case turns them on.
fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
        c.founders[f.index()].vigilance = Vigilance::default();
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let g = &mut c.founders[GRAZER.index()];
    g.senses = SenseTable {
        live: true,
        sight_m: 7.0,
        sight_night_m: 7.0,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: 180.0,
        kin_tracks: 4,
        ..SenseTable::default()
    };
    g.vigilance = Vigilance {
        mode: VigilMode::Herd,
        flee_start_m: 4.0,
        flee_stop_m: 6.0,
        calm_s: 0.5,
        lost_calm_s: 1000.0,
        group_min: 1,
        watch_calm: false,
        orient_s: 0.0,
        run_noise: 1.0,
        alarm_bolt_s: 1.0,
        alarm_refractory_s: 2.0,
        ..Vigilance::default()
    };
    g.hunker = Hunker {
        enabled: true,
        herd_enabled: true,
        enter_m: 1.0,
        leave_m: 3.0,
        calm_s: 0.5,
        hunger_calm_exp: 0.0,
        min_condition: 0.0,
        ring_m: 0.8,
        run_start_m: 5.0,
        catch_m: 0.0,
        escape_probe_m: 0.0,
        shelter_seek_m: 2.5,
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
    crate::perception::refresh(f, i, &w.view(), &flora.view(), None, crate::perception::Want { tremor: true, ..Default::default() });
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

const GX: f64 = 3.0;

/// One grazer at `GX` on the lane facing west, one moving hunter `d` metres east of it.
fn pair(c: FaunaConfig, d: f64, adult: bool) -> (World, Flora, Fauna, u64, u64) {
    let w = clock_off(401);
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let g = body(&mut f, &w, GRAZER, GX, LANE_Z, EAST, adult);
    let h = hunter(&mut f, &w, GX + d, LANE_Z);
    (w, Flora::new(FloraConfig::default()), f, g, h)
}

fn run_once(c: FaunaConfig, d: f64, adult: bool) -> (Vigil, bool, Actions) {
    let (w, flora, mut f, g, _) = pair(c, d, adult);
    sense(&mut f, &w);
    let act = respond(&mut f, &w, &flora, g);
    (vigil(&f, g), animal(&f, g).hunkered, act)
}

#[test]
fn a_herd_run_starts_at_exactly_run_start_and_not_a_hair_beyond() {
    let c = config();
    let r = c.founder(GRAZER).hunker.run_start_m;
    let (v, hunkered, act) = run_once(c.clone(), r, true);
    assert!(v.fleeing && !hunkered && act.feed == 0.0, "at run_start_m it runs: {v:?}");
    let (v, hunkered, _) = run_once(c, r + 1e-9, true);
    assert!(!v.fleeing && !hunkered, "beyond run_start_m it does not: {v:?}");
}

#[test]
fn with_the_herd_switch_off_a_pressing_hunter_beyond_enter_starts_no_run() {
    let mut c = config();
    let hk = c.founder(GRAZER).hunker;
    let vg = c.founder(GRAZER).vigilance;
    // Beyond enter_m: inside flee_start_m, then between flee_start_m and run_start_m.
    for d in [0.5 * (hk.enter_m + vg.flee_start_m), 0.5 * (vg.flee_start_m + hk.run_start_m)] {
        c.founders[GRAZER.index()].hunker.herd_enabled = true;
        let (v, _, _) = run_once(c.clone(), d, true);
        assert!(v.fleeing, "fixture: with HERD on it runs at {d} m");
        c.founders[GRAZER.index()].hunker.herd_enabled = false;
        let (v, hunkered, _) = run_once(c.clone(), d, true);
        assert!(!v.fleeing && !hunkered, "HUNKER's response: a lone grazer does not run at {d} m: {v:?}");
    }
}

#[test]
fn with_the_herd_switch_off_a_calf_inside_enter_hunkers_as_before() {
    let mut c = config();
    let d = 0.5 * c.founder(GRAZER).hunker.enter_m;
    let (v, hunkered, _) = run_once(c.clone(), d, false);
    assert!(!hunkered && v.fleeing, "HERD: a lone calf runs inside enter_m");
    c.founders[GRAZER.index()].hunker.herd_enabled = false;
    let (v, hunkered, act) = run_once(c, d, false);
    assert!(hunkered && v.hunkered && !v.fleeing, "switch off: the calf hunkers under its soft shield");
    assert_eq!((act.forward, act.feed), (0.0, 0.0));
}

/// The config with catch at `catch_m`, the hunter `speed_factor` × the grazer's cruise in
/// body lengths.
fn catching(catch_m: f64, speed_factor: f64) -> FaunaConfig {
    let mut c = config();
    c.founders[GRAZER.index()].hunker.catch_m = catch_m;
    c.founders[VH.index()].cruise_body_lengths_per_s = speed_factor * c.founder(GRAZER).cruise_body_lengths_per_s;
    c
}

#[test]
fn a_faster_hunter_catches_at_exactly_catch_m_and_not_a_hair_beyond() {
    let catch = 2.0;
    let (v, hunkered, _) = run_once(catching(catch, 4.0), catch, true);
    assert!(hunkered && v.hunkered, "a faster hunter at catch_m: hunker: {v:?}");
    let (v, hunkered, _) = run_once(catching(catch, 4.0), catch + 1e-9, true);
    assert!(!hunkered && v.fleeing, "beyond catch_m it still runs: {v:?}");
}

#[test]
fn a_hunter_it_can_outrun_never_catches_it_and_catch_zero_is_off() {
    let catch = 2.0;
    // Gait is the hunter's lineage cruise in body lengths × its visible length: slower than
    // the grazer's own cruise by a margin far beyond the size difference.
    let d = 0.5 * (config().founder(GRAZER).hunker.enter_m + catch);
    let (v, hunkered, _) = run_once(catching(catch, 0.01), d, true);
    assert!(!hunkered && v.fleeing, "a slower hunter inside catch_m does not catch it: {v:?}");
    let (v, hunkered, _) = run_once(catching(0.0, 4.0), 0.5 * (config().founder(GRAZER).hunker.enter_m + catch), true);
    assert!(!hunkered && v.fleeing, "catch_m 0 is off whatever the hunter's speed: {v:?}");
}

/// A wall of soil `rows` voxels tall right behind (west of) the grazer's footprint.
fn wall_behind(w: &mut World, f: &Fauna, g: u64, rows: u32) {
    let a = animal(f, g);
    let r = f.config.founder(GRAZER).body_at(a.body).footprint_radius();
    let wall_x = ((a.pose.x - r) / VOXEL_M).floor() as i64 - 1;
    for z in 0..w.view().config.depth {
        for y in (SURFACE_Y + 1)..=(SURFACE_Y + rows) {
            w.apply(WorldCommand::SetMaterial { x: wall_x, y, z, material: Material::Soil });
        }
    }
}

fn probe_refused(w: &World, f: &Fauna, g: u64, probe: f64) -> bool {
    let a = animal(f, g);
    let phys = *f.config.founder(GRAZER);
    let geo = phys.body_at(a.body);
    crate::body::motor_walk(&w.view(), (a.pose.x, a.pose.z), (a.pose.x - probe, a.pose.z), a.site.y,
        geo.footprint_radius(), phys.wade_depth_m(&geo), geo.headroom_voxels(VOXEL_M), phys.step_limits(VOXEL_M)).is_none()
}

#[test]
fn an_escape_probe_of_zero_never_corners_a_walled_grazer() {
    let probe = 0.7;
    let mut c = config();
    c.founders[GRAZER.index()].hunker.escape_probe_m = probe;
    let d = 0.5 * (c.founder(GRAZER).hunker.enter_m + c.founder(GRAZER).hunker.run_start_m);
    for on in [true, false] {
        let mut c = c.clone();
        if !on {
            c.founders[GRAZER.index()].hunker.escape_probe_m = 0.0;
        }
        let (mut w, flora, mut f, g, _) = pair(c, d, true);
        wall_behind(&mut w, &f, g, 4);
        assert!(probe_refused(&w, &f, g, probe), "fixture: the way behind is walled");
        sense(&mut f, &w);
        respond(&mut f, &w, &flora, g);
        assert_eq!(animal(&f, g).hunkered, on, "probe on={on}: cornered only with a probe");
    }
}

#[test]
fn a_run_toward_a_ledge_it_cannot_step_down_is_cornered() {
    // The grazer stands on a raised shelf with its west edge right behind it: the drop is
    // far beyond its step down, so its escape probe has nowhere to go.
    let probe = 0.7;
    let mut c = config();
    c.founders[GRAZER.index()].hunker.escape_probe_m = probe;
    let d = 0.5 * (c.founder(GRAZER).hunker.enter_m + c.founder(GRAZER).hunker.run_start_m);
    let rise = 6u32;
    let mut w = plain_of_height(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, 402, 16);
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let g = body(&mut f, &w, GRAZER, GX, LANE_Z, EAST, true);
    let r = f.config.founder(GRAZER).body_at(animal(&f, g).body).footprint_radius();
    let edge_x = ((GX - r) / VOXEL_M).floor() as i64;
    let width = i64::from(w.view().config.width);
    for z in 0..w.view().config.depth {
        for x in edge_x..width {
            for y in (SURFACE_Y + 1)..=(SURFACE_Y + rise) {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    let top = SURFACE_Y + rise;
    let h = hunter(&mut f, &w, GX + d, LANE_Z);
    for id in [g, h] {
        let a = animal_mut(&mut f, id);
        a.site.y = top;
        a.pose.y_m = crate::surface_m(top, VOXEL_M);
    }
    assert!(f.config.founder(GRAZER).step_down_m < f64::from(rise) * VOXEL_M, "fixture: the drop is beyond its step down");
    assert!(probe_refused(&w, &f, g, probe), "fixture: the motor refuses the step off the shelf");
    let flora = Flora::new(FloraConfig::default());
    sense(&mut f, &w);
    let act = respond(&mut f, &w, &flora, g);
    assert!(animal(&f, g).hunkered, "a run that would go off the ledge clamps instead");
    assert_eq!(act.forward, 0.0, "it does not walk off the drop");
}

/// A calf at `GX`, a hunkered adult `gap` east of it on the lane, a moving hunter 2 m west.
fn calf_scene(c: FaunaConfig, gap: f64) -> (World, Flora, Fauna, u64, u64) {
    let w = clock_off(403);
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let calf = body(&mut f, &w, GRAZER, GX, LANE_Z, EAST, false);
    let adult = body(&mut f, &w, GRAZER, GX + gap, LANE_Z, EAST, true);
    hunker(&mut f, adult);
    hunter(&mut f, &w, GX - 2.0, LANE_Z);
    (w, Flora::new(FloraConfig::default()), f, calf, adult)
}

#[test]
fn a_calf_seeks_a_closed_adult_at_exactly_shelter_seek_and_not_a_hair_beyond() {
    let c = config();
    let seek = c.founder(GRAZER).hunker.shelter_seek_m;
    for (gap, seeks) in [(seek, true), (seek + 1e-9, false)] {
        let (w, flora, mut f, calf, _) = calf_scene(c.clone(), gap);
        sense(&mut f, &w);
        let act = respond(&mut f, &w, &flora, calf);
        let v = vigil(&f, calf);
        assert!(!animal(&f, calf).hunkered && act.feed == 0.0);
        // A seek is a bolt toward the shield's far side (east here); a plain run is a flight.
        assert_eq!(v.bolt_until > f.tick, seeks, "gap {gap}: seek {seeks}: {v:?}");
        assert!(v.fleeing, "gap {gap}: either way it is running from the hunter");
        if seeks {
            let off = (v.bolt_heading - EAST).abs();
            assert!(off < 1e-9, "toward the adult: {v:?}");
        }
    }
}

/// A calf against a closed adult's far side, a striker on the other side of the adult.
fn cover(c: FaunaConfig) -> (World, Fauna, u64, u64) {
    let w = clock_off(404);
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let calf = body(&mut f, &w, GRAZER, 5.0, LANE_Z, WEST, false);
    let adult = body(&mut f, &w, GRAZER, 4.0, LANE_Z, WEST, true);
    let gap = 0.75 * (crate::body_of(&f.config, &animal(&f, adult)).unwrap().footprint_radius()
        + crate::body_of(&f.config, &animal(&f, calf)).unwrap().footprint_radius());
    place(&mut f, adult, 5.0 - gap, LANE_Z, WEST);
    hunker(&mut f, adult);
    let striker = body(&mut f, &w, VH, 2.0, LANE_Z, EAST, true);
    (w, f, calf, striker)
}

#[test]
fn with_the_herd_switch_off_a_closed_adult_gives_a_calf_no_cover() {
    for on in [true, false] {
        let mut c = config();
        c.founders[GRAZER.index()].hunker.herd_enabled = on;
        let (w, f, calf, striker) = cover(c);
        let p = animal(&f, calf);
        assert_eq!(crate::vigil::sheltered(&f, &p, ring(&w), VOXEL_M), on);
        let e = crate::vigil::shelter_exposure(&f, &p, &animal(&f, striker), ring(&w), VOXEL_M);
        let want = if on { f.config.founder(GRAZER).hunker.shelter_exposure } else { 1.0 };
        assert_eq!(e, want, "herd on={on}: the Veilhound's strike meets exposure {want}");
    }
}

#[test]
fn a_calf_tucked_against_its_shield_does_not_begin_a_new_flight_every_sample() {
    let c = config();
    let w = clock_off(405);
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let adult = body(&mut f, &w, GRAZER, 4.0, LANE_Z, EAST, true);
    hunker(&mut f, adult);
    let threat_x = 1.0;
    hunter(&mut f, &w, threat_x, LANE_Z);
    // The goal: the adult's far side from the hunter, summed footprints out.
    let calf = body(&mut f, &w, GRAZER, 6.0, LANE_Z, EAST, false);
    let gap = crate::body_of(&f.config, &animal(&f, calf)).unwrap().footprint_radius()
        + crate::body_of(&f.config, &animal(&f, adult)).unwrap().footprint_radius();
    place(&mut f, calf, 4.0 + gap, LANE_Z, EAST);
    let flora = Flora::new(FloraConfig::default());
    let k = GRAZER.index();
    let mut flees = Vec::new();
    for _ in 0..6 {
        sense(&mut f, &w);
        let act = respond(&mut f, &w, &flora, calf);
        assert_eq!(act.forward, 0.0, "tucked in, it stands");
        hunker(&mut f, adult);
        flees.push(f.ledger.vigil.flees[k]);
    }
    assert!(crate::vigil::sheltered(&f, &animal(&f, calf), ring(&w), VOXEL_M), "fixture: sheltered");
    assert!(flees[5] <= 1, "a calf standing in shelter is not a flight begun each sample: {flees:?}");
}

/// A hunkered adult grazer at `GX`, hunters around it in contact: `hungry` eating, `sated`
/// not, `pups` pack pups that are eating; `herd` the switch. Returns the pry after one call.
fn pry_once(herd: bool, hungry: usize, sated: usize, emergency: usize, pups: usize) -> f64 {
    let w = clock_off(406);
    let mut c = config();
    let hk = &mut c.founders[GRAZER.index()].hunker;
    hk.herd_enabled = herd;
    hk.pry_per_member_s = 1.0;
    hk.resist_per_s = 1.5;
    hk.ring_resist_per_kin = 0.0;
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let g = body(&mut f, &w, GRAZER, GX, LANE_Z, WEST, true);
    hunker(&mut f, g);
    let n = hungry + sated + emergency + pups;
    for k in 0..n {
        let th = k as f64 * std::f64::consts::TAU / n as f64;
        let id = hunter(&mut f, &w, GX + 0.25 * th.cos(), LANE_Z + 0.25 * th.sin());
        let a = animal_mut(&mut f, id);
        a.eating = k < hungry || k >= hungry + sated + emergency;
        a.emergency = (hungry + sated..hungry + sated + emergency).contains(&k);
        if k >= hungry + sated + emergency {
            a.pack = Some(crate::PackMember { pack: 99, pup: true, calling_ticks: 0 });
        }
    }
    f.tick += 1;
    crate::vigil::pry(&mut f, ring(&w));
    vigil(&f, g).pry
}

fn net(members: f64) -> f64 {
    (cubarium_voxel::DT * (members * 1.0 - 1.5)).max(0.0)
}

#[test]
fn a_sated_hunter_beside_hungry_ones_adds_nothing_to_the_pry() {
    assert_eq!(pry_once(true, 2, 1, 0, 0), net(2.0), "two hungry pry; the sated third does not");
    assert_eq!(pry_once(true, 2, 0, 0, 0), net(2.0));
    assert_eq!(pry_once(true, 1, 2, 0, 0), 0.0, "one hungry alone loses to the resistance");
}

#[test]
fn with_the_herd_switch_off_a_sated_hunter_pries_as_before() {
    assert_eq!(pry_once(false, 2, 1, 0, 0), net(3.0));
    assert_eq!(pry_once(false, 0, 2, 0, 0), net(2.0));
}

#[test]
fn a_hunter_in_an_emergency_pries_though_its_eating_latch_is_off() {
    assert_eq!(pry_once(true, 1, 0, 1, 0), net(2.0));
}

#[test]
fn hungry_pups_in_contact_never_pry() {
    assert_eq!(pry_once(true, 2, 0, 0, 2), net(2.0), "PUP x HERD: a pack's pups do no herd work");
    assert_eq!(pry_once(false, 2, 0, 0, 2), net(2.0));
}

#[test]
fn a_run_whose_hunter_is_seen_still_ends_after_the_calm_not_the_lost_calm() {
    let c = config();
    let vg = c.founder(GRAZER).vigilance;
    assert!(vg.lost_calm_s > 100.0 * vg.calm_s, "fixture: the lost calm is far longer");
    let d = 0.5 * (c.founder(GRAZER).hunker.enter_m + c.founder(GRAZER).hunker.run_start_m);
    let (w, flora, mut f, g, h) = pair(c.clone(), d, true);
    sense(&mut f, &w);
    respond(&mut f, &w, &flora, g);
    assert!(vigil(&f, g).fleeing, "fixture: the run starts");
    // The hunter stops where it is, beyond `still_threat_near_m`, still seen.
    assert!(d > f.config.predation.still_threat_near_m);
    {
        let a = animal_mut(&mut f, h);
        a.speed_m_s = 0.0;
        a.still_ticks = u64::MAX / 2;
        a.state = State::Resting;
    }
    let horizon = cubarium_voxel::ticks_at_least_one(vg.calm_s) + 2 * f.config.perception.stagger_ticks() + 2;
    for _ in 0..horizon {
        sense(&mut f, &w);
        respond(&mut f, &w, &flora, g);
    }
    let v = vigil(&f, g);
    assert!(!v.fleeing && !animal(&f, g).hunkered, "a still hunter it can see ends the run at its calm: {v:?}");
}

#[test]
fn a_run_caught_by_a_close_hunter_books_one_run_to_hunker() {
    let c = config();
    let hk = c.founder(GRAZER).hunker;
    let d = 0.5 * (hk.enter_m + hk.run_start_m);
    let (w, flora, mut f, g, h) = pair(c, d, true);
    sense(&mut f, &w);
    respond(&mut f, &w, &flora, g);
    assert!(vigil(&f, g).fleeing);
    place(&mut f, h, GX + 0.5 * hk.enter_m, LANE_Z, WEST);
    for _ in 0..3 {
        sense(&mut f, &w);
        respond(&mut f, &w, &flora, g);
    }
    let k = GRAZER.index();
    assert!(animal(&f, g).hunkered);
    assert_eq!(f.ledger.vigil.runs_to_hunker[k], 1);
    assert_eq!(f.ledger.vigil.hunkers[k], 1);
}

#[test]
fn herd_config_readouts_and_vigil_state_survive_a_postcard_round_trip() {
    let mut c = config();
    let hk = &mut c.founders[GRAZER.index()].hunker;
    hk.run_start_m = 7.25;
    hk.catch_m = 2.5;
    hk.escape_probe_m = 0.375;
    hk.shelter_seek_m = 5.5;
    hk.shelter_margin_m = 0.125;
    hk.shelter_exposure = 0.375;
    let (w, flora, mut f, g, _) = pair(c, 3.0, true);
    sense(&mut f, &w);
    respond(&mut f, &w, &flora, g);
    let k = GRAZER.index();
    f.ledger.vigil.runs_to_hunker[k] = 4;
    f.ledger.vigil.grazer_kills_by_posture[VH.index()] = [1, 2, 3, 4, 5];
    let back = Fauna::load(&f.save()).expect("round trip");
    assert_eq!(back.config.founder(GRAZER).hunker, f.config.founder(GRAZER).hunker);
    assert_eq!(back.ledger.vigil, f.ledger.vigil);
    assert_eq!(back.percepts.get(&g).map(|p| p.vigil), f.percepts.get(&g).map(|p| p.vigil));
    let mut off = config();
    off.founders[GRAZER.index()].hunker.herd_enabled = false;
    let back = Fauna::load(&Fauna::new(off).save()).unwrap();
    assert!(!back.config.founder(GRAZER).hunker.herd_enabled);
}
