//! Short matched care/no-care audit from an exact checkpoint, entirely in memory.
//! No state writes, HTTP, shim output, or changes to the running cube.

use anyhow::{Context, Result, anyhow, ensure};
use clap::Parser;
use cubarium_core::{CareCommand, CareKind, CareTarget, World, WorldState, decode_snapshot};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    /// Exact checkpoint to copy into both in-memory scenarios.
    path: PathBuf,
    /// 20 simulation ticks per second; default is ten simulated minutes.
    #[arg(long, default_value_t = 12000, value_parser = clap::value_parser!(u64).range(420..=72000))]
    ticks: u64,
}

fn material(s: &WorldState) -> f64 {
    s.fields.total_material() + s.organisms.iter().map(|(_, o)| o.material()).sum::<f64>()
}

fn energy(s: &WorldState) -> f64 {
    let reserve = s.config.organism.reserve_energy_density;
    s.fields.p.iter().sum::<f64>() * s.config.producer.energy_density
        + s.fields.f.iter().sum::<f64>() * s.config.fruit.energy_density
        + s.fields.de.iter().sum::<f64>()
        + s.organisms
            .iter()
            .map(|(_, o)| {
                o.energy
                    + reserve * o.reserve
                    + o.escrow
                        .as_ref()
                        .map_or(0.0, |e| e.energy + reserve * (e.structure + e.reserve))
            })
            .sum::<f64>()
}

fn run(initial: &WorldState, ticks: u64, care: bool) -> Result<Value> {
    let mut world = World::from_state(initial.clone()).map_err(|e| anyhow!(e))?;
    let opening_mass = material(initial);
    let opening_energy = energy(initial);
    let opening_water: f64 = initial.fields.w.iter().sum();
    let mut worst = [0.0_f64; 3];
    let mut population_min = world.population();
    let mut population_max = population_min;
    let mut receipts = Vec::new();
    // Ordinary face, side seam, open bottom rim: doses use the same constants.
    let targets = [
        CareTarget {
            face: 0,
            u: 32.0,
            v: 48.0,
        },
        CareTarget {
            face: 0,
            u: 63.5,
            v: 48.0,
        },
        CareTarget {
            face: 1,
            u: 32.0,
            v: 63.5,
        },
    ];
    for elapsed in 0..ticks {
        if care {
            // Each kind is at least 120 simulated seconds apart; no host cooldown
            // bypass is needed to reproduce this schedule through the real controls.
            let kind = match elapsed % 2400 {
                0 => Some(CareKind::Feed),
                60 => Some(CareKind::Rain),
                300 => Some(CareKind::Clean),
                _ => None,
            };
            if let Some(kind) = kind {
                let receipt = world.apply_care(&CareCommand {
                    seq: world
                        .care()
                        .admitted_seq
                        .checked_add(1)
                        .context("care seq exhausted")?,
                    apply_after_tick: world.tick(),
                    kind,
                    target: targets[(elapsed / 2400) as usize % targets.len()],
                });
                receipts.push(json!({"elapsed":elapsed,"kind":kind.as_str(),"receipt":receipt}));
            }
        }
        world.step();
        world
            .check_invariants()
            .map_err(|e| anyhow!("tick {}: {e}", world.tick()))?;
        world.drain_events(); // Keep the observer's transient event list bounded.
        population_min = population_min.min(world.population());
        population_max = population_max.max(world.population());
        let s = &world.state;
        let delta_feed = s.care.feed_material_in - initial.care.feed_material_in;
        let delta_clean = s.care.clean_material_out - initial.care.clean_material_out;
        let residuals = [
            material(s)
                - opening_mass
                - (s.external_material_in - initial.external_material_in)
                - delta_feed
                + delta_clean,
            energy(s) - opening_energy - (s.light_in_total - initial.light_in_total)
                + (s.heat_out_total - initial.heat_out_total)
                - (s.care.feed_energy_in - initial.care.feed_energy_in)
                + (s.care.clean_energy_out - initial.care.clean_energy_out),
            s.fields.w.iter().sum::<f64>()
                - opening_water
                - (s.rain_in_total - initial.rain_in_total)
                + (s.evap_out_total - initial.evap_out_total),
        ];
        for (peak, residual) in worst.iter_mut().zip(residuals) {
            ensure!(
                residual.is_finite(),
                "nonfinite audit at tick {}",
                world.tick()
            );
            *peak = peak.max(residual.abs());
        }
    }
    // Relative tolerances scale with the opening inventory, not with cumulative
    // inputs, so additional care cannot relax the audit.
    ensure!(
        worst[0] < 1e-8 * opening_mass.max(1.0),
        "material drift {}",
        worst[0]
    );
    ensure!(
        worst[1] < 1e-8 * opening_energy.max(1.0),
        "energy drift {}",
        worst[1]
    );
    ensure!(
        worst[2] < 1e-8 * opening_water.max(1.0),
        "water drift {}",
        worst[2]
    );
    let ledgers = world.care().clone();
    let mut sample = serde_json::to_value(world.telemetry())?;
    // Hashes are strings so browser/JSON consumers do not round u64 values.
    sample["state_hash"] = json!(cubarium_core::snapshot::state_hash(&world.state).to_string());
    sample["ecology_hash"] = json!(cubarium_core::ecology_hash(&world.state).to_string());
    Ok(
        json!({"care":care,"population_min":population_min,"population_max":population_max,
        "max_absolute_drift":{"material":worst[0],"energy":worst[1],"water":worst[2]},
        "care_ledgers":ledgers,"receipts":receipts,"final":sample}),
    )
}

fn main() -> Result<()> {
    let args = Args::parse();
    let bytes = std::fs::read(&args.path).context("reading exact checkpoint")?;
    let (meta, initial) = decode_snapshot(&bytes).context("decoding checkpoint")?;
    ensure!(
        initial.care.showers.is_empty(),
        "choose a checkpoint without a pending shower"
    );
    ensure!(
        initial.tick.checked_add(args.ticks).is_some(),
        "tick overflow"
    );
    let baseline = run(&initial, args.ticks, false)?;
    let cared = run(&initial, args.ticks, true)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "input":args.path,"input_schema":meta.schema,"start_tick":initial.tick,
            "ticks":args.ticks,"simulated_seconds":args.ticks as f64 * cubarium_core::DT,
            "note":"Short, single-checkpoint numerical scenario; not host durability, visual-response, or long-run stability evidence.",
            "baseline":baseline,"cared":cared
        }))?
    );
    Ok(())
}
