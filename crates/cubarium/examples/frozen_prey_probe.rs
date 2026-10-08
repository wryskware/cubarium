//! Frozen-prey probe: why a littershredder is held still by its flee layer, and why a
//! loftstrider stands `Walking` without moving.
//!
//! ```text
//! cargo run --release -p cubarium --example frozen_prey_probe -- \
//!     config=config/desktop/terrarium-d72.toml seed=1 minutes=5 every=30 threads=4
//! ```
//!
//! Founds the arm (or `load=DIR`), steps `minutes` sim minutes and every `every` sim seconds
//! prints, for the littershredders, how many each layer holds ([`Fauna::steer_of`]), how many
//! are frozen, and for the frozen ones the footfall they feel split by the feeling body's
//! lineage (kin, other prey, hunters of it); for the loftstriders, one line per body that
//! has stood still for a minute or more. Diagnostic only.

use cubarium_voxel_fauna::{Fauna, Founder, TICK_HZ};
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::{Sim, SimConfig};
use std::collections::BTreeMap;

fn arg<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    args.iter()
        .find_map(|a| a.strip_prefix(key).and_then(|s| s.parse().ok()))
        .unwrap_or(default)
}

fn shredders(fauna: &Fauna, t_s: f64, ring: f64) {
    let v = fauna.view();
    let hunters: Vec<Founder> = Founder::ALL
        .into_iter()
        .filter(|&f| v.config.founder(f).active_hunter().is_some_and(|h| h.hunts(Founder::Blind)))
        .collect();
    let mut steer: BTreeMap<String, usize> = BTreeMap::new();
    let (mut n, mut frozen) = (0, 0);
    let mut by_src: BTreeMap<&'static str, (f64, usize)> = BTreeMap::new();
    let mut held_by: BTreeMap<String, usize> = BTreeMap::new();
    for a in v.animals.iter().filter(|a| a.founder == Some(Founder::Blind)) {
        n += 1;
        let s = fauna.steer_of(a.id).map_or("none".to_string(), |s| format!("{s:?}"));
        *steer.entry(s.clone()).or_default() += 1;
        let Some(p) = fauna.percept(a.id) else { continue };
        if p.vigil.freeze_until <= v.tick || s != "Flee" {
            continue;
        }
        frozen += 1;
        // Who it feels (the live tremor reading's rule: grounded moving bodies within its
        // tremor range), by lineage.
        let tremor_m = v.config.founder(Founder::Blind).senses.tremor_m;
        let mut top: (f64, String) = (0.0, String::new());
        for o in v.animals.iter().filter(|o| o.id != a.id && o.speed_m_s > 0.0 && o.attached.is_none()) {
            if o.mobility.mode != cubarium_voxel_fauna::MobilityMode::Ground {
                continue;
            }
            let mut dx = (o.pose.x - a.pose.x).rem_euclid(ring);
            if dx > 0.5 * ring {
                dx -= ring;
            }
            let dz = o.pose.z - a.pose.z;
            let d = (dx * dx + dz * dz).sqrt();
            if d > tremor_m {
                continue;
            }
            let t = cubarium_voxel_fauna::tremor_of(o, d, v.config.predation.tremor_near_m);
            let f = o.founder.unwrap_or(Founder::Blind);
            let src = if f == Founder::Blind {
                "kin"
            } else if hunters.contains(&f) {
                "hunter"
            } else {
                "other"
            };
            let e = by_src.entry(src).or_default();
            e.0 += t;
            e.1 += 1;
            if t > top.0 {
                top = (t, format!("{}#{} d={d:.2} v={:.3} m={:.3}", f.name(), o.id, o.speed_m_s, o.organic()));
            }
        }
        *held_by.entry(if top.1.is_empty() { "nobody".into() } else { top.1.split('#').next().unwrap().to_string() }).or_default() += 1;
        if frozen <= 3 {
            println!(
                "  t={t_s:.0} shredder#{} still={} felt={:.4} turn={} top={:.4} {}",
                a.id,
                a.still_ticks,
                p.tremor[0] + p.tremor[1],
                p.vigil.withdraw_turn,
                top.0,
                top.1
            );
        }
    }
    println!(
        "t={t_s:.0}s shredders {n} frozen {frozen} steer {steer:?} felt_by {:?} top_feeler {held_by:?} freezes {}",
        by_src.iter().map(|(k, (t, c))| format!("{k}:{c}/{t:.3}")).collect::<Vec<_>>(),
        v.ledger.vigil.freezes[Founder::Blind.index()]
    );
}

fn loftstriders(fauna: &Fauna, t_s: f64) {
    let v = fauna.view();
    for a in v.animals.iter().filter(|a| a.founder == Some(Founder::Loftstrider)) {
        if a.still_ticks < 60 * u64::from(TICK_HZ) {
            continue;
        }
        println!(
            "  t={t_s:.0} loftstrider#{} {:?}/{:?} still={} held={:?} steer={:?} eating={} reserve={:.3} rest={} press={:?}",
            a.id,
            a.state,
            a.mobility.mode,
            a.still_ticks,
            (a.founder_state.held.forward, a.founder_state.held.turn, a.founder_state.held.feed),
            fauna.steer_of(a.id),
            a.eating,
            a.reserve,
            a.rest.is_some(),
            fauna.press_of(a.id),
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    cubarium_voxel_sim::experiment_args(&args);
    let threads: usize = arg(&args, "threads=", 4);
    cubarium_voxel::set_thread_override(threads);
    let seed: u64 = arg(&args, "seed=", 1);
    let minutes: u64 = arg(&args, "minutes=", 5);
    let every: u64 = arg(&args, "every=", 30);
    let save = args.iter().find_map(|a| a.strip_prefix("save="));
    let load = args.iter().find_map(|a| a.strip_prefix("load="));
    let (mut sim, start_min) = if let Some(dir) = load {
        let s = cubarium_voxel_sim::saved::load(std::path::Path::new(dir), &[]).expect("load");
        (Sim::ambient(s.world, s.flora, s.fauna, SimConfig::experiment()), s.minute)
    } else {
        found_arm(&args, seed)
    };
    let ring = {
        let (w, _, _) = sim.layers();
        let c = w.config();
        f64::from(c.width) * c.voxel_m
    };
    // `trace=ID[,ID..]` and `trace_ticks=N`: one line a tick for those bodies, after the minutes.
    let trace: Vec<u64> = args
        .iter()
        .find_map(|a| a.strip_prefix("trace="))
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default();
    let trace_ticks: u64 = arg(&args, "trace_ticks=", 100);
    // `terrain=ID`: the support faces around that body (highest support at or below its
    // layer + 8, per voxel column), its footprint and step limits.
    if let Some(id) = args.iter().find_map(|a| a.strip_prefix("terrain=")).and_then(|s| s.parse::<u64>().ok()) {
        let (w, _, fa) = sim.layers();
        let view = w.view();
        let c = w.config();
        let a = *fa.view().animal(id).expect("alive");
        let phys = fa.config().founder(a.founder.unwrap());
        let b = phys.body_at(a.body);
        let st = phys.step_limits(c.voxel_m);
        println!(
            "body {id} at ({:.2},{:.2}) site {:?} r={:.3} height={:.2} headroom={} step up {} down {} voxel {} size {}x{}x{}",
            a.pose.x, a.pose.z, (a.site.x, a.site.y, a.site.z), b.footprint_radius(), b.height_m, b.headroom_voxels(c.voxel_m), st.up, st.down, c.voxel_m, c.width, c.height, c.depth
        );
        let k = 14i64;
        for dz in -k..=k {
            let z = i64::from(a.site.z) + dz;
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            let mut row = String::new();
            for dx in -k..=k {
                let x = i64::from(a.site.x) + dx;
                let top = (a.site.y + 8).min(c.height - 1);
                let h = view.highest_support_in(x.rem_euclid(i64::from(c.width)), z as u32, 0, top);
                let mark = if dx == 0 && dz == 0 { "*" } else { " " };
                row += &match h {
                    Some(y) => format!("{mark}{:+3}", i64::from(y) - i64::from(a.site.y)),
                    None => format!("{mark}  .") ,
                };
            }
            println!("z={z:3} {row}");
        }
    }
    let step = every * u64::from(TICK_HZ);
    let total = minutes * 60 * u64::from(TICK_HZ);
    // `watch=1`: a hungry awake grounded body still for a minute or more, not biting, gets
    // a ten-tick trace (at most once every two minutes per body).
    let watch = arg(&args, "watch=", 0u8) == 1;
    let mut watching: std::collections::HashMap<u64, u64> = Default::default();
    let mut dumped: std::collections::HashMap<u64, u64> = Default::default();
    let mut k = 0;
    while k < total {
        for _ in 0..step {
            sim.step();
            if !watch {
                continue;
            }
            let fa = sim.fauna();
            let v = fa.view();
            for a in v.animals {
                let Some(f) = a.founder else { continue };
                let hungry = a.eating || a.emergency;
                let candidate = hungry
                    && a.state != cubarium_voxel_fauna::State::Asleep
                    && a.state != cubarium_voxel_fauna::State::Cropping
                    && a.attached.is_none()
                    && a.mobility.mode == cubarium_voxel_fauna::MobilityMode::Ground
                    && a.still_ticks >= 60 * u64::from(TICK_HZ)
                    && f != Founder::Blind;
                if candidate && !watching.contains_key(&a.id) && dumped.get(&a.id).is_none_or(|&t| v.tick >= t + 2400) {
                    watching.insert(a.id, 10);
                    dumped.insert(a.id, v.tick);
                }
                if let Some(left) = watching.get_mut(&a.id) {
                    let fb = a.founder_state.feedback;
                    println!(
                        "  watch tick={} {}#{} {:?} x={:.3} z={:.3} y={} h={:.3} still={} held=({:.2},{:.2},{:.2}) fb=(att {:.4} del {:.4} fwd {:.4} turn {:.4}) steer={:?} press={:?} trapped={:?} eat={} emerg={} reserve={:.3} body={:.3}",
                        v.tick, f.name(), a.id, a.state, a.pose.x, a.pose.z, a.site.y, a.pose.heading_rad, a.still_ticks,
                        a.founder_state.held.forward, a.founder_state.held.turn, a.founder_state.held.feed,
                        fb.attempted_equivalent, fb.delivered_equivalent, fb.delivered_forward, fb.delivered_turn,
                        fa.steer_of(a.id), fa.press_of(a.id), fa.trapped_of(a.id), a.eating, a.emergency, a.reserve, a.body,
                    );
                    *left -= 1;
                    if *left == 0 {
                        watching.remove(&a.id);
                    }
                }
            }
        }
        k += step;
        let t = (start_min * 60) as f64 + (k / u64::from(TICK_HZ)) as f64;
        shredders(sim.fauna(), t, ring);
        loftstriders(sim.fauna(), t);
    }
    if !trace.is_empty() {
        for _ in 0..trace_ticks {
            sim.step();
            let fa = sim.fauna();
            let v = fa.view();
            for &id in &trace {
                let Some(a) = v.animal(id) else { continue };
                let fb = a.founder_state.feedback;
                println!(
                    "tick={} id={id} {:?}/{:?} x={:.3} z={:.3} y={} h={:.3} held=({:.2},{:.2},{:.2}) fb=(att {:.4} del {:.4} fwd {:.4} turn {:.4}) steer={:?} press={:?} eating={} emerg={} body={:.3} reserve={:.3} age={} stride={:?}",
                    v.tick, a.state, a.mobility.mode, a.pose.x, a.pose.z, a.site.y, a.pose.heading_rad,
                    a.founder_state.held.forward, a.founder_state.held.turn, a.founder_state.held.feed,
                    fb.attempted_equivalent, fb.delivered_equivalent, fb.delivered_forward, fb.delivered_turn,
                    fa.steer_of(id), fa.press_of(id), a.eating, a.emergency, a.body, a.reserve, a.age_ticks,
                    a.founder,
                );
            }
        }
    }
    if let Some(dir) = save {
        let (w, fl, fa) = sim.layers();
        cubarium_voxel_sim::saved::save(std::path::Path::new(dir), w, fl, fa, start_min + minutes).expect("save");
    }
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
        Params { world: cfg.sim.world, ..Params::defaults() },
        &Layers { file: Some(file), ..Layers::default() },
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
