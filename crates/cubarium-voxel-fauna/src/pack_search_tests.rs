//! **VEILHOUND-SEARCH**, written before the rule (Wrysk, 2026-10-06: "veilhounds should have
//! good eyesight and be able to see pretty far. also good hearing and smell. like wolves or
//! lions"): the Veilhound (`chorister`) pack finds prey through its lineage's senses
//! ([`crate::SenseTable`]: sight, hearing and smell, each with its knobs) — and prey it senses beyond its chase reach on its
//! route component draws it there by route. A child of `step` beside the predation files,
//! sharing their fixture.
//!
//! Every case checks a mechanism on a hand-built world against the config it sets; no
//! default, ratio or population is asserted. The pack's chase reach is set short (1.5 m by
//! route) so a prey can stand beyond it and within a sense.
//!
//! - The senses: sight sees within its range inside its forward cone and not through a wall
//!   of terrain; hearing hears a moving body all around and not a still one; smell smells
//!   any body all around, through the wall.
//! - A hungry pack that smells a prey beyond its chase reach, reachable only by the route
//!   round a plateau, walks there by that route and starts a chase on it.
//! - A prey sensed on the plateau (off the pack's route component) draws nothing; the same
//!   prey on the floor does.
//! - With main's senses set by config (a 2 m cone ahead, no hearing, no smell) a far prey
//!   changes nothing the pack does; with longer senses it does.
//! - A sated pack neither is drawn nor chases; the same pack hungry does.

use super::predation_fixture::*;
use crate::{Activity, Command, Fauna, FaunaConfig, Founder, HuntConfig, StartingStores};
use cubarium_voxel::{Command as WorldCommand, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const CH: Founder = Founder::Chorister;
const PREY: Founder = Founder::Browser;
/// The plateau across the whole strip, `x ∈ [5, 8)` m.
const PLATEAU: (f64, f64) = (5.0, 8.0);
/// The ring: 48 columns at 0.25 m.
const RING: f64 = 48.0 * VOXEL_M;
/// The pack's chase reach by route in the cases, metres.
const REACH_M: f64 = 1.5;
/// The far prey's place on the floor: east of the plateau, reached from the pack only the
/// other way round the ring (about 5 m west of it).
const FAR_X: f64 = 10.0;

/// The senses a case sets: `(sight_m, sight_half_angle_deg, hearing_m, smell_m)`.
type Senses = (f64, f64, f64, f64);
/// Main's: its own 2 m cone ahead, no hearing, no smell.
const MAINS: Senses = (2.0, 90.0, 0.0, 0.0);

/// Every lineage awake, the patrol on with no exploring, no dodge for the prey, the pack's
/// chase reach `REACH_M` and the given senses.
fn config((sight, half, hearing, smell): Senses) -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[PREY.index()].agility = 0.0;
    c.hunt = HuntConfig { patrol: true, patrol_explore: 0.0, ..HuntConfig::default() };
    let cruise = c.manifest(CH).cruise_m_per_s;
    let pack = c.founders[CH.index()].pack.as_mut().expect("the chorister's pack table");
    pack.route_reach_fraction = REACH_M / (cruise * pack.chase_give_up_s);
    let senses = &mut c.founders[CH.index()].senses;
    senses.sight_m = sight;
    senses.sight_half_angle_deg = half;
    senses.hearing_m = hearing;
    senses.smell_m = smell;
    c
}

/// The plain with the plateau `rise` voxels over the floor; the plateau's top layer.
fn plateau_world(rise: u32, seed: u64) -> (World, u32) {
    let mut w = plain_of_height(
        cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() },
        seed,
        8 + rise,
    );
    for z in 0..12u32 {
        for x in (PLATEAU.0 / VOXEL_M) as i64..(PLATEAU.1 / VOXEL_M) as i64 {
            for y in SURFACE_Y + 1..=SURFACE_Y + rise {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    (w, SURFACE_Y + rise)
}

/// A plateau two voxels above the Veilhound's step up: off its route component.
fn ledge(fauna: &Fauna, seed: u64) -> (World, u32) {
    plateau_world(fauna.config.founder(CH).step_limits(VOXEL_M).up + 2, seed)
}

/// A Veilhound pack founded west of the plateau on its members' heuristics, placed in two
/// rows at `x ∈ [2, 3]` m facing `heading`, each with the eating latch `hungry`; the
/// members' ids, the breeder first.
fn pack(fauna: &mut Fauna, world: &World, stores: StartingStores, hungry: bool, heading: f64) -> Vec<u64> {
    let den = Site { x: 10, y: SURFACE_Y, z: 6 };
    assert!(fauna.apply(world, Command::FoundPack { den, founder: CH, stores, heading_rad: heading }));
    let teachers = fauna.config.teachers;
    let breeder = fauna.packs()[0].breeder;
    let mut members: Vec<u64> = fauna.animals.iter().filter(|a| a.founder == Some(CH)).map(|a| a.id).collect();
    members.sort_by_key(|&id| Some(id) != breeder);
    for (k, &id) in members.iter().enumerate() {
        fauna.controllers.set(id, crate::controller::heuristic(CH, teachers));
        place(fauna, id, 2.0 + 0.5 * (k / 2) as f64, 0.75 + 1.5 * (k % 2) as f64, heading);
        animal_mut(fauna, id).eating = hungry;
    }
    members
}

/// A still frondgrazer at `x_m` on the lane, standing on layer `y`.
fn prey_at(fauna: &mut Fauna, world: &World, x_m: f64, y: u32) -> u64 {
    let id = introduce(fauna, world, PREY, StartingStores::FULL);
    place(fauna, id, x_m, LANE_Z, WEST);
    animal_mut(fauna, id).site.y = y;
    drive(fauna, id, 0.0, false);
    id
}

fn wdx(x0: f64, x1: f64) -> f64 {
    crate::pose::wrapped_delta(x0, 0.0, x1, 0.0, RING).0
}

fn table(fauna: &Fauna) -> (crate::SenseTable, crate::Hunter) {
    let p = fauna.config.founder(CH);
    (p.senses, p.hunter.expect("hunter"))
}

/// Whether the pack senses `prey` now, and by which sense (0 sight, 1 hearing, 2 smell).
fn sensed(fauna: &mut Fauna, world: &World, prey: u64) -> Option<u8> {
    let (_, h) = table(fauna);
    let own = fauna.packs()[0].id;
    fauna.tick += 1; // a fresh reading
    let list = crate::pack::pack_sensed(fauna, &world.view(), own, &h);
    list.iter().find(|(id, _)| *id == prey).map(|(_, s)| *s)
}

/// Fill `id`'s stomach to its capacity with carrion, booked as introduced, the eating latch
/// off: a sated body (the latch goes on only at a low stomach).
fn fill_stomach(fauna: &mut Fauna, id: u64) {
    let k = fauna.config.founder(CH).gut.capacity(animal(fauna, id).body);
    let a = animal_mut(fauna, id);
    let old = a.stomach.total();
    let new = cubarium_voxel_flora::Taken { organic: k, mineral: 0.05 * k, energy: 2.0 * k };
    a.stomach = crate::Stomach::default();
    a.stomach.pools[crate::Food::Carrion.index()] = new;
    a.eating = false;
    let l = &mut fauna.ledger;
    l.introduced_organic_in += new.organic - old.organic;
    l.introduced_mineral_in += new.mineral - old.mineral;
    l.introduced_energy_in += new.energy - old.energy;
}

// ------------------------------------------------------------------ the senses

/// Sight sees a still prey within its range inside its forward cone; not one behind it, not
/// one past its range, and not one behind a wall of terrain taller than its eye. Hearing hears
/// a moving prey all around and not a still one; smell smells any prey all around, through
/// the wall.
#[test]
fn each_sense_reaches_what_it_should() {
    // A wall far taller than the Veilhound's eye on x ∈ [5, 8).
    let (world, _) = plateau_world(10, 31);
    let mut fauna = Fauna::new(config((6.0, 60.0, 0.0, 0.0)));
    let members = pack(&mut fauna, &world, HUNTING, true, EAST);
    // Pack at x ∈ [2, 2.5]. Ahead in the open (x = 4.5): seen; behind (x = 0.5): not.
    let p = prey_at(&mut fauna, &world, 4.5, SURFACE_Y);
    assert_eq!(sensed(&mut fauna, &world, p), Some(0), "in its cone and range, in the open: seen");
    place(&mut fauna, p, 0.5, LANE_Z, WEST);
    assert_eq!(sensed(&mut fauna, &world, p), None, "behind it: not seen");
    // By the wall's west face, the prey past its east face, within range: not seen.
    for (k, &id) in members.iter().enumerate() {
        place(&mut fauna, id, 4.2 + 0.1 * k as f64, LANE_Z, EAST);
    }
    place(&mut fauna, p, 8.5, LANE_Z, WEST);
    assert_eq!(sensed(&mut fauna, &world, p), None, "behind the wall: not seen");
    // The same open view with the range cut under the distance: not seen.
    let mut fauna = Fauna::new(config((1.0, 60.0, 0.0, 0.0)));
    pack(&mut fauna, &world, HUNTING, true, EAST);
    let p = prey_at(&mut fauna, &world, 4.5, SURFACE_Y);
    assert_eq!(sensed(&mut fauna, &world, p), None, "past its range: not seen");

    // Hearing all around, of a moving body; smell all around, through the wall.
    let mut fauna = Fauna::new(config((0.0, 0.0, 3.0, 0.0)));
    let (t, _) = table(&fauna);
    pack(&mut fauna, &world, HUNTING, true, EAST);
    let p = prey_at(&mut fauna, &world, 0.5, SURFACE_Y); // behind
    assert_eq!(sensed(&mut fauna, &world, p), None, "a still body is not heard");
    animal_mut(&mut fauna, p).speed_m_s = t.hearing_min_speed_m_s + 0.1;
    assert_eq!(sensed(&mut fauna, &world, p), Some(1), "a moving body behind it is heard");
    let mut fauna = Fauna::new(config((0.0, 0.0, 0.0, 5.0)));
    let members = pack(&mut fauna, &world, HUNTING, true, EAST);
    for (k, &id) in members.iter().enumerate() {
        place(&mut fauna, id, 4.2 + 0.1 * k as f64, LANE_Z, WEST); // facing away from the wall
    }
    let p = prey_at(&mut fauna, &world, 8.5, SURFACE_Y);
    assert_eq!(sensed(&mut fauna, &world, p), Some(2), "smelled behind it, through the wall");
}

// ------------------------------------------------------------------ the draw

/// A hungry pack smells a still frondgrazer on the floor beyond its chase reach, reachable
/// only by the route round the ring (the plateau blocks the straight way east): the leader is
/// drawn to it, walks west by the route, and the pack starts a chase on it.
#[test]
fn a_hungry_pack_walks_by_route_to_sensed_prey_beyond_its_reach_and_chases_it() {
    let mut fauna = Fauna::new(config((0.0, 0.0, 0.0, 7.0)));
    let (world, _) = ledge(&fauna, 21);
    let mut flora = Flora::new(FloraConfig::default());
    let members = pack(&mut fauna, &world, HUNTING, true, EAST);
    let prey = prey_at(&mut fauna, &world, FAR_X, SURFACE_Y);
    let leader = members[0];
    let x0 = animal(&fauna, leader).pose.x;
    let d0 = wdx(x0, FAR_X).abs();
    assert!(d0 > REACH_M, "the case starts with the prey out of reach: {d0}");
    let mut drawn = false;
    let cruise = fauna.config.manifest(CH).cruise_m_per_s;
    let max = (3.0 * d0 / cruise * f64::from(crate::TICK_HZ)).ceil() as u32 + 100;
    let started = step_until(&mut fauna, &world, &mut flora, max, |f| {
        drawn |= f.packs()[0].drawn == Some(prey);
        f.ledger.pack_chases > 0
    });
    assert!(drawn, "the pack was drawn to the far prey");
    assert!(started.is_some(), "the pack started a chase within {max} ticks");
    assert_eq!(fauna.packs()[0].target, Some(prey), "on that prey");
    assert!(fauna.ledger.pack_draw_chases > 0 && fauna.ledger.pack_draw_lead_samples > 0);
    let moved = wdx(x0, animal(&fauna, leader).pose.x);
    assert!(moved < 0.0, "the leader went west, round the ring, not into the plateau: {moved}");
    assert_closes(&fauna, "after the draw");
}

/// The pack's draws over `ticks` with one still frondgrazer at `x_m` on layer `y`, smelled.
fn draws_for_prey_at(x_m: f64, on_plateau: bool, ticks: u32) -> (u64, Option<u64>, Option<u8>) {
    let mut fauna = Fauna::new(config((0.0, 0.0, 0.0, 7.0)));
    let (world, top) = ledge(&fauna, 22);
    let mut flora = Flora::new(FloraConfig::default());
    pack(&mut fauna, &world, HUNTING, true, EAST);
    let y = if on_plateau { top } else { SURFACE_Y };
    let prey = prey_at(&mut fauna, &world, x_m, y);
    let sense = sensed(&mut fauna, &world, prey);
    fauna.tick -= 1;
    let mut drawn = None;
    for _ in 0..ticks {
        fauna.step_with(&world, &mut flora, 1);
        if let Some(p) = fauna.packs()[0].drawn {
            drawn = Some(p);
        }
    }
    assert!(fauna.view().animal(prey).is_some(), "the prey stands");
    (fauna.ledger.pack_draws_by_sense.iter().sum(), drawn, sense)
}

/// A frondgrazer on the plateau's top, smelled by the pack but off its route component, draws
/// nothing; the same body on the floor east of the plateau (on the component, round the ring)
/// does.
#[test]
fn prey_off_the_route_component_draws_nothing() {
    let mid = 0.5 * (PLATEAU.0 + PLATEAU.1);
    let (on_top, drawn_top, sense) = draws_for_prey_at(mid, true, 100);
    assert!(sense.is_some(), "the pack senses the prey on the plateau");
    assert_eq!((on_top, drawn_top), (0, None), "it draws nothing");
    let (on_floor, drawn_floor, _) = draws_for_prey_at(FAR_X, false, 100);
    assert!(on_floor > 0 && drawn_floor.is_some(), "the same prey on the floor draws it (the case can tell)");
}

/// Every member's pose each tick over `ticks`, with the far frondgrazer present or not.
fn trajectories(senses: Senses, with_prey: bool, ticks: u32) -> (Vec<(f64, f64, f64)>, Fauna) {
    let mut fauna = Fauna::new(config(senses));
    let (world, _) = ledge(&fauna, 23);
    let mut flora = Flora::new(FloraConfig::default());
    let members = pack(&mut fauna, &world, HUNTING, true, EAST);
    if with_prey {
        prey_at(&mut fauna, &world, FAR_X, SURFACE_Y);
    }
    let mut out = Vec::new();
    for _ in 0..ticks {
        fauna.step_with(&world, &mut flora, 1);
        for &id in &members {
            let p = animal(&fauna, id).pose;
            out.push((p.x, p.z, p.heading_rad));
        }
    }
    (out, fauna)
}

/// Main's senses set by config (its own 2 m cone ahead, no hearing, no smell): over a
/// stretch too short for any member to come within 2 m of the far prey, the pack moves
/// exactly as it does with no prey in the world and is never drawn. With a longer sense the
/// far prey changes what it does (the case can tell).
#[test]
fn mains_senses_by_config_leave_a_far_prey_unsensed() {
    let cruise = config(MAINS).manifest(CH).cruise_m_per_s;
    // The nearest member starts 5 m from the far prey (round the ring); it walks under 2 m.
    let ticks = (2.0 / cruise * f64::from(crate::TICK_HZ)).floor() as u32;
    let (with, fauna) = trajectories(MAINS, true, ticks);
    let (without, _) = trajectories(MAINS, false, ticks);
    assert_eq!(with, without, "with main's senses a far prey changes nothing");
    let l = &fauna.ledger;
    assert_eq!(fauna.packs()[0].drawn, None);
    assert_eq!((l.pack_draws_by_sense, l.pack_draw_lead_samples, l.pack_draw_follow_samples), ([0; 3], 0, 0));
    let long = (0.0, 0.0, 0.0, 7.0);
    let (with_long, _) = trajectories(long, true, ticks);
    let (without_long, _) = trajectories(long, false, ticks);
    assert_ne!(with_long, without_long, "with a longer sense the far prey leads the pack (the case can tell)");
}

/// A sated pack (full reserve, a full stomach, the eating latch off) that smells a prey both in and beyond its
/// reach is neither drawn nor chases; the same pack hungry is drawn.
#[test]
fn a_sated_pack_does_not_hunt() {
    let run = |stores: StartingStores, hungry: bool| {
        let mut fauna = Fauna::new(config((0.0, 0.0, 0.0, 7.0)));
        let (world, _) = ledge(&fauna, 24);
        let mut flora = Flora::new(FloraConfig::default());
        let members = pack(&mut fauna, &world, stores, hungry, EAST);
        if !hungry {
            for &id in &members {
                fill_stomach(&mut fauna, id);
            }
        }
        prey_at(&mut fauna, &world, FAR_X, SURFACE_Y);
        prey_at(&mut fauna, &world, 1.0, SURFACE_Y);
        for _ in 0..100 {
            fauna.step_with(&world, &mut flora, 1);
        }
        let still_sated = members.iter().all(|&id| !animal(&fauna, id).eating && !animal(&fauna, id).emergency);
        let l = &fauna.ledger;
        (l.pack_draws_by_sense.iter().sum::<u64>() + l.pack_chases, l.pack_draw_lead_samples, still_sated)
    };
    let (hunts, led, still_sated) = run(StartingStores::FULL, false);
    assert!(still_sated, "the case keeps the pack sated");
    assert_eq!((hunts, led), (0, 0), "a sated pack is neither drawn nor chases");
    let (hunts, _, _) = run(HUNTING, true);
    assert!(hunts > 0, "the same pack hungry hunts (the case can tell)");
}

/// Every lineage but the Veilhound carries exactly what it sensed before the senses table:
/// its manifest's cone range over the half-plane ahead, no hearing, no smell.
#[test]
fn every_other_lineage_keeps_its_cone_and_no_other_sense() {
    let c = FaunaConfig::default();
    for f in Founder::ALL.into_iter().filter(|&f| f != CH) {
        let s = c.founder(f).senses;
        assert_eq!(
            (s.sight_m, s.sight_half_angle_deg, s.hearing_m, s.smell_m),
            (f.manifest().cone_range_m, 90.0, 0.0, 0.0),
            "{}",
            f.name()
        );
    }
}
