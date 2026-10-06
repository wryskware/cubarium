//! **Senses probe** (SENSES-CORE, S1): a short foreground run of a founded world on its
//! founders' heuristics, for the no-outcome-change readout and the tick's cost.
//!
//! ```text
//! cargo run --release -p cubarium --example senses_probe -- seed=1 ticks=2000 threads=8
//! cargo run --release -p cubarium --example senses_probe -- config=config/desktop/terrarium.toml set=PATH=VALUE
//! ```
//!
//! Founds the world (`config=`, default the desktop terrarium) at `seed=`, steps `ticks=`
//! ticks and prints, per lineage: bodies at the start and the end, births (ids that
//! appeared), deaths, bites, strikes made and hits; then the mean milliseconds a tick
//! and the perception readouts (ray cells and pairs a tick, the hunt funnel).

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
    if !cfg.flora.is_empty() {
        file.insert("flora".into(), toml::Value::Table(cfg.flora));
    }
    if !cfg.fauna.is_empty() {
        file.insert("fauna".into(), toml::Value::Table(cfg.fauna));
    }
    let sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect();
    Params::resolve_over(Params { world: cfg.world, ..Params::defaults() }, &Layers { file: Some(file), sets, ..Layers::default() })
        .unwrap_or_else(|e| panic!("the parameter set: {e}"))
}

fn ids_of(fauna: &Fauna, f: Founder) -> Vec<u64> {
    fauna.view().animals.iter().filter(|a| a.founder == Some(f)).map(|a| a.id).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(n) = arg(&args, "threads=").and_then(|s| s.parse().ok()) {
        cubarium_voxel::set_thread_override(n);
    }
    let params = resolve(&args);
    let seed: u64 = arg(&args, "seed=").map_or(1, |s| s.parse().unwrap());
    let ticks: u64 = arg(&args, "ticks=").map_or(2000, |s| s.parse().unwrap());
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
    let start: Vec<Vec<u64>> = Founder::ALL.iter().map(|&f| ids_of(sim.fauna(), f)).collect();
    let mut seen: Vec<std::collections::BTreeSet<u64>> = start.iter().map(|v| v.iter().copied().collect()).collect();
    let mut born = [0u64; Founder::COUNT];
    let mut spent = std::time::Duration::ZERO;
    for _ in 0..ticks {
        let t0 = Instant::now();
        sim.step();
        spent += t0.elapsed();
        for (k, &f) in Founder::ALL.iter().enumerate() {
            for id in ids_of(sim.fauna(), f) {
                if seen[k].insert(id) {
                    born[k] += 1;
                }
            }
        }
    }
    let view = sim.fauna().view();
    let l = view.ledger;
    println!("lineage,start,end,births,deaths,bites,strikes,hits");
    for (k, &f) in Founder::ALL.iter().enumerate() {
        let deaths: u64 = l.deaths_by_founder_cause[f.index()].iter().sum();
        println!(
            "{},{},{},{},{},{},{},{}",
            f.name(),
            start[k].len(),
            ids_of(sim.fauna(), f).len(),
            born[k],
            deaths,
            l.bites_by_founder[f.index()],
            l.strikes_by_founder[f.index()],
            l.hits_by_founder[f.index()],
        );
    }
    println!("ms_tick,{:.3}", spent.as_secs_f64() * 1000.0 / ticks.max(1) as f64);
    println!("pack_chases,{},pack_kills,{}", l.pack_chases, l.pack_chase_kills);
    let t = ticks.max(1) as f64;
    println!(
        "percept,sensings_per_tick,{:.2},pairs_per_tick,{:.2},ray_cells_per_tick,{:.1},bodies_end,{}",
        l.percept_sensings as f64 / t,
        l.percept_pairs as f64 / t,
        l.percept_ray_cells as f64 / t,
        view.animals.len()
    );
    println!(
        "pack,draws_by_sense[sight hearing tremor smell],{:?},chases_by_sense,{:?},targets_lost,{}",
        l.pack_draws_by_sense, l.pack_chases_by_sense, l.pack_targets_lost
    );
    println!("funnel,hunter,prey,{}", cubarium_voxel_fauna::perception::FUNNEL_STAGES.join(","));
    for h in Founder::ALL {
        for p in Founder::ALL {
            let f = l.funnel[h.index()][p.index()];
            if f.iter().any(|&n| n > 0) {
                println!("funnel,{},{},{}", h.name(), p.name(), f.map(|n| n.to_string()).join(","));
            }
        }
    }
}
