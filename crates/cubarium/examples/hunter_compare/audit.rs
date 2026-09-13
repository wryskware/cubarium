//! Common PRE-import inventories and independent short-window flow accounting.
//! Observer only: no World mutation, RNG, baseline resets or relaxed tolerances.

use anyhow::{Result, ensure};
use cubarium_core::{EnergyLedgers, Telemetry, WorldState};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Inventory {
    pub material: f64,
    pub energy: f64,
    pub water: f64,
}

impl Inventory {
    pub fn read(s: &WorldState) -> Self {
        let er = s.config.organism.reserve_energy_density;
        Self {
            material: s.fields.total_material()
                + s.organisms.iter().map(|(_, o)| o.material()).sum::<f64>()
                + s.hunters.gut_material_total(),
            energy: s.fields.p.iter().sum::<f64>() * s.config.producer.energy_density
                + s.fields.f.iter().sum::<f64>() * s.config.fruit.energy_density
                + s.fields.de.iter().sum::<f64>()
                + s.organisms
                    .iter()
                    .map(|(_, o)| {
                        o.energy
                            + er * o.reserve
                            + o.escrow
                                .as_ref()
                                .map_or(0.0, |e| e.energy + er * (e.structure + e.reserve))
                    })
                    .sum::<f64>()
                + s.hunters.gut_energy_total(),
            water: s.fields.w.iter().sum(),
        }
    }

    fn values(self) -> [f64; 3] {
        [self.material, self.energy, self.water]
    }
}

#[derive(Clone, Copy, Default, Serialize)]
pub struct Sum {
    raw: f64,
    correction: f64,
}
impl Sum {
    pub fn add(&mut self, x: f64) {
        let next = self.raw + x;
        self.correction += if self.raw.abs() >= x.abs() {
            (self.raw - next) + x
        } else {
            (x - next) + self.raw
        };
        self.raw = next;
    }
    pub fn value(self) -> f64 {
        self.raw + self.correction
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Peak {
    pub magnitude: f64,
    pub signed_at_peak: f64,
    pub tick_at_peak: u64,
    pub first_crossing: Option<u64>,
    pub nonfinite_at: Option<u64>,
}
impl Peak {
    fn observe(&mut self, x: f64, limit: f64, tick: u64) {
        if !x.is_finite() {
            self.nonfinite_at.get_or_insert(tick);
            self.first_crossing.get_or_insert(tick);
            return;
        }
        if x.abs() > self.magnitude {
            self.magnitude = x.abs();
            self.signed_at_peak = x;
            self.tick_at_peak = tick;
        }
        if x.abs() >= limit {
            self.first_crossing.get_or_insert(tick);
        }
    }
    fn passed(&self) -> bool {
        self.first_crossing.is_none()
    }
}

#[derive(Serialize)]
pub struct AuditReport {
    pub opening_tick: u64,
    pub opening: Inventory,
    pub fixed_limits: [f64; 3],
    pub material: Peak,
    pub corrected_energy: Peak,
    pub water: Peak,
    pub independent_energy: Peak,
    pub initializer_boundary: [Peak; 3],
    pub legacy_raw_energy: Peak,
    pub passed: bool,
    pub legacy_passed: bool,
    pub failure: Option<String>,
    pub actual_receipt_material: f64,
    pub actual_receipt_energy: f64,
    pub actual_receipt_heat: f64,
    pub observer_light: Sum,
    pub observer_heat: Sum,
}

/// Saved common opening, not World's reconstructed per-instance mass baseline.
pub struct Audit {
    pub report: AuditReport,
    ledgers: EnergyLedgers,
    external_material: f64,
    hunter_material: f64,
    hunter_energy: f64,
    care: [f64; 4],
    rain: f64,
    evap: f64,
    initialized: bool,
}
impl Audit {
    /// Caller constructs each arm with World::from_state (empty transient flow
    /// counters), or explicitly resets telemetry at this same opening boundary.
    pub fn new(s: &WorldState) -> Result<Self> {
        let opening = Inventory::read(s);
        ensure!(
            opening.values().iter().all(|x| x.is_finite() && *x >= 0.0),
            "invalid audit opening"
        );
        Ok(Self {
            report: AuditReport {
                opening_tick: s.tick,
                opening,
                fixed_limits: opening.values().map(|x| 1e-8 * x.max(1.0)),
                material: Peak::default(),
                corrected_energy: Peak::default(),
                water: Peak::default(),
                independent_energy: Peak::default(),
                legacy_raw_energy: Peak::default(),
                initializer_boundary: std::array::from_fn(|_| Peak::default()),
                passed: false,
                legacy_passed: false,
                failure: None,
                actual_receipt_material: 0.0,
                actual_receipt_energy: 0.0,
                actual_receipt_heat: 0.0,
                observer_light: Sum::default(),
                observer_heat: Sum::default(),
            },
            ledgers: s.energy_ledgers(),
            external_material: s.external_material_in,
            hunter_material: s.hunters.imported_material(),
            hunter_energy: s.hunters.imported_energy(),
            care: [
                s.care.feed_material_in,
                s.care.clean_material_out,
                s.care.feed_energy_in,
                s.care.clean_energy_out,
            ],
            rain: s.rain_in_total,
            evap: s.evap_out_total,
            initialized: false,
        })
    }

    /// The initializer's immediate heat is absent from current step telemetry;
    /// include the actual control receipt once, before observing the first tick.
    pub fn initialize(
        &mut self,
        s: &WorldState,
        material: f64,
        energy: f64,
        heat: f64,
    ) -> Result<()> {
        let result = self.initialize_inner(s, material, energy, heat);
        self.record_failure(&result);
        result
    }

    fn initialize_inner(
        &mut self,
        s: &WorldState,
        material: f64,
        energy: f64,
        heat: f64,
    ) -> Result<()> {
        ensure!(!self.initialized, "observer initializer repeated");
        ensure!(
            s.tick == self.report.opening_tick,
            "initializer advanced world"
        );
        ensure!(
            [material, energy, heat]
                .iter()
                .all(|x| x.is_finite() && *x >= 0.0),
            "invalid receipt"
        );
        self.initialized = true;
        self.report.actual_receipt_material = material;
        self.report.actual_receipt_energy = energy;
        self.report.actual_receipt_heat = heat;
        self.report.observer_heat.add(heat);
        let before = self.report.opening;
        let after = Inventory::read(s);
        let residuals = [
            after.material - before.material - material,
            after.energy - before.energy - energy + heat,
            after.water - before.water,
        ];
        for (i, residual) in residuals.into_iter().enumerate() {
            self.report.initializer_boundary[i].observe(
                residual,
                self.report.fixed_limits[i],
                s.tick,
            );
        }
        // Receipt-to-ledger identity is independent of inventory arithmetic.
        for (value, limit) in [
            (
                s.hunters.imported_material() - self.hunter_material - material,
                self.report.fixed_limits[0],
            ),
            (
                s.hunters.imported_energy() - self.hunter_energy - energy,
                self.report.fixed_limits[1],
            ),
            (
                s.energy_ledgers().heat_out.since(self.ledgers.heat_out) - heat,
                self.report.fixed_limits[1],
            ),
        ] {
            ensure!(
                value.is_finite() && value.abs() < limit,
                "initializer receipt/ledger mismatch"
            );
        }
        self.observe(s, 0.0, 0.0)
    }

    /// Counters are cumulative since last telemetry reset, NOT one tick's flow.
    pub fn observe(&mut self, s: &WorldState, light: f64, heat: f64) -> Result<()> {
        let result = self.observe_inner(s, light, heat);
        self.record_failure(&result);
        result
    }

    fn record_failure(&mut self, result: &Result<()>) {
        if let Err(error) = result {
            self.report.passed = false;
            self.report.failure.get_or_insert_with(|| error.to_string());
        }
    }

    fn observe_inner(&mut self, s: &WorldState, light: f64, heat: f64) -> Result<()> {
        ensure!(self.initialized, "audit observed before initialization");
        ensure!(self.report.failure.is_none(), "audit already failed");
        let now = Inventory::read(s);
        let r = &mut self.report;
        let dhm = s.hunters.imported_material() - self.hunter_material;
        let dhe = s.hunters.imported_energy() - self.hunter_energy;
        // This trial admits exactly one actual receipt. A second unreceipted
        // import cannot be made invisible by increasing its source ledger too.
        ensure!(
            dhm == r.actual_receipt_material && dhe == r.actual_receipt_energy,
            "hunter import ledger changed without another authorized receipt"
        );
        ensure!(
            s.external_material_in == self.external_material,
            "primary trial changed external material without a receipt"
        );
        let dcm =
            (s.care.feed_material_in - self.care[0]) - (s.care.clean_material_out - self.care[1]);
        let dce = (s.care.feed_energy_in - self.care[2]) - (s.care.clean_energy_out - self.care[3]);
        let dq = now.energy - r.opening.energy;
        r.material.observe(
            now.material
                - r.opening.material
                - (s.external_material_in - self.external_material)
                - dhm
                - dcm,
            r.fixed_limits[0],
            s.tick,
        );
        r.corrected_energy.observe(
            dq - s.energy_ledgers().net_since(self.ledgers) - dhe - dce,
            r.fixed_limits[1],
            s.tick,
        );
        r.legacy_raw_energy.observe(
            dq - (s.light_in_total - self.ledgers.light_in.raw)
                + (s.heat_out_total - self.ledgers.heat_out.raw)
                - dhe
                - dce,
            r.fixed_limits[1],
            s.tick,
        );
        r.water.observe(
            now.water - r.opening.water - (s.rain_in_total - self.rain)
                + (s.evap_out_total - self.evap),
            r.fixed_limits[2],
            s.tick,
        );
        // Primary trial is care-free. Never fabricate independent care receipts
        // from persisted source ledgers if a caller accidentally enables care.
        ensure!(
            [
                s.care.feed_material_in,
                s.care.clean_material_out,
                s.care.feed_energy_in,
                s.care.clean_energy_out
            ] == self.care
                && s.care.admitted_seq == 0
                && s.care.showers.is_empty(),
            "primary trial admitted care"
        );
        r.independent_energy.observe(
            dq - r.observer_light.value() - light + r.observer_heat.value() + heat
                - r.actual_receipt_energy,
            r.fixed_limits[1],
            s.tick,
        );
        r.passed = r.material.passed()
            && r.corrected_energy.passed()
            && r.water.passed()
            && r.independent_energy.passed()
            && r.initializer_boundary.iter().all(Peak::passed);
        r.legacy_passed = r.material.passed() && r.water.passed() && r.legacy_raw_energy.passed();
        ensure!(
            r.passed,
            "strict PRE-import resource audit failed at tick {}",
            s.tick
        );
        Ok(())
    }

    /// Call exactly once with the counters returned by a telemetry reset.
    pub fn close_window(&mut self, counters: &Telemetry) {
        self.report.observer_light.add(counters.light_in);
        self.report.observer_heat.add(counters.heat_out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::{FixedHunterProfile, HunterTarget, World, WorldConfig};

    fn target() -> HunterTarget {
        HunterTarget {
            face: 4,
            u: 12.0,
            v: 8.0,
        }
    }

    #[test]
    fn fixed_opening_includes_the_actual_paid_founder_once() {
        let mut world = World::new(WorldConfig::default()).unwrap();
        let mut audit = Audit::new(&world.state).unwrap();
        let opening = audit.report.opening.material;
        let q = world
            .start_hunter_trial(
                FixedHunterProfile::lanternjaw_trial(world.config()),
                target(),
            )
            .unwrap();
        audit
            .initialize(&world.state, q.material_in, q.energy_in, 0.0)
            .unwrap();
        assert_eq!(audit.report.opening.material, opening);
        assert!(
            audit
                .initialize(&world.state, q.material_in, q.energy_in, 0.0)
                .is_err()
        );
    }

    #[test]
    fn missing_receipt_is_not_hidden_by_a_new_world_baseline() {
        let mut world = World::new(WorldConfig::default()).unwrap();
        let mut audit = Audit::new(&world.state).unwrap();
        world
            .start_hunter_trial(
                FixedHunterProfile::lanternjaw_trial(world.config()),
                target(),
            )
            .unwrap();
        let resumed = World::from_state(world.state.clone()).unwrap();
        assert!(resumed.mass_residual().abs() < 1e-9);
        assert!(audit.initialize(&resumed.state, 0.0, 0.0, 0.0).is_err());
        assert!(!audit.report.passed);
        assert!(audit.report.failure.is_some());
    }

    #[test]
    fn budget_cap_heat_is_counted_once_before_the_first_window() {
        let mut config = WorldConfig::default();
        config.detritus.energy_cap = 0.01;
        let mut world = World::new(config).unwrap();
        let mut audit = Audit::new(&world.state).unwrap();
        let q = world
            .deposit_hunter_budget_control(
                FixedHunterProfile::lanternjaw_trial(world.config()),
                target(),
            )
            .unwrap();
        assert!(q.energy_heat > 0.0);
        assert_eq!(world.telemetry().heat_out, 0.0);
        audit
            .initialize(&world.state, q.material_in, q.energy_in, q.energy_heat)
            .unwrap();
        for _ in 0..20 {
            let sample = world.step();
            let flows = (sample.light_in, sample.heat_out);
            audit.observe(&world.state, flows.0, flows.1).unwrap();
        }
        audit.close_window(&world.telemetry());
        audit.observe(&world.state, 0.0, 0.0).unwrap();
    }

    #[test]
    fn exact_limit_and_nonfinite_are_failures_not_ignored_maxima() {
        for value in [1.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut peak = Peak::default();
            peak.observe(value, 1.0, 42);
            assert!(!peak.passed());
            assert_eq!(peak.first_crossing, Some(42));
            serde_json::to_string(&peak).unwrap();
        }
    }

    #[test]
    fn observer_cadence_does_not_change_the_no_hunter_world() {
        let opening = World::new(WorldConfig::default()).unwrap().state;
        let run = |period| {
            let mut world = World::from_state(opening.clone()).unwrap();
            let mut audit = Audit::new(&opening).unwrap();
            audit.initialize(&world.state, 0.0, 0.0, 0.0).unwrap();
            for t in 1..=400 {
                let counters = world.step();
                let flows = (counters.light_in, counters.heat_out);
                audit.observe(&world.state, flows.0, flows.1).unwrap();
                if t % period == 0 {
                    audit.close_window(&world.telemetry());
                }
            }
            cubarium_core::snapshot::state_hash(&world.state)
        };
        assert_eq!(run(20), run(200));
    }

    #[test]
    fn balanced_but_unreceipted_imports_fail_and_stay_failed() {
        for hunter_ledger in [false, true] {
            let mut world = World::new(WorldConfig::default()).unwrap();
            let mut audit = Audit::new(&world.state).unwrap();
            audit.initialize(&world.state, 0.0, 0.0, 0.0).unwrap();
            world.state.fields.d[0] += 1.0;
            if hunter_ledger {
                world.state.hunters.founder_material_in += 1.0;
            } else {
                world.state.external_material_in += 1.0;
            }
            assert!(audit.observe(&world.state, 0.0, 0.0).is_err());
            assert!(!audit.report.passed);
            assert!(audit.report.failure.is_some());
            world.state.fields.d[0] -= 1.0;
            if hunter_ledger {
                world.state.hunters.founder_material_in -= 1.0;
            } else {
                world.state.external_material_in -= 1.0;
            }
            assert!(audit.observe(&world.state, 0.0, 0.0).is_err());
        }
    }
}
