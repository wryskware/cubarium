//! **Trace lanternjaws in a training world** (BRAIN-FIX-2 step 2): the eating latch, the
//! stomach against its start and full lines, the patrol mode, the distance walked and the
//! teacher's label, once per `every_s` seconds per body. A diagnostic, not a study.
//!
//! ```text
//! lj_trace seed=1 fullness=1.0,0.2 seconds=600 every_s=10
//! ```
//!
//! Each `fullness` entry tops up one more grown lanternjaw on its heuristic at that fraction
//! of its stores (`world::top_up`; `none` for a placed body's default).

use cubarium_search::es::voxel::live::{ecology::EcologyParams, world};
use cubarium_voxel_fauna::{intent, Founder};

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seed: u64 = arg(&args, "seed").unwrap_or("1").parse().map_err(|e| format!("seed: {e}"))?;
    let seconds: u64 = arg(&args, "seconds").unwrap_or("600").parse().map_err(|e| format!("seconds: {e}"))?;
    let every: u64 = arg(&args, "every_s").unwrap_or("10").parse().map_err(|e| format!("every_s: {e}"))?;
    let fulls: Vec<Option<f64>> = arg(&args, "fullness").unwrap_or("1.0,0.2").split(',')
        .map(|s| if s.trim() == "none" { Ok(None) } else { s.trim().parse().map(Some).map_err(|e| format!("fullness: {e}")) })
        .collect::<Result<_, _>>()?;
    let f = Founder::Lanternjaw;
    let eco = EcologyParams::defaults();
    let (mut sim, info) = world::found_world_redrawing(&world::training_config(), seed,
        eco.fauna().founder_counts(), &eco, |n| eprintln!("# {n}"))?;
    let mut ids = Vec::new();
    for (k, full) in fulls.iter().enumerate() {
        world::top_up(&mut sim, world::TopUp { founder: f, bodies: k + 1, fullness: *full });
        let now: Vec<u64> = sim.fauna().view().animals.iter().filter(|a| a.founder == Some(f)).map(|a| a.id).collect();
        if let Some(id) = now.iter().find(|id| !ids.iter().any(|(i, _)| i == *id)) {
            ids.push((*id, *full));
        }
    }
    println!("# world seed {} (asked {seed}); bodies {:?}", info.seed, ids);
    let gut = sim.fauna().config().founder(f).gut;
    println!("# gut s0 {:.3} full_tolerance {:.3} refill_gap {:.3}", gut.s0, gut.full_tolerance, gut.refill_gap);
    println!("t_s\tid\tfull0\teating\tstomach/K\tstate\tmode\tmoved_m\tlabel");
    sim.fauna_mut().start_tap();
    let menu = intent::menu_of(f);
    let pos = |sim: &cubarium_voxel_sim::Sim, id: u64| sim.fauna().view().animals.iter()
        .find(|a| a.id == id).map(|a| (a.pose.x, a.pose.z));
    let mut last: Vec<Option<(f64, f64)>> = ids.iter().map(|(id, _)| pos(&sim, *id)).collect();
    let mut counts = std::collections::BTreeMap::<(u64, &str), u64>::new();
    for s in 1..=seconds {
        for _ in 0..cubarium_voxel_fauna::TICK_HZ as u64 {
            sim.step();
        }
        if s % every != 0 {
            continue;
        }
        for (k, (id, full)) in ids.iter().enumerate() {
            let v = sim.fauna().view();
            let Some(a) = v.animals.iter().find(|a| a.id == *id) else {
                println!("{s}\t{id}\t{full:?}\tgone");
                continue;
            };
            let k_cap = gut.capacity(a.body);
            let now = (a.pose.x, a.pose.z);
            let moved = last[k].map_or(0.0, |(x, z)| ((now.0 - x).powi(2) + (now.1 - z).powi(2)).sqrt());
            last[k] = Some(now);
            let label = sim.fauna().tap().and_then(|t| t.decisions(*id)).and_then(|d| d.last())
                .and_then(|d| menu.named.get(d.proposal as usize)).map_or("-", |c| c.name);
            *counts.entry((*id, label)).or_default() += 1;
            println!("{s}\t{id}\t{full:?}\t{}\t{:.3}\t{:?}\t{:?}\t{moved:.2}\t{label}",
                a.eating, a.stomach.organic() / k_cap.max(1e-12), a.state, a.hunt.mode);
        }
    }
    // Every decision sample's proposal over the whole trace, per body.
    let tap = sim.fauna_mut().take_tap().ok_or("tap")?;
    for (id, full) in &ids {
        let mut by = std::collections::BTreeMap::<&str, u64>::new();
        for d in tap.decisions(*id).unwrap_or(&[]) {
            *by.entry(menu.named.get(d.proposal as usize).map_or("?", |c| c.name)).or_default() += 1;
        }
        println!("# body {id} fullness {full:?}: decision labels {by:?}");
    }
    Ok(())
}
