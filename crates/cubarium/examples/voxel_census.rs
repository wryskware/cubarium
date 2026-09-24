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
use cubarium::voxel::{install_founders_with, policy_args};
use cubarium::voxel::scene;
use cubarium_voxel::{Command as WorldCommand, World};
use cubarium_voxel_fauna::{
    Departure, Fauna, FaunaConfig, Founder, Senses, Species as Beast, TICK_HZ,
};
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
    // `policy=<lineage>=<file>`: that lineage runs a saved policy instead of the
    // built-in centre (P5-C).
    let policies = policy_args(&args).expect("policy=<lineage>=<file>");
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
    // `config=<path>` is a landscape arm from a host TOML (the Tachyon terrarium, say),
    // founded exactly like a preset arm. `nofauna` seeds no founders: the plants alone.
    let file_arm: Option<(String, cubarium_voxel::Config)> = args
        .iter()
        .find_map(|a| a.strip_prefix("config="))
        .map(|p| {
            let cfg = cubarium::voxel::load_config(std::path::Path::new(p))
                .unwrap_or_else(|e| panic!("config {p:?}: {e}"));
            (p.to_string(), cfg.world)
        });
    let arm: Option<(String, cubarium_voxel::Config)> =
        file_arm.or_else(|| preset.map(|p| (p.name.to_string(), p.config())));
    let founder_counts = if args.iter().any(|a| a == "nofauna") {
        [0; habitat::FOUNDER_COUNTS.len()]
    } else {
        habitat::FOUNDER_COUNTS
    };

    let cfg = VoxelConfig::default();
    if let Some((arm_name, arm_config)) = arm {
        assert!(!generated, "a preset arm builds its own world");
        // The host's founding loop, verbatim: lake gate, pre-roll with the opening
        // shower, seeding and the acceptance check, redrawn on a refusal. The world
        // comes back already seeded.
        let founded = cubarium::voxel::ambient_habitat(
            &arm_config,
            seed_base,
            FloraConfig::for_voxel_size,
            founder_counts,
        );
        let (world, flora, mut fauna, seeded) =
            (founded.world, founded.flora, founded.fauna, founded.seeded);
        eprintln!(
            "scene: preset {} ({}x{}x{} at {} m, seed {}, {} lake / {} habitat rejected, {})",
            arm_name,
            world.config().width,
            world.config().height,
            world.config().depth,
            world.config().voxel_m,
            founded.seed,
            founded.lake_rejected,
            founded.habitat_rejected,
            if founded.accepted { "accepted" } else { "NOT accepted" },
        );
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
            install_founders_with(&mut fauna, &policies).expect("the founder centres validate");
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
        install_founders_with(&mut fauna, &policies).expect("the founder centres validate");
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
    print_row(0, sim, 0.0);
    let mut tick = 0u64;
    let mut window = SeedWindow::default();
    let mut since = std::time::Instant::now();
    while tick < total_ticks {
        sim.step();
        tick += 1;
        if tick % TICKS_PER_MIN == 0 {
            let ms = since.elapsed().as_secs_f64() * 1e3 / TICKS_PER_MIN as f64;
            print_row(tick / TICKS_PER_MIN, sim, ms);
            since = std::time::Instant::now();
        }
        if tick % (60 * TICKS_PER_MIN) == 0 || tick == total_ticks {
            window.report(tick, sim);
        }
    }
}

/// The seed-bank line on stderr, once a simulated hour: how many sites hold a bank (what
/// D5's seed marks draw), recruits so far, and the SeedBank / Propagate phases' cost per
/// tick over the hour just past. Measurement only.
#[derive(Default)]
struct SeedWindow {
    nanos: [u64; 3],
    calls: [u64; 3],
}

impl SeedWindow {
    fn report(&mut self, tick: u64, sim: &Sim) {
        use cubarium_voxel::profile::{Phase, calls, nanos};
        let f = sim.flora().view();
        let sites = f.ground.iter().filter(|g| !g.seeds.is_empty()).count();
        let mut ms = [0.0; 3];
        for (i, p) in [Phase::SeedBank, Phase::Propagate, Phase::FloraStep]
            .into_iter()
            .enumerate()
        {
            let (n, c) = (nanos(p), calls(p));
            let dc = c.saturating_sub(self.calls[i]).max(1);
            ms[i] = n.saturating_sub(self.nanos[i]) as f64 / dc as f64 / 1e6;
            self.nanos[i] = n;
            self.calls[i] = c;
        }
        eprintln!(
            "seedbank: h={:.2} bank_sites={sites} ground={} stands={} establishments={} \
             seed_bank_ms={:.4} propagate_ms={:.4} flora_ms={:.3}",
            tick as f64 / (3600.0 * f64::from(TICK_HZ)),
            f.ground.len(),
            f.stands.len(),
            f.ledger.establishments,
            ms[0],
            ms[1],
            ms[2],
        );
        // Package S's counters: per species, g seeds germinated, c runner/rhizome
        // daughters, l seeds landed in a bank, d seeds died in it, x spore/water seeds
        // that found no ground; and the whole seeds banked now.
        let l = f.ledger;
        let mut modes = String::new();
        for s in Plant::ALL {
            let i = s.index();
            let (g, c, ld, d, x) = (
                l.seeds_germinated[i],
                l.clonal_births[i],
                l.seeds_landed[i],
                l.seeds_died[i],
                l.seeds_lost[i],
            );
            if g + c + ld + d + x > 0 {
                modes.push_str(&format!(" {}:g{g}/c{c}/l{ld}/d{d}/x{x}", s.name()));
            }
        }
        let banked: f64 = f
            .ground
            .iter()
            .flat_map(|g| g.seeds.iter())
            .map(|c| {
                let sc = f.config.species(c.species);
                c.organic / (sc.alive_min / sc.propagule_split[0])
            })
            .sum();
        eprintln!(
            "seedmodes: h={:.2} banked_seeds={banked:.0}{modes}",
            tick as f64 / (3600.0 * f64::from(TICK_HZ))
        );
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
    header.push_str(",fixed_in,consumed_organic,standing_foliage");
    for s in Plant::ALL {
        header.push_str(&format!(",donors_{}", s.name()));
    }
    // The water's own reading: the lake's mean surface in voxels (`y + free` of its open
    // surface cells), its volume and visible area, and the in-world stores beside it.
    header.push_str(",lake_level_v,lake_m3,lake_m2,free_m3,pore_m3,aquifer_m3,tick_ms");
    // Package SM: sites holding a seed bank, free sites showing a D5 seed mark (a recent
    // landing or about to sprout), the marked sites that are about to sprout, and every
    // support face in the world, the share's denominator.
    header.push_str(",banked_sites,marked_sites,sprouting_sites,support_sites");
    // Package mobility: deaths by cause per lineage, cumulative.
    for f in Founder::ALL {
        header.push_str(&format!(",starved_{0},drowned_{0}", f.name()));
    }
    // Package mobility's movement counters per lineage, cumulative: walls climbed to the
    // top, cliffs climbed to the foot, founder-ticks on a face and alive, ledge steps up
    // and down.
    for f in Founder::ALL {
        header.push_str(&format!(
            ",ascents_{0},descents_{0},wall_ticks_{0},alive_ticks_{0},ledges_up_{0},ledges_down_{0}",
            f.name()
        ));
    }
    println!("{header}");
}

/// One CSV row: `sim_min`, then per plant species the stand count, then per animal
/// species the animal count and mean body, then per lineage its count, mean body, eggs
/// and gestations, then the ledger counters, then the total litter.
fn print_row(minute: u64, sim: &Sim, tick_ms: f64) {
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
    // Production and offtake, cumulative: consumed / fixed is the herbivores' share.
    row.push(format!("{:.4}", f.ledger.fixed_in));
    row.push(format!("{:.4}", f.ledger.consumed_organic_out));
    let foliage: f64 = f.stands.iter().map(|st| st.foliage).sum();
    row.push(format!("{foliage:.4}"));
    for s in Plant::ALL {
        let donor_min = f.config.species(s).donor_min;
        let n = f.stands.iter().filter(|st| st.species == s && st.wood >= donor_min).count();
        row.push(n.to_string());
    }
    let world = sim.world();
    let lake = cubarium_voxel::hydrate::lake(world);
    let level = if lake.surface_cells.is_empty() {
        f64::from(lake.level_y)
    } else {
        lake.surface_cells
            .iter()
            .map(|&i| f64::from(world.config().coords(i).1) + w.free[i])
            .sum::<f64>()
            / lake.surface_cells.len() as f64
    };
    row.push(format!("{level:.3}"));
    row.push(format!("{:.4}", lake.volume_m3));
    row.push(format!("{:.2}", lake.visible_m2));
    row.push(format!("{:.4}", world.pooled_m3()));
    row.push(format!("{:.4}", world.pore_m3()));
    row.push(format!("{:.4}", w.aquifer_m3));
    row.push(format!("{tick_ms:.3}"));
    let banked = f.ground.iter().filter(|g| !g.seeds.is_empty()).count();
    // Showing: a mark on a site with no stand on it (a stand's own cells outrank it).
    let marked = f
        .ground
        .iter()
        .filter(|g| g.seed_mark(f.tick).is_some() && f.stand_at(g.site).is_none())
        .count();
    let c = world.config();
    let mut supports = 0usize;
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            supports += w.supports_in_column(x, z).len();
        }
    }
    let sprouting = f
        .ground
        .iter()
        .filter(|g| g.sprouting.is_some() && g.seed_mark(f.tick) == g.sprouting)
        .count();
    row.push(banked.to_string());
    row.push(marked.to_string());
    row.push(sprouting.to_string());
    row.push(supports.to_string());
    for founder in Founder::ALL {
        for cause in [Departure::Starved, Departure::Drowned] {
            row.push(a.ledger.departed_founder(founder, cause).to_string());
        }
    }
    for founder in Founder::ALL {
        let k = founder.index();
        let l = &a.ledger;
        for n in [
            l.wall_ascents_by_founder[k],
            l.wall_descents_by_founder[k],
            l.wall_ticks_by_founder[k],
            l.founder_ticks_by_founder[k],
            l.ledges_up_by_founder[k],
            l.ledges_down_by_founder[k],
        ] {
            row.push(n.to_string());
        }
    }
    println!("{}", row.join(","));
}
