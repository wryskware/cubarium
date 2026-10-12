//! **Why a cloned brain does or doesn't roost** (BRAIN-FIX step 3): the roost scene on
//! `seeds`, closed loop, the dry-run teacher labelling every state the brain reaches. Per
//! seed: when the teacher first proposes roost, when the brain first holds it, the share of
//! teacher-roost samples the brain held roost on, the brain's held intents, and how many
//! focal bodies end perched in a crown. `force=1` adds a large bias to the top's roost logit,
//! so the brain holds roost from its first decision: what the bottom does under the intent,
//! with the choice taken out. `brain=heuristic` runs the rules (their own labels).
//!
//! ```text
//! roost_diag brain=FILE seeds=101,102,103,104 [force=1] [scene=roost]
//! ```

use cubarium_search::es::voxel::EpisodeDriver;
use cubarium_search::es::voxel::two_layer::scenes::StagedScene;
use cubarium_search::es::voxel::two_layer::{TwoLayerNet, TwoLayerPolicyFile};
use cubarium_search::es::voxel::kin::matched::{Scene, SceneCopy};
use cubarium_voxel_fauna::{MobilityMode, PerchAnchor, menu_of};
use cubarium_voxel_sim::mode_scenes::{self, ModeScene};

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seeds: Vec<u64> = arg(&args, "seeds").unwrap_or("101,102,103,104").split(',').filter_map(|s| s.parse().ok()).collect();
    let scene = ModeScene::parse(arg(&args, "scene").unwrap_or("roost")).ok_or("scene")?;
    let f = scene.founder();
    let menu = menu_of(f);
    let k = menu.index(scene.intent()).ok_or("intent")?;
    let brain = arg(&args, "brain").ok_or("brain=FILE|heuristic")?;
    let driver = if brain == "heuristic" {
        None
    } else {
        let file = TwoLayerPolicyFile::load(std::path::Path::new(brain))?;
        let mut theta = file.theta.clone();
        if arg(&args, "force") == Some("1") {
            let t = file.layout().top.ok_or("no top")?;
            theta[t.b2 + k] += 1e3;
        }
        let net = std::sync::Arc::new(TwoLayerNet::new(file.shape.clone(), theta)?);
        let arbitration = cubarium_voxel_fauna::FaunaConfig::default().founder(f).brain;
        Some(EpisodeDriver::two_layer(net, arbitration, f))
    };
    let cfg = mode_scenes::Config::default();
    let hz = f64::from(cubarium_voxel_fauna::TICK_HZ);
    let (mut perched_all, mut bodies_all) = (0usize, 0usize);
    for &seed in &seeds {
        let s = StagedScene { config: cfg.clone(), scene, seed, driver: driver.clone(), dry: driver.is_some(), tap: true };
        let mut e = s.open()?;
        while SceneCopy::step(&mut e) {}
        let tap = e.sim.fauna_mut().take_tap().ok_or("tap")?;
        let v = e.sim.fauna().view();
        let mut line = format!("seed {seed}:");
        for id in &e.focal {
            let decisions = tap.decisions(*id).unwrap_or(&[]);
            let labels: std::collections::HashMap<u64, u8> = tap
                .teacher_labels(*id)
                .map(|l| l.iter().map(|x| (x.tick, x.proposal)).collect())
                .unwrap_or_default();
            let teach = |d: &cubarium_voxel_fauna::TapDecision| -> u8 { labels.get(&d.tick).copied().unwrap_or(d.proposal) };
            let t_first = decisions.iter().find(|d| usize::from(teach(d)) == k).map(|d| d.tick as f64 / hz);
            let b_first = decisions.iter().find(|d| usize::from(d.held) == k).map(|d| d.tick as f64 / hz);
            let t_n = decisions.iter().filter(|d| usize::from(teach(d)) == k).count();
            let both = decisions.iter().filter(|d| usize::from(teach(d)) == k && usize::from(d.held) == k).count();
            let mut held: std::collections::BTreeMap<&str, usize> = Default::default();
            for d in decisions {
                *held.entry(menu.name(usize::from(d.held))).or_insert(0) += 1;
            }
            let a = v.animal(*id);
            let perched = a.is_some_and(|a| a.mobility.mode == MobilityMode::Perched && matches!(a.mobility.perch.map(|p| p.anchor), Some(PerchAnchor::Stand { .. })));
            let mode = a.map(|a| format!("{:?}", a.mobility.mode)).unwrap_or("gone".into());
            perched_all += usize::from(perched);
            bodies_all += 1;
            line += &format!(
                "\n  id {id}: teacher first {} at {:?}s, brain first holds it at {:?}s; brain held it on {both}/{t_n} teacher-{} samples; held {held:?}; end {mode} perched_in_crown={perched}",
                scene.intent(),
                t_first,
                b_first,
                scene.intent(),
            );
        }
        println!("{line}");
    }
    println!("perched in a crown at the end: {perched_all}/{bodies_all}");
    Ok(())
}
