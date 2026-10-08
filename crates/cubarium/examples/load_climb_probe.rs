//! Load-climb probe: does a world round-tripped through the one safe load
//! (`cubarium_voxel_sim::saved`) move like the same world continuing unsaved?
//!
//! ```text
//! cargo run --release -p cubarium --example load_climb_probe -- \
//!     config=config/desktop/terrarium-d72.toml seed=1 at=4 after=3 threads=4
//! ```
//!
//! Founds the arm on the heuristic (or starts from `load=DIR`, a save directory, through the
//! one safe load), steps `at` sim minutes, then copies the three layers
//! through their save bytes and `saved::decode` (twin B) and steps the unsaved world (A)
//! and the twin side by side for `after` minutes. Every `every` sim seconds (default 10)
//! it prints, per founder lineage and twin, how many bodies moved since the last sample,
//! their mean path and their states; `detail=<lineage>` adds one line per body of that
//! lineage. Diagnostic only.

use cubarium_voxel_fauna::{Fauna, Founder, TICK_HZ};
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::{Sim, SimConfig};
use std::collections::HashMap;

fn arg<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    args.iter()
        .find_map(|a| a.strip_prefix(key).and_then(|s| s.parse().ok()))
        .unwrap_or(default)
}

fn lineage(name: &str) -> Option<Founder> {
    Founder::ALL.into_iter().find(|f| f.name() == name)
}

type Last = HashMap<u64, (f64, f64, f64)>;

fn sample(tag: &str, fauna: &Fauna, last: &mut Last, detail: Option<Founder>, t_s: f64) {
    let v = fauna.view();
    let mut rows: Vec<String> = Vec::new();
    for f in Founder::ALL {
        let mut n = 0;
        let mut moved = 0;
        let mut path = 0.0;
        let mut states: HashMap<String, usize> = HashMap::new();
        for a in v.animals.iter().filter(|a| a.founder == Some(f)) {
            n += 1;
            let p = (a.pose.x, a.pose.z, a.pose.y_m);
            let d = last.get(&a.id).map_or(0.0, |q| {
                let (dx, dz, dy) = (p.0 - q.0, p.1 - q.1, p.2 - q.2);
                (dx * dx + dz * dz + dy * dy).sqrt()
            });
            // The ring wrap shows as a jump of a circumference; ignore it.
            let d = if d > 50.0 { 0.0 } else { d };
            if d > 0.05 {
                moved += 1;
            }
            path += d;
            let st = format!("{:?}/{:?}{}", a.state, a.mobility.mode, if a.mobility.climb.is_some() { "+wall" } else { "" });
            *states.entry(st).or_default() += 1;
            if detail == Some(f) {
                eprintln!(
                    "  {tag} t={t_s:.0} id={} {:?}/{:?} climb={:?} perch={:?} falling={} y={:.2} d={:.2} held={:?} steer={:?} still={} rest={:?} eating={} reserve={:.3}",
                    a.id,
                    a.state,
                    a.mobility.mode,
                    a.mobility.climb.map(|c| (c.ascending, c.progress_m, c.rise_m)),
                    a.mobility.perch.map(|p| p.kind),
                    a.mobility.falling,
                    a.pose.y_m,
                    d,
                    (a.founder_state.held.forward, a.founder_state.held.turn, a.founder_state.held.feed),
                    fauna.steer_of(a.id),
                    a.still_ticks,
                    a.rest.is_some(),
                    a.eating,
                    a.reserve,
                );
            }
            last.insert(a.id, p);
        }
        if n > 0 {
            let mut st: Vec<_> = states.into_iter().collect();
            st.sort();
            rows.push(format!("{} {moved}/{n} path {:.1} {:?}", f.name(), path / n as f64, st));
        }
    }
    println!("{tag} t={t_s:.0}s | {}", rows.join(" | "));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    cubarium_voxel_sim::experiment_args(&args);
    let threads: usize = arg(&args, "threads=", 4);
    cubarium_voxel::set_thread_override(threads);
    let seed: u64 = arg(&args, "seed=", 1);
    let at: u64 = arg(&args, "at=", 4);
    let after: u64 = arg(&args, "after=", 3);
    let every: u64 = arg(&args, "every=", 10);
    let detail = args.iter().find_map(|a| a.strip_prefix("detail=")).and_then(lineage);
    let load = args.iter().find_map(|a| a.strip_prefix("load="));
    let (mut a, start_min) = if let Some(dir) = load {
        let s = cubarium_voxel_sim::saved::load(std::path::Path::new(dir), &[]).expect("load");
        (Sim::ambient(s.world, s.flora, s.fauna, SimConfig::experiment()), s.minute)
    } else {
        found_arm(&args, seed)
    };
    run(&mut a, start_min, at, after, every, detail);
}

fn found_arm(args: &[String], seed: u64) -> (Sim, u64) {
    let config: String = arg(args, "config=", "config/desktop/terrarium-d72.toml".to_string());
    let cfg = cubarium::voxel::load_config(std::path::Path::new(&config)).expect("config");
    let mut file = toml::Table::new();
    if !cfg.sim.flora.is_empty() {
        file.insert("flora".into(), toml::Value::Table(cfg.sim.flora));
    }
    if !cfg.sim.fauna.is_empty() {
        file.insert("fauna".into(), toml::Value::Table(cfg.sim.fauna));
    }
    let params = Params::resolve_over(
        Params {
            world: cfg.sim.world,
            ..Params::defaults()
        },
        &Layers {
            file: Some(file),
            ..Layers::default()
        },
    )
    .expect("params");
    let world_cfg = params.world.clone();
    let founded = cubarium::voxel::found(
        &world_cfg,
        &cubarium::voxel::LayerConfig::new(&params, world_cfg.voxel_m),
        params.fauna.founder_counts(),
        cubarium::voxel::Seeds::from(seed),
        false,
    );
    eprintln!("founded seed {} accepted {}", founded.seed, founded.accepted);
    (Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::experiment()), 0)
}

fn run(a: &mut Sim, start_min: u64, at: u64, after: u64, every: u64, detail: Option<Founder>) {
    let per_min = 60 * u64::from(TICK_HZ);
    let mut last_a = Last::new();
    for m in 0..at {
        for _ in 0..per_min {
            a.step();
        }
        sample("A", a.fauna(), &mut last_a, None, ((start_min + m + 1) * 60) as f64);
    }
    // The round trip: the bytes a save writes, read back by the one safe load.
    let (w, fl, fa) = a.layers();
    let (w, fl, fa) = (w.save(), cubarium_voxel_flora::snapshot::encode(fl), fa.save());
    let (w, fl, fa) = cubarium_voxel_sim::saved::decode(&w, &fl, &fa, &[]).expect("decode");
    let mut b = Sim::ambient(w, fl, fa, SimConfig::experiment());
    let mut last_b = last_a.clone();
    let t0 = ((start_min + at) * 60) as f64;
    sample("A", a.fauna(), &mut last_a, detail, t0);
    sample("B", b.fauna(), &mut last_b, detail, t0);
    let ticks = after * per_min;
    let step = every * u64::from(TICK_HZ);
    let mut k = 0;
    while k < ticks {
        for _ in 0..step {
            a.step();
            b.step();
        }
        k += step;
        let t = t0 + (k / u64::from(TICK_HZ)) as f64;
        sample("A", a.fauna(), &mut last_a, detail, t);
        sample("B", b.fauna(), &mut last_b, detail, t);
    }
}
