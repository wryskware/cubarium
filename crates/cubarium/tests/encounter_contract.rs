//! The encounter query is read-only: the live tick reads exactly what it read before.
//!
//! Package 0 (`design/handoffs/voxel-edible-stock-2026-09-21.md`) added `pub` query
//! helpers derived from live paths — `senses::ray_first_hit` now delegates to a variant
//! that also returns the cell it struck, `senses::cone_occupancy` to one whose surface
//! pools are optional, and `body::mouth_foliage_stands` to the scan the band arm shares.
//! This is the brief's "a ≤200-tick test must show the existing reading is unchanged".

use cubarium::voxel::{VoxelConfig, habitat, install_default_founders, scene};
use cubarium_voxel_fauna::{Fauna, FaunaConfig, Founder, Senses};
use cubarium_voxel_flora::{Flora, FloraConfig};
use cubarium_voxel_sim::{Sim, SimConfig};

/// 200 ticks of the shipped authored habitat with the built-in centres installed, then
/// where the founders stand, what their mouths took and what their cones read.
///
/// Every live reading the new helpers touch is upstream of these numbers: the cone feeds
/// the trained controller, the controller's held actions move the pose, and the mouth
/// decides the bite. The expected values were recorded by running this same measurement
/// on the branch point (7dba001), before the extraction; they reproduced to the bit
/// afterwards.
#[test]
fn two_hundred_ticks_of_the_live_path_read_what_they_read_before() {
    let cfg = VoxelConfig::default();
    let mut world = scene::authored(cfg.world.clone());
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    habitat::seed(&mut world, &mut flora, &mut fauna);
    install_default_founders(&mut fauna).expect("the built-in centres validate");
    let mut senses = Senses::new();
    senses.settle(&world.view(), &flora.view());
    let mut sim = Sim::new(world, flora, fauna, SimConfig::default(), Some(senses));
    for _ in 0..200 {
        sim.step();
    }

    let av = sim.fauna().view();
    let mut poses: Vec<(u64, u32, f64, f64, f64)> = av
        .animals
        .iter()
        .filter(|a| a.founder.is_some())
        .map(|a| (a.id, a.site.y, a.pose.x, a.pose.z, a.pose.heading_rad))
        .collect();
    poses.sort_by_key(|p| p.0);

    assert_eq!(poses.len(), 16, "eight founders of each lineage");
    assert_eq!(
        poses.iter().map(|p| p.1).collect::<Vec<u32>>(),
        vec![
            12, 17, 22, 25, 28, 36, 39, 45, 13, 19, 20, 20, 20, 22, 33, 48
        ],
        "the standing layers a founder never leaves"
    );
    for (sum, want, what) in [
        (
            poses.iter().map(|p| p.2).sum::<f64>(),
            211.03399667625976,
            "pose x",
        ),
        (
            poses.iter().map(|p| p.3).sum::<f64>(),
            45.60842100665320,
            "pose z",
        ),
        (
            poses.iter().map(|p| p.4).sum::<f64>(),
            60.24210876396594,
            "heading",
        ),
        (
            av.ledger.eaten_organic_in,
            0.16375011862748,
            "eaten organic",
        ),
    ] {
        assert!((sum - want).abs() < 1e-12, "{what} drifted: {sum:.14}");
    }

    // The browser cone the policy is given, summed over every browser and sector: this
    // is the reading that goes through `ray_first_hit` and `cone_occupancy`.
    let fv = sim.flora().view();
    let wv = sim.world().view();
    let mut cone = 0.0;
    for a in av
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Browser))
    {
        if let Some(sectors) = cubarium_voxel_fauna::browser_cone_readings(&wv, &fv, &av, a) {
            for (fraction, proximity) in sectors {
                cone += fraction + proximity;
            }
        }
    }
    assert!(
        (cone - 36.85297619047619).abs() < 1e-12,
        "the browsers' cone reading drifted: {cone:.14}"
    );
}
