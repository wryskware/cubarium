//! **SNAIL-LAND** (Wrysk, 2026-10-06: "snails should lay eggs on land"): the ripple-snail's
//! clutch goes on a face out of standing water near the water's edge, on moist ground
//! ([`crate::EggSite::Shore`], the shore knobs on [`crate::Amphibious`]); an egg left under
//! standing water past `egg_drown_s`, or on ground drier than `egg_dry_moisture` past
//! `egg_dry_s`, dies to the litter of its face; a ready layer walks ashore to lay and goes
//! back to the water.
//!
//! Every number is set here or read from the config; nothing tuned is pinned.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

use crate::{Actions, Command, Fauna, FaunaConfig, Founder, Reproduction, Scripted, StartingStores};

const V: f64 = 0.25;
const SNAIL: Founder = Founder::RippleSnail;
const W: u32 = 24;
const D: u32 = 6;

/// Set the pore fill of soil voxel `(x, y, z)` to `p` (free water put into it as air, which
/// turned back to soil keeps as pore water).
fn set_pore(world: &mut World, x: i64, y: u32, z: u32, p: f64) {
    let cap = world.config().materials.pore_capacity(Material::Soil) * world.config().voxel_volume();
    world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Air });
    if p > 0.0 {
        world.apply(WorldCommand::AddWater { x, y, z, volume_m3: p * cap });
    }
    world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
}

/// The shore: soil `y = 1..=2` everywhere (`W × 10 × D` at 0.25 m, day clock `day`); a pond
/// of one cell of water over columns `0..=5` (face `y = 2`, water surface 1.0 m); a bank
/// one cell up at columns 6 and 8..=23 (face `y = 3`, its top at the water's surface) and
/// a bluff three cells up at column 7 (face `y = 5`, 0.5 m above the water). Every land
/// face holds pore fill `pore(x, z)`.
fn shore_world(day: cubarium_voxel::weather::DayClock, pore: impl Fn(u32, u32) -> f64) -> World {
    let mut world = World::empty(VoxelConfig { width: W, height: 10, depth: D, voxel_m: V, day, ..VoxelConfig::default() });
    for z in 0..D {
        for x in 0..i64::from(W) {
            let top = match x {
                0..=5 => 2,
                7 => 5,
                _ => 3,
            };
            for y in 1..=top {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    let cell = world.config().voxel_volume();
    for z in 0..D {
        for x in 0..=5 {
            world.apply(WorldCommand::AddWater { x, y: 3, z, volume_m3: cell });
        }
    }
    for z in 0..D {
        for x in 6..W {
            let y = face_y(x);
            let p = pore(x, z);
            set_pore(&mut world, i64::from(x), y, z, p);
        }
    }
    let v = world.view();
    for z in 0..D {
        assert!((v.standing_depth_m(2, 2, z) - V).abs() < 1e-9, "fixture: the pond stands");
        for x in 6..W {
            assert_eq!(v.highest_support_in(i64::from(x), z, 0, 9), Some(face_y(x)), "fixture: column {x}'s face");
            assert_eq!(v.standing_depth_m(i64::from(x), face_y(x), z), 0.0, "fixture: column {x} is dry-surfaced");
        }
    }
    world
}

fn face_y(x: u32) -> u32 {
    match x {
        0..=5 => 2,
        7 => 5,
        _ => 3,
    }
}

fn clock_off() -> cubarium_voxel::weather::DayClock {
    cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() }
}

fn noon() -> cubarium_voxel::weather::DayClock {
    cubarium_voxel::weather::DayClock {
        mode: cubarium_voxel::weather::DayMode::Cycle,
        start_hour: 12.0,
        ..Default::default()
    }
}

fn site(x: u32, z: u32) -> Site {
    Site { x, y: face_y(x), z }
}

/// A snail lineage that is ready to lay on its first tick (a one-tick hold, one egg of
/// twice `body_min`, so a hatchling has a reserve), lays on the shore under the knobs given, never refuses a crowded nest,
/// and incubates `incubation_ticks`.
fn config(incubation_ticks: f64, shore: impl Fn(&mut crate::Amphibious)) -> FaunaConfig {
    let mut config = FaunaConfig::default();
    let phys = &mut config.founders[SNAIL.index()];
    phys.egg_site = crate::EggSite::Shore;
    let core = &mut phys.core;
    core.reserve_cap = 1.0;
    core.reproduction = Reproduction {
        surplus_floor: 0.0,
        surplus_hold_s: DT,
        birth_interval_s: 1.0e6 * DT,
        clutch_size: 1,
        egg_organic: 2.0 * core.body_min,
        incubation_s: incubation_ticks * DT,
        nest_crowd_max: 1000,
        ..core.reproduction
    };
    assert_eq!(core.reproduction.mode, crate::BirthMode::Eggs, "an egg-layer");
    let am = phys.amphibious.as_mut().expect("the ripple-snail is amphibious");
    am.egg_water_max_m = 0.6; // columns 6 (0.25 m) and 7 (0.5 m) are near the pond, 8 (0.75 m) is not
    am.egg_climb_max_m = 0.3; // column 6 (0 m over the water) is low enough, the bluff (0.5 m) is not
    am.egg_moisture_min = 0.5;
    am.egg_dry_moisture = 0.2;
    am.egg_drown_s = 1.0e6 * DT;
    am.egg_dry_s = 1.0e6 * DT;
    shore(am);
    config
}

fn introduce(fauna: &mut Fauna, world: &World, x: i64, z: u32, stores: StartingStores) -> u64 {
    assert!(fauna.apply(world, Command::IntroduceFounder { x, z, founder: SNAIL, stores, heading_rad: 0.0 }));
    fauna.view().ledger.births - 1
}

fn still(fauna: &mut Fauna, id: u64) {
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 0.0, turn: 0.0, feed: 0.0 }]))));
}

fn steps(fauna: &mut Fauna, world: &World, flora: &mut Flora, n: usize) {
    for _ in 0..n {
        fauna.step(world, flora);
    }
}

fn litter_at(flora: &Flora, s: Site) -> f64 {
    flora.view().ground_at(s).map_or(0.0, |g| g.litter)
}

/// Both layers' books close, and what the plant layer gained is what the animal layer put
/// back (no eating here).
fn ledgers_close(fauna: &Fauna, flora: &Flora, flora_organic_before: f64) {
    let av = fauna.view();
    let fv = flora.view();
    for (name, got, want) in [
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
    ] {
        assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "the {name} ledger must close: {got} against {want}");
    }
    let gained = fv.organic() - flora_organic_before;
    let net = av.ledger.deposited_organic_out - av.ledger.eaten_organic_in;
    assert!((gained - net).abs() <= 1e-12 * gained.abs().max(1.0), "the plant layer gained {gained}, the animal layer put back {net}");
}

/// One snail held still at `(x, z)`, ready to lay; one tick; the clutch it laid, if any.
fn lays_at(world: &World, x: u32, z: u32) -> Option<crate::Clutch> {
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(1.0e6, |_| {}));
    let id = introduce(&mut fauna, world, i64::from(x), z, StartingStores::FULL);
    still(&mut fauna, id);
    steps(&mut fauna, world, &mut flora, 1);
    let v = fauna.view();
    let c: Vec<_> = v.clutches.iter().filter(|c| c.lineage == SNAIL).copied().collect();
    assert!(c.len() <= 1);
    if c.is_empty() {
        assert_eq!(v.animal(id).expect("alive").reproduction.refractory_ticks, 0, "a refused laying costs nothing");
    }
    c.first().copied()
}

// ------------------------------------------------------------------ the shore rule

/// **A ready ripple-snail lays only on a face that meets the shore rule**: out of standing
/// water, within `egg_water_max_m` of standing water, at most `egg_climb_max_m` above its
/// surface, at pore fill `egg_moisture_min` or more. One ready snail at a time: on the
/// damp bank by the pond (column 6) it lays there; in the pond, on the bluff above the
/// climb, on damp ground beyond the distance, and on dry bank beside the pond, it lays
/// nothing and pays nothing.
#[test]
fn a_ready_ripple_snail_lays_only_on_a_face_that_meets_the_shore_rule() {
    let world = shore_world(clock_off(), |_, z| if z == 4 { 0.1 } else { 0.8 });
    let c = config(1.0e6, |_| {});
    let am = c.founder(SNAIL).amphibious.unwrap();
    let pore = |x: i64, z: u32| world.view().pore_at(x, face_y(x as u32), z);
    assert!(pore(6, 1) >= am.egg_moisture_min && pore(6, 4) < am.egg_moisture_min, "fixture: damp and dry bank");

    let laid = lays_at(&world, 6, 1).expect("the damp bank by the pond takes the clutch");
    assert_eq!(laid.site, site(6, 1));
    assert!(lays_at(&world, 2, 1).is_none(), "not under standing water");
    assert!(lays_at(&world, 7, 1).is_none(), "not above the climb");
    assert!(lays_at(&world, 8, 1).is_none(), "not beyond the distance from water");
    assert!(lays_at(&world, 6, 4).is_none(), "not on ground drier than the minimum");
}

// ------------------------------------------------------------------ the egg's risks

/// Lay one clutch on `(x, z)` of `world` under `cfg` (the snail held still where it laid).
/// Returns the layers and the plant layer's organic before the run.
fn laid(world: &World, cfg: FaunaConfig, x: u32, z: u32) -> (Fauna, Flora, f64) {
    let mut flora = Flora::new(FloraConfig::default());
    let before = flora.view().organic();
    let mut fauna = Fauna::new(cfg);
    let id = introduce(&mut fauna, world, i64::from(x), z, StartingStores::FULL);
    still(&mut fauna, id);
    steps(&mut fauna, world, &mut flora, 1);
    assert_eq!(fauna.view().clutches.len(), 1, "fixture: one clutch laid");
    (fauna, flora, before)
}

/// **A submerged egg dies after its knob, to the litter of its face, and the books close.**
/// A clutch laid on the bank, then the bank flooded: under water for `egg_drown_s` less a
/// tick it still lies there; a couple of ticks later it is gone, booked drowned, its
/// organic matter in the face's litter.
#[test]
fn a_submerged_egg_dies_after_its_knob_and_its_organic_goes_to_litter() {
    let drown_ticks = 12.0;
    let mut world = shore_world(clock_off(), |_, _| 0.8);
    let cfg = config(1.0e6, |am| am.egg_drown_s = drown_ticks * DT);
    let (mut fauna, mut flora, before) = laid(&world, cfg, 6, 1);
    let egg = fauna.view().clutches[0];
    world.apply(WorldCommand::AddWater { x: 6, y: 4, z: 1, volume_m3: world.config().voxel_volume() });
    assert!(world.view().standing_depth_m(6, 3, 1) > 0.0, "fixture: the bank is flooded");
    let litter0 = litter_at(&flora, egg.site);
    steps(&mut fauna, &world, &mut flora, drown_ticks as usize - 1);
    assert_eq!(fauna.view().clutches.len(), 1, "under its knob it lives");
    steps(&mut fauna, &world, &mut flora, 3);
    let v = fauna.view();
    assert!(v.clutches.is_empty(), "past its knob it drowned");
    assert_eq!(v.ledger.eggs_drowned_by_founder[SNAIL.index()], u64::from(egg.count));
    assert_eq!(v.ledger.eggs_dried_by_founder[SNAIL.index()], 0);
    assert_eq!(v.ledger.eggs_hatched_by_founder[SNAIL.index()], 0);
    let litter = litter_at(&flora, egg.site) - litter0;
    assert!((litter - egg.organic).abs() <= 1e-12, "its organic matter is the face's litter: {litter} of {}", egg.organic);
    ledgers_close(&fauna, &flora, before);
}

/// **A dried egg dies after its knob.** Laid on ground that clears the laying floor but is
/// under the egg's own dry floor (both set here): for `egg_dry_s` less a tick it lies
/// there; then it is gone, booked dried, its organic matter in the face's litter.
#[test]
fn a_dried_egg_dies_after_its_knob_and_its_organic_goes_to_litter() {
    let dry_ticks = 15.0;
    let world = shore_world(clock_off(), |_, _| 0.4);
    let cfg = config(1.0e6, |am| {
        am.egg_moisture_min = 0.3;
        am.egg_dry_moisture = 0.5;
        am.egg_dry_s = dry_ticks * DT;
    });
    let (mut fauna, mut flora, before) = laid(&world, cfg, 6, 1);
    let egg = fauna.view().clutches[0];
    let litter0 = litter_at(&flora, egg.site);
    steps(&mut fauna, &world, &mut flora, dry_ticks as usize - 2);
    assert_eq!(fauna.view().clutches.len(), 1, "under its knob it lives");
    steps(&mut fauna, &world, &mut flora, 3);
    let v = fauna.view();
    assert!(v.clutches.is_empty(), "past its knob it dried");
    assert_eq!(v.ledger.eggs_dried_by_founder[SNAIL.index()], u64::from(egg.count));
    assert_eq!(v.ledger.eggs_drowned_by_founder[SNAIL.index()], 0);
    let litter = litter_at(&flora, egg.site) - litter0;
    assert!((litter - egg.organic).abs() <= 1e-12, "its organic matter is the face's litter: {litter} of {}", egg.organic);
    ledgers_close(&fauna, &flora, before);
}

/// **An egg that neither drowns nor dries hatches**, on land, where it lay. The two knobs
/// are shorter than the incubation, so the egg would have died had either clock run.
#[test]
fn an_egg_that_neither_drowns_nor_dries_hatches_on_land() {
    let incubation = 20.0;
    let world = shore_world(clock_off(), |_, _| 0.8);
    let cfg = config(incubation, |am| {
        am.egg_dry_moisture = 0.5;
        am.egg_drown_s = 5.0 * DT;
        am.egg_dry_s = 5.0 * DT;
    });
    let (mut fauna, mut flora, before) = laid(&world, cfg, 6, 1);
    let egg = fauna.view().clutches[0];
    let n0 = fauna.view().animals.len();
    steps(&mut fauna, &world, &mut flora, incubation as usize + 2);
    let v = fauna.view();
    assert!(v.clutches.is_empty(), "hatched");
    assert_eq!(v.ledger.eggs_hatched_by_founder[SNAIL.index()], u64::from(egg.count));
    assert_eq!(v.ledger.eggs_drowned_by_founder[SNAIL.index()] + v.ledger.eggs_dried_by_founder[SNAIL.index()], 0);
    assert_eq!(v.animals.len(), n0 + egg.count as usize);
    let young = v.animals.last().expect("the hatchling");
    assert_eq!(young.site, egg.site, "it starts on land, where the egg lay");
    assert_eq!(world.view().standing_depth_m(i64::from(young.site.x), young.site.y, young.site.z), 0.0);
    ledgers_close(&fauna, &flora, before);
}

// ------------------------------------------------------------------ the trip ashore

/// **A ready layer goes ashore to lay and back to the water.** By day (noon; the test's
/// snail keeps the day so it is awake — outside its roam hours), a ready snail
/// under its own heuristic in the pond (column 3), the damp bank beside it: it walks out,
/// lays on a face that meets the shore rule, and is back within its water margin by the
/// end, its drying clock never at its thirst line for the day.
#[test]
fn a_ready_layer_walks_ashore_to_lay_and_returns_to_the_water() {
    let world = shore_world(noon(), |_, _| 0.8);
    let mut cfg = config(1.0e6, |_| {});
    cfg.founders[SNAIL.index()].cruise_body_lengths_per_s = 2.0;
    // Awake at noon (the trip's mechanism, whatever hours the lineage keeps).
    cfg.founders[SNAIL.index()].active = crate::Activity::Diurnal;
    let am = cfg.founder(SNAIL).amphibious.unwrap();
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(cfg);
    let id = introduce(&mut fauna, &world, 3, 2, StartingStores::FULL);
    let teachers = fauna.config().teachers;
    assert!(fauna.set_controller(id, crate::controller::heuristic(SNAIL, teachers)));
    assert!(world.life_daylight() >= fauna.config().life.day_line, "fixture: day");
    let mut worst: f64 = 0.0;
    for _ in 0..1200 {
        fauna.step(&world, &mut flora);
        let b = fauna.view().animal(id).expect("alive");
        worst = worst.max(b.mobility.dryness);
    }
    let v = fauna.view();
    let c: Vec<_> = v.clutches.iter().filter(|c| c.lineage == SNAIL).collect();
    assert_eq!(c.len(), 1, "it laid once");
    assert!(c[0].site.x >= 6 && c[0].site.x != 7, "ashore, on the bank: {:?}", c[0].site);
    let b = v.animal(id).expect("alive");
    assert!(!b.mobility.ashore, "back in the water: {:?}", b.site);
    assert!(worst < am.lethal_fraction, "the trip never dries it to death: {worst}");
}

// ------------------------------------------------------------------ save and load

/// **The new state round-trips**: the shore knobs, a clutch's drowning and drying clocks,
/// and the drowned and dried egg counts.
#[test]
fn the_shore_state_round_trips_through_save_and_load() {
    let world = shore_world(clock_off(), |_, _| 0.8);
    let cfg = config(1.0e6, |am| {
        am.egg_water_max_m = 0.7;
        am.egg_climb_max_m = 0.35;
        am.egg_drown_s = 123.0;
        am.egg_dry_s = 45.0;
    });
    let (mut fauna, _flora, _) = laid(&world, cfg, 6, 1);
    fauna.clutches[0].wet_ticks = 3;
    fauna.clutches[0].dry_ticks = 4;
    fauna.ledger.eggs_drowned_by_founder[SNAIL.index()] = 5;
    fauna.ledger.eggs_dried_by_founder[SNAIL.index()] = 6;
    let back = crate::snapshot::decode(&crate::snapshot::encode(&fauna)).expect("its own bytes load");
    assert_eq!(back.config, fauna.config, "the shore knobs");
    assert_eq!(back.config.founder(SNAIL).egg_site, crate::EggSite::Shore);
    assert_eq!(back.clutches, fauna.clutches, "the clutch's clocks");
    assert_eq!(back.ledger, fauna.ledger, "the drowned and dried counts");
}

// ------------------------------------------------------------------ SNAIL-LAND-2

/// A controller that holds still and opts into the reflexes (the flee rule's gate): the
/// body moves only where a reflex walks it.
struct StillReflexive;

impl crate::Controller for StillReflexive {
    fn drive(&mut self, _observation: &[f64]) -> crate::Response {
        crate::Response::Bounded(Actions { forward: 0.0, turn: 0.0, feed: 0.0 })
    }
    fn reset(&mut self) {}
    fn flees(&self) -> bool {
        true
    }
}

/// **A layer saving for its full clutch is not trip-ready** (SNAIL-LAND-2): with a clutch
/// from surplus and a wait (`clutch_hold_max_s`), a body whose hold is met but whose
/// reserve pays for less than its full clutch keeps feeding (no trip) until the wait is
/// spent — the laying's own rule, so the trip never takes a snail ashore to hold still
/// for a wait it could spend eating. A full clutch, a spent wait, or no wait at all is
/// trip-ready. Read against the config the test sets.
#[test]
fn a_layer_saving_for_its_full_clutch_is_not_trip_ready_until_its_wait_is_spent() {
    let mut cfg = config(1.0e6, |_| {});
    let wait_ticks = 50u64;
    {
        let r = &mut cfg.founders[SNAIL.index()].core.reproduction;
        r.clutch_max = 3;
        r.surplus_floor = 0.0;
        r.clutch_hold_max_s = wait_ticks as f64 * DT;
    }
    let sc = cfg.founder(SNAIL).core;
    let rule = sc.reproduction;
    let cap = sc.reserve_of(sc.body_max);
    let full = rule.full_clutch(cap);
    assert!(full >= 2, "fixture: a full clutch of two or more eggs ({full})");
    let partial = 1.5 * rule.egg_organic;
    assert!(rule.clutch_for(partial) >= 1 && rule.clutch_for(partial) < full, "fixture: a partial clutch");
    let mut st = crate::ReproductionState { eligible_now: true, surplus_ticks: rule.hold_required(), ..Default::default() };
    assert!(rule.hold_met(&st), "fixture: the hold is met");
    let ready = |st: &crate::ReproductionState, reserve: f64, rule: &Reproduction| crate::step::trip_ready(rule, st, reserve, cap);
    assert!(!ready(&st, partial, &rule), "saving: no trip at the wait's start");
    st.clutch_wait_ticks = rule.clutch_hold_max_ticks() - 1;
    assert!(!ready(&st, partial, &rule), "saving: no trip a tick before the wait is spent");
    st.clutch_wait_ticks = rule.clutch_hold_max_ticks();
    assert!(ready(&st, partial, &rule), "the wait spent: it goes with what it has");
    st.clutch_wait_ticks = 0;
    assert!(ready(&st, f64::from(full) * rule.egg_organic, &rule), "a full clutch goes at once");
    let no_wait = Reproduction { clutch_hold_max_s: 0.0, ..rule };
    assert!(ready(&st, partial, &no_wait), "no wait: it goes at once");
    st.retry_ticks = 1;
    assert!(!ready(&st, f64::from(full) * rule.egg_organic, &rule), "a refused laying waits out its retry");
}

/// **The trip passes over a crowded nest** (SNAIL-LAND-2): with the nest-site brake on
/// (`nest_radius_m`, `nest_crowd_max` 0), a ready layer in the pond, the bank beside it
/// taken by another snail and its clutch, walks to a shore face outside that nest's
/// radius and lays there at the first try: no laying is refused.
#[test]
fn the_trip_passes_over_a_crowded_nest_for_a_free_shore_face() {
    let world = shore_world(noon(), |_, _| 0.8);
    let mut cfg = config(1.0e6, |_| {});
    cfg.founders[SNAIL.index()].cruise_body_lengths_per_s = 2.0;
    cfg.founders[SNAIL.index()].active = crate::Activity::Diurnal;
    {
        let r = &mut cfg.founders[SNAIL.index()].core.reproduction;
        r.nest_radius_m = 0.5;
        r.nest_crowd_max = 0;
    }
    let radius = cfg.founder(SNAIL).core.reproduction.nest_radius_m;
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(cfg);
    // The first snail lays on the bank at (6, 0) and stays there.
    let first = introduce(&mut fauna, &world, 6, 0, StartingStores::FULL);
    still(&mut fauna, first);
    steps(&mut fauna, &world, &mut flora, 1);
    assert_eq!(fauna.view().clutches.len(), 1, "fixture: the first clutch");
    let taken = fauna.view().clutches[0].site;
    let refused0 = fauna.view().ledger.nest_refusals_by_founder[SNAIL.index()];
    let id = introduce(&mut fauna, &world, 3, 0, StartingStores::FULL);
    assert!(fauna.set_controller(id, Box::new(StillReflexive)));
    for _ in 0..1200 {
        fauna.step(&world, &mut flora);
        if fauna.view().clutches.len() == 2 {
            break;
        }
    }
    let v = fauna.view();
    assert_eq!(v.clutches.len(), 2, "it laid");
    let c = v.clutches[1];
    let ring = f64::from(W) * V;
    let (dx, dz) = crate::pose::wrapped_delta(
        (f64::from(taken.x) + 0.5) * V,
        (f64::from(taken.z) + 0.5) * V,
        (f64::from(c.site.x) + 0.5) * V,
        (f64::from(c.site.z) + 0.5) * V,
        ring,
    );
    assert!((dx * dx + dz * dz).sqrt() > radius, "outside the taken nest: {:?} against {:?}", c.site, taken);
    let am = v.config.founder(SNAIL).amphibious.unwrap();
    assert!(crate::shore_egg_site(&world.view(), &am, c.site), "on a shore face: {:?}", c.site);
    assert_eq!(v.ledger.nest_refusals_by_founder[SNAIL.index()], refused0, "no laying refused");
}

/// **The trip looks as far as `egg_search_m`** (SNAIL-LAND-2), not `roam_search_m`: a
/// ready layer in mid-pond, its nearest shore face beyond its roam reach, lays ashore with
/// the bank within its egg search and does not when the bank is beyond it.
#[test]
fn the_trip_looks_as_far_as_its_egg_search() {
    let world = shore_world(noon(), |_, _| 0.8);
    let lays = |egg_search_m: f64| {
        let mut cfg = config(1.0e6, |am| {
            am.roam_search_m = 0.25;
            am.egg_search_m = egg_search_m;
        });
        cfg.founders[SNAIL.index()].cruise_body_lengths_per_s = 2.0;
        cfg.founders[SNAIL.index()].active = crate::Activity::Diurnal;
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(cfg);
        let id = introduce(&mut fauna, &world, 3, 2, StartingStores::FULL);
        assert!(fauna.set_controller(id, Box::new(StillReflexive)));
        steps(&mut fauna, &world, &mut flora, 1200);
        fauna.view().clutches.len()
    };
    // The bank's nearest shore face (column 6) is 0.75 m from the pond's middle (column 3).
    assert_eq!(lays(1.0), 1, "the bank within the egg search: it lays ashore");
    assert_eq!(lays(0.5), 0, "the bank beyond the egg search: no trip, no clutch");
}

/// **A grazer leaves a grazed face for a richer one** (SNAIL-LAND-2, the giving-up
/// density): film on every pond face, the snail's own face grazed to the refuge; a hungry
/// snail under its heuristic with `graze_give_up` above the refuge walks off and eats more;
/// with it 0 (main's rule) it stays on its grazed face, which still tastes of food, eating
/// only what its mouth reaches on the face ahead.
#[test]
fn a_hungry_grazer_leaves_a_grazed_face_for_a_richer_one() {
    let world = shore_world(clock_off(), |_, _| 0.8);
    let run = |give_up: f64| {
        let mut cfg = config(1.0e6, |am| am.graze_give_up = give_up);
        // No laying here.
        cfg.founders[SNAIL.index()].core.reproduction.surplus_hold_s = 1.0e9;
        let fc = FloraConfig { film_seed_per_m2: FloraConfig::default().film_capacity_per_m2, ..FloraConfig::default() };
        let mut flora = Flora::new(fc.clone());
        assert!(flora.seed_film(&world) > 0, "fixture: film on the pond");
        let k = fc.film_capacity_per_m2 * V * V;
        let am = cfg.founder(SNAIL).amphibious.unwrap();
        assert!(fc.film_graze_refuge < am.graze_give_up || give_up == 0.0, "fixture: the give-up above the refuge");
        assert!(fc.film_seed_per_m2 * V * V >= am.graze_worth * k, "fixture: the other faces are worth the walk");
        let home = site(3, 2);
        let _ = flora.take_film(home, 1.0);
        let grazed = flora.view().film_at(home);
        assert!(grazed <= fc.film_graze_refuge * k + 1e-12 && grazed > 0.0, "fixture: its face at the refuge");
        let mut fauna = Fauna::new(cfg);
        let id = introduce(&mut fauna, &world, 3, 2, StartingStores { body: 1.0, reserve: 0.3 });
        let teachers = fauna.config().teachers;
        assert!(fauna.set_controller(id, crate::controller::heuristic(SNAIL, teachers)));
        steps(&mut fauna, &world, &mut flora, 300);
        let b = fauna.view().animal(id).expect("alive").clone();
        (b.site, fauna.view().ledger.eaten_by_food[crate::Food::Film.index()])
    };
    let (at, ate_on) = run(0.3);
    assert_ne!(at, site(3, 2), "it left the grazed face");
    let (at, ate_off) = run(0.0);
    assert_eq!(at, site(3, 2), "with the give-up off it stays on the grazed face");
    assert!(ate_on > ate_off, "leaving, it ate more: {ate_on} against {ate_off} (its mouth reaches only the face ahead)");
}

/// **Where a grazer goes** (SNAIL-LAND-2, [`crate::step::graze_target`]): the face within
/// `graze_search_m` with the most film per snail on it above its own, per metre, among
/// faces worth the walk (`graze_worth` of a site's capacity per snail, itself counted); a
/// face out of standing water only in its roam hours; none worth it, no walk.
#[test]
fn a_grazer_goes_to_the_richest_face_worth_it_and_onto_land_only_in_its_hours() {
    let world = shore_world(clock_off(), |_, _| 0.8);
    let cfg = config(1.0e6, |am| {
        am.graze_search_m = 0.6;
        am.graze_worth = 0.5;
    });
    let am = cfg.founder(SNAIL).amphibious.unwrap();
    let mut fauna = Fauna::new(cfg);
    let id = introduce(&mut fauna, &world, 5, 2, StartingStores::FULL);
    let a = fauna.view().animal(id).expect("alive").clone();
    let k = 1.0;
    let (bank, pond) = (site(6, 2), site(3, 2));
    let film = |s: Site| if s == bank { 0.9 * k } else if s == pond { 0.6 * k } else { 0.0 };
    let view = world.view();
    let to = |roam: bool, others: &[Site]| {
        crate::step::graze_target(&view, &am, &a, roam, k, &film, others).map(|(dx, dz)| {
            let x = ((a.pose.x + dx) / V).floor() as i64;
            let z = ((a.pose.z + dz) / V).floor() as u32;
            (x.rem_euclid(i64::from(W)) as u32, z)
        })
    };
    assert_eq!(to(true, &[]), Some((6, 2)), "in its hours: the richer bank, nearer");
    assert_eq!(to(false, &[]), Some((3, 2)), "out of its hours: only the pond");
    assert_eq!(to(true, &[bank]), Some((3, 2)), "a snail on the bank halves it below the worth: the pond");
    let poor = |s: Site| if s == pond { 0.4 * k } else { 0.0 };
    assert_eq!(crate::step::graze_target(&view, &am, &a, false, k, &poor, &[]), None, "nothing worth the walk");
}
