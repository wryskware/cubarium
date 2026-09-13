//! Read-only checkpoint verification for a controlled rollout.
//! `cargo run -p cubarium --example inspect_snapshot -- PATH [--state-json]`
//! This tool never instantiates a running World or writes to the state directory.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use cubarium_core::{decode_snapshot, snapshot::state_hash};

#[derive(Parser)]
struct Args {
    /// Exact snapshot file to validate (not a directory or highest-tick guess).
    path: PathBuf,
    /// Include the complete decoded state for explicit migration comparisons.
    #[arg(long)]
    state_json: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let bytes =
        std::fs::read(&args.path).with_context(|| format!("reading {}", args.path.display()))?;
    let (meta, state) = decode_snapshot(&bytes).context("validating snapshot")?;
    let organism_material: f64 = state.organisms.iter().map(|(_, o)| o.material()).sum();
    let mut result = serde_json::json!({
        "path": args.path,
        "metadata": meta,
        "tick": state.tick,
        "seed": state.config.seed,
        "population": state.organisms.len(),
        "state_hash": state_hash(&state).to_string(),
        "ecology_hash": cubarium_core::ecology_hash(&state).to_string(),
        "care": state.care,
        "material": state.fields.total_material() + organism_material,
        "nutrient": state.fields.n.iter().sum::<f64>(),
        "producer": state.fields.p.iter().sum::<f64>(),
        "detritus": state.fields.d.iter().sum::<f64>(),
        "detritus_energy": state.fields.de.iter().sum::<f64>(),
        "fruit": state.fields.f.iter().sum::<f64>(),
        "water": state.fields.w.iter().sum::<f64>(),
        "organism_material": organism_material,
        "note": "Decoded and validated only; this does not reset or infer historical accounting baselines."
    });
    if args.state_json {
        result["state"] = serde_json::to_value(&state).context("serializing decoded state")?;
    }
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
