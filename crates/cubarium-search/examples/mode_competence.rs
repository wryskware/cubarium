//! **The rules' per-mode competence on the staged scenes** (BRAIN-FIX step 1): every scene
//! (or `scenes=a,b`) on `seeds=` under the heuristic, through
//! `two_layer::scenes::competence`, one line per run and a mean per scene. Optionally a
//! brain file (`brain=PATH`) for the scene's lineage instead of the heuristic.
//!
//! ```text
//! mode_competence seeds=101,102,103,104 [scenes=groom,feed] [brain=FILE]
//! ```

use cubarium_search::es::voxel::store::load_brain_driver;
use cubarium_search::es::voxel::two_layer::scenes;
use cubarium_voxel_sim::mode_scenes::{self, ModeScene};

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seeds: Vec<u64> = arg(&args, "seeds").unwrap_or("101,102,103,104").split(',').filter_map(|s| s.parse().ok()).collect();
    let list: Vec<ModeScene> = match arg(&args, "scenes") {
        Some(s) => s.split(',').map(|n| ModeScene::parse(n).ok_or(format!("unknown scene {n}"))).collect::<Result<_, _>>()?,
        None => ModeScene::ALL.to_vec(),
    };
    let cfg = mode_scenes::Config::default();
    for scene in list {
        let driver = match arg(&args, "brain") {
            Some(p) => match load_brain_driver(std::path::Path::new(p), scene.founder()) {
                Ok(d) => Some(d),
                Err(_) => continue,
            },
            None => None,
        };
        let mut sum = 0.0;
        for &seed in &seeds {
            let c = scenes::competence(&cfg, scene, seed, driver.as_ref())?;
            println!("{} seed={seed} competence={:.3} mode_share={:.3} deaths={} null_gap={}", scene.name(), c.competence, c.mode_share, c.deaths, c.null_gap);
            sum += c.competence;
        }
        println!("{} mean={:.3}", scene.name(), sum / seeds.len().max(1) as f64);
    }
    Ok(())
}
