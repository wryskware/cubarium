//! Local straight-line unfoldings: surface distance and chart images for nearby points.

use crate::travel::{earliest_exit, edge_point, seam_turns};
use crate::{Edge, FACE_EXTENT, Face, GEOM_EPS, Seam, SurfacePoint, TangentMap, Vec2};

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
    // Longer paths are not enumerated; asking for more is a caller bug, not a panic.
    debug_assert!(max_seams <= MAX_SEAMS, "max_seams {max_seams} exceeds MAX_SEAMS");
    let max_seams = max_seams.min(MAX_SEAMS);
    out.clear();
    out.push(ChartImage {
        path: ChartPath::direct(),
        target_face: observer_face,
        origin: Vec2::ZERO,
        map: TangentMap::IDENTITY,
    });
    if max_seams == 0 {
        return;
    }
    for e in Edge::ALL {
        let Some((seam, origin1, map1)) = seam_image(observer_face, e) else {
            continue;
        };
        out.push(ChartImage {
            path: ChartPath { len: 1, steps: [(observer_face, e), PATH_FILLER] },
            target_face: seam.face,
            origin: origin1,
            map: map1,
        });
        if max_seams < 2 {
            continue;
        }
        for e3 in Edge::ALL {
            // Never immediately return through the edge we just entered by.
            if e3 == seam.edge {
                continue;
            }
            let Some((seam2, origin2, map2)) = seam_image(seam.face, e3) else {
                continue;
            };
            out.push(ChartImage {
                path: ChartPath { len: 2, steps: [(observer_face, e), (seam.face, e3)] },
                target_face: seam2.face,
                // The second chart's image inside the first, carried into the observer's.
                origin: origin1 + map1.apply(origin2),
                map: map2.then(map1),
            });
        }
    }
    out.sort_by_key(|i| i.path);
}

/// The unused tail of a [`ChartPath`], so that ordering only ever compares real steps.
const PATH_FILLER: (Face, Edge) = (Face::Front, Edge::Top);

/// The image of the chart across `edge` inside `face`'s own chart.
///
/// `map` carries neighbour tangents into `face`'s chart (the inverse of the rotation a
/// travelling tangent picks up on the way out), and `origin` is fixed by making the
/// neighbour's entry point at along-edge parameter `s'` land on the exit point at `s`.
fn seam_image(face: Face, edge: Edge) -> Option<(Seam, Vec2, TangentMap)> {
    let seam = face.neighbor(edge)?;
    let turns = seam_turns(face, edge).expect("the seam exists");
    let map = TangentMap::quarter_turns(turns).inverse();

    let entry_param = |s: f64| if seam.reversed { FACE_EXTENT - s } else { s };
    let origin = edge_point(edge, 0.0) - map.apply(edge_point(seam.edge, entry_param(0.0)));

    debug_assert!(
        {
            // The far end of the shared edge must land on the far end of the exit edge,
            // which also pins both shared corners of the two charts.
            let far = origin + map.apply(edge_point(seam.edge, entry_param(FACE_EXTENT)));
            (far - edge_point(edge, FACE_EXTENT)).length() < 1e-9
        },
        "seam image {face:?}.{edge:?} -> {seam:?} does not match at s = 64"
    );
    debug_assert_eq!(map.det(), 1, "chart images never reflect");
    Some((seam, origin, map))
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
    if !observer.is_finite() || !image.is_finite() {
        return false;
    }
    let d_obs = image - observer;
    let sweep_len = d_obs.length();

    // Running image of the current chart inside the observer's chart.
    let mut origin = Vec2::ZERO;
    let mut map = TangentMap::IDENTITY;
    let mut face = observer_face;
    let mut t_enter = 0.0f64;

    for k in 0..path.len as usize {
        let (step_face, step_edge) = path.steps[k];
        if step_face != face {
            return false;
        }
        // The whole segment, in this chart's own coordinates.
        let inv = map.inverse();
        let here = inv.apply(observer - origin);
        let dir = inv.apply(d_obs);

        let Some(exit) = earliest_exit(here, dir, sweep_len) else {
            return false;
        };
        // The exit must lie on the segment, at or after this chart's entry.
        if (exit.t - 1.0) * sweep_len > GEOM_EPS || (t_enter - exit.t) * sweep_len > GEOM_EPS {
            return false;
        }
        // Ties within GEOM_EPS are accepted when the listed edge is among them.
        if !exit.includes(step_edge) {
            return false;
        }
        let Some((seam, origin_next, map_next)) = seam_image(face, step_edge) else {
            // The open rim is never a valid unfolding step.
            return false;
        };
        origin += map.apply(origin_next);
        map = map_next.then(map);
        face = seam.face;
        t_enter = exit.t;
    }

    // The endpoint must lie inside the final chart.
    let end = map.inverse().apply(image - origin);
    (-GEOM_EPS..=FACE_EXTENT + GEOM_EPS).contains(&end.x)
        && (-GEOM_EPS..=FACE_EXTENT + GEOM_EPS).contains(&end.y)
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
    assert!(
        max_distance <= MAX_LOCAL_RADIUS,
        "max_distance {max_distance} exceeds MAX_LOCAL_RADIUS"
    );
    debug_assert!(observer.is_canonical(), "unfold from non-canonical {observer:?}");
    debug_assert!(target.is_canonical(), "unfold to non-canonical {target:?}");

    let limit = max_distance + GEOM_EPS;
    // A 3D chord is a lower bound on surface distance: reject far pairs before unfolding.
    if observer.chord_sq(&target) > limit * limit {
        return None;
    }
    let here = observer.chart();
    let there = target.chart();

    let mut best: Option<Unfolded> = None;
    for img in images {
        if img.target_face != target.face {
            continue;
        }
        let local = img.image_point(there);
        let distance = (local - here).length();
        if distance > limit {
            continue;
        }
        // Images arrive in ChartPath order, so equal lengths keep the earlier path.
        if best.as_ref().is_some_and(|b| distance >= b.distance - GEOM_EPS) {
            continue;
        }
        if !segment_is_valid(observer.face, here, local, &img.path) {
            continue;
        }
        best = Some(Unfolded { local, map: img.map, distance, path: img.path });
    }
    best
}

/// Surface distance between two canonical points if it is ≤ `max_distance`.
/// Symmetric: `surface_distance(a, b, r) == surface_distance(b, a, r)` within `GEOM_EPS`.
pub fn surface_distance(a: SurfacePoint, b: SurfacePoint, max_distance: f64) -> Option<f64> {
    unfold(a, b, max_distance).map(|u| u.distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{travel, PathSegment};

    fn images(face: Face) -> Vec<ChartImage> {
        let mut v = Vec::new();
        chart_images(face, MAX_SEAMS, &mut v);
        v
    }

    /// A small deterministic xorshift so the property checks are reproducible.
    struct Rng(u64);
    impl Rng {
        fn next_f64(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn point(&mut self) -> SurfacePoint {
            let f = Face::ALL[(self.next_f64() * 5.0) as usize % 5];
            SurfacePoint::new(f, self.next_f64() * 64.0, self.next_f64() * 64.0).canonicalize()
        }
        /// A point within roughly `reach` pixels of `a`, so that pairs land in range
        /// often and seams and vertices get hit.
        fn near(&mut self, a: SurfacePoint, reach: f64) -> SurfacePoint {
            let angle = self.next_f64() * std::f64::consts::TAU;
            let len = self.next_f64() * reach;
            travel(a, Vec2::from_screen_angle(angle) * len).end
        }
    }

    #[test]
    fn chart_image_counts_and_order() {
        // Front: itself, three seams (Bottom is open), and the two-seam routes.
        assert_eq!(images(Face::Front).len(), 11);
        assert_eq!(images(Face::Right).len(), 11);
        assert_eq!(images(Face::Back).len(), 11);
        assert_eq!(images(Face::Left).len(), 11);
        // Top: itself, four seams, and two continuations from each side face.
        assert_eq!(images(Face::Top).len(), 13);

        for face in Face::ALL {
            let v = images(face);
            assert_eq!(v[0].path, ChartPath::direct());
            assert_eq!(v[0].target_face, face);
            for w in v.windows(2) {
                assert!(w[0].path < w[1].path, "images are in ChartPath order");
            }
            for img in &v {
                assert_eq!(img.path.final_face(face), img.target_face);
                assert_eq!(img.map.det(), 1, "chart images never reflect");
                // The identity image is exactly the identity.
                if img.path.len == 0 {
                    assert_eq!(img.origin, Vec2::ZERO);
                    assert_eq!(img.map, TangentMap::IDENTITY);
                }
                // image_point / preimage_point round-trip.
                let p = Vec2::new(11.25, 47.5);
                let q = img.preimage_point(img.image_point(p));
                assert!((q - p).length() < 1e-9);
            }
        }
    }

    #[test]
    fn one_seam_images_place_the_neighbour_where_the_net_does() {
        let v = images(Face::Front);
        let find = |e: Edge| {
            *v.iter()
                .find(|i| i.path.len == 1 && i.path.steps[0] == (Face::Front, e))
                .expect("one-seam image")
        };
        // Right sits to the right of Front with no twist.
        let right = find(Edge::Right);
        assert_eq!(right.target_face, Face::Right);
        assert_eq!(right.map, TangentMap::IDENTITY);
        assert_eq!(right.image_point(Vec2::new(0.0, 20.0)), Vec2::new(64.0, 20.0));
        // Top sits above Front with no twist.
        let top = find(Edge::Top);
        assert_eq!(top.target_face, Face::Top);
        assert_eq!(top.map, TangentMap::IDENTITY);
        assert_eq!(top.image_point(Vec2::new(10.0, 64.0)), Vec2::new(10.0, 0.0));
        // Left sits to the left of Front.
        let left = find(Edge::Left);
        assert_eq!(left.target_face, Face::Left);
        assert_eq!(left.image_point(Vec2::new(64.0, 20.0)), Vec2::new(0.0, 20.0));
    }

    #[test]
    fn twisted_top_seam_image_matches_the_design_example() {
        // Right (10, 0.25) stepping up by 0.5 lands on Top (63.75, 54): so the Top point
        // (63.75, 54) must have a Right-chart image at (10, -0.25).
        let v = images(Face::Right);
        let top = *v
            .iter()
            .find(|i| i.path.len == 1 && i.path.steps[0] == (Face::Right, Edge::Top))
            .expect("Right.Top image");
        assert_eq!(top.target_face, Face::Top);
        let local = top.image_point(Vec2::new(63.75, 54.0));
        assert!((local - Vec2::new(10.0, -0.25)).length() < 1e-9, "{local:?}");
        // Up on Top-leftward, i.e. Top (-1, 0), reads as Right (0, -1).
        assert_eq!(top.map.apply(Vec2::new(-1.0, 0.0)), Vec2::new(0.0, -1.0));
    }

    #[test]
    fn direct_distance_inside_one_chart() {
        let a = SurfacePoint::new(Face::Front, 10.0, 10.0);
        let b = SurfacePoint::new(Face::Front, 13.0, 14.0);
        let u = unfold(a, b, 12.0).expect("in range");
        assert_eq!(u.path, ChartPath::direct());
        assert!((u.distance - 5.0).abs() < 1e-12);
        assert_eq!(u.map, TangentMap::IDENTITY);
        assert_eq!(u.local, Vec2::new(13.0, 14.0));
        assert!(unfold(a, b, 4.0).is_none(), "out of range");
    }

    #[test]
    fn one_pixel_across_a_vertical_seam() {
        let a = SurfacePoint::pixel_center(Face::Front, 63, 20);
        let b = SurfacePoint::pixel_center(Face::Right, 0, 20);
        let u = unfold(a, b, 12.0).expect("adjacent pixels");
        assert!((u.distance - 1.0).abs() < 1e-12, "{}", u.distance);
        assert_eq!(u.path.steps(), &[(Face::Front, Edge::Right)]);
    }

    #[test]
    fn one_pixel_across_a_twisted_top_seam() {
        let a = SurfacePoint::pixel_center(Face::Right, 10, 0);
        let (f, x, y, _) = crate::cross_seam(Face::Right, Edge::Top, 10).expect("seam");
        assert_eq!(f, Face::Top);
        let b = SurfacePoint::pixel_center(f, x, y);
        let u = unfold(a, b, 12.0).expect("adjacent pixels");
        assert!((u.distance - 1.0).abs() < 1e-12, "{}", u.distance);
        assert_eq!(u.path.steps(), &[(Face::Right, Edge::Top)]);
        // The heading transport matches what travelling across the seam does.
        let t = travel(a, u.local - a.chart());
        assert_eq!(t.map.inverse(), u.map);
    }

    #[test]
    fn unfolding_agrees_with_travel() {
        let mut rng = Rng(0x0f1e_2d3c_4b5a_6978);
        let mut checked = 0u32;
        for i in 0..40_000 {
            let a = rng.point();
            let b = if i % 2 == 0 { rng.near(a, 14.0) } else { rng.point() };
            let Some(u) = unfold(a, b, 12.0) else { continue };
            // Walking the unfolded straight segment must actually arrive at the target.
            let t = travel(a, u.local - a.chart());
            if t.ties > 0 {
                continue;
            }
            assert!(!t.fallback);
            assert_eq!(t.reflections, 0, "an unfolding never reflects: {a:?} {b:?}");
            assert_eq!(t.crossings, u32::from(u.path.len), "{a:?} {b:?} {:?}", u.path);
            assert_eq!(t.end.face, b.face, "{a:?} {b:?}");
            assert!(
                (t.end.u - b.u).abs() < 1e-7 && (t.end.v - b.v).abs() < 1e-7,
                "{a:?} -> {b:?} landed on {:?}",
                t.end
            );
            // The tangent map is the inverse of the transport along that walk.
            assert_eq!(t.map.inverse(), u.map, "{a:?} {b:?}");
            // The reported distance is the swept length.
            let swept: f64 = t.segments.iter().map(PathSegment::length).sum();
            assert!((swept - u.distance).abs() < 1e-7);
            // And it is at least the 3D chord.
            assert!(u.distance * u.distance >= a.chord_sq(&b) - 1e-6);
            checked += 1;
        }
        assert!(checked > 2000, "only {checked} pairs were in range");
    }

    #[test]
    fn unfolding_is_never_longer_than_a_travelled_path() {
        // Every path the sweep can actually walk is an upper bound on surface distance,
        // and within the radius the two-seam enumeration must find something at least
        // that short.
        let mut rng = Rng(0x5151_2626_3737_4848);
        for _ in 0..20_000 {
            let a = rng.point();
            let angle = rng.next_f64() * std::f64::consts::TAU;
            let len = rng.next_f64() * 12.0;
            let t = travel(a, Vec2::from_screen_angle(angle) * len);
            if t.fallback || t.ties > 0 {
                continue;
            }
            let d = surface_distance(a, t.end, 12.0);
            let d = d.unwrap_or_else(|| panic!("no unfolding for {a:?} -> {:?} (walked {len})", t.end));
            assert!(d <= len + 1e-7, "{a:?} -> {:?}: {d} > {len}", t.end);
        }
    }

    #[test]
    fn surface_distance_is_symmetric() {
        let mut rng = Rng(0xabcd_1234_5678_9f01);
        let mut checked = 0u32;
        for i in 0..40_000 {
            let a = rng.point();
            let b = if i % 2 == 0 { rng.near(a, 14.0) } else { rng.point() };
            match (surface_distance(a, b, 12.0), surface_distance(b, a, 12.0)) {
                (Some(x), Some(y)) => {
                    assert!((x - y).abs() < 1e-9, "{a:?} {b:?}: {x} vs {y}");
                    checked += 1;
                }
                (None, None) => {}
                (x, y) => panic!("asymmetric range for {a:?} {b:?}: {x:?} vs {y:?}"),
            }
        }
        assert!(checked > 2000, "only {checked} pairs were in range");
    }

    #[test]
    fn a_point_is_at_distance_zero_from_itself() {
        let mut rng = Rng(0x9999_1111_2222_3333);
        for _ in 0..2000 {
            let a = rng.point();
            let u = unfold(a, a, 12.0).expect("self");
            assert_eq!(u.path, ChartPath::direct());
            assert_eq!(u.distance, 0.0);
        }
    }

    #[test]
    fn invalid_segments_are_rejected() {
        let front = Face::Front;
        let o = Vec2::new(32.0, 32.0);
        // A straight move to the right stays inside Front.
        assert!(segment_is_valid(front, o, Vec2::new(40.0, 32.0), &ChartPath::direct()));
        // ...but claiming it crossed the right seam is wrong.
        let crossed = ChartPath { len: 1, steps: [(front, Edge::Right), PATH_FILLER] };
        assert!(!segment_is_valid(front, o, Vec2::new(40.0, 32.0), &crossed));
        // A move past the right edge really does cross it.
        assert!(segment_is_valid(front, o, Vec2::new(70.0, 32.0), &crossed));
        // ...and is not a direct segment.
        assert!(!segment_is_valid(front, o, Vec2::new(70.0, 32.0), &ChartPath::direct()));
        // The open rim is never a valid step.
        let rim = ChartPath { len: 1, steps: [(front, Edge::Bottom), PATH_FILLER] };
        assert!(!segment_is_valid(front, o, Vec2::new(32.0, 70.0), &rim));
        // A path whose first step names the wrong chart is rejected.
        let wrong = ChartPath { len: 1, steps: [(Face::Back, Edge::Right), PATH_FILLER] };
        assert!(!segment_is_valid(front, o, Vec2::new(70.0, 32.0), &wrong));
    }

    #[test]
    #[should_panic(expected = "MAX_LOCAL_RADIUS")]
    fn radius_above_the_local_bound_panics() {
        let a = SurfacePoint::new(Face::Front, 1.0, 1.0);
        let _ = unfold(a, a, MAX_LOCAL_RADIUS + 1.0);
    }
}
