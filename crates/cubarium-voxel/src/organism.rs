//! **The organism budget**: one tick of an organism's organic books, shared by the plant
//! stands, the latticevine and the animals.
//!
//! Every organism runs the same steps, in this order:
//! 1. **Income**, from its own income function: photosynthesis (a stand, the vine),
//!    saprotrophy (a fungus), feeding (an animal's assimilated bite). [`Budget::new`]
//!    takes it.
//! 2. **Upkeep from income, then reserve** ([`Budget::upkeep`]). What is paid is respired.
//! 3. **Build at `1 + c_g`** ([`Budget::build`], [`Budget::store`],
//!    [`Budget::build_from`], [`build_from`]): a unit of tissue costs one unit of organic
//!    matter plus `c_g` of construction respiration.
//! 4. **The mineral fraction rule** ([`tissue_funded`], [`settle_mineral`]): tissue holds
//!    `n_tissue` mineral per unit, so mineral funds `mineral / n_tissue` of it, and what was
//!    built settles its mineral against what arrived with the income.
//! 5. **Energy and heat**: whatever is respired ([`Budget::close`]) leaves the system with
//!    its energy as heat, which the organism books at its own density.
//! 6. **Dieback**: upkeep that neither income nor reserve paid ([`Upkeep::unpaid`]) comes
//!    out of the organism's tissue by its own rule: a stand's wood to dead wood, the vine's
//!    leaves burned, an animal's structure respired.
//!
//! The organism supplies its income, its demands, its costs and its dieback. No rate lives
//! here: every ecological number is the caller's config.

/// Tissue `mineral` can fund at `n_tissue` mineral per unit built: unbounded for a tissue
/// that needs none, never negative.
pub fn tissue_funded(mineral: f64, n_tissue: f64) -> f64 {
    if n_tissue > 0.0 {
        (mineral / n_tissue).max(0.0)
    } else {
        f64::INFINITY
    }
}

/// Upkeep as [`Budget::upkeep`] paid it, organic units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Upkeep {
    pub from_income: f64,
    pub from_reserve: f64,
    /// What neither paid: the organism's dieback settles it.
    pub unpaid: f64,
}

impl Upkeep {
    pub fn paid(&self) -> f64 {
        self.from_income + self.from_reserve
    }
}

/// One tick's books: income not yet spent, tissue the mineral still funds, and the
/// organic matter respired so far.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Budget {
    /// Income not yet spent, organic units.
    pub income: f64,
    /// Tissue the mineral budget still funds ([`tissue_funded`]).
    pub tissue_left: f64,
    /// Organic matter respired so far: paid upkeep and construction.
    pub respired: f64,
}

impl Budget {
    pub fn new(income: f64, tissue_left: f64) -> Budget {
        Budget {
            income,
            tissue_left,
            respired: 0.0,
        }
    }

    /// Pay `upkeep` out of the income first, then out of `reserve`. Both are respired.
    pub fn upkeep(&mut self, reserve: &mut f64, upkeep: f64) -> Upkeep {
        let from_income = self.income.min(upkeep);
        self.income -= from_income;
        let short = upkeep - from_income;
        let from_reserve = reserve.min(short).max(0.0);
        *reserve -= from_reserve;
        self.respired += from_income + from_reserve;
        Upkeep {
            from_income,
            from_reserve,
            unpaid: short - from_reserve,
        }
    }

    /// Put up to `want` of the unspent income into a reserve, at par: reserve is tissue
    /// with no construction cost, and it holds its mineral like any other.
    pub fn store(&mut self, want: f64) -> f64 {
        let x = want.min(self.income).min(self.tissue_left).max(0.0);
        self.income -= x;
        self.tissue_left -= x;
        x
    }

    /// Build up to `demand` units of tissue out of the unspent income at `1 + c_g` each,
    /// within the mineral budget. The `c_g` share is respired.
    pub fn build(&mut self, demand: f64, c_g: f64) -> f64 {
        let cost = 1.0 + c_g;
        let x = (self.income / cost).min(demand).min(self.tissue_left).max(0.0);
        self.income -= cost * x;
        self.tissue_left -= x;
        self.respired += c_g * x;
        x
    }

    /// Build up to `demand` units out of a held stock (a reserve) at `1 + c_g` each. The
    /// stock's mineral is already in the organism, so the mineral budget is not charged.
    pub fn build_from(&mut self, stock: &mut f64, demand: f64, c_g: f64) -> f64 {
        let x = build_from(stock, demand, 1.0 + c_g);
        self.respired += c_g * x;
        x
    }

    /// The tick's end: unspent income is respired. Returns the organic matter respired in
    /// all, whose energy leaves as heat.
    pub fn close(self) -> f64 {
        self.respired + self.income.max(0.0)
    }
}

/// Up to `demand` units out of `stock` at `cost` each, never more than the stock pays for.
pub fn build_from(stock: &mut f64, demand: f64, cost: f64) -> f64 {
    let x = (*stock / cost).min(demand).max(0.0);
    *stock -= cost * x;
    x
}

/// Mineral moved by [`settle_mineral`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Settled {
    /// Arrived mineral the tissue did not need, out to the organism's sink.
    pub released: f64,
    /// The shortfall, drawn from the pool.
    pub drawn: f64,
}

/// Settle the mineral of the tissue built: `need` is met from what `arrived` with the
/// income first, the excess is released, and a shortfall is drawn from `pool` as far as it
/// holds. Mineral only moves; it is never created or respired.
pub fn settle_mineral(arrived: f64, need: f64, pool: f64) -> Settled {
    if arrived > need {
        Settled {
            released: arrived - need,
            drawn: 0.0,
        }
    } else {
        Settled {
            released: 0.0,
            drawn: (need - arrived).min(pool.max(0.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upkeep_comes_from_income_then_reserve_and_the_rest_is_unpaid() {
        let mut reserve = 0.3;
        let mut b = Budget::new(0.5, f64::INFINITY);
        let up = b.upkeep(&mut reserve, 1.0);
        assert_eq!(up.from_income, 0.5);
        assert_eq!(up.from_reserve, 0.3);
        assert!((up.unpaid - 0.2).abs() < 1e-15);
        assert_eq!(reserve, 0.0);
        assert_eq!(b.income, 0.0);
        assert_eq!(b.respired, 0.8);

        let mut reserve = 1.0;
        let mut b = Budget::new(2.0, f64::INFINITY);
        let up = b.upkeep(&mut reserve, 0.5);
        assert_eq!((up.paid(), up.unpaid, reserve, b.income), (0.5, 0.0, 1.0, 1.5));
    }

    #[test]
    fn building_costs_one_plus_c_g_and_respires_the_c_g() {
        let mut b = Budget::new(1.2, f64::INFINITY);
        let x = b.build(10.0, 0.2);
        assert!((x - 1.0).abs() < 1e-12);
        assert!(b.income.abs() < 1e-12);
        assert!((b.respired - 0.2).abs() < 1e-12);
        assert!((b.close() - 0.2).abs() < 1e-12);
    }

    #[test]
    fn the_mineral_budget_bounds_what_is_built_and_stored() {
        let mut b = Budget::new(1.0, tissue_funded(0.01, 0.05));
        assert!((b.store(0.1) - 0.1).abs() < 1e-12);
        assert!((b.build(1.0, 0.0) - 0.1).abs() < 1e-12);
        assert_eq!(b.build(1.0, 0.0), 0.0);
        // The unfunded income is respired at the close.
        assert!((b.close() - 0.8).abs() < 1e-12);
        assert_eq!(tissue_funded(1.0, 0.0), f64::INFINITY);
        assert_eq!(tissue_funded(-1.0, 0.1), 0.0);
    }

    #[test]
    fn a_reserve_pays_for_what_it_holds_and_no_more() {
        let mut stock = 0.6;
        assert!((build_from(&mut stock, 1.0, 1.5) - 0.4).abs() < 1e-12);
        assert!(stock.abs() < 1e-12);
        let mut stock = 0.6;
        let mut b = Budget::default();
        assert_eq!(b.build_from(&mut stock, 0.1, 0.5), 0.1);
        assert!((b.respired - 0.05).abs() < 1e-15);
        assert!((stock - 0.45).abs() < 1e-15);
    }

    #[test]
    fn mineral_settles_by_moving_never_by_creating() {
        let s = settle_mineral(0.3, 0.1, 5.0);
        assert!((s.released - 0.2).abs() < 1e-15 && s.drawn == 0.0);
        let s = settle_mineral(0.1, 0.3, 0.05);
        assert_eq!((s.released, s.drawn), (0.0, 0.05));
        let s = settle_mineral(0.1, 0.3, 5.0);
        assert!((s.drawn - 0.2).abs() < 1e-15);
    }
}
