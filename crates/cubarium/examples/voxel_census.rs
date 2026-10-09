//! Voxel census: does the seeded habitat last unattended?
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_census -- 6 > runs/voxel-census-6h.csv
//! ```
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_census -- 6 generated closed \
//!     > runs/voxel-census-generated-closed-6h.csv
//! ```
//!
//! Builds a world — the authored fixture by default, or the core's generator — seeds it
//! with the standard habitat, and steps the
//! coupled simulation for the given number of simulated hours (default 6). Every simulated
//! minute — 1,200 ticks at [`cubarium_voxel_fauna::TICK_HZ`] — it prints one CSV row to
//! stdout: the per-species stand counts, the per-species animal counts and mean body, the
//! per-founder-lineage counts and mean body, the four ledger birth/death counters, and the
//! total litter organic. The seed report goes to
//! stderr so it never mixes with the CSV.
//!
//! Every lineage runs its observation-only heuristic by default, as in the ambient run.
//! `policy=LINEAGE=FILE` explicitly installs a saved policy for that lineage.
//! A trailing `heuristic` argument is accepted as a legacy no-op:
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_census -- 1 generated closed heuristic
//! ```
//!
//! `save=<dir>` [`save_min=<m>`] writes the whole run once, at sim minute `m` (default:
//! the run's last whole minute), as the three layers' own always-fresh envelopes —
//! `world.bin`, `flora.bin`, `fauna.bin` — plus `minute`: the same directory
//! `voxel_dense` writes and reads. `load=<dir>` starts from such a directory instead of
//! founding a world: the founders' drivers are re-installed and the senses re-settled,
//! as the host does at start, and the rows continue from the saved minute. A stale
//! envelope is refused by the layer's own loader. The generation arguments (`preset=`,
//! `config=`, `seed=`, `generated`, `closed`, `nofauna`) are refused beside `load=`.
//! `set=flora.…` / `set=fauna.…` beside `load=` change the saved run's own plant and
//! animal configs (not the code defaults), the drivers re-installed on the result; a
//! plant value is in its authored unit and rescaled to the world's cell size exactly as on
//! a fresh run, and the resolved set goes to `params.toml` as below. `set=world.…` is
//! refused: the save fixes the world, climate and day clock included. A rate changed on a
//! branch from a save shows a transient away from the save's state first, not the new
//! rate's equilibrium — an arm about the equilibrium is founded under its own config.
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_census -- 6 preset=default save=runs/h6
//! cargo run --release -p cubarium --example voxel_census -- 1 load=runs/h6 threads=16
//! ```
//!
//! Predation (P1, `design/predation-and-flight-2026-09-28.md`): a fresh world is founded
//! with its lanternjaw **egg bank**; `eggbank=off` founds the same world without it (the
//! control arm), and `introduce=lanternjaw:N` adds N adult lanternjaws where the prey
//! live once the world is founded. The rows carry the predation columns at the end.
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_census -- 8 preset=small seed=1 heuristic
//! cargo run --release -p cubarium --example voxel_census -- 8 preset=small seed=1 heuristic eggbank=off
//! ```
//!
//! `set=PATH=VALUE` (repeatable) sets one parameter by its registry path over the arm's
//! world and the default plant and animal configs
//! (`design/ecology-parameter-registry-2026-09-28.md`; `cargo run -p cubarium-voxel-sim
//! --example params` lists every path). The run's resolved set is written as
//! `params.toml` into the `save=` directory, and to `params=<file>` when given — beside
//! the CSV the run's stdout is redirected to.
//!
//! `genome=FILE` (a search file, `config/training/search/*.toml`) with `gene=NAME=VALUE`
//! (repeatable, an expanded gene name such as `upkeep@frondgrazer`; unnamed genes stay at
//! their file value, x1 by default) decodes the genome over the arm's own configs — the
//! `config=` file's tables on a fresh run, the save's configs beside `load=` — and applies
//! the changed leaves as `set=`s ahead of the explicit ones (`genome_sets`). `expand=OUT`
//! writes the expanded genome (with those values) to `OUT`, prints its `set=` lines on
//! stdout and exits without running: what `scripts/studies/eco_search.py` reads.
//!
//! `gap=off` turns the climate's dry gap off on a `preset=` or `config=` arm.
//! `--weather-log FILE` writes one CSV row per rain event that ends during the run (start
//! and end tick, mode, cause, RH, °C and day phase at the start, seconds since the
//! previous event, volume, and the moisture `M` at the start), and the run ends with a `weather:` line on stderr: events per
//! mode, forced starts, gap-blocked draws per mode, strikes, rain by mode and by quarter
//! of the day, and the highest RH, all over the run alone. The rows gain the weather's
//! own columns (`temp_c` … `day_phase`, `stream_m3_s`, `moist`;
//! `design/handoffs/voxel-weather-2026-09-24.md`).
//!
//! `vitals=DIR` opts into `vitals.csv`, `plants_vitals.csv`, `life.csv` and `meta.json`
//! in DIR. Activity is sampled each simulated second; life transitions and 3-D path
//! length are polled each tick. `stuck_window_min=N` defaults to 5 simulated minutes and
//! `stuck_body_lengths=N` defaults to 1 current body length; `stuck_dump=FILE` (debug)
//! writes one row per exposed body per minute with its window's states, steer sources and
//! latches (`vitals::STUCK_DUMP_HEADER`). The stuck rule, the two
//! energy readouts (`survivor_stock_change_per_body_hour`, `cohort_energy_change`),
//! `delivered_bl_per_s` and the ring-wrapped distances are defined in
//! `cubarium_observatory::vitals`. Activity shares overlap (e.g. moving and fleeing);
//! denominators are awake individual-seconds. Fractional final minutes get an endpoint
//! row. The census rows (stdout) always start with the run's own start minute: 0 on a
//! founding, the load minute on `load=` or `resume=`, before any tick.
//!
//! Fullness keeps its pooled columns and appends `fullness_p10_STAGE`,
//! `fullness_p50_STAGE`, `n_fullness_STAGE`, `starving_n_STAGE` for rider, pup,
//! juvenile and adult, classified by `query::stage_of` at each minute. Stage fields
//! are empty without living bodies. `starving_fullness=N` (default 0.05) counts
//! reserve / reserve capacity strictly below N; it is a diagnostic, not a rule.
//!
//! **Segments.** Every row of the three CSVs carries `segment`: 0 for a fresh founding,
//! 1 for a `load=` branch, and the previous segment + 1 for each `resume=`.
//! - A founding or `load=` starts the files fresh and writes a row at its start minute.
//! - `resume=` continues the same files: it first drops every row the checkpoint at the
//!   resume minute `m` does not hold (minute rows after `m`; `life.csv` deaths after `m`
//!   and living rows last seen after `m` — a crash may have written past the
//!   checkpoint), then appends without a second header. The previous segment's row at
//!   `m` (it carries the minute before) is kept and the resumed segment does not repeat
//!   it, so every minute appears once.
//! - Histories do not cross a boundary: bodies alive at a load or resume start new
//!   `origin=observed` life rows with their own `id`, so a reducer joins the segments by
//!   `id`. A segment that ended cleanly wrote its living rows (`alive=true`) at its end
//!   minute; one that crashed did not, and its living bodies' earlier meals, path and
//!   activity are lost. Stuck windows restart at a boundary.
//! - `meta.json` (rewritten at each start and end): the world's `voxel_m`, `width`,
//!   `depth`, `height`, `ring_m` (x circumference), `seed`, the current `segment`, its
//!   `minute_start`/`minute_end` (planned, then actual), the binary's `git_commit`, and
//!   `segments`, one entry per segment kept (kind found/load/resume).
//!
//! `overlap=0` runs the chained tick instead of the overlapped one (`SimConfig::overlap`),
//! and `ab=MIN` flips between the two every simulated minute from minute `MIN` on, so the
//! two ticks are timed on one world. `modes=c,6:6,5:1` makes that rotation a list: `c` is
//! the chained tick and `W:B` the overlapped one with `W` water and `B` plant-and-animal
//! workers (`cubarium_voxel_sim::set_overlap_split`). Every minute a `phases:` line on
//! stderr gives that minute's milliseconds per tick by phase.

use cubarium::voxel::VoxelConfig;
use cubarium::voxel::habitat;
use cubarium::voxel::{install_founders_with, policy_args};
use cubarium::voxel::scene;
use cubarium_voxel_sim::staged;
use cubarium_voxel::{Command as WorldCommand, World};
use cubarium_voxel_fauna::{
    Departure, Fauna, Founder, Senses, Species as Beast, TICK_HZ,
};
use cubarium_voxel_flora::{Flora, Species as Plant};
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::{Sim, SimConfig};
use cubarium_observatory::vitals::{self, Options as VitalsOptions, Recorder as VitalsRecorder};
use std::io::Write;

/// One simulated minute, in ticks: 60 s at the fixed tick rate.
const TICKS_PER_MIN: u64 = 60 * TICK_HZ as u64;

/// The closed budget's shower rate: the rain rate the retired flora study harness ran
/// on. An experiment condition.
const HARNESS_RAIN_M_PER_S: f64 = 0.0002;
/// Evaporation for the closed budget: half the shower rate. It has to be **under** the
/// shower rate or a falling drop is lifted again in the same tick — `evaporate` runs
/// right after `rain` — and no rain ever reaches the soil. An experiment condition, not
/// a model default, and the same one `cubarium-voxel`'s `water_cycle` example uses.
const CLOSED_EVAPORATION_M_PER_S: f64 = 0.0001;

/// The species whose deaths by cause the rows carry: the turf, the pad and the two
/// woody species the contest is about.
const CONTEST_SPECIES: [Plant; 4] = [Plant::Springturf, Plant::Velvetpad, Plant::Lanternberry, Plant::Vaulttree];

/// First seed a `preset=` arm offers the host's lake gate; `seed=N` for another. The
/// host draws at random, and an arm has to be re-runnable.
const PRESET_SEED_BASE: u64 = 1;

/// The rows' patch columns count patches of at least this many faces
/// (`meadow_patch_min_faces=`): 25, a five-by-five lawn. A readout knob, not a sim one.
const MEADOW_PATCH_MIN_FACES: usize = 25;

/// The plants whose connected patches the rows carry (MEADOW-3,
/// [`cubarium::voxel::patches`]): the meadow, the groves and the forest.
const PATCH_SPECIES: [(Plant, &str); 3] = [(Plant::Springturf, "meadow"), (Plant::Lanternberry, "grove"), (Plant::Vaulttree, "forest")];

/// Arguments that only shape a fresh world: `resume=` leaves them out (the checkpoint
/// carries the world). A bare word matches exactly; `k=` matches a prefix.
const FOUNDING_ONLY: &[&str] = &[
    "preset=", "config=", "seed=", "generated", "closed", "nofauna", "set=", "eggbank=", "introduce=", "world=",
    "gap=", "night=", "still", "save=", "save_min=", "params=", "genome=", "gene=", "expand=",
];

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    // `resume=DIR` continues a staged run from its latest checkpoint
    // (`cubarium_voxel_sim::staged`).
    let args = staged::resume_args(&raw, FOUNDING_ONLY).unwrap_or_else(|e| panic!("{e}"));
    // Training, search and experiment runs take the coarse water (`water_every=`).
    cubarium_voxel_sim::experiment_args(&args);
    let stage = staged::StageConfig::from_args(&args).unwrap_or_else(|e| panic!("{e}"));
    let stage = stage.map(|cfg| (cfg, args.clone()));
    // `threads=N`: this run's share of the machine. The gate runs many of these side by
    // side; without it each takes every core (`cubarium_voxel::set_thread_override`).
    if let Some(n) = args
        .iter()
        .find_map(|a| a.strip_prefix("threads=").and_then(|s| s.parse().ok()))
    {
        cubarium_voxel::set_thread_override(n);
    }
    // `policy=<lineage>=<file>` explicitly replaces that lineage's default heuristic.
    let policies = policy_args(&args).expect("policy=<lineage>=<file>");
    let hours: f64 = args.get(1).map_or(6.0, |a| {
        a.parse()
            .expect("usage: voxel_census [HOURS] [authored|generated] [open|closed] [heuristic]")
    });
    let generated = args.iter().any(|a| a == "generated");
    let closed = args.iter().any(|a| a == "closed");
    // `preset=<small|default|wide>` is the landscape arm: the world `cubarium voxel`
    // builds with no TOML for that preset, not the world this file builds for itself.
    // Same construction as `voxel_founder_autopsy`'s arm, on purpose.
    let preset: Option<&'static cubarium_voxel::Preset> = args
        .iter()
        .find_map(|a| a.strip_prefix("preset="))
        .map(|name| {
            cubarium_voxel::Preset::find(name).unwrap_or_else(|| {
                let known: Vec<&str> = cubarium_voxel::PRESETS.iter().map(|p| p.name).collect();
                panic!("no landform preset is called {name:?}; the shipped ones are {known:?}")
            })
        });
    let seed_base: u64 = args
        .iter()
        .find_map(|a| a.strip_prefix("seed=").and_then(|s| s.parse().ok()))
        .unwrap_or(PRESET_SEED_BASE);
    // `heuristic` remains accepted as a legacy no-op; explicit policies win.
    // `pin=B`: the overlapped tick's plant and animal leg on the last `B` physical cores,
    // the water on the rest (`chiplet::split_overlap_pools`). Measurement.
    if let Some(b) = args
        .iter()
        .find_map(|a| a.strip_prefix("pin=").and_then(|s| s.parse().ok()))
    {
        cubarium::voxel::chiplet::split_overlap_pools(b)
            .expect("pin=B needs more than B physical cores");
    }
    let timing = Timing {
        vitals: args.iter().find_map(|a| a.strip_prefix("vitals=")).map(|dir| {
            let number = |name: &str, default: f64| {
                args.iter().find_map(|a| a.strip_prefix(name))
                    .map_or(default, |v| v.parse().expect("vitals option needs a number"))
            };
            // The defaults are `VitalsOptions::default()`, which the Godot session's
            // recorder runs with too.
            let d = VitalsOptions::default();
            (dir.to_string(), VitalsOptions {
                stuck_window_min: number("stuck_window_min=", d.stuck_window_min),
                stuck_body_lengths: number("stuck_body_lengths=", d.stuck_body_lengths),
                starving_fullness: number("starving_fullness=", d.starving_fullness),
            })
        }),
        policies: policies.clone(),
        patch_min: args
            .iter()
            .find_map(|a| a.strip_prefix("meadow_patch_min_faces="))
            .map_or(MEADOW_PATCH_MIN_FACES, |v| v.parse().expect("meadow_patch_min_faces=N")),
        weather_log: args
            .iter()
            .position(|a| a == "--weather-log")
            .map(|i| args.get(i + 1).expect("--weather-log FILE").clone()),
        overlap: args
            .iter()
            .find_map(|a| a.strip_prefix("overlap="))
            .map(|v| v != "0"),
        ab_from_min: args
            .iter()
            .find_map(|a| a.strip_prefix("ab=").and_then(|s| s.parse().ok())),
        nice_water: args
            .iter()
            .find_map(|a| a.strip_prefix("nice_water=").and_then(|s| s.parse().ok())),
        modes: args
            .iter()
            .find_map(|a| a.strip_prefix("modes="))
            .map_or_else(
                || vec![None, Some(None)],
                |list| {
                    list.split(',')
                        .map(|m| {
                            if m == "c" {
                                return None;
                            }
                            let (w, b) = m.split_once(':').expect("a mode is c or W:B");
                            Some(Some((w.parse().expect("W"), b.parse().expect("B"))))
                        })
                        .collect()
                },
            ),
        segment_kind: if staged::resumed(&args) {
            SegmentKind::Resume
        } else if args.iter().any(|a| a.starts_with("load=")) {
            SegmentKind::Load
        } else {
            SegmentKind::Found
        },
    };
    let save = SaveAt::from_args(&args);
    // `set=PATH=VALUE`: the parameter layers over each arm's world and the default plant
    // and animal configs; the resolved set goes beside the run's output.
    let sets: Vec<String> = args
        .iter()
        .filter_map(|a| a.strip_prefix("set=").map(str::to_string))
        .collect();
    let params_file = args.iter().find_map(|a| a.strip_prefix("params="));
    let genome = genome_args(&args);
    let expand = args.iter().find_map(|a| a.strip_prefix("expand=")).map(str::to_string);
    // `config=<path>`'s own `[flora]` / `[fauna]` tables (a preset's founding table, say),
    // under the `set=`s, exactly as `cubarium voxel` reads that file.
    let file_tables: Option<toml::Table> =
        args.iter().find_map(|a| a.strip_prefix("config=")).map(|p| {
            let cfg = cubarium::voxel::load_config(std::path::Path::new(p))
                .unwrap_or_else(|e| panic!("config {p:?}: {e}"));
            let mut file = toml::Table::new();
            if !cfg.sim.flora.is_empty() {
                file.insert("flora".into(), toml::Value::Table(cfg.sim.flora));
            }
            if !cfg.sim.fauna.is_empty() {
                file.insert("fauna".into(), toml::Value::Table(cfg.sim.fauna));
            }
            file
        });
    let write_params = |params: &Params| {
        let mut written = Vec::new();
        if let Some(s) = &save {
            written.push(params.write_beside(&s.dir).map(|p| p.display().to_string()));
        }
        if let Some(file) = params_file {
            written.push(
                params
                    .to_toml()
                    .and_then(|t| std::fs::write(file, t).map_err(|e| e.to_string()))
                    .map(|()| file.to_string()),
            );
        }
        for w in written {
            match w {
                Ok(path) => eprintln!("parameters in {path}"),
                Err(e) => eprintln!("could not write the parameter set: {e}"),
            }
        }
    };
    // The genome's `set=`s over a fresh arm: decoded over the `config=` file's tables.
    let fresh_genome_sets = |world: cubarium_voxel::Config| -> Vec<String> {
        let Some((text, genes)) = &genome else { return Vec::new() };
        let base = Params::resolve_over(
            Params { world, ..Params::defaults() },
            &Layers { file: file_tables.clone(), ..Layers::default() },
        )
        .unwrap_or_else(|e| panic!("the parameter set: {e}"));
        genome_or_expand(&base, text, genes, expand.as_deref())
    };
    if expand.is_some() && !args.iter().any(|a| a.starts_with("load=")) {
        fresh_genome_sets(cubarium_voxel::Config::default());
        unreachable!("expand= exits");
    }
    let resolve = |world: cubarium_voxel::Config| -> Params {
        let mut all = fresh_genome_sets(world.clone());
        all.extend(sets.iter().cloned());
        let params = Params::resolve_over(
            Params {
                world,
                ..Params::defaults()
            },
            &Layers {
                file: file_tables.clone(),
                sets: all,
                ..Layers::default()
            },
        )
        .unwrap_or_else(|e| panic!("the parameter set: {e}"));
        write_params(&params);
        params
    };
    if let Some(dir) = args.iter().find_map(|a| a.strip_prefix("load=")) {
        let (mut sim, minute0) = load_run(
            std::path::Path::new(dir),
            &args,
            &policies,
            &sets,
            genome.as_ref(),
            expand.as_deref(),
            &write_params,
        );
        run(&mut sim, hours, minute0, save.as_ref(), &timing, stage);
        return;
    }
    let nofauna = args.iter().any(|a| a == "nofauna");
    // `eggbank=off`: the predation control arm, the same world with no egg bank.
    let egg_bank = !args.iter().any(|a| a == "eggbank=off");
    // The founding table: each lineage's `founding_count` in the arm's resolved animal
    // config (the code default, a `config=` file's `[fauna.founders.<lineage>]`, a `set=`).
    let founder_counts = |fauna: &cubarium_voxel_fauna::FaunaConfig| {
        if nofauna {
            [0; Founder::COUNT]
        } else if egg_bank {
            fauna.founder_counts()
        } else {
            fauna.founder_counts_without_egg_bank()
        }
    };
    // `introduce=lanternjaw:N`: N adult lanternjaws where the prey live, after founding.
    let introduce: usize = args
        .iter()
        .find_map(|a| a.strip_prefix("introduce=lanternjaw:"))
        .map_or(0, |n| n.parse().expect("introduce=lanternjaw:N"));
    let introduce_into = |world: &World, flora: &Flora, fauna: &mut Fauna| {
        if introduce > 0 {
            match habitat::introduce_lanternjaws(world, flora, fauna, introduce) {
                Ok(ids) => eprintln!("introduced {} adult lanternjaws", ids.len()),
                Err(why) => eprintln!("introduce=lanternjaw:{introduce} refused — {why}"),
            }
        }
    };
    // `world=<file>` starts from a live host's world snapshot (terrain and water only), and
    // seeds it the way `cubarium voxel` seeds a resumed world: pre-roll without the opening
    // shower, then the standard habitat.
    if let Some(path) = args.iter().find_map(|a| a.strip_prefix("world=")) {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("world {path}: {e}"));
        let mut world = World::load(&bytes).unwrap_or_else(|e| panic!("load world: {e:#}"));
        let params = resolve(world.config().clone());
        let mut flora = Flora::new(params.flora_for_world());
        let mut fauna = Fauna::new(params.fauna.clone());
        let pre = habitat::pre_roll(&mut world, false);
        let seeded =
            habitat::seed_pre_rolled(&mut world, &mut flora, &mut fauna, &pre, founder_counts(&params.fauna));
        eprintln!(
            "scene: world {path} ({}x{}x{} at {} m, tick {}); seeded stands={} animals={}",
            world.config().width,
            world.config().height,
            world.config().depth,
            world.config().voxel_m,
            world.tick(),
            seeded.stands,
            seeded.animals()
        );
        install_founders_with(&mut fauna, &policies).expect("the explicit founder policies validate");
        introduce_into(&world, &flora, &mut fauna);
        let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
        run(&mut sim, hours, 0, save.as_ref(), &timing, stage);
        return;
    }
    // `config=<path>` is a landscape arm from a host TOML (the Tachyon terrarium, say),
    // founded exactly like a preset arm. `nofauna` seeds no founders: the plants alone.
    let file_arm: Option<(String, cubarium_voxel::Config)> = args
        .iter()
        .find_map(|a| a.strip_prefix("config="))
        .map(|p| {
            let cfg = cubarium::voxel::load_config(std::path::Path::new(p))
                .unwrap_or_else(|e| panic!("config {p:?}: {e}"));
            (p.to_string(), cfg.sim.world)
        });
    let arm: Option<(String, cubarium_voxel::Config)> =
        file_arm.or_else(|| {
            preset.map(|p| {
                // `still`: the live-training world's water (`live::world::still_water`).
                if args.iter().any(|a| a == "still") {
                    (
                        format!("{} (still)", p.name),
                        cubarium_search::es::voxel::live::world::still_water(p.config()),
                    )
                } else {
                    (p.name.to_string(), p.config())
                }
            })
        });
    let cfg = VoxelConfig::default();
    // `gap=off`: the climate's dry gap at 0 / 0, so every natural start is the weather's
    // own (WX1's gap-off arms).
    let gap_off = args.iter().any(|a| a == "gap=off");
    if let Some((arm_name, mut arm_config)) = arm {
        // `night=F`: the plant night factor, `[world.day] plant_night_factor` (WX3 round 4).
        if let Some(f) = args
            .iter()
            .find_map(|a| a.strip_prefix("night=").and_then(|s| s.parse::<f64>().ok()))
        {
            arm_config.day.plant_night_factor = f;
            eprintln!("plant_night_factor {f}");
        }
        if gap_off {
            arm_config.climate.dry_gap_min_s = 0.0;
            arm_config.climate.dry_gap_max_s = 0.0;
        }
        assert!(!generated, "a preset arm builds its own world");
        // The host's founding loop, verbatim: lake gate, pre-roll with the opening
        // shower, seeding and the acceptance check, redrawn on a refusal. The world
        // comes back already seeded.
        let params = resolve(arm_config);
        let arm_config = params.world.clone();
        let founding_started = std::time::Instant::now();
        let founded = cubarium::voxel::found(
            &arm_config,
            &cubarium::voxel::LayerConfig::new(&params, arm_config.voxel_m),
            founder_counts(&params.fauna),
            cubarium::voxel::Seeds::from(seed_base),
            false,
        );
        eprintln!("founded in {:.1} s", founding_started.elapsed().as_secs_f64());
        let (world, flora, mut fauna, seeded) =
            (founded.world, founded.flora, founded.fauna, founded.seeded);
        eprintln!(
            "scene: preset {} ({}x{}x{} at {} m, seed {}, {} lake / {} habitat rejected, {})",
            arm_name,
            world.config().width,
            world.config().height,
            world.config().depth,
            world.config().voxel_m,
            founded.seed,
            founded.lake_rejected,
            founded.habitat_rejected,
            if founded.accepted { "accepted" } else { "NOT accepted" },
        );
        eprintln!(
            "seeded: stands={} logs={} litter_tiles={} founders={:?} animals={} \
             lanternjaw_eggs={}{} land_m2={:.1} land_scale={:.3} wanted={:?} shortfall={:?}",
            seeded.stands,
            seeded.logs,
            seeded.litter_tiles,
            seeded.founders,
            seeded.animals(),
            seeded.founding_eggs,
            seeded
                .founding_bank_refused
                .as_ref()
                .map_or(String::new(), |why| format!(" (no egg bank: {why})")),
            seeded.land_m2,
            seeded.land_scale,
            seeded.wanted,
            seeded.shortfall,
        );
        install_founders_with(&mut fauna, &policies).expect("the explicit founder policies validate");
        introduce_into(&world, &flora, &mut fauna);
        let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
        run(&mut sim, hours, 0, save.as_ref(), &timing, stage);
        return;
    }
    let params = resolve(cfg.sim.world.clone());
    let mut world = if generated {
        // The closed budget's water is the retired flora study's: the shower rate it ran on,
        // the aquifer charged a metre above the basin floor, and the outlet open — which
        // in a closed world is the return flow into the atmosphere, not an export.
        let mut world_cfg = params.world.clone();
        if closed {
            let dry = VoxelConfig::default().sim.world;
            let basin_floor_m = World::new(dry.clone())
                .outlet_cell()
                .map_or(0.0, |(_, y, _)| f64::from(y))
                * dry.voxel_m;
            world_cfg = cubarium_voxel::Config {
                rain_m_per_s: HARNESS_RAIN_M_PER_S,
                evaporation_m_per_s: CLOSED_EVAPORATION_M_PER_S,
                initial_aquifer_head_m: basin_floor_m + 1.0,
                closed_water_budget: true,
                ..world_cfg
            };
        }
        let mut w = World::new(world_cfg);
        if closed {
            w.apply(WorldCommand::SetOutlet { open: true });
        }
        w
    } else {
        assert!(
            !closed,
            "the closed budget is measured on the generated world"
        );
        scene::authored(params.world.clone())
    };
    eprintln!(
        "scene: {} world, {} water budget",
        if generated { "generated" } else { "authored" },
        if closed { "closed" } else { "open" }
    );

    let mut flora = Flora::new(params.flora.clone());
    let mut fauna = Fauna::new(params.fauna.clone());
    let seeded = habitat::seed(&mut world, &mut flora, &mut fauna);
    eprintln!(
        "seeded: stands={} logs={} litter_tiles={} founders={:?} animals={}",
        seeded.stands,
        seeded.logs,
        seeded.litter_tiles,
        seeded.founders,
        seeded.animals()
    );
    install_founders_with(&mut fauna, &policies).expect("the explicit founder policies validate");
    introduce_into(&world, &flora, &mut fauna);

    // The live founders sense a settled litter field; the ambient run settles it the
    // same way before the first tick.
    let mut senses = Senses::with_cue(fauna.config().cue);
    senses.settle(&world.view(), &flora.view());
    let mut sim = Sim::new(world, flora, fauna, SimConfig::experiment(), Some(senses));

    run(&mut sim, hours, 0, save.as_ref(), &timing, stage);
}

/// How the tick is run and timed: `overlap=`, `ab=` and `modes=`.
#[derive(Clone)]
struct Timing {
    vitals: Option<(String, VitalsOptions)>,
    policies: Vec<(Founder, std::path::PathBuf)>,
    /// `--weather-log FILE`: one row per finished rain event.
    weather_log: Option<String>,
    overlap: Option<bool>,
    ab_from_min: Option<u64>,
    nice_water: Option<i32>,
    /// The `ab=` rotation: `None` the chained tick, `Some(split)` the overlapped one
    /// (`Some(None)` at its default split).
    modes: Vec<Option<Option<(usize, usize)>>>,
    /// `meadow_patch_min_faces=N` (default [`MEADOW_PATCH_MIN_FACES`]): the size a patch
    /// must reach to count in the rows' `*_patches` columns.
    patch_min: usize,
    /// How this process's run began: the vitals readouts' segment rule.
    segment_kind: SegmentKind,
}

/// How a process's run began, for the vitals readouts (doc comment, `vitals=`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum SegmentKind {
    Found,
    Load,
    Resume,
}

/// Step the coupled layers for the asked-for hours, writing one row a simulated minute.
/// Both arms end here, so a preset world is measured by the same code path as the ridge.
/// A loaded run starts at `minute0` and prints from the minute after it.
fn run(
    sim: &mut Sim,
    hours: f64,
    minute0: u64,
    save: Option<&SaveAt>,
    timing: &Timing,
    stage: Option<(staged::StageConfig, Vec<String>)>,
) {
    for lineage in Founder::ALL {
        let fauna = sim.fauna();
        let source = if fauna.policy_driven(lineage) {
            timing.policies.iter().find(|(f, _)| *f == lineage).map_or_else(
                || "explicit policy".into(),
                |(_, path)| format!("file {}", path.display()))
        } else { "heuristic".into() };
        eprintln!("installed_driver lineage={} source={source} weights={:016x}",
            lineage.name(), fauna.policy_digest(lineage));
    }
    if let Some(on) = timing.overlap {
        sim.set_overlap(on);
    }
    // `nice_water=N`: the water pools' workers at nice N (measurement). `Sim::new`
    // built the pools.
    if let Some(n) = timing.nice_water {
        cubarium::voxel::chiplet::nice_water_pools(n);
    }
    let mut total_ticks = (hours * 3600.0 * f64::from(TICK_HZ)) as u64;
    // A staged run (`run=DIR`): checkpoints, `STOP`, the liveness check; a resumed one
    // keeps its own end.
    let mut runner = stage.map(|(cfg, args)| {
        let end = minute0 + total_ticks / TICKS_PER_MIN;
        staged::Runner::start(cfg, &args, sim, minute0, end).unwrap_or_else(|e| {
            eprintln!("staged: the load check failed — {e}");
            std::process::exit(2)
        })
    });
    if let Some(r) = &runner {
        total_ticks = (r.end_minute() - minute0) * TICKS_PER_MIN;
        eprintln!("staged run {} to minute {}", r.dir().display(), r.end_minute());
    }
    let save_min = save.map(|s| s.minute.unwrap_or(minute0 + total_ticks / TICKS_PER_MIN));
    eprintln!("census: actual world seed {}", sim.world().config().seed);
    // The reachability readout at founding (training maps): per lineage, how its walkable
    // land splits under its own route map; `reach.tsv` beside the vitals when asked.
    if minute0 == 0 {
        let reach = habitat::reachability(sim.world(), sim.flora(), sim.fauna());
        let mut tsv = format!("{}\n", habitat::Reach::TSV_HEADER);
        for (f, r) in Founder::ALL.iter().zip(reach.iter()) {
            if r.flyer {
                eprintln!("reach {}: flyer, bodies={}", f.name(), r.bodies);
            } else {
                eprintln!(
                    "reach {}: largest {:.1}% of {} faces ({} components), food in {} components{}, bodies {}/{} in largest",
                    f.name(),
                    100.0 * r.largest_share,
                    r.faces,
                    r.components,
                    r.food_components,
                    if r.largest_has_food { " incl. largest" } else { "" },
                    r.bodies_in_largest,
                    r.bodies
                );
            }
            tsv.push_str(&r.tsv_row(*f));
            tsv.push('\n');
        }
        if let Some((dir, _)) = &timing.vitals {
            let path = std::path::Path::new(dir).join("reach.tsv");
            if let Err(e) = std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&path, tsv)) {
                eprintln!("could not write {}: {e}", path.display());
            }
        }
    }
    print_header();
    // Every run's first row is its start: a founding's minute 0, or the load minute of a
    // loaded or resumed one, before any tick.
    print_row(minute0, sim, 0.0, timing.patch_min);
    let mut tick = minute0 * TICKS_PER_MIN;
    let total_ticks = tick + total_ticks;
    let mut window = SeedWindow::default();
    let mut phases = PhaseWindow::default();
    let mut since = std::time::Instant::now();
    let mut weather = WeatherLog::new(sim, timing.weather_log.as_deref());
    let mut deaths = DeathLog::new(sim);
    let mut spells = SpellLog::new(sim);
    let mut lj = LjChain::new(sim);
    let mut vitals = timing.vitals.as_ref().map(|(dir, options)| {
        VitalsLog::new(dir, *options, sim, minute0, total_ticks / TICKS_PER_MIN, timing.segment_kind)
            .unwrap_or_else(|e| panic!("vitals readouts in {dir}: {e}"))
    });
    let mut vitals_time = std::time::Duration::ZERO;
    let first_tick = tick;
    while tick < total_ticks {
        lj.before(sim);
        let attached_before = sim.fauna().view().ledger.riders_attached;
        sim.step();
        lj.after(sim, attached_before);
        tick += 1;
        if let Some(v) = vitals.as_mut() {
            let start = std::time::Instant::now();
            v.recorder.observe(sim.fauna(), tick);
            if tick % u64::from(TICK_HZ) == 0 || tick == total_ticks {
                v.recorder.sample(sim.fauna(), tick as f64 / f64::from(TICK_HZ));
            }
            vitals_time += start.elapsed();
        }
        weather.poll(sim);
        deaths.poll(sim, tick);
        if let Some(r) = runner.as_mut() {
            r.tick(sim);
        }
        if tick % TICKS_PER_MIN == 0 {
            let ms = since.elapsed().as_secs_f64() * 1e3 / TICKS_PER_MIN as f64;
            let minute = tick / TICKS_PER_MIN;
            if let Some(v) = vitals.as_mut() {
                let start = std::time::Instant::now();
                v.minute(sim, minute as f64).expect("write vitals minute");
                vitals_time += start.elapsed();
            }
            print_row(minute, sim, ms, timing.patch_min);
            spells.minute(sim, minute);
            phases.report(minute, sim, ms);
            if let Some(from) = timing.ab_from_min.filter(|from| minute >= *from) {
                let n = timing.modes.len() as u64;
                match timing.modes[((minute - from + 1) % n) as usize] {
                    None => sim.set_overlap(false),
                    Some(split) => {
                        cubarium_voxel_sim::set_overlap_split(split);
                        sim.set_overlap(true);
                    }
                }
            }
            if let Some(s) = save
                && save_min == Some(tick / TICKS_PER_MIN)
            {
                s.write(sim, tick / TICKS_PER_MIN);
            }
            since = std::time::Instant::now();
            if let Some(r) = runner.as_mut()
                && let Some(ended) = r.minute(sim, tick / TICKS_PER_MIN)
            {
                eprintln!("staged: {ended:?} at minute {}", tick / TICKS_PER_MIN);
                if ended != staged::Ended::Done {
                    window.report(tick, sim);
                    break;
                }
            }
        }
        if tick % (60 * TICKS_PER_MIN) == 0 || tick == total_ticks {
            window.report(tick, sim);
        }
    }
    if let Some(v) = vitals.as_mut() {
        let start = std::time::Instant::now();
        if tick % TICKS_PER_MIN != 0 {
            v.minute(sim, tick as f64 / TICKS_PER_MIN as f64).expect("write final vitals");
        }
        v.finish(tick as f64 / TICKS_PER_MIN as f64).expect("finish life table");
        vitals_time += start.elapsed();
        eprintln!("vitals: readout_ms_per_tick={:.6} ticks={}",
            vitals_time.as_secs_f64() * 1e3 / (tick - first_tick).max(1) as f64,
            tick - first_tick);
    }
    weather.summary(sim);
    spells.end(tick / TICKS_PER_MIN);
    lj.summary(sim);
    forage_summary(sim);
}

/// FORAGE-LEAVE's readout over the run (it is session state, so a segment's own): one
/// `forage:` line per lineage whose heuristic held still to feed — stays ended, seconds
/// spent in stays, the longest ended stay, the stays still under way and the longest of
/// them, the mean interval offer (a fraction of the rate that fills its stomach), the
/// share of offers under `give_up_fraction`, patches given up and samples walked off one.
fn forage_summary(sim: &Sim) {
    let fauna = sim.fauna();
    let tally = fauna.forage_tally();
    let open = fauna.forage_stays();
    let animals = fauna.view().animals;
    for f in cubarium_voxel_fauna::Founder::ALL {
        let t = tally[f.index()];
        let mine: Vec<f64> = open
            .iter()
            .filter(|(id, _, _)| animals.iter().any(|a| a.id == *id && a.founder == Some(f)))
            .map(|o| o.1)
            .collect();
        if t.stay_samples == 0 && mine.is_empty() {
            continue;
        }
        let samples = t.offer_samples.max(1) as f64;
        eprintln!(
            "forage: {} stays={} stay_s={:.0} max_ended_s={:.0} open={} max_open_s={:.0} \
             mean_offer={:.3} low_offer_share={:.3} give_ups={} leave_samples={}",
            f.name(),
            t.stays,
            t.stay_s,
            t.stay_max_s,
            mine.len(),
            mine.iter().copied().fold(0.0, f64::max),
            t.offer_sum / samples,
            t.offer_low as f64 / samples,
            t.give_ups,
            t.leave_samples,
        );
    }
}

/// Streaming deaths and minute snapshots; living life-table rows appear at run end.
/// The segment rule is in the doc comment (`vitals=`).
struct VitalsLog {
    recorder: VitalsRecorder,
    animals: std::io::BufWriter<std::fs::File>,
    plants: std::io::BufWriter<std::fs::File>,
    lives: std::io::BufWriter<std::fs::File>,
    meta_path: std::path::PathBuf,
    meta: serde_json::Value,
}

impl VitalsLog {
    fn new(
        dir: &str,
        options: VitalsOptions,
        sim: &Sim,
        minute0: u64,
        end_minute: u64,
        kind: SegmentKind,
    ) -> Result<Self, String> {
        let dir = std::path::Path::new(dir);
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let meta_path = dir.join("meta.json");
        let minute = minute0 as f64;
        // A resume continues the files; a founding or a load starts them fresh.
        let previous: Option<serde_json::Value> = (kind == SegmentKind::Resume)
            .then(|| std::fs::read_to_string(&meta_path).ok())
            .flatten()
            .and_then(|t| serde_json::from_str(&t).ok());
        let segment = match kind {
            SegmentKind::Found => 0,
            SegmentKind::Load => 1,
            SegmentKind::Resume => previous
                .as_ref()
                .and_then(|m| m["segment"].as_u64())
                .map_or(1, |s| s as u32 + 1),
        };
        let mut boundary_written = false;
        let open = |name: &str, header: &str, readout: vitals::Readout, boundary: &mut bool| {
            let path = dir.join(name);
            let existing = match kind {
                SegmentKind::Resume => std::fs::read_to_string(&path).ok(),
                _ => None,
            };
            let text = match existing {
                Some(text) => {
                    let (kept, at) = vitals::keep_through(&text, header, readout, minute)
                        .map_err(|e| format!("{}: {e}", path.display()))?;
                    *boundary |= at;
                    kept
                }
                None => format!("{header}\n"),
            };
            std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .map(std::io::BufWriter::new)
                .map_err(|e| format!("{}: {e}", path.display()))
        };
        let mut at_animals = false;
        let animals = open("vitals.csv", vitals::VITALS_HEADER, vitals::Readout::Minutes, &mut at_animals)?;
        let mut at_plants = false;
        let plants = open("plants_vitals.csv", vitals::PLANTS_HEADER, vitals::Readout::Minutes, &mut at_plants)?;
        let mut unused = false;
        let lives = open("life.csv", &vitals::life_header(), vitals::Readout::Life, &mut unused)?;
        let _ = at_plants;
        boundary_written |= at_animals;
        let config = sim.world().config();
        let ring_m = f64::from(config.width) * config.voxel_m;
        let mut segments: Vec<serde_json::Value> = previous
            .as_ref()
            .and_then(|m| m["segments"].as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter(|s| s["minute_start"].as_f64().is_some_and(|m| m < minute))
            .collect();
        segments.push(serde_json::json!({
            "segment": segment,
            "kind": format!("{kind:?}").to_lowercase(),
            "minute_start": minute,
            "minute_end": end_minute as f64,
            "git_commit": env!("CUBARIUM_GIT_HASH"),
        }));
        let meta = serde_json::json!({
            "voxel_m": config.voxel_m,
            "width": config.width,
            "depth": config.depth,
            "height": config.height,
            "ring_m": ring_m,
            "seed": config.seed,
            "segment": segment,
            "minute_start": minute,
            "minute_end": end_minute as f64,
            "git_commit": env!("CUBARIUM_GIT_HASH"),
            "segments": segments,
            "starving_fullness": options.starving_fullness,
        });
        let mut recorder = VitalsRecorder::new(options, ring_m, segment, sim.fauna(), minute);
        // `stuck_dump=FILE` (debug): every body the stuck rule exposes, per minute.
        if let Some(path) = std::env::args().find_map(|a| a.strip_prefix("stuck_dump=").map(str::to_string)) {
            let file = std::fs::File::create(&path).map_err(|e| format!("{path}: {e}"))?;
            recorder
                .set_stuck_dump(Box::new(std::io::LineWriter::new(file)))
                .map_err(|e| format!("{path}: {e}"))?;
        }
        let mut log = Self {
            recorder,
            animals,
            plants,
            lives,
            meta_path,
            meta,
        };
        log.write_meta().map_err(|e| e.to_string())?;
        // The load boundary's row, unless the resumed files already end with it.
        if boundary_written {
            let _ = log.recorder.minute(sim.flora(), sim.fauna(), minute);
        } else {
            log.minute(sim, minute).map_err(|e| e.to_string())?;
        }
        Ok(log)
    }

    fn write_meta(&self) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(&self.meta).map_err(std::io::Error::other)?;
        std::fs::write(&self.meta_path, text + "\n")
    }

    fn minute(&mut self, sim: &Sim, minute: f64) -> std::io::Result<()> {
        let (animals, plants) = self.recorder.minute(sim.flora(), sim.fauna(), minute);
        for r in animals { r.write_csv(&mut self.animals)?; }
        for r in plants { r.write_csv(&mut self.plants)?; }
        for r in self.recorder.take_dead() { r.write_csv(&mut self.lives)?; }
        self.animals.flush()?; self.plants.flush()?; self.lives.flush()
    }

    /// Living rows at the actual end minute, and the segment's end in `meta.json`.
    fn finish(&mut self, end_minute: f64) -> std::io::Result<()> {
        for r in self.recorder.take_dead() { r.write_csv(&mut self.lives)?; }
        for r in self.recorder.lives().filter(|r| r.died_minute.is_none()) { r.write_csv(&mut self.lives)?; }
        self.lives.flush()?;
        self.meta["minute_end"] = end_minute.into();
        if let Some(last) = self.meta["segments"].as_array_mut().and_then(|s| s.last_mut()) {
            last["minute_end"] = end_minute.into();
        }
        self.write_meta()
    }
}

/// `SPELLLOG=path` (env, package DORMANCY): **dry spells under the turf**. Each minute,
/// every springturf stand whose root box reads below the species' wake water
/// (`cure_resume_water`, through its moisture ramp: `μ < moisture_at(resume)`) is in a
/// spell; one line per spell when it ends: `site,start_min,end_min,len_min,how` with `how`
/// `wet` (the water came back), `died` (the stand is gone) or `run_end`, and whether the
/// stand was cured at its end. The ground's dry spells as the turf on it reads them.
struct SpellLog {
    out: Option<std::io::BufWriter<std::fs::File>>,
    open: std::collections::HashMap<u64, (cubarium_voxel_flora::Site, u64, bool)>,
    mu_wake: f64,
}

impl SpellLog {
    fn new(sim: &Sim) -> SpellLog {
        use std::io::Write;
        let out = std::env::var("SPELLLOG").ok().map(|p| {
            let mut f = std::io::BufWriter::new(std::fs::File::create(p).expect("spell log"));
            writeln!(f, "x,y,z,start_min,end_min,len_min,how,cured").unwrap();
            f
        });
        let sc = sim.flora().config().species(Plant::Springturf);
        SpellLog { out, open: std::collections::HashMap::new(), mu_wake: sc.moisture_at(sc.cure_resume_water) }
    }

    fn line(&mut self, site: cubarium_voxel_flora::Site, start: u64, end: u64, how: &str, cured: bool) {
        use std::io::Write;
        if let Some(f) = self.out.as_mut() {
            writeln!(f, "{},{},{},{start},{end},{},{how},{}", site.x, site.y, site.z, end - start, u8::from(cured)).unwrap();
        }
    }

    fn minute(&mut self, sim: &Sim, minute: u64) {
        if self.out.is_none() {
            return;
        }
        let f = sim.flora().view();
        let mut seen = std::collections::HashSet::with_capacity(self.open.len());
        let mut ended = Vec::new();
        for st in f.stands.iter().filter(|s| s.species == Plant::Springturf) {
            let dry = st.moisture < self.mu_wake;
            match (dry, self.open.get_mut(&st.id)) {
                (true, Some(e)) => {
                    e.2 = st.cured;
                    seen.insert(st.id);
                }
                (true, None) => {
                    self.open.insert(st.id, (st.site, minute, st.cured));
                    seen.insert(st.id);
                }
                (false, Some(_)) => ended.push((st.id, "wet", st.cured)),
                (false, None) => {}
            }
        }
        for (id, how, cured) in ended {
            if let Some((site, start, _)) = self.open.remove(&id) {
                self.line(site, start, minute, how, cured);
            }
        }
        let gone: Vec<u64> = self.open.keys().copied().filter(|id| !seen.contains(id)).collect();
        for id in gone {
            if let Some((site, start, cured)) = self.open.remove(&id) {
                self.line(site, start, minute, "died", cured);
            }
        }
        if minute % 30 == 0
            && let Some(f) = self.out.as_mut()
        {
            use std::io::Write;
            f.flush().ok();
        }
    }

    fn end(&mut self, minute: u64) {
        let open: Vec<_> = self.open.drain().collect();
        for (_, (site, start, cured)) in open {
            self.line(site, start, minute, "run_end", cured);
        }
        if let Some(f) = self.out.as_mut() {
            use std::io::Write;
            f.flush().ok();
        }
    }
}

/// `--weather-log`: the finished events, and the run's weather totals.
struct WeatherLog {
    out: Option<std::io::BufWriter<std::fs::File>>,
    seen: u64,
    start: cubarium_voxel::weather::WeatherCounts,
}

impl WeatherLog {
    fn new(sim: &Sim, path: Option<&str>) -> WeatherLog {
        use std::io::Write;
        let out = path.map(|p| {
            let mut f = std::io::BufWriter::new(
                std::fs::File::create(p).unwrap_or_else(|e| panic!("--weather-log {p}: {e}")),
            );
            writeln!(
                f,
                "start_tick,end_tick,mode,cause,rh,temp_c,day_phase,since_prev_s,volume_m3,moist"
            )
            .expect("the weather log");
            f
        });
        let counts = sim.world().weather().counts;
        WeatherLog {
            out,
            seen: counts.events_finished,
            start: counts,
        }
    }

    fn poll(&mut self, sim: &Sim) {
        use std::io::Write;
        let wx = sim.world().weather();
        if wx.counts.events_finished == self.seen {
            return;
        }
        self.seen = wx.counts.events_finished;
        let (Some(out), Some(e)) = (self.out.as_mut(), wx.last_finished()) else {
            return;
        };
        writeln!(
            out,
            "{},{},{:?},{:?},{:.4},{:.3},{:.4},{},{:.5},{:.4}",
            e.start_tick,
            e.end_tick,
            e.mode,
            e.cause,
            e.rh,
            e.temperature_c,
            e.day_phase,
            e.since_previous_s.map_or(String::new(), |s| format!("{s:.1}")),
            e.volume_m3,
            e.moisture,
        )
        .expect("the weather log");
        // A few rows an hour: flush each, so a run cut short keeps its log.
        out.flush().expect("the weather log");
    }

    fn summary(&mut self, sim: &Sim) {
        use std::io::Write;
        if let Some(out) = self.out.as_mut() {
            out.flush().expect("the weather log");
        }
        let (c, s) = (sim.world().weather().counts, self.start);
        let d = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let q = |a: [f64; 4], b: [f64; 4]| [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]];
        let g = |a: [u64; 3], b: [u64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let by_mode = d(c.rain_by_mode_m3, s.rain_by_mode_m3);
        let by_quarter = q(c.rain_by_quarter_m3, s.rain_by_quarter_m3);
        eprintln!(
            "weather: drizzles={} showers={} downpours={} forced={} gap_blocked={:?} \
             strikes={} rain_mode_m3={:.3}/{:.3}/{:.3} rain_quarter_m3={:.3}/{:.3}/{:.3}/{:.3} \
             max_rh={:.3}",
            c.drizzles - s.drizzles,
            c.showers - s.showers,
            c.downpours - s.downpours,
            c.forced - s.forced,
            g(c.gap_blocked, s.gap_blocked),
            c.strikes - s.strikes,
            by_mode[0],
            by_mode[1],
            by_mode[2],
            by_quarter[0],
            by_quarter[1],
            by_quarter[2],
            by_quarter[3],
            c.max_rh,
        );
    }
}

/// `save=<dir>` and `save_min=<m>`: where and when the run writes itself once.
struct SaveAt {
    dir: std::path::PathBuf,
    minute: Option<u64>,
}

impl SaveAt {
    fn from_args(args: &[String]) -> Option<SaveAt> {
        let arg = |k: &str| args.iter().find_map(|a| a.strip_prefix(k));
        let minute = arg("save_min=").map(|m| m.parse().expect("save_min=<sim minute>"));
        let Some(dir) = arg("save=") else {
            assert!(minute.is_none(), "save_min= needs save=<dir>");
            return None;
        };
        Some(SaveAt {
            dir: dir.into(),
            minute,
        })
    }

    /// The three layers' own envelopes and the minute, as `voxel_dense` lays them out.
    fn write(&self, sim: &Sim, minute: u64) {
        let (world, flora, fauna) = sim.layers();
        let d = &self.dir;
        std::fs::create_dir_all(d).unwrap_or_else(|e| panic!("save dir {}: {e}", d.display()));
        let put = |name: &str, bytes: Vec<u8>| {
            std::fs::write(d.join(name), bytes)
                .unwrap_or_else(|e| panic!("writing {}: {e}", d.join(name).display()))
        };
        put("world.bin", world.save());
        put("flora.bin", cubarium_voxel_flora::snapshot::encode(flora));
        put("fauna.bin", fauna.save());
        put("minute", minute.to_string().into_bytes());
        eprintln!(
            "saved {} at minute {minute}: stands {} animals {}",
            d.display(),
            flora.view().stands.len(),
            fauna.view().animals.len()
        );
    }
}

/// `genome=FILE` and its `gene=NAME=VALUE`s: the file's text and the values.
fn genome_args(args: &[String]) -> Option<(String, Vec<(String, f64)>)> {
    let genes: Vec<(String, f64)> = args
        .iter()
        .filter_map(|a| a.strip_prefix("gene="))
        .map(|g| cubarium_voxel_sim::params::parse_gene_arg(g).unwrap_or_else(|e| panic!("{e}")))
        .collect();
    let path = args.iter().find_map(|a| a.strip_prefix("genome="));
    assert!(path.is_some() || genes.is_empty(), "gene= needs genome=FILE");
    path.map(|p| {
        let text = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("genome={p}: {e}"));
        (text, genes)
    })
}

/// The genome's `set=`s over `base`; with `expand=OUT`, the expanded genome goes to `OUT`,
/// the `set=` lines to stdout, and the process exits.
fn genome_or_expand(base: &Params, text: &str, genes: &[(String, f64)], expand: Option<&str>) -> Vec<String> {
    let (g, sets) = cubarium_voxel_sim::params::genome_sets(base, text, genes)
        .unwrap_or_else(|e| panic!("genome: {e}"));
    if let Some(out) = expand {
        std::fs::write(out, g.to_toml("expanded by voxel_census expand=; values as given by gene="))
            .unwrap_or_else(|e| panic!("expand={out}: {e}"));
        for s in &sets {
            println!("set={s}");
        }
        std::process::exit(0);
    }
    eprintln!("genome: {} ({} set=s)", g.describe(), sets.len());
    sets
}

/// `load=<dir>`: a saved run, re-assembled the way the founding arms assemble a new one.
/// The world's shape, water and seed live in the file, so the generation arguments are
/// refused rather than silently ignored.
fn load_run(
    dir: &std::path::Path,
    args: &[String],
    policies: &[(Founder, std::path::PathBuf)],
    sets: &[String],
    genome: Option<&(String, Vec<(String, f64)>)>,
    expand: Option<&str>,
    write_params: &dyn Fn(&Params),
) -> (Sim, u64) {
    let shaping: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| {
            ["preset=", "config=", "seed="].iter().any(|k| a.starts_with(k))
                || ["generated", "closed", "nofauna"].contains(a)
        })
        .collect();
    assert!(
        shaping.is_empty(),
        "load= resumes a saved world; {shaping:?} only shape a new one — drop them"
    );
    // The one safe load: every body's heuristic back, and a lineage the save ran on a
    // policy refused unless this run explicitly supplies it (`policy=`).
    let driven_by_policy = cubarium::voxel::policy_founders(policies);
    let saved = cubarium_voxel_sim::saved::load(dir, &driven_by_policy).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, flora, mut fauna, minute) = (saved.world, saved.flora, saved.fauna, saved.minute);
    eprintln!(
        "loaded {} at minute {minute}: {}x{}x{} stands {} animals {}",
        dir.display(),
        world.config().width,
        world.config().height,
        world.config().depth,
        flora.view().stands.len(),
        fauna.view().animals.len()
    );
    // `set=` over the loaded layers: resolved over the save's own configs, the plant
    // values in their authored unit and rescaled to the world's cell size as a fresh run
    // would ([`Params::over_loaded`]).
    let mut flora = flora;
    // The genome decodes over the save's own configs, then goes in as `set=`s.
    let mut all: Vec<String> = Vec::new();
    if let Some((text, genes)) = genome {
        let base = Params {
            world: world.config().clone(),
            flora: flora.config().clone().authored(),
            fauna: fauna.config().clone(),
        };
        all = genome_or_expand(&base, text, genes, expand);
    }
    all.extend(sets.iter().cloned());
    let sets = &all[..];
    let loaded = Params::over_loaded(world.config().clone(), flora.config(), fauna.config().clone(), sets)
        .unwrap_or_else(|e| panic!("the parameter set: {e}"));
    if !sets.is_empty() {
        let plants = cubarium_voxel_flora::registry();
        let animals = cubarium_voxel_fauna::registry();
        for s in sets {
            let path = s.split_once('=').map_or(s.as_str(), |(p, _)| p);
            let shown = if path.starts_with("flora.") {
                plants.get(&loaded.flora, path).map(|v| format!("{v} at {} m", world.config().voxel_m))
            } else {
                animals.get(fauna.config(), path).map(|v| format!("was {v}"))
            };
            eprintln!("set {s} ({})", shown.unwrap_or_else(|e| e));
        }
        flora.replace_config(loaded.flora.clone()).unwrap_or_else(|e| panic!("set= flora: {e}"));
        fauna.replace_config(loaded.params.fauna.clone()).unwrap_or_else(|e| panic!("set= fauna: {e}"));
        // The drivers read the animal config when installed: put them back on the new one.
        cubarium_voxel_sim::saved::restore(&mut fauna, &driven_by_policy)
            .unwrap_or_else(|e| panic!("load: {e}"));
    }
    write_params(&loaded.params);
    install_founders_with(&mut fauna, policies).expect("the explicit founder policies validate");
    let sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    (sim, minute)
}

/// The `phases:` line on stderr, once a simulated minute: that minute's milliseconds per
/// tick by phase. Water is the sum of its leaves on the chained tick and the leg's own
/// total on the overlapped one. Measurement only.
#[derive(Default)]
struct PhaseWindow {
    nanos: Vec<u64>,
}

impl PhaseWindow {
    fn report(&mut self, minute: u64, sim: &Sim, tick_ms: f64) {
        use cubarium_voxel::profile::{Phase, nanos};
        let now: Vec<u64> = Phase::ALL.iter().map(|p| nanos(*p)).collect();
        if self.nanos.len() != now.len() {
            self.nanos = vec![0; now.len()];
        }
        let ms = |p: Phase| {
            now[p.index()].saturating_sub(self.nanos[p.index()]) as f64 / TICKS_PER_MIN as f64 / 1e6
        };
        let leaves = [
            Phase::Rain,
            Phase::Evaporate,
            Phase::Infiltrate,
            Phase::Fall,
            Phase::Exchange,
            Phase::Drain,
            Phase::WaterTable,
            Phase::Spring,
            Phase::Outlet,
        ];
        let overlap = sim.config().overlap;
        let water: f64 = if overlap {
            ms(Phase::WaterLeg)
        } else {
            leaves.iter().map(|p| ms(*p)).sum()
        };
        let split = sim.config().overlap_split();
        eprintln!(
            "phases: min={minute} overlap={} split={}:{} tick_ms={tick_ms:.3} begin={:.3} read_copy={:.3} \
             water={water:.3} exchange={:.3} flora={:.3} light={:.3} drink={:.3} grow={:.3} \
             cover={:.3} fauna={:.3} sense={:.3} settle={:.3}",
            u8::from(overlap),
            split.0,
            split.1,
            ms(Phase::Begin),
            ms(Phase::ReadCopy),
            ms(Phase::Exchange),
            ms(Phase::FloraStep),
            ms(Phase::Light),
            ms(Phase::Drink),
            ms(Phase::Grow),
            ms(Phase::Cover),
            ms(Phase::FaunaStep),
            ms(Phase::FaunaSense),
            ms(Phase::Settle),
        );
        // The plant step phase by phase (`Cover` is its prep and its growth together).
        let flora = [
            ("prune", Phase::Prune),
            ("sky", Phase::SkyCache),
            ("cover", Phase::Cover),
            ("drown", Phase::Drown),
            ("light", Phase::Light),
            ("drink", Phase::Drink),
            ("feed", Phase::Feed),
            ("grow", Phase::Grow),
            ("decompose", Phase::Decompose),
            ("seed_bank", Phase::SeedBank),
            ("propagate", Phase::Propagate),
            ("crowns", Phase::Crowns),
        ];
        let mut line = format!(
            "flora: min={minute} overlap={} total={:.3}",
            u8::from(overlap),
            ms(Phase::FloraStep)
        );
        for (name, p) in flora {
            line.push_str(&format!(" {name}={:.3}", ms(p)));
        }
        eprintln!("{line} stands={}", sim.flora().view().stands.len());
        let fauna = [
            ("terrain", Phase::FaunaTerrain),
            ("maintenance", Phase::FaunaMaintenance),
            ("field", Phase::FaunaField),
            ("sense", Phase::FaunaSense),
            ("windows", Phase::FaunaWindows),
            ("observe", Phase::FaunaObserve),
            ("reflex", Phase::FaunaReflex),
            ("remote", Phase::FaunaRemote),
            ("window_read", Phase::FaunaWindowRead),
            ("window_lay", Phase::FaunaWindowLay),
            ("light", Phase::FaunaLight),
            ("nectar", Phase::FaunaNectar),
            ("act", Phase::FaunaAct),
            ("births", Phase::FaunaBirths),
            ("deaths", Phase::FaunaDeaths),
            ("burrs", Phase::FaunaBurrs),
        ];
        let mut line = format!("fauna: min={minute} total={:.3}", ms(Phase::FaunaStep));
        for (name, p) in fauna {
            line.push_str(&format!(" {name}={:.3}", ms(p)));
        }
        eprintln!("{line} animals={}", sim.fauna().view().animals.len());
        self.nanos = now;
    }
}

/// The seed-bank line on stderr, once a simulated hour: how many sites hold a bank (what
/// D5's seed marks draw), recruits so far, and the SeedBank / Propagate phases' cost per
/// tick over the hour just past. Measurement only.
#[derive(Default)]
struct SeedWindow {
    nanos: [u64; 3],
    calls: [u64; 3],
}

impl SeedWindow {
    fn report(&mut self, tick: u64, sim: &Sim) {
        use cubarium_voxel::profile::{Phase, calls, nanos};
        let f = sim.flora().view();
        let sites = f.ground.iter().filter(|g| !g.seeds.is_empty()).count();
        let mut ms = [0.0; 3];
        for (i, p) in [Phase::SeedBank, Phase::Propagate, Phase::FloraStep]
            .into_iter()
            .enumerate()
        {
            let (n, c) = (nanos(p), calls(p));
            let dc = c.saturating_sub(self.calls[i]).max(1);
            ms[i] = n.saturating_sub(self.nanos[i]) as f64 / dc as f64 / 1e6;
            self.nanos[i] = n;
            self.calls[i] = c;
        }
        eprintln!(
            "seedbank: h={:.2} bank_sites={sites} ground={} stands={} establishments={} \
             seed_bank_ms={:.4} propagate_ms={:.4} flora_ms={:.3}",
            tick as f64 / (3600.0 * f64::from(TICK_HZ)),
            f.ground.len(),
            f.stands.len(),
            f.ledger.establishments,
            ms[0],
            ms[1],
            ms[2],
        );
        // Package S's counters: per species, g seeds germinated, c runner/rhizome
        // daughters, l seeds landed in a bank, d seeds died in it, x spore/water seeds
        // that found no ground; and the whole seeds banked now.
        let l = f.ledger;
        let mut modes = String::new();
        for s in Plant::ALL {
            let i = s.index();
            let (g, c, ld, d, x) = (
                l.seeds_germinated[i],
                l.clonal_births[i],
                l.seeds_landed[i],
                l.seeds_died[i],
                l.seeds_lost[i],
            );
            if g + c + ld + d + x > 0 {
                modes.push_str(&format!(" {}:g{g}/c{c}/l{ld}/d{d}/x{x}", s.name()));
            }
        }
        let banked: f64 = f
            .ground
            .iter()
            .flat_map(|g| g.seeds.iter())
            .map(|c| {
                let sc = f.config.species(c.species);
                c.organic / (sc.alive_min / sc.propagule_split[0])
            })
            .sum();
        eprintln!(
            "seedmodes: h={:.2} banked_seeds={banked:.0}{modes}",
            tick as f64 / (3600.0 * f64::from(TICK_HZ))
        );
        // Succession (package SU), measurement only and named here rather than read off
        // config roles so the same line runs on a build before them: the woody species'
        // share of stands and of standing foliage, the ground covers' share, and the
        // ground covers the woody seedlings overtopped.
        let woody = [Plant::Bloomcrown, Plant::Vaulttree, Plant::Lanternberry];
        let cover = [Plant::Springturf, Plant::Velvetpad, Plant::Stonecushion];
        let (mut n, mut nw, mut nc) = (0usize, 0usize, 0usize);
        let (mut p, mut pw, mut pc) = (0.0f64, 0.0f64, 0.0f64);
        for st in f.stands {
            n += 1;
            p += st.foliage;
            if woody.contains(&st.species) {
                nw += 1;
                pw += st.foliage;
            }
            if cover.contains(&st.species) {
                nc += 1;
                pc += st.foliage;
            }
        }
        let share = |a: f64, b: f64| if b > 0.0 { a / b } else { 0.0 };
        eprintln!(
            "succession: h={:.2} stands={n} woody={:.3} woody_foliage={:.3} cover={:.3} \
             cover_foliage={:.3} overtopped={}",
            tick as f64 / (3600.0 * f64::from(TICK_HZ)),
            share(nw as f64, n as f64),
            share(pw, p),
            share(nc as f64, n as f64),
            share(pc, p),
            overtopped(f.ledger),
        );
    }
}

/// Ground covers overtopped so far (package SU's ledger counter).
fn overtopped(l: &cubarium_voxel_flora::FloraLedger) -> u64 {
    l.overtopped.iter().sum()
}

/// The CSV header: `sim_min`, then per plant species `stands_<name>`, then per animal
/// species `animals_<name>` and `body_<name>`, then per founder lineage its count, mean
/// body, standing eggs and gestating bodies, then the ledger counters, then `litter`.
fn print_header() {
    let mut header = String::from("sim_min");
    for s in Plant::ALL {
        header.push_str(&format!(",stands_{}", s.name()));
    }
    for s in Beast::ALL {
        header.push_str(&format!(",animals_{}", s.name()));
        header.push_str(&format!(",body_{}", s.name()));
    }
    for f in Founder::ALL {
        header.push_str(&format!(",founders_{}", f.name()));
        header.push_str(&format!(",body_{}", f.name()));
        // The eggs of that lineage standing in the world and the bodies of it
        // gestating: a lineage with no living body but a clutch on the ground has not
        // gone extinct yet, and the count alone could not say so.
        header.push_str(&format!(",eggs_{}", f.name()));
        header.push_str(&format!(",gestating_{}", f.name()));
    }
    header.push_str(",flora_births,flora_deaths,fauna_births,fauna_deaths,fauna_hatched,litter");
    header.push_str(",stored,atmosphere,rain_events,residual");
    header.push_str(",fixed_in,consumed_organic,standing_foliage");
    for s in Plant::ALL {
        header.push_str(&format!(",donors_{}", s.name()));
    }
    // The water's own reading: the lake's mean surface in voxels (`y + free` of its open
    // surface cells), its volume and visible area, and the in-world stores beside it.
    header.push_str(",lake_level_v,lake_m3,lake_m2,free_m3,pore_m3,aquifer_m3,tick_ms");
    // Package SM: sites holding a seed bank, free sites showing a D5 seed mark (a recent
    // landing or about to sprout), the marked sites that are about to sprout, and every
    // support face in the world, the share's denominator.
    header.push_str(",banked_sites,marked_sites,sprouting_sites,support_sites");
    // Package mobility: deaths by cause per lineage, cumulative.
    for f in Founder::ALL {
        header.push_str(&format!(",starved_{0},drowned_{0}", f.name()));
    }
    // Package mobility's movement counters per lineage, cumulative: walls climbed to the
    // top, cliffs climbed to the foot, founder-ticks on a face and alive, ledge steps up
    // and down.
    for f in Founder::ALL {
        header.push_str(&format!(
            ",ascents_{0},descents_{0},wall_ticks_{0},alive_ticks_{0},ledges_up_{0},ledges_down_{0}",
            f.name()
        ));
    }
    // WX1's weather, cumulative counts since the world began.
    header.push_str(
        ",temp_c,rh,mode,fog,rain_in,drizzles,showers,downpours,forced,gap_blocked,strikes,day_phase,stream_m3_s,moist",
    );
    // WX4: how many of each lineage are asleep at the sample.
    for f in Founder::ALL {
        header.push_str(&format!(",asleep_{}", f.name()));
    }
    // Predation (P1), per lineage: strikes, hits and dodges by the striker's lineage;
    // wound units healed; deaths killed (by any lineage); bodies fed (eating latch off) and
    // wounded now; carrion eaten; eggs laid and hatched — all cumulative but the two
    // "now" counts. Then kills by killer and prey lineage, and the world's carrion
    // stock, carcass records, dormant eggs, eggs died and eggs buried.
    for f in Founder::ALL {
        header.push_str(&format!(
            ",strikes_{0},hits_{0},dodges_{0},healed_{0},killed_{0},fed_{0},wounded_{0},carrion_eaten_{0},eggs_laid_{0},eggs_hatched_{0}",
            f.name()
        ));
    }
    for killer in Founder::ALL {
        for prey in Founder::ALL {
            header.push_str(&format!(",kills_{}_{}", killer.name(), prey.name()));
        }
    }
    header.push_str(",carrion,carcasses,dormant_eggs,eggs_died,eggs_buried");
    // Parasite hatchlings: riders on a host now, then cumulative riders attached,
    // drop-offs at the detach size and riders freed by their host's departure.
    header.push_str(",riders_now,riders_attached,rider_dropoffs,riders_freed");
    // Flowering and fruiting (plants-flowering-vines 2026-10-03): the latticevine's
    // covered faces (on walls, on undersides) and its spurs by phase, the organic matter
    // in its flowers and beads; bloomcrowns holding nectar and lanternberries hanging
    // fruit, and the organic matter in each.
    header.push_str(",vines,vine_faces,vine_wall_faces,vine_under_faces,vine_buds,vine_flowers,vine_fruits");
    header.push_str(",vine_nectar,vine_fruit,bloom_in_nectar,bloom_nectar,lantern_in_fruit,lantern_fruit");
    // The woody contest's readouts (package CONTEST, 2026-10-05): the edible foliage
    // standing (above every layer's grazing floor), then plant deaths by cause for the
    // turf, the pad and the two woody species, cumulative.
    header.push_str(",edible_foliage");
    for s in CONTEST_SPECIES {
        for c in cubarium_voxel_flora::DeathCause::ALL {
            header.push_str(&format!(",died_{}_{}", c.name(), s.name()));
        }
    }
    // The contest itself (`FaceLayer::Under`): stands holding the place beneath now,
    // recruits that came up beside a cover, rises to the top place by crown height, and
    // deaths beneath (starved, and every cause), cumulative.
    for s in CONTEST_SPECIES {
        let n = s.name();
        header.push_str(&format!(",under_{n},recruits_{n},escapes_{n},under_starved_{n},under_died_{n}"));
    }
    // Curing (package DORMANCY, 2026-10-05): the turf cured now, and cures and wakes,
    // cumulative.
    header.push_str(",cured_springturf,cures_springturf,wakes_springturf");
    // Connected patches (MEADOW-3, 2026-10-06): the drained ground's faces now, and per
    // patch plant the largest patch in faces and as a share of the drained ground, and
    // the patches of at least `meadow_patch_min_faces`.
    header.push_str(",drained_faces");
    for (_, n) in PATCH_SPECIES {
        header.push_str(&format!(",{n}_patch_max,{n}_patch_share,{n}_patches"));
    }
    header.push_str(",dry_season,drained_root_water");
    // SCORE-V3: configured floor (not dry/support area), projected wooded floor,
    // second-largest connected vaulttree grove, and direct leaf growth/offtake books.
    header.push_str(",floor_area_m2,voxel_m,forest_floor_faces,forest_patch_second_max");
    header.push_str(",glowcap_lit_share,leaf_produced,leaf_eaten,seed");
    println!("{header}");
}

/// One CSV row: `sim_min`, then per plant species the stand count, then per animal
/// species the animal count and mean body, then per lineage its count, mean body, eggs
/// and gestations, then the ledger counters, then the total litter.
fn print_row(minute: u64, sim: &Sim, tick_ms: f64, patch_min: usize) {
    let f = sim.flora().view();
    let a = sim.fauna().view();

    let mut row = vec![minute.to_string()];
    for s in Plant::ALL {
        let n = f.stands.iter().filter(|st| st.species == s).count();
        row.push(n.to_string());
    }
    for s in Beast::ALL {
        let bodies: Vec<f64> = a
            .animals
            .iter()
            .filter(|an| an.species == s)
            .map(|an| an.body)
            .collect();
        row.push(bodies.len().to_string());
        let mean = if bodies.is_empty() {
            0.0
        } else {
            bodies.iter().sum::<f64>() / bodies.len() as f64
        };
        row.push(format!("{mean:.4}"));
    }
    for founder in Founder::ALL {
        let bodies: Vec<f64> = a
            .animals
            .iter()
            .filter(|an| an.founder == Some(founder))
            .map(|an| an.body)
            .collect();
        row.push(bodies.len().to_string());
        let mean = if bodies.is_empty() {
            0.0
        } else {
            bodies.iter().sum::<f64>() / bodies.len() as f64
        };
        row.push(format!("{mean:.4}"));
        row.push(a.eggs_by_founder(founder).to_string());
        row.push(a.gestating_by_founder(founder).to_string());
    }
    row.push(f.ledger.births.to_string());
    row.push(f.ledger.deaths.to_string());
    row.push(a.ledger.births.to_string());
    row.push(a.ledger.deaths.to_string());
    row.push(a.ledger.hatched.to_string());
    let litter: f64 = f.ground.iter().map(|g| g.litter).sum();
    row.push(format!("{litter:.4}"));
    let w = sim.world().view();
    row.push(format!("{:.4}", w.stored_m3()));
    row.push(format!("{:.4}", w.atmosphere_m3));
    row.push(w.ledger.rain_events.to_string());
    row.push(format!("{:.3e}", w.total_residual()));
    // Production and offtake, cumulative: consumed / fixed is the herbivores' share.
    row.push(format!("{:.4}", f.ledger.fixed_in));
    row.push(format!("{:.4}", f.ledger.consumed_organic_out));
    let foliage: f64 = f.stands.iter().map(|st| st.foliage).sum();
    row.push(format!("{foliage:.4}"));
    for s in Plant::ALL {
        let donor_min = f.config.species(s).donor_min;
        let n = f.stands.iter().filter(|st| st.species == s && st.wood >= donor_min).count();
        row.push(n.to_string());
    }
    let world = sim.world();
    let lake = cubarium_voxel::hydrate::lake(world);
    let level = if lake.surface_cells.is_empty() {
        f64::from(lake.level_y)
    } else {
        lake.surface_cells
            .iter()
            .map(|&i| f64::from(world.config().coords(i).1) + w.free[i])
            .sum::<f64>()
            / lake.surface_cells.len() as f64
    };
    row.push(format!("{level:.3}"));
    row.push(format!("{:.4}", lake.volume_m3));
    row.push(format!("{:.2}", lake.visible_m2));
    row.push(format!("{:.4}", world.pooled_m3()));
    row.push(format!("{:.4}", world.pore_m3()));
    row.push(format!("{:.4}", w.aquifer_m3));
    row.push(format!("{tick_ms:.3}"));
    let banked = f.ground.iter().filter(|g| !g.seeds.is_empty()).count();
    // Showing: a mark on a site with no stand on it (a stand's own cells outrank it).
    let marked = f
        .ground
        .iter()
        .filter(|g| g.seed_mark(f.tick, f.config).is_some() && f.stand_at(g.site).is_none())
        .count();
    let c = world.config();
    let mut supports = 0usize;
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            supports += w.supports_in_column(x, z).len();
        }
    }
    let sprouting = f
        .ground
        .iter()
        .filter(|g| g.sprouting.is_some() && g.seed_mark(f.tick, f.config) == g.sprouting)
        .count();
    row.push(banked.to_string());
    row.push(marked.to_string());
    row.push(sprouting.to_string());
    row.push(supports.to_string());
    for founder in Founder::ALL {
        for cause in [Departure::Starved, Departure::Drowned] {
            row.push(a.ledger.departed_founder(founder, cause).to_string());
        }
    }
    for founder in Founder::ALL {
        let k = founder.index();
        let l = &a.ledger;
        for n in [
            l.wall_ascents_by_founder[k],
            l.wall_descents_by_founder[k],
            l.wall_ticks_by_founder[k],
            l.founder_ticks_by_founder[k],
            l.ledges_up_by_founder[k],
            l.ledges_down_by_founder[k],
        ] {
            row.push(n.to_string());
        }
    }
    let world = sim.world();
    let wx = world.weather();
    let c = wx.counts;
    row.push(format!("{:.3}", wx.temperature_c()));
    row.push(format!("{:.4}", world.rh()));
    row.push(format!("{:?}", wx.mode()));
    row.push(format!("{:.3}", wx.fog()));
    row.push(format!("{:.4}", world.view().ledger.rain_in));
    row.push(c.drizzles.to_string());
    row.push(c.showers.to_string());
    row.push(c.downpours.to_string());
    row.push(c.forced.to_string());
    row.push(c.gap_blocked.iter().sum::<u64>().to_string());
    row.push(c.strikes.to_string());
    row.push(format!("{:.4}", world.day_phase()));
    row.push(format!("{:.6}", wx.stream_m3_s()));
    row.push(format!("{:.4}", world.moisture()));
    for founder in Founder::ALL {
        let asleep = a
            .animals
            .iter()
            .filter(|an| {
                an.founder == Some(founder) && an.state == cubarium_voxel_fauna::State::Asleep
            })
            .count();
        row.push(asleep.to_string());
    }
    let l = a.ledger;
    for founder in Founder::ALL {
        let i = founder.index();
        let killed: u64 = Founder::ALL
            .iter()
            .map(|&by| l.departed_founder(founder, Departure::Killed { by }))
            .sum();
        let of = |p: &dyn Fn(&cubarium_voxel_fauna::Animal) -> bool| {
            a.animals
                .iter()
                .filter(|an| an.founder == Some(founder) && p(an))
                .count()
        };
        row.push(l.strikes_by_founder[i].to_string());
        row.push(l.hits_by_founder[i].to_string());
        row.push(l.dodges_by_founder[i].to_string());
        row.push(format!("{:.4}", l.healed_by_founder[i]));
        row.push(killed.to_string());
        // The minimal gut: fed (the eating latch off) where the satiated rest was.
        row.push(of(&|an| !an.eating).to_string());
        row.push(of(&|an| an.wound > 0.0).to_string());
        row.push(format!("{:.6}", l.eaten_carrion_by_founder[i]));
        row.push(l.eggs_laid_by_founder[i].to_string());
        row.push(l.eggs_hatched_by_founder[i].to_string());
    }
    for killer in Founder::ALL {
        for prey in Founder::ALL {
            row.push(
                l.departed_founder(prey, Departure::Killed { by: killer })
                    .to_string(),
            );
        }
    }
    let carrion: f64 = f.ground.iter().map(|g| g.carrion).sum();
    let carcasses = f.ground.iter().filter(|g| g.carcass.is_some()).count();
    let dormant = a.clutches.iter().filter(|c| c.dormancy.is_some()).count();
    row.push(format!("{carrion:.6}"));
    row.push(carcasses.to_string());
    row.push(dormant.to_string());
    row.push(l.eggs_died.to_string());
    row.push(l.eggs_buried.to_string());
    let riders = a.animals.iter().filter(|b| b.attached.is_some()).count();
    row.push(riders.to_string());
    row.push(l.riders_attached.to_string());
    row.push(l.rider_dropoffs.to_string());
    row.push(l.riders_freed.to_string());
    {
        use cubarium_voxel_flora::SpurPhase;
        let faces = f.cover.faces();
        let phase = |p: SpurPhase| faces.iter().filter(|c| c.spur == p).count();
        let under = faces.iter().filter(|c| c.face.dir.is_underside()).count();
        row.push(f.cover.vines().len().to_string());
        row.push(faces.len().to_string());
        row.push((faces.len() - under).to_string());
        row.push(under.to_string());
        row.push(phase(SpurPhase::Bud).to_string());
        row.push(phase(SpurPhase::Flower).to_string());
        row.push(phase(SpurPhase::Fruit).to_string());
        row.push(format!("{:.6}", faces.iter().map(|c| c.nectar).sum::<f64>()));
        row.push(format!("{:.6}", faces.iter().map(|c| c.fruit).sum::<f64>()));
        let of = |sp: Plant| f.stands.iter().filter(move |s| s.species == sp);
        row.push(of(Plant::Bloomcrown).filter(|s| s.nectar > 0.0).count().to_string());
        row.push(format!("{:.6}", of(Plant::Bloomcrown).map(|s| s.nectar).sum::<f64>()));
        row.push(of(Plant::Lanternberry).filter(|s| s.fruit > 0.0).count().to_string());
        row.push(format!("{:.6}", of(Plant::Lanternberry).map(|s| s.fruit).sum::<f64>()));
    }
    let edible: f64 = f.stands.iter().flat_map(|s| f.layers(s)).map(|l| l.edible()).sum();
    row.push(format!("{edible:.6}"));
    for s in CONTEST_SPECIES {
        for c in cubarium_voxel_flora::DeathCause::ALL {
            row.push(f.ledger.deaths_by_cause[c.index()][s.index()].to_string());
        }
    }
    for s in CONTEST_SPECIES {
        let k = s.index();
        let l = f.ledger;
        let under = f.stands.iter().filter(|st| st.species == s && st.place == cubarium_voxel_flora::FaceLayer::Under).count();
        row.push(under.to_string());
        row.push(l.recruits[k].to_string());
        row.push(l.escapes[k].to_string());
        row.push(l.under_deaths[cubarium_voxel_flora::DeathCause::Starved.index()][k].to_string());
        row.push(l.under_deaths.iter().map(|c| c[k]).sum::<u64>().to_string());
    }
    {
        let k = Plant::Springturf.index();
        row.push(f.stands.iter().filter(|st| st.species == Plant::Springturf && st.cured).count().to_string());
        row.push(f.ledger.cures[k].to_string());
        row.push(f.ledger.cure_wakes[k].to_string());
    }
    {
        use cubarium::voxel::patches;
        let view = sim.world().view();
        let (drained, drained_water) = patches::drained_ground_water(&view, f.config);
        row.push(drained.to_string());
        for (s, _) in PATCH_SPECIES {
            let p = patches::patches(&patches::held_faces(f.stands, s), view.config.width, patch_min);
            row.push(p.largest.to_string());
            row.push(format!("{:.4}", p.largest as f64 / drained.max(1) as f64));
            row.push(p.at_least.to_string());
        }
        // The dry season (DRY-SEASON): whether it holds now, and the drained ground's
        // mean root water on the available-water scale.
        row.push(u8::from(sim.world().in_dry_season()).to_string());
        row.push(format!("{drained_water:.4}"));
    }
    {
        use cubarium::voxel::patches;
        let c = sim.world().config();
        row.push(format!("{:.6}", f64::from(c.width) * f64::from(c.depth) * c.voxel_m.powi(2)));
        row.push(c.voxel_m.to_string());
        let mut held = patches::held_faces(f.stands, Plant::Vaulttree);
        held.sort_unstable();
        held.dedup();
        let floor: std::collections::HashSet<_> = held.iter().map(|s| (s.x, s.z)).collect();
        row.push(floor.len().to_string());
        // Reuse the world's wrap-aware connectivity and the census patch stair rule.
        let faces: Vec<_> = held.iter().map(|s| (s.x, s.y, s.z)).collect();
        let groups = cubarium_voxel::walk::components(&faces, c.width, patches::STEP_MAX);
        let mut sizes = vec![0usize; held.len()];
        for group in groups {
            sizes[group] += 1;
        }
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        row.push(sizes.get(1).copied().unwrap_or(0).to_string());
        // Stand.light is canopy/terrain exposure, without the day/night multiplier:
        // night cannot make an exposed colony qualify as dark habitat.
        let caps: Vec<_> = f.stands.iter().filter(|s| s.species == Plant::Glowcap).collect();
        let lit = caps.iter().filter(|s| s.light > f.config.glowcap.sun_shade_max).count();
        row.push(format!("{:.6}", lit as f64 / caps.len().max(1) as f64));
        let leaves = |book: &[f64; Plant::COUNT]| Plant::ALL.into_iter()
            .filter(|&s| f.config.species(s).trophic == cubarium_voxel_flora::Trophic::Photo)
            .map(|s| book[s.index()]).sum::<f64>();
        row.push(format!("{:.6}", leaves(&f.ledger.foliage_built)));
        row.push(format!("{:.6}", leaves(&f.ledger.foliage_taken)));
    }
    // Read the kept world's seed, including fallback and loaded-world starts.
    row.push(sim.world().config().seed.to_string());
    println!("{}", row.join(","));
}

/// `DEATHLOG=path` / `STANDLOG=path` (env): diagnosis of plant deaths. Every stand that
/// disappears, with its last state; and per minute per species the stands' reserve and
/// foliage against their caps.
struct DeathLog {
    out: Option<std::io::BufWriter<std::fs::File>>,
    stands_out: Option<std::io::BufWriter<std::fs::File>>,
    last: std::collections::HashMap<u64, (cubarium_voxel_flora::Stand, u64)>,
}

impl DeathLog {
    fn new(sim: &Sim) -> DeathLog {
        use std::io::Write;
        let open = |var: &str, header: &str| {
            std::env::var(var).ok().map(|p| {
                let mut f = std::io::BufWriter::new(std::fs::File::create(p).expect("log"));
                writeln!(f, "{header}").unwrap();
                f
            })
        };
        let out = open(
            "DEATHLOG",
            "tick,day_phase,daylight,species,age_s,wood,alive_min,foliage,p_cap,reserve,q_cap,light,moisture,stress",
        );
        let stands_out = open(
            "STANDLOG",
            "minute,day_phase,daylight,species,n,mean_q_over_w,mean_p_over_cap,frac_q_lt_1pct,frac_newborn",
        );
        let mut last = std::collections::HashMap::new();
        for st in sim.flora().view().stands {
            last.insert(st.id, (*st, 0));
        }
        DeathLog { out, stands_out, last }
    }

    fn poll(&mut self, sim: &Sim, tick: u64) {
        use std::io::Write;
        if self.out.is_none() && self.stands_out.is_none() {
            return;
        }
        let f = sim.flora().view();
        let w = sim.world();
        let mut next = std::collections::HashMap::with_capacity(f.stands.len());
        for st in f.stands {
            let born = self.last.get(&st.id).map_or(tick, |(_, b)| *b);
            next.insert(st.id, (*st, born));
        }
        if let Some(out) = self.out.as_mut() {
            for (id, (st, born)) in &self.last {
                if next.contains_key(id) {
                    continue;
                }
                let sc = f.config.species(st.species);
                writeln!(
                    out,
                    "{tick},{:.4},{:.3},{:?},{:.0},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.4},{:.4},{:.4}",
                    w.day_phase(),
                    w.life_daylight(),
                    st.species,
                    (tick - born) as f64 / TICK_HZ as f64,
                    st.wood,
                    sc.alive_min,
                    st.foliage,
                    sc.alpha * st.wood,
                    st.reserve,
                    sc.reserve_cap * st.wood,
                    st.light,
                    st.moisture,
                    st.aeration_stress
                )
                .unwrap();
            }
        }
        if tick % TICKS_PER_MIN == 0
            && let Some(out) = self.stands_out.as_mut()
        {
            for s in Plant::ALL {
                let sc = f.config.species(s);
                let v: Vec<&(cubarium_voxel_flora::Stand, u64)> =
                    next.values().filter(|(st, _)| st.species == s).collect();
                let n = v.len().max(1) as f64;
                let q: f64 = v.iter().map(|(st, _)| st.reserve / st.wood.max(1e-12)).sum::<f64>() / n;
                let p: f64 = v
                    .iter()
                    .map(|(st, _)| st.foliage / (sc.alpha * st.wood).max(1e-12))
                    .sum::<f64>()
                    / n;
                let low = v.iter().filter(|(st, _)| st.reserve < 0.01 * st.wood).count() as f64 / n;
                let young = v
                    .iter()
                    .filter(|(_, b)| tick - b < 600 * TICK_HZ as u64)
                    .count() as f64
                    / n;
                writeln!(
                    out,
                    "{},{:.4},{:.3},{:?},{},{:.4},{:.4},{:.4},{:.4}",
                    tick / TICKS_PER_MIN,
                    w.day_phase(),
                    w.life_daylight(),
                    s,
                    v.len(),
                    q,
                    p,
                    low,
                    young
                )
                .unwrap();
            }
        }
        self.last = next;
    }
}

/// **Lanternjaw recruitment readouts** (`design/handoffs/lanternjaw-recruitment-2026-10-02.md`):
/// observation only, nothing here changes the run. Three kinds of stderr line:
///
/// - `lj_dusk:` at each dusk the fauna layer sees — the dormant eggs' hatch gate, link by
///   link, in the order viable → footfall ≥ threshold → no adult in reach → a host with
///   a free slot in reach → the per-dusk draw → hatched. Each count is of the eggs that
///   passed every link before it; the gates are re-evaluated here from the eggs as they
///   stood before the dusk tick and the bodies as they stand after it (the layer
///   evaluates them in its births phase, between the two), and `hatched` is the ledger's
///   own `riders_attached` over that tick.
/// - `lj_body:` once per lanternjaw when it leaves the world or the run ends: its rider
///   phase (attach tick, the host's lineage, growth per hour on the host, how it let go
///   and at what age), its free phase (strikes begun, the lowest and last reserve as a
///   share of body, the largest body, whether it reached `birth_body`), and its death
///   cause and age.
/// - `lj_chain:` one summary per run: the summed dusk links, riders, free juveniles,
///   adults established, the lineage's strikes, hits, kills, intake and eggs laid over
///   the run, and the first link with zero throughput.
struct LjChain {
    line: f64,
    clock_on: bool,
    last_daylight: Option<f64>,
    dusk_now: bool,
    pre_eggs: Vec<cubarium_voxel_fauna::Clutch>,
    pre_riders: std::collections::HashMap<u64, u32>,
    links: [u64; 6],
    dusks: u64,
    off_dusk_hatches: u64,
    bodies: std::collections::BTreeMap<u64, LjBody>,
    done: Vec<LjBody>,
    start: cubarium_voxel_fauna::FaunaLedger,
    last_deaths: [u64; Departure::COUNT],
}

#[derive(Clone, Copy, Default)]
struct LjBody {
    id: u64,
    born_tick: u64,
    rider: bool,
    host_lineage: Option<Founder>,
    attach_body: f64,
    detach: Option<(&'static str, u64, f64)>,
    strikes: u64,
    striking: bool,
    min_reserve_share: f64,
    last_reserve_share: f64,
    max_body: f64,
    adult: bool,
    death: Option<(&'static str, u64)>,
    last_age: u64,
    last_body: f64,
}

const LJ_LINKS: [&str; 6] = ["viable", "footfall", "unblocked", "host", "drawn", "hatched"];

impl LjChain {
    fn new(sim: &Sim) -> LjChain {
        let fa = sim.fauna().view();
        let cfg = sim.world().view().config;
        let mut lj = LjChain {
            line: fa.config.life.day_line,
            clock_on: cfg.day.mode != cubarium_voxel::weather::DayMode::Off,
            last_daylight: None,
            dusk_now: false,
            pre_eggs: Vec::new(),
            pre_riders: Default::default(),
            links: [0; 6],
            dusks: 0,
            off_dusk_hatches: 0,
            bodies: Default::default(),
            done: Vec::new(),
            start: fa.ledger.clone(),
            last_deaths: fa.ledger.deaths_by_founder_cause[Founder::Lanternjaw.index()],
        };
        lj.track(sim);
        lj
    }

    /// Before a step: the daylight the fauna layer will read this tick (the clock advances
    /// after the fauna leg), so whether this tick is its dusk, and at a dusk the eggs and
    /// the hosts' rider counts as they stand before it.
    fn before(&mut self, sim: &Sim) {
        let daylight = sim.world().life_daylight();
        self.dusk_now = self.clock_on
            && self.last_daylight.is_some_and(|b| b >= self.line)
            && daylight < self.line;
        self.last_daylight = Some(daylight);
        if !self.dusk_now {
            return;
        }
        let fa = sim.fauna().view();
        self.pre_eggs = fa.clutches.iter().filter(|c| c.dormancy.is_some()).copied().collect();
        self.pre_riders.clear();
        for a in fa.animals {
            if let Some(h) = a.attached {
                *self.pre_riders.entry(h.host).or_default() += 1;
            }
        }
    }

    fn after(&mut self, sim: &Sim, attached_before: u64) {
        let fa = sim.fauna().view();
        let hatched = fa.ledger.riders_attached - attached_before;
        if self.dusk_now {
            self.dusk(sim, hatched);
        } else if hatched > 0 {
            self.off_dusk_hatches += hatched;
        }
        self.track(sim);
    }

    fn dusk(&mut self, sim: &Sim, hatched: u64) {
        let fa = sim.fauna().view();
        let wv = sim.world().view();
        let v = wv.config.voxel_m;
        let ring = f64::from(wv.config.width) * v;
        let day_ticks = cubarium_voxel::ticks_at_least_one(wv.config.day.length_min * 60.0);
        let pc = fa.config.predation;
        let tick = fa.tick;
        let mut n = [0u64; 6];
        n[5] = hatched;
        let wrap = |ex: f64, ez: f64, a: &cubarium_voxel_fauna::Animal| {
            let mut dx = (a.pose.x - ex).rem_euclid(ring);
            if dx > 0.5 * ring {
                dx -= ring;
            }
            let dz = a.pose.z - ez;
            (dx * dx + dz * dz).sqrt()
        };
        for c in &self.pre_eggs {
            let Some(mut d) = c.dormancy else { continue };
            let phys = fa.config.founder(c.lineage);
            if tick >= d.viable_until_tick || c.organic < phys.core.body_min {
                continue;
            }
            n[0] += 1;
            let (ex, ez) = ((f64::from(c.site.x) + 0.5) * v, (f64::from(c.site.z) + 0.5) * v);
            let mut nearest_adult: Option<f64> = None;
            for a in fa.animals {
                let Some(f) = a.founder else { continue };
                let dist = wrap(ex, ez, a);
                if f == c.lineage && a.body >= phys.core.birth_body {
                    nearest_adult = Some(nearest_adult.map_or(dist, |m: f64| m.min(dist)));
                }
                if phys.hunter.is_some_and(|h| h.hunts(f))
                    && dist <= pc.footfall_radius_m
                    && cubarium_voxel_fauna::tremor_of(a, dist, pc.tremor_near_m)
                        >= pc.footfall_tremor_min
                {
                    d.hear(a.id, tick);
                }
            }
            let heard = d.footfall(tick, day_ticks);
            if heard < d.threshold {
                continue;
            }
            n[1] += 1;
            if !cubarium_voxel_fauna::egg_hatches(&pc, d.threshold, heard, true, nearest_adult) {
                continue;
            }
            n[2] += 1;
            let host = fa.animals.iter().any(|a| {
                a.founder != Some(c.lineage)
                    && a.attached.is_none()
                    && wrap(ex, ez, a) <= fa.config.parasite_attach_radius_m
                    && self.pre_riders.get(&a.id).copied().unwrap_or(0) < fa.config.parasite_max_per_host
            });
            if !host {
                continue;
            }
            n[3] += 1;
            if cubarium_voxel_fauna::hatch_draw(fa.config.hatch_chance_per_dusk, wv.config.seed, d.serial, tick) {
                n[4] += 1;
            }
        }
        self.dusks += 1;
        for k in 0..6 {
            self.links[k] += n[k];
        }
        eprintln!(
            "lj_dusk: tick={tick} min={:.1} eggs={} viable={} footfall={} unblocked={} host={} drawn={} hatched={}",
            tick as f64 / TICKS_PER_MIN as f64,
            self.pre_eggs.len(),
            n[0], n[1], n[2], n[3], n[4], n[5]
        );
    }

    /// Every tick: each lanternjaw's phase, strikes, reserve and departure.
    fn track(&mut self, sim: &Sim) {
        let fa = sim.fauna().view();
        let lj = Founder::Lanternjaw;
        let birth_body = fa.config.founder(lj).core.birth_body;
        let mut seen = 0usize;
        for a in fa.animals.iter().filter(|a| a.founder == Some(lj)) {
            seen += 1;
            let b = self.bodies.entry(a.id).or_insert_with(|| {
                let host = a.attached.and_then(|h| fa.animal(h.host));
                LjBody {
                    id: a.id,
                    born_tick: fa.tick.saturating_sub(a.age_ticks),
                    rider: a.attached.is_some(),
                    host_lineage: host.and_then(|h| h.founder),
                    attach_body: a.body,
                    min_reserve_share: f64::INFINITY,
                    ..LjBody::default()
                }
            });
            if b.rider && b.detach.is_none() && a.attached.is_none() {
                let line = fa.config.parasite_detach_body_fraction
                    * cubarium_voxel_fauna::effective_config(fa.config, a).body_max;
                let cause = if a.body >= line { "dropoff" } else { "freed" };
                b.detach = Some((cause, a.age_ticks, a.body));
            }
            if a.attached.is_none() {
                let share = a.reserve / a.body.max(1e-12);
                b.min_reserve_share = b.min_reserve_share.min(share);
                b.last_reserve_share = share;
                if a.strike.is_some() && !b.striking {
                    b.strikes += 1;
                }
            }
            b.striking = a.strike.is_some();
            b.max_body = b.max_body.max(a.body);
            b.adult |= a.body >= birth_body;
            b.last_age = a.age_ticks;
            b.last_body = a.body;
        }
        if seen == self.bodies.len() {
            return;
        }
        // Departures this tick: their causes from the lineage's ledger row.
        let row = fa.ledger.deaths_by_founder_cause[lj.index()];
        let mut causes: Vec<&'static str> = Vec::new();
        for (k, cause) in Departure::ALL.iter().enumerate() {
            for _ in self.last_deaths[k]..row[k] {
                causes.push(cause.name());
            }
        }
        self.last_deaths = row;
        let gone: Vec<u64> =
            self.bodies.keys().copied().filter(|id| fa.animal(*id).is_none()).collect();
        for (i, id) in gone.into_iter().enumerate() {
            let mut b = self.bodies.remove(&id).expect("tracked");
            let cause = if causes.len() == 1 || i < causes.len() {
                causes[i.min(causes.len() - 1)]
            } else {
                "unbooked"
            };
            if b.rider && b.detach.is_none() {
                b.detach = Some(("died_riding", b.last_age, b.last_body));
            }
            b.death = Some((cause, b.last_age));
            Self::print(&b, fa.tick);
            self.done.push(b);
        }
    }

    fn print(b: &LjBody, tick: u64) {
        let h = |t: u64| t as f64 / (3600.0 * f64::from(TICK_HZ));
        let (rider, growth) = match (b.rider, b.detach) {
            (false, _) => ("never".to_string(), "-".to_string()),
            (true, Some((cause, age, body))) => (
                format!("{cause}@{:.2}h", h(age)),
                format!("{:.5}", (body - b.attach_body) / h(age).max(1e-9)),
            ),
            (true, None) => (
                format!("riding@{:.2}h", h(b.last_age)),
                format!("{:.5}", (b.last_body - b.attach_body) / h(b.last_age).max(1e-9)),
            ),
        };
        let death = b.death.map_or("alive".to_string(), |(c, age)| format!("{c}@{:.2}h", h(age)));
        let min_r = if b.min_reserve_share.is_finite() {
            format!("{:.3}", b.min_reserve_share)
        } else {
            "-".to_string()
        };
        eprintln!(
            "lj_body: id={} born_min={:.1} host={} rider={} growth_per_h={} attach_body={:.5} \
             strikes={} min_reserve_share={} last_reserve_share={:.3} max_body={:.5} adult={} death={} tick={}",
            b.id,
            b.born_tick as f64 / TICKS_PER_MIN as f64,
            b.host_lineage.map_or("-", |f| f.name()),
            rider,
            growth,
            b.attach_body,
            b.strikes,
            min_r,
            b.last_reserve_share,
            b.max_body,
            b.adult,
            death,
            tick,
        );
    }

    fn summary(&mut self, sim: &Sim) {
        let fa = sim.fauna().view();
        for b in self.bodies.values() {
            Self::print(b, fa.tick);
        }
        let all: Vec<LjBody> = self.done.iter().chain(self.bodies.values()).copied().collect();
        let k = Founder::Lanternjaw.index();
        let (l, s) = (fa.ledger, &self.start);
        let kills: u64 = (0..cubarium_voxel_fauna::Founder::COUNT)
            .map(|f| {
                let i = Departure::Killed { by: Founder::Lanternjaw }.index();
                l.deaths_by_founder_cause[f][i] - s.deaths_by_founder_cause[f][i]
            })
            .sum();
        let riders = all.iter().filter(|b| b.rider).count();
        let detached = all.iter().filter(|b| b.rider && matches!(b.detach, Some((c, ..)) if c != "died_riding")).count();
        let free_juv = all
            .iter()
            .filter(|b| !b.rider || matches!(b.detach, Some((c, ..)) if c != "died_riding"))
            .count();
        let adults = all.iter().filter(|b| b.adult).count();
        let eggs_laid = l.eggs_laid_by_founder[k] - s.eggs_laid_by_founder[k];
        let mut chain: Vec<(&str, u64)> = LJ_LINKS.iter().zip(self.links).map(|(n, c)| (*n, c)).collect();
        chain.push(("detached", detached as u64));
        chain.push(("adult", adults as u64));
        chain.push(("bred", eggs_laid));
        let first = chain.iter().find(|(_, c)| *c == 0).map_or("none", |(n, _)| n);
        eprintln!(
            "lj_chain: dusks={} viable={} footfall={} unblocked={} host={} drawn={} hatched={} \
             off_dusk_hatches={} riders={} detached={} free_bodies={} adults={} strikes={} hits={} \
             kills={} intake={:.5} carrion={:.5} eggs_laid={} eggs_died={} first_zero={}",
            self.dusks,
            self.links[0], self.links[1], self.links[2], self.links[3], self.links[4], self.links[5],
            self.off_dusk_hatches,
            riders,
            detached,
            free_juv,
            adults,
            l.strikes_by_founder[k] - s.strikes_by_founder[k],
            l.hits_by_founder[k] - s.hits_by_founder[k],
            kills,
            l.assimilated_by_founder[k] - s.assimilated_by_founder[k],
            l.eaten_carrion_by_founder[k] - s.eaten_carrion_by_founder[k],
            eggs_laid,
            l.eggs_died - s.eggs_died,
            first,
        );
    }
}
