//! **CHURN**: a committed pack keeps its target. The give-up reads time without progress
//! (closing in, or working the target), not a chase clock; another target takes the hunt
//! only past `Pack::switch_margin` of the current one's score. A child of `step` sharing the
//! predation fixture.
//!
//! **CHURN2**: a switch is weighed only once the chase has stalled (`Pack::switch_stall_s`
//! without progress), on what the prey is worth and how open it is (prospect × body, the
//! same members counted for both), not on which is nearest a member.
//!
//! Every case sets the config it relies on; no shipped default, ratio or verdict is asserted.

use super::predation_fixture::*;
use crate::{Activity, ChaseEnd, Command, Fauna, FaunaConfig, Founder, HuntConfig, PackEvent, StartingStores, VigilMode};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const CH: Founder = Founder::Chorister;
const PREY: Founder = Founder::Browser;
const RING: f64 = 48.0 * VOXEL_M;

fn world(seed: u64) -> World {
    plain(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, seed)
}

/// Every lineage awake, the patrol on with no exploring, no dodge, the Veilhound's sight all
/// round to 7 m (still bodies too), remote senses every tick; the prey with no hunker and no
/// ground response, so the prospect is 1 on any of it and the score is its body (exponent 1)
/// over the distance term (`select_judge_m`, set per case); the reach covers the plain; no
/// stalk.
fn config(judge_m: f64) -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[PREY.index()].agility = 0.0;
    c.founders[PREY.index()].hunker.enabled = false;
    c.founders[PREY.index()].vigilance.mode = VigilMode::None;
    c.hunt = HuntConfig { patrol: true, patrol_explore: 0.0, ..HuntConfig::default() };
    c.founders[CH.index()].senses = crate::SenseTable {
        live: true,
        sight_m: 7.0,
        sight_night_m: 7.0,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: 180.0,
        hearing_m: 0.0,
        hearing_min: 1e-12,
        ..crate::SenseTable::default()
    };
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let cruise = c.manifest(CH).cruise_m_per_s;
    let pk = c.founders[CH.index()].pack.as_mut().expect("the chorister's pack table");
    pk.select_kin_factor = 1.0;
    pk.select_aware_factor = 1.0;
    pk.select_wound_gain = 0.0;
    pk.select_airborne_factor = 1.0;
    pk.select_body_exp = 1.0;
    pk.select_judge_m = judge_m;
    pk.select_min_prospect = 0.0;
    pk.pack_radius_m = 6.0;
    pk.commit_m = 1e9; // no stalk: members rush from the commit
    pk.route_reach_fraction = 8.0 / (cruise * pk.chase_give_up_s);
    c
}

/// A hungry Veilhound pack founded at x ∈ [2, 2.5] m on its heuristics; the members.
fn pack(fauna: &mut Fauna, world: &World) -> Vec<u64> {
    let den = Site { x: 10, y: SURFACE_Y, z: 6 };
    assert!(fauna.apply(world, Command::FoundPack { den, founder: CH, stores: HUNTING, heading_rad: EAST }));
    let teachers = fauna.config.teachers;
    let mut members: Vec<u64> = fauna.animals.iter().filter(|a| a.founder == Some(CH)).map(|a| a.id).collect();
    members.sort_unstable();
    for (k, &id) in members.iter().enumerate() {
        fauna.controllers.set(id, crate::controller::heuristic(CH, teachers));
        place(fauna, id, 2.0 + 0.5 * (k / 2) as f64, 0.75 + 1.5 * (k % 2) as f64, EAST);
        animal_mut(fauna, id).eating = true;
    }
    members
}

/// A still frondgrazer of `fraction` of its adult body at `(x, z)`.
fn grazer(fauna: &mut Fauna, world: &World, x: f64, z: f64, fraction: f64) -> u64 {
    let id = introduce(fauna, world, PREY, StartingStores::FULL);
    place(fauna, id, x, z, WEST);
    let birth = fauna.config.founder(PREY).core.birth_body;
    animal_mut(fauna, id).body = fraction * birth;
    drive(fauna, id, 0.0, false);
    id
}

fn nearest(fauna: &Fauna, members: &[u64], prey: u64) -> f64 {
    let p = animal(fauna, prey).pose;
    members
        .iter()
        .map(|&m| {
            let a = animal(fauna, m).pose;
            let (dx, dz) = crate::pose::wrapped_delta(a.x, a.z, p.x, p.z, RING);
            (dx * dx + dz * dz).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

fn step_s(fauna: &mut Fauna, world: &World, flora: &mut Flora, s: f64, events: &mut Vec<PackEvent>) {
    for _ in 0..cubarium_voxel::ticks_at_least_one(s) {
        fauna.step_with(world, flora, 1);
        events.extend(fauna.take_pack_events());
    }
}

fn ends(events: &[PackEvent], why: ChaseEnd) -> usize {
    events.iter().filter(|e| matches!(e, PackEvent::ChaseEnd { why: w, .. } if *w == why)).count()
}

// ------------------------------------------------------------------ the give-up

/// **A pack keeps a target it is closing on past `chase_give_up_s`**: a pack commits to a
/// still grazer over 3 m off, its give-up is then cut to 0.3 s, and three give-ups later it
/// still holds it (or killed it), nearer, with no chase ended late. The control arm, the same scene with a closing step
/// (`chase_progress_m`) longer than the plain so no closing counts, drops it late.
#[test]
fn a_pack_keeps_a_closing_target_past_chase_give_up_s() {
    let arm = |progress_m: f64| {
        let world = world(301);
        let mut flora = Flora::new(FloraConfig::default());
        let c = config(6.0);
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let members = pack(&mut fauna, &world);
        let g = grazer(&mut fauna, &world, 8.0, LANE_Z, 0.5);
        step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
        assert_eq!(fauna.packs()[0].target, Some(g), "the only prey is the target");
        // Committed (its reach is the chase's), the give-up shortens to well under the walk.
        let mut c = c;
        let pk = c.founders[CH.index()].pack.as_mut().unwrap();
        pk.chase_give_up_s = 0.3;
        pk.chase_progress_m = progress_m;
        pk.route_reach_fraction = 1.0;
        let give_up = pk.chase_give_up_s;
        fauna.replace_config(c).expect("the shorter give-up validates");
        let at_commit = nearest(&fauna, &members, g);
        let mut events = Vec::new();
        step_s(&mut fauna, &world, &mut flora, 3.0 * give_up, &mut events);
        // A kill ends the hunt held (the grazer gone, the pack's chase kill booked).
        let killed = fauna.ledger.pack_chase_kills > 0;
        let now = if killed { 0.0 } else { nearest(&fauna, &members, g) };
        (at_commit, now, fauna.packs()[0].target == Some(g) || killed, ends(&events, ChaseEnd::Late), fauna.ledger.pack_chases)
    };
    let (d0, d1, kept, late, chases) = arm(0.25);
    assert!(d0 > 3.0, "the case commits from afar: {d0} m");
    assert!(d1 < d0 - 0.5, "the pack closed in over the three give-ups: {d0} -> {d1} m");
    assert!(kept && late == 0 && chases == 1, "a closing target is kept: kept {kept}, late ends {late}, chases {chases}");
    let (_, _, kept, late, _) = arm(100.0);
    assert!(!kept && late >= 1, "control (no closing counts): kept {kept}, late ends {late}");
}

// ------------------------------------------------------------------ the switch

/// The scene for a switch: a pack commits to a still grazer `a` of 0.3 of the adult body
/// (its chase stalled at once: no closing counts, `switch_stall_s` 0.3 s),
/// then a second grazer `b` of `ratio` × its body is put across the pack's line from it and
/// a metre nearer the pack (CHURN2: a switch takes only prey no farther from the pack than
/// the target); the score is body only (`select_judge_m` huge). Returns, a second
/// later, whether the pack holds `b`, holds `a`, its chases and its switch ends.
fn switch_case(ratio: f64, margin: f64) -> (bool, bool, u64, usize) {
    let world = world(302);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config(1e9);
    let pk = c.founders[CH.index()].pack.as_mut().unwrap();
    pk.switch_margin = margin;
    // CHURN2: a switch needs a stalled chase; here no closing counts as progress and the
    // stall is short, so the chase is open to a switch well inside the second.
    pk.chase_progress_m = 100.0;
    pk.switch_stall_s = 0.3;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let _members = pack(&mut fauna, &world);
    let a = grazer(&mut fauna, &world, 7.0, LANE_Z - 0.6, 0.3);
    step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
    assert_eq!(fauna.packs()[0].target, Some(a), "the only prey is the target");
    let b = grazer(&mut fauna, &world, 6.0, LANE_Z + 0.6, 0.3 * ratio);
    let mut events = Vec::new();
    step_s(&mut fauna, &world, &mut flora, 1.0, &mut events);
    let t = fauna.packs()[0].target;
    (t == Some(b), t == Some(a), fauna.ledger.pack_chases, ends(&events, ChaseEnd::Switched))
}

/// **A target that scores slightly better does not steal the hunt; one past the margin
/// does**: with `switch_margin` 1.5, a newcomer scoring 1.2 × the target is ignored, and one
/// scoring 2 × takes the hunt (one switch, one more chase).
#[test]
fn a_slightly_better_target_does_not_steal_the_hunt_and_one_past_the_margin_does() {
    let (on_b, on_a, chases, switches) = switch_case(1.2, 1.5);
    assert!(on_a && !on_b && chases == 1 && switches == 0, "1.2x under a 1.5 margin: on a {on_a}, on b {on_b}, chases {chases}, switches {switches}");
    let (on_b, on_a, chases, switches) = switch_case(2.0, 1.5);
    assert!(on_b && !on_a && chases == 2 && switches == 1, "2x past a 1.5 margin: on a {on_a}, on b {on_b}, chases {chases}, switches {switches}");
}

/// **Two equal targets do not ping-pong**: two still grazers of one body either side of the
/// pack's line, the score weighing distance (so members on either side see either one
/// better); over five seconds of the chase (or until its kill) the pack holds the one it took, with no
/// switch and no second chase.
#[test]
fn two_equal_targets_do_not_ping_pong() {
    let world = world(303);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config(1.0);
    c.founders[CH.index()].pack.as_mut().unwrap().switch_margin = 1.25;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let _members = pack(&mut fauna, &world);
    let a = grazer(&mut fauna, &world, 6.0, 0.5, 0.4);
    let b = grazer(&mut fauna, &world, 6.0, 2.5, 0.4);
    step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
    let first = fauna.packs()[0].target;
    assert!(first == Some(a) || first == Some(b), "it took one of the two: {first:?}");
    let mut events = Vec::new();
    let mut held = true;
    for _ in 0..5 * crate::TICK_HZ {
        step_s(&mut fauna, &world, &mut flora, 0.0, &mut events);
        if fauna.ledger.pack_chase_kills > 0 {
            break; // the hunt ended in a kill: a new one may begin
        }
        held &= fauna.packs()[0].target == first;
    }
    assert!(held, "the pack held {first:?} (now {:?})", fauna.packs()[0].target);
    // STRIKE: a kill can end the hunt and a new one begin on the same tick; the chases
    // counted are those before the kill.
    let before_kill = events.iter().position(|e| matches!(e, PackEvent::ChaseEnd { why: ChaseEnd::Killed, .. })).unwrap_or(events.len());
    let after_kill = events[before_kill..].iter().filter(|e| matches!(e, PackEvent::ChaseStart { .. })).count() as u64;
    let chases = fauna.ledger.pack_chases - after_kill;
    assert_eq!((chases, ends(&events, ChaseEnd::Switched)), (1, 0), "(chases, switches): {events:?}");
}

// ------------------------------------------------------------------ CHURN2: the stall

/// The scene for the stall rule: a pack commits to a still grazer `a` of 0.3 of the adult
/// body ahead of it (x = 7 m); then a grazer `b` of `ratio` × that body is put behind the
/// pack (x = 1 m, so no member passes over it), nearer the pack than `a`. The score
/// weighs distance (`select_judge_m` 1 m), so `b` also scores past any margin on the
/// choice's own score; `switch_margin` 1.5, `switch_stall_s` 0.1 s (a stall is weighed at
/// the first samplings, while `b` is still the nearer), and progress is closing by
/// `progress_m`. Returns, after `secs`, whether the pack holds `a`, whether it holds `b`,
/// how far the pack closed on `a` meanwhile, and its switch ends.
fn stall_case(progress_m: f64, ratio: f64, secs: f64) -> (bool, bool, f64, usize) {
    let world = world(304);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config(1.0);
    let pk = c.founders[CH.index()].pack.as_mut().unwrap();
    pk.switch_margin = 1.5;
    pk.switch_stall_s = 0.1;
    pk.chase_progress_m = progress_m;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world);
    let a = grazer(&mut fauna, &world, 7.0, LANE_Z, 0.3);
    step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
    assert_eq!(fauna.packs()[0].target, Some(a), "the only prey is the target");
    let d0 = nearest(&fauna, &members, a);
    let b = grazer(&mut fauna, &world, 1.0, LANE_Z, 0.3 * ratio);
    assert!(nearest(&fauna, &members, b) < d0, "b is nearer the pack than a");
    let mut events = Vec::new();
    step_s(&mut fauna, &world, &mut flora, secs, &mut events);
    let t = fauna.packs()[0].target;
    (t == Some(a), t == Some(b), d0 - nearest(&fauna, &members, a), ends(&events, ChaseEnd::Switched))
}

/// **A progressing chase keeps its target**, even with a prey nearer the pack that is worth
/// four times as much: the pack closes on its grazer (by more than `chase_progress_m` each
/// sampling), and over a second it holds it with no switch.
#[test]
fn a_progressing_chase_keeps_its_target_against_a_far_better_prey() {
    let (on_a, on_b, closed, switches) = stall_case(0.05, 4.0, 1.0);
    assert!(closed > 0.05, "the pack closed on its target: {closed} m");
    assert!(on_a && !on_b && switches == 0, "progressing: on a {on_a}, on b {on_b}, switches {switches}");
}

/// **A stalled chase takes that prey**: the same scene with a closing step longer than the
/// plain (no closing counts, so the chase stalls after `switch_stall_s`), and the four-times
/// prey takes the hunt (one switch).
#[test]
fn a_stalled_chase_takes_a_far_better_prey() {
    let (on_a, on_b, _, switches) = stall_case(100.0, 4.0, 1.0);
    assert!(on_b && !on_a && switches == 1, "stalled: on a {on_a}, on b {on_b}, switches {switches}");
}

/// **A nearer but no better prey does not take a stalled chase**: the newcomer has the
/// target's body (worth the same), and though it is nearer the pack — so the choice's
/// distance-weighed score puts it past the margin — the stalled pack keeps its target.
#[test]
fn a_nearer_but_no_better_prey_does_not_take_a_stalled_chase() {
    let (on_a, on_b, _, switches) = stall_case(100.0, 1.0, 1.0);
    assert!(on_a && !on_b && switches == 0, "stalled, b no better: on a {on_a}, on b {on_b}, switches {switches}");
}
