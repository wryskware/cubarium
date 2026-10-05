//! **The sim's own parameter set**, loaded the way a study example loads it: a config file
//! with its `base = "<file>"` chain merged under it (the app's `config_chain`,
//! `crates/cubarium/src/voxel/mod.rs:745-775`), its `[world]`, `[flora]` and `[fauna]`
//! tables laid over the code defaults by [`cubarium_voxel_sim::params::Params::resolve_over`],
//! then `path=value` sets. No number is retyped here: the merge is the app's rule, and every
//! value comes out of the registry the sim validates against.

use std::path::Path;

use cubarium_voxel_sim::params::{Layers, Params};

/// A config file's table with its `base` chain merged under it: the file named by `base`
/// (relative to this one) is read first and this file's keys laid over it, tables merged
/// key by key and anything else replaced. Mirrors `config_chain` and `merge_table` in
/// `crates/cubarium/src/voxel/mod.rs:745-775`, which live in the app crate this one does not
/// depend on.
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
    let base_path = path.parent().unwrap_or(Path::new(".")).join(base);
    let mut merged = chain(&base_path, depth + 1)?;
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

/// **The resolved parameter set of a scenario file**, with `sets` (`path=value`) last: the
/// numbers a sim run under that file uses. `[world]` is read as the sim reads it (the
/// landform preset expanded, the default cell size), `[flora]` and `[fauna]` through their
/// registries, the whole validated.
pub fn load(path: &Path, sets: &[String]) -> Result<Params, String> {
    let table = config_table(path)?;
    let mut file = toml::Table::new();
    for key in ["world", "flora", "fauna"] {
        if let Some(v) = table.get(key) {
            file.insert(key.into(), v.clone());
        }
    }
    Params::resolve(&Layers { file: Some(file), sets: sets.to_vec(), ..Layers::default() })
}

/// The repository root, found from this crate's manifest directory: where the scenario
/// files live (`config/scenarios/*.toml`).
pub fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// [`load`] of a path relative to the repository root.
pub fn scenario(rel: &str, sets: &[String]) -> Result<Params, String> {
    load(&repo_root().join(rel), sets)
}
