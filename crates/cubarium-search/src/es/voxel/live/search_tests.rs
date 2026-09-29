//! The ecology search's tests (`design/handoffs/voxel-ecology-search-2026-09-24.md`,
//! "Tests"): each at most 200 ticks, no pinned hashes.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Command as FaunaCommand, Controller, Fauna, FaunaConfig, Founder, StartingStores,
};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site, Species};

use super::ecology::{EcologyParams, PARAMS};
use super::search::{Band, Trace, penalty};
use crate::es::cma::Cma;
use crate::es::voxel::controller::StationaryFeeding;

fn flat_world() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 6,
        depth: 6,
        voxel_m: 0.25,
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    world
}

fn with(name: &str, v: f64) -> EcologyParams {
    let mut e = EcologyParams::defaults();
    e.set(name, v).expect("in range");
    e
}

fn base_flora() -> FloraConfig {
    FloraConfig::for_voxel_size(0.25)
}

/// Flat soil watered to field capacity and let drain, so a stand neither wilts nor
/// drowns: a little water per column, stepped until it has soaked in.
fn watered_world() -> World {
    let mut world = flat_world();
    for z in 0..6 {
        for x in 0..8 {
            world.apply(WorldCommand::AddWater {
                x,
                y: 3,
                z,
                volume_m3: 0.004,
            });
        }
    }
    for _ in 0..150 {
        world.step_with(1);
    }
    world
}

/// One photosynthetic stand on watered flat soil; returns its site.
fn planted(e: &EcologyParams, species: Species) -> (World, Flora, Site) {
    let world = watered_world();
    let mut flora = Flora::new(e.flora_config(base_flora()));
    let wood = flora.view().config.species(species).wood_max * 0.5;
    assert!(flora.apply(
        &world,
        cubarium_voxel_flora::Command::Seed {
            x: 3,
            z: 3,
            species,
            wood,
        }
    ));
    let site = flora.view().stands[0].site;
    (world, flora, site)
}

fn foliage(flora: &Flora) -> f64 {
    flora.view().stands.iter().map(|s| s.foliage).sum()
}

fn litter(flora: &Flora) -> f64 {
    flora.view().ground.iter().map(|g| g.litter).sum()
}

/// `flora.foliage_rate`: a stand cropped below its reflush threshold regrows from its
/// reserve faster at twice the rate. The rate caps the reflush; income-funded regrowth is
/// limited by the income. With the graze refuge on, a crop stops at 30 % of the foliage,
/// above springturf's 25 % reflush line, so this fixture takes the refuge off to reach it.
#[test]
fn foliage_rate_speeds_regrowth() {
    let regrown = |scale: f64| {
        let e = with("flora.foliage_rate", scale);
        let world = watered_world();
        let mut flora = Flora::new(e.flora_config(base_flora().without_graze_refuge()));
        let wood = flora.view().config.species(Species::Springturf).wood_max * 0.5;
        assert!(flora.apply(
            &world,
            cubarium_voxel_flora::Command::Seed {
                x: 3,
                z: 3,
                species: Species::Springturf,
                wood,
            }
        ));
        let site = flora.view().stands[0].site;
        let mut world = world;
        let before = foliage(&flora);
        flora.take_foliage(site, before * 0.9).expect("cropped");
        let cropped = foliage(&flora);
        for _ in 0..200 {
            world.step_with(1);
            flora.step(&mut world);
        }
        foliage(&flora) - cropped
    };
    let (one, two) = (regrown(1.0), regrown(2.0));
    assert!(one > 0.0, "the stand regrew at all: {one}");
    assert!(two > one, "x2 regrew {two} vs x1 {one}");
}

/// `flora.senescence`: a full stand sheds more litter at twice the rate.
#[test]
fn senescence_sheds_more_litter() {
    let shed = |scale: f64| {
        let e = with("flora.senescence", scale);
        let (mut world, mut flora, _) = planted(&e, Species::Springturf);
        for _ in 0..200 {
            world.step_with(1);
            flora.step(&mut world);
        }
        litter(&flora)
    };
    let (one, two) = (shed(1.0), shed(2.0));
    assert!(one > 0.0 && two > one, "x2 shed {two} vs x1 {one}");
}

/// `flora.decomposition`: a litter deposit on bare ground rots faster at twice the rate.
#[test]
fn decomposition_rots_litter_faster() {
    let left = |scale: f64| {
        let e = with("flora.decomposition", scale);
        let mut world = flat_world();
        let mut flora = Flora::new(e.flora_config(base_flora()));
        // A support voxel: the top soil cell.
        assert!(flora.deposit(
            Site { x: 3, y: 2, z: 3 },
            Deposit {
                kind: DepositKind::Litter,
                organic: 0.1,
                mineral: 0.002,
                energy: 0.2,
            }
        ));
        for _ in 0..200 {
            flora.step(&mut world);
        }
        litter(&flora)
    };
    let (one, two) = (left(1.0), left(2.0));
    assert!(one < 0.1, "litter decomposed at all: {one}");
    assert!(two < one, "x2 left {two} vs x1 {one}");
}

/// `founder.yield_by_food`: a shredder feeding in place on litter assimilates more of
/// the same bites at a higher yield.
#[test]
fn yield_by_food_raises_assimilation() {
    let assimilated = |scale: f64| {
        let e = with("founder.yield_by_food", scale);
        let world = flat_world();
        let mut flora = Flora::new(e.flora_config(base_flora()));
        let mut fauna = Fauna::new(e.fauna_config());
        assert!(fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder {
                x: 3,
                z: 3,
                founder: Founder::Blind,
                stores: StartingStores::HUNGRY,
                heading_rad: 0.0,
            }
        ));
        let (id, site) = {
            let a = &fauna.view().animals[0];
            (a.id, a.site)
        };
        assert!(flora.deposit(
            site,
            Deposit {
                kind: DepositKind::Litter,
                organic: 0.05,
                mineral: 0.001,
                energy: 0.1,
            }
        ));
        let feed: Box<dyn Controller> = Box::new(StationaryFeeding);
        assert!(fauna.set_controller(id, feed));
        for _ in 0..200 {
            fauna.step(&world, &mut flora);
        }
        fauna.view().ledger.assimilated_by_founder[Founder::Blind.index()]
    };
    let (one, more) = (assimilated(1.0), assimilated(1.5));
    assert!(one > 0.0, "the shredder ate: {one}");
    assert!(more > one, "x1.5 assimilated {more} vs x1 {one}");
}

/// `founder.reproduction.surplus_floor`: the reserve a body must hold to breed rises with
/// the floor, for every lineage.
#[test]
fn surplus_floor_raises_the_breeding_threshold() {
    let (one, two) = (
        with("founder.reproduction.surplus_floor", 1.0).fauna_config(),
        with("founder.reproduction.surplus_floor", 2.0).fauna_config(),
    );
    for f in Founder::ALL {
        let (a, b) = (one.founder(f).core, two.founder(f).core);
        assert!(
            b.reproduction.surplus_reserve(&b) > a.reproduction.surplus_reserve(&a),
            "{f:?}"
        );
    }
}

/// `founder.maintenance_per_s`: a body at rest respires more upkeep at twice the rate.
#[test]
fn maintenance_raises_upkeep() {
    let upkeep = |scale: f64| {
        let e = with("founder.maintenance_per_s", scale);
        let world = flat_world();
        let mut flora = Flora::new(e.flora_config(base_flora()));
        let mut fauna = Fauna::new(e.fauna_config());
        assert!(fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder {
                x: 3,
                z: 3,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            }
        ));
        let id = fauna.view().animals[0].id;
        let rest: Box<dyn Controller> = Box::new(crate::es::voxel::controller::NoIntake);
        assert!(fauna.set_controller(id, rest));
        for _ in 0..200 {
            fauna.step(&world, &mut flora);
        }
        fauna.view().ledger.respired_maintenance_out
    };
    let (one, two) = (upkeep(1.0), upkeep(2.0));
    assert!(
        one > 0.0 && (two / one - 2.0).abs() < 0.05,
        "x2 {two} vs x1 {one}"
    );
}

/// The lifespan and defence parameters reach their layers.
#[test]
fn the_lifespan_and_defence_parameters_reach_their_layers() {
    let f = with("founder.lifespan_median_s", 0.5).fauna_config();
    for l in Founder::ALL {
        assert_eq!(
            f.founder(l).core.lifespan_median_s,
            0.5 * FaunaConfig::default().founder(l).core.lifespan_median_s
        );
    }
    let fl = with("flora.defence_per_bite", 2.0).flora_config(base_flora());
    let fh = with("flora.defence_half_life_s", 0.5).flora_config(base_flora());
    for s in Species::ALL {
        assert_eq!(
            fl.species(s).defence_per_bite,
            2.0 * base_flora().species(s).defence_per_bite
        );
        assert_eq!(
            fh.species(s).defence_half_life_s,
            0.5 * base_flora().species(s).defence_half_life_s
        );
    }
}

/// The live app's defaults are unchanged: the default vector builds exactly the shipped
/// configs, a tuned vector never writes back into them, and a log vector of zeros is the
/// defaults.
#[test]
fn the_live_defaults_are_unchanged() {
    let e = EcologyParams::defaults();
    assert_eq!(e.fauna_config(), FaunaConfig::default());
    assert_eq!(e.flora_config(base_flora()), base_flora());
    let tuned = EcologyParams::from_search_log(
        &(0..EcologyParams::search_names().len())
            .map(|i| 0.3 * ((i % 5) as f64 - 2.0))
            .collect::<Vec<_>>(),
    );
    let _ = (tuned.fauna_config(), tuned.flora_config(base_flora()));
    assert_eq!(FaunaConfig::default(), e.fauna_config());
    assert_eq!(EcologyParams::from_log(&vec![0.0; PARAMS.len()]), e);
    // Out-of-range log values are clamped to the parameter's range.
    let wild = EcologyParams::from_log(&vec![9.0; PARAMS.len()]);
    for (p, v) in PARAMS.iter().zip(wild.values()) {
        assert!(*v <= p.max + 1e-12, "{} = {v}", p.name);
    }
}

fn trace_at(n: usize, moving: f64) -> Trace {
    Trace {
        horizon_min: 360.0,
        minutes: (1..=360).map(f64::from).collect(),
        animals: vec![n; 360],
        browser_moving: moving,
        missing: 0,
        silent_lineages: 0,
    }
}

/// The penalty is zero inside the band and grows monotonically outside it: fewer animals
/// under the floor, more over the ceiling, over the peak, a stiller browser, and every
/// missing lineage or species.
#[test]
fn the_penalty_is_zero_in_the_band_and_monotonic_outside() {
    let band = Band::default();
    for n in [30, 45, 80] {
        assert_eq!(penalty(&trace_at(n, 0.3), &band).total, 0.0, "n = {n}");
    }
    let p = |n: usize, m: f64| penalty(&trace_at(n, m), &band).total;
    let mut last = 0.0;
    for n in (0..30).rev() {
        let v = p(n, 0.3);
        assert!(v > last, "below the band, {n} animals: {v} after {last}");
        last = v;
    }
    let mut last = 0.0;
    for n in 81..300 {
        let v = p(n, 0.3);
        assert!(v > last, "above the band, {n} animals: {v} after {last}");
        last = v;
    }
    assert!(
        p(50, 0.2) > 0.0 && p(50, 0.1) > p(50, 0.2),
        "stiller browsers"
    );
    // A spike over the peak costs even when the second half is in the band.
    let mut spike = trace_at(50, 0.3);
    spike.animals[30] = 150;
    assert!(penalty(&spike, &band).total > 0.0);
    // Persistence dominates: a missing species outweighs any band distance.
    let mut gone = trace_at(50, 0.3);
    gone.missing = 1;
    assert!(penalty(&gone, &band).total > p(299, 0.0));
    // A lineage that stopped breeding in the second half costs as much as a lost one.
    let mut silent = trace_at(50, 0.3);
    silent.silent_lineages = 1;
    assert_eq!(penalty(&silent, &band).total, band.missing_cost);
    // A trace that stopped early carries its last count to the horizon.
    let mut short = trace_at(200, 0.3);
    short.minutes.truncate(100);
    short.animals.truncate(100);
    assert!(
        (penalty(&short, &band).bound - penalty(&trace_at(200, 0.3), &band).bound).abs() < 1e-9
    );
}

/// The trend projection: a flat trace stays flat, a doubling one keeps doubling (up to
/// the cap), and the projection reaches the scored horizon.
#[test]
fn a_short_trace_is_projected_on_its_last_hours_trend() {
    let mut flat = trace_at(50, 0.3);
    flat.minutes.truncate(180);
    flat.animals.truncate(180);
    assert_eq!(super::search::project(&mut flat, 360.0), 180.0);
    assert_eq!(flat.animals.len(), 360);
    assert!(flat.animals.iter().all(|&n| n == 50));
    let mut growing = Trace {
        horizon_min: 180.0,
        minutes: (1..=180).map(f64::from).collect(),
        animals: (1..=180)
            .map(|m| (20.0 * (f64::from(m) * 2f64.ln() / 60.0).exp()) as usize)
            .collect(),
        browser_moving: 0.3,
        missing: 0,
        silent_lineages: 0,
    };
    let end = *growing.animals.last().unwrap();
    super::search::project(&mut growing, 360.0);
    let at_240 = growing.animals[239];
    assert!(
        (at_240 as f64 / end as f64 - 2.0).abs() < 0.1,
        "one hour later it doubled again: {end} -> {at_240}"
    );
    let last = *growing.animals.last().unwrap() as f64;
    assert!(
        (last / end as f64 - 8.0).abs() < 0.5,
        "three hours on: {end} -> {last}"
    );
    let mut boom = growing.clone();
    boom.minutes.truncate(180);
    boom.animals.truncate(180);
    super::search::project(&mut boom, 900.0);
    assert_eq!(*boom.animals.last().unwrap(), 2000, "capped");
}

/// On a toy objective with a known optimum inside the box, the search's mean moves toward
/// it and its best candidate beats the start.
#[test]
fn the_search_moves_toward_a_known_optimum() {
    let target = [0.8, -0.5, 0.3, -1.0, 0.6];
    let f = |x: &[f64]| {
        x.iter()
            .zip(&target)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
    };
    let mut cma = Cma::new(vec![0.0; 5], 0.5, 8, 11);
    let start = f(cma.mean());
    for _ in 0..40 {
        let xs = cma.ask();
        let fs: Vec<f64> = xs.iter().map(|x| f(x)).collect();
        cma.tell(&xs, &fs);
    }
    let end = f(cma.mean());
    assert!(end < 0.05 * start, "mean distance² {start} -> {end}");
}
