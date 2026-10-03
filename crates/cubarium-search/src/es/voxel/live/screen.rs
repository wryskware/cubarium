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

use cubarium_voxel::weather::{DayClock, DayMode, Sun};
use cubarium_voxel_fauna::{
    Activity, BirthMode, FaunaConfig, Food, Founder, FounderPhysiology, Founding, LifeConfig,
};
use cubarium_voxel_flora::{FloraConfig, Species, Trophic};
use serde::{Deserialize, Serialize};

/// **The night envelope** (proposal 1a, Astra A2; the search file's `[screen.night]`): an
/// optimistic adult that goes to sleep at reserve fraction `f0`, with its stomach at the
/// appetite floor `s0 · K` (the minimal gut), must outlast `margin` x its longest sleeping
/// bout. A chosen **design envelope** (uninterrupted sleep from 80 %), not
/// biological impossibility: an animal that wakes to feed could live outside it (Astra F7).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NightSpec {
    /// Reserve fraction at sleep onset.
    pub f0: f64,
    /// Endurance over the sleeping bout it must reach.
    pub margin: f64,
}

impl Default for NightSpec {
    fn default() -> NightSpec {
        NightSpec { f0: 0.8, margin: 1.2 }
    }
}

/// **Food access per food type** (proposal Part 3, Astra A5, A11, F3; `[screen.food]`):
/// each founded lineage's basal need against its foods' supply estimated at founding, in
/// the same (assimilated) units. **Soft**: these are estimates, not proven upper bounds
/// (plants and vines grow, stocks bridge, twilight produces), so a short supply runs and
/// pays `k_soft` x its log-distance; only a lineage with **no** supply path at all is
/// refused.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FoodSpec {
    /// Supply over need (and a ground grazer's reach over its walking need) below this pays
    /// the soft term.
    pub min_ratio: f64,
    /// **A founding measurement**, not a bound: latticevine faces per vine at founding.
    /// The founding roots each vine on one face (`Cover::seed_vine_founders`), and the
    /// founded terrarium measures exactly that (12 vines, 12 faces, seeds 1–8:
    /// `roster_tests::vine_faces_founded_on_the_terrarium`). Vines spread afterwards, so a
    /// supply estimated on it may be far below what the vines later give (Astra F3).
    pub vine_faces_founded: f64,
    /// **Predator balance** (a readout, never a refusal; Astra A6): the share of its prey's
    /// production allotted to each predator lineage.
    pub balance_share: f64,
}

impl Default for FoodSpec {
    fn default() -> FoodSpec {
        FoodSpec { min_ratio: 1.0, vine_faces_founded: 1.0, balance_share: 0.5 }
    }
}

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
    /// **R as a readout** (`r_readout = true`, roster.toml; Fable 2026-10-02): R is
    /// reported but neither refuses nor pays. S and F stay hard.
    pub r_readout: bool,
    /// The night-endurance screen (`[screen.night]`; off without the table).
    pub night: Option<NightSpec>,
    /// The food-access screens (`[screen.food]`; off without the table).
    pub food: Option<FoodSpec>,
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
            r_readout: false,
            night: None,
            food: None,
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
        let within = (self.r_readout || (self.r[0] / m..=self.r[1] * m).contains(&rsf.r))
            && (self.s[0] / m..=self.s[1] * m).contains(&rsf.s)
            && rsf.f >= self.f_min / m;
        if within { Verdict::Margin } else { Verdict::Refused }
    }

    /// Inside, in the margin, or refused, the roster screens included: any roster
    /// refusal refuses ([`RosterScreen::refusals`]).
    pub fn verdict_with(&self, rsf: &Rsf, roster: &RosterScreen) -> Verdict {
        if roster.refusals().is_empty() { self.verdict(rsf) } else { Verdict::Refused }
    }

    /// The soft terms a candidate that runs pays: R/S/F's and the food rows'.
    pub fn soft_with(&self, rsf: &Rsf, roster: &RosterScreen) -> f64 {
        self.soft(rsf) + roster.soft(self.k_soft)
    }

    /// A refused candidate's fitness with the roster screens: [`ScreenSpec::graded`] plus
    /// `k` x the roster's log-distance.
    pub fn graded_with(&self, rsf: &Rsf, roster: &RosterScreen, worst_run: f64) -> f64 {
        self.graded(rsf, worst_run) + self.k * roster.distance()
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
        if !spec.r_readout && !(spec.r[0]..=spec.r[1]).contains(&self.r) {
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
        let r = if spec.r_readout { 0.0 } else { band(self.r, spec.r[0], spec.r[1]) };
        r + band(self.s, spec.s[0], spec.s[1])
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

// --- The roster screens (`design/handoffs/roster-search-groundwork-2026-10-02.md`) ------

/// **Uninterrupted sleep from onset reserve fraction `f0`** (proposal 1a, Astra A2),
/// seconds: `(max(0, f0 − (1 − h)) × c + d) / (m × s)` — reserve per body `c`, basal upkeep
/// `m` per second, the sleep multiplier `s`, wake hunger `h` (the emergency latch wakes it
/// once the reserve falls to `1 − h` of its cap), and `d` the **digestible stomach** per
/// unit body at dusk ([`digestible_stomach_per_body`]). The stomach digests into the room
/// upkeep makes, so the reserve only starts to fall once it is empty: what it holds adds to
/// the night. Optimistic: an adult, no rider, no alert, no den travel.
pub fn night_endurance_s(
    reserve_cap: f64,
    upkeep: f64,
    sleep_upkeep: f64,
    wake_hunger: f64,
    f0: f64,
    stomach: f64,
) -> f64 {
    let usable = (f0 - (1.0 - wake_hunger)).max(0.0) * reserve_cap + stomach.max(0.0);
    let burn = upkeep * sleep_upkeep;
    if usable <= 0.0 {
        0.0
    } else if burn > 0.0 {
        usable / burn
    } else {
        f64::INFINITY
    }
}

/// **A stomach at the appetite floor, digested** (the minimal gut; the night guard), per
/// unit adult body: `s0 · k · y`, with `y` the best yield of its diet (optimistic, as the
/// rest of the guard). What a fed body carries into the night beside its reserve.
pub fn digestible_stomach_per_body(p: &FounderPhysiology) -> f64 {
    let y = p.diet.foods().map(|f| p.yield_by_food[f.index()]).fold(0.0, f64::max);
    p.gut.s0 * p.gut.stomach_k * y.max(0.0)
}

/// **The daily need `f`** (the minimal gut, `design/metabolism-meals-2026-10-02.md` §1):
/// the share of its adult body a lineage burns per ecological day awake and at rest,
/// `maintenance_per_s × ecological_day_s`.
pub fn daily_need(p: &FounderPhysiology, life: &LifeConfig) -> f64 {
    p.core.maintenance_per_s * life.ecological_day_s
}

/// **A body's basal demand over one day of the world's clock**, organic (assimilated)
/// units: the daily need `f` of its adult body, spread over the ecological day and paid
/// awake for `awake_s` and at the sleep multiplier for `asleep_s` (the design's
/// rhythm-aware `E` on the default 120-min cycle).
pub fn basal_demand_per_day(p: &FounderPhysiology, life: &LifeConfig, awake_s: f64, asleep_s: f64) -> f64 {
    let day = life.ecological_day_s.max(1e-12);
    daily_need(p, life) * p.core.body_max * (awake_s + p.sleep_upkeep * asleep_s) / day
}

/// The day's length, seconds, and its sunrise and sunset phases (`None` with the clock
/// off). Under `local` the screen takes a 24-hour day rising at 06:00 and setting at 18:00:
/// the date's own day is not known before a run.
fn day_of(day: &DayClock) -> Option<(f64, f64, f64)> {
    match day.mode {
        DayMode::Cycle => {
            let (rise, set) = day.cycle_rise_set();
            Some((day.length_min * 60.0, rise, set))
        }
        DayMode::Local => Some((86_400.0, 0.25, 0.75)),
        DayMode::Off => None,
    }
}

/// Seconds of one day the sun is up and down (all up with the clock off, on a day of
/// `length_min`).
fn sun_up_down_s(day: &DayClock) -> (f64, f64) {
    match day_of(day) {
        Some((len, rise, set)) => {
            let up = (set - rise).clamp(0.0, 1.0) * len;
            (up, len - up)
        }
        None => (day.length_min * 60.0, 0.0),
    }
}

/// **The sleeping bouts** of a lineage with activity `active` on `day`, seconds each, from
/// its own daylight lines ([`Activity::awake`]: 0.3 for diurnal and nocturnal, the twilight
/// band for crepuscular, which sleeps twice). Each crossing is found on a fine grid and
/// refined by bisection to machine precision: computed, not rounded. Empty with the clock
/// off or for a lineage always awake.
pub fn sleep_bouts_s(day: &DayClock, active: Activity, life: &LifeConfig) -> Vec<f64> {
    let Some((len, rise, set)) = day_of(day) else {
        return Vec::new();
    };
    let awake = |p: f64| active.awake(true, Sun::at(day, p.rem_euclid(1.0), rise, set).daylight, life);
    const N: usize = 14_400;
    let mut edges: Vec<(f64, bool)> = Vec::new(); // (phase, awake after it)
    for i in 0..N {
        let (a, b) = (i as f64 / N as f64, (i + 1) as f64 / N as f64);
        let (wa, wb) = (awake(a), awake(b));
        if wa != wb {
            let (mut lo, mut hi) = (a, b);
            for _ in 0..80 {
                let mid = 0.5 * (lo + hi);
                if awake(mid) == wa { lo = mid } else { hi = mid }
            }
            edges.push((0.5 * (lo + hi), wb));
        }
    }
    if edges.is_empty() {
        return if awake(0.0) { Vec::new() } else { vec![len] };
    }
    let mut bouts = Vec::new();
    for (k, &(p, now_awake)) in edges.iter().enumerate() {
        if now_awake {
            continue;
        }
        let next = edges[(k + 1) % edges.len()].0;
        bouts.push((next - p).rem_euclid(1.0) * len);
    }
    bouts
}

/// **Basal food over one day**, food units: `upkeep × body × (awake_s + sleep_upkeep ×
/// asleep_s) / yield` (Astra A5's units: what must be eaten, not what is assimilated).
pub fn basal_food_per_day(upkeep: f64, body: f64, yield_: f64, awake_s: f64, asleep_s: f64, sleep_upkeep: f64) -> f64 {
    upkeep * body * (awake_s + sleep_upkeep * asleep_s) / yield_.max(1e-12)
}

/// **A stand's nectar or fruit over one day**: the day rate through the light hours and
/// `night_scale` of it through the dark ones.
pub fn stand_product_per_day(rate_per_s: f64, day_s: f64, night_s: f64, night_scale: f64) -> f64 {
    rate_per_s * (day_s + night_scale * night_s)
}

/// One lineage's night: its longest sleeping bout and what it endures.
#[derive(Clone, Debug, Serialize)]
pub struct NightRow {
    pub lineage: &'static str,
    pub sleep_s: f64,
    pub endurance_s: f64,
    /// Endurance over `margin` x the bout.
    pub ratio: f64,
    pub passes: bool,
}

/// One food of a lineage's diet: its optimistic supply over a day, food units, and what
/// that supply is worth assimilated: the lineage's yield, limited for nectar and fruit by
/// the mineral each source's product carries ([`mineral_limited_yield`]).
#[derive(Clone, Debug, Serialize)]
pub struct FoodSupply {
    pub food: &'static str,
    pub per_day: f64,
    pub assimilable_per_day: f64,
}

/// **The yield a product's mineral can fund** (the minimal gut's digestion rule): `min(y,
/// mineral_fraction / n_tissue)`. Food whose mineral funds nothing passes undigested
/// (repair 3 item 1), so a mineral-free product feeds nothing.
pub fn mineral_limited_yield(yield_: f64, mineral_fraction: f64, n_tissue: f64) -> f64 {
    let funded = if n_tissue > 0.0 { mineral_fraction.max(0.0) / n_tissue } else { f64::INFINITY };
    yield_.min(funded).max(0.0)
}

/// One lineage's food access: its founders' basal need and its foods' supply over a day,
/// assimilated units, and (a ground grazer) its reach: a walker's harvest at full stock
/// over its walking need.
#[derive(Clone, Debug, Serialize)]
pub struct FoodRow {
    pub lineage: &'static str,
    pub bodies: f64,
    pub foods: Vec<FoodSupply>,
    pub need_per_day: f64,
    pub supply_per_day: f64,
    pub ratio: f64,
    pub reach: Option<f64>,
    /// The `min_ratio` it was judged at.
    pub min_ratio: f64,
    pub passes: bool,
}

impl FoodRow {
    /// No supply path at all (or no reach): the one hard food refusal.
    pub fn absent(&self) -> bool {
        !(self.supply_per_day > 0.0) || self.reach.is_some_and(|r| !(r > 0.0))
    }
}

/// **Predator balance** (a readout; Astra A6): `balance_share` of the prey list's
/// production, valued at the predator's carrion yield, over one predator's basal day.
#[derive(Clone, Debug, Serialize)]
pub struct BalanceRow {
    pub predator: &'static str,
    pub predators: usize,
    pub prey_per_day: f64,
    pub need_per_day: f64,
    pub supported: f64,
}

/// The roster screens of one candidate.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RosterScreen {
    pub night: Vec<NightRow>,
    pub food: Vec<FoodRow>,
    pub balance: Vec<BalanceRow>,
}

impl RosterScreen {
    /// Each refusal, named: `night: …` (the design envelope) or `food: …` (a lineage with
    /// no supply path at all). Short food is soft ([`RosterScreen::soft`]); the balance
    /// never refuses.
    pub fn refusals(&self) -> Vec<String> {
        let mut out = Vec::new();
        for n in self.night.iter().filter(|n| !n.passes) {
            out.push(format!(
                "night: {} sleeps {:.0} s and endures {:.0} s ({:.2}x the design envelope's need)",
                n.lineage, n.sleep_s, n.endurance_s, n.ratio
            ));
        }
        for f in self.food.iter().filter(|f| f.absent()) {
            let foods: Vec<&str> = f.foods.iter().map(|x| x.food).collect();
            let reach = f.reach.map(|r| format!(", reach {r:.2}x")).unwrap_or_default();
            out.push(format!(
                "food: {} on {} supplies {:.2}x its {} founders' basal need{reach}",
                f.lineage,
                foods.join("+"),
                f.ratio,
                f.bodies
            ));
        }
        out
    }

    /// The log-distance of the refusals from passing, summed (0 when all pass).
    pub fn distance(&self) -> f64 {
        let ln = |x: f64| x.max(1e-12).ln();
        let mut d = 0.0;
        for n in self.night.iter().filter(|n| !n.passes) {
            d += -ln(n.ratio);
        }
        for f in self.food.iter().filter(|f| f.absent()) {
            d += (-ln(f.ratio)).max(0.0) + f.reach.map_or(0.0, |r| (-ln(r)).max(0.0));
        }
        d
    }

    /// **The soft food term** of a candidate that runs: `k` x the log-distance of every
    /// food row short of `min_ratio` (supply, and a grazer's reach) but not absent.
    pub fn soft(&self, k: f64) -> f64 {
        let ln = |x: f64| x.max(1e-12).ln();
        self.food
            .iter()
            .filter(|f| !f.passes && !f.absent())
            .map(|f| {
                k * ((ln(f.min_ratio) - ln(f.ratio)).max(0.0)
                    + f.reach.map_or(0.0, |r| (ln(f.min_ratio) - ln(r)).max(0.0)))
            })
            .sum()
    }
}

/// Bodies a lineage founds as a **consumer** (the fewest that eat: an egg bank's clutch
/// one future adult) and as a **producer** of carrion (the most there can be: every egg).
fn bodies_at_founding(p: &FounderPhysiology, as_producer: bool) -> f64 {
    let n = p.founding_count as f64;
    match p.founding {
        Founding::Bodies => n,
        Founding::Pack => n * p.pack.map_or(1, |k| k.pack_size) as f64,
        Founding::Clutches if as_producer => n * f64::from(p.core.reproduction.clutch_size.max(1)),
        Founding::Clutches => n,
    }
}

/// An adult's optimistic **carrion production**, organic per second: its body and full
/// reserve once per lifespan, and each young it could raise per breeding interval as one
/// more such adult ("every newborn an eventual full adult", Astra A6).
fn carrion_per_adult_s(p: &FounderPhysiology) -> f64 {
    let c = &p.core;
    let carcass = c.body_max * (1.0 + c.reserve_cap);
    let death = if c.lifespan_median_s > 0.0 { 1.0 / c.lifespan_median_s } else { 0.0 };
    let r = &c.reproduction;
    let (young, interval) = match (p.pack, r.mode) {
        (Some(k), _) => (f64::from(k.litter_size_max), k.litter_interval_s.max(r.birth_interval_s) + r.gestation_s),
        (None, BirthMode::Eggs) => (f64::from(r.clutch_size.max(1)), r.birth_interval_s + r.incubation_s),
        (None, BirthMode::Gestation) => (1.0, r.birth_interval_s + r.gestation_s),
    };
    carcass * (death + if interval > 0.0 { young / interval } else { 0.0 })
}

/// The share of a stand's adult foliage within heights `[floor_m, ceiling_m]` above its
/// support.
fn band_between(sc: &cubarium_voxel_flora::SpeciesConfig, floor_m: f64, ceiling_m: f64) -> f64 {
    let Some(adult) = sc.profile.last() else {
        return 0.0;
    };
    let height = sc.crown_height_m[1].max(1e-9);
    let (bot, top) = (floor_m / height, ceiling_m / height);
    adult
        .foliage_layers()
        .map(|(_, l)| {
            let [lo, hi] = l.band;
            let span = (hi - lo).max(1e-12);
            l.share * ((top.min(hi) - bot.max(lo)) / span).clamp(0.0, 1.0)
        })
        .sum()
}

/// A food whose yield is the lineage's own: `(per_day, per_day x y)`.
fn with_yield(y: f64, per_day: f64) -> (f64, f64) {
    (per_day, per_day * y)
}

/// **The roster screens** of `flora` (at the world's cell size) and `fauna` on `world`:
/// the night envelope per founded lineage, food access per lineage over its diet (soft
/// unless absent), and the predator balance readout. Founded means `founding_count` above
/// zero.
///
/// Food supply is **estimated at founding**, generously but not as a proven upper bound
/// (Astra F3): stands at founding are `min(founder_cap, founder_per_m2
/// × floor area)` (the whole floor counted as habitat), harvested continuously, with no
/// competition between lineages, no stock caps, no fruit drop, no plant reserve limit;
/// need is basal only (the gut's daily need `f`, [`basal_demand_per_day`]; no motion,
/// growth, breeding or rider take). Short supply is a soft term; only
/// a lineage with no supply path is refused. One passed may still starve.
/// - foliage: each photosynthetic species' regrowth (R's rule) where its adult crown meets
///   the lineage's mouth band;
/// - cap tissue: a saprotroph's cap regrowth cap where it meets the band;
/// - litter: every stand's leaf fall at full foliage (endowments, such as the starter
///   litter, are not income);
/// - film: the logistic film's best net production per m², light-hours weighted, over the
///   floor; and its reach, a walker's harvest at capacity over its walking need;
/// - nectar, fruit: each stand's day rate through the light hours and
///   each species' `night_bloom_scale` of it through the dark (the night budget's cap;
///   the budget itself may give less), plus `vine_faces_founded` latticevine faces per
///   vine at the vine's long-run production per face;
/// - carrion: a hunter's prey list (a scavenger's: every other founded lineage), each prey
///   adult's [`carrion_per_adult_s`].
pub fn roster(flora: &FloraConfig, fauna: &FaunaConfig, world: &cubarium_voxel::Config, spec: &ScreenSpec) -> RosterScreen {
    let mut out = RosterScreen::default();
    let founded: Vec<Founder> = Founder::ALL
        .into_iter()
        .filter(|f| fauna.founder(*f).founding_count > 0)
        .collect();
    let len_s = day_of(&world.day).map_or(world.day.length_min * 60.0, |d| d.0);
    let phases = |p: &FounderPhysiology| -> (Vec<f64>, f64, f64) {
        let bouts = sleep_bouts_s(&world.day, p.active, &fauna.life);
        let asleep: f64 = bouts.iter().sum();
        (bouts, len_s - asleep, asleep)
    };
    if let Some(night) = spec.night {
        for &f in &founded {
            let p = fauna.founder(f);
            let (bouts, _, _) = phases(p);
            let sleep = bouts.iter().copied().fold(0.0, f64::max);
            let endurance = night_endurance_s(
                p.core.reserve_cap,
                p.core.maintenance_per_s,
                p.sleep_upkeep,
                p.wake_hunger,
                night.f0,
                digestible_stomach_per_body(p),
            );
            let need = night.margin * sleep;
            let ratio = if need > 0.0 { endurance / need } else { f64::INFINITY };
            out.night.push(NightRow { lineage: f.name(), sleep_s: sleep, endurance_s: endurance, ratio, passes: endurance >= need });
        }
    }
    let Some(food) = spec.food else {
        return out;
    };
    let area = f64::from(world.width) * f64::from(world.depth) * world.voxel_m * world.voxel_m;
    let stands = |sc: &cubarium_voxel_flora::SpeciesConfig| -> f64 {
        if sc.founder_per_m2 > 0.0 && sc.founder_cap > 0 {
            (sc.founder_per_m2 * area).min(f64::from(sc.founder_cap))
        } else {
            0.0
        }
    };
    let (up_s, down_s) = sun_up_down_s(&world.day);
    let vine = &flora.latticevine;
    let vines = f64::from(vine.founders).max((vine.founders_per_m2 * area).round());
    // A face's long-run production is the vine's own knob (per face per second).
    let vine_per_day = |per_face_s: f64| vines * food.vine_faces_founded * per_face_s * len_s;
    let carrion_of = |eater: Founder| -> f64 {
        let p = fauna.founder(eater);
        founded
            .iter()
            .filter(|&&q| match p.hunter {
                Some(h) => h.prey[q.index()],
                None => q != eater,
            })
            .map(|&q| {
                let qp = fauna.founder(q);
                bodies_at_founding(qp, true) * carrion_per_adult_s(qp) * len_s
            })
            .sum()
    };
    for &f in &founded {
        let p = fauna.founder(f);
        let (_, awake_s, asleep_s) = phases(p);
        let bodies = bodies_at_founding(p, false);
        // Demand is the gut's daily need f (the design's E over the world's day).
        let need = bodies * basal_demand_per_day(p, &fauna.life, awake_s, asleep_s);
        let floor_m = p.mouth_floor_fraction * p.adult_height_m;
        let ceiling_m = p.mouth_ceiling_fraction * p.adult_height_m;
        let mut foods = Vec::new();
        let mut reach = None;
        for fd in p.diet.foods() {
            let y = p.yield_by_food[fd.index()];
            // Nectar and fruit carry their sources' mineral fractions (stands' own, the
            // vine's `mineral_fraction` since repair 3): the yield they fund per source.
            let y_of = |fraction: f64| mineral_limited_yield(y, fraction, p.core.n_tissue);
            let (per_day, assimilable) = match fd {
                Food::Foliage => with_yield(y, Species::ALL
                    .iter()
                    .map(|s| flora.species(*s))
                    .filter(|sc| sc.trophic == Trophic::Photo && band_between(sc, floor_m, ceiling_m) > 0.0)
                    .map(|sc| {
                        let w = sc.wood_max;
                        let income = sc.assimilation * sc.alpha * w * spec.light_moisture;
                        let g = (sc.foliage_rate * w).min(((income - sc.maintenance * w) / (1.0 + sc.build)).max(0.0));
                        stands(sc) * g * len_s
                    })
                    .sum()),
                Food::CapTissue => with_yield(y, Species::ALL
                    .iter()
                    .map(|s| flora.species(*s))
                    .filter(|sc| sc.trophic == Trophic::Saprotroph && band_between(sc, floor_m, ceiling_m) > 0.0)
                    .map(|sc| stands(sc) * sc.foliage_rate * sc.wood_max * len_s)
                    .sum()),
                Food::Litter => with_yield(y, Species::ALL
                    .iter()
                    .map(|s| flora.species(*s))
                    .map(|sc| stands(sc) * sc.senescence * sc.alpha * sc.wood_max * len_s)
                    .sum()),
                Food::Film => {
                    let r = flora.film_growth_per_s * up_s / len_s.max(1e-12);
                    let m = flora.film_maintenance_per_s;
                    let k = flora.film_capacity_per_m2;
                    let net = if r > m { (r - m).powi(2) * k / (4.0 * r) } else { 0.0 };
                    let y = p.yield_by_food[Food::Film.index()];
                    let v = p.cruise_body_lengths_per_s * p.adult_length_m;
                    let walk = (p.core.maintenance_per_s + p.motor_respiration_per_s) * p.core.body_max / y.max(1e-12);
                    let net_live = if flora.film_growth_per_s > 0.0 { k } else { 0.0 };
                    reach = Some(net_live * v * p.adult_width_m * spec.harvest / walk.max(1e-300));
                    let d = area * net * len_s;
                    (d, d * y)
                }
                Food::Nectar => {
                    let (mut raw, mut got) = (vine_per_day(vine.nectar_per_face_s), 0.0);
                    got += raw * y_of(vine.mineral_fraction);
                    for sc in Species::ALL.iter().map(|s| flora.species(*s)) {
                        let d = stands(sc) * stand_product_per_day(sc.nectar_per_s, up_s, down_s, sc.night_bloom_scale);
                        raw += d;
                        got += d * y_of(sc.nectar_mineral_fraction);
                    }
                    (raw, got)
                }
                Food::Fruit => {
                    let (mut raw, mut got) = (vine_per_day(vine.fruit_per_face_s), 0.0);
                    got += raw * y_of(vine.mineral_fraction);
                    for sc in Species::ALL.iter().map(|s| flora.species(*s)) {
                        let d = stands(sc) * stand_product_per_day(sc.fruit_per_s, up_s, down_s, sc.night_bloom_scale);
                        raw += d;
                        got += d * y_of(sc.fruit_mineral_fraction);
                    }
                    (raw, got)
                }
                Food::Carrion => {
                    let d = carrion_of(f);
                    (d, d * y)
                }
            };
            foods.push(FoodSupply { food: fd.name(), per_day, assimilable_per_day: assimilable });
        }
        let supply: f64 = foods.iter().map(|s| s.assimilable_per_day).sum();
        let ratio = if need > 0.0 { supply / need } else { f64::INFINITY };
        let passes = ratio >= food.min_ratio && reach.is_none_or(|r| r >= food.min_ratio);
        out.food.push(FoodRow {
            lineage: f.name(),
            bodies,
            foods,
            need_per_day: need,
            supply_per_day: supply,
            ratio,
            reach,
            min_ratio: food.min_ratio,
            passes,
        });
        if p.hunter.is_some() {
            let y = p.yield_by_food[Food::Carrion.index()];
            let one = basal_demand_per_day(p, &fauna.life, awake_s, asleep_s);
            let prey = carrion_of(f);
            out.balance.push(BalanceRow {
                predator: f.name(),
                predators: bodies.round() as usize,
                prey_per_day: prey,
                need_per_day: one,
                supported: if one > 0.0 { food.balance_share * prey * y / one } else { f64::INFINITY },
            });
        }
    }
    out
}
