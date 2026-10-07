//! Mechanism tests for the laws added with the held lineages and the transferable hunt
//! (WEBMODEL-ANIMALS-3): the Hill response, size-asymmetric crowding, the hunting rule by the
//! sim's `hunt.patrol`, the encounter area, the meal cap, the pack's litters, pup provisioning,
//! the body ceiling, and the ledger with all of them running. Hand-built states, every
//! expectation computed from the config on hand: no fitted or tuned number is asserted.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel_fauna::{Food, Founder};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::params::Params;

use super::fits::{Encounter, Fits, HunterFit, Response};
use super::laws;
use super::model::{Cohort, FaunaModel};
use super::rng::{Draws, Rng};
use crate::exchange::Supply;

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-12)
}

fn clock_off() -> Params {
    let mut p = Params::defaults();
    p.world.day.mode = DayMode::Off;
    p
}

fn only(m: &mut FaunaModel, fs: &[Founder]) {
    for g in Founder::ALL {
        m.enabled[g.index()] = fs.contains(&g);
    }
}

/// **The Hill response**: at a stock equal to its half (plus the crowd) ψ is half its
/// maximum whatever the exponent; above 1 the response is sigmoid — lower below the half
/// than Holling type II, higher above it.
#[test]
fn the_hill_exponent_bends_the_response_about_its_half() {
    let r = |q: f64| Response { psi_max: 0.8, half: 2.0, interference: 0.5, hill: q };
    for q in [1.0, 2.0, 3.0] {
        // n = 3: the crowd adds 0.5 · 2 to the half.
        assert!(close(r(q).psi(3.0, 3.0), 0.4, 1e-12), "q {q}: {}", r(q).psi(3.0, 3.0));
    }
    assert!(r(3.0).psi(1.0, 1.0) < r(1.0).psi(1.0, 1.0));
    assert!(r(3.0).psi(6.0, 1.0) > r(1.0).psi(6.0, 1.0));
}

/// One feeding step of two shredder cohorts, big and small, hungry and awake, on litter:
/// each cohort's bite over its own want (ψ as the model applied it).
fn bite_shares(theta: f64, small_body: f64) -> (f64, f64) {
    let p = clock_off();
    let mut fits = Fits::unfitted();
    let f = Founder::Blind;
    fits.of_mut(f).response[Food::Litter.index()] = Some(Response { psi_max: 0.9, half: 50.0, interference: 30.0, hill: 1.0 });
    fits.of_mut(f).crowd_size_exponent = theta;
    let mut m = FaunaModel::new(&p, fits, 1.0);
    only(&mut m, &[f]);
    let sc = p.fauna.founder(f).core;
    for body in [sc.body_max, small_body] {
        let mut c = Cohort::new(4.0, body, 0.0);
        c.b.eating = true;
        m.add(f, c);
    }
    let phys = *p.fauna.founder(f);
    let spill = phys.bite_spill_fraction;
    let mut s = Supply::default();
    s.litter = 200.0;
    // The want from each cohort's state as the step will see it after upkeep (its reserve is
    // 0, so upkeep comes off the body: recomputed after the step from the body then).
    m.step(&s);
    let shares: Vec<f64> = m.pops[f.index()]
        .iter()
        .map(|c| c.b.stomach_total() / ((1.0 - spill) * laws::bite_want(&phys, &c.b, 1.0)))
        .collect();
    (shares[0], shares[1])
}

/// **Size-asymmetric crowding**: with `θ` 0 a small body and a large one beside it get the
/// same share of their bite; with `θ` above 0 the small one gets less (it feels the large as
/// more than one body) and the large more; with every body the same size, `θ` changes nothing.
#[test]
fn a_small_body_feels_large_neighbours_more() {
    let p = clock_off();
    let sc = p.fauna.founder(Founder::Blind).core;
    let small = 0.5 * sc.birth_body;
    let (big0, small0) = bite_shares(0.0, small);
    assert!(close(big0, small0, 1e-6), "θ 0: {big0} vs {small0}");
    let (big1, small1) = bite_shares(1.0, small);
    assert!(small1 < small0 && big1 > big0, "θ 1: big {big1} small {small1}, θ 0 {big0}");
    let (a, b) = bite_shares(0.0, sc.body_max);
    let (c, d) = bite_shares(2.0, sc.body_max);
    assert!(close(a, c, 1e-9) && close(b, d, 1e-9));
}

/// **The hunting rule follows the sim's `hunt.patrol`**: a solitary hunter runs its ambush
/// row when the patrol is off and its own row when on; a pack hunter its own row either way.
#[test]
fn the_hunting_rule_is_the_configs_behaviour() {
    let mut fits = Fits::unfitted();
    let lj = Founder::Lanternjaw;
    let vh = Founder::Chorister;
    let row = |k: f64| HunterFit { kill: [k; Founder::COUNT], reach: 1.0, meal_cap: f64::INFINITY, strikes_per_kill: 1.0, encounter: Encounter::Floor };
    fits.hunters[lj.index()] = Some(row(1.0));
    fits.ambush[lj.index()] = Some(row(2.0));
    fits.hunters[vh.index()] = Some(row(3.0));
    fits.ambush[vh.index()] = Some(row(4.0));
    for patrol in [true, false] {
        let mut p = clock_off();
        p.fauna.hunt.patrol = patrol;
        let m = FaunaModel::new(&p, fits.clone(), 1.0);
        assert_eq!(m.hunt_rule(lj).kill[0], if patrol { 1.0 } else { 2.0 });
        assert_eq!(m.hunt_rule(vh).kill[0], 3.0);
    }
}

/// A lone hungry lanternjaw over ten shredders for one step under `rule`, with the supply's
/// litter area `area` against a reference of 10: its kills.
fn kills_over(rule: HunterFit, area: f64) -> f64 {
    let p = clock_off();
    let lj = Founder::Lanternjaw;
    let mut fits = Fits::unfitted();
    fits.hunters[lj.index()] = Some(rule);
    let mut m = FaunaModel::new(&p, fits, 1.0);
    m.config.hunt.patrol = true;
    only(&mut m, &[lj, Founder::Blind]);
    m.ref_area[Food::Litter.index()] = 10.0;
    m.area_scale = 1.0;
    let sc = p.fauna.founder(lj).core;
    let mut h = Cohort::new(1.0, sc.body_max, 0.0);
    h.b.stomach[Food::Litter.index()] = p.fauna.founder(lj).gut.capacity(sc.body_max);
    h.b.eating = true;
    m.add(lj, h);
    let bs = p.fauna.founder(Founder::Blind).core;
    m.add(Founder::Blind, Cohort::new(10.0, bs.birth_body, 0.0));
    let mut s = Supply::default();
    s.area.litter = area;
    m.step(&s);
    m.ledger[lj.index()].kills
}

/// **Where it meets prey**: under [`Encounter::PreyHabitat`] a prey's density is read over its
/// own food's area, so twice the shredder's litter area halves the hazard; over the floor
/// ([`Encounter::Floor`]) the litter area changes nothing.
#[test]
fn the_patrol_meets_prey_at_their_habitats_density() {
    let mut rule = HunterFit { kill: [0.0; Founder::COUNT], reach: 1.0, meal_cap: f64::INFINITY, strikes_per_kill: 1.0, encounter: Encounter::PreyHabitat };
    rule.kill[Founder::Blind.index()] = 1e-4;
    let (a, b) = (kills_over(rule, 10.0), kills_over(rule, 20.0));
    assert!(a > 0.0 && close(a / b, 2.0, 1e-3), "habitat: {a} vs {b}");
    rule.encounter = Encounter::Floor;
    let (c, d) = (kills_over(rule, 10.0), kills_over(rule, 20.0));
    assert!(c > 0.0 && close(c, d, 1e-12), "floor: {c} vs {d}");
}

/// **A kill's meal is capped**: of each corpse the hunter gets its reach's share, at most the
/// cap; the rest is carrion at once; every corpse is booked one way or the other.
#[test]
fn a_kill_feeds_at_most_its_meal_cap() {
    let p = clock_off();
    let lj = Founder::Lanternjaw;
    let bs = p.fauna.founder(Founder::Blind).core;
    let corpse = bs.body_max + 0.5 * bs.reserve_of(bs.body_max);
    for cap in [0.25 * corpse, f64::INFINITY] {
        let mut fits = Fits::unfitted();
        let mut rule = HunterFit { kill: [0.0; Founder::COUNT], reach: 0.8, meal_cap: cap, strikes_per_kill: 1.0, encounter: Encounter::Floor };
        rule.kill[Founder::Blind.index()] = 0.05;
        fits.hunters[lj.index()] = Some(rule);
        let mut m = FaunaModel::new(&p, fits, 1.0);
        m.config.hunt.patrol = true;
        only(&mut m, &[lj, Founder::Blind]);
        let sc = p.fauna.founder(lj).core;
        let mut h = Cohort::new(1.0, sc.body_max, 0.0);
        h.b.stomach[Food::Litter.index()] = p.fauna.founder(lj).gut.capacity(sc.body_max);
        h.b.eating = true;
        m.add(lj, h);
        let mut prey = Cohort::new(10.0, bs.body_max, 0.5 * bs.reserve_of(bs.body_max));
        prey.matured = true;
        m.add(Founder::Blind, prey);
        // The prey's corpse as it dies: its organic matter after its own step's upkeep.
        let (_, r) = m.step(&Supply::default());
        let kills = m.ledger[lj.index()].kills;
        let each = m.pops[Founder::Blind.index()][0].b.organic();
        assert!(kills > 0.0);
        assert!(close(m.carcass_laid, kills * (0.8 * each).min(cap), 1e-9), "cap {cap}: laid {}", m.carcass_laid);
        assert!(close(m.carcass_laid + r.carrion, kills * each, 1e-9));
    }
}

/// A Veilhound pack world: `n` adults at the arrival stores, full reserve, clock off.
fn pack_world(n: f64, litter_max: u32, draws: Draws) -> (FaunaModel, Params) {
    let mut p = clock_off();
    let vh = Founder::Chorister;
    p.fauna.founders[vh.index()].pack.as_mut().unwrap().litter_size_max = litter_max;
    // The interval rule alone (no LIFE-HISTORY season, the drawn litter of 2 up).
    p.fauna.founders[vh.index()].core.reproduction.litters_per_year = 0;
    p.fauna.founders[vh.index()].pack.as_mut().unwrap().litter_by_condition = false;
    p.fauna.founders[vh.index()].pack.as_mut().unwrap().breed_reserve_fraction = 0.0;
    let mut m = FaunaModel::new(&p, Fits::unfitted(), 1.0);
    m.draws = draws;
    only(&mut m, &[vh]);
    let sc = p.fauna.founder(vh).core;
    let mut c = Cohort::new(n, sc.body_max, sc.reserve_of(sc.body_max));
    c.matured = true;
    m.add(vh, c);
    (m, p)
}

fn run_litters(m: &mut FaunaModel, p: &Params) -> (f64, f64) {
    let vh = Founder::Chorister;
    let rule = p.fauna.founder(vh).core.reproduction;
    let steps = (rule.surplus_hold_s + rule.gestation_s + 600.0) as usize;
    // A full carcass at its mouth all along, so its reserve stays up.
    for _ in 0..steps {
        for c in m.pops[vh.index()].iter_mut() {
            c.carcass = 10.0 * c.n;
        }
        m.step(&Supply::default());
    }
    let l = &m.ledger[vh.index()];
    (l.litters, l.born)
}

/// **A pack's litters**: only its breeders conceive — never more litters inside one
/// `litter_interval_s` than the lineage has packs (`pack_size` adults each) — and each
/// litter holds between 2 and `litter_size_max` young; a larger `litter_size_max` gives no
/// fewer young (config flow). In the twin every litter is whole.
#[test]
fn only_a_packs_breeder_litters_within_its_interval() {
    let vh = Founder::Chorister;
    let mut young = Vec::new();
    for max in [3u32, 6] {
        let (mut m, p) = pack_world(8.0, max, Draws::Mean);
        let t = p.fauna.founder(vh).pack.unwrap();
        let packs = (8.0 / t.pack_size as f64).round().max(1.0);
        let (litters, born) = run_litters(&mut m, &p);
        assert!(litters > 0.0 && litters <= packs + 1e-9, "max {max}: {litters} litters, {packs} packs");
        let per = born / litters;
        assert!(per >= 2.0 - 1e-9 && per <= f64::from(max) + 1e-9, "max {max}: {per} young a litter");
        young.push(per);
    }
    assert!(young[1] >= young[0]);
    let (mut m, p) = pack_world(8.0, 4, Draws::Twin(Rng::new(5)));
    let (litters, born) = run_litters(&mut m, &p);
    assert!(litters >= 1.0 && (born - born.round()).abs() < 1e-9 && born >= 2.0 * litters);
}

/// One step of an adult and a pup at an hour the diurnal adults sleep, the adult's reserve at
/// `frac` of its cap: (the adult's reserve lost, the pup's gained, the loss booked).
fn provision_once(frac: f64) -> (f64, f64, f64, f64) {
    let mut p = Params::defaults();
    p.world.day = DayClock::default();
    let vh = Founder::Chorister;
    let mut m = FaunaModel::new(&p, Fits::unfitted(), 1.0);
    only(&mut m, &[vh]);
    // An hour the adults are asleep (at the den).
    let day = m.day.day_s();
    let night = (0..1440).map(|k| k as f64 * day / 1440.0).find(|t| m.on_phase(vh, m.daylight_at(*t + 1.0)) == 0.0).expect("a night");
    m.t_s = night;
    let sc = p.fauna.founder(vh).core;
    let mut a = Cohort::new(2.0, sc.body_max, frac * sc.reserve_of(sc.body_max));
    a.matured = true;
    a.b.stomach[Food::Carrion.index()] = 0.0;
    m.add(vh, a);
    // A pup with a little reserve (so its upkeep does not starve it at `body_min`).
    let mut pup = Cohort::new(3.0, sc.body_min, 0.2 * sc.reserve_of(sc.body_min));
    pup.native = true;
    m.add(vh, pup);
    let before: Vec<f64> = m.pops[vh.index()].iter().map(|c| c.n * c.b.reserve).collect();
    // Upkeep moves both reserves too: the same step without the pup gives the adult's own.
    let mut alone = m.clone();
    alone.pops[vh.index()].truncate(1);
    alone.step(&Supply::default());
    m.step(&Supply::default());
    let after: Vec<f64> = m.pops[vh.index()].iter().map(|c| c.n * c.b.reserve).collect();
    let adult_alone = alone.pops[vh.index()][0].n * alone.pops[vh.index()][0].b.reserve;
    let l = &m.ledger[vh.index()];
    (adult_alone - after[0], after[1] - before[1], l.provision_loss, p.fauna.founder(vh).pack.unwrap().provision_efficiency)
}

/// **Provisioning**: an adult asleep at the den above `provision_reserve_fraction` of its cap
/// gives its pups reserve; they receive `provision_efficiency` of it and the rest is booked
/// respired. Under the fraction it gives nothing (config flow on the fraction).
#[test]
fn a_fed_adult_provisions_its_pups() {
    let p = Params::defaults();
    let t = p.fauna.founder(Founder::Chorister).pack.unwrap();
    let (given, got, loss, eff) = provision_once(1.0);
    assert!(given > 0.0, "a full adult gives");
    // The pup's reserve also paid its own upkeep: what arrived is at least what it gained.
    assert!(close(loss, given * (1.0 - eff), 1e-6), "loss {loss} of {given}");
    assert!(got <= given * eff * (1.0 + 1e-9));
    let (given, _, loss, _) = provision_once(0.9 * t.provision_reserve_fraction);
    assert!(given.abs() < 1e-12 && loss == 0.0, "under the fraction: gave {given}");
}

/// **The body ceiling** (`lineage_body_ceiling`): a lineage holding the ceiling's bodies
/// breeds no more and books the refusals; under it, the same bodies breed.
#[test]
fn a_lineage_at_its_ceiling_refuses_breeding() {
    let g = Founder::Browser;
    let mut born = Vec::new();
    for ceiling in [4u32, 1000] {
        let mut p = clock_off();
        p.fauna.lineage_body_ceiling = ceiling;
        let mut m = FaunaModel::new(&p, Fits::unfitted(), 1.0);
        only(&mut m, &[g]);
        let sc = p.fauna.founder(g).core;
        let mut c = Cohort::new(4.0, sc.body_max, sc.reserve_of(sc.body_max));
        c.matured = true;
        m.add(g, c);
        let mut s = Supply::default();
        s.foliage[Species::Springturf.index()] = 1e4;
        let rule = sc.reproduction;
        for _ in 0..(rule.surplus_hold_s + rule.gestation_s + 300.0) as usize {
            m.step(&s);
        }
        born.push((m.ledger[g.index()].born, m.ledger[g.index()].ceiling_refusals));
    }
    assert!(born[0].0 == 0.0 && born[0].1 > 0.0, "at the ceiling: {:?}", born[0]);
    assert!(born[1].0 > 0.0 && born[1].1 == 0.0, "under it: {:?}", born[1]);
}

/// **A pup neither hunts nor feeds itself**: a pack's young under `independence_age_s`,
/// hungry and awake beside prey, makes no kill and swallows nothing; the same body past that
/// age hunts.
#[test]
fn a_pup_does_not_hunt() {
    let p = clock_off();
    let vh = Founder::Chorister;
    let indep = p.fauna.founder(vh).pack.unwrap().independence_age_s;
    let mut kills = Vec::new();
    for age in [0.0, indep + 1.0] {
        let mut fits = Fits::unfitted();
        let mut rule = HunterFit { kill: [0.0; Founder::COUNT], reach: 1.0, meal_cap: f64::INFINITY, strikes_per_kill: 1.0, encounter: Encounter::Floor };
        rule.kill[Founder::Browser.index()] = 0.05;
        fits.hunters[vh.index()] = Some(rule);
        let mut m = FaunaModel::new(&p, fits, 1.0);
        only(&mut m, &[vh, Founder::Browser]);
        let sc = p.fauna.founder(vh).core;
        let mut pup = Cohort::new(2.0, 2.0 * sc.body_min, 0.2 * sc.reserve_of(2.0 * sc.body_min));
        pup.native = true;
        pup.age_s = age;
        pup.b.eating = true;
        m.add(vh, pup);
        let gs = p.fauna.founder(Founder::Browser).core;
        m.add(Founder::Browser, Cohort::new(5.0, gs.body_min, 0.0));
        let mut s = Supply::default();
        s.carrion = 100.0;
        for _ in 0..30 {
            m.step(&s);
        }
        assert!(m.census(vh).n > 0.0, "the young still live");
        let l = &m.ledger[vh.index()];
        if age == 0.0 {
            assert_eq!(l.hunting_s, 0.0);
            assert_eq!(l.swallowed.iter().sum::<f64>(), 0.0);
        }
        kills.push(l.kills);
    }
    assert!(kills[0] == 0.0 && kills[1] > 0.0, "pup {} vs independent {}", kills[0], kills[1]);
}

/// **The ledger closes with the pack running**: kills by the Veilhound, its shared feast,
/// provisioning at night, litters, the meal cap; the mean field and the twin, over a world day.
#[test]
fn the_ledger_closes_with_the_pack() {
    for draws in [Draws::Mean, Draws::Twin(Rng::new(9))] {
        let mut p = Params::defaults();
        p.world.day = DayClock::default();
        let vh = Founder::Chorister;
        // Litters inside the hour (LIFE-HISTORY's season and gestation would put the first
        // after it): no season and a short gestation, so the pack provisions its young.
        p.fauna.founders[vh.index()].core.reproduction.litters_per_year = 0;
        p.fauna.founders[vh.index()].core.reproduction.gestation_s = 180.0;
        let mut fits = Fits::recorded();
        let mut rule = HunterFit { kill: [0.0; Founder::COUNT], reach: 1.0, meal_cap: 0.2, strikes_per_kill: 2.0, encounter: Encounter::Floor };
        rule.kill[Founder::Browser.index()] = 1e-3;
        fits.hunters[vh.index()] = Some(rule);
        let mut m = FaunaModel::new(&p, fits, 2.0);
        m.draws = draws;
        only(&mut m, &[vh, Founder::Browser]);
        let sc = p.fauna.founder(vh).core;
        let mut a = Cohort::new(4.0, sc.body_max, sc.reserve_of(sc.body_max));
        a.matured = true;
        m.add(vh, a);
        let mut pup = Cohort::new(2.0, sc.body_min, 0.0);
        pup.native = true;
        m.add(vh, pup);
        m.add(Founder::Browser, Cohort::founders(&p.fauna, Founder::Browser, 12.0));
        let mut s = Supply::default();
        s.foliage[Species::Springturf.index()] = 500.0;
        let held0 = m.organic_held();
        let (mut took, mut gave) = (0.0, 0.0);
        for _ in 0..3600 {
            let (t, r) = m.step(&s);
            took += t.litter + t.carrion + t.film + t.nectar + t.fruit + t.foliage.iter().sum::<f64>();
            gave += r.litter + r.carrion;
        }
        let respired: f64 = m.ledger.iter().map(|l| l.flows.upkeep + l.flows.motor + l.flows.digestion_heat + l.gestation_loss + l.bank_upkeep).sum();
        let lhs = m.organic_held() - held0;
        let rhs = took - gave - respired - m.carcass_decayed;
        assert!((lhs - rhs).abs() <= 1e-9 * held0.max(1.0), "held changed {lhs}, flows say {rhs}");
        let l = &m.ledger[vh.index()];
        assert!(l.kills > 0.0 && l.provision_given > 0.0, "kills {} provision {}", l.kills, l.provision_given);
    }
}

/// **LIFE-HISTORY in the model** (`conceive_litters`, as the sim's `gestate`): a pack with a
/// breeding season conceives only inside its window and at most `litters_per_year` times a
/// year (here one pack, one a year, over three short years), and a breeder under
/// `breed_reserve_fraction` of its cap does not conceive.
#[test]
fn a_seasonal_pack_conceives_inside_its_window_once_a_year() {
    let vh = Founder::Chorister;
    let (phase, window) = (0.5, 0.2);
    let run = |reserve_fraction: f64| -> Vec<(i64, f64)> {
        let mut p = clock_off();
        p.world.day.length_min = 10.0;
        p.world.climate.dry_season_every_days = 1.0;
        p.world.climate.dry_season_phase_days = 0.0;
        {
            let f = &mut p.fauna.founders[vh.index()];
            let r = &mut f.core.reproduction;
            r.litters_per_year = 1;
            r.breeding_phase = phase;
            r.breeding_window = window;
            r.surplus_hold_s = 10.0;
            r.gestation_s = 20.0;
            r.birth_interval_s = 1.0;
            let t = f.pack.as_mut().unwrap();
            t.pack_size = 4;
            t.litter_interval_s = 0.0;
            t.breed_reserve_fraction = 0.6;
        }
        let mut m = FaunaModel::new(&p, Fits::unfitted(), 1.0);
        only(&mut m, &[vh]);
        let sc = p.fauna.founder(vh).core;
        let mut c = Cohort::new(4.0, sc.body_max, reserve_fraction * sc.reserve_of(sc.body_max));
        c.matured = true;
        m.add(vh, c);
        let mut seen = Vec::new();
        let mut last = 0.0;
        for _ in 0..1800 {
            if reserve_fraction > 0.9 {
                for c in m.pops[vh.index()].iter_mut() {
                    c.carcass = 10.0 * c.n;
                }
            }
            m.step(&Supply::default());
            let n = m.ledger[vh.index()].conceived;
            if n > last {
                let y = m.year().expect("a year");
                seen.push((y.number, y.phase));
                last = n;
            }
        }
        seen
    };
    let fed = run(1.0);
    assert!(!fed.is_empty(), "a fed pack conceives");
    for &(_, ph) in &fed {
        assert!((ph - phase).rem_euclid(1.0) < window + 0.01, "conceived at phase {ph}: {fed:?}");
    }
    let mut years: Vec<i64> = fed.iter().map(|c| c.0).collect();
    years.dedup();
    assert_eq!(years.len(), fed.len(), "once a year: {fed:?}");
    assert!(fed.len() >= 2, "a conception in each of the years: {fed:?}");
    assert!(run(0.3).is_empty(), "a breeder under the breeding reserve does not conceive");
}
// MIMIC: written before the daytime phase and optional seedporter hunting change.
#[test]
fn daytime_mimic_phase_follows_its_switch() {
    let mut p = Params::defaults();
    p.world.day.mode = DayMode::Cycle;
    let lj = Founder::Lanternjaw;
    p.fauna.founders[lj.index()].lure_daylight = false;
    let off = FaunaModel::new(&p, Fits::unfitted(), 1.0);
    let old = off.on_phase(lj, 1.0);
    p.fauna.founders[lj.index()].lure_daylight = true;
    let on = FaunaModel::new(&p, Fits::unfitted(), 1.0);
    assert!(old < 1.0);
    assert_eq!(on.on_phase(lj, 1.0), 1.0);
}

#[test]
fn seedporter_model_catches_only_when_enabled_and_profitable() {
    let run = |enabled: bool, gain: f64| {
        let mut p = clock_off();
        let sp = Founder::Seedporter;
        let bw = Founder::Bellwing;
        p.fauna.founders[sp.index()].catch_bellwings = enabled;
        p.fauna.founders[sp.index()].catch_gain_min = gain;
        let mut h = p.fauna.founder(Founder::Lanternjaw).hunter.unwrap();
        h.prey = [false; Founder::COUNT];
        h.prey[bw.index()] = true;
        h.prey_max_ratio = 10.0;
        h.strike_power = 10.0;
        h.strike_cost_s = 0.0;
        p.fauna.founders[sp.index()].hunter = Some(h);
        let mut fits = Fits::unfitted();
        let mut row = HunterFit::NONE;
        row.kill[bw.index()] = 0.1;
        fits.hunters[sp.index()] = Some(row);
        let mut m = FaunaModel::new(&p, fits, 1.0);
        only(&mut m, &[sp, bw]);
        let body = p.fauna.founder(sp).core.body_max;
        let mut c = Cohort::new(1.0, body, 0.0);
        c.b.eating = true;
        m.add(sp, c);
        m.add(bw, Cohort::new(10.0, 0.5 * body, 0.0));
        m.step(&Supply::default());
        m.ledger[bw.index()].killed_by[sp.index()]
    };
    assert!(run(true, 0.0) > 0.0);
    assert_eq!(run(false, 0.0), 0.0);
    assert_eq!(run(true, 100.0), 0.0);
}

#[test]
fn seedporter_model_switch_off_keeps_fruit_and_refuses_carrion() {
    let run = |enabled: bool, food: Food| {
        let mut p = clock_off();
        let sp = Founder::Seedporter;
        p.fauna.founders[sp.index()].catch_bellwings = enabled;
        p.fauna.founders[sp.index()].diet = cubarium_voxel_fauna::Diet::of(&[Food::Fruit, Food::Carrion]);
        let mut fits = Fits::unfitted();
        fits.of_mut(sp).response[food.index()] = Some(Response::holling(1.0, 0.0));
        let mut m = FaunaModel::new(&p, fits, 1.0);
        only(&mut m, &[sp]);
        let mut c = Cohort::new(1.0, p.fauna.founder(sp).core.body_max, 0.0);
        c.b.eating = true;
        m.add(sp, c);
        let mut s = Supply::default();
        if food == Food::Fruit { s.fruit = 1.0 } else { s.carrion = 1.0 }
        m.step(&s);
        m.ledger[sp.index()].swallowed[food.index()]
    };
    assert_eq!(run(false, Food::Carrion), 0.0);
    assert!(run(true, Food::Carrion) > 0.0);
    assert!(run(false, Food::Fruit) > 0.0 && run(true, Food::Fruit) > 0.0);
}
