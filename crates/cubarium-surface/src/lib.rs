//! cubarium-surface — one connected five-face cube surface.
//!
//! The shim's `cube-proto` crate owns the discrete contract: `Face`, `Edge`, `Seam`,
//! `Face::neighbor`, `cross_seam`, `rotate_heading`, and the cube embedding
//! `pixel_direction`. This crate extends that contract to continuous coordinates
//! and adds the local surface operations every Cubarium system shares:
//!
//! * [`travel`]: swept continuous transport of a displacement across seams, with
//!   pure specular reflection at the open bottom rim and a documented vertex tie rule.
//! * [`unfold`]: the shortest valid straight-line unfolding between two nearby
//!   points, giving surface distance, the target's image in the observer's chart,
//!   and the tangent map that transports the target's directions into that chart.
//! * [`unfold_pixels`]: every surface pixel within a radius of an anchor, each
//!   exactly once, owned by its shortest valid unfolding (used for rasterization).
//! * [`FieldGraph`] / [`ScalarField`]: the 16×16-cells-per-face scalar field graph
//!   with reciprocal seam edges, conservative diffusion, and surface-distance deposits.
//!
//! Conventions (from `design/surface-topology.md`):
//!
//! * Faces are `cube_proto::Face` charts with indices Front=0, Right=1, Back=2,
//!   Left=3, Top=4. Local coordinates `(u, v)` span `[0, 64)`; `+u` is image-right,
//!   `+v` is image-down. Pixel `(x, y)` has center `(x + 0.5, y + 0.5)`. Chart
//!   boundaries are at 0 and 64. A transient boundary point may have a coordinate
//!   equal to 64; a canonical stored point never does (see [`SurfacePoint::canonicalize`]).
//! * Quarter turns are counter-clockwise on screen as in `cube_proto::rotate_heading`:
//!   one turn maps `(dx, dy)` to `(dy, -dx)`.
//! * The only competing adjacency table in this crate is *none*: every seam fact is
//!   read from `Face::neighbor` and `cross_seam`. Continuous along-edge reversal is
//!   `t -> 64 - t`; the integer API's `63 - t` is for pixel indices only.
//! * The bottom edges of the four side faces are an open boundary: no flux, no
//!   neighbor, pure reflection for moving points.
//!
//! Everything here is deterministic, allocation-free on the hot paths where an
//! `_into` variant exists, and free of wall-clock, I/O, and randomness.

#![forbid(unsafe_code)]

mod field;
mod point;
mod raster;
mod travel;
mod unfold;
mod vec2;

pub use cube_proto::geometry::{cross_seam, rotate_heading};
pub use cube_proto::{Edge, Face, Seam};

pub use field::{CELL_COUNT, CELL_PIXELS, CELLS_PER_FACE_EDGE, CellId, FieldGraph, ScalarField, cell_of, deposit, diffuse};
pub use point::{FaceFrame, SurfacePoint, face_frame, pixel_neighbor};
pub use raster::{PixelImage, unfold_pixels};
pub use travel::{MAX_CROSSINGS, PathSegment, Travel, travel, travel_into};
pub use unfold::{ChartImage, ChartPath, MAX_LOCAL_RADIUS, MAX_SEAMS, Unfolded, chart_images, segment_is_valid, surface_distance, unfold, unfold_with};
pub use vec2::{TangentMap, Vec2};

/// Side length of one face chart in pixel units. Chart coordinates span `[0, FACE_EXTENT)`.
pub const FACE_EXTENT: f64 = 64.0;

/// Geometric tie epsilon in pixel units. Two boundary intersections closer than this
/// along a sweep are a tie and resolve by the lowest `Edge` index; a coordinate within
/// this distance of 0 or 64 counts as on that boundary for validity checks.
pub const GEOM_EPS: f64 = 1e-9;

/// Inward nudge, in pixels, applied only when [`travel`] hits its forward-progress
/// bound at a vertex. Far below visible resolution; every use is reported.
pub const NUDGE: f64 = 1e-6;
