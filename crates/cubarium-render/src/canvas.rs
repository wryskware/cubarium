//! Linear-light canvas, shaped by the topology, and its 8-bit encodings.
//!
//! A canvas is a stack of equal-sized charts: five 64×64 faces for
//! [`Topology::Cube`], one `w×h` chart for [`Topology::Ring`]. Pixels live in one flat
//! row-major buffer whose *global row* is `chart_index · height + y`, so any contiguous
//! run of global rows is a contiguous slice — which is what makes the deterministic
//! row-band split of §4 a borrow of the buffer rather than a second rasterization.

use cubarium_surface::{PixelImage, Scale, Topology};
use cube_proto::{FACE_SIZE, Face, Frame, NUM_FACES, Raster};

mod srgb;

pub use srgb::{srgb_decode, srgb_encode};

/// A linear-RGB `f32` canvas over one topology's charts, row-major like `Frame`.
///
/// A *band* is the same type over a sub-range of the global rows: it owns its pixels,
/// every write outside its rows is dropped, and [`Canvas::for_each_band`] puts the bands
/// back. See [`Canvas::bands`].
#[derive(Clone)]
pub struct Canvas {
    topo: Topology,
    scale: Scale,
    /// Chart width and height in pixels; every chart of a topology has the same size.
    w: u16,
    h: u16,
    /// Global row of `px[0]`. Zero for a whole canvas.
    row0: u32,
    /// Number of global rows stored, `charts · h` for a whole canvas.
    rows: u32,
    px: Vec<[f32; 3]>,
}

impl Canvas {
    /// A black canvas shaped by `topo` at world scale `scale`.
    ///
    /// The scale is carried rather than derived because every stamp budget
    /// ([`Scale::footprint_radius`]) is measured against it: a canvas *is* the world's
    /// raster, so nothing that draws on it has to be told the scale a second time.
    pub fn new(topo: Topology, scale: Scale) -> Canvas {
        let (w, h) = match topo {
            Topology::Cube => (FACE_SIZE as u16, FACE_SIZE as u16),
            Topology::Ring { w, h } => (w, h),
        };
        let charts = topo.charts().len() as u32;
        let rows = charts * u32::from(h);
        Canvas {
            topo,
            scale,
            w,
            h,
            row0: 0,
            rows,
            px: vec![[0.0; 3]; rows as usize * usize::from(w)],
        }
    }

    /// The five 64×64 faces at `S = 1`: `Canvas::new(Topology::Cube, Scale::ONE)`, which
    /// is every canvas that existed before the topology.
    pub fn cube() -> Canvas {
        Canvas::new(Topology::Cube, Scale::ONE)
    }

    #[inline]
    pub fn topology(&self) -> Topology {
        self.topo
    }

    #[inline]
    pub fn scale(&self) -> Scale {
        self.scale
    }

    /// Chart width in pixels.
    #[inline]
    pub fn width(&self) -> u16 {
        self.w
    }

    /// Chart height in pixels.
    #[inline]
    pub fn height(&self) -> u16 {
        self.h
    }

    /// The charts this canvas draws, in index order.
    #[inline]
    pub fn charts(&self) -> &'static [Face] {
        self.topo.charts()
    }

    /// The global rows this canvas owns: `0..charts·h` for a whole canvas, the band's own
    /// rows for a band.
    #[inline]
    pub fn band(&self) -> std::ops::Range<u32> {
        self.row0..self.row0 + self.rows
    }

    /// The rows of `face` this canvas owns — the whole chart unless this is a band, and
    /// empty for a chart the band does not reach. The loop bound every per-pixel pass
    /// uses, so that a band walks only its own pixels instead of rejecting the rest.
    #[inline]
    pub fn rows_of(&self, face: Face) -> std::ops::Range<u16> {
        if !self.topo.has_chart(face) {
            return 0..0;
        }
        let base = self.topo.chart_index(face) as u32 * u32::from(self.h);
        let lo = self.row0.max(base);
        let hi = (self.row0 + self.rows).min(base + u32::from(self.h));
        if lo >= hi {
            return 0..0;
        }
        (lo - base) as u16..(hi - base) as u16
    }

    /// Whether this canvas stores the pixel `(face, _, y)`.
    #[inline]
    pub fn owns(&self, face: Face, y: u16) -> bool {
        self.topo.has_chart(face) && self.rows_of(face).contains(&y)
    }

    /// Every pixel this canvas owns, in storage order: charts outermost, then `y`, then
    /// `x`. One line in place of the `for face { for y { for x` of the cube.
    pub fn coords(&self) -> impl Iterator<Item = (Face, u16, u16)> + '_ {
        let w = self.w;
        self.charts().iter().flat_map(move |&face| {
            self.rows_of(face)
                .flat_map(move |y| (0..w).map(move |x| (face, x, y)))
        })
    }

    /// The linear pixels this canvas stores, row-major, `charts · h` rows of `w`. For a
    /// ring this *is* the `w×h` image; for the cube it is the five faces end to end.
    #[inline]
    pub fn pixels(&self) -> &[[f32; 3]] {
        &self.px
    }

    #[inline]
    pub fn pixels_mut(&mut self) -> &mut [[f32; 3]] {
        &mut self.px
    }

    pub fn clear(&mut self) {
        self.px.fill([0.0; 3]);
    }

    /// The storage slot of a pixel, or `None` when a band does not own it. Panics when
    /// the coordinate is outside the chart, exactly as the fixed five-face array did.
    #[inline]
    fn slot(&self, face: Face, x: u16, y: u16) -> Option<usize> {
        assert!(
            x < self.w && y < self.h,
            "canvas pixel ({x}, {y}) is outside a {}x{} chart",
            self.w,
            self.h
        );
        let row = self.topo.chart_index(face) as u32 * u32::from(self.h) + u32::from(y);
        let r = row.wrapping_sub(self.row0);
        (r < self.rows).then(|| r as usize * usize::from(self.w) + usize::from(x))
    }

    /// The pixel's linear value; black for a pixel a band does not own.
    #[inline]
    pub fn get(&self, face: Face, x: u16, y: u16) -> [f32; 3] {
        match self.slot(face, x, y) {
            Some(i) => self.px[i],
            None => [0.0; 3],
        }
    }

    /// Writes outside a band's rows are dropped: that is what clips a stamp to its band.
    #[inline]
    pub fn set(&mut self, face: Face, x: u16, y: u16, rgb: [f32; 3]) {
        if let Some(i) = self.slot(face, x, y) {
            self.px[i] = rgb;
        }
    }

    /// Additive blend (linear light adds); clamping happens at encode time.
    #[inline]
    pub fn add(&mut self, face: Face, x: u16, y: u16, rgb: [f32; 3]) {
        if let Some(i) = self.slot(face, x, y) {
            let p = &mut self.px[i];
            p[0] += rgb[0];
            p[1] += rgb[1];
            p[2] += rgb[2];
        }
    }

    /// The run of an unfolded stamp's pixels this canvas owns.
    ///
    /// `cubarium_surface::unfold_pixels` emits its pixels sorted by `(chart, y, x)`, which
    /// is global-row order, so a band's share is one contiguous sub-slice found by two
    /// partition points — no per-pixel ownership test in the stamp loop, and for a whole
    /// canvas the slice is the whole list.
    #[inline]
    pub fn band_pixels<'a>(&self, pixels: &'a [PixelImage]) -> &'a [PixelImage] {
        if self.row0 == 0 && self.rows == self.topo.charts().len() as u32 * u32::from(self.h) {
            return pixels;
        }
        let row = |p: &PixelImage| {
            self.topo.chart_index(p.face) as u32 * u32::from(self.h) + u32::from(p.y)
        };
        let lo = pixels.partition_point(|p| row(p) < self.row0);
        let hi = pixels.partition_point(|p| row(p) < self.row0 + self.rows);
        &pixels[lo..hi]
    }

    /// Encode into the shim cube frame: clamp to `[0, 1]`, sRGB transfer, round to `u8`.
    /// Deterministic and total (NaN encodes as 0). Cube canvases only.
    pub fn encode(&self, frame: &mut Frame) {
        assert!(
            matches!(self.topo, Topology::Cube) && self.rows == (NUM_FACES * FACE_SIZE) as u32,
            "Canvas::encode writes a cube Frame; this canvas is {:?} rows {:?}",
            self.topo,
            self.band()
        );
        let t = srgb::table();
        for face in Face::ALL {
            let base = face.index() * FACE_SIZE * FACE_SIZE;
            let src = &self.px[base..base + FACE_SIZE * FACE_SIZE];
            let dst = frame.face_mut(face);
            for (px, out) in src.iter().zip(dst.as_chunks_mut::<3>().0) {
                out[0] = t.encode(px[0]);
                out[1] = t.encode(px[1]);
                out[2] = t.encode(px[2]);
            }
        }
    }

    /// Encode into a flat raster: nearest-neighbour resample of this canvas's single
    /// chart, then the same sRGB transfer as [`Canvas::encode`], byte for byte.
    ///
    /// A raster the canvas's own size is a straight 1:1 copy; a larger or smaller one
    /// takes the source pixel `⌊X · w / W⌋, ⌊Y · h / H⌋`, so a whole-number upscale
    /// repeats each pixel exactly and nothing is filtered. Single-chart topologies only —
    /// a cube has no rectangle to write.
    pub fn encode_raster(&self, raster: &mut Raster) {
        assert!(
            self.topo.charts().len() == 1 && self.rows == u32::from(self.h),
            "Canvas::encode_raster writes one rectangle; this canvas is {:?} rows {:?}",
            self.topo,
            self.band()
        );
        let t = srgb::table();
        let (rw, rh) = raster.size();
        let (w, h) = (self.w, self.h);
        for ry in 0..rh {
            let sy = (u32::from(ry) * u32::from(h) / u32::from(rh)).min(u32::from(h) - 1) as usize;
            let src = &self.px[sy * usize::from(w)..][..usize::from(w)];
            let dst = raster.rows_mut(ry, 1).expect("one row is in range");
            for (rx, out) in dst.as_chunks_mut::<3>().0.iter_mut().enumerate() {
                let sx = (rx * usize::from(w) / usize::from(rw)).min(usize::from(w) - 1);
                let px = src[sx];
                out[0] = t.encode(px[0]);
                out[1] = t.encode(px[1]);
                out[2] = t.encode(px[2]);
            }
        }
    }

    // --- the deterministic row-band split ---------------------------------------------

    /// Split this canvas into `n` horizontal bands of global rows, as evenly as the row
    /// count allows and always in the same places for the same `(topology, n)`.
    ///
    /// Each band is a `Canvas` that owns only its rows: a stamp drawn on it paints the
    /// pixels inside the band and drops the rest, so drawing the same sequence on every
    /// band and putting them back ([`Canvas::for_each_band`]) gives *exactly* the serial
    /// image — every pixel is written by exactly one band, in the order it would have
    /// been written serially, from the value it would have had.
    pub fn bands(&self, n: usize) -> Bands {
        let n = n.clamp(1, self.rows.max(1) as usize);
        let mut bands = Vec::with_capacity(n);
        let mut row = self.row0;
        for i in 0..n {
            let rows = self.rows / n as u32 + u32::from((i as u32) < self.rows % n as u32);
            bands.push(Canvas {
                topo: self.topo,
                scale: self.scale,
                w: self.w,
                h: self.h,
                row0: row,
                rows,
                px: vec![[0.0; 3]; rows as usize * usize::from(self.w)],
            });
            row += rows;
        }
        debug_assert_eq!(row, self.row0 + self.rows);
        Bands { bands }
    }

    /// Draw `f` on every band of `bands` and put the result back, serially.
    ///
    /// Each band starts from this canvas's current pixels, so a pass that reads what is
    /// under it sees what it would have seen serially.
    pub fn for_each_band<F: FnMut(&mut Canvas)>(&mut self, bands: &mut Bands, mut f: F) {
        self.split_into(bands);
        for band in &mut bands.bands {
            f(band);
        }
        self.gather(bands);
    }

    /// [`Canvas::for_each_band`] on one scoped thread per band.
    pub fn par_each_band<F: Fn(&mut Canvas) + Sync>(&mut self, bands: &mut Bands, f: F) {
        self.split_into(bands);
        if bands.bands.len() == 1 {
            f(&mut bands.bands[0]);
        } else {
            let f = &f;
            std::thread::scope(|s| {
                for band in &mut bands.bands {
                    s.spawn(move || f(band));
                }
            });
        }
        self.gather(bands);
    }

    /// Seed every band with this canvas's rows. Public so a caller driving its own thread
    /// pool over [`Bands::iter_mut`] can do the same two steps by hand.
    pub fn split_into(&self, bands: &mut Bands) {
        assert!(
            bands
                .bands
                .iter()
                .all(|b| b.topo == self.topo && b.w == self.w && b.h == self.h),
            "bands were cut from a differently shaped canvas"
        );
        for band in &mut bands.bands {
            let o = (band.row0 - self.row0) as usize * usize::from(self.w);
            let n = band.px.len();
            band.px.copy_from_slice(&self.px[o..o + n]);
        }
    }

    /// Copy every band's rows back into this canvas.
    pub fn gather(&mut self, bands: &Bands) {
        for band in &bands.bands {
            let o = (band.row0 - self.row0) as usize * usize::from(self.w);
            self.px[o..o + band.px.len()].copy_from_slice(&band.px);
        }
    }
}

/// The bands of one canvas, allocated once and reused every frame.
pub struct Bands {
    bands: Vec<Canvas>,
}

impl Bands {
    pub fn len(&self) -> usize {
        self.bands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bands.is_empty()
    }

    /// The bands, for a caller that drives its own thread pool: seed them with
    /// [`Canvas::split_into`], draw, then [`Canvas::gather`].
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Canvas> {
        self.bands.iter_mut()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Canvas> {
        self.bands.iter()
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas::cube()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_round_trips_on_every_code() {
        for code in 0..=255u8 {
            let linear = srgb_decode(code);
            assert_eq!(srgb_encode(linear), code, "code {code} decoded to {linear}");
        }
        assert_eq!(srgb_decode(0), 0.0);
        assert!((srgb_decode(255) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn srgb_encode_is_monotone_and_total() {
        let mut prev = 0u8;
        for i in 0..=1000 {
            let v = i as f32 / 1000.0;
            let e = srgb_encode(v);
            assert!(e >= prev, "not monotone at {v}");
            prev = e;
        }
        assert_eq!(srgb_encode(f32::NAN), 0);
        assert_eq!(srgb_encode(-1.0), 0);
        assert_eq!(srgb_encode(f32::NEG_INFINITY), 0);
        assert_eq!(srgb_encode(-0.0), 0);
        assert_eq!(srgb_encode(1.5), 255);
        assert_eq!(srgb_encode(f32::INFINITY), 255);
    }

    #[test]
    fn encode_clamps_nan_negative_and_overflow() {
        let mut c = Canvas::cube();
        c.set(Face::Front, 0, 0, [f32::NAN, -3.0, 12.0]);
        c.set(Face::Top, 63, 63, [1.0, 0.0, 0.5]);
        c.set(
            Face::Back,
            5,
            7,
            [f32::INFINITY, f32::NEG_INFINITY, f32::NAN],
        );
        let mut f = Frame::black();
        // Prefill with a nonzero pattern so encode must overwrite every byte.
        f.fill([77, 77, 77]);
        c.encode(&mut f);
        assert_eq!(f.get(Face::Front, 0, 0), [0, 0, 255]);
        assert_eq!(f.get(Face::Top, 63, 63), [255, 0, srgb_encode(0.5)]);
        assert_eq!(f.get(Face::Back, 5, 7), [255, 0, 0]);
        assert_eq!(f.get(Face::Left, 1, 1), [0, 0, 0]);
        // Nothing left from the prefill.
        assert_eq!(f.as_bytes().iter().filter(|&&b| b == 77).count(), 0);
    }

    #[test]
    fn encode_matches_canvas_values_everywhere() {
        let mut c = Canvas::cube();
        for (i, face) in Face::ALL.into_iter().enumerate() {
            for y in 0..64u16 {
                for x in 0..64u16 {
                    let t = ((i as f32) * 13.0 + f32::from(x) * 0.7 + f32::from(y) * 0.3) % 1.0;
                    c.set(face, x, y, [t, t * 0.5, 1.0 - t]);
                }
            }
        }
        let mut f = Frame::black();
        c.encode(&mut f);
        for face in Face::ALL {
            for y in 0..64u16 {
                for x in 0..64u16 {
                    let v = c.get(face, x, y);
                    let want = [srgb_encode(v[0]), srgb_encode(v[1]), srgb_encode(v[2])];
                    assert_eq!(f.get(face, usize::from(x), usize::from(y)), want);
                }
            }
        }
    }

    #[test]
    fn add_accumulates_and_clear_resets() {
        let mut c = Canvas::cube();
        c.add(Face::Right, 2, 3, [0.1, 0.2, 0.3]);
        c.add(Face::Right, 2, 3, [0.1, 0.2, 0.3]);
        let v = c.get(Face::Right, 2, 3);
        assert!(
            (v[0] - 0.2).abs() < 1e-6 && (v[1] - 0.4).abs() < 1e-6 && (v[2] - 0.6).abs() < 1e-6
        );
        c.clear();
        assert_eq!(c.get(Face::Right, 2, 3), [0.0; 3]);
    }

    #[test]
    fn a_cube_canvas_is_five_faces_and_a_ring_is_one_rectangle() {
        let cube = Canvas::cube();
        assert_eq!(cube.pixels().len(), 5 * 64 * 64);
        assert_eq!(cube.coords().count(), 5 * 64 * 64);
        assert_eq!((cube.width(), cube.height()), (64, 64));

        let ring = Canvas::new(Topology::Ring { w: 320, h: 180 }, Scale::ONE);
        assert_eq!(ring.pixels().len(), 320 * 180);
        assert_eq!(ring.coords().count(), 320 * 180);
        assert_eq!(ring.charts(), &[Face::Front]);
        assert!(!ring.owns(Face::Top, 0), "a ring has no Top chart");
        // Coordinate order is storage order on both.
        let first: Vec<_> = ring.coords().take(3).collect();
        assert_eq!(
            first,
            vec![
                (Face::Front, 0, 0),
                (Face::Front, 1, 0),
                (Face::Front, 2, 0)
            ]
        );
    }

    #[test]
    fn a_ring_encodes_its_raster_1_to_1_and_by_nearest_when_upscaled() {
        let mut c = Canvas::new(Topology::Ring { w: 4, h: 3 }, Scale::ONE);
        for (f, x, y) in c.coords().collect::<Vec<_>>() {
            let t = (f32::from(x) * 0.25 + f32::from(y) * 0.1) % 1.0;
            c.set(f, x, y, [t, 1.0 - t, 0.5]);
        }
        let mut r = Raster::black(4, 3);
        c.encode_raster(&mut r);
        for y in 0..3u16 {
            for x in 0..4u16 {
                let v = c.get(Face::Front, x, y);
                assert_eq!(
                    r.get(x, y),
                    [srgb_encode(v[0]), srgb_encode(v[1]), srgb_encode(v[2])]
                );
            }
        }
        // A 2x raster repeats each pixel exactly.
        let mut up = Raster::black(8, 6);
        c.encode_raster(&mut up);
        for y in 0..6u16 {
            for x in 0..8u16 {
                assert_eq!(up.get(x, y), r.get(x / 2, y / 2));
            }
        }
    }

    #[test]
    fn bands_partition_the_rows_exactly_once() {
        for topo in [Topology::Cube, Topology::Ring { w: 17, h: 11 }] {
            let c = Canvas::new(topo, Scale::ONE);
            for n in [1usize, 2, 3, 4, 7, 320] {
                let bands = c.bands(n);
                let mut next = 0;
                let mut seen = 0;
                for b in bands.iter() {
                    assert_eq!(b.band().start, next);
                    next = b.band().end;
                    seen += b.pixels().len();
                }
                assert_eq!(next, c.band().end);
                assert_eq!(seen, c.pixels().len());
            }
        }
    }

    #[test]
    fn a_band_drops_the_writes_outside_its_rows() {
        let c = Canvas::cube();
        let mut bands = c.bands(5);
        // Five bands over 320 rows: one face each.
        let b = &mut bands.iter_mut().nth(2).unwrap();
        assert_eq!(b.rows_of(Face::Back), 0..64);
        assert_eq!(b.rows_of(Face::Front), 0..0);
        b.set(Face::Front, 1, 1, [1.0, 1.0, 1.0]);
        b.set(Face::Back, 1, 1, [1.0, 1.0, 1.0]);
        assert_eq!(b.get(Face::Front, 1, 1), [0.0; 3]);
        assert_eq!(b.get(Face::Back, 1, 1), [1.0; 3]);
        assert_eq!(b.coords().count(), 64 * 64);
    }
}
