//! A day of the closed water budget on a **generated** world: does it cycle, or lock dry?
//!
//! ```text
//! cargo run --release -p cubarium-voxel --example water_cycle -- \
//!     [HOURS] [SEED] [TRIGGER_FRACTION] [SHOWER_M3] [EVAP_M_PER_S] [THREADS]
//! ```
//!
//! One CSV row per simulated ten minutes on stdout — stored, free, pore, aquifer,
//! atmosphere, showers so far, the residual — and a verdict on stderr. Twenty-four
//! simulated hours is 1,728,000 ticks.
//!
//! **Experiment conditions**, not model defaults, printed in the header of every run:
//! the shower rate is the flora study harness's own rain rate
//! (`crates/cubarium-voxel-flora/examples/harness/mod.rs`, `HARNESS_RAIN_M_PER_S` =
//! 2e-4 m/s), the aquifer is charged to a metre above the basin floor exactly as the
//! study charges it, and the outlet is open — which under a closed budget is the return
//! flow, not an export. Evaporation defaults to **half** the shower rate and is an
//! argument: it has to be under the shower rate or a falling drop is lifted again in the
//! same tick — `evaporate` runs right after `rain` — and no rain ever reaches the soil.
//!
//! Nothing here is tuned and nothing here is a balance claim. It is a measurement.

use cubarium_voxel::{Command, Config, TICK_HZ, World};

/// The flora study harness's rain rate, reused as the shower rate so the plants meet the
/// water they were last seen alive in. Under the default 128 x 24 footprint (192 m²) it
/// is 0.0384 m³/s while a shower runs.
const HARNESS_RAIN_M_PER_S: f64 = 0.0002;

/// One report line per simulated ten minutes.
const TICKS_PER_ROW: u64 = 10 * 60 * TICK_HZ as u64;

fn arg<T: std::str::FromStr>(n: usize) -> Option<T> {
    std::env::args().nth(n).and_then(|a| a.parse().ok())
}

fn main() {
    let hours: f64 = arg(1).unwrap_or(24.0);
    let seed: u64 = arg(2).unwrap_or(1);
    let trigger: f64 = arg(3).unwrap_or(0.02);
    let shower_m3: f64 = arg(4).unwrap_or(5.0);
    let evaporation: f64 = arg(5).unwrap_or(0.0001);
    // Execution only. This world is small enough that the exchange's column scan barely
    // splits, so seeds run in parallel *processes* rather than threads.
    let threads: usize = arg(6).unwrap_or(2);

    // Generate once dry to find where the generator put the basin, then generate the
    // world the run uses with the table charged a metre above that floor. Generation is
    // deterministic in the seed, so the second world is the first one with water in it —
    // the flora study's own move.
    let dry = Config {
        seed,
        rain_m_per_s: HARNESS_RAIN_M_PER_S,
        ..Config::default()
    };
    let basin_floor_m = World::new(dry.clone())
        .outlet_cell()
        .map_or(0.0, |(_, y, _)| f64::from(y))
        * dry.voxel_m;
    let config = Config {
        initial_aquifer_head_m: basin_floor_m + 1.0,
        evaporation_m_per_s: evaporation,
        closed_water_budget: true,
        shower_trigger_fraction: trigger,
        shower_volume_m3: shower_m3,
        ..dry
    };
    let mut world = World::new(config.clone());
    world.apply(Command::SetOutlet { open: true });

    let total = world.view().ledger.expected_total();
    let footprint = f64::from(config.width) * f64::from(config.depth) * config.cell_area();
    eprintln!(
        "water_cycle: seed {seed}, {hours} h closed budget on the generated world; \
         shower rate {HARNESS_RAIN_M_PER_S} m/s over {footprint:.0} m2 \
         ({:.4} m3/s while raining), shower volume {shower_m3} m3, trigger {trigger} of \
         total water {total:.2} m3 ({:.3} m3 aloft), evaporation {evaporation} m/s, \
         outlet {} m3/s OPEN (the return flow), aquifer head {:.2} m",
        HARNESS_RAIN_M_PER_S * footprint,
        trigger * total,
        config.outlet_m3_per_s,
        config.initial_aquifer_head_m,
    );

    println!(
        "sim_min,stored,free,pore,aquifer,atmosphere,shower_left,showers,rain_in,evap,outlet,residual"
    );
    let total_ticks = (hours * 3600.0 * f64::from(TICK_HZ)) as u64;
    row(&world);
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let mut halves = [Vec::new(), Vec::new()];
    for tick in 1..=total_ticks {
        world.step_with(threads);
        if tick % TICKS_PER_ROW == 0 {
            row(&world);
            let stored = world.view().stored_m3();
            lo = lo.min(stored);
            hi = hi.max(stored);
            halves[usize::from(tick * 2 > total_ticks)].push(stored);
        }
    }

    let v = world.view();
    let drift = (mean(&halves[1]) - mean(&halves[0])) / total;
    eprintln!(
        "water_cycle: seed {seed} after {hours} h — stored {lo:.2}–{hi:.2} m3 \
         (swing {:.2}, drift {:+.3}% of total), atmosphere {:.3} m3, {} showers, \
         residual {:.2e} / atmosphere {:.2e} / total {:.2e}; {}",
        hi - lo,
        100.0 * drift,
        v.atmosphere_m3,
        v.ledger.showers,
        v.water_residual(),
        v.atmosphere_residual(),
        v.total_residual(),
        verdict(&v, drift),
    );
}

fn verdict(v: &cubarium_voxel::VoxelView<'_>, drift: f64) -> String {
    let locked = v.ledger.showers == 0 || v.atmosphere_m3 < 1e-9;
    if locked {
        format!(
            "LOCKED DRY: {} shower(s) and {:.3e} m3 aloft against a trigger of {:.3} m3",
            v.ledger.showers,
            v.atmosphere_m3,
            v.config.shower_trigger_fraction * v.ledger.expected_total()
        )
    } else if drift.abs() <= 0.02 {
        format!(
            "BOUNDED CYCLE: drift {:+.3}% over the window",
            100.0 * drift
        )
    } else {
        format!(
            "DRIFTING: {:+.3}% of total water over the window",
            100.0 * drift
        )
    }
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

fn row(world: &World) {
    let v = world.view();
    let vol = v.config.voxel_volume();
    let (mut free, mut pore) = (0.0, 0.0);
    for (i, m) in v.material.iter().enumerate() {
        if !m.is_solid() {
            free += v.free[i] * vol;
        }
        pore += v.pore[i] * vol * m.pore_capacity();
    }
    println!(
        "{},{:.4},{:.4},{:.4},{:.4},{:.5},{:.5},{},{:.4},{:.4},{:.4},{:.3e}",
        v.tick / (60 * u64::from(TICK_HZ)),
        v.stored_m3(),
        free,
        pore,
        v.aquifer_m3,
        v.atmosphere_m3,
        world.shower_left_m3(),
        v.ledger.showers,
        v.ledger.rain_in,
        v.ledger.evaporation_out,
        v.ledger.outlet_out,
        v.total_residual(),
    );
}
