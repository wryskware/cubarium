use crate::config::WorldConfig;
use crate::organism::Organism;

use super::*;

/// Rounding slack for the persisted gut bound and the adult-structure gate.
pub const TOLERANCE: f64 = 1e-9;

/// The gut residue below which the remainder is flushed as heat rather than carried: the
/// last few ulps of a finished meal, never a hidden discard of real food.
pub const GUT_RESIDUE: f64 = 1e-12;

// ---------------------------------------------------------------- pure rules

/// The metabolic constants digestion reads out of the world config: reserve energy density,
/// and the two assimilation efficiencies. Exactly the ones ordinary feeding uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metabolism {
    pub e_r: f64,
    pub eta_m: f64,
    pub eta_e: f64,
}

impl Metabolism {
    pub fn of(config: &WorldConfig) -> Metabolism {
        let o = &config.organism;
        Metabolism {
            e_r: o.reserve_energy_density,
            eta_m: o.assimilation_material,
            eta_e: o.assimilation_energy,
        }
    }
}

/// One creature's per-tick movement bill, in the world's own terms: the part that does not
/// depend on speed (maintenance and sensing) and the part that does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveBill {
    pub structure: f64,
    pub maintenance: f64,
    pub sense_radius: f64,
    pub move_cost: f64,
    pub sense_cost: f64,
}

impl MoveBill {
    pub fn of(o: &Organism, config: &WorldConfig) -> MoveBill {
        MoveBill {
            structure: o.structure,
            maintenance: o.phenotype.maintenance,
            sense_radius: o.phenotype.sense_radius,
            move_cost: config.organism.move_cost,
            sense_cost: config.organism.sense_cost,
        }
    }

    /// The fastest speed this creature can pay for out of `energy` in one tick, never below
    /// `ordinary_speed`: a boost is limited by the movement energy actually available, and
    /// ordinary movement keeps its existing semantics (pay what you can, then run out).
    pub fn affordable_speed(
        &self,
        energy: f64,
        dt: f64,
        ordinary_speed: f64,
        wanted_speed: f64,
    ) -> f64 {
        if wanted_speed <= ordinary_speed {
            return wanted_speed;
        }
        let fixed = (self.maintenance * self.structure + self.sense_cost * self.sense_radius) * dt;
        let per_speed = self.move_cost * self.structure * dt;
        if per_speed <= 0.0 {
            return wanted_speed;
        }
        let budget = (energy - fixed).max(0.0) / per_speed;
        wanted_speed.min(budget).max(ordinary_speed)
    }
}

/// The prey inventory a capture would transfer: `M = S + R (+ escrow S + R)` and
/// `Q = E + e_r · R (+ escrow E + e_r · (S + R))`. Structure carries no energy.
pub fn prey_inventory(prey: &Organism, e_r: f64) -> (f64, f64) {
    let mut material = prey.structure + prey.reserve;
    let mut energy = prey.energy + e_r * prey.reserve;
    if let Some(es) = &prey.escrow {
        material += es.structure + es.reserve;
        energy += es.energy + e_r * (es.structure + es.reserve);
    }
    (material, energy)
}

/// `clamp(capture_base · S_h / (S_h + S_p), capture_min, capture_max)`.
pub fn capture_probability(
    profile: &FixedHunterProfile,
    hunter_structure: f64,
    prey_structure: f64,
) -> f64 {
    let total = hunter_structure + prey_structure;
    if !total.is_finite() || total <= 0.0 {
        return profile.capture_min;
    }
    (profile.capture_base * hunter_structure / total)
        .clamp(profile.capture_min, profile.capture_max)
}

/// Is this prey inside the eligibility window, and does its whole inventory fit the gut
/// headroom? Membership is checked by the caller, which owns the member list.
pub fn prey_is_eligible(
    profile: &FixedHunterProfile,
    hunter: &Organism,
    prey: &Organism,
    gut_headroom: f64,
    e_r: f64,
) -> bool {
    if prey.structure < profile.prey_structure_min {
        return false;
    }
    if prey.structure > profile.prey_structure_fraction_max * hunter.structure {
        return false;
    }
    let (material, _) = prey_inventory(prey, e_r);
    material <= gut_headroom + TOLERANCE
}

/// The one paid-offspring gate, on **local parent state only**: no world population is
/// consulted, and the ordinary organism thresholds are overridden by the saved profile rather
/// than changed for every creature.
pub fn may_reproduce(
    profile: &FixedHunterProfile,
    parent: &Organism,
    member: &HunterMember,
    now: u64,
    dt: f64,
) -> bool {
    parent.escrow.is_none()
        && !member.carrying()
        && member.target.is_none()
        && !member.phase.hunting()
        && parent.structure >= parent.phenotype.structure_adult - TOLERANCE
        && parent.reserve >= profile.reproduce_reserve_fraction * parent.phenotype.reserve_max
        && parent.energy >= profile.reproduce_energy_fraction * parent.phenotype.energy_max
        && parent.age_ticks(now) as f64 * dt >= profile.reproduce_min_age_seconds
        && now >= member.next_reproduction_tick
}

/// One tick of digestion of homogeneous gut contents, in the world's own currencies.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DigestStep {
    /// Material removed from the gut this tick (`q`).
    pub material: f64,
    /// Energy removed with it (`carried = gut_energy · q / gut_material`).
    pub carried: f64,
    /// Material stored as reserve (`a`).
    pub to_reserve: f64,
    /// Material rejected into the cell's detritus, carrying zero energy (`q − a`).
    pub to_detritus: f64,
    /// Usable energy gained.
    pub energy_gain: f64,
    /// Everything else the meal released.
    pub heat: f64,
}

/// The energy-honest digestion step of the implementation plan.
///
/// `a = eta_m · min(q, carried / e_r)`, so prey that is mostly structure and almost no
/// reserve stores proportionally less material — the same `min(1, ρ / e_r)` factor ordinary
/// scavenging uses. `q` is then reduced consistently rather than clamping `a`, so the
/// material and energy leaving the gut always describe the same portion.
pub fn digest_step(
    profile: &FixedHunterProfile,
    dt: f64,
    gut_material: f64,
    gut_energy: f64,
    metabolism: Metabolism,
    reserve_headroom: f64,
    energy_headroom: f64,
) -> DigestStep {
    let Metabolism { e_r, eta_m, eta_e } = metabolism;
    if gut_material <= 0.0 || reserve_headroom <= 0.0 {
        return DigestStep::default();
    }
    let mut q = (profile.digest_rate * dt).min(gut_material);
    if q <= 0.0 {
        return DigestStep::default();
    }
    let density = gut_energy / gut_material;
    let eta = if e_r > 0.0 {
        eta_m * (density / e_r).min(1.0)
    } else {
        eta_m
    };
    if eta <= 0.0 {
        return DigestStep::default();
    }
    let mut to_reserve = eta * q;
    if to_reserve > reserve_headroom {
        // Reduce the portion, not the assimilation: material and energy must describe the
        // same bite.
        q *= reserve_headroom / to_reserve;
        to_reserve = eta * q;
    }
    let carried = density * q;
    // `eta <= eta_m · ρ / e_r` keeps this nonnegative analytically; only rounding-scale
    // residue is clamped.
    let spare = carried - e_r * to_reserve;
    debug_assert!(spare > -1e-9, "digestion spare energy {spare} is negative");
    let spare = spare.max(0.0);
    let energy_gain = (eta_e * spare).clamp(0.0, energy_headroom.max(0.0));
    DigestStep {
        material: q,
        carried,
        to_reserve,
        to_detritus: q - to_reserve,
        energy_gain,
        heat: spare - energy_gain,
    }
}
