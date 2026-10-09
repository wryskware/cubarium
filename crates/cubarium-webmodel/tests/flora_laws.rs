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

/// **Law anchor, curing** (DORMANCY): the model's curing clock is the sim's (`cure_step`,
/// on the budget and below `cure_water`). A springturf adult on a dry root box — short on
/// its water at its light, above `cure_water` — then rewetted to `cure_resume_water`: the
/// model cures and wakes on the same ticks as `study::probe` and ends on the same stocks.
#[test]
fn the_curing_clock_is_the_sims_through_a_cure_and_a_wake() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let sc = fc.species(Species::Springturf);
    assert!(sc.cure_after_s > 0.0 && sc.cure_cover > 0.0, "fixture: turf cures on the budget");
    let t0 = 0.2 * dm.day_s();
    let ticks = (0.6 * dm.day_s() / cubarium_voxel::DT) as usize;
    let wet_at = ticks / 2;
    let dry = (sc.cure_water + sc.cure_resume_water) / 2.0;
    let water = |k: usize| if k < wet_at { dry } else { sc.cure_resume_water };
    let held = |k: usize| Held { water: water(k), saturated: 0.0, light: 0.9, mineral: 0.6 };
    let clim = clock(&dm, t0, ticks);
    let s0 = start_stand(&fc, Species::Springturf, Start::Adult);
    let mut sim_cured = vec![false; ticks];
    let o = cubarium_voxel_flora::study::probe_traced(&fc, s0, &clim, held, |k, s| sim_cured[k] = s.cured);
    assert!(o.alive, "fixture: it lives through the window");
    assert!(sim_cured.iter().any(|&c| c), "fixture: the sim's stand cured");
    assert!(!sim_cured[ticks - 1], "fixture: and woke");
    let mut st = StandState { wood: s0.wood, foliage: s0.foliage, reserve: s0.reserve, ..StandState::default() };
    for k in 0..ticks {
        let h = held(k);
        let site = stand::Site {
            light_eff: light_response(sc, h.light),
            available: h.water,
            saturated: 0.0,
            mineral: h.mineral,
            darkness: 0.0,
            substrate_taken: 0.0,
            substrate_gross: 0.0,
        };
        let c = dm.at(t0 + k as f64 * cubarium_voxel::DT, cubarium_voxel::DT);
        let (_, d) = stand::grow(sc, &mut st, site, c);
        assert!(d.is_none(), "model: alive at tick {k}");
        stand::fund(sc, &mut st, cubarium_voxel::DT);
        assert_eq!(st.cured, sim_cured[k], "cured at tick {k}");
    }
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()));
    assert!(close(st.wood, o.wood[1]) && close(st.foliage, o.foliage[1]) && close(st.reserve, o.reserve[1]), "stocks: {st:?} vs {o:?}");
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
    // The zone as the model builds it from a census of these faces (its newborn samples
    // among them), with the weather held: every gate open, water at field capacity.
    let mut zones = cubarium_webmodel::flora::zones::build(fc, &[faces.clone()]);
    assert_eq!(zones.zones.len(), 1, "one zone");
    let r = Reading { mu: 1.0, available: 1.0, saturated: 0.0, stress_target: 0.0, gate: 1.0, mu_held: 1.0, stress_held: 0.0, available_held: 1.0 };
    let zone: &mut Zone = &mut zones.zones[0];
    zone.weather = vec![[r; Species::COUNT]];
    zone.fungal_space = 1.0;
    let _ = Key { soil: true, under_roof: false, band: 4, wet: false };
    (zones, faces)
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

#[test]
fn a_required_substrate_fungus_gets_no_carbon_from_dark_damp() {
    let (p, mut fc) = resolved();
    fc.glowcap.maintenance = 0.1;
    fc.glowcap.dark_uptake_per_s = 1.0;
    fc.glowcap.foliage_rate = 0.0;
    fc.glowcap.wood_rate = 0.0;
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let site = stand::Site { light_eff: 0.0, available: 1.0, saturated: 0.0,
        mineral: 1.0, darkness: 1.0, substrate_taken: 0.0, substrate_gross: 0.0 };
    for required in [true, false] {
        fc.glowcap.substrate_required = required;
        let sc = &fc.glowcap;
        let mut st = StandState { wood: sc.wood_max, foliage: sc.alpha*sc.wood_max, ..Default::default() };
        let (flows, _) = stand::grow(sc, &mut st, site, dm.at(0.0, cubarium_voxel::DT));
        if required { assert_eq!(flows.seep, 0.0); assert!(flows.litter > 0.0); }
        else { assert!(flows.seep > 0.0); }
    }
}

#[test]
fn the_model_bounds_fungal_uptake_by_upkeep_and_offers_cropped_mycelium() {
    let (p, mut fc) = resolved();
    fc.glowcap.substrate_required = true;
    fc.glowcap.substrate_budgeted = true;
    fc.glowcap.mycelium_graze_fraction = 0.6;
    fc.glowcap.substrate_uptake_per_s = 10.0;
    fc.glowcap.foliage_rate = 0.0;
    fc.glowcap.wood_rate = 0.0;
    fc.glowcap.propagule_rate = 0.0;
    fc.glowcap.senescence = 0.0;
    let (zones, faces) = one_zone(&fc, 1, 0.8, 1.0, &[(Species::Glowcap,1.0,1)]);
    let mut mp = params(); mp.dt = cubarium_voxel::DT;
    let mut m = build(&fc, &p, zones, &faces, mp);
    let k = Species::Glowcap.index();
    let sc = fc.glowcap.clone();
    let c = &mut m.z[0].cohorts[k][0];
    c.st.foliage = sc.alpha*c.st.wood;
    c.st.reserve = sc.reserve_cap*c.st.wood;
    m.z[0].litter = 1.0;
    let expected = sc.maintenance * m.day.at(m.minute()*60.0, m.p.dt).upkeep * sc.wood_max * m.p.dt / sc.substrate_yield;
    m.step();
    assert!((m.c.substrate-expected).abs()<1e-10, "{} vs {expected}",m.c.substrate);
    let c = &mut m.z[0].cohorts[k][0];
    let before = c.st.wood;
    let mycelium = sc.mycelium_graze_fraction*before;
    let cap = mycelium/4.0;
    c.st.foliage = sc.graze_refuge*sc.alpha*before + cap;
    assert!((m.offer().foliage[k] - (cap+mycelium)).abs()<1e-10);
    let mut take = cubarium_webmodel::exchange::Take::default(); take.foliage[k] = cap+mycelium/2.0;
    m.take_from(&take);
    assert!((m.z[0].cohorts[k][0].st.wood-(before-mycelium/2.0)).abs()<1e-10);
    let c = &mut m.z[0].cohorts[k][0];
    c.st.foliage = sc.graze_refuge*sc.alpha*c.st.wood;
    let before = c.st.wood;
    let offer = sc.mycelium_graze_fraction*before;
    assert!((m.offer().foliage[k]-offer).abs()<1e-10);
    let mut take = cubarium_webmodel::exchange::Take::default(); take.foliage[k] = offer;
    m.take_from(&take);
    assert!((m.z[0].cohorts[k][0].st.wood-(before-offer)).abs()<1e-10);
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
    let (p, mut base) = resolved();
    // Isolate cap regrowth; the sun law has its own lit-versus-shade fixture.
    base.glowcap.sun_cap_loss_per_s = 0.0;
    base.glowcap.sun_mycelium_loss_per_s = 0.0;
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

/// **Numerics, the death rule's edge**: two newborns at `alive_min`, merged in any shares,
/// stay at or above `alive_min` (a weighted mean that rounds below its inputs would be killed
/// by the sim's own death rule `W < alive_min`), and the merged stand survives a daylight
/// step on a good site.
#[test]
fn merged_newborns_never_round_below_alive_min() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    for sp in [Species::Velvetpad, Species::Springturf, Species::Lanternberry] {
        let sc = fc.species(sp);
        let nb = StandState::newborn(sc, stand::package_of(sc));
        for i in 1..200 {
            let wa = f64::from(i) * 0.37 + 0.013;
            let mut a = model::Cohort { n: wa, st: nb, covered: false, opened_s: 0.0, site: model::Site::default() };
            let b = model::Cohort { n: 1.0 / wa + 0.1, st: nb, covered: false, opened_s: 0.0, site: model::Site::default() };
            model::merge(&mut a, &b);
            assert!(a.st.wood >= sc.alive_min, "{sp:?}: merged W {} under alive_min {}", a.st.wood, sc.alive_min);
        }
        // A noon step on a bright, wet, rich site: no death.
        let noon = (0..720).map(|k| dm.at(k as f64 * 10.0, 10.0)).find(|c| c.daylight >= 1.0).expect("a full-daylight step");
        let mut st = nb;
        let site = stand::Site { light_eff: light_response(sc, 1.0), available: sc.full_water, saturated: 0.0, mineral: 1.0, darkness: 0.0, substrate_taken: 0.0, substrate_gross: 0.0 };
        assert!(stand::grow(sc, &mut st, site, noon).1.is_none(), "{sp:?} newborn dies on a good site");
    }
}

/// **Fitted (spatial), the seed shadow**: `seeded_share` is the negative binomial's
/// `P(≥1 seed)`: it rises with the seeds per face, falls as the clumping `k` falls, and is
/// the Poisson `1 − e^−m` in the well-mixed limit; and the clumping fit, on a census whose
/// seeds pile on a few faces, returns a `k` whose share is the census's.
#[test]
fn the_seed_shadow_fit_returns_the_census_share() {
    use cubarium_webmodel::flora::fit::{clumping, clumping_by_zone, seeded_share};
    for m in [0.01, 0.3, 1.0, 4.0] {
        assert!((seeded_share(m, f64::INFINITY) - (1.0 - (-m as f64).exp())).abs() < 1e-12);
        assert!(seeded_share(m, 0.05) < seeded_share(m, 1.0) && seeded_share(m, 1.0) < seeded_share(m, 100.0));
        assert!(seeded_share(m, 1.0) < seeded_share(m * 2.0, 1.0));
    }
    let (_, fc) = resolved();
    // 400 bare faces; 30 of them hold 10 lanternberry seeds each (a bank of 300 on 30 faces).
    let (_, mut faces) = one_zone(&fc, 400, 0.85, 1.0, &[]);
    let k = Species::Lanternberry.index();
    for f in faces.iter_mut().take(30) {
        f.seeds[k] = 10;
    }
    let zones = cubarium_webmodel::flora::zones::build(&fc, &[faces.clone()]);
    let observed = 30.0 / 400.0;
    let g = clumping(&zones);
    let z = clumping_by_zone(&zones, &g);
    let fitted = seeded_share(300.0 / 400.0, z[0][k]);
    // The fit's grid is log-spaced (a step of 10^(1/12)): its share is the census's to the
    // grid's resolution.
    assert!((fitted - observed).abs() < 0.25 * observed, "fitted share {fitted} vs census {observed}");
    assert!(fitted < 1.0 - (-0.75f64).exp(), "a piled bank covers fewer faces than a mixed one");
}

/// **A water seed** (`Dispersal::Water`, `seeds::water_target`) lands only on a margin its
/// gates pass: with the zone's gate share at zero no siphonreed comes up, with it open some
/// do, from the same bank.
#[test]
fn a_water_seed_comes_up_only_where_its_gates_pass() {
    let (p, fc) = resolved();
    let sp = Species::Siphonreed;
    assert_eq!(fc.species(sp).dispersal, cubarium_voxel_flora::Dispersal::Water, "the preset's siphonreed floats");
    let k = cubarium_webmodel::flora::census::PHOTO.iter().position(|&s| s == sp).unwrap();
    let sc = fc.species(sp);
    let births = |gate: f64| {
        let (_, mut faces) = one_zone(&fc, 200, 0.95, 1.0, &[]);
        for f in faces.iter_mut() {
            f.water[k] = Some(sc.full_water + 0.1);
        }
        let mut zones = cubarium_webmodel::flora::zones::build(&fc, &[faces.clone()]);
        let mut r = zones.zones[0].weather[0][sp.index()];
        r.gate = gate;
        r.mu = 1.0;
        r.mu_held = 1.0;
        r.available_held = sc.full_water + 0.1;
        zones.zones[0].weather[0][sp.index()] = r;
        let mut m = build(&fc, &p, zones, &faces, params());
        m.z[0].bank[sp.index()] = 50.0;
        for _ in 0..360 {
            m.step();
        }
        m.c.births_seed[sp.index()]
    };
    assert_eq!(births(0.0), 0.0);
    assert!(births(1.0) > 0.0);
}

/// **Config flow, the canopy gate**: raising a woody species' `establish_light_margin` (the
/// sim's knob: the newborn must earn this many times its upkeep) never lets more of its
/// seedlings up through a shaded mat, and lets fewer when some came up.
#[test]
fn the_canopy_margin_gates_the_seedlings() {
    let (p, base) = resolved();
    let sp = Species::Lanternberry;
    let k = cubarium_webmodel::flora::census::PHOTO.iter().position(|&s| s == sp).unwrap();
    let go = |fc: &FloraConfig| {
        let (_, mut faces) = one_zone(fc, 300, 0.9, 1.0, &[(Species::Springturf, 1.0, 300)]);
        for (i, f) in faces.iter_mut().enumerate() {
            f.seeds[sp.index()] = 2;
            // A spread of shade from the mat's neighbours over the newborn's crown top.
            f.nb_tau[k][Species::Springturf.index()] = 0.2 + 1.2 * (i as f32 / 300.0);
        }
        let zones = cubarium_webmodel::flora::zones::build(fc, &[faces.clone()]);
        let mut m = build(fc, &p, zones, &faces, params());
        m.z[0].bank[sp.index()] = 600.0;
        for _ in 0..360 {
            m.step();
        }
        m.c.births_seed[sp.index()]
    };
    let b0 = go(&base);
    assert!(b0 > 0.0, "some seedlings come up under the base margin");
    let mut strict = base.clone();
    strict.species_mut(sp).establish_light_margin *= 2.0;
    assert!(go(&strict) < b0, "a stricter margin lets fewer up");
}

/// **Scenario lever**: holding a species' site mineral keeps its pool at the start mean on
/// its faces (the top-up is booked as `held_in`), so its stands never read a drained site.
#[test]
fn the_hold_lever_tops_the_pool_up_to_the_start_mean() {
    let (p, fc) = resolved();
    let sp = Species::Springturf;
    let (zones, faces) = one_zone(&fc, 200, 0.9, 0.5, &[(sp, 0.3, 150)]);
    let mut mp = params();
    mp.hold_mineral[sp.index()] = true;
    let mut m = build(&fc, &p, zones, &faces, mp);
    let start = m.start_mineral[0][sp.index()];
    for _ in 0..720 {
        m.step();
        let n = m.z[0].count(sp.index());
        assert!(m.z[0].pool[sp.index()] >= n * start - 1e-9, "the held pool fell under the start mean");
    }
    assert!(m.c.held_in >= 0.0);
}
