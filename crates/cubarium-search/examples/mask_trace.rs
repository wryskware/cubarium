//! **Why a teacher label is masked** (LS-TEACH step 2): the `two_layer_clone` world stepped
//! under the heuristics with the tap on; every decision sample of the lineage whose proposal
//! (the top's label) its own intent mask rules out, counted by its steering layer, whether
//! it is a fresh label or one carried from the sample before, and what its percept record
//! holds of threats after the sample (tracks of the threat class: how many, the freshest's
//! age, whether sight carries it) and of its ground response (fleeing, the alarm it heeds).
//! The valid labels of the same intents are counted beside them.
//!
//! ```text
//! mask_trace founder=loftstrider seeds=9 seconds=600 bodies=8 [also=LINEAGE=N[:FULLNESS],...] \
//!     [scenario=PATH] [intents=watch,guard] [lines=1]
//! ```

use cubarium_search::es::voxel::live::{ecology::EcologyParams, world};
use cubarium_voxel_fauna::perception::class;
use std::collections::{BTreeMap, HashMap};

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
    let also: Vec<world::TopUp> = arg(&args, "also")
        .map(|v| v.split(',').filter(|x| !x.is_empty()).map(|x| world::TopUp::parse(x).expect("also")).collect())
        .unwrap_or_default();
    let scenario = arg(&args, "scenario").map(|p| cubarium_voxel_sim::edit::Scenario::read(std::path::Path::new(p))).transpose()?;
    let lines = arg(&args, "lines").is_some();
    let menu = f.manifest().menu;
    let want: Vec<usize> = arg(&args, "intents")
        .unwrap_or("watch")
        .split(',')
        .map(|n| menu.index(n).ok_or_else(|| format!("no intent {n}")))
        .collect::<Result<_, _>>()?;
    let per_s = u64::from(cubarium_voxel_fauna::TICK_HZ);
    let stagger = 2 * cubarium_voxel_fauna::FaunaConfig::default().perception.stagger_ticks();
    let mut counts: BTreeMap<(String, bool, String), usize> = BTreeMap::new();
    for seed in seeds {
        let eco = EcologyParams::defaults();
        let (mut sim, _) = world::try_found_world(&world::training_config(), seed, eco.fauna().founder_counts(), &eco)?;
        for t in &also {
            world::top_up(&mut sim, *t);
        }
        world::top_up(&mut sim, world::TopUp { founder: f, bodies, fullness: None });
        if let Some(s) = &scenario {
            let r = sim.edit(|e| s.apply(e));
            eprintln!("# seed {seed}: scenario {:?} refused {:?}", r.applied, r.refused);
        }
        sim.fauna_mut().start_tap();
        // (id, tick) -> what its record holds after the sample.
        let mut seen: HashMap<(u64, u64), String> = HashMap::new();
        for _ in 0..seconds * per_s {
            sim.step();
            let fauna = sim.fauna();
            let now = fauna.tick();
            for a in fauna.view().animals.iter().filter(|a| a.founder == Some(f) && a.intent.sample_tick == now) {
                let p = fauna.percept(a.id);
                let threats: Vec<_> = p.map(|p| p.tracks().iter().filter(|t| t.known && t.class == class::THREAT).copied().collect()).unwrap_or_default();
                let age = threats.iter().map(|t| now.saturating_sub(t.tick)).min();
                let age_band = match age {
                    None => "none".to_string(),
                    Some(a) if a <= stagger => "fresh".to_string(),
                    Some(a) => format!("{}s", (a as f64 / per_s as f64).ceil()),
                };
                let sight = threats.iter().any(|t| t.modes & cubarium_voxel_fauna::perception::SIGHT != 0);
                let v = p.map(|p| p.vigil).unwrap_or_default();
                seen.insert(
                    (a.id, now),
                    format!("threat_tracks={} freshest={age_band} sight={sight} fleeing={} heard={}", threats.len(), v.fleeing, v.heard_from != 0),
                );
            }
        }
        let tap = sim.fauna_mut().take_tap().ok_or("tap")?;
        for (id, g, samples) in tap.streams() {
            if g != f {
                continue;
            }
            let steer: HashMap<u64, _> = samples.iter().map(|s| (s.tick, s.steer)).collect();
            let Some(ds) = tap.decisions(id) else { continue };
            let mut last: Option<u8> = None;
            for d in ds {
                let k = usize::from(d.proposal);
                let carried = last == Some(d.proposal);
                last = Some(d.proposal);
                if !want.contains(&k) {
                    continue;
                }
                let masked = !d.prep.intent_mask.get(k).copied().unwrap_or(false);
                let key = format!(
                    "steer={:?} {} held_before={} {}",
                    steer.get(&d.tick),
                    if carried { "carried" } else { "fresh" },
                    menu.name(usize::from(d.held_before)),
                    seen.get(&(id, d.tick)).map_or("?", String::as_str)
                );
                if lines && masked {
                    println!("seed={seed} id={id} tick={} {} masked {key}", d.tick, menu.name(k));
                }
                *counts.entry((menu.name(k).to_string(), masked, key)).or_default() += 1;
            }
        }
    }
    for &k in &want {
        let name = menu.name(k);
        for masked in [true, false] {
            let mut rows: Vec<_> = counts.iter().filter(|((n, m, _), _)| n == name && *m == masked).map(|((_, _, key), n)| (key, *n)).collect();
            let total: usize = rows.iter().map(|r| r.1).sum();
            println!("== {name} {} {total}", if masked { "MASKED" } else { "valid" });
            rows.sort_by(|a, b| b.1.cmp(&a.1));
            for (key, n) in rows.into_iter().take(14) {
                println!("{n:6} {key}");
            }
        }
    }
    Ok(())
}
