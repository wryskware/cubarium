//! How a terrarium's water behaves once it runs: `cargo run --release -p cubarium-voxel
//! --example terrarium_water -- <terrarium|terrarium-small> <seed> <minutes>`. Prints the
//! lake, the water table, the sky store and the tier pools once a simulated minute.
//! Development diagnostics only.

use cubarium_voxel::{Config, Landform, TICK_HZ, Terrarium, World, hydrate, terrarium};

fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "terrarium-small".into());
    let t = Terrarium::preset(&name).expect("terrarium or terrarium-small");
    let seed: u64 = args.next().map_or(1, |s| s.parse().expect("a seed"));
    let minutes: u64 = args.next().map_or(10, |s| s.parse().expect("minutes"));
    let small = name != "terrarium";
    let config = Config {
        width: if small { 160 } else { 256 },
        height: if small { 72 } else { 128 },
        depth: if small { 24 } else { 48 },
        voxel_m: 0.125,
        water_substeps: 4,
        seed,
        landform: Landform::Terrarium(t),
        ..Config::default()
    };
    // The same terrain again, for its report: where the stream runs.
    let course = {
        let mut probe = World::empty(config.clone());
        terrarium::build(&mut probe, &t).river
    };
    let mut world = World::new(config);
    let vm = world.config().voxel_m;
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    for minute in 0..=minutes {
        let lake = hydrate::lake(&world);
        let surface = if lake.surface_cells.is_empty() {
            0.0
        } else {
            lake.surface_cells
                .iter()
                .map(|&i| world.config().coords(i).1 as f64 + world.view().free[i])
                .sum::<f64>()
                / lake.surface_cells.len() as f64
        };
        let pools = hydrate::pools(&world);
        let tiers = hydrate::tier_pools(&pools, lake.level_y);
        let v = world.view();
        let cell = world.config().voxel_volume();
        // The water over the drain: the graded lake itself.
        let drains = world.lake_drain();
        let over_drain = if drains.is_empty() {
            0.0
        } else {
            drains
                .iter()
                .map(|&(x, y, z)| {
                    let mut top = y;
                    while top + 1 < world.config().height && v.free[world.config().index(x as i64, top, z)] >= 0.99 {
                        top += 1;
                    }
                    top as f64 + v.free[world.config().index(x as i64, top, z)]
                })
                .sum::<f64>()
                / drains.len() as f64
        };
        let free: f64 = v.free.iter().sum::<f64>() * cell;
        let in_lake: f64 = lake.cells.iter().map(|&i| v.free[i]).sum::<f64>() * cell;
        let pore: f64 = v
            .material
            .iter()
            .zip(v.pore)
            .map(|(m, p)| m.pore_capacity() * p)
            .sum::<f64>()
            * cell;
        println!(
            "{minute:>3} min: lake {:.1} m2 surface {:.2} v ({:.3} m), over drain {over_drain:.2} v, table {:.3} m, sky {:.2} m3, tier pools {tiers} | m3: lake {in_lake:.2} other free {:.2} pore {pore:.2} aquifer {:.2}",
            lake.visible_m2,
            surface,
            surface * vm,
            world.aquifer_head_m(),
            world.atmosphere_m3(),
            free - in_lake,
            v.aquifer_m3,
        );
        if std::env::var_os("COURSE").is_some() {
            // Water in the stream's columns, spring to lake: the most free water in any
            // cell of the column, sampled every few columns.
            let c = world.config();
            let v = world.view();
            let line: Vec<String> = course
                .iter()
                .step_by(3)
                .map(|&(x, z)| {
                    let most = (0..c.height)
                        .map(|y| v.free[c.index(x as i64, y, z)])
                        .fold(0.0f64, f64::max);
                    format!("{most:.2}")
                })
                .collect();
            println!("  stream: {}", line.join(" "));
        }
        for _ in 0..60 * TICK_HZ as u64 {
            world.step_with(threads);
        }
    }
}
