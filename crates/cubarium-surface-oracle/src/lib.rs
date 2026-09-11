//! cubarium-surface-oracle — a slow, independent 3D reference for local surface queries.
//!
//! Test support only. This crate deliberately does **not** depend on `cubarium-surface`
//! and does not use `Face::neighbor`, `cross_seam`, or any 2D seam table. Its only
//! geometric input is the shim's verified cube embedding `cube_proto::geometry::pixel_direction`,
//! from which it reconstructs each face square in 3D. Adjacency is discovered from shared
//! corners; seam rotations are rigid 3D rotations about the actual shared cube edge; the
//! rim is whichever square edge no other square shares. Correctness therefore rests on
//! 3D rigid geometry rather than on the transition arithmetic the production crate uses.
//!
//! Scope: local queries (points within `MAX_RADIUS` of each other, sweeps of bounded
//! length). It is not a general geodesic solver.

#![forbid(unsafe_code)]

use cube_proto::geometry::pixel_direction;

pub use cube_proto::{Edge, Face};

/// Largest query radius the oracle enumerates paths for, in pixels (chart paths of at
/// most two seams are complete below one face width; the same bound as production).
pub const MAX_RADIUS: f64 = 32.0;

pub type P3 = [f64; 3];

/// A chart-plane segment `(start, end)` in pixel coordinates.
type ChartEdge = ((f64, f64), (f64, f64));

// --- minimal 3D vector algebra -------------------------------------------------------

#[inline]
fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
fn scale(a: P3, s: f64) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn norm(a: P3) -> f64 {
    dot(a, a).sqrt()
}

#[inline]
fn dist(a: P3, b: P3) -> f64 {
    norm(sub(a, b))
}

fn unit(a: P3) -> P3 {
    let n = norm(a);
    assert!(n > 0.0 && n.is_finite(), "cannot normalize {a:?}");
    scale(a, 1.0 / n)
}

/// Rotate `v` about the unit axis `k` by the angle with the given cosine and sine.
fn rodrigues(v: P3, k: P3, c: f64, s: f64) -> P3 {
    add(
        add(scale(v, c), scale(cross(k, v), s)),
        scale(k, dot(k, v) * (1.0 - c)),
    )
}

/// A rigid map `p -> r·p + t` used to unfold successive squares into the observer's plane.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rigid {
    r: [[f64; 3]; 3],
    t: P3,
}

impl Rigid {
    const ID: Rigid = Rigid {
        r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0, 0.0, 0.0],
    };

    fn apply(&self, p: P3) -> P3 {
        add(
            [dot(self.r[0], p), dot(self.r[1], p), dot(self.r[2], p)],
            self.t,
        )
    }

    /// The rotation about the line through `a` with unit direction `k` by the angle with
    /// the given cosine and sine.
    fn about_axis(a: P3, k: P3, c: f64, s: f64) -> Rigid {
        let col = |e: P3| rodrigues(e, k, c, s);
        let cx = col([1.0, 0.0, 0.0]);
        let cy = col([0.0, 1.0, 0.0]);
        let cz = col([0.0, 0.0, 1.0]);
        let r = [
            [cx[0], cy[0], cz[0]],
            [cx[1], cy[1], cz[1]],
            [cx[2], cy[2], cz[2]],
        ];
        let ra = [dot(r[0], a), dot(r[1], a), dot(r[2], a)];
        Rigid { r, t: sub(a, ra) }
    }

    /// `outer ∘ inner`.
    fn compose(outer: &Rigid, inner: &Rigid) -> Rigid {
        let mut r = [[0.0; 3]; 3];
        for (row, orow) in r.iter_mut().zip(outer.r.iter()) {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = orow[0] * inner.r[0][j] + orow[1] * inner.r[1][j] + orow[2] * inner.r[2][j];
            }
        }
        let t = add(
            [
                dot(outer.r[0], inner.t),
                dot(outer.r[1], inner.t),
                dot(outer.r[2], inner.t),
            ],
            outer.t,
        );
        Rigid { r, t }
    }
}

// --- face squares ---------------------------------------------------------------------

/// One face square in 3D with its chart frame reconstructed from pixel centers:
/// `embed(u, v) = origin + u·du + v·dv` in unit-cube coordinates (`du`, `dv` have length
/// `1/32`), and `normal` is the outward unit normal (`du × dv` oriented outward by the
/// square's center direction).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceSquare {
    pub face: Face,
    pub origin: P3,
    pub du: P3,
    pub dv: P3,
    pub normal: P3,
}

impl FaceSquare {
    /// Reconstructed from `pixel_direction(face, 0, 0)`, `(1, 0)` and `(0, 1)` only.
    pub fn of(face: Face) -> FaceSquare {
        // Pixel (x, y) has its center at chart coordinates (x + 0.5, y + 0.5), so the
        // per-pixel chart steps are the differences of neighboring pixel centers and the
        // chart origin sits half a pixel back along each.
        let p00 = pixel_direction(face, 0, 0);
        let p10 = pixel_direction(face, 1, 0);
        let p01 = pixel_direction(face, 0, 1);
        let du = sub(p10, p00);
        let dv = sub(p01, p00);
        let origin = sub(p00, add(scale(du, 0.5), scale(dv, 0.5)));
        let center = add(origin, add(scale(du, 32.0), scale(dv, 32.0)));
        let mut normal = unit(cross(du, dv));
        if dot(normal, center) < 0.0 {
            normal = scale(normal, -1.0);
        }
        FaceSquare {
            face,
            origin,
            du,
            dv,
            normal,
        }
    }

    pub fn embed(&self, u: f64, v: f64) -> P3 {
        add(self.origin, add(scale(self.du, u), scale(self.dv, v)))
    }

    /// Chart coordinates of a 3D point assumed to lie in this square's plane.
    pub fn chart(&self, p: P3) -> (f64, f64) {
        let w = sub(p, self.origin);
        (dot(w, self.du) / dot(self.du, self.du), dot(w, self.dv) / dot(self.dv, self.dv))
    }

    /// Chart components, in pixels, of a 3D vector lying in this square's plane.
    pub fn chart_dir(&self, d: P3) -> (f64, f64) {
        (dot(d, self.du) / dot(self.du, self.du), dot(d, self.dv) / dot(self.dv, self.dv))
    }

    /// A chart vector expressed in 3D (unit-cube units).
    pub fn embed_dir(&self, du: f64, dv: f64) -> P3 {
        add(scale(self.du, du), scale(self.dv, dv))
    }

    /// The four corners in chart order `(0,0), (64,0), (64,64), (0,64)`.
    pub fn corners(&self) -> [P3; 4] {
        [
            self.embed(0.0, 0.0),
            self.embed(64.0, 0.0),
            self.embed(64.0, 64.0),
            self.embed(0.0, 64.0),
        ]
    }

    /// The 3D endpoints of a chart edge (`Top`: v=0, `Right`: u=64, `Bottom`: v=64,
    /// `Left`: u=0), ordered by increasing along-edge parameter.
    pub fn edge_endpoints(&self, edge: Edge) -> (P3, P3) {
        match edge {
            Edge::Top => (self.embed(0.0, 0.0), self.embed(64.0, 0.0)),
            Edge::Right => (self.embed(64.0, 0.0), self.embed(64.0, 64.0)),
            Edge::Bottom => (self.embed(0.0, 64.0), self.embed(64.0, 64.0)),
            Edge::Left => (self.embed(0.0, 0.0), self.embed(0.0, 64.0)),
        }
    }
}

/// Corner coincidence tolerance in unit-cube units.
const CORNER_EPS: f64 = 1e-12;

/// Which face shares the given chart edge of `face`, discovered purely from 3D corner
/// coincidence (both endpoints within `1e-12`), or `None` for a rim edge. The returned
/// edge is the neighbor's chart edge with the same endpoints.
pub fn shared_edge(face: Face, edge: Edge) -> Option<(Face, Edge)> {
    let (a, b) = FaceSquare::of(face).edge_endpoints(edge);
    for other in Face::ALL {
        if other == face {
            continue;
        }
        let sq = FaceSquare::of(other);
        for e2 in Edge::ALL {
            let (c, d) = sq.edge_endpoints(e2);
            let same = dist(a, c) < CORNER_EPS && dist(b, d) < CORNER_EPS;
            let flipped = dist(a, d) < CORNER_EPS && dist(b, c) < CORNER_EPS;
            if same || flipped {
                return Some((other, e2));
            }
        }
    }
    None
}

// --- sweeping -------------------------------------------------------------------------

/// Result of a 3D sweep.
#[derive(Clone, Debug, PartialEq)]
pub struct Sweep {
    pub face: Face,
    pub u: f64,
    pub v: f64,
    /// Final tangent direction in 3D (unit-cube units), after all rotations/reflections.
    /// Normalized; the zero vector when the sweep had no displacement.
    pub tangent: P3,
    /// Faces visited in order, starting with the start face.
    pub faces: Vec<Face>,
    pub reflections: u32,
    /// Boundary hits where two chart edges were reached within `eps` of each other along
    /// the sweep and the lowest `Edge` index won. Not part of the original interface: the
    /// comparison tests need to know when the oracle itself resolved a vertex tie.
    pub ties: u32,
}

/// Sweep a displacement given in the start chart (`du`, `dv` pixels) using only 3D
/// rigid geometry: move in the square's plane; on reaching a square edge, either rotate
/// the remaining 3D displacement about the shared edge axis by the angle between the two
/// outward normals (Rodrigues) and continue in the neighbor square, or, at a rim edge,
/// reflect the remaining displacement's component perpendicular to the edge within the
/// plane. Vertex ties (two edges hit within `eps` along the sweep) resolve by the lowest
/// `Edge` index of the current chart, the same convention production uses, so results
/// are comparable there too. Panics after 256 boundary events (a hang is a test failure,
/// not a fallback).
pub fn sweep(face: Face, u: f64, v: f64, du: f64, dv: f64, eps: f64) -> Sweep {
    let mut sq = FaceSquare::of(face);
    let mut here = face;
    let mut p = sq.embed(u, v);
    let mut d = sq.embed_dir(du, dv);
    // The heading is carried separately so it survives the remaining displacement
    // reaching exactly zero on a boundary.
    let len = norm(d);
    let mut dir = if len > 0.0 { scale(d, 1.0 / len) } else { [0.0; 3] };
    let mut faces = vec![face];
    let mut reflections = 0u32;
    let mut ties = 0u32;

    for event in 0..=256 {
        assert!(event < 256, "oracle sweep did not terminate in 256 boundary events");
        let (cu, cv) = sq.chart(p);
        let (ddu, ddv) = sq.chart_dir(d);
        let step = (ddu * ddu + ddv * ddv).sqrt();
        if step == 0.0 {
            break;
        }
        // Earliest exit through an edge whose outward component is positive.
        let mut hit: Option<(f64, Edge)> = None;
        for e in Edge::ALL {
            let t = match e {
                Edge::Top if ddv < 0.0 => Some(-cv / ddv),
                Edge::Right if ddu > 0.0 => Some((64.0 - cu) / ddu),
                Edge::Bottom if ddv > 0.0 => Some((64.0 - cv) / ddv),
                Edge::Left if ddu < 0.0 => Some(-cu / ddu),
                _ => None,
            };
            let Some(t) = t else { continue };
            let t = t.max(0.0);
            if t > 1.0 {
                continue;
            }
            match hit {
                None => hit = Some((t, e)),
                Some((bt, _)) => {
                    if (t - bt).abs() * step <= eps {
                        // A vertex tie. `Edge::ALL` is in index order and `hit` already
                        // holds the lowest-index tied edge, so it stands.
                        ties += 1;
                    } else if t < bt {
                        hit = Some((t, e));
                    }
                }
            }
        }
        let Some((t, edge)) = hit else {
            p = add(p, d);
            break;
        };

        // Advance to the boundary and snap the crossed coordinate exactly onto it.
        p = add(p, scale(d, t));
        d = scale(d, 1.0 - t);
        let (mut hu, mut hv) = sq.chart(p);
        hu = hu.clamp(0.0, 64.0);
        hv = hv.clamp(0.0, 64.0);
        match edge {
            Edge::Top => hv = 0.0,
            Edge::Right => hu = 64.0,
            Edge::Bottom => hv = 64.0,
            Edge::Left => hu = 0.0,
        }
        p = sq.embed(hu, hv);

        let (ea, eb) = sq.edge_endpoints(edge);
        let axis = unit(sub(eb, ea));
        match shared_edge(here, edge) {
            Some((next, _)) => {
                let n1 = sq.normal;
                let next_sq = FaceSquare::of(next);
                let n2 = next_sq.normal;
                // The rotation about the shared edge that carries this square's outward
                // normal onto the neighbor's carries this square's tangent plane onto the
                // neighbor's, and the outgoing displacement into the neighbor's interior.
                let c = dot(n1, n2);
                let s = dot(cross(n1, n2), axis);
                d = rodrigues(d, axis, c, s);
                dir = rodrigues(dir, axis, c, s);
                here = next;
                sq = next_sq;
                faces.push(next);
            }
            None => {
                // Rim: reflect the in-plane component perpendicular to the edge.
                d = sub(scale(axis, 2.0 * dot(d, axis)), d);
                dir = sub(scale(axis, 2.0 * dot(dir, axis)), dir);
                reflections += 1;
            }
        }
    }

    let (fu, fv) = sq.chart(p);
    Sweep {
        face: here,
        u: fu.clamp(0.0, 64.0),
        v: fv.clamp(0.0, 64.0),
        tangent: dir,
        faces,
        reflections,
        ties,
    }
}

// --- geodesics ------------------------------------------------------------------------

/// A shortest-path answer: the distance in pixels and the face sequence of the unfolding
/// that achieved it (starting with `a`'s face).
#[derive(Clone, Debug, PartialEq)]
pub struct Geodesic {
    pub distance: f64,
    pub faces: Vec<Face>,
    /// The target's position expressed in the observer chart under the winning unfolding.
    pub local: (f64, f64),
}

/// Tolerance, in pixels, for accepting an edge crossing at a segment endpoint or exactly
/// at an edge endpoint.
const SEG_EPS: f64 = 1e-9;

/// Every simple face sequence of at most three faces starting at `from`, in order of
/// increasing length and then lexicographic face index.
fn face_sequences(from: Face) -> Vec<Vec<Face>> {
    let mut out = vec![vec![from]];
    let mut len1 = Vec::new();
    let mut len2 = Vec::new();
    for e in Edge::ALL {
        if let Some((f1, _)) = shared_edge(from, e) {
            len1.push(vec![from, f1]);
        }
    }
    len1.sort_by_key(|s| s[1].index());
    for s in &len1 {
        let mut next: Vec<Face> = Vec::new();
        for e in Edge::ALL {
            if let Some((f2, _)) = shared_edge(s[1], e)
                && f2 != from
                && f2 != s[1]
                && !next.contains(&f2)
            {
                next.push(f2);
            }
        }
        next.sort_by_key(|f| f.index());
        for f2 in next {
            len2.push(vec![from, s[1], f2]);
        }
    }
    out.extend(len1);
    out.extend(len2);
    out
}

/// Shortest surface distance between two points if ≤ `max` (≤ [`MAX_RADIUS`]), by
/// enumerating every simple face sequence of at most three faces from `a`'s face,
/// unfolding each subsequent square rigidly into the observer's plane by rotating about
/// the shared 3D edge (composing rotations along the sequence), measuring the straight
/// segment, and validating that the segment crosses each shared edge segment in order
/// and stays inside each unfolded square. The minimum over valid sequences is returned;
/// equal lengths (within `1e-9`) prefer the shorter sequence, then lexicographic face
/// indices.
pub fn geodesic(a: (Face, f64, f64), b: (Face, f64, f64), max: f64) -> Option<Geodesic> {
    assert!(
        max <= MAX_RADIUS + SEG_EPS,
        "oracle geodesic: max {max} exceeds MAX_RADIUS"
    );
    let sq0 = FaceSquare::of(a.0);
    let p0 = (a.1, a.2);
    let target3 = FaceSquare::of(b.0).embed(b.1, b.2);

    let mut best: Option<Geodesic> = None;
    for seq in face_sequences(a.0) {
        if *seq.last().expect("nonempty sequence") != b.0 {
            continue;
        }
        // Unfold: accumulate the rigid map that carries each square into the observer's
        // plane, and record each shared edge's image there.
        let mut map = Rigid::ID;
        let mut edge_images: Vec<((f64, f64), (f64, f64))> = Vec::new();
        let mut ok = true;
        for w in seq.windows(2) {
            let (from, to) = (w[0], w[1]);
            let sq_from = FaceSquare::of(from);
            let Some(edge) = Edge::ALL
                .into_iter()
                .find(|&e| shared_edge(from, e).map(|(f, _)| f) == Some(to))
            else {
                ok = false;
                break;
            };
            let (ea, eb) = sq_from.edge_endpoints(edge);
            let axis = unit(sub(eb, ea));
            let sq_to = FaceSquare::of(to);
            // Rotate the neighbor's plane onto this one about the shared edge.
            let c = dot(sq_to.normal, sq_from.normal);
            let s = dot(cross(sq_to.normal, sq_from.normal), axis);
            let rot = Rigid::about_axis(ea, axis, c, s);
            let a_img = map.apply(ea);
            let b_img = map.apply(eb);
            edge_images.push((sq0.chart(a_img), sq0.chart(b_img)));
            map = Rigid::compose(&map, &rot);
        }
        if !ok {
            continue;
        }
        let p1 = sq0.chart(map.apply(target3));
        let distance = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt();
        if distance > max + SEG_EPS {
            continue;
        }
        if !segment_crosses_in_order(p0, p1, &edge_images) {
            continue;
        }
        let better = match &best {
            None => true,
            Some(g) => distance < g.distance - SEG_EPS,
        };
        if better {
            best = Some(Geodesic {
                distance,
                faces: seq.clone(),
                local: p1,
            });
        }
    }
    best
}

/// The straight segment `p0 -> p1` must cross each listed edge image, in order, at
/// increasing parameters within `[0, 1]`, and within the edge segment itself.
fn segment_crosses_in_order(
    p0: (f64, f64),
    p1: (f64, f64),
    edges: &[ChartEdge],
) -> bool {
    let dx = p1.0 - p0.0;
    let dy = p1.1 - p0.1;
    let len = (dx * dx + dy * dy).sqrt();
    let mut last = -SEG_EPS;
    for &(e0, e1) in edges {
        let ex = e1.0 - e0.0;
        let ey = e1.1 - e0.1;
        // p0 + t·d = e0 + s·e
        let den = dx * (-ey) - dy * (-ex);
        if den.abs() < 1e-15 {
            return false;
        }
        let rx = e0.0 - p0.0;
        let ry = e0.1 - p0.1;
        let t = (rx * (-ey) - ry * (-ex)) / den;
        let s = (dx * ry - dy * rx) / den;
        let tol = if len > 0.0 { SEG_EPS / len } else { SEG_EPS };
        if !(t >= last - tol && t <= 1.0 + tol) {
            return false;
        }
        // The crossing must land on the shared edge, not on its extension.
        let elen = (ex * ex + ey * ey).sqrt();
        let stol = if elen > 0.0 { SEG_EPS / elen } else { SEG_EPS };
        if !(s >= -stol && s <= 1.0 + stol) {
            return false;
        }
        last = t;
    }
    true
}

/// Straight 3D chord in pixel units (lower bound on surface distance).
pub fn chord(a: (Face, f64, f64), b: (Face, f64, f64)) -> f64 {
    let pa = FaceSquare::of(a.0).embed(a.1, a.2);
    let pb = FaceSquare::of(b.0).embed(b.1, b.2);
    dist(pa, pb) * 32.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn squares_have_unit_cube_corners() {
        // Every reconstructed corner is a corner of [-1, 1]^3, exactly (the chart steps
        // are exact binary fractions, so no rounding creeps in).
        let mut seen: Vec<P3> = Vec::new();
        for face in Face::ALL {
            let sq = FaceSquare::of(face);
            for c in sq.corners() {
                for k in 0..3 {
                    assert!(c[k] == 1.0 || c[k] == -1.0, "{face:?} corner {c:?}");
                }
                if !seen.iter().any(|s| dist(*s, c) < CORNER_EPS) {
                    seen.push(c);
                }
            }
            // The frame is orthonormal at 1/32 scale with an outward normal.
            assert!(close(norm(sq.du), 1.0 / 32.0, 1e-15));
            assert!(close(norm(sq.dv), 1.0 / 32.0, 1e-15));
            assert!(close(dot(sq.du, sq.dv), 0.0, 1e-18));
            assert!(close(norm(sq.normal), 1.0, 1e-15));
            let center = sq.embed(32.0, 32.0);
            assert!(dot(sq.normal, center) > 0.0, "{face:?} normal points inward");
            // The embedding must reproduce pixel_direction at every pixel center.
            for &(x, y) in &[(0u8, 0u8), (63, 0), (0, 63), (63, 63), (17, 42)] {
                let want = pixel_direction(face, x, y);
                let got = sq.embed(f64::from(x) + 0.5, f64::from(y) + 0.5);
                assert!(dist(want, got) < 1e-15, "{face:?} pixel ({x},{y})");
            }
        }
        // Five squares touch 8 distinct cube corners.
        assert_eq!(seen.len(), 8, "distinct corners");
    }

    #[test]
    fn adjacency_has_eight_shared_and_four_rim_edges() {
        let mut shared = 0;
        let mut rim = Vec::new();
        for face in Face::ALL {
            for edge in Edge::ALL {
                match shared_edge(face, edge) {
                    Some(_) => shared += 1,
                    None => rim.push((face, edge)),
                }
            }
        }
        assert_eq!(shared, 16, "16 connected half-edges = 8 undirected seams");
        assert_eq!(rim.len(), 4, "four open rim edges");
        for (face, edge) in rim {
            assert_eq!(edge, Edge::Bottom, "{face:?} rim edge");
            assert_ne!(face, Face::Top, "Top has no rim edge");
        }
        // Hand-derived spot checks, read straight off the unit cube rather than off any
        // adjacency table: Front's top edge and Top's bottom edge are both the segment
        // (-1,1,1)-(1,1,1); Right's top edge and Top's right edge are both (1,1,1)-(1,1,-1).
        assert_eq!(shared_edge(Face::Front, Edge::Top), Some((Face::Top, Edge::Bottom)));
        assert_eq!(shared_edge(Face::Right, Edge::Top), Some((Face::Top, Edge::Right)));
        assert_eq!(shared_edge(Face::Back, Edge::Top), Some((Face::Top, Edge::Top)));
        assert_eq!(shared_edge(Face::Left, Edge::Top), Some((Face::Top, Edge::Left)));
        assert_eq!(shared_edge(Face::Front, Edge::Right), Some((Face::Right, Edge::Left)));
        assert_eq!(shared_edge(Face::Back, Edge::Right), Some((Face::Left, Edge::Left)));
    }

    #[test]
    fn shared_edge_is_an_involution() {
        for face in Face::ALL {
            for edge in Edge::ALL {
                if let Some((g, e2)) = shared_edge(face, edge) {
                    assert_ne!(g, face);
                    assert_eq!(
                        shared_edge(g, e2),
                        Some((face, edge)),
                        "{face:?}/{edge:?} -> {g:?}/{e2:?} is not an involution"
                    );
                    // The two chart edges are the same 3D segment.
                    let (a, b) = FaceSquare::of(face).edge_endpoints(edge);
                    let (c, d) = FaceSquare::of(g).edge_endpoints(e2);
                    let same = dist(a, c) < CORNER_EPS && dist(b, d) < CORNER_EPS;
                    let flipped = dist(a, d) < CORNER_EPS && dist(b, c) < CORNER_EPS;
                    assert!(same || flipped);
                }
            }
        }
    }

    #[test]
    fn sweep_across_each_shared_edge_preserves_3d_speed() {
        // From one pixel inside an edge, straight outward for two pixels: the sweep must
        // land two pixels of surface distance away, with a unit tangent lying in the
        // destination face's plane. Length preservation across the Rodrigues rotation is
        // exactly what would break if the seam rotation were not rigid.
        for face in Face::ALL {
            for edge in Edge::ALL {
                let Some(_) = shared_edge(face, edge) else { continue };
                for t in [0.5f64, 17.3, 63.5] {
                    let (u, v, du, dv) = match edge {
                        Edge::Top => (t, 1.0, 0.0, -2.0),
                        Edge::Right => (63.0, t, 2.0, 0.0),
                        Edge::Bottom => (t, 63.0, 0.0, 2.0),
                        Edge::Left => (1.0, t, -2.0, 0.0),
                    };
                    let s = sweep(face, u, v, du, dv, 1e-9);
                    assert_eq!(s.reflections, 0);
                    assert_eq!(s.faces.len(), 2, "{face:?}/{edge:?} at {t}");
                    assert_ne!(s.face, face);
                    assert!(close(norm(s.tangent), 1.0, 1e-12));
                    let sq = FaceSquare::of(s.face);
                    assert!(
                        close(dot(s.tangent, sq.normal), 0.0, 1e-12),
                        "tangent left the destination plane"
                    );
                    let g = geodesic((face, u, v), (s.face, s.u, s.v), 8.0)
                        .expect("a two-pixel step is within range");
                    assert!(
                        close(g.distance, 2.0, 1e-9),
                        "{face:?}/{edge:?} at {t}: swept 2px but the surface distance is {}",
                        g.distance
                    );
                }
            }
        }
    }

    #[test]
    fn sweep_reflects_at_the_rim() {
        for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
            let s = sweep(face, 20.0, 63.5, 0.0, 1.0, 1e-9);
            assert_eq!(s.face, face);
            assert_eq!(s.reflections, 1);
            assert_eq!(s.faces, vec![face]);
            assert!(close(s.u, 20.0, 1e-12) && close(s.v, 63.5, 1e-12), "{s:?}");
            // The vertical component reversed; the horizontal one did not. A unit 3D
            // vector is 32 pixels long in chart units.
            let sq = FaceSquare::of(face);
            let (tu, tv) = sq.chart_dir(s.tangent);
            assert!(close(tu / 32.0, 0.0, 1e-12) && close(tv / 32.0, -1.0, 1e-12), "{tu} {tv}");
        }
    }

    #[test]
    fn geodesic_on_one_face_is_the_euclidean_distance() {
        let cases: [((f64, f64), (f64, f64)); 3] = [
            ((10.0, 10.0), (20.0, 24.0)),
            ((0.5, 63.5), (3.5, 60.5)),
            ((32.0, 32.0), (32.0, 32.0)),
        ];
        for face in Face::ALL {
            for &((au, av), (bu, bv)) in &cases {
                let want = ((bu - au).powi(2) + (bv - av).powi(2)).sqrt();
                let g = geodesic((face, au, av), (face, bu, bv), 32.0).expect("in range");
                assert!(close(g.distance, want, 1e-12), "{face:?}: {g:?} want {want}");
                assert_eq!(g.faces, vec![face]);
                assert!(close(g.local.0, bu, 1e-12) && close(g.local.1, bv, 1e-12));
            }
        }
    }

    #[test]
    fn geodesic_across_a_flat_seam_is_the_unfolded_planar_distance() {
        // The four vertical seams unfold into a flat strip: Right (u, v) sits at
        // Front (64 + u, v). Hand-checked: Front (60, 20) to Right (4, 20) is
        // (64 + 4) - 60 = 8 pixels.
        let g = geodesic((Face::Front, 60.0, 20.0), (Face::Right, 4.0, 20.0), 32.0).unwrap();
        assert!(close(g.distance, 8.0, 1e-12), "{g:?}");
        assert_eq!(g.faces, vec![Face::Front, Face::Right]);
        assert!(close(g.local.0, 68.0, 1e-9) && close(g.local.1, 20.0, 1e-9), "{g:?}");

        let g = geodesic((Face::Front, 60.0, 10.0), (Face::Right, 4.0, 30.0), 32.0).unwrap();
        assert!(close(g.distance, (64.0f64 + 400.0).sqrt(), 1e-12), "{g:?}");

        // Front/Top is flat too: Top (u, v) sits at Front (u, v - 64).
        let g = geodesic((Face::Front, 20.0, 3.0), (Face::Top, 20.0, 60.0), 32.0).unwrap();
        assert!(close(g.distance, 7.0, 1e-12), "{g:?}");
        assert!(close(g.local.0, 20.0, 1e-9) && close(g.local.1, -4.0, 1e-9), "{g:?}");

        // Back/Left, hand-checked the same way: Left (u, v) sits at Back (64 + u, v).
        let g = geodesic((Face::Back, 61.5, 40.0), (Face::Left, 2.5, 40.0), 32.0).unwrap();
        assert!(close(g.distance, 5.0, 1e-12), "{g:?}");

        // Out of range is None, and the far side of the cube is unreachable locally.
        assert!(geodesic((Face::Front, 60.0, 32.0), (Face::Right, 36.0, 32.0), 32.0).is_none());
        assert!(geodesic((Face::Front, 32.0, 32.0), (Face::Back, 32.0, 32.0), 32.0).is_none());
    }

    #[test]
    fn geodesic_across_a_twisted_seam_matches_a_hand_unfolding() {
        // Right's top edge meets Top's right edge with the parameter reversed: Right
        // (u, v) unfolds into Top at (64 + v, 64 - u). Right (10, 2) therefore sits at
        // Top (66, 54), and Top (60, 50) is 2*sqrt(10) away.
        let g = geodesic((Face::Top, 60.0, 50.0), (Face::Right, 10.0, 2.0), 32.0).unwrap();
        let want = ((66.0f64 - 60.0).powi(2) + (54.0f64 - 50.0).powi(2)).sqrt();
        assert!(close(g.distance, want, 1e-9), "{g:?} want {want}");
        assert!(close(g.local.0, 66.0, 1e-9) && close(g.local.1, 54.0, 1e-9), "{g:?}");
    }

    #[test]
    fn chord_is_never_longer_than_the_geodesic() {
        let mut state = 0x243f_6a88_85a3_08d3u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut checked = 0;
        for _ in 0..4000 {
            let fa = Face::ALL[(next() * 5.0) as usize % 5];
            let fb = Face::ALL[(next() * 5.0) as usize % 5];
            let a = (fa, next() * 64.0, next() * 64.0);
            let b = (fb, next() * 64.0, next() * 64.0);
            let c = chord(a, b);
            if let Some(g) = geodesic(a, b, 32.0) {
                assert!(c <= g.distance + 1e-9, "chord {c} > geodesic {} for {a:?} {b:?}", g.distance);
                assert!(g.distance.is_finite() && g.distance >= 0.0);
                checked += 1;
            } else {
                // Unreachable within 32 px implies the chord is not a counterexample
                // either: a chord shorter than 32 px with no geodesic would mean the
                // enumeration missed a path.
                assert!(c > 32.0 - 1e-9 || fa != fb, "no geodesic for a near pair {a:?} {b:?}");
            }
        }
        assert!(checked > 100, "only {checked} pairs were in range");
    }

    #[test]
    fn geodesic_is_symmetric_and_never_exceeds_a_swept_path() {
        // Completeness check for the face-sequence enumeration: a sweep from a to b is a
        // path of its own swept length, so the geodesic must exist and be no longer. This
        // is what would fail if a reachable face sequence were missing.
        let mut state = 0x0123_4567_89ab_cdefu64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut checked = 0;
        for _ in 0..3000 {
            let f = Face::ALL[(next() * 5.0) as usize % 5];
            let (u, v) = (next() * 64.0, next() * 64.0);
            let len = 1.0 + next() * 20.0;
            let angle = next() * std::f64::consts::TAU;
            let s = sweep(f, u, v, len * angle.cos(), len * angle.sin(), 1e-9);
            let a = (f, u, v);
            let b = (s.face, s.u, s.v);
            let g = geodesic(a, b, 32.0)
                .unwrap_or_else(|| panic!("no geodesic for a swept pair {a:?} -> {b:?}"));
            assert!(
                g.distance <= len + 1e-9,
                "geodesic {} exceeds the swept length {len} ({a:?} -> {b:?})",
                g.distance
            );
            let back = geodesic(b, a, 32.0).expect("symmetric");
            assert!(
                (g.distance - back.distance).abs() <= 1e-9,
                "asymmetric: {} vs {}",
                g.distance,
                back.distance
            );
            assert!(chord(a, b) <= g.distance + 1e-9);
            checked += 1;
        }
        assert_eq!(checked, 3000);
    }

    #[test]
    fn sweep_and_geodesic_agree_on_multi_face_paths() {
        // A sweep that crosses a seam well away from any cube vertex is the shortest path
        // between its endpoints, so the geodesic must report exactly the swept length.
        // (Near a vertex this is false on purpose: with a 90 degree angular deficit a
        // straight sweep that subtends more than 180 degrees at the vertex is beaten by
        // the path around the other side, which is why every case here crosses mid-edge.)
        let cases: [(Face, f64, f64, f64, f64); 7] = [
            (Face::Front, 32.0, 4.0, 6.0, -20.0),
            (Face::Right, 30.0, 10.0, 0.0, -20.0),
            (Face::Back, 30.0, 5.0, 0.0, -20.0),
            (Face::Left, 30.0, 5.0, 0.0, -20.0),
            (Face::Top, 32.0, 5.0, 0.0, -20.0),
            (Face::Front, 60.0, 32.0, 20.0, 0.0),
            (Face::Top, 5.0, 32.0, -20.0, 0.0),
        ];
        for &(f, u, v, du, dv) in &cases {
            let s = sweep(f, u, v, du, dv, 1e-9);
            let len = (du * du + dv * dv).sqrt();
            assert_eq!(s.reflections, 0);
            assert_eq!(s.faces.len(), 2, "{f:?} ({u},{v}) + ({du},{dv}): {s:?}");
            let g = geodesic((f, u, v), (s.face, s.u, s.v), 32.0).expect("within range");
            assert!(
                close(g.distance, len, 1e-9),
                "swept {len} from {f:?} ({u},{v}) but geodesic says {} ({s:?})",
                g.distance
            );
        }
    }
}
