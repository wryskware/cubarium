//! **SENSES-CORE** (S1 of the senses pass, `design/7_Research/senses-review-2026-10-06.md` §3,
//! §5; Astra's mechanism list in `design/7_Research/astra-senses-2026-10-06.md`), written
//! before the core: the perception record, its pairwise senses, the scent field, the funnel
//! and the egg bank's footfall. A child of `step` beside the predation files, sharing their
//! fixture.
//!
//! Every case builds its scene by hand and checks a mechanism against the config it sets;
//! no default, ratio or population is asserted.

use super::predation_fixture::*;
use crate::perception::{self, HEARING, SIGHT, TREMOR, class};
use crate::scent::ScentChannel;
use crate::{Activity, Fauna, FaunaConfig, Founder, MobilityMode, SenseTable, StartingStores};
use cubarium_voxel::{Command as WorldCommand, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const CH: Founder = Founder::Chorister;
const PREY: Founder = Founder::Browser;
const RING: f64 = 48.0 * VOXEL_M;

/// The Veilhound live with `table`, every lineage awake, remote senses every tick.
fn config(table: SenseTable) -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[CH.index()].senses = SenseTable { live: true, ..table };
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c
}

/// A table with only the given ranges, every floor at its least, a still body seen as far as
/// a moving one.
fn table(sight_m: f64, half_deg: f64, hearing_m: f64, tremor_m: f64) -> SenseTable {
    SenseTable {
        live: true,
        sight_m,
        sight_night_m: sight_m,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: half_deg,
        hearing_m,
        hearing_min: 1e-12,
        tremor_m,
        tremor_min: 1e-12,
        smell_min: 0.0,
        scent: None,
        ..SenseTable::default()
    }
}

/// A lone Veilhound at `x` on the lane facing east.
fn observer(fauna: &mut Fauna, world: &World, x: f64) -> u64 {
    let id = introduce(fauna, world, CH, StartingStores::FULL);
    place(fauna, id, x, LANE_Z, EAST);
    drive(fauna, id, 0.0, false);
    id
}

/// A body of `f` at `(x, z)` moving at `speed` (as read last tick; 0 still).
fn body(fauna: &mut Fauna, world: &World, f: Founder, x: f64, z: f64, speed: f64) -> u64 {
    let id = introduce(fauna, world, f, StartingStores::FULL);
    place(fauna, id, x, z, WEST);
    drive(fauna, id, 0.0, false);
    let a = animal_mut(fauna, id);
    a.speed_m_s = speed;
    a.still_ticks = if speed > 0.0 { 0 } else { u64::MAX / 2 };
    id
}

/// One remote sensing for every live body now.
fn sense(fauna: &mut Fauna, world: &World) {
    fauna.tick += 1;
    perception::sense_pass(fauna, &world.view(), false, 1.0, 1);
}

/// The track `who` holds of `of`, if any.
fn track(fauna: &Fauna, who: u64, of: u64) -> Option<crate::Track> {
    fauna.percept(who).and_then(|p| p.track(of)).copied()
}

fn modes(fauna: &Fauna, who: u64, of: u64) -> u8 {
    track(fauna, who, of).map_or(0, |t| t.modes)
}

/// The plain `height` rows tall with a wall `rise` voxels over the floor on `x ∈ [x0, x1)`
/// across the whole strip.
fn walled(height: u32, walls: &[(f64, f64, u32)], seed: u64) -> World {
    let mut w = plain_of_height(
        cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() },
        seed,
        height,
    );
    for &(x0, x1, rise) in walls {
        for z in 0..12u32 {
            for x in (x0 / VOXEL_M).round() as i64..(x1 / VOXEL_M).round() as i64 {
                for y in SURFACE_Y + 1..=SURFACE_Y + rise {
                    w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
                }
            }
        }
    }
    w
}

fn open(seed: u64) -> World {
    walled(32, &[], seed)
}

// ------------------------------------------------------------------ the pairwise senses

/// Range is the 3D distance from the eye: a bellwing 3 m off on the ground is seen within a
/// 4 m sight; the same bellwing 3 m off and 4 m up (within 4 m horizontally) is not.
#[test]
fn range_is_three_dimensional() {
    let world = open(1);
    let mut fauna = Fauna::new(config(table(4.0, 180.0, 0.0, 0.0)));
    let me = observer(&mut fauna, &world, 2.0);
    let bw = body(&mut fauna, &world, Founder::Bellwing, 5.0, LANE_Z, 0.0);
    {
        let a = animal_mut(&mut fauna, bw);
        a.mobility.mode = MobilityMode::Ground;
        a.mobility.perch = None;
    }
    sense(&mut fauna, &world);
    assert_ne!(modes(&fauna, me, bw) & SIGHT, 0, "3 m off on the ground: seen");
    {
        let a = animal_mut(&mut fauna, bw);
        a.mobility.mode = MobilityMode::Fly;
        a.pose.y_m = SURFACE_M + 4.0;
    }
    sense(&mut fauna, &world);
    assert_eq!(modes(&fauna, me, bw), 0, "3 m off and 4 m up: past a 4 m range in 3D");
    let dx = crate::pose::wrapped_delta(2.0, 0.0, 5.0, 0.0, RING).0.abs();
    assert!(dx < 4.0, "horizontally it is within range (the case can tell): {dx}");
}

/// Every sense that detects a body is in its mask: a moving prey ahead is seen, heard and
/// felt; behind the field of view, heard and felt only; still ahead, seen only. A sense
/// switched off reports nothing, even at the observer's own place.
#[test]
fn every_detecting_sense_is_in_the_mask() {
    let world = open(2);
    let mut fauna = Fauna::new(config(table(4.0, 60.0, 4.0, 4.0)));
    let me = observer(&mut fauna, &world, 3.0);
    let ahead = body(&mut fauna, &world, PREY, 5.0, LANE_Z, 0.5);
    let behind = body(&mut fauna, &world, PREY, 1.0, LANE_Z, 0.5);
    let still = body(&mut fauna, &world, PREY, 5.0, LANE_Z + 1.0, 0.0);
    sense(&mut fauna, &world);
    assert_eq!(modes(&fauna, me, ahead), SIGHT | HEARING | TREMOR, "moving, ahead");
    assert_eq!(modes(&fauna, me, behind), HEARING | TREMOR, "moving, behind its field");
    assert_eq!(modes(&fauna, me, still), SIGHT, "still, ahead");

    let mut fauna = Fauna::new(config(table(0.0, 180.0, 0.0, 0.0)));
    let me = observer(&mut fauna, &world, 3.0);
    let on_top = body(&mut fauna, &world, PREY, 3.0, LANE_Z, 0.5);
    sense(&mut fauna, &world);
    assert_eq!(modes(&fauna, me, on_top), 0, "every sense off: nothing, even overlapping");
}

/// One terrain ray serves the three senses: through a wall a moving prey is not seen, and
/// is heard and felt only through the occlusion factor — none at 0, whole at 1. In the open
/// the same prey is seen.
#[test]
fn one_ray_stops_sight_and_scales_hearing_and_tremor() {
    let world = walled(32, &[(3.0, 3.5, 8)], 3);
    for (occlusion, expect) in [(0.0, 0u8), (1.0, HEARING | TREMOR)] {
        let mut c = config(table(4.0, 180.0, 4.0, 4.0));
        c.perception.occlusion = occlusion;
        let mut fauna = Fauna::new(c);
        let me = observer(&mut fauna, &world, 2.0);
        let p = body(&mut fauna, &world, PREY, 4.5, LANE_Z, 0.5);
        sense(&mut fauna, &world);
        assert_eq!(modes(&fauna, me, p), expect, "through the wall, occlusion {occlusion}");
    }
    let world = open(3);
    let mut fauna = Fauna::new(config(table(4.0, 180.0, 4.0, 4.0)));
    let me = observer(&mut fauna, &world, 2.0);
    let p = body(&mut fauna, &world, PREY, 4.5, LANE_Z, 0.5);
    sense(&mut fauna, &world);
    assert_ne!(modes(&fauna, me, p) & SIGHT, 0, "in the open it is seen (the case can tell)");
}

/// Hearing's emission is `speed^a × length^b` over the distance: at one distance, the
/// loudness of two speeds goes as the speed ratio to the configured `a`, and of two body
/// lengths at one speed as the length ratio to `b`.
#[test]
fn hearing_follows_the_emission_rule() {
    let world = open(4);
    for (a, b) in [(1.0, 1.0), (2.0, 0.5)] {
        let mut c = config(table(0.0, 180.0, 6.0, 0.0));
        c.perception.hearing_speed_exp = a;
        c.perception.hearing_size_exp = b;
        let mut fauna = Fauna::new(c);
        let me = observer(&mut fauna, &world, 2.0);
        let slow = body(&mut fauna, &world, PREY, 4.0, 0.5, 0.2);
        let fast = body(&mut fauna, &world, PREY, 4.0, 2.5, 0.6);
        let other = body(&mut fauna, &world, Founder::Seedporter, 4.0, 1.5, 0.2);
        // Same distance for the two grazers: z symmetric about the lane.
        sense(&mut fauna, &world);
        let loud = |id| track(&fauna, me, id).expect("heard").loud;
        let got = loud(fast) / loud(slow);
        assert!((got - 3f64.powf(a)).abs() < 1e-9 * got.abs().max(1.0), "speed: {got} for a = {a}");
        let len = |id: u64| crate::body_of(&fauna.config, &animal(&fauna, id)).map(|g| g.length_m).unwrap();
        let (ls, lg) = (len(other), len(slow));
        let (ds, dg) = (track(&fauna, me, other).unwrap().d_m, track(&fauna, me, slow).unwrap().d_m);
        let want = (ls / lg).powf(b) * dg.max(c_near(&fauna)) / ds.max(c_near(&fauna));
        let got = loud(other) / loud(slow);
        assert!((got - want).abs() < 1e-9 * want.max(1.0), "size: {got} against {want} for b = {b}");
    }
}

fn c_near(fauna: &Fauna) -> f64 {
    fauna.config.perception.hearing_near_m
}

/// The candidate cap: with more prey in range than `max_candidates`, the record weighs (one
/// ray each) only that many, the nearest.
#[test]
fn the_candidate_cap_weighs_the_nearest() {
    let world = open(5);
    for cap in [3u32, 6] {
        let mut c = config(table(5.0, 180.0, 0.0, 0.0));
        c.perception.max_candidates = cap;
        let mut fauna = Fauna::new(c);
        let me = observer(&mut fauna, &world, 1.0);
        let mut ids: Vec<(f64, u64)> = Vec::new();
        for k in 0..6 {
            let x = 2.0 + 0.5 * k as f64;
            ids.push((x, body(&mut fauna, &world, PREY, x, 0.5 + 0.4 * k as f64, 0.0)));
        }
        let pairs0 = fauna.ledger.percept_pairs;
        sense(&mut fauna, &world);
        let p = *fauna.percept(me).expect("a record");
        let got: Vec<u64> = p.tracks().iter().filter(|t| t.class == class::PREY).map(|t| t.id).collect();
        let want: Vec<u64> = ids.iter().take(cap as usize).map(|&(_, id)| id).collect();
        assert_eq!(got, want, "the {cap} nearest, nearest first");
        assert!(fauna.ledger.percept_pairs - pairs0 <= u64::from(cap), "at most {cap} rays");
    }
}

/// Remote senses run on a body's staggered tick only: at `remote_hz`, on the ticks where
/// `tick ≡ id` modulo the ticks between sensings.
#[test]
fn remote_senses_run_on_their_staggered_tick() {
    let world = open(6);
    let mut c = config(table(5.0, 180.0, 0.0, 0.0));
    c.perception.remote_hz = 4.0;
    let stagger = c.perception.stagger_ticks();
    assert!(stagger > 1, "the case staggers");
    let mut fauna = Fauna::new(c);
    let me = observer(&mut fauna, &world, 1.0);
    body(&mut fauna, &world, PREY, 2.0, LANE_Z, 0.0);
    let mut sensed_at = Vec::new();
    for _ in 0..3 * stagger {
        let before = fauna.percept(me).map(|p| p.tick);
        sense(&mut fauna, &world);
        if fauna.percept(me).map(|p| p.tick) != before {
            sensed_at.push(fauna.tick);
        }
    }
    assert_eq!(sensed_at.len() as u64, 3, "once a stagger: {sensed_at:?}");
    assert!(sensed_at.iter().all(|t| t % stagger == me % stagger), "on its own tick: {sensed_at:?}");
}

/// The record's memory: the focus keeps a body's last-known point and its age while a later
/// sensing does not have it, and moves with it when a sensing does.
#[test]
fn the_focus_keeps_the_last_known_point() {
    let world = open(7);
    let mut fauna = Fauna::new(config(table(3.0, 180.0, 0.0, 0.0)));
    let me = observer(&mut fauna, &world, 2.0);
    let p = body(&mut fauna, &world, PREY, 4.0, LANE_Z, 0.0);
    sense(&mut fauna, &world);
    let now = fauna.tick;
    fauna.percepts.get_mut(&me).unwrap().set_focus(p, now);
    let seen_at = fauna.percept(me).unwrap().focus;
    assert!(seen_at.known && (seen_at.x - 4.0).abs() < 1e-9);
    place(&mut fauna, p, 8.0, LANE_Z, WEST); // 6 m round the ring either way: out of sight
    for _ in 0..5 {
        sense(&mut fauna, &world);
    }
    let f = fauna.percept(me).unwrap().focus;
    assert_eq!((f.x, f.tick), (seen_at.x, seen_at.tick), "unsensed: the last-known point, aging");
    assert!(f.age(fauna.tick) >= 5);
    place(&mut fauna, p, 4.5, LANE_Z, WEST);
    sense(&mut fauna, &world);
    let f = fauna.percept(me).unwrap().focus;
    assert!((f.x - 4.5).abs() < 1e-9 && f.tick == fauna.tick, "sensed again: updated");
}

// ------------------------------------------------------------------ the scent field

/// Steps the scent field `updates` times over the bodies standing.
fn settle(fauna: &mut Fauna, world: &World, flora: &Flora, updates: u32) {
    for _ in 0..updates {
        let mut field = std::mem::take(&mut fauna.scent);
        field.update(&world.view(), &flora.view(), &fauna.animals, &fauna.config);
        fauna.scent = field;
    }
}

fn level(fauna: &Fauna, world: &World, ch: ScentChannel, x_m: f64) -> f64 {
    let site = Site { x: (x_m / VOXEL_M).floor() as u32, y: SURFACE_Y, z: (LANE_Z / VOXEL_M).floor() as u32 };
    fauna.scent_level(&world.view(), ch, site)
}

/// Scent spreads between connected surface cells only: a grazer's scent in a pen walled
/// on both sides by rock taller than the step does not reach the floor past the wall; with
/// one wall a ledge within the step, it does.
#[test]
fn scent_does_not_cross_a_wall_but_crosses_a_connected_ledge() {
    let flora = Flora::new(FloraConfig::default());
    let step_vox = (FaunaConfig::default().scent.step_m / VOXEL_M).floor() as u32;
    let mut far = Vec::new();
    for rise in [step_vox + 4, step_vox.max(1)] {
        let world = walled(32, &[(0.0, 0.5, step_vox + 4), (5.0, 5.5, rise)], 8);
        let mut fauna = Fauna::new(config(table(0.0, 90.0, 0.0, 0.0)));
        fauna.config.founders[PREY.index()].senses.scent = Some(ScentChannel::Grazer);
        body(&mut fauna, &world, PREY, 3.0, LANE_Z, 0.0);
        settle(&mut fauna, &world, &flora, 200);
        assert!(level(&fauna, &world, ScentChannel::Grazer, 4.0) > 0.0, "the pen holds its scent");
        far.push(level(&fauna, &world, ScentChannel::Grazer, 7.0));
    }
    assert_eq!(far[0], 0.0, "past a wall taller than the step: nothing");
    assert!(far[1] > 0.0, "past a ledge within the step: it arrives");
}

/// A body lays scent in proportion to its organic matter; the gradient a receptor reads
/// points to the source.
#[test]
fn scent_follows_mass_and_its_gradient_points_to_the_source() {
    let flora = Flora::new(FloraConfig::default());
    let world = walled(32, &[(5.75, 6.25, 8), (11.75, 12.0, 8), (0.0, 0.25, 8)], 9);
    let mut c = config(table(0.0, 90.0, 0.0, 0.0));
    c.scent.diffuse_fraction = 0.0;
    c.scent.half_life_s = 1e12;
    c.founders[PREY.index()].senses.scent = Some(ScentChannel::Grazer);
    let mut fauna = Fauna::new(c);
    let small = body(&mut fauna, &world, PREY, 3.0, LANE_Z, 0.0);
    let big = body(&mut fauna, &world, PREY, 9.0, LANE_Z, 0.0);
    {
        let a = animal_mut(&mut fauna, small);
        a.reserve *= 0.25;
    }
    settle(&mut fauna, &world, &flora, 1);
    let ratio = level(&fauna, &world, ScentChannel::Grazer, 3.0) / level(&fauna, &world, ScentChannel::Grazer, 9.0);
    let want = animal(&fauna, small).organic() / animal(&fauna, big).organic();
    assert!((ratio - want).abs() < 1e-9, "deposit by mass: {ratio} against {want}");

    let mut fauna = Fauna::new(config(table(0.0, 90.0, 0.0, 0.0)));
    fauna.config.founders[PREY.index()].senses.scent = Some(ScentChannel::Grazer);
    body(&mut fauna, &world, PREY, 2.5, LANE_Z, 0.0);
    settle(&mut fauna, &world, &flora, 60);
    let receptor = body(&mut fauna, &world, Founder::Lanternjaw, 4.0, LANE_Z, 0.0);
    let r = fauna.scent.read(&world.view(), &animal(&fauna, receptor))[ScentChannel::Grazer.index()];
    assert!(r.level > 0.0 && r.gx < 0.0, "east of the source the gradient points west: {r:?}");
    assert!(r.gx.abs() > r.gz.abs(), "along the line to it: {r:?}");
}

// ------------------------------------------------------------------ the egg bank

/// The egg bank's footfall counter reads its own tremor rule, not the senses: the same
/// grazer walking past a buried lanternjaw egg is heard the same with every lineage's senses
/// live and long as with the shipped table.
#[test]
fn the_egg_banks_footfall_does_not_read_the_senses() {
    let run = |c: FaunaConfig| -> u32 {
        let world = clock_off(11);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let site = Site { x: 24, y: SURFACE_Y, z: 6 };
        assert!(fauna.apply(&world, crate::Command::BuryClutch { site, lineage: Founder::Lanternjaw, eggs: 1 }));
        for k in 0..3 {
            let g = introduce(&mut fauna, &world, PREY, StartingStores::FULL);
            place(&mut fauna, g, 4.5 + 0.3 * k as f64, 1.0 + 0.5 * k as f64, EAST);
            drive(&mut fauna, g, 1.0, false);
        }
        for _ in 0..200 {
            fauna.step_with(&world, &mut flora, 1);
        }
        let day = cubarium_voxel::ticks_at_least_one(world.view().config.day.length_min * 60.0);
        let egg = fauna.clutches.iter().find(|c| c.lineage == Founder::Lanternjaw).expect("the egg");
        egg.dormancy.expect("dormant").footfall(fauna.tick, day)
    };
    let shipped = run(hunting_config());
    let mut loud = hunting_config();
    for f in Founder::ALL {
        loud.founders[f.index()].senses = SenseTable { live: true, ..table(8.0, 180.0, 8.0, 8.0) };
    }
    loud.perception.remote_hz = f64::from(crate::TICK_HZ);
    assert!(shipped > 0, "the egg heard the grazers (the case can tell)");
    assert_eq!(run(loud), shipped, "the senses do not change the egg's footfall");
}

// ------------------------------------------------------------------ the configured table

/// Each lineage senses by its own configured table, whatever it is: a lineage made live
/// with sight finds a body of its own lineage ahead, and with every range off finds nothing.
/// A table that is not live is read by nobody's core (its record stays empty of the core).
#[test]
fn each_lineage_senses_by_its_configured_table() {
    let world = open(12);
    for f in [Founder::Browser, Founder::Lanternjaw, Founder::Chorister, Founder::Capgnawer] {
        for (t, sees) in [(table(3.0, 180.0, 0.0, 0.0), true), (table(0.0, 180.0, 0.0, 0.0), false), (SenseTable { live: false, ..table(3.0, 180.0, 0.0, 0.0) }, false)] {
            let mut c = config(table(0.0, 90.0, 0.0, 0.0));
            c.founders[f.index()].senses = t;
            let mut fauna = Fauna::new(c);
            let me = body(&mut fauna, &world, f, 2.0, LANE_Z, 0.0);
            animal_mut(&mut fauna, me).pose.heading_rad = EAST;
            let kin = body(&mut fauna, &world, f, 3.5, LANE_Z, 0.0);
            sense(&mut fauna, &world);
            let got = modes(&fauna, me, kin) & SIGHT != 0;
            assert_eq!(got, sees, "{}: live {} sight {}", f.name(), t.live, t.sight_m);
        }
    }
}

/// A non-default table round-trips through the config's serde for every lineage, and every
/// lineage's numeric senses have a registry row.
#[test]
fn every_lineage_table_is_a_knob() {
    let mut c = FaunaConfig::default();
    for (k, f) in Founder::ALL.iter().enumerate() {
        let s = &mut c.founders[f.index()].senses;
        s.sight_m = 1.0 + k as f64;
        s.hearing_min = 0.5 + k as f64;
        s.smell_min = 0.25 * k as f64;
        s.scent = Some(ScentChannel::ALL[k % ScentChannel::COUNT]);
        s.live = k % 2 == 0;
    }
    let text = serde_json::to_string(&c).expect("serialises");
    let back: FaunaConfig = serde_json::from_str(&text).expect("deserialises");
    for f in Founder::ALL {
        assert_eq!(back.founder(f).senses, c.founder(f).senses, "{}", f.name());
    }
    let reg = crate::registry::registry();
    for f in Founder::ALL {
        for field in ["sight_m", "sight_night_m", "sight_still_fraction", "sight_half_angle_deg", "hearing_m", "hearing_min", "tremor_m", "tremor_min", "smell_min"] {
            let path = format!("fauna.founders.{}.senses.{field}", f.name());
            assert!(reg.rows().iter().any(|r| r.path == path), "a row for {path}");
        }
    }
    for path in ["fauna.perception.max_candidates", "fauna.perception.give_up_s", "fauna.scent.half_life_s", "fauna.scent.step_m"] {
        assert!(reg.rows().iter().any(|r| r.path == path), "a row for {path}");
    }
}

// ------------------------------------------------------------------ the funnel

/// The funnel counts a hunt's stages for its lineage pair, in order: a hungry lanternjaw on
/// its patrol with a still littershredder ahead senses it, sets out after it, strikes, kills
/// and eats its carcass; each stage is counted at most as often as the one before.
#[test]
fn the_funnel_counts_each_stage_in_order() {
    let world = clock_off(13);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
        c.founders[f.index()].agility = 0.0;
    }
    c.hunt.patrol = true;
    c.hunt.patrol_explore = 0.0;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, HUNTING);
    place(&mut fauna, lj, 2.0, LANE_Z, EAST);
    let teachers = fauna.config.teachers;
    fauna.controllers.set(lj, crate::controller::heuristic(Founder::Lanternjaw, teachers));
    animal_mut(&mut fauna, lj).eating = true;
    let prey = body(&mut fauna, &world, Founder::Blind, 3.2, LANE_Z, 0.0);
    let f = |fauna: &Fauna| fauna.ledger.funnel[Founder::Lanternjaw.index()][Founder::Blind.index()];
    step_until(&mut fauna, &world, &mut flora, 600, |fa| f(fa)[perception::EATEN] > 0).expect("sensed, approached, struck, killed and eaten");
    assert!(fauna.view().animal(prey).is_none(), "the prey was killed");
    let s = f(&fauna);
    assert!(s.iter().all(|&n| n > 0), "every stage counted: {s:?}");
    assert!(s[0] >= s[1] && s[1] >= s[2] && s[2] >= s[3], "each at most the one before: {s:?}");
    for g in Founder::ALL.into_iter().filter(|&g| g != Founder::Blind) {
        assert_eq!(fauna.ledger.funnel[Founder::Lanternjaw.index()][g.index()][perception::KILLED], 0, "only its own pair");
    }
}

/// The funnel counts episodes: an event is first once, not again while it recurs within the
/// window, and first again after a gap longer than the window; stages and keys are apart.
#[test]
fn the_funnel_counts_an_episode_once() {
    let mut p = crate::Percept::default();
    let w = 10;
    assert!(p.first(perception::SENSED, 7, 100, w), "a new prey");
    assert!(!p.first(perception::SENSED, 7, 105, w), "still sensed");
    assert!(!p.first(perception::SENSED, 7, 115, w), "refreshed at 105, so 115 is within the window");
    assert!(p.first(perception::APPROACHED, 7, 115, w), "another stage");
    assert!(p.first(perception::SENSED, 8, 115, w), "another prey");
    assert!(p.first(perception::SENSED, 7, 126, w), "lost longer than the window: a new episode");
}

/// **Study** (run by name): the grazer channel's steady level by distance from one still
/// adult frondgrazer on open flat ground the terrarium's depth (6 m), at the config's scent
/// knobs — what the smell floors in [`crate::body::senses_of`] were read from.
#[test]
#[ignore = "study: run by name"]
fn smell_floors_follow_the_field() {
    use cubarium_voxel::{Config as VoxelConfig, weather::{DayClock, DayMode}};
    let (width, depth) = (128u32, 24u32);
    let mut world = World::empty(VoxelConfig {
        width,
        height: 8,
        depth,
        voxel_m: VOXEL_M,
        seed: 14,
        day: DayClock { mode: DayMode::Off, ..Default::default() },
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=SURFACE_Y {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let g = introduce(&mut fauna, &world, PREY, StartingStores::FULL);
    place(&mut fauna, g, 16.0, 3.0, EAST);
    eprintln!("organic {:.3}", animal(&fauna, g).organic());
    settle(&mut fauna, &world, &flora, 2400);
    for d in [0.0, 1.0, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0, 6.0, 7.0, 8.0] {
        let site = Site { x: ((16.0 + d) / VOXEL_M).floor() as u32, y: SURFACE_Y, z: (3.0 / VOXEL_M) as u32 };
        eprintln!("{d:>4} m  {:.3e}", fauna.scent_level(&world.view(), ScentChannel::Grazer, site));
    }
}
