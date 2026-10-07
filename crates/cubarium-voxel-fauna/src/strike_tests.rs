//! **STRIKE**: a Veilhound next to its prey bites it. Close in (within `Pack::close_in_m`
//! of its target, on the target's level), a pack member leaves its route and its flank point
//! and steers at its **stand-off**: the jaw's reach plus `Pack::stand_off_reach_fraction` of
//! the strike's reach from the prey, facing it. A prey to its side or behind it is turned to
//! in place; one inside its outline is stepped off from; it never presses into the prey. A
//! child of `step` sharing the predation fixture.
//!
//! Every case sets the config it relies on, and its time bound is the config's own: two half
//! turns at the yaw cap, a walk of twice the stand-off at cruise, three controller periods and
//! the windup. No shipped default, ratio or verdict is asserted.

use super::predation_fixture::*;
use crate::{Activity, ChaseEnd, Command, Fauna, FaunaConfig, Founder, HuntConfig, PackEvent, StartingStores, VigilMode};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use std::f64::consts::PI;

const CH: Founder = Founder::Chorister;
const PREY: Founder = Founder::Browser;
const RING: f64 = 48.0 * VOXEL_M;

fn world(seed: u64) -> World {
    plain(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, seed)
}

/// Every lineage awake, the patrol on with no exploring; the prey with no dodge, no hunker and
/// no ground response; the Veilhound's sight all round (still bodies too), remote senses every
/// tick; no stalk; a strike that does no harm (`strike_power` 0, so the prey stays to be struck
/// again) and a chase that is never given up; the close-in at `close_in_m`, the stand-off at
/// the jaw.
fn config(close_in_m: f64) -> FaunaConfig {
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
    c.founders[CH.index()].hunter.as_mut().expect("the chorister hunts").strike_power = 0.0;
    let cruise = c.manifest(CH).cruise_m_per_s;
    let pk = c.founders[CH.index()].pack.as_mut().expect("the chorister's pack table");
    pk.select_min_prospect = 0.0;
    pk.pack_radius_m = 6.0;
    pk.commit_m = 1e9; // no stalk
    pk.chase_give_up_s = 1e6;
    pk.switch_stall_s = 1e6;
    pk.route_reach_fraction = 8.0 / (cruise * pk.chase_give_up_s);
    pk.close_in_m = close_in_m;
    pk.stand_off_reach_fraction = 0.0;
    c
}

/// The scene: a hungry pack commits to a still grazer of half the adult body; then only
/// `hunters` stay hungry (the rest are fed: they neither take part nor strike), the grazer is
/// put at `prey`, and each hunter `k` at `at[k]` with its heading. Returns the world, the plant
/// layer, the fauna, the hunters' ids and the grazer's.
#[allow(clippy::type_complexity)]
fn scene(seed: u64, c: FaunaConfig, prey: (f64, f64), at: &[(f64, f64, f64)]) -> (World, Flora, Fauna, Vec<u64>, u64) {
    let world = world(seed);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let den = Site { x: 10, y: SURFACE_Y, z: 6 };
    assert!(fauna.apply(&world, Command::FoundPack { den, founder: CH, stores: HUNTING, heading_rad: EAST }));
    let teachers = fauna.config.teachers;
    let mut members: Vec<u64> = fauna.animals.iter().filter(|a| a.founder == Some(CH)).map(|a| a.id).collect();
    members.sort_unstable();
    for (k, &id) in members.iter().enumerate() {
        fauna.controllers.set(id, crate::controller::heuristic(CH, teachers));
        place(&mut fauna, id, 2.0 + 0.5 * (k / 2) as f64, 0.75 + 1.5 * (k % 2) as f64, EAST);
        animal_mut(&mut fauna, id).eating = true;
    }
    let g = introduce(&mut fauna, &world, PREY, StartingStores::FULL);
    place(&mut fauna, g, 6.0, LANE_Z, WEST);
    let birth = fauna.config.founder(PREY).core.birth_body;
    animal_mut(&mut fauna, g).body = 0.5 * birth;
    drive(&mut fauna, g, 0.0, false);
    step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
    assert_eq!(fauna.packs()[0].target, Some(g), "the only prey is the target");
    // Far from the scene, the fed members.
    for (k, &id) in members.iter().enumerate().skip(at.len()) {
        place(&mut fauna, id, 0.5 + 0.5 * k as f64, 0.5, EAST);
        animal_mut(&mut fauna, id).eating = false;
    }
    place(&mut fauna, g, prey.0, prey.1, WEST);
    for (k, &(x, z, heading)) in at.iter().enumerate() {
        let id = members[k];
        place(&mut fauna, id, x, z, heading);
        let a = animal_mut(&mut fauna, id);
        a.eating = true;
        a.strike = None;
        // Its next sampling decides: no walk held over from before the move.
        a.founder_state.held = crate::controller::Actions::REST;
    }
    let hunters = members[..at.len()].to_vec();
    (world, flora, fauna, hunters, g)
}

/// The case's time bound from the config: two half turns at the yaw cap, a walk of twice the
/// stand-off at cruise, three controller periods and the windup.
fn bound_ticks(fauna: &Fauna, id: u64) -> u32 {
    let m = fauna.config.manifest(CH);
    let phys = fauna.config.founder(CH);
    let h = phys.hunter.unwrap();
    let pk = phys.pack.unwrap();
    let length = phys.body_at(animal(fauna, id).body).length_m;
    let stand = (h.jaw_forward_length_fraction + pk.stand_off_reach_fraction * h.strike_reach_length_fraction) * length;
    let s = 2.0 * PI / m.yaw_cap_rad_per_s + 2.0 * stand / m.cruise_m_per_s + 3.0 * m.controller_period_s + h.windup_s;
    cubarium_voxel::ticks_at_least_one(s) as u32
}

fn distance(fauna: &Fauna, a: u64, b: u64) -> f64 {
    let (p, q) = (animal(fauna, a).pose, animal(fauna, b).pose);
    let (dx, dz) = crate::pose::wrapped_delta(p.x, p.z, q.x, q.z, RING);
    (dx * dx + dz * dz).sqrt()
}

fn striking(fauna: &Fauna, id: u64, prey: u64) -> bool {
    animal(fauna, id).strike.is_some_and(|s| s.prey == prey)
}

/// **A member with the prey beside it turns and strikes within a bounded time**: the grazer a
/// metre to its left (90° off its heading), the only hungry member strikes it within the
/// config's bound.
#[test]
fn a_member_with_the_prey_beside_it_turns_and_strikes() {
    let (world, mut flora, mut fauna, hunters, g) = scene(401, config(2.5), (5.0, 2.5), &[(5.0, 1.5, EAST)]);
    let a = hunters[0];
    let bound = bound_ticks(&fauna, a);
    let t = step_until(&mut fauna, &world, &mut flora, bound, |f| striking(f, a, g));
    assert!(t.is_some(), "no strike within {bound} ticks; at {:.2} m", distance(&fauna, a, g));
}

/// **A member with the prey inside its outline steps off or turns, and strikes**: the grazer
/// 0.15 m ahead of its centre (behind its jaw, nearer than any capsule facing it reaches), and
/// in a second case 0.2 m to its side; each is struck within the config's bound, and the
/// member never walks farther from the grazer than its close-in distance meanwhile.
#[test]
fn a_member_with_the_prey_inside_its_outline_steps_off_and_strikes() {
    for (seed, prey, at) in [(402, (5.15, LANE_Z), (5.0, LANE_Z, EAST)), (403, (5.0, 1.7), (5.0, 1.5, EAST))] {
        let close_in = 2.5;
        let (world, mut flora, mut fauna, hunters, g) = scene(seed, config(close_in), prey, &[at]);
        let a = hunters[0];
        let bound = bound_ticks(&fauna, a);
        let mut farthest: f64 = 0.0;
        let t = step_until(&mut fauna, &world, &mut flora, bound, |f| {
            farthest = farthest.max(distance(f, a, g));
            striking(f, a, g)
        });
        assert!(t.is_some(), "seed {seed}: no strike within {bound} ticks; at {:.2} m", distance(&fauna, a, g));
        assert!(farthest <= close_in, "seed {seed}: it stepped off to {farthest:.2} m, past the close-in {close_in} m");
    }
}

/// **A member within the close-in distance does not steer toward a point away from the
/// prey**: three hungry members; a packmate nearer the grazer drives, and the member under
/// test is a flanker 1.3 m off the grazer whose flank point (`flank_offset_m` 2 m across the
/// driver's line, on its side by rank) lies 0.7 m farther from the grazer than it stands. It
/// closes on the grazer instead — never more than a tick's walk farther than it began — and
/// strikes within the bound.
#[test]
fn a_member_within_the_close_in_does_not_steer_toward_a_point_away_from_the_prey() {
    let mut c = config(2.5);
    c.founders[CH.index()].pack.as_mut().unwrap().flank_offset_m = 2.0;
    // Members by id: a far flanker, the driver (nearest), the flanker under test.
    let (world, mut flora, mut fauna, hunters, g) =
        scene(404, c, (6.0, 2.2), &[(8.5, 1.0, WEST), (7.0, 2.2, WEST), (6.0, 0.9, EAST)]);
    let a = hunters[2];
    let bound = bound_ticks(&fauna, a);
    let slack = fauna.config.manifest(CH).cruise_m_per_s * crate::DT;
    let d0 = distance(&fauna, a, g);
    let mut farthest: f64 = d0;
    let t = step_until(&mut fauna, &world, &mut flora, bound, |f| {
        farthest = farthest.max(distance(f, a, g));
        striking(f, a, g)
    });
    assert!(farthest <= d0 + slack, "it moved away from the grazer: {d0:.2} -> {farthest:.2} m");
    assert!(t.is_some(), "no strike within {bound} ticks; at {:.2} m", distance(&fauna, a, g));
}

// ------------------------------------------------------------------ STRIKE-2

/// The plain with a ledge `rise` voxels high over every column from `x0` (voxels) to the
/// ring's end: soil laid on the plain's surface.
fn ledged(seed: u64, x0: i64, rise: u32) -> World {
    let mut w = plain_of_height(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, seed, 8);
    let (width, depth) = (i64::from(w.config().width), w.config().depth);
    for z in 0..depth {
        for x in x0..width {
            for y in SURFACE_Y + 1..=SURFACE_Y + rise {
                w.apply(cubarium_voxel::Command::SetMaterial { x, y, z, material: cubarium_voxel::Material::Soil });
            }
        }
    }
    w
}

/// **STRIKE-2: a member below its prey's level climbs to a face it can strike it from**: the
/// grazer stands at a ledge's edge two voxels up (above the member's jaw band, within its
/// step), and the only hungry member stands below the edge, nearer the grazer than the chase's
/// stop distance (half its length). It strikes the grazer within the config's bound plus a walk
/// across the close-in distance, having climbed onto the grazer's level within that walk and
/// three controller periods. (STRIKE's close-in left such a member to the chase, whose stop
/// distance held it below the ledge, its jaw short of the prey's body.)
#[test]
fn a_member_below_its_prey_climbs_to_a_face_it_strikes_from() {
    let close_in = 2.5;
    let world = ledged(411, 32, 2);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(close_in));
    fauna.set_births_enabled(false);
    let den = Site { x: 10, y: SURFACE_Y, z: 6 };
    assert!(fauna.apply(&world, Command::FoundPack { den, founder: CH, stores: HUNTING, heading_rad: EAST }));
    let teachers = fauna.config.teachers;
    let mut members: Vec<u64> = fauna.animals.iter().filter(|a| a.founder == Some(CH)).map(|a| a.id).collect();
    members.sort_unstable();
    for (k, &id) in members.iter().enumerate() {
        fauna.controllers.set(id, crate::controller::heuristic(CH, teachers));
        place(&mut fauna, id, 2.0 + 0.5 * (k / 2) as f64, 0.75 + 1.5 * (k % 2) as f64, EAST);
        animal_mut(&mut fauna, id).eating = true;
    }
    let g = introduce(&mut fauna, &world, PREY, StartingStores::FULL);
    place(&mut fauna, g, 6.0, LANE_Z, WEST);
    let birth = fauna.config.founder(PREY).core.birth_body;
    animal_mut(&mut fauna, g).body = 0.5 * birth;
    drive(&mut fauna, g, 0.0, false);
    step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
    assert_eq!(fauna.packs()[0].target, Some(g), "the only prey is the target");
    for (k, &id) in members.iter().enumerate().skip(1) {
        place(&mut fauna, id, 0.5 + 0.5 * k as f64, 0.5, EAST);
        animal_mut(&mut fauna, id).eating = false;
    }
    // The grazer on the ledge (its face two voxels up) by its edge, the member below the edge
    // within the chase's stop distance of it.
    let a = members[0];
    let stop = 0.5 * fauna.config.founder(CH).body_at(animal(&fauna, a).body).length_m;
    let (gx, ax) = (8.05, 8.05 - 0.6 * stop);
    assert!(ax < 8.0 - 0.05, "the member stands below the edge");
    place(&mut fauna, g, gx, LANE_Z, WEST);
    animal_mut(&mut fauna, g).site.y = SURFACE_Y + 2;
    place(&mut fauna, a, ax, LANE_Z, EAST);
    {
        let m = animal_mut(&mut fauna, a);
        m.eating = true;
        m.strike = None;
        m.founder_state.held = crate::controller::Actions::REST;
    }
    // Out of its jaw band from where it stands: the case is the one the rule is for.
    let phys = *fauna.config.founder(CH);
    let (h, geo) = (phys.hunter.unwrap(), phys.body_at(animal(&fauna, a).body));
    let rise = 2.0 * VOXEL_M;
    assert!(rise >= h.jaw_band_height_fraction * geo.height_m, "the ledge lifts the grazer out of the jaw band");
    assert!(rise <= phys.step_up_m, "the ledge is within the member's step");
    let m = fauna.config.manifest(CH);
    let bound = bound_ticks(&fauna, a) + cubarium_voxel::ticks_at_least_one(close_in / m.cruise_m_per_s) as u32;
    // It goes up within a walk across the close-in distance and three controller periods...
    let climb = cubarium_voxel::ticks_at_least_one(close_in / m.cruise_m_per_s + 3.0 * m.controller_period_s) as u32;
    let up = step_until(&mut fauna, &world, &mut flora, climb, |f| animal(f, a).site.y == animal(f, g).site.y);
    assert!(up.is_some(), "still below the ledge after {climb} ticks, at {:.2} m", distance(&fauna, a, g));
    // ...and strikes within the bound.
    let t = step_until(&mut fauna, &world, &mut flora, bound, |f| striking(f, a, g));
    let at = animal(&fauna, a);
    assert!(t.is_some(), "no strike within {bound} ticks; at {:.2} m, on layer {} (the grazer's {})", distance(&fauna, a, g), at.site.y, animal(&fauna, g).site.y);
}

/// The plain with a pen: walls `SURFACE_Y + 1 ..= SURFACE_Y + 4` round the columns x 19..=22,
/// z 0..=4 (the strip's own end is its fourth side).
fn penned(seed: u64) -> World {
    let mut w = world(seed);
    let wall = |w: &mut World, x: i64, z: u32| {
        for y in SURFACE_Y + 1..=SURFACE_Y + 4 {
            w.apply(cubarium_voxel::Command::SetMaterial { x, y, z, material: cubarium_voxel::Material::Soil });
        }
    };
    for z in 0..=5u32 {
        wall(&mut w, 18, z);
        wall(&mut w, 23, z);
    }
    for x in 18..=23i64 {
        wall(&mut w, x, 5);
    }
    w
}

/// **STRIKE-2: a pack that gives up an armoured grazer of a herd leaves the herd**: two adult
/// grazers hunkered side by side (each the other's ring), a pack of four hungry members, two of
/// them penned (counted at hand, never in contact), so the two that reach the target pry it
/// short of its resistance and the chase goes late. With `herd_shun_m` set, the pack shuns the
/// herd-mate it could not open alone with its target and takes neither within the shun; with it
/// 0 (the control) it takes the herd-mate next.
#[test]
fn a_pack_that_gives_up_an_armoured_grazer_leaves_its_herd() {
    let arm = |herd_shun_m: f64| -> (bool, bool) {
        let world = penned(412);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config(2.5);
        c.founders[PREY.index()].hunker = crate::Hunker { enabled: true, ..crate::vigil::hunker_of(PREY) };
        let cruise = c.manifest(CH).cruise_m_per_s;
        let pk = c.founders[CH.index()].pack.as_mut().unwrap();
        pk.chase_give_up_s = 6.0;
        pk.route_reach_fraction = (8.0 / (cruise * pk.chase_give_up_s)).min(1.0);
        pk.shun_s = 60.0;
        pk.herd_shun_m = herd_shun_m;
        pk.select_kin_factor = 1.0;
        pk.select_aware_factor = 1.0;
        let hk = c.founders[PREY.index()].hunker;
        let shun_s = pk.shun_s;
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let den = Site { x: 20, y: SURFACE_Y, z: 6 };
        assert!(fauna.apply(&world, Command::FoundPack { den, founder: CH, stores: HUNTING, heading_rad: EAST }));
        let teachers = fauna.config.teachers;
        let mut members: Vec<u64> = fauna.animals.iter().filter(|a| a.founder == Some(CH)).map(|a| a.id).collect();
        members.sort_unstable();
        assert!(members.len() >= 4, "a pack of four");
        for (k, &id) in members.iter().enumerate() {
            fauna.controllers.set(id, crate::controller::heuristic(CH, teachers));
            let (x, z) = [(5.0, 0.5), (5.5, 0.6), (3.0, 2.0), (3.0, 2.6)][k.min(3)];
            place(&mut fauna, id, x, z, EAST);
            animal_mut(&mut fauna, id).eating = true;
        }
        let birth = fauna.config.founder(PREY).core.birth_body;
        let herd: Vec<u64> = [7.5, 8.5]
            .into_iter()
            .map(|x| {
                let id = introduce(&mut fauna, &world, PREY, StartingStores::FULL);
                place(&mut fauna, id, x, LANE_Z, WEST);
                animal_mut(&mut fauna, id).body = birth;
                animal_mut(&mut fauna, id).hunkered = true;
                drive(&mut fauna, id, 0.0, false);
                id
            })
            .collect();
        // Each needs every member of the pack with its ring (so the pack commits), and more
        // than one alone (so it is armour, not a soft juvenile).
        let need = hk.members_to_open(birth, birth, 1).expect("a pry opens it");
        assert!(need as usize <= members.len() && need > 2, "the herd is openable by the pack, not by the two free members ({need})");
        // The first chase, until it goes late; then, over half the shun, whether the herd-mate is
        // taken (a re-target in the same tick counts).
        let mut first: Option<u64> = None;
        let mut dropped: Option<(u64, u64)> = None;
        let mut took_b = false;
        let mut shunned = false;
        let limit = cubarium_voxel::ticks_at_least_one(60.0) + cubarium_voxel::ticks_at_least_one(0.5 * shun_s);
        for _ in 0..limit {
            fauna.step_with(&world, &mut flora, 1);
            for e in fauna.take_pack_events() {
                match e {
                    PackEvent::ChaseStart { target, .. } if first.is_none() => first = Some(target),
                    PackEvent::ChaseStart { target, .. } => {
                        took_b |= dropped.is_some_and(|(a, _)| herd.contains(&target) && target != a);
                    }
                    PackEvent::ChaseEnd { why: ChaseEnd::Late, target, .. } if dropped.is_none() && Some(target) == first => {
                        dropped = Some((target, fauna.tick));
                        let b = *herd.iter().find(|&&h| h != target).unwrap();
                        shunned = fauna.packs()[0].shunned.iter().any(|&(s, until)| s == b && fauna.tick < until);
                    }
                    _ => {}
                }
            }
            if dropped.is_some_and(|(_, t)| fauna.tick >= t + cubarium_voxel::ticks_at_least_one(0.5 * shun_s)) {
                break;
            }
        }
        assert!(dropped.is_some(), "the chase on the first grazer goes late");
        (shunned, took_b)
    };
    let (shunned, took) = arm(3.0);
    assert!(shunned && !took, "with the herd shun: the herd-mate shunned {shunned}, taken {took}");
    let (shunned, took) = arm(0.0);
    assert!(!shunned && took, "the control: the herd-mate shunned {shunned}, taken {took}");
}
