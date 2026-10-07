//! The animal half's tests (`design/webmodel/README.md`, "Tests"): **law anchors** (the
//! model's rate on a hand-built state equals the sim's, the sim side a one-body fixture stepped
//! by the sim itself), **conservation** (the model's ledger closes), and **config flow**
//! (moving a knob moves the model's rate the way the sim's law says). No tuned number is
//! pinned: every expectation is computed from the config on hand.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command, Controller, Fauna, FaunaConfig, Food, Founder, Response as CtlResponse, StartingStores, TICK_HZ,
};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site, Species};
use cubarium_voxel_sim::params::Params;

use super::fits::{Fits, Response};
use super::laws::{self, Body};
use super::model::{Cohort, FaunaModel};
use super::rng::{Draws, Rng};
use crate::exchange::Supply;

// --- The sim side: a one-body fixture on a litter plain (the young_budget probe's plain) ---

struct StandAndFeed;
impl Controller for StandAndFeed {
    fn drive(&mut self, _o: &[f64]) -> CtlResponse {
        CtlResponse::Bounded(Actions { forward: 0.0, turn: 0.0, feed: 1.0 })
    }
    fn reset(&mut self) {}
}

const SURFACE_Y: u32 = 2;
const W: u32 = 16;
const D: u32 = 8;

fn plain() -> World {
    let mut w = World::empty(VoxelConfig {
        width: W,
        height: 6,
        depth: D,
        voxel_m: 0.125,
        seed: 7,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    for z in 0..D {
        for x in 0..i64::from(W) {
            for y in 1..=SURFACE_Y {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

/// Litter on every face, deep enough that the bite's own `E / (E + K)` is ~1 and the
/// mineral funds every yield.
fn carpet(flora: &mut Flora) {
    for z in 0..D {
        for x in 0..W {
            let site = Site { x, y: SURFACE_Y, z };
            flora.deposit(site, Deposit { kind: DepositKind::Litter, organic: 50.0, mineral: 5.0, energy: 100.0 });
        }
    }
}

/// What a body went through in the sim over `ticks` from introduction at `stores`.
struct SimRun {
    /// The body as the sim had it after its first tick (an introduced founder arrives with a
    /// stomach at the appetite floor): (body, reserve, stomach by food class).
    start: (f64, f64, [f64; Food::COUNT]),
    end_body: f64,
    end_reserve: f64,
    end_stomach: f64,
    assimilated: f64,
    upkeep: f64,
    growth: f64,
}

fn sim_run(config: &FaunaConfig, f: Founder, stores: StartingStores, ticks: u64) -> SimRun {
    let world = plain();
    let mut flora = Flora::new(FloraConfig::default());
    carpet(&mut flora);
    let mut cfg = config.clone();
    cfg.founders[f.index()].core.lifespan_median_s = 0.0;
    cfg.founders[f.index()].core.reproduction.surplus_hold_s = 1e9;
    let mut fauna = Fauna::new(cfg);
    assert!(fauna.apply(&world, Command::IntroduceFounder { x: 8, z: 4, founder: f, stores, heading_rad: 0.0 }));
    let id = fauna.view().animals.last().unwrap().id;
    fauna.set_controller(id, Box::new(StandAndFeed));
    fauna.step_with(&world, &mut flora, 1);
    let a0 = *fauna.view().animal(id).unwrap();
    let l0 = fauna.view().ledger.clone();
    for _ in 0..ticks {
        fauna.step_with(&world, &mut flora, 1);
    }
    let a = *fauna.view().animal(id).unwrap();
    let l = &fauna.view().ledger;
    let i = f.index();
    SimRun {
        start: (a0.body, a0.reserve, Food::ALL.map(|g| a0.stomach.pools[g.index()].organic)),
        end_body: a.body,
        end_reserve: a.reserve,
        end_stomach: a.stomach.organic(),
        assimilated: l.assimilated_by_founder[i] - l0.assimilated_by_founder[i],
        upkeep: l.upkeep_by_founder[i] - l0.upkeep_by_founder[i],
        growth: l.growth_by_founder[i] - l0.growth_by_founder[i],
    }
}

/// The model's one body from the same start, food under its mouth (ψ = 1), clock off, at the
/// sim's own tick (`dt = DT`).
fn model_run(config: &FaunaConfig, f: Founder, start: (f64, f64, [f64; Food::COUNT]), ticks: u64) -> (FaunaModel, Body) {
    let mut p = Params::defaults();
    p.fauna = config.clone();
    p.fauna.founders[f.index()].core.lifespan_median_s = 0.0;
    p.fauna.founders[f.index()].core.reproduction.surplus_hold_s = 1e9;
    p.world.day.mode = DayMode::Off;
    let mut fits = Fits::unfitted();
    fits.of_mut(f).motor_effort = 0.0;
    let mut m = FaunaModel::new(&p, fits, 1.0 / f64::from(TICK_HZ));
    for g in Founder::ALL {
        m.enabled[g.index()] = g == f;
    }
    let mut c = Cohort::new(1.0, start.0, start.1);
    c.b.stomach = start.2;
    c.b.eating = true;
    m.add(f, c);
    let mut s = Supply::default();
    s.litter = 1e9;
    for _ in 0..ticks {
        m.step(&s);
    }
    let b = m.pops[f.index()][0].b;
    (m, b)
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-12)
}

/// **Law anchor: the hatchling's budget** — bite, stomach, digestion with the mass exponent,
/// assimilation into the reserve's room, upkeep, growth on fullness — over 300 sim ticks
/// (15 s) from a hatchling's stores, the model at the sim's tick against the sim itself.
#[test]
fn hatchling_budget_matches_the_sim() {
    let cfg = FaunaConfig::default();
    let f = Founder::Blind;
    let sc = cfg.founder(f).core;
    let stores = StartingStores { body: sc.body_min / sc.body_max * 1.5, reserve: 0.3 };
    let ticks = 300;
    let s = sim_run(&cfg, f, stores, ticks);
    let (m, b) = model_run(&cfg, f, s.start, ticks);
    let l = &m.ledger[f.index()];
    assert!(close(b.stomach_total(), s.end_stomach, 0.05), "stomach model {} sim {}", b.stomach_total(), s.end_stomach);
    assert!(close(l.flows.assimilated, s.assimilated, 0.05), "assimilated model {} sim {}", l.flows.assimilated, s.assimilated);
    assert!(close(l.flows.upkeep, s.upkeep, 0.02), "upkeep model {} sim {}", l.flows.upkeep, s.upkeep);
    assert!(close(l.flows.growth, s.growth, 0.05), "growth model {} sim {}", l.flows.growth, s.growth);
    assert!(close(b.body, s.end_body, 0.01), "body model {} sim {}", b.body, s.end_body);
    assert!(close(b.reserve, s.end_reserve, 0.05), "reserve model {} sim {}", b.reserve, s.end_reserve);
}

/// **Law anchor: a mature body's brood** (the κ-rule) and the full reserve's room limit, from
/// an adult with a full reserve: digestion is capped by the room, the brood takes
/// `brood_share` of what is placed.
#[test]
fn adult_budget_matches_the_sim() {
    let cfg = FaunaConfig::default();
    let f = Founder::Blind;
    let sc = cfg.founder(f).core;
    let stores = StartingStores { body: (sc.birth_body / sc.body_max * 1.2).min(1.0), reserve: 0.4 };
    let ticks = 300;
    let s = sim_run(&cfg, f, stores, ticks);
    let (m, b) = model_run(&cfg, f, s.start, ticks);
    let l = &m.ledger[f.index()];
    assert!(close(l.flows.assimilated, s.assimilated, 0.05), "assimilated model {} sim {}", l.flows.assimilated, s.assimilated);
    assert!(close(l.flows.growth, s.growth, 0.05), "growth model {} sim {}", l.flows.growth, s.growth);
    assert!(close(b.reserve, s.end_reserve, 0.05), "reserve model {} sim {}", b.reserve, s.end_reserve);
}

/// **Law anchor: the gut ceiling** is the sim's digestion of a full stomach: `y · K / τ'`,
/// with the stomach (`Gut::capacity`) and the throughput factor (`Gut::throughput_factor`)
/// the sim's own methods, and `digested_share` compounding the sim's per-tick `S · DT / τ`.
#[test]
fn gut_ceiling_is_the_sims_digestion() {
    let cfg = FaunaConfig::default();
    for f in [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail, Founder::Lanternjaw] {
        let ph = cfg.founder(f);
        let food = ph.diet.foods().next().unwrap();
        for body in [ph.core.body_min, ph.core.birth_body, ph.core.body_max] {
            let tau = ph.gut.digestion_s[food.index()] / ph.gut.throughput_factor(body, ph.core.body_max);
            let k = ph.gut.capacity(body);
            // Twenty sim ticks of S·DT/τ against the model's one-second compound.
            let mut s = k;
            for _ in 0..TICK_HZ {
                s -= s * cubarium_voxel_fauna::DT / tau;
            }
            let model = k * laws::digested_share(tau, 1.0);
            assert!(close(model, k - s, 1e-9));
            let ceiling = laws::gut_ceiling_per_s(ph, body, food);
            assert!(close(ceiling, ph.yield_for(food) * k / tau, 1e-12));
        }
    }
}

/// **Law anchor: the lifespan draw.** The model's survival at a lifespan's ends and middle
/// matches `draw_lifespan_ticks`'s distribution sampled through the sim's own function.
#[test]
fn lifespan_survival_matches_the_sims_draw() {
    let cfg = FaunaConfig::default();
    let life = cfg.life;
    let median = 3600.0;
    for arriving in [false, true] {
        let n = 4000u64;
        let draws: Vec<f64> = (0..n)
            .map(|id| cubarium_voxel_fauna::draw_lifespan_ticks(&life, median, id, 0xABCD, arriving) as f64 / f64::from(TICK_HZ))
            .collect();
        for t in [0.3 * median, 0.6 * median, 0.9 * median, 1.1 * median] {
            let sim = draws.iter().filter(|d| **d > t).count() as f64 / n as f64;
            let model = laws::lifespan_survival(&life, median, t, arriving);
            assert!((sim - model).abs() < 0.03, "arriving {arriving} t {t}: sim {sim} model {model}");
        }
    }
}

/// **Law anchor: the clutch.** The model lays what `Reproduction::clutch_for` says the reserve
/// pays for, waits for `full_clutch` up to `clutch_hold_max_s`, and pays `eggs · egg_organic`.
#[test]
fn clutch_follows_the_sims_rules() {
    let mut p = Params::defaults();
    p.world.day.mode = DayMode::Off;
    let f = Founder::Blind;
    let sc = p.fauna.founder(f).core;
    let rule = sc.reproduction;
    let mut fits = Fits::unfitted();
    fits.of_mut(f).response[Food::Litter.index()] = Some(Response::holling(0.0, 0.0));
    let mut m = FaunaModel::new(&p, fits, 1.0);
    for g in Founder::ALL {
        m.enabled[g.index()] = g == f;
    }
    let body = sc.body_max;
    let reserve = sc.reserve_of(body);
    m.add(f, Cohort::new(1.0, body, reserve));
    let s = Supply::default();
    for _ in 0..((rule.surplus_hold_s * rule.surplus_share).ceil() as usize + 5) {
        m.step(&s);
    }
    let l = &m.ledger[f.index()];
    assert!(l.clutches > 0.0, "a full adult over its hold lays");
    let eggs = l.laid / l.clutches;
    // What the reserve paid for at laying (it fell a little to upkeep over the hold).
    assert!(eggs >= 1.0 && eggs <= f64::from(rule.full_clutch(reserve).max(1)));
    assert!(close(l.eggs_paid, l.laid * rule.egg_organic, 1e-9));
}

// --- Conservation ---

/// A small world that exercises every flow: detritivores laying and hatching, a gestating
/// grazer, the hunter killing and feeding at the carcass, its bank and a rider.
fn busy(draws: Draws) -> (FaunaModel, Supply) {
    let mut p = Params::defaults();
    p.world.day = DayClock::default();
    let mut fits = Fits::recorded();
    fits.of_mut(Founder::Blind).kill_rate = 2e-4;
    fits.of_mut(Founder::Browser).kill_rate = 1e-4;
    fits.bank_trigger_per_dusk = 0.5;
    fits.carcass_stay_s = 600.0;
    fits.kill_reach = 0.5;
    fits.of_mut(Founder::Lanternjaw).egg_loss = 0.3;
    let mut m = FaunaModel::new(&p, fits, 2.0);
    m.draws = draws;
    for f in [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail, Founder::Browser, Founder::Lanternjaw] {
        let n = if f == Founder::Lanternjaw { 2.0 } else { 4.0 };
        m.add(f, Cohort::founders(&p.fauna, f, n));
    }
    let lp = p.fauna.founder(Founder::Lanternjaw).core;
    m.eggs.push(super::model::EggCohort { lineage: Founder::Lanternjaw, n: 3.0, organic: lp.reproduction.egg_organic, age_s: 0.0, dormant: true, ..Default::default() });
    let mut r = Cohort::new(1.0, lp.body_min, 0.01);
    r.host = Some(Founder::Browser);
    m.add(Founder::Lanternjaw, r);
    let mut s = Supply::default();
    s.litter = 500.0;
    s.film = 4.0;
    s.foliage[Species::Glowcap.index()] = 30.0;
    s.foliage[Species::Springturf.index()] = 300.0;
    (m, s)
}

fn respired(m: &FaunaModel) -> f64 {
    m.ledger
        .iter()
        .map(|l| l.flows.upkeep + l.flows.motor + l.flows.digestion_heat + l.gestation_loss + l.bank_upkeep)
        .sum()
}

/// **The model's ledger closes**: what the animals hold changes by what they took, less what
/// they gave back (dung, spill, corpses, lost and dead eggs), less what they respired, less
/// the carcass that decayed. Both the mean field and the twin, over a world day.
#[test]
fn the_ledger_closes() {
    for draws in [Draws::Mean, Draws::Twin(Rng::new(3))] {
        let (mut m, s) = busy(draws);
        let held0 = m.organic_held();
        let (mut took, mut gave) = (0.0, 0.0);
        for _ in 0..3600 {
            let (t, r) = m.step(&s);
            took += t.litter + t.carrion + t.film + t.nectar + t.fruit + t.foliage.iter().sum::<f64>();
            gave += r.litter + r.carrion;
        }
        let lhs = m.organic_held() - held0;
        let rhs = took - gave - respired(&m) - m.carcass_decayed;
        assert!((lhs - rhs).abs() <= 1e-9 * held0.max(1.0), "held changed {lhs}, flows say {rhs}");
        let l = &m.ledger[Founder::Lanternjaw.index()];
        assert!(l.kills > 0.0, "the busy world exercises the hunt");
        assert!(m.ledger[Founder::Blind.index()].hatched > 0.0, "and hatching");
    }
}

/// **The twin counts bodies**: every cohort's size stays a whole number.
#[test]
fn the_twin_keeps_whole_bodies() {
    let (mut m, s) = busy(Draws::Twin(Rng::new(11)));
    for _ in 0..1800 {
        m.step(&s);
    }
    for f in Founder::ALL {
        for c in &m.pops[f.index()] {
            assert!((c.n - c.n.round()).abs() < 1e-9, "{} cohort of {}", f.name(), c.n);
        }
    }
}

// --- Config flow ---

/// **Upkeep follows `maintenance_per_s` and `sleep_upkeep`** linearly, as `maintenance` does.
#[test]
fn upkeep_follows_its_knobs() {
    let cfg = FaunaConfig::default();
    let mut ph = *cfg.founder(Founder::Blind);
    let body = ph.core.body_max;
    let awake = laws::upkeep_per_s(&ph, body, 1.0);
    let asleep = laws::upkeep_per_s(&ph, body, 0.0);
    assert!(close(asleep, awake * ph.sleep_upkeep, 1e-12));
    ph.core.maintenance_per_s *= 2.0;
    assert!(close(laws::upkeep_per_s(&ph, body, 1.0), 2.0 * awake, 1e-12));
}

/// **The gut's mass exponent** raises a small body's ceiling per unit body by
/// `(body / body_max)^(e − 1)` and leaves the adult's alone.
#[test]
fn mass_exponent_moves_the_small_bodys_ceiling() {
    let cfg = FaunaConfig::default();
    let mut ph = *cfg.founder(Founder::Blind);
    let food = Food::Litter;
    let (small, big) = (ph.core.body_min, ph.core.body_max);
    ph.gut.mass_exponent = 1.0;
    let (s1, b1) = (laws::gut_ceiling_per_s(&ph, small, food), laws::gut_ceiling_per_s(&ph, big, food));
    for e in [0.5, 0.75, 0.9] {
        ph.gut.mass_exponent = e;
        let s = laws::gut_ceiling_per_s(&ph, small, food);
        assert!(close(s / s1, (small / big).powf(e - 1.0), 1e-12));
        assert!(close(laws::gut_ceiling_per_s(&ph, big, food), b1, 1e-12));
    }
}

/// **The brood** (κ-rule): with `brood_share` above 0 a mature body grows less over the
/// same intake and holds a brood; at 0 it holds none.
#[test]
fn brood_share_diverts_growth() {
    let run = |share: f64| {
        // The patch preset's shredder: a laying size below its body_max.
        let mut cfg = super::config::scenario("config/scenarios/patch.toml", &[]).unwrap().fauna;
        cfg.founders[Founder::Blind.index()].core.reproduction.brood_share = share;
        let sc = cfg.founder(Founder::Blind).core;
        assert!(sc.birth_body < sc.body_max);
        let body = sc.birth_body + 0.25 * (sc.body_max - sc.birth_body);
        let (m, b) = model_run(&cfg, Founder::Blind, (body, 0.0, [0.0; Food::COUNT]), 1200 * u64::from(TICK_HZ));
        (m.ledger[Founder::Blind.index()].flows.growth, b.brood)
    };
    let (g0, b0) = run(0.0);
    let (g1, b1) = run(0.8);
    assert!(b0 == 0.0 && b1 > 0.0);
    assert!(g1 < g0, "growth with brood {g1} without {g0}, brood {b1}");
}

/// **Nest crowding** refuses more as `nest_crowd_max` falls, at the same count.
#[test]
fn nest_crowd_max_brakes_laying() {
    let refusals = |max: u32| {
        let mut p = Params::defaults();
        p.world.day.mode = DayMode::Off;
        let f = Founder::Blind;
        p.fauna.founders[f.index()].core.reproduction.nest_crowd_max = max;
        p.fauna.founders[f.index()].core.reproduction.nest_radius_m = 1.0;
        let mut fits = Fits::unfitted();
        fits.of_mut(f).crowd_per_body = 0.5;
        let mut m = FaunaModel::new(&p, fits, 1.0);
        for g in Founder::ALL {
            m.enabled[g.index()] = g == f;
        }
        m.add(f, Cohort::founders(&p.fauna, f, 20.0));
        let mut s = Supply::default();
        s.litter = 1e6;
        for _ in 0..1200 {
            m.step(&s);
        }
        m.ledger[f.index()].nest_refusals
    };
    assert!(refusals(4) > refusals(16));
}

/// **The kill hazard** follows the wound law: halving the hunter's `strike_power` takes a
/// prey that needed one strike to `share²` of the hazard.
#[test]
fn strike_power_moves_the_kill_hazard() {
    let cfg = FaunaConfig::default();
    let mut hp = *cfg.founder(Founder::Lanternjaw);
    let pp = *cfg.founder(Founder::Browser);
    let (hb, pb) = (hp.core.body_max, pp.core.body_max);
    let s1 = laws::strike_share(&hp, hb, &pp, pb);
    let h = hp.hunter.as_mut().unwrap();
    h.strike_power *= 0.5;
    let s2 = laws::strike_share(&hp, hb, &pp, pb);
    if s1 < 1.0 {
        assert!(close(s2, 0.5 * s1, 1e-12));
    } else {
        assert!(s2 <= s1);
    }
}


// --- Cohort bookkeeping (the merge is numerics, not ecology: it must not move the answer) ---

/// A crowded laying world: one lineage at a nest-site count that refuses most clutches, so
/// every laying attempt splits a cohort into the layers and the refused.
fn crowded(draws: Draws, max_cohorts: usize, min_cohort: f64) -> FaunaModel {
    let mut p = Params::defaults();
    p.world.day.mode = DayMode::Off;
    let f = Founder::Blind;
    p.fauna.founders[f.index()].core.reproduction.nest_radius_m = 1.0;
    p.fauna.founders[f.index()].core.reproduction.nest_crowd_max = 6;
    let mut fits = Fits::unfitted();
    // A Poisson mean of 0.25 · 23 ≈ 5.75 at a limit of 6: about a third of attempts refused.
    fits.of_mut(f).crowd_per_body = 0.25;
    fits.of_mut(f).motor_effort = 0.0;
    let mut m = FaunaModel::new(&p, fits, 1.0);
    m.draws = draws;
    m.max_cohorts = max_cohorts;
    m.min_cohort = min_cohort;
    for g in Founder::ALL {
        m.enabled[g.index()] = g == f;
    }
    m.add(f, Cohort::founders(&p.fauna, f, 24.0));
    m
}

/// **Merging keeps bodies and matter**: a merge pass leaves the lineage's body count and the
/// organic matter it holds exactly where they were, whatever it merges.
#[test]
fn merging_keeps_bodies_and_matter() {
    let mut m = crowded(Draws::Mean, 4, 0.05);
    let f = Founder::Blind;
    let sc = m.phys(f).core;
    // Hand-built: ten cohorts spread in body, reserve and timing, three of them tiny.
    for k in 0..10 {
        let x = f64::from(k) / 10.0;
        let body = sc.birth_body * (1.0 + x);
        let mut c = Cohort::new(if k < 3 { 0.01 } else { 1.0 + x }, body, 0.3 * x * sc.reserve_of(body));
        c.age_s = 600.0 * x;
        c.refractory_s = 100.0 * x;
        c.b.stomach[Food::Litter.index()] = 0.001 * x;
        m.add(f, c);
    }
    let n0: f64 = m.pops[f.index()].iter().map(|c| c.n).sum();
    let held0 = m.organic_held();
    m.merge_cohorts();
    let n1: f64 = m.pops[f.index()].iter().map(|c| c.n).sum();
    assert!(close(n0, n1, 1e-12), "bodies {n0} -> {n1}");
    assert!(close(held0, m.organic_held(), 1e-12), "matter {held0} -> {}", m.organic_held());
    assert!(m.pops[f.index()].len() <= 4, "the bound holds: {}", m.pops[f.index()].len());
    assert!(m.pops[f.index()].iter().all(|c| c.n >= 0.05), "no cohort under the floor survives a pass");
}

/// **The bound holds under heavy refusals**, in the mean field and the twin: the count of
/// cohorts stays near `max_cohorts` however often clutches split them.
#[test]
fn heavy_refusals_stay_bounded() {
    for draws in [Draws::Mean, Draws::Twin(Rng::new(5))] {
        let mut m = crowded(draws, 16, 0.01);
        let mut s = Supply::default();
        s.litter = 1e6;
        let mut most = 0;
        for _ in 0..5400 {
            m.step(&s);
            most = most.max(m.pops[Founder::Blind.index()].len());
        }
        assert!(m.ledger[Founder::Blind.index()].nest_refusals > 0.0, "the world refuses");
        // Between passes a cohort can split once per retry: at most doubling, plus hatchlings.
        assert!(most <= 2 * 16 + 64, "cohorts peaked at {most}");
    }
}

/// **The bound does not move the mean field**: the same crowded world with a tight bound and
/// with a loose one lays and keeps the same bodies, within a few per cent.
#[test]
fn the_bound_does_not_move_the_mean_field() {
    let run = |max: usize, min: f64| {
        let mut m = crowded(Draws::Mean, max, min);
        let mut s = Supply::default();
        s.litter = 1e6;
        for _ in 0..7200 {
            m.step(&s);
        }
        let l = &m.ledger[Founder::Blind.index()];
        (m.census(Founder::Blind).n, l.laid, l.nest_refusals)
    };
    let (n_tight, laid_tight, ref_tight) = run(16, 0.01);
    let (n_loose, laid_loose, ref_loose) = run(4096, 0.0);
    assert!(close(n_tight, n_loose, 0.03), "bodies {n_tight} vs {n_loose}");
    assert!(close(laid_tight, laid_loose, 0.03), "laid {laid_tight} vs {laid_loose}");
    assert!(close(ref_tight, ref_loose, 0.05), "refusals {ref_tight} vs {ref_loose}");
}

// --- The hunter at its kill, and the bank's losses ---

/// One lanternjaw cohort, clock off, hungry, nothing else stepped.
fn lone_hunter(fits: Fits) -> (FaunaModel, Params) {
    let mut p = Params::defaults();
    p.world.day.mode = DayMode::Off;
    let mut m = FaunaModel::new(&p, fits, 1.0);
    for g in Founder::ALL {
        m.enabled[g.index()] = g == Founder::Lanternjaw;
    }
    (m, p)
}

/// **A hunter eats its own kill**, not another's: of two hungry hunter cohorts, the one
/// holding a carcass swallows carrion and the other, with no carrion in the pool, does not.
#[test]
fn a_hunter_eats_only_its_own_kill() {
    let (mut m, p) = lone_hunter(Fits::unfitted());
    let lj = Founder::Lanternjaw;
    let sc = p.fauna.founder(lj).core;
    for k in 0..2 {
        let mut c = Cohort::new(1.0, sc.birth_body, 0.05 * sc.reserve_of(sc.birth_body));
        c.b.eating = true;
        if k == 0 {
            c.carcass = 0.5;
        }
        m.add(lj, c);
    }
    let s = Supply::default();
    for _ in 0..30 {
        m.step(&s);
    }
    let fed: Vec<f64> = m.pops[lj.index()].iter().map(|c| c.b.stomach[Food::Carrion.index()]).collect();
    let holder = m.pops[lj.index()].iter().position(|c| c.carcass > 0.0 || c.b.stomach_total() > 0.0).expect("the holder");
    assert!(fed[holder] > 0.0, "the holder fed");
    assert!(fed.iter().enumerate().all(|(k, f)| k == holder || *f == 0.0), "only the holder fed: {fed:?}");
}

/// **An abandoned carcass goes to the carrion pool**: a sated hunter's kill decays at
/// `carrion_decomposition` and is left at `1 / carcass_stay_s`; what is left is returned as
/// carrion, in the share the two rates set. Halving the stay leaves more of it.
#[test]
fn an_abandoned_carcass_goes_to_the_carrion_pool() {
    let returned = |stay: f64| {
        let mut fits = Fits::unfitted();
        fits.carcass_stay_s = stay;
        let (mut m, p) = lone_hunter(fits);
        let lj = Founder::Lanternjaw;
        let sc = p.fauna.founder(lj).core;
        // Sated: a full reserve and stomach, so it neither eats nor hunts.
        let mut c = Cohort::new(1.0, sc.birth_body, sc.reserve_of(sc.birth_body));
        c.b.stomach[Food::Carrion.index()] = p.fauna.founder(lj).gut.capacity(sc.birth_body);
        c.carcass = 1.0;
        m.add(lj, c);
        let mut back = 0.0;
        let t = 600;
        for _ in 0..t {
            back += m.step(&Supply::default()).1.carrion;
        }
        let k = p.flora.carrion_decomposition;
        let r = 1.0 / stay;
        let want = r / (r + k) * (1.0 - (-(r + k) * f64::from(t)).exp());
        (back, want)
    };
    let (b1, w1) = returned(300.0);
    let (b2, w2) = returned(150.0);
    assert!(close(b1, w1, 0.02), "returned {b1}, the rates say {w1}");
    assert!(close(b2, w2, 0.02), "returned {b2}, the rates say {w2}");
    assert!(b2 > b1);
}

/// **Dormant eggs are lost like any clutch** (the sim's `clutch_fate` takes dormant clutches
/// too): at an egg-loss share of one half, half the bank's eggs go, as carrion.
#[test]
fn dormant_eggs_are_lost_like_any_clutch() {
    let mut fits = Fits::unfitted();
    fits.of_mut(Founder::Lanternjaw).egg_loss = 0.5;
    let (mut m, p) = lone_hunter(fits);
    let lj = Founder::Lanternjaw;
    let sc = p.fauna.founder(lj).core;
    let mut c = Cohort::new(1.0, sc.body_max, sc.reserve_of(sc.body_max));
    c.matured = true;
    m.add(lj, c);
    let mut carrion = 0.0;
    for _ in 0..1200 {
        carrion += m.step(&Supply::default()).1.carrion;
    }
    let l = &m.ledger[lj.index()];
    assert!(l.laid > 0.0, "it laid");
    assert!(close(l.eggs_lost, 0.5 * l.laid, 1e-9), "lost {} of {}", l.eggs_lost, l.laid);
    assert!(close(m.census(lj).bank, l.laid - l.eggs_lost, 1e-6));
    assert!(carrion >= l.eggs_lost * sc.reproduction.egg_organic * (1.0 - 1e-9));
}

/// **A kill's reach**: of each corpse the hunter's mouth reaches `kill_reach`, held as its
/// carcass; the rest goes to the carrion pool at once. Every corpse is booked one way or the
/// other.
#[test]
fn a_kill_splits_by_its_reach() {
    for reach in [1.0, 0.3] {
        let mut fits = Fits::unfitted();
        fits.kill_reach = reach;
        fits.of_mut(Founder::Blind).kill_rate = 0.05;
        let (mut m, p) = lone_hunter(fits);
        m.enabled[Founder::Blind.index()] = true;
        let lj = Founder::Lanternjaw;
        let sc = p.fauna.founder(lj).core;
        // Hungry and awake, so it hunts; it cannot eat (its stomach is full) so the carcass
        // stays as laid.
        let mut h = Cohort::new(1.0, sc.body_max, 0.0);
        h.b.stomach[Food::Litter.index()] = p.fauna.founder(lj).gut.capacity(sc.body_max);
        h.b.eating = true;
        m.add(lj, h);
        let bs = p.fauna.founder(Founder::Blind).core;
        m.add(Founder::Blind, Cohort::new(10.0, bs.birth_body, 0.0));
        let (_, r) = m.step(&Supply::default());
        let held: f64 = m.pops[lj.index()].iter().map(|c| c.carcass).sum();
        let total = m.carcass_laid + r.carrion - 0.0;
        assert!(m.ledger[lj.index()].kills > 0.0, "it killed");
        assert!(close(m.carcass_laid, reach * total, 1e-9), "reached {} of {total}", m.carcass_laid);
        assert!(held <= m.carcass_laid * (1.0 + 1e-12));
    }
}
