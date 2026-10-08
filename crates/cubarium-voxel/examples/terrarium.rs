//! What a terrarium build made, per seed: `cargo run -p cubarium-voxel --example terrarium
//! -- <terrarium|terrarium-small|config.toml> <seed>...` (`DIMS=WxHxD` for another size; a
//! config file's `[world]` table, landform overrides included). Development diagnostics only.

use cubarium_voxel::{Config, Landform, Terrarium, World, terrarium, tidy};

fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "terrarium-small".into());
    let file = name.ends_with(".toml").then(|| {
        cubarium_voxel::world_config_from_file(std::path::Path::new(&name)).expect("a config file's [world]")
    });
    let t = match &file {
        Some(Config { landform: Landform::Terrarium(t), .. }) => *t,
        Some(_) => panic!("{name} is not a terrarium"),
        None => Terrarium::preset(&name).expect("terrarium, terrarium-small or a config.toml"),
    };
    // `DIMS=WxHxD` builds the preset on another world size (the scenario maps' sizing).
    let (width, height, depth) = match std::env::var("DIMS") {
        Err(_) if file.is_some() => file.as_ref().map(|c| (c.width, c.height, c.depth)).unwrap(),
        Ok(d) => {
            let v: Vec<u32> = d.split('x').map(|n| n.parse().expect("DIMS=WxHxD")).collect();
            (v[0], v[1], v[2])
        }
        Err(_) if name == "terrarium" => (256, 128, 48),
        Err(_) => (160, 72, 24),
    };
    let seeds: Vec<u64> = args.map(|s| s.parse().expect("a seed")).collect();
    for seed in if seeds.is_empty() { vec![1, 2, 3] } else { seeds } {
        let config = Config {
            width,
            height,
            depth,
            voxel_m: 0.125,
            seed,
            landform: Landform::Terrarium(t),
            ..Config::default()
        };
        let mut world = World::empty(config.clone());
        let report = terrarium::build(&mut world, &t);
        let (seen, all) = terrarium::floor_visibility(&world);
        let thin = tidy::count(&config, world.view().material);
        println!("{name} seed {seed}: {report:?}");
        if std::env::var_os("THIN").is_some() {
            for (x, y, z, kind) in tidy::list(&config, world.view().material) {
                println!("  {kind} at ({x}, {y}, {z})");
            }
        }
        // The lake's floor columns around the ring: does it cross the seam?
        let xs: Vec<u32> = world.lake_drain().iter().map(|&(x, _, _)| x).collect();
        let seam = xs.contains(&0) || xs.contains(&(width - 1));
        println!(
            "  lake columns {}..={} of {width}{}",
            xs.iter().min().unwrap_or(&0),
            xs.iter().max().unwrap_or(&0),
            if seam { " (across the seam)" } else { "" }
        );
        println!(
            "  floors seen {seen}/{all} ({:.0} %), thin left {thin:?}, outlet {:?}, spring {:?}",
            100.0 * seen as f64 / all.max(1) as f64,
            world.outlet_cell(),
            world.spring_cell()
        );
    }
}
