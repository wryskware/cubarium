//! **LJ-HUNT: the lanternjaw strikes from ambush.** A child of `step`, so a case can reach
//! the controller table and the layer's own fields.
//!
//! Every case checks a mechanism on hand-built inputs **against the config it sets**: no
//! default, ratio or population is asserted.
//!
//! - A patroller waiting on its patch strikes a size-eligible prey that comes into its strike
//!   capsule though its patrol never engaged it (it sees nothing: `sight_m` 0); with
//!   [`HuntConfig::wait_strikes`] off it lets the same prey go.
//! - A bellwing hovering at a waiting lanternjaw's lure point — where a lured one inspects
//!   it — is struck.
//! - The lure entrances ([`crate::LightConfig::lure_masks_loom`]): a lanternjaw closing on a
//!   bellwing looms to it; to a bellwing drawn to its lure it does not, nor does a strike begun
//!   on a lured bellwing; and the moving lure of that lunge stays its beacon instead of
//!   repelling it; with the switch off both loom and the lure repels.
//! - The lunge sweeps its band ([`HuntConfig::lunge_sweeps_band`]): a lanternjaw that lunged
//!   down off a ledge meets a bellwing hovering in the band over the ledge it began on, over
//!   the band where it stands now; with the switch off, or no lunge, it does not.
//! - Ledgers conserve through each world case.

use super::predation_fixture::*;
use crate::{Activity, Command, Fauna, FaunaConfig, Founder, HuntConfig, HuntMemory, HuntMode, MobilityMode, Pose, StartingStores};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig};

const LJ: Founder = Founder::Lanternjaw;
const SHREDDER: Founder = Founder::Blind;

/// The hunting table, every lineage awake, the patrol on with `wait_strikes`, blind (its
/// patrol sees no prey, so it never engages one) and patient (the wait does not run out).
fn ambush_config(wait_strikes: bool) -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.hunt = HuntConfig {
        patrol: true,
        wait_strikes,
        sight_m: 0.0,
        patch_wait_s: 1.0e6,
        strike_chosen_only: false,
        ..HuntConfig::default()
    };
    c
}

/// A hungry lanternjaw on its own heuristic at the mark facing `+x`, already waiting on its
/// patch there.
fn waiting_lanternjaw(fauna: &mut Fauna, world: &World) -> u64 {
    let lj = introduce(fauna, world, LJ, HUNTING);
    place(fauna, lj, STRIKER_X, LANE_Z, EAST);
    let teachers = fauna.config.teachers;
    fauna.controllers.set(lj, crate::controller::heuristic(LJ, teachers));
    let a = animal_mut(fauna, lj);
    a.hunt = HuntMemory { mode: HuntMode::Wait, patch: (STRIKER_X, LANE_Z), ..HuntMemory::default() };
    a.eating = true;
    lj
}

/// Strikes begun on `prey` lineage, and the patrol's engaged samples, over `ticks`.
fn run(fauna: &mut Fauna, world: &World, flora: &mut Flora, prey: Founder, ticks: u32) -> (u64, u64) {
    for _ in 0..ticks {
        fauna.step_with(world, flora, 1);
    }
    assert_closes(fauna, "after the ambush case");
    (fauna.ledger.strikes_on_by_founder[prey.index()], fauna.ledger.hunt_engaged_samples)
}

/// A still littershredder standing in the capsule of a lanternjaw waiting on its patch, which
/// never sees it: struck with `wait_strikes` on, let go with it off.
#[test]
fn a_waiting_lanternjaw_strikes_an_unseen_prey_in_its_capsule_only_with_its_jaw_ready() {
    let case = |wait_strikes: bool| {
        let world = clock_off(61);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(ambush_config(wait_strikes));
        let shredder = introduce(&mut fauna, &world, SHREDDER, StartingStores::FULL);
        place(&mut fauna, shredder, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
        drive(&mut fauna, shredder, 0.0, false);
        let lj = waiting_lanternjaw(&mut fauna, &world);
        assert!(
            crate::is_prey_of(&fauna.config, &animal(&fauna, lj), &animal(&fauna, shredder)),
            "the fixture: the shredder is in the lanternjaw's list and size window"
        );
        run(&mut fauna, &world, &mut flora, SHREDDER, 60)
    };
    let (struck, engaged) = case(true);
    assert_eq!(engaged, 0, "the fixture: the blind patrol never engages it");
    assert!(struck > 0, "with its jaw ready a waiting lanternjaw strikes the prey in its capsule");
    let (struck, engaged) = case(false);
    assert_eq!(engaged, 0, "the fixture: the blind patrol never engages it");
    assert_eq!(struck, 0, "with the switch off the waiting lanternjaw lets it go");
}

/// A bellwing hovering at a waiting lanternjaw's lure point (the capsule's middle, the lure's
/// height: where a lured bellwing inspects the lure) is struck.
#[test]
fn a_bellwing_at_a_waiting_lanternjaws_lure_is_struck() {
    let world = clock_off(62);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(ambush_config(true));
    let lj = waiting_lanternjaw(&mut fauna, &world);
    animal_mut(&mut fauna, lj).pose.y_m = SURFACE_M;
    let (x, z, y) = crate::light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).expect("a lanternjaw has a lure");
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft { pose: Pose { x, z, heading_rad: WEST, y_m: y }, founder: Founder::Bellwing, stores: StartingStores::FULL },
    ));
    let bw = fauna.animals.last().expect("just introduced").id;
    assert_eq!(animal(&fauna, bw).mobility.mode, MobilityMode::Fly, "the fixture has it hovering at the lure");
    let (struck, engaged) = run(&mut fauna, &world, &mut flora, Founder::Bellwing, 60);
    assert_eq!(engaged, 0, "the fixture: the blind patrol never engages it");
    assert!(struck > 0, "a bellwing at the lure of a waiting lanternjaw is struck");
}

/// The loom a bellwing hovering just ahead of a lanternjaw sees, with the lanternjaw closing
/// on it (moving toward it, its lure lit): `lured` puts the bellwing on an open inspection of
/// a lure, `striking` puts the lanternjaw in a lunge begun on it while lured.
fn seen(masks: bool, lured: bool, striking: bool) -> crate::perception::LightReading {
    let mut c = ambush_config(true);
    c.light.lure_masks_loom = masks;
    let world = clock_off(64);
    let mut fauna = Fauna::new(c);
    let lj = introduce(&mut fauna, &world, LJ, HUNTING);
    place(&mut fauna, lj, STRIKER_X, LANE_Z, EAST);
    animal_mut(&mut fauna, lj).pose.y_m = SURFACE_M;
    let (x, z, y) = crate::light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).expect("a lanternjaw has a lure");
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft { pose: Pose { x, z, heading_rad: WEST, y_m: y }, founder: Founder::Bellwing, stores: StartingStores::FULL },
    ));
    let bw = fauna.animals.last().expect("just introduced").id;
    {
        let a = animal_mut(&mut fauna, lj);
        a.founder_state.lure = 1.0;
        a.speed_m_s = 1.0;
        a.still_ticks = 0;
        if striking {
            a.strike = Some(crate::Strike {
                phase: crate::StrikePhase::Lunge,
                ticks_left: 3,
                prey: bw,
                from_x: STRIKER_X,
                from_z: LANE_Z,
                lure: true,
            });
        }
    }
    if lured {
        let lure = crate::BeaconKind::Lure.index() as u8;
        fauna.percepts.entry(bw).or_default().visit = crate::perception::Visit { active: true, kind: lure, x, z, since: 0 };
    }
    let a = animal(&fauna, bw);
    crate::light::light_reading(&fauna, &a, &world.view(), 1.0).expect("a bellwing has a light sense")
}

/// A lanternjaw closing on a bellwing looms to it; drawn to its lure, the bellwing sees no loom
/// in it, nor in a strike begun on it while lured; with the switch off, both loom.
#[test]
fn a_lured_bellwing_sees_no_loom_in_the_lanternjaw_behind_its_lure() {
    let loom = |m, l, s| seen(m, l, s).looming;
    assert!(loom(true, false, false) > 0.0, "the fixture: the closing lanternjaw looms to an unlured bellwing");
    assert_eq!(loom(true, true, false), 0.0, "drawn to the lure, it sees no loom in the lit lanternjaw");
    assert_eq!(loom(true, false, true), 0.0, "a strike begun on it while lured does not loom to it");
    assert!(loom(false, true, false) > 0.0, "switch off: the lit lanternjaw looms to a lured bellwing");
    assert!(loom(false, false, true) > 0.0, "switch off: the strike looms");
}

/// The lure of a lanternjaw lunging at a bellwing it lured moves: to that bellwing it stays the
/// still light it follows, not a moving light that repels; with the switch off it repels.
#[test]
fn a_lunging_lure_stays_the_lured_bellwings_beacon() {
    let lure = crate::BeaconKind::Lure.index() as u8;
    let on = seen(true, false, true);
    assert_eq!(on.repel_level, 0.0, "the lunging lure does not repel the bellwing it lured");
    assert!(on.beacon && on.beacon_kind == lure, "it is still the bellwing's beacon");
    let off = seen(false, false, true);
    assert!(off.repel_level > 0.0, "switch off: the moving lure repels");
    assert!(!(off.beacon && off.beacon_kind == lure), "switch off: it is no beacon to the bellwing");
}

/// A lanternjaw that lunged off a one-voxel ledge (it began on the ledge and stands below it
/// now) meets a bellwing hovering in the band over the ledge and over the band where it stands
/// — with the switch on and only during its lunge.
#[test]
fn a_lunge_off_a_ledge_sweeps_its_band_from_the_face_it_began_on() {
    let case = |sweeps: bool, lunging: bool| -> (bool, bool) {
        let mut world = clock_off(66);
        // The ledge: one more row of soil on every column west of the mark.
        let ledge_x = (STRIKER_X / VOXEL_M).floor() as i64;
        for z in 0..world.view().config.depth {
            for x in 0..ledge_x {
                world.apply(cubarium_voxel::Command::SetMaterial { x, y: SURFACE_Y + 1, z, material: cubarium_voxel::Material::Soil });
            }
        }
        let mut c = ambush_config(true);
        c.hunt.lunge_sweeps_band = sweeps;
        let mut fauna = Fauna::new(c);
        let lj = introduce(&mut fauna, &world, LJ, HUNTING);
        place(&mut fauna, lj, STRIKER_X + 0.5 * VOXEL_M, LANE_Z, EAST);
        let (h, geometry) = {
            let phys = fauna.config.founder(LJ);
            (phys.active_hunter().expect("a hunter"), phys.body_at(animal(&fauna, lj).body))
        };
        let band = h.strike_height_m(&geometry);
        // Over the band from the floor it stands on, inside the band from the ledge's top.
        let y = SURFACE_M + band + 0.5 * VOXEL_M;
        assert!(fauna.apply(
            &world,
            Command::IntroduceAloft { pose: Pose { x: STRIKER_X + IN_CAPSULE_M, z: LANE_Z, heading_rad: WEST, y_m: y }, founder: Founder::Bellwing, stores: StartingStores::FULL },
        ));
        let bw = fauna.animals.last().expect("just introduced").id;
        if lunging {
            animal_mut(&mut fauna, lj).strike = Some(crate::Strike {
                phase: crate::StrikePhase::Lunge,
                ticks_left: 1,
                prey: bw,
                // It began a voxel and a half back, on the ledge.
                from_x: STRIKER_X - VOXEL_M,
                from_z: LANE_Z,
                lure: true,
            });
        }
        let (a, p) = (animal(&fauna, lj), animal(&fauna, bw));
        let pg = crate::body_of(&fauna.config, &p).expect("a body");
        let view = world.view();
        (
            super::in_strike_band(&view, &a, &geometry, &h, &p, &pg),
            super::lunge_start_band(&fauna.config, &view, &a, &geometry, &h, &p, &pg),
        )
    };
    let (here, from_ledge) = case(true, true);
    assert!(!here, "the fixture: the bellwing hovers over the band where the striker stands");
    assert!(from_ledge, "the lunge begun on the ledge sweeps its band from the ledge's top");
    assert!(!case(false, true).1, "switch off: the band where the lunge ends only");
    assert!(!case(true, false).1, "no lunge: nothing swept");
}
