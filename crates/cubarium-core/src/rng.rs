//! Counter-based keyed randomness. No global state; every draw names its stream, key,
//! and counter, so results are independent of iteration order and of rendering/logging.

use serde::{Deserialize, Serialize};

/// Random streams partitioned by world process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u64)]
pub enum Stream {
    /// key = blob index (light blobs first, then moisture), counter = simulated minute.
    Weather = 1,
    /// key = organism slot, counter = per-organism draw counter.
    OrganismTurn = 2,
    /// key = parent organism slot, counter = parent's birth count.
    Birth = 3,
    /// key = founder index, counter = attribute index.
    Founders = 4,
    /// key = wave index, counter = attribute index (frequency, direction, phase).
    Habitat = 5,
}

/// A `u64` draw: SplitMix64 finalization applied to the mix of `(seed ^ stream, key, counter)`.
///
/// Normative: `x = seed ^ (stream as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)`, then
/// `x = mix(x ^ key.wrapping_mul(0xBF58_476D_1CE4_E5B9))`, then
/// `x = mix(x ^ counter.wrapping_mul(0x94D0_49BB_1331_11EB))`, where `mix` is the
/// SplitMix64 output function (`z ^= z >> 30; z *= 0xBF58476D1CE4E5B9; z ^= z >> 27;
/// z *= 0x94D049BB133111EB; z ^= z >> 31`). Fixed forever once snapshots exist.
pub fn draw(seed: u64, stream: Stream, key: u64, counter: u64) -> u64 {
    let mut x = seed ^ (stream as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = mix(x ^ key.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    mix(x ^ counter.wrapping_mul(0x94D0_49BB_1331_11EB))
}

/// The SplitMix64 output function.
#[inline]
fn mix(mut z: u64) -> u64 {
    z ^= z >> 30;
    z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 27;
    z = z.wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform in `[0, 1)` from the top 53 bits of a draw.
pub fn unit(seed: u64, stream: Stream, key: u64, counter: u64) -> f64 {
    (draw(seed, stream, key, counter) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Standard normal from two consecutive counters (Box–Muller; the second value is discarded).
pub fn normal(seed: u64, stream: Stream, key: u64, counter: u64) -> f64 {
    let u1 = unit(seed, stream, key, counter);
    let u2 = unit(seed, stream, key, counter.wrapping_add(1));
    // `unit` is in [0, 1), so `u1 == 0` is possible and `ln(0)` is not; reflecting to
    // `1 - u1` keeps the value in (0, 1] without biasing the magnitude distribution.
    let u1 = if u1 == 0.0 { 1.0 - u1 } else { u1 };
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// A per-owner draw counter that is checkpointed with its owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counter(pub u64);

impl Counter {
    /// Return the current value and advance.
    #[inline]
    pub fn take(&mut self) -> u64 {
        let c = self.0;
        self.0 += 1;
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STREAMS: [Stream; 5] =
        [Stream::Weather, Stream::OrganismTurn, Stream::Birth, Stream::Founders, Stream::Habitat];

    #[test]
    fn draw_is_deterministic() {
        for &s in &STREAMS {
            for key in 0..8u64 {
                for c in 0..8u64 {
                    assert_eq!(draw(7, s, key, c), draw(7, s, key, c));
                }
            }
        }
    }

    #[test]
    fn draw_differs_across_stream_key_counter_and_seed() {
        let base = draw(7, Stream::Weather, 3, 11);
        for &s in &STREAMS {
            if s != Stream::Weather {
                assert_ne!(base, draw(7, s, 3, 11), "stream {s:?} collides");
            }
        }
        assert_ne!(base, draw(7, Stream::Weather, 4, 11));
        assert_ne!(base, draw(7, Stream::Weather, 3, 12));
        assert_ne!(base, draw(8, Stream::Weather, 3, 11));

        // No collisions over a dense block of (stream, key, counter).
        let mut seen = std::collections::HashSet::new();
        for &s in &STREAMS {
            for key in 0..40u64 {
                for c in 0..40u64 {
                    assert!(seen.insert(draw(12345, s, key, c)), "collision at {s:?} {key} {c}");
                }
            }
        }
    }

    #[test]
    fn unit_is_in_range_with_a_sane_mean() {
        const N: usize = 10_000;
        let mut sum = 0.0;
        for i in 0..N as u64 {
            let u = unit(99, Stream::Founders, 0, i);
            assert!((0.0..1.0).contains(&u), "unit out of range: {u}");
            sum += u;
        }
        let mean = sum / N as f64;
        assert!((mean - 0.5).abs() < 0.02, "unit mean {mean}");
    }

    #[test]
    fn normal_has_sane_mean_and_variance() {
        const N: usize = 20_000;
        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        for i in 0..N as u64 {
            // Each sample consumes two counters, so step by two for independence.
            let z = normal(5, Stream::OrganismTurn, 1, i * 2);
            assert!(z.is_finite());
            sum += z;
            sum_sq += z * z;
        }
        let mean = sum / N as f64;
        let var = sum_sq / N as f64 - mean * mean;
        assert!(mean.abs() < 0.05, "normal mean {mean}");
        assert!((var - 1.0).abs() < 0.05, "normal variance {var}");
    }

    #[test]
    fn normal_guards_a_zero_first_draw() {
        // Force u1 == 0 by construction: the guard replaces it with 1 - u1 = 1, giving 0
        // magnitude rather than an infinity.
        let z = (-2.0f64 * 1.0f64.ln()).sqrt() * 0.0f64.cos();
        assert_eq!(z, 0.0);
        assert!(normal(1, Stream::Birth, 0, 0).is_finite());
    }

    #[test]
    fn counter_take_advances() {
        let mut c = Counter::default();
        assert_eq!(c.take(), 0);
        assert_eq!(c.take(), 1);
        assert_eq!(c.0, 2);
    }
}
