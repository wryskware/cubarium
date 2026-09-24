//! A shipped ring's water on its own — no plants, no animals — read at chosen minutes:
//! the lake gate's reading, the lake's level and volume, the in-world stores and the
//! water's milliseconds per tick. Development diagnostics only.
//!
//! ```text
//! cargo run --release -p cubarium-voxel --example water_level -- \
//!     <small|default|wide|terrarium|terrarium-small> <seed> [0,30,120] [threads]
//! ```
//!
//! The lake gate is the host's (`crates/cubarium/src/voxel/mod.rs`,
//! `generate_with_a_lake`): the lake's visible area after a 40-tick settle on a clone,
//! against the recipe's `min_lake_m2`. `seed` is the world's own, not the base of the
//! host's random stream.

use std::time::Instant;

use cubarium_voxel::{Config, Landform, Preset, TICK_HZ, Terrarium, World, hydrate};

fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "default".into());
    let seed: u64 = args.next().map_or(1, |s| s.parse().expect("a seed"));
    let marks: Vec<u64> = args
        .next()
        .unwrap_or_else(|| "0,30,120".into())
        .split(',')
        .map(|m| m.parse().expect("minutes"))
        .collect();
    let threads: usize = args.next().map_or(1, |s| s.parse().expect("threads"));

    let (config, want) = if let Some(t) = Terrarium::preset(&name) {
        let small = name != "terrarium";
        let config = Config {
            width: if small { 160 } else { 256 },
            height: if small { 72 } else { 128 },
            depth: if small { 24 } else { 48 },
            voxel_m: 0.125,
            seed,
            landform: Landform::Terrarium(t),
            ..Config::default()
        };
        (config, t.water.min_lake_m2)
    } else {
        let p = Preset::find(&name).expect("small, default, wide, terrarium or terrarium-small");
        let want = p.recipe.water.min_lake_m2;
        (Config { seed, ..p.config() }, want)
    };
    let mut world = World::new(config);
    let gate = {
        let mut probe = world.clone();
        probe.settle(40);
        hydrate::lake(&probe).visible_m2
    };
    println!(
        "{name} seed {seed}: gate {gate:.1} m2 visible against {want:.1} ({})",
        if gate >= want { "pass" } else { "REJECT" }
    );
    println!(
        "min,lake_level_v,lake_m3,lake_m2,pooled_m3,pore_m3,aquifer_m3,atmosphere_m3,stored_m3,ms_per_tick,residual"
    );
    let per_min = 60 * TICK_HZ as u64;
    let mut minute = 0u64;
    let mut ms = 0.0;
    for &mark in &marks {
        if mark > minute {
            let ticks = (mark - minute) * per_min;
            let t = Instant::now();
            for _ in 0..ticks {
                world.step_with(threads);
            }
            ms = t.elapsed().as_secs_f64() * 1e3 / ticks as f64;
            minute = mark;
        }
        let lake = hydrate::lake(&world);
        let v = world.view();
        let level = if lake.surface_cells.is_empty() {
            f64::from(lake.level_y)
        } else {
            lake.surface_cells
                .iter()
                .map(|&i| f64::from(world.config().coords(i).1) + v.free[i])
                .sum::<f64>()
                / lake.surface_cells.len() as f64
        };
        println!(
            "{minute},{level:.3},{:.4},{:.2},{:.4},{:.4},{:.4},{:.4},{:.4},{ms:.3},{:.2e}",
            lake.volume_m3,
            lake.visible_m2,
            world.pooled_m3(),
            world.pore_m3(),
            v.aquifer_m3,
            world.atmosphere_m3(),
            v.stored_m3(),
            v.total_residual(),
        );
    }
}
