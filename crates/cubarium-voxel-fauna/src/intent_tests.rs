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
    assert_eq!((c1.rows[r1].key(), c2.rows[r2].key()), (ids[1], ids[1]), "keyed by its id");
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

// ---- Step 3: the brain boundary ----

use crate::{Actions, BrainMemory, Controller, Decision, DecisionInput, Response};
use std::sync::{Arc, Mutex};

/// A brain that answers rest, records what each decision sample gave it, and answers a fixed
/// decision; `drive` alone counts too.
#[derive(Clone)]
struct Probe {
    seen: Arc<Mutex<Vec<(usize, usize, usize, usize, usize)>>>,
    drives: Arc<Mutex<usize>>,
    answer: Decision,
    memory: Option<BrainMemory>,
}

impl Controller for Probe {
    fn drive(&mut self, _o: &[f64]) -> Response {
        *self.drives.lock().unwrap() += 1;
        Response::Bounded(Actions::REST)
    }
    fn reset(&mut self) {}
    fn decide(&mut self, _o: &[f64], input: &DecisionInput<'_>) -> (Response, Option<Decision>) {
        self.seen.lock().unwrap().push((input.menu, input.intent_mask.len(), input.rows(), input.candidates.len(), input.candidate_mask.len()));
        (Response::Bounded(Actions::REST), Some(self.answer))
    }
    fn memory(&self) -> Option<BrainMemory> {
        self.memory.clone()
    }
    fn restore(&mut self, m: &BrainMemory) -> bool {
        self.memory = Some(m.clone());
        true
    }
}

fn probe(answer: Decision) -> Probe {
    Probe { seen: Arc::default(), drives: Arc::default(), answer, memory: None }
}

#[test]
fn a_plain_lineage_brain_is_driven_as_before_and_a_menu_brain_decides_with_the_manifest_dims() {
    let w = clock_off(3);
    let mut flora = Flora::new(FloraConfig::default());
    let mut f = Fauna::new(quiet_config());
    let plain = introduce(&mut f, &w, Founder::Browser, StartingStores::FULL);
    let menu = introduce(&mut f, &w, Founder::Loftstrider, StartingStores::FULL);
    place(&mut f, plain, 2.0, LANE_Z, EAST);
    place(&mut f, menu, 8.0, LANE_Z, EAST);
    let (p, m) = (probe(Decision::default()), probe(Decision { intent: 0, ..Decision::default() }));
    f.controllers.set(plain, Box::new(p.clone()));
    f.controllers.set(menu, Box::new(m.clone()));
    for _ in 0..40 {
        f.step(&w, &mut flora);
    }
    assert!(p.seen.lock().unwrap().is_empty(), "a plain brain never gets a decision sample");
    assert!(*p.drives.lock().unwrap() > 0);
    let seen = m.seen.lock().unwrap();
    assert!(!seen.is_empty());
    let man = f.config.manifest(Founder::Loftstrider);
    let (n, k, fw) = (man.menu.len(), man.candidates(), man.row_width());
    for &(menu_len, mask_len, rows, flat, cmask) in seen.iter() {
        assert_eq!((menu_len, mask_len, rows, flat, cmask), (n, n, k, k * fw, k * n));
    }
}

#[test]
fn a_brains_subject_row_is_recorded_as_the_body_it_stands_for() {
    let (w, mut f, me, ids) = observer_world(&[Founder::Chorister]);
    let mut flora = Flora::new(FloraConfig::default());
    let hound = ids[0];
    let lm = menu_of(Founder::Loftstrider);
    let watch = lm.index("watch").unwrap();
    // Its threat slot is the first after the kin slots.
    let row = f.config.manifest(Founder::Loftstrider).kin_slots;
    let mut proposal = [0.0; intent::MAX_MENU];
    proposal[watch] = 1.0;
    let m = probe(Decision { intent: watch, subject: Some(row), proposal, ..Decision::default() });
    f.controllers.set(me, Box::new(m));
    for _ in 0..8 {
        let now = f.tick;
        set_tracks(&mut f, me, &[track(hound, class::THREAT, 7.0, 1.0, now)]);
        f.step(&w, &mut flora);
    }
    let r = f.view().animal(me).unwrap().intent;
    assert_eq!(usize::from(r.held), watch);
    assert_eq!(usize::from(r.proposal), watch);
    assert_eq!(r.subject.body(), Some(hound));
}

#[test]
fn the_tap_records_a_menu_teachers_decision_samples_with_the_label_before_the_hold() {
    let (w, mut flora, mut f, ids) = mixed_world(quiet_config());
    f.start_tap();
    for _ in 0..120 {
        f.step(&w, &mut flora);
    }
    let tap = f.take_tap().unwrap();
    let mut n = 0;
    for &id in &ids {
        let Some(founder) = f.view().animal(id).and_then(|a| a.founder) else { continue };
        let man = f.config.manifest(founder);
        let ds = tap.decisions(id).unwrap_or(&[]);
        if man.menu.plain() {
            assert!(ds.is_empty());
            continue;
        }
        for d in ds {
            n += 1;
            assert_eq!(d.prep.kinds.len(), man.candidates());
            assert_eq!(d.prep.intent_mask.len(), man.menu.len());
            assert!(d.prep.intent_mask[usize::from(d.held)], "a held intent is valid");
            if let Some(r) = d.held_row {
                assert!(d.prep.candidate_mask[r * man.menu.len() + usize::from(d.held)], "a held subject's row is valid for it");
            }
        }
    }
    assert!(n > 0);
}

// ---- Step 4: the kick (one budget), the rest request ----

use crate::kick::{self, Kick, Phase, Refused, Source};

/// A loftstrider with the kick on and a Veilhound `ahead_m` in front of it (east), awake,
/// neither dodging; the clock off, no controllers (both rest).
fn kick_world(ahead_m: f64, k: Kick) -> (World, Flora, Fauna, u64, u64) {
    let w = clock_off(9);
    let flora = Flora::new(FloraConfig::default());
    let mut c = quiet_config();
    c.founders[Founder::Loftstrider.index()].kick = Some(k);
    c.founders[Founder::Chorister.index()].agility = 0.0;
    let mut f = Fauna::new(c);
    let ls = introduce(&mut f, &w, Founder::Loftstrider, StartingStores::FULL);
    let vh = introduce(&mut f, &w, Founder::Chorister, StartingStores::FULL);
    place(&mut f, ls, 4.0, LANE_Z, EAST);
    place(&mut f, vh, 4.0 + ahead_m, LANE_Z, EAST);
    (w, flora, f, ls, vh)
}

fn ticks(s: f64) -> usize {
    (s / crate::DT).round() as usize
}

#[test]
fn a_kick_lands_only_after_its_windup_on_a_target_in_reach_then_recovers() {
    let k = Kick::LOFTSTRIDER;
    let (w, mut flora, mut f, ls, vh) = kick_world(1.0, k);
    let i = index_of(&f, ls);
    kick::request(&mut f, i, vh, Source::Aimed).unwrap();
    let w0 = animal(&f, vh).wound;
    for _ in 0..ticks(k.windup_s) - 1 {
        f.step(&w, &mut flora);
        assert_eq!(animal(&f, vh).wound, w0, "nothing lands during the windup");
        assert_eq!(animal(&f, ls).kick.phase, Phase::Windup);
    }
    f.step(&w, &mut flora);
    assert!(animal(&f, vh).wound > w0, "contact at the windup's end");
    assert_eq!(animal(&f, ls).kick.phase, Phase::Recovery);
    // One budget: another request, aimed or back, is refused through the recovery.
    let i = index_of(&f, ls);
    assert_eq!(kick::request(&mut f, i, vh, Source::Aimed), Err(Refused::Busy));
    assert!(!kick::kick_back(&mut f, ls, vh));
    for _ in 0..ticks(k.recovery_s) {
        f.step(&w, &mut flora);
    }
    assert_eq!(animal(&f, ls).kick.phase, Phase::Ready);
    // Ready again, a bite from inside the arc draws a kick-back.
    assert!(kick::kick_back(&mut f, ls, vh));
    assert_eq!(animal(&f, ls).kick.source, Source::Back);
}

#[test]
fn a_kick_misses_a_target_beyond_reach_and_a_kick_back_from_behind_is_refused() {
    let k = Kick::LOFTSTRIDER;
    let (w, mut flora, mut f, ls, vh) = kick_world(k.reach_m + 3.0, k);
    let i = index_of(&f, ls);
    kick::request(&mut f, i, vh, Source::Aimed).unwrap();
    for _ in 0..ticks(k.windup_s) {
        f.step(&w, &mut flora);
    }
    assert_eq!(animal(&f, vh).wound, 0.0);
    assert_eq!(animal(&f, ls).kick.phase, Phase::Recovery, "a miss spends the budget too");
    // Behind it: out of the arc.
    let (_, _, mut g, ls2, vh2) = kick_world(1.0, k);
    place(&mut g, vh2, 3.0, LANE_Z, EAST);
    let j = index_of(&g, ls2);
    assert_eq!(kick::request(&mut g, j, vh2, Source::Back), Err(Refused::OutOfArc));
    assert!(!kick::kick_back(&mut g, ls2, vh2));
}

#[test]
fn the_kick_tracks_a_target_moving_during_the_windup_within_the_turn_rate() {
    let k = Kick { windup_s: 1.0, ..Kick::LOFTSTRIDER };
    let (w, mut flora, mut f, ls, vh) = kick_world(1.0, k);
    // The target stands to its left-front: the kicker turns toward it, no faster than its cap.
    place(&mut f, vh, 4.7, LANE_Z + 0.7, EAST);
    let h0 = animal(&f, ls).pose.heading_rad;
    let i = index_of(&f, ls);
    kick::request(&mut f, i, vh, Source::Aimed).unwrap();
    f.step(&w, &mut flora);
    let cap = f.config.founder(Founder::Loftstrider).yaw_cap_rad_per_s * crate::DT;
    let turned = (animal(&f, ls).pose.heading_rad - h0).abs();
    assert!(turned > 0.0 && turned <= cap + 1e-9, "turned {turned} within {cap}");
}

#[test]
fn no_lineage_kicks_by_default() {
    let c = FaunaConfig::default();
    assert!(Founder::ALL.iter().all(|&f| c.founder(f).kick.is_none()));
    let (_, _, mut f, ls, vh) = kick_world(1.0, Kick::LOFTSTRIDER);
    f.config.founders[Founder::Loftstrider.index()].kick = None;
    let i = index_of(&f, ls);
    assert_eq!(kick::request(&mut f, i, vh, Source::Aimed), Err(Refused::NoKick));
}

// ---- Step 5: calls and hearing (§11.2) ----

use crate::calls;
use cubarium_voxel::{Command as WorldCommand, Material};

/// A tall plain (10 m; the ring is 12 m round) with loftstriders: a caller at x = 2 m and hearers at the given x, all
/// facing east; its calls table edited by `edit`; a rock wall at `wall_x` metres if any.
fn call_world(hearers: &[f64], wall_x: Option<f64>, edit: impl FnOnce(&mut calls::CallTable)) -> (World, Fauna, u64, Vec<u64>) {
    let mut w = plain_of_height(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, 2, 40);
    if let Some(x) = wall_x {
        let col = (x / VOXEL_M) as i64;
        for z in 0..12 {
            for y in 1..40 {
                w.apply(WorldCommand::SetMaterial { x: col, y, z, material: Material::Rock });
            }
        }
    }
    let mut c = quiet_config();
    let t = &mut c.founders[Founder::Loftstrider.index()].calls;
    *t = calls::CallTable { range_m: 3.0, loudness: [1.0, 0.3], records: 4, ..*t };
    edit(t);
    c.perception.occlusion = 0.25;
    let mut f = Fauna::new(c);
    let caller = introduce(&mut f, &w, Founder::Loftstrider, StartingStores::FULL);
    place(&mut f, caller, 2.0, LANE_Z, EAST);
    let ids = hearers
        .iter()
        .map(|&x| {
            let id = introduce(&mut f, &w, Founder::Loftstrider, StartingStores::FULL);
            place(&mut f, id, x, LANE_Z, EAST);
            id
        })
        .collect();
    f.world_geometry = Some((48.0 * VOXEL_M, VOXEL_M));
    (w, f, caller, ids)
}

fn call_and_hear(w: &World, f: &mut Fauna, caller: u64, kind: usize) {
    let i = index_of(f, caller);
    assert!(calls::emit(f, i, kind));
    calls::hear(f, &w.view());
}

#[test]
fn a_call_is_heard_in_range_muffled_behind_rock_and_unheard_beyond_it() {
    // In range at 2 m on open ground; beyond the 3 m range (5 m one way, 7 m the other) unheard.
    let (w, mut f, caller, ids) = call_world(&[4.0, 7.0], None, |_| {});
    call_and_hear(&w, &mut f, caller, 0);
    let near = calls::heard(&f, ids[0]);
    assert_eq!(near.len(), 1);
    assert!(near[0].proximity > 0.0 && near[0].loudness == 1.0);
    assert!(calls::heard(&f, ids[1]).is_empty(), "beyond range");
    // Behind rock: the same 2 m is past the muffled range (3 × 0.25), and a hearer 0.7 m off
    // behind it hears it muffled.
    let (w, mut f, caller, ids) = call_world(&[4.0, 2.7], Some(2.35), |_| {});
    call_and_hear(&w, &mut f, caller, 0);
    assert!(calls::heard(&f, ids[0]).is_empty(), "muffled out of range behind rock");
    let muffled = calls::heard(&f, ids[1]);
    assert_eq!(muffled.len(), 1);
    assert!(muffled[0].loudness < 1.0, "muffled");
}

#[test]
fn hearing_is_by_range_not_through_the_candidate_cap() {
    let (w, mut f, caller, ids) = call_world(&[3.0, 4.0, 4.5], None, |_| {});
    f.config.perception.max_candidates = 0;
    f.config.founders[Founder::Loftstrider.index()].senses.kin_tracks = 0;
    call_and_hear(&w, &mut f, caller, 0);
    assert!(ids.iter().all(|&id| calls::heard(&f, id).len() == 1));
}

#[test]
fn a_sleeper_records_a_loud_call_it_could_hear_awake_but_not_a_quiet_one_or_one_behind_rock() {
    let asleep = |f: &mut Fauna, id: u64| animal_mut(f, id).state = crate::State::Asleep;
    // Loud (1.0 ≥ 0.5), open: recorded asleep.
    let (w, mut f, caller, ids) = call_world(&[4.0], None, |t| t.asleep_loudness = 0.5);
    asleep(&mut f, ids[0]);
    call_and_hear(&w, &mut f, caller, 0);
    let r = calls::heard(&f, ids[0]);
    assert!(r.len() == 1 && r[0].asleep);
    // Quiet (0.3 < 0.5), in its awake range (0.3 × 3 = 0.9 m at 0.7 m): heard awake, not asleep.
    let (w, mut f, caller, ids) = call_world(&[2.7, 2.7], None, |t| t.asleep_loudness = 0.5);
    asleep(&mut f, ids[0]);
    call_and_hear(&w, &mut f, caller, 1);
    assert!(calls::heard(&f, ids[0]).is_empty());
    assert_eq!(calls::heard(&f, ids[1]).len(), 1, "the awake one hears it");
    // Loud but behind rock (1.0 × 0.25 < 0.5), within the muffled range: heard awake only.
    let (w, mut f, caller, ids) = call_world(&[2.7, 2.7], Some(2.35), |t| t.asleep_loudness = 0.5);
    asleep(&mut f, ids[0]);
    call_and_hear(&w, &mut f, caller, 0);
    assert!(calls::heard(&f, ids[0]).is_empty());
    assert_eq!(calls::heard(&f, ids[1]).len(), 1);
}

#[test]
fn two_calves_bleating_at_once_are_two_records_and_one_repeating_is_one() {
    let (w, mut f, a, ids) = call_world(&[4.0, 6.5], None, |t| t.loudness = [1.0, 1.0]);
    let (hearer, b) = (ids[0], ids[1]);
    let (ia, ib) = (index_of(&f, a), index_of(&f, b));
    assert!(calls::emit(&mut f, ia, 1));
    assert!(calls::emit(&mut f, ib, 1));
    calls::hear(&mut f, &w.view());
    let r = calls::heard(&f, hearer);
    assert_eq!(r.len(), 2, "two bleats from two places");
    assert!(r[0].kind == 1 && r[1].kind == 1);
    // A caller cannot repeat within its cooldown; past it, a call from the same spot
    // refreshes its record rather than adding one.
    assert!(!calls::emit(&mut f, ia, 1));
    f.tick += (f.config.founder(Founder::Loftstrider).calls.cooldown_s / crate::DT) as u64 + 1;
    assert!(calls::emit(&mut f, ia, 1));
    calls::hear(&mut f, &w.view());
    assert_eq!(calls::heard(&f, hearer).len(), 2);
    assert!(calls::heard(&f, hearer).iter().any(|r| r.tick == f.tick));
}

#[test]
fn a_mothers_voice_is_recognised_where_the_lineage_knows_voices_and_never_names_the_caller() {
    let (w, mut f, mother, ids) = call_world(&[4.0], None, |t| t.recognise_voice = true);
    let calf = ids[0];
    animal_mut(&mut f, calf).mother = Some(mother);
    call_and_hear(&w, &mut f, mother, 0);
    assert_eq!(calls::heard(&f, calf)[0].relation, 1);
    let (w, mut f, mother, ids) = call_world(&[4.0], None, |t| t.recognise_voice = false);
    animal_mut(&mut f, ids[0]).mother = Some(mother);
    call_and_hear(&w, &mut f, mother, 0);
    assert_eq!(calls::heard(&f, ids[0])[0].relation, 0);
}
