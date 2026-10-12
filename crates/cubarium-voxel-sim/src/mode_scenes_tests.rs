//! BRAIN-NEXT step 3: each staged scene triggers its mode in the heuristic. The mode is
//! asserted, not when it comes.

use crate::mode_scenes::{Config, ModeScene, found};
use cubarium_voxel_fauna::intent::menu_of;

/// Run a scene under its heuristics with the tap on, up to `cap` ticks or until a focal
/// body's rules propose the scene's intent; the labels seen, by name.
fn labels(scene: ModeScene, seed: u64, cap: u64) -> (bool, std::collections::BTreeMap<&'static str, usize>) {
    let mut e = found(&Config::default(), scene, seed).unwrap_or_else(|why| panic!("{}: {why}", scene.name()));
    e.sim.fauna_mut().start_tap();
    let menu = menu_of(scene.founder());
    let mut seen = std::collections::BTreeMap::new();
    let mut hit = false;
    let mut n = 0;
    while n < cap && e.step() {
        n += 1;
        let tick = e.sim.fauna().tick();
        let tap = e.sim.fauna().tap().unwrap();
        for id in &e.focal {
            if let Some(d) = tap.decisions(*id).and_then(<[_]>::last).filter(|d| d.tick == tick) {
                let name = menu.name(usize::from(d.proposal));
                *seen.entry(name).or_insert(0) += 1;
                hit |= name == scene.intent();
            }
        }
        if hit {
            break;
        }
    }
    eprintln!("{} seed {seed}: hit={hit} after {n} ticks: {seen:?}", scene.name());
    (hit, seen)
}

/// Every scene's mode arises under the heuristic, on several seeds, within a few hundred
/// ticks.
#[test]
fn each_staged_scene_triggers_its_mode_in_the_heuristic() {
    for scene in ModeScene::ALL {
        for seed in [1, 2, 3] {
            let (hit, seen) = labels(scene, seed, 400);
            assert!(hit, "{} seed {seed}: the rules never proposed {} ({seen:?})", scene.name(), scene.intent());
        }
    }
}

/// One seed, one start; the arena and every placement come from it.
#[test]
fn a_scene_is_one_start_per_seed_and_its_knobs_are_checked() {
    for scene in ModeScene::ALL {
        let (a, b) = (found(&Config::default(), scene, 5).unwrap(), found(&Config::default(), scene, 5).unwrap());
        assert!(a.sim.fauna().save() == b.sim.fauna().save(), "{}", scene.name());
        assert_eq!((a.focal.clone(), a.others.clone(), a.site), (b.focal.clone(), b.others.clone(), b.site));
        assert!(!a.focal.is_empty());
        let c = found(&Config::default(), scene, 6).unwrap();
        assert!(a.sim.fauna().save() != c.sim.fauna().save(), "{}: another seed, another start", scene.name());
        assert_eq!(ModeScene::parse(scene.name()), Some(scene));
        assert!(menu_of(scene.founder()).index(scene.intent()).is_some());
    }
    let bad = Config { troop: 1, ..Config::default() };
    assert!(bad.validate().is_err(), "a troop of one is refused by the registry");
    let cramped = Config { width: 8, ..Config::default() };
    assert!(cramped.validate().is_err());
}

/// BRAIN-FIX step 1: the groom scene gives grooming room to finish. Its host holds still at
/// its meal, and its troopmates, facing it, groom the larva off under the rules within the
/// grooming contact time and a little walking (`grooming.groom_s`, read from the config).
#[test]
fn in_the_groom_scene_the_rules_groom_the_larva_off() {
    use cubarium_voxel_fauna::Founder;
    for seed in [1, 2, 3] {
        let mut e = found(&Config::default(), ModeScene::Groom, seed).unwrap();
        let host = e.others[0];
        let rider = |e: &crate::mode_scenes::Episode| e.sim.fauna().view().animals.iter().any(|a| a.attached.map(|t| t.host) == Some(host));
        assert!(rider(&e), "seed {seed}: the larva rides the host");
        let groom_s = e.sim.fauna().config().founder(Founder::Seedporter).grooming.groom_s;
        let cap = ((groom_s + 10.0) * f64::from(cubarium_voxel_fauna::TICK_HZ)) as u64;
        let x0 = e.sim.fauna().view().animal(host).unwrap().pose.x;
        let mut n = 0;
        while n < cap && rider(&e) && e.step() {
            n += 1;
        }
        assert!(!rider(&e), "seed {seed}: the larva is still on the host after {n} ticks");
        let x1 = e.sim.fauna().view().animal(host).unwrap().pose.x;
        assert!((x1 - x0).abs() < 0.25, "seed {seed}: the host held still at its meal ({x0} → {x1})");
    }
}
