//! Producers on the generated strip: founder stands on the support faces each species'
//! **contract habitat** asks for, then N seconds of coupled world and plants.
//!
//! ```text
//! cargo run --release -p cubarium-voxel-flora --example two_producers -- [seconds] [seed] [noise_seed]
//! cargo run --release -p cubarium-voxel-flora --example two_producers -- community [seconds] [seed] [noise_seed]
//! cargo run --release -p cubarium-voxel-flora --example two_producers -- compare [seconds] [seed] [noise_a] [noise_b] [control_seed] [species_a] [species_b]
//! cargo run --release -p cubarium-voxel-flora --example two_producers -- chesson [fill] [probe] [seed] [noise_seed] [resident] [newcomer]
//! cargo run --release -p cubarium-voxel-flora --example two_producers -- harvest [seconds] [rate] [seed] [noise_seed]
//! ```
//!
//! Every mode names its species **by name** and none of them is wired in: the bare run and
//! `community` take whatever [`Species::ALL`] holds, and `compare` and `chesson` take any
//! pair as arguments, defaulting to the original bloomcrown/umbrellafrond pair so that the
//! round-3 invocations still mean what they meant.
//!
//! The generated world starts bone dry and the plant model reads pore water, so the
//! example rains on it. That rain is not weather and it is not a model default: it is an
//! **experiment condition** of this harness, printed in the header of every run and
//! recorded in the results note with the numbers it produced.
//!
//! Round 3b set it from the world's own arithmetic (Astra's R4.2). The default footprint
//! is 128 x 24 voxels of 0.25 m, which is 192 m², and the core's outlet exports at most
//! `outlet_m3_per_s = 0.05` with evaporation left at zero, so nominal rain above
//! `0.05 / 192 = 2.604e-4` m/s cannot leave however long the run is. The old 5e-4 m/s was
//! 0.096 m³/s in against 0.05 m³/s out: the table climbed for the whole run and every arm
//! was a rising-water disturbance rather than a habitat baseline. [`HARNESS_RAIN_M_PER_S`]
//! is now under that ceiling, and the run prints its own water budget — accepted rain,
//! outlet export, evaporation, transpiration, storage change and the head — over every
//! progress interval and at the end, so whether the head is actually bounded is a measured
//! thing and not a claim in a comment.
//!
//! What a single run prints: per-species stand counts, occupancy by support-height
//! quartile (the quartiles are of the *terrain's* own skyline, so "q4" is the top
//! quarter of the surface, not of the stands), the set of occupied skyline columns, the
//! water budget and the two flora residuals.
//!
//! `community` is round 4's smoke run: one founder cohort of **every** species on its own
//! contract habitat, one duration, no arms. It is not a study — it has no control, no
//! replication and no stationary resident — and it exists to say whether five presets can
//! be in one world at once and which gate is shutting for each of them.
//!
//! `harvest` is round 5a's **producer-response probe** and nothing more: two arms of one
//! conditioned world, one plant-only and one with a scripted harvester that takes reachable
//! foliage from three predeclared support faces for the first half of the run, and then
//! stops. It says what a stripped producer does about it — foliage, reserve and wood, and
//! how much of what was grazed is back at full foliage by the end — and it says **nothing**
//! about an animal: there is no body, no movement, no population, no carrying capacity and
//! no viability claim anywhere in it. `design/theoretical-biosphere-2026-09-16.md` §5 asks
//! for exactly this before voxel animals exist.
//!
//! `compare` is the decisive experiment of `design/voxel-ecology-sketch-2026-09-16.md`
//! §4: re-draw **only** the generator's final weak correlated noise with a new seed,
//! keep the landform, seed the same founder columns, and see whether each species
//! reoccupies the same ground. A control run with a different `seed` — a different
//! landform entirely — says what "unrelated" looks like on the same scale. If the
//! noise-pair overlap is no better than the control's, the terrain coupling is
//! decorative and the patches are reading the noise.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, World};
use cubarium_voxel_flora::{
    Command, Deposit, DepositKind, Flora, FloraConfig, Reach, Site, SkyCache, Species, Trophic,
};

/// The shared experiment conditions and placement helpers, one copy for both studies:
/// the rain, the warm-up, the founder fraction, the [`Habitat`] table and the crown
/// geometry `replacement` needs too. Included by path because an example is its own crate
/// root and these are not library rules (`examples/harness/mod.rs`).
#[path = "harness/mod.rs"]
mod harness;
use harness::*;

/// How many **declared logs** the `community` run lays down, and how much dead wood each
/// one holds.
///
/// A fresh world holds **no dead wood at all** — nothing has died in it — so a saprotroph
/// has nothing to eat and its substrate gate shuts everywhere. The harness therefore lays
/// logs before anything is planted, through round 5a's `deposit` with round 5b's
/// `DepositKind::DeadWood`, and each of them carries the mineral and the energy a dead
/// trunk of that size holds: `n_tissue · organic` and `e_v · organic`, because a log laid
/// without its energy is a log with nothing in it to eat.
///
/// **Declared experiment conditions, not model rules**, printed with every run and listed
/// in `design/backlog.md` §1. `LOG_ORGANIC` 1.0 is between one and three times a producer's
/// whole body (bloomcrown's `wood_max` is 0.6, springturf's 0.06), which is what a fallen
/// trunk is, and it is **material imported into the world** — booked as
/// `deposited_organic_in`, an inflow like `seeded_*`, and not a stock the plants grew.
/// Eight of them is [`FOUNDERS_PER_SPECIES`], so the fungus gets as many starting places as
/// every other species.
const DECLARED_LOGS: usize = FOUNDERS_PER_SPECIES;
const LOG_ORGANIC: f64 = 1.0;

/// Lay the declared logs: [`DECLARED_LOGS`] faces that pass every glowcap gate **but** the
/// substrate one, drawn in the same keyed order `Habitat::OpenSoil` uses so that the choice
/// says nothing about how wet or how high a face is. Returns the sites, sorted.
///
/// The faces are chosen with the substrate gate **satisfied by assumption** — the model's
/// own predicate, read through `establishment_gates_on_substrate` with the threshold handed
/// in — which is the honest way to ask "would a fungus be able to start here once there is
/// a log". Everything else is the real gate.
fn lay_declared_logs(world: &World, flora: &mut Flora, skyline: &[Site]) -> Vec<Site> {
    let sc = flora.config().species(Species::Glowcap).clone();
    let view = world.view();
    let mut pool: Vec<Site> = skyline
        .iter()
        .copied()
        .filter(|s| {
            cubarium_voxel_flora::establishment_gates_on_substrate(
                &view,
                *s,
                &sc,
                sc.establish_substrate_min,
            )
            .passes()
        })
        .collect();
    pool.sort_by_key(|s| (site_key(Species::Glowcap, *s), s.x, s.z));
    let candidates = pool.len();
    let stride = (pool.len() / DECLARED_LOGS).max(1);
    let chosen: Vec<Site> = pool
        .iter()
        .copied()
        .step_by(stride)
        .take(DECLARED_LOGS)
        .collect();
    let (mineral, energy) = (sc.n_tissue * LOG_ORGANIC, sc.energy_density * LOG_ORGANIC);
    let mut laid: Vec<Site> = Vec::new();
    for site in chosen {
        let deposit = Deposit {
            kind: DepositKind::DeadWood,
            organic: LOG_ORGANIC,
            mineral,
            energy,
        };
        if flora.deposit(site, deposit) {
            laid.push(site);
        }
    }
    laid.sort_unstable();
    println!(
        "declared logs: {} of {LOG_ORGANIC} organic matter each (with {mineral} of mineral \
         and {energy} of energy, a dead trunk's own), laid on {candidates} candidate faces \
         by the keyed spread — an experiment condition and not a model rule, and material \
         **imported** into the world as `deposited_organic_in`",
        laid.len()
    );
    let cells: Vec<String> = laid
        .iter()
        .map(|s| format!("({},{}) y{}", s.x, s.z, s.y))
        .collect();
    println!("declared logs at: {}", cells.join(", "));
    laid
}

/// Everything one run leaves behind for the comparison.
struct Outcome {
    seed: u64,
    noise_seed: u64,
    /// Occupied skyline columns per species, sorted, deduplicated: every stand.
    occupied: [Vec<(u32, u32)>; Species::COUNT],
    /// The same, alive stands only.
    alive: [Vec<(u32, u32)>; Species::COUNT],
    /// The columns of the lowest and the highest quarter of this run's own skyline: the
    /// hollows and the ridges, as pure landform, with no plant and no water in them.
    low_quartile: Vec<(u32, u32)>,
    high_quartile: Vec<(u32, u32)>,
    /// Per species, the skyline columns whose highest support passed the establishment
    /// predicate **at introduction**: after the 1,000-tick (50 s) warm-up and before any
    /// plant acted. Pure terrain and water, and an instantaneous reading — the water
    /// budget shows the head still falling at 50 s, so this is the eligible set of that
    /// moment and not a settled habitat (Astra R5.2).
    eligible: [Vec<(u32, u32)>; Species::COUNT],
    /// The same predicate over the same columns **at observation**: the last tick of the
    /// run, with the plants in it. Not "settled water" — the water budget shows the head
    /// still falling at the end of these runs, so this is a reading of the last tick and
    /// nothing more (Astra R6.3). Printed beside the introduction set so that a reader can
    /// see how far the eligible set moved while the arm ran.
    eligible_at_end: [Vec<(u32, u32)>; Species::COUNT],
    /// The founders this run's own selection rule would have planted, whatever it was
    /// actually handed: each species from the end of its own eligible skyline its
    /// [`Habitat`] names. Comparing these across runs asks where the *process* would put a
    /// species when it is free to choose, which the fixed-founder arms deliberately do
    /// not.
    own_founders: Vec<Founder>,
    /// What was actually planted.
    founders: Vec<Founder>,
    /// How many of the founder columns handed to this run no longer passed their
    /// species' establishment predicate, and were seeded anyway.
    off_predicate: usize,
    /// Per species: living stands at the end that are **descendants** — born from a seed
    /// bank by germination, not planted by `Command::Seed`. Counted by `Stand::id`, the
    /// ledger's own birth identity, so it is exact: a founder that died and was replaced
    /// on its own site by its own species in the same tick counts as one death and one
    /// descendant, which the site watch this replaced could not see.
    descendants: [usize; Species::COUNT],
    /// Per species: living stands that are still the original founder, by identity.
    founder_stands: [usize; Species::COUNT],
    /// The flora ledger's two life-cycle counters at the end of the run.
    establishments: u64,
    deaths: u64,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("compare") => return compare(&args[1..]),
        Some("chesson") => return chesson(&args[1..]),
        Some("community") => return community(&args[1..]),
        Some("harvest") => return harvest(&args[1..]),
        _ => {}
    }
    let seconds: f64 = arg(&args, 0).unwrap_or(60.0);
    let seed: u64 = arg(&args, 1).unwrap_or(1);
    let noise_seed: u64 = arg(&args, 2).unwrap_or(0);
    run(seconds, seed, noise_seed, &Species::ALL, None, true);
}

fn arg<T: std::str::FromStr>(args: &[String], i: usize) -> Option<T> {
    args.get(i).and_then(|s| s.parse().ok())
}

/// A species named on the command line, through the model's own [`Species::parse`]. An
/// unparseable name is a hard stop with the list printed: a silent fallback to a default
/// species would make a run mean something other than what it was asked for.
fn species_arg(args: &[String], i: usize, fallback: Species) -> Species {
    match args.get(i) {
        None => fallback,
        Some(name) => Species::parse(name).unwrap_or_else(|| {
            let names: Vec<&str> = Species::ALL.iter().map(|s| s.name()).collect();
            panic!(
                "unknown species {name:?}; the five are {}",
                names.join(", ")
            )
        }),
    }
}

/// One run: generate, warm up, seed founders, step the coupled world and plants.
///
/// `species` is which species get founders; `founders` is `None` for a run that picks its
/// own — the base of a comparison, or a single run — and `Some` for one that must plant the
/// same columns as another, whether or not its own terrain still likes them.
fn run(
    seconds: f64,
    seed: u64,
    noise_seed: u64,
    species: &[Species],
    founders: Option<&[Founder]>,
    verbose: bool,
) -> Outcome {
    // Generate once to find where the generator put the basin, then generate the world
    // the run uses with the water table charged to a metre above that floor. Generation
    // is deterministic in the seed, so the second world is the first one with water in
    // it. The outlet cell is the lowest void cell of the receiving basin, so its `y` is
    // the basin floor.
    let dry = VoxelConfig {
        seed,
        noise_seed,
        rain_m_per_s: HARNESS_RAIN_M_PER_S,
        ..VoxelConfig::default()
    };
    let basin_floor_m = World::new(dry.clone())
        .outlet_cell()
        .map_or(0.0, |(_, y, _)| y as f64)
        * dry.voxel_m;
    let config = VoxelConfig {
        initial_aquifer_head_m: basin_floor_m + 1.0,
        ..dry
    };
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

    let mut flora = Flora::new(FloraConfig::default());
    // One sky cache per run, reused across the six species and across the introduction and
    // observation sweeps: the terrain never moves, so one ray per site serves both.
    let mut sky = SkyCache::new();
    let eligible = eligible_sets(&world, &flora, &skyline, &mut sky);
    let own_founders = pick_founders(
        &world,
        &flora,
        &skyline,
        species,
        verbose && founders.is_none(),
    );
    let (planted, off_predicate) = match founders {
        None => (own_founders.clone(), 0),
        Some(given) => (given.to_vec(), off_count(&world, &flora, given)),
    };
    let mut seeded = 0;
    for f in &planted {
        if flora.apply(
            &world,
            Command::Seed {
                x: f.x as i64,
                z: f.z,
                species: f.species,
                wood: founder_wood(flora.config().species(f.species)),
            },
        ) {
            seeded += 1;
        }
    }
    if verbose && founders.is_some() {
        println!(
            "founders: the base run's {} columns replanted here, {off_predicate} of them no \
             longer passing their species' establishment predicate (seeded anyway)",
            planted.len()
        );
    }

    // The founders, by **identity**. `Stand::id` comes from the ledger's birth counter, so
    // a stand at the end either is one of these or was germinated during the run — no
    // per-tick site watch, and no way to miss a founder dying and its own species taking
    // the site in the same tick (Astra R4.7). Every stand created before the first step is
    // a founder by construction, so the ids are simply the ones standing now.
    let founder_ids: Vec<u64> = flora.view().stands.iter().map(|s| s.id).collect();
    assert_eq!(
        founder_ids.len() as u64,
        flora.view().ledger.births,
        "every stand alive before the first tick is a founder"
    );

    let ticks = (seconds * cubarium_voxel::TICK_HZ as f64).round() as u64;
    let every = 100 * cubarium_voxel::TICK_HZ as u64;
    // The water budget is read against the state the founders were planted into, and then
    // against the previous progress interval, so both the whole run and each interval have
    // an in/out/storage line of their own.
    let start_mark = water_mark(&world, 0.0);
    let mut last_mark = start_mark;
    for tick in 0..ticks {
        world.step();
        flora.step(&mut world);
        if verbose && (tick + 1) % every == 0 {
            let v = flora.view();
            let mut line = format!(
                "  t {:>5.0} s: establishments {}, deaths {}",
                (tick + 1) as f64 * cubarium_voxel::DT,
                v.ledger.establishments,
                v.ledger.deaths
            );
            // Per species: how many stands, how waterlogged their root zones are, how
            // deep the water standing on them is, and how big the biggest seed bank is
            // against the germination threshold. The two gates and the two kill paths in
            // one line, so a run that ends with nothing says which of them did it.
            for species in Species::ALL {
                let all: Vec<_> = v.stands.iter().filter(|s| s.species == species).collect();
                let n = all.len().max(1) as f64;
                let stress = all.iter().map(|s| s.aeration_stress).sum::<f64>() / n;
                let depth = all
                    .iter()
                    .map(|s| {
                        world
                            .view()
                            .water_depth_m(s.site.x as i64, s.site.y, s.site.z)
                    })
                    .sum::<f64>()
                    / n;
                let sc = flora.config().species(species);
                let threshold = sc.alive_min / sc.propagule_split[0];
                let bank = v
                    .ground
                    .iter()
                    .map(|g| g.seed_organic(species))
                    .fold(0.0f64, f64::max);
                let donors = all
                    .iter()
                    .filter(|s| {
                        s.wood >= sc.donor_min
                            && s.reserve > sc.donor_reserve_floor * sc.reserve_cap * s.wood
                    })
                    .count();
                line.push_str(&format!(
                    "; {} {} stands ({donors} donors), stress {stress:.3}, pool {depth:.3} m, \
                     best bank {:.1}% of threshold",
                    all.len(),
                    species.name(),
                    100.0 * bank / threshold
                ));
            }
            println!("{line}");
            let mark = water_mark(&world, (tick + 1) as f64 * cubarium_voxel::DT);
            println!("    {}", water_budget_line(&last_mark, &mark));
            last_mark = mark;
        }
    }

    // Descendants: a living stand whose identity is not one of the founders'. Exact, and
    // not a site watch: it counts a founder replaced on its own site by its own species in
    // the tick it died.
    let mut descendants = [0usize; Species::COUNT];
    let mut founder_stands = [0usize; Species::COUNT];
    for stand in flora.view().stands {
        if founder_ids.contains(&stand.id) {
            founder_stands[stand.species.index()] += 1;
        } else {
            descendants[stand.species.index()] += 1;
        }
    }

    // The same predicate again, now: at observation rather than at introduction. Astra's
    // R5.2 — an eligible count is a reading of one moment, and carrying the warm-up's
    // reading into the result made it look like a habitat size.
    let eligible_at_end = eligible_sets(&world, &flora, &skyline, &mut sky);

    // Round 3: `alive` is the stands, and `occupied` is the stands plus the sites where
    // a species' seed bank is waiting — the old "establishing" half of `occupied`, which
    // is a cohort in the ground now and not a frozen stand.
    let mut occupied = std::array::from_fn(|_| Vec::new());
    let mut alive: [Vec<(u32, u32)>; Species::COUNT] = std::array::from_fn(|_| Vec::new());
    for stand in flora.view().stands {
        let key = (stand.site.x, stand.site.z);
        occupied[stand.species.index()].push(key);
        alive[stand.species.index()].push(key);
    }
    for g in flora.view().ground {
        for species in Species::ALL {
            if g.seed_organic(species) > 0.0 {
                occupied[species.index()].push((g.site.x, g.site.z));
            }
        }
    }
    for set in occupied.iter_mut().chain(alive.iter_mut()) {
        set.sort_unstable();
        set.dedup();
    }
    // The lowest quarter of this run's own skyline, by column.
    let cut = skyline.len() / 4;
    let mut low_quartile: Vec<(u32, u32)> = skyline[..cut].iter().map(|s| (s.x, s.z)).collect();
    low_quartile.sort_unstable();
    let mut high_quartile: Vec<(u32, u32)> = skyline[skyline.len() - cut..]
        .iter()
        .map(|s| (s.x, s.z))
        .collect();
    high_quartile.sort_unstable();

    let outcome = Outcome {
        seed,
        noise_seed,
        occupied,
        alive,
        low_quartile,
        high_quartile,
        eligible,
        eligible_at_end,
        own_founders,
        founders: planted,
        off_predicate,
        descendants,
        founder_stands,
        establishments: flora.view().ledger.establishments,
        deaths: flora.view().ledger.deaths,
    };
    if verbose {
        let end_mark = water_mark(&world, ticks as f64 * cubarium_voxel::DT);
        report(
            &world,
            &flora,
            &config,
            &skyline,
            seeded,
            ticks,
            seconds,
            basin_floor_m,
        );
        println!("whole run: {}", water_budget_line(&start_mark, &end_mark));
        print_columns(&outcome);
    }
    outcome
}

/// Every skyline column that passes each species' establishment predicate right now, as
/// sorted column keys. An instantaneous reading: the caller says when it took it.
///
/// One `SkyCache` is shared across the six species and reused by the caller across
/// observations, so a site's hemisphere ray is cast once while the terrain is unchanged; the
/// water, the aeration and the saprotroph's dead wood are read afresh on every call.
fn eligible_sets(
    world: &World,
    flora: &Flora,
    skyline: &[Site],
    sky: &mut SkyCache,
) -> [Vec<(u32, u32)>; Species::COUNT] {
    let view = world.view();
    let mut out: [Vec<(u32, u32)>; Species::COUNT] = std::array::from_fn(|_| Vec::new());
    for species in Species::ALL {
        let gates = flora
            .view()
            .establishment_gates_over(&view, skyline, species, sky);
        let mut set: Vec<(u32, u32)> = skyline
            .iter()
            .copied()
            .zip(gates)
            .filter(|(_, g)| g.passes())
            .map(|(s, _)| (s.x, s.z))
            .collect();
        set.sort_unstable();
        out[species.index()] = set;
    }
    out
}

/// Which gate refuses the columns a species cannot establish on, over the state the run
/// ended in, and how many eligible recipients each donor actually has inside its `hop`.
///
/// Every number here comes from the model's own predicate through
/// `cubarium_voxel_flora::establishment_gates`, whose `passes()` **is** `can_establish`:
/// no second approximate predicate, which is what package J deleted and what Astra's R5.2
/// asks to keep deleted. A column can fail several gates at once, so the counts overlap by
/// construction; the point is which of them is doing the work.
fn gate_diagnosis(world: &World, flora: &Flora, skyline: &[Site], when: &str, sky: &mut SkyCache) {
    let view = world.view();
    println!(
        "establishment gates {when} ({} skyline columns):",
        skyline.len()
    );
    for species in Species::ALL {
        let sc = flora.config().species(species);
        let gates = flora
            .view()
            .establishment_gates_over(&view, skyline, species, sky);
        let mut eligible = 0usize;
        let (mut no_soil, mut pore, mut aeration, mut depth, mut light) = (0, 0, 0, 0, 0);
        // Round 5b's sixth gate: dead wood in the mycelium box, and open by construction
        // for the five plants.
        let mut substrate = 0usize;
        let mut only_pore = 0usize;
        let mut only_aeration = 0usize;
        let mut only_light = 0usize;
        let mut mean_pore_sum = 0.0;
        let mut mean_pore_n = 0usize;
        for g in gates {
            if g.passes() {
                eligible += 1;
            }
            if !g.substrate_ok {
                substrate += 1;
            }
            if g.soil_voxels == 0 {
                no_soil += 1;
            }
            if let Some(mean) = g.mean_pore {
                mean_pore_sum += mean;
                mean_pore_n += 1;
            }
            let shut = [!g.pore_ok, !g.aeration_ok, !g.depth_ok, !g.light_ok];
            if shut[0] {
                pore += 1;
            }
            if shut[1] {
                aeration += 1;
            }
            if shut[2] {
                depth += 1;
            }
            if shut[3] {
                light += 1;
            }
            if shut.iter().filter(|&&f| f).count() == 1 {
                if shut[0] {
                    only_pore += 1;
                } else if shut[1] {
                    only_aeration += 1;
                } else if shut[3] {
                    only_light += 1;
                }
            }
        }
        let mean_pore = if mean_pore_n > 0 {
            mean_pore_sum / mean_pore_n as f64
        } else {
            f64::NAN
        };
        println!(
            "  {:>14}: {eligible} eligible; shut gates (a column can fail several): no soil in the root box {no_soil}, mean pore < {:.2} {pore}, saturated fraction > {:.2} {aeration}, water over {:.2} m {depth}, sky < {:.2} {light}{}",
            species.name(),
            sc.establish_pore_min,
            sc.establish_saturated_max,
            sc.drown_depth_m,
            sc.establish_light_min,
            match sc.trophic {
                Trophic::Photo => String::new(),
                Trophic::Saprotroph => format!(
                    ", dead wood in the mycelium box < {:.3} {substrate} (and **no light \
                     gate**: the sky column above is inert for this species)",
                    sc.establish_substrate_min
                ),
            }
        );
        println!(
            "  {:>14}: sole cause — pore alone {only_pore}, saturation alone {only_aeration}, light alone {only_light}; mean root-box pore over the {mean_pore_n} columns with soil {mean_pore:.3}",
            ""
        );
        // The dispersal side of eligibility: a donor's packages can only recruit where they
        // land, so what matters to it is the eligible faces inside its own hop.
        let donors: Vec<&cubarium_voxel_flora::Stand> = flora
            .view()
            .stands
            .iter()
            .filter(|s| s.species == species && s.wood >= sc.donor_min)
            .collect();
        let (mut candidates, mut ok) = (0usize, 0usize);
        for donor in &donors {
            let hop = sc.hop as i64;
            let mut seen: Vec<Site> = Vec::new();
            for dz in -hop..=hop {
                let z = donor.site.z as i64 + dz;
                if z < 0 || z >= view.config.depth as i64 {
                    continue;
                }
                for dx in -hop..=hop {
                    let x = donor.site.x as i64 + dx;
                    let Some(site) = cubarium_voxel_flora::highest_support(&view, x, z as u32)
                    else {
                        continue;
                    };
                    if site == donor.site || seen.contains(&site) {
                        continue;
                    }
                    seen.push(site);
                }
            }
            candidates += seen.len();
            ok += seen
                .iter()
                .filter(|s| passes(world, flora, species, **s))
                .count();
        }
        println!(
            "  {:>14}: {} donors, {candidates} candidate faces within hop {}, {ok} of them eligible ({:.1} per donor)",
            "",
            donors.len(),
            sc.hop,
            if donors.is_empty() {
                0.0
            } else {
                ok as f64 / donors.len() as f64
            }
        );
    }
}

/// Founders go where their own species could establish — wet enough for its roots,
/// aerated enough, bright enough, not under water it cannot stand in — and, among those
/// sites, where its own [`Habitat`] says. Without the establishment filter every founder of
/// a light-demanding species lands on bare sloping rock and dies in the first tick; without
/// the habitat rule the five species would all be planted in the same place and the run
/// would say nothing about any of them.
///
/// `species` is the list to plant and its order is only the order of the printed lines: the
/// species whose habitat is `UnderACrown` are planted **after** every other species,
/// because their rule reads the crowns the others make.
fn pick_founders(
    world: &World,
    flora: &Flora,
    skyline: &[Site],
    species: &[Species],
    verbose: bool,
) -> Vec<Founder> {
    let mut out: Vec<Founder> = Vec::new();
    let mut rounds: Vec<Vec<Species>> = vec![Vec::new(), Vec::new()];
    for &s in species {
        rounds[usize::from(planted_last(s))].push(s);
    }
    for round in rounds {
        for species in round {
            let habitat = habitat_of(species);
            let eligible: Vec<Site> = skyline
                .iter()
                .copied()
                .filter(|s| passes(world, flora, species, *s))
                .collect();
            let n = eligible.len();
            // No fallback (Astra R7.4). A species with nowhere to establish used to be
            // seeded anyway, on whatever its own ordering preferred among *every* skyline
            // site — which put founders on sites their own predicate refuses and made the
            // arm unreadable. If no contract site exists, that is the result: it is
            // printed, and nothing of that species is planted.
            if eligible.is_empty() {
                println!(
                    "{:>14}: 0 of {} skyline sites pass its establishment predicate at \
                     introduction, so it has no contract site and **none were planted** — a \
                     founder off its own predicate is not a founder of anything (Astra R7.4)",
                    species.name(),
                    skyline.len()
                );
                continue;
            }
            let ok = order_for(world, flora, species, habitat, eligible, &out);
            if ok.is_empty() {
                println!(
                    "{:>14}: {n} of {} skyline sites pass its establishment predicate, but its \
                     habitat rule {habitat:?} leaves **no candidate at all**, so none were \
                     planted (Astra R7.4)",
                    species.name(),
                    skyline.len()
                );
                continue;
            }
            // Columns another founder has already reserved, or that something is already
            // standing on, are dropped **before** the sample (Astra R7.1). `Command::Seed`
            // refuses an occupied site, so a sample that keeps them silently plants fewer
            // founders than it printed: that is what cost the round-4 velvetpad cohort two
            // of its eight, and it was read as a failed habitat trial.
            let free: Vec<Site> = ok
                .into_iter()
                .filter(|s| !out.iter().any(|f| f.x == s.x && f.z == s.z))
                .filter(|s| flora.view().stand_at(*s).is_none())
                .collect();
            if free.is_empty() {
                println!(
                    "{:>14}: {n} of {} skyline sites pass its establishment predicate, and its \
                     habitat rule {habitat:?} leaves no **unreserved** candidate, so none were \
                     planted (Astra R7.1)",
                    species.name(),
                    skyline.len()
                );
                continue;
            }
            let ok = free;
            let stride = (ok.len() / FOUNDERS_PER_SPECIES).max(1);
            let mut lo = u32::MAX;
            let mut hi = 0;
            let mut chosen: Vec<Site> = Vec::new();
            for site in ok.iter().step_by(stride).take(FOUNDERS_PER_SPECIES) {
                out.push(Founder {
                    species,
                    x: site.x,
                    z: site.z,
                });
                chosen.push(*site);
                lo = lo.min(site.y);
                hi = hi.max(site.y);
            }
            if verbose {
                println!(
                    "{:>14}: {n} of {} skyline sites pass its establishment predicate at \
                     introduction (50 s warm-up, before planting); habitat rule {habitat:?} over \
                     {} candidates; founders at y {lo}..{hi}",
                    species.name(),
                    skyline.len(),
                    ok.len()
                );
                let picked: Vec<(u32, u32)> = out
                    .iter()
                    .filter(|f| f.species == species)
                    .map(|f| (f.x, f.z))
                    .collect();
                println!("{:>14}: founder columns {picked:?}", species.name());
                // What each selected founder's own root zone and sky read at planting
                // (Astra R7.4). Springturf's saturated fraction here is the point: a site
                // that passes its gate cannot be over `establish_saturated_max`, so every
                // founder starts at zero aeration stress by construction and any later
                // stress is a later condition and not this placement.
                for site in &chosen {
                    println!(
                        "{:>14}: founder ({:>3},{:>3}) y {:>2} — {}",
                        species.name(),
                        site.x,
                        site.z,
                        site.y,
                        gate_line(world, flora, species, *site)
                    );
                }
            }
        }
    }
    out
}

/// Every founder by identity at observation: where it stands, what its root box reads now,
/// and the stress it is actually carrying.
///
/// Astra's R7.4 asked for this record so that a cohort's stress is reported **per stand**
/// instead of inferred from the placement rule. For springturf the inference cannot work at
/// all: a site that passes its gate has saturated fraction at most
/// `establish_saturated_max`, so `aeration_target` is zero at planting for every founder and
/// any later stress is a later root-zone condition.
fn founder_identity_report(world: &World, flora: &Flora, founders: &[(u64, Species, Site)]) {
    println!(
        "founders by identity at observation — saturated fraction is the root box now, stress \
         is the stand's own `aeration_stress`:"
    );
    println!(
        "{:>14} {:>5} {:>14} {:>9} {:>8} {:>8} {:>8} {:>8}",
        "species", "id", "column", "sat.frac", "stress", "moist", "light", "wood"
    );
    let v = flora.view();
    for &(id, species, site) in founders {
        let g = flora
            .view()
            .establishment_gates(&world.view(), site, species);
        let at = format!("({},{}) y{}", site.x, site.z, site.y);
        match v.stands.iter().find(|s| s.id == id) {
            Some(s) => println!(
                "{:>14} {id:>5} {at:>14} {:>9.3} {:>8.3} {:>8.3} {:>8.3} {:>8.4}",
                species.name(),
                g.saturated_fraction,
                s.aeration_stress,
                s.moisture,
                s.light,
                s.wood
            ),
            None => println!(
                "{:>14} {id:>5} {at:>14} {:>9.3} {:>8} {:>8} {:>8} {:>8}",
                species.name(),
                g.saturated_fraction,
                "gone",
                "-",
                "-",
                "-"
            ),
        }
    }
}

/// Why a run had no second generation, if it had none: per species, how many sites hold a
/// bank, how big the biggest one is against the germination threshold, and — of the sites
/// whose bank is over the threshold — how many the model's own predicate would let
/// germinate. Two independent gates, and this says which one is shut.
fn germination_diagnosis(world: &World, flora: &Flora) {
    let view = flora.view();
    // Reproduction first: what the rate asked for, what the reserves could pay, and what
    // actually left as packages. Astra's R4.4 — a rate that is not the binding constraint
    // cannot be raised into income, and this line says which of the three it is.
    println!("reproductive flux (organic matter, net of construction, cumulative):");
    for species in Species::ALL {
        let i = species.index();
        let (req, fund, land) = (
            view.ledger.propagule_requested[i],
            view.ledger.propagule_funded[i],
            view.ledger.propagule_landed[i],
        );
        let parcels: f64 = view
            .stands
            .iter()
            .filter(|s| s.species == species)
            .map(|s| s.parcel)
            .sum();
        let sc = flora.config().species(species);
        let package = sc.alive_min / sc.propagule_split[0];
        println!(
            "  {:>14}: requested {req:.5}, funded {fund:.5} ({:.1} %), landed {land:.5} \
             ({:.1} packages of {package:.4}); {parcels:.5} standing in parcels on {} stands",
            species.name(),
            if req > 0.0 { 100.0 * fund / req } else { 0.0 },
            land / package,
            view.stands.iter().filter(|s| s.species == species).count()
        );
    }
    println!("germination gates:");
    for species in Species::ALL {
        let sc = flora.config().species(species);
        let threshold = sc.alive_min / sc.propagule_split[0];
        let banks: Vec<(Site, f64)> = view
            .ground
            .iter()
            .filter(|g| g.seed_organic(species) > 0.0)
            .map(|g| (g.site, g.seed_organic(species)))
            .collect();
        let biggest = banks.iter().map(|&(_, o)| o).fold(0.0f64, f64::max);
        let mean = if banks.is_empty() {
            0.0
        } else {
            banks.iter().map(|&(_, o)| o).sum::<f64>() / banks.len() as f64
        };
        let over: Vec<Site> = banks
            .iter()
            .filter(|&&(_, o)| o >= threshold)
            .map(|&(s, _)| s)
            .collect();
        let over_and_ok = over
            .iter()
            .filter(|&&s| passes(world, flora, species, s))
            .count();
        let predicate_ok = banks
            .iter()
            .filter(|&&(s, _)| passes(world, flora, species, s))
            .count();
        // Vacancy is the third constraint and it used not to be printed at all, so a
        // fractional bank at observation could be read as a refused predicate when an
        // occupied site is just as consistent with it (Astra R7.5). The germinable count is
        // the conjunction: a whole package, a vacant site, and the predicate open.
        let vacant = banks
            .iter()
            .filter(|&&(s, _)| view.stand_at(s).is_none())
            .count();
        // And **which** gate is shutting a waiting bank, per site, through the same
        // `Gates`: "0 of 3 banked sites pass the predicate" says that one of them did
        // without saying which, and for a saprotroph the answer is usually its substrate —
        // a package lands on the *highest support* of a column inside the donor's hop, and
        // that face's own mycelium box need not hold the log the donor is standing on.
        let refused: Vec<Site> = banks
            .iter()
            .map(|&(s, _)| s)
            .filter(|&s| !passes(world, flora, species, s))
            .collect();
        let germinable = over
            .iter()
            .filter(|&&s| view.stand_at(s).is_none() && passes(world, flora, species, s))
            .count();
        println!(
            "  {:>14}: threshold {threshold:.4}; {} banked sites, mean {mean:.5}, biggest \
             {biggest:.5} ({:.1}% of threshold); {} over threshold, {over_and_ok} of those \
             pass the model predicate; {predicate_ok} of all {} banked sites pass it; \
             {vacant} of them are vacant; {germinable} are over threshold, vacant and \
             passing at once",
            species.name(),
            banks.len(),
            100.0 * biggest / threshold,
            over.len(),
            banks.len()
        );
        for site in refused.iter().take(3) {
            println!(
                "  {:>14}: banked site ({},{}) y{} is refused — {}",
                "",
                site.x,
                site.z,
                site.y,
                gate_line(world, flora, species, *site)
            );
        }
    }
}

/// How many of another run's founder columns this world no longer qualifies.
fn off_count(world: &World, flora: &Flora, founders: &[Founder]) -> usize {
    founders
        .iter()
        .filter(
            |f| match cubarium_voxel_flora::highest_support(&world.view(), f.x as i64, f.z) {
                None => true,
                Some(site) => !passes(world, flora, f.species, site),
            },
        )
        .count()
}

#[allow(clippy::too_many_arguments)]
fn report(
    world: &World,
    flora: &Flora,
    config: &VoxelConfig,
    skyline: &[Site],
    founders: usize,
    ticks: u64,
    seconds: f64,
    basin_floor_m: f64,
) {
    let view = flora.view();
    println!(
        "two_producers: {}x{}x{} seed {} noise_seed {}, rain {} m/s, outlet open",
        config.width,
        config.height,
        config.depth,
        config.seed,
        config.noise_seed,
        config.rain_m_per_s
    );
    println!(
        "water table charged to {:.2} m (basin floor {:.2} m + 1 m), now at {:.2} m",
        config.initial_aquifer_head_m,
        basin_floor_m,
        world.aquifer_head_m()
    );
    // The experiment condition, as an arithmetic statement rather than a claim: nominal
    // rain against the only exit the world has at these settings.
    let footprint_m2 = config.width as f64 * config.voxel_m * config.depth as f64 * config.voxel_m;
    println!(
        "forcing: rain {:.5} m/s over {footprint_m2:.0} m2 is a nominal {:.4} m3/s in, against \
         an outlet of {:.4} m3/s and evaporation {:.5} m/s — {:.0} % of the outlet-only \
         ceiling {:.6} m/s",
        config.rain_m_per_s,
        config.rain_m_per_s * footprint_m2,
        config.outlet_m3_per_s,
        config.evaporation_m_per_s,
        100.0 * config.rain_m_per_s * footprint_m2 / config.outlet_m3_per_s,
        config.outlet_m3_per_s / footprint_m2
    );
    println!(
        "{WARMUP_TICKS} warm-up ticks, then {ticks} coupled ticks ({seconds:.0} s); {founders} \
         founders at {FOUNDER_FRACTION} of each species' own wood_max"
    );

    for species in Species::ALL {
        let all: Vec<_> = view
            .stands
            .iter()
            .filter(|s| s.species == species)
            .collect();
        let n = all.len();
        let wood: f64 = all.iter().map(|s| s.wood).sum();
        let light: f64 = all.iter().map(|s| s.light).sum::<f64>() / n.max(1) as f64;
        let moisture: f64 = all.iter().map(|s| s.moisture).sum::<f64>() / n.max(1) as f64;
        // The seed bank: how many sites are waiting, and how much organic matter waits.
        let banks = view
            .ground
            .iter()
            .filter(|g| g.seed_organic(species) > 0.0)
            .count();
        let banked: f64 = view.ground.iter().map(|g| g.seed_organic(species)).sum();
        println!(
            "{:>14}: {n} stands, wood {wood:.4}, mean light {light:.3}, moisture {moisture:.3}; \
             seed bank on {banks} sites holding {banked:.5}",
            species.name(),
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
            .map(|&species| {
                let n = view
                    .stands
                    .iter()
                    .filter(|s| s.species == species && s.site.y >= lo && s.site.y < hi)
                    .count();
                format!("{} {n}", species.name())
            })
            .collect();
        println!(
            "  q{} y {lo}..{}: {} columns, {}",
            q + 1,
            hi.min(999),
            columns,
            counts.join(", ")
        );
    }

    let l = view.ledger;
    println!(
        "ledger: fixed_in {:.6} respired_out {:.6} light_in {:.6} heat_out {:.6} \
         transpired {:.6} m3 deaths {} establishments {}",
        l.fixed_in,
        l.respired_out,
        l.light_in,
        l.heat_out,
        l.transpired_m3,
        l.deaths,
        l.establishments
    );
    println!(
        "residuals: organic {:.3e} mineral {:.3e} energy {:.3e} (stocks: organic {:.4} \
         mineral {:.4} energy {:.4})",
        view.organic() - l.expected_organic(),
        view.mineral() - l.expected_mineral(),
        view.energy() - l.expected_energy(),
        view.organic(),
        view.mineral(),
        view.energy()
    );
    germination_diagnosis(world, flora);
    let mut sky = SkyCache::new();
    gate_diagnosis(world, flora, skyline, "at observation", &mut sky);
    let water = world.view().stored_m3() - world.view().ledger.expected_stored();
    println!(
        "core water: stored {:.4} m3, residual {:.3e}, transpiration_out {:.6} m3 (flora says {:.6})",
        world.view().stored_m3(),
        water,
        world.view().ledger.transpiration_out,
        l.transpired_m3
    );
}

/// The occupied skyline columns themselves, so a run can be compared with another by
/// eye as well as by a number.
fn print_columns(outcome: &Outcome) {
    for species in Species::ALL {
        let all = &outcome.occupied[species.index()];
        let alive = &outcome.alive[species.index()];
        let (d, f) = (
            outcome.descendants[species.index()],
            outcome.founder_stands[species.index()],
        );
        println!(
            "{:>14}: {} occupied columns ({} with a living stand), lowest skyline quartile {:.2}; \
             {d} of {} living stands are descendants ({}), {f} still the founder",
            species.name(),
            all.len(),
            alive.len(),
            quartile_fraction(all, &outcome.low_quartile),
            d + f,
            if d + f == 0 {
                "n/a".to_string()
            } else {
                format!("{:.2}", d as f64 / (d + f) as f64)
            }
        );
        println!("    {}", columns(all));
    }
}

/// The set, one entry per `x` with its `z` values collapsed into ranges: the full set,
/// short enough to read and to diff between two runs by eye.
fn columns(set: &[(u32, u32)]) -> String {
    if set.is_empty() {
        return "(none)".to_string();
    }
    let mut out: Vec<String> = Vec::new();
    let mut at = 0;
    while at < set.len() {
        let x = set[at].0;
        let mut end = at;
        while end < set.len() && set[end].0 == x {
            end += 1;
        }
        let zs = &set[at..end];
        let mut parts: Vec<String> = Vec::new();
        let mut i = 0;
        while i < zs.len() {
            let start = zs[i].1;
            let mut last = start;
            while i + 1 < zs.len() && zs[i + 1].1 == last + 1 {
                i += 1;
                last = zs[i].1;
            }
            parts.push(if last == start {
                format!("{start}")
            } else {
                format!("{start}-{last}")
            });
            i += 1;
        }
        out.push(format!("x{x}:z{}", parts.join(",")));
        at = end;
    }
    out.join(" ")
}

/// `|A ∩ B| / |A ∪ B|`, and `None` for two empty sets — which is not an overlap of one.
fn jaccard(a: &[(u32, u32)], b: &[(u32, u32)]) -> Option<f64> {
    let inter = a.iter().filter(|k| b.binary_search(k).is_ok()).count();
    let union = a.len() + b.len() - inter;
    if union == 0 {
        None
    } else {
        Some(inter as f64 / union as f64)
    }
}

fn show(v: Option<f64>) -> String {
    v.map_or_else(|| "n/a".to_string(), |x| format!("{x:.3}"))
}

/// `d / (d + f)` as "d/total (fraction)", and "n/a" for a species with no living stand.
fn frac(d: usize, f: usize) -> String {
    if d + f == 0 {
        return "0/0 (n/a)".to_string();
    }
    format!("{d}/{} ({:.2})", d + f, d as f64 / (d + f) as f64)
}

fn quartile_fraction(set: &[(u32, u32)], low: &[(u32, u32)]) -> f64 {
    if set.is_empty() {
        return f64::NAN;
    }
    set.iter().filter(|k| low.binary_search(k).is_ok()).count() as f64 / set.len() as f64
}

/// One species' founder columns, sorted for `jaccard`.
fn founder_columns(founders: &[Founder], species: Species) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = founders
        .iter()
        .filter(|f| f.species == species)
        .map(|f| (f.x, f.z))
        .collect();
    out.sort_unstable();
    out
}

/// The columns of a set that no founder was planted in: the part of the patch the run
/// itself produced, since the founder columns are identical in all three runs by
/// construction and would otherwise inflate every overlap.
fn spread(set: &[(u32, u32)], founders: &[Founder]) -> Vec<(u32, u32)> {
    set.iter()
        .copied()
        .filter(|&(x, z)| !founders.iter().any(|f| f.x == x && f.z == z))
        .collect()
}

/// The decisive experiment: the same landform under two noise seeds, against a different
/// landform as a control.
fn compare(args: &[String]) {
    let seconds: f64 = arg(args, 0).unwrap_or(300.0);
    let seed: u64 = arg(args, 1).unwrap_or(1);
    let noise_a: u64 = arg(args, 2).unwrap_or(101);
    let noise_b: u64 = arg(args, 3).unwrap_or(202);
    let control_seed: u64 = arg(args, 4).unwrap_or(7);
    // Any pair, by name. The default is the round-3 pair, so an invocation written before
    // round 4 still runs the experiment it ran then.
    let pair = [
        species_arg(args, 5, Species::Bloomcrown),
        species_arg(args, 6, Species::Umbrellafrond),
    ];

    println!(
        "=== base: seed {seed}, noise_seed {noise_a}, pair {} and {} ===",
        pair[0].name(),
        pair[1].name()
    );
    let base = run(seconds, seed, noise_a, &pair, None, true);
    println!("\n=== re-drawn noise: seed {seed}, noise_seed {noise_b} (same landform) ===");
    let alt = run(seconds, seed, noise_b, &pair, Some(&base.founders), true);
    println!("\n=== control: seed {control_seed}, noise_seed {noise_a} (another landform) ===");
    let ctl = run(
        seconds,
        control_seed,
        noise_a,
        &pair,
        Some(&base.founders),
        true,
    );

    println!("\n=== the comparison ===");
    println!(
        "seconds {seconds:.0}; base seed {} noise {}; re-drawn noise {}; control seed {} \
         (founder columns identical in all three: {} of them, off-predicate {} in the \
         re-draw and {} in the control)",
        base.seed,
        base.noise_seed,
        alt.noise_seed,
        ctl.seed,
        base.founders.len(),
        alt.off_predicate,
        ctl.off_predicate
    );
    // The landform itself, before any plant or any drop of water: if the ridges and the
    // hollows do not stay put under a re-drawn noise, nothing that follows them can.
    println!(
        "landform: the skyline's lowest quartile overlaps {} with the re-drawn noise and {} \
         with the control; its highest quartile {} and {}",
        show(jaccard(&base.low_quartile, &alt.low_quartile)),
        show(jaccard(&base.low_quartile, &ctl.low_quartile)),
        show(jaccard(&base.high_quartile, &alt.high_quartile)),
        show(jaccard(&base.high_quartile, &ctl.high_quartile))
    );
    println!(
        "life cycle per arm: base establishments {} deaths {}; re-drawn noise {} / {}; \
         control {} / {}",
        base.establishments,
        base.deaths,
        alt.establishments,
        alt.deaths,
        ctl.establishments,
        ctl.deaths
    );
    for species in pair {
        let i = species.index();
        let (b, a, c) = (&base.occupied[i], &alt.occupied[i], &ctl.occupied[i]);
        println!("{}:", species.name());
        println!(
            "  descendant stands / living stands: base {}, re-drawn noise {}, control {}",
            frac(base.descendants[i], base.founder_stands[i]),
            frac(alt.descendants[i], alt.founder_stands[i]),
            frac(ctl.descendants[i], ctl.founder_stands[i])
        );
        println!(
            "  occupied columns: base {}, re-drawn noise {}, control {}",
            b.len(),
            a.len(),
            c.len()
        );
        println!(
            "  Jaccard base vs re-drawn noise {}   base vs control {}",
            show(jaccard(b, a)),
            show(jaccard(b, c))
        );
        let (bs, as_, cs) = (
            spread(b, &base.founders),
            spread(a, &base.founders),
            spread(c, &base.founders),
        );
        println!(
            "  founders excluded ({} / {} / {} columns): noise {}   control {}",
            bs.len(),
            as_.len(),
            cs.len(),
            show(jaccard(&bs, &as_)),
            show(jaccard(&bs, &cs))
        );
        let (bl, al, cl) = (&base.alive[i], &alt.alive[i], &ctl.alive[i]);
        println!(
            "  living stands only ({} / {} / {} columns): noise {}   control {}",
            bl.len(),
            al.len(),
            cl.len(),
            show(jaccard(bl, al)),
            show(jaccard(bl, cl))
        );
        println!(
            "  in the lowest skyline quartile: base {:.2}, re-drawn noise {:.2}, control {:.2}",
            quartile_fraction(b, &base.low_quartile),
            quartile_fraction(a, &alt.low_quartile),
            quartile_fraction(c, &ctl.low_quartile)
        );
        // The patch is bounded by founders held identical on purpose, so the eligible set
        // is what answers the sketch's question without them in the way — read twice,
        // because it is a reading of a moment and not a habitat size (Astra R5.2).
        let (be, ae, ce) = (&base.eligible[i], &alt.eligible[i], &ctl.eligible[i]);
        println!(
            "  eligible at introduction (50 s warm-up, before planting) ({} / {} / {} of {} \
             columns): noise {}   control {}",
            be.len(),
            ae.len(),
            ce.len(),
            base.low_quartile.len() * 4,
            show(jaccard(be, ae)),
            show(jaccard(be, ce))
        );
        let (bo, ao, co) = (
            &base.eligible_at_end[i],
            &alt.eligible_at_end[i],
            &ctl.eligible_at_end[i],
        );
        println!(
            "  eligible at observation (the last tick of the run, with the plants in it) \
             ({} / {} / {}): noise {}   control {}",
            bo.len(),
            ao.len(),
            co.len(),
            show(jaccard(bo, ao)),
            show(jaccard(bo, co))
        );
        println!(
            "  the eligible count changed by {} / {} / {} columns between the two readings — a \
             net change in the size of each set, not a count of columns that moved",
            (bo.len() as i64) - (be.len() as i64),
            (ao.len() as i64) - (ae.len() as i64),
            (co.len() as i64) - (ce.len() as i64)
        );
        println!(
            "  eligible at introduction, in the lowest skyline quartile: base {:.2}, \
             re-drawn noise {:.2}, control {:.2}",
            quartile_fraction(be, &base.low_quartile),
            quartile_fraction(ae, &alt.low_quartile),
            quartile_fraction(ce, &ctl.low_quartile)
        );
        // Where the selection rule would have put this species if each run had chosen
        // freely. It comes out near zero for both the noise pair and the control, and
        // that is a property of the *rule*, not of the terrain: it takes a strided
        // sample of the eligible sites sorted by height, and a one-voxel wobble reorders
        // near-ties, so the sample lands elsewhere while the set it samples barely moves
        // (compare the habitat line above). It is printed because it is the reason the
        // founders have to be held identical for the comparison to mean anything.
        let (bf, af, cf) = (
            founder_columns(&base.own_founders, species),
            founder_columns(&alt.own_founders, species),
            founder_columns(&ctl.own_founders, species),
        );
        println!(
            "  the strided founder sample its own rule would pick ({} columns, order-sensitive \
             by construction): noise {}   control {}",
            bf.len(),
            show(jaccard(&bf, &af)),
            show(jaccard(&bf, &cf))
        );
    }
}

// ===================================================================== Chesson
//
// One direction at a time: a coexistence mechanism has to let the *rare* species increase
// while the other stands. Round 3's corrections 1 and 2 are what give it anywhere to bite
// — a seed bank that can wait for a gap, and a saturated root zone that costs the
// intolerant species something.
//
// What this probe is **not**, as of round 3b (Astra R4.7): it is not a reading of the
// invasion criterion, and it prints "recruitment observed / not observed within T s"
// rather than a verdict. It has no stationary resident — the resident's own habitat moves
// as the world wets up — and no measured single-founder generation time, so a positive
// descendant count after an arbitrary fill and probe says a birth happened, nothing more.
// Before it is rerun it needs a **positive control**: the same founder treatment
// replacing itself under the same forcing, with the competitor absent, and a duration
// taken from that control's own full reproduction time. That design is an open item in
// `design/7_Research/voxel-round3-experiment-2026-09-16.md`, and round 3b deliberately
// does not rerun the probe.

fn chesson(args: &[String]) {
    let fill: f64 = arg(args, 0).unwrap_or(1000.0);
    let probe: f64 = arg(args, 1).unwrap_or(1000.0);
    let seed: u64 = arg(args, 2).unwrap_or(1);
    let noise_seed: u64 = arg(args, 3).unwrap_or(101);
    // Any pair, by name, in both directions. The default is the round-3 pair.
    let a = species_arg(args, 4, Species::Bloomcrown);
    let b = species_arg(args, 5, Species::Umbrellafrond);
    assert_ne!(a, b, "an invasion needs two different species");
    println!(
        "chesson: seed {seed} noise_seed {noise_seed}; {} and {}; {fill:.0} s of the resident \
         alone, then one founder of the newcomer and {probe:.0} s",
        a.name(),
        b.name()
    );
    for (resident, newcomer) in [(a, b), (b, a)] {
        invasion(fill, probe, seed, noise_seed, resident, newcomer);
    }
}

/// One direction of the probe.
fn invasion(
    fill: f64,
    probe: f64,
    seed: u64,
    noise_seed: u64,
    resident: Species,
    newcomer: Species,
) {
    println!(
        "\n=== resident {} , newcomer {} ===",
        resident.name(),
        newcomer.name()
    );
    let mut world = prepared_world(seed, noise_seed);
    let skyline = skyline_of(&world);
    let mut flora = Flora::new(FloraConfig::default());

    // The resident alone, on a strided sample of its own habitat: the same rule and the
    // same count `run` uses, so the resident's stand is as thick as the main experiment's.
    let ok = habitat(&world, &flora, &skyline, resident);
    let stride = (ok.len() / FOUNDERS_PER_SPECIES).max(1);
    let mut planted = 0;
    for site in ok.iter().step_by(stride).take(FOUNDERS_PER_SPECIES) {
        if flora.apply(
            &world,
            Command::Seed {
                x: site.x as i64,
                z: site.z,
                species: resident,
                wood: founder_wood(flora.config().species(resident)),
            },
        ) {
            planted += 1;
        }
    }
    println!(
        "  {} founders of the resident on {} of {} skyline columns eligible at introduction",
        planted,
        ok.len(),
        skyline.len()
    );
    step_coupled(&mut flora, &mut world, fill);
    let resident_filled = count(&flora, resident);
    let banks_filled = flora
        .view()
        .ground
        .iter()
        .filter(|g| g.seed_organic(resident) > 0.0)
        .count();
    let (est0, deaths0) = (
        flora.view().ledger.establishments,
        flora.view().ledger.deaths,
    );
    println!(
        "  after {fill:.0} s alone: {resident_filled} resident stands, seed bank on \
         {banks_filled} sites, establishments {est0}, deaths {deaths0}"
    );

    // One founder of the newcomer, in the best site its own habitat still offers that the
    // resident is not standing on.
    let ok = habitat(&world, &flora, &skyline, newcomer);
    let target = ok
        .iter()
        .copied()
        .find(|s| flora.view().stand_at(*s).is_none());
    let Some(target) = target else {
        println!("  the newcomer has no free habitat site at all: no invasion to test");
        return;
    };
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: target.x as i64,
            z: target.z,
            species: newcomer,
            wood: founder_wood(flora.config().species(newcomer)),
        }
    ));
    // The founder's identity, from the ledger's birth counter: the one stand this probe is
    // allowed to call a founder.
    let founder_id = flora.view().stand_at(target).expect("just planted").id;
    println!(
        "  one {} founder at x{} z{} y{} ({} sites eligible for it now and free of the \
         resident)",
        newcomer.name(),
        target.x,
        target.z,
        target.y,
        ok.iter()
            .filter(|s| flora.view().stand_at(**s).is_none())
            .count()
    );

    // Every newcomer identity seen alive at the end of any tick, so a descendant that is
    // born and dies inside the window is still counted as the birth it was. Astra's R5.4:
    // reading only the final living stands reported "recruitment NOT OBSERVED" over a real
    // birth, and the ledger's own establishment delta cannot be used instead because it
    // includes the resident's births too.
    let mut seen: Vec<u64> = Vec::new();
    let ticks = (probe * cubarium_voxel::TICK_HZ as f64).round() as u64;
    for _ in 0..ticks {
        world.step();
        flora.step(&mut world);
        for stand in flora.view().stands.iter().filter(|s| s.species == newcomer) {
            if stand.id != founder_id && !seen.contains(&stand.id) {
                seen.push(stand.id);
            }
        }
    }

    let view = flora.view();
    let newcomer_stands = count(&flora, newcomer);
    // By identity, not by site: anything of the newcomer's that is not the founder itself
    // germinated during the probe, including a stand that took the founder's own site in
    // the tick the founder died.
    let descendants = view
        .stands
        .iter()
        .filter(|s| s.species == newcomer && s.id != founder_id)
        .count();
    let births = seen.len();
    let lost = births - descendants;
    let founder_gone = !view.stands.iter().any(|s| s.id == founder_id);
    let banks = view
        .ground
        .iter()
        .filter(|g| g.seed_organic(newcomer) > 0.0)
        .count();
    let banked: f64 = view.ground.iter().map(|g| g.seed_organic(newcomer)).sum();
    println!(
        "  after {probe:.0} s of invasion: newcomer {newcomer_stands} stands ({descendants} of \
         them descendants), seed bank on {banks} sites holding {banked:.5}; resident \
         {resident_filled} -> {}",
        count(&flora, resident)
    );
    println!(
        "  establishments {} -> {}, deaths {} -> {}; the founder itself {}",
        est0,
        view.ledger.establishments,
        deaths0,
        view.ledger.deaths,
        if founder_gone {
            "died during the probe"
        } else {
            "survived the whole probe"
        }
    );
    // Not a verdict on coexistence (Astra R4.7). One birth, or a founder surviving, is not
    // an invasion: the criterion needs the rare species to increase through complete
    // generations while the resident persists, and this probe has neither a stationary
    // resident nor a measured single-founder generation time. It reports what it saw —
    // and reports the three numbers separately, because a window that ends with no living
    // descendant is not a window with no recruitment in it (R5.4).
    println!(
        "  recruitment {} within {probe:.0} s: {births} newcomer birth(s) by identity, \
         {lost} of them dead before the end, {descendants} surviving descendant(s)",
        if births > 0 {
            "OBSERVED"
        } else {
            "NOT OBSERVED"
        }
    );
}

// =================================================================== the community run
//
// Round 4's smoke run: one founder cohort of every species on its own contract habitat,
// one duration, no arms. **Not a study.** There is no control, no replication, no
// stationary resident and no measured generation time, so what it can report is what five
// presets did in one world for one duration and which gate was shutting for each of them.
// Astra's R5.2 applies to every eligible count in it: a count is a reading of the moment it
// was taken, and never a settled habitat.

/// Every stand identity this run has seen alive at the end of a tick, per species, sorted
/// so a lookup is a binary search and no hash iteration can reach the model.
///
/// A birth that dies inside the run is still the birth it was, which is Astra's R5.4:
/// reading only the final living stands reported "no recruitment" over a real one. Death
/// precedes germination inside a tick, so a newborn is always alive at the end of its own
/// birth tick and no birth can slip past this.
struct Seen([Vec<u64>; Species::COUNT]);

impl Seen {
    fn new() -> Seen {
        Seen(std::array::from_fn(|_| Vec::new()))
    }

    fn note(&mut self, species: Species, id: u64) {
        let v = &mut self.0[species.index()];
        if let Err(i) = v.binary_search(&id) {
            v.insert(i, id);
        }
    }

    fn len(&self, species: Species) -> usize {
        self.0[species.index()].len()
    }
}

fn community(args: &[String]) {
    let seconds: f64 = arg(args, 0).unwrap_or(400.0);
    let seed: u64 = arg(args, 1).unwrap_or(1);
    let noise_seed: u64 = arg(args, 2).unwrap_or(0);
    let started = std::time::Instant::now();

    let mut world = prepared_world(seed, noise_seed);
    let skyline = skyline_of(&world);
    let mut flora = Flora::new(FloraConfig::default());
    // One cache for the introduction and the observation sweep: the terrain is unchanged.
    let mut sky = SkyCache::new();
    let config = world.config().clone();
    println!(
        "community: {}x{}x{} seed {} noise_seed {}, rain {} m/s, outlet open; {WARMUP_TICKS} \
         warm-up ticks (50 s) then {seconds:.0} coupled seconds, so the world ends at {} s",
        config.width,
        config.height,
        config.depth,
        config.seed,
        config.noise_seed,
        config.rain_m_per_s,
        50.0 + seconds
    );
    // The logs first: a fresh world holds no dead wood, so the glowcap's substrate gate is
    // shut everywhere until they are down, and `pick_founders` reads the real gate.
    let logs = lay_declared_logs(&world, &mut flora, &skyline);
    println!(
        "the six contract habitats, as founder-placement rules of this harness and not \
         model rules:"
    );
    for species in Species::ALL {
        let sc = flora.config().species(species);
        println!(
            "  {:>14}: {:?}, {FOUNDERS_PER_SPECIES} founders at wood {:.3} (half its own \
             wood_max), hop {}, package {:.4}",
            species.name(),
            habitat_of(species),
            founder_wood(sc),
            sc.hop,
            sc.alive_min / sc.propagule_split[0]
        );
    }

    let eligible = eligible_sets(&world, &flora, &skyline, &mut sky);
    println!("\n--- at introduction (after the warm-up, before any plant acted) ---");
    gate_diagnosis(&world, &flora, &skyline, "at introduction", &mut sky);
    let founders = pick_founders(&world, &flora, &skyline, &Species::ALL, true);
    let mut planted = [0usize; Species::COUNT];
    let mut seen = Seen::new();
    for f in &founders {
        let wood = founder_wood(flora.config().species(f.species));
        if flora.apply(
            &world,
            Command::Seed {
                x: f.x as i64,
                z: f.z,
                species: f.species,
                wood,
            },
        ) {
            planted[f.species.index()] += 1;
        }
    }
    let founder_ids: Vec<u64> = flora.view().stands.iter().map(|s| s.id).collect();
    // Identity, species and site of every founder, for the per-stand record at observation
    // (Astra R7.4).
    let founder_rows: Vec<(u64, Species, Site)> = flora
        .view()
        .stands
        .iter()
        .map(|s| (s.id, s.species, s.site))
        .collect();
    assert_eq!(
        founder_ids.len() as u64,
        flora.view().ledger.births,
        "every stand alive before the first tick is a founder"
    );
    for stand in flora.view().stands {
        seen.note(stand.species, stand.id);
    }

    let ticks = (seconds * cubarium_voxel::TICK_HZ as f64).round() as u64;
    let every = 100 * cubarium_voxel::TICK_HZ as u64;
    let start_mark = water_mark(&world, 0.0);
    let mut last_mark = start_mark;
    println!();
    for tick in 0..ticks {
        world.step();
        flora.step(&mut world);
        for stand in flora.view().stands {
            seen.note(stand.species, stand.id);
        }
        if (tick + 1) % every == 0 {
            let v = flora.view();
            let mut line = format!(
                "  t {:>5.0} s: establishments {}, deaths {}",
                (tick + 1) as f64 * cubarium_voxel::DT,
                v.ledger.establishments,
                v.ledger.deaths
            );
            for species in Species::ALL {
                let n = v.stands.iter().filter(|s| s.species == species).count();
                let sc = flora.config().species(species);
                let threshold = sc.alive_min / sc.propagule_split[0];
                let bank = v
                    .ground
                    .iter()
                    .map(|g| g.seed_organic(species))
                    .fold(0.0f64, f64::max);
                line.push_str(&format!(
                    "; {} {n} ({:.0}% bank)",
                    species.name(),
                    100.0 * bank / threshold
                ));
            }
            println!("{line}");
            let mark = water_mark(&world, (tick + 1) as f64 * cubarium_voxel::DT);
            println!("    {}", water_budget_line(&last_mark, &mark));
            last_mark = mark;
        }
    }

    // Per species, by identity: what was planted, what is standing, how much of it is a
    // descendant, how many births this run ever saw, and how many of those are gone. The
    // ledger's own `establishments` and `deaths` have no species in them, so these are the
    // per-species figures and they are counted from `Stand::id` alone.
    println!("\n--- at observation (the last tick of the run) ---");
    let v = flora.view();
    println!(
        "{:>14}  {:>8} {:>7} {:>7} {:>7} {:>7} {:>7} {:>8} {:>7} {:>7} {:>7} {:>6}",
        "species",
        "founders",
        "alive",
        "found.",
        "desc.",
        "births",
        "deaths",
        "wood",
        "light",
        "moist",
        "stress",
        "banks"
    );
    for species in Species::ALL {
        let all: Vec<_> = v.stands.iter().filter(|s| s.species == species).collect();
        let n = all.len().max(1) as f64;
        let still = all.iter().filter(|s| founder_ids.contains(&s.id)).count();
        let desc = all.len() - still;
        let born = seen.len(species) - planted[species.index()];
        let gone = seen.len(species) - all.len();
        let banks = v
            .ground
            .iter()
            .filter(|g| g.seed_organic(species) > 0.0)
            .count();
        println!(
            "{:>14}  {:>8} {:>7} {:>7} {:>7} {:>7} {:>7} {:>8.4} {:>7.3} {:>7.3} {:>7.3} {:>6}",
            species.name(),
            planted[species.index()],
            all.len(),
            still,
            desc,
            born,
            gone,
            all.iter().map(|s| s.wood).sum::<f64>(),
            all.iter().map(|s| s.light).sum::<f64>() / n,
            all.iter().map(|s| s.moisture).sum::<f64>() / n,
            all.iter().map(|s| s.aeration_stress).sum::<f64>() / n,
            banks
        );
    }
    founder_identity_report(&world, &flora, &founder_rows);
    println!(
        "eligible skyline columns per species, introduction -> observation (a reading of a \
         moment each, never a settled habitat):"
    );
    let eligible_at_end = eligible_sets(&world, &flora, &skyline, &mut sky);
    for species in Species::ALL {
        let i = species.index();
        println!(
            "  {:>14}: {} -> {} of {}",
            species.name(),
            eligible[i].len(),
            eligible_at_end[i].len(),
            skyline.len()
        );
    }
    // The saprotroph's own flux, and what is left of the logs. `substrate_uptake` is a
    // diagnostic and not a boundary flow: the organic matter moved from a ground stock to a
    // stand, both inside this layer, so it is in none of the three `expected_*` totals.
    let dead_wood: f64 = v.ground.iter().map(|g| g.dead_wood).sum();
    let on_logs: f64 = logs
        .iter()
        .filter_map(|s| v.ground_at(*s))
        .map(|g| g.dead_wood)
        .sum();
    println!(
        "dead wood: {:.4} in the world, {:.4} of it on the {} declared logs (laid: {:.4}); \
         substrate uptake by the fungus {:.5} organic matter, cumulative",
        dead_wood,
        on_logs,
        logs.len(),
        logs.len() as f64 * LOG_ORGANIC,
        v.ledger.substrate_uptake[Species::Glowcap.index()]
    );
    let l = v.ledger;
    println!(
        "ledger: fixed_in {:.6} respired_out {:.6} light_in {:.6} heat_out {:.6} transpired \
         {:.6} m3 births {} establishments {} deaths {}",
        l.fixed_in,
        l.respired_out,
        l.light_in,
        l.heat_out,
        l.transpired_m3,
        l.births,
        l.establishments,
        l.deaths
    );
    println!(
        "residuals: organic {:.3e} mineral {:.3e} energy {:.3e} (stocks: organic {:.4} mineral \
         {:.4} energy {:.4})",
        v.organic() - l.expected_organic(),
        v.mineral() - l.expected_mineral(),
        v.energy() - l.expected_energy(),
        v.organic(),
        v.mineral(),
        v.energy()
    );
    germination_diagnosis(&world, &flora);
    gate_diagnosis(&world, &flora, &skyline, "at observation", &mut sky);
    let end_mark = water_mark(&world, ticks as f64 * cubarium_voxel::DT);
    println!("whole run: {}", water_budget_line(&start_mark, &end_mark));
    let water = world.view().stored_m3() - world.view().ledger.expected_stored();
    println!(
        "core water: stored {:.4} m3, residual {:.3e}, transpiration_out {:.6} m3 (flora says \
         {:.6})",
        world.view().stored_m3(),
        water,
        world.view().ledger.transpiration_out,
        l.transpired_m3
    );
    println!(
        "wall time: {:.1} s for {ticks} coupled ticks",
        started.elapsed().as_secs_f64()
    );
}

// ------------------------------------------------------------------ 5a: harvest

/// The reach box the harvest probe uses, straight out of the round-5a brief: two voxels
/// sideways, one up — a ground browser's box.
///
/// Not a model default (the model has no consumer) and not tuned: it is a **declared
/// experiment condition**, printed with every run. One consequence is worth knowing before
/// reading any result, and it is a statement about **relative elevation** and not about the
/// species (Astra R8.4): `SpeciesConfig::crown_voxels` is 2 for a bloomcrown at the
/// harness's founder wood, so its crown cells sit two voxels above its own support face and
/// an `up: 1` eater standing on **that same face** cannot reach them — while one standing
/// on a face a voxel higher can, because the rule compares the crown's *absolute* cell
/// height against the eater's own ceiling
/// (`tests/round5a.rs::the_same_crown_is_out_of_reach_from_its_own_face_and_in_reach_from_the_one_above`).
/// The declared faces of this probe are the patch species' own founder faces, so what a
/// harvester eats here is whatever else is within two voxels of it, and the per-species
/// lines below say exactly what that was.
const HARVEST_REACH: Reach = Reach {
    horizontal: 2,
    up: 1,
};

/// How many support faces the scripted harvester works from. Predeclared before the run,
/// printed with what each of them can reach at the start.
const HARVEST_SITES: usize = 3;

/// The default harvest rate: organic matter per second **per support face**, taken from the
/// stands that face can reach, spent on them in **site order** — so the harvester strips the
/// first stand it can reach before it touches the next. An experiment condition like
/// [`HARNESS_RAIN_M_PER_S`], and an argument so a rerun can state a different one.
///
/// 0.002 /s against a springturf founder's own foliage regrowth — `foliage_rate · W` is
/// 6e-4 /s at founder wood 0.03 — is a harvester that takes faster than one stand in reach
/// can put foliage back. That is deliberate: the probe asks what a **stripped** producer
/// does, not where a grazing equilibrium sits, and nothing here claims that a consumer of
/// any size would eat at this rate.
const HARVEST_RATE_PER_S: f64 = 0.002;

/// "Full foliage" for the recovery count: within 1 % of `α · W`. A stand at equilibrium sits
/// just under its own cap, because senescence takes `senescence · P · dt` every tick and the
/// cap is only what growth aims at, so an exact `P = α · W` would count nobody.
const FULL_FOLIAGE: f64 = 0.99;

/// One species' living stocks at one moment, summed over its stands.
#[derive(Clone, Copy, Default)]
struct Stocks {
    stands: usize,
    foliage: f64,
    reserve: f64,
    wood: f64,
}

fn stocks_of(flora: &Flora) -> [Stocks; Species::COUNT] {
    let mut out = [Stocks::default(); Species::COUNT];
    for s in flora.view().stands {
        let e = &mut out[s.species.index()];
        e.stands += 1;
        e.foliage += s.foliage;
        e.reserve += s.reserve;
        e.wood += s.wood;
    }
    out
}

/// The cohort's mean **canopy fill** `P / (α · W)` over its living stands, and how many
/// those were.
///
/// This is what "recovered" has to be read against, and the reason the strict
/// [`FULL_FOLIAGE`] bar cannot be read on its own: an **ungrazed** stand at equilibrium does
/// not sit at 1.0 either, because senescence takes `senescence · P · dt` every tick and the
/// cap is only what growth aims at. So the measure of a producer response is the grazed
/// cohort's fill against the **control's own** fill, and the bar is a stricter question
/// asked beside it.
fn cohort_fill(flora: &Flora, ids: &[u64]) -> (f64, usize) {
    let (mut sum, mut n) = (0.0, 0);
    for s in flora.view().stands.iter().filter(|s| ids.contains(&s.id)) {
        let cap = flora.config().species(s.species).alpha * s.wood;
        if cap > 0.0 {
            sum += s.foliage / cap;
            n += 1;
        }
    }
    (if n > 0 { sum / n as f64 } else { 0.0 }, n)
}

/// The foliage, the reserve and the living count of one **cohort by identity**.
fn cohort_sums(flora: &Flora, ids: &[u64]) -> (f64, f64, usize) {
    let mut out = (0.0, 0.0, 0);
    for s in flora.view().stands.iter().filter(|s| ids.contains(&s.id)) {
        out.0 += s.foliage;
        out.1 += s.reserve;
        out.2 += 1;
    }
    out
}

/// Everything one arm of the harvest probe leaves behind.
struct Arm {
    label: &'static str,
    /// Per-species stocks every 100 s, plus the start and the end.
    samples: Vec<(f64, [Stocks; Species::COUNT])>,
    /// The harvester's own faces, and what each reached at the start: stands and foliage.
    /// Declared in **both** arms, because a face is a property of the patch and not of the
    /// treatment — the control has to be able to report the same cohort.
    sites: Vec<(Site, usize, f64)>,
    /// The stands reachable from those faces **at the start**, by identity: the patch this
    /// arm is about, fixed before anything was eaten so that both arms mean one set.
    cohort: Vec<u64>,
    /// That cohort's foliage, reserve and living count at the start, at the halfway point
    /// where the harvest stops, and at the end.
    cohort_at: [(f64, f64, usize); 3],
    /// The same three moments, as mean canopy fill `P / (α · W)` and the count it averages.
    fill_at: [(f64, usize); 3],
    /// Every identity this arm actually bit, and those of them seen back at full foliage
    /// after the harvest stopped.
    bitten: Vec<u64>,
    recovered: Vec<u64>,
    /// Bites that took something, and the organic matter each species lost to them: the
    /// ledger's `consumed_*` have no species in them, and a reach box does not care which
    /// species it is holding.
    bites: u64,
    taken_by_species: [f64; Species::COUNT],
    bitten_by_species: [usize; Species::COUNT],
    consumed: (f64, f64, f64),
    establishments: u64,
    deaths: u64,
    /// The three residuals at the end, **and the stocks they are residuals of**: an absolute
    /// residual says nothing on its own, and this mode used to print only the absolute.
    residuals: (f64, f64, f64),
    stocks: (f64, f64, f64),
}

/// One arm: the same conditioned world, the same founders, the same predeclared faces, and a
/// scripted harvester only if `harvesting`.
///
/// Both arms are built from `prepared_world(seed, noise_seed)` and `pick_founders`, which are
/// deterministic in the seed, so the two are the **same world with the same founders** and
/// diverge at the first bite and nowhere else. The founder lists are compared afterwards
/// rather than trusted.
fn harvest_arm(
    seconds: f64,
    rate: f64,
    seed: u64,
    noise_seed: u64,
    patch: Species,
    harvesting: bool,
    verbose: bool,
) -> (Arm, Vec<Founder>) {
    let mut world = prepared_world(seed, noise_seed);
    let skyline = skyline_of(&world);
    let mut flora = Flora::new(FloraConfig::default());
    let founders = pick_founders(&world, &flora, &skyline, &Species::ALL, verbose);
    for f in &founders {
        let wood = founder_wood(flora.config().species(f.species));
        flora.apply(
            &world,
            Command::Seed {
                x: f.x as i64,
                z: f.z,
                species: f.species,
                wood,
            },
        );
    }

    // The harvester's faces, predeclared: the first `HARVEST_SITES` founder faces of the
    // patch species in site order. A founder's own face is a support face by construction,
    // and the eater stands on it.
    let mut faces: Vec<Site> = founders
        .iter()
        .filter(|f| f.species == patch)
        .filter_map(|f| cubarium_voxel_flora::highest_support(&world.view(), f.x as i64, f.z))
        .collect();
    faces.sort_unstable();
    faces.dedup();
    faces.truncate(HARVEST_SITES);

    let mut sites: Vec<(Site, usize, f64)> = Vec::new();
    let mut cohort: Vec<u64> = Vec::new();
    for &face in &faces {
        let reach = flora
            .view()
            .reachable_foliage(&world.view(), face, HARVEST_REACH);
        // `+ 0.0` only so that an empty reach prints as a positive zero.
        let foliage: f64 = 0.0 + reach.iter().map(|&(_, f)| f).sum::<f64>();
        sites.push((face, reach.len(), foliage));
        for (site, _) in reach {
            if let Some(stand) = flora.view().stand_at(site) {
                if !cohort.contains(&stand.id) {
                    cohort.push(stand.id);
                }
            }
        }
    }
    cohort.sort_unstable();

    let ticks = (seconds * cubarium_voxel::TICK_HZ as f64).round() as u64;
    let half = ticks / 2;
    let every = 100 * cubarium_voxel::TICK_HZ as u64;
    let mut arm = Arm {
        label: if harvesting {
            "harvested"
        } else {
            "plant-only"
        },
        samples: vec![(0.0, stocks_of(&flora))],
        sites,
        cohort: cohort.clone(),
        cohort_at: [cohort_sums(&flora, &cohort), (0.0, 0.0, 0), (0.0, 0.0, 0)],
        fill_at: [cohort_fill(&flora, &cohort), (0.0, 0), (0.0, 0)],
        bitten: Vec::new(),
        recovered: Vec::new(),
        bites: 0,
        taken_by_species: [0.0; Species::COUNT],
        bitten_by_species: [0; Species::COUNT],
        consumed: (0.0, 0.0, 0.0),
        establishments: 0,
        deaths: 0,
        residuals: (0.0, 0.0, 0.0),
        stocks: (0.0, 0.0, 0.0),
    };

    for tick in 0..ticks {
        world.step();
        flora.step(&mut world);
        // The bite is **between** ticks, which is where a transfer belongs: this tick has
        // run, and the next one reads the smaller `P`.
        if harvesting && tick < half {
            let budget = rate * cubarium_voxel::DT;
            for &face in &faces {
                let mut left = budget;
                let reachable = flora
                    .view()
                    .reachable_foliage(&world.view(), face, HARVEST_REACH);
                for (site, _) in reachable {
                    if left <= 0.0 {
                        break;
                    }
                    let Some((id, species)) =
                        flora.view().stand_at(site).map(|s| (s.id, s.species))
                    else {
                        continue;
                    };
                    if let Some(taken) = flora.take_foliage(site, left) {
                        left -= taken.organic;
                        arm.bites += 1;
                        arm.taken_by_species[species.index()] += taken.organic;
                        if !arm.bitten.contains(&id) {
                            arm.bitten.push(id);
                            arm.bitten_by_species[species.index()] += 1;
                        }
                    }
                }
            }
        }
        // Recovery is asked only after the harvest has stopped, and by identity: a stand
        // that was bitten and is back within 1 % of its own `α · W`.
        if tick >= half {
            for stand in flora.view().stands {
                if arm.bitten.contains(&stand.id) && !arm.recovered.contains(&stand.id) {
                    let cap = flora.config().species(stand.species).alpha * stand.wood;
                    if cap > 0.0 && stand.foliage >= FULL_FOLIAGE * cap {
                        arm.recovered.push(stand.id);
                    }
                }
            }
        }
        if tick + 1 == half {
            arm.cohort_at[1] = cohort_sums(&flora, &cohort);
            arm.fill_at[1] = cohort_fill(&flora, &cohort);
        }
        if (tick + 1) % every == 0 {
            arm.samples
                .push(((tick + 1) as f64 * cubarium_voxel::DT, stocks_of(&flora)));
        }
    }

    arm.cohort_at[2] = cohort_sums(&flora, &cohort);
    arm.fill_at[2] = cohort_fill(&flora, &cohort);
    let v = flora.view();
    arm.consumed = (
        v.ledger.consumed_organic_out,
        v.ledger.consumed_mineral_out,
        v.ledger.consumed_energy_out,
    );
    arm.establishments = v.ledger.establishments;
    arm.deaths = v.ledger.deaths;
    arm.residuals = (
        v.organic() - v.ledger.expected_organic(),
        v.mineral() - v.ledger.expected_mineral(),
        v.energy() - v.ledger.expected_energy(),
    );
    arm.stocks = (v.organic(), v.mineral(), v.energy());
    if arm.samples.last().map(|&(s, _)| s) != Some(ticks as f64 * cubarium_voxel::DT) {
        arm.samples
            .push((ticks as f64 * cubarium_voxel::DT, stocks_of(&flora)));
    }
    arm.bitten.sort_unstable();
    arm.recovered.sort_unstable();
    (arm, founders)
}

/// The probe: two patches, and for each of them a plant-only arm and a harvested arm of the
/// same conditioned world.
fn harvest(args: &[String]) {
    let seconds: f64 = arg(args, 0).unwrap_or(400.0);
    let rate: f64 = arg(args, 1).unwrap_or(HARVEST_RATE_PER_S);
    let seed: u64 = arg(args, 2).unwrap_or(1);
    let noise_seed: u64 = arg(args, 3).unwrap_or(0);
    let started = std::time::Instant::now();
    println!(
        "harvest: seed {seed} noise_seed {noise_seed}, rain {HARNESS_RAIN_M_PER_S} m/s with the \
         outlet open, {WARMUP_TICKS} warm-up ticks (50 s), then {seconds:.0} coupled seconds per \
         arm; the harvester takes up to {rate} organic/s per support face from stands inside \
         {HARVEST_REACH:?}, spends it in site order, and **stops at {:.0} s**",
        seconds / 2.0
    );
    println!(
        "a producer-response probe and nothing else: no animal body, no movement, no population, \
         no carrying capacity and no viability claim. What it measures is what the existing \
         foliage_rate and reflush-from-reserve rules do to a stripped stand."
    );

    let mut first = true;
    for patch in [Species::Springturf, Species::Bloomcrown] {
        let (control, founders) = harvest_arm(seconds, rate, seed, noise_seed, patch, false, first);
        first = false;
        let (grazed, again) = harvest_arm(seconds, rate, seed, noise_seed, patch, true, false);
        assert_eq!(again, founders, "the two arms must plant the same founders");
        assert_eq!(
            control.consumed,
            (0.0, 0.0, 0.0),
            "the plant-only arm ate something"
        );
        assert_eq!(
            control.cohort, grazed.cohort,
            "the two arms must mean the same cohort"
        );
        report_harvest(patch, &control, &grazed, rate);
    }
    println!(
        "\nwall time: {:.1} s for four arms of {seconds:.0} coupled seconds",
        started.elapsed().as_secs_f64()
    );
}

/// One patch's report: the declaration, the two arms' trajectories, what was taken from
/// whom, and what came back.
fn report_harvest(patch: Species, control: &Arm, grazed: &Arm, rate: f64) {
    println!("\n=== {} patch ===", patch.name());
    if grazed.sites.is_empty() {
        println!(
            "  no founder of {} was planted, so there is no predeclared face and no treatment to \
             report",
            patch.name()
        );
        return;
    }
    println!(
        "  predeclared support faces (the first {HARVEST_SITES} founder faces of {} in site \
         order), and what each reached at the start under {HARVEST_REACH:?}:",
        patch.name()
    );
    for &(site, stands, foliage) in &grazed.sites {
        println!(
            "    ({:>3},{:>3}) y{:<3} {stands} stands in reach holding {foliage:.5} of foliage",
            site.x, site.z, site.y
        );
    }
    let in_reach: usize = grazed.sites.iter().map(|&(_, n, _)| n).sum();
    if in_reach == 0 {
        println!(
            "  **nothing was in reach at the start**: with {HARVEST_REACH:?} a crown is food only \
             if its lowest cell is at most one voxel over the eater's own face. Read the totals \
             below as the empty treatment they are."
        );
    }

    println!("  trajectories (stands / foliage / reserve / wood, summed per species):");
    for arm in [control, grazed] {
        println!("    -- {} --", arm.label);
        for &(seconds, stocks) in &arm.samples {
            let mut line = format!("    t {seconds:>5.0} s:");
            for species in Species::ALL {
                let s = stocks[species.index()];
                if s.stands == 0 {
                    continue;
                }
                line.push_str(&format!(
                    " {} {}/{:.4}/{:.4}/{:.4};",
                    species.name(),
                    s.stands,
                    s.foliage,
                    s.reserve,
                    s.wood
                ));
            }
            println!("{line}");
        }
    }

    println!(
        "  taken at {rate} organic/s per face: {} bites on {} distinct stands; consumed organic \
         {:.6}, mineral {:.7}, energy {:.6}",
        grazed.bites,
        grazed.bitten.len(),
        grazed.consumed.0,
        grazed.consumed.1,
        grazed.consumed.2
    );
    println!("  by species, because a reach box does not choose one:");
    for species in Species::ALL {
        let i = species.index();
        if grazed.bitten_by_species[i] == 0 {
            continue;
        }
        println!(
            "    {:>14}: {:.6} organic off {} stands",
            species.name(),
            grazed.taken_by_species[i],
            grazed.bitten_by_species[i]
        );
    }
    let bitten = grazed.bitten.len();
    let recovered = grazed.recovered.len();
    println!(
        "  recovery, by identity, after the harvest stopped: {recovered} of {bitten} bitten \
         stands reached {:.0} % of their own α·W before the end — a recovery fraction of {:.2}; \
         the other {} are still under it or no longer standing",
        100.0 * FULL_FOLIAGE,
        if bitten == 0 {
            0.0
        } else {
            recovered as f64 / bitten as f64
        },
        bitten - recovered
    );
    println!(
        "  that bar is strict, and the plant-only arm is what says how strict: its own cohort's \
         mean canopy fill P/(α·W) ends at {:.3}, so a count against a {:.2} bar is not by itself \
         evidence that nothing regrew. The fill comparison below is the measure.",
        control.fill_at[2].0, FULL_FOLIAGE
    );
    println!(
        "  the declared cohort ({} stands reachable at the start), foliage / reserve / alive at \
         0 s, at the harvest stop and at the end:",
        grazed.cohort.len()
    );
    for arm in [control, grazed] {
        let [a, b, c] = arm.cohort_at;
        let [fa, fb, fc] = arm.fill_at;
        println!(
            "    {:>10}: foliage {:.5} -> {:.5} -> {:.5}; reserve {:.5} -> {:.5} -> {:.5}; alive \
             {} -> {} -> {}; mean fill P/(α·W) {:.3} -> {:.3} -> {:.3}",
            arm.label, a.0, b.0, c.0, a.1, b.1, c.1, a.2, b.2, c.2, fa.0, fb.0, fc.0
        );
    }
    for arm in [control, grazed] {
        println!(
            "    {:>10}: establishments {}, deaths {}, residuals organic {:.3e} mineral {:.3e} \
             energy {:.3e} against stocks {:.4} / {:.4} / {:.4} (relative {:.2e} / {:.2e} / \
             {:.2e})",
            arm.label,
            arm.establishments,
            arm.deaths,
            arm.residuals.0,
            arm.residuals.1,
            arm.residuals.2,
            arm.stocks.0,
            arm.stocks.1,
            arm.stocks.2,
            arm.residuals.0 / arm.stocks.0.abs().max(1.0),
            arm.residuals.1 / arm.stocks.1.abs().max(1.0),
            arm.residuals.2 / arm.stocks.2.abs().max(1.0)
        );
    }
}

#[cfg(test)]
mod tests {
    //! The placement geometry Astra's R7.1 asked for, as two short cases. They are unit
    //! tests of this harness's own helpers — the model is not stepped here — and the crate
    //! is configured to run them (`[[example]] test = true`).

    use super::*;
    use cubarium_voxel::Material;

    /// Two ledges and a wrap: column 0's support face is at `y = 2`, column 1's at `y = 4`,
    /// and column 7's at `y = 2`, which is one step from column 0 across the world's `x`
    /// wrap. Nothing is ever stepped, so no water moves and the faces stay where they are.
    fn two_ledges() -> World {
        let mut w = World::empty(VoxelConfig {
            width: 8,
            height: 12,
            depth: 1,
            voxel_m: 1.0,
            ..VoxelConfig::default()
        });
        for (x, top) in [(0i64, 2u32), (1, 4), (7, 2)] {
            for y in 1..=top {
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z: 0,
                    material: Material::Soil,
                });
            }
        }
        for (x, top) in [(0i64, 2u32), (1, 4), (7, 2)] {
            assert_eq!(
                cubarium_voxel_flora::highest_support(&w.view(), x, 0),
                Some(Site {
                    x: x as u32,
                    y: top,
                    z: 0
                }),
                "column {x} must have its face at {top}"
            );
        }
        w
    }

    /// **The two-height case.** A half-grown bloomcrown planned on the low ledge tops out
    /// at an absolute 4.0 with radius 1, so it covers both neighbouring columns. The
    /// velvetpad candidate on the **high** ledge would top out at 4.75 and the model applies
    /// no shade to it, so the rule must refuse that site; the one on the level ledge tops
    /// out at 2.75 and is genuinely under the crown.
    ///
    /// The old level-face test admitted both, because it compared 2.0 against 0.75 and never
    /// looked at either support height.
    #[test]
    fn a_crown_lower_than_the_site_it_covers_does_not_shade_it() {
        let world = two_ledges();
        let flora = Flora::new(FloraConfig::default());
        let planned = [Founder {
            species: Species::Bloomcrown,
            x: 0,
            z: 0,
        }];
        let canopy = canopy_over(&world, &flora, &planned);
        assert_eq!(canopy.len(), 1, "one planned crown: {canopy:?}");
        assert!(
            (canopy[0].top - 4.0).abs() < 1e-12,
            "absolute top {}",
            canopy[0].top
        );
        assert!(
            (canopy[0].radius - 1.0).abs() < 1e-12,
            "radius {}",
            canopy[0].radius
        );
        assert!(
            canopy[0].foliage > 0.0,
            "a planned founder arrives with alpha * wood"
        );

        let bloom = flora.config().species(Species::Bloomcrown);
        let velvet = flora.config().species(Species::Velvetpad);
        let high = Site { x: 1, y: 4, z: 0 };
        let level = Site { x: 7, y: 2, z: 0 };
        // The premise of the old test, so the case cannot rot: over their own faces the
        // bloomcrown crown is the taller of the two.
        assert!(
            bloom.crown_height(founder_wood(bloom)) > velvet.crown_height(founder_wood(velvet)),
            "the level-face comparison the old helper made"
        );
        // Both columns are inside the crown's footprint, so it is the height that decides.
        assert!(
            covered_by(&canopy, 8, high, 0.0),
            "the high ledge is under the footprint"
        );
        assert!(
            covered_by(&canopy, 8, level, 0.0),
            "the level ledge is under the footprint"
        );

        assert!(
            !under_a_crown(&canopy, 8, &flora, Species::Velvetpad, high),
            "a crown topping out at 4.0 cannot shade a velvetpad whose own top is 4.75"
        );
        assert!(
            under_a_crown(&canopy, 8, &flora, Species::Velvetpad, level),
            "on the level ledge the same crown does top it, at 4.0 against 2.75"
        );
    }

    /// **The undersized-resident case.** A standing bloomcrown at `alive_min` is a much
    /// smaller crown than a half-grown founder: radius 0.533 against 1.0, absolute top
    /// 3.067 against 4.0. So the neighbouring column across the wrap is **outside** its
    /// footprint, where treating the resident as a founder would have called that column
    /// shaded and — for `UnderACrown` — planted a velvetpad there in full sky.
    #[test]
    fn an_existing_stand_is_measured_at_its_own_wood_and_not_as_a_founder() {
        let world = two_ledges();
        let sc = FloraConfig::default().bloomcrown.clone();
        let mut flora = Flora::new(FloraConfig::default());
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 0,
                z: 0,
                species: Species::Bloomcrown,
                wood: sc.alive_min
            }
        ));

        let canopy = canopy_over(&world, &flora, &[]);
        assert_eq!(canopy.len(), 1, "one standing crown: {canopy:?}");
        assert!(
            (canopy[0].radius - sc.crown_radius(sc.alive_min)).abs() < 1e-12,
            "radius {} is not the stand's own",
            canopy[0].radius
        );
        assert!(
            canopy[0].radius < sc.crown_radius(founder_wood(&sc)),
            "a newborn is not a half-grown founder: {} against {}",
            canopy[0].radius,
            sc.crown_radius(founder_wood(&sc))
        );
        assert!(
            (canopy[0].top - (2.0 + sc.crown_height(sc.alive_min))).abs() < 1e-12,
            "absolute top {}",
            canopy[0].top
        );

        let level = Site { x: 7, y: 2, z: 0 };
        assert!(
            !under_a_crown(&canopy, 8, &flora, Species::Velvetpad, level),
            "a newborn's 0.533 radius does not reach the next column"
        );
        // The same column, read as the founder-sized crown the old helper assumed: it is
        // covered, and its top does clear a velvetpad's 2.75, so the two readings disagree
        // and the resident's actual wood is what decides.
        let as_founder = [Canopy {
            x: 0.0,
            z: 0.0,
            top: 2.0 + sc.crown_height(founder_wood(&sc)),
            radius: sc.crown_radius(founder_wood(&sc)),
            foliage: sc.alpha * founder_wood(&sc),
        }];
        assert!(
            under_a_crown(&as_founder, 8, &flora, Species::Velvetpad, level),
            "the founder-sized reading is the one that admitted this site"
        );
    }
}
