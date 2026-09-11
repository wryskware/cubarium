//! Cube-surface geometry: seam adjacency between face images, and a GL cube-map adapter.
//!
//! Everything here is derived from the face convention in `docs/ARCHITECTURE.md`:
//! five 64×64 images, `x` right and `y` down, **as seen from outside** the upright cube.
//!
//! # The net
//!
//! Unrolling the cube gives this net. The four side faces form a strip (Back's right edge
//! wraps around to Left's left edge), and Top is folded up out of the strip above Front:
//!
//! ```text
//!                              Back.top  (REVERSED: x -> 63-x)
//!                            +--------------------+
//!                            |  y=0 row is Back   |
//!             Left.top ------|                    |------ Right.top
//!               (x -> y)     |      TOP (4)       |   (REVERSED: x -> 63-y)
//!                            |   x ->    y v      |
//!                            |  y=63 row is Front |
//!                            +--------------------+
//!                                Front.top  (x -> x)
//!
//!   +-----------+-----------+-----------+-----------+
//!   |  LEFT (3) | FRONT (0) | RIGHT (1) |  BACK (2) |   <- and Back.right wraps to Left.left
//!   |   (-X)    |   (+Z)    |   (+X)    |   (-Z)    |
//!   +-----------+-----------+-----------+-----------+
//!         (the four bottom edges are open: there is no bottom face)
//! ```
//!
//! The side strip is easy: all four side faces share the same "up", so the four vertical
//! seams join left-column to right-column at equal `y`, with no twist at all.
//!
//! The four seams around Top are where the care is needed. Top is viewed from above with
//! **Back at the top of the image**, so on Top the image axes are `x` → +X (toward Right)
//! and `y` → +Z (toward Front). Walk each seam:
//!
//! * **Front.top ↔ Top.bottom** — both edges are horizontal and both run in +X as their
//!   `x` grows, so `x` maps straight through. No twist, not reversed.
//! * **Back.top ↔ Top.top** — Back is seen from outside at −Z, so Back's image `x` runs in
//!   **−X**; Top's `x` runs in **+X**. Same physical edge, opposite parameterisations:
//!   `x -> 63-x`. **Reversed**, and the heading turns a half turn.
//! * **Right.top ↔ Top.right** — Right is seen from outside at +X, so Right's image `x`
//!   runs in **−Z**; Top's right edge is parameterised by `y`, which runs in **+Z**.
//!   Opposite again: `x -> 63-y`. **Reversed**, and a horizontal edge meets a vertical one,
//!   so the heading turns a quarter turn ("twisted" seam).
//! * **Left.top ↔ Top.left** — Left is seen from outside at −X, so Left's image `x` runs in
//!   **+Z**, and Top's left edge parameter `y` also runs in **+Z**: `x -> y`, *not*
//!   reversed — but a horizontal edge still meets a vertical one, so this is the other
//!   quarter-turn ("twisted") seam.
//!
//! So the two seams that *turn* (Right↔Top, Left↔Top) and the two seams that *reverse*
//! (Right↔Top, Back↔Top) are different pairs, overlapping only at Right↔Top.
//!
//! # Axes
//!
//! Right = **+X**, Top = **+Y**, Front = **+Z** (a viewer standing in front of the cube
//! looks along −Z, the usual GL camera), Back = −Z, Left = −X; there is no −Y face. The
//! cube spans `[-1, 1]³`, so pixel `(x, y)` of a face covers a 1/32 × 1/32 patch whose
//! centre is at `pixel_direction(face, x, y)`.

use crate::{Face, Frame, FACE_BYTES, FACE_SIZE, NUM_FACES};
use std::sync::OnceLock;

/// Largest valid pixel coordinate / along-edge coordinate (63).
pub const FACE_MAX: u8 = (FACE_SIZE - 1) as u8;

/// One edge of a face *image*, named in image terms (`Top` is the `y = 0` row, `Right` the
/// `x = 63` column, `Bottom` the `y = 63` row, `Left` the `x = 0` column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[repr(u8)]
pub enum Edge {
    Top = 0,
    Right = 1,
    Bottom = 2,
    Left = 3,
}

impl Edge {
    pub const ALL: [Edge; 4] = [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left];

    /// The pixel on this edge at along-edge coordinate `t`.
    ///
    /// `t` is measured **left → right** for `Top`/`Bottom` and **top → bottom** for
    /// `Left`/`Right`. Panics if `t >= 64`.
    #[inline]
    pub fn pixel(self, t: u8) -> (u8, u8) {
        assert!(t < FACE_SIZE as u8, "cube-proto: edge coordinate {t} >= 64");
        match self {
            Edge::Top => (t, 0),
            Edge::Right => (FACE_MAX, t),
            Edge::Bottom => (t, FACE_MAX),
            Edge::Left => (0, t),
        }
    }

    /// The along-edge coordinate of a pixel that lies on this edge.
    ///
    /// Panics if `(x, y)` is not on the edge.
    #[inline]
    pub fn coord_of(self, x: u8, y: u8) -> u8 {
        let (t, ok) = match self {
            Edge::Top => (x, y == 0),
            Edge::Right => (y, x == FACE_MAX),
            Edge::Bottom => (x, y == FACE_MAX),
            Edge::Left => (y, x == 0),
        };
        assert!(
            ok && t < FACE_SIZE as u8,
            "cube-proto: pixel ({x}, {y}) is not on the {self:?} edge"
        );
        t
    }

    /// Unit vector, in image coordinates, pointing **out of** the face across this edge.
    #[inline]
    pub fn outward(self) -> (i32, i32) {
        match self {
            Edge::Top => (0, -1),
            Edge::Right => (1, 0),
            Edge::Bottom => (0, 1),
            Edge::Left => (-1, 0),
        }
    }

    /// Index of `outward()` in counter-clockwise order starting at +x: right, up, left, down.
    #[inline]
    fn outward_turn(self) -> u8 {
        match self {
            Edge::Right => 0,
            Edge::Top => 1,
            Edge::Left => 2,
            Edge::Bottom => 3,
        }
    }
}

/// The far side of a seam: which face and edge it is, and whether the shared edge's
/// along-edge parameter runs the opposite way there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Seam {
    pub face: Face,
    pub edge: Edge,
    pub reversed: bool,
}

impl Face {
    /// The face across `edge`, which of its edges is shared, and whether the shared edge's
    /// parameter runs the opposite way. Returns `None` for the four open bottom edges.
    ///
    /// The relation is an involution (`a.neighbor(e)` names the seam that points back at
    /// `(a, e)`) and `reversed` is symmetric. See the module docs for the derivation; the
    /// full table is:
    ///
    /// ```text
    ///   Front.top    -> Top.bottom          Top.top    -> Back.top   (reversed)
    ///   Front.right  -> Right.left          Top.right  -> Right.top  (reversed)
    ///   Front.bottom -> open                Top.bottom -> Front.top
    ///   Front.left   -> Left.right          Top.left   -> Left.top
    ///   Right.top    -> Top.right (rev)
    ///   Right.right  -> Back.left           Back.top   -> Top.top    (reversed)
    ///   Right.bottom -> open                Back.right -> Left.left
    ///   Right.left   -> Front.right         Back.bottom-> open
    ///                                       Back.left  -> Right.right
    ///   Left.top     -> Top.left
    ///   Left.right   -> Front.left
    ///   Left.bottom  -> open
    ///   Left.left    -> Back.right
    /// ```
    pub fn neighbor(self, edge: Edge) -> Option<Seam> {
        let (face, edge, reversed) = match (self, edge) {
            // --- the four vertical seams: same "up" on both sides, never reversed ---
            (Face::Front, Edge::Right) => (Face::Right, Edge::Left, false),
            (Face::Right, Edge::Left) => (Face::Front, Edge::Right, false),
            (Face::Right, Edge::Right) => (Face::Back, Edge::Left, false),
            (Face::Back, Edge::Left) => (Face::Right, Edge::Right, false),
            (Face::Back, Edge::Right) => (Face::Left, Edge::Left, false),
            (Face::Left, Edge::Left) => (Face::Back, Edge::Right, false),
            (Face::Left, Edge::Right) => (Face::Front, Edge::Left, false),
            (Face::Front, Edge::Left) => (Face::Left, Edge::Right, false),

            // --- the four seams around Top ---
            // Front's x runs +X, Top's x runs +X: straight through.
            (Face::Front, Edge::Top) => (Face::Top, Edge::Bottom, false),
            (Face::Top, Edge::Bottom) => (Face::Front, Edge::Top, false),
            // Right's x runs -Z, Top's right-edge y runs +Z: reversed, quarter turn.
            (Face::Right, Edge::Top) => (Face::Top, Edge::Right, true),
            (Face::Top, Edge::Right) => (Face::Right, Edge::Top, true),
            // Back's x runs -X, Top's x runs +X: reversed, half turn.
            (Face::Back, Edge::Top) => (Face::Top, Edge::Top, true),
            (Face::Top, Edge::Top) => (Face::Back, Edge::Top, true),
            // Left's x runs +Z, Top's left-edge y runs +Z: same sense, quarter turn.
            (Face::Left, Edge::Top) => (Face::Top, Edge::Left, false),
            (Face::Top, Edge::Left) => (Face::Left, Edge::Top, false),

            // --- the open bottom of the cube: no -Y face ---
            (Face::Front | Face::Right | Face::Back | Face::Left, Edge::Bottom) => {
                return None;
            }
        };
        Some(Seam {
            face,
            edge,
            reversed,
        })
    }
}

/// Rotate a heading `quarter_turns` counter-clockwise **as the image appears on screen**
/// (`x` right, `y` down), i.e. one turn maps `(dx, dy)` to `(dy, -dx)`: right becomes up.
#[inline]
pub fn rotate_heading(quarter_turns: u8, dx: i32, dy: i32) -> (i32, i32) {
    match quarter_turns % 4 {
        0 => (dx, dy),
        1 => (dy, -dx),
        2 => (-dx, -dy),
        _ => (-dy, dx),
    }
}

/// Move a point that leaves `face` across `edge` at along-edge coordinate `t` (`0..64`,
/// measured left→right for `Top`/`Bottom` edges and top→bottom for `Left`/`Right` edges)
/// onto the neighbouring face.
///
/// Returns `(face, x, y, quarter_turns)`: the entry pixel, and how far a velocity vector
/// must be rotated counter-clockwise in image coordinates (see [`rotate_heading`]) to keep
/// going straight across the seam. `None` for the four open bottom edges. Panics if
/// `t >= 64`.
///
/// The rotation is fixed by the edges alone: leaving across `edge` the heading points along
/// `edge.outward()`, and entering across the neighbour's edge `e2` it must point along
/// `-e2.outward()`, so the turn is `(e2.outward_turn() + 2 - edge.outward_turn()) mod 4`.
/// That the same rotation also carries the along-edge tangent correctly (matching
/// `reversed`) is a fact about the cube, and is asserted in `tests/geometry.rs`.
pub fn cross_seam(face: Face, edge: Edge, t: u8) -> Option<(Face, u8, u8, u8)> {
    assert!(t < FACE_SIZE as u8, "cube-proto: edge coordinate {t} >= 64");
    let seam = face.neighbor(edge)?;
    let t2 = if seam.reversed { FACE_MAX - t } else { t };
    let (x, y) = seam.edge.pixel(t2);
    let turns = (seam.edge.outward_turn() + 4 + 2 - edge.outward_turn()) % 4;
    Some((seam.face, x, y, turns))
}

/// The centre of face pixel `(x, y)` on the cube `[-1, 1]³`, which doubles as the outward
/// direction of that pixel (unnormalised).
///
/// With `a = (2x+1)/64 - 1` and `b = (2y+1)/64 - 1` (both in `(-1, 1)`, increasing with the
/// pixel coordinate), the five faces are:
///
/// ```text
///   Front (+Z): ( a, -b,  1)     Right (+X): ( 1, -b, -a)
///   Back  (-Z): (-a, -b, -1)     Left  (-X): (-1, -b,  a)
///   Top   (+Y): ( a,  1,  b)
/// ```
///
/// `-b` because image `y` runs **down** while +Y is up; Right's `-a` because, seen from
/// outside at +X, image `x` runs toward −Z; Top's `+b` because, with Back at the top of the
/// image, image `y` runs toward +Z (Front).
pub fn pixel_direction(face: Face, x: u8, y: u8) -> [f64; 3] {
    assert!(
        x < FACE_SIZE as u8 && y < FACE_SIZE as u8,
        "cube-proto: pixel ({x}, {y}) out of range"
    );
    let a = (2.0 * f64::from(x) + 1.0) / FACE_SIZE as f64 - 1.0;
    let b = (2.0 * f64::from(y) + 1.0) / FACE_SIZE as f64 - 1.0;
    match face {
        Face::Front => [a, -b, 1.0],
        Face::Right => [1.0, -b, -a],
        Face::Back => [-a, -b, -1.0],
        Face::Left => [-1.0, -b, a],
        Face::Top => [a, 1.0, b],
    }
}

/// Six 64×64 RGB8 images in OpenGL cube-map face order (+X, −X, +Y, −Y, +Z, −Z), each
/// row-major with row 0 = texture coordinate `t = 0` exactly as the GL spec's cube-map
/// selection table defines (what you get by rendering the six standard views into a
/// cube-map render target; see `docs/GEOMETRY.md`). Each slice must be [`FACE_BYTES`] long.
#[derive(Debug, Clone, Copy)]
pub struct CubemapFaces<'a> {
    pub faces: [&'a [u8]; 6],
}

/// Index of the +X face in [`CubemapFaces::faces`].
pub const GL_POS_X: usize = 0;
/// Index of the −X face in [`CubemapFaces::faces`].
pub const GL_NEG_X: usize = 1;
/// Index of the +Y face in [`CubemapFaces::faces`].
pub const GL_POS_Y: usize = 2;
/// Index of the −Y face in [`CubemapFaces::faces`].
pub const GL_NEG_Y: usize = 3;
/// Index of the +Z face in [`CubemapFaces::faces`].
pub const GL_POS_Z: usize = 4;
/// Index of the −Z face in [`CubemapFaces::faces`].
pub const GL_NEG_Z: usize = 5;

impl CubemapFaces<'_> {
    fn validate(&self) {
        for (i, f) in self.faces.iter().enumerate() {
            assert_eq!(
                f.len(),
                FACE_BYTES,
                "cube-proto: cubemap face {i} is {} bytes, expected {FACE_BYTES}",
                f.len()
            );
        }
    }
}

/// The GL cube-map selection table (OpenGL spec, "Cube Map Texture Selection"):
///
/// ```text
///   major axis   face   sc    tc    ma
///     +rx        +X    -rz   -ry    rx
///     -rx        -X    +rz   -ry    rx
///     +ry        +Y    +rx   +rz    ry
///     -ry        -Y    +rx   -rz    ry
///     +rz        +Z    +rx   -ry    rz
///     -rz        -Z    -rx   -ry    rz
/// ```
///
/// Returns `(face index, sc, tc, |ma|)`.
fn gl_select(d: [f64; 3]) -> (usize, f64, f64, f64) {
    let [rx, ry, rz] = d;
    let (ax, ay, az) = (rx.abs(), ry.abs(), rz.abs());
    if ax >= ay && ax >= az {
        if rx > 0.0 {
            (GL_POS_X, -rz, -ry, ax)
        } else {
            (GL_NEG_X, rz, -ry, ax)
        }
    } else if ay >= az {
        if ry > 0.0 {
            (GL_POS_Y, rx, rz, ay)
        } else {
            (GL_NEG_Y, rx, -rz, ay)
        }
    } else if rz > 0.0 {
        (GL_POS_Z, rx, -ry, az)
    } else {
        (GL_NEG_Z, -rx, -ry, az)
    }
}

#[inline]
fn texel_index(u: f64) -> u8 {
    let i = (u * FACE_SIZE as f64).floor();
    i.clamp(0.0, f64::from(FACE_MAX)) as u8
}

/// Which cube-map texel our face pixel `(x, y)` reads: `(gl face index, s_i, t_i)`.
///
/// The direction of the pixel centre goes through the GL selection table, then
/// `s = (sc/|ma| + 1)/2`, `t = (tc/|ma| + 1)/2` and nearest-texel `floor(s * 64)`.
///
/// Because a GL cube-map face image is laid out with `s` increasing to the right and `t`
/// increasing down *and* the resulting image is the face as seen from **outside** the cube
/// (the well-known "cube maps are stored left-handed" quirk), this comes out as the
/// identity: our Front/Right/Back/Left/Top are exactly GL +Z/+X/−Z/−X/+Y with `s_i = x` and
/// `t_i = y`, and the −Y face is never read. That is a *derived* result, not an assumption —
/// this function goes through the table, and `tests/geometry.rs` checks the corners,
/// centres and every one of the 20480 pixels against hand-worked arithmetic.
pub fn cubemap_source(face: Face, x: u8, y: u8) -> (usize, u8, u8) {
    let (gl, sc, tc, ma) = gl_select(pixel_direction(face, x, y));
    (
        gl,
        texel_index((sc / ma + 1.0) * 0.5),
        texel_index((tc / ma + 1.0) * 0.5),
    )
}

const LUT_LEN: usize = NUM_FACES * FACE_SIZE * FACE_SIZE; // 20480

/// `(gl face) << 12 | (t_i * 64 + s_i)` for every frame pixel, in frame byte order.
fn cubemap_lut() -> &'static [u32; LUT_LEN] {
    static LUT: OnceLock<Box<[u32; LUT_LEN]>> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut v = vec![0u32; LUT_LEN].into_boxed_slice();
        let mut i = 0;
        for face in Face::ALL {
            for y in 0..FACE_SIZE as u8 {
                for x in 0..FACE_SIZE as u8 {
                    let (gl, s, t) = cubemap_source(face, x, y);
                    v[i] = ((gl as u32) << 12) | (u32::from(t) * FACE_SIZE as u32 + u32::from(s));
                    i += 1;
                }
            }
        }
        v.try_into().expect("LUT_LEN entries")
    })
}

impl Frame {
    /// Fill this frame from a cubemap: for each face pixel, take the outward direction of
    /// the pixel centre, select the cube-map face and `(s, t)` per the GL major-axis table,
    /// and copy the nearest texel. Panics unless every face slice is [`FACE_BYTES`] long.
    pub fn fill_from_cubemap(&mut self, cm: &CubemapFaces) {
        cm.validate();
        let lut = cubemap_lut();
        for (px, &e) in self
            .as_bytes_mut()
            .as_chunks_mut::<3>()
            .0
            .iter_mut()
            .zip(lut)
        {
            let src = cm.faces[(e >> 12) as usize];
            let o = (e & 0xfff) as usize * 3;
            *px = [src[o], src[o + 1], src[o + 2]];
        }
    }

    /// [`Frame::fill_from_cubemap`] into a fresh frame.
    pub fn from_cubemap(cm: &CubemapFaces) -> Frame {
        let mut f = Frame::black();
        f.fill_from_cubemap(cm);
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_pixels_round_trip() {
        for e in Edge::ALL {
            for t in 0..64u8 {
                let (x, y) = e.pixel(t);
                assert_eq!(e.coord_of(x, y), t);
            }
        }
        assert_eq!(Edge::Top.pixel(5), (5, 0));
        assert_eq!(Edge::Right.pixel(5), (63, 5));
        assert_eq!(Edge::Bottom.pixel(5), (5, 63));
        assert_eq!(Edge::Left.pixel(5), (0, 5));
    }

    #[test]
    fn heading_rotation_is_ccw_on_screen() {
        // right -> up -> left -> down, as the image looks on screen (y is down).
        assert_eq!(rotate_heading(1, 1, 0), (0, -1));
        assert_eq!(rotate_heading(2, 1, 0), (-1, 0));
        assert_eq!(rotate_heading(3, 1, 0), (0, 1));
        assert_eq!(rotate_heading(4, 1, 0), (1, 0));
    }

    #[test]
    fn lut_is_built_once_and_matches_cubemap_source() {
        let lut = cubemap_lut();
        assert_eq!(lut.len(), 20480);
        // Spot-check the first and last entries against cubemap_source.
        let (gl, s, t) = cubemap_source(Face::Front, 0, 0);
        assert_eq!(
            lut[0],
            ((gl as u32) << 12) | (u32::from(t) * 64 + u32::from(s))
        );
        let (gl, s, t) = cubemap_source(Face::Top, 63, 63);
        assert_eq!(
            lut[20479],
            ((gl as u32) << 12) | (u32::from(t) * 64 + u32::from(s))
        );
    }
}
