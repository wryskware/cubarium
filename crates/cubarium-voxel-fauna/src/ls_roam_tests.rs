//! **LS-ROAM, the patch against elsewhere** (`forage`): a lineage with `give_up_vs_memory`
//! weighs its patch against its own remembered intake while foraging, travel included.
//! Hand-built inputs; every threshold read from the config the case sets — no default,
//! ratio or population is asserted.
//!
//! - A patch well over the gut line is given up once its offer falls under what the body
//!   remembers, kept while over it, and kept with the memory off.
//! - The memory starts at the first bitten interval, counts a hungry interval that bit
//!   nothing as zero (the walk between patches), and holds while the body is sated.

use super::*;
use crate::controller::Actions;
use crate::step::predation_fixture::*;
use crate::{Fauna, FaunaConfig, Food, Founder, StartingStores};
use cubarium_voxel::World;

const SHREDDER: Founder = Founder::Blind;
const STOP: Actions = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };
const WALK: Actions = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };

/// The rule on with `vs_memory`, a memory that barely moves in one sample unless `memory_s`
/// says otherwise, a stay counted from its first sample.
fn config(vs_memory: f64, memory_s: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.founders[SHREDDER.index()].senses.live = false;
    c.founders[SHREDDER.index()].give_up_vs_memory = vs_memory;
    let t = &mut c.teachers;
    t.give_up_min_s = 1.0;
    t.give_up_window_s = 0.0;
    t.give_up_memory_s = memory_s;
    c
}

/// A hungry shredder and its index.
fn body(cfg: FaunaConfig) -> (Fauna, World, usize) {
    let world = clock_off(7);
    let mut fauna = Fauna::new(cfg);
    let s = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: 0.3 });
    place(&mut fauna, s, 4.1, 1.6, EAST);
    animal_mut(&mut fauna, s).eating = true;
    let i = fauna.animals.iter().position(|a| a.id == s).unwrap();
    (fauna, world, i)
}

fn ring(w: &World) -> f64 {
    f64::from(w.config().width) * w.config().voxel_m
}

fn period() -> f64 {
    SHREDDER.manifest().controller_period_s
}

/// The offer of a bite at `stock` for body `i`.
fn offer_at(fauna: &Fauna, i: usize, stock: f64) -> f64 {
    offer(&fauna.animals[i], fauna.config.founder(SHREDDER), Food::Litter, period(), 1.0, stock)
}

/// One controller sample of body `i` on a stay begun long enough ago, after a bite at
/// `stock`, its memory at `memory`: whether the layer walked it off.
fn sample_on_patch(fauna: &mut Fauna, w: &World, i: usize, stock: f64, memory: Option<f64>) -> bool {
    let id = fauna.animals[i].id;
    let a = fauna.animals[i];
    fauna.animals[i].founder_state.held = STOP;
    fauna.tick = cubarium_voxel::ticks_at_least_one(fauna.config.teachers.give_up_min_s) + SHREDDER.manifest().cadence_ticks();
    fauna.forage.insert(id, Patch {
        stay: Some(Stay { at: (a.pose.x, a.pose.z, a.pose.y_m), since: 0, rate: None, key: None }),
        memory,
        ..Patch::default()
    });
    let phys = *fauna.config.founder(SHREDDER);
    note_bite(fauna, i, &phys, Food::Litter, period(), 1.0, stock, None);
    leave(fauna, i, STOP, ring(w)).is_some()
}

/// A stock whose offer clears the gut-only line by a wide margin.
fn rich_stock(fauna: &Fauna, i: usize) -> f64 {
    let line = fauna.config.founder(SHREDDER).give_up_fraction.unwrap_or(fauna.config.teachers.give_up_fraction);
    let k = fauna.config.founder(SHREDDER).core.bite_half_stock.max(1e-12);
    let mut e = k;
    while offer_at(fauna, i, e) < 4.0 * line {
        e *= 2.0;
        assert!(e.is_finite() && e < k * 1e12, "no stock clears the gut line");
    }
    e
}

#[test]
fn a_rich_patch_is_left_once_it_offers_less_than_the_body_remembers() {
    let (mut fauna, w, i) = body(config(1.0, 1.0e9));
    let stock = rich_stock(&fauna, i);
    let q = offer_at(&fauna, i, stock);
    let line = fauna.config.teachers.give_up_fraction;
    assert!(q > line, "the patch is over the gut-only line: {q} vs {line}");
    assert!(sample_on_patch(&mut fauna, &w, i, stock, Some(2.0 * q)), "under what it remembers: left");
    assert_eq!(fauna.forage_tally()[SHREDDER.index()].give_ups, 1);

    let (mut fauna, w, i) = body(config(1.0, 1.0e9));
    assert!(!sample_on_patch(&mut fauna, &w, i, stock, Some(0.5 * q)), "over what it remembers: kept");

    let (mut fauna, w, i) = body(config(0.0, 1.0e9));
    assert!(!sample_on_patch(&mut fauna, &w, i, stock, Some(2.0 * q)), "the memory off: the gut-only line keeps it");
}

#[test]
fn the_memory_starts_at_a_bite_counts_the_walks_and_holds_while_sated() {
    let (mut fauna, w, i) = body(config(1.0, 4.0 * period()));
    let id = fauna.animals[i].id;
    let stock = rich_stock(&fauna, i);
    let q = offer_at(&fauna, i, stock);

    // Hungry and walking, never having bitten: nothing to remember yet.
    fauna.forage.insert(id, Patch::default());
    fauna.animals[i].founder_state.held = WALK;
    assert!(leave(&mut fauna, i, WALK, ring(&w)).is_none());
    assert_eq!(fauna.forage_memory(id), None, "no bite yet, no memory");

    // The first bitten interval starts it at that interval's offer.
    fauna.forage.insert(id, Patch::default());
    fauna.animals[i].founder_state.held = STOP;
    let phys = *fauna.config.founder(SHREDDER);
    note_bite(&mut fauna, i, &phys, Food::Litter, period(), 1.0, stock, None);
    leave(&mut fauna, i, STOP, ring(&w));
    let m0 = fauna.forage_memory(id).expect("a bite starts the memory");
    assert!((m0 - q).abs() <= 1e-12 * q.max(1.0), "{m0} vs {q}");

    // A hungry interval that bit nothing (walking between patches) pulls it down.
    fauna.animals[i].founder_state.held = WALK;
    leave(&mut fauna, i, WALK, ring(&w));
    let m1 = fauna.forage_memory(id).expect("kept");
    assert!(m1 < m0 && m1 > 0.0, "a walk counts as zero intake: {m0} -> {m1}");

    // Sated, it holds.
    fauna.animals[i].eating = false;
    leave(&mut fauna, i, WALK, ring(&w));
    assert_eq!(fauna.forage_memory(id), Some(m1), "sated, the memory holds");
}
