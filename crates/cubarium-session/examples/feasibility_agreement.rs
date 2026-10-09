//! How well the cheap feasibility check predicts the founding: per terrain seed, the check
//! on the bare terrain against the first placement try after the full pre-roll.
//! Development measurement.
//!
//! `feasibility_agreement CONFIG SEED...`

use cubarium_session::WorldSpec;
use cubarium_voxel_sim::found::{LayerConfig, TerrainFounding, feasibility, found_terrain};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let spec = WorldSpec { config: Some(args[0].clone().into()), ..WorldSpec::default() };
    let p = spec.sim_params().unwrap().resolve_sets(&[]).unwrap();
    let layers = LayerConfig::new(&p, p.world.voxel_m);
    let counts = p.fauna.founder_counts();
    for seed in args[1..].iter().filter_map(|s| s.parse::<u64>().ok()) {
        let world = cubarium_voxel::World::new(cubarium_voxel::Config { seed, ..p.world.clone() });
        let mut probe = world.clone();
        probe.settle(40);
        let check = feasibility(&mut probe, &layers, counts).acceptance;
        let actual = match found_terrain(world, seed, &layers, counts, 1, &()) {
            TerrainFounding::Founded(_) => "accepted".to_string(),
            TerrainFounding::Rejected(why) => format!("terrain rejected: {why}"),
            TerrainFounding::Refused(why) => format!("refused: {}", why.join("; ")),
            TerrainFounding::Cancelled => "cancelled".into(),
        };
        println!(
            "AGREE seed {seed}: check {} [{}] | founding {actual}",
            check.accepted,
            check.reasons()
        );
    }
}
