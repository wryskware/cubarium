//! **Light study** (S3 LIGHT AND FLIGHT): a staged run of a founded world on its founders'
//! heuristics, for the S3-vs-main check — bellwings, lanternjaws and capgnawers every 10 sim
//! minutes, the hunt funnel per pair, and (S3 builds) the light counters.
//!
//! ```text
//! cargo run --release -p cubarium --example light_study -- seed=1 hours=6 threads=1 lanternjaws=4 out=DIR [config=...] [set=PATH=VALUE]
//! ```
//!
//! - **Staged**: a checkpoint (`world.bin`, `flora.bin`, `fauna.bin`, `minute`; resume with
//!   `voxel_census load=DIR`) is written to `out/ckpt` every 10 sim minutes.
//! - **Liveness** after the first 5 sim minutes: bodies moved, something ate, the bellwing ate
//!   if it was awake, the fauna ledger closes, and (S3) the bellwing's light or smell steered
//!   it if it was awake. A failure prints `LIVENESS,FAIL,…` and exits 3.
//! - **Rows** (stdout, CSV): `row,minute,…` every 10 minutes; `funnel,…` and `light,…` at the
//!   end; `ms_tick`.
//!
//! Lines marked `S3-ONLY` read the S3 ledger and `S3B-ONLY` the S3b readouts (the time budget,
//! stalls, naive kills); the main build strips them.

use std::path::Path;
use std::time::Instant;

use cubarium_voxel_fauna::{Fauna, Founder};
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::{Sim, SimConfig};

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k))
}

fn resolve(args: &[String]) -> Params {
    let path = arg(args, "config=").unwrap_or("config/desktop/terrarium.toml");
    let cfg = cubarium::voxel::load_config(Path::new(path)).unwrap_or_else(|e| panic!("config {path}: {e}"));
    let mut file = toml::Table::new();
    if !cfg.sim.flora.is_empty() {
        file.insert("flora".into(), toml::Value::Table(cfg.sim.flora));
    }
    if !cfg.sim.fauna.is_empty() {
        file.insert("fauna".into(), toml::Value::Table(cfg.sim.fauna));
    }
    let sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect();
    Params::resolve_over(Params { world: cfg.sim.world, ..Params::defaults() }, &Layers { file: Some(file), sets, ..Layers::default() })
        .unwrap_or_else(|e| panic!("the parameter set: {e}"))
}

fn count(fauna: &Fauna, f: Founder) -> usize {
    fauna.view().animals.iter().filter(|a| a.founder == Some(f)).count()
}

fn save(sim: &Sim, dir: &Path, minute: u64) {
    let (world, flora, fauna) = sim.layers();
    std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("checkpoint dir {}: {e}", dir.display()));
    let put = |name: &str, bytes: Vec<u8>| std::fs::write(dir.join(name), bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
    put("world.bin", world.save());
    put("flora.bin", cubarium_voxel_flora::snapshot::encode(flora));
    put("fauna.bin", fauna.save());
    put("minute", minute.to_string().into_bytes());
}

const BW: Founder = Founder::Bellwing;
const LJ: Founder = Founder::Lanternjaw;
const CAP: Founder = Founder::Capgnawer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(n) = arg(&args, "threads=").and_then(|s| s.parse().ok()) {
        cubarium_voxel::set_thread_override(n);
    }
    let params = resolve(&args);
    let seed: u64 = arg(&args, "seed=").map_or(1, |s| s.parse().unwrap());
    let hours: f64 = arg(&args, "hours=").map_or(6.0, |s| s.parse().unwrap());
    let out = std::path::PathBuf::from(arg(&args, "out=").unwrap_or("light_study_out"));
    let per_min = u64::from(cubarium_voxel_fauna::TICK_HZ) * 60;
    let ticks = (hours * 60.0 * per_min as f64).round() as u64;
    let wc = params.world.clone();
    let founded = cubarium::voxel::found(
        &wc,
        &cubarium::voxel::LayerConfig::new(&params, wc.voxel_m),
        params.fauna.founder_counts(),
        cubarium::voxel::Seeds::from(seed),
        false,
    );
    eprintln!("founded seed {} ({})", founded.seed, if founded.accepted { "accepted" } else { "NOT accepted" });
    let mut sim = Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::experiment());
    if let Some(n) = arg(&args, "lanternjaws=").and_then(|s| s.parse::<usize>().ok()).filter(|&n| n > 0) {
        match sim.with_layers_mut(|w, fl, fa| cubarium_voxel_sim::habitat::introduce_lanternjaws(w, fl, fa, n)) {
            Ok(ids) => eprintln!("introduced {} lanternjaws", ids.len()),
            Err(e) => eprintln!("no lanternjaws introduced: {e}"),
        }
    }
    let start_pos: Vec<(u64, f64, f64)> = sim.fauna().view().animals.iter().map(|a| (a.id, a.pose.x, a.pose.z)).collect();
    println!("row,minute,bellwings,lanternjaws,capgnawers,bw_swallowed,lj_swallowed,cap_swallowed,lj_bw_killed,lj_cap_killed,bw_awake_ticks,lj_awake_ticks,ms_tick");
    let mut spent = std::time::Duration::ZERO;
    let wall = Instant::now();
    let liveness_at = 5 * per_min;
    let row_every = 10 * per_min;
    for t in 1..=ticks {
        let t0 = Instant::now();
        sim.step();
        spent += t0.elapsed();
        if t == liveness_at {
            let f = sim.fauna();
            let v = f.view();
            let l = v.ledger;
            let moved = v
                .animals
                .iter()
                .filter(|a| start_pos.iter().any(|&(id, x, z)| id == a.id && ((a.pose.x - x).powi(2) + (a.pose.z - z).powi(2)).sqrt() > 0.1))
                .count();
            let closes = (v.organic() - l.expected_organic()).abs() <= 1e-9 * v.organic().abs().max(1e-6);
            let bw_awake = l.awake_ticks_by_founder[BW.index()] > 0;
            let bw_ate = !bw_awake || l.bites_by_founder[BW.index()] > 0 || count(f, BW) == 0;
            let mut light_fired = true;
            light_fired = !bw_awake || count(f, BW) == 0 || l.beacon_samples[BW.index()] + l.scent_led_samples[BW.index()] + l.loom_samples[BW.index()] > 0; // S3-ONLY
            let ok = moved > 0 && l.bites > 0 && bw_ate && closes && light_fired;
            println!(
                "LIVENESS,{},moved,{moved},bites,{},bw_awake,{bw_awake},bw_bites,{},closes,{closes},light_fired,{light_fired}",
                if ok { "OK" } else { "FAIL" },
                l.bites,
                l.bites_by_founder[BW.index()]
            );
            if !ok {
                save(&sim, &out.join("ckpt"), t / per_min);
                std::process::exit(3);
            }
        }
        if t % row_every == 0 || t == ticks {
            let f = sim.fauna();
            let l = f.view().ledger;
            let k = cubarium_voxel_fauna::perception::KILLED;
            println!(
                "row,{},{},{},{},{:.6},{:.6},{:.6},{},{},{},{},{:.3}",
                t / per_min,
                count(f, BW),
                count(f, LJ),
                count(f, CAP),
                l.swallowed_by_founder[BW.index()],
                l.swallowed_by_founder[LJ.index()],
                l.swallowed_by_founder[CAP.index()],
                l.funnel[LJ.index()][BW.index()][k],
                l.funnel[LJ.index()][CAP.index()][k],
                l.awake_ticks_by_founder[BW.index()],
                l.awake_ticks_by_founder[LJ.index()],
                spent.as_secs_f64() * 1000.0 / t as f64
            );
            save(&sim, &out.join("ckpt"), t / per_min);
        }
    }
    eprintln!("stepped {ticks} ticks in {:.1} s", wall.elapsed().as_secs_f64());
    let l = sim.fauna().view().ledger;
    println!("ms_tick,{:.3}", spent.as_secs_f64() * 1000.0 / ticks.max(1) as f64);
    println!("alive_ticks,bellwing,{},lanternjaw,{},capgnawer,{}", l.alive_ticks_by_founder[BW.index()], l.alive_ticks_by_founder[LJ.index()], l.alive_ticks_by_founder[CAP.index()]);
    for f in [BW, LJ, CAP] {
        println!("deaths,{},[starved drowned aged removed killed_by(9 lineages)],{:?},births,{}", f.name(), l.deaths_by_founder_cause[f.index()], l.eggs_hatched_by_founder[f.index()]);
    }
    println!("eaten_nectar,{:.6},sips_by_bloom,{:?}", l.eaten_by_food[cubarium_voxel_fauna::Food::Nectar.index()], l.sips_by_bloom);
    println!("funnel,hunter,prey,{}", cubarium_voxel_fauna::perception::FUNNEL_STAGES.join(","));
    for h in [LJ, Founder::Chorister] {
        for p in [BW, CAP] {
            let f = l.funnel[h.index()][p.index()];
            println!("funnel,{},{},{}", h.name(), p.name(), f.map(|n| n.to_string()).join(","));
        }
    }
    for f in [BW, CAP] {
        println!("light,{},visits[flower glowcap lure],{:?},failed,{:?},loom,{},repel,{},beacon,{},scent_led,{}", f.name(), l.beacon_visits[f.index()], l.failed_visits[f.index()], l.loom_samples[f.index()], l.repel_samples[f.index()], l.beacon_samples[f.index()], l.scent_led_samples[f.index()]); // S3-ONLY
    }
    println!("lure_kills,bellwing,{},capgnawer,{}", l.lure_kills[BW.index()], l.lure_kills[CAP.index()]); // S3-ONLY
    println!("budget,bellwing,{}", cubarium_voxel_fauna::light::LightMode::NAMES.iter().zip(l.light_budget[BW.index()]).map(|(n, c)| format!("{n}={c}")).collect::<Vec<_>>().join(",")); // S3B-ONLY
    println!("s3b,bellwing,stalled,{},sipped_visits[flower glowcap lure],{:?},naive_killed_by_lanternjaw,{},naive_killed_by_chorister,{}", l.stalled_visits[BW.index()], l.sipped_visits[BW.index()], l.naive_kills[LJ.index()][BW.index()], l.naive_kills[Founder::Chorister.index()][BW.index()]); // S3B-ONLY
}
