//! Material fields and their reactions.
//!
//! Ecology v1 (`design/ecology-v1-contract.md`) splits the field-reaction phase into the
//! eight subphases of its §4.0 table. [`Fields`] keeps the pools every earlier schema had
//! (`N`, `P`, `D`, `De`, `F`, `w`); the pools this milestone adds — living wood `W`, the
//! plant reserve `Q`, dead wood `Wd` and animal remains `C`/`Ce` — live in
//! [`EcologyV1State`], a **trailing** extension of `WorldState` rather than a change to the
//! wire shape of `Fields`. The two are always stepped together.

use cubarium_surface::{Scale, Topology};
use serde::{Deserialize, Serialize};

use cubarium_surface::{CUBE_CELL_COUNT, CellId, FieldGraph, ScalarField, diffuse};

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
    vec![0.0; CUBE_CELL_COUNT]
}

/// `Q_0` as a fraction of `Q_max` in a cell that starts alive
/// (`design/ecology-v1-contract.md` §11). Not a knob: §14 fixes [`crate::config::PlantConfig`]
/// to the fields it lists and this is not one of them, so the number appears once, here.
pub const INITIAL_RESERVE_FRACTION: f64 = 0.5;

/// Ecology v1's new per-cell pools and its two world counters
/// (`design/ecology-v1-contract.md` §3.1, §3.3, §14).
///
/// Appended to [`crate::world::WorldState`] after every earlier extension, which is what
/// makes schema 15 a byte-exact prefix of schema 16. Schema 16 refuses every older snapshot
/// outright (§15.1: worlds always restart fresh, never migrate), so nothing here is ever
/// synthesised for a world that did not have it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EcologyV1State {
    /// `W`: living wood (m). Persistent structure; no v1 animal can take it. Its value
    /// decides the cell's class (§3.1) and carries the foliage cap and the maintenance bill.
    pub wood: Vec<f64>,
    /// `Q`: plant reserve (m). Pays the maintenance shortfall, the reflush after
    /// defoliation, and propagules.
    pub plant_reserve: Vec<f64>,
    /// `Wd`: dead wood (m). Decomposes slowly, never falls, is never eaten, and keeps its
    /// identity: it is not litter.
    pub dead_wood: Vec<f64>,
    /// `C`, `Ce`: animal remains and their energy. Bodies, failed gestations and dead
    /// hunters land here; detrital digesters eat them.
    pub carrion: Vec<f64>,
    pub carrion_energy: Vec<f64>,
    /// Cumulative stands that crossed from alive to dead (§4.7), and cells that crossed
    /// `W_min` from a propagule (§4.8).
    pub plant_deaths_total: u64,
    pub recolonisations_total: u64,
}

impl Default for EcologyV1State {
    /// Every pool empty and correctly sized: a bare, wood-free surface.
    fn default() -> Self {
        EcologyV1State {
            wood: dry(),
            plant_reserve: dry(),
            dead_wood: dry(),
            carrion: dry(),
            carrion_energy: dry(),
            plant_deaths_total: 0,
            recolonisations_total: 0,
        }
    }
}

/// A cell's class this tick, decided once from the **pre-tick** wood `W⁻`
/// (`design/ecology-v1-contract.md` §3.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CellClass {
    /// `W⁻ = 0`: no plant. Litter, fruit and remains may still lie here.
    #[default]
    Bare,
    /// `0 < W⁻ < W_min`: propagule material, frozen. No income, maintenance, growth,
    /// senescence, dieback or death.
    Establishing,
    /// `W⁻ ≥ W_min`: runs §4.1–4.7.
    Alive,
}

impl CellClass {
    pub fn of(wood: f64, alive_min: f64) -> CellClass {
        if !(wood > 0.0) {
            CellClass::Bare
        } else if wood < alive_min {
            CellClass::Establishing
        } else {
            CellClass::Alive
        }
    }

    /// Whether a propagule may land here (§4.8: recipients are establishing or bare).
    pub fn recipient(self) -> bool {
        self != CellClass::Alive
    }
}

/// `W_0` for one cell (`design/ecology-v1-contract.md` §11): `initial_wood · W_max · L₀ · μ₀`,
/// **set to zero where that is below `W_min`** so a fresh world never starts a cell in the
/// frozen establishing class. One definition, read by both constructors.
pub fn initial_wood(cfg: &WorldConfig, light0: f64, moisture0: f64) -> f64 {
    let pc = &cfg.plant;
    let w = pc.initial_wood * pc.wood_max * light0.clamp(0.0, 1.0) * moisture0.clamp(0.0, 1.0);
    if w > 0.0 && w >= pc.alive_min { w } else { 0.0 }
}

impl EcologyV1State {
    /// Initial pools per §11: `W_0` from [`initial_wood`], `Q_0 = 0.5 · q_cap · W_0` in the
    /// cells that start alive, no dead wood and no remains.
    pub fn new(
        cfg: &WorldConfig,
        light0: &[f64; CUBE_CELL_COUNT],
        moisture0: &[f64; CUBE_CELL_COUNT],
    ) -> EcologyV1State {
        let wood: Vec<f64> = (0..CUBE_CELL_COUNT)
            .map(|i| initial_wood(cfg, light0[i], moisture0[i]))
            .collect();
        let plant_reserve = wood
            .iter()
            .map(|w| INITIAL_RESERVE_FRACTION * cfg.plant.reserve_cap * w)
            .collect();
        EcologyV1State {
            wood,
            plant_reserve,
            dead_wood: dry(),
            carrion: dry(),
            carrion_energy: dry(),
            plant_deaths_total: 0,
            recolonisations_total: 0,
        }
    }

    /// `W + Q + Wd + C`, summed over cells: the material this extension holds, which the
    /// mass identity of §10 adds to [`Fields::total_material`].
    pub fn total_material(&self) -> f64 {
        self.wood.iter().sum::<f64>()
            + self.plant_reserve.iter().sum::<f64>()
            + self.dead_wood.iter().sum::<f64>()
            + self.carrion.iter().sum::<f64>()
    }

    /// `e_v·(W + Q + Wd) + Ce`: the stored energy of §10 that this extension holds. Foliage
    /// `e_v·P` stays with [`Fields`].
    pub fn stored_energy(&self, e_v: f64) -> f64 {
        e_v * (self.wood.iter().sum::<f64>()
            + self.plant_reserve.iter().sum::<f64>()
            + self.dead_wood.iter().sum::<f64>())
            + self.carrion_energy.iter().sum::<f64>()
    }

    /// Finite, nonnegative, correctly sized, and `Ce ≤ e_c_max · C`.
    pub fn check(&self, carrion_energy_cap: f64) -> Result<(), String> {
        for (name, v) in [
            ("wood", &self.wood),
            ("plant_reserve", &self.plant_reserve),
            ("dead_wood", &self.dead_wood),
            ("carrion", &self.carrion),
            ("carrion_energy", &self.carrion_energy),
        ] {
            if v.len() != CUBE_CELL_COUNT {
                return Err(format!("{name} has {} cells, expected {CUBE_CELL_COUNT}", v.len()));
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
        for i in 0..CUBE_CELL_COUNT {
            let cap = carrion_energy_cap * self.carrion[i] + 1e-9;
            if self.carrion_energy[i] > cap {
                return Err(format!(
                    "carrion_energy[{i}] = {} exceeds e_c_max · C = {}",
                    self.carrion_energy[i],
                    carrion_energy_cap * self.carrion[i]
                ));
            }
        }
        Ok(())
    }
}

/// Reusable per-tick working storage for the cross-cell subphases 3f and 3h. Held by the
/// world so a 36,000-tick run allocates none of it per tick; never persisted, never hashed.
#[derive(Clone, Debug)]
pub struct EcoScratch {
    /// The pre-tick stocks 3e and 3f both draw on (§5's joint withdrawal budget).
    pre_d: Vec<f64>,
    pre_c: Vec<f64>,
    /// The immutable `X⁵`/`Xe⁵` snapshot 3f falls from.
    d5: Vec<f64>,
    de5: Vec<f64>,
    c5: Vec<f64>,
    ce5: Vec<f64>,
    /// Each cell's class this tick, from `W⁻`.
    class: Vec<CellClass>,
    /// 3h's immutable donor-budget snapshot and the per-recipient incoming sum.
    budget: Vec<f64>,
    incoming: Vec<f64>,
}

impl Default for EcoScratch {
    fn default() -> Self {
        EcoScratch {
            pre_d: dry(),
            pre_c: dry(),
            d5: dry(),
            de5: dry(),
            c5: dry(),
            ce5: dry(),
            class: vec![CellClass::Bare; CUBE_CELL_COUNT],
            budget: dry(),
            incoming: dry(),
        }
    }
}

/// Energy ledger for one tick's field reactions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FieldLedger {
    /// Energy that entered through producer growth.
    pub light_in: f64,
    /// Energy dissipated by mortality clamping and decomposition.
    pub heat_out: f64,
    /// **Gross** foliage material grown this tick, summed over cells, before any senescence,
    /// ripening or grazing removes it. Transient: returned to the step, never persisted and
    /// never hashed. It exists because production is otherwise only visible as a net change,
    /// and a net change cannot separate a patch that is not growing from one that is growing
    /// and being eaten at the same rate (`crate::world::IntakeDiagnostics`).
    pub producer_growth: f64,
    /// `Σ A` (§4.2): the material plant income actually took out of `N` this tick, before
    /// maintenance and construction respiration send most of it back.
    pub plant_income: f64,
    /// `Σ unpaid` (§4.3): maintenance that neither income nor reserve could cover, which is
    /// exactly what 3d turns into dieback.
    pub plant_maintenance_unpaid: f64,
    /// `Σ s` (§4.8): reserve material donors actually sent to establishing or bare
    /// neighbours this tick.
    pub propagule_sent: f64,
}

impl Fields {
    /// Initial fields per `design/ecology-v1-contract.md` §11: `N = initial`,
    /// `P = initial_fraction · P_cap(W_0)` in cells that start alive and zero elsewhere, the
    /// litter `D = initial_dark · (1 − L₀)` with `De = e_d_max · D` (fully charged)
    /// (`design/stratified-world.md`: the dark soil starts littered, the lit canopy clean),
    /// `F = 0`, dry.
    ///
    /// The wood those cells start with is [`EcologyV1State::new`]'s; both read
    /// [`initial_wood`], so a cell has foliage exactly when it has a stand to carry it.
    pub fn new(cfg: &WorldConfig, light0: &[f64; CUBE_CELL_COUNT], moisture0: &[f64; CUBE_CELL_COUNT]) -> Fields {
        let p = (0..CUBE_CELL_COUNT)
            .map(|i| {
                let w0 = initial_wood(cfg, light0[i], moisture0[i]);
                if w0 <= 0.0 {
                    return 0.0;
                }
                let p_cap = cfg.producer.max.min(cfg.plant.alpha * w0);
                cfg.producer.initial_fraction * p_cap
            })
            .collect();
        let d: Vec<f64> = (0..CUBE_CELL_COUNT)
            .map(|i| cfg.detritus.initial_dark * (1.0 - light0[i].clamp(0.0, 1.0)))
            .collect();
        let de = d.iter().map(|d| cfg.detritus.energy_cap * d).collect();
        Fields {
            n: vec![cfg.nutrient.initial; CUBE_CELL_COUNT],
            p,
            d,
            de,
            f: dry(),
            w: dry(),
        }
    }

    /// `N + P + D + F`. The ecology v1 pools are [`EcologyV1State::total_material`]; the mass
    /// identity of §10 is the sum of the two.
    pub fn total_material(&self) -> f64 {
        self.n.iter().sum::<f64>()
            + self.p.iter().sum::<f64>()
            + self.d.iter().sum::<f64>()
            + self.f.iter().sum::<f64>()
    }

    /// One tick of field reactions: the eight subphases of `design/ecology-v1-contract.md`
    /// §4.0, in order, each reading exactly the state its row names.
    ///
    /// - **3a** (alive cells) income, maintenance and growth (§4.1–4.4). Potential income is
    ///   the old growth law without its logistic term, capped by what the structure can
    ///   actually hold: `A = min(A_pot, M + (1 + c_g)(D_P + D_W) + D_Q)`. Maintenance is paid
    ///   from income first and then from reserve; what neither covers is `unpaid` and is
    ///   carried to 3d. Growth goes to foliage before wood, each paying construction
    ///   respiration `c_g` into the growing cell's own `N`.
    /// - **3b** (alive) senescence `m_p · P¹ · dt` into litter, its energy under the `e_d_max`
    ///   cap with the excess as heat.
    /// - **3c** (all) ripening and fruit drop, today's rule unchanged.
    /// - **3d** (alive) dieback `κ · unpaid` into dead wood, then stand death: a cell that
    ///   falls below `W_min` sends its remaining wood to `Wd` and its foliage and reserve to
    ///   litter. Dead wood keeps its identity — it is not litter and no v1 animal eats it.
    /// - **3e** (all) decomposition of litter, remains and dead wood: material to `N`, the
    ///   stock's energy to heat in the same proportion, nothing ever recharged.
    /// - **3f** (all) downhill fall of litter and remains. 3e and 3f share **one** withdrawal
    ///   budget per stock: both draw on the pre-tick material and fall acts only on the
    ///   portion decomposition left, so `dec + fall ≤ X⁻` at every admitted endpoint.
    /// - **3g** (all) nutrient diffusion on a snapshot of `N⁵`, unchanged.
    /// - **3h** propagules: a donor doing well spends reserve on its establishing or bare
    ///   neighbours, from one immutable snapshot, committed together.
    ///
    /// Energy is withdrawn from a detrital stock at its **current** density, so a withdrawal
    /// never changes the density and the caps `De ≤ e_d_max·D`, `Ce ≤ e_c_max·C` survive
    /// without a re-clamp. Every ratio guards its zero denominator: an empty stock withdraws
    /// nothing and books no heat.
    ///
    /// Never produces negatives; non-finite input is a bug (debug_assert).
    #[allow(clippy::too_many_arguments)]
    pub fn react(
        &mut self,
        eco: &mut EcologyV1State,
        cfg: &WorldConfig,
        light: &[f64; CUBE_CELL_COUNT],
        moisture: &[f64; CUBE_CELL_COUNT],
        graph: &FieldGraph,
        scratch: &mut (ScalarField, ScalarField),
        work: &mut EcoScratch,
    ) -> FieldLedger {
        debug_assert_eq!(self.n.len(), CUBE_CELL_COUNT);
        let mut ledger = FieldLedger::default();
        let pc = &cfg.producer;
        let pl = &cfg.plant;
        let dc = &cfg.detritus;
        let nc = &cfg.nutrient;
        let fc = &cfg.fruit;
        let e_v = pl.energy_density;
        let e_f = fc.energy_density;
        let e_d_max = dc.energy_cap;
        let build = 1.0 + pl.build;

        for i in 0..CUBE_CELL_COUNT {
            // Every delta below is a function of this cell's pre-tick values and of what an
            // earlier subphase wrote *in this same cell*, exactly as the §4.0 table allows.
            let n0 = self.n[i];
            let p0 = self.p[i];
            let d0 = self.d[i];
            let de0 = self.de[i];
            let f0 = self.f[i];
            let w0 = eco.wood[i];
            let q0 = eco.plant_reserve[i];
            let wd0 = eco.dead_wood[i];
            let c0 = eco.carrion[i];
            let ce0 = eco.carrion_energy[i];
            debug_assert!(
                n0.is_finite() && p0.is_finite() && d0.is_finite() && de0.is_finite()
                    && w0.is_finite() && q0.is_finite() && wd0.is_finite() && c0.is_finite(),
                "non-finite field at cell {i}"
            );
            work.pre_d[i] = d0;
            work.pre_c[i] = c0;
            // This tick's class, from the **pre-tick** wood (§3.1): it decides whether 3a–3d
            // run at all. `work.class` is 3h's, and is written from the **post-3d** wood at
            // the end of this cell's block — see there.
            let class = CellClass::of(w0, pl.alive_min);

            let mut n = n0;
            let mut p = p0;
            let mut w = w0;
            let mut q = q0;
            let mut d = d0;
            let mut de = de0;
            let mut wd = wd0;
            let mut c = c0;
            let mut ce = ce0;
            let mut unpaid = 0.0;

            if class == CellClass::Alive {
                // --- 3a: income (§4.1), demands (§4.2), maintenance (§4.3), growth (§4.4).
                let monod = if n0 > 0.0 { n0 / (n0 + nc.half_saturation) } else { 0.0 };
                let (wet, drown) = water_factors(self.w[i], moisture[i], cfg);
                let lit = algae_light(self.w[i], light[i], cfg);
                let a_pot = (pc.growth * lit * wet * p0 * monod * drown * DT)
                    .min(pc.uptake_max * n0 * DT)
                    .min(n0)
                    .max(0.0);
                let p_cap = pc.max.min(pl.alpha * w0);
                let maintenance = pl.maintenance * w0 * DT;
                let demand_p = (p_cap - p0).max(0.0).min(pl.foliage_rate * w0 * DT);
                let demand_w = (pl.wood_max - w0).max(0.0).min(pl.wood_rate * w0 * DT);
                let q_max = pl.reserve_cap * w0;
                let demand_q = (q_max - q0).max(0.0);
                let a = a_pot.min(maintenance + build * (demand_p + demand_w) + demand_q);
                n -= a;
                ledger.light_in += e_v * a;
                ledger.plant_income += a;

                // 4.3 — maintenance from income first, then reserve; the rest is `unpaid`.
                let paid_a = a.min(maintenance);
                let mut rem = a - paid_a;
                let short = maintenance - paid_a;
                let paid_q = q0.min(short);
                q = q0 - paid_q;
                unpaid = short - paid_q;
                n += paid_a + paid_q;
                ledger.heat_out += e_v * (paid_a + paid_q);
                ledger.plant_maintenance_unpaid += unpaid;

                // 4.4 — reserve share off the top, then foliage, then wood, then the rest
                //     back to the reserve; each growth paying `c_g` into this cell's own `N`.
                //
                //     **Repair cycle 1.** The original order refilled the reserve only from
                //     what foliage and wood left — under foliage-first allocation, nothing —
                //     and spent reserve on routine foliage top-up whenever `P < P_cap`, which
                //     is always. The first implementation run measured the consequence: a
                //     stand's reserve never persisted, so no simulator-grown stand was ever a
                //     §4.8 donor. Two rules fix it, and both are in the loop below: a fixed
                //     `q_share` of the surplus goes to the reserve first, and reserve is spent
                //     on foliage **only** below `p_reflush · P_cap`, capped at that ceiling.
                //
                //     Every `.max(0.0)` below is a rounding guard, not a rule: `build ·
                //     (x / build)` can exceed `x` by an ulp, and a stock that went one ulp
                //     negative would fail the world's own nonnegativity invariant. The
                //     material it forgives is ~1e-22 per cell per tick, twelve orders below
                //     the 1e-9 the mass identity is audited to.
                let share_q = (pl.reserve_share * rem).min(demand_q).max(0.0);
                rem = (rem - share_q).max(0.0);
                q += share_q;

                let grow_p_income = (rem / build).min(demand_p).max(0.0);
                rem = (rem - build * grow_p_income).max(0.0);
                // Reflush is an **emergency** draw, not routine top-up: it opens only when the
                // pre-tick foliage is below `p_reflush · P_cap`, and it stops at that ceiling
                // rather than at `P_cap`.
                let reflush_ceiling = pl.reflush_below * p_cap;
                let grow_p_reserve = if p0 < reflush_ceiling {
                    (q / build)
                        .min(demand_p - grow_p_income)
                        .min((reflush_ceiling - p0 - grow_p_income).max(0.0))
                        .max(0.0)
                } else {
                    0.0
                };
                q = (q - build * grow_p_reserve).max(0.0);
                let grown_p = grow_p_income + grow_p_reserve;
                p = p0 + grown_p;
                n += pl.build * grown_p;
                ledger.heat_out += e_v * pl.build * grown_p;
                ledger.producer_growth += grown_p;

                let grow_w = (rem / build).min(demand_w).max(0.0);
                rem = (rem - build * grow_w).max(0.0);
                w = w0 + grow_w;
                n += pl.build * grow_w;
                ledger.heat_out += e_v * pl.build * grow_w;

                let refill_q = rem.min((demand_q - share_q).max(0.0)).max(0.0);
                q += refill_q;
                rem = (rem - refill_q).max(0.0);
                // Rounding residue only; it must be ~0 whenever `A` bound on the demand sum.
                n += rem;
                ledger.heat_out += e_v * rem;

                // --- 3b: senescence (§4.5).
                let shed = (pc.mortality * p * DT).clamp(0.0, p);
                p -= shed;
                d = d0 + shed;
                let want = de0 + e_v * shed;
                let cap = e_d_max * d;
                if want > cap {
                    ledger.heat_out += want - cap;
                    de = cap;
                } else {
                    de = want;
                }
            }

            // --- 3c: ripening and drop, on every cell, today's rule. Ripening reads the
            //     pre-tick `P⁻` for its trigger and is capped by what 3b left.
            let over = if pc.max > 0.0 { (p0 / pc.max - fc.fruit_min).max(0.0) } else { 0.0 };
            let ripened = (fc.ripen * p0 * over * light[i] * DT).clamp(0.0, p.max(0.0));
            let dropped = (fc.drop * f0 * DT).clamp(0.0, f0);
            ledger.light_in += (e_f - e_v) * ripened;
            p -= ripened;
            let f_next = f0 + ripened - dropped;
            d += dropped;
            let want = de + e_f * dropped;
            let cap = e_d_max * d;
            if want > cap {
                ledger.heat_out += want - cap;
                de = cap;
            } else {
                de = want;
            }

            if class == CellClass::Alive {
                // --- 3d: dieback (§4.6) then death (§4.7).
                let died_back = w.min(pl.dieback * unpaid).max(0.0);
                w -= died_back;
                wd = wd0 + died_back;
                if w < pl.alive_min {
                    wd += w;
                    let fallen = p + q;
                    d += fallen;
                    let room = (e_d_max * d - de).max(0.0);
                    let want = e_v * fallen;
                    let kept = want.min(room);
                    de += kept;
                    ledger.heat_out += want - kept;
                    w = 0.0;
                    p = 0.0;
                    q = 0.0;
                    eco.plant_deaths_total += 1;
                }
            }

            // --- 3e: decomposition of the three detrital stocks (§5). The *amounts* come
            //     from the pre-tick stocks, so material deposited above is eligible from the
            //     next tick; the *energy* leaves at the stock's current density.
            let dec_d = (dc.decomposition * DT * d0).clamp(0.0, d);
            if dec_d > 0.0 {
                let removed = de * (dec_d / d);
                ledger.heat_out += removed;
                de -= removed;
                d -= dec_d;
                n += dec_d;
            }
            let dec_c = (dc.carrion_decomposition * DT * c0).clamp(0.0, c);
            if dec_c > 0.0 {
                let removed = ce * (dec_c / c);
                ledger.heat_out += removed;
                ce -= removed;
                c -= dec_c;
                n += dec_c;
            }
            let dec_wd = (dc.wood_decomposition * DT * wd0).clamp(0.0, wd);
            if dec_wd > 0.0 {
                ledger.heat_out += e_v * dec_wd;
                wd -= dec_wd;
                n += dec_wd;
            }

            // 3h reads `W⁴`, the wood **after** 3d, for donors and recipients alike (§4.0).
            // A stand that died in 3d therefore becomes an eligible recipient on the same
            // tick, rather than waiting for the next one: `W⁴ = 0` is bare by §3.1, whatever
            // the cell was at the start of the tick. (Astra's implementation review, finding
            // 1: reading the pre-tick class here made the result depend on a state the table
            // does not name.) Donor eligibility is unchanged by this — a cell that died has
            // `W⁴ = 0 < W_est` and was never a donor under either reading.
            work.class[i] = CellClass::of(w, pl.alive_min);

            self.n[i] = n;
            self.p[i] = p;
            self.d[i] = d;
            self.de[i] = de;
            self.f[i] = f_next;
            eco.wood[i] = w;
            eco.plant_reserve[i] = q;
            eco.dead_wood[i] = wd;
            eco.carrion[i] = c;
            eco.carrion_energy[i] = ce;
        }

        // --- 3f: litter and remains slide downhill. `work` holds the immutable `X⁵`/`Xe⁵`
        //     so every transfer is a function of what 3e left, never of material that
        //     arrived from an uphill cell in this same pass. The withdrawn fraction is
        //     `fall · dt · (1 − k_X · dt) · X⁻`: the portion of the **pre-tick** stock that
        //     decomposition did not already take, which is what keeps the two withdrawals
        //     inside one budget. A pure transfer — no ledger entry, no heat.
        let fall = dc.fall * DT;
        if fall > 0.0 {
            work.d5.copy_from_slice(&self.d);
            work.de5.copy_from_slice(&self.de);
            work.c5.copy_from_slice(&eco.carrion);
            work.ce5.copy_from_slice(&eco.carrion_energy);
            let left_d = 1.0 - (dc.decomposition * DT).min(1.0);
            let left_c = 1.0 - (dc.carrion_decomposition * DT).min(1.0);
            for cell in CellId::all(Topology::Cube, Scale::ONE) {
                let Some(down) = graph.downhill(cell) else { continue };
                let (here, there) = (cell.index(), down.index());
                let out_d = (fall * left_d * work.pre_d[here]).clamp(0.0, work.d5[here]);
                if out_d > 0.0 {
                    let out_e = work.de5[here] * (out_d / work.d5[here]);
                    self.d[here] -= out_d;
                    self.de[here] -= out_e;
                    self.d[there] += out_d;
                    self.de[there] += out_e;
                }
                let out_c = (fall * left_c * work.pre_c[here]).clamp(0.0, work.c5[here]);
                if out_c > 0.0 {
                    let out_e = work.ce5[here] * (out_c / work.c5[here]);
                    eco.carrion[here] -= out_c;
                    eco.carrion_energy[here] -= out_e;
                    eco.carrion[there] += out_c;
                    eco.carrion_energy[there] += out_e;
                }
            }
        }

        // --- 3g: nutrient diffusion over the cell graph; `diffuse` is conservative and never
        //     pushes flux across the open rim.
        scratch.0.values.copy_from_slice(&self.n);
        diffuse(&mut scratch.0, &mut scratch.1, graph, nc.diffusion * DT);
        self.n.copy_from_slice(&scratch.0.values[..]);

        // --- 3h: propagules (§4.8). One immutable snapshot of every donor's budget, then
        //     every transfer committed together, so no donor can be drained by the order its
        //     neighbours are visited in and no recipient can be credited twice.
        if pl.propagule_rate > 0.0 {
            work.budget.fill(0.0);
            work.incoming.fill(0.0);
            let mut any = false;
            for cell in CellId::all(Topology::Cube, Scale::ONE) {
                let j = cell.index();
                // A donor is alive **after 3d** — a stand that died this tick sends nothing.
                if work.class[j] != CellClass::Alive || eco.wood[j] < pl.donor_min {
                    continue;
                }
                let floor = pl.donor_reserve_floor * pl.reserve_cap * eco.wood[j];
                let spare = eco.plant_reserve[j] - floor;
                if !(spare > 0.0) {
                    continue;
                }
                let recipients = graph
                    .neighbors(cell)
                    .iter()
                    .flatten()
                    .filter(|n| work.class[n.index()].recipient())
                    .count();
                if recipients == 0 {
                    continue;
                }
                let b = spare.min(pl.propagule_rate * DT * recipients as f64);
                if b > 0.0 {
                    work.budget[j] = b;
                    any = true;
                }
            }
            if any {
                for cell in CellId::all(Topology::Cube, Scale::ONE) {
                    let j = cell.index();
                    if work.budget[j] <= 0.0 {
                        continue;
                    }
                    let recipients: Vec<usize> = graph
                        .neighbors(cell)
                        .iter()
                        .flatten()
                        .map(|n| n.index())
                        .filter(|i| work.class[*i].recipient())
                        .collect();
                    debug_assert!(!recipients.is_empty());
                    let each = work.budget[j] / recipients.len() as f64;
                    for i in recipients {
                        work.incoming[i] += each;
                    }
                    eco.plant_reserve[j] -= work.budget[j];
                    ledger.propagule_sent += work.budget[j];
                }
                let (w_frac, p_frac, q_frac) = (
                    pl.propagule_split[0],
                    pl.propagule_split[1],
                    pl.propagule_split[2],
                );
                for i in 0..CUBE_CELL_COUNT {
                    let s = work.incoming[i];
                    if s <= 0.0 {
                        continue;
                    }
                    let net = s / build;
                    let crossed_before = eco.wood[i] >= pl.alive_min && eco.wood[i] > 0.0;
                    eco.wood[i] += w_frac * net;
                    self.p[i] += p_frac * net;
                    eco.plant_reserve[i] += q_frac * net;
                    self.n[i] += pl.build * net;
                    ledger.heat_out += e_v * pl.build * net;
                    if !crossed_before && eco.wood[i] >= pl.alive_min && eco.wood[i] > 0.0 {
                        eco.recolonisations_total += 1;
                    }
                }
            }
        }

        ledger
    }

    /// Debug/telemetry check: finite and nonnegative everywhere, `De ≤ e_d_max · D + 1e-9`,
    /// water finite and nonnegative.
    pub fn check(&self, energy_cap: f64) -> Result<(), String> {
        crate::water::check(&self.w)?;
        for (name, v) in
            [("N", &self.n), ("P", &self.p), ("D", &self.d), ("De", &self.de), ("F", &self.f)]
        {
            if v.len() != CUBE_CELL_COUNT {
                return Err(format!("{name} has {} cells, expected {CUBE_CELL_COUNT}", v.len()));
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
        for i in 0..CUBE_CELL_COUNT {
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

/// The light producer growth sees in a cell holding depth `w` of water (`design/water.md`
/// "Algae"): `max(L, algae_light · min(w / algae_depth, 1))`. Dry cells see the sky's `L`
/// exactly; a pool at or beyond `algae_depth` is lit at least to `algae_light` however dark
/// the floor is, which is what lets a shallow pool grow a mat.
pub fn algae_light(w: f64, light: f64, cfg: &WorldConfig) -> f64 {
    if !(w.is_finite() && w > 0.0) {
        return light;
    }
    let wc = &cfg.water;
    let depth = if wc.algae_depth > 0.0 { (w / wc.algae_depth).min(1.0) } else { 1.0 };
    light.max(wc.algae_light * depth)
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
        light: Box<[f64; CUBE_CELL_COUNT]>,
        moisture: Box<[f64; CUBE_CELL_COUNT]>,
        graph: FieldGraph,
        scratch: (ScalarField, ScalarField),
        work: EcoScratch,
        /// The ecology v1 pools this harness steps alongside `Fields`. A test that only
        /// exercises `N`/`P`/`D`/`F` leaves it at the constructor's values.
        eco: EcologyV1State,
    }

    impl Harness {
        /// **Staging.** Every test in this module predates ecology v1 and is about `N`, `P`,
        /// `D`, `De`, `F` and water. So the harness gives every cell a mature stand with no
        /// maintenance bill, no construction respiration and no reserve: `A = min(A_pot,
        /// (P_cap − P)⁺)`, `ΔP = A`, `ΔN = −A`, which is the pre-ecology-v1 growth law with
        /// the structural cap in place of the logistic term. `W = 1` with `α = 2` puts
        /// `P_cap` at `P_max` exactly, so the ceiling is unchanged. Ecology v1's own
        /// arithmetic — maintenance, reflush, dieback, death, the three decompositions and
        /// propagules — is tested in `tests/ecology_v1.rs`, not here.
        fn new(cfg: WorldConfig) -> Harness {
            let mut cfg = cfg;
            cfg.plant.maintenance = 0.0;
            cfg.plant.build = 0.0;
            cfg.plant.reserve_cap = 0.0;
            cfg.plant.propagule_rate = 0.0;
            cfg.plant.foliage_rate = 1e6;
            let habitat = Habitat::new(&cfg.habitat, cfg.seed);
            let mut eco = EcologyV1State::new(&cfg, &habitat.light_base, &habitat.moisture_base);
            eco.wood.iter_mut().for_each(|w| *w = 1.0);
            eco.plant_reserve.iter_mut().for_each(|q| *q = 0.0);
            Harness {
                cfg,
                light: habitat.light_base.clone(),
                moisture: habitat.moisture_base.clone(),
                graph: FieldGraph::new(Topology::Cube, Scale::ONE),
                scratch: (ScalarField::zeros(Topology::Cube, Scale::ONE), ScalarField::zeros(Topology::Cube, Scale::ONE)),
                work: EcoScratch::default(),
                eco,
            }
        }

        fn fields(&self) -> Fields {
            Fields::new(&self.cfg, &self.light, &self.moisture)
        }

        fn react(&mut self, f: &mut Fields) -> FieldLedger {
            f.react(
                &mut self.eco,
                &self.cfg,
                &self.light,
                &self.moisture,
                &self.graph,
                &mut self.scratch,
                &mut self.work,
            )
        }

        /// The whole world's material: the fields plus the ecology v1 pools.
        fn material(&self, f: &Fields) -> f64 {
            f.total_material() + self.eco.total_material()
        }
    }

    /// The stored energy of contract §10 restricted to the fields:
    /// `e_v·(P + W + Q + Wd) + e_f·F + De + Ce`.
    fn stored_energy(h: &Harness, f: &Fields) -> f64 {
        let e_v = h.cfg.plant.energy_density;
        let e_f = h.cfg.fruit.energy_density;
        f.p.iter().map(|&p| e_v * p).sum::<f64>()
            + f.f.iter().map(|&x| e_f * x).sum::<f64>()
            + f.de.iter().sum::<f64>()
            + h.eco.stored_energy(e_v)
    }

    #[test]
    fn standing_water_lights_its_own_producers() {
        let cfg = WorldConfig::default();
        // The floor: dry cells see the sky exactly, a pool is lit to `algae_light` in
        // proportion to its depth up to `algae_depth`, and a brighter sky still wins.
        assert_eq!(algae_light(0.0, 0.1, &cfg), 0.1);
        assert_eq!(algae_light(f64::NAN, 0.1, &cfg), 0.1);
        let full = cfg.water.algae_light;
        assert!((algae_light(0.15, 0.0, &cfg) - 0.5 * full).abs() < 1e-12);
        assert!((algae_light(0.3, 0.0, &cfg) - full).abs() < 1e-12);
        assert!((algae_light(5.0, 0.0, &cfg) - full).abs() < 1e-12);
        assert_eq!(algae_light(1.0, 0.9, &cfg), 0.9);

        // A dark cell grows nothing dry and something wet; a flooded dark cell drowns.
        let mut h = Harness::new(cfg);
        h.cfg.producer.mortality = 0.0;
        h.cfg.detritus.decomposition = 0.0;
        h.cfg.nutrient.diffusion = 0.0;
        h.cfg.detritus.fall = 0.0;
        h.cfg.fruit.ripen = 0.0;
        let cell = 5;
        h.light[cell] = 0.0;
        let mut dry = h.fields();
        dry.p[cell] = 0.3;
        dry.n[cell] = 0.5;
        let mut wet = dry.clone();
        wet.w[cell] = 0.3;
        let mut flooded = dry.clone();
        flooded.w[cell] = 10.0 * h.cfg.water.flood;
        h.react(&mut dry);
        h.react(&mut wet);
        h.react(&mut flooded);
        assert_eq!(dry.p[cell], 0.3, "a dark dry cell grows nothing");
        assert!(wet.p[cell] > 0.3, "a dark pool grows a mat: {}", wet.p[cell]);
        assert_eq!(flooded.p[cell], 0.3, "deep water still drowns growth");
        // No algae light: the pool is as dark as the sky.
        h.cfg.water.algae_light = 0.0;
        let mut unlit = wet.clone();
        unlit.p[cell] = 0.3;
        h.react(&mut unlit);
        assert_eq!(unlit.p[cell], 0.3);
    }

    /// `design/ecology-v1-contract.md` §11: wood follows light and moisture, foliage follows
    /// the structure that carries it, and a cell whose wood would start below `W_min` starts
    /// bare rather than frozen in the establishing class.
    #[test]
    fn initial_fields_follow_the_contract() {
        let cfg = WorldConfig::default();
        let habitat = Habitat::new(&cfg.habitat, cfg.seed);
        let f = Fields::new(&cfg, &habitat.light_base, &habitat.moisture_base);
        let eco = EcologyV1State::new(&cfg, &habitat.light_base, &habitat.moisture_base);
        assert_eq!(f.n.len(), CUBE_CELL_COUNT);
        let mut alive = 0;
        let mut bare = 0;
        for i in 0..CUBE_CELL_COUNT {
            let (l, m) = (habitat.light_base[i], habitat.moisture_base[i]);
            assert_eq!(f.n[i], cfg.nutrient.initial);
            let want_w = cfg.plant.initial_wood * cfg.plant.wood_max * l * m;
            if want_w >= cfg.plant.alive_min {
                alive += 1;
                assert_eq!(eco.wood[i], want_w);
                assert_eq!(eco.plant_reserve[i], 0.5 * cfg.plant.reserve_cap * want_w);
                let p_cap = cfg.producer.max.min(cfg.plant.alpha * want_w);
                assert_eq!(f.p[i], cfg.producer.initial_fraction * p_cap);
                assert_eq!(CellClass::of(eco.wood[i], cfg.plant.alive_min), CellClass::Alive);
            } else {
                bare += 1;
                assert_eq!((eco.wood[i], eco.plant_reserve[i], f.p[i]), (0.0, 0.0, 0.0));
                assert_eq!(CellClass::of(eco.wood[i], cfg.plant.alive_min), CellClass::Bare);
            }
            let litter = cfg.detritus.initial_dark * (1.0 - l);
            assert_eq!(f.d[i], litter);
            assert_eq!(f.de[i], cfg.detritus.energy_cap * litter);
            assert_eq!(f.f[i], 0.0);
            assert_eq!((eco.dead_wood[i], eco.carrion[i], eco.carrion_energy[i]), (0.0, 0.0, 0.0));
        }
        assert!(alive > 0 && bare > 0, "the default habitat has both: {alive} alive, {bare} bare");
        assert_eq!((eco.plant_deaths_total, eco.recolonisations_total), (0, 0));
        f.check(cfg.detritus.energy_cap).unwrap();
        eco.check(cfg.detritus.carrion_energy_cap).unwrap();
    }

    /// `design/stratified-world.md` mechanism 2: the initial litter follows the dark, and a
    /// world without it starts clean.
    #[test]
    fn the_initial_litter_lies_where_it_is_dark() {
        let cfg = WorldConfig::default();
        assert_eq!(cfg.detritus.initial_dark, 1.2);
        let mut light = Box::new([0.0f64; CUBE_CELL_COUNT]);
        let moisture = Box::new([1.0f64; CUBE_CELL_COUNT]);
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
        let start = h.material(&f);
        let mut worst = 0.0f64;
        for tick in 0..600 {
            let before = h.material(&f);
            h.react(&mut f);
            let after = h.material(&f);
            let rel = (after - before).abs() / before;
            worst = worst.max(rel);
            assert!(rel < 1e-12, "tick {tick}: relative material drift {rel}");
            f.check(h.cfg.detritus.energy_cap).unwrap();
        }
        let drift = (h.material(&f) - start).abs() / start;
        assert!(drift < 1e-12, "600 ticks drifted by {drift} relative");
        println!("worst per-tick relative material drift over 600 ticks: {worst:e}");
        println!("cumulative relative drift: {drift:e}");
    }

    #[test]
    fn the_energy_ledger_matches_the_change_in_stored_energy() {
        let mut h = Harness::new(WorldConfig::default());
        let mut f = h.fields();
        let mut worst = 0.0f64;
        for tick in 0..600 {
            let before = stored_energy(&h, &f);
            let ledger = h.react(&mut f);
            let after = stored_energy(&h, &f);
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
        cfg.plant.energy_density = 20.0;
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
                let start = h.material(&f);
                for tick in 0..50 {
                    h.react(&mut f);
                    f.check(h.cfg.detritus.energy_cap).unwrap_or_else(|e| {
                        panic!("tick {tick} with uptake {uptake} growth {growth} P {p0}: {e}")
                    });
                    h.eco.check(h.cfg.detritus.carrion_energy_cap).unwrap();
                }
                let drift = (h.material(&f) - start).abs() / start.max(1e-12);
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
        let total = h.material(&f);
        for _ in 0..50 {
            h.react(&mut f);
        }
        assert!((h.material(&f) - total).abs() < 1e-12);
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
        for i in 0..CUBE_CELL_COUNT {
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
        let start_material = h.material(&f);
        for tick in 0..400 {
            let ledger = h.react(&mut f);
            // A transfer is neither a source nor a sink of energy.
            assert_eq!(ledger, FieldLedger::default(), "tick {tick}");
            let d: f64 = f.d.iter().sum();
            let de: f64 = f.de.iter().sum();
            assert!((d - d0).abs() / d0 < 1e-12, "tick {tick}: D drifted to {d} from {d0}");
            assert!((de - de0).abs() / de0 < 1e-12, "tick {tick}: De drifted to {de} from {de0}");
            for i in 0..CUBE_CELL_COUNT {
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
        let drift = (h.material(&f) - start_material).abs() / start_material;
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
        let source = CellId::new(Topology::Cube, Scale::ONE, Face::Right, 6, 0);
        let sink = CellId::new(Topology::Cube, Scale::ONE, Face::Right, 6, 15);
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
        for cell in CellId::all(Topology::Cube, Scale::ONE) {
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
            [CellId::new(Topology::Cube, Scale::ONE, Face::Top, 0, 0), CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8), CellId::new(Topology::Cube, Scale::ONE, Face::Front, 3, 15)];
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
        let cell = (0..CUBE_CELL_COUNT)
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
        assert_eq!(f.w.len(), CUBE_CELL_COUNT);
        let cap = h.cfg.detritus.energy_cap;
        f.check(cap).unwrap();
        f.w[7] = -0.5;
        assert!(f.check(cap).unwrap_err().contains("w[7]"));
    }
}
