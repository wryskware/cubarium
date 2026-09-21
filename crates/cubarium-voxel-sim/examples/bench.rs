//! **The tick profile: where a coupled voxel tick's time goes.** Measurement only — no
//! rule, no number and no observation of the ecology is in here.
//!
//! ```text
//! cargo run --release -p cubarium-voxel-sim --features profile --example bench \
//!   -- [ticks] [grazers] [warmup_s] [seed] [sample_every] [threads]
//! ```
//!
//! Since the schedule landed the tick is `cubarium_voxel_sim::Sim::step`, and `threads` is
//! `SimConfig::threads`: `1` runs every phase serially and is the baseline this bench is
//! read against. The compute pool is process-global, so **one process per thread count** —
//! a run that wants the whole table runs the binary four times.
//!
//! The condition, declared: the **generated default world** (128 × 48 × 24 voxels of
//! 0.25 m, `water_substeps` 4, `seed` 1, `noise_seed` 0) with the harness rain of 2e-4 m/s
//! and the outlet open, conditioned for `warmup_s` seconds of world-only ticks (50 s by
//! default), then planted with eight founders of each of the six species at half their own
//! `wood_max` on their own gate-passing skyline faces — with eight declared logs laid
//! first so glowcap has a substrate — and stepped `ticks` coupled ticks (2,000 by default)
//! with the timers running. `grazers` animals are introduced on gate-passing open-soil
//! faces before the measured window, so the animal layer is measured with animals in it.
//!
//! **How this differs from `community`**, which it is modelled on: founders are placed by
//! `grazed.rs`'s strided gate-passing rule rather than `two_producers.rs`'s `Habitat`
//! table, because that table lives in the flora crate's harness. Same six species, same
//! eight founders each, same founder fraction, same logs — the same *scale* of world, and
//! not the same sites. Nothing here is an ecological observation, so the placement rule
//! only has to load the tick honestly.
//!
//! **What is measured.** Three things, separately:
//!
//! 1. The tick's own phases, from the `profile` feature's timers inside the three crates.
//! 2. The **active sets** an entity-and-system tick would iterate instead of the grid —
//!    cells holding free water, cells whose pore water is over their material's own field
//!    capacity, sites with a `Ground`, stands, animals — sampled every `SAMPLE_EVERY`
//!    ticks so the sampling is not in the numbers it explains.
//! 3. What the **harness observers** cost, which is not simulation at all: the
//!    eligible-set scan `two_producers`/`replacement` run at observation (every skyline
//!    column against every species' germination predicate) and the identity tracking
//!    `grazed` runs every tick (a `reachable_foliage` query per animal plus a foliage
//!    snapshot of every stand in reach). Both are timed here and reported per tick so that
//!    a "the sim is slow" reading cannot be a harness reading.

use std::time::Instant;

use cubarium_voxel::profile::{self, Count, Phase};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Command as FaunaCommand, DT, Fauna, FaunaConfig, Species as Beast, TICK_HZ,
};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, SkyCache, Species,
};
use cubarium_voxel_sim::{Sim, SimConfig};

/// `grazed.rs`'s and `two_producers.rs`'s conditioning rain: under the outlet's capacity.
const HARNESS_RAIN_M_PER_S: f64 = 0.0002;
/// `two_producers.rs`'s founder size and count, and `community`'s logs.
const FOUNDER_FRACTION: f64 = 0.5;
const FOUNDERS_PER_SPECIES: usize = 8;
const DECLARED_LOGS: usize = 8;
const LOG_ORGANIC: f64 = 1.0;
/// How often the active sets are counted, unless the command line says otherwise. A
/// full-grid scan and an eligible-set scan each cost about as much as a tick, so they are
/// sampled and not run every tick — and `0` turns them off entirely, which is what a
/// `perf record` of the tick alone wants.
const SAMPLE_EVERY: u64 = 100;
/// The water leaves, in tick order: what `World::step` used to report as `WorldStep`.
const WATER_PHASES: [Phase; 9] = [
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: Vec<String> = match args.first().map(String::as_str) {
        Some("bench") => args[1..].to_vec(),
        _ => args.clone(),
    };
    let ticks: u64 = arg(&rest, 0).unwrap_or(2000);
    let grazers: usize = arg(&rest, 1).unwrap_or(0);
    let warmup_s: f64 = arg(&rest, 2).unwrap_or(50.0);
    let seed: u64 = arg(&rest, 3).unwrap_or(1);
    let sample_every: u64 = arg(&rest, 4).unwrap_or(SAMPLE_EVERY);
    let threads: usize = arg(&rest, 5).unwrap_or(1);

    let warmup_ticks = (warmup_s * f64::from(TICK_HZ)).round() as u64;
    println!(
        "# voxel tick profile: {ticks} coupled ticks, {grazers} grazers, {warmup_s} s warm-up, \
         seed {seed}, {threads} thread(s)"
    );

    let world = prepared_world(seed, warmup_ticks, threads);
    let c = world.config().clone();
    println!(
        "world {}x{}x{} voxels of {} m ({} cells, {} columns), water_substeps {}, rain \
         {HARNESS_RAIN_M_PER_S} m/s, outlet open",
        c.width,
        c.height,
        c.depth,
        c.voxel_m,
        c.cells(),
        c.width * c.depth,
        c.water_substeps
    );

    let mut flora = Flora::new(FloraConfig::default());
    let logs = lay_logs(&mut flora, &world);
    let founders = plant_founders(&mut flora, &world);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let placed = introduce(&mut fauna, &flora, &world, grazers);
    println!(
        "planted {founders} founders of {} species, {logs} logs, {placed} grazers",
        Species::COUNT
    );

    let mut sim = Sim::new(world, flora, fauna, SimConfig { threads }, None);

    // Nothing before this line is in the numbers.
    profile::reset();
    let mut sets: Vec<Sets> = Vec::new();
    let mut observers = Observers::default();
    // The samplers are timed out of the tick: a full-grid set count and an eligible-set
    // scan each cost about as much as a tick, and they are not the tick. They stay out of
    // `TickPhase::Sample` for exactly that reason — a system in the schedule would be
    // inside `Sim::step` and therefore inside the wall time being reported.
    let mut wall = 0.0f64;
    for tick in 0..ticks {
        let at = Instant::now();
        sim.step();
        wall += at.elapsed().as_secs_f64();
        if sample_every > 0 && tick % sample_every == 0 {
            let (world, flora, fauna) = sim.layers();
            sets.push(measure_sets(world, flora, fauna));
            observers.measure(world, flora, fauna);
        }
    }
    report(ticks, wall, &sets, &observers, &c, sample_every, threads);
}

fn arg<T: std::str::FromStr>(args: &[String], i: usize) -> Option<T> {
    args.get(i).and_then(|s| s.parse().ok())
}

/// `grazed.rs`'s `prepared_world`, with the conditioning ticks in it: generate once to find
/// the basin floor, generate the world the run uses with the table a metre above it, open
/// the outlet, and step it world-only for the warm-up.
fn prepared_world(seed: u64, warmup_ticks: u64, threads: usize) -> World {
    let dry = VoxelConfig {
        seed,
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
    let mut world = World::new(config);
    world.apply(WorldCommand::SetOutlet { open: true });
    for _ in 0..warmup_ticks {
        // Warm-up can initialise the process-global pool before Sim::new does.
        // Use the requested count here too, rather than fixing it to every CPU.
        world.step_with(threads);
    }
    world
}

/// Every column's highest support face, sorted low to high: `grazed.rs`'s skyline.
fn skyline_of(world: &World) -> Vec<Site> {
    let (width, depth) = (world.config().width, world.config().depth);
    let mut skyline: Vec<Site> = Vec::new();
    for z in 0..depth {
        for x in 0..width as i64 {
            if let Some(site) = cubarium_voxel_flora::highest_support(&world.view(), x, z) {
                skyline.push(site);
            }
        }
    }
    skyline.sort_by_key(|s| (s.y, s.x, s.z));
    skyline
}

fn strided<T: Copy>(pool: &[T], want: usize) -> Vec<T> {
    if pool.is_empty() || want == 0 {
        return Vec::new();
    }
    let stride = (pool.len() / want).max(1);
    pool.iter().step_by(stride).take(want).copied().collect()
}

/// `community`'s declared logs, on gate-passing open-soil faces, so the saprotroph has a
/// substrate to stand on at all.
fn lay_logs(flora: &mut Flora, world: &World) -> usize {
    let sc = flora.config().species(Species::Glowcap).clone();
    let (mineral, energy) = (sc.n_tissue * LOG_ORGANIC, sc.energy_density * LOG_ORGANIC);
    let pool: Vec<Site> = skyline_of(world)
        .into_iter()
        .filter(|&s| {
            world.view().material_at(i64::from(s.x), s.y, s.z) == Material::Soil
                && flora
                    .view()
                    .establishment_gates(&world.view(), s, Species::Glowcap)
                    .pore_ok
        })
        .collect();
    let mut laid = 0;
    for site in strided(&pool, DECLARED_LOGS) {
        let deposit = Deposit {
            kind: DepositKind::DeadWood,
            organic: LOG_ORGANIC,
            mineral,
            energy,
        };
        if flora.deposit(site, deposit) {
            laid += 1;
        }
    }
    laid
}

/// Eight founders of each species at half its own `wood_max`, on its own gate-passing
/// skyline faces, strided, never a site another founder has.
fn plant_founders(flora: &mut Flora, world: &World) -> usize {
    let skyline = skyline_of(world);
    let mut taken: Vec<Site> = Vec::new();
    let mut planted = 0;
    for species in Species::ALL {
        let pool: Vec<Site> = skyline
            .iter()
            .copied()
            .filter(|&s| {
                !taken.contains(&s) && flora.view().can_establish(&world.view(), s, species)
            })
            .collect();
        let wood = FOUNDER_FRACTION * flora.config().species(species).wood_max;
        for site in strided(&pool, FOUNDERS_PER_SPECIES) {
            if flora.apply(
                world,
                FloraCommand::Seed {
                    x: i64::from(site.x),
                    z: site.z,
                    species,
                    wood,
                },
            ) {
                taken.push(site);
                planted += 1;
            }
        }
    }
    planted
}

/// `grazed.rs`'s grazer rule: gate-passing open soil, strided.
fn introduce(fauna: &mut Fauna, flora: &Flora, world: &World, n: usize) -> usize {
    let pool: Vec<Site> = skyline_of(world)
        .into_iter()
        .filter(|&s| {
            world.view().material_at(i64::from(s.x), s.y, s.z) == Material::Soil
                && flora
                    .view()
                    .can_establish(&world.view(), s, Species::Springturf)
        })
        .collect();
    let body = fauna.config().species(Beast::Frondgrazer).body_max;
    let mut placed = 0;
    for site in strided(&pool, n) {
        if fauna.apply(
            world,
            FaunaCommand::Introduce {
                x: i64::from(site.x),
                z: site.z,
                species: Beast::Frondgrazer,
                body,
            },
        ) {
            placed += 1;
        }
    }
    placed
}

/// The sets a tick could iterate instead of the grid, at one moment.
#[derive(Clone, Copy, Debug, Default)]
struct Sets {
    /// Cells that are not solid: where free water can be at all.
    void: u64,
    /// Cells holding free water.
    free_wet: u64,
    /// Cells holding any pore water.
    pore_any: u64,
    /// Cells whose pore water is over their material's own field capacity — the only ones
    /// `drain` can move anything out of.
    pore_over_field: u64,
    /// Cells with a void neighbour below and free water: the only ones `fall` can move.
    fall_movable: u64,
    ground: u64,
    stands: u64,
    animals: u64,
}

fn measure_sets(world: &World, flora: &Flora, fauna: &Fauna) -> Sets {
    let v = world.view();
    let c = v.config;
    let mut s = Sets::default();
    for z in 0..c.depth {
        for y in 0..c.height {
            for x in 0..c.width as i64 {
                let i = c.index(x, y, z);
                let m = v.material[i];
                if !m.is_solid() {
                    s.void += 1;
                }
                if v.free[i] > 0.0 {
                    s.free_wet += 1;
                    if y > 0 {
                        let below = c.index(x, y - 1, z);
                        if !v.material[below].is_solid() && v.free[below] < 1.0 {
                            s.fall_movable += 1;
                        }
                    }
                }
                if v.pore[i] > 0.0 {
                    s.pore_any += 1;
                    if v.pore[i] > m.field_capacity() {
                        s.pore_over_field += 1;
                    }
                }
            }
        }
    }
    s.ground = flora.view().ground.len() as u64;
    s.stands = flora.view().stands.len() as u64;
    s.animals = fauna.view().animals.len() as u64;
    s
}

/// What the two published harnesses spend per observation, which is not the tick.
#[derive(Default)]
struct Observers {
    /// `two_producers`/`replacement`: every skyline column against every species'
    /// germination predicate, through the model's shared sky cache.
    eligible_ns: u64,
    eligible_calls: u64,
    eligible_columns: u64,
    /// One terrain sky cache for the whole run: the observer re-reads the gates but not the
    /// geometry while the terrain is unchanged.
    sky: SkyCache,
    /// `grazed`: one `reachable_foliage` per animal plus the foliage of every stand in
    /// reach, which is what the receipts table needs.
    identity_ns: u64,
    identity_calls: u64,
}

impl Observers {
    fn measure(&mut self, world: &World, flora: &Flora, fauna: &Fauna) {
        let skyline = skyline_of(world);
        let view = world.view();
        let at = Instant::now();
        let mut passing = 0u64;
        for species in Species::ALL {
            for g in flora
                .view()
                .establishment_gates_over(&view, &skyline, species, &mut self.sky)
            {
                if g.passes() {
                    passing += 1;
                }
            }
        }
        self.eligible_ns += at.elapsed().as_nanos() as u64;
        self.eligible_calls += 1;
        self.eligible_columns = skyline.len() as u64;
        std::hint::black_box(passing);

        let reach = fauna.config().species(Beast::Frondgrazer).reach;
        let at = Instant::now();
        let mut held = 0.0f64;
        for a in fauna.view().animals {
            for (site, _) in flora.view().reachable_foliage(&world.view(), a.site, reach) {
                held += flora.view().stand_at(site).map_or(0.0, |s| s.foliage);
            }
        }
        self.identity_ns += at.elapsed().as_nanos() as u64;
        self.identity_calls += 1;
        std::hint::black_box(held);
    }
}

fn mean(of: impl Fn(&Sets) -> u64, sets: &[Sets]) -> f64 {
    if sets.is_empty() {
        return 0.0;
    }
    sets.iter().map(|s| of(s) as f64).sum::<f64>() / sets.len() as f64
}

fn report(
    ticks: u64,
    wall: f64,
    sets: &[Sets],
    observers: &Observers,
    c: &VoxelConfig,
    sample_every: u64,
    threads: usize,
) {
    let n = ticks as f64;
    // The schedule runs the water phases one at a time, so `World::step`'s own
    // `WorldStep`/`Substeps` frames never open and the water total is the **sum of its
    // leaves**. Those two rows are therefore absent from the table below rather than zero.
    let water: u64 = WATER_PHASES.iter().copied().map(profile::nanos).sum();
    let total = water + profile::nanos(Phase::FloraStep) + profile::nanos(Phase::FaunaStep);
    println!(
        "\nsim wall {:.2} s for {ticks} ticks = {:.3} ms/tick = {:.1} ticks/s = {:.1}x real \
         time (the samplers are outside this)",
        wall,
        1e3 * wall / n,
        n / wall,
        n / (wall * f64::from(TICK_HZ))
    );
    println!(
        "timed phases sum to {:.3} ms/tick of that ({:.1} %); the rest is the three step \
         functions' own frames and the timers themselves",
        1e-6 * total as f64 / n,
        100.0 * (total as f64 * 1e-9) / wall
    );

    println!(
        "\nwater {:.2} us/tick ({:.1} %), flora {:.2} us/tick ({:.1} %), fauna {:.2} us/tick \
         ({:.1} %) of the timed total",
        1e-3 * water as f64 / n,
        100.0 * water as f64 / total.max(1) as f64,
        1e-3 * profile::nanos(Phase::FloraStep) as f64 / n,
        100.0 * profile::nanos(Phase::FloraStep) as f64 / total.max(1) as f64,
        1e-3 * profile::nanos(Phase::FaunaStep) as f64 / n,
        100.0 * profile::nanos(Phase::FaunaStep) as f64 / total.max(1) as f64,
    );

    println!("\n## per-phase wall time per tick, {threads} thread(s)\n");
    println!("| phase | µs/tick | % of timed | calls/tick |");
    println!("| --- | --- | --- | --- |");
    for p in Phase::ALL {
        let ns = profile::nanos(p);
        if ns == 0 && profile::calls(p) == 0 {
            continue;
        }
        println!(
            "| {} | {:.2} | {:.1}{} | {:.2} |",
            p.name(),
            1e-3 * ns as f64 / n,
            100.0 * ns as f64 / total.max(1) as f64,
            if p.is_total() { " (total)" } else { "" },
            profile::calls(p) as f64 / n
        );
    }

    println!("\n## the work each phase was handed, per tick\n");
    println!("| count | per tick |");
    println!("| --- | --- |");
    for k in Count::ALL {
        let v = profile::count(k);
        if v == 0 {
            continue;
        }
        println!("| {} | {:.1} |", k.name(), v as f64 / n);
    }

    if sets.is_empty() {
        println!("\n(no active-set samples: the sampler was off)");
    }
    println!("\n## the active sets, mean over {} samples\n", sets.len());
    let cells = c.cells() as f64;
    println!(
        "| set | cells or items | share of the {} -cell grid |",
        c.cells()
    );
    println!("| --- | --- | --- |");
    for (name, value) in [
        ("cells that are not solid (void)", mean(|s| s.void, sets)),
        ("cells holding free water", mean(|s| s.free_wet, sets)),
        (
            "of those, with room below (fall can move)",
            mean(|s| s.fall_movable, sets),
        ),
        ("cells holding pore water", mean(|s| s.pore_any, sets)),
        (
            "of those, over field capacity (drain can move)",
            mean(|s| s.pore_over_field, sets),
        ),
        ("sites with a Ground", mean(|s| s.ground, sets)),
        ("stands", mean(|s| s.stands, sets)),
        ("animals", mean(|s| s.animals, sets)),
    ] {
        println!("| {name} | {value:.1} | {:.3} % |", 100.0 * value / cells);
    }

    println!("\n## what the harness observers cost (not the tick)\n");
    let per = |ns: u64, calls: u64| {
        if calls == 0 {
            0.0
        } else {
            1e-3 * ns as f64 / calls as f64
        }
    };
    println!(
        "| observer | µs per observation | as a share of one {:.0} µs tick |",
        1e3 * wall / n * 1e3
    );
    println!("| --- | --- | --- |");
    let tick_us = 1e6 * wall / n;
    for (name, ns, calls) in [
        (
            "eligible-set scan (6 species x skyline columns)",
            observers.eligible_ns,
            observers.eligible_calls,
        ),
        (
            "identity tracking (per animal reach + stand foliage)",
            observers.identity_ns,
            observers.identity_calls,
        ),
    ] {
        println!(
            "| {name} | {:.1} | {:.2} |",
            per(ns, calls),
            per(ns, calls) / tick_us
        );
    }
    println!(
        "\n(the eligible-set scan is {} skyline columns x {} species = {} predicates; \
         `two_producers` and `replacement` run it at each observation, not each tick.)",
        observers.eligible_columns,
        Species::COUNT,
        observers.eligible_columns * Species::COUNT as u64
    );
    println!(
        "\nsample: the active sets and the observer timings were measured every \
         {sample_every} ticks, outside the phase timers (0 means never)."
    );
    println!(
        "dt is {DT} s, so {:.0} ticks/s is real time.",
        f64::from(TICK_HZ)
    );
}
