//! **Generation time, package D1b**: detritivores that breed fast enough to follow their
//! food. Written before the mechanisms were tuned; every case checks a rule on hand-built
//! inputs **against the config it sets**, so no default, ratio or population is asserted.
//!
//! - The gut's mass exponent: a small body digests and bites more per unit of itself than
//!   a large one, by the configured `(body / body_max)^(e − 1)`; at `e` = 1 they are equal.
//! - The brood: a mature body earmarks `brood_share` of its net income for its next clutch
//!   and grows on the rest; a juvenile earmarks nothing; the brood stops at what the full
//!   clutch its reserve cap holds costs.
//! - So a fed young body at laying size reaches laying eligibility, and a hungry one does
//!   not.
//! - The brood does not sate: hunger counts only the reserve above it.
//! - A full-clutch wait waits for what the body's own reserve cap can pay for, not for a
//!   `clutch_max` it can never carry.
//! - Ledgers conserve through each.

use super::predation_fixture::*;
use crate::{Fauna, FaunaConfig, Food, Founder, Reproduction, StartingStores, DT};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site, Taken};

const SHREDDER: Founder = Founder::Blind;

/// Litter on every face of the plain, at the composition the arenas deposit it at.
fn carpet(world: &World, flora: &mut Flora, organic: f64) {
    let c = world.view().config;
    for z in 0..c.depth {
        for x in 0..c.width {
            assert!(flora.deposit(
                Site { x, y: SURFACE_Y, z },
                Deposit { kind: DepositKind::Litter, organic, mineral: 0.05 * organic, energy: 2.0 * organic },
            ));
        }
    }
}

/// The shredder as a small breeder: laying at 0.4 of the adult, hatching at 0.12, an egg
/// of 1.2 hatchlings above a floor of a quarter egg, clutches of up to 3, a fast gut (τ 2 s,
/// k 0.2) so a few hundred ticks are a meal and more, growth never the limit, no lifespan,
/// the gut's exponent `e` and the brood share `share`. The fixture's own numbers.
fn breeder(e: f64, share: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    let p = &mut c.founders[SHREDDER.index()];
    let max = p.core.body_max;
    p.core.birth_body = 0.4 * max;
    p.core.body_min = 0.12 * max;
    p.core.lifespan_median_s = 0.0;
    // Growth as fast as the live presets' is beside their gut: never the limit, so the
    // reserve above the brood is built into structure the tick it is there.
    p.core.growth_max_per_s = 1.0;
    p.gut.stomach_k = 0.2;
    p.gut.digestion_s = [2.0; Food::COUNT];
    p.gut.bite_per_body_s = 2.0 * 0.2 / 2.0;
    p.gut.mass_exponent = e;
    let egg = 1.2 * p.core.body_min;
    p.core.reproduction = Reproduction {
        egg_organic: egg,
        surplus_floor: 0.25 * egg,
        clutch_max: 3,
        nest_radius_m: 0.0,
        brood_share: share,
        surplus_hold_s: 10.0 * DT,
        birth_interval_s: 1.0e6 * DT,
        incubation_s: 1.0e6 * DT,
        ..Reproduction::EGGS_PLACEHOLDER
    };
    c
}

/// One shredder of structure `body` (a fraction of `body_max`) and reserve `reserve` (a
/// fraction of its cap) on a plain carpeted with `litter`, standing, feeding as `feed`.
fn one(config: FaunaConfig, body: f64, reserve: f64, litter: f64, feed: bool) -> (Fauna, World, Flora, u64) {
    let world = clock_off(53);
    let mut flora = Flora::new(FloraConfig::default());
    if litter > 0.0 {
        carpet(&world, &mut flora, litter);
    }
    let mut fauna = Fauna::new(config);
    let id = introduce(&mut fauna, &world, SHREDDER, StartingStores { body, reserve });
    drive(&mut fauna, id, 0.0, feed);
    (fauna, world, flora, id)
}

// ---------------------------------------------------------- the gut's mass exponent

/// Organic matter digested in one tick per unit body by a shredder of structure `frac` ×
/// `body_max` whose stomach holds half its capacity of litter, under `config`.
fn digested_per_body(config: &FaunaConfig, frac: f64) -> f64 {
    let (mut fauna, world, mut flora, id) = one(config.clone(), frac, 0.5, 0.0, false);
    fauna.set_births_enabled(false);
    let k = fauna.config.founder(SHREDDER).gut.capacity(animal(&fauna, id).body);
    animal_mut(&mut fauna, id).stomach.pools[Food::Litter.index()] =
        Taken { organic: 0.5 * k, mineral: 0.05 * 0.5 * k, energy: 2.0 * 0.5 * k };
    let before = fauna.ledger.digested_by_founder[SHREDDER.index()];
    // Upkeep comes out of the reserve, so the structure digestion reads is this one.
    let body = animal(&fauna, id).body;
    fauna.step_with(&world, &mut flora, 1);
    (fauna.ledger.digested_by_founder[SHREDDER.index()] - before) / body
}

/// **A small body's gut is faster per unit of itself**: digestion per unit body of a body
/// at 0.15 of `body_max` against the adult's is `0.15^(e − 1)` under the configured `e`,
/// and so is its bite; at `e` = 1 they are the same.
#[test]
fn a_small_body_digests_and_bites_more_per_unit_body_by_the_configured_exponent() {
    let small: f64 = 0.15;
    for e in [0.6, 0.75, 1.0] {
        let config = breeder(e, 0.0);
        let want = small.powf(e - 1.0);
        let ratio = digested_per_body(&config, small) / digested_per_body(&config, 1.0);
        assert!((ratio - want).abs() <= 1e-6 * want, "e {e}: digestion per body small / adult {ratio}, want {want}");
        if e < 1.0 {
            assert!(ratio > 1.0, "e {e}: the small body digests more per unit body");
        }

        // The bite, per unit body, at full effort on a deep pile with an empty stomach.
        let phys = *config.founder(SHREDDER);
        let bite = |frac: f64| {
            let (fauna, _, _, id) = one(config.clone(), frac, 0.5, 0.0, false);
            let mut a = animal(&fauna, id);
            a.eating = true;
            super::gut_bite(&a, &phys, 0.25, 1.0, 1.0e3) / a.body
        };
        let ratio = bite(small) / bite(1.0);
        assert!((ratio - want).abs() <= 1e-9 * want, "e {e}: bite per body small / adult {ratio}, want {want}");
    }
}

// ------------------------------------------------------------------------ the brood

/// **The κ split**: a body at laying size, fed, earmarks `brood_share` of its net income
/// (assimilated less upkeep) as brood and grows on the rest; a juvenile, fed the same,
/// earmarks nothing. Births off, so nothing is laid.
#[test]
fn a_mature_body_earmarks_its_brood_share_of_net_income_and_grows_on_the_rest() {
    for share in [0.3, 0.6, 1.0] {
        let config = breeder(1.0, share);
        let core = config.founder(SHREDDER).core;
        let laying = core.birth_body / core.body_max;
        let (mut fauna, world, mut flora, id) = one(config.clone(), laying, 0.05, 1.0, true);
        fauna.set_births_enabled(false);
        // Two ticks to a first bite digested, so the brood the measurement starts from is
        // already open.
        for _ in 0..2 {
            fauna.step_with(&world, &mut flora, 1);
        }
        let a0 = animal(&fauna, id);
        assert!(a0.reproduction.brood > 0.0, "fixture: the brood has opened");
        let (assim0, upkeep0) = (fauna.ledger.assimilated_by_founder[SHREDDER.index()], fauna.ledger.upkeep_by_founder[SHREDDER.index()]);
        for _ in 0..100 {
            fauna.step_with(&world, &mut flora, 1);
            let a = animal(&fauna, id);
            assert!(a.reproduction.brood <= a.reserve + 1e-15, "the brood is inside the reserve");
        }
        let a = animal(&fauna, id);
        let net = (fauna.ledger.assimilated_by_founder[SHREDDER.index()] - assim0)
            - (fauna.ledger.upkeep_by_founder[SHREDDER.index()] - upkeep0);
        assert!(net > 0.0, "fixture: it ate");
        let target = core.reproduction.brood_target(&core, a.body);
        assert!(a.reproduction.brood < target, "fixture: the brood is still filling ({} of {target})", a.reproduction.brood);
        let brood = a.reproduction.brood - a0.reproduction.brood;
        assert!((brood - share * net).abs() <= 1e-9 * net, "share {share}: brood gained {brood}, want {}", share * net);
        // The rest went to structure, or is still free in the reserve.
        let rest = (a.body - a0.body) + (a.reserve - a.reproduction.brood) - (a0.reserve - a0.reproduction.brood);
        assert!((rest - (1.0 - share) * net).abs() <= 1e-9 * net, "share {share}: the rest {rest}, want {}", (1.0 - share) * net);
        if share < 1.0 {
            assert!(a.body > a0.body, "share {share}: it grew on the rest");
        }
        assert_closes(&fauna, "after a fed brood");

        // A juvenile earmarks nothing.
        let (mut fauna, world, mut flora, id) = one(config, 0.5 * laying, 0.05, 1.0, true);
        fauna.set_births_enabled(false);
        let b0 = animal(&fauna, id).body;
        for _ in 0..100 {
            fauna.step_with(&world, &mut flora, 1);
        }
        let a = animal(&fauna, id);
        if a.body < core.birth_body {
            assert_eq!(a.reproduction.brood, 0.0, "a juvenile has no brood");
        }
        assert!(a.body > b0, "the juvenile grew");
    }
}

/// **The brood stops at its clutch**: fed long enough, the brood reaches what the full
/// clutch its reserve cap pays for costs and never exceeds it; what comes after grows the
/// body.
#[test]
fn the_brood_stops_at_the_full_clutch_its_cap_pays_for() {
    let share = 0.8;
    let config = breeder(1.0, share);
    let core = config.founder(SHREDDER).core;
    let (mut fauna, world, mut flora, id) = one(config, core.birth_body / core.body_max, 0.05, 1.0, true);
    fauna.set_births_enabled(false);
    let mut reached = None;
    for t in 0..600u32 {
        fauna.step_with(&world, &mut flora, 1);
        let a = animal(&fauna, id);
        let target = core.reproduction.brood_target(&core, a.body);
        assert!(target > 0.0, "fixture: the cap pays for an egg");
        assert!(a.reproduction.brood <= target * (1.0 + 1e-12), "tick {t}: brood {} over its target {target}", a.reproduction.brood);
        if reached.is_none() && a.reproduction.brood >= target * (1.0 - 1e-9) {
            reached = Some((t, a.body));
        }
    }
    let (_, body_then) = reached.expect("the brood fills within the fixture's run");
    assert!(animal(&fauna, id).body > body_then, "after the brood is full the income grows the body");
    assert_closes(&fauna, "after a full brood");
}

/// **Fed, a young body at laying size becomes eligible to lay while it is still growing;
/// hungry, it does not become eligible at all.** Both start at laying size with a nearly
/// empty reserve; one stands on a deep litter carpet, the other on bare soil. Without the
/// brood, growth takes every surplus and the reserve fills only once the body has stopped
/// at `body_max`.
#[test]
fn a_fed_young_body_reaches_laying_eligibility_while_growing_and_a_hungry_one_does_not() {
    let config = breeder(1.0, 0.8);
    let core = config.founder(SHREDDER).core;
    let laying = core.birth_body / core.body_max;
    let when = |litter: f64| {
        let (mut fauna, world, mut flora, id) = one(config.clone(), laying, 0.05, litter, true);
        let t = step_until(&mut fauna, &world, &mut flora, 800, |fa| {
            fa.view().animal(id).is_some_and(|a| a.reproduction.eligible_now)
        });
        assert_closes(&fauna, "after the eligibility run");
        t.map(|t| (t, animal(&fauna, id).body))
    };
    let (_, body) = when(1.0).expect("a fed body at laying size becomes eligible");
    assert!(body < core.body_max, "eligible while still growing: body {body} of {}", core.body_max);
    assert_eq!(when(0.0), None, "a hungry one does not");
}

/// **The brood does not sate**: a body whose reserve is all brood is as hungry as an
/// empty one; the same reserve with no brood reads as fed.
#[test]
fn hunger_counts_only_the_reserve_above_the_brood() {
    let config = breeder(1.0, 0.8);
    let core = config.founder(SHREDDER).core;
    let (fauna, _, _, id) = one(config, 1.0, 0.9, 0.0, false);
    let mut a = animal(&fauna, id);
    let cap = core.reserve_of(a.body);
    assert!((a.reserve - 0.9 * cap).abs() < 1e-12, "fixture");
    a.reproduction.brood = 0.0;
    let fed = super::hunger(&a, &core);
    a.reproduction.brood = a.reserve;
    let saving = super::hunger(&a, &core);
    assert!((fed - (1.0 - 0.9)).abs() < 1e-12, "no brood: hunger is the reserve's");
    assert!((saving - 1.0).abs() < 1e-12, "all brood: as hungry as an empty reserve");
    a.reproduction.brood = 0.5 * a.reserve;
    assert!((super::hunger(&a, &core) - (1.0 - 0.45)).abs() < 1e-12, "half brood: the other half sates");
}

// ---------------------------------------------------------- the full clutch it can hold

/// **A full clutch is what the body can carry**: with `clutch_max` 3 and a wait, a body
/// whose whole reserve cap pays for one egg lays it as soon as its hold is met instead of
/// waiting out the hold for three it can never afford; a body whose cap pays for three but
/// whose reserve pays for one still waits.
#[test]
fn a_full_clutch_wait_waits_for_what_the_reserve_cap_can_pay_for() {
    let wait = 5u32;
    let mut config = breeder(1.0, 0.0);
    {
        let p = &mut config.founders[SHREDDER.index()];
        p.core.reproduction.clutch_hold_max_s = f64::from(wait) * DT;
        p.core.reproduction.surplus_hold_s = DT;
        p.gut.s0 = 0.0;
    }
    let core = config.founder(SHREDDER).core;
    let r = core.reproduction;

    // A cap at the adult's size that pays for 1.5 eggs above the floor.
    let mut small = config.clone();
    small.founders[SHREDDER.index()].core.reserve_cap = (r.surplus_floor + 1.5 * r.egg_organic) / core.body_max;
    let sc = small.founder(SHREDDER).core;
    assert_eq!(r.full_clutch(sc.reserve_of(core.body_max)), 1, "fixture: the cap pays for one");
    let (mut fauna, world, mut flora, _) = one(small, 1.0, 1.0, 1.0, false);
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(fauna.clutches.len(), 1, "it lays its one egg at once");
    assert_eq!(fauna.clutches[0].count, 1);
    assert_closes(&fauna, "after the small layer's clutch");

    // A cap that pays for three, a reserve that pays for one: it waits, then lays one.
    let mut big = config;
    big.founders[SHREDDER.index()].core.reserve_cap = (r.surplus_floor + 3.5 * r.egg_organic) / core.body_max;
    let cap = big.founder(SHREDDER).core.reserve_of(core.body_max);
    assert_eq!(r.full_clutch(cap), 3, "fixture: the cap pays for three");
    let fraction = (r.surplus_floor + 1.5 * r.egg_organic) / cap;
    let (mut fauna, world, mut flora, _) = one(big, 1.0, fraction, 1.0, false);
    fauna.step_with(&world, &mut flora, 1);
    assert!(fauna.clutches.is_empty(), "it waits for the clutch its cap can hold");
    for _ in 0..wait + 1 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert_eq!(fauna.clutches.len(), 1, "and lays what it has at the end of the wait");
    assert_eq!(fauna.clutches[0].count, 1);
    assert_closes(&fauna, "after the big layer's clutch");
}
