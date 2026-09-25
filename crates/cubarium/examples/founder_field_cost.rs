//! **What the live litter cue field costs on the seeded habitat.**
//!
//! The live schedule can now hold a `Senses` (`Sim::new`'s optional argument), and the
//! founders that walk the seeded habitat read it. This measures what that costs, because
//! the field is the one thing the live tick did not do before.
//!
//! Two measurements, because one of them alone would be misleading:
//!
//! 1. **The field's own work, directly.** `Senses::settle` on an already-settled field
//!    returns after the eight consecutive sub-threshold updates its convergence rule
//!    asks for, so timing it divided by eight is the cost of one field update against
//!    this habitat's real sources. The tick fires one update every `UPDATE_TICKS`, so
//!    the per-tick charge is that divided by `UPDATE_TICKS`. Nothing about animal
//!    behaviour enters this number.
//! 2. **The whole tick, with and without the field.** The honest end-to-end figure, and
//!    it carries a caveat that is stated rather than hidden: the two arms *diverge*. A
//!    blind founder with no field reads `Chem` as invalid and wanders differently, so
//!    after a few hundred ticks the two worlds are not the same world. The difference is
//!    therefore the cost of running the habitat with sensing founders, not a controlled
//!    isolation of the field alone — which is what measurement 1 is for.
//!
//! The compute pool is process-global, so run **one process per thread count**:
//!
//! ```text
//! cargo run --release -p cubarium --example founder_field_cost -- 1
//! cargo run --release -p cubarium --example founder_field_cost -- 16
//! ```

use std::time::Instant;

use cubarium_voxel_fauna::{Fauna, FaunaConfig, Founder, Senses, UPDATE_TICKS};
use cubarium_voxel_flora::{Flora, FloraConfig};
use cubarium_voxel_sim::{Sim, SimConfig};

/// Ticks each timed arm runs: one simulated minute at 20 Hz, long enough for the
/// litterfall the field's sources grow from and short enough to be a check, not a study.
const TICKS: u64 = 1_200;
/// Ticks run before the clock starts, so neither arm pays for a cold cache.
const WARMUP: u64 = 200;

fn habitat() -> (cubarium_voxel::World, Flora, Fauna) {
    let cfg = cubarium::voxel::VoxelConfig::default();
    let mut world = cubarium::voxel::scene::authored(cfg.world.clone());
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let seeded = cubarium::voxel::habitat::seed(&mut world, &mut flora, &mut fauna);
    eprintln!(
        "seeded: {} stands, {} logs, {} litter tiles, {} littershredders, {} frondgrazer founders",
        seeded.stands,
        seeded.logs,
        seeded.litter_tiles,
        seeded.founders[Founder::Blind.index()],
        seeded.founders[Founder::Browser.index()],
    );
    (world, flora, fauna)
}

/// Mean microseconds per tick over `TICKS`, after `WARMUP`, plus the run's census.
fn arm(threads: usize, with_field: bool) -> (f64, String) {
    let (world, flora, fauna) = habitat();
    let senses = with_field.then(|| {
        let mut s = Senses::new();
        s.settle(&world.view(), &flora.view());
        s
    });
    let mut sim = Sim::new(world, flora, fauna, SimConfig::with_threads(threads), senses);
    for _ in 0..WARMUP {
        sim.step();
    }
    let t0 = Instant::now();
    for _ in 0..TICKS {
        sim.step();
    }
    let per_tick = t0.elapsed().as_secs_f64() * 1e6 / TICKS as f64;

    let av = sim.fauna().view();
    let census = Founder::ALL
        .into_iter()
        .map(|f| {
            format!(
                "{} {} alive/{} bites/{:.3e} assimilated",
                f.name(),
                av.animals.iter().filter(|a| a.founder == Some(f)).count(),
                av.ledger.bites_by_founder[f.index()],
                av.ledger.assimilated_by_founder[f.index()],
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    (per_tick, census)
}

/// Microseconds of one field update, measured directly against a settled field.
fn field_update_us(threads: usize) -> f64 {
    let (world, flora, fauna) = habitat();
    let mut senses = Senses::new();
    let (updates, converged) = senses.settle(&world.view(), &flora.view());
    eprintln!("settle: {updates} updates, converged {converged}");
    // Run the habitat for a while so the field's sources are the live ones — litterfall
    // and the founders' own leavings, not only the seeder's starter tiles.
    let mut sim = Sim::new(world, flora, fauna, SimConfig::with_threads(threads), Some(senses));
    for _ in 0..(WARMUP + TICKS) {
        sim.step();
    }
    let mut settled = sim.senses().expect("the live field").clone();
    let (world, flora, _) = sim.layers();
    // A settled field's `settle` stops after exactly the eight consecutive quiet updates
    // its rule asks for; time those and divide.
    let t0 = Instant::now();
    let (updates, _) = settled.settle(&world.view(), &flora.view());
    let us = t0.elapsed().as_secs_f64() * 1e6 / updates as f64;
    eprintln!("direct: {updates} updates timed");
    us
}

fn main() {
    let threads: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1);

    let update_us = field_update_us(threads);
    let (with, census) = arm(threads, true);
    let (without, _) = arm(threads, false);

    println!("threads {threads}");
    println!("  tick with field       {with:9.1} us");
    println!("  tick without field    {without:9.1} us");
    println!(
        "  end-to-end difference {:9.1} us/tick ({:+.2} % of the sensing tick; the two arms \
         diverge behaviourally)",
        with - without,
        100.0 * (with - without) / with,
    );
    println!(
        "  one field update      {update_us:9.1} us, fired every {UPDATE_TICKS} ticks \
         = {:.1} us/tick ({:.2} % of the sensing tick)",
        update_us / UPDATE_TICKS as f64,
        100.0 * (update_us / UPDATE_TICKS as f64) / with,
    );
    println!("  census (with field): {census}");
}
