//! **The individual's laws**, one representative body at a time: the sim's own rules,
//! written down with their `file:line` (all in `crates/cubarium-voxel-fauna/src/`), on the
//! sim's own config structs ([`FounderPhysiology`], [`Gut`], [`Reproduction`]) and calling
//! their pure methods wherever the sim has one (`Gut::throughput_factor`, `Gut::capacity`,
//! `Gut::s_start`, `Reproduction::clutch_for`, `full_clutch`, `brood_target`,
//! `package_cost`, `surplus_reserve`, `SpeciesConfig::reserve_of`, `hunt::profitability`).
//!
//! The sim steps these at 20 Hz ([`cubarium_voxel_fauna::DT`]); the model steps them at its
//! own `dt` (seconds). Where a per-tick rule compounds (digestion's `S·DT/τ`), the model takes
//! the exact compound over `dt / DT` ticks, so a 1 s step and twenty sim ticks agree.
//!
//! What is **not** here, because it emerges from behaviour in space: how often a mouth is at
//! food (the functional response), how far a body walks (the motor effort), where a
//! dark-active body spends the day, how crowded a nest site is. Those are fitted
//! ([`super::fits`]).

use cubarium_voxel_fauna::{DT, Food, FounderPhysiology, SpeciesConfig};

/// One body's stocks, in the sim's organic units: structure, reserve, the brood earmarked
/// inside the reserve, and the stomach's raw food per class; the eating and emergency latches.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub body: f64,
    pub reserve: f64,
    /// The brood (D1b), held inside the reserve: `brood ≤ reserve` always.
    pub brood: f64,
    pub stomach: [f64; Food::COUNT],
    pub eating: bool,
    pub emergency: bool,
}

impl Body {
    /// Raw food in the stomach, all classes.
    pub fn stomach_total(&self) -> f64 {
        self.stomach.iter().sum()
    }

    /// The organic matter the body holds: structure, reserve and stomach (what a corpse is).
    pub fn organic(&self) -> f64 {
        self.body + self.reserve + self.stomach_total()
    }
}

/// What one step moved through a body, organic units: the ledger lines a budget sheet sums.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flows {
    /// Raw food out of the stomach, digested or passed.
    pub digested: f64,
    /// Into the reserve (the sim's `assimilated_by_founder`).
    pub assimilated: f64,
    /// Out of the gut as dung (to the litter pool).
    pub egested: f64,
    /// Respired at digestion: the absorbed share the room could not take.
    pub digestion_heat: f64,
    /// Basal upkeep respired (`upkeep_by_founder`).
    pub upkeep: f64,
    /// Motor respired (`motor_by_founder`).
    pub motor: f64,
    /// Of upkeep and motor, what came out of the structure (dieback).
    pub from_body: f64,
    /// Reserve built into structure (`growth_by_founder`).
    pub growth: f64,
}

impl std::ops::AddAssign for Flows {
    fn add_assign(&mut self, o: Flows) {
        self.digested += o.digested;
        self.assimilated += o.assimilated;
        self.egested += o.egested;
        self.digestion_heat += o.digestion_heat;
        self.upkeep += o.upkeep;
        self.motor += o.motor;
        self.from_body += o.from_body;
        self.growth += o.growth;
    }
}

impl Flows {
    /// Every line times `k`: a cohort's flows from one body's.
    pub fn scaled(self, k: f64) -> Flows {
        Flows {
            digested: self.digested * k,
            assimilated: self.assimilated * k,
            egested: self.egested * k,
            digestion_heat: self.digestion_heat * k,
            upkeep: self.upkeep * k,
            motor: self.motor * k,
            from_body: self.from_body * k,
            growth: self.growth * k,
        }
    }
}

/// **Hunger**, 0 full to 1 empty: the reserve above the brood short of `reserve_cap · body`
/// (`step.rs:997-1006`; the brood does not sate, D1b).
pub fn hunger(sc: &SpeciesConfig, b: &Body) -> f64 {
    let cap = sc.reserve_of(b.body);
    if !(cap > 0.0) {
        return 0.0;
    }
    let fed = b.reserve - b.brood.min(b.reserve).max(0.0);
    (1.0 - fed / cap).clamp(0.0, 1.0)
}

/// **The emergency latch** (`step.rs:613-618`): on at `wake_hunger`, held until hunger is
/// below `emergency_clear_hunger`.
pub fn emergency(phys: &FounderPhysiology, b: &Body) -> bool {
    let h = hunger(&phys.core, b);
    if b.emergency { h >= phys.emergency_clear_hunger } else { h >= phys.wake_hunger }
}

/// **The eating latch goes on** (`step.rs:692-704`): awake on phase or in an emergency, and
/// the stomach at or under `s_start(hunger) · K`.
pub fn wants_food(phys: &FounderPhysiology, b: &Body) -> bool {
    let h = hunger(&phys.core, b);
    b.stomach_total() <= phys.gut.s_start(h) * phys.gut.capacity(b.body)
}

/// **The full event** (`step.rs:731-751`): the latch goes off at `full_tolerance · K`.
pub fn full(phys: &FounderPhysiology, b: &Body) -> bool {
    b.stomach_total() >= phys.gut.full_tolerance * phys.gut.capacity(b.body)
}

/// **Respiration out of a body** (`step.rs:355-390` with `withdraw`, `step.rs:398-431`): the
/// reserve first, then the structure; the brood pays its share `brood_share` of what the
/// reserve paid (D1b). Returns (paid, of which from the structure).
pub fn respire(sc: &SpeciesConfig, b: &mut Body, want: f64) -> (f64, f64) {
    if !(want > 0.0) {
        return (0.0, 0.0);
    }
    let from_reserve = want.min(b.reserve.max(0.0));
    let from_body = (want - from_reserve).min(b.body.max(0.0));
    b.reserve -= from_reserve;
    b.body -= from_body;
    if b.brood > 0.0 {
        b.brood = (b.brood - sc.reproduction.brood_share * from_reserve).max(0.0);
    }
    b.brood = b.brood.min(b.reserve.max(0.0));
    (from_reserve + from_body, from_body)
}

/// **Basal upkeep wanted**, organic units per second (`step.rs:878-898`):
/// `maintenance_per_s · body`, times `sleep_upkeep` asleep. `awake` is the share of the step
/// the body is awake (the mean field's awake fraction).
pub fn upkeep_per_s(phys: &FounderPhysiology, body: f64, awake: f64) -> f64 {
    phys.core.maintenance_per_s * body * (awake + (1.0 - awake) * phys.sleep_upkeep)
}

/// **Motor respiration wanted**, organic units per second (`step.rs:1564-1579`):
/// `motor_respiration_per_s · body · (equivalent displacement / cruise)`; `effort` is that
/// last ratio averaged over awake time (fitted: how much an awake body walks).
pub fn motor_per_s(phys: &FounderPhysiology, body: f64, awake: f64, effort: f64) -> f64 {
    phys.motor_respiration_per_s * body * awake * effort
}

/// The share of a stomach pool digested over `dt` seconds at time constant `tau`: the sim
/// takes `S · DT / τ` each tick (`step.rs:843`), so over `dt / DT` ticks
/// `1 − (1 − DT/τ)^(dt/DT)`.
pub fn digested_share(tau: f64, dt: f64) -> f64 {
    if !(tau > 0.0) {
        return 1.0;
    }
    let per_tick = (DT / tau).min(1.0);
    1.0 - (1.0 - per_tick).powf(dt / DT)
}

/// **Digestion and assimilation** over `dt` (`step.rs:822-876` and `assimilate`,
/// `step.rs:2705-2766`): each class's pool empties at `S/τ`, `τ` shortened for a small body
/// by the gut's mass exponent (`Gut::throughput_factor`); the classes' yields are scaled
/// together to the reserve's room `reserve_cap · body − reserve`; what the yield does not
/// absorb is dung; a mature body earmarks `brood_share` of what was placed as its brood,
/// up to `Reproduction::brood_target`.
///
/// **Mineral is assumed to fund every yield** (the sim's `min(yield · organic, mineral /
/// n_tissue)` is not modelled: the pools' mineral is the other half's), so `y_eff = yield`.
pub fn digest(phys: &FounderPhysiology, b: &mut Body, dt: f64) -> Flows {
    let sc = &phys.core;
    let mut f = Flows::default();
    if !(b.stomach_total() > 0.0) {
        return f;
    }
    let speed = phys.gut.throughput_factor(b.body, sc.body_max);
    let room = (sc.reserve_of(b.body) - b.reserve).max(0.0);
    let mut raw = [0.0f64; Food::COUNT];
    let mut yields = 0.0;
    for food in Food::ALL {
        let p = b.stomach[food.index()];
        if !(p > 0.0) {
            continue;
        }
        let tau = phys.gut.digestion_s[food.index()] / speed;
        raw[food.index()] = p * digested_share(tau, dt);
        yields += raw[food.index()] * phys.yield_for(food);
    }
    let scale = if yields > room && yields > 0.0 { room / yields } else { 1.0 };
    for food in Food::ALL {
        let d = raw[food.index()] * scale;
        if !(d > 0.0) {
            continue;
        }
        b.stomach[food.index()] = (b.stomach[food.index()] - d).max(0.0);
        let y = phys.yield_for(food).clamp(0.0, 1.0);
        let placed = (y * d).min((sc.reserve_of(b.body) - b.reserve).max(0.0));
        b.reserve += placed;
        let rule = sc.reproduction;
        if rule.brood_share > 0.0 && b.body >= sc.birth_body {
            let target = rule.brood_target(sc, b.body);
            if b.brood < target {
                b.brood = (b.brood + rule.brood_share * placed).min(target);
            }
        }
        f.digested += d;
        f.assimilated += placed;
        f.digestion_heat += y * d - placed;
        f.egested += (1.0 - y) * d;
    }
    f
}

/// **Growth** over `dt` (`step.rs:959-993`): structure is built out of the reserve above the
/// brood at `growth_max_per_s · fullness`, `fullness` the reserve over `reserve_cap · body`
/// (at most 1), never past `body_max` or the free reserve. Returns what was built.
pub fn grow(sc: &SpeciesConfig, b: &mut Body, dt: f64) -> f64 {
    b.brood = b.brood.min(b.reserve.max(0.0)).max(0.0);
    let free = b.reserve - b.brood;
    let room = (sc.body_max - b.body).max(0.0);
    if !(room > 0.0) || !(free > 0.0) {
        return 0.0;
    }
    let cap = sc.reserve_of(b.body);
    let fullness = if cap > 0.0 { (b.reserve / cap).min(1.0) } else { 1.0 };
    let built = (sc.growth_max_per_s * dt * fullness).min(room).min(free);
    b.reserve -= built;
    b.body += built;
    built
}

/// **The bite the mouth asks for** over `dt` at full effort and food under its nose
/// (`gut_bite`, `step.rs:796-810`): `bite_per_body_s · throughput · body · dt`, never more
/// than reaches the stomach's room `K − S` after a sloppy feeder's spill. The stock's
/// diminishing `E / (E + bite_half_stock)` and the share of time the mouth is at food are
/// the fitted functional response, applied by the caller.
pub fn bite_want(phys: &FounderPhysiology, b: &Body, dt: f64) -> f64 {
    let want = phys.gut.bite_per_body_s * phys.gut.throughput_factor(b.body, phys.core.body_max) * b.body * dt;
    let room = (phys.gut.capacity(b.body) - b.stomach_total()).max(0.0);
    let reaches = (1.0 - phys.bite_spill_fraction.clamp(0.0, 1.0)).max(1e-12);
    want.max(0.0).min(room / reaches)
}

/// **The gut ceiling**: assimilation per second of a body whose stomach is full and whose
/// reserve has room, `yield · k · body · throughput / τ` for one food class. The yardstick a
/// realized intake is measured against.
pub fn gut_ceiling_per_s(phys: &FounderPhysiology, body: f64, food: Food) -> f64 {
    let tau = phys.gut.digestion_s[food.index()];
    if !(tau > 0.0) {
        return 0.0;
    }
    phys.yield_for(food) * phys.gut.capacity(body) * phys.gut.throughput_factor(body, phys.core.body_max) / tau
}

/// **The wound one strike makes** (`hunt::profitability`, `hunt.rs:218-224`, the wound
/// formula of `apply_hit`, `step.rs:2207-2229`): `min(1, power · S_pred/S_prey · (1 − armour))`.
/// A prey whose share is 1 dies to one hit.
pub fn strike_share(hunter: &FounderPhysiology, hunter_body: f64, prey: &FounderPhysiology, prey_body: f64) -> f64 {
    match hunter.hunter {
        Some(h) => cubarium_voxel_fauna::profitability(&h, hunter_body, prey_body, prey.armour),
        None => 0.0,
    }
}

/// Whether a prey of `prey_body` is inside a hunter's size window (`Hunter::prey_max_ratio`,
/// `body.rs:806-831`): at most `ratio ×` the hunter's own body.
pub fn in_size_window(hunter: &FounderPhysiology, hunter_body: f64, prey_body: f64) -> bool {
    hunter.hunter.is_some_and(|h| prey_body <= h.prey_max_ratio * hunter_body)
}

/// **A lifespan draw's survival** at age `age_s` (`draw_lifespan_ticks`, `lib.rs:2210-2233`):
/// the span is uniform on `[min, min + span] × median`; an arriving founder's remaining
/// span is that times a uniform `[arriving_share_min, 1)`. The share of a cohort still alive.
pub fn lifespan_survival(life: &cubarium_voxel_fauna::LifeConfig, median_s: f64, age_s: f64, arriving: bool) -> f64 {
    if !(median_s > 0.0) {
        return 1.0;
    }
    let lo = life.lifespan_draw_min * median_s;
    let hi = (life.lifespan_draw_min + life.lifespan_draw_span) * median_s;
    let uni = |t: f64, lo: f64, hi: f64| -> f64 {
        if t <= lo {
            1.0
        } else if t >= hi {
            0.0
        } else {
            (hi - t) / (hi - lo)
        }
    };
    if !arriving {
        return uni(age_s, lo, hi);
    }
    // Remaining span L·U with L ~ U[lo, hi], U ~ U[a, 1): integrate over U (64 points).
    let a = life.arriving_share_min.clamp(0.0, 1.0);
    let n = 64;
    let mut s = 0.0;
    for k in 0..n {
        let u = a + (1.0 - a) * (k as f64 + 0.5) / n as f64;
        s += uni(age_s, lo * u, hi * u);
    }
    s / n as f64
}
