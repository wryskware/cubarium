//! **LS-DEFEND law tests** (`design/behaviour-ai-2026-10-11.md` §7's loftstrider tests; the
//! brief `design/handoffs/ls-defend-2026-10-12.md`, "Checks").
//!
//! Mechanisms on hand-built fixtures: a loftstrider's senses are fixed by the case (its
//! percept record and call records) and one sample of its defence layer is read. Every number
//! a case relies on is read from the config; no tuned default, ratio or verdict is asserted.
//! Nothing steps past a few hundred ticks.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use super::*;
use crate::calls::{CallRecord, Heard};
use crate::perception::{SIGHT, Track, class, posture};
use crate::{Activity, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use fixture::{EAST, SURFACE_Y, VOXEL_M, WEST, animal, animal_mut, drive, hunting_config, introduce, place};

const CH: Founder = Founder::Chorister;
const LS: Founder = Founder::Loftstrider;

// ------------------------------------------------------------------ fixture

/// Arena columns and rows (0.25 m voxels): 40 m around the ring, 30 m deep.
const W: u32 = 160;
const D: u32 = 120;
const RING: f64 = W as f64 * VOXEL_M;
const MID_Z: f64 = D as f64 * VOXEL_M / 2.0;

/// A flat soil arena, the day clock off.
fn arena(seed: u64) -> World {
    let mut w = World::empty(VoxelConfig {
        width: W,
        height: 12,
        depth: D,
        voxel_m: VOXEL_M,
        seed,
        day: cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() },
        ..VoxelConfig::default()
    });
    for z in 0..D {
        for x in 0..i64::from(W) {
            for y in 1..=SURFACE_Y {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c
}

fn d(f: &Fauna) -> Defence {
    f.config.founder(LS).defence
}

/// A loftstrider at `(x, z)` facing `heading`: grown, or a calf at a fifth of the grown body.
fn strider(f: &mut Fauna, w: &World, young: bool, x: f64, z: f64, heading: f64) -> u64 {
    let id = introduce(f, w, LS, StartingStores::FULL);
    if young {
        let max = f.config.founder(LS).core.body_max;
        animal_mut(f, id).body = 0.2 * max;
    }
    place(f, id, x, z, heading);
    drive(f, id, 0.0, false);
    id
}

/// A Veilhound at `(x, z)` facing `heading`, still (no controller steps it).
fn hound(f: &mut Fauna, w: &World, x: f64, z: f64, heading: f64) -> u64 {
    let id = introduce(f, w, CH, StartingStores::FULL);
    place(f, id, x, z, heading);
    drive(f, id, 0.0, false);
    id
}

fn index(f: &Fauna, id: u64) -> usize {
    f.animals.iter().position(|a| a.id == id).expect("a live body")
}

fn length(f: &Fauna, id: u64) -> f64 {
    crate::body_of(&f.config, &animal(f, id)).expect("a body").length_m
}

fn radius(f: &Fauna, id: u64) -> f64 {
    crate::body_of(&f.config, &animal(f, id)).expect("a body").footprint_radius()
}

/// What body `me` sees of body `id` now: a sight track as the senses write it; `moving`
/// makes a threat pressing.
fn seen(f: &Fauna, me: u64, id: u64, cls: u8, moving: bool) -> Track {
    let (m, o) = (animal(f, me), animal(f, id));
    let (dx, dz) = crate::pose::wrapped_delta(m.pose.x, m.pose.z, o.pose.x, o.pose.z, RING);
    Track {
        known: true,
        id,
        lineage: o.founder.expect("a lineage").index() as u8,
        class: cls,
        modes: SIGHT,
        x: o.pose.x,
        z: o.pose.z,
        y_m: o.pose.y_m,
        site: (o.site.x, o.site.y, o.site.z),
        grounded: true,
        moving,
        d_m: (dx * dx + dz * dz).sqrt(),
        size_m: length(f, id),
        tick: f.tick,
        heading: o.pose.heading_rad,
        posture: posture::ACTIVE,
        ..Track::default()
    }
}

/// Body `me` sees exactly `bodies` now: `(id, class, moving)`.
fn sees(f: &mut Fauna, me: u64, bodies: &[(u64, u8, bool)]) {
    let tracks: Vec<Track> = bodies.iter().map(|&(id, c, m)| seen(f, me, id, c, m)).collect();
    let mut p = f.percepts.get(&me).copied().unwrap_or_default();
    p.tracks = [Track::default(); crate::perception::MAX_TRACKS];
    p.tracks[..tracks.len()].copy_from_slice(&tracks);
    p.n = tracks.len() as u8;
    p.tick = f.tick;
    f.percepts.insert(me, p);
}

/// Body `me` has heard a call of `kind` from `(x, z)` now, `relation` its recognition of the
/// voice (0 a stranger, 2 its own young).
fn hears(f: &mut Fauna, me: u64, kind: u8, (x, z): (f64, f64), relation: u8) {
    let r = CallRecord { kind, x, z, y_m: 0.0, loudness: 1.0, proximity: 1.0, tick: f.tick, relation, asleep: false, heading: 0.0 };
    f.heard.insert(me, Heard { records: vec![r] });
}

fn world(seed: u64) -> (World, Fauna) {
    let w = arena(seed);
    let mut f = Fauna::new(config());
    f.world_geometry = Some((RING, VOXEL_M));
    f.tick = 1000;
    (w, f)
}

/// One sample of body `me`'s defence layer.
fn decide(f: &mut Fauna, w: &World, me: u64) -> Option<crate::Actions> {
    let i = index(f, me);
    steer(f, i, &w.view())
}

/// Body `me`'s defence hold cleared: its next sample chooses afresh.
fn fresh_hold(f: &mut Fauna, me: u64) {
    let v = &mut f.percepts.entry(me).or_default().vigil;
    v.defend_held = 0;
    v.defend_since = 0;
}

fn held(f: &Fauna, me: u64) -> Option<&'static str> {
    f.ls_teach.get(&me).and_then(|t| t.held)
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dz) = crate::pose::wrapped_delta(a.0, a.1, b.0, b.1, RING);
    dx.hypot(dz)
}

/// Whether actions `act` of body `me` head toward `(x, z)`: turning toward it, or walking with
/// it within 60° ahead.
fn heads_toward(f: &Fauna, me: u64, act: crate::Actions, (x, z): (f64, f64)) -> bool {
    let a = animal(f, me);
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, RING);
    let want = dx.atan2(dz);
    let off = {
        let d = (want - a.pose.heading_rad).rem_euclid(std::f64::consts::TAU);
        if d > std::f64::consts::PI { d - std::f64::consts::TAU } else { d }
    };
    if off.abs() <= std::f64::consts::FRAC_PI_3 { act.forward > 0.0 } else { act.turn.signum() == off.signum() }
}

// ------------------------------------------------------------------ step 1: the intent layer

#[test]
fn a_calf_with_a_pressing_threat_runs_to_the_far_side_of_the_nearest_adult_and_settles() {
    let (w, mut f) = world(1);
    // A calf at 10 m, two adults east of it (the nearer at 14 m), a hound pressing from the
    // west at 6 m.
    let calf = strider(&mut f, &w, true, 10.0, MID_Z, EAST);
    let near = strider(&mut f, &w, false, 14.0, MID_Z + 1.0, WEST);
    let far = strider(&mut f, &w, false, 18.0, MID_Z - 1.0, WEST);
    let vh = hound(&mut f, &w, 6.0, MID_Z, EAST);
    sees(&mut f, calf, &[(near, class::KIN, false), (far, class::KIN, false), (vh, class::THREAT, true)]);
    let act = decide(&mut f, &w, calf).expect("a calf with a threat defends");
    assert_eq!(held(&f, calf), Some("go_to_adults"));
    assert_eq!(f.ls_teach[&calf].subject.body(), Some(near), "the nearest adult");
    // The far side of the adult from the hound, clear of both footprints.
    let gap = radius(&f, near) + radius(&f, calf) + d(&f).guard_gap_m;
    let (n, h) = (animal(&f, near).pose, animal(&f, vh).pose);
    let along = |m: f64| {
        let (dx, dz) = crate::pose::wrapped_delta(n.x, n.z, h.x, h.z, RING);
        let l = dx.hypot(dz);
        (n.x - dx / l * m, n.z - dz / l * m)
    };
    let point = along(gap);
    assert!(heads_toward(&f, calf, act, point), "toward the far point {point:?}: {act:?}");
    // There, it settles.
    place(&mut f, calf, point.0, point.1, EAST);
    sees(&mut f, calf, &[(near, class::KIN, false), (far, class::KIN, false), (vh, class::THREAT, true)]);
    let act = decide(&mut f, &w, calf).expect("still defending");
    assert_eq!(act.forward, 0.0, "settled on the far side");
}

#[test]
fn an_adult_hearing_a_distress_call_goes_to_the_guard_point_between_the_calf_and_the_hound() {
    let (w, mut f) = world(2);
    let adult = strider(&mut f, &w, false, 22.0, MID_Z, WEST);
    let calf = strider(&mut f, &w, true, 10.0, MID_Z + 2.0, EAST);
    let vh = hound(&mut f, &w, 6.0, MID_Z + 2.0, EAST);
    animal_mut(&mut f, calf).mother = Some(adult);
    // It hears its own calf's bleat; it sees the hound but not the calf.
    let at = (animal(&f, calf).pose.x, animal(&f, calf).pose.z);
    hears(&mut f, adult, 1, at, 2);
    sees(&mut f, adult, &[(vh, class::THREAT, true)]);
    let act = decide(&mut f, &w, adult).expect("a heeded bleat engages it");
    assert_eq!(held(&f, adult), Some("go_to_calf"));
    let (c, h) = (animal(&f, calf).pose, animal(&f, vh).pose);
    let (dx, dz) = crate::pose::wrapped_delta(c.x, c.z, h.x, h.z, RING);
    let l = dx.hypot(dz);
    let m = radius(&f, adult) + d(&f).guard_gap_m;
    let point = (c.x + dx / l * m, c.z + dz / l * m);
    assert!(dist(point, (c.x, c.z)) < dist((h.x, h.z), (c.x, c.z)), "the point lies between calf and hound");
    assert!(heads_toward(&f, adult, act, point), "toward the guard point {point:?}: {act:?}");
}

#[test]
fn a_stranger_answers_a_bleat_only_after_its_delay() {
    let (w, mut f) = world(3);
    let adult = strider(&mut f, &w, false, 22.0, MID_Z, WEST);
    let calf = strider(&mut f, &w, true, 10.0, MID_Z, EAST);
    let at = (animal(&f, calf).pose.x, animal(&f, calf).pose.z);
    hears(&mut f, adult, 1, at, 0);
    sees(&mut f, adult, &[]);
    assert!(decide(&mut f, &w, adult).is_none(), "a stranger's reaction waits");
    // Its delay later (its ground response keeps when its heeding began).
    let delay = f.config.founder(LS).calls.stranger_delay_s[1];
    assert!(delay + 0.5 < f.config.founder(LS).calls.heed_s[1], "the case stays inside the heed window");
    f.percepts.entry(adult).or_default().vigil.heard_from = f.tick;
    f.tick += ((delay + 0.5) / crate::DT) as u64;
    assert!(decide(&mut f, &w, adult).is_some(), "then it goes");
    assert_eq!(held(&f, adult), Some("go_to_calf"));
}

#[test]
fn a_grown_loftstrider_does_not_flee_a_pair_of_hounds_and_a_lone_calf_still_runs() {
    let (w, mut f) = world(4);
    let adult = strider(&mut f, &w, false, 20.0, MID_Z, EAST);
    let a = hound(&mut f, &w, 23.0, MID_Z + 1.0, WEST);
    let b = hound(&mut f, &w, 23.0, MID_Z - 1.0, WEST);
    let vg = f.config.founder(LS).vigilance;
    assert!(vg.group_min <= 2, "the case puts the old flight's group within reach");
    let fv = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());
    let respond = |f: &mut Fauna, id: u64| {
        let i = index(f, id);
        crate::vigil::respond(f, i, &w.view(), &fv.view(), crate::Actions::REST);
        f.percepts[&id].vigil.fleeing
    };
    sees(&mut f, adult, &[(a, class::THREAT, true), (b, class::THREAT, true)]);
    assert!(!respond(&mut f, adult), "the defence retires the group flight");
    // The old rule, for comparison: the defence off, it flees.
    let mut g = Fauna::new({
        let mut c = config();
        c.founders[LS.index()].defence.enabled = false;
        c
    });
    g.world_geometry = Some((RING, VOXEL_M));
    g.tick = 1000;
    let adult2 = strider(&mut g, &w, false, 20.0, MID_Z, EAST);
    let a2 = hound(&mut g, &w, 23.0, MID_Z + 1.0, WEST);
    let b2 = hound(&mut g, &w, 23.0, MID_Z - 1.0, WEST);
    sees(&mut g, adult2, &[(a2, class::THREAT, true), (b2, class::THREAT, true)]);
    assert!(respond(&mut g, adult2), "without it, the old flight");
    // A lone calf (no adult known) still runs.
    let calf = strider(&mut f, &w, true, 10.0, MID_Z, EAST);
    let c = hound(&mut f, &w, 13.0, MID_Z, WEST);
    sees(&mut f, calf, &[(c, class::THREAT, true)]);
    assert!(respond(&mut f, calf), "a calf alone flees");
    assert!(decide(&mut f, &w, calf).is_none(), "no adult to run to: the flight stands");
}

#[test]
fn an_adult_watches_a_still_hound_moves_off_from_one_approaching_at_range_and_guards_one_inside_guard_m() {
    let (w, mut f) = world(5);
    let dd = d(&f);
    let adult = strider(&mut f, &w, false, 20.0, MID_Z, EAST);
    // Still, at range: watch, facing it.
    let vh = hound(&mut f, &w, 20.0 + 0.5 * (dd.guard_m + dd.threat_m), MID_Z, WEST);
    sees(&mut f, adult, &[(vh, class::THREAT, false)]);
    let act = decide(&mut f, &w, adult).expect("a hound seen engages it");
    assert_eq!(held(&f, adult), Some("watch"));
    assert_eq!(act.forward, 0.0);
    // Approaching at the same range: move off, away from it.
    fresh_hold(&mut f, adult);
    sees(&mut f, adult, &[(vh, class::THREAT, true)]);
    let act = decide(&mut f, &w, adult).expect("move off");
    assert_eq!(held(&f, adult), Some("move_off"));
    assert!(!heads_toward(&f, adult, act, (animal(&f, vh).pose.x, animal(&f, vh).pose.z)) || act.forward == 0.0);
    // Inside guard_m: guard, facing it.
    fresh_hold(&mut f, adult);
    place(&mut f, vh, 20.0 + 0.5 * dd.guard_m, MID_Z, WEST);
    sees(&mut f, adult, &[(vh, class::THREAT, true)]);
    let act = decide(&mut f, &w, adult).expect("guard");
    assert_eq!(held(&f, adult), Some("guard"));
    assert_eq!(f.ls_teach[&adult].subject.body(), Some(vh));
    assert_eq!(act.forward, 0.0, "it holds its ground");
}

#[test]
fn a_held_guard_ends_early_when_its_hound_is_no_longer_sensed() {
    let (w, mut f) = world(6);
    let dd = d(&f);
    let adult = strider(&mut f, &w, false, 20.0, MID_Z, EAST);
    let vh = hound(&mut f, &w, 20.0 + 0.5 * dd.guard_m, MID_Z, WEST);
    sees(&mut f, adult, &[(vh, class::THREAT, true)]);
    decide(&mut f, &w, adult).expect("guard");
    // Record it as the write side does.
    let i = index(&f, adult);
    let teach = f.ls_teach[&adult];
    crate::intent::record(&mut f, i, LS, crate::Steer::Defend, teach, &[true; 10], crate::Executed::default());
    assert_eq!(crate::intent::LOFTSTRIDER.name(usize::from(animal(&f, adult).intent.held)), "guard");
    // The hound gone: under the hold, its guard stands; past it, the calm rules run.
    sees(&mut f, adult, &[]);
    f.tick += 1;
    let since = f.percepts[&adult].vigil.defend_since;
    assert!((f.tick - since) as f64 * crate::DT < f.config.founder(LS).intent.hold_s, "inside the hold");
    assert!(decide(&mut f, &w, adult).is_none(), "nothing to guard: the intent is masked, its hold ends early");
}
