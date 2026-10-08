//! The one safe load, the staged runner's liveness check and pause/resume, and the
//! founding table's scaling with land area. Hand-built worlds, a few hundred ticks at
//! most; nothing here reads a tuned default.

use cubarium_voxel::weather::DayMode;
use cubarium_voxel::{Config, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig, Founder};
use cubarium_voxel_flora::{Flora, Species};

use super::{Ended, Runner, StageConfig, Window};
use crate::{Sim, SimConfig, habitat, saved};

/// A small authored ring with no night (every lineage awake), seeded with the habitat.
fn seeded(width: u32, counts: Option<[usize; Founder::COUNT]>, reference_land_m2: f64) -> (World, Flora, Fauna, habitat::Seeded) {
    let mut cfg = Config { width, height: 24, depth: 6, ..Config::default() };
    cfg.day.mode = DayMode::Off;
    let mut world = crate::scene::authored(cfg);
    let mut flora = Flora::new(Default::default());
    let mut fauna_cfg = FaunaConfig::placeholder();
    fauna_cfg.founding.reference_land_m2 = reference_land_m2;
    let mut fauna = Fauna::new(fauna_cfg);
    let s = match counts {
        Some(c) => habitat::seed_with_founder_counts(&mut world, &mut flora, &mut fauna, c),
        None => habitat::seed(&mut world, &mut flora, &mut fauna),
    };
    (world, flora, fauna, s)
}

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("cubarium-staged-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn founder_bodies(fauna: &Fauna) -> Vec<(u64, Founder)> {
    fauna.view().animals.iter().filter_map(|a| a.founder.map(|f| (a.id, f))).collect()
}

/// **A saved and reloaded habitat drives every founder body**: a controller on every
/// body and a birth factory for every lineage, through the directory loader and the
/// byte loader every tool uses. A raw `Fauna::load` (what the frozen study did) is
/// caught by the same check.
#[test]
fn a_reloaded_habitat_drives_every_founder_body() {
    let (world, flora, fauna, _) = seeded(64, None, 0.0);
    assert!(!founder_bodies(&fauna).is_empty(), "the seeded habitat has founder bodies");
    let dir = scratch("load");
    saved::save(&dir, &world, &flora, &fauna, 7).expect("saves");
    let s = saved::load(&dir, &[]).expect("loads");
    assert_eq!(s.minute, 7);
    let raw = Fauna::load(&std::fs::read(dir.join("fauna.bin")).unwrap()).unwrap();
    let bytes = |n: &str| std::fs::read(dir.join(n)).unwrap();
    let (_, _, decoded) = saved::decode(&bytes("world.bin"), &bytes("flora.bin"), &bytes("fauna.bin"), &[]).expect("decodes");
    std::fs::remove_dir_all(&dir).ok();
    for layer in [&s.fauna, &decoded] {
        saved::driven(layer).expect("every body driven");
        for (id, f) in founder_bodies(layer) {
            assert!(layer.has_controller(id), "body {id} ({f:?}) has a controller");
            assert!(layer.has_founder_factory(f), "{f:?} births are driven");
        }
    }
    assert!(saved::driven(&raw).is_err(), "a raw decode leaves the bodies undriven, and the check says so");
}

/// A policy-driven save is refused unless the caller names that lineage as the one it
/// drives by policy.
#[test]
fn a_policy_driven_save_needs_its_driver() {
    let (_, _, mut fauna, _) = seeded(64, None, 0.0);
    let f = founder_bodies(&fauna)[0].1;
    fauna.set_policy_driven(f, true);
    assert!(saved::restore(&mut fauna.clone(), &[]).is_err(), "no driver named: refused");
    saved::restore(&mut fauna, &[f]).expect("its driver named: restored");
}

fn stage(dir: &std::path::Path, extra: &[&str]) -> (StageConfig, Vec<String>) {
    let mut args = vec!["test".to_string(), format!("run={}", dir.display()), "check_min=1".into(), "segment_min=1".into()];
    args.extend(extra.iter().map(|s| s.to_string()));
    let mut cfg = StageConfig::from_args(&args).unwrap().expect("staged");
    cfg.ticks_per_min = 100;
    (cfg, args)
}

/// **The liveness check fails a frozen world**: bodies with no controller are refused at
/// the load, and, watched over a window, a lineage that neither moved nor fed is called
/// frozen.
#[test]
fn the_liveness_check_fails_frozen_animals() {
    let (world, flora, mut fauna, _) = seeded(64, None, 0.0);
    for (id, _) in founder_bodies(&fauna) {
        fauna.take_controller(id);
    }
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::with_threads(1));
    let dir = scratch("frozen");
    let (cfg, args) = stage(&dir, &[]);
    let err = Runner::start(cfg.clone(), &args, &mut sim, 0, 3).err().expect("the load check refuses it");
    assert!(err.contains("no controller"), "{err}");
    assert!(dir.join("FAILED").is_file());
    // Past the load check (as if the bodies had drivers there), the window still finds
    // them frozen: a factory with no body controllers.
    habitat::install_heuristics(sim.fauna_mut());
    let mut window = Window::open(&sim);
    for _ in 0..cfg.ticks_per_min {
        sim.step();
        window.watch(&sim);
    }
    let verdict = window.verdict(&sim, &cfg);
    std::fs::remove_dir_all(&dir).ok();
    let why = verdict.expect_err("frozen bodies fail");
    assert!(why.contains("frozen"), "{why}");
}

/// **A lineage idle by choice passes; the same idleness without controllers does not**: a
/// window in which every body was resting (a pack lying up at its den) and none moved or
/// fed is noted as idle while every body is driven, and called frozen once the bodies lose
/// their controllers.
#[test]
fn a_resting_driven_lineage_is_idle_and_an_undriven_one_is_frozen() {
    let (world, flora, fauna, _) = seeded(64, None, 0.0);
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::with_threads(1));
    let dir = scratch("idle");
    let (cfg, _) = stage(&dir, &[]);
    std::fs::remove_dir_all(&dir).ok();
    // A window with no ticks in it: nobody moved or fed, and every sample says resting.
    let rested = |sim: &Sim| {
        let mut window = Window::open(sim);
        for f in Founder::ALL {
            let s = &mut window.states[f.index()];
            let n: u64 = s.values().sum();
            s.clear();
            s.insert("Resting".into(), n.max(1));
            window.awake[f.index()] = true;
        }
        window
    };
    let ok = rested(&sim).verdict(&sim, &cfg).expect("resting, driven lineages pass");
    assert!(ok.contains("idle"), "{ok}");
    for (id, _) in founder_bodies(sim.fauna()) {
        sim.fauna_mut().take_controller(id);
    }
    let why = rested(&sim).verdict(&sim, &cfg).expect_err("resting, undriven lineages fail");
    assert!(why.contains("frozen"), "{why}");
}

/// **A sated grazer is not asked to bite; one that wanted food is**: in a window where no
/// body ate, a grazer lineage whose bodies never had their eating latch on passes as sated,
/// and the same window counted as wanting fails for want of plant bites. Every lineage is
/// movement-exempt here, so only the grazing item is under test.
#[test]
fn a_sated_grazer_is_not_asked_to_bite_but_a_wanting_one_is() {
    let (world, flora, fauna, _) = seeded(64, None, 0.0);
    let sim = Sim::ambient(world, flora, fauna, SimConfig::with_threads(1));
    let dir = scratch("sated");
    let names: Vec<&str> = Founder::ALL.iter().map(|f| f.name()).collect();
    let still = format!("still={}", names.join(","));
    let (cfg, _) = stage(&dir, &[still.as_str()]);
    std::fs::remove_dir_all(&dir).ok();
    // A window with no ticks in it: nobody ate, and every lineage was seen awake.
    let awake = |sim: &Sim| {
        let mut window = Window::open(sim);
        for f in Founder::ALL {
            let s = &mut window.states[f.index()];
            s.clear();
            s.insert("Walking".into(), 1);
            window.awake[f.index()] = true;
        }
        window
    };
    let ok = awake(&sim).verdict(&sim, &cfg).expect("sated grazers pass");
    assert!(ok.contains("sated"), "{ok}");
    // The same window, with each lineage's wanting count moved since its start.
    let mut wanting = awake(&sim);
    wanting.wanting = [u64::MAX; Founder::COUNT];
    let why = wanting.verdict(&sim, &cfg).expect_err("grazers that wanted food and took no bite fail");
    assert!(why.contains("no grazer bit a plant"), "{why}");
}

/// **An expected counter that stays zero fails the check**: the arm asks for burrs, and
/// no species carries any.
#[test]
fn an_expected_counter_that_stays_zero_fails() {
    let (world, mut flora, fauna, _) = seeded(64, None, 0.0);
    let mut fc = flora.config().clone();
    for s in Species::ALL {
        fc.species_mut(s).burr_pickup = 0.0;
    }
    flora.replace_config(fc).expect("valid");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::with_threads(1));
    let dir = scratch("expect");
    let (cfg, args) = stage(&dir, &["expect=burrs_picked"]);
    let mut runner = Some(Runner::start(cfg, &args, &mut sim, 0, 3).expect("a driven world passes the load check"));
    let ended = super::drive(&mut sim, &mut runner, 0, 3, |_| {}, |_, _| {});
    std::fs::remove_dir_all(&dir).ok();
    match ended {
        Ended::Failed(why) => assert!(why.contains("burrs_picked did not move"), "{why}"),
        other => panic!("expected a failed check, got {other:?}"),
    }
    // An unknown counter is a failure too, not a pass.
    let none = serde_json::json!({});
    assert!(super::counter_delta("no_such_counter", (&none, &none), (&none, &none)).is_err());
}

/// **Stopped and resumed reaches the same state class as unstopped**: the same tick, the
/// ledgers conserved, the same animals present (no bit-identical requirement).
#[test]
fn a_stopped_and_resumed_run_matches_an_unstopped_one() {
    // Shredders only (no grazer to be asked for a bite in a few hundred ticks), exempt
    // from the movement test: this is about the stop and the resume.
    let mut counts = [0; Founder::COUNT];
    counts[Founder::Blind.index()] = 4;
    let (world, flora, fauna, _) = seeded(64, Some(counts), 0.0);
    let end = 3;
    // Unstopped.
    let a = scratch("straight");
    // A cloned layer has no minds (controllers are not cloned): it gets them back as a
    // load does.
    let mut twin = fauna.clone();
    saved::restore(&mut twin, &[]).unwrap();
    let mut sim = Sim::ambient(world.clone(), flora.clone(), twin, SimConfig::with_threads(1));
    let (cfg, args) = stage(&a, &["still=littershredder"]);
    let mut r = Some(Runner::start(cfg.clone(), &args, &mut sim, 0, end).unwrap());
    assert_eq!(super::drive(&mut sim, &mut r, 0, end, |_| {}, |_, _| {}), Ended::Done);
    // Stopped at the first boundary, then resumed from its checkpoint.
    let b = scratch("stopped");
    std::fs::create_dir_all(&b).unwrap();
    std::fs::write(b.join("STOP"), "").unwrap();
    let mut first = Sim::ambient(world, flora, fauna, SimConfig::with_threads(1));
    let (cfg_b, args_b) = stage(&b, &args[4..].iter().map(String::as_str).collect::<Vec<_>>());
    let mut r = Some(Runner::start(cfg_b.clone(), &args_b, &mut first, 0, end).unwrap());
    assert_eq!(super::drive(&mut first, &mut r, 0, end, |_| {}, |_, _| {}), Ended::Paused);
    let resumed = super::resume_args(&["test".into(), format!("resume={}", b.display())], &[]).unwrap();
    assert!(super::resumed(&resumed));
    let load = resumed.iter().find_map(|x| x.strip_prefix("load=")).expect("the latest checkpoint");
    let s = saved::load(std::path::Path::new(load), &[]).unwrap();
    let minute0 = s.minute;
    assert!(minute0 > 0 && minute0 < end, "paused at a boundary");
    let mut second = Sim::ambient(s.world, s.flora, s.fauna, SimConfig::with_threads(1));
    let mut cfg_r = StageConfig::from_args(&resumed).unwrap().unwrap();
    cfg_r.ticks_per_min = cfg_b.ticks_per_min;
    let mut r = Some(Runner::start(cfg_r, &resumed, &mut second, minute0, 999).unwrap());
    assert_eq!(r.as_ref().unwrap().end_minute(), end, "a resumed run keeps its own end");
    assert_eq!(super::drive(&mut second, &mut r, minute0, end, |_| {}, |_, _| {}), Ended::Done);
    for d in [&a, &b] {
        assert!(d.join("DONE").is_file());
        std::fs::remove_dir_all(d).ok();
    }
    assert_eq!(second.world().tick(), sim.world().tick(), "the same tick");
    for s in [&sim, &second] {
        let (w, fl, fa) = s.layers();
        assert!(super::Residuals::of(w, fl, fa).worst() <= 1e-6, "ledgers conserve");
    }
    let lineages = |s: &Sim| {
        let mut l: Vec<Founder> = founder_bodies(s.fauna()).into_iter().map(|b| b.1).collect();
        l.dedup();
        l
    };
    assert_eq!(lineages(&second), lineages(&sim), "the same animals present");
}

/// **The founding table scales with land area**: on two ring sizes the asked-for
/// founders per square metre of dry land agree within rounding, and the scale is the
/// land over the table's own area.
#[test]
fn the_founding_table_scales_with_land_area() {
    let mut counts = [0; Founder::COUNT];
    counts[Founder::Blind.index()] = 10;
    counts[Founder::Browser.index()] = 7;
    let reference = 20.0;
    let (_, _, _, small) = seeded(48, Some(counts), reference);
    let (_, _, _, large) = seeded(96, Some(counts), reference);
    for s in [&small, &large] {
        assert!(s.land_m2 > 0.0);
        assert!((s.land_scale - s.land_m2 / reference).abs() < 1e-12);
    }
    assert!(large.land_m2 > small.land_m2);
    for f in [Founder::Blind, Founder::Browser] {
        let i = f.index();
        for s in [&small, &large] {
            let exact = counts[i] as f64 * s.land_scale;
            assert!((s.wanted[i] as f64 - exact).abs() <= 0.5 + 1e-9, "{f:?}: {} for {exact}", s.wanted[i]);
        }
        // Per area: each side is within half a founder of the table's own density.
        let density = counts[i] as f64 / reference;
        for s in [&small, &large] {
            assert!((s.wanted[i] as f64 / s.land_m2 - density).abs() <= 0.5 / s.land_m2 + 1e-9);
        }
    }
    // Zero stays zero: a lineage the map leaves out is not founded by rounding up.
    assert_eq!(small.wanted[Founder::Lanternjaw.index()], 0);
    // With no reference the table is taken as written.
    let (_, _, _, as_written) = seeded(48, Some(counts), 0.0);
    assert_eq!(as_written.wanted, counts);
}

