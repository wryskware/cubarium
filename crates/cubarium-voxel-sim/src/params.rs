//! **The resolved parameter set** (package R1, `design/ecology-parameter-registry-2026-09-28.md`
//! §2): the world, plant and animal configs a run uses, built in layers — code defaults,
//! the preset or world TOML, the search genome, `--set path=value` — validated against the
//! three registries, dumped, and written beside a run's output.

use std::path::{Path, PathBuf};

use cubarium_voxel::Config;
use cubarium_voxel::registry::DumpLine;
use cubarium_voxel_fauna::FaunaConfig;
use cubarium_voxel_flora::FloraConfig;

/// The file a run writes its resolved set to, in its output directory.
pub const PARAMS_FILE: &str = "params.toml";

/// Every number a run uses.
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub world: Config,
    /// The plant layer as authored, on the reference 0.25 m voxel;
    /// [`Params::flora_for_world`] is it rescaled to the world's.
    pub flora: FloraConfig,
    pub fauna: FaunaConfig,
}

/// What lies over the code defaults, applied in field order.
#[derive(Default)]
pub struct Layers<'a> {
    /// A TOML document: its `[world]`, `[flora]` and `[fauna]` tables; any other key is
    /// someone else's (the presenter's) and ignored.
    pub file: Option<toml::Table>,
    /// The search genome's edit.
    pub genome: Option<&'a dyn Fn(&mut Params)>,
    /// `path=value` assignments, last.
    pub sets: Vec<String>,
}

impl Params {
    pub fn defaults() -> Params {
        todo!()
    }

    pub fn resolve(_layers: &Layers<'_>) -> Result<Params, String> {
        todo!()
    }

    pub fn validate(&self) -> Result<(), String> {
        todo!()
    }

    pub fn flora_for_world(&self) -> FloraConfig {
        todo!()
    }

    pub fn dump(&self) -> Result<Vec<DumpLine>, String> {
        todo!()
    }

    pub fn markdown(&self) -> Result<String, String> {
        todo!()
    }

    pub fn to_toml(&self) -> Result<String, String> {
        todo!()
    }

    pub fn write_beside(&self, _dir: &Path) -> Result<PathBuf, String> {
        todo!()
    }
}
