//! **The R/S/F pre-screen** (`design/ecology-parameter-registry-2026-09-28.md` §5; audit
//! §1.4): stay-or-go arithmetic over the resolved parameters and the founding plan, so a
//! candidate world where parking wins (or walking cannot feed) by arithmetic never costs
//! a run. A port of the audit's `scripts/invariants.py`, for the browser (frondgrazer) on
//! foliage — the script's only animal — with its fixed numbers read from the configs.
//!
//! Per founded photosynthetic species `s` (founder density and cap above zero), adult:
//! - `P = alpha · wood_max`; the mouth band's share `b` of its foliage (the adult profile's
//!   layers below the browser's mouth ceiling); edible `E = (1 − graze_refuge) · b · P`.
//! - regrowth `G = min(foliage_rate · W, max(0, (assimilation · P · light_moisture −
//!   maintenance · W) / (1 + build)))` — regrowth fills from the bottom, so into the band;
//!   zero for a crown with nothing in the band.
//! - stands in reach `n = clamp(A_mouth / a, 1, founder_colony)`, with `A_mouth = π (width/2 +
//!   mouth reach)²` and `a = max(cell area, π r_crown²)` a stand's footprint.
//!
//! and with the browser's need at rest `need = maintenance · body_max / yield(foliage)`,
//! walking `need_walk = (maintenance + motor) · body_max / yield`, turnover `B = 1 /
//! maintenance`, cruise `v` and swath `w` (its width):
//! - **R** = `max_s n·G / need` — regrowth in reach ÷ need;
//! - **S** = `max_s n·E / (need · B)` — standing food in reach ÷ need, per turnover;
//! - **F** = `max_s (E / a) · v · w · harvest / need_walk` — a walker on the densest field.
//!
//! A world passes inside R `[0.02, 0.3]`, S `[0.02, 0.2]` and with F ≥ 10 (the audit's
//! targets: one spot must run out, and walking must still feed); outside them a candidate
//! takes a graded penalty unrun ([`ScreenSpec::graded`]). Every number is a
//! [`ScreenSpec`] field, read from the search file's `[screen]` table.

use cubarium_voxel_fauna::{FaunaConfig, Food, Founder};
use cubarium_voxel_flora::{FloraConfig, Species, Trophic};
use serde::{Deserialize, Serialize};

/// The screen's bands and its two fixed factors.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScreenSpec {
    /// R's band: regrowth in reach ÷ need.
    pub r: [f64; 2],
    /// S's band: standing food in reach ÷ need, per body turnover.
    pub s: [f64; 2],
    /// F's floor: the walker's feasibility.
    pub f_min: f64,
    /// A candidate outside the bands takes, unrun, `base + k × (its log-distance outside
    /// the bands, summed over R, S and F)`: graded, so CMA has a way back. `base` is raised
    /// past the worst candidate that ran in the same generation, so a refused candidate
    /// always ranks below every run.
    pub base: f64,
    pub k: f64,
    /// **The soft margin**: a candidate outside the bands but within this factor of them
    /// (R and S in `[lo / m, hi × m]`, F at least `f_min / m`) still runs, and pays
    /// `k_soft × (its log-distance outside the bands)` on top of its run's penalty.
    /// Beyond it, refused as above. 1 is no margin.
    pub run_margin: f64,
    pub k_soft: f64,
    /// Light and moisture's factor on a plant's income (invariants.py's 0.67).
    pub light_moisture: f64,
    /// The share of a field's edible foliage a walker takes in passing (0.2).
    pub harvest: f64,
}

impl Default for ScreenSpec {
    fn default() -> ScreenSpec {
        ScreenSpec {
            r: [0.02, 0.3],
            s: [0.02, 0.2],
            f_min: 10.0,
            base: 200.0,
            k: 10.0,
            run_margin: 2.0,
            k_soft: 5.0,
            light_moisture: 0.67,
            harvest: 0.2,
        }
    }
}

/// What the screen does with a candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Inside every band: runs, no soft term.
    Inside,
    /// Outside a band but within the run margin: runs, pays the soft term.
    Margin,
    /// Beyond the margin: not run ([`ScreenSpec::graded`]).
    Refused,
}

impl ScreenSpec {
    /// Inside, in the margin, or refused.
    pub fn verdict(&self, rsf: &Rsf) -> Verdict {
        if rsf.passes(self) {
            return Verdict::Inside;
        }
        let m = self.run_margin.max(1.0);
        let within = (self.r[0] / m..=self.r[1] * m).contains(&rsf.r)
            && (self.s[0] / m..=self.s[1] * m).contains(&rsf.s)
            && rsf.f >= self.f_min / m;
        if within { Verdict::Margin } else { Verdict::Refused }
    }

    /// The soft term a candidate that runs pays: `k_soft ×` its log-distance outside the
    /// bands (zero inside them).
    pub fn soft(&self, rsf: &Rsf) -> f64 {
        self.k_soft * rsf.distance(self)
    }

    /// A refused candidate's fitness ([`ScreenSpec::base`]): above `worst_run`, the worst
    /// fitness of a candidate that ran this generation (minus infinity for none).
    pub fn graded(&self, rsf: &Rsf, worst_run: f64) -> f64 {
        self.base.max(worst_run + 1.0) + self.k * rsf.distance(self)
    }

    /// The `[screen]` table of a search file, defaults for what it leaves out.
    pub fn from_rest(rest: &toml::Table) -> Result<ScreenSpec, String> {
        match rest.get("screen") {
            None => Ok(ScreenSpec::default()),
            Some(v) => v
                .clone()
                .try_into()
                .map_err(|e: toml::de::Error| format!("[screen]: {e}")),
        }
    }
}

/// One species' three numbers.
#[derive(Clone, Debug, Serialize)]
pub struct SpeciesRsf {
    pub species: &'static str,
    pub stands_in_reach: f64,
    pub edible: f64,
    pub regrowth_per_s: f64,
    pub r: f64,
    pub s: f64,
    pub f: f64,
}

/// A world's R, S and F (the most permissive species each) and the per-species rows.
#[derive(Clone, Debug, Serialize)]
pub struct Rsf {
    pub r: f64,
    pub s: f64,
    pub f: f64,
    pub need_per_s: f64,
    pub species: Vec<SpeciesRsf>,
}

impl Rsf {
    /// What lies outside the bands, one line each (`R …`, `S …`, `F …`).
    pub fn failures(&self, spec: &ScreenSpec) -> Vec<String> {
        let mut out = Vec::new();
        if !(spec.r[0]..=spec.r[1]).contains(&self.r) {
            out.push(format!(
                "R {:.3} outside [{}, {}]",
                self.r, spec.r[0], spec.r[1]
            ));
        }
        if !(spec.s[0]..=spec.s[1]).contains(&self.s) {
            out.push(format!(
                "S {:.3} outside [{}, {}]",
                self.s, spec.s[0], spec.s[1]
            ));
        }
        if !(self.f >= spec.f_min) {
            out.push(format!("F {:.1} below {}", self.f, spec.f_min));
        }
        out
    }

    /// The log-distance outside the bands, summed over R, S and F: zero inside them.
    pub fn distance(&self, spec: &ScreenSpec) -> f64 {
        let ln = |x: f64| x.max(1e-12).ln();
        let band = |x: f64, lo: f64, hi: f64| (ln(lo) - ln(x)).max(0.0) + (ln(x) - ln(hi)).max(0.0);
        band(self.r, spec.r[0], spec.r[1])
            + band(self.s, spec.s[0], spec.s[1])
            + (ln(spec.f_min) - ln(self.f)).max(0.0)
    }

    pub fn passes(&self, spec: &ScreenSpec) -> bool {
        self.failures(spec).is_empty()
    }
}

/// The browser band's share of a stand's adult foliage: each foliage layer of the last
/// profile stage, by the fraction of its band below the mouth ceiling.
fn band_share(sc: &cubarium_voxel_flora::SpeciesConfig, ceiling_m: f64) -> f64 {
    let Some(adult) = sc.profile.last() else {
        return 0.0;
    };
    let height = sc.crown_height_m[1].max(1e-9);
    let top = ceiling_m / height;
    adult
        .foliage_layers()
        .map(|(_, l)| {
            let [lo, hi] = l.band;
            let span = (hi - lo).max(1e-12);
            l.share * ((top - lo) / span).clamp(0.0, 1.0)
        })
        .sum()
}

/// R, S and F of `flora` (at its world's cell size `voxel_m`) under `fauna`.
pub fn rsf(flora: &FloraConfig, fauna: &FaunaConfig, voxel_m: f64, spec: &ScreenSpec) -> Rsf {
    let p = fauna.founder(Founder::Browser);
    let core = &p.core;
    let body = core.body_max;
    let yield_ = p.yield_by_food[Food::Foliage.index()].max(1e-12);
    let need = core.maintenance_per_s * body / yield_;
    let need_walk = (core.maintenance_per_s + p.motor_respiration_per_s) * body / yield_;
    let turnover = 1.0 / core.maintenance_per_s.max(1e-300);
    let v = p.cruise_body_lengths_per_s * p.adult_length_m;
    let swath = p.adult_width_m;
    let reach = p.mouth_reach_length_fraction * p.adult_length_m;
    let a_mouth = std::f64::consts::PI * (0.5 * p.adult_width_m + reach).powi(2);
    let ceiling = p.mouth_ceiling_fraction * p.adult_height_m;
    let cell = voxel_m * voxel_m;
    let mut rows = Vec::new();
    for s in Species::ALL {
        let sc = flora.species(s);
        if sc.trophic != Trophic::Photo || !(sc.founder_per_m2 > 0.0) || sc.founder_cap == 0 {
            continue;
        }
        let w = sc.wood_max;
        let foliage = sc.alpha * w;
        let band = band_share(sc, ceiling);
        let edible = (1.0 - sc.graze_refuge) * band * foliage;
        let income = sc.assimilation * foliage * spec.light_moisture;
        // Regrowth fills from the bottom layer up, so into the band — if the band holds
        // any of the crown at all.
        let regrowth = if band > 0.0 {
            (sc.foliage_rate * w).min(((income - sc.maintenance * w) / (1.0 + sc.build)).max(0.0))
        } else {
            0.0
        };
        let footprint = cell.max(std::f64::consts::PI * sc.crown_radius_m[1].powi(2));
        let n = (a_mouth / footprint).clamp(1.0, f64::from(sc.founder_colony.max(1)));
        let r = n * regrowth / need;
        let s_ = n * edible / (need * turnover);
        let f = (edible / footprint) * v * swath * spec.harvest / need_walk;
        rows.push(SpeciesRsf {
            species: s.name(),
            stands_in_reach: n,
            edible,
            regrowth_per_s: regrowth,
            r,
            s: s_,
            f,
        });
    }
    let max = |k: fn(&SpeciesRsf) -> f64| rows.iter().map(k).fold(0.0, f64::max);
    Rsf {
        r: max(|x| x.r),
        s: max(|x| x.s),
        f: max(|x| x.f),
        need_per_s: need,
        species: rows,
    }
}
