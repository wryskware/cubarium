use crate::organism::DeathCause;

use super::*;

use super::state::WorldState;

impl World {
    /// Water budget residual (`design/water.md`): `Σw − (rain_in_total − evap_out_total)`.
    /// Zero to rounding for a world created dry; telemetry consumers check it like the
    /// mass residual.
    pub fn water_residual(&self) -> f64 {
        self.state.fields.w.iter().sum::<f64>()
            - (self.state.rain_in_total - self.state.evap_out_total)
    }

    /// Mass invariant: `Σ fields + Σ organisms (incl. escrow) − external_material_in
    /// − feed_material_in + clean_material_out − initial`. Should stay within
    /// `1e-9 · initial` per hour of simulated time; telemetry reports it. Fed crumbs are
    /// material admitted from outside and cleaned litter is material exported, exactly like
    /// the founders in `external_material_in`.
    ///
    /// With the hunter extension the same rule applies to it: a carried carcass
    /// (`Σ gut_material`) is material still inside the world, and the hunter founders and any
    /// budget-matched control deposit are material admitted from outside
    /// (`HunterState::imported_material`), booked once in the extension and never again in
    /// `external_material_in`.
    pub fn mass_residual(&self) -> f64 {
        let organisms: f64 = self.state.organisms.iter().map(|(_, o)| o.material()).sum();
        self.state.fields.total_material() + organisms + self.state.hunters.gut_material_total()
            - self.state.external_material_in
            - self.state.care.feed_material_in
            + self.state.care.clean_material_out
            - self.state.hunters.imported_material()
            - self.initial_material
    }

    /// Full invariant check (fields finite/nonnegative, organisms finite, positions canonical,
    /// escrows consistent, population ≤ cap). Called every tick in debug builds and every
    /// telemetry sample in release; a failure is a fatal implementation error.
    pub fn check_invariants(&self) -> Result<(), String> {
        let cfg = &self.state.config;
        self.state.fields.check(cfg.detritus.energy_cap)?;
        // The care ledgers and any shower's progress are runtime invariants too: catching a
        // defect here stops a checkpoint that would not load back. So are the compensated
        // energy ledgers.
        self.state.care.validate(self.state.tick)?;
        self.state.energy_ledgers().validate()?;
        self.state
            .hunters
            .validate(self.state.tick, &self.state.organisms, &self.state.config)?;
        let cap = cfg.capacity.max_organisms as usize;
        if self.state.organisms.len() > cap {
            return Err(format!(
                "population {} exceeds cap {cap}",
                self.state.organisms.len()
            ));
        }
        for (id, o) in self.state.organisms.iter() {
            let who = format!("organism {}:{}", id.slot, id.generation);
            if !o.structure.is_finite() || o.structure < 0.0 {
                return Err(format!("{who}: structure = {}", o.structure));
            }
            if !o.reserve.is_finite() || o.reserve < 0.0 {
                return Err(format!("{who}: reserve = {}", o.reserve));
            }
            if !o.energy.is_finite() || o.energy < 0.0 {
                return Err(format!("{who}: energy = {}", o.energy));
            }
            if !o.hunger_memory.is_finite() {
                return Err(format!("{who}: hunger memory is not finite"));
            }
            if !o.pos.is_canonical() {
                return Err(format!("{who}: position {:?} is not canonical", o.pos));
            }
            if !o.heading.is_finite() || (o.heading.length() - 1.0).abs() > HEADING_TOLERANCE {
                return Err(format!(
                    "{who}: heading {:?} is not a unit vector",
                    o.heading
                ));
            }
            if !o.ou.is_finite() {
                return Err(format!("{who}: OU vector is not finite"));
            }
            if let Some(e) = &o.escrow {
                if !(e.structure.is_finite() && e.reserve.is_finite() && e.energy.is_finite()) {
                    return Err(format!("{who}: escrow is not finite"));
                }
                if e.structure < 0.0 || e.reserve < 0.0 || e.energy < 0.0 {
                    return Err(format!("{who}: escrow is negative"));
                }
                if e.started_tick > self.state.tick {
                    return Err(format!("{who}: escrow starts after the current tick"));
                }
            }
        }
        Ok(())
    }

    /// Death causes in `deaths_total` order.
    pub const DEATH_CAUSES: [DeathCause; 3] = [
        DeathCause::Starvation,
        DeathCause::Age,
        DeathCause::Collapse,
    ];
}

/// The audited energy total of `design/m2-world-spec.md` "Units and quantities":
/// `Σ_cells (e_p·P + e_f·F + De) + Σ_organisms (E + e_r·R) + Σ_escrow (e_r·(S_c + R_c) + E_c)`.
/// Reserve material carries chemical energy; structure does not; fruit carries `e_f`.
///
/// A hunter's carried carcass is stored energy too: `Σ gut_energy` is exactly the energy that
/// was removed from the prey, held until it is digested, rejected or released by the hunter's
/// own death (`crate::hunter`).
#[cfg(any(debug_assertions, test))]
pub(super) fn stored_energy(state: &WorldState) -> f64 {
    let e_p = state.config.producer.energy_density;
    let e_f = state.config.fruit.energy_density;
    let e_r = state.config.organism.reserve_energy_density;
    let cells: f64 = state.fields.p.iter().map(|p| e_p * p).sum::<f64>()
        + state.fields.f.iter().map(|f| e_f * f).sum::<f64>()
        + state.fields.de.iter().sum::<f64>();
    let organisms: f64 = state
        .organisms
        .iter()
        .map(|(_, o)| {
            o.energy
                + e_r * o.reserve
                + o.escrow
                    .as_ref()
                    .map_or(0.0, |e| e_r * (e.structure + e.reserve) + e.energy)
        })
        .sum();
    cells + organisms + state.hunters.gut_energy_total()
}

/// Edible detritus `D_eff = D · min(1, ρ / e_r)` with `ρ = De / D` (zero when `D == 0`),
/// from `design/m2-world-spec.md` "Controller". Detritus too energy-poor to pay for its own
/// reserve storage is not food: this is the same `min(1, ρ / e_r)` factor that scales
/// scavenging assimilation, so what an organism sees and what it can digest agree.
pub(super) fn edible_detritus(detritus: f64, energy: f64, e_r: f64) -> f64 {
    if detritus <= 0.0 {
        return 0.0;
    }
    let rho = energy / detritus;
    if e_r > 0.0 {
        detritus * (rho / e_r).min(1.0)
    } else {
        detritus
    }
}
