//! **The lanternjaw's patrol** (the detritus web D2,
//! `design/lanternjaw-detritus-web-proposal-2026-10-05.md` §3.2 with Astra's corrections,
//! `design/7_Research/astra-detritus-web-review-2026-10-05.md`). Tests-first stub.

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::FloraView;
use serde::{Deserialize, Serialize};

use crate::{Actions, Fauna, Founder, HuntConfig};

/// How many fruitless patches one hunter remembers at once (a cache size, not ecology).
pub const FRUITLESS_SLOTS: usize = 6;

/// What a patroller is doing ([`HuntMemory::mode`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HuntMode {
    /// No patch: the next sample chooses one.
    #[default]
    Idle,
    /// Walking to [`HuntMemory::patch`].
    Travel,
    /// Waiting still on it, the lure lit.
    Wait,
}

/// One remembered fruitless patch: its centre, metres, and the tick it is forgotten at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FruitlessPatch {
    pub x: f64,
    pub z: f64,
    pub until_tick: u64,
}

/// A patroller's memory ([`crate::Animal::hunt`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HuntMemory {
    pub mode: HuntMode,
    /// The patch centre it walks to or waits on, metres `(x, z)`.
    pub patch: (f64, f64),
    /// Controller samples in the current mode.
    pub samples: u32,
    /// Samples of an escape from a refused motor left.
    pub escape: u32,
    pub fruitless: [FruitlessPatch; FRUITLESS_SLOTS],
    /// **The search image**: seconds of contact with size-eligible prey per lineage, decayed
    /// with [`HuntConfig::search_memory_s`].
    pub image: [f64; Founder::COUNT],
    /// The tick the image was last decayed to.
    pub image_tick: u64,
}

impl HuntMemory {
    /// Decay the image to `tick`, then add `dt_s` of contact to every lineage `met`.
    pub fn meet(&mut self, _cfg: &HuntConfig, _tick: u64, _met: &[bool; Founder::COUNT], _dt_s: f64) {}

    /// Remember a fruitless patch at `(x, z)` until `until_tick`.
    pub fn remember_fruitless(&mut self, _x: f64, _z: f64, _until_tick: u64) {}

    /// Whether `(x, z)` is a remembered fruitless patch at `tick`.
    pub fn is_fruitless(&self, _cfg: &HuntConfig, _x: f64, _z: f64, _tick: u64, _ring: f64) -> bool {
        false
    }
}

/// **The attack preference** for a prey lineage met for `encounter_s` seconds of contact
/// lately.
pub fn attack_preference(_cfg: &HuntConfig, _encounter_s: f64) -> f64 {
    1.0
}

/// The patrol's answer for the due body at `i`, or `None` to leave its controller's.
pub(crate) fn steer(
    _fauna: &mut Fauna,
    _i: usize,
    _view: &VoxelView<'_>,
    _fv: &FloraView<'_>,
    _occupancy: Option<&crate::senses::ConeOccupancy>,
) -> Option<(Actions, f64)> {
    None
}
