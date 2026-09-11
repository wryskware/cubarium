//! The scalar field graph: 16×16 cells per face with reciprocal seam edges.

use crate::{Edge, Face, SurfacePoint};

/// Cells along one face edge.
pub const CELLS_PER_FACE_EDGE: usize = 16;
/// Pixels along one cell edge.
pub const CELL_PIXELS: f64 = 4.0;
/// Total cells: five faces × 16 × 16.
pub const CELL_COUNT: usize = 5 * CELLS_PER_FACE_EDGE * CELLS_PER_FACE_EDGE;

/// A field cell: `face.index() * 256 + cy * 16 + cx`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
        todo!("FieldGraph::new")
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
    let _ = (field, scratch, graph, rate);
    todo!("diffuse")
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
    let _ = (field, center, radius, amount);
    todo!("deposit")
}
