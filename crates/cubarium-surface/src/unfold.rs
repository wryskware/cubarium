//! Local straight-line unfoldings: surface distance and chart images for nearby points.

use crate::travel::{earliest_exit, edge_len, edge_point};
use crate::{Edge, Face, GEOM_EPS, Seam, SurfacePoint, TangentMap, Topology, Vec2};

/// Largest supported query radius in pixels **on the cube**. Within this radius every
/// shortest path crosses at most [`MAX_SEAMS`] seams (both points are well inside one
/// face width of each other, and the open bottom offers no shortcut), so enumerating
/// chart paths of that length is complete. Callers asking for more get a panic, not a
/// wrong answer.
///
/// This is a completeness *proof*, not a tunable: nothing about the world scale may
/// touch it. A ring has its own, chosen, cap — see [`Topology::max_local_radius`], which
/// is what the functions here actually assert against.
pub const MAX_LOCAL_RADIUS: f64 = 32.0;

/// Seam crossings enumerated per chart path on the cube. A ring never needs two: its
/// three images (direct, `+w`, `−w`) are one crossing apart, and
/// [`Topology::validate`] guarantees at most two of them are ever in range.
pub const MAX_SEAMS: u8 = 2;

/// A sequence of at most [`MAX_SEAMS`] seam crossings starting in the observer's chart.
/// Each step names the chart being exited and the edge it exits through; the entry
/// chart/edge follow from [`Topology::neighbor`]. Ordering is lexicographic on
/// `(len, steps)`, which is the tie-break rule for equal-length unfoldings; `direct()`
/// sorts first.
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
    pub fn final_face(&self, topo: Topology, observer_face: Face) -> Face {
        let mut f = observer_face;
        for &(face, edge) in self.steps() {
            debug_assert_eq!(face, f);
            f = topo.neighbor(face, edge).expect("chart paths never cross an open rim").face;
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

/// All chart images reachable from `observer_face` by at most `max_seams` seam crossings,
/// including the identity image of the observer's own chart, in [`ChartPath`] order.
/// Paths never cross an open rim and never immediately return through the edge they
/// entered by. The same target face may appear under several paths (for example Top via
/// Front→Top and via Front→Right→Top); all are kept because each is the valid image for
/// a different region.
///
/// **A cube** enumerates up to [`MAX_SEAMS`] crossings: 11 images from a side face, 13
/// from Top. **A ring** enumerates exactly three — the direct image and the translations
/// by `+w` (exiting `Edge::Right`) and `−w` (exiting `Edge::Left`) — because its only
/// seam is the chart to itself and a second crossing could only produce `±2w`, which
/// [`Topology::validate`]'s width bound puts permanently out of range.
///
/// Derivation of a one-seam image (normative): for the observer chart exiting through
/// edge `e` into `(n, e2, reversed)`, the rotation `R` is the quarter-turn count from
/// [`Topology::seam_turns`]; the image rotation is `R.inverse()` (it carries target-chart
/// tangents back into the observer chart); `origin` is fixed by requiring that the entry
/// point at along-edge parameter `s'` on `e2` maps onto the exit point at parameter `s`
/// on `e` (with `s' = edge_len - s` when reversed) for `s = 0`, and debug-checked for
/// `s = edge_len`. Two-seam images compose the second chart's image within the first.
pub fn chart_images(topo: Topology, observer_face: Face, max_seams: u8, out: &mut Vec<ChartImage>) {
    // Longer paths are not enumerated; asking for more is a caller bug, not a panic.
    debug_assert!(max_seams <= MAX_SEAMS, "max_seams {max_seams} exceeds MAX_SEAMS");
    let max_seams = match topo {
        Topology::Cube => max_seams.min(MAX_SEAMS),
        // One crossing is the whole wrap; a second would only name ±2w.
        Topology::Ring { .. } => max_seams.min(1),
    };
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
        let Some((seam, origin1, map1)) = seam_image(topo, observer_face, e) else {
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
            let Some((seam2, origin2, map2)) = seam_image(topo, seam.face, e3) else {
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
/// On a ring this comes out as the pure translation by `±w` with `map = IDENTITY`.
fn seam_image(topo: Topology, face: Face, edge: Edge) -> Option<(Seam, Vec2, TangentMap)> {
    let seam = topo.neighbor(face, edge)?;
    let turns = topo.seam_turns(face, edge).expect("the seam exists");
    let map = TangentMap::quarter_turns(turns).inverse();

    let here = topo.extent(face);
    let there = topo.extent(seam.face);
    let len = edge_len(edge, here);
    let entry_param = |s: f64| if seam.reversed { len - s } else { s };
    let origin = edge_point(edge, 0.0, here) - map.apply(edge_point(seam.edge, entry_param(0.0), there));

    debug_assert!(
        {
            // The far end of the shared edge must land on the far end of the exit edge,
            // which also pins both shared corners of the two charts.
            let far = origin + map.apply(edge_point(seam.edge, entry_param(len), there));
            (far - edge_point(edge, len, here)).length() < 1e-9
        },
        "seam image {face:?}.{edge:?} -> {seam:?} does not match at s = {len}"
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
/// among the tied edges) and, for the final chart, must end inside `[0, w] × [0, h]`
/// (boundary inclusive within `GEOM_EPS`). The empty path is valid iff the segment stays
/// inside the observer's chart. Reflection is never valid for an unfolding: a segment
/// that would leave through an open rim is invalid.
pub fn segment_is_valid(topo: Topology, observer_face: Face, observer: Vec2, image: Vec2, path: &ChartPath) -> bool {
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

        let Some(exit) = earliest_exit(here, dir, sweep_len, topo.extent(face)) else {
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
        let Some((seam, origin_next, map_next)) = seam_image(topo, face, step_edge) else {
            // An open rim is never a valid unfolding step.
            return false;
        };
        origin += map.apply(origin_next);
        map = map_next.then(map);
        face = seam.face;
        t_enter = exit.t;
    }

    // The endpoint must lie inside the final chart.
    let (w, h) = topo.extent(face);
    let end = map.inverse().apply(image - origin);
    (-GEOM_EPS..=w + GEOM_EPS).contains(&end.x) && (-GEOM_EPS..=h + GEOM_EPS).contains(&end.y)
}

/// Shortest valid unfolding of `target` as seen from `observer`, or `None` when no valid
/// unfolding has length ≤ `max_distance`. Panics if `max_distance` exceeds
/// [`Topology::max_local_radius`]. Candidates are pre-rejected when
/// [`Topology::chord_sq`] exceeds `max_distance` (with a `GEOM_EPS` margin).
/// Equal-length candidates (within `GEOM_EPS`) resolve by [`ChartPath`] order.
/// Both points must be canonical.
pub fn unfold(topo: Topology, observer: SurfacePoint, target: SurfacePoint, max_distance: f64) -> Option<Unfolded> {
    let mut images = Vec::new();
    chart_images(topo, observer.face, MAX_SEAMS, &mut images);
    unfold_with(topo, &images, observer, target, max_distance)
}

/// [`unfold`] using precomputed `chart_images(topo, observer.face, MAX_SEAMS, ..)`; the
/// caller caches those per face to keep the hot path allocation-free.
pub fn unfold_with(
    topo: Topology,
    images: &[ChartImage],
    observer: SurfacePoint,
    target: SurfacePoint,
    max_distance: f64,
) -> Option<Unfolded> {
    let local_radius = topo.max_local_radius();
    assert!(
        max_distance <= local_radius,
        "max_distance {max_distance} exceeds the local radius {local_radius} of {topo:?} \
         (MAX_LOCAL_RADIUS on the cube)"
    );
    debug_assert!(observer.is_canonical(topo), "unfold from non-canonical {observer:?}");
    debug_assert!(target.is_canonical(topo), "unfold to non-canonical {target:?}");

    let limit = max_distance + GEOM_EPS;
    // A lower bound on surface distance (exact on a ring): reject far pairs before
    // unfolding them.
    if topo.chord_sq(&observer, &target) > limit * limit {
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
        if !segment_is_valid(topo, observer.face, here, local, &img.path) {
            continue;
        }
        best = Some(Unfolded { local, map: img.map, distance, path: img.path });
    }
    best
}

/// Surface distance between two canonical points if it is ≤ `max_distance`.
/// Symmetric: `surface_distance(t, a, b, r) == surface_distance(t, b, a, r)` within
/// `GEOM_EPS`.
pub fn surface_distance(topo: Topology, a: SurfacePoint, b: SurfacePoint, max_distance: f64) -> Option<f64> {
    unfold(topo, a, b, max_distance).map(|u| u.distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{travel, PathSegment};

    fn images(face: Face) -> Vec<ChartImage> {
        let mut v = Vec::new();
        chart_images(Topology::Cube, face, MAX_SEAMS, &mut v);
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
            SurfacePoint::new(f, self.next_f64() * 64.0, self.next_f64() * 64.0).canonicalize(Topology::Cube)
        }
        /// A point within roughly `reach` pixels of `a`, so that pairs land in range
        /// often and seams and vertices get hit.
        fn near(&mut self, a: SurfacePoint, reach: f64) -> SurfacePoint {
            let angle = self.next_f64() * std::f64::consts::TAU;
            let len = self.next_f64() * reach;
            travel(Topology::Cube, a, Vec2::from_screen_angle(angle) * len).end
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
                assert_eq!(img.path.final_face(Topology::Cube, face), img.target_face);
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
        let u = unfold(Topology::Cube, a, b, 12.0).expect("in range");
        assert_eq!(u.path, ChartPath::direct());
        assert!((u.distance - 5.0).abs() < 1e-12);
        assert_eq!(u.map, TangentMap::IDENTITY);
        assert_eq!(u.local, Vec2::new(13.0, 14.0));
        assert!(unfold(Topology::Cube, a, b, 4.0).is_none(), "out of range");
    }

    #[test]
    fn one_pixel_across_a_vertical_seam() {
        let a = SurfacePoint::pixel_center(Topology::Cube, Face::Front, 63, 20);
        let b = SurfacePoint::pixel_center(Topology::Cube, Face::Right, 0, 20);
        let u = unfold(Topology::Cube, a, b, 12.0).expect("adjacent pixels");
        assert!((u.distance - 1.0).abs() < 1e-12, "{}", u.distance);
        assert_eq!(u.path.steps(), &[(Face::Front, Edge::Right)]);
    }

    #[test]
    fn one_pixel_across_a_twisted_top_seam() {
        let a = SurfacePoint::pixel_center(Topology::Cube, Face::Right, 10, 0);
        let (f, x, y, _) = crate::cross_seam(Face::Right, Edge::Top, 10).expect("seam");
        assert_eq!(f, Face::Top);
        let b = SurfacePoint::pixel_center(Topology::Cube, f, u16::from(x), u16::from(y));
        let u = unfold(Topology::Cube, a, b, 12.0).expect("adjacent pixels");
        assert!((u.distance - 1.0).abs() < 1e-12, "{}", u.distance);
        assert_eq!(u.path.steps(), &[(Face::Right, Edge::Top)]);
        // The heading transport matches what travelling across the seam does.
        let t = travel(Topology::Cube, a, u.local - a.chart());
        assert_eq!(t.map.inverse(), u.map);
    }

    #[test]
    fn unfolding_agrees_with_travel() {
        let mut rng = Rng(0x0f1e_2d3c_4b5a_6978);
        let mut checked = 0u32;
        for i in 0..40_000 {
            let a = rng.point();
            let b = if i % 2 == 0 { rng.near(a, 14.0) } else { rng.point() };
            let Some(u) = unfold(Topology::Cube, a, b, 12.0) else { continue };
            // Walking the unfolded straight segment must actually arrive at the target.
            let t = travel(Topology::Cube, a, u.local - a.chart());
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
            assert!(u.distance * u.distance >= Topology::Cube.chord_sq(&a, &b) - 1e-6);
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
            let t = travel(Topology::Cube, a, Vec2::from_screen_angle(angle) * len);
            if t.fallback || t.ties > 0 {
                continue;
            }
            let d = surface_distance(Topology::Cube, a, t.end, 12.0);
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
            match (surface_distance(Topology::Cube, a, b, 12.0), surface_distance(Topology::Cube, b, a, 12.0)) {
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
            let u = unfold(Topology::Cube, a, a, 12.0).expect("self");
            assert_eq!(u.path, ChartPath::direct());
            assert_eq!(u.distance, 0.0);
        }
    }

    #[test]
    fn invalid_segments_are_rejected() {
        let front = Face::Front;
        let o = Vec2::new(32.0, 32.0);
        // A straight move to the right stays inside Front.
        assert!(segment_is_valid(Topology::Cube, front, o, Vec2::new(40.0, 32.0), &ChartPath::direct()));
        // ...but claiming it crossed the right seam is wrong.
        let crossed = ChartPath { len: 1, steps: [(front, Edge::Right), PATH_FILLER] };
        assert!(!segment_is_valid(Topology::Cube, front, o, Vec2::new(40.0, 32.0), &crossed));
        // A move past the right edge really does cross it.
        assert!(segment_is_valid(Topology::Cube, front, o, Vec2::new(70.0, 32.0), &crossed));
        // ...and is not a direct segment.
        assert!(!segment_is_valid(Topology::Cube, front, o, Vec2::new(70.0, 32.0), &ChartPath::direct()));
        // The open rim is never a valid step.
        let rim = ChartPath { len: 1, steps: [(front, Edge::Bottom), PATH_FILLER] };
        assert!(!segment_is_valid(Topology::Cube, front, o, Vec2::new(32.0, 70.0), &rim));
        // A path whose first step names the wrong chart is rejected.
        let wrong = ChartPath { len: 1, steps: [(Face::Back, Edge::Right), PATH_FILLER] };
        assert!(!segment_is_valid(Topology::Cube, front, o, Vec2::new(70.0, 32.0), &wrong));
    }

    #[test]
    #[should_panic(expected = "MAX_LOCAL_RADIUS")]
    fn radius_above_the_local_bound_panics() {
        let a = SurfacePoint::new(Face::Front, 1.0, 1.0);
        let _ = unfold(Topology::Cube, a, a, MAX_LOCAL_RADIUS + 1.0);
    }
}

#[cfg(test)]
mod ring_tests {
    //! Unfolding a ring: three images of the one chart, of which at most two are ever in
    //! range, and a distance that is exact rather than a bound.

    use super::*;
    use crate::{Scale, travel};

    const RING: Topology = Topology::Ring { w: 320, h: 180 };
    const RING2: Topology = Topology::Ring { w: 640, h: 360 };

    fn at(u: f64, v: f64) -> SurfacePoint {
        SurfacePoint::new(Face::Front, u, v)
    }

    fn images(topo: Topology) -> Vec<ChartImage> {
        let mut v = Vec::new();
        chart_images(topo, Face::Front, MAX_SEAMS, &mut v);
        v
    }

    #[test]
    fn a_ring_has_exactly_three_images_the_direct_one_and_plus_or_minus_w() {
        for (topo, w) in [(RING, 320.0), (RING2, 640.0)] {
            let v = images(topo);
            assert_eq!(v.len(), 3, "{topo:?}");
            // In ChartPath order: direct, then Right (edge 1), then Left (edge 3).
            assert_eq!(v[0].path, ChartPath::direct());
            assert_eq!(v[0].origin, Vec2::ZERO);
            assert_eq!(v[1].path.steps(), &[(Face::Front, Edge::Right)]);
            assert_eq!(v[1].origin, Vec2::new(w, 0.0));
            assert_eq!(v[2].path.steps(), &[(Face::Front, Edge::Left)]);
            assert_eq!(v[2].origin, Vec2::new(-w, 0.0));
            for img in &v {
                // The wrap is a pure translation: no rotation, no reflection, one chart.
                assert_eq!(img.target_face, Face::Front);
                assert_eq!(img.map, TangentMap::IDENTITY);
                assert_eq!(img.map.det(), 1);
                let p = Vec2::new(11.25, 47.5);
                assert!((img.preimage_point(img.image_point(p)) - p).length() < 1e-12);
            }
            // The two shifts are 2w apart, so at most two can be within the local radius
            // of any observer — which is what `Topology::validate` buys.
            assert!(2.0 * w > 2.0 * topo.max_local_radius(), "{topo:?}");
        }
    }

    #[test]
    fn a_pixel_step_across_the_wrap_is_one_pixel_of_distance() {
        let a = SurfacePoint::pixel_center(RING, Face::Front, 319, 20);
        let b = SurfacePoint::pixel_center(RING, Face::Front, 0, 20);
        let u = unfold(RING, a, b, 12.0).expect("adjacent pixels");
        assert!((u.distance - 1.0).abs() < 1e-12, "{}", u.distance);
        assert_eq!(u.path.steps(), &[(Face::Front, Edge::Right)]);
        assert_eq!(u.local, Vec2::new(320.5, 20.5));
        assert_eq!(u.map, TangentMap::IDENTITY);
        // And the other way round, through the Left image.
        let back = unfold(RING, b, a, 12.0).expect("adjacent pixels");
        assert!((back.distance - 1.0).abs() < 1e-12);
        assert_eq!(back.path.steps(), &[(Face::Front, Edge::Left)]);
    }

    #[test]
    fn the_short_way_round_wins() {
        // Three pixels apart the short way, 317 the long way.
        let a = at(1.5, 20.5);
        let b = at(318.5, 20.5);
        let u = unfold(RING, a, b, 12.0).expect("in range");
        assert!((u.distance - 3.0).abs() < 1e-12, "{}", u.distance);
        assert_eq!(u.path.steps(), &[(Face::Front, Edge::Left)]);
        // Inside the chart the direct image wins even close to the seam.
        let c = at(4.5, 20.5);
        let u = unfold(RING, a, c, 12.0).expect("in range");
        assert_eq!(u.path, ChartPath::direct());
        assert!((u.distance - 3.0).abs() < 1e-12);
    }

    #[test]
    fn chord_sq_is_the_exact_distance_so_nothing_in_range_is_rejected() {
        let mut seed = 0x77c4_2e8f_1d3b_5a90u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let imgs = images(RING);
        let mut in_range = 0;
        for i in 0..40_000 {
            let a = at(rnd() * 320.0, rnd() * 180.0).canonicalize(RING);
            // Half the pairs are placed near `a` — often across the wrap — so the exact
            // distance is exercised where it matters and not just rejected.
            let b = if i % 2 == 0 {
                travel(RING, a, Vec2::from_screen_angle(rnd() * std::f64::consts::TAU) * (rnd() * 14.0)).end
            } else {
                at(rnd() * 320.0, rnd() * 180.0).canonicalize(RING)
            };
            let chord = RING.chord_sq(&a, &b).sqrt();
            match unfold_with(RING, &imgs, a, b, 12.0) {
                Some(u) => {
                    // Exact, not a bound: the unfolded distance *is* the chord.
                    assert!((u.distance - chord).abs() < 1e-9, "{a:?} {b:?}: {} vs {chord}", u.distance);
                    in_range += 1;
                }
                None => assert!(chord > 12.0 - 1e-9, "{a:?} {b:?} at {chord} was rejected"),
            }
        }
        assert!(in_range > 10_000, "only {in_range} pairs were in range");
    }

    #[test]
    fn unfolding_agrees_with_travel() {
        let mut seed = 0x1a2b_3c4d_5e6f_7081u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut checked = 0u32;
        for topo in [RING, RING2] {
            let (w, h) = topo.extent(Face::Front);
            for _ in 0..20_000 {
                let a = at(rnd() * w, rnd() * h).canonicalize(topo);
                let angle = rnd() * std::f64::consts::TAU;
                let b = travel(topo, a, Vec2::from_screen_angle(angle) * (rnd() * 14.0)).end;
                let Some(u) = unfold(topo, a, b, 12.0) else { continue };
                let t = travel(topo, a, u.local - a.chart());
                if t.ties > 0 {
                    continue;
                }
                assert!(!t.fallback);
                assert_eq!(t.reflections, 0, "an unfolding never reflects: {a:?} {b:?}");
                assert_eq!(t.crossings, u32::from(u.path.len), "{a:?} {b:?} {:?}", u.path);
                assert!(
                    (t.end.u - b.u).abs() < 1e-7 && (t.end.v - b.v).abs() < 1e-7,
                    "{a:?} -> {b:?} landed on {:?}",
                    t.end
                );
                assert_eq!(t.map.inverse(), u.map);
                let swept: f64 = t.segments.iter().map(crate::PathSegment::length).sum();
                assert!((swept - u.distance).abs() < 1e-7);
                checked += 1;
            }
        }
        assert!(checked > 4000, "only {checked} pairs were in range");
    }

    #[test]
    fn surface_distance_is_symmetric_and_zero_on_the_diagonal() {
        let mut seed = 0x2468_1357_9bdf_ace0u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..20_000 {
            let a = at(rnd() * 320.0, rnd() * 180.0).canonicalize(RING);
            let b = at(rnd() * 320.0, rnd() * 180.0).canonicalize(RING);
            match (surface_distance(RING, a, b, 12.0), surface_distance(RING, b, a, 12.0)) {
                (Some(x), Some(y)) => assert!((x - y).abs() < 1e-9, "{a:?} {b:?}: {x} vs {y}"),
                (None, None) => {}
                (x, y) => panic!("asymmetric range for {a:?} {b:?}: {x:?} vs {y:?}"),
            }
            let u = unfold(RING, a, a, 12.0).expect("self");
            assert_eq!(u.path, ChartPath::direct());
            assert_eq!(u.distance, 0.0);
        }
    }

    #[test]
    fn a_rim_is_never_a_valid_unfolding_step() {
        let o = Vec2::new(160.0, 5.0);
        // Straight up and out through the top rim: no path, direct or otherwise, is valid.
        let rim = ChartPath { len: 1, steps: [(Face::Front, Edge::Top), PATH_FILLER] };
        assert!(!segment_is_valid(RING, Face::Front, o, Vec2::new(160.0, -5.0), &rim));
        assert!(!segment_is_valid(RING, Face::Front, o, Vec2::new(160.0, -5.0), &ChartPath::direct()));
        // The wrap is valid, and claiming the wrong one of the two is not.
        let right = ChartPath { len: 1, steps: [(Face::Front, Edge::Right), PATH_FILLER] };
        let left = ChartPath { len: 1, steps: [(Face::Front, Edge::Left), PATH_FILLER] };
        assert!(segment_is_valid(RING, Face::Front, Vec2::new(318.0, 20.0), Vec2::new(322.0, 20.0), &right));
        assert!(!segment_is_valid(RING, Face::Front, Vec2::new(318.0, 20.0), Vec2::new(322.0, 20.0), &left));
        assert!(segment_is_valid(RING, Face::Front, Vec2::new(2.0, 20.0), Vec2::new(-2.0, 20.0), &left));
    }

    #[test]
    fn the_local_radius_is_the_rings_own_and_a_larger_one_panics() {
        assert_eq!(RING.max_local_radius(), 90.0);
        assert_eq!(RING2.max_local_radius(), 180.0);
        // Well past the cube's 32, which is a cube proof and not a ring limit.
        let a = at(160.0, 90.0);
        let b = at(160.0, 130.0);
        assert!((surface_distance(RING, a, b, 50.0).expect("in range") - 40.0).abs() < 1e-12);
        // The stamp budget fits inside it at both scales.
        assert!(Scale::ONE.footprint_radius() <= RING.max_local_radius());
        assert!(Scale::new(2.0).footprint_radius() <= RING2.max_local_radius());
    }

    #[test]
    #[should_panic(expected = "MAX_LOCAL_RADIUS")]
    fn a_radius_above_the_rings_bound_panics() {
        let a = at(1.0, 1.0);
        let _ = unfold(RING, a, a, RING.max_local_radius() + 1.0);
    }
}
