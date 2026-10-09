//! **Why do x30 founders starve amid plants?** Measurement only (stage 1 diagnosis,
//! 2026-10-01).
//!
//! ```text
//! starve_probe brain=<transfer:centre|heuristic> shredder=<…> [seed=1] [minutes=20] [world=config/desktop/terrarium.toml]
//! ```
//!
//! Founds one world at the defaults, drives the two trained lineages with the given brains
//! (each body's controller wrapped to record what it sensed and asked), and reports per
//! lineage: upkeep, the intake cap and the observed intake per body-hour, the time share
//! eating / moving / idle / asleep, and for every body that starved, its last 5 minutes:
//! how often food was at the mouth (taste), in sight (cone foliage, or the detritus
//! smell), and whether it asked to feed then.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use cubarium_search::es::voxel::live::{self, ecology::EcologyParams, search, window, world};
use cubarium_search::es::voxel::score::BodyMeter;
use cubarium_voxel_fauna::{Controller, Departure, Food, Founder, Response, State};

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter().find_map(|a| a.strip_prefix(key).map(str::to_string))
}

/// One controller sample: taste cue, sight (cone foliage or detritus smell), feed asked.
type Sample = (f64, f64, f64, f64);
type Log = Arc<Mutex<HashMap<u64, VecDeque<Sample>>>>;
const KEEP: usize = 1200; // 5 min of 0.25 s samples
static ALL: std::sync::LazyLock<Mutex<HashMap<usize, [f64; 6]>>> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

struct Recorder {
    id: u64,
    founder: Founder,
    inner: Box<dyn Controller>,
    log: Log,
}

impl Controller for Recorder {
    fn drive(&mut self, o: &[f64]) -> Response {
        let taste = if o[17] > 0.5 { o[15] } else { 0.0 };
        let sight = match self.founder {
            Founder::Browser => o[20].max(o[26]).max(o[32]),
            _ => o[18],
        };
        let r = self.inner.drive(o);
        let (fwd, feed) = match r {
            Response::Logits(l) => (l[0].tanh(), 1.0 / (1.0 + (-l[2]).exp())),
            Response::Bounded(a) | Response::Body(a, _) => (a.forward, a.feed),
        };
        {
            let mut all = ALL.lock().unwrap();
            let e = all.entry(self.founder.index()).or_insert([0.0; 6]);
            e[0] += 1.0;
            if taste > 1e-3 {
                e[1] += 1.0;
                e[2] += feed;
                if taste > 0.02 {
                    e[5] += 1.0;
                }
            }
            if sight > 0.02 {
                e[3] += 1.0;
            }
            e[4] += fwd.abs();
        }
        let mut log = self.log.lock().unwrap();
        let q = log.entry(self.id).or_default();
        q.push_back((taste, sight, feed, fwd));
        if q.len() > KEEP {
            q.pop_front();
        }
        r
    }
    fn reset(&mut self) {
        self.inner.reset();
    }
}

#[derive(Default)]
struct Acc {
    body_ticks: u64,
    states: [u64; 4], // eating, moving, idle, asleep
    upkeep: f64,
    intake: f64,
    motor: f64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = arg(&args, "seed=").and_then(|s| s.parse().ok()).unwrap_or(1);
    let minutes: u64 = arg(&args, "minutes=").and_then(|s| s.parse().ok()).unwrap_or(20);
    let world_file = arg(&args, "world=").unwrap_or("config/desktop/terrarium.toml".into());
    let brains = [
        (Founder::Browser, arg(&args, "brain=").expect("brain=")),
        (Founder::Blind, arg(&args, "shredder=").expect("shredder=")),
    ];
    let cfg = search::check_world_config(Some(std::path::Path::new(&world_file))).expect("world");
    let (mut sim, info) = world::found_world(
        &cfg,
        seed,
        cubarium_voxel_fauna::FaunaConfig::default().founder_counts(),
        &EcologyParams::defaults(),
    );
    let drivers: Vec<_> = brains
        .iter()
        .map(|(f, b)| (*f, search::brain_driver(*f, b).expect("a brain")))
        .collect();
    live::install_centres(sim.fauna_mut(), &drivers);
    let teachers = sim.fauna().config().teachers;
    let log: Log = Arc::new(Mutex::new(HashMap::new()));
    let bodies: Vec<(u64, Founder)> = sim
        .fauna()
        .view()
        .animals
        .iter()
        .filter_map(|a| a.founder.map(|f| (a.id, f)))
        .collect();
    for (id, f) in &bodies {
        if let Some((_, d)) = drivers.iter().find(|(g, _)| g == f) {
            let rec = Recorder {
                id: *id,
                founder: *f,
                inner: d.fresh(teachers),
                log: Arc::clone(&log),
            };
            sim.fauna_mut().set_controller(*id, Box::new(rec));
        }
    }
    let cfgf = sim.fauna().config().clone();
    println!(
        "# seed {} world {} founders {:?} brains {:?}",
        seed, info.seed, info.founders, brains.iter().map(|b| &b.1).collect::<Vec<_>>()
    );
    for f in [Founder::Blind, Founder::Browser] {
        let p = cfgf.founder(f);
        let food = if f == Founder::Browser { Food::Foliage } else { Food::Litter };
        println!(
            "# {}: body_max {} maintenance {}/s motor {}/s at cruise bite {}/s yield {} K {} reserve_cap {} sleep_upkeep {}",
            f.name(),
            p.core.body_max,
            p.core.maintenance_per_s,
            p.motor_respiration_per_s,
            p.gut.bite_per_body_s * p.core.body_max,
            p.yield_for(food),
            p.core.bite_half_stock,
            p.core.reserve_cap,
            p.sleep_upkeep
        );
    }
    let dt = cubarium_voxel::DT;
    let mut acc: HashMap<Founder, Acc> = HashMap::new();
    let mut meters: HashMap<u64, BodyMeter> = HashMap::new();
    let mut last: HashMap<u64, (Founder, f64, f64)> = HashMap::new();
    let deaths0 = sim.fauna().view().ledger.deaths_by_founder_cause;
    let mut report_deaths: Vec<String> = Vec::new();
    let ticks = minutes * window::TICKS_PER_MIN;
    for t in 0..ticks {
        sim.step();
        let fv = sim.fauna().view();
        let mut seen = std::collections::HashSet::new();
        for a in fv.animals.iter() {
            let Some(f) = a.founder else { continue };
            if f == Founder::Lanternjaw {
                continue;
            }
            seen.insert(a.id);
            let p = cfgf.founder(f);
            let e = acc.entry(f).or_default();
            e.body_ticks += 1;
            let k = match a.state {
                State::Cropping => 0,
                State::Walking | State::Climbing => 1,
                State::Asleep => 3,
                _ => 2,
            };
            e.states[k] += 1;
            let factor = if a.state == State::Asleep { p.sleep_upkeep } else { 1.0 };
            e.upkeep += p.core.maintenance_per_s * a.body * factor * dt;
            let m = meters.entry(a.id).or_insert_with(|| BodyMeter::new(f, a));
            let d = m.observe(a);
            e.intake += d.intake;
            e.motor += d.motor;
            last.insert(a.id, (f, a.body, a.reserve));
        }
        let gone: Vec<u64> = last.keys().filter(|id| !seen.contains(id)).copied().collect();
        for id in gone {
            let (f, body, reserve) = last.remove(&id).unwrap();
            let q = log.lock().unwrap().remove(&id).unwrap_or_default();
            let n = q.len().max(1) as f64;
            let tasted: Vec<&Sample> = q.iter().filter(|s| s.0 > 1e-3).collect();
            let sighted = q.iter().filter(|s| s.1 > 0.02).count() as f64 / n;
            let feed_tasted = if tasted.is_empty() {
                f64::NAN
            } else {
                tasted.iter().map(|s| s.2).sum::<f64>() / tasted.len() as f64
            };
            let feed_all = q.iter().map(|s| s.2).sum::<f64>() / n;
            report_deaths.push(format!(
                "  died min {:.1} {} body {:.3} reserve {:.4}: last 5 min food at mouth {:.0}% of samples (mean taste {:.3}), in sight {:.0}%, feed asked {:.2} when tasting / {:.2} overall",
                t as f64 / window::TICKS_PER_MIN as f64,
                f.name(),
                body,
                reserve,
                100.0 * tasted.len() as f64 / n,
                if tasted.is_empty() { 0.0 } else { tasted.iter().map(|s| s.0).sum::<f64>() / tasted.len() as f64 },
                100.0 * sighted,
                feed_tasted,
                feed_all
            ));
        }
    }
    let deaths1 = sim.fauna().view().ledger.deaths_by_founder_cause;
    for f in [Founder::Blind, Founder::Browser] {
        let Some(e) = acc.get(&f) else { continue };
        let p = cfgf.founder(f);
        let hours = e.body_ticks as f64 * dt / 3600.0;
        let food = if f == Founder::Browser { Food::Foliage } else { Food::Litter };
        // The adult's raw bite rate (the minimal gut's body-scaled bite) at the yield.
        let cap_h = p.gut.bite_per_body_s * p.core.body_max * p.yield_for(food) * 3600.0;
        let tot = e.body_ticks.max(1) as f64;
        let starved = deaths1[f.index()][Departure::Starved.index()] - deaths0[f.index()][Departure::Starved.index()];
        println!(
            "{}: {:.2} body-h; per body-hour upkeep {:.3} motor {:.3} intake {:.3}; intake cap (hunger 1, E>>K) {:.2}; eating {:.0}% moving {:.0}% idle {:.0}% asleep {:.0}%; starved {}",
            f.name(),
            hours,
            e.upkeep / hours,
            e.motor / hours,
            e.intake / hours,
            cap_h,
            100.0 * e.states[0] as f64 / tot,
            100.0 * e.states[1] as f64 / tot,
            100.0 * e.states[2] as f64 / tot,
            100.0 * e.states[3] as f64 / tot,
            starved
        );
    }
    for (i, e) in ALL.lock().unwrap().iter() {
        let n = e[0].max(1.0);
        println!(
            "{}: all samples {}: food at mouth {:.0}% (taste > feed_gate 0.02: {:.0}%), in sight {:.0}%, feed asked when tasting {:.2}, mean |forward| {:.2}",
            Founder::ALL[*i].name(), e[0], 100.0 * e[1] / n, 100.0 * e[5] / n, 100.0 * e[3] / n, e[2] / e[1].max(1.0), e[4] / n
        );
    }
    for d in report_deaths {
        println!("{d}");
    }
}
