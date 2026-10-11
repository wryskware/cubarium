//! CAPGNAWER-CAPS probe: load a staged checkpoint and watch the capgnawers feed for a few
//! sim-minutes. Prints the lineage's loaded diet numbers, the litter offer its taste gate
//! needs, the litter under capgnawers, which food each bite takes, the reserve fill, and
//! the distance to the nearest glowcap holding caps.
//!
//! cargo run --release -p cubarium --example capgnawer_probe -- <ckpt dir> [minutes] [threads]
use cubarium_voxel_fauna::{Food, Founder, TICK_HZ};
use cubarium_voxel_flora::{Site, Species};
use cubarium_voxel_sim::{Sim, SimConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&args[1]);
    let minutes: u64 = args.get(2).map_or(10, |a| a.parse().expect("minutes"));
    let threads: usize = args.get(3).map_or(4, |a| a.parse().expect("threads"));
    cubarium_voxel::set_thread_override(threads);
    let saved = cubarium_voxel_sim::saved::load(dir, &[]).expect("load");
    let minute0 = saved.minute;
    let mut sim = Sim::ambient(saved.world, saved.flora, saved.fauna, SimConfig::experiment());
    let g = Founder::Capgnawer;
    let phys = *sim.fauna().config().founder(g);
    let voxel_m = sim.world().config().voxel_m;
    let width = i64::from(sim.world().config().width);
    let reference = phys.taste_reference_on(
        sim.fauna().config().cue.emit_reference,
        voxel_m,
        sim.flora().config(),
    );
    let worth = phys.food_worth();
    let teachers = sim.fauna().config().teachers;
    let gate = teachers.feed_gate;
    // The stock-weighed taste of a49cb742 (worth x stock against the reference, sat 1) passes
    // the gate when c/(c+1) > gate: litter stock over gate/(1-gate) x reference / worth.
    let litter_need = gate / (1.0 - gate) * reference / worth[Food::Litter.index()];
    println!(
        "loaded minute {minute0}; capgnawer yield litter {} caps {}; taste_by_yield {}; worth litter {:.4} caps {:.4}; \
         taste reference {reference:.4}; feed gate {gate}; stay worth {}; litter stock the stock-weighed taste needed {litter_need:.4}; n_tissue {}",
        phys.yield_for(Food::Litter),
        phys.yield_for(Food::CapTissue),
        phys.taste_by_yield,
        worth[Food::Litter.index()],
        worth[Food::CapTissue.index()],
        teachers.capgnawer_stay_worth,
        phys.core.n_tissue,
    );
    let ticks = minutes * 60 * TICK_HZ as u64;
    let mut bites = [0u64; Food::COUNT];
    let mut eaten = [0.0f64; Food::COUNT];
    let mut litter_at_bite: Vec<f64> = Vec::new();
    let mut litter_under: Vec<f64> = Vec::new();
    let mut dist: Vec<f64> = Vec::new();
    let mut fill: Vec<f64> = Vec::new();
    let mut funded: Vec<f64> = Vec::new();
    let mut eating_samples = (0u64, 0u64);
    let mut caps: Vec<Site> = Vec::new();
    for t in 0..ticks {
        if t % (60 * TICK_HZ as u64) == 0 {
            let fv = sim.flora().view();
            caps = fv
                .stands
                .iter()
                .filter(|s| s.species == Species::Glowcap && fv.edible_foliage(s) > 0.0)
                .map(|s| s.site)
                .collect();
        }
        let before: Vec<(u64, [f64; Food::COUNT])> = sim
            .fauna()
            .view()
            .animals
            .iter()
            .filter(|a| a.founder == Some(g))
            .map(|a| (a.id, a.stomach.pools.map(|p| p.organic)))
            .collect();
        sim.step();
        let sample = t % (TICK_HZ as u64 * 10) == 0;
        let fv = sim.flora().view();
        for a in sim.fauna().view().animals.iter().filter(|a| a.founder == Some(g)) {
            let Some((_, pre)) = before.iter().find(|(id, _)| *id == a.id) else { continue };
            let ground = fv.ground_at(a.site);
            let litter = ground.map_or(0.0, |g| g.litter);
            // The 3x3 neighbourhood's richest litter (the mouth may reach the next column).
            let mut near = litter;
            for dx in -1i64..=1 {
                for dz in -1i64..=1 {
                    let x = (i64::from(a.site.x) + dx).rem_euclid(width) as u32;
                    let z = i64::from(a.site.z) + dz;
                    if z < 0 {
                        continue;
                    }
                    if let Some(g) = fv.ground_at(Site { x, y: a.site.y, z: z as u32 }) {
                        near = near.max(g.litter);
                    }
                }
            }
            for f in Food::ALL {
                let d = a.stomach.pools[f.index()].organic - pre[f.index()];
                if d > 1e-12 {
                    bites[f.index()] += 1;
                    eaten[f.index()] += d;
                    if f == Food::Litter {
                        litter_at_bite.push(near);
                        if let Some(g) = ground {
                            if g.litter > 0.0 {
                                funded.push(g.litter_mineral / (phys.core.n_tissue * g.litter));
                            }
                        }
                    }
                }
            }
            if sample {
                litter_under.push(near);
                eating_samples.1 += 1;
                if a.eating {
                    eating_samples.0 += 1;
                }
                let cap = phys.core.reserve_of(a.body);
                if cap > 0.0 {
                    fill.push(a.reserve / cap);
                }
                let d = caps
                    .iter()
                    .map(|c| {
                        let mut dx = (i64::from(c.x) - i64::from(a.site.x)).abs();
                        dx = dx.min(width - dx);
                        let dy = i64::from(c.y) - i64::from(a.site.y);
                        let dz = i64::from(c.z) - i64::from(a.site.z);
                        (((dx * dx + dy * dy + dz * dz) as f64).sqrt()) * voxel_m
                    })
                    .fold(f64::INFINITY, f64::min);
                dist.push(d);
            }
        }
    }
    let q = |v: &mut Vec<f64>| -> String {
        if v.is_empty() {
            return "none".into();
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
        format!("n {} p10 {:.3} p50 {:.3} p90 {:.3} max {:.3}", v.len(), p(0.1), p(0.5), p(0.9), v[v.len() - 1])
    };
    let above = |v: &[f64]| v.iter().filter(|&&x| x > litter_need).count() as f64 / v.len().max(1) as f64;
    let fv = sim.flora().view();
    let mut cap_stock: Vec<f64> = fv
        .stands
        .iter()
        .filter(|s| s.species == Species::Glowcap)
        .map(|s| fv.edible_foliage(s))
        .collect();
    println!("glowcaps holding caps: {}; bite_half_stock {}", caps.len(), phys.core.bite_half_stock);
    println!("edible caps per glowcap (all glowcaps): {}", q(&mut cap_stock));
    println!(
        "bites (ticks with a stomach gain): litter {} caps {}; raw eaten litter {:.4} caps {:.4}",
        bites[Food::Litter.index()],
        bites[Food::CapTissue.index()],
        eaten[Food::Litter.index()],
        eaten[Food::CapTissue.index()]
    );
    println!("eating latch on: {} of {} samples", eating_samples.0, eating_samples.1);
    println!("share of capgnawer samples over litter the stock-weighed taste held them on: {:.3}", above(&litter_under));
    println!("litter (3x3 max) under capgnawers: {}", q(&mut litter_under));
    println!("litter (3x3 max) at litter bites: {}", q(&mut litter_at_bite));
    println!("litter funded yield (mineral/(n_tissue*organic)) at bites: {}", q(&mut funded));
    println!("reserve / reserve cap: {}", q(&mut fill));
    println!("distance to nearest capped glowcap, m: {}", q(&mut dist));
}
