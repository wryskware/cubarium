//! **GAIT, written before the rule** (Wrysk 2026-10-06): the Veilhound's own cruise and its
//! rush, the seedporter's ground dash, and every other lineage held to its cruise. A child of
//! `step` beside the predation files, sharing their fixture.
//!
//! Every case reads the sprint and the cruise from its own config and checks the mechanism
//! against them; no default speed, ratio or population is asserted.

use super::predation_fixture::*;
use crate::{Actions, Activity, Fauna, FaunaConfig, Founder, FounderState, StartingStores, Sprint};
use cubarium_voxel::World;
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel_flora::{Flora, FloraConfig};

const CH: Founder = Founder::Chorister;
const LJ: Founder = Founder::Lanternjaw;
const SP: Founder = Founder::Seedporter;

fn world(seed: u64) -> World {
    plain(DayClock { mode: DayMode::Off, ..DayClock::default() }, seed)
}

fn awake(mut c: FaunaConfig) -> FaunaConfig {
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c
}

/// Hold `forward` on `id` for one tick with no controller: its age is set off the sampling
/// cadence, so the held actions are the body's for the tick. Returns its ground speed.
fn hold_tick(fauna: &mut Fauna, world: &World, flora: &mut Flora, id: u64, forward: f64) -> f64 {
    let f = animal(fauna, id).founder.expect("a founder");
    let cad = f.manifest().cadence_ticks().max(3);
    let a = animal_mut(fauna, id);
    a.age_ticks = 10 * cad + 1;
    a.founder_state.held = Actions { forward, turn: 0.0, feed: 0.0 };
    fauna.step_with(world, flora, 1);
    animal(fauna, id).speed_m_s
}

/// **The Veilhound cruises at its own configured BL/s × its length**, and changing the
/// lanternjaw's cruise leaves it alone; a held full forward moves it at that cruise.
#[test]
fn the_veilhound_cruises_at_its_own_pace_and_the_lanternjaws_does_not_move_it() {
    let mut c = awake(FaunaConfig::default());
    let own = |c: &FaunaConfig| c.founder(CH).cruise_body_lengths_per_s * c.founder(CH).adult_length_m;
    assert!((c.manifest(CH).cruise_m_per_s - own(&c)).abs() < 1e-12, "its manifest's cruise is its own BL/s x length");
    let before = c.manifest(CH).cruise_m_per_s;
    c.founders[LJ.index()].cruise_body_lengths_per_s *= 3.0;
    assert_eq!(c.manifest(CH).cruise_m_per_s, before, "the lanternjaw's cruise is not the Veilhound's");
    c.founders[CH.index()].cruise_body_lengths_per_s *= 1.5;
    assert!((c.manifest(CH).cruise_m_per_s - own(&c)).abs() < 1e-12, "its own knob moves it");

    let world = world(3);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let vh = introduce(&mut fauna, &world, CH, StartingStores::FULL);
    place(&mut fauna, vh, 2.0, LANE_Z, EAST);
    let cruise = fauna.config.manifest(CH).cruise_m_per_s;
    for _ in 0..10 {
        let v = hold_tick(&mut fauna, &world, &mut flora, vh, 1.0);
        assert!((v - cruise).abs() < 0.02 * cruise, "full forward is its cruise: {v} against {cruise}");
    }
}

/// **A rushing Veilhound moves at the rush multiple until its stamina runs out, then drops
/// to cruise, and recovers**: asking its sprint ([`crate::gait::sprint_forward`]) it runs at
/// the sprint's top speed for `endurance_s`, is winded and runs at cruise though it still
/// asks; at cruise it rests, and once rested to `resume_fraction` (`(1 - resume) ×
/// recovery_s`) it sprints at the top speed again.
#[test]
fn a_rushing_veilhound_sprints_until_winded_then_cruises_and_recovers() {
    let mut c = awake(FaunaConfig::default());
    let bl = c.founder(CH).cruise_body_lengths_per_s;
    let sprint = Sprint { body_lengths_per_s: 3.0 * bl, endurance_s: 1.0, recovery_s: 2.0, resume_fraction: 0.5 };
    c.founders[CH.index()].sprint = sprint;
    let world = world(5);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let vh = introduce(&mut fauna, &world, CH, StartingStores::FULL);
    place(&mut fauna, vh, 2.0, LANE_Z, EAST);
    let p = *fauna.config.founder(CH);
    let cruise = fauna.config.manifest(CH).cruise_m_per_s;
    let top = sprint.body_lengths_per_s * p.adult_length_m;
    let rush = crate::gait::sprint_forward(&fauna.config, CH);
    assert!((rush * cruise - top).abs() < 1e-9, "the rush asks for the sprint's top speed");

    let endurance = cubarium_voxel::ticks_at_least_one(sprint.endurance_s);
    let mut fast = 0u64;
    let mut spent_ticks = 0u64;
    // A few ticks past the endurance: well inside the rest it needs before it may sprint again
    // (time at cruise rests it even while it asks for more).
    for t in 0..endurance + 5 {
        let v = hold_tick(&mut fauna, &world, &mut flora, vh, rush);
        let winded = animal(&fauna, vh).founder_state.winded;
        if t < endurance - 1 {
            assert!((v - top).abs() < 0.02 * top, "tick {t}: at the sprint's top speed: {v} against {top}");
        }
        if v > 1.05 * cruise {
            fast += 1;
        } else {
            assert!((v - cruise).abs() < 0.02 * cruise, "tick {t}: spent, it runs at cruise: {v}");
            assert!(winded, "tick {t}: held to cruise only once winded");
            spent_ticks += 1;
        }
    }
    assert!(fast + 1 >= endurance && fast <= endurance + 1, "{fast} sprinting ticks against an endurance of {endurance}");
    assert_eq!(fauna.ledger.winded_by_founder[CH.index()], 1, "winded once");
    assert!(fauna.ledger.sprint_ticks_by_founder[CH.index()] >= fast);

    // Rest at cruise (counting the winded ticks above): winded until rested to `resume_fraction`, then it sprints again.
    let rested = cubarium_voxel::ticks_at_least_one((1.0 - sprint.resume_fraction) * sprint.recovery_s);
    let mut t = spent_ticks;
    while animal(&fauna, vh).founder_state.winded {
        hold_tick(&mut fauna, &world, &mut flora, vh, 1.0);
        t += 1;
        assert!(t <= rested + 2, "still winded after {t} ticks at cruise, rested at {rested}");
    }
    assert!(t + 2 >= rested, "rested in {t} ticks, sooner than {rested}");
    let v = hold_tick(&mut fauna, &world, &mut flora, vh, rush);
    assert!((v - top).abs() < 0.02 * top, "recovered, it sprints again: {v} against {top}");
}

/// **The gate's stamina** on its own: spent at `1 / endurance_s` a second at top speed, pro
/// rata below it; a body that may not sprint (on a wall) is held to cruise and rests.
#[test]
fn the_gate_spends_stamina_pro_rata_and_rests_off_the_sprint() {
    let sp = Sprint { body_lengths_per_s: 3.0, endurance_s: 4.0, recovery_s: 8.0, resume_fraction: 0.5 };
    let mut fs = FounderState::default();
    let (f, w) = sp.gate(1.0, &mut fs, 10.0, true, 1.0);
    assert_eq!((f, w), (3.0, false), "capped at the top speed");
    assert!((fs.sprint_spent - 0.25).abs() < 1e-12, "a second at top spends 1 / endurance_s");
    let (f, _) = sp.gate(1.0, &mut fs, 2.0, true, 1.0);
    assert_eq!(f, 2.0);
    assert!((fs.sprint_spent - 0.375).abs() < 1e-12, "half the margin over cruise spends half as fast");
    let (f, _) = sp.gate(1.0, &mut fs, 3.0, false, 1.0);
    assert_eq!(f, 1.0, "not allowed: cruise");
    assert!((fs.sprint_spent - 0.25).abs() < 1e-12, "and it rests at 1 / recovery_s");
    // No sprint at all: cruise, whatever is asked.
    let mut fs = FounderState::default();
    assert_eq!(Sprint::default().gate(1.0, &mut fs, 5.0, true, 1.0).0, 1.0);
}

/// **The seedporter's ground dash is below the Veilhound's rush**, both from the config (m/s:
/// the sprint's BL/s × the adult length), and its ground flight asks for its dash while the
/// threat is close and its cruise beyond.
#[test]
fn the_seedporters_ground_dash_is_below_the_veilhounds_rush() {
    let c = FaunaConfig::default();
    let top = |f: Founder| {
        let p = c.founder(f);
        p.sprint.effort(p.cruise_body_lengths_per_s) * c.manifest(f).cruise_m_per_s
    };
    assert!(top(SP) > c.manifest(SP).cruise_m_per_s, "the seedporter has a dash");
    assert!(top(CH) > c.manifest(CH).cruise_m_per_s, "the Veilhound has a rush");
    assert!(top(SP) < top(CH), "dash {} m/s below the rush {} m/s", top(SP), top(CH));
    let walk = Actions { forward: 1.0, turn: 0.3, feed: 0.0 };
    let close = crate::gait::dash(&c, SP, true, walk);
    assert_eq!(close.forward, crate::gait::sprint_forward(&c, SP));
    assert_eq!(close.turn, walk.turn);
    assert_eq!(crate::gait::dash(&c, SP, false, walk), walk, "beyond the threat, its walk");
}

/// **Every other lineage's step speed is unchanged**: none has a sprint, so whatever forward
/// its controller or a rule asks, the gate holds it to its cruise.
#[test]
fn every_other_lineage_is_held_to_its_cruise() {
    let c = FaunaConfig::default();
    for f in Founder::ALL.into_iter().filter(|f| ![CH, SP].contains(f)) {
        let p = c.founder(f);
        let mut fs = FounderState::default();
        assert_eq!(p.sprint.gate(p.cruise_body_lengths_per_s, &mut fs, 3.0, true, 1.0).0, 1.0, "{}", f.name());
    }
    // And a walker asked for more than full forward walks at its cruise.
    let world = world(9);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(awake(c));
    fauna.set_births_enabled(false);
    let g = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    place(&mut fauna, g, 2.0, LANE_Z, EAST);
    let cruise = fauna.config.manifest(Founder::Browser).cruise_m_per_s;
    for _ in 0..5 {
        let v = hold_tick(&mut fauna, &world, &mut flora, g, 3.0);
        assert!((v - cruise).abs() < 0.02 * cruise, "held to its cruise: {v} against {cruise}");
    }
}
