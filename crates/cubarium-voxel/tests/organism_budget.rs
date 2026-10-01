//! **The organism budget kernel**, from its semantics (audit §7,
//! `design/7_Research/ecology-parameter-audit-2026-09-28.md`): income, then upkeep from
//! income first and reserve second with the rest unpaid, then building at `1 + c_g` with
//! the `c_g` respired, the mineral fraction rule, and the leftover income respired at the
//! close. Property loops over a seeded generator, zeros and ties included; no pinned
//! numbers.

use cubarium_voxel::organism::{Budget, build_from, settle_mineral, tissue_funded};

/// splitmix64: enough randomness for property loops, no dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A non-negative finite amount: exact zeros, a few fixed edge values, and a
    /// log-uniform spread over fifteen decades otherwise.
    fn amount(&mut self) -> f64 {
        match self.next() % 10 {
            0 => 0.0,
            1 => [1.0, 0.5, 1e-15, 1e-300, 1e6][(self.next() % 5) as usize],
            _ => 10f64.powf(-12.0 + 15.0 * self.unit()),
        }
    }

    /// A construction respiration coefficient: zero sometimes, otherwise 0..3.
    fn c_g(&mut self) -> f64 {
        if self.next() % 4 == 0 {
            0.0
        } else {
            3.0 * self.unit()
        }
    }
}

fn close_to(a: f64, b: f64, scale: f64) -> bool {
    (a - b).abs() <= 1e-12 * scale.abs().max(1e-300) + 1e-300
}

fn sane(x: f64, what: &str, case: &str) {
    assert!(x.is_finite() && x >= 0.0, "{case}: {what} = {x:e}");
}

/// One tick through the whole budget, random inputs, random demand order. Every unit of
/// income and of reserve or stock drawn is accounted for as upkeep paid, tissue built,
/// matter stored or respired; upkeep is paid in order; nothing built costs more than
/// `1 + c_g`; the mineral budget bounds what income builds and stores; every output is
/// finite and non-negative.
#[test]
fn a_tick_conserves_organic_matter_pays_upkeep_in_order_and_builds_within_means() {
    let mut rng = Rng(0x0C3_C3_C3);
    for case_no in 0..20_000 {
        let income0 = rng.amount();
        let reserve0 = rng.amount();
        let stock0 = rng.amount();
        let upkeep = rng.amount();
        let tissue0 = if rng.next() % 5 == 0 {
            f64::INFINITY
        } else {
            rng.amount()
        };
        let case = format!(
            "case {case_no}: income {income0:e} reserve {reserve0:e} stock {stock0:e} \
             upkeep {upkeep:e} tissue_left {tissue0:e}"
        );

        let mut b = Budget::new(income0, tissue0);
        let mut reserve = reserve0;
        let up = b.upkeep(&mut reserve, upkeep);
        for (x, w) in [
            (up.from_income, "from_income"),
            (up.from_reserve, "from_reserve"),
            (up.unpaid, "unpaid"),
            (reserve, "reserve after upkeep"),
            (b.income, "income after upkeep"),
        ] {
            sane(x, w, &case);
        }
        // Paid plus unpaid is the upkeep asked for.
        assert!(
            close_to(up.paid() + up.unpaid, upkeep, upkeep),
            "{case}: paid {} + unpaid {} != upkeep",
            up.paid(),
            up.unpaid
        );
        assert!(close_to(
            up.paid(),
            up.from_income + up.from_reserve,
            up.paid()
        ));
        // Income first: income pays all of it, or is spent out before the reserve pays.
        assert!(
            close_to(up.from_income, income0.min(upkeep), upkeep),
            "{case}: income paid {:e}",
            up.from_income
        );
        if up.from_reserve > 0.0 {
            assert!(
                b.income <= 1e-12 * income0,
                "{case}: the reserve paid {:e} while {:e} income was unspent",
                up.from_reserve,
                b.income
            );
        }
        // Unpaid only once both are spent.
        if up.unpaid > 1e-12 * upkeep {
            assert!(
                b.income <= 1e-12 * income0 && reserve <= 1e-12 * reserve0,
                "{case}: unpaid {:e} with income {:e} and reserve {:e} left",
                up.unpaid,
                b.income,
                reserve
            );
        }
        assert!(
            close_to(reserve0 - reserve, up.from_reserve, reserve0),
            "{case}: the reserve lost {:e} but paid {:e}",
            reserve0 - reserve,
            up.from_reserve
        );

        // Demands in a random order, each a random size, some far above the income.
        let mut stock = stock0;
        let (mut built_income, mut built_stock, mut stored, mut construction) =
            (0.0, 0.0, 0.0, 0.0);
        for _ in 0..(rng.next() % 6) {
            let before = b;
            let demand = rng.amount();
            let c_g = rng.c_g();
            match rng.next() % 3 {
                0 => {
                    let x = b.build(demand, c_g);
                    sane(x, "built from income", &case);
                    assert!(x <= demand, "{case}: built {x:e} over demand {demand:e}");
                    assert!(
                        x * (1.0 + c_g) <= before.income.max(0.0) * (1.0 + 1e-12),
                        "{case}: built {x:e} at 1+{c_g} out of {:e} income",
                        before.income
                    );
                    assert!(
                        x <= before.tissue_left * (1.0 + 1e-12),
                        "{case}: built {x:e} past the mineral budget {:e}",
                        before.tissue_left
                    );
                    assert!(
                        close_to(before.income - b.income, (1.0 + c_g) * x, before.income),
                        "{case}: build took {:e} of income for {x:e} at 1+{c_g}",
                        before.income - b.income
                    );
                    assert!(close_to(b.respired - before.respired, c_g * x, b.respired));
                    built_income += x;
                    construction += c_g * x;
                }
                1 => {
                    // A share of the unspent income, over one sometimes.
                    let share = 3.0 * rng.unit();
                    let x = b.store(share * b.income);
                    sane(x, "stored", &case);
                    assert!(
                        x <= before.income.max(0.0),
                        "{case}: stored {x:e} out of {:e} income",
                        before.income
                    );
                    assert!(x <= before.tissue_left * (1.0 + 1e-12));
                    assert!(close_to(before.income - b.income, x, before.income));
                    assert_eq!(b.respired, before.respired, "{case}: storing respires");
                    stored += x;
                }
                _ => {
                    let held = stock;
                    let x = b.build_from(&mut stock, demand, c_g);
                    sane(x, "built from stock", &case);
                    assert!(x <= demand);
                    assert!(
                        x * (1.0 + c_g) <= held.max(0.0) * (1.0 + 1e-12),
                        "{case}: built {x:e} at 1+{c_g} out of {held:e} stock"
                    );
                    assert!(
                        close_to(held - stock, (1.0 + c_g) * x, held),
                        "{case}: stock lost {:e} for {x:e} at 1+{c_g}",
                        held - stock
                    );
                    assert_eq!(b.income, before.income, "{case}: the stock spent income");
                    assert_eq!(
                        b.tissue_left, before.tissue_left,
                        "{case}: the stock's mineral is already held"
                    );
                    assert!(stock >= -1e-12 * stock0, "{case}: stock {stock:e}");
                    built_stock += x;
                    construction += c_g * x;
                }
            }
            assert!(
                b.income >= -1e-12 * income0,
                "{case}: income {:e}",
                b.income
            );
            assert!(
                b.tissue_left >= 0.0,
                "{case}: tissue_left {:e}",
                b.tissue_left
            );
        }
        assert!(
            built_income + stored <= tissue0 * (1.0 + 1e-12),
            "{case}: income turned into {:e} of tissue past the mineral budget",
            built_income + stored
        );

        let left = b.income.max(0.0);
        let respired = b.close();
        sane(respired, "respired", &case);
        // The close respires the unspent income and nothing else new.
        let scale = income0 + reserve0 + stock0;
        assert!(
            close_to(respired, up.paid() + construction + left, scale),
            "{case}: respired {respired:e} != paid upkeep {:e} + construction \
             {construction:e} + leftover {left:e}",
            up.paid()
        );
        // The organic books: what came in or was drawn is what left or was put away.
        let drawn = income0 + (reserve0 - reserve) + (stock0 - stock);
        let spent = built_income + built_stock + stored + respired;
        assert!(
            close_to(drawn, spent, scale),
            "{case}: drawn {drawn:e} != built {:e} + stored {stored:e} + respired \
             {respired:e}",
            built_income + built_stock
        );
    }
}

/// With nothing to build or store, the whole income is either upkeep or respired at the
/// close; the reserve is touched only for upkeep the income could not pay.
#[test]
fn an_idle_tick_respires_its_income_and_keeps_its_reserve() {
    let mut rng = Rng(7);
    for _ in 0..5_000 {
        let (income, reserve0, upkeep) = (rng.amount(), rng.amount(), rng.amount());
        let mut reserve = reserve0;
        let mut b = Budget::new(income, f64::INFINITY);
        let up = b.upkeep(&mut reserve, upkeep);
        let respired = b.close();
        if upkeep <= income {
            assert_eq!(
                reserve, reserve0,
                "income {income:e} covered upkeep {upkeep:e}"
            );
            assert!(close_to(respired, income, income));
        } else {
            assert!(close_to(
                respired,
                income + up.from_reserve,
                income + reserve0
            ));
        }
    }
    // Zero everything: nothing moves.
    let mut reserve = 0.0;
    let mut b = Budget::new(0.0, 0.0);
    assert_eq!(b.upkeep(&mut reserve, 0.0), Default::default());
    assert_eq!((b.build(1.0, 0.5), b.store(1.0)), (0.0, 0.0));
    let mut stock = 0.0;
    assert_eq!(b.build_from(&mut stock, 1.0, 0.5), 0.0);
    assert_eq!(b.close(), 0.0);
}

/// Hand-checkable cases of the order and the price, independent of the property loop.
#[test]
fn the_order_and_the_price_on_round_numbers() {
    // Upkeep 3 against income 1, reserve 1.5: 1 from income, 1.5 from reserve, 0.5 unpaid.
    let mut reserve = 1.5;
    let mut b = Budget::new(1.0, f64::INFINITY);
    let up = b.upkeep(&mut reserve, 3.0);
    assert_eq!(
        (up.from_income, up.from_reserve, up.unpaid),
        (1.0, 1.5, 0.5)
    );
    assert_eq!(reserve, 0.0);
    // Nothing left to build with.
    assert_eq!(b.build(1.0, 0.0), 0.0);
    assert_eq!(b.close(), 2.5);

    // Income 3 at c_g 0.5 builds at most 2, respiring 1, and the close adds nothing.
    let mut b = Budget::new(3.0, f64::INFINITY);
    assert_eq!(b.build(10.0, 0.5), 2.0);
    assert_eq!(b.close(), 1.0);

    // A demand smaller than the means is met exactly; the rest is respired at the close.
    let mut b = Budget::new(3.0, f64::INFINITY);
    assert_eq!(b.build(1.0, 0.5), 1.0);
    assert_eq!(b.close(), 0.5 + 1.5);

    // Store asks for twice the income and gets the income, never more.
    let mut b = Budget::new(0.25, f64::INFINITY);
    assert_eq!(b.store(0.5), 0.25);
    assert_eq!(b.store(1.0), 0.0);
    assert_eq!(b.close(), 0.0);

    // The free build_from: a stock of 1 at cost 4 pays for a quarter.
    let mut stock = 1.0;
    assert_eq!(build_from(&mut stock, 1.0, 4.0), 0.25);
    assert_eq!(stock, 0.0);
}

/// `tissue_funded` is what mineral pays for at `n_tissue` per unit: never more than the
/// mineral covers, never negative, unbounded when the tissue holds none.
#[test]
fn tissue_funded_never_overspends_mineral() {
    let mut rng = Rng(11);
    for _ in 0..10_000 {
        let (mineral, n) = (rng.amount(), rng.amount());
        let t = tissue_funded(mineral, n);
        assert!(
            !t.is_nan() && t >= 0.0,
            "tissue_funded({mineral:e}, {n:e}) = {t:e}"
        );
        if n > 0.0 && t.is_finite() {
            assert!(
                t * n <= mineral * (1.0 + 1e-12),
                "tissue_funded({mineral:e}, {n:e}) = {t:e} costs {:e}",
                t * n
            );
        }
    }
    assert_eq!(tissue_funded(0.0, 0.0), f64::INFINITY);
    assert_eq!(tissue_funded(0.0, 0.1), 0.0);
    assert_eq!(tissue_funded(0.2, 0.1), 2.0);
}

/// `settle_mineral` only moves mineral: it releases at most what arrived, draws at most
/// what the pool holds, never both, and the tissue ends with `need` when arrival and pool
/// together cover it and with all of both otherwise.
#[test]
fn settle_mineral_moves_and_never_creates() {
    let mut rng = Rng(13);
    for case_no in 0..20_000 {
        let (arrived, need, pool) = (rng.amount(), rng.amount(), rng.amount());
        // Ties matter: arrival exactly the need, or the pool exactly the shortfall.
        let (arrived, pool) = match rng.next() % 6 {
            0 => (need, pool),
            1 if arrived < need => (arrived, need - arrived),
            _ => (arrived, pool),
        };
        let case = format!("case {case_no}: arrived {arrived:e} need {need:e} pool {pool:e}");
        let s = settle_mineral(arrived, need, pool);
        sane(s.released, "released", &case);
        sane(s.drawn, "drawn", &case);
        assert!(s.released <= arrived, "{case}: released {:e}", s.released);
        assert!(s.drawn <= pool, "{case}: drew {:e}", s.drawn);
        assert!(
            s.released == 0.0 || s.drawn == 0.0,
            "{case}: released {:e} and drew {:e}",
            s.released,
            s.drawn
        );
        let in_tissue = arrived - s.released + s.drawn;
        let scale = arrived + need + pool;
        assert!(
            close_to(in_tissue, need.min(arrived + pool), scale),
            "{case}: the tissue holds {in_tissue:e}"
        );
    }
    let s = settle_mineral(0.0, 0.0, 0.0);
    assert_eq!((s.released, s.drawn), (0.0, 0.0));
    let s = settle_mineral(0.3, 0.3, 1.0);
    assert_eq!((s.released, s.drawn), (0.0, 0.0));
    let s = settle_mineral(0.0, 0.5, 0.0);
    assert_eq!((s.released, s.drawn), (0.0, 0.0));
}

/// End to end, the stand's use: a mineral budget of `tissue_funded(arrived + pool)`, then
/// build and store out of the income, then settle. Every unit of tissue made out of the
/// income ends holding its full `n_tissue`, and the pool never goes below zero.
#[test]
fn tissue_built_within_the_mineral_budget_always_settles_its_full_mineral() {
    let mut rng = Rng(17);
    for case_no in 0..10_000 {
        let (income, arrived, pool, n) = (rng.amount(), rng.amount(), rng.amount(), rng.amount());
        let case =
            format!("case {case_no}: income {income:e} arrived {arrived:e} pool {pool:e} n {n:e}");
        let mut b = Budget::new(income, tissue_funded(arrived + pool, n));
        let mut made = b.store(rng.unit() * 2.0 * income);
        made += b.build(rng.amount(), rng.c_g());
        made += b.store(rng.amount());
        let need = n * made;
        let s = settle_mineral(arrived, need, pool);
        let in_tissue = arrived - s.released + s.drawn;
        assert!(
            close_to(in_tissue, need, arrived + pool + need),
            "{case}: made {made:e} needing {need:e}, the tissue holds {in_tissue:e}"
        );
        assert!(
            pool - s.drawn >= -1e-12 * pool,
            "{case}: pool {:e}",
            pool - s.drawn
        );
    }
}
