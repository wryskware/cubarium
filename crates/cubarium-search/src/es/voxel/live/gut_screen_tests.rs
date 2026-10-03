//! The screens and the replenishment books under the minimal gut
//! (`design/metabolism-meals-2026-10-02.md` repair 2 item 11; the search rebase brief
//! `design/handoffs/search-rebase-2026-10-03.md` §4). Each changed screen refuses a
//! hand-built failing case and passes a passing one, at the design's worked numbers:
//! the frondgrazer's f 0.06, k 0.06, s0 0.15, foliage yield 0.5, sleep 0.5; the
//! chorister's f 0.08, k 0.20, carrion yield 0.5. Arithmetic, and one introduced body.

use cubarium_voxel::weather::DayMode;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Command as FaunaCommand, Fauna, FaunaConfig, Food, Founder, StartingStores,
};
use cubarium_voxel_flora::{FloraConfig, Species};

use super::screen::{
    FoodSpec, NightSpec, ScreenSpec, basal_demand_per_day, daily_need, digestible_stomach_per_body,
    mineral_limited_yield, roster,
};
use super::targets::{cycle_books, digestible_stomach, replenishment};

const VOXEL_M: f64 = 0.125;

/// The terrarium's world: 256 x 48 columns of 0.125 m, the 120-min cycle, 0.67 daylight.
/// A diurnal body sleeps one bout of about 2,304 s, so the night guard's need is 1.2 x that.
fn world() -> cubarium_voxel::Config {
    let mut c = cubarium_voxel::Config::default();
    c.width = 256;
    c.depth = 48;
    c.voxel_m = VOXEL_M;
    c.day.mode = DayMode::Cycle;
    c.day.length_min = 120.0;
    c.day.daylight = 0.67;
    c
}

fn only(lineage: Founder, n: usize) -> FaunaConfig {
    let mut f = FaunaConfig::default();
    for l in Founder::ALL {
        f.founders[l.index()].founding_count = 0;
    }
    f.founders[lineage.index()].founding_count = n;
    f
}

fn night_spec() -> ScreenSpec {
    ScreenSpec { night: Some(NightSpec::default()), food: None, ..ScreenSpec::default() }
}

fn food_spec() -> ScreenSpec {
    ScreenSpec { night: None, food: Some(FoodSpec::default()), ..ScreenSpec::default() }
}

/// The design's numbers are the code defaults this file reasons from.
#[test]
fn the_defaults_are_the_designs_worked_numbers() {
    let c = FaunaConfig::default();
    let g = c.founder(Founder::Browser);
    assert!((daily_need(g, &c.life) - 0.06).abs() < 1e-12);
    assert_eq!((g.gut.stomach_k, g.gut.s0, g.sleep_upkeep), (0.06, 0.15, 0.5));
    assert_eq!(g.yield_by_food[Food::Foliage.index()], 0.5);
    let ch = c.founder(Founder::Chorister);
    assert!((daily_need(ch, &c.life) - 0.08).abs() < 1e-12);
    assert_eq!((ch.gut.stomach_k, ch.yield_by_food[Food::Carrion.index()]), (0.20, 0.5));
}

// --- The night guard ------------------------------------------------------------------

/// A stomach of `s0 K` at dusk, digested at the lineage's yield: the frondgrazer's
/// 0.15 x 0.06 x 0.5 = 0.0045 of its body.
#[test]
fn the_digestible_stomach_is_s0_k_y() {
    let c = FaunaConfig::default();
    let d = digestible_stomach_per_body(c.founder(Founder::Browser));
    assert!((d - 0.0045).abs() < 1e-15, "{d}");
    let ch = digestible_stomach_per_body(c.founder(Founder::Chorister));
    assert!((ch - 0.015).abs() < 1e-15, "{ch}");
}

/// The frondgrazer's night, hand-built: woken at reserve hunger 0.215, it goes to sleep at
/// 0.8 of its reserve with only (0.8 − 0.785) x 0.5 = 0.0075 of its body usable. At the
/// gut's upkeep (0.06 / 7,200 s x 0.5 asleep) that alone lasts 1,800 s, short of 1.2 x
/// its 2,304-s night: refused with an empty stomach. Its `s0 K` stomach adds 0.0045:
/// 0.012 lasts 2,880 s and passes. With no usable reserve at all (hunger 0.2), the
/// stomach alone lasts 1,080 s: refused.
#[test]
fn the_night_guard_counts_the_digestible_stomach() {
    let w = world();
    let row = |fauna: &FaunaConfig| {
        let r = roster(&FloraConfig::for_voxel_size(VOXEL_M), fauna, &w, &night_spec());
        let n = r.night.iter().find(|n| n.lineage == "frondgrazer").expect("a row").clone();
        (n, r.refusals())
    };
    let mut fauna = only(Founder::Browser, 8);
    fauna.founders[Founder::Browser.index()].wake_hunger = 0.215;

    // An empty stomach (s0 0): the reserve alone.
    let mut empty = fauna;
    empty.founders[Founder::Browser.index()].gut.s0 = 0.0;
    let (n, refusals) = row(&empty);
    assert!((n.endurance_s - 1800.0).abs() < 1e-6, "{n:?}");
    assert!(!n.passes && refusals.iter().any(|r| r.starts_with("night") && r.contains("frondgrazer")));

    // The design's s0 0.15: the stomach carries it through.
    let (n, refusals) = row(&fauna);
    assert!((n.endurance_s - 2880.0).abs() < 1e-6, "{n:?}");
    assert!((n.sleep_s - 2304.0).abs() < 1.0, "{n:?}");
    assert!(n.passes && refusals.is_empty(), "{n:?} {refusals:?}");

    // No usable reserve: the stomach alone is not a night.
    fauna.founders[Founder::Browser.index()].wake_hunger = 0.2;
    let (n, refusals) = row(&fauna);
    assert!((n.endurance_s - 1080.0).abs() < 1e-6, "{n:?}");
    assert!(!n.passes && !refusals.is_empty());
}

// --- Food access: the gut's daily need ------------------------------------------------

/// Demand is the daily need `f` over the rhythm (the design's `E`): a frondgrazer burns
/// 0.06 x (4,896 + 0.5 x 2,304) / 7,200 = 0.0504 of its 1.5 body a world day, 0.0756, and
/// eight of them 0.6048. The upkeep the old screen read was ~100x that.
#[test]
fn food_demand_is_the_guts_daily_need() {
    let c = FaunaConfig::default();
    let g = c.founder(Founder::Browser);
    let one = basal_demand_per_day(g, &c.life, 4896.0, 2304.0);
    assert!((one - 0.0504 * 1.5).abs() < 1e-12, "{one}");
    let r = roster(&FloraConfig::for_voxel_size(VOXEL_M), &only(Founder::Browser, 8), &world(), &food_spec());
    let row = r.food.iter().find(|x| x.lineage == "frondgrazer").expect("a row");
    assert!((row.need_per_day - 8.0 * 0.0756).abs() < 1e-3, "{row:?}");
}

/// The seedporter on fruit: at the gut's need four of them pass on the terrarium's
/// fruit; with every fruit source cut to a thousandth they are short (soft, not refused);
/// with none at all, refused.
#[test]
fn the_food_screen_judges_supply_against_the_daily_need() {
    let w = world();
    let fauna = only(Founder::Seedporter, 4);
    let flora = FloraConfig::for_voxel_size(VOXEL_M);
    let r = roster(&flora, &fauna, &w, &food_spec());
    let row = r.food.iter().find(|x| x.lineage == "seedporter").expect("a row");
    assert!(row.passes && row.ratio > 1.0, "{row:?}");
    let need = 4.0 * basal_demand_per_day(fauna.founder(Founder::Seedporter), &fauna.life, 4896.0, 2304.0);
    assert!((row.need_per_day - need).abs() < 1e-3 * need, "{row:?} vs {need}");

    let mut short = flora.clone();
    for sp in Species::ALL {
        short.species_mut(sp).fruit_per_s *= 1e-3;
    }
    short.latticevine.fruit_per_face_s *= 1e-3;
    let r = roster(&short, &fauna, &w, &food_spec());
    let row = r.food.iter().find(|x| x.lineage == "seedporter").expect("a row");
    assert!(!row.passes && row.ratio > 0.0 && row.ratio < 1.0, "{row:?}");
    assert!(r.refusals().is_empty() && r.soft(5.0) > 0.0);
}

/// Vine products carry the vine's mineral (repair 3 item 2), and what a product's mineral
/// cannot fund passes undigested (item 1). The seedporter (yield 0.6, n_tissue 0.05) on
/// latticevine fruit alone: at the vine's 0.01 mineral the fruit funds a yield of 0.2 (a
/// third of its 0.6); mineral-free, it feeds nothing and the seedporter is refused. (The
/// founding faces alone fed one seedporter while the screen counted every face fruiting
/// back to back; at the vine's production knob, plants-flowering-vines 2026-10-03, they
/// give a quarter of its need: short, not refused.)
#[test]
fn vine_fruit_feeds_at_the_yield_its_mineral_funds() {
    assert!((mineral_limited_yield(0.6, 0.01, 0.05) - 0.2).abs() < 1e-15);
    assert_eq!(mineral_limited_yield(0.5, 0.01, 0.02), 0.5, "the bellwing's nectar is fully funded");
    assert_eq!(mineral_limited_yield(0.6, 0.0, 0.05), 0.0);
    let w = world();
    let fauna = only(Founder::Seedporter, 1);
    let mut vine_only = FloraConfig::for_voxel_size(VOXEL_M);
    for sp in Species::ALL {
        vine_only.species_mut(sp).fruit_per_s = 0.0;
    }
    assert_eq!(vine_only.latticevine.mineral_fraction, 0.01);
    let r = roster(&vine_only, &fauna, &w, &food_spec());
    let row = r.food.iter().find(|x| x.lineage == "seedporter").expect("a row");
    let fruit = &row.foods[0];
    assert!(fruit.per_day > 0.0, "{row:?}");
    assert!((fruit.assimilable_per_day - 0.2 * fruit.per_day).abs() < 1e-12 * fruit.per_day, "{row:?}");
    assert!(row.ratio > 0.0, "{row:?}");
    assert!(!r.refusals().iter().any(|x| x.starts_with("food") && x.contains("seedporter")), "{:?}", r.refusals());
    let mut bare = vine_only.clone();
    bare.latticevine.mineral_fraction = 0.0;
    let r = roster(&bare, &fauna, &w, &food_spec());
    assert!(r.refusals().iter().any(|x| x.starts_with("food") && x.contains("seedporter")), "{:?}", r.refusals());
}

// --- Replenishment: reserve plus digestible stomach -----------------------------------

/// A chorister (body B 2.744) starts the cycle with a full stomach, K = 0.2 B raw, worth
/// 0.1 B to its reserve, and eats nothing all day. It digests what it burns, 0.0672 B
/// (f 0.08 over its rhythm), so its reserve and body stand still. Counting only the
/// reserve, that digestion looks like income and the day replenishes exactly 1: met. The
/// gut's books count the stomach as stock, so the day's income is 0: not met. The same
/// chorister eating one meal that refills its stomach replenishes 1: met.
#[test]
fn replenishment_counts_the_digestible_stomach() {
    let b = FaunaConfig::default().founder(Founder::Chorister).core.body_max;
    let burned = 0.0672 * b;
    let (full, end) = (0.1 * b, 0.1 * b - burned);
    let biomass = 2.0 * b;

    // The reserve-only books: digestion is income.
    assert_eq!(replenishment(burned, biomass, biomass), 1.0);

    // The gut's books: moving food from stomach to reserve is not income.
    let (intake, start, stop) = cycle_books(burned, biomass, biomass, full, end);
    assert!(intake.abs() < 1e-12, "{intake}");
    assert_eq!(replenishment(intake, start, stop), 0.0);

    // One meal that refills the stomach: income equals spending.
    let (intake, start, stop) = cycle_books(burned, biomass, biomass, full, full);
    assert!((intake - burned).abs() < 1e-15);
    assert!(replenishment(intake, start, stop) >= 1.0);
}

fn flat_world() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 6,
        depth: 6,
        voxel_m: 0.25,
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    world
}

/// A founded chorister arrives with `s0 K` of carrion at its tissue's mineral density:
/// worth 0.15 x 0.2 x B x 0.5 to its reserve. Short of mineral, the yield is what the
/// mineral can fund, as digestion counts it.
#[test]
fn a_bodys_digestible_stomach_is_its_pools_at_their_effective_yield() {
    let world = flat_world();
    let config = FaunaConfig::default();
    let mut fauna = Fauna::new(config);
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 3,
            z: 3,
            founder: Founder::Chorister,
            stores: StartingStores::FULL,
            heading_rad: 0.0,
        }
    ));
    let mut a = fauna.view().animals[0].clone();
    let p = config.founder(Founder::Chorister);
    let want = 0.15 * 0.2 * a.body * 0.5;
    let got = digestible_stomach(&a, p);
    assert!((got - want).abs() < 1e-12, "{got} vs {want}");
    // A quarter of the mineral funds a quarter of the tissue: yield 0.25, not 0.5.
    a.stomach.pools[Food::Carrion.index()].mineral *= 0.25;
    let short = digestible_stomach(&a, p);
    assert!((short - want / 2.0).abs() < 1e-12, "{short} vs {}", want / 2.0);
}
