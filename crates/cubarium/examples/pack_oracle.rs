//! `pack_oracle`: the change-only pack held to the full pack on a real world, every tick,
//! with the sim advancing (`cubarium::sink::gpu::voxel::oracle`). A check, not a test:
//! run it by name after touching the pack.
//!
//! ```text
//! cargo run --release -p cubarium --example pack_oracle -- \
//!     [--config config/desktop/terrarium.toml] [--set 'lighting = "lit"']... \
//!     [--state DIR | --seed 1 --found-ticks 6000] [--px 6] [--ticks 1200] [--shower-at 100]
//! ```
//!
//! `--state DIR` loads a saved world (`world.voxel`, `flora.bin`, `fauna.bin`, as
//! `light_capture --state` writes them); without it the world is founded from `--seed`
//! and stepped `--found-ticks` first. The frames vary: most run at once, some stay in
//! flight across the next pack, some are never recorded, a few are displaced. Prints the
//! first mismatch and exits non-zero, or each pack's CPU time and bricks per tick.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::voxel::oracle::{Frame, PackOracle};
use cubarium::voxel::project::Projection;
use cubarium::voxel::{LayerConfig, Seeds, VoxelConfig, found as found_habitat, params_of};
use cubarium_voxel::World;
use cubarium_voxel_fauna::Fauna;
use cubarium_voxel_flora::Flora;
use cubarium_voxel_sim::{Sim, SimConfig};

fn main() -> Result<()> {
    let mut config = PathBuf::from("config/desktop/terrarium.toml");
    let mut set: Vec<String> = Vec::new();
    let mut state: Option<PathBuf> = None;
    let (mut seed, mut found_ticks, mut ticks, mut shower_at, mut px) = (1u64, 0u64, 1200u64, None, None);
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().context("missing value");
        match k.as_str() {
            "--config" => config = v()?.into(),
            "--set" => set.push(v()?),
            "--state" => state = Some(v()?.into()),
            "--seed" => seed = v()?.parse()?,
            "--found-ticks" => found_ticks = v()?.parse()?,
            "--ticks" => ticks = v()?.parse()?,
            "--shower-at" => shower_at = Some(v()?.parse::<u64>()?),
            "--px" => px = Some(v()?.parse::<u32>()?),
            _ => bail!("unknown argument {k}"),
        }
    }
    let mut table: toml::Table = std::fs::read_to_string(&config)?.parse()?;
    for line in &set {
        let s: toml::Table = line.parse()?;
        for (k, v) in s {
            match (table.get_mut(&k), v) {
                (Some(toml::Value::Table(t)), toml::Value::Table(s)) => t.extend(s),
                (_, v) => {
                    table.insert(k, v);
                }
            }
        }
    }
    let mut cfg: VoxelConfig = toml::Value::Table(table).try_into()?;
    if let Some(px) = px {
        cfg.px_per_voxel = px;
    }
    let (world, flora, fauna) = load_or_found(&cfg, seed, found_ticks, state.as_ref())?;
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &cfg.world)?;
    let mut o = PackOracle::for_config(&cfg, proj, cubarium::voxel::load_models(&cfg), None);
    let mut senses = cubarium_voxel_fauna::Senses::with_cue(fauna.config().cue);
    senses.settle(&world.view(), &flora.view());
    let threads = cubarium_voxel::default_threads();
    let mut sim = Sim::new(world, flora, fauna, SimConfig::with_threads(threads), Some(senses));
    let candle = |tick: u64| tick as f64 / f64::from(cubarium_voxel::TICK_HZ);

    // Both packs' sky planes first, so the comparison is whole.
    let started = Instant::now();
    while {
        let w = sim.world();
        o.tick(&w.view(), sim.flora().view(), sim.fauna().view(), candle(w.tick()), Frame::Now)
            .map_err(anyhow::Error::msg)?;
        o.light_pending()
    } {
        if started.elapsed().as_secs() > 120 {
            bail!("the sky planes never arrived");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    eprintln!(
        "pack_oracle: tick {} ({:.1} sim min), {} ticks to run",
        sim.world().tick(),
        sim.world().tick() as f64 / 1200.0,
        ticks
    );
    let (mut full_ms, mut delta_ms, mut bricks) = (Vec::new(), Vec::new(), Vec::new());
    let mut whole = 0;
    let mut raining = 0;
    for k in 0..ticks {
        if Some(k) == shower_at {
            let ok = sim.world_mut().force_rain(cubarium_voxel::weather::RainMode::Shower);
            eprintln!("pack_oracle: a shower forced at tick {k}: {ok}");
        }
        sim.step();
        let frame = match k % 13 {
            5 => Frame::InFlight,
            9 => Frame::Unrecorded,
            _ => Frame::Now,
        };
        let w = sim.world();
        raining += usize::from(w.view().is_raining());
        let r = o
            .tick(&w.view(), sim.flora().view(), sim.fauna().view(), candle(w.tick()), frame)
            .map_err(|e| anyhow::anyhow!("tick {k} (world tick {}): {e}", w.tick()))?;
        if frame == Frame::InFlight && k % 39 == 5 {
            o.discard();
        }
        full_ms.push(r.full_ms);
        delta_ms.push(r.delta_ms);
        match r.bricks {
            Some(b) => bricks.push(b as f64),
            None => whole += 1,
        }
    }
    let q = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let at = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
        format!("{:.3} / {:.3} / {:.3}", at(0.5), at(0.95), at(1.0))
    };
    println!(
        "pack_oracle: {ticks} ticks match ({raining} raining). median / p95 / max: full pack {} ms, \
         change-only {} ms, bricks {} ({whole} whole-volume uploads), {} styles made",
        q(full_ms),
        q(delta_ms),
        if bricks.is_empty() { "-".into() } else { q(bricks) },
        o.styles_made()
    );
    Ok(())
}

fn load_or_found(cfg: &VoxelConfig, seed: u64, ticks: u64, state: Option<&PathBuf>) -> Result<(World, Flora, Fauna)> {
    if let Some(d) = state {
        let world = World::load(&std::fs::read(d.join("world.voxel"))?)?;
        let flora = cubarium_voxel_flora::snapshot::decode(&std::fs::read(d.join("flora.bin"))?)?;
        let fauna = Fauna::load(&std::fs::read(d.join("fauna.bin"))?)?;
        return Ok((world, flora, fauna));
    }
    let params = params_of(cfg, &[])?;
    let founded = found_habitat(
        &cfg.world,
        &LayerConfig::new(&params, cfg.world.voxel_m),
        params.fauna.founder_counts(),
        Seeds::from(seed),
        false,
    );
    let (world, flora, fauna) = (founded.world, founded.flora, founded.fauna);
    if ticks == 0 {
        return Ok((world, flora, fauna));
    }
    let mut senses = cubarium_voxel_fauna::Senses::with_cue(fauna.config().cue);
    senses.settle(&world.view(), &flora.view());
    let mut sim = Sim::new(world, flora, fauna, SimConfig::with_threads(cubarium_voxel::default_threads()), Some(senses));
    for _ in 0..ticks {
        sim.step();
    }
    Ok((sim.world().clone(), sim.flora().clone(), sim.fauna().clone()))
}
