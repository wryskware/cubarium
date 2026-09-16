//! The sRGB transfer, as a table that is *exactly* the curve it replaces (FW-P's W7).
//!
//! [`srgb_encode`] used to widen every channel to `f64` and call `powf`, once per channel
//! per pixel: 1.37 ms of the board's 15.28 ms frame. The curve is one dimensional and its
//! output is one of 256 codes, so all it really needs is the 255 **thresholds** — the
//! linear values at which the rounded code steps up — and the answer is the number of
//! thresholds at or below the input.
//!
//! Those thresholds are found once, by bisecting the `f32` bit patterns against the
//! original `powf` expression ([`reference`]), so each one is the exact `f32` at which the
//! old function's own output changed. That makes the table **bit-identical on every
//! `f32`**, not merely accurate: it is not an approximation of the curve, it is a
//! tabulation of the old function's own step points. An interpolated 4,096-entry table
//! would have been within half a code, which is not the same thing and would have moved
//! pixels on the cube.
//!
//! The lookup itself is a single indexed load plus one compare. A positive `f32` below 1
//! is ordered by its bit pattern, so the top 8 mantissa bits and the exponent name a
//! bucket 2⁻⁸ wide in relative terms; the closest two thresholds are 0.89 % apart (at the
//! bright end, where the curve is flattest), so **a bucket can hold at most one
//! threshold** — asserted while the table is built. Each bucket therefore stores the code
//! its lowest value takes and the one place inside it, if any, where that code steps up.

use std::sync::LazyLock;

/// Exponent field of the smallest `f32` that does not encode to 0. The code-1 threshold is
/// `0.5 / (255 · 12.92) = 1.5177e-4`, which lies in `[2⁻¹³, 2⁻¹²)`.
const FIRST_EXP: u32 = 114;
/// Mantissa bits kept in the bucket index.
const MANTISSA_BITS: u32 = 8;
const SHIFT: u32 = 23 - MANTISSA_BITS;
const BASE: u32 = FIRST_EXP << MANTISSA_BITS;
/// Exponents `114..=126`, the whole range between the first non-zero code and 1.0.
const BUCKETS: usize = 13 << MANTISSA_BITS;
/// Offset mask inside a bucket, and a value no offset can reach.
const OFFSET_MASK: u32 = (1 << SHIFT) - 1;
const NO_STEP: u16 = 1 << SHIFT;

/// The original `srgb_encode` body, kept because it *defines* the answer: the table is
/// built from it and the tests compare against it.
fn reference(linear: f32) -> u8 {
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

pub(crate) struct Table {
    /// The code taken by the lowest `f32` of each bucket.
    code: Box<[u8; BUCKETS]>,
    /// The in-bucket offset at which that code steps up by one, or [`NO_STEP`].
    step: Box<[u16; BUCKETS]>,
}

impl Table {
    #[inline]
    pub(crate) fn encode(&self, linear: f32) -> u8 {
        // NaN and every non-positive value encode as 0; `>= 1.0` saturates.
        if linear.is_nan() || linear <= 0.0 {
            return 0;
        }
        if linear >= 1.0 {
            return 255;
        }
        let bits = linear.to_bits();
        let index = (bits >> SHIFT).wrapping_sub(BASE) as usize;
        if index >= BUCKETS {
            // Below the first non-zero code (`wrapping_sub` underflowed).
            return 0;
        }
        self.code[index] + u8::from((bits & OFFSET_MASK) as u16 >= self.step[index])
    }
}

/// The smallest `f32` bit pattern whose [`reference`] code is at least `code`.
fn threshold(code: u8) -> u32 {
    let (mut lo, mut hi) = (0u32, 1.0f32.to_bits());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if reference(f32::from_bits(mid)) >= code {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let thresholds: Vec<u32> = (1..=255u8).map(threshold).collect();
    let mut code = Box::new([0u8; BUCKETS]);
    let mut step = Box::new([NO_STEP; BUCKETS]);
    // `thresholds` is ascending, so one pass over the buckets places every step.
    let mut next = 0usize;
    let mut current = 0u8;
    for i in 0..BUCKETS {
        let lo = (BASE + i as u32) << SHIFT;
        let hi = lo | OFFSET_MASK;
        while next < thresholds.len() && thresholds[next] <= lo {
            next += 1;
            current += 1;
        }
        code[i] = current;
        if next < thresholds.len() && thresholds[next] <= hi {
            step[i] = (thresholds[next] - lo) as u16;
            next += 1;
            current += 1;
            assert!(
                next >= thresholds.len() || thresholds[next] > hi,
                "sRGB bucket {i} holds two code steps; the table's one-step-per-bucket \
                 invariant needs more mantissa bits"
            );
        }
    }
    assert_eq!(next, thresholds.len(), "every sRGB code step must land in a bucket");
    Table { code, step }
});

/// Linear `[0, 1]` to sRGB-encoded 8-bit (IEC 61966-2-1 piecewise curve), clamping
/// out-of-range and NaN input to 0 or 255.
///
/// Table-driven and bit-identical on every `f32` to the `powf` form it replaces.
#[inline]
pub fn srgb_encode(linear: f32) -> u8 {
    TABLE.encode(linear)
}

/// Inverse of [`srgb_encode`] on the 8-bit lattice.
pub fn srgb_decode(encoded: u8) -> f32 {
    let c = f64::from(encoded) / 255.0;
    let l = if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    l as f32
}

/// The table itself, fetched once for a whole-canvas encode rather than through the
/// `LazyLock` guard on every channel.
#[inline]
pub(crate) fn table() -> &'static Table {
    &TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_answers_the_reference_curve_on_every_code_and_its_neighbourhood() {
        for code in 0..=255u8 {
            let linear = srgb_decode(code);
            assert_eq!(srgb_encode(linear), code, "code {code} decoded to {linear}");
            assert_eq!(srgb_encode(linear), reference(linear));
        }
        // Every step point and the four `f32` on either side of it: the only places a
        // tabulation could disagree with the curve it was cut from.
        for c in 1..=255u8 {
            let t = threshold(c);
            for d in -4i64..=4 {
                let b = (t as i64 + d).clamp(0, 1.0f32.to_bits() as i64) as u32;
                let v = f32::from_bits(b);
                assert_eq!(srgb_encode(v), reference(v), "at {v} ({b:#x}), {d} from step {c}");
            }
        }
    }

    #[test]
    fn the_table_answers_the_reference_curve_on_a_sweep() {
        // A stride over the whole positive `f32` range below 1, plus the toe, where the
        // codes are decades apart in linear light and a bucket index must not underflow.
        let top = 1.0f32.to_bits();
        let mut b = 0u32;
        while b < top {
            let v = f32::from_bits(b);
            assert_eq!(srgb_encode(v), reference(v), "at {v} ({b:#x})");
            b += 9_973; // a prime stride: 107,000 samples, none of them aligned
        }
        for i in 0..=100_000u32 {
            let v = i as f32 / 100_000.0;
            assert_eq!(srgb_encode(v), reference(v), "at {v}");
        }
        for v in [f32::MIN_POSITIVE, f32::from_bits(1), 1e-30, 1.5177e-4, 0.003_130_8, 1.0, 2.0] {
            assert_eq!(srgb_encode(v), reference(v), "at {v}");
        }
        assert_eq!(srgb_encode(f32::NAN), reference(f32::NAN));
        assert_eq!(srgb_encode(-0.0), 0);
        assert_eq!(srgb_encode(f32::INFINITY), 255);
    }

}
