//! **The observatory**: a running voxel world's stats, for a companion dashboard today and
//! an in-game UI (or a Godot GDExtension shell) later.
//!
//! Two halves, kept apart on purpose:
//! - **Transport-free** ([`query`], [`life`], [`history`], [`observer`]): plain functions
//!   and types over `&World`, `&Flora` and `&Fauna` that return serde values. Anything that
//!   holds the world calls them directly.
//! - **The server** ([`server`], feature `server`, on by default): an HTTP/1.1 + server-sent
//!   events server on its own threads, fed one [`observer::Publication`] at a time over a
//!   channel. It never sees the world, so it can never lock it mid-step, and a slow or
//!   absent reader never slows the loop. It serves the embedded dashboard at `/`.
//!
//! Commands ([`Command`]) travel the other way, as values; the host applies them at its
//! own command point through the same path its console uses.

pub mod history;
pub mod life;
pub mod observer;
pub mod query;
#[cfg(feature = "server")]
pub mod server;

#[cfg(test)]
mod tests;

pub use history::{History, HistoryConfig, Sample};
pub use life::{LifeBook, LifeEvent, LifeRecord, LifeRecorder, What};
pub use observer::{Cadence, Controls, Observer, Publication, RunState, Snapshot};

use serde::{Deserialize, Serialize};

/// A run command from a reader. The host turns it into its console's own command, so a
/// dashboard and a typed line change the run the same way.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    /// Set the speed multiplier (the host clamps it to its range).
    SetSpeed(f64),
    Pause,
    Resume,
    /// Double or halve the speed.
    Faster,
    Slower,
    /// Pause, and advance exactly this many ticks.
    Step(u32),
}
