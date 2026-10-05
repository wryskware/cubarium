//! Film census: where glassfilm sits and what water keeps it there (a diagnostic).
//!
//! ```text
//! cargo run --release -p cubarium --example film_census -- config/desktop/terrarium.toml 60 1
//! ```
//!
//! Founds the TOML's world as `founder_tally` does, but with no animals (seed `SEED`,
//! asked), and steps it `MINUTES` simulated minutes. Every `every=S` seconds (default 120)
//! and on each change of the weather's rain mode it prints one `film@` line: the weather,
//! the relative humidity, and the film sites split by
//!
//! - the standing depth over the site (`standing_depth_m`), in bins: `0`, then decades of
//!   metres, then `deep` (a quarter metre and more); and a `wet-` line per sample bins every wet support face (film or not) the same way;
//! - whether the column above the face is open sky (no solid above it: evaporation can
//!   take its water) or `cov` (covered);
//! - and the film as a share of the site's capacity: `lo` under 1/16 (no mark drawn),
//!   `hi` at or above it.
//!
//! A `bin` cell prints `count(hi)`. It also prints the render's capacity next to the
//! sim's (the flora config's `voxel_m` against the world's).

use cubarium::voxel::{LayerConfig, Seeds, found, load_config, params_of};
use cubarium_voxel::weather::RainMode;
use cubarium_voxel_fauna::Founder;
use cubarium_voxel_sim::{Sim, SimConfig};

/// Bin upper edges, metres; a depth at or past the last is `deep` (a pond).
const EDGES: [f64; 10] = [1e-7, 1e-6, 1e-5, 1e-4, 1e-3, 3e-3, 1e-2, 3e-2, 0.0625, 0.25];
const BINS: [&str; 12] = [
    "0", "<1e-7", "<1e-6", "<1e-5", "<1e-4", "<1e-3", "<3e-3", "<1e-2", "<3e-2", "<.0625", "<.25", "deep",
];

fn bin(d: f64) -> usize {
    if !(d > 0.0) {
        return 0;
    }
    EDGES.iter().position(|&e| d < e).map_or(EDGES.len() + 1, |i| i + 1)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Training, search and experiment runs take the coarse water (`water_every=`).
    cubarium_voxel_sim::experiment_args(&args);
    let path = args.get(1).map_or("config/desktop/terrarium.toml", String::as_str);
    let minutes: u64 = args.get(2).map_or(60, |a| a.parse().expect("MINUTES"));
    let seed: u64 = args.get(3).map_or(1, |a| a.parse().expect("SEED"));
    let mut every: u64 = 120;
    for a in args.iter().skip(4) {
        if let Some(s) = a.strip_prefix("every=") {
            every = s.parse().expect("every=S");
        } else {
            panic!("unknown argument {a}");
        }
    }
    cubarium_voxel::set_thread_override(1);
    let cfg = load_config(std::path::Path::new(path)).expect("config");
    let params = params_of(&cfg, &[]).expect("params");
    let world_cfg = params.world.clone();
    let founded = found(
        &world_cfg,
        &LayerConfig::new(&params, world_cfg.voxel_m),
        [0; Founder::COUNT],
        Seeds::Asked(seed),
        false,
    );
    println!(
        "founded: seed {} film_sites {} world_voxel_m {} flora_voxel_m {} evaporation_m_per_s {:e} closed {}",
        founded.seed,
        founded.seeded.film_sites,
        founded.world.config().voxel_m,
        founded.flora.config().voxel_m,
        founded.world.config().evaporation_m_per_s,
        founded.world.config().closed_water_budget,
    );
    let mut sim = Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::experiment());
    sim.set_overlap(false);
    let hz = u64::from(cubarium_voxel::TICK_HZ);
    let mut last_mode = sim.world().view().weather.mode;
    census(&sim, 0);
    for tick in 1..=minutes * 60 * hz {
        sim.step();
        let mode = sim.world().view().weather.mode;
        if mode != last_mode || tick % (every * hz) == 0 {
            if mode != last_mode {
                println!("weather {:?} -> {:?} at {}s", last_mode, mode, tick / hz);
                last_mode = mode;
            }
            census(&sim, tick / hz);
        }
    }
}

fn census(sim: &Sim, s: u64) {
    let world = sim.world();
    let view = world.view();
    let fv = sim.flora().view();
    let c = fv.config;
    let v = view.config.voxel_m;
    let cap_sim = c.film_capacity_per_m2 * v * v;
    let cap_render = c.film_capacity_per_m2 * c.voxel_m * c.voxel_m;
    // [open/covered][bin] = (count, count at or above 1/16 of capacity, film)
    let mut cells = [[(0usize, 0usize, 0.0f64); 12]; 2];
    let mut eligible = 0usize;
    for (site, film) in fv.film_sites() {
        let x = i64::from(site.x);
        let d = view.standing_depth_m(x, site.y, site.z);
        let open = !view.solid_in(x, site.z, site.y + 1, view.config.height);
        if d > 0.0 && d <= c.film_max_depth_m {
            eligible += 1;
        }
        let e = &mut cells[usize::from(!open)][bin(d)];
        e.0 += 1;
        if film >= cap_sim / 16.0 {
            e.1 += 1;
        }
        e.2 += film;
    }
    let mode = view.weather.mode;
    let row = |k: usize| {
        cells[k]
            .iter()
            .zip(BINS)
            .map(|(e, b)| format!("{b}:{}({})", e.0, e.1))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let total: usize = cells.iter().flatten().map(|e| e.0).sum();
    println!(
        "film@{s}s mode={} rh={:.3} sites={total} eligible={eligible} cap_sim={cap_sim:.6} cap_render={cap_render:.6}",
        match mode {
            RainMode::Clear => "clear".to_string(),
            m => format!("{m:?}"),
        },
        world.rh(),
    );
    println!("  open  {}", row(0));
    println!("  cov   {}", row(1));
    // Every wet support face, film or not: the water alone.
    let mut faces = [[0usize; 12]; 2];
    let cfg = view.config;
    for z in 0..cfg.depth {
        for x in 0..i64::from(cfg.width) {
            for y in 0..cfg.height {
                if !view.is_support(x, y, z) {
                    continue;
                }
                let d = view.standing_depth_m(x, y, z);
                if d > 0.0 {
                    let open = !view.solid_in(x, z, y + 1, cfg.height);
                    faces[usize::from(!open)][bin(d)] += 1;
                }
            }
        }
    }
    for (k, name) in [(0, "open"), (1, "cov")] {
        let r: Vec<String> = faces[k].iter().zip(BINS).skip(1).map(|(n, b)| format!("{b}:{n}")).collect();
        println!("  wet-{name} {}", r.join(" "));
    }
}
