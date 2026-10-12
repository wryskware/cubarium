//! **BRAIN-NEXT's dry-run teacher** ([`crate::Fauna::dry_teach`]): beside brain-driven bodies
//! it labels every decision sample and changes nothing about the world; beside heuristic
//! bodies its labels are the real teacher's. Mechanisms on a hand-built scene; no shipped
//! number is asserted.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use crate::{Actions, Activity, Controller, Decision, DecisionInput, DryScope, Fauna, FaunaConfig, Founder, Response, StartingStores};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use fixture::*;

/// A brain that walks a slow curve, eating, and holds its menu's first intent.
struct Walker(f64);

impl Controller for Walker {
    fn drive(&mut self, _o: &[f64]) -> Response {
        Response::Bounded(Actions { forward: 0.6, turn: self.0, feed: 0.5 })
    }
    fn reset(&mut self) {}
    fn decide(&mut self, o: &[f64], _input: &DecisionInput<'_>) -> (Response, Option<Decision>) {
        (self.drive(o), Some(Decision { intent: 0, ..Decision::default() }))
    }
}

/// Seedporters and a lanternjaw among loftstriders beside a hunting Veilhound pair (the
/// contracts' scene); the menu bodies on `brains` (else their heuristics).
fn scene(brains: bool) -> (World, Flora, Fauna, Vec<(u64, Founder)>) {
    let w = clock_off(11);
    let flora = Flora::new(FloraConfig::default());
    let mut c: FaunaConfig = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    let mut f = Fauna::new(c);
    let teachers = f.config.teachers;
    let mut ids = Vec::new();
    for (k, founder) in [Founder::Seedporter, Founder::Seedporter, Founder::Loftstrider, Founder::Seedporter, Founder::Lanternjaw]
        .into_iter()
        .enumerate()
    {
        let id = introduce(&mut f, &w, founder, StartingStores::FULL);
        place(&mut f, id, 1.0 + 2.0 * k as f64, LANE_Z, EAST);
        let menu = matches!(founder, Founder::Seedporter | Founder::Lanternjaw);
        if brains && menu {
            f.controllers.set(id, Box::new(Walker(0.05 * k as f64 - 0.1)));
        } else {
            f.controllers.set(id, crate::heuristic(founder, teachers));
        }
        if menu {
            ids.push((id, founder));
        }
    }
    let den = Site { x: 30, y: SURFACE_Y, z: 6 };
    f.found_pack_sized(&w, den, Founder::Chorister, StartingStores::FULL, 0.0, 2).expect("a Veilhound pair");
    for a in f.animals.clone() {
        if a.founder == Some(Founder::Chorister) {
            f.controllers.set(a.id, crate::heuristic(Founder::Chorister, teachers));
        }
    }
    (w, flora, f, ids)
}

#[test]
fn the_dry_teacher_labels_brain_bodies_and_changes_nothing_about_the_world() {
    let (w, mut flora_a, mut a, ids) = scene(true);
    let (_, mut flora_b, mut b, _) = scene(true);
    for f in [Founder::Seedporter, Founder::Lanternjaw] {
        a.dry_teach(f, Some(DryScope::Brains));
    }
    a.start_tap();
    b.start_tap();
    for t in 0..400 {
        a.step(&w, &mut flora_a);
        b.step(&w, &mut flora_b);
        if t % 50 == 49 {
            // The whole saved layer (bodies, stores, records, every saved stream state) and
            // the flora it ate from are the same.
            assert!(a.save() == b.save(), "tick {t}: the dry run changed the world");
            assert!(cubarium_voxel_flora::snapshot::encode(&flora_a) == cubarium_voxel_flora::snapshot::encode(&flora_b), "tick {t}");
        }
    }
    let (ta, tb) = (a.take_tap().unwrap(), b.take_tap().unwrap());
    let mut labelled = 0;
    for &(id, founder) in &ids {
        // Every executed-action sample and decision sample is the same.
        assert_eq!(ta.stream(id), tb.stream(id), "{}", founder.name());
        assert_eq!(ta.decisions(id), tb.decisions(id), "{}", founder.name());
        assert!(tb.teacher_labels(id).is_none(), "off, no labels");
        let decisions = ta.decisions(id).unwrap_or_default();
        let labels = ta.teacher_labels(id).unwrap_or_default();
        // One label per decision sample, at its tick, a named intent of the menu.
        assert_eq!(labels.len(), decisions.len(), "{}", founder.name());
        for (l, d) in labels.iter().zip(decisions) {
            assert_eq!(l.tick, d.tick);
            assert!(usize::from(l.proposal) < crate::intent::menu_of(founder).named.len());
        }
        labelled += labels.len();
    }
    assert!(labelled > 0, "the brain bodies were labelled");
}

#[test]
fn beside_a_heuristic_body_the_dry_teacher_is_the_real_teacher() {
    let (w, mut flora, mut f, ids) = scene(false);
    for founder in [Founder::Seedporter, Founder::Lanternjaw] {
        f.dry_teach(founder, Some(DryScope::Every));
    }
    f.start_tap();
    for _ in 0..400 {
        f.step(&w, &mut flora);
    }
    let tap = f.take_tap().unwrap();
    let (mut n, mut overridden) = (0, 0);
    for &(id, founder) in &ids {
        let labels = tap.teacher_labels(id).unwrap_or_default();
        let decisions = tap.decisions(id).unwrap_or_default();
        let stream = tap.stream(id).unwrap_or_default();
        assert_eq!(labels.len(), decisions.len(), "{}", founder.name());
        for (l, d) in labels.iter().zip(decisions) {
            let s = stream.iter().find(|s| s.tick == d.tick).expect("its tap sample");
            let name = |k: u8| crate::intent::menu_of(founder).name(usize::from(k));
            assert_eq!((name(l.proposal), name(l.held)), (name(d.proposal), name(d.held)), "{} tick {}", founder.name(), d.tick);
            assert_eq!((l.proposal_row, l.held_row), (d.proposal_row, d.held_row), "{} tick {}", founder.name(), d.tick);
            assert_eq!(l.steer, s.steer, "{} tick {}", founder.name(), d.tick);
            assert_eq!((l.own, l.executed), (s.own, s.executed), "{} tick {}", founder.name(), d.tick);
            overridden += usize::from(!matches!(l.steer, crate::Steer::Heuristic | crate::Steer::Rest));
            n += 1;
        }
    }
    assert!(n > 0, "the heuristic bodies were labelled");
    assert!(overridden > 0, "the scene exercised the heuristic layer");
}
