//! **SHRED-NEST: the walk off a crowded nest goes by a route** (Wrysk 2026-10-11: "black
//! holes on the map where animals just get stuck and rotate around forever"). SNAIL-STUCK
//! found the ripple snail's reflex walks steering straight at a face a world query chose; the
//! littershredder's SHREDDER-NEST walk did the same: it steered straight at its free egg face
//! wherever that was, so a face over a wall it cannot climb or across a trench held it
//! pressing at the barrier until its give-up time.
//!
//! Each fixture is the predation plain with a barrier between a crowded nest and the one free
//! litter face, open at one end. The give-up time is set out of reach, so only the walk itself
//! can end the trip. Every bound comes from the config (the cruise, the controller period, the
//! pressed-walk clocks) and the fixture's geometry; no default, ratio or verdict is asserted.

use super::predation_fixture::*;
use crate::controller::{Actions, Controller, Response};
use crate::{Activity, Fauna, FaunaConfig, Founder, Reproduction, StartingStores, DT};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Material, World};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site};

const SHREDDER: Founder = Founder::Blind;
/// The layer's face (8, 6) and its middle, metres.
const HOME: (u32, u32) = (8, 6);
const AT: (f64, f64) = (2.125, 1.625);
/// The free litter face, 2.5 m east across the barrier.
const FAR: (u32, u32) = (18, 6);
/// The barrier runs along `x = BARRIER_X0..=BARRIER_X1` from `z = 0` to `z = GAP_Z - 1`; the
/// rows `GAP_Z..` are open ground around its end.
const BARRIER_X0: u32 = 12;
const BARRIER_X1: u32 = 14;
const GAP_Z: u32 = 10;
const HEIGHT: u32 = 12;

/// Holds still and opts into the reflexes: the body moves only where a reflex walks it.
struct StillReflexive;

impl Controller for StillReflexive {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        Response::Bounded(Actions { forward: 0.0, turn: 0.0, feed: 0.0 })
    }
    fn reset(&mut self) {}
    fn layer(&self) -> crate::Layer {
        crate::Layer::Heuristic
    }
}

#[derive(Clone, Copy, Debug)]
enum Barrier {
    /// A rock wall far taller than the body, roofed on both sides by a ledge one voxel out:
    /// a climber walking into it is turned back under the ledge.
    OverhungWall,
    /// A trench along the barrier, down to the bedrock, its sides undercut a voxel under the
    /// lip: nothing walks or climbs across it.
    UndercutTrench,
}

fn set(w: &mut World, x: u32, y: u32, z: u32, material: Material) {
    w.apply(WorldCommand::SetMaterial { x: i64::from(x), y, z, material });
}

/// The plain, raised to `HEIGHT` for room, with `barrier` between the nest and the free face:
/// open round its end, or across the whole plain when `closed`.
fn world(barrier: Barrier, closed: bool) -> World {
    let mut w = plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, 41, HEIGHT);
    let depth = w.config().depth;
    let top = 8;
    let end = if closed { depth } else { GAP_Z.min(depth) };
    for z in 0..end {
        match barrier {
            Barrier::OverhungWall => {
                for y in SURFACE_Y + 1..=top {
                    set(&mut w, 13, y, z, Material::Rock);
                }
                set(&mut w, 12, top, z, Material::Rock);
                set(&mut w, 14, top, z, Material::Rock);
            }
            Barrier::UndercutTrench => {
                // The soil cut away over the barrier's columns, and under the lip on
                // each side, which is left as a one-voxel roof over its side wall.
                for x in BARRIER_X0..=BARRIER_X1 {
                    for y in 1..=SURFACE_Y {
                        set(&mut w, x, y, z, Material::Air);
                    }
                }
                set(&mut w, BARRIER_X0 - 1, SURFACE_Y - 1, z, Material::Air);
                set(&mut w, BARRIER_X1 + 1, SURFACE_Y - 1, z, Material::Air);
            }
        }
    }
    w
}

fn litter(flora: &mut Flora, site: Site) {
    assert!(flora.deposit(site, Deposit { kind: DepositKind::Litter, organic: 1.0, mineral: 0.02, energy: 2.0 }));
}

/// The SHREDDER-NEST fixture's egg rule (a one-tick hold, room for eggs at a full reserve, a
/// 1 m nest that more than two others crowd, a 3 m seek), its give-up time out of reach.
fn rule() -> Reproduction {
    let core = FaunaConfig::default().founders[SHREDDER.index()].core;
    let egg = core.body_min;
    Reproduction {
        surplus_floor: core.body_max - 2.5 * egg,
        surplus_hold_s: DT,
        birth_interval_s: 10_000.0 * DT,
        clutch_size: 1,
        clutch_max: 4,
        egg_organic: egg,
        incubation_s: 10_000.0 * DT,
        nest_radius_m: 1.0,
        nest_crowd_max: 2,
        nest_seek: true,
        nest_seek_m: 3.0,
        nest_seek_give_up_s: 1.0e9,
        ..Reproduction::EGGS_PLACEHOLDER
    }
}

fn config(r: Reproduction) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for p in c.founders.iter_mut() {
        p.gut.s0 = 0.0;
    }
    let p = &mut c.founders[SHREDDER.index()];
    p.active = Activity::Always;
    p.core.reserve_cap = 1.0;
    p.core.reproduction = r;
    c
}

/// A ready layer on litter at [`AT`] in a crowd of three short of reserve (held still by a
/// script), the one free litter face at [`FAR`] across the barrier.
fn scene(barrier: Barrier, closed: bool) -> (Fauna, World, Flora, u64) {
    let world = world(barrier, closed);
    let r = rule();
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(r));
    let core = fauna.config.founder(SHREDDER).core;
    let short = (r.surplus_floor + 0.5 * r.egg_organic) / core.body_max;
    let id = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: 1.0 });
    place(&mut fauna, id, AT.0, AT.1, EAST);
    assert!(fauna.set_controller(id, Box::new(StillReflexive)));
    for k in 0..3 {
        let n = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: short });
        let ang = f64::from(k) * 2.1;
        place(&mut fauna, n, AT.0 + 0.4 * ang.cos(), AT.1 + 0.4 * ang.sin(), EAST);
        drive(&mut fauna, n, 0.0, false);
    }
    assert_eq!((animal(&fauna, id).site.x, animal(&fauna, id).site.z), HOME, "fixture: the layer's face");
    litter(&mut flora, animal(&fauna, id).site);
    let far = Site { x: FAR.0, y: SURFACE_Y, z: FAR.1 };
    assert_eq!(world.view().highest_support_in(i64::from(FAR.0), FAR.1, 0, HEIGHT - 1), Some(SURFACE_Y), "fixture: the free face is level with the nest");
    litter(&mut flora, far);
    (fauna, world, flora, id)
}

/// **A ready layer whose free egg face lies behind a barrier it cannot cross straight walks
/// to it round the barrier's end, or gives the walk up, in bounded time — and never turns on
/// the spot.** Bounded by twice the walk round the end at its cruise, the pressed-walk clock
/// and a few controller periods; the give-up time is out of reach. "On the spot": a full turn
/// of steered heading (its motor's turn-backs from a face aside) before it has moved a body
/// length.
#[test]
fn a_nest_walk_across_a_barrier_goes_round_it_or_is_given_up_and_never_spins() {
    for barrier in [Barrier::OverhungWall, Barrier::UndercutTrench] {
        let (mut fauna, world, mut flora, id) = scene(barrier, false);
        let c = fauna.config.clone();
        let m = c.manifest(SHREDDER);
        let length = c.founder(SHREDDER).body_at(1.0).length_m;
        let cruise = m.cruise_m_per_s;
        assert!(cruise > 0.0 && length > 0.0, "fixture: it walks");
        let period = m.cadence_ticks().max(1) as f64 * DT;
        // Round the barrier's end: to the corner past it, then on to the free face.
        let corner = ((f64::from(BARRIER_X0) - 0.5) * VOXEL_M, (f64::from(GAP_Z) + 0.5) * VOXEL_M);
        let corner2 = ((f64::from(BARRIER_X1) + 1.5) * VOXEL_M, corner.1);
        let far = ((f64::from(FAR.0) + 0.5) * VOXEL_M, (f64::from(FAR.1) + 0.5) * VOXEL_M);
        let d = |p: (f64, f64), q: (f64, f64)| ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
        let round = d(AT, corner) + d(corner, corner2) + d(corner2, far);
        let bound = 2.0 * round / cruise + c.teachers.press_replan_s + 4.0 * period;
        let ticks = (bound / DT).ceil() as usize;

        // A turn its motor makes for it (turned back from a face it would climb) flips its
        // heading at once; only the turning it steers, at most its yaw cap a tick, counts.
        let yaw_tick = c.founder(SHREDDER).yaw_cap_rad_per_s * DT * (1.0 + 1e-6);
        let mut anchor = animal(&fauna, id).pose;
        let mut turned = 0.0f64;
        let mut last = anchor.heading_rad;
        let mut done = None;
        for t in 0..ticks {
            fauna.step_with(&world, &mut flora, 1);
            let a = animal(&fauna, id);
            let mut dh = a.pose.heading_rad - last;
            dh = (dh + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            last = a.pose.heading_rad;
            if dh.abs() <= yaw_tick {
                turned += dh.abs();
            }
            if d((a.pose.x, a.pose.z), (anchor.x, anchor.z)) >= length {
                anchor = a.pose;
                turned = 0.0;
            }
            assert!(
                turned < std::f64::consts::TAU,
                "{barrier:?}: turned a full circle on the spot by {:.1} s at {:?} ({:.3}, {:.3})",
                t as f64 * DT,
                a.site,
                a.pose.x,
                a.pose.z
            );
            let tally = fauna.nest_seek_tally()[SHREDDER.index()];
            if !fauna.clutches.is_empty() || tally.give_ups > 0 {
                done = Some(t);
                break;
            }
        }
        let a = animal(&fauna, id);
        assert!(
            done.is_some(),
            "{barrier:?}: neither laid nor gave up in {bound:.0} s; at {:?} ({:.3}, {:.3}), trip {:?}",
            a.site,
            a.pose.x,
            a.pose.z,
            fauna.nest_trips.get(&id)
        );
        if let Some(c) = fauna.clutches.first() {
            let dx = f64::from(c.site.x) - f64::from(HOME.0);
            let dz = f64::from(c.site.z) - f64::from(HOME.1);
            assert!((dx * dx + dz * dz).sqrt() * VOXEL_M > rule().nest_radius_m, "{barrier:?}: laid outside the crowd's nest: {:?}", c.site);
        } else {
            assert!(!fauna.nest_trips.contains_key(&id), "{barrier:?}: a walk given up ends");
            assert!(a.reproduction.retry_ticks > 0, "{barrier:?}: and it waits out its retry, foraging");
        }
        assert_closes(&fauna, "after the walk");
    }
}

/// **A free egg face it can walk to by no route is no walk**: with the barrier across the
/// whole plain, the refused layer does not set out for the face behind it (it would press at
/// the barrier); it is refused, waits out its retry and lays nothing in the crowd.
#[test]
fn a_free_face_behind_a_closed_barrier_is_not_set_out_for() {
    for barrier in [Barrier::OverhungWall, Barrier::UndercutTrench] {
        let (mut fauna, world, mut flora, id) = scene(barrier, true);
        fauna.step_with(&world, &mut flora, 1);
        assert!(fauna.clutches.is_empty(), "{barrier:?}: refused in the crowd");
        assert!(!fauna.nest_trips.contains_key(&id), "{barrier:?}: no walk to a face it cannot reach: {:?}", fauna.nest_trips.get(&id));
        assert_eq!(fauna.nest_seek_tally()[SHREDDER.index()].walk_offs, 0, "{barrier:?}");
        assert!(animal(&fauna, id).reproduction.retry_ticks > 0, "{barrier:?}: it waits out its retry");
        assert_closes(&fauna, "after the refusal");
    }
}
