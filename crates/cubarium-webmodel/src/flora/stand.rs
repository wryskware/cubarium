//! **One stand's books**: the sim's per-stand growth law, written down here because its
//! entry point (`step::grow_stand_cause`, `crates/cubarium-voxel-flora/src/step.rs:2924`)
//! takes crate-private types. Every line cites the sim's line; the organism budget itself
//! ([`cubarium_voxel::organism::Budget`]) is the sim's own, called.
//!
//! What is **not** carried: the per-layer foliage stocks (`Stand::layer_stock`; they decide
//! which tissue a bite or senescence takes, not how much), the stand's mineral inventory
//! (income reads the site's pool, `lib.rs:319`), induced defence (the animal half's bite
//! factor), and the seed-bank lottery (the occupancy model's). The site mineral is a held
//! number. Law anchors (`tests/flora_laws.rs`) step this against `study::probe`, which runs
//! the sim's own `grow_stand` on a one-stand fixture.

use cubarium_voxel::organism::{Budget, settle_mineral, tissue_funded};
use cubarium_voxel_flora::{CropPhase, CureRead, CureStep, SpeciesConfig, Trophic, cure_step, pollinated_share};

use super::climate::Clock;

/// A stand's state: `W`, `P`, `Q`, the propagule parcel, and the clocks the law keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StandState {
    pub wood: f64,
    pub foliage: f64,
    pub reserve: f64,
    pub parcel: f64,
    pub aeration_stress: f64,
    pub cured: bool,
    pub dry_s: f64,
    pub nectar: f64,
    pub fruit: f64,
    pub fruit_age_s: f64,
    pub fruit_rest_s: f64,
    /// The crop's cycle (`Stand::crop`, FLOWER-FRUIT): its phase, the flowering's time left,
    /// the pollinator visits (here a mean over the cohort, booked from the nectar taken,
    /// [`StandState::book_nectar_taken`]) and the fruit the set still has to make.
    pub crop_phase: CropPhase,
    pub flower_left_s: f64,
    pub visits: f64,
    pub to_make: f64,
    /// Runner support left, seconds (`Stand::support`, `step.rs:2525`); 0 is none.
    pub support_s: f64,
}

impl StandState {
    /// The stand one package of `organic` builds (`seeds::newborn_stocks`, `seeds.rs:163`).
    pub fn newborn(sc: &SpeciesConfig, organic: f64) -> StandState {
        let scaled = sc.propagule_split[0] * organic;
        let wood = if scaled > sc.alive_min * (1.0 + 1e-9) { scaled } else { sc.alive_min }.min(organic.max(0.0));
        let left = (organic - wood).max(0.0);
        let foliage = (sc.propagule_split[1] * organic).clamp(0.0, left);
        StandState {
            wood,
            foliage,
            reserve: left - foliage,
            ..StandState::default()
        }
    }

    /// **Visits from the nectar taken** (FLOWER-FRUIT): the web model has no bodies to count,
    /// so a flowering stand drunk of `taken` nectar is booked `pollination_visits · taken /
    /// nectar_cap` visits: a flowering's whole nectar drunk is a fully pollinated crop. A
    /// stand not in flower books none.
    pub fn book_nectar_taken(&mut self, sc: &SpeciesConfig, taken: f64) {
        if self.crop_phase == CropPhase::Flower && sc.nectar_cap > 0.0 && taken > 0.0 {
            let per = if sc.pollination_visits > 0.0 { sc.pollination_visits } else { 1.0 };
            self.visits += per * taken / sc.nectar_cap;
        }
    }

    /// `W + P + Q + parcel + nectar + fruit` (`Stand::material`, `lib.rs:511`).
    pub fn material(&self) -> f64 {
        self.wood + self.foliage + self.reserve + self.parcel + self.nectar + self.fruit
    }
}

/// The package one seed is (`seeds::package_of`, `seeds.rs:61`).
pub fn package_of(sc: &SpeciesConfig) -> f64 {
    let w = sc.propagule_split[0];
    if w > 0.0 { f64::from(sc.seed_mass.max(1)) * (sc.alive_min / w) } else { 0.0 }
}

/// What a stand reads from its site this step (`step::Drink`, the light sweep, the pool).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    /// `L_eff`, after the species' light response.
    pub light_eff: f64,
    /// Root-box mean available water (the cure clock reads it; `μ` is its ramp).
    pub available: f64,
    /// Root-box saturated fraction (aeration stress closes on it).
    pub saturated: f64,
    /// The site's mineral pool `N`.
    pub mineral: f64,
    /// The glowcap's darkness (its seep).
    pub darkness: f64,
    /// A fungus's substrate income this step: organic matter taken and its yield (`feed`).
    pub substrate_taken: f64,
    pub substrate_gross: f64,
}

/// What one step moved, organic units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flows {
    /// Gross fixation (a plant) or substrate income kept plus seep (a fungus).
    pub income: f64,
    pub respired: f64,
    /// Foliage built (`FloraLedger::foliage_built`).
    pub built: f64,
    /// To the litter: senescence, a cure's drop, fallen fruit.
    pub litter: f64,
    /// To the dead wood: dieback.
    pub dead_wood: f64,
    /// Seep organic matter in (a fungus in the dark).
    pub seep: f64,
}

/// Why a stand died in a step (the sim's `DeathCause`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Death {
    Starved,
    Drought,
}

/// `L_eff = L (1 + h) / (L + h)` (`step::light_response`, `step.rs:1206`).
pub fn light_response(sc: &SpeciesConfig, l: f64) -> f64 {
    let half = sc.light_half.max(0.0);
    if l <= 0.0 {
        return 0.0;
    }
    if l + half <= 0.0 {
        return 1.0;
    }
    l * (1.0 + half) / (l + half)
}

/// The share `1 − exp(−rate · dt)` (`step::decay_share`, `step.rs:224`).
pub fn decay_share(rate: f64, dt: f64) -> f64 {
    if rate > 0.0 && dt > 0.0 { 1.0 - (-rate * dt).exp() } else { 0.0 }
}

/// The aeration stress level a saturated fraction asks for (`step::aeration_target`,
/// `step.rs:2022`).
pub fn aeration_target(saturated: f64, sc: &SpeciesConfig) -> f64 {
    let lo = sc.establish_saturated_max;
    if lo >= 1.0 {
        return 0.0;
    }
    ((saturated - lo) / (1.0 - lo)).clamp(0.0, 1.0)
}

/// `N / (N + K_N)` (`step::monod`, `step.rs:3741`).
pub fn monod(sc: &SpeciesConfig, n: f64) -> f64 {
    if n + sc.nutrient_half > 0.0 { n / (n + sc.nutrient_half) } else { 0.0 }
}

/// **The longest step the stand law is taken in**, seconds (numerics, not ecology). The law is
/// the sim's explicit per-tick update; taken whole at the model's 10 s step its error grows
/// in proportion to the step and always runs low (the independent test pass: a dry adult
/// springturf's foliage −56 % over a day at 10 s, −8 % at 1 s, −1.3 % at 0.2 s). [`grow`]
/// sub-steps a longer step at this length, the clock's light, upkeep and night held over it
/// and its time to dawn counted down.
pub const SUB_STEP_S: f64 = 0.25;

/// The crop sets at `size`: the fruit phase, its clock at zero (`step::set_crop`).
fn set_crop(st: &mut StandState, size: f64) {
    st.crop_phase = CropPhase::Fruit;
    st.flower_left_s = 0.0;
    st.to_make = size.max(0.0);
    st.fruit_age_s = 0.0;
}

/// **One step** of one stand: the growth law (`step::grow_stand_cause`, `step.rs:2924–3260`),
/// then the donor's saving into its parcel (`seeds::propagate`, `seeds.rs:880–905`, [`save`]),
/// its construction respiration in the flows; sending whole packages is [`fund`]. The
/// fruit-rest draw is the mean of the sim's exponential (`step.rs:3201`), not a keyed draw.
/// A step longer than [`SUB_STEP_S`] is taken in equal sub-steps no longer than it, growth
/// and saving in each (the saving's cadence matters as much as the law's: saved once per
/// 10 s, a newborn springturf ends a day with 22 % less reserve); a death ends it.
pub fn grow(sc: &SpeciesConfig, st: &mut StandState, site: Site, c: Clock) -> (Flows, Option<Death>) {
    let n = (c.dt / SUB_STEP_S).ceil().max(1.0) as usize;
    if n <= 1 {
        let (mut f, d) = grow_once(sc, st, site, c);
        if d.is_none() {
            f.respired += save(sc, st, c.dt);
        }
        return (f, d);
    }
    let h = c.dt / n as f64;
    // A fungus's substrate is an amount for the whole step: each sub-step takes its share.
    let site = Site { substrate_taken: site.substrate_taken / n as f64, substrate_gross: site.substrate_gross / n as f64, ..site };
    let mut out = Flows::default();
    for k in 0..n {
        let ck = Clock { dt: h, to_dawn_s: c.to_dawn_s.map(|t| (t - k as f64 * h).max(h)), ..c };
        let (f, d) = grow_once(sc, st, site, ck);
        let saved = if d.is_none() { save(sc, st, h) } else { 0.0 };
        out.respired += saved;
        out.income += f.income;
        out.respired += f.respired;
        out.built += f.built;
        out.litter += f.litter;
        out.dead_wood += f.dead_wood;
        out.seep += f.seep;
        if d.is_some() {
            // The substrate the step drew for the sub-steps it did not live through is still
            // gone from the pools: respired, as the step's unkept substrate is.
            out.respired += site.substrate_taken * (n - k - 1) as f64;
            return (out, d);
        }
    }
    (out, None)
}

fn grow_once(sc: &SpeciesConfig, st: &mut StandState, site: Site, c: Clock) -> (Flows, Option<Death>) {
    let dt = c.dt;
    let mut out = Flows::default();
    let mu = sc.moisture_at(site.available);
    let (w0, p0, q0) = (st.wood, st.foliage, st.reserve);
    // aeration stress (step.rs:2958)
    let target = aeration_target(site.saturated, sc);
    let was = st.aeration_stress;
    let stress = if target > was {
        (was + decay_share(sc.stress_rate_per_s, dt) * (target - was)).clamp(0.0, 1.0)
    } else if target < was {
        (was - decay_share(sc.relax_rate_per_s, dt) * (was - target)).clamp(0.0, 1.0)
    } else {
        was
    };
    st.aeration_stress = stress;
    // 4.0 curing (step.rs, `grow_stand_cause` §4.0): the sim's own clock, `cure_step`, on
    // the budget (`water_short`, DORMANCY) as well as below `cure_water`.
    if sc.cure_after_s > 0.0 && sc.trophic == Trophic::Photo {
        let read = CureRead {
            available: site.available,
            moisture: mu,
            light_eff: site.light_eff,
            stress,
            mineral: site.mineral,
            wood: st.wood,
            foliage: st.foliage,
        };
        match cure_step(sc, st.cured, &mut st.dry_s, &read, dt) {
            CureStep::Green => {}
            CureStep::Wake => st.cured = false,
            CureStep::Dormant => return dormant(sc, st, c.upkeep * c.night, dt, out),
            CureStep::Cure => {
                let shed = (sc.cure_shed.clamp(0.0, 1.0) * st.foliage).max(0.0);
                st.foliage -= shed;
                out.litter += shed;
                st.cured = true;
                st.dry_s = 0.0;
                return dormant(sc, st, c.upkeep * c.night, dt, out);
            }
        }
    }
    // 4.1 income, 4.2 demands (step.rs:3002)
    let n0 = site.mineral;
    let p_cap = sc.alpha * w0;
    let q_max = sc.reserve_cap * w0;
    let night = match sc.trophic {
        Trophic::Photo => c.night,
        Trophic::Saprotroph => 1.0,
    };
    let m = sc.maintenance * c.upkeep * night * w0 * dt;
    let d_p = (p_cap - p0).max(0.0).min(sc.foliage_rate * w0 * dt);
    let d_w = (sc.wood_max - w0).max(0.0).min(sc.wood_rate * w0 * dt);
    let d_q = (q_max - q0).max(0.0);
    let (a, arrived_mineral) = match sc.trophic {
        Trophic::Photo => {
            let a = (sc.assimilation * site.light_eff * p0 * c.daylight * mu * (1.0 - stress) * monod(sc, n0) * dt)
                .max(0.0)
                .min(sc.nutrient_draw_max * n0 * dt)
                .min(tissue_funded(n0, sc.n_tissue))
                .max(0.0)
                .min(m + (1.0 + sc.build) * (d_p + d_w) + d_q);
            (a, 0.0)
        }
        Trophic::Saprotroph => {
            // saprotrophy (step.rs:2898): the yield is income, the rest respired at once.
            let gross = site.substrate_gross.max(0.0).min(site.substrate_taken);
            out.respired += (site.substrate_taken - gross).max(0.0);
            let spend = m + (1.0 + sc.build) * (d_p + d_w) + d_q;
            let seep = (if sc.substrate_required { 0.0 } else { sc.dark_uptake_per_s * w0 * site.darkness * mu * dt }).max(0.0).min((spend - gross).max(0.0));
            out.seep = seep;
            // The substrate's own mineral at the substrate's density (the pool's n_tissue);
            // the seep brings n_tissue per unit.
            (gross + seep, sc.n_tissue * (gross + seep))
        }
    };
    out.income = a;
    // 4.3 maintenance, 4.4a mineral budget (step.rs:3069)
    let mut budget = Budget::new(a, tissue_funded(arrived_mineral + n0.max(0.0), sc.n_tissue));
    let unpaid = budget.upkeep(&mut st.reserve, m).unpaid;
    // runner support (step.rs:2525): the parent pays the daughter's shortfall; here the
    // shortfall is met at once while support lasts (the parent is above its floor).
    let unpaid = if st.support_s > 0.0 {
        st.support_s -= dt;
        0.0
    } else {
        unpaid
    };
    // 4.4 growth (step.rs:3075)
    let dq_s = budget.store((sc.reserve_share * budget.income).min(d_q));
    st.reserve += dq_s;
    let dp_a = budget.build(d_p, sc.build);
    let mut night_left = match (sc.trophic, c.to_dawn_s) {
        (Trophic::Photo, Some(t)) => {
            let hold = m / dt * t + sc.night_floor * q_max;
            Some((st.reserve - hold).max(0.0) / t.max(sc.night_min_s) * dt)
        }
        _ => None,
    };
    let flush_light = match (sc.trophic, night_left) {
        (Trophic::Photo, None) => c.daylight,
        _ => 1.0,
    };
    let flush_to = sc.reflush_below * sc.reflush_cap(w0);
    let dp_q = if p0 < flush_to {
        let mut want = ((d_p - dp_a) * flush_light).min((flush_to - p0 - dp_a).max(0.0));
        if let Some(left) = night_left {
            want = want.min(left / (1.0 + sc.build));
        }
        let x = budget.build_from(&mut st.reserve, want, sc.build);
        if let Some(left) = night_left.as_mut() {
            *left = (*left - (1.0 + sc.build) * x).max(0.0);
        }
        x
    } else {
        0.0
    };
    st.foliage += dp_a + dp_q;
    out.built = dp_a + dp_q;
    let dw = budget.build(d_w, sc.build);
    st.wood += dw;
    let dq_r = budget.store((d_q - dq_s).max(0.0));
    st.reserve += dq_r;
    out.respired += budget.close();
    // 4.4b the crop opens, 4.4c nectar, 4.4d the set, 4.4e fruit (step.rs, FLOWER-FRUIT)
    let adult = sc.profile_index(st.wood) >= 1;
    if st.fruit_rest_s > 0.0 {
        st.fruit_rest_s = (st.fruit_rest_s - dt).max(0.0);
    } else if st.crop_phase == CropPhase::Idle && sc.fruit_per_s > 0.0 && adult {
        if sc.flower_s > 0.0 {
            st.crop_phase = CropPhase::Flower;
            st.flower_left_s = sc.flower_s;
            st.visits = 0.0;
        } else {
            set_crop(st, sc.fruit_cap.max(0.0));
        }
    }
    let open = sc.flower_s <= 0.0 || st.crop_phase == CropPhase::Flower;
    if open && sc.nectar_per_s > 0.0 && (c.daylight > 0.0 || night_left.is_some()) && adult {
        let room = (sc.nectar_cap - st.nectar).max(0.0);
        let rate = if night_left.is_some() { sc.nectar_per_s * sc.night_bloom_scale } else { sc.nectar_per_s };
        let make = (rate * dt).min(room).min(st.reserve.max(0.0)).min(night_left.unwrap_or(f64::INFINITY));
        if make > 0.0 {
            st.reserve -= make;
            st.nectar += make;
            if let Some(left) = night_left.as_mut() {
                *left = (*left - make).max(0.0);
            }
        }
    }
    if st.crop_phase == CropPhase::Flower {
        st.flower_left_s -= dt;
        if st.flower_left_s <= 0.0 {
            st.reserve += std::mem::replace(&mut st.nectar, 0.0);
            let share = pollinated_share(sc.pollination_floor, sc.pollination_visits, st.visits);
            set_crop(st, share * sc.fruit_cap.max(0.0));
        }
    }
    if st.crop_phase == CropPhase::Fruit {
        let cost = sc.fruit_cost.max(1.0);
        if st.to_make > 0.0 && (c.daylight > 0.0 || night_left.is_some()) && adult {
            let rate = if night_left.is_some() { sc.fruit_per_s * sc.night_bloom_scale } else { sc.fruit_per_s };
            let make = (rate * dt).min(st.to_make).min(st.reserve.max(0.0) / cost).min(night_left.unwrap_or(f64::INFINITY) / cost).max(0.0);
            if make > 0.0 {
                st.reserve -= cost * make;
                st.fruit += make;
                st.to_make -= make;
                if st.to_make <= 1e-15 {
                    st.to_make = 0.0;
                }
                out.respired += (cost - 1.0) * make;
            }
        }
        st.fruit_age_s += dt;
        let ended = if st.fruit_age_s >= sc.fruit_drop_s {
            out.litter += std::mem::replace(&mut st.fruit, 0.0);
            true
        } else {
            !(st.fruit > 0.0) && !(st.to_make > 0.0)
        };
        if ended {
            st.crop_phase = CropPhase::Idle;
            st.to_make = 0.0;
            st.fruit_age_s = 0.0;
            st.fruit_rest_s = sc.fruit_rest_s.max(0.0);
        }
    }
    // mineral settling changes no organic stock here (step.rs:3213); the site pool is held.
    let _ = settle_mineral(arrived_mineral, sc.n_tissue * (dq_s + dp_a + dw + dq_r), n0);
    // 4.5 senescence (step.rs:3219)
    let shed = (sc.senescence * night * st.foliage * dt).min(st.foliage);
    st.foliage -= shed;
    out.litter += shed;
    // 4.6 dieback (step.rs:3242)
    let die_back = st.wood.min(sc.dieback * unpaid);
    st.wood -= die_back;
    if sc.trophic == Trophic::Saprotroph && sc.substrate_required { out.litter += die_back; }
    else { out.dead_wood += die_back; }
    // 4.7 death (step.rs:3260)
    let death = (st.wood < sc.alive_min).then_some(Death::Starved);
    (out, death)
}

/// A cured stand's step (`step::dormant`, `step.rs:3293`).
fn dormant(sc: &SpeciesConfig, st: &mut StandState, scale: f64, dt: f64, mut out: Flows) -> (Flows, Option<Death>) {
    let m = (sc.maintenance * scale * sc.cure_upkeep * st.wood * dt).max(0.0);
    let mut budget = Budget::new(0.0, 0.0);
    let unpaid = budget.upkeep(&mut st.reserve, m).unpaid;
    out.respired += budget.close();
    let die_back = st.wood.min(sc.dieback * unpaid);
    st.wood -= die_back;
    out.dead_wood += die_back;
    st.dry_s += dt;
    // A dormant stand that runs out of stores died of the drought (step.rs, `dormant`).
    let death = if st.wood < sc.alive_min || (sc.cure_max_s > 0.0 && st.dry_s >= sc.cure_max_s) {
        Some(Death::Drought)
    } else {
        None
    };
    (out, death)
}

/// **The donor's saving rule** (`seeds::propagate`, `seeds.rs:880–905`): a stand at or
/// over `donor_min` saves `propagule_rate · dt` (times its caps' fullness for a fungus,
/// `seeds.rs:815`) out of its reserve above the floor into its parcel, at `1 + c_g`.
/// Returns the construction respiration. [`grow`] runs it after each (sub-)step.
pub fn save(sc: &SpeciesConfig, st: &mut StandState, dt: f64) -> f64 {
    if st.cured {
        return 0.0;
    }
    let mut respired = 0.0;
    if st.wood >= sc.donor_min {
        let s = sc.propagule_cap_share.clamp(0.0, 1.0);
        let full = sc.alpha * st.wood;
        let fullness = if s > 0.0 && full > 0.0 { 1.0 - s + s * (st.foliage / full).clamp(0.0, 1.0) } else { 1.0 };
        let ask = (sc.propagule_rate * dt * fullness).max(0.0);
        let floor = sc.donor_reserve_floor * sc.reserve_cap * st.wood;
        let take = (st.reserve - floor).max(0.0).min(ask).min(st.reserve.max(0.0));
        if take > 0.0 {
            let net = take / (1.0 + sc.build);
            st.reserve -= take;
            st.parcel += net;
            respired += take - net;
        }
    }
    respired
}

/// **Whole packages leave** (`seeds.rs:905`): the parcel [`grow`] saved, sent a package at a
/// time. Returns the packages sent and the construction respiration (none here: it was booked
/// as [`grow`] saved; the pair is kept for the callers).
pub fn fund(sc: &SpeciesConfig, st: &mut StandState, _dt: f64) -> (u32, f64) {
    if st.cured {
        return (0, 0.0);
    }
    let respired = 0.0;
    let package = package_of(sc);
    let mut sent = 0;
    // The sim sends at most one package a tick; the model's longer step may hold several.
    while package > 0.0 && st.parcel >= package {
        st.parcel -= package;
        sent += 1;
    }
    (sent, respired)
}
