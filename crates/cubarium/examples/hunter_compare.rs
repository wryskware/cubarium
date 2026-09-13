//! Six matched, care-free arms over every prescribed aged seed. Never touches
//! the owning runner, live state, web endpoints or display transport.
//! Recovery uses exact pre-removal capture evidence and six worlds in lockstep.

#[path = "hunter_compare/audit.rs"]
mod audit;
#[path = "hunter_compare/eligibility.rs"]
mod eligibility;
#[path = "hunter_compare/recovery.rs"]
mod recovery;
#[path = "hunter_compare/reproduction.rs"]
mod reproduction;
#[path = "hunter_compare/spatial.rs"]
mod spatial;

use anyhow::{Context, Result, anyhow, ensure};
use clap::{Parser, ValueEnum};
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

/// Named, immutable experiment recipes. Never changes the owning runner's defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
enum ProfileVariant {
    #[default]
    Baseline,
    /// Only the reserve targets for acquiring/ending a hunt change to .80/.90.
    ReserveTargetsV1,
    /// [`ProfileVariant::ReserveTargetsV1`]'s background, and the paid-charging policy on top
    /// of it: semantic profile version 4, whose one meaning is a fixed 0.80 `E_max` oxidation
    /// activation threshold for authoritative members
    /// (`design/7_Research/astra-hunter-paid-charging-proposal-2026-09-13.md`). Its profile
    /// diff from `reserve-targets-v1` is the version number and nothing else.
    ReserveTargetsCharge80V1,
    /// [`ProfileVariant::ReserveTargetsCharge80V1`] and the size-aware growth-permission
    /// experiment on top of it: semantic profile version 5, which carries charge80's fixed
    /// 0.80 `E_max` oxidation activation *and* scales the growth permission threshold by the
    /// member's actual size
    /// (`design/7_Research/astra-hunter-size-aware-growth-proposal-2026-09-13.md`). Its profile
    /// diff from `reserve-targets-charge80-v1` is the version number and nothing else.
    ReserveTargetsCharge80SizeGateV1,
}

impl ProfileVariant {
    /// The reserve-target background this recipe runs on, if any.
    fn raises_reserve_targets(self) -> bool {
        matches!(
            self,
            ProfileVariant::ReserveTargetsV1
                | ProfileVariant::ReserveTargetsCharge80V1
                | ProfileVariant::ReserveTargetsCharge80SizeGateV1
        )
    }

    /// Whether this recipe runs the paid-charging policy, under whichever version carries it.
    fn charges(self) -> bool {
        matches!(
            self,
            ProfileVariant::ReserveTargetsCharge80V1
                | ProfileVariant::ReserveTargetsCharge80SizeGateV1
        )
    }
}

fn profile_for(
    config: &cubarium_core::WorldConfig,
    index: usize,
    variant: ProfileVariant,
) -> FixedHunterProfile {
    let mut profile = FixedHunterProfile::lanternjaw_trial(config);
    if variant.raises_reserve_targets() {
        profile.seek_reserve_fraction = 0.80;
        profile.perch_reserve_fraction = 0.90;
    }
    if variant == ProfileVariant::ReserveTargetsCharge80V1 {
        // The only change, and the only one there is: the semantic version. Every geometry,
        // cost, gate, target and stock is the one `reserve-targets-v1` already carried.
        profile = profile.charge80();
    }
    if variant == ProfileVariant::ReserveTargetsCharge80SizeGateV1 {
        // Likewise: version 5 is charge80's fixed activation plus the size-aware growth
        // permission, and no other serialized field moves.
        profile = profile.size_gate();
    }
    if index >= 4 {
        profile = profile.facultative();
    }
    if index == 2 || index == 4 {
        // An attack-disabled control takes its recipe's charging policy too: it is there to
        // expose starvation and intake without predation, and a control that quietly ran the
        // other policy would not be a control for this experiment.
        profile = profile.without_attacks();
    }
    profile
}

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
    /// Explicit experiment recipe; baseline preserves the original fixed profile.
    #[arg(long, value_enum, default_value_t = ProfileVariant::Baseline)]
    profile: ProfileVariant,
    /// Restrict the run to these cohort seeds, for a pilot. Omitted means all twelve, which is
    /// the only form that can be a cohort result: a restricted run records the seeds it ran and
    /// marks itself `seed_subset_pilot`, so a partial collection can never be read as one.
    #[arg(long, value_delimiter = ',')]
    seeds: Vec<u64>,
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

fn measurement_complete(
    planned: u64,
    elapsed: u64,
    uninterrupted: bool,
    observers_current: bool,
) -> bool {
    // A short smoke can pass technically without completing even the first
    // prescribed biological observation horizon. This flag certifies data,
    // never viability, parameter selection, or approval to deploy.
    planned >= 144000 && elapsed == planned && uninterrupted && observers_current
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
    reproduction: reproduction::ReproductionAudit,
    adult_descendants: u64,
    descendant_parents: u64,
    last_complete_observer_tick: u64,
    whole_recovery: recovery::WholeRecovery,
    capture_audit: spatial::CaptureAudit,
    eligibility: eligibility::Eligibility,
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

/// The bounded oxidation diagnostics for one arm, with their scope stated in the record rather
/// than left to a reader's assumption.
///
/// These are totals over **this arm's whole run** — from its post-initialization state, which is
/// where its `World` value was built, to whatever tick it stopped at. They count only the member
/// oxidation transactions that the world's configured threshold would not have permitted, so an
/// arm whose resolved member threshold equals the configured one reports zeros by construction,
/// not by luck. The core never persists them, so nothing here survives a restart and nothing
/// here can reach a snapshot or a hash.
fn oxidation_json(world: &World) -> Value {
    let d = world.charging_diagnostics();
    json!({
        "member_threshold":world.member_oxidation_threshold(),
        "world_threshold":world.config().organism.oxidation_threshold,
        "charging_above_reference":{
            "transactions":d.extra_transactions,
            "reserve_burned":d.extra_reserve_burned,
            "energy_gained":d.extra_energy_gained,
            "conversion_heat":d.extra_heat,
        },
        "scope":"member oxidation transactions above the world's configured threshold, over this arm's whole run; never persisted, never hashed",
    })
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
    #[cfg(test)]
    fn new(opening: &WorldState, index: usize, dir: &Path) -> Result<Self> {
        Self::new_with_profile(opening, index, dir, ProfileVariant::Baseline)
    }

    fn new_with_profile(
        opening: &WorldState,
        index: usize,
        dir: &Path,
        variant: ProfileVariant,
    ) -> Result<Self> {
        fs::create_dir(dir)?;
        let mut world = World::from_state(opening.clone()).map_err(|e| anyhow!(e))?;
        let mut audit = audit::Audit::new(opening)?;
        let target = placement(opening.config.seed);
        let profile = profile_for(world.config(), index, variant);
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
            &json!({"arm":NAMES[index], "target":target,"profile_recipe":variant,
            // The resolved policy, beside the version that selects it: a reader should not have
            // to know what version 4 means to know what this arm actually ran. Two numbers,
            // because they are two different facts — what the recipe carries, and what this
            // arm's world resolves. The untouched arm installs no profile, so its world
            // resolves the configured threshold however the recipe is labelled.
            "recipe_oxidation_policy":profile.oxidation_policy().as_str(),
            "recipe_oxidation_threshold":profile.oxidation_threshold(&world.config().organism),
            "world_member_oxidation_threshold":world.member_oxidation_threshold(),
            "world_oxidation_threshold":world.config().organism.oxidation_threshold,
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
        let capture_audit = spatial::CaptureAudit::new(&world.state);
        let reproduction = reproduction::ReproductionAudit::new(&world.state)?;
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
            reproduction,
            adult_descendants: 0,
            descendant_parents: 0,
            last_complete_observer_tick: opening.tick,
            whole_recovery: recovery::WholeRecovery::new(opening.tick, &prey_counts(opening))
                .map_err(|e| anyhow!(e))?,
            capture_audit,
            eligibility: eligibility::Eligibility::default(),
            events: stream(&dir.join("events.jsonl"))?,
            census: stream(&dir.join("census.jsonl"))?,
        })
    }

    fn step(&mut self, index: usize, elapsed: u64, window: u64) -> Result<Vec<recovery::Capture>> {
        let counters = self.world.step();
        let flows = (counters.light_in, counters.heat_out);
        self.world.check_invariants().map_err(|e| anyhow!(e))?;
        self.audit.observe(&self.world.state, flows.0, flows.1)?;
        let life = self.world.drain_events();
        let hunting = self.world.drain_hunter_events();
        self.reproduction
            .observe(&hunting, &life, &self.world.state)?;
        let captures = self
            .capture_audit
            .observe(index, &hunting, &self.world.state)?;
        ensure!(
            life.iter().all(|e| e.tick() == self.world.tick()),
            "life event has wrong settlement tick"
        );
        for event in &hunting {
            ensure!(
                event.tick() == self.world.tick(),
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
                    ensure!(hunting.iter().any(|e|matches!(e,HunterEvent::Offspring{parent:p,child,..} if p==parent&&child==id)), "hunter parent/event mismatch");
                    // The reproduction audit already matched this exact child
                    // to its funded key, both event streams and closing stocks.
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
        self.eligibility.observe(&self.world.state);
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
                "hunter_state":self.world.hunters(),"hunter_stocks":eligibility::members(&self.world.state),
                "audit":self.audit.report}),
            )?;
        }
        self.last_complete_observer_tick = self.world.tick();
        Ok(captures)
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
        let measured = measurement_complete(
            planned,
            self.world
                .tick()
                .saturating_sub(self.audit.report.opening_tick),
            reason.is_none() && self.audit.report.passed,
            self.last_complete_observer_tick == self.world.tick()
                && self.reproduction.last_complete_tick() == self.world.tick(),
        );
        let remaining = if measured {
            Vec::new()
        } else {
            vec![
                "requires at least two elapsed hours with uninterrupted, audited observer coverage",
            ]
        };
        let summary = json!({"planned_ticks":planned,"closing_tick":self.world.tick(),
            "termination":reason.as_deref().unwrap_or("planned_horizon"),
            "technical_complete":reason.is_none(),"complete_experiment_measurement":measured,
            "remaining_measurements":remaining,"biological_acceptance":"not assessed by the measurement flag",
            "local_recovery":"paired seed-level local-recovery.jsonl; exact settlement prey position and paid attempt key",
            "whole_recovery_channels":"total prey, then forms0–7",
            "whole_recovery":self.whole_recovery.summary(reason.as_deref().unwrap_or("planned_horizon")),
            "audit":self.audit.report,"prey_min":self.prey_min,"prey_tick_integral":self.prey_sum,
            "adult_definition":"structure + core TOLERANCE >= decoded structure_adult",
            "oxidation":oxidation_json(&self.world),
            "reproductive_opportunity":self.eligibility.summary(),
            "adult_occupancy_ticks_0_1_2_over2":self.adult_bins,"adult_max":self.adults_max,
            "longest_over_two_ticks":self.over_two_longest,"zero_hunter_ticks":self.zero_hunter_ticks,
            "captures":self.captures,"offspring":self.offspring,"founder_extinction_tick":self.founder_extinction,
            "reproduction_audit":self.reproduction.summary(),
            "open_gestations":self.reproduction.open_gestations(),
            "last_complete_reproduction_tick":self.reproduction.last_complete_tick(),
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

/// One bounded local observer per six-arm seed, never six unrelated local histories.
struct PairedLocal {
    recovery: recovery::LocalRecovery,
    neighborhoods: Vec<Vec<usize>>,
    opening: u64,
    last_entered_tick: u64,
    last_complete_tick: u64,
    records: BufWriter<File>,
    exposures: BufWriter<File>,
}

fn paired_states(arms: &[Arm]) -> Result<[&WorldState; recovery::ARMS]> {
    ensure!(
        arms.len() == recovery::ARMS,
        "local recovery requires all six arms"
    );
    let states: [&WorldState; recovery::ARMS] = std::array::from_fn(|a| &arms[a].world.state);
    ensure!(
        states.iter().all(|s| s.tick == states[0].tick),
        "paired local census has unequal world ticks"
    );
    Ok(states)
}

impl PairedLocal {
    fn new(arms: &[Arm], dir: &Path) -> Result<Self> {
        let states = paired_states(arms)?;
        let tick = states[0].tick;
        let neighborhoods = spatial::neighborhoods();
        let mut recovery =
            recovery::LocalRecovery::new(tick, neighborhoods.clone(), spatial::ON_ARMS.to_vec())
                .map_err(|e| anyhow!(e))?;
        recovery
            .observe_sample(tick, &spatial::counts(states))
            .map_err(|e| anyhow!(e))?;
        Ok(Self {
            recovery,
            neighborhoods,
            opening: tick,
            last_entered_tick: tick,
            last_complete_tick: tick,
            records: stream(&dir.join("local-recovery.jsonl"))?,
            exposures: stream(&dir.join("local-exposures.jsonl"))?,
        })
    }

    fn emit(&mut self, records: Vec<recovery::LocalRecord>) -> Result<()> {
        for record in records {
            line(&mut self.records, &serde_json::to_value(record)?)?;
        }
        Ok(())
    }

    fn observe(&mut self, arms: &[Arm], captures: &[recovery::Capture]) -> Result<()> {
        let states = paired_states(arms)?;
        let tick = states[0].tick;
        ensure!(
            tick == self.last_complete_tick + 1,
            "paired local observation skipped a tick"
        );
        self.last_entered_tick = tick;
        if !captures.is_empty() || (tick - self.opening) % recovery::CADENCE == 0 {
            let counts = spatial::counts(states);
            if !captures.is_empty() {
                for capture in captures {
                    let cells = &self.neighborhoods[capture.cell];
                    let forms = spatial::local_forms(states, cells);
                    let totals = forms.map(|f| f.iter().sum::<u32>());
                    line(
                        &mut self.exposures,
                        &json!({"tick":tick,"capture":capture,"cells":cells,
                        "post_step_prey":totals,"post_step_prey_by_form":forms,
                        "basis":"same cells in all six arms at end of settlement tick; not instantaneous pre/post-removal counts"}),
                    )?;
                }
                let records = self
                    .recovery
                    .record_captures(tick, captures, &counts)
                    .map_err(|e| anyhow!(e))?;
                self.emit(records)?;
            }
            // Strictly after all captures, including a capture on the census boundary.
            if (tick - self.opening) % recovery::CADENCE == 0 {
                let records = self
                    .recovery
                    .observe_sample(tick, &counts)
                    .map_err(|e| anyhow!(e))?;
                self.emit(records)?;
            }
        }
        self.last_complete_tick = tick;
        Ok(())
    }

    fn finish(&mut self, reason: Option<&str>) -> Result<Value> {
        let records = self
            .recovery
            .finish(self.last_entered_tick, reason.unwrap_or("planned_horizon"))
            .map_err(|e| anyhow!(e))?;
        self.emit(records)?;
        for file in [&mut self.records, &mut self.exposures] {
            file.flush()?;
            file.get_ref().sync_all()?;
        }
        Ok(
            json!({"available":true,"technical_complete":reason.is_none(),
            "last_complete_paired_tick":self.last_complete_tick,"last_entered_tick":self.last_entered_tick,
            "statistics_trusted_through_closing_tick":reason.is_none(),
            "termination":reason.unwrap_or("planned_horizon"),
            "captures_seen":self.recovery.captures_seen,"selected_captures":self.recovery.selected_captures,
            "unselected_captures":std::array::from_fn::<_,6,_>(|a|self.recovery.captures_seen[a]-self.recovery.selected_captures[a]),
            "graph_radius":3,"sample_ticks":recovery::CADENCE,"capture_bin_ticks":recovery::CAPTURE_BIN,
            "window_horizon_ticks":recovery::LOCAL_HORIZON,"retained_samples":self.recovery.retained_samples(),
            "active_windows_after_close":self.recovery.active_windows(),
            "records":"local-recovery.jsonl","all_capture_exposures":"local-exposures.jsonl",
            "limits":"Treatment-selected locations. Milestone counts are cadence-observed total prey; per-form counts are immediate end-of-step exposure counts, not per-form recovery windows. Any failed observer step may have partially updated statistics."}),
        )
    }
}

/// What this recipe changes from the original fixed profile, in words, for the manifest. A
/// reader must not have to infer the scope of an experiment from a version number.
fn recipe_scope(variant: ProfileVariant) -> &'static str {
    match variant {
        ProfileVariant::Baseline => "baseline preserves the original fixed profile exactly",
        ProfileVariant::ReserveTargetsV1 => "reserve-targets-v1 changes only seek/perch reserve fractions to .80/.90; all costs, reproductive gates, geometry, imports and placements unchanged",
        ProfileVariant::ReserveTargetsCharge80V1 => "reserve-targets-charge80-v1 is reserve-targets-v1 (seek/perch .80/.90) with semantic profile version 3 raised to 4 and nothing else; version 4's one meaning is a fixed 0.80 E_max oxidation activation threshold for authoritative members at every age and phase. The conversion block, costs, reproductive gates, geometry, imports and placements are unchanged, and ordinary organisms keep the world's configured threshold",
        ProfileVariant::ReserveTargetsCharge80SizeGateV1 => "reserve-targets-charge80-size-gate-v1 is reserve-targets-charge80-v1 with semantic profile version 4 raised to 5 and nothing else; version 5 keeps charge80's fixed 0.80 E_max oxidation activation for authoritative members AND scales their growth permission threshold by actual size, gate = growth_reserve_min * reserve_max * clamp(structure/structure_adult, 0, 1), at the existing post-oxidation growth site. The gate is a precondition only: the increment's rate, remaining-structure, reserve and battery caps, its reserve and battery debits and its construction heat are unchanged, nothing is capped to reserve minus gate, no reserve floor is protected, and ordinary organisms and profile versions 3 and 4 keep the original growth_reserve_min * reserve_max expression",
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        [20, 200].contains(&args.audit_window),
        "independent window must be20 or200 ticks"
    );
    let (cohort, openings) = load_cohort(&args.cohort)?;
    // Resolved from the cohort's **own** configs, so the manifest records the number this run
    // actually applied rather than a constant repeated by hand. Every opening must agree, or
    // one manifest line could not honestly describe all twelve.
    let world_threshold = openings
        .first()
        .context("a cohort has at least one opening")?
        .state
        .config
        .organism
        .oxidation_threshold;
    ensure!(
        openings.iter().all(|o| o.state.config.organism.oxidation_threshold == world_threshold),
        "the cohort's openings disagree about the configured oxidation threshold"
    );
    let resolved = {
        let config = &openings[0].state.config;
        let p = profile_for(config, 3, args.profile);
        (p.oxidation_policy().as_str(), p.oxidation_threshold(&config.organism))
    };
    // A seed restriction is a pilot, and the manifest says so in its own field rather than
    // leaving a reader to infer completeness from how many directories exist.
    let all_seeds: Vec<u64> = openings.iter().map(|o| o.state.config.seed).collect();
    for seed in &args.seeds {
        ensure!(all_seeds.contains(seed), "seed {seed} is not in this cohort");
    }
    let ran_seeds: Vec<u64> =
        if args.seeds.is_empty() { all_seeds.clone() } else { args.seeds.clone() };
    let openings: Vec<Opening> = openings
        .into_iter()
        .filter(|o| ran_seeds.contains(&o.state.config.seed))
        .collect();
    ensure!(!openings.is_empty(), "no seed selected");
    let seed_subset_pilot = ran_seeds.len() != all_seeds.len();

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
        "profile_recipe":args.profile,
        "observer_contract":"exact-reproduction-v2; bounded branch-checked transactions and boundary-stock diagnostics",
        "profile_recipe_scope":recipe_scope(args.profile),
        "member_oxidation_policy":resolved.0,"member_oxidation_threshold":resolved.1,
        "world_oxidation_threshold":world_threshold,
        "oxidation_observer":"charging_above_reference; per-arm running totals of the member oxidation transactions the world's configured threshold would not have permitted, over that arm's whole run from its post-initialization state to its closing tick. Counts transactions, reserve burned, battery gained and conversion heat only; no per-tick history, no RNG draw, no world state read or written for measurement. Zero by construction under any recipe whose resolved member threshold equals the configured one.",
        "care":false,"resume_supported":false,"ancestry_basis":"aged opening cohorts, not original founders",
        "cohort_seeds":all_seeds,"ran_seeds":ran_seeds,"seed_subset_pilot":seed_subset_pilot,
        "seed_subset_note":if seed_subset_pilot {"A SEED SUBSET. This is a pilot, not a cohort result: the unrun seeds are not absent observations, they were never started."} else {"every cohort seed ran"},
        "notes":"All selected seeds retained. Exact paid capture evidence drives paired local recovery; reproduction quantities come from core mutation records with independently checked identities. Measurement completion never implies biological acceptance. Renderer is not part of this headless trial."}),
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
            match Arm::new_with_profile(&opening.state, i, &dir.join(name), args.profile) {
                Ok(arm) => arms.push(arm),
                Err(error) => {
                    reason = Some(format!("initialization_failure: {name}: {error:#}"));
                    failed = true;
                    break;
                }
            }
        }
        let mut local = if reason.is_none() {
            match PairedLocal::new(&arms, &dir) {
                Ok(local) => Some(local),
                Err(error) => {
                    reason = Some(format!("local_initialization_failure: {error:#}"));
                    failed = true;
                    None
                }
            }
        } else {
            None
        };
        'ticks: for elapsed in 1..=if reason.is_none() { args.ticks } else { 0 } {
            let mut captures = Vec::new();
            for (index, arm) in arms.iter_mut().enumerate() {
                match arm.step(index, elapsed, args.audit_window) {
                    Ok(batch) => captures.extend(batch),
                    Err(e) => {
                        reason = Some(format!(
                            "technical_failure: {} elapsed{elapsed}: {e:#}",
                            NAMES[index]
                        ));
                        failed = true;
                        break 'ticks;
                    }
                }
            }
            if let Err(e) = local
                .as_mut()
                .expect("initialized paired observer")
                .observe(&arms, &captures)
            {
                reason = Some(format!("local_observer_failure: elapsed{elapsed}: {e:#}"));
                failed = true;
                break;
            }
        }
        let local_summary = match local.as_mut().map(|l| l.finish(reason.as_deref())) {
            None => json!({"available":false,"technical_complete":false,"reason":reason}),
            Some(Ok(summary)) => summary,
            Some(Err(error)) => {
                failed = true;
                let error = format!("local_finalization_failure: {error:#}");
                reason.get_or_insert(error.clone());
                json!({"available":false,"technical_complete":false,"error":error})
            }
        };
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
        let result =
            json!({"seed":seed,"arms":summaries,"local_recovery":local_summary,"failure":reason});
        json_new(&dir.join("result.json"), &result)?;
        all.push(result);
        eprintln!(
            "seed{seed}/12 complete: {}",
            reason.as_deref().unwrap_or("planned horizon")
        );
    }
    let measured = !failed
        && all.iter().all(|seed| {
            seed["arms"].as_array().is_some_and(|arms| {
                arms.len() == 6
                    && arms
                        .iter()
                        .all(|arm| arm["complete_experiment_measurement"] == true)
            })
        });
    json_new(
        &args.out.join("summary.json"),
        &json!({"technical_complete":!failed,
        "complete_experiment_measurement":measured,"seeds":all}),
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
    #[test]
    fn measurement_flag_requires_the_full_horizon_and_uninterrupted_observers() {
        assert!(!measurement_complete(6000, 6000, true, true));
        assert!(!measurement_complete(144000, 6000, true, true));
        assert!(!measurement_complete(144000, 144000, false, true));
        assert!(!measurement_complete(144000, 144000, true, false));
        assert!(measurement_complete(144000, 144000, true, true));
        assert!(measurement_complete(5184000, 5184000, true, true));
    }
    #[test]
    fn reserve_target_recipe_changes_only_its_two_named_fields() {
        let config = cubarium_core::WorldConfig::default();
        for index in 0..6 {
            let baseline = profile_for(&config, index, ProfileVariant::Baseline);
            let candidate = profile_for(&config, index, ProfileVariant::ReserveTargetsV1);
            assert_eq!(baseline.seek_reserve_fraction, 0.35);
            assert_eq!(baseline.perch_reserve_fraction, 0.65);
            let mut expected = baseline.clone();
            expected.seek_reserve_fraction = 0.80;
            expected.perch_reserve_fraction = 0.90;
            assert_eq!(candidate, expected);
            baseline.validate().unwrap();
            candidate.validate().unwrap();
            assert_eq!(candidate.attacks_enabled, index != 2 && index != 4);
            assert_eq!(
                candidate.scavenge_fraction,
                if index >= 4 { 0.25 } else { 0.0 }
            );
        }
    }

    /// The candidate's profile diff from `reserve-targets-v1` is the semantic version and
    /// nothing else — on every arm, including the two attack-disabled controls.
    #[test]
    fn the_charge80_recipe_changes_only_the_semantic_version_of_reserve_targets() {
        let config = cubarium_core::WorldConfig::default();
        for index in 0..6 {
            let background = profile_for(&config, index, ProfileVariant::ReserveTargetsV1);
            let candidate = profile_for(&config, index, ProfileVariant::ReserveTargetsCharge80V1);
            assert_eq!(background.version, 3);
            assert_eq!(candidate.version, 4);
            let mut expected = background.clone();
            expected.version = 4;
            assert_eq!(candidate, expected, "arm {index} changed a field other than `version`");
            // The reserve-target background really is the fixed .80/.90 one.
            assert_eq!(candidate.seek_reserve_fraction, 0.80);
            assert_eq!(candidate.perch_reserve_fraction, 0.90);
            background.validate().unwrap();
            candidate.validate().unwrap();
            // And the one thing it resolves to.
            assert_eq!(
                background.oxidation_threshold(&config.organism),
                config.organism.oxidation_threshold
            );
            assert_eq!(candidate.oxidation_threshold(&config.organism), 0.80);
            assert_eq!(candidate.oxidation_policy().as_str(), "fixed-member-threshold");
            // Attack-off controls take the candidate policy too; that is the whole point of
            // running them under the same recipe.
            assert_eq!(candidate.attacks_enabled, index != 2 && index != 4);
            assert_eq!(candidate.scavenge_fraction, if index >= 4 { 0.25 } else { 0.0 });
        }
    }

    /// Every recipe states its own scope, and the strings are distinct: a manifest line is how a
    /// later reader learns what was varied.
    #[test]
    fn every_recipe_states_a_distinct_scope() {
        let all = [
            ProfileVariant::Baseline,
            ProfileVariant::ReserveTargetsV1,
            ProfileVariant::ReserveTargetsCharge80V1,
        ];
        let scopes: Vec<&str> = all.iter().map(|v| recipe_scope(*v)).collect();
        for (i, a) in scopes.iter().enumerate() {
            assert!(!a.is_empty());
            for b in &scopes[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert!(recipe_scope(ProfileVariant::ReserveTargetsCharge80V1).contains("0.80 E_max"));
        assert!(recipe_scope(ProfileVariant::ReserveTargetsCharge80V1).contains("version 3 raised to 4"));
    }

    #[test]
    fn recipe_selection_is_explicit_and_unknown_variants_are_refused() {
        let baseline = Args::try_parse_from(["compare", "cohort", "out"]).unwrap();
        assert_eq!(baseline.profile, ProfileVariant::Baseline);
        let candidate = Args::try_parse_from([
            "compare",
            "cohort",
            "out",
            "--profile",
            "reserve-targets-v1",
        ])
        .unwrap();
        assert_eq!(candidate.profile, ProfileVariant::ReserveTargetsV1);
        let charge = Args::try_parse_from([
            "compare",
            "cohort",
            "out",
            "--profile",
            "reserve-targets-charge80-v1",
        ])
        .unwrap();
        assert_eq!(charge.profile, ProfileVariant::ReserveTargetsCharge80V1);
        let size = Args::try_parse_from([
            "compare",
            "cohort",
            "out",
            "--profile",
            "reserve-targets-charge80-size-gate-v1",
        ])
        .unwrap();
        assert_eq!(size.profile, ProfileVariant::ReserveTargetsCharge80SizeGateV1);
        assert!(Args::try_parse_from(["compare", "cohort", "out", "--profile", "rescue"]).is_err());
        assert!(Args::try_parse_from(["compare", "cohort", "out", "--profile", "charge80"]).is_err());
        assert!(
            Args::try_parse_from(["compare", "cohort", "out", "--profile", "size-gate"]).is_err()
        );
    }

    /// The size-gate recipe's profile diff from charge80 is the version and nothing else, in
    /// every arm — including the attack-disabled controls, which take the policy too.
    #[test]
    fn the_size_gate_recipe_differs_from_charge80_only_by_its_semantic_version() {
        let config = cubarium_core::WorldConfig::default();
        for index in 0..6 {
            let charge = profile_for(&config, index, ProfileVariant::ReserveTargetsCharge80V1);
            let size = profile_for(&config, index, ProfileVariant::ReserveTargetsCharge80SizeGateV1);
            assert_eq!(charge.version, cubarium_core::hunter::PROFILE_VERSION_CHARGE80);
            assert_eq!(size.version, cubarium_core::hunter::PROFILE_VERSION_SIZE_GATE);
            let mut expected = charge.clone();
            expected.version = size.version;
            assert_eq!(size, expected, "arm {index}: a field other than `version` moved");
            // Both recipes charge; the size-gate version must not silently drop the policy.
            assert_eq!(size.oxidation_policy(), charge.oxidation_policy());
            assert!(ProfileVariant::ReserveTargetsCharge80SizeGateV1.charges());
            // The background it runs on is unchanged.
            assert_eq!(size.seek_reserve_fraction, 0.80);
            assert_eq!(size.perch_reserve_fraction, 0.90);
        }
    }

    /// Installing the size-gate recipe changes no opening body, field or import: the recipe is
    /// a policy selector, not a different world.
    #[test]
    fn the_size_gate_recipe_changes_no_founder_inventory_or_opening_body() {
        let opening = World::new(cubarium_core::WorldConfig::default())
            .unwrap()
            .state;
        let dir = temp_parent();
        for index in 0..6 {
            let charge = Arm::new_with_profile(
                &opening,
                index,
                &dir.join(format!("charge-{index}")),
                ProfileVariant::ReserveTargetsCharge80V1,
            )
            .unwrap();
            let size = Arm::new_with_profile(
                &opening,
                index,
                &dir.join(format!("size-{index}")),
                ProfileVariant::ReserveTargetsCharge80SizeGateV1,
            )
            .unwrap();
            assert_eq!(
                charge.world.state.organisms, size.world.state.organisms,
                "arm {index}: a body differs at the opening"
            );
            assert_eq!(charge.world.state.fields, size.world.state.fields, "arm {index}");
            assert_eq!(
                charge.world.state.external_material_in, size.world.state.external_material_in,
                "arm {index}: imported material differs"
            );
            assert_eq!(
                charge.world.state.hunters.founder_energy_in,
                size.world.state.hunters.founder_energy_in,
                "arm {index}: imported energy differs"
            );
            assert_eq!(
                charge.world.state.hunters.founder_material_in,
                size.world.state.hunters.founder_material_in,
                "arm {index}"
            );
        }
    }

    #[test]
    fn candidate_initializer_does_not_add_extra_food_or_change_any_opening_body() {
        let opening = World::new(cubarium_core::WorldConfig::default())
            .unwrap()
            .state;
        let dir = temp_parent();
        for index in 0..6 {
            let baseline =
                Arm::new(&opening, index, &dir.join(format!("baseline-{index}"))).unwrap();
            let candidate = Arm::new_with_profile(
                &opening,
                index,
                &dir.join(format!("candidate-{index}")),
                ProfileVariant::ReserveTargetsV1,
            )
            .unwrap();
            assert_eq!(
                baseline.world.state.organisms,
                candidate.world.state.organisms
            );
            assert_eq!(baseline.world.state.fields, candidate.world.state.fields);
            assert_eq!(
                baseline.audit.report.actual_receipt_material,
                candidate.audit.report.actual_receipt_material
            );
            assert_eq!(
                baseline.audit.report.actual_receipt_energy,
                candidate.audit.report.actual_receipt_energy
            );
            assert_eq!(
                baseline.audit.report.actual_receipt_heat,
                candidate.audit.report.actual_receipt_heat
            );
            assert_eq!(baseline.world.state.config, candidate.world.state.config);
        }
    }
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
    fn paired_local_rejects_a_partly_advanced_seed_and_preserves_coverage() {
        let opening = World::new(cubarium_core::WorldConfig::default())
            .unwrap()
            .state;
        let dir = temp_parent();
        let mut arms: Vec<_> = NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| Arm::new(&opening, i, &dir.join(name)).unwrap())
            .collect();
        let mut local = PairedLocal::new(&arms, &dir).unwrap();
        arms[0].step(0, 1, 200).unwrap();
        assert!(local.observe(&arms, &[]).is_err());
        assert_eq!(local.last_entered_tick, opening.tick);
        let summary = local.finish(Some("synthetic partial-arm failure")).unwrap();
        assert_eq!(summary["last_complete_paired_tick"], opening.tick);
        assert_eq!(summary["technical_complete"], false);
        assert_eq!(summary["statistics_trusted_through_closing_tick"], false);
    }

    #[test]
    fn paired_local_streams_capture_boundary_forms_and_censored_followup() {
        let opening = World::new(cubarium_core::WorldConfig::default())
            .unwrap()
            .state;
        let dir = temp_parent();
        let mut arms: Vec<_> = NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| Arm::new(&opening, i, &dir.join(name)).unwrap())
            .collect();
        let mut local = PairedLocal::new(&arms, &dir).unwrap();
        let cell =
            cubarium_surface::cell_of(&opening.organisms.iter().next().unwrap().1.pos).index();
        // Deliberate synthetic exposure to test orchestration/file ordering at
        // exactly a census boundary; spatial's separate fixture uses a REAL kill.
        let capture = recovery::Capture {
            arm: 3,
            cell,
            id: recovery::CaptureId {
                hunter_slot: 123,
                hunter_generation: 4,
                attempt: 5,
            },
        };
        for elapsed in 1..=400 {
            let mut batch = Vec::new();
            for (i, arm) in arms.iter_mut().enumerate() {
                batch.extend(arm.step(i, elapsed, 200).unwrap());
            }
            assert!(batch.is_empty(), "this stream fixture unexpectedly hunted");
            if elapsed == 200 {
                batch.push(capture.clone());
            }
            local.observe(&arms, &batch).unwrap();
        }
        let summary = local.finish(None).unwrap();
        assert_eq!(summary["last_complete_paired_tick"], 400);
        assert_eq!(summary["captures_seen"][3], 1);
        assert_eq!(summary["selected_captures"][3], 1);
        assert_eq!(summary["active_windows_after_close"], 0);
        let exposures = fs::read_to_string(dir.join("local-exposures.jsonl")).unwrap();
        let exposure: Value = serde_json::from_str(exposures.trim()).unwrap();
        assert_eq!(exposure["tick"], 200);
        assert_eq!(exposure["capture"]["id"]["hunter_generation"], 4);
        for i in 0..6 {
            let total: u64 = exposure["post_step_prey_by_form"][i]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .sum();
            assert_eq!(exposure["post_step_prey"][i], total);
        }
        let output = fs::read_to_string(dir.join("local-recovery.jsonl")).unwrap();
        let record: Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(record["capture_tick"], 200);
        assert_eq!(
            record["pre_samples"], 1,
            "same-tick census must not enter prehistory"
        );
        assert_eq!(record["status"], "insufficient_pre");
        assert_eq!(record["closed_tick"], 400);
        assert_eq!(record["termination_reason"], "planned_horizon");
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
