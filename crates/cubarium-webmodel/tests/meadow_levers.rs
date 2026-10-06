//! **The meadow levers' mechanism tests** (`design/webmodel/meadow.md`,
//! `src/flora/meadow.rs`): each lever does what its doc says on hand-built inputs, in
//! direction and form, against the config on hand; and the ledgers close with every lever
//! on. No tuned number is pinned: every expectation is computed from the config and the
//! lever's own value.

use std::path::Path;

use cubarium_voxel_flora::{FloraConfig, Species, Trophic};
use cubarium_webmodel::flora::census::{Face, StandRec};
use cubarium_webmodel::flora::climate::DayModel;
use cubarium_webmodel::flora::config;
use cubarium_webmodel::flora::kernel::{Kernel, RunnerBox};
use cubarium_webmodel::flora::meadow::Meadow;
use cubarium_webmodel::flora::model::{self, Cohort, Model, NS, Params, Weather};
use cubarium_webmodel::flora::stand::{StandState, package_of};
use cubarium_webmodel::flora::zones::{self, Reading, Zones};

const TURF: Species = Species::Springturf;
const LB: Species = Species::Lanternberry;

fn resolved() -> (cubarium_voxel_sim::params::Params, FloraConfig) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/scenarios/terrarium.toml");
    let p = config::resolve(&root, &[]).expect("the terrarium resolves");
    let f = config::flora_of(&p);
    (p, f)
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-300)
}

/// Faces `x0..x0+n` at sky `sky`, mineral `n0`, the first held by `occupants` (species,
/// wood as a share of `wood_max`, count).
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

/// Every gate open, water held at `water` (available) with `μ` from each species' ramp.
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

fn build(fc: &FloraConfig, p: &cubarium_voxel_sim::params::Params, faces: &[Face], kernels: Vec<Kernel>, start_minute: f64, mp: Params, water: f64) -> Model {
    let z = zones_of(fc, faces, water);
    let rb = (0..NS).map(|_| RunnerBox { faces: 8.0, same_zone: 1.0 }).collect();
    Model::new(fc.clone(), DayModel::of(&p.world, fc.upkeep_q10), z, kernels, rb, faces, start_minute, mp)
}

fn local_kernels(nz: usize) -> Vec<Kernel> {
    (0..NS).map(|_| Kernel { land: (0..nz).map(|i| (0..nz).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect(), lost: vec![0.0; nz] }).collect()
}

fn frozen(meadow: Meadow) -> Params {
    Params { weather: Weather::Frozen, dt: 10.0, meadow, ..Params::default() }
}

/// The first minute of the world clock with the given daylight (1: full day, 0: night),
/// held for the next ten minutes.
fn minute_at(dm: &DayModel, daylight: f64) -> f64 {
    let day = dm.day_s();
    (0..1440).map(|k| k as f64 * day / 1440.0).find(|&t| (dm.at(t, 1.0).daylight - daylight).abs() < 1e-12 && (dm.at(t + 600.0, 1.0).daylight - daylight).abs() < 1e-12).expect("fixture: such a time") / 60.0
}

/// A cohort of `n` lanternberry juveniles at wood `w`, well fed, on a neutral site (the
/// zone's lanternberry adults give their pool its mineral).
fn juveniles(fc: &FloraConfig, n: f64, w: f64, t: f64) -> Cohort {
    let sc = fc.species(LB);
    let st = StandState { wood: w, foliage: sc.alpha * w * 0.8, reserve: sc.reserve_cap * w * 0.8, ..StandState::default() };
    Cohort { n, st, covered: false, opened_s: t, site: model::Site::default() }
}

fn woody_n(m: &Model, lo: f64, hi: f64) -> f64 {
    m.z.iter().flat_map(|z| z.cohorts[LB.index()].iter()).filter(|c| c.st.wood >= lo && c.st.wood < hi).map(|c| c.n).sum()
}

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

// ===================================================================================
// Conservation with every lever on
// ===================================================================================

/// Two zones with turf, pad, lanternberry and vaulttree, woody seed on the faces, a turf take
/// (so the crop fires), and every meadow lever on at once: the crop, a dry spell, thatch and
/// topsoil draw, the contest, adult lifespans and a soil-depth requirement. The organic and
/// mineral ledgers close, and each lever's counter moved.
#[test]
fn the_ledgers_close_with_every_meadow_lever_on() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let t0 = minute_at(&dm, 1.0);
    let mut f = faces(&fc, 0, 300, 0.95, 1.0, &[(TURF, 1.0, 200), (Species::Velvetpad, 1.0, 40), (LB, 1.0, 20)]);
    f.extend(faces(&fc, 300, 300, 0.95, 0.8, &[(TURF, 0.7, 150), (LB, 1.0, 20), (Species::Vaulttree, 0.6, 5)]));
    for (i, x) in f.iter_mut().enumerate() {
        x.seeds[LB.index()] = 3;
        x.seeds[Species::Vaulttree.index()] = u32::from(i % 4 == 0);
        x.soil_depth = if i % 3 == 0 { 2 } else { 8 };
        x.litter = 0.02;
    }
    let day = dm.day_s();
    let mut mw = Meadow { crop_per_graze: 1e3, dry_every_s: 0.5 * day, dry_len_s: 0.1 * day, dry_phase_s: 0.2 * day, dry_water: 0.2, sward_germ: 3.0, sward_draw: 0.5, contest_escape: 0.3, contest_tau: 1.0, ..Meadow::default() };
    mw.lifespan_s[LB.index()] = 2.0 * day;
    mw.lifespan_s[Species::Vaulttree.index()] = 4.0 * day;
    mw.root_soil_min[LB.index()] = 3;
    mw.root_soil_min[Species::Vaulttree.index()] = 6;
    let mut m = build(&fc, &p, &f, local_kernels(2), t0, frozen(mw), 0.8);
    m.take.foliage[TURF.index()] = 2e-4;
    m.take.foliage[Species::Velvetpad.index()] = 2e-4;
    let (o, n) = ledger_residuals(&mut m, 2.0);
    let c = &m.c;
    let l = LB.index();
    assert!(c.cropped[l] > 0.0 && c.aged[l] > 0.0 && c.contested[l] > 0.0, "fixture: crop {} aged {} contested {}", c.cropped[l], c.aged[l], c.contested[l]);
    assert!(o.abs() <= 1e-8, "organic residual {o:e}");
    assert!(n.abs() <= 1e-8, "mineral residual {n:e}");
}

/// **A recovery-from-rarity start** (`Model::make_rare`) keeps the asked share of the
/// species' stands and banked seeds, and the ledgers close through it.
#[test]
fn make_rare_keeps_its_share_and_conserves() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let mut f = faces(&fc, 0, 300, 0.95, 1.0, &[(TURF, 1.0, 200)]);
    for x in &mut f {
        x.seeds[TURF.index()] = 4;
    }
    let mut m = build(&fc, &p, &f, local_kernels(1), minute_at(&dm, 1.0), frozen(Meadow::default()), 0.8);
    let (n0, b0) = (m.z[0].count(TURF.index()), m.z[0].bank[TURF.index()]);
    let o0 = m.organic();
    let (a, b, c) = m.mineral();
    m.make_rare(TURF.index(), 0.1);
    assert!(close(m.z[0].count(TURF.index()), 0.1 * n0, 1e-9) && close(m.z[0].bank[TURF.index()], 0.1 * b0, 1e-9));
    assert!(close(m.organic(), o0, 1e-12), "organic moved to the dead pools, not lost");
    let (a1, b1, c1) = m.mineral();
    assert!(close(a1 + b1 + c1, a + b + c, 1e-12), "mineral");
}

// ===================================================================================
// a. The crop
// ===================================================================================

/// **The crop's hazard is `crop_per_graze` times the sward foliage grazed per face per
/// second**, on juveniles under `crop_wood_max · donor_min` only. One zone of turf and
/// lanternberry juveniles and adults; a turf take in the first step; in the second, the
/// juveniles lose exactly `1 − exp(−h·dt)` of their number to the crop, the adults none; and
/// with no take, nothing is cropped.
#[test]
fn the_crop_follows_the_grazing_on_the_juveniles_only() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let t0 = minute_at(&dm, 1.0);
    let sc = fc.species(LB);
    let k = 3e3;
    let run = |take: f64| {
        let f = faces(&fc, 0, 400, 0.95, 1.0, &[(TURF, 1.0, 300), (LB, 1.0, 10)]);
        let mut m = build(&fc, &p, &f, local_kernels(1), t0, frozen(Meadow { crop_per_graze: k, ..Meadow::default() }), 0.8);
        let t = m.t_s;
        m.z[0].cohorts[LB.index()].push(juveniles(&fc, 40.0, 0.3 * sc.donor_min, t));
        m.take.foliage[TURF.index()] = take;
        m.step();
        let grazed = m.c.taken.foliage[TURF.index()];
        let juv = woody_n(&m, 0.0, sc.donor_min);
        let adults = woody_n(&m, sc.donor_min, f64::INFINITY);
        let starved = m.c.starved[LB.index()];
        m.take = Default::default();
        m.step();
        assert_eq!(m.c.starved[LB.index()], starved, "fixture: no juvenile starved in the cropping step");
        (grazed, juv, adults, m.c.cropped[LB.index()], woody_n(&m, sc.donor_min, f64::INFINITY))
    };
    let (grazed, juv, adults, cropped, adults_after) = run(1e-3);
    assert!(grazed > 0.0, "fixture: the turf was grazed");
    let h = k * (grazed / 10.0) / 400.0;
    let want = juv * (1.0 - (-h * 10.0).exp());
    assert!(close(cropped, want, 1e-9), "cropped {cropped} vs {want}");
    assert!(close(adults_after, adults, 1e-12), "adults are out of the band");
    let (_, _, _, none, _) = run(0.0);
    assert_eq!(none, 0.0, "no grazing, no crop");
}

// ===================================================================================
// b. Dry spells
// ===================================================================================

/// **A dry spell starves uncured woody juveniles while the turf cures and waits** — the
/// sim's own laws under the lever's water: lanternberry has no cure (`cure_after_s` 0) and
/// turf cures after `cure_after_s` below `cure_water`, dying only past `cure_max_s`. A spell
/// with no water, longer than the turf's cure delay and shorter than its cure limit: the
/// juveniles starve, the turf cures and none of it dies; the same run without the spell
/// starves no juvenile.
#[test]
fn a_dry_spell_starves_woody_juveniles_and_cures_the_turf() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let (tsc, lsc) = (fc.species(TURF), fc.species(LB));
    assert!(lsc.cure_after_s <= 0.0 && tsc.cure_after_s > 0.0, "fixture: woody does not cure, turf does");
    let len = (3.0 * tsc.cure_after_s).max(3600.0);
    assert!(len < tsc.cure_max_s, "fixture: the spell is shorter than the cure limit");
    let run = |water: f64| {
        let f = faces(&fc, 0, 300, 0.95, 1.0, &[(TURF, 1.0, 200), (LB, 1.0, 10)]);
        let mw = Meadow { dry_every_s: 10.0 * dm.day_s(), dry_len_s: len, dry_water: water, ..Meadow::default() };
        let mut m = build(&fc, &p, &f, local_kernels(1), minute_at(&dm, 1.0), frozen(mw), 0.8);
        let t = m.t_s;
        m.z[0].cohorts[LB.index()].push(juveniles(&fc, 30.0, 1.5 * lsc.alive_min, t));
        assert_eq!(m.dry_zone(0), water < 1.0, "the spell starts with the run");
        for _ in 0..(len / 10.0) as usize {
            m.step();
        }
        let cured = m.z[0].cohorts[TURF.index()].iter().filter(|c| c.st.cured).map(|c| c.n).sum::<f64>();
        (m.c.starved[LB.index()], cured, m.c.starved[TURF.index()] + m.c.drought[TURF.index()])
    };
    let (lb_dead, cured, turf_dead) = run(0.0);
    assert!(lb_dead > 0.0, "the woody juveniles starved");
    assert!(cured > 0.0 && turf_dead == 0.0, "the turf cured ({cured}) and none died ({turf_dead})");
    let (lb_wet, _, _) = run(1.0);
    assert_eq!(lb_wet, 0.0, "without the spell no juvenile starved");
}

// ===================================================================================
// c. Sward: thatch and topsoil draw
// ===================================================================================

/// **Thatch divides the woody overtopping rate through turf by `1 + sward_germ · fill`.**
/// A zone wholly turf (no bare face, so every woody birth is through it), lanternberry seed
/// on every face, one daytime step: the births through turf with thatch against without
/// follow the rate's form, `1 − exp(−λ dt / (1 + g·fill))` over `1 − exp(−λ dt)`.
#[test]
fn thatch_divides_the_overtopping_rate() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let run = |g: f64| {
        let mut f = faces(&fc, 0, 300, 0.95, 1.0, &[(TURF, 1.0, 300)]);
        for x in &mut f {
            x.seeds[LB.index()] = 5;
        }
        let mut m = build(&fc, &p, &f, local_kernels(1), minute_at(&dm, 1.0), frozen(Meadow { sward_germ: g, ..Meadow::default() }), 0.8);
        let n = m.z[0].count(TURF.index());
        m.step();
        // The fill germination read: after the step's growth; the overtopped stands left
        // in proportion, which keeps the fill.
        (m.c.overtopped[TURF.index()], n, m.turf_fill(0))
    };
    let (k0, n, fill) = run(0.0);
    assert!(k0 > 0.0 && fill > 0.0, "fixture: woody came up through the turf");
    let g = 4.0;
    let (kg, _, _) = run(g);
    let ldt = -(1.0 - k0 / n).ln();
    let want = n * (1.0 - (-ldt / (1.0 + g * fill)).exp());
    assert!(close(kg, want, 1e-6), "with thatch {kg} vs {want}");
}

/// **Topsoil draw lowers a woody juvenile's water by the turf's share of the zone**: on a
/// zone half turf, at water on the lanternberry's ramp, its juveniles fix less with the draw
/// than without, and the same with no turf in the zone.
#[test]
fn the_topsoil_draw_follows_the_turf_share() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let lsc = fc.species(LB);
    let water = lsc.wilt_water + 0.5 * (lsc.full_water - lsc.wilt_water);
    let fixed = |draw: f64, turf: u32| {
        let f = faces(&fc, 0, 300, 0.95, 1.0, &[(TURF, 1.0, turf), (LB, 1.0, 10)]);
        let mut m = build(&fc, &p, &f, local_kernels(1), minute_at(&dm, 1.0), frozen(Meadow { sward_draw: draw, ..Meadow::default() }), water);
        let t = m.t_s;
        m.z[0].cohorts[LB.index()].push(juveniles(&fc, 20.0, 0.3 * lsc.donor_min, t));
        let f0 = m.c.fixed;
        let turf_only = {
            let mut q = m.clone();
            q.z[0].cohorts[LB.index()].clear();
            q.step();
            q.c.fixed - f0
        };
        m.step();
        m.c.fixed - f0 - turf_only
    };
    assert!(fixed(0.8, 150) < fixed(0.0, 150), "the draw cut the juveniles' income");
    assert!(close(fixed(0.8, 0), fixed(0.0, 0), 1e-12), "no turf, no draw");
}

// ===================================================================================
// d. Lifespan
// ===================================================================================

/// **Adults die of age at `1 / lifespan_s`; juveniles never.** One step at night (nothing
/// grows wood): the adults lost `1 − exp(−dt / L)` of their number, the juveniles none.
#[test]
fn adults_die_of_age_at_the_lifespan_hazard() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let sc = fc.species(LB);
    let l = 3.0 * dm.day_s();
    let mut mw = Meadow::default();
    mw.lifespan_s[LB.index()] = l;
    let f = faces(&fc, 0, 300, 0.95, 1.0, &[(LB, 1.0, 50)]);
    let mut m = build(&fc, &p, &f, local_kernels(1), minute_at(&dm, 0.0), frozen(mw), 0.8);
    let t = m.t_s;
    m.z[0].cohorts[LB.index()].push(juveniles(&fc, 30.0, 0.3 * sc.donor_min, t));
    let (adults, juv) = (woody_n(&m, sc.donor_min, f64::INFINITY), woody_n(&m, 0.0, sc.donor_min));
    m.step();
    assert!(close(m.c.aged[LB.index()], adults * (1.0 - (-10.0 / l).exp()), 1e-9), "aged {}", m.c.aged[LB.index()]);
    assert!(close(woody_n(&m, 0.0, sc.donor_min), juv, 1e-12), "juveniles untouched");
}

// ===================================================================================
// e. Thin soil
// ===================================================================================

/// **The deep share** counts the faces with enough soil, less those the woody stands hold
/// (all of them for a species needing as much soil, a pro-rata part for one needing less),
/// over the faces they do not hold.
#[test]
fn the_deep_share_is_the_free_deep_faces() {
    let (_, fc) = resolved();
    let f = {
        let mut f = faces(&fc, 0, 100, 0.95, 1.0, &[]);
        for (i, x) in f.iter_mut().enumerate() {
            x.soil_depth = if i < 60 { 8 } else if i < 80 { 4 } else { 2 };
        }
        f
    };
    let z = zones_of(&fc, &f, 0.8);
    let mut mw = Meadow::default();
    mw.root_soil_min[LB.index()] = 3;
    mw.root_soil_min[Species::Vaulttree.index()] = 6;
    mw.set_deep(&z, &f);
    let mut counts = [0.0; NS];
    counts[LB.index()] = 20.0;
    counts[Species::Vaulttree.index()] = 10.0;
    // Lanternberry: 80 deep faces; the vaulttrees sit on them, so do the lanternberries.
    assert!(close(mw.deep_share(0, LB.index(), 100.0, &counts), (80.0 - 30.0) / 70.0, 1e-12));
    // Vaulttree: 60 deep faces; its own 10, and lanternberries on them pro rata 60/80.
    assert!(close(mw.deep_share(0, Species::Vaulttree.index(), 100.0, &counts), (60.0 - 10.0 - 20.0 * 60.0 / 80.0) / 70.0, 1e-12));
    assert_eq!(mw.deep_share(0, Species::Springturf.index(), 100.0, &counts), 1.0, "no requirement");
}

/// **With every face too thin, no woody seed comes up** — on bare faces or through turf —
/// while the same zone on deep soil recruits.
#[test]
fn thin_soil_shuts_woody_recruitment() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let births = |depth: u32| {
        let mut f = faces(&fc, 0, 300, 0.95, 1.0, &[(TURF, 1.0, 150)]);
        for x in &mut f {
            x.seeds[LB.index()] = 5;
            x.soil_depth = depth;
        }
        let mut mw = Meadow::default();
        mw.root_soil_min[LB.index()] = 3;
        let mut m = build(&fc, &p, &f, local_kernels(1), minute_at(&dm, 1.0), frozen(mw), 0.8);
        for _ in 0..30 {
            m.step();
        }
        m.c.births_seed[LB.index()]
    };
    assert_eq!(births(2), 0.0);
    assert!(births(8) > 0.0, "fixture: deep soil recruits");
}

// ===================================================================================
// f. The contest
// ===================================================================================

/// **A recruit through turf leaves the turf standing until it escapes.** A zone wholly turf
/// with lanternberry seed: with the contest on, one daytime step brings recruits into the
/// turf and overtops none; a contested cohort put past the escape size overtops exactly its
/// number of turf stands at the next step and joins the lanternberry stands; a contested
/// cohort that starves leaves the turf as it was.
#[test]
fn a_contested_recruit_overtops_only_when_it_escapes() {
    let (p, fc) = resolved();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let lsc = fc.species(LB);
    let mw = Meadow { contest_escape: 0.5, contest_tau: 1.0, ..Meadow::default() };
    let mut f = faces(&fc, 0, 300, 0.95, 1.0, &[(TURF, 1.0, 300)]);
    for x in &mut f {
        x.seeds[LB.index()] = 5;
    }
    let mut m = build(&fc, &p, &f, local_kernels(1), minute_at(&dm, 1.0), frozen(mw.clone()), 0.8);
    let turf0 = m.z[0].count(TURF.index());
    m.step();
    assert!(m.c.contested[LB.index()] > 0.0, "recruits came up into the turf");
    assert_eq!(m.c.overtopped[TURF.index()], 0.0, "and overtopped none");
    assert!(close(m.z[0].count(TURF.index()), turf0, 1e-9), "the turf stands");
    // Escape: no seed now, a contested cohort past the escape size.
    let f2 = faces(&fc, 0, 300, 0.95, 1.0, &[(TURF, 1.0, 300)]);
    let mut m = build(&fc, &p, &f2, local_kernels(1), minute_at(&dm, 1.0), frozen(mw.clone()), 0.8);
    let t = m.t_s;
    m.z[0].contest[LB.index()].push(juveniles(&fc, 7.0, 0.6 * lsc.donor_min, t));
    let turf0 = m.z[0].count(TURF.index());
    m.step();
    assert!(close(m.c.escaped[LB.index()], 7.0, 1e-12) && close(m.c.overtopped[TURF.index()], 7.0, 1e-12));
    assert!(close(m.z[0].count(TURF.index()), turf0 - 7.0, 1e-9) && close(woody_n(&m, 0.0, f64::INFINITY), 7.0, 1e-12));
    // Death in the turf: a starving recruit at night.
    let mut m = build(&fc, &p, &f2, local_kernels(1), minute_at(&dm, 0.0), frozen(mw), 0.8);
    let t = m.t_s;
    let mut c = juveniles(&fc, 5.0, lsc.alive_min, t);
    c.st.reserve = 0.0;
    c.st.foliage = 0.0;
    m.z[0].contest[LB.index()].push(c);
    let turf0 = m.z[0].count(TURF.index());
    m.step();
    assert!(close(m.c.contest_died[LB.index()], 5.0, 1e-12), "the recruit died in the turf");
    assert!(close(m.z[0].count(TURF.index()), turf0, 1e-9) && m.c.overtopped[TURF.index()] == 0.0, "the turf kept its faces");
    let _ = package_of(lsc);
}
