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
//! - Switching (D2b): encounters move the search image, which forgets on its timescale; a
//!   lineage whose share of recent encounters falls loses attack preference faster than
//!   its share; a rare prey is accepted less often per encounter than a common one, and a
//!   rare prey in the strike capsule is not struck while the hunter goes for a common one.
//! - Breeding: a well-fed adult can afford a clutch under its package, a hungry one cannot,
//!   and the clutch follows the reserve.
//! - Ledgers conserve through each world case.

use std::sync::atomic::Ordering;

use super::predation_fixture::*;
use crate::{
    Activity, Command, Fauna, FaunaConfig, Founder, HuntConfig, HuntMemory, HuntMode,
    MobilityMode, Pose, StartingStores, accepts, attack_preference, encounter_share,
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

/// Carrion at the mouth: a carcass holds the patroller to eat (the patrol stands back), a
/// smear under the teachers' feed gate does not (it walks on to its next patch).
#[test]
fn a_carcass_holds_the_patroller_and_a_smear_does_not() {
    let hunt = HuntConfig { patrol: true, patrol_explore: 0.0, ..HuntConfig::default() };
    let gate = crate::TeacherConfig::default().feed_gate;
    let mut walked = Vec::new();
    for carrion in [1.0, 1e-9] {
        let world = clock_off(46);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(patrol_config(hunt));
        litter_patch(&mut flora, 9.0, 1.5);
        let lj = patroller(&mut fauna, &world, 4.0, 1.5, EAST, HUNTING);
        // Carrion on the faces around its mouth.
        let (mx, mz) = ahead_of(&fauna, lj, 0.5);
        for dx in -2i32..=2 {
            for dz in -1i32..=1 {
                let site = Site {
                    x: ((mx / VOXEL_M).floor() as i32 + dx) as u32,
                    y: SURFACE_Y,
                    z: ((mz / VOXEL_M).floor() as i32 + dz) as u32,
                };
                assert!(flora.deposit(site, Deposit { kind: DepositKind::Carrion, organic: carrion, mineral: 0.02 * carrion, energy: 2.0 * carrion }));
            }
        }
        let start = animal(&fauna, lj).pose;
        for _ in 0..60 {
            fauna.step_with(&world, &mut flora, 1);
        }
        walked.push(dist(&fauna, lj, (start.x, start.z)));
        assert_closes(&fauna, "after the carrion case");
    }
    assert!(walked[0] < 0.3, "a carcass holds it to eat: it moved {} m", walked[0]);
    assert!(walked[1] > 0.5, "a smear under the feed gate ({gate}) does not: it moved {} m", walked[1]);
}

// ------------------------------------------------------------------ tremor

/// A shredder walking away behind the hunter on its left (out of sight) turns it left; on
/// its right, right. Its contact moves the search image.
#[test]
fn a_tremor_on_one_side_turns_the_hunter_toward_it() {
    for (prey_z, toward_left) in [(2.5, true), (0.5, false)] {
        let hunt = HuntConfig { patrol: true, attack_explore: 1.0, attack_prior_s: 1e9, ..HuntConfig::default() };
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
    // A small juvenile (one strike kills it, so profitability is whole) and a full adult.
    for (prey_stores, inside) in [(StartingStores { body: 0.2, reserve: 1.0 }, true), (StartingStores::FULL, false)] {
        let hunt = HuntConfig { patrol: true, attack_explore: 1.0, attack_prior_s: 1e9, ..HuntConfig::default() };
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
        if inside {
            let armour = fauna.config.founder(Founder::Browser).armour;
            assert_eq!(crate::profitability(&hunter(&fauna), hb, pb, armour), 1.0, "the fixture: one strike kills it");
        }
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
    let bw = bellwing_at(&mut fauna, &world, STRIKER_X + ahead_m, LANE_Z, SURFACE_M + lift_m.min(1.0));
    animal_mut(&mut fauna, bw).pose.y_m = SURFACE_M + lift_m;
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
/// Veilhound (`chorister`); S3 (`is_prey_of`): one **hovering** just over the ground at the
/// same place, inside the jaw band, is struck too; one hovering above the band is not.
#[test]
fn a_grounded_perched_or_low_hovering_bellwing_in_reach_is_struck_and_a_higher_one_is_not() {
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
        let band = h.strike_height_m(&geometry);
        // Hovering inside the jaw band: a quarter of the band's height up.
        let (n, mode) = strikes_on_bellwing(hunter_lineage, 0.25 * band, ahead);
        assert_eq!(mode, MobilityMode::Fly, "the fixture has it flying, {} m up", 0.25 * band);
        assert!(n > 0, "{}: a bellwing hovering in the jaw band is struck", hunter_lineage.name());
        // Hovering a band's height over the band's top.
        let (n, mode) = strikes_on_bellwing(hunter_lineage, 2.0 * band, ahead);
        assert_eq!(mode, MobilityMode::Fly, "the fixture has it flying, {} m up", 2.0 * band);
        assert_eq!(n, 0, "{}: a bellwing hovering over the jaw band is not struck", hunter_lineage.name());
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

/// The search image is moved by encounters and forgets on its configured timescale:
/// contact with a lineage raises its memory and leaves the others'; with no contact it
/// decays by e over [`HuntConfig::search_memory_s`], whatever that is set to.
#[test]
fn encounters_move_the_search_image_and_it_forgets_on_its_timescale() {
    for memory_s in [300.0, 2000.0] {
        let cfg = HuntConfig { search_memory_s: memory_s, ..HuntConfig::default() };
        let dt_s = 0.25;
        let ticks_per_sample = (dt_s / crate::DT).round() as u64;
        let mut m = HuntMemory::default();
        let mut met = [false; Founder::COUNT];
        met[Founder::Blind.index()] = true;
        for k in 1..=40u64 {
            m.meet(&cfg, k * ticks_per_sample, &met, dt_s);
        }
        let (s, cap) = (Founder::Blind.index(), Founder::Capgnawer.index());
        assert!(m.image[s] > 0.0, "contact is remembered");
        assert_eq!(m.image[cap], 0.0, "a lineage never met is not");
        for (fold, want) in [(0.5, (-0.5f64).exp()), (1.0, (-1.0f64).exp()), (3.0, (-3.0f64).exp())] {
            let mut later = m;
            let t = m.image_tick + (fold * memory_s / crate::DT).round() as u64;
            later.meet(&cfg, t, &[false; Founder::COUNT], dt_s);
            let ratio = later.image[s] / m.image[s];
            assert!((ratio - want).abs() < 1e-6, "memory {memory_s} s: after {fold} e-folds it keeps {ratio}, not {want}");
        }
    }
}

/// The share-switching table: switching on, the given exponent and prior.
fn switching(exponent: f64, prior_s: f64) -> HuntConfig {
    HuntConfig { share_switching: true, switch_exponent: exponent, attack_prior_s: prior_s, ..HuntConfig::default() }
}

/// **Switching on encounter share**: a lineage whose share of the recent eligible
/// encounters falls, while another stays common, loses attack preference faster than its
/// share (for any exponent above 1); the preference stays in `[0, 1]`, the most-met
/// lineage's is the highest, and with the prior faded a lineage never met goes toward 0
/// (no floor). With nothing met lately every listed lineage is attacked alike.
#[test]
fn a_lineage_whose_share_falls_loses_preference_faster_than_its_share() {
    let h = FaunaConfig::default().founder(LJ).hunter.expect("a hunter");
    let (shred, graze) = (Founder::Blind, Founder::Browser);
    assert!(h.hunts(shred) && h.hunts(graze), "the fixture: both on its list");
    for (exponent, prior) in [(1.5, 0.0), (2.0, 10.0), (3.0, 30.0)] {
        let cfg = switching(exponent, prior);
        let mut image = [0.0; Founder::COUNT];
        image[graze.index()] = 200.0;
        let mut last: Option<(f64, f64)> = None;
        for m in [200.0, 120.0, 60.0, 20.0, 5.0, 0.5] {
            image[shred.index()] = m;
            let share = encounter_share(&cfg, &h, &image, shred);
            let p = attack_preference(&cfg, &h, &image, shred);
            assert!((0.0..=1.0).contains(&p), "bounded: {p}");
            assert!(p <= attack_preference(&cfg, &h, &image, graze) + 1e-12, "the most-met lineage is preferred most");
            let total: f64 = Founder::ALL.iter().map(|&f| encounter_share(&cfg, &h, &image, f)).sum();
            assert!((total - 1.0).abs() < 1e-9, "the shares over its list sum to 1: {total}");
            if let Some((s0, p0)) = last {
                assert!(share < s0, "the fixture: its share falls");
                assert!(p / p0 < share / s0, "γ {exponent}: preference {p0} → {p} falls faster than share {s0} → {share}");
            }
            last = Some((share, p));
        }
        // No floor: the prior fades as the common lineage's memory grows.
        image[shred.index()] = 0.0;
        image[graze.index()] = 1e6;
        assert!(attack_preference(&cfg, &h, &image, shred) < 1e-6, "a lineage unmet beside a dominant one goes toward 0");
        // Nothing met: every lineage on its list alike, and at the top.
        let none = [0.0; Founder::COUNT];
        for f in Founder::ALL.iter().copied().filter(|&f| h.hunts(f)) {
            assert_eq!(attack_preference(&cfg, &h, &none, f), 1.0, "nothing met lately: {} at 1", f.name());
        }
    }
}

/// **A rare prey met alongside a common one is attacked less often per encounter** than
/// when it was common: the same prey bodies, the same decision windows, the hunter's image
/// in two states — the capgnawer common (grazers rare), then rare (grazers common). The
/// acceptance rate over many encounters falls with its share.
#[test]
fn a_rare_prey_is_accepted_less_often_per_encounter_than_a_common_one() {
    let world = clock_off(46);
    let mut fauna = Fauna::new(patrol_config(switching(2.0, 5.0)));
    let lj = introduce(&mut fauna, &world, LJ, HUNTING);
    let gnawer = introduce(&mut fauna, &world, Founder::Capgnawer, StartingStores::FULL);
    let (cap, graze) = (Founder::Capgnawer.index(), Founder::Browser.index());
    let rate = |cap_s: f64, graze_s: f64| -> f64 {
        let mut a = animal(&fauna, lj);
        a.hunt.image = [0.0; Founder::COUNT];
        a.hunt.image[cap] = cap_s;
        a.hunt.image[graze] = graze_s;
        let mut o = animal(&fauna, gnawer);
        let window = cubarium_voxel::ticks_at_least_one(fauna.config.hunt.attack_decision_s) as u64;
        let mut yes = 0u32;
        let n = 4000u32;
        for k in 0..n {
            o.id = 1000 + u64::from(k % 40); // forty prey bodies of the lineage
            if accepts(&fauna.config, &a, &o, u64::from(k / 40) * window) {
                yes += 1;
            }
        }
        f64::from(yes) / f64::from(n)
    };
    let common = rate(200.0, 20.0);
    let even = rate(100.0, 100.0);
    let rare = rate(20.0, 200.0);
    assert!(common > 0.0, "the fixture: a common one is attacked");
    assert!(rare < even && even <= common, "per-encounter acceptance falls with its share: common {common}, even {even}, rare {rare}");
    assert!(rare < 0.5 * common, "and a rare one is passed over: {rare} against {common}");
}

/// A rare prey in the strike capsule is not struck while the hunter goes for a common one
/// it sees beyond it ([`HuntConfig::strike_chosen_only`]); the same prey when common is.
/// With the switch off, the rare one standing in reach is struck anyway (D2's
/// opportunistic strike), so the case tests the capsule, not the fixture.
#[test]
fn a_rare_prey_in_reach_is_not_struck_while_the_hunter_goes_for_a_common_one() {
    let (cap, shred) = (Founder::Capgnawer, Founder::Blind);
    // (capgnawer common?, strike_chosen_only) → strikes on the capgnawer, engaged samples
    let case = |cap_common: bool, chosen_only: bool| -> (u64, u64) {
        let hunt = HuntConfig { strike_chosen_only: chosen_only, ..switching(2.0, 0.0) };
        let world = clock_off(47);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(patrol_config(hunt));
        let gnawer = introduce(&mut fauna, &world, cap, StartingStores::FULL);
        place(&mut fauna, gnawer, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
        drive(&mut fauna, gnawer, 0.0, false);
        let shredder = introduce(&mut fauna, &world, shred, StartingStores::FULL);
        place(&mut fauna, shredder, STRIKER_X + 1.5, LANE_Z + 0.5, WEST);
        drive(&mut fauna, shredder, 0.0, false);
        let lj = patroller(&mut fauna, &world, STRIKER_X, LANE_Z, EAST, HUNTING);
        let tick = fauna.tick;
        let a = animal_mut(&mut fauna, lj);
        a.hunt.image = [0.0; Founder::COUNT];
        let (common, rare) = if cap_common { (cap, shred) } else { (shred, cap) };
        a.hunt.image[common.index()] = 1e6;
        a.hunt.image[rare.index()] = 0.0;
        a.hunt.image_tick = tick;
        a.eating = true; // hungry from the first sample
        let (h, body) = (hunter(&fauna), animal(&fauna, lj).body);
        for (id, f) in [(gnawer, cap), (shredder, shred)] {
            let armour = fauna.config.founder(f).armour;
            assert_eq!(crate::profitability(&h, body, animal(&fauna, id).body, armour), 1.0, "the fixture: one strike kills a {}", f.name());
        }
        for _ in 0..60 {
            fauna.step_with(&world, &mut flora, 1);
        }
        assert_closes(&fauna, "after the capsule case");
        (fauna.ledger.strikes_on_by_founder[cap.index()], fauna.ledger.hunt_engaged_samples)
    };
    let (struck, _) = case(true, true);
    assert!(struck > 0, "a common capgnawer in reach is struck");
    let (struck, engaged) = case(false, true);
    assert!(engaged > 0, "the fixture: it is engaged on the shredder, feed held");
    assert_eq!(struck, 0, "a rare capgnawer in reach is passed over");
    let (struck, _) = case(false, false);
    assert!(struck > 0, "with the switch off, the rare one in reach is struck anyway");
}

/// Profitability follows the wound formula on the config's own table: a prey one strike
/// kills is worth a whole attack, one that takes several hits proportionally less, and
/// armour lowers it.
#[test]
fn a_prey_that_takes_several_hits_is_worth_less() {
    let c = FaunaConfig::default();
    let h = c.founder(LJ).hunter.expect("a hunter");
    let body = c.founder(LJ).core.body_max;
    let one_hit = body * h.strike_power; // a prey this size takes exactly one hit
    assert_eq!(crate::profitability(&h, body, 0.5 * one_hit, 0.0), 1.0);
    let three = crate::profitability(&h, body, 3.0 * one_hit, 0.0);
    assert!((three - 1.0 / 3.0).abs() < 1e-12, "three hits: a third ({three})");
    assert!(crate::profitability(&h, body, 3.0 * one_hit, 0.5) < three, "armour lowers it");
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

/// Reach applies to clinging and perched seedporters as well as grounded prey.
/// The same height gate controls prey sensing and the physical strike capsule.
#[test]
fn a_seedporter_below_the_veilhounds_reach_is_struck_and_one_above_is_safe() {
    for mode in [MobilityMode::Perched, MobilityMode::Climb] {
        for high in [false, true] {
            let world = plain_of_height(
                cubarium_voxel::weather::DayClock {
                    mode: cubarium_voxel::weather::DayMode::Off,
                    ..Default::default()
                },
                54,
                40,
            );
            let mut cfg = patrol_config(HuntConfig::default());
            cfg.founders[Founder::Chorister.index()]
                .hunter
                .as_mut()
                .unwrap()
                .vertical_reach_m = 2.0;
            let phys = *cfg.founder(Founder::Chorister);
            let body = phys.adult_body();
            let h = phys.hunter.unwrap();
            let reach = h.strike_height_m(&body);
            let lift = if high {
                reach + 2.0 * VOXEL_M
            } else {
                0.75 * reach
            };
            let mut fc = FloraConfig::default();
            fc.vaulttree.crown_height_m = [lift, lift];
            let wood = fc.vaulttree.wood_max;
            let mut flora = Flora::new(fc);
            let ahead = (h.jaw_forward_length_fraction + 0.5 * h.strike_reach_length_fraction)
                * body.length_m;
            let x = STRIKER_X + ahead;
            let site = Site {
                x: (x / VOXEL_M).floor() as u32,
                y: SURFACE_Y,
                z: (LANE_Z / VOXEL_M).floor() as u32,
            };
            assert!(flora.apply(
                &world,
                cubarium_voxel_flora::Command::Seed {
                    x: i64::from(site.x),
                    z: site.z,
                    species: cubarium_voxel_flora::Species::Vaulttree,
                    wood,
                }
            ));
            let top = flora
                .view()
                .layers_at(site)
                .filter(|l| l.foliage_index.is_some())
                .max_by_key(|l| l.cell)
                .unwrap();
            let mut fauna = Fauna::new(cfg);
            let prey = introduce(
                &mut fauna,
                &world,
                Founder::Seedporter,
                StartingStores::FULL,
            );
            place(&mut fauna, prey, x, LANE_Z, WEST);
            drive(&mut fauna, prey, 0.0, false);
            let a = animal_mut(&mut fauna, prey);
            a.pose.y_m = if mode == MobilityMode::Perched {
                (top.cell + 1) as f64 * VOXEL_M
            } else {
                SURFACE_M + lift
            };
            a.mobility.mode = mode;
            a.mobility.perch = Some(crate::Perch {
                kind: crate::PerchKind::CrownTop,
                anchor: crate::PerchAnchor::Stand {
                    site,
                    layer: top.foliage_index.unwrap() as u8,
                },
            });
            let hunter = introduce(&mut fauna, &world, Founder::Chorister, HUNTING);
            place(&mut fauna, hunter, STRIKER_X, LANE_Z, EAST);
            animal_mut(&mut fauna, hunter).pose.y_m = SURFACE_M;
            let feed = drive(&mut fauna, hunter, 0.0, false);
            feed.store(true, Ordering::Relaxed);
            assert_eq!(
                crate::is_prey_of(
                    &fauna.config,
                    &animal(&fauna, hunter),
                    &animal(&fauna, prey)
                ),
                !high,
                "{mode:?}: sensing agrees with the configured strike height"
            );
            for _ in 0..60 {
                fauna.step_with(&world, &mut flora, 1);
            }
            assert_eq!(
                fauna.ledger.strikes_on_by_founder[Founder::Seedporter.index()] > 0,
                !high,
                "{mode:?}: the strike agrees with sensing"
            );
            assert_closes(&fauna, "after the seedporter reach case");
        }
    }
}
