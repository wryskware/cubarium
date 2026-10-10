//! Parasite hatchlings (`design/handoffs/voxel-predation-parasite-2026-10-01.md`): a
//! lanternjaw egg hatches onto a host, rides it, drains it, and drops off as a free
//! juvenile.
//!
//! Readings these cases pin, where the brief leaves room:
//!
//! - **Distance** to a candidate host is from the egg's column centre to the host's pose
//!   centre, on the ring (wrapped on `x`).
//! - **A rider's position is its host's:** its `site` is its host's `site` every tick,
//!   and its pose lies on the host's body (within half the host's length of the host's
//!   centre). The "face seam" of the voxel ring is its `x` wrap.
//! - **Basal upkeep** is `maintenance_per_s × body` of the host's own lineage, whatever
//!   its activity state (the hosts here are kept awake, so the reading is never tested
//!   against a sleeping host).
//! - **A rider is no target:** a hunter whose prey list includes lanternjaws neither
//!   strikes nor hits one riding a host, though it hits a free one at the same spot.
//! - **`Command::Remove`** on a host's column removes the host and frees its riders; it
//!   does not remove them with it.
//! - **A detached or freed rider** steps off onto the nearest standable face beside its
//!   host, keeping the host's heading (or stays on the host's site when none is in reach,
//!   or when the release has no world to read: an edit's removal, a bare `apply_hit`), and
//!   is driven by the controller its lineage's factory gave it at hatching, never consulted
//!   while it rode.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::predation_fixture::*;
use crate::controller::{Actions, Controller, Response};
use crate::{
    Activity, Attachment, Command, DT, Departure, Fauna, FaunaConfig, Founder, SCHEMA,
    StartingStores, apply_hit,
};
use cubarium_voxel::World;
use cubarium_voxel::weather::DayClock;
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const EGG_SITE: Site = Site {
    x: 24,
    y: SURFACE_Y,
    z: 6,
};

/// The ring's circumference, metres (the fixture plain is 48 voxels of 0.25 m).
const RING_M: f64 = 48.0 * VOXEL_M;

/// An id no body carries, "heard" by a primed egg so its footfall gate is met.
const PHANTOM: u64 = 999_999;

fn egg_xz() -> (f64, f64) {
    (
        (f64::from(EGG_SITE.x) + 0.5) * VOXEL_M,
        (f64::from(EGG_SITE.z) + 0.5) * VOXEL_M,
    )
}

/// The default table with every lineage awake at every hour (so a host walks and is
/// never asleep when its basal upkeep is read) and every egg past its gate hatching at
/// the first dusk it meets.
fn config() -> FaunaConfig {
    let mut c = FaunaConfig {
        hatch_chance_per_dusk: 1.0,
        ..FaunaConfig::default()
    };
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c
}

/// A controller that stands still and counts how often it is asked.
struct Counting(Arc<AtomicUsize>);

impl Controller for Counting {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        self.0.fetch_add(1, Ordering::Relaxed);
        Response::Bounded(Actions::REST)
    }
    fn reset(&mut self) {}
}

/// A fast day (6 s, 120 ticks) from 16:00 and the step its daylight first falls below
/// the day line (the brief's dusk); the next dusk is one day (120 steps) later.
fn fast_day() -> (World, u64) {
    let world = plain(
        DayClock {
            length_min: 0.1,
            start_hour: 16.0,
            ..DayClock::default()
        },
        31,
    );
    let mut probe = world.clone();
    let mut last = probe.life_daylight();
    for t in 1..=120u64 {
        probe.step();
        let now = probe.life_daylight();
        if now < crate::LifeConfig::default().day_line && last >= crate::LifeConfig::default().day_line {
            return (world, t);
        }
        last = now;
    }
    panic!("no dusk within a day");
}

const DAY_TICKS: u64 = 120;

/// Every lanternjaw egg past its footfall gate: threshold 1 and one prey heard now.
fn prime_eggs(fauna: &mut Fauna) {
    let tick = fauna.tick();
    for c in fauna
        .clutches
        .iter_mut()
        .filter(|c| c.lineage == Founder::Lanternjaw)
    {
        let d = c.dormancy.as_mut().expect("a dormant egg");
        d.threshold = 1;
        d.hear(PHANTOM, tick);
    }
}

fn eggs(fauna: &Fauna) -> usize {
    fauna
        .clutches
        .iter()
        .filter(|c| c.lineage == Founder::Lanternjaw)
        .count()
}

/// Lanternjaw ids alive, in id order.
fn lanternjaw_ids(fauna: &Fauna) -> Vec<u64> {
    fauna
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Lanternjaw))
        .map(|a| a.id)
        .collect()
}

fn riders_of(fauna: &Fauna, host: u64) -> Vec<u64> {
    fauna
        .animals
        .iter()
        .filter(|a| a.attached == Some(Attachment { host }))
        .map(|a| a.id)
        .collect()
}

/// Wrapped `(dx, dz)` from `from` to `to`, metres.
fn offset(fauna: &Fauna, from: u64, to: u64) -> (f64, f64) {
    let (a, b) = (animal(fauna, from).pose, animal(fauna, to).pose);
    crate::pose::wrapped_delta(a.x, a.z, b.x, b.z, RING_M)
}

fn distance(fauna: &Fauna, from: u64, to: u64) -> f64 {
    let (dx, dz) = offset(fauna, from, to);
    (dx * dx + dz * dz).sqrt()
}

fn half_length(fauna: &Fauna, id: u64) -> f64 {
    let a = animal(fauna, id);
    let phys = fauna.config.founder(a.founder.expect("a lineage"));
    0.5 * phys.body_at(a.body).length_m
}

/// Set a body's structure and reserve by hand, the organic difference booked as
/// introduced or removed so the ledger still closes (`dung_tests`' `give_gut` pattern).
fn set_stores(fauna: &mut Fauna, id: u64, body: f64, reserve: f64) {
    let a = animal_mut(fauna, id);
    let delta = (body + reserve) - a.organic();
    a.body = body;
    a.reserve = reserve;
    if delta >= 0.0 {
        fauna.ledger.introduced_organic_in += delta;
    } else {
        fauna.ledger.removed_organic_out -= delta;
    }
}

/// A scene around one buried egg bank on a fast day.
struct Scene {
    world: World,
    flora: Flora,
    fauna: Fauna,
    dusk: u64,
    /// Steps taken so far.
    t: u64,
    /// Lanternjaw controller consultations (the lineage's factory hands out [`Counting`]).
    consults: Arc<AtomicUsize>,
}

impl Scene {
    fn new(config: FaunaConfig, eggs: u32) -> Scene {
        let (world, dusk) = fast_day();
        let mut fauna = Fauna::new(config);
        let consults = Arc::new(AtomicUsize::new(0));
        let c = consults.clone();
        fauna.set_founder_factory(
            Founder::Lanternjaw,
            Arc::new(move || -> Box<dyn Controller> { Box::new(Counting(c.clone())) }),
        );
        if eggs > 0 {
            assert!(fauna.apply(
                &world,
                Command::BuryClutch {
                    site: EGG_SITE,
                    lineage: Founder::Lanternjaw,
                    eggs,
                }
            ));
            prime_eggs(&mut fauna);
        }
        Scene {
            world,
            flora: Flora::new(FloraConfig::default()),
            fauna,
            dusk,
            t: 0,
            consults,
        }
    }

    /// A body of `founder` standing still `dx_m` east of the egg's column centre.
    fn add(&mut self, founder: Founder, stores: StartingStores, dx_m: f64) -> u64 {
        let id = introduce(&mut self.fauna, &self.world, founder, stores);
        let (x, z) = egg_xz();
        place(&mut self.fauna, id, (x + dx_m).rem_euclid(RING_M), z, EAST);
        drive(&mut self.fauna, id, 0.0, false);
        id
    }

    fn step(&mut self) {
        self.world.step();
        self.fauna.step_with(&self.world, &mut self.flora, 1);
        self.t += 1;
    }

    fn steps(&mut self, n: u64) {
        for _ in 0..n {
            self.step();
        }
    }

    /// Step to three steps past the next dusk.
    fn past_next_dusk(&mut self) {
        let mut next = self.dusk;
        while next + 3 <= self.t {
            next += DAY_TICKS;
        }
        self.steps(next + 3 - self.t);
    }

    fn assert_closes(&self, when: &str) {
        assert_closes(&self.fauna, when);
    }
}

/// One grazer standing on the egg, and the rider that hatched onto it at dusk.
fn ride(config: FaunaConfig) -> (Scene, u64, u64) {
    let mut s = Scene::new(config, 1);
    let host = s.add(Founder::Browser, StartingStores::FULL, 0.0);
    s.past_next_dusk();
    let riders = riders_of(&s.fauna, host);
    assert_eq!(riders.len(), 1, "one egg hatched onto the grazer on it");
    (s, host, riders[0])
}

// ------------------------------------------------------------------ the knobs

/// Every new number is a serde-visible config field, and the default validates.
#[test]
fn the_four_parasite_knobs_are_serde_config_fields() {
    let c = FaunaConfig::default();
    let v = serde_json::to_value(c).expect("the config serializes");
    for name in [
        "parasite_attach_radius_m",
        "parasite_max_per_host",
        "parasite_take_body_per_day",
        "parasite_host_bear_per_day",
        "parasite_detach_body_fraction",
        "parasite_switch",
        "parasite_switch_radius_m",
        "parasite_switch_give_up_s",
    ] {
        assert!(
            v.get(name).is_some(),
            "{name} is not a serialized config field"
        );
    }
    c.validate().expect("the default validates");
}

/// A knob that is not a number the rule can use is refused.
#[test]
fn the_parasite_knobs_are_validated() {
    let bad = |edit: &dyn Fn(&mut FaunaConfig)| {
        let mut c = FaunaConfig::default();
        edit(&mut c);
        c.validate().is_err()
    };
    assert!(bad(&|c| c.parasite_attach_radius_m = -0.1));
    assert!(bad(&|c| c.parasite_attach_radius_m = f64::NAN));
    assert!(bad(&|c| c.parasite_take_body_per_day = -0.01));
    assert!(bad(&|c| c.parasite_take_body_per_day = f64::INFINITY));
    assert!(bad(&|c| c.parasite_host_bear_per_day = -0.01));
    assert!(bad(&|c| c.parasite_host_bear_per_day = f64::NAN));
    assert!(bad(&|c| c.parasite_detach_body_fraction = 0.0));
    assert!(bad(&|c| c.parasite_detach_body_fraction = 1.5));
    assert!(bad(&|c| c.parasite_detach_body_fraction = f64::NAN));
    assert!(bad(&|c| c.parasite_switch_radius_m = -0.1));
    assert!(bad(&|c| c.parasite_switch_give_up_s = f64::NAN));
    assert!(
        !bad(&|c| c.parasite_take_body_per_day = 0.0),
        "no take is a knob setting"
    );
}

// ------------------------------------------------------------------ hatching

/// An egg past its gate at dusk with a grazer standing within the radius hatches onto
/// it: a `body_min` rider holding the grazer's id, on the grazer's face; the books close.
#[test]
fn an_egg_hatches_onto_a_host_within_the_radius() {
    let mut s = Scene::new(config(), 1);
    let r = s.fauna.config.parasite_attach_radius_m;
    let host = s.add(Founder::Browser, StartingStores::FULL, 0.5 * r);
    s.steps(s.dusk.saturating_sub(2));
    assert_eq!(eggs(&s.fauna), 1, "nothing hatches by day");
    s.past_next_dusk();
    assert_eq!(eggs(&s.fauna), 0, "the egg hatched at dusk");
    let riders = riders_of(&s.fauna, host);
    assert_eq!(riders.len(), 1, "it hatched onto the grazer");
    let rider = animal(&s.fauna, riders[0]);
    let phys = s.fauna.config.founder(Founder::Lanternjaw);
    assert!(
        rider.body <= phys.core.body_min * (1.0 + 1e-3),
        "a hatchling rides: body {} vs body_min {}",
        rider.body,
        phys.core.body_min
    );
    assert_eq!(
        rider.site,
        animal(&s.fauna, host).site,
        "it rides on its host's face"
    );
    assert_eq!(s.fauna.ledger.riders_attached, 1);
    assert_eq!(s.fauna.ledger.hatched, 1);
    s.assert_closes("hatched onto a host");
}

/// With no host in reach the egg stays dormant through the dusk — no free hatchling, its
/// viability and threshold unchanged — and hatches onto a host at a later dusk.
#[test]
fn with_no_host_in_reach_the_egg_stays_dormant_and_tries_again_at_a_later_dusk() {
    let mut s = Scene::new(config(), 1);
    let r = s.fauna.config.parasite_attach_radius_m;
    let host = s.add(Founder::Browser, StartingStores::FULL, 2.0 * r + 0.5);
    let before = s.fauna.clutches[0].dormancy.expect("dormant");
    s.past_next_dusk();
    assert!(
        lanternjaw_ids(&s.fauna).is_empty(),
        "no hatchling without a host"
    );
    assert_eq!(eggs(&s.fauna), 1, "the egg is still in the bank");
    let after = s.fauna.clutches[0].dormancy.expect("still dormant");
    assert_eq!(after.viable_until_tick, before.viable_until_tick);
    assert_eq!(after.threshold, before.threshold);
    assert_eq!(s.fauna.ledger.riders_attached, 0);

    // The grazer comes within reach, a prey is heard again, and the next dusk takes it.
    let (x, z) = egg_xz();
    place(&mut s.fauna, host, x + 0.25 * r, z, EAST);
    prime_eggs(&mut s.fauna);
    s.past_next_dusk();
    assert_eq!(eggs(&s.fauna), 0, "the egg hatched at the later dusk");
    assert_eq!(riders_of(&s.fauna, host).len(), 1);
    assert_eq!(s.fauna.ledger.riders_attached, 1);
    s.assert_closes("hatched a day late");
}

/// A host is a living **non-lanternjaw**: a juvenile lanternjaw on the egg is not one
/// (and, being no adult, does not suppress the hatch either). Of two grazers in reach,
/// the nearer is taken.
#[test]
fn only_a_non_lanternjaw_is_a_host_and_the_nearest_is_taken() {
    let mut s = Scene::new(config(), 1);
    let young = StartingStores {
        body: 0.15,
        reserve: 0.5,
    };
    let juvenile = s.add(Founder::Lanternjaw, young, 0.0);
    s.past_next_dusk();
    assert_eq!(eggs(&s.fauna), 1, "a lanternjaw is no host");
    assert!(riders_of(&s.fauna, juvenile).is_empty());

    let mut s = Scene::new(config(), 1);
    let r = s.fauna.config.parasite_attach_radius_m;
    let far = s.add(Founder::Browser, StartingStores::FULL, 0.8 * r);
    let near = s.add(Founder::Blind, StartingStores::FULL, -0.3 * r);
    s.past_next_dusk();
    assert_eq!(
        riders_of(&s.fauna, near).len(),
        1,
        "the nearer animal hosts"
    );
    assert!(riders_of(&s.fauna, far).is_empty());
}

/// A host carries at most `parasite_max_per_host` riders: at 1, of two eggs under one
/// grazer one hatches onto it and the other stays in the bank; at 2 both ride.
#[test]
fn a_host_carries_at_most_its_max_riders() {
    let mut s = Scene::new(
        FaunaConfig {
            parasite_max_per_host: 1,
            ..config()
        },
        2,
    );
    let host = s.add(Founder::Browser, StartingStores::FULL, 0.0);
    s.past_next_dusk();
    assert_eq!(riders_of(&s.fauna, host).len(), 1, "one rider at most");
    assert_eq!(
        eggs(&s.fauna),
        1,
        "the second egg waits for a host with room"
    );
    assert_eq!(lanternjaw_ids(&s.fauna).len(), 1, "and did not hatch free");

    let mut s = Scene::new(
        FaunaConfig {
            parasite_max_per_host: 2,
            ..config()
        },
        2,
    );
    let host = s.add(Founder::Browser, StartingStores::FULL, 0.0);
    s.past_next_dusk();
    assert_eq!(riders_of(&s.fauna, host).len(), 2);
    assert_eq!(s.fauna.ledger.riders_attached, 2);
}

// ------------------------------------------------------------------ riding

/// A rider is carried: while its host walks straight east it holds the same place on the
/// host's body, stands on the host's face, and crosses the ring's seam with it.
#[test]
fn a_rider_rides_its_host_across_the_seam() {
    let (mut s, host, rider) = ride(config());
    let z = animal(&s.fauna, host).pose.z;
    place(&mut s.fauna, host, RING_M - 1.0, z, EAST);
    drive(&mut s.fauna, host, 1.0, false);
    s.step();
    let mut held = offset(&s.fauna, host, rider);
    let mut crossed = false;
    for _ in 0..200 {
        let before = animal(&s.fauna, host).pose.x;
        s.step();
        let (h, r) = (animal(&s.fauna, host), animal(&s.fauna, rider));
        assert_eq!(r.attached, Some(Attachment { host }));
        assert_eq!(r.site, h.site, "the rider stands on its host's face");
        assert!(
            distance(&s.fauna, host, rider) <= half_length(&s.fauna, host),
            "the rider is on its host's body"
        );
        assert!(
            (0.0..RING_M).contains(&r.pose.x),
            "the rider's pose is on the ring"
        );
        let now = offset(&s.fauna, host, rider);
        assert!(
            (now.0 - held.0).abs() < 1e-9 && (now.1 - held.1).abs() < 1e-9,
            "a fixed anchor: {held:?} became {now:?}"
        );
        held = now;
        if h.pose.x < before {
            crossed = true;
        }
        if crossed && h.pose.x > 1.0 {
            break;
        }
    }
    assert!(crossed, "the host never crossed the seam");
    s.assert_closes("carried over the seam");
}

/// A rider has no motion, controller or strike of its own: on a still host its pose does
/// not change and its lineage's controller is never asked.
#[test]
fn a_rider_has_no_motion_controller_or_strike_of_its_own() {
    let (mut s, _host, rider) = ride(config());
    let asked = s.consults.load(Ordering::Relaxed);
    let pose = animal(&s.fauna, rider).pose;
    let period = Founder::Lanternjaw.manifest().cadence_ticks();
    s.steps(6 * period);
    assert_eq!(
        s.consults.load(Ordering::Relaxed),
        asked,
        "a rider is not consulted"
    );
    assert_eq!(asked, 0, "not even at hatching");
    let a = animal(&s.fauna, rider);
    assert_eq!(a.pose, pose, "a rider on a still host does not move");
    assert_eq!(a.speed_m_s, 0.0);
    assert_eq!(a.strike, None);
    assert_eq!(strikes(&s.fauna), 0);
}

/// A rider cannot be struck on its own: a hunter whose only prey is the lanternjaw,
/// holding its strike at the host and rider for three seconds, begins no strike and
/// lands no hit; the same hunter does hit a free juvenile standing in the same place.
#[test]
fn a_rider_cannot_be_struck_on_its_own() {
    let hunts_lanternjaws = |mut c: FaunaConfig| {
        let h = c.founders[Founder::Lanternjaw.index()]
            .hunter
            .as_mut()
            .expect("the lanternjaw hunts");
        h.prey = [false, false, true, false, false, false, false, false, false];
        c
    };
    let (mut s, host, rider) = ride(hunts_lanternjaws(config()));
    let at = animal(&s.fauna, host).pose;
    let striker = introduce(&mut s.fauna, &s.world, Founder::Lanternjaw, HUNTING);
    place(&mut s.fauna, striker, at.x - IN_CAPSULE_M, at.z, EAST);
    drive(&mut s.fauna, striker, 0.0, true);
    s.steps(60);
    assert_eq!(strikes(&s.fauna), 0, "no strike begins at a rider");
    assert_eq!(hits(&s.fauna), 0);
    let r = animal(&s.fauna, rider);
    assert_eq!(r.wound, 0.0);
    assert_eq!(r.attached, Some(Attachment { host }));

    // The control: a free juvenile in the same place is struck.
    let mut s = Scene::new(hunts_lanternjaws(config()), 0);
    let young = s.add(
        Founder::Lanternjaw,
        StartingStores {
            body: 0.1,
            reserve: 0.5,
        },
        0.0,
    );
    s.fauna.config.founders[Founder::Lanternjaw.index()].agility = 0.0;
    let at = animal(&s.fauna, young).pose;
    let striker = introduce(&mut s.fauna, &s.world, Founder::Lanternjaw, HUNTING);
    place(&mut s.fauna, striker, at.x - IN_CAPSULE_M, at.z, EAST);
    drive(&mut s.fauna, striker, 0.0, true);
    s.steps(60);
    assert!(
        hits(&s.fauna) > 0,
        "the control: a free juvenile there is hit"
    );
}

// ------------------------------------------------------------------ the drain

/// Each tick a rider takes `parasite_take_body_per_day × its own body × DT / ecological_day_s`
/// from its host — read against an identical grazer standing out of reach — and the
/// three ledgers close.
#[test]
fn a_rider_takes_its_own_body_share_and_the_books_close() {
    let mut s = Scene::new(config(), 1);
    let host = s.add(Founder::Browser, StartingStores::FULL, 0.0);
    let twin = s.add(Founder::Browser, StartingStores::FULL, 4.0);
    s.past_next_dusk();
    assert_eq!(riders_of(&s.fauna, host).len(), 1);
    assert!(riders_of(&s.fauna, twin).is_empty());
    let (h0, t0) = (animal(&s.fauna, host), animal(&s.fauna, twin));
    // The drain so far may have come partly out of the host's body; its own upkeep then
    // differs from the twin's by far less than the tolerance below.
    assert!(
        (h0.body - t0.body).abs() < 1e-4 * t0.body,
        "the twins carry the same body"
    );
    s.step();
    let (h1, t1) = (animal(&s.fauna, host), animal(&s.fauna, twin));
    let extra =
        (h0.stored_organic() - h1.stored_organic()) - (t0.stored_organic() - t1.stored_organic());
    // LANTERNJAW-LIFE: the take is `parasite_take_body_per_day` of the rider's own body per
    // ecological day (under what the host bears), whatever its upkeep.
    let c = s.fauna.config;
    let r0 = animal(&s.fauna, riders_of(&s.fauna, host)[0]);
    assert!(crate::host_bears(&c, r0.body, h0.body), "the grazer bears its hatchling");
    let want = c.parasite_take_body_per_day / c.life.ecological_day_s * r0.body * DT;
    assert!(want > 0.0);
    assert!(
        // The host's emptier reserve digests a hair more into its room than the twin's.
        (extra - want).abs() <= 1e-3 * want,
        "the host lost {extra} more than its twin; the drain is {want}"
    );
    s.assert_closes("drained");
}

/// The rider pays its own upkeep — with no drain it only loses matter and does not grow
/// — and grows from what it takes by the normal growth rule: with a drain far above its
/// needs its body rises, never faster than its lineage's `growth_max_per_s`.
#[test]
fn a_rider_pays_its_own_upkeep_and_grows_from_what_it_drains() {
    let (mut s, _host, rider) = ride(FaunaConfig {
        parasite_take_body_per_day: 0.0,
        ..config()
    });
    let a0 = animal(&s.fauna, rider);
    s.steps(20);
    let a1 = animal(&s.fauna, rider);
    assert!(
        a1.stored_organic() < a0.stored_organic(),
        "it pays its upkeep"
    );
    assert!(
        a1.body - a0.body <= (a0.reserve - a1.reserve) * (1.0 + 1e-9),
        "and grows from nothing: its structure came out of its own reserve"
    );
    s.assert_closes("undrained");

    let (mut s, _host, rider) = ride(FaunaConfig {
        parasite_take_body_per_day: 200.0,
        parasite_host_bear_per_day: 1.0e6,
        ..config()
    });
    let cap = s
        .fauna
        .config
        .founder(Founder::Lanternjaw)
        .core
        .growth_max_per_s
        * DT;
    let start = animal(&s.fauna, rider).body;
    let mut last = start;
    for _ in 0..200 {
        s.step();
        let body = animal(&s.fauna, rider).body;
        assert!(
            body - last <= cap * (1.0 + 1e-9),
            "grew {} in a tick, cap {cap}",
            body - last
        );
        last = body;
    }
    assert!(last > start, "a well-fed rider grows: {start} → {last}");
    s.assert_closes("drained and growing");
}

/// The drain never kills the host directly: a grazer with no reserve under a heavy drain
/// starves, booked `Starved` like any other, and its rider is freed.
#[test]
fn a_drained_host_starves_as_usual() {
    let (mut s, host, rider) = ride(FaunaConfig {
        parasite_take_body_per_day: 200.0,
        parasite_host_bear_per_day: 1.0e6,
        ..config()
    });
    let min = s.fauna.config.founder(Founder::Browser).core.body_min;
    set_stores(&mut s.fauna, host, min * 1.001, 0.0);
    for _ in 0..2_000 {
        if s.fauna.view().animal(host).is_none() {
            break;
        }
        s.step();
    }
    assert!(
        s.fauna.view().animal(host).is_none(),
        "the drained host died"
    );
    let l = &s.fauna.ledger;
    assert_eq!(l.departed_founder(Founder::Browser, Departure::Starved), 1);
    for by in Founder::ALL {
        assert_eq!(
            l.departed_founder(Founder::Browser, Departure::Killed { by }),
            0
        );
    }
    assert_eq!(animal(&s.fauna, rider).attached, None);
    s.assert_closes("host starved");
}

// ------------------------------------------------------------------ dropping off

/// Below `parasite_detach_body_fraction × body_max` a rider stays on; at it, it drops off
/// where its host stands and is then a free juvenile, driven by its own controller.
#[test]
fn a_rider_drops_off_at_its_detach_size_and_is_then_free() {
    let (mut s, host, rider) = ride(FaunaConfig {
        parasite_take_body_per_day: 0.0,
        ..config()
    });
    let c = s.fauna.config;
    let line = c.parasite_detach_body_fraction * c.founder(Founder::Lanternjaw).core.body_max;
    let reserve = animal(&s.fauna, rider).reserve;
    set_stores(&mut s.fauna, rider, 0.98 * line, reserve);
    s.steps(40);
    assert_eq!(
        animal(&s.fauna, rider).attached,
        Some(Attachment { host }),
        "not yet"
    );
    assert_eq!(s.fauna.ledger.rider_dropoffs, 0);
    assert_eq!(s.consults.load(Ordering::Relaxed), 0);

    let reserve = animal(&s.fauna, rider).reserve;
    set_stores(&mut s.fauna, rider, 1.001 * line, reserve);
    let mut dropped = false;
    for _ in 0..3 {
        s.step();
        if animal(&s.fauna, rider).attached.is_none() {
            dropped = true;
            break;
        }
    }
    assert!(dropped, "a rider at its detach size drops off");
    let (h, r) = (animal(&s.fauna, host), animal(&s.fauna, rider));
    assert_ne!(r.site, h.site, "on open ground it steps off beside its host");
    assert!(distance(&s.fauna, host, rider) <= crate::edit::DROP_OFF_REACH_M + VOXEL_M);
    assert_eq!(r.pose.heading_rad, h.pose.heading_rad, "keeping its host's heading");
    assert!(
        r.body < c.founder(Founder::Lanternjaw).core.birth_body,
        "a juvenile"
    );
    assert_eq!(s.fauna.ledger.rider_dropoffs, 1);
    assert_eq!(s.fauna.ledger.riders_freed, 0);

    // Free: its own controller drives it, and its host walks away from it.
    let period = Founder::Lanternjaw.manifest().cadence_ticks();
    s.steps(2 * period);
    assert!(
        s.consults.load(Ordering::Relaxed) > 0,
        "a free juvenile is consulted"
    );
    drive(&mut s.fauna, host, 1.0, false);
    s.steps(60);
    assert!(
        distance(&s.fauna, host, rider) > half_length(&s.fauna, host),
        "the host walked off without it"
    );
    assert!(riders_of(&s.fauna, host).is_empty());
    s.assert_closes("dropped off");
}

/// With no face it could stand on within reach beside its host (every column around the
/// host's is rock to the world's top), a rider at its detach size drops off on the host's
/// own place.
#[test]
fn with_no_standable_face_beside_it_a_rider_drops_off_on_its_hosts_place() {
    let (mut s, host, rider) = ride(FaunaConfig {
        parasite_take_body_per_day: 0.0,
        ..config()
    });
    let site = animal(&s.fauna, host).site;
    let (top, reach) = (s.world.config().height - 1, (crate::edit::DROP_OFF_REACH_M / VOXEL_M).ceil() as i64 + 2);
    for dx in -reach..=reach {
        for z in 0..s.world.config().depth {
            let x = i64::from(site.x) + dx;
            if dx == 0 && z == site.z {
                continue;
            }
            for y in site.y + 1..=top {
                s.world.apply(cubarium_voxel::Command::SetMaterial { x, y, z, material: cubarium_voxel::Material::Rock });
            }
        }
    }
    let c = s.fauna.config;
    let line = c.parasite_detach_body_fraction * c.founder(Founder::Lanternjaw).core.body_max;
    let reserve = animal(&s.fauna, rider).reserve;
    set_stores(&mut s.fauna, rider, 1.001 * line, reserve);
    s.step();
    let (h, r) = (animal(&s.fauna, host), animal(&s.fauna, rider));
    assert_eq!(r.attached, None, "it dropped off");
    assert_eq!(r.site, h.site, "nowhere beside to stand: it stays on its host's place");
}

/// A host's departure of any cause — starved, aged, killed, removed — frees every rider
/// at once, whatever its size, on the host's last face.
#[test]
fn a_hosts_departure_of_any_cause_frees_every_rider() {
    for cause in [
        Departure::Starved,
        Departure::Aged,
        Departure::Killed {
            by: Founder::Lanternjaw,
        },
        Departure::Removed,
    ] {
        // No take: a host starved below what it bears would otherwise shed its riders as
        // outgrown (LANTERNJAW-LIFE) before it dies; this case is about the departure.
        let mut s = Scene::new(
            FaunaConfig {
                parasite_max_per_host: 2,
                parasite_take_body_per_day: 0.0,
                ..config()
            },
            2,
        );
        let host = s.add(Founder::Browser, StartingStores::FULL, 0.0);
        s.past_next_dusk();
        let riders = riders_of(&s.fauna, host);
        assert_eq!(riders.len(), 2, "{cause:?}: two riders");
        let spot = animal(&s.fauna, host).site;
        match cause {
            Departure::Starved => {
                let min = s.fauna.config.founder(Founder::Browser).core.body_min;
                set_stores(&mut s.fauna, host, 0.5 * min, 0.0);
                s.step();
            }
            Departure::Aged => {
                let h = animal_mut(&mut s.fauna, host);
                h.lifespan_ticks = h.age_ticks + 1;
                s.steps(2);
            }
            Departure::Killed { by } => {
                apply_hit(&mut s.fauna, &mut s.flora, host, 1.0, by);
            }
            Departure::Removed => {
                assert!(s.fauna.apply(
                    &s.world,
                    Command::Remove {
                        x: i64::from(spot.x),
                        z: spot.z,
                    }
                ));
            }
            Departure::Drowned | Departure::Desiccation | Departure::Fell => unreachable!(),
        }
        assert!(
            s.fauna.view().animal(host).is_none(),
            "{cause:?}: the host is gone"
        );
        assert_eq!(
            s.fauna.ledger.departed_founder(Founder::Browser, cause),
            1,
            "{cause:?}: booked as itself"
        );
        for &id in &riders {
            let r = s
                .fauna
                .view()
                .animal(id)
                .copied()
                .unwrap_or_else(|| panic!("{cause:?}: rider #{id} went with its host"));
            assert_eq!(r.attached, None, "{cause:?}: rider #{id} is freed");
            // A bare `apply_hit` has no world to read, so its riders stay on the spot;
            // every other departure here steps them off beside it.
            if matches!(cause, Departure::Killed { .. }) {
                assert_eq!(r.site, spot, "{cause:?}: rider #{id} is freed where the host was");
            } else {
                assert_ne!(r.site, spot, "{cause:?}: rider #{id} steps off beside the host");
            }
        }
        assert_eq!(s.fauna.ledger.riders_freed, 2, "{cause:?}");
        assert_eq!(s.fauna.ledger.rider_dropoffs, 0, "{cause:?}");
        s.assert_closes(&format!("{cause:?}"));
    }
}

// ------------------------------------------------------------------ the snapshot

/// A snapshot keeps every rider on its host, and the schema has moved past P1's.
#[test]
fn a_snapshot_keeps_riders_on_their_hosts() {
    assert!(
        SCHEMA >= 22,
        "the animal record gained `attached`: schema {SCHEMA}"
    );
    let (mut s, host, rider) = ride(config());
    s.steps(3);
    let loaded = Fauna::load(&s.fauna.save()).expect("round trip");
    assert_eq!(loaded.animals, s.fauna.animals);
    assert_eq!(loaded.ledger, s.fauna.ledger);
    assert_eq!(
        loaded.view().animal(rider).map(|a| a.attached),
        Some(Some(Attachment { host }))
    );
}

/// The loader refuses a rider whose host is missing, a rider on a lanternjaw, and more
/// riders on one host than `parasite_max_per_host`.
#[test]
fn the_loader_refuses_a_missing_host_a_lanternjaw_host_and_too_many_riders() {
    let refused = |fauna: &Fauna, what: &str| {
        let err = Fauna::load(&fauna.save())
            .err()
            .unwrap_or_else(|| panic!("{what} loaded"));
        let msg = format!("{err:#}");
        assert!(msg.contains("host"), "{what}: {msg}");
    };

    let (mut s, _host, rider) = ride(config());
    animal_mut(&mut s.fauna, rider).attached = Some(Attachment { host: 777_777 });
    refused(&s.fauna, "a rider on a missing host");

    let (mut s, _host, rider) = ride(config());
    let adult = introduce(
        &mut s.fauna,
        &s.world,
        Founder::Lanternjaw,
        StartingStores::FULL,
    );
    animal_mut(&mut s.fauna, rider).attached = Some(Attachment { host: adult });
    refused(&s.fauna, "a rider on a lanternjaw");

    let mut s = Scene::new(
        FaunaConfig {
            parasite_max_per_host: 2,
            ..config()
        },
        2,
    );
    let host = s.add(Founder::Browser, StartingStores::FULL, 0.0);
    s.past_next_dusk();
    assert_eq!(riders_of(&s.fauna, host).len(), 2);
    Fauna::load(&s.fauna.save()).expect("two riders where two are allowed load");
    s.fauna.config.parasite_max_per_host = 1;
    refused(&s.fauna, "two riders where one is allowed");
}

// ------------------------------------------------------------------ LANTERNJAW-LIFE

/// What a host lost in one step beyond its twin standing out of reach, organic units.
fn extra_drain(s: &mut Scene, host: u64, twin: u64) -> f64 {
    let (h0, t0) = (animal(&s.fauna, host), animal(&s.fauna, twin));
    s.step();
    let (h1, t1) = (animal(&s.fauna, host), animal(&s.fauna, twin));
    (h0.stored_organic() - h1.stored_organic()) - (t0.stored_organic() - t1.stored_organic())
}

/// **Adult at 35 % of body_max** (Wrysk 2026-10-10: "35%-100% adult size range"): a
/// lanternjaw at 0.35 of its `body_max` is grown — it has reached `birth_body`, and a hunter
/// reads it at its `adult_size_window_factor` — and one just below is a juvenile at its own
/// size.
#[test]
fn a_lanternjaw_at_35_percent_of_body_max_is_adult_and_below_it_a_juvenile() {
    let world = clock_off(41);
    let c = FaunaConfig::default();
    let mut fauna = Fauna::new(c);
    let id = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    let core = crate::effective_config(&c, &animal(&fauna, id));
    let factor = c.founder(Founder::Lanternjaw).adult_size_window_factor;

    animal_mut(&mut fauna, id).body = 0.35 * core.body_max * (1.0 + 1e-9);
    let a = animal(&fauna, id);
    assert!(a.body >= core.birth_body, "35 % of body_max is adult");
    assert!((crate::window_size(&c, &a) - factor * a.body).abs() <= 1e-12 * a.body);

    animal_mut(&mut fauna, id).body = 0.34 * core.body_max;
    let a = animal(&fauna, id);
    assert!(a.body < core.birth_body, "below 35 % it is a juvenile");
    assert_eq!(crate::window_size(&c, &a), a.body, "a juvenile counts at its own size");
}

/// **A rider takes more as it grows** (Wrysk: "they take more the bigger they get"): on a
/// grazer that bears it, a rider of three times the body drains three times as much a tick,
/// read against an identical grazer standing out of reach; the books close.
#[test]
fn a_riders_take_grows_with_its_size() {
    let mut s = Scene::new(config(), 1);
    let host = s.add(Founder::Browser, StartingStores::FULL, 0.0);
    let twin = s.add(Founder::Browser, StartingStores::FULL, 4.0);
    s.past_next_dusk();
    let rider = riders_of(&s.fauna, host)[0];
    let small_body = animal(&s.fauna, rider).body;
    let small = extra_drain(&mut s, host, twin);
    assert!(small > 0.0, "a hatchling takes from its host");

    let c = s.fauna.config;
    let line = c.parasite_detach_body_fraction * c.founder(Founder::Lanternjaw).core.body_max;
    let big = 3.0 * animal(&s.fauna, rider).body;
    assert!(
        big < line && crate::host_bears(&c, big, animal(&s.fauna, host).body),
        "fixture: a rider three times the hatchling still rides the grazer"
    );
    let reserve = animal(&s.fauna, rider).reserve;
    set_stores(&mut s.fauna, rider, big, reserve);
    let large = extra_drain(&mut s, host, twin);
    let ratio = large / small;
    assert!(
        (ratio - big / small_body).abs() <= 0.01 * big / small_body,
        "the take went ×{ratio} for a body ×{}",
        big / small_body
    );
    s.assert_closes("a grown rider's take");
}

/// LJ-HOSTSWITCH's scene: a hatchling on a littershredder whose bear is set so the shredder
/// lets go halfway between a hatchling and the detach size (`limit`, at the shredder's full
/// body), a grazer that bears far more `grazer_dx` east of the egg (`None`: no grazer), and
/// the rider grown just past `limit`, so it has outgrown any shredder (all at most full
/// body) below its detach size. Hands back the scene, the shredder, the grazer and the rider.
fn outgrown(mut c: FaunaConfig, grazer_dx: Option<f64>) -> (Scene, u64, Option<u64>, u64) {
    let lj = c.founder(Founder::Lanternjaw).core;
    let line = c.parasite_detach_body_fraction * lj.body_max;
    let limit = 0.5 * (lj.body_min + line);
    let shredder = c.founder(Founder::Blind).core.body_max;
    c.parasite_host_bear_per_day = c.parasite_take_body_per_day * limit / shredder;
    let mut s = Scene::new(c, 1);
    let host = s.add(Founder::Blind, StartingStores::FULL, 0.0);
    s.past_next_dusk();
    let riders = riders_of(&s.fauna, host);
    assert_eq!(riders.len(), 1, "the hatchling took the shredder, which bears it");
    let rider = riders[0];
    let grazer = grazer_dx.map(|dx| s.add(Founder::Browser, StartingStores::FULL, dx));
    if let Some(g) = grazer {
        let c = s.fauna.config;
        assert!(crate::host_bears(&c, line, animal(&s.fauna, g).body), "fixture: the grazer bears it");
    }
    // Borne, it stays on.
    let reserve = animal(&s.fauna, rider).reserve;
    let borne = 0.97 * limit * animal(&s.fauna, host).body / shredder;
    set_stores(&mut s.fauna, rider, borne, reserve);
    s.steps(5);
    assert_eq!(animal(&s.fauna, rider).attached, Some(Attachment { host }), "still borne");
    let outgrown = 1.03 * limit;
    assert!(outgrown < line, "fixture: under the normal drop-off size");
    let reserve = animal(&s.fauna, rider).reserve;
    set_stores(&mut s.fauna, rider, outgrown, reserve);
    s.step();
    let a = animal(&s.fauna, rider);
    assert!(a.body < line, "below the detach size");
    (s, host, grazer, rider)
}

/// LJ-HOSTSWITCH (Wrysk 2026-10-10: an outgrown larva looks for a bigger host): a rider that
/// outgrows its shredder below its detach size, with a grazer that bears it within the
/// hatchlings' contact radius, takes the grazer and does not go free; the grazer then feeds
/// it, and the books close across the switch.
#[test]
fn an_outgrown_rider_takes_a_bearing_host_in_reach_and_the_books_close() {
    let r = config().parasite_attach_radius_m;
    let (mut s, host, grazer, rider) = outgrown(config(), Some(0.25 * r));
    let grazer = grazer.expect("a grazer");
    assert_eq!(animal(&s.fauna, rider).attached, Some(Attachment { host: grazer }), "it switched to the grazer");
    assert!(riders_of(&s.fauna, host).is_empty(), "and let go of the shredder");
    assert_eq!(s.fauna.ledger.rider_dropoffs, 1, "the let-go is a drop-off");
    assert_eq!(s.fauna.ledger.riders_attached, 1, "a switch is not a hatch");
    let t = s.fauna.host_switches();
    assert_eq!((t.tried, t.made, t.gave_up), (1, 1, 0));
    assert_eq!(t.by_host[Founder::Browser.index()], 1);
    assert_eq!(s.fauna.host_seekers(), 0);
    s.assert_closes("switched");
    let r0 = animal(&s.fauna, rider);
    let g0 = animal(&s.fauna, grazer);
    s.steps(20);
    let (r1, g1) = (animal(&s.fauna, rider), animal(&s.fauna, grazer));
    assert_eq!(r1.attached, Some(Attachment { host: grazer }), "it rides on");
    assert_eq!(r1.site, g1.site, "carried on the grazer");
    assert!(r1.organic() > r0.organic(), "the grazer feeds it");
    assert!(g1.stored_organic() < g0.stored_organic());
    s.assert_closes("riding the new host");
}

/// The walk to a host (LJ-HOSTSWITCH): on its own heuristic, an outgrown rider with a host
/// that bears it inside its sense radius but beyond contact walks to it and takes it.
#[test]
fn an_outgrown_rider_on_its_heuristic_walks_to_a_bearing_host_it_senses() {
    let c = config();
    let dx = 0.5 * (c.parasite_attach_radius_m + c.parasite_switch_radius_m);
    assert!(dx > c.parasite_attach_radius_m + 0.25, "fixture: out of contact");
    let (mut s, _host, grazer, rider) = outgrown(c, Some(dx));
    let grazer = grazer.expect("a grazer");
    assert_eq!(animal(&s.fauna, rider).attached, None, "out of contact, it is off its host");
    assert_eq!(s.fauna.host_seekers(), 1, "and looking");
    let teachers = s.fauna.config.teachers;
    s.fauna.controllers.set(rider, crate::controller::heuristic(Founder::Lanternjaw, teachers));
    let mut took = false;
    for _ in 0..300 {
        s.step();
        if animal(&s.fauna, rider).attached == Some(Attachment { host: grazer }) {
            took = true;
            break;
        }
    }
    assert!(took, "it walked to the grazer and took it");
    assert_eq!(s.fauna.host_switches().made, 1);
    s.assert_closes("walked to a host");
}

/// With no host that bears it in reach, an outgrown rider looks for the give-up time and
/// then goes on as a free juvenile.
#[test]
fn with_no_bearing_host_an_outgrown_rider_goes_free_after_the_give_up_time() {
    let c = FaunaConfig {
        parasite_switch_give_up_s: 2.0,
        ..config()
    };
    let give_up = cubarium_voxel::ticks_at_least_one(c.parasite_switch_give_up_s);
    let (mut s, host, _, rider) = outgrown(c, None);
    assert_eq!(animal(&s.fauna, rider).attached, None, "it outgrew the shredder");
    assert_eq!(s.fauna.host_seekers(), 1, "and looks for another");
    s.steps(give_up / 2);
    assert_eq!(s.fauna.host_seekers(), 1, "still looking");
    s.steps(give_up);
    assert_eq!(s.fauna.host_seekers(), 0, "it gave up");
    let t = s.fauna.host_switches();
    assert_eq!((t.tried, t.made, t.gave_up), (1, 0, 1));
    assert_eq!(animal(&s.fauna, rider).attached, None, "a free juvenile");
    assert!(riders_of(&s.fauna, host).is_empty());
    s.assert_closes("gave up");
}

/// A host too small to bear it is never taken: a second shredder at full body standing in
/// contact with the outgrown rider is passed over, on contact and by the walk.
#[test]
fn an_outgrown_rider_never_takes_a_host_too_small_to_bear_it() {
    let (mut s, host, _, rider) = outgrown(config(), None);
    let r = s.fauna.config.parasite_attach_radius_m;
    let (x, z) = (animal(&s.fauna, rider).pose.x, animal(&s.fauna, rider).pose.z);
    let small = s.add(Founder::Blind, StartingStores::FULL, 0.0);
    place(&mut s.fauna, small, x + 0.25 * r, z, EAST);
    let c = s.fauna.config;
    assert!(!crate::host_bears(&c, animal(&s.fauna, rider).body, animal(&s.fauna, small).body), "fixture: too small");
    let teachers = c.teachers;
    s.fauna.controllers.set(rider, crate::controller::heuristic(Founder::Lanternjaw, teachers));
    s.steps(80);
    assert!(riders_of(&s.fauna, small).is_empty(), "the small shredder is no host");
    assert!(riders_of(&s.fauna, host).is_empty(), "nor is the one it outgrew");
    assert_eq!(animal(&s.fauna, rider).attached, None);
    assert_eq!(s.fauna.host_switches().made, 0);
    s.assert_closes("passed over");
}

/// At or past its detach size a rider drops off free as before, a host that would bear it in
/// contact or not: it does not look for another.
#[test]
fn a_rider_at_its_detach_size_goes_free_and_looks_for_no_host() {
    let (mut s, host, rider) = ride(config());
    let r = s.fauna.config.parasite_attach_radius_m;
    let other = s.add(Founder::Browser, StartingStores::FULL, 0.25 * r);
    let c = s.fauna.config;
    let line = c.parasite_detach_body_fraction * c.founder(Founder::Lanternjaw).core.body_max;
    let reserve = animal(&s.fauna, rider).reserve;
    set_stores(&mut s.fauna, rider, 1.001 * line, reserve);
    s.steps(3);
    assert_eq!(animal(&s.fauna, rider).attached, None, "dropped off");
    assert!(crate::host_bears(&c, animal(&s.fauna, rider).body, animal(&s.fauna, other).body), "fixture: the other bears it");
    s.steps(40);
    assert_eq!(animal(&s.fauna, rider).attached, None, "free");
    assert!(riders_of(&s.fauna, other).is_empty() && riders_of(&s.fauna, host).is_empty());
    assert_eq!(s.fauna.host_switches(), crate::HostSwitchTally::default(), "no quest");
    s.assert_closes("free at its size");
}

/// With the switch off, an outgrown rider goes free and takes no other host (the earlier rule).
#[test]
fn with_the_switch_off_an_outgrown_rider_takes_no_other_host() {
    let r = config().parasite_attach_radius_m;
    let c = FaunaConfig {
        parasite_switch: false,
        ..config()
    };
    let (mut s, _host, grazer, rider) = outgrown(c, Some(0.25 * r));
    s.steps(40);
    assert_eq!(animal(&s.fauna, rider).attached, None, "it takes no other host");
    assert!(riders_of(&s.fauna, grazer.expect("a grazer")).is_empty());
    assert_eq!(s.fauna.host_switches(), crate::HostSwitchTally::default());
    s.assert_closes("outgrown, switch off");
}

/// **Seedporters eat small lanternjaw juveniles** (Wrysk: "seedporters should also eat the
/// juveniles if they are small enough"), and a grown lanternjaw is formidable: a full-grown
/// seedporter takes a free lanternjaw juvenile inside its window but not one at adult size.
#[test]
fn a_seedporter_takes_a_small_lanternjaw_juvenile_but_not_an_adult() {
    let world = clock_off(43);
    let c = FaunaConfig::default();
    let mut fauna = Fauna::new(c);
    let sp = introduce(&mut fauna, &world, Founder::Seedporter, StartingStores::FULL);
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    place(&mut fauna, sp, 2.0, LANE_Z, EAST);
    place(&mut fauna, lj, 2.0 + IN_CAPSULE_M, LANE_Z, WEST);
    let h = c.founder(Founder::Seedporter).active_hunter().expect("the seedporter's catch");
    let core = crate::effective_config(&c, &animal(&fauna, lj));
    let sp_body = animal(&fauna, sp).body;

    let small = 0.5 * core.birth_body.min(h.prey_max_ratio * sp_body);
    assert!(small > core.body_min, "fixture: a living juvenile");
    animal_mut(&mut fauna, lj).body = small;
    assert!(
        crate::is_prey_of(&c, &animal(&fauna, sp), &animal(&fauna, lj)),
        "a small juvenile is the seedporter's prey"
    );

    animal_mut(&mut fauna, lj).body = core.birth_body;
    assert!(
        !crate::is_prey_of(&c, &animal(&fauna, sp), &animal(&fauna, lj)),
        "an adult is not"
    );
}

/// **Many offspring**: a full-grown lanternjaw with a full reserve lays a clutch of more than
/// one egg (the clutch from surplus, its hold shortened here to one tick), and the books
/// close.
#[test]
fn a_lanternjaw_clutch_holds_more_than_one_egg_and_the_books_close() {
    let world = clock_off(47);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = hunting_config();
    for p in c.founders.iter_mut() {
        p.gut.s0 = 0.0;
    }
    let rule = &mut c.founders[Founder::Lanternjaw.index()].core.reproduction;
    rule.surplus_hold_s = DT;
    rule.clutch_hold_max_s = 0.0;
    let mut fauna = Fauna::new(c);
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    drive(&mut fauna, lj, 0.0, false);
    for _ in 0..3 {
        fauna.step_with(&world, &mut flora, 1);
    }
    let laid = fauna.ledger.eggs_laid_by_founder[Founder::Lanternjaw.index()];
    assert!(laid > 1, "a clutch of {laid}");
    assert_closes(&fauna, "after a lanternjaw clutch");
}

// ------------------------------------------------------------------ LJ-GROW

/// The lanternjaw's physiology with its growth slowed: the growth ceiling a third and the
/// conversion yield three quarters of `c`'s. Directions only: no tuned number is read.
fn slower(mut c: FaunaConfig) -> FaunaConfig {
    let p = &mut c.founders[Founder::Lanternjaw.index()];
    p.core.growth_max_per_s /= 3.0;
    for y in p.yield_by_food.iter_mut() {
        *y *= 0.75;
    }
    c
}

/// Ticks until a rider grown to near its detach size, on a full grazer that bears it, drops
/// off, at most `cap`.
fn ticks_to_drop_off(config: FaunaConfig, cap: u64) -> u64 {
    let (mut s, host, rider) = ride(config);
    let c = s.fauna.config;
    let lj = c.founder(Founder::Lanternjaw).core;
    let line = c.parasite_detach_body_fraction * lj.body_max;
    let body = 0.85 * line;
    assert!(
        crate::host_bears(&c, line, animal(&s.fauna, host).body),
        "fixture: the grazer bears the rider all the way to its detach size"
    );
    set_stores(&mut s.fauna, rider, body, lj.reserve_cap * body);
    for t in 0..cap {
        if animal(&s.fauna, rider).attached.is_none() {
            s.assert_closes("dropped off");
            return t;
        }
        s.step();
    }
    cap
}

/// **Faster growth** (LJ-GROW, Wrysk 2026-10-10: "bugs dont take like 4 months to grow"): on
/// the same host and from the same body and reserve, a rider with the lanternjaw's growth
/// reaches its drop-off size sooner than one with it slowed.
#[test]
fn with_faster_growth_a_rider_reaches_drop_off_sooner_on_the_same_host() {
    let cap = 40_000;
    let fast = ticks_to_drop_off(config(), cap);
    let slow = ticks_to_drop_off(slower(config()), cap);
    assert!(fast < cap, "the rider dropped off at all");
    assert!(fast < slow, "faster growth dropped off at tick {fast}, slowed at {slow}");
}

/// Ticks until a free lanternjaw juvenile standing still, fed the same carrion meal every
/// 30 s, grows to its adult `birth_body`, at most `cap`.
fn ticks_to_adult_on_fixed_meals(config: FaunaConfig, cap: u64) -> u64 {
    let world = clock_off(53);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config);
    let id = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    drive(&mut fauna, id, 0.0, false);
    let lj = fauna.config.founder(Founder::Lanternjaw).core;
    let body = 0.9 * lj.birth_body;
    set_stores(&mut fauna, id, body, 0.3 * lj.reserve_cap * body);
    let meal = 0.05 * lj.birth_body;
    let every = (30.0 / DT).round() as u64;
    let i = fauna.animals.iter().position(|a| a.id == id).expect("introduced");
    for t in 0..cap {
        if t % every == 0 {
            let pool = &mut fauna.animals[i].stomach.pools[crate::Food::Carrion.index()];
            pool.organic += meal;
            pool.mineral += 0.03 * meal;
            pool.energy += 2.0 * meal;
            fauna.ledger.introduced_organic_in += meal;
            fauna.ledger.introduced_mineral_in += 0.03 * meal;
            fauna.ledger.introduced_energy_in += 2.0 * meal;
        }
        if animal(&fauna, id).body >= lj.birth_body {
            assert_closes(&fauna, "grown on fixed meals");
            return t;
        }
        fauna.step_with(&world, &mut flora, 1);
    }
    cap
}

/// **A free juvenile grows up sooner on the same intake**: fed identical meals, a lanternjaw
/// juvenile with the lanternjaw's conversion and growth ceiling reaches adult sooner than
/// one with them slowed.
#[test]
fn a_free_juvenile_with_the_same_intake_reaches_adult_sooner() {
    let cap = 40_000;
    let fast = ticks_to_adult_on_fixed_meals(config(), cap);
    let slow = ticks_to_adult_on_fixed_meals(slower(config()), cap);
    assert!(fast < cap, "the juvenile grew up at all");
    assert!(fast < slow, "adult at tick {fast}, slowed at {slow}");
}

/// **The take never exceeds the host's bear**: tick by tick through a ride, and for a rider
/// grown past what its host bears, the host loses (beyond its twin out of reach)
/// at most `parasite_host_bear_per_day` × its body per world day.
#[test]
fn a_riders_take_never_exceeds_what_its_host_bears() {
    let mut s = Scene::new(config(), 1);
    let host = s.add(Founder::Browser, StartingStores::FULL, 0.0);
    let twin = s.add(Founder::Browser, StartingStores::FULL, 4.0);
    s.past_next_dusk();
    let rider = riders_of(&s.fauna, host)[0];
    let c = s.fauna.config;
    let bear = |host_body: f64| c.parasite_host_bear_per_day * host_body / c.life.ecological_day_s * DT;
    for _ in 0..200 {
        let h = animal(&s.fauna, host).body;
        let lost = extra_drain(&mut s, host, twin);
        assert!(lost <= bear(h) * (1.0 + 1e-9) + 1e-15, "lost {lost}, bears {}", bear(h));
    }
    // Grow the rider until it wants twice what its host bears (it drops off after this
    // tick's take; the take itself is still bounded).
    let h = animal(&s.fauna, host).body;
    let big = 2.0 * c.parasite_host_bear_per_day * h / c.parasite_take_body_per_day;
    assert!(!crate::host_bears(&c, big, h), "fixture: the host no longer bears it");
    let reserve = animal(&s.fauna, rider).reserve;
    set_stores(&mut s.fauna, rider, big, reserve);
    let lost = extra_drain(&mut s, host, twin);
    assert!(lost > 0.0, "it still took this tick");
    assert!(lost <= bear(h) * (1.0 + 1e-9), "lost {lost}, bears {}", bear(h));
    s.assert_closes("bounded by the bear");
}

/// Compress physiological time, preserving the daily upkeep, take and growth budget.
/// A complete ride fits in a few hundred ticks; no tuned value is asserted.
fn short_ride_config() -> FaunaConfig {
    let mut c = config();
    let day = 200.0 * DT;
    let scale = c.life.ecological_day_s / day;
    c.life.ecological_day_s = day;
    for p in &mut c.founders {
        p.core.maintenance_per_s *= scale;
        p.core.growth_max_per_s *= scale;
    }
    c
}

fn ticks_to_drop_off_with_take(take_scale: f64) -> u64 {
    let (mut s, host, rider) = ride(short_ride_config());
    s.fauna.set_births_enabled(false);
    // Both arms hatch with identical stores on the same host; change only the take.
    s.fauna.config.parasite_take_body_per_day *= take_scale;
    for t in 0..240 {
        assert!(s.fauna.animals.iter().any(|a| a.id == host), "the host survives");
        if animal(&s.fauna, rider).attached.is_none() {
            s.assert_closes("take-driven drop-off");
            return t;
        }
        s.step();
    }
    panic!("fixture: the rider must finish its ride");
}

#[test]
fn a_faster_take_brings_a_rider_to_drop_off_sooner_on_the_same_host() {
    let fast = ticks_to_drop_off_with_take(1.0);
    let slow = ticks_to_drop_off_with_take(0.5);
    assert!(fast < slow, "faster take drops off at {fast}, slower at {slow}");
}

/// Isolate one ride on each adult host, with full or empty reserves, without feeding,
/// predators or other riders. A small host may release an outgrown rider early;
/// its reserve pays before its structure.
#[test]
fn a_host_survives_a_full_ride_from_hatch_to_drop_off() {
    for founder in Founder::ALL.into_iter().filter(|f| *f != Founder::Lanternjaw) {
        for fullness in [0.0, 1.0] {
            let mut s = Scene::new(config(), 1);
            let host = s.add(founder, StartingStores::FULL, 0.0);
            let core = s.fauna.config.founder(founder).core;
            set_stores(&mut s.fauna, host, core.body_max, fullness * core.reserve_cap * core.body_max);
            s.past_next_dusk();
            let riders = riders_of(&s.fauna, host);
            let hatch_body = s.fauna.config.founder(Founder::Lanternjaw).core.body_min;
            if !crate::host_bears(&s.fauna.config, hatch_body, animal(&s.fauna, host).body) {
                assert!(riders.is_empty(), "a host too small to bear a hatchling is spared");
                continue;
            }
            assert_eq!(riders.len(), 1, "fixture: {} bears a hatchling", founder.name());
            let rider = riders[0];
            s.fauna.config = short_ride_config();
            s.fauna.set_births_enabled(false);
            let mut dropped = false;
            for _ in 0..240 {
                let h = s.fauna.animals.iter().find(|a| a.id == host).expect("one ride must not kill its host");
                let minimum = s.fauna.config.founder(founder).core.body_min;
                assert!(h.body > minimum, "{} stays above its death threshold", founder.name());
                if animal(&s.fauna, rider).attached.is_none() {
                    dropped = true;
                    break;
                }
                s.step();
            }
            assert!(dropped, "{} finishes its ride", founder.name());
            assert!(s.fauna.ledger.parasite_taken_organic > 0.0, "the host paid for the ride");
            s.assert_closes("survived the full ride");
        }
    }
}

/// **The hatch chance raises hatches from the same egg bank**: over the same eggs and dusks,
/// every egg a lower chance hatches a higher one hatches too, and the higher hatches more.
#[test]
fn a_higher_hatch_chance_hatches_more_of_the_same_egg_bank() {
    let (lo, hi) = (0.25, 0.75);
    let (mut n_lo, mut n_hi) = (0, 0);
    for serial in 0..400u64 {
        for dusk in [1_000u64, 145_000, 289_000] {
            let a = crate::hatch_draw(lo, 7, serial, dusk);
            let b = crate::hatch_draw(hi, 7, serial, dusk);
            assert!(!a || b, "egg {serial} hatched at {lo} but not at {hi}");
            n_lo += u32::from(a);
            n_hi += u32::from(b);
        }
    }
    assert!(n_hi > n_lo, "{n_hi} hatches at {hi}, {n_lo} at {lo}");
}
