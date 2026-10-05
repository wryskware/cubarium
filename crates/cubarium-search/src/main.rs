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
use cubarium_search::census;
use cubarium_search::es;
use cubarium_search::evaluate::{BUILD_ID, Protocol, Status, evaluate};
use cubarium_search::factorial;
use cubarium_search::metrics::Scoring;
use cubarium_search::params;
use cubarium_search::population;
use cubarium_search::precondition;
use cubarium_search::search::{self, Budget, TRAINING_SEEDS, Variation};

#[derive(Parser, Debug)]
#[command(
    name = "cubarium-search",
    about = "Headless whole-ecosystem parameter search"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// The ecology (a genome, a parameter file or an old `[ecology]` file, as
    /// `voxel-ecology-check --arm` reads) the frozen trainer's arenas and landscapes are
    /// built on; the live defaults when absent (audit round step 5).
    #[arg(long, global = true)]
    training_ecology: Option<PathBuf>,
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
        /// The motor contract every fixture world runs under: `sweep` is the shipped
        /// outer-point model, `inertial` is workstream T's paired disc model. It moves the
        /// protocol hash (a `sweep` protocol keeps the hash it has always had) and is written
        /// into every exported policy, which is refused by name under the other contract.
        #[arg(long, default_value = "sweep")]
        motor: String,
        /// The action adapter every fixture world decodes a raw head with: `cub-act-1` is the
        /// shipped adapter (deadband 0.05 on thrust and turn), `cub-act-2` is workstream X's
        /// released turn band (0.05 on thrust, 0.0 on turn). It moves the protocol hash and the
        /// policy digest (a `cub-act-1` protocol keeps the hash it has always had) and is
        /// written into every exported policy, which is refused by name under the other
        /// adapter.
        #[arg(long, default_value = "cub-act-1")]
        adapter: String,
        #[arg(long, default_value = "runs/es-first")]
        out: PathBuf,
    },
    /// Workstream X: replay one generation's centre and candidates on their own training
    /// layouts under **both** action adapters, weights untouched, and measure the retained pair
    /// contributions' gradient-direction stability.
    EsTurnBand {
        /// The retained training run: its `checkpoint.json`, `centers/` and nothing else.
        #[arg(long, default_value = "runs/es-eco-v1-fastleaf")]
        run: PathBuf,
        /// The ecology the run was trained in. Required: replaying in another world would
        /// compare two tasks, and the policy files are checked against this by name.
        #[arg(long)]
        config: PathBuf,
        /// Which recorded centre to replay, with its own `2n` candidates.
        #[arg(long, default_value_t = 9)]
        generation: u64,
        #[arg(long, default_value_t = cubarium_search::es::HORIZON_TICKS)]
        horizon: u64,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value_t = 600)]
        wall_seconds: u64,
        /// Workstream Q's retained pair reduction, for the stability half. It exists for one
        /// run and one generation; omitted, the replay runs alone, which is what replaying
        /// another run's centre wants.
        #[arg(long)]
        pairs: Option<PathBuf>,
        #[arg(long, default_value_t = 1_000)]
        bootstrap: usize,
        #[arg(long, default_value = "runs/ecology-v1-turn-deadband")]
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
        /// The motor contract every fixture world runs under: `sweep` is the shipped
        /// outer-point model, `inertial` is workstream T's paired disc model. It moves the
        /// protocol hash (a `sweep` protocol keeps the hash it has always had) and is written
        /// into every exported policy, which is refused by name under the other contract.
        #[arg(long, default_value = "sweep")]
        motor: String,
        // No `--pursuit-stop`: an evaluation layout founds no hunter, so the rule is
        // unreachable in an episode (`es::fixture::Layout`).
        /// The action adapter every fixture world decodes a raw head with: `cub-act-1` is the
        /// shipped adapter (deadband 0.05 on thrust and turn), `cub-act-2` is workstream X's
        /// released turn band (0.05 on thrust, 0.0 on turn). It moves the protocol hash and the
        /// policy digest (a `cub-act-1` protocol keeps the hash it has always had) and is
        /// written into every exported policy, which is refused by name under the other
        /// adapter.
        #[arg(long, default_value = "cub-act-1")]
        adapter: String,
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
        /// The pursuit stopping rule the run's world uses: `reach-envelope` is the shipped
        /// rule — `ContactMeasure::in_contact()`, the reach envelope the predicate's own
        /// comment has always named, adopted on 2026-09-16 — and `half-space` is the rule
        /// before it, `body.x < capture_offset_body.x + tolerance`, which held a member at its
        /// resting effort and suppressed the strike burst it had already paid for. A transient
        /// on the world: it does not enter the config hash, and naming `half-space` is how a
        /// row retained before the adoption is reproduced bit for bit. This changes what the
        /// world does, deliberately.
        #[arg(long, default_value = "reach-envelope")]
        pursuit_stop: String,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Hard wall cap. Trials not started by then are recorded as skipped, never extended.
        #[arg(long, default_value_t = 600)]
        wall_seconds: u64,
        /// The action adapter the whole world runs. Only `cub-act-1` — the shipped adapter and
        /// the display host's — is accepted here: this command's stage runner
        /// (`crate::population`) is owned by another workstream this round and was not opened
        /// to the switch, so naming `cub-act-2` is refused rather than silently ignored. A
        /// `cub-act-2` policy file is in any case refused by name by `PolicyFile::policy`.
        #[arg(long, default_value = "cub-act-1")]
        adapter: String,
        #[arg(long, default_value = "runs/es-eco-v1-fastleaf/population")]
        out: PathBuf,
    },
    /// The matched feasibility experiment: four drivers on the same twelve layouts, measured
    /// with the world's own per-organism store ledger.
    EsBudget {
        /// The trained policy to compare. Its recorded config hash must be `--config`'s.
        #[arg(
            long,
            default_value = "runs/es-eco-v1-fastleaf/selected/center-00009-policy.json"
        )]
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
        /// Place the founders as if they had already lived this many seconds. `0` is the door
        /// the screen used. An age larger than `--introduce-tick` worth of world is refused:
        /// a body cannot have been born before the world began.
        #[arg(long, default_value_t = 0.0)]
        founder_age_seconds: f64,
        /// Turn the per-body store ledger **and the per-attempt strike record** off (the two
        /// diagnostics share this switch in the audit). Both are on by default for this command;
        /// neither changes dynamics, and a run with them off records no death cause and no
        /// strike geometry.
        #[arg(long, default_value_t = false)]
        no_ledger: bool,
        /// The pursuit's stopping predicate. `reach-envelope` is the **shipped rule** since
        /// 2026-09-16 — the `in_contact()` the predicate's own comment names — and is the
        /// default, so a bare audit runs the world exactly as it ships. `half-space` is the
        /// rule before that date, `body.x < capture_offset_body.x + tolerance`, which held a
        /// member at its resting effort and suppressed the burst it had paid for; name it to
        /// reproduce a row retained under it. This changes what the world does, deliberately.
        #[arg(long, default_value = "reach-envelope")]
        pursuit_stop: String,
        /// The motor contract. `sweep` is the shipped outer-point model, where an apex member's
        /// turn radius is its 14.8 px grasp; `inertial` is workstream T's paired disc model,
        /// where rotation is `r·ω/√2` over the member's own lobes, the envelope is
        /// `√(v² + v_rot²) ≤ cap`, and the grasp is contact geometry only. The default runs the
        /// world exactly as it ships. This changes what the world does, deliberately.
        #[arg(long, default_value = "sweep")]
        motor: String,
        /// Which radius an apex member's 14.8 px grasp puts in the turn budget under `--motor
        /// sweep`. `grasp` is the shipped rule, `max(lobes, |capture_offset| + capture_reach)`;
        /// `lobes` is workstream U's paired variant, the member's own 9 px extent, with the
        /// grasp left to the strike that reaches with it. No ordinary body is affected either
        /// way. The default runs the world exactly as it ships.
        #[arg(long, default_value = "grasp")]
        apex_turn_radius: String,
        /// The motor contract the **apex members alone** run, leaving every ordinary body on
        /// `--motor`. Omitted — the default — every body runs `--motor`, which is what the world
        /// ships. Naming one makes the prey world at introduction identical between the two
        /// halves of a motor pair, which `--motor` alone cannot do: workstream T's inertial arm
        /// held 628 prey at introduction against the shipped arm's 745. Same two names as
        /// `--motor`; no ordinary body is affected either way.
        #[arg(long)]
        apex_motor: Option<String>,
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
        /// Record workstream E's per-body budget ledger, so the net energy margin per body is
        /// measured rather than inferred. Off by default: it is throughput the screen and the
        /// trainer do not need.
        #[arg(long, default_value_t = false)]
        ledger: bool,
        /// Record workstream M's per-cell plant budget: every §4 plant flow and every §6.4
        /// consumer withdrawal, booked in the cell it happened in. Off by default.
        #[arg(long, default_value_t = false)]
        plant_record: bool,
        /// Found no animals at all — `founders.kinds` empty and `founders.count` zero — so the
        /// world is plants, water and weather. Workstream M's herbivore-absent arm. This
        /// changes the world, and its hash, deliberately.
        #[arg(long, default_value_t = false)]
        no_animals: bool,
        /// The motor contract the run's world uses: `sweep` is the shipped outer-point model
        /// and reproduces every retained row bit for bit; `inertial` is workstream T's paired
        /// disc model, where rotation is an energy-equivalent speed `r·ω/√2`, the envelope is
        /// `√(v² + v_rot²) ≤ cap`, and an apex's grasp is no longer a turn radius. A transient
        /// on the world: it does not enter the config hash. This changes what the world does,
        /// deliberately.
        #[arg(long, default_value = "sweep")]
        motor: String,
        /// The pursuit stopping rule the run's world uses: `reach-envelope` is the shipped
        /// rule — `ContactMeasure::in_contact()`, the reach envelope the predicate's own
        /// comment has always named, adopted on 2026-09-16 — and `half-space` is the rule
        /// before it, `body.x < capture_offset_body.x + tolerance`, which held a member at its
        /// resting effort and suppressed the strike burst it had already paid for. A transient
        /// on the world: it does not enter the config hash, and naming `half-space` is how a
        /// row retained before the adoption is reproduced bit for bit. This changes what the
        /// world does, deliberately.
        #[arg(long, default_value = "reach-envelope")]
        pursuit_stop: String,
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
    /// Workstream S: the fixed-age preconditioned opening. `--stage field` runs each
    /// (candidate, seed) plant-only to the last declared age, saves the whole field at every
    /// age and measures how settled it is; `--stage compare` founds the ordinary roster at
    /// each age and runs the ordinary horizon after it, with age 0 as the status quo.
    ///
    /// Workstream Z adds `--stage grazed`: the burn-in is the **ordinary coupled world**, so
    /// the field settles against grazing rather than against nobody; that population is then
    /// removed and the identical fresh roster founded into the field it grazed.
    Precondition {
        /// `field` (the plant-only operator and its settling measures), `compare` (S's arms)
        /// or `grazed` (Z's coupled burn-in, removal and re-founding).
        #[arg(long, default_value = "field")]
        stage: String,
        /// Comma-separated candidate names, or `all`.
        #[arg(long, default_value = "baseline,fast-leaf")]
        candidates: String,
        /// `training` or `holdout`. The held-out set is for the final validation only.
        #[arg(long, default_value = "training")]
        seed_set: String,
        /// How many seeds of that set, from the front.
        #[arg(long, default_value_t = 6)]
        seeds: usize,
        /// Comma-separated plant-only ages, in ticks. `0` is the status quo.
        #[arg(long, default_value = "0,48000,96000,180000")]
        ages: String,
        /// The pursuit stopping rule the run's world uses: `reach-envelope` is the shipped
        /// rule — `ContactMeasure::in_contact()`, the reach envelope the predicate's own
        /// comment has always named, adopted on 2026-09-16 — and `half-space` is the rule
        /// before it, `body.x < capture_offset_body.x + tolerance`, which held a member at its
        /// resting effort and suppressed the strike burst it had already paid for. A transient
        /// on the world: it does not enter the config hash, and naming `half-space` is how a
        /// row retained before the adoption is reproduced bit for bit. This changes what the
        /// world does, deliberately.
        #[arg(long, default_value = "reach-envelope")]
        pursuit_stop: String,

        /// The horizon each comparison arm runs after founding.
        #[arg(long, default_value_t = 180_000)]
        ticks: u64,
        #[arg(long, default_value_t = 600)]
        sample_every: u64,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Hard wall cap. Trials not started by then are recorded as skipped, never extended.
        #[arg(long, default_value_t = 1_800)]
        wall_seconds: u64,
        #[arg(long, default_value = "runs/ecology-v1-precondition")]
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
        /// founder pairing), `As` (A counterbalanced), `D1`–`D4` (the four rows of the
        /// depth x diet Latin square; run all four together or the design is not one).
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
        /// The pursuit stopping rule the run's world uses: `reach-envelope` is the shipped
        /// rule — `ContactMeasure::in_contact()`, the reach envelope the predicate's own
        /// comment has always named, adopted on 2026-09-16 — and `half-space` is the rule
        /// before it, `body.x < capture_offset_body.x + tolerance`, which held a member at its
        /// resting effort and suppressed the strike burst it had already paid for. A transient
        /// on the world: it does not enter the config hash, and naming `half-space` is how a
        /// row retained before the adoption is reproduced bit for bit. This changes what the
        /// world does, deliberately.
        #[arg(long, default_value = "reach-envelope")]
        pursuit_stop: String,

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
    /// Run the depth census: F's 150-minute variety census with the roster skimmer's `depth`
    /// overridden search-side at tick 0 and nothing else changed. Workstream Y widened it to
    /// a six-rung ladder at apex arm 0; R's two levels are still rungs of it.
    Census(census::Args),
    /// Re-run one recorded row and check it reproduces.
    Replay {
        /// The `evals.jsonl` written by a search.
        #[arg(long)]
        record: PathBuf,
        /// Zero-based row index in that file.
        #[arg(long, default_value_t = 0)]
        index: usize,
    },
    /// Phase-one voxel slice (P1-D): arena validity for both founders plus a GRU and
    /// heuristic controller smoke through the real episode driver.
    VoxelCheck {
        /// One founder (`blind`/`browser`) or both by default.
        #[arg(long)]
        founder: Option<String>,
        /// How many of the frozen training layout seeds to check, from the front.
        #[arg(long, default_value_t = 4)]
        seeds: usize,
        /// `a` (acquire) or `b` (deplete the first patch, then reacquire).
        #[arg(long, default_value = "a")]
        stage: String,
        /// Ticks per smoke episode.
        #[arg(long, default_value_t = 240)]
        ticks: u64,
    },
    /// Phase-one voxel slice (P1-D): episode throughput — setup cost, episodes/second at
    /// one worker, and the same at up to the current offline-training worker limit.
    VoxelBench {
        /// `blind` (littershredder) or `browser` (frondgrazer).
        #[arg(long, default_value = "blind")]
        founder: String,
        /// `a` or `b`.
        #[arg(long, default_value = "a")]
        stage: String,
        #[arg(long, default_value_t = 1_200)]
        ticks: u64,
        /// Episodes per batch.
        #[arg(long, default_value_t = 8)]
        episodes: usize,
        /// Episode workers for the parallel batch. The runtime limit reserves ten percent
        /// of logical CPUs and stops at the measured sixteen-worker saturation point.
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::episode_worker_limit())]
        workers: usize,
    },
    /// P5-B: found the training (or held-out) landscape sets, report what they kept and
    /// what preparing them cost, and measure arena and landscape episode throughput.
    VoxelLandscapes {
        /// `blind` (littershredder) or `browser` (frondgrazer).
        #[arg(long, default_value = "blind")]
        founder: String,
        /// Comma-separated presets.
        #[arg(long, default_value = "small,default,wide")]
        presets: String,
        /// The held-out seed bases instead of the training ones.
        #[arg(long)]
        held_out: bool,
        /// Ticks per measured episode.
        #[arg(long, default_value_t = 600)]
        ticks: u64,
        /// Episodes per pool at one worker (and per worker in the parallel batch).
        #[arg(long, default_value_t = 4)]
        episodes: usize,
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::episode_worker_limit())]
        workers: usize,
        /// Worker pinning: `auto` (physical cores first, the largest L3 first, within the
        /// affinity mask), `off`, or a CPU list in order (`0-7,16-23`); worker i takes one.
        #[arg(long, default_value = cubarium_search::es::voxel::pin::DEFAULT_PIN)]
        pin: String,
        /// The cache study's arms instead of the per-pool table: `workers:pin` separated
        /// by `;` (`16:off;16:0-7,16-23;8:0-7`), each run `--repeats` times, interleaved,
        /// over P5-C's training pool. `--ticks 0` runs the lineage's horizon.
        #[arg(long)]
        arms: Option<String>,
        #[arg(long, default_value_t = 3)]
        repeats: usize,
        /// With `--arms`: the CPUs the founding runs on (a CPU list); the arms still place
        /// their workers within the process's own mask.
        #[arg(long)]
        setup_pin: Option<String>,
    },
    /// Phase-three voxel slice (P3-C): record the fauna's own foraging heuristic on the
    /// training layouts as `(observation, adapted action)` streams — the teacher an
    /// imitation seed is fitted to. The held-out seeds are never recorded.
    VoxelImitate {
        /// `blind` (littershredder) or `browser` (frondgrazer).
        #[arg(long, default_value = "blind")]
        founder: String,
        /// Comma-separated stages to record, in order: `a`, `b`, or `a,b`.
        #[arg(long, default_value = "a,b")]
        stages: String,
        /// Stage B's successor separation band for the recorded layouts.
        #[arg(long, default_value = "landed")]
        band: String,
        /// How many of the frozen training layouts to record, from the front.
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::TRAINING_LAYOUT_SEEDS.len())]
        layouts: usize,
        /// Episode horizon in ticks; defaults to each stage's own.
        #[arg(long)]
        horizon: Option<u64>,
        /// Episode workers, one simulation thread per episode; also the fit's threads.
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::episode_worker_limit())]
        workers: usize,
        /// Fit a clone to the recorded streams and save it as a centre. `--fit false`
        /// records only.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        fit: bool,
        /// Adam steps over the full stream set, teacher-forced.
        #[arg(long, default_value_t = 4_000)]
        updates: u32,
        #[arg(long, default_value_t = 0.01)]
        learning_rate: f64,
        /// Truncated-BPTT window in controller steps. The hidden state carries across a
        /// window boundary; the gradient does not.
        #[arg(long, default_value_t = 120)]
        chunk: usize,
        /// Global gradient-norm clip.
        #[arg(long, default_value_t = 5.0)]
        clip: f64,
        /// Seed for the initial centre the fit starts from.
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::TRAINING_SEED)]
        fit_seed: u64,
        /// Directory for `streams/` and `clone-center.json`. Disposable run output.
        #[arg(long, default_value = "runs/voxel-imitate-blind")]
        out: PathBuf,
        /// P5-C (C4): record the arenas on both grids and every training landscape
        /// (one stream per acting body), then report the clone against its teacher and
        /// standing still on the held-out landscapes.
        #[arg(long, default_value_t = false)]
        p5: bool,
        /// Write the recorded streams under `--out/streams`.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        write_streams: bool,
    },
    /// The ecology half (`design/handoffs/voxel-ecology-search-2026-09-24.md`): CMA-ES
    /// over the ecology parameter vector, each candidate scored on freshly founded
    /// training worlds against the population and liveliness band, brains held fixed.
    VoxelEcologySearch {
        /// `<lineage>=<centre.json>` (or `transfer:<file>`), per trained lineage. A lineage
        /// given none runs its heuristic and takes no ES step.
        #[arg(long = "centre")]
        centres: Vec<String>,
        /// The search file (genes, ranges, `[screen]`): a `config/training/search/` file, or
        /// a genome to start the mean from.
        #[arg(long, default_value = "config/training/search/cotrain.toml")]
        search: PathBuf,
        /// A genome whose genes' values start the CMA mean (by name, the rest at the
        /// search file's): e.g. config/training/search/cotrain-start.toml.
        #[arg(long)]
        start: Option<PathBuf>,
        /// Found from this config file's `[world]` (e.g. config/desktop/terrarium.toml)
        /// instead of the still-water `small` preset.
        #[arg(long)]
        world: Option<PathBuf>,
        /// Founders per lineage, `shredders,browsers,lanternjaw-clutches`; the live
        /// world's founding (the egg bank included) by default.
        #[arg(long)]
        founders: Option<String>,
        #[arg(long, default_value_t = 7)]
        lambda: usize,
        #[arg(long, default_value_t = 2)]
        seeds_per_generation: usize,
        #[arg(long, default_value_t = 100)]
        generations: u32,
        #[arg(long, default_value_t = 10_800.0)]
        wall_seconds: f64,
        /// Initial step size, natural-log units.
        #[arg(long, default_value_t = 0.5)]
        sigma0: f64,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        /// Cores the last worlds share once none is left queued (each about cores / running,
        /// re-read every simulated minute); `0` is every core the OS reports, `1` keeps
        /// every world on one thread throughout.
        #[arg(long, default_value_t = 0)]
        tail_cores: usize,
        #[arg(long, default_value_t = 50_001)]
        seed_base: u64,
        #[arg(long, default_value_t = 20260924)]
        cma_seed: u64,
        /// Simulated hours per candidate world.
        #[arg(long, default_value_t = 1.0)]
        hours: f64,
        #[arg(long, default_value_t = es::voxel::live::window::DEFAULT_WINDOW_MIN)]
        window_min: f64,
        #[arg(long, default_value_t = cubarium_voxel_sim::EXPERIMENT_WATER_EVERY)]
        water_every: u32,
        #[arg(long, default_value_t = 240)]
        abort_animals: usize,
        /// Simulated hours the band is scored on; past `--hours` (or an early stop) the
        /// count is projected on its last hour's trend. Defaults to `--hours`.
        #[arg(long)]
        score_hours: Option<f64>,
        /// The animal band's floor, × the world's founding animal count.
        #[arg(long, default_value_t = 1.0)]
        band_lo: f64,
        /// Its ceiling over the second half, × the founding count.
        #[arg(long, default_value_t = 3.0)]
        band_hi: f64,
        /// The count it never passes, × the founding count.
        #[arg(long, default_value_t = 5.0)]
        band_peak: f64,
        /// The browsers' moving share floor.
        #[arg(long, default_value_t = 0.30)]
        moving_min: f64,
        /// The lanternjaws' share of the animals over the second half: `lo,hi`.
        #[arg(long, default_value = "0.05,0.15")]
        lanternjaw_share: String,
        /// The penalty per lineage or species lost, and per lineage not breeding late.
        #[arg(long, default_value_t = 10.0)]
        missing_cost: f64,
        /// Unified mode: brain antithetic pairs per lineage with a centre. 0 holds the brains.
        #[arg(long, default_value_t = 0)]
        brain_pairs: usize,
        #[arg(long, default_value_t = cubarium_search::es::optimizer::SIGMA)]
        brain_sigma: f64,
        #[arg(long, default_value_t = cubarium_search::es::optimizer::LEARNING_RATE)]
        brain_learning_rate: f64,
        #[arg(long, default_value = "runs/ecology-search")]
        out: PathBuf,
        /// Override the search file's `[targets] persist_only`: `none`, or lineages
        /// comma-separated (stage 2 runs `none` on 2 h horizons).
        #[arg(long)]
        persist_only: Option<String>,
        /// Continue the search checkpointed in this directory (its `checkpoint.json`: the
        /// CMA state, the centres and their Adam state, the brain step, the best so far).
        #[arg(long)]
        resume: Option<PathBuf>,
        /// Per-lineage brain curiosity (`config/training/curiosity.toml`): the novelty term
        /// `weight × novelty` in the brains' window score. Without it every weight is 0.
        #[arg(long)]
        curiosity: Option<PathBuf>,
        /// Where every candidate starts: `e0` (today's founding, the default) or `e1`
        /// (needs `--starts`). (`--start` is the CMA's starting genome here.)
        #[arg(long)]
        start_set: Option<String>,
        /// A start library (`voxel-starts --out DIR`): each generation's worlds are its
        /// seeds in turn, every candidate on the same ones, rate-only genes applied and the
        /// rest frozen at the start's values.
        #[arg(long)]
        starts: Option<PathBuf>,
        /// The burn-in's minimum, simulated minutes (0 on E0, 20 on a library by default).
        #[arg(long)]
        burn_in_min: Option<f64>,
    },
    /// The R/S/F pre-screen of an ecology on a world: the three stay-or-go numbers and
    /// each founded species' row.
    VoxelEcologyScreen {
        /// `default`, a genome or search file, a parameter file or an old `[ecology]` file.
        #[arg(long, default_value = "default")]
        arm: String,
        #[arg(long)]
        world: Option<PathBuf>,
        /// The `[screen]` settings: this search file's (the defaults otherwise).
        #[arg(long)]
        search: Option<PathBuf>,
    },
    /// The ecology search's held-out check: arms (`name=default` or `name=<file.toml>`)
    /// × seeds, one CSV per world with one row per window.
    VoxelEcologyCheck {
        /// `<lineage>=<brain>` per lineage (heuristic, parker, herding, a centre file); a
        /// lineage given none runs its heuristic.
        #[arg(long = "centre")]
        centres: Vec<String>,
        /// Founders per lineage, `shredders,browsers,lanternjaw-clutches`; the live
        /// world's founding (the egg bank included) by default.
        #[arg(long)]
        founders: Option<String>,
        #[arg(long = "arm", required = true)]
        arms: Vec<String>,
        #[arg(long, default_value = "1,2,3,4")]
        seeds: String,
        #[arg(long, default_value_t = 6.0)]
        hours: f64,
        #[arg(long, default_value_t = es::voxel::live::window::DEFAULT_WINDOW_MIN)]
        window_min: f64,
        /// Coarse water (training, search and experiment runs; the display runs 1).
        #[arg(long, default_value_t = cubarium_voxel_sim::EXPERIMENT_WATER_EVERY)]
        water_every: u32,
        #[arg(long, default_value_t = 8)]
        threads: usize,
        /// Cores the last worlds share once none is left queued (each about cores / running,
        /// re-read every simulated minute); `0` is every core the OS reports, `1` keeps
        /// every world on one thread throughout.
        #[arg(long, default_value_t = 0)]
        tail_cores: usize,
        #[arg(long, default_value = "runs/ecology-check")]
        out: PathBuf,
        /// Found from this config file's `[world]` (a host's, e.g.
        /// config/desktop/terrarium.toml) instead of the still-water `small` preset.
        #[arg(long)]
        world: Option<PathBuf>,
        /// The cells and memory the CSV's `mean_novelty` column is measured on
        /// (`config/training/curiosity.toml`); the defaults without. A measurement only.
        #[arg(long)]
        curiosity: Option<PathBuf>,
        /// **Finalist validation** (proposal Part 3; Astra A10): run each arm's worlds past
        /// the search horizon (`--finalist-from`, hours) by the longest of the next night
        /// (through the following dawn), pup independence and one litter interval (about 8 h
        /// on the terrarium), scored by the search's own evaluation with the lanternjaw
        /// held to an established hunter. Writes one JSON per world and a per-lineage table;
        /// `--hours` is ignored.
        #[arg(long, default_value_t = false)]
        finalist: bool,
        #[arg(long, default_value_t = 2.0)]
        finalist_from: f64,
        /// Score each world `--hours` long by the search's own evaluation (the arm's
        /// `[targets]`: per-lineage outcomes, penalty) instead of writing window CSVs.
        #[arg(long, default_value_t = false)]
        targets: bool,
    },
    /// **The big-lever grid** (`design/handoffs/lever-grid-tool-2026-10-03.md`): master
    /// scales (animal metabolism `m` x plant productivity `p`, plus any `--axis`) x distinct
    /// worlds, resumable; writes `cells.jsonl`, `summary.tsv` and `summary.md` to `--out`.
    VoxelEcologyGrid {
        /// Multipliers of every lineage's daily need (motor follows); `none` drops the axis.
        #[arg(long, default_value = es::voxel::live::grid::DEFAULT_LEVELS)]
        m: String,
        /// Multipliers of plant production; `none` drops the axis.
        #[arg(long, default_value = es::voxel::live::grid::DEFAULT_LEVELS)]
        p: String,
        /// A further axis, `NAME:LEVEL|LEVEL|…`; a level is `PATH=V` (set) or `PATH=xV`
        /// (multiply) overrides joined by `;`, or a number for a named scale.
        #[arg(long = "axis")]
        axes: Vec<String>,
        /// Distinct worlds the founding accepts as themselves, tried from `--first-seed`.
        #[arg(long, default_value_t = 8)]
        seeds: usize,
        #[arg(long, default_value_t = 1)]
        first_seed: u64,
        #[arg(long, default_value_t = 2.0)]
        hours: f64,
        /// A host config file (its `[world]`, and its `[flora]`/`[fauna]` as every cell's
        /// x1) or a preset's name (the code-default ecology).
        #[arg(long, default_value = "config/desktop/terrarium.toml")]
        world: String,
        /// Worlds run at once, one simulation thread each while jobs are queued.
        #[arg(long, default_value_t = 8)]
        threads: usize,
        /// Cores the last worlds share once the queue is empty (each about cores / running,
        /// re-read every simulated minute); `0` is every core the OS reports, `1` keeps
        /// every world on one thread throughout.
        #[arg(long, default_value_t = 0)]
        tail_cores: usize,
        /// Coarse water (training, search and experiment runs; the display runs 1).
        #[arg(long, default_value_t = cubarium_voxel_sim::EXPERIMENT_WATER_EVERY)]
        water_every: u32,
        /// Stop a world holding more animals than this.
        #[arg(long, default_value_t = 600)]
        abort_animals: usize,
        /// `<lineage>=<brain>`; a lineage given none runs its heuristic.
        #[arg(long = "centre")]
        centres: Vec<String>,
        #[arg(long, default_value = "runs/ecology-grid")]
        out: PathBuf,
        /// Where every cell starts: `e0` (today's founding, the default) or `e1` (the
        /// constructed rich start; needs `--starts`).
        #[arg(long)]
        start: Option<String>,
        /// A start library (`voxel-starts --out DIR`): every cell on its seeds, loaded
        /// through the episode initializer.
        #[arg(long)]
        starts: Option<PathBuf>,
        /// The burn-in's minimum, simulated minutes, before scoring starts on the day
        /// clock's window grid: 0 on E0, 20 on a library by default.
        #[arg(long)]
        burn_in_min: Option<f64>,
        /// These worlds exactly (`1,3`), each founded as asked, instead of `--seeds` picked
        /// from `--first-seed`.
        #[arg(long)]
        seed_list: Option<String>,
    },
    /// **Start libraries** (`design/handoffs/assembly-step1-2026-10-03.md`): one start
    /// directory per seed, in the whole-world save layout plus `start.toml` and
    /// `recipe.toml`, for `--starts`. Resumable: a seed already written is skipped.
    VoxelStarts {
        /// `e0` (today's founding) or `e1` (constructed rich).
        #[arg(long)]
        recipe: String,
        #[arg(long, default_value = "1")]
        seeds: String,
        /// As the grid's `--world`: a host config file (its `[world]`, its `[flora]`/`[fauna]`
        /// the start's own parameters) or a preset's name.
        #[arg(long, default_value = "config/desktop/terrarium.toml")]
        world: String,
        /// `<lineage>=<brain>`: the policy the starts are made for (recorded and checked).
        #[arg(long = "centre")]
        centres: Vec<String>,
        /// The E1 recipe's numbers, a TOML file (`live::assembly::E1Recipe`); the defaults
        /// without.
        #[arg(long)]
        recipe_file: Option<PathBuf>,
        /// E1: hours of plants-only growth (over the recipe file's).
        #[arg(long)]
        plant_hours: Option<f64>,
        /// Seeds made at once.
        #[arg(long, default_value_t = 2)]
        threads: usize,
        /// Simulation threads per seed during E1's grow.
        #[arg(long, default_value_t = 1)]
        sim_threads: usize,
        #[arg(long)]
        out: PathBuf,
    },
    /// Live-world training (`design/handoffs/voxel-live-world-training-2026-09-24.md`):
    /// every body of every lineage given a centre runs its own antithetic variant inside
    /// persistent live worlds (the `small` preset, still water), one ES update per lineage
    /// per window, with the ecology readout every generation.
    VoxelLiveTrain {
        /// `<lineage>=<centre.json>`, once per trained lineage (`browser=…`,
        /// `littershredder=…`).
        #[arg(long = "centre", required = true)]
        centres: Vec<String>,
        #[arg(long, default_value_t = 16)]
        worlds: usize,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        #[arg(long, default_value_t = 16)]
        pairs: usize,
        /// Window length, simulated minutes.
        #[arg(long, default_value_t = es::voxel::live::window::DEFAULT_WINDOW_MIN)]
        window_min: f64,
        #[arg(long, default_value_t = 1000)]
        generations: u32,
        #[arg(long, default_value_t = 7000.0)]
        wall_seconds: f64,
        #[arg(long, default_value_t = cubarium_search::es::optimizer::SIGMA)]
        sigma: f64,
        #[arg(long, default_value_t = cubarium_search::es::optimizer::LEARNING_RATE)]
        learning_rate: f64,
        #[arg(long, default_value_t = 20260924)]
        train_seed: u64,
        #[arg(long, default_value_t = 3001)]
        world_seed: u64,
        /// Training-only coarse water: the lateral and groundwater phases on one tick in
        /// this many. `1` is the live water.
        #[arg(long, default_value_t = cubarium_voxel_sim::EXPERIMENT_WATER_EVERY)]
        water_every: u32,
        #[arg(long, default_value_t = 240)]
        population_cap: usize,
        #[arg(long, default_value_t = 10)]
        save_every: u32,
        /// A training-only ecology: a genome, a parameter file or an old `[ecology]` file;
        /// the shipped world without.
        #[arg(long)]
        ecology: Option<PathBuf>,
        #[arg(long, default_value = "runs/live-train")]
        out: PathBuf,
        /// Found the training worlds from this config file's `[world]` (a host's, e.g.
        /// config/desktop/terrarium.toml) instead of the still-water `small` preset.
        #[arg(long)]
        world: Option<PathBuf>,
        /// Per-lineage brain curiosity (`config/training/curiosity.toml`): the novelty term
        /// `weight × novelty` in the brains' window score. Without it every weight is 0.
        #[arg(long)]
        curiosity: Option<PathBuf>,
    },
    /// Phase-one voxel slice (P1-D): the bounded ES run over one founder's manifest, with
    /// shape-aware antithetic pairs, the initial centre evaluation, and the plan's caps.
    VoxelTrain {
        /// `blind` (littershredder) or `browser` (frondgrazer).
        #[arg(long, default_value = "blind")]
        founder: String,
        /// `a` (acquire) or `b` (deplete the first patch, then reacquire). Stage B
        /// defaults to its own longer horizon.
        #[arg(long, default_value = "a")]
        stage: String,
        /// Stage B's successor separation band: `landed` (the task, 2 m and out) or
        /// `near` (the curriculum rung, 1.0-1.5 m, inside both founders' senses). Part
        /// of the arena protocol string and so of the run's protocol hash.
        #[arg(long, default_value = "landed")]
        band: String,
        /// Warm start: a saved centre (`centers/*.json`) whose exact weights this run
        /// begins from, with a fresh Adam state and its own protocol hash. The source
        /// file, its weight hash and its protocol are recorded in the checkpoint. A
        /// centre of the other founder is refused.
        #[arg(long)]
        init_center: Option<PathBuf>,
        /// The training controller: `gru` is the only trainable body (the heuristic slot
        /// carries no parameters and is refused here by name).
        #[arg(long, default_value = "gru")]
        controller: String,
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::DEFAULT_PAIRS)]
        pairs: usize,
        /// How many of the frozen training layouts to run, from the front.
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::TRAINING_LAYOUT_SEEDS.len())]
        layouts: usize,
        /// Updates, capped at 512.
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::MAX_UPDATES)]
        updates: u32,
        /// Episode horizon in ticks. Defaults to the stage's own (1,200 for A,
        /// 2,400 for B).
        #[arg(long)]
        horizon: Option<u64>,
        /// Episode workers, one simulation thread per episode. The runtime limit reserves
        /// ten percent of logical CPUs and stops at the measured sixteen-worker saturation point.
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::episode_worker_limit())]
        workers: usize,
        /// Wall cap in seconds (the plan allots one archetype up to eight minutes).
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::DEFAULT_TRAIN_WALL_SECONDS)]
        wall_seconds: u64,
        /// The run's episode limit, counting every attempted episode including
        /// discarded and cancelled work. The plan's per-archetype count is 2,180.
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::DEFAULT_EPISODE_LIMIT)]
        episode_limit: u64,
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::TRAINING_SEED)]
        train_seed: u64,
        /// Evaluate the unperturbed centre each generation. A sampled perturbation's
        /// score is not the centre's.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        center_eval: bool,
        /// Discard an existing run in `--out` and start fresh.
        #[arg(long, default_value_t = false)]
        overwrite: bool,
        /// Directory for `checkpoint.json` and `centers/`.
        #[arg(long, default_value = "runs/voxel-es-blind")]
        out: PathBuf,
        /// P5-C (C1, C2): the arena half on both grids plus a fresh draw of training
        /// landscapes every generation, and held-out checkpoints.
        #[arg(long, default_value_t = false)]
        p5: bool,
        /// Training landscapes drawn per generation (with `--p5`).
        #[arg(long, default_value_t = 16)]
        landscapes_per_generation: usize,
        /// Updates between held-out checkpoints (with `--p5`); a fine-tune can check
        /// every 16.
        #[arg(long, default_value_t = 32, alias = "heldout-every")]
        held_out_every: u32,
        /// Transfer start: a saved centre whose vector is this manifest's length,
        /// taken even though its manifest digest is another build's (package L moved
        /// the anchors, not the shape). The source file and digest go into the run's
        /// provenance. A training start only: `--init-center` stays strict.
        #[arg(long, conflicts_with = "init_center")]
        transfer_from: Option<PathBuf>,
        /// `N,g`: stop when the held-out best has not risen by a relative `g` over the
        /// last `N` checkpoints, or `off`. Default for `--p5` runs: 3,0.02. The C5
        /// fall rule applies beside it.
        #[arg(long)]
        plateau: Option<String>,
        /// A remote worker, `host:threads` (repeatable): runs `ssh host <remote-bin>
        /// voxel-eval-worker --threads N` and sends it pairs over the pipe. The remote
        /// binary must be this build (`scripts/voxel-remote-ship.sh`).
        #[arg(long = "remote")]
        remotes: Vec<String>,
        /// The worker binary's path on the remote machines.
        #[arg(long, default_value = cubarium_search::es::voxel::remote::DEFAULT_REMOTE_BIN)]
        remote_bin: String,
        /// Seconds to wait, once this machine's fixtures are founded, for a remote still
        /// founding its own before starting without it (it joins later when ready).
        #[arg(long, default_value_t = 900)]
        remote_wait: u64,
        /// Worker pinning: `auto` (physical cores first, the largest L3 first, within the
        /// affinity mask), `off`, or a CPU list in order (`0-7,16-23`); worker i takes one.
        #[arg(long, default_value = cubarium_search::es::voxel::pin::DEFAULT_PIN)]
        pin: String,
    },
    /// Package S: a remote episode worker. Speaks `voxel-train --remote`'s protocol on
    /// stdin and stdout — started by it over ssh, never by hand.
    VoxelEvalWorker {
        /// Episode threads: units run at once.
        #[arg(long)]
        threads: usize,
        /// Milliseconds between heartbeats.
        #[arg(long, default_value_t = 2_000)]
        heartbeat_ms: u64,
        /// Test hook: exit abruptly after sending this many results.
        #[arg(long, hide = true)]
        die_after: Option<u64>,
        /// Test hook: go silent, pipe open, after sending this many results.
        #[arg(long, hide = true)]
        hang_after: Option<u64>,
        /// Worker pinning: `auto` (physical cores first, the largest L3 first, within the
        /// affinity mask), `off`, or a CPU list in order (`0-7,16-23`); worker i takes one.
        /// `voxel-train --remote` passes its own policy.
        #[arg(long, default_value = "off")]
        pin: String,
    },
    /// P5-C S1: a shredder landscape episode's cost with the plant leg off and on.
    VoxelFloraCost {
        #[arg(long, default_value = "small,default")]
        presets: String,
        #[arg(long, default_value_t = 24_000)]
        ticks: u64,
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::episode_worker_limit())]
        workers: usize,
    },
    /// P5-C diagnostic: held-out baselines from the drawn starts at several horizons,
    /// with and without the other lineage, and the t = 0 distance and cue per body.
    VoxelStartProbe {
        #[arg(long, default_value = "blind")]
        founder: String,
        #[arg(long, default_value = "4800,12000,24000")]
        horizons: String,
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::episode_worker_limit())]
        workers: usize,
    },
    /// P5-C: the held-out landscapes (seeds 201-208 × three presets, drained) against
    /// disclosed controls and saved policies.
    VoxelHoldout {
        /// `blind` (littershredder) or `browser` (frondgrazer).
        #[arg(long, default_value = "blind")]
        founder: String,
        /// Saved policy files to run, each its own arm.
        #[arg(long)]
        policy: Vec<PathBuf>,
        /// Comma-separated controls: heuristic, stationary-feeding, no-intake, cruise.
        #[arg(long, default_value = "heuristic,stationary-feeding")]
        controls: String,
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::episode_worker_limit())]
        workers: usize,
        /// Also print one row per fixture per arm.
        #[arg(long, default_value_t = false)]
        per_fixture: bool,
        /// Worker pinning: `auto` (physical cores first, the largest L3 first, within the
        /// affinity mask), `off`, or a CPU list in order (`0-7,16-23`); worker i takes one.
        #[arg(long, default_value = cubarium_search::es::voxel::pin::DEFAULT_PIN)]
        pin: String,
    },
    /// Voxel sensing evaluation: one saved policy or a disclosed control over the
    /// training, held-out, or balanced blind-offset diagnostic set.
    VoxelEvaluate {
        /// A saved policy file (`voxel-train`'s `centers/*.json`). Omit to run a control.
        #[arg(long)]
        policy: Option<PathBuf>,
        /// Required when a control runs; checked against a policy's lineage otherwise.
        #[arg(long)]
        founder: Option<String>,
        /// `gru` (with `--policy`), or one of the disclosed controls: `no-intake`,
        /// `stationary-feeding`, `cruise`, `heuristic`.
        #[arg(long, default_value = "gru")]
        controller: String,
        /// Diagnostic: zero every observation channel outside `Self` (indices 8..),
        /// validity included, before the GRU reads it. The body still senses; the
        /// policy's own memory is untouched. No effect on a control.
        #[arg(long, default_value_t = false)]
        ablate_senses: bool,
        /// `a` or `b`. Stage B also reports the reacquisition accounting.
        #[arg(long, default_value = "a")]
        stage: String,
        /// Stage B's successor separation band the *layouts* are built in: `landed` or
        /// `near`.
        #[arg(long, default_value = "landed")]
        band: String,
        /// The band the policy was trained on, when it differs from `--band`. Naming it
        /// declares a disclosed transfer measurement; leaving it out means the policy
        /// must carry the evaluated band's own arena protocol or be refused.
        #[arg(long)]
        policy_band: Option<String>,
        /// `training`, `holdout`, or `offset-sweep`. The sweep is blind Stage B only.
        #[arg(long, default_value = "holdout")]
        set: String,
        /// Episode horizon in ticks; defaults to the stage's own.
        #[arg(long)]
        horizon: Option<u64>,
        /// Episode workers. The runtime limit reserves ten percent of logical CPUs and
        /// stops at the measured sixteen-worker saturation point.
        #[arg(long, default_value_t = cubarium_search::es::voxel::task::episode_worker_limit())]
        workers: usize,
        #[arg(long, default_value_t = 300)]
        wall_seconds: u64,
        /// Stop after this many episode dispatches.
        #[arg(long, default_value_t = 64)]
        episode_limit: u64,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// `<lineage>=<centre.json>` arguments.
/// `--centre` when given, else the search file's `[brains]` table (`<lineage> = <file>`).
/// `--tail-cores`: `0` is every core the OS reports.
fn machine_cores(tail_cores: usize) -> usize {
    if tail_cores == 0 {
        std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
    } else {
        tail_cores
    }
}

/// The grid's `--world`: a host config file (its `[world]`, and the file as the cells'
/// base) or a preset's name.
fn world_and_base(world: &str) -> Result<(cubarium_voxel::Config, Option<PathBuf>), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = [PathBuf::from(world), root.join(world)].into_iter().find(|p| p.is_file());
    Ok(match file {
        Some(f) => (cubarium_voxel::world_config_from_file(&f)?, Some(f)),
        None => {
            let preset = cubarium_voxel::Preset::find(world)
                .ok_or_else(|| format!("--world {world}: neither a file nor a preset"))?;
            (preset.config(), None)
        }
    })
}

/// `1,3,5` or `1-4`.
fn parse_seed_list(s: &str) -> Result<Vec<u64>, String> {
    let mut out = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        if let Some((a, b)) = part.split_once('-') {
            let (a, b): (u64, u64) = (
                a.trim().parse().map_err(|e| format!("seeds {part}: {e}"))?,
                b.trim().parse().map_err(|e| format!("seeds {part}: {e}"))?,
            );
            out.extend(a..=b);
        } else {
            out.push(part.parse().map_err(|e| format!("seeds {part}: {e}"))?);
        }
    }
    if out.is_empty() {
        return Err(format!("no seeds in `{s}`"));
    }
    Ok(out)
}

/// **The start set** from `--start` / `--starts` / `--burn-in-min`: E0 by default; a
/// library opened and checked against this run (its world, its x1 `reference`, its launch
/// policy). The burn-in ends on the `window_ticks` grid of the day clock; its minimum is 0
/// on E0 (today's scoring from the founding) and 20 minutes on a library.
fn start_set(
    start: Option<&str>,
    starts: Option<&std::path::Path>,
    burn_in_min: Option<f64>,
    window_ticks: u64,
    world: &cubarium_voxel::Config,
    reference: &es::voxel::live::ecology::EcologyParams,
    centres: &[(cubarium_voxel_fauna::Founder, PathBuf)],
) -> Result<(es::voxel::live::start::StartSet, es::voxel::live::start::BurnIn), String> {
    use es::voxel::live::start::{BurnIn, Expect, StartLibrary, StartSet, policy_identity};
    let ticks = |min: f64| (min * 60.0 * f64::from(cubarium_voxel_fauna::TICK_HZ)).round() as u64;
    match (start, starts) {
        (None | Some("e0"), None) => {
            let min = ticks(burn_in_min.unwrap_or(0.0));
            Ok((StartSet::E0, BurnIn { min_ticks: min, align_ticks: if min > 0 { window_ticks } else { 0 } }))
        }
        (Some("e1"), None) => Err("--start e1 needs --starts DIR: make one with `cubarium-search voxel-starts \
             --recipe e1 --seeds 1,3 --out DIR`"
            .into()),
        (want, Some(dir)) => {
            let lib = StartLibrary::open(dir, Expect::new(world, reference, policy_identity(centres)?))?;
            if let Some(w) = want
                && w != lib.recipe
            {
                return Err(format!("--start {w}, but {} holds recipe {}", dir.display(), lib.recipe));
            }
            println!("starts: {} (seeds {:?})", lib.label(), lib.seeds);
            let burn = BurnIn { min_ticks: ticks(burn_in_min.unwrap_or(20.0)), align_ticks: window_ticks };
            Ok((StartSet::Library(std::sync::Arc::new(lib)), burn))
        }
        (Some(other), None) => Err(format!("--start takes e0 or e1, not {other}")),
    }
}

fn centres_or_brains(
    centres: &[String],
    rest: &toml::Table,
) -> Result<Vec<(cubarium_voxel_fauna::Founder, PathBuf)>, String> {
    if !centres.is_empty() {
        return parse_centres(centres);
    }
    let Some(brains) = rest.get("brains") else {
        return Ok(Vec::new());
    };
    let brains = brains.as_table().ok_or("[brains] is a table")?;
    brains
        .iter()
        .map(|(name, v)| {
            let path = v.as_str().ok_or_else(|| format!("[brains] {name} is a file"))?;
            Ok((es::voxel::parse_founder(name)?, PathBuf::from(path)))
        })
        .collect()
}

fn parse_centres(
    centres: &[String],
) -> Result<Vec<(cubarium_voxel_fauna::Founder, PathBuf)>, String> {
    centres
        .iter()
        .map(|c| {
            let (name, path) = c
                .split_once('=')
                .ok_or_else(|| format!("--centre takes <lineage>=<file>, not {c}"))?;
            Ok((es::voxel::parse_founder(name)?, PathBuf::from(path)))
        })
        .collect()
}

/// The founding table: the live world's (each lineage's `founding_count`) with the named
/// lineages set over it, `--founders frondgrazer=4,lanternjaw=0`; or one count per lineage
/// in `Founder::ALL` order, `--founders 8,8,3` (the lanternjaw's are clutches).
fn parse_founders(
    s: Option<&str>,
) -> Result<[usize; cubarium_voxel_fauna::Founder::COUNT], String> {
    parse_founders_over(s, cubarium_voxel_fauna::FaunaConfig::default().founder_counts())
}

/// [`parse_founders`] over `base`: a search file's own founding (its `[base]`'s), the
/// named lineages set over it.
fn parse_founders_over(
    s: Option<&str>,
    base: [usize; cubarium_voxel_fauna::Founder::COUNT],
) -> Result<[usize; cubarium_voxel_fauna::Founder::COUNT], String> {
    use cubarium_voxel_fauna::Founder;
    let mut out = base;
    let Some(s) = s else {
        return Ok(out);
    };
    let count = |x: &str| x.trim().parse::<usize>().map_err(|e| format!("--founders {s}: {e}"));
    if !s.contains('=') {
        let v: Vec<usize> = s.split(',').map(count).collect::<Result<_, _>>()?;
        return v
            .try_into()
            .map_err(|_| format!("--founders takes one count per lineage, not {s}"));
    }
    for part in s.split(',') {
        let (name, n) = part
            .split_once('=')
            .ok_or_else(|| format!("--founders takes <lineage>=<N>,..., not {part}"))?;
        let f = Founder::ALL
            .into_iter()
            .find(|f| f.name() == name.trim())
            .ok_or_else(|| format!("--founders: no lineage {name:?}"))?;
        out[f.index()] = count(n)?;
    }
    Ok(out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    if let Some(path) = &cli.training_ecology {
        let e = es::voxel::live::ecology::EcologyParams::load(path)?;
        es::voxel::training_world::set_training_ecology(e)?;
    }
    match cli.command {
        Command::Params => print_params(),
        Command::Baseline {
            ticks,
            sample_every,
            apex,
            workers,
            worlds,
            seed,
            out,
        } => baseline(
            ticks,
            sample_every,
            apex,
            workers,
            worlds.unwrap_or(workers),
            seed,
            out,
        ),
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
            ledger,
            plant_record,
            no_animals,
            motor,
            pursuit_stop,
            ticks,
            sample_every,
            introduce_tick,
            workers,
            wall_seconds,
            out,
        } => {
            let motor = cubarium_core::MotorModel::parse(&motor)?;
            let pursuit_stop = cubarium_search::apex_audit::parse_pursuit_stop(&pursuit_stop)?;
            let names: Vec<String> = if candidates == "all" {
                calibrate::CANDIDATES
                    .iter()
                    .map(|c| c.name.to_string())
                    .collect()
            } else {
                candidates
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
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
                cubarium_search::evaluate::RunOptions {
                    ledger,
                    plant_record,
                    no_animals,
                    precondition: None,
                    motor,
                    pursuit_stop,
                },
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
        Command::Precondition {
            stage,
            candidates,
            seed_set,
            seeds,
            ages,
            pursuit_stop,
            ticks,
            sample_every,
            workers,
            wall_seconds,
            out,
        } => {
            let names: Vec<String> = if candidates == "all" {
                calibrate::CANDIDATES
                    .iter()
                    .map(|c| c.name.to_string())
                    .collect()
            } else {
                candidates
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            };
            let ages: Vec<u64> = ages
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::parse::<u64>)
                .collect::<Result<_, _>>()?;
            let set = calibrate::SeedSet::parse(&seed_set)?;
            // The `field` stage founds no animals at all, so the rule reaches nobody there; it
            // is parsed for both stages so an unrecognised name is refused either way rather
            // than accepted and ignored.
            let pursuit_stop = cubarium_search::apex_audit::parse_pursuit_stop(&pursuit_stop)?;
            let dir = out.join(&stage);
            let report = match stage.as_str() {
                "field" => {
                    precondition::run_field(&names, set, seeds, &ages, workers, wall_seconds, &dir)?
                }
                "compare" => precondition::run_compare(
                    &names,
                    set,
                    seeds,
                    &ages,
                    ticks,
                    sample_every,
                    pursuit_stop,
                    workers,
                    wall_seconds,
                    &dir,
                )?,
                // Workstream Z: the **coupled** grazed opening. The burn-in has the ordinary
                // roster in it, so the field settles against grazing rather than against
                // nobody; the population is then removed through
                // `World::remove_all_animals` and the identical fresh roster founded into
                // the field it grazed. Age 0 is the status-quo arm and is neither burnt in
                // nor emptied — it is the reproduction target for workstream S's rows.
                "grazed" => precondition::run_grazed(
                    &names,
                    set,
                    seeds,
                    &ages,
                    ticks,
                    sample_every,
                    pursuit_stop,
                    workers,
                    wall_seconds,
                    &dir,
                )?,
                other => {
                    return Err(format!(
                        "unknown --stage {other}; use `field`, `compare` or `grazed`"
                    )
                    .into());
                }
            };
            precondition::print_report(&report);
            println!("rows and states under {}", dir.display());
            Ok(())
        }
        Command::CalibrateExport {
            candidate,
            seed,
            selected,
            why,
            out,
        } => {
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
            pursuit_stop,
            drain_every,
            wet_min,
            workers,
            cells_only,
            out,
        } => Ok(factorial::command(
            &arms,
            &seed_set,
            seeds,
            factorial::Design {
                ticks,
                warm_up_ticks,
                probe_every,
                drain_every,
                wet_min,
                pursuit_stop: cubarium_search::apex_audit::parse_pursuit_stop(&pursuit_stop)?,
            },
            workers,
            cells_only,
            &out,
        )?),
        Command::Census(args) => Ok(census::run_command(args)?),
        Command::Replay { record, index } => replay(&record, index),
        Command::VoxelCheck {
            founder,
            seeds,
            stage,
            ticks,
        } => es::voxel::commands::check(founder, stage, seeds, ticks),
        Command::VoxelBench {
            founder,
            stage,
            ticks,
            episodes,
            workers,
        } => es::voxel::commands::bench(founder, stage, ticks, episodes, workers),
        Command::VoxelLandscapes {
            founder,
            presets,
            held_out,
            ticks,
            episodes,
            workers,
            pin,
            arms,
            repeats,
            setup_pin,
        } => match arms {
            Some(arms) => es::voxel::commands::landscape_arms(
                founder, ticks, episodes, arms, repeats, setup_pin, workers,
            ),
            None => set_pin(&pin).and_then(|_| {
                es::voxel::commands::landscapes(
                    founder, presets, held_out, ticks, episodes, workers,
                )
            }),
        },
        Command::VoxelImitate {
            founder,
            stages,
            band,
            layouts,
            horizon,
            workers,
            fit,
            updates,
            learning_rate,
            chunk,
            clip,
            fit_seed,
            out,
            p5,
            write_streams,
        } => es::voxel::commands::imitate(
            founder,
            stages,
            band,
            layouts,
            horizon,
            workers,
            fit,
            updates,
            learning_rate,
            chunk,
            clip,
            fit_seed,
            out,
            p5,
            write_streams,
        ),
        Command::VoxelFloraCost {
            presets,
            ticks,
            workers,
        } => es::voxel::commands::flora_cost(presets, ticks, workers),
        Command::VoxelStartProbe {
            founder,
            horizons,
            workers,
        } => es::voxel::commands::start_probe(founder, horizons, workers),
        Command::VoxelHoldout {
            founder,
            policy,
            controls,
            workers,
            per_fixture,
            pin,
        } => set_pin(&pin).and_then(|_| {
            es::voxel::commands::holdout(founder, policy, controls, workers, per_fixture)
        }),
        Command::VoxelEcologySearch {
            centres,
            search,
            start,
            world,
            founders,
            lambda,
            seeds_per_generation,
            generations,
            wall_seconds,
            sigma0,
            threads,
            tail_cores,
            seed_base,
            cma_seed,
            hours,
            window_min,
            water_every,
            abort_animals,
            score_hours,
            band_lo,
            band_hi,
            band_peak,
            moving_min,
            lanternjaw_share,
            missing_cost,
            brain_pairs,
            brain_sigma,
            brain_learning_rate,
            out,
            persist_only,
            resume,
            curiosity,
            start_set: start_set_name,
            starts,
            burn_in_min,
        } => {
            let hz = f64::from(cubarium_voxel_fauna::TICK_HZ);
            let genome = match es::voxel::live::ecology::EcologyParams::load(&search)? {
                es::voxel::live::ecology::EcologyParams::Genome(g) => g,
                _ => return Err(format!("{} has no genes to search", search.display()).into()),
            };
            let screen = es::voxel::live::screen::ScreenSpec::from_rest(&genome.rest)?;
            let world_cfg = es::voxel::live::search::check_world_config(world.as_deref())?;
            let share: Vec<f64> = lanternjaw_share
                .split(',')
                .map(|x| x.trim().parse::<f64>())
                .collect::<Result<_, _>>()
                .map_err(|e| format!("--lanternjaw-share {lanternjaw_share}: {e}"))?;
            let [share_lo, share_hi] = share[..] else {
                return Err("--lanternjaw-share takes lo,hi".into());
            };
            let mut eval = es::voxel::live::search::EvalSpec::new(
                world_cfg,
                (hours * 3600.0 * hz).round() as u64,
                (window_min * 60.0 * hz).round() as u64,
            );
            // The search file's own founding (a `[base]`'s: the terrarium founds all nine).
            let base_counts = es::voxel::live::ecology::EcologyParams::Genome(genome.clone())
                .fauna()
                .founder_counts();
            eval.founders = parse_founders_over(founders.as_deref(), base_counts)?;
            eval.curiosity =
                es::voxel::live::curiosity::Curiosity::from_flag(curiosity.as_deref())?;
            eval.water_every = water_every;
            eval.abort_animals = abort_animals;
            eval.score_horizon_ticks = (score_hours.unwrap_or(hours) * 3600.0 * hz).round() as u64;
            eval.band = es::voxel::live::search::BandSpec {
                lo_x: band_lo,
                hi_x: band_hi,
                peak_x: band_peak,
                moving_min,
                missing_cost,
                lanternjaw_share: [share_lo, share_hi],
                persist_only: [false; cubarium_voxel_fauna::Founder::COUNT],
                ..Default::default()
            }
            .with_targets(&genome.rest)?;
            if let Some(list) = persist_only {
                let names: Vec<String> = if list == "none" {
                    Vec::new()
                } else {
                    list.split(',').map(|x| x.trim().to_string()).collect()
                };
                let mut t = toml::Table::new();
                let mut inner = toml::Table::new();
                inner.insert(
                    "persist_only".into(),
                    toml::Value::Array(names.into_iter().map(toml::Value::String).collect()),
                );
                t.insert("targets".into(), toml::Value::Table(inner));
                eval.band = eval.band.with_targets(&t)?;
            }
            let start = match start {
                Some(p) => match es::voxel::live::ecology::EcologyParams::load(&p)? {
                    es::voxel::live::ecology::EcologyParams::Genome(g) => Some(g),
                    _ => return Err(format!("--start {} has no genes", p.display()).into()),
                },
                None => None,
            };
            let centres = centres_or_brains(&centres, &genome.rest)?;
            let reference = match &start {
                Some(g) => es::voxel::live::search::start_from(&genome, g)?,
                None => genome.clone(),
            };
            (eval.start, eval.burn_in) = start_set(
                start_set_name.as_deref(),
                starts.as_deref(),
                burn_in_min,
                (window_min * 60.0 * hz).round() as u64,
                &eval.world,
                &es::voxel::live::ecology::EcologyParams::Genome(reference),
                &centres,
            )?;
            let spec = es::voxel::live::search::SearchSpec {
                centres,
                genome,
                start,
                screen,
                lambda,
                seeds_per_generation,
                generations,
                wall_seconds,
                sigma0,
                threads,
                tail_cores: machine_cores(tail_cores),
                seed_base,
                cma_seed,
                eval,
                brain_pairs,
                brain_sigma,
                brain_learning_rate,
                out,
                resume,
            };
            es::voxel::live::search::search(&spec)?;
            Ok(())
        }
        Command::VoxelEcologyScreen { arm, world, search } => {
            let e = if arm == "default" {
                es::voxel::live::ecology::EcologyParams::defaults()
            } else {
                es::voxel::live::ecology::EcologyParams::load(std::path::Path::new(&arm))?
            };
            let screen = match search {
                Some(p) => match es::voxel::live::ecology::EcologyParams::load(&p)? {
                    es::voxel::live::ecology::EcologyParams::Genome(g) => {
                        es::voxel::live::screen::ScreenSpec::from_rest(&g.rest)?
                    }
                    _ => es::voxel::live::screen::ScreenSpec::default(),
                },
                // A genome arm carries its search file's `[screen]`.
                None => match e.genome() {
                    Some(g) => es::voxel::live::screen::ScreenSpec::from_rest(&g.rest)?,
                    None => es::voxel::live::screen::ScreenSpec::default(),
                },
            };
            let world_cfg = es::voxel::live::search::check_world_config(world.as_deref())?;
            let r = es::voxel::live::search::screen_of(&e, &world_cfg, &screen);
            println!(
                "R {:.4}{}  S {:.4}  F {:.2}  need {:.3e}/s  {}",
                r.r,
                if screen.r_readout { " (a readout)" } else { "" },
                r.s,
                r.f,
                r.need_per_s,
                if r.passes(&screen) {
                    "passes".to_string()
                } else {
                    format!("fails: {}", r.failures(&screen).join("; "))
                }
            );
            for s in &r.species {
                println!(
                    "  {:<14} n={:<5.1} edible={:<9.4} regrow={:<10.3e} R={:<8.4} S={:<8.4} F={:.2}",
                    s.species, s.stands_in_reach, s.edible, s.regrowth_per_s, s.r, s.s, s.f
                );
            }
            let roster = es::voxel::live::search::roster_of(&e, &world_cfg, &screen);
            for n in &roster.night {
                println!(
                    "night {:<14} sleeps {:>6.0} s  endures {:>8.0} s  x{:<6.3} {}",
                    n.lineage,
                    n.sleep_s,
                    n.endurance_s,
                    n.ratio,
                    if n.passes { "ok" } else { "REFUSED" }
                );
            }
            for f in &roster.food {
                let foods: Vec<String> =
                    f.foods.iter().map(|x| format!("{}={:.3}", x.food, x.per_day)).collect();
                println!(
                    "food  {:<14} bodies {:<4} need {:>8.3}/day  supply {:>9.3}/day  x{:<7.3}{} {}  [{}]",
                    f.lineage,
                    f.bodies,
                    f.need_per_day,
                    f.supply_per_day,
                    f.ratio,
                    f.reach.map(|r| format!(" reach x{r:.2}")).unwrap_or_default(),
                    if f.passes { "ok" } else { "REFUSED" },
                    foods.join(" ")
                );
            }
            for b in &roster.balance {
                println!(
                    "balance {:<12} (readout) founds {} supports {:.2} on {:.3}/day of prey",
                    b.predator, b.predators, b.supported, b.prey_per_day
                );
            }
            let refusals = roster.refusals();
            println!(
                "{}",
                if refusals.is_empty() && r.passes(&screen) {
                    "every screen passes".to_string()
                } else {
                    format!("refused: {}", refusals.join("; "))
                }
            );
            Ok(())
        }
        Command::VoxelEcologyCheck {
            centres,
            founders,
            arms,
            seeds,
            hours,
            window_min,
            water_every,
            threads,
            tail_cores,
            out,
            world,
            curiosity,
            finalist,
            finalist_from,
            targets,
        } => {
            let world_cfg =
                es::voxel::live::search::check_world_config(world.as_deref())?;
            let mut parsed = Vec::new();
            for a in &arms {
                let (name, src) = a
                    .split_once('=')
                    .ok_or_else(|| format!("--arm takes <name>=default|<file.toml>, not {a}"))?;
                let params = if src == "default" {
                    es::voxel::live::ecology::EcologyParams::defaults()
                } else {
                    es::voxel::live::ecology::EcologyParams::load(std::path::Path::new(src))?
                };
                parsed.push((name.to_string(), params));
            }
            let seeds: Vec<u64> = seeds
                .split(',')
                .map(|s| s.trim().parse::<u64>().map_err(|e| format!("seed {s}: {e}")))
                .collect::<Result<_, _>>()?;
            // The first arm's own founding and brains when none are given (a `[base]`
            // search genome founds all nine; its `[brains]` name the held centres).
            let first = parsed.first().map(|(_, p)| p.clone()).unwrap_or_default();
            let base_counts = first.fauna().founder_counts();
            let rest = first.genome().map(|g| g.rest.clone()).unwrap_or_default();
            let centres = centres_or_brains(&centres, &rest)?;
            let founders = parse_founders_over(founders.as_deref(), base_counts)?;
            if finalist || targets {
                es::voxel::live::search::finalist(
                    &world_cfg,
                    &parsed,
                    &seeds,
                    (!finalist).then_some(hours),
                    finalist_from,
                    window_min,
                    water_every,
                    &centres,
                    founders,
                    threads,
                    machine_cores(tail_cores),
                    &out,
                )?;
                return Ok(());
            }
            es::voxel::live::search::check(
                &world_cfg,
                &parsed,
                &seeds,
                hours,
                window_min,
                water_every,
                &centres,
                founders,
                threads,
                &es::voxel::live::curiosity::Curiosity::from_flag(curiosity.as_deref())?,
                &out,
            )?;
            Ok(())
        }
        Command::VoxelEcologyGrid {
            m,
            p,
            axes,
            seeds,
            first_seed,
            hours,
            world,
            threads,
            tail_cores,
            water_every,
            abort_animals,
            centres,
            out,
            start,
            starts,
            burn_in_min,
            seed_list,
        } => {
            use es::voxel::live::grid;
            let mut parsed = Vec::new();
            for (name, levels) in [("m", &m), ("p", &p)] {
                if levels.trim() != "none" {
                    parsed.push(grid::Axis::named(name, &grid::parse_values(levels)?)?);
                }
            }
            for a in &axes {
                parsed.push(grid::Axis::parse(a)?);
            }
            let (world_cfg, base) = world_and_base(&world)?;
            let centres = centres_or_brains(&centres, &toml::Table::new())?;
            let (start, burn_in) = start_set(
                start.as_deref(),
                starts.as_deref(),
                burn_in_min,
                grid::WINDOW_MIN * es::voxel::live::window::TICKS_PER_MIN,
                &world_cfg,
                &grid::neutral_ecology(base.as_deref())?,
                &centres,
            )?;
            let seed_list = seed_list.map(|l| parse_seed_list(&l)).transpose()?;
            grid::grid(&grid::GridSpec {
                world: world_cfg,
                world_name: world,
                base,
                axes: parsed,
                seeds,
                first_seed,
                hours,
                threads,
                tail_cores: machine_cores(tail_cores),
                water_every,
                abort_animals,
                centres,
                out,
                start,
                burn_in,
                seed_list,
            })?;
            Ok(())
        }
        Command::VoxelStarts {
            recipe,
            seeds,
            world,
            centres,
            recipe_file,
            plant_hours,
            threads,
            sim_threads,
            out,
        } => {
            use es::voxel::live::{assembly, grid, start};
            let (world_cfg, base) = world_and_base(&world)?;
            let centres = centres_or_brains(&centres, &toml::Table::new())?;
            let mut e1 = match recipe_file {
                Some(p) => toml::from_str::<assembly::E1Recipe>(
                    &std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?,
                )
                .map_err(|e| format!("{}: {e}", p.display()))?,
                None => assembly::E1Recipe::default(),
            };
            if let Some(h) = plant_hours {
                e1.plant_hours = h;
            }
            assembly::generate_library(&assembly::StartsSpec {
                recipe,
                seeds: parse_seed_list(&seeds)?,
                world: world_cfg,
                world_name: world,
                eco: grid::neutral_ecology(base.as_deref())?,
                e1,
                policy: start::policy_identity(&centres)?,
                threads,
                sim_threads,
                out,
            })?;
            Ok(())
        }
        Command::VoxelLiveTrain {
            centres,
            worlds,
            threads,
            pairs,
            window_min,
            generations,
            wall_seconds,
            sigma,
            learning_rate,
            train_seed,
            world_seed,
            water_every,
            population_cap,
            save_every,
            out,
            world,
            ecology,
            curiosity,
        } => {
            let ecology = match ecology {
                Some(p) => es::voxel::live::ecology::EcologyParams::load(&p)?,
                None => es::voxel::live::ecology::EcologyParams::defaults(),
            };
            let spec = es::voxel::live::trainer::LiveSpec {
                world: match &world {
                    Some(p) => Some(cubarium_voxel::world_config_from_file(p)?),
                    None => None,
                },
                ecology,
                centres: parse_centres(&centres)?,
                worlds,
                threads,
                pairs,
                window_ticks: (window_min * 60.0 * f64::from(cubarium_voxel_fauna::TICK_HZ))
                    .round() as u64,
                generations,
                wall_seconds,
                sigma,
                learning_rate,
                train_seed,
                world_seed,
                water_every,
                population_cap,
                save_every,
                out,
                curiosity: es::voxel::live::curiosity::Curiosity::from_flag(curiosity.as_deref())?,
            };
            es::voxel::live::trainer::train(&spec)?;
            Ok(())
        }
        Command::VoxelTrain {
            founder,
            stage,
            band,
            init_center,
            controller,
            pairs,
            layouts,
            updates,
            horizon,
            workers,
            wall_seconds,
            episode_limit,
            train_seed,
            center_eval,
            overwrite,
            out,
            p5,
            landscapes_per_generation,
            held_out_every,
            transfer_from,
            plateau,
            remotes,
            remote_bin,
            remote_wait,
            pin,
        } => {
            es::voxel::training_world::check_remote(remotes.len())?;
            if overwrite {
                let _ = std::fs::remove_dir_all(&out);
            }
            let pin = set_pin(&pin)?;
            es::voxel::commands::train(es::voxel::commands::TrainArgs {
                founder,
                stage,
                band,
                init_center,
                transfer_from,
                controller,
                pairs,
                layouts,
                updates,
                horizon,
                workers,
                wall_seconds,
                episode_limit,
                train_seed,
                center_eval,
                out,
                p5,
                per_generation: landscapes_per_generation,
                held_out_every,
                plateau,
                remotes,
                remote_bin,
                remote_wait,
                pin,
            })
        }
        Command::VoxelEvalWorker {
            threads,
            heartbeat_ms,
            die_after,
            hang_after,
            pin,
        } => set_pin(&pin).and_then(|_| {
            es::voxel::remote::serve(es::voxel::remote::WorkerOptions {
                threads,
                heartbeat: std::time::Duration::from_millis(heartbeat_ms.max(1)),
                die_after,
                hang_after,
            })
            .map_err(Into::into)
        }),
        Command::VoxelEvaluate {
            policy,
            founder,
            controller,
            ablate_senses,
            stage,
            band,
            policy_band,
            set,
            horizon,
            workers,
            wall_seconds,
            episode_limit,
            out,
        } => es::voxel::commands::evaluate(
            policy,
            founder,
            controller,
            ablate_senses,
            stage,
            band,
            policy_band,
            set,
            horizon,
            workers,
            wall_seconds,
            episode_limit,
            out,
        ),
        Command::EsProtocol { config } => es::commands::protocol(config),
        Command::EsControls {
            workers,
            wall_seconds,
            config,
            out,
        } => es::commands::controls(workers, wall_seconds, config, out),
        Command::EsSmoke { config, out } => es::commands::smoke(config, out),
        Command::EsBench {
            ticks,
            workers,
            config,
        } => es::commands::bench(ticks, workers, config),
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
            motor,
            adapter,
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
            cubarium_core::MotorModel::parse(&motor)?,
            cubarium_core::neural::ActionAdapter::parse(&adapter)?,
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
            motor,
            adapter,
            out,
        } => {
            let probe = es::commands::EvalProbe {
                reset_hidden_every,
                copies,
            };
            let motor = cubarium_core::MotorModel::parse(&motor)?;
            let adapter = cubarium_core::neural::ActionAdapter::parse(&adapter)?;
            es::commands::evaluate(
                policy,
                &set,
                horizon,
                wall_seconds,
                probe,
                config,
                motor,
                adapter,
                out,
            )
        }
        Command::EsBudget {
            policy,
            config,
            horizon,
            initial_seed,
            workers,
            wall_seconds,
            out,
        } => es::budget::run(
            policy,
            config,
            horizon,
            initial_seed,
            workers,
            wall_seconds,
            out,
        ),
        Command::ApexAudit {
            config,
            seeds,
            apex,
            ticks,
            introduce_tick,
            founder_age_seconds,
            no_ledger,
            pursuit_stop,
            motor,
            apex_turn_radius,
            apex_motor,
            workers,
            wall_seconds,
            out,
        } => {
            let arm = cubarium_search::apex_audit::Arm {
                apex,
                horizon: ticks,
                introduce_tick,
                founder_age_seconds,
                ledger: !no_ledger,
                stop: cubarium_search::apex_audit::parse_pursuit_stop(&pursuit_stop)?,
                motor: cubarium_search::apex_audit::parse_motor(&motor)?,
                apex_turn_radius: cubarium_search::apex_audit::parse_apex_turn_radius(
                    &apex_turn_radius,
                )?,
                apex_motor: apex_motor
                    .as_deref()
                    .map(cubarium_search::apex_audit::parse_apex_motor)
                    .transpose()?,
            };
            cubarium_search::apex_audit::run(
                config
                    .split(',')
                    .map(|s| PathBuf::from(s.trim()))
                    .filter(|p| !p.as_os_str().is_empty())
                    .collect(),
                seeds,
                arm,
                workers,
                wall_seconds,
                out,
            )
        }
        Command::EsTurnBand {
            run,
            config,
            generation,
            horizon,
            workers,
            wall_seconds,
            pairs,
            bootstrap,
            out,
        } => es::turnband::run(
            run,
            config,
            generation,
            horizon,
            workers,
            wall_seconds,
            pairs,
            bootstrap,
            out,
        ),
        Command::EsExport {
            checkpoint,
            config,
            generation,
            out,
            verify_ticks,
        } => es::commands::export(checkpoint, config, generation, out, verify_ticks),
        Command::EsPopulation {
            policy,
            config,
            seeds,
            arms,
            copies,
            ticks,
            sample_every,
            introduce_tick,
            pursuit_stop,
            workers,
            wall_seconds,
            adapter,
            out,
        } => {
            // The stage runner is another workstream's file this round, so it was not opened
            // to the switch. Refuse the other adapter by name rather than run `cub-act-1` under
            // a `cub-act-2` flag.
            if cubarium_core::neural::ActionAdapter::parse(&adapter)?
                != cubarium_core::neural::ActionAdapter::CubAct1
            {
                return Err(
                    "es-population runs the shipped `cub-act-1` adapter only: its stage runner \
                     (crates/cubarium-search/src/population.rs) was not opened to the action \
                     adapter switch in workstream X. Evaluate a cub-act-2 policy with \
                     `es-evaluate --adapter cub-act-2`."
                        .into(),
                );
            }
            let ecology = es::Ecology::load(&config)?;
            let pursuit_stop = cubarium_search::apex_audit::parse_pursuit_stop(&pursuit_stop)?;
            let arms: Vec<u32> = arms
                .split(',')
                .map(|s| s.trim().parse::<u32>())
                .collect::<Result<_, _>>()?;
            let seeds: Vec<u64> = TRAINING_SEEDS
                .get(..seeds)
                .ok_or_else(|| {
                    format!("seed count {seeds} is outside 1..={}", TRAINING_SEEDS.len())
                })?
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
                pursuit_stop,
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

/// Set the process's episode-worker pinning from `--pin` (`es::voxel::pin`).
fn set_pin(pin: &str) -> Result<es::voxel::pin::PinSpec, Box<dyn std::error::Error>> {
    let spec = es::voxel::pin::PinSpec::parse(pin)?;
    es::voxel::pin::set_process(&spec)?;
    Ok(spec)
}

fn print_params() -> Result<(), Box<dyn std::error::Error>> {
    println!("build {BUILD_ID}");
    println!("\n{} searched parameters\n", params::PARAMS.len());
    println!(
        "{:<34} {:>10} {:>10} {:>10}  unit / why",
        "field", "default", "lo", "hi"
    );
    for p in params::PARAMS {
        println!(
            "{:<34} {:>10} {:>10} {:>10}  {}",
            p.name, p.default, p.lo, p.hi, p.unit
        );
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
            s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| {
                l.split_whitespace()
                    .nth(1)
                    .and_then(|k| k.parse::<f64>().ok())
            })
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
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&rows_path)?,
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
                && let Err(e) = serde_json::to_writer(&mut rows, row)
                    .and_then(|()| rows.write_all(b"\n").map_err(serde_json::Error::io))
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
    println!(
        "\ncandidate  gen origin      status                fitness  pers plnt turn matr lin  vary apex"
    );
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
                    c.candidate,
                    c.generation,
                    c.origin,
                    status,
                    f,
                    a[0],
                    a[1],
                    a[2],
                    a[3],
                    a[4],
                    a[5],
                    a[6]
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
        report
            .nondominated
            .iter()
            .map(|i| report.candidates[*i].candidate)
            .collect::<Vec<_>>()
    );
    if let Some(best) = report.best_by_scalar {
        println!(
            "best by scalar: candidate {}",
            report.candidates[best].candidate
        );
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
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or("param_bits holds a non-string")
        })
        .collect::<Result<_, _>>()?;
    let values = params::from_bit_labels(&bits)?;
    let recorded_fingerprint = row["param_fingerprint"]
        .as_u64()
        .ok_or("row has no fingerprint")?;
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

    println!(
        "status   recorded {recorded_status:?}  replayed {:?}",
        again.status
    );
    println!("ecology  recorded {recorded_hash:?}  replayed {replayed_hash:?}");
    let matched = recorded_status == again.status && recorded_hash == replayed_hash;
    println!("{}", if matched { "REPRODUCED" } else { "DIVERGED" });
    if !matched {
        return Err("replay did not reproduce the recorded row".into());
    }
    Ok(())
}
