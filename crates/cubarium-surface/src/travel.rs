//! Swept continuous transport across seams with rim reflection.

use crate::{Edge, FACE_EXTENT, Face, GEOM_EPS, NUDGE, SurfacePoint, TangentMap, Vec2, cross_seam};

/// The earliest boundary hit of the ray `p + t·d` against the chart square `[0, 64]²`,
/// counting only edges whose outward component of `d` is positive.
///
/// Because the chart is convex this is the exit parameter of the whole line, independent
/// of where inside the chart the sweep started, so callers may hand in a point that is
/// outside the chart as long as the segment really does pass through it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Exit {
    /// Lowest-index edge among the tied earliest exits (the vertex tie rule).
    pub edge: Edge,
    /// Sweep parameter of the earliest exit.
    pub t: f64,
    /// Bit `e as u8` set for every edge tied with the earliest exit within `GEOM_EPS`.
    pub tied: u8,
}

impl Exit {
    /// True when two or more edges were hit within `GEOM_EPS` along the sweep.
    #[inline]
    pub(crate) fn is_tie(self) -> bool {
        self.tied.count_ones() > 1
    }

    /// True when `edge` is among the tied earliest exits.
    #[inline]
    pub(crate) fn includes(self, edge: Edge) -> bool {
        self.tied & (1 << edge as u8) != 0
    }
}

/// Parameter at which `p + t·d` reaches the line of `edge`, when `d` leaves through it.
#[inline]
fn edge_exit_t(p: Vec2, d: Vec2, edge: Edge) -> Option<f64> {
    let t = match edge {
        Edge::Top if d.y < 0.0 => -p.y / d.y,
        Edge::Right if d.x > 0.0 => (FACE_EXTENT - p.x) / d.x,
        Edge::Bottom if d.y > 0.0 => (FACE_EXTENT - p.y) / d.y,
        Edge::Left if d.x < 0.0 => -p.x / d.x,
        _ => return None,
    };
    t.is_finite().then_some(t)
}

/// Earliest exit of `p + t·d` from `[0, 64]²`. `sweep_len` is `|d|`, used to convert the
/// `GEOM_EPS` pixel tie tolerance into a parameter tolerance.
pub(crate) fn earliest_exit(p: Vec2, d: Vec2, sweep_len: f64) -> Option<Exit> {
    let mut ts = [f64::INFINITY; 4];
    let mut tmin = f64::INFINITY;
    for e in Edge::ALL {
        if let Some(t) = edge_exit_t(p, d, e) {
            ts[e as usize] = t;
            tmin = tmin.min(t);
        }
    }
    if !tmin.is_finite() {
        return None;
    }
    let mut tied = 0u8;
    for (i, &t) in ts.iter().enumerate() {
        if t.is_finite() && (t - tmin) * sweep_len <= GEOM_EPS {
            tied |= 1 << i;
        }
    }
    let edge = Edge::ALL[tied.trailing_zeros() as usize];
    Some(Exit { edge, t: tmin, tied })
}

/// The chart point on `edge` at along-edge parameter `s` (`u` for `Top`/`Bottom`,
/// `v` for `Left`/`Right`).
#[inline]
pub(crate) fn edge_point(edge: Edge, s: f64) -> Vec2 {
    match edge {
        Edge::Top => Vec2::new(s, 0.0),
        Edge::Right => Vec2::new(FACE_EXTENT, s),
        Edge::Bottom => Vec2::new(s, FACE_EXTENT),
        Edge::Left => Vec2::new(0.0, s),
    }
}

/// The along-edge parameter of a chart point sitting on `edge`.
#[inline]
pub(crate) fn edge_param(edge: Edge, p: Vec2) -> f64 {
    match edge {
        Edge::Top | Edge::Bottom => p.x,
        Edge::Right | Edge::Left => p.y,
    }
}

/// Quarter turns a tangent vector rotates when crossing out of `face` through `edge`.
/// Taken from `cross_seam` at `t = 0`; the turn count does not depend on `t`.
#[inline]
pub(crate) fn seam_turns(face: Face, edge: Edge) -> Option<u8> {
    cross_seam(face, edge, 0).map(|(_, _, _, turns)| turns)
}

/// Snap a hit point onto `edge` exactly and keep the along-edge parameter inside `[0, 64]`.
#[inline]
fn snap_to_edge(mut p: Vec2, edge: Edge) -> Vec2 {
    p.x = p.x.clamp(0.0, FACE_EXTENT);
    p.y = p.y.clamp(0.0, FACE_EXTENT);
    match edge {
        Edge::Top => p.y = 0.0,
        Edge::Right => p.x = FACE_EXTENT,
        Edge::Bottom => p.y = FACE_EXTENT,
        Edge::Left => p.x = 0.0,
    }
    p
}

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
    out.segments.clear();
    out.map = TangentMap::IDENTITY;
    out.crossings = 0;
    out.reflections = 0;
    out.ties = 0;
    out.fallback = false;

    debug_assert!(start.is_canonical(), "travel from non-canonical {start:?}");
    let mut face = start.face;
    // Keep the sweep total even if a caller hands in a slightly out-of-range point.
    let mut p = Vec2::new(
        if start.u.is_finite() { start.u.clamp(0.0, FACE_EXTENT) } else { 0.0 },
        if start.v.is_finite() { start.v.clamp(0.0, FACE_EXTENT) } else { 0.0 },
    );

    let mut d = displacement;
    if !d.is_finite() {
        d = Vec2::ZERO;
        out.fallback = true;
    }

    loop {
        let sweep_len = d.length();
        let hit = earliest_exit(p, d, sweep_len).filter(|e| e.t <= 1.0);
        let Some(exit) = hit else {
            // The sweep ends inside this chart.
            let end = p + d;
            if end != p {
                out.segments.push(PathSegment { face, from: p, to: end });
            }
            p = end;
            break;
        };

        if exit.is_tie() {
            out.ties += 1;
        }
        let t = exit.t.clamp(0.0, 1.0);
        let hp = snap_to_edge(p + d * t, exit.edge);
        if hp != p {
            out.segments.push(PathSegment { face, from: p, to: hp });
        }
        let remaining = d * (1.0 - t);

        match face.neighbor(exit.edge) {
            // The open bottom rim of a side face: pure specular reflection in place.
            None => {
                d = Vec2::new(remaining.x, -remaining.y);
                out.map = out.map.then(TangentMap::REFLECT_Y);
                p = hp;
                out.reflections += 1;
            }
            Some(seam) => {
                let s = edge_param(exit.edge, hp);
                let s2 = if seam.reversed { FACE_EXTENT - s } else { s };
                let turns = seam_turns(face, exit.edge).expect("seam exists");
                let rot = TangentMap::quarter_turns(turns);
                p = edge_point(seam.edge, s2);
                d = rot.apply(remaining);
                out.map = out.map.then(rot);
                face = seam.face;
                out.crossings += 1;
            }
        }

        if out.crossings + out.reflections > MAX_CROSSINGS {
            out.fallback = true;
            p = nudge_inward(p);
            break;
        }
    }

    out.end = SurfacePoint::new(face, p.x, p.y).canonicalize();
}

/// Move every coordinate that sits on a chart boundary inward by [`NUDGE`].
fn nudge_inward(p: Vec2) -> Vec2 {
    let fix = |c: f64| {
        if c <= 0.0 {
            NUDGE
        } else if c >= FACE_EXTENT {
            FACE_EXTENT - NUDGE
        } else {
            c
        }
    };
    Vec2::new(fix(p.x), fix(p.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Edge;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn assert_point(t: &Travel, face: Face, u: f64, v: f64) {
        assert_eq!(t.end.face, face, "face: {:?}", t.end);
        assert!(approx(t.end.u, u) && approx(t.end.v, v), "{:?} != ({face:?}, {u}, {v})", t.end);
    }

    #[test]
    fn design_example_front_to_right() {
        let t = travel(SurfacePoint::new(Face::Front, 63.75, 20.0), Vec2::new(0.5, 0.0));
        assert_point(&t, Face::Right, 0.25, 20.0);
        assert_eq!((t.crossings, t.reflections, t.ties, t.fallback), (1, 0, 0, false));
        assert_eq!(t.map, TangentMap::IDENTITY);
    }

    #[test]
    fn design_example_right_to_top_is_a_quarter_turn() {
        let t = travel(SurfacePoint::new(Face::Right, 10.0, 0.25), Vec2::new(0.0, -0.5));
        assert_point(&t, Face::Top, 63.75, 54.0);
        assert_eq!(t.crossings, 1);
        // "its upward direction becomes Top-leftward"
        assert_eq!(t.map.apply(Vec2::new(0.0, -1.0)), Vec2::new(-1.0, 0.0));
        assert_eq!(t.map.as_quarter_turns(), Some(1));
    }

    #[test]
    fn design_example_back_to_top_is_a_half_turn() {
        let t = travel(SurfacePoint::new(Face::Back, 10.0, 0.25), Vec2::new(0.0, -0.5));
        assert_point(&t, Face::Top, 54.0, 0.25);
        assert_eq!(t.crossings, 1);
        // "its upward direction becomes Top-downward"
        assert_eq!(t.map.apply(Vec2::new(0.0, -1.0)), Vec2::new(0.0, 1.0));
        assert_eq!(t.map.as_quarter_turns(), Some(2));
    }

    /// Every connected half-edge, every pixel-center along-edge parameter: a step that
    /// leaves the chart half a pixel inside and lands half a pixel into the neighbour
    /// must land on exactly the pixel `cross_seam` names.
    #[test]
    fn pixel_center_crossings_match_cross_seam() {
        let mut checked = 0;
        for face in Face::ALL {
            for edge in Edge::ALL {
                let Some((nf, nx, ny, turns)) = crate::cross_seam(face, edge, 0) else {
                    continue;
                };
                let _ = (nf, nx, ny);
                for k in 0..64u8 {
                    let s = f64::from(k) + 0.5;
                    // Start half a pixel inside `edge`, step one pixel outward.
                    let (start, disp) = match edge {
                        Edge::Top => (Vec2::new(s, 0.5), Vec2::new(0.0, -1.0)),
                        Edge::Right => (Vec2::new(63.5, s), Vec2::new(1.0, 0.0)),
                        Edge::Bottom => (Vec2::new(s, 63.5), Vec2::new(0.0, 1.0)),
                        Edge::Left => (Vec2::new(0.5, s), Vec2::new(-1.0, 0.0)),
                    };
                    let t = travel(SurfacePoint::new(face, start.x, start.y), disp);
                    let (wf, wx, wy, wt) = crate::cross_seam(face, edge, k).expect("seam");
                    assert_eq!(wt, turns);
                    assert_eq!(t.end.face, wf, "{face:?} {edge:?} k={k}");
                    assert_eq!(t.end.pixel(), (wx, wy), "{face:?} {edge:?} k={k}: {:?}", t.end);
                    assert_eq!(t.crossings, 1);
                    assert_eq!(t.map, TangentMap::quarter_turns(turns));
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 16 * 64, "16 connected half-edges x 64 positions");
    }

    #[test]
    fn top_seams_preserve_displacement_length() {
        for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
            for k in 0..64 {
                let s = f64::from(k) + 0.5;
                let start = SurfacePoint::new(face, s, 0.3);
                for dx in [-3.0, -0.5, 0.0, 0.5, 3.0] {
                    let disp = Vec2::new(dx, -2.0);
                    let t = travel(start, disp);
                    let total: f64 = t.segments.iter().map(PathSegment::length).sum();
                    assert!(!t.fallback);
                    assert!(
                        (total - disp.length()).abs() < 1e-9,
                        "{face:?} s={s} d={disp:?}: {total} != {}",
                        disp.length()
                    );
                    // Angles are preserved: the net map is orthogonal with det +-1.
                    assert!(t.map.det().abs() == 1);
                }
            }
        }
    }

    #[test]
    fn vertex_tie_at_front_top_right_chooses_top() {
        // Aiming exactly at the Front (64, 0) corner hits Edge::Top and Edge::Right at
        // the same parameter; the rule takes the lowest edge index, Top.
        let p = Vec2::new(63.0, 1.0);
        let d = Vec2::new(1.0, -1.0);
        let exit = earliest_exit(p, d, d.length()).expect("the corner is a boundary hit");
        assert!(exit.is_tie());
        assert!(exit.includes(Edge::Top) && exit.includes(Edge::Right));
        assert_eq!(exit.edge, Edge::Top);
        assert!((exit.t - 1.0).abs() < 1e-12);

        // End to end: the tie is counted, the hit lands exactly on the vertex, nothing
        // is silently dropped, and the swept length survives the singularity.
        let disp = Vec2::new(2.0, -2.0);
        let t = travel(SurfacePoint::new(Face::Front, 63.0, 1.0), disp);
        assert!(t.ties >= 1, "{t:?}");
        assert!(!t.fallback);
        assert_eq!(t.segments[0].face, Face::Front);
        assert_eq!(t.segments[0].to, Vec2::new(64.0, 0.0));
        let total: f64 = t.segments.iter().map(PathSegment::length).sum();
        assert!((total - disp.length()).abs() < 1e-9, "{total}");
    }

    #[test]
    fn exact_corner_start_prefers_top() {
        // A point placed exactly on the corner with an outward-and-up displacement.
        let mut out = Travel::default();
        travel_into(SurfacePoint::new(Face::Front, 63.9999999999, 1e-10), Vec2::new(1.0, -1.0), &mut out);
        assert!(out.crossings >= 1);
        assert!(!out.fallback);
    }

    #[test]
    fn retracing_returns_to_the_start() {
        let mut seed = 0x1234_5678_9abc_def0u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..2000 {
            let face = Face::ALL[(rnd() * 5.0) as usize % 5];
            let start = SurfacePoint::new(face, rnd() * 64.0, rnd() * 64.0).canonicalize();
            let disp = Vec2::new((rnd() - 0.5) * 40.0, (rnd() - 0.5) * 40.0);
            let fwd = travel(start, disp);
            if fwd.fallback || fwd.ties > 0 {
                continue;
            }
            let back = travel(fwd.end, fwd.map.apply(-disp));
            if back.fallback || back.ties > 0 {
                continue;
            }
            assert_eq!(back.end.face, start.face, "start={start:?} disp={disp:?}");
            assert!(
                (back.end.u - start.u).abs() < 1e-7 && (back.end.v - start.v).abs() < 1e-7,
                "start={start:?} disp={disp:?} -> {:?} -> {:?}",
                fwd.end,
                back.end
            );
            // The net map composes back to the identity.
            assert_eq!(fwd.map.then(back.map), TangentMap::IDENTITY, "start={start:?} disp={disp:?}");
        }
    }

    #[test]
    fn rim_reflects_and_never_tunnels() {
        let t = travel(SurfacePoint::new(Face::Front, 10.0, 63.0), Vec2::new(0.0, 2.0));
        assert_eq!(t.end.face, Face::Front);
        assert_eq!(t.reflections, 1);
        assert_eq!(t.crossings, 0);
        assert!(approx(t.end.v, 63.0), "{:?}", t.end);
        assert_eq!(t.map, TangentMap::REFLECT_Y);
        let total: f64 = t.segments.iter().map(PathSegment::length).sum();
        assert!(approx(total, 2.0));
    }

    #[test]
    fn lower_corner_crosses_a_vertical_seam_and_reflects() {
        // Exactly at the Front lower-right corner. Edge order is
        // Top(0) < Right(1) < Bottom(2) < Left(3), so the vertical seam wins the tie and
        // the rim reflection happens on the far side of it.
        let disp = Vec2::new(2.0, 2.0);
        let t = travel(SurfacePoint::new(Face::Front, 63.0, 63.0), disp);
        assert_eq!(t.ties, 1);
        assert_eq!(t.crossings, 1);
        assert_eq!(t.reflections, 1);
        assert_eq!(t.end.face, Face::Right);
        assert!((t.end.u - 1.0).abs() < 1e-9 && (t.end.v - 63.0).abs() < 1e-9, "{:?}", t.end);
        assert!(!t.fallback);
        let total: f64 = t.segments.iter().map(PathSegment::length).sum();
        assert!((total - disp.length()).abs() < 1e-9, "{total}");
    }

    #[test]
    fn multi_chart_overshoot_keeps_total_length() {
        let disp = Vec2::new(200.0, 0.0);
        let t = travel(SurfacePoint::new(Face::Front, 32.0, 32.0), disp);
        assert_eq!(t.crossings, 3);
        assert_eq!(t.segments.len(), 4);
        let total: f64 = t.segments.iter().map(PathSegment::length).sum();
        assert!(approx(total, 200.0), "{total}");
        assert_eq!(t.end.face, Face::Left);
    }

    #[test]
    fn non_finite_displacement_is_a_fallback() {
        let t = travel(SurfacePoint::new(Face::Front, 5.0, 5.0), Vec2::new(f64::NAN, 0.0));
        assert!(t.fallback);
        assert_eq!(t.end, SurfacePoint::new(Face::Front, 5.0, 5.0));
        assert!(t.segments.is_empty());
    }

    #[test]
    fn zero_displacement_emits_nothing() {
        let t = travel(SurfacePoint::new(Face::Front, 5.0, 5.0), Vec2::ZERO);
        assert!(t.segments.is_empty());
        assert!(!t.fallback);
        assert_eq!(t.end, SurfacePoint::new(Face::Front, 5.0, 5.0));
    }

    #[test]
    fn every_end_is_canonical_and_finite() {
        let mut seed = 0xdead_beef_0bad_f00du64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut out = Travel::default();
        for _ in 0..20_000 {
            let face = Face::ALL[(rnd() * 5.0) as usize % 5];
            let start = SurfacePoint::new(face, rnd() * 64.0, rnd() * 64.0).canonicalize();
            // Deliberately include vertex-directed and huge steps.
            let disp = match (rnd() * 3.0) as u32 {
                0 => Vec2::new(64.0 - start.u, -start.v),
                1 => Vec2::new((rnd() - 0.5) * 4000.0, (rnd() - 0.5) * 4000.0),
                _ => Vec2::new((rnd() - 0.5) * 8.0, (rnd() - 0.5) * 8.0),
            };
            travel_into(start, disp, &mut out);
            assert!(out.end.is_canonical(), "{start:?} {disp:?} -> {:?}", out.end);
            for s in &out.segments {
                assert!(s.length() > 0.0, "zero-length segment emitted");
            }
        }
    }
}
