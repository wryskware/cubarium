//! **Trace one staged mode scene under its heuristics** (BRAIN-FIX step 1): one line per
//! focal body every `every_s` seconds — its state, steer source, held intent and proposal,
//! mobility, eating latch, stomach, rest timers and the scene's own distance (to the host,
//! the mother, the nearest hound, or the carcass's carrion left). Evidence for whether a
//! scene gives its mode room to finish and whether its competence test asks for what the
//! heuristic does.
//!
//! ```text
//! mode_scene_trace scene=groom seed=101 every_s=2 [duration_s=60]
//! ```

use cubarium_voxel_fauna::{Animal, menu_of};
use cubarium_voxel_sim::mode_scenes::{self, ModeScene};

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let scene = ModeScene::parse(arg(&args, "scene").ok_or("scene=")?).ok_or("unknown scene")?;
    let seed: u64 = arg(&args, "seed").and_then(|s| s.parse().ok()).unwrap_or(101);
    let every_s: u64 = arg(&args, "every_s").and_then(|s| s.parse().ok()).unwrap_or(2);
    let mut cfg = mode_scenes::Config::default();
    if let Some(d) = arg(&args, "duration_s").and_then(|s| s.parse().ok()) {
        cfg.duration_s = d;
    }
    let mut e = mode_scenes::found(&cfg, scene, seed)?;
    let f = scene.founder();
    let menu = menu_of(f);
    let ring = f64::from(cfg.width) * cfg.voxel_m;
    let dist = |a: &Animal, b: &Animal| {
        let dx = (b.pose.x - a.pose.x + ring / 2.0).rem_euclid(ring) - ring / 2.0;
        (dx.hypot(b.pose.z - a.pose.z)).hypot(b.pose.y_m - a.pose.y_m)
    };
    let hz = u64::from(cubarium_voxel_fauna::TICK_HZ);
    println!("scene={} seed={seed} focal={:?} others={:?} site={:?}", scene.name(), e.focal, e.others, e.site);
    loop {
        let t = e.ticks();
        if t % (every_s * hz) == 0 {
            let fauna = e.sim.fauna();
            let v = fauna.view();
            let carrion = e.site.and_then(|s| e.sim.flora().view().ground.iter().find(|g| g.site == s).map(|g| g.carrion)).unwrap_or(0.0);
            for o in &e.others {
                if let Some(a) = v.animal(*o) {
                    println!(
                        "t={:>5.1} other={o} state={:?} steer={:?} held={} mob={:?} eat={} speed={:.2} rest_ticks={} x={:.2} z={:.2}",
                        t as f64 / hz as f64,
                        a.state,
                        fauna.steer_of(a.id),
                        menu_of(a.founder.unwrap_or(f)).name(usize::from(a.intent.held)),
                        a.mobility.mode,
                        a.eating,
                        a.speed_m_s,
                        a.rest_ticks,
                        a.pose.x,
                        a.pose.z,
                    );
                }
            }
            for id in &e.focal {
                let Some(a) = v.animal(*id) else {
                    println!("t={:>5.1} id={id} gone", t as f64 / hz as f64);
                    continue;
                };
                let other = e.others.iter().filter_map(|o| v.animal(*o)).map(|o| dist(a, o)).fold(f64::INFINITY, f64::min);
                let riders = v.animals.iter().filter(|r| r.attached.is_some()).count();
                println!(
                    "t={:>5.1} id={id} state={:?} steer={:?} held={} prop={} mob={:?} eat={} emerg={} stom={:.4} rest_ticks={} rest={} doze_light={} other_d={:.2} carrion={:.4} riders={} y={:.2} hunt={:?}",
                    t as f64 / hz as f64,
                    a.state,
                    fauna.steer_of(a.id),
                    menu.name(usize::from(a.intent.held)),
                    menu.name(usize::from(a.intent.proposal)),
                    a.mobility.mode,
                    a.eating,
                    a.emergency,
                    a.stomach.total().organic,
                    a.rest_ticks,
                    a.rest.is_some(),
                    a.doze.light,
                    other,
                    carrion,
                    riders,
                    a.pose.y_m,
                    a.hunt.mode,
                );
            }
        }
        if !e.step() {
            break;
        }
    }
    Ok(())
}
