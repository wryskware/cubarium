//! **The water census**: how much water state the voxel solver processes, and where a tick's
//! water milliseconds go, on the world `cubarium voxel` builds. Measurement only
//! (`design/handoffs/voxel-water-algorithms-2026-09-22.md`, "Proposed next work").
//!
//! ```text
//! taskset -c 0-5 cargo run --release -p cubarium --example water_census -- \
//!     [config=config/desktop/voxel.toml] [seed=14400042426867678818] [warmup=1000] \
//!     [quiet=2400] [shower=1200] [pulse=3.36e-4] [budget=118] [every=4]
//! ```
//!
//! Founds the world `cubarium voxel --config <config> --seed <seed>` founds — the host's
//! founding loop (lake gate, pre-roll with the opening shower, seeding, acceptance), the
//! founders on the built-in trained centres, the senses settled, the outlet open — and
//! steps the **live schedule** (`Sim::step`) headless, as fast as it goes, with the thread
//! count the host picks. `warmup` ticks are stepped unmeasured, then two measured windows
//! run back to back, each capped at `budget` wall seconds: `quiet` ticks as the world
//! runs, then `shower` ticks with a `RainPulse` of `pulse` m³ before every tick — the
//! forced shower. The default pulse is `default`'s shower rate on its 192 m² of sky
//! (3.5e-5 m/s × 0.05 s × 192 m²), and 1,200 ticks of it is one shower's 0.4 m³. The
//! founding loop is minutes of work on the desktop world, so both windows share one.
//!
//! Every `every`th window tick is a **census tick** (`cubarium_voxel::profile::census`):
//! it snapshots the wet set around each water phase, so its milliseconds are left out of
//! the timings, which come from the other ticks. The work counts come from every tick.

use std::time::Instant;

use cubarium::voxel::{self, habitat, install_default_founders};
use cubarium_voxel::profile::{self, Count, Phase, census};
use cubarium_voxel::{Command as WorldCommand, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig, Senses};
use cubarium_voxel_flora::{Flora, FloraConfig};
use cubarium_voxel_sim::{Sim, SimConfig};

/// The water leaves the host's `world` column sums (`voxel/mod.rs`, `WATER_PHASES`).
const WATER: [Phase; 9] = [
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

fn arg<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("{name}=")).map(str::to_string))
        .map_or(default, |v| {
            v.parse()
                .unwrap_or_else(|_| panic!("{name}={v} does not parse"))
        })
}

fn nanos() -> [u64; Phase::COUNT] {
    Phase::ALL.map(profile::nanos)
}

fn main() {
    let started = Instant::now();
    let config: String = arg("config", "config/desktop/voxel.toml".to_string());
    let seed: u64 = arg("seed", 14400042426867678818);
    let warmup: u64 = arg("warmup", 1000);
    let quiet: u64 = arg("quiet", 2400);
    let shower: u64 = arg("shower", 1200);
    let pulse: f64 = arg("pulse", 3.5e-5 * cubarium_voxel::DT * 192.0);
    let budget: f64 = arg("budget", 118.0);
    let every: u64 = arg::<u64>("every", 4).max(1);

    let cfg = voxel::load_config(std::path::Path::new(&config)).expect("the voxel config");
    let mut next = seed.wrapping_add(1);
    let founded = voxel::found_a_habitat(
        &cfg.world,
        Some(seed),
        voxel::LAKE_SEED_TRIES,
        voxel::HABITAT_TRIES,
        move || {
            next = next.wrapping_add(1);
            next
        },
        |w: &World| {
            (
                Flora::new(FloraConfig::for_voxel_size(w.config().voxel_m)),
                Fauna::new(FaunaConfig::default()),
            )
        },
        habitat::FOUNDER_COUNTS,
        |s: &habitat::Seeded| s.acceptance.accepted,
    );
    let (world, flora, mut fauna) = (founded.world, founded.flora, founded.fauna);
    install_default_founders(&mut fauna).expect("the built-in centres validate");
    let mut senses = Senses::new();
    senses.settle(&world.view(), &flora.view());
    let threads = cubarium_voxel::default_threads();
    let mut sim = Sim::new(world, flora, fauna, SimConfig::with_threads(threads), Some(senses));
    if sim.world().config().closed_water_budget && !sim.world().outlet_open() {
        sim.world_mut()
            .apply(WorldCommand::SetOutlet { open: true });
    }
    let c = sim.world().config().clone();
    println!(
        "world: {config}, {}x{}x{} at {} m, seed {}, {}; {threads} sim threads; \
         founded in {:.1} s",
        c.width,
        c.height,
        c.depth,
        c.voxel_m,
        founded.seed,
        if founded.accepted {
            "accepted"
        } else {
            "NOT accepted"
        },
        started.elapsed().as_secs_f64(),
    );

    let t = Instant::now();
    for _ in 0..warmup {
        sim.step();
    }
    println!(
        "warm-up: {warmup} ticks in {:.1} s, world tick {}, {} showers so far, next due at \
         tick {}",
        t.elapsed().as_secs_f64(),
        sim.world().tick(),
        sim.world().view().ledger.showers,
        sim.world().next_shower_tick(),
    );
    if quiet > 0 {
        window(&mut sim, "quiet", quiet, 0.0, every, budget);
    }
    if shower > 0 {
        window(&mut sim, "forced shower", shower, pulse, every, budget);
    }
}

/// One measured window: `ticks` ticks or `budget` wall seconds, a `pulse` before each.
fn window(sim: &mut Sim, name: &str, ticks: u64, pulse: f64, every: u64, budget: f64) {
    profile::reset();
    census::reset();
    let mut acc = [0u64; Phase::COUNT];
    let (mut timed, mut step_ns, mut step_max, mut n) = (0u64, 0u64, 0u64, 0u64);
    let window = Instant::now();
    let from = sim.world().tick();
    while n < ticks && window.elapsed().as_secs_f64() < budget {
        let is_census = n % every == 0;
        if is_census {
            census::begin_tick(sim.world());
        }
        let before = nanos();
        let at = Instant::now();
        if pulse > 0.0 {
            sim.world_mut()
                .apply(WorldCommand::RainPulse { volume_m3: pulse });
        }
        sim.step();
        let ns = at.elapsed().as_nanos() as u64;
        if is_census {
            census::end_tick(sim.world());
        } else {
            let after = nanos();
            for k in 0..Phase::COUNT {
                acc[k] += after[k] - before[k];
            }
            timed += 1;
            step_ns += ns;
            step_max = step_max.max(ns);
        }
        n += 1;
        if n % 300 == 0 {
            eprintln!(
                "  {name} tick {n}: {:.1} s, {} wet, raining {}",
                window.elapsed().as_secs_f64(),
                sim.world().wet_cells(),
                sim.world().view().is_raining(),
            );
        }
    }
    let wall = window.elapsed().as_secs_f64();
    let ms = |ns: u64| ns as f64 / 1e6 / timed.max(1) as f64;
    let water: u64 = WATER.iter().map(|p| acc[p.index()]).sum();
    println!(
        "\n## {name}: world ticks {from}..{}, {n} ticks in {wall:.1} s ({:.1} ticks/s), pulse \
         {pulse:e} m³/tick, {} showers by the end; timings over the {timed} non-census ticks",
        sim.world().tick(),
        n as f64 / wall,
        sim.world().view().ledger.showers,
    );
    println!(
        "at the close: {} wet cells, {:.4} m³ free (pooled), {:.4} m³ pore",
        sim.world().wet_cells(),
        sim.world().pooled_m3(),
        sim.world().pore_m3(),
    );
    println!(
        "step {:.2} ms/tick (max {:.1}); water {:.2} ({:.0} %), flora {:.2}, fauna {:.2}",
        ms(step_ns),
        step_max as f64 / 1e6,
        ms(water),
        100.0 * water as f64 / step_ns.max(1) as f64,
        ms(acc[Phase::FloraStep.index()]),
        ms(acc[Phase::FaunaStep.index()]),
    );
    println!("\n| phase | ms/tick | calls/tick |\n| --- | --- | --- |");
    let calls = |p: Phase| profile::calls(p) as f64 / n.max(1) as f64;
    for p in Phase::ALL {
        let v = acc[p.index()];
        if v == 0 || matches!(p, Phase::Census) {
            continue;
        }
        println!("| {} | {:.3} | {:.1} |", p.name(), ms(v), calls(p));
    }
    println!("\n| count | per tick |\n| --- | --- |");
    for k in Count::ALL {
        let v = profile::count(k);
        if v > 0 {
            println!("| {} | {:.1} |", k.name(), v as f64 / n.max(1) as f64);
        }
    }

    let s = census::totals();
    let per = |v: u64| v as f64 / s.ticks.max(1) as f64;
    let share = |v: u64, of: u64| 100.0 * v as f64 / of.max(1) as f64;
    println!(
        "\ncensus ({} ticks, {:.2} ms/census tick of its own): wet {:.0}, full {:.0} ({:.1} %), \
         runs {:.0} (wet/runs {:.2}), columns {:.0} (wet/column {:.2})",
        s.ticks,
        profile::nanos(Phase::Census) as f64 / 1e6 / s.ticks.max(1) as f64,
        per(s.wet),
        per(s.full),
        share(s.full, s.wet),
        per(s.runs),
        s.wet as f64 / s.runs.max(1) as f64,
        per(s.columns),
        s.wet as f64 / s.columns.max(1) as f64,
    );
    println!(
        "exchange substeps ({}): {:.0} active wet, {:.1} % with no nonzero edge, runs {:.0} \
         (wet/runs {:.2}), columns {:.0}, unpacked {:.1} %; at the close {:.0} nearly full",
        s.substeps,
        s.exchange_wet as f64 / s.substeps.max(1) as f64,
        share(s.exchange_no_edge, s.exchange_wet),
        s.exchange_runs as f64 / s.substeps.max(1) as f64,
        s.exchange_wet as f64 / s.exchange_runs.max(1) as f64,
        s.exchange_columns as f64 / s.substeps.max(1) as f64,
        share(s.exchange_unpacked, s.exchange_wet),
        per(s.nearly_full),
    );
    println!(
        "per tick: {:.0} cells changed by any phase, {:.0} net ({:.1} %); wet untouched {:.1} %, \
         evaporation-only {:.1} %, untouched by fall/exchange {:.1} %",
        per(s.touched),
        per(s.net),
        share(s.net, s.touched),
        share(s.untouched_wet, s.wet),
        share(s.evaporation_only, s.wet),
        share(s.flow_quiet, s.wet),
    );
    println!(
        "tiles of {0}x{0} columns: {1:.1} wet; asleep {2:.1} % of tiles / {3:.1} % of wet; \
         ignoring evaporation {4:.1} % / {5:.1} %",
        census::TILE,
        per(s.tiles_wet),
        share(s.tiles_asleep, s.tiles_wet),
        share(s.wet_in_asleep_tiles, s.wet),
        share(s.tiles_asleep_but_evaporation, s.tiles_wet),
        share(s.wet_in_asleep_but_evaporation_tiles, s.wet),
    );
    println!("\n| phase | cells changed/tick | summed over calls/tick |\n| --- | --- | --- |");
    for tag in census::Tag::ALL {
        println!(
            "| {} | {:.0} | {:.0} |",
            tag.name(),
            per(s.by_tag[tag.index()]),
            per(s.per_call[tag.index()]),
        );
    }
    println!(
        "\n| free (cells) | wet cells | share | below this bin's top | exchange edges from \
         givers below its top | of all edges: unpacked givers in this bin | net changes of \
         this size |\n| --- | --- | --- | --- | --- | --- | --- |"
    );
    let edges: u64 = s.edges_by_depth.iter().sum();
    let (mut below, mut edges_below) = (0u64, 0u64);
    for k in 0..census::BINS {
        let label = match k {
            0 => format!("< {:e}", census::EDGES[0]),
            k if k == census::BINS - 1 => "full".to_string(),
            k if k == census::BINS - 2 => format!("[{:e}, full)", census::EDGES[k - 1]),
            k => format!("[{:e}, {:e})", census::EDGES[k - 1], census::EDGES[k]),
        };
        below += s.depth[k];
        edges_below += s.edges_by_depth[k];
        println!(
            "| {label} | {:.0} | {:.2} % | {:.2} % | {:.2} % | {:.2} % | {:.2} % |",
            per(s.depth[k]),
            share(s.depth[k], s.wet),
            share(below, s.wet),
            share(edges_below, edges),
            share(s.edges_by_depth_unpacked[k], edges),
            share(s.net_by_size[k], s.net),
        );
    }
}
