//! Matched care/no-care audit from an exact checkpoint or seed, entirely in memory.
//! No state writes, HTTP, shim output, or changes to the running cube.

use anyhow::{Context, Result, anyhow, ensure};
use clap::Parser;
use cubarium_core::{
    CareCommand, CareKind, CareTarget, LifeEvent, OrganismId, World, WorldConfig, WorldState,
    decode_snapshot,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    /// Exact checkpoint to copy into both in-memory scenarios.
    #[arg(required_unless_present = "seed", conflicts_with = "seed")]
    path: Option<PathBuf>,
    /// Generate a fresh in-memory default world instead of loading a checkpoint.
    #[arg(long)]
    seed: Option<u64>,
    /// 20 simulation ticks per second; default is ten simulated minutes.
    #[arg(long, default_value_t = 12000, value_parser = clap::value_parser!(u64).range(420..=1728000))]
    ticks: u64,
    /// Care cycle in ticks: feed at 0, rain at +60, clean at +300. Minimum 60 s.
    #[arg(long, default_value_t = 2400, value_parser = clap::value_parser!(u64).range(1200..=72000))]
    care_every: u64,
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

/// Bounded by the living population, not by total births in a long experiment.
/// A checkpoint's opening cohort is not necessarily its original founder lineage.
struct Ancestry {
    live: BTreeMap<OrganismId, (OrganismId, u64)>,
    maximum_depth: u64,
}

impl Ancestry {
    fn new(state: &WorldState) -> Self {
        Self {
            live: state
                .organisms
                .iter()
                .map(|(id, _)| (id, (id, 0)))
                .collect(),
            maximum_depth: 0,
        }
    }

    fn observe(&mut self, events: &[LifeEvent]) -> Result<()> {
        // Resolve births before removals: a parent can die in the birth's tick.
        for event in events {
            if let LifeEvent::Birth { id, parent, .. } = event {
                let (cohort, depth) = *self.live.get(parent).context("unobserved birth parent")?;
                let depth = depth.checked_add(1).context("ancestry depth overflow")?;
                ensure!(
                    self.live.insert(*id, (cohort, depth)).is_none(),
                    "duplicate birth id"
                );
                self.maximum_depth = self.maximum_depth.max(depth);
            }
        }
        for event in events {
            if let LifeEvent::Death { id, .. } = event {
                ensure!(self.live.remove(id).is_some(), "unobserved death id");
            }
        }
        Ok(())
    }

    fn surviving_cohorts(&self) -> usize {
        self.live
            .values()
            .map(|(cohort, _)| *cohort)
            .collect::<BTreeSet<_>>()
            .len()
    }
}

fn census(s: &WorldState, ancestry: &Ancestry) -> Value {
    let mut forms = [0u32; 8];
    for (_, o) in s.organisms.iter() {
        forms[usize::from(o.phenotype.form).min(7)] += 1;
    }
    json!({"tick":s.tick,"population":s.organisms.len(),"population_by_form":forms,
        "producer":s.fields.p.iter().sum::<f64>(),"water":s.fields.w.iter().sum::<f64>(),
        "surviving_opening_cohorts":ancestry.surviving_cohorts(),
        "maximum_descendant_depth":ancestry.maximum_depth})
}

fn scheduled_kind(elapsed: u64, period: u64) -> Option<CareKind> {
    match elapsed % period {
        0 => Some(CareKind::Feed),
        60 => Some(CareKind::Rain),
        300 => Some(CareKind::Clean),
        _ => None,
    }
}

fn run(initial: &WorldState, ticks: u64, care: bool, period: u64) -> Result<Value> {
    let mut world = World::from_state(initial.clone()).map_err(|e| anyhow!(e))?;
    let opening_mass = material(initial);
    let opening_energy = energy(initial);
    let opening_water: f64 = initial.fields.w.iter().sum();
    let mut worst = [0.0_f64; 3];
    let mut population_min = world.population();
    let mut population_max = population_min;
    let mut receipts = Vec::new();
    let mut ancestry = Ancestry::new(initial);
    let mut samples = vec![census(initial, &ancestry)];
    let mut extinction_tick = if population_min == 0 {
        Some(initial.tick)
    } else {
        None
    };
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
            // Each kind is at least 60 simulated seconds apart; no host cooldown
            // bypass is needed to reproduce this schedule through the real controls.
            let kind = scheduled_kind(elapsed, period);
            if let Some(kind) = kind {
                let receipt = world.apply_care(&CareCommand {
                    seq: world
                        .care()
                        .admitted_seq
                        .checked_add(1)
                        .context("care seq exhausted")?,
                    apply_after_tick: world.tick(),
                    kind,
                    target: targets[(elapsed / period) as usize % targets.len()],
                });
                receipts.push(json!({"elapsed":elapsed,"kind":kind.as_str(),"receipt":receipt}));
            }
        }
        world.step();
        world
            .check_invariants()
            .map_err(|e| anyhow!("tick {}: {e}", world.tick()))?;
        ancestry.observe(&world.drain_events())?;
        ensure!(
            ancestry.live.len() == world.population() as usize,
            "ancestry census mismatch"
        );
        population_min = population_min.min(world.population());
        population_max = population_max.max(world.population());
        if world.population() == 0 && extinction_tick.is_none() {
            extinction_tick = Some(world.tick());
        }
        // Read directly rather than calling telemetry(), which resets counters.
        // At most 145 samples for the maximum 24-hour run.
        if (elapsed + 1) % 12000 == 0 || elapsed + 1 == ticks {
            samples.push(census(&world.state, &ancestry));
        }
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
        "care_ledgers":ledgers,"receipts":receipts,"samples":samples,
        "first_extinction_tick":extinction_tick,
        "surviving_opening_cohorts":ancestry.surviving_cohorts(),
        "maximum_descendant_depth":ancestry.maximum_depth,"final":sample}),
    )
}

fn main() -> Result<()> {
    let args = Args::parse();
    let (input_schema, initial) = if let Some(path) = &args.path {
        let bytes = std::fs::read(path).context("reading exact checkpoint")?;
        let (meta, initial) = decode_snapshot(&bytes).context("decoding checkpoint")?;
        (Some(meta.schema), initial)
    } else {
        let config = WorldConfig {
            seed: args.seed.context("missing seed")?,
            ..WorldConfig::default()
        };
        (None, World::new(config).map_err(|e| anyhow!(e))?.state)
    };
    ensure!(
        initial.care.showers.is_empty(),
        "choose a checkpoint without a pending shower"
    );
    ensure!(
        initial.tick.checked_add(args.ticks).is_some(),
        "tick overflow"
    );
    let baseline = run(&initial, args.ticks, false, args.care_every)?;
    let cared = run(&initial, args.ticks, true, args.care_every)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "input":args.path,"seed":initial.config.seed,"input_schema":input_schema,"start_tick":initial.tick,
            "ticks":args.ticks,"simulated_seconds":args.ticks as f64 * cubarium_core::DT,
            "care_every_ticks":args.care_every,
            "ancestry_basis":if args.seed.is_some() { "original founders" } else { "individuals alive at opening checkpoint; earlier ancestry unknown" },
            "note":"Matched in-memory numerical scenario; forms are not lineages. No host durability or visual-response proof; survival is censored at the reported duration.",
            "baseline":baseline,"cared":cared
        }))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_requires_exactly_one_source_and_bounded_schedules() {
        assert!(Args::try_parse_from(["audit"]).is_err());
        assert!(Args::try_parse_from(["audit", "world.cubw", "--seed", "1"]).is_err());
        assert!(Args::try_parse_from(["audit", "--seed", "1", "--ticks", "419"]).is_err());
        assert!(Args::try_parse_from(["audit", "--seed", "1", "--ticks", "1728001"]).is_err());
        assert!(Args::try_parse_from(["audit", "--seed", "1", "--care-every", "1199"]).is_err());
        let args = Args::try_parse_from(["audit", "world.cubw"]).unwrap();
        assert_eq!(args.ticks, 12000);
        assert_eq!(args.care_every, 2400);
    }

    #[test]
    fn schedules_only_three_inputs_per_cycle() {
        for period in [1200, 2400, 12000, 72000] {
            let actions: Vec<_> = (0..period * 2)
                .filter_map(|tick| scheduled_kind(tick, period).map(|kind| (tick, kind)))
                .collect();
            assert_eq!(
                actions,
                vec![
                    (0, CareKind::Feed),
                    (60, CareKind::Rain),
                    (300, CareKind::Clean),
                    (period, CareKind::Feed),
                    (period + 60, CareKind::Rain),
                    (period + 300, CareKind::Clean)
                ]
            );
        }
    }

    #[test]
    fn ancestry_preserves_a_dead_parent_and_distinguishes_reused_slots() {
        let state = World::new(WorldConfig::default()).unwrap().state;
        let mut ancestry = Ancestry::new(&state);
        let parent = state.organisms.iter().next().unwrap().0;
        let child = OrganismId {
            slot: parent.slot,
            generation: parent.generation + 1,
        };
        let death = LifeEvent::Death {
            tick: 1,
            id: parent,
            age_ticks: 1,
            cause: cubarium_core::organism::DeathCause::Age,
            births: 1,
            genome: 0,
        };
        let birth = LifeEvent::Birth {
            tick: 1,
            id: child,
            parent,
            parent_age_ticks: 1,
            parent_births: 1,
            genome: 0,
            origin: cubarium_core::organism::Origin::Descendant,
            mutations: vec![],
        };
        ancestry.observe(&[death, birth.clone()]).unwrap();
        assert_eq!(ancestry.live.get(&child), Some(&(parent, 1)));
        assert!(!ancestry.live.contains_key(&parent));
        assert_eq!(ancestry.live.len(), state.organisms.len());
        assert_eq!(ancestry.surviving_cohorts(), state.organisms.len());
        assert_eq!(ancestry.maximum_depth, 1);
        assert!(ancestry.observe(&[birth]).is_err());
    }

    #[test]
    fn short_matched_audit_is_repeatable_and_observation_is_inert() {
        let initial = World::new(WorldConfig::default()).unwrap().state;
        let first = run(&initial, 420, true, 2400).unwrap();
        assert_eq!(first, run(&initial, 420, true, 2400).unwrap());
        assert_eq!(first["receipts"].as_array().unwrap().len(), 3);
        assert_eq!(first["samples"].as_array().unwrap().len(), 2);
        let baseline = run(&initial, 420, false, 2400).unwrap();
        let mut reference = World::from_state(initial.clone()).unwrap();
        for _ in 0..420 {
            reference.step();
            reference.drain_events();
        }
        assert_eq!(
            baseline["final"]["ecology_hash"],
            cubarium_core::ecology_hash(&reference.state).to_string()
        );
        assert_eq!(baseline["care_ledgers"]["admitted_seq"], 0);
    }
}
