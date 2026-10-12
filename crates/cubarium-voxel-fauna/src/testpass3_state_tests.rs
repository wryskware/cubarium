//! Independent LURE and schema-62 state contracts:
//! Active windows extend the old rhythm only while the clock is on, strictly inside each
//! sunrise/sunset boundary, independently on each side, across a wrapped day.
//! Derived night time is rebuilt rather than saved. Postcard preserves new configuration,
//! wounds/Fell/drop ledgers, clutch clocks, track posture and calm/refuge vigilance state.

use crate::step::jump;
use crate::testpass3_support::{V, introduce, plain};
use crate::*;
use cubarium_voxel::weather::{DayClock, DayMode, Sun};
use cubarium_voxel_flora::{Flora, FloraConfig};

fn life() -> LifeConfig {
    LifeConfig {
        day_line: 0.6,
        twilight_lo: 0.2,
        twilight_hi: 0.8,
        ..Default::default()
    }
}

fn night(since: f64, until: f64) -> NightClock {
    NightClock {
        since_set_h: since,
        to_rise_h: until,
    }
}

#[test]
fn dusk_window_is_strict_at_its_configured_edge() {
    let w = ActiveWindow {
        before_rise_h: 0.0,
        after_set_h: 1.25,
    };
    assert!(Activity::Diurnal.awake_in(
        true,
        0.0,
        1.0,
        night(w.after_set_h - 0.01, 8.0),
        w,
        &life()
    ));
    assert!(!Activity::Diurnal.awake_in(true, 0.0, 1.0, night(w.after_set_h, 8.0), w, &life()));
    assert!(!Activity::Diurnal.awake_in(
        true,
        0.0,
        1.0,
        night(w.after_set_h + 0.01, 8.0),
        w,
        &life()
    ));
}

#[test]
fn dawn_window_is_strict_at_its_configured_edge() {
    let w = ActiveWindow {
        before_rise_h: 0.75,
        after_set_h: 0.0,
    };
    assert!(Activity::Diurnal.awake_in(
        true,
        0.0,
        1.0,
        night(8.0, w.before_rise_h - 0.01),
        w,
        &life()
    ));
    assert!(!Activity::Diurnal.awake_in(true, 0.0, 1.0, night(8.0, w.before_rise_h), w, &life()));
}

#[test]
fn dawn_and_dusk_windows_are_independent() {
    let w = ActiveWindow {
        before_rise_h: 0.25,
        after_set_h: 2.0,
    };
    assert!(w.holds(night(1.0, 8.0)));
    assert!(!w.holds(night(8.0, 1.0)));
    assert!(w.holds(night(8.0, 0.125)));
    assert!(!w.holds(NightClock::DAY));
}

#[test]
fn zero_window_matches_old_activity_for_every_rhythm_and_face_light() {
    let zero = ActiveWindow {
        before_rise_h: 0.0,
        after_set_h: 0.0,
    };
    for activity in [
        Activity::Always,
        Activity::Diurnal,
        Activity::Nocturnal,
        Activity::Crepuscular,
        Activity::DarkActive,
    ] {
        for clock in [false, true] {
            for daylight in [
                0.0,
                life().twilight_lo,
                life().day_line,
                life().twilight_hi,
                1.0,
            ] {
                for sky in [0.0, 0.5, 1.0] {
                    assert_eq!(
                        activity.awake_in(clock, daylight, sky, night(0.0, 0.0), zero, &life()),
                        activity.awake_under(clock, daylight, sky, &life())
                    );
                }
            }
        }
    }
}

#[test]
fn night_clock_wraps_a_day_whose_sunrise_is_after_sunset_phase() {
    let clock = DayClock::default();
    let sun = Sun::at(&clock, 0.5, 0.75, 0.25);
    assert_eq!(NightClock::of(&sun), night(6.0, 6.0));
    let up = Sun::at(&clock, 0.0, 0.75, 0.25);
    assert_eq!(NightClock::of(&up), NightClock::DAY);
}

#[test]
fn night_clock_treats_exact_sunrise_and_sunset_as_day() {
    let clock = DayClock::default();
    for phase in [0.25, 0.75] {
        assert_eq!(
            NightClock::of(&Sun::at(&clock, phase, 0.25, 0.75)),
            NightClock::DAY
        );
    }
    let after = NightClock::of(&Sun::at(&clock, 0.75 + 0.125, 0.25, 0.75));
    assert_eq!(after, night(3.0, 9.0));
}

fn configured() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        let p = &mut c.founders[f.index()];
        p.drop = DropRule {
            after_s: 0.7,
            max_m: 1.25,
            wound_per_m: 0.2,
        };
        p.active_window = ActiveWindow {
            before_rise_h: 0.3,
            after_set_h: 0.9,
        };
        p.vigilance.watch_calm = true;
        p.vigilance.calm_margin_m = 1.3;
        p.vigilance.calm_facing_deg = 23.0;
        p.vigilance.lost_calm_s = 0.8;
        if let Some(am) = p.amphibious.as_mut() {
            am.egg_water_max_m = 0.7;
            am.egg_climb_max_m = 0.2;
            am.egg_moisture_min = 0.4;
            am.egg_drown_s = 2.0;
            am.egg_dry_moisture = 0.3;
            am.egg_dry_s = 3.0;
            am.egg_search_m = 1.5;
            am.graze_give_up = 0.2;
            am.graze_worth = 0.6;
            am.graze_search_m = 1.2;
        }
    }
    c.founders[Founder::RippleSnail.index()].egg_site = EggSite::Shore;
    c
}

#[test]
fn postcard_preserves_each_lineages_new_config_fields() {
    let f = Fauna::new(configured());
    let bytes = f.save();
    let schema: u32 = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(schema, crate::snapshot::SCHEMA);
    let back = Fauna::load(&bytes).unwrap();
    assert_eq!(back.config, f.config);
}

#[test]
fn postcard_preserves_fall_wound_and_every_drop_ledger() {
    let w = plain(48, 16, 24);
    let mut f = Fauna::new(configured());
    let i = introduce(&mut f, &w, Founder::Seedporter, 8, 6);
    f.animals[i].wound = 0.375;
    let k = Founder::Seedporter.index();
    f.ledger.drops_by_founder[k] = 3;
    f.ledger.drop_m_by_founder[k] = 0.9;
    f.ledger.drop_wound_by_founder[k] = 0.2;
    // A fall departure is represented by both the total and lineage counts.
    f.ledger.deaths_by_cause[Departure::Fell.index()] = 2;
    f.ledger.deaths_by_founder_cause[k][Departure::Fell.index()] = 2;
    let back = Fauna::load(&f.save()).unwrap();
    assert_eq!(back.animals, f.animals);
    assert_eq!(back.ledger, f.ledger);
}

#[test]
fn postcard_preserves_both_consecutive_clutch_clocks() {
    let mut f = Fauna::new(configured());
    f.clutches.push(Clutch {
        site: Site { x: 2, y: 2, z: 2 },
        count: 2,
        organic: 0.5,
        mineral: 0.02,
        energy: 1.0,
        laid_tick: 0,
        lineage: Founder::RippleSnail,
        dormancy: None,
        wet_ticks: 2,
        dry_ticks: 7,
        face: None,
    });
    f.ledger.eggs_drowned_by_founder[Founder::RippleSnail.index()] = 3;
    f.ledger.eggs_dried_by_founder[Founder::RippleSnail.index()] = 4;
    let back = Fauna::load(&f.save()).unwrap();
    assert_eq!(back.clutches, f.clutches);
    assert_eq!(back.ledger, f.ledger);
}

#[test]
fn postcard_preserves_posture_calm_walk_and_refuge_for_multiple_observers() {
    let w = plain(48, 16, 24);
    let mut f = Fauna::new(configured());
    for (x, posture) in [
        (8, perception::posture::ASLEEP),
        (14, perception::posture::REST),
    ] {
        let i = introduce(&mut f, &w, Founder::Seedporter, x, 6);
        let a = f.animals[i];
        let mut p = Percept {
            n: 1,
            tick: 5,
            ..Default::default()
        };
        p.tracks[0] = Track {
            known: true,
            posture,
            id: 95,
            modes: perception::SIGHT,
            tick: 5,
            ..Default::default()
        };
        p.best[perception::class::THREAT as usize] = p.tracks[0];
        p.focus = p.tracks[0];
        p.vigil.calm_walk = x == 8;
        p.vigil.refuge = x == 14;
        p.vigil.close_tick = 3;
        f.percepts.insert(a.id, p);
    }
    let back = Fauna::load(&f.save()).unwrap();
    assert_eq!(back.percepts, f.percepts);
}

#[test]
fn loaded_night_clock_is_rebuilt_at_the_next_tick() {
    let w = cubarium_voxel::World::empty(cubarium_voxel::Config {
        width: 8,
        depth: 4,
        height: 8,
        voxel_m: V,
        day: DayClock {
            mode: DayMode::Cycle,
            start_hour: 0.0,
            ..Default::default()
        },
        ..Default::default()
    });
    let mut f = Fauna::new(configured());
    f.night = night(1.0, 2.0);
    let mut back = Fauna::load(&f.save()).unwrap();
    assert_eq!(back.night, NightClock::DAY);
    let mut flora = Flora::new(FloraConfig::default());
    back.begin_tick(&w, &mut flora);
    let expected = NightClock::of(&w.weather().sun());
    assert!(
        expected.since_set_h.is_finite(),
        "fixture: the clock is in deep night"
    );
    assert_eq!(back.night, expected);
}

#[test]
fn drop_session_plans_are_recomputed_after_postcard_load() {
    let w = plain(48, 16, 24);
    let mut f = Fauna::new(configured());
    let i = introduce(&mut f, &w, Founder::Seedporter, 8, 6);
    let a = f.animals[i];
    f.trapped.insert(
        a.id,
        jump::Trapped {
            since: 1,
            checked: 2,
            site: a.site,
            cornered: true,
            aloft: None,
        },
    );
    f.drop_plans.insert(
        a.id,
        jump::DropPlan {
            from: a.site,
            land: Site { y: 1, ..a.site },
            edge: a.site,
            at: 0,
        },
    );
    let back = Fauna::load(&f.save()).unwrap();
    assert!(back.trapped.is_empty() && back.drop_plans.is_empty());
    assert_eq!(back.config, f.config);
}
