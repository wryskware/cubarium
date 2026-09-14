//! `cubarium-search` — measure the real core's headless throughput, then search it.
//!
//! Four subcommands: `params` prints the searched box, `baseline` measures one actual-core run
//! (and optional independent-world CPU batching), `search` runs the bounded genetic search, and
//! `replay` re-runs one recorded row and checks that it reproduces bit for bit.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};
use cubarium_search::evaluate::{BUILD_ID, Protocol, Status, evaluate};
use cubarium_search::metrics::Scoring;
use cubarium_search::params;
use cubarium_search::search::{self, Budget, TRAINING_SEEDS, Variation};

#[derive(Parser, Debug)]
#[command(name = "cubarium-search", about = "Headless whole-ecosystem parameter search")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print the searched parameter box and everything deliberately excluded from it.
    Params,
    /// One or more actual-core runs, for throughput, peak memory and a default-parameter reference.
    Baseline {
        #[arg(long, default_value_t = 2_000)]
        ticks: u64,
        #[arg(long, default_value_t = 100)]
        sample_every: u64,
        #[arg(long, default_value_t = 2)]
        apex: u32,
        /// How many independent worlds to run at once. `1` is the single-world reference.
        #[arg(long, default_value_t = 1)]
        workers: usize,
        /// How many worlds in total. Defaults to `workers`.
        #[arg(long)]
        worlds: Option<usize>,
        #[arg(long, default_value_t = TRAINING_SEEDS[0])]
        seed: u64,
        /// Write the JSON report here as well as to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// The bounded genetic search.
    Search {
        #[arg(long, default_value = "smoke")]
        label: String,
        #[arg(long, default_value_t = 8)]
        evaluations: u64,
        #[arg(long, default_value_t = 2_000)]
        ticks: u64,
        #[arg(long, default_value_t = 100)]
        sample_every: u64,
        #[arg(long, default_value_t = 4)]
        workers: usize,
        #[arg(long, default_value_t = 600)]
        wall_seconds: u64,
        #[arg(long, default_value_t = 4)]
        population: usize,
        #[arg(long, default_value_t = 1)]
        elite: usize,
        #[arg(long, default_value_t = 3)]
        generations: u32,
        #[arg(long, default_value_t = 1)]
        seeds: usize,
        #[arg(long, default_value_t = 2)]
        apex: u32,
        #[arg(long, default_value_t = 4_096)]
        max_rows: u64,
        #[arg(long, default_value_t = 20_260_914)]
        search_seed: u64,
        /// Directory for `evals.jsonl` and `summary.json`.
        #[arg(long, default_value = "runs/ecology-search")]
        out: PathBuf,
    },
    /// Re-run one recorded row and check it reproduces.
    Replay {
        /// The `evals.jsonl` written by a search.
        #[arg(long)]
        record: PathBuf,
        /// Zero-based row index in that file.
        #[arg(long, default_value_t = 0)]
        index: usize,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Params => print_params(),
        Command::Baseline { ticks, sample_every, apex, workers, worlds, seed, out } => {
            baseline(ticks, sample_every, apex, workers, worlds.unwrap_or(workers), seed, out)
        }
        Command::Search {
            label,
            evaluations,
            ticks,
            sample_every,
            workers,
            wall_seconds,
            population,
            elite,
            generations,
            seeds,
            apex,
            max_rows,
            search_seed,
            out,
        } => run_search(
            &label,
            Protocol {
                horizon_ticks: ticks,
                sample_every,
                apex_founders: apex,
                apex_introduce_tick: 0,
            },
            Budget {
                max_evaluations: evaluations,
                workers,
                wall_seconds,
                population,
                elite,
                generations,
                seeds,
                max_rows,
            },
            search_seed,
            out,
        ),
        Command::Replay { record, index } => replay(&record, index),
    }
}

fn print_params() -> Result<(), Box<dyn std::error::Error>> {
    println!("build {BUILD_ID}");
    println!("\n{} searched parameters\n", params::PARAMS.len());
    println!("{:<34} {:>10} {:>10} {:>10}  why", "field", "default", "lo", "hi");
    for p in params::PARAMS {
        println!("{:<34} {:>10} {:>10} {:>10}  {}", p.name, p.default, p.lo, p.hi, p.why);
    }
    println!("\nidentified but not searched in this milestone\n");
    for (what, why) in params::EXCLUDED {
        println!("- {what}\n    {why}");
    }
    println!("\ntraining seeds {:?}", TRAINING_SEEDS);
    println!("held-out seeds {:?}", search::HELDOUT_SEEDS);
    Ok(())
}

/// Linux peak resident set size of this process, in MiB.
fn peak_rss_mib() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|k| k.parse::<f64>().ok()))
        })
        .map(|kib| kib / 1024.0)
        .unwrap_or(f64::NAN)
}

fn baseline(
    ticks: u64,
    sample_every: u64,
    apex: u32,
    workers: usize,
    worlds: usize,
    seed: u64,
    out: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if workers == 0 || workers > 32 {
        return Err("--workers must be in 1..=32".into());
    }
    if worlds == 0 {
        return Err("--worlds must be at least one".into());
    }
    let protocol = Protocol {
        horizon_ticks: ticks,
        sample_every,
        apex_founders: apex,
        apex_introduce_tick: 0,
    };
    protocol.validate()?;
    let values = params::defaults();
    let seeds: Vec<u64> = (0..worlds).map(|i| seed + i as u64).collect();

    let rss_before = peak_rss_mib();
    let start = Instant::now();
    let results = std::sync::Mutex::new(Vec::new());
    let cursor = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers.min(worlds) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if i >= seeds.len() {
                        return;
                    }
                    let e = evaluate(&values, seeds[i], protocol);
                    results.lock().expect("baseline mutex").push((i, e));
                }
            });
        }
    });
    let wall = start.elapsed().as_secs_f64();
    let rss_after = peak_rss_mib();

    let mut results = results.into_inner()?;
    results.sort_by_key(|(i, _)| *i);
    let simulated: u64 = results
        .iter()
        .map(|(_, e)| e.metrics.as_ref().map_or(0, |m| m.ticks_run))
        .sum();
    let scoring = Scoring::default();
    let report = serde_json::json!({
        "build_id": BUILD_ID,
        "protocol": protocol,
        "workers": workers,
        "worlds": worlds,
        "seeds": seeds,
        "wall_seconds": wall,
        "ticks_simulated": simulated,
        "ticks_per_second_total": simulated as f64 / wall,
        "ticks_per_second_per_world": simulated as f64 / wall / worlds as f64,
        "simulated_seconds_per_wall_second": simulated as f64 * cubarium_core::DT / wall,
        "peak_rss_mib_before": rss_before,
        "peak_rss_mib_after": rss_after,
        "peak_rss_mib_per_world": (rss_after - rss_before).max(0.0) / worlds as f64,
        "runs": results.iter().map(|(i, e)| serde_json::json!({
            "seed": seeds[*i],
            "status": e.status,
            "reason": e.reason,
            "elapsed_ms": e.elapsed_ms,
            "apex_material_in": e.apex_material_in,
            "apex_energy_in": e.apex_energy_in,
            "fitness": e.metrics.as_ref().map(|m| m.fitness(&scoring)),
            "objectives": e.metrics.as_ref().map(|m| m.objectives(&scoring)),
            "metrics": e.metrics,
        })).collect::<Vec<_>>(),
    });
    let text = serde_json::to_string_pretty(&report)?;
    println!("{text}");
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
    }
    Ok(())
}

fn run_search(
    label: &str,
    protocol: Protocol,
    budget: Budget,
    search_seed: u64,
    out: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = out.join(label);
    std::fs::create_dir_all(&dir)?;
    let rows_path = dir.join("evals.jsonl");
    let mut rows = BufWriter::new(
        OpenOptions::new().create(true).write(true).truncate(true).open(&rows_path)?,
    );
    let mut write_error = None;

    let report = search::run(
        protocol,
        budget,
        Variation::default(),
        Scoring::default(),
        search_seed,
        |row| {
            if write_error.is_none()
                && let Err(e) = serde_json::to_writer(&mut rows, row).and_then(|()| {
                    rows.write_all(b"\n").map_err(serde_json::Error::io)
                })
            {
                write_error = Some(e);
            }
        },
    )?;
    rows.flush()?;
    if let Some(e) = write_error {
        return Err(Box::new(e));
    }

    let summary_path = dir.join("summary.json");
    std::fs::write(&summary_path, serde_json::to_string_pretty(&report)?)?;

    println!(
        "stopped: {:?} after {} generation(s), {} simulation(s), {} cached reuse(s), {:.1}s wall",
        report.stop_reason,
        report.generations_run,
        report.evaluations_run,
        report.cached_reuses,
        report.wall_seconds
    );
    println!(
        "{} ticks simulated, {:.0} ticks/s aggregate over {} worker(s)",
        report.ticks_simulated, report.ticks_per_second, report.budget.workers
    );
    println!("\ncandidate  gen origin      status                fitness  pers plnt turn matr lin  vary apex");
    for c in &report.candidates {
        let status = if c.completed + c.invalid + c.failed == 0 {
            "unevaluated".to_string()
        } else if c.failed > 0 {
            format!("failed({})", c.failed)
        } else if c.invalid > 0 {
            format!("invalid({})", c.invalid)
        } else {
            format!("completed({})", c.completed)
        };
        match (c.fitness, c.objectives) {
            (Some(f), Some(o)) => {
                let a = o.as_array();
                println!(
                    "{:>9}  {:>3} {:<11} {:<18} {:>8.4}  {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
                    c.candidate, c.generation, c.origin, status, f,
                    a[0], a[1], a[2], a[3], a[4], a[5], a[6]
                );
            }
            _ => println!(
                "{:>9}  {:>3} {:<11} {:<18} {:>8}  {}",
                c.candidate,
                c.generation,
                c.origin,
                status,
                "-",
                c.reason.as_deref().unwrap_or("not evaluated (budget)")
            ),
        }
    }
    println!(
        "\nnondominated candidates: {:?}",
        report.nondominated.iter().map(|i| report.candidates[*i].candidate).collect::<Vec<_>>()
    );
    if let Some(best) = report.best_by_scalar {
        println!("best by scalar: candidate {}", report.candidates[best].candidate);
    }
    println!("\nrows    {}", rows_path.display());
    println!("summary {}", summary_path.display());
    println!(
        "replay  cargo run --release -p cubarium-search -- replay --record {} --index 0",
        rows_path.display()
    );
    Ok(())
}

fn replay(record: &PathBuf, index: usize) -> Result<(), Box<dyn std::error::Error>> {
    let line = BufReader::new(File::open(record)?)
        .lines()
        .nth(index)
        .ok_or_else(|| format!("{} has no row {index}", record.display()))??;
    let row: serde_json::Value = serde_json::from_str(&line)?;

    // The exact bit patterns, not the readable decimals: `serde_json` 1.0.151 does not
    // guarantee that a decimal round trip returns the same `f64`, and one ULP on a growth rate
    // is a different world. This was caught by `replay` itself diverging on a recorded row.
    let bits: Vec<String> = row["param_bits"]
        .as_array()
        .ok_or("row has no param_bits: it was written by a build before exact replay")?
        .iter()
        .map(|v| v.as_str().map(str::to_string).ok_or("param_bits holds a non-string"))
        .collect::<Result<_, _>>()?;
    let values = params::from_bit_labels(&bits)?;
    let recorded_fingerprint = row["param_fingerprint"].as_u64().ok_or("row has no fingerprint")?;
    if params::fingerprint(&values) != recorded_fingerprint {
        return Err(format!(
            "row {index} fingerprint {recorded_fingerprint} does not match the vector it records"
        )
        .into());
    }
    let seed = row["seed"].as_u64().ok_or("row has no seed")?;
    let protocol: Protocol = serde_json::from_value(row["protocol"].clone())?;
    let recorded_build = row["build_id"].as_str().unwrap_or("unknown");
    if recorded_build != BUILD_ID {
        println!("warning: row was produced by build {recorded_build}, this is {BUILD_ID}");
    }

    let again = evaluate(&values, seed, protocol);
    let recorded_status: Status = serde_json::from_value(row["status"].clone())?;
    let recorded_hash = row["metrics"]["final_ecology_hash"].as_u64();
    let replayed_hash = again.metrics.as_ref().map(|m| m.final_ecology_hash);

    println!("status   recorded {recorded_status:?}  replayed {:?}", again.status);
    println!("ecology  recorded {recorded_hash:?}  replayed {replayed_hash:?}");
    let matched = recorded_status == again.status && recorded_hash == replayed_hash;
    println!("{}", if matched { "REPRODUCED" } else { "DIVERGED" });
    if !matched {
        return Err("replay did not reproduce the recorded row".into());
    }
    Ok(())
}
