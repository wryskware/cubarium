//! One terrain draw's stages, timed: generate, the gate's settle, the feasibility check
//! (`habitat::seed_pre_rolled` on the bare terrain). Development measurement only.
//!
//! `feasibility_timing CONFIG [seed=1]`

use std::time::Instant;

use cubarium_session::WorldSpec;
use cubarium_voxel_sim::found::{LayerConfig, feasibility};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = args.first().expect("feasibility_timing CONFIG [seed=]").clone();
    let seed: u64 = args.iter().find_map(|a| a.strip_prefix("seed=").and_then(|v| v.parse().ok())).unwrap_or(1);
    let spec = WorldSpec { config: Some(config.into()), ..WorldSpec::default() };
    let p = spec.sim_params().unwrap().resolve_sets(&[]).unwrap();
    let t = Instant::now();
    let world = cubarium_voxel::World::new(cubarium_voxel::Config { seed, ..p.world.clone() });
    println!("generate {:.2} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let mut probe = world.clone();
    probe.settle(40);
    println!("clone + settle 40 {:.2} s", t.elapsed().as_secs_f64());
    let layers = LayerConfig::new(&p, p.world.voxel_m);
    let t = Instant::now();
    let s = feasibility(&mut probe, &layers, p.fauna.founder_counts());
    println!("feasibility {:.2} s: accepted {}, {}", t.elapsed().as_secs_f64(), s.acceptance.accepted, s.acceptance.reasons());
}
