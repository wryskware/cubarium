//! `Raster` — a flat `W×H` RGB8 image, the second thing a client can send.
//!
//! The cube `Frame` is five fixed 64×64 faces; a raster is one rectangle of any size
//! up to [`MAX_RASTER_DIM`] on a side. It exists because the Tachyon's panel shows a
//! flat 2D world, not cube faces. Nothing about the cube path changes: the two travel
//! over the same socket, told apart by the header's format byte.
//!
//! A raster is usually too big for one datagram (a 1920×1080 image is 6.2 MB against a
//! 65,507-byte UDP limit), so it goes out as horizontal strips — see
//! [`crate::encode_raster`].

/// Longest side a raster may have, on either axis.
pub const MAX_RASTER_DIM: u16 = 4096;

/// A flat RGB8 image, row-major, `x` right and `y` down.
///
/// The fields are private so `data.len()` can never drift from `width · height · 3`;
/// everything a caller needs to fill one is here.
#[derive(Clone, PartialEq, Eq)]
pub struct Raster {
    width: u16,
    height: u16,
    data: Vec<u8>,
}

impl Raster {
    /// An all-black raster. Panics unless both sides are in `1..=4096` — a raster
    /// whose size is not a wire-legal size is a caller bug, not a runtime condition.
    pub fn black(width: u16, height: u16) -> Raster {
        assert!(
            dims_ok(width, height),
            "cube-proto: raster {width}x{height} is out of range (both sides must be \
             1..={MAX_RASTER_DIM})"
        );
        Raster {
            width,
            height,
            data: vec![0u8; width as usize * height as usize * 3],
        }
    }

    /// `None` rather than a panic, for sizes that arrived off the wire.
    pub fn try_black(width: u16, height: u16) -> Option<Raster> {
        dims_ok(width, height).then(|| Raster::black(width, height))
    }

    #[inline]
    pub fn width(&self) -> u16 {
        self.width
    }

    #[inline]
    pub fn height(&self) -> u16 {
        self.height
    }

    /// `(width, height)`.
    #[inline]
    pub fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// Row stride in bytes: `width · 3`.
    #[inline]
    pub fn row_bytes(&self) -> usize {
        self.width as usize * 3
    }

    /// Panics if `x >= width` or `y >= height`.
    #[inline]
    pub fn get(&self, x: u16, y: u16) -> [u8; 3] {
        let o = self.offset(x, y);
        [self.data[o], self.data[o + 1], self.data[o + 2]]
    }

    /// Panics if `x >= width` or `y >= height`.
    #[inline]
    pub fn set(&mut self, x: u16, y: u16, rgb: [u8; 3]) {
        let o = self.offset(x, y);
        self.data[o..o + 3].copy_from_slice(&rgb);
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Fill every pixel with one colour.
    pub fn fill(&mut self, rgb: [u8; 3]) {
        for px in self.data.as_chunks_mut::<3>().0 {
            *px = rgb;
        }
    }

    /// One row of pixels. Panics if `y >= height`.
    pub fn row(&self, y: u16) -> &[u8] {
        let o = y as usize * self.row_bytes();
        &self.data[o..o + self.row_bytes()]
    }

    /// `rows` rows starting at `y0`, writable — what a received strip is copied into.
    /// `None` if the span runs past the bottom.
    pub fn rows_mut(&mut self, y0: u16, rows: u16) -> Option<&mut [u8]> {
        if u32::from(y0) + u32::from(rows) > u32::from(self.height) {
            return None;
        }
        let stride = self.row_bytes();
        let o = y0 as usize * stride;
        Some(&mut self.data[o..o + rows as usize * stride])
    }

    #[inline]
    fn offset(&self, x: u16, y: u16) -> usize {
        assert!(
            x < self.width && y < self.height,
            "cube-proto: pixel ({x}, {y}) out of range for a {}x{} raster",
            self.width,
            self.height
        );
        (y as usize * self.width as usize + x as usize) * 3
    }
}

impl std::fmt::Debug for Raster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Raster({}x{}, {} bytes)",
            self.width,
            self.height,
            self.data.len()
        )
    }
}

#[inline]
pub(crate) fn dims_ok(width: u16, height: u16) -> bool {
    (1..=MAX_RASTER_DIM).contains(&width) && (1..=MAX_RASTER_DIM).contains(&height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_black_raster_is_the_right_size_and_all_zero() {
        let r = Raster::black(320, 180);
        assert_eq!(r.size(), (320, 180));
        assert_eq!(r.as_bytes().len(), 320 * 180 * 3);
        assert_eq!(r.row_bytes(), 960);
        assert!(r.as_bytes().iter().all(|&b| b == 0));
        assert_eq!(r.get(0, 0), [0, 0, 0]);
    }

    #[test]
    fn get_and_set_address_row_major_with_x_right_and_y_down() {
        let mut r = Raster::black(4, 3);
        r.set(1, 2, [9, 8, 7]);
        assert_eq!(r.get(1, 2), [9, 8, 7]);
        // Row-major: (1, 2) is byte (2*4 + 1) * 3 = 27.
        assert_eq!(&r.as_bytes()[27..30], &[9, 8, 7]);
        assert_eq!(r.get(0, 0), [0, 0, 0]);
        // The row view agrees.
        assert_eq!(&r.row(2)[3..6], &[9, 8, 7]);
    }

    #[test]
    fn rows_mut_spans_exactly_the_requested_rows() {
        let mut r = Raster::black(4, 3);
        r.rows_mut(1, 2).expect("rows 1..3 fit").fill(0x5a);
        assert!(r.row(0).iter().all(|&b| b == 0));
        assert!(r.row(1).iter().all(|&b| b == 0x5a));
        assert!(r.row(2).iter().all(|&b| b == 0x5a));
        // Past the bottom is None, not a panic.
        assert!(r.rows_mut(2, 2).is_none());
        assert!(r.rows_mut(3, 1).is_none());
        assert!(r.rows_mut(0, 3).is_some());
    }

    #[test]
    fn fill_touches_every_pixel() {
        let mut r = Raster::black(7, 5);
        r.fill([1, 2, 3]);
        for y in 0..5 {
            for x in 0..7 {
                assert_eq!(r.get(x, y), [1, 2, 3], "({x}, {y})");
            }
        }
    }

    #[test]
    fn the_size_limits_are_enforced() {
        assert!(Raster::try_black(1, 1).is_some());
        assert!(Raster::try_black(MAX_RASTER_DIM, MAX_RASTER_DIM).is_some());
        assert!(Raster::try_black(0, 10).is_none());
        assert!(Raster::try_black(10, 0).is_none());
        assert!(Raster::try_black(MAX_RASTER_DIM + 1, 10).is_none());
        assert!(Raster::try_black(10, MAX_RASTER_DIM + 1).is_none());
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn get_is_bounds_checked() {
        Raster::black(4, 4).get(4, 0);
    }
}
