//! FW-6: pixel ownership and distance on the ring, written from
//! `design/flat-world-plan-2026-09-16.md` §2 ("Unfolding: at most two images in range",
//! "Distance is a topology method, not a scaled embedding") and the FW-1 freeze.

use cubarium_surface::{
    ChartImage, Face, MAX_SEAMS, PixelImage, SurfacePoint, Topology, chart_images,
    surface_distance, unfold_pixels, unfold_with,
};

const W: u16 = 320;
const H: u16 = 180;

fn ring() -> Topology {
    Topology::Ring { w: W, h: H }
}

fn front(u: f64, v: f64) -> SurfacePoint {
    SurfacePoint::new(Face::Front, u, v)
}

/// The plan's ring distance, written out here instead of being borrowed from the crate:
/// `min(|Δu|, w − |Δu|)² + Δv²`.
fn wrapped_sq(a: &SurfacePoint, b: &SurfacePoint) -> f64 {
    let du = (a.u - b.u).abs();
    let du = du.min(f64::from(W) - du);
    let dv = a.v - b.v;
    du * du + dv * dv
}

/// The normative definition of `unfold_pixels`: every pixel centre the observer can
/// unfold to within `radius`, computed one pixel at a time.
fn brute_force(anchor: SurfacePoint, radius: f64) -> Vec<(u16, u16, f64)> {
    let mut images = Vec::new();
    chart_images(ring(), anchor.face, MAX_SEAMS, &mut images);
    let mut out = Vec::new();
    for y in 0..H {
        for x in 0..W {
            let p = SurfacePoint::pixel_center(ring(), Face::Front, x, y);
            if let Some(u) = unfold_with(ring(), &images, anchor, p, radius) {
                out.push((x, y, u.distance));
            }
        }
    }
    out
}

fn owned(anchor: SurfacePoint, radius: f64) -> Vec<PixelImage> {
    let mut out = Vec::new();
    unfold_pixels(ring(), anchor, radius, &mut out);
    out
}

// ---------------------------------------------------------------------------
// Exactly once per pixel
// ---------------------------------------------------------------------------

/// Anchors on the wrap, on both rims, in both corners and in the middle. Half-integer
/// coordinates and a radius whose square is not an integer keep every pixel strictly
/// inside or strictly outside, so the comparison has no boundary ties in it.
const ANCHORS: [(&str, f64, f64); 7] = [
    ("middle", 160.5, 90.5),
    ("on the wrap, east side", 319.5, 90.5),
    ("on the wrap, west side", 0.5, 90.5),
    ("top rim", 160.5, 0.5),
    ("bottom rim", 160.5, 179.5),
    ("top-left corner", 0.5, 0.5),
    ("bottom-right corner", 319.5, 179.5),
];

const RADIUS: f64 = 10.3;

#[test]
fn unfold_pixels_owns_every_pixel_exactly_once_across_the_wrap_and_at_the_rims() {
    for (label, u, v) in ANCHORS {
        let anchor = front(u, v);
        let got = owned(anchor, RADIUS);

        let mut seen = vec![0u32; usize::from(W) * usize::from(H)];
        for img in &got {
            assert_eq!(img.face, Face::Front, "{label}: a ring has one chart");
            let slot = usize::from(img.y) * usize::from(W) + usize::from(img.x);
            seen[slot] += 1;
            assert_eq!(
                seen[slot], 1,
                "{label}: pixel ({}, {}) owned twice",
                img.x, img.y
            );
        }

        let expected = brute_force(anchor, RADIUS);
        assert_eq!(
            got.len(),
            expected.len(),
            "{label}: {} pixels owned, {} reachable by unfold",
            got.len(),
            expected.len()
        );
        for (x, y, distance) in expected {
            let img = got
                .iter()
                .find(|i| i.x == x && i.y == y)
                .unwrap_or_else(|| panic!("{label}: pixel ({x}, {y}) is reachable but unowned"));
            assert!(
                (img.distance - distance).abs() <= 1e-9,
                "{label}: pixel ({x}, {y}) distance {} vs {distance}",
                img.distance
            );
        }
    }
}

/// The owned set is exactly the set the plan's wrapped Euclidean puts in range, and each
/// image's `local` really is the pixel centre in the anchor's chart coordinates.
#[test]
fn ownership_agrees_with_the_wrapped_euclidean_and_local_is_the_unfolded_centre() {
    for (label, u, v) in ANCHORS {
        let anchor = front(u, v);
        let got = owned(anchor, RADIUS);
        let mut expected = 0usize;
        for y in 0..H {
            for x in 0..W {
                let p = SurfacePoint::pixel_center(ring(), Face::Front, x, y);
                if wrapped_sq(&anchor, &p) <= RADIUS * RADIUS {
                    expected += 1;
                }
            }
        }
        assert_eq!(
            got.len(),
            expected,
            "{label}: owned vs wrapped-Euclidean count"
        );

        for img in &got {
            let p = SurfacePoint::pixel_center(ring(), Face::Front, img.x, img.y);
            let d = wrapped_sq(&anchor, &p).sqrt();
            assert!(
                (img.distance - d).abs() <= 1e-9,
                "{label}: ({}, {}) distance {} vs wrapped {d}",
                img.x,
                img.y,
                img.distance
            );
            let delta = img.local - anchor.chart();
            assert!(
                (delta.length() - img.distance).abs() <= 1e-9,
                "{label}: ({}, {}) local {:?} is not the pixel centre unfolded",
                img.x,
                img.y,
                img.local
            );
        }
    }
}

/// A footprint straddling the wrap owns exactly as many pixels as one in the middle of
/// the chart — the seam costs it nothing — while one on a rim is clipped.
#[test]
fn the_wrap_costs_a_footprint_no_pixels_but_a_rim_clips_it() {
    let budget = 9.0; // FOOTPRINT_PIXELS at S = 1
    let middle = owned(front(160.5, 90.5), budget).len();
    let on_wrap = owned(front(0.5, 90.5), budget).len();
    let over_wrap = owned(front(319.5, 90.5), budget).len();
    assert_eq!(
        on_wrap, middle,
        "a footprint at u = 0.5 reaches around the wrap"
    );
    assert_eq!(over_wrap, middle, "and so does one at u = 319.5");

    let on_rim = owned(front(160.5, 0.5), budget).len();
    assert!(on_rim < middle, "the top rim clips: {on_rim} vs {middle}");
    assert!(
        on_rim > middle / 3,
        "but only by about half: {on_rim} vs {middle}"
    );
}

// ---------------------------------------------------------------------------
// At most two images
// ---------------------------------------------------------------------------

/// §2: the ring enumerates its own three candidates — the direct image and the
/// translations by `+w` and `−w` — and `MAX_SEAMS` is not consulted.
#[test]
fn a_ring_enumerates_three_chart_images() {
    let mut images: Vec<ChartImage> = Vec::new();
    chart_images(ring(), Face::Front, MAX_SEAMS, &mut images);
    assert_eq!(images.len(), 3, "direct, +w and −w");
    let mut shifts: Vec<f64> = images.iter().map(|i| i.origin.x).collect();
    shifts.sort_by(f64::total_cmp);
    assert_eq!(
        shifts,
        vec![-f64::from(W), 0.0, f64::from(W)],
        "the origins are the two shifts"
    );
    for img in &images {
        assert_eq!(img.target_face, Face::Front);
        assert_eq!(img.origin.y, 0.0, "the wrap is horizontal only");
        assert_eq!(
            img.map,
            cubarium_surface::TangentMap::IDENTITY,
            "zero quarter turns"
        );
    }
}

/// §2: "at most **two** can lie within `max_local_radius()` of any observer, since the two
/// shifts are `2w` apart". Measured over a grid of observer/target pairs.
#[test]
fn at_most_two_images_are_ever_in_range() {
    let topo = ring();
    let radius = topo.max_local_radius();
    let mut images = Vec::new();
    chart_images(topo, Face::Front, MAX_SEAMS, &mut images);

    let mut worst = 0usize;
    for ou in (0..W).step_by(7) {
        let observer = front(f64::from(ou) + 0.5, 90.5);
        for tu in (0..W).step_by(11) {
            let target = front(f64::from(tu) + 0.5, 30.5);
            let in_range = images
                .iter()
                .filter(|img| {
                    (img.image_point(target.chart()) - observer.chart()).length() <= radius
                })
                .count();
            assert!(
                in_range <= 2,
                "observer u = {ou}, target u = {tu}: {in_range} images within {radius}"
            );
            worst = worst.max(in_range);
        }
    }
    // Measured, and recorded rather than merely bounded: at 320×180 the radius is 90 and
    // the shifts are 320 apart, so in fact never more than one image is in range at once.
    assert_eq!(worst, 1, "the most images ever simultaneously in range");
}

// ---------------------------------------------------------------------------
// chord_sq
// ---------------------------------------------------------------------------

#[test]
fn ring_chord_sq_is_the_wrapped_euclidean() {
    let topo = ring();
    // Straight across the seam: 2 px apart, not 318.
    let a = front(1.0, 10.0);
    let b = front(319.0, 10.0);
    assert_eq!(topo.chord_sq(&a, &b), 4.0, "the short way round");
    assert_eq!(topo.chord_sq(&b, &a), 4.0, "and it is symmetric");
    assert_eq!(topo.chord_sq(&a, &a), 0.0);
    // Exactly half way round is the same either way.
    assert_eq!(
        topo.chord_sq(&front(0.0, 0.0), &front(160.0, 0.0)),
        160.0 * 160.0
    );

    // A deterministic sweep of pairs against the formula the plan writes.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / 9007199254740992.0
    };
    for _ in 0..4000 {
        let a = front(next() * f64::from(W), next() * f64::from(H));
        let b = front(next() * f64::from(W), next() * f64::from(H));
        let expected = wrapped_sq(&a, &b);
        let got = topo.chord_sq(&a, &b);
        assert!(
            (got - expected).abs() <= 1e-9,
            "chord_sq({a:?}, {b:?}) = {got} vs {expected}"
        );
    }
}

/// On a ring the chord is not a conservative bound any more: it is the true surface
/// distance, including across the wrap.
#[test]
fn ring_chord_sq_is_the_true_surface_distance() {
    let topo = ring();
    let pairs = [
        (front(1.5, 90.5), front(319.5, 90.5)),
        (front(0.5, 20.5), front(310.5, 40.5)),
        (front(160.5, 90.5), front(200.5, 120.5)),
        (front(319.5, 5.5), front(4.5, 60.5)),
    ];
    for (a, b) in pairs {
        let d = surface_distance(topo, a, b, topo.max_local_radius())
            .unwrap_or_else(|| panic!("{a:?} to {b:?} should be within the local radius"));
        let chord = topo.chord_sq(&a, &b).sqrt();
        assert!(
            (d - chord).abs() <= 1e-9,
            "{a:?} to {b:?}: surface {d} vs chord {chord}"
        );
    }
}

/// The cube arm is unchanged: within one face the chord is the plain pixel distance.
#[test]
fn cube_chord_sq_is_still_pixel_distance_within_a_face() {
    let a = front(10.0, 10.0);
    let b = front(13.0, 14.0);
    assert!(
        (Topology::Cube.chord_sq(&a, &b) - 25.0).abs() <= 1e-9,
        "{}",
        Topology::Cube.chord_sq(&a, &b)
    );
}
