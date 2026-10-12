//! **A staged scene's per-second trace of the bodies holding one intent** (BRAIN-REST step 3):
//! for each focal body holding `intent` at a whole second, its state, held forward, mobility
//! mode, steering layer, rest bout and executed rest ask; then a count of each combination.
//! The heuristic drives (or `brain=FILE`).
//!
//! ```text
//! rest_trace scene=seedporter_rest seeds=101,102 [intent=rest] [brain=FILE] [lines=1]
//! ```

use cubarium_search::es::voxel::store::load_brain_driver;
use cubarium_voxel_sim::mode_scenes::{self, ModeScene};
use std::collections::BTreeMap;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let scene = ModeScene::parse(arg(&args, "scene").unwrap_or("seedporter_rest")).ok_or("unknown scene")?;
    let seeds: Vec<u64> = arg(&args, "seeds").unwrap_or("101").split(',').filter_map(|s| s.parse().ok()).collect();
    let f = scene.founder();
    let intent = arg(&args, "intent").unwrap_or(scene.intent());
    let k = f.manifest().menu.index(intent).ok_or("intent not on the menu")?;
    let lines = arg(&args, "lines").is_some();
    let cfg = mode_scenes::Config::default();
    let per_s = u64::from(cubarium_voxel_fauna::TICK_HZ);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for seed in seeds {
        let mut e = mode_scenes::found(&cfg, scene, seed)?;
        if let Some(p) = arg(&args, "brain") {
            let d = load_brain_driver(std::path::Path::new(p), f)?;
            cubarium_search::es::voxel::live::install_centres(e.sim.fauna_mut(), &[(f, d)]);
        }
        let stay = e.sim.fauna().config().teachers.forage_stay_forward;
        let mut t = 0u64;
        while e.step() {
            t += 1;
            if t % per_s != 0 {
                continue;
            }
            let fauna = e.sim.fauna();
            let v = fauna.view();
            for id in &e.focal {
                let Some(a) = v.animal(*id) else { continue };
                let held = usize::from(a.intent.held) == k;
                let fwd = a.founder_state.held.forward;
                let key = format!(
                    "held={held} state={:?} still={} mode={:?} steer={:?} bout={} ask={:?}",
                    a.state,
                    fwd <= stay,
                    a.mobility.mode,
                    fauna.steer_of(*id),
                    a.rest_ticks > 0,
                    a.intent.executed.rest
                );
                if lines {
                    println!("seed={seed} s={} id={id} fwd={fwd:.2} {key}", t / per_s);
                }
                *counts.entry(key).or_default() += 1;
            }
        }
    }
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    for (key, n) in rows {
        println!("{n:6} {key}");
    }
    Ok(())
}
