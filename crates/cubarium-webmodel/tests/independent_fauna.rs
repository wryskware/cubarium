//! **The independent test pass on the animal half** (`design/webmodel/fauna.md`), written from
//! the sim's own code (`crates/cubarium-voxel-fauna/src/step.rs`, `lib.rs`, `predation.rs`),
//! not from the model's. Each test names the sim rule it holds the model to. The sim side is
//! either a one-body fixture stepped by the sim itself or the sim's own public function; the
//! expectations are computed from the config on hand, so no tuned number is pinned.
//!
//! Tests marked `#[ignore = "BUG: …"]` found a defect in the model and are left failing on
//! purpose (the brief: don't change model code).

use cubarium_voxel::weather::DayMode;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command, Controller, DT, Dormancy, Fauna, FaunaConfig, Food, Founder, Response as CtlResponse, StartingStores, TICK_HZ,
};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site};
use cubarium_voxel_sim::params::Params;
use cubarium_webmodel::{Supply, Take};
use cubarium_webmodel::fauna::model::EggCohort;
use cubarium_webmodel::fauna::{Body, Cohort, Draws, FaunaModel, Fits, Response, Rng, laws};

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-300)
}

fn patch() -> FaunaConfig {
    cubarium_webmodel::fauna::config::scenario("config/scenarios/patch.toml", &[]).expect("patch resolves").fauna
}

/// A model with only `on` stepped, the day clock off, the sim's own `cfg`.
fn model(cfg: &FaunaConfig, fits: Fits, dt: f64, on: &[Founder]) -> FaunaModel {
    let mut p = Params::defaults();
    p.fauna = cfg.clone();
    p.world.day.mode = DayMode::Off;
    let mut m = FaunaModel::new(&p, fits, dt);
    for g in Founder::ALL {
        m.enabled[g.index()] = on.contains(&g);
    }
    m
}

/// Everything respired, by every line the model books respiration on.
fn respired(m: &FaunaModel) -> f64 {
    m.ledger.iter().map(|l| l.flows.upkeep + l.flows.motor + l.flows.digestion_heat + l.gestation_loss + l.bank_upkeep).sum()
}

fn took(t: &Take) -> f64 {
    t.litter + t.carrion + t.film + t.nectar + t.fruit + t.foliage.iter().sum::<f64>()
}

// ===================================================================================
// The sim side: a one-body fixture on a soil plain (clock off).
// ===================================================================================

struct Feed(f64);
impl Controller for Feed {
    fn drive(&mut self, _o: &[f64]) -> CtlResponse {
        CtlResponse::Bounded(Actions { forward: 0.0, turn: 0.0, feed: self.0 })
    }
    fn reset(&mut self) {}
}

const W: u32 = 16;
const D: u32 = 8;
const Y: u32 = 2;

fn plain() -> World {
    let mut w = World::empty(VoxelConfig {
        width: W,
        height: 6,
        depth: D,
        voxel_m: 0.125,
        seed: 11,
        day: cubarium_voxel::weather::DayClock { mode: DayMode::Off, ..Default::default() },
        ..VoxelConfig::default()
    });
    for z in 0..D {
        for x in 0..i64::from(W) {
            for y in 1..=Y {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

/// One founder of `f` introduced at `stores` on the plain, fed by a controller that asks
/// for `feed` of its full bite; litter (deep, mineral-rich) on every face when `litter`.
fn one_body(cfg: FaunaConfig, f: Founder, stores: StartingStores, feed: f64, litter: bool) -> (World, Flora, Fauna, u64) {
    let world = plain();
    let mut flora = Flora::new(FloraConfig::default());
    if litter {
        for z in 0..D {
            for x in 0..W {
                flora.deposit(Site { x, y: Y, z }, Deposit { kind: DepositKind::Litter, organic: 50.0, mineral: 5.0, energy: 100.0 });
            }
        }
    }
    let mut fauna = Fauna::new(cfg);
    assert!(fauna.apply(&world, Command::IntroduceFounder { x: 8, z: 4, founder: f, stores, heading_rad: 0.0 }), "fixture: the founder is introduced");
    let id = fauna.view().animals.last().unwrap().id;
    fauna.set_controller(id, Box::new(Feed(feed)));
    (world, flora, fauna, id)
}

// ===================================================================================
// 1. Cohort merging (numerics: it must keep every stock and kill nobody)
// ===================================================================================

/// Per-body stocks a merge must keep in total: bodies, structure, reserve, brood, each
/// stomach class, the fresh-kill carcass, and the organic matter held.
fn totals(m: &FaunaModel, f: Founder) -> Vec<f64> {
    let v = &m.pops[f.index()];
    let mut t = vec![
        v.iter().map(|c| c.n).sum(),
        v.iter().map(|c| c.n * c.b.body).sum(),
        v.iter().map(|c| c.n * c.b.reserve).sum(),
        v.iter().map(|c| c.n * c.b.brood).sum(),
        v.iter().map(|c| c.carcass).sum(),
        m.organic_held(),
    ];
    for food in Food::ALL {
        t.push(v.iter().map(|c| c.n * c.b.stomach[food.index()]).sum());
    }
    t
}

/// **Merging keeps every stock exactly.** A merge pass over hand-built cohorts (spread in
/// structure, reserve, brood, two stomach classes, carcass, clocks; some tiny, some large)
/// leaves the lineage's bodies, structure, reserve, brood, each stomach class, carcass and
/// organic matter where they were, to rounding; every merged cohort keeps `brood ≤ reserve`
/// (the sim's `brood_held`), and the bound holds.
///
/// The model has no energy or mineral currency (it books organic matter only), so "energy"
/// in the brief has nothing to check here.
#[test]
fn merging_keeps_every_stock_exactly() {
    let cfg = patch();
    let f = Founder::Blind;
    let sc = cfg.founder(f).core;
    let mut m = model(&cfg, Fits::unfitted(), 1.0, &[f]);
    m.max_cohorts = 3;
    m.min_cohort = 0.05;
    for k in 0..14 {
        let x = f64::from(k) / 13.0;
        let body = sc.body_min + x * (sc.body_max - sc.body_min);
        let reserve = (0.2 + 0.7 * (1.0 - x)) * sc.reserve_of(body);
        let mut c = Cohort::new(if k % 4 == 0 { 0.003 + 0.01 * x } else { 0.5 + 3.0 * x }, body, reserve);
        c.b.brood = if body >= sc.birth_body { 0.6 * reserve } else { 0.0 };
        c.b.stomach[Food::Litter.index()] = 0.3 * x * cfg.founder(f).gut.capacity(body);
        c.b.stomach[Food::Carrion.index()] = 0.1 * (1.0 - x) * cfg.founder(f).gut.capacity(body);
        c.carcass = 0.01 * x;
        c.age_s = 3000.0 * x;
        c.refractory_s = 400.0 * (1.0 - x);
        c.matured = body >= sc.birth_body;
        c.native = true;
        m.add(f, c);
    }
    let before = totals(&m, f);
    m.merge_cohorts();
    let after = totals(&m, f);
    let names = ["bodies", "structure", "reserve", "brood", "carcass", "organic", "stomach"];
    for (i, (a, b)) in before.iter().zip(&after).enumerate() {
        assert!(close(*a, *b, 1e-12), "{} {a} -> {b}", names[i.min(6)]);
    }
    // The pass merged across the two booked stages only where it may (matured differs is
    // infinite distance), so the bound is met per compatible class: at most 3 + 1.
    assert!(m.pops[f.index()].len() <= 4, "cohorts after the pass: {}", m.pops[f.index()].len());
    for c in &m.pops[f.index()] {
        assert!(c.b.brood <= c.b.reserve * (1.0 + 1e-12), "brood {} over reserve {}", c.b.brood, c.b.reserve);
    }
}

/// Hatchling cohorts of lineage `f` at exactly `body_min`, of sizes `ns`, holding half a
/// reserve cap each.
fn hatchlings(cfg: &FaunaConfig, f: Founder, ns: &[f64]) -> FaunaModel {
    let sc = cfg.founder(f).core;
    let mut m = model(cfg, Fits::unfitted(), 1.0, &[f]);
    for &n in ns {
        let mut c = Cohort::new(n, sc.body_min, 0.5 * sc.reserve_of(sc.body_min));
        c.native = true;
        m.add(f, c);
    }
    m
}

/// **A merge never takes a body below `body_min`** (the sim's starvation rule, `deaths`,
/// `step.rs:3780`: `body < body_min` dies). Hatchlings are built at exactly `body_min`
/// (`hatch`, `step.rs:3435`; the model's `Cohort::new(n, body_min, …)`), so merging
/// hatchling cohorts of any sizes must leave the merged structure at or above every
/// input's. The flora half found and fixed this bug in its own merge (`flora.md`, finding
/// 2: a weighted mean a rounding below `alive_min`); the animal half's `absorb` takes
/// `x·wa + y·wb` unclamped and rounds below.
#[test]
fn a_merge_never_takes_a_body_below_body_min() {
    let cfg = patch();
    for f in [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail, Founder::Lanternjaw] {
        let sc = cfg.founder(f).core;
        let mut worst = f64::INFINITY;
        for i in 1..400 {
            let na = f64::from(i) * 0.0173 + 0.01;
            let nb = 1.0 / na + 0.37 * f64::from(i % 7);
            let mut m = hatchlings(&cfg, f, &[na, nb]);
            m.merge_cohorts();
            assert_eq!(m.pops[f.index()].len(), 1, "fixture: identical hatchlings merge");
            worst = worst.min(m.pops[f.index()][0].b.body - sc.body_min);
        }
        assert!(worst >= 0.0, "{}: a merged hatchling sits {worst:e} under body_min {}", f.name(), sc.body_min);
    }
}

/// **The consequence, where nothing else moves the structure**: with upkeep, motor and
/// growth switched off in the config, merged hatchlings at `body_min` must not starve. (In a
/// running world the step's own growth lifts a fed hatchling past the rounding before the
/// death check, so the defect above bites only a body whose free reserve is exactly zero.)
#[test]
fn merged_hatchlings_at_body_min_do_not_starve() {
    let mut cfg = patch();
    let f = Founder::Blind;
    cfg.founders[f.index()].core.maintenance_per_s = 0.0;
    cfg.founders[f.index()].core.growth_max_per_s = 0.0;
    cfg.founders[f.index()].motor_respiration_per_s = 0.0;
    let mut starved = 0.0;
    for i in 1..200 {
        let na = f64::from(i) * 0.0173 + 0.01;
        let mut m = hatchlings(&cfg, f, &[na, 1.0 / na + 0.37 * f64::from(i % 7)]);
        m.merge_every_s = 1.0;
        for _ in 0..3 {
            m.step(&Supply::default());
        }
        starved += m.ledger[f.index()].starved;
    }
    assert_eq!(starved, 0.0, "merges starved {starved} hatchlings that were at body_min");
}

// ===================================================================================
// 2. The kill: reach to the killer, the rest to carrion, the ledger closes
// ===================================================================================

/// A hungry lanternjaw cohort (it cannot eat this step: its stomach is full) over three
/// shredder cohorts of different sizes, all in its size window.
fn hunt_world(reach: f64, alpha: f64) -> FaunaModel {
    let cfg = FaunaConfig::default();
    let mut fits = Fits::unfitted();
    fits.kill_reach = reach;
    fits.of_mut(Founder::Blind).kill_rate = alpha;
    fits.of_mut(Founder::Blind).motor_effort = 0.0;
    let mut m = model(&cfg, fits, 1.0, &[Founder::Lanternjaw, Founder::Blind]);
    let lj = cfg.founder(Founder::Lanternjaw);
    let mut h = Cohort::new(2.0, lj.core.body_max, 0.05 * lj.core.reserve_of(lj.core.body_max));
    h.b.stomach[Food::Carrion.index()] = lj.gut.capacity(lj.core.body_max);
    h.b.eating = true;
    m.add(Founder::Lanternjaw, h);
    let bs = cfg.founder(Founder::Blind).core;
    for (n, x) in [(6.0, 0.0), (4.0, 0.5), (3.0, 1.0)] {
        let body = bs.body_min * 1.5 + x * (bs.body_max - bs.body_min * 1.5);
        let mut c = Cohort::new(n, body, 0.5 * bs.reserve_of(body));
        c.b.stomach[Food::Litter.index()] = 0.2 * cfg.founder(Founder::Blind).gut.capacity(body);
        m.add(Founder::Blind, c);
    }
    m
}

/// **The kill split** (`kill` → `bury_corpse`, `step.rs:2199-2245`: the whole body — reserve,
/// structure and the stomach's raw food — is the corpse). One step: the organic matter of
/// the bodies that left the prey cohorts is the corpse; `kill_reach` of it is laid at the
/// killer's mouth (its carcass), the rest returned to the carrion pool at once; the kills
/// are booked on both lineages.
#[test]
fn a_kill_goes_by_its_reach_to_the_killer_and_the_rest_to_carrion() {
    for reach in [0.0, 0.37, 1.0] {
        let mut m = hunt_world(reach, 2e-3);
        let before: Vec<f64> = m.pops[Founder::Blind.index()].iter().map(|c| c.n).collect();
        let (_, ret) = m.step(&Supply::default());
        let after = &m.pops[Founder::Blind.index()];
        assert_eq!(after.len(), before.len(), "fixture: no prey cohort emptied or merged");
        let killed: f64 = before.iter().zip(after).map(|(b, c)| b - c.n).sum();
        let corpse: f64 = before.iter().zip(after).map(|(b, c)| (b - c.n) * c.b.organic()).sum();
        assert!(killed > 0.0, "the hunter killed");
        assert!(close(m.ledger[Founder::Blind.index()].killed_by[Founder::Lanternjaw.index()], killed, 1e-12));
        assert!(close(m.ledger[Founder::Lanternjaw.index()].kills, killed, 1e-12));
        let held: f64 = m.pops[Founder::Lanternjaw.index()].iter().map(|c| c.carcass).sum();
        let decayed = m.carcass_decayed;
        assert!(close(held + decayed, reach * corpse, 1e-9), "reach {reach}: carcass {} of corpse {corpse}", held + decayed);
        assert!(close(ret.carrion, (1.0 - reach) * corpse, 1e-9) || (reach == 1.0 && ret.carrion == 0.0), "reach {reach}: carrion {} of corpse {corpse}", ret.carrion);
    }
}

/// **The hunt's ledger closes**: over ten minutes of a hunter that kills, feeds at its
/// carcass, leaves it (finite stay) and pays its strikes, beside prey that eat litter, the
/// organic matter held changes by what was eaten from the pools, less returns, respiration
/// and the carcass that decayed.
#[test]
fn the_hunts_ledger_closes() {
    let mut m = hunt_world(0.4, 5e-4);
    m.fits.carcass_stay_s = 200.0;
    for c in m.pops[Founder::Lanternjaw.index()].iter_mut() {
        c.b.stomach = [0.0; Food::COUNT];
    }
    let mut s = Supply::default();
    s.litter = 300.0;
    let h0 = m.organic_held();
    let (mut t, mut g) = (0.0, 0.0);
    for _ in 0..600 {
        let (take, ret) = m.step(&s);
        t += took(&take);
        g += ret.litter + ret.carrion;
    }
    assert!(m.ledger[Founder::Lanternjaw.index()].kills > 0.0 && m.carcass_left > 0.0, "fixture: it killed and left a carcass");
    assert!(m.ledger[Founder::Lanternjaw.index()].swallowed[Food::Carrion.index()] > 0.0, "fixture: it fed at its kill");
    let lhs = m.organic_held() - h0;
    let rhs = t - g - respired(&m) - m.carcass_decayed;
    assert!((lhs - rhs).abs() <= 1e-9 * h0, "held moved {lhs}, the flows say {rhs}");
}

/// A frondgrazer cohort mid-gestation (an open escrow), the sim's `LIVE_BIRTH` rule.
fn gestating(cfg: &FaunaConfig, n: f64, body: f64, reserve: f64) -> (Cohort, f64) {
    let sc = cfg.founder(Founder::Browser).core;
    let esc = 0.5 * sc.birth_cost;
    let mut c = Cohort::new(n, body, reserve);
    c.escrow = Some((esc, 0.5 * sc.reproduction.gestation_s));
    c.matured = true;
    (c, esc)
}

/// **A killed parent's escrow** (`kill`, `step.rs:2203`: `fail_gestation` first, then the
/// corpse; `fail_gestation`, `step.rs:3107`: `gestation_loss_fraction` of the escrow is
/// respired and the rest goes back into the reserve, so into the corpse). The model's
/// `hunt` lays `dead · b.organic()` (structure, reserve, stomach) and drops the escrow of
/// the killed bodies from every ledger line: matter vanishes.
#[test]
fn a_killed_parents_escrow_goes_as_the_sim_sends_it() {
    let cfg = FaunaConfig::default();
    let mut fits = Fits::unfitted();
    fits.kill_reach = 0.5;
    fits.of_mut(Founder::Browser).kill_rate = 2e-3;
    let mut m = model(&cfg, fits, 1.0, &[Founder::Lanternjaw, Founder::Browser]);
    let lj = cfg.founder(Founder::Lanternjaw);
    let mut h = Cohort::new(2.0, lj.core.body_max, 0.05 * lj.core.reserve_of(lj.core.body_max));
    h.b.stomach[Food::Carrion.index()] = lj.gut.capacity(lj.core.body_max);
    h.b.eating = true;
    m.add(Founder::Lanternjaw, h);
    let bs = cfg.founder(Founder::Browser).core;
    let (c, esc) = gestating(&cfg, 5.0, bs.birth_body, 0.8 * bs.reserve_of(bs.birth_body));
    m.add(Founder::Browser, c);
    // (webmodel-loop: the corpse is laid in the act, before this step's births pay the next
    // instalment out of the survivors' reserve, as in the sim's tick, `step.rs:162-281`; so the
    // body at the kill is the body after upkeep and digestion, before the instalment.)
    let h0 = m.organic_held();
    let n0 = 5.0;
    let (take, ret) = m.step(&Supply::default());
    let p = &m.pops[Founder::Browser.index()][0];
    let killed = n0 - p.n;
    assert!(killed > 0.0, "fixture: it killed a gestating body");
    let lam = bs.reproduction.gestation_loss_fraction;
    let instalment = p.escrow.map_or(0.0, |e| e.0) - esc;
    let at_kill = p.b.organic() + instalment;
    let corpse = m.carcass_laid + ret.carrion;
    assert!(close(corpse, killed * (at_kill + (1.0 - lam) * esc), 1e-9), "corpse {corpse} vs the sim's {}", killed * (at_kill + (1.0 - lam) * esc));
    let lhs = m.organic_held() - h0;
    let rhs = took(&take) - ret.litter - ret.carrion - respired(&m) - m.carcass_decayed;
    assert!((lhs - rhs).abs() <= 1e-9 * h0, "held moved {lhs}, the flows say {rhs}");
}

/// **An aged-out parent's escrow** (`deaths`, `step.rs:3800`: "A body that dies carrying an
/// escrow loses the gestation first" — `fail_gestation`, then `bury_corpse`). The model's
/// `deaths` returns `died · b.organic()` for the bodies the lifespan draw takes and leaves
/// their escrow nowhere. (A starving parent is not caught: its unpaid instalment already
/// returns the escrow in `reproduce`, as the sim's `gestate` does.)
#[test]
fn an_aged_parents_escrow_goes_as_the_sim_sends_it() {
    let mut cfg = FaunaConfig::default();
    cfg.founders[Founder::Browser.index()].core.lifespan_median_s = 300.0;
    let mut fits = Fits::unfitted();
    fits.of_mut(Founder::Browser).motor_effort = 0.0;
    let mut m = model(&cfg, fits, 1.0, &[Founder::Browser]);
    let bs = cfg.founder(Founder::Browser).core;
    let (mut c, esc) = gestating(&cfg, 3.0, bs.body_max, 0.9 * bs.reserve_of(bs.body_max));
    // The middle of the lifespan draw (`draw_lifespan_ticks`): the hazard is on.
    c.age_s = (cfg.life.lifespan_draw_min + 0.5 * cfg.life.lifespan_draw_span) * 300.0;
    m.add(Founder::Browser, c);
    let h0 = m.organic_held();
    let (mut t, mut g) = (0.0, 0.0);
    for _ in 0..10 {
        let (take, ret) = m.step(&Supply::default());
        t += took(&take);
        g += ret.litter + ret.carrion;
    }
    assert!(m.ledger[Founder::Browser.index()].aged > 0.0, "fixture: some aged out mid-gestation");
    let _ = esc;
    let lhs = m.organic_held() - h0;
    let rhs = t - g - respired(&m) - m.carcass_decayed;
    assert!((lhs - rhs).abs() <= 1e-9 * h0, "held moved {lhs}, the flows say {rhs}");
}

// ===================================================================================
// 3. Dormant eggs: lost at laying, dying in the bank
// ===================================================================================

/// **Dormant egg loss and death** (`clutch_fate`, `step.rs:3388`, takes every clutch whose
/// face fails, dormant ones too, *to carrion*; `dormant_eggs`, `step.rs:3537`, sends a bank
/// egg past its viability or under `body_min` *to litter*; `egg_upkeep`, `step.rs:933`,
/// respires `egg_upkeep_per_s` of it). A full adult lanternjaw lays one dormant clutch: what
/// its reserve paid is the lost eggs' carrion plus the bank; once every bank egg is dead,
/// what was paid is the lost eggs' carrion plus the dead eggs' litter plus the upkeep.
#[test]
fn dormant_eggs_are_lost_to_carrion_and_die_to_litter() {
    let mut cfg = FaunaConfig::default();
    let lj = Founder::Lanternjaw;
    cfg.founders[lj.index()].core.reproduction.surplus_hold_s = 20.0;
    cfg.founders[lj.index()].core.lifespan_median_s = 0.0;
    cfg.predation.egg_viability_median_s = 900.0;
    cfg.predation.egg_viability_spread = 2.0;
    cfg.predation.egg_upkeep_per_s = 2e-5;
    let mut fits = Fits::unfitted();
    fits.of_mut(lj).egg_loss = 0.25;
    let mut m = model(&cfg, fits, 1.0, &[lj]);
    let sc = cfg.founder(lj).core;
    let rule = sc.reproduction;
    assert!(rule.dormant, "fixture: the lanternjaw lays dormant eggs");
    let mut c = Cohort::new(1.0, sc.body_max, sc.reserve_of(sc.body_max));
    c.matured = true;
    m.add(lj, c);
    let mut carrion = 0.0;
    let mut litter = 0.0;
    let mut laid_at = None;
    for t in 0..((rule.surplus_hold_s as usize) + 10) {
        let (_, r) = m.step(&Supply::default());
        carrion += r.carrion;
        litter += r.litter;
        if laid_at.is_none() && m.ledger[lj.index()].clutches > 0.0 {
            laid_at = Some(t);
        }
    }
    let l = &m.ledger[lj.index()];
    assert_eq!(l.clutches, 1.0, "fixture: one clutch");
    let eggs = f64::from(rule.clutch_for(sc.reserve_of(sc.body_max)));
    assert!(l.laid > 0.0 && l.laid <= eggs, "laid {} of the {eggs} the full reserve pays for", l.laid);
    let paid = l.eggs_paid;
    assert!(close(paid, l.laid * rule.egg_organic, 1e-12), "a clutch from surplus costs eggs × egg_organic");
    assert!(close(l.eggs_lost, 0.25 * l.laid, 1e-12), "lost {} of {}", l.eggs_lost, l.laid);
    assert!(close(carrion, l.eggs_lost * rule.egg_organic, 1e-9), "lost eggs to carrion: {carrion}");
    let bank: f64 = m.eggs.iter().filter(|e| e.dormant).map(|e| e.n * e.organic).sum();
    assert!(close(paid, carrion + bank + l.bank_upkeep + litter, 1e-9), "paid {paid} vs carrion {carrion} + bank {bank} + upkeep {}", l.bank_upkeep);
    // Past every egg's viability (median × spread): the bank is gone, to litter.
    for _ in 0..(2.0 * 900.0 * 2.0) as usize {
        let (_, r) = m.step(&Supply::default());
        carrion += r.carrion;
        litter += r.litter;
    }
    let l = &m.ledger[lj.index()];
    assert_eq!(l.clutches, 1.0, "fixture: no second clutch (refractory)");
    assert!(m.eggs.iter().all(|e| !e.dormant || e.n < 1e-9), "the bank is empty past viability");
    assert!(l.bank_died > 0.0 && litter > 0.0, "bank eggs died to litter");
    assert!(close(paid, carrion + litter + l.bank_upkeep, 1e-9), "paid {paid} vs carrion {carrion} + litter {litter} + upkeep {}", l.bank_upkeep);
    let _ = laid_at;
}

/// **A bank egg's viability is the sim's draw** (`Dormancy::draw`, `predation.rs:261`:
/// log-uniform `median · spread^u`, `u ~ U[−1, 1]`). The model's bank survival at several
/// ages matches the share of 4,000 eggs drawn by the sim's own function still viable then.
#[test]
fn bank_viability_is_the_sims_draw() {
    let mut cfg = FaunaConfig::default();
    cfg.predation.egg_upkeep_per_s = 0.0;
    let pc = cfg.predation;
    let n = 4000u64;
    let spans: Vec<f64> = (0..n).map(|k| Dormancy::draw(&pc, 0x5EED, k, 0).viable_until_tick as f64 / f64::from(TICK_HZ)).collect();
    let lj = Founder::Lanternjaw;
    let sc = cfg.founder(lj).core;
    let mut m = model(&cfg, Fits::unfitted(), 10.0, &[lj]);
    m.eggs.push(EggCohort { lineage: lj, n: 1.0, organic: sc.reproduction.egg_organic, age_s: 0.0, dormant: true, ..Default::default() });
    let lo = pc.egg_viability_median_s / pc.egg_viability_spread;
    let checks = [0.5 * lo, 1.2 * lo, 0.8 * pc.egg_viability_median_s, 1.3 * pc.egg_viability_median_s, 0.9 * pc.egg_viability_median_s * pc.egg_viability_spread];
    let mut t = 0.0;
    for &at in &checks {
        while t + 1e-9 < at {
            m.step(&Supply::default());
            t += 10.0;
        }
        let model_s: f64 = m.eggs.iter().filter(|e| e.dormant).map(|e| e.n).sum();
        let sim = spans.iter().filter(|s| **s > t).count() as f64 / n as f64;
        assert!((model_s - sim).abs() < 0.03, "at {t} s: model {model_s} vs the sim's draw {sim}");
    }
}

// ===================================================================================
// 4. The gut: ceiling and mass exponent against the sim's digestion
// ===================================================================================

/// **The gut law against the sim stepping a hand-built body.** A founder arrives with a
/// stomach at `s0 · K` of its first food at its own tissue's mineral (`lib.rs:3948`); with
/// `s0` raised to 0.9 in the config, no food on the plain and the mouth still, one sim tick
/// digests `S · DT / τ'` with `τ' = τ / (B/B_max)^(e−1)` (`digest`, `step.rs:822`). The
/// model's `laws::digest` on the same body digests the same; its compound over a second
/// matches twenty sim ticks; and `laws::gut_ceiling_per_s` is the sim's per-tick rate scaled
/// to a full stomach. Across structure from near `body_min` to `body_max` and gut mass
/// exponents 1, 0.75, 0.5 — so the exponent's effect on a small body is the sim's.
#[test]
fn the_gut_ceiling_and_mass_exponent_are_the_sims_digestion() {
    for e in [1.0, 0.75, 0.5] {
        for f in [Founder::Blind, Founder::Capgnawer, Founder::Lanternjaw, Founder::Browser] {
            let mut cfg = patch();
            cfg.founders[f.index()].gut.s0 = 0.9;
            cfg.founders[f.index()].gut.mass_exponent = e;
            cfg.founders[f.index()].core.lifespan_median_s = 0.0;
            let ph = *cfg.founder(f);
            let food = ph.diet.foods().next().unwrap();
            let mut per_body = Vec::new();
            for frac in [1.15 * ph.core.body_min / ph.core.body_max, 0.5, 1.0] {
                let (world, mut flora, mut fauna, id) = one_body(cfg.clone(), f, StartingStores { body: frac, reserve: 0.1 }, 0.0, false);
                let a0 = *fauna.view().animal(id).unwrap();
                let s0 = a0.stomach.pools[food.index()].organic;
                assert!(s0 > 0.0, "fixture: the arrival stomach holds {food:?}");
                let d0 = fauna.view().ledger.digested_by_founder[f.index()];
                fauna.step_with(&world, &mut flora, 1);
                let sim_tick = fauna.view().ledger.digested_by_founder[f.index()] - d0;
                let mut b = Body { body: a0.body, reserve: a0.reserve, ..Body::default() };
                b.stomach[food.index()] = s0;
                let model_tick = laws::digest(&ph, &mut b, DT).digested;
                assert!(close(model_tick, sim_tick, 1e-9), "{} e {e} B {:.4}: one tick model {model_tick} sim {sim_tick}", f.name(), a0.body);
                // The ceiling: the sim's tick rate at a full stomach, times the yield.
                let k = ph.gut.capacity(a0.body);
                let sim_ceiling = ph.yield_for(food) * sim_tick / DT * k / s0;
                let ceiling = laws::gut_ceiling_per_s(&ph, a0.body, food);
                assert!(close(ceiling, sim_ceiling, 1e-9), "{} e {e} B {:.4}: ceiling model {ceiling} sim {sim_ceiling}", f.name(), a0.body);
                per_body.push((a0.body, sim_ceiling / a0.body));
                // Twenty more sim ticks against the model's one-second compound.
                let a1 = *fauna.view().animal(id).unwrap();
                let s1 = a1.stomach.pools[food.index()].organic;
                let d1 = fauna.view().ledger.digested_by_founder[f.index()];
                for _ in 0..TICK_HZ {
                    fauna.step_with(&world, &mut flora, 1);
                }
                let sim_s = fauna.view().ledger.digested_by_founder[f.index()] - d1;
                let mut b = Body { body: a1.body, reserve: a1.reserve, ..Body::default() };
                b.stomach[food.index()] = s1;
                let model_s = laws::digest(&ph, &mut b, 1.0).digested;
                assert!(close(model_s, sim_s, 1e-4), "{} e {e}: one second model {model_s} sim {sim_s}", f.name());
            }
            // The mass exponent: the per-unit-body ceiling of the small body over the adult's
            // is (B/B_max)^(e − 1), in the sim's own digestion.
            let (bs, cs) = per_body[0];
            let (bb, cb) = per_body[2];
            assert!(close(cs / cb, (bs / bb).powf(e - 1.0), 1e-6), "{} e {e}: sim ratio {} vs law {}", f.name(), cs / cb, (bs / bb).powf(e - 1.0));
        }
    }
}

// ===================================================================================
// 5. The κ brood earmark and the full-clutch rule, against the sim's reproduction
// ===================================================================================

/// The first clutch of one patch shredder on the plain, in the sim and in the model at the
/// sim's tick: (the tick it laid, eggs, brood and reserve at sampled ticks).
struct Laying {
    tick: Option<u64>,
    eggs: u64,
    brood: Vec<f64>,
    reserve: Vec<f64>,
}

const SAMPLES: [u64; 4] = [200, 1200, 4000, 9000];

fn sim_laying(cfg: &FaunaConfig, stores: StartingStores, feed: f64, ticks: u64) -> Laying {
    let f = Founder::Blind;
    let (world, mut flora, mut fauna, id) = one_body(cfg.clone(), f, stores, feed, true);
    let mut out = Laying { tick: None, eggs: 0, brood: Vec::new(), reserve: Vec::new() };
    for t in 1..=ticks {
        fauna.step_with(&world, &mut flora, 1);
        let a = fauna.view().animal(id).copied();
        if SAMPLES.contains(&t) {
            let a = a.expect("alive");
            out.brood.push(a.reproduction.brood);
            out.reserve.push(a.reserve);
        }
        if out.tick.is_none() && fauna.view().ledger.clutches_by_founder[f.index()] > 0 {
            out.tick = Some(t);
            out.eggs = fauna.view().ledger.eggs_laid_by_founder[f.index()];
        }
    }
    out
}

fn model_laying(cfg: &FaunaConfig, start: Body, feed: f64, ticks: u64) -> Laying {
    let f = Founder::Blind;
    let mut fits = Fits::unfitted();
    fits.of_mut(f).motor_effort = 0.0;
    fits.of_mut(f).response[Food::Litter.index()] = Some(Response::holling(feed, 0.0));
    let mut m = model(cfg, fits, DT, &[f]);
    let mut c = Cohort::new(1.0, start.body, start.reserve);
    c.b = start;
    c.matured = start.body >= cfg.founder(f).core.birth_body;
    m.add(f, c);
    let mut s = Supply::default();
    s.litter = 1e9;
    let mut out = Laying { tick: None, eggs: 0, brood: Vec::new(), reserve: Vec::new() };
    for t in 1..=ticks {
        m.step(&s);
        if SAMPLES.contains(&t) {
            let c = m.pops[f.index()].iter().find(|c| c.native == false).expect("the parent");
            out.brood.push(c.b.brood);
            out.reserve.push(c.b.reserve);
        }
        if out.tick.is_none() && m.ledger[f.index()].clutches > 0.0 {
            out.tick = Some(t);
            out.eggs = m.ledger[f.index()].laid.round() as u64;
        }
    }
    out
}

/// The patch shredder with a short surplus hold (a knob: the rule is unchanged) and no
/// lifespan draw. The start's reserve pays for one egg fewer than the full clutch, `short`
/// of the full clutch's price (`None`: the middle of that band).
fn laying_config(wait_s: f64, short: Option<f64>) -> (FaunaConfig, StartingStores) {
    let mut cfg = patch();
    let f = Founder::Blind;
    let r = &mut cfg.founders[f.index()].core.reproduction;
    r.surplus_hold_s = 30.0;
    r.clutch_hold_max_s = wait_s;
    cfg.founders[f.index()].core.lifespan_median_s = 0.0;
    let sc = cfg.founder(f).core;
    let rule = sc.reproduction;
    let cap = sc.reserve_of(sc.body_max);
    let full = rule.full_clutch(cap);
    assert!(full >= 2 && rule.brood_share > 0.0 && rule.clutch_max > 0, "fixture: a clutch from surplus with a brood");
    let price = rule.surplus_floor + f64::from(full) * rule.egg_organic;
    let reserve = price - short.unwrap_or(0.5 * rule.egg_organic);
    assert_eq!(rule.clutch_for(reserve), full - 1);
    (cfg, StartingStores { body: 1.0, reserve: reserve / cap })
}

/// The model's start: the sim's body as it arrived (stomach at `s0 · K`, latch off).
fn arrived(cfg: &FaunaConfig, stores: StartingStores) -> Body {
    let (_, _, fauna, id) = one_body(cfg.clone(), Founder::Blind, stores, 1.0, true);
    let a = *fauna.view().animal(id).unwrap();
    let mut b = Body { body: a.body, reserve: a.reserve, brood: a.reproduction.brood, ..Body::default() };
    for food in Food::ALL {
        b.stomach[food.index()] = a.stomach.pools[food.index()].organic;
    }
    b
}

/// **The κ brood earmark and the full-clutch wait, fed** (`assimilate`, `step.rs:2696`: a
/// mature body earmarks `brood_share` of what it places, up to `brood_target`; `respire`,
/// `step.rs:371`: the brood pays `brood_share` of what the reserve respires; `lay`,
/// `step.rs:3145`: a body short of the full clutch its own cap pays for keeps feeding until
/// its reserve pays for it or `clutch_hold_max_s` runs out; the clutch is paid from the
/// brood first). An adult on a litter carpet starting one egg short: the model's brood and
/// reserve follow the sim's tick by tick, and it lays the full clutch when the sim does.
#[test]
fn the_brood_and_the_full_clutch_wait_follow_the_sim_fed() {
    // A few thousandths short: the fed body's reserve reaches the full clutch's price well
    // inside the wait.
    let (cfg, stores) = laying_config(1800.0, Some(0.003));
    let ticks = 20_000;
    let sim = sim_laying(&cfg, stores, 1.0, ticks);
    let m = model_laying(&cfg, arrived(&cfg, stores), 1.0, ticks);
    let rule = cfg.founder(Founder::Blind).core.reproduction;
    let full = u64::from(rule.full_clutch(cfg.founder(Founder::Blind).core.reserve_of(cfg.founder(Founder::Blind).core.body_max)));
    let st = sim.tick.expect("the sim laid within the run");
    assert_eq!(sim.eggs, full, "fixture: the sim waited for and laid the full clutch");
    assert!(st < ((rule.surplus_hold_s + rule.clutch_hold_max_s) * f64::from(TICK_HZ)) as u64, "fixture: before the wait ran out");
    let mt = m.tick.expect("the model laid within the run");
    assert_eq!(m.eggs, full, "the model's clutch");
    assert!((mt as f64 - st as f64).abs() <= 0.02 * st as f64 + 40.0, "laid at tick {mt}, the sim at {st}");
    for (k, t) in SAMPLES.iter().enumerate().filter(|(_, t)| **t < st.min(mt)) {
        assert!(close(m.brood[k], sim.brood[k], 0.02), "tick {t}: brood model {} sim {}", m.brood[k], sim.brood[k]);
        assert!(close(m.reserve[k], sim.reserve[k], 0.02), "tick {t}: reserve model {} sim {}", m.reserve[k], sim.reserve[k]);
    }
    assert!(sim.brood.iter().any(|b| *b > 0.0), "fixture: the sim earmarked a brood");
}

/// **The full-clutch wait runs out, unfed**: the same adult with its mouth still (so its
/// reserve only falls) waits `clutch_hold_max_s` and then lays what its reserve pays for, one
/// short of the full clutch, at the sim's tick; the brood stays empty (nothing placed).
#[test]
fn the_full_clutch_wait_runs_out_as_the_sims() {
    let (cfg, stores) = laying_config(120.0, None);
    let ticks = 6_000;
    let sim = sim_laying(&cfg, stores, 0.0, ticks);
    let m = model_laying(&cfg, arrived(&cfg, stores), 0.0, ticks);
    let rule = cfg.founder(Founder::Blind).core.reproduction;
    let full = u64::from(rule.full_clutch(cfg.founder(Founder::Blind).core.reserve_of(cfg.founder(Founder::Blind).core.body_max)));
    let st = sim.tick.expect("the sim laid");
    let mt = m.tick.expect("the model laid");
    assert_eq!(sim.eggs, full - 1, "fixture: the sim laid short at the wait's end");
    assert_eq!(m.eggs, sim.eggs, "eggs");
    assert!((mt as f64 - st as f64).abs() <= 2.0, "laid at tick {mt}, the sim at {st}");
}

// ===================================================================================
// 6. The stochastic twin: whole bodies, and the mean field's expectation
// ===================================================================================

/// Twelve shredders whose lifespan draw falls in the run, fed, nothing else.
fn aging(draws: Draws) -> FaunaModel {
    let mut cfg = FaunaConfig::default();
    let f = Founder::Blind;
    cfg.founders[f.index()].core.lifespan_median_s = 600.0;
    let mut fits = Fits::unfitted();
    fits.of_mut(f).motor_effort = 0.0;
    let mut m = model(&cfg, fits, 5.0, &[f]);
    m.draws = draws;
    let sc = cfg.founder(f).core;
    let mut c = Cohort::new(12.0, sc.body_max, sc.reserve_of(sc.body_max));
    c.matured = true;
    m.add(f, c);
    m
}

/// **The twin keeps whole bodies, and its mean is the mean field.** Over many draws, the
/// twin's survivors of a lifespan-limited cohort average to the mean field's (and to
/// `12 · S(t)` of the sim's own draw, `lifespan_survival`), within four standard errors of
/// the twin's mean; every cohort of every draw holds a whole number of bodies at every step.
#[test]
fn the_twin_keeps_whole_bodies_and_has_the_mean_fields_expectation() {
    let mut s = Supply::default();
    s.litter = 1e6;
    let steps = 120; // 600 s
    let mut mf = aging(Draws::Mean);
    for _ in 0..steps {
        mf.step(&s);
    }
    let mean_field = mf.census(Founder::Blind).n;
    let runs = 400;
    let mut xs = Vec::with_capacity(runs);
    for r in 0..runs {
        let mut m = aging(Draws::Twin(Rng::new(1000 + r as u64)));
        for _ in 0..steps {
            m.step(&s);
            for c in &m.pops[Founder::Blind.index()] {
                assert!((c.n - c.n.round()).abs() < 1e-9 && c.n >= 0.0, "a twin cohort of {} bodies", c.n);
            }
        }
        xs.push(m.census(Founder::Blind).n);
    }
    let mean = xs.iter().sum::<f64>() / runs as f64;
    let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (runs as f64 - 1.0);
    let se = (var / runs as f64).sqrt();
    assert!(mean_field > 0.5 && mean_field < 11.5, "fixture: deaths happen but not all ({mean_field})");
    assert!((mean - mean_field).abs() <= 4.0 * se, "twin mean {mean} ± {se} vs mean field {mean_field}");
    let life = FaunaConfig::default().life;
    let law = 12.0 * laws::lifespan_survival(&life, 600.0, 600.0, false);
    assert!((mean_field - law).abs() < 1e-9 * 12.0, "mean field {mean_field} vs 12·S(t) {law}");
}

// ===================================================================================
// 7. Config flow: a knob moves the model's rate in the sim law's direction and form
// ===================================================================================

/// One body of `f` with a reserve that pays its upkeep, no food, run `secs` at `dt`.
fn starving(cfg: &FaunaConfig, f: Founder, secs: usize) -> FaunaModel {
    let mut fits = Fits::unfitted();
    fits.of_mut(f).motor_effort = 0.0;
    let mut m = model(cfg, fits, 1.0, &[f]);
    let sc = cfg.founder(f).core;
    let mut c = Cohort::new(1.0, sc.body_max, 0.8 * sc.reserve_of(sc.body_max));
    c.matured = true;
    m.add(f, c);
    for _ in 0..secs {
        m.step(&Supply::default());
    }
    m
}

/// **`maintenance_per_s`** (`maintenance`, `step.rs:878`: `m · rest · B · DT`): doubling it
/// doubles the model's upkeep from a reserve-paid body; **`sleep_upkeep`** scales an
/// asleep body's upkeep by itself.
#[test]
fn config_flow_maintenance_is_linear() {
    let base = patch();
    let f = Founder::Blind;
    let u = |cfg: &FaunaConfig| starving(cfg, f, 60).ledger[f.index()].flows.upkeep;
    let mut twice = base.clone();
    twice.founders[f.index()].core.maintenance_per_s *= 2.0;
    assert!(close(u(&twice), 2.0 * u(&base), 1e-9), "upkeep {} vs 2 × {}", u(&twice), u(&base));
    let ph = base.founder(f);
    let b = ph.core.body_max;
    assert!(close(laws::upkeep_per_s(ph, b, 0.0), ph.sleep_upkeep * ph.core.maintenance_per_s * b, 1e-12));
}

/// **`digestion_s`** (`digest`, `step.rs:843`: each tick `S · DT / τ`): the model's digestion
/// over one second from a hand-built stomach is the twenty-tick compound
/// `S (1 − (1 − DT/τ)^20)`, computed here from the config, for `τ` and `2τ`.
#[test]
fn config_flow_digestion_time_compounds_per_tick() {
    let base = patch();
    let f = Founder::Blind;
    for k in [1.0, 2.0] {
        let mut cfg = base.clone();
        cfg.founders[f.index()].gut.digestion_s[Food::Litter.index()] *= k;
        let ph = *cfg.founder(f);
        let b0 = 0.5 * ph.core.body_max;
        let s0 = 0.5 * ph.gut.capacity(b0);
        let mut b = Body { body: b0, reserve: 0.0, ..Body::default() };
        b.stomach[Food::Litter.index()] = s0;
        let got = laws::digest(&ph, &mut b, 1.0).digested;
        let tau = ph.gut.digestion_s[Food::Litter.index()] / (b0 / ph.core.body_max).powf(ph.gut.mass_exponent - 1.0);
        let mut s = s0;
        for _ in 0..TICK_HZ {
            s -= s * DT / tau;
        }
        assert!(close(got, s0 - s, 1e-12), "τ × {k}: digested {got} vs the ticks' {}", s0 - s);
    }
}

/// **`growth_max_per_s`** (`grow`, `step.rs:959`: `g · DT · fullness` out of the free
/// reserve): doubling it doubles the structure a juvenile builds in a step when neither the
/// room nor the free reserve binds (the fullness is the same at the step's start).
#[test]
fn config_flow_growth_is_linear_in_its_rate() {
    let base = patch();
    let f = Founder::Blind;
    let g = |cfg: &FaunaConfig| {
        let mut fits = Fits::unfitted();
        fits.of_mut(f).motor_effort = 0.0;
        let mut m = model(cfg, fits, 1.0, &[f]);
        let sc = cfg.founder(f).core;
        let body = 1.2 * sc.body_min;
        m.add(f, Cohort::new(1.0, body, 0.7 * sc.reserve_of(body)));
        m.step(&Supply::default());
        m.ledger[f.index()].flows.growth
    };
    let mut twice = base.clone();
    twice.founders[f.index()].core.growth_max_per_s *= 2.0;
    let (a, b) = (g(&base), g(&twice));
    assert!(a > 0.0);
    assert!(close(b, 2.0 * a, 1e-12), "growth {b} vs 2 × {a}");
}

/// **`lifespan_median_s`** (`draw_lifespan_ticks`, `lib.rs:2210`: the span is a uniform share
/// of the median): survival is a function of age over the median, so doubling the median
/// leaves at twice the age the share the base left at the age.
#[test]
fn config_flow_lifespan_scales_survival_in_time() {
    let base = FaunaConfig::default();
    let f = Founder::Blind;
    let alive = |cfg: &FaunaConfig, secs: usize| {
        let mut fits = Fits::unfitted();
        fits.of_mut(f).motor_effort = 0.0;
        let mut m = model(cfg, fits, 5.0, &[f]);
        let sc = cfg.founder(f).core;
        m.add(f, Cohort::new(10.0, sc.body_max, sc.reserve_of(sc.body_max)));
        let mut s = Supply::default();
        s.litter = 1e6;
        for _ in 0..secs / 5 {
            m.step(&s);
        }
        m.census(f).n
    };
    let mut short = base.clone();
    short.founders[f.index()].core.lifespan_median_s = 400.0;
    let mut long = base.clone();
    long.founders[f.index()].core.lifespan_median_s = 800.0;
    let (a, b) = (alive(&short, 400), alive(&long, 800));
    assert!(a < 10.0 && a > 0.0, "fixture: some aged out ({a})");
    assert!(close(a, b, 1e-6), "survivors {a} at the median vs {b} at twice the age under twice the median");
}

/// **`incubation_s`** (`hatch`, `step.rs:3435`: a clutch hatches at `incubation_s`): the
/// model's eggs hatch after `incubation_s`, and twice that when it is doubled.
#[test]
fn config_flow_incubation_sets_the_hatch() {
    let base = patch();
    let f = Founder::Blind;
    let hatch_at = |cfg: &FaunaConfig| {
        let mut m = model(cfg, Fits::unfitted(), 1.0, &[f]);
        m.eggs.push(EggCohort { lineage: f, n: 2.0, organic: cfg.founder(f).core.reproduction.egg_organic, age_s: 0.0, dormant: false, ..Default::default() });
        for t in 1..10_000 {
            m.step(&Supply::default());
            if m.ledger[f.index()].hatched > 0.0 {
                return t as f64;
            }
        }
        f64::INFINITY
    };
    let mut twice = base.clone();
    twice.founders[f.index()].core.reproduction.incubation_s *= 2.0;
    let inc = base.founder(f).core.reproduction.incubation_s;
    let (a, b) = (hatch_at(&base), hatch_at(&twice));
    assert!((a - inc).abs() <= 1.0, "hatched at {a} s, incubation {inc}");
    assert!((b - 2.0 * inc).abs() <= 1.0, "hatched at {b} s, incubation {}", 2.0 * inc);
}

/// **`parasite_take_per_day`** (`drain_riders`, `step.rs:456`: each rider takes
/// `take / ecological_day_s × host body` per second): doubling it doubles what a rider
/// drains from a host over a minute.
#[test]
fn config_flow_parasite_take_is_linear() {
    let base = FaunaConfig::default();
    let drained = |cfg: &FaunaConfig| {
        let mut fits = Fits::unfitted();
        fits.of_mut(Founder::Browser).motor_effort = 0.0;
        let mut m = model(cfg, fits, 1.0, &[Founder::Lanternjaw, Founder::Browser]);
        let bs = cfg.founder(Founder::Browser).core;
        m.add(Founder::Browser, Cohort::new(1.0, bs.body_max, 0.8 * bs.reserve_of(bs.body_max)));
        let lj = cfg.founder(Founder::Lanternjaw).core;
        let mut r = Cohort::new(1.0, lj.body_min, 0.0);
        r.host = Some(Founder::Browser);
        m.add(Founder::Lanternjaw, r);
        for _ in 0..60 {
            m.step(&Supply::default());
        }
        m.ledger[Founder::Browser.index()].rider_loss
    };
    let mut twice = base.clone();
    twice.parasite_take_per_day *= 2.0;
    let (a, b) = (drained(&base), drained(&twice));
    assert!(a > 0.0);
    assert!(close(b, 2.0 * a, 1e-3), "drained {b} vs 2 × {a}");
}

