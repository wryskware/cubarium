//! The search's own deterministic randomness, kept separate from the world's.
//!
//! Every draw names `(stream, key, counter)` the way [`cubarium_core::rng`] does, so a value
//! never depends on how many other draws happened first — which is what lets four workers
//! finish in any order and still produce the same generation.

/// SplitMix64 finalization over `(seed ^ stream, key, counter)`.
pub fn draw(seed: u64, stream: u64, key: u64, counter: u64) -> u64 {
    let mut x = seed ^ stream.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = mix(x ^ key.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    mix(x ^ counter.wrapping_mul(0x94D0_49BB_1331_11EB))
}

fn mix(mut z: u64) -> u64 {
    z ^= z >> 30;
    z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 27;
    z = z.wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform in `[0, 1)` from the top 53 bits.
pub fn unit(seed: u64, stream: u64, key: u64, counter: u64) -> f64 {
    (draw(seed, stream, key, counter) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Standard normal (Box–Muller; the second value is discarded so the counter stays a
/// one-to-one name for the draw).
pub fn normal(seed: u64, stream: u64, key: u64, counter: u64) -> f64 {
    let u1 = unit(seed, stream, key, counter).max(f64::MIN_POSITIVE);
    let u2 = unit(seed, stream, key, counter.wrapping_add(1));
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// Named streams, so two unrelated decisions can never consume each other's draws.
pub mod stream {
    pub const INIT: u64 = 1;
    pub const TOURNAMENT: u64 = 2;
    pub const CROSSOVER: u64 = 3;
    pub const MUTATION: u64 = 4;
    pub const APEX_PLACEMENT: u64 = 5;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_are_reproducible_and_stream_separated() {
        assert_eq!(draw(7, stream::INIT, 3, 9), draw(7, stream::INIT, 3, 9));
        assert_ne!(draw(7, stream::INIT, 3, 9), draw(7, stream::MUTATION, 3, 9));
        assert_ne!(draw(7, stream::INIT, 3, 9), draw(8, stream::INIT, 3, 9));
    }

    #[test]
    fn unit_stays_in_range_and_normal_is_finite() {
        for c in 0..2000 {
            let u = unit(11, stream::INIT, 0, c);
            assert!((0.0..1.0).contains(&u), "unit out of range: {u}");
            assert!(normal(11, stream::MUTATION, 0, c).is_finite());
        }
    }
}
