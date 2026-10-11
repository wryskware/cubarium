//! Short live-tick measurements on the same fully founded worlds as the desktop.
//! Run the release executable through `scripts/run-study.py --slot N -- ...`.
//! Example: `tick_profile --config config/desktop/terrarium.toml --size 288x138x48
//! --seeds 5,17,29 --threads 8 --warmup 10 --ticks 50 --windows 3`.
//! JSONL goes to stdout; founding diagnostics go to stderr. No worlds are saved.

use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use cubarium_session::{WorldSpec, prepare_new};
use cubarium_voxel::profile::{self, Phase};
use cubarium_voxel_fauna::{Founder, TICK_HZ};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::Sim;
use serde_json::{Value, json};

#[derive(Parser)]
#[command(about = "Measure short live-tick windows on shipped, fully founded ecosystems")]
struct Args {
    #[arg(long, default_value = "config/desktop/terrarium.toml")]
    config: PathBuf,
    /// WIDTHxHEIGHTxDEPTH, overriding only the shipped world's dimensions.
    #[arg(long, value_parser = parse_size)]
    size: Option<[u32; 3]>,
    #[arg(long, value_delimiter = ',', default_value = "5")]
    seeds: Vec<u64>,
    #[arg(long, default_value = "8")]
    threads: NonZeroUsize,
    /// Full ticks before the measured windows; founding has its own normal pre-roll.
    #[arg(long, default_value = "5")]
    warmup: u64,
    /// Full ticks in each measured window.
    #[arg(long, default_value = "20")]
    ticks: NonZeroU64,
    #[arg(long, default_value = "1")]
    windows: NonZeroU32,
}

fn parse_size(text: &str) -> Result<[u32; 3], String> {
    let dimensions = text
        .split(['x', ','])
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "size must be three positive voxel counts: WIDTHxHEIGHTxDEPTH".to_string())?;
    let size: [u32; 3] = dimensions
        .try_into()
        .map_err(|_| "size must contain exactly three voxel counts".to_string())?;
    if size.contains(&0) {
        return Err("every size dimension must be positive".to_string());
    }
    Ok(size)
}

fn emit(out: &mut impl Write, row: Value) -> Result<()> {
    serde_json::to_writer(&mut *out, &row)?;
    writeln!(out)?;
    out.flush()?;
    Ok(())
}

fn roster(sim: &Sim) -> Value {
    let fauna = sim.fauna().view();
    let flora = sim.flora().view();
    let mut animals: BTreeMap<&str, usize> =
        Founder::ALL.into_iter().map(|f| (f.name(), 0)).collect();
    let mut plants: BTreeMap<&str, usize> =
        Species::ALL.into_iter().map(|s| (s.name(), 0)).collect();
    for animal in fauna.animals {
        *animals
            .entry(animal.founder.map_or("unassigned", |f| f.name()))
            .or_default() += 1;
    }
    for stand in flora.stands {
        *plants.entry(stand.species.name()).or_default() += 1;
    }
    json!({
        "animals": fauna.animals.len(), "animals_by_lineage": animals,
        "stands": flora.stands.len(), "stands_by_species": plants,
        "clutches": fauna.clutches.len(), "ground_sites": flora.ground.len(),
    })
}

fn main() -> Result<()> {
    let args = Args::parse();
    let measured_ticks = args
        .ticks
        .get()
        .checked_mul(u64::from(args.windows.get()))
        .context("ticks × windows exceeds the tick counter")?;
    let all_start = Instant::now();
    cubarium_voxel::set_thread_override(args.threads.get());
    cubarium_voxel_sim::prepare_compute_pool(args.threads.get());
    let mut out = BufWriter::new(std::io::stdout().lock());
    emit(
        &mut out,
        json!({
            "kind": "run", "config": args.config, "size": args.size, "seeds": args.seeds,
            "threads": args.threads.get(), "water_every": 1, "overlap": true,
            "tick_hz": TICK_HZ, "warmup_ticks": args.warmup,
            "ticks_per_window": args.ticks.get(), "windows": args.windows.get(),
        }),
    )?;
    for &seed in &args.seeds {
        let round_start = Instant::now();
        let spec = WorldSpec {
            config: Some(args.config.clone()),
            size: args.size,
            seed: Some(seed),
            ..WorldSpec::default()
        };
        let mut params = spec.sim_params()?;
        params.threads = args.threads.get();
        params.water_every = 1;
        let founding_start = Instant::now();
        let prepared = prepare_new(&params, &spec).with_context(|| format!("found seed {seed}"))?;
        let founding_s = founding_start.elapsed().as_secs_f64();
        let label = prepared.label;
        let mut world = prepared.world;
        world.set_water_every(1);
        let mut sim = Sim::ambient(world, prepared.flora, prepared.fauna, params.sim_config());
        let startup_s = round_start.elapsed().as_secs_f64();
        let cfg = sim.world().config();
        let size = [cfg.width, cfg.height, cfg.depth];
        let start_roster = roster(&sim);
        emit(
            &mut out,
            json!({
                "kind": "startup", "seed": seed, "label": label, "size": size,
                "cells": u64::from(size[0]) * u64::from(size[1]) * u64::from(size[2]),
                "founding_s": founding_s, "startup_s": startup_s,
                "tick": sim.world().tick(), "roster": start_roster,
                "resolved_config": params,
                "threads": sim.config().threads, "overlap": sim.config().overlap,
                "water_every": sim.world().water_every(),
                "plant_step_every": sim.flora().config().step_every,
            }),
        )?;
        let warm_start = Instant::now();
        for _ in 0..args.warmup {
            sim.step();
        }
        let warmup_s = warm_start.elapsed().as_secs_f64();
        let mut measured_s = 0.0;
        for window in 0..args.windows.get() {
            let tick_start = sim.world().tick();
            let roster_start = roster(&sim);
            profile::reset();
            let tick_timer = Instant::now();
            for _ in 0..args.ticks.get() {
                sim.step();
            }
            let wall_s = tick_timer.elapsed().as_secs_f64();
            measured_s += wall_s;
            let phases: Vec<Value> = Phase::ALL
                .into_iter()
                .map(|phase| {
                    let nanos = profile::nanos(phase);
                    json!({
                        "phase": format!("{phase:?}"), "name": phase.name().trim(),
                        "nested_total": phase.is_total(), "nanos": nanos,
                        "calls": profile::calls(phase),
                        "ms_per_tick": nanos as f64 / 1e6 / args.ticks.get() as f64,
                        "wall_fraction": nanos as f64 / 1e9 / wall_s,
                    })
                })
                .collect();
            emit(
                &mut out,
                json!({
                    "kind": "window", "seed": seed, "size": size, "window": window,
                    "tick_start": tick_start, "tick_end": sim.world().tick(),
                    "ticks": args.ticks.get(), "wall_s": wall_s,
                    "ms_per_tick": wall_s * 1e3 / args.ticks.get() as f64,
                    "realtime_x": args.ticks.get() as f64 / f64::from(TICK_HZ) / wall_s,
                    "roster_start": roster_start, "roster_end": roster(&sim), "phases": phases,
                }),
            )?;
        }
        emit(
            &mut out,
            json!({
                "kind": "world", "seed": seed, "size": size, "ticks": measured_ticks,
                "founding_s": founding_s, "startup_s": startup_s, "warmup_s": warmup_s,
                "measured_s": measured_s, "ms_per_tick": measured_s * 1e3 / measured_ticks as f64,
                "all_round_wall_s": round_start.elapsed().as_secs_f64(),
                "roster_start": start_roster, "roster_end": roster(&sim),
            }),
        )?;
    }
    emit(
        &mut out,
        json!({"kind": "end", "all_wall_s": all_start.elapsed().as_secs_f64()}),
    )
}
