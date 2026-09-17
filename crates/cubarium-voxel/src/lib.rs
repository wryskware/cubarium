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

pub mod config;
pub mod generate;
pub mod ledger;
pub mod material;
pub mod snapshot;
pub mod water;
pub mod world;

pub use config::Config;
pub use ledger::Ledger;
pub use material::Material;
pub use world::{Command, VoxelView, World};

/// Simulation ticks per second. Matches `cubarium-core::TICK_HZ` so a frontend can
/// drive both worlds with the same clock.
pub const TICK_HZ: u32 = 20;
/// Seconds per tick.
pub const DT: f64 = 1.0 / TICK_HZ as f64;
