//! **HUNTER-MEAL** (Astra's open questions §3, 2026-10-05): a hunter eats what it kills,
//! and patrols where it can go. Written before the rule. A child of `step` beside the
//! predation files, sharing their fixture.
//!
//! Every case checks a mechanism on hand-built inputs against the config it sets; no
//! default, ratio or population is asserted.
//!
//! - A hunter that kills a prey in reach eats from that corpse until its stomach is full or
//!   the carcass is gone: what the carcass loses its mouths took, the stomach gains that less
//!   the spill, and the ledger closes. The kill marks its place productive.
//! - A hungry hunter walks to a usable carcass out of its mouth's reach and eats it (with the
//!   meal rule off, main's rule, it does not).
//! - A hunter whose mouth is over a usable carcass does not strike a prey in its capsule,
//!   hungry (it eats) or sated.
//! - The patrol never picks a patch it cannot walk to (a ledge above its step), even
//!   exploring; and it prefers a patch by a remembered kill.
//! - The Veilhound (`chorister`) between its pack's hunts walks to a usable carcass and eats.
//!
//! The trace (run by name, `--nocapture`) follows one kill tick by tick: strike → kill →
//! corpse → what the controller does next → mouth contact → gut.

use super::predation_fixture::*;
use crate::{Activity, Command, Fauna, FaunaConfig, Founder, HuntConfig, StartingStores};
use cubarium_voxel::{Command as WorldCommand, Material, World};
use cubarium_voxel_flora::{Carcass, Deposit, DepositKind, Flora, FloraConfig, Site};

const LJ: Founder = Founder::Lanternjaw;

/// Every lineage awake, the given patrol numbers, and no dodge for `prey`.
fn meal_config(hunt: HuntConfig, prey: Founder) -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[prey.index()].agility = 0.0;
    c.hunt = hunt;
    c
}

fn carrion_total(flora: &Flora) -> f64 {
    flora.view().ground.iter().map(|g| g.carrion).sum()
}

/// One tick's line of the trace.
fn trace_line(fauna: &Fauna, world: &World, flora: &Flora, hunter: u64, prey: u64, death: Option<Site>) -> String {
    let Some(a) = fauna.view().animal(hunter).copied() else {
        return "hunter gone".into();
    };
    let phys = *fauna.config.founder(a.founder.unwrap());
    let geometry = phys.body_at(a.body);
    let view = world.view();
    let fv = flora.view();
    let cols = crate::body::mouth_columns(&view, &a.pose, &geometry);
    let mouth = crate::body::mouth_pool_site(&fv, &cols, a.site.y, crate::Food::Carrion);
    let at_death = death.and_then(|s| fv.ground_at(s)).map_or(0.0, |g| g.carrion);
    format!(
        "t{:>4} x={:.3} hd={:.2} strike={:?} prey_alive={} held=({:.2},{:.2},{:.2}) mode={:?} at_carcass={} eating={} stomach={:.5} reserve={:.4} eaten_carrion={:.5} carrion@death={:.5} mouth_carrion={:?}",
        fauna.tick,
        a.pose.x,
        a.pose.heading_rad,
        a.strike.map(|s| s.phase),
        fauna.view().animal(prey).is_some(),
        a.founder_state.held.forward,
        a.founder_state.held.turn,
        a.founder_state.held.feed,
        a.hunt.mode,
        fauna.controllers.at_carcass(hunter),
        a.eating,
        a.stomach.organic(),
        a.reserve,
        fauna.ledger.eaten_carrion_by_founder[a.founder.unwrap().index()],
        at_death,
        mouth.map(|(s, c)| (s.x, s.z, (c * 1e5).round() / 1e5)),
    )
}

/// The trace for one hunter lineage on its own heuristic, a prey of `prey` in its capsule.
fn trace(hunter_founder: Founder, prey_founder: Founder, ticks: u32) {
    let hunt = HuntConfig { patrol: true, patrol_explore: 0.0, ..HuntConfig::default() };
    let world = clock_off(7);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(meal_config(hunt, prey_founder));
    let prey = introduce(&mut fauna, &world, prey_founder, StartingStores::FULL);
    place(&mut fauna, prey, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    drive(&mut fauna, prey, 0.0, false);
    let hunter = introduce(&mut fauna, &world, hunter_founder, HUNTING);
    place(&mut fauna, hunter, STRIKER_X, LANE_Z, EAST);
    let teachers = fauna.config.teachers;
    fauna.controllers.set(hunter, crate::controller::heuristic(hunter_founder, teachers));
    println!("== {} on {} ==", hunter_founder.name(), prey_founder.name());
    let mut death: Option<Site> = None;
    let mut last_site = animal(&fauna, prey).site;
    for _ in 0..ticks {
        fauna.step_with(&world, &mut flora, 1);
        match fauna.view().animal(prey) {
            Some(p) => last_site = p.site,
            None if death.is_none() => death = Some(last_site),
            None => {}
        }
        println!("{}", trace_line(&fauna, &world, &flora, hunter, prey, death));
    }
    println!(
        "kills={} carrion on ground={:.5} eaten={:.5}",
        fauna.ledger.departed_founder(prey_founder, crate::Departure::Killed { by: hunter_founder }),
        carrion_total(&flora),
        fauna.ledger.eaten_carrion_by_founder[hunter_founder.index()],
    );
}

#[test]
#[ignore = "trace: run by name with --nocapture"]
fn trace_lanternjaw_kill_to_meal() {
    trace(LJ, Founder::Blind, 400);
}

#[test]
#[ignore = "trace: run by name with --nocapture"]
fn trace_lone_chorister_kill_to_meal() {
    trace(Founder::Chorister, Founder::Blind, 400);
}

// ------------------------------------------------------------------ helpers

/// The patrol on with no exploring, and the lanternjaw's and the Veilhound's bite sized so a
/// full stomach is a handful of bites (a meal inside a short case), from the config's own
/// gut: one full-effort bite is a quarter of the adult stomach.
fn quick_meal_config(hunt: HuntConfig, prey: Founder) -> FaunaConfig {
    let mut c = meal_config(hunt, prey);
    for f in [LJ, Founder::Chorister] {
        let p = c.founders[f.index()];
        let body = p.core.body_max;
        let period = c.manifest(f).controller_period_s;
        let per = p.gut.throughput_factor(body, body) * body * period;
        c.founders[f.index()].gut.bite_per_body_s = 0.25 * p.gut.capacity(body) / per;
    }
    c
}

fn patrol() -> HuntConfig {
    HuntConfig { patrol: true, patrol_explore: 0.0, ..HuntConfig::default() }
}

/// A lanternjaw on its own heuristic at `(x, z)` facing `heading`.
fn patroller(fauna: &mut Fauna, world: &World, x: f64, z: f64, heading: f64) -> u64 {
    let id = introduce(fauna, world, LJ, HUNTING);
    place(fauna, id, x, z, heading);
    let teachers = fauna.config.teachers;
    fauna.controllers.set(id, crate::controller::heuristic(LJ, teachers));
    id
}

/// A kill's carcass on the face under `(x_m, z_m)`: `organic` of carrion and the carcass
/// record, as `kill` lays them.
fn carcass_at(flora: &mut Flora, x_m: f64, z_m: f64, y: u32, organic: f64) -> Site {
    let site = Site { x: (x_m / VOXEL_M).floor() as u32, y, z: (z_m / VOXEL_M).floor() as u32 };
    assert!(flora.deposit(site, Deposit { kind: DepositKind::Carrion, organic, mineral: 0.05 * organic, energy: 2.0 * organic }));
    assert!(flora.lay_carcass(site, Carcass { lineage: Founder::Blind.index() as u8, heading_rad: 0.0, organic }));
    site
}

/// Litter on the 2 × 2 faces around `(x_m, z_m)` at layer `y`.
fn litter_at(flora: &mut Flora, x_m: f64, z_m: f64, y: u32) {
    let (cx, cz) = ((x_m / VOXEL_M).round() as u32, (z_m / VOXEL_M).round() as u32);
    for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        assert!(flora.deposit(
            Site { x: cx + dx - 1, y, z: cz + dz - 1 },
            Deposit { kind: DepositKind::Litter, organic: 1.0, mineral: 0.02, energy: 2.0 },
        ));
    }
}

fn eaten(fauna: &Fauna, f: Founder) -> f64 {
    fauna.ledger.eaten_carrion_by_founder[f.index()]
}

fn swallowed(fauna: &Fauna, f: Founder) -> f64 {
    fauna.ledger.swallowed_by_founder[f.index()]
}

/// Ticks to walk `d_m` at `founder`'s cruise, twice over, plus room to turn and start.
fn walk_ticks(fauna: &Fauna, founder: Founder, d_m: f64) -> u32 {
    let cruise = fauna.config.manifest(founder).cruise_m_per_s;
    (2.0 * d_m / cruise * f64::from(crate::TICK_HZ)).ceil() as u32 + 100
}

// ------------------------------------------------------------------ the meal

/// A hungry patroller kills a shredder in its capsule and eats from the corpse until its
/// stomach is full or the carcass is gone. The carcass loses exactly what its mouths took;
/// the stomach gains that less the configured spill; the ledger closes; and the kill's place
/// is remembered productive.
#[test]
fn a_kill_is_eaten_until_sated_or_the_carcass_is_gone() {
    let world = clock_off(11);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(quick_meal_config(patrol(), Founder::Blind));
    let prey = introduce(&mut fauna, &world, Founder::Blind, StartingStores::FULL);
    place(&mut fauna, prey, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    drive(&mut fauna, prey, 0.0, false);
    let lj = patroller(&mut fauna, &world, STRIKER_X, LANE_Z, EAST);
    let mut kill_xz = (0.0, 0.0);
    step_until(&mut fauna, &world, &mut flora, 60, |f| match f.view().animal(prey) {
        Some(p) => {
            kill_xz = (p.pose.x, p.pose.z);
            false
        }
        None => true,
    })
    .expect("the prey in its capsule is killed");
    let carrion0 = carrion_total(&flora);
    let (eaten0, swallowed0) = (eaten(&fauna, LJ), swallowed(&fauna, LJ));
    assert!(carrion0 > 0.0, "the kill laid a corpse");
    let carcass_left = |flora: &Flora| flora.view().ground.iter().any(|g| g.carcass.is_some() && g.carrion > 0.0);
    for _ in 0..300 {
        fauna.step_with(&world, &mut flora, 1);
        if !animal(&fauna, lj).eating || !carcass_left(&flora) {
            break;
        }
    }
    let ate = eaten(&fauna, LJ) - eaten0;
    assert!(ate > 0.0, "it ate from its kill");
    assert!(
        !animal(&fauna, lj).eating || !carcass_left(&flora),
        "it ate until its stomach was full or the carcass was gone"
    );
    let lost = carrion0 - carrion_total(&flora);
    assert!((lost - ate).abs() <= 1e-12 * carrion0.max(1.0), "the carcass lost {lost}, its mouths took {ate}");
    let spill = fauna.config.founder(LJ).bite_spill_fraction.clamp(0.0, 1.0);
    let gained = swallowed(&fauna, LJ) - swallowed0;
    assert!((gained - ate * (1.0 - spill)).abs() <= 1e-12 * ate.max(1e-6), "the stomach gained {gained} of {ate} taken");
    assert_closes(&fauna, "after the meal");
    let cfg = fauna.config.hunt;
    let ring = 48.0 * VOXEL_M;
    assert!(
        animal(&fauna, lj).hunt.is_productive(&cfg, kill_xz.0, kill_xz.1, fauna.tick, ring),
        "the kill's place is remembered productive"
    );
}

/// A hungry patroller with a usable carcass behind it, past its mouth's reach and within
/// `carcass_scan_m`, and litter ahead: it turns, walks to the carcass and eats it. With the
/// meal rule off (main's rule) it does not eat in the same time.
#[test]
fn a_hungry_hunter_walks_to_a_carcass_out_of_its_mouths_reach_and_eats_it() {
    let mut ate_by_rule = Vec::new();
    for meal_first in [true, false] {
        let hunt = HuntConfig { meal_first, ..patrol() };
        let world = clock_off(12);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(quick_meal_config(hunt, Founder::Blind));
        litter_at(&mut flora, 8.0, LANE_Z, SURFACE_Y);
        let lj = patroller(&mut fauna, &world, 4.0, LANE_Z, EAST);
        // Behind it, half the scan away: well past its mouth, inside the scan.
        let d = 0.5 * hunt.carcass_scan_m;
        let site = carcass_at(&mut flora, 4.0 - d, LANE_Z, SURFACE_Y, 0.5);
        let cols = crate::body::mouth_columns(&world.view(), &animal(&fauna, lj).pose, &fauna.config.founder(LJ).body_at(animal(&fauna, lj).body));
        assert!(!cols.iter().any(|&(x, z)| x as u32 == site.x && z == site.z), "the carcass starts out of its mouth's reach");
        let carrion0 = carrion_total(&flora);
        let ticks = walk_ticks(&fauna, LJ, d);
        step_until(&mut fauna, &world, &mut flora, ticks, |f| eaten(f, LJ) > 0.0);
        let ate = eaten(&fauna, LJ);
        let lost = carrion0 - carrion_total(&flora);
        assert!((lost - ate).abs() <= 1e-12, "the carcass lost {lost}, its mouths took {ate}");
        assert_closes(&fauna, "after the walk to the carcass");
        ate_by_rule.push(ate);
    }
    assert!(ate_by_rule[0] > 0.0, "with the meal rule it walks to the carcass and eats");
    assert_eq!(ate_by_rule[1], 0.0, "with main's rule it does not");
}

/// A hunter whose mouth is over a usable carcass and a shredder standing in its capsule:
/// hungry, it eats and does not strike; sated, it does neither. With the meal rule off
/// (main's rule) the hungry one strikes.
#[test]
fn a_hunter_at_a_carcass_does_not_strike_a_prey_in_reach() {
    for (meal_first, hungry) in [(true, true), (true, false), (false, true)] {
        let hunt = HuntConfig { meal_first, ..patrol() };
        let world = clock_off(13);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(quick_meal_config(hunt, Founder::Blind));
        let prey = introduce(&mut fauna, &world, Founder::Blind, StartingStores::FULL);
        place(&mut fauna, prey, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
        drive(&mut fauna, prey, 0.0, false);
        let lj = patroller(&mut fauna, &world, STRIKER_X, LANE_Z, EAST);
        // The carcass on every face of its mouth.
        let pose = animal(&fauna, lj).pose;
        let geometry = fauna.config.founder(LJ).body_at(animal(&fauna, lj).body);
        for (x, z) in crate::body::mouth_columns(&world.view(), &pose, &geometry) {
            let site = Site { x: x.rem_euclid(48) as u32, y: SURFACE_Y, z };
            assert!(flora.deposit(site, Deposit { kind: DepositKind::Carrion, organic: 0.2, mineral: 0.01, energy: 0.4 }));
            assert!(flora.lay_carcass(site, Carcass { lineage: Founder::Blind.index() as u8, heading_rad: 0.0, organic: 0.2 }));
        }
        if !hungry {
            animal_mut(&mut fauna, lj).eating = false;
        }
        // Long enough for a strike's windup to begin and its lunge to land.
        for _ in 0..40 {
            fauna.step_with(&world, &mut flora, 1);
            if !hungry {
                animal_mut(&mut fauna, lj).eating = false;
            }
        }
        let struck = strikes(&fauna);
        match (meal_first, hungry) {
            (true, true) => {
                assert_eq!(struck, 0, "hungry at a carcass, it does not strike");
                assert!(eaten(&fauna, LJ) > 0.0, "hungry at a carcass, it eats");
                assert!(fauna.view().animal(prey).is_some(), "the prey lives");
            }
            (true, false) => assert_eq!(struck, 0, "sated at a carcass, it does not strike"),
            (false, _) => assert!(struck > 0, "main's rule: the strike wins over the carcass"),
        }
        assert_closes(&fauna, "after the carcass case");
    }
}

// ------------------------------------------------------------------ where it can go

/// The plain with a plateau across the whole strip on `x ∈ [x0, x1)` metres, raised two
/// voxels more than the lanternjaw steps up: its top is a ledge the hunter cannot walk onto.
fn ledge_world(fauna: &Fauna, x0: f64, x1: f64) -> (World, u32) {
    let rise = fauna.config.founder(LJ).step_limits(VOXEL_M).up + 2;
    let mut w = plain_of_height(
        cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() },
        14,
        8 + rise,
    );
    for z in 0..12u32 {
        for x in (x0 / VOXEL_M) as i64..(x1 / VOXEL_M) as i64 {
            for y in SURFACE_Y + 1..=SURFACE_Y + rise {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    (w, SURFACE_Y + rise)
}

/// A patroller on the floor; litter on a ledge it cannot step up to (nearer) and on the
/// floor (farther), both inside its scan. Exploring every choice (a uniform draw among the
/// open patches), it never picks the ledge; it does pick the floor patch. With
/// `reachable_only` off (main's rule) it picks the ledge at least once.
#[test]
fn the_patrol_never_picks_a_patch_it_cannot_walk_to() {
    let mut on_ledge_by_rule = Vec::new();
    for reachable_only in [true, false] {
        let hunt = HuntConfig { reachable_only, patrol_explore: 1.0, ..patrol() };
        let mut fauna = Fauna::new(quick_meal_config(hunt, Founder::Blind));
        let (x0, x1) = (5.0, 8.0);
        let (world, top) = ledge_world(&fauna, x0, x1);
        let mut flora = Flora::new(FloraConfig::default());
        litter_at(&mut flora, 6.5, LANE_Z, top);
        litter_at(&mut flora, 0.5, LANE_Z, SURFACE_Y);
        let lj = patroller(&mut fauna, &world, 3.0, LANE_Z, EAST);
        let i = fauna.animals.iter().position(|a| a.id == lj).expect("alive");
        let h = hunter(&fauna);
        let (mut on_ledge, mut on_floor) = (0, 0);
        for t in 0..64 {
            fauna.tick = t * 1000;
            let (x, _) = crate::hunt::choose_patch(&mut fauna, i, &world.view(), &flora.view(), &h);
            let x = x.rem_euclid(48.0 * VOXEL_M);
            if (x0..x1).contains(&x) {
                on_ledge += 1;
            } else if !(x0 - 1.0..x1 + 1.0).contains(&x) {
                on_floor += 1;
            }
        }
        if reachable_only {
            assert!(on_floor > 0, "it picks the floor patch it can walk to");
        }
        on_ledge_by_rule.push(on_ledge);
    }
    assert_eq!(on_ledge_by_rule[0], 0, "reachable only: never the ledge");
    assert!(on_ledge_by_rule[1] > 0, "main's rule picks the ledge (the case can tell)");
}

/// Two equal litter patches, P a little farther than Q. With nothing remembered the patrol
/// (no exploring) takes the nearer, Q; with a kill remembered at P and `success_boost` > 0
/// it takes P.
#[test]
fn a_remembered_kill_makes_its_patch_preferred() {
    let hunt = HuntConfig { success_boost: 1.0, ..patrol() };
    let world = clock_off(15);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(quick_meal_config(hunt, Founder::Blind));
    let (p, q) = ((7.0, LANE_Z), (0.5, LANE_Z));
    litter_at(&mut flora, p.0, p.1, SURFACE_Y);
    litter_at(&mut flora, q.0, q.1, SURFACE_Y);
    let lj = patroller(&mut fauna, &world, 3.5, LANE_Z, EAST);
    let i = fauna.animals.iter().position(|a| a.id == lj).expect("alive");
    let h = hunter(&fauna);
    let ring = 48.0 * VOXEL_M;
    let near = |(x, z): (f64, f64), (tx, tz): (f64, f64)| {
        let (dx, dz) = crate::pose::wrapped_delta(x, z, tx, tz, ring);
        (dx * dx + dz * dz).sqrt() <= 1.0
    };
    let first = crate::hunt::choose_patch(&mut fauna, i, &world.view(), &flora.view(), &h);
    assert!(near(first, q), "with nothing remembered it takes the nearer patch: {first:?}");
    let until = fauna.tick + 1_000_000;
    let tick = fauna.tick;
    fauna.animals[i].hunt.remember_productive(&hunt, p.0, p.1, tick, until, ring);
    let second = crate::hunt::choose_patch(&mut fauna, i, &world.view(), &flora.view(), &h);
    assert!(near(second, p), "with a kill remembered there it takes P: {second:?}");
}

// ------------------------------------------------------------------ the Veilhound

/// A Veilhound pack of the table's size, hungry, between hunts (no target, no feast), and a
/// usable carcass past one member's mouth within `carcass_scan_m`: the member walks to it and
/// eats.
#[test]
fn a_veilhound_walks_to_a_carcass_between_hunts_and_eats_it() {
    assert!(veilhound_meal(true) > 0.0, "with the meal rule a member walks to the carcass and eats");
    assert_eq!(veilhound_meal(false), 0.0, "with main's rule none does");
}

/// What the Veilhounds ate in the case, under `meal_first`.
fn veilhound_meal(meal_first: bool) -> f64 {
    let world = clock_off(16);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(quick_meal_config(HuntConfig { meal_first, ..patrol() }, Founder::Blind));
    let den = Site { x: 16, y: SURFACE_Y, z: 6 };
    assert!(fauna.apply(&world, Command::FoundPack { den, founder: Founder::Chorister, stores: HUNTING, heading_rad: EAST }));
    let teachers = fauna.config.teachers;
    let members: Vec<u64> = fauna.animals.iter().filter(|a| a.founder == Some(Founder::Chorister)).map(|a| a.id).collect();
    for (k, &id) in members.iter().enumerate() {
        place(&mut fauna, id, 4.0 + 0.5 * k as f64, 0.75 + 1.5 * (k % 2) as f64, EAST);
        fauna.controllers.set(id, crate::controller::heuristic(Founder::Chorister, teachers));
    }
    let d = 0.5 * fauna.config.hunt.carcass_scan_m;
    let a = animal(&fauna, members[0]).pose;
    carcass_at(&mut flora, a.x - d, a.z, SURFACE_Y, 0.5);
    let carrion0 = carrion_total(&flora);
    let ticks = walk_ticks(&fauna, Founder::Chorister, d);
    step_until(&mut fauna, &world, &mut flora, ticks, |f| eaten(f, Founder::Chorister) > 0.0);
    let ate = eaten(&fauna, Founder::Chorister);
    assert!(((carrion0 - carrion_total(&flora)) - ate).abs() <= 1e-12);
    assert_closes(&fauna, "after the Veilhound's meal");
    ate
}
