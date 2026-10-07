//! **FEAR** (Wrysk 2026-10-06: "Should seedporters ignore a resting or sleeping Veilhound, the
//! way gazelles graze near resting lions?" — "sure."): a prey watches a hunter it sees asleep,
//! or resting and not facing it, instead of fleeing it; it keeps `calm_margin_m` from it,
//! walking off without a sprint inside that; and it fears it fully again once the hunter
//! wakes, faces it, or moves.
//!
//! Written before the rule. Every case builds its scene by hand and checks the mechanism
//! against the config it sets ([`Vigilance::watch_calm`], `calm_margin_m`, `calm_facing_deg`);
//! no default, ratio or population is asserted. A child of `step` beside the predation files,
//! sharing their fixture.

use super::predation_fixture::*;
use crate::perception::{self, SIGHT};
use crate::{Actions, Activity, Fauna, FaunaConfig, Founder, SenseTable, StartingStores, State, Vigil, VigilMode, Vigilance};
use cubarium_voxel::World;
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel_flora::{Flora, FloraConfig};

const SP: Founder = Founder::Seedporter;
const GRAZER: Founder = Founder::Browser;
const VH: Founder = Founder::Chorister;
/// Cropping where it stands.
const CROP: Actions = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };
/// The seedporter's x; the Veilhound lies east of it on the lane.
const SP_X: f64 = 2.0;

fn world(seed: u64) -> World {
    plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, seed, 16)
}

/// Every lineage awake, remote senses every tick, the frondgrazer's hunker off unless a case
/// turns it on; the seedporter's shipped response with the calm watch set as given.
fn config(watch: bool, margin_m: f64, facing_deg: f64) -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[GRAZER.index()].hunker.enabled = false;
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    for f in [SP, GRAZER] {
        let vg = &mut c.founders[f.index()].vigilance;
        vg.watch_calm = watch;
        vg.calm_margin_m = margin_m;
        vg.calm_facing_deg = facing_deg;
    }
    c
}

/// A body of `f` at `(x, LANE_Z)` facing `heading`, moving at `speed` (0: still), in `state`.
fn body(fauna: &mut Fauna, world: &World, f: Founder, x: f64, heading: f64, speed: f64, state: State) -> u64 {
    let id = introduce(fauna, world, f, StartingStores::FULL);
    place(fauna, id, x, LANE_Z, heading);
    drive(fauna, id, 0.0, false);
    let a = animal_mut(fauna, id);
    a.speed_m_s = speed;
    a.still_ticks = if speed > 0.0 { 0 } else { u64::MAX / 2 };
    a.state = state;
    id
}

/// One remote sensing for every live body, at the next tick.
fn sense(fauna: &mut Fauna, world: &World) {
    fauna.tick += 1;
    perception::sense_pass(fauna, &world.view(), false, 1.0, 1);
}

fn respond(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, held: Actions) -> Option<Actions> {
    let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
    let want = perception::Want { tremor: true, ..Default::default() };
    perception::refresh(fauna, i, &world.view(), &flora.view(), None, want);
    crate::vigil::respond(fauna, i, &world.view(), &flora.view(), held)
}

fn vigil(fauna: &Fauna, id: u64) -> Vigil {
    fauna.percept(id).map(|p| p.vigil).unwrap_or_default()
}

fn sees(fauna: &Fauna, who: u64, of: u64) -> bool {
    fauna.percept(who).and_then(|p| p.track(of)).is_some_and(|t| t.modes & SIGHT != 0)
}

/// **A seedporter 4 m from a sleeping Veilhound feeds and does not flee**, though the
/// Veilhound faces it and lies inside its flight distance; with the watch off (S2's rule:
/// any threat it senses counts) the same scene sends it running.
#[test]
fn a_seedporter_four_metres_from_a_sleeping_veilhound_feeds_and_does_not_flee() {
    let world = world(41);
    let flora = Flora::new(FloraConfig::default());
    let d = 4.0;
    for watch in [false, true] {
        let c = config(watch, 0.5 * d, 30.0);
        assert!(c.founder(SP).vigilance.flee_start_m >= d, "fixture: the Veilhound lies inside its flight distance");
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let sp = body(&mut fauna, &world, SP, SP_X, EAST, 0.0, State::Cropping);
        let vh = body(&mut fauna, &world, VH, SP_X + d, WEST, 0.0, State::Asleep);
        sense(&mut fauna, &world);
        assert!(sees(&fauna, sp, vh), "fixture: the seedporter sees the Veilhound");
        let got = respond(&mut fauna, &world, &flora, sp, CROP);
        if watch {
            assert!(!vigil(&fauna, sp).fleeing, "watching: it does not flee a sleeping Veilhound");
            assert!(got.is_none(), "watching: its own heuristic keeps cropping ({got:?})");
            assert_eq!(fauna.ledger.vigil.flees[SP.index()], 0);
        } else {
            assert!(vigil(&fauna, sp).fleeing, "the watch off: any threat it senses sends it off");
        }
    }
}

/// **The same seedporter flees once the Veilhound wakes and approaches**: watching it asleep,
/// then the Veilhound up and walking at it.
#[test]
fn the_seedporter_flees_once_the_veilhound_wakes_and_approaches() {
    let world = world(42);
    let flora = Flora::new(FloraConfig::default());
    let d = 4.0;
    let mut fauna = Fauna::new(config(true, 0.5 * d, 30.0));
    fauna.set_births_enabled(false);
    let sp = body(&mut fauna, &world, SP, SP_X, EAST, 0.0, State::Cropping);
    let vh = body(&mut fauna, &world, VH, SP_X + d, WEST, 0.0, State::Asleep);
    sense(&mut fauna, &world);
    assert!(respond(&mut fauna, &world, &flora, sp, CROP).is_none() && !vigil(&fauna, sp).fleeing, "asleep: watched");
    // It wakes, stands and walks at the seedporter.
    let cruise = fauna.config.manifest(VH).cruise_m_per_s;
    {
        let a = animal_mut(&mut fauna, vh);
        a.state = State::Walking;
        a.speed_m_s = cruise;
        a.still_ticks = 0;
    }
    place(&mut fauna, vh, SP_X + d - 0.3, LANE_Z, WEST);
    sense(&mut fauna, &world);
    let act = respond(&mut fauna, &world, &flora, sp, CROP).expect("it answers the approach");
    assert!(vigil(&fauna, sp).fleeing, "awake and approaching: full fear");
    assert_eq!(act.feed, 0.0, "in flight it does not feed");
    assert_eq!(fauna.ledger.vigil.flees[SP.index()], 1, "one flight");
}

/// **Inside `calm_margin_m` it moves off without sprinting**: a seedporter and a frondgrazer
/// with a sleeping Veilhound inside their margin walk away at no more than their walk, start
/// no flight, run to no refuge, and stop feeding while they walk; outside the margin they are
/// left to their heuristic. The frondgrazer's case lies inside the predation table's
/// `still_threat_near_m`, where S2 read a still body as approaching: with the watch off it
/// flees.
#[test]
fn inside_the_calm_margin_it_moves_off_without_sprinting() {
    let world = world(43);
    let flora = Flora::new(FloraConfig::default());
    let margin = 2.0;
    for f in [SP, GRAZER] {
        let near = config(true, margin, 30.0).predation.still_threat_near_m;
        let d_in = if f == GRAZER { 0.8 * near.min(margin) } else { 0.5 * margin };
        for watch in [true, false] {
            let mut c = config(watch, margin, 30.0);
            if f == GRAZER {
                c.founders[GRAZER.index()].senses =
                    SenseTable { live: true, sight_m: 6.0, sight_night_m: 6.0, sight_still_fraction: 1.0, sight_half_angle_deg: 180.0, ..SenseTable::default() };
                c.founders[GRAZER.index()].vigilance = Vigilance {
                    mode: VigilMode::Herd,
                    flee_start_m: 4.0,
                    flee_stop_m: 8.0,
                    calm_s: 10.0,
                    group_min: 1,
                    watch_calm: watch,
                    calm_margin_m: margin,
                    calm_facing_deg: 30.0,
                    ..Vigilance::default()
                };
            }
            if f == SP && !watch {
                continue;
            }
            let mut fauna = Fauna::new(c);
            fauna.set_births_enabled(false);
            // Facing the Veilhound, then facing away from it.
            let me = body(&mut fauna, &world, f, SP_X, EAST, 0.0, State::Cropping);
            let vh = body(&mut fauna, &world, VH, SP_X + d_in, EAST, 0.0, State::Asleep);
            sense(&mut fauna, &world);
            assert!(sees(&fauna, me, vh), "fixture: {} sees the Veilhound", f.name());
            let act = respond(&mut fauna, &world, &flora, me, CROP);
            if !watch {
                assert!(vigil(&fauna, me).fleeing, "{}: the watch off, a still body this near starts a flight", f.name());
                continue;
            }
            let act = act.unwrap_or_else(|| panic!("{}: inside the margin it moves off", f.name()));
            let v = vigil(&fauna, me);
            assert!(!v.fleeing && v.calm_walk, "{}: a calm walk, not a flight ({v:?})", f.name());
            assert_eq!(act.feed, 0.0, "{}: it does not feed while it walks off", f.name());
            assert!(act.forward <= 1.0, "{}: no sprint ({act:?})", f.name());
            assert!(act.forward == 0.0 && act.turn != 0.0, "{}: facing the Veilhound, it turns round in place ({act:?})", f.name());
            animal_mut(&mut fauna, me).pose.heading_rad = WEST;
            let act = respond(&mut fauna, &world, &flora, me, CROP).expect("still inside the margin");
            assert!(act.forward > 0.0 && act.forward <= 1.0, "{}: facing away it walks, at most its walk ({act:?})", f.name());
            assert_eq!(fauna.ledger.vigil.flees[f.index()], 0, "{}: no flight", f.name());
            assert_eq!(fauna.ledger.vigil.refuges[f.index()], 0, "{}: no refuge run", f.name());
            // Outside the margin (still inside the flight distance): its heuristic again.
            let start = fauna.config.founder(f).vigilance.flee_start_m;
            place(&mut fauna, vh, SP_X + 0.5 * (margin + start), LANE_Z, EAST);
            sense(&mut fauna, &world);
            let got = respond(&mut fauna, &world, &flora, me, CROP);
            assert!(got.is_none() && !vigil(&fauna, me).calm_walk, "{}: outside the margin it is left to feed ({got:?})", f.name());
        }
    }
}

/// **A resting Veilhound oriented at it, or approaching it, is a threat**: awake, still and
/// facing away (beyond `calm_facing_deg` of the seedporter's bearing) it is watched; still
/// and facing it (within that) it is fled; and moving toward it, it is fled.
#[test]
fn a_resting_veilhound_oriented_at_it_or_approaching_is_a_threat() {
    let world = world(44);
    let flora = Flora::new(FloraConfig::default());
    let facing = 30.0f64;
    // The seedporter lies due west of the Veilhound: the bearing it faces is WEST.
    let off = facing.to_radians();
    for (heading, moving, flees) in [
        (WEST + 1.5 * off, false, false),
        (EAST, false, false),
        (WEST + 0.5 * off, false, true),
        (WEST - 0.5 * off, false, true),
        (WEST, true, true),
    ] {
        let mut fauna = Fauna::new(config(true, 1.0, facing));
        fauna.set_births_enabled(false);
        let sp = body(&mut fauna, &world, SP, SP_X, EAST, 0.0, State::Cropping);
        let speed = if moving { fauna.config.manifest(VH).cruise_m_per_s } else { 0.0 };
        let state = if moving { State::Walking } else { State::Resting };
        let vh = body(&mut fauna, &world, VH, SP_X + 4.0, heading, speed, state);
        sense(&mut fauna, &world);
        assert!(sees(&fauna, sp, vh), "fixture: the seedporter sees the Veilhound");
        respond(&mut fauna, &world, &flora, sp, CROP);
        assert_eq!(vigil(&fauna, sp).fleeing, flees, "heading {heading:.2}, moving {moving}");
    }
}

/// **The frondgrazer's hunker still needs an approaching hunter**: a sleeping Veilhound inside
/// its hunker distance leaves it grazing; the same Veilhound up and walking at it clamps it
/// down (its armour rules are FEAR's to keep, not to change).
#[test]
fn a_sleeping_veilhound_does_not_hunker_a_frondgrazer_and_an_approaching_one_does() {
    let world = world(45);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config(true, 0.5, 30.0);
    c.founders[GRAZER.index()].hunker = crate::vigil::hunker_of(GRAZER);
    c.founders[GRAZER.index()].senses =
        SenseTable { live: true, sight_m: 6.0, sight_night_m: 6.0, sight_still_fraction: 1.0, sight_half_angle_deg: 180.0, ..SenseTable::default() };
    let enter = c.founder(GRAZER).hunker.enter_m;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, SP_X, EAST, 0.0, State::Cropping);
    let vh = body(&mut fauna, &world, VH, SP_X + 0.6 * enter, WEST, 0.0, State::Asleep);
    sense(&mut fauna, &world);
    respond(&mut fauna, &world, &flora, g, CROP);
    assert!(!vigil(&fauna, g).hunkered, "asleep: no hunker");
    let cruise = fauna.config.manifest(VH).cruise_m_per_s;
    {
        let a = animal_mut(&mut fauna, vh);
        a.state = State::Walking;
        a.speed_m_s = cruise;
        a.still_ticks = 0;
    }
    sense(&mut fauna, &world);
    respond(&mut fauna, &world, &flora, g, CROP);
    assert!(vigil(&fauna, g).hunkered, "awake and approaching: it hunkers");
}
