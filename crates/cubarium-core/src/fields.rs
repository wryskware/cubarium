//! Material fields and their reactions.

use serde::{Deserialize, Serialize};

use cubarium_surface::{CELL_COUNT, FieldGraph, ScalarField, diffuse};

use crate::DT;
use crate::config::WorldConfig;

/// The four checkpointed per-cell quantities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fields {
    pub n: Vec<f64>,
    pub p: Vec<f64>,
    pub d: Vec<f64>,
    pub de: Vec<f64>,
}

/// Energy ledger for one tick's field reactions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FieldLedger {
    /// Energy that entered through producer growth.
    pub light_in: f64,
    /// Energy dissipated by mortality clamping and decomposition.
    pub heat_out: f64,
}

impl Fields {
    /// Initial fields per the spec: `N = initial`, `P = initial_fraction · P_max · L₀ · W₀`,
    /// `D = De = 0`.
    pub fn new(cfg: &WorldConfig, light0: &[f64; CELL_COUNT], moisture0: &[f64; CELL_COUNT]) -> Fields {
        let p = (0..CELL_COUNT)
            .map(|i| cfg.producer.initial_fraction * cfg.producer.max * light0[i] * moisture0[i])
            .collect();
        Fields {
            n: vec![cfg.nutrient.initial; CELL_COUNT],
            p,
            d: vec![0.0; CELL_COUNT],
            de: vec![0.0; CELL_COUNT],
        }
    }

    pub fn total_material(&self) -> f64 {
        self.n.iter().sum::<f64>() + self.p.iter().sum::<f64>() + self.d.iter().sum::<f64>()
    }

    /// One tick of reactions, all from pre-tick values, per cell:
    /// 1. growth `Δ = min(g · L · W · P · (1 − P/P_max) · N/(N + K_N) · dt, f_max · N · dt, N)`
    ///    (Δ ≥ 0): `N −= Δ`, `P += Δ`, `light_in += e_p · Δ`. The Monod factor `N/(N + K_N)`
    ///    makes scarce nutrient limit uptake instead of only capping it; `K_N = 0` restores
    ///    the unsaturated law. It only scales Δ, so conservation is unaffected.
    /// 2. mortality `Δ = m_p · P · dt`: `P −= Δ`, `D += Δ`, `De += e_p · Δ`, then if
    ///    `De > e_d_max · D` the excess goes to `heat_out` and `De` is clamped.
    /// 3. decomposition `Δ = k_d · D · dt`: `D −= Δ`, `N += Δ`, `De` reduced by the same
    ///    fraction with the removed energy in `heat_out`.
    ///
    /// Then `N` diffuses with `cubarium_surface::diffuse` at `diffusion · dt` using
    /// `scratch`. Never produces negatives; non-finite input is a bug (debug_assert).
    pub fn react(&mut self, cfg: &WorldConfig, light: &[f64; CELL_COUNT], moisture: &[f64; CELL_COUNT], graph: &FieldGraph, scratch: &mut (ScalarField, ScalarField)) -> FieldLedger {
        debug_assert_eq!(self.n.len(), CELL_COUNT);
        let mut ledger = FieldLedger::default();
        let pc = &cfg.producer;
        let dc = &cfg.detritus;
        let nc = &cfg.nutrient;

        for i in 0..CELL_COUNT {
            // Every delta below is a function of the pre-tick values only.
            let n0 = self.n[i];
            let p0 = self.p[i];
            let d0 = self.d[i];
            let de0 = self.de[i];
            debug_assert!(
                n0.is_finite() && p0.is_finite() && d0.is_finite() && de0.is_finite(),
                "non-finite field at cell {i}: N={n0} P={p0} D={d0} De={de0}"
            );

            // 1. Growth: light-driven uptake of free nutrient, capped by the logistic term,
            //    by the per-second uptake fraction, and by what the cell actually holds.
            let monod = if n0 > 0.0 { n0 / (n0 + nc.half_saturation) } else { 0.0 };
            let logistic = pc.growth * light[i] * moisture[i] * p0 * (1.0 - p0 / pc.max) * monod * DT;
            let grow = logistic.min(pc.uptake_max * n0 * DT).min(n0).max(0.0);

            // 2. Mortality: producers fall to detritus, carrying their energy with them.
            //    The `min` keeps `P` nonnegative even for a per-tick rate above one.
            let die = (pc.mortality * p0 * DT).clamp(0.0, p0 + grow);
            let d_after_death = d0 + die;
            let mut de_after_death = de0 + pc.energy_density * die;
            let de_cap = dc.energy_cap * d_after_death;
            if de_after_death > de_cap {
                ledger.heat_out += de_after_death - de_cap;
                de_after_death = de_cap;
            }

            // 3. Decomposition: detritus returns to free nutrient, its stored energy
            //    leaving as heat in the same proportion as the material removed.
            let decay = (dc.decomposition * d0 * DT).clamp(0.0, d_after_death);
            let removed_fraction =
                if d_after_death > 0.0 { decay / d_after_death } else { 0.0 };
            let de_removed = de_after_death * removed_fraction;
            ledger.heat_out += de_removed;

            ledger.light_in += pc.energy_density * grow;
            self.n[i] = n0 - grow + decay;
            self.p[i] = p0 + grow - die;
            self.d[i] = d_after_death - decay;
            self.de[i] = de_after_death - de_removed;
        }

        // Nutrient diffusion over the cell graph; `diffuse` is conservative and never
        // pushes flux across the open rim.
        scratch.0.values.copy_from_slice(&self.n);
        diffuse(&mut scratch.0, &mut scratch.1, graph, cfg.nutrient.diffusion * DT);
        self.n.copy_from_slice(&scratch.0.values[..]);

        ledger
    }

    /// Debug/telemetry check: finite and nonnegative everywhere, `De ≤ e_d_max · D + 1e-9`.
    pub fn check(&self, energy_cap: f64) -> Result<(), String> {
        for (name, v) in
            [("N", &self.n), ("P", &self.p), ("D", &self.d), ("De", &self.de)]
        {
            if v.len() != CELL_COUNT {
                return Err(format!("{name} has {} cells, expected {CELL_COUNT}", v.len()));
            }
            for (i, &x) in v.iter().enumerate() {
                if !x.is_finite() {
                    return Err(format!("{name}[{i}] is not finite: {x}"));
                }
                if x < 0.0 {
                    return Err(format!("{name}[{i}] is negative: {x}"));
                }
            }
        }
        for i in 0..CELL_COUNT {
            let cap = energy_cap * self.d[i] + 1e-9;
            if self.de[i] > cap {
                return Err(format!(
                    "De[{i}] = {} exceeds e_d_max · D = {}",
                    self.de[i],
                    energy_cap * self.d[i]
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::habitat::Habitat;

    struct Harness {
        cfg: WorldConfig,
        light: Box<[f64; CELL_COUNT]>,
        moisture: Box<[f64; CELL_COUNT]>,
        graph: FieldGraph,
        scratch: (ScalarField, ScalarField),
    }

    impl Harness {
        fn new(cfg: WorldConfig) -> Harness {
            let habitat = Habitat::new(&cfg.habitat, cfg.seed);
            Harness {
                cfg,
                light: habitat.light_base.clone(),
                moisture: habitat.moisture_base.clone(),
                graph: FieldGraph::new(),
                scratch: (ScalarField::zeros(), ScalarField::zeros()),
            }
        }

        fn fields(&self) -> Fields {
            Fields::new(&self.cfg, &self.light, &self.moisture)
        }

        fn react(&mut self, f: &mut Fields) -> FieldLedger {
            f.react(&self.cfg, &self.light, &self.moisture, &self.graph, &mut self.scratch)
        }
    }

    fn stored_energy(f: &Fields, e_p: f64) -> f64 {
        f.p.iter().map(|&p| e_p * p).sum::<f64>() + f.de.iter().sum::<f64>()
    }

    #[test]
    fn initial_fields_follow_the_spec() {
        let h = Harness::new(WorldConfig::default());
        let f = h.fields();
        assert_eq!(f.n.len(), CELL_COUNT);
        for i in 0..CELL_COUNT {
            assert_eq!(f.n[i], h.cfg.nutrient.initial);
            let want = h.cfg.producer.initial_fraction * h.cfg.producer.max * h.light[i] * h.moisture[i];
            assert_eq!(f.p[i], want);
            assert_eq!(f.d[i], 0.0);
            assert_eq!(f.de[i], 0.0);
        }
        f.check(h.cfg.detritus.energy_cap).unwrap();
    }

    #[test]
    fn the_monod_term_limits_growth_by_nutrient() {
        // Growth scales as N/(N + K_N) when nothing else binds.
        fn harness(half_saturation: f64) -> Harness {
            let mut cfg = WorldConfig::default();
            cfg.producer.mortality = 0.0;
            cfg.detritus.decomposition = 0.0;
            cfg.nutrient.diffusion = 0.0;
            // Large enough that the uptake cap never binds before the Monod factor.
            cfg.producer.uptake_max = 1e9;
            cfg.nutrient.half_saturation = half_saturation;
            Harness::new(cfg)
        }
        // `light_in` is `e_p` times the tick's total growth.
        fn growth(h: &mut Harness, nutrient: f64) -> f64 {
            let mut f = h.fields();
            f.n.iter_mut().for_each(|n| *n = nutrient);
            h.react(&mut f).light_in
        }

        let k = WorldConfig::default().nutrient.half_saturation;
        let mut saturating = harness(k);
        let at_k = growth(&mut saturating, k);
        let at_3k = growth(&mut saturating, 3.0 * k);
        assert!(at_k > 0.0);
        // N = K_N gives half the saturated rate, N = 3·K_N three quarters: a ratio of 1.5.
        assert!((at_3k / at_k - 1.5).abs() < 1e-9, "ratio {}", at_3k / at_k);

        // K_N = 0 restores the unsaturated law, which is twice the rate at N = K_N.
        let mut unsaturated = harness(0.0);
        let plain = growth(&mut unsaturated, k);
        assert!((plain / at_k - 2.0).abs() < 1e-9, "ratio {}", plain / at_k);

        // An empty cell grows nothing however much light it gets.
        assert_eq!(growth(&mut saturating, 0.0), 0.0);
    }

    #[test]
    fn material_is_conserved_every_tick() {
        let mut h = Harness::new(WorldConfig::default());
        let mut f = h.fields();
        let start = f.total_material();
        let mut worst = 0.0f64;
        for tick in 0..600 {
            let before = f.total_material();
            h.react(&mut f);
            let after = f.total_material();
            let rel = (after - before).abs() / before;
            worst = worst.max(rel);
            assert!(rel < 1e-12, "tick {tick}: relative material drift {rel}");
            f.check(h.cfg.detritus.energy_cap).unwrap();
        }
        let drift = (f.total_material() - start).abs() / start;
        assert!(drift < 1e-12, "600 ticks drifted by {drift} relative");
        println!("worst per-tick relative material drift over 600 ticks: {worst:e}");
        println!("cumulative relative drift: {drift:e}");
    }

    #[test]
    fn the_energy_ledger_matches_the_change_in_stored_energy() {
        let mut h = Harness::new(WorldConfig::default());
        let e_p = h.cfg.producer.energy_density;
        let mut f = h.fields();
        let mut worst = 0.0f64;
        for tick in 0..600 {
            let before = stored_energy(&f, e_p);
            let ledger = h.react(&mut f);
            let after = stored_energy(&f, e_p);
            let residual = (ledger.light_in - ledger.heat_out) - (after - before);
            worst = worst.max(residual.abs());
            assert!(residual.abs() < 1e-9, "tick {tick}: energy residual {residual}");
            assert!(ledger.light_in >= 0.0 && ledger.heat_out >= 0.0);
        }
        println!("worst per-tick energy residual over 600 ticks: {worst:e}");
    }

    #[test]
    fn detritus_energy_never_exceeds_its_cap() {
        let mut cfg = WorldConfig::default();
        // Producers with a lot of energy per unit and a low detritus energy cap: the
        // mortality clamp must dump the excess as heat every tick.
        cfg.producer.energy_density = 20.0;
        cfg.producer.mortality = 0.2;
        cfg.detritus.energy_cap = 0.5;
        let mut h = Harness::new(cfg);
        let mut f = h.fields();
        let mut clamped = 0.0;
        for _ in 0..200 {
            clamped += h.react(&mut f).heat_out;
            f.check(h.cfg.detritus.energy_cap).unwrap();
        }
        assert!(clamped > 0.0, "the clamp should have produced heat");
    }

    #[test]
    fn extreme_configs_never_produce_negatives() {
        for (uptake, growth, mortality, decomposition) in [
            (1e9, 1e9, 1e9, 1e9),
            (1e9, 0.005, 0.0005, 0.002),
            (0.5, 1e6, 1e6, 0.0),
            (0.0, 0.0, 0.0, 1e9),
        ] {
            for p_level in [0.0, 1.0] {
                let mut cfg = WorldConfig::default();
                cfg.producer.uptake_max = uptake;
                cfg.producer.growth = growth;
                cfg.producer.mortality = mortality;
                cfg.detritus.decomposition = decomposition;
                let mut h = Harness::new(cfg);
                let mut f = h.fields();
                // P at 0 and at P_max exactly, the two ends of the logistic term.
                let p0 = p_level * h.cfg.producer.max;
                f.p.iter_mut().for_each(|p| *p = p0);
                f.d.iter_mut().for_each(|d| *d = 0.25);
                f.de.iter_mut().for_each(|de| *de = 0.25);
                let start = f.total_material();
                for tick in 0..50 {
                    h.react(&mut f);
                    f.check(h.cfg.detritus.energy_cap).unwrap_or_else(|e| {
                        panic!("tick {tick} with uptake {uptake} growth {growth} P {p0}: {e}")
                    });
                }
                let drift = (f.total_material() - start).abs() / start.max(1e-12);
                assert!(drift < 1e-12, "extreme config drifted {drift}");
            }
        }
    }

    #[test]
    fn a_constant_nutrient_field_stays_constant_without_reactions() {
        let mut cfg = WorldConfig::default();
        cfg.producer.growth = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.detritus.decomposition = 0.0;
        let mut h = Harness::new(cfg);
        let mut f = h.fields();
        f.p.iter_mut().for_each(|p| *p = 0.0);
        let before = f.n.clone();
        for _ in 0..100 {
            let ledger = h.react(&mut f);
            assert_eq!(ledger, FieldLedger::default());
        }
        // Diffusion of a constant field is bit-identical.
        assert_eq!(f.n, before);
    }

    #[test]
    fn diffusion_moves_nutrient_without_creating_it() {
        let mut cfg = WorldConfig::default();
        cfg.producer.growth = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.detritus.decomposition = 0.0;
        let mut h = Harness::new(cfg);
        let mut f = h.fields();
        f.n.iter_mut().for_each(|n| *n = 0.0);
        f.p.iter_mut().for_each(|p| *p = 0.0);
        f.n[0] = 10.0;
        let total = f.total_material();
        for _ in 0..50 {
            h.react(&mut f);
        }
        assert!((f.total_material() - total).abs() < 1e-12);
        assert!(f.n[0] < 10.0, "the spike should have spread");
        assert!(f.n.iter().filter(|&&x| x > 1e-9).count() > 1);
        f.check(h.cfg.detritus.energy_cap).unwrap();
    }

    #[test]
    fn check_rejects_bad_fields() {
        let h = Harness::new(WorldConfig::default());
        let cap = h.cfg.detritus.energy_cap;

        let mut f = h.fields();
        f.n[3] = -1e-9;
        assert!(f.check(cap).is_err());

        let mut f = h.fields();
        f.p[7] = f64::NAN;
        assert!(f.check(cap).is_err());

        let mut f = h.fields();
        f.de[9] = 1.0;
        f.d[9] = 0.0;
        assert!(f.check(cap).is_err());

        let mut f = h.fields();
        f.d.pop();
        assert!(f.check(cap).is_err());
    }
}
