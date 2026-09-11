//! Linear-light five-face canvas and 8-bit encoding.

use cube_proto::{FACE_SIZE, Face, Frame, NUM_FACES};

const FACE_PIXELS: usize = FACE_SIZE * FACE_SIZE;

/// Five 64×64 linear-RGB `f32` faces, row-major like `Frame`.
#[derive(Clone)]
pub struct Canvas {
    faces: Box<[[[f32; 3]; FACE_PIXELS]; NUM_FACES]>,
}

impl Canvas {
    pub fn new() -> Canvas {
        Canvas { faces: Box::new([[[0.0; 3]; FACE_PIXELS]; NUM_FACES]) }
    }

    pub fn clear(&mut self) {
        for f in self.faces.iter_mut() {
            f.fill([0.0; 3]);
        }
    }

    #[inline]
    pub fn get(&self, face: Face, x: u8, y: u8) -> [f32; 3] {
        self.faces[face.index()][usize::from(y) * FACE_SIZE + usize::from(x)]
    }

    #[inline]
    pub fn set(&mut self, face: Face, x: u8, y: u8, rgb: [f32; 3]) {
        self.faces[face.index()][usize::from(y) * FACE_SIZE + usize::from(x)] = rgb;
    }

    /// Additive blend (linear light adds); clamping happens at encode time.
    #[inline]
    pub fn add(&mut self, face: Face, x: u8, y: u8, rgb: [f32; 3]) {
        let p = &mut self.faces[face.index()][usize::from(y) * FACE_SIZE + usize::from(x)];
        p[0] += rgb[0];
        p[1] += rgb[1];
        p[2] += rgb[2];
    }

    /// Encode into the shim frame: clamp to `[0, 1]`, sRGB transfer, round to `u8`.
    /// Deterministic and total (NaN encodes as 0).
    pub fn encode(&self, frame: &mut Frame) {
        let _ = frame;
        todo!("Canvas::encode")
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas::new()
    }
}

/// Linear `[0, 1]` to sRGB-encoded 8-bit (IEC 61966-2-1 piecewise curve), clamping
/// out-of-range and NaN input to 0 or 255.
pub fn srgb_encode(linear: f32) -> u8 {
    let _ = linear;
    todo!("srgb_encode")
}

/// Inverse of [`srgb_encode`] on the 8-bit lattice.
pub fn srgb_decode(encoded: u8) -> f32 {
    let _ = encoded;
    todo!("srgb_decode")
}
