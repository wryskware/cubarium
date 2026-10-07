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
//! - **A detached or freed rider** stands on the host's site and is driven by the
//!   controller its lineage's factory gave it at hatching, never consulted while it rode.

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
        "parasite_take_per_day",
        "parasite_detach_body_fraction",
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
    assert!(bad(&|c| c.parasite_take_per_day = -0.01));
    assert!(bad(&|c| c.parasite_take_per_day = f64::INFINITY));
    assert!(bad(&|c| c.parasite_detach_body_fraction = 0.0));
    assert!(bad(&|c| c.parasite_detach_body_fraction = 1.5));
    assert!(bad(&|c| c.parasite_detach_body_fraction = f64::NAN));
    assert!(
        !bad(&|c| c.parasite_take_per_day = 0.0),
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

/// Each tick a rider takes `parasite_take_per_day × host body × DT / ecological_day_s`
/// from its host — read against an identical grazer standing out of reach — and the
/// three ledgers close.
#[test]
fn a_rider_drains_its_hosts_basal_share_and_the_books_close() {
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
    // The minimal gut: the take is `parasite_take_per_day` of the host's body per
    // ecological day, whatever its upkeep.
    let c = s.fauna.config;
    let want = c.parasite_take_per_day / c.life.ecological_day_s * h0.body * DT;
    assert!(want > 0.0);
    assert!(
        (extra - want).abs() <= 1e-6 * want,
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
        parasite_take_per_day: 0.0,
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
        parasite_take_per_day: 7.2,
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
        parasite_take_per_day: 7.2,
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
        parasite_take_per_day: 0.0,
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
    assert_eq!(r.site, h.site, "it drops where its host stands");
    assert!(distance(&s.fauna, host, rider) <= half_length(&s.fauna, host));
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

/// A host's departure of any cause — starved, aged, killed, removed — frees every rider
/// at once, whatever its size, on the host's last face.
#[test]
fn a_hosts_departure_of_any_cause_frees_every_rider_on_the_spot() {
    for cause in [
        Departure::Starved,
        Departure::Aged,
        Departure::Killed {
            by: Founder::Lanternjaw,
        },
        Departure::Removed,
    ] {
        let mut s = Scene::new(
            FaunaConfig {
                parasite_max_per_host: 2,
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
            assert_eq!(
                r.site, spot,
                "{cause:?}: rider #{id} is freed where the host was"
            );
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
