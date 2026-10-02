//! Founder tally: a short heuristic run, per-lineage bites, births and deaths.
//!
//! ```text
//! cargo run --release -p cubarium --example founder_tally -- config/desktop/terrarium.toml 2000 1
//! ```
//!
//! Founds the TOML's world exactly as `voxel_census config=` does (seed `SEED`, the
//! code-default founding table, heuristics, the chained tick on one thread), steps it
//! `TICKS` ticks, and prints one line per lineage: alive at the end, bodies born or
//! hatched during the run, deaths by cause, bites, bites by food, eggs laid and hatched.

use std::collections::HashSet;

use cubarium::voxel::{LayerConfig, Seeds, found, load_config, params_of};
use cubarium_voxel_fauna::{Departure, Food, Founder};
use cubarium_voxel_sim::{Sim, SimConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .map_or("config/desktop/terrarium.toml", String::as_str);
    let ticks: u64 = args.get(2).map_or(2000, |a| a.parse().expect("TICKS"));
    let seed: u64 = args.get(3).map_or(1, |a| a.parse().expect("SEED"));
    cubarium_voxel::set_thread_override(1);
    let cfg = load_config(std::path::Path::new(path)).expect("config");
    let params = params_of(&cfg, &[]).expect("params");
    let world_cfg = params.world.clone();
    let founded = found(
        &world_cfg,
        &LayerConfig::new(&params, world_cfg.voxel_m),
        founding(&params),
        Seeds::from(seed),
        false,
    );
    eprintln!(
        "seed {} accepted {} founders {:?}",
        founded.seed, founded.accepted, founded.seeded.founders
    );
    println!(
        "founded: bodies {:?} founding_eggs {} litter_tiles {}",
        founded.seeded.founders, founded.seeded.founding_eggs, founded.seeded.litter_tiles
    );
    let mut sim = Sim::ambient(
        founded.world,
        founded.flora,
        founded.fauna,
        SimConfig::default(),
    );
    sim.set_overlap(false);
    let mut seen: HashSet<u64> = sim.fauna().view().animals.iter().map(|a| a.id).collect();
    let mut born = [0u64; Founder::COUNT];
    for _ in 0..ticks {
        sim.step();
        for a in sim.fauna().view().animals {
            if seen.insert(a.id) {
                if let Some(f) = a.founder {
                    born[f.index()] += 1;
                }
            }
        }
    }
    let view = sim.fauna().view();
    let l = view.ledger;
    for f in Founder::ALL {
        let alive = view.animals.iter().filter(|a| a.founder == Some(f)).count();
        let deaths: Vec<String> = Departure::ALL
            .iter()
            .filter_map(|d| {
                let n = l.deaths_by_founder_cause[f.index()][d.index()];
                (n > 0).then(|| format!("{}={n}", d.name()))
            })
            .collect();
        println!(
            "{:15} alive={:3} born={:3} bites={:6} assim={:.6} eggs_laid={} eggs_hatched={} eggs_now={} deaths=[{}]",
            f.name(),
            alive,
            born[f.index()],
            l.bites_by_founder[f.index()],
            l.assimilated_by_founder[f.index()],
            l.eggs_laid_by_founder[f.index()],
            l.eggs_hatched_by_founder[f.index()],
            view.eggs_by_founder(f),
            deaths.join(" "),
        );
    }
    let foods: Vec<String> = Food::ALL
        .iter()
        .map(|f| format!("{}={}", f.name(), l.bites_by_food[f.index()]))
        .collect();
    println!("bites_by_food {}", foods.join(" "));
    println!(
        "births={} deaths={} hatched={}",
        l.births, l.deaths, l.hatched
    );
}

fn founding(params: &cubarium_voxel_sim::params::Params) -> [usize; Founder::COUNT] {
    params.fauna.founder_counts()
}
