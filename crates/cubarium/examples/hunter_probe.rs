//! Read-only replay diagnostics from a copied trial snapshot. Never saves over
//! its input, attaches to a runner, imports a founder, or changes tuning.

#[path = "hunter_compare/eligibility.rs"]
mod eligibility;

use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use cubarium_core::{HunterEvent, World, decode_snapshot};
use serde_json::json;
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    snapshot: PathBuf,
    #[arg(long, value_parser=clap::value_parser!(u64).range(1..=5184000))]
    ticks: u64,
    /// Compare the entire resulting WorldState with an earlier frozen trial.
    #[arg(long)]
    expect_state_hash: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let (meta, state) = decode_snapshot(&fs::read(&args.snapshot)?)?;
    ensure!(
        state.care == Default::default(),
        "probe requires a care-free trial snapshot"
    );
    let opening_tick = state.tick;
    let opening_hash = cubarium_core::snapshot::state_hash(&state).to_string();
    let opening_members = eligibility::members(&state);
    let mut world = World::from_state(state).map_err(|e| anyhow!(e))?;
    let mut stats = eligibility::Eligibility::default();
    let mut attempts = BTreeMap::<String, u64>::new();
    let mut deaths = BTreeMap::<String, u64>::new();
    let mut reproduction = BTreeMap::<String, u64>::new();
    let mut captures = 0_u64;
    let mut offspring = 0_u64;
    for _ in 0..args.ticks {
        world.step();
        world.check_invariants().map_err(|e| anyhow!(e))?;
        stats.observe(&world.state);
        world.drain_events();
        for event in world.drain_hunter_events() {
            match event {
                HunterEvent::Attempt { outcome, .. } => {
                    *attempts.entry(outcome.as_str().to_owned()).or_default() += 1
                }
                HunterEvent::Capture { .. } => captures += 1,
                HunterEvent::Offspring { .. } => offspring += 1,
                HunterEvent::Death { cause, .. } => {
                    *deaths.entry(format!("{cause:?}")).or_default() += 1
                }
                HunterEvent::Reproduction { record, .. } => {
                    let value = serde_json::to_value(record)?;
                    *reproduction
                        .entry(
                            value["transaction"]
                                .as_str()
                                .expect("tagged record")
                                .to_owned(),
                        )
                        .or_default() += 1;
                }
            }
        }
    }
    let closing_hash = cubarium_core::snapshot::state_hash(&world.state).to_string();
    let identity = args
        .expect_state_hash
        .as_ref()
        .map(|expected| *expected == closing_hash);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "kind":"hunter-end-of-step-opportunity-probe",
            "build":concat!(env!("CARGO_PKG_VERSION"),"+",env!("CUBARIUM_GIT_HASH")),
            "input":args.snapshot,"schema":meta.schema,"opening_tick":opening_tick,
            "opening_state_hash":opening_hash,"opening_members":opening_members,
            "ticks":args.ticks,"closing_tick":world.tick(),"closing_state_hash":closing_hash,
            "expected_closing_state_hash":args.expect_state_hash,"trajectory_matches_frozen_trial":identity,
            "closing_members":eligibility::members(&world.state),"reproductive_opportunity":stats.summary(),
            "attempt_outcomes":attempts,"captures":captures,"offspring":offspring,
            "hunter_death_causes":deaths,"reproduction_record_counts":reproduction,
            "limits":"Read-only deterministic replay, not an independent resource audit or a replacement paired trial. End-of-step gates are diagnostic, not actual physiology-pass funding attempts."
        }))?
    );
    ensure!(
        identity != Some(false),
        "replay differs from the expected frozen trial state"
    );
    Ok(())
}
