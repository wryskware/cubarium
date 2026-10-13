//! **DEN-REACH: how many grown Veilhounds can walk home** (the `two_layer_clone` world,
//! stepped under the heuristics). Every `every_s` seconds, per seed: the pack's grown
//! members, how many stand on the route graph, how many of those share their den's walking
//! component, and how many know a way home (`den_way`, the `Reach.den` sense). The stranded
//! count is the members on the graph outside the den's component.
//!
//! ```text
//! den_reach_trace founder=chorister seeds=9 seconds=600 bodies=8 fullness=0.3 every_s=30 [share=F]
//! ```
//! `share=` sets `pack.den_reach_share` (0: the breeder's reach alone, the rule before DEN-REACH).

use cubarium_search::es::voxel::live::{ecology::EcologyParams, world};

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    cubarium_voxel::world::set_thread_override(1);
    let f = cubarium_search::es::voxel::parse_founder(arg(&args, "founder").unwrap_or("chorister"))?;
    let seeds: Vec<u64> = arg(&args, "seeds").unwrap_or("9").split(',').filter_map(|s| s.parse().ok()).collect();
    let seconds: u64 = arg(&args, "seconds").and_then(|v| v.parse().ok()).unwrap_or(600);
    let bodies: usize = arg(&args, "bodies").and_then(|v| v.parse().ok()).unwrap_or(8);
    let fullness: Option<f64> = arg(&args, "fullness").and_then(|v| v.parse().ok());
    let every: u64 = arg(&args, "every_s").and_then(|v| v.parse().ok()).unwrap_or(30).max(1);
    let map_at: Option<u64> = arg(&args, "map_at_s").and_then(|v| v.parse().ok());
    let share: Option<f64> = arg(&args, "share").and_then(|v| v.parse().ok());
    let per_s = u64::from(cubarium_voxel_fauna::TICK_HZ);
    for seed in seeds {
        let mut eco = EcologyParams::defaults();
        if let Some(share) = share {
            // The shipped animals and plants, read whole, with the one knob set.
            let mut p = eco.params()?;
            if let Some(t) = p.fauna.founders[f.index()].pack.as_mut() {
                t.den_reach_share = share;
            }
            eco = EcologyParams::Resolved { flora: Box::new(p.flora), fauna: Box::new(p.fauna) };
        }
        let mut sim = match world::try_found_world(&world::training_config(), seed, eco.fauna().founder_counts(), &eco) {
            Ok((sim, _)) => sim,
            Err(why) => {
                eprintln!("# skipped seed {seed}: {why}");
                continue;
            }
        };
        world::top_up(&mut sim, world::TopUp { founder: f, bodies, fullness });
        let (mut sum_stranded, mut sum_no_way, mut samples) = (0usize, 0usize, 0usize);
        let moves = arg(&args, "moves").is_some_and(|v| v == "1");
        let mut last: std::collections::HashMap<u64, (Option<usize>, String)> = Default::default();
        for tick in 1..=seconds * per_s {
            sim.step();
            if moves {
                for (id, k, site, (x, z), mob) in cubarium_voxel_fauna::den_reach_member_components(sim.fauna(), &sim.world().view()) {
                    let now = format!("site=({},{},{}) pose=({x:.2},{z:.2}) {mob}", site.x, site.y, site.z);
                    if let Some((was, from)) = last.get(&id) {
                        if *was != k {
                            println!("seed={seed} tick={tick} MOVE member={id} k {was:?} -> {k:?} from {from} to {now}");
                        }
                    }
                    last.insert(id, (k, now));
                }
            }
            if tick % (every * per_s) != 0 {
                continue;
            }
            let fauna = sim.fauna();
            let census: Vec<_> = cubarium_voxel_fauna::den_reach_census(fauna, &sim.world().view())
                .into_iter()
                .filter(|(p, ..)| fauna.pack(*p).is_some_and(|r| r.lineage == f))
                .collect();
            let on = census.iter().filter(|c| c.2).count();
            let with_den = census.iter().filter(|c| c.3).count();
            let way = census.iter().filter(|c| c.4).count();
            let dens: Vec<String> = fauna.packs().iter().filter(|r| r.lineage == f).map(|r| format!("({},{},{})", r.den.x, r.den.y, r.den.z)).collect();
            println!(
                "seed={seed} t={}s grown={} on_graph={on} with_den={with_den} stranded={} way_home={way} no_way={} dens={}",
                tick / per_s,
                census.len(),
                on - with_den,
                census.len() - way,
                dens.join(",")
            );
            for line in cubarium_voxel_fauna::den_reach_components(fauna, &sim.world().view()) {
                println!("seed={seed} t={}s   {line}", tick / per_s);
            }
            if map_at.is_some_and(|m| m == tick / per_s) {
                for line in cubarium_voxel_fauna::den_reach_map(fauna, &sim.world().view(), 12) {
                    println!("seed={seed} t={}s MAP {line}", tick / per_s);
                }
            }
            sum_stranded += on - with_den;
            sum_no_way += census.len() - way;
            samples += 1;
        }
        println!("seed={seed} SUMMARY samples={samples} stranded_member_samples={sum_stranded} no_way_member_samples={sum_no_way}");
    }
    Ok(())
}
