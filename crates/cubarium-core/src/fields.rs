//! Material fields and their reactions.

use serde::{Deserialize, Serialize};

use cubarium_surface::{CELL_COUNT, CellId, FieldGraph, ScalarField, diffuse};

use crate::DT;
use crate::config::WorldConfig;

/// The checkpointed per-cell quantities: the material pools `N`, `P`, `D`, `F`, the
/// detritus energy `De`, and the water depth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fields {
    pub n: Vec<f64>,
    pub p: Vec<f64>,
    pub d: Vec<f64>,
    pub de: Vec<f64>,
    /// Fruit `F` (m) per cell (`design/fauna-v2.md` "Fruit"); a serialized state without
    /// it loads fruitless.
    #[serde(default = "dry")]
    pub f: Vec<f64>,
    /// Surface water depth `w` (d), not material (`design/water.md`). A serialized state
    /// without it loads dry.
    #[serde(default = "dry")]
    pub w: Vec<f64>,
}

/// A dry surface: the default for `Fields::w` when a serialized state lacks it.
fn dry() -> Vec<f64> {
    vec![0.0; CELL_COUNT]
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
    /// the litter `D = initial_dark · (1 − L₀)` with `De = e_d_max · D` (fully charged)
    /// (`design/stratified-world.md`: the dark soil starts littered, the lit canopy clean),
    /// `F = 0`, dry.
    pub fn new(cfg: &WorldConfig, light0: &[f64; CELL_COUNT], moisture0: &[f64; CELL_COUNT]) -> Fields {
        let p = (0..CELL_COUNT)
            .map(|i| cfg.producer.initial_fraction * cfg.producer.max * light0[i] * moisture0[i])
            .collect();
        let d: Vec<f64> = (0..CELL_COUNT)
            .map(|i| cfg.detritus.initial_dark * (1.0 - light0[i].clamp(0.0, 1.0)))
            .collect();
        let de = d.iter().map(|d| cfg.detritus.energy_cap * d).collect();
        Fields {
            n: vec![cfg.nutrient.initial; CELL_COUNT],
            p,
            d,
            de,
            f: dry(),
            w: dry(),
        }
    }

    pub fn total_material(&self) -> f64 {
        self.n.iter().sum::<f64>()
            + self.p.iter().sum::<f64>()
            + self.d.iter().sum::<f64>()
            + self.f.iter().sum::<f64>()
    }

    /// One tick of reactions, all from pre-tick values, per cell:
    /// 1. growth `Δ = min(g · L · W_eff · P · (1 − P/P_max) · N/(N + K_N) · drown · dt,
    ///    f_max · N · dt, N)` (Δ ≥ 0): `N −= Δ`, `P += Δ`, `light_in += e_p · Δ`. The Monod
    ///    factor `N/(N + K_N)` makes scarce nutrient limit uptake instead of only capping it;
    ///    `K_N = 0` restores the unsaturated law. Water (`design/water.md`) enters only here:
    ///    `W_eff = clamp(W + wet_gain · min(w, 1), W_min, 1)` and
    ///    `drown = max(0, 1 − (w − flood)/flood)` for `w > flood`, else 1. Both only scale
    ///    Δ, so conservation is unaffected.
    /// 2. mortality `Δ = m_p · P · dt`: `P −= Δ`, `D += Δ`, `De += e_p · Δ`, then if
    ///    `De > e_d_max · D` the excess goes to `heat_out` and `De` is clamped.
    /// 3. decomposition `Δ = k_d · D · dt`: `D −= Δ`, `N += Δ`, `De` reduced by the same
    ///    fraction with the removed energy in `heat_out`.
    /// 4. fruit (`design/fauna-v2.md` "Fruit"): ripening `ΔF = ripen · P · (P/P_max −
    ///    fruit_min)⁺ · L · dt` from the pre-tick `P`, never more than the cell still holds
    ///    after growth and mortality: `P −= ΔF`, `F += ΔF`, `light_in += (e_f − e_p) · ΔF`;
    ///    drop `ΔD = drop · F · dt`: `F −= ΔD`, `D += ΔD`, `De += e_f · ΔD`, under the same
    ///    `e_d_max` cap as mortality (excess is heat).
    ///
    /// Then, from the values those three steps left, detritus falls: for every cell with a
    /// downhill neighbour ([`FieldGraph::downhill`]) the fractions `ΔD = fall · dt · D` and
    /// `ΔDe = fall · dt · De` leave the cell and arrive in that neighbour, every transfer
    /// read from the same pre-fall snapshot. It is a pure transfer — no ledger entry, no
    /// heat — and moving the same fraction of `D` and `De` leaves `De ≤ e_d_max · D`
    /// intact in both cells. `fall · dt ≤ 1` (enforced by `WorldConfig::validate`) keeps
    /// every cell nonnegative, and `fall = 0` leaves the fields bit-identical.
    ///
    /// Finally `N` diffuses with `cubarium_surface::diffuse` at `diffusion · dt` using
    /// `scratch`. Never produces negatives; non-finite input is a bug (debug_assert).
    pub fn react(&mut self, cfg: &WorldConfig, light: &[f64; CELL_COUNT], moisture: &[f64; CELL_COUNT], graph: &FieldGraph, scratch: &mut (ScalarField, ScalarField)) -> FieldLedger {
        debug_assert_eq!(self.n.len(), CELL_COUNT);
        let mut ledger = FieldLedger::default();
        let pc = &cfg.producer;
        let dc = &cfg.detritus;
        let nc = &cfg.nutrient;
        let fc = &cfg.fruit;

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
            let (wet, drown) = water_factors(self.w[i], moisture[i], cfg);
            let logistic =
                pc.growth * light[i] * wet * p0 * (1.0 - p0 / pc.max) * monod * drown * DT;
            let grow = logistic.min(pc.uptake_max * n0 * DT).min(n0).max(0.0);

            // 2. Mortality: producers fall to detritus, carrying their energy with them.
            //    The `min` keeps `P` nonnegative even for a per-tick rate above one.
            let die = (pc.mortality * p0 * DT).clamp(0.0, p0 + grow);

            // 3b. Fruit: rich, lit producers ripen into fruit, and standing fruit drops back
            //     to detritus. Ripening reads the pre-tick `P` and never takes more than the
            //     cell holds after growth and mortality; fruit's extra energy density comes
            //     from light.
            let f0 = self.f[i];
            let over = if pc.max > 0.0 { (p0 / pc.max - fc.fruit_min).max(0.0) } else { 0.0 };
            let ripen = (fc.ripen * p0 * over * light[i] * DT).clamp(0.0, (p0 + grow - die).max(0.0));
            let dropped = (fc.drop * f0 * DT).clamp(0.0, f0);
            ledger.light_in += (fc.energy_density - pc.energy_density) * ripen;

            let d_after_death = d0 + die + dropped;
            let mut de_after_death = de0 + pc.energy_density * die + fc.energy_density * dropped;
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
            self.p[i] = p0 + grow - die - ripen;
            self.d[i] = d_after_death - decay;
            self.de[i] = de_after_death - de_removed;
            self.f[i] = f0 + ripen - dropped;
        }

        // Detritus slides downhill. `scratch` holds the pre-fall `D` and `De` so that every
        // transfer is a function of the values the reaction loop left, never of material
        // that arrived from an uphill cell in this same pass. Each cell subtracts its own
        // share exactly once, so the totals move but never change.
        let fall = dc.fall * DT;
        if fall > 0.0 {
            scratch.0.values.copy_from_slice(&self.d);
            scratch.1.values.copy_from_slice(&self.de);
            for cell in CellId::all() {
                let Some(down) = graph.downhill(cell) else { continue };
                let (here, there) = (cell.index(), down.index());
                let moved_d = fall * scratch.0.values[here];
                let moved_de = fall * scratch.1.values[here];
                self.d[here] -= moved_d;
                self.de[here] -= moved_de;
                self.d[there] += moved_d;
                self.de[there] += moved_de;
            }
        }

        // Nutrient diffusion over the cell graph; `diffuse` is conservative and never
        // pushes flux across the open rim.
        scratch.0.values.copy_from_slice(&self.n);
        diffuse(&mut scratch.0, &mut scratch.1, graph, cfg.nutrient.diffusion * DT);
        self.n.copy_from_slice(&scratch.0.values[..]);

        ledger
    }

    /// Debug/telemetry check: finite and nonnegative everywhere, `De ≤ e_d_max · D + 1e-9`,
    /// water finite and nonnegative.
    pub fn check(&self, energy_cap: f64) -> Result<(), String> {
        crate::water::check(&self.w)?;
        for (name, v) in
            [("N", &self.n), ("P", &self.p), ("D", &self.d), ("De", &self.de), ("F", &self.f)]
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

/// The two water factors on producer growth (`design/water.md`): the effective moisture
/// `W_eff = clamp(W + wet_gain · min(w, 1), W_min, 1)` and the drowning multiplier
/// `max(0, 1 − (w − flood)/flood)` above `flood`. A dry cell returns `(W, 1)` exactly.
pub fn water_factors(w: f64, moisture: f64, cfg: &WorldConfig) -> (f64, f64) {
    if w <= 0.0 {
        return (moisture, 1.0);
    }
    let wc = &cfg.water;
    let wet = (moisture + wc.wet_gain * w.min(1.0)).clamp(cfg.habitat.moisture_min, 1.0);
    let drown = if w > wc.flood { (1.0 - (w - wc.flood) / wc.flood).max(0.0) } else { 1.0 };
    (wet, drown)
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

    /// The fields' stored energy: `e_p·P + e_f·F + De` (`design/fauna-v2.md` adds fruit).
    fn stored_energy(f: &Fields, e_p: f64) -> f64 {
        let e_f = WorldConfig::default().fruit.energy_density;
        f.p.iter().map(|&p| e_p * p).sum::<f64>() + f.f.iter().map(|&x| e_f * x).sum::<f64>() + f.de.iter().sum::<f64>()
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
            let litter = h.cfg.detritus.initial_dark * (1.0 - h.light[i]);
            assert_eq!(f.d[i], litter);
            assert_eq!(f.de[i], h.cfg.detritus.energy_cap * litter);
            assert_eq!(f.f[i], 0.0);
        }
        f.check(h.cfg.detritus.energy_cap).unwrap();
    }

    /// `design/stratified-world.md` mechanism 2: the initial litter follows the dark, and a
    /// world without it starts clean.
    #[test]
    fn the_initial_litter_lies_where_it_is_dark() {
        let cfg = WorldConfig::default();
        assert_eq!(cfg.detritus.initial_dark, 1.2);
        let mut light = Box::new([0.0f64; CELL_COUNT]);
        let moisture = Box::new([1.0f64; CELL_COUNT]);
        light[0] = 1.0;
        light[1] = 0.0;
        light[2] = 0.25;
        let f = Fields::new(&cfg, &light, &moisture);
        assert_eq!(f.d[0], 0.0, "a fully lit cell gets no litter");
        assert_eq!(f.d[1], 1.2, "a dark cell gets initial_dark");
        assert_eq!(f.d[2], 1.2 * 0.75);
        assert_eq!(f.de[2], cfg.detritus.energy_cap * f.d[2], "the litter is fully charged");
        let litter: f64 = f.d.iter().sum();
        assert!(litter > 0.0);
        assert!((f.total_material() - (f.n.iter().sum::<f64>() + f.p.iter().sum::<f64>() + litter)).abs() < 1e-9, "the litter is initial material");
        let mut clean = cfg.clone();
        clean.detritus.initial_dark = 0.0;
        let g = Fields::new(&clean, &light, &moisture);
        assert!(g.d.iter().all(|&d| d == 0.0) && g.de.iter().all(|&de| de == 0.0));
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
            // Ripening would take a slice of `P` and book its own light; growth alone here.
            cfg.fruit.ripen = 0.0;
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

    /// A config in which nothing happens to `D` except the fall step.
    fn fall_only(fall: f64) -> WorldConfig {
        let mut cfg = WorldConfig::default();
        cfg.producer.growth = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.nutrient.diffusion = 0.0;
        // No fruit either: ripening books light and dropped fruit would add detritus.
        cfg.fruit.ripen = 0.0;
        cfg.fruit.drop = 0.0;
        cfg.detritus.fall = fall;
        cfg
    }

    /// A deterministic pseudo-random field, so the conservation check is not run on a
    /// suspiciously smooth input.
    fn scatter(f: &mut Fields, energy_cap: f64) {
        let mut x = 0x2545_F491_4F6C_DD1Du64;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 11) as f64 / (1u64 << 53) as f64
        };
        for i in 0..CELL_COUNT {
            f.d[i] = next() * 2.0;
            // Anywhere from empty to exactly at the cap.
            f.de[i] = next() * energy_cap * f.d[i];
        }
    }

    #[test]
    fn falling_detritus_moves_material_without_creating_it() {
        let cap = WorldConfig::default().detritus.energy_cap;
        let mut h = Harness::new(fall_only(0.02));
        let mut f = h.fields();
        scatter(&mut f, cap);
        let (d0, de0) = (f.d.iter().sum::<f64>(), f.de.iter().sum::<f64>());
        let start_material = f.total_material();
        for tick in 0..400 {
            let ledger = h.react(&mut f);
            // A transfer is neither a source nor a sink of energy.
            assert_eq!(ledger, FieldLedger::default(), "tick {tick}");
            let d: f64 = f.d.iter().sum();
            let de: f64 = f.de.iter().sum();
            assert!((d - d0).abs() / d0 < 1e-12, "tick {tick}: D drifted to {d} from {d0}");
            assert!((de - de0).abs() / de0 < 1e-12, "tick {tick}: De drifted to {de} from {de0}");
            for i in 0..CELL_COUNT {
                assert!(f.d[i] >= 0.0 && f.de[i] >= 0.0, "tick {tick} cell {i} went negative");
                assert!(
                    f.de[i] <= cap * f.d[i] + 1e-12,
                    "tick {tick} cell {i}: De {} above the cap {}",
                    f.de[i],
                    cap * f.d[i]
                );
            }
            f.check(cap).unwrap();
        }
        let drift = (f.total_material() - start_material).abs() / start_material;
        assert!(drift < 1e-12, "400 ticks of falling drifted {drift} relative");
    }

    #[test]
    fn a_zero_fall_rate_is_bit_identical_to_no_fall_step() {
        // With growth, mortality and decomposition off, `D` and `De` can only change if
        // the fall step runs, so "unchanged" is exactly "the step was inert".
        let cap = WorldConfig::default().detritus.energy_cap;
        let mut still = Harness::new(fall_only(0.0));
        let mut f = still.fields();
        scatter(&mut f, cap);
        let (d0, de0, n0) = (f.d.clone(), f.de.clone(), f.n.clone());
        for tick in 0..100 {
            assert_eq!(still.react(&mut f), FieldLedger::default(), "tick {tick}");
        }
        assert_eq!(f.d, d0, "fall = 0 must not move a single bit of detritus");
        assert_eq!(f.de, de0, "nor a single bit of its energy");

        // The same world at the default rate does move it, and neither rate touches `N`.
        let mut falling = Harness::new(fall_only(WorldConfig::default().detritus.fall));
        let mut g = falling.fields();
        scatter(&mut g, cap);
        for _ in 0..100 {
            falling.react(&mut g);
        }
        assert_ne!(g.d, d0, "the default fall rate must actually move detritus");
        assert_eq!(g.n, n0, "the fall step moves no nutrient");
    }

    #[test]
    fn a_column_of_detritus_ends_up_in_the_bottom_row_of_its_own_face() {
        use cubarium_surface::Face;

        let cap = WorldConfig::default().detritus.energy_cap;
        let mut h = Harness::new(fall_only(0.5));
        let mut f = h.fields();
        f.d.iter_mut().for_each(|d| *d = 0.0);
        f.de.iter_mut().for_each(|de| *de = 0.0);
        let source = CellId::new(Face::Right, 6, 0);
        let sink = CellId::new(Face::Right, 6, 15);
        f.d[source.index()] = 1.0;
        f.de[source.index()] = cap;

        // 0.5/s · 0.05 s = 2.5% per tick; 4,000 ticks is ample for a 15-step column.
        for _ in 0..4_000 {
            h.react(&mut f);
        }

        assert!(
            f.d[sink.index()] > 1.0 - 1e-9,
            "the bottom of the column holds {} of the 1.0 seeded",
            f.d[sink.index()]
        );
        assert!(f.de[sink.index()] > cap - 1e-9, "its energy came with it: {}", f.de[sink.index()]);
        for cell in CellId::all() {
            if cell == sink {
                continue;
            }
            assert!(
                f.d[cell.index()] < 1e-9,
                "{cell:?} still holds {} — material left the column",
                f.d[cell.index()]
            );
        }
        f.check(cap).unwrap();
    }

    #[test]
    fn detritus_on_the_canopy_and_on_the_rim_stays_put() {
        use cubarium_surface::Face;

        let mut h = Harness::new(fall_only(1.0));
        let mut f = h.fields();
        f.d.iter_mut().for_each(|d| *d = 0.0);
        f.de.iter_mut().for_each(|de| *de = 0.0);
        // A canopy cell against the seam, a canopy cell in the middle, and a rim cell.
        let stayers =
            [CellId::new(Face::Top, 0, 0), CellId::new(Face::Top, 8, 8), CellId::new(Face::Front, 3, 15)];
        for c in stayers {
            f.d[c.index()] = 1.0;
        }
        for _ in 0..200 {
            h.react(&mut f);
        }
        for c in stayers {
            assert_eq!(f.d[c.index()], 1.0, "{c:?} lost detritus");
        }
        assert_eq!(f.d.iter().filter(|&&d| d > 0.0).count(), stayers.len());
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

    /// `design/water.md`: a flooded cell is bare water, and a wet cell below full moisture
    /// grows faster than a dry one. Growth is isolated by switching the other reactions off,
    /// so the change in `N` is exactly `−Δgrow`.
    #[test]
    fn water_wets_growth_and_a_flood_drowns_it() {
        use cube_proto::Face;
        let mut cfg = WorldConfig::default();
        cfg.producer.mortality = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.nutrient.diffusion = 0.0;
        cfg.detritus.fall = 0.0;
        let flood = cfg.water.flood;
        let mut h = Harness::new(cfg);
        // Pick a cell whose moisture leaves headroom for wetting and a real standing crop.
        let cell = (0..CELL_COUNT)
            .find(|&i| h.moisture[i] < 0.6 && h.light[i] > 0.3)
            .expect("a dry-ish lit cell exists");
        let _ = Face::Top;
        let mut dry = h.fields();
        dry.p[cell] = 0.5;
        let mut wet = dry.clone();
        wet.w[cell] = 1.0;
        let mut drowned = dry.clone();
        drowned.w[cell] = 2.0 * flood;
        let mut half_drowned = dry.clone();
        half_drowned.w[cell] = 1.5 * flood;
        // The flow and evaporation never run here (no graph step in `react`), so `w` holds.
        let n0 = dry.n[cell];
        h.react(&mut dry);
        h.react(&mut wet);
        h.react(&mut drowned);
        h.react(&mut half_drowned);
        let grow = |f: &Fields| n0 - f.n[cell];
        assert!(grow(&dry) > 0.0);
        assert!(grow(&wet) > grow(&dry), "wet {} vs dry {}", grow(&wet), grow(&dry));
        let (w_eff, _) = water_factors(1.0, h.moisture[cell], &h.cfg);
        assert!((grow(&wet) / grow(&dry) - w_eff / h.moisture[cell]).abs() < 1e-9);
        assert_eq!(grow(&drowned), 0.0, "a flooded cell grows nothing");
        // Halfway between `flood` and `2·flood` the drowning factor is one half; the cell is
        // also fully wetted (`min(w, 1) = 1`), so the comparison is against the wet cell.
        assert!((grow(&half_drowned) / grow(&wet) - 0.5).abs() < 1e-9, "halfway to full drowning halves growth");
        // A dry cell's factors are the identity, exactly.
        assert_eq!(water_factors(0.0, 0.42, &h.cfg), (0.42, 1.0));
        // Wetting saturates at one unit of depth and never lifts moisture above one.
        let (deep, _) = water_factors(5.0, 0.9, &h.cfg);
        assert_eq!(deep, 1.0);
        assert_eq!(water_factors(3.0, 0.5, &h.cfg).0, water_factors(1.0, 0.5, &h.cfg).0);
    }

    #[test]
    fn fields_start_dry_and_check_covers_water() {
        let h = Harness::new(WorldConfig::default());
        let mut f = h.fields();
        assert!(f.w.iter().all(|&w| w == 0.0));
        assert_eq!(f.w.len(), CELL_COUNT);
        let cap = h.cfg.detritus.energy_cap;
        f.check(cap).unwrap();
        f.w[7] = -0.5;
        assert!(f.check(cap).unwrap_err().contains("w[7]"));
    }
}
