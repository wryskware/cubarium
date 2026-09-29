//! **The live-training world's tick, by phase**, at `threads=1`: measurement only.
//!
//! ```text
//! live_world_profile [ticks=6000] [seed=1] [founders=8,8] [warm=0] \
//!     policy=browser=<centre.json> policy=littershredder=<centre.json> [flowing]
//! ```
//!
//! Founds the training world (`small`, still water:
//! [`cubarium_search::es::voxel::live::world::training_config`]; `flowing` keeps the
//! preset's own water for comparison), drives the founders with the given centres, runs
//! `warm` ticks untimed (`water_every=k` runs the training-only coarse water), and then `ticks` ticks with the phase timers, and prints the
//! water / flora / fauna split of a tick, the animal count at start and end, and the whole
//! tick's wall time. `founders=N,M` seeds N shredders and M browsers, which is how a
//! boomed population is measured without waiting for a boom.

use std::time::Instant;

use cubarium::voxel::policy_args;
use cubarium_search::es::voxel::VoxelPolicyFile;
use cubarium_search::es::voxel::live::{self, ecology::EcologyParams, world};
use cubarium_voxel::profile::{self, Phase};
use cubarium_voxel_fauna::Founder;

/// The water leaves as the schedule runs them (the substep loop is a schedule of its
/// own there, so its total timer never runs; its three leaves do).
const WATER_PHASES: [Phase; 10] = [
    Phase::Begin,
    Phase::Rain,
    Phase::Evaporate,
    Phase::Infiltrate,
    Phase::Fall,
    Phase::Exchange,
    Phase::Drain,
    Phase::WaterTable,
    Phase::Spring,
    Phase::Outlet,
];

fn arg<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    args.iter()
        .find_map(|a| a.strip_prefix(key).and_then(|v| v.parse().ok()))
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ticks: u64 = arg(&args, "ticks=", 6000);
    let warm: u64 = arg(&args, "warm=", 0);
    let seed: u64 = arg(&args, "seed=", 1);
    let founders: String = arg(&args, "founders=", "8,8".to_string());
    let counts: Vec<usize> = founders
        .split(',')
        .map(|s| s.parse().expect("founders=N,M"))
        .collect();
    let counts: [usize; Founder::COUNT] =
        std::array::from_fn(|i| counts.get(i).copied().unwrap_or(0));
    let flowing = args.iter().any(|a| a == "flowing");
    let every: u32 = arg(&args, "water_every=", 1);
    let cfg = if flowing {
        cubarium_voxel::Preset::find("small").unwrap().config()
    } else {
        world::training_config()
    };
    let t0 = Instant::now();
    let (mut sim, info) = world::found_world(&cfg, seed, counts, &EcologyParams::defaults());
    eprintln!(
        "founded seed {} accepted={} founders={:?} stands={} in {:.1} s ({})",
        info.seed,
        info.accepted,
        info.founders,
        info.stands,
        t0.elapsed().as_secs_f64(),
        if flowing { "flowing" } else { "still" },
    );
    let drivers: Vec<_> = policy_args(&args)
        .expect("policy=")
        .into_iter()
        .map(|(f, p)| {
            let file = VoxelPolicyFile::load(&p).expect("policy file");
            (f, file.driver().expect("driver"))
        })
        .collect();
    live::install_centres(sim.fauna_mut(), &drivers);
    sim.world_mut().set_water_every(every);
    for _ in 0..warm {
        sim.step();
    }
    let n0 = sim.fauna().view().animals.len();
    profile::reset();
    let t = Instant::now();
    for _ in 0..ticks {
        sim.step();
    }
    let wall = t.elapsed().as_secs_f64();
    let n1 = sim.fauna().view().animals.len();
    let ms = |ns: u64| ns as f64 / 1e6 / ticks as f64;
    let water: u64 = WATER_PHASES.iter().map(|p| profile::nanos(*p)).sum();
    let flora = profile::nanos(Phase::FloraStep);
    let fauna = profile::nanos(Phase::FaunaStep);
    let tick_ms = wall * 1e3 / ticks as f64;
    println!(
        "ticks={ticks} animals {n0}->{n1} tick_ms={tick_ms:.3} water_ms={:.3} ({:.0}%) \
         flora_ms={:.3} ({:.0}%) fauna_ms={:.3} ({:.0}%) other_ms={:.3}",
        ms(water),
        100.0 * ms(water) / tick_ms,
        ms(flora),
        100.0 * ms(flora) / tick_ms,
        ms(fauna),
        100.0 * ms(fauna) / tick_ms,
        tick_ms - ms(water) - ms(flora) - ms(fauna),
    );
    for p in Phase::ALL {
        let n = profile::nanos(p);
        if n > 0 {
            println!("  {:<48} {:.4} ms/tick", p.name(), ms(n));
        }
    }
}
