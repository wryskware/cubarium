//! **The episode initializer** (`design/community-assembly-2026-10-03.md` revisions 2–3,
//! `design/handoffs/assembly-step1-2026-10-03.md`): one path every grid and search
//! evaluation starts through.
//!
//! - **E0** is today's founding, under the candidate's own ecology ([`StartSet::E0`]).
//! - **A start library** ([`StartLibrary`]) holds saved starts, one directory per seed
//!   (`seed-<n>/`), each in the whole-world save layout `cubarium voxel --load DIR` reads
//!   (`world-<tick>.voxel`, `flora-<tick>.bin`, `fauna-<tick>.bin`) plus its
//!   [`Manifest`] (`start.toml`) and its recipe's provenance (`recipe.toml`).
//!
//! Loading a start ([`load_start`]) checks it against the run ([`Expect`]) and **rejects,
//! never repairs**: the layer files, the schemas, the tick and day phase, the build, the
//! policy, the world config, and the start's own parameters (recorded, and every
//! state-coupled leaf equal to the run's reference). The candidate's parameters then go
//! over the loaded state **rate-only** ([`rate_override`]): the grid's `m` paths (upkeep,
//! motor tied) and `p` paths take the candidate's values; every other leaf stays the
//! start's, and a candidate gene on one is frozen and recorded. Stocks, ages and states
//! stay as loaded. The burn-in ([`burn_in_ticks`]) is fixed per start and ends on the day
//! clock's window grid, so scoring windows are aligned to phase and fixed in advance.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cubarium_voxel::registry::{Registry, to_value};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Config, World};
use cubarium_voxel_fauna::{Fauna, Founder};
use cubarium_voxel_flora::Flora;
use cubarium_voxel_sim::LayerConfig;
use serde::{Deserialize, Serialize};

use super::ecology::EcologyParams;
use super::grid::{M_FOLLOWS, M_PATHS, P_PATHS};

/// A start's manifest, in its directory.
pub const MANIFEST: &str = "start.toml";
/// A start's recipe provenance, in its directory.
pub const RECIPE_FILE: &str = "recipe.toml";
/// The start's own resolved parameters (its layers' configs), in its directory.
pub const PARAMS_FILE: &str = "params.toml";

/// Seed `seed`'s start in library `lib`.
pub fn seed_dir(lib: &Path, seed: u64) -> PathBuf {
    lib.join(format!("seed-{seed}"))
}

/// The three layer files of a start saved at `tick`, as the whole-world save names them.
pub fn layer_files(dir: &Path, tick: u64) -> [PathBuf; 3] {
    [
        dir.join(format!("world-{tick}.voxel")),
        dir.join(format!("flora-{tick}.bin")),
        dir.join(format!("fauna-{tick}.bin")),
    ]
}

/// FNV-1a 64 of `text`, as 16 hex digits.
fn digest(text: &str) -> String {
    format!("{:016x}", crate::es::fixture::fnv1a(text.as_bytes()))
}

/// The world config's identity: its TOML, hashed.
pub fn world_hash(cfg: &Config) -> String {
    let text = to_value(cfg).map(|v| v.to_string()).unwrap_or_default();
    digest(&text)
}

/// The two layers' configs as one TOML document (`[flora]`, `[fauna]`).
pub fn params_toml(layers: &LayerConfig) -> String {
    let mut doc = toml::Table::new();
    if let Ok(v) = to_value(&layers.flora) {
        doc.insert("flora".into(), v);
    }
    if let Ok(v) = to_value(&layers.fauna) {
        doc.insert("fauna".into(), v);
    }
    toml::to_string(&doc).unwrap_or_default()
}

/// A layer set's identity: [`params_toml`], hashed.
pub fn params_hash(layers: &LayerConfig) -> String {
    digest(&params_toml(layers))
}

/// What a start was made by and for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    /// The build (`BUILD_ID`: the commit, `-dirty` when the tree was).
    pub build: String,
    pub world_schema: u32,
    pub flora_schema: u32,
    pub fauna_schema: u32,
    /// [`world_hash`] of the world config the start was founded from.
    pub world_config: String,
    /// [`policy_identity`]: what each lineage was driven by.
    pub policy: Vec<String>,
}

impl Identity {
    /// This build's identity for a world config and a policy.
    pub fn this_build(world: &Config, policy: Vec<String>) -> Identity {
        Identity {
            build: crate::evaluate::BUILD_ID.to_string(),
            world_schema: cubarium_voxel::snapshot::SCHEMA,
            flora_schema: cubarium_voxel_flora::snapshot::SCHEMA,
            fauna_schema: cubarium_voxel_fauna::SCHEMA,
            world_config: world_hash(world),
            policy,
        }
    }
}

/// **A start's manifest** (`start.toml`): written last, so a directory with one is whole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// `e0`, `e1`, …
    pub recipe: String,
    pub seed: u64,
    /// The world's tick at the save: the layer files' names.
    pub tick: u64,
    /// The plant and animal layers' own ticks.
    pub flora_tick: u64,
    pub fauna_tick: u64,
    /// The day clock's phase at `tick` (0 midnight, 0.5 noon).
    pub phase: f64,
    /// Whether the founding's acceptance check passed this world.
    pub accepted: bool,
    pub identity: Identity,
    /// [`params_hash`] of the start's own resolved parameters (`params.toml`).
    pub params: String,
}

impl Manifest {
    /// The manifest of layers about to be written.
    pub fn of(
        recipe: &str,
        seed: u64,
        accepted: bool,
        identity: Identity,
        world: &World,
        flora: &Flora,
        fauna: &Fauna,
    ) -> Manifest {
        Manifest {
            recipe: recipe.into(),
            seed,
            tick: world.tick(),
            flora_tick: flora.tick(),
            fauna_tick: fauna.view().tick,
            phase: world.config().day.cycle_phase(world.tick()),
            accepted,
            identity,
            params: params_hash(&layers_of(flora, fauna)),
        }
    }
}

/// **What a run holds its starts to**: this build's identity for the run's world and
/// policy, and the run's reference parameters (its x1 ecology at the world's cell size),
/// whose state-coupled leaves every start must share.
#[derive(Clone, Debug)]
pub struct Expect {
    pub identity: Identity,
    pub reference: LayerConfig,
}

impl Expect {
    pub fn new(world: &Config, reference: &EcologyParams, policy: Vec<String>) -> Expect {
        Expect {
            identity: Identity::this_build(world, policy),
            reference: reference.layers(world.voxel_m),
        }
    }
}

/// **The policy identity** of a run's launch drivers, one `lineage=…` per lineage in
/// [`Founder::ALL`] order: `heuristic` without a centre (or with `heuristic`), the named
/// control (`parker`, `herding`), or `centre:<weights fnv1a>` for a centre file — the
/// digest a fauna layer records beside its policy flag.
pub fn policy_identity(centres: &[(Founder, PathBuf)]) -> Result<Vec<String>, String> {
    Founder::ALL
        .iter()
        .map(|f| {
            let what = match centres.iter().find(|(c, _)| c == f) {
                None => "heuristic".to_string(),
                Some((_, p)) => {
                    let spec = p.display().to_string();
                    match spec.as_str() {
                        "heuristic" | "parker" | "herding" => spec,
                        _ => {
                            let d = super::search::brain_driver(*f, &spec)?;
                            format!("centre:{:016x}", d.digest())
                        }
                    }
                }
            };
            Ok(format!("{}={what}", f.name()))
        })
        .collect()
}

/// Why a start was refused. Nothing refused is repaired.
#[derive(Clone, Debug, PartialEq)]
pub enum Reject {
    Unreadable { file: String, why: String },
    MissingLayer(String),
    Schema { layer: &'static str, start: u32, build: u32 },
    Tick { layer: &'static str, start: u64, loaded: u64 },
    Phase { start: f64, world: f64 },
    Build { start: String, run: String },
    Policy { start: Vec<String>, run: Vec<String> },
    World { start: String, run: String },
    /// A state-coupled leaf differs from the run's reference: the start belongs to other
    /// parameters.
    Params { path: String, start: f64, run: f64 },
    /// The layers' configs are not the parameters the manifest recorded.
    ParamsRecord { recorded: String, layers: String },
}

impl std::fmt::Display for Reject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reject::Unreadable { file, why } => write!(f, "{file}: {why}"),
            Reject::MissingLayer(file) => write!(f, "missing layer {file}"),
            Reject::Schema { layer, start, build } => {
                write!(f, "{layer} schema {start}, this build reads {build}")
            }
            Reject::Tick { layer, start, loaded } => {
                write!(f, "{layer} tick {loaded}, the manifest says {start}")
            }
            Reject::Phase { start, world } => write!(f, "day phase {world}, the manifest says {start}"),
            Reject::Build { start, run } => write!(f, "made by build {start}, this is {run}"),
            Reject::Policy { start, run } => {
                write!(f, "grown under policy {start:?}, this run drives {run:?}")
            }
            Reject::World { start, run } => write!(f, "world config {start}, this run's is {run}"),
            Reject::Params { path, start, run } => {
                write!(f, "state-coupled {path} is {start} in the start and {run} in this run")
            }
            Reject::ParamsRecord { recorded, layers } => {
                write!(f, "the layers' parameters hash {layers}, the manifest recorded {recorded}")
            }
        }
    }
}

/// A loaded start.
pub struct Start {
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
}

/// Write `bytes` to `path` through a `tmp-` file and a rename: whole or absent.
fn write_whole(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = path.with_file_name(format!("tmp-{name}"));
    std::fs::write(&tmp, bytes).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot rename to {}: {e}", path.display()))
}

/// Write a start: the three layers, `params.toml`, then the manifest last.
pub fn write_start(
    dir: &Path,
    m: &Manifest,
    world: &World,
    flora: &Flora,
    fauna: &Fauna,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let [w, fl, fa] = layer_files(dir, m.tick);
    write_whole(&fl, &cubarium_voxel_flora::snapshot::encode(flora))?;
    write_whole(&fa, &fauna.save())?;
    write_whole(&w, &world.save())?;
    write_whole(&dir.join(PARAMS_FILE), params_toml(&layers_of(flora, fauna)).as_bytes())?;
    let text = toml::to_string(m).map_err(|e| format!("cannot write the manifest: {e}"))?;
    write_whole(&dir.join(MANIFEST), text.as_bytes())
}

/// The manifest alone (a library's listing).
pub fn read_manifest(dir: &Path) -> Result<Manifest, Reject> {
    let file = dir.join(MANIFEST);
    let text = std::fs::read_to_string(&file).map_err(|e| Reject::Unreadable {
        file: file.display().to_string(),
        why: e.to_string(),
    })?;
    toml::from_str(&text).map_err(|e| Reject::Unreadable {
        file: file.display().to_string(),
        why: e.to_string(),
    })
}

/// **Load a start and check it against the run.** Every refusal names what differs.
pub fn load_start(dir: &Path, expect: &Expect) -> Result<Start, Reject> {
    let m = read_manifest(dir)?;
    let (id, run) = (&m.identity, &expect.identity);
    for (layer, start, build) in [
        ("world", id.world_schema, run.world_schema),
        ("flora", id.flora_schema, run.flora_schema),
        ("fauna", id.fauna_schema, run.fauna_schema),
    ] {
        if start != build {
            return Err(Reject::Schema { layer, start, build });
        }
    }
    if id.build != run.build {
        return Err(Reject::Build { start: id.build.clone(), run: run.build.clone() });
    }
    if id.policy != run.policy {
        return Err(Reject::Policy { start: id.policy.clone(), run: run.policy.clone() });
    }
    if id.world_config != run.world_config {
        return Err(Reject::World { start: id.world_config.clone(), run: run.world_config.clone() });
    }
    let files = layer_files(dir, m.tick);
    for f in &files {
        if !f.is_file() {
            return Err(Reject::MissingLayer(f.display().to_string()));
        }
    }
    let read = |f: &Path| {
        std::fs::read(f).map_err(|e| Reject::Unreadable { file: f.display().to_string(), why: e.to_string() })
    };
    let bad = |f: &Path, e: &dyn std::fmt::Display| Reject::Unreadable { file: f.display().to_string(), why: format!("{e:#}") };
    // The one safe load (`cubarium_voxel_sim::saved`): every body gets its heuristic
    // back here. Any lineage may have been policy-driven: the identity check above
    // already holds the run to the start's own policy, and the run installs it again
    // over these (`install_centres`).
    let (world, flora, fauna) = cubarium_voxel_sim::saved::decode(&read(&files[0])?, &read(&files[1])?, &read(&files[2])?, &Founder::ALL)
        .map_err(|e| bad(dir, &e))?;
    for (layer, start, loaded) in [
        ("world", m.tick, world.tick()),
        ("flora", m.flora_tick, flora.tick()),
        ("fauna", m.fauna_tick, fauna.view().tick),
    ] {
        if start != loaded {
            return Err(Reject::Tick { layer, start, loaded });
        }
    }
    let phase = world.config().day.cycle_phase(world.tick());
    if (phase - m.phase).abs() > 1e-9 {
        return Err(Reject::Phase { start: m.phase, world: phase });
    }
    let layers = layers_of(&flora, &fauna);
    let hash = params_hash(&layers);
    if hash != m.params {
        return Err(Reject::ParamsRecord { recorded: m.params.clone(), layers: hash });
    }
    if let Some(c) = state_coupled_differences(&layers, &expect.reference).into_iter().next() {
        return Err(Reject::Params { path: c.path, start: c.start, run: c.candidate });
    }
    Ok(Start { dir: dir.to_path_buf(), manifest: m, world, flora, fauna })
}

/// The configs a pair of layers runs.
pub fn layers_of(flora: &Flora, fauna: &Fauna) -> LayerConfig {
    LayerConfig { flora: flora.config().clone(), fauna: *fauna.config() }
}

/// One leaf that differs between a start's parameters and a candidate's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub start: f64,
    pub candidate: f64,
}

/// Candidate and installed values beside the original reference/start value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstalledChange {
    pub path: String,
    pub start: f64,
    pub candidate: f64,
    pub installed: f64,
}

pub fn parameter_changes(start: &LayerConfig, candidate: &LayerConfig, installed: &LayerConfig) -> Vec<InstalledChange> {
    let mut out = Vec::new();
    for (reg, s, c, i) in [
        (cubarium_voxel_flora::registry(), toml::Value::try_from(&start.flora).unwrap(),
            toml::Value::try_from(&candidate.flora).unwrap(), toml::Value::try_from(&installed.flora).unwrap()),
        (cubarium_voxel_fauna::registry(), toml::Value::try_from(&start.fauna).unwrap(),
            toml::Value::try_from(&candidate.fauna).unwrap(), toml::Value::try_from(&installed.fauna).unwrap()),
    ] {
        let starts = reg.typed_leaves(&s);
        let candidates = reg.typed_leaves(&c);
        let installed = reg.typed_leaves(&i);
        for ((path, a, _), ((_, b, _), (_, actual, _))) in starts.into_iter().zip(candidates.into_iter().zip(installed)) {
            if !same(a, b) || !same(a, actual) { out.push(InstalledChange { path, start: a, candidate: b, installed: actual }); }
        }
    }
    out
}

/// **The rate-only override's result**: the set to install, the rate leaves it moved, and
/// the candidate's other differences, frozen at the start's values.
#[derive(Clone, Debug)]
pub struct Override {
    pub installed: LayerConfig,
    pub changed: Vec<Change>,
    pub frozen: Vec<Change>,
}

/// The rate-only globs: the grid's `m` paths, what follows them, and its `p` paths.
pub fn rate_globs() -> Vec<&'static str> {
    M_PATHS.iter().chain(&M_FOLLOWS).chain(&P_PATHS).copied().collect()
}

fn glob_match(glob: &str, path: &str) -> bool {
    let (g, p): (Vec<&str>, Vec<&str>) = (glob.split('.').collect(), path.split('.').collect());
    g.len() == p.len() && g.iter().zip(&p).all(|(a, b)| *a == "*" || a == b)
}

/// Whether a registry path is one the override takes from the candidate.
pub fn is_rate_path(path: &str) -> bool {
    rate_globs().iter().any(|g| glob_match(g, path))
}

/// Every numeric leaf of one config under its registry: `(path, value, integer)`.
fn typed<T: Serialize>(reg: &Registry, cfg: &T) -> Result<Vec<(String, f64, bool)>, String> {
    Ok(reg.typed_leaves(&to_value(cfg)?))
}

fn same(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
}

/// The leaves of `a` and `b` that differ, both layers, with `filter` on the path.
fn differences(
    a: &LayerConfig,
    b: &LayerConfig,
    filter: impl Fn(&str) -> bool,
) -> Result<Vec<(usize, Change, bool)>, String> {
    let mut out = Vec::new();
    let parts: [(&Registry, Vec<(String, f64, bool)>, Vec<(String, f64, bool)>); 2] = [
        (cubarium_voxel_flora::registry(), typed(cubarium_voxel_flora::registry(), &a.flora)?, typed(cubarium_voxel_flora::registry(), &b.flora)?),
        (cubarium_voxel_fauna::registry(), typed(cubarium_voxel_fauna::registry(), &a.fauna)?, typed(cubarium_voxel_fauna::registry(), &b.fauna)?),
    ];
    for (k, (_, la, lb)) in parts.iter().enumerate() {
        let index: std::collections::HashMap<&str, (f64, bool)> =
            lb.iter().map(|(p, v, i)| (p.as_str(), (*v, *i))).collect();
        if la.len() != lb.len() {
            return Err(format!("the two parameter sets have different shapes ({} and {} leaves)", la.len(), lb.len()));
        }
        for (path, va, _) in la {
            let (vb, integer) = *index
                .get(path.as_str())
                .ok_or_else(|| format!("{path} is in one parameter set and not the other"))?;
            if filter(path) && !same(*va, vb) {
                out.push((k, Change { path: path.clone(), start: *va, candidate: vb }, integer));
            }
        }
    }
    Ok(out)
}

/// The state-coupled (not rate-only) leaves where a start's parameters differ from the
/// run's reference, `candidate` holding the reference's value.
pub fn state_coupled_differences(start: &LayerConfig, reference: &LayerConfig) -> Vec<Change> {
    match differences(start, reference, |p| !is_rate_path(p)) {
        Ok(d) => d.into_iter().map(|(_, c, _)| c).collect(),
        Err(e) => vec![Change { path: e, start: f64::NAN, candidate: f64::NAN }],
    }
}

/// **The rate-only override**: `start` with every rate-only leaf set to `candidate`'s,
/// then validated (each layer's own cross-field rules and every registry range). The
/// candidate's differences elsewhere are frozen at the start's values and listed.
pub fn rate_override(start: &LayerConfig, candidate: &LayerConfig) -> Result<Override, String> {
    let all = differences(start, candidate, |_| true)?;
    let mut installed = start.clone();
    let (mut sets_flora, mut sets_fauna) = (Vec::new(), Vec::new());
    let (mut changed, mut frozen) = (Vec::new(), Vec::new());
    for (k, c, integer) in all {
        if is_rate_path(&c.path) {
            let v = if integer {
                toml::Value::Integer(c.candidate.round() as i64)
            } else {
                toml::Value::Float(c.candidate)
            };
            if k == 0 {
                sets_flora.push((c.path.clone(), v));
            } else {
                sets_fauna.push((c.path.clone(), v));
            }
            changed.push(c);
        } else {
            frozen.push(c);
        }
    }
    cubarium_voxel_flora::registry().apply(&mut installed.flora, &sets_flora)?;
    cubarium_voxel_fauna::registry().apply(&mut installed.fauna, &sets_fauna)?;
    installed.flora.validate()?;
    installed.fauna.validate()?;
    Ok(Override { installed, changed, frozen })
}

/// **Install a resolved set in every layer**: the plants' and the animals' configs
/// replaced whole (each validated by its layer). Stored state is not touched.
pub fn install(flora: &mut Flora, fauna: &mut Fauna, layers: &LayerConfig) -> Result<(), String> {
    flora.replace_config(layers.flora.clone())?;
    fauna.replace_config(layers.fauna)
}

/// **The ledgers' residuals**: the staged runs' own ([`cubarium_voxel_sim::staged::Residuals`]).
pub use cubarium_voxel_sim::staged::Residuals;

/// **The burn-in**, ticks from `start_tick`: at least `min_ticks`, then on to the next
/// boundary of the day clock's window grid (every `align_ticks` from the clock's
/// midnight), so scoring windows begin at a fixed phase. `align_ticks` 0, or a clock that
/// is not cycling, is the minimum alone.
pub fn burn_in_ticks(start_tick: u64, min_ticks: u64, align_ticks: u64, day: &DayClock) -> u64 {
    if align_ticks == 0 || day.mode != DayMode::Cycle {
        return min_ticks;
    }
    let day_ticks = day.length_min * 60.0 * f64::from(cubarium_voxel::TICK_HZ);
    let offset = (day.start_hour / 24.0 * day_ticks).round() as u64;
    let earliest = start_tick + min_ticks;
    let into = (offset + earliest) % align_ticks;
    let end = if into == 0 { earliest } else { earliest + (align_ticks - into) };
    end - start_tick
}

/// **A start library**: `seed-<n>/` directories, every one checked against the run when
/// the library is opened. One incompatible start refuses the whole library: no seed is
/// skipped or swapped.
#[derive(Debug)]
pub struct StartLibrary {
    pub dir: PathBuf,
    /// The recipe every start names (`mixed` when they differ).
    pub recipe: String,
    pub seeds: Vec<u64>,
    pub expect: Expect,
}

impl StartLibrary {
    pub fn open(dir: &Path, expect: Expect) -> Result<StartLibrary, String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("start library {}: {e}", dir.display()))?;
        let mut seeds: Vec<u64> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str()?.strip_prefix("seed-")?.parse().ok())
            .collect();
        seeds.sort_unstable();
        if seeds.is_empty() {
            return Err(format!("start library {} holds no seed-<n> starts", dir.display()));
        }
        let mut recipes = Vec::new();
        let mut refused = Vec::new();
        for &s in &seeds {
            match load_start(&seed_dir(dir, s), &expect) {
                Ok(start) => {
                    if start.manifest.seed != s {
                        refused.push(format!("seed-{s}: its manifest names seed {}", start.manifest.seed));
                    }
                    recipes.push(start.manifest.recipe);
                }
                Err(e) => refused.push(format!("seed-{s}: {e}")),
            }
        }
        if !refused.is_empty() {
            return Err(format!(
                "start library {} refused (regenerate it; nothing is repaired):\n  {}",
                dir.display(),
                refused.join("\n  ")
            ));
        }
        recipes.dedup();
        let recipe = if recipes.len() == 1 { recipes.remove(0) } else { "mixed".into() };
        Ok(StartLibrary { dir: dir.to_path_buf(), recipe, seeds, expect })
    }

    /// Seed `seed`'s start, checked again.
    pub fn load(&self, seed: u64) -> Result<Start, Reject> {
        load_start(&seed_dir(&self.dir, seed), &self.expect)
    }

    /// `e1 (DIR)`.
    pub fn label(&self) -> String {
        format!("{} ({})", self.recipe, self.dir.display())
    }
}

/// **Where every candidate starts.**
#[derive(Clone, Default)]
pub enum StartSet {
    /// Today's founding under the candidate's own ecology: the disclosed early-community
    /// diagnostic, and the only setting a state-coupled gene acts in.
    #[default]
    E0,
    /// A start library: every candidate on the same loaded starts, rate-only.
    Library(Arc<StartLibrary>),
}

impl StartSet {
    pub fn label(&self) -> String {
        match self {
            StartSet::E0 => "e0".into(),
            StartSet::Library(l) => l.label(),
        }
    }

    /// Whether this is an established setting (every lineage scored, R1).
    pub fn established(&self) -> bool {
        matches!(self, StartSet::Library(_))
    }
}

impl std::fmt::Debug for StartSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// The burn-in: a minimum, and the window grid it ends on (0: no alignment).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BurnIn {
    pub min_ticks: u64,
    pub align_ticks: u64,
}

/// **How one evaluation started**, for its record.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StartRecord {
    /// Exact installed driver identities, including weights digests.
    #[serde(default)]
    pub drivers: Vec<String>,
    /// Numeric leaves that differ from the reference/start, with installed values.
    #[serde(default)]
    pub parameter_changes: Vec<InstalledChange>,
    #[serde(default)]
    pub start_identity: Option<Identity>,
    /// [`StartSet::label`].
    pub set: String,
    pub seed: u64,
    /// The world's tick as loaded (or founded).
    pub start_tick: u64,
    pub burn_in_ticks: u64,
    /// Why the burn-in stopped early (recorded; the run is scored as it stands).
    pub burn_in_stopped: Option<String>,
    /// Lineages present at the start and gone (no body, no egg) by the burn-in's end.
    pub burn_in_lost: Vec<String>,
    /// Where scoring starts: the burn-in's end.
    pub scoring_tick: u64,
    pub scoring_phase: f64,
    /// [`params_hash`] of the start's own parameters, the candidate's, and the set
    /// installed.
    pub start_params: String,
    pub candidate_params: String,
    pub installed_params: String,
    /// Rate-only leaves the candidate moved.
    pub changed: usize,
    /// The candidate's state-coupled differences, held at the start's values.
    pub frozen: Vec<Change>,
    /// [`Residuals`] by stage: `load`, `override`, `burn-in` (`found`, `burn-in` on E0).
    pub residuals: Vec<(String, Residuals)>,
}

/// **A ledger baseline**: the burn-in's end and every scoring-window boundary, so scores
/// and trends read from checkpoints.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    pub tick: u64,
    pub phase: f64,
    /// Bodies, eggs, body + reserve + escrow, and assimilated (cumulative) per lineage.
    pub bodies: [usize; Founder::COUNT],
    pub eggs: [usize; Founder::COUNT],
    pub biomass: [f64; Founder::COUNT],
    pub assimilated: [f64; Founder::COUNT],
    /// Deaths per lineage, every cause, cumulative.
    pub deaths: [u64; Founder::COUNT],
    /// Plant organic stock and stands.
    pub plant_organic: f64,
    pub stands: usize,
    /// The worst ledger residual here.
    pub residual: f64,
}

impl Baseline {
    pub fn of(world: &World, flora: &Flora, fauna: &Fauna) -> Baseline {
        let v = fauna.view();
        let mut b = Baseline {
            tick: world.tick(),
            phase: world.config().day.cycle_phase(world.tick()),
            assimilated: v.ledger.assimilated_by_founder,
            plant_organic: flora.view().organic(),
            stands: flora.view().stands.len(),
            residual: Residuals::of(world, flora, fauna).worst(),
            ..Baseline::default()
        };
        for a in v.animals.iter() {
            let Some(f) = a.founder else { continue };
            b.bodies[f.index()] += 1;
            b.biomass[f.index()] += a.body + a.reserve + a.reproduction.escrow.map_or(0.0, |e| e.organic);
        }
        for f in Founder::ALL {
            b.eggs[f.index()] = v.eggs_by_founder(f) as usize;
            b.deaths[f.index()] = v.ledger.deaths_by_founder_cause[f.index()].iter().sum();
        }
        b
    }
}
