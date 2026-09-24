//! What a terrarium build made, per seed: `cargo run -p cubarium-voxel --example terrarium
//! -- <terrarium|terrarium-small> <seed>...`. Development diagnostics only.

use cubarium_voxel::{Config, Landform, Terrarium, World, terrarium, tidy};

fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "terrarium-small".into());
    let t = Terrarium::preset(&name).expect("terrarium or terrarium-small");
    let (width, height, depth) = if name == "terrarium" { (256, 128, 48) } else { (160, 72, 24) };
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
        println!(
            "  floors seen {seen}/{all} ({:.0} %), thin left {thin:?}, outlet {:?}, spring {:?}",
            100.0 * seen as f64 / all.max(1) as f64,
            world.outlet_cell(),
            world.spring_cell()
        );
    }
}
