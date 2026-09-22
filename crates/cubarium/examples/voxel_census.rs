//! Voxel census: does the seeded habitat last unattended?
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_census -- 6 > runs/voxel-census-6h.csv
//! ```
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_census -- 6 generated closed \
//!     > runs/voxel-census-generated-closed-6h.csv
//! ```
//!
//! Builds a world — the authored fixture by default, or the core's generator — seeds it
//! with the standard habitat, and steps the
//! coupled simulation for the given number of simulated hours (default 6). Every simulated
//! minute — 1,200 ticks at [`cubarium_voxel_fauna::TICK_HZ`] — it prints one CSV row to
//! stdout: the per-species stand counts, the per-species animal counts and mean body, the
//! per-founder-lineage counts and mean body, the four ledger birth/death counters, and the
//! total litter organic. The seed report goes to
//! stderr so it never mixes with the CSV.
//!
//! The founders are driven by the built-in trained centres by default — the same drivers
//! `cubarium voxel`'s ambient run installs — so this measures the world that ships, not
//! the seeder's bare heuristics. A trailing `heuristic` argument keeps the old,
//! observation-only control:
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_census -- 1 generated closed heuristic
//! ```

use cubarium::voxel::VoxelConfig;
use cubarium::voxel::habitat;
use cubarium::voxel::install_default_founders;
use cubarium::voxel::scene;
use cubarium_voxel::{Command as WorldCommand, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig, Founder, Senses, Species as Beast, TICK_HZ};
use cubarium_voxel_flora::{Flora, FloraConfig, Species as Plant};
use cubarium_voxel_sim::{Sim, SimConfig};

/// One simulated minute, in ticks: 60 s at the fixed tick rate.
const TICKS_PER_MIN: u64 = 60 * TICK_HZ as u64;

/// The flora study harness's rain rate, reused as the closed budget's shower rate
/// (`crates/cubarium-voxel-flora/examples/harness/mod.rs`). An experiment condition.
const HARNESS_RAIN_M_PER_S: f64 = 0.0002;
/// Evaporation for the closed budget: half the shower rate. It has to be **under** the
/// shower rate or a falling drop is lifted again in the same tick — `evaporate` runs
/// right after `rain` — and no rain ever reaches the soil. An experiment condition, not
/// a model default, and the same one `cubarium-voxel`'s `water_cycle` example uses.
const CLOSED_EVAPORATION_M_PER_S: f64 = 0.0001;

/// First seed a `preset=` arm offers the host's lake gate; `seed=N` for another. The
/// host draws at random, and an arm has to be re-runnable.
const PRESET_SEED_BASE: u64 = 1;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let hours: f64 = args.get(1).map_or(6.0, |a| {
        a.parse()
            .expect("usage: voxel_census [HOURS] [authored|generated] [open|closed] [heuristic]")
    });
    let generated = args.iter().any(|a| a == "generated");
    let closed = args.iter().any(|a| a == "closed");
    // `preset=<small|default|wide>` is the landscape arm: the world `cubarium voxel`
    // builds with no TOML for that preset, not the world this file builds for itself.
    // Same construction as `voxel_founder_autopsy`'s arm, on purpose.
    let preset: Option<&'static cubarium_voxel::Preset> = args
        .iter()
        .find_map(|a| a.strip_prefix("preset="))
        .map(|name| {
            cubarium_voxel::Preset::find(name).unwrap_or_else(|| {
                let known: Vec<&str> = cubarium_voxel::PRESETS.iter().map(|p| p.name).collect();
                panic!("no landform preset is called {name:?}; the shipped ones are {known:?}")
            })
        });
    let seed_base: u64 = args
        .iter()
        .find_map(|a| a.strip_prefix("seed=").and_then(|s| s.parse().ok()))
        .unwrap_or(PRESET_SEED_BASE);
    // The shipped world's founders are driven by the built-in trained centres by
    // default, exactly as `cubarium voxel`'s ambient run installs them — this measures
    // the world that ships, not the seeder's bare heuristics. `heuristic` as a trailing
    // argument keeps the old, observation-only control.
    let heuristic = args.iter().any(|a| a == "heuristic");

    let cfg = VoxelConfig::default();
    if let Some(preset) = preset {
        assert!(!generated, "a preset arm builds its own world");
        let (mut world, seed, rejected) =
            cubarium::voxel::ambient_world(&preset.config(), seed_base);
        let mut flora = Flora::new(FloraConfig::for_voxel_size(world.config().voxel_m));
        let mut fauna = Fauna::new(FaunaConfig::default());
        eprintln!(
            "scene: preset {} ({}x{}x{} at {} m, seed {seed}, {rejected} rejected)",
            preset.name,
            world.config().width,
            world.config().height,
            world.config().depth,
            world.config().voxel_m,
        );
        let seeded = habitat::seed(&mut world, &mut flora, &mut fauna);
        eprintln!(
            "seeded: stands={} logs={} litter_tiles={} founders={:?} animals={}",
            seeded.stands,
            seeded.logs,
            seeded.litter_tiles,
            seeded.founders,
            seeded.animals()
        );
        if heuristic {
            eprintln!("founders: the observation-only heuristic (control)");
        } else {
            install_default_founders(&mut fauna).expect("the built-in centres validate");
        }
        let mut senses = Senses::new();
        senses.settle(&world.view(), &flora.view());
        let mut sim = Sim::new(world, flora, fauna, SimConfig::default(), Some(senses));
        // The host opens a closed world's outlet after the layers are built and before
        // the first tick; under a closed budget it is the return flow, not an export.
        if sim.world().config().closed_water_budget && !sim.world().outlet_open() {
            sim.world_mut()
                .apply(WorldCommand::SetOutlet { open: true });
        }
        run(&mut sim, hours);
        return;
    }
    let mut world = if generated {
        // The closed budget's water is the flora study's own: the shower rate it ran on,
        // the aquifer charged a metre above the basin floor, and the outlet open — which
        // in a closed world is the return flow into the atmosphere, not an export.
        let mut world_cfg = cfg.world.clone();
        if closed {
            let dry = VoxelConfig::default().world;
            let basin_floor_m = World::new(dry.clone())
                .outlet_cell()
                .map_or(0.0, |(_, y, _)| f64::from(y))
                * dry.voxel_m;
            world_cfg = cubarium_voxel::Config {
                rain_m_per_s: HARNESS_RAIN_M_PER_S,
                evaporation_m_per_s: CLOSED_EVAPORATION_M_PER_S,
                initial_aquifer_head_m: basin_floor_m + 1.0,
                closed_water_budget: true,
                ..world_cfg
            };
        }
        let mut w = World::new(world_cfg);
        if closed {
            w.apply(WorldCommand::SetOutlet { open: true });
        }
        w
    } else {
        assert!(
            !closed,
            "the closed budget is measured on the generated world"
        );
        scene::authored(cfg.world.clone())
    };
    eprintln!(
        "scene: {} world, {} water budget",
        if generated { "generated" } else { "authored" },
        if closed { "closed" } else { "open" }
    );

    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let seeded = habitat::seed(&mut world, &mut flora, &mut fauna);
    eprintln!(
        "seeded: stands={} logs={} litter_tiles={} founders={:?} animals={}",
        seeded.stands,
        seeded.logs,
        seeded.litter_tiles,
        seeded.founders,
        seeded.animals()
    );
    if heuristic {
        eprintln!("founders: the observation-only heuristic (control)");
    } else {
        install_default_founders(&mut fauna).expect("the built-in centres validate");
    }

    // The live founders sense a settled litter field; the ambient run settles it the
    // same way before the first tick.
    let mut senses = Senses::new();
    senses.settle(&world.view(), &flora.view());
    let mut sim = Sim::new(world, flora, fauna, SimConfig::default(), Some(senses));

    run(&mut sim, hours);
}

/// Step the coupled layers for the asked-for hours, writing one row a simulated minute.
/// Both arms end here, so a preset world is measured by the same code path as the ridge.
fn run(sim: &mut Sim, hours: f64) {
    let total_ticks = (hours * 3600.0 * f64::from(TICK_HZ)) as u64;
    print_header();
    print_row(0, sim);
    let mut tick = 0u64;
    while tick < total_ticks {
        sim.step();
        tick += 1;
        if tick % TICKS_PER_MIN == 0 {
            print_row(tick / TICKS_PER_MIN, sim);
        }
    }
}

/// The CSV header: `sim_min`, then per plant species `stands_<name>`, then per animal
/// species `animals_<name>` and `body_<name>`, then per founder lineage its count, mean
/// body, standing eggs and gestating bodies, then the ledger counters, then `litter`.
fn print_header() {
    let mut header = String::from("sim_min");
    for s in Plant::ALL {
        header.push_str(&format!(",stands_{}", s.name()));
    }
    for s in Beast::ALL {
        header.push_str(&format!(",animals_{}", s.name()));
        header.push_str(&format!(",body_{}", s.name()));
    }
    for f in Founder::ALL {
        header.push_str(&format!(",founders_{}", f.name()));
        header.push_str(&format!(",body_{}", f.name()));
        // The eggs of that lineage standing in the world and the bodies of it
        // gestating: a lineage with no living body but a clutch on the ground has not
        // gone extinct yet, and the count alone could not say so.
        header.push_str(&format!(",eggs_{}", f.name()));
        header.push_str(&format!(",gestating_{}", f.name()));
    }
    header.push_str(",flora_births,flora_deaths,fauna_births,fauna_deaths,fauna_hatched,litter");
    header.push_str(",stored,atmosphere,showers,residual");
    println!("{header}");
}

/// One CSV row: `sim_min`, then per plant species the stand count, then per animal
/// species the animal count and mean body, then per lineage its count, mean body, eggs
/// and gestations, then the ledger counters, then the total litter.
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
    for founder in Founder::ALL {
        let bodies: Vec<f64> = a
            .animals
            .iter()
            .filter(|an| an.founder == Some(founder))
            .map(|an| an.body)
            .collect();
        row.push(bodies.len().to_string());
        let mean = if bodies.is_empty() {
            0.0
        } else {
            bodies.iter().sum::<f64>() / bodies.len() as f64
        };
        row.push(format!("{mean:.4}"));
        row.push(a.eggs_by_founder(founder).to_string());
        row.push(a.gestating_by_founder(founder).to_string());
    }
    row.push(f.ledger.births.to_string());
    row.push(f.ledger.deaths.to_string());
    row.push(a.ledger.births.to_string());
    row.push(a.ledger.deaths.to_string());
    row.push(a.ledger.hatched.to_string());
    let litter: f64 = f.ground.iter().map(|g| g.litter).sum();
    row.push(format!("{litter:.4}"));
    let w = sim.world().view();
    row.push(format!("{:.4}", w.stored_m3()));
    row.push(format!("{:.4}", w.atmosphere_m3));
    row.push(w.ledger.showers.to_string());
    row.push(format!("{:.3e}", w.total_residual()));
    println!("{}", row.join(","));
}
