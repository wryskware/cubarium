//! The exhaustive discrete seam contract.
//!
//! Every rule checked here is stated in `crates/cubarium-surface/src/travel.rs`
//! (the normative `travel` algorithm), `src/point.rs` (`pixel_neighbor`) and
//! `src/lib.rs` (the conventions section), and is required as evidence by
//! `design/surface-topology.md` ("Exhaust all 16 connected half-edges and all 64
//! integer edge positions against `cube-proto`; verify inverses, reversals, heading
//! rotation, and the four open edges").
//!
//! The reference for the discrete answers is `cube_proto::cross_seam` itself: this file
//! pins the continuous `travel` to the discrete contract the shim already owns and tests.

use cubarium_surface::{
    Edge, Face, FACE_EXTENT, GEOM_EPS, SurfacePoint, TangentMap, Vec2, cross_seam,
    pixel_neighbor, travel,
};

const SIDE_FACES: [Face; 4] = [Face::Front, Face::Right, Face::Back, Face::Left];

/// The center of the pixel just inside `edge` at along-edge index `t`, together with the
/// unit displacement pointing straight out through that edge. Half a pixel of the step is
/// spent reaching the chart boundary and half is spent past it, so the landing pixel is
/// the neighbouring chart's edge pixel.
fn start_and_step(face: Face, edge: Edge, t: u8) -> (SurfacePoint, Vec2) {
    let s = f64::from(t) + 0.5;
    match edge {
        Edge::Top => (SurfacePoint::new(face, s, 0.5), Vec2::new(0.0, -1.0)),
        Edge::Right => (SurfacePoint::new(face, 63.5, s), Vec2::new(1.0, 0.0)),
        Edge::Bottom => (SurfacePoint::new(face, s, 63.5), Vec2::new(0.0, 1.0)),
        Edge::Left => (SurfacePoint::new(face, 0.5, s), Vec2::new(-1.0, 0.0)),
    }
}

/// The continuous along-edge parameter of a point sitting on `edge`: `u` for the
/// horizontal edges, `v` for the vertical ones (`src/travel.rs` step 5).
fn along_param(edge: Edge, p: SurfacePoint) -> f64 {
    match edge {
        Edge::Top | Edge::Bottom => p.u,
        Edge::Left | Edge::Right => p.v,
    }
}

/// Where a point half a pixel inside the entry edge puts the entry parameter back.
fn entry_param(edge: Edge, p: SurfacePoint) -> f64 {
    along_param(edge, p)
}

fn assert_close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} != {b} (tol {tol})");
}

/// All 16 connected half-edges at all 64 integer positions land exactly where
/// `cross_seam` says, with the heading rotated by the quarter turns it reports.
#[test]
fn every_connected_half_edge_lands_where_cross_seam_says() {
    let mut checked = 0;
    for face in Face::ALL {
        for edge in Edge::ALL {
            for t in 0..64u8 {
                let Some((nf, nx, ny, turns)) = cross_seam(face, edge, t) else {
                    continue;
                };
                let (start, step) = start_and_step(face, edge, t);
                let tr = travel(start, step);
                assert_eq!(tr.end.face, nf, "{face:?}/{edge:?} t={t}: face");
                assert_eq!(tr.end.pixel(), (nx, ny), "{face:?}/{edge:?} t={t}: pixel");
                assert_eq!(
                    tr.map,
                    TangentMap::quarter_turns(turns),
                    "{face:?}/{edge:?} t={t}: tangent map"
                );
                assert_eq!(tr.crossings, 1, "{face:?}/{edge:?} t={t}: crossings");
                assert_eq!(tr.reflections, 0, "{face:?}/{edge:?} t={t}: reflections");
                assert_eq!(tr.ties, 0, "{face:?}/{edge:?} t={t}: ties");
                assert!(!tr.fallback, "{face:?}/{edge:?} t={t}: fallback");
                assert!(tr.end.is_canonical(), "{face:?}/{edge:?} t={t}: {:?}", tr.end);
                // One segment in each chart, together exactly the displacement length.
                assert_eq!(tr.segments.len(), 2, "{face:?}/{edge:?} t={t}: segments");
                let total: f64 = tr.segments.iter().map(|s| s.length()).sum();
                assert_close(total, 1.0, 1e-12, "swept length");
                assert_eq!(tr.segments[0].face, face);
                assert_eq!(tr.segments[1].face, nf);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 16 * 64, "16 connected half-edges x 64 positions");
}

/// Crossing and then travelling back with the transported, reversed displacement returns
/// to the exact starting point, and the two tangent maps compose to the identity
/// ("Returning across that seam is identity", design/surface-topology.md).
#[test]
fn crossing_a_seam_is_invertible() {
    for face in Face::ALL {
        for edge in Edge::ALL {
            if face.neighbor(edge).is_none() {
                continue;
            }
            for t in 0..64u8 {
                let (start, step) = start_and_step(face, edge, t);
                let out = travel(start, step);
                let back = travel(out.end, out.map.apply(-step));
                assert_eq!(back.end.face, start.face, "{face:?}/{edge:?} t={t}");
                assert_close(back.end.u, start.u, 1e-12, "u after return");
                assert_close(back.end.v, start.v, 1e-12, "v after return");
                assert_eq!(
                    out.map.then(back.map),
                    TangentMap::IDENTITY,
                    "{face:?}/{edge:?} t={t}: round trip rotated the tangent frame"
                );
                assert_eq!(back.crossings, 1);
                assert_eq!(back.reflections, 0);
                assert!(!back.fallback);
            }
        }
    }
}

/// The four open bottom edges reflect instead of crossing: same chart, `REFLECT_Y`, one
/// reflection, and (for a symmetric step) the point comes straight back to where it was.
#[test]
fn the_four_open_bottom_edges_reflect() {
    for face in SIDE_FACES {
        assert!(face.neighbor(Edge::Bottom).is_none(), "{face:?} bottom is open");
        for t in 0..64u8 {
            let (start, step) = start_and_step(face, Edge::Bottom, t);
            let tr = travel(start, step);
            assert_eq!(tr.end.face, face, "{face:?} t={t}: reflection changed chart");
            assert_eq!(tr.reflections, 1, "{face:?} t={t}: reflections");
            assert_eq!(tr.crossings, 0, "{face:?} t={t}: crossings");
            assert_eq!(tr.map, TangentMap::REFLECT_Y, "{face:?} t={t}: map");
            assert!(!tr.fallback);
            assert_close(tr.end.u, start.u, 1e-12, "u after reflection");
            assert_close(tr.end.v, start.v, 1e-12, "v after reflection");
            let total: f64 = tr.segments.iter().map(|s| s.length()).sum();
            assert_close(total, 1.0, 1e-12, "swept length");
            // Nothing tunnels through the rim.
            for s in &tr.segments {
                assert!(s.from.y <= FACE_EXTENT + GEOM_EPS && s.to.y <= FACE_EXTENT + GEOM_EPS);
            }
        }
    }
    // Top has no open edge at all.
    for edge in Edge::ALL {
        assert!(Face::Top.neighbor(edge).is_some(), "Top/{edge:?} must be connected");
    }
}

/// The continuous along-edge parameter is `64 - s` on exactly the two reversed seams
/// (Right/Top and Back/Top and their inverses) and `s` on the other six, never the
/// integer API's `63 - t` (`src/lib.rs`: "Continuous along-edge reversal is `t -> 64 - t`;
/// the integer API's `63 - t` is for pixel indices only").
#[test]
fn the_along_edge_parameter_reverses_on_exactly_the_two_twisted_top_seams() {
    let reversed_seams = [
        (Face::Right, Edge::Top),
        (Face::Top, Edge::Right),
        (Face::Back, Edge::Top),
        (Face::Top, Edge::Top),
    ];
    let mut reversed_seen = Vec::new();
    let mut straight_seen = 0;
    for face in Face::ALL {
        for edge in Edge::ALL {
            let Some(seam) = face.neighbor(edge) else { continue };
            for t in 0..64u8 {
                let (start, step) = start_and_step(face, edge, t);
                let s = f64::from(t) + 0.5;
                let tr = travel(start, step);
                // Half a pixel past the entry edge, so the entry parameter is unchanged
                // along that edge and reading it back off the end point is exact.
                let got = entry_param(seam.edge, tr.end);
                let want = if seam.reversed { FACE_EXTENT - s } else { s };
                assert_close(
                    got,
                    want,
                    1e-12,
                    &format!("{face:?}/{edge:?} t={t} -> {:?}/{:?}", seam.face, seam.edge),
                );
                // 63 - t would be a whole pixel off; make that explicit at one end.
                if t == 0 && seam.reversed {
                    assert_close(got, 63.5, 1e-12, "reversed entry at t=0");
                }
            }
            if seam.reversed {
                reversed_seen.push((face, edge));
            } else {
                straight_seen += 1;
            }
        }
    }
    reversed_seen.sort();
    let mut want = reversed_seams;
    want.sort();
    assert_eq!(reversed_seen, want, "exactly four reversed half-edges");
    assert_eq!(straight_seen, 12, "twelve straight half-edges");
}

/// `pixel_neighbor` is the pixel-resolution shadow of the same contract: inside a chart it
/// is the adjacent pixel, on an edge it is `cross_seam`, and `None` only at the open rim
/// (`src/point.rs`).
#[test]
fn pixel_neighbor_matches_the_discrete_contract() {
    for face in Face::ALL {
        for y in 0..64u8 {
            for x in 0..64u8 {
                for edge in Edge::ALL {
                    let on_edge = match edge {
                        Edge::Top => y == 0,
                        Edge::Right => x == 63,
                        Edge::Bottom => y == 63,
                        Edge::Left => x == 0,
                    };
                    let got = pixel_neighbor(face, x, y, edge);
                    if !on_edge {
                        let (dx, dy) = edge.outward();
                        let want = (
                            face,
                            (i32::from(x) + dx) as u8,
                            (i32::from(y) + dy) as u8,
                        );
                        assert_eq!(got, Some(want), "{face:?} ({x},{y}) {edge:?}");
                    } else {
                        let t = edge.coord_of(x, y);
                        let want = cross_seam(face, edge, t).map(|(f, nx, ny, _)| (f, nx, ny));
                        assert_eq!(got, want, "{face:?} ({x},{y}) {edge:?}");
                    }
                }
            }
        }
    }
    // The rim is the only place a neighbour is missing.
    let mut missing = 0;
    for face in Face::ALL {
        for x in 0..64u8 {
            for edge in Edge::ALL {
                for y in 0..64u8 {
                    if pixel_neighbor(face, x, y, edge).is_none() {
                        missing += 1;
                        assert_eq!(edge, Edge::Bottom);
                        assert_eq!(y, 63);
                        assert_ne!(face, Face::Top);
                    }
                }
            }
        }
    }
    assert_eq!(missing, 4 * 64, "four open edges of 64 pixels each");
}
