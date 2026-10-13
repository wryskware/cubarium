//! Contract v3's observation (P1), against hand-built records and a live world. Mechanism
//! only: no tuned number is pinned; every expectation is read off the manifest or the
//! record the case builds.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use crate::observe_v3::{Frame, count, nearest_carcass, proximity, scent, slot, tremor};
use crate::perception::{Percept, Track, class, posture};
use crate::scent::ScentReading;
use crate::{Fauna, Founder, StartingStores};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use fixture::*;

const RING: f64 = 12.0;
const RANGE: f64 = 8.0;
const FRESH: f64 = 10.0;

/// A body at (4, 1.5) facing +z (heading 0) at tick 1000.
fn frame() -> Frame {
    Frame { x: 4.0, z: 1.5, heading: 0.0, ring_m: RING, tick: 1000 }
}

fn track(x: f64, z: f64, tick: u64) -> Track {
    Track { known: true, class: class::THREAT, x, z, tick, ..Track::default() }
}

fn ticks(s: f64) -> u64 {
    (s / cubarium_voxel::DT).round() as u64
}

/// The bearing is the body frame's, positive to the right: an item to the body's right
/// reads `sin > 0`, to its left `sin < 0`, ahead `cos > 0`, behind `cos < 0`; the proximity
/// is `1 − d / range` of the horizontal distance now.
#[test]
fn a_slot_reads_the_bearing_in_the_body_frame_and_the_proximity() {
    let f = frame();
    // Heading 0 faces +z; its right is +x (forward (sin h, cos h), right (cos h, −sin h)).
    let right = slot(&track(f.x + 2.0, f.z, f.tick), &f, RANGE, FRESH);
    let left = slot(&track(f.x - 2.0, f.z, f.tick), &f, RANGE, FRESH);
    let ahead = slot(&track(f.x, f.z + 3.0, f.tick), &f, RANGE, FRESH);
    let behind = slot(&track(f.x, f.z - 3.0, f.tick), &f, RANGE, FRESH);
    assert_eq!(right[0], 1.0, "valid");
    assert!(right[1] > 0.99 && right[2].abs() < 1e-9, "right: {right:?}");
    assert!(left[1] < -0.99, "left: {left:?}");
    assert!(ahead[2] > 0.99 && ahead[1].abs() < 1e-9, "ahead: {ahead:?}");
    assert!(behind[2] < -0.99, "behind: {behind:?}");
    assert!((right[3] - proximity(2.0, RANGE)).abs() < 1e-12);
    assert!((ahead[3] - (1.0 - 3.0 / RANGE)).abs() < 1e-12);
    // Turning the body turns the bearing: facing +x (heading π/2), the item at +z is on its
    // left.
    let turned = Frame { heading: std::f64::consts::FRAC_PI_2, ..f };
    let s = slot(&track(f.x, f.z + 3.0, f.tick), &turned, RANGE, FRESH);
    assert!(s[1] < -0.99, "turned: {s:?}");
    // Beyond the range it is still valid, at proximity 0.
    let far = slot(&track(f.x, f.z + RANGE + 1.0, f.tick), &f, RANGE, FRESH);
    assert_eq!((far[0], far[3]), (1.0, 0.0));
}

/// An empty slot reads all zero, validity included; a stale one fades by `1 − age /
/// fresh_s` and is empty past `fresh_s`.
#[test]
fn an_empty_or_stale_slot_reads_zero_and_freshness_falls_with_age() {
    let f = frame();
    assert_eq!(slot(&Track::default(), &f, RANGE, FRESH), [0.0; 7], "no track");
    let now = slot(&track(f.x, f.z + 1.0, f.tick), &f, RANGE, FRESH);
    let half = slot(&track(f.x, f.z + 1.0, f.tick - ticks(FRESH / 2.0)), &f, RANGE, FRESH);
    let gone = slot(&track(f.x, f.z + 1.0, f.tick - ticks(FRESH + 1.0)), &f, RANGE, FRESH);
    assert_eq!(now[6], 1.0);
    assert!((half[6] - 0.5).abs() < 1e-9, "half fresh: {half:?}");
    assert_eq!(gone, [0.0; 7], "past the horizon");
    // The record's flags.
    let mut t = track(f.x, f.z + 1.0, f.tick);
    t.moving = true;
    t.posture = posture::ASLEEP;
    let s = slot(&t, &f, RANGE, FRESH);
    assert_eq!((s[4], s[5]), (1.0, 1.0));
    t.moving = false;
    t.posture = posture::ACTIVE;
    let s = slot(&t, &f, RANGE, FRESH);
    assert_eq!((s[4], s[5]), (0.0, 0.0));
}

/// No world coordinate reaches the vector: moving the body and everything it knows along
/// the ring, or turning them all together, leaves every v3 channel as it was.
#[test]
fn moving_or_turning_the_whole_scene_leaves_the_encoding_unchanged() {
    let f = frame();
    let t = track(f.x - 1.5, f.z + 2.0, f.tick - 3);
    let carcass = [Site { x: 20, y: 1, z: 9 }];
    let r = ScentReading { level: 0.3, gx: 0.2, gz: -0.1 };
    let base = (slot(&t, &f, RANGE, FRESH), nearest_carcass(&carcass, &f, 0.25, RANGE), scent(&r, &f));
    // Along the ring by whole voxels (and across the seam).
    for dx in [0.25 * 7.0, RING - 0.5] {
        let g = Frame { x: (f.x + dx).rem_euclid(RING), ..f };
        let tt = Track { x: (t.x + dx).rem_euclid(RING), ..t };
        let sx = (carcass[0].x + (dx / 0.25).round() as u32) % (RING / 0.25) as u32;
        let cc = [Site { x: sx, ..carcass[0] }];
        let moved = (slot(&tt, &g, RANGE, FRESH), nearest_carcass(&cc, &g, 0.25, RANGE), scent(&r, &g));
        for (a, b) in base.0.iter().zip(moved.0.iter()).chain(base.1.iter().zip(moved.1.iter())) {
            assert!((a - b).abs() < 1e-9, "{base:?} vs {moved:?}");
        }
        assert_eq!(base.2, moved.2);
    }
    // Turned together by a quarter: the body's heading, the item's offset and the gradient.
    let q = std::f64::consts::FRAC_PI_2;
    let rot = |dx: f64, dz: f64| (dx * q.cos() + dz * q.sin(), -dx * q.sin() + dz * q.cos());
    let g = Frame { heading: f.heading + q, ..f };
    let (ox, oz) = rot(t.x - f.x, t.z - f.z);
    let tt = Track { x: f.x + ox, z: f.z + oz, ..t };
    let (gx, gz) = rot(r.gx, r.gz);
    let turned = (slot(&tt, &g, RANGE, FRESH), scent(&ScentReading { gx, gz, ..r }, &g));
    for (a, b) in base.0.iter().zip(turned.0.iter()).chain(base.2.iter().zip(turned.1.iter())) {
        assert!((a - b).abs() < 1e-9, "turned: {base:?} vs {turned:?}");
    }
}

/// The counts, the scent's body-frame gradient, the tremor's sides and the nearest
/// carcass read the record they are given.
#[test]
fn the_counts_scent_tremor_and_carcass_read_the_record() {
    let f = frame();
    let mut p = Percept::default();
    let mut tracks = [Track::default(); crate::perception::MAX_TRACKS];
    for (k, t) in tracks.iter_mut().take(6).enumerate() {
        *t = Track { known: true, class: if k < 2 { class::THREAT } else { class::KIN }, ..Track::default() };
    }
    p.tracks = tracks;
    p.n = 6;
    assert_eq!(count(&p, class::THREAT), 0.5, "two of four");
    assert_eq!(count(&p, class::KIN), 1.0, "four or more is full");
    assert_eq!(count(&p, class::PREY), 0.0);
    // A gradient along +z is ahead of a body facing +z; along +x it is to its right.
    let ahead = scent(&ScentReading { level: 0.0, gx: 0.0, gz: 1.0 }, &f);
    let right = scent(&ScentReading { level: 1.0, gx: 1.0, gz: 0.0 }, &f);
    assert!(ahead[1] > 0.0 && ahead[2].abs() < 1e-12 && ahead[0] == 0.0, "{ahead:?}");
    assert!(right[2] > 0.0 && right[1].abs() < 1e-12 && right[0] > 0.0, "{right:?}");
    let t = tremor([0.0, 1.0]);
    assert!(t[0] == 0.0 && t[1] > 0.0, "{t:?}");
    // The nearer of two carcasses, an empty block beyond the range.
    let near = Site { x: 16, y: 1, z: 9 };
    let far = Site { x: 30, y: 1, z: 9 };
    let c = nearest_carcass(&[far, near], &f, 0.25, RANGE);
    let d = (((16.5 * 0.25) - f.x).powi(2) + ((9.5 * 0.25) - f.z).powi(2)).sqrt();
    assert_eq!(c[0], 1.0);
    assert!((c[3] - proximity(d, RANGE)).abs() < 1e-12, "{c:?}");
    assert_eq!(nearest_carcass(&[far], &f, 0.25, 0.5), [0.0; 4]);
}

/// Every lineage's whole v3 observation, encoded in a live world, is finite and inside its
/// channel's range — a bearing or gradient in `[−1, 1]`, everything else in `[0, 1]` — and
/// is as long as its manifest; its v2 slots are the v2 observation's.
#[test]
fn every_lineages_v3_observation_is_finite_and_bounded() {
    let world = clock_off(7);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(hunting_config());
    let stores = StartingStores { body: 1.0, reserve: 0.5 };
    let mut ids = Vec::new();
    for (k, f) in Founder::ALL.into_iter().enumerate() {
        let id = introduce(&mut fauna, &world, f, stores);
        place(&mut fauna, id, 1.0 + 1.2 * k as f64, 1.5, 0.3 * k as f64);
        ids.push((id, f));
    }
    fauna.start_tap();
    for _ in 0..40 {
        fauna.step(&world, &mut flora);
    }
    let tap = fauna.take_tap().expect("the tap ran");
    let signed = |c: &str| matches!(c, "resolved_turn" | "trend" | "forward" | "right") || c.ends_with("sin") || c.ends_with("cos");
    let mut checked = 0;
    for (id, f) in ids {
        let m = f.manifest();
        let stream = tap.stream(id).unwrap_or_else(|| panic!("{} recorded nothing", f.name()));
        assert!(!stream.is_empty(), "{}", f.name());
        checked += 1;
        for s in stream {
            assert_eq!(s.observation.len(), m.inputs(), "{}", f.name());
            for module in m.modules {
                for (k, c) in module.channels.iter().enumerate() {
                    let v = s.observation[module.offset + k];
                    let low = if signed(c) { -1.0 } else { 0.0 };
                    assert!(v.is_finite() && v >= low && v <= 1.0, "{} {}.{c} = {v}", f.name(), module.name);
                }
            }
            let self_v3 = m.modules.iter().find(|x| x.name == "SelfV3").expect("SelfV3");
            let phase = &s.observation[self_v3.offset + 5..self_v3.offset + 7];
            assert!((phase[0].powi(2) + phase[1].powi(2) - 1.0).abs() < 1e-9, "the day's phase is a unit vector");
        }
    }
    assert_eq!(checked, Founder::COUNT, "every lineage was sampled");
}

/// CLONE-AGREE: the Veilhound's `Rhythm.rest_bout` reads its own rest bout — 1 while
/// [`crate::Animal::rest_ticks`] is above 0, else 0 — and nothing else in its observation moves.
#[test]
fn a_veilhounds_rest_bout_reads_on_its_rhythm_channel() {
    let world = clock_off(7);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(hunting_config());
    let id = introduce(&mut fauna, &world, Founder::Chorister, StartingStores::FULL);
    place(&mut fauna, id, 4.0, 1.5, 0.0);
    let i = fauna.animals.iter().position(|a| a.id == id).expect("the hound");
    let m = Founder::Chorister.manifest();
    let rhythm = m.modules.iter().find(|x| x.name == "Rhythm").expect("the Veilhound reads its rhythm");
    let clock = crate::observe_v3::Clock { clock_on: true, daylight: 1.0, day_phase: 0.5 };
    let read = |fauna: &Fauna| {
        let mut obs = vec![0.0; m.inputs()];
        crate::observe_v3::encode(fauna, i, &world.view(), &flora.view(), &m, clock, &mut obs);
        obs
    };
    fauna.animals[i].rest_ticks = 0;
    let awake = read(&fauna);
    fauna.animals[i].rest_ticks = 5;
    let bout = read(&fauna);
    assert_eq!(awake[rhythm.offset], 0.0, "awake");
    assert_eq!(bout[rhythm.offset], 1.0, "in a rest bout");
    for k in (0..m.inputs()).filter(|&k| k != rhythm.offset) {
        assert_eq!(awake[k], bout[k], "channel {k} moved with the bout");
    }
}
