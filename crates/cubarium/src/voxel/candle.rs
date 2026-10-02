//! The glowcap candle (Wrysk, 2026-10-02; `design/handoffs/glowcap-candle-vine-tiles-2026-10-02.md`):
//! each glowcap's lip brightens and dims slowly, like a candle, and only a few caps are
//! near full at any moment.
//!
//! A cap's brightness is `b = candle_min^(1 − u)` with `u` uniform over time, so across
//! caps at any instant the density of `b` is ∝ 1/b on `[candle_min, 1]`: the share within
//! 10 % of full is `ln(1/0.9) / ln(1/candle_min)`, 7.6 % at 0.25. `u` is a smooth wander:
//! three sines of about `candle_period_s` at the cap's own phases and paces, summed, then
//! mapped through the sum's own distribution function ([`sum_cdf`]), which makes it
//! exactly uniform while keeping it smooth.
//!
//! Every cap draws one of [`CLASSES`] curves, picked by its stand id: the renderer gives
//! each curve its own copy of the lip's style, and a style is a scarce slot
//! (`cubarium_gpu::voxel::MAX_STYLES`). A later spike (a cap flaring when an animal
//! passes) joins in [`Candle::brightness`].

use std::f64::consts::TAU;
use std::sync::OnceLock;

use super::LightConfig;

/// How many candle curves the caps share out. Two caps share a curve one time in this
/// many; each emitting lip style splits into this many render styles.
pub const CLASSES: u8 = 16;

/// How a cap's brightness wanders: `[light] candle_min` and `candle_period_s`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candle {
    pub min: f32,
    pub period_s: f32,
}

impl Candle {
    pub fn of(light: &LightConfig) -> Candle {
        Candle {
            min: light.candle_min,
            period_s: light.candle_period_s,
        }
    }

    /// Whether any cap ever dims: off (`candle_min` 1) is the picture without a candle.
    pub fn on(&self) -> bool {
        self.min < 1.0 && self.period_s > 0.0
    }

    /// The candle curve a stand draws with, `1..=CLASSES`; `0` (always full) when the
    /// candle is off.
    pub fn class_of(&self, stand_id: u64) -> u8 {
        if !self.on() {
            return 0;
        }
        1 + (mix(stand_id ^ 0x6c61_6d70_6c69_7021) % u64::from(CLASSES)) as u8
    }

    /// Curve `class`'s brightness at `seconds` of the presenter's clock, in
    /// `[candle_min, 1]`; `1` for class 0 or with the candle off.
    pub fn class_brightness(&self, class: u8, seconds: f64) -> f32 {
        if class == 0 || !self.on() {
            return 1.0;
        }
        let mut h = mix(u64::from(class).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut x = 0.0;
        for ratio in PACES {
            h = mix(h);
            let phase = (h >> 11) as f64 / (1u64 << 53) as f64 * TAU;
            let pace = 0.85 + 0.3 * ((h & 0xffff) as f64 / 65535.0);
            let period = f64::from(self.period_s) * ratio * pace;
            x += (TAU * seconds / period + phase).sin();
        }
        let u = sum_cdf(x);
        let min = f64::from(self.min.max(1e-3));
        (min.powf(1.0 - u) as f32).clamp(self.min.max(1e-3), 1.0)
    }

    /// The cap's brightness at `seconds`: what multiplies its lip's emission and the light
    /// it casts.
    pub fn brightness(&self, stand_id: u64, seconds: f64) -> f32 {
        self.class_brightness(self.class_of(stand_id), seconds)
    }
}

/// The three sines' periods, in `candle_period_s` (before each curve's own ±15 %).
const PACES: [f64; 3] = [1.0, 0.71, 1.43];

/// splitmix64's finaliser.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Bins per unit of the arcsine table behind [`sum_cdf`].
const BINS_PER_UNIT: usize = 256;

/// The distribution function of `sin a + sin b + sin c` for independent uniform phases:
/// the time share a sum of three incommensurate sines spends at or below `x`. Built once
/// by convolving the arcsine law's bin masses, linear between its knots.
fn sum_cdf(x: f64) -> f64 {
    static KNOTS: OnceLock<Vec<f64>> = OnceLock::new();
    let knots = KNOTS.get_or_init(|| {
        let h = 1.0 / BINS_PER_UNIT as f64;
        let arcsine = |y: f64| 0.5 + y.clamp(-1.0, 1.0).asin() / std::f64::consts::PI;
        let one: Vec<f64> = (0..2 * BINS_PER_UNIT)
            .map(|i| arcsine(-1.0 + (i + 1) as f64 * h) - arcsine(-1.0 + i as f64 * h))
            .collect();
        let conv = |a: &[f64], b: &[f64]| {
            let mut out = vec![0.0; a.len() + b.len() - 1];
            for (i, &p) in a.iter().enumerate() {
                for (j, &q) in b.iter().enumerate() {
                    out[i + j] += p * q;
                }
            }
            out
        };
        let three = conv(&conv(&one, &one), &one);
        // Bin-sum n sits at -3 + (n + 1.5) h; its mass spread over one bin around that,
        // so knot n (the cumulative mass below bin-sum n) is at -3 + (n + 1) h.
        let mut knots = Vec::with_capacity(three.len() + 1);
        let mut cum = 0.0;
        knots.push(0.0);
        for m in three {
            cum += m;
            knots.push(cum);
        }
        let total = cum;
        knots.iter().map(|k| k / total).collect()
    });
    let h = 1.0 / BINS_PER_UNIT as f64;
    let at = (x + 3.0) / h - 1.0;
    if at <= 0.0 {
        return 0.0;
    }
    let i = at.floor() as usize;
    if i + 1 >= knots.len() {
        return 1.0;
    }
    let f = at - i as f64;
    knots[i] + (knots[i + 1] - knots[i]) * f
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANDLE: Candle = Candle {
        min: 0.25,
        period_s: 8.0,
    };

    /// Across 2,000 caps at 600 instants the brightness is reciprocal on [0.25, 1]: about
    /// 7.6 % within 10 % of full, the median at 0.25^0.5.
    #[test]
    fn brightness_is_reciprocal_across_caps() {
        let mut all = Vec::with_capacity(2000 * 600);
        for id in 0..2000u64 {
            for k in 0..600 {
                all.push(CANDLE.brightness(id, 100.0 + k as f64 * 1.37));
            }
        }
        assert!(all.iter().all(|&b| (0.25..=1.0).contains(&b)));
        let bright = all.iter().filter(|&&b| b >= 0.9).count() as f64 / all.len() as f64;
        assert!((0.05..=0.10).contains(&bright), "share at b ≥ 0.9: {bright}");
        all.sort_by(f32::total_cmp);
        let median = all[all.len() / 2];
        assert!((median - 0.5).abs() <= 0.05, "median {median}");
    }

    /// No frame pops: a 60 fps step moves no cap by more than 0.02.
    #[test]
    fn brightness_moves_little_per_frame() {
        for id in 0..2000u64 {
            let mut last = CANDLE.brightness(id, 0.0);
            for f in 1..=3600 {
                let b = CANDLE.brightness(id, f as f64 / 60.0);
                assert!((b - last).abs() <= 0.02, "cap {id} frame {f}: {last} → {b}");
                last = b;
            }
        }
    }

    /// A stand id is its own curve, every time; two stands are not the same curve.
    #[test]
    fn a_stand_keeps_its_own_curve() {
        let at = |id: u64| (0..200).map(|k| CANDLE.brightness(id, k as f64 * 0.5)).collect::<Vec<_>>();
        assert_eq!(at(7), at(7));
        let (a, b) = (at(1), at(2));
        assert!(a.iter().zip(&b).any(|(x, y)| (x - y).abs() > 0.05));
    }

    /// `candle_min` 1 is no candle: every cap full, always.
    #[test]
    fn min_one_is_always_full() {
        let off = Candle {
            min: 1.0,
            period_s: 8.0,
        };
        assert!(!off.on());
        for id in 0..50u64 {
            for k in 0..100 {
                assert_eq!(off.brightness(id, k as f64 * 0.37), 1.0);
            }
        }
    }
}
