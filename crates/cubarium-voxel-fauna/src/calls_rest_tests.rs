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

// ------------------------------------------------------------------ step 3: rest physiology

/// The loftstrider's senses cut to sight only, no still gain, at `sight_m`.
fn sight_only(c: &mut FaunaConfig, sight_m: f64) {
    let s = &mut c.founders[LS.index()].senses;
    s.sight_m = sight_m;
    s.sight_night_m = sight_m;
    s.sight_still_fraction = 1.0;
    s.hearing_m = 0.0;
    s.tremor_m = 0.0;
    s.still_gain = 1.0;
}

fn rest_table(c: &mut FaunaConfig) -> &mut crate::sleep::RestTable {
    &mut c.founders[LS.index()].rest
}

/// A light rester senses a moving hound within `rest_sense` of its range and not beyond it; a
/// deep sleeper senses nothing; awake, it sees the farther hound too.
#[test]
fn a_light_rester_senses_at_the_reduced_range_and_a_deep_sleeper_not_at_all() {
    let world = world(41);
    let mut c = config();
    sight_only(&mut c, 5.0);
    rest_table(&mut c).light = true;
    rest_table(&mut c).rest_sense = 0.5;
    let reduced = 5.0 * 0.5;
    for (state, near_seen, far_seen) in [("awake", true, true), ("light", true, false), ("deep", false, false)] {
        let mut f = Fauna::new(c.clone());
        f.set_births_enabled(false);
        let me = body(&mut f, &world, LS, 2.0, EAST, 0.0);
        let speed = cruise(&f, VH);
        let near = body(&mut f, &world, VH, 2.0 + 0.8 * reduced, WEST, speed);
        let far = body(&mut f, &world, VH, 2.0 + 1.4 * reduced, WEST, speed);
        if state != "awake" {
            let a = animal_mut(&mut f, me);
            a.state = crate::State::Asleep;
            a.doze.light = state == "light";
        }
        sense(&mut f, &world);
        let has = |id: u64| f.percept(me).is_some_and(|p| p.track(id).is_some());
        assert_eq!((has(near), has(far)), (near_seen, far_seen), "{state}");
    }
}

/// Lying down starts light rest; a deep bout begins only after `light_min_s`, lasts within
/// `deep_min_s..deep_max_s`, then light rest again; and a body of a lineage without light rest
/// is deep (blind) the whole rest.
#[test]
fn rest_starts_light_and_deep_bouts_come_after_the_light_minimum() {
    let mut c = config();
    let t = rest_table(&mut c);
    t.light = true;
    t.light_min_s = 2.0;
    t.deep_per_min = 1e6;
    t.deep_alone_scale = 1.0;
    t.deep_min_s = 1.0;
    t.deep_max_s = 1.5;
    t.need_s = 0.0;
    let world = world(42);
    let mut f = Fauna::new(c.clone());
    f.set_births_enabled(false);
    let me = body(&mut f, &world, LS, 2.0, EAST, 0.0);
    let i = f.animals.iter().position(|a| a.id == me).unwrap();
    f.animals[i].state = crate::State::Asleep;
    crate::sleep::step(&mut f, i, LS, false, 1);
    assert!(crate::sleep::light_rest(&f.animals[i]), "lying down starts light");
    let light_min = cubarium_voxel::ticks_at_least_one(2.0);
    let (mut first_deep, mut deep_len) = (None, 0u64);
    for k in 1..=4 * light_min {
        f.tick += 1;
        let began = crate::sleep::step(&mut f, i, LS, true, 1);
        if began && first_deep.is_none() {
            first_deep = Some(k);
        }
        if first_deep.is_some() && crate::sleep::deep_asleep(&f.animals[i]) {
            deep_len += 1;
        }
        if first_deep.is_some() && crate::sleep::light_rest(&f.animals[i]) {
            break;
        }
    }
    let first = first_deep.expect("a deep bout");
    assert!(first >= light_min, "not before the light minimum ({first} < {light_min})");
    let (lo, hi) = (cubarium_voxel::ticks_at_least_one(1.0), cubarium_voxel::ticks_at_least_one(1.5));
    assert!((lo..=hi).contains(&deep_len), "deep for {deep_len} ticks, drawn in {lo}..={hi}");
    // Light rest off: asleep is deep.
    let mut c = c;
    rest_table(&mut c).light = false;
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let me = body(&mut f, &world, LS, 2.0, EAST, 0.0);
    let i = f.animals.iter().position(|a| a.id == me).unwrap();
    f.animals[i].state = crate::State::Asleep;
    assert!(crate::sleep::step(&mut f, i, LS, false, 1), "falling asleep is a deep bout");
    assert!(crate::sleep::deep_asleep(&f.animals[i]));
}

/// **Sleeps lighter**: with `deep_alone_scale` 0, a light rester that has seen no relative up
/// never goes deep, while one that saw a relative up within `kin_awake_s` does at the same
/// rate and draws; the sight must be sight of a relative not in deep sleep. And the need
/// ceiling puts even the lone one into a deep bout once `need_s` has passed.
#[test]
fn deep_bouts_are_rarer_with_no_relative_seen_awake_and_the_need_ceiling_still_brings_one() {
    let world = world(43);
    let mut c = config();
    sight_only(&mut c, 5.0);
    let t = rest_table(&mut c);
    t.light = true;
    t.rest_sense = 1.0;
    t.light_min_s = 0.0;
    t.deep_per_min = 600.0;
    t.deep_alone_scale = 0.0;
    t.kin_awake_s = 30.0;
    t.need_s = 0.0;
    let run = |c: FaunaConfig, kin: Option<&str>| -> u64 {
        let mut f = Fauna::new(c);
        f.set_births_enabled(false);
        let me = body(&mut f, &world, LS, 2.0, EAST, 0.0);
        if let Some(how) = kin {
            let k = body(&mut f, &world, LS, 4.0, WEST, 0.0);
            if how == "deep" {
                animal_mut(&mut f, k).state = crate::State::Asleep;
            }
        }
        sense(&mut f, &world);
        let i = f.animals.iter().position(|a| a.id == me).unwrap();
        f.animals[i].state = crate::State::Asleep;
        let mut bouts = 0;
        let mut was = false;
        for _ in 0..200 {
            f.tick += 1;
            bouts += u64::from(crate::sleep::step(&mut f, i, LS, was, 7));
            was = true;
        }
        bouts
    };
    assert_eq!(run(c.clone(), None), 0, "alone: no deep bout at scale 0");
    assert_eq!(run(c.clone(), Some("deep")), 0, "a relative seen deep asleep is not one seen up");
    assert!(run(c.clone(), Some("up")) > 0, "with a relative seen up it goes deep");
    let mut c = c;
    rest_table(&mut c).need_s = 2.0;
    assert!(run(c, None) > 0, "the need ceiling");
}

// ------------------------------------------------------------------ step 4: waking

/// A loftstrider asleep at `x` (deep unless `light`), lying down at the current tick.
fn sleeper(f: &mut Fauna, w: &World, x: f64, light: bool) -> u64 {
    let id = body(f, w, LS, x, EAST, 0.0);
    let now = f.tick;
    let a = animal_mut(f, id);
    a.state = crate::State::Asleep;
    a.doze.light = light;
    a.doze.asleep_since = now;
    id
}

fn wakes(f: &Fauna, w: &World, id: u64) -> Option<crate::rhythm::Wake> {
    let i = f.animals.iter().position(|a| a.id == id).unwrap();
    crate::rhythm::wakes(f, &w.view(), i, &crate::rhythm::loud_bodies(f))
}

/// Hearing only (no sight), sized to the ring, sleeping at half its awake sensitivity.
fn hearing_config() -> FaunaConfig {
    let mut c = config();
    let s = &mut c.founders[LS.index()].senses;
    s.sight_m = 0.0;
    s.sight_night_m = 0.0;
    s.tremor_m = 0.0;
    s.hearing_m = 5.0;
    s.still_gain = 1.0;
    c.founders[LS.index()].vigilance.sleep_hearing = 0.5;
    c.founders[LS.index()].rest.rush_over_cruise = 1.2;
    c.perception.occlusion = 0.25;
    c
}

/// A deep sleeper misses a stalking hound (at its cruise, however close) and wakes to a
/// rushing one within hearing; beyond its hearing, or behind rock that muffles the rush below
/// its asleep threshold, the rush does not wake it. The threshold is set from the config: the
/// rush is heard asleep at the test distance on open ground and not at the occluded loudness.
#[test]
fn a_deep_sleeper_misses_a_stalking_hound_and_wakes_to_a_rushing_one_within_hearing() {
    let tall = || plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, 44, 40);
    let open = tall();
    let mut rocky = tall();
    for z in 0..12 {
        for y in 1..40 {
            rocky.apply(cubarium_voxel::Command::SetMaterial { x: 12, y, z, material: cubarium_voxel::Material::Rock });
        }
    }
    let mut c = hearing_config();
    let probe = Fauna::new(c.clone());
    let cruise_vh = cruise(&probe, VH);
    let rush = 2.5 * cruise_vh;
    let d = 2.5;
    // The asleep threshold between the rush's open and occluded loudness at d.
    let e_rush = {
        let mut f = Fauna::new(c.clone());
        let h = body(&mut f, &open, VH, 1.0, WEST, rush);
        crate::perception::emission(&f.config, &animal(&f, h))
    };
    let at_d = e_rush / d;
    c.founders[LS.index()].senses.hearing_min = 0.5 * 0.5 * (at_d + at_d * 0.25);
    for (w, speed, x_hound, wakes_up) in [
        (&open, cruise_vh, 2.0 + d, false),
        (&open, rush, 2.0 + d, true),
        (&open, rush, 2.0 + 1.1 * 5.0, false),
        (&rocky, rush, 2.0 + d, false),
    ] {
        let mut f = Fauna::new(c.clone());
        f.set_births_enabled(false);
        let me = sleeper(&mut f, w, 2.0, false);
        body(&mut f, w, VH, x_hound, WEST, speed);
        let got = wakes(&f, w, me);
        assert_eq!(got.is_some(), wakes_up, "hound at {x_hound} m, {speed} m/s, rock {}", std::ptr::eq(w, &rocky));
        if wakes_up {
            assert_eq!(got, Some(crate::rhythm::Wake::Footfall));
        }
    }
}

/// With `rest.rush_over_cruise` 0 (every lineage but the loftstrider), a rush wakes no one: the
/// sleeper wakes to running prey only, as before.
#[test]
fn without_the_rush_knob_a_rush_wakes_no_one() {
    let w = world(45);
    let mut c = hearing_config();
    c.founders[LS.index()].rest.rush_over_cruise = 0.0;
    c.founders[LS.index()].senses.hearing_min = 1e-9;
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let me = sleeper(&mut f, &w, 2.0, false);
    let speed = 2.5 * cruise(&f, VH);
    body(&mut f, &w, VH, 3.0, WEST, speed);
    assert_eq!(wakes(&f, &w, me), None);
}

/// A heeded call heard since the sleeper lay down wakes it; one heard before does not; and a
/// kind it does not heed (a calf and the bleat, heeded by adults) does not.
#[test]
fn a_heeded_call_heard_since_lying_down_wakes_a_sleeper() {
    let w = world(46);
    let c = config();
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let caller = body(&mut f, &w, LS, 2.0, EAST, 0.0);
    let adult = sleeper(&mut f, &w, 3.0, false);
    let young = calf(&mut f, &w, 3.5, EAST);
    {
        let now = f.tick;
        let a = animal_mut(&mut f, young);
        a.state = crate::State::Asleep;
        a.doze.asleep_since = now;
    }
    let i = f.animals.iter().position(|a| a.id == caller).unwrap();
    f.tick += 1;
    assert!(calls::emit(&mut f, i, 1));
    calls::hear(&mut f, &w.view());
    assert_eq!(wakes(&f, &w, adult), Some(crate::rhythm::Wake::Call), "an adult heeds the bleat");
    assert_eq!(wakes(&f, &w, young), None, "a calf does not heed a bleat");
    // Lying down after the call: it does not wake to it.
    let t = f.tick + 1;
    animal_mut(&mut f, adult).doze.asleep_since = t;
    assert_eq!(wakes(&f, &w, adult), None);
}

/// In light rest a sleeper wakes to a stalking hound it sees moving within its reduced range;
/// in deep sleep the same hound does not wake it.
#[test]
fn a_light_rester_wakes_to_a_stalking_hound_it_sees_and_a_deep_sleeper_does_not() {
    let w = world(47);
    let mut c = config();
    sight_only(&mut c, 5.0);
    rest_table(&mut c).light = true;
    rest_table(&mut c).rest_sense = 0.5;
    for light in [true, false] {
        let mut f = Fauna::new(c.clone());
        f.set_births_enabled(false);
        let me = sleeper(&mut f, &w, 2.0, light);
        let speed = cruise(&f, VH);
        body(&mut f, &w, VH, 2.0 + 0.8 * 2.5, WEST, speed);
        sense(&mut f, &w);
        assert_eq!(wakes(&f, &w, me), light.then_some(crate::rhythm::Wake::Sight), "light {light}");
    }
}

// ------------------------------------------------------------------ step 5: rest sites

/// **A resting loftstrider picks a site without hound scent over an otherwise equal one that
/// has it**: on a field a hound laid at one end (the hound then gone), a body about to rest on
/// the scented spot keeps looking (asks to stay up) and walks down the scent, not feeding; the
/// same body on the far, unscented spot of the same plain lies down there. Once its search time
/// is spent it rests wherever it is.
#[test]
fn a_resting_loftstrider_picks_a_site_without_hound_scent() {
    let w = world(51);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.scent.half_life_s = 1e12;
    let t = rest_table(&mut c);
    t.bed_scent = 1.0;
    t.bed_search_s = 5.0;
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let hound = body(&mut f, &w, VH, 1.0, EAST, 0.0);
    for _ in 0..40 {
        let mut field = std::mem::take(&mut f.scent);
        field.update(&w.view(), &flora.view(), &f.animals, &f.config);
        f.scent = field;
    }
    f.animals.retain(|a| a.id != hound);
    let floor = f.config.founder(LS).senses.smell_min;
    let me = body(&mut f, &w, LS, 2.0, WEST, 0.0);
    let i = f.animals.iter().position(|a| a.id == me).unwrap();
    let smell = |f: &Fauna, x: f64| {
        let mut a = animal(f, me);
        a.pose.x = x;
        a.site.x = (x / VOXEL_M) as u32;
        f.scent.read(&w.view(), &a)[crate::scent::ScentChannel::Hunter.index()]
    };
    let (near, far) = (smell(&f, 2.0), smell(&f, 7.0));
    assert!(near.level >= floor && far.level < floor, "the fixture: scent {} at the near spot, {} at the far, floor {floor}", near.level, far.level);
    assert!(crate::sleep::bed_ask(&mut f, i, &w.view()), "on the scented spot it keeps looking");
    let go = crate::sleep::bed_reflex(&f, i, &w.view()).expect("it walks to a bed");
    assert_eq!(go.feed, 0.0);
    // Down the gradient: the scent's gradient points back at the source (west), so it turns
    // east from facing west.
    assert!(near.gx < 0.0 && go.turn != 0.0, "turns away from the source: {go:?}");
    // The same body on the unscented spot lies down there.
    place(&mut f, me, 7.0, LANE_Z, WEST);
    animal_mut(&mut f, me).doze.bed_until = 0;
    assert!(!crate::sleep::bed_ask(&mut f, i, &w.view()), "no scent: it rests here");
    // Back on the scent with its search spent: it rests anyway.
    place(&mut f, me, 2.0, LANE_Z, WEST);
    f.tick += cubarium_voxel::ticks_at_least_one(5.0) + 1;
    assert!(!crate::sleep::bed_ask(&mut f, i, &w.view()), "search spent");
    assert!(crate::sleep::bed_reflex(&f, i, &w.view()).is_none());
}

// ------------------------------------------------------------------ step 6: kin and the mark

/// A family member weighs up to its lineage's `kin_tracks` of its own kind each sensing, nearest
/// first, for the loftstrider and the Veilhound alike (read from the config).
#[test]
fn a_family_or_pack_member_sees_up_to_its_kin_tracks_of_its_group() {
    let w = world(61);
    for lineage in [LS, VH] {
        let mut c = config();
        c.founders[lineage.index()].senses = crate::FaunaConfig::default().founder(lineage).senses;
        let cap = c.founder(lineage).senses.kin_tracks as usize;
        let mut f = Fauna::new(c);
        f.set_births_enabled(false);
        let me = body(&mut f, &w, lineage, 2.0, EAST, 0.0);
        for k in 0..cap + 2 {
            body(&mut f, &w, lineage, 2.5 + 0.5 * k as f64, EAST, 0.0);
        }
        sense(&mut f, &w);
        let kin = f.percept(me).unwrap().tracks().iter().filter(|t| t.class == perception::class::KIN).count();
        assert_eq!(kin, cap, "{}", lineage.name());
    }
}

/// **The "mine" mark**: a mother and her calf each see the other marked in their kin rows (the
/// relation channel: mother for the calf, own calf for the mother); another relative is plain
/// kin.
#[test]
fn a_mother_and_her_calf_carry_the_mine_mark_on_each_other() {
    use crate::intent::Subject;
    use crate::subjects::candidates;
    let w = world(62);
    let mut f = Fauna::new(config());
    f.set_births_enabled(false);
    let mother = body(&mut f, &w, LS, 2.0, EAST, 0.0);
    let young = calf(&mut f, &w, 3.0, WEST);
    let other = body(&mut f, &w, LS, 2.5, EAST, 0.0);
    animal_mut(&mut f, young).mother = Some(mother);
    sense(&mut f, &w);
    let row = |f: &Fauna, me: u64, of: u64| {
        let i = f.animals.iter().position(|a| a.id == me).unwrap();
        candidates(f, i, LS, Subject::None).rows.iter().find(|r| r.subject.body() == Some(of)).map(|r| r.row).expect("a kin row")
    };
    let (calf_sees_mother, mother_sees_calf, mother_sees_other) = (row(&f, young, mother), row(&f, mother, young), row(&f, mother, other));
    assert_eq!((calf_sees_mother[20], calf_sees_mother[21]), (1.0, 0.0), "her mother");
    assert_eq!((mother_sees_calf[20], mother_sees_calf[21]), (0.0, 1.0), "her own calf");
    assert_eq!((mother_sees_other[20], mother_sees_other[21], mother_sees_other[22]), (0.0, 0.0, 1.0), "plain kin");
}
