//! **Light probe** (S3b diagnosis): a bellwing time budget from what the bodies show each
//! tick, on a founded world (`seed= config= lanternjaws=`) or a save (`load=DIR`), for
//! `minutes=` sim minutes. Classes, per awake bellwing per tick, first that holds:
//! sip (state Cropping), loom (its light reading's looming), inspect_<kind> (a visit
//! active), approach_<kind> (a still beacon held, flying, eating latch on), fed/perched/
//! dropping (latch off, perched or falling), other (search, smell, its fan). The ledger's
//! own counters (deltas) are printed beside. Looming sources are recomputed: lineage and
//! whether the body closes on the bellwing.

use std::path::Path;

use cubarium_voxel_fauna::{Fauna, Founder, MobilityMode, State};
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

const BW: Founder = Founder::Bellwing;
const CLASSES: [&str; 12] = [
    "sip", "loom", "inspect_flower", "inspect_glowcap", "inspect_lure", "approach_flower", "approach_glowcap", "approach_lure", "fed_perched_dropping", "fan_heading_to_bloom", "other_search_or_smell", "x",
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(n) = arg(&args, "threads=").and_then(|s| s.parse().ok()) {
        cubarium_voxel::set_thread_override(n);
    }
    let minutes: f64 = arg(&args, "minutes=").map_or(10.0, |s| s.parse().unwrap());
    let every_s: u64 = arg(&args, "trace_s=").map_or(0, |s| s.parse().unwrap());
    let mut sim = if let Some(dir) = arg(&args, "load=") {
        let s = cubarium_voxel_sim::saved::load(Path::new(dir), &[]).unwrap_or_else(|e| panic!("load: {e}"));
        eprintln!("loaded {dir} at minute {}", s.minute);
        Sim::ambient(s.world, s.flora, s.fauna, SimConfig::experiment())
    } else {
        let params = resolve(&args);
        let seed: u64 = arg(&args, "seed=").map_or(1, |s| s.parse().unwrap());
        let wc = params.world.clone();
        let founded = cubarium::voxel::found(
            &wc,
            &cubarium::voxel::LayerConfig::new(&params, wc.voxel_m),
            params.fauna.founder_counts(),
            cubarium::voxel::Seeds::from(seed),
            false,
        );
        let mut sim = Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::experiment());
        if let Some(n) = arg(&args, "lanternjaws=").and_then(|s| s.parse::<usize>().ok()).filter(|&n| n > 0) {
            let _ = sim.with_layers_mut(|w, fl, fa| cubarium_voxel_sim::habitat::introduce_lanternjaws(w, fl, fa, n));
        }
        if let Some(dir) = arg(&args, "save=") {
            let (w, fl, fa) = sim.layers();
            cubarium_voxel_sim::saved::save(Path::new(dir), w, fl, fa, 0).unwrap_or_else(|e| panic!("save: {e}"));
            eprintln!("saved the founding to {dir}");
        }
        sim
    };
    let per_s = u64::from(cubarium_voxel_fauna::TICK_HZ);
    let ticks = (minutes * 60.0 * per_s as f64).round() as u64;
    let l0 = sim.fauna().view().ledger.clone();
    let mut class = [0u64; CLASSES.len()];
    let mut loom_src = [[0u64; 2]; Founder::COUNT];
    let mut bw_ticks = 0u64;
    let mut awake = 0u64;
    let loom_m = sim.fauna().view().config.founder(BW).senses.loom_m;
    for t in 1..=ticks {
        sim.step();
        let (world, flora, f) = sim.layers();
        let v = f.view();
        let ring = f64::from(world.config().width) * world.config().voxel_m;
        for a in v.animals.iter().filter(|a| a.founder == Some(BW)) {
            bw_ticks += 1;
            if a.state == State::Asleep || a.attached.is_some() {
                continue;
            }
            awake += 1;
            let p = f.percept(a.id).copied().unwrap_or_default();
            let held = a.founder_state.held;
            let k = if a.state == State::Cropping {
                0
            } else if p.light.looming > 0.0 {
                // The source: nearest moving body of another lineage within loom_m.
                let mut best: Option<(f64, Founder, bool)> = None;
                for o in v.animals.iter() {
                    if o.id == a.id || o.founder == Some(BW) || o.attached.is_some() || !cubarium_voxel_fauna::moved_recently(&v.config.predation, o) {
                        continue;
                    }
                    let (dx, dz) = wrapped_delta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
                    let dy = o.pose.y_m - a.pose.y_m;
                    let d = (dx * dx + dy * dy + dz * dz).sqrt();
                    if d <= loom_m + 0.5 && best.is_none_or(|(n, _, _)| d < n) {
                        // Closing: the other's ground velocity toward the bellwing minus the
                        // bellwing's own away from it.
                        let (ox, oz) = (o.speed_m_s * o.pose.heading_rad.sin(), o.speed_m_s * o.pose.heading_rad.cos());
                        let (ax, az) = (a.speed_m_s * a.pose.heading_rad.sin(), a.speed_m_s * a.pose.heading_rad.cos());
                        let closing = -((ox - ax) * dx + (oz - az) * dz) / (dx * dx + dz * dz).sqrt().max(1e-9) > 0.05;
                        best = Some((d, o.founder.unwrap_or(BW), closing));
                    }
                }
                if let Some((_, src, closing)) = best {
                    loom_src[src.index()][usize::from(closing)] += 1;
                }
                1
            } else if p.visit.active {
                2 + usize::from(p.visit.kind).min(2)
            } else if !a.eating || a.mobility.mode == MobilityMode::Perched || a.mobility.falling {
                8
            } else if p.light.beacon && a.mobility.mode == MobilityMode::Fly {
                5 + usize::from(p.light.beacon_kind).min(2)
            } else if held.feed > 0.5 {
                9
            } else {
                10
            };
            class[k] += 1;
        }
        if every_s > 0 && t % (every_s * per_s) == 0 {
            let fv = flora.view();
            let blooms: Vec<(f64, f64)> = fv
                .stands
                .iter()
                .filter(|s| s.nectar > 0.0)
                .map(|s| ((f64::from(s.site.x) + 0.5) * world.config().voxel_m, (f64::from(s.site.z) + 0.5) * world.config().voxel_m))
                .collect();
            for a in v.animals.iter().filter(|a| a.founder == Some(BW)) {
                let p = f.percept(a.id).copied().unwrap_or_default();
                let near = blooms
                    .iter()
                    .map(|&(x, z)| {
                        let (dx, dz) = wrapped_delta(a.pose.x, a.pose.z, x, z, ring);
                        (dx * dx + dz * dz).sqrt()
                    })
                    .fold(f64::INFINITY, f64::min);
                println!(
                    "trace,t_s,{},id,{},state,{:?},mode,{:?},eating,{},falling,{},x,{:.2},z,{:.2},y,{:.2},held,{:.2}/{:.2}/{:.2},lift,{:.2},beacon,{},kind,{},level,{:.3},bx,{:.2},bz,{:.2},by,{:.2},looming,{:.2},visit,{},marks,{},nearest_bloom_m,{:.2},n_blooms,{},reserve,{:.4}",
                    t / per_s,
                    a.id,
                    a.state,
                    a.mobility.mode,
                    a.eating,
                    a.mobility.falling,
                    a.pose.x,
                    a.pose.z,
                    a.pose.y_m,
                    a.founder_state.held.forward,
                    a.founder_state.held.turn,
                    a.founder_state.held.feed,
                    a.founder_state.lift,
                    p.light.beacon,
                    p.light.beacon_kind,
                    p.light.beacon_level,
                    p.light.beacon_x,
                    p.light.beacon_z,
                    p.light.beacon_y,
                    p.light.looming,
                    p.visit.active,
                    p.marks.iter().filter(|m| m.until_tick > v.tick).count(),
                    near,
                    blooms.len(),
                    a.reserve
                );
            }
        }
    }
    let l = sim.fauna().view().ledger.clone();
    let b = BW.index();
    println!("bellwing_ticks,{bw_ticks},awake,{awake}");
    for (k, n) in class.iter().enumerate().take(CLASSES.len() - 1) {
        println!("budget,{},{},{:.4}", CLASSES[k], n, *n as f64 / awake.max(1) as f64);
    }
    for f in Founder::ALL {
        let [far, close] = loom_src[f.index()];
        if far + close > 0 {
            println!("loom_source,{},not_closing,{far},closing,{close}", f.name());
        }
    }
    let d = |x: u64, y: u64| x - y;
    println!(
        "ledger_delta,bites,{},loom,{},beacon,{},scent_led,{},repel,{},visits,{:?},failed,{:?}",
        d(l.bites_by_founder[b], l0.bites_by_founder[b]),
        d(l.loom_samples[b], l0.loom_samples[b]),
        d(l.beacon_samples[b], l0.beacon_samples[b]),
        d(l.scent_led_samples[b], l0.scent_led_samples[b]),
        d(l.repel_samples[b], l0.repel_samples[b]),
        [0, 1, 2].map(|k| l.beacon_visits[b][k] - l0.beacon_visits[b][k]),
        [0, 1, 2].map(|k| l.failed_visits[b][k] - l0.failed_visits[b][k]),
    );
    println!("swallowed_bw,{:.6}", l.swallowed_by_founder[b] - l0.swallowed_by_founder[b]);
    // S3b: the steer's own budget (exact), stalls and sipped visits.
    let names = cubarium_voxel_fauna::light::LightMode::NAMES;
    let total: u64 = (0..names.len()).map(|k| l.light_budget[b][k] - l0.light_budget[b][k]).sum();
    for (k, n) in names.iter().enumerate() {
        let c = l.light_budget[b][k] - l0.light_budget[b][k];
        println!("steer_budget,{n},{c},{:.4}", c as f64 / total.max(1) as f64);
    }
    println!("stalled,{},sipped_visits,{:?}", l.stalled_visits[b] - l0.stalled_visits[b], [0, 1, 2].map(|k| l.sipped_visits[b][k] - l0.sipped_visits[b][k]));
    let _ = count_unused;
}

/// The shortest ring offset from (x0, z0) to (x1, z1): x wraps on a ring `ring` metres round.
fn wrapped_delta(x0: f64, z0: f64, x1: f64, z1: f64, ring: f64) -> (f64, f64) {
    let mut dx = (x1 - x0).rem_euclid(ring);
    if dx > 0.5 * ring {
        dx -= ring;
    }
    (dx, z1 - z0)
}

fn count_unused(fauna: &Fauna) -> usize {
    fauna.view().animals.len()
}
