//! **The episode initializer** (`design/community-assembly-2026-10-03.md` revisions 2–3,
//! `design/handoffs/assembly-step1-2026-10-03.md`): stubs, tests first.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cubarium_voxel::{Config, World};
use cubarium_voxel_fauna::{Fauna, Founder};
use cubarium_voxel_flora::Flora;
use cubarium_voxel_sim::LayerConfig;
use serde::{Deserialize, Serialize};

use super::ecology::EcologyParams;

pub const MANIFEST: &str = "start.toml";
pub const RECIPE_FILE: &str = "recipe.toml";

pub fn seed_dir(lib: &Path, seed: u64) -> PathBuf {
    lib.join(format!("seed-{seed}"))
}

pub fn layer_files(dir: &Path, tick: u64) -> [PathBuf; 3] {
    [
        dir.join(format!("world-{tick}.voxel")),
        dir.join(format!("flora-{tick}.bin")),
        dir.join(format!("fauna-{tick}.bin")),
    ]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    pub build: String,
    pub world_schema: u32,
    pub flora_schema: u32,
    pub fauna_schema: u32,
    pub world_config: String,
    pub policy: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub recipe: String,
    pub seed: u64,
    pub tick: u64,
    pub phase: f64,
    pub accepted: bool,
    pub identity: Identity,
    pub params: String,
}

#[derive(Clone, Debug)]
pub struct Expect {
    pub identity: Identity,
    pub reference: LayerConfig,
}

impl Expect {
    pub fn new(_world: &Config, _reference: &EcologyParams, _policy: Vec<String>) -> Expect {
        todo!()
    }
}

pub fn policy_identity(_centres: &[(Founder, PathBuf)]) -> Result<Vec<String>, String> {
    todo!()
}

#[derive(Clone, Debug, PartialEq)]
pub enum Reject {
    Unreadable { file: String, why: String },
    MissingLayer(String),
    Schema { layer: &'static str, start: u32, build: u32 },
    Tick { file: String, start: u64, layer: u64 },
    Phase { start: f64, world: f64 },
    Build { start: String, run: String },
    Policy { start: Vec<String>, run: Vec<String> },
    World { start: String, run: String },
    Params { path: String, start: f64, run: f64 },
    ParamsRecord { recorded: String, layers: String },
}

pub struct Start {
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
}

pub fn write_start(_dir: &Path, _m: &Manifest, _world: &World, _flora: &Flora, _fauna: &Fauna) -> Result<(), String> {
    todo!()
}

pub fn load_start(_dir: &Path, _expect: &Expect) -> Result<Start, Reject> {
    todo!()
}

pub fn layers_of(flora: &Flora, fauna: &Fauna) -> LayerConfig {
    LayerConfig { flora: flora.config().clone(), fauna: *fauna.config() }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub start: f64,
    pub candidate: f64,
}

pub struct Override {
    pub installed: LayerConfig,
    pub changed: Vec<Change>,
    pub frozen: Vec<Change>,
}

pub fn is_rate_path(_path: &str) -> bool {
    todo!()
}

pub fn rate_override(_start: &LayerConfig, _candidate: &LayerConfig) -> Result<Override, String> {
    todo!()
}

pub fn install(_flora: &mut Flora, _fauna: &mut Fauna, _layers: &LayerConfig) -> Result<(), String> {
    todo!()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Residuals {
    pub water: f64,
    pub flora: [f64; 3],
    pub fauna: [f64; 3],
}

impl Residuals {
    pub fn of(_world: &World, _flora: &Flora, _fauna: &Fauna) -> Residuals {
        todo!()
    }
    pub fn worst(&self) -> f64 {
        todo!()
    }
}

pub fn burn_in_ticks(_start_tick: u64, _min_ticks: u64, _align_ticks: u64, _day: &cubarium_voxel::weather::DayClock) -> u64 {
    todo!()
}

pub struct StartLibrary {
    pub dir: PathBuf,
    pub recipe: String,
    pub seeds: Vec<u64>,
    pub expect: Expect,
}

impl StartLibrary {
    pub fn open(_dir: &Path, _expect: Expect) -> Result<StartLibrary, String> {
        todo!()
    }
}

#[derive(Clone, Default)]
pub enum StartSet {
    #[default]
    E0,
    Library(Arc<StartLibrary>),
}

impl std::fmt::Debug for StartSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartSet::E0 => write!(f, "e0"),
            StartSet::Library(l) => write!(f, "{}", l.dir.display()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BurnIn {
    pub min_ticks: u64,
    pub align_ticks: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StartRecord {
    pub set: String,
    pub seed: u64,
    pub start_tick: u64,
    pub burn_in_ticks: u64,
    pub burn_in_stopped: Option<String>,
    pub burn_in_lost: Vec<String>,
    pub scoring_tick: u64,
    pub scoring_phase: f64,
    pub start_params: String,
    pub candidate_params: String,
    pub installed_params: String,
    pub changed: usize,
    pub frozen: Vec<Change>,
    pub residuals: Vec<(String, Residuals)>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    pub tick: u64,
    pub phase: f64,
}
