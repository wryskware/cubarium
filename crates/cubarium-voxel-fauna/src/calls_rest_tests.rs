//! **CALLS-REST** (`design/handoffs/calls-rest-2026-10-12.md`, behaviour-AI §2.1, §3.4, §7):
//! the loftstrider's calls and the hearers' reactions, the frondgrazer's bolt alarm on the
//! call substrate, rest physiology, waking, scent-led rest sites and kin recognition. A child
//! of `step` beside the ground-response cases, sharing the predation fixture.
//!
//! Every case builds its scene by hand and sets the knobs it relies on; no shipped number,
//! ratio, population or verdict is asserted.

use super::predation_fixture::*;
use crate::calls::{self, Heed};
use crate::perception;
use crate::{Actions, Activity, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel::World;
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel_flora::{Flora, FloraConfig};

const LS: Founder = Founder::Loftstrider;
const VH: Founder = Founder::Chorister;
/// A body cropping where it stands.
const CROP: Actions = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };

fn world(seed: u64) -> World {
    plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, seed, 16)
}

/// Every lineage awake, remote senses every tick, and loftstrider calls sized to the 12 m
/// ring.
fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let t = &mut c.founders[LS.index()].calls;
    t.range_m = 5.0;
    t.loudness = [1.0, 1.0];
    t.heed = [Heed::Everyone, Heed::Adults];
    t.trigger_m = [3.0, 2.0];
    t.alone_m = 2.0;
    t.heed_s = [4.0, 4.0];
    t.stranger_delay_s = [0.0, 1.0];
    t.gather_m = 1.0;
    t.arrive_m = 0.5;
    t.keep_s = 10.0;
    t.recognise_voice = true;
    c.founders[LS.index()].senses.kin_tracks = 4;
    c
}

fn body(fauna: &mut Fauna, world: &World, f: Founder, x: f64, heading: f64, speed: f64) -> u64 {
    let id = introduce(fauna, world, f, StartingStores::FULL);
    place(fauna, id, x, LANE_Z, heading);
    drive(fauna, id, 0.0, false);
    let a = animal_mut(fauna, id);
    a.speed_m_s = speed;
    a.still_ticks = if speed > 0.0 { 0 } else { u64::MAX / 2 };
    id
}

/// A calf: the lineage's newborn body, under `calf_below` of the adult's.
fn calf(fauna: &mut Fauna, world: &World, x: f64, heading: f64) -> u64 {
    let id = body(fauna, world, LS, x, heading, 0.0);
    let min = fauna.config.founder(LS).core.body_min;
    animal_mut(fauna, id).body = min;
    assert!(calls::is_calf(&fauna.config, LS, &animal(fauna, id)));
    id
}

fn sense(fauna: &mut Fauna, world: &World) {
    fauna.tick += 1;
    perception::sense_pass(fauna, &world.view(), false, 1.0, 1);
}

fn respond(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, held: Actions) -> Option<Actions> {
    let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
    let want = perception::Want { tremor: true, ..Default::default() };
    perception::refresh(fauna, i, &world.view(), &flora.view(), None, want);
    crate::vigil::respond(fauna, i, &world.view(), &flora.view(), held)
}

fn called(fauna: &Fauna, id: u64) -> Vec<u8> {
    fauna.calls.iter().filter(|e| e.caller == id).map(|e| e.kind).collect()
}

fn cruise(fauna: &Fauna, f: Founder) -> f64 {
    fauna.config.manifest(f).cruise_m_per_s
}

// ------------------------------------------------------------------ step 1: the calls

/// An adult that sees a pressing hound within the alarm's trigger snorts; beyond it, it does
/// not; a calf chased within the distress trigger bleats and never snorts.
#[test]
fn an_adult_seeing_a_pressing_hound_snorts_and_a_chased_calf_bleats() {
    let world = world(31);
    let flora = Flora::new(FloraConfig::default());
    let c = config();
    let t = c.founder(LS).calls;
    for (d, snorts) in [(0.6 * t.trigger_m[0], true), (1.4 * t.trigger_m[0], false)] {
        let mut f = Fauna::new(c.clone());
        f.set_births_enabled(false);
        let adult = body(&mut f, &world, LS, 2.0, EAST, 0.0);
        let speed = cruise(&f, VH);
        body(&mut f, &world, VH, 2.0 + d, WEST, speed);
        sense(&mut f, &world);
        respond(&mut f, &world, &flora, adult, CROP);
        assert_eq!(called(&f, adult), if snorts { vec![0] } else { vec![] }, "hound at {d} m");
    }
    let mut f = Fauna::new(c.clone());
    f.set_births_enabled(false);
    let young = calf(&mut f, &world, 2.0, EAST);
    let speed = cruise(&f, VH);
    body(&mut f, &world, VH, 2.0 + 0.6 * t.trigger_m[1], WEST, speed);
    sense(&mut f, &world);
    respond(&mut f, &world, &flora, young, CROP);
    assert_eq!(called(&f, young), vec![1], "a chased calf bleats");
}

/// A calf with a hound known beyond its chase trigger bleats only while no adult-sized kin is
/// known within `alone_m`; a calf struck since its last sample bleats with no threat near.
#[test]
fn a_calf_alone_with_a_threat_or_struck_bleats() {
    let world = world(32);
    let flora = Flora::new(FloraConfig::default());
    let c = config();
    let t = c.founder(LS).calls;
    let hound_at = 2.0 + 0.5 * (t.trigger_m[1] + c.founder(LS).senses.sight_m.min(5.0));
    for with_adult in [false, true] {
        let mut f = Fauna::new(c.clone());
        f.set_births_enabled(false);
        let young = calf(&mut f, &world, 2.0, EAST);
        if with_adult {
            body(&mut f, &world, LS, 2.0 - 0.5 * t.alone_m, EAST, 0.0);
        }
        let speed = cruise(&f, VH);
        body(&mut f, &world, VH, hound_at, WEST, speed);
        sense(&mut f, &world);
        respond(&mut f, &world, &flora, young, CROP);
        assert_eq!(called(&f, young), if with_adult { vec![] } else { vec![1] }, "adult near: {with_adult}");
    }
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let young = calf(&mut f, &world, 2.0, EAST);
    sense(&mut f, &world);
    respond(&mut f, &world, &flora, young, CROP);
    assert!(called(&f, young).is_empty());
    animal_mut(&mut f, young).wound += 0.1;
    sense(&mut f, &world);
    respond(&mut f, &world, &flora, young, CROP);
    assert_eq!(called(&f, young), vec![1], "struck");
}

/// Both loftstrider kinds are heard within range, muffled behind rock, and unheard beyond.
#[test]
fn each_loftstrider_call_is_heard_in_range_muffled_behind_rock_and_unheard_beyond() {
    // Tall enough for a loftstrider's head under the wall's top.
    let tall = || plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, 33, 40);
    let open = tall();
    let mut rocky = tall();
    for z in 0..12 {
        for y in 1..40 {
            rocky.apply(cubarium_voxel::Command::SetMaterial { x: 10, y, z, material: cubarium_voxel::Material::Rock });
        }
    }
    let mut c = config();
    c.perception.occlusion = 0.25;
    let range = c.founder(LS).calls.range_m;
    for kind in 0..2 {
        // Open: the near hearer (half the range) hears it, the far one (beyond) does not.
        // Behind the wall at x = 2.5..2.75 m the near hearer is past the muffled range.
        for (w, near_heard) in [(&open, true), (&rocky, false)] {
            let mut f = Fauna::new(c.clone());
            f.set_births_enabled(false);
            f.world_geometry = Some((48.0 * VOXEL_M, VOXEL_M));
            let caller = body(&mut f, w, LS, 1.0, EAST, 0.0);
            let near = body(&mut f, w, LS, 1.0 + 0.5 * range, EAST, 0.0);
            let i = f.animals.iter().position(|a| a.id == caller).unwrap();
            assert!(calls::emit(&mut f, i, kind));
            calls::hear(&mut f, &w.view());
            assert_eq!(!calls::heard(&f, near).is_empty(), near_heard, "kind {kind}, rock {}", !near_heard);
        }
        let mut f = Fauna::new(c.clone());
        f.set_births_enabled(false);
        f.world_geometry = Some((48.0 * VOXEL_M, VOXEL_M));
        f.config.founders[LS.index()].calls.range_m = 2.0;
        let caller = body(&mut f, &open, LS, 1.0, EAST, 0.0);
        let far = body(&mut f, &open, LS, 4.0, EAST, 0.0);
        let close = body(&mut f, &open, LS, 2.5, EAST, 0.0);
        let i = f.animals.iter().position(|a| a.id == caller).unwrap();
        assert!(calls::emit(&mut f, i, kind));
        calls::hear(&mut f, &open.view());
        assert!(calls::heard(&f, far).is_empty(), "kind {kind} beyond range");
        assert!(!calls::heard(&f, close).is_empty(), "kind {kind} within range");
    }
}

/// A snort gathers its hearer: one farther than `gather_m` from the nearest kin it knows walks
/// to it, head up; one already within it stands facing the call, not cropping.
#[test]
fn a_snort_gathers_a_hearer_to_its_kin_head_up() {
    let world = world(34);
    let flora = Flora::new(FloraConfig::default());
    let c = config();
    let gather = c.founder(LS).calls.gather_m;
    for (gap, walks) in [(2.0 * gather, true), (0.5 * gather, false)] {
        let mut f = Fauna::new(c.clone());
        f.set_births_enabled(false);
        let hearer = body(&mut f, &world, LS, 2.0, EAST, 0.0);
        let caller = body(&mut f, &world, LS, 2.0 + gap, EAST, 0.0);
        sense(&mut f, &world);
        let i = f.animals.iter().position(|a| a.id == caller).unwrap();
        assert!(calls::emit(&mut f, i, 0));
        calls::hear(&mut f, &world.view());
        let act = respond(&mut f, &world, &flora, hearer, CROP).expect("a heeded snort overrides cropping");
        assert_eq!(act.feed, 0.0, "head up");
        assert_eq!(act.forward > 0.0, walks, "gap {gap} m");
    }
}

/// A calf's bleat draws its own mother at once; another adult the same distance away comes
/// only after `stranger_delay_s`.
#[test]
fn a_calfs_bleat_draws_its_own_mother_before_another_adult() {
    let world = world(35);
    let flora = Flora::new(FloraConfig::default());
    let c = config();
    let delay = c.founder(LS).calls.stranger_delay_s[1];
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let mother = body(&mut f, &world, LS, 3.0, EAST, 0.0);
    let young = calf(&mut f, &world, 5.0, EAST);
    let other = body(&mut f, &world, LS, 7.0, WEST, 0.0);
    animal_mut(&mut f, young).mother = Some(mother);
    sense(&mut f, &world);
    let i = f.animals.iter().position(|a| a.id == young).unwrap();
    assert!(calls::emit(&mut f, i, 1));
    calls::hear(&mut f, &world.view());
    let m = respond(&mut f, &world, &flora, mother, CROP).expect("the mother goes at once");
    assert!(m.forward > 0.0 && m.feed == 0.0);
    assert!(respond(&mut f, &world, &flora, other, CROP).is_none_or(|a| a.forward == 0.0), "a stranger waits");
    f.tick += cubarium_voxel::ticks_at_least_one(delay);
    let o = respond(&mut f, &world, &flora, other, CROP).expect("then the other adult comes");
    assert!(o.forward > 0.0 && o.feed == 0.0);
}

/// A calf does not heed a bleat (the distress call is heeded by adults only), and nobody heeds
/// a kind marked `Nobody`.
#[test]
fn heed_masks_who_reacts() {
    let world = world(36);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    let mut f = Fauna::new(c.clone());
    f.set_births_enabled(false);
    let young = calf(&mut f, &world, 2.0, EAST);
    let caller = calf(&mut f, &world, 4.0, EAST);
    sense(&mut f, &world);
    let i = f.animals.iter().position(|a| a.id == caller).unwrap();
    assert!(calls::emit(&mut f, i, 1));
    calls::hear(&mut f, &world.view());
    assert!(!calls::heard(&f, young).is_empty(), "recorded");
    assert!(respond(&mut f, &world, &flora, young, CROP).is_none_or(|a| a.forward == 0.0));
    c.founders[LS.index()].calls.heed = [Heed::Nobody, Heed::Nobody];
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let hearer = body(&mut f, &world, LS, 2.0, EAST, 0.0);
    let caller = body(&mut f, &world, LS, 4.0, EAST, 0.0);
    sense(&mut f, &world);
    let i = f.animals.iter().position(|a| a.id == caller).unwrap();
    assert!(calls::emit(&mut f, i, 0));
    calls::hear(&mut f, &world.view());
    assert!(respond(&mut f, &world, &flora, hearer, CROP).is_none());
}

// ------------------------------------------------------------------ step 2: the bolt alarm

const GRAZER: Founder = Founder::Browser;

/// Today's herd alarm rule (S2, before CALLS-REST), read straight off the listener's tracks:
/// the nearest kin heard in the latest sensings whose emission is at least
/// `alarm_emission_factor` times its walking emission. The reference the substrate must match.
fn todays_alarm(f: &Fauna, id: u64) -> Option<crate::Track> {
    let phys = f.config.founder(GRAZER);
    let (vg, pc, now) = (phys.vigilance, f.config.perception, f.tick);
    if !(vg.alarm_emission_factor > 0.0) {
        return None;
    }
    let walk = |size: f64| (phys.cruise_body_lengths_per_s * size).max(0.0).powf(pc.hearing_speed_exp) * size.max(0.0).powf(pc.hearing_size_exp);
    f.percept(id)?
        .tracks()
        .iter()
        .filter(|t| t.class == perception::class::KIN && t.modes & perception::HEARING != 0 && t.tick + 2 * pc.stagger_ticks() >= now)
        .find(|t| t.loud * t.d_m.max(pc.hearing_near_m) >= vg.alarm_emission_factor * walk(t.size_m))
        .copied()
}

/// **The frondgrazer's bolt alarm on the substrate matches today's** on a fixture at the
/// shipped frondgrazer knobs and the shipped sensing stagger: a bolting and a walking herd-mate
/// crossing a herd, some listeners behind rock, some beyond hearing. Every tick, for every
/// listener: who hears it (a record exactly when today's rule finds a bolting kin), when (the
/// record's tick is the track's sensing), from where and along which heading, and the
/// observation's kin-alarm channel all equal today's rule; and the response bolts along the
/// runner's heading as before.
#[test]
fn the_frondgrazers_bolt_alarm_matches_todays_on_a_fixture() {
    let mut w = plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, 37, 24);
    for z in 0..6 {
        for y in 1..24 {
            w.apply(cubarium_voxel::Command::SetMaterial { x: 30, y, z, material: cubarium_voxel::Material::Rock });
        }
    }
    let flora = Flora::new(FloraConfig::default());
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    f.world_geometry = Some((48.0 * VOXEL_M, VOXEL_M));
    let speed = cruise(&f, GRAZER);
    let bolter = body(&mut f, &w, GRAZER, 1.0, EAST, speed);
    let walker = body(&mut f, &w, GRAZER, 2.0, EAST, speed);
    f.percepts.entry(bolter).or_default().vigil.bolt_until = u64::MAX / 2;
    let mut listeners = Vec::new();
    for (k, x) in [3.0, 4.5, 6.0, 7.5, 9.0, 10.5].into_iter().enumerate() {
        let id = body(&mut f, &w, GRAZER, x, WEST, 0.0);
        place(&mut f, id, x, if k % 2 == 0 { 0.6 } else { 2.4 }, WEST);
        listeners.push(id);
    }
    let (mut heard, mut unheard) = (0, 0);
    for _ in 0..40 {
        for id in [bolter, walker] {
            let a = animal_mut(&mut f, id);
            a.pose.x = (a.pose.x + speed * crate::DT).rem_euclid(48.0 * VOXEL_M);
        }
        sense(&mut f, &w);
        let now = f.tick;
        for &id in &listeners {
            let today = todays_alarm(&f, id);
            let a = animal(&f, id);
            let got = calls::sensed_alarm(&f, &a, now);
            assert_eq!(got.is_some(), today.is_some(), "listener {id} at tick {now}");
            let p = *f.percept(id).unwrap_or(&crate::Percept::default());
            assert_eq!(crate::observe_v3::kin_alarm(&f, &a, &p), f64::from(today.is_some()), "the observation channel");
            match (got, today) {
                (Some(r), Some(t)) => {
                    assert_eq!((r.tick, r.x, r.z, r.heading), (t.tick, t.x, t.z, t.heading));
                    heard += 1;
                }
                _ => unheard += 1,
            }
        }
    }
    assert!(heard > 0 && unheard > 0, "the fixture holds both: {heard} heard, {unheard} not");
    // The response: a listener holding the record bolts along the runner's heading.
    let id = *listeners.iter().find(|&&id| calls::sensed_alarm(&f, &animal(&f, id), f.tick).is_some()).expect("one hears it");
    let heading = calls::sensed_alarm(&f, &animal(&f, id), f.tick).unwrap().heading;
    respond(&mut f, &w, &flora, id, CROP);
    let v = f.percept(id).unwrap().vigil;
    assert!(v.bolt_until > f.tick);
    assert_eq!(v.bolt_heading, heading);
}

/// The frondgrazer's observation input keeps its shape: its manifest carries no Calls block.
#[test]
fn the_frondgrazers_manifest_has_no_calls_block() {
    let m = crate::FaunaConfig::default().manifest(GRAZER);
    assert!(m.modules.iter().all(|b| b.name != "Calls"));
}
