//! **LS-BROWSE's laws** (Wrysk 2026-10-14, "yes on loftstriders"; written before the rule):
//! a loftstrider picks leaves slowly, and a tree's bites shrink as it is stripped.
//!
//! - At one stand a loftstrider's bite is smaller when the stand holds a smaller share of its
//!   own full foliage, with the stand's stock still far above `bite_half_stock`.
//! - With slow picking a loftstrider takes longer to fill from empty at a full stand, and what
//!   it can hold and digest is the same.
//! - Eating at a stand it strips, its offer falls under its remembered intake and it leaves
//!   before the stand is bare.
//! - A lineage without the knobs (the frondgrazer) bites as before: the gut's bite times the
//!   stock's saturation, nothing else.
//!
//! The knobs are set by name through serde (`foliage_handling`, `strip_exponent`), so a build
//! without them reads the same cases with the knobs absent. Every knob value here is the case's
//! own; no shipped number, ratio or world hash is asserted. Hand-built world at the 0.25 m
//! reference voxel: soil at `y = 1..=2`, the support face `y = 2`; the day clock off.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command as FloraCommand, FaceLayer, Flora, FloraConfig, Site, Species as Plant};

use super::{Patch, offer};
use crate::{Actions, Command, Fauna, FaunaConfig, Food, Founder, FounderPhysiology, Scripted, StartingStores};

const V: f64 = 0.25;
const LS: Founder = Founder::Loftstrider;
const FG: Founder = Founder::Browser;
const EAST: f64 = std::f64::consts::FRAC_PI_2;
const HUNGRY: StartingStores = StartingStores { body: 1.0, reserve: 0.0 };

// ------------------------------------------------------------------ fixtures

fn world() -> World {
    let c = VoxelConfig {
        width: 40,
        height: 64,
        depth: 16,
        voxel_m: V,
        seed: 7,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    };
    let mut w = World::empty(c);
    let c = w.config().clone();
    let m = cubarium_voxel::Materials::DEFAULT;
    let cap = m.pore_capacity(Material::Soil) * c.voxel_volume();
    let fc = m.field_capacity(Material::Soil);
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            for y in 1..=2 {
                w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc * cap });
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

/// No fruit, and no induced defence: a bite takes what the mouth asks, so what is under test
/// is the mouth.
fn flora_config() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.vaulttree.fruit_per_s = 0.0;
    c.vaulttree.fruit_cap = 0.0;
    for sp in Plant::ALL {
        c.species_mut(sp).defence_per_bite = 0.0;
    }
    c
}

/// A `species` stand of `wood_fraction × wood_max` seeded on `(x, z)`: its root site.
fn stand(flora: &mut Flora, world: &World, species: Plant, wood_fraction: f64, x: i64, z: u32) -> Site {
    let wood = wood_fraction * flora.config().species(species).wood_max;
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species, wood }), "fixture: seed {species:?}");
    let fv = flora.view();
    fv.stands.iter().find(|s| i64::from(s.site.x) == x && s.site.z == z).expect("the stand").site
}

/// `founder`'s physiology with the LS-BROWSE knobs set by name.
fn knobs(phys: FounderPhysiology, handling: f64, strip: f64) -> FounderPhysiology {
    let mut v = serde_json::to_value(phys).expect("physiology serializes");
    v["foliage_handling"] = serde_json::json!(handling);
    v["strip_exponent"] = serde_json::json!(strip);
    serde_json::from_value(v).expect("physiology deserializes")
}

/// The default table, `founder`'s senses off (its mouth is what is under test), with the
/// loftstrider's knobs.
fn config(handling: f64, strip: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.founders[LS.index()] = knobs(c.founders[LS.index()], handling, strip);
    for f in [LS, FG] {
        c.founders[f.index()].senses.live = false;
    }
    c
}

/// A hungry `founder` at `(x, z)` facing `+x`, its eating latch on: its id.
fn body(fauna: &mut Fauna, world: &World, founder: Founder, x: i64, z: u32) -> u64 {
    assert!(fauna.apply(world, Command::IntroduceFounder { x, z, founder, stores: HUNGRY, heading_rad: EAST }));
    let id = fauna.animals.last().expect("just introduced").id;
    fauna.animals.iter_mut().find(|a| a.id == id).expect("alive").eating = true;
    id
}

/// Hold body `id` still, feeding at full effort.
fn feed_still(fauna: &mut Fauna, id: u64) {
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 0.0, turn: 0.0, feed: 1.0 }]))));
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("alive")
}

/// The mouth's band and columns for body `id`, and the stand its foliage bite would take
/// from: `(root, edible in reach)`.
fn mouth(fauna: &Fauna, world: &World, flora: &Flora, id: u64) -> Option<(Site, f64)> {
    let a = fauna.animals[index(fauna, id)];
    let geometry = crate::body_of(&fauna.config, &a)?;
    let view = world.view();
    let cols = crate::body::mouth_columns(&view, &a.pose, &geometry);
    crate::body::mouth_foliage_offer(&flora.view(), &view, &cols, a.site.y, &geometry, crate::Diet::only(Food::Foliage))
        .map(|(root, _, stock, _)| (root, stock))
}

/// What `founder`'s bites have taken of foliage so far.
fn eaten(fauna: &Fauna, founder: Founder) -> f64 {
    fauna.view().ledger.eaten_by_founder_food[founder.index()][Food::Foliage.index()]
}

/// The first bite a fresh hungry `founder` held still at `(x, z)` takes: `(bite, stock at
/// the mouth before it)`.
fn first_bite(cfg: &FaunaConfig, world: &World, flora: &mut Flora, founder: Founder, x: i64, z: u32) -> (f64, f64) {
    let mut fauna = Fauna::new(cfg.clone());
    let id = body(&mut fauna, world, founder, x, z);
    feed_still(&mut fauna, id);
    for _ in 0..200 {
        let (_, stock) = mouth(&fauna, world, flora, id).expect("fixture: the mouth reaches the stand");
        let before = eaten(&fauna, founder);
        fauna.step(world, flora);
        let bite = eaten(&fauna, founder) - before;
        if bite > 0.0 {
            return (bite, stock);
        }
    }
    panic!("fixture: no bite in 200 ticks");
}

/// The stock's saturation `E / (E + K)` for `founder`'s foliage bite.
fn saturation(cfg: &FaunaConfig, founder: Founder, stock: f64) -> f64 {
    let k = cfg.founder(founder).bite_half_stock_for(Food::Foliage);
    if k > 0.0 { stock / (stock + k) } else { 1.0 }
}

/// Withdraw `fraction` of what the mouth band of a fresh `founder` at `(x, z)` reaches.
fn strip(cfg: &FaunaConfig, world: &World, flora: &mut Flora, founder: Founder, x: i64, z: u32, fraction: f64) {
    let mut fauna = Fauna::new(cfg.clone());
    let id = body(&mut fauna, world, founder, x, z);
    let (root, stock) = mouth(&fauna, world, flora, id).expect("fixture: the mouth reaches the stand");
    let a = fauna.animals[index(&fauna, id)];
    let band = crate::body_of(&fauna.config, &a).expect("a body").mouth_layers(a.site.y, V);
    flora.take_foliage_of(root, FaceLayer::Plant, fraction * stock, &band).expect("fixture: a withdrawal");
}

// ------------------------------------------------------------------ 1. easy leaves first

/// **At one stand a loftstrider's bite is smaller when the stand holds a smaller share of its
/// own full foliage**, with its stock far above `bite_half_stock`: each bite, over the stock's
/// saturation, is smaller than the one before the stand was stripped further.
#[test]
fn a_loftstriders_bite_shrinks_as_its_stand_is_stripped_far_above_bite_half_stock() {
    let cfg = config(1.0, 1.0);
    let world = world();
    let mut flora = Flora::new(flora_config());
    stand(&mut flora, &world, Plant::Vaulttree, 1.0, 20, 8);
    let k = cfg.founder(LS).bite_half_stock_for(Food::Foliage);
    let mut last: Option<f64> = None;
    for step in 0..4 {
        let (bite, stock) = first_bite(&cfg, &world, &mut flora, LS, 16, 8);
        assert!(stock > 50.0 * k, "the stand's stock stays far above bite_half_stock: {stock} vs K {k}");
        let per = bite / saturation(&cfg, LS, stock);
        if let Some(prev) = last {
            assert!(per < prev * (1.0 - 1e-9), "stripped further (step {step}), a smaller bite: {per} vs {prev} (stock {stock})");
        }
        last = Some(per);
        strip(&cfg, &world, &mut flora, LS, 16, 8, 0.3);
    }
}

// ------------------------------------------------------------------ 2. slow picking

/// Ticks a fresh hungry loftstrider held still at a full stand takes until its eating latch
/// lets go (its stomach full), and its stomach's organic then; `None` within `limit`.
fn fill_time(cfg: &FaunaConfig, limit: u64) -> Option<(u64, f64)> {
    let world = world();
    let mut flora = Flora::new(flora_config());
    stand(&mut flora, &world, Plant::Vaulttree, 1.0, 20, 8);
    let mut fauna = Fauna::new(cfg.clone());
    let id = body(&mut fauna, &world, LS, 16, 8);
    feed_still(&mut fauna, id);
    for t in 1..=limit {
        fauna.step(&world, &mut flora);
        let a = fauna.animals[index(&fauna, id)];
        if !a.eating {
            return Some((t, a.stomach.organic()));
        }
    }
    None
}

/// **With slow picking a loftstrider takes longer to fill from empty at a full stand than
/// without, and what it can hold and digest is the same.** Both arms fill (the picking still
/// outruns digestion), the slow one later; their guts are one gut.
#[test]
fn with_slow_picking_a_loftstrider_fills_later_and_holds_and_digests_the_same() {
    let fast = config(1.0, 0.0);
    let slow = config(0.75, 0.0);
    let (pf, ps) = (fast.founder(LS), slow.founder(LS));
    assert_eq!(pf.gut, ps.gut, "one gut: capacity, digestion and bite");
    assert_eq!(pf.core, ps.core, "one body: yields and upkeep");
    let limit = 200_000;
    let (t_fast, s_fast) = fill_time(&fast, limit).expect("the gut's own bite fills the stomach");
    let (t_slow, s_slow) = fill_time(&slow, limit).expect("slow picking still fills the stomach");
    assert!(t_slow > t_fast, "slow picking fills later: {t_slow} vs {t_fast} ticks");
    let cap = pf.gut.capacity(pf.core.body_max);
    assert!((s_slow - s_fast).abs() <= 0.05 * cap, "both let go at the same fill: {s_slow} vs {s_fast} of {cap}");
}

// ------------------------------------------------------------------ 3. the leave rule

/// **A loftstrider eating at a stand that it strips falls under its remembered intake and
/// leaves before the stand is bare.** Its memory starts under what the full stand offers (it
/// walked to get here); its own heuristic holds it still to feed; the leave rule gives the
/// stand up while the stock at its mouth is still far above `bite_half_stock`, before its
/// meal ends.
#[test]
fn a_loftstrider_stripping_its_stand_falls_under_its_memory_and_leaves_before_it_is_bare() {
    let mut cfg = config(1.0, 1.0);
    cfg.founders[LS.index()].senses.live = true;
    cfg.teachers.give_up_window_s = 0.0;
    let world = world();
    let mut flora = Flora::new(flora_config());
    stand(&mut flora, &world, Plant::Vaulttree, 1.0, 20, 8);
    let mut fauna = Fauna::new(cfg.clone());
    let id = body(&mut fauna, &world, LS, 16, 8);
    assert!(fauna.set_controller(id, crate::controller::heuristic(LS, cfg.teachers)));
    let (_, stock) = mouth(&fauna, &world, &flora, id).expect("fixture: the mouth reaches the stand");
    let i = index(&fauna, id);
    let phys = *cfg.founder(LS);
    let period = LS.manifest().controller_period_s;
    let full = offer(&fauna.animals[i], &phys, Food::Foliage, period, 1.0, stock);
    fauna.forage.insert(id, Patch { memory: Some(0.9 * full), ..Patch::default() });
    let k = phys.bite_half_stock_for(Food::Foliage);
    let limit = 200_000;
    for _ in 0..limit {
        fauna.step(&world, &mut flora);
        if fauna.forage_tally()[LS.index()].give_ups > 0 {
            let left = mouth(&fauna, &world, &flora, id).map_or(0.0, |m| m.1);
            assert!(left > 50.0 * k, "left before the stand was bare: {left} at its mouth, K {k}");
            return;
        }
        let a = fauna.animals[index(&fauna, id)];
        assert!(a.eating, "the meal ended at the stand without a leave");
    }
    panic!("no leave in {limit} ticks");
}

// ------------------------------------------------------------------ 4. lineages without the knobs

/// **A lineage without the knobs bites as before**: a frondgrazer's bite at a stand is the
/// gut's full-effort bite times the stock's saturation, full or stripped, the loftstrider's
/// knobs set or not.
#[test]
fn a_frondgrazer_bites_as_before_whatever_the_loftstriders_knobs() {
    let world = world();
    let species = [Plant::Bloomcrown, Plant::Umbrellafrond, Plant::Springturf, Plant::Vaulttree]
        .into_iter()
        .find(|&sp| {
            let mut f = Flora::new(flora_config());
            stand(&mut f, &world, sp, 1.0, 20, 8);
            let mut fauna = Fauna::new(config(1.0, 0.0));
            let id = body(&mut fauna, &world, FG, 19, 8);
            mouth(&fauna, &world, &f, id).is_some()
        })
        .expect("fixture: a stand a frondgrazer's mouth reaches");
    let mut bites: Vec<Vec<f64>> = Vec::new();
    for cfg in [config(1.0, 0.0), config(0.4, 2.0)] {
        let mut flora = Flora::new(flora_config());
        stand(&mut flora, &world, species, 1.0, 20, 8);
        let p = cfg.founder(FG);
        let mut arm = Vec::new();
        for _ in 0..3 {
            let mut fauna = Fauna::new(cfg.clone());
            let id = body(&mut fauna, &world, FG, 19, 8);
            let a = fauna.animals[index(&fauna, id)];
            let want = p.gut.bite_per_body_s * p.gut.throughput_factor(a.body, p.core.body_max) * a.body * FG.manifest().controller_period_s;
            let (bite, stock) = first_bite(&cfg, &world, &mut flora, FG, 19, 8);
            // The gut's bite at the stock's saturation (the body's upkeep moves it by a hair
            // within the tick).
            let expected = (want * saturation(&cfg, FG, stock)).min(stock);
            assert!((bite - expected).abs() <= 1e-5 * expected, "{species:?}: bite {bite} vs the gut's {expected} (stock {stock})");
            arm.push(bite);
            strip(&cfg, &world, &mut flora, FG, 19, 8, 0.5);
        }
        bites.push(arm);
    }
    assert_eq!(bites[0], bites[1], "the loftstrider's knobs leave the frondgrazer's bites exactly as they were");
}

