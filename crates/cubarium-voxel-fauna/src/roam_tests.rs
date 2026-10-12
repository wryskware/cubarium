//! **YOUNG-ROAM law tests** (the brief `design/handoffs/young-roam-2026-10-12.md`, "Checks").
//!
//! Mechanisms on hand-built fixtures, every number read from the config; no tuned default,
//! ratio or verdict is asserted. Nothing steps past a few hundred ticks.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

#[allow(unused_imports)]
use super::*;
use crate::perception::{SIGHT, Track, class, posture};
use crate::{Activity, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use fixture::{EAST, SURFACE_Y, VOXEL_M, WEST, animal, animal_mut, drive, hunting_config, introduce, place};

const LS: Founder = Founder::Loftstrider;
const SP: Founder = Founder::Seedporter;

/// Arena columns and rows (0.25 m voxels): 40 m around the ring, 30 m deep.
const W: u32 = 160;
const D: u32 = 120;
const RING: f64 = W as f64 * VOXEL_M;
const MID_Z: f64 = D as f64 * VOXEL_M / 2.0;

/// A flat soil arena, the day clock off.
fn arena(seed: u64) -> World {
    let mut w = World::empty(VoxelConfig {
        width: W,
        height: 12,
        depth: D,
        voxel_m: VOXEL_M,
        seed,
        day: cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() },
        ..VoxelConfig::default()
    });
    for z in 0..D {
        for x in 0..i64::from(W) {
            for y in 1..=SURFACE_Y {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
        // No stop-and-go pauses: a driven body walks every tick.
        c.founders[f.index()].cadence = crate::Cadence::default();
    }
    c
}

fn world(seed: u64) -> (World, Fauna) {
    let w = arena(seed);
    let mut f = Fauna::new(config());
    f.world_geometry = Some((RING, VOXEL_M));
    f.tick = 1000;
    (w, f)
}

fn index(f: &Fauna, id: u64) -> usize {
    f.animals.iter().position(|a| a.id == id).expect("a live body")
}

fn length(f: &Fauna, id: u64) -> f64 {
    crate::body_of(&f.config, &animal(f, id)).expect("a body").length_m
}

/// What body `me` sees of body `id` now: a sight track as the senses write it.
fn seen(f: &Fauna, me: u64, id: u64, cls: u8, moving: bool) -> Track {
    let (m, o) = (animal(f, me), animal(f, id));
    let (dx, dz) = crate::pose::wrapped_delta(m.pose.x, m.pose.z, o.pose.x, o.pose.z, RING);
    Track {
        known: true,
        id,
        lineage: o.founder.expect("a lineage").index() as u8,
        class: cls,
        modes: SIGHT,
        x: o.pose.x,
        z: o.pose.z,
        y_m: o.pose.y_m,
        site: (o.site.x, o.site.y, o.site.z),
        grounded: true,
        moving,
        d_m: (dx * dx + dz * dz).sqrt(),
        size_m: length(f, id),
        tick: f.tick,
        heading: o.pose.heading_rad,
        posture: posture::ACTIVE,
        ..Track::default()
    }
}

/// Body `me` sees exactly `bodies` now: `(id, class, moving)`.
fn sees(f: &mut Fauna, me: u64, bodies: &[(u64, u8, bool)]) {
    let tracks: Vec<Track> = bodies.iter().map(|&(id, c, m)| seen(f, me, id, c, m)).collect();
    let mut p = f.percepts.get(&me).copied().unwrap_or_default();
    p.tracks = [Track::default(); crate::perception::MAX_TRACKS];
    p.tracks[..tracks.len()].copy_from_slice(&tracks);
    p.n = tracks.len() as u8;
    p.tick = f.tick;
    f.percepts.insert(me, p);
}

/// A mother of `founder` at `(x, MID_Z)` and her young `share` of her `birth_body`, aged
/// `age_s`, at `(x + gap, MID_Z)`; the young sees her.
fn family(f: &mut Fauna, w: &World, founder: Founder, x: f64, gap: f64, share: f64, age_s: f64) -> (u64, u64) {
    let mother = introduce(f, w, founder, StartingStores::FULL);
    let young = introduce(f, w, founder, StartingStores::FULL);
    let birth = f.config.founder(founder).core.birth_body;
    animal_mut(f, young).body = share * birth;
    animal_mut(f, young).mother = Some(mother);
    animal_mut(f, young).age_ticks = (age_s / cubarium_voxel::DT).round() as u64;
    place(f, mother, x, MID_Z, WEST);
    place(f, young, x + gap, MID_Z, EAST);
    drive(f, mother, 0.0, false);
    drive(f, young, 0.0, false);
    sees(f, young, &[(mother, class::KIN, false)]);
    (mother, young)
}

fn set_age(f: &mut Fauna, id: u64, age_s: f64) {
    animal_mut(f, id).age_ticks = (age_s / cubarium_voxel::DT).round() as u64;
}

// ------------------------------------------------------------------ step 1: young pace

#[test]
fn a_young_bodys_cruise_is_its_own_lengths_never_under_the_floor_and_an_adults_is_its_lineages() {
    let c = config();
    for founder in Founder::ALL {
        let p = c.founder(founder);
        let adult = c.manifest(founder).cruise_m_per_s;
        let birth = p.core.birth_body;
        // An adult, at and past its adult body: the lineage's cruise.
        for body in [birth, p.core.body_max.max(birth)] {
            assert_eq!(c.body_manifest(founder, body).cruise_m_per_s, adult, "{} adult", founder.name());
        }
        // A young body: its own length's, cube root of its body over the adult's.
        let floor = p.young.pace_floor;
        for share in [0.9, 0.5, 0.2, 1e-3] {
            let body = share * birth;
            let own = (body / birth).cbrt();
            let want = adult * own.max(floor).min(1.0);
            let got = c.body_manifest(founder, body).cruise_m_per_s;
            assert!((got - want).abs() <= 1e-12 * adult.max(1.0), "{} at {share}: {got} vs {want}", founder.name());
            assert!(got <= adult, "never faster than the adult");
        }
    }
    // The floor binds a tiny body; a floor of 1 is the adult's pace at any size.
    let mut c = config();
    let p = *c.founder(LS);
    let tiny = 1e-6 * p.core.birth_body;
    let adult = c.manifest(LS).cruise_m_per_s;
    assert!((c.body_manifest(LS, tiny).cruise_m_per_s - p.young.pace_floor * adult).abs() <= 1e-12);
    c.founders[LS.index()].young.pace_floor = 1.0;
    assert_eq!(c.body_manifest(LS, tiny).cruise_m_per_s, adult);
}

#[test]
fn a_driven_calf_moves_at_its_own_lengths_pace_and_a_driven_adult_at_its_lineages() {
    let (w, mut f) = world(1);
    let birth = f.config.founder(LS).core.birth_body;
    let adult = introduce(&mut f, &w, LS, StartingStores::FULL);
    let calf = introduce(&mut f, &w, LS, StartingStores::FULL);
    animal_mut(&mut f, adult).body = birth;
    animal_mut(&mut f, calf).body = 0.2 * birth;
    place(&mut f, adult, 5.0, MID_Z - 4.0, EAST);
    place(&mut f, calf, 5.0, MID_Z + 4.0, EAST);
    drive(&mut f, adult, 1.0, false);
    drive(&mut f, calf, 1.0, false);
    let mut flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());
    let (mut top_adult, mut top_calf) = (0.0f64, 0.0f64);
    for _ in 0..60 {
        f.step(&w, &mut flora);
        top_adult = top_adult.max(f.animals[index(&f, adult)].speed_m_s);
        top_calf = top_calf.max(f.animals[index(&f, calf)].speed_m_s);
    }
    let cruise = f.config.manifest(LS).cruise_m_per_s;
    let scale = crate::roam::pace_scale(f.config.founder(LS), 0.2 * birth);
    assert!(scale < 1.0);
    assert!((top_adult - cruise).abs() <= 0.02 * cruise, "adult {top_adult} vs cruise {cruise}");
    assert!((top_calf - scale * cruise).abs() <= 0.02 * cruise, "calf {top_calf} vs {}", scale * cruise);
}

// ------------------------------------------------------------------ step 2: the leash

#[test]
fn the_leash_grows_with_age_from_follow_m_to_roam_m_and_ends_at_independence() {
    let (w, mut f) = world(2);
    let y = f.config.founder(LS).young;
    assert!(y.roams() && y.roam_m > y.follow_m);
    // Between the two comforts: pulled at birth, left alone near independence.
    let gap = 0.5 * (y.follow_m + y.roam_m);
    let (_, calf) = family(&mut f, &w, LS, 10.0, gap, 0.3, 0.0);
    let i = index(&f, calf);
    let mut last = 0.0;
    for k in [0.0, 0.25, 0.5, 0.75, 0.999] {
        set_age(&mut f, calf, k * y.independence_s);
        let (comfort, weight) = leash(&f, i).expect("a young calf with a mother is leashed");
        let want = y.follow_m + (y.roam_m - y.follow_m) * k;
        assert!((comfort - want).abs() <= 1e-3 * y.roam_m, "at {k}: {comfort} vs {want}");
        assert!(comfort >= last);
        assert_eq!(weight, y.follow_pull);
        last = comfort;
    }
    set_age(&mut f, calf, 0.0);
    let pull = crate::nurse::mother_pull(&f, i, RING).expect("it senses its mother");
    let pull = pull.expect("past its birth comfort: pulled toward her");
    assert!(pull.dx < 0.0, "toward its mother, west of it");
    assert_eq!(pull.weight, y.follow_pull);
    set_age(&mut f, calf, 0.999 * y.independence_s);
    assert_eq!(crate::nurse::mother_pull(&f, i, RING), Some(None), "within its grown comfort: no pull");
    // At independence the leash is gone: the family's pull again.
    set_age(&mut f, calf, y.independence_s);
    assert!(leash(&f, i).is_none());
    assert!(crate::nurse::mother_pull(&f, i, RING).is_none());
    // No recorded mother: no leash.
    set_age(&mut f, calf, 0.0);
    animal_mut(&mut f, calf).mother = None;
    assert!(leash(&f, i).is_none());
}

#[test]
fn a_seedporter_infant_before_weaning_keeps_its_nursing_follow_and_never_strays() {
    let (w, mut f) = world(3);
    let n = f.config.founder(SP).nursing;
    let y = f.config.founder(SP).young;
    assert!(n.on() && y.roams() && y.independence_s > n.weaning_age_s);
    let (_, young) = family(&mut f, &w, SP, 10.0, n.leash_m.max(n.follow_m) + 0.5, 0.3, 0.0);
    let i = index(&f, young);
    for k in [0.0, 0.5, 0.999] {
        set_age(&mut f, young, k * n.weaning_age_s);
        assert!(crate::nurse::dependent(&f, i));
        assert!(progress(&f, i).is_none(), "an infant has no roam");
        // Its comfort is the nursing leash, whatever the young table says.
        let pull = crate::nurse::mother_pull(&f, i, RING).expect("it senses its mother");
        assert!(pull.is_some_and(|p| p.weight == n.follow_pull.clamp(0.0, 1.0)), "pulled back past the nursing leash");
    }
    // Weaned: the roam starts at `follow_m` and grows.
    set_age(&mut f, young, n.weaning_age_s);
    assert!(!crate::nurse::dependent(&f, i));
    assert_eq!(progress(&f, i), Some(0.0));
    let (c0, _) = leash(&f, i).expect("a weaned young is leashed");
    assert!((c0 - y.follow_m).abs() <= 1e-9);
    set_age(&mut f, young, 0.5 * (n.weaning_age_s + y.independence_s));
    let (c1, _) = leash(&f, i).expect("leashed");
    assert!(c1 > c0 && c1 < y.roam_m);
    // A weaned young weighs its mother on its sensings, as an infant does.
    let mother = animal(&f, young).mother.expect("a mother");
    assert!(crate::nurse::weighs_as_mother(&f, i, mother));
}

// ------------------------------------------------------------------ step 3: excursions

/// Body `me` has heard a call of `kind` from `(x, z)` now, `relation` its recognition of the
/// voice (0 a stranger, 1 its mother, 2 its own young).
fn hears(f: &mut Fauna, me: u64, kind: u8, (x, z): (f64, f64), relation: u8) {
    let r = crate::calls::CallRecord { kind, x, z, y_m: 0.0, loudness: 1.0, proximity: 1.0, tick: f.tick, relation, asleep: false, heading: 0.0 };
    f.heard.insert(me, crate::calls::Heard { records: vec![r] });
}

/// A calf of `founder` midway through its roam, its mother 2 m off and seen, on an excursion
/// to a place `goal_m` east of it set out `ago_s` before now.
fn out_on_excursion(founder: Founder, goal_m: f64, ago_s: f64) -> (World, Fauna, u64, u64) {
    let (w, mut f) = world(4);
    f.tick = 1_000_000;
    let (start, end) = {
        let p = f.config.founder(founder);
        let start = if p.nursing.on() { p.nursing.weaning_age_s } else { 0.0 };
        (start, p.young.independence_s)
    };
    let (mother, calf) = family(&mut f, &w, founder, 10.0, 2.0, 0.3, 0.5 * (start + end));
    let a = animal(&f, calf);
    let since = f.tick - (ago_s / cubarium_voxel::DT).round() as u64;
    let e = Excursion { goal: Goal::Place, track: 0, x: a.pose.x + goal_m, z: a.pose.z, y_m: a.pose.y_m, since };
    f.excursions.insert(calf, e);
    (w, f, mother, calf)
}

fn bleated(f: &Fauna, id: u64) -> bool {
    f.calls.iter().any(|e| e.caller == id && e.kind == 1)
}

fn run_excursion(f: &mut Fauna, id: u64) -> Option<crate::Actions> {
    let i = index(f, id);
    excursion(f, i, true, 0.5, 7, RING)
}

#[test]
fn an_excursion_ends_on_alarm_on_its_mothers_call_and_on_losing_sight_and_the_young_bleats() {
    for founder in [LS, SP] {
        let name = founder.name();
        // Under way, nothing amiss: it walks on toward its goal, east.
        let (_w, mut f, mother, calf) = out_on_excursion(founder, 8.0, 1.0);
        let go = run_excursion(&mut f, calf).expect("still out");
        assert!(go.forward > 0.0 && go.turn.abs() < 1.0, "{name}: heading east to its goal");
        assert!(!bleated(&f, calf));
        // Alarmed: a threat in its record.
        let (w, mut f, mother2, calf2) = out_on_excursion(founder, 8.0, 1.0);
        let vh = introduce(&mut f, &w, Founder::Chorister, StartingStores::FULL);
        place(&mut f, vh, 20.0, MID_Z, WEST);
        sees(&mut f, calf2, &[(mother2, class::KIN, false), (vh, class::THREAT, true)]);
        assert!(run_excursion(&mut f, calf2).is_none());
        assert!(f.excursions.get(&calf2).is_none(), "{name}: ended");
        assert_eq!(f.roam_tally(founder).ended[End::Alarmed.index()], 1);
        assert!(bleated(&f, calf2), "{name}: it bleats when alarmed");
        // The bleat is an executed action of its intent record (kind 1 is `call` 2).
        let i2 = index(&f, calf2);
        let valid = vec![true; crate::intent::menu_of(founder).len()];
        crate::intent::record(&mut f, i2, founder, crate::Steer::Heuristic, crate::intent::Teach::default(), &valid, crate::intent::Executed::default());
        assert_eq!(animal(&f, calf2).intent.executed.call, 2, "{name}");
        // An alarm call heard since it set out, no threat in sight: alarmed too.
        let (_w, mut f, _, calf3) = out_on_excursion(founder, 8.0, 1.0);
        hears(&mut f, calf3, 0, (30.0, MID_Z), 0);
        assert!(run_excursion(&mut f, calf3).is_none());
        assert_eq!(f.roam_tally(founder).ended[End::Alarmed.index()], 1, "{name}");
        assert!(bleated(&f, calf3));
        // Its mother's voice.
        let (_w, mut f, _, calf4) = out_on_excursion(founder, 8.0, 1.0);
        let at = (animal(&f, mother).pose.x, MID_Z);
        hears(&mut f, calf4, 0, at, 1);
        assert!(run_excursion(&mut f, calf4).is_none());
        assert_eq!(f.roam_tally(founder).ended[End::MotherCalled.index()], 1, "{name}");
        assert!(bleated(&f, calf4));
        // Out of sight of its family: no kin in its record.
        let (_w, mut f, _, calf5) = out_on_excursion(founder, 8.0, 1.0);
        sees(&mut f, calf5, &[]);
        assert!(run_excursion(&mut f, calf5).is_none());
        assert_eq!(f.roam_tally(founder).ended[End::LostSight.index()], 1, "{name}");
        assert!(bleated(&f, calf5));
        // At its goal, and after its time: quietly.
        let y = f.config.founder(founder).young;
        let (_w, mut f, _, calf6) = out_on_excursion(founder, 0.5 * y.arrive_m, 1.0);
        assert!(run_excursion(&mut f, calf6).is_none());
        assert_eq!(f.roam_tally(founder).ended[End::Arrived.index()], 1, "{name}");
        assert!(!bleated(&f, calf6));
        let (_w, mut f, _, calf7) = out_on_excursion(founder, 8.0, y.excursion_s + 1.0);
        assert!(run_excursion(&mut f, calf7).is_none());
        assert_eq!(f.roam_tally(founder).ended[End::TimedOut.index()], 1, "{name}");
        assert!(!bleated(&f, calf7));
    }
}

#[test]
fn a_calm_young_sets_off_at_its_chance_and_never_as_an_infant_or_under_threat() {
    for founder in [LS, SP] {
        let name = founder.name();
        let (w, mut f) = world(5);
        let (start, end) = {
            let p = f.config.founder(founder);
            (if p.nursing.on() { p.nursing.weaning_age_s } else { 0.0 }, p.young.independence_s)
        };
        let (mother, calf) = family(&mut f, &w, founder, 10.0, 2.0, 0.3, 0.5 * (start + end));
        // A certain chance: it sets off, toward a goal, recorded.
        f.config.founders[founder.index()].young.excursion_per_h = [1e9, 1e9];
        let go = run_excursion(&mut f, calf).expect("it sets off");
        let e = f.excursion_of(calf).expect("under way");
        assert!(go.forward > 0.0 || go.turn != 0.0);
        assert_eq!(f.roam_tally(founder).started.iter().sum::<u64>(), 1, "{name}");
        // A place goal lies `excursion_m` off, away from its mother (east of her here).
        if e.goal == Goal::Place {
            let a = animal(&f, calf);
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, e.x, e.z, RING);
            assert!((dx.hypot(dz) - f.config.founder(founder).young.excursion_m).abs() < 1e-6);
            assert!(dx > 0.0, "{name}: away from its mother");
        }
        // No chance: it never sets off.
        f.excursions.clear();
        f.config.founders[founder.index()].young.excursion_per_h = [0.0, 0.0];
        for _ in 0..50 {
            f.tick += 1;
            assert!(run_excursion(&mut f, calf).is_none());
        }
        // Under threat it never sets off, whatever the chance.
        f.config.founders[founder.index()].young.excursion_per_h = [1e9, 1e9];
        let vh = introduce(&mut f, &w, Founder::Chorister, StartingStores::FULL);
        place(&mut f, vh, 25.0, MID_Z, WEST);
        sees(&mut f, calf, &[(mother, class::KIN, false), (vh, class::THREAT, false)]);
        assert!(run_excursion(&mut f, calf).is_none(), "{name}");
        // Alone (no kin in sense) it never sets off.
        sees(&mut f, calf, &[]);
        assert!(run_excursion(&mut f, calf).is_none(), "{name}");
        // Grown past independence it never sets off.
        sees(&mut f, calf, &[(mother, class::KIN, false)]);
        set_age(&mut f, calf, end);
        assert!(run_excursion(&mut f, calf).is_none(), "{name}");
    }
    // A seedporter infant before weaning never sets off, whatever the chance.
    let (w, mut f) = world(6);
    let n = f.config.founder(SP).nursing;
    f.config.founders[SP.index()].young.excursion_per_h = [1e9, 1e9];
    let (_, infant) = family(&mut f, &w, SP, 10.0, 0.3, 0.3, 0.5 * n.weaning_age_s);
    for _ in 0..50 {
        f.tick += 1;
        assert!(run_excursion(&mut f, infant).is_none(), "an infant never strays");
    }
}

#[test]
fn a_mother_hearing_her_own_youngs_bleat_goes_to_it() {
    for founder in [LS, SP] {
        let name = founder.name();
        let (w, mut f) = world(8);
        let (mother, calf) = family(&mut f, &w, founder, 10.0, 6.0, 0.3, 1.0);
        let at = (animal(&f, calf).pose.x, MID_Z);
        let i = index(&f, mother);
        // Her own young's voice: she walks toward it (east).
        hears(&mut f, mother, 1, at, 2);
        let (go, _) = answer(&mut f, i, RING).unwrap_or_else(|| panic!("{name}: she answers"));
        let a = animal(&f, mother);
        let want = (at.0 - a.pose.x).atan2(0.0);
        assert!(crate::vigil::head(&a, want, 1.0, 0.0) == go, "{name}: toward the call");
        // A stranger's bleat: not this answer (the defence layer's, under threat).
        hears(&mut f, mother, 1, at, 0);
        assert!(answer(&mut f, i, RING).is_none(), "{name}");
        // Already there: she stops answering.
        let arrive = f.config.founder(founder).calls.arrive_m;
        hears(&mut f, mother, 1, (a.pose.x + 0.5 * arrive, MID_Z), 2);
        assert!(answer(&mut f, i, RING).is_none(), "{name}");
        // A call older than the kind's heed time: forgotten.
        let heed = f.config.founder(founder).calls.heed_s[1];
        hears(&mut f, mother, 1, at, 2);
        f.tick += (heed / cubarium_voxel::DT).ceil() as u64 + 1;
        assert!(answer(&mut f, i, RING).is_none(), "{name}");
    }
}

// ------------------------------------------------------------------ step 4: the intent record

#[test]
fn an_excursion_is_recorded_as_the_young_menus_explore_with_its_goal() {
    for founder in [LS, SP] {
        let menu = crate::intent::menu_of(founder);
        let k = menu.index("explore").expect("the menu has explore");
        assert_eq!(menu.option(k).map(|o| o.who), Some(crate::intent::Who::Young));
        let (_w, mut f, _, calf) = out_on_excursion(founder, 8.0, 1.0);
        let i = index(&f, calf);
        let e = f.excursion_of(calf).expect("under way");
        let teach = crate::intent::teacher_label(&f, i, founder, crate::Steer::Explore, crate::Actions { forward: 1.0, turn: 0.0, feed: 0.0 }, false, false);
        assert_eq!(teach.name, Some("explore"), "{}", founder.name());
        assert_eq!(teach.subject, subject(&e));
        assert!(matches!(teach.subject, crate::intent::Subject::Place { kind: crate::intent::PlaceKind::ExcursionGoal, .. }));
        // An adult's mask never holds it.
        let mask = crate::intent::intent_mask(menu, false, |_| true);
        assert!(!mask[k]);
        let _ = &mut f;
    }
}
