//! The **training** randomness: seeded, positional, and completely separate from the world's.
//!
//! Two rules the brief fixes, and the reason for this module existing beside
//! [`crate::rng`]:
//!
//! 1. **Training RNG and world RNG stay separate.** Nothing here ever touches
//!    `cubarium_core::rng`, and no world draw is consumed to make a perturbation. A world's
//!    randomness comes from its own `WorldConfig::seed`, which the fixture fixes per layout.
//! 2. **A draw is a pure function of its position**, never of how many draws came first.
//!    `gaussian(seed, stream, key, index)` names one value; a worker can produce element
//!    9,000 of a perturbation without having produced the first 8,999. That is what lets a
//!    generation be split across any number of workers and still reduce identically.
//!
//! [`crate::rng::normal`] is not reused because it spends *two* consecutive counters per
//! value, so `normal(.., i)` and `normal(.., i + 1)` share a uniform draw. That is harmless
//! for the M1 search, which uses a fresh key per draw, but it would put a visible correlation
//! between adjacent elements of a 10,215-dimensional perturbation.

use std::f64::consts::TAU;

use crate::rng::unit;

/// Named streams for the trainer. Deliberately disjoint from [`crate::rng::stream`], which
/// belongs to the M1 ecological search.
pub mod stream {
    /// The seeded centre's matrix weights.
    pub const ES_INIT: u64 = 101;
    /// Antithetic perturbations.
    pub const ES_PERTURBATION: u64 = 102;
    /// Held-out layout construction.
    pub const ES_HOLDOUT: u64 = 103;
}

/// Standard normal at one position (Box–Muller over two dedicated counters, `2i` and `2i+1`,
/// so distinct indices never share a uniform draw).
pub fn gaussian(seed: u64, stream: u64, key: u64, index: u64) -> f64 {
    let i = index.wrapping_mul(2);
    let u1 = unit(seed, stream, key, i).max(f64::MIN_POSITIVE);
    let u2 = unit(seed, stream, key, i.wrapping_add(1));
    (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
}

/// Uniform in `[0, 1)` at one position, re-exported so a caller never reaches past this
/// module into the M1 search's own randomness by accident.
pub fn unit_at(seed: u64, stream: u64, key: u64, counter: u64) -> f64 {
    unit(seed, stream, key, counter)
}

/// The key that names perturbation pair `pair` of generation `generation`.
///
/// Packed rather than multiplied so the mapping is injective for any pair count below 2^20
/// and readable in a log line. Both signs of a pair use this same key: the antithetic pair is
/// `theta ± sigma·epsilon` with *one* `epsilon`.
pub fn pair_key(generation: u64, pair: u64) -> u64 {
    assert!(pair < (1 << 20), "pair index {pair} does not fit the key packing");
    (generation << 20) | pair
}

/// Write `epsilon_i ~ N(0, I)` for one `(generation, pair)` into `out`.
pub fn perturbation(train_seed: u64, generation: u64, pair: u64, out: &mut [f64]) {
    let key = pair_key(generation, pair);
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = gaussian(train_seed, stream::ES_PERTURBATION, key, i as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draw_is_a_pure_function_of_its_position() {
        assert_eq!(gaussian(5, stream::ES_INIT, 2, 900), gaussian(5, stream::ES_INIT, 2, 900));
        assert_ne!(gaussian(5, stream::ES_INIT, 2, 900), gaussian(5, stream::ES_INIT, 2, 901));
        assert_ne!(gaussian(5, stream::ES_INIT, 2, 900), gaussian(6, stream::ES_INIT, 2, 900));
        assert_ne!(
            gaussian(5, stream::ES_INIT, 2, 900),
            gaussian(5, stream::ES_PERTURBATION, 2, 900)
        );
    }

    #[test]
    fn adjacent_elements_are_uncorrelated_enough_to_be_a_gaussian_vector() {
        let n = 20_000;
        let mut v = vec![0.0; n];
        perturbation(20_260_915, 0, 0, &mut v);
        let mean = v.iter().sum::<f64>() / n as f64;
        let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.05, "mean {mean}");
        assert!((var - 1.0).abs() < 0.08, "variance {var}");
        // Lag-1 correlation: the defect `crate::rng::normal` would have shown here.
        let lag: f64 = v.windows(2).map(|w| (w[0] - mean) * (w[1] - mean)).sum::<f64>()
            / ((n - 1) as f64 * var);
        assert!(lag.abs() < 0.05, "lag-1 correlation {lag}");
        assert!(v.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn a_perturbation_can_be_produced_out_of_order() {
        let mut whole = vec![0.0; 64];
        perturbation(11, 3, 2, &mut whole);
        let key = pair_key(3, 2);
        for i in (0..64).rev() {
            assert_eq!(whole[i], gaussian(11, stream::ES_PERTURBATION, key, i as u64));
        }
    }

    #[test]
    fn pair_keys_do_not_collide() {
        assert_ne!(pair_key(0, 1), pair_key(1, 0));
        assert_ne!(pair_key(1, 0), pair_key(0, 1 << 19));
    }
}
