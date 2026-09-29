//! **How the grazing defence plays in a live world**: measurement only.
//!
//! ```text
//! defence_probe [seed=1] [minutes=30] policy=browser=<centre> policy=littershredder=<centre>
//! ```
//!
//! Founds the training world (`small`, still water), drives the founders with the given
//! centres, and prints once a simulated minute: browser bites per browser-tick, the
//! number of stands carrying any defence, the largest defence (so the smallest bite
//! fraction any crown now gives), and the mean defence of the defended stands.

use cubarium::voxel::policy_args;
use cubarium_search::es::voxel::VoxelPolicyFile;
use cubarium_search::es::voxel::live::{self, ecology::EcologyParams, world};
use cubarium_voxel_fauna::Founder;

fn arg<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    args.iter()
        .find_map(|a| a.strip_prefix(key).and_then(|v| v.parse().ok()))
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = arg(&args, "seed=", 1);
    let minutes: u64 = arg(&args, "minutes=", 30);
    let (mut sim, info) = world::found_world(
        &world::training_config(),
        seed,
        cubarium_voxel_sim::habitat::FOUNDER_COUNTS,
        &EcologyParams::defaults(),
    );
    let drivers: Vec<_> = policy_args(&args)
        .expect("policy=")
        .into_iter()
        .map(|(f, p)| {
            (
                f,
                VoxelPolicyFile::load(&p)
                    .expect("file")
                    .driver()
                    .expect("driver"),
            )
        })
        .collect();
    live::install_centres(sim.fauna_mut(), &drivers);
    eprintln!("seed {} stands {}", info.seed, info.stands);
    println!("minute,browsers,bites_per_browser_tick,defended,max_defence,mean_defence");
    let per_min = 60 * u64::from(cubarium_voxel_fauna::TICK_HZ);
    let b = Founder::Browser.index();
    for minute in 1..=minutes {
        let bites0 = sim.fauna().view().ledger.bites_by_founder[b];
        let mut browser_ticks = 0u64;
        for _ in 0..per_min {
            sim.step();
            browser_ticks += sim
                .fauna()
                .view()
                .animals
                .iter()
                .filter(|a| a.founder == Some(Founder::Browser))
                .count() as u64;
        }
        let bites = sim.fauna().view().ledger.bites_by_founder[b] - bites0;
        let d: Vec<f64> = sim
            .flora()
            .view()
            .stands
            .iter()
            .map(|s| s.defence)
            .filter(|d| *d > 1e-6)
            .collect();
        println!(
            "{minute},{},{:.3},{},{:.3},{:.3}",
            browser_ticks / per_min,
            bites as f64 / browser_ticks.max(1) as f64,
            d.len(),
            d.iter().cloned().fold(0.0, f64::max),
            if d.is_empty() {
                0.0
            } else {
                d.iter().sum::<f64>() / d.len() as f64
            }
        );
    }
}
