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

// ---- Step 2: subjects and slots (§11.1) ----

use crate::intent::{EarlyEnd, Subject};
use crate::perception::{Percept, Track, class};
use crate::subjects::{ROW_WIDTH, candidates};

/// A loftstrider with `n` kin, threat or prey neighbours introduced on the plain (placed by
/// the case), its record filled by hand.
fn observer_world(others: &[Founder]) -> (World, Fauna, u64, Vec<u64>) {
    let w = clock_off(5);
    let mut f = Fauna::new(quiet_config());
    let me = introduce(&mut f, &w, Founder::Loftstrider, StartingStores::FULL);
    place(&mut f, me, 6.0, LANE_Z, EAST);
    let ids = others
        .iter()
        .enumerate()
        .map(|(k, &o)| {
            let id = introduce(&mut f, &w, o, StartingStores::FULL);
            place(&mut f, id, 7.0 + k as f64, LANE_Z, EAST);
            id
        })
        .collect();
    f.world_geometry = Some((48.0 * VOXEL_M, VOXEL_M));
    (w, f, me, ids)
}

fn track(id: u64, cls: u8, x: f64, d: f64, tick: u64) -> Track {
    Track { known: true, id, class: cls, modes: crate::perception::SIGHT, x, z: LANE_Z, d_m: d, size_m: 1.0, tick, ..Track::default() }
}

fn set_tracks(f: &mut Fauna, me: u64, tracks: &[Track]) {
    let mut p = Percept::default();
    p.tracks[..tracks.len()].copy_from_slice(tracks);
    p.n = tracks.len() as u8;
    p.tick = f.tick;
    f.percepts.insert(me, p);
}

fn index_of(f: &Fauna, id: u64) -> usize {
    f.animals.iter().position(|a| a.id == id).unwrap()
}

#[test]
fn slot_order_does_not_change_the_subject_or_its_row() {
    let (_, mut f, me, ids) = observer_world(&[Founder::Loftstrider, Founder::Loftstrider, Founder::Loftstrider]);
    let tr: Vec<Track> = ids.iter().enumerate().map(|(k, &id)| track(id, class::KIN, 7.0 + k as f64, 1.0 + k as f64, 0)).collect();
    let subject = Subject::of_track(&tr[1]);
    let i = index_of(&f, me);
    set_tracks(&mut f, me, &tr);
    let c1 = candidates(&f, i, Founder::Loftstrider, subject);
    let mut rev = tr.clone();
    rev.reverse();
    set_tracks(&mut f, me, &rev);
    let c2 = candidates(&f, i, Founder::Loftstrider, subject);
    let (r1, r2) = (c1.row_of(subject).unwrap(), c2.row_of(subject).unwrap());
    assert_eq!(c1.rows[r1].subject.body(), Some(ids[1]));
    assert_eq!(c1.rows[r1].row, c2.rows[r2].row, "the same body reads the same row");
    assert_eq!(c1.rows.len(), f.config.manifest(Founder::Loftstrider).candidates());
    assert!(c1.rows.iter().all(|c| c.row.len() == ROW_WIDTH));
}

#[test]
fn two_targets_crossing_keep_the_held_subject_on_its_own_body() {
    let (_, mut f, me, ids) = observer_world(&[Founder::Chorister, Founder::Chorister]);
    let i = index_of(&f, me);
    let (a, b) = (ids[0], ids[1]);
    let held = Subject::of_track(&track(a, class::THREAT, 7.0, 1.0, 0));
    // Before the crossing a is nearer; after it b is: the held subject's row moves with a.
    for (da, db) in [(1.0, 2.0), (1.5, 1.5), (2.0, 1.0)] {
        set_tracks(&mut f, me, &[track(a, class::THREAT, 6.0 + da, da, 0), track(b, class::THREAT, 6.0 + db, db, 0)]);
        let c = candidates(&f, i, Founder::Loftstrider, held);
        let r = c.row_of(held).expect("held");
        assert_eq!(c.rows[r].subject.body(), Some(a));
        assert!((c.rows[r].row[5] - crate::observe_v3::proximity(da, f.config.manifest(Founder::Loftstrider).slot_range_m)).abs() < 1e-9);
    }
}

#[test]
fn a_subject_dying_or_vanishing_ends_the_hold() {
    let (_, mut f, me, ids) = observer_world(&[Founder::Chorister, Founder::Chorister]);
    let lost = f.config.founder(Founder::Loftstrider).intent.subject_lost_s;
    let (a, b) = (ids[0], ids[1]);
    set_tracks(&mut f, me, &[track(a, class::THREAT, 7.0, 1.0, 0), track(b, class::THREAT, 8.0, 2.0, 0)]);
    let sa = Subject::of_track(&track(a, class::THREAT, 7.0, 1.0, 0));
    assert!(crate::intent::check_subject(&f, me, sa, lost).is_ok());
    // Vanishing: unsensed past subject_lost_s. While within it, it is a remembered row.
    set_tracks(&mut f, me, &[track(b, class::THREAT, 8.0, 2.0, 0)]);
    f.tick = 1;
    let c = candidates(&f, index_of(&f, me), Founder::Loftstrider, sa);
    let r = c.row_of(sa).expect("kept while not given up");
    assert_eq!(c.rows[r].row[31], 1.0, "remembered");
    f.tick = (lost / crate::DT).ceil() as u64 + 2;
    assert_eq!(crate::intent::check_subject(&f, me, sa, lost), Err(EarlyEnd::SubjectLost));
    assert!(candidates(&f, index_of(&f, me), Founder::Loftstrider, sa).row_of(sa).is_none());
    // Dying: its id names no living body.
    let sb = Subject::of_track(&track(b, class::THREAT, 8.0, 2.0, f.tick));
    f.animals.retain(|x| x.id != b);
    assert_eq!(crate::intent::check_subject(&f, me, sb, lost), Err(EarlyEnd::SubjectDead));
    // In the record: the hold ends early and the subject becomes none.
    let i = index_of(&f, me);
    let mut rec = f.animals[i].intent;
    rec.subject = sb;
    rec.held = 2;
    rec.since_tick = f.tick;
    rec.sample_tick = f.tick;
    f.animals[i].intent = rec;
    let valid = vec![true; crate::intent::menu_of(Founder::Loftstrider).len()];
    crate::intent::record(&mut f, i, Founder::Loftstrider, crate::Steer::Heuristic, crate::intent::Teach::default(), &valid, crate::Executed::default());
    let after = f.animals[i].intent;
    assert_eq!(after.ended, Some(EarlyEnd::SubjectDead));
    assert!(after.subject.is_none() || after.subject.body() != Some(b));
}

#[test]
fn a_recognised_mother_keeps_the_first_kin_slot_over_nearer_kin() {
    let (_, mut f, me, ids) = observer_world(&[Founder::Loftstrider, Founder::Loftstrider]);
    f.config.founders[Founder::Loftstrider.index()].intent.kin_slots = 1;
    let (near, mother) = (ids[0], ids[1]);
    let i = index_of(&f, me);
    f.animals[i].mother = Some(mother);
    set_tracks(&mut f, me, &[track(near, class::KIN, 7.0, 1.0, 0), track(mother, class::KIN, 9.0, 3.0, 0)]);
    let c = candidates(&f, i, Founder::Loftstrider, Subject::None);
    assert_eq!(c.rows[0].subject.body(), Some(mother));
    assert_eq!(c.rows[0].row[20], 1.0, "marked mother");
    // A founding mother id names nobody: no one is marked as its mother.
    f.animals[i].mother = Some(crate::founding_mother(1));
    let c = candidates(&f, i, Founder::Loftstrider, Subject::None);
    assert_eq!(c.rows[0].subject.body(), Some(near));
    assert_eq!(c.rows[0].row[20], 0.0);
}

#[test]
fn plain_manifests_carry_no_intent_contract_and_menu_ones_add_it_after_the_v3_text() {
    for f in Founder::ALL {
        let m = f.manifest();
        let text = m.canonical_text();
        let mut before = m;
        before.menu = &crate::intent::PLAIN;
        let pre = before.canonical_text();
        if menu_of(f).plain() {
            assert_eq!(text, pre);
            assert!(!text.contains("|intent:"), "{}", f.name());
            assert_eq!((m.candidates(), m.row_width()), (0, 0));
        } else {
            assert!(text.starts_with(&pre) && text.len() > pre.len(), "{}: additive", f.name());
            assert!(m.candidates() > 0 && m.row_width() == ROW_WIDTH);
            // A policy made before the contract is refused by name.
            let old = crate::manifest::fnv1a_pub(pre.as_bytes());
            assert_ne!(old, m.digest());
            assert!(m.digest_refusal(old).contains("before the intent contract"), "{}", f.name());
        }
    }
}
