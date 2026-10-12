//! **VH-TACTICS law tests** (`design/behaviour-ai-2026-10-11.md` §7, §11.2, §11.4, §11.5; the
//! brief `design/handoffs/vh-tactics-2026-10-12.md`, "Checks").
//!
//! Mechanisms on hand-built fixtures. Every number a case relies on is read from the config (or
//! set by the case); no tuned default, ratio, verdict or hash is asserted. Nothing steps past a
//! few hundred ticks.

#![allow(dead_code, unused_imports)] // WIP

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use super::*;
use crate::controller::Actions;
use crate::perception::{Percept, SIGHT, Track, class, posture};
use crate::{Activity, Fauna, FaunaConfig, Founder, StartingStores, StrikePhase};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use fixture::{EAST, SURFACE_Y, VOXEL_M, WEST, animal, animal_mut, drive, hunting_config, introduce, place};

const CH: Founder = Founder::Chorister;
const LS: Founder = Founder::Loftstrider;

// ------------------------------------------------------------------ fixture

/// Arena columns and rows (0.25 m voxels): 32 m around the ring, 16 m deep.
const W: u32 = 128;
const D: u32 = 64;
const RING: f64 = W as f64 * VOXEL_M;
/// The arena's middle row, metres.
const MID_Z: f64 = D as f64 * VOXEL_M / 2.0;

/// A flat soil arena, the day clock off.
fn arena(seed: u64) -> World {
    let mut w = World::empty(VoxelConfig {
        width: W,
        height: 10,
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

/// Every lineage awake; the prey with no dodge, flight or ground response (they stand where
/// a case puts them).
fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[LS.index()].agility = 0.0;
    c.founders[LS.index()].vigilance.mode = crate::VigilMode::None;
    c
}

fn tac(f: &Fauna) -> Tactics {
    f.config.founder(CH).pack.expect("the Veilhound's pack table").tactics
}

/// A Veilhound pack of `n` founded on the arena (its den at the west end), every member under
/// its heuristic, hungry, placed by the case. Returns the hounds' ids, lowest first.
fn pack(f: &mut Fauna, w: &World, n: usize) -> Vec<u64> {
    let den = Site { x: 4, y: SURFACE_Y, z: D / 2 };
    f.found_pack_sized(w, den, CH, fixture::HUNTING, 0.0, n).expect("a Veilhound pack");
    f.set_births_enabled(false);
    f.world_geometry = Some((RING, VOXEL_M));
    let teachers = f.config.teachers;
    let mut ids: Vec<u64> = f.animals.iter().filter(|a| a.founder == Some(CH) && a.pack.is_some_and(|m| !m.pup)).map(|a| a.id).collect();
    ids.sort_unstable();
    for &id in &ids {
        f.controllers.set(id, crate::controller::heuristic(CH, teachers));
        animal_mut(f, id).eating = true;
    }
    ids
}

/// A loftstrider at `(x, z)` facing `heading`: grown (`young` false) or a calf of a fifth of
/// the grown body. It stands (no controller steps it).
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

fn index(f: &Fauna, id: u64) -> usize {
    f.animals.iter().position(|a| a.id == id).expect("a live body")
}

fn length(f: &Fauna, id: u64) -> f64 {
    crate::body_of(&f.config, &animal(f, id)).expect("a body").length_m
}

/// What a hound `me` sees of body `id` now: a sight track as the senses would write it.
fn seen(f: &Fauna, me: u64, id: u64, cls: u8) -> Track {
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
        d_m: (dx * dx + dz * dz).sqrt(),
        size_m: length(f, id),
        tick: f.tick,
        heading: o.pose.heading_rad,
        posture: posture::ACTIVE,
        ..Track::default()
    }
}

/// Hound `me`'s record holds exactly `tracks` (its senses fixed by the case).
fn set_percept(f: &mut Fauna, me: u64, tracks: &[Track]) {
    let mut p = Percept::default();
    p.tracks[..tracks.len()].copy_from_slice(tracks);
    p.n = tracks.len() as u8;
    p.tick = f.tick;
    f.percepts.insert(me, p);
}

/// One sample of hound `me`'s tactic, over a still held action.
fn decide(f: &mut Fauna, w: &World, flora: &Flora, me: u64) -> Actions {
    let i = index(f, me);
    tactic(f, i, &w.view(), &flora.view(), false, 1.0, Actions::REST, None).expect("a pack body samples")
}

/// A hand-built track for the pure rules: lineage `l`, length `size`, at `(x, z)` facing
/// `heading`, seen.
fn tr(id: u64, l: Founder, cls: u8, size: f64, (x, z): (f64, f64), heading: f64) -> Track {
    Track { known: true, id, lineage: l.index() as u8, class: cls, modes: SIGHT, x, z, grounded: true, size_m: size, heading, ..Track::default() }
}

fn hound_me(f: &Fauna, x: f64, z: f64) -> Me {
    Me { x, z, len: f.config.founder(CH).adult_length_m, wound: 0.0, hunger: 0.5, spent: 0.0 }
}

fn norm((x, z): (f64, f64)) -> f64 {
    (x * x + z * z).sqrt()
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dz) = crate::pose::wrapped_delta(a.0, a.1, b.0, b.1, RING);
    (dx * dx + dz * dz).sqrt()
}

/// The bearing from `a` to `b`, radians from `+z` toward `+x`.
fn bearing(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dz) = crate::pose::wrapped_delta(a.0, a.1, b.0, b.1, RING);
    dx.atan2(dz)
}

// ------------------------------------------------------------------ encircle (§7, Escobedo)

/// The encircle knobs and a grown loftstrider at bay, as a hound judges it: `dc` and `da`.
fn ring_of(f: &Fauna) -> (Tactics, f64, f64, f64) {
    let t = tac(f);
    let size = f.config.founder(LS).adult_length_m;
    let target = tr(1, LS, class::PREY, size, (0.0, 0.0), 0.0);
    let s = Assess { kicker: true, ..Assess::default() };
    let dc = safe_m(&t, &target, &s, &Hound::default(), 0);
    (t, dc, t.da_factor * dc, t.kick_reach_fraction * size)
}

/// **The two forces, apart** (§7; Astra review #10: `da` is where repulsion switches on, not
/// a gap to keep): outside `da` a hound only approaches, at most at full pace and never past
/// the target; inside `dc` it backs off; inside `da` a packmate within `da` of the target
/// pushes it away from that packmate, one beyond `da` of the target does not.
#[test]
fn encircle_approaches_to_dc_and_repels_only_inside_da() {
    let f = Fauna::new(config());
    let (t, dc, da, _) = ring_of(&f);
    let target = (10.0, 8.0);
    let v = |me: (f64, f64), others: &[(f64, f64)]| {
        encircle_velocity(me, target, others, dc, da, &t, 1.0, RING)
    };
    // Far out, with a packmate beside it: the approach alone, at full pace, straight in.
    let far = (target.0 - (da + t.approach_band_m + 3.0), target.1);
    let beside = (far.0, far.1 + 0.3);
    let u = v(far, &[beside]);
    assert!((norm(u) - 1.0).abs() < 1e-9, "full pace far out: {u:?}");
    assert!(u.0 > 0.0 && u.1.abs() < 1e-9, "straight at the target, the packmate ignored: {u:?}");
    // Bounded: from anywhere on a line through the target, the approach never exceeds full
    // pace and points back out once inside dc.
    for k in 1..=40 {
        let d = 0.05 * f64::from(k) * (da + 2.0 * t.approach_band_m);
        let u = v((target.0 - d, target.1), &[]);
        assert!(norm(u) <= 1.0 + 1e-9, "bounded at {d}");
        if d < dc - 1e-9 {
            assert!(u.0 < 0.0, "inside dc ({d} < {dc}) it backs off: {u:?}");
        } else if d > dc + 1e-9 {
            assert!(u.0 > 0.0, "outside dc ({d} > {dc}) it closes: {u:?}");
        }
    }
    // Inside da: a packmate within da of the target pushes it away from that packmate.
    let me = (target.0 - 0.5 * (dc + da), target.1);
    let alone = v(me, &[]);
    let mate = (target.0 - 0.5 * (dc + da), target.1 + 0.4 * dc);
    let pushed = v(me, &[mate]);
    let push = (pushed.0 - alone.0, pushed.1 - alone.1);
    let away = (me.0 - mate.0, me.1 - mate.1);
    assert!(push.0 * away.0 + push.1 * away.1 > 0.0, "repelled away from the packmate: {push:?}");
    // A packmate beyond da of the target does not repel, however near the hound.
    let outside = (target.0 - (da + 0.5), target.1 + 0.1);
    assert!(dist(outside, target) > da);
    assert_eq!(v(me, &[outside]), alone, "a body beyond da of the target does not repel");
}

/// Integrate hounds under the encircle field (the chase's own step: standing under
/// `still_pace`, else at most full pace for one controller period). Returns the positions
/// after `samples` and the nearest any hound came to the target.
fn settle(f: &Fauna, start: &[(u64, (f64, f64))], target: (f64, f64), guards: &[(f64, f64)], samples: usize) -> (Vec<(f64, f64)>, f64) {
    let (t, dc, da, _) = ring_of(f);
    let m = f.config.manifest(CH);
    let step = m.cruise_m_per_s * m.controller_period_s;
    let mut at: Vec<(f64, f64)> = start.iter().map(|s| s.1).collect();
    let mut nearest = f64::INFINITY;
    for _ in 0..samples {
        let now = at.clone();
        for (k, &(id, _)) in start.iter().enumerate() {
            let others: Vec<(f64, f64)> = now.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, p)| *p).chain(guards.iter().copied()).collect();
            let v = encircle_velocity(now[k], target, &others, dc, da, &t, side_of(id), RING);
            let s = norm(v);
            if s >= t.still_pace {
                let go = s.min(1.0) * step;
                at[k] = (now[k].0 + v.0 / s * go, now[k].1 + v.1 / s * go);
            }
            nearest = nearest.min(dist(at[k], target));
        }
    }
    (at, nearest)
}

/// **N hounds around a standing target settle outside kick reach without overlapping** (§7),
/// from an asymmetric start (all from one side, within a narrow arc) and with a grown kin
/// standing to one side: the approach is bounded (no hound ever comes inside the kick's reach
/// as judged), the bodies end apart, the ring spreads from its arc, and the hounds come to
/// stand (none still moves after the field has settled).
#[test]
fn n_hounds_around_a_standing_target_settle_outside_kick_reach_without_overlapping() {
    let f = Fauna::new(config());
    let (t, dc, da, reach) = ring_of(&f);
    let hound_r = f.config.founder(CH).body_at(f.config.founder(CH).core.body_max).footprint_radius();
    let target = (16.0, 8.0);
    let guard = (target.0 + 0.6 * dc, target.1 + 0.8 * dc);
    for n in 2..=4usize {
        // All from the west, within 30°, 8 m out.
        let start: Vec<(u64, (f64, f64))> = (0..n)
            .map(|k| {
                let a = -FRAC_PI_2 + (k as f64 - 0.5 * (n - 1) as f64) * (30f64.to_radians() / n as f64);
                (100 + k as u64, (target.0 + 8.0 * a.sin(), target.1 + 8.0 * a.cos()))
            })
            .collect();
        let spread0 = arc(&start.iter().map(|s| s.1).collect::<Vec<_>>(), target);
        let (end, nearest) = settle(&f, &start, target, &[guard], 400);
        assert!(nearest >= reach, "{n} hounds: the approach is bounded outside the kick's reach ({nearest} < {reach})");
        for (k, p) in end.iter().enumerate() {
            let d = dist(*p, target);
            assert!(d >= reach && d <= da + t.approach_band_m, "{n} hounds: hound {k} rests in the ring ({d}; reach {reach}, da {da})");
            for q in &end[k + 1..] {
                assert!(dist(*p, *q) >= 2.0 * hound_r, "{n} hounds: bodies apart ({} < {})", dist(*p, *q), 2.0 * hound_r);
            }
        }
        assert!(arc(&end, target) > spread0, "{n} hounds: the ring spreads from its arc");
        // Settled: another round of samples moves nobody.
        let again: Vec<(u64, (f64, f64))> = start.iter().zip(&end).map(|(s, p)| (s.0, *p)).collect();
        let (after, _) = settle(&f, &again, target, &[guard], 1);
        assert_eq!(after, end, "{n} hounds stand once settled");
    }
}

/// The widest angular gap left uncovered around `target`, subtracted from a full turn: how
/// far round the target the bodies spread.
fn arc(ps: &[(f64, f64)], target: (f64, f64)) -> f64 {
    let mut a: Vec<f64> = ps.iter().map(|p| bearing(target, *p).rem_euclid(TAU)).collect();
    a.sort_by(f64::total_cmp);
    let mut widest = TAU - (a[a.len() - 1] - a[0]);
    for w in a.windows(2) {
        widest = widest.max(w[1] - w[0]);
    }
    TAU - widest
}

/// **Symmetric hounds part** (§11.5's per-hound side as a tie-break): two hounds on the same
/// line to the target, one behind the other, with opposite sides, end on different sides of
/// that line; and the tie-break fades once they stand apart (no endless circling).
#[test]
fn hounds_on_one_line_part_by_their_sides() {
    let f = Fauna::new(config());
    let (_, _, da, _) = ring_of(&f);
    let ids: Vec<u64> = (1..64).collect();
    let a = *ids.iter().find(|&&i| side_of(i) > 0.0).unwrap();
    let b = *ids.iter().find(|&&i| side_of(i) < 0.0).unwrap();
    let target = (16.0, 8.0);
    let start = [(a, (target.0 - 0.9 * da, target.1)), (b, (target.0 - 0.9 * da - 0.3, target.1))];
    let (end, _) = settle(&f, &start, target, &[], 400);
    assert!(end[0].1 != target.1 && end[1].1 != target.1);
    assert!((end[0].1 - target.1).signum() != (end[1].1 - target.1).signum(), "they part to either side: {end:?}");
}

// ------------------------------------------------------------------ cut off (§4.2, §11.5)

/// **A cut-off point lies between the target and its nearest defender**, outside the reach
/// of both, `cut_fraction` along the safe stretch when that point is accepted; across the
/// ring's seam too.
#[test]
fn a_cut_off_point_lies_between_the_target_and_its_defender_outside_both_reaches() {
    let f = Fauna::new(config());
    let t = tac(&f);
    let (t_safe, d_safe) = (1.0, 1.5);
    for (calf, adult) in [((10.0, 8.0), (16.0, 10.0)), ((RING - 1.0, 4.0), (3.0, 5.0))] {
        let p = cut_off_point(calf, adult, t_safe, d_safe, t.cut_fraction, RING, &mut |_| true).expect("a safe stretch");
        let (l, a, b) = (dist(calf, adult), dist(calf, p), dist(p, adult));
        assert!((a + b - l).abs() < 1e-9, "on the line between them: {a} + {b} vs {l}");
        assert!(a >= t_safe - 1e-9 && b >= d_safe - 1e-9, "outside both reaches: {a}, {b}");
        assert!((0.0..RING).contains(&p.0), "on the ring: {p:?}");
        let want = t_safe + t.cut_fraction * (l - t_safe - d_safe);
        assert!((a - want).abs() < 1e-9, "cut_fraction along the safe stretch");
    }
}

/// **Reachable and safe, or none** (§11.5): a point the check rejects is passed over for the
/// next safe point along the line; with every point rejected, or no safe stretch (the
/// defender's reach meets the target's), there is no cut-off point.
#[test]
fn a_cut_off_point_that_fails_the_check_is_passed_over_and_none_is_none() {
    let f = Fauna::new(config());
    let t = tac(&f);
    let (calf, adult) = ((10.0, 8.0), (18.0, 8.0));
    let first = cut_off_point(calf, adult, 1.0, 1.0, t.cut_fraction, RING, &mut |_| true).unwrap();
    let mut tried = Vec::new();
    let other = cut_off_point(calf, adult, 1.0, 1.0, t.cut_fraction, RING, &mut |p| {
        tried.push(p);
        dist(p, first) > 1e-6
    })
    .expect("another safe point");
    assert!(dist(other, first) > 1e-6 && tried.len() >= 2);
    assert!(dist(calf, other) >= 1.0 - 1e-9 && dist(other, adult) >= 1.0 - 1e-9);
    assert_eq!(cut_off_point(calf, adult, 1.0, 1.0, t.cut_fraction, RING, &mut |_| false), None, "none accepted");
    assert_eq!(cut_off_point(calf, adult, 5.0, 4.0, t.cut_fraction, RING, &mut |_| true), None, "no safe stretch");
}

// ------------------------------------------------------------------ intercept (§4.2, §11.5)

/// **A feasible interception** (§11.5, tested apart from failed ones): a target crossing the
/// hound's front slower than it is met where both arrive together, within the lead.
#[test]
fn a_feasible_interception_meets_the_target_where_both_arrive_together() {
    let f = Fauna::new(config());
    let t = tac(&f);
    let speed = 2.0;
    let me = (10.0, 8.0);
    let target = (14.0, 4.0);
    let vel = (0.0, 1.0);
    let (p, lead) = intercept_point(me, target, vel, speed, t.lead_max_s, t.lead_min_s, RING, &mut |_| true).expect("feasible");
    assert!(lead > 0.0 && lead <= t.lead_max_s);
    assert!((dist(p, (target.0 + vel.0 * lead, target.1 + vel.1 * lead))) < 1e-9, "where the target will be");
    assert!((dist(me, p) - speed * lead).abs() < 1e-6, "where the hound gets in the same time");
}

/// **Failed interceptions**: a faster target running straight away cannot be headed off; a
/// meeting beyond `lead_max_s` is not taken; terrain that blocks the target's line halves the
/// lead (replanned) to a point it can reach, and blocking every lead down to `lead_min_s`
/// fails it.
#[test]
fn failed_interceptions_return_none_and_a_blocked_lead_is_replanned_shorter() {
    let f = Fauna::new(config());
    let t = tac(&f);
    let me = (10.0, 8.0);
    // Faster and moving away.
    assert_eq!(intercept_point(me, (12.0, 8.0), (3.0, 0.0), 2.0, t.lead_max_s, t.lead_min_s, RING, &mut |_| true), None);
    // Too far ahead for the lead.
    let far = (10.0 + 3.0 * t.lead_max_s * 2.0, 8.0);
    assert_eq!(intercept_point(me, far, (0.0, 0.5), 2.0, t.lead_max_s, t.lead_min_s, RING, &mut |_| true), None);
    // A wall across the target's line: the lead halves to a point short of it.
    let target = (14.0, 4.0);
    let vel = (0.0, 1.0);
    let (open, lead) = intercept_point(me, target, vel, 2.0, t.lead_max_s, t.lead_min_s, RING, &mut |_| true).unwrap();
    let wall_z = 0.5 * (target.1 + open.1);
    let (short, l2) = intercept_point(me, target, vel, 2.0, t.lead_max_s, t.lead_min_s, RING, &mut |p| p.1 < wall_z).expect("replanned");
    assert!(l2 < lead && short.1 < wall_z, "a shorter lead short of the wall");
    assert_eq!(intercept_point(me, target, vel, 2.0, t.lead_max_s, t.lead_min_s, RING, &mut |_| false), None, "every lead blocked");
}


// ------------------------------------------------------------------ target choice (§4.3)

/// The hound at `(15, 2)` assesses `t` among `prey` with no packmates at hand.
fn value_of(f: &Fauna, t: &Track, prey: &[Track]) -> Assess {
    let pk = f.config.founder(CH).pack.unwrap();
    assess(&f.config, &tac(f), &pk, &hound_me(f, 15.0, 2.0), &Hound::default(), t, prey, &[], 0, RING)
}

/// **With config weights an isolated calf outscores a guarded one** (§7): two calves the same
/// distance from the hound, one alone, one with grown kin beside it; and the more of them, and
/// the more face the hound, the less the guarded calf is worth.
#[test]
fn an_isolated_calf_outscores_a_guarded_one() {
    let f = Fauna::new(config());
    let adult = f.config.founder(LS).adult_length_m;
    let calf_len = 0.5 * tac(&f).young_length_fraction * adult;
    let hound = (15.0, 2.0);
    let lone = tr(1, LS, class::PREY, calf_len, (10.0, 8.0), 0.0);
    let calf = tr(2, LS, class::PREY, calf_len, (20.0, 8.0), 0.0);
    let away = |id, at: (f64, f64)| tr(id, LS, class::PREY, adult, at, bearing(hound, at));
    let toward = |id, at: (f64, f64)| tr(id, LS, class::PREY, adult, at, bearing(at, hound));
    let g = tac(&f).guard_m;
    let (g1, g2) = ((20.0 + 0.4 * g, 8.0), (20.0, 8.0 + 0.4 * g));
    let alone = value_of(&f, &lone, &[lone, calf, away(3, g1), away(4, g2)]);
    let one = value_of(&f, &calf, &[lone, calf, away(3, g1)]);
    let two = value_of(&f, &calf, &[lone, calf, away(3, g1), away(4, g2)]);
    let facing = value_of(&f, &calf, &[lone, calf, toward(3, g1), toward(4, g2)]);
    assert!(alone.young && two.young && alone.guards == 0 && two.guards == 2);
    assert!(alone.p_kill > one.p_kill && one.p_kill > two.p_kill, "numbers: {} > {} > {}", alone.p_kill, one.p_kill, two.p_kill);
    assert!(two.p_kill > facing.p_kill, "adults facing the hound: {} > {}", two.p_kill, facing.p_kill);
    assert!(alone.value > two.value && two.value > facing.value, "values {} > {} > {}", alone.value, two.value, facing.value);
    assert!(alone.risk <= two.risk && two.risk <= facing.risk, "risk rises with kickers about it, more facing the hound");
}

/// **A lame adult outscores a sound one** (§7); a bigger body and a body standing facing the
/// hound score lower, one running away from it higher.
#[test]
fn a_lame_adult_outscores_a_sound_one() {
    let f = Fauna::new(config());
    let adult = f.config.founder(LS).adult_length_m;
    let hound = (15.0, 2.0);
    let (a, b) = ((10.0, 8.0), (20.0, 8.0));
    let sound = tr(1, LS, class::PREY, adult, a, bearing(hound, a));
    let lame = Track { lame: true, ..tr(2, LS, class::PREY, adult, b, bearing(hound, b)) };
    let s = value_of(&f, &sound, &[sound, lame]);
    let l = value_of(&f, &lame, &[sound, lame]);
    assert!(l.lame && !s.lame && l.p_kill > s.p_kill && l.value > s.value, "lame {} > sound {}", l.value, s.value);
    // Running away from the hound: easier. Facing it: harder.
    let running = value_of(&f, &Track { moving: true, ..sound }, &[sound, lame]);
    let facing = value_of(&f, &Track { heading: bearing(a, hound), ..sound }, &[sound, lame]);
    assert!(running.fleeing && running.p_kill > s.p_kill);
    assert!(facing.facing == 1 && facing.p_kill < s.p_kill);
    // Still, facing the hound: at bay, not resting.
    let bay = value_of(&f, &Track { heading: bearing(a, hound), posture: posture::REST, ..sound }, &[sound, lame]);
    assert!(bay.at_bay && bay.p_kill == facing.p_kill, "a still adult facing the pack is not taken as resting");
    let asleep = value_of(&f, &Track { posture: posture::ASLEEP, ..sound }, &[sound, lame]);
    assert!(asleep.p_kill > s.p_kill);
    // A bigger body is harder to pull down.
    let big = value_of(&f, &Track { size_m: 1.5 * adult, ..sound }, &[sound, lame]);
    assert!(big.p_one < s.p_one);
}

// ------------------------------------------------------------------ giving up (§4.3)

/// **A hound gives up when its wound passes the configured threshold** (§7), and on each of
/// the other reasons by its own knob: winded, stalled, the value under `give_up_value`.
#[test]
fn the_give_up_rule_reads_its_own_wound_stamina_stall_and_value() {
    let f = Fauna::new(config());
    let t = tac(&f);
    let ok = Me { wound: 0.0, spent: 0.0, ..hound_me(&f, 0.0, 0.0) };
    let v = t.give_up_value + 1.0;
    assert_eq!(give_up(&t, &ok, v, 0.0), None);
    assert_eq!(give_up(&t, &Me { wound: t.give_up_wound - 1e-9, ..ok }, v, 0.0), None, "just under the wound threshold");
    assert_eq!(give_up(&t, &Me { wound: t.give_up_wound, ..ok }, v, 0.0), Some(GiveUp::Wound));
    assert_eq!(give_up(&t, &Me { spent: t.give_up_spent, ..ok }, v, 0.0), Some(GiveUp::Winded));
    assert_eq!(give_up(&t, &ok, v, t.give_up_stall_s), Some(GiveUp::Stalled));
    assert_eq!(give_up(&t, &ok, t.give_up_value - 1e-9, 0.0), Some(GiveUp::Value));
    // A wound raises the risk it reads: the same target is worth less to a wounded hound.
    let pk = f.config.founder(CH).pack.unwrap();
    let adult = f.config.founder(LS).adult_length_m;
    let target = tr(1, LS, class::PREY, adult, (10.0, 8.0), 0.0);
    let sound = assess(&f.config, &t, &pk, &ok, &Hound::default(), &target, &[target], &[], 0, RING);
    let hurt = assess(&f.config, &t, &pk, &Me { wound: 0.5 * t.give_up_wound, ..ok }, &Hound::default(), &target, &[target], &[], 0, RING);
    assert!(hurt.risk > sound.risk && hurt.value < sound.value);
}

// ------------------------------------------------------------------ the hunt cue (§2.1, §11.2)

/// A hearer's record of `tracks`, without a body (the cue rules read only the record).
fn hearer(tracks: &[Track]) -> Fauna {
    let mut f = Fauna::new(config());
    f.world_geometry = Some((RING, VOXEL_M));
    set_percept(&mut f, 900, tracks);
    f
}

/// **A hunt cue gives the hearer the caller's bearing only; it adopts the quarry only with
/// its own track on it and the caller seen heading at it** (§11.2).
#[test]
fn a_hunt_cue_is_adopted_only_with_the_hearers_own_track_and_the_caller_seen_heading_at_it() {
    let base = Fauna::new(config());
    let t = tac(&base);
    let lost = base.config.founder(CH).intent.subject_lost_s;
    let hl = base.config.founder(CH).adult_length_m;
    let calf_len = 0.5 * t.young_length_fraction * base.config.founder(LS).adult_length_m;
    let caller_at = (12.0, 8.0);
    let (q, other) = ((18.0, 8.0), (12.0, 14.0));
    let cue = crate::calls::CallRecord { kind: HUNT_CUE as u8, x: caller_at.0, z: caller_at.1, tick: 0, ..Default::default() };
    let caller = |heading: f64, modes: u8| Track { modes, ..tr(50, CH, class::KIN, hl, caller_at, heading) };
    let quarry = tr(60, LS, class::PREY, calf_len, q, 0.0);
    let bystander = tr(61, LS, class::PREY, calf_len, other, 0.0);
    let at_q = bearing(caller_at, q);
    // Its own track on the quarry, the caller seen heading at it: adopted.
    let f = hearer(&[caller(at_q, SIGHT), quarry, bystander]);
    assert_eq!(adopt_from_cue(&f, 900, &cue, &t, lost, RING), Some(60));
    // No track of its own on the quarry: nothing (the cue carries no subject; the bystander is
    // not where the caller heads).
    let f = hearer(&[caller(at_q, SIGHT), bystander]);
    assert_eq!(adopt_from_cue(&f, 900, &cue, &t, lost, RING), None);
    // The caller only heard, not seen: nothing.
    let f = hearer(&[caller(at_q, crate::perception::HEARING), quarry, bystander]);
    assert_eq!(adopt_from_cue(&f, 900, &cue, &t, lost, RING), None);
    // The caller seen heading elsewhere: the prey it heads at, if the hearer tracks it.
    let f = hearer(&[caller(bearing(caller_at, other), SIGHT), quarry, bystander]);
    assert_eq!(adopt_from_cue(&f, 900, &cue, &t, lost, RING), Some(61));
    // No packmate in sight where the cue came from: nothing.
    let far = crate::calls::CallRecord { x: caller_at.0 - 3.0 * t.cue_caller_m - 1.0, ..cue };
    let f = hearer(&[caller(at_q, SIGHT), quarry, bystander]);
    assert_eq!(adopt_from_cue(&f, 900, &far, &t, lost, RING), None);
}
