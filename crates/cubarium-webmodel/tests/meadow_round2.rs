//! **Meadow round 2** (`design/webmodel/meadow.md` §6–7): the diagnosis ledger and the new
//! levers, on hand-built inputs, against the config on hand. No tuned number is pinned.
//!
//! - the per-zone diagnosis ledger (`model::ZoneDiag`) adds up to the half's own counters;
//! - lever g, seed rain: the bank gains exactly the rain over the open-sky drained zones and
//!   nothing elsewhere, in proportion to the rate, and the ledgers close with the import;
//! - (the bank-life lever is the two sim knobs `seed_max_age_s` and `seed_attrition_per_s`; that
//!   the bank's one rate follows both is `independent_flora::config_flow_the_bank_follows_both_of_its_knobs`);
//! - the runner lever (`clonal_share`, a sim knob): more clonal share, never fewer runner births.

use std::path::Path;

use cubarium_voxel_flora::{FloraConfig, Species, Trophic};
use cubarium_webmodel::flora::census::{Face, StandRec};
use cubarium_webmodel::flora::climate::DayModel;
use cubarium_webmodel::flora::config;
use cubarium_webmodel::flora::kernel::{Kernel, RunnerBox};
use cubarium_webmodel::flora::meadow::Meadow;
use cubarium_webmodel::flora::model::{Model, NS, Params, Weather};
use cubarium_webmodel::flora::stand::package_of;
use cubarium_webmodel::flora::zones::{self, Reading, Zones};

const TURF: Species = Species::Springturf;
const LB: Species = Species::Lanternberry;

fn resolved() -> (cubarium_voxel_sim::params::Params, FloraConfig) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/scenarios/terrarium.toml");
    let p = config::resolve(&root, &[]).expect("the terrarium resolves");
    let f = config::flora_of(&p);
    (p, f)
}

/// Faces `x0..x0+n` at sky `sky`, mineral `n0`, the first held by `occupants` (species, wood
/// as a share of `wood_max`, count); `wet` marks them waterlogged for the zone key.
fn faces(fc: &FloraConfig, x0: u32, n: u32, sky: f64, n0: f64, occupants: &[(Species, f64, u32)], wet: bool) -> Vec<Face> {
    let mut out: Vec<Face> = (x0..x0 + n).map(|x| Face::bare(x, 1, 0, sky, n0)).collect();
    if wet {
        let sat = fc.species(TURF).establish_saturated_max + 0.1;
        for f in &mut out {
            f.turf_saturated = sat.min(1.0);
            f.saturated = [sat.min(1.0); 8];
        }
    }
    let mut i = 0usize;
    for &(sp, frac, count) in occupants {
        let sc = fc.species(sp);
        for _ in 0..count {
            let w = sc.wood_max * frac;
            let rec = StandRec { wood: w, foliage: sc.alpha * w * 0.6, reserve: sc.reserve_cap * w * 0.5, light: 1.0, moisture: 1.0, ..Default::default() };
            if sc.trophic == Trophic::Saprotroph {
                out[i].fungus = Some(rec);
            } else {
                out[i].plant = Some((sp, rec));
            }
            i += 1;
        }
    }
    out
}

fn zones_of(fc: &FloraConfig, faces: &[Face], water: f64) -> Zones {
    let mut z = zones::build(fc, &[faces.to_vec()]);
    let r: [Reading; NS] = std::array::from_fn(|s| {
        let mu = fc.species(Species::ALL[s]).moisture_at(water);
        Reading { mu, available: water, saturated: 0.0, stress_target: 0.0, gate: 1.0, mu_held: mu, stress_held: 0.0, available_held: water }
    });
    for zone in &mut z.zones {
        zone.weather = vec![r];
        zone.fungal_space = 1.0;
    }
    z
}

/// Kernels that send a share `cross` of every zone's seed to the next zone, the rest home.
fn kernels(nz: usize, cross: f64) -> Vec<Kernel> {
    (0..NS)
        .map(|_| Kernel {
            land: (0..nz).map(|i| (0..nz).map(|j| if nz == 1 { 1.0 } else if i == j { 1.0 - cross } else if j == (i + 1) % nz { cross } else { 0.0 }).collect()).collect(),
            lost: vec![0.0; nz],
        })
        .collect()
}

fn build(fc: &FloraConfig, p: &cubarium_voxel_sim::params::Params, faces: &[Face], kern: Vec<Kernel>, start_minute: f64, mp: Params, water: f64) -> Model {
    let z = zones_of(fc, faces, water);
    let rb = (0..NS).map(|_| RunnerBox { faces: 8.0, same_zone: 1.0 }).collect();
    Model::new(fc.clone(), DayModel::of(&p.world, fc.upkeep_q10), z, kern, rb, faces, start_minute, mp)
}

fn frozen(meadow: Meadow) -> Params {
    Params { weather: Weather::Frozen, dt: 10.0, meadow, diag: true, ..Params::default() }
}

/// A noon start minute.
fn noon(dm: &DayModel) -> f64 {
    let day = dm.day_s();
    (0..1440).map(|k| k as f64 * day / 1440.0).max_by(|a, b| dm.at(*a, 1.0).daylight.total_cmp(&dm.at(*b, 1.0).daylight)).unwrap() / 60.0
}

fn steps(m: &mut Model, days: f64) {
    for _ in 0..(days * m.day.day_s() / m.p.dt).round() as usize {
        m.step();
    }
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-12)
}

/// A two-zone world (a bright drained one and a dim one) with turf, pad and lanternberry,
/// banked seed and vacant faces, seed crossing between the zones.
fn two_zone(fc: &FloraConfig) -> Vec<Face> {
    let mut f = faces(fc, 0, 200, 0.95, 1.0, &[(TURF, 1.0, 80), (Species::Velvetpad, 1.0, 30), (LB, 1.0, 10)], false);
    f.extend(faces(fc, 200, 200, 0.5, 0.9, &[(TURF, 0.5, 40), (LB, 1.0, 30)], false));
    for (i, x) in f.iter_mut().enumerate() {
        x.seeds[TURF.index()] = 20;
        x.seeds[LB.index()] = u32::from(i % 3 == 0) * 2;
    }
    f
}

#[test]
fn the_zone_ledger_adds_up_to_the_counters() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let f = two_zone(&fc);
    let mut m = build(&fc, &p, &f, kernels(2, 0.3), noon(&dm), frozen(Meadow { contest_escape: -1.0, contest_tau: -1.0, ..Meadow::default() }), 0.6);
    assert!(m.z.len() >= 2, "fixture: two zones");
    steps(&mut m, 1.0);
    let c = &m.c;
    for s in [TURF, LB, Species::Velvetpad].map(|s| s.index()) {
        let sum = |g: fn(&cubarium_webmodel::flora::model::ZoneDiag) -> [f64; NS]| c.zdiag.iter().map(|d| g(d)[s]).sum::<f64>();
        assert!(close(sum(|d| d.born_vacant) + sum(|d| d.born_cover), c.births_seed[s], 1e-9), "seed births {s}");
        assert!(close(sum(|d| d.born_runner), c.births_runner[s], 1e-9), "runner births {s}");
        assert!(close(sum(|d| d.landed), c.seeds_landed[s], 1e-9), "landed {s}");
        assert!(close(sum(|d| d.bank_died), c.seeds_died[s], 1e-9), "bank deaths {s}");
        assert!(close(sum(|d| d.starved), c.starved[s] + c.contest_died[s], 1e-9), "starved {s}");
        assert!(close(sum(|d| d.drought), c.drought[s], 1e-9), "drought {s}");
        assert!(close(sum(|d| d.overtopped), c.overtopped[s], 1e-9), "overtopped {s}");
        assert!(close(sum(|d| d.displaced), c.displaced[s], 1e-9), "displaced {s}");
        assert!(close(sum(|d| d.under_starved), c.starved_under[s], 1e-9), "under {s}");
        // Seeds sent = landed somewhere + lost (the kernels lose none here).
        assert!(close(sum(|d| d.seeds_out), sum(|d| d.landed) + sum(|d| d.lost), 1e-9), "seed flow {s}");
    }
    let t = TURF.index();
    assert!(c.births_seed[t] + c.births_runner[t] > 0.0 && c.seeds_landed[t] > 0.0, "fixture: turf reproduced");
    // Seed crossed: the landed-from-self share is below the landed total somewhere.
    assert!(c.zdiag.iter().any(|d| d.landed_self[t] < d.landed[t]), "fixture: seed crossed zones");
    // The funnel narrows: seeded ≥ gates ≥ canopy, and it ran by day.
    for d in &c.zdiag {
        assert!(d.f_time > 0.0);
        assert!(d.f_seeded[t] + 1e-9 >= d.f_gates[t] && d.f_gates[t] + 1e-9 >= d.f_canopy[t], "{:?} {:?} {:?}", d.f_seeded[t], d.f_gates[t], d.f_canopy[t]);
    }
}

fn ledger_residuals(m: &mut Model, days: f64) -> (f64, f64) {
    let o0 = m.organic();
    let (a, b, c) = m.mineral();
    let n0 = a + b + c;
    steps(m, days);
    let k = &m.c;
    let taken = k.taken.litter + k.taken.foliage.iter().sum::<f64>() + k.taken.carrion + k.taken.film + k.taken.nectar + k.taken.fruit;
    let expect = o0 + k.fixed + k.seep + k.returned + k.rain_in - k.respired - k.decomposed - taken;
    let (a, b, c) = m.mineral();
    let n1 = a + b + c + k.taken_mineral - k.held_in - k.rain_mineral;
    ((m.organic() - expect) / o0, (n1 - n0) / n0)
}

#[test]
fn seed_rain_lands_only_on_open_drained_ground_in_proportion_and_conserves() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let mut f = faces(&fc, 0, 100, 0.95, 1.0, &[], false);
    f.extend(faces(&fc, 100, 100, 0.95, 1.0, &[], true));
    let t = TURF.index();
    let run = |rate: f64| {
        let mut mw = Meadow::default();
        mw.rain_per_face_s[t] = rate;
        let mut m = build(&fc, &p, &f, kernels(2, 0.0), noon(&dm), frozen(mw), 0.6);
        // One step at night, so nothing germinates: the bank moves by rain and decay only.
        let night = (0..1440).map(|k| k as f64 * dm.day_s() / 1440.0).find(|&s| dm.at(s, 1.0).daylight <= 0.0).unwrap();
        m.t_s = night;
        let b0: Vec<f64> = m.z.iter().map(|z| z.bank[t]).collect();
        m.step();
        (m, b0)
    };
    let (m0, b00) = run(0.0);
    let rate = 1e-3;
    let (m1, b10) = run(rate);
    let (m2, _) = run(2.0 * rate);
    let dt = m1.p.dt;
    let mut saw_wet = false;
    for (zi, z) in m1.zones.zones.iter().enumerate() {
        let gain = m1.z[zi].bank[t] - m0.z[zi].bank[t] - (b10[zi] - b00[zi]);
        let gain2 = m2.z[zi].bank[t] - m0.z[zi].bank[t];
        if z.key.wet {
            saw_wet = true;
            assert!(gain.abs() < 1e-12, "no rain on a wet zone");
        } else {
            // The rain lands before the bank's decay step: a step's decay of it is the only gap.
            let sc = fc.species(TURF);
            let life = { let (ra, tm) = (sc.seed_attrition_per_s, sc.seed_max_age_s.max(1.0)); if ra > 0.0 { (1.0 - (-ra * tm).exp()) / ra } else { tm }.max(1.0) };
            let want = rate * z.area * dt * (-dt / life).exp();
            assert!(close(gain, want, 1e-9), "zone {zi}: {gain} vs {want}");
            assert!(close(gain2, 2.0 * gain, 1e-9), "rain in proportion to its rate");
        }
    }
    assert!(saw_wet, "fixture: a wet zone");
    // The ledgers close with the import, over a day with everything growing.
    let mut f2 = two_zone(&fc);
    f2.extend(faces(&fc, 400, 50, 0.95, 1.0, &[], true));
    let mut mw = Meadow::default();
    mw.rain_per_face_s[t] = rate;
    mw.rain_per_face_s[LB.index()] = rate;
    let nz = zones::build(&fc, &[f2.clone()]).zones.len();
    let mut m = build(&fc, &p, &f2, kernels(nz, 0.2), noon(&dm), frozen(mw), 0.6);
    let (o, n) = ledger_residuals(&mut m, 1.0);
    assert!(m.c.rain_in > 0.0);
    assert!(o.abs() <= 1e-8, "organic residual {o:e}");
    assert!(n.abs() <= 1e-8, "mineral residual {n:e}");
    let _ = package_of(fc.species(TURF));
}

#[test]
fn more_clonal_share_never_fewer_runner_births() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let f = faces(&fc, 0, 300, 0.95, 1.0, &[(TURF, 1.0, 100)], false);
    let t = TURF.index();
    let births = |share: f64| {
        let mut c = fc.clone();
        c.species_mut(TURF).clonal_share = share;
        let mut m = build(&c, &p, &f, kernels(1, 0.0), noon(&dm), frozen(Meadow::default()), 0.6);
        steps(&mut m, 0.5);
        m.c.births_runner[t]
    };
    let base = fc.species(TURF).clonal_share;
    let (a, b) = (births(base), births((base + 1.0) / 2.0));
    assert!(a > 0.0, "fixture: turf sends runners");
    assert!(b >= a, "clonal share {base} -> {}: runner births {a} -> {b}", (base + 1.0) / 2.0);
    assert_eq!(births(0.0), 0.0, "no clonal share, no runners");
}
