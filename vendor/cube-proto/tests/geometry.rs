//! Hand-derived checks of the cube-surface geometry helpers.
//!
//! Every expectation in this file is worked out from the face convention on paper (the
//! arithmetic is in the comments) — never by calling the code under test.
//!
//! The convention, restated: five 64×64 images, `x` right, `y` down, seen from **outside**
//! the upright cube; Front=+Z, Right=+X, Back=−Z, Left=−X, Top=+Y over `[-1, 1]³`; Top is
//! seen from above with Back at the top of the image. Writing `a = (2x+1)/64 - 1` and
//! `b = (2y+1)/64 - 1`, a pixel centre sits at
//!
//! ```text
//!   Front ( a, -b,  1)   Right ( 1, -b, -a)   Back (-a, -b, -1)
//!   Left  (-1, -b,  a)   Top   ( a,  1,  b)
//! ```

use cube_proto::geometry::{
    cross_seam, rotate_heading, CubemapFaces, Edge, Seam, GL_NEG_X, GL_NEG_Y, GL_NEG_Z, GL_POS_X,
    GL_POS_Y, GL_POS_Z,
};
use cube_proto::{Face, Frame, FACE_BYTES};

const N: u8 = 64;
const MAX: u8 = 63;

fn seam(face: Face, edge: Edge, reversed: bool) -> Option<Seam> {
    Some(Seam {
        face,
        edge,
        reversed,
    })
}

// ---------------------------------------------------------------------------------------
// Deliverable 1: seam adjacency
// ---------------------------------------------------------------------------------------

/// All twenty half-edges, written out. Derivations:
///
/// *Vertical seams.* Walking Front → Right → Back → Left is counter-clockwise seen from
/// above, and all four side faces share the same up direction, so each face's right column
/// abuts the next face's left column at equal `y`: never reversed.
///
/// *Top seams.* On Top, image `x` runs toward +X (Right) and image `y` runs toward +Z
/// (Front), because Top is viewed from above with Back at the top of the image.
/// - Front's top edge is the +Z/+Y cube edge; Front's `x` runs +X and Top's `x` runs +X,
///   so `x -> x`: not reversed.
/// - Right's top edge is the +X/+Y cube edge; Right's `x` runs −Z (seen from outside at +X)
///   while Top's right-edge `y` runs +Z, so `x -> 63-y`: reversed.
/// - Back's top edge is the −Z/+Y cube edge; Back's `x` runs −X while Top's `x` runs +X, so
///   `x -> 63-x`: reversed.
/// - Left's top edge is the −X/+Y cube edge; Left's `x` runs +Z (seen from outside at −X)
///   and Top's left-edge `y` runs +Z, so `x -> y`: not reversed.
#[test]
fn neighbor_table_all_twenty_half_edges() {
    // Front
    assert_eq!(
        Face::Front.neighbor(Edge::Top),
        seam(Face::Top, Edge::Bottom, false)
    );
    assert_eq!(
        Face::Front.neighbor(Edge::Right),
        seam(Face::Right, Edge::Left, false)
    );
    assert_eq!(Face::Front.neighbor(Edge::Bottom), None);
    assert_eq!(
        Face::Front.neighbor(Edge::Left),
        seam(Face::Left, Edge::Right, false)
    );

    // Right
    assert_eq!(
        Face::Right.neighbor(Edge::Top),
        seam(Face::Top, Edge::Right, true)
    );
    assert_eq!(
        Face::Right.neighbor(Edge::Right),
        seam(Face::Back, Edge::Left, false)
    );
    assert_eq!(Face::Right.neighbor(Edge::Bottom), None);
    assert_eq!(
        Face::Right.neighbor(Edge::Left),
        seam(Face::Front, Edge::Right, false)
    );

    // Back
    assert_eq!(
        Face::Back.neighbor(Edge::Top),
        seam(Face::Top, Edge::Top, true)
    );
    assert_eq!(
        Face::Back.neighbor(Edge::Right),
        seam(Face::Left, Edge::Left, false)
    );
    assert_eq!(Face::Back.neighbor(Edge::Bottom), None);
    assert_eq!(
        Face::Back.neighbor(Edge::Left),
        seam(Face::Right, Edge::Right, false)
    );

    // Left
    assert_eq!(
        Face::Left.neighbor(Edge::Top),
        seam(Face::Top, Edge::Left, false)
    );
    assert_eq!(
        Face::Left.neighbor(Edge::Right),
        seam(Face::Front, Edge::Left, false)
    );
    assert_eq!(Face::Left.neighbor(Edge::Bottom), None);
    assert_eq!(
        Face::Left.neighbor(Edge::Left),
        seam(Face::Back, Edge::Right, false)
    );

    // Top (no open edge: all four are closed)
    assert_eq!(
        Face::Top.neighbor(Edge::Top),
        seam(Face::Back, Edge::Top, true)
    );
    assert_eq!(
        Face::Top.neighbor(Edge::Right),
        seam(Face::Right, Edge::Top, true)
    );
    assert_eq!(
        Face::Top.neighbor(Edge::Bottom),
        seam(Face::Front, Edge::Top, false)
    );
    assert_eq!(
        Face::Top.neighbor(Edge::Left),
        seam(Face::Left, Edge::Top, false)
    );
}

#[test]
fn only_the_four_side_bottoms_are_open() {
    let mut open = 0;
    let mut closed = 0;
    for f in Face::ALL {
        for e in Edge::ALL {
            match f.neighbor(e) {
                None => {
                    assert_eq!(e, Edge::Bottom, "{f:?} {e:?} should be closed");
                    assert_ne!(f, Face::Top, "Top has no open edge");
                    open += 1;
                }
                Some(_) => closed += 1,
            }
        }
    }
    assert_eq!(open, 4);
    assert_eq!(closed, 16);
}

#[test]
fn neighbor_is_an_involution_with_symmetric_reversed() {
    for f in Face::ALL {
        for e in Edge::ALL {
            let Some(s) = f.neighbor(e) else { continue };
            let back = s
                .face
                .neighbor(s.edge)
                .expect("the far side of a closed seam is closed");
            assert_eq!(back.face, f, "{f:?}.{e:?} -> {s:?} does not point back");
            assert_eq!(back.edge, e, "{f:?}.{e:?} -> {s:?} does not point back");
            assert_eq!(
                back.reversed, s.reversed,
                "reversed must be symmetric at {f:?}.{e:?}"
            );
        }
    }
}

/// Exactly two seams reverse their along-edge parameter: Right↔Top and Back↔Top.
#[test]
fn exactly_two_seams_are_reversed() {
    let reversed: Vec<(Face, Edge)> = Face::ALL
        .into_iter()
        .flat_map(|f| Edge::ALL.map(move |e| (f, e)))
        .filter(|&(f, e)| f.neighbor(e).is_some_and(|s| s.reversed))
        .collect();
    assert_eq!(
        reversed,
        vec![
            (Face::Right, Edge::Top), // -> Top.right
            (Face::Back, Edge::Top),  // -> Top.top
            (Face::Top, Edge::Top),   // -> Back.top
            (Face::Top, Edge::Right), // -> Right.top
        ]
    );
}

#[test]
fn cross_seam_explicit_cases() {
    for t in 0..N {
        // Front's right column (x=63) is the +X/+Z cube edge at height y=t; Right's left
        // column (x=0) is the same edge at the same height, so y is carried straight over
        // and the heading (pointing +x) is unchanged.
        assert_eq!(
            cross_seam(Face::Front, Edge::Right, t),
            Some((Face::Right, 0, t, 0))
        );

        // Front's top row is the +Y/+Z cube edge; Top's bottom row is the same edge with x
        // running the same way (both toward +X). Straight through, no rotation.
        assert_eq!(
            cross_seam(Face::Front, Edge::Top, t),
            Some((Face::Top, t, MAX, 0))
        );

        // Right's top row is the +X/+Y cube edge. On Right (seen from outside at +X) image
        // x runs toward -Z: pixel (t,0) sits at z = 1 - (2t+1)/64. On Top the right column
        // (x=63) is the same edge with y running toward +Z: pixel (63,y) sits at
        // z = (2y+1)/64 - 1. Equal z gives (2t+1) + (2y+1) = 128, i.e. y = 63 - t.
        // A heading of (0,-1) (leaving upward) must become (-1,0) (entering leftward from
        // the right column): one quarter turn counter-clockwise on screen.
        assert_eq!(
            cross_seam(Face::Right, Edge::Top, t),
            Some((Face::Top, MAX, MAX - t, 1))
        );

        // Back's top row is the -Z/+Y cube edge. Back's image x runs toward -X, Top's x
        // runs toward +X, so x -> 63 - t, and (0,-1) must become (0,1): a half turn.
        assert_eq!(
            cross_seam(Face::Back, Edge::Top, t),
            Some((Face::Top, MAX - t, 0, 2))
        );

        // Left's top row is the -X/+Y cube edge. Left's image x runs toward +Z and Top's
        // left column runs (in y) toward +Z as well, so y = t. Heading (0,-1) must become
        // (1,0) (entering rightward from the left column): three quarter turns CCW.
        assert_eq!(
            cross_seam(Face::Left, Edge::Top, t),
            Some((Face::Top, 0, t, 3))
        );

        // And the remaining vertical seams, all untwisted.
        assert_eq!(
            cross_seam(Face::Right, Edge::Right, t),
            Some((Face::Back, 0, t, 0))
        );
        assert_eq!(
            cross_seam(Face::Back, Edge::Right, t),
            Some((Face::Left, 0, t, 0))
        );
        assert_eq!(
            cross_seam(Face::Left, Edge::Right, t),
            Some((Face::Front, 0, t, 0))
        );
        assert_eq!(
            cross_seam(Face::Front, Edge::Left, t),
            Some((Face::Left, MAX, t, 0))
        );

        // Back the other way across the Top seams.
        assert_eq!(
            cross_seam(Face::Top, Edge::Bottom, t),
            Some((Face::Front, t, 0, 0))
        );
        assert_eq!(
            cross_seam(Face::Top, Edge::Right, t),
            Some((Face::Right, MAX - t, 0, 3))
        );
        assert_eq!(
            cross_seam(Face::Top, Edge::Top, t),
            Some((Face::Back, MAX - t, 0, 2))
        );
        assert_eq!(
            cross_seam(Face::Top, Edge::Left, t),
            Some((Face::Left, t, 0, 1))
        );
    }

    for f in [Face::Front, Face::Right, Face::Back, Face::Left] {
        assert_eq!(cross_seam(f, Edge::Bottom, 0), None);
        assert_eq!(cross_seam(f, Edge::Bottom, 63), None);
    }
}

#[test]
fn cross_seam_round_trips() {
    for f in Face::ALL {
        for e in Edge::ALL {
            let Some(s) = f.neighbor(e) else { continue };
            for t in 0..N {
                let (f2, x2, y2, rot) = cross_seam(f, e, t).unwrap();
                assert_eq!(f2, s.face);
                // Immediately leave the face we just entered, across the edge we entered by.
                let t2 = s.edge.coord_of(x2, y2);
                let (f3, x3, y3, rot2) = cross_seam(f2, s.edge, t2).unwrap();
                assert_eq!(f3, f, "{f:?}.{e:?} t={t} did not come back");
                assert_eq!(e.coord_of(x3, y3), t, "{f:?}.{e:?} t={t} changed t");
                assert_eq!((rot + rot2) % 4, 0, "{f:?}.{e:?} rotations must undo");
            }
        }
    }
}

/// The returned rotation must carry both the normal and the tangent correctly: a heading
/// that leaves across `e` points along `e.outward()` and must enter pointing along
/// `-e2.outward()`, and one step along the seam on this side must be one step along the
/// seam on the other side (backwards if `reversed`). Together these say straight lines stay
/// straight.
#[test]
fn rotations_keep_straight_lines_straight() {
    for f in Face::ALL {
        for e in Edge::ALL {
            let Some(s) = f.neighbor(e) else { continue };
            let (_, _, _, rot) = cross_seam(f, e, 0).unwrap();

            // Normal: outward here becomes inward there.
            let out = e.outward();
            let (nx, ny) = rotate_heading(rot, out.0, out.1);
            let far_out = s.edge.outward();
            assert_eq!(
                (nx, ny),
                (-far_out.0, -far_out.1),
                "{f:?}.{e:?}: heading does not enter {:?}.{:?}",
                s.face,
                s.edge
            );

            // Tangent: +1 along t here maps to ±1 along t there, matching `reversed`.
            let tan = match e {
                Edge::Top | Edge::Bottom => (1, 0), // t is x, running left -> right
                Edge::Left | Edge::Right => (0, 1), // t is y, running top -> bottom
            };
            let far_tan = match s.edge {
                Edge::Top | Edge::Bottom => (1, 0),
                Edge::Left | Edge::Right => (0, 1),
            };
            let sign = if s.reversed { -1 } else { 1 };
            assert_eq!(
                rotate_heading(rot, tan.0, tan.1),
                (sign * far_tan.0, sign * far_tan.1),
                "{f:?}.{e:?}: tangent does not match reversed={}",
                s.reversed
            );

            // ... and the entry pixels really do step that way.
            for t in 0..MAX {
                let (_, xa, ya) = cross_seam(f, e, t).map(|(a, b, c, _)| (a, b, c)).unwrap();
                let (_, xb, yb) = cross_seam(f, e, t + 1)
                    .map(|(a, b, c, _)| (a, b, c))
                    .unwrap();
                let step = (i32::from(xb) - i32::from(xa), i32::from(yb) - i32::from(ya));
                assert_eq!(step, rotate_heading(rot, tan.0, tan.1), "{f:?}.{e:?} t={t}");
            }
        }
    }
}

/// The two quarter-turn ("twisted") seams, spelled out on a concrete diagonal heading.
#[test]
fn twisted_seams_continue_straight() {
    // A particle at Right (10, 0) moving up-and-right, (dx, dy) = (1, -1). It leaves across
    // Right's top edge at t = 10 and enters Top at (63, 53) — one quarter turn CCW, so the
    // heading becomes (dy, -dx) = (-1, -1): it moves left (into Top, away from the x=63
    // column) and up (toward Top's Back row). Correct: on Right it was heading toward -Z
    // (increasing x) and upward; on Top, -Z is decreasing y.
    let (f, x, y, rot) = cross_seam(Face::Right, Edge::Top, 10).unwrap();
    assert_eq!((f, x, y, rot), (Face::Top, 63, 53, 1));
    assert_eq!(rotate_heading(rot, 1, -1), (-1, -1));

    // A particle at Left (10, 0) moving up-and-right, (1, -1). It enters Top at (0, 10) —
    // three quarter turns CCW, heading becomes (-dy, dx) = (1, 1): rightward into Top and
    // down toward Front. Correct: on Left, increasing x is toward +Z (Front), which on Top
    // is increasing y.
    let (f, x, y, rot) = cross_seam(Face::Left, Edge::Top, 10).unwrap();
    assert_eq!((f, x, y, rot), (Face::Top, 0, 10, 3));
    assert_eq!(rotate_heading(rot, 1, -1), (1, 1));

    // The half-turn seam: Back (10, 0) moving up-and-right enters Top at (53, 0) with the
    // heading negated, (-1, 1): leftward and downward, i.e. still toward +X in world terms
    // (Back's -x is Top's +x) and still away from the -Z edge.
    let (f, x, y, rot) = cross_seam(Face::Back, Edge::Top, 10).unwrap();
    assert_eq!((f, x, y, rot), (Face::Top, 53, 0, 2));
    assert_eq!(rotate_heading(rot, 1, -1), (-1, 1));
}

// ---------------------------------------------------------------------------------------
// Deliverable 2: cubemap adapter
// ---------------------------------------------------------------------------------------

/// Face `k`, texel `(s_i, t_i)` holds RGB = `(k, s_i, t_i)`.
fn synthetic_cubemap() -> Vec<Vec<u8>> {
    (0..6u8)
        .map(|k| {
            let mut buf = vec![0u8; FACE_BYTES];
            for t in 0..N {
                for s in 0..N {
                    let o = (usize::from(t) * 64 + usize::from(s)) * 3;
                    buf[o] = k;
                    buf[o + 1] = s;
                    buf[o + 2] = t;
                }
            }
            buf
        })
        .collect()
}

fn frame_from_synthetic(cm: &[Vec<u8>]) -> Frame {
    let faces = CubemapFaces {
        faces: [
            cm[0].as_slice(),
            cm[1].as_slice(),
            cm[2].as_slice(),
            cm[3].as_slice(),
            cm[4].as_slice(),
            cm[5].as_slice(),
        ],
    };
    Frame::from_cubemap(&faces)
}

/// Twenty corners, four per face, each worked out from the direction of the pixel centre
/// and the GL selection table (`s = (sc/|ma| + 1)/2`, `t = (tc/|ma| + 1)/2`,
/// `s_i = floor(64 s)`).
///
/// Every corner pixel centre has coordinates ±63/64 on the two non-major axes, so the two
/// possible values of `s` (and of `t`) are `(1 - 63/64)/2 = 1/128` (→ `64 s = 0.5`,
/// `s_i = 0`) and `(1 + 63/64)/2 = 127/128` (→ `64 s = 63.5`, `s_i = 63`). The work is in
/// deciding which, per face and per axis.
#[test]
fn cubemap_corners_of_all_five_faces() {
    let cm = synthetic_cubemap();
    let fr = frame_from_synthetic(&cm);

    // ---- Front: centre = (a, -b, 1). Major axis +Z (|1| > 63/64) -> GL face 4 (+Z),
    // sc = +rx = a, tc = -ry = b, ma = 1.
    // (0,0):   a = -63/64, b = -63/64 -> s = 1/128   -> s_i = 0 ; t = 1/128   -> t_i = 0
    assert_eq!(fr.get(Face::Front, 0, 0), [GL_POS_Z as u8, 0, 0]);
    // (63,0):  a = +63/64, b = -63/64 -> s = 127/128 -> s_i = 63; t_i = 0
    assert_eq!(fr.get(Face::Front, 63, 0), [GL_POS_Z as u8, 63, 0]);
    // (0,63):  a = -63/64, b = +63/64 -> s_i = 0 ; t = 127/128 -> t_i = 63
    assert_eq!(fr.get(Face::Front, 0, 63), [GL_POS_Z as u8, 0, 63]);
    // (63,63): s_i = 63; t_i = 63
    assert_eq!(fr.get(Face::Front, 63, 63), [GL_POS_Z as u8, 63, 63]);

    // ---- Right: centre = (1, -b, -a). Major axis +X -> GL face 0 (+X),
    // sc = -rz = a, tc = -ry = b, ma = 1.
    // (0,0):   a = -63/64 -> sc = -63/64 -> s_i = 0 ; b = -63/64 -> t_i = 0
    assert_eq!(fr.get(Face::Right, 0, 0), [GL_POS_X as u8, 0, 0]);
    // (63,0):  a = +63/64 -> sc = +63/64 -> s_i = 63; t_i = 0
    assert_eq!(fr.get(Face::Right, 63, 0), [GL_POS_X as u8, 63, 0]);
    // (0,63):  s_i = 0 ; b = +63/64 -> t_i = 63
    assert_eq!(fr.get(Face::Right, 0, 63), [GL_POS_X as u8, 0, 63]);
    assert_eq!(fr.get(Face::Right, 63, 63), [GL_POS_X as u8, 63, 63]);

    // ---- Back: centre = (-a, -b, -1). Major axis -Z -> GL face 5 (-Z),
    // sc = -rx = a, tc = -ry = b, |ma| = 1.
    // (0,0):   rx = +63/64 -> sc = -63/64 -> s_i = 0 ; t_i = 0
    assert_eq!(fr.get(Face::Back, 0, 0), [GL_NEG_Z as u8, 0, 0]);
    // (63,0):  rx = -63/64 -> sc = +63/64 -> s_i = 63; t_i = 0
    assert_eq!(fr.get(Face::Back, 63, 0), [GL_NEG_Z as u8, 63, 0]);
    assert_eq!(fr.get(Face::Back, 0, 63), [GL_NEG_Z as u8, 0, 63]);
    assert_eq!(fr.get(Face::Back, 63, 63), [GL_NEG_Z as u8, 63, 63]);

    // ---- Left: centre = (-1, -b, a). Major axis -X -> GL face 1 (-X),
    // sc = +rz = a, tc = -ry = b, |ma| = 1.
    // (0,0):   a = -63/64 -> s_i = 0 ; t_i = 0
    assert_eq!(fr.get(Face::Left, 0, 0), [GL_NEG_X as u8, 0, 0]);
    // (63,0):  a = +63/64 -> s_i = 63; t_i = 0
    assert_eq!(fr.get(Face::Left, 63, 0), [GL_NEG_X as u8, 63, 0]);
    assert_eq!(fr.get(Face::Left, 0, 63), [GL_NEG_X as u8, 0, 63]);
    assert_eq!(fr.get(Face::Left, 63, 63), [GL_NEG_X as u8, 63, 63]);

    // ---- Top: centre = (a, 1, b). Major axis +Y -> GL face 2 (+Y),
    // sc = +rx = a, tc = +rz = b, ma = 1.
    // (0,0):   a = -63/64 -> s_i = 0 ; b = -63/64 (the Back edge) -> t = 1/128 -> t_i = 0
    assert_eq!(fr.get(Face::Top, 0, 0), [GL_POS_Y as u8, 0, 0]);
    // (63,0):  a = +63/64 -> s_i = 63; t_i = 0
    assert_eq!(fr.get(Face::Top, 63, 0), [GL_POS_Y as u8, 63, 0]);
    // (0,63):  b = +63/64 (the Front edge) -> t = 127/128 -> t_i = 63
    assert_eq!(fr.get(Face::Top, 0, 63), [GL_POS_Y as u8, 0, 63]);
    assert_eq!(fr.get(Face::Top, 63, 63), [GL_POS_Y as u8, 63, 63]);
}

/// Centre-ish pixel (32, 32) of each face: `a = b = (65/64) - 1 = 1/64`, so on every face
/// the non-major components are ±1/64, giving `s = (1 ± 1/64)/2` -> `64 s = 32.5` or
/// `31.5`, i.e. texel 32 or 31. Working the signs through as above:
///   Front sc = a = +1/64 -> 32, tc = b = +1/64 -> 32.  Right sc = a -> 32, tc = b -> 32.
///   Back  sc = a -> 32, tc = b -> 32.  Left sc = a -> 32, tc = b -> 32.
///   Top   sc = a -> 32, tc = b -> 32.
#[test]
fn cubemap_centre_pixels() {
    let cm = synthetic_cubemap();
    let fr = frame_from_synthetic(&cm);
    assert_eq!(fr.get(Face::Front, 32, 32), [GL_POS_Z as u8, 32, 32]);
    assert_eq!(fr.get(Face::Right, 32, 32), [GL_POS_X as u8, 32, 32]);
    assert_eq!(fr.get(Face::Back, 32, 32), [GL_NEG_Z as u8, 32, 32]);
    assert_eq!(fr.get(Face::Left, 32, 32), [GL_NEG_X as u8, 32, 32]);
    assert_eq!(fr.get(Face::Top, 32, 32), [GL_POS_Y as u8, 32, 32]);
}

/// Every one of the 20480 pixels comes from the expected GL face, the −Y face is never
/// touched, and each used face is covered exactly once per texel (a bijection: no texel is
/// sampled twice and none is skipped).
#[test]
fn cubemap_face_selection_is_exhaustive_and_bijective() {
    let cm = synthetic_cubemap();
    let fr = frame_from_synthetic(&cm);
    let mut seen = [[false; 4096]; 6];
    for (face, expect) in [
        (Face::Front, GL_POS_Z),
        (Face::Right, GL_POS_X),
        (Face::Back, GL_NEG_Z),
        (Face::Left, GL_NEG_X),
        (Face::Top, GL_POS_Y),
    ] {
        for y in 0..N {
            for x in 0..N {
                let [k, s, t] = fr.get(face, usize::from(x), usize::from(y));
                assert_eq!(
                    usize::from(k),
                    expect,
                    "{face:?} ({x},{y}) sampled GL face {k}"
                );
                assert_ne!(usize::from(k), GL_NEG_Y, "the -Y face must never be used");
                assert!(s < N && t < N);
                let slot = &mut seen[usize::from(k)][usize::from(t) * 64 + usize::from(s)];
                assert!(!*slot, "{face:?} ({x},{y}) re-samples texel ({s},{t})");
                *slot = true;
            }
        }
    }
    assert!(seen[GL_NEG_Y].iter().all(|&b| !b));
    for k in [GL_POS_X, GL_NEG_X, GL_POS_Y, GL_POS_Z, GL_NEG_Z] {
        assert!(seen[k].iter().all(|&b| b), "GL face {k} not fully covered");
    }
}

/// Position of the centre of cube-map texel `(s_i, t_i)` of GL face `k` on the cube
/// `[-1, 1]³`, by hand-inverting the GL selection table. With `u = 2s - 1` and
/// `v = 2t - 1` (so `sc/|ma| = u`, `tc/|ma| = v`):
///   +X: sc = -rz, tc = -ry, rx = +1 -> ( 1, -v, -u)
///   -X: sc = +rz, tc = -ry, rx = -1 -> (-1, -v,  u)
///   +Y: sc = +rx, tc = +rz, ry = +1 -> ( u,  1,  v)
///   -Y: sc = +rx, tc = -rz, ry = -1 -> ( u, -1, -v)
///   +Z: sc = +rx, tc = -ry, rz = +1 -> ( u, -v,  1)
///   -Z: sc = -rx, tc = -ry, rz = -1 -> (-u, -v, -1)
fn texel_position(k: usize, s_i: u8, t_i: u8) -> [f64; 3] {
    let u = (2.0 * f64::from(s_i) + 1.0) / 64.0 - 1.0;
    let v = (2.0 * f64::from(t_i) + 1.0) / 64.0 - 1.0;
    match k {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        5 => [-u, -v, -1.0],
        _ => unreachable!(),
    }
}

/// Across every one of the 16 closed half-edges, the texel sampled by the last pixel inside
/// one face and the texel sampled by the first pixel on the other face are neighbours on
/// the cube: both sit 1/64 from the shared cube edge on their own side, so their centres
/// are `sqrt(2)/64` apart (squared distance exactly `2/4096`).
#[test]
fn cubemap_is_continuous_across_every_seam() {
    let cm = synthetic_cubemap();
    let fr = frame_from_synthetic(&cm);
    let sample = |f: Face, x: u8, y: u8| {
        let [k, s, t] = fr.get(f, usize::from(x), usize::from(y));
        texel_position(usize::from(k), s, t)
    };

    for f in Face::ALL {
        for e in Edge::ALL {
            if f.neighbor(e).is_none() {
                continue;
            }
            for t in 0..N {
                let (ax, ay) = e.pixel(t);
                let (f2, bx, by, _) = cross_seam(f, e, t).unwrap();
                let pa = sample(f, ax, ay);
                let pb = sample(f2, bx, by);
                let d2: f64 = (0..3).map(|i| (pa[i] - pb[i]).powi(2)).sum();
                assert!(
                    (d2 - 2.0 / 4096.0).abs() < 1e-12,
                    "{f:?}.{e:?} t={t}: {pa:?} vs {pb:?} (d^2 = {d2})"
                );
            }
        }
    }
}

#[test]
fn from_cubemap_and_fill_from_cubemap_agree() {
    let cm = synthetic_cubemap();
    let faces = CubemapFaces {
        faces: [
            cm[0].as_slice(),
            cm[1].as_slice(),
            cm[2].as_slice(),
            cm[3].as_slice(),
            cm[4].as_slice(),
            cm[5].as_slice(),
        ],
    };
    let a = Frame::from_cubemap(&faces);
    let mut b = Frame::black();
    b.fill([255, 255, 255]);
    b.fill_from_cubemap(&faces);
    assert_eq!(a.as_bytes(), b.as_bytes());
}

#[test]
#[should_panic(expected = "cubemap face 3")]
fn wrong_sized_cubemap_face_panics() {
    let cm = synthetic_cubemap();
    let short = vec![0u8; 3];
    let faces = CubemapFaces {
        faces: [
            cm[0].as_slice(),
            cm[1].as_slice(),
            cm[2].as_slice(),
            short.as_slice(),
            cm[4].as_slice(),
            cm[5].as_slice(),
        ],
    };
    let _ = Frame::from_cubemap(&faces);
}
