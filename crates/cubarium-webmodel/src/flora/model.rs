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
    /// Where its stands stand ([`Site`]): their light and mineral against the zone's.
    pub site: Site,
}

/// **A cohort's faces**, carried from its birth (the passing sample faces, or the census
/// stands): their light is `zone sky · l_base · exp(−τ)`, `τ` the fixed part (its parents'
/// shade, which moves with the stand) plus each species' shade then (`tau_ref`, at the
/// zone foliage `f_ref`), moved with that species' foliage in the zone: down in proportion
/// as it shrinks, and up by the zone's mean rate per unit foliage (`Zone::kappa`) as it
/// grows (new crowns spread over the zone, not over the faces already shaded); their site
/// mineral is the species' pool mean
/// times `n_rel` (over the zone's mean `n_rel`). A non-cover stand past `donor_min` stands
/// above the ground covers and its parents: their parts drop out. **Aggregation**: the
/// faces' spread rides in cohorts binned by light and mineral; a cohort's site is its
/// stands' mean.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub l_base: f64,
    pub tau_fixed: f64,
    pub tau_ref: [f64; NS],
    pub f_ref: [f64; NS],
    pub n_rel: f64,
    /// Its root-box available water over the mean of the species' held faces in the zone
    /// (faces keep their rank as the weather moves; the zone's state sets the level).
    pub w_rel: f64,
}

impl Default for Site {
    fn default() -> Site {
        Site { l_base: 1.0, tau_fixed: 0.0, tau_ref: [0.0; NS], f_ref: [0.0; NS], n_rel: 1.0, w_rel: 1.0 }
    }
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
    /// The zone's foliage per species now (refreshed with the canopy), which scales the
    /// newborn light samples' optical depths.
    pub foliage: [f64; NS],
}

impl ZoneState {
    pub fn count(&self, s: usize) -> f64 {
        self.cohorts[s].iter().map(|c| c.n).sum()
    }
    fn plants(&self) -> f64 {
        (0..NS).filter(|&s| s != GLOW).map(|s| self.count(s)).sum()
    }
}

/// Which sampled faces an establishment reads ([`super::zones::Zone`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleKind {
    Open,
    Seeded,
    Runner,
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
    /// The canopy gate on the **newborn's own light** over the zone's sampled faces
    /// ([`super::zones::Sample`]), the sample's optical depths scaled by each shading
    /// species' foliage now over then. `false`: the earlier rule, one light per zone from
    /// the juveniles' fitted `a` (kept to compare).
    pub newborn_light: bool,
    /// **Fitted (spatial)**: the seed shadow's negative-binomial clumping per species
    /// ([`super::fit::clumping`]); infinity is well mixed (Poisson).
    pub seed_k: [f64; NS],
    /// **Fitted (spatial)**: the same per zone ([`super::fit::clumping_by_zone`]); empty:
    /// `seed_k` everywhere.
    pub seed_k_zone: Vec<[f64; NS]>,
    /// **Fitted (spatial)**: how a species' own shade over its seed shadow grows with its
    /// foliage, `τ_self ∝ F^β` ([`super::fit::self_shade_beta`]).
    pub self_beta: [f64; NS],
    /// **Fitted (emergent)**: the share of a snapshot's establishable faces (seed, gates and
    /// canopy gate passing) that a newborn takes per unit of the sim's own conversion rate
    /// (`overtop_per_check / seed_check_s` under a cover, a lottery within two checks on
    /// bare ground): faces flicker out of eligibility (water, the arming check) faster than
    /// they convert, so the sustained rate is a share of the instantaneous one. From the
    /// `sp` probes e5/e6/l5/l6 (minutes 30–90 after load, the load burst left out) against
    /// each start's census funnel, a ratio of sums over the four (per-probe spread in
    /// brackets): lanternberry 0.33 (0.24–0.49, 75 faces), velvetpad 0.30 (0.14–0.63, 53),
    /// springturf 0.99 (0.2–12, 53; weather-driven), bloomcrown 0.41 (0.1–1.25, 9),
    /// vaulttree 0.86 (0.56–1.2, 11), stonecushion 0.19 (0.03–0.25, 6), umbrellafrond 0.29
    /// (0.04–0.16, 5); siphonreed none (0.31, the lanternberry–pad pool).
    pub establish_phi: [f64; NS],
    /// Each cohort reads its own faces' water ([`Site::w_rel`]) rather than the zone's mean.
    pub water_rank: bool,
    /// **Scenario lever, not a sim knob**: per species, its stands read their zone's
    /// start-of-run site mineral mean instead of the pool now, and the pool is topped up to
    /// that mean from outside (the counterfactual without self-depletion; the top-up is
    /// `Counters::held_in`).
    pub hold_mineral: [bool; NS],
    /// Numerics: the log-sd each sampled face's mineral is smoothed by.
    pub sample_sigma: f64,
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
            newborn_light: true,
            seed_k: [f64::INFINITY; NS],
            seed_k_zone: Vec::new(),
            self_beta: [0.5; NS],
            establish_phi: [0.41, 0.29, 0.99, 0.19, 0.30, 1.0, 0.86, 0.33, 0.31],
            water_rank: true,
            hold_mineral: [false; NS],
            sample_sigma: 0.1,
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
    /// Of `decomposed`, the carrion pool's share.
    pub decomposed_carrion: f64,
    pub taken: Take,
    /// Mineral the animals' returns brought back to the site pools (closed loop).
    pub returned_mineral: f64,
    /// Mineral in the glowcap caps the animals ate: the fungal place keeps no mineral
    /// inventory in this half, so it enters the plant ledger only when it comes back.
    pub cap_mineral_taken: f64,
    /// Organic matter of seedlings a browser took ([`Take::seedlings`]): out of the half.
    pub browsed_organic: f64,
    /// Per zone, what the last exchange took: `[organic, mineral]`. The returns go back where
    /// the animals fed ([`Model::give`]).
    pub take_z: Vec<[f64; 2]>,
    /// Glassfilm grown (gross) and its upkeep, organic units.
    pub film_grown: f64,
    pub film_upkeep: f64,
    /// Mineral the `hold_mineral` lever added (a scenario's external source).
    pub held_in: f64,
    /// Per zone, the producers' output: `[litter in, dead wood in, glowcap caps built,
    /// plant foliage built]` (organic units; the animals' returns not in it).
    pub prod_z: Vec<[f64; 4]>,
    /// Per zone: `[births (seed + runner), starved + drought]` per species.
    pub by_zone: Vec<[[f64; NS]; 2]>,
}

/// The whole half.
#[derive(Clone)]
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
    /// Each zone's site mineral mean per species at the start (the `hold_mineral` lever).
    pub start_mineral: Vec<[f64; NS]>,
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

/// Merge cohort `b` into `a`: counts add, every state is the count-weighted mean.
pub fn merge(a: &mut Cohort, b: &Cohort) {
    let n = a.n + b.n;
    if n <= 0.0 {
        return;
    }
    let (wa, wb) = (a.n / n, b.n / n);
    // A weighted mean that never leaves its inputs' range: two newborns at `alive_min`
    // must not merge into a stand a rounding below it (which the death rule would kill).
    let m = |x: f64, y: f64| (wa * x + wb * y).clamp(x.min(y), x.max(y));
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
    let (p, q) = (&mut a.site, &b.site);
    p.l_base = m(p.l_base, q.l_base);
    p.tau_fixed = m(p.tau_fixed, q.tau_fixed);
    p.n_rel = m(p.n_rel, q.n_rel);
    p.w_rel = m(p.w_rel, q.w_rel);
    for j in 0..NS {
        p.tau_ref[j] = m(p.tau_ref[j], q.tau_ref[j]);
        p.f_ref[j] = m(p.f_ref[j], q.f_ref[j]);
    }
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
                foliage: [0.0; NS],
            })
            .collect();
        let t0 = start_minute * 60.0;
        // The zones' foliage per species and each species' mean site mineral, which the
        // census stands' sites are measured against.
        let mut fol0 = vec![[0.0f64; NS]; nz];
        let mut nmean = vec![[[0.0f64; 3]; NS]; nz];
        let photo_k = |sp: Species| super::census::PHOTO.iter().position(|&p| p == sp);
        for f in start {
            let zi = zones.of_face[&(f.x, f.y, f.z)];
            if let Some((sp, r)) = f.plant {
                fol0[zi][sp.index()] += r.foliage;
                nmean[zi][sp.index()][0] += f.mineral;
                nmean[zi][sp.index()][1] += 1.0;
                nmean[zi][sp.index()][2] += photo_k(sp).and_then(|k| f.water[k]).unwrap_or(0.0);
            }
        }
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
                let sc = config.species(sp);
                let sky = zones.zones[zi].sky.max(1e-3);
                let k = sp.index();
                // The light it read last tick, its response inverted; the shade over its
                // crown top split by species and taken out of it.
                let h = sc.light_half.max(0.0);
                let l_raw = if r.light > 0.0 && r.light < 1.0 + h { r.light * h / (1.0 + h - r.light) } else { sky * p.light_k[k][usize::from(r.wood >= sc.donor_min)][0] };
                let tau: f64 = f.pl_tau.iter().map(|&v| f64::from(v)).sum();
                let tau_ref: [f64; NS] = std::array::from_fn(|j| f64::from(f.pl_tau[j]));
                let nm = nmean[zi][k][0] / nmean[zi][k][1].max(1.0);
                let wm = nmean[zi][k][2] / nmean[zi][k][1].max(1.0);
                let w = photo_k(sp).and_then(|kk| f.water[kk]).unwrap_or(0.0);
                let site = Site { l_base: (l_raw / sky * tau.exp()).min(1.0 / sky), tau_fixed: 0.0, tau_ref, f_ref: fol0[zi], n_rel: if nm > 0.0 { f.mineral / nm } else { 1.0 }, w_rel: if wm > 0.0 { w / wm } else { 1.0 } };
                zs.cohorts[k].push(Cohort { n: 1.0, st, covered: f.canopy < 0.9, opened_s: t0, site });
            }
            if let Some(r) = f.fungus {
                let st = StandState { wood: r.wood, foliage: r.foliage, reserve: r.reserve, ..StandState::default() };
                zs.cohorts[GLOW].push(Cohort { n: 1.0, st, covered: false, opened_s: t0, site: Site::default() });
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
            next_weather_s: t0 + match p.weather {
                Weather::Replay { period_s, .. } => period_s,
                Weather::Frozen => 0.0,
            },
            next_rebin_s: t0,
            rng: Rng(match p.weather {
                Weather::Replay { seed, .. } => seed,
                Weather::Frozen => 0,
            }),
            p,
            c: Counters { by_zone: vec![[[0.0; NS]; 2]; nz], prod_z: vec![[0.0; 4]; nz], take_z: vec![[0.0; 2]; nz], ..Counters::default() },
            weather_ix: 0,
            frozen,
            carrion: start.iter().map(|_| 0.0).sum(),
            film: start.iter().map(|f| f.film).sum(),
            film_capacity: 0.0,
            film_light: 0.0,
            take: Take::default(),
            returns: Returns::default(),
            start_mineral: Vec::new(),
        };
        m.start_mineral = (0..m.z.len()).map(|zi| std::array::from_fn(|s| { let c = m.z[zi].count(s); if c > 0.0 { m.z[zi].pool[s] / c } else { m.config.initial_mineral } })).collect();
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

    /// **The establishment pass share** for a newborn of `s` on open faces of zone `zi`
    /// whose site mineral is lognormal (`mean`, `sigma`): over the zone's sampled faces of
    /// `kind` in this weather state, the mean of (the face's gates pass) × (the share of
    /// faces rich enough for the newborn at that face's light and `μ`, [`n_star`]); and
    /// the mean mineral of the faces that pass. A sample's optical depths are scaled by
    /// each shading species' foliage now over then, except its own species' over seeded
    /// and runner faces (those faces come with their parents). With no sample of `kind` in
    /// the zone in any state, the gate-passing faces at large stand in, times `gate_now`.
    /// `fallback`: the zone-light rule's `(L_eff, μ)` when [`Params::newborn_light`] is off
    /// (then times `gate_now` too).
    /// Returns the share, the passing faces' mean mineral, and the newborns' [`Site`]: the
    /// passing faces' mean terrain light and shade (pass-weighted), their mineral over
    /// `mean`.
    fn pass_share(&self, zi: usize, s: usize, occ: usize, mean: f64, sigma: f64, kind: SampleKind, gate_now: f64, fallback: (f64, f64)) -> (f64, f64, Site) {
        let sc = self.config.species(Species::ALL[s]);
        let zone = &self.zones.zones[zi];
        let sky = zone.sky.max(1e-3);
        let flat = |l_raw: f64, cm: f64| Site { l_base: l_raw / sky, tau_fixed: 0.0, tau_ref: [0.0; NS], f_ref: self.z[zi].foliage, n_rel: if mean > 0.0 { cm / mean } else { 1.0 }, w_rel: 1.0 };
        if !self.p.newborn_light || !is_photo(&self.config, s) {
            let (f, cm) = above(mean, sigma, n_star(sc, fallback.0, fallback.1));
            return (f * gate_now, cm, flat(sky * self.p.light_k[s][0][0], cm));
        }
        let Some(k) = super::census::PHOTO.iter().position(|&p| p == Species::ALL[s]) else {
            return (0.0, 0.0, Site::default());
        };
        let set = |kd: SampleKind| match kd {
            SampleKind::Seeded => &zone.seeded,
            SampleKind::Runner => &zone.runner,
            SampleKind::Open => &zone.samples,
        };
        let now_state: Vec<usize> = match self.p.weather {
            Weather::Replay { .. } => vec![self.weather_ix],
            Weather::Frozen => (0..zone.weather.len()).collect(),
        };
        let all: Vec<usize> = (0..zone.weather.len()).collect();
        let has = |kd: SampleKind, st: &[usize]| st.iter().any(|&i| !set(kd)[i][k].is_empty());
        let (kd, states, times) = if has(kind, &now_state) {
            (kind, now_state, 1.0)
        } else if has(kind, &all) {
            (kind, all, 1.0)
        } else {
            (SampleKind::Open, now_state, gate_now)
        };
        let now = &self.z[zi].foliage;
        // The samples of the occupant class asked about (its faces' mineral is measured
        // against its own mean), all of them when it has none.
        let own = states.iter().any(|&st| set(kd)[st][k].iter().any(|x| usize::from(x.occ) == occ));
        let (mut n, mut f_sum, mut m_sum, mut sky_sum, mut fixed_sum, mut w_sum) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let mut tau_sum = [0.0f64; NS];
        for st in states {
            let r = &zone.foliage_ref[st];
            // Self-shade over seeded and runner faces: the fitted power of its foliage;
            // every other shade: the increment rule ([`Model::tau_now`]).
            let self_scale = if r[s] > 1e-9 { (now[s] / r[s]).powf(self.p.self_beta[s]) } else { 1.0 };
            for smp in &set(kd)[st][k] {
                if own && usize::from(smp.occ) != occ {
                    continue;
                }
                n += 1.0;
                if !smp.gate {
                    continue;
                }
                let tau_j = |j: usize| if j == s && kd != SampleKind::Open { f64::from(smp.tau[j]) * self_scale } else { self.tau_now(zi, k, j, f64::from(smp.tau[j]), r[j]) };
                let tau: f64 = (0..NS).map(tau_j).sum();
                let l = f64::from(smp.sky) * (-tau).exp();
                // The face's own mineral, moved with its class's mean, smoothed by a narrow
                // lognormal (the samples stand for a zone's worth of faces).
                let (f, cm) = above(mean * f64::from(smp.n_rel), self.p.sample_sigma.min(sigma), n_star(sc, light_response(sc, l), f64::from(smp.mu)));
                if std::env::var("WM_SMP").is_ok_and(|v| v == format!("{zi}:{s}:{kd:?}:{occ}")) {
                    eprintln!("    smp st={st} sky={:.2} tau={:.2} L={:.2} mu={:.2} N={:.2} n*={:.2} f={:.2}", smp.sky, tau, l, smp.mu, mean * f64::from(smp.n_rel), n_star(sc, light_response(sc, l), f64::from(smp.mu)), f);
                }
                if f <= 0.0 {
                    continue;
                }
                f_sum += f;
                m_sum += f * cm;
                sky_sum += f * f64::from(smp.sky);
                w_sum += f * f64::from(smp.avail) / zone.weather[st][s].available_held.max(1e-6);
                for j in 0..NS {
                    let t = tau_j(j);
                    if j == s && kd != SampleKind::Open {
                        fixed_sum += f * t;
                    } else {
                        tau_sum[j] += f * t;
                    }
                }
            }
        }
        if n <= 0.0 || f_sum <= 0.0 {
            return (0.0, 0.0, Site::default());
        }
        let cm = m_sum / f_sum;
        let site = Site {
            l_base: sky_sum / f_sum / sky,
            tau_fixed: fixed_sum / f_sum,
            tau_ref: std::array::from_fn(|j| tau_sum[j] / f_sum),
            f_ref: *now,
            n_rel: if mean > 0.0 { cm / mean } else { 1.0 },
            w_rel: w_sum / f_sum,
        };
        (times * f_sum / n, cm, site)
    }

    /// A cohort's light now, raw (a readout).
    pub fn cohort_light(&self, zi: usize, s: usize, c: &Cohort) -> f64 {
        self.light_of(zi, s, c)
    }

    /// A cohort's light now, raw (`L`, before the species' response).
    fn light_of(&self, zi: usize, s: usize, c: &Cohort) -> f64 {
        let sc = self.config.species(Species::ALL[s]);
        let escape = !sc.ground_cover && c.st.wood >= sc.donor_min;
        let k = super::census::PHOTO.iter().position(|p| p.index() == s).unwrap_or(0);
        let mut tau = if escape { 0.0 } else { c.site.tau_fixed };
        for j in 0..NS {
            if escape && self.config.species(Species::ALL[j]).ground_cover {
                continue;
            }
            tau += self.tau_now(zi, k, j, c.site.tau_ref[j], c.site.f_ref[j]);
        }
        (self.zones.zones[zi].sky * c.site.l_base * (-tau).exp()).clamp(0.0, 1.0)
    }

    /// Shade by species `j` over a face of receiver `k` ([`super::census::PHOTO`] order)
    /// that read `tau_ref` when the zone held `f_ref` of `j`'s foliage, now.
    fn tau_now(&self, zi: usize, k: usize, j: usize, tau_ref: f64, f_ref: f64) -> f64 {
        let now = self.z[zi].foliage[j];
        if now <= f_ref {
            if f_ref > 1e-9 { tau_ref * now / f_ref } else { tau_ref }
        } else {
            let z = &self.zones.zones[zi];
            tau_ref + z.kappa[k][j] * (now - f_ref) / z.area.max(1.0)
        }
    }

    /// The zone's mean `n_rel` over species `s`'s cohorts (its pool mean is theirs).
    fn n_rel_mean(&self, zi: usize, s: usize) -> f64 {
        let (mut a, mut b) = (0.0, 0.0);
        for c in &self.z[zi].cohorts[s] {
            a += c.n * c.site.n_rel;
            b += c.n;
        }
        if b > 0.0 && a > 0.0 { a / b } else { 1.0 }
    }

    /// Whether species `s`'s seeds float to the water's margins ([`Dispersal::Water`]).
    fn floats(&self, s: usize) -> bool {
        self.config.species(Species::ALL[s]).dispersal == cubarium_voxel_flora::Dispersal::Water
    }

    /// The seeded share of a zone's open faces for species `s` now.
    fn seeded(&self, zi: usize, s: usize) -> f64 {
        let a = self.zones.zones[zi].area.max(1.0);
        let k = self.p.seed_k_zone.get(zi).map_or(self.p.seed_k[s], |v| v[s]);
        super::fit::seeded_share(self.p.psi[s] * self.z[zi].bank[s] / a, k)
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
            let fol: [f64; NS] = std::array::from_fn(|s| self.z[zi].cohorts[s].iter().map(|c| c.n * c.st.foliage).sum());
            let zs = &mut self.z[zi];
            zs.foliage = fol;
            zs.cover = sum_area / a;
            zs.transmit = if sum_area > 0.0 { sum_t / sum_area } else { 1.0 };
        }
    }

    /// A cohort's site class: its light now over the zone's sky times its mineral factor,
    /// cut at 0.3, 0.55 and 0.8 (four classes).
    fn site_bin(&self, zi: usize, s: usize, c: &Cohort) -> u8 {
        if s == GLOW {
            return 0;
        }
        let q = self.light_of(zi, s, c) / self.zones.zones[zi].sky.max(1e-3) * c.site.n_rel / self.n_rel_mean(zi, s);
        let lq = [0.3, 0.55, 0.8].iter().filter(|&&e| q >= e).count() as u8;
        let wq = [0.6, 1.3].iter().filter(|&&e| c.site.w_rel >= e).count() as u8;
        lq + 4 * wq
    }

    /// Merge cohorts by (light class, log-wood bin, vigour, site class), and move receivers
    /// between the light classes as the crown cover moved. Vigour is the foliage over its
    /// cap, cut at 0.1 and 0.4: a starving stand is not averaged into a fed one.
    fn rebin(&mut self) {
        const BINS: f64 = 16.0;
        let config = self.config.clone();
        let bins: Vec<Vec<Vec<u8>>> = (0..self.z.len()).map(|zi| (0..NS).map(|s| self.z[zi].cohorts[s].iter().map(|c| self.site_bin(zi, s, c)).collect()).collect()).collect();
        for (zi, zs) in self.z.iter_mut().enumerate() {
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
                let mut keys: Vec<(bool, i64, bool, u8, u8)> = Vec::new();
                for (ci, c) in zs.cohorts[s].drain(..).enumerate() {
                    if c.n <= 1e-9 {
                        continue;
                    }
                    let b = (((c.st.wood.max(1e-9).ln() - lo) / (hi - lo)) * BINS).floor().clamp(0.0, BINS) as i64;
                    let cap = sc.alpha * c.st.wood;
                    let fill = if cap > 0.0 { c.st.foliage / cap } else { 1.0 };
                    let vigour = [0.1, 0.4].iter().filter(|&&e| fill >= e).count() as u8;
                    let key = (c.covered, b, c.st.cured, vigour, bins[zi][s].get(ci).copied().unwrap_or(0));
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
    /// `cm`: the site mineral of the faces they came up on (their `n_rel` is it over the
    /// species' pool mean in the zone).
    fn add_newborns(&mut self, zi: usize, s: usize, n: f64, organic: f64, covered_share: f64, support_s: f64, mut site: Site, cm: f64) {
        if n <= 0.0 {
            return;
        }
        let sc = self.config.species(Species::ALL[s]);
        if s != GLOW {
            let cnt = self.z[zi].count(s);
            let ms = if cnt > 0.0 { self.z[zi].pool[s] / cnt } else { cm };
            site.n_rel = if ms > 0.0 && cm > 0.0 { cm / ms * self.n_rel_mean(zi, s) } else { 1.0 };
        }
        let mut st = StandState::newborn(sc, organic);
        st.support_s = support_s;
        let t = self.t_s;
        let nursery = self.p.nursery_s;
        for (covered, share) in [(false, 1.0 - covered_share), (true, covered_share)] {
            let k = n * share;
            if k <= 0.0 {
                continue;
            }
            let fresh = Cohort { n: k, st, covered, opened_s: t, site };
            let bin = self.site_bin(zi, s, &fresh);
            let i = self.z[zi].cohorts[s].iter().rposition(|c| c.covered == covered && t - c.opened_s < nursery && c.st.wood < 2.0 * sc.alive_min * f64::from(sc.seed_mass.max(1)) && self.site_bin(zi, s, c) == bin);
            match i.map(|i| &mut self.z[zi].cohorts[s][i]) {
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
            self.c.prod_z[zi][0] += d * (k.st.material() - k.st.wood);
            self.c.prod_z[zi][1] += d * k.st.wood;
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
                    if self.p.hold_mineral[s] {
                        self.start_mineral[zi][s]
                    } else if counts[s] > 0.0 {
                        self.z[zi].pool[s] / counts[s]
                    } else {
                        config.initial_mineral
                    }
                } else {
                    1.0
                };
                let lights: Vec<f64> = self.z[zi].cohorts[s].iter().map(|c| self.light_of(zi, s, c)).collect();
                let nbar = self.n_rel_mean(zi, s);
                let nt = sc.n_tissue;
                let mut dead: Vec<(usize, Death)> = Vec::new();
                let mut d_material = 0.0;
                let (mut add_litter, mut add_wood) = (0.0, 0.0);
                for (ci, c) in self.z[zi].cohorts[s].iter_mut().enumerate() {
                    if c.n <= 0.0 {
                        continue;
                    }
                    // The cohort's own faces' water, at the zone's held level now.
                    let available = if photo && self.p.water_rank { r.available_held * c.site.w_rel } else { available };
                    let site = stand::Site {
                        light_eff: light_response(sc, lights[ci]),
                        available,
                        saturated,
                        mineral: if photo { mineral * c.site.n_rel / nbar } else { mineral },
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
                    self.c.prod_z[zi][if photo { 3 } else { 2 }] += c.n * fl.built;
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
                self.c.prod_z[zi][0] += add_litter;
                self.c.prod_z[zi][1] += add_wood;
                if photo {
                    self.z[zi].pool[s] -= nt * d_material;
                    if self.p.hold_mineral[s] {
                        // The lever's external source: the pool never falls below the start
                        // mean on the species' faces (counted in `held_in`, off the ledger).
                        let floor = counts[s] * self.start_mineral[zi][s];
                        if self.z[zi].pool[s] < floor {
                            self.c.held_in += floor - self.z[zi].pool[s];
                            self.z[zi].pool[s] = floor;
                        }
                    }
                }
                // Deaths free their faces.
                for &(ci, d) in dead.iter().rev() {
                    let c = self.z[zi].cohorts[s].remove(ci);
                    match d {
                        Death::Starved => self.c.starved[s] += c.n,
                        Death::Drought => self.c.drought[s] += c.n,
                    }
                    self.c.by_zone[zi][1][s] += c.n;
                    self.z[zi].dead_wood += c.n * c.st.wood;
                    self.z[zi].litter += c.n * (c.st.material() - c.st.wood);
                    self.c.prod_z[zi][0] += c.n * (c.st.material() - c.st.wood);
                    self.c.prod_z[zi][1] += c.n * c.st.wood;
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
                    // The browser ate the seedlings' tissue: out of the half, as a take.
                    self.c.browsed_organic += mat;
                    self.c.taken.foliage[s] += mat;
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
                    let mut targets: Vec<(usize, bool, f64, f64, Site)> = Vec::new();
                    for (l, share) in [(false, 1.0)] {
                        let l_eff = light_response(sc, zone.sky * (self.p.light_k[s][0][0] - self.p.light_k[s][0][1] * cover).max(0.02));
                        for o in (0..=NS).filter(|&o| o == VACANT || (o != GLOW && runner_may_take(&config, sp, Species::ALL[o]))) {
                            let cnt = if o == VACANT { vacant } else { counts[o] };
                            if cnt <= 0.0 {
                                continue;
                            }
                            let mean = self.z[zi].pool[o] / cnt;
                            let (f, cm, site) = self.pass_share(zi, s, o, mean, self.p.sigma[o], SampleKind::Runner, r.gate, (l_eff, r.mu));
                            let k = cnt * share * f * zone.box_enrich[s][o];
                            if k > 0.0 {
                                targets.push((o, l, k, cm, site));
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
                        for &(o, l, k, cm, site) in &targets {
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
                            self.add_newborns(zi, s, n, package, if l { 1.0 } else { 0.0 }, support, site, cm);
                        }
                        self.c.births_runner[s] += runners;
                        self.c.by_zone[zi][0][s] += runners;
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
                    self.c.prod_z[zi][0] += lost * package;
                    if s != GLOW {
                        self.spread_mineral(zi, nt * lost * package);
                    }
                }
            }
            // ---- the plant place's lottery and overtopping (seeds.rs:466).
            self.germinate(zi, &readings, clock.daylight > 0.0);
            // ---- the fungus place.
            self.germinate_fungus(zi, &readings);
            // ---- the bank's attrition and age (seeds.rs:745), as one rate: the sim's mean seed
            // life, attrition `a` with the cut-off at `T`, is `(1 − e^(−aT)) / a` (`T` with no
            // attrition).
            for s in 0..NS {
                let sc = config.species(Species::ALL[s]);
                let (ra, tm) = (sc.seed_attrition_per_s, sc.seed_max_age_s.max(1.0));
                let life = if ra > 0.0 { (1.0 - (-ra * tm).exp()) / ra } else { tm }.max(1.0);
                let d = self.z[zi].bank[s] * (1.0 - (-dt / life).exp());
                if d > 0.0 {
                    self.z[zi].bank[s] -= d;
                    self.c.seeds_died[s] += d;
                    let pk = package_of(sc);
                    self.z[zi].litter += d * pk;
                    self.c.prod_z[zi][0] += d * pk;
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
        // Over the faces held and vacant: the zone's area when it is not over-full, their
        // count when it is (a zone's stands can outnumber its faces between rebins), so the
        // shares add to one and no mineral is made.
        let total = (counts.iter().sum::<f64>() + vacant).max(1.0);
        for s in 0..NS {
            self.z[zi].pool[s] += m * counts[s] / total;
        }
        self.z[zi].pool[VACANT] += m * vacant / total;
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
            seed[s] = if self.floats(s) {
                // A water seed lands only on a standing-water margin (`seeds::water_target`,
                // `seeds.rs:1539`): the bank spreads over the faces whose gates pass.
                let g = readings[s].gate;
                if g > 0.0 { g * (1.0 - (-self.z[zi].bank[s] / (a * g)).exp()) } else { 0.0 }
            } else {
                self.seeded(zi, s)
            };
        }
        for (l, share) in [(false, 1.0)] {
            if share <= 0.0 {
                continue;
            }
            let fallback: Vec<(f64, f64)> = (0..NS)
                .map(|s| {
                    let sc = config.species(Species::ALL[s]);
                    (light_response(sc, zone_sky * (self.p.light_k[s][0][0] - self.p.light_k[s][0][1] * cover).max(0.02)), readings[s].mu)
                })
                .collect();
            // Vacant faces: armed and sprouted within two checks; the lottery by weight.
            if vacant > 0.0 {
                let mean = self.z[zi].pool[VACANT] / vacant;
                let mut pres = [0.0; NS];
                let mut cm = [0.0; NS];
                let mut sites = [Site::default(); NS];
                let mut weight = [0.0; NS];
                for s in 0..NS {
                    if seed[s] <= 0.0 {
                        continue;
                    }
                    // A water seed's faces are the margins: the gate-passing sample.
                    let (kind, gate_now) = if self.floats(s) { (SampleKind::Open, 1.0) } else { (SampleKind::Seeded, readings[s].gate) };
                    let (f, m, site) = self.pass_share(zi, s, VACANT, mean, self.p.sigma[VACANT], kind, gate_now, fallback[s]);
                    pres[s] = seed[s] * f;
                    cm[s] = m;
                    sites[s] = site;
                    weight[s] = dens[s] * f64::from(config.species(Species::ALL[s]).seed_mass.max(1)) * f;
                }
                let none: f64 = pres.iter().map(|p| 1.0 - p).product();
                let wsum: f64 = weight.iter().sum();
                // The species' persistence weighs the lottery's rate by its winners' shares.
                let phi = if wsum > 0.0 { (0..NS).map(|s| weight[s] * self.p.establish_phi[s]).sum::<f64>() / wsum } else { 1.0 };
                let lam = phi * (1.0 - none) / (2.0 * check);
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
                        self.add_newborns(zi, s, k, organic, if l { 1.0 } else { 0.0 }, 0.0, sites[s], cm[s]);
                        self.c.births_seed[s] += k;
                        self.c.by_zone[zi][0][s] += k;
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
                    let (f, cm, site) = self.pass_share(zi, s, c, mean, self.p.sigma[c], SampleKind::Seeded, readings[s].gate, fallback[s]);
                    let rho = config.species(Species::ALL[s]).overtop_per_check;
                    let lam = self.p.establish_phi[s] * seed[s] * f * rho / check;
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
                    self.add_newborns(zi, s, k, organic, if l { 1.0 } else { 0.0 }, 0.0, site, cm);
                    self.c.births_seed[s] += k;
                    self.c.by_zone[zi][0][s] += k;
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
            self.add_newborns(zi, GLOW, k, package_of(sc), 0.0, 0.0, Site::default(), 0.0);
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
        self.c.decomposed_carrion += dc;
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
            // The glowcap's place keeps no mineral inventory here ([`Model::mineral`]): its
            // caps' mineral is its own line, off the plant ledger.
            if s == GLOW {
                self.c.cap_mineral_taken += sc.n_tissue * want.min(ed);
            } else {
                self.c.taken_mineral += sc.n_tissue * want.min(ed);
            }
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
            // Each stand's inventory gives up the mineral at its own species' `n_tissue`.
            let nts: [f64; NS] = std::array::from_fn(|s| if s == GLOW { 0.0 } else { self.config.species(Species::ALL[s]).n_tissue });
            for zs in &mut self.z {
                for (s, v) in zs.cohorts.iter_mut().enumerate() {
                    for c in v.iter_mut() {
                        let x = if nectar { &mut c.st.nectar } else { &mut c.st.fruit };
                        let d = *x * f;
                        *x -= d;
                        self.c.taken_mineral += nts[s] * c.n * d;
                    }
                }
            }
            if nectar {
                self.c.taken.nectar += want.min(held);
            } else {
                self.c.taken.fruit += want.min(held);
            }
        }
    }

    // ---- The closed loop's exchange (`design/webmodel/loop.md`). In the closed loop the
    // clamped `take` and `returns` stay empty, so [`Model::step`] runs the plants, the dead
    // pools' decay and the film; the animals then draw on [`Model::offer`], and their take
    // and returns come in through [`Model::take_from`] and [`Model::give`] before the next
    // step, as the sim's withdrawals and deposits fall between plant steps
    // (`cubarium-voxel-sim/src/lib.rs:660-662`, `cubarium-voxel-flora/src/lib.rs:5775-5782`).

    /// A cohort's foliage a mouth can take: what stands above the grazing floor
    /// (`graze_refuge · α · W`, the sum of the layers' floors). The same bound [`Model::offer`]
    /// reports and [`Model::take_from`] draws on, so the take never exceeds the offer.
    pub fn takeable(sc: &cubarium_voxel_flora::SpeciesConfig, st: &StandState) -> f64 {
        (st.foliage - sc.graze_refuge * sc.alpha * st.wood).max(0.0)
    }

    /// **The offer to the animals** (closed loop): world totals of what the animals can take
    /// now, each with the area it is spread over (m², [`crate::exchange::Areas`]), and the
    /// seedlings a browser could take. Foliage per species is [`Model::takeable`] (the
    /// layers' own floors summed; [`Model::supply`] reads the layers one by one, the same
    /// above full floors and up to the floor of a layer under it otherwise).
    pub fn offer(&self) -> Supply {
        let cell = self.config.voxel_m * self.config.voxel_m;
        let mut s = Supply::default();
        s.carrion = self.carrion;
        s.film = (self.film - self.config.film_graze_refuge * self.film_capacity).max(0.0);
        let world: f64 = self.zones.zones.iter().map(|z| z.area).sum::<f64>() * cell;
        s.area.litter = world;
        s.area.carrion = world;
        s.area.film = if self.config.film_capacity_per_m2 > 0.0 { self.film_capacity / self.config.film_capacity_per_m2 } else { 0.0 };
        for zs in &self.z {
            s.litter += zs.litter;
            for k in 0..NS {
                let sc = self.config.species(Species::ALL[k]);
                for c in &zs.cohorts[k] {
                    s.foliage[k] += c.n * Self::takeable(sc, &c.st);
                    s.area.foliage[k] += c.n * cell;
                    s.nectar += c.n * c.st.nectar;
                    s.fruit += c.n * c.st.fruit;
                    if c.st.nectar > 0.0 {
                        s.area.nectar += c.n * cell;
                    }
                    if c.st.fruit > 0.0 {
                        s.area.fruit += c.n * cell;
                    }
                    if k != GLOW && c.st.wood < sc.donor_min {
                        s.seedlings[k] += c.n;
                    }
                }
            }
        }
        s
    }

    /// **The animals' take, applied** (closed loop), spread over the zones pro rata to each
    /// zone's takeable stock of the class (mass-action foraging, the parent's decision 2):
    /// every cohort gives up the same share of its takeable foliage, every zone the same share
    /// of its litter, every stand the same share of its nectar or fruit; the film and the
    /// carrion are one pool each. Seedlings come off the juveniles (`W < donor_min`) in the
    /// same way, whole stands, their tissue to the browser and their mineral to the faces
    /// they free (the `browse=` lever's rule). Returns what was removed (never more than
    /// offered) and the mineral that left in living tissue (foliage, nectar, fruit; the caps'
    /// is booked apart, [`Counters::cap_mineral_taken`]).
    pub fn take_from(&mut self, take: &Take) -> (Take, f64) {
        let nz = self.z.len();
        let mut got = Take::default();
        let mut mineral = 0.0;
        let mut tz = vec![[0.0f64; 2]; nz];
        // Litter, per zone by its share of the pool.
        let total: f64 = self.z.iter().map(|z| z.litter).sum();
        let lt = take.litter.min(total).max(0.0);
        if total > 0.0 && lt > 0.0 {
            for (zi, zs) in self.z.iter_mut().enumerate() {
                let d = lt * zs.litter / total;
                zs.litter -= d;
                tz[zi][0] += d;
            }
        }
        got.litter = lt;
        // Carrion and film: one pool each.
        got.carrion = take.carrion.min(self.carrion).max(0.0);
        self.carrion -= got.carrion;
        got.film = take.film.min((self.film - self.config.film_graze_refuge * self.film_capacity).max(0.0)).max(0.0);
        self.film -= got.film;
        // Foliage and caps per species, every cohort the same share of its takeable foliage.
        for s in 0..NS {
            let want = take.foliage[s];
            if !(want > 0.0) {
                continue;
            }
            let sc = self.config.species(Species::ALL[s]).clone();
            let ed: f64 = self.z.iter().flat_map(|z| z.cohorts[s].iter()).map(|c| c.n * Self::takeable(&sc, &c.st)).sum();
            if !(ed > 0.0) {
                continue;
            }
            let f = (want / ed).min(1.0);
            for (zi, zs) in self.z.iter_mut().enumerate() {
                for c in zs.cohorts[s].iter_mut() {
                    let d = Self::takeable(&sc, &c.st) * f;
                    c.st.foliage -= d;
                    tz[zi][0] += c.n * d;
                    if s != GLOW {
                        tz[zi][1] += sc.n_tissue * c.n * d;
                    }
                }
            }
            let t = want.min(ed);
            got.foliage[s] = t;
            if s == GLOW {
                self.c.cap_mineral_taken += sc.n_tissue * t;
            } else {
                mineral += sc.n_tissue * t;
            }
        }
        // Nectar and fruit, every stand the same share, each at its own `n_tissue`.
        let nts: [f64; NS] = std::array::from_fn(|s| if s == GLOW { 0.0 } else { self.config.species(Species::ALL[s]).n_tissue });
        for (want, nectar) in [(take.nectar, true), (take.fruit, false)] {
            if !(want > 0.0) {
                continue;
            }
            let held: f64 = self.z.iter().flat_map(|z| z.cohorts.iter().flatten()).map(|c| c.n * if nectar { c.st.nectar } else { c.st.fruit }).sum();
            if !(held > 0.0) {
                continue;
            }
            let f = (want / held).min(1.0);
            for (zi, zs) in self.z.iter_mut().enumerate() {
                for (s, v) in zs.cohorts.iter_mut().enumerate() {
                    for c in v.iter_mut() {
                        let x = if nectar { &mut c.st.nectar } else { &mut c.st.fruit };
                        let d = *x * f;
                        *x -= d;
                        tz[zi][0] += c.n * d;
                        tz[zi][1] += nts[s] * c.n * d;
                        mineral += nts[s] * c.n * d;
                    }
                }
            }
            if nectar {
                got.nectar = want.min(held);
            } else {
                got.fruit = want.min(held);
            }
        }
        // Seedlings: whole juveniles, each zone and cohort the same share.
        for s in 0..NS {
            let want = take.seedlings[s];
            if s == GLOW || !(want > 0.0) {
                continue;
            }
            let sc = self.config.species(Species::ALL[s]).clone();
            let juv: f64 = self.z.iter().flat_map(|z| z.cohorts[s].iter()).filter(|c| c.st.wood < sc.donor_min).map(|c| c.n).sum();
            if !(juv > 0.0) {
                continue;
            }
            let q = (want / juv).min(1.0);
            for zi in 0..nz {
                let count = self.z[zi].count(s);
                let (mut gone, mut mat) = (0.0, 0.0);
                for c in self.z[zi].cohorts[s].iter_mut().filter(|c| c.st.wood < sc.donor_min) {
                    let d = c.n * q;
                    c.n -= d;
                    gone += d;
                    mat += d * c.st.material();
                }
                if gone > 0.0 {
                    // The tissue to the browser (booked as a take of the species' tissue and
                    // on its own line); its mineral to the faces it frees.
                    self.c.browsed[s] += gone;
                    self.c.browsed_organic += mat;
                    self.c.taken.foliage[s] += mat;
                    let zs = &mut self.z[zi];
                    zs.pool[s] += sc.n_tissue * mat;
                    let carried = zs.pool[s] * gone / count.max(gone);
                    zs.pool[s] -= carried;
                    zs.pool[VACANT] += carried;
                    tz[zi][0] += mat;
                }
            }
            for zs in &mut self.z {
                zs.cohorts[s].retain(|c| c.n > 1e-12);
            }
            got.seedlings[s] = want.min(juv);
        }
        self.c.taken.litter += got.litter;
        self.c.taken.carrion += got.carrion;
        self.c.taken.film += got.film;
        self.c.taken.nectar += got.nectar;
        self.c.taken.fruit += got.fruit;
        for s in 0..NS {
            self.c.taken.foliage[s] += got.foliage[s];
            self.c.taken.seedlings[s] += got.seedlings[s];
        }
        self.c.taken_mineral += mineral;
        self.c.take_z = tz;
        (got, mineral)
    }

    /// **The animals' returns, applied** (closed loop): dung to the litter and its mineral to
    /// the site pools where the last take came from (by each zone's share of the organic and
    /// of the mineral taken; by area when nothing was taken), within a zone by face count
    /// (`spread_mineral`); corpses to the carrion pool. The sim lays each deposit on the face
    /// the animal stands on (`Flora::deposit`, `lib.rs:5783`), and the animals stand where
    /// they feed.
    pub fn give(&mut self, ret: &Returns) {
        let nz = self.z.len();
        let area: Vec<f64> = self.zones.zones.iter().map(|z| z.area).collect();
        let za: f64 = area.iter().sum::<f64>().max(1.0);
        let share = |k: usize, tz: &[[f64; 2]]| -> Vec<f64> {
            let t: f64 = tz.iter().map(|v| v[k]).sum();
            if t > 0.0 { tz.iter().map(|v| v[k] / t).collect() } else { area.iter().map(|a| a / za).collect() }
        };
        let tz = self.c.take_z.clone();
        if ret.litter > 0.0 {
            let w = share(0, &tz);
            for zi in 0..nz {
                self.z[zi].litter += ret.litter * w[zi];
            }
        }
        if ret.mineral > 0.0 {
            let has_m = tz.iter().any(|v| v[1] > 0.0);
            let w = share(if has_m { 1 } else { 0 }, &tz);
            for zi in 0..nz {
                self.spread_mineral(zi, ret.mineral * w[zi]);
            }
        }
        self.carrion += ret.carrion.max(0.0);
        self.c.returned += ret.litter + ret.carrion;
        self.c.returned_mineral += ret.mineral;
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
        self.c.film_grown += (grow * dt).max(0.0);
        self.c.film_upkeep += upkeep * dt;
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

    /// The weather state now (replay), 0 when frozen.
    pub fn weather_state(&self) -> usize {
        self.weather_ix
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

    /// The germination funnel the model reads now for species `s` (summed over zones, in
    /// faces): `[class][faces, seeded, gates passed, canopy passed]` for bare (0) and the
    /// covers it may come up through (1) — the counterpart of the driver's `elig`.
    pub fn funnel(&self, s: usize) -> [[f64; 4]; 2] {
        let mut out = [[0.0; 4]; 2];
        if s == GLOW || !is_photo(&self.config, s) {
            return out;
        }
        let sp = Species::ALL[s];
        let sc = self.config.species(sp);
        for zi in 0..self.z.len() {
            let a = self.zones.zones[zi].area.max(1.0);
            let r = self.reading(zi, s);
            let seeded = self.seeded(zi, s);
            let counts: Vec<f64> = (0..NS).map(|k| self.z[zi].count(k)).collect();
            let vacant = (a - (0..NS).filter(|&k| k != GLOW).map(|k| counts[k]).sum::<f64>()).max(0.0);
            let fb = (light_response(sc, self.zones.zones[zi].sky * (self.p.light_k[s][0][0] - self.p.light_k[s][0][1] * self.z[zi].cover).max(0.02)), r.mu);
            let mut add = |cls: usize, n: f64, o: usize| {
                if n <= 0.0 {
                    return;
                }
                let f = self.pass_share(zi, s, o, self.z[zi].pool[o] / n, self.p.sigma[o], SampleKind::Seeded, r.gate, fb).0;
                let g = self.pass_share(zi, s, o, 1e9, 0.0, SampleKind::Seeded, r.gate, fb).0;
                out[cls][0] += n;
                out[cls][1] += n * seeded;
                out[cls][2] += n * seeded * g;
                out[cls][3] += n * seeded * f;
            };
            add(0, vacant, VACANT);
            for c in 0..NS {
                if c != GLOW && may_overtop(&self.config, sp, Species::ALL[c]) {
                    add(1, counts[c], c);
                }
            }
        }
        out
    }

    /// One zone's funnel for species `s` now, in faces: seeded open faces (or, `seed` false,
    /// open faces in runner reach, as a share of the takeable count), those whose gates
    /// pass, those that pass the canopy gate too.
    pub fn zone_funnel(&self, zi: usize, s: usize, seed: bool) -> [f64; 3] {
        let sp = Species::ALL[s];
        let sc = self.config.species(sp);
        let a = self.zones.zones[zi].area.max(1.0);
        let r = self.reading(zi, s);
        let counts: Vec<f64> = (0..NS).map(|k| self.z[zi].count(k)).collect();
        let vacant = (a - (0..NS).filter(|&k| k != GLOW).map(|k| counts[k]).sum::<f64>()).max(0.0);
        let fb = (light_response(sc, self.zones.zones[zi].sky * self.p.light_k[s][0][0]), r.mu);
        let kind = if seed { SampleKind::Seeded } else { SampleKind::Runner };
        let w = if seed { self.seeded(zi, s) } else { 1.0 };
        let mut out = [0.0; 3];
        for o in (0..=NS).filter(|&o| o == VACANT || (o != GLOW && if seed { may_overtop(&self.config, sp, Species::ALL[o]) } else { runner_may_take(&self.config, sp, Species::ALL[o]) })) {
            let cnt = if o == VACANT { vacant } else { counts[o] };
            if cnt <= 0.0 {
                continue;
            }
            out[0] += cnt * w;
            out[1] += cnt * w * self.pass_share(zi, s, o, 1e9, 0.0, kind, r.gate, fb).0;
            out[2] += cnt * w * self.pass_share(zi, s, o, self.z[zi].pool[o] / cnt, self.p.sigma[o], kind, r.gate, fb).0;
        }
        out
    }

    /// The runner funnel for species `s` now: `[faces of the classes it may take, takeable
    /// (gates and canopy passing), Σ over zones of donors · pt]`, `pt` the chance a runner
    /// draw finds a face.
    pub fn runner_funnel(&self, s: usize) -> [f64; 4] {
        let mut out = [0.0; 4];
        let sp = Species::ALL[s];
        let sc = self.config.species(sp);
        if sc.clonal_share <= 0.0 || s == GLOW {
            return out;
        }
        for zi in 0..self.z.len() {
            let a = self.zones.zones[zi].area.max(1.0);
            let r = self.reading(zi, s);
            let counts: Vec<f64> = (0..NS).map(|k| self.z[zi].count(k)).collect();
            let vacant = (a - (0..NS).filter(|&k| k != GLOW).map(|k| counts[k]).sum::<f64>()).max(0.0);
            let l_eff = light_response(sc, self.zones.zones[zi].sky * self.p.light_k[s][0][0]);
            let mut takeable = 0.0;
            for o in (0..=NS).filter(|&o| o == VACANT || (o != GLOW && runner_may_take(&self.config, sp, Species::ALL[o]))) {
                let cnt = if o == VACANT { vacant } else { counts[o] };
                if cnt <= 0.0 {
                    continue;
                }
                let f = self.pass_share(zi, s, o, self.z[zi].pool[o] / cnt, self.p.sigma[o], SampleKind::Runner, r.gate, (l_eff, r.mu)).0 * self.zones.zones[zi].box_enrich[s][o];
                if std::env::var("WM_RF").is_ok() && cnt * f > 1.0 {
                    eprintln!("  rf z{zi} {} o={} cnt={cnt:.0} mean={:.2} f={f:.3}", self.zones.zones[zi].key.name(), if o == VACANT { "vacant" } else { Species::ALL[o].name() }, self.z[zi].pool[o] / cnt);
                }
                out[0] += cnt;
                takeable += cnt * f;
            }
            out[1] += takeable;
            let theta = (self.p.psi_runner[s] * takeable / a).clamp(0.0, 1.0);
            let rb = self.runner_box[s];
            let pt = 1.0 - (1.0 - theta).powf(rb.faces * rb.same_zone);
            let donors = self.count_at_least(zi, s, sc.donor_min);
            out[2] += donors * pt;
            out[3] += donors;
        }
        out
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
