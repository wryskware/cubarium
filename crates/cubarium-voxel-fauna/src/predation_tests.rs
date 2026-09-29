//! P1 (predation substrate), tests first: the lanternjaw's table, the strike, dodge,
//! wounds and the kill (`design/predation-and-flight-2026-09-28.md` §2.1, §2.3–§2.5,
//! §2.12; brief P1 cases 1–8). Red by design until the implementation commit adds:
//!
//! - `Founder::Lanternjaw` (manifest.rs), named `"lanternjaw"`, with its manifest's
//!   `cruise_m_per_s` at 0.6 BL/s of its 1 m body;
//! - on `FounderPhysiology` (body.rs): `hunter: Option<Hunter>`, `armour: f64`,
//!   `agility: f64`, `counter: Option<Counter>`;
//! - `Hunter` (body.rs, re-exported): `strike_power`, `prey_max_ratio`,
//!   `strike_reach_length_fraction`, `jaw_band_height_fraction`, `windup_s`, `lunge_s`,
//!   `recovery_s`, `strike_cost_s`, `rest_at`, `torpor_upkeep`, `wake_at` (all `f64`) and
//!   `fn hunts(&self, prey: Founder) -> bool`;
//! - `Counter { reach, damage, cost }` (body.rs, re-exported), `None` for every lineage;
//! - `Diet::Carrion` (lib.rs): the lanternjaw's mouth, accepting no stand's foliage;
//! - on `Animal` (lib.rs): `wound: f64` and `strike: Option<Strike>`;
//! - `Strike { phase: StrikePhase, .. }` and `StrikePhase::{Windup, Lunge, Recovery}`
//!   (predation.rs, re-exported);
//! - `State::awareness(self) -> f64` (lib.rs): the dodge table's awareness column;
//! - `dodge_chance(agility: f64, state: State, struck_from_rear: bool,
//!   reserve_fraction: f64) -> f64` and `dodges(chance: f64, seed: u64, striker: u64,
//!   prey: u64, tick: u64) -> bool` (predation.rs, re-exported): the draw is keyed by
//!   the world seed, like every draw in the step;
//! - `Departure::Killed { by: Founder }` (lib.rs);
//! - on `FaunaLedger` (lib.rs): `strikes_by_founder`, `hits_by_founder`,
//!   `dodges_by_founder`, each `[u64; Founder::COUNT]` indexed by the **striker's**
//!   lineage (a strike is counted when it begins, a hit and a dodge when contact is
//!   resolved);
//! - `HEAL_COST: f64` (lib.rs): organic matter a unit of wound costs to heal per unit of
//!   body. The note names `heal_cost` and pins no value.
//! - `cubarium_voxel_flora::Ground::carcass: Option<Carcass>` with `Carcass { lineage:
//!   u8 (the dead body's Founder::index), heading_rad: f64, organic: f64 }`, written by
//!   a kill.
//!
//! Readings of the note these cases rely on: a hit wakes the prey, ending any rest,
//! sleep or satiated (an adult grazer is hard, and big prey can get away); the lunge's
//! own motion is paid
//! inside the strike cost (the case allows at most a tenth more); a prey that leaves the
//! capsule during the windup is still lunged at, and that miss pays the cost.

use std::sync::atomic::Ordering;

use super::predation_fixture::*;
use crate::{
    Activity, DT, Departure, Diet, Fauna, FaunaConfig, Founder, HEAL_COST, StartingStores,
    State, StrikePhase, dodge_chance, dodges,
};
use cubarium_voxel_flora::{Flora, FloraConfig, Site, Trophic};

/// The wound one lanternjaw hit gives `prey`, by the note's formula (§2.4).
fn wound_per_hit(fauna: &Fauna, striker: u64, prey: u64) -> f64 {
    let s = animal(fauna, striker);
    let p = animal(fauna, prey);
    let armour = fauna.config.founder(p.founder.expect("a lineage")).armour;
    hunter(fauna).strike_power * (s.body / p.body) * (1.0 - armour)
}

/// A strike world: the plain, an empty plant layer and the hunting table.
fn arena(world_hour: Option<f64>, seed: u64) -> (cubarium_voxel::World, Flora, Fauna) {
    let world = match world_hour {
        Some(h) => at_hour(h, seed),
        None => clock_off(seed),
    };
    (
        world,
        Flora::new(FloraConfig::default()),
        Fauna::new(hunting_config()),
    )
}

/// A lanternjaw at the striker's mark facing `+x`, feed off, and a prey of `founder`
/// with `stores` centred `ahead_m` in front of it, facing `prey_heading`.
fn striker_and_prey(
    fauna: &mut Fauna,
    world: &cubarium_voxel::World,
    founder: Founder,
    stores: StartingStores,
    ahead_m: f64,
    prey_heading: f64,
) -> (u64, u64, std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let prey = introduce(fauna, world, founder, stores);
    place(fauna, prey, STRIKER_X + ahead_m, LANE_Z, prey_heading);
    let lj = introduce(fauna, world, Founder::Lanternjaw, HUNTING);
    place(fauna, lj, STRIKER_X, LANE_Z, EAST);
    let feed = drive(fauna, lj, 0.0, false);
    (lj, prey, feed)
}

// ------------------------------------------------------------------ the tables

/// §2.12 and §2.3's tables are the defaults.
#[test]
fn the_lanternjaw_and_its_prey_carry_the_notes_numbers() {
    let c = FaunaConfig::default();
    let lj = c.founder(Founder::Lanternjaw);
    assert_eq!(Founder::Lanternjaw.name(), "lanternjaw");
    assert_eq!(lj.core.body_max, 0.035);
    assert_eq!(lj.core.maintenance_per_s, 0.0002);
    assert_eq!(lj.core.reserve_cap, 1.0);
    assert_eq!(
        (lj.adult_length_m, lj.adult_width_m, lj.adult_height_m),
        (1.0, 0.25, 0.45)
    );
    assert_eq!(lj.active, Activity::Nocturnal);
    let cruise = Founder::Lanternjaw.manifest().cruise_m_per_s;
    assert!((cruise - 0.6).abs() < 1e-12, "cruise {cruise} m/s, not 0.6 BL/s");
    assert_eq!(Diet::of(Founder::Lanternjaw), Diet::Carrion);
    for t in [Trophic::Photo, Trophic::Saprotroph] {
        assert!(!Diet::Carrion.accepts(t), "a carnivore's mouth takes no {t:?}");
    }

    let h = lj.hunter.expect("the lanternjaw hunts");
    assert_eq!(h.strike_power, 0.7);
    assert_eq!(h.prey_max_ratio, 1.5);
    assert_eq!(h.strike_reach_length_fraction, 0.6);
    assert_eq!(h.jaw_band_height_fraction, 0.6);
    assert_eq!((h.windup_s, h.lunge_s, h.recovery_s), (0.15, 0.25, 3.0));
    assert_eq!(h.strike_cost_s, 100.0);
    assert_eq!((h.rest_at, h.torpor_upkeep, h.wake_at), (0.9, 0.25, 0.5));
    assert!(h.hunts(Founder::Browser) && h.hunts(Founder::Blind));
    assert!(!h.hunts(Founder::Lanternjaw), "not in its own prey list");

    let (grazer, shredder) = (c.founder(Founder::Browser), c.founder(Founder::Blind));
    assert!(grazer.hunter.is_none() && shredder.hunter.is_none());
    assert_eq!((grazer.armour, grazer.agility), (0.25, 0.5));
    assert_eq!((shredder.armour, shredder.agility), (0.0, 0.3));
    for f in Founder::ALL {
        assert!(c.founder(f).counter.is_none(), "{}: the Counter seam is unused", f.name());
    }
}

/// The dodge table (§2.3): awareness by state, aspect, condition clamped to [0.5, 1].
#[test]
fn the_dodge_chance_is_the_tables_product() {
    for (state, awareness) in [
        (State::Walking, 1.0),
        (State::Resting, 0.7),
        (State::Cropping, 0.4),
        (State::Climbing, 0.4),
        (State::Asleep, 0.0),
    ] {
        assert_eq!(state.awareness(), awareness, "{state:?}");
    }
    let eq = |a: f64, b: f64| (a - b).abs() < 1e-12;
    assert!(eq(dodge_chance(0.5, State::Walking, false, 1.0), 0.5));
    assert!(eq(dodge_chance(0.3, State::Walking, false, 1.0), 0.3));
    assert!(eq(dodge_chance(0.5, State::Walking, true, 1.0), 0.25));
    assert!(eq(dodge_chance(0.5, State::Cropping, true, 1.0), 0.1));
    assert!(eq(dodge_chance(0.5, State::Resting, false, 0.8), 0.28));
    // Condition clamps: a starving animal is slow but never slower than half.
    assert!(eq(dodge_chance(0.5, State::Walking, false, 0.1), 0.25));
    assert!(eq(dodge_chance(0.5, State::Walking, false, 1.4), 0.5));
    assert_eq!(dodge_chance(0.5, State::Asleep, false, 1.0), 0.0);
}

// ------------------------------------------------------------------ case 1

/// Case 1: a sleeping adult grazer in the capsule is surely hit (awareness 0) for 0.37,
/// and the hit wakes it. With its agility then set to 0, so that no dodge is possible,
/// the wounds of two more hits add and the third kills. Three lanternjaws take one strike
/// each, one after the other.
#[test]
fn a_sleeping_adult_grazer_is_hit_for_037_and_the_third_hit_kills() {
    let (world, mut flora, mut fauna) = arena(Some(0.0), 1);
    let grazer = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    place(&mut fauna, grazer, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    let mut strikers = Vec::new();
    for _ in 0..3 {
        let id = introduce(&mut fauna, &world, Founder::Lanternjaw, HUNTING);
        place(&mut fauna, id, STRIKER_X, LANE_Z, EAST);
        strikers.push((id, drive(&mut fauna, id, 0.0, false)));
    }
    let per_hit = wound_per_hit(&fauna, strikers[0].0, grazer);
    assert!((per_hit - 0.3675).abs() < 1e-12, "0.7 × 0.7 × 0.75, not {per_hit}");
    assert!((per_hit - 0.37).abs() < 0.005);

    for (k, (_, feed)) in strikers.iter().enumerate() {
        feed.store(true, Ordering::Relaxed);
        let before = hits(&fauna);
        step_until(&mut fauna, &world, &mut flora, 40, |f| hits(f) > before)
            .unwrap_or_else(|| panic!("lanternjaw {k} never hit the sleeping grazer"));
        feed.store(false, Ordering::Relaxed);
        if k == 0 {
            let g = animal(&fauna, grazer);
            assert!((g.wound - 0.37).abs() < 0.005, "the first hit wounds {}", g.wound);
            assert_ne!(g.state, State::Asleep, "a hit wakes it");
            assert_eq!(g.rest, None, "a hit ends its rest");
            assert_eq!(dodged(&fauna), 0, "asleep, it could not dodge");
            // Awake now: take its dodge away so the next two hits are certain.
            fauna.config.founders[Founder::Browser.index()].agility = 0.0;
        }
        if k < 2 {
            let g = animal(&fauna, grazer);
            let want = (k as f64 + 1.0) * per_hit;
            assert!(
                // 1e-3: the ~0.5 s between strikes heals ~1.4e-4 at the note's 0.5 per 30 min.
                (g.wound - want).abs() < 1e-3,
                "after {} hits the wound is {}, not {want}",
                k + 1,
                g.wound
            );
        } else {
            assert!(fauna.view().animal(grazer).is_none(), "the third hit kills");
        }
    }
    assert_eq!(dodged(&fauna), 0, "no dodge was possible");
    assert_eq!(
        fauna
            .ledger
            .departed_founder(Founder::Browser, Departure::Killed { by: Founder::Lanternjaw }),
        1
    );
    assert_closes(&fauna, "after the kill");
}

// ------------------------------------------------------------------ case 2

/// Case 2: a juvenile grazer (0.015, ratio 2.3) and an adult shredder (0.0125, ratio
/// 2.8) each die of one hit. Both are asleep: the grazer at midnight, the nocturnal
/// shredder at noon.
#[test]
fn a_juvenile_grazer_and_an_adult_shredder_die_in_one_hit() {
    for (founder, stores, hour) in [
        (
            Founder::Browser,
            StartingStores {
                body: 0.3,
                reserve: 1.0,
            },
            0.0,
        ),
        (Founder::Blind, StartingStores::FULL, 12.0),
    ] {
        let (world, mut flora, mut fauna) = arena(Some(hour), 2);
        let (lj, prey, feed) =
            striker_and_prey(&mut fauna, &world, founder, stores, IN_CAPSULE_M, WEST);
        let body = animal(&fauna, prey).body;
        assert!(wound_per_hit(&fauna, lj, prey) >= 1.0, "{}", founder.name());
        feed.store(true, Ordering::Relaxed);
        step_until(&mut fauna, &world, &mut flora, 40, |f| hits(f) > 0)
            .unwrap_or_else(|| panic!("the {} (body {body}) was never hit", founder.name()));
        assert_eq!(hits(&fauna), 1);
        assert!(
            fauna.view().animal(prey).is_none(),
            "one hit kills the {}",
            founder.name()
        );
        assert_eq!(
            fauna
                .ledger
                .departed_founder(founder, Departure::Killed { by: Founder::Lanternjaw }),
            1
        );
        assert_closes(&fauna, founder.name());
    }
}

// ------------------------------------------------------------------ case 3

/// Holds `feed` for 60 ticks and returns the strikes begun.
fn strikes_in_60(
    world: &cubarium_voxel::World,
    flora: &mut Flora,
    fauna: &mut Fauna,
    feed: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> u64 {
    feed.store(true, Ordering::Relaxed);
    for _ in 0..60 {
        fauna.step_with(world, flora, 1);
    }
    strikes(fauna)
}

/// Case 3: nothing outside the capsule is struck, a lanternjaw is not struck by a
/// lanternjaw, and a prey over the size window is not struck; the same young striker
/// does strike a prey inside its window at the same place.
#[test]
fn only_prey_inside_the_capsule_and_the_window_is_struck() {
    // Outside the capsule: far ahead, and behind.
    for ahead in [3.0, -IN_CAPSULE_M] {
        let (world, mut flora, mut fauna) = arena(Some(0.0), 3);
        let (_, prey, feed) = striker_and_prey(
            &mut fauna,
            &world,
            Founder::Browser,
            StartingStores::FULL,
            ahead,
            WEST,
        );
        assert_eq!(strikes_in_60(&world, &mut flora, &mut fauna, &feed), 0, "prey at {ahead} m");
        assert_eq!(animal(&fauna, prey).wound, 0.0);
    }

    // Another lanternjaw inside the capsule: not in the prey list.
    let (world, mut flora, mut fauna) = arena(Some(0.0), 3);
    let (_, other, feed) = striker_and_prey(
        &mut fauna,
        &world,
        Founder::Lanternjaw,
        HUNTING,
        IN_CAPSULE_M,
        WEST,
    );
    assert_eq!(strikes_in_60(&world, &mut flora, &mut fauna, &feed), 0);
    assert_eq!(animal(&fauna, other).wound, 0.0);

    // A young lanternjaw: 0.6 of its adult body, so its window is 1.5 × 0.021 = 0.0315.
    let young = StartingStores {
        body: 0.6,
        reserve: 0.6,
    };
    for (prey_stores, struck) in [
        (StartingStores::FULL, false), // an adult grazer, 0.05: over the window
        (
            StartingStores {
                body: 0.3,
                reserve: 1.0,
            },
            true,
        ), // a juvenile grazer, 0.015: inside it
    ] {
        let (world, mut flora, mut fauna) = arena(Some(0.0), 3);
        let prey = introduce(&mut fauna, &world, Founder::Browser, prey_stores);
        let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, young);
        let (lj_body, prey_body) = (animal(&fauna, lj).body, animal(&fauna, prey).body);
        assert!(lj_body >= fauna.config.founder(Founder::Lanternjaw).core.body_min);
        assert_eq!(prey_body > 1.5 * lj_body, !struck);
        place(&mut fauna, lj, STRIKER_X, LANE_Z, EAST);
        // Inside the young body's own capsule: its jaw is at most half its length ahead.
        let length = fauna
            .config
            .founder(Founder::Lanternjaw)
            .body_at(lj_body)
            .length_m;
        place(&mut fauna, prey, STRIKER_X + 0.5 * length + 0.2, LANE_Z, WEST);
        let feed = drive(&mut fauna, lj, 0.0, false);
        let n = strikes_in_60(&world, &mut flora, &mut fauna, &feed);
        assert_eq!(n > 0, struck, "prey body {prey_body}, striker body {lj_body}: {n} strikes");
    }
}

// ------------------------------------------------------------------ cases 4 and 5

/// A strike whose prey is moved far away as soon as the windup is seen, or (`never`)
/// a striker whose prey was far away all along. Returns the layer after `ticks`.
fn missed_strike(never: bool, ticks: u32) -> (cubarium_voxel::World, Flora, Fauna, u64, u64) {
    let (world, mut flora, mut fauna) = arena(Some(0.0), 4);
    let start = if never { 4.0 } else { IN_CAPSULE_M };
    let (lj, prey, feed) = striker_and_prey(
        &mut fauna,
        &world,
        Founder::Browser,
        StartingStores::FULL,
        start,
        WEST,
    );
    feed.store(true, Ordering::Relaxed);
    let mut moved = never;
    for _ in 0..ticks {
        fauna.step_with(&world, &mut flora, 1);
        if !moved && phase(&fauna, lj) == Some(StrikePhase::Windup) {
            place(&mut fauna, prey, STRIKER_X + 4.0, LANE_Z, WEST);
            moved = true;
        }
    }
    assert!(moved, "no windup began");
    (world, flora, fauna, lj, prey)
}

/// Case 5: a prey that leaves the capsule during the windup is missed by geometry:
/// the strike happened, nothing made contact, nothing dodged, no wound.
#[test]
fn a_prey_that_leaves_the_capsule_during_the_windup_is_missed_by_geometry() {
    let (_, _, fauna, lj, prey) = missed_strike(false, 40);
    assert_eq!(strikes(&fauna), 1);
    assert_eq!(hits(&fauna), 0);
    assert_eq!(dodged(&fauna), 0, "no contact, so nothing to dodge");
    assert_eq!(animal(&fauna, prey).wound, 0.0);
    assert_eq!(phase(&fauna, lj), Some(StrikePhase::Recovery));
}

/// Case 4: the strike's cost (100 s of awake upkeep) is paid on that miss, and recovery
/// blocks a second strike for 3 s even with a prey back in the capsule.
#[test]
fn a_miss_pays_the_strike_cost_and_recovery_blocks_a_second_strike_for_3_s() {
    let (_, _, missed, lj, _) = missed_strike(false, 40);
    let (_, _, idle, _, _) = missed_strike(true, 40);
    let body = animal(&missed, lj).body;
    let phys = missed.config.founder(Founder::Lanternjaw);
    let cost = hunter(&missed).strike_cost_s * phys.core.maintenance_per_s * body;
    let paid = missed.ledger.respired_out - idle.ledger.respired_out;
    assert!(
        paid >= cost * (1.0 - 1e-9) && paid <= cost * 1.1,
        "the miss cost {paid} more than standing still; the strike costs {cost}"
    );
    assert_closes(&missed, "after the miss");

    // Recovery: the same miss, then the prey put back in front of it with feed held.
    let (world, mut flora, mut fauna) = arena(Some(0.0), 4);
    let (lj, prey, feed) = striker_and_prey(
        &mut fauna,
        &world,
        Founder::Browser,
        StartingStores::FULL,
        IN_CAPSULE_M,
        WEST,
    );
    feed.store(true, Ordering::Relaxed);
    let recovery_ticks = (hunter(&fauna).recovery_s / DT).round() as u32;
    let (mut moved, mut recovering_since) = (false, None);
    for t in 1..=200u32 {
        fauna.step_with(&world, &mut flora, 1);
        let now = phase(&fauna, lj);
        if !moved && now == Some(StrikePhase::Windup) {
            place(&mut fauna, prey, STRIKER_X + 4.0, LANE_Z, WEST);
            moved = true;
        }
        if recovering_since.is_none() && now == Some(StrikePhase::Recovery) {
            let (x, z) = ahead_of(&fauna, lj, IN_CAPSULE_M);
            place(&mut fauna, prey, x, z, WEST);
            recovering_since = Some(t);
        }
        let Some(since) = recovering_since else {
            continue;
        };
        if t < since + recovery_ticks - 1 {
            assert_eq!(strikes(&fauna), 1, "a second strike {} ticks into recovery", t - since);
            assert_eq!(now, Some(StrikePhase::Recovery));
        } else if strikes(&fauna) == 2 {
            assert!(t <= since + recovery_ticks + 20, "the second strike waited {} ticks", t - since);
            return;
        }
    }
    panic!("no second strike after recovery (recovery began at {recovering_since:?})");
}

// ------------------------------------------------------------------ case 6

/// Case 6, the draw: awake walking prey facing the striker dodge at the table's rate
/// over many seeded draws (grazer 0.5, shredder 0.3), and asleep prey never do.
#[test]
fn walking_prey_facing_the_striker_dodge_at_the_tables_rate() {
    for (agility, want) in [(0.5, 0.5), (0.3, 0.3)] {
        let chance = dodge_chance(agility, State::Walking, false, 1.0);
        let n = 4000u64;
        let dodged = (0..n)
            .filter(|&k| dodges(chance, 1 + k % 50, 7, 11 + k / 50, 1000 + k))
            .count() as f64;
        let rate = dodged / n as f64;
        assert!(
            (rate - want).abs() < 0.03,
            "agility {agility}: dodged {rate} of {n} draws, the table says {want}"
        );
        let asleep = dodge_chance(agility, State::Asleep, false, 1.0);
        assert!((0..n).all(|k| !dodges(asleep, 1 + k, 7, 11, 1000 + k)));
    }
}

/// Case 6, in the world: an awake resting grazer (awareness 0.7) struck head-on dodges
/// about 0.35 of the time over many seeded worlds, struck from behind about 0.175, and
/// a sleeping one never.
#[test]
fn in_the_world_dodges_follow_awareness_and_aspect() {
    let worlds = 200u64;
    let rate = |hour: f64, prey_heading: f64| -> f64 {
        let mut escapes = 0u64;
        for seed in 1..=worlds {
            let (world, mut flora, mut fauna) = arena(Some(hour), seed);
            let (_, prey, feed) = striker_and_prey(
                &mut fauna,
                &world,
                Founder::Browser,
                StartingStores::FULL,
                IN_CAPSULE_M,
                prey_heading,
            );
            feed.store(true, Ordering::Relaxed);
            step_until(&mut fauna, &world, &mut flora, 40, |f| hits(f) + dodged(f) > 0)
                .unwrap_or_else(|| panic!("seed {seed}: no contact"));
            assert_eq!(hits(&fauna) + dodged(&fauna), 1);
            if dodged(&fauna) == 1 {
                assert_eq!(animal(&fauna, prey).wound, 0.0);
                escapes += 1;
            }
        }
        escapes as f64 / worlds as f64
    };
    // Noon: the diurnal grazer is awake and, with no controller, resting.
    let head_on = rate(12.0, WEST);
    let behind = rate(12.0, EAST);
    assert!((head_on - 0.35).abs() < 0.12, "head-on {head_on}, the table says 0.35");
    assert!((behind - 0.175).abs() < 0.095, "from behind {behind}, the table says 0.175");
    assert!(behind < head_on, "behind {behind} vs head-on {head_on}");
    // Midnight: asleep, never.
    assert_eq!(rate(0.0, WEST), 0.0);
}

// ------------------------------------------------------------------ case 7

/// Case 7: a kill departs `Killed`, the whole body goes to carrion on the prey's face
/// with a carcass record, and the books close; two attackers' wounds add; a prey two
/// simultaneous hits would each kill dies once.
#[test]
fn a_kill_departs_killed_leaves_the_whole_body_and_two_wounds_add() {
    // Two lanternjaws strike a sleeping adult grazer together: the wounds add. The first
    // hit wakes it, so its dodge is taken away (agility 0) to make the second certain.
    let (world, mut flora, mut fauna) = arena(Some(0.0), 7);
    fauna.config.founders[Founder::Browser.index()].agility = 0.0;
    let grazer = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    place(&mut fauna, grazer, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    let pair: Vec<u64> = (0..2)
        .map(|_| {
            let id = introduce(&mut fauna, &world, Founder::Lanternjaw, HUNTING);
            place(&mut fauna, id, STRIKER_X, LANE_Z, EAST);
            drive(&mut fauna, id, 0.0, true);
            id
        })
        .collect();
    let per_hit = wound_per_hit(&fauna, pair[0], grazer);
    step_until(&mut fauna, &world, &mut flora, 40, |f| hits(f) >= 2).expect("two hits");
    let w = animal(&fauna, grazer).wound;
    assert!((w - 2.0 * per_hit).abs() < 1e-4, "two hits made a wound of {w}");

    // Already wounded 0.7: either hit alone kills. It dies once.
    let (world, mut flora, mut fauna) = arena(Some(0.0), 7);
    fauna.config.founders[Founder::Browser.index()].agility = 0.0;
    let grazer = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    place(&mut fauna, grazer, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    animal_mut(&mut fauna, grazer).wound = 0.7;
    for _ in 0..2 {
        let id = introduce(&mut fauna, &world, Founder::Lanternjaw, HUNTING);
        place(&mut fauna, id, STRIKER_X, LANE_Z, EAST);
        drive(&mut fauna, id, 0.0, true);
    }
    let mut last = animal(&fauna, grazer);
    let carrion_at = |flora: &Flora, site: Site| {
        flora.view().ground_at(site).map_or(0.0, |g| g.carrion)
    };
    let before = carrion_at(&flora, last.site);
    step_until(&mut fauna, &world, &mut flora, 40, |f| {
        match f.view().animal(grazer) {
            Some(a) => {
                last = *a;
                false
            }
            None => true,
        }
    })
    .expect("the grazer is killed");
    let l = &fauna.ledger;
    assert_eq!(l.deaths, 1, "it dies once");
    assert_eq!(l.departed(Departure::Killed { by: Founder::Lanternjaw }), 1);
    assert_eq!(
        l.departed_founder(Founder::Browser, Departure::Killed { by: Founder::Lanternjaw }),
        1
    );
    assert_eq!(l.deaths_accounted(), l.deaths);
    // The whole body, on its own face, once: what it held a tick before, less at most
    // that tick's upkeep.
    let whole = last.organic() + last.gut.organic;
    let laid = carrion_at(&flora, last.site) - before;
    assert!(
        (laid - whole).abs() <= 1e-3 * whole,
        "carrion rose by {laid} on the prey's face; the body held {whole}"
    );
    let carcass = flora
        .view()
        .ground_at(last.site)
        .and_then(|g| g.carcass)
        .expect("a carcass record on the pool");
    assert_eq!(carcass.lineage, Founder::Browser.index() as u8);
    assert!((carcass.heading_rad - last.pose.heading_rad).abs() < 1e-9);
    assert!((carcass.organic - laid).abs() <= 1e-9 * whole);
    assert_closes(&fauna, "after the kill");
}

// ------------------------------------------------------------------ case 8

/// Case 8: a wound heals at 0.5 per 30 min and the healing is paid from the reserve at
/// `HEAL_COST` per unit wound per unit body; a starving animal does not heal; a wounded
/// animal's speed cap is × (1 − 0.5·wound).
#[test]
fn a_wound_heals_at_the_set_rate_paying_reserve_and_the_speed_cap_follows_it() {
    const TICKS: u32 = 40;
    // Twins with the clock off: one wounded, one whole.
    let run = |wound: f64, reserve: f64| -> Fauna {
        let (world, mut flora, mut fauna) = arena(None, 8);
        let g = introduce(
            &mut fauna,
            &world,
            Founder::Browser,
            StartingStores { body: 1.0, reserve },
        );
        animal_mut(&mut fauna, g).wound = wound;
        for _ in 0..TICKS {
            fauna.step_with(&world, &mut flora, 1);
        }
        fauna
    };
    let wounded = run(0.5, 1.0);
    let whole = run(0.0, 1.0);
    let a = wounded.animals[0];
    let healed = 0.5 - a.wound;
    let want = f64::from(TICKS) * DT * 0.5 / 1800.0;
    assert!(
        (healed - want).abs() < 1e-9,
        "healed {healed} in {TICKS} ticks, the rate gives {want}"
    );
    let paid = wounded.ledger.respired_out - whole.ledger.respired_out;
    let cost = HEAL_COST * healed * a.body;
    assert!(paid > 0.0, "healing is paid");
    assert!(
        (paid - cost).abs() <= 1e-6 * cost,
        "healing respired {paid}, HEAL_COST gives {cost}"
    );
    assert_closes(&wounded, "healing");

    // Starving: no reserve, no healing.
    let starving = run(0.5, 0.0);
    assert_eq!(starving.animals[0].wound, 0.5, "a starving animal does not heal");

    // The speed cap: two grazers walking flat out, one wounded 0.5.
    let (world, mut flora, mut fauna) = arena(None, 8);
    let mut ids = Vec::new();
    for (k, wound) in [0.0, 0.5].into_iter().enumerate() {
        let id = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
        place(&mut fauna, id, 1.0, 0.75 + 1.5 * k as f64, EAST);
        animal_mut(&mut fauna, id).wound = wound;
        drive(&mut fauna, id, 1.0, false);
        ids.push(id);
    }
    for _ in 0..20 {
        fauna.step_with(&world, &mut flora, 1);
    }
    let x0: Vec<f64> = ids.iter().map(|&id| animal(&fauna, id).pose.x).collect();
    for _ in 0..20 {
        fauna.step_with(&world, &mut flora, 1);
    }
    let d: Vec<f64> = ids
        .iter()
        .zip(&x0)
        .map(|(&id, x)| animal(&fauna, id).pose.x - x)
        .collect();
    assert!(d[0] > 0.3, "the whole grazer walked {} m in 1 s", d[0]);
    let ratio = d[1] / d[0];
    assert!(
        (ratio - (1.0 - 0.5 * 0.5)).abs() < 0.03,
        "wounded 0.5 it walked {ratio} of the whole grazer's distance, not 0.75"
    );
}
