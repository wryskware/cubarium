//! The glowcap candle (Wrysk, 2026-10-02; `design/handoffs/glowcap-candle-vine-tiles-2026-10-02.md`):
//! each glowcap's lip brightens and dims slowly, like a candle, and only a few caps are
//! near full at any moment.

use super::LightConfig;

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
        todo!()
    }

    /// The candle curve a stand draws with: `0` (always full) when the candle is off.
    pub fn class_of(&self, _stand_id: u64) -> u8 {
        todo!()
    }

    /// Curve `class`'s brightness at `seconds` of the presenter's clock.
    pub fn class_brightness(&self, _class: u8, _seconds: f64) -> f32 {
        todo!()
    }

    /// The cap's brightness at `seconds`: what multiplies its lip's emission and the light
    /// it casts.
    pub fn brightness(&self, stand_id: u64, seconds: f64) -> f32 {
        self.class_brightness(self.class_of(stand_id), seconds)
    }
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
