//! **GUT-MASS** (Wrysk 2026-10-09, training plan Q10; shipped 2026-10-10): each lineage is
//! heavier against the plants by its own factor. Per-lineage masses move the org-unit size
//! ratios the predation windows read (`Hunter::prey_max_ratio` against body), so every
//! shipped preset must still load and keep the windows the roles were built on:
//! - the seedporter takes adult littershredders as well as bellwings (SP-SHREDDER);
//! - the lanternjaw, grown, takes capgnawers and bellwings (role call 7 B's guard);
//! - a lone Veilhound takes an adult frondgrazer, but not an adult loftstrider, which a
//!   pack of three can (roster R3);
//! - a grown lanternjaw is out of a lone Veilhound's window (LANTERNJAW-LIFE: formidable
//!   when grown).
//!
//! These are role relations, read off each preset's resolved config; no mass or ratio is
//! pinned.

use cubarium_voxel_fauna::{FaunaConfig, Founder};
use cubarium_voxel_sim::params::{Layers, Params};

const PRESETS: [&str; 9] = [
    "config/desktop/terrarium-d72.toml",
    "config/desktop/terrarium-d96.toml",
    "config/desktop/terrarium.toml",
    "config/desktop/terrarium-wide.toml",
    "config/desktop/voxel.toml",
    "config/tachyon/voxel.toml",
    "config/voidrunner/terrarium-d72.toml",
    "config/voidrunner/terrarium-d96.toml",
    "config/voidrunner/terrarium.toml",
];

fn resolved(path: &str) -> Params {
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full}: {e}"));
    let file: toml::Table = text.parse().unwrap_or_else(|e| panic!("{path}: {e}"));
    Params::resolve(&Layers { file: Some(file), ..Layers::default() })
        .unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A full-grown `hunter`'s reach (alone, or summed over `n` packmates): the largest prey
/// structure its size window admits.
fn reach(fauna: &FaunaConfig, hunter: Founder, n: f64) -> f64 {
    let p = fauna.founder(hunter);
    let h = p.hunter.as_ref().unwrap_or_else(|| panic!("{hunter:?} hunts"));
    h.prey_max_ratio * n * p.core.body_max
}

/// The size a full-grown body of `prey` counts as in a hunter's window
/// (`predation::window_size`: a grown body times its lineage's adult factor).
fn grown(fauna: &FaunaConfig, prey: Founder) -> f64 {
    let p = fauna.founder(prey);
    p.core.body_max * p.adult_size_window_factor.max(1.0)
}

/// The size an adult (just at `birth_body`) of `prey` counts as.
fn adult(fauna: &FaunaConfig, prey: Founder) -> f64 {
    let p = fauna.founder(prey);
    p.core.birth_body * p.adult_size_window_factor.max(1.0)
}

#[test]
fn every_shipped_preset_loads_and_keeps_the_predation_windows() {
    for path in PRESETS {
        let fauna = resolved(path).fauna;
        let sp = reach(&fauna, Founder::Seedporter, 1.0);
        for prey in [Founder::Blind, Founder::Bellwing] {
            assert!(sp >= grown(&fauna, prey), "{path}: a grown seedporter takes a grown {prey:?}");
        }
        let lj = reach(&fauna, Founder::Lanternjaw, 1.0);
        for prey in [Founder::Capgnawer, Founder::Bellwing] {
            assert!(lj >= grown(&fauna, prey), "{path}: a grown lanternjaw takes a grown {prey:?}");
        }
        let vh = reach(&fauna, Founder::Chorister, 1.0);
        assert!(vh >= grown(&fauna, Founder::Browser), "{path}: a Veilhound takes a grown frondgrazer");
        assert!(vh < grown(&fauna, Founder::Loftstrider), "{path}: a lone Veilhound does not take a grown loftstrider");
        assert!(
            reach(&fauna, Founder::Chorister, 3.0) >= grown(&fauna, Founder::Loftstrider),
            "{path}: a pack of three does"
        );
        assert!(vh < adult(&fauna, Founder::Lanternjaw), "{path}: a grown lanternjaw is out of a lone Veilhound's window");
    }
}

/// Every lineage of every preset digests at a gut ceiling (`yield · k / τ` of its first
/// diet food, over its maintenance) that is the same multiple of upkeep at whatever mass
/// it was built: the ceiling is per unit body on both sides.
#[test]
fn a_presets_gut_ceiling_over_upkeep_does_not_depend_on_its_mass() {
    let ceiling = |fauna: &FaunaConfig, f: Founder| {
        let p = fauna.founder(f);
        let food = p.diet.foods().next().expect("a diet");
        p.yield_for(food) * p.gut.stomach_k / p.gut.digestion_s[food.index()] / p.core.maintenance_per_s
    };
    for path in PRESETS {
        let fauna = resolved(path).fauna;
        for f in Founder::ALL {
            let mut heavy = fauna;
            heavy.founders[f.index()].scale_mass(7.0);
            let (a, b) = (ceiling(&fauna, f), ceiling(&heavy, f));
            assert!((a - b).abs() <= 1e-12 * a.abs(), "{path} {f:?}: {a} against {b}");
            assert!(heavy.founder(f).core.body_max > fauna.founder(f).core.body_max, "{path} {f:?}: heavier");
        }
    }
}
