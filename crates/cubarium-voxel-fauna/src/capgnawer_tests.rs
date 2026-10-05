//! **The capgnawer follows the fungus** (the capgnawer package, 2026-10-05; proposal
//! `design/lanternjaw-detritus-web-proposal-2026-10-05.md` §3.1E with Astra's review),
//! written before the mechanisms. A child of `step`, so a case can reach the layer's fields.
//!
//! Every case checks a mechanism on hand-built inputs **against the config it sets**: no
//! default, ratio or population is asserted.
//!
//! - **A full clutch first**: a layer whose reserve pays for `clutch_max` eggs lays them at
//!   once; one that pays for fewer waits up to `clutch_hold_max_s` and then lays what it
//!   can; one short of an egg lays none. The clutch rises with the reserve up to
//!   `clutch_max`.
//! - **Eggs at the fungus**: with `EggSite::Fungus` a ready layer lays within
//!   `egg_fungus_radius_m` of a glowcap holding caps and not on a face farther off.
//! - **The fungal scent carries**: with a long `fungal_half_life_s` a young capgnawer metres
//!   from a glowcap reads its `Chem(fungal)` over the forager's gate, rising toward it, where
//!   the shared 2 s field reads nothing; under its own heuristic it walks to the caps.
//! - Ledgers conserve through each.

use super::predation_fixture::*;
use crate::{EggSite, Fauna, FaunaConfig, Food, Founder, Reproduction, StartingStores, DT};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species};

const SHREDDER: Founder = Founder::Blind;
const GNAWER: Founder = Founder::Capgnawer;

/// The test's egg rule: a one-tick hold, a long interval, eggs of `egg` above `floor`, a
/// clutch from surplus of at most `max`, waiting at most `wait_ticks` for a full one.
fn rule(egg: f64, floor: f64, max: u32, wait_ticks: u32) -> Reproduction {
    Reproduction {
        surplus_floor: floor,
        surplus_hold_s: DT,
        birth_interval_s: 1000.0 * DT,
        clutch_size: 1,
        clutch_max: max,
        egg_organic: egg,
        incubation_s: 1000.0 * DT,
        nest_radius_m: 0.0,
        clutch_hold_max_s: f64::from(wait_ticks) * DT,
        ..Reproduction::EGGS_PLACEHOLDER
    }
}

/// `founder`'s config with egg rule `r`, a reserve cap of 1 (a whole adult's reserve is
/// room for several eggs) and empty-stomach appetite so digestion moves nothing.
fn egg_config(founder: Founder, r: Reproduction) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for p in c.founders.iter_mut() {
        p.gut.s0 = 0.0;
    }
    let core = &mut c.founders[founder.index()].core;
    core.reserve_cap = 1.0;
    core.reproduction = r;
    c
}

fn litter(flora: &mut Flora, site: Site, organic: f64) {
    assert!(flora.deposit(
        site,
        Deposit { kind: DepositKind::Litter, organic, mineral: 0.02 * organic, energy: 2.0 * organic }
    ));
}

/// One adult shredder on litter under `config`, holding still, with a reserve of
/// `reserve` organic units.
fn one_layer(config: FaunaConfig, reserve: f64) -> (Fauna, World, Flora, u64) {
    let world = clock_off(41);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config);
    // The reserve cap is 1, so a whole adult's cap is `body_max`.
    let full = fauna.config().founders[SHREDDER.index()].core.body_max;
    let s = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: reserve / full });
    assert!((animal(&fauna, s).reserve - reserve).abs() < 1e-12, "fixture: the reserve");
    drive(&mut fauna, s, 0.0, false);
    litter(&mut flora, animal(&fauna, s).site, 1.0);
    (fauna, world, flora, s)
}

// ------------------------------------------------------------ a full clutch first

/// **The clutch follows the reserve.** With `clutch_max` 3 and a wait of a few ticks: a
/// reserve paying for 3 lays 3 on the first tick; one paying for 2 or 1 lays nothing until
/// the wait is over and then lays what it pays for; one short of an egg never lays.
#[test]
fn a_fed_layer_lays_a_full_clutch_at_once_and_a_hungry_one_waits_then_lays_fewer() {
    let core = FaunaConfig::default().founders[SHREDDER.index()].core;
    let (egg, floor) = (core.body_min, 0.05 * core.body_max);
    let (max, wait) = (3u32, 5u32);
    let r = rule(egg, floor, max, wait);
    let mut laid_by_reserve = Vec::new();
    for eggs_paid in [3.5, 2.5, 1.5, 0.5] {
        let reserve = floor + eggs_paid * egg;
        let (mut fauna, world, mut flora, _) = one_layer(egg_config(SHREDDER, r), reserve);
        fauna.step_with(&world, &mut flora, 1);
        let want = r.clutch_for(reserve);
        if want >= max {
            assert_eq!(fauna.clutches.len(), 1, "{eggs_paid} eggs paid: a full clutch at once");
            assert_eq!(fauna.clutches[0].count, max);
        } else {
            assert!(fauna.clutches.is_empty(), "{eggs_paid} eggs paid: it waits for a full clutch");
            for _ in 0..wait + 1 {
                fauna.step_with(&world, &mut flora, 1);
            }
            if want == 0 {
                assert!(fauna.clutches.is_empty(), "short of one egg: nothing laid");
            } else {
                assert_eq!(fauna.clutches.len(), 1, "{eggs_paid} eggs paid: laid at the end of the wait");
                assert_eq!(fauna.clutches[0].count, want, "of what the reserve pays for");
            }
        }
        laid_by_reserve.push(fauna.clutches.first().map_or(0, |c| c.count));
        assert_closes(&fauna, "after a clutch");
    }
    // Highest reserve first: the clutch never grows as the reserve falls, and it differs.
    assert!(laid_by_reserve.windows(2).all(|w| w[0] >= w[1]), "{laid_by_reserve:?}");
    assert!(laid_by_reserve[0] > laid_by_reserve[2], "{laid_by_reserve:?}");
}

// ----------------------------------------------------------- eggs at the fungus

/// A full-grown glowcap on column `(x, z)` of the plain, holding caps.
fn glowcap(flora: &mut Flora, world: &World, x: i64, z: u32) -> Site {
    let wood = flora.config().species(Species::Glowcap).wood_max;
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species: Species::Glowcap, wood }));
    let site = Site { x: x as u32, y: SURFACE_Y, z };
    assert!(flora.view().fungus_at(site).expect("a glowcap").foliage > 0.0, "fixture: caps");
    site
}

/// **A capgnawer lays at the fungus.** A ready adult at `(8, 6)` with a glowcap `d` columns
/// east: it lays when the glowcap is within the egg radius and not when it is beyond it.
#[test]
fn a_capgnawer_lays_within_the_radius_of_a_glowcap_and_not_beyond() {
    let core = FaunaConfig::default().founders[GNAWER.index()].core;
    let (egg, floor) = (core.body_min, 0.05 * core.body_max);
    let radius = 1.0;
    let lays = |cols: i64| -> bool {
        let world = clock_off(43);
        let mut flora = Flora::new(FloraConfig::default());
        let mut r = rule(egg, floor, 2, 0);
        r.egg_fungus_radius_m = radius;
        let mut c = egg_config(GNAWER, r);
        c.founders[GNAWER.index()].egg_site = EggSite::Fungus;
        let mut fauna = Fauna::new(c);
        let g = introduce(&mut fauna, &world, GNAWER, StartingStores { body: 1.0, reserve: (floor + 2.5 * egg) / core.body_max });
        drive(&mut fauna, g, 0.0, false);
        let at = animal(&fauna, g).site;
        glowcap(&mut flora, &world, i64::from(at.x) + cols, at.z);
        fauna.step_with(&world, &mut flora, 1);
        assert_closes(&fauna, "after the fungal egg case");
        !fauna.clutches.is_empty()
    };
    let inside = (radius / VOXEL_M).floor() as i64;
    assert!(lays(inside), "a glowcap {inside} columns off is within {radius} m");
    assert!(lays(0), "on the glowcap's own face");
    assert!(!lays(inside + 3), "a glowcap {} columns off is beyond {radius} m", inside + 3);
}

// --------------------------------------------------------- the fungal scent carries

/// The capgnawer's `Chem(fungal)` response at body `id` (validity 1).
fn chem(fauna: &Fauna, id: u64, world: &World, flora: &Flora, senses: &mut crate::Senses) -> f64 {
    let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
    let manifest = fauna.config().manifest(GNAWER);
    let m = *manifest.modules.iter().find(|m| m.name == "Chem(fungal)").expect("the fungal chem");
    let (view, fv) = (world.view(), flora.view());
    let obs = crate::body::observation(fauna, i, &view, &fv, &manifest, Some(senses), None);
    assert_eq!(obs[m.offset + 2], 1.0, "the chem receptor is valid");
    obs[m.offset]
}

/// A young capgnawer (a hatchling's body, hungry) at column `x`, row 6, facing east.
fn young(fauna: &mut Fauna, world: &World, x_m: f64) -> u64 {
    let core = fauna.config().founders[GNAWER.index()].core;
    let id = introduce(fauna, world, GNAWER, StartingStores { body: core.body_min / core.body_max, reserve: 0.3 });
    place(fauna, id, x_m, 6.5 * VOXEL_M, EAST);
    id
}

/// **A young capgnawer finds a glowcap beyond its own face.** One glowcap at column 30; a
/// young capgnawer 2 m west of it and another 1 m west. Under a fungal half-life of 300 s the
/// far one reads the scent over the forager's response gate and the near one reads more;
/// under the shared 2 s field the far one reads under the gate. Driven by its own heuristic
/// under the long scent, the far one, facing the glowcap, reaches the caps and bites them
/// within 300 ticks (15 s; under the 2 s field its no-signal search took about 750).
#[test]
fn a_young_capgnawer_smells_and_reaches_a_glowcap_metres_away() {
    let read = |half_life: f64| -> (f64, f64, f64) {
        let world = clock_off(47);
        let mut flora = Flora::new(FloraConfig::default());
        let cap = glowcap(&mut flora, &world, 30, 6);
        let cap_x = (f64::from(cap.x) + 0.5) * VOXEL_M;
        let mut c = FaunaConfig::default();
        c.cue.fungal_half_life_s = half_life;
        let gate = c.teachers.chem_response_gate;
        let mut fauna = Fauna::new(c);
        let far = young(&mut fauna, &world, cap_x - 2.0);
        let near = young(&mut fauna, &world, cap_x - 1.0);
        let mut senses = crate::Senses::with_cue(fauna.config().cue);
        // Long enough for the long field to stand (several of its half-lives).
        for _ in 0..(8.0 * half_life / crate::senses::UPDATE_S).ceil() as usize {
            senses.update(&world.view(), &flora.view());
        }
        let q_far = chem(&fauna, far, &world, &flora, &mut senses);
        let q_near = chem(&fauna, near, &world, &flora, &mut senses);
        (q_far, q_near, gate)
    };
    let (q_far, q_near, gate) = read(300.0);
    assert!(q_far > gate, "2 m off, the long scent reads {q_far}, the gate is {gate}");
    assert!(q_near > q_far, "it rises toward the glowcap: {q_near} against {q_far}");
    let (q_short, _, gate) = read(2.0);
    assert!(q_short <= gate, "2 m off, the 2 s field reads {q_short} (gate {gate})");

    // Under its own heuristic, the far one walks to the caps.
    let world = clock_off(47);
    let mut flora = Flora::new(FloraConfig::default());
    let cap = glowcap(&mut flora, &world, 30, 6);
    let cap_x = (f64::from(cap.x) + 0.5) * VOXEL_M;
    let mut c = FaunaConfig::default();
    c.cue.fungal_half_life_s = 300.0;
    let mut fauna = Fauna::new(c);
    let far = young(&mut fauna, &world, cap_x - 2.0);
    let teachers = fauna.config().teachers;
    assert!(fauna.set_controller(far, crate::controller::heuristic(GNAWER, teachers)));
    let mut senses = crate::Senses::with_cue(fauna.config().cue);
    for _ in 0..4800 {
        senses.update(&world.view(), &flora.view());
    }
    let x0 = animal(&fauna, far).pose.x;
    let bites = |f: &Fauna| f.ledger.bites_by_founder_food[GNAWER.index()][Food::CapTissue.index()];
    let mut reached = None;
    for t in 1..=300u32 {
        fauna.step_with_senses(&world, &mut flora, 1, &mut senses);
        if bites(&fauna) > 0 {
            reached = Some(t);
            break;
        }
    }
    let x1 = fauna.view().animal(far).map(|a| a.pose.x);
    assert!(reached.is_some(), "no bite on the caps in 300 ticks (x {x0:.2} -> {x1:?}, the cap at {cap_x:.2})");
    assert_closes(&fauna, "after the walk to the caps");
}
