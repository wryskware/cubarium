//! Independent FEAR and FEAR-2 contracts:
//! Sight alone permits watching a sleeping hunter, or a still hunter facing away outside
//! contact. Movement, hearing/tremor, facing the prey and contact restore the threat.
//! The facing boundary is inclusive; the margin uses 3D distance and walks without feeding.
//! The switch restores the prior rule. Losing a hunter ends flight after configured calm;
//! zero lost-calm retains the old hold. A perch ends flight, holds under an awake threat,
//! allows turning/feeding, and releases after its configured lost-calm hysteresis.

use super::*;
use crate::perception::{posture, posture_of};
use crate::testpass3_support::{V, introduce, plain};
use cubarium_voxel::{DT, World};
use cubarium_voxel_flora::{Flora, FloraConfig};
use std::f64::consts::{FRAC_PI_2, PI};

const SP: Founder = Founder::Seedporter;

fn fixture() -> (World, Flora, Fauna) {
    let w = plain(48, 16, 24);
    let mut c = FaunaConfig::default();
    c.predation.still_threat_near_m = 0.5;
    c.predation.still_after_s = 4.0 * DT;
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.founders[SP.index()].vigilance = Vigilance {
        mode: VigilMode::Arboreal,
        watch_calm: true,
        calm_facing_deg: 90.0,
        calm_margin_m: 2.0,
        flee_start_m: 3.0,
        flee_stop_m: 4.0,
        calm_s: 2.0 * DT,
        lost_calm_s: 6.0 * DT,
        ..Default::default()
    };
    c.teachers.press_replan_s = 3.0 * DT;
    let mut fauna = Fauna::new(c);
    introduce(&mut fauna, &w, SP, 12, 6);
    (w, Flora::new(FloraConfig::default()), fauna)
}

fn track(a: &Animal) -> Track {
    Track {
        known: true,
        id: 91,
        class: class::THREAT,
        lineage: Founder::Chorister.index() as u8,
        modes: SIGHT,
        posture: posture::REST,
        x: a.pose.x,
        z: a.pose.z - 2.0,
        y_m: a.pose.y_m,
        heading: PI,
        ..Default::default()
    }
}

fn record(t: Track) -> Percept {
    let mut p = Percept {
        n: 1,
        ..Default::default()
    };
    p.tracks[0] = t;
    p
}

fn vg(f: &Fauna) -> Vigilance {
    f.config.founder(SP).vigilance
}
fn ring(w: &World) -> f64 {
    f64::from(w.config().width) * V
}

#[test]
fn facing_boundary_is_a_threat_on_both_sides_of_heading() {
    let (w, _, f) = fixture();
    let a = f.animals[0];
    for sign in [-1.0, 1.0] {
        let mut t = track(&a);
        t.heading = sign * FRAC_PI_2;
        assert!(
            !calm_of(&f.config, &vg(&f), &a, &t, ring(&w)),
            "at configured facing boundary"
        );
        t.heading = sign * (FRAC_PI_2 + 0.01);
        assert!(
            calm_of(&f.config, &vg(&f), &a, &t, ring(&w)),
            "just outside it"
        );
    }
}

#[test]
fn facing_reads_short_bearing_across_the_ring_seam() {
    let (w, _, mut f) = fixture();
    f.config.founders[SP.index()].vigilance.calm_facing_deg = 45.0;
    let mut a = f.animals[0];
    a.pose.x = 0.1;
    let mut t = track(&a);
    t.x = ring(&w) - 2.0;
    t.z = a.pose.z;
    t.heading = FRAC_PI_2;
    assert!(!calm_of(&f.config, &vg(&f), &a, &t, ring(&w)));
    t.heading = -FRAC_PI_2;
    assert!(calm_of(&f.config, &vg(&f), &a, &t, ring(&w)));
}

#[test]
fn hearing_tremor_and_movement_override_sleeping_posture() {
    let (w, _, f) = fixture();
    let a = f.animals[0];
    for modes in [SIGHT | HEARING, SIGHT | TREMOR, HEARING, TREMOR] {
        let mut t = track(&a);
        t.posture = posture::ASLEEP;
        t.modes = modes;
        assert!(!calm_of(&f.config, &vg(&f), &a, &t, ring(&w)));
    }
    let mut t = track(&a);
    t.posture = posture::ASLEEP;
    t.moving = true;
    assert!(!calm_of(&f.config, &vg(&f), &a, &t, ring(&w)));
}

#[test]
fn contact_boundary_overrides_facing_away_but_not_sleep() {
    let (w, _, f) = fixture();
    let a = f.animals[0];
    let mut t = track(&a);
    t.z = a.pose.z - f.config.predation.still_threat_near_m;
    assert!(!calm_of(&f.config, &vg(&f), &a, &t, ring(&w)));
    t.z -= 0.01;
    assert!(calm_of(&f.config, &vg(&f), &a, &t, ring(&w)));
    t.z = a.pose.z;
    t.posture = posture::ASLEEP;
    assert!(calm_of(&f.config, &vg(&f), &a, &t, ring(&w)));
}

#[test]
fn watch_off_restores_unwatched_contact_threat() {
    let (w, _, mut f) = fixture();
    f.config.founders[SP.index()].vigilance.watch_calm = false;
    let a = f.animals[0];
    let mut t = track(&a);
    t.posture = posture::ASLEEP;
    t.z = a.pose.z - f.config.predation.still_threat_near_m;
    let got = threat_readout(&f.config, &a, &record(t), 0, ring(&w), V);
    assert_eq!((got[0].2, got[0].3), (true, false));
}

#[test]
fn calm_margin_boundary_uses_height_and_walks_without_food() {
    let (w, _, f) = fixture();
    let mut a = f.animals[0];
    a.pose.heading_rad = -FRAC_PI_2;
    let margin = vg(&f).calm_margin_m;
    let t = Threat {
        d: 1.0,
        d3: margin,
        x: a.pose.x + 1.0,
        z: a.pose.z,
        id: 91,
        calm: true,
        pressing: false,
    };
    let act = calm_walk(&f, &w.view(), &a, &vg(&f), &[t], ring(&w)).expect("on the 3D margin");
    assert_eq!((act.forward, act.feed), (1.0, 0.0));
    assert!(
        calm_walk(
            &f,
            &w.view(),
            &a,
            &vg(&f),
            &[Threat {
                d3: margin + 0.01,
                ..t
            }],
            ring(&w)
        )
        .is_none()
    );
    let mut sight = track(&a);
    sight.posture = posture::ASLEEP;
    sight.z = a.pose.z - 1.0;
    sight.y_m = a.pose.y_m + margin;
    let th = threats(&f.config, &a, &record(sight), 0, ring(&w), V);
    assert!(th[0].d < margin && th[0].d3 > margin);
    assert!(calm_walk(&f, &w.view(), &a, &vg(&f), &th, ring(&w)).is_none());
}

#[test]
fn refused_calm_walk_stops_at_replan_boundary() {
    let (w, _, mut f) = fixture();
    let a = f.animals[0];
    let t = Threat {
        d: 1.0,
        d3: 1.0,
        x: a.pose.x + 1.0,
        z: a.pose.z,
        id: 91,
        calm: true,
        pressing: false,
    };
    let limit = cubarium_voxel::ticks_at_least_one(f.config.teachers.press_replan_s);
    f.pressed.insert(a.id, limit - 1);
    assert!(calm_walk(&f, &w.view(), &a, &vg(&f), &[t], ring(&w)).is_some());
    f.pressed.insert(a.id, limit);
    assert!(calm_walk(&f, &w.view(), &a, &vg(&f), &[t], ring(&w)).is_none());
}

fn fleeing(a: &Animal) -> Vigil {
    Vigil {
        fleeing: true,
        from_x: a.pose.x,
        from_z: a.pose.z - 1.0,
        flee_id: 91,
        close_tick: 10,
        ..Default::default()
    }
}

#[test]
fn lost_hunter_timer_ends_exactly_at_its_boundary() {
    let (w, _, f) = fixture();
    let a = f.animals[0];
    let mut v = fleeing(&a);
    let due = v.close_tick + ticks(vg(&f).lost_calm_s);
    flight(&mut v, &vg(&f), &a, &[], 1, due - 1, ring(&w));
    assert!(v.fleeing);
    flight(&mut v, &vg(&f), &a, &[], 1, due, ring(&w));
    assert!(!v.fleeing);
}

#[test]
fn lost_hunter_release_obeys_the_longer_calm() {
    let (w, _, f) = fixture();
    let a = f.animals[0];
    let mut rule = vg(&f);
    rule.calm_s = 9.0 * DT;
    let mut v = fleeing(&a);
    flight(
        &mut v,
        &rule,
        &a,
        &[],
        1,
        10 + ticks(rule.lost_calm_s),
        ring(&w),
    );
    assert!(v.fleeing);
    flight(&mut v, &rule, &a, &[], 1, 10 + ticks(rule.calm_s), ring(&w));
    assert!(!v.fleeing);
}

#[test]
fn a_reappearing_hunter_restarts_the_lost_clock() {
    let (w, _, f) = fixture();
    let a = f.animals[0];
    let mut v = fleeing(&a);
    let t = Threat {
        d: vg(&f).flee_stop_m,
        d3: vg(&f).flee_stop_m,
        x: v.from_x,
        z: v.from_z,
        id: 91,
        calm: false,
        pressing: true,
    };
    let now = 10 + ticks(vg(&f).lost_calm_s);
    flight(&mut v, &vg(&f), &a, &[t], 1, now, ring(&w));
    assert!(v.fleeing);
    assert_eq!(v.close_tick, now);
    flight(
        &mut v,
        &vg(&f),
        &a,
        &[],
        1,
        now + ticks(vg(&f).lost_calm_s) - 1,
        ring(&w),
    );
    assert!(v.fleeing);
}

#[test]
fn zero_lost_calm_preserves_unseen_flight_hold() {
    let (w, _, f) = fixture();
    let a = f.animals[0];
    let mut rule = vg(&f);
    rule.lost_calm_s = 0.0;
    let mut v = fleeing(&a);
    flight(
        &mut v,
        &rule,
        &a,
        &[],
        1,
        10 + 100 * ticks(rule.calm_s),
        ring(&w),
    );
    assert!(v.fleeing);
}

#[test]
fn perched_watch_uses_start_then_stop_and_releases_at_lost_boundary() {
    let (w, flora, mut f) = fixture();
    f.animals[0].mobility.mode = MobilityMode::Perched;
    let mut t = track(&f.animals[0]);
    t.posture = posture::ACTIVE;
    t.z = f.animals[0].pose.z - vg(&f).flee_start_m;
    t.tick = 10;
    let id = f.animals[0].id;
    f.percepts.insert(id, record(t));
    f.tick = 10;
    let held = Actions {
        forward: 1.0,
        turn: 0.25,
        feed: 0.75,
    };
    let act = arboreal(&mut f, 0, &w.view(), &flora.view(), ring(&w), held)
        .expect("refuge at start boundary");
    assert_eq!(
        (act.forward, act.turn, act.feed),
        (0.0, held.turn, held.feed)
    );
    assert!(f.percepts[&id].vigil.refuge && !f.percepts[&id].vigil.fleeing);
    t.z = f.animals[0].pose.z - vg(&f).flee_stop_m;
    t.tick = 11;
    f.percepts.get_mut(&id).unwrap().tracks[0] = t;
    f.tick = 11;
    assert!(arboreal(&mut f, 0, &w.view(), &flora.view(), ring(&w), held).is_some());
    f.percepts.get_mut(&id).unwrap().n = 0;
    f.tick = 11 + ticks(vg(&f).lost_calm_s) - 1;
    assert!(arboreal(&mut f, 0, &w.view(), &flora.view(), ring(&w), held).is_some());
    f.tick += 1;
    assert!(arboreal(&mut f, 0, &w.view(), &flora.view(), ring(&w), held).is_none());
}

#[test]
#[ignore = "FINDING: zero lost_calm_s releases an unseen refuge instead of disabling release"]
fn zero_lost_calm_preserves_unseen_refuge_hold() {
    let (w, flora, mut f) = fixture();
    f.config.founders[SP.index()].vigilance.lost_calm_s = 0.0;
    f.animals[0].mobility.mode = MobilityMode::Perched;
    let id = f.animals[0].id;
    let mut p = Percept::default();
    p.vigil.refuge = true;
    p.vigil.close_tick = 10;
    f.percepts.insert(id, p);
    f.tick = 10 + 10 * ticks(vg(&f).calm_s);
    let got = arboreal(
        &mut f,
        0,
        &w.view(),
        &flora.view(),
        ring(&w),
        Actions::default(),
    );
    assert!(
        got.is_some(),
        "zero lost-calm should hold: now={}, close_tick={}, response={got:?}, refuge={}",
        f.tick,
        f.percepts[&id].vigil.close_tick,
        f.percepts[&id].vigil.refuge
    );
    assert!(f.percepts[&id].vigil.refuge);
}

#[test]
fn held_young_hunter_reads_still_only_after_configured_ticks() {
    let (w, _, mut f) = fixture();
    let i = introduce(&mut f, &w, Founder::Chorister, 20, 6);
    let limit = f.config.predation.still_after_ticks();
    f.animals[i].state = State::Walking;
    f.animals[i].speed_m_s = 0.0;
    f.animals[i].still_ticks = limit - 1;
    assert_eq!(posture_of(&f.config, &f.animals[i]), posture::ACTIVE);
    f.animals[i].still_ticks = limit;
    assert_eq!(posture_of(&f.config, &f.animals[i]), posture::REST);
    f.animals[i].state = State::Cropping;
    assert_eq!(posture_of(&f.config, &f.animals[i]), posture::ACTIVE);
}

#[test]
fn reaching_a_perch_ends_flight_and_a_waking_facing_hunter_holds_refuge() {
    let (w, flora, mut f) = fixture();
    f.animals[0].mobility.mode = MobilityMode::Perched;
    let a = f.animals[0];
    let mut t = track(&a);
    t.posture = posture::ASLEEP;
    t.tick = 10;
    let mut p = record(t);
    p.vigil = fleeing(&a);
    f.percepts.insert(a.id, p);
    f.tick = 10;
    let held = Actions {
        forward: 1.0,
        turn: 0.2,
        feed: 1.0,
    };
    assert!(arboreal(&mut f, 0, &w.view(), &flora.view(), ring(&w), held).is_none());
    assert!(!f.percepts[&a.id].vigil.fleeing && !f.percepts[&a.id].vigil.refuge);
    t.posture = posture::REST;
    t.heading = 0.0; // still but facing the prey from the north
    t.tick = 11;
    f.tick = 11;
    f.percepts.get_mut(&a.id).unwrap().tracks[0] = t;
    let act = arboreal(&mut f, 0, &w.view(), &flora.view(), ring(&w), held)
        .expect("awake threat holds the perch");
    assert!(f.percepts[&a.id].vigil.refuge && !f.percepts[&a.id].vigil.fleeing);
    assert_eq!(
        (act.forward, act.turn, act.feed),
        (0.0, held.turn, held.feed)
    );
}
