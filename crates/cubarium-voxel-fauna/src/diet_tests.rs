//! LANTERNJAW-DIET (Wrysk 2026-10-10: "bellwings and capgnawers should be the main prey.
//! they can eat shredders too. occasionally a juvenile seedporter or frondgrazer. snails can
//! be eaten when ashore."): the young-only and ashore-only prey rules and the prey weights,
//! on hand-built bodies, against whatever the config says.

use super::predation_fixture::*;
use crate::{Fauna, FaunaConfig, Founder, StartingStores};

/// A full-grown lanternjaw and one body of `prey` beside it on a plain, the config's own.
fn pair(prey: Founder, seed: u64) -> (Fauna, u64, u64) {
    let world = clock_off(seed);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    let p = introduce(&mut fauna, &world, prey, StartingStores::FULL);
    place(&mut fauna, lj, STRIKER_X, LANE_Z, EAST);
    place(&mut fauna, p, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    (fauna, lj, p)
}

/// **Snails ashore only**: a ripple-snail out of water is the lanternjaw's prey; the same
/// snail in water is not.
#[test]
fn a_snail_in_water_is_not_taken_and_one_ashore_is() {
    let (mut fauna, lj, snail) = pair(Founder::RippleSnail, 61);
    let c = *fauna.config;
    let h = c.founder(Founder::Lanternjaw).hunter.expect("the lanternjaw's table");
    assert!(h.prey_ashore_only[Founder::RippleSnail.index()], "fixture: the snail is ashore-only");
    animal_mut(&mut fauna, snail).mobility.ashore = true;
    assert!(crate::is_prey_of(&c, &animal(&fauna, lj), &animal(&fauna, snail)), "ashore: prey");
    animal_mut(&mut fauna, snail).mobility.ashore = false;
    assert!(!crate::is_prey_of(&c, &animal(&fauna, lj), &animal(&fauna, snail)), "in water: not");
}

/// **Young grazers only**: a full-grown lanternjaw takes a frondgrazer juvenile inside its
/// size window but not an adult grazer, even one its window would hold.
#[test]
fn an_adult_frondgrazer_is_not_taken_and_a_juvenile_is() {
    let (mut fauna, lj, grazer) = pair(Founder::Browser, 62);
    let c = *fauna.config;
    let h = c.founder(Founder::Lanternjaw).hunter.expect("the lanternjaw's table");
    let core = crate::effective_config(&c, &animal(&fauna, grazer));
    let window = h.prey_max_ratio * animal(&fauna, lj).body;

    let young = 0.5 * core.birth_body.min(window);
    assert!(young > core.body_min, "fixture: a living juvenile");
    animal_mut(&mut fauna, grazer).body = young;
    assert!(crate::is_prey_of(&c, &animal(&fauna, lj), &animal(&fauna, grazer)), "a juvenile is prey");

    let adult = core.birth_body;
    assert!(adult <= window, "fixture: the size window alone would hold the adult");
    animal_mut(&mut fauna, grazer).body = adult;
    assert!(!crate::is_prey_of(&c, &animal(&fauna, lj), &animal(&fauna, grazer)), "an adult is not");
}

/// **Main prey**: the attack chance follows the hunter's `prey_weight` for the prey's
/// lineage — halving the weight halves the chance on the same pair.
#[test]
fn the_attack_chance_follows_the_prey_weight() {
    let (fauna, lj, cap) = pair(Founder::Capgnawer, 63);
    let c = *fauna.config;
    let (a, o) = (animal(&fauna, lj), animal(&fauna, cap));
    let full = crate::attack_chance(&c, &a, &o);
    assert!(full > 0.0, "fixture: a capgnawer is worth attacking");
    let mut half = c;
    let k = Founder::Capgnawer.index();
    let h = half.founders[Founder::Lanternjaw.index()].hunter.as_mut().expect("a hunter");
    h.prey_weight[k] *= 0.5;
    let got = crate::attack_chance(&half, &a, &o);
    assert!((got - 0.5 * full).abs() <= 1e-12, "{got} vs half of {full}");
}
