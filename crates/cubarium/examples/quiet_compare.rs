//! Ordinary quiet: the preregistered twelve-seed, four-arm comparison of
//! `design/7_Research/astra-ordinary-quiet-experiment-proposal-2026-09-13.md`.
//!
//! **An experiment harness, not a feature.** It chooses a snapshotted policy in a copied world's
//! initializer and changes nothing else: no config, no physiology, no ambient support, no
//! hunters, no live state, no viewer, no art. The autonomous default stays Off.
//!
//! The family, per seed:
//!
//! | | no care | one Standard Feed |
//! | --- | --- | --- |
//! | **Off** (reference) | `off_nocare` | `off_feed` |
//! | **candidate** `post_birth_pause_v1` | `candidate_nocare` | `candidate_feed` |
//!
//! The care recipe is exactly one Standard Feed at elapsed tick 600, Front (32, 48) — the quiet
//! diagnosis's own input — and nothing after it. That is a controlled diagnostic exposure, **not**
//! a care obligation. Ambient support is identical in all four arms.
//!
//! The rest classification is the part that had to be built rather than reused: the frozen quiet
//! diagnostic assumed every decision came from the ordinary controller, which is exactly what the
//! candidate breaks. `quiet_compare/bouts.rs` classifies each completed interval from real core
//! records and the pause set captured *before* the decision, and reconciles every published
//! record against the world that published it.
//!
//! A technical pass certifies that the numbers are trustworthy. It is **not** ecological
//! acceptance, not evidence of adequate quiet opportunity, and not permission to change a
//! default. Forty ticks remains the proposal's unvalidated candidate duration.

use anyhow::{Context, Result, anyhow, ensure};
use clap::{Parser, ValueEnum};
use cubarium_core::quiet::{QuietEvent, QuietPolicy, QuietState};
use cubarium_core::organism::DeathCause;
use cubarium_core::{
    CareCommand, CareDose, CareKind, CareTarget, LifeEvent, OrganismId, World, WorldState,
    decode_snapshot,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[path = "shared/audit.rs"]
mod audit;
#[path = "quiet_compare/bouts.rs"]
mod bouts;

use audit::{AccurateSum, Ancestry, WindowAudit, audit_passes, energy, material};

const BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("CUBARIUM_GIT_HASH"));

/// The quiet diagnosis's own single input: one Standard Feed, here, once.
const FEED_TARGET: CareTarget = CareTarget { face: 0, u: 32.0, v: 48.0 };
const FEED_ELAPSED: u64 = 600;
const FEED_DOSE_PERMILLE: u16 = 1000;

/// The restart proof compares this many ticks, and falls back to this elapsed boundary when the
/// arm has no pause to be interrupted in the first place.
const RESUME_WINDOW: u64 = 60;
const RESUME_FALLBACK_ELAPSED: u64 = 400;

/// `(name, candidate policy, one Standard Feed)`. Four arms, fixed, in this order.
const ARMS: [(&str, bool, bool); 4] = [
    ("off_nocare", false, false),
    ("candidate_nocare", true, false),
    ("off_feed", false, true),
    ("candidate_feed", true, true),
];

/// The prescribed horizons, by name. Nothing between them is tuned, and none is relabelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
enum Horizon {
    /// A technical smoke. Certifies nothing, at any level.
    Smoke,
    TenMinute,
    TwoHour,
    TwentyFourHour,
    SeventyTwoHour,
}

impl Horizon {
    fn ticks(self) -> u64 {
        match self {
            Horizon::Smoke => 2400,
            Horizon::TenMinute => 12_000,
            Horizon::TwoHour => 144_000,
            Horizon::TwentyFourHour => 1_728_000,
            Horizon::SeventyTwoHour => 5_184_000,
        }
    }

    /// Only a full prescribed horizon can certify a measurement; a smoke never can.
    fn prescribed(self) -> bool {
        !matches!(self, Horizon::Smoke)
    }
}

#[derive(Parser)]
struct Args {
    /// Completed prepare-hunter-worlds cohort directory (all seeds 1–12).
    #[arg(required_unless_present = "inspect")]
    cohort: Option<PathBuf>,
    /// Brand-new directory; existing output is never overwritten or resumed.
    #[arg(required_unless_present = "inspect")]
    out: Option<PathBuf>,
    /// The prescribed horizon to run.
    #[arg(long, value_enum, default_value_t = Horizon::TenMinute)]
    horizon: Horizon,
    /// Census cadence, and the independent energy window with it.
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u64).range(1..=12000))]
    sample_every: u64,
    /// Read-only: decode one `.cubw` and print what it actually contains, as JSON, on stdout.
    /// Nothing is stepped, nothing is written, and no world is constructed.
    #[arg(long, conflicts_with_all = ["cohort", "out", "horizon", "sample_every"])]
    inspect: Option<PathBuf>,
}

fn sha256(bytes: &[u8]) -> Result<String> {
    // The host's standard checksum utility, with no shell interpolation and no added dependency.
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

/// The twelve prescribed mature openings, verified before anything is constructed from them: the
/// file's own bytes against the manifest's checksum, then schema, tick, seed, census and ecology
/// hash, and that it carries no care and no hunter.
///
/// The originals are read-only inputs. Nothing here writes into the cohort directory.
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
        // The archive's own paths are provenance. Resolve the fixed cohort layout.
        let bytes = fs::read(dir.join(format!("seed-{seed}/world-144000.cubw")))?;
        ensure!(
            sha256(&bytes)? == row["sha256"].as_str().context("snapshot SHA256")?,
            "seed{seed} checksum changed"
        );
        let (meta, state) = decode_snapshot(&bytes)?;
        validate_opening(meta.schema, &state, seed, row)?;
        openings.push(Opening { state, source: row.clone() });
    }
    openings.sort_by_key(|o| o.state.config.seed);
    Ok((manifest, openings))
}

/// Everything an opening must be before a world is built from it, on the decoded state itself
/// rather than on the manifest's description of it.
fn validate_opening(schema: u32, state: &WorldState, seed: u64, row: &Value) -> Result<()> {
    ensure!(
        schema == 9 && state.tick == 144_000 && state.config.seed == seed,
        "seed{seed} is not its frozen pre-hunter two-hour opening"
    );
    ensure!(
        state.hunters == Default::default() && state.care == Default::default(),
        "opening contains hunter/care state"
    );
    ensure!(
        state.quiet == QuietState::default(),
        "opening already carries an ordinary quiet extension"
    );
    ensure!(
        state.organisms.len() as u64 == row["population"].as_u64().context("population")?,
        "opening census mismatch"
    );
    ensure!(
        cubarium_core::ecology_hash(state).to_string()
            == row["ecology_hash"].as_str().context("ecology hash")?,
        "opening ecology hash mismatch"
    );
    Ok(())
}

/// One organism's opening identity, in the harness's own flat shape: who it is, what form it
/// wears, how old it is and what it is carrying. Recruitment and loss are claims about
/// individuals, and a surviving-cohort count cannot support either.
fn organism_row(id: OrganismId, o: &cubarium_core::organism::Organism, now: u64) -> Value {
    json!({
        "id": id,
        "parent": o.parent,
        "form": o.phenotype.form,
        "genome_digest": o.genome.digest(),
        "origin": format!("{:?}", o.origin),
        "born_tick": o.born_tick,
        "age_ticks": o.age_ticks(now),
        "births": o.births,
        "mode": format!("{:?}", o.mode),
        "structure": o.structure,
        "reserve": o.reserve,
        "energy": o.energy,
        "material": o.material(),
        "gestating": o.escrow.is_some(),
        "face": o.pos.face.index(),
        "u": o.pos.u,
        "v": o.pos.v,
    })
}

/// **Read-only.** Decode one snapshot and report what it actually contains — header, identity,
/// semantic inventories, ledgers, care, quiet — without constructing a `World`, stepping
/// anything, drawing any RNG, or writing a byte.
///
/// This exists so an external checker can derive an arm's audit limits and its opening identities
/// from the snapshot itself rather than from the arm's own description of it. A JSON reader
/// parsing a header proves the bytes are a snapshot; it cannot say what is inside the payload.
fn inspect(path: &Path) -> Result<Value> {
    let bytes = fs::read(path)?;
    let (meta, s) = decode_snapshot(&bytes).map_err(|e| anyhow!("{e:?}"))?;
    let mut population_by_form = [0u64; 8];
    let mut population_by_face = [0u64; 6];
    let mut escrows = 0u64;
    let (mut organism_material, mut organism_energy) = (0.0, 0.0);
    let reserve_density = s.config.organism.reserve_energy_density;
    for (_, o) in s.organisms.iter() {
        population_by_form[(o.phenotype.form as usize).min(7)] += 1;
        population_by_face[o.pos.face.index()] += 1;
        escrows += u64::from(o.escrow.is_some());
        organism_material += o.material();
        organism_energy += o.energy
            + reserve_density * o.reserve
            + o.escrow
                .as_ref()
                .map_or(0.0, |e| e.energy + reserve_density * (e.structure + e.reserve));
    }
    let (material_total, energy_total) = (material(&s), energy(&s));
    let water_total = s.fields.w.iter().sum::<f64>();
    Ok(json!({
        "kind": "quiet-compare-snapshot-inspection",
        "inspector_build": BUILD,
        "path": path,
        "bytes": bytes.len(),
        "sha256": sha256(&bytes)?,
        "header": {
            "schema": meta.schema, "build_id": meta.build_id,
            "payload_len": meta.payload_len, "crc32": meta.crc32,
            "fixed_header_bytes": cubarium_core::snapshot::HEADER_FIXED_BYTES,
            "current_schema": cubarium_core::snapshot::SCHEMA_VERSION,
        },
        "tick": s.tick,
        "seed": s.config.seed,
        "config_sha256": sha256(&serde_json::to_vec(&s.config)?)?,
        "state_hash": cubarium_core::snapshot::state_hash(&s).to_string(),
        "ecology_hash": cubarium_core::ecology_hash(&s).to_string(),
        "population": s.organisms.len(),
        "population_by_form": population_by_form,
        "population_by_face": population_by_face,
        "gestating": escrows,
        "inventories": {
            "material": material_total,
            "energy": energy_total,
            "water": water_total,
            "organism_material": organism_material,
            "organism_energy": organism_energy,
            "producer": s.fields.p.iter().sum::<f64>(),
            "fruit": s.fields.f.iter().sum::<f64>(),
            "detritus": s.fields.d.iter().sum::<f64>(),
            "detritus_energy": s.fields.de.iter().sum::<f64>(),
            "nutrient": s.fields.n.iter().sum::<f64>(),
        },
        "audit_limits": {
            "material": 1e-8 * material_total.max(1.0),
            "energy": 1e-8 * energy_total.max(1.0),
            "water": 1e-8 * water_total.max(1.0),
            "rule": "1e-8 * max(opening inventory, 1.0), the unchanged paired-experiment rule",
        },
        "ledgers": {
            "external_material_in": s.external_material_in,
            "light_in_total": s.light_in_total,
            "heat_out_total": s.heat_out_total,
            "rain_in_total": s.rain_in_total,
            "evap_out_total": s.evap_out_total,
            "births_total": s.births_total,
        },
        "care": s.care,
        "quiet": {
            "version": s.quiet.version,
            "policy": s.quiet.policy.as_str(),
            "open_pauses": s.quiet.pauses.len(),
            "pauses": s.quiet.pauses,
        },
        "hunters_present": s.hunters != Default::default(),
        "scope": "a read-only decode of one file: no World is constructed, no tick is taken, no RNG is drawn and nothing is written. Inventories are the semantic totals the audit itself uses, not a header parse.",
    }))
}

/// One transient record, in the harness's own flat shape.
///
/// Deliberately not `serde_json::to_value(event)`. The core enum's derived encoding nests the
/// payload under a CamelCase variant name and spells a reason `"Unaffordable"`, while every
/// summary in this run spells the same reason `"unaffordable"` — the label
/// `QuietReason::as_str` gives it. Two spellings for one thing in one artifact set is a trap for
/// whoever reads it later, so the stream is written with the labels the summaries use and a
/// `kind` field of its own rather than inheriting Rust's variant names.
fn quiet_record(event: &QuietEvent) -> Value {
    match *event {
        QuietEvent::Begin { tick, parent, child, end_tick, underlying } => json!({
            "kind": "begin", "tick": tick, "parent": parent, "child": child,
            "end_tick": end_tick, "underlying": format!("{underlying:?}")
        }),
        QuietEvent::Refuse { tick, parent, child, reason } => json!({
            "kind": "refuse", "tick": tick, "parent": parent, "child": child,
            "reason": reason.as_str()
        }),
        QuietEvent::End { tick, parent, child, completed_ticks, underlying } => json!({
            "kind": "end", "tick": tick, "parent": parent, "child": child,
            "completed_ticks": completed_ticks, "underlying": format!("{underlying:?}")
        }),
        QuietEvent::Abort { tick, parent, child, completed_ticks, reason } => json!({
            "kind": "abort", "tick": tick, "parent": parent, "child": child,
            "completed_ticks": completed_ticks, "reason": reason.as_str()
        }),
    }
}

// ---------------------------------------------------------------- one arm

/// The common PRE-intervention inventory, read once per seed from the untouched opening and
/// shared by all four arms. Not a per-arm rebuild and never reset mid-run: rebuilding a `World`
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
            "limits": {"material": self.limits[0], "energy": self.limits[1], "water": self.limits[2]},
            "basis": "one shared pre-intervention inventory per seed, read from the untouched opening before any policy choice or care; limits are a fixed 1e-8 fraction of it and never scale with cumulative input",
        })
    }
}

/// Apply the one choice, or deliberately not apply it.
///
/// The Off arm never writes the field: `QuietState::default()` is what the opening already
/// carries, and leaving it alone is the point.
fn arm_state(opening: &WorldState, candidate: bool) -> WorldState {
    let mut state = opening.clone();
    if candidate {
        state.quiet = QuietState::post_birth_pause_v1();
    }
    state
}

/// A decoded copy of an arm, stepped **along the arm's own timeline** rather than in place of it.
///
/// The primary world is the uninterrupted one: it takes exactly the steps the experiment
/// prescribes, draws exactly the RNG it would have drawn, and is never rebuilt from its own bytes
/// to serve as its own oracle. The shadow is built once from a snapshot taken at `start_tick`,
/// receives the identical prescribed care if the window crosses it, and is compared against the
/// primary tick for tick. So the proof is uninterrupted-against-restarted, not restarted-against-
/// restarted: two copies decoded from the same bytes would agree even if the bytes had lost a
/// pause.
struct Shadow {
    world: World,
    /// `mid_pause` only when a pause really was open at `start_tick`; `fixed_boundary` otherwise,
    /// and it never claims to have interrupted a pause it did not.
    trigger: &'static str,
    start_tick: u64,
    window: u64,
    compared: u64,
    pauses_carried: usize,
    failure: Option<String>,
}

impl Shadow {
    fn complete(&self) -> bool {
        self.failure.is_none() && self.compared == self.window
    }

    fn json(&self, finish_tick: u64) -> Value {
        json!({
            "trigger": self.trigger,
            "start_tick": self.start_tick,
            "finish_tick": finish_tick,
            "planned_window_ticks": self.window,
            "compared_ticks": self.compared,
            "complete": self.complete(),
            "open_pauses_carried_into_the_shadow": self.pauses_carried,
            "failure": self.failure,
            "scope": "one snapshot decoded at start_tick and stepped along the primary's own timeline, receiving the identical prescribed care if the window crosses it, compared every tick on the whole persisted WorldState and on the exact LifeEvent and QuietEvent streams. The primary is uninterrupted: it is never rebuilt from its own bytes and takes no extra step or RNG draw for this proof.",
        })
    }
}

/// The single comparison the restart proof makes. Factored out so an adversarial test can feed it
/// a deliberately damaged shadow — a dropped pause, a rewritten underlying mode — instead of two
/// copies of one decode that would agree however wrong they both were.
fn shadow_disagreement(
    shadow: &WorldState,
    primary: &WorldState,
    shadow_life: &[LifeEvent],
    primary_life: &[LifeEvent],
    shadow_quiet: &[QuietEvent],
    primary_quiet: &[QuietEvent],
) -> Option<String> {
    let at = primary.tick;
    if shadow.quiet.policy != primary.quiet.policy {
        return Some(format!(
            "tick {at}: the shadow's policy is {} where the primary's is {}",
            shadow.quiet.policy.as_str(),
            primary.quiet.policy.as_str()
        ));
    }
    if shadow.quiet.pauses != primary.quiet.pauses {
        return Some(format!(
            "tick {at}: the persisted pause set differs — the shadow holds {:?}, the primary {:?}",
            shadow.quiet.pauses, primary.quiet.pauses
        ));
    }
    if shadow != primary {
        return Some(format!(
            "tick {at}: the persisted state differs (state hashes {} and {})",
            cubarium_core::snapshot::state_hash(shadow),
            cubarium_core::snapshot::state_hash(primary)
        ));
    }
    if format!("{shadow_quiet:?}") != format!("{primary_quiet:?}") {
        return Some(format!(
            "tick {at}: the quiet records differ — {shadow_quiet:?} against {primary_quiet:?}"
        ));
    }
    if format!("{shadow_life:?}") != format!("{primary_life:?}") {
        return Some(format!(
            "tick {at}: the life records differ — {shadow_life:?} against {primary_life:?}"
        ));
    }
    None
}

struct Arm {
    world: World,
    base: Baseline,
    opening: WorldState,
    feed: bool,
    fed_at: Option<u64>,
    observer: bouts::Observer,
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
    population_ticks: u64,
    extinction_tick: Option<u64>,
    /// Starvation, age, collapse, predation — the last stays zero without hunters, and is
    /// carried so a nonzero value would be visible rather than silently folded into another.
    deaths: [u64; 4],
    births: u64,
    /// Who each living organism is, kept so a **death** can still be reported with the form,
    /// age and opening lineage it had — a terminal record written after the body is gone.
    lineage: BTreeMap<OrganismId, Lineage>,
    life_written: u64,
    planned: u64,
    shadow: Option<Shadow>,
    resume_proofs: Vec<Value>,
    /// A pause was open at a tick with room for a whole comparison window, so a genuine mid-pause
    /// proof was possible and is therefore required.
    mid_pause_opportunity: bool,
    bouts_written: u64,
    bout_stream: BufWriter<File>,
    life_stream: BufWriter<File>,
    quiet_stream: BufWriter<File>,
    census_stream: BufWriter<File>,
    receipts: Vec<Value>,
}

#[derive(Clone, Copy)]
struct Lineage {
    /// The opening organism this one descends from — itself, for an opening organism.
    cohort: OrganismId,
    /// Generations from that opening organism.
    depth: u64,
    form: u8,
    born_tick: u64,
}

impl Arm {
    fn new(
        opening: &WorldState,
        base: Baseline,
        name: &str,
        candidate: bool,
        feed: bool,
        planned: u64,
        dir: &Path,
    ) -> Result<Self> {
        fs::create_dir(dir)?;
        let state = arm_state(opening, candidate);
        let before_hash = cubarium_core::snapshot::state_hash(opening);
        let after_hash = cubarium_core::snapshot::state_hash(&state);
        if candidate {
            ensure!(state.quiet.policy == QuietPolicy::PostBirthPauseV1, "the policy is not set");
            ensure!(before_hash != after_hash, "the candidate choice left the state identical");
        } else {
            ensure!(
                before_hash == after_hash && state.quiet == opening.quiet,
                "the Off arm must leave the opening untouched, bit for bit"
            );
        }
        // The candidate changes the policy and nothing else whatsoever.
        let mut rewound = state.clone();
        rewound.quiet = opening.quiet.clone();
        ensure!(rewound == *opening, "an arm changed something other than the quiet policy");

        let world = World::from_state(state.clone()).map_err(|e| anyhow!(e))?;
        json_new(
            &dir.join("opening.json"),
            &json!({
                "arm": name,
                "quiet_policy": state.quiet.policy.as_str(),
                "candidate": candidate,
                "care": if feed {
                    json!({"kind": "feed", "dose_permille": FEED_DOSE_PERMILLE,
                        "elapsed_tick": FEED_ELAPSED, "target": FEED_TARGET, "count": 1})
                } else {
                    Value::Null
                },
                "opening_tick": state.tick,
                "state_hash_before_choice": before_hash.to_string(),
                "state_hash_after_choice": after_hash.to_string(),
                "ecology_hash": cubarium_core::ecology_hash(&state).to_string(),
                "config_sha256": sha256(&serde_json::to_vec(&state.config)?)?,
                "config": state.config,
                "quiet": state.quiet,
                "pre_intervention_baseline": base.json(),
            }),
        )?;
        // The opening identity census: every organism that was here before anything was chosen,
        // by full generational ID, form, age and structure. Written once, from the arm's own
        // opening state.
        let mut opening_census = stream(&dir.join("opening-organisms.jsonl"))?;
        let mut lineage = BTreeMap::new();
        for (id, o) in state.organisms.iter() {
            line(&mut opening_census, &organism_row(id, o, state.tick))?;
            lineage.insert(
                id,
                Lineage { cohort: id, depth: 0, form: o.phenotype.form, born_tick: o.born_tick },
            );
        }
        opening_census.flush()?;
        opening_census.get_ref().sync_all()?;
        Ok(Self {
            population_min: world.population(),
            population_max: world.population(),
            extinction_tick: (world.population() == 0).then_some(state.tick),
            opening_ledgers: state.energy_ledgers(),
            observer: bouts::Observer::new(&state),
            ancestry: Ancestry::new(&state),
            world,
            base,
            opening: state,
            feed,
            fed_at: None,
            windowed: WindowAudit::default(),
            receipt_energy_in: AccurateSum::default(),
            receipt_energy_out: AccurateSum::default(),
            worst: [0.0; 3],
            worst_corrected_energy: 0.0,
            worst_windowed_energy: 0.0,
            worst_care_energy: 0.0,
            population_ticks: 0,
            deaths: [0; 4],
            births: 0,
            lineage,
            life_written: 0,
            planned,
            shadow: None,
            resume_proofs: Vec::new(),
            mid_pause_opportunity: false,
            bouts_written: 0,
            bout_stream: stream(&dir.join("bouts.jsonl"))?,
            life_stream: stream(&dir.join("life.jsonl"))?,
            quiet_stream: stream(&dir.join("quiet-events.jsonl"))?,
            census_stream: stream(&dir.join("census.jsonl"))?,
            receipts: Vec::new(),
        })
    }

    /// Open a shadow at the primary's current tick, before the primary takes its next step, so the
    /// two are on the same tick and the primary gains nothing for the proof's sake.
    fn begin_shadow(&mut self, trigger: &'static str, window: u64) -> Result<()> {
        let bytes = cubarium_core::encode_snapshot(&self.world.state, BUILD);
        let (_, state) = decode_snapshot(&bytes).map_err(|e| anyhow!("{e:?}"))?;
        // The decoded copy has to *be* this world — pause set and underlying modes included —
        // before it is allowed to stand in for it for sixty ticks.
        ensure!(state == self.world.state, "the snapshot did not round trip");
        ensure!(
            state.quiet == self.world.state.quiet,
            "the decoded copy lost the persisted quiet state"
        );
        let pauses_carried = state.quiet.pauses.len();
        ensure!(
            trigger != "mid_pause" || pauses_carried > 0,
            "a mid-pause proof was claimed with no pause open"
        );
        let world = World::from_state(state).map_err(|e| anyhow!(e))?;
        self.shadow = Some(Shadow {
            world,
            trigger,
            start_tick: self.world.tick(),
            window,
            compared: 0,
            pauses_carried,
            failure: None,
        });
        Ok(())
    }

    /// Step the open shadow alongside the primary's own step, and compare the two. The proof
    /// closes at the end of its window or at the first disagreement, whichever comes first.
    fn advance_shadow(&mut self, life: &[LifeEvent], quiet: &[QuietEvent]) {
        let Some(shadow) = self.shadow.as_mut() else { return };
        shadow.world.step();
        let shadow_life = shadow.world.drain_events();
        let shadow_quiet = shadow.world.drain_quiet_events();
        shadow.compared += 1;
        shadow.failure = shadow_disagreement(
            &shadow.world.state,
            &self.world.state,
            &shadow_life,
            life,
            &shadow_quiet,
            quiet,
        );
        if shadow.failure.is_some() || shadow.compared == shadow.window {
            let proof = shadow.json(self.world.state.tick);
            self.resume_proofs.push(proof);
            self.shadow = None;
        }
    }

    /// Every proof this arm made is complete and equal, an interrupted pause was really proved
    /// wherever one could be, and nothing was left open. An unfinished proof is a failed one.
    fn restart_proofs_pass(&self) -> bool {
        !self.resume_proofs.is_empty()
            && self.shadow.is_none()
            && self.resume_proofs.iter().all(|p| p["complete"] == true)
            && (!self.mid_pause_opportunity
                || self.resume_proofs.iter().any(|p| p["trigger"] == "mid_pause"))
    }

    fn step(&mut self, elapsed: u64, sample_every: u64) -> Result<()> {
        // The restart proof opens here, before the primary's own step, so both worlds stand on
        // the same tick. A genuine mid-pause proof is preferred wherever a pause is open; the
        // fixed boundary is a labelled fallback for an arm that has not paused yet, and a
        // candidate arm that pauses later still owes — and takes — the mid-pause one.
        let open_pauses = self.world.quiet().pauses.len();
        let room = elapsed + RESUME_WINDOW <= self.planned;
        if open_pauses > 0 && room {
            self.mid_pause_opportunity = true;
        }
        if self.shadow.is_none() && room {
            let mid_pause_done = self.resume_proofs.iter().any(|p| p["trigger"] == "mid_pause");
            if open_pauses > 0 && !mid_pause_done {
                self.begin_shadow("mid_pause", RESUME_WINDOW)?;
            } else if elapsed == RESUME_FALLBACK_ELAPSED && self.resume_proofs.is_empty() {
                self.begin_shadow("fixed_boundary", RESUME_WINDOW)?;
            }
        }

        // The one controlled input, at the held boundary, exactly as the runner admits one.
        if self.feed && elapsed == FEED_ELAPSED {
            let before_energy = energy(&self.world.state);
            let seq = self
                .world
                .care()
                .admitted_seq
                .checked_add(1)
                .context("care seq exhausted")?;
            let command = CareCommand {
                seq,
                apply_after_tick: self.world.tick(),
                kind: CareKind::Feed,
                target: FEED_TARGET,
                dose: CareDose::new(FEED_DOSE_PERMILLE).map_err(|e| anyhow!(e))?,
            };
            // A shadow on the primary's timeline receives the identical prescribed care, or it
            // would be comparing a fed world against an unfed one and calling the difference a
            // restart failure.
            if let Some(shadow) = self.shadow.as_mut() {
                shadow.world.apply_care(&command);
            }
            let receipt = self.world.apply_care(&command);
            let booked = receipt.outcome.applied().map_or(0.0, |q| q.energy_in - q.energy_out);
            if let Some(q) = receipt.outcome.applied() {
                self.receipt_energy_in.add(q.energy_in);
                self.receipt_energy_out.add(q.energy_out);
            }
            self.worst_care_energy = self
                .worst_care_energy
                .max((energy(&self.world.state) - before_energy - booked).abs());
            self.fed_at = Some(self.world.tick());
            // Every receipt is retained, refusal and reason included.
            self.receipts.push(json!({
                "elapsed": elapsed, "tick": self.world.tick(), "kind": "feed",
                "dose_permille": FEED_DOSE_PERMILLE, "target": FEED_TARGET,
                "outcome": receipt.outcome.as_str(), "reason": receipt.outcome.reason(),
                "receipt": receipt,
            }));
        }

        let pre = self.observer.before(&self.world);
        let counters = self.world.step();
        let (transient_light, transient_heat) = (counters.light_in, counters.heat_out);
        self.world
            .check_invariants()
            .map_err(|e| anyhow!("tick {}: {e}", self.world.tick()))?;
        let life = self.world.drain_events();
        let quiet = self.world.drain_quiet_events();
        // The shadow follows the step the primary just took; the primary took it for the
        // experiment, not for the proof.
        self.advance_shadow(&life, &quiet);
        // Births and deaths are counted from the world's own per-tick life records, with their
        // full IDs and causes. `TickCounters` accumulate since the last telemetry reset rather
        // than describing one tick, so adding them every tick would multiply every event by the
        // census cadence.
        // Births before deaths, because a parent can die on its child's own tick: the child's
        // lineage must be resolved while the parent is still on the books.
        for event in &life {
            let LifeEvent::Birth {
                tick,
                id,
                parent,
                parent_age_ticks,
                parent_births,
                genome,
                origin,
                mutations,
            } = event
            else {
                continue;
            };
            self.births += 1;
            let born = self.world.state.organisms.get(*id);
            let form = born.map(|o| o.phenotype.form);
            let ancestor = self.lineage.get(parent).copied();
            let row = json!({
                "kind": "birth", "tick": tick, "id": id, "parent": parent,
                "parent_age_ticks": parent_age_ticks, "parent_births": parent_births,
                "genome_digest": genome, "origin": format!("{origin:?}"),
                "mutated_loci": mutations.len(),
                "mutations": mutations.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>(),
                "form": form,
                "opening_cohort": ancestor.map(|a| a.cohort),
                "descendant_depth": ancestor.map(|a| a.depth + 1),
                "structure": born.map(|o| o.structure),
                "reserve": born.map(|o| o.reserve),
                "energy": born.map(|o| o.energy),
                "face": born.map(|o| o.pos.face.index()),
            });
            line(&mut self.life_stream, &row)?;
            self.life_written += 1;
            if let (Some(ancestor), Some(form)) = (ancestor, form) {
                self.lineage.insert(
                    *id,
                    Lineage {
                        cohort: ancestor.cohort,
                        depth: ancestor.depth + 1,
                        form,
                        born_tick: *tick,
                    },
                );
            }
        }
        for event in &life {
            let LifeEvent::Death { tick, id, age_ticks, cause, births, genome } = event else {
                continue;
            };
            self.deaths[match cause {
                DeathCause::Starvation => 0,
                DeathCause::Age => 1,
                DeathCause::Collapse => 2,
                DeathCause::Predation => 3,
            }] += 1;
            // The terminal record keeps what the body carried: the event itself has no form and
            // no lineage, and a form that vanishes entirely would otherwise leave no trace.
            let was = self.lineage.remove(id);
            let row = json!({
                "kind": "death", "tick": tick, "id": id,
                "cause": format!("{cause:?}"), "age_ticks": age_ticks, "births": births,
                "genome_digest": genome,
                "form": was.map(|l| l.form),
                "born_tick": was.map(|l| l.born_tick),
                "opening_cohort": was.map(|l| l.cohort),
                "descendant_depth": was.map(|l| l.depth),
                "was_an_opening_organism": was.is_some_and(|l| l.depth == 0),
            });
            line(&mut self.life_stream, &row)?;
            self.life_written += 1;
        }
        self.ancestry.observe(&life)?;
        self.observer.after(&self.world, pre, &life, &quiet)?;
        for event in &quiet {
            line(&mut self.quiet_stream, &quiet_record(event))?;
        }
        for bout in self.observer.drain_bouts() {
            self.bouts_written += 1;
            line(&mut self.bout_stream, &bout.json())?;
        }
        ensure!(
            self.ancestry.live.len() == self.world.population(),
            "ancestry census mismatch"
        );
        self.population_min = self.population_min.min(self.world.population());
        self.population_max = self.population_max.max(self.world.population());
        self.population_ticks += self.world.population() as u64;
        if self.world.population() == 0 && self.extinction_tick.is_none() {
            self.extinction_tick = Some(self.world.tick());
        }

        let s = &self.world.state;
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

        if (elapsed + 1).is_multiple_of(sample_every) {
            // One telemetry reset per window serves both the independent energy audit and the
            // streamed census: calling it twice would hand one of them an empty window.
            let raw = self.world.telemetry();
            let census = json!({
                "tick": raw.tick, "elapsed": elapsed + 1,
                "population": raw.population, "population_by_form": raw.population_by_form,
                "occupied_cells": raw.occupied_cells,
                "window_births": raw.births,
                "window_deaths": {"starvation": raw.deaths_starvation, "age": raw.deaths_age,
                    "collapse": raw.deaths_collapse},
                "mode": {"resting": raw.mode_resting, "seeking": raw.mode_seeking,
                    "feeding": raw.mode_feeding},
                "escrows": raw.escrows,
                "producer": raw.producer, "fruit": raw.fruit, "detritus": raw.detritus,
                "nutrient": raw.nutrient, "water": raw.water,
                "organism_material": raw.organism_material, "organism_energy": raw.organism_energy,
                "window_light_in": raw.light_in, "window_heat_out": raw.heat_out,
                "surviving_opening_cohorts": self.ancestry.surviving_cohorts(),
                "maximum_descendant_depth": self.ancestry.maximum_depth,
                "open_pauses": self.world.quiet().pauses.len(),
                "state_hash": raw.state_hash.to_string(),
                "ecology_hash": raw.ecology_hash.to_string(),
            });
            line(&mut self.census_stream, &census)?;
            self.windowed.observe(raw);
        }
        Ok(())
    }

    fn finish(
        &mut self,
        dir: &Path,
        planned: u64,
        horizon: Horizon,
        reason: Option<String>,
    ) -> Result<Value> {
        self.observer.close_censored();
        // An unfinished proof is retained as the incomplete proof it is, never dropped so the
        // arm can look clean.
        if let Some(shadow) = self.shadow.take() {
            let tick = self.world.state.tick;
            self.resume_proofs.push(shadow.json(tick));
        }
        for bout in self.observer.drain_bouts() {
            self.bouts_written += 1;
            line(&mut self.bout_stream, &bout.json())?;
        }
        for s in [
            &mut self.bout_stream,
            &mut self.life_stream,
            &mut self.quiet_stream,
            &mut self.census_stream,
        ] {
            s.flush()?;
            s.get_ref().sync_all()?;
        }
        let limits = self.base.limits;
        let legacy_passed = self.worst.iter().zip(limits).all(|(d, l)| *d < l);
        let audits = audit_passes(
            self.worst,
            self.worst_corrected_energy,
            self.worst_windowed_energy,
            self.worst_care_energy,
            limits,
        );
        let s = &self.world.state;
        let bytes = cubarium_core::encode_snapshot(s, BUILD);
        write_new(&dir.join("closing.cubw"), &bytes)?;
        let elapsed = s.tick - self.opening.tick;
        let complete = reason.is_none() && elapsed == planned;
        // Every technical gate at once. A reconciliation failure or a failed restart proof is
        // as disqualifying as a conservation drift: the harness never presents an arm whose own
        // records disagree with the world as a completed measurement.
        let reconciled = self.observer.reconciled();
        let resumed = self.restart_proofs_pass();
        let passed = audits && reconciled && resumed;
        let summary = json!({
            "arm": dir.file_name().and_then(|n| n.to_str()),
            "planned_ticks": planned, "closing_tick": s.tick, "elapsed_ticks": elapsed,
            "horizon": horizon,
            "technical_complete": complete,
            "audit_passed": passed,
            "complete_experiment_measurement": complete && horizon.prescribed() && passed,
            "measurement_note": "certifies audited numerical coverage of a prescribed horizon only; never ecological acceptance, adequate quiet opportunity, improved viability, or permission to change the Off default",
            "termination": reason.clone().unwrap_or_else(|| "planned_horizon".into()),
            "gates": {
                "conservation_and_flow_audits": audits,
                "records_reconcile_to_the_world": reconciled,
                "restart_proofs_complete_and_equal": resumed,
                "legacy_raw_energy": legacy_passed,
            },
            "restart_proofs": self.resume_proofs,
            "a_pause_was_open_with_room_for_a_full_window": self.mid_pause_opportunity,
            "restart_proof_requirement": "every proof recorded must be complete and equal, none may be left open at the close, and an arm that ever held a pause with a full window of ticks remaining must carry a completed mid_pause proof; the fixed_boundary fallback alone never certifies a mid-pause restart",
            "max_absolute_drift": {"material": self.worst[0], "energy": self.worst[1], "water": self.worst[2]},
            "corrected_energy_drift": self.worst_corrected_energy,
            "independent_windowed_energy_drift": self.worst_windowed_energy,
            "care_boundary_energy_drift": self.worst_care_energy,
            "pre_intervention_baseline": self.base.json(),
            "quiet_policy": s.quiet.policy.as_str(),
            "open_pauses_at_close": s.quiet.pauses.len(),
            "care": {
                "planned": self.feed,
                "applied_at_tick": self.fed_at,
                "receipts": self.receipts,
                "ledgers": self.world.care().clone(),
            },
            "rest": self.observer.summary(),
            "population_min": self.population_min, "population_max": self.population_max,
            "population_organism_ticks": self.population_ticks,
            "closing_population": s.organisms.len(),
            "births": self.births,
            "deaths": {"starvation": self.deaths[0], "age": self.deaths[1],
                "collapse": self.deaths[2], "predation": self.deaths[3]},
            "first_extinction_tick": self.extinction_tick,
            "surviving_opening_cohorts": self.ancestry.surviving_cohorts(),
            "maximum_descendant_depth": self.ancestry.maximum_depth,
            "bouts_written": self.bouts_written,
            "life_records_written": self.life_written,
            "living_lineages_at_close": self.lineage.len(),
            "lineage_basis": "every Birth and Death is streamed to life.jsonl with full generational IDs, the parent, the cause, the age, the genome digest, the form and the opening organism it descends from; the opening identities themselves are in opening-organisms.jsonl. A death keeps the form it wore, so a form that disappears entirely still has a record.",
            "closing_state_hash": cubarium_core::snapshot::state_hash(s).to_string(),
            "closing_ecology_hash": cubarium_core::ecology_hash(s).to_string(),
            "closing_snapshot_sha256": sha256(&bytes)?,
            "unsupported_measurements": [
                "exact funding and oxidation amounts: the ordinary API offers no mutation-site evidence, so no post-step delta here is presented as either"
            ],
        });
        json_new(&dir.join("summary.json"), &summary)?;
        Ok(summary)
    }
}

// ---------------------------------------------------------------- driver

/// Finalization can fail after simulation has advanced (a refused output write, say). Keep the
/// actual progress and the original step error without presenting stale observers as a valid
/// completed measurement. Every other prescribed arm still runs.
fn finish_retained(
    arm: &mut Arm,
    dir: &Path,
    planned: u64,
    horizon: Horizon,
    reason: Option<String>,
) -> Value {
    match arm.finish(dir, planned, horizon, reason.clone()) {
        Ok(summary) => summary,
        Err(error) => json!({
            "arm": dir.file_name().and_then(|n| n.to_str()),
            "technical_complete": false, "complete_experiment_measurement": false,
            "audit_passed": false,
            "termination": "finalization_failure", "error": format!("{error:#}"),
            "step_failure": reason,
            "planned_ticks": planned, "closing_tick": arm.world.tick(),
            "elapsed_ticks": arm.world.tick() - arm.opening.tick,
            "closing_population": arm.world.population(),
            "observer_trust": "partial; no completed measurement is claimed",
        }),
    }
}

fn numerical_success(seeds: &[Value]) -> bool {
    seeds.len() == 12
        && seeds.iter().all(|s| {
            s["technical_complete"] == true
                && s["arms"].as_array().is_some_and(|arms| {
                    arms.len() == ARMS.len()
                        && arms.iter().all(|a| {
                            a["technical_complete"] == true && a["audit_passed"] == true
                        })
                })
        })
}

fn run_seed(
    opening: &Opening,
    dir: &Path,
    planned: u64,
    horizon: Horizon,
    sample_every: u64,
) -> Result<Value> {
    fs::create_dir(dir)?;
    let base = Baseline::read(&opening.state)?;
    let mut summaries = Vec::new();
    let mut failure: Option<String> = None;
    for (name, candidate, feed) in ARMS {
        let arm_dir = dir.join(name);
        let mut arm = match Arm::new(&opening.state, base, name, candidate, feed, planned, &arm_dir)
        {
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
        for elapsed in 0..planned {
            if let Err(error) = arm.step(elapsed, sample_every) {
                // A failure is retained, never retried and never silently dropped.
                reason = Some(format!("step_failure at elapsed {elapsed}: {error:#}"));
                failure.get_or_insert_with(|| format!("{name}: {}", reason.clone().unwrap()));
                break;
            }
        }
        let summary = finish_retained(&mut arm, &arm_dir, planned, horizon, reason);
        if summary["technical_complete"] != true || summary["audit_passed"] != true {
            failure.get_or_insert_with(|| format!("{name}: numerical or finalization failure"));
        }
        summaries.push(summary);
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
    // The read-only mode returns before anything else exists: no cohort is loaded, no output
    // directory is created, no world is built.
    if let Some(path) = &args.inspect {
        println!("{}", serde_json::to_string_pretty(&inspect(path)?)?);
        return Ok(());
    }
    let cohort_dir = args.cohort.clone().context("a cohort directory is required")?;
    let out = args.out.clone().context("an output directory is required")?;
    let planned = args.horizon.ticks();
    ensure!(
        planned.is_multiple_of(args.sample_every),
        "the horizon must be a whole number of census windows"
    );
    let (cohort, openings) = load_cohort(&cohort_dir)?;
    // A brand new directory, or nothing: `create_dir` fails on an existing path, so an accidental
    // rerun cannot overwrite or silently resume a completed screen.
    fs::create_dir(&out).context("output must be a NEW directory")?;
    let executable_path = std::env::current_exe()?;
    let executable = fs::read(&executable_path)?;
    write_new(&out.join("quiet_compare.frozen"), &executable)?;
    fs::set_permissions(
        out.join("quiet_compare.frozen"),
        fs::metadata(executable_path)?.permissions(),
    )?;
    json_new(
        &out.join("manifest.json"),
        &json!({
            "kind": "four-arm-ordinary-quiet-comparison",
            "build": BUILD, "executable_sha256": sha256(&executable)?,
            "cohort": cohort,
            "cohort_source": cohort_dir,
            "horizon": args.horizon, "ticks": planned, "sample_every": args.sample_every,
            "arms": ARMS.map(|(n, _, _)| n),
            "factors": {
                "policy": {"field": "WorldState.quiet",
                    "levels": [QuietPolicy::Off.as_str(), QuietPolicy::PostBirthPauseV1.as_str()],
                    "applied": "once, to a copied state, before World::from_state; the Off arm never writes the field and is never hot-toggled"},
                "care": {"levels": [null, "one Standard Feed"],
                    "recipe": {"kind": "feed", "dose_permille": FEED_DOSE_PERMILLE,
                        "elapsed_tick": FEED_ELAPSED, "target": FEED_TARGET, "count": 1}},
            },
            "unchanged": "every config value, ambient support, weather and RNG streams, physiology, reproduction eligibility and costs, stocks, IDs and opening ledgers; no hunters, no cleanup, no rain, no later rescue",
            "classification": "post_birth_recovery, satiated and newborn_initial are distinguished from actual core records and the pause set captured before each decision, never from Mode::Resting alone; the release boundary and an early abort are both ordinary decisions and are counted as such",
            "observer_contract": "ordinary-quiet-v1; shared pre-intervention baseline per seed, strict unchanged material/energy/water limits, persisted compensated energy, independent windowed energy, immediate care-boundary energy, tickwise record reconciliation and a bounded per-arm snapshot-resume proof; streamed bouts, events and census, no in-memory history",
            "limits": "A technical pass certifies audited numbers over the planned horizon. It is not ecological acceptance, not evidence of adequate quiet opportunity, not evidence of improved viability or recruitment, and not permission to change the Off default. Forty ticks is the proposal's unvalidated candidate duration.",
        }),
    )?;

    let mut all = Vec::new();
    let mut failed = false;
    for opening in &openings {
        let seed = opening.state.config.seed;
        let result = match run_seed(
            opening,
            &out.join(format!("seed-{seed}")),
            planned,
            args.horizon,
            args.sample_every,
        ) {
            Ok(result) => result,
            Err(error) => json!({
                "seed": seed, "technical_complete": false,
                "complete_experiment_measurement": false, "audit_passed": false,
                "termination": "seed_output_or_initialization_failure",
                "error": format!("{error:#}"),
            }),
        };
        if result["technical_complete"] != true {
            failed = true;
        }
        eprintln!(
            "seed {seed}/12 {}",
            if result["technical_complete"] == true { "complete" } else { "RETAINED WITH FAILURE" }
        );
        all.push(result);
    }
    let numerical_passed = numerical_success(&all);
    let summary = json!({
        "technical_complete": !failed,
        "audit_passed": numerical_passed,
        "complete_experiment_measurement": !failed
            && args.horizon.prescribed()
            && args.sample_every == 200
            && numerical_passed,
        "horizon": args.horizon, "ticks": planned,
        "seeds": all,
        "retention": "every seed is retained, extinction, empty form, rejected pause and technical failure included; nothing is filtered, retried or reseeded",
    });
    json_new(&out.join("summary.json"), &summary)?;
    println!("{}", out.display());
    ensure!(
        numerical_passed,
        "retained quiet cohort contains technical or numerical failures; see summary.json"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn opening() -> WorldState {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cubarium-core/tests/fixtures/quiet-v12-plain-3000.cubw");
        let (_, state) = decode_snapshot(&fs::read(path).expect("the fixture")).expect("loads");
        state
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cubarium-quiet-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The family is exactly the preregistered four, crossing two policies with two care levels,
    /// and the horizons are the prescribed ones under their own names.
    #[test]
    fn the_family_and_the_horizons_are_the_preregistered_ones() {
        assert_eq!(ARMS.len(), 4);
        assert_eq!(ARMS.iter().filter(|(_, c, _)| *c).count(), 2);
        assert_eq!(ARMS.iter().filter(|(_, _, f)| *f).count(), 2);
        let names: BTreeSet<&str> = ARMS.iter().map(|(n, _, _)| *n).collect();
        assert_eq!(
            names,
            BTreeSet::from(["off_nocare", "candidate_nocare", "off_feed", "candidate_feed"])
        );
        assert_eq!(Horizon::TenMinute.ticks(), 12_000);
        assert_eq!(Horizon::TwoHour.ticks(), 144_000);
        assert_eq!(Horizon::TwentyFourHour.ticks(), 1_728_000);
        assert_eq!(Horizon::SeventyTwoHour.ticks(), 5_184_000);
        assert!(!Horizon::Smoke.prescribed());
        for h in [
            Horizon::TenMinute,
            Horizon::TwoHour,
            Horizon::TwentyFourHour,
            Horizon::SeventyTwoHour,
        ] {
            assert!(h.prescribed());
            assert!(h.ticks().is_multiple_of(200), "the 200-tick cadence must divide {h:?}");
        }
        // The care recipe is the quiet diagnosis's own, once, and nothing else.
        assert_eq!(FEED_ELAPSED, 600);
        assert_eq!(FEED_DOSE_PERMILLE, 1000);
        assert_eq!(FEED_TARGET, CareTarget { face: 0, u: 32.0, v: 48.0 });
    }

    /// The Off arm is the copied world bit for bit, and the candidate differs in the policy and
    /// in nothing else — not a config value, not a stock, not an ID, not an opening ledger.
    #[test]
    fn an_arm_changes_the_policy_and_nothing_else() {
        let o = opening();
        let off = arm_state(&o, false);
        assert_eq!(off, o, "the Off arm changed something");
        assert_eq!(
            cubarium_core::snapshot::state_hash(&off),
            cubarium_core::snapshot::state_hash(&o)
        );

        let candidate = arm_state(&o, true);
        assert_eq!(candidate.quiet.policy, QuietPolicy::PostBirthPauseV1);
        assert!(candidate.quiet.pauses.is_empty(), "a candidate opens with no retroactive pause");
        assert_ne!(
            cubarium_core::snapshot::state_hash(&candidate),
            cubarium_core::snapshot::state_hash(&o)
        );
        let mut rewound = candidate.clone();
        rewound.quiet = o.quiet.clone();
        assert_eq!(rewound, o, "the candidate changed something other than the policy");
        assert_eq!(candidate.config, o.config);
        assert_eq!(candidate.fields, o.fields);
        assert_eq!(candidate.organisms, o.organisms);
        assert_eq!(candidate.weather, o.weather);
        assert_eq!(candidate.care, o.care);
        assert_eq!(candidate.hunters, o.hunters);
        assert_eq!(candidate.external_material_in, o.external_material_in);
        assert_eq!(candidate.light_in_total, o.light_in_total);
        assert_eq!(candidate.heat_out_total, o.heat_out_total);
        assert_eq!(candidate.births_total, o.births_total);
    }

    /// An Off arm is an ordinary continuation, with and without the one Standard Feed: the
    /// harness adds observation, and observation is inert.
    #[test]
    fn the_off_arms_are_ordinary_continuations_including_care() {
        for feed in [false, true] {
            let o = opening();
            let base = Baseline::read(&o).unwrap();
            let dir = temp(if feed { "off-feed" } else { "off-nocare" });
            let name = if feed { "off_feed" } else { "off_nocare" };
            let mut arm = Arm::new(&o, base, name, false, feed, 900, &dir.join("arm")).unwrap();
            let mut plain = World::from_state(o.clone()).unwrap();
            for elapsed in 0..900 {
                arm.step(elapsed, 200).unwrap();
                if feed && elapsed == FEED_ELAPSED {
                    let seq = plain.care().admitted_seq + 1;
                    let tick = plain.tick();
                    plain.apply_care(&CareCommand {
                        seq,
                        apply_after_tick: tick,
                        kind: CareKind::Feed,
                        target: FEED_TARGET,
                        dose: CareDose::new(FEED_DOSE_PERMILLE).unwrap(),
                    });
                }
                plain.step();
                plain.drain_events();
                plain.drain_quiet_events();
                assert_eq!(
                    cubarium_core::snapshot::state_hash(&arm.world.state),
                    cubarium_core::snapshot::state_hash(&plain.state),
                    "{name}: diverged at elapsed {elapsed}"
                );
            }
            assert_eq!(arm.observer.admissions, 0, "{name}: an Off arm admitted a pause");
            assert!(arm.observer.reconciled());
            assert_eq!(arm.fed_at.is_some(), feed);
            if feed {
                assert_eq!(arm.world.care().admitted_seq, 1);
                assert!(arm.world.care().feed_material_in > 0.0);
                assert_eq!(arm.receipts.len(), 1, "the single receipt is retained");
            } else {
                assert_eq!(arm.world.state.care, o.care, "a no-care arm received nothing");
            }
            // An Off arm never pauses, so its proof is the labelled fallback — and it says so
            // rather than claiming a mid-pause restart it never made.
            assert!(arm.shadow.is_none(), "{name}: the proof closed inside the run");
            assert!(arm.restart_proofs_pass(), "{name}: the restart proof must run and pass");
            assert_eq!(arm.resume_proofs.len(), 1, "{name}: exactly one proof");
            assert_eq!(arm.resume_proofs[0]["trigger"], "fixed_boundary");
            assert_eq!(arm.resume_proofs[0]["compared_ticks"], RESUME_WINDOW);
            assert_eq!(arm.resume_proofs[0]["open_pauses_carried_into_the_shadow"], 0);
            assert!(!arm.mid_pause_opportunity, "{name}: an Off arm has no pause to interrupt");
            fs::remove_dir_all(&dir).ok();
        }
    }

    /// A candidate arm really admits pauses, its records reconcile, and its restart proof passes.
    #[test]
    fn a_candidate_arm_admits_reconciled_pauses_and_resumes_exactly() {
        let o = opening();
        let base = Baseline::read(&o).unwrap();
        let dir = temp("candidate");
        let mut arm =
            Arm::new(&o, base, "candidate_nocare", true, false, 3000, &dir.join("arm")).unwrap();
        for elapsed in 0..3000 {
            arm.step(elapsed, 200).unwrap();
        }
        assert!(arm.observer.admissions > 0, "the fixture must admit a pause in 3000 ticks");
        assert!(arm.observer.reconciled(), "{}", arm.observer.summary());
        assert_eq!(arm.observer.held_intake_ticks, 0);
        assert!(arm.restart_proofs_pass(), "the mid-pause restart proof must pass");
        assert!(arm.mid_pause_opportunity, "a pause was open with a whole window to spare");
        let mid = arm
            .resume_proofs
            .iter()
            .find(|p| p["trigger"] == "mid_pause")
            .expect("a candidate arm that pauses owes a genuine mid-pause proof");
        assert!(mid["open_pauses_carried_into_the_shadow"].as_u64().unwrap() > 0);
        assert_eq!(mid["compared_ticks"], RESUME_WINDOW);
        assert_eq!(mid["failure"], Value::Null);
        assert!(arm.bouts_written > 0);
        // Recovery really is separated from the other two classes.
        let rest = arm.observer.summary();
        assert!(rest["post_birth_recovery"]["organism_ticks"].as_u64().unwrap() > 0);
        assert!(rest["newborn_initial"]["bouts"].as_u64().unwrap() > 0);
        fs::remove_dir_all(&dir).ok();
    }

    /// A reconciliation failure is disqualifying, exactly like a conservation drift. An arm whose
    /// own records disagree with the world is never a completed measurement.
    #[test]
    fn every_technical_gate_can_fail_the_arm_on_its_own() {
        let seed = json!({"technical_complete": true,
            "arms": (0..4).map(|_| json!({"technical_complete": true, "audit_passed": true}))
                .collect::<Vec<_>>()});
        let good = vec![seed; 12];
        assert!(numerical_success(&good));
        for field in ["audit_passed", "technical_complete"] {
            let mut bad = good.clone();
            bad[11]["arms"][3][field] = json!(false);
            assert!(!numerical_success(&bad), "{field} must fail the cohort");
        }
        assert!(!numerical_success(&good[..11]), "eleven seeds is not the cohort");
        let mut missing = good.clone();
        missing[0]["arms"].as_array_mut().unwrap().pop();
        assert!(!numerical_success(&missing), "three arms is not the family");
    }

    /// The audit is not decorative: an unbooked change is caught at the unchanged limit, in every
    /// currency, and no tolerance is widened to let a run finish.
    #[test]
    fn an_unbooked_change_fails_the_audit_at_the_unchanged_limit() {
        for (name, damage) in [("material", 0usize), ("energy", 1), ("water", 2)] {
            let o = opening();
            let base = Baseline::read(&o).unwrap();
            let dir = temp(&format!("adversarial-{name}"));
            let mut arm =
                Arm::new(&o, base, "off_nocare", false, false, 2400, &dir.join("arm")).unwrap();
            arm.step(0, 200).unwrap();
            let bump = base.limits[damage];
            match damage {
                0 => arm.world.state.fields.d[17] += bump,
                1 => arm.world.state.fields.de[17] += bump,
                _ => arm.world.state.fields.w[17] += bump,
            }
            for e in 1..40 {
                let _ = arm.step(e, 200);
            }
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
            fs::remove_dir_all(&dir).ok();
        }
    }

    /// The cohort is a fixed, complete, checksummed twelve, and every way of being not-that is a
    /// refusal before a single world is constructed.
    #[test]
    fn an_incomplete_or_tampered_cohort_is_refused() {
        let real = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../captures/hunter-openings-2026-09-13");
        if !real.join("manifest.json").exists() {
            eprintln!("skipping: the prescribed cohort is not present in this checkout");
            return;
        }
        let (manifest, openings) = load_cohort(&real).expect("the prescribed cohort must load");
        assert_eq!(openings.len(), 12);
        assert_eq!(manifest["opening_tick"], 144_000);
        assert!(openings.iter().enumerate().all(|(i, o)| o.state.config.seed == i as u64 + 1));

        let dir = temp("cohort");
        let rows: Value =
            serde_json::from_str(&fs::read_to_string(real.join("manifest.json")).unwrap()).unwrap();
        assert!(load_cohort(&dir.join("absent")).is_err());

        type Damage = fn(&mut Value);
        let cases: [(&str, Damage); 6] = [
            ("incomplete", |m| m["complete"] = json!(false)),
            ("wrong-kind", |m| m["kind"] = json!("something-else")),
            ("eleven", |m| {
                m["openings"].as_array_mut().unwrap().pop();
            }),
            ("duplicate-seed", |m| {
                let seed = m["openings"][0]["seed"].clone();
                m["openings"][1]["seed"] = seed;
            }),
            ("tampered-checksum", |m| m["openings"][0]["sha256"] = json!("0".repeat(64))),
            ("tampered-ecology-hash", |m| m["openings"][0]["ecology_hash"] = json!("1")),
        ];
        for (name, damage) in cases {
            let case = dir.join(name);
            fs::create_dir_all(&case).unwrap();
            let mut m = rows.clone();
            damage(&mut m);
            fs::write(case.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
            for seed in 1..=12u64 {
                let to = case.join(format!("seed-{seed}"));
                fs::create_dir_all(&to).unwrap();
                fs::copy(
                    real.join(format!("seed-{seed}/world-144000.cubw")),
                    to.join("world-144000.cubw"),
                )
                .unwrap();
            }
            assert!(load_cohort(&case).is_err(), "{name}: a damaged cohort must be refused");
        }

        // And a snapshot whose bytes no longer match its recorded checksum.
        let case = dir.join("flipped-byte");
        fs::create_dir_all(&case).unwrap();
        fs::write(case.join("manifest.json"), serde_json::to_vec(&rows).unwrap()).unwrap();
        for seed in 1..=12u64 {
            fs::create_dir_all(case.join(format!("seed-{seed}"))).unwrap();
            fs::copy(
                real.join(format!("seed-{seed}/world-144000.cubw")),
                case.join(format!("seed-{seed}/world-144000.cubw")),
            )
            .unwrap();
        }
        assert!(load_cohort(&case).is_ok(), "the copy must be faithful first");
        let path = case.join("seed-1/world-144000.cubw");
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&path, &bytes).unwrap();
        assert!(load_cohort(&case).is_err(), "a flipped byte must be caught");
        fs::remove_dir_all(&dir).ok();
    }

    /// A cohort opening that already carries a quiet extension, a pause, hunter or care state, a
    /// wrong tick, seed, schema, population or ecology hash is refused by the loader's own
    /// per-opening check — the one `load_cohort` runs, on a decoded state, not on a description
    /// of one.
    #[test]
    fn an_opening_that_already_carries_a_policy_is_refused() {
        let real = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../captures/hunter-openings-2026-09-13");
        if !real.join("manifest.json").exists() {
            eprintln!("skipping: the prescribed cohort is not present in this checkout");
            return;
        }
        let manifest: Value =
            serde_json::from_str(&fs::read_to_string(real.join("manifest.json")).unwrap()).unwrap();
        let row = manifest["openings"][0].clone();
        let seed = row["seed"].as_u64().unwrap();
        let bytes = fs::read(real.join(format!("seed-{seed}/world-144000.cubw"))).unwrap();
        let (meta, state) = decode_snapshot(&bytes).unwrap();
        assert_eq!(meta.schema, 9);
        validate_opening(meta.schema, &state, seed, &row).expect("the real opening must pass");

        // The guard that matters here: a policy, or a pause, already present in the input.
        let mut policied = state.clone();
        policied.quiet = QuietState::post_birth_pause_v1();
        let error = validate_opening(meta.schema, &policied, seed, &row)
            .expect_err("an opening carrying a policy must be refused")
            .to_string();
        assert!(error.contains("quiet extension"), "{error}");

        // And the rest of the same check, each on its own.
        type Damage = fn(&mut WorldState);
        let cases: [(&str, Damage); 4] = [
            ("schema-and-tick", |s| s.tick += 1),
            ("seed", |s| s.config.seed += 1),
            ("care", |s| s.care.admitted_seq += 1),
            ("ecology", |s| {
                s.fields.n[0] += 1.0;
            }),
        ];
        for (name, damage) in cases {
            let mut damaged = state.clone();
            damage(&mut damaged);
            assert!(
                validate_opening(meta.schema, &damaged, seed, &row).is_err(),
                "{name}: a damaged opening must be refused"
            );
        }
        // A census the manifest does not agree with, and a population claim with nothing behind it.
        let mut miscounted = row.clone();
        miscounted["population"] = json!(state.organisms.len() as u64 + 1);
        assert!(validate_opening(meta.schema, &state, seed, &miscounted).is_err());
        let mut absent = row.clone();
        absent["population"] = Value::Null;
        assert!(validate_opening(meta.schema, &state, seed, &absent).is_err());
        assert!(
            validate_opening(13, &state, seed, &row).is_err(),
            "an opening at another schema is not the frozen pre-hunter one"
        );
    }

    /// The restart proof has to be able to fail. A shadow that lost its persisted pause, or whose
    /// retained underlying mode was rewritten, diverges from the **uninterrupted** primary — while
    /// two copies decoded from those same damaged bytes agree with each other perfectly for the
    /// whole window. That agreement is exactly what a mirror-against-mirror proof would have
    /// certified, and exactly why this one compares against the world that was never restarted.
    #[test]
    fn a_damaged_shadow_is_caught_where_two_identical_decodes_agree() {
        use cubarium_core::organism::Mode;
        let o = opening();
        let mut primary = World::from_state(arm_state(&o, true)).unwrap();
        let mut ticks = 0;
        while primary.quiet().pauses.is_empty() {
            primary.step();
            primary.drain_events();
            primary.drain_quiet_events();
            ticks += 1;
            assert!(ticks < 3000, "the fixture must admit a pause to interrupt");
        }

        let bytes = cubarium_core::encode_snapshot(&primary.state, BUILD);
        let (_, faithful) = decode_snapshot(&bytes).unwrap();
        assert!(!faithful.quiet.pauses.is_empty(), "the snapshot must carry the open pause");
        let mut dropped = faithful.clone();
        dropped.quiet.pauses.clear();
        let mut rewritten = faithful.clone();
        let original = rewritten.quiet.pauses[0].underlying;
        // Deliberately `Resting`: the retained underlying mode is re-derived through the ordinary
        // hysteresis every held tick, and `Seeking` and `Feeding` converge on the next tick from
        // the same food and hunger, so swapping those two is absorbed. `Resting` is the one the
        // hysteresis keeps, so a shadow given it stays quiet where the primary went back to work.
        let swapped = if original == Mode::Resting { Mode::Seeking } else { Mode::Resting };
        rewritten.quiet.pauses[0].underlying = swapped;

        // Two decodes of the damaged bytes, to stand in for the mirror the proof used to build.
        let damaged_bytes = cubarium_core::encode_snapshot(&dropped, BUILD);
        let mut mirror_a = World::from_state(decode_snapshot(&damaged_bytes).unwrap().1).unwrap();
        let mut mirror_b = World::from_state(decode_snapshot(&damaged_bytes).unwrap().1).unwrap();

        // Before a shadow is allowed to stand in for the primary it must *be* the primary, and
        // that check is this comparison on the decoded state itself — where a lost pause, a
        // rewritten underlying mode and a flipped policy are all visible, whatever the next step
        // would have done with them.
        assert_eq!(
            shadow_disagreement(&faithful, &primary.state, &[], &[], &[], &[]),
            None,
            "the faithful decode is the primary"
        );
        for (name, damaged) in [("dropped", &dropped), ("rewritten", &rewritten)] {
            let seen = shadow_disagreement(damaged, &primary.state, &[], &[], &[], &[])
                .unwrap_or_else(|| panic!("{name}: damage to the persisted pause must be seen"));
            assert!(seen.contains("pause set"), "{name}: {seen}");
        }
        let mut off_policy = faithful.clone();
        off_policy.quiet = QuietState::default();
        let seen = shadow_disagreement(&off_policy, &primary.state, &[], &[], &[], &[]).unwrap();
        assert!(seen.contains("policy"), "{seen}");

        let mut shadows = [
            ("faithful", World::from_state(faithful).unwrap(), None::<String>),
            ("dropped_pause", World::from_state(dropped).unwrap(), None),
            ("rewritten_underlying", World::from_state(rewritten).unwrap(), None),
        ];
        for _ in 0..RESUME_WINDOW {
            primary.step();
            let life = primary.drain_events();
            let quiet = primary.drain_quiet_events();
            for (_, world, failure) in shadows.iter_mut() {
                world.step();
                let shadow_life = world.drain_events();
                let shadow_quiet = world.drain_quiet_events();
                if failure.is_none() {
                    *failure = shadow_disagreement(
                        &world.state,
                        &primary.state,
                        &shadow_life,
                        &life,
                        &shadow_quiet,
                        &quiet,
                    );
                }
            }
            mirror_a.step();
            mirror_b.step();
            assert_eq!(
                format!("{:?}", mirror_a.drain_quiet_events()),
                format!("{:?}", mirror_b.drain_quiet_events())
            );
            assert_eq!(
                format!("{:?}", mirror_a.drain_events()),
                format!("{:?}", mirror_b.drain_events())
            );
            assert_eq!(
                cubarium_core::snapshot::state_hash(&mirror_a.state),
                cubarium_core::snapshot::state_hash(&mirror_b.state),
                "two decodes of the same damaged bytes agree: a mirror proves nothing about loss"
            );
        }

        assert_eq!(shadows[0].2, None, "an undamaged shadow must track the primary exactly");
        let dropped_failure = shadows[1].2.clone().expect("a lost pause must be caught");
        assert!(dropped_failure.contains("pause set"), "{dropped_failure}");

        // And the measured truth about the third one, stated rather than assumed either way:
        // `QuietPause::underlying` is a **carry**, rewritten from the ordinary hysteresis on every
        // held decision, so for this hungry parent `Resting`, `Seeking` and `Feeding` all resolve
        // to the same next ordinary mode and the damage is absorbed within one step. It is caught
        // where it is persisted — at the decode compared above, which is what `begin_shadow`
        // runs — and a stepped comparison is not a general detector of it.
        assert_ne!(swapped, original, "the rewrite must really change the carried mode");
        assert_eq!(
            shadows[2].1.quiet().pauses,
            primary.quiet().pauses,
            "the core re-derives the carried underlying mode every held tick"
        );
        assert_eq!(shadows[2].2, None, "and so this particular damage leaves no trace to step on");
    }

    /// The output directory is exclusive and every file inside it is written once: a rerun into a
    /// used path is refused rather than resumed, reseeded or merged.
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
