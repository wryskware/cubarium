//! **ROUTE-WALK: why a column has no route-graph face** (the `two_layer_clone` world, as
//! `den_reach_trace` founds it). After `at_s` seconds, for each column `x0..=x1`,
//! `z0..=z1`: each support face's tests in `RouteGraph::for_founder` (`face_tests`), and
//! whether the live walking graph (`FaunaConfig::walking_graph`) and the founding's graph
//! hold it. Read only.
//!
//! ```text
//! route_face_probe founder=chorister seed=9 bodies=8 fullness=0.3 at_s=60 x=5..12 z=0..3 y=8..20
//! ```
//!
//! `bench=K config=HOST.toml seed=N`: instead, the host config's `[world]` generated at seed
//! `N` (terrain only) and the lineage's founding and walking route graphs each built `K`
//! times, timed.

use cubarium_search::es::voxel::live::{ecology::EcologyParams, world};

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn range(s: Option<&str>, d: (u32, u32)) -> (u32, u32) {
    s.and_then(|s| s.split_once("..")).and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).unwrap_or(d)
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    cubarium_voxel::world::set_thread_override(1);
    let f = cubarium_search::es::voxel::parse_founder(arg(&args, "founder").unwrap_or("chorister"))?;
    let seed: u64 = arg(&args, "seed").and_then(|v| v.parse().ok()).unwrap_or(9);
    let bodies: usize = arg(&args, "bodies").and_then(|v| v.parse().ok()).unwrap_or(8);
    let fullness: Option<f64> = arg(&args, "fullness").and_then(|v| v.parse().ok());
    let at_s: u64 = arg(&args, "at_s").and_then(|v| v.parse().ok()).unwrap_or(0);
    let (x0, x1) = range(arg(&args, "x"), (5, 12));
    let (z0, z1) = range(arg(&args, "z"), (0, 3));
    let (y0, y1) = range(arg(&args, "y"), (0, 40));
    if let Some(reps) = arg(&args, "bench").and_then(|v| v.parse::<u32>().ok()) {
        let path = arg(&args, "config").ok_or("bench= wants config=")?;
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let table: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
        let mut w = table.get("world").and_then(|w| w.as_table()).cloned().ok_or("no [world]")?;
        // The landform by its preset name (the host's own loader resolves the rest).
        let preset = w.remove("landform").and_then(|l| l.get("preset").and_then(|p| p.as_str()).map(str::to_string));
        let mut cfg: cubarium_voxel::Config = toml::Value::Table(w).try_into().map_err(|e| format!("{e}"))?;
        if let Some(t) = preset.as_deref().and_then(cubarium_voxel::terrarium::Terrarium::preset) {
            cfg.landform = cubarium_voxel::Landform::Terrarium(t);
        }
        cfg.seed = seed;
        let world = cubarium_voxel::World::new(cfg);
        let fauna = cubarium_voxel_fauna::FaunaConfig::default();
        let (phys, margin) = (fauna.founder(f), fauna.founding.clearance_margin_m);
        let c = world.config();
        for (name, walking) in [("founding", false), ("walking", true)] {
            let mut ms = Vec::new();
            let mut faces = 0;
            for _ in 0..reps.max(1) {
                let t = std::time::Instant::now();
                let g = if walking {
                    fauna.walking_graph(&world.view(), phys)
                } else {
                    cubarium_voxel_fauna::RouteGraph::for_founder(&world.view(), phys, margin)
                };
                ms.push(t.elapsed().as_secs_f64() * 1e3);
                faces = g.faces.len();
            }
            ms.sort_by(f64::total_cmp);
            println!("bench {name} {}x{}x{} seed={seed} faces={faces} reps={} min={:.1} ms median={:.1} ms", c.width, c.height, c.depth, ms.len(), ms[0], ms[ms.len() / 2]);
        }
        return Ok(());
    }
    let eco = EcologyParams::defaults();
    let (mut sim, _) = world::try_found_world(&world::training_config(), seed, eco.fauna().founder_counts(), &eco)?;
    world::top_up(&mut sim, world::TopUp { founder: f, bodies, fullness });
    for _ in 0..at_s * u64::from(cubarium_voxel_fauna::TICK_HZ) {
        sim.step();
    }
    let view = sim.world().view();
    let cfg = sim.fauna().config();
    let phys = cfg.founder(f);
    let margin = cfg.founding.clearance_margin_m;
    let t = std::time::Instant::now();
    let graph = cfg.walking_graph(&view, phys);
    println!("# walking graph build {:.1} ms, {} faces", t.elapsed().as_secs_f64() * 1e3, graph.faces.len());
    let founding = cubarium_voxel_fauna::RouteGraph::for_founder(&view, phys, margin);
    for z in z0..=z1 {
        for x in x0..=x1 {
            for line in cubarium_voxel_fauna::RouteGraph::face_tests(&view, phys, margin, x, z, y0..=y1) {
                let y: u32 = line.split(',').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                let s = cubarium_voxel_flora::Site { x, y, z };
                let node = graph.face(s);
                println!("{line} walking_node={:?} k={:?} founding_node={:?}", node.is_some(), node.map(|i| graph.components[i]), founding.face(s).is_some());
            }
        }
    }
    Ok(())
}
