//! **What a lineage's bodies do under each teacher label in a live training world**
//! (CLONE-AGREE step 2): the `two_layer_clone` world (`small`, still water, default founders,
//! the lineage topped up), stepped under the heuristics; every whole second, per body of the
//! lineage, its proposal (the top's label), the intent it holds, its steering layer, whether it
//! holds still, its state, its rest bout, its eating latch and emergency. Then a count of each
//! combination, grouped by proposal.
//!
//! ```text
//! label_trace founder=loftstrider seeds=9 seconds=600 bodies=8 [fullness=F] \
//!     [also=LINEAGE=N[:FULLNESS],...] [lines=1]
//! ```

use cubarium_search::es::voxel::live::{ecology::EcologyParams, world};
use std::collections::BTreeMap;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    cubarium_voxel::world::set_thread_override(1);
    let f = cubarium_search::es::voxel::parse_founder(arg(&args, "founder").unwrap_or("loftstrider"))?;
    let seeds: Vec<u64> = arg(&args, "seeds").unwrap_or("9").split(',').filter_map(|s| s.parse().ok()).collect();
    let seconds: u64 = arg(&args, "seconds").and_then(|v| v.parse().ok()).unwrap_or(600);
    let bodies: usize = arg(&args, "bodies").and_then(|v| v.parse().ok()).unwrap_or(8);
    let fullness: Option<f64> = arg(&args, "fullness").map(|v| v.parse().expect("fullness: a fraction"));
    let also: Vec<world::TopUp> = arg(&args, "also")
        .map(|v| v.split(',').filter(|x| !x.is_empty()).map(|x| world::TopUp::parse(x).expect("also")).collect())
        .unwrap_or_default();
    let lines = arg(&args, "lines").is_some();
    let menu = f.manifest().menu;
    let per_s = u64::from(cubarium_voxel_fauna::TICK_HZ);
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    for seed in seeds {
        let eco = EcologyParams::defaults();
        let (mut sim, _) = world::try_found_world(&world::training_config(), seed, eco.fauna().founder_counts(), &eco)?;
        for t in &also {
            world::top_up(&mut sim, *t);
        }
        world::top_up(&mut sim, world::TopUp { founder: f, bodies, fullness });
        let stay = sim.fauna().config().teachers.forage_stay_forward;
        for s in 0..seconds {
            for _ in 0..per_s {
                sim.step();
            }
            let fauna = sim.fauna();
            for a in fauna.view().animals.iter().filter(|a| a.founder == Some(f)) {
                let proposal = menu.name(usize::from(a.intent.proposal)).to_string();
                let fwd = a.founder_state.held.forward;
                let key = format!(
                    "held={} steer={:?} still={} state={:?} bout={} eating={} emergency={}",
                    menu.name(usize::from(a.intent.held)),
                    fauna.steer_of(a.id),
                    fwd <= stay,
                    a.state,
                    a.rest_ticks > 0,
                    a.eating,
                    a.emergency
                );
                if lines {
                    println!("seed={seed} s={s} id={} fwd={fwd:.2} proposal={proposal} {key}", a.id);
                }
                *totals.entry(proposal.clone()).or_default() += 1;
                *counts.entry((proposal, key)).or_default() += 1;
            }
        }
    }
    for (p, n) in &totals {
        println!("== {p} {n}");
        let mut rows: Vec<_> = counts.iter().filter(|((q, _), _)| q == p).map(|((_, k), n)| (k, *n)).collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1));
        for (k, n) in rows.into_iter().take(12) {
            println!("{n:6} {k}");
        }
    }
    Ok(())
}
