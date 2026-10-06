//! The stochastic twin's draws: SplitMix64, a binomial by Bernoulli trials (populations
//! here are tens of bodies), and the [`Draws`] switch that lets one code path run both the
//! mean field (expected values) and the twin (integer counts).

/// SplitMix64.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Binomial(`n`, `p`) by trials; `n` is rounded.
    pub fn binomial(&mut self, n: f64, p: f64) -> f64 {
        let n = n.round().max(0.0) as u64;
        let p = p.clamp(0.0, 1.0);
        if n == 0 || p == 0.0 {
            return 0.0;
        }
        if p == 1.0 {
            return n as f64;
        }
        (0..n).filter(|_| self.unit() < p).count() as f64
    }
}

/// **Mean field or twin.** `Mean` returns expected values (`n · p`); `Twin` draws integer
/// outcomes. Every stochastic event in the model (a death by hazard, a nest refusal, an egg
/// lost, a dormant egg hatching) asks this, so the two runs share every law.
#[derive(Clone, Debug)]
pub enum Draws {
    Mean,
    Twin(Rng),
}

impl Draws {
    /// How many of `n` bodies an event with chance `p` takes.
    pub fn of(&mut self, n: f64, p: f64) -> f64 {
        match self {
            Draws::Mean => n * p.clamp(0.0, 1.0),
            Draws::Twin(r) => r.binomial(n, p),
        }
    }

    pub fn is_twin(&self) -> bool {
        matches!(self, Draws::Twin(_))
    }

    /// A uniform draw in `[0, 1)` for the twin; the mean field's is 0.5 (callers that need the
    /// mean field's expectation of a discrete draw compute it themselves).
    pub fn unit(&mut self) -> f64 {
        match self {
            Draws::Mean => 0.5,
            Draws::Twin(r) => r.unit(),
        }
    }
}
