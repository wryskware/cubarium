//! Reproduction: the frondgrazer's gestation escrow and the littershredder's eggs
//! (`design/handoffs/voxel-reproduction-2026-09-21.md`), one short function test per
//! clause of the two rules.
//!
//! **Every case runs on its own reproduction table, and says so.** The shipped
//! placeholders are minutes long — a two-minute surplus hold, a three-minute gestation,
//! a five-minute interval, a five-minute incubation — and a test of the *rule* on those
//! numbers would be a twenty-thousand-tick test of the same arithmetic. Each case here
//! sets the hold, gestation, interval or incubation it needs in its own
//! [`FaunaConfig`], keeps every case inside the round's 200-tick bar, and asserts what
//! the rule says rather than what the placeholder happens to be. Nothing here is read
//! back as a placeholder.
//!
//! Two cases also raise the **blind founder's `reserve_cap`** from the frozen 0.5. That
//! is not tuning either: the shipped blind body cannot hold two eggs' worth of reserve
//! at once (`Reproduction::EGGS_PLACEHOLDER`'s own note), and a clutch of one could not
//! show that a clutch divides. The rule is written for any count and these cases are
//! what proves it.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_fauna::{
    BirthMode, BlindForager, BrowserForager, Command as FaunaCommand, Controller, Fauna,
    FaunaConfig, Founder, Reproduction, SCHEMA, StartingStores, StartingStores as Stores,
};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
};

// ------------------------------------------------------------------- fixtures

/// A flat plain at 1 m voxels: every column solid to `top`, so every support face is
/// `top` in open sky. `round5c.rs`'s fixture, with its water left out.
fn plain(width: u32, top: u32, seed: u64) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed,
        ..VoxelConfig::default()
    });
    for x in 0..width as i64 {
        for y in 1..=top {
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Soil,
            });
        }
    }
    w
}

fn at(x: u32, y: u32) -> Site {
    Site { x, y, z: 0 }
}

/// One whole voxel of free water on the support face of column `x`: a metre of it, far
/// past every `drown_depth_m` in this layer. `death_cause.rs`'s `flood`.
fn flood(w: &mut World, x: i64, top: u32) {
    let volume = w.config().voxel_volume();
    let got = w.apply(WorldCommand::AddWater {
        x,
        y: top + 1,
        z: 0,
        volume_m3: volume,
    });
    assert!(got > 0.0, "the void above the face took water");
}

/// Litter on a site, at the composition the arenas deposit it at: a 0.02 mineral
/// fraction and the litter's own energy density of 2.0.
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

fn turf(flora: &mut Flora, world: &World, x: i64) {
    let wood = 0.5 * flora.config().springturf.wood_max;
    assert!(flora.apply(
        world,
        FloraCommand::Seed {
            x,
            z: 0,
            species: Plant::Springturf,
            wood
        }
    ));
}

/// A config whose founders run the reproduction tables this case needs.
fn config(edit: impl FnOnce(&mut FaunaConfig)) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    edit(&mut c);
    c
}

/// A gestation of `hold` ticks of surplus and `gestation` ticks of instalments, with
/// `interval` ticks of refractory and the shipped loss fraction.
fn gestation(hold: u64, gestation: u64, interval: u64, floor: f64) -> Reproduction {
    Reproduction {
        surplus_floor: floor,
        surplus_hold_s: hold as f64 * DT,
        gestation_s: gestation as f64 * DT,
        birth_interval_s: interval as f64 * DT,
        ..Reproduction::LIVE_BIRTH_PLACEHOLDER
    }
}

/// A clutch of `count` eggs of `egg` organic matter each, after `hold` ticks of surplus,
/// hatching `incubation` ticks later.
fn eggs(hold: u64, count: u32, egg: f64, incubation: u64, interval: u64) -> Reproduction {
    Reproduction {
        surplus_floor: 0.0005,
        surplus_hold_s: hold as f64 * DT,
        birth_interval_s: interval as f64 * DT,
        clutch_size: count,
        egg_organic: egg,
        incubation_s: incubation as f64 * DT,
        ..Reproduction::EGGS_PLACEHOLDER
    }
}

fn browser(fauna: &mut Fauna, world: &World, x: i64, stores: StartingStores) -> u64 {
    let id = fauna.view().ledger.births;
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x,
            z: 0,
            founder: Founder::Browser,
            stores,
            heading_rad: 0.0,
        },
    ));
    id
}

fn blind(fauna: &mut Fauna, world: &World, x: i64, stores: StartingStores) -> u64 {
    let id = fauna.view().ledger.births;
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x,
            z: 0,
            founder: Founder::Blind,
            stores,
            heading_rad: 0.0,
        },
    ));
    id
}

/// The animal layer's three residuals, `round5c.rs`'s shape.
fn assert_fauna_residuals(fauna: &Fauna, when: &str) {
    let v = fauna.view();
    for (got, what) in [
        (v.organic() - v.ledger.expected_organic(), "organic"),
        (v.mineral() - v.ledger.expected_mineral(), "mineral"),
        (v.energy() - v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            got.abs() <= 1e-9 * v.organic().abs().max(1.0),
            "{when}: fauna {what} residual {got:e}"
        );
    }
}

fn assert_flora_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    for (got, what) in [
        (v.organic() - v.ledger.expected_organic(), "organic"),
        (v.mineral() - v.ledger.expected_mineral(), "mineral"),
        (v.energy() - v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            got.abs() <= 1e-9 * v.organic().abs().max(1.0),
            "{when}: flora {what} residual {got:e}"
        );
    }
}

// ------------------------------------------------------------- (a) the hold

/// **(a) No birth before the hold has elapsed, even at full stores.**
///
/// The browser founder arrives at `body_max` with a full reserve — above `birth_body`
/// and well above `birth_cost` plus this case's floor — which under the rule it
/// replaces was a birth on the first tick, every tick. Now nothing happens at all until
/// the surplus has stood for the whole hold, and then the escrow still has to be filled
/// before there is an offspring.
#[test]
fn no_birth_before_the_surplus_has_stood_for_the_hold() {
    let (hold, gest) = (20u64, 10u64);
    let world = plain(4, 2, 5);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(|c| {
        c.founders[Founder::Browser.index()].core.reproduction = gestation(hold, gest, 20, 0.005);
    }));
    let id = browser(&mut fauna, &world, 1, Stores::FULL);
    let sc = fauna.config().founder(Founder::Browser).core;
    let a = *fauna.view().animal(id).expect("introduced");
    assert!(
        a.body >= sc.birth_body && a.reserve >= sc.birth_cost + 0.005,
        "the fixture is eligible from the first tick: {a:?}"
    );

    for tick in 1..hold {
        fauna.step(&world, &mut flora);
        let a = *fauna.view().animal(id).expect("alive");
        assert_eq!(
            a.reproduction.surplus_ticks, tick,
            "the surplus is standing"
        );
        assert!(
            a.reproduction.escrow.is_none(),
            "tick {tick}: nothing is escrowed before the hold has elapsed"
        );
        assert_eq!(fauna.view().ledger.born, 0, "tick {tick}: and nothing born");
    }

    // The hold's own tick opens the escrow and pays the first instalment — and still
    // gives no offspring.
    fauna.step(&world, &mut flora);
    let a = *fauna.view().animal(id).expect("alive");
    let e = a.reproduction.escrow.expect("the gestation has begun");
    assert_eq!(e.ticks, 1, "the first instalment");
    assert_eq!(fauna.view().ledger.gestations_opened, 1);
    assert_eq!(fauna.view().ledger.born, 0, "a gestation is not a birth");

    for tick in 2..gest {
        fauna.step(&world, &mut flora);
        assert_eq!(
            fauna
                .view()
                .animal(id)
                .unwrap()
                .reproduction
                .escrow
                .unwrap()
                .ticks,
            tick,
            "instalment {tick}"
        );
        assert_eq!(fauna.view().ledger.born, 0, "still gestating");
    }
    fauna.step(&world, &mut flora);
    assert_eq!(fauna.view().ledger.born, 1, "and at term, one offspring");
    assert_eq!(fauna.view().animals.len(), 2);
    assert!(
        fauna
            .view()
            .animal(id)
            .unwrap()
            .reproduction
            .escrow
            .is_none(),
        "the escrow became the newborn"
    );
    assert_fauna_residuals(&fauna, "after a held gestation");
}

// ------------------------------------------------------- (b) the instalments

/// **(b) The instalments sum to `birth_cost` exactly, and the reserve falls by the
/// same.**
///
/// Upkeep is set to zero for this one case so that the escrow is the *only* thing
/// moving the reserve: the claim is about what the instalments add up to, and a
/// simultaneous respiration would only be arithmetic in the way of it.
#[test]
fn the_escrow_instalments_sum_to_the_birth_cost_and_the_reserve_falls_by_the_same() {
    let gest = 7u64;
    let world = plain(4, 2, 5);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(|c| {
        let core = &mut c.founders[Founder::Browser.index()].core;
        core.maintenance_per_s = 0.0;
        core.reproduction = gestation(1, gest, 20, 0.005);
    }));
    let id = browser(&mut fauna, &world, 1, Stores::FULL);
    let sc = fauna.config().founder(Founder::Browser).core;
    let before = *fauna.view().animal(id).expect("introduced");

    // Every tick but the last: one equal instalment, and the escrow standing at the
    // running total of them.
    let each = sc.birth_cost / gest as f64;
    let mut held = 0.0;
    for tick in 1..gest {
        fauna.step(&world, &mut flora);
        let e = fauna
            .view()
            .animal(id)
            .expect("alive")
            .reproduction
            .escrow
            .expect("still gestating");
        assert!(
            (e.organic - held - each).abs() <= 1e-15 * sc.birth_cost,
            "tick {tick} paid {}, not the instalment {each}",
            e.organic - held
        );
        assert!(
            (e.organic - sc.birth_cost * tick as f64 / gest as f64).abs() <= 1e-15 * sc.birth_cost,
            "and the escrow stands at {} after {tick} of {gest}",
            e.organic
        );
        held = e.organic;
    }
    // The last instalment closes the package, whatever the division left.
    fauna.step(&world, &mut flora);
    assert_eq!(fauna.view().ledger.born, 1, "one offspring at term");
    let after = *fauna.view().animal(id).expect("alive");
    assert!(
        (before.reserve - after.reserve - sc.birth_cost).abs() <= 1e-15 * sc.birth_cost,
        "the reserve fell by {}, not {}",
        before.reserve - after.reserve,
        sc.birth_cost
    );
    let newborn = *fauna
        .view()
        .animals
        .iter()
        .find(|a| a.id != id)
        .expect("a newborn");
    assert_eq!(newborn.body, sc.body_min, "the newborn is a body_min body");
    assert!(
        (newborn.organic() - sc.birth_cost).abs() <= 1e-15 * sc.birth_cost,
        "and holds the whole parcel: {}",
        newborn.organic()
    );
    assert!(
        (newborn.mineral + after.mineral - before.mineral).abs() <= 1e-18,
        "the mineral moved and none of it went anywhere else"
    );
    assert_eq!(
        fauna.view().ledger.respired_out,
        0.0,
        "with no upkeep, a gestation respires nothing on its own"
    );
    assert_fauna_residuals(&fauna, "after a paid gestation");
}

// ----------------------------------------------------- (c) the failed gestation

/// **(c) A reserve driven to zero mid-gestation loses exactly the loss fraction and
/// returns the rest.**
///
/// The parent is given an upkeep that outruns its reserve — 1.4 /s against the frozen
/// 0.001 — so that on the sixth tick maintenance empties the reserve and the instalment
/// cannot be paid. The escrow is five instalments deep at that point; a quarter of its
/// organic matter is respired with the energy that was in it, and the rest comes back,
/// **all** of the escrow's mineral included (the gestation loss sheds none; the parent's
/// own upkeep sheds its share to the gut).
#[test]
fn a_reserve_that_cannot_pay_loses_the_loss_fraction_and_returns_the_rest() {
    let gest = 10u64;
    let world = plain(4, 2, 5);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(|c| {
        let core = &mut c.founders[Founder::Browser.index()].core;
        core.maintenance_per_s = 1.4;
        core.reproduction = gestation(1, gest, 20, 0.005);
    }));
    let id = browser(&mut fauna, &world, 1, Stores::FULL);
    let sc = fauna.config().founder(Founder::Browser).core;
    let loss = sc.reproduction.gestation_loss_fraction;

    // Step until the escrow disappears without a birth, remembering the state the last
    // tick left, which is the state the failure acts on: nothing is part-paid, so the
    // escrow at failure is the escrow the previous tick ended with.
    let mut prev = *fauna.view().animal(id).expect("introduced");
    let mut ledger = *fauna.view().ledger;
    let mut failed_at = None;
    for tick in 1..=gest {
        fauna.step(&world, &mut flora);
        let now = *fauna.view().animal(id).expect("alive");
        if prev.reproduction.escrow.is_some() && now.reproduction.escrow.is_none() {
            failed_at = Some(tick);
            break;
        }
        prev = now;
        ledger = *fauna.view().ledger;
    }
    let tick = failed_at.expect("the reserve ran out before term");
    assert_eq!(fauna.view().ledger.born, 0, "no offspring came of it");
    assert_eq!(fauna.view().ledger.gestations_failed, 1);
    assert_eq!(fauna.view().ledger.gestations_opened, 1);

    let e = prev.reproduction.escrow.expect("the escrow at failure");
    assert!(
        e.organic > 0.0 && e.ticks + 1 < gest,
        "it failed mid-gestation on tick {tick}"
    );
    let now = *fauna.view().animal(id).expect("alive");
    let l = fauna.view().ledger;

    // The loss: exactly the fraction, booked to the gestation split and nowhere else.
    let respired = l.respired_gestation_out - ledger.respired_gestation_out;
    assert!(
        (respired - loss * e.organic).abs() <= 1e-15 * e.organic,
        "it respired {respired}, not {} of {}",
        loss,
        e.organic
    );
    assert_eq!(
        l.respired_gestation_out, respired,
        "and this is the only failed gestation in the run"
    );

    // The return: the reserve is what the tick's maintenance left plus the rest of the
    // escrow, the mineral comes back whole, and the energy comes back less the share
    // that left with the respired organic matter.
    let upkeep = sc.maintenance_per_s * prev.body * DT;
    let from_reserve = upkeep.min(prev.reserve);
    let maintenance_heat = if prev.organic() > 0.0 {
        prev.energy * (upkeep.min(prev.organic()) / prev.organic())
    } else {
        0.0
    };
    let expected_reserve = prev.reserve - from_reserve + (1.0 - loss) * e.organic;
    assert!(
        (now.reserve - expected_reserve).abs() <= 1e-15 * expected_reserve.max(1e-6),
        "the reserve came back to {} and not {expected_reserve}",
        now.reserve
    );
    // The tick's upkeep sheds the mineral its burned organic matter held to the gut, by
    // the same fraction as its heat; the escrow's lost share sheds its mineral to the gut
    // the same way, and the rest of the escrow's mineral comes back.
    let maintenance_shed = if prev.organic() > 0.0 {
        prev.mineral * (upkeep.min(prev.organic()) / prev.organic())
    } else {
        0.0
    };
    let escrow_shed = loss * e.mineral;
    let expected_mineral = prev.mineral - maintenance_shed + e.mineral - escrow_shed;
    assert!(
        (now.mineral - expected_mineral).abs() <= 1e-15 * expected_mineral,
        "the escrow's unlost mineral came back: {} against {expected_mineral}",
        now.mineral,
    );
    assert!(
        (now.gut.mineral - (prev.gut.mineral + maintenance_shed + escrow_shed)).abs()
            <= 1e-15 * now.gut.mineral.max(1e-12),
        "the upkeep's and the lost escrow's shed mineral is in the gut: {} against {}",
        now.gut.mineral,
        prev.gut.mineral + maintenance_shed + escrow_shed
    );
    let expected_energy = prev.energy - maintenance_heat + (1.0 - loss) * e.energy;
    assert!(
        (now.energy - expected_energy).abs() <= 1e-12 * expected_energy,
        "the energy came back to {} and not {expected_energy}",
        now.energy
    );
    assert_fauna_residuals(&fauna, "after a failed gestation");
}

// ------------------------------------------------------- (d) the interval

/// **(d) After a birth, no second birth inside the interval.**
///
/// The parent keeps enough reserve for another offspring the moment the first is born —
/// under the rule this replaces it would have paid again on the very next tick — and
/// pays for nothing until the refractory has run out and a fresh surplus has stood.
#[test]
fn no_second_birth_inside_the_interval() {
    let interval = 20u64;
    let world = plain(4, 2, 5);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(|c| {
        c.founders[Founder::Browser.index()].core.reproduction = gestation(1, 1, interval, 0.0);
    }));
    let id = browser(&mut fauna, &world, 1, Stores::FULL);
    let sc = fauna.config().founder(Founder::Browser).core;

    fauna.step(&world, &mut flora);
    assert_eq!(fauna.view().ledger.born, 1, "the first offspring");
    let a = *fauna.view().animal(id).expect("alive");
    assert!(
        a.reserve >= sc.birth_cost,
        "and it could pay for another this instant: {} of {}",
        a.reserve,
        sc.birth_cost
    );
    assert_eq!(a.reproduction.refractory_ticks, interval);

    for tick in 1..=interval {
        fauna.step(&world, &mut flora);
        assert_eq!(
            fauna.view().ledger.born,
            1,
            "tick {tick} of the interval bought nothing"
        );
        assert_eq!(
            fauna.view().animal(id).unwrap().reproduction.surplus_ticks,
            0,
            "tick {tick}: no surplus accumulates inside the interval"
        );
    }
    fauna.step(&world, &mut flora);
    assert_eq!(
        fauna.view().ledger.born,
        2,
        "and the first tick past it pays again"
    );
    assert_fauna_residuals(&fauna, "after two births an interval apart");
}

// ----------------------------------------------------------- (e) the clutch

/// **(e) A clutch's currencies are exactly what left the parent's reserve** — and a
/// laying with no litter under it costs nothing at all.
#[test]
fn a_clutch_costs_the_parent_exactly_what_it_holds() {
    let (count, egg) = (2u32, 0.0035);
    let world = plain(4, 2, 5);
    let mut flora = Flora::new(FloraConfig::default());
    litter(&mut flora, at(1, 2), 0.2);
    let mut fauna = Fauna::new(config(|c| {
        let core = &mut c.founders[Founder::Blind.index()].core;
        // The frozen blind body cannot hold two eggs' worth of reserve; this case is
        // about how a clutch is paid for, so it gets a body that can.
        core.reserve_cap = 1.0;
        core.reproduction = eggs(1, count, egg, 10, 20);
    }));
    let layer = blind(&mut fauna, &world, 1, Stores::FULL);
    // A second body on bare soil: the same physiology, the same surplus, no litter.
    let dry = blind(&mut fauna, &world, 3, Stores::FULL);
    let before = *fauna.view().animal(layer).expect("introduced");
    let dry_before = *fauna.view().animal(dry).expect("introduced");
    let sc = fauna.config().founder(Founder::Blind).core;
    let cost = f64::from(count) * egg;

    fauna.step(&world, &mut flora);

    let v = fauna.view();
    assert_eq!(v.clutches.len(), 1, "one clutch, on the litter");
    let c = v.clutches[0];
    assert_eq!(c.site, at(1, 2));
    assert_eq!(c.count, count);
    assert_eq!(c.lineage, Founder::Blind);
    assert_eq!(c.laid_tick, 1);
    assert!(
        (c.organic - cost).abs() <= 1e-18,
        "the clutch holds {}",
        c.organic
    );
    assert_eq!(v.eggs_by_founder(Founder::Blind), u64::from(count));
    assert_eq!(v.ledger.clutches_laid, 1);
    assert_eq!(v.ledger.eggs_laid, u64::from(count));
    assert_eq!(v.ledger.born, 0, "an egg is not a body yet");

    // What the parent lost is what the clutch holds, once the tick's own upkeep is off
    // the books: organic, mineral and energy, by the same fraction rule a birth uses.
    let a = *v.animal(layer).expect("alive");
    let upkeep = sc.maintenance_per_s * before.body * DT;
    assert!(
        (before.reserve - a.reserve - upkeep - cost).abs() <= 1e-15 * cost,
        "the reserve fell by {}, not {}",
        before.reserve - a.reserve,
        upkeep + cost
    );
    // The upkeep sheds the mineral its burned organic matter held to the gut, by the
    // same fraction as its heat; the rest of the fall is the clutch's.
    let shed = before.mineral * (upkeep / before.organic());
    assert!(
        (before.mineral - a.mineral - shed - c.mineral).abs() <= 1e-15 * before.mineral,
        "the clutch's mineral is the parent's"
    );
    assert!(
        (a.gut.mineral - shed).abs() <= 1e-15 * before.mineral,
        "and the upkeep's share went to the gut: {} against {shed}",
        a.gut.mineral
    );
    let heat = before.energy * (upkeep / before.organic());
    assert!(
        (before.energy - a.energy - heat - c.energy).abs() <= 1e-15 * before.energy,
        "and its energy too, less the upkeep's own heat"
    );
    assert_eq!(a.reproduction.refractory_ticks, 20, "then the interval");

    // The body on bare soil paid nothing and kept its surplus standing.
    let d = *v.animal(dry).expect("alive");
    let dry_upkeep = sc.maintenance_per_s * dry_before.body * DT;
    assert!(
        (dry_before.reserve - d.reserve - dry_upkeep).abs() <= 1e-18,
        "a laying with nowhere to go costs nothing: {} against {dry_upkeep}",
        dry_before.reserve - d.reserve
    );
    assert_eq!(d.reproduction.refractory_ticks, 0, "and owes no interval");
    assert_eq!(d.reproduction.surplus_ticks, 1, "its surplus still stands");
    assert_fauna_residuals(&fauna, "after a clutch was laid");
}

// --------------------------------------------------------- (f) the hatching

/// **(f) A clutch hatches into `clutch_size` bodies whose packages sum to the clutch.**
#[test]
fn a_clutch_hatches_into_bodies_whose_packages_sum_to_it() {
    let (count, egg, incubation) = (2u32, 0.0035, 10u64);
    let world = plain(4, 2, 5);
    let mut flora = Flora::new(FloraConfig::default());
    litter(&mut flora, at(1, 2), 0.2);
    let mut fauna = Fauna::new(config(|c| {
        let core = &mut c.founders[Founder::Blind.index()].core;
        core.reserve_cap = 1.0;
        core.reproduction = eggs(1, count, egg, incubation, 200);
    }));
    let parent = blind(&mut fauna, &world, 1, Stores::FULL);
    let sc = fauna.config().founder(Founder::Blind).core;

    fauna.step(&world, &mut flora);
    let c = fauna.view().clutches[0];

    for tick in 1..incubation {
        fauna.step(&world, &mut flora);
        assert_eq!(
            fauna.view().clutches.len(),
            1,
            "tick {tick}: the clutch is still incubating"
        );
        assert_eq!(fauna.view().ledger.hatched, 0);
    }
    fauna.step(&world, &mut flora);

    let v = fauna.view();
    assert!(v.clutches.is_empty(), "the clutch hatched together");
    assert_eq!(v.ledger.hatched, u64::from(count));
    assert_eq!(v.ledger.born, u64::from(count), "hatchlings are born here");
    let young: Vec<_> = v.animals.iter().filter(|a| a.id != parent).collect();
    assert_eq!(young.len(), count as usize);
    for a in &young {
        assert_eq!(a.founder, Some(Founder::Blind), "each carries the lineage");
        assert_eq!(a.site, c.site, "and hatched where the clutch lay");
        assert_eq!(a.body, sc.body_min, "a body_min juvenile");
        assert!(a.reserve > 0.0, "with the egg's remainder as its reserve");
        assert_eq!(a.age_ticks, 0);
    }
    let organic: f64 = young.iter().map(|a| a.organic()).sum();
    let mineral: f64 = young.iter().map(|a| a.mineral).sum();
    let energy: f64 = young.iter().map(|a| a.energy).sum();
    assert!(
        (organic - c.organic).abs() <= 1e-18,
        "{organic} of {}",
        c.organic
    );
    assert!(
        (mineral - c.mineral).abs() <= 1e-18,
        "{mineral} of {}",
        c.mineral
    );
    assert!(
        (energy - c.energy).abs() <= 1e-18,
        "{energy} of {}",
        c.energy
    );
    assert_fauna_residuals(&fauna, "after a clutch hatched");
}

// ------------------------------------------------------------ (g) the loss

/// **(g) A clutch under deep water becomes carrion of the same currencies**, where it
/// lay, through the deposit path a death uses.
///
/// The parent is taken out of the world before the flood so that the only thing the
/// plant layer receives this tick is the clutch: a drowned parent would have left a
/// corpse of its own on the same site and the two deposits could not be told apart.
#[test]
fn a_clutch_under_deep_water_becomes_carrion_of_the_same_currencies() {
    let world_seed = 5;
    let mut world = plain(4, 2, world_seed);
    let mut flora = Flora::new(FloraConfig::default());
    litter(&mut flora, at(1, 2), 0.2);
    let mut fauna = Fauna::new(config(|c| {
        let core = &mut c.founders[Founder::Blind.index()].core;
        core.reserve_cap = 1.0;
        core.reproduction = eggs(1, 2, 0.0035, 40, 20);
    }));
    blind(&mut fauna, &world, 1, Stores::FULL);

    fauna.step(&world, &mut flora);
    let c = fauna.view().clutches[0];
    assert!(fauna.apply(&world, FaunaCommand::Remove { x: 1, z: 0 }));
    assert!(fauna.view().animals.is_empty(), "the parent is gone");
    assert!(
        fauna.view().clutches.len() == 1,
        "and the clutch outlives it"
    );

    let deposited = *fauna.view().ledger;
    let ground = flora
        .view()
        .ground_at(c.site)
        .map(|g| g.carrion)
        .unwrap_or(0.0);
    flood(&mut world, 1, 2);
    fauna.step(&world, &mut flora);

    let v = fauna.view();
    assert!(v.clutches.is_empty(), "the flooded clutch is gone");
    assert_eq!(v.ledger.eggs_lost, 2);
    assert_eq!(v.ledger.hatched, 0, "it never hatched");
    for (got, want, what) in [
        (
            v.ledger.deposited_organic_out - deposited.deposited_organic_out,
            c.organic,
            "organic",
        ),
        (
            v.ledger.deposited_mineral_out - deposited.deposited_mineral_out,
            c.mineral,
            "mineral",
        ),
        (
            v.ledger.deposited_energy_out - deposited.deposited_energy_out,
            c.energy,
            "energy",
        ),
    ] {
        assert!(
            (got - want).abs() <= 1e-18,
            "the carrion's {what} is {got}, not the clutch's {want}"
        );
    }
    let now = flora.view().ground_at(c.site).expect("the site").carrion;
    assert!(
        now > ground,
        "and it went onto the plant layer's carrion pool: {ground} -> {now}"
    );
    assert_fauna_residuals(&fauna, "after a clutch was lost");
    assert_flora_residuals(&flora, "after a clutch was lost");
}

// -------------------------------------------------------- (h) the snapshot

/// **(h) The snapshot carries the escrow, the counters and the clutches, and an older
/// schema is refused.**
///
/// A live layer, saved mid-gestation with a clutch standing and both counters in play:
/// everything comes back as it went in. The refusal is checked by rewriting the leading
/// schema tag, which is one postcard varint byte — no hash is pinned and no old bytes
/// are kept in the tree.
#[test]
fn the_snapshot_carries_the_escrow_the_counters_and_the_clutches_and_refuses_an_older_world() {
    let world = plain(6, 2, 5);
    let mut flora = Flora::new(FloraConfig::default());
    litter(&mut flora, at(1, 2), 0.2);
    let mut fauna = Fauna::new(config(|c| {
        let b = &mut c.founders[Founder::Blind.index()].core;
        b.reserve_cap = 1.0;
        b.reproduction = eggs(1, 2, 0.0035, 400, 400);
        c.founders[Founder::Browser.index()].core.reproduction = gestation(1, 400, 400, 0.005);
    }));
    let layer = blind(&mut fauna, &world, 1, Stores::FULL);
    let gestating = browser(&mut fauna, &world, 4, Stores::FULL);

    for _ in 0..8 {
        fauna.step(&world, &mut flora);
    }
    let a = *fauna.view().animal(gestating).expect("alive");
    let e = a.reproduction.escrow.expect("mid-gestation");
    assert!(e.ticks > 1 && e.organic > 0.0, "a real escrow: {e:?}");
    assert_eq!(fauna.view().clutches.len(), 1, "a real clutch");
    assert!(
        fauna
            .view()
            .animal(layer)
            .unwrap()
            .reproduction
            .refractory_ticks
            > 0,
        "and a real refractory counter"
    );

    let bytes = fauna.save();
    let back = Fauna::load(&bytes).expect("the layer round-trips");
    assert_eq!(back.view().animals, fauna.view().animals);
    assert_eq!(back.view().clutches, fauna.view().clutches);
    assert_eq!(back.view().ledger, fauna.view().ledger);
    assert_eq!(back.view().tick, fauna.view().tick);
    assert_eq!(
        back.view().animal(gestating).unwrap().reproduction.escrow,
        Some(e),
        "the escrow itself came back"
    );

    let mut older = bytes.clone();
    assert_eq!(older[0], SCHEMA as u8, "the tag is the leading varint byte");
    older[0] = SCHEMA as u8 - 1;
    let err = format!("{:#}", Fauna::load(&older).expect_err("an older world"));
    assert!(err.contains("start a fresh world"), "{err}");
    assert!(err.contains(&format!("is not {SCHEMA}")), "{err}");
}

// ------------------------------------------------------- (i) the two ledgers

/// **(i) The closed-ledger habitat still holds over 200 ticks with both rules active.**
///
/// A small coupled habitat — soil, litter, turf, both lineages driven by their own
/// heuristics so that they really eat — run for 200 ticks on reproduction tables short
/// enough that every event in the round happens inside them: gestations opened, births,
/// clutches laid, eggs hatched. Both layers' three residuals stay at float noise, which
/// is the whole claim: none of the new packages creates or destroys matter.
#[test]
fn both_rules_close_the_two_ledgers_over_two_hundred_coupled_ticks() {
    let mut world = plain(16, 2, 11);
    let mut flora = Flora::new(FloraConfig::default());
    for x in [2, 5, 8, 11] {
        turf(&mut flora, &world, x);
    }
    for x in [1, 4, 7, 10, 13] {
        litter(&mut flora, at(x, 2), 0.3);
    }
    let mut fauna = Fauna::new(config(|c| {
        let b = &mut c.founders[Founder::Blind.index()].core;
        b.reserve_cap = 1.0;
        b.reproduction = eggs(5, 2, 0.0035, 40, 30);
        c.founders[Founder::Browser.index()].core.reproduction = gestation(5, 20, 30, 0.002);
    }));
    fauna.set_founder_factory(
        Founder::Blind,
        std::sync::Arc::new(|| -> Box<dyn Controller> { Box::new(BlindForager::new()) }),
    );
    fauna.set_founder_factory(
        Founder::Browser,
        std::sync::Arc::new(|| -> Box<dyn Controller> { Box::new(BrowserForager::new()) }),
    );
    for x in [1, 7, 13] {
        let id = blind(&mut fauna, &world, x, Stores::FULL);
        assert!(fauna.install_founder_controller(id, Founder::Blind));
    }
    for x in [3, 9] {
        let id = browser(&mut fauna, &world, x, Stores::FULL);
        assert!(fauna.install_founder_controller(id, Founder::Browser));
    }

    for tick in 1..=200 {
        world.step();
        flora.step(&mut world);
        fauna.step(&world, &mut flora);
        assert_fauna_residuals(&fauna, &format!("tick {tick}"));
    }
    assert_flora_residuals(&flora, "after 200 coupled ticks");

    let l = fauna.view().ledger;
    assert!(l.gestations_opened > 0, "the browsers gestated");
    assert!(
        l.born > l.hatched,
        "and gave birth as well as hatched: {l:?}"
    );
    assert!(l.clutches_laid > 0, "the shredders laid");
    assert!(l.hatched > 0, "and their eggs hatched: {l:?}");
    assert_eq!(
        l.born,
        l.births - l.introduced,
        "every body here was introduced or born of a parent's reserve"
    );
    assert!(
        (l.respired_maintenance_out
            + l.respired_motor_out
            + l.respired_digestion_out
            + l.respired_gestation_out
            - l.respired_out)
            .abs()
            <= 1e-12 * l.respired_out.max(1e-12),
        "the four splits sum to the total, to float noise: {l:?}"
    );
}

// --------------------------------------------------------- (j) the arena path

/// **(j) `set_births_enabled(false)` suppresses eligibility, gestation and laying.**
///
/// The arena's contract, unchanged: bodies that would be eligible on the first tick
/// accumulate no surplus, open no escrow, lay nothing, and the reproduction counters
/// stay at zero — and turning the switch back on starts the rule from nothing, exactly
/// as a fresh episode expects.
#[test]
fn disabled_births_suppress_eligibility_gestation_and_laying() {
    let world = plain(6, 2, 5);
    let mut flora = Flora::new(FloraConfig::default());
    litter(&mut flora, at(1, 2), 0.2);
    let mut fauna = Fauna::new(config(|c| {
        let b = &mut c.founders[Founder::Blind.index()].core;
        b.reserve_cap = 1.0;
        b.reproduction = eggs(1, 2, 0.0035, 5, 5);
        c.founders[Founder::Browser.index()].core.reproduction = gestation(1, 1, 1, 0.0);
    }));
    fauna.set_births_enabled(false);
    let layer = blind(&mut fauna, &world, 1, Stores::FULL);
    let gestator = browser(&mut fauna, &world, 4, Stores::FULL);

    for _ in 0..60 {
        fauna.step(&world, &mut flora);
    }
    let v = fauna.view();
    assert!(v.clutches.is_empty(), "nothing was laid");
    assert_eq!(v.ledger.born, 0, "and nothing was born");
    assert_eq!(v.ledger.gestations_opened, 0);
    assert_eq!(v.ledger.clutches_laid, 0);
    assert_eq!(v.ledger.eggs_laid, 0);
    assert_eq!(v.ledger.hatched, 0);
    assert_eq!(v.ledger.respired_gestation_out, 0.0);
    for id in [layer, gestator] {
        let a = *v.animal(id).expect("alive");
        assert_eq!(
            a.reproduction,
            Default::default(),
            "no eligibility accumulated at all: {a:?}"
        );
    }
    assert_eq!(
        fauna
            .config()
            .founder(Founder::Blind)
            .core
            .reproduction
            .mode,
        BirthMode::Eggs,
        "the rule is configured; the switch is what is off"
    );
    assert_fauna_residuals(&fauna, "with births disabled");

    fauna.set_births_enabled(true);
    fauna.step(&world, &mut flora);
    fauna.step(&world, &mut flora);
    assert_eq!(
        fauna.view().ledger.born,
        1,
        "and it starts when switched on"
    );
    assert_eq!(fauna.view().clutches.len(), 1);
    assert_fauna_residuals(&fauna, "after the switch went back on");
}
