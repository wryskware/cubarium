//! The continuous balance measures (the search rebase's addendum): a hand-built ledger's
//! `B`, days to empty, surplus and `R̂`; a starving body shows `B < 1` with finite days to
//! empty; a fed one `B >= 1`; and the soft target. Each at most 200 ticks.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Controller, Fauna, FaunaConfig, FaunaLedger, Founder, Response,
    StartingStores,
};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig};

use super::balance::{Books, Context, Measures, context, days, measures, shortfall};
use super::targets::{LineageOutcome, StageSpec, judge};

/// A hand-built frondgrazer lineage over 2 world days: it assimilated 0.3, paid upkeep
/// 0.4, motor 0.1 and lost 0.1 to riders, with stores of 0.6 at the end:
/// - `B = 0.3 / (0.4 + 0.1 + 0.1) = 0.5`;
/// - the deficit is 0.3 over 2 days, 0.15 a day, so the stores last 4 days;
/// - surplus per adult-day `(0.3 − 0.4 − 0.1) / 4 adult-days = −0.05`, so `R̂` is
///   −0.05 x 8 days / 0.3 per young < 0;
/// - access: 300 of 400 wanting ticks spent eating, 0.75; sated share 600 of 1,000 awake.
#[test]
fn a_hand_built_ledger_gives_the_measures() {
    let f = Founder::Browser;
    let mut l = FaunaLedger::default();
    let i = f.index();
    l.assimilated_by_founder[i] = 0.3;
    l.upkeep_by_founder[i] = 0.4;
    l.motor_by_founder[i] = 0.1;
    l.rider_take_lost_by_founder[i] = 0.1;
    l.wanting_ticks_by_founder[i] = 400;
    l.eating_ticks_by_founder[i] = 300;
    l.awake_ticks_by_founder[i] = 1000;
    let b = Books::of(&l, f).since(&Books::default());
    let c = Context { stores: 0.6, adult_days: 4.0, days: 2.0, lifespan_days: 8.0, offspring_cost: 0.3 };
    let m = measures(&b, &c);
    assert!((m.energy_balance - 0.5).abs() < 1e-12, "{m:?}");
    assert!((m.days_to_empty.expect("B < 1") - 4.0).abs() < 1e-12, "{m:?}");
    assert!((m.surplus_per_adult_day + 0.05).abs() < 1e-12, "{m:?}");
    assert!((m.r_hat - (-0.05 * 8.0 / 0.3)).abs() < 1e-12, "{m:?}");
    assert!((m.access - 0.75).abs() < 1e-12, "{m:?}");
    // 600 of 1,000 awake ticks with the latch off.
    assert!((m.sated_share - 0.6).abs() < 1e-12, "{m:?}");

    // A fed lineage: assimilated 0.9 against the same spend. B 1.5, no days to empty;
    // surplus (0.9 − 0.5) / 4 = 0.1 a day, R̂ = 0.1 x 8 / 0.3 = 2.67.
    l.assimilated_by_founder[i] = 0.9;
    let m = measures(&Books::of(&l, f), &c);
    assert!((m.energy_balance - 1.5).abs() < 1e-12 && m.days_to_empty.is_none(), "{m:?}");
    assert!((m.r_hat - 0.1 * 8.0 / 0.3).abs() < 1e-12, "{m:?}");
    assert_eq!(shortfall(&m), 0.0);
}

/// The soft target: `(1 − min(1, B)) + (1 − min(1, R̂))`, nothing measured counts as
/// missed; judged only for a lineage listed and not readout-only.
#[test]
fn the_balance_target_is_soft_and_skips_readouts() {
    let half = Measures { energy_balance: 0.5, r_hat: 0.25, ..Measures::default() };
    assert!((shortfall(&half) - 1.25).abs() < 1e-12);
    assert_eq!(shortfall(&Measures::default()), 2.0, "nothing measured is missed");
    let mut spec = StageSpec::default();
    spec.balance[Founder::Browser.index()] = true;
    let mut o = LineageOutcome::new(Founder::Browser, 8);
    o.alive_end = 8;
    o.balance = half;
    judge(&mut o, &spec, true);
    assert!((o.balance_shortfall - 1.25).abs() < 1e-12, "{o:?}");
    assert!(o.met, "balance is soft: the stage verdicts stand");
    spec.readout_only[Founder::Browser.index()] = true;
    judge(&mut o, &spec, true);
    assert_eq!(o.balance_shortfall, 0.0, "a readout-only lineage is not scored");
    spec.readout_only[Founder::Browser.index()] = false;
    spec.balance[Founder::Browser.index()] = false;
    judge(&mut o, &spec, true);
    assert_eq!(o.balance_shortfall, 0.0, "nor one not listed");
}

/// Flat soil with the day clock off, so a nocturnal shredder is awake.
fn flat_world() -> World {
    let mut config = VoxelConfig { width: 8, height: 6, depth: 6, voxel_m: 0.25, ..VoxelConfig::default() };
    config.day.mode = cubarium_voxel::weather::DayMode::Off;
    let mut world = World::empty(config);
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    world
}

struct Fixed(Actions);

impl Controller for Fixed {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        Response::Bounded(self.0)
    }
    fn reset(&mut self) {}
}

/// One shredder, awake, on flat soil, standing and feeding with `feed`,
/// on `litter` organic of litter, from `stores`, for 200 ticks: its books and measures.
fn shredder(config: FaunaConfig, stores: StartingStores, litter: f64, feed: f64) -> (Books, Measures, FaunaLedger) {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::for_voxel_size(0.25));
    let mut fauna = Fauna::new(config);
    let f = Founder::Blind;
    assert!(fauna.apply(&world, FaunaCommand::IntroduceFounder { x: 3, z: 3, founder: f, stores, heading_rad: 0.0 }));
    let (id, site) = {
        let a = &fauna.view().animals[0];
        (a.id, a.site)
    };
    if litter > 0.0 {
        assert!(flora.deposit(
            site,
            Deposit { kind: DepositKind::Litter, organic: litter, mineral: 0.02 * litter, energy: 2.0 * litter }
        ));
    }
    let controller: Box<dyn Controller> = Box::new(Fixed(Actions { forward: 0.0, turn: 0.0, feed }));
    assert!(fauna.set_controller(id, controller));
    let start = Books::of(fauna.view().ledger, f);
    const TICKS: u32 = 200;
    for _ in 0..TICKS {
        fauna.step(&world, &mut flora);
    }
    let v = fauna.view();
    let books = Books::of(v.ledger, f).since(&start);
    let span = days(f64::from(TICKS) / f64::from(cubarium_voxel_fauna::TICK_HZ), v.config);
    let adult = super::balance::is_adult(&v.animals[0], v.config);
    let ctx = context(v.animals, v.config, f, if adult { span } else { 0.0 }, span);
    (books, measures(&books, &ctx), *v.ledger)
}

/// A shredder with nothing in its stomach and no food: it pays upkeep from its reserve and
/// assimilates nothing, so `B` is 0 and its stores last a finite number of days, its
/// reserve over its daily deficit. The ledger's upkeep is the one body's maintenance.
#[test]
fn a_starving_body_has_b_below_one_and_finite_days_to_empty() {
    let mut config = FaunaConfig::default();
    config.founders[Founder::Blind.index()].gut.s0 = 0.0;
    let (books, m, ledger) = shredder(config, StartingStores::FULL, 0.0, 0.0);
    assert_eq!(books.assimilated, 0.0);
    assert!(books.upkeep > 0.0 && books.upkeep == ledger.respired_maintenance_out, "{books:?}");
    assert_eq!(m.energy_balance, 0.0);
    let d = m.days_to_empty.expect("B < 1");
    // At f 0.03 of a full body with reserve cap 0.5, awake (0.5 / 0.03 days) or asleep at
    // 0.3 of that (0.5 / 0.009): between the two.
    assert!(d.is_finite() && d > 0.99 * 0.5 / 0.03 && d < 1.01 * 0.5 / 0.009, "{d}");
    assert!(m.surplus_per_adult_day < 0.0 && m.r_hat < 0.0, "{m:?}");
}

/// A half-full shredder standing on a litter pile, feeding: it wants food, eats, and
/// digests into the room its reserve has, more than its upkeep and motor: `B >= 1`, no
/// days to empty, and it ate on the ticks it wanted.
#[test]
fn a_fed_body_has_b_at_least_one() {
    let mut config = FaunaConfig::default();
    config.founders[Founder::Blind.index()].gut.s0 = 0.0;
    let stores = StartingStores { body: 1.0, reserve: 0.5 };
    let (books, m, _) = shredder(config, stores, 0.5, 1.0);
    assert!(books.assimilated > 0.0 && books.wanting_ticks > 0 && books.eating_ticks > 0, "{books:?}");
    assert!(m.energy_balance >= 1.0, "{m:?} {books:?}");
    assert!(m.days_to_empty.is_none());
    assert!(m.access > 0.0 && m.access <= 1.0, "{m:?}");
    assert!(books.awake_ticks >= books.wanting_ticks && m.sated_share >= 0.0 && m.sated_share < 1.0, "{m:?}");
}

/// With `counts_readout` the population band, its peak and the lanternjaw share are
/// reported but left out of the total; the balance term is in it.
#[test]
fn alive_counts_are_readouts_and_balance_is_scored() {
    use super::search::{Band, BandSpec, Trace, penalty};
    let band = Band::for_founding(10, true, &BandSpec::default());
    let mut t = Trace {
        horizon_min: 120.0,
        minutes: (1..=120).map(f64::from).collect(),
        animals: vec![100; 120],
        lanternjaws: vec![0; 120],
        browser_moving: 1.0,
        balance: 3.0,
        ..Trace::default()
    };
    let scored = penalty(&t, &band);
    assert!(scored.bound > 0.0 && scored.peak > 0.0);
    assert!((scored.total - (scored.bound + scored.peak + scored.share + scored.lively + scored.persist + 3.0)).abs() < 1e-9);
    t.counts_readout = true;
    let p = penalty(&t, &band);
    assert_eq!((p.bound, p.peak), (scored.bound, scored.peak), "still reported");
    assert!((p.total - (p.lively + p.persist + 3.0)).abs() < 1e-9, "{p:?}");
}
