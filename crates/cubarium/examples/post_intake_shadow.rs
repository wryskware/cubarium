//! The bounded, non-intervening post-intake opportunity pilot
//! (`design/7_Research/root-post-intake-shadow-scope-2026-09-13.md`).
//!
//! **A measurement harness, not a feature and not an intervention.** Every world here runs
//! `QuietPolicy::Off`, carries no hunters, and takes exactly the trajectory it would have taken
//! without this file existing. The shadow reads; it never decides.
//!
//! The pilot root authorised, and nothing more:
//!
//! | | no care | one Standard Feed |
//! | --- | --- | --- |
//! | **seed 1** (Off) | `seed-1/off_nocare` | `seed-1/off_feed` |
//! | **seed 8** (Off) | `seed-8/off_nocare` | `seed-8/off_feed` |
//!
//! Four arms. Twelve thousand elapsed ticks from the retained tick-144000 openings, one Standard
//! Feed at elapsed 600, Front (32, 48), dose 1000. Root's full twelve-by-two ten-minute shadow is
//! **not** authorised by this file and is not run by it.
//!
//! ## What each arm proves before it reports a number
//!
//! 1. **Observer neutrality, exactly.** Each arm steps two worlds from the identical opening in
//!    lockstep — one with the shadow enabled, one without — and compares the whole persisted
//!    `WorldState`, the life record stream and the quiet record stream after *every* tick. A
//!    residual near zero is not the test; bit equality of the state and of the records is.
//! 2. **The retained Off continuation, exactly.** The instrumented arm's closing state is compared
//!    against the retained `quiet-ten-minute-provenance-2026-09-13` arm of the same seed and the
//!    same care, decoded from that run's own `closing.cubw` and compared as a whole state, not by
//!    trusting the summary's description of it. (The file's SHA-256 cannot match: a snapshot header
//!    carries the build that wrote it, and this build is not that one. The state inside it must.)
//! 3. **Conservation**, on the shared per-seed pre-intervention baseline, at the unchanged
//!    `1e-8 · max(opening inventory, 1)` limits.
//!
//! An arm that fails any of these is retained as the failure it is. Nothing is retried or dropped.
//!
//! ## What the output is, and what it is not
//!
//! `shadow-events.jsonl` carries every attempt — repeats included — with the joint per-ID
//! quantities the rule consumed: the episode that earned the credit, the quota, the
//! post-physiology reserve and energy, the budget actually tested, the ordinary mode, the form.
//! Windows carry the length the **baseline stayed compatible** with holding them.
//!
//! That is an admission-opportunity estimate. It is not a pause, not an upper bound on anything,
//! not actual missed food, and not a prediction that an intervention would produce these windows:
//! a real pause changes the patch, the neighbours' shares, the position and every later stock in
//! either direction. Whether any of it would be *readable* at native 64 px is a separate question
//! this run cannot answer.

use anyhow::{Context, Result, anyhow, ensure};
use clap::Parser;
use cubarium_core::organism::DeathCause;
use cubarium_core::post_intake::{
    EPISODE_EXPIRY_TICKS, QUOTA_SECONDS, REFRACTORY_TICKS, SHADOW_VERSION, ShadowEvent,
    ShadowReason, ShadowSample, WINDOW_DECISIONS,
};
use cubarium_core::quiet::{QuietEvent, QuietPolicy, QuietState, SAFETY_MARGIN_TICKS};
use cubarium_core::{
    CareCommand, CareDose, CareKind, CareTarget, DT, LifeEvent, OrganismId, World, WorldState,
    decode_snapshot,
};
use cubarium::meal_present::{MEAL_FADE_SECONDS, MEAL_SETTLE_SECONDS};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// The shared observer helpers, used unchanged. This pilot has no lineage question of its own, so
// the module's ancestry half is deliberately unused rather than copied or edited.
#[path = "shared/audit.rs"]
#[allow(dead_code)]
mod audit;

use audit::{AccurateSum, WindowAudit, audit_passes, energy, material};

const BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("CUBARIUM_GIT_HASH"));

/// The unchanged single input, identical to the retained quiet comparison's own.
const FEED_TARGET: CareTarget = CareTarget { face: 0, u: 32.0, v: 48.0 };
const FEED_ELAPSED: u64 = 600;
const FEED_DOSE_PERMILLE: u16 = 1000;

/// The fixed pilot: two seeds, two care levels, no policy factor. Not a parameter.
const PILOT_SEEDS: [u64; 2] = [1, 8];
const ARMS: [(&str, bool); 2] = [("off_nocare", false), ("off_feed", true)];
const PILOT_ELAPSED: u64 = 12_000;
const OPENING_TICK: u64 = 144_000;
/// The retained run's own census cadence, so the two histories are stepped identically.
const SAMPLE_EVERY: u64 = 200;

/// The source files whose bytes decide what this measurement means. Hashed into the manifest so a
/// reader can tell whether a later artifact came from the same rule.
const SOURCE_FILES: [&str; 5] = [
    "crates/cubarium-core/src/post_intake.rs",
    "crates/cubarium-core/src/world.rs",
    "crates/cubarium-core/src/quiet.rs",
    "crates/cubarium-core/src/controller.rs",
    "crates/cubarium/examples/post_intake_shadow.rs",
];

#[derive(Parser)]
struct Args {
    /// Completed `prepare-hunter-worlds` cohort directory (all twelve seeds).
    cohort: PathBuf,
    /// The retained `quiet-ten-minute-provenance-2026-09-13` directory, read only, for the exact
    /// Off-arm continuation comparison.
    retained: PathBuf,
    /// Brand-new directory; existing output is never overwritten or resumed.
    out: PathBuf,
    /// The repository root the source hashes are read from.
    #[arg(long, default_value = ".")]
    source_root: PathBuf,
}

// ---------------------------------------------------------------- small io

fn sha256(bytes: &[u8]) -> Result<String> {
    let mut child =
        Command::new("sha256sum").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
    child.stdin.take().context("checksum stdin")?.write_all(bytes)?;
    let out = child.wait_with_output()?;
    ensure!(out.status.success(), "sha256sum failed");
    let text = String::from_utf8(out.stdout)?;
    let hash = text.split_whitespace().next().context("missing checksum")?;
    ensure!(hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()), "invalid checksum");
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
    Ok(BufWriter::new(OpenOptions::new().write(true).create_new(true).open(path)?))
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

/// The prescribed openings, verified against the manifest's own checksums before anything is built
/// from them. The manifest's **raw bytes** come back with it: `serde_json` without `float_roundtrip`
/// can land one ULP from the correctly rounded value, so a re-serialized copy is a different number
/// and nothing re-encoded from a `Value` may stand in for the source.
fn load_cohort(dir: &Path) -> Result<(Vec<u8>, Value, Vec<Opening>)> {
    let raw = fs::read(dir.join("manifest.json"))?;
    let manifest: Value = serde_json::from_slice(&raw)?;
    ensure!(
        manifest["complete"] == true && manifest["kind"] == "pre-hunter-cohort-preparation",
        "a complete unfiltered preparation manifest is required"
    );
    ensure!(
        manifest["opening_tick"].as_u64() == Some(OPENING_TICK),
        "the cohort is not the retained tick-{OPENING_TICK} opening set"
    );
    let rows = manifest["openings"].as_array().context("opening rows")?;
    ensure!(rows.len() == 12, "the complete twelve-seed cohort is required");
    let mut openings = Vec::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        let seed = row["seed"].as_u64().context("seed")?;
        ensure!((1..=12).contains(&seed) && seen.insert(seed), "duplicate/unprescribed seed");
        if !PILOT_SEEDS.contains(&seed) {
            continue;
        }
        let bytes = fs::read(dir.join(format!("seed-{seed}/world-{OPENING_TICK}.cubw")))?;
        ensure!(
            sha256(&bytes)? == row["sha256"].as_str().context("snapshot SHA256")?,
            "seed{seed} checksum changed"
        );
        let (meta, state) = decode_snapshot(&bytes).map_err(|e| anyhow!("{e:?}"))?;
        ensure!(
            meta.schema == 9 && state.tick == OPENING_TICK && state.config.seed == seed,
            "seed{seed} is not its frozen pre-hunter two-hour opening"
        );
        ensure!(
            state.hunters == Default::default() && state.care == Default::default(),
            "opening contains hunter/care state"
        );
        ensure!(state.quiet == QuietState::default(), "opening already carries a quiet extension");
        ensure!(
            state.organisms.len() as u64 == row["population"].as_u64().context("population")?,
            "opening census mismatch"
        );
        ensure!(
            cubarium_core::ecology_hash(&state).to_string()
                == row["ecology_hash"].as_str().context("ecology hash")?,
            "opening ecology hash mismatch"
        );
        let mut source = row.clone();
        if let Some(row) = source.as_object_mut() {
            row.remove("telemetry");
            row.insert(
                "telemetry".into(),
                json!(
                    "in cohort-manifest.json, byte for byte; not re-serialized here because \
                     serde_json's default decimal parse can move a value by one ULP"
                ),
            );
        }
        openings.push(Opening { state, source });
    }
    ensure!(
        openings.len() == PILOT_SEEDS.len(),
        "the cohort does not carry every prescribed pilot seed"
    );
    openings.sort_by_key(|o| o.state.config.seed);
    Ok((raw, manifest, openings))
}

// ---------------------------------------------------------------- the retained Off reference

/// One retained Off arm of the frozen quiet comparison, read only. Its closing **state** is the
/// reference this pilot must continue to exactly.
struct Retained {
    state: WorldState,
    summary: Value,
    file_sha256: String,
    path: PathBuf,
}

fn load_retained(dir: &Path, seed: u64, arm: &str) -> Result<Retained> {
    let arm_dir = dir.join(format!("seed-{seed}")).join(arm);
    let summary: Value = serde_json::from_slice(&fs::read(arm_dir.join("summary.json"))?)?;
    ensure!(
        summary["arm"] == arm
            && summary["quiet_policy"] == QuietPolicy::Off.as_str()
            && summary["technical_complete"] == true
            && summary["audit_passed"] == true,
        "the retained {arm} reference for seed {seed} is not a completed Off measurement"
    );
    ensure!(
        summary["planned_ticks"].as_u64() == Some(PILOT_ELAPSED)
            && summary["elapsed_ticks"].as_u64() == Some(PILOT_ELAPSED)
            && summary["closing_tick"].as_u64() == Some(OPENING_TICK + PILOT_ELAPSED),
        "the retained reference does not cover this pilot's exact horizon"
    );
    let path = arm_dir.join("closing.cubw");
    let bytes = fs::read(&path)?;
    let file_sha256 = sha256(&bytes)?;
    // The summary's own receipt definition: the harness that wrote it hashed exactly these bytes.
    ensure!(
        summary["closing_snapshot_sha256"].as_str() == Some(file_sha256.as_str()),
        "the retained closing snapshot does not match its own recorded checksum"
    );
    let (_, state) = decode_snapshot(&bytes).map_err(|e| anyhow!("{e:?}"))?;
    ensure!(
        cubarium_core::snapshot::state_hash(&state).to_string()
            == summary["closing_state_hash"].as_str().context("retained state hash")?,
        "the retained closing state does not match its own recorded state hash"
    );
    Ok(Retained { state, summary, file_sha256, path })
}

// ---------------------------------------------------------------- records

fn sample_json(s: &ShadowSample) -> Value {
    json!({
        "form": s.form,
        "structure": s.structure, "structure_adult": s.structure_adult,
        "reserve": s.reserve, "energy": s.energy,
        "reserve_max": s.reserve_max, "energy_max": s.energy_max,
        "reserve_fraction": if s.reserve_max > 0.0 { Some(s.reserve / s.reserve_max) } else { None },
        "mode": format!("{:?}", s.mode),
        "gestating": s.gestating, "escrow_started_tick": s.escrow_started_tick,
        "hunter_member": s.hunter_member, "age_ticks": s.age_ticks,
        "quota": s.quota,
        "graze_rate": s.graze_rate, "scavenge_rate": s.scavenge_rate, "diet": s.diet,
        "budget": s.budget.map(|b| json!({
            "horizon_seconds": b.horizon_seconds, "per_second": b.per_second,
            "growth": b.growth, "oxidation": b.oxidation,
            "energy": b.energy, "material": b.material,
        })),
    })
}

/// One transient record in the harness's own flat shape, spelled with the labels the summaries use
/// rather than Rust's variant names — two spellings for one thing in one artifact set is a trap.
fn shadow_record(event: &ShadowEvent) -> Value {
    match event {
        ShadowEvent::Attempt {
            tick,
            id,
            admitted,
            reason,
            episode,
            sample,
            window_end_tick,
            refractory_until,
        } => json!({
            "kind": "attempt", "tick": tick, "id": id,
            "admitted": admitted, "reason": reason.map(ShadowReason::as_str),
            "episode": {
                "start_tick": episode.start_tick,
                "last_positive_tick": episode.last_positive_tick,
                "positive_ticks": episode.positive_ticks,
                "elapsed_ticks": episode.elapsed_ticks,
                "elapsed_seconds": episode.elapsed_ticks as f64 * DT,
                "assimilated": episode.assimilated,
                "credit": episode.credit,
            },
            "sample": sample_json(sample),
            "window_end_tick": window_end_tick,
            "refractory_until": refractory_until,
        }),
        ShadowEvent::Release {
            tick,
            id,
            window_start_tick,
            held_decisions,
            baseline_intake_material,
            baseline_intake_ticks,
            baseline_died_at_release,
            sample,
            refractory_until,
        } => json!({
            "kind": "release", "tick": tick, "id": id,
            "window_start_tick": window_start_tick,
            "compatible_decisions": held_decisions,
            "compatible_seconds": *held_decisions as f64 * DT,
            "baseline_intake_material": baseline_intake_material,
            "baseline_intake_ticks": baseline_intake_ticks,
            "baseline_died_at_release": baseline_died_at_release,
            "sample": sample_json(sample),
            "refractory_until": refractory_until,
        }),
        ShadowEvent::Abort {
            tick,
            id,
            window_start_tick,
            completed_decisions,
            reason,
            baseline_intake_material,
            baseline_intake_ticks,
            sample,
            refractory_until,
        } => json!({
            "kind": "abort", "tick": tick, "id": id,
            "window_start_tick": window_start_tick,
            "compatible_decisions": completed_decisions,
            "compatible_seconds": *completed_decisions as f64 * DT,
            "reason": reason.as_str(),
            "baseline_intake_material": baseline_intake_material,
            "baseline_intake_ticks": baseline_intake_ticks,
            "sample": sample.as_ref().map(sample_json),
            "refractory_until": refractory_until,
        }),
        ShadowEvent::Censor {
            tick,
            id,
            window_start_tick,
            completed_decisions,
            baseline_intake_material,
            baseline_intake_ticks,
        } => json!({
            "kind": "censor", "tick": tick, "id": id,
            "window_start_tick": window_start_tick,
            "compatible_decisions": completed_decisions,
            "compatible_seconds": *completed_decisions as f64 * DT,
            "baseline_intake_material": baseline_intake_material,
            "baseline_intake_ticks": baseline_intake_ticks,
            "censored": true,
        }),
    }
}

fn exposure_json(e: &cubarium_core::post_intake::Exposure) -> Value {
    let mut refusals = serde_json::Map::new();
    for (at, reason) in ShadowReason::ALL.iter().enumerate() {
        refusals.insert(reason.as_str().into(), json!(e.refusals[at]));
    }
    json!({
        "organism_ticks": e.organism_ticks,
        "adult_ticks": e.adult_ticks,
        "adult_unencumbered_ticks": e.adult_unencumbered_ticks,
        "eligible_ticks": e.eligible_ticks,
        "settlement_ticks": e.settlement_ticks,
        "assimilated": e.assimilated,
        "settlement_ticks_without_quota": e.settlement_ticks_without_quota,
        "episodes_expired": e.episodes_expired,
        "attempts": e.attempts,
        "admissions": e.admissions,
        "releases": e.releases,
        "aborts": e.aborts,
        "censored": e.censored,
        "compatible_window_decisions": e.completed_window_decisions,
        "baseline_intake_in_windows": e.baseline_intake_in_windows,
        "refusals": Value::Object(refusals),
    })
}

// ---------------------------------------------------------------- the lockstep identity proof

/// The one comparison that decides whether the shadow is an observer. Factored out so an
/// adversarial test can feed it two deliberately different worlds instead of two copies of one
/// decode that would agree however wrong they both were.
fn instrumentation_disagreement(
    observed: &WorldState,
    reference: &WorldState,
    observed_life: &[LifeEvent],
    reference_life: &[LifeEvent],
    observed_quiet: &[QuietEvent],
    reference_quiet: &[QuietEvent],
) -> Option<String> {
    let at = reference.tick;
    if observed != reference {
        return Some(format!(
            "tick {at}: the instrumented state differs from the uninstrumented one (state hashes \
             {} and {})",
            cubarium_core::snapshot::state_hash(observed),
            cubarium_core::snapshot::state_hash(reference)
        ));
    }
    if format!("{observed_life:?}") != format!("{reference_life:?}") {
        return Some(format!("tick {at}: the life records differ"));
    }
    if format!("{observed_quiet:?}") != format!("{reference_quiet:?}") {
        return Some(format!("tick {at}: the quiet records differ"));
    }
    None
}

// ---------------------------------------------------------------- one arm

#[derive(Clone, Copy)]
struct Baseline {
    material: f64,
    energy: f64,
    water: f64,
    limits: [f64; 3],
}

impl Baseline {
    fn read(s: &WorldState) -> Result<Self> {
        let (material, energy, water) = (material(s), energy(s), s.fields.w.iter().sum::<f64>());
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
            "limits": {"material": self.limits[0], "energy": self.limits[1],
                "water": self.limits[2]},
            "basis": "one shared pre-intervention inventory per seed, read from the untouched \
                      opening before any care; limits are a fixed 1e-8 fraction of it and never \
                      scale with cumulative input",
        })
    }
}

/// Per-ID sets, kept so repeats and distinct individuals can both be reported. Bounded by the
/// number of distinct organisms that ever reached the rule, which is bounded by the opening
/// population plus the births — **not** by the live capacity, unlike the core's own map.
#[derive(Default)]
struct Distinct {
    attempted: BTreeSet<OrganismId>,
    admitted: BTreeSet<OrganismId>,
    released: BTreeSet<OrganismId>,
    /// One entry per (organism, gestation), so a gestating adult's repeated refusals collapse to
    /// the gestations they really concern without being deleted from the event record.
    refused_gestations: BTreeSet<(OrganismId, u64)>,
    refused_by_reason: BTreeMap<&'static str, BTreeSet<OrganismId>>,
}

impl Distinct {
    fn observe(&mut self, event: &ShadowEvent) {
        match event {
            ShadowEvent::Attempt { id, admitted, reason, sample, .. } => {
                self.attempted.insert(*id);
                if *admitted {
                    self.admitted.insert(*id);
                }
                if let Some(reason) = reason {
                    self.refused_by_reason.entry(reason.as_str()).or_default().insert(*id);
                    if let Some(started) = sample.escrow_started_tick {
                        self.refused_gestations.insert((*id, started));
                    }
                }
            }
            ShadowEvent::Release { id, .. } => {
                self.released.insert(*id);
            }
            ShadowEvent::Abort { .. } | ShadowEvent::Censor { .. } => {}
        }
    }

    fn json(&self) -> Value {
        let by_reason: serde_json::Map<String, Value> = self
            .refused_by_reason
            .iter()
            .map(|(reason, ids)| ((*reason).to_owned(), json!(ids.len())))
            .collect();
        json!({
            "attempted_ids": self.attempted.len(),
            "admitted_ids": self.admitted.len(),
            "released_ids": self.released.len(),
            "refused_gestations": self.refused_gestations.len(),
            "refused_ids_by_reason": Value::Object(by_reason),
            "basis": "distinct full generational IDs, computed beside the event stream and never \
                      by deleting repeats from it. A repeat attempt by one gestating adult is a \
                      real attempt and stays in shadow-events.jsonl.",
        })
    }
}

struct Arm {
    /// The world carrying the shadow. Its trajectory is the measurement's.
    observed: World,
    /// The identical world without the shadow. Its only purpose is to disagree if the shadow is
    /// not an observer.
    reference: World,
    base: Baseline,
    opening: WorldState,
    feed: bool,
    fed_at: Option<u64>,
    receipts: Vec<Value>,
    receipt_energy_in: AccurateSum,
    receipt_energy_out: AccurateSum,
    opening_ledgers: cubarium_core::EnergyLedgers,
    windowed: WindowAudit,
    worst: [f64; 3],
    worst_corrected_energy: f64,
    worst_windowed_energy: f64,
    worst_care_energy: f64,
    /// The first tick at which the instrumented and uninstrumented worlds disagreed, if ever.
    instrumentation_failure: Option<String>,
    compared_ticks: u64,
    births: u64,
    deaths: [u64; 4],
    population_min: usize,
    population_max: usize,
    /// Boundaries at which at least one hypothetical window was open anywhere in the world.
    world_window_ticks: u64,
    peak_entries: usize,
    events_written: u64,
    distinct: Distinct,
    /// Every closed window's compatible length in decisions, for the distribution.
    window_lengths: Vec<u64>,
    event_stream: BufWriter<File>,
    census_stream: BufWriter<File>,
}

impl Arm {
    fn new(opening: &WorldState, base: Baseline, feed: bool, dir: &Path) -> Result<Self> {
        fs::create_dir(dir)?;
        // The Off arm never writes the policy field: the opening already carries the default, and
        // leaving it untouched is the point.
        let state = opening.clone();
        ensure!(
            state.quiet == opening.quiet && state.quiet.policy == QuietPolicy::Off,
            "an arm changed the quiet policy"
        );
        ensure!(&state == opening, "an arm changed the opening");
        let mut observed = World::from_state(state.clone()).map_err(|e| anyhow!(e))?;
        let reference = World::from_state(state.clone()).map_err(|e| anyhow!(e))?;
        observed.enable_post_intake_shadow().map_err(|e| anyhow!(e))?;
        ensure!(
            observed.state == reference.state,
            "enabling the shadow changed the world's persisted state"
        );
        let shadow = observed.post_intake().context("the shadow did not start")?;
        ensure!(
            shadow.entries() == 0 && shadow.capacity() == state.config.capacity.max_organisms as usize,
            "the shadow did not start empty and bounded by the live capacity"
        );
        json_new(
            &dir.join("opening.json"),
            &json!({
                "arm": dir.file_name().and_then(|n| n.to_str()),
                "quiet_policy": state.quiet.policy.as_str(),
                "care": if feed {
                    json!({"kind": "feed", "dose_permille": FEED_DOSE_PERMILLE,
                        "elapsed_tick": FEED_ELAPSED, "target": FEED_TARGET, "count": 1})
                } else {
                    Value::Null
                },
                "opening_tick": state.tick,
                "opening_population": state.organisms.len(),
                "state_hash": cubarium_core::snapshot::state_hash(&state).to_string(),
                "ecology_hash": cubarium_core::ecology_hash(&state).to_string(),
                "config_sha256": sha256(&serde_json::to_vec(&state.config)?)?,
                "config": state.config,
                "pre_intervention_baseline": base.json(),
                "shadow_bound": state.config.capacity.max_organisms,
            }),
        )?;
        Ok(Self {
            population_min: observed.population(),
            population_max: observed.population(),
            opening_ledgers: state.energy_ledgers(),
            observed,
            reference,
            base,
            opening: state,
            feed,
            fed_at: None,
            receipts: Vec::new(),
            receipt_energy_in: AccurateSum::default(),
            receipt_energy_out: AccurateSum::default(),
            windowed: WindowAudit::default(),
            worst: [0.0; 3],
            worst_corrected_energy: 0.0,
            worst_windowed_energy: 0.0,
            worst_care_energy: 0.0,
            instrumentation_failure: None,
            compared_ticks: 0,
            births: 0,
            deaths: [0; 4],
            world_window_ticks: 0,
            peak_entries: 0,
            events_written: 0,
            distinct: Distinct::default(),
            window_lengths: Vec::new(),
            event_stream: stream(&dir.join("shadow-events.jsonl"))?,
            census_stream: stream(&dir.join("census.jsonl"))?,
        })
    }

    fn step(&mut self, elapsed: u64) -> Result<()> {
        // The one controlled input, applied identically to both worlds at the identical boundary.
        if self.feed && elapsed == FEED_ELAPSED {
            let before_energy = energy(&self.observed.state);
            let seq =
                self.observed.care().admitted_seq.checked_add(1).context("care seq exhausted")?;
            let command = CareCommand {
                seq,
                apply_after_tick: self.observed.tick(),
                kind: CareKind::Feed,
                target: FEED_TARGET,
                dose: CareDose::new(FEED_DOSE_PERMILLE).map_err(|e| anyhow!(e))?,
            };
            let receipt = self.observed.apply_care(&command);
            let mirror = self.reference.apply_care(&command);
            ensure!(
                format!("{receipt:?}") == format!("{mirror:?}"),
                "the instrumented and uninstrumented care receipts differ"
            );
            let booked = receipt.outcome.applied().map_or(0.0, |q| q.energy_in - q.energy_out);
            if let Some(q) = receipt.outcome.applied() {
                self.receipt_energy_in.add(q.energy_in);
                self.receipt_energy_out.add(q.energy_out);
            }
            self.worst_care_energy = self
                .worst_care_energy
                .max((energy(&self.observed.state) - before_energy - booked).abs());
            self.fed_at = Some(self.observed.tick());
            self.receipts.push(json!({
                "elapsed": elapsed, "tick": self.observed.tick(), "kind": "feed",
                "dose_permille": FEED_DOSE_PERMILLE, "target": FEED_TARGET,
                "outcome": receipt.outcome.as_str(), "reason": receipt.outcome.reason(),
                "receipt": receipt,
            }));
        }

        let counters = self.observed.step();
        let (transient_light, transient_heat) = (counters.light_in, counters.heat_out);
        self.observed
            .check_invariants()
            .map_err(|e| anyhow!("tick {}: {e}", self.observed.tick()))?;
        let life = self.observed.drain_events();
        let quiet = self.observed.drain_quiet_events();

        // The uninstrumented world takes the same step, and the two are compared whole. This is
        // the neutrality proof: equality of the persisted state and of the records, not a small
        // residual.
        self.reference.step();
        let reference_life = self.reference.drain_events();
        let reference_quiet = self.reference.drain_quiet_events();
        self.compared_ticks += 1;
        if self.instrumentation_failure.is_none() {
            self.instrumentation_failure = instrumentation_disagreement(
                &self.observed.state,
                &self.reference.state,
                &life,
                &reference_life,
                &quiet,
                &reference_quiet,
            );
        }
        ensure!(quiet.is_empty(), "an Off world published a quiet record");

        for event in self.observed.drain_post_intake_events() {
            self.distinct.observe(&event);
            match &event {
                ShadowEvent::Release { held_decisions, .. } => {
                    self.window_lengths.push(*held_decisions)
                }
                ShadowEvent::Abort { completed_decisions, .. }
                | ShadowEvent::Censor { completed_decisions, .. } => {
                    self.window_lengths.push(*completed_decisions)
                }
                ShadowEvent::Attempt { .. } => {}
            }
            line(&mut self.event_stream, &shadow_record(&event))?;
            self.events_written += 1;
        }
        let shadow = self.observed.post_intake().context("the shadow vanished mid-run")?;
        ensure!(
            shadow.entries() <= shadow.capacity(),
            "the shadow's per-ID map exceeded the live capacity"
        );
        self.peak_entries = self.peak_entries.max(shadow.entries());
        let open_windows = shadow.open_windows();
        if open_windows > 0 {
            self.world_window_ticks += 1;
        }

        for event in &life {
            match event {
                LifeEvent::Birth { .. } => self.births += 1,
                LifeEvent::Death { cause, .. } => {
                    self.deaths[match cause {
                        DeathCause::Starvation => 0,
                        DeathCause::Age => 1,
                        DeathCause::Collapse => 2,
                        DeathCause::Predation => 3,
                    }] += 1
                }
            }
        }
        self.population_min = self.population_min.min(self.observed.population());
        self.population_max = self.population_max.max(self.observed.population());

        let s = &self.observed.state;
        let initial = &self.opening;
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

        if (elapsed + 1).is_multiple_of(SAMPLE_EVERY) {
            let raw = self.observed.telemetry();
            // Stepped identically on both worlds so the reference's transient counters follow the
            // same cadence; the value is discarded.
            let _ = self.reference.telemetry();
            let shadow = self.observed.post_intake().context("the shadow vanished mid-run")?;
            let c = shadow.counters();
            let census = json!({
                "tick": raw.tick, "elapsed": elapsed + 1,
                "population": raw.population, "population_by_form": raw.population_by_form,
                "window_births": raw.births,
                "window_deaths": {"starvation": raw.deaths_starvation, "age": raw.deaths_age,
                    "collapse": raw.deaths_collapse},
                "mode": {"resting": raw.mode_resting, "seeking": raw.mode_seeking,
                    "feeding": raw.mode_feeding},
                "escrows": raw.escrows,
                "producer": raw.producer, "fruit": raw.fruit, "detritus": raw.detritus,
                "nutrient": raw.nutrient, "water": raw.water,
                "organism_material": raw.organism_material,
                "organism_energy": raw.organism_energy,
                "window_light_in": raw.light_in, "window_heat_out": raw.heat_out,
                "state_hash": raw.state_hash.to_string(),
                "ecology_hash": raw.ecology_hash.to_string(),
                "shadow": {
                    "entries": shadow.entries(), "open_windows": shadow.open_windows(),
                    "peak_entries": c.peak_entries, "bound": shadow.capacity(),
                    "attempts": c.total.attempts, "admissions": c.total.admissions,
                    "releases": c.total.releases, "aborts": c.total.aborts,
                    "world_window_ticks": c.world_window_ticks,
                },
            });
            line(&mut self.census_stream, &census)?;
            self.windowed.observe(raw);
        }
        Ok(())
    }

    fn finish(
        &mut self,
        dir: &Path,
        retained: &Retained,
        reason: Option<String>,
    ) -> Result<Value> {
        // Every still-open window is censored where it stands, before anything is summarised.
        self.observed.censor_post_intake_shadow();
        for event in self.observed.drain_post_intake_events() {
            self.distinct.observe(&event);
            if let ShadowEvent::Censor { completed_decisions, .. } = &event {
                self.window_lengths.push(*completed_decisions);
            }
            line(&mut self.event_stream, &shadow_record(&event))?;
            self.events_written += 1;
        }
        for s in [&mut self.event_stream, &mut self.census_stream] {
            s.flush()?;
            s.get_ref().sync_all()?;
        }

        let s = &self.observed.state;
        let bytes = cubarium_core::encode_snapshot(s, BUILD);
        write_new(&dir.join("closing.cubw"), &bytes)?;
        let reference_bytes = cubarium_core::encode_snapshot(&self.reference.state, BUILD);
        write_new(&dir.join("closing-uninstrumented.cubw"), &reference_bytes)?;

        let elapsed = s.tick - self.opening.tick;
        let complete = reason.is_none() && elapsed == PILOT_ELAPSED;
        let limits = self.base.limits;
        let audits = audit_passes(
            self.worst,
            self.worst_corrected_energy,
            self.worst_windowed_energy,
            self.worst_care_energy,
            limits,
        );
        // Bit equality, both ways: the two closing snapshots and every compared tick.
        let closing_sha = sha256(&bytes)?;
        let uninstrumented_sha = sha256(&reference_bytes)?;
        let neutral = self.instrumentation_failure.is_none()
            && self.compared_ticks == PILOT_ELAPSED
            && closing_sha == uninstrumented_sha
            && self.observed.state == self.reference.state;

        // The retained Off continuation, as a whole decoded state. The file's SHA-256 deliberately
        // is not the test: a snapshot header carries the build that wrote it, and this build is not
        // the retained run's.
        let continuation_state_equal = self.observed.state == retained.state;
        let closing_state_hash = cubarium_core::snapshot::state_hash(s).to_string();
        let closing_ecology_hash = cubarium_core::ecology_hash(s).to_string();
        let continuation = json!({
            "source": retained.path,
            "retained_file_sha256": retained.file_sha256,
            "retained_state_hash": retained.summary["closing_state_hash"],
            "retained_ecology_hash": retained.summary["closing_ecology_hash"],
            "retained_closing_population": retained.summary["closing_population"],
            "retained_births": retained.summary["births"],
            "retained_deaths": retained.summary["deaths"],
            "whole_state_equal": continuation_state_equal,
            "state_hash_equal": retained.summary["closing_state_hash"] == json!(closing_state_hash),
            "ecology_hash_equal":
                retained.summary["closing_ecology_hash"] == json!(closing_ecology_hash),
            "population_equal":
                retained.summary["closing_population"].as_u64() == Some(s.organisms.len() as u64),
            "births_equal": retained.summary["births"].as_u64() == Some(self.births),
            "deaths_equal": retained.summary["deaths"] == json!({
                "starvation": self.deaths[0], "age": self.deaths[1],
                "collapse": self.deaths[2], "predation": self.deaths[3]}),
            "basis": "the retained arm's own closing.cubw, decoded and compared as a whole \
                      WorldState. The snapshot file's SHA-256 cannot match across builds because \
                      the header records the build that wrote it; the state inside must.",
        });
        let continuation_pass = continuation_state_equal
            && continuation["state_hash_equal"] == true
            && continuation["ecology_hash_equal"] == true
            && continuation["births_equal"] == true
            && continuation["deaths_equal"] == true;

        let shadow = self.observed.post_intake().context("the shadow vanished")?;
        let c = *shadow.counters();
        let by_form: Vec<Value> = (0..8)
            .map(|form| {
                let mut row = exposure_json(&c.by_form[form]);
                if let Some(map) = row.as_object_mut() {
                    map.insert("form".into(), json!(form));
                }
                row
            })
            .collect();
        let mut lengths = self.window_lengths.clone();
        lengths.sort_unstable();
        let window_seconds = |d: u64| d as f64 * DT;
        let presenter_tail = MEAL_SETTLE_SECONDS + MEAL_FADE_SECONDS;

        // Assembled in pieces rather than as one literal: a single deeply nested `json!` blows the
        // macro's recursion limit, and a reader is better served by named blocks anyway.
        let memory = json!({
            "bound": shadow.capacity(),
            "peak_entries": c.peak_entries.max(self.peak_entries),
            "entries_at_close": shadow.entries(),
            "bounded_refusals": c.bounded_refusals,
            "basis": "one entry per live full generational ID, dropped at the organism's removal \
                      and when credit, window and deadline are all absent; the map may never \
                      exceed the world's live organism capacity",
        });
        let windows = json!({
            "closed": lengths.len(),
            "compatible_decisions_total": c.total.completed_window_decisions,
            "min_decisions": lengths.first(),
            "max_decisions": lengths.last(),
            "median_decisions": lengths.get(lengths.len() / 2),
            "full_length_windows": lengths.iter().filter(|d| **d == WINDOW_DECISIONS).count(),
            "whole_world_compatible_window_ticks": self.world_window_ticks,
            "whole_world_compatible_window_fraction":
                self.world_window_ticks as f64 / PILOT_ELAPSED as f64,
            "meaning": "the number of decisions the freely feeding baseline stayed compatible with \
                        holding. Not rest, not a pause, not an upper bound on an intervention.",
        });
        let readability = json!({
            "full_window_seconds": window_seconds(WINDOW_DECISIONS),
            "presenter_tail_seconds": presenter_tail,
            "arithmetic_remainder_seconds": window_seconds(WINDOW_DECISIONS) - presenter_tail,
            "basis": "MEAL_SETTLE_SECONDS + MEAL_FADE_SECONDS from the current presenter \
                      constants, subtracted from a full window. This is arithmetic on presenter \
                      constants, reported conservatively as root asked; it is NOT an observed \
                      native-64 transition and establishes no readable quiet interval. The 1.5 s \
                      timer is unchanged and is not lengthened here.",
        });
        let shadow_json = json!({
            "events_written": self.events_written,
            "total": exposure_json(&c.total),
            "by_form": by_form,
            "distinct": self.distinct.json(),
            "memory": memory,
            "windows": windows,
            "readability": readability,
        });
        let instrumentation = json!({
            "compared_ticks": self.compared_ticks,
            "planned_ticks": PILOT_ELAPSED,
            "first_disagreement": self.instrumentation_failure,
            "closing_snapshot_sha256": closing_sha,
            "uninstrumented_closing_snapshot_sha256": uninstrumented_sha,
            "snapshots_identical": closing_sha == uninstrumented_sha,
            "basis": "two worlds from the identical opening, stepped in lockstep, the whole \
                      persisted WorldState and both record streams compared after every tick. Not \
                      a residual: equality.",
        });
        let gates = json!({
            "conservation_and_flow_audits": audits,
            "observer_is_exactly_neutral": neutral,
            "retained_off_continuation_identical": continuation_pass,
        });
        let limits = json!([
            "A hypothetical window is baseline-compatible window time, never a real pause, never \
             an upper bound, and never measured missed food.",
            "Baseline intake during a window is recorded separately and is not the food a pause \
             would have cost: a real pause changes patch, shares, position and every later stock \
             in either direction.",
            "Eligible exposure here excludes affordability, which is tested only where the rule \
             tests it: at an attempt and at each later baseline boundary.",
            "Two seeds and one care level each. Nothing here supports a per-seed or per-form \
             generalisation, and the usefulness screen is not evaluated.",
        ]);
        let passed = audits && neutral && continuation_pass;
        let summary = json!({
            "arm": dir.file_name().and_then(|n| n.to_str()),
            "kind": "post-intake-opportunity-shadow-arm",
            "shadow_version": SHADOW_VERSION,
            "planned_ticks": PILOT_ELAPSED, "closing_tick": s.tick, "elapsed_ticks": elapsed,
            "technical_complete": complete,
            "audit_passed": passed,
            "complete_measurement": complete && passed,
            "termination": reason.clone().unwrap_or_else(|| "planned_horizon".into()),
            "gates": gates,
            "instrumentation": instrumentation,
            "retained_off_continuation": continuation,
            "max_absolute_drift": {"material": self.worst[0], "energy": self.worst[1],
                "water": self.worst[2]},
            "corrected_energy_drift": self.worst_corrected_energy,
            "independent_windowed_energy_drift": self.worst_windowed_energy,
            "care_boundary_energy_drift": self.worst_care_energy,
            "pre_intervention_baseline": self.base.json(),
            "quiet_policy": s.quiet.policy.as_str(),
            "care": {
                "planned": self.feed, "applied_at_tick": self.fed_at,
                "receipts": self.receipts, "ledgers": self.observed.care().clone(),
            },
            "population_min": self.population_min, "population_max": self.population_max,
            "closing_population": s.organisms.len(),
            "births": self.births,
            "deaths": {"starvation": self.deaths[0], "age": self.deaths[1],
                "collapse": self.deaths[2], "predation": self.deaths[3]},
            "closing_state_hash": closing_state_hash,
            "closing_ecology_hash": closing_ecology_hash,
            "closing_snapshot_sha256": closing_sha,
            "shadow": shadow_json,
            "rule": rule_json(),
            "limits": limits,
        });
        json_new(&dir.join("summary.json"), &summary)?;
        Ok(summary)
    }
}

/// The fixed rule, written into every artifact so a reader never has to reconstruct it from the
/// numbers it produced.
fn rule_json() -> Value {
    json!({
        "name": "post_intake_opportunity_shadow_v1",
        "version": SHADOW_VERSION,
        "quota": "Q = QUOTA_SECONDS · eta_m · (I_g·graze_rate + I_f·graze_rate + I_s·scavenge_rate); \
                  I_g needs grazing and diet >= DIET_GATE, I_f needs grazing and diet >= FRUIT_DIET, \
                  I_s needs scavenging and diet <= 1 - DIET_GATE. A diet-permitted unconstrained \
                  rate ceiling, not an attainable meal rate.",
        "quota_seconds": QUOTA_SECONDS,
        "episode_expiry_ticks": EPISODE_EXPIRY_TICKS,
        "window_decisions": WINDOW_DECISIONS,
        "safety_margin_ticks": SAFETY_MARGIN_TICKS,
        "refractory_ticks": REFRACTORY_TICKS,
        "credit": "actual material credited to reserve by field settlement, accumulated within an \
                   episode and capped at Q; never raw food, a fed flag or a reserve difference",
        "expiry": "evaluated at a settlement boundary before that boundary's new settlement is \
                   added, and again in the commit sweep; the refractory deadline is untouched by it",
        "trigger": "a fresh positive settlement at this boundary carrying the episode to Q, with no \
                    window open and no refractory outstanding. Nothing fires from a timer.",
        "admission": "tested after the completed tick's own physiology, funding and death checks, \
                      with the unchanged Budget::of / Budget::affordable over the window's \
                      decisions plus one safety tick",
        "consumption": "every attempt consumes the credit, refusal included; the cooldown does not",
        "population": "ordinary adults, no escrow, alive, ordinary Seeking or Feeding mode, no \
                       hunter world",
    })
}

// ---------------------------------------------------------------- driver

fn finish_retained_arm(
    arm: &mut Arm,
    dir: &Path,
    retained: &Retained,
    reason: Option<String>,
) -> Value {
    match arm.finish(dir, retained, reason.clone()) {
        Ok(summary) => summary,
        Err(error) => json!({
            "arm": dir.file_name().and_then(|n| n.to_str()),
            "technical_complete": false, "audit_passed": false, "complete_measurement": false,
            "termination": "finalization_failure", "error": format!("{error:#}"),
            "step_failure": reason,
            "closing_tick": arm.observed.tick(),
            "observer_trust": "partial; no completed measurement is claimed",
        }),
    }
}

fn run_seed(opening: &Opening, retained_dir: &Path, dir: &Path) -> Result<Value> {
    fs::create_dir(dir)?;
    let seed = opening.state.config.seed;
    let base = Baseline::read(&opening.state)?;
    let mut summaries = Vec::new();
    let mut failure: Option<String> = None;
    for (name, feed) in ARMS {
        let arm_dir = dir.join(name);
        let retained = match load_retained(retained_dir, seed, name) {
            Ok(retained) => retained,
            Err(error) => {
                let text = format!("{name}: retained reference unusable: {error:#}");
                failure.get_or_insert(text.clone());
                summaries.push(json!({"arm": name, "technical_complete": false,
                    "audit_passed": false, "error": text}));
                continue;
            }
        };
        let mut arm = match Arm::new(&opening.state, base, feed, &arm_dir) {
            Ok(arm) => arm,
            Err(error) => {
                let text = format!("{name}: initialization_failure: {error:#}");
                json_new(&arm_dir.join("initialization-failure.json"), &json!({"error": text}))
                    .ok();
                failure.get_or_insert(text.clone());
                summaries.push(json!({"arm": name, "technical_complete": false,
                    "audit_passed": false, "error": text}));
                continue;
            }
        };
        let mut reason = None;
        for elapsed in 0..PILOT_ELAPSED {
            if let Err(error) = arm.step(elapsed) {
                reason = Some(format!("step_failure at elapsed {elapsed}: {error:#}"));
                failure.get_or_insert_with(|| format!("{name}: {}", reason.clone().unwrap()));
                break;
            }
        }
        let summary = finish_retained_arm(&mut arm, &arm_dir, &retained, reason);
        if summary["technical_complete"] != true || summary["audit_passed"] != true {
            failure.get_or_insert_with(|| format!("{name}: numerical or finalization failure"));
        }
        summaries.push(summary);
    }
    let complete = summaries.iter().all(|s| s["technical_complete"] == true);
    let result = json!({
        "seed": seed,
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
    let (cohort_bytes, cohort, openings) = load_cohort(&args.cohort)?;
    let retained_manifest = fs::read(args.retained.join("manifest.json"))?;

    // A brand new directory, or nothing.
    fs::create_dir(&args.out).context("output must be a NEW directory")?;
    let executable_path = std::env::current_exe()?;
    let executable = fs::read(&executable_path)?;
    write_new(&args.out.join("post_intake_shadow.frozen"), &executable)?;
    fs::set_permissions(
        args.out.join("post_intake_shadow.frozen"),
        fs::metadata(executable_path)?.permissions(),
    )?;
    // Both provenance manifests copied byte for byte, never re-encoded from a parsed `Value`.
    write_new(&args.out.join("cohort-manifest.json"), &cohort_bytes)?;
    write_new(&args.out.join("retained-quiet-manifest.json"), &retained_manifest)?;

    let mut sources = serde_json::Map::new();
    for name in SOURCE_FILES {
        let bytes = fs::read(args.source_root.join(name))
            .with_context(|| format!("source file {name} is not readable"))?;
        sources.insert(
            name.into(),
            json!({"sha256": sha256(&bytes)?, "bytes": bytes.len()}),
        );
    }

    json_new(
        &args.out.join("manifest.json"),
        &json!({
            "kind": "post-intake-opportunity-shadow-pilot",
            "build": BUILD,
            "executable_sha256": sha256(&executable)?,
            "source": {
                "root": args.source_root,
                "files": Value::Object(sources),
                "basis": "the actual bytes of the files that decide what this measurement means, \
                          hashed by this process at the moment it ran",
            },
            "cohort_manifest": {
                "source": args.cohort, "copy": "cohort-manifest.json",
                "sha256": sha256(&cohort_bytes)?, "bytes": cohort_bytes.len(),
                "basis": "the cohort's manifest as it is on disk, copied rather than re-encoded. \
                          serde_json without float_roundtrip can parse a decimal one ULP away from \
                          its correctly rounded value, so a re-serialized copy is a different \
                          number and no exact checker can accept it as the source",
            },
            "retained_quiet_manifest": {
                "source": args.retained, "copy": "retained-quiet-manifest.json",
                "sha256": sha256(&retained_manifest)?, "bytes": retained_manifest.len(),
                "basis": "the retained Off-arm run's own manifest bytes, copied for the same reason",
            },
            "cohort_summary": {
                "kind": cohort["kind"], "complete": cohort["complete"],
                "opening_tick": cohort["opening_tick"],
                "prescribed_seeds": cohort["prescribed_seeds"],
                "runner_sha256": cohort["runner_sha256"],
                "seeds_used": openings.iter().map(|o| o.source.clone()).collect::<Vec<_>>(),
                "basis": "the exact, integer and string provenance every check here uses. The \
                          float-bearing preparation telemetry is deliberately not reproduced: read \
                          cohort-manifest.json for it",
            },
            "pilot": {
                "seeds": PILOT_SEEDS,
                "arms": ARMS.map(|(n, _)| n),
                "elapsed_ticks": PILOT_ELAPSED,
                "opening_tick": OPENING_TICK,
                "sample_every": SAMPLE_EVERY,
                "quiet_policy": QuietPolicy::Off.as_str(),
                "hunters": false,
                "care": {"levels": [null, "one Standard Feed"],
                    "recipe": {"kind": "feed", "dose_permille": FEED_DOSE_PERMILLE,
                        "elapsed_tick": FEED_ELAPSED, "target": FEED_TARGET, "count": 1}},
                "authorised_scope": "root's four-arm pilot. The full twelve-by-two ten-minute \
                                     shadow is NOT authorised and is not run here.",
            },
            "rule": rule_json(),
            "unchanged": "every config value, ambient support, weather and RNG streams, \
                          physiology, reproduction eligibility and costs, stocks, IDs, opening \
                          ledgers and the Off quiet default; no hunters, no cleanup, no rain, no \
                          rolling parameter change",
            "observer_contract": "post-intake-opportunity-shadow-v1; a transient per-live-full-ID \
                                  map bounded by the world's organism capacity, fed only by the \
                                  actual to_reserve transfers the settlement stage applies and by \
                                  the completed tick's own physiology. It is absent from every \
                                  snapshot, hash, decision, stock, draw, transport and mode. Each \
                                  arm proves that by stepping an uninstrumented twin in lockstep \
                                  and comparing whole states and record streams every tick.",
            "limits": "An admission-opportunity estimate on two seeds. Not an active rest policy, \
                       not ecological acceptance, not a proven upper bound, not measured missed \
                       food, not evidence that any interval would read at native 64 px, and not \
                       permission to change a default or to run a longer horizon.",
        }),
    )?;

    let mut all = Vec::new();
    let mut failed = false;
    for opening in &openings {
        let seed = opening.state.config.seed;
        let result = match run_seed(opening, &args.retained, &args.out.join(format!("seed-{seed}")))
        {
            Ok(result) => result,
            Err(error) => json!({
                "seed": seed, "technical_complete": false, "audit_passed": false,
                "termination": "seed_output_or_initialization_failure",
                "error": format!("{error:#}"),
            }),
        };
        if result["technical_complete"] != true {
            failed = true;
        }
        eprintln!(
            "seed {seed} {}",
            if result["technical_complete"] == true { "complete" } else { "RETAINED WITH FAILURE" }
        );
        all.push(result);
    }
    let arms_passed = all.len() == PILOT_SEEDS.len()
        && all.iter().all(|s| {
            s["arms"].as_array().is_some_and(|arms| {
                arms.len() == ARMS.len()
                    && arms.iter().all(|a| a["complete_measurement"] == true)
            })
        });
    let summary = json!({
        "kind": "post-intake-opportunity-shadow-pilot",
        "technical_complete": !failed,
        "audit_passed": arms_passed,
        "complete_measurement": !failed && arms_passed,
        "seeds": all,
        "retention": "every arm is retained, failure included; nothing is filtered, retried or \
                      reseeded",
        "next_gate": "root reviews this four-arm pilot. No twelve-by-two shadow, no longer \
                      horizon, no parameter sweep and no active policy is authorised by it.",
    });
    json_new(&args.out.join("summary.json"), &summary)?;
    println!("{}", args.out.display());
    ensure!(arms_passed, "the pilot contains technical or numerical failures; see summary.json");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opening() -> WorldState {
        let mut cfg = cubarium_core::WorldConfig::default();
        cfg.founders.count = 12;
        World::new(cfg).unwrap().state
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("post-intake-shadow-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_pilot_is_the_four_arms_root_authorised_and_no_others() {
        assert_eq!(PILOT_SEEDS, [1, 8]);
        assert_eq!(ARMS.map(|(n, _)| n), ["off_nocare", "off_feed"]);
        assert_eq!(PILOT_SEEDS.len() * ARMS.len(), 4);
        assert_eq!(PILOT_ELAPSED, 12_000);
        assert_eq!(OPENING_TICK, 144_000);
        assert_eq!((FEED_ELAPSED, FEED_DOSE_PERMILLE), (600, 1000));
        assert_eq!((FEED_TARGET.face, FEED_TARGET.u, FEED_TARGET.v), (0, 32.0, 48.0));
        // Every arm is Off. There is no policy factor in this pilot at all.
        assert!(ARMS.iter().all(|_| QuietPolicy::Off == QuietPolicy::default()));
    }

    #[test]
    fn the_rules_constants_are_the_cores_own_and_not_restated_here() {
        assert_eq!(WINDOW_DECISIONS, 30);
        assert_eq!(REFRACTORY_TICKS, 600);
        assert_eq!(EPISODE_EXPIRY_TICKS, 20);
        assert_eq!(QUOTA_SECONDS, 2.0);
        assert_eq!(SAFETY_MARGIN_TICKS, 1);
        let rule = rule_json();
        assert_eq!(rule["window_decisions"], json!(WINDOW_DECISIONS));
        assert_eq!(rule["refractory_ticks"], json!(REFRACTORY_TICKS));
        assert_eq!(rule["episode_expiry_ticks"], json!(EPISODE_EXPIRY_TICKS));
    }

    #[test]
    fn enabling_the_shadow_changes_no_persisted_byte_and_no_trajectory() {
        let state = opening();
        let mut observed = World::from_state(state.clone()).unwrap();
        let mut reference = World::from_state(state.clone()).unwrap();
        observed.enable_post_intake_shadow().unwrap();
        assert_eq!(observed.state, reference.state, "enabling wrote into the state");
        for _ in 0..400 {
            observed.step();
            reference.step();
            let a = observed.drain_events();
            let b = reference.drain_events();
            let qa = observed.drain_quiet_events();
            let qb = reference.drain_quiet_events();
            assert_eq!(
                instrumentation_disagreement(
                    &observed.state,
                    &reference.state,
                    &a,
                    &b,
                    &qa,
                    &qb
                ),
                None
            );
            let _ = observed.drain_post_intake_events();
        }
        assert_eq!(
            cubarium_core::encode_snapshot(&observed.state, BUILD),
            cubarium_core::encode_snapshot(&reference.state, BUILD),
            "the instrumented world's snapshot differs byte for byte"
        );
    }

    #[test]
    fn the_neutrality_check_really_catches_a_difference() {
        let a = opening();
        let mut b = a.clone();
        b.tick += 1;
        assert!(instrumentation_disagreement(&a, &b, &[], &[], &[], &[]).is_some());
        // ... and a record-stream difference too, with the states equal.
        let life = [LifeEvent::Death {
            tick: 1,
            id: OrganismId { slot: 0, generation: 1 },
            age_ticks: 1,
            cause: DeathCause::Age,
            births: 0,
            genome: 0,
        }];
        assert!(instrumentation_disagreement(&a, &a, &life, &[], &[], &[]).is_some());
        assert_eq!(instrumentation_disagreement(&a, &a, &life, &life, &[], &[]), None);
    }

    #[test]
    fn the_shadow_refuses_a_hunter_world_and_an_enabled_quiet_policy() {
        let mut state = opening();
        state.quiet = QuietState::post_birth_pause_v1();
        let mut world = World::from_state(state).unwrap();
        assert!(world.enable_post_intake_shadow().is_err(), "an enabled policy was accepted");

        let mut world = World::from_state(opening()).unwrap();
        world.enable_post_intake_shadow().unwrap();
        assert!(world.enable_post_intake_shadow().is_err(), "a second enable was accepted");
    }

    #[test]
    fn a_shadow_record_keeps_the_labels_the_summaries_use() {
        let event = ShadowEvent::Abort {
            tick: 7,
            id: OrganismId { slot: 1, generation: 2 },
            window_start_tick: 2,
            completed_decisions: 5,
            reason: ShadowReason::UnaffordableRemaining,
            baseline_intake_material: 0.25,
            baseline_intake_ticks: 3,
            sample: None,
            refractory_until: Some(607),
        };
        let row = shadow_record(&event);
        assert_eq!(row["kind"], "abort");
        assert_eq!(row["reason"], "unaffordable_remaining");
        assert_eq!(row["compatible_decisions"], 5);
        assert_eq!(row["compatible_seconds"], 5.0 * DT);
        assert_eq!(row["baseline_intake_material"], 0.25);
    }

    #[test]
    fn a_per_form_table_always_carries_every_form_and_every_reason() {
        let e = cubarium_core::post_intake::Exposure::default();
        let row = exposure_json(&e);
        let refusals = row["refusals"].as_object().unwrap();
        assert_eq!(refusals.len(), ShadowReason::ALL.len());
        for reason in ShadowReason::ALL {
            assert_eq!(refusals[reason.as_str()], 0, "{}", reason.as_str());
        }
    }

    #[test]
    fn distinct_summaries_never_delete_a_repeat_from_the_record() {
        let id = OrganismId { slot: 3, generation: 1 };
        let sample = ShadowSample {
            form: 0,
            structure: 1.0,
            structure_adult: 1.0,
            reserve: 1.0,
            energy: 1.0,
            reserve_max: 2.0,
            energy_max: 2.0,
            mode: cubarium_core::organism::Mode::Feeding,
            gestating: true,
            escrow_started_tick: Some(40),
            hunter_member: false,
            age_ticks: 900,
            quota: Some(0.1),
            graze_rate: 0.03,
            scavenge_rate: 0.01,
            diet: 0.6,
            budget: None,
        };
        let episode = cubarium_core::post_intake::EpisodeRecord {
            start_tick: 1,
            last_positive_tick: 2,
            positive_ticks: 2,
            elapsed_ticks: 1,
            assimilated: 0.2,
            credit: 0.1,
        };
        let mut distinct = Distinct::default();
        for tick in [100, 700, 1300] {
            distinct.observe(&ShadowEvent::Attempt {
                tick,
                id,
                admitted: false,
                reason: Some(ShadowReason::Gestating),
                episode,
                sample,
                window_end_tick: None,
                refractory_until: Some(tick + REFRACTORY_TICKS),
            });
        }
        let row = distinct.json();
        assert_eq!(row["attempted_ids"], 1, "one individual");
        assert_eq!(row["refused_gestations"], 1, "one gestation, three real attempts");
        assert_eq!(row["refused_ids_by_reason"]["gestating"], 1);
    }

    #[test]
    fn an_existing_output_path_is_refused() {
        let dir = temp("existing");
        fs::create_dir_all(&dir).unwrap();
        assert!(fs::create_dir(&dir).is_err(), "an existing output directory was accepted");
        assert!(write_new(&dir.join("x"), b"a").is_ok());
        assert!(write_new(&dir.join("x"), b"b").is_err(), "an existing file was overwritten");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_retained_reference_that_is_not_a_completed_off_measurement_is_refused() {
        let dir = temp("retained");
        let arm = dir.join("seed-1").join("off_nocare");
        fs::create_dir_all(&arm).unwrap();
        let good = json!({
            "arm": "off_nocare", "quiet_policy": "off",
            "technical_complete": true, "audit_passed": true,
            "planned_ticks": PILOT_ELAPSED, "elapsed_ticks": PILOT_ELAPSED,
            "closing_tick": OPENING_TICK + PILOT_ELAPSED,
            "closing_snapshot_sha256": "0".repeat(64),
        });
        type Damage = fn(&mut Value);
        let damages: [(&str, Damage); 4] = [
            ("policy", |v| v["quiet_policy"] = json!("post_birth_pause_v1")),
            ("incomplete", |v| v["technical_complete"] = json!(false)),
            ("audit", |v| v["audit_passed"] = json!(false)),
            ("horizon", |v| v["elapsed_ticks"] = json!(2400)),
        ];
        for (name, damage) in damages {
            let mut summary = good.clone();
            damage(&mut summary);
            let path = arm.join("summary.json");
            fs::write(&path, serde_json::to_vec(&summary).unwrap()).unwrap();
            assert!(
                load_retained(&dir, 1, "off_nocare").is_err(),
                "a {name}-damaged retained reference was accepted"
            );
        }
        // Even undamaged, a summary whose closing snapshot is missing cannot stand in for one.
        fs::write(arm.join("summary.json"), serde_json::to_vec(&good).unwrap()).unwrap();
        assert!(load_retained(&dir, 1, "off_nocare").is_err(), "a missing snapshot was accepted");
        fs::remove_dir_all(&dir).ok();
    }
}
