//! Voxel census: does the seeded habitat last unattended?
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_census -- 6 > runs/voxel-census-6h.csv
//! ```
//!
//! Builds the default authored world, seeds it with the standard habitat, and steps the
//! coupled simulation for the given number of simulated hours (default 6). Every simulated
//! minute — 1,200 ticks at [`cubarium_voxel_fauna::TICK_HZ`] — it prints one CSV row to
//! stdout: the per-species stand counts, the per-species animal counts and mean body, the
//! four ledger birth/death counters, and the total litter organic. The seed report goes to
//! stderr so it never mixes with the CSV.

use cubarium::voxel::habitat;
use cubarium::voxel::scene;
use cubarium::voxel::VoxelConfig;
use cubarium_voxel_fauna::{Fauna, FaunaConfig, Species as Beast, TICK_HZ};
use cubarium_voxel_flora::{Flora, FloraConfig, Species as Plant};
use cubarium_voxel_sim::{Sim, SimConfig};

/// One simulated minute, in ticks: 60 s at the fixed tick rate.
const TICKS_PER_MIN: u64 = 60 * TICK_HZ as u64;

fn main() {
    let hours: f64 = std::env::args()
        .nth(1)
        .map_or(6.0, |a| a.parse().expect("usage: voxel_census [HOURS]"));

    let cfg = VoxelConfig::default();
    let mut world = scene::authored(cfg.world.clone());

    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let seeded = habitat::seed(&mut world, &mut flora, &mut fauna);
    eprintln!(
        "seeded: stands={} logs={} animals={}",
        seeded.stands, seeded.logs, seeded.animals
    );

    let mut sim = Sim::new(world, flora, fauna, SimConfig::default());

    let total_ticks = (hours * 3600.0 * f64::from(TICK_HZ)) as u64;
    print_header();
    print_row(0, &sim);
    let mut tick = 0u64;
    while tick < total_ticks {
        sim.step();
        tick += 1;
        if tick % TICKS_PER_MIN == 0 {
            print_row(tick / TICKS_PER_MIN, &sim);
        }
    }
}

/// The CSV header: `sim_min`, then per plant species `stands_<name>`, then per animal
/// species `animals_<name>` and `body_<name>`, then the four ledger counters, then
/// `litter`.
fn print_header() {
    let mut header = String::from("sim_min");
    for s in Plant::ALL {
        header.push_str(&format!(",stands_{}", s.name()));
    }
    for s in Beast::ALL {
        header.push_str(&format!(",animals_{}", s.name()));
        header.push_str(&format!(",body_{}", s.name()));
    }
    header.push_str(",flora_births,flora_deaths,fauna_births,fauna_deaths,litter");
    println!("{header}");
}

/// One CSV row: `sim_min`, then per plant species the stand count, then per animal species
/// the animal count and mean body, then the four ledger counters, then the total litter.
fn print_row(minute: u64, sim: &Sim) {
    let f = sim.flora().view();
    let a = sim.fauna().view();

    let mut row = vec![minute.to_string()];
    for s in Plant::ALL {
        let n = f.stands.iter().filter(|st| st.species == s).count();
        row.push(n.to_string());
    }
    for s in Beast::ALL {
        let bodies: Vec<f64> = a
            .animals
            .iter()
            .filter(|an| an.species == s)
            .map(|an| an.body)
            .collect();
        row.push(bodies.len().to_string());
        let mean = if bodies.is_empty() {
            0.0
        } else {
            bodies.iter().sum::<f64>() / bodies.len() as f64
        };
        row.push(format!("{mean:.4}"));
    }
    row.push(f.ledger.births.to_string());
    row.push(f.ledger.deaths.to_string());
    row.push(a.ledger.births.to_string());
    row.push(a.ledger.deaths.to_string());
    let litter: f64 = f.ground.iter().map(|g| g.litter).sum();
    row.push(format!("{litter:.4}"));
    println!("{}", row.join(","));
}
