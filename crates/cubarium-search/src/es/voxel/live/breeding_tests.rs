//! The graded breeding term (stage 1's flat spot, 2026-10-01): a lineage alive with no
//! late births pays `missing_cost × (1 − min(1, readiness / ready_target))`, where
//! readiness is its living bodies' second-half mean breeding progress. Each at most 200
//! ticks; no pinned hashes.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Controller, Fauna, FaunaConfig, Founder, Response,
    StartingStores,
};
use cubarium_voxel_flora::{Flora, FloraConfig};

use super::search::{BandSpec, body_readiness, breeding_shortfall};

const FOUNDED: [usize; Founder::COUNT] = [8, 8, 9, 0, 0, 0, 0, 0, 0];
const NOT_LOST: [bool; Founder::COUNT] = [false; Founder::COUNT];

fn shortfall(late: [u64; 3], readiness: [f64; 3], spec: &BandSpec) -> f64 {
    breeding_shortfall(&FOUNDED, &late, &NOT_LOST, &readiness, spec, true)
}

/// Zero with late births; full at readiness 0; falling as readiness rises to the target,
/// and zero from there.
#[test]
fn the_term_is_graded_by_readiness() {
    let spec = BandSpec {
        persist_only: [false; Founder::COUNT],
        ..BandSpec::default()
    };
    assert_eq!(spec.ready_target, 0.5, "the default target");
    assert_eq!(
        shortfall([1, 1, 1], [0.0; 3], &spec),
        0.0,
        "late births pay nothing"
    );
    assert_eq!(
        shortfall([1, 0, 1], [0.0; 3], &spec),
        1.0,
        "readiness 0 pays in full"
    );
    let mut last = f64::INFINITY;
    for r in [0.0, 0.1, 0.2, 0.3, 0.4, 0.49] {
        let v = shortfall([1, 0, 1], [0.0, r, 0.0], &spec);
        assert!(v < last && v > 0.0, "readiness {r}: {v} after {last}");
        last = v;
    }
    assert_eq!(shortfall([1, 0, 1], [0.0, 0.5, 0.0], &spec), 0.0);
    assert_eq!(shortfall([1, 0, 1], [0.0, 0.9, 0.0], &spec), 0.0);
    // A lost lineage pays in full whatever its bodies had built up, as before.
    let lost = [false, true, false];
    assert_eq!(
        breeding_shortfall(&FOUNDED, &[1, 0, 1], &lost, &[0.0, 0.9, 0.0], &spec, true),
        1.0
    );
    // Stopped before the second half: every lineage held to breeding pays in full.
    assert_eq!(
        breeding_shortfall(&FOUNDED, &[0; 3], &NOT_LOST, &[1.0; 3], &spec, false),
        3.0
    );
}

/// A `persist_only` lineage pays no breeding term, ready or not.
#[test]
fn a_persist_only_lineage_is_unaffected() {
    let mut spec = BandSpec::default();
    spec.persist_only = [false; Founder::COUNT];
    spec.persist_only[Founder::Lanternjaw.index()] = true;
    assert_eq!(shortfall([1, 1, 0], [0.0; 3], &spec), 0.0);
    assert_eq!(shortfall([1, 1, 0], [0.0, 0.0, 0.3], &spec), 0.0);
}

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

struct Still;

impl Controller for Still {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        Response::Bounded(Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 0.0,
        })
    }
    fn reset(&mut self) {}
}

/// One browser, introduced with `stores`, stood still `ticks`; its readiness then.
fn readiness_after(cfg: FaunaConfig, stores: StartingStores, ticks: u32) -> (f64, bool) {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::for_voxel_size(0.25));
    let mut fauna = Fauna::new(cfg);
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 3,
            z: 3,
            founder: Founder::Browser,
            stores,
            heading_rad: 0.0,
        }
    ));
    let id = fauna.view().animals[0].id;
    assert!(fauna.set_controller(id, Box::new(Still)));
    for _ in 0..ticks {
        fauna.step(&world, &mut flora);
    }
    let a = fauna.view().animals[0].clone();
    let sc = cfg.founder(Founder::Browser).core;
    (body_readiness(&a, &sc), a.reproduction.escrow.is_some())
}

/// A gestating body counts 1; a full body part-way through its hold counts its progress;
/// a body below breeding size counts only its reserve part.
#[test]
fn a_gestating_body_counts_one() {
    let mut cfg = FaunaConfig::default();
    let core = &mut cfg.founders[Founder::Browser.index()].core;
    core.reproduction.surplus_hold_s = 0.05;
    core.reproduction.gestation_s = 600.0;
    let (r, gestating) = readiness_after(cfg, StartingStores::FULL, 20);
    assert!(
        gestating,
        "a full browser with a one-tick hold opened its escrow"
    );
    assert_eq!(r, 1.0);

    let mut cfg = FaunaConfig::default();
    cfg.founders[Founder::Browser.index()]
        .core
        .reproduction
        .surplus_hold_s = 100.0;
    let (r, gestating) = readiness_after(cfg, StartingStores::FULL, 100);
    assert!(!gestating);
    assert!(r > 0.0 && r < 1.0, "part-way through its hold: {r}");

    let small = StartingStores {
        body: 0.2,
        reserve: 1.0,
    };
    let (r, _) = readiness_after(FaunaConfig::default(), small, 20);
    let core = FaunaConfig::default().founder(Founder::Browser).core;
    let reserve = core.reserve_of(0.2 * core.body_max);
    let part = 0.5 * (reserve / core.reproduction.surplus_reserve(&core)).min(1.0);
    assert!(r > 0.0, "below birth_body, it still counts its reserve");
    assert!((r - part).abs() < 0.02 * part, "only its reserve part: {r} vs {part}");
}

/// Progress is half the reserve toward the surplus line and half the hold: monotonic in
/// the reserve a body holds, at the same hold.
#[test]
fn readiness_is_monotonic_in_reserve() {
    let mut last = -1.0;
    // The browser's surplus line is 0.6 of its reserve cap. Under the minimal gut's
    // upkeep (f / 7,200 s) a body placed at 0.6 no longer drops under the line in two
    // ticks, so the ladder steps from below the line (0.5) to above it (0.8).
    for reserve in [0.0, 0.2, 0.4, 0.5, 0.8] {
        let stores = StartingStores { body: 1.0, reserve };
        let (r, _) = readiness_after(FaunaConfig::default(), stores, 2);
        assert!(r > last, "reserve {reserve}: {r} after {last}");
        assert!(r <= 0.5 + 1e-9 || reserve >= 0.6, "the hold has barely begun: {r}");
        last = r;
    }
}

