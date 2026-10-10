//! ROLES5 (Wrysk 2026-10-10): the shipped frondgrazer gives at most one birth per world
//! day. The rule is read from each shipped preset, so the assertion is the relation Wrysk
//! asked for ("once a day birth rate"), not a tuned number.

use super::predation_fixture::*;
use crate::{Fauna, FaunaConfig, Founder, Reproduction, StartingStores};

const GRAZER: Founder = Founder::Browser;

/// The presets that found frondgrazers on a live world.
const SHIPPED: &[&str] = &[
    "config/desktop/terrarium-d72.toml",
    "config/desktop/terrarium-d96.toml",
    "config/desktop/terrarium.toml",
    "config/desktop/terrarium-wide.toml",
    "config/desktop/voxel.toml",
    "config/tachyon/voxel.toml",
    "config/voidrunner/terrarium-d72.toml",
    "config/voidrunner/terrarium-d96.toml",
    "config/voidrunner/terrarium.toml",
];

fn preset(path: &str) -> toml::Table {
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full}: {e}"));
    text.parse::<toml::Table>().unwrap_or_else(|e| panic!("{full}: {e}"))
}

fn table<'a>(t: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Table> {
    path.iter().try_fold(t, |t, k| t.get(*k)?.as_table())
}

/// The frondgrazer's rule as `doc` sets it over the code default, and the world day, s.
fn grazer_rule(doc: &toml::Table) -> (Reproduction, f64) {
    let base = FaunaConfig::default().founders[GRAZER.index()].core.reproduction;
    let mut value = toml::Table::try_from(base).expect("the rule as TOML");
    if let Some(set) = table(doc, &["fauna", "founders", "frondgrazer", "core", "reproduction"]) {
        for (k, v) in set {
            value.insert(k.clone(), v.clone());
        }
    }
    let rule: Reproduction = value.try_into().expect("the preset's rule");
    let day_min = table(doc, &["world", "day"])
        .and_then(|d| d.get("length_min"))
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        .unwrap_or(cubarium_voxel::weather::DayClock::default().length_min);
    (rule, day_min * 60.0)
}

/// A frondgrazer kept fed past its every gate gives births at least a world day apart, on
/// every shipped preset, and does give them (so the gap is measured, not vacuous).
#[test]
fn a_shipped_frondgrazer_gives_at_most_one_birth_per_world_day() {
    let world = clock_off(510);
    for path in SHIPPED {
        let doc = preset(path);
        assert!(
            table(&doc, &["fauna", "founders", "frondgrazer"]).is_some(),
            "{path} founds no frondgrazer"
        );
        let (rule, day_s) = grazer_rule(&doc);
        let mut c = FaunaConfig::default();
        c.founders[GRAZER.index()].core.reproduction = rule;
        let mut f = Fauna::new(c);
        let parent = introduce(&mut f, &world, GRAZER, StartingStores::FULL);
        let day_ticks = (day_s / super::DT).round() as u64;
        let mut births = Vec::new();
        for tick in 0..3 * day_ticks + day_ticks / 2 {
            let index = f.animals.iter().position(|a| a.id == parent).expect("the parent");
            let sc = crate::body::effective_config(&f.config, &f.animals[index]);
            f.animals[index].reserve = sc.reproduction.package_cost(&sc) * 10.0 + sc.reproduction.surplus_floor;
            super::advance_eligibility(&mut f, index, &sc, &rule);
            let mut newborns = Vec::new();
            super::gestate(&mut f, index, &sc, &rule, (world.config().seed, None), &mut newborns);
            if !newborns.is_empty() {
                assert_eq!(newborns.len(), 1, "{path}: one calf a birth");
                births.push(tick);
            }
        }
        assert!(births.len() >= 2, "{path}: a fed frondgrazer gave {} births in 3.5 days", births.len());
        for w in births.windows(2) {
            assert!(
                w[1] - w[0] >= day_ticks,
                "{path}: births {} ticks apart, under the {day_ticks}-tick world day",
                w[1] - w[0]
            );
        }
    }
}

// ------------------------------------------------------- the capgnawer lives on caps

const GNAWER: Founder = Founder::Capgnawer;
const WIDE: &str = "config/desktop/terrarium-wide.toml";

/// The code defaults with the wide preset's capgnawer yields and fungal scent, the gut's
/// appetite at an empty stomach so digestion is all that moves food.
fn gnawer_config() -> FaunaConfig {
    let doc = preset(WIDE);
    let mut c = FaunaConfig::default();
    let p = &mut c.founders[GNAWER.index()];
    if let Some(yields) = table(&doc, &["fauna", "founders", "capgnawer", "yield_by_food"]) {
        for (name, v) in yields {
            let food = crate::Food::ALL.into_iter().find(|f| f.name() == name).expect("a food name");
            p.yield_by_food[food.index()] = v.as_float().expect("a yield");
        }
    }
    p.gut.s0 = 0.0;
    if let Some(h) = table(&doc, &["fauna", "cue"]).and_then(|t| t.get("fungal_half_life_s")) {
        c.cue.fungal_half_life_s = h.as_float().expect("a half-life");
    }
    c
}

/// A full-grown glowcap on column `(x, z)` of the plain, holding caps.
fn glowcap(flora: &mut cubarium_voxel_flora::Flora, world: &cubarium_voxel::World, x: i64, z: u32) -> cubarium_voxel_flora::Site {
    use cubarium_voxel_flora::{Command, Site, Species};
    let wood = flora.config().species(Species::Glowcap).wood_max;
    assert!(flora.apply(world, Command::Seed { x, z, species: Species::Glowcap, wood }));
    let site = Site { x: x as u32, y: SURFACE_Y, z };
    assert!(flora.view().fungus_at(site).expect("a glowcap").foliage > 0.0, "fixture: caps");
    site
}

fn litter(flora: &mut cubarium_voxel_flora::Flora, site: cubarium_voxel_flora::Site, organic: f64) {
    use cubarium_voxel_flora::{Deposit, DepositKind};
    assert!(flora.deposit(site, Deposit { kind: DepositKind::Litter, organic, mineral: 0.02 * organic, energy: 2.0 * organic }));
}

/// One hungry adult capgnawer holding still and feeding, with caps at its mouth, litter
/// of `litter_organic` on its face, or both. Returns (bites by food, assimilated, eaten).
fn feed_at(caps: bool, litter_organic: f64, feed_ticks: u32) -> ([u64; crate::Food::COUNT], f64, f64) {
    let world = clock_off(53);
    let mut flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());
    let mut fauna = Fauna::new(gnawer_config());
    let id = introduce(&mut fauna, &world, GNAWER, StartingStores { body: 1.0, reserve: 0.0 });
    let switch = drive(&mut fauna, id, 0.0, true);
    let at = animal(&fauna, id).site;
    if caps {
        glowcap(&mut flora, &world, i64::from(at.x), at.z);
    }
    if litter_organic > 0.0 {
        litter(&mut flora, at, litter_organic);
    }
    for _ in 0..feed_ticks {
        fauna.step_with(&world, &mut flora, 1);
    }
    let bites = fauna.ledger.bites_by_founder_food[GNAWER.index()];
    // Digest what was taken: several gut time constants with the mouth shut.
    switch.store(false, std::sync::atomic::Ordering::Relaxed);
    for _ in 0..20 * 120 * crate::TICK_HZ {
        fauna.step_with(&world, &mut flora, 1);
    }
    let eaten: f64 = fauna.ledger.eaten_by_founder_food[GNAWER.index()].iter().sum();
    (bites, fauna.ledger.assimilated_by_founder[GNAWER.index()], eaten)
}

/// **Caps feed it; litter barely does.** Under the shipped yields, an adult capgnawer
/// gains more per unit of caps eaten than per unit of litter, and at a face holding both,
/// with more litter than caps, it bites the caps (the old rule took the larger stock).
#[test]
fn a_capgnawer_at_capped_glowcaps_gains_more_from_caps_than_from_litter() {
    use crate::Food;
    let (cap_bites, cap_gain, cap_eaten) = feed_at(true, 0.0, 40);
    let (litter_bites, litter_gain, litter_eaten) = feed_at(false, 1.0, 40);
    assert!(cap_bites[Food::CapTissue.index()] > 0 && cap_eaten > 0.0, "fixture: it ate caps");
    assert!(litter_bites[Food::Litter.index()] > 0 && litter_eaten > 0.0, "fixture: it ate litter");
    let (per_cap, per_litter) = (cap_gain / cap_eaten, litter_gain / litter_eaten);
    assert!(per_cap > per_litter, "assimilated per unit eaten: caps {per_cap}, litter {per_litter}");

    let world = clock_off(53);
    let mut flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());
    let site = glowcap(&mut flora, &world, 8, 6);
    let caps_stock = flora.view().fungus_at(site).unwrap().foliage;
    let (both, _, _) = feed_at(true, 3.0 * caps_stock, 40);
    assert!(both[Food::CapTissue.index()] > 0, "it bit no caps beside more litter: {both:?}");
    assert_eq!(both[Food::Litter.index()], 0, "it took the larger litter stock over the caps: {both:?}");
}

/// **It leaves ordinary litter for the caps.** A hungry young capgnawer on a lane strewn
/// with litter, a glowcap 2 m east: under its own heuristic it walks to the caps and bites
/// them. With the yield weighing off (every food worth its stock, the old rule), it stays
/// on the litter at its feet.
#[test]
fn a_hungry_capgnawer_on_litter_walks_to_the_glowcap() {
    use crate::Food;
    let run = |by_yield: bool| -> (u64, u64) {
        let world = clock_off(47);
        let mut flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());
        let cap = glowcap(&mut flora, &world, 30, 6);
        for x in 0..48u32 {
            for z in 3..10u32 {
                if (x, z) != (cap.x, cap.z) {
                    litter(&mut flora, cubarium_voxel_flora::Site { x, y: SURFACE_Y, z }, 0.3);
                }
            }
        }
        let cap_x = (f64::from(cap.x) + 0.5) * VOXEL_M;
        let mut c = gnawer_config();
        c.founders[GNAWER.index()].taste_by_yield = by_yield;
        let mut fauna = Fauna::new(c);
        let core = fauna.config().founders[GNAWER.index()].core;
        let id = introduce(&mut fauna, &world, GNAWER, StartingStores { body: core.body_min / core.body_max, reserve: 0.3 });
        place(&mut fauna, id, cap_x - 2.0, 6.5 * VOXEL_M, EAST);
        let teachers = fauna.config().teachers;
        assert!(fauna.set_controller(id, crate::controller::heuristic(GNAWER, teachers)));
        let mut senses = crate::Senses::with_cue(fauna.config().cue);
        for _ in 0..4800 {
            senses.update(&world.view(), &flora.view());
        }
        for _ in 0..600 {
            fauna.step_with_senses(&world, &mut flora, 1, &mut senses);
        }
        let b = fauna.ledger.bites_by_founder_food[GNAWER.index()];
        (b[Food::CapTissue.index()], b[Food::Litter.index()])
    };
    let (caps, _) = run(true);
    assert!(caps > 0, "weighing by yield, it never reached the caps 2 m off");
    let (old_caps, old_litter) = run(false);
    assert!(old_litter > 0 && old_caps == 0, "fixture: the stock rule holds it on litter ({old_caps} cap bites, {old_litter} litter)");
}
