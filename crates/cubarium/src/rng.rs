//! SplitMix64, the scenes' own deterministic PRNG.
//!
//! Scene state is a pure function of `--seed` and the tick counter; wall time never
//! enters it. Rendering and logging never draw from these streams.

/// The reference SplitMix64 generator (Steele, Lea & Flood 2014).
#[derive(Clone, Debug)]
pub struct SplitMix64 {
    state: u64,
}

const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

impl SplitMix64 {
    pub fn new(seed: u64) -> SplitMix64 {
        SplitMix64 { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)` from the top 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in `[lo, hi)`.
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// A standard normal draw (Box–Muller; both draws come from this stream, so the
    /// sequence stays reproducible whatever the caller does with the value).
    pub fn normal(&mut self) -> f64 {
        // `next_f64` can return exactly 0; nudge onto (0, 1] so the log is finite.
        let u1 = 1.0 - self.next_f64();
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published SplitMix64 reference output for seed 0.
    #[test]
    fn matches_the_reference_stream() {
        let mut r = SplitMix64::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(r.next_u64(), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn is_deterministic_per_seed_and_differs_between_seeds() {
        let a: Vec<u64> = (0..8).scan(SplitMix64::new(7), |r, _| Some(r.next_u64())).collect();
        let b: Vec<u64> = (0..8).scan(SplitMix64::new(7), |r, _| Some(r.next_u64())).collect();
        let c: Vec<u64> = (0..8).scan(SplitMix64::new(8), |r, _| Some(r.next_u64())).collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn uniform_and_normal_draws_are_well_behaved() {
        let mut r = SplitMix64::new(1);
        let mut sum = 0.0;
        let mut nsum = 0.0;
        let mut nsq = 0.0;
        for _ in 0..20_000 {
            let u = r.next_f64();
            assert!((0.0..1.0).contains(&u));
            sum += u;
            let n = r.normal();
            assert!(n.is_finite());
            nsum += n;
            nsq += n * n;
        }
        assert!((sum / 20_000.0 - 0.5).abs() < 0.02, "uniform mean {}", sum / 20_000.0);
        assert!((nsum / 20_000.0).abs() < 0.05, "normal mean {}", nsum / 20_000.0);
        assert!((nsq / 20_000.0 - 1.0).abs() < 0.1, "normal variance {}", nsq / 20_000.0);
    }
}
