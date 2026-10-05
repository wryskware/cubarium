//! **The numbers the sim reads**, through the sim's own loaders: a config file's `base`
//! chain merged under it, its `[world]` table through [`cubarium_voxel::world_config_from_table`],
//! and its `[flora]`/`[fauna]` tables through [`cubarium_voxel_sim::params::Params::resolve_over`]
//! (the registry's layering), plus any `path=value` sets. The plant layer is then rescaled
//! to the world's cell as the sim does ([`Params::flora_for_world`]).
//!
//! The `base` merge restates `cubarium::voxel::config_chain`
//! (`crates/cubarium/src/voxel/mod.rs:751`): a file-reading convenience with no number in
//! it, restated so this crate need not link the GPU host.

use std::path::Path;

use cubarium_voxel_flora::FloraConfig;
use cubarium_voxel_sim::params::{Layers, Params};

/// A config file's table with its `base` chain merged under it (tables merged key by key,
/// anything else replaced), as `cubarium::voxel::config_chain` does.
pub fn config_table(path: &Path) -> Result<toml::Table, String> {
    chain(path, 0)
}

fn chain(path: &Path, depth: usize) -> Result<toml::Table, String> {
    if depth >= 8 {
        return Err(format!("{}: `base` nests too deep", path.display()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let mut table: toml::Table = toml::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?;
    let Some(base) = table.remove("base") else {
        return Ok(table);
    };
    let base = base.as_str().ok_or_else(|| format!("{}: `base` names a file", path.display()))?;
    let mut merged = chain(&path.parent().unwrap_or(Path::new(".")).join(base), depth + 1)?;
    merge(&mut merged, table);
    Ok(merged)
}

fn merge(under: &mut toml::Table, over: toml::Table) {
    for (k, v) in over {
        match (under.get_mut(&k), v) {
            (Some(toml::Value::Table(u)), toml::Value::Table(o)) => merge(u, o),
            (_, v) => {
                under.insert(k, v);
            }
        }
    }
}

/// The resolved parameter set of a config file, as `succession_study`'s `resolve` builds it
/// (`crates/cubarium/examples/succession_study.rs:79`): the file's world, then code
/// defaults with the file's `[flora]`/`[fauna]` and `sets` laid over them.
pub fn resolve(path: &Path, sets: &[String]) -> Result<Params, String> {
    let mut table = config_table(path)?;
    let world = match table.remove("world") {
        Some(toml::Value::Table(t)) => t,
        _ => toml::Table::new(),
    };
    let world = cubarium_voxel::world_config_from_table(world)?;
    let mut file = toml::Table::new();
    for key in ["flora", "fauna"] {
        if let Some(toml::Value::Table(t)) = table.remove(key) {
            file.insert(key.into(), toml::Value::Table(t));
        }
    }
    Params::resolve_over(
        Params {
            world,
            ..Params::defaults()
        },
        &Layers {
            file: Some(file),
            sets: sets.to_vec(),
            ..Layers::default()
        },
    )
}

/// The plant layer a world of the resolved set runs (rescaled to its cell).
pub fn flora_of(params: &Params) -> FloraConfig {
    params.flora_for_world()
}
