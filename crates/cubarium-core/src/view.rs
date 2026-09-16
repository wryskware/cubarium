//! Immutable render view published after each completed tick, and the observer's
//! raw field dump.

use serde::Serialize;

use cubarium_surface::{PathSegment, Scale, SurfacePoint, Topology, Vec2};

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
    /// The shape of the world these cells and positions belong to
    /// (`design/flat-world-plan-2026-09-16.md` §3). A presenter reads it instead of assuming
    /// the cube: it decides the chart count, the cell grid and where a body may be drawn.
    pub topology: Topology,
    /// The world scale `S` the topology was measured with. Cells are `4·S` pixels wide, and
    /// a stamp's budget is `9·S`.
    pub scale: Scale,
    pub producer: Vec<f64>,
    pub detritus: Vec<f64>,
    /// Fruit `F` per cell (m), `design/fauna-v2.md` "Fruit".
    pub fruit: Vec<f64>,
    /// Ecology v1's pools (`design/ecology-v1-contract.md` §3.1). Carried from day one so
    /// the follow-up presentation task (§12) — persistent living structure, foliage loss and
    /// recovery on it, and dead wood — needs no core change. **Nothing draws them yet.**
    pub wood: Vec<f64>,
    pub plant_reserve: Vec<f64>,
    pub dead_wood: Vec<f64>,
    /// Animal remains `C` per cell (m); a presenter with no carcass look may sum it with
    /// `detritus` until it has one.
    pub carrion: Vec<f64>,
    /// Surface water depth per cell (d), `design/water.md`.
    pub water: Vec<f64>,
    /// This tick's rain rate per cell (d/s); zero outside the showers.
    pub rain: Vec<f32>,
    pub producer_max: f64,
    /// `W_max` from `config.plant.wood_max` (m): the most living wood one cell can hold.
    /// The presenter normalises `wood` and `dead_wood` against it, exactly as it normalises
    /// `producer` against `producer_max`. Read-only, like every other field here.
    pub wood_max: f64,
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
    /// Ecology v1's pools, exactly as the world holds them.
    pub wood: Vec<f64>,
    pub plant_reserve: Vec<f64>,
    pub dead_wood: Vec<f64>,
    pub carrion: Vec<f64>,
    pub carrion_energy: Vec<f64>,
    pub organisms: Vec<u16>,
}
