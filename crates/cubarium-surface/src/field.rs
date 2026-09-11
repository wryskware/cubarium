//! The scalar field graph: 16×16 cells per face with reciprocal seam edges.

use crate::{ChartImage, Edge, Face, MAX_SEAMS, SurfacePoint, chart_images, cross_seam, unfold_with};

/// Cells along one face edge.
pub const CELLS_PER_FACE_EDGE: usize = 16;
/// Pixels along one cell edge.
pub const CELL_PIXELS: f64 = 4.0;
/// Total cells: five faces × 16 × 16.
pub const CELL_COUNT: usize = 5 * CELLS_PER_FACE_EDGE * CELLS_PER_FACE_EDGE;

/// A field cell: `face.index() * 256 + cy * 16 + cx`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellId(pub u16);

impl CellId {
    pub fn new(face: Face, cx: u8, cy: u8) -> CellId {
        assert!(cx < 16 && cy < 16, "cell ({cx}, {cy}) out of range");
        CellId((face.index() * 256 + usize::from(cy) * 16 + usize::from(cx)) as u16)
    }

    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn face(self) -> Face {
        Face::from_index((self.0 / 256) as u8).expect("valid cell face")
    }

    #[inline]
    pub fn cx(self) -> u8 {
        (self.0 % 16) as u8
    }

    #[inline]
    pub fn cy(self) -> u8 {
        ((self.0 % 256) / 16) as u8
    }

    /// The cell's center: `(cx*4 + 2, cy*4 + 2)`.
    pub fn center(self) -> SurfacePoint {
        SurfacePoint::new(
            self.face(),
            f64::from(self.cx()) * CELL_PIXELS + 2.0,
            f64::from(self.cy()) * CELL_PIXELS + 2.0,
        )
    }

    /// Every cell in index order.
    pub fn all() -> impl Iterator<Item = CellId> {
        (0..CELL_COUNT as u16).map(CellId)
    }
}

/// The cell containing a point (floor of `u/4`, `v/4`; a transient 64 clamps to 15).
pub fn cell_of(p: &SurfacePoint) -> CellId {
    let c = |x: f64| (x / CELL_PIXELS).floor().clamp(0.0, 15.0) as u8;
    CellId::new(p.face, c(p.u), c(p.v))
}

/// Cardinal adjacency of cells across the whole surface, derived once from
/// `Face::neighbor` and `cross_seam` at cell resolution.
///
/// Normative derivation: the neighbor of cell `(face, cx, cy)` across `edge` is the
/// adjacent cell in the same chart when one exists; otherwise it is the cell containing
/// the pixel `cross_seam(face, edge, t)` returns for `t = 4·k` where `k` is the cell's
/// along-edge index (`cx` for Top/Bottom, `cy` for Left/Right). Reversal at the Right/Top
/// and Back/Top seams therefore comes out of `cross_seam` (`63 - t` at pixel resolution
/// maps cell `k` to cell `15 - k`). The open rim yields `None`.
///
/// Invariants: the relation is reciprocal (`b` across `e` of `a` implies `a` is across
/// some edge of `b`), every cell has degree 4 except the 64 rim cells with degree 3, and
/// there are exactly 2,528 undirected edges (2,400 within charts, 128 across seams).
pub struct FieldGraph {
    neighbors: Box<[[Option<CellId>; 4]; CELL_COUNT]>,
    edges: Vec<(CellId, CellId)>,
}

impl FieldGraph {
    pub fn new() -> FieldGraph {
        let mut neighbors: Box<[[Option<CellId>; 4]; CELL_COUNT]> =
            vec![[None; 4]; CELL_COUNT].into_boxed_slice().try_into().expect("CELL_COUNT cells");

        let last = (CELLS_PER_FACE_EDGE - 1) as u8;
        for cell in CellId::all() {
            let (face, cx, cy) = (cell.face(), cell.cx(), cell.cy());
            for edge in Edge::ALL {
                let inside = match edge {
                    Edge::Top => (cy > 0).then(|| (cx, cy - 1)),
                    Edge::Right => (cx < last).then(|| (cx + 1, cy)),
                    Edge::Bottom => (cy < last).then(|| (cx, cy + 1)),
                    Edge::Left => (cx > 0).then(|| (cx - 1, cy)),
                };
                let n = match inside {
                    Some((nx, ny)) => Some(CellId::new(face, nx, ny)),
                    None => {
                        // A boundary cell: ask the seam contract at pixel resolution for
                        // the first pixel of this cell's along-edge run.
                        let k = match edge {
                            Edge::Top | Edge::Bottom => cx,
                            Edge::Right | Edge::Left => cy,
                        };
                        let t = k * CELL_PIXELS as u8;
                        cross_seam(face, edge, t).map(|(nf, nx, ny, _)| {
                            CellId::new(nf, nx / CELL_PIXELS as u8, ny / CELL_PIXELS as u8)
                        })
                    }
                };
                neighbors[cell.index()][edge as usize] = n;
            }
        }

        let mut edges: Vec<(CellId, CellId)> = Vec::with_capacity(2 * 2528);
        for cell in CellId::all() {
            for n in neighbors[cell.index()].iter().flatten() {
                edges.push(if cell < *n { (cell, *n) } else { (*n, cell) });
            }
        }
        edges.sort_unstable();
        edges.dedup();

        let graph = FieldGraph { neighbors, edges };
        debug_assert!(graph.is_reciprocal(), "cell adjacency is not reciprocal");
        debug_assert_eq!(graph.edges.len(), 2528, "expected 2,400 in-chart + 128 seam edges");
        graph
    }

    /// Every named neighbour names this cell back across some edge (debug check).
    fn is_reciprocal(&self) -> bool {
        CellId::all().all(|a| {
            self.neighbors[a.index()]
                .iter()
                .flatten()
                .all(|b| self.neighbors[b.index()].iter().flatten().any(|c| *c == a))
        })
    }

    /// Neighbor across `edge`, or `None` at the open rim.
    #[inline]
    pub fn neighbor(&self, cell: CellId, edge: Edge) -> Option<CellId> {
        self.neighbors[cell.index()][edge as usize]
    }

    /// All four directions in `Edge` order (`Top, Right, Bottom, Left`).
    #[inline]
    pub fn neighbors(&self, cell: CellId) -> &[Option<CellId>; 4] {
        &self.neighbors[cell.index()]
    }

    pub fn degree(&self, cell: CellId) -> usize {
        self.neighbors[cell.index()].iter().flatten().count()
    }

    /// Every undirected edge exactly once, as `(a, b)` with `a < b`, sorted.
    pub fn edges(&self) -> &[(CellId, CellId)] {
        &self.edges
    }
}

impl Default for FieldGraph {
    fn default() -> Self {
        FieldGraph::new()
    }
}

/// One scalar per cell.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarField {
    pub values: Box<[f64; CELL_COUNT]>,
}

impl ScalarField {
    pub fn zeros() -> ScalarField {
        ScalarField { values: Box::new([0.0; CELL_COUNT]) }
    }

    pub fn constant(x: f64) -> ScalarField {
        ScalarField { values: Box::new([x; CELL_COUNT]) }
    }

    #[inline]
    pub fn get(&self, c: CellId) -> f64 {
        self.values[c.index()]
    }

    #[inline]
    pub fn set(&mut self, c: CellId, x: f64) {
        self.values[c.index()] = x;
    }

    #[inline]
    pub fn add(&mut self, c: CellId, x: f64) {
        self.values[c.index()] += x;
    }

    /// Sum over all cells (plain left-to-right summation; tests choose tolerances
    /// accordingly).
    pub fn total(&self) -> f64 {
        self.values.iter().sum()
    }

    pub fn min(&self) -> f64 {
        self.values.iter().copied().fold(f64::INFINITY, f64::min)
    }

    pub fn max(&self) -> f64 {
        self.values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn is_finite(&self) -> bool {
        self.values.iter().all(|x| x.is_finite())
    }

    pub fn is_nonnegative(&self) -> bool {
        self.values.iter().all(|&x| x >= 0.0)
    }
}

/// One explicit, conservative diffusion step with per-edge exchange coefficient `rate`
/// (dimensionless, `D·dt/h²`). Returns the number of substeps used.
///
/// Normative: for every undirected edge `(a, b)` the flux `k·(field[a] - field[b])` is
/// subtracted from `a` and added to `b` exactly once, computed from the pre-step values
/// (`scratch` holds them), so total mass is conserved to rounding and no flux crosses the
/// open rim. If `rate > 0.25` the step is split into `n = ceil(rate / 0.25)` substeps of
/// `rate / n` so that a cell's total outflow never exceeds its content (at most four
/// edges × 0.25): nonnegativity is then exact, not approximate. A nonpositive or
/// non-finite `rate` performs no step and returns 0. Constant fields stay bit-identical.
pub fn diffuse(field: &mut ScalarField, scratch: &mut ScalarField, graph: &FieldGraph, rate: f64) -> u32 {
    if !rate.is_finite() || rate <= 0.0 {
        return 0;
    }
    // At most four edges per cell, so a per-edge coefficient of 0.25 is the largest that
    // cannot drain a cell below zero. Split anything larger into equal substeps.
    let substeps = (rate / 0.25).ceil().max(1.0) as u32;
    let k = rate / f64::from(substeps);
    for _ in 0..substeps {
        scratch.values.copy_from_slice(&field.values[..]);
        for &(a, b) in &graph.edges {
            let flux = k * (scratch.values[a.index()] - scratch.values[b.index()]);
            field.values[a.index()] -= flux;
            field.values[b.index()] += flux;
        }
    }
    substeps
}

/// Add `amount` to the field with a raised-cosine footprint of surface radius `radius`
/// pixels (≤ `MAX_LOCAL_RADIUS`) around `center`, normalized over the cells actually
/// present so the field total rises by exactly `amount` (to rounding). Returns the number
/// of cells touched.
///
/// Normative: weight of cell `c` is `w = 0.5·(1 + cos(π·d/radius))` for its center's
/// surface distance `d ≤ radius` from `center` (via [`crate::unfold`]), else 0; each
/// touched cell receives `amount · w / Σw`. A footprint clipped by the rim or covering
/// a vertex deposits the same total as one in the middle of a face. If no cell is within
/// range (radius below half a cell), the containing cell receives everything.
pub fn deposit(field: &mut ScalarField, center: SurfacePoint, radius: f64, amount: f64) -> usize {
    debug_assert!(center.is_canonical(), "deposit at non-canonical {center:?}");
    let mut images: Vec<ChartImage> = Vec::new();
    chart_images(center.face, MAX_SEAMS, &mut images);

    // (cell, raised-cosine weight) for every cell center inside the footprint.
    let mut touched: Vec<(CellId, f64)> = Vec::new();
    let mut total = 0.0f64;
    if radius > 0.0 {
        for cell in CellId::all() {
            let Some(u) = unfold_with(&images, center, cell.center(), radius) else {
                continue;
            };
            if u.distance > radius {
                continue;
            }
            let w = 0.5 * (1.0 + (std::f64::consts::PI * u.distance / radius).cos());
            if w > 0.0 {
                touched.push((cell, w));
                total += w;
            }
        }
    }
    if touched.is_empty() || !total.is_finite() || total <= 0.0 {
        // Below half a cell: the containing cell takes everything.
        field.add(cell_of(&center), amount);
        return 1;
    }
    let scale = amount / total;
    for &(cell, w) in &touched {
        field.add(cell, w * scale);
    }
    touched.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FACE_EXTENT, Vec2, pixel_neighbor, travel};

    fn graph() -> FieldGraph {
        FieldGraph::new()
    }

    #[test]
    fn cell_ids_round_trip() {
        for cell in CellId::all() {
            assert_eq!(CellId::new(cell.face(), cell.cx(), cell.cy()), cell);
            assert_eq!(cell_of(&cell.center()), cell);
        }
        assert_eq!(CellId::all().count(), CELL_COUNT);
        assert_eq!(CELL_COUNT, 1280);
    }

    #[test]
    fn graph_counts_and_degrees() {
        let g = graph();
        assert_eq!(g.edges().len(), 2528, "2,400 in-chart + 128 seam edges");
        let mut rim = 0;
        for cell in CellId::all() {
            match g.degree(cell) {
                4 => {}
                3 => rim += 1,
                d => panic!("{cell:?} has degree {d}"),
            }
        }
        assert_eq!(rim, 64, "16 rim cells on each of the four side faces");
        // Sorted, unique, and canonically ordered.
        for w in g.edges().windows(2) {
            assert!(w[0] < w[1]);
        }
        for &(a, b) in g.edges() {
            assert!(a < b);
        }
        // Exactly 128 of them cross a seam.
        let seam_edges = g.edges().iter().filter(|(a, b)| a.face() != b.face()).count();
        assert_eq!(seam_edges, 128);
    }

    #[test]
    fn adjacency_is_reciprocal_and_has_no_duplicates() {
        let g = graph();
        for a in CellId::all() {
            let ns = g.neighbors(a);
            for (i, n) in ns.iter().enumerate() {
                let Some(b) = *n else { continue };
                assert_ne!(b, a, "{a:?} is its own neighbour");
                for (j, m) in ns.iter().enumerate() {
                    if i != j {
                        assert_ne!(*m, Some(b), "{a:?} lists {b:?} twice");
                    }
                }
                assert!(
                    g.neighbors(b).iter().flatten().any(|c| *c == a),
                    "{a:?} -> {b:?} is not reciprocated"
                );
            }
        }
    }

    #[test]
    fn only_the_rim_is_open() {
        let g = graph();
        for cell in CellId::all() {
            for edge in Edge::ALL {
                let open = g.neighbor(cell, edge).is_none();
                let is_rim = edge == Edge::Bottom
                    && cell.cy() == 15
                    && cell.face() != Face::Top;
                assert_eq!(open, is_rim, "{cell:?} {edge:?}");
            }
        }
    }

    #[test]
    fn seam_neighbours_agree_with_pixel_adjacency() {
        // The cell graph must be the cell-resolution shadow of the pixel-level seam
        // contract: the cell across an edge contains the pixel across that edge.
        let g = graph();
        for cell in CellId::all() {
            for edge in Edge::ALL {
                // The first pixel of this cell's along-edge run, on the cell's own
                // side of `edge`.
                let (px, py) = match edge {
                    Edge::Top => (cell.cx() * 4, cell.cy() * 4),
                    Edge::Right => (cell.cx() * 4 + 3, cell.cy() * 4),
                    Edge::Bottom => (cell.cx() * 4, cell.cy() * 4 + 3),
                    Edge::Left => (cell.cx() * 4, cell.cy() * 4),
                };
                let want = pixel_neighbor(cell.face(), px, py, edge)
                    .map(|(f, x, y)| CellId::new(f, x / 4, y / 4));
                assert_eq!(g.neighbor(cell, edge), want, "{cell:?} {edge:?}");
            }
        }
    }

    #[test]
    fn top_seams_reverse_at_cell_resolution() {
        let g = graph();
        // Right.top cell k joins Top's right column cell 15 - k.
        for k in 0..16u8 {
            let a = CellId::new(Face::Right, k, 0);
            let b = g.neighbor(a, Edge::Top).expect("seam");
            assert_eq!((b.face(), b.cx(), b.cy()), (Face::Top, 15, 15 - k), "k={k}");
        }
        // Back.top cell k joins Top's top row cell 15 - k.
        for k in 0..16u8 {
            let a = CellId::new(Face::Back, k, 0);
            let b = g.neighbor(a, Edge::Top).expect("seam");
            assert_eq!((b.face(), b.cx(), b.cy()), (Face::Top, 15 - k, 0), "k={k}");
        }
        // Left.top cell k joins Top's left column cell k (not reversed, but twisted).
        for k in 0..16u8 {
            let a = CellId::new(Face::Left, k, 0);
            let b = g.neighbor(a, Edge::Top).expect("seam");
            assert_eq!((b.face(), b.cx(), b.cy()), (Face::Top, 0, k), "k={k}");
        }
        // Front.top cell k joins Top's bottom row cell k.
        for k in 0..16u8 {
            let a = CellId::new(Face::Front, k, 0);
            let b = g.neighbor(a, Edge::Top).expect("seam");
            assert_eq!((b.face(), b.cx(), b.cy()), (Face::Top, k, 15), "k={k}");
        }
    }

    #[test]
    fn cell_centers_are_neighbours_on_the_surface() {
        // Every graph edge joins cells whose centers are four pixels apart across the
        // real surface, which is an independent check of the seam wiring.
        let g = graph();
        for &(a, b) in g.edges() {
            let d = crate::surface_distance(a.center(), b.center(), 8.0);
            let d = d.unwrap_or_else(|| panic!("{a:?} and {b:?} are not within 8 pixels"));
            assert!((d - 4.0).abs() < 1e-9, "{a:?} {b:?}: {d}");
        }
    }

    #[test]
    fn constant_fields_stay_bit_identical() {
        let g = graph();
        let mut scratch = ScalarField::zeros();
        for rate in [0.05, 0.25, 0.5, 1.0, 3.0] {
            let mut f = ScalarField::constant(1.25);
            let n = diffuse(&mut f, &mut scratch, &g, rate);
            assert!(n >= 1);
            assert_eq!(f, ScalarField::constant(1.25), "rate {rate}");
        }
    }

    #[test]
    fn diffusion_conserves_mass_and_nonnegativity() {
        let g = graph();
        let mut scratch = ScalarField::zeros();
        let mut f = ScalarField::zeros();
        // Sources at a top vertex, at the rim, and in the middle of a face.
        f.set(CellId::new(Face::Front, 15, 0), 100.0);
        f.set(CellId::new(Face::Front, 8, 15), 40.0);
        f.set(CellId::new(Face::Top, 8, 8), 7.5);
        let start = f.total();
        for step in 0..200 {
            let n = diffuse(&mut f, &mut scratch, &g, 0.9);
            assert_eq!(n, 4, "0.9 / 0.25 -> 4 substeps");
            assert!(f.is_finite());
            assert!(f.is_nonnegative(), "negative value at step {step}: {}", f.min());
            assert!(
                (f.total() - start).abs() < 1e-9 * start.max(1.0) * f64::from(step + 1),
                "mass drifted at step {step}: {} vs {start}",
                f.total()
            );
        }
        // It really did spread: no cell still holds most of the mass.
        assert!(f.max() < 5.0, "{}", f.max());
    }

    #[test]
    fn substep_counts_follow_the_rate() {
        let g = graph();
        let mut scratch = ScalarField::zeros();
        let mut f = ScalarField::zeros();
        f.set(CellId::new(Face::Front, 4, 4), 1.0);
        for (rate, want) in [(0.1, 1u32), (0.25, 1), (0.26, 2), (0.5, 2), (0.75, 3), (1.0, 4)] {
            assert_eq!(diffuse(&mut f, &mut scratch, &g, rate), want, "rate {rate}");
        }
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let before = f.clone();
            assert_eq!(diffuse(&mut f, &mut scratch, &g, bad), 0, "rate {bad}");
            assert_eq!(f, before);
        }
    }

    #[test]
    fn no_flux_crosses_the_open_rim() {
        let g = graph();
        // The rim cells of the side faces never exchange downward.
        for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
            for cx in 0..16u8 {
                assert_eq!(g.neighbor(CellId::new(face, cx, 15), Edge::Bottom), None);
            }
        }
        // And a field that is uniform above the rim does not leak mass out of it.
        let mut scratch = ScalarField::zeros();
        let mut f = ScalarField::zeros();
        for cx in 0..16u8 {
            f.set(CellId::new(Face::Front, cx, 15), 1.0);
        }
        let start = f.total();
        for _ in 0..50 {
            diffuse(&mut f, &mut scratch, &g, 0.5);
        }
        assert!((f.total() - start).abs() < 1e-9, "{} vs {start}", f.total());
    }

    #[test]
    fn deposits_add_exactly_their_amount() {
        let cases = [
            (SurfacePoint::new(Face::Front, 32.0, 32.0), 9.0),
            (SurfacePoint::new(Face::Front, 63.5, 0.5), 9.0),
            (SurfacePoint::new(Face::Front, 32.0, 63.5), 9.0),
            (SurfacePoint::new(Face::Top, 0.5, 0.5), 12.0),
            (SurfacePoint::new(Face::Right, 63.5, 63.5), 6.0),
            (SurfacePoint::new(Face::Back, 0.0, 0.0), 4.0),
        ];
        for (center, radius) in cases {
            let mut f = ScalarField::zeros();
            let n = deposit(&mut f, center, radius, 25.0);
            assert!(n > 0);
            assert!(
                (f.total() - 25.0).abs() < 1e-9,
                "{center:?} r={radius}: {} over {n} cells",
                f.total()
            );
            assert!(f.is_nonnegative());
            // Everything landed near the center.
            for cell in CellId::all() {
                if f.get(cell) > 0.0 {
                    let d = crate::surface_distance(center, cell.center(), radius);
                    assert!(d.is_some_and(|d| d <= radius), "{cell:?} is outside the footprint");
                }
            }
        }
    }

    #[test]
    fn a_footprint_near_a_seam_deposits_the_same_total_as_one_in_the_middle() {
        let mut middle = ScalarField::zeros();
        deposit(&mut middle, SurfacePoint::new(Face::Front, 32.0, 32.0), 9.0, 1.0);
        let mut seam = ScalarField::zeros();
        deposit(&mut seam, SurfacePoint::new(Face::Front, 63.9, 32.0), 9.0, 1.0);
        let mut rim = ScalarField::zeros();
        deposit(&mut rim, SurfacePoint::new(Face::Front, 32.0, 63.9), 9.0, 1.0);
        for f in [&middle, &seam, &rim] {
            assert!((f.total() - 1.0).abs() < 1e-9, "{}", f.total());
        }
        // The seam-spanning footprint really does straddle two charts.
        assert!(seam.values.iter().enumerate().any(|(i, &x)| x > 0.0 && i >= 256));
    }

    #[test]
    fn a_tiny_radius_falls_back_to_the_containing_cell() {
        let mut f = ScalarField::zeros();
        let center = SurfacePoint::new(Face::Left, 5.0, 7.0);
        let n = deposit(&mut f, center, 0.5, 3.0);
        assert_eq!(n, 1);
        assert_eq!(f.get(cell_of(&center)), 3.0);
        assert!((f.total() - 3.0).abs() < 1e-12);
    }

    #[test]
    fn a_deposit_moved_over_a_seam_keeps_its_shape() {
        // Sliding the same footprint across the Front/Right seam must not change the
        // total, and the weight profile must stay smooth: track the peak cell value.
        let mut peaks = Vec::new();
        for u in [56.0, 60.0, 63.0, FACE_EXTENT - 0.01] {
            let mut f = ScalarField::zeros();
            // Sliding the point there with `travel` keeps it canonical, seam or not.
            let start = travel(SurfacePoint::new(Face::Front, 32.0, 32.0), Vec2::new(u - 32.0, 0.0)).end;
            deposit(&mut f, start, 9.0, 1.0);
            assert!((f.total() - 1.0).abs() < 1e-9);
            peaks.push(f.max());
        }
        for w in peaks.windows(2) {
            assert!((w[0] - w[1]).abs() < 0.02, "{peaks:?}");
        }
    }
}
