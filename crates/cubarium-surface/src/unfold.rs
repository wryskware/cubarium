//! Local straight-line unfoldings: surface distance and chart images for nearby points.

use crate::{Edge, Face, SurfacePoint, TangentMap, Vec2};

/// Largest supported query radius in pixels. Within this radius every shortest path
/// crosses at most [`MAX_SEAMS`] seams (both points are well inside one face width of
/// each other, and the open bottom offers no shortcut), so enumerating chart paths of
/// that length is complete. Callers asking for more get a panic, not a wrong answer.
pub const MAX_LOCAL_RADIUS: f64 = 32.0;

/// Seam crossings enumerated per chart path.
pub const MAX_SEAMS: u8 = 2;

/// A sequence of at most [`MAX_SEAMS`] seam crossings starting in the observer's chart.
/// Each step names the chart being exited and the edge it exits through; the entry
/// chart/edge follow from `Face::neighbor`. Ordering is lexicographic on `(len, steps)`,
/// which is the tie-break rule for equal-length unfoldings; `direct()` sorts first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChartPath {
    pub len: u8,
    pub steps: [(Face, Edge); MAX_SEAMS as usize],
}

impl ChartPath {
    /// The empty path: target and observer share a chart.
    pub const fn direct() -> ChartPath {
        ChartPath { len: 0, steps: [(Face::Front, Edge::Top); MAX_SEAMS as usize] }
    }

    /// The steps actually taken.
    pub fn steps(&self) -> &[(Face, Edge)] {
        &self.steps[..self.len as usize]
    }

    /// The chart the path ends in, starting from `observer_face`.
    pub fn final_face(&self, observer_face: Face) -> Face {
        let mut f = observer_face;
        for &(face, edge) in self.steps() {
            debug_assert_eq!(face, f);
            f = face.neighbor(edge).expect("chart paths never cross the open rim").face;
        }
        f
    }
}

/// The affine image of a target chart inside the observer's chart along one path:
/// a point `p` in the target chart appears at `origin + map.apply(p)` in observer
/// coordinates, and a target tangent `t` appears as `map.apply(t)`. `map` is always a
/// pure rotation (paths never reflect).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartImage {
    pub path: ChartPath,
    pub target_face: Face,
    pub origin: Vec2,
    pub map: TangentMap,
}

impl ChartImage {
    /// Observer-chart position of a target-chart point.
    #[inline]
    pub fn image_point(&self, p: Vec2) -> Vec2 {
        self.origin + self.map.apply(p)
    }

    /// Target-chart position of an observer-chart point (inverse of `image_point`).
    #[inline]
    pub fn preimage_point(&self, q: Vec2) -> Vec2 {
        self.map.inverse().apply(q - self.origin)
    }
}

/// All chart images reachable from `observer_face` by at most `max_seams` (≤ [`MAX_SEAMS`])
/// seam crossings, including the identity image of the observer's own chart, in
/// [`ChartPath`] order. Paths never cross the open rim and never immediately return
/// through the edge they entered by. The same target face may appear under several
/// paths (for example Top via Front→Top and via Front→Right→Top); all are kept because
/// each is the valid image for a different region.
///
/// Derivation of a one-seam image (normative): for the observer chart exiting through
/// edge `e` into `(n, e2, reversed)`, the rotation `R` is the quarter-turn count from
/// `cross_seam(observer_face, e, 0)`; the image rotation is `R.inverse()` (it carries
/// target-chart tangents back into the observer chart); `origin` is fixed by requiring
/// that the entry point at along-edge parameter `s'` on `e2` maps onto the exit point at
/// parameter `s` on `e` (with `s' = 64 - s` when reversed) for `s = 0`, and debug-checked
/// for `s = 64`. Two-seam images compose the second chart's image within the first.
pub fn chart_images(observer_face: Face, max_seams: u8, out: &mut Vec<ChartImage>) {
    let _ = (observer_face, max_seams, out);
    todo!("chart_images")
}

/// A target point seen from an observer through its shortest valid unfolding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unfolded {
    /// Target position in observer-chart coordinates.
    pub local: Vec2,
    /// Transports target-chart tangents into the observer chart.
    pub map: TangentMap,
    /// Surface distance in pixels.
    pub distance: f64,
    pub path: ChartPath,
}

/// True when the straight segment from `observer` to `image` (both in the observer's
/// chart coordinates) leaves and enters charts exactly through the edges `path` lists,
/// in order, and ends inside the final chart.
///
/// Normative check: sweep the segment chart by chart. In each chart (expressed in that
/// chart's own coordinates through the running affine map) the segment must exit through
/// the listed edge (earliest exit, ties within `GEOM_EPS` accepted if the listed edge is
/// among the tied edges) and, for the final chart, must end inside `[0, 64]²` (boundary
/// inclusive within `GEOM_EPS`). The empty path is valid iff the segment stays inside the
/// observer's chart. Reflection is never valid for an unfolding: a segment that would leave
/// through the open rim is invalid.
pub fn segment_is_valid(observer_face: Face, observer: Vec2, image: Vec2, path: &ChartPath) -> bool {
    let _ = (observer_face, observer, image, path);
    todo!("segment_is_valid")
}

/// Shortest valid unfolding of `target` as seen from `observer`, or `None` when no valid
/// unfolding has length ≤ `max_distance`. Panics if `max_distance > MAX_LOCAL_RADIUS`.
/// Candidates are pre-rejected when the 3D chord exceeds `max_distance` (with a `GEOM_EPS`
/// margin). Equal-length candidates (within `GEOM_EPS`) resolve by `ChartPath` order.
/// Both points must be canonical.
pub fn unfold(observer: SurfacePoint, target: SurfacePoint, max_distance: f64) -> Option<Unfolded> {
    let mut images = Vec::new();
    chart_images(observer.face, MAX_SEAMS, &mut images);
    unfold_with(&images, observer, target, max_distance)
}

/// [`unfold`] using precomputed `chart_images(observer.face, MAX_SEAMS, ..)`; the caller
/// caches those per face to keep the hot path allocation-free.
pub fn unfold_with(images: &[ChartImage], observer: SurfacePoint, target: SurfacePoint, max_distance: f64) -> Option<Unfolded> {
    let _ = (images, observer, target, max_distance);
    todo!("unfold_with")
}

/// Surface distance between two canonical points if it is ≤ `max_distance`.
/// Symmetric: `surface_distance(a, b, r) == surface_distance(b, a, r)` within `GEOM_EPS`.
pub fn surface_distance(a: SurfacePoint, b: SurfacePoint, max_distance: f64) -> Option<f64> {
    unfold(a, b, max_distance).map(|u| u.distance)
}
