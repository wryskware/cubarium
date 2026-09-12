//! Genome v2 and phenotype decode, plus the sparse mutation of `design/fauna-v2.md`.
//!
//! Version 2 adds `diet`, `depth`, `swim` and `form` (and the drive `w_depth`) to v1. A v1
//! genome decodes with the documented defaults and [`Genome::upgrade`] stamps it version 2
//! in place, so worlds written before fauna v2 keep their organisms.

use serde::{Deserialize, Serialize};

use crate::config::{DriveConfig, OrganismConfig};

/// Bounded heritable parameters. Every field has a documented closed range enforced by
/// `clamp`. The v2 fields carry serde defaults so a v1 encoding still decodes.
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
    /// 0–1: grazing rate `mouth_rate · diet`, scavenging rate `mouth_rate · (1 − diet)`; the
    /// steering weight on `∇P` (and `∇F`) scales with `diet`, on `∇D_eff` with `1 − diet`.
    /// Grazing needs `diet ≥ 0.05`, scavenging `diet ≤ 0.95`, fruit `diet ≥ 0.5`.
    #[serde(default = "default_diet")]
    pub diet: f32,
    /// 0–1: preferred height `h_pref = −1 + 2 · depth` (Top = 1, rim = −1).
    #[serde(default = "default_depth")]
    pub depth: f32,
    /// 0–1: wading penalty `speed / (1 + w · (1 − swim))`; a swimmer ignores pools.
    #[serde(default)]
    pub swim: f32,
    /// Which authored rig draws the body, `0..MAX_FORMS`. Heritable, copied exactly, never
    /// mutated. [`FORM_UNSET`] marks a v1 genome awaiting [`Genome::upgrade`], which takes
    /// the hue tercile.
    #[serde(default = "default_form")]
    pub form: u8,
    /// Named drives; founders copy the config defaults, mutation moves them within bounds.
    pub drives: Drives,
}

/// One more than the largest legal `form`; the presenter falls back to the hue tercile for
/// a form beyond its pack.
pub const MAX_FORMS: u8 = 8;
/// `form` value of a genome decoded from a v1 encoding, replaced by [`Genome::upgrade`].
pub const FORM_UNSET: u8 = u8::MAX;

fn default_diet() -> f32 {
    0.7
}
fn default_depth() -> f32 {
    0.5
}
fn default_form() -> u8 {
    FORM_UNSET
}
fn default_w_depth() -> f32 {
    1.0
}

/// The hue tercile as a rig: `min(2, floor(hue × 3))`, NaN → 0. The rule the presenter used
/// before `form` existed, kept so an upgraded v1 organism keeps its look.
pub fn form_of_hue(hue: f32) -> u8 {
    if hue.is_nan() {
        return 0;
    }
    ((hue * 3.0).floor() as i64).clamp(0, 2) as u8
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
    /// 0–2: gain of the depth term `w_depth · (h_pref − h) · up` in the steering.
    #[serde(default = "default_w_depth")]
    pub w_depth: f32,
}

/// One locus changed by mutation, as recorded in the birth event and nowhere else.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Mutation {
    pub locus: &'static str,
    pub from: f32,
    pub to: f32,
}

/// The loci sparse mutation may touch (`design/fauna-v2.md` "Mutation"), in the order a
/// uniform draw indexes them. `form` is never here: the look is the lineage's badge.
pub const MUTABLE_LOCI: [&str; 10] =
    ["size", "speed", "sense", "reserve", "mouth", "hue", "diet", "depth", "swim", "w_depth"];

/// Closed range of a mutable locus, in `MUTABLE_LOCI` order.
const LOCUS_RANGES: [(f32, f32); 10] = [
    (0.5, 2.0),
    (0.3, 1.0),
    (2.0, 12.0),
    (0.5, 2.0),
    (0.2, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 2.0),
];

impl Genome {
    pub const VERSION: u32 = 2;

    /// Mutable access to a locus by its `MUTABLE_LOCI` index.
    fn locus_mut(&mut self, index: usize) -> &mut f32 {
        match index {
            0 => &mut self.size,
            1 => &mut self.speed,
            2 => &mut self.sense,
            3 => &mut self.reserve,
            4 => &mut self.mouth,
            5 => &mut self.hue,
            6 => &mut self.diet,
            7 => &mut self.depth,
            8 => &mut self.swim,
            _ => &mut self.drives.w_depth,
        }
    }

    /// Bring a decoded genome to the current version in place. A v1 genome (or any genome
    /// with `form == FORM_UNSET`) takes `form` from its hue tercile and the serde defaults
    /// for the other v2 loci, and is stamped version 2. Returns true if anything changed.
    pub fn upgrade(&mut self) -> bool {
        let mut changed = false;
        if self.form == FORM_UNSET {
            self.form = form_of_hue(self.hue);
            changed = true;
        }
        if self.version == 1 {
            self.version = Genome::VERSION;
            changed = true;
        }
        changed
    }

    /// Sparse mutation (`design/fauna-v2.md`): with probability `probability` the genome
    /// changes at one or two loci (equally likely) drawn without replacement from
    /// [`MUTABLE_LOCI`], each by a Gaussian step of `step` of the locus range, clamped to
    /// the range. `form` never changes. `unit` supplies uniform `[0, 1)` draws in a fixed
    /// order (6 draws at most: gate, count, first locus, second locus, then two per step),
    /// so the caller's counter-based stream makes the result reproducible. Returns the
    /// changes made, in the order they were applied; empty when the copy is exact. A step
    /// that clamps back onto the parent's own value (a locus already at a range boundary,
    /// pushed further out) is not a change and records nothing, so at the boundaries a child
    /// differs less often than `probability`.
    pub fn mutate(&mut self, probability: f64, step: f64, mut unit: impl FnMut() -> f64) -> Vec<Mutation> {
        let mut out = Vec::new();
        if probability <= 0.0 || probability.is_nan() || unit() >= probability {
            return out;
        }
        let count = if unit() < 0.5 { 1 } else { 2 };
        let n = MUTABLE_LOCI.len();
        let first = ((unit() * n as f64).floor() as usize).min(n - 1);
        let mut loci = vec![first];
        if count == 2 {
            let pick = ((unit() * (n - 1) as f64).floor() as usize).min(n - 2);
            loci.push(if pick >= first { pick + 1 } else { pick });
        }
        for locus in loci {
            let (lo, hi) = LOCUS_RANGES[locus];
            let u1 = unit();
            let u2 = unit();
            let u1 = if u1 == 0.0 { 1.0 } else { u1 };
            let gaussian = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
            let delta = (gaussian * step * f64::from(hi - lo)) as f32;
            let slot = self.locus_mut(locus);
            let from = *slot;
            let to = (from + delta).clamp(lo, hi);
            if to != from {
                *slot = to;
                out.push(Mutation { locus: MUTABLE_LOCI[locus], from, to });
            }
        }
        out
    }

    /// The M2 founder genotype: all multipliers 1 (mouth 1, speed 1, sense 6), hue from the
    /// argument, the v2 defaults (`diet` 0.7, `depth` 0.5, `swim` 0, `form` = hue tercile),
    /// drives copied from `DriveConfig`.
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
            diet: default_diet(),
            depth: default_depth(),
            swim: 0.0,
            form: form_of_hue(hue),
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
                w_depth: drives.w_depth as f32,
            },
        }
    }

    /// Clamp every field into its documented range (in place). Returns true if anything changed.
    pub fn clamp(&mut self) -> bool {
        let mut changed = false;
        if self.form != FORM_UNSET && self.form >= MAX_FORMS {
            self.form = MAX_FORMS - 1;
            changed = true;
        }
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
        fix(&mut self.diet, 0.0, 1.0);
        fix(&mut self.depth, 0.0, 1.0);
        fix(&mut self.swim, 0.0, 1.0);
        let d = &mut self.drives;
        for w in [&mut d.w_food, &mut d.w_detritus, &mut d.w_persist, &mut d.w_crowd, &mut d.w_depth] {
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
        eat(&[self.form]);
        let d = &self.drives;
        for f in [
            self.size,
            self.metabolism,
            self.sense,
            self.reserve,
            self.mouth,
            self.speed,
            self.hue,
            self.diet,
            self.depth,
            self.swim,
            d.w_depth,
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
    /// `mouth_rate · diet`: the intake rate on producer and fruit.
    pub graze_rate: f64,
    /// `mouth_rate · (1 − diet)`: the intake rate on edible detritus.
    pub scavenge_rate: f64,
    /// The genome's `diet`, widened, for the steering weights and the intake gates.
    pub diet: f64,
    /// Preferred embedded height `−1 + 2 · depth`.
    pub h_pref: f64,
    /// The genome's `swim`, widened.
    pub swim: f64,
    /// The rig index.
    pub form: u8,
    pub drives: Drives,
}

/// Decode per `design/m2-world-spec.md` "Organism representation":
/// `S_adult = size · structure_adult`, `R_max = reserve · size · reserve_max`,
/// `E_max = energy_max · size`, `v_max = speed · speed_max · size^(−0.25)`,
/// `mouth_rate = mouth · mouth_rate · size^0.75`, `sense_radius = sense`,
/// `maintenance = metabolism · maintenance`, `graze_rate = mouth_rate · diet`,
/// `scavenge_rate = mouth_rate · (1 − diet)`, `h_pref = −1 + 2 · depth`
/// (`design/fauna-v2.md`); lobes: core `(0, 0, 0.9 + 0.5·size)`, head
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

    let mouth_rate = f64::from(genome.mouth) * cfg.mouth_rate * size.powf(0.75);
    let diet = f64::from(genome.diet.clamp(0.0, 1.0));
    Phenotype {
        structure_adult: size * cfg.structure_adult,
        reserve_max: f64::from(genome.reserve) * size * cfg.reserve_max,
        energy_max: cfg.energy_max * size,
        speed_max: f64::from(genome.speed) * cfg.speed_max * size.powf(-0.25),
        mouth_rate,
        sense_radius: f64::from(genome.sense),
        maintenance: f64::from(genome.metabolism) * cfg.maintenance,
        lobes,
        extent,
        hue: genome.hue,
        graze_rate: mouth_rate * diet,
        scavenge_rate: mouth_rate * (1.0 - diet),
        diet,
        h_pref: -1.0 + 2.0 * f64::from(genome.depth.clamp(0.0, 1.0)),
        swim: f64::from(genome.swim.clamp(0.0, 1.0)),
        form: if genome.form == FORM_UNSET { form_of_hue(genome.hue) } else { genome.form },
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
            ("diet", 0.0, 1.0, |g| &mut g.diet),
            ("depth", 0.0, 1.0, |g| &mut g.depth),
            ("swim", 0.0, 1.0, |g| &mut g.swim),
            ("w_depth", 0.0, 2.0, |g| &mut g.drives.w_depth),
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
        assert_eq!((g.diet, g.depth, g.swim, g.form), (0.7, 0.5, 0.0, 1), "v2 founder defaults; hue 0.5 is tercile 1");
        assert_eq!(g.drives.w_depth, 1.0);
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

    // --- Genome v2 (`design/fauna-v2.md`) ------------------------------------------------

    #[test]
    fn a_v1_encoding_decodes_with_the_defaults_and_upgrades_in_place() {
        // Serialize a founder, strip the v2 keys, and read it back as a v1 genome would be.
        let mut v1 = founder();
        v1.version = 1;
        let text = toml::to_string(&v1).expect("a genome serializes");
        let stripped: String = text
            .lines()
            .filter(|l| !(l.starts_with("diet") || l.starts_with("depth") || l.starts_with("swim") || l.starts_with("form") || l.starts_with("w_depth")))
            .map(|l| format!("{l}\n"))
            .collect();
        assert!(!stripped.contains("diet"), "the v2 keys were stripped: {stripped}");
        let mut g: Genome = toml::from_str(&stripped).expect("a v1 genome still decodes");
        assert_eq!(g.version, 1);
        assert_eq!((g.diet, g.depth, g.swim), (0.7, 0.5, 0.0), "the design's v1 defaults");
        assert_eq!(g.form, FORM_UNSET);
        assert_eq!(g.drives.w_depth, 1.0);
        assert!(g.upgrade(), "an upgrade changes a v1 genome");
        assert_eq!(g.version, Genome::VERSION);
        assert_eq!(g.form, form_of_hue(g.hue));
        assert_eq!(g.form, 1, "hue 0.5 is the middle tercile");
        assert!(!g.upgrade(), "a second upgrade is a no-op");
        // A current genome is untouched.
        let mut current = founder();
        assert!(!current.upgrade());
        assert_eq!(current, founder());
    }

    #[test]
    fn the_form_tercile_and_clamp_cover_the_new_loci() {
        assert_eq!((form_of_hue(0.0), form_of_hue(0.33), form_of_hue(0.34), form_of_hue(0.99), form_of_hue(1.0)), (0, 0, 1, 2, 2));
        assert_eq!(form_of_hue(f32::NAN), 0);
        let mut g = founder();
        g.form = MAX_FORMS + 3;
        assert!(g.clamp());
        assert_eq!(g.form, MAX_FORMS - 1);
        let mut unset = founder();
        unset.form = FORM_UNSET;
        unset.clamp();
        assert_eq!(unset.form, FORM_UNSET, "clamp leaves the upgrade marker for `upgrade`");
    }

    #[test]
    fn the_digest_sees_form_and_the_phenotype_carries_the_kind() {
        let base = founder().digest();
        let mut g = founder();
        g.form = 3;
        assert_ne!(g.digest(), base, "form: digest did not change");
        let cfg = WorldConfig::default().organism;
        let mut g = founder();
        g.diet = 0.25;
        g.depth = 1.0;
        g.swim = 0.5;
        g.form = 3;
        let p = decode(&g, &cfg);
        assert!((p.diet - f64::from(0.25f32)).abs() < 1e-12);
        assert!((p.graze_rate - p.mouth_rate * f64::from(0.25f32)).abs() < 1e-15);
        assert!((p.scavenge_rate - p.mouth_rate * (1.0 - f64::from(0.25f32))).abs() < 1e-15);
        assert!((p.graze_rate + p.scavenge_rate - p.mouth_rate).abs() < 1e-15, "specialization is a trade");
        assert_eq!(p.h_pref, 1.0);
        assert_eq!(p.swim, 0.5);
        assert_eq!(p.form, 3);
        // An un-upgraded genome still decodes to a usable rig.
        g.form = FORM_UNSET;
        assert_eq!(decode(&g, &cfg).form, form_of_hue(g.hue));
    }

    /// A counter-based unit source like the world's `Stream::Birth` draws.
    fn unit_stream(seed: u64) -> impl FnMut() -> f64 {
        let mut counter = 0u64;
        move || {
            let u = crate::rng::unit(seed, crate::rng::Stream::Birth, 7, counter);
            counter += 1;
            u
        }
    }

    #[test]
    fn mutation_is_sparse_bounded_recorded_and_never_touches_form() {
        let (mut mutated, mut one, mut two) = (0u32, 0u32, 0u32);
        let mut touched = std::collections::HashMap::<&str, u32>::new();
        // Every locus mid-range, so no step can clamp back onto the parent and the recorded
        // rate is the gate's own (`speed`, `mouth` and `swim` sit on a boundary in the
        // founder genome).
        let parent = {
            let mut g = founder();
            g.form = 5;
            g.speed = 0.65;
            g.mouth = 0.6;
            g.swim = 0.5;
            g
        };
        const N: u64 = 4000;
        for seed in 0..N {
            let mut g = parent.clone();
            let before = g.clone();
            let changes = g.mutate(0.3, 0.08, unit_stream(seed));
            assert_eq!(g.form, 5, "form never mutates");
            assert_eq!(g.version, before.version);
            assert!(!g.clone().clamp(), "a mutated genome is always in range: {g:?}");
            match changes.len() {
                0 => assert_eq!(g, before, "no record, no change"),
                1 => one += 1,
                2 => two += 1,
                n => panic!("{n} loci changed in one birth"),
            }
            if !changes.is_empty() {
                mutated += 1;
            }
            for m in &changes {
                assert!(MUTABLE_LOCI.contains(&m.locus));
                assert_ne!(m.from, m.to);
                *touched.entry(m.locus).or_default() += 1;
            }
            // The record is exact: applying it to the parent gives the child.
            let mut replay = before.clone();
            for m in &changes {
                let i = MUTABLE_LOCI.iter().position(|l| *l == m.locus).expect("a mutable locus");
                assert_eq!(*replay.locus_mut(i), m.from, "{}: `from` is the parent's value", m.locus);
                *replay.locus_mut(i) = m.to;
            }
            assert_eq!(replay, g);
        }
        let rate = f64::from(mutated) / N as f64;
        assert!((rate - 0.3).abs() < 0.03, "mutation rate {rate} vs p_mut 0.3");
        assert!(one > 0 && two > 0, "both one- and two-locus births occur ({one}, {two})");
        assert!((f64::from(one) / f64::from(one + two) - 0.5).abs() < 0.06, "one vs two loci are equally likely");
        assert_eq!(touched.len(), MUTABLE_LOCI.len(), "every mutable locus was touched: {touched:?}");
        assert!(!touched.contains_key("form"));

        // Probability zero and a zero step are exact copies; a step is bounded by the range.
        let mut g = founder();
        assert!(g.mutate(0.0, 0.08, unit_stream(1)).is_empty());
        assert_eq!(g, founder());
        let mut g = founder();
        assert!(g.mutate(1.0, 0.0, unit_stream(1)).is_empty(), "a zero step changes nothing");
        assert_eq!(g, founder());
        let mut g = founder();
        g.mutate(1.0, 100.0, unit_stream(3));
        assert!(!g.clone().clamp(), "even a huge step is clamped into range");
    }
}
