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
    /// The segments traveled this tick (for renderer-side trails).
    pub moved: Vec<PathSegment>,
}

/// Everything the renderer needs; the renderer can never reach the world through it.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderView {
    pub tick: u64,
    pub producer: Vec<f64>,
    pub detritus: Vec<f64>,
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
    pub organisms: Vec<u16>,
}
