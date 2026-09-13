//! Six matched, care-free arms over every prescribed aged seed. Never touches
//! the owning runner, live state, web endpoints or display transport.
//! Local recovery integration is a separate observer module; until exact capture
//! positions are exposed, summaries explicitly refuse to claim that measurement.

#[path = "hunter_compare/audit.rs"]
mod audit;
#[path = "hunter_compare/recovery.rs"]
#[allow(dead_code)] // Local observer is integrated after exact core capture positions land.
mod recovery;

use anyhow::{Context, Result, anyhow, ensure};
use clap::Parser;
use cubarium_core::organism::DeathCause;
use cubarium_core::{
    FixedHunterProfile, HunterEvent, HunterTarget, LifeEvent, OrganismId, World, WorldState,
    decode_snapshot, encode_snapshot,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const NAMES: [&str; 6] = [
    "untouched",
    "budget_control",
    "specialist_off",
    "specialist_on",
    "facultative_off",
    "facultative_on",
];
const BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("CUBARIUM_GIT_HASH"));

#[derive(Parser)]
struct Args {
    /// Completed prepare-hunter-worlds cohort directory (all seeds1–12).
    cohort: PathBuf,
    /// Brand-new directory; existing output is never overwritten or resumed.
    out: PathBuf,
    /// Elapsed trial ticks AFTER import, up to72h; short values are smoke tests.
    #[arg(long, default_value_t = 144000, value_parser = clap::value_parser!(u64).range(1..=5184000))]
    ticks: u64,
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u64).range(1..=200))]
    audit_window: u64,
}

fn sha256(bytes: &[u8]) -> Result<String> {
    // Use the host's standard checksum utility, with no shell interpolation or
    // added package dependency. Input is bytes, not a command or path argument.
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .context("checksum stdin")?
        .write_all(bytes)?;
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
fn line(file: &mut BufWriter<File>, value: &Value) -> Result<()> {
    serde_json::to_writer(&mut *file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}
fn stream(path: &Path) -> Result<BufWriter<File>> {
    Ok(BufWriter::new(
        OpenOptions::new().write(true).create_new(true).open(path)?,
    ))
}

struct Opening {
    state: WorldState,
    source: Value,
}
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
        // The archive's paths are provenance. Resolve the fixed cohort layout,
        // not arbitrary paths supplied by its manifest.
        let bytes = fs::read(dir.join(format!("seed-{seed}/world-144000.cubw")))?;
        ensure!(
            sha256(&bytes)? == row["sha256"].as_str().context("snapshot SHA256")?,
            "seed{seed} checksum changed"
        );
        let (meta, state) = decode_snapshot(&bytes)?;
        ensure!(
            meta.schema == 9 && state.tick == 144000 && state.config.seed == seed,
            "seed{seed} is not its frozen pre-hunter two-hour opening"
        );
        ensure!(
            state.hunters == Default::default() && state.care == Default::default(),
            "primary opening contains hunter/care state"
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
        openings.push(Opening {
            state,
            source: row.clone(),
        });
    }
    openings.sort_by_key(|o| (o.state.organisms.len(), o.state.config.seed));
    for (rank, o) in openings.iter().enumerate() {
        ensure!(
            o.source["population_rank"] == rank + 1
                && o.source["population_stratum"] == ["low", "middle", "high"][rank / 4],
            "cohort stratum changed"
        );
    }
    openings.sort_by_key(|o| o.state.config.seed);
    Ok((manifest, openings))
}

#[derive(Clone)]
struct Lineage {
    root: OrganismId,
    depth: u64,
    hunter: bool,
    adult_seen: bool,
    reproduced: bool,
}
struct Arm {
    world: World,
    audit: audit::Audit,
    live: BTreeMap<OrganismId, Lineage>,
    founder: Option<OrganismId>,
    founder_extinction: Option<u64>,
    lineage_extinction: Option<u64>,
    prey_min: usize,
    prey_sum: u64,
    adult_bins: [u64; 4],
    adults_max: usize,
    over_two_run: u64,
    over_two_longest: u64,
    zero_hunter_ticks: u64,
    captures: u64,
    offspring: u64,
    escrows: BTreeMap<OrganismId, cubarium_core::organism::Escrow>,
    escrow_starts: u64,
    escrows_closed_without_birth: u64,
    adult_descendants: u64,
    descendant_parents: u64,
    last_complete_observer_tick: u64,
    whole_recovery: recovery::WholeRecovery,
    events: BufWriter<File>,
    census: BufWriter<File>,
}

fn offspring_children(life: &[LifeEvent], hunting: &[HunterEvent]) -> Result<BTreeSet<OrganismId>> {
    let mut children = BTreeSet::new();
    for event in hunting {
        if let HunterEvent::Offspring { parent, child, .. } = event {
            ensure!(children.insert(*child), "duplicate hunter offspring event");
            ensure!(
                life.iter()
                    .any(|e| matches!(e,LifeEvent::Birth{id,parent:p,..} if id==child&&p==parent)),
                "hunter offspring does not match exact life birth parent/child"
            );
        }
    }
    Ok(children)
}

fn prey_counts(state: &WorldState) -> [u32; 9] {
    let mut counts = [0; 9];
    for (id, o) in state.organisms.iter() {
        if !state.hunters.contains(id) {
            counts[0] += 1;
            counts[1 + usize::from(o.phenotype.form).min(7)] += 1;
        }
    }
    counts
}

fn placement(seed: u64) -> HunterTarget {
    let i = seed - 1;
    HunterTarget {
        face: 4,
        u: (12 + 20 * (i % 3)) as f64,
        v: (8 + 16 * (i / 3)) as f64,
    }
}

impl Arm {
    fn new(opening: &WorldState, index: usize, dir: &Path) -> Result<Self> {
        fs::create_dir(dir)?;
        let mut world = World::from_state(opening.clone()).map_err(|e| anyhow!(e))?;
        let mut audit = audit::Audit::new(opening)?;
        let target = placement(opening.config.seed);
        let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
        if index >= 4 {
            profile = profile.facultative();
        }
        if index == 2 || index == 4 {
            profile = profile.without_attacks();
        }
        let mut founder = None;
        let initialized: Result<Value> = (|| {
            Ok(match index {
                0 => {
                    audit.initialize(&world.state, 0.0, 0.0, 0.0)?;
                    Value::Null
                }
                1 => {
                    let q = world
                        .deposit_hunter_budget_control(profile.clone(), target)
                        .map_err(|e| anyhow!(e))?;
                    audit.initialize(&world.state, q.material_in, q.energy_in, q.energy_heat)?;
                    let before = cubarium_core::snapshot::state_hash(&world.state);
                    ensure!(
                        world
                            .deposit_hunter_budget_control(profile.clone(), target)
                            .is_err(),
                        "repeat control accepted"
                    );
                    ensure!(
                        before == cubarium_core::snapshot::state_hash(&world.state),
                        "refused control mutated state"
                    );
                    serde_json::to_value(q)?
                }
                _ => {
                    let q = world
                        .start_hunter_trial(profile.clone(), target)
                        .map_err(|e| anyhow!(e))?;
                    founder = Some(q.id);
                    audit.initialize(&world.state, q.material_in, q.energy_in, 0.0)?;
                    let before = cubarium_core::snapshot::state_hash(&world.state);
                    ensure!(
                        world.start_hunter_trial(profile.clone(), target).is_err(),
                        "repeat founder accepted"
                    );
                    ensure!(
                        before == cubarium_core::snapshot::state_hash(&world.state),
                        "refused founder mutated state"
                    );
                    serde_json::to_value(q)?
                }
            })
        })();
        let receipt = match initialized {
            Ok(receipt) => receipt,
            Err(error) => {
                json_new(
                    &dir.join("initialization-failure.json"),
                    &json!({
                    "arm":NAMES[index],"error":format!("{error:#}"),"audit":audit.report,
                    "state_hash":cubarium_core::snapshot::state_hash(&world.state).to_string(),
                    "inventory":audit::Inventory::read(&world.state)}),
                )?;
                write_new(
                    &dir.join("initialization-failure.cubw"),
                    &encode_snapshot(&world.state, BUILD),
                )?;
                return Err(error);
            }
        };
        let snapshot = encode_snapshot(&world.state, BUILD);
        write_new(&dir.join("post-initialization.cubw"), &snapshot)?;
        json_new(
            &dir.join("opening.json"),
            &json!({"arm":NAMES[index], "target":target,
            "heading":founder.and_then(|id|world.state.organisms.get(id).map(|o|o.heading)),
            "config":world.config(), "profile":world.hunters().profile(),
            "profile_sha256":sha256(&serde_json::to_vec(&world.hunters().profile())?)?,
            "receipt":receipt, "pre_import_inventory":audit.report.opening,
            "post_import_inventory":audit::Inventory::read(&world.state),
            "post_snapshot_sha256":sha256(&snapshot)?,
            "post_state_hash":cubarium_core::snapshot::state_hash(&world.state).to_string()}),
        )?;
        let live = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| {
                (
                    id,
                    Lineage {
                        root: id,
                        depth: 0,
                        hunter: world.hunters().contains(id),
                        adult_seen: true,
                        reproduced: false,
                    },
                )
            })
            .collect();
        Ok(Self {
            world,
            audit,
            live,
            founder,
            founder_extinction: None,
            lineage_extinction: None,
            prey_min: opening.organisms.len(),
            prey_sum: 0,
            adult_bins: [0; 4],
            adults_max: 0,
            over_two_run: 0,
            over_two_longest: 0,
            zero_hunter_ticks: 0,
            captures: 0,
            offspring: 0,
            escrows: BTreeMap::new(),
            escrow_starts: 0,
            escrows_closed_without_birth: 0,
            adult_descendants: 0,
            descendant_parents: 0,
            last_complete_observer_tick: opening.tick,
            whole_recovery: recovery::WholeRecovery::new(opening.tick, &prey_counts(opening))
                .map_err(|e| anyhow!(e))?,
            events: stream(&dir.join("events.jsonl"))?,
            census: stream(&dir.join("census.jsonl"))?,
        })
    }

    fn step(&mut self, elapsed: u64, window: u64) -> Result<()> {
        let counters = self.world.step();
        let flows = (counters.light_in, counters.heat_out);
        self.world.check_invariants().map_err(|e| anyhow!(e))?;
        self.audit.observe(&self.world.state, flows.0, flows.1)?;
        let life = self.world.drain_events();
        let hunting = self.world.drain_hunter_events();
        ensure!(
            life.iter().all(|e| e.tick() == self.world.tick()),
            "life event has wrong settlement tick"
        );
        for event in &hunting {
            let tick = match event {
                HunterEvent::Attempt { tick, .. }
                | HunterEvent::Capture { tick, .. }
                | HunterEvent::Offspring { tick, .. }
                | HunterEvent::Death { tick, .. } => *tick,
            };
            ensure!(
                tick == self.world.tick(),
                "hunter event has wrong settlement tick"
            );
        }
        let mut captured = BTreeSet::new();
        let mut predation = BTreeSet::new();
        let children = offspring_children(&life, &hunting)?;
        // Resolve every birth before removing same-tick parents.
        for e in &life {
            if let LifeEvent::Birth { id, parent, .. } = e {
                let p = self
                    .live
                    .get(parent)
                    .context("unobserved birth parent")?
                    .clone();
                ensure!(
                    p.hunter == children.contains(id),
                    "hunter offspring/life birth mismatch"
                );
                if p.hunter {
                    let escrow = self
                        .escrows
                        .get(parent)
                        .context("hunter child lacks previously observed escrow")?;
                    ensure!(hunting.iter().any(|e|matches!(e,HunterEvent::Offspring{parent:p,child,..} if p==parent&&child==id)), "hunter parent/event mismatch");
                    line(
                        &mut self.events,
                        &json!({"stream":"observed_escrow_birth_link",
                        "tick":self.world.tick(),"parent":parent,"child":id,"prior_escrow":escrow,
                        "funding_evidence":"observed escrow-to-child linkage; exact parent debits and heat require core transfer tests"}),
                    )?;
                    if p.depth > 0 && !p.reproduced {
                        self.descendant_parents += 1;
                    }
                    self.live
                        .get_mut(parent)
                        .expect("observed parent")
                        .reproduced = true;
                }
                ensure!(
                    self.live
                        .insert(
                            *id,
                            Lineage {
                                root: p.root,
                                depth: p.depth + 1,
                                hunter: p.hunter,
                                adult_seen: false,
                                reproduced: false
                            }
                        )
                        .is_none(),
                    "duplicate birth"
                );
            }
        }
        for e in &life {
            if let LifeEvent::Death { id, cause, .. } = e {
                ensure!(self.live.remove(id).is_some(), "duplicate/unobserved death");
                if *cause == DeathCause::Predation {
                    ensure!(predation.insert(*id), "duplicate predation death");
                }
            }
            let mut row = serde_json::to_value(e)?;
            if let Some(n) = row["genome"].as_u64() {
                row["genome"] = json!(n.to_string());
            }
            line(&mut self.events, &json!({"stream":"life", "event":row}))?;
        }
        for e in &hunting {
            match e {
                HunterEvent::Capture { prey, .. } => {
                    ensure!(captured.insert(*prey), "prey captured twice");
                    self.captures += 1;
                }
                HunterEvent::Offspring { child, .. } => {
                    ensure!(
                        life.iter()
                            .any(|e| matches!(e,LifeEvent::Birth{id,..} if id==child)),
                        "offspring without birth"
                    );
                    self.offspring += 1;
                }
                _ => {}
            }
            line(&mut self.events, &json!({"stream":"hunter", "event":e}))?;
        }
        ensure!(captured == predation, "capture/predation death mismatch");
        ensure!(
            self.live.keys().copied().collect::<BTreeSet<_>>()
                == self
                    .world
                    .state
                    .organisms
                    .iter()
                    .map(|(id, _)| id)
                    .collect(),
            "live population/event mismatch"
        );
        for (id, l) in &self.live {
            ensure!(
                l.hunter == self.world.hunters().contains(*id),
                "lineage membership changed"
            );
        }
        let current_escrows: BTreeMap<_, _> = self
            .world
            .hunters()
            .members
            .iter()
            .filter_map(|m| {
                self.world
                    .state
                    .organisms
                    .get(m.id)
                    .and_then(|o| o.escrow.clone().map(|e| (m.id, e)))
            })
            .collect();
        for (parent, e) in &current_escrows {
            if self.escrows.get(parent).map(|old| old.started_tick) != Some(e.started_tick) {
                self.escrow_starts += 1;
                line(
                    &mut self.events,
                    &json!({"stream":"observed_escrow_start","tick":self.world.tick(),
                    "parent":parent,"escrow":e,"evidence":"post-step stored escrow, not isolated funding transaction"}),
                )?;
            }
        }
        for (parent, e) in &self.escrows {
            if current_escrows.get(parent).map(|now| now.started_tick) != Some(e.started_tick)
                && !hunting
                    .iter()
                    .any(|event| matches!(event,HunterEvent::Offspring{parent:p,..} if p==parent))
            {
                self.escrows_closed_without_birth += 1;
                line(
                    &mut self.events,
                    &json!({"stream":"observed_escrow_closed_without_birth",
                    "tick":self.world.tick(),"parent":parent,"prior_escrow":e,
                    "cause":"not separately exposed; may include death, cancellation or cap refusal"}),
                )?;
            }
        }
        self.escrows = current_escrows;
        let hunters = self.world.hunters().members.len();
        let prey = self.live.len() - hunters;
        let prey_census = prey_counts(&self.world.state);
        self.whole_recovery
            .observe_tick(self.world.tick(), &prey_census)
            .map_err(|e| anyhow!(e))?;
        // Full adult structure, not the presenter's historical 70% juvenile cue.
        let adults = self
            .world
            .hunters()
            .members
            .iter()
            .filter(|m| {
                let o = self
                    .world
                    .state
                    .organisms
                    .get(m.id)
                    .expect("validated member");
                o.structure + cubarium_core::hunter::TOLERANCE >= o.phenotype.structure_adult
            })
            .count();
        for m in &self.world.hunters().members {
            let o = self
                .world
                .state
                .organisms
                .get(m.id)
                .expect("validated member");
            let lineage = self.live.get_mut(&m.id).expect("observed member");
            if !lineage.adult_seen
                && o.structure + cubarium_core::hunter::TOLERANCE >= o.phenotype.structure_adult
            {
                lineage.adult_seen = true;
                if lineage.depth > 0 {
                    self.adult_descendants += 1;
                }
                line(
                    &mut self.events,
                    &json!({"stream":"observed_maturity","tick":self.world.tick(),
                    "id":m.id,"depth":lineage.depth,"structure":o.structure}),
                )?;
            }
        }
        self.prey_min = self.prey_min.min(prey);
        self.prey_sum += prey as u64;
        self.adult_bins[adults.min(3)] += 1;
        self.adults_max = self.adults_max.max(adults);
        self.over_two_run = if adults > 2 { self.over_two_run + 1 } else { 0 };
        self.over_two_longest = self.over_two_longest.max(self.over_two_run);
        if hunters == 0 {
            self.zero_hunter_ticks += 1;
        }
        if let Some(founder) = self.founder {
            if !self.live.contains_key(&founder) {
                self.founder_extinction.get_or_insert(self.world.tick());
            }
            if hunters == 0 {
                self.lineage_extinction.get_or_insert(self.world.tick());
            }
        }
        if elapsed % window == 0 {
            self.audit.close_window(&self.world.telemetry());
        }
        if elapsed % 200 == 0 {
            line(
                &mut self.census,
                &json!({"tick":self.world.tick(),"elapsed":elapsed,
                "prey":prey,"prey_by_form":&prey_census[1..],"hunters":hunters,"adults":adults,
                "juveniles":hunters-adults,"inventory":audit::Inventory::read(&self.world.state),
                "hunter_state":self.world.hunters(),"audit":self.audit.report}),
            )?;
        }
        self.last_complete_observer_tick = self.world.tick();
        Ok(())
    }

    fn finish(&mut self, dir: &Path, reason: &Option<String>, planned: u64) -> Result<Value> {
        self.audit.close_window(&self.world.telemetry());
        if reason.is_none() {
            self.audit.observe(&self.world.state, 0.0, 0.0)?;
        }
        self.events.flush()?;
        self.census.flush()?;
        self.events.get_ref().sync_all()?;
        self.census.get_ref().sync_all()?;
        let bytes = encode_snapshot(&self.world.state, BUILD);
        write_new(&dir.join("closing.cubw"), &bytes)?;
        let cohorts: BTreeSet<_> = self
            .live
            .values()
            .filter(|l| !l.hunter)
            .map(|l| l.root)
            .collect();
        let summary = json!({"planned_ticks":planned,"closing_tick":self.world.tick(),
            "termination":reason.as_deref().unwrap_or("planned_horizon"),
            "technical_complete":reason.is_none(),"complete_experiment_measurement":false,
            "remaining_measurements":["exact capture-position local recovery",
                "exact reproduction funding debits/heat and escrow closure cause (core evidence required)"],
            "whole_recovery_channels":"total prey, then forms0–7",
            "whole_recovery":self.whole_recovery.summary(reason.as_deref().unwrap_or("planned_horizon")),
            "audit":self.audit.report,"prey_min":self.prey_min,"prey_tick_integral":self.prey_sum,
            "adult_definition":"structure + core TOLERANCE >= decoded structure_adult",
            "adult_occupancy_ticks_0_1_2_over2":self.adult_bins,"adult_max":self.adults_max,
            "longest_over_two_ticks":self.over_two_longest,"zero_hunter_ticks":self.zero_hunter_ticks,
            "captures":self.captures,"offspring":self.offspring,"founder_extinction_tick":self.founder_extinction,
            "observed_escrow_starts":self.escrow_starts,"escrows_closed_without_birth":self.escrows_closed_without_birth,
            "adult_descendants":self.adult_descendants,"descendants_that_reproduced":self.descendant_parents,
            "lineage_extinction_tick":self.lineage_extinction,"surviving_opening_prey_cohorts":cohorts.len(),
            "maximum_live_descendant_depth":self.live.values().map(|l|l.depth).max().unwrap_or(0),
            "closing_population":self.world.state.organisms.len(),"closing_hunters":self.world.hunters().members.len(),
            "observer_live_count":self.live.len(),"last_complete_observer_tick":self.last_complete_observer_tick,
            "observer_statistics_trusted_through_closing_tick":reason.is_none(),
            "observer_failure_note":if reason.is_some(){Some("A failed step may have partly updated observer statistics; use event logs and the explicit last complete tick, not these aggregates for ecological conclusions.")}else{None},
            "closing_state_hash":cubarium_core::snapshot::state_hash(&self.world.state).to_string(),
            "closing_ecology_projection_hash":cubarium_core::ecology_hash(&self.world.state).to_string(),
            "closing_snapshot_sha256":sha256(&bytes)?});
        json_new(&dir.join("summary.json"), &summary)?;
        Ok(summary)
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        [20, 200].contains(&args.audit_window),
        "independent window must be20 or200 ticks"
    );
    let (cohort, openings) = load_cohort(&args.cohort)?;
    fs::create_dir(&args.out).context("output must be a NEW directory")?;
    let executable_path = std::env::current_exe()?;
    let executable = fs::read(&executable_path)?;
    write_new(&args.out.join("hunter_compare.frozen"), &executable)?;
    fs::set_permissions(
        args.out.join("hunter_compare.frozen"),
        fs::metadata(executable_path)?.permissions(),
    )?;
    json_new(
        &args.out.join("manifest.json"),
        &json!({"kind":"six-arm-hunter-comparison",
        "build":BUILD,"executable_sha256":sha256(&executable)?,"cohort":cohort,
        "ticks":args.ticks,"audit_window":args.audit_window,"arms":NAMES,
        "care":false,"resume_supported":false,"ancestry_basis":"aged opening cohorts, not original founders",
        "notes":"All twelve seeds retained. Initial implementation: resource/event/occupancy measurement; missing recovery/funding metrics are not claimed. Profile geometry remains unintegrated with art."}),
    )?;
    let mut all = Vec::new();
    let mut failed = false;
    for opening in openings {
        let seed = opening.state.config.seed;
        let dir = args.out.join(format!("seed-{seed}"));
        fs::create_dir(&dir)?;
        let mut arms: Vec<Arm> = Vec::new();
        let mut reason = None;
        for (i, name) in NAMES.iter().enumerate() {
            match Arm::new(&opening.state, i, &dir.join(name)) {
                Ok(arm) => arms.push(arm),
                Err(error) => {
                    reason = Some(format!("initialization_failure: {name}: {error:#}"));
                    failed = true;
                    break;
                }
            }
        }
        'ticks: for elapsed in 1..=if reason.is_none() { args.ticks } else { 0 } {
            for (index, arm) in arms.iter_mut().enumerate() {
                if let Err(e) = arm.step(elapsed, args.audit_window) {
                    reason = Some(format!(
                        "technical_failure: {} elapsed{elapsed}: {e:#}",
                        NAMES[index]
                    ));
                    failed = true;
                    break 'ticks;
                }
            }
        }
        let mut summaries = Vec::new();
        for (i, arm) in arms.iter_mut().enumerate() {
            match arm.finish(&dir.join(NAMES[i]), &reason, args.ticks) {
                Ok(summary) => summaries.push(summary),
                Err(error) => {
                    failed = true;
                    let error = format!("finalization_failure: {}: {error:#}", NAMES[i]);
                    reason.get_or_insert(error.clone());
                    summaries.push(json!({"technical_complete":false,"error":error,
                        "arm":NAMES[i],"audit":arm.audit.report,
                        "last_complete_observer_tick":arm.last_complete_observer_tick}));
                }
            }
        }
        let result = json!({"seed":seed,"arms":summaries,"failure":reason});
        json_new(&dir.join("result.json"), &result)?;
        all.push(result);
        eprintln!(
            "seed{seed}/12 complete: {}",
            reason.as_deref().unwrap_or("planned horizon")
        );
    }
    json_new(
        &args.out.join("summary.json"),
        &json!({"technical_complete":!failed,
        "complete_experiment_measurement":false,"seeds":all}),
    )?;
    ensure!(
        !failed,
        "one or more seeds failed; evidence retained in output directory"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn birth(parent: OrganismId, id: OrganismId) -> LifeEvent {
        LifeEvent::Birth {
            tick: 1,
            id,
            parent,
            parent_age_ticks: 1,
            parent_births: 1,
            genome: 0,
            origin: cubarium_core::organism::Origin::Descendant,
            mutations: Vec::new(),
        }
    }
    fn temp_parent() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cubarium-hunter-observer-tests-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
    #[test]
    fn offspring_reconciliation_rejects_duplicates_and_wrong_parents() {
        let parent = OrganismId {
            slot: 1,
            generation: 0,
        };
        let child = OrganismId {
            slot: 2,
            generation: 0,
        };
        let other = OrganismId {
            slot: 3,
            generation: 0,
        };
        let good = HunterEvent::Offspring {
            tick: 1,
            parent,
            child,
        };
        let life = [birth(parent, child)];
        assert!(offspring_children(&life, std::slice::from_ref(&good)).is_ok());
        assert!(offspring_children(&life, &[good.clone(), good]).is_err());
        assert!(
            offspring_children(
                &life,
                &[HunterEvent::Offspring {
                    tick: 1,
                    parent: other,
                    child
                }]
            )
            .is_err()
        );
    }
    #[test]
    fn failed_step_summary_separates_actual_state_from_partial_observer() {
        let opening = World::new(cubarium_core::WorldConfig::default())
            .unwrap()
            .state;
        let dir = temp_parent().join("arm");
        let mut arm = Arm::new(&opening, 0, &dir).unwrap();
        let id = arm.world.state.organisms.iter().next().unwrap().0;
        arm.world.state.organisms.remove(id);
        arm.world.state.tick += 1;
        let q = arm
            .finish(&dir, &Some("synthetic incomplete observer step".into()), 10)
            .unwrap();
        assert_eq!(q["closing_population"], opening.organisms.len() - 1);
        assert_eq!(q["observer_live_count"], opening.organisms.len());
        assert_eq!(q["last_complete_observer_tick"], 0);
        assert_eq!(q["observer_statistics_trusted_through_closing_tick"], false);
    }
    #[test]
    fn refused_founder_retains_initializer_failure_evidence() {
        let mut opening = World::new(cubarium_core::WorldConfig::default())
            .unwrap()
            .state;
        opening.config.capacity.max_organisms = opening.organisms.len() as u32;
        opening.config.founders.count = opening.organisms.len() as u32;
        let dir = temp_parent().join("arm");
        assert!(Arm::new(&opening, 2, &dir).is_err());
        let report: Value =
            serde_json::from_slice(&fs::read(dir.join("initialization-failure.json")).unwrap())
                .unwrap();
        assert_eq!(report["audit"]["passed"], false);
        assert!(dir.join("initialization-failure.cubw").is_file());
    }
    #[test]
    fn all_prescribed_placements_are_fixed_on_top_and_valid() {
        let points: Vec<_> = (1..=12).map(placement).collect();
        assert!(points.iter().all(|p| p.face == 4 && p.resolve().is_some()));
        assert_eq!((points[0].u, points[0].v), (12.0, 8.0));
        assert_eq!((points[11].u, points[11].v), (52.0, 56.0));
    }
    #[test]
    fn seventy_two_hour_horizon_is_accepted_without_truncation() {
        let a = Args::try_parse_from(["hunter_compare", "cohort", "out", "--ticks", "5184000"])
            .unwrap();
        assert_eq!(a.ticks, 5184000);
        assert!(
            Args::try_parse_from(["hunter_compare", "cohort", "out", "--ticks", "5184001"])
                .is_err()
        );
        assert!(Args::try_parse_from(["hunter_compare", "cohort", "out", "--ticks", "0"]).is_err());
    }
    #[test]
    fn checksum_uses_bytes_without_shell_interpretation() {
        assert_eq!(
            sha256(b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
