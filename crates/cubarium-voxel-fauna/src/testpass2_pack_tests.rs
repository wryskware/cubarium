//! **TEST-PASS-2 (independent), the pack's joins**: CHURN's progress clock against STRIKE's
//! close-in and step-off, CHURN2's switch gate and its one switch per stall, STRIKE's
//! out-of-reach break-off, STRIKE's step-off against a wall and EDGE-2's step out of solid,
//! PIT-2's whelping at the den against HUNTER-MEAL's carcass and LIFE-HISTORY's season. A child
//! of `step` sharing the predation fixture.
//!
//! Every case sets the config it relies on and takes its bounds from it; no shipped default,
//! ratio or verdict is asserted.

use super::predation_fixture::*;
use crate::{Activity, ChaseEnd, Command, Escrow, Fauna, FaunaConfig, Founder, HuntConfig, PackEvent, StartingStores, VigilMode};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Carcass, Deposit, DepositKind, Flora, FloraConfig, Site};
use std::f64::consts::PI;

const CH: Founder = Founder::Chorister;
const PREY: Founder = Founder::Browser;
const RING: f64 = 48.0 * VOXEL_M;

fn world(seed: u64) -> World {
    plain(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, seed)
}

/// Every lineage awake, the patrol on with no exploring; the prey with no dodge, no hunker and
/// no ground response, so the prospect is 1 and the switch's worth is its body (exponent 1);
/// the Veilhound's sight all round to 7 m (still bodies too), remote senses every tick; no
/// stalk; a strike that does no harm (`strike_power` 0: no wound, so no work on the target
/// counts as progress); every grazer in one member's window; the reach covers the plain.
fn config() -> FaunaConfig {
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
    {
        let h = c.founders[CH.index()].hunter.as_mut().expect("the chorister hunts");
        h.strike_power = 0.0;
        // Any grazer a case sizes is in one member's window.
        h.prey_max_ratio = 1e3;
    }
    let cruise = c.manifest(CH).cruise_m_per_s;
    let pk = c.founders[CH.index()].pack.as_mut().expect("the chorister's pack table");
    pk.select_kin_factor = 1.0;
    pk.select_aware_factor = 1.0;
    pk.select_wound_gain = 0.0;
    pk.select_airborne_factor = 1.0;
    pk.select_body_exp = 1.0;
    pk.select_judge_m = 1e9;
    pk.select_min_prospect = 0.0;
    pk.break_off_below = 0.0;
    pk.pack_radius_m = 6.0;
    pk.commit_m = 1e9; // no stalk
    pk.chase_give_up_s = 1e6;
    pk.switch_stall_s = 1e6;
    pk.route_reach_fraction = 8.0 / (cruise * pk.chase_give_up_s);
    // The old pack tactic's rules: VH-TACTICS off (the comparison arm).
    crate::pack::tactics::old_tactic(&mut c);
    c
}

/// A hungry Veilhound pack founded at x ∈ [2, 2.5] m on its heuristics; the members, by id.
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

/// A still grazer of `fraction` of its adult body at `(x, z)`.
fn grazer(fauna: &mut Fauna, world: &World, x: f64, z: f64, fraction: f64) -> u64 {
    let id = introduce(fauna, world, PREY, StartingStores::FULL);
    place(fauna, id, x, z, WEST);
    let birth = fauna.config.founder(PREY).core.birth_body;
    animal_mut(fauna, id).body = fraction * birth;
    drive(fauna, id, 0.0, false);
    id
}

fn distance(fauna: &Fauna, a: u64, b: u64) -> f64 {
    let (p, q) = (animal(fauna, a).pose, animal(fauna, b).pose);
    let (dx, dz) = crate::pose::wrapped_delta(p.x, p.z, q.x, q.z, RING);
    (dx * dx + dz * dz).sqrt()
}

fn nearest(fauna: &Fauna, members: &[u64], prey: u64) -> f64 {
    members.iter().filter(|&&m| fauna.view().animal(m).is_some()).map(|&m| distance(fauna, m, prey)).fold(f64::INFINITY, f64::min)
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

fn switches(events: &[PackEvent]) -> usize {
    ends(events, ChaseEnd::Switched)
}

/// Whether body `id` held forward over its last interval and its motor delivered under the
/// config's `motor_refused` share of it (the tactic's own pressed reading).
fn pressing(fauna: &Fauna, id: u64) -> bool {
    let a = animal(fauna, id);
    let fb = a.founder_state.feedback;
    a.founder_state.held.forward > 0.0 && fb.attempted_equivalent > 1e-12 && fb.delivered_equivalent / fb.attempted_equivalent < fauna.config.teachers.motor_refused
}

fn striking(fauna: &Fauna, id: u64, prey: u64) -> bool {
    animal(fauna, id).strike.is_some_and(|s| s.prey == prey)
}

/// STRIKE's time bound from the config: two half turns at the yaw cap, a walk of twice the
/// stand-off at cruise, three controller periods and the windup.
fn strike_bound(fauna: &Fauna, id: u64) -> u32 {
    let m = fauna.config.manifest(CH);
    let phys = fauna.config.founder(CH);
    let h = phys.hunter.unwrap();
    let pk = phys.pack.unwrap();
    let length = phys.body_at(animal(fauna, id).body).length_m;
    let stand = (h.jaw_forward_length_fraction + pk.stand_off_reach_fraction * h.strike_reach_length_fraction) * length;
    let s = 2.0 * PI / m.yaw_cap_rad_per_s + 2.0 * stand / m.cruise_m_per_s + 3.0 * m.controller_period_s + h.windup_s;
    cubarium_voxel::ticks_at_least_one(s) as u32
}

// ------------------------------------------------------------------ CHURN x STRIKE: the progress clock

/// The breakaway scene: a pack commits to a still grazer 6 m off, and its give-up is then cut
/// to 1 s (closing by `chase_progress_m` 0.25 m counts); when `contact` its members
/// first close on it until the nearest is within a metre (the close-in, at the jaw), else not;
/// then the grazer is moved 6 m beyond the nearest member, as a prey that breaks away. Over
/// the next 3 s (the pack
/// closing at cruise, well over 0.25 m a second, and the grazer still more than a metre off):
/// whether it kept the target, its late ends, and how far the pack closed.
fn breakaway(contact: bool) -> (bool, usize, f64) {
    let world = world(901);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world);
    let g = grazer(&mut fauna, &world, 8.0, LANE_Z, 0.5);
    step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
    assert_eq!(fauna.packs()[0].target, Some(g), "fixture: the only prey is the target");
    // Committed (its reach is the chase's), the give-up shortens to a second.
    {
        let pk = c.founders[CH.index()].pack.as_mut().unwrap();
        pk.chase_give_up_s = 1.0;
        pk.route_reach_fraction = 1.0;
        pk.chase_progress_m = 0.25;
    }
    fauna.replace_config(c).expect("the shorter give-up validates");
    if contact {
        step_until(&mut fauna, &world, &mut flora, 400, |f| nearest(f, &members, g) <= 1.0).expect("fixture: the pack reaches the grazer");
        assert_eq!(fauna.packs()[0].target, Some(g), "fixture: still its target at contact");
    }
    // The breakaway: 6 m beyond the nearest member along the lane.
    let lead = members.iter().map(|&m| animal(&fauna, m).pose.x).fold(f64::NEG_INFINITY, f64::max);
    place(&mut fauna, g, (lead + 6.0).rem_euclid(RING), LANE_Z, WEST);
    let d0 = nearest(&fauna, &members, g);
    let mut events = Vec::new();
    let mut kept = true;
    let mut late = 0;
    for _ in 0..cubarium_voxel::ticks_at_least_one(3.0) {
        step_s(&mut fauna, &world, &mut flora, 0.0, &mut events);
        late = ends(&events, ChaseEnd::Late);
        kept &= fauna.packs()[0].target == Some(g);
        if late > 0 {
            break;
        }
    }
    (kept, late, d0 - nearest(&fauna, &members, g))
}

/// **A target that breaks away after contact is kept while the pack closes on it again**
/// (CHURN's "closing on a fleeing one, keeps it" against STRIKE's close-in): the control, a
/// pack that never reached its grazer, keeps it over three give-ups while it closes; the same
/// pack after a contact (its nearest-so-far a metre or less) is weighed against that contact
/// distance, so its renewed closing never counts and the chase goes late on the first give-up.
///
/// FINDING (evidence): after contact the pack closed 0.52 m in the next second and the chase
/// ended `Late` (kept false, 1 late end); the control kept it over 3 s and closed over 1 m.
/// Progress is `d <= best_m - chase_progress_m` (pack.rs:1505) against the chase's all-time
/// nearest, so after a close-in no renewed closing counts and `late` (pack.rs:1519) fires one
/// give-up after the breakaway. Decision: reset or window the nearest-so-far when the gap opens.
#[test]
fn a_target_that_breaks_away_after_contact_is_kept_while_the_pack_closes_on_it_again() {
    let (kept, late, closed) = breakaway(false);
    assert!(closed > 1.0, "control: the pack closed on the grazer: {closed:.2} m");
    assert!(kept && late == 0, "control (no contact first): kept {kept}, late ends {late}");
    let (kept, late, closed) = breakaway(true);
    assert!(kept && late == 0, "after a contact, closing again by {closed:.2} m: kept {kept}, late ends {late}");
}

// ------------------------------------------------------------------ CHURN2: the switch

/// One controller sample of member `id` by itself, the world held: the tick moves on one, the
/// senses are read (every body, `remote_hz` at the tick rate), and the pack's tactic answers
/// ([`crate::pack::steer`]); nothing moves.
fn sample(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, events: &mut Vec<PackEvent>) {
    fauna.tick += 1;
    let view = world.view();
    crate::perception::sense_pass(fauna, &view, false, 1.0, 1);
    let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
    let _ = crate::pack::steer(fauna, i, &view, &flora.view(), false, 1.0, crate::controller::Actions::REST, None);
    events.extend(fauna.take_pack_events());
}

/// The switch scene, read one sample at a time: a hungry pack commits to a still grazer `a`
/// of 0.3 of the adult body; then the first member stands at `(2, LANE_Z)` facing east, the
/// rest are fed and far off (they take no part), `a` stands 2 m ahead of it, and the chase is
/// stalled (no closing counts, the progress clock `switch_stall_s` old). `switch_margin` 1.5.
/// Returns the world, plant layer, fauna, the member under test and `a`.
fn switch_scene(seed: u64, stall_s: f64) -> (World, Flora, Fauna, u64, u64) {
    let world = world(seed);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    {
        let pk = c.founders[CH.index()].pack.as_mut().unwrap();
        pk.switch_margin = 1.5;
        pk.chase_progress_m = 100.0;
        pk.switch_stall_s = stall_s;
        pk.close_in_m = 0.0;
    }
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world);
    let a = grazer(&mut fauna, &world, 6.0, LANE_Z, 0.3);
    step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
    assert_eq!(fauna.packs()[0].target, Some(a), "fixture: the only prey is the target");
    for (k, &id) in members.iter().enumerate().skip(1) {
        place(&mut fauna, id, 7.0 + 0.5 * k as f64, 0.5, EAST);
        animal_mut(&mut fauna, id).eating = false;
    }
    let m = members[0];
    place(&mut fauna, m, 2.0, LANE_Z, EAST);
    animal_mut(&mut fauna, m).eating = true;
    place(&mut fauna, a, 4.0, LANE_Z, WEST);
    let stall = cubarium_voxel::ticks_at_least_one(stall_s);
    fauna.tick += stall + 1;
    let now = fauna.tick;
    let p = fauna.pack_mut(fauna.packs()[0].id).unwrap();
    p.chase_progress_tick = now - stall - 1;
    (world, flora, fauna, m, a)
}

/// **The switch's distance gate is "no farther than the target"** (CHURN2): a stalled pack,
/// its one hungry member 2 m from its target, and a newcomer worth four times the target
/// behind it: a millimetre nearer than the target, it takes the hunt; a centimetre farther,
/// it does not.
#[test]
fn the_switch_takes_a_prey_no_farther_than_the_target_and_not_one_farther() {
    let run = |extra: f64| {
        let (world, flora, mut fauna, m, a) = switch_scene(911, 0.5);
        let b = grazer(&mut fauna, &world, (2.0 - 2.0 - extra).rem_euclid(RING), LANE_Z, 1.2);
        let (da, db) = (distance(&fauna, m, a), distance(&fauna, m, b));
        let mut events = Vec::new();
        sample(&mut fauna, &world, &flora, m, &mut events);
        (da, db, fauna.packs()[0].target == Some(b), switches(&events))
    };
    let (da, db, on_b, sw) = run(-0.001);
    assert!(db < da, "fixture: b nearer than a ({db:.4} vs {da:.4})");
    assert!(on_b && sw == 1, "no farther than the target ({db:.3} <= {da:.3}): on b {on_b}, switches {sw}");
    let (da, db, on_b, sw) = run(0.01);
    assert!(db > da, "fixture: b farther than a ({db:.4} vs {da:.4})");
    assert!(!on_b && sw == 0, "farther than the target ({db:.3} > {da:.3}): on b {on_b}, switches {sw}");
}

/// **A switch restarts the stall, so a pack switches at most once per `switch_stall_s`**: a
/// stalled pack switches from `a` to `b` (worth four times `a`); a third grazer `c` worth four
/// times `b`, no farther, put in on the switch's tick, is not taken one tick short of
/// `switch_stall_s` after the switch, and is taken at it.
#[test]
fn a_switch_restarts_the_stall_so_a_pack_switches_once_per_stall() {
    let stall_s = 0.5;
    let (world, flora, mut fauna, m, a) = switch_scene(912, stall_s);
    let b = grazer(&mut fauna, &world, 0.5, LANE_Z, 1.2);
    let mut events = Vec::new();
    sample(&mut fauna, &world, &flora, m, &mut events);
    assert!(switches(&events) == 1 && fauna.packs()[0].target == Some(b), "fixture: the stalled pack switches to b: {events:?}");
    let at = fauna.tick;
    // c a millimetre nearer the member than b, off its line (the gate: no farther than b).
    let (rc, th) = (distance(&fauna, m, b) - 0.001, 0.35f64);
    let c = grazer(&mut fauna, &world, 2.0 - rc * th.cos(), LANE_Z + rc * th.sin(), 4.8);
    assert!(distance(&fauna, m, c) < distance(&fauna, m, b), "fixture: c no farther than b");
    let stall = cubarium_voxel::ticks_at_least_one(stall_s);
    fauna.tick = at + stall - 2;
    sample(&mut fauna, &world, &flora, m, &mut events);
    assert_eq!(switches(&events), 1, "a second switch {} ticks after the first, under the stall of {stall}", fauna.tick - at);
    assert_eq!(fauna.packs()[0].target, Some(b));
    fauna.tick = at + stall - 1;
    sample(&mut fauna, &world, &flora, m, &mut events);
    assert!(switches(&events) == 2 && fauna.packs()[0].target == Some(c), "once the stall has run again, c takes the hunt: {events:?}");
    let _ = a;
}

/// **A target switched from is not shunned** (CHURN: "the old target is not shunned"): after
/// the switch from `a` to `b`, `a` is in no live slot of the pack's shun list and is not its
/// abandoned target.
#[test]
fn a_target_switched_from_is_not_shunned() {
    let (world, flora, mut fauna, m, a) = switch_scene(913, 0.5);
    let b = grazer(&mut fauna, &world, 0.5, LANE_Z, 1.2);
    let mut events = Vec::new();
    sample(&mut fauna, &world, &flora, m, &mut events);
    assert!(switches(&events) == 1 && fauna.packs()[0].target == Some(b), "fixture: one switch to b: {events:?}");
    let p = fauna.packs()[0];
    assert!(!p.shunned.iter().any(|&(s, until)| s == a && fauna.tick < until), "a is shunned: {:?}", p.shunned);
    assert_ne!(p.abandoned, Some(a), "a is the abandoned target");
}

// ------------------------------------------------------------------ STRIKE: out of reach

/// **A target on a pillar no member can walk up is broken off once the chase stalls, booked
/// out of reach, and not taken again while it stands there** (STRIKE: "after a stall >=
/// `switch_stall_s` when no taking-part member can walk within chase reach"): a pack commits to
/// a grazer on the plain; it is then put on a rock pillar two columns square whose top is more
/// steps up than the members climb and higher over the plain than their jaw band reaches. No
/// closing counts (`chase_progress_m` past the plain), so the chase stalls `switch_stall_s`
/// after the lift: it is broken off within that and two controller periods, `pack_out_of_reach`
/// booked; with its shun cut short, it is not taken again in two seconds.
#[test]
fn a_target_on_a_pillar_no_member_can_climb_is_broken_off_once_stalled_and_not_taken_again() {
    let mut world = plain_of_height(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, 921, 16);
    let stall_s = 0.5;
    let mut c = config();
    {
        let pk = c.founders[CH.index()].pack.as_mut().unwrap();
        pk.switch_stall_s = stall_s;
        pk.chase_progress_m = 100.0;
    }
    let phys = *c.founder(CH);
    let (h, geo) = (phys.hunter.unwrap(), phys.adult_body());
    let band = h.jaw_band_height_fraction * geo.height_m;
    let rise = ((band / VOXEL_M).ceil() as u32 + 2).max(phys.step_limits(VOXEL_M).up + 2);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let _members = pack(&mut fauna, &world);
    let g = grazer(&mut fauna, &world, 5.0, LANE_Z, 0.3);
    step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
    // The pillar, and the grazer lifted onto it.
    let (px, pz) = (24i64, 6u32);
    for x in px..=px + 1 {
        for z in pz..=pz + 1 {
            for y in SURFACE_Y + 1..=SURFACE_Y + rise {
                world.apply(cubarium_voxel::Command::SetMaterial { x, y, z, material: cubarium_voxel::Material::Rock });
            }
        }
    }
    let (x, z) = ((px as f64 + 1.0) * VOXEL_M, (f64::from(pz) + 1.0) * VOXEL_M);
    place(&mut fauna, g, x, z, WEST);
    {
        let a = animal_mut(&mut fauna, g);
        a.site.y = SURFACE_Y + rise;
        a.pose.y_m = crate::surface_m(SURFACE_Y + rise, VOXEL_M);
    }
    let mut c2 = *fauna.config;
    c2.founders[CH.index()].pack.as_mut().unwrap().route_refresh_s = 0.0;
    fauna.replace_config(c2).unwrap();
    let m = fauna.config.manifest(CH);
    let bound = cubarium_voxel::ticks_at_least_one(stall_s + 2.0 * m.controller_period_s) as u32;
    let mut events = Vec::new();
    for _ in 0..bound {
        step_s(&mut fauna, &world, &mut flora, 0.0, &mut events);
        if ends(&events, ChaseEnd::BrokeOff) > 0 {
            break;
        }
    }
    assert!(ends(&events, ChaseEnd::BrokeOff) >= 1, "broken off within {bound} ticks: {events:?}");
    assert!(fauna.ledger.pack_out_of_reach >= 1, "booked out of reach");
    let chases = fauna.ledger.pack_chases;
    let mut c3 = *fauna.config;
    c3.founders[CH.index()].pack.as_mut().unwrap().shun_s = 0.1;
    fauna.replace_config(c3).unwrap();
    step_s(&mut fauna, &world, &mut flora, 2.0, &mut events);
    assert_eq!(fauna.ledger.pack_chases, chases, "the grazer out of reach is taken again: {events:?}");
}

// ------------------------------------------------------------------ STRIKE x EDGE: the step-off

/// Rock over columns `xs` × `zs`, `rows` voxels over the plain.
fn rock(w: &mut World, xs: std::ops::RangeInclusive<i64>, zs: std::ops::RangeInclusive<u32>, rows: u32) {
    for z in zs {
        for x in xs.clone() {
            for y in SURFACE_Y + 1..=SURFACE_Y + rows {
                w.apply(cubarium_voxel::Command::SetMaterial { x, y, z, material: cubarium_voxel::Material::Rock });
            }
        }
    }
}

/// The scene for one member at its prey: a pack commits to a still grazer on the open plain;
/// then only the first member stays hungry (the rest fed and far off). Returns the fauna, the
/// member and the grazer; the case places them.
fn one_member(world: &World, flora: &mut Flora, c: FaunaConfig) -> (Fauna, u64, u64) {
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, world);
    let g = grazer(&mut fauna, world, 7.5, LANE_Z, 0.5);
    step_until(&mut fauna, world, flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
    for (k, &id) in members.iter().enumerate().skip(1) {
        place(&mut fauna, id, 0.5 + 0.5 * k as f64, 0.5, EAST);
        animal_mut(&mut fauna, id).eating = false;
    }
    let a = members[0];
    {
        let m = animal_mut(&mut fauna, a);
        m.eating = true;
        m.strike = None;
        m.founder_state.held = crate::controller::Actions::REST;
    }
    (fauna, a, g)
}

/// **A member stepping off a prey inside its outline, with its back to a wall, does not press
/// the wall for good** (STRIKE's step-off against EDGE's "a body does not press into a wall"):
/// the member stands clear, facing east, with a rock wall a centimetre behind it, on a face that
/// is a node of its route graph (the graph's margin 0), and the grazer 0.15 m ahead of its
/// centre — inside its outline. The control, the same scene in the open, strikes within
/// STRIKE's bound. With the wall it must strike within twice that bound, and press no longer
/// than `teachers.press_replan_s` at a time.
#[test]
fn a_member_stepping_off_a_prey_with_its_back_to_a_wall_does_not_press_it() {
    let mut c = config();
    c.founders[CH.index()].pack.as_mut().unwrap().close_in_m = 2.5;
    c.founding.clearance_margin_m = 0.0;
    let run = |wall: bool| {
        let mut world = world(931);
        let mut flora = Flora::new(FloraConfig::default());
        let (mut fauna, a, g) = one_member(&world, &mut flora, c.clone());
        // The wall's east face at x = 6 m; the member's back a centimetre off it, on a column
        // whose centre the adult disc stands clear on.
        let r = fauna.config.founder(CH).body_at(animal(&fauna, a).body).footprint_radius();
        let ra = fauna.config.founder(CH).adult_body().footprint_radius();
        let face = 6.0;
        let col = ((face + ra) / VOXEL_M - 0.5).ceil();
        let x = (face + r + 0.01).max(col * VOXEL_M + 1e-6);
        if wall {
            rock(&mut world, 20..=23, 0..=11, 8);
        }
        place(&mut fauna, a, x, LANE_Z, EAST);
        place(&mut fauna, g, x + 0.15, LANE_Z, WEST);
        if wall {
            let graph = fauna.pack_routes(&world.view(), CH);
            assert!(graph.face(animal(&fauna, a).site).is_some(), "fixture: the member's face is a node of its graph (x {x:.3}, r {r:.3})");
        }
        let bound = strike_bound(&fauna, a);
        let cadence = fauna.config.manifest(CH).cadence_ticks().max(1);
        let (mut run, mut longest) = (0u64, 0u64);
        let t = step_until(&mut fauna, &world, &mut flora, 2 * bound, |f| {
            let m = animal(f, a);
            if m.age_ticks % cadence == 0 {
                if pressing(f, a) {
                    run += cadence;
                    longest = longest.max(run);
                } else {
                    run = 0;
                }
            }
            striking(f, a, g)
        });
        (t, bound, longest, fauna.config.teachers.press_replan_s, distance(&fauna, a, g), animal(&fauna, a).pose.x - x)
    };
    let (t, bound, _, _, d, _) = run(false);
    assert!(t.is_some_and(|t| t <= bound), "control, in the open: struck within {bound} ticks ({t:?}; at {d:.2} m)");
    let (t, bound, longest, replan_s, d, moved) = run(true);
    let longest_s = longest as f64 * crate::DT;
    assert!(t.is_some(), "with its back to the wall: no strike within {} ticks; at {d:.2} m from the grazer, moved {moved:.3} m, pressed {longest_s:.2} s at a time", 2 * bound);
    assert!(longest_s <= replan_s, "it pressed the wall {longest_s:.2} s at a time (press_replan_s {replan_s})");
}

/// **A Veilhound caught in a wall beside its prey steps out and strikes** (EDGE-2's step out of
/// solid against STRIKE's close-in): the member stands with its disc a third of its radius into
/// a rock wall at its south (as a body grown against a wall), facing east, the grazer a metre
/// ahead along the wall at the same distance from it. It strikes within STRIKE's bound and the
/// time its cruise takes over its radius and two controller periods, and its disc is never
/// deeper in the wall than it began.
#[test]
fn a_veilhound_caught_in_a_wall_beside_its_prey_steps_out_and_strikes() {
    let mut world = world(932);
    let mut c = config();
    c.founders[CH.index()].pack.as_mut().unwrap().close_in_m = 2.5;
    let mut flora = Flora::new(FloraConfig::default());
    let (mut fauna, a, g) = one_member(&world, &mut flora, c.clone());
    // The wall: rock over z 0..=2 (its north face at z = 0.75 m) on every column.
    rock(&mut world, 0..=47, 0..=2, 8);
    let face = 0.75;
    let r = c.founder(CH).body_at(animal(&fauna, a).body).footprint_radius();
    let z = face + r - r / 3.0;
    place(&mut fauna, a, 4.0, z, EAST);
    place(&mut fauna, g, 5.0, z, WEST);
    let depth = |f: &Fauna| face + r - animal(f, a).pose.z;
    let d0 = depth(&fauna);
    assert!(d0 > 0.0, "fixture: the member is caught in the wall");
    let m = c.manifest(CH);
    let bound = strike_bound(&fauna, a) + cubarium_voxel::ticks_at_least_one(r / m.cruise_m_per_s + 2.0 * m.controller_period_s) as u32;
    let mut deepest = d0;
    let t = step_until(&mut fauna, &world, &mut flora, bound, |f| {
        deepest = deepest.max(depth(f));
        striking(f, a, g)
    });
    assert!(deepest <= d0 + 1e-9, "it went deeper into the wall: {d0:.3} -> {deepest:.3} m");
    assert!(t.is_some(), "no strike within {bound} ticks; at {:.2} m, {:.3} m into the wall", distance(&fauna, a, g), depth(&fauna));
}

// ------------------------------------------------------------------ PIT-2: the litter at the den

/// The den scene for PIT-2's whelping: a pack of one founded on `den` with `whelp_at_den`, its
/// adult the breeder on its heuristic, hungry; `HuntConfig::meal_first` on. Returns the fauna
/// and the breeder.
fn den_scene(world: &World, den: Site, whelp: bool) -> (Fauna, u64) {
    let mut c = config();
    c.hunt = HuntConfig { patrol: true, patrol_explore: 0.0, meal_first: true, ..HuntConfig::default() };
    {
        let pk = c.founders[CH.index()].pack.as_mut().unwrap();
        pk.pack_size = 1;
        pk.den_radius_m = 2.0;
        pk.home_reserve_share = 1.0;
        pk.whelp_at_den = whelp;
    }
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    assert!(fauna.apply(world, Command::FoundPack { den, founder: CH, stores: HUNTING, heading_rad: EAST }));
    let b = fauna.packs()[0].breeder.expect("a breeder");
    let teachers = fauna.config.teachers;
    fauna.controllers.set(b, crate::controller::heuristic(CH, teachers));
    animal_mut(&mut fauna, b).eating = true;
    (fauna, b)
}

/// A kill's carcass on the face under `(x_m, z_m)`, as `kill` lays it.
fn carcass_at(flora: &mut Flora, x_m: f64, z_m: f64, organic: f64) -> Site {
    let site = Site { x: (x_m / VOXEL_M).floor() as u32, y: SURFACE_Y, z: (z_m / VOXEL_M).floor() as u32 };
    assert!(flora.deposit(site, Deposit { kind: DepositKind::Carrion, organic, mineral: 0.05 * organic, energy: 2.0 * organic }));
    assert!(flora.lay_carcass(site, Carcass { lineage: Founder::Blind.index() as u8, heading_rad: 0.0, organic }));
    site
}

/// **A hungry breeder carrying a litter eats a carcass lying at its den** (PIT-2's whelping
/// against HUNTER-MEAL's carcass between hunts): the breeder, hungry, at its den's centre with
/// an open gestation; a carcass within the den radius (three quarters of it) but past half of
/// it, where the whelping rule keeps the breeder. The control (no litter carried) walks to it
/// and eats within the walk's bound; carrying a litter it must eat too — its pups, at the same
/// den, would.
///
/// FINDING (evidence): carrying, it never ate within the bound (it stands); the control ate.
/// The whelping branch (pack.rs:1413-1424) returns before the feast step (pack.rs:1654-1696) and
/// HUNTER-MEAL's meal step (pack.rs:1700-1704); only the emergency latch releases it. Decision:
/// let a carrying breeder eat within `den_radius_m`, as its pups do (pack.rs:1375-1386).
#[test]
fn a_hungry_breeder_carrying_a_litter_eats_a_carcass_lying_at_its_den() {
    let world = world(941);
    let den = Site { x: 24, y: SURFACE_Y, z: 6 };
    let run = |carrying: bool| {
        let mut flora = Flora::new(FloraConfig::default());
        let (mut fauna, b) = den_scene(&world, den, true);
        let (cx, cz) = crate::pack::centre(den, VOXEL_M);
        place(&mut fauna, b, cx, cz, EAST);
        if carrying {
            animal_mut(&mut fauna, b).reproduction.escrow = Some(Escrow { young: 2, ..Escrow::default() });
        }
        let r = fauna.config.founder(CH).pack.unwrap().den_radius_m;
        carcass_at(&mut flora, cx + 0.75 * r, cz, 0.5);
        let cruise = fauna.config.manifest(CH).cruise_m_per_s;
        let bound = cubarium_voxel::ticks_at_least_one(3.0 * r / cruise + 10.0 * fauna.config.manifest(CH).controller_period_s) as u32;
        step_until(&mut fauna, &world, &mut flora, bound, |f| f.ledger.eaten_carrion_by_founder[CH.index()] > 0.0).is_some()
    };
    assert!(run(false), "control: a hungry breeder with no litter eats the carcass at its den");
    assert!(run(true), "a hungry breeder carrying a litter does not eat the carcass at its den");
}

/// **A breeder carrying a litter in an emergency is not held at the den** (PIT-2: "not in an
/// emergency"): the breeder, carrying a litter, at its den's edge (past half the radius) in an
/// emergency, a still grazer 3 m beyond it: within the time it takes to turn half round, walk
/// three den radii and ten controller periods it is still past half the radius. The control,
/// only hungry, goes home within half the radius in that time.
#[test]
fn a_breeder_carrying_a_litter_in_an_emergency_hunts() {
    let world = world(942);
    let den = Site { x: 24, y: SURFACE_Y, z: 6 };
    let run = |emergency: bool| {
        let mut flora = Flora::new(FloraConfig::default());
        let (mut fauna, b) = den_scene(&world, den, true);
        let (cx, cz) = crate::pack::centre(den, VOXEL_M);
        let r = fauna.config.founder(CH).pack.unwrap().den_radius_m;
        place(&mut fauna, b, cx - 0.9 * r, cz, WEST);
        animal_mut(&mut fauna, b).reproduction.escrow = Some(Escrow { young: 2, ..Escrow::default() });
        if emergency {
            // Hungrier than its lineage's `wake_hunger`: the emergency latch goes on.
            let a = animal(&fauna, b);
            let wake = fauna.config.founder(CH).wake_hunger;
            let sc = crate::body::effective_config(&fauna.config, &a);
            animal_mut(&mut fauna, b).reserve = 0.5 * (1.0 - wake) * sc.reserve_of(a.body);
        }
        let _g = grazer(&mut fauna, &world, cx - 0.9 * r - 3.0, cz, 0.3);
        let m = fauna.config.manifest(CH);
        let bound = cubarium_voxel::ticks_at_least_one(PI / m.yaw_cap_rad_per_s + 3.0 * r / m.cruise_m_per_s + 10.0 * m.controller_period_s);
        let mut events = Vec::new();
        for _ in 0..bound {
            step_s(&mut fauna, &world, &mut flora, 0.0, &mut events);
        }
        assert_eq!(animal(&fauna, b).emergency, emergency, "fixture: the latch");
        let a = animal(&fauna, b);
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, cx, cz, RING);
        ((dx * dx + dz * dz).sqrt(), r)
    };
    let (d, r) = run(false);
    assert!(d <= 0.5 * r + 0.1, "control, hungry: carrying a litter it goes home ({d:.2} m from the den)");
    let (d, r) = run(true);
    assert!(d > 0.5 * r, "in an emergency it is not held at the den ({d:.2} m from the den)");
}

// ------------------------------------------------------------------ CHURN: work is progress

/// **Work on the target is progress**: a pack at a hunkered adult grazer it can open (two
/// members' pry against its resistance), every member hungry and in contact, with no closing
/// counted (`chase_progress_m` past the plain) and a give-up of 1 s, keeps its target over three
/// give-ups while it pries. The control, the same pack at the same grazer unhunkered (no pry, and
/// strikes that do no harm: no work), goes late.
#[test]
fn work_on_the_target_is_progress() {
    let arm = |hunkered: bool| -> (bool, usize) {
        let world = world(951);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config();
        let base = crate::vigil::hunker_of(PREY);
        c.founders[PREY.index()].hunker = crate::Hunker { enabled: true, ring_resist_per_kin: 0.0, pry_per_member_s: base.resist_per_s / 1.5, ..base };
        c.founders[CH.index()].pack.as_mut().unwrap().close_in_m = 2.5;
        let hk = c.founders[PREY.index()].hunker;
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let members = pack(&mut fauna, &world);
        let g = grazer(&mut fauna, &world, 6.0, LANE_Z, 1.0);
        let adult = fauna.config.founder(PREY).core.birth_body;
        assert_eq!(hk.members_to_open(adult, adult, 0), Some(2), "fixture: two members open it");
        animal_mut(&mut fauna, g).hunkered = hunkered;
        step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("fixture: the pack commits");
        step_until(&mut fauna, &world, &mut flora, 400, |f| nearest(f, &members, g) <= 1.5).expect("fixture: the pack reaches the grazer");
        {
            let pk = c.founders[CH.index()].pack.as_mut().unwrap();
            pk.chase_give_up_s = 1.0;
            pk.route_reach_fraction = 1.0;
            pk.chase_progress_m = 100.0;
        }
        fauna.replace_config(c).unwrap();
        let mut events = Vec::new();
        let mut kept = true;
        for _ in 0..cubarium_voxel::ticks_at_least_one(3.0) {
            step_s(&mut fauna, &world, &mut flora, 0.0, &mut events);
            kept &= fauna.packs()[0].target == Some(g) || fauna.view().animal(g).is_none();
        }
        (kept, ends(&events, ChaseEnd::Late))
    };
    let (kept, late) = arm(false);
    assert!(!kept && late >= 1, "control, no work: kept {kept}, late {late}");
    let (kept, late) = arm(true);
    assert!(kept && late == 0, "prying: kept {kept}, late {late}");
}

// ------------------------------------------------------------------ STRIKE: the switch off

/// **`close_in_m` 0 restores the chase**: a member with its grazer inside its outline (0.15 m
/// ahead), alone and hungry: with the close-in off nothing is booked to it (no close-in
/// samples, step-offs or pivots) and the member stays within the chase's stop distance of the
/// grazer (half its length); with it on, its close-in samples and step-offs are booked.
#[test]
fn close_in_m_zero_restores_the_chase() {
    let run = |close_in_m: f64| {
        let world = world(952);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config();
        c.founders[CH.index()].pack.as_mut().unwrap().close_in_m = close_in_m;
        let (mut fauna, a, g) = one_member(&world, &mut flora, c);
        place(&mut fauna, a, 5.0, LANE_Z, EAST);
        place(&mut fauna, g, 5.15, LANE_Z, WEST);
        let before = (fauna.ledger.pack_close_in_samples, fauna.ledger.pack_step_offs, fauna.ledger.pack_pivots);
        let stop = 0.5 * fauna.config.founder(CH).body_at(animal(&fauna, a).body).length_m;
        let mut farthest: f64 = 0.0;
        for _ in 0..cubarium_voxel::ticks_at_least_one(1.0) {
            fauna.step_with(&world, &mut flora, 1);
            if fauna.view().animal(g).is_none() {
                break;
            }
            farthest = farthest.max(distance(&fauna, a, g));
        }
        let after = (fauna.ledger.pack_close_in_samples, fauna.ledger.pack_step_offs, fauna.ledger.pack_pivots);
        (after.0 - before.0, after.1 - before.1, after.2 - before.2, farthest, stop)
    };
    let (samples, offs, pivots, farthest, stop) = run(0.0);
    assert_eq!((samples, offs, pivots), (0, 0, 0), "close-in off: nothing booked to it");
    assert!(farthest <= stop, "close-in off: it stays within the chase's stop ({farthest:.2} > {stop:.2})");
    let (samples, offs, _, _, _) = run(2.5);
    assert!(samples > 0 && offs > 0, "close-in on: samples {samples}, step-offs {offs}");
}

// ------------------------------------------------------------------ save and load

/// **Every state field and knob the night's packages added round-trips a snapshot** (fauna
/// schema 58): CHURN's progress clock on a pack (a finite nearest-so-far and an infinite one),
/// LIFE-HISTORY's season on a pack and on a body, the new ledger lines, and the knobs of CHURN,
/// STRIKE, STRIKE-2, LIFE-HISTORY, PIT, PIT-2, BELLWING and EDGE set off their defaults; EDGE's
/// session state (the pressed run, the walk and its give-ups, the route flight) is not saved
/// and loads empty.
#[test]
fn every_new_state_field_and_knob_round_trips_a_snapshot() {
    let world = world(961);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    {
        let pk = c.founders[CH.index()].pack.as_mut().unwrap();
        pk.chase_progress_m = 0.35;
        pk.switch_margin = 1.7;
        pk.switch_stall_s = 7.0;
        pk.close_in_m = 2.2;
        pk.stand_off_reach_fraction = 0.3;
        pk.herd_shun_m = 2.5;
        pk.breed_reserve_fraction = 0.55;
        pk.litter_by_condition = !pk.litter_by_condition;
        pk.pit_search_m = 3.5;
        pk.route_swing_voxels = 0.4;
        pk.whelp_at_den = !pk.whelp_at_den;
        let r = &mut c.founders[CH.index()].core.reproduction;
        r.litters_per_year = 2;
        r.breeding_phase = 0.35;
        r.breeding_window = 0.2;
        r.gestation_upkeep_factor = 1.3;
    }
    c.light.lure_nir = 0.6;
    c.light.lure_scent = 0.4;
    c.teachers.press_replan_s = 2.5;
    c.teachers.press_shun_s = 45.0;
    c.teachers.route_refresh_s = 30.0;
    c.teachers.route_detour = 1.5;
    c.teachers.way_on_m = 3.0;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world);
    let g = grazer(&mut fauna, &world, 6.0, LANE_Z, 0.5);
    step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("fixture: the pack commits");
    let own = fauna.packs()[0].id;
    {
        let p = fauna.pack_mut(own).unwrap();
        p.chase_progress_tick = 123;
        p.chase_best_m = 2.75;
        p.chase_work = 0.4;
        p.season = crate::Season { year: 3, litters: 1 };
    }
    animal_mut(&mut fauna, g).reproduction.season = crate::Season { year: -1, litters: 2 };
    let l = &mut fauna.ledger;
    l.pups_independent = 3;
    l.season_waits = 4;
    l.pack_switches = 5;
    l.pack_close_in_samples = 6;
    l.pack_step_offs = 7;
    l.pack_pivots = 8;
    l.pack_out_of_reach = 9;
    // EDGE's session state.
    let m = members[0];
    fauna.pressed.insert(m, 40);
    fauna.walk_targets.insert(m, (77, 10, 20));
    fauna.walk_shun.insert(m, vec![(77, 9999)]);
    fauna.route_flee.insert(m, 5);
    let back = crate::snapshot::decode(&crate::snapshot::encode(&fauna)).expect("a live layer's own bytes load");
    assert_eq!(back.config, fauna.config, "the knobs");
    assert_eq!(back.packs(), fauna.packs(), "the pack's progress clock and season");
    assert_eq!(back.animals, fauna.animals, "the body's season");
    assert_eq!(back.ledger, fauna.ledger, "the new ledger lines");
    assert_eq!(back.press_of(m), (0, None, Vec::new()), "EDGE's session state loads empty");
    assert!(back.route_flee.is_empty());
    // An unbounded nearest-so-far (a commit on a target gone) loads as it was.
    fauna.pack_mut(own).unwrap().chase_best_m = f64::INFINITY;
    let back = crate::snapshot::decode(&crate::snapshot::encode(&fauna)).expect("an infinite nearest-so-far loads");
    assert_eq!(back.packs()[0].chase_best_m, f64::INFINITY);
}

// ------------------------------------------------------------------ STRIKE-2 x the draw

/// **A pack's draw does not lead it to a prey it shuns** (STRIKE-2's herd shun and PACK-TUNE's
/// shun against VEILHOUND-SEARCH's draw): a hungry pack with no target whose only sensed prey is
/// a grazer it shuns (as a herd-mate left with a dropped armoured target), 6 m off: its leader's
/// sample takes no target and records no draw to that grazer. The control, the same grazer not
/// shunned, is drawn to or taken.
///
/// FINDING (evidence): shunned, the grazer is not taken (the choice skips it, pack.rs:1131) but
/// `drawn == Some(grazer)`; draw_step filters only the abandoned target (pack.rs:927). After
/// STRIKE-2's leave_herd the draw leads the pack back to the herd it left. One-line candidate
/// `|| shuns(fauna, own, k.id)` at pack.rs:927, not applied (whether a shunned herd still draws
/// is a design call).
#[test]
fn a_packs_draw_does_not_lead_it_to_a_prey_it_shuns() {
    let run = |shunned: bool| {
        let world = world(971);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config();
        // The choice reaches 2 m: the grazer is beyond it, so the draw is what would move the pack.
        {
            let cruise = c.manifest(CH).cruise_m_per_s;
            let pk = c.founders[CH.index()].pack.as_mut().unwrap();
            pk.route_reach_fraction = 2.0 / (cruise * pk.chase_give_up_s);
        }
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let members = pack(&mut fauna, &world);
        let g = grazer(&mut fauna, &world, 8.5, LANE_Z, 0.5);
        let own = fauna.packs()[0].id;
        if shunned {
            let until = fauna.tick + 100_000;
            fauna.pack_mut(own).unwrap().shunned[0] = (g, until);
        }
        let rec = fauna.packs()[0];
        let leader = rec.breeder.filter(|b| members.contains(b)).unwrap_or(members[0]);
        let mut events = Vec::new();
        sample(&mut fauna, &world, &flora, leader, &mut events);
        let _ = &mut flora;
        let p = fauna.packs()[0];
        (p.target == Some(g), p.drawn == Some(g))
    };
    let (taken, drawn) = run(false);
    assert!(taken || drawn, "control: the unshunned grazer is drawn to or taken (taken {taken}, drawn {drawn})");
    let (taken, drawn) = run(true);
    assert!(!taken, "a shunned grazer is taken");
    assert!(!drawn, "the pack is drawn to a grazer it shuns");
}
