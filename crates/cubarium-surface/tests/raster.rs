//! Pixel ownership for seam-aware rasterization.
//!
//! `src/raster.rs` is normative: "the result equals the brute-force set
//! `{ p : unfold(anchor, p, radius) }` over all 20,480 pixel centers", each pixel at most
//! once, sorted by `(face index, y, x)`, with the shortest valid unfolding and
//! `ChartPath` order as the tie-break. Every test here compares `unfold_pixels` against
//! exactly that brute force, including the fast-path/general-path agreement the doc
//! demands ("the fast path and the general path must agree exactly") by using anchors
//! both far from and on top of every seam, vertex and rim corner.

use cubarium_surface::{
    Face, MAX_LOCAL_RADIUS, MAX_SEAMS, PixelImage, SurfacePoint, chart_images, unfold_pixels,
    unfold_with,
};

/// The brute force the doc comment defines: every pixel centre on the surface, kept when
/// `unfold` finds a valid unfolding within `radius`. Emitted in `(face, y, x)` order,
/// which is the order `unfold_pixels` promises.
fn brute_force(anchor: SurfacePoint, radius: f64) -> Vec<PixelImage> {
    let mut images = Vec::new();
    chart_images(anchor.face, MAX_SEAMS, &mut images);
    let mut out = Vec::new();
    let mut examined = 0usize;
    for face in Face::ALL {
        for y in 0..64u8 {
            for x in 0..64u8 {
                examined += 1;
                let p = SurfacePoint::pixel_center(face, x, y);
                if let Some(u) = unfold_with(&images, anchor, p, radius) {
                    out.push(PixelImage {
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
    assert_eq!(examined, 20_480, "the brute force must cover every surface pixel");
    out
}

fn corner(face: Face, u: f64, v: f64) -> SurfacePoint {
    SurfacePoint::new(face, u, v).canonicalize()
}

/// Every kind of anchor the doc calls out: interior (single-chart fast path), next to each
/// of the eight seams, on each of the four top vertices seen from each of its three
/// incident charts, and in each lower rim corner.
fn anchors() -> Vec<(String, SurfacePoint)> {
    let mut out: Vec<(String, SurfacePoint)> = Vec::new();
    out.push(("interior".into(), SurfacePoint::pixel_center(Face::Front, 32, 32)));
    out.push(("interior off-lattice".into(), SurfacePoint::new(Face::Front, 20.25, 41.75)));
    for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
        out.push((format!("{face:?} vertical seam"), SurfacePoint::new(face, 63.5, 32.5)));
        out.push((format!("{face:?} top seam"), SurfacePoint::new(face, 32.5, 0.5)));
    }
    let vertices: [[(Face, f64, f64); 3]; 4] = [
        [(Face::Front, 64.0, 0.0), (Face::Right, 0.0, 0.0), (Face::Top, 64.0, 64.0)],
        [(Face::Right, 64.0, 0.0), (Face::Back, 0.0, 0.0), (Face::Top, 64.0, 0.0)],
        [(Face::Back, 64.0, 0.0), (Face::Left, 0.0, 0.0), (Face::Top, 0.0, 0.0)],
        [(Face::Left, 64.0, 0.0), (Face::Front, 0.0, 0.0), (Face::Top, 0.0, 64.0)],
    ];
    for (i, vertex) in vertices.into_iter().enumerate() {
        for (face, u, v) in vertex {
            out.push((format!("vertex {i} from {face:?}"), corner(face, u, v)));
        }
    }
    for (face, u) in [(Face::Front, 63.5), (Face::Right, 0.5), (Face::Back, 63.5), (Face::Left, 0.5)] {
        out.push((format!("{face:?} lower corner"), SurfacePoint::new(face, u, 63.5)));
    }
    out
}

fn assert_matches_brute_force(what: &str, anchor: SurfacePoint, radius: f64) -> Vec<PixelImage> {
    let mut got = Vec::new();
    unfold_pixels(anchor, radius, &mut got);
    let want = brute_force(anchor, radius);
    assert_eq!(
        got.len(),
        want.len(),
        "{what} r={radius}: {} pixels vs {} from brute force",
        got.len(),
        want.len()
    );
    for (g, w) in got.iter().zip(&want) {
        assert_eq!((g.face, g.x, g.y), (w.face, w.x, w.y), "{what} r={radius}: pixel order");
        assert!(
            (g.local - w.local).length() <= 1e-9,
            "{what} r={radius}: {:?} local {:?} vs {:?}",
            (g.face, g.x, g.y),
            g.local,
            w.local
        );
        assert!(
            (g.distance - w.distance).abs() <= 1e-9,
            "{what} r={radius}: distance {} vs {}",
            g.distance,
            w.distance
        );
        assert_eq!(g.path, w.path, "{what} r={radius}: owning path");
        assert!(g.distance <= radius + 1e-9);
    }
    // Each pixel once, in (face, y, x) order.
    for w in got.windows(2) {
        let key = |p: &PixelImage| (p.face.index(), p.y, p.x);
        assert!(key(&w[0]) < key(&w[1]), "{what} r={radius}: duplicate or unsorted pixel");
    }
    got
}

#[test]
fn unfold_pixels_equals_brute_force_everywhere() {
    for (what, anchor) in anchors() {
        assert!(anchor.is_canonical(), "{what}");
        for radius in [1.5f64, 5.0, 9.5] {
            let got = assert_matches_brute_force(&what, anchor, radius);
            assert!(!got.is_empty(), "{what} r={radius}: no pixels at all");
        }
    }
}

/// Away from every seam the disc is exactly the pixel lattice disc, and its area follows
/// pi r^2 to within the lattice's own boundary error (about one pixel per unit of
/// circumference).
#[test]
fn an_interior_disc_is_the_pixel_lattice_disc() {
    let anchor = SurfacePoint::pixel_center(Face::Front, 32, 32);
    for radius in [1.5f64, 5.0, 9.5] {
        let mut got = Vec::new();
        unfold_pixels(anchor, radius, &mut got);
        // Every pixel is a direct image on the anchor's own chart.
        for p in &got {
            assert_eq!(p.face, Face::Front, "r={radius}");
            assert_eq!(p.path.len, 0, "r={radius}: interior pixels need no seam");
            let dx = f64::from(p.x) + 0.5 - anchor.u;
            let dy = f64::from(p.y) + 0.5 - anchor.v;
            assert!((p.distance - (dx * dx + dy * dy).sqrt()).abs() <= 1e-12);
            assert!((p.local.x - (f64::from(p.x) + 0.5)).abs() <= 1e-12);
            assert!((p.local.y - (f64::from(p.y) + 0.5)).abs() <= 1e-12);
        }
        // Exactly the integer lattice points inside the circle.
        let mut expected = 0;
        let k = radius.ceil() as i32;
        for dy in -k..=k {
            for dx in -k..=k {
                if f64::from(dx * dx + dy * dy) <= radius * radius {
                    expected += 1;
                }
            }
        }
        assert_eq!(got.len(), expected, "r={radius}: lattice count");
        let area = std::f64::consts::PI * radius * radius;
        assert!(
            (got.len() as f64 - area).abs() <= 3.0 * radius + 2.0,
            "r={radius}: {} pixels for an area of {area}",
            got.len()
        );
    }
}

/// "The same patch, sensor disk, body, or contact pair moved over a seam does not gain
/// energy, brightness, neighbors, or interaction range": a stamp translated by a whole
/// number of pixels across a flat seam keeps exactly the same multiset of distances.
#[test]
fn a_stamp_carried_across_a_flat_seam_keeps_its_distances() {
    let radius = 9.5;
    let mut here = Vec::new();
    unfold_pixels(SurfacePoint::new(Face::Front, 34.5, 34.5), radius, &mut here);
    let mut there = Vec::new();
    // 34.5 + 32 = 66.5, i.e. Right (2.5, 34.5): eight whole pixels past the flat seam.
    unfold_pixels(SurfacePoint::new(Face::Right, 2.5, 34.5), radius, &mut there);
    assert_eq!(here.len(), there.len(), "the stamp changed size across the seam");
    let mut a: Vec<f64> = here.iter().map(|p| p.distance).collect();
    let mut b: Vec<f64> = there.iter().map(|p| p.distance).collect();
    a.sort_by(f64::total_cmp);
    b.sort_by(f64::total_cmp);
    for (x, y) in a.iter().zip(&b) {
        assert!((x - y).abs() <= 1e-9, "distance {x} became {y} across the seam");
    }
}

/// A footprint reaching around a top vertex keeps one contribution per pixel and stays
/// within the documented radius, even though the surface has a 90 degree angular deficit
/// there ("A pixel visible through two images ... keeps only its shortest one").
#[test]
fn a_footprint_around_a_top_vertex_owns_each_pixel_once() {
    let anchor = corner(Face::Front, 64.0, 0.0);
    let mut got = Vec::new();
    unfold_pixels(anchor, 9.5, &mut got);
    let mut seen = std::collections::HashSet::new();
    let mut faces = std::collections::HashSet::new();
    for p in &got {
        assert!(seen.insert((p.face, p.x, p.y)), "{:?} appears twice", (p.face, p.x, p.y));
        faces.insert(p.face);
        assert!(p.distance <= 9.5 + 1e-9);
        assert!(p.local.is_finite());
        assert!(p.path.len <= MAX_SEAMS);
    }
    assert!(
        faces.len() >= 3,
        "a disc at a top vertex must reach all three incident charts, saw {faces:?}"
    );
    // Three quarter-discs meeting at a cone point: the count is about 3/4 of a full disc.
    let full = std::f64::consts::PI * 9.5 * 9.5;
    assert!(
        (got.len() as f64) < full + 3.0 * 9.5,
        "{} pixels is more than a full disc ({full})",
        got.len()
    );
    assert!((got.len() as f64) > 0.6 * full, "{} pixels is too few", got.len());
}

#[test]
#[should_panic]
fn a_radius_beyond_max_local_radius_panics() {
    let mut out = Vec::new();
    unfold_pixels(
        SurfacePoint::new(Face::Front, 32.0, 32.0),
        MAX_LOCAL_RADIUS + 0.5,
        &mut out,
    );
}

#[test]
#[should_panic]
fn a_non_canonical_anchor_panics() {
    let mut out = Vec::new();
    unfold_pixels(SurfacePoint::new(Face::Front, 64.0, 32.0), 4.0, &mut out);
}

/// `out` is cleared and its capacity reused, so repeated stamping allocates nothing.
#[test]
fn the_output_buffer_is_reused() {
    let mut out = Vec::new();
    unfold_pixels(SurfacePoint::new(Face::Front, 32.5, 32.5), 9.5, &mut out);
    let big = out.len();
    let cap = out.capacity();
    unfold_pixels(SurfacePoint::new(Face::Front, 32.5, 32.5), 1.5, &mut out);
    assert!(out.len() < big, "the buffer was not cleared");
    assert!(out.capacity() >= cap, "the buffer lost its capacity");
}
