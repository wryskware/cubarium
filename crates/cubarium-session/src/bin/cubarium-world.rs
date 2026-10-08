//! Headless save creation and unpaced advancement; no session loop or presentation.
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand};
use cubarium_observatory::query;
use cubarium_session::{SimParams, WorldSpec, create, prepare_loaded, prepare_new, store};
use cubarium_voxel::{DT, World, weather::DayMode};
use cubarium_voxel_sim::Sim;

#[derive(Parser)]
#[command(
    name = "cubarium-world",
    about = "Make and advance game-loadable worlds without Godot or a GPU",
    after_help = "Workers inherit the existing CPU affinity; this tool never widens it.\nFor studies: chrt -i 0 nice -n 19 scripts/run-study.py --slot 0 -- cubarium-world ..."
)]
struct Cli {
    /// Worker threads; 0 uses the config/default within the inherited CPU mask.
    #[arg(long, global = true, default_value_t = 0)]
    threads: usize,
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Found a world and immediately write a whole-world save with meta.json.
    Found {
        /// TOML or JSON WorldSpec: config, size, terrain, climate, seed, sets, empty.
        /// Unknown keys are refused. Paths in the spec are relative to the working directory.
        #[arg(long, required_unless_present = "config", conflicts_with = "config")]
        spec: Option<PathBuf>,
        /// Simulation config (the same config the game uses).
        #[arg(long, required_unless_present = "spec")]
        config: Option<PathBuf>,
        /// Override the terrain seed.
        #[arg(long)]
        seed: Option<u64>,
        /// Found terrain only, without plants or animals.
        #[arg(long)]
        empty: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Step unpaced and save each stop in command-line order.
    #[command(
        after_help = "Stops: tick:N is an absolute tick; minute:M is absolute simulated minutes since tick 0 (rounded up to a tick).\nHH:MM is the next crossing of that 24-hour clock time, strictly after the preceding stop/load.\nNames: dawn = sunrise; midday = midpoint of daylight; dusk = sunset; night = midpoint of the night following sunset.\nClock times/names require cycle mode and land within one tick. They wrap to the next day as needed.\nOutputs: PREFIX-tick-N, PREFIX-minute-M, PREFIX-HH-MM, or PREFIX-NAME, each with meta.json.\nDuplicate output names and stops earlier than the preceding stop are refused. Progress prints every 3 seconds; timing excludes loading/saving."
    )]
    Advance {
        #[arg(long)]
        load: PathBuf,
        /// Repeat for multiple checkpoints: tick:N, minute:M, HH:MM, dawn, midday, dusk, night.
        #[arg(long, required = true, value_parser = parse_stop)]
        stop: Vec<Stop>,
        #[arg(long)]
        out_prefix: PathBuf,
    },
}

#[derive(Clone, Debug)]
struct Stop {
    name: String,
    kind: StopKind,
}

#[derive(Clone, Debug)]
enum StopKind {
    Tick(u64),
    Phase(f64),
    Named(String),
}

fn age_tick(seconds: f64) -> Result<u64> {
    ensure!(
        seconds.is_finite() && seconds >= 0.0,
        "world age must be nonnegative and finite"
    );
    let ticks = (seconds / DT).ceil();
    ensure!(
        ticks.is_finite() && ticks >= 0.0 && ticks < u64::MAX as f64,
        "world age is out of range"
    );
    Ok(ticks as u64)
}

fn parse_stop(s: &str) -> Result<Stop, String> {
    let kind = if let Some(n) = s.strip_prefix("tick:") {
        StopKind::Tick(
            n.parse()
                .map_err(|_| "tick:N needs a nonnegative integer")?,
        )
    } else if let Some(n) = s.strip_prefix("minute:") {
        let minutes: f64 = n
            .parse()
            .map_err(|_| "minute:M needs nonnegative finite minutes")?;
        StopKind::Tick(age_tick(minutes * 60.0).map_err(|e| e.to_string())?)
    } else if ["dawn", "midday", "dusk", "night"].contains(&s) {
        StopKind::Named(s.to_string())
    } else {
        let bytes = s.as_bytes();
        if bytes.len() != 5
            || bytes[2] != b':'
            || !bytes
                .iter()
                .enumerate()
                .all(|(i, b)| i == 2 || b.is_ascii_digit())
        {
            return Err("expected tick:N, minute:M, HH:MM, dawn, midday, dusk, or night".into());
        }
        let h: u32 = s[..2].parse().unwrap();
        let m: u32 = s[3..].parse().unwrap();
        if h >= 24 || m >= 60 {
            return Err("HH:MM must be 00:00 through 23:59".into());
        }
        StopKind::Phase(f64::from(h * 60 + m) / 1440.0)
    };
    // Parsed tokens contain no path separators, including minute's numeric syntax.
    Ok(Stop {
        name: s.replace(':', "-"),
        kind,
    })
}

fn read_spec(path: &Path) -> Result<WorldSpec> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("read spec {}", path.display()))?;
    match path.extension().and_then(|e| e.to_str()) {
        Some("toml") => Ok(toml::from_str(&text).context("parse TOML WorldSpec")?),
        Some("json") => Ok(serde_json::from_str(&text).context("parse JSON WorldSpec")?),
        _ => bail!("spec must have a .toml or .json extension"),
    }
}

fn save(sim: &Sim, path: &Path) -> Result<()> {
    let (world, flora, fauna) = sim.layers();
    let meta = store::save_whole(path, world, flora, fauna)?;
    println!(
        "saved {} tick={} clock={}",
        path.display(),
        meta.tick,
        meta.time_of_day
    );
    Ok(())
}

enum Target {
    Tick(u64),
    Clock { remaining: f64, previous: f64 },
}

impl Target {
    fn new(stop: &Stop, world: &World) -> Result<Self> {
        if let StopKind::Tick(tick) = stop.kind {
            ensure!(
                tick >= world.tick(),
                "{} precedes current tick {}",
                stop.name,
                world.tick()
            );
            return Ok(Self::Tick(tick));
        }
        let clock = query::clock(world);
        ensure!(
            world.config().day.mode == DayMode::Cycle,
            "clock stops require cycle mode (this world uses {})",
            clock.mode
        );
        ensure!(
            clock.day_length_s.is_finite() && clock.day_length_s > 0.0,
            "clock needs a positive finite day length"
        );
        ensure!(
            DT < clock.day_length_s,
            "clock day must be longer than one tick"
        );
        let daylight = (clock.sunset_phase - clock.sunrise_phase).rem_euclid(1.0);
        let phase = match &stop.kind {
            StopKind::Phase(phase) => *phase,
            StopKind::Named(name) => match name.as_str() {
                "dawn" => clock.sunrise_phase,
                "midday" => clock.sunrise_phase + daylight / 2.0,
                "dusk" => clock.sunset_phase,
                "night" => clock.sunset_phase + (1.0 - daylight) / 2.0,
                _ => unreachable!(),
            }
            .rem_euclid(1.0),
            StopKind::Tick(_) => unreachable!(),
        };
        let distance = (phase - clock.day_phase).rem_euclid(1.0);
        Ok(Self::Clock {
            remaining: if distance < 1e-12 { 1.0 } else { distance },
            previous: clock.day_phase,
        })
    }

    fn reached(&mut self, world: &World) -> bool {
        match self {
            Self::Tick(tick) => world.tick() >= *tick,
            Self::Clock {
                remaining,
                previous,
            } => {
                let phase = world.day_phase();
                *remaining -= (phase - *previous).rem_euclid(1.0);
                *previous = phase;
                *remaining <= 1e-12
            }
        }
    }
}

fn advance(load: &Path, stops: &[Stop], prefix: &Path, threads: usize) -> Result<()> {
    let mut names = HashSet::new();
    for stop in stops {
        ensure!(
            names.insert(&stop.name),
            "duplicate output name {}",
            stop.name
        );
    }
    let exec = SimParams {
        threads,
        ..SimParams::default()
    };
    let prepared = prepare_loaded(&exec, load).map_err(anyhow::Error::msg)?;
    let mut sim = create::ready_sim(
        prepared.world,
        prepared.flora,
        prepared.fauna,
        exec.sim_config(),
    );
    create::open_closed_outlet(&mut sim);
    let mut step_time = Duration::ZERO;
    let mut steps = 0u64;
    let mut report = Instant::now();
    for stop in stops {
        let mut target = Target::new(stop, sim.world())?;
        while !target.reached(sim.world()) {
            let at = Instant::now();
            sim.step();
            step_time += at.elapsed();
            steps += 1;
            if report.elapsed() >= Duration::from_secs(3) {
                eprintln!(
                    "tick={} sim_s={:.3} ms/tick={:.3}",
                    sim.world().tick(),
                    sim.world().tick() as f64 * DT,
                    step_time.as_secs_f64() * 1000.0 / steps as f64
                );
                report = Instant::now();
            }
        }
        let mut path = prefix.as_os_str().to_os_string();
        path.push(format!("-{}", stop.name));
        save(&sim, Path::new(&path))?;
    }
    if steps > 0 {
        let seconds = step_time.as_secs_f64();
        println!(
            "advanced {steps} ticks in {seconds:.3}s: {:.3} ms/tick, {:.3} ticks/s (steps only)",
            seconds * 1000.0 / steps as f64,
            steps as f64 / seconds
        );
    }
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Action::Found {
            spec,
            config,
            seed,
            empty,
            out,
        } => {
            let mut spec = match spec {
                Some(path) => read_spec(&path)?,
                None => WorldSpec {
                    config,
                    ..WorldSpec::default()
                },
            };
            if seed.is_some() {
                spec.seed = seed;
            }
            spec.empty |= empty;
            let mut exec = spec.sim_params()?;
            if cli.threads > 0 {
                exec.threads = cli.threads;
            }
            let prepared = prepare_new(&exec, &spec)?;
            let meta = store::save_whole(&out, &prepared.world, &prepared.flora, &prepared.fauna)?;
            println!(
                "saved {} tick={} clock={}",
                out.display(),
                meta.tick,
                meta.time_of_day
            );
            Ok(())
        }
        Action::Advance {
            load,
            stop,
            out_prefix,
        } => advance(&load, &stop, &out_prefix, cli.threads),
    }
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("cubarium-world: {error:#}");
        std::process::exit(1);
    }
}
