//! Genome v1 and phenotype decode.

use serde::{Deserialize, Serialize};

use crate::config::{DriveConfig, OrganismConfig};

/// Bounded heritable parameters. M2 uses one fixed genotype (`Genome::founder`); M3a adds
/// sparse mutation. Every field has a documented closed range enforced by `clamp`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Genome {
    pub version: u32,
    /// 0.5–2: scales adult structure, reserve, energy, mouth (¾ power), speed (−¼ power).
    pub size: f32,
    /// 0.5–2: multiplies maintenance.
    pub metabolism: f32,
    /// 2–12 px sensing radius.
    pub sense: f32,
    /// 0.5–2: multiplies reserve capacity.
    pub reserve: f32,
    /// 0.2–1: multiplies mouth intake rate.
    pub mouth: f32,
    /// 0.3–1: multiplies maximum speed.
    pub speed: f32,
    /// 0–1 cosmetic hue accent.
    pub hue: f32,
    /// Named drives; M2 copies the config defaults, M3a mutates within bounds.
    pub drives: Drives,
}

/// Heritable behavioral gains and thresholds. Ranges: weights 0–2, thresholds 0–1 with
/// `seek_off < seek_on` enforced on decode, `tau_hunger` 1–60 s, `turn_noise` 0–2.
/// The two quantities the spec leaves open are bounded by `clamp` at
/// `bud_min_age_seconds ≤ 3600` and `turn_rate_max_deg ≤ 360`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drives {
    pub w_food: f32,
    pub w_detritus: f32,
    pub w_persist: f32,
    pub w_crowd: f32,
    pub seek_on: f32,
    pub seek_off: f32,
    pub feed_min: f32,
    pub rest_effort: f32,
    pub feed_effort: f32,
    pub bud_reserve: f32,
    pub bud_energy: f32,
    pub bud_min_age_seconds: f32,
    pub tau_hunger_seconds: f32,
    pub turn_rate_max_deg: f32,
    pub turn_noise: f32,
}

impl Genome {
    pub const VERSION: u32 = 1;

    /// The M2 founder genotype: all multipliers 1 (mouth 1, speed 1, sense 6), hue from the
    /// argument, drives copied from `DriveConfig`.
    pub fn founder(hue: f32, drives: &DriveConfig) -> Genome {
        Genome {
            version: Genome::VERSION,
            size: 1.0,
            metabolism: 1.0,
            sense: 6.0,
            reserve: 1.0,
            mouth: 1.0,
            speed: 1.0,
            hue,
            drives: Drives {
                w_food: drives.w_food as f32,
                w_detritus: drives.w_detritus as f32,
                w_persist: drives.w_persist as f32,
                w_crowd: drives.w_crowd as f32,
                seek_on: drives.seek_on as f32,
                seek_off: drives.seek_off as f32,
                feed_min: drives.feed_min as f32,
                rest_effort: drives.rest_effort as f32,
                feed_effort: drives.feed_effort as f32,
                bud_reserve: drives.bud_reserve as f32,
                bud_energy: drives.bud_energy as f32,
                bud_min_age_seconds: drives.bud_min_age_seconds as f32,
                tau_hunger_seconds: drives.tau_hunger_seconds as f32,
                turn_rate_max_deg: drives.turn_rate_max_deg as f32,
                turn_noise: drives.turn_noise as f32,
            },
        }
    }

    /// Clamp every field into its documented range (in place). Returns true if anything changed.
    pub fn clamp(&mut self) -> bool {
        let mut changed = false;
        let mut fix = |v: &mut f32, lo: f32, hi: f32| {
            let c = if v.is_nan() { lo } else { v.clamp(lo, hi) };
            if c != *v {
                *v = c;
                changed = true;
            }
        };
        fix(&mut self.size, 0.5, 2.0);
        fix(&mut self.metabolism, 0.5, 2.0);
        fix(&mut self.sense, 2.0, 12.0);
        fix(&mut self.reserve, 0.5, 2.0);
        fix(&mut self.mouth, 0.2, 1.0);
        fix(&mut self.speed, 0.3, 1.0);
        fix(&mut self.hue, 0.0, 1.0);
        let d = &mut self.drives;
        for w in [&mut d.w_food, &mut d.w_detritus, &mut d.w_persist, &mut d.w_crowd] {
            fix(w, 0.0, 2.0);
        }
        for t in [
            &mut d.seek_on,
            &mut d.seek_off,
            &mut d.feed_min,
            &mut d.rest_effort,
            &mut d.feed_effort,
            &mut d.bud_reserve,
            &mut d.bud_energy,
        ] {
            fix(t, 0.0, 1.0);
        }
        // The spec leaves these two unbounded; the clamp uses the widest values that stay
        // physically meaningful (an hour of waiting, a full turn per second).
        fix(&mut d.bud_min_age_seconds, 0.0, 3600.0);
        fix(&mut d.tau_hunger_seconds, 1.0, 60.0);
        fix(&mut d.turn_rate_max_deg, 0.0, 360.0);
        fix(&mut d.turn_noise, 0.0, 2.0);
        changed
    }

    /// A stable 64-bit digest of the genome (FNV-1a over the little-endian field bytes,
    /// in declaration order) for lineage records.
    pub fn digest(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut eat = |bytes: &[u8]| {
            for &b in bytes {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x100_0000_01b3);
            }
        };
        eat(&self.version.to_le_bytes());
        let d = &self.drives;
        for f in [
            self.size,
            self.metabolism,
            self.sense,
            self.reserve,
            self.mouth,
            self.speed,
            self.hue,
            d.w_food,
            d.w_detritus,
            d.w_persist,
            d.w_crowd,
            d.seek_on,
            d.seek_off,
            d.feed_min,
            d.rest_effort,
            d.feed_effort,
            d.bud_reserve,
            d.bud_energy,
            d.bud_min_age_seconds,
            d.tau_hunger_seconds,
            d.turn_rate_max_deg,
            d.turn_noise,
        ] {
            eat(&f.to_le_bytes());
        }
        h
    }
}

/// Decoded once at birth. All physical quantities in world units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Phenotype {
    pub structure_adult: f64,
    pub reserve_max: f64,
    pub energy_max: f64,
    pub speed_max: f64,
    pub mouth_rate: f64,
    pub sense_radius: f64,
    pub maintenance: f64,
    /// Body lobes in the body frame: `(offset_x, offset_y, radius)`, extent ≤ config max.
    pub lobes: Vec<(f64, f64, f64)>,
    pub extent: f64,
    pub hue: f32,
    pub drives: Drives,
}

/// Decode per `design/m2-world-spec.md` "Organism representation":
/// `S_adult = size · structure_adult`, `R_max = reserve · size · reserve_max`,
/// `E_max = energy_max · size`, `v_max = speed · speed_max · size^(−0.25)`,
/// `mouth_rate = mouth · mouth_rate · size^0.75`, `sense_radius = sense`,
/// `maintenance = metabolism · maintenance`; lobes: core `(0, 0, 0.9 + 0.5·size)`, head
/// `(1.6·size, 0, 0.6 + 0.3·size)`, and tail `(−1.4·size, 0, 0.5 + 0.2·size)` when
/// `speed > 0.6`; the extent is then clamped by scaling offsets down if it exceeds
/// `body_extent_max` (record nothing; the clamp is a decode rule).
pub fn decode(genome: &Genome, cfg: &OrganismConfig) -> Phenotype {
    let size = f64::from(genome.size);
    let mut lobes = vec![(0.0, 0.0, 0.9 + 0.5 * size), (1.6 * size, 0.0, 0.6 + 0.3 * size)];
    if genome.speed > 0.6 {
        lobes.push((-1.4 * size, 0.0, 0.5 + 0.2 * size));
    }
    let extent_of = |lobes: &[(f64, f64, f64)]| {
        lobes.iter().fold(0.0f64, |m, &(x, y, r)| m.max(x.hypot(y) + r))
    };
    let mut extent = extent_of(&lobes);
    if extent > cfg.body_extent_max && extent > 0.0 {
        // Decode rule: shrink the offsets (radii unchanged) until the body fits.
        let s = cfg.body_extent_max / extent;
        for lobe in &mut lobes {
            lobe.0 *= s;
            lobe.1 *= s;
        }
        extent = extent_of(&lobes);
    }

    let mut drives = genome.drives.clone();
    // `seek_off < seek_on` is enforced here rather than in `clamp`, which treats the two
    // thresholds independently.
    if drives.seek_off > drives.seek_on {
        drives.seek_off = drives.seek_on;
    }

    Phenotype {
        structure_adult: size * cfg.structure_adult,
        reserve_max: f64::from(genome.reserve) * size * cfg.reserve_max,
        energy_max: cfg.energy_max * size,
        speed_max: f64::from(genome.speed) * cfg.speed_max * size.powf(-0.25),
        mouth_rate: f64::from(genome.mouth) * cfg.mouth_rate * size.powf(0.75),
        sense_radius: f64::from(genome.sense),
        maintenance: f64::from(genome.metabolism) * cfg.maintenance,
        lobes,
        extent,
        hue: genome.hue,
        drives,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldConfig;

    /// Every `f32` field with its documented range and a mutable accessor, so the tests
    /// below cover the whole genome rather than a sample of it.
    #[allow(clippy::type_complexity)]
    fn fields() -> Vec<(&'static str, f32, f32, fn(&mut Genome) -> &mut f32)> {
        vec![
            ("size", 0.5, 2.0, (|g| &mut g.size) as fn(&mut Genome) -> &mut f32),
            ("metabolism", 0.5, 2.0, |g| &mut g.metabolism),
            ("sense", 2.0, 12.0, |g| &mut g.sense),
            ("reserve", 0.5, 2.0, |g| &mut g.reserve),
            ("mouth", 0.2, 1.0, |g| &mut g.mouth),
            ("speed", 0.3, 1.0, |g| &mut g.speed),
            ("hue", 0.0, 1.0, |g| &mut g.hue),
            ("w_food", 0.0, 2.0, |g| &mut g.drives.w_food),
            ("w_detritus", 0.0, 2.0, |g| &mut g.drives.w_detritus),
            ("w_persist", 0.0, 2.0, |g| &mut g.drives.w_persist),
            ("w_crowd", 0.0, 2.0, |g| &mut g.drives.w_crowd),
            ("seek_on", 0.0, 1.0, |g| &mut g.drives.seek_on),
            ("seek_off", 0.0, 1.0, |g| &mut g.drives.seek_off),
            ("feed_min", 0.0, 1.0, |g| &mut g.drives.feed_min),
            ("rest_effort", 0.0, 1.0, |g| &mut g.drives.rest_effort),
            ("feed_effort", 0.0, 1.0, |g| &mut g.drives.feed_effort),
            ("bud_reserve", 0.0, 1.0, |g| &mut g.drives.bud_reserve),
            ("bud_energy", 0.0, 1.0, |g| &mut g.drives.bud_energy),
            ("bud_min_age_seconds", 0.0, 3600.0, |g| &mut g.drives.bud_min_age_seconds),
            ("tau_hunger_seconds", 1.0, 60.0, |g| &mut g.drives.tau_hunger_seconds),
            ("turn_rate_max_deg", 0.0, 360.0, |g| &mut g.drives.turn_rate_max_deg),
            ("turn_noise", 0.0, 2.0, |g| &mut g.drives.turn_noise),
        ]
    }

    fn founder() -> Genome {
        Genome::founder(0.5, &WorldConfig::default().drives)
    }

    #[test]
    fn founder_is_in_range_and_unchanged_by_clamp() {
        let mut g = founder();
        assert_eq!(g.version, Genome::VERSION);
        assert_eq!((g.size, g.metabolism, g.reserve, g.mouth, g.speed), (1.0, 1.0, 1.0, 1.0, 1.0));
        assert_eq!(g.sense, 6.0);
        assert_eq!(g.hue, 0.5);
        assert!(!g.clamp(), "founder genome should already be in range");
        assert_eq!(g, founder());
    }

    #[test]
    fn clamp_bounds_every_field_from_both_sides() {
        for (name, lo, hi, get) in fields() {
            for (bad, want) in [(lo - 10.0, lo), (hi + 10.0, hi), (f32::NAN, lo)] {
                let mut g = founder();
                *get(&mut g) = bad;
                assert!(g.clamp(), "{name}: clamp reported no change for {bad}");
                let got = *get(&mut g);
                assert_eq!(got, want, "{name}: {bad} clamped to {got}, want {want}");
            }
            // In-range values are untouched.
            let mut g = founder();
            let mid = 0.5 * (lo + hi);
            *get(&mut g) = mid;
            g.clamp();
            assert_eq!(*get(&mut g), mid, "{name}: in-range value was altered");
        }
    }

    #[test]
    fn digest_is_stable_and_changes_with_every_field() {
        let base = founder().digest();
        assert_eq!(base, founder().digest());
        for (name, lo, hi, get) in fields() {
            let mut g = founder();
            let v = get(&mut g);
            // Move within the field's own range so the change is meaningful.
            *v = if *v == lo { hi } else { lo };
            assert_ne!(g.digest(), base, "{name}: digest did not change");
        }
        let mut g = founder();
        g.version += 1;
        assert_ne!(g.digest(), base, "version: digest did not change");
    }

    #[test]
    fn decode_matches_the_spec_at_size_one() {
        let cfg = WorldConfig::default().organism;
        let g = founder();
        let p = decode(&g, &cfg);
        assert_eq!(p.structure_adult, 1.0);
        assert_eq!(p.reserve_max, 1.0);
        assert_eq!(p.energy_max, 2.0);
        assert_eq!(p.speed_max, cfg.speed_max);
        assert_eq!(p.mouth_rate, 0.05);
        assert_eq!(p.sense_radius, 6.0);
        assert_eq!(p.maintenance, 0.005);
        // Core, head, and (speed 1.0 > 0.6) tail.
        let want = [(0.0, 0.0, 1.4), (1.6, 0.0, 0.9), (-1.4, 0.0, 0.7)];
        assert_eq!(p.lobes.len(), want.len());
        for (got, w) in p.lobes.iter().zip(want.iter()) {
            assert!((got.0 - w.0).abs() < 1e-12 && (got.1 - w.1).abs() < 1e-12 && (got.2 - w.2).abs() < 1e-12, "{got:?} vs {w:?}");
        }
        assert!((p.extent - 2.5).abs() < 1e-12, "extent {}", p.extent);
        assert_eq!(p.hue, g.hue);
    }

    #[test]
    fn decode_matches_the_spec_at_size_two() {
        let cfg = WorldConfig::default().organism;
        let mut g = founder();
        g.size = 2.0;
        g.reserve = 1.5;
        g.metabolism = 2.0;
        g.mouth = 0.5;
        g.speed = 0.5; // ≤ 0.6: no tail lobe.
        let p = decode(&g, &cfg);
        assert_eq!(p.structure_adult, 2.0);
        assert_eq!(p.reserve_max, 3.0);
        assert_eq!(p.energy_max, 4.0);
        assert!((p.speed_max - 0.5 * cfg.speed_max * 2.0f64.powf(-0.25)).abs() < 1e-15);
        assert!((p.mouth_rate - 0.5 * 0.05 * 2.0f64.powf(0.75)).abs() < 1e-15);
        assert_eq!(p.maintenance, 0.01);
        assert_eq!(p.lobes.len(), 2);
        assert_eq!(p.lobes[0], (0.0, 0.0, 1.9));
        assert!((p.lobes[1].0 - 3.2).abs() < 1e-12 && (p.lobes[1].2 - 1.2).abs() < 1e-12);
        assert!((p.extent - 4.4).abs() < 1e-12, "extent {}", p.extent);
        assert!(p.extent <= cfg.body_extent_max);
    }

    #[test]
    fn decode_clamps_the_extent_by_shrinking_offsets_only() {
        let mut cfg = WorldConfig::default().organism;
        cfg.body_extent_max = 3.0;
        let mut g = founder();
        g.size = 2.0;
        let unclamped = {
            let mut wide = cfg.clone();
            wide.body_extent_max = 100.0;
            decode(&g, &wide)
        };
        assert!((unclamped.extent - 4.4).abs() < 1e-12);
        let p = decode(&g, &cfg);
        let s = 3.0 / 4.4;
        // Radii are unchanged; offsets shrank by body_extent_max / extent.
        for (a, b) in p.lobes.iter().zip(unclamped.lobes.iter()) {
            assert!((a.0 - b.0 * s).abs() < 1e-12, "{a:?} vs {b:?}");
            assert_eq!(a.1, b.1 * s);
            assert_eq!(a.2, b.2);
        }
        // The recorded extent is the recomputed one, which — because radii are not scaled —
        // may still sit above `body_extent_max` for a body that is mostly radius. The rule
        // is one scaling pass, not a fixed point.
        assert!(p.extent < unclamped.extent, "{} vs {}", p.extent, unclamped.extent);
        assert!((p.extent - (3.2 * s + 1.2)).abs() < 1e-12, "extent {}", p.extent);
        // Genomes inside the documented ranges always fit the default bound.
        let default_cfg = WorldConfig::default().organism;
        assert!(decode(&g, &default_cfg).extent <= default_cfg.body_extent_max);
    }

    #[test]
    fn decode_enforces_seek_off_not_above_seek_on() {
        let cfg = WorldConfig::default().organism;
        let mut g = founder();
        g.drives.seek_on = 0.2;
        g.drives.seek_off = 0.9;
        let p = decode(&g, &cfg);
        assert_eq!(p.drives.seek_off, 0.2);
        assert_eq!(p.drives.seek_on, 0.2);
    }
}
