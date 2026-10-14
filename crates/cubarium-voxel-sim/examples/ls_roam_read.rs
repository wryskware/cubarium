//! **LS-ROAM's read** (`design/handoffs/ls-roam-2026-10-14.md`): where loftstrider families are,
//! hour to hour. Headless; readouts only, nothing here changes a rule.
//!
//! - `ls_roam_read saves ROOT...` — every world directory under each ROOT that holds
//!   `run/ckpt/m*` saves: each save's grown loftstriders grouped into families, and each
//!   family's centroid moved between saves.
//! - `ls_roam_read run (config=PATH seed=N | load=DIR) MINUTES OUT [every_min=5] [set=K=V]...
//!   [threads=N] [water_every=N]` — steps the world `MINUTES`, samples every `every_min`, and
//!   tallies each grown loftstrider's waking seconds by its held intent, moving or still, its
//!   eating latch on or off. Writes `OUT/track.csv`, `OUT/summary.json` and `OUT/DONE`.
//!
//! **Families.** Grown loftstriders (not a young of a living mother) within `link_m` (30 m) of
//! one another, single-link; a family keeps its label across samples by the most shared ids.
//! **Moved** is the centroid's ground distance between samples (x wraps); **area** is the
//! count of 10 m cells any member stood in at a sample, times 100 m².
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use cubarium_voxel_fauna::{Fauna, Founder, State, TICK_HZ, intent, roam};
use cubarium_voxel_sim::{
    Sim, SimConfig,
    found::{LayerConfig, Seeds, found},
    params::{Layers, Params},
};
use serde_json::json;

const LINK_M: f64 = 30.0;
const CELL_M: f64 = 10.0;

#[derive(Clone, Debug)]
struct Body {
    id: u64,
    x: f64,
    z: f64,
}

/// One sample: the minute and its grown loftstriders.
struct Sample {
    minute: f64,
    grown: Vec<Body>,
    calves: usize,
}

fn wrap(d: f64, ring: f64) -> f64 {
    let d = d.rem_euclid(ring);
    if d > 0.5 * ring { d - ring } else { d }
}

fn sample(fauna: &Fauna, minute: f64) -> Sample {
    let view = fauna.view();
    let mut grown = Vec::new();
    let mut calves = 0;
    for a in view.animals.iter().filter(|a| a.founder == Some(Founder::Loftstrider)) {
        if roam::young_of_mother(fauna.config(), a) {
            calves += 1;
        } else {
            grown.push(Body { id: a.id, x: a.pose.x, z: a.pose.z });
        }
    }
    Sample { minute, grown, calves }
}

/// Single-link groups of `bodies` within `LINK_M`.
fn families(bodies: &[Body], ring: f64) -> Vec<Vec<usize>> {
    let n = bodies.len();
    let mut label: Vec<usize> = (0..n).collect();
    fn root(l: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while l[r] != r {
            r = l[r];
        }
        l[i] = r;
        r
    }
    for i in 0..n {
        for j in i + 1..n {
            let d = wrap(bodies[j].x - bodies[i].x, ring).hypot(bodies[j].z - bodies[i].z);
            if d <= LINK_M {
                let (a, b) = (root(&mut label, i), root(&mut label, j));
                label[a] = b;
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        let r = root(&mut label, i);
        groups.entry(r).or_default().push(i);
    }
    groups.into_values().collect()
}

fn centroid(bodies: &[Body], members: &[usize], ring: f64) -> (f64, f64) {
    let (x0, mut sx, mut sz) = (bodies[members[0]].x, 0.0, 0.0);
    for &m in members {
        sx += wrap(bodies[m].x - x0, ring);
        sz += bodies[m].z;
    }
    let k = members.len() as f64;
    ((x0 + sx / k).rem_euclid(ring), sz / k)
}

/// Each family's track over the samples: label → (minute, centroid, members, cells).
fn tracks(samples: &[Sample], ring: f64) -> BTreeMap<usize, Vec<(f64, (f64, f64), BTreeSet<u64>)>> {
    let mut out: BTreeMap<usize, Vec<(f64, (f64, f64), BTreeSet<u64>)>> = BTreeMap::new();
    let mut last: Vec<(usize, BTreeSet<u64>)> = Vec::new();
    let mut next_label = 0;
    for s in samples {
        let mut now = Vec::new();
        for g in families(&s.grown, ring) {
            let ids: BTreeSet<u64> = g.iter().map(|&i| s.grown[i].id).collect();
            let best = last.iter().map(|(l, prev)| (prev.intersection(&ids).count(), *l)).filter(|&(k, _)| k > 0).max();
            let label = match best {
                Some((_, l)) if !now.iter().any(|(m, _): &(usize, BTreeSet<u64>)| *m == l) => l,
                _ => {
                    next_label += 1;
                    next_label - 1
                }
            };
            out.entry(label).or_default().push((s.minute, centroid(&s.grown, &g, ring), ids.clone()));
            now.push((label, ids));
        }
        last = now;
    }
    out
}

fn report(samples: &[Sample], ring: f64) -> serde_json::Value {
    let mut fams = Vec::new();
    for (label, t) in tracks(samples, ring) {
        let path: f64 = t.windows(2).map(|w| wrap(w[1].1.0 - w[0].1.0, ring).hypot(w[1].1.1 - w[0].1.1)).sum();
        let (first, last) = (&t[0], &t[t.len() - 1]);
        let net = wrap(last.1.0 - first.1.0, ring).hypot(last.1.1 - first.1.1);
        // Hourly net moves: each sample to the first one an hour or more later.
        let mut hourly = Vec::new();
        for (i, a) in t.iter().enumerate() {
            if let Some(b) = t[i..].iter().find(|b| b.0 - a.0 >= 59.9) {
                hourly.push(wrap(b.1.0 - a.1.0, ring).hypot(b.1.1 - a.1.1));
            }
        }
        let sizes: Vec<usize> = t.iter().map(|e| e.2.len()).collect();
        fams.push(json!({
            "family": label, "samples": t.len(), "from_min": first.0, "to_min": last.0,
            "size_min": sizes.iter().min(), "size_max": sizes.iter().max(),
            "centroids": t.iter().map(|e| json!([e.0, (e.1.0 * 10.0).round() / 10.0, (e.1.1 * 10.0).round() / 10.0])).collect::<Vec<_>>(),
            "path_m": path, "net_m": net,
            "hourly_net_m": hourly,
        }));
    }
    // Area: cells any grown body stood in at a sample, all families together, and per hour.
    let cells: BTreeSet<(i64, i64)> =
        samples.iter().flat_map(|s| s.grown.iter().map(|b| ((b.x / CELL_M).floor() as i64, (b.z / CELL_M).floor() as i64))).collect();
    json!({
        "samples": samples.len(),
        "grown": samples.iter().map(|s| s.grown.len()).collect::<Vec<_>>(),
        "calves": samples.iter().map(|s| s.calves).collect::<Vec<_>>(),
        "area_m2": cells.len() as f64 * CELL_M * CELL_M,
        "families": fams,
    })
}

fn saves(roots: &[String]) -> Result<(), String> {
    for root in roots {
        let mut worlds: Vec<PathBuf> = Vec::new();
        let mut stack = vec![PathBuf::from(root)];
        while let Some(d) = stack.pop() {
            if d.join("run/ckpt").is_dir() {
                worlds.push(d);
                continue;
            }
            if let Ok(rd) = std::fs::read_dir(&d) {
                stack.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
            }
        }
        worlds.sort();
        for w in worlds {
            let mut ckpts: Vec<PathBuf> =
                std::fs::read_dir(w.join("run/ckpt")).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).filter(|p| p.join("fauna.bin").is_file()).collect();
            ckpts.sort();
            let mut samples = Vec::new();
            let mut ring = 0.0;
            for c in &ckpts {
                let world = cubarium_voxel::World::load(&std::fs::read(c.join("world.bin")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                ring = f64::from(world.config().width) * world.config().voxel_m;
                let fauna = Fauna::load(&std::fs::read(c.join("fauna.bin")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                let minute: f64 = std::fs::read_to_string(c.join("minute")).map_err(|e| e.to_string())?.trim().parse().map_err(|e| format!("{e}"))?;
                samples.push(sample(&fauna, minute));
            }
            let mut r = report(&samples, ring);
            r["world"] = json!(w.display().to_string());
            println!("{}", serde_json::to_string(&r).map_err(|e| e.to_string())?);
        }
    }
    Ok(())
}

fn run(args: &[String]) -> Result<(), String> {
    cubarium_voxel_sim::experiment_args(args);
    let get = |key: &str| args.iter().find_map(|a| a.strip_prefix(key));
    let pos: Vec<&String> = args.iter().filter(|a| !a.contains('=')).collect();
    if pos.len() < 2 {
        return Err("usage: ls_roam_read run (config=PATH seed=N | load=DIR) MINUTES OUT [every_min=5] [set=K=V]...".into());
    }
    let minutes: f64 = pos[0].parse().map_err(|e| format!("{e}"))?;
    let out = Path::new(pos[1]);
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let every_min: f64 = get("every_min=").map_or(Ok(5.0), str::parse).map_err(|e| format!("{e}"))?;
    let sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=")).map(str::to_string).collect();
    let (mut sim, start_min) = if let Some(dir) = get("load=") {
        let s = cubarium_voxel_sim::saved::load(Path::new(dir), &[])?;
        (Sim::ambient(s.world, s.flora, s.fauna, SimConfig::experiment()), s.minute as f64)
    } else {
        let file = std::fs::read_to_string(get("config=").ok_or("config= or load= required")?).map_err(|e| e.to_string())?.parse::<toml::Table>().map_err(|e| e.to_string())?;
        let p = Params::resolve(&Layers { file: Some(file), sets: sets.clone(), ..Layers::default() })?;
        let seed: u64 = get("seed=").unwrap_or("1").parse().map_err(|e| format!("{e}"))?;
        let f = found(&p.world, &LayerConfig::new(&p, p.world.voxel_m), p.fauna.founder_counts(), Seeds::from(seed), false);
        (Sim::ambient(f.world, f.flora, f.fauna, SimConfig::experiment()), 0.0)
    };
    let ring = f64::from(sim.world().config().width) * sim.world().config().voxel_m;
    let hz = f64::from(TICK_HZ);
    let limit = (minutes * 60.0 * hz).round() as u64;
    let every = ((every_min * 60.0 * hz).round() as u64).max(1);
    let menu = intent::menu_of(Founder::Loftstrider);
    let mut track = std::fs::File::create(out.join("track.csv")).map_err(|e| e.to_string())?;
    writeln!(track, "minute,id,calf,x,z,state,eating,intent,stay_s,stay_offer").map_err(|e| e.to_string())?;
    // Waking seconds of grown bodies by (intent, moving, eating), and asleep seconds.
    let mut time: BTreeMap<String, f64> = BTreeMap::new();
    let mut asleep_s = 0.0;
    let mut samples = vec![sample(sim.fauna(), start_min)];
    let started = std::time::Instant::now();
    for tick in 1..=limit {
        sim.step();
        let fauna = sim.fauna();
        let view = fauna.view();
        for a in view.animals.iter().filter(|a| a.founder == Some(Founder::Loftstrider)) {
            if roam::young_of_mother(fauna.config(), a) {
                continue;
            }
            if a.state == State::Asleep {
                asleep_s += 1.0 / hz;
                continue;
            }
            let len = cubarium_voxel_fauna::body_of(fauna.config(), a).map_or(1.0, |b| b.length_m);
            let moving = a.speed_m_s > 0.1 * len;
            let name = menu.named.get(usize::from(a.intent.held)).map_or("none", |c| c.name);
            let key = format!("{name}|{}|{}", if moving { "moving" } else { "still" }, if a.eating { "eating" } else { "sated" });
            *time.entry(key).or_default() += 1.0 / hz;
        }
        if tick % every == 0 {
            let minute = start_min + tick as f64 / hz / 60.0;
            let stays = fauna.forage_stays();
            for a in view.animals.iter().filter(|a| a.founder == Some(Founder::Loftstrider)) {
                let name = menu.named.get(usize::from(a.intent.held)).map_or("none", |c| c.name);
                let (stay_s, offer) = stays.iter().find(|s| s.0 == a.id).map_or((String::new(), String::new()), |s| (format!("{:.0}", s.1), s.2.map_or(String::new(), |r| format!("{r:.3}"))));
                writeln!(track, "{minute:.1},{},{},{:.2},{:.2},{:?},{},{name},{stay_s},{offer}", a.id, roam::young_of_mother(fauna.config(), a), a.pose.x, a.pose.z, a.state, a.eating)
                    .map_err(|e| e.to_string())?;
            }
            samples.push(sample(fauna, minute));
            std::fs::write(out.join("status"), format!("minute={minute:.0} wall_s={:.0}\n", started.elapsed().as_secs_f64())).map_err(|e| e.to_string())?;
        }
    }
    let awake: f64 = time.values().sum();
    let mut r = report(&samples, ring);
    r["sets"] = json!(sets);
    r["asleep_h"] = json!(asleep_s / 3600.0);
    r["awake_h"] = json!(awake / 3600.0);
    r["awake_share"] = json!(time.iter().map(|(k, v)| (k.clone(), (v / awake.max(1e-9) * 1000.0).round() / 1000.0)).collect::<BTreeMap<_, _>>());
    let t = sim.fauna().forage_tally()[Founder::Loftstrider.index()];
    r["forage"] = json!({"stays": t.stays, "stay_s": t.stay_s, "stay_max_s": t.stay_max_s, "offer_samples": t.offer_samples,
        "offer_mean": if t.offer_samples > 0 { t.offer_sum / t.offer_samples as f64 } else { 0.0 }, "offer_low": t.offer_low,
        "give_ups": t.give_ups, "leave_samples": t.leave_samples});
    let h = sim.fauna().herd_tally()[Founder::Loftstrider.index()];
    r["herd"] = json!({"pulled_samples": h.pulled_samples, "followed_samples": h.followed_samples});
    r["wall_s"] = json!(started.elapsed().as_secs_f64());
    let mut w = std::fs::File::create(out.join("summary.json")).map_err(|e| e.to_string())?;
    w.write_all(serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::write(out.join("DONE"), "").map_err(|e| e.to_string())?;
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("saves") => saves(&args[1..]),
        Some("run") => run(&args[1..]),
        _ => Err("ls_roam_read saves ROOT... | run ...".into()),
    }
}
