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
    let _ = (seed, stream, key, counter);
    todo!("draw")
}

/// Uniform in `[0, 1)` from the top 53 bits of a draw.
pub fn unit(seed: u64, stream: Stream, key: u64, counter: u64) -> f64 {
    (draw(seed, stream, key, counter) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Standard normal from two consecutive counters (Box–Muller; the second value is discarded).
pub fn normal(seed: u64, stream: Stream, key: u64, counter: u64) -> f64 {
    let _ = (seed, stream, key, counter);
    todo!("normal")
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
