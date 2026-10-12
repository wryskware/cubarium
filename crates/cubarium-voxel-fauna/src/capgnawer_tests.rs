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
    // YOUNG-ROAM: a young walks at its own length's pace, slower than an adult.
    for t in 1..=600u32 {
        fauna.step_with_senses(&world, &mut flora, 1, &mut senses);
        if bites(&fauna) > 0 {
            reached = Some(t);
            break;
        }
    }
    let x1 = fauna.view().animal(far).map(|a| a.pose.x);
    assert!(reached.is_some(), "no bite on the caps in 600 ticks (x {x0:.2} -> {x1:?}, the cap at {cap_x:.2})");
    assert_closes(&fauna, "after the walk to the caps");
}

/// A mouth accepting fungal tissue switches to the stand after caps are cropped.
/// The shared plant stays untouched, and both transfer ledgers book the same bite.
#[test]
fn a_cropped_cap_feeds_mycelium_to_a_capgnawer_or_another_fungal_diet() {
    use cubarium_voxel_flora::FaceLayer;
    for founder in [GNAWER, SHREDDER] {
        for cropped in [false, true] {
            let world = clock_off(89);
            let mut fc = FloraConfig::default();
            fc.glowcap.mycelium_graze_fraction = 0.6;
            fc.glowcap.defence_per_bite = 0.0;
            let mut flora = Flora::new(fc);
            let mut c = FaunaConfig::default();
            c.founders[founder.index()].diet = crate::Diet::only(Food::CapTissue);
            c.founders[founder.index()].gut.s0 = 0.0;
            let mut fauna = Fauna::new(c);
            let gnawer = introduce(
                &mut fauna,
                &world,
                founder,
                StartingStores {
                    body: 1.0,
                    reserve: 0.0,
                },
            );
            drive(&mut fauna, gnawer, 0.0, true);
            let at = animal(&fauna, gnawer).site;
            let fungus = glowcap(&mut flora, &world, i64::from(at.x), at.z);
            assert!(flora.apply(
                &world,
                FloraCommand::SeedOnFace {
                    site: at,
                    species: Species::Springturf,
                    wood: flora.config().springturf.wood_max,
                }
            ));
            let plant = *flora.view().stand_at(at).unwrap();
            if cropped {
                flora
                    .take_foliage_of(fungus, FaceLayer::Fungus, f64::MAX, &(i64::MIN..=i64::MAX))
                    .expect("crop the edible cap tissue");
            }
            let before = *flora.view().fungus_at(fungus).unwrap();
            let edible_caps = flora.view().edible_foliage(&before);
            assert_eq!(
                edible_caps <= 1e-12,
                cropped,
                "fixture: cap availability decides the diet switch"
            );
            if !cropped {
                let a = animal(&fauna, gnawer);
                let shape = crate::body_of(fauna.config(), &a).unwrap();
                let cols = crate::body::mouth_columns(&world.view(), &a.pose, &shape);
                assert!(
                    crate::body::mouth_meal(
                        &flora.view(),
                        &world.view(),
                        &cols,
                        a.site.y,
                        &shape,
                        crate::Diet::only(Food::CapTissue)
                    )
                    .is_some(),
                    "fixture: caps out of mouth reach: cols={cols:?}, band={:?}, layers={:?}",
                    shape.mouth_layers(a.site.y, VOXEL_M),
                    flora.view().layers(&before).collect::<Vec<_>>()
                );
            }
            let cap_taken = flora.view().ledger.foliage_taken[Species::Glowcap.index()];
            let mut ate = false;
            for _ in 0..20 {
                fauna.step_with(&world, &mut flora, 1);
                if fauna.ledger.bites_by_food[Food::CapTissue.index()] > 0 {
                    ate = true;
                    break;
                }
            }
            assert!(
                ate,
                "{} left fungal tissue uneaten (cropped={cropped})",
                founder.name()
            );
            let after = *flora.view().fungus_at(fungus).unwrap();
            let lost = before.wood - after.wood;
            let offer = flora.config().glowcap.mycelium_graze_fraction * before.wood;
            let eaten = if cropped {
                assert!(lost > 0.0 && lost <= offer + 1e-12);
                assert_eq!(
                    after.foliage, before.foliage,
                    "the bite came from W rather than refuge caps"
                );
                assert_eq!(
                    flora.view().ledger.foliage_taken[Species::Glowcap.index()],
                    cap_taken
                );
                lost
            } else {
                assert_eq!(lost, 0.0, "a reachable cap is chosen ahead of mycelium");
                let cap_loss = before.foliage - after.foliage;
                assert!(cap_loss > 0.0);
                assert!(
                    (flora.view().ledger.foliage_taken[Species::Glowcap.index()]
                        - cap_taken
                        - cap_loss)
                        .abs()
                        < 1e-12
                );
                cap_loss
            };
            assert!(
                (flora.view().ledger.mycelium_taken[Species::Glowcap.index()] - lost).abs() < 1e-12
            );
            assert!((fauna.ledger.eaten_by_food[Food::CapTissue.index()] - eaten).abs() < 1e-12);
            assert!((fauna.ledger.eaten_mycelium_by_founder[founder.index()] - lost).abs() < 1e-12);
            assert_eq!(
                flora.view().stand_at(at).unwrap().material(),
                plant.material()
            );
            assert_closes(&fauna, "after grazing exposed mycelium");
            let v = flora.view();
            for (held, expected) in [
                (v.organic(), v.ledger.expected_organic()),
                (v.mineral(), v.ledger.expected_mineral()),
                (v.energy(), v.ledger.expected_energy()),
            ] {
                assert!((held - expected).abs() < 1e-10 * held.abs().max(1.0));
            }
        }
    }
}

/// Cropped caps still advertise edible stand tissue; a zero mycelium offer is quiet.
#[test]
fn cropped_fungal_scent_follows_the_edible_mycelium_offer() {
    use cubarium_voxel_flora::FaceLayer;
    let world = clock_off(97);
    for fraction in [0.0, 0.5] {
        let mut fc = FloraConfig::default();
        fc.glowcap.mycelium_graze_fraction = fraction;
        let mut flora = Flora::new(fc);
        let mut c = FaunaConfig::default();
        c.cue.fungal_half_life_s = 1.0;
        let mut fauna = Fauna::new(c);
        let id = introduce(
            &mut fauna,
            &world,
            GNAWER,
            StartingStores {
                body: 1.0,
                reserve: 0.0,
            },
        );
        let at = animal(&fauna, id).site;
        let fungus = glowcap(&mut flora, &world, i64::from(at.x), at.z);
        flora
            .take_foliage_of(fungus, FaceLayer::Fungus, f64::MAX, &(i64::MIN..=i64::MAX))
            .expect("crop the edible caps");
        let stand = *flora.view().fungus_at(fungus).unwrap();
        assert!(flora.view().edible_foliage(&stand) <= 1e-12);
        let mut senses = crate::Senses::with_cue(fauna.config().cue);
        for _ in 0..4 {
            senses.update(&world.view(), &flora.view());
        }
        let signal = chem(&fauna, id, &world, &flora, &mut senses);
        assert_eq!(
            signal > 1e-12,
            fraction > 0.0,
            "mycelium fraction {fraction}: cropped fungal scent {signal}"
        );
    }
}

/// Cropped mycelium is ground tissue, so raising the mouth above the face loses it.
#[test]
fn a_mycelium_offer_requires_a_mouth_band_touching_the_root_face() {
    use cubarium_voxel_flora::FaceLayer;
    let world = clock_off(101);
    let mut fc = FloraConfig::default();
    fc.glowcap.mycelium_graze_fraction = 0.5;
    let mut flora = Flora::new(fc);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = introduce(
        &mut fauna,
        &world,
        GNAWER,
        StartingStores {
            body: 1.0,
            reserve: 0.0,
        },
    );
    let a = animal(&fauna, id);
    let fungus = glowcap(&mut flora, &world, i64::from(a.site.x), a.site.z);
    flora
        .take_foliage_of(fungus, FaceLayer::Fungus, f64::MAX, &(i64::MIN..=i64::MAX))
        .expect("crop the edible caps");
    let mut low = crate::body_of(fauna.config(), &a).unwrap();
    low.mouth_floor_m = 0.0;
    let cols = crate::body::mouth_columns(&world.view(), &a.pose, &low);
    let diet = crate::Diet::only(Food::CapTissue);
    assert!(
        matches!(
            crate::body::mouth_meal(&flora.view(), &world.view(), &cols, a.site.y, &low, diet),
            Some((crate::body::Meal::Mycelium { .. }, _, _))
        ),
        "fixture: the grounded mouth reaches cropped mycelium"
    );
    let mut raised = low;
    raised.mouth_floor_m = world.config().voxel_m;
    raised.mouth_ceiling_m = raised.mouth_floor_m + low.height_m;
    assert!(
        crate::body::mouth_meal(&flora.view(), &world.view(), &cols, a.site.y, &raised, diet)
            .is_none(),
        "a mouth one configured voxel above the root face received mycelium"
    );
}

/// A bite may finish a small regrown cap parcel and spend its remaining room on W.
#[test]
fn one_bite_finishes_regrown_caps_then_grazes_exposed_mycelium() {
    use cubarium_voxel_flora::FaceLayer;
    for scarce in [false, true] {
        let mut world = clock_off(103);
        let mut c = FaunaConfig::default();
        let p = &mut c.founders[GNAWER.index()];
        p.diet = crate::Diet::only(Food::CapTissue);
        p.gut.s0 = 0.0;
        p.gut.stomach_k = 1.0;
        p.gut.bite_per_body_s = 1.0;
        p.gut.mass_exponent = 1.0;
        p.core.bite_half_stock = 0.0;
        p.bite_spill_fraction = 0.0;
        let full_bite =
            p.gut.bite_per_body_s * p.core.body_max * GNAWER.manifest().controller_period_s;
        p.core.bite_half_stock = if scarce { 2.0 * full_bite } else { 0.0 };
        let mut fc = FloraConfig::default();
        fc.step_every = 1;
        fc.glowcap.mycelium_graze_fraction = 0.5;
        fc.glowcap.defence_per_bite = 0.0;
        fc.glowcap.maintenance = 0.0;
        fc.glowcap.wood_rate = 0.0;
        fc.glowcap.senescence = 0.0;
        fc.glowcap.propagule_rate = 0.0;
        fc.glowcap.dark_uptake_per_s = 0.0;
        // Open sky: the fixture's arithmetic has no sun in it (GLOWCAP-2).
        fc.glowcap.sun_cap_loss_per_s = 0.0;
        fc.glowcap.sun_mycelium_loss_per_s = 0.0;
        let wood = fc.glowcap.wood_max;
        let reserve = fc.glowcap.reserve_cap * wood;
        let regrown = (0.25 * full_bite).min(0.25 * reserve / (1.0 + fc.glowcap.build));
        fc.glowcap.foliage_rate = regrown / (wood * cubarium_voxel_flora::DT);
        let mut flora = Flora::new(fc);
        let mut fauna = Fauna::new(c);
        let id = introduce(
            &mut fauna,
            &world,
            GNAWER,
            StartingStores {
                body: 1.0,
                reserve: 0.0,
            },
        );
        drive(&mut fauna, id, 0.0, true);
        let at = animal(&fauna, id).site;
        let fungus = glowcap(&mut flora, &world, i64::from(at.x), at.z);
        flora
            .take_foliage_of(fungus, FaceLayer::Fungus, f64::MAX, &(i64::MIN..=i64::MAX))
            .expect("crop caps to their refuge");
        flora.step(&mut world);
        let before = *flora.view().fungus_at(fungus).unwrap();
        let fresh_caps = flora.view().edible_foliage(&before);
        assert!(
            fresh_caps > 0.0 && fresh_caps < full_bite,
            "fixture: regrowth {fresh_caps} leaves spare room in a configured bite {full_bite}"
        );
        assert!(
            (fresh_caps - regrown).abs() < 1e-12,
            "fixture: configured cap regrowth"
        );
        let cap_ledger_before = flora.view().ledger.foliage_taken[Species::Glowcap.index()];
        for _ in 0..20 {
            fauna.step_with(&world, &mut flora, 1);
            if fauna.ledger.bites_by_food[Food::CapTissue.index()] > 0 {
                break;
            }
        }
        assert_eq!(fauna.ledger.bites_by_food[Food::CapTissue.index()], 1);
        let after = *flora.view().fungus_at(fungus).unwrap();
        let cap_loss = before.foliage - after.foliage;
        let mycelium_loss = before.wood - after.wood;
        assert!(
            (cap_loss - fresh_caps).abs() < 1e-12,
            "the bite must finish the fresh caps first"
        );
        assert!(
            mycelium_loss > 0.0,
            "fresh cap regrowth kept a hungry mouth from grazing W in its spare bite room"
        );
        assert!(
            (flora.view().ledger.foliage_taken[Species::Glowcap.index()]
                - cap_ledger_before
                - cap_loss)
                .abs()
                < 1e-12
        );
        assert!(
            (flora.view().ledger.mycelium_taken[Species::Glowcap.index()] - mycelium_loss).abs()
                < 1e-12
        );
        assert!(
            (fauna.ledger.eaten_mycelium_by_founder[GNAWER.index()] - mycelium_loss).abs() < 1e-12
        );
        assert!(
            (fauna.ledger.eaten_by_food[Food::CapTissue.index()] - cap_loss - mycelium_loss).abs()
                < 1e-12
        );
        assert_closes(&fauna, "after one combined cap and mycelium bite");
        let v = flora.view();
        for (held, expected) in [
            (v.organic(), v.ledger.expected_organic()),
            (v.mineral(), v.ledger.expected_mineral()),
            (v.energy(), v.ledger.expected_energy()),
        ] {
            assert!((held - expected).abs() < 1e-10 * held.abs().max(1.0));
        }
    }
}
