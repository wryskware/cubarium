//! `cubarium-search` — measure the real core's headless throughput, then search it.
//!
//! M1 subcommands: `params` prints the searched box, `baseline` measures one actual-core run
//! (and optional independent-world CPU batching), `search` runs the bounded genetic search, and
//! `replay` re-runs one recorded row and checks that it reproduces bit for bit.
//!
//! The ecology v1 calibration adds three: `calibrate-candidates` prints the declared screen,
//! `calibrate` runs one stage of the candidate × seed × apex-arm matrix, and
//! `calibrate-export` writes a selected candidate as a `cubarium run --config` TOML. Their
//! rows carry the same `param_bits` shape, so `replay` reads them too.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};
use cubarium_search::calibrate;
use cubarium_search::es;
use cubarium_search::evaluate::{BUILD_ID, Protocol, Status, evaluate};
use cubarium_search::factorial;
use cubarium_search::metrics::Scoring;
use cubarium_search::params;
use cubarium_search::population;
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
    /// Print the frozen R2a training protocol: optimizer, score, layouts and hashes.
    EsProtocol {
        /// The ecology every fixture world is built on: a `cubarium run --config` TOML, such
        /// as the calibration's selected configuration. Omitted means the shipped defaults.
        /// It moves every layout hash and the protocol hash.
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Run the three fixture controls on every training layout.
    EsControls {
        #[arg(long, default_value_t = 2)]
        workers: usize,
        #[arg(long, default_value_t = 120)]
        wall_seconds: u64,
        /// The ecology every fixture world is built on: a `cubarium run --config` TOML, such
        /// as the calibration's selected configuration. Omitted means the shipped defaults.
        /// It moves every layout hash and the protocol hash.
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// The ES plumbing smoke: four episodes, then the same four at another worker count.
    EsSmoke {
        /// The ecology every fixture world is built on: a `cubarium run --config` TOML, such
        /// as the calibration's selected configuration. Omitted means the shipped defaults.
        /// It moves every layout hash and the protocol hash.
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Measure single-animal episode throughput.
    EsBench {
        #[arg(long, default_value_t = 4_000)]
        ticks: u64,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// The ecology every fixture world is built on: a `cubarium run --config` TOML, such
        /// as the calibration's selected configuration. Omitted means the shipped defaults.
        /// It moves every layout hash and the protocol hash.
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// The evolution-strategy learning run.
    EsTrain {
        #[arg(long, default_value_t = 16)]
        pairs: usize,
        #[arg(long, default_value_t = 16)]
        generations: u64,
        #[arg(long, default_value_t = cubarium_search::es::HORIZON_TICKS)]
        horizon: u64,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value_t = 1_200)]
        wall_seconds: u64,
        #[arg(long, default_value_t = 20_260_915)]
        train_seed: u64,
        /// Evaluate the unperturbed centre each generation. A sampled perturbation's score is
        /// not the centre's.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        center_eval: bool,
        /// How per-layout survival combines into the score: `min` (the frozen R2a `t_min`) or
        /// `mean`. Anything but `min` is a different task with a different protocol hash.
        #[arg(long, default_value = "min")]
        aggregate: String,
        /// Continue from a checkpoint written at a completed generation boundary.
        #[arg(long)]
        resume: Option<PathBuf>,
        /// Discard an existing run in `--out` and start fresh. Without it a fresh run into a
        /// directory that already holds one is refused rather than truncating its history.
        #[arg(long, default_value_t = false)]
        overwrite: bool,
        /// The ecology every fixture world is built on: a `cubarium run --config` TOML, such
        /// as the calibration's selected configuration. Omitted means the shipped defaults.
        /// It moves every layout hash and the protocol hash.
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long, default_value = "runs/es-first")]
        out: PathBuf,
    },
    /// Export a checkpoint's centre as a policy and verify it round-trips and runs.
    EsExport {
        #[arg(long)]
        checkpoint: PathBuf,
        /// The ecology every fixture world is built on: a `cubarium run --config` TOML, such
        /// as the calibration's selected configuration. Omitted means the shipped defaults.
        /// It moves every layout hash and the protocol hash.
        #[arg(long)]
        config: Option<PathBuf>,

        /// Which recorded centre to export. The run's selection rule is the highest recorded
        /// centre score, which is rarely the last generation. Omitted exports the run's final
        /// centre.
        #[arg(long)]
        generation: Option<u64>,
        #[arg(long, default_value = "runs/es-first/policy.json")]
        out: PathBuf,
        #[arg(long, default_value_t = 200)]
        verify_ticks: u64,
    },
    /// Evaluate one saved centre/policy file on the training or held-out layout set.
    EsEvaluate {
        #[arg(long)]
        policy: PathBuf,
        /// `training` or `holdout`.
        #[arg(long, default_value = "holdout")]
        set: String,
        #[arg(long, default_value_t = cubarium_search::es::HORIZON_TICKS)]
        horizon: u64,
        #[arg(long, default_value_t = 300)]
        wall_seconds: u64,
        /// Diagnostic: zero every animal's hidden state every N ticks.
        #[arg(long)]
        reset_hidden_every: Option<u64>,
        /// Diagnostic: how many identical animals share the arena (1 = the plain rollout).
        #[arg(long, default_value_t = 1)]
        copies: usize,
        /// The ecology every fixture world is built on: a `cubarium run --config` TOML, such
        /// as the calibration's selected configuration. Omitted means the shipped defaults.
        /// It moves every layout hash and the protocol hash.
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    /// The population-level comparison: `N` copies of one trained policy in a whole
    /// calibrated world, against the same world with `N` ordinary copies of the same body.
    EsPopulation {
        /// The exported policy. Its recorded config hash must be `--config`'s.
        #[arg(long)]
        policy: PathBuf,
        /// The ecology the world is built on. Required: this command has no default world.
        #[arg(long)]
        config: PathBuf,
        /// How many of `TRAINING_SEEDS`, from the front.
        #[arg(long, default_value_t = 2)]
        seeds: usize,
        /// Comma-separated apex arms: how many adults are introduced.
        #[arg(long, default_value = "0,1,2")]
        arms: String,
        /// How many copies of the policy's body each arm imports at tick 0.
        #[arg(long, default_value_t = 4)]
        copies: usize,
        #[arg(long, default_value_t = 180_000)]
        ticks: u64,
        #[arg(long, default_value_t = 600)]
        sample_every: u64,
        /// The tick every arm introduces its apex cohort on.
        #[arg(long, default_value_t = 6_000)]
        introduce_tick: u64,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Hard wall cap. Trials not started by then are recorded as skipped, never extended.
        #[arg(long, default_value_t = 600)]
        wall_seconds: u64,
        #[arg(long, default_value = "runs/es-eco-v1-fastleaf/population")]
        out: PathBuf,
    },
    /// The matched feasibility experiment: four drivers on the same twelve layouts, measured
    /// with the world's own per-organism store ledger.
    EsBudget {
        /// The trained policy to compare. Its recorded config hash must be `--config`'s.
        #[arg(long, default_value = "runs/es-eco-v1-fastleaf/selected/center-00009-policy.json")]
        policy: PathBuf,
        /// The ecology every fixture world is built on. Required: this experiment is about one
        /// named ecology, and a silent default would make the comparison meaningless.
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value_t = cubarium_search::es::HORIZON_TICKS)]
        horizon: u64,
        /// The training seed whose unperturbed centre is the fourth driver: the policy the ES
        /// run actually started from.
        #[arg(long, default_value_t = 20_260_915)]
        initial_seed: u64,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value_t = 900)]
        wall_seconds: u64,
        #[arg(long, default_value = "runs/ecology-v1-budget/feasibility.json")]
        out: PathBuf,
    },
    /// Re-run the two-apex arm and report why no two adults mated: readiness overlap, the
    /// minimum ready-pair distance, and the first failing predicate per candidate pair.
    ApexAudit {
        /// One or more `cubarium run --config` TOMLs, comma-separated. The screen's two are
        /// `runs/ecology-v1-calibration/selected/{baseline,fast-leaf}.toml`.
        #[arg(long)]
        config: String,
        /// How many of `HELDOUT_SEEDS`, from the front.
        #[arg(long, default_value_t = 4)]
        seeds: usize,
        /// Adults introduced. The screen's two-apex arm is `2`.
        #[arg(long, default_value_t = 2)]
        apex: u32,
        #[arg(long, default_value_t = 180_000)]
        ticks: u64,
        /// The tick the cohort is introduced on, exactly as the screen introduced it.
        #[arg(long, default_value_t = 6_000)]
        introduce_tick: u64,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value_t = 600)]
        wall_seconds: u64,
        #[arg(long, default_value = "runs/ecology-v1-budget/apex-audit.json")]
        out: PathBuf,
    },
    /// Print every declared ecology v1 calibration candidate and what it moves.
    CalibrateCandidates,
    /// Run one stage of the ecology v1 calibration matrix: declared candidates × seeds ×
    /// matched zero/one/two-apex arms, one JSONL row per run.
    Calibrate {
        /// A name for this stage, and the subdirectory it writes into.
        #[arg(long, default_value = "screen")]
        stage: String,
        /// Comma-separated candidate names, or `all`.
        #[arg(long, default_value = "all")]
        candidates: String,
        /// `training` or `holdout`. The held-out set is for the final validation only.
        #[arg(long, default_value = "training")]
        seed_set: String,
        /// How many seeds of that set, from the front.
        #[arg(long, default_value_t = 6)]
        seeds: usize,
        /// Comma-separated apex arms: how many adults are introduced.
        #[arg(long, default_value = "0,1,2")]
        arms: String,
        /// Comma-separated `organism.move_cost` levels, in e per unit of structure per pixel
        /// travelled. The matrix is crossed with them, so one level is the calibration
        /// screen's own matrix. The default is the shipped price.
        #[arg(long, default_value = "0.00036")]
        prices: String,
        #[arg(long, default_value_t = 125_000)]
        ticks: u64,
        #[arg(long, default_value_t = 500)]
        sample_every: u64,
        /// The tick every arm introduces its cohort on.
        #[arg(long, default_value_t = 6_000)]
        introduce_tick: u64,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Hard wall cap. Trials not started by then are recorded as skipped, never extended.
        #[arg(long, default_value_t = 1_800)]
        wall_seconds: u64,
        #[arg(long, default_value = "runs/ecology-v1-calibration")]
        out: PathBuf,
    },
    /// Export one candidate as a complete `WorldConfig` TOML `cubarium run --config` accepts.
    CalibrateExport {
        #[arg(long)]
        candidate: String,
        /// The seed written into the exported config.
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Whether this is the calibration's selected configuration or a fallback export.
        #[arg(long, default_value_t = false)]
        selected: bool,
        /// One line saying why, recorded beside the config.
        #[arg(long, default_value = "")]
        why: String,
        #[arg(long, default_value = "runs/ecology-v1-calibration/selected")]
        out: PathBuf,
    },
    /// Run the controlled form x diet factorial: cloned founders at matched cells in whole
    /// `fast-leaf` worlds, mutation and reproduction off for the clones, measured with the
    /// core's per-body ledger (workstream J).
    Factorial {
        /// Comma-separated arms: `A` (diet within body), `B` (body within diet), `C` (the
        /// founder pairing).
        #[arg(long, default_value = "A,B,C")]
        arms: String,
        /// `training` or `holdout`. The held-out set is for the final validation only.
        #[arg(long, default_value = "training")]
        seed_set: String,
        /// How many seeds of that set, from the front.
        #[arg(long, default_value_t = 4)]
        seeds: usize,
        #[arg(long, default_value_t = 90_000)]
        ticks: u64,
        /// Ticks of the clone-free warm-up that finds each seed's pools. A world is created
        /// dry; the basins only exist once rain has arrived.
        #[arg(long, default_value_t = 24_000)]
        warm_up_ticks: u64,
        /// Ticks between probes of where each clone is standing (20 = one simulated second).
        #[arg(long, default_value_t = 20)]
        probe_every: u64,
        /// Ticks between drains of the core's 4,096-record closed-ledger buffer.
        #[arg(long, default_value_t = 250)]
        drain_every: u64,
        /// Mean warm-up water depth at or above which a cell is the wet stratum (d). The
        /// default is where a non-swimmer starts paying a measurable wading penalty.
        #[arg(long, default_value_t = 0.05)]
        wet_min: f64,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Print the eight cells each seed's landscape offers, and run nothing.
        #[arg(long, default_value_t = false)]
        cells_only: bool,
        #[arg(long, default_value = "runs/ecology-v1-diet-factorial")]
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
        Command::CalibrateCandidates => {
            calibrate::print_candidates();
            Ok(())
        }
        Command::Calibrate {
            stage,
            candidates,
            seed_set,
            seeds,
            arms,
            prices,
            ticks,
            sample_every,
            introduce_tick,
            workers,
            wall_seconds,
            out,
        } => {
            let names: Vec<String> = if candidates == "all" {
                calibrate::CANDIDATES.iter().map(|c| c.name.to_string()).collect()
            } else {
                candidates.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
            };
            let arms: Vec<u32> = arms
                .split(',')
                .map(|s| s.trim().parse::<u32>())
                .collect::<Result<_, _>>()?;
            let prices: Vec<f64> = prices
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::parse::<f64>)
                .collect::<Result<_, _>>()?;
            let set = calibrate::SeedSet::parse(&seed_set)?;
            let dir = out.join(&stage);
            let report = calibrate::run_stage(
                &stage,
                &names,
                set,
                seeds,
                &arms,
                &prices,
                ticks,
                sample_every,
                introduce_tick,
                workers,
                wall_seconds,
                &dir,
            )?;
            calibrate::print_report(&report);
            println!("\nrows    {}", dir.join("evals.jsonl").display());
            println!("summary {}", dir.join("summary.json").display());
            println!(
                "replay  cargo run --release -p cubarium-search -- replay --record {} --index 0",
                dir.join("evals.jsonl").display()
            );
            Ok(())
        }
        Command::CalibrateExport { candidate, seed, selected, why, out } => {
            let path = calibrate::export(&candidate, seed, &out, selected, &why)?;
            let config = calibrate::candidate(&candidate)
                .ok_or_else(|| format!("{candidate} is not a declared candidate"))?
                .config(seed)?;
            println!("exported {}", path.display());
            println!("config hash {:016x}", calibrate::config_hash(&config));
            println!("build {BUILD_ID}");
            println!("load with: cubarium run --config {}", path.display());
            Ok(())
        }
        Command::Factorial {
            arms,
            seed_set,
            seeds,
            ticks,
            warm_up_ticks,
            probe_every,
            drain_every,
            wet_min,
            workers,
            cells_only,
            out,
        } => factorial_command(
            &arms,
            &seed_set,
            seeds,
            factorial::Design { ticks, warm_up_ticks, probe_every, drain_every, wet_min },
            workers,
            cells_only,
            &out,
        ),
        Command::Replay { record, index } => replay(&record, index),
        Command::EsProtocol { config } => es::commands::protocol(config),
        Command::EsControls { workers, wall_seconds, config, out } => {
            es::commands::controls(workers, wall_seconds, config, out)
        }
        Command::EsSmoke { config, out } => es::commands::smoke(config, out),
        Command::EsBench { ticks, workers, config } => es::commands::bench(ticks, workers, config),
        Command::EsTrain {
            pairs,
            generations,
            horizon,
            workers,
            wall_seconds,
            train_seed,
            center_eval,
            aggregate,
            resume,
            overwrite,
            config,
            out,
        } => es::commands::train(
            pairs,
            generations,
            horizon,
            workers,
            wall_seconds,
            train_seed,
            center_eval,
            es::trainer::Aggregate::parse(&aggregate)?,
            resume,
            overwrite,
            config,
            out,
        ),
        Command::EsEvaluate {
            policy,
            set,
            horizon,
            wall_seconds,
            reset_hidden_every,
            copies,
            config,
            out,
        } => {
            let probe = es::commands::EvalProbe { reset_hidden_every, copies };
            es::commands::evaluate(policy, &set, horizon, wall_seconds, probe, config, out)
        }
        Command::EsBudget { policy, config, horizon, initial_seed, workers, wall_seconds, out } => {
            es::budget::run(policy, config, horizon, initial_seed, workers, wall_seconds, out)
        }
        Command::ApexAudit { config, seeds, apex, ticks, introduce_tick, workers, wall_seconds, out } => {
            cubarium_search::apex_audit::run(
                config.split(',').map(|s| PathBuf::from(s.trim())).filter(|p| !p.as_os_str().is_empty()).collect(),
                seeds, apex, ticks, introduce_tick, workers, wall_seconds, out,
            )
        }
        Command::EsExport { checkpoint, config, generation, out, verify_ticks } => {
            es::commands::export(checkpoint, config, generation, out, verify_ticks)
        }
        Command::EsPopulation {
            policy,
            config,
            seeds,
            arms,
            copies,
            ticks,
            sample_every,
            introduce_tick,
            workers,
            wall_seconds,
            out,
        } => {
            let ecology = es::Ecology::load(&config)?;
            let arms: Vec<u32> =
                arms.split(',').map(|s| s.trim().parse::<u32>()).collect::<Result<_, _>>()?;
            let seeds: Vec<u64> = TRAINING_SEEDS
                .get(..seeds)
                .ok_or_else(|| format!("seed count {seeds} is outside 1..={}", TRAINING_SEEDS.len()))?
                .to_vec();
            let report = population::run_stage(
                &ecology,
                &policy,
                &seeds,
                &arms,
                copies,
                ticks,
                sample_every,
                introduce_tick,
                workers,
                wall_seconds,
                &out,
            )?;
            population::print_report(&report);
            println!("\nrows    {}", out.join("rows.jsonl").display());
            println!("summary {}", out.join("summary.json").display());
            Ok(())
        }
    }
}

/// The controlled form × diet factorial (workstream J). `--cells-only` prints the eight cells
/// each seed's own landscape offers and runs nothing, which is how a design is checked before
/// compute is spent on it.
fn factorial_command(
    arms: &str,
    seed_set: &str,
    seeds: usize,
    design: factorial::Design,
    workers: usize,
    cells_only: bool,
    out: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let arms: Vec<factorial::Arm> =
        arms.split(',').map(str::trim).filter(|s| !s.is_empty()).map(factorial::Arm::parse).collect::<
            Result<_, _>,
        >()?;
    let seeds = calibrate::SeedSet::parse(seed_set)?.seeds(seeds)?;
    println!("build {BUILD_ID}");
    println!("ecology {}", factorial::ECOLOGY);
    println!("seeds {seeds:?}  arms {:?}", arms.iter().map(|a| a.label()).collect::<Vec<_>>());
    println!(
        "design ticks {} warm-up {} probe/{} drain/{}",
        design.ticks, design.warm_up_ticks, design.probe_every, design.drain_every
    );

    if cells_only {
        for seed in &seeds {
            let config = factorial::ecology_config(*seed)?;
            let land = factorial::warm_up(&config, design.warm_up_ticks)?;
            println!("\nseed {seed}");
            println!("| depth band (d) | cells | of those, carrying foliage |");
            println!("| --- | --- | --- |");
            for (hi, cells, fed) in factorial::landscape_census(&land) {
                println!("| <= {hi} | {cells} | {fed} |");
            }
            let cells = factorial::choose_cells(&land, design.wet_min)?;
            println!();
            println!("| slot | cell | face | class | mean depth | mean foliage | mean litter |");
            println!("| --- | --- | --- | --- | --- | --- | --- |");
            for (slot, p) in cells.iter().enumerate() {
                let cell = cubarium_surface::CellId(p.cell);
                println!(
                    "| {slot} | {} | {:?} ({},{}) | {} | {:.4} | {:.4} | {:.4} |",
                    p.cell,
                    cell.face(),
                    cell.cx(),
                    cell.cy(),
                    if p.wet { "wet" } else { "dry" },
                    p.mean_depth,
                    p.mean_foliage,
                    p.mean_litter
                );
            }
        }
        return Ok(());
    }

    let report = factorial::run(&arms, &seeds, design, workers, out)?;
    let rows: Vec<factorial::ArmRun> = {
        let text = std::fs::read_to_string(out.join("runs.jsonl"))?;
        let mut rows: Vec<factorial::ArmRun> =
            text.lines().filter(|l| !l.is_empty()).map(serde_json::from_str).collect::<Result<_, _>>()?;
        rows.sort_by_key(|r| (r.arm, r.seed));
        rows
    };
    factorial::print_report(&rows);
    println!("\nruns {} in {:.1} s on {workers} workers", report.runs, report.wall_seconds);
    println!("simulated ticks {}", report.simulated_ticks);
    println!(
        "worst |material residual| {:.3e}   worst |energy residual| {:.3e}",
        report.worst_material_residual, report.worst_energy_residual
    );
    println!("clone births {}   dropped ledger records {}", report.clone_births, report.dropped_records);
    println!("rows    {}", out.join("runs.jsonl").display());
    println!("summary {}", out.join("summary.json").display());
    Ok(())
}

fn print_params() -> Result<(), Box<dyn std::error::Error>> {
    println!("build {BUILD_ID}");
    println!("\n{} searched parameters\n", params::PARAMS.len());
    println!("{:<34} {:>10} {:>10} {:>10}  unit / why", "field", "default", "lo", "hi");
    for p in params::PARAMS {
        println!("{:<34} {:>10} {:>10} {:>10}  {}", p.name, p.default, p.lo, p.hi, p.unit);
        println!("{:<34} {:>10} {:>10} {:>10}    {}", "", "", "", "", p.why);
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
