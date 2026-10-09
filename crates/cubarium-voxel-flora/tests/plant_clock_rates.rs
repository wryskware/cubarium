//! **The plant clock, rate by rate** (`FloraConfig::step_every`): an independent pass over
//! the per-step quantities `tests/plant_clock.rs` leaves out — timers, per-check draws,
//! cycles and decays — each stepped per tick and on a coarse clock over the same span, on
//! a small hand-built input. Every expectation is the other clock, the config, or a bound
//! derived from the step size and the sampling; nothing here pins a tuned number.

use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, Materials, World};
use cubarium_voxel_flora::{
    Command, DeathCause, Deposit, DepositKind, Face, FaceDir, Flora, FloraConfig, Site, Species, SpurPhase,
    VineSeed,
};

/// The coarse clock under test, in ticks. Any `n > 1` should do; a step of a few ticks is
/// what the schedule runs.
const COARSE: u32 = 5;

/// A 0.25 m world at noon with no rain: soil at `y = 1..=2` across the floor holding
/// `fill` of its pore capacity, then the solids in `cells`.
fn world(width: u32, height: u32, depth: u32, fill: f64, cells: &[((i64, u32, u32), Material)]) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m: 0.25,
        seed: 11,
        rain_m_per_s: 0.0,
        day: DayClock { start_hour: 12.0, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    let cap = Materials::DEFAULT.pore_capacity(Material::Soil) * w.config().voxel_volume();
    // The last write per cell, then water before material: a solid laid over wet soil
    // would push its water out into the air in front of it.
    let mut last: std::collections::BTreeMap<(i64, u32, u32), Material> = Default::default();
    for z in 0..depth {
        for x in 0..width as i64 {
            for y in 1..=2 {
                last.insert((x, y, z), Material::Soil);
            }
        }
    }
    for &(k, material) in cells {
        last.insert(k, material);
    }
    for (&(x, y, z), &material) in &last {
        if material == Material::Soil && fill > 0.0 {
            w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fill * cap });
        }
        w.apply(WorldCommand::SetMaterial { x, y, z, material });
    }
    w
}

/// Field capacity (a share of the pore capacity): wet, not waterlogged.
fn field() -> f64 {
    Materials::DEFAULT.field_capacity(Material::Soil)
}

fn clocked(mut config: FloraConfig, n: u32) -> FloraConfig {
    config.step_every = n;
    config
}

fn tick(flora: &mut Flora, world: &mut World) {
    world.step();
    flora.step_clocked(world, 1);
}

fn closes(flora: &Flora, when: &str) {
    let v = flora.view();
    for (name, got, want) in [
        ("organic", v.organic(), v.ledger.expected_organic()),
        ("mineral", v.mineral(), v.ledger.expected_mineral()),
        ("energy", v.energy(), v.ledger.expected_energy()),
    ] {
        assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "{when}: {name} {got} against {want}");
    }
}

// ------------------------------------------------------------------ vine spurs

const WALL_X: u32 = 6;

/// A rock wall at `x = 6`, `y = 1..=6`, across `z = 0..depth`, its west faces
/// `y = 3..=6` covered by one vine at full leaf with a purse that pays every spur. Spread
/// and sisters are unaffordable and the vine drinks nothing, so only the spur clock moves.
fn colony(mut c: FloraConfig, depth: u32, fill: f64) -> (World, Flora, Vec<Face>) {
    let mut cells = Vec::new();
    for z in 0..depth {
        for y in 1..=6 {
            cells.push(((i64::from(WALL_X), y, z), Material::Rock));
        }
    }
    let w = world(12, 10, depth, fill, &cells);
    c.latticevine.spread_cost = 1e12;
    c.latticevine.sister_cost = 1e12;
    c.latticevine.transpiration_m3_per_s = 0.0;
    let mut flora = Flora::new(c);
    let wf = |y: u32, z: u32| Face::new(WALL_X, y, z, FaceDir::NegX);
    let id = flora
        .seed_vine(&w, VineSeed { root: Site { x: WALL_X - 1, y: 2, z: 0 }, first: wf(3, 0), reserve: 1e4, lineage: None })
        .expect("fixture: the founder");
    let mut faces = vec![wf(3, 0)];
    for z in 0..depth {
        for y in 3..=6 {
            if (y, z) != (3, 0) {
                assert!(flora.cover_face(&w, id, wf(y, z), 1.0), "fixture: cover {:?}", wf(y, z));
                faces.push(wf(y, z));
            }
        }
    }
    (w, flora, faces)
}

/// Flowers opened over `ticks`, every face: phase changes into `Flower`, read every tick.
fn openings(flora: &mut Flora, world: &mut World, faces: &[Face], ticks: u64) -> u64 {
    let phase = |flora: &Flora, f: Face| flora.view().cover.face(f).expect("covered").spur;
    let mut was: Vec<SpurPhase> = faces.iter().map(|&f| phase(flora, f)).collect();
    let mut opened = 0;
    for _ in 0..ticks {
        tick(flora, world);
        for (i, &f) in faces.iter().enumerate() {
            let now = phase(flora, f);
            if now != was[i] && now == SpurPhase::Flower {
                opened += 1;
            }
            was[i] = now;
        }
    }
    opened
}

/// Spur phases of `phase_s` each, two coarse steps long so no phase is shorter than a step.
fn spur_config(phase_s: f64, share: f64) -> FloraConfig {
    let mut c = FloraConfig::default();
    let vc = &mut c.latticevine;
    vc.bud_s = phase_s;
    vc.flower_s = phase_s;
    vc.fruit_s = phase_s;
    vc.spent_s = phase_s;
    vc.spur_share = share;
    c
}

/// **A spur with no rest cycles at the config's period on either clock.** With the rest
/// at zero every face opens a flower at `bud_s` and then once per `spur_cycle_s`, so over
/// `T` seconds it opens `T / cycle` flowers, give or take the one at the window's edge.
/// A per-step dead time between cycles (a draw step, a lost overshoot) lengthens the
/// cycle by a few steps: a few ticks per cycle per tick, five times that on the coarse
/// clock.
#[test]
fn a_spur_without_rest_cycles_at_the_configured_period_on_either_clock() {
    let phase_s = 2.0 * f64::from(COARSE) * DT;
    // A share of one: the cycle is the four phases back to back and the rest is zero.
    let config = spur_config(phase_s, 1.0);
    assert_eq!(config.latticevine.spur_rest_s(), 0.0, "fixture: no rest");
    let cycle = config.latticevine.spur_cycle_s();
    let span = 600u64;
    let per_face_expected = span as f64 * DT / cycle;
    let mut per_face = Vec::new();
    for n in [1, COARSE] {
        let (mut w, mut flora, faces) = colony(clocked(config.clone(), n), 4, field());
        let opened = openings(&mut flora, &mut w, &faces, span);
        closes(&flora, &format!("step_every {n}"));
        per_face.push(opened as f64 / faces.len() as f64);
    }
    for (n, got) in [1, COARSE].into_iter().zip(&per_face) {
        // The window's edge: one opening either way, and the first opening's quantisation.
        assert!(
            (got - per_face_expected).abs() <= 1.5,
            "step_every {n}: {got} flowers per face in {span} ticks, the config's cycle gives {per_face_expected}"
        );
    }
    assert!(
        (per_face[0] - per_face[1]).abs() <= 1.0,
        "flowers per face: per tick {}, coarse {}",
        per_face[0],
        per_face[1]
    );
}

/// **A spur's random rest keeps its mean on either clock.** Each face's flowers are a
/// renewal process of mean period `μ = spur_cycle_s` and variance `σ²` (the exponential
/// rest's, `rest²`): over `T` the count has variance about `T σ² / μ³` per face, the two
/// clocks' totals differ by at most five standard deviations of their difference, plus up
/// to one coarse step of quantisation per cycle.
#[test]
fn a_spur_with_a_random_rest_keeps_its_mean_period_on_either_clock() {
    let phase_s = 2.0 * f64::from(COARSE) * DT;
    // Half the cycle in flower or fruit: a rest as long as the four phases.
    let config = spur_config(phase_s, 0.25);
    let (mu, rest) = (config.latticevine.spur_cycle_s(), config.latticevine.spur_rest_s());
    assert!(rest > 0.0, "fixture: a rest");
    let span = 800u64;
    let t = span as f64 * DT;
    let mut counts = Vec::new();
    let mut faces_n = 0;
    for n in [1, COARSE] {
        let (mut w, mut flora, faces) = colony(clocked(config.clone(), n), 10, field());
        faces_n = faces.len();
        counts.push(openings(&mut flora, &mut w, &faces, span) as f64);
    }
    let f = faces_n as f64;
    let var = f * t * rest * rest / mu.powi(3);
    let step = f64::from(COARSE) * DT;
    let tol = 5.0 * (2.0 * var).sqrt() + f * t * step / (mu * mu);
    let (a, b) = (counts[0], counts[1]);
    assert!(a > 0.0, "no flower opened per tick");
    assert!((a - b).abs() <= tol, "flowers: per tick {a}, coarse {b}, tolerance {tol}");
    // And against the config: `T / μ` per face, within the same sampling and edge.
    let want = f * t / mu;
    assert!((a - want).abs() <= tol + f, "per tick {a} flowers, the config's mean period gives {want}");
}

// ------------------------------------------------------------------ stand fruit

/// A grove of full-grown lanternberries on a wet plain at noon, fruiting fast to a small
/// cap, no seed parcel.
fn grove(config: FloraConfig) -> (World, Flora, Vec<Site>) {
    let w = world(12, 10, 6, field(), &[]);
    let mut flora = Flora::new(config);
    let wood = flora.view().config.species(Species::Lanternberry).wood_max;
    let mut sites = Vec::new();
    for x in (0..12).step_by(2) {
        for z in [1u32, 4] {
            assert!(flora.apply(&w, Command::Seed { x, z, species: Species::Lanternberry, wood }));
            sites.push(flora.view().stands.iter().find(|s| s.site.x == x as u32 && s.site.z == z).expect("seeded").site);
        }
    }
    (w, flora, sites)
}

/// **An uneaten crop falls and the bush rests on the same cycle on either clock.** Each
/// bush's crop hangs `fruit_drop_s` and then it rests a random time of mean
/// `fruit_rest_s`: a renewal process of mean `μ = drop + rest` and variance `rest²`, so the
/// falls over `T` agree within five standard deviations of the difference plus one coarse
/// step of quantisation per cycle.
#[test]
fn an_uneaten_crop_falls_and_rests_on_the_same_cycle_on_either_clock() {
    let step = f64::from(COARSE) * DT;
    let mut c = FloraConfig::default();
    {
        let sc = c.species_mut(Species::Lanternberry);
        sc.propagule_rate = 0.0;
        sc.fruit_cap = 1e-3;
        // Fills its cap within one coarse step.
        sc.fruit_per_s = 2.0 * sc.fruit_cap / step;
        sc.fruit_drop_s = 4.0 * step;
        sc.fruit_rest_s = 4.0 * step;
        // The fall-and-rest cycle alone (FLOWER-FRUIT: no flowering stage).
        sc.flower_s = 0.0;
    }
    let (drop_s, rest) = (c.species(Species::Lanternberry).fruit_drop_s, c.species(Species::Lanternberry).fruit_rest_s);
    let span = 800u64;
    let t = span as f64 * DT;
    let mut falls = Vec::new();
    let mut bushes = 0;
    for n in [1, COARSE] {
        let (mut w, mut flora, sites) = grove(clocked(c.clone(), n));
        bushes = sites.len();
        let fruit = |flora: &Flora, s: Site| flora.view().stand_at(s).expect("the bush").fruit;
        let mut hung: Vec<bool> = sites.iter().map(|&s| fruit(&flora, s) > 0.0).collect();
        let mut fell = 0u64;
        for _ in 0..span {
            tick(&mut flora, &mut w);
            for (i, &s) in sites.iter().enumerate() {
                let now = fruit(&flora, s) > 0.0;
                fell += u64::from(hung[i] && !now);
                hung[i] = now;
            }
        }
        closes(&flora, &format!("step_every {n}"));
        falls.push(fell as f64);
    }
    let mu = drop_s + rest;
    let b = bushes as f64;
    let var = b * t * rest * rest / mu.powi(3);
    let tol = 5.0 * (2.0 * var).sqrt() + b * t * step / (mu * mu);
    let (fine, coarse) = (falls[0], falls[1]);
    assert!(fine > 0.0, "no crop fell per tick");
    assert!((fine - coarse).abs() <= tol, "crops fallen: per tick {fine}, coarse {coarse}, tolerance {tol}");
}

// ------------------------------------------------------------------ curing

/// **A dry stand cures after `cure_after_s` and the drought kills it after `cure_max_s`
/// dormant, on either clock within one plant step.** A dry soil plain; springturf with
/// short cure timers.
#[test]
fn curing_and_drought_death_land_within_one_plant_step_on_either_clock() {
    let mut c = FloraConfig::default();
    {
        let sc = c.species_mut(Species::Springturf);
        sc.cure_after_s = 1.0;
        sc.cure_max_s = 3.0;
    }
    let (after, max) = (c.species(Species::Springturf).cure_after_s, c.species(Species::Springturf).cure_max_s);
    assert!(c.species(Species::Springturf).cure_water > 0.0, "fixture: dry soil is below the cure line");
    let span = ((after + max) / DT) as u64 + 40;
    let mut at = Vec::new();
    for n in [1, COARSE] {
        let mut w = world(8, 8, 3, 0.0, &[]);
        let mut flora = Flora::new(clocked(c.clone(), n));
        let wood = flora.view().config.species(Species::Springturf).wood_max;
        assert!(flora.apply(&w, Command::Seed { x: 3, z: 1, species: Species::Springturf, wood }));
        let site = flora.view().stands[0].site;
        let (mut cured, mut died) = (None, None);
        for k in 1..=span {
            tick(&mut flora, &mut w);
            if cured.is_none() && flora.view().stand_at(site).is_some_and(|s| s.cured) {
                cured = Some(k);
            }
            if died.is_none() && flora.death_log().iter().any(|d| d.site == site && d.cause == DeathCause::Drought) {
                died = Some(k);
            }
        }
        closes(&flora, &format!("step_every {n}"));
        let (cured, died) = (cured.expect("the stand cured"), died.expect("the drought killed it"));
        // Against the config: dry from the first step, so it cures once `cure_after_s` has
        // passed, at the step that crosses it, and dies `cure_max_s` dormant later.
        let step_s = f64::from(n) * DT;
        let cured_s = cured as f64 * DT;
        assert!(
            cured_s >= after - 1e-9 && cured_s <= after + step_s + 1e-9,
            "step_every {n}: cured at {cured_s} s, cure_after_s {after}"
        );
        let dormant_s = (died - cured) as f64 * DT;
        assert!(
            (dormant_s - max).abs() <= step_s + 1e-9,
            "step_every {n}: dormant {dormant_s} s before the drought killed it, cure_max_s {max}"
        );
        at.push((cured, died));
    }
    // Each lands within one of its own steps of the config's time: the two within one
    // coarse step.
    let n = u64::from(COARSE);
    assert!(at[0].0.abs_diff(at[1].0) <= n, "cured: per tick at {}, coarse at {}", at[0].0, at[1].0);
    assert!(at[0].1.abs_diff(at[1].1) <= n, "drought death: per tick at {}, coarse at {}", at[0].1, at[1].1);
}

// ------------------------------------------------------------------ exact closures

/// **Aeration stress and the grazing defence close the same way on either clock.** Both
/// are first-order closures applied as `exp(−r·dt)`. The defence decays toward zero, so
/// read at the ticks both clocks step the two agree to rounding. The stress closes on a
/// target read off the root box, which a coarse step reads once, at its end: on a
/// waterlogged plot draining (the plants drink nothing, so both runs see one world) a
/// change of target lands up to one step early, which moves the stress by at most
/// `1 − exp(−r·h)` of that change. The target falls once from waterlogged to drained, so
/// that share of the whole range bounds the gap; a rate applied per step instead of per
/// second leaves the coarse stress a fifth of the way along, far outside it.
#[test]
fn aeration_stress_and_grazing_defence_agree_at_every_shared_step() {
    let mut c = FloraConfig::default();
    {
        let sc = c.species_mut(Species::Bloomcrown);
        // The plants drink nothing, so both runs see one world.
        sc.transpiration_m3_per_s = 0.0;
        sc.defence_half_life_s = 2.0;
    }
    assert!(c.species(Species::Bloomcrown).stress_rate_per_s > 0.0, "fixture: stress closes");
    let span = 200u64;
    let mut runs: Vec<Vec<(f64, f64)>> = Vec::new();
    for n in [1, COARSE] {
        let mut w = world(8, 8, 3, 1.0, &[]);
        let mut flora = Flora::new(clocked(c.clone(), n));
        let wood = flora.view().config.species(Species::Bloomcrown).wood_max;
        assert!(flora.apply(&w, Command::Seed { x: 3, z: 1, species: Species::Bloomcrown, wood }));
        let site = flora.view().stands[0].site;
        assert!(flora.take_foliage(site, 1e-3).is_some(), "fixture: a bite");
        assert!(flora.view().stand_at(site).expect("stand").defence > 0.0, "fixture: the bite induced a defence");
        let mut seen = Vec::new();
        for k in 1..=span {
            tick(&mut flora, &mut w);
            if k % u64::from(COARSE) == 0 {
                let s = flora.view().stand_at(site).expect("the stand");
                seen.push((s.aeration_stress, s.defence));
            }
        }
        runs.push(seen);
    }
    let last = runs[0].last().copied().expect("steps");
    assert!(last.0 > 0.0, "fixture: the waterlogged stand is stressed");
    let sc = c.species(Species::Bloomcrown);
    let rate = sc.stress_rate_per_s.max(sc.relax_rate_per_s);
    let lag = 1.0 - (-rate * f64::from(COARSE) * DT).exp();
    for (k, (a, b)) in runs[0].iter().zip(&runs[1]).enumerate() {
        let tick = (k as u64 + 1) * u64::from(COARSE);
        assert!((a.0 - b.0).abs() <= lag, "tick {tick}: aeration stress per tick {}, coarse {}, tolerance {lag}", a.0, b.0);
        assert!((a.1 - b.1).abs() <= 1e-9 * a.1.max(1e-12), "tick {tick}: defence per tick {}, coarse {}", a.1, b.1);
    }
}

// ------------------------------------------------------------------ propagation

/// **A donor saves and sends packages at the same rate on either clock.** Full-grown
/// bloomcrowns on a wet plain, a one-package seed and no reserve floor, asking well under
/// one package per coarse step and less over the run than the reserve they start with:
/// what they fund is the ask on either clock, and the packages sent agree within one per
/// donor (the parcel's state at the window's edge).
#[test]
fn donors_fund_and_send_packages_alike_on_either_clock() {
    let mut c = FloraConfig::default();
    {
        let sc = c.species_mut(Species::Bloomcrown);
        sc.propagule_rate = 0.01;
        sc.seed_mass = 1;
        sc.donor_reserve_floor = 0.0;
    }
    let k = Species::Bloomcrown.index();
    let span = 400u64;
    let mut runs = Vec::new();
    let mut donors = 0;
    for n in [1, COARSE] {
        let mut w = world(12, 10, 6, field(), &[]);
        let mut flora = Flora::new(clocked(c.clone(), n));
        let wood = flora.view().config.species(Species::Bloomcrown).wood_max;
        for x in (1..12).step_by(3) {
            for z in [1u32, 4] {
                assert!(flora.apply(&w, Command::Seed { x, z, species: Species::Bloomcrown, wood }));
            }
        }
        donors = flora.view().stands.len();
        for _ in 0..span {
            tick(&mut flora, &mut w);
        }
        closes(&flora, &format!("step_every {n}"));
        let l = &flora.view().ledger;
        runs.push((l.propagule_requested[k], l.propagule_funded[k], l.propagule_landed[k]));
    }
    let sc = c.species(Species::Bloomcrown);
    let package = f64::from(sc.seed_mass.max(1)) * sc.alive_min / sc.propagule_split[0];
    let per_step = sc.propagule_rate * f64::from(COARSE) * DT / (1.0 + sc.build);
    assert!(per_step < package, "fixture: under one package per coarse step");
    let (fine, coarse) = (runs[0], runs[1]);
    assert!(fine.2 >= 2.0 * donors as f64 * package, "fixture: each donor sent a few packages ({})", fine.2 / package);
    // Requested is the rate times the elapsed time: exact on either clock.
    assert!((fine.0 - coarse.0).abs() <= 1e-9 * fine.0, "requested: per tick {}, coarse {}", fine.0, coarse.0);
    let close = |a: f64, b: f64, tol: f64, what: &str| {
        assert!((a - b).abs() <= tol, "{what}: per tick {a}, coarse {b}, tolerance {tol}");
    };
    // `propagule_requested` is the ask net of construction, as `propagule_funded` is.
    assert!((fine.1 - fine.0).abs() <= 1e-9 * fine.1, "fixture: the ask is funded in full: {fine:?}");
    close(fine.1, coarse.1, 1e-9 * fine.1, "funded");
    close(fine.2, coarse.2, donors as f64 * package + 1e-9, "landed");
}

// ------------------------------------------------------------------ the seed bank

/// **Every banked site is checked on its own tick inside a coarse step.** Seeds banked on
/// a wet plain at noon with a check period that is not a multiple of the step: each site
/// arms at one check and sprouts at the next, so on the coarse clock its seedling appears
/// at the end of the step that holds that check — never earlier than per tick, and less
/// than a step later.
#[test]
fn banked_sites_sprout_on_their_own_check_inside_a_coarse_step() {
    let mut c = FloraConfig::default();
    c.seed_check_s = 12.0 * DT;
    assert!(c.sprout_after_arming, "fixture: the arming rule");
    let span = 120u64;
    let mut born: Vec<Vec<(Site, u64)>> = Vec::new();
    for n in [1, COARSE] {
        let mut w = world(12, 8, 6, field(), &[]);
        let mut flora = Flora::new(clocked(c.clone(), n));
        let mut sites = Vec::new();
        for x in 0..12 {
            for z in 0..6 {
                let site = Site { x, y: 2, z };
                assert!(flora.deposit_seeds(site, Species::Bloomcrown, 1.0, 0.0, 0.0));
                sites.push(site);
            }
        }
        let mut first: Vec<Option<u64>> = vec![None; sites.len()];
        for k in 1..=span {
            tick(&mut flora, &mut w);
            for (i, &s) in sites.iter().enumerate() {
                if first[i].is_none() && flora.view().stand_at(s).is_some() {
                    first[i] = Some(k);
                }
            }
        }
        closes(&flora, &format!("step_every {n}"));
        born.push(sites.iter().zip(&first).filter_map(|(&s, f)| f.map(|k| (s, k))).collect());
    }
    assert!(born[0].len() > 10, "fixture: seedlings came up per tick ({})", born[0].len());
    assert_eq!(born[0].len(), born[1].len(), "the same sites sprout on either clock");
    for ((s, a), (s2, b)) in born[0].iter().zip(&born[1]) {
        assert_eq!(s, s2);
        assert!(b >= a && b - a < u64::from(COARSE), "{s:?}: per tick at {a}, coarse at {b}");
    }
}

/// **The overtopping draw is per check on either clock.** Seeds banked under a springturf
/// mat that the seedling may come up through at `overtop_per_check`: the births through
/// the cover over the same span (woody recruits beside it, CONTEST) agree within the
/// binomial spread of the draws.
#[test]
fn seedlings_overtop_a_mat_at_the_same_rate_on_either_clock() {
    let mut c = FloraConfig::default();
    c.seed_check_s = 12.0 * DT;
    {
        let sc = c.species_mut(Species::Bloomcrown);
        sc.overtop_per_check = 0.2;
        sc.overtop_covers = Vec::new();
        // The canopy gate, read under the mat's own shade, is not the subject.
        sc.establish_light_margin = 0.0;
    }
    assert!(c.species(Species::Springturf).ground_cover, "fixture: springturf is a ground cover");
    let k = Species::Bloomcrown.index();
    let span = 300u64;
    let mut over = Vec::new();
    for n in [1, COARSE] {
        let mut w = world(12, 8, 6, field(), &[]);
        let mut flora = Flora::new(clocked(c.clone(), n));
        let wood = flora.view().config.species(Species::Springturf).wood_max;
        for x in 0..12 {
            for z in 0..6 {
                assert!(flora.apply(&w, Command::Seed { x, z, species: Species::Springturf, wood }));
                assert!(flora.deposit_seeds(Site { x: x as u32, y: 2, z }, Species::Bloomcrown, 1.0, 0.0, 0.0));
            }
        }
        for _ in 0..span {
            tick(&mut flora, &mut w);
        }
        closes(&flora, &format!("step_every {n}"));
        over.push(flora.view().ledger.recruits[k] as f64);
    }
    let (a, b) = (over[0], over[1]);
    assert!(a > 0.0, "fixture: seedlings came up through the mat per tick");
    let tol = 5.0 * (2.0 * a.max(b)).sqrt();
    assert!((a - b).abs() <= tol, "recruits: per tick {a}, coarse {b}, tolerance {tol}");
}

// ------------------------------------------------------------------ decomposition

/// **Litter rots at the same per-second rate on either clock.** Litter laid on bare
/// sites, a fast decomposition so the step shows: each clock integrates `dL = −r L dt` to
/// first order, so after `T` the two remainders differ by about `r² (h_c − h_f) T / 2` of
/// the stock; twice that is the tolerance, and a rate applied per step instead of per
/// second (a factor of five) is far outside it.
#[test]
fn litter_decomposes_at_the_same_rate_on_either_clock() {
    let mut c = FloraConfig::default();
    c.decomposition = 0.05;
    let r = c.decomposition;
    let span = 400u64;
    let t = span as f64 * DT;
    let mut left = Vec::new();
    for n in [1, COARSE] {
        let mut w = world(8, 8, 3, field(), &[]);
        let mut flora = Flora::new(clocked(c.clone(), n));
        for x in 0..8 {
            let deposit = Deposit { kind: DepositKind::Litter, organic: 1.0, mineral: 0.01, energy: 1.0 };
            assert!(flora.deposit(Site { x, y: 2, z: 1 }, deposit));
        }
        let before: f64 = flora.view().ground.iter().map(|g| g.litter).sum();
        for _ in 0..span {
            tick(&mut flora, &mut w);
        }
        closes(&flora, &format!("step_every {n}"));
        let after: f64 = flora.view().ground.iter().map(|g| g.litter).sum();
        left.push(after / before);
    }
    let (fine, coarse) = (left[0], left[1]);
    assert!(fine < 1.0 && fine > 0.0, "fixture: litter rotted ({fine})");
    let tol = r * r * f64::from(COARSE) * DT * t * fine;
    assert!((fine - coarse).abs() <= tol, "litter left: per tick {fine}, coarse {coarse}, tolerance {tol}");
}

// ------------------------------------------------------------------ vine dormancy

/// **A vine on a dry root goes dormant after `dry_spell_s` and drops its leaf over
/// `leaf_fall_s`, on either clock within one plant step.** The colony's root in dry soil;
/// short dormancy and leaf-fall timers.
#[test]
fn a_drying_vine_goes_dormant_and_drops_its_leaf_alike_on_either_clock() {
    let mut c = FloraConfig::default();
    c.latticevine.dry_spell_s = 1.0;
    c.latticevine.leaf_fall_s = 2.0;
    let (spell, fall) = (c.latticevine.dry_spell_s, c.latticevine.leaf_fall_s);
    let span = ((spell + fall) / DT) as u64 + 20;
    let mut runs = Vec::new();
    let mut faces_n = 0;
    for n in [1, COARSE] {
        let (mut w, mut flora, faces) = colony(clocked(c.clone(), n), 3, 0.0);
        faces_n = faces.len();
        let mut dormant_at = None;
        let mut leaf = Vec::new();
        for k in 1..=span {
            tick(&mut flora, &mut w);
            if dormant_at.is_none() && flora.view().ledger.vine_dormancies > 0 {
                dormant_at = Some(k);
            }
            if k % u64::from(COARSE) == 0 {
                let cover = &flora.view().cover;
                leaf.push(faces.iter().map(|&f| cover.face(f).map_or(0.0, |cf| cf.leafiness)).sum::<f64>());
            }
        }
        closes(&flora, &format!("step_every {n}"));
        let at = dormant_at.expect("the vine went dormant");
        let step_s = f64::from(n) * DT;
        let at_s = at as f64 * DT;
        assert!(at_s >= spell - 1e-9 && at_s <= spell + step_s + 1e-9, "step_every {n}: dormant at {at_s} s, dry_spell_s {spell}");
        runs.push((at, leaf));
    }
    let n = u64::from(COARSE);
    assert!(runs[0].0.abs_diff(runs[1].0) < n, "dormant: per tick at {}, coarse at {}", runs[0].0, runs[1].0);
    // The leaf falls linearly at `1 / leaf_fall_s` a second from the dormancy: at a shared
    // step the two differ by at most the fall over the onset's offset, every face.
    let tol = faces_n as f64 * (f64::from(COARSE) * DT / fall) + 1e-9;
    for (k, (a, b)) in runs[0].1.iter().zip(&runs[1].1).enumerate() {
        assert!((a - b).abs() <= tol, "tick {}: leaf per tick {a}, coarse {b}, tolerance {tol}", (k as u64 + 1) * n);
    }
    assert!(runs[0].1.last().copied().unwrap_or(1.0) < 1e-9, "fixture: the leaf all fell");
}

