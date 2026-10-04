//! One grid world's step time at a given thread count (`design/handoffs/tail-threads-2026-10-03.md`
//! step 1): the grid's `m=M p=P` cell over `config/desktop/terrarium.toml`, world seed
//! `SEED` (asked), founded, then `MINUTES` simulated minutes timed at `THREADS`.
//!
//! `cargo run --release -p cubarium-search --example world_threads_bench -- THREADS [MINUTES] [M] [P] [SEED]`
//!
//! One process per thread count: the compute pool is sized by its first user.

use cubarium_search::es::voxel::live::grid::{Axis, cell_ecology, cells, neutral_ecology};
use cubarium_search::es::voxel::live::search::{EvalRun, EvalSpec};
use cubarium_search::es::voxel::live::window::TICKS_PER_MIN;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize, d: &str| args.get(i).cloned().unwrap_or_else(|| d.to_string());
    let threads: usize = arg(0, "1").parse().map_err(|e| format!("threads: {e}"))?;
    let minutes: u64 = arg(1, "10").parse().map_err(|e| format!("minutes: {e}"))?;
    let m: f64 = arg(2, "1").parse().map_err(|e| format!("m: {e}"))?;
    let p: f64 = arg(3, "2").parse().map_err(|e| format!("p: {e}"))?;
    let seed: u64 = arg(4, "1").parse().map_err(|e| format!("seed: {e}"))?;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = root.join("config/desktop/terrarium.toml");
    let world = cubarium_voxel::world_config_from_file(&file)?;
    let cell = &cells(&[Axis::named("m", &[m])?, Axis::named("p", &[p])?])[0];
    let eco = cell_ecology(cell, Some(&file))?;
    let founders = neutral_ecology(Some(&file))?.fauna().founder_counts();
    let horizon = minutes * TICKS_PER_MIN;
    let mut spec = EvalSpec::new(world, horizon, horizon);
    spec.founders = founders;
    spec.abort_animals = 600;
    spec.stop_on_loss = false;
    spec.asked_seed = true;

    cubarium_voxel_sim::prepare_compute_pool(threads);
    let t0 = std::time::Instant::now();
    let mut run = EvalRun::begin(&eco, seed, &[], &spec);
    let founding = t0.elapsed().as_secs_f64();
    run.set_threads(threads);
    let tables = vec![None; cubarium_voxel_fauna::Founder::COUNT];
    let zeros = vec![0usize; cubarium_voxel_fauna::Founder::COUNT];
    let t1 = std::time::Instant::now();
    while !run.done() {
        run.window(&tables, &zeros, 0);
    }
    let wall = t1.elapsed().as_secs_f64();
    let animals = run.sim().fauna().view().animals.len();
    let stands = run.sim().flora().view().stands.len();
    println!(
        "threads={threads} cell={} seed={seed} minutes={} founding_s={founding:.1} step_s={wall:.2} ms_per_tick={:.3} animals={animals} stands={stands}",
        cell.label,
        run.elapsed() / TICKS_PER_MIN,
        wall * 1000.0 / run.elapsed().max(1) as f64
    );
    Ok(())
}
