//! **Continuous balance measures** (the search rebase's addendum,
//! `design/handoffs/search-rebase-2026-10-03.md`): with the minimal gut a full adult lasts
//! 3–25 world days, so alive counts after a 2 h run (about one world day) no longer show
//! starvation. These read a lineage's spend and income from the fauna ledger's per-lineage
//! counters instead, over any interval (a window, a run), and are the search's
//! per-lineage viability signal. A **world day** here is the ecological day
//! ([`cubarium_voxel_fauna::LifeConfig::ecological_day_s`]), the day the gut's daily need
//! is written in.

use cubarium_voxel_fauna::{Animal, BirthMode, Departure, FaunaConfig, FaunaLedger, Founder};
use serde::Serialize;

/// One lineage's books over an interval, organic units and ticks: what it assimilated,
/// what it spent, and how often it wanted food and ate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Books {
    pub assimilated: f64,
    /// Upkeep paid, the sleep factor applied.
    pub upkeep: f64,
    /// Motor paid (walking, flying, climbing, a hunter's lure and strikes).
    pub motor: f64,
    /// What riders took out of this lineage's bodies.
    pub rider_take_lost: f64,
    /// Structure built, reproduction invested and provisioning given: spending on the
    /// future, reported apart from `B`'s denominator.
    pub growth: f64,
    pub reproduction: f64,
    pub provision: f64,
    /// Awake ticks with the eating latch on, and of those the ticks spent eating.
    pub wanting_ticks: u64,
    pub eating_ticks: u64,
    /// Awake ticks, latch on or off.
    pub awake_ticks: u64,
}

impl Books {
    /// The ledger's running totals for `f`.
    pub fn of(l: &FaunaLedger, f: Founder) -> Books {
        let i = f.index();
        Books {
            assimilated: l.assimilated_by_founder[i],
            upkeep: l.upkeep_by_founder[i],
            motor: l.motor_by_founder[i],
            rider_take_lost: l.rider_take_lost_by_founder[i],
            growth: l.growth_by_founder[i],
            reproduction: l.reproduction_by_founder[i],
            provision: l.provision_by_founder[i],
            wanting_ticks: l.wanting_ticks_by_founder[i],
            eating_ticks: l.eating_ticks_by_founder[i],
            awake_ticks: l.awake_ticks_by_founder[i],
        }
    }

    /// The books between `start` and these (running totals at both ends).
    pub fn since(&self, start: &Books) -> Books {
        Books {
            assimilated: self.assimilated - start.assimilated,
            upkeep: self.upkeep - start.upkeep,
            motor: self.motor - start.motor,
            rider_take_lost: self.rider_take_lost - start.rider_take_lost,
            growth: self.growth - start.growth,
            reproduction: self.reproduction - start.reproduction,
            provision: self.provision - start.provision,
            wanting_ticks: self.wanting_ticks.saturating_sub(start.wanting_ticks),
            eating_ticks: self.eating_ticks.saturating_sub(start.eating_ticks),
            awake_ticks: self.awake_ticks.saturating_sub(start.awake_ticks),
        }
    }

    /// The spend `B` divides by: upkeep, motor and the rider take lost.
    pub fn spend(&self) -> f64 {
        self.upkeep + self.motor + self.rider_take_lost
    }
}

/// What the measures need beside the books.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Context {
    /// The lineage's stores at the interval's end: reserve plus digestible stomach
    /// (stomach x yield, [`super::targets::digestible_stomach`]), summed over its bodies.
    pub stores: f64,
    /// Adult-days lived in the interval: adults (free bodies at `birth_body` or more)
    /// integrated over world days.
    pub adult_days: f64,
    /// The interval's length, world days.
    pub days: f64,
    /// The lineage's median lifespan, world days (0: none).
    pub lifespan_days: f64,
    /// The cost of one offspring: a live birth's `birth_cost`, one egg's `egg_organic`.
    pub offspring_cost: f64,
}

/// **The balance measures** of one lineage over one interval. NaN where the interval has
/// nothing to measure (no spend, no adults, no wanting).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Measures {
    /// **Energy balance** `B = assimilated / (upkeep + motor + rider take lost)`.
    pub energy_balance: f64,
    /// **Days to empty**, when `B < 1`: the stores over the deficit per world day,
    /// `stores / ((spend − assimilated) / days)`. `None` at `B ≥ 1` (to rounding).
    pub days_to_empty: Option<f64>,
    /// **Surplus** per adult per world day: `(assimilated − upkeep − motor − rider losses) / adult_days`.
    pub surplus_per_adult_day: f64,
    /// **Surplus-extrapolated replacement potential** `R̂ = surplus per adult per day × median lifespan in
    /// days / the cost of one offspring`: the offspring a lifetime's surplus could pay for.
    /// **Optimistic**: it assumes every juvenile survives to replace its parent, and that
    /// all surplus goes to young. Under the gut a full reserve digests only what is burnt,
    /// so the surplus is what was actually built and invested (growth, young, reserve
    /// refilled), not what a fed body could have eaten.
    pub r_hat: f64,
    /// Explicitly labelled alias: a surplus extrapolation, not observed recruitment.
    pub r_hat_surplus_extrapolated: f64,
    /// **Access**: eating ticks over wanting ticks, at most 1.
    pub access: f64,
    /// **Sated share**: awake ticks with the eating latch off over awake ticks. It reads
    /// `R̂`, which is realised (what food actually funded, not unrealised capacity): a low
    /// `R̂` with a **high** sated share means the breeding rule is the limit (fed, but not
    /// investing); a low `R̂` with a **low** sated share means food is.
    pub sated_share: f64,
}

impl Default for Measures {
    fn default() -> Measures {
        Measures {
            energy_balance: f64::NAN,
            days_to_empty: None,
            surplus_per_adult_day: f64::NAN,
            r_hat: f64::NAN,
            r_hat_surplus_extrapolated: f64::NAN,
            access: f64::NAN,
            sated_share: f64::NAN,
        }
    }
}

/// `B` within this of 1 is in balance (a float tolerance, not an ecological number).
const BALANCE_TOLERANCE: f64 = 1e-9;

/// The measures of books `b` in context `c`.
pub fn measures(b: &Books, c: &Context) -> Measures {
    let spend = b.spend();
    let energy_balance = if spend > 0.0 {
        b.assimilated / spend
    } else if b.assimilated > 0.0 {
        f64::INFINITY
    } else {
        f64::NAN
    };
    // A full reserve digests exactly what it burns, so a fed body sits at B = 1 to the
    // last bits: below it by rounding is not a deficit.
    let days_to_empty = (energy_balance < 1.0 - BALANCE_TOLERANCE && c.days > 0.0).then(|| {
        let deficit_per_day = (spend - b.assimilated) / c.days;
        if deficit_per_day > 0.0 { c.stores.max(0.0) / deficit_per_day } else { f64::INFINITY }
    });
    let surplus_per_adult_day = if c.adult_days > 0.0 {
        (b.assimilated - spend) / c.adult_days
    } else {
        f64::NAN
    };
    let r_hat = if c.offspring_cost > 0.0 && c.lifespan_days > 0.0 {
        surplus_per_adult_day * c.lifespan_days / c.offspring_cost
    } else {
        f64::NAN
    };
    let access = if b.wanting_ticks > 0 {
        (b.eating_ticks as f64 / b.wanting_ticks as f64).min(1.0)
    } else {
        f64::NAN
    };
    let sated_share = if b.awake_ticks > 0 {
        (b.awake_ticks.saturating_sub(b.wanting_ticks) as f64 / b.awake_ticks as f64).clamp(0.0, 1.0)
    } else {
        f64::NAN
    };
    Measures { energy_balance, days_to_empty, surplus_per_adult_day, r_hat, r_hat_surplus_extrapolated: r_hat, access, sated_share }
}

/// **The soft shortfall** a lineage scored on balance pays: `(1 − min(1, B)) + (1 −
/// min(1, R̂))`, each in `[0, 1]`; a measure with nothing to measure (NaN) counts as
/// missed.
pub fn shortfall(m: &Measures) -> f64 {
    let short = |x: f64| if x.is_nan() { 1.0 } else { (1.0 - x).clamp(0.0, 1.0) };
    short(m.energy_balance) + short(m.r_hat)
}

/// Seconds to world days.
pub fn days(seconds: f64, config: &FaunaConfig) -> f64 {
    let day = config.life.ecological_day_s;
    if day > 0.0 { seconds / day } else { 0.0 }
}

/// Whether a body counts as an adult of its lineage: free (not riding) and at its
/// `birth_body` or more.
pub fn is_adult(a: &Animal, config: &FaunaConfig) -> bool {
    a.founder.is_some_and(|f| a.attached.is_none() && a.body >= config.founder(f).core.birth_body)
}

/// A lineage's stores now: reserve plus digestible stomach over its bodies.
pub fn stores(animals: &[Animal], config: &FaunaConfig, f: Founder) -> f64 {
    let p = config.founder(f);
    animals
        .iter()
        .filter(|a| a.founder == Some(f))
        .map(|a| a.reserve + super::targets::digestible_stomach(a, p))
        .sum()
}

/// The [`Context`] of lineage `f` for an interval of `days` world days with `adult_days`
/// lived, read at its end.
pub fn context(animals: &[Animal], config: &FaunaConfig, f: Founder, adult_days: f64, days: f64) -> Context {
    let core = &config.founder(f).core;
    let r = &core.reproduction;
    let offspring_cost = match r.mode {
        BirthMode::Gestation => core.birth_cost,
        BirthMode::Eggs => r.egg_organic,
    };
    Context {
        stores: stores(animals, config, f),
        adult_days,
        days,
        lifespan_days: self::days(core.lifespan_median_s, config),
        offspring_cost,
    }
}

/// **A predator's kills** over an interval, from the ledgers at both ends: bodies its
/// lineage killed (every prey's `Killed { by }` departures).
pub fn kills(start: &FaunaLedger, end: &FaunaLedger, f: Founder) -> u64 {
    let k = Departure::Killed { by: f }.index();
    Founder::ALL
        .iter()
        .map(|q| end.deaths_by_founder_cause[q.index()][k].saturating_sub(start.deaths_by_founder_cause[q.index()][k]))
        .sum()
}
