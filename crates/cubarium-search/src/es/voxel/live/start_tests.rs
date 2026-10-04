//! The episode initializer and the E0/E1 starts (`design/handoffs/assembly-step1-2026-10-03.md`,
//! "Tests"): a tiny founded world written as a start, loaded, overridden and burnt in for a
//! few hundred ticks at most.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cubarium_voxel::registry::to_value;
use cubarium_voxel::{Config, Landform, Recipe};
use cubarium_voxel_fauna::Founder;
use cubarium_voxel_sim::LayerConfig;
use cubarium_voxel_sim::found;

use super::assembly::{E1Recipe, construct_fauna, e0_start};
use super::ecology::EcologyParams;
use super::grid::{Axis, cell_ecology, cells};
use super::search::{EvalRun, EvalSpec};
use super::start::*;
use super::world::still_water;

/// A world small enough to found in a few hundred milliseconds that still holds stands,
/// vines, two walking lineages and an egg bank.
fn tiny() -> Config {
    still_water(Config {
        width: 48,
        height: 24,
        depth: 6,
        voxel_m: 0.25,
        landform: Landform::Staged(Recipe::DEFAULT),
        ..Config::default()
    })
}

/// A scratch directory of this test's own, removed first.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cubarium-start-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn heuristic_policy() -> Vec<String> {
    policy_identity(&[]).expect("no centres is every lineage on its heuristic")
}

/// The x1 ecology and the expectation a run under it holds a start to.
fn expect(world: &Config) -> Expect {
    Expect::new(world, &EcologyParams::defaults(), heuristic_policy())
}

/// An E0 start of seed 1 written as `lib/seed-1`; the library directory.
fn library(name: &str) -> PathBuf {
    let lib = scratch(name);
    let cfg = tiny();
    let built = e0_start(&cfg, &EcologyParams::defaults(), 1, heuristic_policy()).expect("founds");
    write_start(&seed_dir(&lib, 1), &built.manifest, &built.world, &built.flora, &built.fauna)
        .expect("writes");
    lib
}

/// A cell ecology: `m`, `p`, and optionally a state-coupled gene (the gut's `k` doubled).
fn ecology(m: f64, p: f64, gut: bool) -> EcologyParams {
    let mut axes = vec![Axis::named("m", &[m]).unwrap(), Axis::named("p", &[p]).unwrap()];
    if gut {
        axes.push(Axis::parse("k:fauna.founders.*.gut.k=x2").unwrap());
    }
    cell_ecology(&cells(&axes)[0], None).expect("decodes")
}

fn glob(g: &str, path: &str) -> bool {
    let (g, p): (Vec<&str>, Vec<&str>) = (g.split('.').collect(), path.split('.').collect());
    g.len() == p.len() && g.iter().zip(&p).all(|(a, b)| *a == "*" || a == b)
}

/// Every numeric leaf of both layers' configs, by registry path.
fn leaves(l: &LayerConfig) -> Vec<(String, f64)> {
    let mut out = cubarium_voxel_flora::registry().numeric_leaves(&to_value(&l.flora).unwrap());
    out.extend(cubarium_voxel_fauna::registry().numeric_leaves(&to_value(&l.fauna).unwrap()));
    out
}

fn leaf(l: &LayerConfig, g: &str) -> Vec<f64> {
    leaves(l).into_iter().filter(|(p, _)| glob(g, p)).map(|(_, v)| v).collect()
}

fn spec_on(lib: &Path, burn_in: u64) -> EvalSpec {
    let cfg = tiny();
    let mut spec = EvalSpec::new(cfg.clone(), 20, 20);
    spec.start = StartSet::Library(Arc::new(StartLibrary::open(lib, expect(&cfg)).expect("opens")));
    spec.burn_in = BurnIn { min_ticks: burn_in, align_ticks: 0 };
    spec.stop_on_loss = false;
    spec
}

/// The start as written loads; each kind of incompatibility is refused by name, and none
/// is repaired. A difference in a rate-only parameter is not an incompatibility.
#[test]
fn an_incompatible_start_is_rejected() {
    let lib = library("reject");
    let dir = seed_dir(&lib, 1);
    let cfg = tiny();
    let ok = load_start(&dir, &expect(&cfg)).expect("the start loads under its own run");
    assert_eq!(ok.manifest.seed, 1);
    assert_eq!(ok.world.tick(), ok.manifest.tick);

    // A run at another rate still loads it: rates are the candidate's, not the start's.
    let rates = Expect::new(&cfg, &ecology(2.0, 0.5, false), heuristic_policy());
    load_start(&dir, &rates).expect("rate-only differences are not incompatible");

    // Mismatched build.
    let mut e = expect(&cfg);
    e.identity.build = "another-build".into();
    assert!(matches!(load_start(&dir, &e), Err(Reject::Build { .. })), "build");

    // Mismatched policy: one lineage driven by a centre the start never had.
    let mut e = expect(&cfg);
    e.identity.policy[0] = format!("{}=centre:0123456789abcdef", Founder::ALL[0].name());
    assert!(matches!(load_start(&dir, &e), Err(Reject::Policy { .. })), "policy");

    // Mismatched world config.
    let mut other = cfg.clone();
    other.day.start_hour = 3.0;
    let e = Expect::new(&other, &EcologyParams::defaults(), heuristic_policy());
    assert!(matches!(load_start(&dir, &e), Err(Reject::World { .. })), "world");

    // A state-coupled parameter differs from the start's: the run's base is not the start's.
    let e = Expect::new(&cfg, &ecology(1.0, 1.0, true), heuristic_policy());
    match load_start(&dir, &e) {
        Err(Reject::Params { path, .. }) => assert!(glob("fauna.founders.*.gut.k", &path), "{path}"),
        other => panic!("a state-coupled mismatch is refused, got {:?}", other.err()),
    }

    // Wrong schema, recorded in the manifest.
    let text = std::fs::read_to_string(dir.join(MANIFEST)).unwrap();
    let mut m: Manifest = toml::from_str(&text).unwrap();
    m.identity.fauna_schema += 1;
    std::fs::write(dir.join(MANIFEST), toml::to_string(&m).unwrap()).unwrap();
    assert!(matches!(load_start(&dir, &expect(&cfg)), Err(Reject::Schema { .. })), "schema");
    std::fs::write(dir.join(MANIFEST), text).unwrap();

    // A missing layer.
    let fauna = layer_files(&dir, ok.manifest.tick)[2].clone();
    std::fs::remove_file(&fauna).unwrap();
    assert!(matches!(load_start(&dir, &expect(&cfg)), Err(Reject::MissingLayer(_))), "layer");
    // And a library holding it refuses to open rather than skip the seed.
    assert!(StartLibrary::open(&lib, expect(&cfg)).is_err(), "the library refuses");
    let _ = std::fs::remove_dir_all(&lib);
}

/// The override moves exactly the rate-only paths to the candidate's values, freezes (and
/// records) a state-coupled gene at the start's value, and leaves every stock, age and
/// state as loaded.
#[test]
fn rate_only_overrides_change_the_rates_and_leave_every_stock_untouched() {
    let lib = library("rates");
    let cfg = tiny();
    let mut s = load_start(&seed_dir(&lib, 1), &expect(&cfg)).expect("loads");
    let start = layers_of(&s.flora, &s.fauna);
    let candidate = ecology(2.0, 0.5, true).layers(cfg.voxel_m);
    let o = rate_override(&start, &candidate).expect("overrides");

    for (path, v) in leaves(&o.installed) {
        let want = if is_rate_path(&path) {
            leaves(&candidate).into_iter().find(|(p, _)| *p == path).unwrap().1
        } else {
            leaves(&start).into_iter().find(|(p, _)| *p == path).unwrap().1
        };
        assert_eq!(v, want, "{path}");
    }
    let upkeep = "fauna.founders.*.core.maintenance_per_s";
    assert!(leaf(&o.installed, upkeep).iter().zip(leaf(&start, upkeep)).any(|(a, b)| *a == 2.0 * b && b > 0.0));
    assert_eq!(leaf(&o.installed, "fauna.founders.*.gut.k"), leaf(&start, "fauna.founders.*.gut.k"));
    assert!(o.frozen.iter().any(|c| glob("fauna.founders.*.gut.k", &c.path)), "the gut gene is frozen");
    assert!(o.frozen.iter().all(|c| !is_rate_path(&c.path)));
    assert!(o.changed.iter().all(|c| is_rate_path(&c.path)) && !o.changed.is_empty());

    let (animals, clutches) = (s.fauna.view().animals.to_vec(), s.fauna.view().clutches.to_vec());
    let stands = s.flora.view().stands.to_vec();
    let vines = s.flora.view().cover.vines().len();
    let (organic, mineral) = (s.flora.view().organic(), s.flora.view().mineral());
    let world = s.world.save();
    install(&mut s.flora, &mut s.fauna, &o.installed).expect("installs");
    assert_eq!(layers_of(&s.flora, &s.fauna), o.installed, "every layer runs the installed set");
    assert_eq!(s.fauna.view().animals, &animals[..], "bodies, ages, stores, states");
    assert_eq!(s.fauna.view().clutches, &clutches[..]);
    assert_eq!(s.flora.view().stands, &stands[..]);
    assert_eq!(s.flora.view().cover.vines().len(), vines);
    assert_eq!((s.flora.view().organic(), s.flora.view().mineral()), (organic, mineral));
    assert_eq!(s.world.save(), world);
    let _ = std::fs::remove_dir_all(&lib);
}

/// Water, plant and animal ledgers close at the start's writing, after the load, after
/// the override and after the burn-in, and the scores' baseline is the burn-in's end.
#[test]
fn every_ledger_is_conserved_across_load_override_and_burn_in() {
    let lib = library("ledgers");
    let spec = spec_on(&lib, 150);
    let run = EvalRun::begin(&ecology(2.0, 0.5, false), 1, &[], &spec);
    let r = run.start_record();
    let stages: Vec<&str> = r.residuals.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(stages, ["load", "override", "burn-in"]);
    for (stage, res) in &r.residuals {
        assert!(res.worst() <= 1e-9, "{stage}: {res:?}");
    }
    assert!(Residuals::of(run.sim().world(), run.sim().flora(), run.sim().fauna()).worst() <= 1e-9);
    assert_eq!(r.burn_in_ticks, 150);
    assert_eq!(r.scoring_tick, r.start_tick + 150);
    assert_eq!(run.sim().world().tick(), r.scoring_tick);
    // Checkpoint-relative: the first baseline is the burn-in's end.
    assert_eq!(run.baselines().first().map(|b| b.tick), Some(r.scoring_tick));
    assert!(r.frozen.is_empty() && r.changed > 0);
    let _ = std::fs::remove_dir_all(&lib);
}

/// Two candidates on one start begin from the same state; only their rates differ.
#[test]
fn two_candidates_on_the_same_start_see_identical_initial_state() {
    let lib = library("paired");
    let spec = spec_on(&lib, 0);
    let a = EvalRun::begin(&ecology(1.0, 1.0, false), 1, &[], &spec);
    let b = EvalRun::begin(&ecology(4.0, 2.0, false), 1, &[], &spec);
    let (sa, sb) = (a.sim(), b.sim());
    assert_eq!(sa.world().save(), sb.world().save());
    assert_eq!(sa.fauna().view().animals, sb.fauna().view().animals);
    assert_eq!(sa.fauna().view().clutches, sb.fauna().view().clutches);
    assert_eq!(sa.fauna().view().ledger, sb.fauna().view().ledger);
    assert_eq!(sa.flora().view().stands, sb.flora().view().stands);
    assert_eq!(sa.flora().view().organic(), sb.flora().view().organic());
    assert_eq!(a.start_record().start_params, b.start_record().start_params);
    assert_ne!(a.start_record().installed_params, b.start_record().installed_params);
    assert_eq!(a.start_record().scoring_tick, b.start_record().scoring_tick);
    assert_ne!(sa.fauna().config(), sb.fauna().config(), "the rates are each candidate's");
    let _ = std::fs::remove_dir_all(&lib);
}

/// A plants-only tiny world, its support faces and no history of water.
fn plants_only() -> (cubarium_voxel::World, cubarium_voxel_flora::Flora, cubarium_voxel_fauna::Fauna) {
    let cfg = tiny();
    let eco = EcologyParams::defaults();
    let f = found::found(&cfg, &eco.layers(cfg.voxel_m), [0; Founder::COUNT], found::Seeds::Asked(1), false);
    (f.world, f.flora, f.fauna)
}

fn injected_now(flora: &cubarium_voxel_flora::Flora, fauna: &cubarium_voxel_fauna::Fauna) -> [f64; 3] {
    let (fl, fa) = (flora.view().ledger, fauna.view().ledger);
    [
        fa.introduced_organic_in + fl.deposited_organic_in + fl.seeded_organic_in,
        fa.introduced_mineral_in + fl.deposited_mineral_in + fl.seeded_mineral_in,
        fa.introduced_energy_in + fl.deposited_energy_in + fl.seeded_energy_in,
    ]
}

/// The E1 constructor books every body, reserve, stomach, egg and starter tile it injects,
/// draws juveniles small and never gravid, and keeps the layers' ledgers closed.
#[test]
fn the_e1_fauna_constructor_books_what_it_injects_and_never_places_a_gravid_juvenile() {
    let (world, mut flora, mut fauna) = plants_only();
    let sites = cubarium_voxel_sim::habitat::support_sites(&world);
    let wettest = vec![0.0; sites.len()];
    let today = EcologyParams::defaults().fauna().founder_counts();
    let before = injected_now(&flora, &fauna);
    let recipe = E1Recipe { juvenile_share: 0.5, ..E1Recipe::default() };
    let report = construct_fauna(&world, &mut flora, &mut fauna, &recipe, today, &sites, &wettest);
    let after = injected_now(&flora, &fauna);
    for k in 0..3 {
        let booked = after[k] - before[k];
        let said = [report.injected.organic, report.injected.mineral, report.injected.energy][k];
        assert!((booked - said).abs() <= 1e-9 * booked.abs().max(1.0), "currency {k}: {booked} vs {said}");
    }
    assert!(report.injected.organic > 0.0);
    assert!(Residuals::of(&world, &flora, &fauna).worst() <= 1e-9);
    let cfg = fauna.config();
    let mut juveniles = 0;
    for a in fauna.view().animals.iter() {
        let core = &cfg.founder(a.founder.expect("a founder")).core;
        assert!(a.reproduction.escrow.is_none(), "nobody arrives gravid");
        if a.body < core.birth_body {
            juveniles += 1;
            assert!(a.body >= core.body_min);
        }
    }
    let said: usize = report.bodies.iter().map(|p| p.juveniles).sum();
    assert_eq!(juveniles, said);
    assert!(juveniles > 0, "the mix drew juveniles");
    // Roughly three times today's walking founders were asked for.
    for p in &report.bodies {
        let f = Founder::ALL.iter().find(|f| f.name() == p.lineage).unwrap();
        assert_eq!(p.wanted, (recipe.consumer_multiplier * today[f.index()] as f64).round() as usize);
    }
    assert!(report.eggs > 0, "the egg bank as today's founding");
}

/// Every body asked for is placed, refused, unplaceable or over the cap — counted, never
/// retried into another count — and the caps hold.
#[test]
fn a_failed_placement_is_recorded_and_counted() {
    let (world, mut flora, mut fauna) = plants_only();
    let sites = cubarium_voxel_sim::habitat::support_sites(&world);
    let wettest = vec![0.0; sites.len()];
    let today = EcologyParams::defaults().fauna().founder_counts();
    let recipe = E1Recipe { consumer_multiplier: 40.0, max_bodies: 30, max_attempts: 60, ..E1Recipe::default() };
    let report = construct_fauna(&world, &mut flora, &mut fauna, &recipe, today, &sites, &wettest);
    let mut placed = 0;
    for p in &report.bodies {
        assert_eq!(p.wanted, p.placed + p.failed + p.unplaceable + p.capped, "{p:?}");
        assert!(p.attempted == p.placed + p.failed, "{p:?}");
        placed += p.placed;
    }
    assert!(report.bodies.iter().any(|p| p.unplaceable + p.capped + p.failed > 0), "a shortfall is on record");
    assert!(placed <= recipe.max_bodies);
    assert!(report.attempts <= recipe.max_attempts);
    let bodies = fauna.view().animals.len();
    assert_eq!(bodies, placed + report.pack.placed);
    // The pack is one, sized by its rule, its outcome recorded either way.
    assert!(report.pack.placed == 0 || report.pack.placed == report.pack.size);
    assert!(report.pack.placed > 0 || report.pack.failed.is_some());
}

/// Scoring windows start on the day's window grid: the first boundary at or after the
/// minimum burn-in.
#[test]
fn the_burn_in_ends_on_the_day_phase_grid() {
    let day = cubarium_voxel::weather::DayClock { start_hour: 0.0, length_min: 120.0, ..Default::default() };
    let hz = 20u64;
    let window = 30 * 60 * hz;
    // From tick 0 (midnight), at least 10 minutes: the 30-minute boundary.
    assert_eq!(burn_in_ticks(0, 10 * 60 * hz, window, &day), window);
    // From 31 minutes, at least 10 (41): the next boundary is 60.
    let start = 31 * 60 * hz;
    assert_eq!(start + burn_in_ticks(start, 10 * 60 * hz, window, &day), 60 * 60 * hz);
    // Already on a boundary with no minimum: no burn-in.
    assert_eq!(burn_in_ticks(window, 0, window, &day), 0);
    // No alignment: the minimum.
    assert_eq!(burn_in_ticks(start, 123, 0, &day), 123);
}
