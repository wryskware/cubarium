//! FW-6: the ring's transport, written from `design/flat-world-plan-2026-09-16.md` §2
//! and `design/7_Research/flat-world-fw1-2026-09-16.md` §5, not from the implementation.
//!
//! The plan's claims under test:
//!
//! * the vertical seam is the chart to itself — exiting at `u = w` enters at `u = 0`,
//!   along-edge parameter unchanged, zero quarter turns, `TangentMap::IDENTITY`;
//! * the only reflection is the existing rim bounce, now at **both** `v = 0` and `v = h`;
//! * a ring corner is the cube's lower side corner: the existing lowest-`Edge` tie rule
//!   applies unchanged, and one displacement may both cross and reflect;
//! * the swept length is preserved and the fallback never fires;
//! * the same fixture at twice the extent gives exactly twice the coordinates.

use cubarium_surface::{SurfacePoint, TangentMap, Topology, Travel, Vec2, travel};

/// The §6 ladder's S = 1 raster.
const W: f64 = 320.0;
const H: f64 = 180.0;

fn ring() -> Topology {
    Topology::Ring { w: 320, h: 180 }
}

/// The same ring at twice the extent (the S = 2 rung of §6's ladder).
fn ring_2x() -> Topology {
    Topology::Ring { w: 640, h: 360 }
}

fn front(u: f64, v: f64) -> SurfacePoint {
    SurfacePoint::new(cubarium_surface::Face::Front, u, v)
}

#[track_caller]
fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-9,
        "{what}: expected {expected}, got {actual} (delta {})",
        actual - expected
    );
}

/// The length actually swept, which the plan requires to equal `|displacement|`.
fn swept(t: &Travel) -> f64 {
    t.segments.iter().map(|s| s.length()).sum()
}

#[track_caller]
fn assert_sound(t: &Travel, topo: Topology, displacement: Vec2, what: &str) {
    assert!(!t.fallback, "{what}: the forward-progress fallback fired");
    assert!(t.end.is_canonical(topo), "{what}: end {:?} is not canonical", t.end);
    assert_close(swept(t), displacement.length(), &format!("{what}: swept length"));
}

// ---------------------------------------------------------------------------
// The self-seam
// ---------------------------------------------------------------------------

/// Exiting at `u = w` enters at `u = 0` with `v` untouched: a pure translation by `-w`.
#[test]
fn self_seam_exit_at_w_enters_at_zero() {
    let d = Vec2::new(1.0, 0.0);
    let t = travel(ring(), front(319.5, 90.5), d);
    assert_sound(&t, ring(), d, "wrap east");
    assert_close(t.end.u, 0.5, "end u");
    assert_close(t.end.v, 90.5, "end v (the along-edge parameter is unchanged)");
    assert_eq!(t.crossings, 1, "one seam crossing");
    assert_eq!(t.reflections, 0, "the wrap is not a reflection");
    assert_eq!(t.ties, 0);
    assert_eq!(t.map, TangentMap::IDENTITY, "zero quarter turns across the self-seam");
    assert_eq!(t.segments.len(), 2, "one segment per chart visit");
    assert_close(t.segments[0].to.x, W, "the first segment ends on the seam");
    assert_close(t.segments[1].from.x, 0.0, "and the second starts at u = 0");
}

/// And the other way: exiting at `u = 0` enters at `u = w`.
#[test]
fn self_seam_exit_at_zero_enters_at_w() {
    let d = Vec2::new(-1.0, 0.0);
    let t = travel(ring(), front(0.5, 90.5), d);
    assert_sound(&t, ring(), d, "wrap west");
    assert_close(t.end.u, 319.5, "end u");
    assert_close(t.end.v, 90.5, "end v");
    assert_eq!((t.crossings, t.reflections, t.ties), (1, 0, 0));
    assert_eq!(t.map, TangentMap::IDENTITY);
}

/// Transport across the self-seam is the identity on tangent vectors, so a diagonal
/// heading comes out of the wrap pointing exactly where it went in.
#[test]
fn self_seam_transport_is_the_identity_map() {
    let d = Vec2::new(3.0, 4.0);
    let t = travel(ring(), front(318.0, 90.0), d);
    assert_sound(&t, ring(), d, "diagonal wrap");
    assert_eq!(t.crossings, 1);
    assert_eq!(t.reflections, 0);
    assert_eq!(t.map, TangentMap::IDENTITY);
    // The displacement is unrotated, so the end is the start advanced by (3, 4) mod w.
    assert_close(t.end.u, 318.0 + 3.0 - W, "end u");
    assert_close(t.end.v, 94.0, "end v");
    // A heading transported across the wrap is unchanged.
    let heading = Vec2::new(-0.6, 0.8);
    let carried = t.map.apply(heading);
    assert_close(carried.x, heading.x, "carried heading x");
    assert_close(carried.y, heading.y, "carried heading y");
}

// ---------------------------------------------------------------------------
// Both rims reflect
// ---------------------------------------------------------------------------

/// `v = 0` is a rim on a ring — the cube's open bottom, moved to the top as well.
#[test]
fn top_rim_reflects() {
    let d = Vec2::new(0.0, -3.0);
    let t = travel(ring(), front(160.5, 1.0), d);
    assert_sound(&t, ring(), d, "top rim");
    assert_close(t.end.u, 160.5, "u is untouched by REFLECT_Y");
    assert_close(t.end.v, 2.0, "1 down to the rim, 2 back up");
    assert_eq!((t.crossings, t.reflections, t.ties), (0, 1, 0));
    assert_eq!(t.map, TangentMap::REFLECT_Y, "the rim bounce is REFLECT_Y");
}

/// `v = h` is the other rim, and it reflects the same way.
#[test]
fn bottom_rim_reflects() {
    let d = Vec2::new(0.0, 3.0);
    let t = travel(ring(), front(160.5, 179.0), d);
    assert_sound(&t, ring(), d, "bottom rim");
    assert_close(t.end.u, 160.5, "u is untouched");
    assert_close(t.end.v, 178.0, "1 down to the rim, 2 back up");
    assert_eq!((t.crossings, t.reflections, t.ties), (0, 1, 0));
    assert_eq!(t.map, TangentMap::REFLECT_Y);
}

/// Nothing tunnels through a rim: a long vertical sweep bounces between the two rims and
/// stays inside the chart, having spent its whole length.
#[test]
fn a_long_vertical_sweep_bounces_between_both_rims() {
    let d = Vec2::new(0.0, 1000.0);
    let t = travel(ring(), front(160.5, 90.0), d);
    assert_sound(&t, ring(), d, "long vertical sweep");
    assert_eq!(t.crossings, 0, "a vertical sweep never reaches the seam");
    assert!(t.reflections >= 5, "1000 px over a 180 px chart is at least five bounces");
    // 90 px to the bottom rim, then a full 180 px chart per bounce: 90 + 5·180 = 990 < 1000,
    // so six bounces, and an even number of REFLECT_Y composes back to the identity.
    assert_eq!(t.reflections, 6, "bounces");
    assert_eq!(t.map, TangentMap::IDENTITY, "an even number of bounces composes to identity");
    // Unfolded, a billiard in [0, h] is the triangle wave of `v0 + s` with period 2h.
    let x = (90.0 + 1000.0) % (2.0 * H);
    assert_close(t.end.v, if x <= H { x } else { 2.0 * H - x }, "end v after the bounces");
}

// ---------------------------------------------------------------------------
// The four corners, exact ties
// ---------------------------------------------------------------------------

/// The plan's corner claim, per corner: the aim is exactly at the vertex, two edges tie,
/// the sweep costs exactly one crossing, one reflection and one tie, and the end point is
/// the one both event orders agree on (a rim bounce and a translation by `w` commute).
#[track_caller]
fn assert_corner(
    label: &str,
    topo: Topology,
    start: SurfacePoint,
    d: Vec2,
    expected_end: (f64, f64),
) {
    let t = travel(topo, start, d);
    assert_sound(&t, topo, d, label);
    assert_close(t.end.u, expected_end.0, &format!("{label}: end u"));
    assert_close(t.end.v, expected_end.1, &format!("{label}: end v"));
    assert_eq!(
        (t.crossings, t.reflections, t.ties),
        (1, 1, 1),
        "{label}: (crossings, reflections, ties)"
    );
    assert_eq!(t.map, TangentMap::REFLECT_Y, "{label}: one reflection, no quarter turns");
    assert_eq!(t.segments.len(), 2, "{label}: two charts visited, no zero-length segment");
}

#[test]
fn exact_tie_at_the_top_left_corner() {
    // Top (0) beats Left (3): the rim bounce resolves the tie, then the wrap follows.
    assert_corner("top-left", ring(), front(1.0, 1.0), Vec2::new(-2.0, -2.0), (319.0, 1.0));
}

#[test]
fn exact_tie_at_the_top_right_corner() {
    // Top (0) beats Right (1).
    assert_corner("top-right", ring(), front(319.0, 1.0), Vec2::new(2.0, -2.0), (1.0, 1.0));
}

#[test]
fn exact_tie_at_the_bottom_left_corner() {
    // Bottom (2) beats Left (3).
    assert_corner("bottom-left", ring(), front(1.0, 179.0), Vec2::new(-2.0, 2.0), (319.0, 179.0));
}

#[test]
fn exact_tie_at_the_bottom_right_corner() {
    // Right (1) beats Bottom (2): this is the one corner where the seam crossing is the
    // first event. The outcome is the same as the other three because a ring's rim
    // reflection and its wrap commute (see the report accompanying this file).
    assert_corner("bottom-right", ring(), front(319.0, 179.0), Vec2::new(2.0, 2.0), (1.0, 179.0));
}

/// Skewing either component by 2% breaks the tie, and the step still costs exactly one
/// crossing and one reflection whichever edge wins.
#[test]
fn near_ties_at_every_corner_resolve_to_one_edge() {
    let corners: [(&str, f64, f64, f64, f64); 4] = [
        ("top-left", 1.0, 1.0, -2.0, -2.0),
        ("top-right", 319.0, 1.0, 2.0, -2.0),
        ("bottom-left", 1.0, 179.0, -2.0, 2.0),
        ("bottom-right", 319.0, 179.0, 2.0, 2.0),
    ];
    for (name, u, v, du, dv) in corners {
        for (skew, which) in [(1.02, "u"), (1.0 / 1.02, "u")] {
            let d = Vec2::new(du * skew, dv);
            let label = format!("{name} near-tie ({which} × {skew:.4})");
            let t = travel(ring(), front(u, v), d);
            assert_sound(&t, ring(), d, &label);
            assert_eq!(t.ties, 0, "{label}: skewing 2% must break the tie");
            assert_eq!(t.crossings, 1, "{label}: one crossing");
            assert_eq!(t.reflections, 1, "{label}: one reflection");
        }
        for skew in [1.02, 1.0 / 1.02] {
            let d = Vec2::new(du, dv * skew);
            let label = format!("{name} near-tie (v × {skew:.4})");
            let t = travel(ring(), front(u, v), d);
            assert_sound(&t, ring(), d, &label);
            assert_eq!(t.ties, 0, "{label}: skewing 2% must break the tie");
            assert_eq!(t.crossings, 1, "{label}: one crossing");
            assert_eq!(t.reflections, 1, "{label}: one reflection");
        }
    }
}

// ---------------------------------------------------------------------------
// Compound displacements
// ---------------------------------------------------------------------------

/// One displacement that both wraps and reflects, away from any corner, so the two events
/// are ordered by geometry rather than by the tie rule.
#[test]
fn one_step_wraps_and_then_reflects() {
    let d = Vec2::new(10.0, 4.0);
    let t = travel(ring(), front(318.0, 178.0), d);
    assert_sound(&t, ring(), d, "wrap then reflect");
    assert_eq!((t.crossings, t.reflections, t.ties), (1, 1, 0));
    assert_eq!(t.map, TangentMap::REFLECT_Y);
    assert_eq!(t.segments.len(), 3, "three chart visits: before, after, after the bounce");
    assert_close(t.end.u, 8.0, "end u");
    assert_close(t.end.v, 178.0, "end v: 2 px past the rim, bounced back");
    // The seam crossing preserved the along-edge parameter.
    assert_close(t.segments[0].to.y, 178.8, "v at the seam, leaving");
    assert_close(t.segments[1].from.y, 178.8, "v at the seam, entering");
}

/// A displacement longer than the ring's circumference wraps more than once.
#[test]
fn a_displacement_longer_than_the_circumference_wraps_twice() {
    let d = Vec2::new(700.0, 0.0);
    let t = travel(ring(), front(10.5, 90.5), d);
    assert_sound(&t, ring(), d, "two wraps");
    assert_eq!(t.crossings, 2, "700 px from u = 10.5 crosses u = w twice");
    assert_eq!(t.reflections, 0);
    assert_eq!(t.map, TangentMap::IDENTITY, "two identity transports compose to identity");
    assert_close(t.end.u, 10.5 + 700.0 - 2.0 * W, "end u");
    assert_close(t.end.v, 90.5, "end v");
    assert_eq!(t.segments.len(), 3);
}

/// Sanity on the rim that is not a seam: no displacement, however long, leaves `[0, h]`.
#[test]
fn a_long_diagonal_stays_inside_the_chart() {
    let d = Vec2::new(997.0, 613.0);
    let t = travel(ring(), front(7.25, 33.75), d);
    assert_sound(&t, ring(), d, "long diagonal");
    assert!(t.end.u >= 0.0 && t.end.u < W, "u in range: {}", t.end.u);
    assert!(t.end.v >= 0.0 && t.end.v < H, "v in range: {}", t.end.v);
    assert!(t.crossings >= 3, "997 px wraps at least three times");
    assert!(t.reflections >= 3, "613 px bounces at least three times");
}

// ---------------------------------------------------------------------------
// Scale
// ---------------------------------------------------------------------------

/// The same fixtures on the 640×360 ring give exactly twice the coordinates with exactly
/// the same counters. (`travel` is scale-free: what doubles is the topology's extent.)
#[test]
fn the_double_size_ring_doubles_every_coordinate() {
    // The bottom-right corner tie, doubled.
    assert_corner(
        "bottom-right at 2x",
        ring_2x(),
        front(638.0, 358.0),
        Vec2::new(4.0, 4.0),
        (2.0, 358.0),
    );
    // The top-left corner tie, doubled.
    assert_corner("top-left at 2x", ring_2x(), front(2.0, 2.0), Vec2::new(-4.0, -4.0), (638.0, 2.0));

    // The double wrap, doubled.
    let d = Vec2::new(1400.0, 0.0);
    let t = travel(ring_2x(), front(21.0, 181.0), d);
    assert_sound(&t, ring_2x(), d, "two wraps at 2x");
    assert_eq!(t.crossings, 2);
    assert_close(t.end.u, 2.0 * (10.5 + 700.0 - 2.0 * W), "end u is twice the S = 1 end u");
    assert_close(t.end.v, 2.0 * 90.5, "end v is twice the S = 1 end v");
}
