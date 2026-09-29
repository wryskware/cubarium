//! Browser-movement package tests (`design/handoffs/voxel-browser-movement-2026-09-28.md`):
//! the readout's intake and nearest-neighbour factors, and the brain options the check
//! takes. Each at most 200 ticks; no pinned hashes.

use cubarium_voxel_fauna::Founder;
use cubarium_voxel_sim::{ScheduleMode, Sim, SimConfig};

use super::search::brain_driver;
use super::window::{Window, mean_nearest_neighbour};
use crate::es::voxel::EpisodeDriver;
use crate::es::voxel::controller::{EpisodeKind, VoxelControl};
use crate::es::voxel::driver::sampled_stores;
use crate::es::voxel::task::{PreparedArena, Stage, TRAINING_LAYOUT_SEEDS};

fn arena_sim(founder: Founder) -> Sim {
    let seed = TRAINING_LAYOUT_SEEDS[0];
    let prepared = PreparedArena::build_stage(founder, seed, Stage::A);
    let mut arena = prepared.episode_arena();
    arena.refound(sampled_stores(founder, seed, seed, 0));
    let mut sim = arena.into_sim_prepared(SimConfig::with_threads(1), prepared.episode_senses());
    sim.set_mode(ScheduleMode::Static);
    sim
}

/// A window's intake for a lineage is the ledger's assimilation for that lineage over
/// the same ticks, whoever drives the bodies (here the heuristic, with no tables).
#[test]
fn a_windows_intake_is_the_ledgers_assimilation() {
    for founder in crate::es::voxel::TRAINED_LINEAGES {
        let mut sim = arena_sim(founder);
        let tables = vec![None; Founder::COUNT];
        let before = sim.fauna().view().ledger.assimilated_by_founder[founder.index()];
        let mut w = Window::begin(&mut sim, &tables, &[0; Founder::COUNT], 150);
        while !w.done() {
            w.step(&mut sim);
        }
        let r = w.finish(&sim, 0);
        let after = sim.fauna().view().ledger.assimilated_by_founder[founder.index()];
        let got = r.lineages[founder.index()].intake;
        assert!(
            (got - (after - before)).abs() <= 1e-12 + 1e-9 * (after - before).abs(),
            "{founder:?}: window {got} vs ledger {}",
            after - before
        );
    }
}

/// Mean nearest-neighbour distance on the ring: wraps across the seam, needs two bodies.
#[test]
fn the_mean_nearest_neighbour_distance_wraps_the_ring() {
    assert_eq!(mean_nearest_neighbour(&[(1.0, 0.5)], 20.0), None);
    let pair = mean_nearest_neighbour(&[(1.0, 0.5), (4.0, 0.5)], 20.0).expect("two");
    assert!((pair - 3.0).abs() < 1e-12);
    // Across the seam: 19.5 and 0.5 are 1 m apart on a 20 m ring.
    let seam =
        mean_nearest_neighbour(&[(19.5, 0.0), (0.5, 0.0), (10.0, 0.0)], 20.0).expect("three");
    assert!((seam - (1.0 + 1.0 + 9.5) / 3.0).abs() < 1e-12, "{seam}");
    // Depth counts too.
    let diag = mean_nearest_neighbour(&[(0.0, 0.0), (3.0, 4.0)], 20.0).expect("two");
    assert!((diag - 5.0).abs() < 1e-12);
}

/// The check's brain options: a centre file, or `heuristic` (the fauna's own forager).
#[test]
fn the_brain_options_parse() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/centres");
    let file = brain_driver(
        Founder::Browser,
        &dir.join("r2-frondgrazer-upd160.json").display().to_string(),
    )
    .expect("a centre");
    assert!(matches!(file.kind(), EpisodeKind::Gru(_)));
    let h = brain_driver(Founder::Browser, "heuristic").expect("the heuristic");
    assert!(matches!(
        h.kind(),
        EpisodeKind::Control(VoxelControl::Heuristic)
    ));
    assert!(brain_driver(Founder::Blind, "no-such-brain").is_err());
    let _: EpisodeDriver = h;
}
