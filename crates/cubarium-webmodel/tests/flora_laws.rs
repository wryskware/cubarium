//! The producer half's checks (`design/webmodel/flora.md`, "Tests"): law anchors against the
//! sim's own one-stand fixture, the model's ledgers, and config flow. No tuned number is
//! pinned: every expectation is the sim's own function or the config's own direction.

use std::path::Path;

use cubarium_voxel_flora::study::{ClimateSample, Held, Start, probe, start_stand};
use cubarium_voxel_flora::{FloraConfig, Species, Trophic};
use cubarium_webmodel::flora::census::Face;
use cubarium_webmodel::flora::climate::DayModel;
use cubarium_webmodel::flora::kernel::{Kernel, RunnerBox};
use cubarium_webmodel::flora::model::{Model, NS, Params, Weather};
use cubarium_webmodel::flora::stand::{self, StandState, light_response};
use cubarium_webmodel::flora::zones::{Key, Reading, Zone, Zones};
use cubarium_webmodel::flora::{config, model};

fn resolved() -> (cubarium_voxel_sim::params::Params, FloraConfig) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/scenarios/terrarium.toml");
    let p = config::resolve(&root, &[]).expect("the terrarium resolves");
    let f = config::flora_of(&p);
    (p, f)
}

/// The sim's clock samples from the model's day clock, one per tick from `t0`.
fn clock(dm: &DayModel, t0: f64, ticks: usize) -> Vec<ClimateSample> {
    (0..ticks)
        .map(|k| {
            let c = dm.at(t0 + k as f64 * cubarium_voxel::DT, cubarium_voxel::DT);
            ClimateSample { daylight: c.daylight, upkeep: c.upkeep, night: c.night, to_dawn_s: c.to_dawn_s }
        })
        .collect()
}

/// **Law anchor**: the model's stand law, stepped at the sim's tick, against `study::probe`
/// (the sim's own `grow_stand` on a one-stand fixture) through a dusk, a night and a dawn.
/// Lanternberry is left out: its fruit rest is the sim's keyed exponential draw, which the
/// model takes at its mean.
#[test]
fn the_stand_law_is_the_sims_on_a_one_stand_fixture() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    // From mid-afternoon through dusk into the night: both budgets run.
    let t0 = 0.55 * dm.day_s();
    let ticks = (0.35 * dm.day_s() / cubarium_voxel::DT) as usize;
    let clim = clock(&dm, t0, ticks);
    let cases = [
        (Species::Springturf, Start::Newborn, Held { water: 0.5, saturated: 0.0, light: 0.8, mineral: 1.0 }),
        (Species::Springturf, Start::Adult, Held { water: 0.15, saturated: 0.2, light: 0.7, mineral: 0.4 }),
        (Species::Velvetpad, Start::Adult, Held { water: 0.7, saturated: 0.6, light: 0.5, mineral: 0.9 }),
        (Species::Vaulttree, Start::Newborn, Held { water: 0.5, saturated: 0.0, light: 0.95, mineral: 1.0 }),
        (Species::Stonecushion, Start::Newborn, Held { water: 0.2, saturated: 0.0, light: 0.6, mineral: 0.6 }),
        (Species::Bloomcrown, Start::Adult, Held { water: 0.6, saturated: 0.0, light: 0.7, mineral: 1.0 }),
    ];
    for (sp, start, held) in cases {
        let sc = fc.species(sp);
        let s0 = start_stand(&fc, sp, start);
        let o = probe(&fc, s0, &clim, |_| held);
        let mut st = StandState { wood: s0.wood, foliage: s0.foliage, reserve: s0.reserve, ..StandState::default() };
        let site = stand::Site {
            light_eff: light_response(sc, held.light),
            available: held.water,
            saturated: held.saturated,
            mineral: held.mineral,
            darkness: 0.0,
            substrate_taken: 0.0,
            substrate_gross: 0.0,
        };
        let (mut fixed, mut packages, mut died) = (0.0, 0u32, false);
        for k in 0..ticks {
            let c = dm.at(t0 + k as f64 * cubarium_voxel::DT, cubarium_voxel::DT);
            let (fl, d) = stand::grow(sc, &mut st, site, c);
            fixed += fl.income;
            if d.is_some() {
                died = true;
                break;
            }
            packages += stand::fund(sc, &mut st, cubarium_voxel::DT).0;
        }
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()));
        assert_eq!(died, !o.alive, "{sp:?} {start:?}: death");
        if o.alive {
            assert!(close(st.wood, o.wood[1]), "{sp:?} {start:?}: wood {} vs sim {}", st.wood, o.wood[1]);
            assert!(close(st.foliage, o.foliage[1]), "{sp:?} {start:?}: foliage {} vs sim {}", st.foliage, o.foliage[1]);
            assert!(close(st.reserve, o.reserve[1]), "{sp:?} {start:?}: reserve {} vs sim {}", st.reserve, o.reserve[1]);
            assert!(close(fixed, o.fixed), "{sp:?} {start:?}: fixed {fixed} vs sim {}", o.fixed);
            assert_eq!(packages, o.packages, "{sp:?} {start:?}: packages");
        }
    }
}

/// **Law anchor**: the model's clock is the world's day clock: daylight, the night factor
/// and the time to dawn at a world tick, read from the sim's own `DayClock::sun`.
#[test]
fn the_plant_clock_is_the_worlds() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let day = &p.world.day;
    for k in 0..48u64 {
        let tick = k * (dm.day_s() / cubarium_voxel::DT) as u64 / 48;
        let sun = day.sun(tick, None);
        let c = dm.at(tick as f64 * cubarium_voxel::DT, 1.0);
        assert!((c.daylight - sun.daylight).abs() < 1e-12);
        assert!((c.night - (sun.daylight + (1.0 - sun.daylight) * day.plant_night_factor)).abs() < 1e-12);
        assert_eq!(c.to_dawn_s.is_some(), sun.daylight < 1.0 && (sun.phase - sun.rise).rem_euclid(1.0) > (sun.set - sun.rise).rem_euclid(1.0));
    }
}

/// A one-zone world of `n` faces, held weather, every species' gate open, mineral `n0`.
fn one_zone(fc: &FloraConfig, n: usize, sky: f64, n0: f64, occupants: &[(Species, f64, usize)]) -> (Zones, Vec<Face>) {
    let mut faces: Vec<Face> = (0..n).map(|i| Face::bare(i as u32, 1, 0, sky, n0)).collect();
    let mut i = 0;
    for &(sp, frac, count) in occupants {
        let sc = fc.species(sp);
        for _ in 0..count {
            let w = sc.wood_max * frac;
            let rec = cubarium_webmodel::flora::census::StandRec { wood: w, foliage: sc.alpha * w * 0.5, reserve: sc.reserve_cap * w * 0.6, light: 1.0, moisture: 1.0, ..Default::default() };
            if sc.trophic == Trophic::Saprotroph {
                faces[i].fungus = Some(rec);
            } else {
                faces[i].plant = Some((sp, rec));
            }
            i += 1;
        }
    }
    let r = Reading { mu: 1.0, available: 1.0, saturated: 0.0, stress_target: 0.0, gate: 1.0, mu_held: 1.0, stress_held: 0.0, available_held: 1.0 };
    let zone = Zone {
        key: Key { soil: true, under_roof: false, band: 4, wet: false },
        area: n as f64,
        sky,
        darkness: 0.0,
        fungal_space: 1.0,
        weather: vec![[r; Species::COUNT]],
    };
    let of_face = faces.iter().map(|f| ((f.x, f.y, f.z), 0)).collect();
    (Zones { zones: vec![zone], of_face }, faces)
}

fn kernels() -> (Vec<Kernel>, Vec<RunnerBox>) {
    (
        (0..NS).map(|_| Kernel { land: vec![vec![1.0]], lost: vec![0.0] }).collect(),
        (0..NS).map(|_| RunnerBox { faces: 8.0, same_zone: 1.0 }).collect(),
    )
}

fn build(fc: &FloraConfig, p: &cubarium_voxel_sim::params::Params, zones: Zones, faces: &[Face], mp: Params) -> Model {
    let (k, rb) = kernels();
    Model::new(fc.clone(), DayModel::of(&p.world, fc.upkeep_q10), zones, k, rb, faces, 480.0, mp)
}

fn params() -> Params {
    Params { weather: Weather::Frozen, ..Params::default() }
}

/// **Conservation**: over a day and a half of growth, seed rain, germination, takeover,
/// deaths, decomposition and a clamped take and return, the organic ledger closes —
/// `Δ held = fixed + seep + returned − respired − decomposed − taken` — and the mineral
/// (site pools, stand inventories, banks) is unchanged.
#[test]
fn the_ledgers_close() {
    let (p, fc) = resolved();
    let (zones, faces) = one_zone(&fc, 400, 0.85, 1.0, &[(Species::Springturf, 1.0, 60), (Species::Velvetpad, 1.0, 120), (Species::Lanternberry, 0.8, 10), (Species::Glowcap, 1.0, 50)]);
    let mut m = build(&fc, &p, zones, &faces, params());
    for z in &mut m.z {
        z.litter = 5.0;
        z.bank[Species::Lanternberry.index()] = 50.0;
    }
    m.take.foliage[Species::Velvetpad.index()] = 1e-3;
    m.take.litter = 1e-4;
    m.returns.litter = 5e-5;
    m.returns.carrion = 1e-4;
    let o0 = m.organic();
    let (a, b, c) = m.mineral();
    let n0 = a + b + c;
    for _ in 0..(1.5 * m.day.day_s() / m.p.dt) as usize {
        m.step();
    }
    let k = &m.c;
    let taken = k.taken.litter + k.taken.foliage.iter().sum::<f64>() + k.taken.carrion + k.taken.film + k.taken.nectar + k.taken.fruit;
    let expect = o0 + k.fixed + k.seep + k.returned - k.respired - k.decomposed - taken;
    let o1 = m.organic();
    assert!((o1 - expect).abs() <= 1e-8 * o0.max(1.0), "organic {o1} vs ledger {expect}");
    let (a, b, c) = m.mineral();
    let n1 = a + b + c + k.taken_mineral;
    assert!((n1 - n0).abs() <= 1e-8 * n0, "mineral {n1} vs {n0}");
    assert!(k.births_seed.iter().sum::<f64>() > 0.0 && k.overtopped.iter().sum::<f64>() > 0.0, "the flows under test fired");
}

/// **Config flow**: raising springturf's maintenance in the config raises the model's
/// upkeep the way the law says — less net income, so fewer packages from the same adult —
/// and raising its propagule rate raises the packages a funded donor sends.
#[test]
fn a_knob_moves_the_model_the_way_the_sim_law_does() {
    let (p, base) = resolved();
    let dm = DayModel::of(&p.world, base.upkeep_q10);
    let run = |fc: &FloraConfig| {
        let sc = fc.species(Species::Springturf);
        let s0 = start_stand(fc, Species::Springturf, Start::Adult);
        let mut st = StandState { wood: s0.wood, foliage: s0.foliage, reserve: s0.reserve, ..StandState::default() };
        let site = stand::Site { light_eff: light_response(sc, 0.8), available: 0.6, saturated: 0.0, mineral: 1.0, darkness: 0.0, substrate_taken: 0.0, substrate_gross: 0.0 };
        let mut sent = 0;
        for k in 0..720 {
            let c = dm.at(k as f64 * 10.0, 10.0);
            let _ = stand::grow(sc, &mut st, site, c);
            sent += stand::fund(sc, &mut st, 10.0).0;
        }
        sent
    };
    let mut up = base.clone();
    up.species_mut(Species::Springturf).maintenance *= 4.0;
    assert!(run(&up) < run(&base), "more upkeep, fewer packages");
    let mut more = base.clone();
    more.species_mut(Species::Springturf).propagule_rate *= 2.0;
    assert!(run(&more) >= run(&base), "a higher propagule rate never sends fewer");
}

/// **Config flow**: a woody species' overtopping chance is what lets its seedlings take a
/// ground cover's faces. At zero (the config switching it off) no turf is overtopped by it;
/// at the config's value some is.
#[test]
fn the_overtop_chance_is_the_takeover_switch() {
    let (p, base) = resolved();
    let go = |fc: &FloraConfig| {
        let (zones, faces) = one_zone(fc, 200, 0.95, 1.0, &[(Species::Springturf, 1.0, 200)]);
        let mut m = build(fc, &p, zones, &faces, params());
        m.z[0].bank[Species::Vaulttree.index()] = 100.0;
        for _ in 0..360 {
            m.step();
        }
        m.c.pairs[Species::Springturf.index()][Species::Vaulttree.index()]
    };
    assert!(go(&base) > 0.0);
    let mut off = base.clone();
    off.species_mut(Species::Vaulttree).overtop_per_check = 0.0;
    assert_eq!(go(&off), 0.0);
}

/// **Config flow**: the glowcap's foliage (cap) regrowth rate sets how fast grazed caps
/// come back, in the law's direction.
#[test]
fn cap_regrowth_follows_the_foliage_rate() {
    let (p, base) = resolved();
    let caps = |fc: &FloraConfig| {
        let (zones, faces) = one_zone(fc, 50, 0.3, 1.0, &[(Species::Glowcap, 1.0, 50)]);
        let mut m = build(fc, &p, zones, &faces, params());
        m.z[0].litter = 50.0;
        for c in m.z[0].cohorts[Species::Glowcap.index()].iter_mut() {
            c.st.foliage = 0.0;
        }
        for _ in 0..180 {
            m.step();
        }
        m.z[0].cohorts[Species::Glowcap.index()].iter().map(|c| c.n * c.st.foliage).sum::<f64>()
    };
    let mut fast = base.clone();
    fast.species_mut(Species::Glowcap).foliage_rate *= 4.0;
    assert!(caps(&fast) > caps(&base));
    let _ = model::NS;
}
