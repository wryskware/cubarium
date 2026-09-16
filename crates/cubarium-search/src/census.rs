//! Workstream R — F's variety census with one locus of the founder roster moved.
//!
//! Stub. The definitions are in
//! `design/7_Research/ecology-v1-depth-census-2026-09-16.md`; the tests in
//! `crates/cubarium-search/tests/depth_census.rs` are written against them and run red here.

use std::path::Path;

use cubarium_core::World;
use serde::{Deserialize, Serialize};

use crate::evaluate::Protocol;
use crate::movement::{Census, Crossings, FounderBroods, Margins};

pub const DEPTH_CONTROL: f32 = 0.10;
pub const DEPTH_TREATMENT: f32 = 0.55;
pub const CONFIGURATIONS: [&str; 2] = ["baseline", "fast-leaf"];
pub const ARMS: [u32; 3] = [0, 1, 2];
pub const DEPTH_BANDS: [f64; 4] = [1e-3, 0.05, 0.15, f64::INFINITY];

pub fn depth_band(_depth: f64) -> usize {
    0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    pub bodies: u64,
    pub depth_before: f32,
    pub depth_after: f32,
    pub h_pref_before: f64,
    pub h_pref_after: f64,
}

pub fn apply_depth_override(_world: &mut World, _depth: f32) -> Result<Applied, String> {
    Err("stub".into())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Job {
    pub candidate: &'static str,
    pub depth: f32,
    pub seed: u64,
    pub arm: u32,
}

pub fn plan(_seeds: &[u64]) -> Vec<Job> {
    Vec::new()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub final_state_hash: u64,
    pub ticks_run: u64,
    pub prey_births: u64,
    pub prey_deaths: u64,
    pub deaths_by_cause: [u64; 4],
    pub override_bodies: u64,
    pub census: Census,
    pub founder_broods: FounderBroods,
    pub crossings: Crossings,
    pub margins: Margins,
}

pub fn run_one(
    _candidate: &str,
    _seed: u64,
    _arm: u32,
    _depth: f32,
    _protocol: Protocol,
) -> Result<Row, String> {
    Err("stub".into())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunFacts {
    pub seed: u64,
    pub arm: u32,
    pub skimmer_alive_final: u64,
    pub skimmer_births: u64,
    pub founder_forms_alive: u64,
    pub top_form_share: f64,
    pub alive_by_form: [u64; 5],
    pub founder_skimmer_mean_lifetime_seconds: f64,
    pub skimmer_entered: u64,
    pub skimmer_bin2_entered: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Clause {
    pub name: String,
    pub holds: bool,
    pub runs_agreeing: usize,
    pub seeds_agreeing: usize,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Confirmed,
    Refuted,
    #[default]
    Partial,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Assessment {
    pub runs: usize,
    pub lineage: Clause,
    pub monoculture: Clause,
    pub variety_harmed: Clause,
    pub rescue_gone: Clause,
    pub diet_drift: Clause,
    pub verdict: Verdict,
}

impl Assessment {
    pub fn of(_control: &[RunFacts], _treatment: &[RunFacts]) -> Assessment {
        Assessment::default()
    }
}

pub fn command(
    _seeds: usize,
    _workers: usize,
    _protocol: Protocol,
    _wall_seconds: u64,
    _out: &Path,
) -> Result<(), String> {
    Err("stub".into())
}
