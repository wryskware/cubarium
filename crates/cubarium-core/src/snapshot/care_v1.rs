//! The frozen **pre-dose** care shape, as schemas 8 through 11 wrote it.
//!
//! `CareState` and `ActiveShower` are nested inside every one of those payloads. When the
//! adjustable dose added `ActiveShower::dose_permille` (schema 12), the live types' postcard
//! layout changed — so the older mirrors cannot go on borrowing them. These are the field
//! lists exactly as they stood at commit `e55501d`, the last pre-dose build. **Never change
//! them.** A `#[serde(default)]` would not have helped: postcard is not self-describing, so a
//! missing trailing field is not a missing field, it is the next value read from the wrong
//! offset.
//!
//! Migration is one direction only. A legacy shower has no dose because every dose it could
//! have had was the standard one, so it opens at [`CareDose::STANDARD`] — that is what it was,
//! not a guess. Going the other way is where the honesty matters: [`project`] refuses a
//! **nonstandard** active shower rather than writing a legacy shower that silently drops the
//! amount somebody asked for.

use serde::{Deserialize, Serialize};

use crate::care::{ActiveShower, CareDose, CareState};

/// `ActiveShower` as schemas 8–11 wrote it: no dose. Frozen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveShowerV1 {
    pub seq: u64,
    pub apply_after_tick: u64,
    pub cells: Vec<u16>,
    pub weights: Vec<f64>,
    pub delivered: u32,
}

/// `CareState` as schemas 8–11 wrote it. Frozen.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CareStateV1 {
    pub admitted_seq: u64,
    pub showers: Vec<ActiveShowerV1>,
    pub feed_material_in: f64,
    pub feed_energy_in: f64,
    pub rain_depth_in: f64,
    pub clean_material_out: f64,
    pub clean_energy_out: f64,
    pub allowance_used: f64,
}

/// Migration: a pre-dose world's care, with every in-flight shower opened at the standard
/// dose — the only dose those builds could deliver.
impl From<CareStateV1> for CareState {
    fn from(old: CareStateV1) -> CareState {
        CareState {
            admitted_seq: old.admitted_seq,
            showers: old
                .showers
                .into_iter()
                .map(|s| ActiveShower {
                    seq: s.seq,
                    apply_after_tick: s.apply_after_tick,
                    cells: s.cells,
                    weights: s.weights,
                    delivered: s.delivered,
                    dose_permille: CareDose::STANDARD_PERMILLE,
                })
                .collect(),
            feed_material_in: old.feed_material_in,
            feed_energy_in: old.feed_energy_in,
            rain_depth_in: old.rain_depth_in,
            clean_material_out: old.clean_material_out,
            clean_energy_out: old.clean_energy_out,
            allowance_used: old.allowance_used,
        }
    }
}

/// The pre-dose image of a current care state, or `None` when there is no honest one.
///
/// **Refuses** a world whose active shower carries a nonstandard dose. The old shape has
/// nowhere to put the amount, and a projection that dropped it would claim two different
/// worlds are the same world — exactly the comparison these projections exist to make
/// trustworthy. Every other care value, including the cumulative ledgers a nonstandard command
/// already moved, carries across unchanged: the ledgers are amounts, not shapes.
pub fn project(care: &CareState) -> Option<CareStateV1> {
    let mut showers = Vec::with_capacity(care.showers.len());
    for s in &care.showers {
        if !s.dose().is_standard() {
            return None;
        }
        showers.push(ActiveShowerV1 {
            seq: s.seq,
            apply_after_tick: s.apply_after_tick,
            cells: s.cells.clone(),
            weights: s.weights.clone(),
            delivered: s.delivered,
        });
    }
    Some(CareStateV1 {
        admitted_seq: care.admitted_seq,
        showers,
        feed_material_in: care.feed_material_in,
        feed_energy_in: care.feed_energy_in,
        rain_depth_in: care.rain_depth_in,
        clean_material_out: care.clean_material_out,
        clean_energy_out: care.clean_energy_out,
        allowance_used: care.allowance_used,
    })
}
