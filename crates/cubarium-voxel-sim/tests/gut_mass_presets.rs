//! **GUT-MASS** (Wrysk 2026-10-09, training plan Q10; shipped 2026-10-10): each lineage is
//! heavier against the plants by its own factor. Since SIZE-NOT-MASS every size comparison
//! reads body / `mass_scale` ([`FounderPhysiology::size`]), so the masses no longer move the
//! predation windows; every shipped preset must still load and keep the windows the roles
//! were built on (GUT-MASS-2's confirmations):
//! - a lone Veilhound takes an adult frondgrazer, but not an adult loftstrider, which a
//!   pack of three can (roster R3);
//! - a grown lanternjaw takes frondgrazer juveniles, capgnawers and bellwings (role call
//!   7 B), and is out of a lone Veilhound's window (LANTERNJAW-LIFE);
//! - a grown seedporter takes a grown littershredder (Wrysk's decision A) and bellwings;
//! - a lanternjaw larva's hosts are the lineages that would host it were every lineage at
//!   one mass (main's relation): its host test is energy, and the masses move no lineage
//!   in or out of it.
//!
//! These are role relations, read off each preset's resolved config; no mass or ratio is
//! pinned.

use cubarium_voxel_fauna::{FaunaConfig, Food, Founder, FounderPhysiology};
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
/// size its window admits (`predation::is_prey_of` on physical sizes).
fn reach(fauna: &FaunaConfig, hunter: Founder, n: f64) -> f64 {
    let p = fauna.founder(hunter);
    let h = p.hunter.as_ref().unwrap_or_else(|| panic!("{hunter:?} hunts"));
    h.prey_max_ratio * n * p.size(p.core.body_max)
}

/// The size a full-grown body of `prey` counts as in a hunter's window
/// (`predation::window_size`: a grown body's size times its lineage's adult factor).
fn grown(fauna: &FaunaConfig, prey: Founder) -> f64 {
    let p = fauna.founder(prey);
    p.size(p.core.body_max) * p.adult_size_window_factor.max(1.0)
}

/// The size an adult (just at `birth_body`) of `prey` counts as.
fn adult(fauna: &FaunaConfig, prey: Founder) -> f64 {
    let p = fauna.founder(prey);
    p.size(p.core.birth_body) * p.adult_size_window_factor.max(1.0)
}

/// The size a newborn (`body_min`, a juvenile) of `prey` counts as.
fn newborn(fauna: &FaunaConfig, prey: Founder) -> f64 {
    let p = fauna.founder(prey);
    p.size(p.core.body_min)
}

#[test]
fn every_shipped_preset_loads_and_keeps_the_predation_windows() {
    for path in PRESETS {
        let fauna = resolved(path).fauna;
        let vh = reach(&fauna, Founder::Chorister, 1.0);
        assert!(vh >= adult(&fauna, Founder::Browser), "{path}: a lone Veilhound takes an adult frondgrazer");
        assert!(vh >= grown(&fauna, Founder::Browser), "{path}: and a grown one");
        assert!(vh < grown(&fauna, Founder::Loftstrider), "{path}: a lone Veilhound does not take a grown loftstrider");
        assert!(
            reach(&fauna, Founder::Chorister, 3.0) >= grown(&fauna, Founder::Loftstrider),
            "{path}: a pack of three does"
        );
        assert!(vh < adult(&fauna, Founder::Lanternjaw), "{path}: a grown lanternjaw is out of a lone Veilhound's window");
        let lj = reach(&fauna, Founder::Lanternjaw, 1.0);
        assert!(lj >= newborn(&fauna, Founder::Browser), "{path}: a grown lanternjaw takes frondgrazer juveniles");
        for prey in [Founder::Capgnawer, Founder::Bellwing] {
            assert!(lj >= grown(&fauna, prey), "{path}: a grown lanternjaw takes a grown {prey:?}");
        }
        let sp = reach(&fauna, Founder::Seedporter, 1.0);
        for prey in [Founder::Blind, Founder::Bellwing] {
            assert!(sp >= grown(&fauna, prey), "{path}: a grown seedporter takes a grown {prey:?}");
        }
    }
}

/// `fauna` with every lineage rebuilt at the lanternjaw's mass: main's relation, where
/// every lineage shared one `mass_scale`.
fn at_one_mass(fauna: &FaunaConfig) -> FaunaConfig {
    let mut one = fauna.clone();
    let m = fauna.founder(Founder::Lanternjaw).mass_scale;
    for f in Founder::ALL {
        let p: &mut FounderPhysiology = &mut one.founders[f.index()];
        let k = m / p.mass_scale;
        p.scale_mass(k);
    }
    one
}

/// Whether a grown `host` bears a lanternjaw rider at `share` of the way from a hatchling
/// (`body_min`, 0) to its drop-off size (`parasite_detach_body_fraction` x `body_max`, 1).
fn bears(fauna: &FaunaConfig, host: Founder, share: f64) -> bool {
    let lj = &fauna.founder(Founder::Lanternjaw).core;
    let line = fauna.parasite_detach_body_fraction * lj.body_max;
    let rider = lj.body_min + share * (line - lj.body_min);
    cubarium_voxel_fauna::host_bears(fauna, rider, fauna.founder(host).core.body_max)
}

#[test]
fn a_lanternjaw_larvas_hosts_are_the_lineages_that_host_it_at_one_mass() {
    for path in PRESETS {
        let fauna = resolved(path).fauna;
        let one = at_one_mass(&fauna);
        for host in Founder::ALL.into_iter().filter(|&f| f != Founder::Lanternjaw) {
            for share in [0.0, 0.5, 1.0] {
                assert_eq!(
                    bears(&fauna, host, share),
                    bears(&one, host, share),
                    "{path}: a grown {host:?} bears a rider {share} of the way to drop-off as at one mass"
                );
            }
        }
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

/// **SP-PROTEIN: a preset that sets a lineage's digestion time sets it for every food the
/// lineage eats.** A food left out keeps the code's default beside the preset's rescaled
/// times (the seedporter's larvae, added after GUT-MASS rescaled the rest, digested at the
/// default 60 s beside 386 s for every other food). No time or ratio is asserted, only that
/// none of a lineage's foods is left out.
#[test]
fn a_preset_that_sets_a_lineages_digestion_sets_it_for_every_food_it_eats() {
    for path in PRESETS {
        let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full}: {e}"));
        let file: toml::Table = text.parse().unwrap_or_else(|e| panic!("{path}: {e}"));
        let fauna = resolved(path).fauna;
        let Some(founders) = file.get("fauna").and_then(|f| f.get("founders")).and_then(|f| f.as_table()) else {
            continue;
        };
        for (name, section) in founders {
            let Some(set) = section.get("gut").and_then(|g| g.get("digestion_s")).and_then(|d| d.as_table()) else {
                continue;
            };
            let founder = *Founder::ALL.iter().find(|f| f.name() == name).unwrap_or_else(|| panic!("{path}: no lineage is called {name}"));
            let diet = fauna.founder(founder).eating_diet();
            for food in Food::ALL.into_iter().filter(|&food| diet.eats(food)) {
                assert!(set.contains_key(food.name()), "{path}: {name} sets digestion_s for some foods but not for {}, which it eats", food.name());
            }
        }
    }
}
