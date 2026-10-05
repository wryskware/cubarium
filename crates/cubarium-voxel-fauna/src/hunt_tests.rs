//! **The detritus web, package D2: the lanternjaw hunts the booming small prey**
//! (`design/lanternjaw-detritus-web-proposal-2026-10-05.md` §3.2–3.4 with Astra's review,
//! `design/7_Research/astra-detritus-web-review-2026-10-05.md`), written before the
//! mechanisms. A child of `step`, so a case can reach the controller table and the layer's
//! own fields.
//!
//! Every case checks a mechanism on hand-built inputs **against the config it sets**: no
//! default, ratio or population is asserted.
//!
//! - Patrol: after a fruitless wait a hunter leaves its patch, and does not go back to a
//!   remembered fruitless patch while another cue is open.
//! - Tremor: a prey's tremor on one side turns the hunter toward that side.
//! - The size window conditions the choice: a prey outside it is not chosen.
//! - The diet: a grounded or perched bellwing in reach is struck by the lanternjaw and by
//!   the Veilhound, a flying one is not; a ripple-snail in reach is struck.
//! - Switching: encounters move the search image, and the attack preference falls as the
//!   eligible prey get scarce.
//! - Breeding: a well-fed adult can afford a clutch under its package, a hungry one cannot,
//!   and the clutch follows the reserve.
//! - Ledgers conserve through each world case.

use std::sync::atomic::Ordering;

use super::predation_fixture::*;
use crate::{
    Activity, Command, Fauna, FaunaConfig, Founder, HuntConfig, HuntMemory, HuntMode,
    MobilityMode, Pose, StartingStores, attack_preference,
};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site};

const LJ: Founder = Founder::Lanternjaw;

/// The hunting table with the given patrol numbers, every lineage awake at every hour.
fn patrol_config(hunt: HuntConfig) -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.hunt = hunt;
    c
}

/// A lanternjaw on its own heuristic at `(x, z)` facing `heading`.
fn patroller(fauna: &mut Fauna, world: &World, x: f64, z: f64, heading: f64, stores: StartingStores) -> u64 {
    let id = introduce(fauna, world, LJ, stores);
    place(fauna, id, x, z, heading);
    let teachers = fauna.config.teachers;
    fauna.controllers.set(id, crate::controller::heuristic(LJ, teachers));
    id
}

/// Litter on the 2 × 2 cells around `(x_m, z_m)`.
fn litter_patch(flora: &mut Flora, x_m: f64, z_m: f64) {
    let (cx, cz) = ((x_m / VOXEL_M).round() as u32, (z_m / VOXEL_M).round() as u32);
    for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        assert!(flora.deposit(
            Site { x: cx + dx - 1, y: SURFACE_Y, z: cz + dz - 1 },
            Deposit { kind: DepositKind::Litter, organic: 1.0, mineral: 0.02, energy: 2.0 },
        ));
    }
}

fn dist(fauna: &Fauna, id: u64, (x, z): (f64, f64)) -> f64 {
    let p = animal(fauna, id).pose;
    ((p.x - x).powi(2) + (p.z - z).powi(2)).sqrt()
}

// ------------------------------------------------------------------ patrol

/// Three litter patches on a line, A, B and C, 3 m apart; the hunter waits on A. After the
/// configured fruitless wait it leaves A for the nearer open patch, B; after waiting there
/// it goes on to C, which is as near as A, and does not come back within the fruitless
/// radius of A on the way.
#[test]
fn a_fruitless_wait_leaves_the_patch_and_skips_a_remembered_one() {
    let hunt = HuntConfig {
        patrol: true,
        patch_wait_s: 1.0,
        patrol_explore: 0.0,
        patrol_give_up_s: 30.0,
        ..HuntConfig::default()
    };
    let world = clock_off(41);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(patrol_config(hunt));
    let (a, b, c) = ((2.5, 1.5), (5.5, 1.5), (8.5, 1.5));
    for (x, z) in [a, b, c] {
        litter_patch(&mut flora, x, z);
    }
    let lj = patroller(&mut fauna, &world, a.0, a.1, EAST, HUNTING);
    animal_mut(&mut fauna, lj).hunt = HuntMemory { mode: HuntMode::Wait, patch: a, ..HuntMemory::default() };

    let left_a = step_until(&mut fauna, &world, &mut flora, 200, |f| dist(f, lj, a) > hunt.fruitless_radius_m)
        .expect("after a fruitless wait it leaves its patch");
    assert!(
        left_a as f64 * crate::DT >= hunt.patch_wait_s,
        "not before the wait is out: left after {left_a} ticks"
    );
    step_until(&mut fauna, &world, &mut flora, 400, |f| dist(f, lj, b) <= hunt.patch_arrive_m + 0.25)
        .expect("it goes on to the nearer open patch, B");
    let mut back_at_a = false;
    let reached_c = step_until(&mut fauna, &world, &mut flora, 600, |f| {
        back_at_a |= dist(f, lj, a) <= hunt.fruitless_radius_m;
        dist(f, lj, c) <= hunt.patch_arrive_m + 0.25
    });
    assert!(!back_at_a, "it does not return to A, remembered fruitless, while C is open");
    assert!(reached_c.is_some(), "after B's wait it goes to C");
    assert!(fauna.ledger.hunt_fruitless >= 2, "A and B were left fruitless");
    assert_closes(&fauna, "after the patrol");
}

// ------------------------------------------------------------------ tremor

/// A shredder walking away behind the hunter on its left (out of sight) turns it left; on
/// its right, right. Its contact moves the search image.
#[test]
fn a_tremor_on_one_side_turns_the_hunter_toward_it() {
    for (prey_z, toward_left) in [(2.5, true), (0.5, false)] {
        let hunt = HuntConfig { patrol: true, attack_explore: 1.0, ..HuntConfig::default() };
        let world = clock_off(42);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(patrol_config(hunt));
        let prey = introduce(&mut fauna, &world, Founder::Blind, StartingStores::FULL);
        // Walking away behind it, so it stays out of sight.
        place(&mut fauna, prey, 5.6, prey_z, WEST);
        drive(&mut fauna, prey, 1.0, false);
        // Walking before the hunter arrives, so its first sample feels it.
        for _ in 0..6 {
            fauna.step_with(&world, &mut flora, 1);
        }
        // Facing +x, the left is +z.
        let lj = patroller(&mut fauna, &world, 6.0, 1.5, EAST, HUNTING);
        let mut turns = Vec::new();
        for _ in 0..12 {
            fauna.step_with(&world, &mut flora, 1);
            turns.push(animal(&fauna, lj).founder_state.held.turn);
        }
        let felt = turns.iter().filter(|&&t| if toward_left { t < 0.0 } else { t > 0.0 }).count();
        assert!(felt > 0, "prey at z {prey_z}: the hunter turns toward it ({turns:?})");
        assert!(
            turns.iter().all(|&t| if toward_left { t <= 0.0 } else { t >= 0.0 }),
            "and never away ({turns:?})"
        );
        let h = animal(&fauna, lj).hunt;
        assert!(h.image[Founder::Blind.index()] > 0.0, "the contact is in its search image");
        assert!(fauna.ledger.hunt_encounter_s_by_prey[Founder::Blind.index()] > 0.0);
        assert_closes(&fauna, "after the tremor case");
    }
}

// ------------------------------------------------------------------ size window

/// A still prey ahead on the left within sight: a juvenile grazer inside a young hunter's
/// window engages it and turns it left; an adult grazer outside the window, at the same
/// place, is not chosen.
#[test]
fn a_prey_outside_the_size_window_is_not_chosen() {
    let young = StartingStores { body: 0.6, reserve: 0.6 };
    for (prey_stores, inside) in [(StartingStores { body: 0.3, reserve: 1.0 }, true), (StartingStores::FULL, false)] {
        let hunt = HuntConfig { patrol: true, attack_explore: 1.0, ..HuntConfig::default() };
        let world = clock_off(43);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(patrol_config(hunt));
        let prey = introduce(&mut fauna, &world, Founder::Browser, prey_stores);
        place(&mut fauna, prey, 7.3, 2.4, WEST);
        drive(&mut fauna, prey, 0.0, false);
        let lj = patroller(&mut fauna, &world, 6.0, 1.5, EAST, young);
        let (pb, hb) = (animal(&fauna, prey).body, animal(&fauna, lj).body);
        let window = hunter(&fauna).prey_max_ratio * hb;
        assert_eq!(pb <= window, inside, "the fixture: prey {pb} against window {window}");
        let mut turns = Vec::new();
        for _ in 0..8 {
            fauna.step_with(&world, &mut flora, 1);
            turns.push(animal(&fauna, lj).founder_state.held.turn);
        }
        let engaged = fauna.ledger.hunt_engaged_samples;
        if inside {
            assert!(engaged > 0, "an eligible prey in sight is engaged");
            assert!(turns.iter().any(|&t| t < 0.0), "and faced, to the left ({turns:?})");
        } else {
            assert_eq!(engaged, 0, "a prey over the window is not chosen");
            assert_eq!(fauna.ledger.hunt_encounter_s_by_prey[Founder::Browser.index()], 0.0, "nor met");
        }
        assert_closes(&fauna, "after the window case");
    }
}

// ------------------------------------------------------------------ the diet

/// Put a bellwing at `(x, z)` with its base `y_m` metres up.
fn bellwing_at(fauna: &mut Fauna, world: &World, x: f64, z: f64, y_m: f64) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft {
            pose: Pose { x, z, heading_rad: WEST, y_m },
            founder: Founder::Bellwing,
            stores: StartingStores::FULL,
        },
    ));
    fauna.view().ledger.births - 1
}

/// A striker of `hunter_lineage` at the mark facing `+x` holding feed, and a bellwing
/// `ahead_m` in front of it at `lift_m` over the ground: the strikes begun on it in 60 ticks.
fn strikes_on_bellwing(hunter_lineage: Founder, lift_m: f64, ahead_m: f64) -> (u64, MobilityMode) {
    let world = clock_off(44);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(patrol_config(HuntConfig::default()));
    let bw = bellwing_at(&mut fauna, &world, STRIKER_X + ahead_m, LANE_Z, SURFACE_M + lift_m);
    let mode = animal(&fauna, bw).mobility.mode;
    let striker = introduce(&mut fauna, &world, hunter_lineage, HUNTING);
    place(&mut fauna, striker, STRIKER_X, LANE_Z, EAST);
    let feed = drive(&mut fauna, striker, 0.0, false);
    feed.store(true, Ordering::Relaxed);
    for _ in 0..60 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert_closes(&fauna, "after the bellwing case");
    (fauna.ledger.strikes_on_by_founder[Founder::Bellwing.index()], mode)
}

/// A bellwing perched on the ground in the capsule is struck by the lanternjaw and by the
/// Veilhound (`chorister`); one flying just over the ground at the same place, inside the
/// jaw band, is not.
#[test]
fn a_grounded_or_perched_bellwing_in_reach_is_struck_and_a_flying_one_is_not() {
    let c = FaunaConfig::default();
    for hunter_lineage in [LJ, Founder::Chorister] {
        let phys = c.founder(hunter_lineage);
        let h = phys.hunter.expect("a hunter");
        let geometry = phys.body_at(phys.core.body_max);
        // The capsule's middle: the jaw plus half the reach.
        let ahead = (h.jaw_forward_length_fraction + 0.5 * h.strike_reach_length_fraction) * geometry.length_m;
        let (n, mode) = strikes_on_bellwing(hunter_lineage, 0.0, ahead);
        assert_eq!(mode, MobilityMode::Perched, "the fixture perches it on the ground");
        assert!(n > 0, "{}: a perched bellwing in reach is struck", hunter_lineage.name());
        // Flying inside the jaw band: half the band's height up.
        let lift = 0.25 * h.jaw_band_height_fraction * geometry.height_m;
        let (n, mode) = strikes_on_bellwing(hunter_lineage, lift, ahead);
        assert_eq!(mode, MobilityMode::Fly, "the fixture has it flying, {lift} m up");
        assert_eq!(n, 0, "{}: a flying bellwing is never struck", hunter_lineage.name());
    }
}

/// A ripple-snail in the lanternjaw's capsule is struck.
#[test]
fn a_ripple_snail_in_reach_is_struck() {
    let world = clock_off(45);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(patrol_config(HuntConfig::default()));
    let snail = introduce(&mut fauna, &world, Founder::RippleSnail, StartingStores::FULL);
    place(&mut fauna, snail, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    let lj = introduce(&mut fauna, &world, LJ, HUNTING);
    place(&mut fauna, lj, STRIKER_X, LANE_Z, EAST);
    let feed = drive(&mut fauna, lj, 0.0, false);
    feed.store(true, Ordering::Relaxed);
    for _ in 0..60 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert!(
        fauna.ledger.strikes_on_by_founder[Founder::RippleSnail.index()] > 0,
        "a snail in reach is struck"
    );
    assert_closes(&fauna, "after the snail case");
}

// ------------------------------------------------------------------ switching

/// The search image is moved by encounters: contact with a lineage raises its memory and
/// its attack preference, and leaves the others'; with no contact it decays over the
/// memory's e-fold time. The preference is bounded by the exploration floor and 1, rises
/// with contact, and a scarce prey (met a tenth as often over the same time) gets a lower
/// preference than a common one.
#[test]
fn encounters_move_the_search_image_and_preference_falls_with_scarcity() {
    let cfg = HuntConfig::default();
    let dt_s = 0.25;
    let samples = 400u64;
    let ticks_per_sample = (dt_s / crate::DT).round() as u64;
    let run = |every: u64| -> HuntMemory {
        let mut m = HuntMemory::default();
        let mut met = [false; Founder::COUNT];
        for k in 1..=samples {
            met[Founder::Blind.index()] = k % every == 0;
            m.meet(&cfg, k * ticks_per_sample, &met, dt_s);
        }
        m
    };
    let common = run(1);
    let scarce = run(10);
    let (s, cap) = (Founder::Blind.index(), Founder::Capgnawer.index());
    assert!(common.image[s] > 0.0 && scarce.image[s] > 0.0, "contact is remembered");
    assert_eq!(common.image[cap], 0.0, "a lineage never met is not");
    assert!(scarce.image[s] < common.image[s]);
    let (p_common, p_scarce) = (attack_preference(&cfg, common.image[s]), attack_preference(&cfg, scarce.image[s]));
    assert!(p_scarce < p_common, "the scarce prey is attacked less readily: {p_scarce} vs {p_common}");
    assert_eq!(attack_preference(&cfg, 0.0), cfg.attack_explore, "an unmet prey gets the exploration floor");
    let mut last = 0.0;
    for k in 0..50 {
        let p = attack_preference(&cfg, f64::from(k) * 10.0);
        assert!((cfg.attack_explore..=1.0).contains(&p) && p >= last, "bounded and rising: {p}");
        last = p;
    }
    // A memory's e-fold time with no contact.
    let mut m = common;
    let before = m.image[s];
    let t = m.image_tick + (cfg.search_memory_s / crate::DT).round() as u64;
    m.meet(&cfg, t, &[false; Founder::COUNT], dt_s);
    let ratio = m.image[s] / before;
    assert!((ratio - (-1.0f64).exp()).abs() < 1e-6, "decays by e over the memory: {ratio}");
}

// ------------------------------------------------------------------ breeding

/// Under the lanternjaw's package: a well-fed adult (its reserve at its cap) is in
/// surplus and can afford its full clutch; a hungry one (its reserve under one egg and the
/// floor) cannot lay; between them the clutch is what the reserve above the floor pays for.
#[test]
fn a_well_fed_adult_can_afford_a_clutch_and_a_hungry_one_cannot() {
    let c = FaunaConfig::default();
    let core = c.founder(LJ).core;
    let r = core.reproduction;
    let full = core.reserve_of(core.body_max);
    assert!(r.egg_organic > 0.0);
    assert!(full >= r.surplus_reserve(&core), "a full adult is in surplus: {full} against {}", r.surplus_reserve(&core));
    let most = if r.clutch_max > 0 { r.clutch_max } else { r.clutch_size };
    assert_eq!(r.clutch_for(full), most, "and its reserve pays for the whole clutch");
    let hungry = r.surplus_floor + 0.5 * r.egg_organic;
    assert!(hungry < r.surplus_reserve(&core), "under one egg and the floor it is not in surplus");
    assert_eq!(r.clutch_for(hungry), if r.clutch_max > 0 { 0 } else { r.clutch_size });
    // Moderately fed: one and a half eggs above the floor lays one egg.
    let moderate = r.surplus_floor + 1.5 * r.egg_organic;
    assert!(moderate >= r.surplus_reserve(&core), "one affordable egg makes it eligible");
    assert_eq!(r.clutch_for(moderate), 1, "and it lays what its surplus pays for");
}
