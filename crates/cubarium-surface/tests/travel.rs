//! Continuous swept transport: the worked examples from `design/surface-topology.md`,
//! the properties the normative algorithm in `src/travel.rs` promises, and the vertex and
//! lower-corner fixtures the design document demands as evidence.
//!
//! The independent check is `cubarium-surface-oracle`, which re-derives the same sweep
//! from the 3D cube embedding with Rodrigues rotations about the actual shared edges and
//! knows nothing about `Face::neighbor`, `cross_seam` or any 2D transition table.

use cubarium_surface::{
    Edge, FACE_EXTENT, Face, GEOM_EPS, MAX_CROSSINGS, SurfacePoint, TangentMap, Travel, Vec2,
    travel, travel_into,
};
use cubarium_surface_oracle as oracle;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

// --- helpers ---------------------------------------------------------------------------

fn dist3(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn assert_point(got: SurfacePoint, face: Face, u: f64, v: f64, tol: f64, what: &str) {
    assert_eq!(got.face, face, "{what}: face ({got:?})");
    assert!(
        (got.u - u).abs() <= tol && (got.v - v).abs() <= tol,
        "{what}: expected ({u}, {v}), got {got:?}"
    );
}

/// A deterministic proptest runner: the suite must produce the same cases on every
/// machine and every run, so the RNG is seeded explicitly and nothing is persisted.
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

fn any_displacement(max_len: f64) -> impl Strategy<Value = Vec2> {
    (0.0f64..std::f64::consts::TAU, 1e-3f64..max_len)
        .prop_map(|(a, l)| Vec2::from_screen_angle(a) * l)
}

fn faces_adjacent(a: Face, b: Face) -> bool {
    a == b
        || Edge::ALL
            .into_iter()
            .any(|e| a.neighbor(e).map(|s| s.face) == Some(b))
}

/// The four cube vertices where three faces meet, as the chart corner of each incident
/// face. Read off `SurfacePoint::embed`: e.g. the (1, 1, 1) vertex is Front (64, 0),
/// Right (0, 0) and Top (64, 64).
const TOP_VERTICES: [[(Face, f64, f64); 3]; 4] = [
    [(Face::Front, 64.0, 0.0), (Face::Right, 0.0, 0.0), (Face::Top, 64.0, 64.0)],
    [(Face::Right, 64.0, 0.0), (Face::Back, 0.0, 0.0), (Face::Top, 64.0, 0.0)],
    [(Face::Back, 64.0, 0.0), (Face::Left, 0.0, 0.0), (Face::Top, 0.0, 0.0)],
    [(Face::Left, 64.0, 0.0), (Face::Front, 0.0, 0.0), (Face::Top, 0.0, 64.0)],
];

/// The eight compass directions, as unit displacements in chart coordinates.
fn compass() -> [Vec2; 8] {
    let d = std::f64::consts::FRAC_1_SQRT_2;
    [
        Vec2::new(1.0, 0.0),
        Vec2::new(d, -d),
        Vec2::new(0.0, -1.0),
        Vec2::new(-d, -d),
        Vec2::new(-1.0, 0.0),
        Vec2::new(-d, d),
        Vec2::new(0.0, 1.0),
        Vec2::new(d, d),
    ]
}

// --- the three worked examples from design/surface-topology.md -------------------------

#[test]
fn front_to_right_example() {
    // "Front (63.75, 20) moving (0.5, 0) finishes at Right (0.25, 20)."
    let tr = travel(SurfacePoint::new(Face::Front, 63.75, 20.0), Vec2::new(0.5, 0.0));
    assert_point(tr.end, Face::Right, 0.25, 20.0, 1e-12, "front -> right");
    assert_eq!(tr.map, TangentMap::IDENTITY, "the vertical seams do not twist");
    assert_eq!((tr.crossings, tr.reflections, tr.ties), (1, 0, 0));
    assert!(!tr.fallback);
    let total: f64 = tr.segments.iter().map(|s| s.length()).sum();
    assert!((total - 0.5).abs() < 1e-12);
}

#[test]
fn right_to_top_example_turns_up_into_top_leftward() {
    // "Right (10, 0.25) moving (0, -0.5) finishes at Top (63.75, 54); its upward
    // direction becomes Top-leftward." The along-edge parameter is reversed
    // continuously: 64 - 10 = 54, not 63 - 10.
    let tr = travel(SurfacePoint::new(Face::Right, 10.0, 0.25), Vec2::new(0.0, -0.5));
    assert_point(tr.end, Face::Top, 63.75, 54.0, 1e-12, "right -> top");
    assert_eq!(tr.map, TangentMap::quarter_turns(1));
    assert_eq!(tr.map.apply(Vec2::new(0.0, -1.0)), Vec2::new(-1.0, 0.0), "up -> Top-left");
    assert_eq!((tr.crossings, tr.reflections, tr.ties), (1, 0, 0));
    assert!(!tr.fallback);
}

#[test]
fn back_to_top_example_turns_up_into_top_downward() {
    // "Back (10, 0.25) moving (0, -0.5) finishes at Top (54, 0.25); its upward direction
    // becomes Top-downward."
    let tr = travel(SurfacePoint::new(Face::Back, 10.0, 0.25), Vec2::new(0.0, -0.5));
    assert_point(tr.end, Face::Top, 54.0, 0.25, 1e-12, "back -> top");
    assert_eq!(tr.map, TangentMap::quarter_turns(2));
    assert_eq!(tr.map.apply(Vec2::new(0.0, -1.0)), Vec2::new(0.0, 1.0), "up -> Top-down");
    assert_eq!((tr.crossings, tr.reflections, tr.ties), (1, 0, 0));
    assert!(!tr.fallback);
}

// --- swept properties ------------------------------------------------------------------

/// The properties `src/travel.rs` lists ("segment lengths sum to the displacement length;
/// retracing returns to the start away from ties; speed and angles are preserved across
/// seams; a straight path never tunnels through the open bottom") plus the 3D agreement
/// with the oracle, over random starts and displacements of up to 200 pixels (three face
/// widths, so several seams per sweep).
#[test]
fn swept_transport_matches_3d_geometry() {
    // `TestRunner::run` takes an `Fn`, so the counters live in cells.
    let ties = std::cell::Cell::new(0u32);
    let fallbacks = std::cell::Cell::new(0u32);
    let oracle_ties = std::cell::Cell::new(0u32);
    // Coverage counters: a property test that silently skipped every hard case would
    // otherwise pass vacuously.
    let compared = std::cell::Cell::new(0u32);
    let multi_chart = std::cell::Cell::new(0u32);
    let reflected = std::cell::Cell::new(0u32);
    runner(384, 0x5e)
        .run(&(any_point(), any_displacement(200.0)), |(start, d)| {
            let tr = travel(start, d);
            prop_assert!(tr.end.is_canonical(), "end not canonical: {:?}", tr.end);
            prop_assert!(tr.end.u.is_finite() && tr.end.v.is_finite());
            prop_assert!(tr.crossings + tr.reflections <= MAX_CROSSINGS + 1);
            if tr.fallback {
                fallbacks.set(fallbacks.get() + 1);
                return Ok(());
            }

            // Segment lengths sum to the displacement length.
            let total: f64 = tr.segments.iter().map(|s| s.length()).sum();
            prop_assert!(
                (total - d.length()).abs() <= 1e-9 * d.length().max(1.0),
                "swept {total} for a displacement of {}",
                d.length()
            );

            // Every segment stays in its chart, and consecutive segments are in the same
            // or an adjacent chart. A side face's v never exceeds 64: a straight path
            // cannot tunnel through the open bottom.
            for s in &tr.segments {
                for p in [s.from, s.to] {
                    prop_assert!(
                        p.x >= -GEOM_EPS
                            && p.x <= FACE_EXTENT + GEOM_EPS
                            && p.y >= -GEOM_EPS
                            && p.y <= FACE_EXTENT + GEOM_EPS,
                        "segment {s:?} leaves its chart"
                    );
                }
                prop_assert!(s.length() > 0.0, "zero-length segment emitted");
            }
            for w in tr.segments.windows(2) {
                prop_assert!(
                    faces_adjacent(w[0].face, w[1].face),
                    "{:?} and {:?} are not adjacent",
                    w[0].face,
                    w[1].face
                );
                if w[0].face == w[1].face {
                    // A reflection: the kink is continuous inside the chart.
                    prop_assert!((w[0].to - w[1].from).length() <= 1e-9);
                }
            }
            if let Some(first) = tr.segments.first() {
                prop_assert_eq!(first.face, start.face);
                prop_assert!((first.from - start.chart()).length() <= 1e-12);
            }

            // The independent 3D sweep agrees on where the point and its heading ended.
            let s = oracle::sweep(start.face, start.u, start.v, d.x, d.y, GEOM_EPS);
            if tr.ties > 0 {
                ties.set(ties.get() + 1);
            }
            if s.ties > 0 {
                oracle_ties.set(oracle_ties.get() + 1);
            }
            if tr.ties == 0 && s.ties == 0 {
                compared.set(compared.get() + 1);
                if tr.crossings >= 2 {
                    multi_chart.set(multi_chart.get() + 1);
                }
                if tr.reflections >= 1 {
                    reflected.set(reflected.get() + 1);
                }
                prop_assert_eq!(
                    tr.segments.len() as u32,
                    tr.crossings + tr.reflections + 1,
                    "one segment per chart entered plus one per reflection kink"
                );
                prop_assert_eq!(s.faces.len() as u32, tr.crossings + 1, "chart sequence length");
                prop_assert_eq!(s.reflections, tr.reflections, "reflection count");
                let want = oracle::FaceSquare::of(s.face).embed(s.u, s.v);
                prop_assert!(
                    dist3(want, tr.end.embed()) <= 1e-7,
                    "3D endpoint {:?} vs oracle {want:?} ({:?} -> {:?} vs {:?})",
                    tr.end.embed(),
                    start,
                    tr.end,
                    (s.face, s.u, s.v)
                );
                // Heading: the transported chart direction, embedded, must equal the
                // oracle's transported 3D tangent (a unit vector; a unit chart vector is
                // 1/32 of a unit cube edge).
                let dhat = d.normalized().expect("nonzero displacement");
                let moved = tr.end.embed_tangent(tr.map.apply(dhat));
                let moved = [moved[0] * 32.0, moved[1] * 32.0, moved[2] * 32.0];
                prop_assert!(
                    dist3(moved, s.tangent) <= 1e-9,
                    "heading {moved:?} vs oracle {:?}",
                    s.tangent
                );
                // Speed is preserved: the transported displacement has the same length.
                prop_assert!((tr.map.apply(d).length() - d.length()).abs() <= 1e-12);
            }

            // Retracing the transported, reversed displacement returns to the start.
            if tr.ties == 0 {
                let back = travel(tr.end, tr.map.apply(-d));
                if !back.fallback && back.ties == 0 {
                    prop_assert_eq!(back.end.face, start.face, "retrace landed elsewhere");
                    prop_assert!(
                        (back.end.chart() - start.chart()).length() <= 1e-6,
                        "retrace ended at {:?}, started at {:?}",
                        back.end,
                        start
                    );
                    prop_assert_eq!(tr.map.then(back.map), TangentMap::IDENTITY);
                }
            }

            // Large-dt splitting: four quarter steps, transporting the remainder each
            // time, reach the same place as one whole step.
            if tr.ties == 0 {
                let mut p = start;
                let mut m = TangentMap::IDENTITY;
                let mut split_ok = true;
                for _ in 0..4 {
                    let step = travel(p, m.apply(d * 0.25));
                    if step.fallback || step.ties > 0 {
                        split_ok = false;
                        break;
                    }
                    p = step.end;
                    m = m.then(step.map);
                }
                if split_ok {
                    prop_assert_eq!(p.face, tr.end.face, "split sweep changed chart");
                    prop_assert!(
                        (p.chart() - tr.end.chart()).length() <= 1e-6,
                        "split sweep ended at {:?}, whole sweep at {:?}",
                        p,
                        tr.end
                    );
                    prop_assert_eq!(m, tr.map, "split sweep transported differently");
                }
            }
            Ok(())
        })
        .expect("swept transport properties");
    // Ties are astronomically unlikely on random floats; the count is reported so a
    // future change that starts producing them is visible rather than silently skipped.
    assert_eq!(
        fallbacks.get(),
        0,
        "random sweeps hit the forward-progress fallback"
    );
    assert!(
        compared.get() >= 384 * 9 / 10,
        "only {} of 384 cases reached the 3D comparison ({} production ties, {} oracle ties)",
        compared.get(),
        ties.get(),
        oracle_ties.get()
    );
    assert!(multi_chart.get() > 20, "only {} sweeps crossed two or more seams", multi_chart.get());
    assert!(reflected.get() > 5, "only {} sweeps reflected off the rim", reflected.get());
}

/// `travel_into` reuses its buffer and produces exactly what `travel` produces.
#[test]
fn travel_into_matches_travel() {
    let mut buf = Travel::default();
    let mut seed = 0x1234_5678_9abc_def0u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..500 {
        let start = SurfacePoint::new(
            Face::ALL[(next() * 5.0) as usize % 5],
            next() * 64.0,
            next() * 64.0,
        );
        let d = Vec2::from_screen_angle(next() * std::f64::consts::TAU) * (next() * 120.0);
        travel_into(start, d, &mut buf);
        assert_eq!(buf, travel(start, d));
    }
}

// --- vertices --------------------------------------------------------------------------

/// Cube vertices are curvature singularities. Starting exactly on one, and aiming exactly
/// at one, must stay bounded, finite and canonical, resolve by the documented tie rule,
/// and never fall back ("Cover exact vertices and nearby perturbations without hangs,
/// nonfinite values, duplicate neighbors, or unbounded geometric drift").
#[test]
fn exact_vertex_starts_and_aims_are_bounded_and_deterministic() {
    let mut exact_ties = 0;
    let mut fallbacks = 0;

    // (a) Start exactly on the vertex corner of every incident chart (canonicalised, so
    // 64 becomes the largest double below 64) and leave in all eight compass directions.
    for vertex in TOP_VERTICES {
        for (face, cu, cv) in vertex {
            let start = SurfacePoint::new(face, cu, cv).canonicalize();
            assert!(start.is_canonical());
            for dir in compass() {
                let tr = travel(start, dir * 5.0);
                let what = format!("{face:?} corner ({cu},{cv}) dir {dir:?}");
                assert!(tr.end.is_canonical(), "{what}: {:?}", tr.end);
                assert!(tr.end.u.is_finite() && tr.end.v.is_finite(), "{what}");
                assert!(
                    tr.crossings + tr.reflections <= MAX_CROSSINGS,
                    "{what}: {} events",
                    tr.crossings + tr.reflections
                );
                assert!(!tr.fallback, "{what}: fell back");
                let total: f64 = tr.segments.iter().map(|s| s.length()).sum();
                assert!((total - 5.0).abs() <= 1e-9, "{what}: swept {total}");
            }
        }
    }

    // (b) Aim exactly through the vertex: start four pixels in along the chart diagonal
    // and displace eight pixels back along it, so both chart boundaries are reached at
    // exactly t = 0.5 (both quotients are exact binary fractions, so this is a genuine
    // tie, not a near-tie) and the lowest `Edge` index must win.
    for vertex in TOP_VERTICES {
        for (face, cu, cv) in vertex {
            let su = if cu == 0.0 { 1.0 } else { -1.0 };
            let sv = if cv == 0.0 { 1.0 } else { -1.0 };
            let start = SurfacePoint::new(face, cu + 4.0 * su, cv + 4.0 * sv);
            let d = Vec2::new(-8.0 * su, -8.0 * sv);
            let tr = travel(start, d);
            let what = format!("{face:?} aimed at ({cu},{cv})");
            assert!(tr.end.is_canonical(), "{what}: {:?}", tr.end);
            assert!(!tr.fallback, "{what}: fell back");
            assert!(tr.ties >= 1, "{what}: an exact vertex hit reported no tie");
            assert!(
                tr.crossings + tr.reflections <= MAX_CROSSINGS,
                "{what}: too many events"
            );
            let total: f64 = tr.segments.iter().map(|s| s.length()).sum();
            assert!((total - d.length()).abs() <= 1e-9, "{what}: swept {total}");
            exact_ties += 1;

            // (c) The same path nudged off the vertex by 0.02 px of displacement (seven
            // orders of magnitude above GEOM_EPS, so the two boundary hits are cleanly
            // ordered) resolves without any tie and without a fallback.
            for skew in [-0.02f64, 0.02] {
                let d = Vec2::new(-8.0 * su, -(8.0 + skew) * sv);
                let tr = travel(start, d);
                assert!(!tr.fallback, "{what} skew {skew}: fell back");
                assert_eq!(tr.ties, 0, "{what} skew {skew}: reported a tie");
                assert!(tr.end.is_canonical(), "{what} skew {skew}: {:?}", tr.end);
                let total: f64 = tr.segments.iter().map(|s| s.length()).sum();
                assert!((total - d.length()).abs() <= 1e-9, "{what} skew {skew}");
                if tr.fallback {
                    fallbacks += 1;
                }
            }
        }
    }
    assert_eq!(exact_ties, 12, "four vertices x three incident charts");
    assert_eq!(fallbacks, 0, "the perturbed cases must never fall back");
}

// --- the lower rim corners -------------------------------------------------------------

/// "At a lower side corner the unfolded boundary is straight; test a step that both
/// crosses a vertical seam and reflects, including an exact tie"
/// (design/surface-topology.md).
#[test]
fn lower_side_corner_crosses_and_reflects() {
    // Front (63.5, 63.5) + (1, 1): u and v reach 64 together, the tie rule takes
    // Edge::Right (1) before Edge::Bottom (2), the point enters Right at (0, 64) with
    // half a pixel left in each axis, immediately reflects off the rim, and finishes at
    // Right (0.5, 63.5).
    let tr = travel(SurfacePoint::new(Face::Front, 63.5, 63.5), Vec2::new(1.0, 1.0));
    assert_point(tr.end, Face::Right, 0.5, 63.5, 1e-12, "lower corner step");
    assert_eq!(tr.crossings, 1, "one seam crossing");
    assert_eq!(tr.reflections, 1, "one rim reflection");
    assert!(tr.ties >= 1, "the simultaneous hit is a tie");
    assert!(!tr.fallback);
    assert_eq!(tr.map, TangentMap::REFLECT_Y, "a straight seam then a rim reflection");
    let total: f64 = tr.segments.iter().map(|s| s.length()).sum();
    assert!((total - Vec2::new(1.0, 1.0).length()).abs() <= 1e-12, "swept {total}");

    // The same geometry at a different scale: Front (63.9, 63.9) + (0.2, 0.2).
    let tr = travel(SurfacePoint::new(Face::Front, 63.9, 63.9), Vec2::new(0.2, 0.2));
    assert_point(tr.end, Face::Right, 0.1, 63.9, 1e-9, "exact tie variant");
    assert_eq!((tr.crossings, tr.reflections), (1, 1));
    assert!(tr.ties >= 1);
    assert!(!tr.fallback);
}

/// A straight path never tunnels out through the open bottom, from anywhere, in any
/// direction, at any length: no segment endpoint on a side face ever passes v = 64.
#[test]
fn straight_paths_never_tunnel_through_the_open_bottom() {
    let mut buf = Travel::default();
    let mut seed = 0x0f0f_2718_2818_2845u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut reflections = 0u32;
    for _ in 0..4000 {
        let face = Face::ALL[(next() * 5.0) as usize % 5];
        // Bias the start toward the lower rim, where tunnelling would show up.
        let start = SurfacePoint::new(face, next() * 64.0, 48.0 + next() * 16.0);
        let d = Vec2::from_screen_angle(next() * std::f64::consts::TAU) * (next() * 90.0);
        travel_into(start, d, &mut buf);
        reflections += buf.reflections;
        assert!(buf.end.is_canonical(), "{start:?} + {d:?} -> {:?}", buf.end);
        for s in &buf.segments {
            for p in [s.from, s.to] {
                assert!(
                    p.y <= FACE_EXTENT + 1e-9,
                    "{start:?} + {d:?}: segment {s:?} passed the rim"
                );
                assert!(p.y >= -1e-9 && p.x >= -1e-9 && p.x <= FACE_EXTENT + 1e-9);
            }
        }
    }
    assert!(reflections > 100, "the fixture must actually exercise the rim");
}
