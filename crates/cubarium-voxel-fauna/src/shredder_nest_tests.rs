//! **SHREDDER-NEST: walking off a crowded nest** (Wrysk 2026-10-10, option A: "a shredder that
//! finds its spot crowded goes somewhere quieter to lay"). A child of `step`, so a case can
//! reach the layer's own fields.
//!
//! Every case checks the mechanism against the config it sets: no default, ratio or
//! population is asserted.
//!
//! - A ready layer inside a crowd walks to an uncrowded litter face and lays there, not in
//!   the crowd.
//! - With no uncrowded face in reach it does not lay in the crowd and takes no walk; a walk
//!   that never arrives is given up after the give-up time, still with no clutch.
//! - With the knob off it is refused and stays put, as before.
//! - The ledger closes on every tick across a walk-off lay.

use super::predation_fixture::*;
use crate::controller::{Actions, Controller, Response};
use crate::{Activity, Fauna, FaunaConfig, Founder, Reproduction, StartingStores, DT};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site};

const SHREDDER: Founder = Founder::Blind;
/// Where the layer stands, metres: the middle of face (8, 6).
const AT: (f64, f64) = (2.125, 1.625);

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

fn litter(flora: &mut Flora, site: Site, organic: f64) {
    assert!(flora.deposit(
        site,
        Deposit {
            kind: DepositKind::Litter,
            organic,
            mineral: 0.02 * organic,
            energy: 2.0 * organic,
        },
    ));
}

/// The test's egg rule: a one-tick hold, a long interval and incubation, room for 2.5 eggs
/// above the floor at a full reserve cap of 1, a nest radius of 1 m that more than 2 others
/// crowd, and the walk off a crowded nest `seek` within 3 m, given up after 1,000 ticks
/// (several times the fixture's walk).
fn rule(seek: bool) -> Reproduction {
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
        nest_seek: seek,
        nest_seek_m: 3.0,
        nest_seek_give_up_s: 1_000.0 * DT,
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

/// A crowd around [`AT`] (three of the lineage within half a metre, too short of reserve to
/// lay, held still by a script), a ready layer at its centre on litter, and litter on
/// `free` (each face listed) as well. The layer's controller: its own reflexes when
/// `walks`, a still script otherwise.
fn scene(r: Reproduction, free: &[Site], walks: bool) -> (Fauna, World, Flora, u64) {
    let world = clock_off(41);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(r));
    let core = fauna.config.founder(SHREDDER).core;
    let short = (r.surplus_floor + 0.5 * r.egg_organic) / core.body_max;
    let id = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: 1.0 });
    place(&mut fauna, id, AT.0, AT.1, EAST);
    if walks {
        assert!(fauna.set_controller(id, Box::new(StillReflexive)));
    } else {
        drive(&mut fauna, id, 0.0, false);
    }
    for k in 0..3 {
        let n = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: short });
        let ang = f64::from(k) * 2.1;
        place(&mut fauna, n, AT.0 + 0.4 * ang.cos(), AT.1 + 0.4 * ang.sin(), EAST);
        drive(&mut fauna, n, 0.0, false);
    }
    litter(&mut flora, animal(&fauna, id).site, 1.0);
    for &s in free {
        litter(&mut flora, s, 1.0);
    }
    (fauna, world, flora, id)
}

/// Level distance, metres, between the centres of two faces on the fixture's ring.
fn apart(world: &World, a: Site, b: Site) -> f64 {
    let ring = f64::from(world.config().width) * VOXEL_M;
    let c = |s: Site| ((f64::from(s.x) + 0.5) * VOXEL_M, (f64::from(s.z) + 0.5) * VOXEL_M);
    let ((ax, az), (bx, bz)) = (c(a), c(b));
    let (dx, dz) = crate::pose::wrapped_delta(ax, az, bx, bz, ring);
    (dx * dx + dz * dz).sqrt()
}

/// A free litter face 2.5 m from the crowd (outside its 1 m nest), and one 0.5 m from it
/// (inside), which the walk passes over.
fn faces() -> (Site, Site) {
    let far = Site { x: 18, y: SURFACE_Y, z: 6 };
    let near = Site { x: 10, y: SURFACE_Y, z: 6 };
    (far, near)
}

/// Steps until the first clutch, at most `ticks`; the ledger checked after each when `check`.
fn until_laid(fauna: &mut Fauna, world: &World, flora: &mut Flora, ticks: usize, check: bool) {
    for _ in 0..ticks {
        fauna.step_with(world, flora, 1);
        if check {
            assert_closes(fauna, "on the walk off a crowded nest");
        }
        if !fauna.clutches.is_empty() {
            return;
        }
    }
}

#[test]
fn a_ready_layer_in_a_crowd_walks_to_a_free_face_and_lays_there() {
    let r = rule(true);
    let (far, near) = faces();
    let (mut fauna, world, mut flora, id) = scene(r, &[far, near], true);
    let start = animal(&fauna, id).site;
    fauna.step_with(&world, &mut flora, 1);
    assert!(fauna.clutches.is_empty(), "refused in the crowd");
    let trip = fauna.nest_trips.get(&id).copied().expect("it sets out instead of waiting");
    assert_eq!((trip.target.x, trip.target.z), (far.x, far.z), "for the free face, not the crowded one: {:?}", trip.target);
    assert_eq!(animal(&fauna, id).reproduction.retry_ticks, 0, "no retry wait while it walks");
    assert_eq!(fauna.nest_seek_tally()[SHREDDER.index()].walk_offs, 1);
    until_laid(&mut fauna, &world, &mut flora, 4000, false);
    assert_eq!(fauna.clutches.len(), 1, "it laid");
    let c = fauna.clutches[0];
    assert_eq!((c.site.x, c.site.z), (far.x, far.z), "on the free face: {:?}", c.site);
    assert!(apart(&world, c.site, start) > r.nest_radius_m, "outside the crowd's nest");
    let t = fauna.nest_seek_tally()[SHREDDER.index()];
    assert_eq!((t.walk_offs, t.walk_lays, t.give_ups), (1, 1, 0));
    assert!(!fauna.nest_trips.contains_key(&id), "the walk ends with the clutch");
    assert_eq!(fauna.ledger.nest_refusals_by_founder[SHREDDER.index()], 1, "one refusal, the one that started the walk");
    assert_closes(&fauna, "after the walk-off lay");
}

#[test]
fn with_no_free_face_it_never_lays_in_the_crowd_and_a_walk_that_never_arrives_is_given_up() {
    // No free face in reach: refused, no walk, the retry wait, no clutch.
    let r = rule(true);
    let (_, near) = faces();
    let (mut fauna, world, mut flora, id) = scene(r, &[near], true);
    fauna.step_with(&world, &mut flora, 1);
    assert!(fauna.clutches.is_empty() && fauna.nest_trips.is_empty(), "no clutch, no walk");
    assert!(animal(&fauna, id).reproduction.retry_ticks > 0, "it waits out its retry");
    assert_eq!(fauna.nest_seek_tally()[SHREDDER.index()].walk_offs, 0);

    // A free face it cannot walk to (a still script drives it, so no reflex moves it): the
    // walk is begun, nothing is laid in the crowd, and it is given up at the give-up time.
    let (far, near) = faces();
    let (mut fauna, world, mut flora, id) = scene(r, &[far, near], false);
    fauna.step_with(&world, &mut flora, 1);
    let since = fauna.nest_trips.get(&id).expect("a walk begun").since;
    let give_up = r.nest_seek_give_up_ticks();
    while fauna.tick - since < give_up {
        assert!(fauna.nest_trips.contains_key(&id), "the goal is kept until the give-up time");
        fauna.step_with(&world, &mut flora, 1);
        assert!(fauna.clutches.is_empty(), "never a clutch in the crowd");
    }
    let held = crate::LAY_GATES.iter().position(|&g| g == "seeking").unwrap();
    assert!(fauna.lay_refusals_by_founder()[SHREDDER.index()][held] > 0, "the walk's ticks are held at `seeking`");
    fauna.step_with(&world, &mut flora, 1);
    assert!(!fauna.nest_trips.contains_key(&id), "given up");
    let t = fauna.nest_seek_tally()[SHREDDER.index()];
    assert_eq!((t.walk_offs, t.walk_lays, t.give_ups), (1, 0, 1));
    assert!(animal(&fauna, id).reproduction.retry_ticks > 0, "and it waits out its retry");
    assert!(fauna.clutches.is_empty());
    assert_closes(&fauna, "after a walk given up");
}

#[test]
fn with_the_knob_off_a_refused_layer_stays_put() {
    let r = rule(false);
    let (far, near) = faces();
    let (mut fauna, world, mut flora, id) = scene(r, &[far, near], true);
    let start = animal(&fauna, id).pose;
    fauna.step_with(&world, &mut flora, 1);
    assert!(fauna.clutches.is_empty(), "refused");
    assert!(fauna.nest_trips.is_empty(), "no walk");
    assert!(animal(&fauna, id).reproduction.retry_ticks > 0, "it waits out its retry");
    assert_eq!(fauna.ledger.nest_refusals_by_founder[SHREDDER.index()], 1);
    // As long as the walk-off case took to lay, and longer: it never leaves the crowd.
    until_laid(&mut fauna, &world, &mut flora, 1_500, false);
    assert!(fauna.clutches.is_empty(), "never laid");
    let p = animal(&fauna, id).pose;
    assert!((p.x - start.x).abs() < 1e-9 && (p.z - start.z).abs() < 1e-9, "stayed put: {p:?}");
    assert_eq!(fauna.nest_seek_tally()[SHREDDER.index()], crate::NestSeekTally::default());
}

#[test]
fn the_ledger_closes_across_a_walk_off_lay() {
    let (far, near) = faces();
    let (mut fauna, world, mut flora, _) = scene(rule(true), &[far, near], true);
    assert_closes(&fauna, "before");
    until_laid(&mut fauna, &world, &mut flora, 4000, true);
    assert_eq!(fauna.clutches.len(), 1, "fixture: it laid after its walk");
    assert_eq!(fauna.nest_seek_tally()[SHREDDER.index()].walk_lays, 1);
    for _ in 0..20 {
        fauna.step_with(&world, &mut flora, 1);
        assert_closes(&fauna, "after the walk-off lay");
    }
}

