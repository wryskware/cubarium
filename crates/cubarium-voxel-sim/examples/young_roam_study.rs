//! **YOUNG-ROAM's check** (step 5 of `design/handoffs/young-roam-2026-10-12.md`). Headless; no
//! renderer, no window. Readouts only: nothing here changes a rule.
//!
//! - `young_roam_study engage config=PATH seeds=A-B [arm=NAME]` — staged family–pack
//!   encounters through the engagement harness: one JSON row per episode, then one `SUMMARY`
//!   line per arm (excursions, their ends, bleats, reunions, chases on the calf and how far it
//!   was from its mother when they began).
//! - `young_roam_study wide CONFIG SEED MINUTES OUT [set=PATH=VALUE]... [threads=N]
//!   [water_every=N]` — one world from `CONFIG` (the wide terrarium), stepped `MINUTES`; every
//!   tick it reads each young loftstrider and seedporter whose recorded mother lives: its
//!   distance to her, its excursions and bleats, and the seconds from each bleat until she is
//!   within its birth comfort (`young.follow_m`). A dependent seedporter's contact share (3D
//!   within `nurse_radius_m`) and mean distance are SP-FOLLOW's metric. Writes `OUT/summary.json`
//!   (by lineage and by quarter of the roam) and `OUT/status`, then `OUT/DONE`.
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use cubarium_voxel_fauna::{Founder, TICK_HZ};
use cubarium_voxel_sim::{
    Sim, SimConfig, engagement,
    found::{LayerConfig, Seeds, found},
    params::{Layers, Params},
};
use serde_json::json;

fn range(s: &str) -> Result<(u64, u64), String> {
    let (a, b) = s.split_once('-').unwrap_or((s, s));
    Ok((a.parse().map_err(|e| format!("{e}"))?, b.parse().map_err(|e| format!("{e}"))?))
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

fn engage(args: &[String]) -> Result<(), String> {
    let get = |key: &str| args.iter().find_map(|a| a.strip_prefix(key));
    let cfg = engagement::Config::read(Path::new(get("config=").ok_or("config= required")?))?;
    let (a, b) = range(get("seeds=").unwrap_or("1-10"))?;
    let only = get("arm=");
    for arm in cfg.arms.iter().filter(|x| only.is_none_or(|o| o == x.name)) {
        let mut rows = Vec::new();
        for seed in a..=b {
            let row = engagement::run(&cfg, seed, arm)?;
            println!("{}", serde_json::to_string(&row).map_err(|e| e.to_string())?);
            rows.push(row);
        }
        let n = rows.len();
        let sum = |f: &dyn Fn(&engagement::RowOut) -> u64| rows.iter().map(f).sum::<u64>();
        let mut ends: BTreeMap<String, u64> = BTreeMap::new();
        for r in &rows {
            for (k, v) in &r.excursion_ends {
                *ends.entry(k.clone()).or_default() += v;
            }
        }
        let mut reunions: Vec<f64> = rows.iter().flat_map(|r| r.reunions_s.iter().copied()).collect();
        let reunited = reunions.len();
        let chase_d: Vec<f64> = rows.iter().flat_map(|r| r.calf_chase_from_mother_m.iter().copied()).collect();
        let comfort = 6.0; // the family's herd comfort, for the "strayed" split only
        let mut far: Vec<f64> = rows.iter().map(|r| r.calf_far_m).collect();
        println!(
            "SUMMARY {}",
            json!({
                "arm": arm.name, "episodes": n,
                "episodes_with_excursion": rows.iter().filter(|r| r.excursions.iter().sum::<u64>() > 0).count(),
                "excursions_by_goal_food_animal_place": [sum(&|r| r.excursions[0]), sum(&|r| r.excursions[1]), sum(&|r| r.excursions[2])],
                "excursion_ends": ends,
                "bleats": sum(&|r| r.bleats),
                "reunited": reunited,
                "reunion_s_median": median(&mut reunions),
                "mother_approach_m_median": median(&mut rows.iter().flat_map(|r| r.mother_approach_m.iter().copied()).collect::<Vec<_>>()),
                "explore_ticks": sum(&|r| r.explore_ticks),
                "mother_answer_ticks": sum(&|r| r.answer_ticks),
                "calf_far_m_median": median(&mut far),
                "calf_chases": sum(&|r| r.calf_chases),
                "calf_chases_strayed_over_6m": chase_d.iter().filter(|&&d| d > comfort).count(),
                "calf_kills": rows.iter().filter(|r| r.outcome == engagement::Outcome::CalfKilled).count(),
            })
        );
    }
    Ok(())
}

/// One young's running read.
#[derive(Default)]
struct Young {
    since: Option<u64>,
    bleat_at: Option<u64>,
}

/// Readouts of one lineage in one quarter of the roam.
#[derive(Default, Clone)]
struct Bin {
    ticks: u64,
    dist_sum: f64,
    dists: Vec<f64>,
    starts: u64,
    bleats: u64,
    reunions_s: Vec<f64>,
}

fn wide(args: &[String]) -> Result<(), String> {
    if args.len() < 5 {
        return Err("usage: young_roam_study wide CONFIG SEED MINUTES OUT [set=K=V]...".into());
    }
    cubarium_voxel_sim::experiment_args(args);
    let file = std::fs::read_to_string(&args[1]).map_err(|e| e.to_string())?.parse::<toml::Table>().map_err(|e| e.to_string())?;
    let sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=")).map(str::to_string).collect();
    let p = Params::resolve(&Layers { file: Some(file), sets: sets.clone(), ..Layers::default() })?;
    let seed: u64 = args[2].parse().map_err(|e| format!("{e}"))?;
    let minutes: f64 = args[3].parse().map_err(|e| format!("{e}"))?;
    let out = Path::new(&args[4]);
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let f = found(&p.world, &LayerConfig::new(&p, p.world.voxel_m), p.fauna.founder_counts(), Seeds::from(seed), false);
    let mut sim = Sim::ambient(f.world, f.flora, f.fauna, SimConfig::experiment());
    let ring = f64::from(p.world.width) * p.world.voxel_m;
    let hz = f64::from(TICK_HZ);
    let limit = (minutes * 60.0 * hz).round() as u64;
    let lineages = [Founder::Loftstrider, Founder::Seedporter];
    let mut young: BTreeMap<u64, Young> = BTreeMap::new();
    let mut bins: BTreeMap<(usize, usize), Bin> = BTreeMap::new();
    // Dependent seedporters (SP-FOLLOW's metric).
    let (mut dep_ticks, mut dep_contact, mut dep_dist) = (0u64, 0u64, 0.0f64);
    let mut last_bleats = [0u64; 2];
    let started = std::time::Instant::now();
    for tick in 1..=limit {
        sim.step();
        let fauna = sim.fauna();
        let view = fauna.view();
        let bleats_now = lineages.map(|l| fauna.roam_tally(l).bleats);
        let mut bleated = [bleats_now[0] > last_bleats[0], bleats_now[1] > last_bleats[1]];
        last_bleats = bleats_now;
        for a in view.animals {
            let Some(li) = a.founder.and_then(|f| lineages.iter().position(|&l| l == f)) else { continue };
            let Some(m) = a.mother.and_then(|m| view.animal(m)) else { continue };
            let phys = view.config.founder(lineages[li]);
            let age = a.age_ticks as f64 / hz;
            let mut dx = (m.pose.x - a.pose.x).rem_euclid(ring);
            if dx > 0.5 * ring {
                dx -= ring;
            }
            let d = dx.hypot(m.pose.z - a.pose.z);
            let n = phys.nursing;
            if n.on() && age < n.weaning_age_s {
                dep_ticks += 1;
                dep_dist += d;
                if d.hypot(m.pose.y_m - a.pose.y_m) <= n.nurse_radius_m {
                    dep_contact += 1;
                }
                continue;
            }
            let y = phys.young;
            let start = if n.on() { n.weaning_age_s } else { 0.0 };
            if !y.roams() || age >= y.independence_s {
                young.remove(&a.id);
                continue;
            }
            let k = ((age - start) / (y.independence_s - start)).clamp(0.0, 0.999);
            let bin = bins.entry((li, (k * 4.0) as usize)).or_default();
            bin.ticks += 1;
            bin.dist_sum += d;
            if tick % (hz as u64) == 0 {
                bin.dists.push(d);
            }
            let me = young.entry(a.id).or_default();
            let now = fauna.excursion_of(a.id).map(|e| e.since);
            if now.is_some() && now != me.since {
                bin.starts += 1;
            }
            if me.since.is_some() && now.is_none() && bleated[li] {
                bin.bleats += 1;
                bleated[li] = false;
                me.bleat_at = Some(tick);
            }
            me.since = now;
            if let Some(at) = me.bleat_at {
                if d <= y.follow_m {
                    bin.reunions_s.push((tick - at) as f64 / hz);
                    me.bleat_at = None;
                }
            }
        }
        if tick % (600 * hz as u64) == 0 {
            std::fs::write(out.join("status"), format!("minute={:.0} wall_s={:.0}\n", tick as f64 / hz / 60.0, started.elapsed().as_secs_f64())).map_err(|e| e.to_string())?;
        }
    }
    let fauna = sim.fauna();
    let mut by = Vec::new();
    for ((li, q), b) in &bins {
        let hours = b.ticks as f64 / hz / 3600.0;
        let mut ds = b.dists.clone();
        ds.sort_by(f64::total_cmp);
        let p90 = ds.get(ds.len() * 9 / 10).copied();
        let mut r = b.reunions_s.clone();
        by.push(json!({
            "lineage": lineages[*li].name(), "roam_quarter": q, "young_hours": hours,
            "mean_from_mother_m": if b.ticks > 0 { b.dist_sum / b.ticks as f64 } else { 0.0 },
            "p90_from_mother_m": p90,
            "excursions": b.starts, "excursions_per_young_h": if hours > 0.0 { b.starts as f64 / hours } else { 0.0 },
            "bleats": b.bleats, "bleats_per_young_h": if hours > 0.0 { b.bleats as f64 / hours } else { 0.0 },
            "reunited": r.len(), "reunion_s_median": median(&mut r),
        }));
    }
    let tallies: Vec<_> = lineages
        .iter()
        .map(|&l| {
            let t = fauna.roam_tally(l);
            json!({"lineage": l.name(), "started_food_animal_place": t.started,
                "ended": cubarium_voxel_fauna::roam::End::ALL.iter().map(|e| (e.name(), t.ended[e.index()])).collect::<BTreeMap<_, _>>(),
                "bleats": t.bleats, "answer_samples": t.answer_samples})
        })
        .collect();
    let ledger = &fauna.view().ledger;
    let dep_h = dep_ticks as f64 / hz / 3600.0;
    let summary = json!({
        "seed": seed, "minutes": minutes, "sets": sets,
        "dependent_seedporter": {
            "young_hours": dep_h,
            "contact_share": if dep_ticks > 0 { dep_contact as f64 / dep_ticks as f64 } else { 0.0 },
            "mean_from_mother_m": if dep_ticks > 0 { dep_dist / dep_ticks as f64 } else { 0.0 },
            "milk_per_young_h": if dep_h > 0.0 { ledger.nursed_received_by_founder[Founder::Seedporter.index()] / dep_h } else { 0.0 },
        },
        "by_roam_quarter": by,
        "tallies": tallies,
        "wall_s": started.elapsed().as_secs_f64(),
    });
    let mut w = std::fs::File::create(out.join("summary.json")).map_err(|e| e.to_string())?;
    w.write_all(serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::write(out.join("DONE"), "").map_err(|e| e.to_string())?;
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("engage") => engage(&args[1..]),
        Some("wide") => wide(&args[1..]),
        _ => Err("young_roam_study engage|wide ...".into()),
    }
}
