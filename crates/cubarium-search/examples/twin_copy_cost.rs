//! **Where a twin copy's wall time goes** (TRAIN-REVIEW, 2026-10-14): loads one saved live
//! training world, captures it as the trainer's twins do ([`TwinStart::capture`]) and runs
//! `pairs` twin pairs through the trainer's own [`run_copy`] on one thread, timing:
//!
//! - `open`: decoding the start into a fresh `Sim` and installing the centres (per copy);
//! - `copy`: one whole [`run_copy`] (open, `ticks` steps, the per-tick pose hash, metering);
//! - `step`: `ticks` bare steps of an opened copy on the centre (no variant, no hash);
//!
//! Run it under `perf record` for the breakdown inside the step. Analysis only.
//!
//! ```text
//! twin_copy_cost world=runs/lab/<job>/run/run/worlds/world-0 lineage=seedporter \
//!     centre=<centre .json> [pairs=4] [ticks=2400] [water_every=20] [sigma=0.05]
//! ```

use cubarium_search::es::voxel::kin::BrainStage;
use cubarium_search::es::voxel::kin::matched::SimStart;
use cubarium_search::es::voxel::kin::twins::{Metering, TwinStart, run_copy};
use cubarium_search::es::voxel::live::curiosity::Memory;
use cubarium_search::es::voxel::live::variants::VariantTable;
use cubarium_search::es::voxel::two_layer::TwoLayerPolicyFile;
use cubarium_search::es::voxel::{EpisodeDriver, parse_founder};
use std::path::Path;
use std::time::Instant;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = Path::new(arg(&args, "world").ok_or("world=DIR")?);
    let lineage = parse_founder(arg(&args, "lineage").ok_or("lineage=NAME")?)?;
    let file = TwoLayerPolicyFile::load(Path::new(arg(&args, "centre").ok_or("centre=FILE")?))?;
    let num = |k: &str, d: f64| arg(&args, k).map_or(d, |s| s.parse().expect("a number"));
    let pairs = num("pairs", 4.0) as usize;
    let ticks = num("ticks", 2400.0) as u64;
    let water_every = num("water_every", 20.0) as u32;
    let sigma = num("sigma", 0.05);
    cubarium_voxel::world::set_thread_override(1);

    let t = Instant::now();
    let start = SimStart::read(dir, &[lineage])?;
    let mut sim = start.open(1, water_every)?;
    let arbitration = sim.fauna().config().founder(lineage).brain;
    let driver = EpisodeDriver::two_layer(file.net()?, arbitration, lineage);
    let drivers = vec![(lineage, driver)];
    cubarium_search::es::voxel::live::install_centres(sim.fauna_mut(), &drivers);
    let twin = TwinStart::capture(0, &sim, &drivers, water_every, &Memory::default());
    let animals = sim.fauna().view().animals.len();
    let bodies = twin.bodies[lineage.index()].len();
    eprintln!(
        "# loaded in {:.3}s: {} animals, {} {} bodies; start bytes world {} flora {} fauna {}",
        t.elapsed().as_secs_f64(),
        animals,
        bodies,
        lineage.name(),
        twin.start.world.len(),
        twin.start.flora.len(),
        twin.start.fauna.len()
    );

    // open
    let reps = 8;
    let t = Instant::now();
    for _ in 0..reps {
        let mut s = twin.start.open(1, water_every)?;
        cubarium_search::es::voxel::live::install_centres(s.fauna_mut(), &drivers);
        std::hint::black_box(&s);
    }
    let open_s = t.elapsed().as_secs_f64() / reps as f64;

    // bare steps on the centre
    let mut s = twin.start.open(1, water_every)?;
    cubarium_search::es::voxel::live::install_centres(s.fauna_mut(), &drivers);
    let t = Instant::now();
    for _ in 0..ticks {
        s.step();
    }
    let step_s = t.elapsed().as_secs_f64();

    // whole copies, as the trainer runs them
    let table = VariantTable::antithetic_two_layer(&file, BrainStage::Top, arbitration, 20260924, sigma, 0, pairs)?;
    let metering = Metering::default();
    let mut copy_s = Vec::new();
    for c in 0..2 * pairs {
        let r = run_copy(&twin, ticks, lineage, &table, c, None, &metering)?;
        copy_s.push(r.wall_s);
    }
    let mean_copy = copy_s.iter().sum::<f64>() / copy_s.len() as f64;
    println!(
        "{{\"world\":\"{}\",\"animals\":{animals},\"bodies\":{bodies},\"ticks\":{ticks},\"water_every\":{water_every},\"open_s\":{open_s:.4},\"bare_step_s\":{step_s:.3},\"bare_ticks_per_s\":{:.0},\"copy_s\":{mean_copy:.3},\"copy_ticks_per_s\":{:.0},\"copies\":{}}}",
        dir.display(),
        ticks as f64 / step_s,
        ticks as f64 / mean_copy,
        copy_s.len()
    );
    Ok(())
}
