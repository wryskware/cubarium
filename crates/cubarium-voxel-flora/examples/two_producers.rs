//! Two producers on the generated strip: founder stands of bloomcrown on the highest
//! support faces and of umbrellafrond in the lowest, then N seconds of coupled world
//! and plants.
//!
//! `cargo run -p cubarium-voxel-flora --example two_producers [seconds] [seed]
//! [noise_seed]`
//!
//! The generated world starts bone dry and the plant model reads pore water, so the
//! example rains on it: `rain_m_per_s = 0.0005` is not weather, it is a tap, chosen to
//! bring flat ground to field capacity inside a 50-second warm-up without filling the
//! basin faster than the outlet can export. The outlet is opened for the same reason.
//! One flora rate is the example's own as well: see `BLOOMCROWN_DROWN_M`.
//!
//! What it prints: per-species stand counts, occupancy by support-height quartile (the
//! quartiles are of the *terrain's* own skyline, so "q4" is the top quarter of the
//! surface, not of the stands), and the two flora residuals.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species, Stage};

const WARMUP_TICKS: u32 = 1000;
/// The example's own `drown_depth_m` for bloomcrown, against the placeholder 0.0.
///
/// Zero means *any* free water on the support face kills it, and with rain falling
/// there is always a film in transit — the tick ends with the free-water solver's
/// `equalize`, not with `fall`, so a millimetre of rain on its way downhill is standing
/// water as far as any reader of the state is concerned. At the placeholder every
/// bloomcrown founder on this world dies in the first three ticks in 0.3 to 2 mm of
/// water. Five centimetres is a fifth of a voxel: still "dies in standing water".
const BLOOMCROWN_DROWN_M: f64 = 0.05;
const FOUNDERS_PER_SPECIES: usize = 8;
const FOUNDER_WOOD: f64 = 0.3;

fn main() {
    let mut args = std::env::args().skip(1);
    let seconds: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(60.0);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let noise_seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);

    let config = VoxelConfig { seed, noise_seed, rain_m_per_s: 0.0005, ..VoxelConfig::default() };
    let (width, depth) = (config.width, config.depth);
    let mut world = World::new(config.clone());
    world.apply(WorldCommand::SetOutlet { open: true });
    for _ in 0..WARMUP_TICKS {
        world.step();
    }

    // Every column's own highest support face, sorted low to high: the ridges are the
    // tail of this list and the hollows are its head.
    let mut skyline: Vec<Site> = Vec::new();
    for z in 0..depth {
        for x in 0..width as i64 {
            if let Some(site) = cubarium_voxel_flora::highest_support(&world.view(), x, z) {
                skyline.push(site);
            }
        }
    }
    skyline.sort_by_key(|s| (s.y, s.x, s.z));

    // Founders go where their own species could establish: wet enough at the support
    // face, bright enough, and not already under water it cannot stand in. Without that
    // filter every founder of the high species lands on bare sloping rock and dies in
    // the first tick — see the commit message on why a slope never wets.
    let mut flora = Flora::new(FloraConfig::default());
    let mut founders = 0;
    let mut placed: Vec<(Species, usize, u32, u32)> = Vec::new();
    for (species, from_the_top) in [(Species::Umbrellafrond, false), (Species::Bloomcrown, true)] {
        let sc = flora.config().species(species);
        let view = world.view();
        let mut ok: Vec<Site> = skyline
            .iter()
            .copied()
            .filter(|s| {
                let (x, y, z) = (s.x as i64, s.y, s.z);
                view.soil_below(x, y, z) >= 1
                    && view.pore_at(x, y, z) >= sc.establish_pore_min
                    && view.water_depth_m(x, y, z) <= sc.drown_depth_m
                    && view.sky_visibility(x, y, z) >= sc.establish_light_min
            })
            .collect();
        let n = ok.len();
        // A species with nowhere to establish is still seeded, on the sites its own
        // ordering prefers, so the run has two producers in it and the summary shows
        // what happens to it. The printed count is the honest one.
        if ok.is_empty() {
            ok = skyline.clone();
        }
        if from_the_top {
            ok.reverse();
        }
        let stride = (ok.len() / FOUNDERS_PER_SPECIES).max(1);
        let mut lo = u32::MAX;
        let mut hi = 0;
        for site in ok.iter().step_by(stride).take(FOUNDERS_PER_SPECIES) {
            if flora.apply(
                &world,
                Command::Seed { x: site.x as i64, z: site.z, species, wood: FOUNDER_WOOD },
            ) {
                founders += 1;
                lo = lo.min(site.y);
                hi = hi.max(site.y);
            }
        }
        placed.push((species, n, lo, hi));
    }
    for (species, n, lo, hi) in &placed {
        println!(
            "{:>14}: {n} of {} skyline sites pass its establishment predicate; founders at y {lo}..{hi}{}",
            species.name(),
            skyline.len(),
            if *n == 0 { " (seeded anyway, nowhere qualifies)" } else { "" }
        );
    }

    let ticks = (seconds * cubarium_voxel::TICK_HZ as f64).round() as u64;
    for _ in 0..ticks {
        world.step();
        flora.step(&mut world);
    }

    // ---- the summary
    let view = flora.view();
    println!(
        "two_producers: {}x{}x{} seed {seed} noise_seed {noise_seed}, rain {} m/s, outlet open",
        config.width, config.height, config.depth, config.rain_m_per_s
    );
    println!(
        "{WARMUP_TICKS} warm-up ticks, then {ticks} coupled ticks ({seconds:.0} s); {founders} founders at wood {FOUNDER_WOOD}"
    );

    for species in Species::ALL {
        let all: Vec<_> = view.stands.iter().filter(|s| s.species == species).collect();
        let alive = all.iter().filter(|s| s.stage == Stage::Alive).count();
        let wood: f64 = all.iter().map(|s| s.wood).sum();
        // Only the alive ones have read light and water: an establishing stand is frozen
        // and its two factors stay at zero.
        let live = all.iter().filter(|s| s.stage == Stage::Alive);
        let light: f64 = live.clone().map(|s| s.light).sum::<f64>() / alive.max(1) as f64;
        let moisture: f64 = live.map(|s| s.moisture).sum::<f64>() / alive.max(1) as f64;
        println!(
            "{:>14}: {} stands ({alive} alive, {} establishing), wood {wood:.4}, alive mean light {light:.3}, moisture {moisture:.3}",
            species.name(),
            all.len(),
            all.len() - alive
        );
    }

    // Quartiles of the terrain's own skyline, so occupancy is read against the shape of
    // the world rather than against where the stands happen to be.
    let bounds: Vec<u32> = (1..4).map(|q| skyline[skyline.len() * q / 4].y).collect();
    println!(
        "occupancy by skyline quartile (breaks at y = {}, {}, {}):",
        bounds[0], bounds[1], bounds[2]
    );
    for q in 0..4 {
        let lo = if q == 0 { 0 } else { bounds[q - 1] };
        let hi = if q == 3 { u32::MAX } else { bounds[q] };
        let columns = skyline.iter().filter(|s| s.y >= lo && s.y < hi).count();
        let counts: Vec<String> = Species::ALL
            .iter()
            .map(|&sp| {
                let n = view
                    .stands
                    .iter()
                    .filter(|s| s.species == sp && s.site.y >= lo && s.site.y < hi)
                    .count();
                format!("{} {n}", sp.name())
            })
            .collect();
        println!("  q{} y {lo}..{}: {} columns, {}", q + 1, hi.min(999), columns, counts.join(", "));
    }

    let l = view.ledger;
    println!(
        "ledger: light_in {:.6} heat_out {:.6} transpired {:.6} m3 deaths {} establishments {}",
        l.light_in, l.heat_out, l.transpired_m3, l.deaths, l.establishments
    );
    println!(
        "residuals: material {:.3e} energy {:.3e} (stocks: material {:.4} energy {:.4})",
        view.material() - l.expected_material(),
        view.energy() - l.expected_energy(),
        view.material(),
        view.energy()
    );
    let water = world.view().stored_m3() - world.view().ledger.expected_stored();
    println!(
        "core water: stored {:.4} m3, residual {:.3e}, transpiration_out {:.6} m3 (flora says {:.6})",
        world.view().stored_m3(),
        water,
        world.view().ledger.transpiration_out,
        l.transpired_m3
    );
}
