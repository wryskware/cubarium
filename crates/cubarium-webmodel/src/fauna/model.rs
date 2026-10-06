//! **The stage-structured animal model**: every lineage as cohorts of identical bodies
//! (de Roos's escalator boxcar train, the individual-based limit of the stage-structured
//! biomass model of de Roos et al. 2008), each cohort stepping the sim's own individual
//! budget ([`super::laws`]). Stages are read off the state: eggs (and the lanternjaw's
//! dormant bank), juveniles under `birth_body`, adults at it, riders on a host.
//!
//! **Aggregation, deliberately.** A cohort is a set of bodies born or founded together and
//! identical from then on: what space does to them — finding food, a crowded nest, a
//! hunter's encounter — enters as fitted rates ([`super::fits`]) against world stocks and
//! counts. What this loses is the spread *within* a cohort: the body that found the litter
//! bed and the one that did not. Food enters as world totals (Supply), so patchiness and
//! local depletion are inside the fitted response.
//!
//! One code path runs both the **mean field** (`Draws::Mean`: cohort sizes are reals, every
//! event its expectation) and the **stochastic twin** (`Draws::Twin`: integer bodies, every
//! death, refusal and hatch a draw), so the twin is never a second model.

use cubarium_voxel::weather::{DayClock, DayMode, Sun};
use cubarium_voxel_fauna::{Activity, BirthMode, FaunaConfig, Food, Founder, FounderPhysiology, SurplusWindow, TICK_HZ};
use cubarium_voxel_flora::Species;

use super::fits::Fits;
use super::laws::{self, Body, Flows};
use super::rng::Draws;
use crate::exchange::{Returns, Supply, Take};

/// How a body left (the sim's `Departure`, `lib.rs:2130-2147`, less `Removed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Starved,
    Drowned,
    Aged,
    Killed(Founder),
}

/// A set of identical bodies.
#[derive(Clone, Debug)]
pub struct Cohort {
    /// Bodies (real in the mean field, whole in the twin).
    pub n: f64,
    pub b: Body,
    pub age_s: f64,
    /// A founder introduced into the world: its lifespan is the arriving draw.
    pub arriving: bool,
    /// Hatched or born in the world (the replacement readouts count only these).
    pub native: bool,
    /// A parasite hatchling on a host of this lineage (`attached`).
    pub host: Option<Founder>,
    /// Booked as grown through `birth_body`.
    pub matured: bool,
    /// Booked as having laid or borne its first young.
    pub bred: bool,
    pub refractory_s: f64,
    pub retry_s: f64,
    pub clutch_wait_s: f64,
    pub window: SurplusWindow,
    /// An open gestation's escrow, organic units, and how long it has run.
    pub escrow: Option<(f64, f64)>,
    /// **A hunter's fresh kills**, organic units, all its bodies together: the corpses at
    /// its mouth, which it alone eats (at a full bite) while it stays, which decay at the
    /// carrion rate, and which it leaves at `1 / carcass_stay_s` to the carrion pool.
    pub carcass: f64,
}

impl Cohort {
    /// `n` bodies of `body` and `reserve`, nothing else.
    pub fn new(n: f64, body: f64, reserve: f64) -> Cohort {
        Cohort {
            n,
            b: Body { body, reserve, ..Body::default() },
            age_s: 0.0,
            arriving: false,
            native: false,
            host: None,
            matured: false,
            bred: false,
            refractory_s: 0.0,
            retry_s: 0.0,
            clutch_wait_s: 0.0,
            window: SurplusWindow::default(),
            escrow: None,
            carcass: 0.0,
        }
    }

    /// `n` founders arriving at the sim's arrival stores (`FoundingConfig::arrival_body`,
    /// `arrival_reserve`): fractions of `body_max` and of the reserve cap.
    pub fn founders(cfg: &FaunaConfig, f: Founder, n: f64) -> Cohort {
        let sc = cfg.founder(f).core;
        let body = cfg.founding.arrival_body * sc.body_max;
        let mut c = Cohort::new(n, body, cfg.founding.arrival_reserve * sc.reserve_of(body));
        c.arriving = true;
        c.matured = body >= sc.birth_body;
        c
    }
}

/// A clutch, or the lanternjaw's dormant bank eggs laid together.
#[derive(Clone, Debug)]
pub struct EggCohort {
    pub lineage: Founder,
    pub n: f64,
    /// Organic matter in one egg.
    pub organic: f64,
    pub age_s: f64,
    pub dormant: bool,
}

/// One lineage's running ledger: the sim's `FaunaLedger` lines by founder, and the
/// departures by cause.
#[derive(Clone, Debug, Default)]
pub struct LineageLedger {
    pub flows: Flows,
    pub swallowed: [f64; Food::COUNT],
    pub eggs_paid: f64,
    pub clutches: f64,
    pub laid: f64,
    pub eggs_lost: f64,
    pub hatched: f64,
    pub born: f64,
    pub matured: f64,
    pub maturation_age_s: f64,
    pub first_clutches: f64,
    pub first_clutch_age_s: f64,
    pub nest_refusals: f64,
    pub starved: f64,
    pub drowned: f64,
    pub aged: f64,
    /// Bodies of this lineage killed, by the killer's lineage.
    pub killed_by: [f64; Founder::COUNT],
    /// As a hunter: kills made and strikes thrown.
    pub kills: f64,
    pub strikes: f64,
    pub strike_cost: f64,
    pub bank_died: f64,
    /// What the dormant bank respired (`egg_upkeep`).
    pub bank_upkeep: f64,
    pub riders_attached: f64,
    pub dropoffs: f64,
    /// Organic matter a rider took out of hosts (on the rider's line) and lost to riders
    /// (on the host's line).
    pub rider_take: f64,
    pub rider_loss: f64,
    pub gestation_loss: f64,
    /// Body-seconds alive and awake (free bodies), and body-seconds of structure alive: the
    /// sim's `alive_ticks_by_founder`, `awake_ticks_by_founder` over 20 Hz, and its
    /// body-days readout.
    pub alive_s: f64,
    pub awake_s: f64,
    pub body_s: f64,
    /// As a hunter: hunting body-seconds (awake, free, eating latch on).
    pub hunting_s: f64,
}

/// What a lineage is now: the census readouts the sim's study examples print.
#[derive(Clone, Copy, Debug, Default)]
pub struct Census {
    pub n: f64,
    pub adults: f64,
    pub juveniles: f64,
    pub riders: f64,
    pub eggs: f64,
    pub bank: f64,
    pub mean_body: f64,
    pub mean_fullness: f64,
    pub biomass: f64,
    pub reserve: f64,
}

/// The model's whole state and its numbers.
pub struct FaunaModel {
    pub config: FaunaConfig,
    /// The carrion pool's decomposition rate, per second (`flora.carrion_decomposition`): the
    /// fresh-kill carcass decays at it.
    pub carrion_decomposition: f64,
    pub day: DayClock,
    pub fits: Fits,
    /// Seconds since the world was founded (tick 0, the clock's `start_hour`).
    pub t_s: f64,
    pub dt: f64,
    pub pops: [Vec<Cohort>; Founder::COUNT],
    pub eggs: Vec<EggCohort>,
    /// Organic matter the hunters' kills laid at their mouths ([`Cohort::carcass`]), what
    /// decayed there, and what they left to the carrion pool (returned as carrion).
    pub carcass_laid: f64,
    pub carcass_decayed: f64,
    pub carcass_left: f64,
    /// Organic matter of kills beyond the hunter's reach, returned as carrion at the kill.
    pub carcass_out: f64,
    pub ledger: [LineageLedger; Founder::COUNT],
    pub draws: Draws,
    /// Lineages the model steps; the others are held as they are.
    pub enabled: [bool; Founder::COUNT],
    /// Bodies of a lineage taken as present without being stepped (a probe's neighbours): they
    /// count in its interference and its nest-site count.
    pub extra_bodies: [f64; Founder::COUNT],
    /// **Encounter scale**: the hunt's kill rates are per prey body on the floor they were
    /// fitted on; on a floor this many times larger a hunter meets each prey body this much
    /// less often (mass action over area), and a layer's neighbours are as much sparser.
    pub area_scale: f64,
    /// **The cohort bound** (numerics, not ecology): a merge pass keeps at most this many
    /// cohorts per lineage, merging the closest compatible pair until it holds. Every clutch a
    /// crowded nest refuses splits a cohort in two (the layers and the refused, who retry), so
    /// without a bound the count grows geometrically with the clutch cycles.
    pub max_cohorts: usize,
    /// In the mean field, a cohort with fewer bodies than this is merged into its nearest
    /// compatible cohort at each pass: the tail of a refusal series (`p^k` of a body) carries
    /// no weight but costs a full cohort's step.
    pub min_cohort: f64,
    /// Seconds between merge passes.
    pub merge_every_s: f64,
    last_daylight: Option<f64>,
    merge_clock: f64,
}

/// The [`Food`] classes' stocks out of a [`Supply`]: litter, the glowcap's caps, carrion,
/// the photosynthetic stands' foliage, film, nectar, fruit.
pub fn stocks_of(s: &Supply) -> [f64; Food::COUNT] {
    let mut out = [0.0; Food::COUNT];
    let glow = Species::Glowcap.index();
    let foliage: f64 = s.foliage.iter().enumerate().filter(|(i, _)| *i != glow).map(|(_, v)| *v).sum();
    for food in Food::ALL {
        out[food.index()] = match food {
            Food::Litter => s.litter,
            Food::CapTissue => s.foliage[glow],
            Food::Carrion => s.carrion,
            Food::Foliage => foliage,
            Food::Film => s.film,
            Food::Nectar => s.nectar,
            Food::Fruit => s.fruit,
        };
    }
    out
}

fn book_take(t: &mut Take, food: Food, amount: f64, foliage_share: &[f64; Species::COUNT]) {
    match food {
        Food::Litter => t.litter += amount,
        Food::CapTissue => t.foliage[Species::Glowcap.index()] += amount,
        Food::Carrion => t.carrion += amount,
        Food::Foliage => {
            for (i, s) in foliage_share.iter().enumerate() {
                t.foliage[i] += amount * s;
            }
        }
        Food::Film => t.film += amount,
        Food::Nectar => t.nectar += amount,
        Food::Fruit => t.fruit += amount,
    }
}

impl FaunaModel {
    /// An empty world under `params`: no bodies, the sim's own config and day clock.
    pub fn new(params: &cubarium_voxel_sim::params::Params, fits: Fits, dt: f64) -> FaunaModel {
        FaunaModel {
            config: params.fauna.clone(),
            carrion_decomposition: params.flora.carrion_decomposition,
            day: params.world.day.clone(),
            fits,
            t_s: 0.0,
            dt,
            pops: Default::default(),
            eggs: Vec::new(),
            carcass_laid: 0.0,
            carcass_decayed: 0.0,
            carcass_left: 0.0,
            carcass_out: 0.0,
            ledger: Default::default(),
            draws: Draws::Mean,
            enabled: [true; Founder::COUNT],
            extra_bodies: [0.0; Founder::COUNT],
            area_scale: 1.0,
            max_cohorts: 48,
            min_cohort: 0.01,
            merge_every_s: 60.0,
            last_daylight: None,
            merge_clock: 0.0,
        }
    }

    pub fn phys(&self, f: Founder) -> &FounderPhysiology {
        self.config.founder(f)
    }

    /// **The world's daylight** at `t_s` (`DayClock::cycle_phase` and `Sun::at`, the sim's
    /// own functions; `World::life_daylight`, `weather.rs:1597`): 1 with the clock off.
    pub fn daylight_at(&self, t_s: f64) -> f64 {
        if self.day.mode == DayMode::Off {
            return 1.0;
        }
        let tick = (t_s * f64::from(TICK_HZ)).round().max(0.0) as u64;
        let phase = self.day.cycle_phase(tick);
        let (rise, set) = self.day.cycle_rise_set();
        Sun::at(&self.day, phase, rise, set).daylight
    }

    /// **The awake share** of a body of `f` now, outside the emergency latch
    /// (`Activity::awake_under`, `lib.rs:587-598`, read through `circadian`, `step.rs:586-710`):
    /// 1 or 0 by the rhythm, and for a dark-active lineage by day the fitted share of daytime
    /// on a dark face.
    pub fn on_phase(&self, f: Founder, daylight: f64) -> f64 {
        let p = self.phys(f);
        let clock_on = self.day.mode != DayMode::Off;
        let life = &self.config.life;
        match p.active {
            Activity::DarkActive if clock_on && daylight >= life.day_line => self.fits.of(f).day_dark_awake,
            a => {
                if a.awake(clock_on, daylight, life) {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// The census of lineage `f`.
    pub fn census(&self, f: Founder) -> Census {
        let sc = self.phys(f).core;
        let mut c = Census::default();
        let mut full = 0.0;
        for k in &self.pops[f.index()] {
            c.n += k.n;
            c.biomass += k.n * k.b.body;
            c.reserve += k.n * k.b.reserve;
            full += k.n * (k.b.reserve / sc.reserve_of(k.b.body).max(1e-300));
            if k.host.is_some() {
                c.riders += k.n;
            } else if k.b.body >= sc.birth_body {
                c.adults += k.n;
            } else {
                c.juveniles += k.n;
            }
        }
        for e in self.eggs.iter().filter(|e| e.lineage == f) {
            if e.dormant {
                c.bank += e.n;
            } else {
                c.eggs += e.n;
            }
        }
        if c.n > 0.0 {
            c.mean_body = c.biomass / c.n;
            c.mean_fullness = full / c.n;
        }
        c
    }

    /// Add `cohort` to lineage `f`.
    pub fn add(&mut self, f: Founder, cohort: Cohort) {
        self.pops[f.index()].push(cohort);
    }

    /// **One step of `dt` seconds** against the producers' and the pools' `supply`: the
    /// sim's tick order (`step.rs:162-281`) — circadian state, upkeep, the riders' take,
    /// digestion, growth; the act (feeding, then the hunt); births; deaths. Returns what
    /// the animals took and what they gave back.
    pub fn step(&mut self, supply: &Supply) -> (Take, Returns) {
        let dt = self.dt;
        let daylight = self.daylight_at(self.t_s + dt);
        let dusk = self.day.mode != DayMode::Off
            && self.last_daylight.is_some_and(|d| d >= self.config.life.day_line)
            && daylight < self.config.life.day_line;
        self.last_daylight = Some(daylight);
        let mut take = Take::default();
        let mut ret = Returns::default();

        // 1. Circadian state, upkeep, digestion and growth.
        let mut awake_of: Vec<Vec<f64>> = vec![Vec::new(); Founder::COUNT];
        for f in Founder::ALL {
            if !self.enabled[f.index()] {
                continue;
            }
            let phys = *self.phys(f);
            let phase = self.on_phase(f, daylight);
            let effort = self.fits.of(f).motor_effort;
            let mut acc = Flows::default();
            let mut matured = (0.0, 0.0);
            let mut aw = Vec::with_capacity(self.pops[f.index()].len());
            let mut lsum = (0.0, 0.0, 0.0);
            for c in self.pops[f.index()].iter_mut() {
                c.age_s += dt;
                c.b.emergency = laws::emergency(&phys, &c.b);
                let a = if c.host.is_some() || c.b.emergency { 1.0 } else { phase };
                aw.push(a);
                if c.host.is_none() {
                    lsum.0 += c.n * dt;
                    lsum.1 += c.n * a * dt;
                }
                lsum.2 += c.n * c.b.body * dt;
                if !c.b.eating && a > 0.0 && c.host.is_none() && laws::wants_food(&phys, &c.b) {
                    c.b.eating = true;
                }
                let mut fl = Flows::default();
                let up = laws::upkeep_per_s(&phys, c.b.body, a) * dt;
                let (paid, fb) = laws::respire(&phys.core, &mut c.b, up);
                fl.upkeep += paid;
                fl.from_body += fb;
                if c.host.is_none() {
                    let mo = laws::motor_per_s(&phys, c.b.body, a, effort) * dt;
                    let (paid, fb) = laws::respire(&phys.core, &mut c.b, mo);
                    fl.motor += paid;
                    fl.from_body += fb;
                }
                let d = laws::digest(&phys, &mut c.b, dt);
                fl += d;
                let before = c.b.body;
                fl.growth += laws::grow(&phys.core, &mut c.b, dt);
                if !c.matured && before < phys.core.birth_body && c.b.body >= phys.core.birth_body {
                    c.matured = true;
                    matured.0 += c.n;
                    matured.1 += c.n * c.age_s;
                }
                acc += fl.scaled(c.n);
                ret.litter += fl.egested * c.n;
            }
            awake_of[f.index()] = aw;
            let l = &mut self.ledger[f.index()];
            l.flows += acc;
            l.matured += matured.0;
            l.maturation_age_s += matured.1;
            l.alive_s += lsum.0;
            l.awake_s += lsum.1;
            l.body_s += lsum.2;
        }
        self.riders(dt);

        // 2. The act: feeding, then the hunt.
        let stocks = stocks_of(supply);
        let mut left = stocks;
        let glow = Species::Glowcap.index();
        let foliage_total = stocks[Food::Foliage.index()];
        let mut foliage_share = [0.0; Species::COUNT];
        if foliage_total > 0.0 {
            for (i, v) in supply.foliage.iter().enumerate() {
                if i != glow {
                    foliage_share[i] = v / foliage_total;
                }
            }
        }
        for f in Founder::ALL {
            if !self.enabled[f.index()] {
                continue;
            }
            let phys = *self.phys(f);
            let fit = *self.fits.of(f);
            let hunter = phys.hunter.is_some();
            let diet: Vec<Food> = phys.diet.foods().collect();
            // The response is fitted on patch's world totals: on a larger floor the stock and the
            // crowd are read per patch-sized area.
            let area = self.area_scale.max(1e-9);
            let own: f64 = (self.pops[f.index()].iter().filter(|c| c.host.is_none()).map(|c| c.n).sum::<f64>() + self.extra_bodies[f.index()]) / area;
            for (k, c) in self.pops[f.index()].iter_mut().enumerate() {
                let a = awake_of[f.index()].get(k).copied().unwrap_or(0.0);
                if !c.b.eating || !(a > 0.0) || c.host.is_some() || !(c.n > 0.0) {
                    continue;
                }
                let want = laws::bite_want(&phys, &c.b, dt) * a;
                if !(want > 0.0) {
                    continue;
                }
                // ψ per food at its stock; a hunter's carrion is its own kill's carcass.
                let mut psi = [0.0f64; Food::COUNT];
                for &food in &diet {
                    psi[food.index()] = if hunter && food == Food::Carrion {
                        if c.carcass > 0.0 { 1.0 } else { fit.psi(food, left[food.index()] / area, own) }
                    } else {
                        fit.psi(food, left[food.index()] / area, own)
                    };
                }
                let total: f64 = psi.iter().sum();
                if !(total > 0.0) {
                    continue;
                }
                let norm = total.max(1.0);
                for &food in &diet {
                    let share = psi[food.index()] / norm;
                    if !(share > 0.0) {
                        continue;
                    }
                    let mut bite = want * share;
                    let from_carcass = hunter && food == Food::Carrion && c.carcass > 0.0;
                    let avail = if from_carcass { c.carcass } else { left[food.index()] };
                    bite = bite.min(avail / c.n.max(1e-12));
                    if !(bite > 0.0) {
                        continue;
                    }
                    let spill = phys.bite_spill_fraction.clamp(0.0, 1.0);
                    let all = bite * c.n;
                    if from_carcass {
                        c.carcass = (c.carcass - all).max(0.0);
                    } else {
                        left[food.index()] -= all;
                        book_take(&mut take, food, all, &foliage_share);
                    }
                    ret.litter += all * spill;
                    c.b.stomach[food.index()] += bite * (1.0 - spill);
                    self.ledger[f.index()].swallowed[food.index()] += all;
                }
                if laws::full(&phys, &c.b) {
                    c.b.eating = false;
                }
            }
        }
        self.hunt(dt, &awake_of, &mut ret);

        // 3. Births, then 4. deaths.
        self.reproduce(dt, dusk, &mut ret);
        self.deaths(dt, &mut ret);

        // The hunters' fresh kills decay at the carrion rate, and are left to the carrion pool
        // at `1 / carcass_stay_s`.
        self.carcasses(dt, &mut ret);

        self.t_s += dt;
        self.merge_clock += dt;
        if self.merge_clock >= self.merge_every_s {
            self.merge_clock = 0.0;
            self.merge_cohorts();
        }
        (take, ret)
    }

    /// **The hunters' fresh kills** over `dt`: each cohort's carcass decays at
    /// `carrion_decomposition` (`flora.carrion_decomposition`, the pool's own rate) and is left
    /// at `1 / carcass_stay_s` ([`Fits::carcass_stay_s`]: how long a hunter stays at its kill,
    /// fitted); the two together as one exponential, split in proportion. What is left goes to
    /// the carrion pool ([`Returns::carrion`]).
    fn carcasses(&mut self, dt: f64, ret: &mut Returns) {
        let k = self.carrion_decomposition.max(0.0);
        let r = if self.fits.carcass_stay_s > 0.0 { 1.0 / self.fits.carcass_stay_s } else { f64::INFINITY };
        for v in self.pops.iter_mut() {
            for c in v.iter_mut().filter(|c| c.carcass > 0.0) {
                let (gone, left_share) = if r.is_infinite() {
                    (c.carcass, 1.0)
                } else {
                    (c.carcass * (1.0 - (-(k + r) * dt).exp()), if k + r > 0.0 { r / (k + r) } else { 0.0 })
                };
                c.carcass -= gone;
                self.carcass_decayed += gone * (1.0 - left_share);
                self.carcass_left += gone * left_share;
                ret.carrion += gone * left_share;
            }
        }
    }

    /// **The parasite riders** (`drain_riders`, `step.rs:456-484`; `drop_off_grown`,
    /// `step.rs:489-506`): each rider takes `parasite_take_per_day / ecological_day_s` of its
    /// host's body per second into its reserve (from the host's reserve, then structure,
    /// spread over the host lineage's cohorts), and drops off free at
    /// `parasite_detach_body_fraction · body_max`. A rider whose host lineage is gone is freed.
    fn riders(&mut self, dt: f64) {
        let per_s = if self.config.life.ecological_day_s > 0.0 {
            self.config.parasite_take_per_day / self.config.life.ecological_day_s
        } else {
            0.0
        };
        for f in Founder::ALL {
            let detach = self.config.parasite_detach_body_fraction * self.phys(f).core.body_max;
            let n = self.pops[f.index()].len();
            for k in 0..n {
                let Some(host) = self.pops[f.index()][k].host else { continue };
                let hc = self.census(host);
                if !(hc.n > 0.0) {
                    self.pops[f.index()][k].host = None;
                    self.ledger[f.index()].dropoffs += self.pops[f.index()][k].n;
                    continue;
                }
                let riders = self.pops[f.index()][k].n;
                let want_each = per_s * hc.mean_body * dt;
                let want = want_each * riders;
                // From the host lineage pro rata to its bodies.
                let hsc = self.phys(host).core;
                let mut got = 0.0;
                for h in self.pops[host.index()].iter_mut() {
                    let share = h.n / hc.n;
                    let each = want * share / h.n.max(1e-12);
                    let (paid, _) = laws::respire(&hsc, &mut h.b, each);
                    got += paid * h.n;
                }
                self.ledger[host.index()].rider_loss += got;
                self.ledger[f.index()].rider_take += got;
                let r = &mut self.pops[f.index()][k];
                r.b.reserve += got / riders.max(1e-12);
                if r.b.body >= detach {
                    r.host = None;
                    self.ledger[f.index()].dropoffs += r.n;
                }
            }
        }
    }

    /// **The lanternjaw's hunt** (and any lineage with a [`cubarium_voxel_fauna::Hunter`]): a
    /// hunter awake with its eating latch on hunts; a prey body of a
    /// lineage on its list, inside its size window, dies at hazard
    /// `kill_rate(prey) · hunters hunting · share²` — the attack chance is the profitability
    /// (`hunt::attack_chance` with the opportunistic preference 1, `hunt.rs:171-202`) and the
    /// kill per strike the wound share (`apply_hit`, `step.rs:2207-2229`). The share
    /// `kill_reach` of the corpse goes to the killer cohort's carcass, the rest to the carrion
    /// pool; each kill costs `strikes_per_kill` strikes at `strike_cost_s` of awake
    /// upkeep (`Hunter::strike_cost_s`).
    fn hunt(&mut self, dt: f64, awake_of: &[Vec<f64>], ret: &mut Returns) {
        for hf in Founder::ALL {
            if !self.enabled[hf.index()] {
                continue;
            }
            let hp = *self.phys(hf);
            let Some(h) = hp.hunter else { continue };
            // Hunting bodies: awake, free, with the eating latch on — a hunter strikes only while
            // it wants food (`founder_act`, `step.rs:1655-1668`), and its patrol does not wait at
            // a carcass: it feeds from it as it passes and strikes again while still hungry.
            let mut hunting = Vec::new();
            for (k, c) in self.pops[hf.index()].iter().enumerate() {
                let a = awake_of[hf.index()].get(k).copied().unwrap_or(0.0);
                if c.host.is_none() && c.b.eating && a > 0.0 {
                    hunting.push((k, c.n * a, c.b.body));
                }
            }
            if hunting.is_empty() {
                continue;
            }
            for (k, hn, hb) in hunting {
                if !(hn > 0.0) {
                    continue;
                }
                self.ledger[hf.index()].hunting_s += hn * dt;
                let mut kills_here = 0.0;
                let mut laid_here = 0.0;
                for pf in Founder::ALL {
                    if !h.hunts(pf) || !self.enabled[pf.index()] {
                        continue;
                    }
                    let alpha = self.fits.of(pf).kill_rate;
                    if !(alpha > 0.0) {
                        continue;
                    }
                    let pp = *self.phys(pf);
                    for c in self.pops[pf.index()].iter_mut() {
                        if c.host.is_some() || !(c.n > 0.0) || !laws::in_size_window(&hp, hb, c.b.body) {
                            continue;
                        }
                        let share = laws::strike_share(&hp, hb, &pp, c.b.body);
                        let hazard = alpha / self.area_scale.max(1e-9) * hn * share * share;
                        let p = 1.0 - (-hazard * dt).exp();
                        let dead = self.draws.of(c.n, p).min(c.n);
                        if !(dead > 0.0) {
                            continue;
                        }
                        c.n -= dead;
                        laid_here += dead * c.b.organic();
                        self.ledger[pf.index()].killed_by[hf.index()] += dead;
                        kills_here += dead;
                    }
                }
                // The share of the corpse at its mouth is its carcass; the rest is carrion now.
                let reach = self.fits.kill_reach.clamp(0.0, 1.0);
                self.pops[hf.index()][k].carcass += reach * laid_here;
                self.carcass_laid += reach * laid_here;
                ret.carrion += (1.0 - reach) * laid_here;
                self.carcass_out += (1.0 - reach) * laid_here;
                if kills_here > 0.0 {
                    let strikes = kills_here * self.fits.strikes_per_kill;
                    let hc = &mut self.pops[hf.index()][k];
                    let each = strikes * h.strike_cost_s * laws::upkeep_per_s(&hp, hc.b.body, 1.0) / hc.n.max(1e-12);
                    let (paid, _) = laws::respire(&hp.core, &mut hc.b, each);
                    let l = &mut self.ledger[hf.index()];
                    l.kills += kills_here;
                    l.strikes += strikes;
                    l.strike_cost += paid * hc.n;
                    l.flows.motor += paid * hc.n;
                }
            }
        }
    }

    /// The lineage's own bodies and unhatched (non-dormant) eggs: what a nest-site count sees.
    fn nest_count(&self, f: Founder) -> f64 {
        let c = self.census(f);
        (c.n - c.riders + c.eggs + self.extra_bodies[f.index()]) / self.area_scale.max(1e-9)
    }

    /// **Births** (`reproduce`, `step.rs:2808-2850`; `advance_eligibility`, `step.rs:2855-2879`;
    /// `lay`, `step.rs:3168-3313`; `gestate`, `step.rs:2883-3018`; `hatch`, `step.rs:3470-3515`;
    /// the dormant bank, `dormant_eggs`, `step.rs:3552-3637`).
    fn reproduce(&mut self, dt: f64, dusk: bool, ret: &mut Returns) {
        // Clutches at term hatch; dormant eggs pay upkeep, die past viability, hatch at dusk.
        let mut hatchlings: Vec<(Founder, Cohort)> = Vec::new();
        let mut keep = Vec::with_capacity(self.eggs.len());
        let eggs = std::mem::take(&mut self.eggs);
        for mut e in eggs {
            e.age_s += dt;
            let phys = *self.phys(e.lineage);
            let sc = phys.core;
            if !e.dormant {
                if e.age_s >= sc.reproduction.incubation_s {
                    let mut c = Cohort::new(e.n, sc.body_min, (e.organic - sc.body_min).max(0.0));
                    c.native = true;
                    self.ledger[e.lineage.index()].hatched += e.n;
                    hatchlings.push((e.lineage, c));
                } else {
                    keep.push(e);
                }
                continue;
            }
            // The bank (`egg_upkeep`, `step.rs:933-947`; viability, `Dormancy::draw`).
            let pc = self.config.predation;
            let paid = (pc.egg_upkeep_per_s * e.organic * dt).min(e.organic);
            e.organic -= paid;
            self.ledger[e.lineage.index()].bank_upkeep += paid * e.n;
            let lo = pc.egg_viability_median_s / pc.egg_viability_spread.max(1.0);
            let hi = pc.egg_viability_median_s * pc.egg_viability_spread.max(1.0);
            let surv = |t: f64| -> f64 {
                if t <= lo {
                    1.0
                } else if t >= hi {
                    0.0
                } else {
                    1.0 - (t / lo).ln() / (hi / lo).ln()
                }
            };
            let s0 = surv(e.age_s - dt);
            let p_die = if s0 > 0.0 { 1.0 - surv(e.age_s) / s0 } else { 1.0 };
            let died = if e.organic < sc.body_min { e.n } else { self.draws.of(e.n, p_die) };
            if died > 0.0 {
                e.n -= died;
                ret.litter += died * e.organic;
                self.ledger[e.lineage.index()].bank_died += died;
            }
            if dusk && e.n > 0.0 {
                let p = self.fits.bank_trigger_per_dusk * self.config.hatch_chance_per_dusk;
                let hatch = self.draws.of(e.n, p);
                // A host: any living non-hunter body, chosen in proportion to the counts.
                let hosts: Vec<(Founder, f64)> = Founder::ALL
                    .iter()
                    .filter(|&&g| g != e.lineage && self.enabled[g.index()])
                    .map(|&g| (g, self.census(g).n - self.census(g).riders))
                    .filter(|(_, n)| *n > 0.0)
                    .collect();
                let total: f64 = hosts.iter().map(|(_, n)| n).sum();
                if hatch > 0.0 && total > 0.0 {
                    e.n -= hatch;
                    for (g, n) in hosts {
                        let mut c = Cohort::new(hatch * n / total, sc.body_min, (e.organic - sc.body_min).max(0.0));
                        c.native = true;
                        c.host = Some(g);
                        self.ledger[e.lineage.index()].riders_attached += c.n;
                        self.ledger[e.lineage.index()].hatched += c.n;
                        hatchlings.push((e.lineage, c));
                    }
                }
            }
            if e.n > 1e-9 {
                keep.push(e);
            }
        }
        self.eggs = keep;

        for f in Founder::ALL {
            if !self.enabled[f.index()] {
                continue;
            }
            let phys = *self.phys(f);
            let sc = phys.core;
            let rule = sc.reproduction;
            let fit = *self.fits.of(f);
            let hold_steps = ((rule.surplus_hold_s / dt).ceil() as u64).max(1);
            let required = (rule.surplus_share.clamp(0.0, 1.0) * hold_steps as f64).ceil() as u64;
            let mut split: Vec<Cohort> = Vec::new();
            let nest_n = self.nest_count(f);
            let n_cohorts = self.pops[f.index()].len();
            for k in 0..n_cohorts {
                let c = &mut self.pops[f.index()][k];
                if c.host.is_some() || !(c.n > 0.0) {
                    continue;
                }
                c.retry_s = (c.retry_s - dt).max(0.0);
                if c.refractory_s > 0.0 {
                    c.refractory_s = (c.refractory_s - dt).max(0.0);
                    c.window.reset();
                    continue;
                }
                if c.escrow.is_none() {
                    let eligible = c.b.body >= sc.birth_body && c.b.reserve >= rule.surplus_reserve(&sc) && rule.package_cost(&sc) > 0.0;
                    c.window.tick(eligible, hold_steps);
                    let met = eligible && c.window.count_for(hold_steps) >= required;
                    if !met || c.retry_s > 0.0 {
                        continue;
                    }
                }
                match rule.mode {
                    BirthMode::Gestation => {
                        let (mut esc, mut ran) = c.escrow.unwrap_or((0.0, 0.0));
                        ran += dt;
                        let target = sc.birth_cost * (ran / rule.gestation_s.max(dt)).min(1.0);
                        let want = (target - esc).max(0.0);
                        if c.b.reserve < want {
                            // An instalment it cannot pay: the loss respired, the rest back.
                            let loss = esc * rule.gestation_loss_fraction;
                            c.b.reserve += esc - loss;
                            self.ledger[f.index()].gestation_loss += loss * c.n;
                            c.escrow = None;
                            continue;
                        }
                        c.b.reserve -= want;
                        c.b.brood = c.b.brood.min(c.b.reserve);
                        esc += want;
                        self.ledger[f.index()].eggs_paid += want * c.n;
                        if ran + 1e-9 < rule.gestation_s {
                            c.escrow = Some((esc, ran));
                            continue;
                        }
                        c.escrow = None;
                        c.refractory_s = rule.birth_interval_s;
                        c.window.reset();
                        let mut young = Cohort::new(c.n, sc.body_min, (esc - sc.body_min).max(0.0));
                        young.native = true;
                        let l = &mut self.ledger[f.index()];
                        l.born += c.n;
                        l.clutches += c.n;
                        l.laid += c.n;
                        if c.native && !c.bred {
                            l.first_clutches += c.n;
                            l.first_clutch_age_s += c.n * c.age_s;
                        }
                        c.bred = true;
                        hatchlings.push((f, young));
                    }
                    BirthMode::Eggs => {
                        if rule.clutch_max > 0 {
                            let wait = rule.clutch_hold_max_s;
                            let full = rule.full_clutch(sc.reserve_of(c.b.body));
                            if wait > 0.0 && rule.clutch_for(c.b.reserve) < full && c.clutch_wait_s < wait {
                                c.clutch_wait_s += dt;
                                continue;
                            }
                        }
                        let eggs = rule.clutch_for(c.b.reserve);
                        let cost = if rule.clutch_max > 0 { f64::from(eggs) * rule.egg_organic } else { rule.package_cost(&sc) };
                        if eggs == 0 || !(cost > 0.0) || c.b.reserve < cost {
                            continue;
                        }
                        // An egg site, then the nest-site count: refusals cost nothing and
                        // retry after `nest_retry_s`.
                        let mut refuse = 0.0;
                        if !rule.dormant {
                            if fit.egg_site < 1.0 {
                                refuse += 1.0 - fit.egg_site;
                            }
                            if rule.nest_radius_m > 0.0 {
                                let mean = fit.nest_aggregation * fit.crowd_per_body * (nest_n - 1.0).max(0.0);
                                refuse += (1.0 - refuse) * poisson_above(mean, rule.nest_crowd_max);
                            }
                        }
                        let refused = if refuse > 0.0 { self.draws.of(c.n, refuse).min(c.n) } else { 0.0 };
                        if refused > 0.0 {
                            let mut r = c.clone();
                            r.n = refused;
                            r.retry_s = rule.nest_retry_s;
                            r.carcass = c.carcass * refused / c.n;
                            c.carcass -= r.carcass;
                            c.n -= refused;
                            self.ledger[f.index()].nest_refusals += refused;
                            split.push(r);
                            if !(c.n > 1e-12) {
                                continue;
                            }
                        }
                        c.b.reserve -= cost;
                        c.b.brood = (c.b.brood - cost).max(0.0).min(c.b.reserve);
                        c.refractory_s = rule.birth_interval_s;
                        c.window.reset();
                        c.clutch_wait_s = 0.0;
                        let laid = c.n * f64::from(eggs);
                        let per_egg = cost / f64::from(eggs);
                        let l = &mut self.ledger[f.index()];
                        l.clutches += c.n;
                        l.laid += laid;
                        l.eggs_paid += c.n * cost;
                        if c.native && !c.bred {
                            l.first_clutches += c.n;
                            l.first_clutch_age_s += c.n * c.age_s;
                        }
                        c.bred = true;
                        // Lost where they lie (flooded, the face gone: `clutch_fate`, which takes
                        // dormant clutches too), to carrion.
                        let lost = self.draws.of(laid, fit.egg_loss);
                        if lost > 0.0 {
                            ret.carrion += lost * per_egg;
                            self.ledger[f.index()].eggs_lost += lost;
                        }
                        self.eggs.push(EggCohort { lineage: f, n: laid - lost, organic: per_egg, age_s: 0.0, dormant: rule.dormant });
                    }
                }
            }
            self.pops[f.index()].extend(split);
        }
        for (f, c) in hatchlings {
            if c.n > 0.0 {
                self.pops[f.index()].push(c);
            }
        }
    }

    /// **Deaths** (`deaths`, `step.rs:3764-3842`): starved below `body_min` (a whole cohort,
    /// being identical), aged out on the lifespan draw ([`laws::lifespan_survival`]). Every
    /// corpse, whole, to the carrion pool. Drowning is spatial and has no fitted hazard yet.
    fn deaths(&mut self, dt: f64, ret: &mut Returns) {
        for f in Founder::ALL {
            if !self.enabled[f.index()] {
                continue;
            }
            let sc = self.phys(f).core;
            let life = self.config.life;
            let mut v = std::mem::take(&mut self.pops[f.index()]);
            for c in v.iter_mut() {
                if !(c.n > 0.0) {
                    continue;
                }
                if c.b.body < sc.body_min {
                    ret.carrion += c.n * c.b.organic() + c.carcass;
                    self.carcass_left += c.carcass;
                    c.carcass = 0.0;
                    self.ledger[f.index()].starved += c.n;
                    c.n = 0.0;
                    continue;
                }
                let s0 = laws::lifespan_survival(&life, sc.lifespan_median_s, c.age_s - dt, c.arriving);
                let s1 = laws::lifespan_survival(&life, sc.lifespan_median_s, c.age_s, c.arriving);
                let p = if s0 > 0.0 { 1.0 - s1 / s0 } else { 1.0 };
                if p > 0.0 {
                    let died = self.draws.of(c.n, p).min(c.n);
                    if died > 0.0 {
                        let kill_share = c.carcass * died / c.n;
                        ret.carrion += died * c.b.organic() + kill_share;
                        self.carcass_left += kill_share;
                        c.carcass -= kill_share;
                        self.ledger[f.index()].aged += died;
                        c.n -= died;
                    }
                }
            }
            for c in v.iter().filter(|c| !(c.n > 1e-9) && c.carcass > 0.0) {
                ret.carrion += c.carcass;
                self.carcass_left += c.carcass;
            }
            v.retain(|c| c.n > 1e-9);
            self.pops[f.index()] = v;
        }
    }

    /// **The merge pass**: per lineage, cohorts that have become the same bodies (closer than
    /// [`MERGE_SAME`] in [`cohort_distance`]) merge; in the mean field a cohort under
    /// `min_cohort` bodies merges into its nearest compatible one; then the closest compatible
    /// pairs merge until the lineage holds at most `max_cohorts`. Egg cohorts of a lineage laid
    /// within [`EGG_MERGE_S`] of each other with the same egg merge too. A merge is
    /// body-weighted, so bodies and organic matter are kept exactly; what it loses is the
    /// spread between the merged bodies (their timing within a clutch cycle above all).
    pub fn merge_cohorts(&mut self) {
        let mean = !self.draws.is_twin();
        for f in Founder::ALL {
            let phys = *self.phys(f);
            let v = std::mem::take(&mut self.pops[f.index()]);
            self.pops[f.index()] = merge_lineage(v, &phys, self.max_cohorts.max(1), if mean { self.min_cohort } else { 0.0 });
        }
        let eggs = std::mem::take(&mut self.eggs);
        let mut out: Vec<EggCohort> = Vec::with_capacity(eggs.len());
        'egg: for e in eggs {
            for o in out.iter_mut() {
                if o.lineage == e.lineage
                    && o.dormant == e.dormant
                    && (o.organic - e.organic).abs() <= 1e-9 * o.organic.max(1e-12)
                    && (o.age_s - e.age_s).abs() <= EGG_MERGE_S
                {
                    let n = o.n + e.n;
                    o.age_s = (o.age_s * o.n + e.age_s * e.n) / n;
                    o.n = n;
                    continue 'egg;
                }
            }
            out.push(e);
        }
        self.eggs = out;
    }

    /// Organic matter the animals hold now: bodies, reserves, stomachs, escrows, eggs, the bank,
    /// and the fresh-kill carcass. The conservation ledger's stock.
    pub fn organic_held(&self) -> f64 {
        let mut s = 0.0;
        for f in Founder::ALL {
            for c in &self.pops[f.index()] {
                s += c.n * (c.b.organic() + c.escrow.map_or(0.0, |e| e.0)) + c.carcass;
            }
        }
        for e in &self.eggs {
            s += e.n * e.organic;
        }
        s
    }
}

/// `P(X > k)` for `X ~ Poisson(mean)`.
pub fn poisson_above(mean: f64, k: u32) -> f64 {
    if !(mean > 0.0) {
        return 0.0;
    }
    let mut term = (-mean).exp();
    let mut cdf = term;
    for i in 1..=k {
        term *= mean / f64::from(i);
        cdf += term;
    }
    (1.0 - cdf).clamp(0.0, 1.0)
}

/// Two cohorts closer than this in [`cohort_distance`] are the same bodies: about 0.5 % of
/// body, 0.5 % of the reserve cap, 3 min of age, 1 % of the clutch interval.
pub const MERGE_SAME: f64 = 0.1;

/// Egg cohorts laid this close in time, with the same egg, merge (seconds).
pub const EGG_MERGE_S: f64 = 15.0;

/// **How far apart two cohorts are**, in units where 1 is a difference that matters: 5 % of
/// body (log), 5 % of the reserve cap in reserve and in brood, 20 % of the stomach, 30 min of
/// age, 10 % of the clutch interval in refractory, a minute of nest retry or clutch wait, and
/// a latch that differs. Infinite for cohorts that cannot merge: a different host, origin
/// (native, arriving), booked stage (matured, bred), or an open gestation.
pub fn cohort_distance(phys: &FounderPhysiology, a: &Cohort, b: &Cohort) -> f64 {
    if a.host != b.host
        || a.native != b.native
        || a.arriving != b.arriving
        || a.matured != b.matured
        || a.bred != b.bred
        || a.escrow.is_some()
        || b.escrow.is_some()
    {
        return f64::INFINITY;
    }
    let sc = phys.core;
    let cap = sc.reserve_of(a.b.body.max(b.b.body)).max(1e-12);
    let k = phys.gut.capacity(a.b.body.max(b.b.body)).max(1e-12);
    let interval = sc.reproduction.birth_interval_s.max(60.0);
    let terms = [
        (a.b.body.max(1e-12) / b.b.body.max(1e-12)).ln() / 0.05,
        (a.b.reserve - b.b.reserve) / cap / 0.05,
        (a.b.brood - b.b.brood) / cap / 0.05,
        (a.b.stomach_total() - b.b.stomach_total()) / k / 0.2,
        (a.age_s - b.age_s) / 1800.0,
        (a.refractory_s - b.refractory_s) / (0.1 * interval),
        (a.retry_s - b.retry_s) / 60.0,
        (a.clutch_wait_s - b.clutch_wait_s) / 60.0,
        (a.carcass / a.n.max(1e-12) - b.carcass / b.n.max(1e-12)) / k / 0.2,
        if a.b.eating == b.b.eating { 0.0 } else { 1.0 },
        if a.b.emergency == b.b.emergency { 0.0 } else { 1.0 },
    ];
    terms.iter().map(|t| t * t).sum::<f64>().sqrt()
}

/// `b` into `a`, body-weighted: every per-body stock and clock is the bodies' mean, so the
/// cohort's totals are kept exactly. The latches and the surplus window are the larger's.
fn absorb(a: &mut Cohort, b: &Cohort) {
    let n = a.n + b.n;
    if !(n > 0.0) {
        return;
    }
    let (wa, wb) = (a.n / n, b.n / n);
    let w = |x: f64, y: f64| x * wa + y * wb;
    a.b.body = w(a.b.body, b.b.body);
    a.b.reserve = w(a.b.reserve, b.b.reserve);
    a.b.brood = w(a.b.brood, b.b.brood).min(a.b.reserve);
    for i in 0..Food::COUNT {
        a.b.stomach[i] = w(a.b.stomach[i], b.b.stomach[i]);
    }
    a.age_s = w(a.age_s, b.age_s);
    a.refractory_s = w(a.refractory_s, b.refractory_s);
    a.retry_s = w(a.retry_s, b.retry_s);
    a.clutch_wait_s = w(a.clutch_wait_s, b.clutch_wait_s);
    a.carcass += b.carcass;
    if b.n > a.n {
        a.window = b.window;
        a.b.eating = b.b.eating;
        a.b.emergency = b.b.emergency;
    }
    a.n = n;
}

/// One lineage's merge pass ([`FaunaModel::merge_cohorts`]).
fn merge_lineage(mut v: Vec<Cohort>, phys: &FounderPhysiology, max: usize, min_n: f64) -> Vec<Cohort> {
    v.retain(|c| c.n > 0.0);
    if v.len() < 2 {
        return v;
    }
    let n = v.len();
    let mut alive = vec![true; n];
    // The distance matrix, upper triangle; rows refreshed as cohorts absorb others.
    let mut d = vec![f64::INFINITY; n * n];
    for i in 0..n {
        for j in i + 1..n {
            d[i * n + j] = cohort_distance(phys, &v[i], &v[j]);
        }
    }
    let dist = |d: &[f64], i: usize, j: usize| if i < j { d[i * n + j] } else { d[j * n + i] };
    let mut count = n;
    // Tiny cohorts with no compatible partner are left as they are.
    let mut stuck = vec![false; n];
    loop {
        // The tiniest cohort under the floor, else the closest pair.
        let tiny = (0..n).filter(|&i| alive[i] && !stuck[i] && v[i].n < min_n).min_by(|&a, &b| v[a].n.partial_cmp(&v[b].n).unwrap());
        let pair = if let Some(i) = tiny {
            let p = (0..n).filter(|&j| j != i && alive[j]).map(|j| (dist(&d, i, j), i, j)).min_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            if p.is_none_or(|p| !p.0.is_finite()) {
                stuck[i] = true;
                continue;
            }
            p
        } else {
            let mut best: Option<(f64, usize, usize)> = None;
            for i in 0..n {
                if !alive[i] {
                    continue;
                }
                for j in i + 1..n {
                    if alive[j] && best.is_none_or(|b| d[i * n + j] < b.0) {
                        best = Some((d[i * n + j], i, j));
                    }
                }
            }
            best.filter(|b| b.0.is_finite() && (b.0 < MERGE_SAME || count > max))
        };
        let Some((_, a, b)) = pair else { break };
        let (keep, gone) = if v[a].n >= v[b].n { (a, b) } else { (b, a) };
        let g = v[gone].clone();
        absorb(&mut v[keep], &g);
        alive[gone] = false;
        count -= 1;
        for j in 0..n {
            if j != keep && alive[j] {
                let x = cohort_distance(phys, &v[keep], &v[j]);
                if keep < j { d[keep * n + j] = x } else { d[j * n + keep] = x }
            }
        }
    }
    v.into_iter().zip(alive).filter(|(_, a)| *a).map(|(c, _)| c).collect()
}
