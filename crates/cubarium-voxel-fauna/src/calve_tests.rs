//! **CALF-PLACE mechanism tests** (`design/handoffs/calf-place-2026-10-13.md`): a calf placed
//! by [`Fauna::place_calf`] is a young of its mother as a born one is, on a running world
//! (tick ≠ 0), and [`Fauna::mark_birth`] leaves on its mother what a term would. Hand-built
//! fixtures; every number is read from the config; nothing steps.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use super::*;
use crate::perception::{SIGHT, Track, class, posture};
use crate::{Activity, FaunaConfig, StartingStores};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, TICK_HZ};
use cubarium_voxel_flora::FloraConfig;
use fixture::{EAST, SURFACE_Y, VOXEL_M, WEST, animal, animal_mut, drive, hunting_config, introduce, place};

const LS: Founder = Founder::Loftstrider;
const SP: Founder = Founder::Seedporter;
const CH: Founder = Founder::Chorister;

const W: u32 = 160;
const D: u32 = 120;
const RING: f64 = W as f64 * VOXEL_M;
const MID_Z: f64 = D as f64 * VOXEL_M / 2.0;

/// A flat soil arena; the day a minute long and the year ten days, so a test can set the
/// Cubarium year by the world's tick.
fn arena() -> World {
    let mut climate = cubarium_voxel::weather::Climate::default();
    climate.dry_season = false;
    climate.dry_season_every_days = 10.0;
    climate.dry_season_phase_days = 0.0;
    let mut w = World::empty(VoxelConfig {
        width: W,
        height: 12,
        depth: D,
        voxel_m: VOXEL_M,
        seed: 3,
        day: cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            length_min: 1.0,
            ..Default::default()
        },
        climate,
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
    }
    c
}

/// A running world: the layer's clock past tick 0 (where `stage_identity` stops working).
fn running(c: FaunaConfig) -> (World, Flora, Fauna) {
    let w = arena();
    let mut f = Fauna::new(c);
    f.world_geometry = Some((RING, VOXEL_M));
    f.tick = 1000;
    (w, Flora::new(FloraConfig::for_voxel_size(VOXEL_M)), f)
}

/// A grown body of `founder` at `(x, z)`.
fn grown(f: &mut Fauna, w: &World, founder: Founder, x: f64, z: f64, heading: f64) -> u64 {
    let id = introduce(f, w, founder, StartingStores::FULL);
    place(f, id, x, z, heading);
    drive(f, id, 0.0, false);
    id
}

fn face_at(x: f64, z: f64) -> Spot {
    Spot::Face(Site { x: (x / VOXEL_M).floor() as u32, y: SURFACE_Y, z: (z / VOXEL_M).floor() as u32 })
}

fn index(f: &Fauna, id: u64) -> usize {
    f.animals.iter().position(|a| a.id == id).expect("a live body")
}

fn ticks(s: f64) -> u64 {
    (s * f64::from(TICK_HZ)).round() as u64
}

#[test]
fn a_calf_placed_on_a_running_world_is_its_mothers_young_at_its_age() {
    let (w, flora, mut f) = running(config());
    let mother = grown(&mut f, &w, LS, 10.0, MID_Z, EAST);
    let y = f.config.founder(LS).young;
    assert!(y.roams(), "the loftstrider's young roam");
    let age = ticks(0.25 * y.independence_s);
    assert!(!f.stage_identity(mother, age, None), "the tick-0 staging refuses a running world");
    let calf = f.place_calf(&w, &flora, mother, face_at(10.0, MID_Z), age, None).expect("placed");
    let c = animal(&f, calf);
    assert_eq!(c.founder, Some(LS));
    assert_eq!(c.mother, Some(mother));
    assert_eq!(crate::mother_body(&c), Some(mother));
    assert_eq!(c.age_ticks, age);
    assert_eq!(c.body, f.config.founder(LS).core.body_min, "a newborn's structure");
    assert!(c.lifespan_ticks == 0 || c.lifespan_ticks >= age, "its drawn life runs on from its age");
    assert_eq!(c.pose.heading_rad, EAST, "facing as its mother faces");
    assert!(f.has_controller(calf) || f.factories.make(LS).is_none(), "driven as any placed body");
    assert_eq!(f.ledger.introduced, 2, "booked a placement, not a birth");
    assert_eq!(f.ledger.born, 0);
    // Its roam is its age's: on the leash, part way to independence.
    let i = index(&f, calf);
    let k = crate::roam::progress(&f, i).expect("a leashed young");
    let want = (age as f64 * DT) / y.independence_s;
    assert!((k - want).abs() < 1e-9, "roam {k} at its age, {want}");
    let (comfort, _) = crate::roam::leash(&f, i).expect("a leash");
    assert!(comfort > y.follow_m && comfort < y.roam_m.max(y.follow_m), "its comfort grown with its age: {comfort}");
    assert!(crate::roam::young_of_mother(&f.config, &c));
    // A body placed as a juvenile without a mother has none of this.
    let lone = f.place_body(&w, &flora, LS, face_at(30.0, MID_Z), BodySpec { stage: BodyStage::Juvenile, ..BodySpec::default() });
    let Ok(Placed::Body { id: lone, .. }) = lone else { panic!("placed") };
    assert!(crate::roam::progress(&f, index(&f, lone)).is_none());
    assert!(!crate::roam::young_of_mother(&f.config, &animal(&f, lone)));
}

#[test]
fn a_calf_is_refused_for_a_missing_ungrown_or_egg_laying_mother_and_nothing_changes() {
    let (w, flora, mut f) = running(config());
    let ungrown = grown(&mut f, &w, LS, 10.0, MID_Z, EAST);
    animal_mut(&mut f, ungrown).body = f.config.founder(LS).core.birth_body * 0.5;
    let layer = Founder::ALL
        .into_iter()
        .find(|&g| f.config.founder(g).core.reproduction.mode == BirthMode::Eggs)
        .expect("an egg-laying lineage");
    let hen = grown(&mut f, &w, layer, 20.0, MID_Z, EAST);
    let (n, introduced) = (f.animals.len(), f.ledger.introduced);
    for (mother, at) in [(ungrown, (10.0, MID_Z)), (hen, (20.0, MID_Z)), (9_999, (30.0, MID_Z))] {
        let r = f.place_calf(&w, &flora, mother, face_at(at.0, at.1), 0, None);
        assert!(r.is_err(), "mother {mother}: {r:?}");
    }
    assert_eq!(f.animals.len(), n);
    assert_eq!(f.ledger.introduced, introduced);
}

#[test]
fn a_mother_guards_her_placed_calf_first_and_the_calf_runs_to_her() {
    let (w, flora, mut f) = running(config());
    let mother = grown(&mut f, &w, LS, 20.0, MID_Z, WEST);
    let mine = f.place_calf(&w, &flora, mother, face_at(12.0, MID_Z + 4.0), ticks(600.0), None).expect("placed");
    place(&mut f, mine, 12.0, MID_Z + 4.0, EAST);
    // Another calf, not hers, in more danger.
    let other = f.place_calf(&w, &flora, mother, face_at(12.0, MID_Z - 4.0), ticks(600.0), None).expect("placed");
    place(&mut f, other, 12.0, MID_Z - 4.0, EAST);
    animal_mut(&mut f, other).mother = None;
    let h1 = grown(&mut f, &w, CH, 8.0, MID_Z + 4.0, EAST);
    let h2 = grown(&mut f, &w, CH, 10.0, MID_Z - 4.0, EAST);
    let all = [(mine, class::KIN, false), (other, class::KIN, false), (h1, class::THREAT, true), (h2, class::THREAT, true)];
    sees(&mut f, mother, &all);
    let i = index(&f, mother);
    crate::defend::steer(&mut f, i, &w.view()).expect("defends");
    assert_eq!(f.ls_teach[&mother].subject.body(), Some(mine), "her own calf first");
    // The calf, threatened, runs to its mother over a nearer adult.
    let near = grown(&mut f, &w, LS, 14.0, MID_Z + 4.0, WEST);
    sees(&mut f, mine, &[(near, class::KIN, false), (mother, class::KIN, false), (h1, class::THREAT, true)]);
    let i = index(&f, mine);
    crate::defend::steer(&mut f, i, &w.view()).expect("defends");
    assert_eq!(f.ls_teach[&mine].subject.body(), Some(mother), "its mother, though another adult is nearer");
}

#[test]
fn a_nursing_mother_nurses_her_placed_young() {
    let (w, flora, mut f) = running(config());
    let n = f.config.founder(SP).nursing;
    assert!(n.on(), "the seedporter nurses");
    let mother = grown(&mut f, &w, SP, 10.0, MID_Z, EAST);
    let age = ticks(0.1 * n.weaning_age_s);
    let young = f.place_calf(&w, &flora, mother, face_at(10.0, MID_Z), age, Some(0.1)).expect("placed");
    place(&mut f, young, 10.0, MID_Z, EAST);
    assert!(crate::nurse::dependent(&f, index(&f, young)));
    let (m0, y0) = (animal(&f, mother).reserve, animal(&f, young).reserve);
    crate::nurse::nurse(&mut f, &w.view());
    let (m1, y1) = (animal(&f, mother).reserve, animal(&f, young).reserve);
    assert!(m1 < m0 && y1 > y0, "she passes reserve ({m0} -> {m1}) and it receives ({y0} -> {y1})");
}

#[test]
fn a_pack_mothers_calf_joins_her_pack_as_a_pup() {
    let (w, flora, mut f) = running(config());
    let Ok(Placed::Body { id: mother, pack: Some(pack) }) =
        f.place_body(&w, &flora, CH, face_at(10.0, MID_Z), BodySpec { group: Group::New, ..BodySpec::default() })
    else {
        panic!("a pack founded")
    };
    assert_eq!(f.config.founder(CH).core.reproduction.mode, BirthMode::Gestation, "the Veilhound bears live young");
    let pup = f.place_calf(&w, &flora, mother, face_at(10.0, MID_Z), 0, None).expect("placed");
    let p = animal(&f, pup).pack.expect("in a pack");
    assert_eq!((p.pack, p.pup), (pack, true));
}

#[test]
fn mark_birth_holds_what_is_left_of_the_refractory_and_counts_the_conception_year() {
    let mut c = config();
    let r = &mut c.founders[LS.index()].core.reproduction;
    r.litters_per_year = 1;
    let mut w = arena();
    let year_s = w.config().climate.year_s(&w.config().day);
    assert!(year_s > 0.0);
    r.gestation_s = 0.25 * year_s;
    r.birth_interval_s = 1.5 * year_s;
    let rule = *r;
    // The world at the middle of year 2.
    for _ in 0..ticks(2.5 * year_s) {
        w.advance_tick();
    }
    let mut f = Fauna::new(c);
    f.tick = w.tick();
    let year_of = |w: &World, back_s: f64| w.config().climate.year(&w.config().day, w.tick() - ticks(back_s)).expect("a year").number;
    for (k, age_s) in [0.1, 0.6, 2.0].into_iter().enumerate() {
        let m = grown(&mut f, &w, LS, 10.0 + 10.0 * k as f64, MID_Z, EAST);
        let age = ticks(age_s * year_s);
        assert!(f.mark_birth(&w.view(), m, age));
        let a = animal(&f, m);
        assert_eq!(a.reproduction.refractory_ticks, rule.interval_ticks().saturating_sub(age), "age {age_s} y");
        let conceived = year_of(&w, (age_s + 0.25) * year_s);
        assert_eq!(a.reproduction.season.used_in(conceived), 1, "age {age_s} y: conceived in year {conceived}");
        let now = w.view().year().expect("a year").number;
        if conceived != now {
            assert_eq!(a.reproduction.season.used_in(now), 0, "an earlier year's conception leaves this year open");
        }
    }
    assert!(!f.mark_birth(&w.view(), 9_999, 0), "no such body");
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
        size_m: crate::body_of(&f.config, &o).expect("a body").length_m,
        tick: f.tick,
        heading: o.pose.heading_rad,
        posture: posture::ACTIVE,
        ..Track::default()
    }
}
