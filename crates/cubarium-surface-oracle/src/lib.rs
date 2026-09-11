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

pub use cube_proto::{Edge, Face};

/// Largest query radius the oracle enumerates paths for, in pixels (chart paths of at
/// most two seams are complete below one face width; the same bound as production).
pub const MAX_RADIUS: f64 = 32.0;

pub type P3 = [f64; 3];

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
        let _ = face;
        todo!("FaceSquare::of")
    }

    pub fn embed(&self, u: f64, v: f64) -> P3 {
        let _ = (u, v);
        todo!("FaceSquare::embed")
    }

    /// Chart coordinates of a 3D point assumed to lie in this square's plane.
    pub fn chart(&self, p: P3) -> (f64, f64) {
        let _ = p;
        todo!("FaceSquare::chart")
    }

    /// The four corners in chart order `(0,0), (64,0), (64,64), (0,64)`.
    pub fn corners(&self) -> [P3; 4] {
        todo!("FaceSquare::corners")
    }

    /// The 3D endpoints of a chart edge (`Top`: v=0, `Right`: u=64, `Bottom`: v=64,
    /// `Left`: u=0), ordered by increasing along-edge parameter.
    pub fn edge_endpoints(&self, edge: Edge) -> (P3, P3) {
        let _ = edge;
        todo!("FaceSquare::edge_endpoints")
    }
}

/// Which face shares the given chart edge of `face`, discovered purely from 3D corner
/// coincidence (both endpoints within `1e-12`), or `None` for a rim edge. The returned
/// edge is the neighbor's chart edge with the same endpoints.
pub fn shared_edge(face: Face, edge: Edge) -> Option<(Face, Edge)> {
    let _ = (face, edge);
    todo!("shared_edge")
}

/// Result of a 3D sweep.
#[derive(Clone, Debug, PartialEq)]
pub struct Sweep {
    pub face: Face,
    pub u: f64,
    pub v: f64,
    /// Final tangent direction in 3D (unit-cube units), after all rotations/reflections.
    pub tangent: P3,
    /// Faces visited in order, starting with the start face.
    pub faces: Vec<Face>,
    pub reflections: u32,
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
    let _ = (face, u, v, du, dv, eps);
    todo!("sweep")
}

/// A shortest-path answer: the distance in pixels and the face sequence of the unfolding
/// that achieved it (starting with `a`'s face).
#[derive(Clone, Debug, PartialEq)]
pub struct Geodesic {
    pub distance: f64,
    pub faces: Vec<Face>,
    /// The target's position expressed in the observer chart under the winning unfolding.
    pub local: (f64, f64),
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
    let _ = (a, b, max);
    todo!("geodesic")
}

/// Straight 3D chord in pixel units (lower bound on surface distance).
pub fn chord(a: (Face, f64, f64), b: (Face, f64, f64)) -> f64 {
    let _ = (a, b);
    todo!("chord")
}
