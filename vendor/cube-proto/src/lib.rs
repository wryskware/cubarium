//! cube-proto — frame format, UDP wire protocol and a blocking client for the LED cube.
//!
//! A frame is five 64×64 RGB8 face images (`Face` order, row-major, `x` right, `y` down).
//! See `docs/ARCHITECTURE.md` for the binding contract. No async, no heavy dependencies.

mod client;
pub mod geometry;
mod wire;

pub use client::CubeClient;
pub use geometry::{cross_seam, CubemapFaces, Edge, Seam};
pub use wire::{
    decode, encode_face, encode_full, seq_is_newer, Format, Header, ProtoError, HEADER_BYTES,
    MAGIC, NO_FACE, VERSION,
};

pub const FACE_SIZE: usize = 64;
pub const NUM_FACES: usize = 5;
pub const FACE_BYTES: usize = FACE_SIZE * FACE_SIZE * 3; // 12288
pub const FRAME_BYTES: usize = NUM_FACES * FACE_BYTES; // 61440
pub const DEFAULT_PORT: u16 = 7392;

/// The five cube faces. Right is on the viewer's right when facing Front, Back is opposite
/// Front, Left is opposite Right (so Front → Right → Back → Left runs counter-clockwise seen
/// from above); Top is seen looking down with the Back face at the top of the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[repr(u8)]
pub enum Face {
    Front = 0,
    Right = 1,
    Back = 2,
    Left = 3,
    Top = 4,
}

impl Face {
    pub const ALL: [Face; NUM_FACES] = [
        Face::Front,
        Face::Right,
        Face::Back,
        Face::Left,
        Face::Top,
    ];

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub fn from_index(i: u8) -> Option<Face> {
        match i {
            0 => Some(Face::Front),
            1 => Some(Face::Right),
            2 => Some(Face::Back),
            3 => Some(Face::Left),
            4 => Some(Face::Top),
            _ => None,
        }
    }

    /// Short label used by the test pattern: F/R/B/L/T.
    pub fn letter(self) -> char {
        match self {
            Face::Front => 'F',
            Face::Right => 'R',
            Face::Back => 'B',
            Face::Left => 'L',
            Face::Top => 'T',
        }
    }
}

/// Owned frame: faces stored contiguously in `Face` order, each face row-major RGB8.
#[derive(Clone)]
pub struct Frame {
    data: Box<[u8; FRAME_BYTES]>,
}

#[inline]
fn pixel_offset(f: Face, x: usize, y: usize) -> usize {
    assert!(
        x < FACE_SIZE && y < FACE_SIZE,
        "cube-proto: pixel ({x}, {y}) out of range for a {FACE_SIZE}x{FACE_SIZE} face"
    );
    f.index() * FACE_BYTES + (y * FACE_SIZE + x) * 3
}

impl Frame {
    /// An all-black frame (heap-allocated; never built on the stack).
    pub fn black() -> Frame {
        let data: Box<[u8; FRAME_BYTES]> = vec![0u8; FRAME_BYTES]
            .into_boxed_slice()
            .try_into()
            .expect("FRAME_BYTES-sized allocation");
        Frame { data }
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8; FRAME_BYTES] {
        &self.data
    }

    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8; FRAME_BYTES] {
        &mut self.data
    }

    /// One face image, `FACE_BYTES` long.
    #[inline]
    pub fn face(&self, f: Face) -> &[u8] {
        let o = f.index() * FACE_BYTES;
        &self.data[o..o + FACE_BYTES]
    }

    #[inline]
    pub fn face_mut(&mut self, f: Face) -> &mut [u8] {
        let o = f.index() * FACE_BYTES;
        &mut self.data[o..o + FACE_BYTES]
    }

    /// Panics if `x` or `y` is >= `FACE_SIZE`.
    #[inline]
    pub fn get(&self, f: Face, x: usize, y: usize) -> [u8; 3] {
        let o = pixel_offset(f, x, y);
        [self.data[o], self.data[o + 1], self.data[o + 2]]
    }

    /// Panics if `x` or `y` is >= `FACE_SIZE`.
    #[inline]
    pub fn set(&mut self, f: Face, x: usize, y: usize, rgb: [u8; 3]) {
        let o = pixel_offset(f, x, y);
        self.data[o..o + 3].copy_from_slice(&rgb);
    }

    /// Fill one face with a solid color.
    pub fn fill_face(&mut self, f: Face, rgb: [u8; 3]) {
        for px in self.face_mut(f).as_chunks_mut::<3>().0 {
            *px = rgb;
        }
    }

    /// Fill every face with a solid color.
    pub fn fill(&mut self, rgb: [u8; 3]) {
        for px in self.data.as_chunks_mut::<3>().0 {
            *px = rgb;
        }
    }
}

impl Default for Frame {
    fn default() -> Self {
        Frame::black()
    }
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Frame({FRAME_BYTES} bytes)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_layout() {
        assert_eq!(FACE_BYTES, 12288);
        assert_eq!(FRAME_BYTES, 61440);

        let mut fr = Frame::black();
        assert!(fr.as_bytes().iter().all(|&b| b == 0));
        fr.set(Face::Top, 1, 2, [9, 8, 7]);
        assert_eq!(fr.get(Face::Top, 1, 2), [9, 8, 7]);
        // Top is the 5th face, so pixel (1,2) lands at byte 4*FACE_BYTES + (2*64+1)*3.
        assert_eq!(fr.as_bytes()[4 * FACE_BYTES + (2 * 64 + 1) * 3], 9);
        assert_eq!(fr.face(Face::Top).len(), FACE_BYTES);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn get_bounds_checked() {
        Frame::black().get(Face::Front, 64, 0);
    }

    #[test]
    fn face_indices() {
        for (i, f) in Face::ALL.iter().enumerate() {
            assert_eq!(f.index(), i);
            assert_eq!(Face::from_index(i as u8), Some(*f));
        }
        assert_eq!(Face::from_index(5), None);
    }
}
