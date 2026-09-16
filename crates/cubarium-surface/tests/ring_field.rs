//! FW-6: the ring's cell graph and `Topology::validate`, written from
//! `design/flat-world-plan-2026-09-16.md` §2 and §5 and the FW-1 freeze.
//!
//! The plan's numbers, verbatim: "At S = 1, 320×180 → 80×45 = 3,600 cells. **Every row is
//! a ring**, so the horizontal wrap adds one edge per row: `80·45 + 80·44 = 7,120`
//! undirected edges. Degrees are simpler than a rectangle's — **there are no corners**:
//! 3,440 interior cells of degree 4, and the 80 + 80 cells of the top and bottom rows at
//! degree 3 … `downhill` … `None` on the top row, else `(cx, cy+1)`."

use cubarium_surface::{
    CellId, Edge, Face, FieldGraph, ScalarField, Scale, SurfacePoint, Topology, TopologyError,
    cell_of, deposit, diffuse,
};

const COLS: u16 = 80;
const ROWS: u16 = 45;

fn ring() -> Topology {
    Topology::Ring { w: 320, h: 180 }
}

fn cell(cx: u16, cy: u16) -> CellId {
    CellId::new(ring(), Scale::ONE, Face::Front, cx, cy)
}

// ---------------------------------------------------------------------------
// The cells
// ---------------------------------------------------------------------------

#[test]
fn a_320_by_180_ring_has_80_by_45_cells() {
    let topo = ring();
    assert_eq!(topo.cells(Scale::ONE, Face::Front), (COLS, ROWS), "cells per chart");
    assert_eq!(topo.cell_count(Scale::ONE), 3600, "80 × 45");

    let all: Vec<CellId> = CellId::all(topo, Scale::ONE).collect();
    assert_eq!(all.len(), 3600, "CellId::all yields every cell");
    let mut seen = vec![false; 3600];
    for c in &all {
        assert!(!seen[c.index()], "cell index {} yielded twice", c.index());
        seen[c.index()] = true;
    }

    let field = ScalarField::zeros(topo, Scale::ONE);
    assert_eq!(field.len(), 3600, "a scalar field is sized from the topology");
}

/// At S = 2 the cell is 8 px, so the 640×360 ring has the *same* 3,600 cells: the world
/// gets bigger in pixels, not in cells. (§2: "cell size for a ring world: 4·S px".)
#[test]
fn the_double_scale_ring_keeps_the_same_cell_count() {
    let topo = Topology::Ring { w: 640, h: 360 };
    let s2 = Scale::new(2.0);
    assert_eq!(s2.cell_pixels(), 8.0, "4·S");
    assert_eq!(topo.cells(s2, Face::Front), (COLS, ROWS));
    assert_eq!(topo.cell_count(s2), 3600);
}

#[test]
fn cell_centres_are_cell_middles_and_cell_of_inverts_them() {
    let topo = ring();
    for cy in 0..ROWS {
        for cx in 0..COLS {
            let c = cell(cx, cy);
            assert_eq!(c.face(topo, Scale::ONE), Face::Front, "a ring is one chart");
            assert_eq!((c.cx(topo, Scale::ONE), c.cy(topo, Scale::ONE)), (cx, cy));
            let p = c.center(topo, Scale::ONE);
            assert_eq!(p.u, f64::from(cx) * 4.0 + 2.0, "centre u of ({cx}, {cy})");
            assert_eq!(p.v, f64::from(cy) * 4.0 + 2.0, "centre v of ({cx}, {cy})");
            assert_eq!(cell_of(topo, Scale::ONE, &p), c, "cell_of(centre) round trip");
        }
    }
}

// ---------------------------------------------------------------------------
// The graph
// ---------------------------------------------------------------------------

#[test]
fn every_row_is_a_ring_so_the_graph_has_7120_edges() {
    let g = FieldGraph::new(ring(), Scale::ONE);
    assert_eq!(g.topology(), ring());
    assert_eq!(g.scale(), Scale::ONE);
    assert_eq!(g.cell_count(), 3600);
    // 80·45 horizontal (one per row *including* the wrap) + 80·44 vertical.
    assert_eq!(g.edges().len(), 7120, "undirected edges");
}

#[test]
fn a_ring_has_no_corner_cells() {
    let g = FieldGraph::new(ring(), Scale::ONE);
    let mut by_degree = [0usize; 5];
    for c in CellId::all(ring(), Scale::ONE) {
        by_degree[g.degree(c)] += 1;
    }
    assert_eq!(by_degree[4], 3440, "interior cells of degree 4");
    assert_eq!(by_degree[3], 160, "the 80 + 80 cells of the top and bottom rows");
    assert_eq!(by_degree[2], 0, "the previous revision's degree-2 corner case is deleted");
    assert_eq!(by_degree[1], 0);
    assert_eq!(by_degree[0], 0);
    // and the degree-3 cells are exactly the two rim rows.
    for cy in 0..ROWS {
        let expected = if cy == 0 || cy == ROWS - 1 { 3 } else { 4 };
        for cx in 0..COLS {
            assert_eq!(g.degree(cell(cx, cy)), expected, "degree of ({cx}, {cy})");
        }
    }
    // Twice the edge count, as a cross-check on both numbers.
    let total_degree: usize = CellId::all(ring(), Scale::ONE).map(|c| g.degree(c)).sum();
    assert_eq!(total_degree, 2 * 7120);
}

#[test]
fn the_wrap_joins_the_first_and_last_column() {
    let g = FieldGraph::new(ring(), Scale::ONE);
    for cy in 0..ROWS {
        assert_eq!(
            g.neighbor(cell(0, cy), Edge::Left),
            Some(cell(COLS - 1, cy)),
            "left of column 0 in row {cy}"
        );
        assert_eq!(
            g.neighbor(cell(COLS - 1, cy), Edge::Right),
            Some(cell(0, cy)),
            "right of the last column in row {cy}"
        );
    }
}

#[test]
fn both_horizontal_rims_are_open() {
    let g = FieldGraph::new(ring(), Scale::ONE);
    for cx in 0..COLS {
        assert_eq!(g.neighbor(cell(cx, 0), Edge::Top), None, "above the top row at {cx}");
        assert_eq!(
            g.neighbor(cell(cx, ROWS - 1), Edge::Bottom),
            None,
            "below the bottom row at {cx}"
        );
        assert_eq!(g.neighbor(cell(cx, 0), Edge::Bottom), Some(cell(cx, 1)));
        assert_eq!(g.neighbor(cell(cx, ROWS - 1), Edge::Top), Some(cell(cx, ROWS - 2)));
    }
}

/// §5's decided rule: `None` on the top row (the canopy holds its water and detritus),
/// otherwise the neighbour at `(cx, cy + 1)` — which the bottom row does not have.
#[test]
fn downhill_is_none_on_the_top_row_and_one_step_down_elsewhere() {
    let g = FieldGraph::new(ring(), Scale::ONE);
    for cx in 0..COLS {
        assert_eq!(g.downhill(cell(cx, 0)), None, "the canopy row never drains ({cx})");
        for cy in 1..ROWS - 1 {
            assert_eq!(
                g.downhill(cell(cx, cy)),
                Some(cell(cx, cy + 1)),
                "downhill of ({cx}, {cy})"
            );
        }
        assert_eq!(
            g.downhill(cell(cx, ROWS - 1)),
            None,
            "the bottom row has nothing below it ({cx})"
        );
    }
}

// ---------------------------------------------------------------------------
// Flux
// ---------------------------------------------------------------------------

#[test]
fn diffusion_is_conservative_and_crosses_the_wrap() {
    let topo = ring();
    let g = FieldGraph::new(topo, Scale::ONE);
    let mut f = ScalarField::zeros(topo, Scale::ONE);
    let mut scratch = ScalarField::zeros(topo, Scale::ONE);
    f.set(cell(0, 22), 1.0);
    let before = f.total();

    diffuse(&mut f, &mut scratch, &g, 0.2);
    assert!((f.total() - before).abs() <= 1e-12, "mass after one step: {}", f.total());
    assert!(
        f.get(cell(COLS - 1, 22)) > 0.0,
        "the only path from column 0 to column 79 is the wrap edge"
    );
    assert!(f.get(cell(1, 22)) > 0.0, "and the ordinary neighbour got the same share");
    assert!(
        (f.get(cell(COLS - 1, 22)) - f.get(cell(1, 22))).abs() <= 1e-15,
        "the wrap edge is an ordinary edge: {} vs {}",
        f.get(cell(COLS - 1, 22)),
        f.get(cell(1, 22))
    );

    for _ in 0..200 {
        diffuse(&mut f, &mut scratch, &g, 0.2);
    }
    assert!((f.total() - before).abs() <= 1e-9, "mass after 201 steps: {}", f.total());
    assert!(f.is_nonnegative(), "diffusion never goes negative");
}

/// No flux crosses a rim: a field that is uniform on the top row stays bounded by its own
/// extremes, and the total is unchanged — the rims lose nothing.
#[test]
fn no_flux_leaves_through_a_rim() {
    let topo = ring();
    let g = FieldGraph::new(topo, Scale::ONE);
    let mut f = ScalarField::zeros(topo, Scale::ONE);
    let mut scratch = ScalarField::zeros(topo, Scale::ONE);
    for cx in 0..COLS {
        f.set(cell(cx, 0), 1.0);
        f.set(cell(cx, ROWS - 1), 1.0);
    }
    let before = f.total();
    for _ in 0..50 {
        diffuse(&mut f, &mut scratch, &g, 0.24);
    }
    assert!((f.total() - before).abs() <= 1e-9, "mass: {} vs {before}", f.total());
    assert!(f.max() <= 1.0 + 1e-12, "no cell exceeds the initial maximum");
    assert!(f.min() >= 0.0);
}

/// §2/`deposit`: a footprint straddling the wrap deposits the same total as one in the
/// middle of the chart, and it really does reach around.
#[test]
fn a_deposit_straddling_the_wrap_keeps_its_whole_amount() {
    let topo = ring();
    let mut f = ScalarField::zeros(topo, Scale::ONE);
    let centre = SurfacePoint::new(Face::Front, 1.0, 90.0);
    let touched = deposit(topo, Scale::ONE, &mut f, centre, 9.0, 5.0);
    assert!(touched > 0, "the footprint touches cells");
    assert!((f.total() - 5.0).abs() <= 1e-9, "deposited total: {}", f.total());
    let west: f64 = (0..ROWS).map(|cy| f.get(cell(COLS - 1, cy))).sum();
    assert!(west > 0.0, "a footprint at u = 1 reaches around the wrap into column 79");

    // The same deposit in the middle of the chart puts the same amount down.
    let mut mid = ScalarField::zeros(topo, Scale::ONE);
    let mid_centre = SurfacePoint::new(Face::Front, 161.0, 90.0);
    let mid_touched = deposit(topo, Scale::ONE, &mut mid, mid_centre, 9.0, 5.0);
    assert!((mid.total() - 5.0).abs() <= 1e-9);
    assert_eq!(touched, mid_touched, "the wrap costs the footprint no cells");
}

// ---------------------------------------------------------------------------
// validate
// ---------------------------------------------------------------------------

#[test]
fn validate_accepts_the_section_6_ladder() {
    assert_eq!(Topology::Cube.validate(Scale::ONE), Ok(()));
    assert_eq!(ring().validate(Scale::ONE), Ok(()), "320×180 at S = 1");
    assert_eq!(
        Topology::Ring { w: 640, h: 360 }.validate(Scale::new(2.0)),
        Ok(()),
        "640×360 at S = 2"
    );
}

#[test]
fn validate_refuses_extents_that_are_not_cell_multiples() {
    assert_eq!(
        Topology::Ring { w: 321, h: 180 }.validate(Scale::ONE),
        Err(TopologyError::ExtentNotCellMultiple { w: 321, h: 180, cell_pixels: 4.0 })
    );
    assert_eq!(
        Topology::Ring { w: 320, h: 181 }.validate(Scale::ONE),
        Err(TopologyError::ExtentNotCellMultiple { w: 320, h: 181, cell_pixels: 4.0 })
    );
    // At S = 2 the cell is 8 px, so a raster that was legal at S = 1 need not be.
    assert_eq!(
        ring().validate(Scale::new(2.0)),
        Err(TopologyError::ExtentNotCellMultiple { w: 320, h: 180, cell_pixels: 8.0 })
    );
}

#[test]
fn validate_refuses_more_cells_than_a_u16_can_index() {
    // §2's worked example: "at cell = 4 px a 1920×1080 world would want 129,600".
    assert_eq!(
        Topology::Ring { w: 1920, h: 1080 }.validate(Scale::ONE),
        Err(TopologyError::TooManyCells { cells: 129_600, w: 1920, h: 1080, cell_pixels: 4.0 })
    );
    // The same raster at S = 2 is 8 px cells, 240 × 135 = 32,400, and fits.
    assert_eq!(Topology::Ring { w: 1920, h: 1080 }.validate(Scale::new(2.0)), Ok(()));
}

#[test]
fn validate_refuses_a_ring_too_narrow_for_two_images() {
    // FW-1 §7.3: `w >= 2·max_local_radius() + 2·CELL_PIXELS` holds by construction, so
    // what `validate` can actually refuse is a non-positive local radius.
    for (w, h) in [(8u16, 180u16), (4, 8), (4, 180)] {
        let topo = Topology::Ring { w, h };
        assert!(
            matches!(topo.validate(Scale::ONE), Err(TopologyError::RingTooNarrow { .. })),
            "{topo:?} must be refused as too narrow, got {:?}",
            topo.validate(Scale::ONE)
        );
    }
}

#[test]
fn validate_refuses_a_cube_whose_world_scale_is_not_one() {
    // §2: "`world_scale` is a ring-only parameter. `Topology::Cube` pins `S = 1`".
    assert_eq!(
        Topology::Cube.validate(Scale::new(2.0)),
        Err(TopologyError::CubeScaleNotOne { world: 2.0 })
    );
    assert_eq!(
        Topology::Cube.validate(Scale::new(0.5)),
        Err(TopologyError::CubeScaleNotOne { world: 0.5 })
    );
}

#[test]
fn validate_refuses_an_empty_ring_and_a_bad_scale() {
    assert_eq!(
        Topology::Ring { w: 0, h: 180 }.validate(Scale::ONE),
        Err(TopologyError::EmptyRing { w: 0, h: 180 })
    );
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let got = ring().validate(Scale::new(bad));
        assert!(
            matches!(got, Err(TopologyError::BadScale { .. })),
            "scale {bad} must be refused, got {got:?}"
        );
    }
}

/// FW-1 §7.4 added a refusal the plan does not list: a ring whose stamp budget `9·S` does
/// not fit inside its own local radius, because otherwise a validated world panics inside
/// `unfold_pixels` the first time anything is stamped. It never fires on §6's ladder.
#[test]
fn validate_refuses_a_ring_whose_stamp_budget_exceeds_its_local_radius() {
    let topo = Topology::Ring { w: 64, h: 16 };
    assert_eq!(topo.max_local_radius(), 8.0, "min(16, 64 − 8) / 2");
    assert!(Scale::ONE.footprint_radius() > topo.max_local_radius());
    assert_eq!(
        topo.validate(Scale::ONE),
        Err(TopologyError::FootprintExceedsLocalRadius { footprint: 9.0, radius: 8.0 })
    );
    // The ladder is clear of it: 9 ≤ 90 at S = 1, 18 ≤ 180 at S = 2, 9 ≤ 32 on the cube.
    assert_eq!(ring().max_local_radius(), 90.0, "min(180, 320 − 8) / 2");
    assert_eq!(Topology::Ring { w: 640, h: 360 }.max_local_radius(), 180.0);
    assert_eq!(Topology::Cube.max_local_radius(), 32.0);
}
