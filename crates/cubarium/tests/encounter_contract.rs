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
/// decides the bite.
///
/// **The recorded trajectory is gone, 2026-09-22.** It was the evidence that package 0
/// changed nothing, and it held to the bit. Package 1a
/// (`design/handoffs/voxel-founder-step-2026-09-22.md`) changes the live path on
/// purpose — a founder steps ledges, the contact receptor stops calling a steppable
/// ledge a wall, and the seeder's walk reaches further — so there is no "before" left
/// to defend and a re-recorded trajectory would be a pinned world, which this project
/// does not keep. What the change did to these numbers is in
/// `design/7_Research/voxel-census-2026-09-20.md`, "Step rule, 2026-09-22". What stays
/// here is what the live path must always satisfy.
#[test]
fn two_hundred_ticks_of_the_live_path_stand_eat_and_see() {
    let cfg = VoxelConfig::default();
    let mut world = scene::authored(cfg.world.clone());
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    habitat::seed(&mut world, &mut flora, &mut fauna);
    install_default_founders(&mut fauna).expect("the built-in centres validate");
    let mut senses = Senses::new();
    senses.settle(&world.view(), &flora.view());
    let mut sim = Sim::new(world, flora, fauna, SimConfig::default(), Some(senses));
    let seeded_layers: std::collections::HashSet<u32> = sim
        .fauna()
        .view()
        .animals
        .iter()
        .filter(|a| a.founder.is_some())
        .map(|a| a.site.y)
        .collect();
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
    // A shredder on a terrain face (package mobility) keeps the face it started from as
    // its site while its pose hangs beside the face; its site is what must be a face.
    let climbing: std::collections::HashMap<u64, cubarium_voxel_flora::Site> = av
        .animals
        .iter()
        .filter(|a| a.mobility.climb.is_some())
        .map(|a| (a.id, a.site))
        .collect();
    poses.sort_by_key(|p| p.0);

    assert_eq!(poses.len(), 16, "eight founders of each lineage");
    // Every founder still stands on a support face: the step rule moves `site.y` and
    // `step::terrain` removes anything whose site is not one, so a body that stepped
    // wrong would be gone rather than wrong.
    let wv0 = sim.world().view();
    for p in &poses {
        if let Some(s) = climbing.get(&p.0) {
            assert!(
                wv0.is_support(i64::from(s.x), s.y, s.z),
                "climbing founder #{} started from air",
                p.0
            );
            continue;
        }
        assert!(
            wv0.is_support(
                i64::from((p.2 / wv0.config.voxel_m).floor() as i64),
                p.1,
                (p.3 / wv0.config.voxel_m).floor() as u32
            ),
            "founder #{} stands on air at layer {}",
            p.0,
            p.1
        );
    }
    // The step rule is live: on the shipped terraced habitat, some founder leaves the
    // layer it was seeded on inside ten seconds.
    assert!(
        poses
            .iter()
            .map(|p| p.1)
            .collect::<std::collections::HashSet<u32>>()
            != seeded_layers,
        "no founder changed its standing layer in 200 ticks"
    );
    for (sum, what) in [
        (poses.iter().map(|p| p.2).sum::<f64>(), "pose x"),
        (poses.iter().map(|p| p.3).sum::<f64>(), "pose z"),
        (poses.iter().map(|p| p.4).sum::<f64>(), "heading"),
    ] {
        assert!(sum.is_finite() && sum > 0.0, "{what} is {sum}");
    }
    assert!(
        av.ledger.eaten_organic_in > 0.0,
        "the centres fed at all in 200 ticks"
    );

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
        cone > 0.0 && cone.is_finite(),
        "the browsers see something: {cone:.14}"
    );
}
