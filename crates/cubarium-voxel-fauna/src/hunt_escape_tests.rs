//! **HUNT-ESCAPE**: the bellwing's flush ([`crate::flush`]) against a strike, and the grown
//! lanternjaw's counter-strike ([`crate::Counter`]). Strikes are begun by hand
//! (`start_strike`) on hand-placed bodies; every knob a case relies on is set in it, and
//! cases compare an arm with the mechanism against one without, never a shipped number.

use super::predation_fixture::*;
use crate::light::BeaconKind;
use crate::perception::Visit;
use crate::{Activity, Command, Counter, Fauna, FaunaConfig, Founder, MobilityMode, Pose, StartingStores, State};
use cubarium_voxel::World;
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const BW: Founder = Founder::Bellwing;
const VH: Founder = Founder::Chorister;
const LJ: Founder = Founder::Lanternjaw;
const SP: Founder = Founder::Seedporter;

/// The clock off (every rhythm awake), and air enough over the plain to fly in.
fn open_world() -> World {
    plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, 811, 24)
}

/// Explicit flush knobs on the bellwing; no dodge dice; the hunters awake at every hour.
fn config(flush: bool) -> FaunaConfig {
    let mut c = hunting_config();
    for f in [BW, VH, LJ] {
        c.founders[f.index()].active = Activity::Always;
    }
    let bw = &mut c.founders[BW.index()];
    bw.agility = 0.0;
    bw.senses.loom_m = 1.0;
    let fl = bw.flight.as_mut().expect("a flyer");
    fl.flush_reaction_s = 0.05;
    fl.flush_speed_factor = if flush { 3.0 } else { 0.0 };
    fl.flush_climb_fraction = 0.5;
    fl.flush_burst_s = 0.3;
    fl.flush_s = 3.0;
    c.light.loom_tau_s = 1.0;
    c
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("alive")
}

fn alive(fauna: &Fauna, id: u64) -> bool {
    fauna.animals.iter().any(|a| a.id == id)
}

/// A striker of lineage `f` on the lane at `STRIKER_X`, facing east, standing still.
fn striker(fauna: &mut Fauna, world: &World, f: Founder) -> u64 {
    let id = introduce(fauna, world, f, HUNTING);
    place(fauna, id, STRIKER_X, LANE_Z, EAST);
    animal_mut(fauna, id).pose.y_m = SURFACE_M;
    animal_mut(fauna, id).eating = true;
    drive(fauna, id, 0.0, false);
    id
}

/// The point `along` of the way into `id`'s strike capsule, `side` metres to its left.
fn in_capsule(fauna: &Fauna, id: u64, along: f64, side: f64) -> (f64, f64) {
    let a = animal(fauna, id);
    let phys = fauna.config.founder(a.founder.unwrap());
    let h = phys.active_hunter().unwrap();
    let g = phys.body_at(a.body);
    let ahead = (h.jaw_forward_length_fraction + along * h.strike_reach_length_fraction) * g.length_m;
    let (x, z) = ahead_of(fauna, id, ahead);
    (x, z + side)
}

/// A bellwing perched on the ground face at `(x, z)`, still under a scripted controller.
fn perched_bellwing(fauna: &mut Fauna, world: &World, (x, z): (f64, f64)) -> u64 {
    perched_bellwing_with(fauna, world, (x, z), StartingStores::FULL)
}

fn perched_bellwing_with(fauna: &mut Fauna, world: &World, (x, z): (f64, f64), stores: StartingStores) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft { pose: Pose { x, z, heading_rad: WEST, y_m: SURFACE_M }, founder: BW, stores }
    ));
    let id = fauna.animals.last().unwrap().id;
    drive(fauna, id, 0.0, false);
    let site = Site { x: (x / VOXEL_M).floor() as u32, y: SURFACE_Y, z: (z / VOXEL_M).floor() as u32 };
    let a = animal_mut(fauna, id);
    a.site = site;
    crate::flight::perch_on_face(a, site);
    id
}

/// One strike by `by` on `prey`, stepped until the striker is out of its windup and lunge.
fn strike(fauna: &mut Fauna, world: &World, flora: &mut Flora, by: u64, prey: u64) {
    let i = index(fauna, by);
    super::start_strike(fauna, i, prey);
    let done = step_until(fauna, world, flora, 60, |f| {
        f.animals.iter().find(|a| a.id == by).is_none_or(|a| a.strike.is_none_or(|s| s.phase == crate::StrikePhase::Recovery))
    });
    assert!(done.is_some(), "the strike resolved");
}

/// Strikes with fixed anatomy on the same awake perched bellwing placements:
/// (bellwings killed, escapes booked, flushes booked, strikes).
fn hunter_on_perched(flush: bool, founder: Founder, warning: f64) -> (usize, u64, u64, usize) {
    let mut kills = 0;
    let (mut escapes, mut flushes, mut n) = (0, 0, 0);
    for along in [0.2, 0.5, 0.8] {
        for side in [0.0, 0.1, -0.1] {
            let world = open_world();
            let mut flora = Flora::new(FloraConfig::default());
            let mut c = config(flush);
            // Hold anatomy, strike timing and prey placements fixed: only the hunter's
            // visual warning differs. These are mechanism fixtures, not tuned defaults.
            c.founders[founder.index()] = c.founders[VH.index()];
            c.founders[founder.index()].hunter.as_mut().unwrap().strike_warning_scale = warning;
            let mut fauna = Fauna::new(c);
            fauna.set_births_enabled(false);
            let vh = striker(&mut fauna, &world, founder);
            let at = in_capsule(&fauna, vh, along, side);
            let bw = perched_bellwing(&mut fauna, &world, at);
            strike(&mut fauna, &world, &mut flora, vh, bw);
            n += 1;
            kills += usize::from(!alive(&fauna, bw));
            escapes += fauna.ledger.flush_escapes_by_founder[BW.index()];
            flushes += fauna.ledger.flushes_by_founder[BW.index()];
            assert_closes(&fauna, "a strike on a perched bellwing");
        }
    }
    (kills, escapes, flushes, n)
}

fn veilhound_on_perched(flush: bool) -> (usize, u64, u64, usize) {
    hunter_on_perched(flush, VH, 1.0)
}

#[test]
fn a_seedporters_short_grab_lands_more_often_than_a_plain_lunge_on_the_same_bellwings() {
    let (sp, _, _, n) = hunter_on_perched(true, SP, 0.15);
    let (vh, _, _, m) = veilhound_on_perched(true);
    assert_eq!(n, m, "the same prey placements");
    assert!(sp > vh, "less visual warning lands more grabs: seedporter {sp}, Veilhound {vh}");
}

#[test]
fn a_neutral_seedporter_warning_matches_the_plain_flush() {
    assert_eq!(hunter_on_perched(true, SP, 1.0), veilhound_on_perched(true));
}

/// Controlled capture calibration, not an ecosystem verdict: the checkpoint supplies
/// physiology and body-size quantiles; both arms get identical capsule placements.
/// `SP_FLUSH_CHECKPOINT` names its fauna.bin; `SP_FLUSH_SCALES` is a comma-separated
/// sweep. No snapshot or world is changed.
#[test]
#[ignore = "study: run by name"]
fn seedporter_warning_checkpoint_probe() {
    let path = std::env::var("SP_FLUSH_CHECKPOINT").expect("SP_FLUSH_CHECKPOINT=.../fauna.bin");
    let saved = crate::snapshot::decode(&std::fs::read(path).unwrap()).unwrap();
    let sizes = |founder| {
        let mut bodies: Vec<f64> = saved.animals.iter().filter(|a| a.founder == Some(founder)).map(|a| a.body).collect();
        bodies.sort_by(f64::total_cmp);
        assert!(!bodies.is_empty(), "checkpoint has {founder:?}");
        [bodies[0], bodies[bodies.len() / 2], bodies[bodies.len() - 1]]
    };
    let world = open_world();
    let scales = std::env::var("SP_FLUSH_SCALES").unwrap_or_else(|_| "1,0.05,0.075,0.1,0.2".into());
    for warning in scales.split(',').map(|s| s.parse::<f64>().expect("numeric warning scale")) {
        assert!(warning.is_finite() && (0.0..=1.0).contains(&warning), "warning fraction in [0, 1]");
        let (mut strikes, mut kills) = (0, 0);
        for sp_body in sizes(SP) {
            for bw_body in sizes(BW) {
                for along in 1..=9 {
                    for side in [-0.25, 0.0, 0.25] {
                        let mut c = saved.config.clone();
                        for f in [SP, BW] { c.founders[f.index()].active = Activity::Always; }
                        c.founders[SP.index()].hunter.as_mut().unwrap().strike_warning_scale = warning;
                        let mut flora = Flora::new(FloraConfig::default());
                        let mut fauna = Fauna::new(c);
                        fauna.set_births_enabled(false);
                        let sp = introduce(&mut fauna, &world, SP, StartingStores { body: sp_body / fauna_max(&saved, SP), reserve: 1.0 });
                        place(&mut fauna, sp, STRIKER_X, LANE_Z, EAST);
                        animal_mut(&mut fauna, sp).pose.y_m = SURFACE_M;
                        animal_mut(&mut fauna, sp).eating = true;
                        drive(&mut fauna, sp, 0.0, false);
                        let g = fauna.config.founder(SP).body_at(animal(&fauna, sp).body);
                        let at = in_capsule(&fauna, sp, f64::from(along) / 10.0, side * g.width_m);
                        let bw = perched_bellwing_with(&mut fauna, &world, at, StartingStores { body: bw_body / fauna_max(&saved, BW), reserve: 1.0 });
                        if !crate::is_prey_of(&fauna.config, &animal(&fauna, sp), &animal(&fauna, bw)) { continue; }
                        strike(&mut fauna, &world, &mut flora, sp, bw);
                        strikes += 1;
                        kills += usize::from(!alive(&fauna, bw));
                        assert_closes(&fauna, "checkpoint-sized strike probe");
                    }
                }
            }
        }
        println!("warning_probe scale={warning} strikes={strikes} kills={kills} land_share={:.4}", kills as f64 / strikes as f64);
    }
}

fn fauna_max(fauna: &Fauna, founder: Founder) -> f64 {
    fauna.config.founder(founder).core.body_max
}

#[test]
fn an_awake_perched_bellwing_that_senses_a_veilhound_strike_usually_escapes_it() {
    let (kills_off, escapes_off, flushes_off, n) = veilhound_on_perched(false);
    assert_eq!(kills_off, n, "fixture: with no flush every one of these strikes kills");
    assert_eq!((escapes_off, flushes_off), (0, 0), "no flush, nothing booked");
    let (kills, escapes, flushes, _) = veilhound_on_perched(true);
    assert!(flushes > 0 && escapes > 0, "it flushed ({flushes}) and the jaw closed on air ({escapes})");
    assert!(2 * kills < n, "most snaps miss a bellwing that reacts: {kills} of {n} killed");
}

#[test]
fn a_flushed_bellwing_flies_off_and_does_not_settle_back_beside_the_hunter_at_once() {
    let world = open_world();
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(true));
    fauna.set_births_enabled(false);
    let vh = striker(&mut fauna, &world, VH);
    let at = in_capsule(&fauna, vh, 0.5, 0.0);
    let bw = perched_bellwing(&mut fauna, &world, at);
    strike(&mut fauna, &world, &mut flora, vh, bw);
    assert!(alive(&fauna, bw), "it escaped");
    let flush_ticks = (fauna.config.founder(BW).flight.unwrap().flush_s / cubarium_voxel::DT) as u32;
    let gap = |f: &Fauna| {
        let (a, b) = (animal(f, vh).pose, animal(f, bw).pose);
        (a.x - b.x).hypot(a.z - b.z)
    };
    let start = gap(&fauna);
    for _ in 0..flush_ticks.saturating_sub(10) {
        fauna.step_with(&world, &mut flora, 1);
        assert_ne!(animal(&fauna, bw).mobility.mode, MobilityMode::Perched, "it stays aloft through its flush");
    }
    // The plain is 3 m deep, so a sideways escape soon meets its edge: it holds off, it
    // does not come back.
    assert!(gap(&fauna) > start, "it kept away from the hunter: {start} m then {} m", gap(&fauna));
}

#[test]
fn a_sleeping_bellwing_can_be_caught() {
    // Midnight: a diurnal bellwing sleeps, the Veilhound hunts at every hour.
    let world = at_hour(0.0, 812);
    let mut c = config(true);
    c.founders[BW.index()].active = Activity::Diurnal;
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let vh = striker(&mut fauna, &world, VH);
    let at = in_capsule(&fauna, vh, 0.5, 0.0);
    let bw = perched_bellwing(&mut fauna, &world, at);
    {
        let a = animal_mut(&mut fauna, bw);
        a.state = State::Asleep;
        a.rest = Some(crate::Rest::Sleep);
    }
    strike(&mut fauna, &world, &mut flora, vh, bw);
    assert_eq!(fauna.ledger.flushes_by_founder[BW.index()], 0, "asleep, it never flushed");
    assert!(!alive(&fauna, bw), "the snap took it");
}

/// A lanternjaw with its lure lit and a bellwing perched in its capsule, drawn to the lure
/// or not: (the bellwing alive after the strike, flushes booked).
fn lure_case(drawn: bool) -> (bool, u64) {
    let world = open_world();
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(true));
    fauna.set_births_enabled(false);
    let lj = striker(&mut fauna, &world, LJ);
    animal_mut(&mut fauna, lj).founder_state.lure = 1.0;
    let at = in_capsule(&fauna, lj, 0.5, 0.0);
    let bw = perched_bellwing(&mut fauna, &world, at);
    if drawn {
        let tick = fauna.tick;
        fauna.percepts.entry(bw).or_default().visit =
            Visit { active: true, kind: BeaconKind::Lure.index() as u8, x: at.0, z: at.1, since: tick };
        assert!(crate::light::drawn_to_lure(&fauna, bw), "fixture: it is drawn to the lure");
    }
    strike(&mut fauna, &world, &mut flora, lj, bw);
    (alive(&fauna, bw), fauna.ledger.flushes_by_founder[BW.index()])
}

#[test]
fn a_bellwing_drawn_to_the_lure_is_still_caught_and_one_that_is_not_flushes() {
    let (lured_alive, lured_flushes) = lure_case(true);
    assert!(!lured_alive && lured_flushes == 0, "at the lure it reads a flower: it stays and is taken ({lured_flushes} flushes)");
    let (free_alive, free_flushes) = lure_case(false);
    assert!(free_alive && free_flushes > 0, "the same strike on a bellwing not drawn to the lure is sensed and escaped");
}

/// A Veilhound's strike on a lanternjaw of `body` (a fraction of its `body_max`), the
/// counter on or off: (the Veilhound's wound, the lanternjaw alive, its wound).
fn counter_case(body: f64, counter: bool) -> (f64, bool, f64) {
    let world = open_world();
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config(true);
    c.founders[LJ.index()].counter = counter.then_some(Counter { reach: 0.5, damage: 0.7, cost: 1.0 });
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let vh = striker(&mut fauna, &world, VH);
    let at = in_capsule(&fauna, vh, 0.5, 0.0);
    let lj = introduce(&mut fauna, &world, LJ, StartingStores { body, reserve: 0.6 });
    place(&mut fauna, lj, at.0, at.1, WEST);
    animal_mut(&mut fauna, lj).pose.y_m = SURFACE_M;
    drive(&mut fauna, lj, 0.0, false);
    strike(&mut fauna, &world, &mut flora, vh, lj);
    assert_closes(&fauna, "a strike on a lanternjaw");
    let lj_after = fauna.animals.iter().find(|a| a.id == lj).map(|a| a.wound);
    (animal(&fauna, vh).wound, lj_after.is_some(), lj_after.unwrap_or(f64::NAN))
}

#[test]
fn a_veilhound_striking_a_grown_lanternjaw_is_wounded_and_one_striking_a_juvenile_is_not() {
    let birth = config(true).founder(LJ).core.birth_body / config(true).founder(LJ).core.body_max;
    let (adult, juvenile) = ((birth + 1.0) / 2.0, birth / 2.0);
    let (wound, _, _) = counter_case(adult, true);
    assert!(wound > 0.0, "a grown lanternjaw answers: the Veilhound's wound {wound}");
    let (wound_juv, _, _) = counter_case(juvenile, true);
    assert_eq!(wound_juv, 0.0, "a juvenile does not answer");
    // The answer grows with the lanternjaw against its attacker.
    let (wound_full, _, _) = counter_case(1.0, true);
    assert!(wound_full > wound, "a bigger lanternjaw answers harder: {wound_full} vs {wound}");
}

#[test]
fn the_counter_strike_leaves_the_strike_on_the_lanternjaw_as_it_was() {
    let birth = config(true).founder(LJ).core.birth_body / config(true).founder(LJ).core.body_max;
    for body in [(birth + 1.0) / 2.0, 1.0] {
        let (w_on, alive_on, lj_on) = counter_case(body, true);
        let (w_off, alive_off, lj_off) = counter_case(body, false);
        assert!(w_on > 0.0 && w_off == 0.0, "only the counter wounds the attacker ({w_on}, {w_off})");
        assert_eq!(alive_on, alive_off, "body {body}: whether the lanternjaw survives is unchanged");
        if alive_on {
            assert_eq!(lj_on, lj_off, "body {body}: its own wound is unchanged");
        }
    }
}
