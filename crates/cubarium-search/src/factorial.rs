//! The controlled form × diet factorial (workstream J) — **stub**.
//!
//! Signatures only, so `tests/diet_factorial.rs` can be authored and committed against a
//! module that does nothing before the module that does something exists. Every function
//! here returns the emptiest legal answer; the definition tests are red against it.

use std::path::Path;

use cubarium_core::World;
use cubarium_core::genome::Genome;
use cubarium_surface::CellId;
use serde::{Deserialize, Serialize};

pub const CLONES: usize = 8;
pub const DIET_LOW: f32 = 0.60;
pub const DIET_HIGH: f32 = 0.85;
pub const GRAZER: u8 = 0;
pub const GLIDER: u8 = 1;
pub const BURROWER: u8 = 2;
pub const SKIMMER: u8 = 3;
pub const FORMS: [u8; 4] = [BURROWER, GRAZER, GLIDER, SKIMMER];
pub const FOLIAGE: usize = 0;
pub const FRUIT: usize = 1;
pub const LITTER: usize = 2;
pub const CARRION: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arm {
    A,
    B,
    C,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Design {
    pub ticks: u64,
    pub warm_up_ticks: u64,
    pub probe_every: u64,
    pub drain_every: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub cell: u16,
    pub wet: bool,
    pub mean_depth: f64,
    pub initial_foliage: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CloneSpec {
    pub slot: usize,
    pub cell: CellId,
    pub wet_start: bool,
    pub form: u8,
    pub genome: Genome,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CloneRow {
    pub slot: usize,
    pub diet: f64,
    pub cap_detrital: f64,
    pub births: u32,
    pub billed_ticks: u64,
    pub probes: u64,
    pub material_residual: f64,
    pub energy_residual: f64,
    pub digestible: [f64; 4],
    pub reserve_credit: [f64; 4],
    pub battery_credit: [f64; 4],
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArmRun {
    pub clones: Vec<CloneRow>,
    pub placements: Vec<Placement>,
    pub legacy_founders: usize,
    pub clone_births: u32,
    pub world_births: u64,
    pub final_state_hash: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Roster;

impl Roster {
    pub fn of(_world: &World) -> Result<Roster, String> {
        Err("not implemented".into())
    }

    pub fn genome(&self, _form: u8) -> &Genome {
        unimplemented!("stub")
    }
}

pub fn choose_cells(
    _mean_depth: &[f64],
    _foliage0: &[f64],
    _wet_min: f64,
    _dry_max: f64,
) -> Result<[Placement; CLONES], String> {
    Err("not implemented".into())
}

pub fn plan(_arm: Arm, _roster: &Roster, _cells: &[Placement; CLONES]) -> Vec<CloneSpec> {
    Vec::new()
}

pub fn run_one(_arm: Arm, _seed: u64, _design: Design) -> Result<ArmRun, String> {
    Err("not implemented".into())
}

pub fn run_one_without_ledger(_arm: Arm, _seed: u64, _design: Design) -> Result<ArmRun, String> {
    Err("not implemented".into())
}

pub fn run(
    _arms: &[Arm],
    _seeds: &[u64],
    _design: Design,
    _workers: usize,
    _out: &Path,
) -> Result<(), String> {
    Err("not implemented".into())
}
