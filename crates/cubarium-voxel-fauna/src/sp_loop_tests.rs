//! Start geometry must agree with the jaw sweep terrain permits at contact.
use super::predation_fixture::*;
use crate::{Fauna, FaunaConfig, Founder, MobilityMode, Strike, StrikePhase};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig};

const SP: Founder = Founder::Seedporter;
const BW: Founder = Founder::Bellwing;

fn catch_scene(blocked: bool) -> (World, Fauna, u64, u64) {
    let mut world = plain_of_height(
        DayClock {
            start_hour: 12.0,
            ..DayClock::default()
        },
        107,
        16,
    );
    let mut c = FaunaConfig::default();
    let sp = &mut c.founders[SP.index()];
    sp.catch_bellwings = true;
    sp.catch_gain_min = 0.0;
    sp.catch_ceiling_m = 1.0;
    let h = sp.hunter.as_mut().expect("seedporter catch machinery");
    h.prey = [false; Founder::COUNT];
    h.prey[BW.index()] = true;
    h.prey_weight = [1.0; Founder::COUNT];
    h.prey_juveniles_only = [false; Founder::COUNT];
    h.prey_ashore_only = [false; Founder::COUNT];
    h.prey_max_ratio = 10.0;
    // Explicit fixture geometry: ample reach across the wall, a short complete lunge,
    // and a certain lethal contact. None of these are assertions on tuned defaults.
    h.strike_reach_length_fraction = 4.0;
    h.jaw_forward_length_fraction = 0.5;
    h.jaw_band_height_fraction = 2.0;
    h.vertical_reach_m = 0.0;
    h.lunge_s = 2.0 * crate::DT;
    h.strike_cost_s = 0.0;
    h.strike_power = 20.0;
    c.founders[BW.index()].agility = 0.0;
    c.founders[BW.index()].armour = 0.0;
    c.founders[BW.index()].senses.live = false;
    c.founders[BW.index()].vigilance.mode = crate::VigilMode::None;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let hunter = introduce(&mut fauna, &world, SP, HUNTING);
    let prey = introduce(&mut fauna, &world, BW, crate::StartingStores::FULL);
    let phys = fauna.config.founder(SP);
    let geometry = phys.body_at(animal(&fauna, hunter).body);
    let h = phys.active_hunter().unwrap();
    let wall_x = 16;
    let x = wall_x as f64 * VOXEL_M - geometry.footprint_radius() - 0.01;
    place(&mut fauna, hunter, x, LANE_Z, EAST);
    animal_mut(&mut fauna, hunter).pose.y_m = SURFACE_M;
    let ahead =
        (h.jaw_forward_length_fraction + 0.75 * h.strike_reach_length_fraction) * geometry.length_m;
    place(&mut fauna, prey, x + ahead, LANE_Z, WEST);
    animal_mut(&mut fauna, prey).pose.y_m = SURFACE_M;
    animal_mut(&mut fauna, prey).mobility.mode = MobilityMode::Perched;
    if blocked {
        // Across the whole lane and taller than a step: no side slip or climb can
        // deliver the body's nominal lunge. The bellwing is on the far side.
        for z in 0..world.view().config.depth {
            for y in SURFACE_Y + 1..world.view().config.height {
                world.apply(WorldCommand::SetMaterial {
                    x: wall_x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    (world, fauna, hunter, prey)
}

fn resolve_lunge(world: &World, fauna: &mut Fauna, hunter: u64, prey: u64) -> Vec<crate::Hit> {
    let i = fauna.animals.iter().position(|a| a.id == hunter).unwrap();
    let phys = *fauna.config.founder(SP);
    let h = phys.active_hunter().unwrap();
    let a = animal(fauna, hunter);
    animal_mut(fauna, hunter).strike = Some(Strike {
        phase: StrikePhase::Lunge,
        ticks_left: crate::Hunter::ticks(h.lunge_s),
        prey,
        from_x: a.pose.x,
        from_z: a.pose.z,
        lure: false,
    });
    let mut hits = Vec::new();
    for tick in 0..crate::Hunter::ticks(h.lunge_s) {
        super::strike_tick(
            fauna,
            i,
            &world.view(),
            &phys,
            h,
            107,
            u64::from(tick),
            &mut hits,
        );
    }
    assert_eq!(
        phase(fauna, hunter),
        Some(StrikePhase::Recovery),
        "the complete lunge resolved"
    );
    hits
}

#[test]
fn a_blocked_missed_seedporter_strike_cannot_immediately_select_the_same_bellwing() {
    let (world, mut fauna, hunter, prey) = catch_scene(true);
    let before = animal(&fauna, hunter).pose.x;
    let hits = resolve_lunge(&world, &mut fauna, hunter, prey);
    assert!(
        hits.is_empty(),
        "a blocked jaw cannot hit the stationary bellwing across the wall"
    );
    let i = fauna.animals.iter().position(|a| a.id == hunter).unwrap();
    let phys = fauna.config.founder(SP);
    let geometry = phys.body_at(animal(&fauna, hunter).body);
    let h = phys.active_hunter().unwrap();
    assert!(
        animal(&fauna, hunter).pose.x - before < h.strike_reach_length_fraction * geometry.length_m,
        "fixture: terrain shortened the lunge"
    );
    // Simulate recovery finishing without giving the prey a chance to move: recovery
    // itself must not make the unreachable target eligible for another windup.
    animal_mut(&mut fauna, hunter).strike = None;
    assert_eq!(
        super::prey_in_capsule(&fauna, i, &world.view(), &geometry, &h),
        None,
        "the same stationary unreachable prey cannot start another strike window"
    );
}

#[test]
fn a_reachable_perched_bellwing_is_still_taken_by_a_seedporter() {
    let (world, mut fauna, hunter, prey) = catch_scene(false);
    let i = fauna.animals.iter().position(|a| a.id == hunter).unwrap();
    let phys = fauna.config.founder(SP);
    let geometry = phys.body_at(animal(&fauna, hunter).body);
    let h = phys.active_hunter().unwrap();
    assert_eq!(
        super::prey_in_capsule(&fauna, i, &world.view(), &geometry, &h),
        Some(prey)
    );
    let hits = resolve_lunge(&world, &mut fauna, hunter, prey);
    assert!(
        hits.iter().any(|hit| hit.prey == prey),
        "a clear lunge still contacts the perched bellwing"
    );
    let mut flora = Flora::new(FloraConfig::default());
    for hit in hits {
        super::apply_hit(
            &mut fauna,
            &mut flora,
            hit.prey,
            hit.damage,
            hit.by,
            Some(&world.view()),
        );
    }
    assert!(
        fauna.view().animal(prey).is_none(),
        "the reachable bellwing was taken"
    );
}
