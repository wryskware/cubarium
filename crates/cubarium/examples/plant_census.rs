//! `plant_census`: how many plant cells a saved world stamps, by kind and by stand, for
//! sizing the plant pass (`[figures] plants = "models"`).
//!
//! ```text
//! cargo run --release -p cubarium --example plant_census -- --save DIR [--config FILE]
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use cubarium::voxel::VoxelConfig;
use cubarium::voxel::stand::{Part, Stands};

fn main() -> Result<()> {
    let mut config = PathBuf::from("config/desktop/terrarium.toml");
    let mut save = None::<PathBuf>;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--config" => config = it.next().context("--config FILE")?.into(),
            "--save" => save = Some(it.next().context("--save DIR")?.into()),
            _ => anyhow::bail!("unknown argument {k}"),
        }
    }
    let cfg: VoxelConfig = toml::from_str(&std::fs::read_to_string(&config)?)?;
    let save = save.context("--save DIR")?;
    let prepared = cubarium_session::prepare_loaded(&cfg.sim, &save).map_err(anyhow::Error::msg)?;
    let (world, flora) = (prepared.world, prepared.flora);
    let lib = cubarium::voxel::load_models(&cfg).context("models")?;
    let c = world.config();
    let mut stands = Stands::empty(c.width, c.height, c.depth);
    stands.track_owners();
    let t = std::time::Instant::now();
    stands.rebuild_with(&world.view(), flora.view(), &lib);
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let (mut trunk, mut crown, mut other) = (0usize, 0usize, 0usize);
    let mut per = vec![0u32; flora.view().stands.len()];
    let mut tops = vec![(u32::MAX, 0u32); flora.view().stands.len()];
    for (x, y, z) in stands.cells() {
        match stands.at(i64::from(x), i64::from(y), z) {
            Part::Trunk(_) => trunk += 1,
            Part::Crown { .. } => crown += 1,
            _ => {
                other += 1;
                continue;
            }
        }
        if let Some(o) = stands.owner(i64::from(x), y, z) {
            per[o] += 1;
            tops[o].0 = tops[o].0.min(y);
            tops[o].1 = tops[o].1.max(y);
        }
    }
    let mut counts: Vec<u32> = per.iter().copied().filter(|&n| n > 0).collect();
    counts.sort_unstable();
    let n = counts.len();
    let pct = |p: f64| counts.get(((n as f64 - 1.0) * p) as usize).copied().unwrap_or(0);
    let tall = tops.iter().filter(|t| t.0 != u32::MAX && t.1 - t.0 >= 8).count();
    println!(
        "world {}x{}x{}: {} flora stands, {} with block cells; block cells {} (trunk {trunk}, crown {crown}), other marks {other}; \
         cells/stand p50 {} p90 {} p99 {} max {}; stands >= 9 voxels tall: {tall}; rebuild {ms:.1} ms",
        c.width,
        c.height,
        c.depth,
        per.len(),
        n,
        trunk + crown,
        pct(0.5),
        pct(0.9),
        pct(0.99),
        counts.last().copied().unwrap_or(0),
    );
    Ok(())
}
