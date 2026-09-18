//! Pixel ownership for seam-aware rasterization.

use crate::{
    ChartImage, ChartPath, Face, GEOM_EPS, MAX_SEAMS, SurfacePoint, Topology, Vec2, chart_images,
    segment_is_valid,
};

/// One destination pixel and its owning unfolding relative to an anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelImage {
    pub face: Face,
    pub x: u16,
    pub y: u16,
    /// The pixel center in the anchor's chart coordinates under the owning unfolding.
    /// Renderers evaluate body masks at `local - anchor.chart()` rotated into the body frame.
    pub local: Vec2,
    /// Surface distance from the anchor to the pixel center.
    pub distance: f64,
    pub path: ChartPath,
}

/// Every surface pixel whose center lies within surface distance `radius` of `anchor`,
/// each exactly once, with its shortest valid unfolding (ties by `ChartPath` order).
/// Output is sorted by `(face index, y, x)`; `out` is cleared first and its capacity
/// reused. Panics if `radius` exceeds [`Topology::max_local_radius`] or the anchor is not
/// canonical.
///
/// Normative: the result equals the brute-force set `{ p : unfold(topo, anchor, p, radius) }`
/// over every pixel centre of every chart. Implementations may take the single-chart fast
/// path when the anchor is at least `radius` from every chart edge (then every pixel is a
/// direct image), and otherwise iterate the pixels of each chart image's preimage
/// bounding box with chord pre-rejection; the fast path and the general path must agree
/// exactly. A pixel visible through two images (a creature near a cube top vertex, or a
/// footprint reaching around a corner, or a body straddling a ring's wrap) keeps only its
/// shortest one, so a stamp gets one contribution per pixel and a shape may show a
/// localized discontinuity at a vertex rather than doubled brightness.
pub fn unfold_pixels(topo: Topology, anchor: SurfacePoint, radius: f64, out: &mut Vec<PixelImage>) {
    let local_radius = topo.max_local_radius();
    assert!(
        radius <= local_radius,
        "radius {radius} exceeds the local radius {local_radius} of {topo:?} \
         (MAX_LOCAL_RADIUS on the cube)"
    );
    assert!(
        anchor.is_canonical(topo),
        "unfold_pixels from non-canonical {anchor:?}"
    );
    out.clear();
    // A negative radius selects nothing; NaN already failed the assert above.
    if radius < 0.0 {
        return;
    }
    let a = anchor.chart();
    let (w, h) = topo.extent(anchor.face);
    // Well inside the chart: every pixel within reach is a direct image.
    let margin = a.x.min(w - a.x).min(a.y).min(h - a.y);
    if margin >= radius {
        unfold_pixels_direct(topo, anchor, radius, out);
    } else {
        // FW-P's W2b: the image list is at most eleven entries and is rebuilt on every
        // seam-crossing stamp — about 560 of them per cube frame — so it comes from a
        // per-thread buffer rather than a fresh allocation. Nothing escapes it.
        IMAGES.with_borrow_mut(|images| {
            chart_images(topo, anchor.face, MAX_SEAMS, images);
            unfold_pixels_general(topo, images, anchor, radius, out);
        });
    }
}

thread_local! {
    /// Scratch for the chart images of one `unfold_pixels` call (see W2b above).
    static IMAGES: std::cell::RefCell<Vec<ChartImage>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Pixel indices whose centers can lie within `radius` of chart coordinate `c`, clipped
/// to `0..extent`. The range is a superset; callers still test the real distance.
#[inline]
fn index_range(c: f64, radius: f64, extent: f64) -> std::ops::RangeInclusive<u16> {
    let last = extent - 1.0;
    let lo = (c - radius - 0.5).ceil().clamp(0.0, last) as u16;
    let hi = (c + radius - 0.5).floor().clamp(0.0, last) as u16;
    lo..=hi
}

/// The fast path: the anchor is at least `radius` from every chart edge, so the whole
/// disk lies in the anchor's own chart and every owning path is `ChartPath::direct()`.
fn unfold_pixels_direct(
    topo: Topology,
    anchor: SurfacePoint,
    radius: f64,
    out: &mut Vec<PixelImage>,
) {
    let a = anchor.chart();
    let (w, h) = topo.extent(anchor.face);
    let limit = radius + GEOM_EPS;
    for y in index_range(a.y, limit, h) {
        for x in index_range(a.x, limit, w) {
            let local = Vec2::new(f64::from(x) + 0.5, f64::from(y) + 0.5);
            let distance = (local - a).length();
            if distance > limit {
                continue;
            }
            out.push(PixelImage {
                face: anchor.face,
                x,
                y,
                local,
                distance,
                path: ChartPath::direct(),
            });
        }
    }
    // Already in (face, y, x) order: one face, y outer, x inner.
}

/// The general path: walk every chart image of the anchor's face, keep the pixels whose
/// straight unfolded segment really does follow that image's chart path, then give each
/// pixel to its shortest such image.
fn unfold_pixels_general(
    topo: Topology,
    images: &[ChartImage],
    anchor: SurfacePoint,
    radius: f64,
    out: &mut Vec<PixelImage>,
) {
    let a = anchor.chart();
    let limit = radius + GEOM_EPS;
    for img in images {
        // The anchor's position in the target chart's own coordinates: the disk of
        // candidates is centred there, because the image map is an isometry.
        let ap = img.preimage_point(a);
        let (w, h) = topo.extent(img.target_face);
        for y in index_range(ap.y, limit, h) {
            for x in index_range(ap.x, limit, w) {
                let p = Vec2::new(f64::from(x) + 0.5, f64::from(y) + 0.5);
                let distance = (p - ap).length();
                if distance > limit {
                    continue;
                }
                let local = img.image_point(p);
                if !segment_is_valid(topo, anchor.face, a, local, &img.path) {
                    continue;
                }
                out.push(PixelImage {
                    face: img.target_face,
                    x,
                    y,
                    local,
                    distance,
                    path: img.path,
                });
            }
        }
    }
    // Sort into output order, with ChartPath order inside each pixel's group so that the
    // winner is picked exactly as `unfold_with` picks it.
    out.sort_by(|p, q| (p.face.index(), p.y, p.x, p.path).cmp(&(q.face.index(), q.y, q.x, q.path)));
    // Keep one image per pixel: the shortest, ties by ChartPath order.
    let mut write = 0usize;
    let mut i = 0usize;
    while i < out.len() {
        let key = (out[i].face, out[i].y, out[i].x);
        let mut j = i + 1;
        while j < out.len() && (out[j].face, out[j].y, out[j].x) == key {
            j += 1;
        }
        let mut best = i;
        for k in (i + 1)..j {
            if out[k].distance < out[best].distance - GEOM_EPS {
                best = k;
            }
        }
        out[write] = out[best];
        write += 1;
        i = j;
    }
    out.truncate(write);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, FACE_EXTENT, MAX_LOCAL_RADIUS, unfold};

    fn general(anchor: SurfacePoint, radius: f64) -> Vec<PixelImage> {
        let mut images = Vec::new();
        chart_images(Topology::Cube, anchor.face, MAX_SEAMS, &mut images);
        let mut v = Vec::new();
        unfold_pixels_general(Topology::Cube, &images, anchor, radius, &mut v);
        v
    }

    fn direct(anchor: SurfacePoint, radius: f64) -> Vec<PixelImage> {
        let mut v = Vec::new();
        unfold_pixels_direct(Topology::Cube, anchor, radius, &mut v);
        v
    }

    fn same(a: &[PixelImage], b: &[PixelImage], what: &str) {
        assert_eq!(
            a.len(),
            b.len(),
            "{what}: {} vs {} pixels",
            a.len(),
            b.len()
        );
        for (p, q) in a.iter().zip(b) {
            assert_eq!((p.face, p.x, p.y), (q.face, q.x, q.y), "{what}");
            assert_eq!(p.path, q.path, "{what} at {:?} ({}, {})", p.face, p.x, p.y);
            assert!((p.distance - q.distance).abs() < 1e-9, "{what}");
            assert!((p.local - q.local).length() < 1e-9, "{what}");
        }
    }

    /// The two implementations must agree exactly where both are legal: an anchor
    /// exactly `radius` from the nearest chart edge is the boundary of the fast path.
    #[test]
    fn fast_and_general_paths_agree_at_the_fast_path_boundary() {
        for radius in [0.5, 1.0, 3.0, 4.5, 9.0, 12.0, 20.0] {
            for face in Face::ALL {
                // Exactly on the boundary of the fast-path condition, on each side.
                let anchors = [
                    SurfacePoint::new(face, radius, radius),
                    SurfacePoint::new(face, FACE_EXTENT - radius, radius),
                    SurfacePoint::new(face, radius, FACE_EXTENT - radius),
                    SurfacePoint::new(face, FACE_EXTENT - radius, FACE_EXTENT - radius),
                    SurfacePoint::new(face, radius, 32.0),
                    SurfacePoint::new(face, 32.0, radius),
                    SurfacePoint::new(face, radius + 1e-9, 32.5),
                    SurfacePoint::new(face, 32.5, FACE_EXTENT - radius - 1e-9),
                ];
                for anchor in anchors {
                    let anchor = anchor.canonicalize(Topology::Cube);
                    let what = format!("{face:?} {anchor:?} r={radius}");
                    same(&direct(anchor, radius), &general(anchor, radius), &what);
                    // And the public entry point picks one of them, consistently.
                    let mut v = Vec::new();
                    unfold_pixels(Topology::Cube, anchor, radius, &mut v);
                    same(&v, &general(anchor, radius), &what);
                }
            }
        }
    }

    /// The normative definition: the same set brute force gets from `unfold`.
    #[test]
    fn matches_the_brute_force_definition() {
        let anchors = [
            SurfacePoint::new(Face::Front, 32.5, 32.5),
            SurfacePoint::new(Face::Front, 0.25, 0.25),
            SurfacePoint::new(Face::Front, 63.75, 0.25),
            SurfacePoint::new(Face::Front, 63.75, 63.75),
            SurfacePoint::new(Face::Top, 0.5, 0.5),
            SurfacePoint::new(Face::Top, 63.5, 63.5),
            SurfacePoint::new(Face::Top, 32.0, 0.0),
            SurfacePoint::new(Face::Right, 0.5, 0.5),
            SurfacePoint::new(Face::Back, 63.5, 2.5),
            SurfacePoint::new(Face::Left, 3.5, 61.5),
        ];
        for anchor in anchors {
            let anchor = anchor.canonicalize(Topology::Cube);
            for radius in [1.5, 6.0, 9.0] {
                let mut got = Vec::new();
                unfold_pixels(Topology::Cube, anchor, radius, &mut got);
                let mut want = Vec::new();
                for face in Face::ALL {
                    for y in 0..64u16 {
                        for x in 0..64u16 {
                            let p = SurfacePoint::pixel_center(Topology::Cube, face, x, y);
                            if let Some(u) = unfold(Topology::Cube, anchor, p, radius) {
                                want.push(PixelImage {
                                    face,
                                    x,
                                    y,
                                    local: u.local,
                                    distance: u.distance,
                                    path: u.path,
                                });
                            }
                        }
                    }
                }
                same(&got, &want, &format!("{anchor:?} r={radius}"));
            }
        }
    }

    #[test]
    fn every_pixel_appears_at_most_once_and_is_sorted() {
        let mut seed = 0x2468_ace0_1357_9bdfu64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut out = Vec::new();
        for _ in 0..400 {
            let face = Face::ALL[(rnd() * 5.0) as usize % 5];
            let anchor =
                SurfacePoint::new(face, rnd() * 64.0, rnd() * 64.0).canonicalize(Topology::Cube);
            let radius = 0.5 + rnd() * 11.5;
            unfold_pixels(Topology::Cube, anchor, radius, &mut out);
            let mut keys: Vec<(usize, u16, u16)> =
                out.iter().map(|p| (p.face.index(), p.y, p.x)).collect();
            let sorted = keys.clone();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(
                keys.len(),
                out.len(),
                "duplicate pixel for {anchor:?} r={radius}"
            );
            assert_eq!(keys, sorted, "output not sorted for {anchor:?} r={radius}");
            for p in &out {
                assert!(p.distance <= radius + GEOM_EPS);
                // `local` is the pixel center as seen in the anchor's chart.
                assert!(((p.local - anchor.chart()).length() - p.distance).abs() < 1e-9);
                assert!(segment_is_valid(
                    Topology::Cube,
                    anchor.face,
                    anchor.chart(),
                    p.local,
                    &p.path
                ));
            }
        }
    }

    #[test]
    fn a_disk_keeps_its_area_across_a_seam() {
        // The same radius anywhere on the closed part of the surface covers the same
        // number of pixels, to within the discretisation of the disk.
        let mut counts = Vec::new();
        let mut out = Vec::new();
        for u in [10.0, 32.0, 63.5] {
            unfold_pixels(
                Topology::Cube,
                SurfacePoint::new(Face::Front, u, 32.0),
                6.0,
                &mut out,
            );
            counts.push(out.len());
        }
        // Away from the open rim and the top vertices the count is stable.
        assert_eq!(counts[0], counts[1], "{counts:?}");
        assert!(
            counts[2] >= counts[1] - 4 && counts[2] <= counts[1] + 4,
            "{counts:?}"
        );
    }

    #[test]
    fn the_anchor_pixel_is_always_present() {
        let mut out = Vec::new();
        for face in Face::ALL {
            for &(x, y) in &[(0u16, 0u16), (63, 0), (0, 63), (63, 63), (31, 31)] {
                let anchor = SurfacePoint::pixel_center(Topology::Cube, face, x, y);
                unfold_pixels(Topology::Cube, anchor, 3.0, &mut out);
                let me = out.iter().find(|p| (p.face, p.x, p.y) == (face, x, y));
                let me =
                    me.unwrap_or_else(|| panic!("anchor pixel missing for {face:?} ({x},{y})"));
                assert_eq!(me.distance, 0.0);
                assert_eq!(me.path, ChartPath::direct());
            }
        }
    }

    #[test]
    fn a_vertex_anchor_reaches_three_faces() {
        // The Front/Right/Top vertex: pixels from all three charts, each exactly once.
        let anchor = SurfacePoint::new(Face::Front, 63.5, 0.5);
        let mut out = Vec::new();
        unfold_pixels(Topology::Cube, anchor, 5.0, &mut out);
        for face in [Face::Front, Face::Right, Face::Top] {
            assert!(out.iter().any(|p| p.face == face), "no {face:?} pixels");
        }
        assert!(
            !out.iter()
                .any(|p| p.face == Face::Back || p.face == Face::Left)
        );
        // Standing exactly on the vertex, all three wedges are one seam away; the 90
        // degree angular deficit shows up as a disk smaller than the flat one.
        assert!(out.iter().all(|p| p.path.len <= 1));
        let flat = std::f64::consts::PI * 25.0;
        assert!(
            (out.len() as f64) < flat * 0.9 && (out.len() as f64) > flat * 0.6,
            "{} pixels vs a flat disk of {flat:.1}",
            out.len()
        );

        // Set back from the vertex, reach around it: some pixels are owned by a
        // two-seam route (here Front -> Top -> Right) rather than the single seam.
        let anchor = SurfacePoint::new(Face::Front, 60.0, 3.0);
        unfold_pixels(Topology::Cube, anchor, 9.0, &mut out);
        assert!(out.iter().any(|p| p.path.len == 2), "no two-seam pixel");

        // Every listed step is a real seam, never the open rim.
        for p in &out {
            for &(f, e) in p.path.steps() {
                assert!(f.neighbor(e).is_some());
                assert_ne!(e, Edge::Bottom, "unfoldings never cross the open rim");
            }
        }
    }

    #[test]
    fn zero_radius_keeps_only_the_containing_pixel_when_centred() {
        let mut out = Vec::new();
        unfold_pixels(
            Topology::Cube,
            SurfacePoint::pixel_center(Topology::Cube, Face::Front, 10, 10),
            0.0,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].face, out[0].x, out[0].y), (Face::Front, 10, 10));
    }

    #[test]
    #[should_panic(expected = "MAX_LOCAL_RADIUS")]
    fn radius_above_the_local_bound_panics() {
        let mut out = Vec::new();
        unfold_pixels(
            Topology::Cube,
            SurfacePoint::new(Face::Front, 1.0, 1.0),
            MAX_LOCAL_RADIUS + 1.0,
            &mut out,
        );
    }
}

#[cfg(test)]
mod ring_tests {
    //! Pixel ownership on a ring: the wrap gives a body two images of the chart, and the
    //! exactly-once guarantee has to survive that.

    use super::*;
    use crate::{Edge, Scale, unfold};

    const RING: Topology = Topology::Ring { w: 320, h: 180 };
    const RING2: Topology = Topology::Ring { w: 640, h: 360 };

    fn at(u: f64, v: f64) -> SurfacePoint {
        SurfacePoint::new(Face::Front, u, v)
    }

    fn general(topo: Topology, anchor: SurfacePoint, radius: f64) -> Vec<PixelImage> {
        let mut images = Vec::new();
        chart_images(topo, anchor.face, MAX_SEAMS, &mut images);
        let mut v = Vec::new();
        unfold_pixels_general(topo, &images, anchor, radius, &mut v);
        v
    }

    fn same(a: &[PixelImage], b: &[PixelImage], what: &str) {
        assert_eq!(
            a.len(),
            b.len(),
            "{what}: {} vs {} pixels",
            a.len(),
            b.len()
        );
        for (p, q) in a.iter().zip(b) {
            assert_eq!((p.face, p.x, p.y), (q.face, q.x, q.y), "{what}");
            assert_eq!(p.path, q.path, "{what} at ({}, {})", p.x, p.y);
            assert!((p.distance - q.distance).abs() < 1e-9, "{what}");
            assert!((p.local - q.local).length() < 1e-9, "{what}");
        }
    }

    /// The normative definition, brute-forced over all 57,600 pixel centres.
    #[test]
    fn matches_the_brute_force_definition_across_the_wrap() {
        let anchors = [
            at(160.5, 90.5),    // the middle: the fast path
            at(0.25, 90.5),     // hard against the wrap
            at(319.75, 90.5),   // and the other side of it
            at(0.25, 0.25),     // the top-left corner
            at(319.75, 179.75), // the bottom-right corner
            at(2.5, 177.5),     // near the bottom-left corner
        ];
        for anchor in anchors {
            let anchor = anchor.canonicalize(RING);
            for radius in [1.5, 6.0, 9.0] {
                let mut got = Vec::new();
                unfold_pixels(RING, anchor, radius, &mut got);
                let mut want = Vec::new();
                for y in 0..180u16 {
                    for x in 0..320u16 {
                        let p = SurfacePoint::pixel_center(RING, Face::Front, x, y);
                        if let Some(u) = unfold(RING, anchor, p, radius) {
                            want.push(PixelImage {
                                face: Face::Front,
                                x,
                                y,
                                local: u.local,
                                distance: u.distance,
                                path: u.path,
                            });
                        }
                    }
                }
                same(&got, &want, &format!("{anchor:?} r={radius}"));
            }
        }
    }

    #[test]
    fn the_fast_and_general_paths_agree_at_the_boundary() {
        for radius in [0.5, 1.0, 3.0, 9.0, 20.0] {
            let anchors = [
                at(radius, radius),
                at(320.0 - radius, radius),
                at(radius, 180.0 - radius),
                at(320.0 - radius, 180.0 - radius),
                at(radius, 90.0),
                at(160.0, radius),
                at(radius + 1e-9, 90.5),
                at(160.5, 180.0 - radius - 1e-9),
            ];
            for anchor in anchors {
                let anchor = anchor.canonicalize(RING);
                let what = format!("{anchor:?} r={radius}");
                let mut direct = Vec::new();
                unfold_pixels_direct(RING, anchor, radius, &mut direct);
                same(&direct, &general(RING, anchor, radius), &what);
                let mut v = Vec::new();
                unfold_pixels(RING, anchor, radius, &mut v);
                same(&v, &general(RING, anchor, radius), &what);
            }
        }
    }

    #[test]
    fn every_pixel_appears_exactly_once_and_in_order() {
        let mut seed = 0x3141_5926_5358_9793u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut out = Vec::new();
        for topo in [RING, RING2] {
            let (w, h) = topo.extent(Face::Front);
            for _ in 0..300 {
                let anchor = at(rnd() * w, rnd() * h).canonicalize(topo);
                let radius = 0.5 + rnd() * 11.5;
                unfold_pixels(topo, anchor, radius, &mut out);
                let keys: Vec<(u16, u16)> = out.iter().map(|p| (p.y, p.x)).collect();
                let mut sorted = keys.clone();
                sorted.sort_unstable();
                sorted.dedup();
                assert_eq!(
                    sorted.len(),
                    out.len(),
                    "duplicate pixel for {anchor:?} r={radius}"
                );
                assert_eq!(sorted, keys, "output not sorted for {anchor:?} r={radius}");
                for p in &out {
                    assert!(p.distance <= radius + GEOM_EPS);
                    assert!(((p.local - anchor.chart()).length() - p.distance).abs() < 1e-9);
                    assert!(segment_is_valid(
                        topo,
                        anchor.face,
                        anchor.chart(),
                        p.local,
                        &p.path
                    ));
                    // Unfoldings never cross a rim.
                    for &(_, e) in p.path.steps() {
                        assert!(e == Edge::Right || e == Edge::Left, "a rim in an unfolding");
                    }
                }
            }
        }
    }

    /// A body straddling `u = 0` is carried by the existing machinery: the disk keeps its
    /// area, and the pixels on the far side of the wrap arrive through the `±w` image.
    #[test]
    fn a_disk_keeps_its_area_across_the_wrap() {
        let mut out = Vec::new();
        let mut counts = Vec::new();
        // All four anchors are pixel centres, so only the wrap can change the count.
        for u in [40.5, 160.5, 0.5, 319.5] {
            unfold_pixels(RING, at(u, 90.5), 6.0, &mut out);
            counts.push(out.len());
        }
        assert_eq!(counts[0], counts[1], "{counts:?}");
        assert_eq!(
            counts[0], counts[2],
            "the wrap is not a boundary: {counts:?}"
        );
        assert_eq!(
            counts[0], counts[3],
            "the wrap is not a boundary: {counts:?}"
        );

        // Straddling the seam, both sides are present, each pixel once.
        unfold_pixels(RING, at(0.5, 90.5), 5.0, &mut out);
        assert!(
            out.iter().any(|p| p.x >= 316),
            "no pixels on the far side of the wrap"
        );
        assert!(out.iter().any(|p| p.x <= 4), "no pixels on the near side");
        assert!(out.iter().any(|p| p.path.len == 1), "no wrapped ownership");
        // A rim clips instead: fewer pixels, and none outside the chart.
        unfold_pixels(RING, at(160.5, 0.5), 5.0, &mut out);
        assert!(out.iter().all(|p| p.y < 180));
        assert!(out.len() < counts[0], "the rim did not clip the disk");
    }

    #[test]
    fn the_anchor_pixel_is_always_present_and_the_stamp_budget_fits() {
        let mut out = Vec::new();
        for &(x, y) in &[(0u16, 0u16), (319, 0), (0, 179), (319, 179), (160, 90)] {
            let anchor = SurfacePoint::pixel_center(RING, Face::Front, x, y);
            unfold_pixels(RING, anchor, Scale::ONE.footprint_radius(), &mut out);
            let me = out
                .iter()
                .find(|p| (p.x, p.y) == (x, y))
                .unwrap_or_else(|| panic!("anchor pixel missing for ({x},{y})"));
            assert_eq!(me.distance, 0.0);
            assert_eq!(me.path, ChartPath::direct());
        }
        // The same stamp at S = 2 covers four times the pixels, as a doubled radius must.
        let mut wide = Vec::new();
        unfold_pixels(
            RING,
            at(160.5, 90.5),
            Scale::ONE.footprint_radius(),
            &mut out,
        );
        unfold_pixels(
            RING2,
            at(320.5, 180.5),
            Scale::new(2.0).footprint_radius(),
            &mut wide,
        );
        let ratio = wide.len() as f64 / out.len() as f64;
        assert!(
            (ratio - 4.0).abs() < 0.2,
            "{} vs {} pixels",
            wide.len(),
            out.len()
        );
    }

    #[test]
    #[should_panic(expected = "MAX_LOCAL_RADIUS")]
    fn a_radius_above_the_rings_bound_panics() {
        let mut out = Vec::new();
        unfold_pixels(RING, at(1.0, 1.0), RING.max_local_radius() + 1.0, &mut out);
    }
}
