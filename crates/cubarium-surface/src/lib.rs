//! cubarium-surface — one connected surface, on either of two shapes.
//!
//! The shim's `cube-proto` crate owns the discrete cube contract: `Face`, `Edge`, `Seam`,
//! `Face::neighbor`, `cross_seam`, `rotate_heading`, and the cube embedding
//! `pixel_direction`. This crate extends that contract to continuous coordinates, adds a
//! second [`Topology`], and provides the local surface operations every Cubarium system
//! shares:
//!
//! * [`travel`]: swept continuous transport of a displacement across seams, with
//!   pure specular reflection at an open rim and a documented vertex tie rule.
//! * [`unfold`]: the shortest valid straight-line unfolding between two nearby
//!   points, giving surface distance, the target's image in the observer's chart,
//!   and the tangent map that transports the target's directions into that chart.
//! * [`unfold_pixels`]: every surface pixel within a radius of an anchor, each
//!   exactly once, owned by its shortest valid unfolding (used for rasterization).
//! * [`FieldGraph`] / [`ScalarField`]: the scalar field graph with reciprocal seam
//!   edges, conservative diffusion, and surface-distance deposits.
//!
//! # The two topologies
//!
//! Every operation takes a [`Topology`] by value, and the [`Topology::Cube`] arm of each
//! is the body that function had before a second shape existed.
//!
//! * **[`Topology::Cube`]** — five 64×64 charts (`Front, Right, Back, Left, Top`) joined
//!   by `cube_proto`'s seam table, with the four lower edges an open rim. `world_scale`
//!   is pinned to 1: the 32-pixel [`MAX_LOCAL_RADIUS`] and the 9-pixel stamp budget are
//!   completeness proofs about a 64-pixel chart, not tunables.
//! * **[`Topology::Ring`]** — one chart, `w × h` pixels, on [`Face::Front`]. **The left
//!   and right edges join; the top and bottom are solid.** It needs no new machinery:
//!   the vertical edge is a seam of the chart *to itself* (identity transport, a pure
//!   translation by `∓w`, zero quarter turns), so [`travel`] takes its existing seam
//!   branch, and the two rims reuse the existing `REFLECT_Y` bounce. A ring corner is a
//!   point where that seam meets a rim — precisely the cube's lower side corner, tie rule
//!   included. Under `Edge::Top = 0 < Right = 1 < Bottom = 2 < Left = 3`, the top-left,
//!   top-right and bottom-left corners reflect first and the bottom-right corner crosses
//!   the seam first.
//!
//!   [`unfold`] sees three images of the chart — the direct one and the translations by
//!   `+w` and `−w` — of which at most two can be in range, because
//!   [`Topology::validate`] requires `w >= 2·max_local_radius() + 2·CELL_PIXELS` and the
//!   two shifts are `2w` apart. [`Topology::chord_sq`] is then the exact wrapped distance
//!   `min(|Δu|, w − |Δu|)² + Δv²` rather than a conservative bound, and every field row is
//!   a ring: no corners, the top and bottom rows at degree 3, `downhill` `None` on the top
//!   row (the canopy) and the neighbour below everywhere else (which the bottom row does
//!   not have).
//!
//! [`Scale`] carries the ring's world scale `S`: cells are `4·S` pixels, stamps are `9·S`,
//! and the cylinder embedding divides by `32·S`, so a ring at 640×360 with `S = 2` has the
//! same cells, the same noise scale and the same ecology as one at 320×180 with `S = 1`.
//!
//! See `design/surface-topology.md` and `design/flat-world-plan-2026-09-16.md` §2, §5a.
//!
//! # Conventions
//!
//! * Charts are `cube_proto::Face` ids with indices Front=0, Right=1, Back=2, Left=3,
//!   Top=4; a ring uses Front alone. Local coordinates `(u, v)` span `[0, w) × [0, h)`;
//!   `+u` is image-right, `+v` is image-down. Pixel `(x, y)` has center
//!   `(x + 0.5, y + 0.5)`. Chart boundaries are at 0 and the extent. A transient boundary
//!   point may have a coordinate equal to the extent; a canonical stored point never does
//!   (see [`SurfacePoint::canonicalize`]).
//! * Quarter turns are counter-clockwise on screen as in `cube_proto::rotate_heading`:
//!   one turn maps `(dx, dy)` to `(dy, -dx)`.
//! * The only competing adjacency table in this crate is *none*: every cube seam fact is
//!   read from `Face::neighbor` and `cross_seam`, and the ring's one seam is stated once,
//!   in [`Topology::neighbor`]. Continuous along-edge reversal is `t -> edge_len - t`; the
//!   integer API's `63 - t` is for cube pixel indices only.
//! * Open rims — the cube's four bottom edges, a ring's top and bottom — carry no flux,
//!   have no neighbor, and reflect a moving point.
//!
//! Everything here is deterministic, allocation-free on the hot paths where an
//! `_into` variant exists, and free of wall-clock, I/O, and randomness.

#![forbid(unsafe_code)]

mod field;
mod geometry;
mod point;
mod raster;
mod travel;
mod unfold;
mod vec2;

pub use cube_proto::geometry::{cross_seam, rotate_heading};
pub use cube_proto::{Edge, Face, Seam};

pub use field::{CELLS_PER_FACE_EDGE, CUBE_CELL_COUNT, CellId, FieldGraph, ScalarField, cell_of, deposit, diffuse};
pub use geometry::{CELL_PIXELS, EMBED_PIXELS, FOOTPRINT_PIXELS, Scale, Topology, TopologyError};
pub use point::{FaceFrame, SurfacePoint, face_frame, pixel_neighbor};
pub use raster::{PixelImage, unfold_pixels};
pub use travel::{MAX_CROSSINGS, PathSegment, Travel, travel, travel_into};
pub use unfold::{ChartImage, ChartPath, MAX_LOCAL_RADIUS, MAX_SEAMS, Unfolded, chart_images, segment_is_valid, surface_distance, unfold, unfold_with};
pub use vec2::{TangentMap, Vec2};

/// Side length of one **cube** face chart in pixel units. A ring's chart extent is its
/// `w` and `h`; ask [`Topology::extent`] rather than this constant wherever either shape
/// is possible.
pub const FACE_EXTENT: f64 = 64.0;

/// Geometric tie epsilon in pixel units. Two boundary intersections closer than this
/// along a sweep are a tie and resolve by the lowest `Edge` index; a coordinate within
/// this distance of 0 or the chart extent counts as on that boundary for validity checks.
pub const GEOM_EPS: f64 = 1e-9;

/// Inward nudge, in pixels, applied only when [`travel`] hits its forward-progress
/// bound at a vertex. Far below visible resolution; every use is reported.
pub const NUDGE: f64 = 1e-6;
