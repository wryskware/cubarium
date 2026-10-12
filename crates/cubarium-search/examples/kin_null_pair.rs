//! **The null-pair noise** (KIN-TRAIN step 4, `design/behaviour-ai-2026-10-11.md` §11.7):
//! two matched copies with identical brains from one start, and how far their scores
//! diverge. Matched copies work only if this is small beside a perturbation's effect.
//!
//! ```text
//! # staged encounters (ENGAGE), focal on a brain variant or on its heuristic
//! kin_null_pair engage config=config/engagement.toml arm=family-3 seeds=4 seconds=300
//! # a whole live training world, one sim thread vs several
//! kin_null_pair live seconds=60 threads=1,4 seeds=2
//! ```
//!
//! One JSON line per null pair on stdout, then a summary line per group.

use cubarium_search::es::voxel::kin::matched::{self, CopyPlan, EngageScene, SavedScene, Scene, SimStart};
use cubarium_search::es::voxel::kin::KinConfig;
use cubarium_search::es::voxel::live::variants::VariantTable;
use cubarium_voxel_fauna::Founder;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn table(f: Founder) -> VariantTable {
    let m = f.manifest();
    let centre = cubarium_search::es::tensor::initial_center_runtime(m.inputs(), m.outputs(), m.hidden, 5);
    VariantTable::antithetic(f, &centre, 3, 0.05, 0, 1).expect("a valid table")
}

fn report(group: &str, rows: &[matched::NullPair]) {
    let max = |f: fn(&matched::NullPair) -> f64| rows.iter().map(f).fold(0.0, f64::max);
    let identical = rows.iter().filter(|r| r.max_combined_gap == 0.0 && r.unmatched_bodies == 0 && r.decisions_changed == 0.0).count();
    println!(
        "{{\"summary\":\"{group}\",\"pairs\":{},\"bit_identical\":{identical},\"max_focal_gap\":{},\"max_own_gap\":{},\"max_combined_gap\":{},\"max_decisions_changed\":{},\"unmatched_bodies\":{}}}",
        rows.len(),
        max(|r| r.focal_gap),
        max(|r| r.max_own_gap),
        max(|r| r.max_combined_gap),
        max(|r| r.decisions_changed),
        rows.iter().map(|r| r.unmatched_bodies).sum::<usize>(),
    );
}

fn run<S: Scene>(group: &str, scene: &S, lineages: &[Founder], threads: usize, rows: &mut Vec<matched::NullPair>) {
    let copy = scene.open().expect("scene opens");
    for &f in lineages {
        let focals = scene.focals(&copy, f);
        let Some(&focal) = focals.first() else { continue };
        let t = table(f);
        for (label, plan) in [("brain", CopyPlan { focal, candidate: Some(0) }), ("heuristic", CopyPlan { focal, candidate: None })] {
            let started = std::time::Instant::now();
            let np = matched::null_pair(scene, plan, Some(&t), &KinConfig::default(), threads).expect("null pair");
            println!(
                "{{\"group\":\"{group}\",\"lineage\":\"{}\",\"focal_on\":\"{label}\",\"wall_s\":{:.2},\"pair\":{}}}",
                f.name(),
                started.elapsed().as_secs_f64(),
                serde_json::to_string(&np).expect("json")
            );
            rows.push(np);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("engage");
    let seeds: u64 = arg(&args, "seeds").map_or(2, |s| s.parse().expect("seeds"));
    match mode {
        "engage" => {
            let path = arg(&args, "config").unwrap_or("config/engagement.toml");
            let mut config = cubarium_voxel_sim::engagement::Config::read(std::path::Path::new(path)).expect("config");
            if let Some(s) = arg(&args, "seconds") {
                config.duration_s = s.parse().expect("seconds");
            }
            let name = arg(&args, "arm").unwrap_or("family-3");
            let arm = config.arms.iter().find(|a| a.name == name).expect("arm").clone();
            let mut rows = Vec::new();
            for seed in 0..seeds {
                let scene = EngageScene { config: config.clone(), arm: arm.clone(), seed, centres: Vec::new() };
                run(&format!("engage-{name}"), &scene, &[Founder::Loftstrider, Founder::Chorister], 2, &mut rows);
            }
            report(&format!("engage-{name}"), &rows);
        }
        "live" => {
            use cubarium_search::es::voxel::live::{ecology::EcologyParams, world};
            let seconds: f64 = arg(&args, "seconds").map_or(60.0, |s| s.parse().expect("seconds"));
            let ticks = (seconds * f64::from(cubarium_voxel_fauna::TICK_HZ)) as u64;
            let threads: Vec<usize> = arg(&args, "threads").unwrap_or("1,4").split(',').map(|t| t.parse().expect("threads")).collect();
            let ecology = EcologyParams::defaults();
            for &th in &threads {
                let mut rows = Vec::new();
                for seed in 0..seeds {
                    let (sim, _) = world::found_world(
                        &world::training_config(),
                        7 + seed * 1000,
                        ecology.fauna().founder_counts_without_egg_bank(),
                        &ecology,
                    );
                    let scene = SavedScene { start: SimStart::capture(&sim, &[]), ticks, centres: Vec::new(), threads: th, water_every: 1 };
                    let lineages: Vec<Founder> = Founder::ALL.into_iter().filter(|f| sim.fauna().view().animals.iter().any(|a| a.founder == Some(*f))).collect();
                    run(&format!("live-threads{th}"), &scene, &lineages, 2, &mut rows);
                }
                report(&format!("live-threads{th}"), &rows);
            }
        }
        other => panic!("unknown mode {other}: engage or live"),
    }
}
