//! **FEAR-2** (Wrysk 2026-10-06: prey watch a sleeping or resting predator, they don't flee it;
//! heuristics must not teach endless flight): a flight ends once the prey is safe — in refuge,
//! beyond reach, or its hunter unknown for `lost_calm_s` — and a hunter that is asleep, or
//! still and facing away in whatever awake state, is watched, while one still and facing the
//! prey, or awake in contact with it, stays a threat.
//!
//! Written before the rule. Every case builds its scene by hand and checks the mechanism
//! against the config it sets (`Vigilance::{lost_calm_s, calm_s, calm_facing_deg}`, the
//! predation table's `still_threat_near_m`, the hunker's `calm_s`); no default, ratio or
//! population is asserted. A child of `step` beside the predation files, sharing their fixture.

use super::predation_fixture::*;
use crate::perception::{self, SIGHT};
use crate::{Actions, Activity, Fauna, FaunaConfig, Founder, MobilityMode, SenseTable, StartingStores, State, Vigil, VigilMode, Vigilance};
use cubarium_voxel::World;
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel_flora::{Flora, FloraConfig};

const SP: Founder = Founder::Seedporter;
const GRAZER: Founder = Founder::Browser;
const VH: Founder = Founder::Chorister;
/// Cropping where it stands.
const CROP: Actions = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };
/// The prey's x; the Veilhound lies east of it on the lane, so the prey's bearing from it is WEST.
const PREY_X: f64 = 2.0;
/// The calm and the lost-hunter calm these cases set, seconds (short, so a case runs fast).
const CALM_S: f64 = 0.5;
const LOST_S: f64 = 2.0;

fn world(seed: u64) -> World {
    plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, seed, 16)
}

fn ticks(s: f64) -> u64 {
    cubarium_voxel::ticks_at_least_one(s)
}

/// Every lineage awake, remote senses every tick, the frondgrazer's hunker off; the seedporter's
/// shipped response and a frondgrazer herd response (sighted all round), both watching, with
/// the calm `CALM_S` and the lost-hunter calm `lost_s`.
fn config(lost_s: f64) -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[GRAZER.index()].hunker.enabled = false;
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.founders[GRAZER.index()].senses =
        SenseTable { live: true, sight_m: 6.0, sight_night_m: 6.0, sight_still_fraction: 1.0, sight_half_angle_deg: 180.0, ..SenseTable::default() };
    c.founders[GRAZER.index()].vigilance = Vigilance {
        mode: VigilMode::Herd,
        flee_start_m: 4.0,
        flee_stop_m: 8.0,
        group_min: 1,
        ..Vigilance::default()
    };
    for f in [SP, GRAZER] {
        let vg = &mut c.founders[f.index()].vigilance;
        vg.watch_calm = true;
        vg.calm_margin_m = 0.0;
        vg.calm_facing_deg = 30.0;
        vg.calm_s = CALM_S;
        vg.lost_calm_s = lost_s;
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

/// The Veilhound up and walking (moving) at its cruise.
fn walking(fauna: &mut Fauna, id: u64) {
    let cruise = fauna.config.manifest(VH).cruise_m_per_s;
    let a = animal_mut(fauna, id);
    a.state = State::Walking;
    a.speed_m_s = cruise;
    a.still_ticks = 0;
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

/// `id` perched in a crown `PERCH_M` over the surface.
fn perch(fauna: &mut Fauna, id: u64) {
    let a = animal_mut(fauna, id);
    a.mobility.mode = MobilityMode::Perched;
    a.pose.y_m = SURFACE_M + PERCH_M;
}

/// A crown perch's height over the surface, metres.
const PERCH_M: f64 = 1.5;

/// The hunter gone from every sense: out of the world.
fn vanish(fauna: &mut Fauna, id: u64) {
    fauna.animals.retain(|a| a.id != id);
}

/// **A flight from a hunter that goes unseen ends within `lost_calm_s`**: a seedporter and a
/// frondgrazer flee a Veilhound walking at them; the Veilhound leaves every sense while the prey
/// is still within `flee_stop_m` of where it last knew it (it does not move here). The flight
/// holds past the calm (`calm_s`) and ends by `lost_calm_s` after the hunter was last known
/// near; with `lost_calm_s` 0 (S2's hold) it is still running at twice that.
#[test]
fn a_flight_from_a_hunter_that_goes_unseen_ends_within_lost_calm_s() {
    let world = world(51);
    let flora = Flora::new(FloraConfig::default());
    for f in [SP, GRAZER] {
        for lost in [LOST_S, 0.0] {
            let mut fauna = Fauna::new(config(lost));
            fauna.set_births_enabled(false);
            let me = body(&mut fauna, &world, f, PREY_X, EAST, 0.0, State::Cropping);
            let vh = body(&mut fauna, &world, VH, PREY_X + 3.0, WEST, 0.0, State::Walking);
            walking(&mut fauna, vh);
            sense(&mut fauna, &world);
            assert!(sees(&fauna, me, vh), "fixture: {} sees the Veilhound", f.name());
            respond(&mut fauna, &world, &flora, me, CROP);
            assert!(vigil(&fauna, me).fleeing, "fixture: {} flees the Veilhound walking at it", f.name());
            let stop = fauna.config.founder(f).vigilance.flee_stop_m;
            assert!(3.0 < stop, "fixture: the prey stays within flee_stop_m of the point");
            vanish(&mut fauna, vh);
            let t0 = fauna.tick;
            let fresh = 2 * fauna.config.perception.stagger_ticks();
            let horizon = 2 * ticks(LOST_S) + fresh + 2;
            let mut ended = None;
            for _ in 0..horizon {
                sense(&mut fauna, &world);
                respond(&mut fauna, &world, &flora, me, CROP);
                if !vigil(&fauna, me).fleeing {
                    ended = Some(fauna.tick - t0);
                    break;
                }
            }
            if lost > 0.0 {
                let n = ended.unwrap_or_else(|| panic!("{}: the flight ends once the hunter is lost", f.name()));
                assert!(n > ticks(CALM_S), "{}: it holds past the calm ({n} ticks)", f.name());
                assert!(n >= ticks(lost), "{}: not before lost_calm_s ({n} ticks)", f.name());
                assert!(n <= ticks(lost) + fresh + 1, "{}: by lost_calm_s after it was last known ({n} ticks)", f.name());
            } else {
                assert!(ended.is_none(), "{}: lost_calm_s 0 keeps S2's hold", f.name());
            }
        }
    }
}

/// **A seedporter perched in refuge with a sleeping Veilhound below feeds**: it fled up from
/// the Veilhound, which then lay down and slept. In its perch the flight is over at once (no
/// calm to wait out), and its own heuristic's actions stand: it feeds, and may climb down.
/// The same Veilhound up and walking keeps it on its perch (the refuge watch, not a flight),
/// still feeding; gone from every sense, the watch ends by `lost_calm_s`.
#[test]
fn a_seedporter_perched_in_refuge_with_a_sleeping_veilhound_below_feeds() {
    let world = world(52);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(LOST_S));
    fauna.set_births_enabled(false);
    let sp = body(&mut fauna, &world, SP, PREY_X, EAST, 0.0, State::Cropping);
    let vh = body(&mut fauna, &world, VH, PREY_X + 2.0, WEST, 0.0, State::Asleep);
    perch(&mut fauna, sp);
    // As it reached the crown: in flight from the Veilhound, last near just now.
    let now = fauna.tick;
    fauna.percepts.entry(sp).or_default().vigil =
        Vigil { fleeing: true, flee_id: vh, flee_tick: now, close_tick: now, from_x: PREY_X + 2.0, from_z: LANE_Z, ..Vigil::default() };
    sense(&mut fauna, &world);
    assert!(sees(&fauna, sp, vh), "fixture: it sees the Veilhound");
    let feed_and_go = Actions { forward: 1.0, turn: 0.25, feed: 1.0 };
    let got = respond(&mut fauna, &world, &flora, sp, feed_and_go);
    let v = vigil(&fauna, sp);
    assert!(!v.fleeing && !v.refuge, "in refuge with the Veilhound asleep: no flight, no watch ({v:?})");
    assert!(got.is_none(), "its heuristic's actions stand: it feeds ({got:?})");
    // Awake and walking below: it keeps its perch and feeds, not in flight.
    walking(&mut fauna, vh);
    sense(&mut fauna, &world);
    let got = respond(&mut fauna, &world, &flora, sp, feed_and_go).expect("the refuge watch holds its perch");
    let v = vigil(&fauna, sp);
    assert!(!v.fleeing && v.refuge, "an awake hunter below: the refuge watch ({v:?})");
    assert_eq!((got.forward, got.feed, got.turn), (0.0, feed_and_go.feed, feed_and_go.turn), "it stays up, feeding and turning as asked");
    // Gone from every sense: the watch ends by lost_calm_s.
    vanish(&mut fauna, vh);
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let t0 = fauna.tick;
    let mut ended = None;
    for _ in 0..2 * ticks(LOST_S) + fresh + 2 {
        sense(&mut fauna, &world);
        respond(&mut fauna, &world, &flora, sp, feed_and_go);
        if !vigil(&fauna, sp).refuge {
            ended = Some(fauna.tick - t0);
            break;
        }
    }
    let n = ended.expect("the refuge watch ends once the hunter is lost");
    assert!(n >= ticks(LOST_S) && n <= ticks(LOST_S) + fresh + 1, "by lost_calm_s ({n} ticks)");
}

/// **A stalking hunter, still and facing the prey, keeps it alert**: a Veilhound awake, still
/// and facing a seedporter inside its flight distance holds the seedporter's flight on the
/// ground, and its refuge watch in a perch, for longer than both the calm and `lost_calm_s`.
#[test]
fn a_stalking_hunter_still_and_facing_the_prey_keeps_it_alert() {
    let world = world(53);
    let flora = Flora::new(FloraConfig::default());
    for perched in [false, true] {
        let mut fauna = Fauna::new(config(LOST_S));
        fauna.set_births_enabled(false);
        let sp = body(&mut fauna, &world, SP, PREY_X, EAST, 0.0, State::Cropping);
        let d = 0.5 * fauna.config.founder(SP).vigilance.flee_start_m;
        let vh = body(&mut fauna, &world, VH, PREY_X + d, WEST, 0.0, State::Walking);
        if perched {
            perch(&mut fauna, sp);
        }
        sense(&mut fauna, &world);
        assert!(sees(&fauna, sp, vh), "fixture: it sees the Veilhound");
        for k in 0..2 * ticks(LOST_S.max(CALM_S)) {
            let got = respond(&mut fauna, &world, &flora, sp, CROP);
            let v = vigil(&fauna, sp);
            if perched {
                assert!(v.refuge && !v.fleeing, "tick {k}: perched, it watches from its refuge ({v:?})");
                assert_eq!(got.map(|a| a.forward), Some(0.0), "tick {k}: it stays up");
            } else {
                assert!(v.fleeing, "tick {k}: on the ground, its flight holds ({v:?})");
            }
            sense(&mut fauna, &world);
        }
    }
}

/// **A Veilhound still and facing away is watched, in whatever awake state**: awake, still,
/// with its heading beyond `calm_facing_deg` of the bearing to the seedporter, standing
/// (walking state, not due a step) or lying (resting), it sends the seedporter nowhere; the
/// same body turned to face it does.
#[test]
fn a_veilhound_still_and_facing_away_is_watched_in_any_awake_state() {
    let world = world(54);
    let flora = Flora::new(FloraConfig::default());
    for state in [State::Walking, State::Resting] {
        for (heading, flees) in [(EAST, false), (WEST, true)] {
            let mut fauna = Fauna::new(config(LOST_S));
            fauna.set_births_enabled(false);
            let sp = body(&mut fauna, &world, SP, PREY_X, EAST, 0.0, State::Cropping);
            let vh = body(&mut fauna, &world, VH, PREY_X + 4.0, heading, 0.0, state);
            sense(&mut fauna, &world);
            assert!(sees(&fauna, sp, vh), "fixture: it sees the Veilhound");
            let got = respond(&mut fauna, &world, &flora, sp, CROP);
            assert_eq!(vigil(&fauna, sp).fleeing, flees, "{state:?}, heading {heading:.2}");
            if !flees {
                assert!(got.is_none(), "{state:?} facing away: it keeps cropping ({got:?})");
            }
        }
    }
}

/// **An awake hunter in contact keeps a hunkered frondgrazer clamped down, facing away or
/// not**: within the predation table's `still_threat_near_m`, still, its heading away from
/// the grazer, standing or lying awake, it holds the hunker past the hunker's calm; asleep
/// there it does not (the grazer comes out by its calm).
#[test]
fn an_awake_hunter_in_contact_keeps_a_hunkered_grazer_clamped_down() {
    let world = world(55);
    let flora = Flora::new(FloraConfig::default());
    for state in [State::Walking, State::Resting, State::Asleep] {
        let mut c = config(LOST_S);
        c.founders[GRAZER.index()].hunker = crate::vigil::hunker_of(GRAZER);
        let near = c.predation.still_threat_near_m;
        let calm = c.founder(GRAZER).hunker.calm_s;
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let g = body(&mut fauna, &world, GRAZER, PREY_X, EAST, 0.0, State::Resting);
        let vh = body(&mut fauna, &world, VH, PREY_X + 0.8 * near, EAST, 0.0, state);
        let now = fauna.tick;
        fauna.percepts.entry(g).or_default().vigil = Vigil { hunkered: true, hunker_close: now, ..Vigil::default() };
        animal_mut(&mut fauna, g).hunkered = true;
        sense(&mut fauna, &world);
        assert!(sees(&fauna, g, vh), "fixture: the grazer sees the Veilhound");
        let mut out_at = None;
        for k in 0..2 * ticks(calm) {
            respond(&mut fauna, &world, &flora, g, CROP);
            if !vigil(&fauna, g).hunkered {
                out_at = Some(k);
                break;
            }
            sense(&mut fauna, &world);
        }
        if state == State::Asleep {
            assert!(out_at.is_some(), "asleep in contact: it comes out by its calm");
        } else {
            assert!(out_at.is_none(), "{state:?} in contact, facing away: it stays clamped down (out at {out_at:?})");
        }
    }
}
