//! **FORAGE-LEAVE** (`forage`): a heuristic forager gives up a patch whose bites cannot end
//! a meal. Hand-built inputs, every number read from the config the case sets — no default,
//! ratio or population is asserted.
//!
//! - A hungry shredder on a thin litter patch stays at least `give_up_min_s`, then gives it
//!   up within a few windows and walks off without feeding.
//! - On a rich patch it is never pushed off.
//! - A just-arrived body is not pushed off.
//! - `give_up_fraction` 0 turns the rule off.
//! - A given-up stand joins the reflex walks' shun list; the walk off holds a feeding answer
//!   straight on for its time, and after it the body is walked off the patch only inside
//!   its zone and only while the shun lasts.

use super::*;
use crate::controller::Actions;
use crate::step::predation_fixture::*;
use crate::{Fauna, FaunaConfig, Founder, StartingStores, Steer};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site};

const SHREDDER: Founder = Founder::Blind;

fn config(fraction: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.founders[SHREDDER.index()].senses.live = false;
    let t = &mut c.teachers;
    // A taste gate far under any patch here: the shredder's own gate leaves litter while
    // its bites are still near full, so the case lowers it to reach a thin patch at all.
    t.feed_gate = 1.0e-6;
    t.give_up_fraction = fraction;
    t.give_up_min_s = 1.0;
    t.give_up_window_s = 0.25;
    t.give_up_shun_s = 30.0;
    c
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

/// A hungry shredder (as introduced, its eating latch on) on its heuristic, standing on a
/// strip of litter of `organic` per site.
fn on_patch(cfg: FaunaConfig, organic: f64) -> (Fauna, World, Flora, u64) {
    let world = clock_off(7);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(cfg);
    let s = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: 0.3 });
    place(&mut fauna, s, 4.1, 1.6, EAST);
    let site = animal(&fauna, s).site;
    for dx in 0..4 {
        litter(&mut flora, Site { x: site.x + dx, ..site }, organic);
    }
    animal_mut(&mut fauna, s).eating = true;
    let teachers = fauna.config.teachers;
    fauna.controllers.set(s, crate::controller::heuristic(SHREDDER, teachers));
    (fauna, world, flora, s)
}

/// A stock whose offer is a small share of the give-up line, from the body's own gut: the
/// offer at stock `E` is `2·E/(E+K)/full_tolerance`-shaped, so `E` a small fraction of `K`
/// offers a small fraction of a fill rate.
fn thin(cfg: &FaunaConfig) -> f64 {
    let p = cfg.founder(SHREDDER);
    let k = p.core.bite_half_stock;
    let mut e = k;
    // Halve until a hungry adult's offer is under a quarter of the line.
    let a = {
        let world = clock_off(7);
        let mut f = Fauna::new(cfg.clone());
        let s = introduce(&mut f, &world, SHREDDER, StartingStores { body: 1.0, reserve: 0.3 });
        animal(&f, s)
    };
    let period = SHREDDER.manifest().controller_period_s;
    while offer(&a, p, crate::Food::Litter, period, 1.0, e) > 0.25 * cfg.teachers.give_up_fraction {
        e *= 0.5;
    }
    e
}

/// Steps until `ticks` have passed, returning the tick (since the start) of the first
/// sample the give-up layer set its actions, if any.
fn first_leave(fauna: &mut Fauna, world: &World, flora: &mut Flora, s: u64, ticks: u64) -> Option<u64> {
    let mut first = None;
    for t in 0..ticks {
        fauna.step_with(world, flora, 1);
        if first.is_none() && fauna.steer_of(s) == Some(Steer::Leave) {
            first = Some(t + 1);
            let held = animal(fauna, s).founder_state.held;
            assert_eq!(held.feed, 0.0, "walked off a given-up patch it does not feed");
            assert!(held.forward > fauna.config.teachers.forage_stay_forward, "it walks on: {held:?}");
        }
    }
    first
}

fn ticks(s: f64) -> u64 {
    cubarium_voxel::ticks_at_least_one(s)
}

#[test]
fn a_hungry_body_on_a_thin_patch_gives_it_up_after_its_least_stay() {
    let cfg = config(0.5);
    let e = thin(&cfg);
    let (mut fauna, world, mut flora, s) = on_patch(cfg.clone(), e);
    let tc = cfg.teachers;
    let period = SHREDDER.manifest().cadence_ticks();
    // A few samples in: it has stopped to feed (the precondition), and is not pushed off.
    for _ in 0..(3 * period) {
        fauna.step_with(&world, &mut flora, 1);
    }
    let held = animal(&fauna, s).founder_state.held;
    assert_eq!(held.feed, 1.0, "the thin patch tastes like food: {held:?}");
    assert_eq!(held.forward, tc.forage_stay_forward, "and it holds still on it");
    let limit = ticks(tc.give_up_min_s) + ticks(8.0 * tc.give_up_window_s) + 4 * period;
    let left = first_leave(&mut fauna, &world, &mut flora, s, limit).map(|t| t + 3 * period);
    let left = left.expect("it gave the thin patch up");
    assert!(left >= ticks(tc.give_up_min_s), "not before its least stay: tick {left}");
    let tally = fauna.forage_tally()[SHREDDER.index()];
    assert!(tally.give_ups >= 1 && tally.leave_samples + tally.give_ups >= 1, "{tally:?}");
    assert_closes(&fauna, "after a give-up");
}

#[test]
fn a_hungry_body_on_a_rich_patch_is_not_pushed_off() {
    let cfg = config(0.5);
    // Far over the bite's half stock: the offer is near the gut's full bite rate.
    let rich = 1.0e3 * cfg.founder(SHREDDER).core.bite_half_stock;
    let (mut fauna, world, mut flora, s) = on_patch(cfg.clone(), rich);
    let tc = cfg.teachers;
    let span = ticks(tc.give_up_min_s) + ticks(8.0 * tc.give_up_window_s) + 4 * SHREDDER.manifest().cadence_ticks();
    assert_eq!(first_leave(&mut fauna, &world, &mut flora, s, span), None);
    assert_eq!(fauna.forage_tally()[SHREDDER.index()].give_ups, 0);
    assert!(fauna.ledger.bites_by_founder[SHREDDER.index()] > 0, "it fed there");
}

#[test]
fn a_just_arrived_body_is_not_pushed_off_and_the_switch_turns_the_rule_off() {
    // Just arrived: up to its least stay, a thin patch is still fed on.
    let cfg = config(0.5);
    let e = thin(&cfg);
    let (mut fauna, world, mut flora, s) = on_patch(cfg.clone(), e);
    let before = ticks(cfg.teachers.give_up_min_s).saturating_sub(1);
    assert_eq!(first_leave(&mut fauna, &world, &mut flora, s, before), None);
    assert_eq!(animal(&fauna, s).founder_state.held.feed, 1.0);

    // Off: the same patch, long past the least stay, is never given up.
    let mut off = cfg.clone();
    off.teachers.give_up_fraction = 0.0;
    let (mut fauna, world, mut flora, s) = on_patch(off, e);
    let span = ticks(cfg.teachers.give_up_min_s) + ticks(8.0 * cfg.teachers.give_up_window_s) + 20;
    assert_eq!(first_leave(&mut fauna, &world, &mut flora, s, span), None);
    assert_eq!(fauna.forage_tally()[SHREDDER.index()].give_ups, 0);
    assert_eq!(animal(&fauna, s).founder_state.held.feed, 1.0, "still feeding");
}

/// The layer itself on a hand-built record: a stay long past its least time with a
/// trailing offer of nothing, its bites off stand 77. Giving up shuns the stand for the
/// reflex walks, and walks the body off while it would stop to feed inside the zone — and
/// not outside it, and not after the shun lapses.
#[test]
fn a_given_up_stand_is_shunned_and_the_walk_off_holds_only_in_its_zone_and_time() {
    let cfg = config(0.5);
    let world = clock_off(7);
    let mut fauna = Fauna::new(cfg.clone());
    let s = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: 0.3 });
    place(&mut fauna, s, 4.1, 1.6, EAST);
    animal_mut(&mut fauna, s).eating = true;
    fauna.tick = 100_000;
    let i = fauna.animals.iter().position(|a| a.id == s).expect("alive");
    let a = animal(&fauna, s);
    let here = (a.pose.x, a.pose.z, a.pose.y_m);
    fauna.forage.insert(
        s,
        Patch {
            stay: Some(Stay {
                at: here,
                since: 0,
                rate: Some(0.0),
                key: Some(77),
            }),
            ..Patch::default()
        },
    );
    let ring = f64::from(world.config().width) * world.config().voxel_m;
    let stop = Actions {
        forward: cfg.teachers.forage_stay_forward,
        turn: 0.0,
        feed: 1.0,
    };
    let go = leave(&mut fauna, i, stop, ring).expect("a stay of nothing past its least time is given up");
    assert_eq!(go.feed, 0.0);
    assert!(go.forward > stop.forward);
    assert!(fauna.press_of(s).2.iter().any(|&(k, _)| k == 77), "the stand is shunned for reflex walks");

    // The walk off: a turn back to food it sees is held straight on, without feeding; an
    // answer that does not feed is its own.
    fauna.tick += 1;
    let back = Actions { forward: 1.0, turn: 0.3, feed: 1.0 };
    let held = leave(&mut fauna, i, back, ring).expect("the walk off holds");
    assert_eq!((held.turn, held.feed), (0.0, 0.0));
    let wander = Actions { forward: 1.0, turn: 0.3, feed: 0.0 };
    assert_eq!(leave(&mut fauna, i, wander, ring), None);
    // An escape's stop turns away from what refused it: never walked on.
    let escape = Actions { forward: 0.0, turn: 1.0, feed: 1.0 };
    assert_eq!(leave(&mut fauna, i, escape, ring), None, "an escape is its own");
    // After the walk off, inside the zone while the shun lasts: a stop to feed is walked
    // off, an approach is left alone.
    fauna.tick += ticks(cfg.teachers.give_up_leave_s);
    assert!(leave(&mut fauna, i, stop, ring).is_some(), "inside the zone it is walked off");
    assert_eq!(leave(&mut fauna, i, back, ring), None);
    // Outside the zone: a new patch, fed on.
    let length = crate::body_of(&fauna.config, &animal(&fauna, s)).expect("a body").length_m;
    let away = here.0 + 2.0 * cfg.teachers.give_up_zone_bl * length + 0.1;
    animal_mut(&mut fauna, s).pose.x = away;
    assert_eq!(leave(&mut fauna, i, stop, ring), None, "outside the zone it may feed");
    // Back inside after the shun lapses: fed on.
    animal_mut(&mut fauna, s).pose.x = here.0;
    fauna.tick += ticks(cfg.teachers.give_up_shun_s) + 1;
    assert_eq!(leave(&mut fauna, i, stop, ring), None, "the shun has lapsed");
}

