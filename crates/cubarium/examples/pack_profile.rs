//! `pack_profile`: the GPU pack's CPU time per tick with the sim advancing, through the
//! live sink (headless), and with the `pack-profile` feature its phases.
//!
//! ```text
//! cargo run --release -p cubarium --features pack-profile --example pack_profile -- \
//!     [--config config/desktop/terrarium.toml] [--set LINE]... [--px 6] \
//!     [--state DIR | --save DIR | --seed 1 --found-ticks N] [--ticks 1200] [--shower-at K] [--png PATH]
//! ```
//!
//! `--save DIR` loads an app save (`world-N.voxel`, the session store's names) the way the
//! app does (`cubarium_session::prepare_loaded`, founders' drivers put back) and steps it
//! with the app's sim settings.
//!
//! `--png` writes the last frame, drawn from the change-only pack's last upload.
//!
//! `--mode serial` (the default) steps, then packs the live world: a tick costs both.
//! `--mode overlap` (H3) packs a snapshot of the last tick on its own thread while the next
//! tick steps (`VoxelGpuSink::stage_snapshot_beside`), then copies the next snapshot: a
//! tick costs the copy plus the longer of the two. Both print the step, the pack, the copy
//! and the whole tick, ms. `--mode snapshot` serializes the same snapshot pack;
//! `--mode compare` interleaves it with overlap, balancing tick cadences and host load.
//! Both include per-phase step timings; explicit thread counts override the GPU budget.
//!
//! Prints the pack's median, p95 and max ms per tick (over every tick, then the dry and the
//! raining ones), the bytes the voxel plane uploads per tick, and each phase's.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};

use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::pack_timer;
use cubarium::sink::gpu::voxel::snapshot::{PackSnapshot, SnapshotCopier};
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::project::Projection;
use cubarium::voxel::{LayerConfig, Seeds, VoxelConfig, found as found_habitat, params_of};
use cubarium_voxel::World;
use cubarium_voxel_fauna::Fauna;
use cubarium_voxel_flora::Flora;
use cubarium_voxel_sim::Sim;

fn main() -> Result<()> {
    let mut config = PathBuf::from("config/desktop/terrarium.toml");
    let mut set: Vec<String> = Vec::new();
    let mut state: Option<PathBuf> = None;
    let mut save: Option<PathBuf> = None;
    let mut png: Option<PathBuf> = None;
    let mut check = false;
    let mut overlap = false;
    let mut compare = false;
    let mut snapshot_serial = false;
    let (mut seed, mut found_ticks, mut ticks, mut shower_at, mut px) = (1u64, 0u64, 1200u64, None, None);
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().context("missing value");
        match k.as_str() {
            "--config" => config = v()?.into(),
            "--set" => set.push(v()?),
            "--state" => state = Some(v()?.into()),
            "--save" => save = Some(v()?.into()),
            "--seed" => seed = v()?.parse()?,
            "--found-ticks" => found_ticks = v()?.parse()?,
            "--ticks" => ticks = v()?.parse()?,
            "--shower-at" => shower_at = Some(v()?.parse::<u64>()?),
            "--px" => px = Some(v()?.parse::<u32>()?),
            "--png" => png = Some(v()?.into()),
            "--check-raster" => check = true,
            "--mode" => (overlap, compare, snapshot_serial) = match v()?.as_str() {
                "serial" => (false, false, false),
                "overlap" => (true, false, false),
                "snapshot" => (false, false, true),
                "compare" => (true, true, false),
                m => bail!("--mode serial | overlap | snapshot | compare, not {m}"),
            },
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
    if overlap || snapshot_serial {
        cfg.sim.threads = cfg.sim.sim_config_with_reserve(cubarium::sink::gpu::voxel::WORKER_RESERVE).threads;
    }
    let mut sim = match &save {
        Some(dir) => {
            let p = cubarium_session::prepare_loaded(&cfg.sim, dir).map_err(anyhow::Error::msg)?;
            let mut sim = cubarium_session::create::ready_sim(p.world, p.flora, p.fauna, cfg.sim.sim_config());
            cubarium_session::create::set_water_every(&mut sim, cfg.sim.water_every);
            cubarium_session::create::open_closed_outlet(&mut sim);
            sim
        }
        None => {
            let (world, flora, fauna) = load_or_found(&cfg, seed, found_ticks, state.as_ref())?;
            let mut senses = cubarium_voxel_fauna::Senses::with_cue(fauna.config().cue);
            senses.settle(&world.view(), &flora.view());
            Sim::new(world, flora, fauna, cfg.sim.sim_config(), Some(senses))
        }
    };
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &cfg.sim.world)?;
    let mut sink = VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            models: cubarium::voxel::load_models(&cfg),
            ..VoxelGpuSinkOptions::default()
        },
    )?;
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
    let (mut sim_ms, mut copy_ms, mut tick_ms) = (Vec::new(), Vec::new(), Vec::new());
    use cubarium_voxel::profile::{self, Phase};
    let mut step_phases: Vec<Vec<f64>> = vec![Vec::new(); Phase::COUNT];
    eprintln!("sim pools: threads {}, water/bio {:?}", sim.config().threads, sim.config().overlap_split());
    let mut copier = SnapshotCopier::new(cfg.pack.pore_sweep);
    let mut snap = PackSnapshot::default();
    if overlap || snapshot_serial {
        let (world, flora, fauna) = sim.layers();
        copier.fill(&world.view(), flora.view(), fauna.view(), &mut snap);
    }
    for k in 0..ticks {
        if Some(k) == shower_at {
            let ok = sim.world_mut().force_rain(cubarium_voxel::weather::RainMode::Shower);
            eprintln!("pack_profile: a shower forced at tick {k}: {ok}");
        }
        let whole = Instant::now();
        let before: Vec<_> = Phase::ALL.iter().copied().map(profile::nanos).collect();
        let (ms, phases) = if overlap || snapshot_serial {
            let mut step = || {
                let t = Instant::now();
                sim.step();
                t.elapsed().as_secs_f64() * 1e3
            };
            // Bit-count parity balances both modes within each power-of-two tick
            // cadence. Simple odd/even alternation puts every plant tick in one mode.
            let (staged, step) = if snapshot_serial || (compare && k.count_ones() % 2 == 1) {
                let staged = sink.stage_snapshot(&mut snap);
                (staged, step())
            } else {
                sink.stage_snapshot_beside(&mut snap, step)
            };
            if !staged {
                bail!("the renderer refused a pack with no frame in flight");
            }
            sim_ms.push(step);
            let (world, flora, fauna) = sim.layers();
            copier.fill(&world.view(), flora.view(), fauna.view(), &mut snap);
            copy_ms.push(snap.copy_ms());
            let (ms, phases) = sink.last_pack();
            (ms, phases.to_vec())
        } else {
            let t = Instant::now();
            sim.step();
            sim_ms.push(t.elapsed().as_secs_f64() * 1e3);
            let t = Instant::now();
            if !sink.stage_world(sim.world(), sim.flora(), sim.fauna()) {
                bail!("the renderer refused a pack with no frame in flight");
            }
            (t.elapsed().as_secs_f64() * 1e3, pack_timer::phases())
        };
        tick_ms.push(whole.elapsed().as_secs_f64() * 1e3);
        for (p, ns) in Phase::ALL.iter().copied().zip(before) {
            step_phases[p.index()].push(profile::nanos(p).saturating_sub(ns) as f64 / 1e6);
        }
        let kib = sink.last_upload_kib();
        rows.push((sim.world().view().is_raining(), ms, kib, phases));
        sink.set_clock(sink.staged_tick().unwrap_or(0), 0.0);
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
    println!("mode {}", if compare { "compare" } else if overlap { "overlap" } else if snapshot_serial { "snapshot" } else { "serial" });
    println!("  sim step ms      {}", q(sim_ms.clone()));
    if overlap || snapshot_serial {
        println!("  copy ms          {}", q(copy_ms.clone()));
    }
    println!("  tick ms          {}", q(tick_ms.clone()));
    for p in Phase::ALL {
        let v = step_phases[p.index()].clone();
        if v.iter().any(|&ms| ms > 0.0) {
            println!("  step {:16} {}", p.name(), q(v));
        }
    }
    if compare {
        for (parity, label) in [(1, "snapshot serial"), (0, "snapshot overlap")] {
            let selected = |k: usize| k.count_ones() % 2 == parity;
            let select = |v: &[f64]| v.iter().copied().enumerate()
                .filter_map(|(k, ms)| selected(k).then_some(ms)).collect();
            println!("compare {label}");
            println!("  sim step ms      {}", q(select(&sim_ms)));
            println!("  pack ms          {}", q(rows.iter().enumerate()
                .filter_map(|(k, r)| selected(k).then_some(r.1)).collect()));
            println!("  copy ms          {}", q(select(&copy_ms)));
            println!("  tick ms          {}", q(select(&tick_ms)));
            for p in Phase::ALL {
                let v = select(&step_phases[p.index()]);
                if v.iter().any(|&ms| ms > 0.0) {
                    println!("  step {:16} {}", p.name(), q(v));
                }
            }
        }
    }
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
        && (d.join("world.bin").exists() || d.join("world.voxel").exists())
    {
        // The one safe load: the bodies get their heuristics back, or they rest.
        let s = cubarium_voxel_sim::saved::load(d, &[]).map_err(anyhow::Error::msg)?;
        return Ok((s.world, s.flora, s.fauna));
    }
    let params = params_of(cfg, &[])?;
    let founded = found_habitat(
        &cfg.sim.world,
        &LayerConfig::new(&params, cfg.sim.world.voxel_m),
        params.fauna.founder_counts(),
        Seeds::from(seed),
        false,
    );
    let (mut world, mut flora, mut fauna) = (founded.world, founded.flora, founded.fauna);
    if ticks > 0 {
        let mut senses = cubarium_voxel_fauna::Senses::with_cue(fauna.config().cue);
        senses.settle(&world.view(), &flora.view());
        let mut sim = Sim::new(world, flora, fauna, cfg.sim.sim_config(), Some(senses));
        for _ in 0..ticks {
            sim.step();
        }
        (world, flora, fauna) = (sim.world().clone(), sim.flora().clone(), sim.fauna().clone());
    }
    if let Some(d) = state {
        std::fs::create_dir_all(d)?;
        let minute = world.tick() / (60 * u64::from(cubarium_voxel_fauna::TICK_HZ));
        cubarium_voxel_sim::saved::save(d, &world, &flora, &fauna, minute).map_err(anyhow::Error::msg)?;
    }
    Ok((world, flora, fauna))
}
