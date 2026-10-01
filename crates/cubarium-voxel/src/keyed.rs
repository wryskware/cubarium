//! Small helpers the world, plant and animal layers share: the keyed splitmix64 stream,
//! seconds to whole ticks, and the pro-rata share of a parcel's stock.

use crate::TICK_HZ;

/// splitmix64's finalizer: the avalanche that makes neighbouring keys independent.
pub fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A uniform `[0, 1)` value from the top 53 bits of `bits`, so every value is an exact
/// `f64`.
pub fn unit_from_bits(bits: u64) -> f64 {
    (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// A deterministic scalar stream (splitmix64), keyed by the values that **identify** a
/// draw rather than seeded from stored state: the same world, the same place and the same
/// tick always produce the same numbers, and nothing about iteration or storage order can
/// reach them. No clock and no thread state.
pub struct KeyedRng(u64);

impl KeyedRng {
    pub fn keyed(domain: u64, a: u64, b: u64, c: u64) -> KeyedRng {
        let mut state = 0u64;
        for part in [domain, a, b, c] {
            state = mix(state ^ part);
        }
        KeyedRng(state)
    }

    /// A stream continuing from a raw state (a controller's own memory).
    pub fn from_state(state: u64) -> KeyedRng {
        KeyedRng(state)
    }

    pub fn state(&self) -> u64 {
        self.0
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    /// A uniform draw in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        unit_from_bits(self.next_u64())
    }

    /// A uniform index below `n`, which must be positive. Modulo, whose bias against
    /// `2^64` is 1e-18 for the handful of candidates anything here draws among.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Whole ticks for a duration in seconds, never below one. A duration that is not a
/// number, or rounds below one tick, is one tick.
pub fn ticks_at_least_one(seconds: f64) -> u64 {
    let t = (seconds * f64::from(TICK_HZ)).round();
    if t >= 1.0 { t as u64 } else { 1 }
}

/// The share of `stock` that goes with `part` of a parcel holding `whole`: the parcel's
/// own density, never more than `stock`, and nothing from an empty parcel.
pub fn pro_rata(stock: f64, part: f64, whole: f64) -> f64 {
    if whole > 0.0 {
        (stock * (part / whole)).min(stock)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_round_and_floor_at_one() {
        assert_eq!(ticks_at_least_one(1.0), u64::from(TICK_HZ));
        assert_eq!(ticks_at_least_one(0.0), 1);
        assert_eq!(ticks_at_least_one(f64::NAN), 1);
        assert_eq!(ticks_at_least_one(-3.0), 1);
    }

    #[test]
    fn pro_rata_follows_the_parcel_density() {
        assert_eq!(pro_rata(2.0, 1.0, 4.0), 0.5);
        assert_eq!(pro_rata(2.0, 8.0, 4.0), 2.0);
        assert_eq!(pro_rata(2.0, 1.0, 0.0), 0.0);
    }

    #[test]
    fn the_keyed_stream_is_a_function_of_its_key() {
        let a = KeyedRng::keyed(1, 2, 3, 4).next_u64();
        assert_eq!(a, KeyedRng::keyed(1, 2, 3, 4).next_u64());
        assert_ne!(a, KeyedRng::keyed(1, 2, 3, 5).next_u64());
        let u = KeyedRng::keyed(0, 0, 0, 0).unit();
        assert!((0.0..1.0).contains(&u));
    }
}
