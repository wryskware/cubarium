//! **VH-TACTICS law tests** (`design/behaviour-ai-2026-10-11.md` §7, §11.2, §11.4, §11.5; the
//! brief `design/handoffs/vh-tactics-2026-10-12.md`, "Checks").
//!
//! Mechanisms on hand-built fixtures. Every number a case relies on is read from the config (or
//! set by the case); no tuned default, ratio, verdict or hash is asserted. Nothing steps past a
//! few hundred ticks.


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

/// Hound `me` sees exactly `bodies` now (`(id, class)`), as they stand.
fn sees(f: &mut Fauna, me: u64, bodies: &[(u64, u8)]) {
    let tracks: Vec<Track> = bodies.iter().map(|&(id, c)| seen(f, me, id, c)).collect();
    set_percept(f, me, &tracks);
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
    Me { x, z, len: f.config.founder(CH).adult_length_m, wound: 0.0, hunger: 0.5, spent: 0.0, body: f.config.founder(CH).core.body_max, usable: f64::INFINITY }
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

// ------------------------------------------------------------------ scenes: the tactic itself

/// A pack of two on the arena, hound `h` at `(10, MID_Z)` facing east, its packmate far east;
/// a lone calf at `(16, MID_Z)`. Returns the world, plants, fauna, `[h, mate]` and the calf.
fn lone_calf_scene() -> (World, Flora, Fauna, [u64; 2], u64) {
    let w = arena(7);
    let flora = Flora::new(FloraConfig::default());
    let mut f = Fauna::new(config());
    let hs = pack(&mut f, &w, 2);
    place(&mut f, hs[0], 10.0, MID_Z, EAST);
    place(&mut f, hs[1], 28.0, MID_Z, WEST);
    let calf = strider(&mut f, &w, true, 16.0, MID_Z, EAST);
    (w, flora, f, [hs[0], hs[1]], calf)
}

/// Everything the tactic decided for hound `h` this sample: its action, memory, teacher label
/// and the calls it made.
fn decision(f: &Fauna, act: Actions, h: u64) -> String {
    format!("{act:?} {:?} {:?} {:?}", f.hounds.get(&h), f.hound_teach.get(&h), f.calls.iter().filter(|c| c.caller == h).collect::<Vec<_>>())
}

/// **Focal invariance** (§11.2, replacing "the pack never targets prey no member senses"):
/// with the hound's record and call history fixed, moving the bodies it does not sense — a
/// grown loftstrider and a calf beside it, its packmate — and pointing the pack's record at
/// one of them changes nothing it decides, sample after sample.
#[test]
fn moving_bodies_a_hound_does_not_sense_does_not_change_its_decision() {
    let (w, flora, mut f, [h, mate], calf) = lone_calf_scene();
    let near_adult = strider(&mut f, &w, false, 11.0, MID_Z + 2.5, WEST);
    let near_calf = strider(&mut f, &w, true, 9.0, MID_Z, EAST);
    let tracks = [seen(&f, h, calf, class::PREY)];
    let mut b = f.clone();
    place(&mut b, near_adult, 20.0, MID_Z - 4.0, EAST);
    place(&mut b, near_calf, 12.5, MID_Z + 1.0, WEST);
    place(&mut b, mate, 11.0, MID_Z - 1.0, EAST);
    let own = animal(&b, h).pack.unwrap().pack;
    if let Some(p) = b.pack_mut(own) {
        p.target = Some(near_calf);
        p.feast = Some(Site { x: 44, y: SURFACE_Y, z: D / 2 + 4 });
    }
    let period = cubarium_voxel::ticks_at_least_one(f.config.manifest(CH).controller_period_s);
    let mut decided = Vec::new();
    for sample in 0..8 {
        for x in [&mut f, &mut b] {
            set_percept(x, h, &tracks.map(|t| Track { tick: x.tick, ..t }));
            x.percepts.entry(mate).or_default();
        }
        let (aa, ab) = (decide(&mut f, &w, &flora, h), decide(&mut b, &w, &flora, h));
        assert_eq!(decision(&f, aa, h), decision(&b, ab, h), "sample {sample}: the same decision");
        decided.push(f.hounds.get(&h).and_then(|m| m.target));
        f.tick += period;
        b.tick += period;
    }
    assert!(decided.iter().all(|t| *t == Some(calf)), "it hunts the calf it senses: {decided:?}");
}

/// **A hound gives up when its wound passes the configured threshold** (§7), in the tactic:
/// committed to a calf, its wound set at `give_up_wound`, its next sample breaks off: the hunt
/// is dropped and the calf shunned, the give-up is counted, it howls the regroup howl and
/// turns from the calf; one hair under the threshold it hunts on.
#[test]
fn a_committed_hound_gives_up_when_its_wound_reaches_the_threshold() {
    for (under, label) in [(true, "under"), (false, "at")] {
        let (w, flora, mut f, [h, _], calf) = lone_calf_scene();
        let t = tac(&f);
        sees(&mut f, h, &[(calf, class::PREY)]);
        decide(&mut f, &w, &flora, h);
        let m = f.hounds[&h];
        assert!(m.target == Some(calf) && m.committed, "{label}: committed to the lone calf: {m:?}");
        animal_mut(&mut f, h).wound = if under { t.give_up_wound - 1e-6 } else { t.give_up_wound };
        f.tick += 1;
        f.last_call.clear();
        sees(&mut f, h, &[(calf, class::PREY)]);
        let act = decide(&mut f, &w, &flora, h);
        let m = f.hounds[&h];
        if under {
            assert_eq!(m.target, Some(calf), "under the threshold it hunts on");
            assert_eq!(f.ledger.hound_give_ups, 0);
            continue;
        }
        assert_eq!(m.target, None, "the hunt is dropped");
        assert!(m.gave_up_tick == f.tick && m.shuns(calf, f.tick), "the calf shunned: {m:?}");
        assert_eq!(f.ledger.hound_give_ups, 1);
        assert!(f.calls.iter().any(|c| c.caller == h && usize::from(c.kind) == REGROUP_HOWL), "the regroup howl");
        assert_eq!(f.hound_teach[&h].held, "give_up");
        // It holds no bite as it goes.
        assert_eq!(act.feed, 0.0, "{act:?}");
        // The shunned calf is not taken again in the shun.
        f.tick += 1;
        sees(&mut f, h, &[(calf, class::PREY)]);
        decide(&mut f, &w, &flora, h);
        assert_eq!(f.hounds[&h].target, None, "not taken again in the shun");
    }
}

/// A family on the arena: two grown loftstriders and a calf between them, standing; hound `h`
/// of a pack of two at `(8, MID_Z)` sees all three.
fn family_scene() -> (World, Flora, Fauna, u64, [u64; 3]) {
    let w = arena(9);
    let flora = Flora::new(FloraConfig::default());
    let mut f = Fauna::new(config());
    let hs = pack(&mut f, &w, 2);
    place(&mut f, hs[0], 8.0, MID_Z, EAST);
    place(&mut f, hs[1], 28.0, MID_Z, WEST);
    let a1 = strider(&mut f, &w, false, 16.0, MID_Z - 1.5, EAST);
    let calf = strider(&mut f, &w, true, 16.5, MID_Z, EAST);
    let a2 = strider(&mut f, &w, false, 16.0, MID_Z + 1.5, EAST);
    (w, flora, f, hs[0], [a1, calf, a2])
}

/// **The test** (§4.2, §4.3: the choice is made there): a group is tested, not chased; at the
/// rush's end the hound commits to the group's best when it is worth `commit_value`, and else
/// breaks off — the whole group shunned, the give-up counted, the regroup howl.
#[test]
fn a_test_ends_in_a_commitment_or_a_break_off_by_the_commit_value() {
    for (commit_value, commits) in [(f64::MIN, true), (f64::MAX, false)] {
        let (w, flora, mut f, h, fam) = family_scene();
        if let Some(p) = f.config.founders[CH.index()].pack.as_mut() {
            p.tactics.commit_value = commit_value;
            p.tactics.test_floor = f64::MIN;
        }
        let t = tac(&f);
        let all: Vec<(u64, u8)> = fam.iter().map(|&id| (id, class::PREY)).collect();
        sees(&mut f, h, &all);
        decide(&mut f, &w, &flora, h);
        let m = f.hounds[&h];
        assert_eq!(f.ledger.hound_tests, 1, "a group is tested");
        assert!(!m.committed && m.test == TestPhase::Approach && fam.contains(&m.target.unwrap()));
        assert_eq!(f.hound_teach[&h].held, "test");
        assert_eq!(f.ledger.pack_chases, 0, "a test is not a chase");
        // At the rush's end.
        let rush = cubarium_voxel::ticks_at_least_one(t.test_rush_s);
        f.tick += rush + 1;
        f.hounds.get_mut(&h).unwrap().test = TestPhase::Rush;
        f.hounds.get_mut(&h).unwrap().phase_tick = f.tick - rush - 1;
        sees(&mut f, h, &all);
        decide(&mut f, &w, &flora, h);
        let m = f.hounds[&h];
        if commits {
            assert!(m.committed && fam.contains(&m.target.unwrap()), "committed: {m:?}");
            assert_eq!(f.ledger.hound_test_commits, 1);
            assert_eq!(f.ledger.pack_chases, 1, "a chase starts");
        } else {
            assert_eq!(m.target, None, "broken off");
            assert!(fam.iter().all(|&id| m.shuns(id, f.tick)), "the group shunned");
            assert_eq!(f.ledger.hound_give_ups, 1);
            assert!(f.calls.iter().any(|c| c.caller == h && usize::from(c.kind) == REGROUP_HOWL));
            // Not tested again within the shun.
            f.tick += 1;
            sees(&mut f, h, &all);
            decide(&mut f, &w, &flora, h);
            assert_eq!((f.hounds[&h].target, f.ledger.hound_tests), (None, 1));
        }
    }
}

/// The hound `h` (facing east at `x0`) and points along its lunge: the jaw where the lunge
/// begins and the reach it sweeps.
fn jaw_line(f: &Fauna, h: u64) -> (f64, f64) {
    let hunter = f.config.founder(CH).active_hunter().unwrap();
    let len = length(f, h);
    (animal(f, h).pose.x + hunter.jaw_forward_length_fraction * len, hunter.strike_reach_length_fraction * len)
}

/// **Strikes go to the target** (§4.2, §11.4): of two bodies in the capsule the hound strikes
/// its target, not the nearer; a grown bystander alone in the capsule is not struck; a young
/// body is struck in its place only as an explicit substitution (counted).
#[test]
fn a_strike_takes_the_target_not_the_nearer_bystander() {
    let (w, _, mut f, [h, mate], calf) = lone_calf_scene();
    let hunter = f.config.founder(CH).active_hunter().unwrap();
    let (jaw, reach) = jaw_line(&f, h);
    let hx = animal(&f, h).pose.x;
    place(&mut f, mate, hx - 1.0, MID_Z, EAST);
    // The calf at the capsule's far end, a grown loftstrider nearer the jaw beside the line.
    place(&mut f, calf, jaw + 0.9 * reach, MID_Z, EAST);
    let adult = strider(&mut f, &w, false, jaw + 0.2 * reach, MID_Z + 0.4, EAST);
    let i = index(&f, h);
    let geometry = f.config.founder(CH).body_at(animal(&f, h).body);
    let view = w.view();
    let mut m = Hound { target: Some(calf), committed: true, ..Hound::default() };
    f.hounds.insert(h, m);
    sees(&mut f, h, &[(calf, class::PREY), (adult, class::PREY)]);
    assert_eq!(crate::step::prey_in_capsule(&f, i, &view, &geometry, &hunter), Some(calf), "the target, not the nearer adult");
    // Committed to nothing: the grown bystander is not struck.
    m.target = None;
    f.hounds.insert(h, m);
    sees(&mut f, h, &[(adult, class::PREY)]);
    place(&mut f, calf, 30.0, MID_Z + 5.0, EAST);
    assert_eq!(crate::step::prey_in_capsule(&f, i, &view, &geometry, &hunter), None, "no bite on a bystander adult");
    // A young body it senses with a high enough one-hound p_kill may be substituted.
    place(&mut f, calf, jaw + 0.9 * reach, MID_Z, EAST);
    sees(&mut f, h, &[(calf, class::PREY), (adult, class::PREY)]);
    let p = assess_of(&f, i, calf).unwrap().assess.p_one;
    let sub = crate::step::prey_in_capsule(&f, i, &view, &geometry, &hunter);
    assert_eq!(sub, (p >= tac(&f).substitute_p_kill).then_some(calf), "a substitution only past substitute_p_kill");
}

/// **Bodies block** (§11.4): a lunge at a calf with a grown loftstrider standing between hound
/// and calf meets the adult first, which takes it or blocks it — the calf is unhurt; with
/// nobody between, the same lunge wounds the calf.
#[test]
fn an_adult_between_hound_and_calf_takes_or_blocks_the_lunge() {
    for between in [false, true] {
        let (w, mut flora, mut f, [h, mate], calf) = lone_calf_scene();
        let (jaw, reach) = jaw_line(&f, h);
        let hx = animal(&f, h).pose.x;
    place(&mut f, mate, hx - 1.0, MID_Z, EAST);
        place(&mut f, calf, jaw + reach + 0.1, MID_Z, EAST);
        strider(&mut f, &w, false, if between { jaw + 0.5 * reach } else { 30.0 }, if between { MID_Z } else { 2.0 }, EAST);
        f.hounds.insert(h, Hound { target: Some(calf), committed: true, ..Hound::default() });
        let (x, z) = (animal(&f, h).pose.x, animal(&f, h).pose.z);
        animal_mut(&mut f, h).strike = Some(crate::Strike { phase: StrikePhase::Windup, ticks_left: 1, prey: calf, from_x: x, from_z: z, lure: false });
        let lunge = crate::Hunter::ticks(f.config.founder(CH).active_hunter().unwrap().lunge_s);
        let mut through_windup = true;
        for _ in 0..lunge + 2 {
            f.step_with(&w, &mut flora, 1);
            if let Some(s) = animal(&f, h).strike {
                through_windup &= s.prey == calf;
            }
            if animal(&f, h).strike.is_some_and(|s| s.phase == StrikePhase::Recovery) {
                break;
            }
        }
        assert!(through_windup, "the strike keeps its target");
        let calf_wound = f.view().animal(calf).map_or(1.0, |a| a.wound);
        if between {
            assert_eq!(calf_wound, 0.0, "the adult between takes or blocks the lunge");
            assert_eq!(f.ledger.lunges_blocked, 1);
        } else {
            assert!(calf_wound > 0.0, "nobody between: the calf is struck");
            assert_eq!(f.ledger.lunges_blocked, 0);
        }
    }
}

// ------------------------------------------------------------------ confined and occluded scenes

/// The committed hound `h`'s options on `target` in world `w` (its record as `sees` set it).
fn options_of(f: &mut Fauna, w: &World, h: u64, target: u64) -> Options {
    let i = index(f, h);
    let phys = *f.config.founder(CH);
    let (pk, hunter) = (phys.pack.unwrap(), phys.active_hunter().unwrap());
    let geometry = phys.body_at(animal(f, h).body);
    let mem = f.hounds.get(&h).copied().unwrap_or_default();
    let p = assess_of(f, i, target).expect("a target it senses");
    options(f, i, &w.view(), &pk.tactics, &pk, &hunter, &geometry, &mem, &p)
}

fn inside(p: (f64, f64), (x0, x1): (i64, i64), (z0, z1): (u32, u32)) -> bool {
    let (x, z) = ((p.0 / VOXEL_M).floor() as i64, (p.1 / VOXEL_M).floor() as i64);
    (x0..=x1).contains(&x) && (i64::from(z0)..=i64::from(z1)).contains(&z)
}

/// **A cut-off point is reachable** (§11.5), in an asymmetric, confined scene: a calf with its
/// grown kin off to one side. On open ground the hound's cut-off point lies between them; with
/// a closed pen of rock over the stretch between them, the point the hound takes is never in
/// the pen (passed over for a reachable one, or none).
#[test]
fn a_cut_off_point_is_reachable_and_not_in_a_closed_pen() {
    let pen = ((66i64, 90i64), (D / 2 - 6, D / 2 + 10));
    for penned in [false, true] {
        let mut w = arena(13);
        if penned {
            let ((x0, x1), (z0, z1)) = pen;
            wall(&mut w, (x0, x1), (z0, z0), 5);
            wall(&mut w, (x0, x1), (z1, z1), 5);
            wall(&mut w, (x0, x0), (z0, z1), 5);
            wall(&mut w, (x1, x1), (z0, z1), 5);
        }
        let mut f = Fauna::new(config());
        let hs = pack(&mut f, &w, 2);
        let h = hs[0];
        place(&mut f, h, 8.0, MID_Z - 4.0, EAST);
        place(&mut f, hs[1], 28.0, 2.0, WEST);
        let calf = strider(&mut f, &w, true, 16.0, MID_Z, EAST);
        let adult = strider(&mut f, &w, false, 23.5, MID_Z + 1.5, WEST);
        f.hounds.insert(h, Hound { target: Some(calf), committed: true, ..Hound::default() });
        sees(&mut f, h, &[(calf, class::PREY), (adult, class::PREY)]);
        let o = options_of(&mut f, &w, h, calf);
        let calf_at = (16.0, MID_Z);
        let adult_at = (23.5, MID_Z + 1.5);
        if !penned {
            let c = o.cut.expect("a cut-off point on open ground");
            assert!((dist(calf_at, c) + dist(c, adult_at) - dist(calf_at, adult_at)).abs() < 1e-6, "between them");
            assert!(o.scores[ix("cut_off")].is_finite());
        } else if let Some(c) = o.cut {
            assert!(!inside(c, pen.0, pen.1), "never in the pen: {c:?}");
        } else {
            assert!(o.scores[ix("cut_off")] == f64::NEG_INFINITY, "no point, no option");
        }
    }
}

/// **Interceptions in terrain** (§11.5, feasible and failed apart): a calf running across the
/// hound's front is intercepted on open ground; with a rock wall across its line the point
/// the hound aims at is never beyond the wall (the lead is replanned short of it, or none).
#[test]
fn an_interception_is_bounded_by_terrain() {
    let wall_z = D / 2 + 4;
    for walled in [false, true] {
        let mut w = arena(17);
        if walled {
            wall(&mut w, (0, i64::from(W) - 1), (wall_z, wall_z + 1), 5);
        }
        let mut f = Fauna::new(config());
        let hs = pack(&mut f, &w, 2);
        let h = hs[0];
        place(&mut f, h, 10.0, MID_Z, EAST);
        place(&mut f, hs[1], 28.0, 2.0, WEST);
        let calf = strider(&mut f, &w, true, 14.0, MID_Z - 0.5, 0.0);
        // Its last two sightings: running +z (north) at a walk's pace.
        let v = f.config.manifest(LS).cruise_m_per_s;
        f.hounds.insert(h, Hound { target: Some(calf), committed: true, vel: [0.0, v], last_tick: f.tick, ..Hound::default() });
        let mut t = seen(&f, h, calf, class::PREY);
        t.moving = true;
        set_percept(&mut f, h, &[t]);
        let o = options_of(&mut f, &w, h, calf);
        if !walled {
            let p = o.intercept.expect("a feasible interception on open ground");
            assert!(p.1 > MID_Z - 0.5, "ahead of the calf");
            assert!(o.scores[ix("intercept")].is_finite());
        } else if let Some(p) = o.intercept {
            assert!(p.1 < f64::from(wall_z) * VOXEL_M, "never beyond the wall: {p:?}");
        }
    }
}

/// **Occlusion** (§11.5): a hound that hunts by sight alone takes a lone calf in the open; with
/// a rock wall between them it does not (it reads only what it senses).
#[test]
fn a_calf_behind_rock_is_not_taken_by_a_hound_that_cannot_sense_it() {
    for walled in [false, true] {
        let mut w = arena(19);
        if walled {
            wall(&mut w, (52, 54), (0, D - 1), 8);
        }
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config();
        c.founders[CH.index()].senses = crate::SenseTable {
            live: true,
            sight_m: 12.0,
            sight_night_m: 12.0,
            sight_still_fraction: 1.0,
            sight_half_angle_deg: 180.0,
            hearing_m: 0.0,
            hearing_min: 1e-12,
            ..crate::SenseTable::default()
        };
        c.perception.remote_hz = f64::from(crate::TICK_HZ);
        let mut f = Fauna::new(c);
        let hs = pack(&mut f, &w, 2);
        place(&mut f, hs[0], 10.0, MID_Z, EAST);
        place(&mut f, hs[1], 9.0, MID_Z + 1.0, EAST);
        let calf = strider(&mut f, &w, true, 17.0, MID_Z, EAST);
        let period = cubarium_voxel::ticks_at_least_one(f.config.manifest(CH).controller_period_s) as u32;
        let mut took = false;
        for _ in 0..3 * period {
            f.step_with(&w, &mut flora, 1);
            took |= hs.iter().any(|h| f.hounds.get(h).is_some_and(|m| m.target == Some(calf)));
        }
        let sensed = hs.iter().any(|&h| f.percept(h).is_some_and(|p| p.track(calf).is_some()));
        assert_eq!(took, !walled, "walled {walled}: taken only when sensed (sensed now: {sensed})");
    }
}

/// **The stalk's rush reads the track** (§2.1): with tactics on, a prey is running only when
/// the hound's track shows it moving away, whatever its own flight state; with tactics off the
/// old tactic keeps its own read (`None`).
#[test]
fn a_prey_is_seen_running_only_from_the_hounds_track() {
    let (_, _, mut f, [h, _], calf) = lone_calf_scene();
    let mut t = seen(&f, h, calf, class::PREY);
    f.percepts.entry(calf).or_default().vigil.fleeing = true;
    let a = animal(&f, h);
    assert_eq!(seen_running(&f, &a, &t), Some(false), "a still track is not running, whatever its flight state");
    t.moving = true;
    t.heading = EAST;
    assert_eq!(seen_running(&f, &a, &t), Some(true), "moving away from the hound");
    t.heading = WEST;
    assert_eq!(seen_running(&f, &a, &t), Some(false), "moving at the hound");
    old_tactic(&mut f.config);
    assert_eq!(seen_running(&f, &a, &t), None);
}

// ------------------------------------------------------------------ VH-FIX: the lone calf is catchable

/// **A lunge is timed to land** (VH-FIX step 1): a target in the capsule that stands is struck
/// at once; one fleeing straight down the lunge's line faster than the lunge covers its reach
/// (windup and lunge) is not — the lunge would end short — and one fleeing slower from near
/// the jaw still is; a target outside the capsule is not the timing's business. Speeds are
/// set from the config's strike numbers.
#[test]
fn a_lunge_is_timed_to_land_on_the_target_as_it_moves() {
    let (_, _, f, [h, _], _) = lone_calf_scene();
    let hunter = f.config.founder(CH).active_hunter().unwrap();
    let geometry = f.config.founder(CH).body_at(animal(&f, h).body);
    let (jaw, reach) = jaw_line(&f, h);
    let lunge_pace = reach / (hunter.windup_s + hunter.lunge_s);
    let calf_len = 0.5 * tac(&f).young_length_fraction * f.config.founder(LS).adult_length_m;
    let at = |x: f64| Track { tick: f.tick, ..tr(9, LS, class::PREY, calf_len, (x, MID_Z), EAST) };
    let lands = |t: &Track, v: f64| lunge_lands(&f.config, &animal(&f, h), &geometry, &hunter, t, (v, 0.0), f.tick, RING);
    assert_eq!(lands(&at(jaw + 0.9 * reach), 0.0), Some(true), "a standing target in the capsule");
    assert_eq!(lands(&at(jaw + 0.9 * reach), 2.0 * lunge_pace), Some(false), "fleeing faster than the lunge: held");
    assert_eq!(lands(&at(jaw + 0.1 * reach), 0.5 * lunge_pace), Some(true), "fleeing slower, near the jaw: struck");
    assert_eq!(lands(&at(jaw + 4.0 * reach + 4.0), 0.0), None, "out of the capsule");
    // In the tactic's hands: the strike (feed) is held exactly when the lunge would miss.
    let t = tac(&f);
    let run = Actions { forward: 1.0, turn: 0.0, feed: 1.0 };
    let mem = Hound { vel: [2.0 * lunge_pace, 0.0], ..Hound::default() };
    let moving = Track { moving: true, ..at(jaw + 0.9 * reach) };
    assert_eq!(timed(&f, &t, &animal(&f, h), &geometry, &hunter, &moving, &mem, RING, run).feed, 0.0, "held");
    let off = Tactics { time_strikes: false, ..t };
    assert_eq!(timed(&f, &off, &animal(&f, h), &geometry, &hunter, &moving, &mem, RING, run).feed, 1.0, "the switch off strikes at once");
}

/// **A lone calf in the open within a hound's reach is caught** (VH-FIX step 1, "Checks"): a
/// calf running straight away at its cruise, a hungry hound a few metres behind it on open
/// ground, every knob from the config except the commitment and value floors (set so the case
/// does not hang on the value placeholders); within a few hundred ticks the calf is struck.
#[test]
fn a_lone_calf_in_the_open_within_a_hounds_reach_is_caught() {
    let (w, mut flora, mut f, [h, mate], calf) = lone_calf_scene();
    if let Some(p) = f.config.founders[CH.index()].pack.as_mut() {
        p.tactics.commit_value = f64::MIN;
        p.tactics.test_floor = f64::MIN;
        p.tactics.give_up_value = f64::MIN;
    }
    // Its packmate is fed and rests at the den: one hound on the calf.
    animal_mut(&mut f, mate).eating = false;
    place(&mut f, mate, 1.0, MID_Z, EAST);
    let (jaw, reach) = jaw_line(&f, h);
    let hx = animal(&f, h).pose.x;
    // The calf just beyond the capsule, running east at its cruise.
    place(&mut f, calf, jaw + reach + 1.5, MID_Z, EAST);
    drive(&mut f, calf, 1.0, false);
    assert!(animal(&f, calf).pose.x - hx < f.config.founder(CH).pack.unwrap().close_in_m + reach);
    let mut struck = false;
    for _ in 0..400 {
        f.step_with(&w, &mut flora, 1);
        if f.view().animal(calf).is_none_or(|a| a.wound > 0.0) {
            struck = true;
            break;
        }
    }
    assert!(struck, "the calf is struck: hound {:?}, calf {:?}", f.hounds.get(&h), f.view().animal(calf).map(|a| (a.pose, a.wound)));
}

// ------------------------------------------------------------------ VH-FIX: usable meals

/// Hound `h`'s stomach holds `s` of its capacity in carrion and its reserve `r` of its cap.
fn fill(f: &mut Fauna, h: u64, s: f64, r: f64) {
    let a = animal(f, h);
    let cap = crate::body::effective_config(&f.config, &a).reserve_of(a.body);
    let k = f.config.founder(CH).gut.capacity(a.body);
    let m = animal_mut(f, h);
    m.stomach = crate::Stomach::default();
    m.stomach.pools[crate::Food::Carrion.index()].organic = s * k;
    m.reserve = r * cap;
}

/// **The meals a hound can use from one carcass** (VH-FIX step 2) come from its own body:
/// with no time to come back (`carcass_keep_s` 0) they are its stomach's room now; given time
/// they grow by what its gut takes, until its reserve's room bounds them; a fuller reserve
/// leaves fewer; a full stomach and a full reserve with no time leave none.
#[test]
fn the_usable_meals_are_the_hounds_gut_room_and_what_it_can_absorb_in_the_keep() {
    let (_, _, mut f, [h, _], _) = lone_calf_scene();
    let usable = |f: &Fauna, keep: f64| {
        let mut g = f.clone();
        g.config.founders[CH.index()].pack.as_mut().unwrap().tactics.carcass_keep_s = keep;
        Me::usable_meals(&g, &animal(&g, h))
    };
    fill(&mut f, h, 0.25, 0.2);
    let k = f.config.founder(CH).gut.stomach_k;
    assert!((usable(&f, 0.0) - 0.75 * k).abs() < 1e-9, "no keep: the stomach's room, {} of k {}", usable(&f, 0.0), k);
    let tau = f.config.founder(CH).gut.digestion_s[crate::Food::Carrion.index()];
    let (short, long, longer) = (usable(&f, 0.5 * tau), usable(&f, 50.0 * tau), usable(&f, 500.0 * tau));
    assert!(usable(&f, 0.0) < short && short < long, "the gut's take grows with the keep");
    assert!(longer < usable(&f, 0.0) + 500.0 * k, "the reserve's room bounds it, not the gut alone");
    let mut fuller = f.clone();
    fill(&mut fuller, h, 0.25, 0.9);
    assert!(usable(&fuller, 50.0 * tau) < long, "a fuller reserve uses less");
    fill(&mut f, h, 1.0, 1.0);
    assert_eq!(usable(&f, 0.0), 0.0, "full and no keep: nothing");
    let mut off = f.clone();
    off.config.founders[CH.index()].pack.as_mut().unwrap().tactics.usable_meals = false;
    assert_eq!(Me::usable_meals(&off, &animal(&off, h)), f64::INFINITY, "the switch off: no bound");
}

/// **With usable meat counted, a pack that cannot eat an adult values an exposed calf above
/// a guarded adult** (VH-FIX step 2, "Checks"): the hounds at hand can use no more than the
/// calf's meat; the adult's counted meat is cut to that, not its whole carcass, and the calf,
/// alone and young, with the adult standing with another and facing the hound, is worth
/// more. Config weights; only the hound's gut room is set by the case.
#[test]
fn a_pack_that_cannot_eat_an_adult_values_an_exposed_calf_above_a_guarded_adult() {
    let f = Fauna::new(config());
    let t = tac(&f);
    let pk = f.config.founder(CH).pack.unwrap();
    let adult_len = f.config.founder(LS).adult_length_m;
    let hound = (15.0, 2.0);
    let calf = tr(1, LS, class::PREY, 0.5 * t.young_length_fraction * adult_len, (10.0, 8.0), 0.0);
    let at = (20.0, 8.0);
    let adult = tr(2, LS, class::PREY, adult_len, at, bearing(at, hound));
    let other = tr(3, LS, class::PREY, adult_len, (at.0 + 0.4 * t.guard_m, at.1), bearing(at, hound));
    let prey = [calf, adult, other];
    let mut me = hound_me(&f, hound.0, hound.1);
    let whole = |x: &Track| assess(&f.config, &t, &pk, &me, &Hound::default(), x, &prey, &[], 0, RING).meat_raw;
    assert!(whole(&adult) > whole(&calf), "an adult carries more meat than a calf");
    me.usable = whole(&calf);
    let (c, a) = (
        assess(&f.config, &t, &pk, &me, &Hound::default(), &calf, &prey, &[], 0, RING),
        assess(&f.config, &t, &pk, &me, &Hound::default(), &adult, &prey, &[], 0, RING),
    );
    assert!(a.meat < a.meat_raw && (a.meat - me.usable).abs() < 1e-12, "the adult counts only what the pack can eat: {a:?}");
    assert!((c.meat - c.meat_raw).abs() < 1e-12, "the calf is eaten whole");
    assert!(c.value > a.value, "the exposed calf {} over the guarded adult {}", c.value, a.value);
    let off = Tactics { usable_meals: false, ..t };
    assert_eq!(assess(&f.config, &off, &pk, &me, &Hound::default(), &adult, &prey, &[], 0, RING).meat, a.meat_raw, "the switch off counts the whole carcass");
}

/// **A break-off when the value falls below zero during a test** (VH-FIX step 2, "Checks"):
/// a hound tests a family; by the rush's end it has nothing left to eat a kill with (its gut
/// and reserve full, no time to come back), so every member's value is under the configured
/// `commit_value` and it breaks off — the break-off counted, the group shunned, the howl —
/// with the configured commitment, not one set by the case.
#[test]
fn a_test_breaks_off_when_the_value_falls_below_zero() {
    let (w, flora, mut f, h, fam) = family_scene();
    let t = tac(&f);
    assert!(t.chase_cost_per_m > 0.0 && t.commit_value >= 0.0, "a chase costs and a commitment needs a positive value");
    let all: Vec<(u64, u8)> = fam.iter().map(|&id| (id, class::PREY)).collect();
    sees(&mut f, h, &all);
    decide(&mut f, &w, &flora, h);
    assert!(f.hounds[&h].test == TestPhase::Approach, "a group is tested");
    // Mid-test it has eaten its fill, with no time to come back for more.
    f.config.founders[CH.index()].pack.as_mut().unwrap().tactics.carcass_keep_s = 0.0;
    fill(&mut f, h, 1.0, 1.0);
    let rush = cubarium_voxel::ticks_at_least_one(t.test_rush_s);
    f.tick += rush + 1;
    f.hounds.get_mut(&h).unwrap().test = TestPhase::Rush;
    f.hounds.get_mut(&h).unwrap().phase_tick = f.tick - rush - 1;
    sees(&mut f, h, &all);
    let i = index(&f, h);
    assert!(fam.iter().all(|&id| assess_of(&f, i, id).unwrap().assess.value < 0.0), "every member below zero");
    decide(&mut f, &w, &flora, h);
    let m = f.hounds[&h];
    assert_eq!(m.target, None, "broken off");
    assert!(fam.iter().all(|&id| m.shuns(id, f.tick)), "the group shunned");
    assert_eq!(f.ledger.hound_give_ups, 1);
    assert!(f.take_pack_events().iter().any(|e| matches!(e, crate::PackEvent::ChaseEnd { why: crate::ChaseEnd::BrokeOff, .. })), "logged as a break-off");
}

// ------------------------------------------------------------------ facing (BALANCE step 1)

/// **A hound reads facing from the body's own heading at its last sighting** (BALANCE, the
/// `facing 0` trace): the engagement traces showed `facing 0` only while the adults had truly
/// turned their backs (`move_off`, or turning into `guard`), and for at most one sight interval
/// (`perception.remote_hz`) after a turn. This fixes the cause found: the heading `faces()`
/// judges is the body's own, as the senses wrote it at the hound's last sighting; a body that
/// turns to face the hound reads as facing from the hound's next sighting on, and a
/// neighbour's heading never stands in for it.
#[test]
fn a_body_that_turns_to_face_the_hound_reads_facing_from_its_next_sighting() {
    let w = arena(11);
    let mut f = Fauna::new(config());
    let h = pack(&mut f, &w, 1)[0];
    let hound = (10.0, MID_Z);
    place(&mut f, h, hound.0, hound.1, EAST);
    let at = (15.0, MID_Z);
    let beside = (15.0, MID_Z + 1.5 * f.config.founder(LS).adult_length_m);
    let away = bearing(hound, at);
    let a = strider(&mut f, &w, false, at.0, at.1, away);
    let b = strider(&mut f, &w, false, beside.0, beside.1, bearing(hound, beside));
    let stagger = f.config.perception.stagger_ticks();
    let sight = |f: &mut Fauna| {
        for _ in 0..stagger {
            f.tick += 1;
            crate::perception::sense_pass(f, &w.view(), false, 1.0, 1);
        }
    };
    let i = index(&f, h);
    sight(&mut f);
    let turned_away = assess_of(&f, i, a).expect("the hound sees it");
    assert_eq!(turned_away.assess.facing, 0, "both backs to the hound");
    // It turns to face the hound; its neighbour keeps its back turned.
    animal_mut(&mut f, a).pose.heading_rad = bearing(at, hound);
    sight(&mut f);
    let p = assess_of(&f, i, a).expect("the hound sees it");
    assert_eq!(p.track.heading, animal(&f, a).pose.heading_rad, "the track carries the body's own heading");
    assert_eq!(p.assess.facing, 1, "it faces the hound, its neighbour does not");
    assert!(p.assess.guards >= 1, "the neighbour stands by it");
    // The neighbour's assessment counts the same one facing body.
    assert_eq!(assess_of(&f, i, b).expect("seen").assess.facing, 1);
}
