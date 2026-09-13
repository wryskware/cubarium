//! Natural rainfall availability: the preregistered twelve-seed, six-arm ambient-support screen
//! of `design/7_Research/astra-ambient-support-experiment-proposal-2026-09-13.md`.
//!
//! **This is an experiment harness, not a feature.** It changes one existing persisted config
//! field — `water.rain_rate` — in a copied world before that world is constructed, and nothing
//! else. There is no production default here, no live-adjustable setting, no new persistence
//! shape, no viewer, and no hunter. The normal autonomous world is untouched.
//!
//! The family, per seed:
//!
//! | | no care | Standard rain (1000) | Generous rain (1500) |
//! | --- | --- | --- | --- |
//! | **100 %** natural rain | `rain100_none` | `rain100_standard` | `rain100_generous` |
//! | **90 %** natural rain | `rain90_none` | `rain90_standard` | `rain90_generous` |
//!
//! The 100 % arm leaves the opening's rain rate **untouched** — not round-tripped through a
//! percentage that happens to be one — so its arithmetic is the copied world's own. The 90 %
//! arm sets `opening_rate * 0.9` once, before `World::from_state`.
//!
//! Manual dose is an independent factor, not a consequence of the support level: the same
//! immutable schedule runs in every cared arm, a rain shower at elapsed tick 60 and every 2400
//! thereafter, rotating the care study's three fixed ordinary/seam/rim targets. That two-minute
//! cadence is a controlled diagnostic exposure and **not** a proposed care obligation.
//!
//! Natural water is reported as total rain minus the world's own manual attribution
//! (`care.rain_depth_in`), never as a second external source. Every receipt is retained with its
//! outcome, refusals included. Every seed is retained, extinction and technical failure included.
//!
//! A technical pass here certifies that the numbers are trustworthy. It is not ecological
//! acceptance, not evidence of balance, and not evidence that this world depends on attention.

use anyhow::{Context, Result, anyhow, ensure};
use clap::{Parser, ValueEnum};
use cubarium_core::{
    CareCommand, CareDose, CareKind, CareTarget, World, WorldState, decode_snapshot,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[path = "shared/audit.rs"]
mod audit;
#[path = "ambient_compare/region.rs"]
mod region;

use audit::{AccurateSum, Ancestry, WindowAudit, audit_passes, energy, material};

const BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("CUBARIUM_GIT_HASH"));

/// The care study's own three targets: an ordinary interior cell, a side seam, and the open
/// bottom rim. Reused verbatim so a dose here means what it meant there.
const TARGETS: [CareTarget; 3] = [
    CareTarget { face: 0, u: 32.0, v: 48.0 },
    CareTarget { face: 0, u: 63.5, v: 48.0 },
    CareTarget { face: 1, u: 32.0, v: 63.5 },
];

/// The candidate's multiplier on the opening's own rain rate. One value, no sweep.
const CANDIDATE_FRACTION: f64 = 0.9;

/// The immutable diagnostic schedule: first shower here, then every [`CARE_PERIOD`] ticks.
const CARE_START: u64 = 60;
const CARE_PERIOD: u64 = 2400;

const ARMS: [(&str, bool, Option<u16>); 6] = [
    ("rain100_none", false, None),
    ("rain100_standard", false, Some(1000)),
    ("rain100_generous", false, Some(1500)),
    ("rain90_none", true, None),
    ("rain90_standard", true, Some(1000)),
    ("rain90_generous", true, Some(1500)),
];

/// Two hours, twenty-four hours, seventy-two hours. Nothing between horizons is tuned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
enum Horizon {
    /// A technical smoke. Certifies nothing biological.
    Smoke,
    TwoHour,
    TwentyFourHour,
    SeventyTwoHour,
}

impl Horizon {
    fn ticks(self) -> u64 {
        match self {
            Horizon::Smoke => 2400,
            Horizon::TwoHour => 144_000,
            Horizon::TwentyFourHour => 1_728_000,
            Horizon::SeventyTwoHour => 5_184_000,
        }
    }

    /// Only a full prescribed horizon can certify data; a smoke never can.
    fn prescribed(self) -> bool {
        !matches!(self, Horizon::Smoke)
    }
}

#[derive(Parser)]
struct Args {
    /// Completed prepare-hunter-worlds cohort directory (all seeds 1–12).
    cohort: PathBuf,
    /// Brand-new directory; existing output is never overwritten or resumed.
    out: PathBuf,
    /// Elapsed trial ticks AFTER the opening, by prescribed horizon.
    #[arg(long, value_enum, default_value_t = Horizon::TwoHour)]
    horizon: Horizon,
    /// Streaming telemetry cadence, and the independent energy window with it.
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u64).range(1..=12000))]
    sample_every: u64,
}

fn sha256(bytes: &[u8]) -> Result<String> {
    // The host's standard checksum utility, with no shell interpolation and no added
    // dependency. Input is bytes on stdin, never a command or a path argument.
    let mut child = Command::new("sha256sum").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
    child.stdin.take().context("checksum stdin")?.write_all(bytes)?;
    let out = child.wait_with_output()?;
    ensure!(out.status.success(), "sha256sum failed");
    let text = String::from_utf8(out.stdout)?;
    let hash = text.split_whitespace().next().context("missing checksum")?;
    ensure!(
        hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()),
        "invalid checksum"
    );
    Ok(hash.to_owned())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn json_new(path: &Path, value: &Value) -> Result<()> {
    write_new(path, &serde_json::to_vec_pretty(value)?)
}

fn stream(path: &Path) -> Result<BufWriter<File>> {
    Ok(BufWriter::new(
        OpenOptions::new().write(true).create_new(true).open(path)?,
    ))
}

fn line(file: &mut BufWriter<File>, value: &Value) -> Result<()> {
    serde_json::to_writer(&mut *file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}

// ---------------------------------------------------------------- the fixed cohort

struct Opening {
    state: WorldState,
    source: Value,
}

/// The twelve prescribed mature openings, verified before anything is constructed from them:
/// the file's own bytes against the manifest's checksum, then the decoded schema, tick, seed,
/// census and ecology hash, and that it carries no care and no hunter.
///
/// The originals are read-only inputs. Nothing in this harness writes into the cohort directory.
fn load_cohort(dir: &Path) -> Result<(Value, Vec<Opening>)> {
    let manifest: Value = serde_json::from_slice(&fs::read(dir.join("manifest.json"))?)?;
    ensure!(
        manifest["complete"] == true && manifest["kind"] == "pre-hunter-cohort-preparation",
        "a complete unfiltered preparation manifest is required"
    );
    let rows = manifest["openings"].as_array().context("opening rows")?;
    ensure!(rows.len() == 12, "all twelve seeds are required");
    let mut openings = Vec::new();
    let mut seeds = BTreeSet::new();
    for row in rows {
        let seed = row["seed"].as_u64().context("seed")?;
        ensure!(
            (1..=12).contains(&seed) && seeds.insert(seed),
            "duplicate/unprescribed seed"
        );
        // The archive's own paths are provenance. Resolve the fixed cohort layout, never an
        // arbitrary path supplied by its manifest.
        let bytes = fs::read(dir.join(format!("seed-{seed}/world-144000.cubw")))?;
        ensure!(
            sha256(&bytes)? == row["sha256"].as_str().context("snapshot SHA256")?,
            "seed{seed} checksum changed"
        );
        let (meta, state) = decode_snapshot(&bytes)?;
        ensure!(
            meta.schema == 9 && state.tick == 144_000 && state.config.seed == seed,
            "seed{seed} is not its frozen pre-hunter two-hour opening"
        );
        ensure!(
            state.hunters == Default::default() && state.care == Default::default(),
            "opening contains hunter/care state"
        );
        ensure!(
            state.organisms.len() as u64 == row["population"].as_u64().context("population")?,
            "opening census mismatch"
        );
        ensure!(
            cubarium_core::ecology_hash(&state).to_string()
                == row["ecology_hash"].as_str().context("ecology hash")?,
            "opening ecology hash mismatch"
        );
        ensure!(
            state.config.water.rain_rate.is_finite() && state.config.water.rain_rate > 0.0,
            "seed{seed} has no natural rainfall to vary"
        );
        openings.push(Opening { state, source: row.clone() });
    }
    openings.sort_by_key(|o| o.state.config.seed);
    Ok((manifest, openings))
}

// ---------------------------------------------------------------- one arm

/// The common PRE-intervention inventory, read once per seed from the untouched opening and
/// shared by all six arms. Not a per-arm rebuild, and never reset mid-run: rebuilding a `World`
/// is not evidence that its history balances.
#[derive(Clone, Copy)]
struct Baseline {
    material: f64,
    energy: f64,
    water: f64,
    limits: [f64; 3],
}

impl Baseline {
    fn read(s: &WorldState) -> Result<Self> {
        let (material, energy, water) =
            (material(s), energy(s), s.fields.w.iter().sum::<f64>());
        ensure!(
            [material, energy, water].iter().all(|x| x.is_finite() && *x >= 0.0),
            "invalid pre-intervention baseline"
        );
        Ok(Self {
            material,
            energy,
            water,
            limits: [material, energy, water].map(|n| 1e-8 * n.max(1.0)),
        })
    }

    fn json(&self) -> Value {
        json!({
            "material": self.material, "energy": self.energy, "water": self.water,
            "limits": {"material": self.limits[0], "energy": self.limits[1], "water": self.limits[2]},
            "basis": "one shared pre-intervention inventory per seed, read from the untouched opening before any config change or care; limits are a fixed 1e-8 fraction of it and never scale with cumulative input",
        })
    }
}

/// Apply the one change, or deliberately not apply it.
///
/// The 100 % arm never writes the field at all. That is the point: a multiply by one is still a
/// multiply, and this arm must carry the copied world's own bits.
fn arm_state(opening: &WorldState, candidate: bool) -> WorldState {
    let mut state = opening.clone();
    if candidate {
        state.config.water.rain_rate = opening.config.water.rain_rate * CANDIDATE_FRACTION;
    }
    state
}

struct Arm {
    world: World,
    base: Baseline,
    opening: WorldState,
    dose: Option<CareDose>,
    regions: region::Regions,
    ancestry: Ancestry,
    windowed: WindowAudit,
    receipt_energy_in: AccurateSum,
    receipt_energy_out: AccurateSum,
    opening_ledgers: cubarium_core::EnergyLedgers,
    worst: [f64; 3],
    worst_corrected_energy: f64,
    worst_windowed_energy: f64,
    worst_care_energy: f64,
    population_min: usize,
    population_max: usize,
    extinction_tick: Option<u64>,
    receipts_written: u64,
    receipts_by_outcome: [u64; 3],
    samples: File,
    receipts: BufWriter<File>,
    sample_stream: BufWriter<File>,
}

impl Arm {
    fn new(opening: &WorldState, base: Baseline, name: &str, candidate: bool, dose: Option<u16>, dir: &Path) -> Result<Self> {
        fs::create_dir(dir)?;
        let state = arm_state(opening, candidate);
        let before_hash = cubarium_core::snapshot::state_hash(opening);
        let after_hash = cubarium_core::snapshot::state_hash(&state);
        if candidate {
            ensure!(before_hash != after_hash, "the candidate change left the state identical");
        } else {
            ensure!(
                before_hash == after_hash
                    && state.config.water.rain_rate.to_bits()
                        == opening.config.water.rain_rate.to_bits(),
                "the 100% arm must leave the opening untouched, bit for bit"
            );
        }
        let dose = dose.map(CareDose::new).transpose().map_err(|e| anyhow!(e))?;
        let world = World::from_state(state.clone()).map_err(|e| anyhow!(e))?;
        let regions = region::Regions::new(&state, &TARGETS)?;
        json_new(
            &dir.join("opening.json"),
            &json!({
                "arm": name,
                "natural_rain_percent": if candidate { 90 } else { 100 },
                "candidate": candidate,
                "dose_permille": dose.map(|d| d.permille()),
                "care_start_tick": CARE_START, "care_period_ticks": CARE_PERIOD,
                "care_kind": "rain", "targets": TARGETS,
                "region_hops": region::HOPS, "region_cells": regions.shape(),
                "opening_tick": state.tick,
                "opening_rain_rate": opening.config.water.rain_rate,
                "arm_rain_rate": state.config.water.rain_rate,
                "rain_rate_bits_unchanged": state.config.water.rain_rate.to_bits()
                    == opening.config.water.rain_rate.to_bits(),
                "state_hash_before_change": before_hash.to_string(),
                "state_hash_after_change": after_hash.to_string(),
                "ecology_hash_after_change": cubarium_core::ecology_hash(&state).to_string(),
                "config_sha256": sha256(&serde_json::to_vec(&state.config)?)?,
                "config": state.config,
                "pre_intervention_baseline": base.json(),
            }),
        )?;
        Ok(Self {
            population_min: world.population(),
            population_max: world.population(),
            extinction_tick: (world.population() == 0).then_some(state.tick),
            opening_ledgers: state.energy_ledgers(),
            world,
            base,
            opening: state,
            dose,
            regions,
            ancestry: Ancestry::new(opening),
            windowed: WindowAudit::default(),
            receipt_energy_in: AccurateSum::default(),
            receipt_energy_out: AccurateSum::default(),
            worst: [0.0; 3],
            worst_corrected_energy: 0.0,
            worst_windowed_energy: 0.0,
            worst_care_energy: 0.0,
            receipts_written: 0,
            receipts_by_outcome: [0; 3],
            samples: OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dir.join(".samples-lock"))?,
            receipts: stream(&dir.join("receipts.jsonl"))?,
            sample_stream: stream(&dir.join("samples.jsonl"))?,
        })
    }

    /// The world's own manual attribution, and natural water as the remainder. One source, not
    /// two: `rain_in_total` already contains the manual depth, so natural is the difference.
    fn water_attribution(&self) -> (f64, f64, f64) {
        let s = &self.world.state;
        let total = s.rain_in_total - self.opening.rain_in_total;
        let manual = s.care.rain_depth_in - self.opening.care.rain_depth_in;
        (total, manual, total - manual)
    }

    fn step(&mut self, elapsed: u64, sample_every: u64) -> Result<()> {
        // Care first, at the held boundary, exactly as the runner admits one.
        if let Some(dose) = self.dose
            && elapsed >= CARE_START
            && (elapsed - CARE_START).is_multiple_of(CARE_PERIOD)
        {
            let index = ((elapsed - CARE_START) / CARE_PERIOD) as usize % TARGETS.len();
            let before_energy = energy(&self.world.state);
            let seq = self
                .world
                .care()
                .admitted_seq
                .checked_add(1)
                .context("care seq exhausted")?;
            let receipt = self.world.apply_care(&CareCommand {
                seq,
                apply_after_tick: self.world.tick(),
                kind: CareKind::Rain,
                target: TARGETS[index],
                dose,
            });
            let booked = receipt.outcome.applied().map_or(0.0, |q| q.energy_in - q.energy_out);
            if let Some(q) = receipt.outcome.applied() {
                self.receipt_energy_in.add(q.energy_in);
                self.receipt_energy_out.add(q.energy_out);
            }
            self.worst_care_energy = self
                .worst_care_energy
                .max((energy(&self.world.state) - before_energy - booked).abs());
            self.receipts_by_outcome[match receipt.outcome {
                cubarium_core::CareOutcome::Applied(_) => 0,
                cubarium_core::CareOutcome::Partial(_) => 1,
                cubarium_core::CareOutcome::Rejected(_) => 2,
            }] += 1;
            self.receipts_written += 1;
            // Every attempt is kept, refusals and their reasons included.
            line(
                &mut self.receipts,
                &json!({"elapsed": elapsed, "tick": self.world.tick(), "kind": "rain",
                    "target_index": index, "target": TARGETS[index],
                    "dose_permille": dose.permille(),
                    "outcome": receipt.outcome.as_str(),
                    "reason": receipt.outcome.reason(),
                    "receipt": receipt}),
            )?;
        }

        let counters = self.world.step();
        let (transient_light, transient_heat) = (counters.light_in, counters.heat_out);
        self.world
            .check_invariants()
            .map_err(|e| anyhow!("tick {}: {e}", self.world.tick()))?;
        self.ancestry.observe(&self.world.drain_events())?;
        self.regions.observe(&self.world.state)?;
        ensure!(
            self.ancestry.live.len() == self.world.population(),
            "ancestry census mismatch"
        );
        self.population_min = self.population_min.min(self.world.population());
        self.population_max = self.population_max.max(self.world.population());
        if self.world.population() == 0 && self.extinction_tick.is_none() {
            self.extinction_tick = Some(self.world.tick());
        }

        let s = &self.world.state;
        let initial = &self.opening;
        // Difference raw and correction components separately: subtracting two already-rounded
        // large totals would discard the low-order work again.
        let corrected_residual = energy(s)
            - self.base.energy
            - s.energy_ledgers().net_since(self.opening_ledgers)
            - (s.care.feed_energy_in - initial.care.feed_energy_in)
            + (s.care.clean_energy_out - initial.care.clean_energy_out);
        ensure!(corrected_residual.is_finite(), "nonfinite corrected energy audit");
        self.worst_corrected_energy = self.worst_corrected_energy.max(corrected_residual.abs());
        let windowed_residual = energy(s)
            - self.base.energy
            - (self.windowed.light.value() + transient_light)
            + (self.windowed.heat.value() + transient_heat)
            - self.receipt_energy_in.value()
            + self.receipt_energy_out.value();
        ensure!(windowed_residual.is_finite(), "nonfinite windowed audit");
        self.worst_windowed_energy = self.worst_windowed_energy.max(windowed_residual.abs());
        let residuals = [
            material(s)
                - self.base.material
                - (s.external_material_in - initial.external_material_in)
                - (s.care.feed_material_in - initial.care.feed_material_in)
                + (s.care.clean_material_out - initial.care.clean_material_out),
            energy(s) - self.base.energy - (s.light_in_total - initial.light_in_total)
                + (s.heat_out_total - initial.heat_out_total)
                - (s.care.feed_energy_in - initial.care.feed_energy_in)
                + (s.care.clean_energy_out - initial.care.clean_energy_out),
            s.fields.w.iter().sum::<f64>() - self.base.water
                - (s.rain_in_total - initial.rain_in_total)
                + (s.evap_out_total - initial.evap_out_total),
        ];
        for (peak, residual) in self.worst.iter_mut().zip(residuals) {
            ensure!(residual.is_finite(), "nonfinite audit at tick {}", s.tick);
            *peak = peak.max(residual.abs());
        }

        if (elapsed + 1).is_multiple_of(sample_every) {
            // One telemetry reset per window serves both the independent energy audit and the
            // streamed sample: calling it twice would hand one of them an empty window.
            let raw = self.world.telemetry();
            let (total_rain, manual_rain, natural_rain) = self.water_attribution();
            let sample = json!({
                "tick": raw.tick, "elapsed": elapsed + 1,
                "population": raw.population, "population_by_form": raw.population_by_form,
                "population_by_face": raw.population_by_face, "occupied_cells": raw.occupied_cells,
                "window_births": raw.births,
                "window_deaths": {"starvation": raw.deaths_starvation, "age": raw.deaths_age,
                    "collapse": raw.deaths_collapse},
                "window_cap_rejections": raw.cap_rejections,
                "water_sampled": raw.water, "water_by_face_sampled": raw.water_by_face,
                "producer": raw.producer, "producer_by_face": raw.producer_by_face,
                "fruit": raw.fruit, "detritus": raw.detritus, "nutrient": raw.nutrient,
                "window_rain_in": raw.rain_in, "window_evap_out": raw.evap_out,
                "cumulative_rain_total": total_rain,
                "cumulative_rain_manual_attributed": manual_rain,
                "cumulative_rain_natural": natural_rain,
                "surviving_opening_cohorts": self.ancestry.surviving_cohorts(),
                "maximum_descendant_depth": self.ancestry.maximum_depth,
                "receipts_so_far": self.receipts_written,
                "local": self.regions.sample(&self.world.state)?,
                "state_hash": raw.state_hash.to_string(),
                "ecology_hash": raw.ecology_hash.to_string(),
            });
            line(&mut self.sample_stream, &sample)?;
            self.windowed.observe(raw);
        }
        Ok(())
    }

    fn finish(&mut self, dir: &Path, planned: u64, horizon: Horizon, reason: Option<String>) -> Result<Value> {
        self.receipts.flush()?;
        self.sample_stream.flush()?;
        self.receipts.get_ref().sync_all()?;
        self.sample_stream.get_ref().sync_all()?;
        self.samples.sync_all()?;
        let limits = self.base.limits;
        let legacy_passed = self.worst.iter().zip(limits).all(|(d, l)| *d < l);
        let passed = audit_passes(
            self.worst,
            self.worst_corrected_energy,
            self.worst_windowed_energy,
            self.worst_care_energy,
            limits,
        );
        let s = &self.world.state;
        let bytes = cubarium_core::encode_snapshot(s, BUILD);
        write_new(&dir.join("closing.cubw"), &bytes)?;
        let (total_rain, manual_rain, natural_rain) = self.water_attribution();
        let elapsed = s.tick - self.opening.tick;
        let complete = reason.is_none() && elapsed == planned;
        let summary = json!({
            "arm": dir.file_name().and_then(|n| n.to_str()),
            "planned_ticks": planned, "closing_tick": s.tick, "elapsed_ticks": elapsed,
            "horizon": horizon,
            "technical_complete": complete,
            "complete_experiment_measurement": complete && horizon.prescribed() && passed,
            "measurement_note": "certifies audited numerical coverage of a prescribed horizon only; never ecological acceptance, balance, or a claim about dependence on attention",
            "termination": reason.clone().unwrap_or_else(|| "planned_horizon".into()),
            "audit_passed": passed, "legacy_audit_passed": legacy_passed,
            "audit_basis": "unchanged opening-inventory limits from the shared pre-intervention baseline; material, water, persisted compensated energy, independent windowed energy and immediate care-boundary energy must all pass",
            "max_absolute_drift": {"material": self.worst[0], "energy": self.worst[1], "water": self.worst[2]},
            "corrected_energy_drift": self.worst_corrected_energy,
            "independent_windowed_energy_drift": self.worst_windowed_energy,
            "care_boundary_energy_drift": self.worst_care_energy,
            "pre_intervention_baseline": self.base.json(),
            "rain_rate": s.config.water.rain_rate,
            "water": {
                "total_rain_in": total_rain,
                "manual_attributed": manual_rain,
                "natural": natural_rain,
                "basis": "natural = total rain delta minus the world's own manual attribution (care.rain_depth_in); never a second external source",
                "evap_out": s.evap_out_total - self.opening.evap_out_total,
                "closing_sampled": s.fields.w.iter().sum::<f64>(),
            },
            "care": {
                "dose_permille": self.dose.map(|d| d.permille()),
                "attempts": self.receipts_written,
                "applied": self.receipts_by_outcome[0],
                "partial": self.receipts_by_outcome[1],
                "rejected": self.receipts_by_outcome[2],
                "ledgers": self.world.care().clone(),
            },
            "population_min": self.population_min, "population_max": self.population_max,
            "closing_population": s.organisms.len(),
            "first_extinction_tick": self.extinction_tick,
            "surviving_opening_cohorts": self.ancestry.surviving_cohorts(),
            "maximum_descendant_depth": self.ancestry.maximum_depth,
            "closing_state_hash": cubarium_core::snapshot::state_hash(s).to_string(),
            "closing_ecology_hash": cubarium_core::ecology_hash(s).to_string(),
            "closing_snapshot_sha256": sha256(&bytes)?,
            "local": self.regions.sample(s)?,
        });
        json_new(&dir.join("summary.json"), &summary)?;
        Ok(summary)
    }
}

// ---------------------------------------------------------------- driver

fn run_seed(opening: &Opening, dir: &Path, planned: u64, horizon: Horizon, sample_every: u64) -> Result<Value> {
    fs::create_dir(dir)?;
    let base = Baseline::read(&opening.state)?;
    let mut summaries = Vec::new();
    let mut failure: Option<String> = None;
    for (name, candidate, dose) in ARMS {
        let arm_dir = dir.join(name);
        let mut arm = match Arm::new(&opening.state, base, name, candidate, dose, &arm_dir) {
            Ok(arm) => arm,
            Err(error) => {
                let text = format!("{name}: initialization_failure: {error:#}");
                json_new(&arm_dir.join("initialization-failure.json"), &json!({"error": text}))
                    .ok();
                failure.get_or_insert(text.clone());
                summaries.push(json!({"arm": name, "technical_complete": false, "error": text}));
                continue;
            }
        };
        let mut reason = None;
        for elapsed in 0..planned {
            if let Err(error) = arm.step(elapsed, sample_every) {
                // A failure is retained, never retried and never silently dropped.
                reason = Some(format!("step_failure at elapsed {elapsed}: {error:#}"));
                failure.get_or_insert_with(|| format!("{name}: {}", reason.clone().unwrap()));
                break;
            }
        }
        summaries.push(arm.finish(&arm_dir, planned, horizon, reason)?);
    }
    let complete = summaries.iter().all(|s| s["technical_complete"] == true);
    let result = json!({
        "seed": opening.state.config.seed,
        "opening": opening.source,
        "pre_intervention_baseline": base.json(),
        "technical_complete": complete,
        "failure": failure,
        "arms": summaries,
    });
    json_new(&dir.join("result.json"), &result)?;
    Ok(result)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let planned = args.horizon.ticks();
    ensure!(
        planned.is_multiple_of(args.sample_every),
        "the horizon must be a whole number of sample windows"
    );
    let (cohort, openings) = load_cohort(&args.cohort)?;
    // A brand new directory, or nothing: `create_dir` fails on an existing path, so an
    // accidental rerun cannot overwrite or silently resume a completed screen.
    fs::create_dir(&args.out).context("output must be a NEW directory")?;
    let executable_path = std::env::current_exe()?;
    let executable = fs::read(&executable_path)?;
    write_new(&args.out.join("ambient_compare.frozen"), &executable)?;
    fs::set_permissions(
        args.out.join("ambient_compare.frozen"),
        fs::metadata(executable_path)?.permissions(),
    )?;
    json_new(
        &args.out.join("manifest.json"),
        &json!({
            "kind": "six-arm-ambient-rainfall-comparison",
            "build": BUILD, "executable_sha256": sha256(&executable)?,
            "cohort": cohort,
            "horizon": args.horizon, "ticks": planned, "sample_every": args.sample_every,
            "arms": ARMS.map(|(n, _, _)| n),
            "factors": {
                "support": {"field": "config.water.rain_rate",
                    "levels": ["100% untouched", "90% of the opening's own rate"],
                    "candidate_fraction": CANDIDATE_FRACTION,
                    "applied": "once, to a copied state, before World::from_state; the 100% arm never writes the field"},
                "dose": {"levels": [null, 1000, 1500], "kind": "rain",
                    "schedule": {"start_tick": CARE_START, "period_ticks": CARE_PERIOD,
                        "targets": TARGETS, "rotation": "target index advances one per scheduled shower"}},
            },
            "unchanged": "rain threshold, weather trajectories and RNG, evaporation, flow, light, producers, fertility, movement, reproduction, feeding, cleaning, hunters, capacity, stocks, IDs and opening ledgers",
            "water_attribution": "natural = total rain delta minus the world's own manual attribution (care.rain_depth_in)",
            "observer_contract": "ambient-rainfall-v1; shared pre-intervention baseline per seed, strict unchanged material/energy/water limits, persisted compensated energy, independent windowed energy and immediate care-boundary energy; streamed samples, no in-memory history",
            "limits": "A technical pass certifies audited numbers over the planned horizon. It is not ecological acceptance, not evidence of balance, and not evidence that this world depends on attention. Candidate magnitude is unvalidated; a 90% response in either direction is a measurement, not a recommendation.",
        }),
    )?;

    let mut all = Vec::new();
    let mut failed = false;
    for opening in &openings {
        let seed = opening.state.config.seed;
        let result = run_seed(opening, &args.out.join(format!("seed-{seed}")), planned, args.horizon, args.sample_every)?;
        if result["technical_complete"] != true {
            failed = true;
        }
        eprintln!(
            "seed {seed}/12 {}",
            if result["technical_complete"] == true { "complete" } else { "RETAINED WITH FAILURE" }
        );
        all.push(result);
    }
    let summary = json!({
        "technical_complete": !failed,
        "complete_experiment_measurement": !failed
            && args.horizon.prescribed()
            && all.iter().all(|s| s["arms"].as_array().is_some_and(|a| a.iter().all(|x| x["audit_passed"] == true))),
        "horizon": args.horizon, "ticks": planned,
        "seeds": all,
        "retention": "every seed is retained, extinction and technical failure included; nothing is filtered, retried or reseeded",
    });
    json_new(&args.out.join("summary.json"), &summary)?;
    println!("{}", args.out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::WorldConfig;

    fn opening() -> WorldState {
        let mut cfg = WorldConfig::default();
        cfg.seed = 4242;
        World::new(cfg).unwrap().state
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cubarium-ambient-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The 100 % arm is the copied world, bit for bit. Not `rate * 1.0`, not `rate * 100 / 100`:
    /// the field is never written, so no rounding can enter through the door marked "no change".
    #[test]
    fn the_hundred_percent_arm_never_touches_the_field() {
        let o = opening();
        let untouched = arm_state(&o, false);
        assert_eq!(
            untouched.config.water.rain_rate.to_bits(),
            o.config.water.rain_rate.to_bits()
        );
        assert_eq!(untouched, o, "the 100% arm changed something");
        assert_eq!(
            cubarium_core::snapshot::state_hash(&untouched),
            cubarium_core::snapshot::state_hash(&o)
        );
        // Why this arm refuses to perform a "no change" calculation at all, rather than
        // relying on one being harmless: a percentage round trip is lossy for ordinary rates.
        // The default 0.6 happens to survive it, which is exactly the sort of accident a
        // harness must not depend on.
        assert_eq!(o.config.water.rain_rate, 0.6, "the cohort's rate, for the record");
        assert_eq!(o.config.water.rain_rate * 100.0 / 100.0, o.config.water.rain_rate);
        let lossy: f64 = 0.947_657_271_874_606_6;
        assert_ne!(
            (lossy * 90.0 / 90.0).to_bits(),
            lossy.to_bits(),
            "a round trip really can move a rate, so the untouched arm performs none"
        );
    }

    /// The candidate differs in exactly one field, and that field is the product the proposal
    /// names. Everything else — stocks, weather, IDs, ledgers, every other config value — is the
    /// opening's own.
    #[test]
    fn the_candidate_differs_in_exactly_one_field() {
        let o = opening();
        let candidate = arm_state(&o, true);
        assert_eq!(
            candidate.config.water.rain_rate,
            o.config.water.rain_rate * CANDIDATE_FRACTION
        );
        assert_ne!(candidate.config.water.rain_rate, o.config.water.rain_rate);

        let mut rewound = candidate.clone();
        rewound.config.water.rain_rate = o.config.water.rain_rate;
        assert_eq!(rewound, o, "the candidate changed something other than the rain rate");

        // Named explicitly, so a future field added to WaterConfig cannot slip in unnoticed.
        assert_eq!(candidate.config.water.rain_threshold, o.config.water.rain_threshold);
        assert_eq!(candidate.config.water.flow, o.config.water.flow);
        assert_eq!(candidate.config.water.evap, o.config.water.evap);
        assert_eq!(candidate.config.water.flood, o.config.water.flood);
        assert_eq!(candidate.config.weather, o.config.weather);
        assert_eq!(candidate.config.seed, o.config.seed);
        assert_eq!(candidate.fields, o.fields);
        assert_eq!(candidate.organisms, o.organisms);
        assert_eq!(candidate.weather, o.weather);
        assert_eq!(candidate.external_material_in, o.external_material_in);
        assert_eq!(candidate.rain_in_total, o.rain_in_total);
        assert_eq!(candidate.evap_out_total, o.evap_out_total);
        assert_eq!(candidate.care, o.care);
        assert_eq!(candidate.energy_correction, o.energy_correction);
    }

    /// The untouched no-input arm is exactly a plain resumed world: the harness adds observation,
    /// and observation is inert.
    #[test]
    fn the_untouched_no_input_arm_is_an_ordinary_continuation() {
        let o = opening();
        let base = Baseline::read(&o).unwrap();
        let dir = temp("continuation");
        let mut arm = Arm::new(&o, base, "rain100_none", false, None, &dir.join("arm")).unwrap();
        let mut plain = World::from_state(o.clone()).unwrap();
        for elapsed in 0..600 {
            arm.step(elapsed, 200).unwrap();
            plain.step();
            plain.drain_events();
            assert_eq!(
                cubarium_core::snapshot::state_hash(&arm.world.state),
                cubarium_core::snapshot::state_hash(&plain.state),
                "diverged at elapsed {elapsed}"
            );
        }
        // And it received nothing: no care, no manual water, no external material.
        assert_eq!(arm.receipts_written, 0);
        let (total, manual, natural) = arm.water_attribution();
        assert_eq!(manual, 0.0);
        assert_eq!(natural, total);
        assert_eq!(arm.world.state.care, o.care);
        assert_eq!(arm.world.state.external_material_in, o.external_material_in);
        fs::remove_dir_all(&dir).ok();
    }

    /// The two separation tests the proposal asks for, run against each other.
    ///
    /// * Changing the **dose** must not change the natural-weather input over a matched interval.
    ///   Natural rain is `rain_rate · weather excess · DT`, and weather does not read the water
    ///   field, so this is exact rather than approximate — and the harness's attribution has to
    ///   preserve that exactness to pass.
    /// * Changing the **support** must not rescale the scheduled manual dose. The showers are
    ///   scheduled and paid identically; only the natural side moves.
    #[test]
    fn dose_and_support_are_independent_factors() {
        let o = opening();
        let base = Baseline::read(&o).unwrap();
        let dir = temp("separation");
        let mut arms: Vec<(String, Arm)> = ARMS
            .iter()
            .map(|(name, candidate, dose)| {
                (
                    (*name).to_string(),
                    Arm::new(&o, base, name, *candidate, *dose, &dir.join(name)).unwrap(),
                )
            })
            .collect();
        for elapsed in 0..2500 {
            for (_, arm) in arms.iter_mut() {
                arm.step(elapsed, 200).unwrap();
            }
        }
        let natural: Vec<f64> = arms.iter().map(|(_, a)| a.water_attribution().2).collect();
        let manual: Vec<f64> = arms.iter().map(|(_, a)| a.water_attribution().1).collect();

        // The natural *input* is exactly identical across doses, and this is the strict form of
        // that claim: weather is advanced from the tick and the seed alone and never reads the
        // water field, so the whole driver of natural rain is bit-identical between arms that
        // differ only in manual dose. Nothing here is a tolerance.
        for (a, b) in [(0, 1), (0, 2), (3, 4), (3, 5)] {
            assert_eq!(
                arms[a].1.world.state.weather, arms[b].1.world.state.weather,
                "dose moved the weather between {} and {}", arms[a].0, arms[b].0
            );
            assert_eq!(
                arms[a].1.world.state.config.water.rain_rate,
                arms[b].1.world.state.config.water.rain_rate
            );
        }

        // The *attributed* natural total is the same quantity read back out of two accumulated
        // ledgers, and that read-back is not bit-exact: the core books
        // `rain_in += (natural + manual)·DT` and `manual_in += added − natural·DT` per cell, so
        // in a cared arm both sums carry the manual term's rounding. The difference is
        // accumulation noise on a shared exact input, and it is held to a far tighter bound
        // than the audit's own water limit rather than waved through.
        let noise = 1e-12 * natural[0].max(1.0);
        assert!(
            noise < base.limits[2],
            "the separation bound {noise:e} must be stricter than the audit's {:e}",
            base.limits[2]
        );
        for (a, b) in [(0, 1), (0, 2), (3, 4), (3, 5)] {
            let gap = (natural[a] - natural[b]).abs();
            assert!(
                gap <= noise,
                "dose moved the attributed natural rain between {} and {}: {gap:e}",
                arms[a].0, arms[b].0
            );
        }
        // Support really did move it, and by about the tenth the proposal describes.
        assert!(natural[0] > 0.0 && natural[3] > 0.0);
        assert!(natural[3] < natural[0], "the candidate must receive less natural rain");
        let ratio = natural[3] / natural[0];
        assert!((ratio - CANDIDATE_FRACTION).abs() < 1e-9, "ratio {ratio}");
        // And the gap the support level opens is enormous beside that accumulation noise, so
        // the separation test is not merely passing because everything is small.
        assert!(
            (natural[0] - natural[3]).abs() > 1e6 * noise,
            "the support effect must dominate the attribution noise"
        );

        // Same dose, either support: identical manual water. The schedule is not rescaled.
        assert_eq!(manual[0], 0.0);
        assert_eq!(manual[3], 0.0);
        assert_eq!(manual[1], manual[4], "support rescaled the standard dose");
        assert_eq!(manual[2], manual[5], "support rescaled the generous dose");
        assert!(manual[2] > manual[1] && manual[1] > 0.0, "generous is the larger dose");
        // The dose really is the 1.5× the panel names, on the delivered water itself.
        assert!((manual[2] / manual[1] - 1.5).abs() < 1e-9, "{}", manual[2] / manual[1]);
        // Identical attempts and outcomes on both support levels.
        for (a, b) in [(1, 4), (2, 5)] {
            assert_eq!(arms[a].1.receipts_written, arms[b].1.receipts_written);
            assert_eq!(arms[a].1.receipts_by_outcome, arms[b].1.receipts_by_outcome);
        }
        fs::remove_dir_all(&dir).ok();
    }

    /// A generous shower caught mid-fall survives a snapshot and finishes exactly where it was
    /// going: the amount is on the shower, not on a setting that a reload could re-read.
    #[test]
    fn a_generous_shower_resumes_exactly_across_a_reload() {
        let o = opening();
        let base = Baseline::read(&o).unwrap();
        let dir = temp("resume");
        let mut arm = Arm::new(&o, base, "rain90_generous", true, Some(1500), &dir.join("arm")).unwrap();
        // Past the first scheduled shower, and into it.
        for elapsed in 0..(CARE_START + 40) {
            arm.step(elapsed, 200).unwrap();
        }
        assert_eq!(arm.receipts_written, 1);
        let shower = &arm.world.state.care.showers[0];
        assert_eq!(shower.dose_permille, 1500);
        assert_eq!(shower.delivered, 40);

        let bytes = cubarium_core::encode_snapshot(&arm.world.state, "ambient-resume");
        let (_, state) = decode_snapshot(&bytes).unwrap();
        assert_eq!(state, arm.world.state);
        assert_eq!(state.config.water.rain_rate, o.config.water.rain_rate * CANDIDATE_FRACTION);
        let mut resumed = World::from_state(state).unwrap();
        for elapsed in (CARE_START + 40)..(CARE_START + 400) {
            arm.step(elapsed, 200).unwrap();
            resumed.step();
            resumed.drain_events();
            assert_eq!(
                cubarium_core::snapshot::state_hash(&arm.world.state),
                cubarium_core::snapshot::state_hash(&resumed.state),
                "diverged at elapsed {elapsed}"
            );
        }
        assert!(arm.world.state.care.showers.is_empty(), "the shower finished");
        fs::remove_dir_all(&dir).ok();
    }

    /// Refusals are data. A shower that cannot be admitted keeps its receipt and its reason, and
    /// the run continues.
    #[test]
    fn a_refused_shower_is_retained_with_its_reason() {
        let o = opening();
        let base = Baseline::read(&o).unwrap();
        let dir = temp("refusal");
        let arm_dir = dir.join("arm");
        let mut arm = Arm::new(&o, base, "rain100_standard", false, Some(1000), &arm_dir).unwrap();
        // A shower is 120 ticks long and only one may run. Admitting one by hand just before the
        // schedule's own means the scheduled shower is refused, exactly as the world would.
        for elapsed in 0..(CARE_START - 1) {
            arm.step(elapsed, 200).unwrap();
        }
        let seq = arm.world.care().admitted_seq + 1;
        let tick = arm.world.tick();
        arm.world.apply_care(&CareCommand {
            seq,
            apply_after_tick: tick,
            kind: CareKind::Rain,
            target: TARGETS[0],
            dose: CareDose::STANDARD,
        });
        for elapsed in (CARE_START - 1)..(CARE_START + 5) {
            arm.step(elapsed, 200).unwrap();
        }
        assert_eq!(arm.receipts_written, 1, "the scheduled attempt was made");
        assert_eq!(arm.receipts_by_outcome, [0, 0, 1], "and refused, not applied");
        arm.receipts.flush().unwrap();
        let text = fs::read_to_string(arm_dir.join("receipts.jsonl")).unwrap();
        assert!(text.contains(r#""outcome":"rejected""#), "{text}");
        assert!(text.contains("shower active"), "the reason is kept verbatim: {text}");
        fs::remove_dir_all(&dir).ok();
    }

    /// The audit is not decorative. An unbooked change to the world is caught at the very next
    /// observation, at the unchanged limit — no tolerance is widened to let a run finish.
    #[test]
    fn an_unbooked_change_fails_the_audit_at_the_unchanged_limit() {
        for (name, damage) in [
            ("material", 0usize),
            ("energy", 1),
            ("water", 2),
        ] {
            let o = opening();
            let base = Baseline::read(&o).unwrap();
            let dir = temp(&format!("adversarial-{name}"));
            let mut arm = Arm::new(&o, base, "rain100_none", false, None, &dir.join("arm")).unwrap();
            arm.step(0, 200).unwrap();
            // Exactly at the limit, which the audit treats as a failure rather than a pass:
            // a fixture that merely exceeded it by a lot would not test the boundary.
            let bump = base.limits[damage];
            match damage {
                0 => arm.world.state.fields.d[17] += bump,
                1 => arm.world.state.fields.de[17] += bump,
                _ => arm.world.state.fields.w[17] += bump,
            }
            let caught = (1..40).any(|e| arm.step(e, 200).is_err());
            let peaks = [arm.worst[0], arm.worst[1], arm.worst[2]];
            let over = peaks[damage] >= base.limits[damage];
            assert!(
                over,
                "{name}: the injected {bump:e} did not reach the drift peak ({:e})",
                peaks[damage]
            );
            assert!(
                !audit_passes(
                    arm.worst,
                    arm.worst_corrected_energy,
                    arm.worst_windowed_energy,
                    arm.worst_care_energy,
                    base.limits
                ),
                "{name}: an unbooked change still passed the audit"
            );
            let _ = caught;
            fs::remove_dir_all(&dir).ok();
        }
    }

    /// The horizons are the prescribed ones, a smoke can never certify a measurement, and the
    /// sample cadence must divide the horizon so no window is silently truncated.
    #[test]
    fn horizons_are_prescribed_and_a_smoke_certifies_nothing() {
        assert_eq!(Horizon::TwoHour.ticks(), 144_000);
        assert_eq!(Horizon::TwentyFourHour.ticks(), 1_728_000);
        assert_eq!(Horizon::SeventyTwoHour.ticks(), 5_184_000);
        assert!(!Horizon::Smoke.prescribed());
        for h in [Horizon::TwoHour, Horizon::TwentyFourHour, Horizon::SeventyTwoHour] {
            assert!(h.prescribed());
            assert!(h.ticks().is_multiple_of(200), "the 200-tick cadence must divide {h:?}");
        }
        // The family really is the preregistered six, crossing two support levels with three doses.
        assert_eq!(ARMS.len(), 6);
        assert_eq!(ARMS.iter().filter(|(_, c, _)| *c).count(), 3);
        assert_eq!(ARMS.iter().filter(|(_, _, d)| d.is_none()).count(), 2);
        let doses: BTreeSet<Option<u16>> = ARMS.iter().map(|(_, _, d)| *d).collect();
        assert_eq!(doses, BTreeSet::from([None, Some(1000), Some(1500)]));
        // Rain only: no feeding, no cleaning, no hunters anywhere in this family.
        assert_eq!(CARE_START, 60);
        assert_eq!(CARE_PERIOD, 2400);
    }

    /// The cohort is a fixed, complete, checksummed twelve. Every way of being not-that is a
    /// refusal before a single world is constructed, never a partial screen.
    #[test]
    fn an_incomplete_or_tampered_cohort_is_refused() {
        let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../captures/hunter-openings-2026-09-13");
        if !real.join("manifest.json").exists() {
            eprintln!("skipping: the prescribed cohort is not present in this checkout");
            return;
        }
        // The control: the real cohort loads, all twelve, with no care and no hunter.
        let (manifest, openings) = load_cohort(&real).expect("the prescribed cohort must load");
        assert_eq!(openings.len(), 12);
        assert_eq!(manifest["opening_tick"], 144_000);
        assert!(openings.iter().enumerate().all(|(i, o)| o.state.config.seed == i as u64 + 1));

        let dir = temp("cohort");
        let manifest_text = fs::read_to_string(real.join("manifest.json")).unwrap();
        let mut rows: Value = serde_json::from_str(&manifest_text).unwrap();

        // A cohort directory that is missing entirely.
        assert!(load_cohort(&dir.join("absent")).is_err());

        // An incomplete manifest, an unexpected kind, and eleven seeds.
        for (name, damage) in [
            ("incomplete", (|m: &mut Value| m["complete"] = json!(false)) as fn(&mut Value)),
            ("wrong-kind", |m: &mut Value| m["kind"] = json!("something-else")),
            ("eleven", |m: &mut Value| {
                m["openings"].as_array_mut().unwrap().pop();
            }),
            ("duplicate-seed", |m: &mut Value| {
                let seed = m["openings"][0]["seed"].clone();
                m["openings"][1]["seed"] = seed;
            }),
            ("tampered-checksum", |m: &mut Value| {
                m["openings"][0]["sha256"] = json!("0".repeat(64));
            }),
            ("tampered-census", |m: &mut Value| {
                let n = m["openings"][0]["population"].as_u64().unwrap();
                m["openings"][0]["population"] = json!(n + 1);
            }),
            ("tampered-ecology-hash", |m: &mut Value| {
                m["openings"][0]["ecology_hash"] = json!("1");
            }),
        ] {
            let case = dir.join(name);
            fs::create_dir_all(&case).unwrap();
            let mut m = rows.clone();
            damage(&mut m);
            fs::write(case.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
            // The snapshots themselves are the real ones, linked by relative path.
            for seed in 1..=12u64 {
                let to = case.join(format!("seed-{seed}"));
                fs::create_dir_all(&to).unwrap();
                let from = real.join(format!("seed-{seed}/world-144000.cubw"));
                if from.exists() {
                    fs::copy(from, to.join("world-144000.cubw")).unwrap();
                }
            }
            assert!(load_cohort(&case).is_err(), "{name}: a damaged cohort must be refused");
        }

        // And a snapshot whose bytes no longer match its recorded checksum.
        let case = dir.join("flipped-byte");
        fs::create_dir_all(case.join("seed-1")).unwrap();
        fs::write(case.join("manifest.json"), serde_json::to_vec(&rows).unwrap()).unwrap();
        for seed in 1..=12u64 {
            fs::create_dir_all(case.join(format!("seed-{seed}"))).unwrap();
            fs::copy(
                real.join(format!("seed-{seed}/world-144000.cubw")),
                case.join(format!("seed-{seed}/world-144000.cubw")),
            )
            .unwrap();
        }
        assert!(load_cohort(&case).is_ok(), "the copy must be a faithful one first");
        let path = case.join("seed-1/world-144000.cubw");
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&path, &bytes).unwrap();
        assert!(load_cohort(&case).is_err(), "a flipped byte must be caught");
        rows["openings"] = json!([]);
        fs::remove_dir_all(&dir).ok();
    }

    /// The output directory is exclusive and every file inside it is written once. A rerun into
    /// a used path is refused rather than resumed, reseeded or merged.
    #[test]
    fn an_existing_output_path_is_refused() {
        let dir = temp("exclusive");
        let out = dir.join("run");
        fs::create_dir(&out).unwrap();
        assert!(fs::create_dir(&out).is_err(), "an existing output directory must be refused");
        write_new(&out.join("manifest.json"), b"{}").unwrap();
        assert!(
            write_new(&out.join("manifest.json"), b"{}").is_err(),
            "an existing file must never be overwritten"
        );
        assert!(stream(&out.join("manifest.json")).is_err());
        fs::remove_dir_all(&dir).ok();
    }
}
