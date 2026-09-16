//! The scalar field graph: 16×16 cells per cube face, or one `w/4S × h/4S` ring grid.

use crate::{
    ChartImage, Edge, Face, MAX_SEAMS, Scale, SurfacePoint, Topology, chart_images, cross_seam,
    face_frame, unfold_with,
};

/// Height differences below this are treated as level (see [`FieldGraph::downhill`]).
const DOWNHILL_EPS: f64 = 1e-9;

/// Cells along one cube face edge.
pub const CELLS_PER_FACE_EDGE: usize = 16;

/// Cells on a cube: five faces × 16 × 16. **A runtime cell count is
/// [`Topology::cell_count`]**; this constant is only for cube-only literals and fixtures,
/// which no ring world may use (`1,280 = 2^8·5` cannot be factored 16:9 with square
/// cells, so no ring raster reproduces it).
pub const CUBE_CELL_COUNT: usize = 5 * CELLS_PER_FACE_EDGE * CELLS_PER_FACE_EDGE;

/// A field cell: a plain index, decoded through the topology.
///
/// On a cube that is `face.index() * 256 + cy * 16 + cx`; on a ring, `cy * cells_x + cx`.
/// The index is a `u16`, so a topology may not exceed 65,535 cells — a bound
/// [`Topology::validate`] enforces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellId(pub u16);

impl CellId {
    /// The cell at `(cx, cy)` of `face`. Panics if the cell is outside the chart's grid.
    pub fn new(topo: Topology, scale: Scale, face: Face, cx: u16, cy: u16) -> CellId {
        let (nx, ny) = topo.cells(scale, face);
        assert!(cx < nx && cy < ny, "cell ({cx}, {cy}) out of range for {topo:?}");
        let chart = topo.chart_index(face);
        let per_chart = usize::from(nx) * usize::from(ny);
        CellId((chart * per_chart + usize::from(cy) * usize::from(nx) + usize::from(cx)) as u16)
    }

    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// The chart this cell belongs to.
    #[inline]
    pub fn face(self, topo: Topology, scale: Scale) -> Face {
        let (nx, ny) = topo.cells(scale, Face::Front);
        let per_chart = usize::from(nx) * usize::from(ny);
        topo.charts()[self.index() / per_chart]
    }

    /// The cell's column within its chart.
    #[inline]
    pub fn cx(self, topo: Topology, scale: Scale) -> u16 {
        let (nx, ny) = topo.cells(scale, Face::Front);
        let per_chart = usize::from(nx) * usize::from(ny);
        ((self.index() % per_chart) % usize::from(nx)) as u16
    }

    /// The cell's row within its chart.
    #[inline]
    pub fn cy(self, topo: Topology, scale: Scale) -> u16 {
        let (nx, ny) = topo.cells(scale, Face::Front);
        let per_chart = usize::from(nx) * usize::from(ny);
        ((self.index() % per_chart) / usize::from(nx)) as u16
    }

    /// The cell's center: half a cell in from its top-left corner.
    pub fn center(self, topo: Topology, scale: Scale) -> SurfacePoint {
        let cp = cell_pixels(topo, scale);
        SurfacePoint::new(
            self.face(topo, scale),
            f64::from(self.cx(topo, scale)) * cp + cp / 2.0,
            f64::from(self.cy(topo, scale)) * cp + cp / 2.0,
        )
    }

    /// Every cell in index order.
    pub fn all(topo: Topology, scale: Scale) -> impl Iterator<Item = CellId> {
        (0..topo.cell_count(scale) as u16).map(CellId)
    }
}

/// Pixels per cell edge: 4 on the cube (pinned to `S = 1`), `4·S` on a ring.
#[inline]
fn cell_pixels(topo: Topology, scale: Scale) -> f64 {
    match topo {
        Topology::Cube => crate::CELL_PIXELS,
        Topology::Ring { .. } => scale.cell_pixels(),
    }
}

/// The cell containing a point (floor of `u/cell_pixels`, `v/cell_pixels`; a transient
/// coordinate equal to the extent clamps to the last cell).
pub fn cell_of(topo: Topology, scale: Scale, p: &SurfacePoint) -> CellId {
    let cp = cell_pixels(topo, scale);
    let (nx, ny) = topo.cells(scale, p.face);
    let c = |x: f64, n: u16| (x / cp).floor().clamp(0.0, f64::from(n - 1)) as u16;
    CellId::new(topo, scale, p.face, c(p.u, nx), c(p.v, ny))
}

/// Cardinal adjacency of cells across the whole surface, derived once from the topology.
///
/// Normative derivation: the neighbor of cell `(face, cx, cy)` across `edge` is the
/// adjacent cell in the same chart when one exists. Otherwise, **on a cube**, it is the
/// cell containing the pixel `cross_seam(face, edge, t)` returns for `t = 4·k` where `k`
/// is the cell's along-edge index (`cx` for Top/Bottom, `cy` for Left/Right) — reversal at
/// the Right/Top and Back/Top seams therefore comes out of `cross_seam` (`63 - t` at pixel
/// resolution maps cell `k` to cell `15 - k`) — and the open rim yields `None`. **On a
/// ring**, `Edge::Right` at the last column is `(0, cy)` and `Edge::Left` at column 0 is
/// `(cells_x - 1, cy)`: every row is a ring. The two horizontal rims yield `None`.
///
/// Invariants: the relation is reciprocal (`b` across `e` of `a` implies `a` is across
/// some edge of `b`); on a cube every cell has degree 4 except the 64 rim cells with
/// degree 3, and there are exactly 2,528 undirected edges (2,400 within charts, 128 across
/// seams); on a ring there are **no corners** — every cell has degree 4 except the top and
/// bottom rows at degree 3 — and `cells_x·cells_y + cells_x·(cells_y − 1)` edges
/// (7,120 at 80×45).
pub struct FieldGraph {
    topology: Topology,
    scale: Scale,
    neighbors: Box<[[Option<CellId>; 4]]>,
    edges: Vec<(CellId, CellId)>,
    downhill: Box<[Option<CellId>]>,
}

impl FieldGraph {
    pub fn new(topo: Topology, scale: Scale) -> FieldGraph {
        assert!(
            topo.validate(scale).is_ok(),
            "field graph for an invalid world: {}",
            topo.validate(scale).unwrap_err()
        );
        let count = topo.cell_count(scale);
        let mut neighbors: Box<[[Option<CellId>; 4]]> = vec![[None; 4]; count].into_boxed_slice();

        for cell in CellId::all(topo, scale) {
            let (face, cx, cy) = (cell.face(topo, scale), cell.cx(topo, scale), cell.cy(topo, scale));
            let (nx, ny) = topo.cells(scale, face);
            let (last_x, last_y) = (nx - 1, ny - 1);
            for edge in Edge::ALL {
                let inside = match edge {
                    Edge::Top => (cy > 0).then(|| (cx, cy - 1)),
                    Edge::Right => (cx < last_x).then(|| (cx + 1, cy)),
                    Edge::Bottom => (cy < last_y).then(|| (cx, cy + 1)),
                    Edge::Left => (cx > 0).then(|| (cx - 1, cy)),
                };
                let n = match inside {
                    Some((mx, my)) => Some(CellId::new(topo, scale, face, mx, my)),
                    None => match topo {
                        Topology::Cube => {
                            // A boundary cell: ask the seam contract at pixel resolution for
                            // the first pixel of this cell's along-edge run.
                            let k = match edge {
                                Edge::Top | Edge::Bottom => cx,
                                Edge::Right | Edge::Left => cy,
                            };
                            let t = (k * crate::CELL_PIXELS as u16) as u8;
                            cross_seam(face, edge, t).map(|(nf, px, py, _)| {
                                CellId::new(
                                    topo,
                                    scale,
                                    nf,
                                    u16::from(px) / crate::CELL_PIXELS as u16,
                                    u16::from(py) / crate::CELL_PIXELS as u16,
                                )
                            })
                        }
                        // Every row of a ring is a ring; the top and bottom are solid.
                        Topology::Ring { .. } => match edge {
                            Edge::Right => Some(CellId::new(topo, scale, face, 0, cy)),
                            Edge::Left => Some(CellId::new(topo, scale, face, last_x, cy)),
                            Edge::Top | Edge::Bottom => None,
                        },
                    },
                };
                neighbors[cell.index()][edge as usize] = n;
            }
        }

        let mut edges: Vec<(CellId, CellId)> = Vec::with_capacity(2 * count);
        for cell in CellId::all(topo, scale) {
            for n in neighbors[cell.index()].iter().flatten() {
                edges.push(if cell < *n { (cell, *n) } else { (*n, cell) });
            }
        }
        edges.sort_unstable();
        edges.dedup();

        let mut downhill: Box<[Option<CellId>]> = vec![None; count].into_boxed_slice();
        for cell in CellId::all(topo, scale) {
            match topo {
                Topology::Cube => {
                    // Gravity is `(0, -1, 0)`; only its component in the face's tangent plane
                    // can move material along the surface. That component is
                    // `|g| · sqrt(1 - n_y²)` for the outward unit normal `n`, so it vanishes
                    // exactly on the level Top face and is full strength on the four side faces.
                    let n = face_frame(cell.face(topo, scale)).normal;
                    if 1.0 - n[1] * n[1] <= DOWNHILL_EPS {
                        continue;
                    }
                    let y = topo.embed(scale, &cell.center(topo, scale))[1];
                    let mut best: Option<(CellId, f64)> = None;
                    for m in neighbors[cell.index()].iter().flatten() {
                        let my = topo.embed(scale, &m.center(topo, scale))[1];
                        if my < y - DOWNHILL_EPS && best.is_none_or(|(_, by)| my < by) {
                            best = Some((*m, my));
                        }
                    }
                    downhill[cell.index()] = best.map(|(c, _)| c);
                }
                Topology::Ring { .. } => {
                    // The top row is the canopy and never drains; every other cell drains to
                    // the cell below it, which the bottom row does not have.
                    if cell.cy(topo, scale) > 0 {
                        downhill[cell.index()] = neighbors[cell.index()][Edge::Bottom as usize];
                    }
                }
            }
        }

        let graph = FieldGraph { topology: topo, scale, neighbors, edges, downhill };
        debug_assert!(graph.is_reciprocal(), "cell adjacency is not reciprocal");
        debug_assert_eq!(
            graph.edges.len(),
            expected_edges(topo, scale),
            "unexpected edge count for {topo:?}"
        );
        graph
    }

    /// The topology this graph was built for.
    #[inline]
    pub fn topology(&self) -> Topology {
        self.topology
    }

    /// The world scale this graph was built for.
    #[inline]
    pub fn scale(&self) -> Scale {
        self.scale
    }

    /// Cells in this graph.
    #[inline]
    pub fn cell_count(&self) -> usize {
        self.neighbors.len()
    }

    /// Every named neighbour names this cell back across some edge (debug check).
    fn is_reciprocal(&self) -> bool {
        CellId::all(self.topology, self.scale).all(|a| {
            self.neighbors[a.index()]
                .iter()
                .flatten()
                .all(|b| self.neighbors[b.index()].iter().flatten().any(|c| *c == a))
        })
    }

    /// Neighbor across `edge`, or `None` at an open rim.
    #[inline]
    pub fn neighbor(&self, cell: CellId, edge: Edge) -> Option<CellId> {
        self.neighbors[cell.index()][edge as usize]
    }

    /// All four directions in `Edge` order (`Top, Right, Bottom, Left`).
    #[inline]
    pub fn neighbors(&self, cell: CellId) -> &[Option<CellId>; 4] {
        &self.neighbors[cell.index()]
    }

    /// The cell one step downhill, or `None` where nothing can slide.
    ///
    /// **Cube** (normative, precomputed once per graph). Write `y(c)` for the embedded
    /// height of a cell center and `n` for the outward unit normal of the cell's face.
    ///
    /// 1. Gravity `(0, −1, 0)` only moves surface material through its component in the
    ///    face's tangent plane, whose magnitude is `sqrt(1 − n_y²)`. On the Top face that
    ///    is zero — the canopy is level — so **every Top-face cell has no downhill
    ///    neighbor**, including the ones that border a side face across a seam. On the
    ///    four side faces it is one, so the rule below applies.
    /// 2. Otherwise the downhill neighbor is the graph neighbor with the lowest `y`,
    ///    and only if that `y` is lower than the cell's own by more than `1e-9`.
    ///    Ties (never reachable on the cube, where the unique lowest neighbor of a side
    ///    cell is the one below it) go to the earliest [`Edge`] in `Edge::ALL` order.
    /// 3. The bottom row of a side face (`cy == 15`) has no neighbor below it — the rim
    ///    is open — and its in-face and cross-seam neighbors are level with it, so it
    ///    has no downhill neighbor either. Material there stays put.
    ///
    /// **Ring**: the same two exceptions, stated directly (`design/flat-world-plan-2026-09-16.md`
    /// §5). `downhill(c)` is `None` when `cy == 0` — the top cell row is the canopy and
    /// holds its water and detritus exactly as the cube's level Top does — and otherwise
    /// the neighbour at `(cx, cy + 1)`, which the bottom row does not have, so it keeps
    /// its litter exactly as the cube's rim row does.
    ///
    /// Consequences a consumer may rely on: the relation is acyclic, every cell outside
    /// the first and last rows has exactly one downhill step, that step lands on a cell of
    /// the same chart, and no downhill step ever points at the cube's Top face.
    #[inline]
    pub fn downhill(&self, cell: CellId) -> Option<CellId> {
        self.downhill[cell.index()]
    }

    pub fn degree(&self, cell: CellId) -> usize {
        self.neighbors[cell.index()].iter().flatten().count()
    }

    /// Every undirected edge exactly once, as `(a, b)` with `a < b`, sorted.
    pub fn edges(&self) -> &[(CellId, CellId)] {
        &self.edges
    }
}

/// The undirected edge count the derivation above must produce.
fn expected_edges(topo: Topology, scale: Scale) -> usize {
    match topo {
        // 2,400 within charts + 128 across seams.
        Topology::Cube => 2528,
        Topology::Ring { .. } => {
            let (nx, ny) = topo.cells(scale, Face::Front);
            let (nx, ny) = (usize::from(nx), usize::from(ny));
            // Every row is a ring, so the wrap adds one horizontal edge per row.
            nx * ny + nx * (ny - 1)
        }
    }
}

/// One scalar per cell, sized at construction from [`Topology::cell_count`].
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarField {
    pub values: Box<[f64]>,
}

impl ScalarField {
    pub fn zeros(topo: Topology, scale: Scale) -> ScalarField {
        ScalarField::constant(topo, scale, 0.0)
    }

    pub fn constant(topo: Topology, scale: Scale, x: f64) -> ScalarField {
        ScalarField { values: vec![x; topo.cell_count(scale)].into_boxed_slice() }
    }

    /// Cells in this field.
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
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
/// pixels (≤ [`Topology::max_local_radius`]) around `center`, normalized over the cells
/// actually present so the field total rises by exactly `amount` (to rounding). Returns
/// the number of cells touched.
///
/// Normative: weight of cell `c` is `w = 0.5·(1 + cos(π·d/radius))` for its center's
/// surface distance `d ≤ radius` from `center` (via [`crate::unfold`]), else 0; each
/// touched cell receives `amount · w / Σw`. A footprint clipped by a rim, covering a cube
/// vertex or straddling a ring's wrap deposits the same total as one in the middle of a
/// chart. If no cell is within range (radius below half a cell), the containing cell
/// receives everything.
pub fn deposit(
    topo: Topology,
    scale: Scale,
    field: &mut ScalarField,
    center: SurfacePoint,
    radius: f64,
    amount: f64,
) -> usize {
    debug_assert!(center.is_canonical(topo), "deposit at non-canonical {center:?}");
    let mut images: Vec<ChartImage> = Vec::new();
    chart_images(topo, center.face, MAX_SEAMS, &mut images);

    // (cell, raised-cosine weight) for every cell center inside the footprint.
    let mut touched: Vec<(CellId, f64)> = Vec::new();
    let mut total = 0.0f64;
    if radius > 0.0 {
        for cell in CellId::all(topo, scale) {
            let Some(u) = unfold_with(topo, &images, center, cell.center(topo, scale), radius) else {
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
        field.add(cell_of(topo, scale, &center), amount);
        return 1;
    }
    let share = amount / total;
    for &(cell, w) in &touched {
        field.add(cell, w * share);
    }
    touched.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FACE_EXTENT, Vec2, pixel_neighbor, travel};

    fn graph() -> FieldGraph {
        FieldGraph::new(Topology::Cube, Scale::ONE)
    }

    #[test]
    fn cell_ids_round_trip() {
        for cell in CellId::all(Topology::Cube, Scale::ONE) {
            assert_eq!(CellId::new(Topology::Cube, Scale::ONE, cell.face(Topology::Cube, Scale::ONE), cell.cx(Topology::Cube, Scale::ONE), cell.cy(Topology::Cube, Scale::ONE)), cell);
            assert_eq!(cell_of(Topology::Cube, Scale::ONE, &cell.center(Topology::Cube, Scale::ONE)), cell);
        }
        assert_eq!(CellId::all(Topology::Cube, Scale::ONE).count(), CUBE_CELL_COUNT);
        assert_eq!(CUBE_CELL_COUNT, 1280);
    }

    #[test]
    fn graph_counts_and_degrees() {
        let g = graph();
        assert_eq!(g.edges().len(), 2528, "2,400 in-chart + 128 seam edges");
        let mut rim = 0;
        for cell in CellId::all(Topology::Cube, Scale::ONE) {
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
        let seam_edges = g.edges().iter().filter(|(a, b)| a.face(Topology::Cube, Scale::ONE) != b.face(Topology::Cube, Scale::ONE)).count();
        assert_eq!(seam_edges, 128);
    }

    #[test]
    fn adjacency_is_reciprocal_and_has_no_duplicates() {
        let g = graph();
        for a in CellId::all(Topology::Cube, Scale::ONE) {
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
        for cell in CellId::all(Topology::Cube, Scale::ONE) {
            for edge in Edge::ALL {
                let open = g.neighbor(cell, edge).is_none();
                let is_rim = edge == Edge::Bottom
                    && cell.cy(Topology::Cube, Scale::ONE) == 15
                    && cell.face(Topology::Cube, Scale::ONE) != Face::Top;
                assert_eq!(open, is_rim, "{cell:?} {edge:?}");
            }
        }
    }

    #[test]
    fn seam_neighbours_agree_with_pixel_adjacency() {
        // The cell graph must be the cell-resolution shadow of the pixel-level seam
        // contract: the cell across an edge contains the pixel across that edge.
        let g = graph();
        for cell in CellId::all(Topology::Cube, Scale::ONE) {
            for edge in Edge::ALL {
                // The first pixel of this cell's along-edge run, on the cell's own
                // side of `edge`.
                let (px, py) = match edge {
                    Edge::Top => (cell.cx(Topology::Cube, Scale::ONE) * 4, cell.cy(Topology::Cube, Scale::ONE) * 4),
                    Edge::Right => (cell.cx(Topology::Cube, Scale::ONE) * 4 + 3, cell.cy(Topology::Cube, Scale::ONE) * 4),
                    Edge::Bottom => (cell.cx(Topology::Cube, Scale::ONE) * 4, cell.cy(Topology::Cube, Scale::ONE) * 4 + 3),
                    Edge::Left => (cell.cx(Topology::Cube, Scale::ONE) * 4, cell.cy(Topology::Cube, Scale::ONE) * 4),
                };
                let want = pixel_neighbor(Topology::Cube, cell.face(Topology::Cube, Scale::ONE), px, py, edge)
                    .map(|(f, x, y)| CellId::new(Topology::Cube, Scale::ONE, f, x / 4, y / 4));
                assert_eq!(g.neighbor(cell, edge), want, "{cell:?} {edge:?}");
            }
        }
    }

    #[test]
    fn top_seams_reverse_at_cell_resolution() {
        let g = graph();
        // Right.top cell k joins Top's right column cell 15 - k.
        for k in 0..16u16 {
            let a = CellId::new(Topology::Cube, Scale::ONE, Face::Right, k, 0);
            let b = g.neighbor(a, Edge::Top).expect("seam");
            assert_eq!((b.face(Topology::Cube, Scale::ONE), b.cx(Topology::Cube, Scale::ONE), b.cy(Topology::Cube, Scale::ONE)), (Face::Top, 15, 15 - k), "k={k}");
        }
        // Back.top cell k joins Top's top row cell 15 - k.
        for k in 0..16u16 {
            let a = CellId::new(Topology::Cube, Scale::ONE, Face::Back, k, 0);
            let b = g.neighbor(a, Edge::Top).expect("seam");
            assert_eq!((b.face(Topology::Cube, Scale::ONE), b.cx(Topology::Cube, Scale::ONE), b.cy(Topology::Cube, Scale::ONE)), (Face::Top, 15 - k, 0), "k={k}");
        }
        // Left.top cell k joins Top's left column cell k (not reversed, but twisted).
        for k in 0..16u16 {
            let a = CellId::new(Topology::Cube, Scale::ONE, Face::Left, k, 0);
            let b = g.neighbor(a, Edge::Top).expect("seam");
            assert_eq!((b.face(Topology::Cube, Scale::ONE), b.cx(Topology::Cube, Scale::ONE), b.cy(Topology::Cube, Scale::ONE)), (Face::Top, 0, k), "k={k}");
        }
        // Front.top cell k joins Top's bottom row cell k.
        for k in 0..16u16 {
            let a = CellId::new(Topology::Cube, Scale::ONE, Face::Front, k, 0);
            let b = g.neighbor(a, Edge::Top).expect("seam");
            assert_eq!((b.face(Topology::Cube, Scale::ONE), b.cx(Topology::Cube, Scale::ONE), b.cy(Topology::Cube, Scale::ONE)), (Face::Top, k, 15), "k={k}");
        }
    }

    #[test]
    fn cell_centers_are_neighbours_on_the_surface() {
        // Every graph edge joins cells whose centers are four pixels apart across the
        // real surface, which is an independent check of the seam wiring.
        let g = graph();
        for &(a, b) in g.edges() {
            let d = crate::surface_distance(Topology::Cube, a.center(Topology::Cube, Scale::ONE), b.center(Topology::Cube, Scale::ONE), 8.0);
            let d = d.unwrap_or_else(|| panic!("{a:?} and {b:?} are not within 8 pixels"));
            assert!((d - 4.0).abs() < 1e-9, "{a:?} {b:?}: {d}");
        }
    }

    /// The downhill table of [`FieldGraph::downhill`], clause by clause.
    #[test]
    fn downhill_points_one_step_down_the_side_faces_only() {
        let g = graph();
        let y = |c: CellId| Topology::Cube.embed(Scale::ONE, &c.center(Topology::Cube, Scale::ONE))[1];
        let sides = [Face::Front, Face::Right, Face::Back, Face::Left];

        // Clause 1: the canopy is level, so no Top-face cell slides, seam-adjacent or not.
        for cx in 0..16u16 {
            for cy in 0..16u16 {
                let c = CellId::new(Topology::Cube, Scale::ONE, Face::Top, cx, cy);
                assert_eq!(g.downhill(c), None, "{c:?} on the level top face");
                assert_eq!(y(c), 1.0, "top-face centers all sit at y = 1");
            }
        }

        for face in sides {
            for cx in 0..16u16 {
                // Clause 3: the bottom row has nothing below it.
                let rim = CellId::new(Topology::Cube, Scale::ONE, face, cx, 15);
                assert_eq!(g.downhill(rim), None, "{rim:?} is on the open rim");

                // Clause 2: everything above it steps down exactly one row.
                for cy in 0..15u16 {
                    let c = CellId::new(Topology::Cube, Scale::ONE, face, cx, cy);
                    let d = g.downhill(c).unwrap_or_else(|| panic!("{c:?} has no downhill"));
                    assert!(y(d) < y(c) - 1e-9, "{c:?} -> {d:?}: {} !< {}", y(d), y(c));
                    assert!(
                        g.neighbors(c).iter().flatten().all(|m| y(*m) >= y(d)),
                        "{c:?} -> {d:?} is not the lowest neighbour"
                    );
                    assert_ne!(d.face(Topology::Cube, Scale::ONE), Face::Top, "{c:?} must never drain onto the canopy");
                    assert_eq!(
                        (d.face(Topology::Cube, Scale::ONE), d.cx(Topology::Cube, Scale::ONE), d.cy(Topology::Cube, Scale::ONE)),
                        (face, cx, cy + 1),
                        "{c:?} should step to the cell directly below it"
                    );
                }
            }
        }

        // The relation is acyclic and terminates in the bottom row: follow it to the end.
        for cell in CellId::all(Topology::Cube, Scale::ONE) {
            let mut c = cell;
            let mut steps = 0;
            while let Some(next) = g.downhill(c) {
                c = next;
                steps += 1;
                assert!(steps <= 16, "{cell:?} did not reach the rim in 16 steps");
            }
            if cell.face(Topology::Cube, Scale::ONE) == Face::Top {
                assert_eq!(steps, 0);
            } else {
                assert_eq!(steps, 15 - usize::from(cell.cy(Topology::Cube, Scale::ONE)), "{cell:?}");
                assert_eq!((c.face(Topology::Cube, Scale::ONE), c.cy(Topology::Cube, Scale::ONE)), (cell.face(Topology::Cube, Scale::ONE), 15), "{cell:?} ended at {c:?}");
            }
        }
    }

    #[test]
    fn constant_fields_stay_bit_identical() {
        let g = graph();
        let mut scratch = ScalarField::zeros(Topology::Cube, Scale::ONE);
        for rate in [0.05, 0.25, 0.5, 1.0, 3.0] {
            let mut f = ScalarField::constant(Topology::Cube, Scale::ONE, 1.25);
            let n = diffuse(&mut f, &mut scratch, &g, rate);
            assert!(n >= 1);
            assert_eq!(f, ScalarField::constant(Topology::Cube, Scale::ONE, 1.25), "rate {rate}");
        }
    }

    #[test]
    fn diffusion_conserves_mass_and_nonnegativity() {
        let g = graph();
        let mut scratch = ScalarField::zeros(Topology::Cube, Scale::ONE);
        let mut f = ScalarField::zeros(Topology::Cube, Scale::ONE);
        // Sources at a top vertex, at the rim, and in the middle of a face.
        f.set(CellId::new(Topology::Cube, Scale::ONE, Face::Front, 15, 0), 100.0);
        f.set(CellId::new(Topology::Cube, Scale::ONE, Face::Front, 8, 15), 40.0);
        f.set(CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8), 7.5);
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
        let mut scratch = ScalarField::zeros(Topology::Cube, Scale::ONE);
        let mut f = ScalarField::zeros(Topology::Cube, Scale::ONE);
        f.set(CellId::new(Topology::Cube, Scale::ONE, Face::Front, 4, 4), 1.0);
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
            for cx in 0..16u16 {
                assert_eq!(g.neighbor(CellId::new(Topology::Cube, Scale::ONE, face, cx, 15), Edge::Bottom), None);
            }
        }
        // And a field that is uniform above the rim does not leak mass out of it.
        let mut scratch = ScalarField::zeros(Topology::Cube, Scale::ONE);
        let mut f = ScalarField::zeros(Topology::Cube, Scale::ONE);
        for cx in 0..16u16 {
            f.set(CellId::new(Topology::Cube, Scale::ONE, Face::Front, cx, 15), 1.0);
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
            let mut f = ScalarField::zeros(Topology::Cube, Scale::ONE);
            let n = deposit(Topology::Cube, Scale::ONE, &mut f, center, radius, 25.0);
            assert!(n > 0);
            assert!(
                (f.total() - 25.0).abs() < 1e-9,
                "{center:?} r={radius}: {} over {n} cells",
                f.total()
            );
            assert!(f.is_nonnegative());
            // Everything landed near the center.
            for cell in CellId::all(Topology::Cube, Scale::ONE) {
                if f.get(cell) > 0.0 {
                    let d = crate::surface_distance(Topology::Cube, center, cell.center(Topology::Cube, Scale::ONE), radius);
                    assert!(d.is_some_and(|d| d <= radius), "{cell:?} is outside the footprint");
                }
            }
        }
    }

    #[test]
    fn a_footprint_near_a_seam_deposits_the_same_total_as_one_in_the_middle() {
        let mut middle = ScalarField::zeros(Topology::Cube, Scale::ONE);
        deposit(Topology::Cube, Scale::ONE, &mut middle, SurfacePoint::new(Face::Front, 32.0, 32.0), 9.0, 1.0);
        let mut seam = ScalarField::zeros(Topology::Cube, Scale::ONE);
        deposit(Topology::Cube, Scale::ONE, &mut seam, SurfacePoint::new(Face::Front, 63.9, 32.0), 9.0, 1.0);
        let mut rim = ScalarField::zeros(Topology::Cube, Scale::ONE);
        deposit(Topology::Cube, Scale::ONE, &mut rim, SurfacePoint::new(Face::Front, 32.0, 63.9), 9.0, 1.0);
        for f in [&middle, &seam, &rim] {
            assert!((f.total() - 1.0).abs() < 1e-9, "{}", f.total());
        }
        // The seam-spanning footprint really does straddle two charts.
        assert!(seam.values.iter().enumerate().any(|(i, &x)| x > 0.0 && i >= 256));
    }

    #[test]
    fn a_tiny_radius_falls_back_to_the_containing_cell() {
        let mut f = ScalarField::zeros(Topology::Cube, Scale::ONE);
        let center = SurfacePoint::new(Face::Left, 5.0, 7.0);
        let n = deposit(Topology::Cube, Scale::ONE, &mut f, center, 0.5, 3.0);
        assert_eq!(n, 1);
        assert_eq!(f.get(cell_of(Topology::Cube, Scale::ONE, &center)), 3.0);
        assert!((f.total() - 3.0).abs() < 1e-12);
    }

    #[test]
    fn a_deposit_moved_over_a_seam_keeps_its_shape() {
        // Sliding the same footprint across the Front/Right seam must not change the
        // total, and the weight profile must stay smooth: track the peak cell value.
        let mut peaks = Vec::new();
        for u in [56.0, 60.0, 63.0, FACE_EXTENT - 0.01] {
            let mut f = ScalarField::zeros(Topology::Cube, Scale::ONE);
            // Sliding the point there with `travel` keeps it canonical, seam or not.
            let start = travel(Topology::Cube, SurfacePoint::new(Face::Front, 32.0, 32.0), Vec2::new(u - 32.0, 0.0)).end;
            deposit(Topology::Cube, Scale::ONE, &mut f, start, 9.0, 1.0);
            assert!((f.total() - 1.0).abs() < 1e-9);
            peaks.push(f.max());
        }
        for w in peaks.windows(2) {
            assert!((w[0] - w[1]).abs() < 0.02, "{peaks:?}");
        }
    }
}

#[cfg(test)]
mod ring_tests {
    //! The ring's field graph: **every row is a ring**, so there are no corners — the
    //! degree-3 cells are the top and bottom rows and nothing else.

    use super::*;
    use crate::Vec2;

    const RING: Topology = Topology::Ring { w: 320, h: 180 };
    const RING2: Topology = Topology::Ring { w: 640, h: 360 };
    const S1: Scale = Scale::ONE;
    const S2: Scale = Scale::new(2.0);

    fn cell(topo: Topology, scale: Scale, cx: u16, cy: u16) -> CellId {
        CellId::new(topo, scale, Face::Front, cx, cy)
    }

    #[test]
    fn the_grid_is_eighty_by_forty_five_at_both_scales() {
        for (topo, scale) in [(RING, S1), (RING2, S2)] {
            assert_eq!(topo.cells(scale, Face::Front), (80, 45));
            assert_eq!(topo.cell_count(scale), 3600);
            assert_eq!(CellId::all(topo, scale).count(), 3600);
            for c in CellId::all(topo, scale) {
                let (cx, cy) = (c.cx(topo, scale), c.cy(topo, scale));
                assert_eq!(c.face(topo, scale), Face::Front);
                assert_eq!(cell(topo, scale, cx, cy), c);
                assert_eq!(c.index(), usize::from(cy) * 80 + usize::from(cx));
                // The centre is half a cell in, and maps back to its own cell.
                let centre = c.center(topo, scale);
                let cp = scale.cell_pixels();
                assert_eq!(centre.u, f64::from(cx) * cp + cp / 2.0);
                assert_eq!(centre.v, f64::from(cy) * cp + cp / 2.0);
                assert_eq!(cell_of(topo, scale, &centre), c);
            }
            // Every pixel of a cell maps back to it.
            let cp = scale.cell_pixels() as u16;
            for &(cx, cy) in &[(0u16, 0u16), (79, 44), (37, 21)] {
                for dy in 0..cp {
                    for dx in 0..cp {
                        let p = SurfacePoint::pixel_center(topo, Face::Front, cx * cp + dx, cy * cp + dy);
                        assert_eq!(cell_of(topo, scale, &p), cell(topo, scale, cx, cy));
                    }
                }
            }
        }
    }

    #[test]
    fn every_row_is_a_ring_and_there_are_no_corners() {
        for (topo, scale) in [(RING, S1), (RING2, S2)] {
            let g = FieldGraph::new(topo, scale);
            // 80·45 horizontal (the wrap adds one per row) + 80·44 vertical.
            assert_eq!(g.edges().len(), 7120, "{topo:?}");
            let (mut deg4, mut deg3) = (0, 0);
            for c in CellId::all(topo, scale) {
                match g.degree(c) {
                    4 => deg4 += 1,
                    3 => {
                        deg3 += 1;
                        let cy = c.cy(topo, scale);
                        assert!(cy == 0 || cy == 44, "{c:?} has degree 3 away from a rim");
                    }
                    d => panic!("{c:?} has degree {d}: a ring has no corners"),
                }
            }
            assert_eq!((deg4, deg3), (3440, 160), "{topo:?}");
            // Sorted, unique, canonically ordered, and all within the one chart.
            for w in g.edges().windows(2) {
                assert!(w[0] < w[1]);
            }
            for &(a, b) in g.edges() {
                assert!(a < b);
                assert_eq!(a.face(topo, scale), Face::Front);
                assert_eq!(b.face(topo, scale), Face::Front);
            }
            assert_eq!(g.topology(), topo);
            assert_eq!(g.cell_count(), 3600);
        }
    }

    #[test]
    fn the_wrap_joins_the_two_ends_of_every_row() {
        let g = FieldGraph::new(RING, S1);
        for cy in 0..45u16 {
            let last = cell(RING, S1, 79, cy);
            let first = cell(RING, S1, 0, cy);
            assert_eq!(g.neighbor(last, Edge::Right), Some(first), "row {cy}");
            assert_eq!(g.neighbor(first, Edge::Left), Some(last), "row {cy}");
            // Reciprocal, and not a self-loop or a duplicate.
            assert_ne!(first, last);
            assert_eq!(g.neighbor(first, Edge::Right), Some(cell(RING, S1, 1, cy)));
        }
        // The rims have no neighbour, and only the rims.
        for cx in 0..80u16 {
            assert_eq!(g.neighbor(cell(RING, S1, cx, 0), Edge::Top), None);
            assert_eq!(g.neighbor(cell(RING, S1, cx, 44), Edge::Bottom), None);
            assert_eq!(g.neighbor(cell(RING, S1, cx, 0), Edge::Bottom), Some(cell(RING, S1, cx, 1)));
        }
        let missing: usize = CellId::all(RING, S1)
            .map(|c| g.neighbors(c).iter().filter(|n| n.is_none()).count())
            .sum();
        assert_eq!(missing, 160, "only the two rims are open");
    }

    /// `design/flat-world-plan-2026-09-16.md` §5: the top row is the canopy and never
    /// drains, the bottom row has nothing below it, and everything between steps to
    /// `(cx, cy + 1)`.
    #[test]
    fn downhill_is_none_on_the_canopy_row_and_the_cell_below_everywhere_else() {
        for (topo, scale) in [(RING, S1), (RING2, S2)] {
            let g = FieldGraph::new(topo, scale);
            for cx in 0..80u16 {
                assert_eq!(g.downhill(cell(topo, scale, cx, 0)), None, "the canopy drains");
                assert_eq!(g.downhill(cell(topo, scale, cx, 44)), None, "the floor drains");
                for cy in 1..44u16 {
                    let c = cell(topo, scale, cx, cy);
                    let d = g.downhill(c).unwrap_or_else(|| panic!("{c:?} has no downhill"));
                    assert_eq!(d, cell(topo, scale, cx, cy + 1), "{c:?}");
                    // Downhill really is downhill in the height the world uses.
                    assert!(topo.height(&d.center(topo, scale)) < topo.height(&c.center(topo, scale)));
                }
            }
            // Acyclic and terminating: following it always reaches the floor.
            let mut c = cell(topo, scale, 13, 1);
            let mut steps = 0;
            while let Some(next) = g.downhill(c) {
                c = next;
                steps += 1;
                assert!(steps <= 45, "downhill does not terminate");
            }
            assert_eq!(c.cy(topo, scale), 44);
            assert_eq!(steps, 43);
        }
    }

    #[test]
    fn diffusion_conserves_mass_and_leaks_through_neither_rim() {
        let g = FieldGraph::new(RING, S1);
        let mut f = ScalarField::zeros(RING, S1);
        let mut scratch = ScalarField::zeros(RING, S1);
        assert_eq!(f.len(), 3600);
        // A line of mass right on the wrap, and a line on each rim.
        for cy in 0..45u16 {
            f.set(cell(RING, S1, 0, cy), 1.0);
        }
        for cx in 0..80u16 {
            f.set(cell(RING, S1, cx, 0), 1.0);
            f.set(cell(RING, S1, cx, 44), 1.0);
        }
        let start = f.total();
        for _ in 0..200 {
            diffuse(&mut f, &mut scratch, &g, 0.9);
        }
        assert!((f.total() - start).abs() < 1e-9, "{} != {start}", f.total());
        assert!(f.is_nonnegative() && f.is_finite());
        // Mass spread the whole way round the ring, not into a wall.
        assert!(f.get(cell(RING, S1, 40, 22)) > 0.0, "nothing reached the far side");
        // A constant field is bit-identical after a step.
        let mut c = ScalarField::constant(RING, S1, 0.37);
        let before = c.clone();
        diffuse(&mut c, &mut scratch, &g, 0.25);
        assert_eq!(c, before);
    }

    /// A footprint deposits the same total wherever it lands, including straddling the
    /// wrap and clipped by a rim.
    #[test]
    fn deposit_normalizes_over_the_cells_that_exist() {
        for (topo, scale) in [(RING, S1), (RING2, S2)] {
            let (w, h) = topo.extent(Face::Front);
            let places = [
                (w / 2.0, h / 2.0),
                (0.5, h / 2.0),
                (w - 0.5, h / 2.0),
                (w / 2.0, 0.5),
                (0.5, 0.5),
                (w - 0.5, h - 0.5),
            ];
            let radius = scale.footprint_radius();
            for (u, v) in places {
                let mut f = ScalarField::zeros(topo, scale);
                let centre = SurfacePoint::new(Face::Front, u, v).canonicalize(topo);
                let touched = deposit(topo, scale, &mut f, centre, radius, 3.0);
                assert!(touched > 1, "{centre:?} touched {touched} cells");
                assert!((f.total() - 3.0).abs() < 1e-9, "{centre:?}: {}", f.total());
                assert!(f.is_nonnegative());
            }
            // Straddling the wrap reaches cells on both sides of it.
            let mut f = ScalarField::zeros(topo, scale);
            deposit(topo, scale, &mut f, SurfacePoint::new(Face::Front, 0.5, h / 2.0), radius, 1.0);
            assert!(f.get(cell(topo, scale, 79, 22)) > 0.0, "the wrap blocked the footprint");
            assert!(f.get(cell(topo, scale, 0, 22)) > 0.0);
        }
    }

    #[test]
    fn a_walker_that_reflects_at_the_rims_stays_inside() {
        // The rim is reflective, not absorbing and not a teleport: a long random walk
        // never leaves the chart and never stops moving.
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut p = SurfacePoint::new(Face::Front, 160.5, 90.5);
        let mut wrapped = 0;
        let mut bounced = 0;
        for _ in 0..20_000 {
            let step = Vec2::new((rnd() - 0.5) * 30.0, (rnd() - 0.5) * 30.0);
            let t = crate::travel(RING, p, step);
            assert!(!t.fallback);
            assert!(t.end.is_canonical(RING), "{:?}", t.end);
            wrapped += t.crossings;
            bounced += t.reflections;
            p = t.end;
        }
        assert!(wrapped > 100, "only {wrapped} wraps");
        assert!(bounced > 100, "only {bounced} bounces");
    }
}
