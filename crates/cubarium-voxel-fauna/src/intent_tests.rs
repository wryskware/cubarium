//! **CONTRACTS, from the brief** (`design/handoffs/contracts-2026-10-12.md`): the menus and
//! their masks, the shared hold, the records the heuristics fill, and that the records ride
//! along without changing a step.
//!
//! Every scene sets the knobs it relies on; no shipped number, ratio or world hash is asserted.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use crate::intent::{self, IntentKnobs, SPARE_INTENTS, arbitrate, menu_of, probabilities};
use crate::{Activity, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use fixture::*;

const MENU_LINEAGES: [Founder; 4] = [Founder::Seedporter, Founder::Loftstrider, Founder::Chorister, Founder::Lanternjaw];

#[test]
fn menu_lineages_carry_named_intents_and_two_masked_spares_and_plain_ones_one_item() {
    for f in Founder::ALL {
        let m = menu_of(f);
        if MENU_LINEAGES.contains(&f) {
            assert!(m.named.len() > 1, "{}", f.name());
            assert_eq!(m.spare, SPARE_INTENTS, "{}", f.name());
            let mask = intent::intent_mask(m, false, |_| true);
            assert_eq!(mask.len(), m.len());
            assert!(mask[m.named.len()..].iter().all(|v| !v), "{}: spares are masked", f.name());
        } else {
            assert!(m.plain(), "{} is plain", f.name());
            assert_eq!(m.len(), 1);
        }
    }
}

#[test]
fn a_required_subject_masks_its_intent_until_a_candidate_fits() {
    let m = menu_of(Founder::Loftstrider);
    let guard = m.index("guard").unwrap();
    let graze = m.index("graze").unwrap();
    let none = intent::intent_mask(m, false, |_| false);
    assert!(!none[guard] && none[graze]);
    let some = intent::intent_mask(m, false, |f| f == intent::Fits::Threat);
    assert!(some[guard]);
    // Who: an adult intent is masked for a young body, a young one for an adult.
    let young = intent::intent_mask(m, true, |_| true);
    assert!(!young[m.index("go_to_calf").unwrap()] && young[m.index("go_to_adults").unwrap()]);
    assert!(!some[m.index("go_to_adults").unwrap()]);
}

fn knobs(hold_s: f64, margin: f64) -> IntentKnobs {
    IntentKnobs { hold_s, switch_margin: margin, ..IntentKnobs::default() }
}

#[test]
fn the_hold_keeps_an_intent_until_hold_s_then_switches_only_past_the_margin() {
    let k = knobs(1.0, 0.2);
    let valid = [true, true, true];
    // Before the hold, a far better proposal does not switch.
    assert_eq!(arbitrate(&k, &valid, &[0.0, 1.0, 0.0], 0, 0.5, false), 0);
    // After it, a proposal better by less than the margin does not switch...
    assert_eq!(arbitrate(&k, &valid, &[0.45, 0.55, 0.0], 0, 2.0, false), 0);
    // ...and one better by more does.
    assert_eq!(arbitrate(&k, &valid, &[0.3, 0.7, 0.0], 0, 2.0, false), 1);
    // An early end chooses at once, inside the hold.
    assert_eq!(arbitrate(&k, &valid, &[0.45, 0.55, 0.0], 0, 0.1, true), 1);
    // A masked intent is never held: not kept, and not chosen.
    assert_eq!(arbitrate(&k, &[false, true, true], &[0.9, 0.05, 0.05], 0, 0.1, false), 1);
    assert_eq!(arbitrate(&k, &[true, false, true], &[0.1, 0.8, 0.1], 0, 5.0, false), 0);
}

#[test]
fn probabilities_are_taken_at_the_fixed_temperature_over_valid_intents() {
    let p = probabilities(&[1.0, 2.0, 50.0], &[true, true, false], 1.0);
    assert_eq!(p[2], 0.0);
    assert!((p[0] + p[1] - 1.0).abs() < 1e-12);
    // Scaling the logits and the temperature together leaves the probabilities, so the
    // margin's switching is a property of the temperature, not the logits' scale.
    let q = probabilities(&[3.0, 6.0, 150.0], &[true, true, false], 3.0);
    assert!((p[1] - q[1]).abs() < 1e-12);
    let hotter = probabilities(&[1.0, 2.0, 50.0], &[true, true, false], 4.0);
    assert!(hotter[1] < p[1]);
}

/// A clock-off plain with plants off, every rhythm awake, no ground response.
fn quiet_config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c
}

/// Two seedporters, two loftstriders, a lanternjaw and a Veilhound pair under their own
/// heuristics on the plain.
fn mixed_world(c: FaunaConfig) -> (World, Flora, Fauna, Vec<u64>) {
    let w = clock_off(11);
    let flora = Flora::new(FloraConfig::default());
    let mut f = Fauna::new(c);
    let teachers = f.config.teachers;
    let mut ids = Vec::new();
    for (k, founder) in [Founder::Seedporter, Founder::Seedporter, Founder::Loftstrider, Founder::Loftstrider, Founder::Lanternjaw]
        .into_iter()
        .enumerate()
    {
        let id = introduce(&mut f, &w, founder, StartingStores::FULL);
        place(&mut f, id, 1.0 + 2.0 * k as f64, LANE_Z, EAST);
        f.controllers.set(id, crate::heuristic(founder, teachers));
        ids.push(id);
    }
    let den = Site { x: 40, y: SURFACE_Y, z: 6 };
    f.found_pack_sized(&w, den, Founder::Chorister, StartingStores::FULL, 0.0, 2).expect("a Veilhound pair");
    {
        for a in f.animals.clone() {
            if a.founder == Some(Founder::Chorister) {
                f.controllers.set(a.id, crate::heuristic(Founder::Chorister, teachers));
                ids.push(a.id);
            }
        }
    }
    (w, flora, f, ids)
}

#[test]
fn the_heuristics_fill_their_records_with_valid_held_intents_and_the_executed_actions() {
    let (w, mut flora, mut f, ids) = mixed_world(quiet_config());
    for _ in 0..200 {
        f.step(&w, &mut flora);
    }
    let mut recorded = 0;
    for &id in &ids {
        let Some(a) = f.view().animal(id).copied() else { continue };
        let founder = a.founder.unwrap();
        let m = menu_of(founder);
        let r = a.intent;
        if r.sample_tick == 0 {
            continue;
        }
        recorded += 1;
        assert!(usize::from(r.held) < m.named.len(), "{}: held a named intent", founder.name());
        assert!(usize::from(r.proposal) < m.named.len());
        assert!(r.since_tick <= r.sample_tick);
        if a.state != crate::State::Asleep {
            assert_eq!(r.executed.actions, a.founder_state.held, "{}: the executed actions are the held", founder.name());
        }
    }
    assert!(recorded >= 4, "most bodies were sampled ({recorded})");
}

#[test]
fn the_records_ride_along_steering_and_positions_match_a_run_with_them_stripped() {
    let (w, mut flora_a, mut a, ids) = mixed_world(quiet_config());
    let (_, mut flora_b, mut b, _) = mixed_world(quiet_config());
    for t in 0..300 {
        a.step(&w, &mut flora_a);
        b.step(&w, &mut flora_b);
        // Strip b's records after every tick: nothing may read them.
        for x in &mut b.animals {
            x.intent = crate::IntentRecord::default();
        }
        for &id in &ids {
            let (pa, pb) = (a.view().animal(id).map(|x| x.pose), b.view().animal(id).map(|x| x.pose));
            assert_eq!(pa, pb, "tick {t}: body {id} moved the same");
            assert_eq!(a.steer_of(id), b.steer_of(id), "tick {t}: body {id} steered the same");
        }
    }
}
