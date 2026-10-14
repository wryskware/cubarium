//! **SP-OSC's laws** (2026-10-14; written before the fix, from the seed-8 probe of the
//! check-aim loop: a hungry seedporter on the ground held still by its catch's wait for a
//! bellwing perched over its own back, while its tree check, under whose label the wait ran,
//! read the stillness as a stalled walk and traded its aim for the next host every
//! `teachers.press_replan_s`):
//!
//! - **a check aim is kept** until the body arrives, gives the walk up, or finds food: it does
//!   not trade aims while nothing changes;
//! - **the catch waits only for a prey its strike can meet**: a bellwing perched in the strike
//!   band but nearer the body's centre than its jaw's capsule reaches back (over its back,
//!   under its belly) is no catch to wait for; no turn in place brings it into the capsule.
//!
//! Fixtures are hand-built at the 0.25 m reference voxel as in `sp_meal_tests`: soil at
//! `y = 1..=2`, the day clock off. Every bound comes from the config or is a test horizon; no
//! shipped number, ratio or world hash is asserted.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species as Plant};

use crate::{Command, Fauna, FaunaConfig, Founder, HuntMode, MobilityMode, Pose, StartingStores};

const V: f64 = 0.25;
const BW: Founder = Founder::Bellwing;
const SP: Founder = Founder::Seedporter;
const EAST: f64 = std::f64::consts::FRAC_PI_2;
/// The top of the soil, metres.
const SURFACE_M: f64 = 3.0 * V;

fn world() -> World {
    let mut w = World::empty(VoxelConfig {
        width: 48,
        height: 64,
        depth: 16,
        voxel_m: V,
        seed: 7,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    let c = w.config().clone();
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            for y in 1..=2 {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

/// No nuts on any vaulttree: a crowned one is a check host for its larvae alone.
fn flora_config() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.vaulttree.fruit_per_s = 0.0;
    c.vaulttree.fruit_cap = 0.0;
    c
}

/// Every lineage awake, no ground response, no troop pull; the seedporter catches bellwings,
/// any perched one under a ceiling well above the scene being prey at rest.
fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for p in &mut c.founders {
        p.active = crate::Activity::Always;
        p.vigilance = crate::Vigilance::default();
        p.herding = crate::Herding::default();
    }
    let sp = &mut c.founders[SP.index()];
    sp.catch_bellwings = true;
    sp.catch_ceiling_m = 10.0;
    sp.catch_gain_min = 0.0;
    c
}

fn animal(fauna: &Fauna, id: u64) -> crate::Animal {
    *fauna.view().animal(id).expect("alive")
}

fn animal_mut(fauna: &mut Fauna, id: u64) -> &mut crate::Animal {
    fauna.animals.iter_mut().find(|a| a.id == id).expect("alive")
}

/// A hungry seedporter on its own heuristic at column `(x, z)`, facing east.
fn seedporter(fauna: &mut Fauna, world: &World, x: i64, z: u32) -> u64 {
    assert!(fauna.apply(world, Command::IntroduceFounder { x, z, founder: SP, stores: StartingStores { body: 1.0, reserve: 0.3 }, heading_rad: EAST }));
    let id = fauna.animals.last().expect("introduced").id;
    let teachers = fauna.config.teachers;
    fauna.controllers.set(id, crate::heuristic(SP, teachers));
    animal_mut(fauna, id).eating = true;
    id
}

/// A bellwing introduced aloft, its controller still.
fn bellwing(fauna: &mut Fauna, world: &World, x: f64, z: f64, y_m: f64) -> u64 {
    assert!(fauna.apply(world, Command::IntroduceAloft { pose: Pose { x, z, heading_rad: -EAST, y_m }, founder: BW, stores: StartingStores::FULL }));
    let id = fauna.animals.last().expect("introduced").id;
    assert!(fauna.set_controller(id, Box::new(crate::Scripted::new(vec![crate::Actions { forward: 0.0, turn: 0.0, feed: 0.0 }]))));
    id
}

/// The height over the seedporter's support at which a perched bellwing's body sits inside
/// the seedporter's strike band (half the band up).
fn in_band_lift(fauna: &Fauna, sp: u64) -> f64 {
    let a = animal(fauna, sp);
    let g = crate::body_of(&fauna.config, &a).expect("a body");
    let h = fauna.config.founder(SP).hunter.expect("the seedporter's catch");
    0.5 * h.strike_height_m(&g)
}

/// Keep bellwing `bw` perched over seedporter `sp`'s centre at `lift` above its support, while
/// the seedporter stands on the ground: inside its strike band, nearer its centre than its
/// jaw's capsule reaches back.
fn hold_over_its_back(fauna: &mut Fauna, sp: u64, bw: u64, lift: f64) {
    let p = animal(fauna, sp);
    if p.mobility.mode != MobilityMode::Ground {
        return;
    }
    let b = animal_mut(fauna, bw);
    b.pose.x = p.pose.x;
    b.pose.z = p.pose.z;
    b.pose.y_m = SURFACE_M + lift;
    b.site = p.site;
    b.mobility.mode = MobilityMode::Perched;
    b.mobility.falling = false;
}

/// Fixture checks shared by both laws: the bellwing is the seedporter's prey, and nearer its
/// centre than its strike capsule reaches back from the jaw.
fn assert_over_its_back(fauna: &Fauna, sp: u64, bw: u64) {
    let a = animal(fauna, sp);
    let b = animal(fauna, bw);
    assert!(crate::is_prey_of(&fauna.config, &a, &b), "fixture: the bellwing is its prey");
    let g = crate::body_of(&fauna.config, &a).expect("a body");
    let pg = crate::body_of(&fauna.config, &b).expect("a body");
    let h = fauna.config.founder(SP).hunter.expect("the seedporter's catch");
    let back = h.jaw_forward_length_fraction * g.length_m - g.footprint_radius() - pg.footprint_radius();
    assert!(back > 0.0, "fixture: the capsule starts ahead of the centre by more than both radii ({back} m)");
}

/// **A seedporter with a check aim keeps it until it arrives, gives the walk up, or finds
/// food.** Hungry on the ground with no food in sight and three crowned vaulttrees in the
/// check's reach (larva hosts, holding none), and a bellwing perched over its back that its
/// strike cannot meet: from its first aim on, over ten `teachers.press_replan_s`, no check walk
/// replaces an aim not reached. (The probe: the catch's wait held it still under the check's
/// label, the stillness read as a pressed walk, and every 3 s the walk's host was shunned and
/// the next taken: 20 switches a minute for ten minutes.)
#[test]
fn a_seedporter_with_a_check_aim_keeps_it_until_it_arrives_gives_up_or_finds_food() {
    let world = world();
    let mut flora = Flora::new(flora_config());
    let wood = flora.config().species(Plant::Vaulttree).wood_max;
    for (x, z) in [(26, 3), (26, 12), (34, 8)] {
        assert!(flora.apply(&world, FloraCommand::Seed { x, z, species: Plant::Vaulttree, wood }), "fixture: a vaulttree at ({x}, {z})");
    }
    let mut fauna = Fauna::new(config());
    fauna.set_births_enabled(false);
    let sp = seedporter(&mut fauna, &world, 12, 8);
    let lift = in_band_lift(&fauna, sp);
    let p = animal(&fauna, sp).pose;
    let bw = bellwing(&mut fauna, &world, p.x, p.z, SURFACE_M + lift);
    hold_over_its_back(&mut fauna, sp, bw, lift);
    assert_over_its_back(&fauna, sp, bw);
    {
        let phys = *fauna.config.founder(SP);
        let fv = flora.view();
        let hosts = fv
            .stands
            .iter()
            .filter(|s| crate::treecheck::may_hold_hidden_food(&fauna, &fv, &phys, crate::treecheck::Host::Stand(s)))
            .count();
        assert_eq!(hosts, 3, "fixture: three check hosts");
    }
    let replan = fauna.config.teachers.press_replan_s;
    assert!(replan > 0.0 && replan.is_finite(), "fixture: a pressed walk re-plans");
    let horizon = cubarium_voxel::ticks_at_least_one(10.0 * replan) as usize;
    let mut aimed = None;
    let mut switches0 = 0;
    for k in 0..horizon * 4 {
        hold_over_its_back(&mut fauna, sp, bw, lift);
        fauna.step(&world, &mut flora);
        if aimed.is_none() && crate::treecheck::aim_of(&fauna, sp).is_some() {
            aimed = Some(k);
            switches0 = fauna.check_tally().switches;
        }
        if aimed.is_some_and(|t| k >= t + horizon) {
            break;
        }
    }
    let t = aimed.expect("fixture: it sets out on a check walk");
    let tally = fauna.check_tally();
    assert_eq!(
        tally.switches - switches0,
        0,
        "no check walk replaced an aim not reached in the {horizon} ticks from its first aim (tick {t}): {tally:?}"
    );
}

/// **The catch waits only for a prey its strike can meet.** A hungry seedporter on the ground
/// with a bellwing perched over its back, inside its strike band in height but nearer its
/// centre than the strike capsule reaches back from its jaw, is not held in the catch's wait:
/// no turn in place brings that bellwing into the capsule, and the probe's wait lasted ten
/// minutes.
#[test]
fn a_seedporter_does_not_wait_for_a_bellwing_over_its_back_that_its_strike_cannot_meet() {
    let world = world();
    let mut flora = Flora::new(flora_config());
    let mut c = config();
    c.founders[SP.index()].glider.as_mut().expect("a glider").check_search_m = 0.0;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let sp = seedporter(&mut fauna, &world, 12, 8);
    let lift = in_band_lift(&fauna, sp);
    let p = animal(&fauna, sp).pose;
    let bw = bellwing(&mut fauna, &world, p.x, p.z, SURFACE_M + lift);
    hold_over_its_back(&mut fauna, sp, bw, lift);
    assert_over_its_back(&fauna, sp, bw);
    let replan = fauna.config.teachers.press_replan_s;
    let horizon = cubarium_voxel::ticks_at_least_one(replan.max(1.0)) as usize;
    let mut waited = 0;
    for _ in 0..horizon {
        hold_over_its_back(&mut fauna, sp, bw, lift);
        fauna.step(&world, &mut flora);
        waited += usize::from(animal(&fauna, sp).hunt.mode == HuntMode::Wait);
    }
    assert_eq!(waited, 0, "it waited {waited} of {horizon} ticks for a bellwing over its back");
}
