//! **TWO-LAYER's mode-forcing hook** ([`crate::Fauna::force_own`]): a forced body holds its
//! heuristic's own answer, so its teacher label is its base mode; an unforced world runs as
//! it did. Mechanisms on a hand-built scene; no shipped number is asserted.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use crate::intent::menu_of;
use crate::{Activity, Fauna, FaunaConfig, Founder, StartingStores, Steer};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use fixture::*;

/// The prey and the lanternjaw of a mixed scene, under their heuristics, beside a hunting
/// Veilhound pair (the contracts' scene).
fn scene() -> (World, Flora, Fauna, Vec<(u64, Founder)>) {
    let w = clock_off(11);
    let flora = Flora::new(FloraConfig::default());
    let mut c: FaunaConfig = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
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
        ids.push((id, founder));
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

/// The base mode a forced body's label must be, or its rest bout.
fn base(founder: Founder) -> [&'static str; 2] {
    match founder {
        Founder::Seedporter => ["forage", "rest"],
        Founder::Loftstrider => ["graze", "rest"],
        Founder::Lanternjaw => ["ambush", "rest"],
        _ => ["live", "live"],
    }
}

#[test]
fn a_forced_body_holds_its_own_answer_and_its_label_is_its_base_mode() {
    let (w, mut flora_a, mut a, ids) = scene();
    let (_, mut flora_b, mut b, _) = scene();
    for &(id, _) in &ids {
        a.force_own(id, true);
    }
    let (mut overridden_unforced, mut sampled) = (0, 0);
    for _ in 0..600 {
        a.step(&w, &mut flora_a);
        b.step(&w, &mut flora_b);
        for &(id, founder) in &ids {
            if let Some(s) = b.steer_of(id) {
                overridden_unforced += usize::from(!matches!(s, Steer::Heuristic | Steer::Rest));
            }
            let Some(s) = a.steer_of(id) else { continue };
            assert!(
                matches!(s, Steer::Heuristic | Steer::Rest | Steer::Drop | Steer::Wade | Steer::Shallows),
                "{}: forced, steered by {s:?}",
                founder.name()
            );
            let Some(animal) = a.view().animal(id).copied() else { continue };
            let r = animal.intent;
            if r.sample_tick == a.tick() && s == Steer::Heuristic {
                sampled += 1;
                let name = menu_of(founder).name(usize::from(r.proposal));
                assert!(base(founder).contains(&name), "{}: forced label {name}", founder.name());
            }
        }
    }
    assert!(sampled > 0, "the forced bodies sampled");
    assert!(overridden_unforced > 0, "unforced, the heuristic layer overrode somewhere: the scene exercises the hook");
}

#[test]
fn an_unforced_world_is_unchanged_by_the_hook() {
    let (w, mut flora_a, mut a, ids) = scene();
    let (_, mut flora_b, mut b, _) = scene();
    // Forced and released before any step: nothing is forced.
    for &(id, _) in &ids {
        a.force_own(id, true);
        a.force_own(id, false);
    }
    for t in 0..300 {
        a.step(&w, &mut flora_a);
        b.step(&w, &mut flora_b);
        for &(id, _) in &ids {
            assert_eq!(a.view().animal(id).map(|x| x.pose), b.view().animal(id).map(|x| x.pose), "tick {t}");
            assert_eq!(a.steer_of(id), b.steer_of(id), "tick {t}");
        }
    }
}

/// **The seedporter's `rest` is the pause** (BRAIN-REST): in a rest bout and held still it is
/// labelled `rest`; walking on through the bout (a glider can't sleep on open ground) it is
/// labelled `forage`, what it does.
#[test]
fn a_seedporter_walking_through_a_rest_bout_is_labelled_forage() {
    let (_, _, mut f, ids) = scene();
    let (id, founder) = ids[0];
    assert_eq!(founder, Founder::Seedporter);
    let i = f.animals.iter().position(|a| a.id == id).expect("the seedporter");
    f.animals[i].rest_ticks = 10;
    let stay = f.config.teachers.forage_stay_forward;
    let name = |f: &Fauna, forward: f64| {
        let held = crate::Actions { forward, turn: 0.0, feed: 0.0 };
        crate::intent::teacher_label(f, i, founder, Steer::Heuristic, held, false, false).name
    };
    assert_eq!(name(&f, stay), Some("rest"), "still in a rest bout");
    assert_eq!(name(&f, stay + 0.25), Some("forage"), "walking through a rest bout");
}
