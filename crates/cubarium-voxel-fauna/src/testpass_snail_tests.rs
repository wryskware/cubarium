//! **TEST-PASS (independent), AMPHIBIOUS-SNAIL × S2**: the drying clock's day and rain
//! factors and thirst lines, the switch off, the retraction's senses, and a strike on a
//! retracted snail read against the awareness ledger. A child of `step`, sharing the
//! predation fixture.
//!
//! Every case sets the config it relies on; no shipped default, ratio or verdict is asserted.

use std::sync::atomic::Ordering;

use super::predation_fixture::*;
use super::Sky;
use crate::perception::{self, class};
use crate::{Amphibious, Fauna, FaunaConfig, Founder, StartingStores, Track};
use cubarium_voxel::weather::RainMode;
use cubarium_voxel::{DT, World};
use cubarium_voxel_flora::{Flora, FloraConfig};

const SNAIL: Founder = Founder::RippleSnail;
const LJ: Founder = Founder::Lanternjaw;

fn amph(c: &FaunaConfig) -> Amphibious {
    c.founder(SNAIL).amphibious.expect("the ripple-snail is amphibious")
}

/// A snail on the dry plain (no water anywhere), held still; its id.
fn snail(fauna: &mut Fauna, world: &World) -> u64 {
    let id = introduce(fauna, world, SNAIL, StartingStores::FULL);
    place(fauna, id, 3.0, LANE_Z, EAST);
    drive(fauna, id, 0.0, false);
    id
}

/// The clock-off plain with water aloft, raining a shower when `rain` (never stepped, so it
/// keeps falling).
fn plain_raining(seed: u64, rain: bool) -> World {
    let mut w = World::empty(cubarium_voxel::Config {
        width: 48,
        height: 8,
        depth: 12,
        voxel_m: VOXEL_M,
        seed,
        day: cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() },
        closed_water_budget: true,
        initial_atmosphere_m3: 0.5,
        ..cubarium_voxel::Config::default()
    });
    for z in 0..12u32 {
        for x in 0..48i64 {
            for y in 1..=SURFACE_Y {
                w.apply(cubarium_voxel::Command::SetMaterial { x, y, z, material: cubarium_voxel::Material::Soil });
            }
        }
    }
    if rain {
        assert!(w.force_rain(RainMode::Shower), "fixture: rain");
        assert!(w.view().is_raining());
    }
    w
}

fn sky(day: bool) -> Sky {
    Sky { downpour: false, rain_m_per_s: 0.0, clock_on: true, daylight: if day { 1.0 } else { 0.0 } }
}

fn dryness(fauna: &Fauna, id: u64) -> f64 {
    animal(fauna, id).mobility.dryness
}

/// **Day and rain multiply the drying**: one tick ashore on the same ground dries the clock by
/// `DT / T(m)` × `day_drying_factor` by day × `rain_drying_factor` in rain, each alone and both
/// together; rain stamps the last rain tick.
#[test]
fn the_drying_clock_takes_the_day_and_rain_factors_each_and_together() {
    let mut c = FaunaConfig::default();
    {
        let a = c.founders[SNAIL.index()].amphibious.as_mut().unwrap();
        a.day_drying_factor = 3.0;
        a.rain_drying_factor = 0.5;
    }
    let am = amph(&c);
    for (day, rain) in [(false, false), (true, false), (false, true), (true, true)] {
        let world = plain_raining(401, rain);
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let id = snail(&mut fauna, &world);
        let s = animal(&fauna, id).site;
        let m = cubarium_voxel_fauna_ground(&world, s);
        fauna.tick = 77;
        super::desiccation(&mut fauna, &world.view(), &sky(day));
        let mut want = DT * am.drying_per_s(m);
        if day {
            want *= am.day_drying_factor;
        }
        if rain {
            want *= am.rain_drying_factor;
        }
        let got = dryness(&fauna, id);
        assert!((got - want).abs() < 1e-15, "day {day} rain {rain}: {got} against {want}");
        assert_eq!(fauna.last_rain_tick, rain.then_some(77), "day {day} rain {rain}");
    }
}

fn cubarium_voxel_fauna_ground(world: &World, s: cubarium_voxel_flora::Site) -> f64 {
    crate::body::ground_moisture(&world.view(), i64::from(s.x), s.y, s.z)
}

/// **Which thirst line holds**: with the clock between the day line and the night line, the
/// snail is thirsty on a dry day and not at night or on a rainy day.
#[test]
fn the_day_return_line_holds_only_on_a_dry_day() {
    let mut c = FaunaConfig::default();
    {
        let a = c.founders[SNAIL.index()].amphibious.as_mut().unwrap();
        a.day_return_fraction = 0.1;
        a.return_fraction = 0.5;
        a.rain_drying_factor = 0.0;
    }
    for (day, rain, thirsty) in [(true, false, true), (false, false, false), (true, true, false)] {
        let world = plain_raining(402, rain);
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let id = snail(&mut fauna, &world);
        animal_mut(&mut fauna, id).mobility.dryness = 0.3;
        super::desiccation(&mut fauna, &world.view(), &sky(day));
        let m = animal(&fauna, id).mobility;
        assert!(m.ashore, "on the dry plain it is ashore");
        assert_eq!(m.thirsty, thirsty, "day {day} rain {rain}: dryness {}", m.dryness);
    }
}

/// **The switch off is the water-bound snail exactly**: with `on_land` off, a snail ashore
/// with its clock past lethal is not ashore, not thirsty, its clock does not move, and the
/// step does not kill it of desiccation; its retraction still counts down.
#[test]
fn with_on_land_off_the_clock_neither_runs_nor_kills() {
    let world = clock_off(403);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    c.founders[SNAIL.index()].amphibious.as_mut().unwrap().on_land = false;
    let lethal = amph(&c).lethal_fraction;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let id = snail(&mut fauna, &world);
    {
        let m = &mut animal_mut(&mut fauna, id).mobility;
        m.dryness = lethal + 0.5;
        m.ashore = true;
        m.thirsty = true;
        m.retract_ticks = 5;
    }
    for _ in 0..3 {
        fauna.step_with(&world, &mut flora, 1);
    }
    let a = fauna.view().animal(id).copied().expect("not killed by its clock");
    assert_eq!(a.mobility.dryness, lethal + 0.5, "the clock does not run");
    assert!(!a.mobility.ashore && !a.mobility.thirsty);
    assert_eq!(a.mobility.retract_ticks, 2, "the retraction still counts down");
    assert_eq!(fauna.view().ledger.departed_founder(SNAIL, crate::Departure::Desiccation), 0);
}

// ------------------------------------------------------------------ the retraction × S2

/// **It retracts at a threat seen or felt, not heard**, and only at a fresh one: a record whose
/// threat track holds only hearing, or a sight older than the remote window, leaves it open; a
/// fresh sight closes it for `retract_hold_s`.
#[test]
fn a_snail_retracts_at_a_fresh_threat_seen_or_felt_and_not_at_one_only_heard() {
    let world = clock_off(404);
    let mut c = FaunaConfig::default();
    c.founders[SNAIL.index()].amphibious.as_mut().unwrap().retract_hold_s = 2.0;
    let window = c.perception.stagger_ticks();
    for (modes, age, closes) in [
        (perception::HEARING, 0, false),
        (perception::SIGHT, window + 1, false),
        (perception::SIGHT, 0, true),
        (perception::TREMOR, window, true),
    ] {
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let id = snail(&mut fauna, &world);
        fauna.tick = 1000;
        let now = fauna.tick;
        let p = fauna.percepts.entry(id).or_default();
        p.tracks[0] = Track { known: true, id: 999, class: class::THREAT, modes, tick: now - age, ..Track::default() };
        p.n = 1;
        let i = fauna.animals.iter().position(|a| a.id == id).unwrap();
        super::retract(&mut fauna, i);
        let want = if closes { crate::Hunter::ticks(2.0) } else { 0 };
        assert_eq!(animal(&fauna, id).mobility.retract_ticks, want, "modes {modes} age {age}");
    }
}

/// One lanternjaw strike on a snail, `retracted` or not, that sees all round (`aware`: it
/// senses the striker by sight) or senses nothing; `(wound, aware strikes, unaware strikes)` on
/// the snail's lineage.
fn strike_on_snail(retracted: bool, aware: bool, retract_armour: f64) -> (f64, u64, u64) {
    let world = clock_off(405);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = hunting_config();
    c.founders[SNAIL.index()].agility = 0.0;
    c.founders[SNAIL.index()].armour = 0.0;
    c.founders[SNAIL.index()].amphibious.as_mut().unwrap().retract_armour = retract_armour;
    c.founders[SNAIL.index()].senses = if aware {
        crate::SenseTable { live: true, sight_m: 3.0, sight_night_m: 3.0, sight_still_fraction: 1.0, sight_half_angle_deg: 180.0, ..crate::SenseTable::default() }
    } else {
        crate::SenseTable { live: true, ..crate::SenseTable::default() }
    };
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.founders[LJ.index()].hunter.as_mut().unwrap().strike_power = 1e-4;
    c.predation.aware_within_s = 1e6;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let s = introduce(&mut fauna, &world, SNAIL, StartingStores::FULL);
    place(&mut fauna, s, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    drive(&mut fauna, s, 0.0, false);
    if retracted {
        animal_mut(&mut fauna, s).mobility.retract_ticks = 100_000;
    }
    let lj = introduce(&mut fauna, &world, LJ, HUNTING);
    place(&mut fauna, lj, STRIKER_X, LANE_Z, EAST);
    let feed = drive(&mut fauna, lj, 0.0, false);
    feed.store(true, Ordering::Relaxed);
    let before = strikes(&fauna);
    let resolved = |f: &Fauna| strikes(f) > before && phase(f, lj) == Some(crate::StrikePhase::Recovery);
    step_until(&mut fauna, &world, &mut flora, 60, resolved).expect("the lanternjaw strikes");
    let v = fauna.ledger.vigil;
    (animal(&fauna, s).wound, v.aware_strikes[SNAIL.index()], v.unaware_strikes[SNAIL.index()])
}

/// **A retracted snail is closed, not immune**: the same strike wounds it `1 − retract_armour`
/// as much as an open one.
#[test]
fn a_strike_on_a_retracted_snail_is_cut_by_its_retract_armour() {
    let k = 0.6;
    let (open, _, _) = strike_on_snail(false, false, k);
    let (closed, _, _) = strike_on_snail(true, false, k);
    assert!(open > 0.0 && closed > 0.0, "both struck: open {open}, closed {closed}");
    assert!((closed - (1.0 - k) * open).abs() < 1e-9 * open, "closed {closed}, open {open}, retract_armour {k}");
}

/// **S2's awareness reads a retracted snail's record**: a strike on a retracted snail that saw
/// its striker is counted aware (its table is live); one on a snail that senses nothing is
/// counted unaware.
#[test]
fn a_strike_on_a_snail_is_counted_aware_by_its_record() {
    let (_, aware, unaware) = strike_on_snail(true, true, 0.5);
    assert_eq!((aware, unaware), (1, 0), "it saw the striker");
    let (_, aware, unaware) = strike_on_snail(true, false, 0.5);
    assert_eq!((aware, unaware), (0, 1), "its record does not");
}
