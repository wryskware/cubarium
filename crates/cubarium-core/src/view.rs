//! Immutable render view published after each completed tick, and the observer's
//! raw field dump.

use serde::Serialize;

use cubarium_surface::{PathSegment, SurfacePoint, Vec2};

use crate::ids::OrganismId;
use crate::organism::Mode;

#[derive(Clone, Debug, PartialEq)]
pub struct OrganismView {
    pub id: OrganismId,
    pub pos: SurfacePoint,
    pub heading: Vec2,
    pub lobes: Vec<(f64, f64, f64)>,
    pub hue: f32,
    pub mode: Mode,
    pub fed: bool,
    pub juvenile: bool,
    /// Gestation progress in `[0, 1]` while an escrow is held, `None` otherwise.
    ///
    /// `(tick - escrow.started_tick) / gestation_ticks`, with `gestation_ticks` derived
    /// exactly as the world derives it (`round(organism.gestation_seconds / DT)`), so a
    /// renderer can show a birth coming without reading the world's escrow.
    pub gestation: Option<f32>,
    /// The heritable rig index (`design/fauna-v2.md`): the presenter draws this rig, falling
    /// back to the hue tercile for a form beyond its pack.
    pub form: u8,
    /// The segments traveled this tick (for renderer-side trails).
    pub moved: Vec<PathSegment>,
}

/// Everything the renderer needs; the renderer can never reach the world through it.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderView {
    pub tick: u64,
    pub producer: Vec<f64>,
    pub detritus: Vec<f64>,
    /// Fruit `F` per cell (m), `design/fauna-v2.md` "Fruit".
    pub fruit: Vec<f64>,
    /// Surface water depth per cell (d), `design/water.md`.
    pub water: Vec<f64>,
    /// This tick's rain rate per cell (d/s); zero outside the showers.
    pub rain: Vec<f32>,
    pub producer_max: f64,
    pub organisms: Vec<OrganismView>,
}

/// One observer field dump: every cell's material fields plus the live organism count in
/// it, in `CellId` index order (1,280 entries each). Analyzers pair it with
/// [`crate::World::cell_neighbors`] to walk the surface graph without this crate.
/// Values are exact; rounding for the wire is the host's concern.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FieldDump {
    pub tick: u64,
    pub n: Vec<f64>,
    pub p: Vec<f64>,
    pub d: Vec<f64>,
    pub de: Vec<f64>,
    /// Surface water depth per cell (d).
    pub w: Vec<f64>,
    /// Fruit `F` per cell (m).
    pub f: Vec<f64>,
    pub organisms: Vec<u16>,
}
