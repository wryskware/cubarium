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
    /// Reset observer counters at this cadence for an independent energy audit.
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u64).range(1..=12000))]
    audit_window: u64,
}

#[derive(Default)]
struct AccurateSum {
    sum: f64,
    correction: f64,
}
impl AccurateSum {
    fn add(&mut self, value: f64) {
        let next = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        self.sum = next;
    }
    fn value(&self) -> f64 {
        self.sum + self.correction
    }
}

#[derive(Default)]
struct WindowAudit {
    light: AccurateSum,
    heat: AccurateSum,
    rain: AccurateSum,
    evap: AccurateSum,
    counts: cubarium_core::Telemetry,
}
impl WindowAudit {
    fn observe(&mut self, mut sample: cubarium_core::Telemetry) -> cubarium_core::Telemetry {
        self.light.add(sample.light_in);
        self.heat.add(sample.heat_out);
        self.rain.add(sample.rain_in);
        self.evap.add(sample.evap_out);
        macro_rules! counters {
            ($($field:ident),*) => { $(
                self.counts.$field += sample.$field;
                sample.$field = self.counts.$field;
            )* };
        }
        counters!(
            births,
            deaths_starvation,
            deaths_age,
            deaths_collapse,
            cap_rejections,
            travel_fallbacks,
            travel_ties,
            pairs_considered,
            pairs_unfolded,
            neighbor_truncations
        );
        sample.light_in = self.light.value();
        sample.heat_out = self.heat.value();
        sample.rain_in = self.rain.value();
        sample.evap_out = self.evap.value();
        sample
    }
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

fn audit_passes(
    legacy: [f64; 3],
    corrected_energy: f64,
    windowed_energy: f64,
    care_boundary_energy: f64,
    limits: [f64; 3],
) -> bool {
    [legacy[0], legacy[2], corrected_energy, windowed_energy, care_boundary_energy]
        .into_iter()
        .zip([limits[0], limits[2], limits[1], limits[1], limits[1]])
        .all(|(drift, limit)| drift.is_finite() && drift >= 0.0 && drift < limit)
}

fn run(
    initial: &WorldState,
    ticks: u64,
    care: bool,
    period: u64,
    audit_window: u64,
) -> Result<Value> {
    let mut world = World::from_state(initial.clone()).map_err(|e| anyhow!(e))?;
    let opening_mass = material(initial);
    let opening_energy = energy(initial);
    let opening_water: f64 = initial.fields.w.iter().sum();
    let mut worst = [0.0_f64; 3];
    let opening_energy_ledgers = initial.energy_ledgers();
    let mut worst_corrected_energy = 0.0_f64;
    let mut windowed = WindowAudit::default();
    let mut worst_windowed_energy = 0.0_f64;
    let mut worst_care_energy = 0.0_f64;
    let mut receipt_energy_in = AccurateSum::default();
    let mut receipt_energy_out = AccurateSum::default();
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
                let before_energy = energy(&world.state);
                let receipt = world.apply_care(&CareCommand::standard(
                    world.care().admitted_seq.checked_add(1).context("care seq exhausted")?,
                    world.tick(),
                    kind,
                    targets[(elapsed / period) as usize % targets.len()],
                ));
                let booked = receipt
                    .outcome
                    .applied()
                    .map_or(0.0, |q| q.energy_in - q.energy_out);
                if let Some(q) = receipt.outcome.applied() {
                    receipt_energy_in.add(q.energy_in);
                    receipt_energy_out.add(q.energy_out);
                }
                worst_care_energy =
                    worst_care_energy.max((energy(&world.state) - before_energy - booked).abs());
                receipts.push(json!({"elapsed":elapsed,"kind":kind.as_str(),"receipt":receipt}));
            }
        }
        let counters = world.step();
        let transient_light = counters.light_in;
        let transient_heat = counters.heat_out;
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
        // Difference raw and correction components separately: subtracting two
        // already-rounded large totals would discard the low-order work again.
        // A migrated checkpoint starts with zero corrections; this audits NEW
        // flow only and does not claim to repair pre-migration rounding.
        let corrected_residual = energy(s)
            - opening_energy
            - s.energy_ledgers().net_since(opening_energy_ledgers)
            - (s.care.feed_energy_in - initial.care.feed_energy_in)
            + (s.care.clean_energy_out - initial.care.clean_energy_out);
        ensure!(
            corrected_residual.is_finite(),
            "nonfinite corrected energy audit"
        );
        worst_corrected_energy = worst_corrected_energy.max(corrected_residual.abs());
        let windowed_residual =
            energy(s) - opening_energy - (windowed.light.value() + transient_light)
                + (windowed.heat.value() + transient_heat)
                - receipt_energy_in.value()
                + receipt_energy_out.value();
        ensure!(windowed_residual.is_finite(), "nonfinite windowed audit");
        worst_windowed_energy = worst_windowed_energy.max(windowed_residual.abs());
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
        if (elapsed + 1) % audit_window == 0 {
            windowed.observe(world.telemetry());
        }
    }
    // Relative tolerances scale with the opening inventory, not with cumulative
    // inputs, so additional care cannot relax the audit.
    let limits = [opening_mass, opening_energy, opening_water].map(|n| 1e-8 * n.max(1.0));
    let legacy_audit_passed = worst
        .iter()
        .zip(limits)
        .all(|(drift, limit)| *drift < limit);
    // Same fixed inventory-scaled tolerances, using the persisted compensated
    // representation AND an independent observer. The raw legacy result remains
    // visible below even when it fails; it is never relabeled passing.
    let audit_passed = audit_passes(
        worst, worst_corrected_energy, worst_windowed_energy, worst_care_energy, limits,
    );
    let ledgers = world.care().clone();
    let mut sample = serde_json::to_value(windowed.observe(world.telemetry()))?;
    let s = &world.state;
    let stored_delta = energy(s) - opening_energy;
    let persisted_residual = stored_delta - (s.light_in_total - initial.light_in_total)
        + (s.heat_out_total - initial.heat_out_total)
        - (s.care.feed_energy_in - initial.care.feed_energy_in)
        + (s.care.clean_energy_out - initial.care.clean_energy_out);
    let measured_residual = stored_delta - windowed.light.value() + windowed.heat.value()
        - receipt_energy_in.value()
        + receipt_energy_out.value();
    let corrected_residual = stored_delta
        - s.energy_ledgers().net_since(opening_energy_ledgers)
        - (s.care.feed_energy_in - initial.care.feed_energy_in)
        + (s.care.clean_energy_out - initial.care.clean_energy_out);
    // Hashes are strings so browser/JSON consumers do not round u64 values.
    sample["state_hash"] = json!(cubarium_core::snapshot::state_hash(&world.state).to_string());
    sample["ecology_hash"] = json!(cubarium_core::ecology_hash(&world.state).to_string());
    Ok(
        json!({"care":care,"population_min":population_min,"population_max":population_max,
        "max_absolute_drift":{"material":worst[0],"energy":worst[1],"water":worst[2]},
        "audit_passed":audit_passed,
        "audit_basis":"Unchanged opening-inventory limits; material, water, persisted compensated energy, independent windowed energy, and immediate care-boundary energy must all pass. max_absolute_drift.energy remains the raw legacy diagnostic.",
        "legacy_audit_passed":legacy_audit_passed,
        "corrected_energy_audit":{"max_absolute_drift":worst_corrected_energy,
            "passed":worst_corrected_energy<limits[1],
            "closing_residual":corrected_residual},
        "windowed_energy_audit":{"max_absolute_drift":worst_windowed_energy,
            "passed":worst_windowed_energy<limits[1],"max_care_boundary_drift":worst_care_energy,
            "note":"Independent compensated sum of short-lived observer counters and actual care receipts; same opening-inventory limit."},
        "closing_signed_energy_evidence":{"tick":s.tick,
            "persisted_residual":persisted_residual,"windowed_residual":measured_residual,
            "windowed_minus_persisted_light":windowed.light.value()-(s.light_in_total-initial.light_in_total),
            "persisted_minus_windowed_heat":(s.heat_out_total-initial.heat_out_total)-windowed.heat.value(),
            "receipt_minus_persisted_feed":receipt_energy_in.value()-(s.care.feed_energy_in-initial.care.feed_energy_in),
            "persisted_minus_receipt_clean":(s.care.clean_energy_out-initial.care.clean_energy_out)-receipt_energy_out.value()},
        "audit_limits":{"material":limits[0],"energy":limits[1],"water":limits[2]},
        "opening_inventory":{"material":opening_mass,"energy":opening_energy,"water":opening_water},
        "cumulative_energy_delta":{"light":world.state.light_in_total-initial.light_in_total,
            "heat":world.state.heat_out_total-initial.heat_out_total},
        "corrected_cumulative_energy_delta":{
            "light":s.energy_ledgers().light_in.since(opening_energy_ledgers.light_in),
            "heat":s.energy_ledgers().heat_out.since(opening_energy_ledgers.heat_out)},
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
    let baseline = run(
        &initial,
        args.ticks,
        false,
        args.care_every,
        args.audit_window,
    )?;
    let cared = run(
        &initial,
        args.ticks,
        true,
        args.care_every,
        args.audit_window,
    )?;
    let audit_passed = baseline["audit_passed"] == true && cared["audit_passed"] == true;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "input":args.path,"seed":initial.config.seed,"input_schema":input_schema,"start_tick":initial.tick,
            "ticks":args.ticks,"simulated_seconds":args.ticks as f64 * cubarium_core::DT,
            "care_every_ticks":args.care_every,
            "audit_window_ticks":args.audit_window,
            "ancestry_basis":if args.seed.is_some() { "original founders" } else { "individuals alive at opening checkpoint; earlier ancestry unknown" },
            "note":"Matched in-memory numerical scenario; forms are not lineages. No host durability or visual-response proof; survival is censored at the reported duration.",
            "baseline":baseline,"cared":cared
        }))?
    );
    // Keep the full numerical evidence even on a strict audit failure. This is
    // deliberately still an unsuccessful command, not a relaxed acceptance gate.
    ensure!(
        audit_passed,
        "strict inventory-scaled audit failed; see emitted report"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrected_gate_requires_both_independent_energy_checks_at_the_same_limit() {
        let limits = [1e-5; 3];
        // Raw energy is retained as an explicitly failing legacy diagnostic;
        // the new gate uses actual persisted compensation, not a larger limit.
        assert!(audit_passes([0.0, 1.0, 0.0], 0.0, 0.0, 0.0, limits));
        for bad in [1e-5, 1.0, f64::INFINITY, f64::NAN] {
            assert!(!audit_passes([bad, 0.0, 0.0], 0.0, 0.0, 0.0, limits));
            assert!(!audit_passes([0.0, 0.0, bad], 0.0, 0.0, 0.0, limits));
            assert!(!audit_passes([0.0; 3], bad, 0.0, 0.0, limits));
            assert!(!audit_passes([0.0; 3], 0.0, bad, 0.0, limits));
            assert!(!audit_passes([0.0; 3], 0.0, 0.0, bad, limits));
        }
    }

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
        let first = run(&initial, 420, true, 2400, 200).unwrap();
        assert_eq!(first, run(&initial, 420, true, 2400, 200).unwrap());
        for window in [1, 60, 420] {
            let other = run(&initial, 420, true, 2400, window).unwrap();
            assert_eq!(
                first["final"]["ecology_hash"],
                other["final"]["ecology_hash"]
            );
            assert_eq!(first["final"]["births"], other["final"]["births"]);
            assert_eq!(other["windowed_energy_audit"]["passed"], true);
            assert_eq!(other["corrected_energy_audit"]["passed"], true);
            assert_eq!(other["audit_passed"], true);
        }
        assert_eq!(first["receipts"].as_array().unwrap().len(), 3);
        assert_eq!(first["samples"].as_array().unwrap().len(), 2);
        let baseline = run(&initial, 420, false, 2400, 200).unwrap();
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
