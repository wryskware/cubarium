//! **The independent test pass on the producer half** (`design/webmodel/flora.md`), written
//! from the sim's own code (`crates/cubarium-voxel-flora/src/step.rs`, `seeds.rs`,
//! `study.rs`), not from the model's. The sim side is the sim's public probe
//! (`study::probe`, its own `grow_stand` on a one-stand fixture), a hand-built world
//! stepped by the sim itself, or the law as the sim's code states it; every expectation is
//! computed from the config on hand.
//!
//! Tests marked `#[ignore = "BUG: …"]` or `#[ignore = "DISAGREES: …"]` found a defect or a
//! disagreement with the sim and are left failing on purpose (the brief: don't change model
//! code).

use std::path::Path;

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::study::{ClimateSample, Held, Start, probe, start_stand};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site as FSite, Species, Trophic};
use cubarium_webmodel::flora::census::{self, Face, PHOTO, StandRec};
use cubarium_webmodel::flora::climate::DayModel;
use cubarium_webmodel::flora::kernel::{Kernel, RunnerBox};
use cubarium_webmodel::flora::model::{self, Cohort, Model, NS, Params, Weather};
use cubarium_webmodel::flora::stand::{self, StandState, light_response, package_of};
use cubarium_webmodel::flora::zones::{self, Reading, Zones};
use cubarium_webmodel::flora::config;

fn resolved() -> (cubarium_voxel_sim::params::Params, FloraConfig) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/scenarios/terrarium.toml");
    let p = config::resolve(&root, &[]).expect("the terrarium resolves");
    let f = config::flora_of(&p);
    (p, f)
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-300)
}

// ===================================================================================
// 1. The stand law across a whole day and night
// ===================================================================================

/// The cases: species, start, held conditions (light raw, before the response).
fn stand_cases() -> Vec<(Species, Start, Held)> {
    vec![
        // Grazed to a tenth of its foliage with a full reserve: it reflushes by day and
        // under the night budget (`step.rs:3093`), and makes nectar at night.
        (Species::Bloomcrown, Start::Adult, Held { water: 0.6, saturated: 0.0, light: 0.6, mineral: 1.0 }),
        (Species::Springturf, Start::Newborn, Held { water: 0.6, saturated: 0.0, light: 0.9, mineral: 1.0 }),
        (Species::Springturf, Start::Adult, Held { water: 0.25, saturated: 0.3, light: 0.6, mineral: 0.5 }),
        (Species::Velvetpad, Start::Newborn, Held { water: 0.8, saturated: 0.5, light: 0.7, mineral: 0.8 }),
        (Species::Vaulttree, Start::Adult, Held { water: 0.5, saturated: 0.0, light: 1.0, mineral: 1.0 }),
        (Species::Umbrellafrond, Start::Newborn, Held { water: 0.7, saturated: 0.1, light: 0.5, mineral: 0.9 }),
        (Species::Bloomcrown, Start::Adult, Held { water: 0.6, saturated: 0.0, light: 0.8, mineral: 1.0 }),
    ]
}

/// The first tick of the night (daylight reaches 0).
fn dusk(dm: &DayModel) -> f64 {
    let dt = cubarium_voxel::DT;
    let n = (dm.day_s() / dt) as usize;
    (1..n).map(|k| k as f64 * dt).find(|&t| dm.at(t, dt).daylight <= 0.0 && dm.at(t - dt, dt).daylight > 0.0).expect("fixture: a dusk")
}

/// Case `i`'s first stand: the sim's own `start_stand`, the first case grazed.
fn first_stand(fc: &FloraConfig, i: usize, sp: Species, start: Start) -> cubarium_voxel_flora::Stand {
    let mut s = start_stand(fc, sp, start);
    if i == 0 {
        let sc = fc.species(sp);
        s.foliage = 0.1 * sc.alpha * s.wood;
        s.reserve = 0.9 * sc.reserve_cap * s.wood;
        s.mineral = sc.n_tissue * (s.wood + s.foliage + s.reserve);
        s.bin_layers(fc);
    }
    s
}

/// The model's stand law stepped at `dt` from `t0` for `secs`, held site: (W, P, Q, fixed,
/// packages, died).
fn model_stand(fc: &FloraConfig, dm: &DayModel, sp: Species, s0: &cubarium_voxel_flora::Stand, held: Held, t0: f64, secs: f64, dt: f64) -> (f64, f64, f64, f64, u32, bool) {
    let sc = fc.species(sp);
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
    let (mut fixed, mut pk) = (0.0, 0u32);
    let steps = (secs / dt).round() as usize;
    for k in 0..steps {
        let c = dm.at(t0 + k as f64 * dt, dt);
        let (fl, d) = stand::grow(sc, &mut st, site, c);
        fixed += fl.income;
        if d.is_some() {
            return (st.wood, st.foliage, st.reserve, fixed, pk, true);
        }
        pk += stand::fund(sc, &mut st, dt).0;
    }
    (st.wood, st.foliage, st.reserve, fixed, pk, false)
}

/// **The stand law over a whole world day — day, dusk, night, dawn — against the sim's own
/// `grow_stand`** (`study::probe`, one stand stepped per tick at the world clock's samples,
/// `step.rs:2924-3260`). At the sim's tick the model's law matches to rounding for every
/// case: wood, foliage, reserve and packages funded, or the death; gross fixation to 1e-5.
#[test]
fn the_stand_law_is_the_sims_over_a_whole_day_and_night() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let day = dm.day_s();
    let ticks = (day / cubarium_voxel::DT).round() as usize;
    // From nightfall: the night first (a grazed stand reflushes on its night budget), then
    // dawn, the day and dusk.
    let t0 = dusk(&dm);
    let clim: Vec<ClimateSample> = (0..ticks)
        .map(|k| {
            let c = dm.at(t0 + k as f64 * cubarium_voxel::DT, cubarium_voxel::DT);
            ClimateSample { daylight: c.daylight, upkeep: c.upkeep, night: c.night, to_dawn_s: c.to_dawn_s }
        })
        .collect();
    assert!(clim.iter().any(|c| c.daylight >= 1.0) && clim.iter().any(|c| c.daylight <= 0.0), "fixture: a full day and a full night");
    for (i, (sp, start, held)) in stand_cases().into_iter().enumerate() {
        let s0 = first_stand(&fc, i, sp, start);
        let o = probe(&fc, s0, &clim, |_| held);
        let (w, f, q, fixed, pk, died) = model_stand(&fc, &dm, sp, &s0, held, t0, day, cubarium_voxel::DT);
        assert_eq!(died, !o.alive, "{sp:?} {start:?}: death");
        if o.alive {
            let c = |a: f64, b: f64| (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()));
            assert!(c(w, o.wood[1]) && c(f, o.foliage[1]) && c(q, o.reserve[1]), "{sp:?} {start:?}: W/P/Q {w} {f} {q} vs sim {} {} {}", o.wood[1], o.foliage[1], o.reserve[1]);
            // Gross fixation is a booked sum over 144,000 ticks; on the velvetpad newborn it
            // drifts from the sim's by 7e-7 relative from tick ~45,000 on while every stock
            // stays exact (a booking at the demand cap, not the law): 1e-5 on it.
            assert!((fixed - o.fixed).abs() <= 1e-5 * o.fixed.abs().max(1e-12), "{sp:?} {start:?}: fixed {fixed} vs sim {}", o.fixed);
            assert_eq!(pk, o.packages, "{sp:?} {start:?}: packages");
        }
    }
}

/// **The law at the model's own step.** The model runs at `Params::default().dt` (10 s), the
/// sim's plant clock at `step_every · DT`. Over a whole day the model's stand at its own step
/// stays within 2 % of the sim's per-tick stand in wood, foliage, reserve and fixation for
/// every case that lives (and the same cases die).
///
/// It does not: the explicit step's error grows in proportion to `dt` (measured over the
/// day, relative to the sim: dt 0.2 s / 1 s / 10 s — adult springturf on a dry, stressed
/// site foliage −1.3 % / −8.0 % / −56 %, wood −0.2 % / −1.5 % / −14 %; springturf newborn
/// reserve −0.3 % / −2.1 % / −22 %; bloomcrown adult reserve −0.2 % / −1.2 % / −12 %;
/// velvetpad newborn reserve −8 % at 10 s). Every case errs low.
#[test]
#[ignore = "DISAGREES: at the model's default dt (10 s) the stand law drifts from the sim's per-tick law over a day, error ∝ dt, always low (worst: dry adult springturf foliage −56 %, wood −14 %; reserves −8 % to −22 %)"]
fn the_stand_law_at_the_models_own_step_tracks_the_sim_over_a_day() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let day = dm.day_s();
    let ticks = (day / cubarium_voxel::DT).round() as usize;
    let clim: Vec<ClimateSample> = (0..ticks)
        .map(|k| {
            let c = dm.at(k as f64 * cubarium_voxel::DT, cubarium_voxel::DT);
            ClimateSample { daylight: c.daylight, upkeep: c.upkeep, night: c.night, to_dawn_s: c.to_dawn_s }
        })
        .collect();
    let dt = Params::default().dt;
    let mut worst = (0.0f64, String::new());
    for (i, (sp, start, held)) in stand_cases().into_iter().enumerate() {
        let s0 = first_stand(&fc, i, sp, start);
        let o = probe(&fc, s0, &clim, |_| held);
        let (w, f, q, fixed, _, died) = model_stand(&fc, &dm, sp, &s0, held, 0.0, day, dt);
        assert_eq!(died, !o.alive, "{sp:?} {start:?}: death at dt {dt}");
        if o.alive {
            for (name, a, b) in [("wood", w, o.wood[1]), ("foliage", f, o.foliage[1]), ("reserve", q, o.reserve[1]), ("fixed", fixed, o.fixed)] {
                let e = (a - b).abs() / b.abs().max(1e-12);
                if e > worst.0 {
                    worst = (e, format!("{sp:?} {start:?} {name}: model {a} sim {b}"));
                }
            }
        }
    }
    assert!(worst.0 <= 0.02, "worst {:.2} %: {}", 100.0 * worst.0, worst.1);
}

// ===================================================================================
// 2. The canopy gate on the newborn's own light, against the sim's gate
// ===================================================================================

const VOXEL_M: f64 = 0.125;
const SLAB_W: u32 = 14;
/// The shade made deep enough that a crown's neighbours fall well under the margin (the
/// sim's own canopy tests make it total; a gradient is kept here).
const SHADE: f64 = 25.0;
/// The turf's assimilation raised so the covers (and the margin) sit far from 1: a gate
/// that dropped the margin would then misjudge faces.
const GAIN: f64 = 5.0;

/// `SLAB_W` soil columns at field capacity, faces at `y = 2`, open sky (the sim's own
/// `canopy_establish_tests` slab).
fn slab() -> World {
    let mut w = World::empty(VoxelConfig { width: SLAB_W, height: 24, depth: 1, voxel_m: VOXEL_M, seed: 5, rain_m_per_s: 0.0, ..VoxelConfig::default() });
    let pore = cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil);
    let cap = w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for x in 0..i64::from(SLAB_W) {
        for y in 1..=2u32 {
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: pore * cap });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    w
}

/// Nothing reproduces or dies of age; the arming check off, so a passing check sprouts.
fn quiet() -> FloraConfig {
    let mut c = FloraConfig::for_voxel_size(VOXEL_M);
    for sp in Species::ALL {
        let s = c.species_mut(sp);
        s.seed_attrition_per_s = 0.0;
        s.propagule_rate = 0.0;
        s.burr_pickup = 0.0;
        s.cure_after_s = 0.0;
    }
    c.sprout_after_arming = false;
    c
}

/// A newborn's cover ratio as the sim's `break_even_cover` states it (`step.rs:3872`):
/// `assimilation · L_eff · P · μ · N/(N+K) / (maintenance · W + (1 + c_g) · senescence · P)`.
fn cover(sc: &cubarium_voxel_flora::SpeciesConfig, l_eff: f64, mu: f64, n: f64) -> f64 {
    let nb = StandState::newborn(sc, package_of(sc));
    let up = sc.maintenance * nb.wood + (1.0 + sc.build) * sc.senescence * nb.foliage;
    sc.assimilation * l_eff * nb.foliage * mu * (n / (n + sc.nutrient_half)) / up
}

/// **The canopy gate on the newborn's own light** (`canopy_pays`, `step.rs:3899`: the
/// newborn earns its margin under the light through every taller stand's foliage,
/// `canopy_light`, at the site's `μ` and mineral). Tall stands shade part of a soil slab.
/// The census reads each face's optical depth over a turf newborn's crown top
/// (`Face::nb_tau`), its sky, water and mineral; the model's gate (`model::n_star` at
/// `light_response(sky · e^−τ)` and `μ`) predicts which faces a seed comes up on. Each face
/// then gets one seed in the sim, and the sim's own seed check decides. Faces within 10 % of
/// the margin are not judged; the fixture asserts both outcomes occur among the rest.
#[test]
fn the_canopy_gate_on_the_newborns_own_light_is_the_sims() {
    let sp = Species::Springturf;
    let k = PHOTO.iter().position(|&s| s == sp).unwrap();
    let mut world = slab();
    // The tall stands, on a plant layer whose canopy gate has `margin`.
    let make = |world: &World, margin: f64| {
        let mut c = quiet();
        c.shade_k_per_m2 *= SHADE;
        c.species_mut(sp).assimilation *= GAIN;
        c.species_mut(sp).establish_light_margin = margin;
        let mut flora = Flora::new(c);
        for (x, tall) in [(3, Species::Bloomcrown), (9, Species::Umbrellafrond)] {
            let wood = flora.view().config.species(tall).wood_max;
            assert!(flora.apply(world, Command::SeedOnFace { site: FSite { x, y: 2, z: 0 }, species: tall, wood }));
        }
        flora
    };
    let mut fc = quiet();
    fc.species_mut(sp).assimilation *= GAIN;
    let sc = fc.species(sp);
    // The model's reading of each face: L_eff, μ, N.
    let read = |f: &Face| {
        let tau: f64 = f.nb_tau[k].iter().map(|&t| f64::from(t)).sum();
        let l_eff = light_response(sc, f.sky * (-tau).exp());
        let mu = sc.moisture_at(f.water[k].expect("fixture: a water band"));
        (l_eff, mu, f.mineral)
    };
    let probe_faces = census::census(&world, &make(&world, 0.0), None);
    let open = probe_faces.iter().filter(|f| f.plant.is_none()).map(|f| { let (l, m, n) = read(f); cover(sc, l, m, n) }).fold(0.0, f64::max);
    let margin = 0.5 * open;
    let mut flora = make(&world, margin);
    let faces = census::census(&world, &flora, None);
    let sc = flora.view().config.species(sp).clone();
    let mut judged = Vec::new();
    for f in faces.iter().filter(|f| f.plant.is_none()) {
        assert!(f.water_ok[k] && f.light_ok[k], "fixture: the site gates pass on face {}", f.x);
        let (l, m, n) = read(f);
        let ratio = cover(&sc, l, m, n) / margin;
        let predicted = n >= model::n_star(&sc, l, m);
        if (0.9..1.1).contains(&ratio) {
            continue;
        }
        judged.push((f.x, predicted, ratio));
        let organic = package_of(&sc);
        assert!(flora.deposit_seeds(FSite { x: f.x, y: 2, z: 0 }, sp, organic, sc.n_tissue * organic, sc.energy_density * organic));
    }
    eprintln!("judged faces (x, predicted up, cover / margin): {judged:?}");
    assert!(judged.iter().any(|j| j.1) && judged.iter().any(|j| !j.1), "fixture: both outcomes among judged faces: {judged:?}");
    // Three seed checks' worth of ticks.
    let ticks = (3.0 * flora.view().config.seed_check_s / cubarium_voxel::DT).ceil() as usize + 10;
    for _ in 0..ticks {
        flora.step(&mut world);
    }
    for (x, predicted, ratio) in judged {
        let up = flora.view().stands.iter().any(|s| s.site == FSite { x, y: 2, z: 0 } && s.species == sp);
        assert_eq!(up, predicted, "face {x}: the sim's gate {} a seed the model predicts {} (cover / margin {ratio:.2})", if up { "let up" } else { "held" }, if predicted { "up" } else { "held" });
        // And the model's break-even mineral is the sim's `break_even_cover` restated.
        assert_eq!(predicted, ratio >= 1.0, "face {x}: n_star disagrees with the cover ratio {ratio:.3}");
    }
}

// ===================================================================================
// Model fixtures: zones of hand-built faces
// ===================================================================================

/// Faces `x0..x0+n` at sky `sky`, mineral `n0`, the first ones held by `occupants`
/// (species, wood fraction, count).
fn faces(fc: &FloraConfig, x0: u32, n: u32, sky: f64, n0: f64, occupants: &[(Species, f64, u32)]) -> Vec<Face> {
    let mut out: Vec<Face> = (x0..x0 + n).map(|x| Face::bare(x, 1, 0, sky, n0)).collect();
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

/// The zones of `faces` with every species' gate open and water at field capacity, held.
fn zones_of(fc: &FloraConfig, faces: &[Face]) -> Zones {
    let mut z = zones::build(fc, &[faces.to_vec()]);
    let r = Reading { mu: 1.0, available: 1.0, saturated: 0.0, stress_target: 0.0, gate: 1.0, mu_held: 1.0, stress_held: 0.0, available_held: 1.0 };
    for zone in &mut z.zones {
        zone.weather = vec![[r; NS]];
        zone.fungal_space = 1.0;
    }
    z
}

fn build(fc: &FloraConfig, p: &cubarium_voxel_sim::params::Params, faces: &[Face], kernels: Vec<Kernel>, start_minute: f64, mp: Params) -> Model {
    let z = zones_of(fc, faces);
    let rb = (0..NS).map(|_| RunnerBox { faces: 8.0, same_zone: 1.0 }).collect();
    Model::new(fc.clone(), DayModel::of(&p.world, fc.upkeep_q10), z, kernels, rb, faces, start_minute, mp)
}

fn local_kernels(nz: usize) -> Vec<Kernel> {
    (0..NS).map(|_| Kernel { land: (0..nz).map(|i| (0..nz).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect(), lost: vec![0.0; nz] }).collect()
}

fn frozen(dt: f64) -> Params {
    Params { weather: Weather::Frozen, dt, ..Params::default() }
}

/// The first minute of the world clock with the given daylight (1: full day, 0: night).
fn minute_at(dm: &DayModel, daylight: f64) -> f64 {
    let day = dm.day_s();
    (0..1440).map(|k| k as f64 * day / 1440.0).find(|&t| (dm.at(t, 1.0).daylight - daylight).abs() < 1e-12 && (dm.at(t + 600.0, 1.0).daylight - daylight).abs() < 1e-12).expect("fixture: such a time") / 60.0
}

// ===================================================================================
// 3. A cohort merge at alive_min (the flora worker's finding 2), through the model's rebin
// ===================================================================================

/// **Merged newborns at `alive_min` survive the night** (the sim's death rule `W <
/// alive_min`, `step.rs:3260`; a newborn's wood is `alive_min` when one package's wood share
/// is exactly it, `seeds::newborn_stocks`). Newborn cohorts of many sizes in one zone, at
/// night: their reserve pays the upkeep and nothing grows wood, so only a merge can move
/// it. After the model's own rebin merges them, no stand has starved and every cohort's
/// wood is at or above `alive_min`.
#[test]
fn merged_newborns_at_alive_min_survive_the_night_through_the_rebin() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let night = minute_at(&dm, 0.0);
    // The species whose one-package newborn sits exactly at alive_min (`seed_mass` 1).
    let at_min: Vec<Species> = PHOTO.iter().copied().filter(|&sp| { let sc = fc.species(sp); StandState::newborn(sc, package_of(sc)).wood == sc.alive_min }).collect();
    assert!(at_min.len() >= 2, "fixture: newborns at alive_min: {at_min:?}");
    for sp in at_min {
        let sc = fc.species(sp);
        let nb = StandState::newborn(sc, package_of(sc));
        assert!(nb.reserve > 0.0, "fixture: its reserve pays the night");
        let f = faces(&fc, 0, 600, 0.95, 1.0, &[]);
        let mut m = build(&fc, &p, &f, local_kernels(1), night, frozen(10.0));
        for i in 0..40 {
            let n = 0.013 + 0.37 * f64::from(i) + 1.0 / (0.7 + f64::from(i));
            m.z[0].cohorts[sp.index()].push(Cohort { n, st: nb, covered: false, opened_s: m.t_s, site: model::Site::default() });
        }
        for _ in 0..4 {
            m.step();
        }
        assert!(m.z[0].cohorts[sp.index()].len() < 40, "fixture: the rebin merged them");
        assert_eq!(m.c.starved[sp.index()], 0.0, "{sp:?}: merged newborns starved at night");
        assert!(m.z[0].cohorts[sp.index()].iter().all(|c| c.st.wood >= sc.alive_min), "{sp:?}: a merged cohort under alive_min");
    }
}

// ===================================================================================
// 4. The take: spread over zones, what was taken is what left
// ===================================================================================

/// Two zones (mid and summit), turf, pad, lanternberry (fruit), bloomcrown (nectar),
/// glowcap; litter in both; the carrion pool stocked.
fn two_zones(fc: &FloraConfig, p: &cubarium_voxel_sim::params::Params, dt: f64, start_minute: f64) -> Model {
    let mut f = faces(fc, 0, 300, 0.5, 1.0, &[(Species::Springturf, 1.0, 60), (Species::Velvetpad, 1.0, 80), (Species::Bloomcrown, 1.0, 15), (Species::Glowcap, 1.0, 40)]);
    f.extend(faces(fc, 300, 300, 0.95, 0.8, &[(Species::Springturf, 0.7, 90), (Species::Lanternberry, 1.0, 20), (Species::Vaulttree, 0.6, 5), (Species::Glowcap, 1.0, 20)]));
    for (i, x) in f.iter_mut().enumerate() {
        x.litter = 0.02 + 0.01 * (i % 7) as f64;
    }
    let mut m = build(fc, p, &f, local_kernels(2), start_minute, frozen(dt));
    assert_eq!(m.z.len(), 2, "fixture: two zones");
    m.carrion = 2.0;
    // Fruit and nectar on the adults.
    for zs in &mut m.z {
        for c in zs.cohorts[Species::Lanternberry.index()].iter_mut() {
            c.st.fruit = 0.01;
        }
        for c in zs.cohorts[Species::Bloomcrown.index()].iter_mut() {
            c.st.nectar = 0.01;
        }
    }
    m
}

fn edible_of(m: &Model, s: usize) -> f64 {
    let sc = m.config.species(Species::ALL[s]);
    m.z.iter().flat_map(|z| z.cohorts[s].iter()).map(|c| c.n * (c.st.foliage - sc.graze_refuge * sc.alpha * c.st.wood).max(0.0)).sum()
}

/// **The take spread over zones keeps what was taken** (the clamped exchange, `apply_take`).
/// Two identical worlds step once, one with a take. Against the untaken world: each zone's
/// litter fell by the litter taken in proportion to its pool; each foliage species' stands
/// lost exactly what the ledger says was taken from it (and only it); nectar and fruit the
/// same; the carrion pool and its decay together by what was taken; the returns spread over
/// the zones by area add up to the litter returned. A take larger than the stock takes the
/// stock and books no more.
#[test]
fn the_take_spread_over_zones_conserves_what_was_taken() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let t0 = minute_at(&dm, 1.0);
    let mut a = two_zones(&fc, &p, 10.0, t0);
    let mut b = two_zones(&fc, &p, 10.0, t0);
    let turf = Species::Springturf.index();
    let pad = Species::Velvetpad.index();
    let pad_edible = edible_of(&b, pad);
    b.take.litter = 0.4;
    b.take.foliage[turf] = 0.05;
    b.take.foliage[pad] = 10.0 * pad_edible.max(1e-6); // more than there is
    b.take.nectar = 0.03;
    b.take.fruit = 0.05;
    b.take.carrion = 0.5;
    b.returns.litter = 0.2;
    a.returns.litter = 0.2;
    a.step();
    b.step();
    let la: Vec<f64> = a.z.iter().map(|z| z.litter).collect();
    let lb: Vec<f64> = b.z.iter().map(|z| z.litter).collect();
    // The untaken world's pools before its returns are the taken world's before its take.
    let area: Vec<f64> = b.zones.zones.iter().map(|z| z.area).collect();
    let at: f64 = area.iter().sum();
    let pre: Vec<f64> = la.iter().zip(&area).map(|(l, ar)| l - 0.2 * ar / at).collect();
    let total: f64 = pre.iter().sum();
    for zi in 0..2 {
        assert!(close(la[zi] - lb[zi], 0.4 * pre[zi] / total, 1e-9), "zone {zi}: litter fell {} vs share {}", la[zi] - lb[zi], 0.4 * pre[zi] / total);
    }
    assert!(close(b.c.taken.litter, 0.4, 1e-12));
    let fol = |m: &Model, s: usize| m.z.iter().flat_map(|z| z.cohorts[s].iter()).map(|c| c.n * c.st.foliage).sum::<f64>();
    for s in 0..NS {
        let d = fol(&a, s) - fol(&b, s);
        assert!((d - b.c.taken.foliage[s]).abs() <= 1e-9 * fol(&a, s).max(1e-9), "{:?}: foliage fell {d}, taken {}", Species::ALL[s], b.c.taken.foliage[s]);
    }
    assert!(close(b.c.taken.foliage[turf], 0.05, 1e-12), "the turf take is met");
    assert!(b.c.taken.foliage[pad] < b.take.foliage[pad] && b.c.taken.foliage[pad] > 0.0, "a take over the stock takes the stock");
    let held = |m: &Model, nectar: bool| m.z.iter().flat_map(|z| z.cohorts.iter().flatten()).map(|c| c.n * if nectar { c.st.nectar } else { c.st.fruit }).sum::<f64>();
    assert!(close(held(&a, true) - held(&b, true), b.c.taken.nectar, 1e-9), "nectar");
    assert!(close(held(&a, false) - held(&b, false), b.c.taken.fruit, 1e-9), "fruit");
    assert!(close((a.carrion - b.carrion) + (a.c.decomposed - b.c.decomposed), b.c.taken.carrion, 1e-9), "carrion and its decay");
    assert!(close(b.c.taken.carrion, 0.5, 1e-12));
    // The whole: the organic held differs by exactly what was booked taken.
    let taken = b.c.taken.litter + b.c.taken.foliage.iter().sum::<f64>() + b.c.taken.carrion + b.c.taken.film + b.c.taken.nectar + b.c.taken.fruit;
    let d = (a.organic() - b.organic()) + (a.c.decomposed - b.c.decomposed) - (a.c.respired - b.c.respired);
    assert!(close(d, taken, 1e-9), "held differs by {d}, booked taken {taken}");
}

/// **What is offered can be taken.** The supply offers each species' edible foliage through
/// the sim's own layers (`layers_of(...).edible()`, above each layer's floor); the take
/// removes foliage above `graze_refuge · alpha · W` summed over the stand. On one state the
/// two are the same number (the model's layers are always binned evenly from `W` and `P`,
/// so no layer sits under its floor while another is above it). A take of the whole offer
/// applied one step later can still fall short: the step's own senescence comes first. On stands whose layers are unevenly filled the two
/// floors differ (a sum of per-layer `max(stock − floor, 0)` is at least the stand-level
/// `P − Σ floor`), so the offer can exceed what the take will remove.
#[test]
fn what_is_offered_can_be_taken() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let t0 = minute_at(&dm, 1.0);
    let m = two_zones(&fc, &p, 10.0, t0);
    let offer = m.supply();
    for s in 0..NS {
        if s == Species::Glowcap.index() || offer.foliage[s] <= 0.0 {
            continue;
        }
        let takeable = edible_of(&m, s);
        assert!(close(takeable, offer.foliage[s], 1e-6), "{:?}: offered {}, the take can remove {takeable}", Species::ALL[s], offer.foliage[s]);
    }
}

// ===================================================================================
// 5. The ledgers over a long run
// ===================================================================================

/// Three zones whose seeds cross between them and partly fall to litter; every flow on: growth,
/// funding, runners and seeds, germination and overtopping, deaths, the bank's decay,
/// decomposition, the fungus's feed, a seedling browser, and a take with returns.
fn busy(fc: &FloraConfig, p: &cubarium_voxel_sim::params::Params, takes_caps: bool, takes_fruit: bool, browse: bool) -> Model {
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let t0 = minute_at(&dm, 1.0);
    let mut f = faces(fc, 0, 300, 0.5, 1.0, &[(Species::Springturf, 1.0, 60), (Species::Velvetpad, 1.0, 80), (Species::Bloomcrown, 1.0, 15), (Species::Glowcap, 1.0, 40)]);
    f.extend(faces(fc, 300, 300, 0.95, 0.8, &[(Species::Springturf, 0.7, 90), (Species::Lanternberry, 1.0, 20), (Species::Vaulttree, 0.6, 5), (Species::Glowcap, 1.0, 20)]));
    // A dim, poor third zone of turf barely past alive_min: it starves there.
    let turf = fc.species(Species::Springturf);
    f.extend(faces(fc, 600, 100, 0.2, 0.3, &[(Species::Springturf, 1.05 * turf.alive_min / turf.wood_max, 40)]));
    for (i, x) in f.iter_mut().enumerate() {
        x.litter = 0.02 + 0.01 * (i % 7) as f64;
        x.seeds[Species::Lanternberry.index()] = (i % 3) as u32;
        x.seeds[Species::Vaulttree.index()] = (i % 5 == 0) as u32;
    }
    let kern = (0..NS).map(|_| Kernel { land: vec![vec![0.5, 0.3, 0.1], vec![0.2, 0.6, 0.1], vec![0.1, 0.1, 0.7]], lost: vec![0.1, 0.1, 0.1] }).collect();
    let mut mp = frozen(10.0);
    if browse {
        mp.juvenile_browse[Species::Lanternberry.index()] = 1e-4;
    }
    let mut m = build(fc, p, &f, kern, t0, mp);
    assert_eq!(m.z.len(), 3, "fixture: three zones");
    for zs in &mut m.z {
        for c in zs.cohorts[Species::Springturf.index()].iter_mut().filter(|c| c.st.wood < 2.0 * turf.alive_min) {
            c.st.foliage = 0.0;
            c.st.reserve = 0.0;
        }
    }
    m.carrion = 1.0;
    m.take.litter = 1e-4;
    m.take.foliage[Species::Velvetpad.index()] = 1e-3;
    m.take.foliage[Species::Springturf.index()] = 5e-4;
    m.take.nectar = 1e-6;
    m.take.carrion = 1e-5;
    if takes_fruit {
        m.take.fruit = 1e-5;
    }
    if takes_caps {
        m.take.foliage[Species::Glowcap.index()] = 1e-3;
    }
    m.returns.litter = 5e-5;
    m.returns.carrion = 1e-4;
    m
}

/// Steps `m` for `days` and returns (organic residual, mineral residual), both relative.
fn ledger_residuals(m: &mut Model, days: f64) -> (f64, f64) {
    let o0 = m.organic();
    let (a, b, c) = m.mineral();
    let n0 = a + b + c;
    for _ in 0..(days * m.day.day_s() / m.p.dt).round() as usize {
        m.step();
    }
    let k = &m.c;
    let taken = k.taken.litter + k.taken.foliage.iter().sum::<f64>() + k.taken.carrion + k.taken.film + k.taken.nectar + k.taken.fruit;
    let expect = o0 + k.fixed + k.seep + k.returned - k.respired - k.decomposed - taken;
    let (a, b, c) = m.mineral();
    let n1 = a + b + c + k.taken_mineral - k.held_in;
    ((m.organic() - expect) / o0, (n1 - n0) / n0)
}

/// **The organic and mineral ledgers close over a long run**: six world days of every flow
/// across three zones (seeds crossing between them and falling to litter, runners, takeovers,
/// deaths, the fungus, a take and returns). The organic matter held
/// moves by fixation + seep + returns − respiration − decomposition − the take; the mineral
/// (site pools, stand inventories, banks, and what bites carried out) is unchanged.
#[test]
fn the_ledgers_close_over_a_long_run() {
    let (p, fc) = resolved();
    let mut m = busy(&fc, &p, false, false, false);
    let (o, n) = ledger_residuals(&mut m, 6.0);
    let c = &m.c;
    assert!(c.births_seed.iter().sum::<f64>() > 0.0 && c.births_runner.iter().sum::<f64>() > 0.0, "fixture: seeds and runners came up");
    assert!(c.starved.iter().sum::<f64>() > 0.0 && c.overtopped.iter().sum::<f64>() > 0.0 && c.seeds_died.iter().sum::<f64>() > 0.0, "fixture: starvation, takeovers and seed rot");
    assert!(c.substrate > 0.0 && c.taken.nectar > 0.0, "fixture: the fungus fed; nectar was taken");
    assert!(o.abs() <= 1e-8, "organic residual {o:e}");
    assert!(n.abs() <= 1e-8, "mineral residual {n:e}");
}

/// **The organic ledger with the seedling-browser lever on** (`Params::juvenile_browse`, the
/// succession runs' `browse=`): the browsed seedlings' organic matter leaves the stands, and
/// their mineral goes to the pool, but the organic is booked nowhere (not litter, not
/// taken, not respired).
#[test]
#[ignore = "BUG: juvenile_browse removes seedlings' organic matter without booking it (no litter, no taken, no respired line): the organic ledger does not close with browse= on"]
fn the_organic_ledger_closes_with_the_browse_lever() {
    let (p, fc) = resolved();
    let mut m = busy(&fc, &p, false, false, true);
    let (o, n) = ledger_residuals(&mut m, 2.0);
    assert!(m.c.browsed.iter().sum::<f64>() > 0.0, "fixture: seedlings were browsed");
    assert!(o.abs() <= 1e-8, "organic residual {o:e}");
    assert!(n.abs() <= 1e-8, "mineral residual {n:e}");
}

/// **The mineral ledger with caps taken.** A grazer's take of the glowcap's caps books
/// `n_tissue · caps` as `taken_mineral`, but the fungus's inventory is outside
/// `Model::mineral()` (it skips the glowcap), so the ledger gains the caps' mineral from
/// nothing.
#[test]
#[ignore = "BUG: apply_take books the glowcap caps' mineral in taken_mineral while Model::mineral() never counted the fungus's inventory: the mineral ledger drifts by n_tissue × caps taken"]
fn the_mineral_ledger_closes_with_caps_taken() {
    let (p, fc) = resolved();
    let mut m = busy(&fc, &p, true, false, false);
    let (o, n) = ledger_residuals(&mut m, 2.0);
    assert!(m.c.taken.foliage[Species::Glowcap.index()] > 0.0, "fixture: caps were taken");
    assert!(o.abs() <= 1e-8, "organic residual {o:e}");
    assert!(n.abs() <= 1e-8, "mineral residual {n:e}");
}

/// **The mineral ledger with fruit taken, when the fruiting species' tissue differs from
/// springturf's.** `apply_take` books nectar and fruit at springturf's `n_tissue`; the stands
/// lose their own species' `n_tissue` per unit. In the terrarium every species has the same
/// `n_tissue`, so the defect is latent; a config with lanternberry's own shows it.
#[test]
#[ignore = "BUG (latent): apply_take books nectar/fruit mineral at springturf's n_tissue, not the producer's; the mineral ledger drifts once n_tissue differs by species"]
fn the_mineral_ledger_closes_with_fruit_of_its_own_tissue() {
    let (p, mut fc) = resolved();
    fc.species_mut(Species::Lanternberry).n_tissue *= 2.0;
    let mut m = busy(&fc, &p, false, true, false);
    let (o, n) = ledger_residuals(&mut m, 2.0);
    assert!(m.c.taken.fruit > 0.0, "fixture: fruit was taken");
    assert!(o.abs() <= 1e-8, "organic residual {o:e}");
    assert!(n.abs() <= 1e-8, "mineral residual {n:e}");
}

// ===================================================================================
// 6. Config flow
// ===================================================================================

/// A zone of bare faces holding only litter (and the carrion pool): nothing grows, nothing
/// sheds, nothing feeds.
fn dead_ground(fc: &FloraConfig, p: &cubarium_voxel_sim::params::Params) -> Model {
    let mut f = faces(fc, 0, 100, 0.8, 1.0, &[]);
    for x in f.iter_mut() {
        x.litter = 0.5;
    }
    let mut m = build(fc, p, &f, local_kernels(1), 0.0, frozen(10.0));
    m.carrion = 3.0;
    m
}

/// **`decomposition` and `carrion_decomposition`** (`step.rs:3659`, `decay_share`: the pools
/// fall by `1 − e^(−k·dt)` a step): over an hour each pool is `e^(−k·t)` of its start, so
/// doubling the knob doubles the log-loss.
#[test]
fn config_flow_decomposition_is_first_order() {
    let (p, base) = resolved();
    let logloss = |fc: &FloraConfig| {
        let mut m = dead_ground(fc, &p);
        let (l0, c0) = (m.z[0].litter, m.carrion);
        for _ in 0..360 {
            m.step();
        }
        ((m.z[0].litter / l0).ln(), (m.carrion / c0).ln())
    };
    let (l1, c1) = logloss(&base);
    assert!(close(l1, -base.decomposition * 3600.0, 1e-9), "litter {l1} vs −k·t {}", -base.decomposition * 3600.0);
    assert!(close(c1, -base.carrion_decomposition * 3600.0, 1e-9), "carrion {c1}");
    let mut twice = base.clone();
    twice.decomposition *= 2.0;
    twice.carrion_decomposition *= 2.0;
    let (l2, c2) = logloss(&twice);
    assert!(close(l2, 2.0 * l1, 1e-9) && close(c2, 2.0 * c1, 1e-9), "doubled: {l2} vs {}, {c2} vs {}", 2.0 * l1, 2.0 * c1);
}

/// The model's bank e-folding time for species `s` (its seeds rot and nothing lands).
fn bank_life(fc: &FloraConfig, p: &cubarium_voxel_sim::params::Params, s: Species) -> f64 {
    let mut f = faces(fc, 0, 100, 0.8, 1.0, &[]);
    for x in f.iter_mut() {
        x.seeds[s.index()] = 2;
    }
    // No germination: the fitted persistence at zero (a model parameter, not a sim knob).
    let mut mp = frozen(10.0);
    mp.establish_phi = [0.0; NS];
    let mut m = build(fc, p, &f, local_kernels(1), 0.0, mp);
    let b0 = m.z[0].bank[s.index()];
    for _ in 0..360 {
        m.step();
    }
    -3600.0 / (m.z[0].bank[s.index()] / b0).ln()
}

/// **The seed bank's two knobs** (`seeds.rs:734-773`: each seed dies at
/// `seed_attrition_per_s` along the way and its bin falls to litter whole past
/// `seed_max_age_s`). A seed's mean life under that law is `(1 − e^(−aT)) / a`; the model's
/// one rate is meant to keep it (`flora.md`: "one rate, `1 / ((1 − e⁻¹) · seed_max_age_s)`",
/// which is that mean life only when `a = 1/T`). The bank's e-folding time follows the mean
/// life when either knob moves.
#[test]
#[ignore = "DISAGREES: the model's bank rate reads only seed_max_age_s (it assumes seed_attrition_per_s = 1/seed_max_age_s, true in today's presets); seed_attrition_per_s does not flow, and seed_max_age_s moves it in the wrong form"]
fn config_flow_the_bank_follows_both_of_its_knobs() {
    let (p, base) = resolved();
    let s = Species::Lanternberry;
    let mean_life = |fc: &FloraConfig| {
        let sc = fc.species(s);
        let (a, t) = (sc.seed_attrition_per_s, sc.seed_max_age_s);
        if a > 0.0 { (1.0 - (-a * t).exp()) / a } else { t }
    };
    let mut a2 = base.clone();
    a2.species_mut(s).seed_attrition_per_s *= 2.0;
    let mut t2 = base.clone();
    t2.species_mut(s).seed_max_age_s *= 2.0;
    for (name, fc) in [("base", &base), ("attrition × 2", &a2), ("max age × 2", &t2)] {
        let (got, want) = (bank_life(fc, &p, s), mean_life(fc));
        assert!(close(got, want, 0.01), "{name}: bank e-folds in {got} s, the sim's mean seed life is {want} s");
    }
}

/// **`foliage_rate`** (`step.rs:3002`: the foliage demand `min(α W − P, foliage_rate · W ·
/// dt)`; a stand under `reflush_below` of its cap reflushes from its reserve,
/// `step.rs:3101`). A defoliated pad cohort in full daylight builds `foliage_rate · W · dt`
/// in a step from its reserve; doubling the knob doubles it.
#[test]
fn config_flow_foliage_rate_sets_the_reflush() {
    let (p, base) = resolved();
    let dm = DayModel::of(&p.world, base.upkeep_q10);
    let noon = minute_at(&dm, 1.0);
    let s = Species::Velvetpad;
    let built = |fc: &FloraConfig| {
        let f = faces(fc, 0, 100, 0.95, 1.0, &[(s, 1.0, 50)]);
        let mut m = build(fc, &p, &f, local_kernels(1), noon, frozen(1.0));
        let sc = fc.species(s);
        for c in m.z[0].cohorts[s.index()].iter_mut() {
            c.st.foliage = 0.0;
            c.st.reserve = sc.reserve_cap * c.st.wood;
        }
        let n: f64 = m.z[0].cohorts[s.index()].iter().map(|c| c.n * c.st.wood).sum();
        m.step();
        (m.c.built[s.index()], n * sc.foliage_rate * 1.0)
    };
    let (a, want_a) = built(&base);
    let mut twice = base.clone();
    twice.species_mut(s).foliage_rate *= 2.0;
    let (b, want_b) = built(&twice);
    let sc = base.species(s);
    assert!(2.0 * sc.foliage_rate * 1.0 < sc.reflush_below * sc.alpha, "fixture: the step's demand is under the reflush cap");
    assert!(close(a, want_a, 1e-9), "built {a} vs foliage_rate·W·dt {want_a}");
    assert!(close(b, want_b, 1e-9) && close(b, 2.0 * a, 1e-9), "doubled: {b} vs {want_b}");
}

/// **`assimilation`** (`step.rs:3036`: income `assimilation · L_eff · P · daylight · μ · (1 −
/// stress) · N/(N+K) · dt`, under the caps of the draw, the tissue and the demand): a turf
/// mat in dim light, where no cap binds, fixes twice as much in a step at twice the knob.
#[test]
fn config_flow_assimilation_is_linear_under_its_caps() {
    let (p, base) = resolved();
    let dm = DayModel::of(&p.world, base.upkeep_q10);
    let noon = minute_at(&dm, 1.0);
    let s = Species::Springturf;
    let fixed = |fc: &FloraConfig| {
        // A rich site, so the pool's draw cap `nutrient_draw_max · N · dt` stays above it.
        let f = faces(fc, 0, 100, 0.2, 4.0, &[(s, 1.0, 60)]);
        let mut m = build(fc, &p, &f, local_kernels(1), noon, frozen(1.0));
        // An empty reserve: its refill is a demand no dim-light income meets.
        for c in m.z[0].cohorts[s.index()].iter_mut() {
            c.st.reserve = 0.0;
        }
        m.step();
        m.c.fixed
    };
    let a = fixed(&base);
    let mut twice = base.clone();
    twice.species_mut(s).assimilation *= 2.0;
    let b = fixed(&twice);
    assert!(a > 0.0);
    assert!(close(b, 2.0 * a, 1e-9), "fixed {b} vs 2 × {a}");
}

