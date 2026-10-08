//! Founding probe (brief FOUNDING, 2026-10-08): what each founded Veilhound does in the
//! first sim minutes, to tell an empty-stomach opening from dens founded on prey.
//!
//! ```text
//! cargo run --release -p cubarium --example founding_probe -- config/desktop/terrarium-d72.toml 2400 1 [threads=4] [every=100]
//! ```
//!
//! Founds the TOML's world as `founder_tally` does (heuristics, the code-default founding
//! table), then prints for every Veilhound at t = 0 its stomach fill `S / K`, reserve
//! fraction, latches and the distance to the nearest body of a lineage it hunts; every
//! `every` ticks the same per body plus the pack's strikes; and each prey founder killed
//! with the tick. Ends with kills by prey lineage and founders alive per lineage.

use cubarium::voxel::{LayerConfig, Seeds, found, load_config, params_of};
use cubarium_voxel_fauna::{Animal, Departure, Founder};
use cubarium_voxel_sim::{Sim, SimConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    cubarium_voxel_sim::experiment_args(&args);
    let path = args.get(1).map_or("config/desktop/terrarium-d72.toml", String::as_str);
    let ticks: u64 = args.get(2).map_or(2400, |a| a.parse().expect("TICKS"));
    let seed: u64 = args.get(3).map_or(1, |a| a.parse().expect("SEED"));
    let mut threads = 4usize;
    let mut every = 100u64;
    for a in args.iter().skip(4) {
        if let Some(n) = a.strip_prefix("threads=") {
            threads = n.parse().expect("threads=N");
        } else if let Some(n) = a.strip_prefix("every=") {
            every = n.parse().expect("every=TICKS");
        } else if a.starts_with("water_every=") {
        } else {
            panic!("unknown argument {a}");
        }
    }
    cubarium_voxel::set_thread_override(threads);
    let cfg = load_config(std::path::Path::new(path)).expect("config");
    let params = params_of(&cfg, &[]).expect("params");
    let world_cfg = params.world.clone();
    let counts = params.fauna.founder_counts();
    let founded = found(&world_cfg, &LayerConfig::new(&params, world_cfg.voxel_m), counts, Seeds::from(seed), false);
    println!("founded seed {} bodies {:?}", founded.seed, founded.seeded.founders);
    let mut sim = Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::experiment());
    sim.set_overlap(false);
    let vh = Founder::Chorister;
    {
        let f = sim.fauna();
        for p in f.packs() {
            println!("pack{} den=({},{},{})", p.id, p.den.x, p.den.y, p.den.z);
        }
    }
    report(&sim, 0, true);
    let mut killed_prev = [0u64; Founder::COUNT];
    for tick in 1..=ticks {
        sim.step();
        let l = &sim.fauna().view().ledger;
        for prey in Founder::ALL {
            let k = l.departed_founder(prey, Departure::Killed { by: vh });
            if k > killed_prev[prey.index()] {
                println!("kill tick={tick} t={:.1}s prey={} total={k}", tick as f64 / 20.0, prey.name());
                killed_prev[prey.index()] = k;
            }
        }
        if tick % every == 0 {
            report(&sim, tick, false);
        }
    }
    let f = sim.fauna();
    let l = &f.view().ledger;
    let kills: Vec<String> = Founder::ALL
        .iter()
        .map(|&p| format!("{}={}", p.name(), l.departed_founder(p, Departure::Killed { by: vh })))
        .filter(|s| !s.ends_with("=0"))
        .collect();
    let alive: Vec<String> = Founder::ALL
        .iter()
        .map(|&p| format!("{}={}", p.name(), f.view().animals.iter().filter(|a| a.founder == Some(p)).count()))
        .collect();
    println!("end tick={ticks} strikes={} kills[{}] alive[{}]", l.strikes_by_founder[vh.index()], kills.join(" "), alive.join(" "));
}

fn report(sim: &Sim, tick: u64, with_prey: bool) {
    let f = sim.fauna();
    let cfg = f.config();
    let v = sim.world().config().voxel_m;
    let ring = f64::from(sim.world().config().width) * v;
    let vh = Founder::Chorister;
    let phys = cfg.founder(vh);
    let dist = |a: &Animal, b: &Animal| {
        let dx = (a.pose.x - b.pose.x).rem_euclid(ring);
        let dx = dx.min(ring - dx);
        (dx * dx + (a.pose.z - b.pose.z).powi(2)).sqrt()
    };
    let animals = &f.view().animals;
    let mut line = format!("t={:.0}s strikes={}", tick as f64 / 20.0, f.view().ledger.strikes_by_founder[vh.index()]);
    for a in animals.iter().filter(|a| a.founder == Some(vh)) {
        let k = phys.gut.capacity(a.body);
        let r = phys.core.reserve_of(a.body);
        line.push_str(&format!(
            " | id{} {:?} eat={} em={} S/K={:.2} R={:.2}",
            a.id,
            a.state,
            u8::from(a.eating),
            u8::from(a.emergency),
            if k > 0.0 { a.stomach.organic() / k } else { 0.0 },
            if r > 0.0 { a.reserve / r } else { 0.0 },
        ));
        if with_prey {
            let near = animals
                .iter()
                .filter(|b| b.founder.is_some_and(|p| p != vh && phys.hunter.is_some_and(|h| h.hunts(p))))
                .map(|b| (dist(a, b), b.founder.map_or("-", |p| p.name())))
                .fold((f64::INFINITY, "-"), |m, x| if x.0 < m.0 { x } else { m });
            line.push_str(&format!(" nearest_prey={:.1}m({})", near.0, near.1));
        }
    }
    println!("{line}");
}
