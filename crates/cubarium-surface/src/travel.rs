//! Swept continuous transport across seams with rim reflection.

use crate::{Face, SurfacePoint, TangentMap, Vec2};

/// Upper bound on seam crossings plus reflections in one [`travel`] call before the
/// forward-progress fallback engages. A displacement of `d` pixels can legitimately
/// cross at most about `d / 64 + 2` charts; the bound is generous so that only genuine
/// vertex loops trigger it.
pub const MAX_CROSSINGS: u32 = 64;

/// One straight piece of a swept path, entirely inside one chart. `from` and `to` are
/// chart coordinates; either may lie exactly on a boundary. Zero-length pieces are
/// never emitted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathSegment {
    pub face: Face,
    pub from: Vec2,
    pub to: Vec2,
}

impl PathSegment {
    #[inline]
    pub fn length(&self) -> f64 {
        (self.to - self.from).length()
    }
}

/// The result of sweeping a displacement along the surface.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Travel {
    /// Final canonical position.
    pub end: SurfacePoint,
    /// Net map from tangent vectors in the start chart (as they were before moving) to
    /// tangent vectors in the end chart. Apply it to heading, steering, and any local
    /// directional memory. Composed of seam rotations and `REFLECT_Y` rim reflections
    /// in the order they happened.
    pub map: TangentMap,
    /// The path actually traveled, one segment per chart visited, in order. The sum of
    /// segment lengths equals the displacement length unless `fallback` is set.
    pub segments: Vec<PathSegment>,
    /// Seam crossings performed.
    pub crossings: u32,
    /// Rim reflections performed.
    pub reflections: u32,
    /// Boundary hits resolved by the vertex tie rule (two edges within `GEOM_EPS`).
    pub ties: u32,
    /// True when the forward-progress bound engaged: the remaining displacement was
    /// discarded and the end point nudged inward by `NUDGE`. Never silent: callers
    /// count these.
    pub fallback: bool,
}

impl Default for SurfacePoint {
    fn default() -> Self {
        SurfacePoint::new(Face::Front, 0.0, 0.0)
    }
}

/// Sweep `displacement` (in the start chart's tangent coordinates) from `start`.
///
/// Algorithm (normative):
///
/// 1. `start` must be canonical. A non-finite displacement is treated as zero with
///    `fallback = true`.
/// 2. Repeat: find the earliest parameter `t ∈ [0, 1]` at which the remaining straight
///    segment leaves the chart `[0, 64]²` through an edge whose outward component of the
///    remaining displacement is positive. If none (or `t > 1`), the sweep ends inside the
///    chart: emit the final segment and stop.
/// 3. If two edges are hit within `GEOM_EPS` (pixel distance along the sweep), it is a
///    vertex tie: choose the lowest `Edge` index (`Top < Right < Bottom < Left`) and count
///    it in `ties`. Emit the segment up to the hit point, with the hit coordinate set
///    exactly to the boundary value.
/// 4. If the edge is a side face's `Edge::Bottom`: reflect. Negate the `y` component of the
///    remaining displacement, compose `REFLECT_Y` into `map`, stay in the chart at the hit
///    point (`v == 64`), count a reflection.
/// 5. Otherwise cross the seam given by `Face::neighbor`: the along-edge parameter `s`
///    (`u` for Top/Bottom edges, `v` for Left/Right edges) becomes `64 - s` when the seam
///    is reversed; the entry point lies on the neighbor's entry edge at that parameter
///    (`Top: (s', 0)`, `Right: (64, s')`, `Bottom: (s', 64)`, `Left: (0, s')`); the
///    remaining displacement rotates by the quarter turns `cross_seam(face, edge, 0)`
///    reports (the turn count does not depend on `t`); compose that rotation into `map`;
///    count a crossing.
/// 6. If `crossings + reflections` exceeds [`MAX_CROSSINGS`]: set `fallback`, drop the
///    remaining displacement, move each boundary coordinate of the current point inward
///    by `NUDGE`, and stop.
/// 7. Canonicalize `end` (a coordinate equal to 64 becomes `64.next_down()`).
///
/// Properties tests rely on: segment lengths sum to the displacement length; retracing
/// (`travel(end, map.apply(-displacement_remaining...))`) returns to the start away from
/// ties; speed and angles are preserved across seams; a straight path never tunnels
/// through the open bottom.
pub fn travel(start: SurfacePoint, displacement: Vec2) -> Travel {
    let mut out = Travel::default();
    travel_into(start, displacement, &mut out);
    out
}

/// [`travel`] into a reused buffer: clears `out.segments` (keeping its capacity) and
/// overwrites every field.
pub fn travel_into(start: SurfacePoint, displacement: Vec2, out: &mut Travel) {
    let _ = (start, displacement, out);
    todo!("travel_into")
}
