//! The voxel ringworld core: a horizontally periodic strip of fixed-size voxels with
//! materials, free water, soil pore water and an aquifer, stepped at a fixed rate.
//!
//! This crate never draws and never reads the clock. A frontend calls [`World::step`],
//! sends [`Command`]s, and reads [`World::view`]. Save/load is plain bytes.
//!
//! Coordinates: `x` along the strip (wraps), `y` up (0 is the solid foundation),
//! `z` into the habitat (0 is the viewing plane, `depth-1` the back wall).
//! Index order is x fastest, then z, then y: see [`Config::index`].

#![forbid(unsafe_code)]

/// Time a phase, with the `profile` feature on, and expand to the body alone without it.
/// `$phase` names a [`profile::Phase`] variant.
///
/// **Measurement only** (`design/7_Research/voxel-tick-profile-2026-09-18.md`): with the
/// feature off the timer statement is removed before name resolution, so neither the
/// module nor the variant has to exist.
#[macro_export]
macro_rules! voxel_phase {
    ($phase:ident, $body:block) => {{
        #[cfg(feature = "profile")]
        let _timer = $crate::profile::start($crate::profile::Phase::$phase);
        $body
    }};
}

pub mod config;
/// Hydraulic erosion on a heightfield. Generation only; geological time is iterations.
pub mod erosion;
pub mod generate;
/// Roofed void a body can stand inside: undercuts, galleries and shelves.
pub mod hollows;
pub mod ledger;
pub mod material;
/// Periodic noise in physical units. Generation only.
pub mod noise;
/// Per-phase timings and work counts, with the `profile` feature only. Never a rule.
#[cfg(feature = "profile")]
pub mod profile;
/// What a landscape is made of, in metres, and which generator builds it.
pub mod recipe;
pub mod snapshot;
mod sparse;
/// Is this world's water cycle one a habitat could live in? Reports, never rejects.
pub mod viability;
pub mod walk;
pub mod water;
pub mod world;

pub use config::Config;
pub use generate::{Budget, Heightfield, Volume};
pub use hollows::Hollow;
pub use ledger::Ledger;
pub use material::Material;
pub use recipe::{Erosion, Hollows, Landform, PRESETS, Preset, Recipe, Streams};
pub use viability::{BandResult, PoreBand, Viability, ViabilitySpec};
pub use world::{Command, VoxelView, World, default_threads};

/// Simulation ticks per second. Matches `cubarium-core::TICK_HZ` so a frontend can
/// drive both worlds with the same clock.
pub const TICK_HZ: u32 = 20;
/// Seconds per tick.
pub const DT: f64 = 1.0 / TICK_HZ as f64;
