//! **HERD-PULL, from the spec** (Wrysk's decision of 2026-10-09, training plan §6 item 15):
//! a calm grazer beyond its comfort from all the kin it knows heads back toward them, one
//! within it forages freely, and one walking off a cropped patch still walks; feeding is never
//! pulled; a big herd draws less; a remembered herd draws for its memory only; many eyes: more
//! kin close by, a smaller share of cropping spent scanning, never none; the seedporter's
//! troop and the loftstrider calf's family pull the same way; the skill walks to the group
//! outside the heuristic's trigger.
//!
//! Every scene sets the knobs it relies on; no shipped number is asserted.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use crate::controller::Seek;
use crate::{Actions, Activity, Fauna, FaunaConfig, Founder, Herding, SenseTable, StartingStores, State, VigilMode, Vigilance};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig};
use fixture::*;

const GRAZER: Founder = Founder::Browser;
const NORTH: f64 = 0.0;
const WALK: Actions = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
const CROP: Actions = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };
const COMFORT: f64 = 2.0;

/// Every lineage awake, its senses every tick; `f`'s table live with an all-round sight that
/// sees a still body at its whole range, its pull at `COMFORT` with half its turn, no herd
/// fade, no memory; no ground response.
fn config_for(f: Founder) -> FaunaConfig {
    let mut c = hunting_config();
    for g in Founder::ALL {
        c.founders[g.index()].active = Activity::Always;
        c.founders[g.index()].vigilance = Vigilance::default();
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let p = &mut c.founders[f.index()];
    p.senses = SenseTable {
        live: true,
        sight_m: 7.0,
        sight_night_m: 7.0,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: 180.0,
        kin_tracks: 8,
        ..SenseTable::default()
    };
    p.herding = Herding { comfort_m: COMFORT, pull: 0.5, herd_max: 0, memory_s: 0.0, young_only: false, follow_moving: false };
    c
}

fn ring(w: &World) -> f64 {
    f64::from(w.view().config.width) * w.view().config.voxel_m
}

/// A still body of `kind` at `(x, z)`, full grown or at `grown` of its adult body.
fn put(f: &mut Fauna, w: &World, kind: Founder, x: f64, z: f64, heading: f64, grown: f64) -> u64 {
    let id = introduce(f, w, kind, StartingStores::FULL);
    place(f, id, x, z, heading);
    let adult = f.config.founder(kind).core.birth_body;
    let a = animal_mut(f, id);
    a.body = grown * adult;
    a.state = State::Walking;
    a.speed_m_s = 0.0;
    id
}

fn sense(f: &mut Fauna, w: &World) {
    f.tick += 1;
    crate::perception::sense_pass(f, &w.view(), false, 1.0, 1);
}

fn index(f: &Fauna, id: u64) -> usize {
    f.animals.iter().position(|a| a.id == id).expect("alive")
}

/// The drift back's answer for `id` over `asked`, after one sensing.
fn steer(f: &mut Fauna, w: &World, id: u64, asked: Actions, seek: Seek) -> Option<Actions> {
    sense(f, w);
    let i = index(f, id);
    crate::herding::steer(f, i, asked, seek, ring(w))
}

/// A scene: the subject of `kind` at x = 3 m facing north, kin at the given x (z on the lane).
fn scene(c: FaunaConfig, kind: Founder, grown: f64, kin: &[f64]) -> (World, Fauna, u64, Vec<u64>) {
    let w = clock_off(77);
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let g = put(&mut f, &w, kind, 3.0, LANE_Z, NORTH, grown);
    let ids = kin
        .iter()
        .map(|&x| {
            let id = put(&mut f, &w, kind, x, LANE_Z, NORTH, 1.0);
            drive(&mut f, id, 0.0, false);
            id
        })
        .collect();
    (w, f, g, ids)
}

/// The turn toward the east: a body facing north turns right (positive) to face it.
fn turns_east(a: Actions) -> bool {
    a.turn > 0.0
}

#[test]
fn a_calm_grazer_beyond_its_comfort_from_all_its_kin_leans_its_walk_toward_them() {
    for seek in [Seek::Search, Seek::Lead, Seek::Unknown] {
        let (w, mut f, g, _) = scene(config_for(GRAZER), GRAZER, 1.0, &[3.0 + 2.0 * COMFORT, 3.0 + 2.5 * COMFORT]);
        let go = steer(&mut f, &w, g, WALK, seek).expect("beyond comfort it is pulled");
        assert!(turns_east(go), "{seek:?}: it turns toward the kin to its east: {go:?}");
        assert_eq!((go.forward, go.feed), (WALK.forward, WALK.feed), "{seek:?}: its pace and feeding are its own");
        assert!(go.turn < 1.0, "{seek:?}: a lean, not a turn straight at them, at half the pull");
    }
}

/// A strip `width_v` voxels round and 3 m deep, soil to the fixture's surface, the clock off.
fn strip(width_v: u32) -> World {
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material};
    let mut w = World::empty(VoxelConfig {
        width: width_v,
        height: 8,
        depth: 12,
        voxel_m: VOXEL_M,
        seed: 9,
        day: cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() },
        ..VoxelConfig::default()
    });
    for z in 0..12 {
        for x in 0..i64::from(width_v) {
            for y in 1..=SURFACE_Y {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

/// The mean distance over `secs` from a heuristic grazer to a still kin `start_m` east of it
/// on a 32 m ring, with its pull at `pull`; and the samples the drift back steered.
fn walk_with(pull: f64, start_m: f64, secs: u32) -> (f64, u64) {
    let mut c = config_for(GRAZER);
    c.founders[GRAZER.index()].herding.pull = pull;
    let w = strip(128);
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let g = put(&mut f, &w, GRAZER, 3.0, LANE_Z, NORTH, 1.0);
    let k = put(&mut f, &w, GRAZER, 3.0 + start_m, LANE_Z, NORTH, 1.0);
    drive(&mut f, k, 0.0, false);
    let teachers = f.config.teachers;
    f.controllers.set(g, crate::heuristic(GRAZER, teachers));
    assert!(f.controllers.layer(g) == crate::Layer::Heuristic, "fixture: the subject runs its own heuristic");
    let mut flora = Flora::new(FloraConfig::default());
    let ring = ring(&w);
    let (mut sum, ticks) = (0.0, secs * crate::TICK_HZ);
    for _ in 0..ticks {
        f.step_with(&w, &mut flora, 1);
        let (a, b) = (animal(&f, g).pose, animal(&f, k).pose);
        let (dx, dz) = crate::pose::wrapped_delta(a.x, a.z, b.x, b.z, ring);
        sum += dx.hypot(dz);
    }
    (sum / f64::from(ticks), f.herd_tally()[GRAZER.index()].pulled_samples)
}

#[test]
fn a_heuristic_grazer_beyond_its_comfort_keeps_nearer_its_herd_than_without_the_pull() {
    let start = 2.5 * COMFORT;
    let (with, pulled) = walk_with(0.5, start, 60);
    let (without, none) = walk_with(0.0, start, 60);
    assert!(pulled > 0 && none == 0, "fixture: the drift back steered only with the pull on ({pulled}, {none})");
    assert!(with < without, "with the pull it keeps nearer its herd ({with:.2} m vs {without:.2} m)");
    assert!(with < start, "and nearer than it started ({with:.2} m vs {start:.2} m)");
}

#[test]
fn a_grazer_within_its_comfort_forages_freely() {
    let (w, mut f, g, _) = scene(config_for(GRAZER), GRAZER, 1.0, &[3.0 + 0.5 * COMFORT, 3.0 + 3.0 * COMFORT]);
    for seek in [Seek::Search, Seek::Lead] {
        assert_eq!(steer(&mut f, &w, g, WALK, seek), None, "{seek:?}: its nearest kin within comfort: no pull");
    }
    let i = index(&f, g);
    assert_eq!(crate::herding::stay_with_group(&f, i, ring(&w)), None, "the skill too: it is with its group");
}

#[test]
fn feeding_is_never_pulled_and_a_walk_off_a_cropped_patch_still_walks() {
    // Its herd far: cropping is left alone, and so is an escape.
    let (w, mut f, g, _) = scene(config_for(GRAZER), GRAZER, 1.0, &[3.0 + 2.5 * COMFORT]);
    assert_eq!(steer(&mut f, &w, g, CROP, Seek::Taste), None, "holding still to feed: no pull");
    assert_eq!(steer(&mut f, &w, g, Actions { forward: 1.0, turn: -1.0, feed: 0.0 }, Seek::Obstacle), None, "out of the way of something: no pull");
    // FORAGE-LEAVE's walk off a spent patch (full forward, not feeding, its heuristic having
    // stopped to feed): with the herd close it is left as it is; with the herd far it still
    // walks on, leaning toward the herd.
    let leave = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
    let go = steer(&mut f, &w, g, leave, Seek::Taste).expect("herd far: the walk off leans to it");
    assert_eq!(go.forward, leave.forward, "it still moves on");
    assert!(turns_east(go));
    let (w, mut f, g, _) = scene(config_for(GRAZER), GRAZER, 1.0, &[3.0 + 0.5 * COMFORT]);
    assert_eq!(steer(&mut f, &w, g, leave, Seek::Taste), None, "herd close: the walk off is its own");
}

#[test]
fn off_or_off_the_ground_there_is_no_pull() {
    let mut c = config_for(GRAZER);
    c.founders[GRAZER.index()].herding.comfort_m = 0.0;
    let (w, mut f, g, _) = scene(c, GRAZER, 1.0, &[3.0 + 2.5 * COMFORT]);
    assert_eq!(steer(&mut f, &w, g, WALK, Seek::Search), None, "comfort_m 0 turns it off");
    let (w, mut f, g, _) = scene(config_for(GRAZER), GRAZER, 1.0, &[3.0 + 2.5 * COMFORT]);
    animal_mut(&mut f, g).mobility.mode = crate::MobilityMode::Climb;
    assert_eq!(steer(&mut f, &w, g, WALK, Seek::Search), None, "climbing: its own climb runs");
}

#[test]
fn a_big_herd_draws_less_than_a_small_one() {
    let far = [3.0 + 2.0 * COMFORT, 3.0 + 2.2 * COMFORT, 3.0 + 2.4 * COMFORT, 3.0 + 2.6 * COMFORT];
    let weight = |herd_max: u32| {
        let mut c = config_for(GRAZER);
        c.founders[GRAZER.index()].herding.herd_max = herd_max;
        let (w, mut f, g, _) = scene(c, GRAZER, 1.0, &far);
        sense(&mut f, &w);
        let i = index(&f, g);
        let p = crate::herding::group_pull(&f, i, ring(&w), COMFORT).expect("pulled");
        assert_eq!(p.kin, far.len(), "fixture: it knows the whole herd");
        p.weight
    };
    let unfaded = weight(0);
    let faded = weight(far.len() as u32 - 2);
    assert!(faded < unfaded && faded > 0.0, "more kin than herd_max: a weaker pull ({faded} vs {unfaded})");
    assert_eq!(weight(far.len() as u32), unfaded, "a herd no bigger than herd_max: the full pull");
}

#[test]
fn a_remembered_herd_draws_for_its_memory_only() {
    let mut c = config_for(GRAZER);
    let memory_s = 5.0;
    c.founders[GRAZER.index()].herding.memory_s = memory_s;
    let (w, mut f, g, kin) = scene(c, GRAZER, 1.0, &[3.0 + 2.5 * COMFORT]);
    sense(&mut f, &w);
    // The kin goes out of sense (beyond its sight, across the ring), its last place remembered.
    let far_x = 3.0 + 0.5 * ring(&w);
    place(&mut f, kin[0], far_x, LANE_Z, NORTH);
    f.config.founders[GRAZER.index()].senses.sight_m = 1.0;
    f.config.founders[GRAZER.index()].senses.sight_night_m = 1.0;
    let go = steer(&mut f, &w, g, WALK, Seek::Search).expect("no kin in sense: the remembered one draws it");
    assert!(turns_east(go), "toward where it last sensed the kin: {go:?}");
    f.tick += cubarium_voxel::ticks_at_least_one(memory_s) + 1;
    assert_eq!(steer(&mut f, &w, g, WALK, Seek::Search), None, "past its memory: no pull");
}

/// The share of `ticks` cropping samples the grazer at x = 3 m spends in a scan pause, with
/// kin at the given x, on a scan of `every_s` cropping and `scan_s` head up, many eyes at
/// `gain` per kin within `kin_m`, counting `max`.
fn scan_share(kin: &[f64], every_s: f64, scan_s: f64, kin_m: f64, gain: f64, max: u32, ticks: u32) -> f64 {
    let mut c = config_for(GRAZER);
    c.founders[GRAZER.index()].herding = Herding::default();
    c.founders[GRAZER.index()].vigilance = Vigilance {
        mode: VigilMode::Herd,
        scan_every_s: every_s,
        scan_s,
        scan_kin_m: kin_m,
        scan_kin_gain: gain,
        scan_kin_max: max,
        watch_calm: false,
        ..Vigilance::default()
    };
    let (w, mut f, g, _) = scene(c, GRAZER, 1.0, kin);
    let flora = Flora::new(FloraConfig::default());
    let mut scanning = 0u32;
    for _ in 0..ticks {
        sense(&mut f, &w);
        let i = index(&f, g);
        crate::vigil::respond(&mut f, i, &w.view(), &flora.view(), CROP);
        scanning += u32::from(f.percept(g).is_some_and(|p| p.vigil.scanning(f.tick)));
    }
    f64::from(scanning) / f64::from(ticks)
}

#[test]
fn a_grazer_with_more_kin_close_by_scans_a_smaller_share_of_its_cropping_and_never_none() {
    let (every, scan, kin_m, gain, max) = (1.0, 0.5, 3.0, 0.5, 2);
    let ticks = 40 * crate::TICK_HZ;
    let alone = scan_share(&[], every, scan, kin_m, gain, max, ticks);
    let one = scan_share(&[4.0], every, scan, kin_m, gain, max, ticks);
    let herd = scan_share(&[3.5, 4.0, 4.5, 5.0], every, scan, kin_m, gain, max, ticks);
    let far = scan_share(&[3.0 + kin_m + 1.0, 3.0 + kin_m + 1.5], every, scan, kin_m, gain, max, ticks);
    assert!(alone > 0.0, "fixture: alone it scans ({alone})");
    assert!(one < alone, "one herd-mate close by: less scanning ({one} vs {alone})");
    assert!(herd < one, "more herd-mates close by: less again ({herd} vs {one})");
    assert!(herd > 0.0, "a big herd still scans ({herd})");
    assert!((far - alone).abs() < 1e-9, "kin beyond scan_kin_m share no watch ({far} vs {alone})");
    // Beyond scan_kin_max the watch is shared no further: bounded.
    let capped = scan_share(&[3.5, 4.0], every, scan, kin_m, gain, max, ticks);
    assert!((herd - capped).abs() < 1e-9, "more kin than scan_kin_max scan as scan_kin_max do ({herd} vs {capped})");
    // However large the gain, the cadence stays finite.
    let vg = Vigilance { scan_every_s: every, scan_s: scan, scan_kin_m: kin_m, scan_kin_gain: 1e6, scan_kin_max: max, ..Vigilance::default() };
    assert!(vg.scan_every_with(u32::MAX).is_finite());
}

#[test]
fn a_seedporter_and_a_loftstrider_calf_pull_toward_their_troop_or_family_the_same_way() {
    for kind in [Founder::Seedporter, Founder::Loftstrider] {
        assert!(crate::herding::herding_of(kind).comfort_m > 0.0, "{}: its lineage herds", kind.name());
        let mut c = config_for(kind);
        c.founders[kind.index()].herding.young_only = kind == Founder::Loftstrider;
        let young = 0.5;
        let (w, mut f, g, _) = scene(c.clone(), kind, young, &[3.0 + 2.5 * COMFORT]);
        let go = steer(&mut f, &w, g, WALK, Seek::Search).unwrap_or_else(|| panic!("{}: the young one is pulled", kind.name()));
        assert!(turns_east(go) && go.forward == WALK.forward, "{}: it leans toward its kin: {go:?}", kind.name());
        let (w, mut f, g, _) = scene(c.clone(), kind, young, &[3.0 + 0.5 * COMFORT]);
        assert_eq!(steer(&mut f, &w, g, WALK, Seek::Search), None, "{}: within comfort, free", kind.name());
        let (w, mut f, g, _) = scene(c, kind, 1.0, &[3.0 + 2.5 * COMFORT]);
        let adult = steer(&mut f, &w, g, WALK, Seek::Search);
        if kind == Founder::Loftstrider {
            assert_eq!(adult, None, "young_only: a grown loftstrider is not pulled");
        } else {
            assert!(adult.is_some_and(turns_east), "a grown seedporter keeps to its troop");
        }
    }
}

#[test]
fn the_skill_walks_to_the_group_outside_the_heuristics_trigger() {
    let (w, mut f, g, _) = scene(config_for(GRAZER), GRAZER, 1.0, &[3.0 + 2.5 * COMFORT]);
    // Facing its kin, beyond comfort: it walks straight to them even where the heuristic's
    // trigger would not pull (cropping).
    place(&mut f, g, 3.0, LANE_Z, EAST);
    assert_eq!(steer(&mut f, &w, g, CROP, Seek::Taste), None, "fixture: the trigger leaves cropping alone");
    let i = index(&f, g);
    let go = crate::herding::stay_with_group(&f, i, ring(&w)).expect("beyond comfort: the skill walks");
    assert_eq!((go.forward, go.feed), (1.0, 0.0));
    assert!(go.turn.abs() < 1e-6, "facing them, straight on: {go:?}");
    // Facing away: it turns to them.
    place(&mut f, g, 3.0, LANE_Z, WEST);
    sense(&mut f, &w);
    let i = index(&f, g);
    let go = crate::herding::stay_with_group(&f, i, ring(&w)).expect("beyond comfort");
    assert_eq!(go.forward, 0.0, "behind it: it turns before it walks");
    assert!(go.turn.abs() > 0.0);
}

/// LS-ROAM's scene: a loftstrider facing east at x = 3 m, sated or hungry, holding still on its
/// stand (its forager's sated stand: mouth asking, no forward), kin at `kin_x` walking or still;
/// `follow` sets its lineage's `follow_moving`.
fn going_with(follow: bool, hungry: bool, kin_moving: bool, kin_x: f64) -> Option<Actions> {
    let ls = Founder::Loftstrider;
    let mut c = config_for(ls);
    c.founders[ls.index()].herding.follow_moving = follow;
    let (w, mut f, g, kin) = scene(c, ls, 1.0, &[kin_x]);
    let still_after = f.config.predation.still_after_ticks();
    animal_mut(&mut f, kin[0]).still_ticks = if kin_moving { 0 } else { still_after + 10 };
    place(&mut f, g, 3.0, LANE_Z, EAST);
    animal_mut(&mut f, g).eating = hungry;
    steer(&mut f, &w, g, CROP, Seek::Taste)
}

#[test]
fn a_sated_body_standing_goes_with_kin_walking_off_beyond_its_comfort() {
    let far = 3.0 + 2.5 * COMFORT;
    let go = going_with(true, false, true, far).expect("a sated body goes with kin walking off");
    assert_eq!(go.feed, 0.0, "it walks, not feeding: {go:?}");
    assert!(go.forward > 0.0, "facing them, it walks: {go:?}");
    assert_eq!(go.turn, 0.0, "straight at them, dead ahead: {go:?}");
}

#[test]
fn a_sated_body_stays_with_kin_standing_or_within_comfort_and_a_hungry_one_feeds() {
    let far = 3.0 + 2.5 * COMFORT;
    assert_eq!(going_with(true, false, false, far), None, "kin standing beyond comfort do not draw it");
    assert_eq!(going_with(true, false, true, 3.0 + 0.5 * COMFORT), None, "kin walking within comfort do not draw it");
    assert_eq!(going_with(true, true, true, far), None, "a hungry body holding still feeds");
    assert_eq!(going_with(false, false, true, far), None, "follow_moving off: it stands, as before");
}

#[test]
fn a_sated_body_turns_on_the_spot_toward_kin_walking_behind_it() {
    let ls = Founder::Loftstrider;
    let mut c = config_for(ls);
    c.founders[ls.index()].herding.follow_moving = true;
    // Kin to the east, the body facing north: a quarter turn off, more than 60°.
    let (w, mut f, g, kin) = scene(c, ls, 1.0, &[3.0 + 2.5 * COMFORT]);
    animal_mut(&mut f, kin[0]).still_ticks = 0;
    animal_mut(&mut f, g).eating = false;
    let go = steer(&mut f, &w, g, CROP, Seek::Taste).expect("it goes with them");
    assert_eq!(go.forward, 0.0, "it turns before it walks: {go:?}");
    assert!(turns_east(go), "toward them: {go:?}");
}
