//! The pixel→cell map, built once instead of per pixel per pass (FW-P's W1).
//!
//! Four of the presenter's passes ask, for every pixel of the raster, "which cell am I in,
//! and which cells are my four neighbours in?" — and answer it with
//! `cell_of(topo, scale, &SurfacePoint::pixel_center(topo, face, x, y))` and
//! `pixel_neighbor(..)`, once per pixel per pass, every frame. That is 11.8 % of the cube
//! frame (1.64 ms on the board) spent recomputing a constant: the map depends only on the
//! topology and the scale, both of which are fixed for a world's life.
//!
//! [`PixelCells`] is that constant, tabulated. Five `u16` per pixel — the own cell and the
//! four [`Edge::ALL`] neighbours, `ABSENT` where the neighbour is off a rim — so the cube's
//! table is 200 KiB and a 640×360 ring's is 2.3 MiB, and every query is one indexed load.
//!
//! It answers **exactly** what the recomputation answers, in the same order: the box filter
//! sums the four neighbours in `Edge::ALL` order, so the floating-point addition order —
//! and therefore the last bit of every filtered pixel — is unchanged.

use cubarium_surface::{
    CellId, Edge, Face, ScalarField, Scale, SurfacePoint, Topology, cell_of, pixel_neighbor,
};

/// The entry for a neighbour that does not exist (an open rim).
const ABSENT: u16 = u16::MAX;

/// Every pixel's own cell and its four pixel-neighbours' cells, for one topology and scale.
#[derive(Clone)]
pub struct PixelCells {
    topo: Topology,
    scale: Scale,
    w: u16,
    h: u16,
    /// `[own, up, right, down, left]` in [`Edge::ALL`] order, `ABSENT` where absent.
    table: Vec<[u16; 5]>,
}

impl PixelCells {
    /// Build the table. Costs one `cell_of` per pixel per neighbour, once.
    pub fn new(topo: Topology, scale: Scale) -> PixelCells {
        let (w, h) = match topo {
            Topology::Cube => (64u16, 64u16),
            Topology::Ring { w, h } => (w, h),
        };
        let mut table = Vec::with_capacity(topo.charts().len() * usize::from(w) * usize::from(h));
        for &face in topo.charts() {
            for y in 0..h {
                for x in 0..w {
                    let mut entry = [ABSENT; 5];
                    entry[0] = cell_of(topo, scale, &SurfacePoint::pixel_center(topo, face, x, y)).0;
                    for (i, edge) in Edge::ALL.into_iter().enumerate() {
                        if let Some((nf, nx, ny)) = pixel_neighbor(topo, face, x, y, edge) {
                            let p = SurfacePoint::pixel_center(topo, nf, nx, ny);
                            entry[i + 1] = cell_of(topo, scale, &p).0;
                        }
                    }
                    table.push(entry);
                }
            }
        }
        PixelCells { topo, scale, w, h, table }
    }

    #[inline]
    pub fn topology(&self) -> Topology {
        self.topo
    }

    #[inline]
    pub fn scale(&self) -> Scale {
        self.scale
    }

    /// Whether this table was built for the same world as `canvas`.
    #[inline]
    pub fn fits(&self, topo: Topology, scale: Scale) -> bool {
        self.topo == topo && self.scale == scale
    }

    #[inline]
    fn index(&self, face: Face, x: u16, y: u16) -> usize {
        debug_assert!(x < self.w && y < self.h);
        (self.topo.chart_index(face) * usize::from(self.h) + usize::from(y)) * usize::from(self.w)
            + usize::from(x)
    }

    /// The cell this pixel's centre lies in.
    #[inline]
    pub fn cell(&self, face: Face, x: u16, y: u16) -> CellId {
        CellId(self.table[self.index(face, x, y)][0])
    }

    /// The field value at this pixel's own cell — the unfiltered sample.
    #[inline]
    pub fn value(&self, field: &ScalarField, face: Face, x: u16, y: u16) -> f64 {
        field.get(self.cell(face, x, y))
    }

    /// The seam-aware one-pixel box filter: the own cell weighted 4, each *existing* pixel
    /// neighbour's cell weighted 1, divided by the weight actually present so an open rim
    /// is not darkened.
    ///
    /// Summed in [`Edge::ALL`] order, which is the order the per-pixel recomputation used,
    /// so the result is bit-identical rather than merely equal.
    #[inline]
    pub fn filtered(&self, field: &ScalarField, face: Face, x: u16, y: u16) -> f64 {
        let entry = &self.table[self.index(face, x, y)];
        let mut sum = field.get(CellId(entry[0])) * 4.0;
        let mut weight = 4.0;
        for &n in &entry[1..] {
            if n != ABSENT {
                sum += field.get(CellId(n));
                weight += 1.0;
            }
        }
        sum / weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table answers what the per-pixel recomputation answers, for every pixel of both
    /// topologies, own cell and all four neighbours.
    #[test]
    fn the_table_is_the_recomputation() {
        for (topo, scale) in [
            (Topology::Cube, Scale::ONE),
            (Topology::Ring { w: 320, h: 180 }, Scale::ONE),
            (Topology::Ring { w: 96, h: 48 }, Scale::new(2.0)),
        ] {
            let cells = PixelCells::new(topo, scale);
            let (w, h) = topo.extent(topo.charts()[0]);
            for &face in topo.charts() {
                for y in 0..h as u16 {
                    for x in 0..w as u16 {
                        let want = cell_of(topo, scale, &SurfacePoint::pixel_center(topo, face, x, y));
                        assert_eq!(cells.cell(face, x, y), want, "{topo:?} {face:?} ({x}, {y})");
                        let entry = &cells.table[cells.index(face, x, y)];
                        for (i, edge) in Edge::ALL.into_iter().enumerate() {
                            let got = entry[i + 1];
                            match pixel_neighbor(topo, face, x, y, edge) {
                                None => assert_eq!(got, ABSENT, "{face:?} ({x},{y}) {edge:?}"),
                                Some((nf, nx, ny)) => {
                                    let p = SurfacePoint::pixel_center(topo, nf, nx, ny);
                                    assert_eq!(CellId(got), cell_of(topo, scale, &p));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// And the filter it computes is bit-identical to the open-coded one, on a field with
    /// values that do not sum associatively.
    #[test]
    fn the_filter_is_bit_identical_to_the_open_coded_one() {
        let topo = Topology::Cube;
        let scale = Scale::ONE;
        let cells = PixelCells::new(topo, scale);
        let mut field = ScalarField::zeros(topo, scale);
        for (i, v) in field.values.iter_mut().enumerate() {
            *v = (i as f64 * 0.417_913_1).sin() * 1e7 + (i % 3) as f64 * 1e-9;
        }
        for &face in topo.charts() {
            for y in 0..64u16 {
                for x in 0..64u16 {
                    let mut sum = field
                        .get(cell_of(topo, scale, &SurfacePoint::pixel_center(topo, face, x, y)))
                        * 4.0;
                    let mut weight = 4.0;
                    for edge in Edge::ALL {
                        if let Some((nf, nx, ny)) = pixel_neighbor(topo, face, x, y, edge) {
                            let p = SurfacePoint::pixel_center(topo, nf, nx, ny);
                            sum += field.get(cell_of(topo, scale, &p));
                            weight += 1.0;
                        }
                    }
                    let want = sum / weight;
                    let got = cells.filtered(&field, face, x, y);
                    assert_eq!(got.to_bits(), want.to_bits(), "{face:?} ({x}, {y})");
                }
            }
        }
    }
}
