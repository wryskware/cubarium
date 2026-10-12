//! **TEST-PASS (independent), PACK-TUNE and S3's `flyer_takeable`**: the pack's prospect
//! read factor by factor against the config the case sets, its join with the flyer rule, the
//! commit and break-off measured on the same target, and the jaw band's edges. A child of
//! `step` sharing the predation fixture.
//!
//! Every case sets the config it relies on; no shipped default, ratio or verdict is asserted.

use super::predation_fixture::*;
use crate::{Activity, Command, Fauna, FaunaConfig, Founder, HuntConfig, Hunker, MobilityMode, Pose, StartingStores, Track};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const CH: Founder = Founder::Chorister;
const PREY: Founder = Founder::Browser;
const BW: Founder = Founder::Bellwing;
const RING: f64 = 48.0 * VOXEL_M;

fn world(seed: u64) -> World {
    plain(cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }, seed)
}

/// Every lineage awake, the patrol on with no exploring, no dodge, the Veilhound's sight all
/// round to 7 m (still bodies too), remote senses every tick, and the prospect's factors
/// neutral (1, no wound gain) so a case can switch on the one it reads.
fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[PREY.index()].agility = 0.0;
    c.hunt = HuntConfig { patrol: true, patrol_explore: 0.0, ..HuntConfig::default() };
    c.founders[CH.index()].senses = crate::SenseTable {
        live: true,
        sight_m: 7.0,
        sight_night_m: 7.0,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: 180.0,
        hearing_m: 0.0,
        hearing_min: 1e-12,
        ..crate::SenseTable::default()
    };
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let pk = c.founders[CH.index()].pack.as_mut().expect("the chorister's pack table");
    pk.select_kin_factor = 1.0;
    pk.select_aware_factor = 1.0;
    pk.select_wound_gain = 0.0;
    pk.select_airborne_factor = 1.0;
    // The old pack tactic's rules: VH-TACTICS off (the comparison arm).
    crate::pack::tactics::old_tactic(&mut c);
    c
}

/// A Veilhound pack founded at x ∈ [2, 2.5] m on its heuristics, hungry; the members, breeder
/// first.
fn pack(fauna: &mut Fauna, world: &World) -> Vec<u64> {
    let den = Site { x: 10, y: SURFACE_Y, z: 6 };
    assert!(fauna.apply(world, Command::FoundPack { den, founder: CH, stores: HUNTING, heading_rad: EAST }));
    let teachers = fauna.config.teachers;
    let breeder = fauna.packs()[0].breeder;
    let mut members: Vec<u64> = fauna.animals.iter().filter(|a| a.founder == Some(CH)).map(|a| a.id).collect();
    members.sort_by_key(|&id| Some(id) != breeder);
    for (k, &id) in members.iter().enumerate() {
        fauna.controllers.set(id, crate::controller::heuristic(CH, teachers));
        place(fauna, id, 2.0 + 0.5 * (k / 2) as f64, 0.75 + 1.5 * (k % 2) as f64, EAST);
        animal_mut(fauna, id).eating = true;
    }
    members
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("alive")
}

fn grazer(fauna: &mut Fauna, world: &World, x: f64, z: f64, fraction: f64) -> u64 {
    let id = introduce(fauna, world, PREY, StartingStores::FULL);
    place(fauna, id, x, z, WEST);
    let birth = fauna.config.founder(PREY).core.birth_body;
    animal_mut(fauna, id).body = fraction * birth;
    drive(fauna, id, 0.0, false);
    id
}

/// A track of `id` where it stands, on the ground.
fn ground_track(fauna: &Fauna, id: u64) -> Track {
    let a = animal(fauna, id);
    Track { known: true, id, x: a.pose.x, z: a.pose.z, grounded: true, tick: fauna.tick, ..Track::default() }
}

fn prospect(fauna: &Fauna, member: u64, prey: u64, t: &Track, n: u32) -> f64 {
    let o = animal(fauna, prey);
    crate::pack::prospect(fauna, index(fauna, member), &o, t, n, RING)
}

// ------------------------------------------------------------------ the prospect

/// **The hunker gate is exact**: on a hunkering adult the prospect is 0 with one member fewer
/// than [`Hunker::members_to_open`] and, with that many, the share of their pry left over the
/// resistance (`1 − R / (n·p)`, every other factor neutral).
#[test]
fn the_prospect_on_a_hunkering_adult_opens_at_exactly_the_members_it_takes() {
    let world = world(201);
    let mut c = config();
    c.founders[PREY.index()].hunker = Hunker { enabled: true, pry_per_member_s: 0.1, resist_per_s: 0.25, ring_resist_per_kin: 0.0, ..crate::vigil::hunker_of(PREY) };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world);
    let g = grazer(&mut fauna, &world, 4.0, LANE_Z, 1.0);
    let h = fauna.config.founder(PREY).hunker;
    let birth = fauna.config.founder(PREY).core.birth_body;
    let need = h.members_to_open(birth, birth, 0).expect("a number opens it");
    let t = ground_track(&fauna, g);
    assert_eq!(prospect(&fauna, members[0], g, &t, need - 1), 0.0, "{} members cannot open it", need - 1);
    let got = prospect(&fauna, members[0], g, &t, need);
    let want = 1.0 - h.resist_per_s / (f64::from(need) * h.pry_per_member_s);
    assert!((got - want).abs() < 1e-12, "{need} members: prospect {got}, want {want}");
}

/// **A body already turned over needs no opening**: the hunker gate is skipped for a target
/// the pack has opened, so one member's prospect is positive where it was 0.
#[test]
fn an_opened_target_has_a_prospect_for_a_single_member() {
    let world = world(202);
    let mut c = config();
    c.founders[PREY.index()].hunker = Hunker { enabled: true, pry_per_member_s: 0.1, resist_per_s: 0.25, ..crate::vigil::hunker_of(PREY) };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world);
    let g = grazer(&mut fauna, &world, 4.0, LANE_Z, 1.0);
    let t = ground_track(&fauna, g);
    assert_eq!(prospect(&fauna, members[0], g, &t, 1), 0.0);
    fauna.percepts.entry(g).or_default().vigil.opened_until = fauna.tick + 100;
    assert!(prospect(&fauna, members[0], g, &t, 1) > 0.0, "opened: one member may take it");
}

/// **The kin factor counts awake kin within `select_kin_m`, at most `select_kin_max`**: on a
/// prey with no hunker, each such kin multiplies the prospect by `select_kin_factor`; an asleep
/// one and one beyond the radius do not.
#[test]
fn the_kin_factor_counts_awake_kin_within_its_radius_up_to_its_cap() {
    let world = world(203);
    let mut c = config();
    c.founders[PREY.index()].hunker.enabled = false;
    let pk = c.founders[CH.index()].pack.as_mut().unwrap();
    pk.select_kin_factor = 0.5;
    pk.select_kin_m = 1.0;
    pk.select_kin_max = 2;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world);
    let x = 4.5;
    let g = grazer(&mut fauna, &world, x, LANE_Z, 1.0);
    let t = ground_track(&fauna, g);
    let alone = prospect(&fauna, members[0], g, &t, 4);
    assert!(alone > 0.0);
    grazer(&mut fauna, &world, x + 0.5, LANE_Z, 1.0);
    let one = prospect(&fauna, members[0], g, &t, 4);
    assert!((one - 0.5 * alone).abs() < 1e-12, "one kin: {one} against {alone}");
    let asleep = grazer(&mut fauna, &world, x - 0.5, LANE_Z, 1.0);
    animal_mut(&mut fauna, asleep).state = crate::State::Asleep;
    grazer(&mut fauna, &world, x + 1.5, LANE_Z, 1.0);
    assert!((prospect(&fauna, members[0], g, &t, 4) - one).abs() < 1e-12, "asleep and far kin do not count");
    grazer(&mut fauna, &world, x, LANE_Z + 0.5, 1.0);
    grazer(&mut fauna, &world, x, LANE_Z - 0.5, 1.0);
    let capped = prospect(&fauna, members[0], g, &t, 4);
    assert!((capped - 0.25 * alone).abs() < 1e-12, "three awake kin, capped at two: {capped} against {alone}");
}

/// **The awareness factor reads awareness of a grown member**: a prey whose record holds a
/// pack adult has its prospect × `select_aware_factor`; a record that holds only a pup of
/// the pack does not.
#[test]
fn the_awareness_factor_reads_a_grown_member_not_a_pup() {
    let world = world(204);
    let mut c = config();
    c.founders[PREY.index()].hunker.enabled = false;
    c.founders[CH.index()].pack.as_mut().unwrap().select_aware_factor = 0.4;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world);
    let g = grazer(&mut fauna, &world, 4.5, LANE_Z, 1.0);
    let t = ground_track(&fauna, g);
    let unaware = prospect(&fauna, members[0], g, &t, 4);
    let now = fauna.tick;
    let hold = |fauna: &mut Fauna, id: u64| {
        let p = fauna.percepts.entry(g).or_default();
        p.tracks[0] = Track { known: true, id, tick: now, ..Track::default() };
        p.n = 1;
    };
    hold(&mut fauna, members[1]);
    let aware = prospect(&fauna, members[0], g, &t, 4);
    assert!((aware - 0.4 * unaware).abs() < 1e-12, "aware of a member: {aware} against {unaware}");
    let pack_id = fauna.packs()[0].id;
    animal_mut(&mut fauna, members[3]).pack = Some(crate::PackMember { pack: pack_id, pup: true, calling_ticks: 0 });
    hold(&mut fauna, members[3]);
    assert!((prospect(&fauna, members[0], g, &t, 4) - unaware).abs() < 1e-12, "aware of a pup only: no factor");
}

// ------------------------------------------------------------------ PACK-TUNE × S3: the flyer

/// A bellwing at `(x, LANE_Z)` with its base `lift` over the plain, in `mode`.
fn bellwing(fauna: &mut Fauna, world: &World, x: f64, lift: f64, mode: MobilityMode) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft { pose: Pose { x, z: LANE_Z, heading_rad: WEST, y_m: SURFACE_M }, founder: BW, stores: StartingStores::FULL },
    ));
    let id = fauna.view().ledger.births - 1;
    drive(fauna, id, 0.0, false);
    let a = animal_mut(fauna, id);
    a.pose.y_m = SURFACE_M + lift;
    a.mobility.mode = mode;
    a.mobility.falling = false;
    id
}

fn jaw_band(fauna: &Fauna, member: u64) -> f64 {
    let h = fauna.config.founder(CH).hunter.expect("a hunter");
    let g = crate::body_of(&fauna.config, &animal(fauna, member)).expect("a body");
    h.strike_height_m(&g)
}

/// **The prospect and the strike agree on a flyer**: a bellwing flying with its base above the
/// member's jaw band has prospect 0 though its track says it is grounded (the strike's rule
/// wins), one hovering low in the band has the config's airborne share of a perched one's.
#[test]
fn a_flyer_has_a_prospect_only_where_the_strike_can_take_it() {
    let world = world(205);
    let mut c = config();
    c.founders[CH.index()].pack.as_mut().unwrap().select_airborne_factor = 0.3;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world);
    let base = SURFACE_M;
    for &m in &members {
        animal_mut(&mut fauna, m).pose.y_m = base;
    }
    let band = jaw_band(&fauna, members[0]);
    let high = bellwing(&mut fauna, &world, 4.0, band + 0.2, MobilityMode::Fly);
    let t = Track { known: true, id: high, grounded: true, tick: fauna.tick, ..Track::default() };
    assert_eq!(prospect(&fauna, members[0], high, &t, 4), 0.0, "above the band: no prospect");
    let low = bellwing(&mut fauna, &world, 4.5, 0.0, MobilityMode::Fly);
    let perched = bellwing(&mut fauna, &world, 5.0, 0.0, MobilityMode::Perched);
    let pl = prospect(&fauna, members[0], low, &ground_track(&fauna, low), 4);
    let pp = prospect(&fauna, members[0], perched, &ground_track(&fauna, perched), 4);
    assert!(pp > 0.0 && (pl - 0.3 * pp).abs() < 1e-12, "low hover {pl}, perched {pp}");
    animal_mut(&mut fauna, low).mobility.falling = true;
    assert_eq!(prospect(&fauna, members[0], low, &ground_track(&fauna, low), 4), 0.0, "dropping: never");
}

/// **The jaw band's edges** (`flyer_takeable`): a flyer whose base sits exactly at the top of
/// the hunter's band, or whose top sits exactly at the hunter's base, is out of reach; a hair
/// inside either edge it is in reach.
#[test]
fn a_flyer_touching_the_jaw_band_only_at_an_edge_is_not_takeable() {
    let world = world(206);
    let c = config();
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let members = pack(&mut fauna, &world);
    let m = members[0];
    animal_mut(&mut fauna, m).pose.y_m = SURFACE_M;
    let band = jaw_band(&fauna, m);
    let h = fauna.config.founder(CH).hunter.unwrap();
    let bw = bellwing(&mut fauna, &world, 4.0, 0.0, MobilityMode::Fly);
    let height = crate::body_of(&fauna.config, &animal(&fauna, bw)).unwrap().height_m;
    let at = |fauna: &mut Fauna, base: f64| {
        animal_mut(fauna, bw).pose.y_m = base;
        let (a, o) = (animal(fauna, m), animal(fauna, bw));
        crate::predation::flyer_takeable(&fauna.config, &a, &h, BW, &o)
    };
    assert!(!at(&mut fauna, SURFACE_M + band), "base at the band's top");
    assert!(at(&mut fauna, SURFACE_M + band - 1e-6), "a hair under the band's top");
    assert!(!at(&mut fauna, SURFACE_M - height), "top at the hunter's base");
    assert!(at(&mut fauna, SURFACE_M - height + 1e-6), "a hair over the hunter's base");
}

// ------------------------------------------------------------------ commit and break-off

/// **A pack does not break off a target only for being far away**: a hungry pack commits to a
/// soft juvenile it can open with one member, standing farther than twice its pack radius
/// from every member; while the pack closes in over the next second the target is kept (a
/// test that has not begun cannot go badly). The control arm, the same scene with a pack
/// radius that covers the target, keeps it.
#[test]
fn a_pack_keeps_a_far_target_it_committed_to_while_it_closes_in() {
    let arm = |pack_radius_m: f64| {
        let world = world(207);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config();
        let cruise = c.manifest(CH).cruise_m_per_s;
        let pk = c.founders[CH.index()].pack.as_mut().unwrap();
        pk.pack_radius_m = pack_radius_m;
        pk.route_reach_fraction = 6.0 / (cruise * pk.chase_give_up_s);
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let members = pack(&mut fauna, &world);
        let g = grazer(&mut fauna, &world, 6.0, LANE_Z, 0.5);
        let h = fauna.config.founder(PREY).hunker;
        let birth = fauna.config.founder(PREY).core.birth_body;
        assert!(h.enabled && h.members_to_open(0.5 * birth, birth, 0) == Some(1), "the case needs a soft juvenile one member opens");
        let nearest = |f: &Fauna| {
            let p = animal(f, g).pose;
            members.iter().map(|&m| {
                let a = animal(f, m).pose;
                let (dx, dz) = crate::pose::wrapped_delta(a.x, a.z, p.x, p.z, RING);
                (dx * dx + dz * dz).sqrt()
            }).fold(f64::INFINITY, f64::min)
        };
        // The commit is read from the chase counter: a commit and a break-off can fall in one tick.
        step_until(&mut fauna, &world, &mut flora, 300, |f| f.ledger.pack_chases > 0).expect("the pack commits");
        let at_commit = nearest(&fauna);
        for _ in 0..crate::TICK_HZ {
            fauna.step_with(&world, &mut flora, 1);
        }
        (at_commit, (fauna.ledger.pack_chases, fauna.ledger.pack_break_offs, fauna.packs()[0].target == Some(g)))
    };
    let (d, control) = arm(6.0);
    assert_eq!(control, (1, 0, true), "control (radius 6 m, nearest member {d} m at the commit)");
    let (d, got) = arm(1.0);
    assert!(d > 2.0, "the case commits from beyond twice the pack radius: {d} m");
    assert_eq!(got, (1, 0, true), "(chases, break-offs, kept) a second after a commit from {d} m");
}
