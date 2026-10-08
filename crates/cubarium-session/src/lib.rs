//! **A running Cubarium world, presentation-agnostic.**
//!
//! Two hosts stand on this crate:
//! - `cubarium voxel`, the ambient display, which keeps its own frame-paced loop (the GPU
//!   renderer reads the world between ticks on the loop's thread) and takes from here the
//!   one copy of everything that is not drawing: the parameter set ([`params`]), making a
//!   world ([`create`], [`founders`]), saves ([`store`]), the console and the pacing
//!   ([`console`]);
//! - [`Session`], which owns a world and its loop on a thread of its own, for Godot (the
//!   `cubarium-godot` extension) and any other reader: run control, candidates, saves and
//!   loads, queries over published snapshots, and the view feed ([`feed`]). Nothing a
//!   reader does can block a tick or lock the world mid-step: the loop hands everything
//!   off over a channel, and readers only ever lock what a second thread fills.
//!
//! No presentation lives here: no meshes, colours, poses or cameras, and no Linux-only
//! dependency, so it builds for macOS.

pub mod console;
pub mod create;
pub mod feed;
pub mod founders;
pub mod params;
mod session;
pub mod store;

#[cfg(test)]
mod tests;

pub use console::{COMMANDS, Console, MAX_SPEED, MIN_SPEED, Pacer, console_line};
pub use feed::{ChunkData, Frame, View};
pub use params::{SimParams, WorldSpec};
pub use session::{CandidateInfo, Notice, Prepared, Session, SessionOptions, Status, prepare_loaded, prepare_new};
pub use store::{LoadError, SaveMeta, SlotInfo};

use cubarium_voxel_fauna::{Founder, MobilityMode, State};
use serde_json::{Value, json};

/// [`feed::Animals::state`]'s codes, in this order.
pub const STATES: [State; 5] = [State::Cropping, State::Walking, State::Resting, State::Climbing, State::Asleep];
/// [`feed::Animals::mobility`]'s codes, in this order.
pub const MOBILITY: [MobilityMode; 6] = [
    MobilityMode::Ground,
    MobilityMode::Wall,
    MobilityMode::Climb,
    MobilityMode::Fly,
    MobilityMode::Glide,
    MobilityMode::Perched,
];

/// **What the feed's ids mean**: every lineage and plant species by id, and the names of
/// the state, mobility, stage and material codes and the flag bits. A presentation maps
/// these to its own assets; a new species adds a row here and nothing to the binding.
pub fn species_table() -> Value {
    let animals: Vec<Value> = Founder::ALL
        .into_iter()
        .map(|f| {
            json!({
                "id": f.index(),
                "name": f.name(),
                "label": cubarium_observatory::query::lineage_label(f),
            })
        })
        .collect();
    let plants: Vec<Value> = cubarium_voxel_flora::Species::ALL
        .into_iter()
        .map(|s| json!({ "id": s.index(), "name": s.name() }))
        .collect();
    let states: Vec<&str> = STATES
        .into_iter()
        .map(cubarium_observatory::query::state_name)
        .collect();
    let mobility: Vec<String> = MOBILITY
        .into_iter()
        .map(|m| format!("{m:?}").to_ascii_lowercase())
        .collect();
    let stages: Vec<&str> = feed::STAGES.into_iter().map(|s| s.name()).collect();
    json!({
        "animals": animals,
        "plants": plants,
        "states": states,
        "mobility": mobility,
        "stages": stages,
        "flags": {
            "striking": feed::FLAG_STRIKING,
            "riding": feed::FLAG_RIDING,
            "hunkered": feed::FLAG_HUNKERED,
            "yielding": feed::FLAG_YIELDING,
            "alert": feed::FLAG_ALERT,
            "wounded": feed::FLAG_WOUNDED,
        },
        "materials": ["air", "bedrock", "rock", "soil"],
        "chunk": feed::CHUNK,
    })
}
