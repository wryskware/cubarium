//! **The simulation's parameter set**: the half of a `config/**/*.toml` file that says what
//! world to build and how to run it, as opposed to how to draw it. `cubarium voxel` reads
//! the whole file and hands this half here ([`SimParams::split_from`]); a session gets it
//! from a [`WorldSpec`] (a config file plus the size, landform and climate a new-world
//! picker chose).
//!
//! Unknown fields are refused at every level the session owns, so a typo is an error and
//! never a silently ignored key.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cubarium_voxel_sim::SimConfig;
use cubarium_voxel_sim::params::{Layers, Params};
use serde::{Deserialize, Deserializer, Serialize};

/// The keys of a config file's top level that belong to the simulation.
pub const SIM_KEYS: [&str; 7] = [
    "world",
    "flora",
    "fauna",
    "threads",
    "water_every",
    "fast_forward_water_every",
    "observatory",
];

/// The simulation half of a config file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SimParams {
    /// The world to build. Extents follow the core; the ambient display uses 0.125 m
    /// cells so organisms retain their physical size with more cells of visual detail.
    #[serde(deserialize_with = "ambient_world_config")]
    pub world: cubarium_voxel::Config,
    /// `[flora]`: plant parameters by registry path over the defaults
    /// (`cubarium_voxel_sim::params`), e.g. `[flora.springturf] foliage_rate = 0.0004`.
    #[serde(skip_serializing_if = "toml::Table::is_empty")]
    pub flora: toml::Table,
    /// `[fauna]`: animal parameters by registry path, e.g.
    /// `[fauna.founders.frondgrazer.core] maintenance_per_s = 0.002`.
    #[serde(skip_serializing_if = "toml::Table::is_empty")]
    pub fauna: toml::Table,
    /// Worker threads for the in-phase splits of the tick
    /// (`cubarium_voxel_sim::SimConfig::threads`); `0` means
    /// [`cubarium_voxel::default_threads`]. Execution only — it reaches no rule, no
    /// number and no picture. **Placeholder** (`design/backlog.md` §1).
    pub threads: usize,
    /// **The display's coarse water** ([`cubarium_voxel::World::set_water_every`],
    /// `cubarium_voxel_sim::SimConfig::water_every`): the lateral and groundwater phases
    /// on one tick in this many. `1`, the live water, is the display's setting (Wrysk,
    /// 2026-10-04); training, search and experiment runs take their own preset.
    pub water_every: u32,
    /// **Hook, not read yet:** the coarse water a future fast-forward display mode would
    /// switch to while it runs above 1× (and back to [`SimParams::water_every`] at 1×),
    /// with the plant clock and the rest of the speed settings beside it. No mode reads it.
    pub fast_forward_water_every: u32,
    /// `[observatory]`: the served history's sampling, in simulated seconds
    /// (`sample_s` between samples, `span_s` kept).
    pub observatory: cubarium_observatory::HistoryConfig,
}

impl Default for SimParams {
    fn default() -> SimParams {
        SimParams {
            // The shipped `default` landscape, ring and all: a run with no TOML generates a
            // staged world, not the ridge. `Config::default()` stays `Landform::Ridge` for
            // fixtures and frozen arenas, and `landform = "ridge"` asks for it here.
            world: cubarium_voxel::Preset::find("default")
                .expect("the shipped presets include `default`")
                .config(),
            flora: toml::Table::new(),
            fauna: toml::Table::new(),
            threads: 0,
            water_every: 1,
            fast_forward_water_every: cubarium_voxel_sim::EXPERIMENT_WATER_EVERY,
            observatory: cubarium_observatory::HistoryConfig::default(),
        }
    }
}

// A partial [world] table must use the same cell size as an omitted table, which since
// the ambient default became the shipped `default` landscape is that landscape's own
// 0.25 m. `cubarium_voxel::Config::default()` is untouched, so frozen arenas and the
// simulation fixtures keep theirs.
fn ambient_world_config<'de, D>(deserializer: D) -> std::result::Result<cubarium_voxel::Config, D::Error>
where
    D: Deserializer<'de>,
{
    let table = toml::Table::deserialize(deserializer)?;
    // The cell-size default and the landform-preset expansion live in the core
    // (`cubarium_voxel::world_table`), shared with the training harness.
    cubarium_voxel::world_config_from_table(table).map_err(serde::de::Error::custom)
}

impl SimParams {
    /// Take the simulation's keys ([`SIM_KEYS`]) out of a config file's top-level table
    /// and parse them; what is left is the host's (the presentation).
    pub fn split_from(table: &mut toml::Table) -> Result<SimParams, String> {
        let mut sim = toml::Table::new();
        for key in SIM_KEYS {
            if let Some(v) = table.remove(key) {
                sim.insert(key.to_string(), v);
            }
        }
        SimParams::from_table(sim)
    }

    /// Parse a table holding only simulation keys.
    pub fn from_table(table: toml::Table) -> Result<SimParams, String> {
        toml::Value::Table(table).try_into().map_err(|e: toml::de::Error| e.to_string())
    }

    /// The simulation half of the config file at `path` (its `base` chain merged): the
    /// presentation keys a desktop config carries are the binary's, and ignored here.
    pub fn from_config_file(path: &Path) -> Result<SimParams> {
        let mut table = config_table(path)?;
        SimParams::split_from(&mut table)
            .map_err(|e| anyhow::anyhow!("the voxel config {}: {e}", path.display()))
    }

    /// **The resolved parameter set** (package R1,
    /// `design/ecology-parameter-registry-2026-09-28.md`): `[world]`, then the `[flora]`
    /// and `[fauna]` tables, then a co-training `--params` file if one is given (a genome
    /// decoded over the tables, or a resolved file replacing them whole), then `sets`,
    /// every layer validated against the parameter registry.
    pub fn resolve(&self, params_file: Option<&Path>, sets: &[String]) -> Result<Params> {
        match params_file {
            None => self.resolve_sets(sets),
            Some(file) => self.resolve_with(file, sets),
        }
    }

    /// The tables, then `sets`.
    pub fn resolve_sets(&self, sets: &[String]) -> Result<Params> {
        let mut file = toml::Table::new();
        if !self.flora.is_empty() {
            file.insert("flora".into(), toml::Value::Table(self.flora.clone()));
        }
        if !self.fauna.is_empty() {
            file.insert("fauna".into(), toml::Value::Table(self.fauna.clone()));
        }
        let base = Params {
            world: self.world.clone(),
            ..Params::defaults()
        };
        Params::resolve_over(
            base,
            &Layers {
                file: Some(file),
                genome: None,
                sets: sets.to_vec(),
            },
        )
        .map_err(|e| anyhow::anyhow!("the parameter set: {e}"))
    }

    /// The tables, a co-training run's ecology over them (`--params`, R1's genome layer),
    /// then `sets`.
    pub fn resolve_with(&self, file: &Path, sets: &[String]) -> Result<Params> {
        use cubarium_search::es::voxel::live::ecology::EcologyParams;
        let mut p = self.resolve_sets(&[])?;
        let ecology = EcologyParams::load(file).map_err(|e| anyhow::anyhow!("--params: {e}"))?;
        match &ecology {
            EcologyParams::Genome(g) => {
                p.apply_genome(g)
                    .map_err(|e| anyhow::anyhow!("--params {}: {e}", file.display()))?;
                let moved: Vec<String> = g
                    .genes
                    .iter()
                    .filter(|x| x.value != 1.0)
                    .map(|x| format!("{}={:.3}", x.name, x.value))
                    .collect();
                static SAID: std::sync::Once = std::sync::Once::new();
                SAID.call_once(|| {
                    eprintln!(
                        "--params {}: a genome of {} genes; off x1: {}",
                        file.display(),
                        g.genes.len(),
                        if moved.is_empty() { "none".into() } else { moved.join(" ") }
                    )
                });
            }
            EcologyParams::Resolved { flora, fauna } => {
                p.flora = (**flora).clone();
                p.fauna = **fauna;
                eprintln!("--params {}: a resolved parameter file", file.display());
            }
        }
        Params::resolve_over(
            p,
            &Layers {
                file: None,
                genome: None,
                sets: sets.to_vec(),
            },
        )
        .map_err(|e| anyhow::anyhow!("the parameter set: {e}"))
    }

    /// The tick schedule's execution settings. `0` threads is "decide for me", and the
    /// decision is the voxel crate's: `default_threads` is every core the OS reports but
    /// one, so the thread that presents is not competing with the tick for the last core.
    pub fn sim_config(&self) -> SimConfig {
        self.sim_config_with_reserve(1)
    }

    /// Size automatic pools to the inherited CPU allowance, leaving `reserve` logical
    /// CPUs for the host's other work. Explicit config and environment counts win.
    pub fn sim_config_with_reserve(&self, reserve: usize) -> SimConfig {
        SimConfig {
            threads: worker_threads(
                std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
                reserve,
                (self.threads != 0).then_some(self.threads).or_else(cubarium_voxel::thread_override),
            ),
            water_every: self.water_every.max(1),
            ..SimConfig::default()
        }
    }
}

fn worker_threads(available: usize, reserve: usize, asked: Option<usize>) -> usize {
    asked.unwrap_or_else(|| available.saturating_sub(reserve).max(1))
}

#[cfg(test)]
mod worker_budget_tests {
    use super::worker_threads;

    #[test]
    fn automatic_workers_leave_the_requested_capacity_and_explicit_counts_win() {
        for available in [1, 3, 9, 27] {
            for reserve in [0, 1, 2, 5] {
                assert_eq!(worker_threads(available, reserve, None), available.saturating_sub(reserve).max(1));
                for asked in [1, 7, 23] {
                    assert_eq!(worker_threads(available, reserve, Some(asked)), asked);
                }
            }
        }
    }
}

/// **What a new world is made from**: a config file's simulation half, with the size,
/// landform and climate a picker chose laid over its `[world]`, and a seed. Unknown fields
/// are refused, so a misspelt key from Godot is an error.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldSpec {
    /// The config file whose simulation half is the base (`config/desktop/terrarium.toml`);
    /// absent, the built-in defaults.
    pub config: Option<PathBuf>,
    /// `[width, height, depth]` in voxels, over the file's `[world]`.
    pub size: Option<[u32; 3]>,
    /// A shipped landform preset name (`[world.landform] preset = …`), e.g. `terrarium`.
    pub terrain: Option<String>,
    /// A climate preset name (`[world.climate] preset = …`), e.g. `tropical_jungle`.
    pub climate: Option<String>,
    /// The terrain seed, kept whatever the founding loop thinks of it; absent, one is drawn.
    pub seed: Option<u64>,
    /// Registry `PATH=VALUE` overrides, as `--set` takes them.
    pub sets: Vec<String>,
    /// A bare world: no stands, no animals.
    pub empty: bool,
    /// **The water table**, metres over the lowest yard ground: a terrarium landform's
    /// `water_level_m` ([`cubarium_voxel::Terrarium::water_level_m`]), over the file's or
    /// the preset's. The new-world picker's water slider (Wrysk 2026-10-08: Low, Medium
    /// and High are 0.375, 0.75 and 1.125 m). Refused for a landform without one.
    pub water_level_m: Option<f64>,
}

/// The new-world picker's water steps (Wrysk 2026-10-08): Low, Medium and High, the
/// terrarium's `water_level_m` in metres — the tables compared on 2026-10-07.
pub const WATER_LEVELS: [(&str, f64); 3] = [("low", 0.375), ("medium", 0.75), ("high", 1.125)];

impl WorldSpec {
    /// The parameter set this spec names: the file's simulation half (or the defaults),
    /// its `[world]` table patched with the size, landform and climate, then parsed with
    /// every unknown key refused.
    pub fn sim_params(&self) -> Result<SimParams> {
        let mut table = match &self.config {
            Some(path) => {
                let mut t = config_table(path)?;
                let mut sim = toml::Table::new();
                for key in SIM_KEYS {
                    if let Some(v) = t.remove(key) {
                        sim.insert(key.to_string(), v);
                    }
                }
                sim
            }
            None => toml::Table::new(),
        };
        let world = table
            .entry("world")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        let toml::Value::Table(world) = world else {
            anyhow::bail!("`world` is not a table");
        };
        if let Some([w, h, d]) = self.size {
            world.insert("width".into(), toml::Value::Integer(i64::from(w)));
            world.insert("height".into(), toml::Value::Integer(i64::from(h)));
            world.insert("depth".into(), toml::Value::Integer(i64::from(d)));
        }
        if let Some(name) = &self.terrain {
            let mut landform = toml::Table::new();
            landform.insert("preset".into(), toml::Value::String(name.clone()));
            world.insert("landform".into(), toml::Value::Table(landform));
        }
        if let Some(name) = &self.climate {
            let climate = world
                .entry("climate")
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            let toml::Value::Table(climate) = climate else {
                anyhow::bail!("`world.climate` is not a table");
            };
            climate.insert("preset".into(), toml::Value::String(name.clone()));
        }
        let mut params = SimParams::from_table(table).map_err(|e| anyhow::anyhow!("the world spec: {e}"))?;
        if let Some(level) = self.water_level_m {
            anyhow::ensure!(
                level.is_finite() && level >= 0.0,
                "the world spec's water_level_m is {level}; it is metres over the yard's ground, 0 or more"
            );
            match &mut params.world.landform {
                cubarium_voxel::Landform::Terrarium(t) => t.water_level_m = level,
                _ => anyhow::bail!(
                    "the world spec's water_level_m sets a terrarium's water table; this landform has none"
                ),
            }
        }
        params
            .world
            .validate()
            .map_err(|e| anyhow::anyhow!("the world spec's [world]: {e}"))?;
        Ok(params)
    }
}

/// **The presets a new-world picker can name** (query `presets`): every shipped landform
/// preset a [`WorldSpec::terrain`] takes, with its generator (`terrarium` or `staged`) and,
/// for a staged landscape, the ring it was written for; and every climate preset a
/// [`WorldSpec::climate`] takes. Names only: what to call them and which to offer is the
/// picker's own data.
pub fn preset_table() -> serde_json::Value {
    use serde_json::json;
    let terrariums = cubarium_voxel::Terrarium::PRESETS
        .iter()
        .map(|(name, _)| json!({ "name": name, "kind": "terrarium" }));
    let staged = cubarium_voxel::PRESETS.iter().map(|p| {
        json!({
            "name": p.name,
            "kind": "staged",
            "written_for": { "size": [p.width, p.height, p.depth], "voxel_m": p.voxel_m },
        })
    });
    let climates: Vec<serde_json::Value> = cubarium_voxel::weather::ClimatePreset::ALL
        .into_iter()
        .map(|c| json!({ "name": c.name() }))
        .collect();
    json!({ "terrain": terrariums.chain(staged).collect::<Vec<_>>(), "climate": climates })
}

/// A config file's table with its `base` chain merged under it.
///
/// **`base = "<file>"`** (scenario maps, 2026-10-04): the file is read first, relative to
/// this one, and this file's keys are laid over it — tables merged key by key, anything
/// else replaced. A scenario map is its world and founding table over the desktop
/// terrarium's ecology, not a copy of it.
pub fn config_table(path: &Path) -> Result<toml::Table> {
    config_chain(path, 0)
}

fn config_chain(path: &Path, depth: usize) -> Result<toml::Table> {
    anyhow::ensure!(depth < 8, "the voxel config {}: `base` nests too deep", path.display());
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the voxel config {}", path.display()))?;
    let mut table: toml::Table =
        toml::from_str(&text).with_context(|| format!("parsing the voxel config {}", path.display()))?;
    let Some(base) = table.remove("base") else {
        return Ok(table);
    };
    let base = base
        .as_str()
        .with_context(|| format!("the voxel config {}: `base` names a file", path.display()))?;
    let base_path = path.parent().unwrap_or(Path::new(".")).join(base);
    let mut merged = config_chain(&base_path, depth + 1)?;
    merge_table(&mut merged, table);
    Ok(merged)
}

fn merge_table(under: &mut toml::Table, over: toml::Table) {
    for (k, v) in over {
        match (under.get_mut(&k), v) {
            (Some(toml::Value::Table(u)), toml::Value::Table(o)) => merge_table(u, o),
            (_, v) => {
                under.insert(k, v);
            }
        }
    }
}
