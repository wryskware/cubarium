//! Matched care/no-care audit from an exact checkpoint or seed, entirely in memory.
//! No state writes, HTTP, shim output, or changes to the running cube.

use anyhow::{Context, Result, anyhow, ensure};
use clap::{Parser, ValueEnum};
use cubarium_core::{
    CareCommand, CareDose, CareKind, CareTarget, World, WorldConfig, WorldState, decode_snapshot,
};
use serde_json::{Value, json};
use std::path::PathBuf;

#[path = "shared/audit.rs"]
mod audit;
#[path = "care_compare/local.rs"]
mod local;

use audit::{AccurateSum, Ancestry, WindowAudit, audit_passes, energy, material};

const TARGETS: [CareTarget; 3] = [
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

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum CareSchedule {
    #[default]
    Cycle,
    Feed,
    Rain,
    Clean,
}

impl CareSchedule {
    fn kind(self, elapsed: u64, period: u64) -> Option<CareKind> {
        match self {
            Self::Cycle => scheduled_kind(elapsed, period),
            Self::Feed if elapsed % period == 0 => Some(CareKind::Feed),
            Self::Rain if elapsed % period == 0 => Some(CareKind::Rain),
            Self::Clean if elapsed % period == 0 => Some(CareKind::Clean),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ObservationOptions {
    schedule: CareSchedule,
    dose: CareDose,
    start: u64,
    local_every: u64,
    target_index: usize,
}

impl ObservationOptions {
    fn validate(self, ticks: u64) -> Result<()> {
        ensure!(
            self.start < ticks,
            "care start must precede the closing tick"
        );
        ensure!(self.target_index < TARGETS.len(), "invalid target index");
        if self.local_every > 0 {
            ensure!(
                ticks.div_ceil(self.local_every) <= 2000,
                "local observations are bounded to 2000 intervals; increase --local-every or shorten the run"
            );
        }
        self.dose
            .validate("care comparison")
            .map_err(|e| anyhow!(e))
    }
}

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
    /// Isolate one action; cycle preserves the original Feed/Rain/Clean stress recipe.
    #[arg(long, value_enum, default_value_t = CareSchedule::Cycle)]
    care_kind: CareSchedule,
    #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u16).range(250..=2000))]
    dose_permille: u16,
    /// First action's elapsed boundary. Single-kind recipes act here, not at cycle offsets.
    #[arg(long, default_value_t = 0)]
    care_start: u64,
    /// Optional local snapshots every N ticks; counts still observe EVERY tick. 0 disables.
    #[arg(long, default_value_t = 0)]
    local_every: u64,
    /// First target: 0 interior, 1 side seam, 2 open rim. Later cycles rotate as before.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=2))]
    target_index: u8,
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

#[cfg(test)]
fn run(
    initial: &WorldState,
    ticks: u64,
    care: bool,
    period: u64,
    audit_window: u64,
) -> Result<Value> {
    run_with_options(
        initial,
        ticks,
        care,
        period,
        audit_window,
        ObservationOptions::default(),
    )
}

fn run_with_options(
    initial: &WorldState,
    ticks: u64,
    care: bool,
    period: u64,
    audit_window: u64,
    options: ObservationOptions,
) -> Result<Value> {
    options.validate(ticks)?;
    ensure!(
        initial.hunters.profile().is_none(),
        "care comparison requires a no-hunter opening"
    );
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
    let mut local = if options.local_every > 0 {
        Some(local::LocalObserver::new(initial, &TARGETS)?)
    } else {
        None
    };
    let mut local_samples = Vec::new();
    if let Some(observer) = &local {
        local_samples.push(observer.sample(initial)?);
    }
    let mut extinction_tick = if population_min == 0 {
        Some(initial.tick)
    } else {
        None
    };
    for elapsed in 0..ticks {
        if elapsed == options.start {
            if let Some(observer) = &mut local {
                observer.mark_first_pulse(&world.state)?;
            }
        }
        if care && elapsed >= options.start {
            // Each kind is at least 60 simulated seconds apart; no host cooldown
            // bypass is needed to reproduce this schedule through the real controls.
            let relative = elapsed - options.start;
            let kind = options.schedule.kind(relative, period);
            if let Some(kind) = kind {
                let before_energy = energy(&world.state);
                let receipt = world.apply_care(&CareCommand {
                    seq: world
                        .care()
                        .admitted_seq
                        .checked_add(1)
                        .context("care seq exhausted")?,
                    apply_after_tick: world.tick(),
                    kind,
                    target: TARGETS
                        [((relative / period) as usize + options.target_index) % TARGETS.len()],
                    dose: options.dose,
                });
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
        if let Some(observer) = &mut local {
            observer.observe(&world.state)?;
            if (elapsed + 1) % options.local_every == 0 || elapsed + 1 == ticks {
                local_samples.push(observer.sample(&world.state)?);
            }
        }
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
        worst,
        worst_corrected_energy,
        worst_windowed_energy,
        worst_care_energy,
        limits,
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
    let mut result = json!({"care":care,"population_min":population_min,"population_max":population_max,
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
        "maximum_descendant_depth":ancestry.maximum_depth,"final":sample});
    if options.local_every > 0 {
        result["local_activity"] = json!({"graph_hops":local::HOPS,
            "sample_every_ticks":options.local_every,"samples":local_samples,
            "first_pulse_cohorts":local.as_ref().unwrap().cohorts(),
            "basis":"Fixed identical graph neighborhoods in both arms. Cumulative counts observe each completed tick, excluding the opening instant; they are member-ticks, not unique animals or amounts eaten. fed_this_tick is any field intake, not proof of eating manual crumbs. Feeding mode is distinct. Exact pre-first-pulse local IDs are also followed anywhere; their traits are reported without claiming all are eligible or within sensing reach. Regions may overlap and must not be summed as a disjoint population."});
    }
    Ok(result)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let options = ObservationOptions {
        schedule: args.care_kind,
        dose: CareDose::new(args.dose_permille).map_err(|e| anyhow!(e))?,
        start: args.care_start,
        local_every: args.local_every,
        target_index: usize::from(args.target_index),
    };
    options.validate(args.ticks)?;
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
    let baseline = run_with_options(
        &initial,
        args.ticks,
        false,
        args.care_every,
        args.audit_window,
        options,
    )?;
    let cared = run_with_options(
        &initial,
        args.ticks,
        true,
        args.care_every,
        args.audit_window,
        options,
    )?;
    let audit_passed = baseline["audit_passed"] == true && cared["audit_passed"] == true;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "input":args.path,"seed":initial.config.seed,"input_schema":input_schema,"start_tick":initial.tick,
            "ticks":args.ticks,"simulated_seconds":args.ticks as f64 * cubarium_core::DT,
            "care_every_ticks":args.care_every,
            "care_kind":args.care_kind.to_possible_value().unwrap().get_name(),
            "care_start_tick":args.care_start,"dose_permille":args.dose_permille,
            "first_target_index":args.target_index,"targets":TARGETS,
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
    fn isolated_actions_have_one_common_pulse_and_exact_requested_dose() {
        let initial = World::new(WorldConfig::default()).unwrap().state;
        for (schedule, kind) in [
            (CareSchedule::Feed, "feed"),
            (CareSchedule::Rain, "rain"),
            (CareSchedule::Clean, "clean"),
        ] {
            let options = ObservationOptions {
                schedule,
                dose: CareDose::new(500).unwrap(),
                start: 60,
                local_every: 20,
                target_index: 1,
            };
            let cared = run_with_options(&initial, 420, true, 2400, 200, options).unwrap();
            let baseline = run_with_options(&initial, 420, false, 2400, 200, options).unwrap();
            assert_eq!(cared["receipts"].as_array().unwrap().len(), 1);
            assert_eq!(cared["receipts"][0]["elapsed"], 60);
            assert_eq!(cared["receipts"][0]["kind"], kind);
            assert_eq!(cared["audit_passed"], true);
            assert_eq!(baseline["audit_passed"], true);
            let expected = match kind {
                "feed" => ("feed_material_in", 1.5),
                "rain" => ("rain_depth_in", 2.0),
                _ => ("clean_material_out", 1.0),
            };
            let amount = cared["care_ledgers"][expected.0].as_f64().unwrap();
            if kind == "clean" {
                assert!(amount > 0.0 && amount <= expected.1);
            } else {
                assert!((amount - expected.1).abs() < 1e-12);
            }
            assert_eq!(
                cared["local_activity"]["first_pulse_cohorts"],
                baseline["local_activity"]["first_pulse_cohorts"]
            );
            assert_eq!(
                cared["local_activity"]["samples"][3], baseline["local_activity"]["samples"][3],
                "both arms are still identical at the pre-pulse tick60"
            );
            let samples = cared["local_activity"]["samples"].as_array().unwrap();
            assert_eq!(samples.len(), 22);
            assert_eq!(samples.last().unwrap()["elapsed"], 420);
            let mut without = cared.clone();
            without.as_object_mut().unwrap().remove("local_activity");
            assert_eq!(
                without,
                run_with_options(
                    &initial,
                    420,
                    true,
                    2400,
                    200,
                    ObservationOptions {
                        local_every: 0,
                        ..options
                    }
                )
                .unwrap(),
                "local measurements change neither ecology nor other audit results"
            );
        }
    }

    #[test]
    fn local_report_and_cli_are_explicitly_bounded() {
        assert!(Args::try_parse_from(["audit", "--seed", "1", "--dose-permille", "249"]).is_err());
        assert!(Args::try_parse_from(["audit", "--seed", "1", "--target-index", "3"]).is_err());
        assert!(
            Args::try_parse_from(["audit", "--seed", "1", "--care-kind", "everything"]).is_err()
        );
        assert!(
            ObservationOptions {
                local_every: 1,
                ..Default::default()
            }
            .validate(2001)
            .is_err()
        );
        assert!(
            ObservationOptions {
                start: 420,
                ..Default::default()
            }
            .validate(420)
            .is_err()
        );
        assert!(
            ObservationOptions {
                local_every: 200,
                ..Default::default()
            }
            .validate(400000)
            .is_ok()
        );
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
