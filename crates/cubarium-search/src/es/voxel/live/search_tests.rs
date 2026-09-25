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

/// One photosynthetic stand on the flat world; returns its site.
fn planted(e: &EcologyParams, species: Species) -> (World, Flora, Site) {
    let mut world = flat_world();
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
    // Water in the soil, so the stand can assimilate.
    for z in 0..6 {
        for x in 0..8 {
            world.apply(WorldCommand::AddWater {
                x,
                y: 3,
                z,
                volume_m3: 0.01,
            });
        }
    }
    let site = flora.view().stands[0].site;
    (world, flora, site)
}

fn foliage(flora: &Flora) -> f64 {
    flora.view().stands.iter().map(|s| s.foliage).sum()
}

fn litter(flora: &Flora) -> f64 {
    flora.view().ground.iter().map(|g| g.litter).sum()
}

/// `flora.foliage_rate`: a cropped stand regrows faster at twice the rate.
#[test]
fn foliage_rate_speeds_regrowth() {
    let regrown = |scale: f64| {
        let e = with("flora.foliage_rate", scale);
        let (mut world, mut flora, site) = planted(&e, Species::Springturf);
        let before = foliage(&flora);
        flora.take_foliage(site, before * 0.5).expect("cropped");
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
        assert!(flora.deposit(
            Site { x: 3, y: 3, z: 3 },
            Deposit {
                kind: DepositKind::Litter,
                organic: 0.1,
                mineral: 0.0,
                energy: 0.0,
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
                mineral: 0.0,
                energy: 0.0,
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

/// The live app's defaults are unchanged: the default vector builds exactly the shipped
/// configs, a tuned vector never writes back into them, and a log vector of zeros is the
/// defaults.
#[test]
fn the_live_defaults_are_unchanged() {
    let e = EcologyParams::defaults();
    assert_eq!(e.fauna_config(), FaunaConfig::default());
    assert_eq!(e.flora_config(base_flora()), base_flora());
    let tuned = EcologyParams::from_log(&[0.7, -0.4, 0.2, -0.3, 1.0]);
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
    // A trace that stopped early carries its last count to the horizon.
    let mut short = trace_at(200, 0.3);
    short.minutes.truncate(100);
    short.animals.truncate(100);
    assert!(
        (penalty(&short, &band).bound - penalty(&trace_at(200, 0.3), &band).bound).abs() < 1e-9
    );
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
