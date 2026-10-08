//! The live water step's cost on a config file's world, and the memory the exchange keeps
//! after a world is dropped (terrain-first package, 2026-10-08: the exchange's scratch went
//! from per stepping thread to per world).
//!
//! `water_step_bench CONFIG [seed=1] [threads=8] [ticks=200] [worlds=2]`
//!
//! Per world, one after another on this thread: generate it from `CONFIG`'s `[world]`
//! table, open a shower so the water moves, step 20 ticks to warm up, then time `ticks`
//! water steps on `threads`. Prints the mean and the median ms per step, and this
//! process's resident memory after the steps and after the world is dropped.
//! Development measurement only.

use std::time::Instant;

use cubarium_voxel::World;

fn rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|k| k.parse::<f64>().ok()))
        })
        .map_or(0.0, |kb| kb / 1024.0)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("water_step_bench CONFIG [seed=] [threads=] [ticks=] [worlds=]");
    let arg = |name: &str, default: u64| -> u64 {
        args.iter()
            .find_map(|a| a.strip_prefix(&format!("{name}=")).and_then(|v| v.parse().ok()))
            .unwrap_or(default)
    };
    let (seed, threads, ticks, worlds) = (arg("seed", 1), arg("threads", 8) as usize, arg("ticks", 200), arg("worlds", 2));
    let base = cubarium_voxel::world_config_from_file(std::path::Path::new(path)).expect("the config's [world]");
    println!("rss at start {:.0} MB", rss_mb());
    for k in 0..worlds {
        let t = Instant::now();
        let mut w = World::new(cubarium_voxel::Config { seed: seed + k, ..base.clone() });
        let built = t.elapsed().as_secs_f64();
        if w.config().closed_water_budget && !w.outlet_open() {
            w.apply(cubarium_voxel::Command::SetOutlet { open: true });
        }
        let _ = w.open_with_rain();
        for _ in 0..20 {
            w.step_with(threads);
        }
        let mut ms: Vec<f64> = Vec::with_capacity(ticks as usize);
        for _ in 0..ticks {
            let t = Instant::now();
            w.step_with(threads);
            ms.push(t.elapsed().as_secs_f64() * 1e3);
        }
        let mean = ms.iter().sum::<f64>() / ms.len().max(1) as f64;
        ms.sort_by(f64::total_cmp);
        let median = ms.get(ms.len() / 2).copied().unwrap_or(0.0);
        let stepped = rss_mb();
        drop(w);
        println!(
            "world {k} (seed {}): built in {built:.2} s; water step mean {mean:.3} ms, median {median:.3} ms \
             over {ticks} ticks on {threads} threads; rss {stepped:.0} MB stepped, {:.0} MB after drop",
            seed + k,
            rss_mb()
        );
    }
}
