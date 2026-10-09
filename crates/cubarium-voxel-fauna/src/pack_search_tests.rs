//! **VEILHOUND-SEARCH** and **SENSES-CORE** (Wrysk, 2026-10-06: "veilhounds should have
//! good eyesight and be able to see pretty far. also good hearing and smell. like wolves or
//! lions"): the Veilhound (`chorister`) pack finds prey through its lineage's senses
//! ([`crate::SenseTable`], read through its record, [`crate::perception`]) — and prey it
//! knows of beyond its chase reach on its route component draws it there by route. A child
//! of `step` beside the predation files, sharing their fixture.
//!
//! Every case checks a mechanism on a hand-built world against the config it sets; no
//! default, ratio or population is asserted. The pack's chase reach is set short (1.5 m by
//! route) so a prey can stand beyond it and within a sense.
//!
//! - The senses: sight sees within its range inside its field and not through a wall of
//!   terrain; hearing hears a moving body all around and not a still one.
//! - A hungry pack that sees a prey beyond its chase reach, reachable only by the route
//!   round a plateau, walks there by that route and starts a chase on it.
//! - A prey sensed on the plateau (off the pack's route component) draws nothing; the same
//!   prey on the floor does.
//! - With main's senses set by config (a 2 m cone ahead, no hearing) a far prey changes
//!   nothing the pack does; with longer senses it does.
//! - A sated pack neither is drawn nor chases; the same pack hungry does.
//! - SENSES-CORE: a packmate knows a sighting only within its hearing range of the member
//!   that sensed it; a perched bellwing within reach is takeable and one out of reach is
//!   not; a pursuit walks to the last-known point and gives the target up after the
//!   pursuit's memory; a hungry pack with no prey sensed walks up its prey's scent.

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

/// The senses a case sets: `(sight_m, sight_half_angle_deg, hearing_m, smell_min)`
/// (smell 0: none).
type Senses = (f64, f64, f64, f64);
/// Main's: its own 2 m cone ahead, no hearing, no smell.
const MAINS: Senses = (2.0, 90.0, 0.0, 0.0);
/// Sight all around to 7 m: the far prey round the ring is in view.
const AROUND: Senses = (7.0, 180.0, 0.0, 0.0);

/// Every lineage awake, the patrol on with no exploring, no dodge for the prey, the pack's
/// chase reach `REACH_M`, the given senses (no tremor; a moving body heard at any loudness;
/// a still body seen as far as a moving one) and remote senses every tick.
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
    c.founders[CH.index()].senses = crate::SenseTable {
        live: true,
        sight_m: sight,
        sight_night_m: sight,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: half,
        hearing_m: hearing,
        hearing_min: 1e-12,
        tremor_m: 0.0,
        tremor_min: 0.0,
        smell_min: smell,
        scent: Some(crate::ScentChannel::Hunter),
        ..crate::SenseTable::default()
    };
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
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

fn table(fauna: &Fauna) -> crate::SenseTable {
    fauna.config.founder(CH).senses
}

/// The index of body `id`.
fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("alive")
}

/// One remote sensing for every live body now.
fn sense_now(fauna: &mut Fauna, world: &World) {
    fauna.tick += 1;
    crate::perception::sense_pass(fauna, &world.view(), false, 1.0, 1);
}

/// What the pack's first member knows of `prey` now (its own sensing or a packmate's within
/// its hearing): the modality mask, `None` when it knows nothing of it.
fn sensed(fauna: &mut Fauna, world: &World, prey: u64) -> Option<u8> {
    sense_now(fauna, world);
    let members: Vec<usize> = fauna.animals.iter().enumerate().filter(|(_, a)| a.founder == Some(CH)).map(|(i, _)| i).collect();
    members.into_iter().find_map(|i| crate::pack::known(fauna, i, RING).into_iter().find(|t| t.id == prey).map(|t| t.modes))
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

/// Sight sees a still prey within its range inside its field; not one behind it, not one
/// past its range, and not one behind a wall of terrain taller than its eye. Hearing hears a
/// moving prey all around and not a still one.
#[test]
fn each_sense_reaches_what_it_should() {
    use crate::perception::{HEARING, SIGHT};
    // A wall far taller than the Veilhound's eye on x ∈ [5, 8).
    let (world, _) = plateau_world(10, 31);
    let mut fauna = Fauna::new(config((6.0, 60.0, 0.0, 0.0)));
    let members = pack(&mut fauna, &world, HUNTING, true, EAST);
    // Pack at x ∈ [2, 2.5]. Ahead in the open (x = 4.5): seen; behind (x = 0.5): not.
    let p = prey_at(&mut fauna, &world, 4.5, SURFACE_Y);
    assert_eq!(sensed(&mut fauna, &world, p), Some(SIGHT), "in its field and range, in the open: seen");
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

    // Hearing all around, of a moving body.
    let mut fauna = Fauna::new(config((0.0, 0.0, 3.0, 0.0)));
    pack(&mut fauna, &world, HUNTING, true, EAST);
    let p = prey_at(&mut fauna, &world, 0.5, SURFACE_Y); // behind
    assert_eq!(sensed(&mut fauna, &world, p), None, "a still body is not heard");
    animal_mut(&mut fauna, p).speed_m_s = 0.5;
    assert_eq!(sensed(&mut fauna, &world, p).map(|m| m & HEARING), Some(HEARING), "a moving body behind it is heard");
    assert!(table(&fauna).hearing_m > 0.0);
}

// ------------------------------------------------------------------ the draw

/// A hungry pack sees a still frondgrazer on the floor beyond its chase reach, reachable
/// only by the route round the ring (the plateau blocks the straight way east): the leader is
/// drawn to it, walks west by the route, and the pack starts a chase on it.
#[test]
fn a_hungry_pack_walks_by_route_to_sensed_prey_beyond_its_reach_and_chases_it() {
    let mut fauna = Fauna::new(config(AROUND));
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

/// The pack's draws over `ticks` with one still frondgrazer at `x_m` on layer `y`, seen.
fn draws_for_prey_at(x_m: f64, on_plateau: bool, ticks: u32) -> (u64, Option<u64>, Option<u8>) {
    let mut fauna = Fauna::new(config(AROUND));
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

/// A frondgrazer on the plateau's top by its west edge, seen by the pack but off its route
/// component, draws nothing; the same body on the floor east of the plateau (on the
/// component, round the ring) does.
#[test]
fn prey_off_the_route_component_draws_nothing() {
    let edge = PLATEAU.0 + 0.2;
    let (on_top, drawn_top, sense) = draws_for_prey_at(edge, true, 100);
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
    // The nearest member starts 5 m from the far prey (round the ring); it walks 2.5 m, still
    // half a metre short of the 2 m cone (GAIT: at the faster cruise 2 m was too short a
    // stretch for the long sense to show).
    let ticks = (2.5 / cruise * f64::from(crate::TICK_HZ)).floor() as u32;
    let (with, fauna) = trajectories(MAINS, true, ticks);
    let (without, _) = trajectories(MAINS, false, ticks);
    assert_eq!(with, without, "with main's senses a far prey changes nothing");
    let l = &fauna.ledger;
    assert_eq!(fauna.packs()[0].drawn, None);
    assert_eq!((l.pack_draws_by_sense, l.pack_draw_lead_samples, l.pack_draw_follow_samples), ([0; 4], 0, 0));
    let long = AROUND;
    let (with_long, _) = trajectories(long, true, ticks);
    let (without_long, _) = trajectories(long, false, ticks);
    assert_ne!(with_long, without_long, "with a longer sense the far prey leads the pack (the case can tell)");
}

/// A sated pack (full reserve, a full stomach, the eating latch off) that sees a prey both
/// in and beyond its reach is neither drawn nor chases; the same pack hungry is drawn.
#[test]
fn a_sated_pack_does_not_hunt() {
    let run = |stores: StartingStores, hungry: bool| {
        let mut fauna = Fauna::new(config(AROUND));
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

// ------------------------------------------------------------------ SENSES-CORE

/// A packmate knows a sighting only within its hearing range of the member that sensed it:
/// a member facing away from a still prey knows of it while it stands within hearing of the
/// member that sees it, and not once it stands farther off.
#[test]
fn a_packmate_knows_a_sighting_only_within_hearing_range() {
    let world = plain(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, 41);
    let hearing = 2.0;
    let mut fauna = Fauna::new(config((3.0, 30.0, hearing, 0.0)));
    let members = pack(&mut fauna, &world, HUNTING, true, EAST);
    let (seer, deaf) = (members[0], members[1]);
    for &id in &members[2..] {
        place(&mut fauna, id, 9.0, LANE_Z, WEST); // the others far off, facing away
    }
    place(&mut fauna, seer, 2.0, LANE_Z, EAST);
    let prey = prey_at(&mut fauna, &world, 4.0, SURFACE_Y);
    let knows = |fauna: &mut Fauna, at_x: f64| {
        place(fauna, deaf, at_x, LANE_Z, WEST);
        sense_now(fauna, &world);
        let own = fauna.percept(deaf).is_some_and(|p| p.track(prey).is_some());
        assert!(!own, "it does not sense the prey itself");
        crate::pack::known(fauna, index(fauna, deaf), RING).iter().any(|t| t.id == prey)
    };
    assert!(knows(&mut fauna, 2.0 - 0.5 * hearing), "within hearing of the seer: shared");
    assert!(!knows(&mut fauna, 2.0 - 1.5 * hearing), "beyond it: not");
}

/// A perched bellwing within the Veilhound's reach (its base under the jaw band over the
/// support below) is takeable; the same bellwing perched above the band is sensed and not
/// takeable.
#[test]
fn a_perched_bellwing_in_reach_is_takeable_and_one_out_of_reach_is_not() {
    let world = plain_of_height(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, 42, 32);
    let mut fauna = Fauna::new(config((5.0, 180.0, 0.0, 0.0)));
    let members = pack(&mut fauna, &world, HUNTING, true, EAST);
    let me = members[0];
    let bw = introduce(&mut fauna, &world, crate::Founder::Bellwing, StartingStores::FULL);
    place(&mut fauna, bw, 4.0, LANE_Z, WEST);
    drive(&mut fauna, bw, 0.0, false);
    let band = {
        let h = fauna.config.founder(CH).hunter.expect("a hunter");
        let g = crate::body_of(&fauna.config, &animal(&fauna, me)).expect("a body");
        h.strike_height_m(&g)
    };
    let mut takeable = Vec::new();
    for lift in [0.5 * band, band + 1.0] {
        {
            let a = animal_mut(&mut fauna, bw);
            a.mobility.mode = crate::MobilityMode::Perched;
            a.pose.y_m = SURFACE_M + lift;
        }
        sense_now(&mut fauna, &world);
        let t = *fauna.percept(me).and_then(|p| p.track(bw)).expect("the bellwing is sensed");
        takeable.push(crate::pack::takeable(&fauna, index(&fauna, me), &t));
    }
    assert_eq!(takeable, vec![true, false], "in reach, then out of reach");
}

/// A pursuit walks to the target's last-known point and is given up after the pursuit's
/// memory: a pack takes a target, the prey is moved out of every member's senses, the pack
/// holds the target while the memory lasts (its members' focus stays on the last-known
/// point) and drops it after.
#[test]
fn a_pursuit_holds_the_last_known_point_and_gives_up() {
    let world = plain(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, 43);
    let mut c = config((4.0, 180.0, 0.0, 0.0));
    c.perception.give_up_s = 2.0;
    // A chase reach of 4 m by route, so the pack takes the prey before an ambush strikes it.
    let cruise = c.manifest(CH).cruise_m_per_s;
    let pk = c.founders[CH.index()].pack.as_mut().expect("the chorister's pack table");
    pk.route_reach_fraction = 4.0 / (cruise * pk.chase_give_up_s);
    let give_up = cubarium_voxel::ticks_at_least_one(c.perception.give_up_s) as u32;
    // The memory at a walk: a sprinting rush (GAIT) would carry the pack past the last-known
    // point and onto the moved prey within its sight.
    c.founders[CH.index()].sprint = crate::Sprint::default();
    let mut fauna = Fauna::new(c);
    let mut flora = Flora::new(FloraConfig::default());
    let members = pack(&mut fauna, &world, HUNTING, true, EAST);
    let prey = prey_at(&mut fauna, &world, 5.5, SURFACE_Y);
    step_until(&mut fauna, &world, &mut flora, 200, |f| f.packs()[0].target == Some(prey)).expect("the pack takes the prey");
    let last = animal(&fauna, prey).pose;
    place(&mut fauna, prey, 9.5, LANE_Z, WEST); // beyond every member's 4 m sight
    // A tick for the focus to settle on the last sensing, then the memory runs.
    let mut held = 0u32;
    while fauna.packs()[0].target == Some(prey) && held < 4 * give_up {
        fauna.step_with(&world, &mut flora, 1);
        held += 1;
        for &id in &members {
            if let Some(p) = fauna.percept(id).filter(|p| p.focus.id == prey && p.focus.known) {
                assert!((p.focus.x - last.x).abs() < 1.0, "the focus stays near the last-known point: {} vs {}", p.focus.x, last.x);
            }
        }
    }
    assert_eq!(fauna.packs()[0].target, None, "given up");
    assert!(held + 1 >= give_up, "held while the memory lasted: {held} ticks of {give_up}");
    assert!(fauna.ledger.pack_targets_lost > 0);
    assert!(fauna.view().animal(prey).is_some());
}

/// A hungry pack that senses no prey walks up the scent of its prey: with smell on, the
/// leader draws by scent (counted under smell) and closes on a still grazer beyond its
/// chase reach; with smell off it is never drawn by scent (the case can tell).
#[test]
fn a_hungry_pack_walks_up_its_preys_scent() {
    let run = |smell: f64| {
        let world = plain(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, 44);
        let mut fauna = Fauna::new(config((0.0, 0.0, 0.0, smell)));
        let mut flora = Flora::new(FloraConfig::default());
        let members = pack(&mut fauna, &world, HUNTING, true, EAST);
        let prey = prey_at(&mut fauna, &world, 5.5, SURFACE_Y);
        let d0 = members.iter().map(|&m| wdx(animal(&fauna, m).pose.x, 5.5).abs()).fold(f64::INFINITY, f64::min);
        for _ in 0..300 {
            fauna.step_with(&world, &mut flora, 1);
        }
        let d1 = members
            .iter()
            .filter_map(|&m| fauna.view().animal(m).map(|a| wdx(a.pose.x, 5.5).abs()))
            .fold(f64::INFINITY, f64::min);
        let _ = prey;
        (fauna.ledger.pack_draws_by_sense[3], d0, d1)
    };
    let (draws, d0, d1) = run(1e-9);
    assert!(draws > 0, "drawn by scent");
    assert!(d1 < d0 - 0.5, "the pack closed on the grazer: {d0} -> {d1}");
    let (draws, _, _) = run(0.0);
    assert_eq!(draws, 0, "no smell: no scent draw");
}

// ------------------------------------------------------------------ PACK-TUNE: prey choice

/// The case for the prey choice: sight all around to 7 m, a chase reach of 4 m by route, the
/// pack (four hungry adults about x = 2.25 m) and its world. `(fauna, world, members)`.
fn choice_case(seed: u64) -> (Fauna, World, Vec<u64>) {
    let world = plain(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, seed);
    let mut c = config(AROUND);
    let cruise = c.manifest(CH).cruise_m_per_s;
    let pk = c.founders[CH.index()].pack.as_mut().expect("the chorister's pack table");
    pk.route_reach_fraction = 4.0 / (cruise * pk.chase_give_up_s);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world, HUNTING, true, EAST);
    (fauna, world, members)
}

/// A grazer of `fraction` × the adult body at `(x, z)`, still; `hunkered` sets it under its
/// shield.
fn grazer(fauna: &mut Fauna, world: &World, x: f64, z: f64, fraction: f64, hunkered: bool) -> u64 {
    let id = introduce(fauna, world, PREY, StartingStores::FULL);
    place(fauna, id, x, z, WEST);
    let birth = fauna.config.founder(PREY).core.birth_body;
    animal_mut(fauna, id).body = fraction * birth;
    drive(fauna, id, 0.0, false);
    if hunkered {
        animal_mut(fauna, id).hunkered = true;
        fauna.percepts.entry(id).or_default().vigil.hunkered = true;
    }
    id
}

/// An adult grazer hunkered at `(x, LANE_Z)` with two hunkered kin beside it (the ring):
/// the adult's id.
fn ring_at(fauna: &mut Fauna, world: &World, x: f64) -> u64 {
    let adult = grazer(fauna, world, x, LANE_Z, 1.0, true);
    grazer(fauna, world, x + 0.4, LANE_Z - 0.8, 1.0, true);
    grazer(fauna, world, x + 0.4, LANE_Z + 0.8, 1.0, true);
    adult
}

/// **A pack picks the soft or the isolated target over a hunkered adult in a ring** (PACK-TUNE,
/// prey choice: wolves and lions favour the young and the isolated and leave a standing herd,
/// Mech 1970; MacNulty et al. 2007; Packer & Ruttan 1988). All else equal, and the ring a
/// little nearer: a half-grown juvenile on the other side, then a lone unhunkered adult there.
#[test]
fn a_pack_picks_a_juvenile_or_an_isolated_grazer_over_a_hunkered_adult_in_a_ring() {
    for (fraction, what) in [(0.5, "a juvenile"), (1.0, "an isolated adult")] {
        let (mut fauna, world, _) = choice_case(61);
        let mut flora = Flora::new(FloraConfig::default());
        // The pack stands at x = 2 to 2.5 m: the ring 2.75 m east of x = 2, the other 3.25 m
        // west of it (round the ring), both within the chase's reach.
        let ringed = ring_at(&mut fauna, &world, 4.75);
        let other = grazer(&mut fauna, &world, (2.0 - 3.25f64).rem_euclid(RING), LANE_Z, fraction, false);
        let took = step_until(&mut fauna, &world, &mut flora, 300, |f| f.packs()[0].target.is_some());
        assert!(took.is_some(), "the pack takes a target");
        let target = fauna.packs()[0].target;
        assert_eq!(target, Some(other), "it takes {what} ({other}), not the ringed adult ({ringed}): took {target:?}");
    }
}

/// **A pack breaks off a test on a defended target**: it takes a lone adult it can open; two
/// kin then hunker beside it, so the members present can no longer open it. The pack drops it
/// well before the chase's give-up, as a break-off, and does not take it again at once.
#[test]
fn a_pack_breaks_off_a_test_on_a_defended_target() {
    let (mut fauna, world, _) = choice_case(62);
    let mut flora = Flora::new(FloraConfig::default());
    let prey = grazer(&mut fauna, &world, 5.25, LANE_Z, 1.0, false);
    step_until(&mut fauna, &world, &mut flora, 300, |f| f.packs()[0].target == Some(prey)).expect("the pack takes the lone adult");
    let give_up = cubarium_voxel::ticks_at_least_one(fauna.config.founder(CH).pack.unwrap().chase_give_up_s) as u32;
    // Kin close in and the herd stands: the ring.
    let x = animal(&fauna, prey).pose.x;
    grazer(&mut fauna, &world, x + 0.4, LANE_Z - 0.8, 1.0, true);
    grazer(&mut fauna, &world, x + 0.4, LANE_Z + 0.8, 1.0, true);
    animal_mut(&mut fauna, prey).hunkered = true;
    fauna.percepts.entry(prey).or_default().vigil.hunkered = true;
    let before = fauna.ledger.pack_break_offs;
    let dropped = step_until(&mut fauna, &world, &mut flora, give_up, |f| f.packs()[0].target != Some(prey));
    assert!(dropped.is_some_and(|t| t < give_up / 2), "dropped in {dropped:?} ticks (give-up {give_up})");
    assert!(fauna.ledger.pack_break_offs > before, "counted as a break-off");
    let retaken = step_until(&mut fauna, &world, &mut flora, give_up, |f| f.packs()[0].target == Some(prey));
    assert!(retaken.is_none(), "taken again at once ({retaken:?} ticks)");
}

/// **A hovering bellwing above reach is never committed to**: a bellwing in flight held over
/// the pack's jaw band, in plain sight of a hungry pack, is never its target.
#[test]
fn a_hovering_bellwing_above_reach_is_never_committed_to() {
    let (mut fauna, world, members) = choice_case(63);
    let mut flora = Flora::new(FloraConfig::default());
    let band = {
        let h = fauna.config.founder(CH).hunter.expect("a hunter");
        let g = crate::body_of(&fauna.config, &animal(&fauna, members[0])).expect("a body");
        h.strike_height_m(&g)
    };
    let bw = introduce(&mut fauna, &world, crate::Founder::Bellwing, StartingStores::FULL);
    drive(&mut fauna, bw, 0.0, false);
    for _ in 0..400 {
        place(&mut fauna, bw, 4.5, LANE_Z, WEST);
        {
            let a = animal_mut(&mut fauna, bw);
            a.mobility.mode = crate::MobilityMode::Fly;
            a.pose.y_m = SURFACE_M + band + 0.5;
        }
        fauna.step_with(&world, &mut flora, 1);
        assert_ne!(fauna.packs()[0].target, Some(bw), "the pack committed to a bellwing above its reach");
    }
}
