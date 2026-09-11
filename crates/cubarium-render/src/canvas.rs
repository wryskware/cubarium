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
        for face in Face::ALL {
            let src = &self.faces[face.index()];
            let dst = frame.face_mut(face);
            for (px, out) in src.iter().zip(dst.as_chunks_mut::<3>().0) {
                out[0] = srgb_encode(px[0]);
                out[1] = srgb_encode(px[1]);
                out[2] = srgb_encode(px[2]);
            }
        }
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
    // NaN and every non-positive value encode as 0.
    if linear.is_nan() || linear <= 0.0 {
        return 0;
    }
    if linear >= 1.0 {
        return 255;
    }
    let c = f64::from(linear);
    let s = if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Inverse of [`srgb_encode`] on the 8-bit lattice.
pub fn srgb_decode(encoded: u8) -> f32 {
    let c = f64::from(encoded) / 255.0;
    let l = if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    l as f32
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
        let mut c = Canvas::new();
        c.set(Face::Front, 0, 0, [f32::NAN, -3.0, 12.0]);
        c.set(Face::Top, 63, 63, [1.0, 0.0, 0.5]);
        c.set(Face::Back, 5, 7, [f32::INFINITY, f32::NEG_INFINITY, f32::NAN]);
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
        let mut c = Canvas::new();
        for (i, face) in Face::ALL.into_iter().enumerate() {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let t = ((i as f32) * 13.0 + f32::from(x) * 0.7 + f32::from(y) * 0.3) % 1.0;
                    c.set(face, x, y, [t, t * 0.5, 1.0 - t]);
                }
            }
        }
        let mut f = Frame::black();
        c.encode(&mut f);
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let v = c.get(face, x, y);
                    let want = [srgb_encode(v[0]), srgb_encode(v[1]), srgb_encode(v[2])];
                    assert_eq!(f.get(face, usize::from(x), usize::from(y)), want);
                }
            }
        }
    }

    #[test]
    fn add_accumulates_and_clear_resets() {
        let mut c = Canvas::new();
        c.add(Face::Right, 2, 3, [0.1, 0.2, 0.3]);
        c.add(Face::Right, 2, 3, [0.1, 0.2, 0.3]);
        let v = c.get(Face::Right, 2, 3);
        assert!((v[0] - 0.2).abs() < 1e-6 && (v[1] - 0.4).abs() < 1e-6 && (v[2] - 0.6).abs() < 1e-6);
        c.clear();
        assert_eq!(c.get(Face::Right, 2, 3), [0.0; 3]);
    }
}
