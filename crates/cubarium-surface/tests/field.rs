//! The 16x16-cells-per-face scalar field graph: adjacency, conservative diffusion,
//! nonnegativity, deposits, and the symmetry checks `design/surface-topology.md` requires
//! ("Constant fields stay constant; pure diffusion preserves total mass and
//! nonnegativity, including at seams, top corners, and the rim", "Check diffusion
//! equivariance under cube rotations that preserve the open bottom", "The field random
//! walk chooses one of four directions and stays put at an open edge").
//!
//! The cell permutations used for the equivariance checks are derived from the 3D
//! embedding (`face_frame` / `SurfacePoint::embed`), never from a seam table, so a wrong
//! seam table cannot make a rotated run agree with itself.

use cubarium_surface::{
    CELL_COUNT, CELL_PIXELS, CELLS_PER_FACE_EDGE, CellId, Edge, Face, FieldGraph, ScalarField,
    SurfacePoint, cell_of, cross_seam, deposit, diffuse, face_frame,
};

const N: usize = CELL_COUNT;
const SIDE_FACES: [Face; 4] = [Face::Front, Face::Right, Face::Back, Face::Left];

// --- 3D helpers for the rotation permutations -------------------------------------------

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A yaw of 90 degrees about +Y: Front -> Right -> Back -> Left -> Front, Top spins in
/// place. It preserves the open bottom, so diffusion must commute with it.
fn yaw(p: [f64; 3]) -> [f64; 3] {
    [p[2], p[1], -p[0]]
}

/// A 120 degree rotation about the (1, 1, 1) vertex axis: Front -> Right -> Top -> Front.
/// It fixes the Front/Right/Top vertex and is a symmetry of the cone around it (but not of
/// the whole surface, because it moves Top into a side face).
fn vertex_spin(p: [f64; 3]) -> [f64; 3] {
    [p[2], p[0], p[1]]
}

/// The cell containing the image of `cell`'s centre under a rigid rotation of the cube.
/// The face is identified by which face plane the rotated point lands in, read off
/// `face_frame`, and the chart coordinates by projecting onto that frame's tangents.
/// `None` when the image lands on the missing bottom face, which happens for the vertex
/// spin (it is only a symmetry of the cone around its vertex, not of the whole surface).
fn rotate_cell(cell: CellId, rot: fn([f64; 3]) -> [f64; 3]) -> Option<CellId> {
    let q = rot(cell.center().embed());
    for face in Face::ALL {
        let f = face_frame(face);
        if dot(f.normal, q) > 1.0 - 1e-9 {
            let a = dot(sub(q, f.center), f.tangent_u);
            let b = dot(sub(q, f.center), f.tangent_v);
            return Some(cell_of(&SurfacePoint::new(
                face,
                (a + 1.0) * 32.0,
                (b + 1.0) * 32.0,
            )));
        }
    }
    // The only direction with no face is -Y.
    assert!(q[1] < -1.0 + 1e-9, "rotated cell centre {q:?} is not on any face");
    None
}

fn permutation(rot: fn([f64; 3]) -> [f64; 3]) -> Vec<Option<CellId>> {
    let perm: Vec<Option<CellId>> = CellId::all().map(|c| rotate_cell(c, rot)).collect();
    let mut seen = vec![false; N];
    for c in perm.iter().flatten() {
        assert!(!seen[c.index()], "cell permutation is not injective");
        seen[c.index()] = true;
    }
    perm
}

/// The along-edge cell of `face` on `edge` at index `k`.
fn edge_cell(face: Face, edge: Edge, k: u8) -> CellId {
    let last = (CELLS_PER_FACE_EDGE - 1) as u8;
    match edge {
        Edge::Top => CellId::new(face, k, 0),
        Edge::Right => CellId::new(face, last, k),
        Edge::Bottom => CellId::new(face, k, last),
        Edge::Left => CellId::new(face, 0, k),
    }
}

fn is_rim_cell(c: CellId) -> bool {
    c.face() != Face::Top && c.cy() == (CELLS_PER_FACE_EDGE - 1) as u8
}

// --- the graph ---------------------------------------------------------------------------

/// "the relation is reciprocal, every cell has degree 4 except the 64 rim cells with
/// degree 3, and there are exactly 2,528 undirected edges (2,400 within charts, 128 across
/// seams)" (`src/field.rs`).
#[test]
fn graph_is_reciprocal_with_the_documented_degrees_and_edge_count() {
    let g = FieldGraph::new();
    assert_eq!(N, 1280, "five faces of 16x16 cells");
    assert_eq!(CELL_PIXELS, 4.0);

    let mut rim = 0;
    let mut undirected = 0;
    for c in CellId::all() {
        let degree = g.degree(c);
        if is_rim_cell(c) {
            rim += 1;
            assert_eq!(degree, 3, "{c:?} on the open rim");
            assert_eq!(g.neighbor(c, Edge::Bottom), None, "{c:?} has flux through the rim");
        } else {
            assert_eq!(degree, 4, "{c:?}");
        }
        let mut seen: Vec<CellId> = Vec::new();
        for e in Edge::ALL {
            let Some(n) = g.neighbor(c, e) else { continue };
            undirected += 1;
            assert_ne!(n, c, "{c:?} is its own neighbour across {e:?}");
            assert!(!seen.contains(&n), "{c:?} reaches {n:?} twice");
            seen.push(n);
            assert!(
                Edge::ALL.into_iter().any(|e2| g.neighbor(n, e2) == Some(c)),
                "{c:?} -> {n:?} across {e:?} is not reciprocated"
            );
        }
    }
    assert_eq!(rim, 64, "16 cells on each of the four open edges");
    assert_eq!(undirected, 2 * 2528, "each undirected edge counted from both ends");

    let edges = g.edges();
    assert_eq!(edges.len(), 2528, "2400 inside charts + 128 across seams");
    assert!(edges.windows(2).all(|w| w[0] < w[1]), "edges are sorted and unique");
    for &(a, b) in edges {
        assert!(a < b, "edges are stored with a < b");
        assert!(
            Edge::ALL.into_iter().any(|e| g.neighbor(a, e) == Some(b)),
            "{a:?}-{b:?} is not an adjacency"
        );
    }
    // Counting the two populations separately: a within-chart edge joins two cells of the
    // same face, and there must be 128 seam edges (8 seams x 16 cells).
    let seam_edges = edges.iter().filter(|(a, b)| a.face() != b.face()).count();
    assert_eq!(seam_edges, 128);
    assert_eq!(edges.len() - seam_edges, 2400);
}

/// "the neighbor of cell `(face, cx, cy)` across `edge` ... is the cell containing the
/// pixel `cross_seam(face, edge, t)` returns for `t = 4k`. Reversal at the Right/Top and
/// Back/Top seams therefore comes out of `cross_seam` (`63 - t` at pixel resolution maps
/// cell `k` to cell `15 - k`)" (`src/field.rs`).
#[test]
fn seam_neighbours_come_out_of_cross_seam_at_cell_resolution() {
    let g = FieldGraph::new();
    for face in Face::ALL {
        for edge in Edge::ALL {
            if face.neighbor(edge).is_none() {
                continue;
            }
            for k in 0..16u8 {
                let cell = edge_cell(face, edge, k);
                let (nf, nx, ny, _) = cross_seam(face, edge, k * 4).expect("connected");
                let want = cell_of(&SurfacePoint::new(
                    nf,
                    f64::from(nx) + 0.5,
                    f64::from(ny) + 0.5,
                ));
                assert_eq!(
                    g.neighbor(cell, edge),
                    Some(want),
                    "{face:?}/{edge:?} cell {k}"
                );
            }
        }
    }
    // Spelled out for the four seams around Top: the two reversed ones map k to 15 - k.
    for k in 0..16u8 {
        assert_eq!(
            g.neighbor(edge_cell(Face::Right, Edge::Top, k), Edge::Top),
            Some(CellId::new(Face::Top, 15, 15 - k)),
            "Right/Top is reversed"
        );
        assert_eq!(
            g.neighbor(edge_cell(Face::Back, Edge::Top, k), Edge::Top),
            Some(CellId::new(Face::Top, 15 - k, 0)),
            "Back/Top is reversed"
        );
        assert_eq!(
            g.neighbor(edge_cell(Face::Front, Edge::Top, k), Edge::Top),
            Some(CellId::new(Face::Top, k, 15)),
            "Front/Top runs straight through"
        );
        assert_eq!(
            g.neighbor(edge_cell(Face::Left, Edge::Top, k), Edge::Top),
            Some(CellId::new(Face::Top, 0, k)),
            "Left/Top runs straight through"
        );
        // The four vertical seams join equal rows with no twist and no reversal.
        assert_eq!(
            g.neighbor(edge_cell(Face::Front, Edge::Right, k), Edge::Right),
            Some(CellId::new(Face::Right, 0, k))
        );
        assert_eq!(
            g.neighbor(edge_cell(Face::Back, Edge::Right, k), Edge::Right),
            Some(CellId::new(Face::Left, 0, k))
        );
    }
}

// --- diffusion ---------------------------------------------------------------------------

#[test]
fn a_constant_field_stays_bit_identical() {
    let g = FieldGraph::new();
    let mut f = ScalarField::constant(1.0);
    let mut scratch = ScalarField::zeros();
    for _ in 0..1000 {
        let substeps = diffuse(&mut f, &mut scratch, &g, 0.9);
        assert_eq!(substeps, 4, "rate 0.9 needs ceil(0.9 / 0.25) = 4 substeps");
    }
    assert_eq!(*f.values, [1.0f64; CELL_COUNT], "a constant field drifted");
}

#[test]
fn diffusion_conserves_mass_and_stays_nonnegative() {
    let g = FieldGraph::new();
    let mut scratch = ScalarField::zeros();

    // A delta at a cell touching a top vertex, one on the open rim, and one mid-face.
    let sources = [
        CellId::new(Face::Front, 15, 0),
        CellId::new(Face::Right, 7, 15),
        CellId::new(Face::Top, 8, 8),
        CellId::new(Face::Top, 0, 0),
    ];
    for source in sources {
        let mut f = ScalarField::zeros();
        f.set(source, 1.0);
        for step in 0..2000 {
            let substeps = diffuse(&mut f, &mut scratch, &g, 0.9);
            assert_eq!(substeps, 4);
            assert!(f.is_finite(), "{source:?} went non-finite at step {step}");
            assert!(
                f.is_nonnegative(),
                "{source:?} went negative at step {step} (min {})",
                f.min()
            );
        }
        assert!(
            (f.total() - 1.0).abs() <= 1e-9,
            "{source:?}: total drifted to {}",
            f.total()
        );
        // 2000 steps at rate 0.9 is far past mixing: nothing is still piled up. (The
        // slowest mode of this graph decays by roughly exp(-0.005) per step, so the
        // remaining deviation is many orders of magnitude below one cell's share.)
        let uniform = 1.0 / CELL_COUNT as f64;
        assert!(
            f.max() < 2.0 * uniform,
            "{source:?} did not mix: max {} vs uniform {uniform}",
            f.max()
        );
    }

    // A nonpositive or non-finite rate is a no-op.
    let mut f = ScalarField::zeros();
    f.set(sources[0], 1.0);
    let before = f.clone();
    assert_eq!(diffuse(&mut f, &mut scratch, &g, 0.0), 0);
    assert_eq!(diffuse(&mut f, &mut scratch, &g, -1.0), 0);
    assert_eq!(diffuse(&mut f, &mut scratch, &g, f64::NAN), 0);
    assert_eq!(f, before, "a nonpositive rate changed the field");
}

/// E1, algebraically: the cell random walk that "chooses one of four directions and stays
/// put at an open edge" has a symmetric, row-stochastic transition matrix, so it is doubly
/// stochastic and the uniform distribution is stationary; power iteration from a delta
/// confirms it converges there.
#[test]
fn the_field_random_walk_is_doubly_stochastic_and_mixes_to_uniform() {
    let g = FieldGraph::new();
    let mut p = vec![0.0f64; N * N];
    for c in CellId::all() {
        for e in Edge::ALL {
            let target = g.neighbor(c, e).unwrap_or(c);
            p[c.index() * N + target.index()] += 0.25;
        }
    }
    for i in 0..N {
        let row: f64 = p[i * N..(i + 1) * N].iter().sum();
        assert!((row - 1.0).abs() <= 1e-15, "row {i} sums to {row}");
        for j in 0..N {
            assert_eq!(p[i * N + j], p[j * N + i], "P is not symmetric at ({i}, {j})");
        }
    }
    // Symmetric + row-stochastic implies column-stochastic, i.e. uniform is stationary.
    for j in 0..N {
        let col: f64 = (0..N).map(|i| p[i * N + j]).sum();
        assert!((col - 1.0).abs() <= 1e-15, "column {j} sums to {col}");
    }
    drop(p);

    // Power iteration from a delta at a rim cell (the most biased start available).
    let mut cur = vec![0.0f64; N];
    cur[CellId::new(Face::Front, 0, 15).index()] = 1.0;
    let mut next = vec![0.0f64; N];
    for _ in 0..20_000 {
        next.iter_mut().for_each(|x| *x = 0.0);
        for c in CellId::all() {
            let share = cur[c.index()] * 0.25;
            if share == 0.0 {
                continue;
            }
            for e in Edge::ALL {
                let target = g.neighbor(c, e).unwrap_or(c);
                next[target.index()] += share;
            }
        }
        std::mem::swap(&mut cur, &mut next);
    }
    let uniform = 1.0 / N as f64;
    let worst = cur
        .iter()
        .map(|x| (x - uniform).abs())
        .fold(0.0f64, f64::max);
    assert!(
        worst <= 1e-6,
        "the walk did not reach uniform occupancy: worst deviation {worst} (uniform {uniform})"
    );
    assert!((cur.iter().sum::<f64>() - 1.0).abs() <= 1e-9, "probability leaked");
}

/// "Check diffusion equivariance under cube rotations that preserve the open bottom."
#[test]
fn diffusion_commutes_with_the_yaw_rotations() {
    let g = FieldGraph::new();
    let perm = permutation(yaw);
    // The permutation is the documented one: Front -> Right -> Back -> Left and Top spins.
    let yawed = |c: CellId| rotate_cell(c, yaw).expect("yaw keeps every cell on the surface");
    assert_eq!(yawed(CellId::new(Face::Front, 3, 5)).face(), Face::Right);
    assert_eq!(yawed(CellId::new(Face::Right, 3, 5)).face(), Face::Back);
    assert_eq!(yawed(CellId::new(Face::Back, 3, 5)).face(), Face::Left);
    assert_eq!(yawed(CellId::new(Face::Left, 3, 5)).face(), Face::Front);
    assert_eq!(yawed(CellId::new(Face::Top, 3, 5)).face(), Face::Top);
    assert!(perm.iter().all(|c| c.is_some()), "a yaw maps the surface onto itself");

    let sources = [
        CellId::new(Face::Front, 6, 9),
        CellId::new(Face::Front, 15, 0),
        CellId::new(Face::Top, 15, 15),
        CellId::new(Face::Left, 2, 15),
    ];
    for source in sources {
        let mut base = ScalarField::zeros();
        base.set(source, 1.0);
        let mut scratch = ScalarField::zeros();
        for _ in 0..30 {
            diffuse(&mut base, &mut scratch, &g, 0.2);
        }
        // Apply the yaw one, two and three times.
        let mut image = source;
        let mut rotated_base = base.clone();
        for turn in 1..=3 {
            image = perm[image.index()].expect("yaw stays on the surface");
            let mut rotated_input = ScalarField::zeros();
            rotated_input.set(image, 1.0);
            let mut scratch = ScalarField::zeros();
            for _ in 0..30 {
                diffuse(&mut rotated_input, &mut scratch, &g, 0.2);
            }
            // Rotating the already-diffused field must give the same thing.
            let mut want = ScalarField::zeros();
            for c in CellId::all() {
                want.set(perm[c.index()].expect("yaw stays on the surface"), rotated_base.get(c));
            }
            rotated_base = want.clone();
            for c in CellId::all() {
                assert!(
                    (rotated_input.get(c) - want.get(c)).abs() <= 1e-12,
                    "turn {turn} from {source:?}: cell {c:?} has {} but the rotated run has {}",
                    rotated_input.get(c),
                    want.get(c)
                );
            }
        }
    }
}

/// "A single vertex-adjacent source is not itself three-way symmetric; compare rotated
/// source runs, or equal deposits on all three incident cells within a local symmetric
/// neighborhood before the lower boundary influences it."
#[test]
fn a_vertex_neighbourhood_is_three_fold_symmetric() {
    let g = FieldGraph::new();
    let perm = permutation(vertex_spin);
    // The three cells meeting the (1, 1, 1) vertex, cycled by the 120 degree spin.
    let front = CellId::new(Face::Front, 15, 0);
    let right = CellId::new(Face::Right, 0, 0);
    let top = CellId::new(Face::Top, 15, 15);
    assert_eq!(perm[front.index()], Some(right));
    assert_eq!(perm[right.index()], Some(top));
    assert_eq!(perm[top.index()], Some(front));

    let mut f = ScalarField::zeros();
    for c in [front, right, top] {
        f.add(c, 1.0);
    }
    let mut scratch = ScalarField::zeros();
    // Eight substeps at the stable rate: influence spreads at most eight cells, which is
    // still inside the three-fold cone (the rim and the far seams are 16 cells away).
    for _ in 0..8 {
        assert_eq!(diffuse(&mut f, &mut scratch, &g, 0.25), 1);
    }
    assert!((f.total() - 3.0).abs() <= 1e-12);

    let near = |c: CellId| -> Option<u8> {
        let (cx, cy) = (c.cx(), c.cy());
        match c.face() {
            Face::Front => Some((15 - cx).max(cy)),
            Face::Right => Some(cx.max(cy)),
            Face::Top => Some((15 - cx).max(15 - cy)),
            _ => None,
        }
    };
    let mut checked = 0;
    for c in CellId::all() {
        let Some(d) = near(c) else { continue };
        if d > 6 {
            continue;
        }
        let image = perm[c.index()].expect("the cone around the vertex maps onto itself");
        assert!(
            (f.get(c) - f.get(image)).abs() <= 1e-12,
            "{c:?} has {} but its 120 degree image {image:?} has {}",
            f.get(c),
            f.get(image)
        );
        checked += 1;
    }
    assert_eq!(checked, 3 * 7 * 7, "three faces x a 7x7 corner block");
}

// --- deposits -----------------------------------------------------------------------------

/// "A footprint clipped by the rim or covering a vertex deposits the same total as one in
/// the middle of a face" (`src/field.rs`).
#[test]
fn deposit_conserves_its_amount_everywhere() {
    let cases = [
        ("mid face", SurfacePoint::new(Face::Front, 32.0, 32.0)),
        ("on a vertical seam", SurfacePoint::new(Face::Front, 63.5, 30.0)),
        ("on the Front/Top seam", SurfacePoint::new(Face::Front, 30.0, 0.5)),
        ("on the twisted Right/Top seam", SurfacePoint::new(Face::Right, 30.0, 0.5)),
        ("straddling a top vertex", SurfacePoint::new(Face::Front, 62.0, 2.0)),
        ("on Top at a vertex", SurfacePoint::new(Face::Top, 63.0, 63.0)),
        ("clipped at the rim", SurfacePoint::new(Face::Front, 30.0, 62.0)),
        ("in a rim corner", SurfacePoint::new(Face::Left, 63.0, 63.0)),
    ];
    for (what, center) in cases {
        let mut f = ScalarField::zeros();
        let touched = deposit(&mut f, center, 10.0, 3.0);
        assert!(touched > 0, "{what}: nothing touched");
        assert!(f.is_nonnegative(), "{what}: negative weight");
        assert!(
            (f.total() - 3.0).abs() <= 1e-9,
            "{what}: deposited {} instead of 3",
            f.total()
        );
        // `touched` counts the cells inside the footprint; a cell whose centre sits at
        // exactly `radius` is inside with weight zero, so only the inequality is implied.
        assert!(
            f.values.iter().filter(|x| **x > 0.0).count() <= touched && touched <= CELL_COUNT,
            "{what}: {touched} cells reported"
        );
    }

    // A radius below half a cell still lands everything in the containing cell.
    let mut f = ScalarField::zeros();
    let center = SurfacePoint::new(Face::Back, 21.0, 45.0);
    let touched = deposit(&mut f, center, 0.25, 2.0);
    assert_eq!(touched, 1);
    assert!((f.get(cell_of(&center)) - 2.0).abs() <= 1e-12);
}

/// The same footprint moved across a flat seam by a whole number of cells must produce the
/// same weights: "the same event deposits the same total near a seam or rim".
#[test]
fn a_footprint_carried_across_a_flat_seam_keeps_its_weights() {
    let mut here = ScalarField::zeros();
    // Front (34, 34) sits on a cell centre; adding 32 pixels of u (eight whole cells)
    // carries it across the flat Front/Right seam to Right (2, 34).
    deposit(&mut here, SurfacePoint::new(Face::Front, 34.0, 34.0), 9.0, 1.0);
    let mut there = ScalarField::zeros();
    deposit(&mut there, SurfacePoint::new(Face::Right, 2.0, 34.0), 9.0, 1.0);

    let mut a: Vec<f64> = here.values.iter().copied().filter(|x| *x > 0.0).collect();
    let mut b: Vec<f64> = there.values.iter().copied().filter(|x| *x > 0.0).collect();
    assert_eq!(a.len(), b.len(), "different number of cells touched");
    a.sort_by(|x, y| x.total_cmp(y));
    b.sort_by(|x, y| x.total_cmp(y));
    for (x, y) in a.iter().zip(&b) {
        assert!((x - y).abs() <= 1e-9, "weight {x} became {y} across the seam");
    }
    assert!((here.total() - there.total()).abs() <= 1e-9);
}

/// `cell_of` and `CellId` agree with the documented indexing.
#[test]
fn cell_indexing_matches_the_documented_layout() {
    for face in Face::ALL {
        for cy in 0..16u8 {
            for cx in 0..16u8 {
                let c = CellId::new(face, cx, cy);
                assert_eq!(c.index(), face.index() * 256 + usize::from(cy) * 16 + usize::from(cx));
                assert_eq!((c.face(), c.cx(), c.cy()), (face, cx, cy));
                let centre = c.center();
                assert_eq!(centre.u, f64::from(cx) * CELL_PIXELS + 2.0);
                assert_eq!(centre.v, f64::from(cy) * CELL_PIXELS + 2.0);
                assert_eq!(cell_of(&centre), c);
                // Every pixel of the cell maps back to it.
                for dy in 0..4u8 {
                    for dx in 0..4u8 {
                        let p = SurfacePoint::pixel_center(face, cx * 4 + dx, cy * 4 + dy);
                        assert_eq!(cell_of(&p), c);
                    }
                }
            }
        }
    }
    assert_eq!(CellId::all().count(), N);
    for f in SIDE_FACES {
        assert!(is_rim_cell(CellId::new(f, 5, 15)));
    }
}
