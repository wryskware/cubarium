//! Package V's one source of numbers: the model's crown ranges, stage profiles and founder
//! bodies as JSON, for the Blender bake (`scripts/blender/bake_voxel_models.py`)
//! (`design/handoffs/voxel-organism-models-2026-09-23.md` §1).
//!
//! ```text
//! cargo run -p cubarium --example voxel_model_source [-- OUT]
//! blender -b -P scripts/blender/bake_voxel_models.py
//! ```
//!
//! `OUT` defaults to `assets/voxel-models/source.json` under the repository. The numbers
//! are `FloraConfig::default()` and `FaunaConfig::default()`: crown ranges are metres at
//! every voxel size, so one dump serves every preset.

use std::path::PathBuf;

use anyhow::{Context, Result};
use cubarium::voxel::model::SourceDump;
use cubarium_voxel_fauna::FaunaConfig;
use cubarium_voxel_flora::FloraConfig;

fn main() -> Result<()> {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/voxel-models/source.json")
        });
    let dump = SourceDump::from_model(&FloraConfig::default(), &FaunaConfig::default());
    let mut json = serde_json::to_string_pretty(&dump)?;
    json.push('\n');
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&out, json).with_context(|| format!("writing {}", out.display()))?;
    eprintln!(
        "{}: {} species, {} founders, voxel sizes {:?} m",
        out.display(),
        dump.species.len(),
        dump.founders.len(),
        dump.voxel_sizes_m
    );
    Ok(())
}
