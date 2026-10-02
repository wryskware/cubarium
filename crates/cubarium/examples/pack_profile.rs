//! `pack_profile`: the GPU pack's CPU time per tick with the sim advancing, through the
//! live sink (headless), and with the `pack-profile` feature its phases.
//!
//! ```text
//! cargo run --release -p cubarium --features pack-profile --example pack_profile -- \
//!     [--config config/desktop/terrarium.toml] [--set LINE]... [--px 6] \
//!     [--state DIR | --seed 1 --found-ticks N] [--ticks 1200] [--shower-at K] [--png PATH]
//! ```
//!
//! `--png` writes the last frame, drawn from the change-only pack's last upload.
//!
//! Prints the pack's median, p95 and max ms per tick (over every tick, then the dry and the
//! raining ones), the bytes the voxel plane uploads per tick, and each phase's.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};

use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::pack_timer;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
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
    let mut png: Option<PathBuf> = None;
    let mut check = false;
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
            "--png" => png = Some(v()?.into()),
            "--check-raster" => check = true,
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
    let mut sink = VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            models: cubarium::voxel::load_models(&cfg),
            ..VoxelGpuSinkOptions::default()
        },
    )?;
    let mut senses = cubarium_voxel_fauna::Senses::with_cue(fauna.config().cue);
    senses.settle(&world.view(), &flora.view());
    let threads = cubarium_voxel::default_threads();
    let mut sim = Sim::new(world, flora, fauna, SimConfig::with_threads(threads), Some(senses));
    // The lit tier's sky plane first.
    let started = Instant::now();
    loop {
        if sink.stage_world(sim.world(), sim.flora(), sim.fauna()) && !sink.light_pending() {
            sink.render()?;
            break;
        }
        sink.render()?;
        if started.elapsed().as_secs() > 120 {
            bail!("the sky plane never arrived");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    eprintln!(
        "pack_profile: tick {} ({:.1} sim min), {} ticks",
        sim.world().tick(),
        sim.world().tick() as f64 / 1200.0,
        ticks
    );
    let mut rows: Vec<(bool, f64, f64, Vec<(&'static str, f64)>)> = Vec::new();
    let mut sim_ms = Vec::new();
    for k in 0..ticks {
        if Some(k) == shower_at {
            let ok = sim.world_mut().force_rain(cubarium_voxel::weather::RainMode::Shower);
            eprintln!("pack_profile: a shower forced at tick {k}: {ok}");
        }
        let t = Instant::now();
        sim.step();
        sim_ms.push(t.elapsed().as_secs_f64() * 1e3);
        sink.set_clock(sim.world().tick(), 0.0);
        let t = Instant::now();
        if !sink.stage_world(sim.world(), sim.flora(), sim.fauna()) {
            bail!("the renderer refused a pack with no frame in flight");
        }
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let phases = pack_timer::phases();
        let kib = sink.last_upload_kib();
        rows.push((sim.world().view().is_raining(), ms, kib, phases));
        sink.render()?;
    }
    let q = |mut v: Vec<f64>| -> String {
        if v.is_empty() {
            return "-".into();
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let at = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
        format!("{:7.3} {:7.3} {:7.3}", at(0.5), at(0.95), at(1.0))
    };
    println!("                     median     p95     max");
    for (label, pick) in [("all", None), ("dry", Some(false)), ("raining", Some(true))] {
        let sel: Vec<_> = rows.iter().filter(|r| pick.is_none_or(|p| p == r.0)).collect();
        if sel.is_empty() {
            continue;
        }
        println!("{label} ({} ticks)", sel.len());
        println!("  pack ms          {}", q(sel.iter().map(|r| r.1).collect()));
        println!("  voxel KiB up     {}", q(sel.iter().map(|r| r.2).collect()));
        let mut names: Vec<&'static str> = Vec::new();
        for r in &sel {
            for (n, _) in &r.3 {
                if !names.contains(n) {
                    names.push(n);
                }
            }
        }
        for n in names {
            let v = sel
                .iter()
                .map(|r| r.3.iter().find(|(m, _)| *m == n).map_or(0.0, |p| p.1))
                .collect();
            println!("    {n:16} {}", q(v));
        }
    }
    println!("  sim step ms      {}", q(sim_ms));
    if check {
        // The image the brick uploads built against a whole upload of the same world: the
        // same tick staged once more change-only (nothing moves), then once in full.
        let tick = sim.world().tick();
        sink.set_clock(tick, 0.0);
        sink.stage_world(sim.world(), sim.flora(), sim.fauna());
        sink.render()?;
        let a = sink.read_raster()?;
        sink.pack_next_in_full();
        sink.stage_world(sim.world(), sim.flora(), sim.fauna());
        sink.render()?;
        let b = sink.read_raster()?;
        let differ = a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(x, y)| x != y).count();
        println!("raster check: {differ} of {} pixels differ from a whole upload", a.len() / 4);
        let rw = sink.params().raster_w as usize;
        for (k, (x, y)) in a.chunks_exact(4).zip(b.chunks_exact(4)).enumerate().filter(|(_, (x, y))| x != y).take(12) {
            println!("  pixel ({}, {}): {x:?} vs whole {y:?}", k % rw, k / rw);
        }
        if let Some(path) = &png {
            let p = sink.params();
            let mask: Vec<u8> = a
                .chunks_exact(4)
                .zip(b.chunks_exact(4))
                .flat_map(|(x, y)| if x == y { [0, 0, 0, 255] } else { [255, 255, 255, 255] })
                .collect();
            cubarium_gpu::target::write_png(&path.with_extension("diff.png"), p.raster_w, p.raster_h, &mask)?;
        }
    }
    if let Some(path) = png {
        let p = sink.params();
        cubarium_gpu::target::write_png(&path, p.raster_w, p.raster_h, &sink.read_raster()?)?;
        eprintln!("pack_profile: wrote {}", path.display());
    }
    Ok(())
}

fn load_or_found(cfg: &VoxelConfig, seed: u64, ticks: u64, state: Option<&PathBuf>) -> Result<(World, Flora, Fauna)> {
    if let Some(d) = state
        && d.join("world.voxel").exists()
    {
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
    let (mut world, mut flora, mut fauna) = (founded.world, founded.flora, founded.fauna);
    if ticks > 0 {
        let mut senses = cubarium_voxel_fauna::Senses::with_cue(fauna.config().cue);
        senses.settle(&world.view(), &flora.view());
        let mut sim = Sim::new(world, flora, fauna, SimConfig::with_threads(cubarium_voxel::default_threads()), Some(senses));
        for _ in 0..ticks {
            sim.step();
        }
        (world, flora, fauna) = (sim.world().clone(), sim.flora().clone(), sim.fauna().clone());
    }
    if let Some(d) = state {
        std::fs::create_dir_all(d)?;
        std::fs::write(d.join("world.voxel"), world.save())?;
        std::fs::write(d.join("flora.bin"), cubarium_voxel_flora::snapshot::encode(&flora))?;
        std::fs::write(d.join("fauna.bin"), fauna.save())?;
    }
    Ok((world, flora, fauna))
}
