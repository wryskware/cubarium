//! Local unfoldings: surface distance, the target's image in the observer chart, and the
//! tangent map that transports the target's directions into that chart.
//!
//! Rules encoded here come from `src/unfold.rs` (the normative derivation of a one-seam
//! image, the `ChartPath` tie-break, `MAX_LOCAL_RADIUS`) and from
//! `design/surface-topology.md` ("Compare unfolded distances against an independent slow
//! reference on random local pairs. Check heading alignment across the twisted Top seams").
//!
//! The independent reference is the 3D oracle: it unfolds each candidate face sequence by
//! rigid rotations about the actual shared cube edges and validates the segment's edge
//! crossings, with no access to the 2D transition table.

use cubarium_surface::{
    ChartImage, ChartPath, Edge, FACE_EXTENT, Face, GEOM_EPS, MAX_LOCAL_RADIUS, MAX_SEAMS,
    SurfacePoint, TangentMap, Vec2, chart_images, cross_seam, segment_is_valid,
    surface_distance, travel, unfold,
};
use cubarium_surface_oracle as oracle;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

const R: f64 = MAX_LOCAL_RADIUS;

fn runner(cases: u32, seed: u8) -> TestRunner {
    TestRunner::new_with_rng(
        Config {
            cases,
            failure_persistence: None,
            ..Config::default()
        },
        TestRng::from_seed(RngAlgorithm::ChaCha, &[seed; 32]),
    )
}

fn any_point() -> impl Strategy<Value = SurfacePoint> {
    (0usize..5, 0.0f64..FACE_EXTENT, 0.0f64..FACE_EXTENT)
        .prop_map(|(f, u, v)| SurfacePoint::new(Face::ALL[f], u, v))
}

/// Random *nearby* pairs: a random anchor plus a swept displacement of at most 28 pixels,
/// so the surface distance is comfortably below `MAX_LOCAL_RADIUS` and the pair lands on
/// every kind of seam, corner and rim neighbourhood in proportion to its area.
fn near_pair() -> impl Strategy<Value = (SurfacePoint, SurfacePoint)> {
    (
        any_point(),
        0.0f64..std::f64::consts::TAU,
        0.0f64..28.0f64,
    )
        .prop_map(|(a, angle, len)| {
            let b = travel(a, Vec2::from_screen_angle(angle) * len).end;
            (a, b)
        })
}

fn oracle_pt(p: SurfacePoint) -> (Face, f64, f64) {
    (p.face, p.u, p.v)
}

/// The along-edge point of `edge` at continuous parameter `s`.
fn edge_point(edge: Edge, s: f64) -> Vec2 {
    match edge {
        Edge::Top => Vec2::new(s, 0.0),
        Edge::Right => Vec2::new(FACE_EXTENT, s),
        Edge::Bottom => Vec2::new(s, FACE_EXTENT),
        Edge::Left => Vec2::new(0.0, s),
    }
}

// --- agreement with the independent 3D reference ---------------------------------------

#[test]
fn unfolded_distance_matches_the_oracle_geodesic() {
    // Coverage counters: a run that only ever produced same-chart pairs would pass
    // without testing an unfolding at all.
    let one_seam = std::cell::Cell::new(0u32);
    let two_seam = std::cell::Cell::new(0u32);
    runner(512, 0xa7)
        .run(&near_pair(), |(a, b)| {
            let got = unfold(a, b, R);
            let want = oracle::geodesic(oracle_pt(a), oracle_pt(b), R);
            prop_assert_eq!(
                got.is_some(),
                want.is_some(),
                "{:?} -> {:?}: production {:?} vs oracle {:?}",
                a,
                b,
                got.map(|u| u.distance),
                want.as_ref().map(|g| g.distance)
            );
            let (Some(got), Some(want)) = (got, want) else {
                return Ok(());
            };
            prop_assert!(
                (got.distance - want.distance).abs() <= 1e-9,
                "{a:?} -> {b:?}: distance {} vs oracle {}",
                got.distance,
                want.distance
            );
            // The 3D chord is a lower bound on surface distance.
            prop_assert!(
                a.chord_sq(&b) <= got.distance * got.distance + 1e-9,
                "chord^2 {} exceeds distance^2 {}",
                a.chord_sq(&b),
                got.distance * got.distance
            );
            // The image is consistent with the distance it reports.
            prop_assert!(
                ((got.local - a.chart()).length() - got.distance).abs() <= 1e-9,
                "local {:?} is {} from the observer but distance says {}",
                got.local,
                (got.local - a.chart()).length(),
                got.distance
            );
            prop_assert!(got.local.is_finite());
            prop_assert!(got.path.len <= MAX_SEAMS);
            match got.path.len {
                1 => one_seam.set(one_seam.get() + 1),
                2 => two_seam.set(two_seam.get() + 1),
                _ => {}
            }
            prop_assert_eq!(got.path.final_face(a.face), b.face, "path does not reach the target");
            // The map is a pure rotation: unfoldings never reflect.
            prop_assert_eq!(got.map.det(), 1, "unfolding reflected: {:?}", got.map);
            prop_assert!(got.map.as_quarter_turns().is_some());

            // Symmetry, and the two maps are mutually inverse (they transport tangents in
            // opposite directions). A pair sitting exactly on a length tie between two
            // different chart paths could in principle pick non-inverse maps; that set has
            // measure zero for random floats.
            let back = unfold(b, a, R).expect("symmetric");
            prop_assert!(
                (back.distance - got.distance).abs() <= 1e-9,
                "asymmetric distance: {} vs {}",
                got.distance,
                back.distance
            );
            prop_assert_eq!(got.map.then(back.map), TangentMap::IDENTITY);
            prop_assert!(
                ((back.local - b.chart()).length() - back.distance).abs() <= 1e-9
            );
            prop_assert_eq!(
                surface_distance(a, b, R).map(f64::to_bits),
                Some(got.distance.to_bits())
            );
            Ok(())
        })
        .expect("unfolded distances match the oracle");
    assert!(one_seam.get() > 20, "only {} pairs crossed a seam", one_seam.get());
    assert!(two_seam.get() > 0, "no pair was reached around a vertex");
}

// --- invariance ------------------------------------------------------------------------

/// "The same patch, sensor disk, body, or contact pair moved over a seam does not gain
/// energy, brightness, neighbors, or interaction range": translating both points of a pair
/// by the same swept displacement leaves the surface distance unchanged, as long as no rim
/// reflection folded the pair.
#[test]
fn translating_a_pair_across_a_seam_preserves_its_distance() {
    // (anchor, offset to the partner, translation applied to both). The translation is
    // chosen so that the moved pair straddles the seam in the last four cases: after the
    // move the anchor is still on the side face and its partner has crossed onto Top.
    let fixtures: [(SurfacePoint, Vec2, Vec2); 7] = [
        // Straight across the flat Front/Right seam.
        (SurfacePoint::new(Face::Front, 50.0, 30.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 0.0)),
        (SurfacePoint::new(Face::Front, 50.0, 30.0), Vec2::new(7.0, -5.0), Vec2::new(16.0, 3.0)),
        // Around the Back/Left vertical seam.
        (SurfacePoint::new(Face::Back, 58.0, 40.0), Vec2::new(9.0, 2.0), Vec2::new(9.0, 0.0)),
        // Onto Top: flat (Front), quarter turn (Right), half turn (Back), quarter turn
        // the other way (Left).
        (SurfacePoint::new(Face::Front, 30.0, 10.0), Vec2::new(0.0, -5.0), Vec2::new(0.0, -6.0)),
        (SurfacePoint::new(Face::Right, 30.0, 10.0), Vec2::new(0.0, -5.0), Vec2::new(0.0, -6.0)),
        (SurfacePoint::new(Face::Back, 30.0, 10.0), Vec2::new(0.0, -5.0), Vec2::new(0.0, -6.0)),
        (SurfacePoint::new(Face::Left, 30.0, 10.0), Vec2::new(0.0, -5.0), Vec2::new(0.0, -6.0)),
    ];
    for (a, offset, shift) in fixtures {
        let b = travel(a, offset);
        assert!(!b.fallback);
        let before = surface_distance(a, b.end, R).expect("pair within range");
        let moved_a = travel(a, shift);
        assert!(!moved_a.fallback);
        if moved_a.reflections > 0 {
            continue;
        }
        // The pair offset travels with the anchor: apply the accumulated tangent map.
        let moved_b = travel(moved_a.end, moved_a.map.apply(offset));
        assert!(!moved_b.fallback);
        if moved_b.reflections > 0 || b.reflections > 0 {
            continue;
        }
        let after = surface_distance(moved_a.end, moved_b.end, R).expect("pair within range");
        assert!(
            (before - after).abs() <= 1e-9,
            "{a:?} + {offset:?} was {before} apart, {after} after moving by {shift:?}"
        );
        assert!(
            (before - offset.length()).abs() <= 1e-9,
            "a straight offset of {:?} should measure {} but measured {before}",
            offset,
            offset.length()
        );
    }
}

/// Heading alignment across the twisted Top seams: the map that unfolding uses to bring a
/// neighbour's directions into the observer's chart is exactly the inverse of the map that
/// transporting the observer to the neighbour produces (`Unfolded::map` transports
/// target-chart tangents into the observer chart, `Travel::map` goes the other way).
#[test]
fn heading_alignment_across_the_seams_around_top() {
    let cases: [(Face, f64, f64, u8); 4] = [
        // (chart, u, v, expected quarter turns of Travel::map) — the twisted seams first.
        (Face::Right, 20.0, 2.0, 1),
        (Face::Back, 20.0, 2.0, 2),
        (Face::Left, 20.0, 2.0, 3),
        (Face::Front, 20.0, 2.0, 0),
    ];
    for (face, u, v, turns) in cases {
        let a = SurfacePoint::new(face, u, v);
        let step = Vec2::new(0.0, -6.0);
        let tr = travel(a, step);
        assert_eq!(tr.end.face, Face::Top, "{face:?} should step onto Top");
        assert_eq!(tr.map, TangentMap::quarter_turns(turns), "{face:?}");

        let uf = unfold(a, tr.end, R).expect("6 px apart");
        assert!(
            (uf.distance - 6.0).abs() <= 1e-9,
            "{face:?}: distance {} for a 6 px step",
            uf.distance
        );
        // The unfolded image of the target sits exactly where the straight step ended.
        let want = a.chart() + step;
        assert!(
            (uf.local - want).length() <= 1e-9,
            "{face:?}: local {:?} should be {want:?}",
            uf.local
        );
        assert_eq!(
            uf.map,
            tr.map.inverse(),
            "{face:?}: unfolding must undo the transport the sweep applied"
        );
        // Concretely: the heading that pointed up in the observer's chart still points up
        // after being transported out and unfolded back.
        let up = Vec2::new(0.0, -1.0);
        assert_eq!(uf.map.apply(tr.map.apply(up)), up, "{face:?}");
        assert_eq!(uf.path.len, 1);
        assert_eq!(uf.path.steps()[0], (face, Edge::Top));
    }
}

// --- range limits ----------------------------------------------------------------------

#[test]
fn a_pair_forty_pixels_apart_is_out_of_range() {
    // Unfolded across the flat Front/Right seam these are (64 - 60) + 36 = 40 px apart.
    let a = SurfacePoint::new(Face::Front, 60.0, 32.0);
    let b = SurfacePoint::new(Face::Right, 36.0, 32.0);
    assert!(unfold(a, b, R).is_none(), "40 px is beyond a 32 px query");
    assert!(unfold(b, a, R).is_none());
    assert!(oracle::geodesic(oracle_pt(a), oracle_pt(b), R).is_none());
    // Shrinking the gap brings it back into range and both agree on the distance.
    let b = SurfacePoint::new(Face::Right, 20.0, 32.0);
    let got = unfold(a, b, R).expect("24 px apart");
    assert!((got.distance - 24.0).abs() <= 1e-9, "{got:?}");
    let want = oracle::geodesic(oracle_pt(a), oracle_pt(b), R).expect("in range");
    assert!((got.distance - want.distance).abs() <= 1e-9);
}

#[test]
#[should_panic]
fn a_query_beyond_max_local_radius_panics() {
    let a = SurfacePoint::new(Face::Front, 32.0, 32.0);
    let b = SurfacePoint::new(Face::Front, 33.0, 32.0);
    let _ = unfold(a, b, MAX_LOCAL_RADIUS + 1.0);
}

// --- chart images ----------------------------------------------------------------------

/// The normative one-seam derivation in `src/unfold.rs`: the image rotation is the inverse
/// of the seam's quarter-turn count, and the origin is fixed by making the neighbour's
/// entry point at parameter `s'` land on the observer's exit point at parameter `s`.
#[test]
fn one_seam_chart_images_place_the_seam_where_the_seam_is() {
    let mut images = Vec::new();
    for face in Face::ALL {
        chart_images(face, 1, &mut images);
        assert_eq!(images[0].path, ChartPath::direct(), "identity image comes first");
        assert_eq!(images[0].target_face, face);
        assert_eq!(images[0].map, TangentMap::IDENTITY);
        assert_eq!(images[0].origin, Vec2::ZERO);

        let connected = Edge::ALL.into_iter().filter(|&e| face.neighbor(e).is_some()).count();
        assert_eq!(images.len(), connected + 1, "{face:?}: one image per open seam");
        assert!(
            images.windows(2).all(|w| w[0].path < w[1].path),
            "{face:?}: images must be sorted by ChartPath and unique"
        );

        for img in images.iter().skip(1) {
            let (from, edge) = img.path.steps()[0];
            assert_eq!(from, face);
            let seam = face.neighbor(edge).expect("chart paths never cross the rim");
            assert_eq!(img.target_face, seam.face);
            let (_, _, _, turns) = cross_seam(face, edge, 0).expect("connected");
            assert_eq!(
                img.map,
                TangentMap::quarter_turns(turns).inverse(),
                "{face:?}/{edge:?}: image rotation"
            );
            assert_eq!(img.map.det(), 1, "images never reflect");

            for s in [0.0f64, 17.0, 64.0] {
                let s2 = if seam.reversed { FACE_EXTENT - s } else { s };
                let entry = edge_point(seam.edge, s2);
                let exit = edge_point(edge, s);
                let got = img.image_point(entry);
                assert!(
                    (got - exit).length() <= 1e-9,
                    "{face:?}/{edge:?} at s={s}: entry {entry:?} imaged to {got:?}, want {exit:?}"
                );
                assert!((img.preimage_point(got) - entry).length() <= 1e-9);
            }
        }
    }
}

/// Two-seam images exist (the routes around a top vertex), never immediately return
/// through the edge they entered by, and are consistent with composing two one-seam
/// images.
#[test]
fn two_seam_chart_images_are_complete_and_consistent() {
    let mut one = Vec::new();
    let mut two = Vec::new();
    for face in Face::ALL {
        chart_images(face, 1, &mut one);
        chart_images(face, MAX_SEAMS, &mut two);
        assert!(two.len() > one.len(), "{face:?}: no two-seam images");
        assert!(
            two.windows(2).all(|w| w[0].path < w[1].path),
            "{face:?}: sorted and unique"
        );
        for img in &two {
            if img.path.len < 2 {
                continue;
            }
            let (f0, e0) = img.path.steps()[0];
            let (f1, e1) = img.path.steps()[1];
            assert_eq!(f0, face);
            let seam = f0.neighbor(e0).expect("never the rim");
            assert_eq!(f1, seam.face);
            assert_ne!(e1, seam.edge, "a path must not return through its entry edge");
            let seam2 = f1.neighbor(e1).expect("never the rim");
            assert_eq!(img.target_face, seam2.face);
            assert_eq!(img.target_face, img.path.final_face(face));

            // Composing the first step's image with the second step's image (taken in the
            // intermediate chart) must reproduce this image.
            let first = one
                .iter()
                .find(|i| i.path.len == 1 && i.path.steps()[0] == (f0, e0))
                .copied()
                .expect("one-seam image");
            let mut mid = Vec::new();
            chart_images(f1, 1, &mut mid);
            let second: ChartImage = *mid
                .iter()
                .find(|i| i.path.len == 1 && i.path.steps()[0] == (f1, e1))
                .expect("one-seam image of the intermediate chart");
            for probe in [Vec2::new(0.0, 0.0), Vec2::new(64.0, 0.0), Vec2::new(17.0, 43.0)] {
                let want = first.image_point(second.image_point(probe));
                let got = img.image_point(probe);
                assert!(
                    (got - want).length() <= 1e-9,
                    "{face:?} {:?}: {got:?} vs composed {want:?}",
                    img.path
                );
            }
        }
    }
}

/// `segment_is_valid` is the gate that rejects an unfolding whose straight segment does
/// not actually cross the edges its path claims (`src/unfold.rs`).
#[test]
fn segment_validity_fixtures() {
    let direct = ChartPath::direct();
    let one = |face: Face, edge: Edge| ChartPath {
        len: 1,
        steps: [(face, edge), (Face::Front, Edge::Top)],
    };

    // Inside one chart: only the empty path is valid.
    assert!(segment_is_valid(Face::Front, Vec2::new(10.0, 10.0), Vec2::new(20.0, 20.0), &direct));
    assert!(!segment_is_valid(
        Face::Front,
        Vec2::new(10.0, 10.0),
        Vec2::new(20.0, 20.0),
        &one(Face::Front, Edge::Right)
    ));

    // Across the Front/Right seam: Right (6, 30) unfolds to (70, 30) in Front's chart.
    assert!(segment_is_valid(
        Face::Front,
        Vec2::new(60.0, 30.0),
        Vec2::new(70.0, 30.0),
        &one(Face::Front, Edge::Right)
    ));
    assert!(!segment_is_valid(Face::Front, Vec2::new(60.0, 30.0), Vec2::new(70.0, 30.0), &direct));
    // ... but not through the wrong edge.
    assert!(!segment_is_valid(
        Face::Front,
        Vec2::new(60.0, 30.0),
        Vec2::new(70.0, 30.0),
        &one(Face::Front, Edge::Top)
    ));

    // Leaving through the open rim is never a valid unfolding.
    assert!(!segment_is_valid(
        Face::Front,
        Vec2::new(30.0, 60.0),
        Vec2::new(30.0, 70.0),
        &one(Face::Front, Edge::Bottom)
    ));
    assert!(!segment_is_valid(Face::Front, Vec2::new(30.0, 60.0), Vec2::new(30.0, 70.0), &direct));

    // Boundary-inclusive within GEOM_EPS: a segment ending exactly on the chart edge is
    // still inside the observer's chart.
    assert!(segment_is_valid(
        Face::Front,
        Vec2::new(30.0, 30.0),
        Vec2::new(FACE_EXTENT - GEOM_EPS / 2.0, 30.0),
        &direct
    ));
}
