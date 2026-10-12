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
    let (act, kick) = steer(f, i, &w.view())?;
    if let Some(t) = kick {
        kick_now(f, i, t);
    }
    Some(act)
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
    // No charge: this case reads the guard's stand (the charge is step 3's case).
    f.config.founders[LS.index()].defence.charge_m = 0.0;
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

// ------------------------------------------------------------------ step 2: guard stance, spread, checks

/// A rock wall `rise` voxels over the plain, on columns `x0..=x1`, rows `z0..=z1`.
fn wall(w: &mut World, (x0, x1): (i64, i64), (z0, z1): (u32, u32), rise: u32) {
    for x in x0..=x1 {
        for z in z0..=z1 {
            for y in SURFACE_Y + 1..=SURFACE_Y + rise {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
            }
        }
    }
}

fn heading_to(f: &Fauna, from: u64, to: u64) -> f64 {
    let (a, b) = (animal(f, from).pose, animal(f, to).pose);
    let (dx, dz) = crate::pose::wrapped_delta(a.x, a.z, b.x, b.z, RING);
    dx.atan2(dz)
}

#[test]
fn two_adults_facing_two_hounds_from_different_sectors_do_not_both_face_the_same_one() {
    let (w, mut f) = world(7);
    let g = d(&f).guard_m;
    let s = g / 6.0;
    let a = strider(&mut f, &w, false, 20.0, MID_Z, EAST);
    let b = strider(&mut f, &w, false, 20.0, MID_Z + 2.0 * s, EAST);
    let h1 = hound(&mut f, &w, 20.0 + 3.0 * s, MID_Z + 0.5 * s, WEST);
    let h2 = hound(&mut f, &w, 20.0 + 3.0 * s, MID_Z + 5.0 * s, WEST);
    let dd = |x: u64, y: u64| dist((animal(&f, x).pose.x, animal(&f, x).pose.z), (animal(&f, y).pose.x, animal(&f, y).pose.z));
    assert!(dd(a, h1) < dd(b, h1) && dd(b, h1) < dd(b, h2) && dd(b, h2) <= g && dd(a, h2) <= g, "the case: both hounds in reach, h1 nearest both, a nearest h1");
    // Both turn to face h1, the nearest.
    for id in [a, b] {
        let h = heading_to(&f, id, h1);
        animal_mut(&mut f, id).pose.heading_rad = h;
    }
    for id in [a, b] {
        let other = if id == a { b } else { a };
        sees(&mut f, id, &[(other, class::KIN, false), (h1, class::THREAT, true), (h2, class::THREAT, true)]);
    }
    decide(&mut f, &w, a).expect("a guards");
    decide(&mut f, &w, b).expect("b guards");
    assert_eq!(held(&f, a), Some("guard"));
    assert_eq!(held(&f, b), Some("guard"));
    let (ta, tb) = (f.ls_teach[&a].subject.body(), f.ls_teach[&b].subject.body());
    assert_eq!(ta, Some(h1), "a, nearer h1, keeps it");
    assert_eq!(tb, Some(h2), "b leaves h1 to a and faces h2");
}

#[test]
fn a_calf_passes_over_an_adult_it_cannot_reach_safely() {
    // The nearest adult across a hound: it takes the farther one whose way is clear.
    let (w, mut f) = world(8);
    let dd = d(&f);
    let calf = strider(&mut f, &w, true, 10.0, MID_Z, EAST);
    let near = strider(&mut f, &w, false, 14.0, MID_Z, WEST);
    let other = strider(&mut f, &w, false, 10.0 - 5.0, MID_Z, EAST);
    let vh = hound(&mut f, &w, 12.0, MID_Z + 0.5 * dd.cross_m, WEST);
    sees(&mut f, calf, &[(near, class::KIN, false), (other, class::KIN, false), (vh, class::THREAT, true)]);
    decide(&mut f, &w, calf).expect("defends");
    assert_eq!(f.ls_teach[&calf].subject.body(), Some(other), "the way to the nearest crosses the hound");
    // A rock wall it cannot step: the same.
    let (mut w2, mut g) = world(9);
    let calf = strider(&mut g, &w2, true, 10.0, MID_Z, EAST);
    let near = strider(&mut g, &w2, false, 14.0, MID_Z, WEST);
    let other = strider(&mut g, &w2, false, 4.0, MID_Z, EAST);
    let vh = hound(&mut g, &w2, 10.0, MID_Z - 4.0, EAST);
    let col = (12.0 / VOXEL_M) as i64;
    wall(&mut w2, (col, col + 1), (0, D - 1), 8);
    sees(&mut g, calf, &[(near, class::KIN, false), (other, class::KIN, false), (vh, class::THREAT, true)]);
    decide(&mut g, &w2, calf).expect("defends");
    assert_eq!(g.ls_teach[&calf].subject.body(), Some(other), "a wall between it and the nearest");
}

#[test]
fn an_adult_guarding_shows_the_guard_stance_and_a_hound_sees_it() {
    let (w, mut f) = world(10);
    let mut flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());
    let g = d(&f).guard_m;
    let adult = introduce(&mut f, &w, LS, StartingStores::FULL);
    place(&mut f, adult, 20.0, MID_Z, EAST);
    let teachers = f.config.teachers;
    f.controllers.set(adult, crate::controller::heuristic(LS, teachers));
    let vh = hound(&mut f, &w, 20.0 + 0.6 * g, MID_Z, WEST);
    drive(&mut f, vh, 0.3, false);
    assert_eq!(animal(&f, adult).stance, crate::perception::stance::NONE);
    let mut guarded = false;
    for _ in 0..(3.0 / crate::DT) as usize {
        f.step(&w, &mut flora);
        if animal(&f, adult).stance == crate::perception::stance::GUARD {
            guarded = true;
            break;
        }
    }
    assert!(guarded, "a pressing hound inside guard_m: the guard stance");
    // The hound's sight carries it at its next sensing.
    let mut shown = false;
    for _ in 0..(2.0 / crate::DT) as usize {
        f.step(&w, &mut flora);
        if f.percept(vh).and_then(|p| p.track(adult)).is_some_and(|t| t.stance == crate::perception::stance::GUARD) {
            shown = true;
            break;
        }
    }
    assert!(shown, "the hound's track of it shows the guard stance");
    // Asleep it shows none.
    let mut a = animal(&f, adult);
    a.state = crate::State::Asleep;
    assert_eq!(crate::perception::stance_of(&a), crate::perception::stance::NONE);
}

#[test]
fn the_hounds_target_value_reads_the_guard_stance() {
    use crate::pack::tactics::{Hound, Me, assess};
    let f = Fauna::new(config());
    let pk = f.config.founder(CH).pack.expect("the Veilhound's pack table");
    let tac = pk.tactics;
    let ls_len = f.config.founder(LS).adult_length_m;
    let me = Me { x: 10.0, z: MID_Z, len: f.config.founder(CH).adult_length_m, wound: 0.0, hunger: 0.5, spent: 0.0, body: f.config.founder(CH).core.body_max, usable: f64::INFINITY };
    let tr = |id: u64, x: f64, stance: u8| Track {
        known: true, id, lineage: LS.index() as u8, class: class::PREY, modes: SIGHT, x, z: MID_Z, grounded: true,
        size_m: ls_len, heading: WEST, stance, ..Track::default()
    };
    let target = tr(1, 14.0, crate::perception::stance::NONE);
    let plain = [target, tr(2, 15.0, crate::perception::stance::NONE)];
    let braced = [target, tr(2, 15.0, crate::perception::stance::GUARD)];
    let mem = Hound::default();
    let a = assess(&f.config, &tac, &pk, &me, &mem, &target, &plain, &[], f.tick, RING);
    let b = assess(&f.config, &tac, &pk, &me, &mem, &target, &braced, &[], f.tick, RING);
    assert!(b.p_kill <= a.p_kill && b.expected_kicks >= a.expected_kicks, "a guard in stance never helps the hound");
    if tac.guard_factor < 1.0 {
        assert!(b.p_kill < a.p_kill, "guard_factor below 1: the stance lowers p_kill");
    }
    if tac.kick_guard_gain > 0.0 && tac.kick_chance > 0.0 {
        assert!(b.expected_kicks > a.expected_kicks, "kick_guard_gain: more kicks expected");
    }
}

// ------------------------------------------------------------------ step 3: the kick

use crate::kick::{Phase, Refused, Source};

fn kick_of(f: &Fauna) -> crate::kick::Kick {
    f.config.founder(LS).kick.expect("the loftstrider kicks")
}

/// A grown loftstrider facing east with a grown hound `ahead_m` (centre to centre) in front,
/// neither dodging nor healing.
fn kick_scene(seed: u64, ahead_m: f64) -> (World, Fauna, u64, u64) {
    let (w, mut f) = world(seed);
    f.config.founders[CH.index()].agility = 0.0;
    f.config.predation.wound_heal_per_s = 0.0;
    let adult = strider(&mut f, &w, false, 20.0, MID_Z, EAST);
    let vh = hound(&mut f, &w, 20.0 + ahead_m, MID_Z, WEST);
    sees(&mut f, adult, &[(vh, class::THREAT, true)]);
    (w, f, adult, vh)
}

/// Centre-to-centre distance at which the kick's reach (from its origin) just takes in a
/// hound's footprint, less a margin.
fn in_reach(f: &Fauna, adult: u64, vh: u64) -> f64 {
    let k = kick_of(f);
    k.origin_forward_fraction * length(f, adult) + 0.5 * (k.reach_m + radius(f, vh))
}

fn steps(f: &mut Fauna, w: &World, flora: &mut cubarium_voxel_flora::Flora, s: f64) {
    for _ in 0..(s / crate::DT).round() as usize {
        f.step(w, flora);
    }
}

#[test]
fn the_loftstrider_kicks_by_default_and_no_other_lineage_does() {
    let c = FaunaConfig::default();
    for f in Founder::ALL {
        assert_eq!(c.founder(f).kick.is_some(), f == LS, "{}", f.name());
    }
}

#[test]
fn a_guard_kicks_a_hound_in_reach_only_after_the_windup_then_recovers_on_one_budget() {
    let (w, mut f, adult, vh) = kick_scene(11, 0.0);
    let ahead = in_reach(&f, adult, vh);
    place(&mut f, vh, 20.0 + ahead, MID_Z, WEST);
    sees(&mut f, adult, &[(vh, class::THREAT, true)]);
    let k = kick_of(&f);
    let mut flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());
    decide(&mut f, &w, adult).expect("guard");
    assert!(f.ls_teach[&adult].kick, "a hound in reach and arc: the aimed kick");
    assert_eq!(animal(&f, adult).kick.phase, Phase::Windup);
    assert_eq!(animal(&f, adult).kick.source, Source::Aimed);
    let windup = (k.windup_s / crate::DT).round() as usize;
    for _ in 0..windup - 1 {
        f.step(&w, &mut flora);
        assert_eq!(animal(&f, vh).wound, 0.0, "nothing lands in the windup");
    }
    f.step(&w, &mut flora);
    let size = |id: u64| f.config.founder(animal(&f, id).founder.unwrap()).size(animal(&f, id).body);
    let expected = k.damage * size(adult) / size(vh);
    assert!((animal(&f, vh).wound - expected).abs() < 1e-9, "damage × size ratio: {} vs {expected}", animal(&f, vh).wound);
    assert_eq!(animal(&f, adult).kick.phase, Phase::Recovery);
    assert_eq!(f.ledger.kicks_landed_by_founder[LS.index()], 1);
    // One budget: the kick-back a bite would draw is refused through the recovery, as is the
    // layer's next aimed kick.
    assert!(!crate::kick::kick_back(&mut f, adult, vh));
    let i = index(&f, adult);
    assert_eq!(crate::kick::request(&mut f, i, vh, Source::Aimed), Err(Refused::Busy));
    sees(&mut f, adult, &[(vh, class::THREAT, true)]);
    decide(&mut f, &w, adult);
    assert!(!f.ls_teach.get(&adult).is_some_and(|t| t.kick), "no aimed kick while recovering");
    steps(&mut f, &w, &mut flora, k.recovery_s);
    assert_eq!(animal(&f, adult).kick.phase, Phase::Ready);
}

#[test]
fn a_guard_charges_a_hound_near_its_post_and_does_not_kick_one_behind_it() {
    let (w, mut f, adult, vh) = kick_scene(12, 0.0);
    let dd = d(&f);
    let reach = in_reach(&f, adult, vh) + kick_of(&f).reach_m;
    assert!(reach < dd.charge_m && dd.charge_m <= dd.guard_m, "the case: a hound past the kick's reach, inside the charge");
    place(&mut f, vh, 20.0 + 0.5 * (reach + dd.charge_m), MID_Z, WEST);
    sees(&mut f, adult, &[(vh, class::THREAT, true)]);
    let act = decide(&mut f, &w, adult).expect("guard");
    assert_eq!(held(&f, adult), Some("guard"));
    assert!(act.forward > 0.0, "it charges");
    assert!(!f.ls_teach[&adult].kick);
    // Behind it, in reach of the hoof's distance but out of the arc: no kick.
    fresh_hold(&mut f, adult);
    let ahead = in_reach(&f, adult, vh);
    place(&mut f, vh, 20.0 - ahead, MID_Z, EAST);
    sees(&mut f, adult, &[(vh, class::THREAT, true)]);
    decide(&mut f, &w, adult).expect("guard");
    assert!(!f.ls_teach[&adult].kick, "out of the front arc");
    assert_eq!(animal(&f, adult).kick.phase, Phase::Ready);
}

/// A hound's bite on a grown loftstrider facing it: (the kick's phase after, kick-backs, the
/// loftstrider's wound). `dodge`: the loftstrider dodges for certain.
fn bite_case(dodge: bool) -> (Phase, u64, f64) {
    let (w, mut f) = world(13);
    let mut flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());
    f.config.predation.awareness_sensed = 1.0;
    f.config.founders[LS.index()].agility = if dodge { 1.0 } else { 0.0 };
    let h = f.config.founder(CH).active_hunter().expect("a hunter");
    let vh = hound(&mut f, &w, 10.0, MID_Z, EAST);
    animal_mut(&mut f, vh).eating = true;
    let g = f.config.founder(CH).body_at(animal(&f, vh).body);
    let adult = strider(&mut f, &w, false, 0.0, MID_Z, WEST);
    let ahead = (h.jaw_forward_length_fraction + 0.5 * h.strike_reach_length_fraction) * g.length_m + 0.5 * radius(&f, adult);
    place(&mut f, adult, 10.0 + ahead, MID_Z, WEST);
    for id in [vh, adult] {
        animal_mut(&mut f, id).pose.y_m = fixture::SURFACE_M;
    }
    sees(&mut f, adult, &[(vh, class::THREAT, true)]);
    let i = index(&f, vh);
    crate::step::start_strike(&mut f, i, adult);
    let done = fixture::step_until(&mut f, &w, &mut flora, 120, |f| {
        f.animals.iter().find(|a| a.id == vh).is_none_or(|a| a.strike.is_none_or(|s| s.phase == crate::StrikePhase::Recovery))
    });
    assert!(done.is_some(), "the strike resolved");
    let a = animal(&f, adult);
    (a.kick.phase, f.ledger.kick_backs, a.wound)
}

#[test]
fn a_landed_bite_draws_a_kick_back_and_a_dodged_one_draws_none() {
    let k = FaunaConfig::default().founder(LS).kick.expect("the loftstrider kicks");
    let (phase, backs, wound) = bite_case(false);
    assert!(wound > 0.0, "the bite landed");
    if k.kick_back {
        assert_eq!(backs, 1, "a landed bite from the front arc: the kick-back");
        assert_ne!(phase, Phase::Ready, "its one kick is spent on it");
    }
    let (phase, backs, wound) = bite_case(true);
    assert_eq!(wound, 0.0, "the bite was dodged");
    assert_eq!(backs, 0, "a dodged bite draws no kick-back");
    assert_eq!(phase, Phase::Ready);
}

#[test]
fn landed_kicks_kill_a_grown_hound_after_as_many_as_its_damage_needs() {
    // At the shipped table (Wrysk, topic 6) that is three; the case reads the count from the
    // configured damage and sizes, with healing off.
    let (w, mut f, adult, vh) = kick_scene(14, 0.0);
    let ahead = in_reach(&f, adult, vh);
    place(&mut f, vh, 20.0 + ahead, MID_Z, WEST);
    let k = kick_of(&f);
    let size = |f: &Fauna, id: u64| f.config.founder(animal(f, id).founder.unwrap()).size(animal(f, id).body);
    let per = k.damage * size(&f, adult) / size(&f, vh);
    assert!(per > 0.0);
    let n = (1.0 / per - 1e-9).ceil() as usize;
    let mut flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());
    for kick in 1..=n {
        assert!(f.animals.iter().any(|a| a.id == vh), "alive before kick {kick}");
        let i = index(&f, adult);
        crate::kick::request(&mut f, i, vh, Source::Aimed).expect("ready");
        steps(&mut f, &w, &mut flora, k.windup_s);
        if kick < n {
            assert!((animal(&f, vh).wound - per * kick as f64).abs() < 1e-9);
            steps(&mut f, &w, &mut flora, k.recovery_s);
        }
    }
    assert!(!f.animals.iter().any(|a| a.id == vh), "dead at the {n}th landed kick");
    assert_eq!(f.ledger.kicks_landed_by_founder[LS.index()], n as u64);
}

// ------------------------------------------------------------------ step 4: the bond

#[test]
fn a_bonded_calf_runs_to_its_mother_over_a_nearer_adult_unless_her_way_crosses_a_hound() {
    let (w, mut f) = world(15);
    let dd = d(&f);
    let calf = strider(&mut f, &w, true, 10.0, MID_Z, EAST);
    let near = strider(&mut f, &w, false, 13.0, MID_Z, WEST);
    let mother = strider(&mut f, &w, false, 10.0, MID_Z + 6.0, WEST);
    animal_mut(&mut f, calf).mother = Some(mother);
    let vh = hound(&mut f, &w, 5.0, MID_Z - 3.0, EAST);
    let all = [(near, class::KIN, false), (mother, class::KIN, false), (vh, class::THREAT, true)];
    sees(&mut f, calf, &all);
    decide(&mut f, &w, calf).expect("defends");
    assert_eq!(f.ls_teach[&calf].subject.body(), Some(mother), "its mother, though another adult is nearer");
    // A hound on the way to her: the nearest adult with a safe way.
    fresh_hold(&mut f, calf);
    place(&mut f, vh, 10.0, MID_Z + 3.0, EAST);
    assert!(0.0 < dd.cross_m);
    sees(&mut f, calf, &all);
    decide(&mut f, &w, calf).expect("defends");
    assert_eq!(f.ls_teach[&calf].subject.body(), Some(near), "her way crosses the hound");
}

#[test]
fn a_mother_goes_to_her_own_calf_before_a_nearer_threatened_one_and_others_still_defend_any_calf() {
    let (w, mut f) = world(16);
    let mother = strider(&mut f, &w, false, 20.0, MID_Z, WEST);
    let mine = strider(&mut f, &w, true, 12.0, MID_Z + 4.0, EAST);
    let other = strider(&mut f, &w, true, 12.0, MID_Z - 4.0, EAST);
    animal_mut(&mut f, mine).mother = Some(mother);
    // Its hound is farther from her own calf than the other's is from the other calf.
    let h1 = hound(&mut f, &w, 8.0, MID_Z + 4.0, EAST);
    let h2 = hound(&mut f, &w, 10.0, MID_Z - 4.0, EAST);
    let all = [(mine, class::KIN, false), (other, class::KIN, false), (h1, class::THREAT, true), (h2, class::THREAT, true)];
    sees(&mut f, mother, &all);
    decide(&mut f, &w, mother).expect("defends");
    assert_eq!(held(&f, mother), Some("go_to_calf"));
    assert_eq!(f.ls_teach[&mother].subject.body(), Some(mine), "her own calf first");
    // An adult that is no calf's mother goes to the calf in most danger.
    let aunt = strider(&mut f, &w, false, 20.0, MID_Z - 1.0, WEST);
    sees(&mut f, aunt, &all);
    decide(&mut f, &w, aunt).expect("defends");
    assert_eq!(held(&f, aunt), Some("go_to_calf"));
    assert_eq!(f.ls_teach[&aunt].subject.body(), Some(other), "the calf nearest its hound");
}

#[test]
fn the_sensed_mother_is_found_for_any_lineage_and_age() {
    let (w, mut f) = world(17);
    for founder in [LS, Founder::Browser] {
        let mother = introduce(&mut f, &w, founder, StartingStores::FULL);
        let grown = introduce(&mut f, &w, founder, StartingStores::FULL);
        place(&mut f, mother, 5.0, MID_Z, EAST);
        place(&mut f, grown, 8.0, MID_Z, WEST);
        animal_mut(&mut f, grown).mother = Some(mother);
        let i = index(&f, grown);
        assert!(crate::nurse::mother_track(&f, i).is_none(), "not in its senses");
        sees(&mut f, grown, &[(mother, class::KIN, false)]);
        assert_eq!(crate::nurse::mother_track(&f, i).map(|t| t.id), Some(mother), "{}", founder.name());
        assert!(!crate::nurse::dependent(&f, i), "a grown body, no nursing: the follow primitive still finds her");
    }
}
