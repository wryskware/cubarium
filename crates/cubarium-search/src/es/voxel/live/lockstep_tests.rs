//! Stage 2's lock step (unified mode stepped one window at a time): the brain centre moves
//! after every window, every world of a generation runs the same centre in a window, and a
//! resumed run restores the CMA state and the centres exactly. Small still worlds, each
//! run at most 200 ticks; no pinned hashes.

use std::path::PathBuf;

use cubarium_voxel_fauna::Founder;

use super::ecology::EcologyParams;
use super::screen::ScreenSpec;
use super::search::{Checkpoint, EvalSpec, SearchRecord, SearchSpec, centre_key, search};
use crate::es::voxel::store::VoxelPolicyFile;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cub-lockstep-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn centre(f: Founder) -> PathBuf {
    let file = match f {
        Founder::Blind => "r2-littershredder-upd512.json",
        _ => "r2-frondgrazer-upd160.json",
    };
    PathBuf::from(format!(
        "transfer:{}/../../config/training/centres/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// Two generations' worth of a unified search on small still worlds: λ = 2 and one seed
/// (three worlds a generation), two 50-tick windows, one brain pair, every candidate past
/// the screen.
fn spec(out: PathBuf, generations: u32, resume: bool) -> SearchSpec {
    let world = super::world::still_water(cubarium_voxel::Config {
        width: 64,
        height: 24,
        depth: 8,
        voxel_m: 0.25,
        landform: cubarium_voxel::Landform::Staged(cubarium_voxel::Recipe::DEFAULT),
        ..cubarium_voxel::Config::default()
    });
    let genome = EcologyParams::cotrain()
        .genome()
        .cloned()
        .expect("a genome");
    SearchSpec {
        centres: crate::es::voxel::TRAINED_LINEAGES
            .iter()
            .map(|&f| (f, centre(f)))
            .collect(),
        genome,
        start: None,
        screen: ScreenSpec {
            r: [0.0, f64::INFINITY],
            s: [0.0, f64::INFINITY],
            f_min: 0.0,
            ..ScreenSpec::default()
        },
        lambda: 2,
        seeds_per_generation: 1,
        generations,
        wall_seconds: 1e9,
        sigma0: 0.3,
        threads: 3,
        seed_base: 7,
        cma_seed: 11,
        eval: EvalSpec::new(world, 100, 50),
        brain_pairs: 1,
        brain_sigma: 0.05,
        brain_learning_rate: 0.01,
        resume: resume.then(|| out.clone()),
        out,
    }
}

fn run(out: PathBuf, generations: u32, resume: bool) -> Vec<SearchRecord> {
    search(&spec(out, generations, resume)).expect("the search runs")
}

/// In unified mode the centre moves after each window, not only at the end of the
/// generation: two windows, two brain steps, each one moving every trained centre.
#[test]
fn in_unified_mode_the_centre_moves_after_each_window() {
    let records = run(scratch("moves"), 1, false);
    let windows = &records[0].windows;
    assert_eq!(
        windows.len(),
        2,
        "two windows in a 100-tick horizon of 50-tick windows"
    );
    for f in crate::es::voxel::TRAINED_LINEAGES {
        let keys: Vec<u64> = windows
            .iter()
            .map(|w| {
                let l = w
                    .lineages
                    .iter()
                    .find(|l| l.lineage == f.name())
                    .expect("a trained lineage is logged every window");
                assert!(
                    l.bodies >= 2,
                    "{f:?} window {}: {} bodies",
                    w.window,
                    l.bodies
                );
                assert!(l.update_rms > 0.0, "{f:?} window {} took no step", w.window);
                l.centre_key
            })
            .collect();
        assert_ne!(
            keys[0], keys[1],
            "{f:?}: the second window runs a moved centre"
        );
    }
    assert_eq!(windows[1].brain_step, windows[0].brain_step + 1);
}

/// Every world of a generation runs the same centre in a given window.
#[test]
fn every_world_of_a_generation_sees_the_same_centre_in_a_window() {
    let records = run(scratch("same"), 1, false);
    for w in &records[0].windows {
        assert!(
            w.world_keys.len() >= 2,
            "window {}: {} worlds",
            w.window,
            w.world_keys.len()
        );
        let first = w.world_keys[0];
        assert!(
            w.world_keys.iter().all(|k| *k == first),
            "window {}: {:?}",
            w.window,
            w.world_keys
        );
        let mut want = 0u64;
        for l in &w.lineages {
            want ^= l.centre_key;
        }
        assert_eq!(
            first, want,
            "the key the worlds ran is the window's centres'"
        );
    }
}

/// A resumed run restores the CMA mean, σ and the centres exactly, and carries on with
/// the next generation and brain step.
#[test]
fn a_resumed_run_restores_the_cma_state_and_the_centres_exactly() {
    let out = scratch("resume");
    let first = run(out.clone(), 1, false);
    let cp = Checkpoint::load(&out).expect("a checkpoint after a generation");
    assert_eq!(cp.next_generation, 1);
    assert_eq!(
        cp.cma.mean(),
        first[0].mean.as_slice(),
        "the mean, bit for bit"
    );
    assert_eq!(cp.cma.sigma().to_bits(), first[0].sigma.to_bits());
    for f in crate::es::voxel::TRAINED_LINEAGES {
        let saved = VoxelPolicyFile::load(&out.join(format!("centre-{}.json", f.name())))
            .expect("the generation's centre");
        let held = cp
            .centres
            .iter()
            .find(|c| c.lineage == f.name())
            .expect("the checkpoint holds every centre");
        assert_eq!(held.theta, saved.theta, "{f:?}: the centre, bit for bit");
    }
    let resumed = run(out.clone(), 2, true);
    assert_eq!(resumed.len(), 1, "only the generation still owed");
    assert_eq!(resumed[0].generation, 1);
    assert_eq!(
        resumed[0].windows[0].brain_step,
        first[0].windows.last().unwrap().brain_step + 1
    );
    assert_eq!(
        resumed[0].sigma_start.to_bits(),
        cp.cma.sigma().to_bits(),
        "the resumed generation starts from the checkpoint's σ"
    );
    assert_eq!(resumed[0].mean_start, first[0].mean, "and its mean");
    for c in &cp.centres {
        let l = resumed[0].windows[0]
            .lineages
            .iter()
            .find(|l| l.lineage == c.lineage)
            .expect("logged");
        assert_eq!(
            l.centre_key,
            centre_key(&c.theta),
            "{}: its first window runs the checkpoint's centre",
            c.lineage
        );
    }
}
