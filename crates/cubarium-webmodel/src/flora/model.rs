//! **The producer half as stocks and flows**: per habitat zone, every species' occupied
//! faces (Levins/Tilman space with the sim's own takeover rules), each held by cohorts of
//! stands stepped under the sim's own growth law ([`super::stand`]), the seed banks, the
//! site mineral per occupant, and the dead pools. See `design/webmodel/flora.md` for each
//! law, its `file:line`, and what its aggregation loses.

use cubarium_voxel_flora::{FloraConfig, Species, Stand, Trophic};

use super::census::Face;
use super::climate::{Clock, DayModel};
use super::kernel::{Kernel, Rng, RunnerBox};
use super::stand::{self, Death, StandState, light_response, monod, package_of};
use super::zones::{Reading, Zones};
use crate::exchange::{Returns, Supply, Take};

pub const NS: usize = Species::COUNT;
/// The vacant plant place's slot in the mineral pools.
pub const VACANT: usize = NS;
const GLOW: usize = 5;

/// One cohort: `n` stands of one species in one zone sharing one state.
#[derive(Clone, Copy, Debug)]
pub struct Cohort {
    pub n: f64,
    pub st: StandState,
    /// Under a taller crown (a receiver's light class).
    pub covered: bool,
    /// World seconds the cohort was opened (a nursery takes newborns while young).
    pub opened_s: f64,
}

/// One zone's state.
#[derive(Clone, Debug)]
pub struct ZoneState {
    /// Per species ([`Species::ALL`] order); the glowcap's are the fungal place's.
    pub cohorts: Vec<Vec<Cohort>>,
    /// Whole seeds banked, per species.
    pub bank: [f64; NS],
    /// Site mineral on the faces each plant occupant holds (`VACANT` last), totals.
    pub pool: [f64; NS + 1],
    pub litter: f64,
    pub dead_wood: f64,
    /// Woody crown cover over the ground layer (share of the zone) and its transmission.
    pub cover: f64,
    pub transmit: f64,
}

impl ZoneState {
    pub fn count(&self, s: usize) -> f64 {
        self.cohorts[s].iter().map(|c| c.n).sum()
    }
    fn plants(&self) -> f64 {
        (0..NS).filter(|&s| s != GLOW).map(|s| self.count(s)).sum()
    }
}

/// How moisture enters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Weather {
    /// Each zone at its time mean (Wrysk's route 2 "frozen water").
    Frozen,
    /// The world's recorded weather states (census snapshots), one drawn every `period_s`.
    Replay { period_s: f64, seed: u64 },
}

/// The model's own parameters: numerics, the fitted spatial factors, and scenario levers
/// (none of them a sim knob).
#[derive(Clone, Debug)]
pub struct Params {
    pub dt: f64,
    pub weather: Weather,
    /// Rebin and re-split cohorts every this many seconds.
    pub rebin_s: f64,
    /// A nursery takes newborns this long.
    pub nursery_s: f64,
    /// **Fitted (spatial)**: seed density on a species' takeable faces over its zone mean
    /// (donors clump near their own kind). 1 is well mixed.
    pub psi: [f64; NS],
    /// **Fitted (spatial)**: the same for a runner's box.
    pub psi_runner: [f64; NS],
    /// **Scenario lever, not a sim knob**: an extra hazard on juveniles (`W < donor_min`)
    /// per species, per second (a browser of seedlings).
    pub juvenile_browse: [f64; NS],
    /// **Fitted (spatial)**: the log-sd of site mineral over the faces one occupant holds
    /// in a zone ([`Species::ALL`] order, `VACANT` last). Who comes up where reads it: a
    /// newborn pays its upkeep only on faces rich enough (the canopy gate), so it takes
    /// the richer faces first.
    pub sigma: [f64; NS + 1],
    /// **Fitted (spatial)**: each species' relative light `[a, k]` per stage `[juvenile,
    /// adult]` against the zone's crown area index (adult crowns over area): a stand's
    /// light is `sky · max(a − k · CAI, 0.02)` ([`super::fit::light`]).
    pub light_k: [[[f64; 2]; 2]; NS],
}

impl Default for Params {
    fn default() -> Params {
        Params {
            dt: 10.0,
            weather: Weather::Replay { period_s: 3600.0, seed: 1 },
            rebin_s: 3600.0,
            nursery_s: 600.0,
            psi: [1.0; NS],
            psi_runner: [1.0; NS],
            juvenile_browse: [0.0; NS],
            // Census means of m250/m370 and m1570, seeds 5 and 6 (`flora.md`, fitted rates).
            sigma: [0.15, 0.08, 0.38, 0.27, 0.42, 0.0, 0.35, 0.23, 0.1, 0.15],
            light_k: [[[1.0, 0.0]; 2]; NS],
        }
    }
}

/// Event counters since the start (stands, or organic units for the flux lines).
#[derive(Clone, Debug, Default)]
pub struct Counters {
    pub births_seed: [f64; NS],
    pub births_runner: [f64; NS],
    pub starved: [f64; NS],
    pub drought: [f64; NS],
    pub overtopped: [f64; NS],
    pub displaced: [f64; NS],
    pub browsed: [f64; NS],
    /// `[cover][newcomer]`.
    pub pairs: [[f64; NS]; NS],
    pub seeds_landed: [f64; NS],
    pub seeds_died: [f64; NS],
    pub built: [f64; NS],
    pub shed: [f64; NS],
    pub fixed: f64,
    /// Organic matter respired (stands, propagule construction, a fungus's unkept
    /// substrate) and the seep that came in: the half's boundary flows with `fixed`,
    /// `decomposed` and the animals' take and returns.
    pub respired: f64,
    pub seep: f64,
    pub returned: f64,
    /// The mineral the animals' bites carried out of the stands (`n_tissue` per unit).
    pub taken_mineral: f64,
    pub substrate: f64,
    pub decomposed: f64,
    pub taken: Supply,
}

/// The whole half.
pub struct Model {
    pub config: FloraConfig,
    pub day: DayModel,
    pub zones: Zones,
    pub kernels: Vec<Kernel>,
    pub runner_box: Vec<RunnerBox>,
    pub z: Vec<ZoneState>,
    pub t_s: f64,
    pub p: Params,
    pub c: Counters,
    weather_ix: usize,
    next_weather_s: f64,
    next_rebin_s: f64,
    rng: Rng,
    frozen: Vec<[Reading; NS]>,
    /// The carrion pool, the film stock, its capacity and its light.
    pub carrion: f64,
    pub film: f64,
    pub film_capacity: f64,
    pub film_light: f64,
    /// The animals' take and returns over the next step (open loop: clamped).
    pub take: Take,
    pub returns: Returns,
}

fn is_photo(config: &FloraConfig, s: usize) -> bool {
    config.species(Species::ALL[s]).trophic == Trophic::Photo
}

/// The newborn's break-even cover (`step::break_even_cover`, `step.rs:3760`).
fn break_even(sc: &cubarium_voxel_flora::SpeciesConfig, w: f64, p: f64, l_eff: f64, mu: f64, mo: f64) -> f64 {
    let upkeep = sc.maintenance * w + (1.0 + sc.build) * sc.senescence * p;
    if upkeep <= 0.0 {
        return f64::INFINITY;
    }
    sc.assimilation * l_eff * p * mu * mo / upkeep
}

/// `erfc`, Numerical Recipes' Chebyshev form (fractional error under 1.2e-7).
fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let r = t * (-z * z - 1.265_512_23
        + t * (1.000_023_68
            + t * (0.374_091_96
                + t * (0.096_784_18 + t * (-0.186_288_06 + t * (0.278_868_07 + t * (-1.135_203_98 + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
        .exp();
    if x >= 0.0 { r } else { 2.0 - r }
}

fn phi(x: f64) -> f64 {
    0.5 * erfc(-x / std::f64::consts::SQRT_2)
}

/// The share of an occupant's faces with site mineral above `n_star`, and their mean,
/// when the faces' mineral is lognormal with mean `mean` and log-sd `sigma`.
pub fn above(mean: f64, sigma: f64, n_star: f64) -> (f64, f64) {
    if !(n_star > 0.0) {
        return (1.0, mean);
    }
    if !n_star.is_finite() || !(mean > 0.0) {
        return (0.0, 0.0);
    }
    if sigma < 1e-9 {
        return if mean >= n_star { (1.0, mean) } else { (0.0, 0.0) };
    }
    let mu = mean.ln() - 0.5 * sigma * sigma;
    let z = (n_star.ln() - mu) / sigma;
    let share = 1.0 - phi(z);
    let part = mean * (1.0 - phi(z - sigma));
    if share > 1e-12 { (share, part / share) } else { (0.0, 0.0) }
}

/// The least site mineral on which a newborn of `sc` pays its upkeep (the canopy gate's
/// break-even at the margin, `step::canopy_pays`, `step.rs:3787`) at `l_eff` and `mu`.
pub fn n_star(sc: &cubarium_voxel_flora::SpeciesConfig, l_eff: f64, mu: f64) -> f64 {
    if sc.trophic != Trophic::Photo || !(sc.establish_light_margin > 0.0) {
        return 0.0;
    }
    let nb = StandState::newborn(sc, package_of(sc));
    let upkeep = sc.maintenance * nb.wood + (1.0 + sc.build) * sc.senescence * nb.foliage;
    let gain = sc.assimilation * l_eff * nb.foliage * mu;
    if !(gain > 0.0) {
        return f64::INFINITY;
    }
    let y = sc.establish_light_margin * upkeep / gain;
    if y >= 1.0 { f64::INFINITY } else { y * sc.nutrient_half / (1.0 - y) }
}

/// Whether a seedling of `s` may come up through `cover` (`seeds::may_overtop`, `seeds.rs:90`).
fn may_overtop(config: &FloraConfig, s: Species, cover: Species) -> bool {
    let sc = config.species(s);
    config.species(cover).ground_cover && sc.overtop_per_check > 0.0 && (sc.overtop_covers.is_empty() || sc.overtop_covers.contains(&cover))
}

/// Whether a runner of `s` may root through `cover` (`seeds::runner_may_take`, `seeds.rs:1063`).
fn runner_may_take(config: &FloraConfig, s: Species, cover: Species) -> bool {
    let sc = config.species(s);
    sc.runners_overtop && cover != s && config.species(cover).ground_cover && (sc.overtop_covers.is_empty() || sc.overtop_covers.contains(&cover))
}

/// Edible foliage on a stand of `wood`/`foliage`: the sim's own layers above each layer's
/// grazing floor (`StandLayer::edible`), on a hand-built stand.
pub fn edible(config: &FloraConfig, sp: Species, wood: f64, foliage: f64) -> f64 {
    let mut s: Stand = cubarium_voxel_flora::study::start_stand(config, sp, cubarium_voxel_flora::study::Start::Newborn);
    s.wood = wood.max(0.0);
    s.foliage = foliage.max(0.0);
    s.bin_layers(config);
    cubarium_voxel_flora::layers_of(config, &s, config.voxel_m).iter().map(|l| l.edible()).sum()
}

/// A crown's area in faces and its transmission (`step::light_per_stand`'s attenuation
/// `exp(−k (1 − porosity) stock / area)`, `lib.rs:2857`), over its foliage layers.
pub fn crown_of(config: &FloraConfig, sp: Species, wood: f64, foliage: f64) -> (f64, f64) {
    let mut s: Stand = cubarium_voxel_flora::study::start_stand(config, sp, cubarium_voxel_flora::study::Start::Newborn);
    s.wood = wood.max(0.0);
    s.foliage = foliage.max(0.0);
    s.bin_layers(config);
    let mut area = 0.0f64;
    let mut t = 1.0;
    for l in cubarium_voxel_flora::layers_of(config, &s, config.voxel_m) {
        if !l.kind.bears_foliage() {
            continue;
        }
        area = area.max(std::f64::consts::PI * l.radius_v * l.radius_v);
        t *= (-config.shade_k_per_m2 * (1.0 - l.porosity) * l.stock / l.area_m2).exp();
    }
    (area.max(1.0), t)
}

fn merge(a: &mut Cohort, b: &Cohort) {
    let n = a.n + b.n;
    if n <= 0.0 {
        return;
    }
    let (wa, wb) = (a.n / n, b.n / n);
    let m = |x: f64, y: f64| wa * x + wb * y;
    let s = &mut a.st;
    let t = &b.st;
    s.wood = m(s.wood, t.wood);
    s.foliage = m(s.foliage, t.foliage);
    s.reserve = m(s.reserve, t.reserve);
    s.parcel = m(s.parcel, t.parcel);
    s.aeration_stress = m(s.aeration_stress, t.aeration_stress);
    s.dry_s = m(s.dry_s, t.dry_s);
    s.nectar = m(s.nectar, t.nectar);
    s.fruit = m(s.fruit, t.fruit);
    s.fruit_age_s = m(s.fruit_age_s, t.fruit_age_s);
    s.fruit_rest_s = m(s.fruit_rest_s, t.fruit_rest_s);
    s.support_s = m(s.support_s, t.support_s);
    s.cured = if wa >= wb { s.cured } else { t.cured };
    a.opened_s = a.opened_s.min(b.opened_s);
    a.n = n;
}

impl Model {
    /// A model of the world the censuses were taken of, starting from `start` at `start_minute`.
    pub fn new(
        config: FloraConfig,
        day: DayModel,
        zones: Zones,
        kernels: Vec<Kernel>,
        runner_box: Vec<RunnerBox>,
        start: &[Face],
        start_minute: f64,
        p: Params,
    ) -> Model {
        let nz = zones.zones.len();
        let mut z: Vec<ZoneState> = (0..nz)
            .map(|_| ZoneState {
                cohorts: vec![Vec::new(); NS],
                bank: [0.0; NS],
                pool: [0.0; NS + 1],
                litter: 0.0,
                dead_wood: 0.0,
                cover: 0.0,
                transmit: 1.0,
            })
            .collect();
        let t0 = start_minute * 60.0;
        for f in start {
            let zi = zones.of_face[&(f.x, f.y, f.z)];
            let zs = &mut z[zi];
            zs.litter += f.litter;
            zs.dead_wood += f.dead_wood;
            for (k, &n) in f.seeds.iter().enumerate() {
                zs.bank[k] += f64::from(n);
            }
            let occ = f.plant.map_or(VACANT, |(s, _)| s.index());
            zs.pool[occ] += f.mineral;
            if let Some((sp, r)) = f.plant {
                let st = StandState {
                    wood: r.wood,
                    foliage: r.foliage,
                    reserve: r.reserve,
                    aeration_stress: r.aeration_stress,
                    cured: r.cured,
                    ..StandState::default()
                };
                zs.cohorts[sp.index()].push(Cohort { n: 1.0, st, covered: f.canopy < 0.9, opened_s: t0 });
            }
            if let Some(r) = f.fungus {
                let st = StandState { wood: r.wood, foliage: r.foliage, reserve: r.reserve, ..StandState::default() };
                zs.cohorts[GLOW].push(Cohort { n: 1.0, st, covered: false, opened_s: t0 });
            }
        }
        let frozen = zones.zones.iter().map(|zn| zn.frozen()).collect();
        let mut m = Model {
            config,
            day,
            zones,
            kernels,
            runner_box,
            z,
            t_s: t0,
            next_weather_s: t0,
            next_rebin_s: t0,
            rng: Rng(match p.weather {
                Weather::Replay { seed, .. } => seed,
                Weather::Frozen => 0,
            }),
            p,
            c: Counters::default(),
            weather_ix: 0,
            frozen,
            carrion: start.iter().map(|_| 0.0).sum(),
            film: start.iter().map(|f| f.film).sum(),
            film_capacity: 0.0,
            film_light: 0.0,
            take: Take::default(),
            returns: Returns::default(),
        };
        // The film's eligible faces: shallow standing water now (`film.rs`), at its
        // capacity per square metre; its light the sky under the water's extinction.
        let cell = m.config.voxel_m * m.config.voxel_m;
        let (mut cap, mut light) = (0.0, 0.0);
        for f in start {
            if f.depth_m >= m.config.film_wet_min_depth_m && f.depth_m <= m.config.film_max_depth_m {
                cap += m.config.film_capacity_per_m2 * cell;
                light += f.sky * (-m.config.film_extinction_per_m * f.depth_m).exp() * m.config.film_capacity_per_m2 * cell;
            }
        }
        m.film_capacity = cap;
        m.film_light = if cap > 0.0 { light / cap } else { 0.0 };
        m.rebin();
        m.canopy();
        m
    }

    fn reading(&self, zi: usize, s: usize) -> Reading {
        match self.p.weather {
            Weather::Frozen => self.frozen[zi][s],
            Weather::Replay { .. } => self.zones.zones[zi].weather[self.weather_ix][s],
        }
    }

    /// The canopy over each zone's ground layer from its adult non-cover stands.
    fn canopy(&mut self) {
        for zi in 0..self.z.len() {
            let a = self.zones.zones[zi].area.max(1.0);
            let (mut sum_area, mut sum_t) = (0.0, 0.0);
            for s in 0..NS {
                let sp = Species::ALL[s];
                let sc = self.config.species(sp);
                if !is_photo(&self.config, s) || sc.ground_cover {
                    continue;
                }
                for c in &self.z[zi].cohorts[s] {
                    if c.st.wood < sc.donor_min {
                        continue;
                    }
                    let (ar, t) = crown_of(&self.config, sp, c.st.wood, c.st.foliage);
                    sum_area += c.n * ar;
                    sum_t += c.n * ar * t;
                }
            }
            let zs = &mut self.z[zi];
            zs.cover = sum_area / a;
            zs.transmit = if sum_area > 0.0 { sum_t / sum_area } else { 1.0 };
        }
    }

    /// Merge cohorts by (light class, log-wood bin), and move receivers between the light
    /// classes as the crown cover moved.
    fn rebin(&mut self) {
        const BINS: f64 = 16.0;
        let config = self.config.clone();
        for zs in &mut self.z {
            let cover = zs.cover;
            for s in 0..NS {
                let sc = config.species(Species::ALL[s]);
                let receiver = |c: &Cohort| sc.ground_cover || c.st.wood < sc.donor_min;
                // Re-split the receivers to the zone's cover share.
                let (open, covered): (f64, f64) = zs.cohorts[s].iter().filter(|c| receiver(c)).fold((0.0, 0.0), |(o, k), c| if c.covered { (o, k + c.n) } else { (o + c.n, k) });
                let tot = open + covered;
                if false && tot > 0.0 && is_photo(&config, s) {
                    let want = cover * tot;
                    let mut extra: Vec<Cohort> = Vec::new();
                    if want > covered && open > 0.0 {
                        let f = ((want - covered) / open).min(1.0);
                        for c in zs.cohorts[s].iter_mut().filter(|c| !c.covered && (sc.ground_cover || c.st.wood < sc.donor_min)) {
                            let mut d = *c;
                            d.n = c.n * f;
                            d.covered = true;
                            c.n -= d.n;
                            extra.push(d);
                        }
                    } else if want < covered && covered > 0.0 {
                        let f = ((covered - want) / covered).min(1.0);
                        for c in zs.cohorts[s].iter_mut().filter(|c| c.covered && (sc.ground_cover || c.st.wood < sc.donor_min)) {
                            let mut d = *c;
                            d.n = c.n * f;
                            d.covered = false;
                            c.n -= d.n;
                            extra.push(d);
                        }
                    }
                    zs.cohorts[s].extend(extra);
                }
                let lo = sc.alive_min.max(1e-6).ln();
                let hi = sc.wood_max.max(sc.alive_min * 1.0001).ln();
                let mut out: Vec<Cohort> = Vec::new();
                let mut keys: Vec<(bool, i64, bool)> = Vec::new();
                for c in zs.cohorts[s].drain(..) {
                    if c.n <= 1e-9 {
                        continue;
                    }
                    let b = (((c.st.wood.max(1e-9).ln() - lo) / (hi - lo)) * BINS).floor().clamp(0.0, BINS) as i64;
                    let key = (c.covered, b, c.st.cured);
                    match keys.iter().position(|k| *k == key) {
                        Some(i) => merge(&mut out[i], &c),
                        None => {
                            keys.push(key);
                            out.push(c);
                        }
                    }
                }
                zs.cohorts[s] = out;
            }
        }
    }

    /// Add `n` newborns of species `s` in zone `zi` built from `organic` each, split
    /// between the light classes by `covered_share`.
    fn add_newborns(&mut self, zi: usize, s: usize, n: f64, organic: f64, covered_share: f64, support_s: f64) {
        if n <= 0.0 {
            return;
        }
        let sc = self.config.species(Species::ALL[s]);
        let mut st = StandState::newborn(sc, organic);
        st.support_s = support_s;
        let t = self.t_s;
        let nursery = self.p.nursery_s;
        for (covered, share) in [(false, 1.0 - covered_share), (true, covered_share)] {
            let k = n * share;
            if k <= 0.0 {
                continue;
            }
            let fresh = Cohort { n: k, st, covered, opened_s: t };
            match self.z[zi].cohorts[s].iter_mut().rev().find(|c| c.covered == covered && t - c.opened_s < nursery && c.st.wood < 2.0 * sc.alive_min * f64::from(sc.seed_mass.max(1))) {
                Some(c) => merge(c, &fresh),
                None => self.z[zi].cohorts[s].push(fresh),
            }
        }
    }

    /// Remove `n` stands of cover `c` (proportionally over its cohorts) from zone `zi`,
    /// their material to the dead pools; returns the site mineral the faces carry.
    fn remove_stands(&mut self, zi: usize, c: usize, n: f64, per_face: Option<f64>) -> f64 {
        let count = self.z[zi].count(c);
        if count <= 0.0 || n <= 0.0 {
            return 0.0;
        }
        let f = (n / count).min(1.0);
        let nt = self.config.species(Species::ALL[c]).n_tissue;
        let zs = &mut self.z[zi];
        let mut material = 0.0;
        for k in zs.cohorts[c].iter_mut() {
            let d = k.n * f;
            zs.dead_wood += d * k.st.wood;
            zs.litter += d * (k.st.material() - k.st.wood);
            material += d * k.st.material();
            k.n -= d;
        }
        if c == GLOW {
            return 0.0;
        }
        // The stands' mineral inventory goes to their faces, and the faces' pool moves on.
        zs.pool[c] += nt * material;
        let carried = per_face.map_or(zs.pool[c] * f, |m| (m * n).min(zs.pool[c]));
        zs.pool[c] -= carried;
        carried
    }

    /// One step of `dt` seconds.
    pub fn step(&mut self) {
        let dt = self.p.dt;
        if let Weather::Replay { period_s, .. } = self.p.weather
            && self.t_s >= self.next_weather_s
        {
            let n = self.zones.zones[0].weather.len();
            self.weather_ix = self.rng.below(n);
            self.next_weather_s += period_s;
        }
        let clock: Clock = self.day.at(self.t_s, dt);
        let config = self.config.clone();
        let nz = self.z.len();
        for zi in 0..nz {
            let zone = self.zones.zones[zi].clone();
            let readings: Vec<Reading> = (0..NS).map(|s| self.reading(zi, s)).collect();
            // ---- the fungi's feed (step.rs:2183): one bounded draw on the zone's pools.
            let gsc = config.species(Species::Glowcap);
            let mu_g = readings[GLOW].mu_held;
            let demand: f64 = self.z[zi].cohorts[GLOW].iter().map(|c| c.n * gsc.substrate_uptake_per_s * c.st.wood * mu_g * dt).sum();
            let held = self.z[zi].litter + self.z[zi].dead_wood;
            let taken = demand.min(held).max(0.0);
            if held > 0.0 && taken > 0.0 {
                let f = taken / held;
                self.z[zi].litter -= self.z[zi].litter * f;
                self.z[zi].dead_wood -= self.z[zi].dead_wood * f;
            }
            self.c.substrate += taken;
            let per_demand = if demand > 0.0 { taken / demand } else { 0.0 };
            // ---- growth of every cohort.
            let counts: Vec<f64> = (0..NS).map(|s| self.z[zi].count(s)).collect();
            let vacant_n = (zone.area - (0..NS).filter(|&s| s != GLOW).map(|s| counts[s]).sum::<f64>()).max(0.0);
            let (cover, transmit) = (self.z[zi].cover, self.z[zi].transmit);
            let mut packages = [0.0; NS];
            for s in 0..NS {
                let sp = Species::ALL[s];
                let sc = config.species(sp);
                let r = readings[s];
                let photo = sc.trophic == Trophic::Photo;
                // The `μ` and aeration levels the zone's faces read, as one stand's inputs.
                // The readings on the faces the species holds, as one stand's inputs: the
                // available water whose ramp is their mean `μ`, the saturated fraction whose
                // target is their mean target.
                let available = if r.mu_held < 1.0 { sc.wilt_water + r.mu_held * (sc.full_water - sc.wilt_water) } else { r.available_held.max(sc.full_water) };
                let saturated = sc.establish_saturated_max + r.stress_held * (1.0 - sc.establish_saturated_max);
                let mineral = if photo {
                    if counts[s] > 0.0 { self.z[zi].pool[s] / counts[s] } else { config.initial_mineral }
                } else {
                    1.0
                };
                let nt = sc.n_tissue;
                let mut dead: Vec<(usize, Death)> = Vec::new();
                let mut d_material = 0.0;
                let (mut add_litter, mut add_wood) = (0.0, 0.0);
                for (ci, c) in self.z[zi].cohorts[s].iter_mut().enumerate() {
                    if c.n <= 0.0 {
                        continue;
                    }
                    let stage = usize::from(c.st.wood >= sc.donor_min);
                    let [la, lk] = self.p.light_k[s][stage];
                    let l_raw = zone.sky * (la - lk * cover).max(0.02);
                    let site = stand::Site {
                        light_eff: light_response(sc, l_raw),
                        available,
                        saturated,
                        mineral,
                        darkness: zone.darkness,
                        substrate_taken: if photo { 0.0 } else { per_demand * sc.substrate_uptake_per_s * c.st.wood * mu_g * dt },
                        substrate_gross: if photo { 0.0 } else { per_demand * sc.substrate_uptake_per_s * c.st.wood * mu_g * dt * sc.substrate_yield },
                    };
                    let before = c.st.material();
                    let (fl, death) = stand::grow(sc, &mut c.st, site, clock);
                    let (sent, resp) = if death.is_none() { stand::fund(sc, &mut c.st, dt) } else { (0, 0.0) };
                    self.c.respired += c.n * (fl.respired + resp);
                    self.c.seep += c.n * fl.seep;
                    let pk = package_of(sc);
                    packages[s] += c.n * f64::from(sent);
                    d_material += c.n * (c.st.material() - before + f64::from(sent) * pk);
                    add_litter += c.n * fl.litter;
                    add_wood += c.n * fl.dead_wood;
                    self.c.built[s] += c.n * fl.built;
                    self.c.shed[s] += c.n * fl.litter;
                    if photo {
                        self.c.fixed += c.n * fl.income;
                    }
                    if let Some(d) = death {
                        dead.push((ci, d));
                    }
                }
                self.z[zi].litter += add_litter;
                self.z[zi].dead_wood += add_wood;
                if photo {
                    self.z[zi].pool[s] -= nt * d_material;
                }
                // Deaths free their faces.
                for &(ci, d) in dead.iter().rev() {
                    let c = self.z[zi].cohorts[s].remove(ci);
                    match d {
                        Death::Starved => self.c.starved[s] += c.n,
                        Death::Drought => self.c.drought[s] += c.n,
                    }
                    self.z[zi].dead_wood += c.n * c.st.wood;
                    self.z[zi].litter += c.n * (c.st.material() - c.st.wood);
                    if photo {
                        let cnt = counts[s].max(c.n);
                        self.z[zi].pool[s] += nt * c.n * c.st.material();
                        let carried = self.z[zi].pool[s] * c.n / cnt;
                        self.z[zi].pool[s] -= carried;
                        self.z[zi].pool[VACANT] += carried;
                    }
                }
                // A browser of seedlings (scenario lever).
                let h = self.p.juvenile_browse[s];
                if h > 0.0 {
                    let q = 1.0 - (-h * dt).exp();
                    let mut gone = 0.0;
                    let mut mat = 0.0;
                    for c in self.z[zi].cohorts[s].iter_mut().filter(|c| c.st.wood < sc.donor_min) {
                        let d = c.n * q;
                        c.n -= d;
                        gone += d;
                        mat += d * c.st.material();
                    }
                    self.c.browsed[s] += gone;
                    if gone > 0.0 && counts[s] > 0.0 {
                        self.z[zi].pool[s] += nt * mat;
                        let carried = self.z[zi].pool[s] * gone / counts[s].max(gone);
                        self.z[zi].pool[s] -= carried;
                        self.z[zi].pool[VACANT] += carried;
                    }
                }
            }
            // ---- runners and seeds (seeds.rs:847).
            let a = zone.area.max(1.0);
            let counts: Vec<f64> = (0..NS).map(|s| self.z[zi].count(s)).collect();
            let vacant = (zone.area - (0..NS).filter(|&s| s != GLOW).map(|s| counts[s]).sum::<f64>()).max(0.0);
            let _ = vacant_n;
            for s in 0..NS {
                let pk = packages[s];
                if pk <= 0.0 {
                    continue;
                }
                let sp = Species::ALL[s];
                let sc = config.species(sp);
                let nt = sc.n_tissue;
                let package = package_of(sc);
                let r = readings[s];
                let mut runners = 0.0;
                if sc.clonal_share > 0.0 && s != GLOW {
                    // The faces a runner may take, by type and light class, and the share
                    // of each rich enough for the daughter (`seeds.rs:1078`).
                    let mut targets: Vec<(usize, bool, f64, f64)> = Vec::new();
                    for (l, share) in [(false, 1.0)] {
                        let l_eff = light_response(sc, zone.sky * (self.p.light_k[s][0][0] - self.p.light_k[s][0][1] * cover).max(0.02));
                        let ns = n_star(sc, l_eff, r.mu);
                        for o in (0..=NS).filter(|&o| o == VACANT || (o != GLOW && runner_may_take(&config, sp, Species::ALL[o]))) {
                            let cnt = if o == VACANT { vacant } else { counts[o] };
                            if cnt <= 0.0 {
                                continue;
                            }
                            let mean = self.z[zi].pool[o] / cnt;
                            let (f, cm) = above(mean, self.p.sigma[o], ns);
                            let k = cnt * share * f * r.gate;
                            if k > 0.0 {
                                targets.push((o, l, k, cm));
                            }
                        }
                    }
                    let takeable: f64 = targets.iter().map(|t| t.2).sum();
                    let theta = (self.p.psi_runner[s] * takeable / a).clamp(0.0, 1.0);
                    let rb = self.runner_box[s];
                    let pt = 1.0 - (1.0 - theta).powf(rb.faces * rb.same_zone);
                    let cs = sc.clonal_share;
                    let share = if cs >= 1.0 { 1.0 } else { cs * pt / (cs * pt + 1.0 - cs) };
                    runners = (pk * share).min(takeable);
                    if runners > 0.0 {
                        let support = if sc.trophic == Trophic::Photo { sc.runner_support_s } else { 0.0 };
                        for &(o, l, k, cm) in &targets {
                            let n = runners * k / takeable;
                            let carried = if o == VACANT {
                                let m = (cm * n).min(self.z[zi].pool[VACANT]);
                                self.z[zi].pool[VACANT] -= m;
                                m
                            } else {
                                self.c.displaced[o] += n;
                                self.c.pairs[o][s] += n;
                                self.remove_stands(zi, o, n, Some(cm))
                            };
                            self.z[zi].pool[s] += carried;
                            self.add_newborns(zi, s, n, package, if l { 1.0 } else { 0.0 }, support);
                        }
                        self.c.births_runner[s] += runners;
                    }
                }
                let seeds = pk - runners;
                // Each package's mineral leaves the donor's faces with it (`seeds.rs:953`):
                // `nt` per unit; the pool already paid it as material. Seeds land in banks.
                let kern = &self.kernels[s];
                for z2 in 0..nz {
                    let f = kern.land[zi][z2];
                    if f > 0.0 {
                        self.z[z2].bank[s] += seeds * f;
                        self.c.seeds_landed[s] += seeds * f;
                    }
                }
                let lost = seeds * kern.lost[zi];
                if lost > 0.0 {
                    self.z[zi].litter += lost * package;
                    if s != GLOW {
                        self.spread_mineral(zi, nt * lost * package);
                    }
                }
            }
            // ---- the plant place's lottery and overtopping (seeds.rs:466).
            self.germinate(zi, &readings, clock.daylight > 0.0);
            // ---- the fungus place.
            self.germinate_fungus(zi, &readings);
            // ---- the bank's attrition and age (seeds.rs:745), as one rate: a bank e-folding at
            // its lifetime and cut off at it keeps `1 − 1/e` of a lifetime on average.
            for s in 0..NS {
                let sc = config.species(Species::ALL[s]);
                let life = (1.0 - (-1.0f64).exp()) * sc.seed_max_age_s.max(1.0);
                let d = self.z[zi].bank[s] * (1.0 - (-dt / life).exp());
                if d > 0.0 {
                    self.z[zi].bank[s] -= d;
                    self.c.seeds_died[s] += d;
                    let pk = package_of(sc);
                    self.z[zi].litter += d * pk;
                    if s != GLOW {
                        self.spread_mineral(zi, sc.n_tissue * d * pk);
                    }
                }
            }
            // ---- decomposition (step.rs:3659).
            let zs = &mut self.z[zi];
            let dl = zs.litter * (1.0 - (-config.decomposition * dt).exp());
            let dw = zs.dead_wood * (1.0 - (-config.wood_decomposition * dt).exp());
            zs.litter -= dl;
            zs.dead_wood -= dw;
            self.c.decomposed += dl + dw;
        }
        self.apply_take();
        self.t_s += dt;
        if self.t_s >= self.next_rebin_s {
            self.canopy();
            self.rebin();
            self.next_rebin_s += self.p.rebin_s;
        }
    }

    /// Mineral landing on a zone's faces at large (a dead seed's), spread by face count.
    fn spread_mineral(&mut self, zi: usize, m: f64) {
        let a = self.zones.zones[zi].area.max(1.0);
        let counts: Vec<f64> = (0..NS).map(|s| if s == GLOW { 0.0 } else { self.z[zi].count(s) }).collect();
        let vacant = (a - counts.iter().sum::<f64>()).max(0.0);
        for s in 0..NS {
            self.z[zi].pool[s] += m * counts[s] / a;
        }
        self.z[zi].pool[VACANT] += m * vacant / a;
    }

    fn germinate(&mut self, zi: usize, readings: &[Reading], day: bool) {
        if !day {
            return;
        }
        let config = self.config.clone();
        let dt = self.p.dt;
        let check = config.seed_check_s.max(cubarium_voxel::DT);
        let zone_sky = self.zones.zones[zi].sky;
        let a = self.zones.zones[zi].area.max(1.0);
        let counts: Vec<f64> = (0..NS).map(|s| self.z[zi].count(s)).collect();
        let vacant = (a - (0..NS).filter(|&s| s != GLOW).map(|s| counts[s]).sum::<f64>()).max(0.0);
        let (cover, transmit) = (self.z[zi].cover, self.z[zi].transmit);
        // Seeds present on a face, per species.
        let mut seed = [0.0; NS];
        let mut dens = [0.0; NS];
        for s in 0..NS {
            if s == GLOW || self.z[zi].bank[s] <= 0.0 || !is_photo(&config, s) {
                continue;
            }
            dens[s] = self.p.psi[s] * self.z[zi].bank[s] / a;
            seed[s] = readings[s].gate * (1.0 - (-dens[s]).exp());
        }
        for (l, share) in [(false, 1.0)] {
            if share <= 0.0 {
                continue;
            }
            let nstar: Vec<f64> = (0..NS)
                .map(|s| {
                    if seed[s] <= 0.0 {
                        return f64::INFINITY;
                    }
                    let sc = config.species(Species::ALL[s]);
                    n_star(sc, light_response(sc, zone_sky * (self.p.light_k[s][0][0] - self.p.light_k[s][0][1] * cover).max(0.02)), readings[s].mu)
                })
                .collect();
            // Vacant faces: armed and sprouted within two checks; the lottery by weight.
            if vacant > 0.0 {
                let mean = self.z[zi].pool[VACANT] / vacant;
                let mut pres = [0.0; NS];
                let mut cm = [0.0; NS];
                let mut weight = [0.0; NS];
                for s in 0..NS {
                    if seed[s] <= 0.0 {
                        continue;
                    }
                    let (f, m) = above(mean, self.p.sigma[VACANT], nstar[s]);
                    pres[s] = seed[s] * f;
                    cm[s] = m;
                    weight[s] = dens[s] * f64::from(config.species(Species::ALL[s]).seed_mass.max(1)) * readings[s].gate * f;
                }
                let none: f64 = pres.iter().map(|p| 1.0 - p).product();
                let wsum: f64 = weight.iter().sum();
                let lam = (1.0 - none) / (2.0 * check);
                let conv = vacant * share * (1.0 - (-lam * dt).exp());
                if conv > 0.0 && wsum > 0.0 {
                    for s in 0..NS {
                        let k = (conv * weight[s] / wsum).min(self.z[zi].bank[s]);
                        if k <= 0.0 {
                            continue;
                        }
                        let mv = (cm[s] * k).min(self.z[zi].pool[VACANT]);
                        self.z[zi].pool[VACANT] -= mv;
                        self.z[zi].pool[s] += mv;
                        self.z[zi].bank[s] -= k;
                        let organic = package_of(config.species(Species::ALL[s]));
                        self.add_newborns(zi, s, k, organic, if l { 1.0 } else { 0.0 }, 0.0);
                        self.c.births_seed[s] += k;
                    }
                }
            }
            // Covers: a seedling that may come up through one wins its draw per check, on
            // the cover's faces rich enough for it.
            for c in 0..NS {
                if c == GLOW || counts[c] <= 0.0 || !config.species(Species::ALL[c]).ground_cover {
                    continue;
                }
                let mean = self.z[zi].pool[c] / counts[c];
                for s in 0..NS {
                    if seed[s] <= 0.0 || !may_overtop(&config, Species::ALL[s], Species::ALL[c]) {
                        continue;
                    }
                    let (f, cm) = above(mean, self.p.sigma[c], nstar[s]);
                    let rho = config.species(Species::ALL[s]).overtop_per_check;
                    let lam = seed[s] * f * rho / check;
                    let k = (counts[c] * share * (1.0 - (-lam * dt).exp())).min(self.z[zi].bank[s]);
                    if k <= 0.0 {
                        continue;
                    }
                    let carried = self.remove_stands(zi, c, k, Some(cm));
                    self.z[zi].pool[s] += carried;
                    self.z[zi].bank[s] -= k;
                    self.c.overtopped[c] += k;
                    self.c.pairs[c][s] += k;
                    let organic = package_of(config.species(Species::ALL[s]));
                    self.add_newborns(zi, s, k, organic, if l { 1.0 } else { 0.0 }, 0.0);
                    self.c.births_seed[s] += k;
                }
            }
        }
    }

    fn germinate_fungus(&mut self, zi: usize, readings: &[Reading]) {
        let sc = self.config.species(Species::Glowcap);
        let a = self.zones.zones[zi].area.max(1.0);
        let space = self.zones.zones[zi].fungal_space * a;
        let free = (space - self.z[zi].count(GLOW)).max(0.0);
        let bank = self.z[zi].bank[GLOW];
        if free <= 0.0 || bank <= 0.0 {
            return;
        }
        let check = self.config.seed_check_s.max(cubarium_voxel::DT);
        let b = self.p.psi[GLOW] * bank / a;
        // The gate's share among the faces a fungus may hold.
        let g = (readings[GLOW].gate / self.zones.zones[zi].fungal_space.max(1e-9)).min(1.0);
        let lam = g * (1.0 - (-b).exp()) / (2.0 * check);
        let k = (free * (1.0 - (-lam * self.p.dt).exp())).min(bank);
        if k > 0.0 {
            self.z[zi].bank[GLOW] -= k;
            self.add_newborns(zi, GLOW, k, package_of(sc), 0.0, 0.0);
            self.c.births_seed[GLOW] += k;
        }
    }

    /// The animals' clamped take this step: foliage from each species' cohorts by edible
    /// share, cap tissue from the glowcap's, litter from the pools; returns to the pools.
    fn apply_take(&mut self) {
        let take = self.take.clone();
        let ret = self.returns.clone();
        let dt = self.p.dt;
        // Litter, from every zone by its share of the pool; dung and shed to litter.
        let total_litter: f64 = self.z.iter().map(|z| z.litter).sum();
        let lt = take.litter.min(total_litter).max(0.0);
        if total_litter > 0.0 {
            for zs in &mut self.z {
                zs.litter -= lt * zs.litter / total_litter;
            }
        }
        let za: f64 = self.zones.zones.iter().map(|z| z.area).sum();
        for zi in 0..self.z.len() {
            let share = self.zones.zones[zi].area / za.max(1.0);
            self.z[zi].litter += ret.litter * share;
        }
        self.c.taken.litter += lt;
        self.c.returned += ret.litter + ret.carrion;
        // Carrion (`Ground::carrion`, `lib.rs:584`): its own pool, decaying at
        // `carrion_decomposition` (`step.rs:3628`), eaten from.
        self.carrion += ret.carrion;
        let ct = take.carrion.min(self.carrion).max(0.0);
        self.carrion -= ct;
        self.c.taken.carrion += ct;
        let dc = self.carrion * (1.0 - (-self.config.carrion_decomposition * dt).exp());
        self.carrion -= dc;
        self.c.decomposed += dc;
        // Glassfilm (`film.rs`): one logistic stock over its eligible faces.
        self.step_film(dt);
        let ft = take.film.min((self.film - self.config.film_graze_refuge * self.film_capacity).max(0.0)).max(0.0);
        self.film -= ft;
        self.c.taken.film += ft;
        // Foliage and caps, by edible share; nectar and fruit from their stocks.
        for s in 0..NS {
            let want = take.foliage[s];
            if want <= 0.0 {
                continue;
            }
            let sc = self.config.species(Species::ALL[s]);
            let ed: f64 = self.z.iter().flat_map(|z| z.cohorts[s].iter()).map(|c| c.n * (c.st.foliage - sc.graze_refuge * sc.alpha * c.st.wood).max(0.0)).sum();
            if ed <= 0.0 {
                continue;
            }
            let f = (want / ed).min(1.0);
            for zs in &mut self.z {
                for c in zs.cohorts[s].iter_mut() {
                    let e = (c.st.foliage - sc.graze_refuge * sc.alpha * c.st.wood).max(0.0);
                    c.st.foliage -= e * f;
                }
            }
            self.c.taken.foliage[s] += want.min(ed);
            self.c.taken_mineral += sc.n_tissue * want.min(ed);
        }
        for (want, nectar) in [(take.nectar, true), (take.fruit, false)] {
            if want <= 0.0 {
                continue;
            }
            let held: f64 = self.z.iter().flat_map(|z| z.cohorts.iter().flatten()).map(|c| c.n * if nectar { c.st.nectar } else { c.st.fruit }).sum();
            if held <= 0.0 {
                continue;
            }
            let f = (want / held).min(1.0);
            for zs in &mut self.z {
                for c in zs.cohorts.iter_mut().flatten() {
                    if nectar {
                        c.st.nectar -= c.st.nectar * f;
                    } else {
                        c.st.fruit -= c.st.fruit * f;
                    }
                }
            }
            self.c.taken_mineral += self.config.species(Species::Springturf).n_tissue * want.min(held);
            if nectar {
                self.c.taken.nectar += want.min(held);
            } else {
                self.c.taken.fruit += want.min(held);
            }
        }
    }

    /// The film's growth, upkeep and nothing else (`film.rs`): `rate · light · F · (1 −
    /// F / K) − upkeep · F`, light the eligible faces' sky under the water's extinction.
    /// **Aggregation**: one stock over every eligible face (the spread to neighbours, the
    /// dry-site death and the depth changes are frozen with the water).
    fn step_film(&mut self, dt: f64) {
        let c = &self.config;
        let k = self.film_capacity;
        if !(k > 0.0) {
            return;
        }
        let day = self.day.at(self.t_s, dt).daylight;
        let grow = c.film_growth_per_s * self.film_light * day * self.film * (1.0 - self.film / k);
        let upkeep = c.film_maintenance_per_s * self.film;
        let d = (grow - upkeep) * dt;
        self.film = (self.film + d).max(0.0);
        self.c.fixed += (grow * dt).max(0.0);
        self.c.respired += upkeep * dt;
    }

    /// What the producers and the pools hold for the animals now (exchange).
    pub fn supply(&self) -> Supply {
        let mut s = Supply::default();
        s.carrion = self.carrion;
        s.film = (self.film - self.config.film_graze_refuge * self.film_capacity).max(0.0);
        for zs in &self.z {
            s.litter += zs.litter;
            for k in 0..NS {
                let sp = Species::ALL[k];
                for c in &zs.cohorts[k] {
                    s.foliage[k] += c.n * edible(&self.config, sp, c.st.wood, c.st.foliage);
                    s.nectar += c.n * c.st.nectar;
                    s.fruit += c.n * c.st.fruit;
                }
            }
        }
        s
    }

    /// World totals: stands, donors (`W ≥ donor_min`) and mean `W` per species.
    pub fn stands(&self) -> [(f64, f64, f64); NS] {
        let mut out = [(0.0, 0.0, 0.0); NS];
        for zs in &self.z {
            for k in 0..NS {
                let sc = self.config.species(Species::ALL[k]);
                for c in &zs.cohorts[k] {
                    out[k].0 += c.n;
                    if c.st.wood >= sc.donor_min {
                        out[k].1 += c.n;
                    }
                    out[k].2 += c.n * c.st.wood;
                }
            }
        }
        out
    }

    pub fn minute(&self) -> f64 {
        self.t_s / 60.0
    }

    /// Total site mineral, stand inventories (`n_tissue · material`) and bank mineral.
    pub fn mineral(&self) -> (f64, f64, f64) {
        let mut pool = 0.0;
        let mut inv = 0.0;
        let mut bank = 0.0;
        for zs in &self.z {
            pool += zs.pool.iter().sum::<f64>();
            for k in 0..NS {
                let sc = self.config.species(Species::ALL[k]);
                if k == GLOW {
                    continue;
                }
                inv += zs.cohorts[k].iter().map(|c| c.n * sc.n_tissue * c.st.material()).sum::<f64>();
                bank += sc.n_tissue * package_of(sc) * zs.bank[k];
            }
        }
        (pool, inv, bank)
    }

    /// Every unit of organic matter the half holds: stands (all of their material), the
    /// banks, litter and dead wood.
    pub fn organic(&self) -> f64 {
        let mut o = 0.0;
        for zs in &self.z {
            o += zs.litter + zs.dead_wood;
            for k in 0..NS {
                let sc = self.config.species(Species::ALL[k]);
                o += zs.cohorts[k].iter().map(|c| c.n * c.st.material()).sum::<f64>();
                o += package_of(sc) * zs.bank[k];
            }
        }
        o + self.carrion + self.film
    }

    /// The cohort count (a cost readout).
    pub fn cohorts(&self) -> usize {
        self.z.iter().map(|z| z.cohorts.iter().map(Vec::len).sum::<usize>()).sum()
    }

    /// Stands of `s` in zone `zi` with wood at least `w`.
    pub fn count_at_least(&self, zi: usize, s: usize, w: f64) -> f64 {
        self.z[zi].cohorts[s].iter().filter(|c| c.st.wood >= w).map(|c| c.n).sum()
    }
}
