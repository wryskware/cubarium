//! The ledger's **departure causes**: one short function test per way a body can leave
//! the world, and the invariant that ties them back to `deaths`.
//!
//! This is instrumentation and nothing else — no rule about when a body dies is touched
//! here, so every test asserts the split *of a death that already happened* and, where it
//! matters, that the totals are unchanged. Conventions are `round5c.rs`'s: `voxel_m` is
//! 1 m, the world is never stepped, and a test that needs a rate the placeholders do not
//! give sets it in its own [`FaunaConfig`] and says why.
//!
//! The 2026-09-20 census could read 65 deaths and not one cause
//! (`design/7_Research/voxel-census-2026-09-20.md`); these counters are what makes that
//! readable, and `voxel_founder_autopsy` is what reads them.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Command, Departure, Fauna, FaunaConfig, Founder, Species, StartingStores,
};
use cubarium_voxel_flora::{Flora, FloraConfig};

// ------------------------------------------------------------------- fixtures

/// A flat one-row plain, soil in `1..=top`, dry: every column's support face is `top`.
fn plain(width: u32, top: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 10,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
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

/// Free water standing on the support face of column `x`: one whole voxel of it, which at
/// `voxel_m` 1 m is 1 m of depth and far past every `drown_depth_m` in the layer.
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

/// A starving layer: 5 /s against the placeholder 0.001 /s for **every** body, species
/// and founder alike. The point of each test is the cause label, not how long a body
/// takes to starve, and 5 /s reaches it in a few dozen ticks.
fn starving_config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.frondgrazer.maintenance_per_s = 5.0;
    for f in Founder::ALL {
        c.founders[f.index()].core.maintenance_per_s = 5.0;
    }
    c
}

fn grazer(fauna: &mut Fauna, world: &World, x: i64, body: f64) -> u64 {
    let id = fauna.view().ledger.births;
    assert!(
        fauna.apply(
            world,
            Command::Introduce {
                x,
                z: 0,
                species: Species::Frondgrazer,
                body,
            }
        ),
        "a grazer at x {x}"
    );
    id
}

fn founder(fauna: &mut Fauna, world: &World, x: i64, founder: Founder) -> u64 {
    let id = fauna.view().ledger.births;
    assert!(
        fauna.apply(
            world,
            Command::IntroduceFounder {
                x,
                z: 0,
                founder,
                stores: StartingStores::HUNGRY,
                heading_rad: 0.0,
            }
        ),
        "a {} at x {x}",
        founder.name()
    );
    id
}

/// Step until every animal is gone, or give up. Returns the ticks it took.
fn step_until_empty(fauna: &mut Fauna, world: &World, flora: &mut Flora, limit: u64) -> u64 {
    let mut ticks = 0;
    while !fauna.view().animals.is_empty() && ticks < limit {
        fauna.step(world, flora);
        ticks += 1;
    }
    assert!(ticks < limit, "the bodies left inside {limit} ticks");
    ticks
}

/// Every departure counter, as one readable tuple.
fn split(fauna: &Fauna) -> (u64, u64, u64) {
    let l = fauna.view().ledger;
    (
        l.departed(Departure::Starved),
        l.departed(Departure::Drowned),
        l.departed(Departure::Removed),
    )
}

// ------------------------------------------------------------------- the causes

/// A body whose reserve ran out and whose dieback took it under `body_min` is booked
/// `Starved`, the total is unchanged, and the two death causes account for it exactly.
#[test]
fn starvation_is_booked_starved_and_the_causes_still_sum_to_deaths() {
    let world = plain(4, 2);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(starving_config());
    // Under `birth_body`: a starving adult that could still afford a birth would leave a
    // newborn to starve beside it, and this test is about one death.
    grazer(&mut fauna, &world, 1, 0.02);

    step_until_empty(&mut fauna, &world, &mut flora, 200);

    assert_eq!(fauna.view().ledger.deaths, 1);
    assert_eq!(split(&fauna), (1, 0, 0));
    assert_eq!(fauna.view().ledger.deaths_accounted(), 1);
    for f in Founder::ALL {
        for c in Departure::ALL {
            assert_eq!(
                fauna.view().ledger.departed_founder(f, c),
                0,
                "a heuristic body carries no lineage"
            );
        }
    }
}

/// A well-fed body standing under water deeper than its own `drown_depth_m` is booked
/// `Drowned` — on the first tick, with its body nowhere near `body_min`, so nothing about
/// this death could be read as starvation.
#[test]
fn drowning_is_booked_drowned() {
    let mut world = plain(4, 2);
    flood(&mut world, 1, 2);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = grazer(&mut fauna, &world, 1, 0.02);
    let body_min = fauna.config().species(Species::Frondgrazer).body_min;
    assert!(fauna.view().animal(id).unwrap().body > body_min);

    fauna.step(&world, &mut flora);

    assert!(
        fauna.view().animals.is_empty(),
        "it drowned on the first tick"
    );
    assert_eq!(fauna.view().ledger.deaths, 1);
    assert_eq!(split(&fauna), (0, 1, 0));
    assert_eq!(fauna.view().ledger.deaths_accounted(), 1);
}

/// A body that satisfies **both** clauses is booked `Starved`: the cause names the clause
/// the death rule reads first, and a counter must not invent an adjudication the rule does
/// not make.
#[test]
fn a_body_that_is_both_starved_and_drowned_is_booked_starved() {
    let mut world = plain(4, 2);
    flood(&mut world, 1, 2);
    let mut flora = Flora::new(FloraConfig::default());
    // 200 /s: one tick of upkeep is more than the whole body, so the starvation clause
    // is true on the very tick the standing water is.
    let mut cfg = FaunaConfig::default();
    cfg.frondgrazer.maintenance_per_s = 200.0;
    let mut fauna = Fauna::new(cfg);
    grazer(&mut fauna, &world, 1, 0.005);
    let body_min = fauna.config().species(Species::Frondgrazer).body_min;
    assert_eq!(body_min, 0.005, "introduced exactly at the floor");

    fauna.step(&world, &mut flora);

    assert!(fauna.view().animals.is_empty());
    assert_eq!(split(&fauna), (1, 0, 0));
}

/// The third way out is **not** a death: a `Remove` command and a support face the terrain
/// took away both book `Removed`, leave no corpse, and leave `deaths` at zero.
#[test]
fn a_remove_and_a_lost_support_face_book_a_removal_and_not_a_death() {
    let mut world = plain(4, 2);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    grazer(&mut fauna, &world, 1, 0.02);
    grazer(&mut fauna, &world, 3, 0.02);

    assert!(fauna.apply(&world, Command::Remove { x: 1, z: 0 }));
    assert_eq!(split(&fauna), (0, 0, 1));
    assert_eq!(fauna.view().ledger.deaths, 0);

    // Dig the second one's floor out from under it: step 1 of the tick takes the body.
    world.apply(WorldCommand::SetMaterial {
        x: 3,
        y: 2,
        z: 0,
        material: Material::Air,
    });
    fauna.step(&world, &mut flora);

    assert!(fauna.view().animals.is_empty());
    assert_eq!(split(&fauna), (0, 0, 2));
    assert_eq!(fauna.view().ledger.deaths, 0, "no corpse, no death");
    assert_eq!(fauna.view().ledger.deaths_accounted(), 0);
    assert_eq!(flora.view().ledger.deposited_organic_in, 0.0);
}

/// A lineage-carrying body books its cause **twice**: once in the totals and once under
/// its own founder. One littershredder starves and one frondgrazer founder drowns, so
/// neither the lineage nor the cause can be confused with the other's. Both arrive
/// [`StartingStores::HUNGRY`] — half structure, no reserve — which is under both
/// founders' `birth_body`, so nothing breeds and the two deaths are the only two.
#[test]
fn a_founder_s_departure_is_split_by_lineage_and_cause() {
    let mut world = plain(6, 2);
    flood(&mut world, 4, 2);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(starving_config());
    founder(&mut fauna, &world, 1, Founder::Blind);
    founder(&mut fauna, &world, 4, Founder::Browser);

    step_until_empty(&mut fauna, &world, &mut flora, 200);

    assert_eq!(fauna.view().ledger.deaths, 2);
    assert_eq!(split(&fauna), (1, 1, 0));
    assert_eq!(fauna.view().ledger.deaths_accounted(), 2);

    let l = fauna.view().ledger;
    assert_eq!(l.departed_founder(Founder::Blind, Departure::Starved), 1);
    assert_eq!(l.departed_founder(Founder::Blind, Departure::Drowned), 0);
    assert_eq!(l.departed_founder(Founder::Browser, Departure::Starved), 0);
    assert_eq!(l.departed_founder(Founder::Browser, Departure::Drowned), 1);
    for f in Founder::ALL {
        assert_eq!(l.departed_founder(f, Departure::Removed), 0);
    }
}
