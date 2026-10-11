//! The screens and the replenishment books under the minimal gut
//! (`design/metabolism-meals-2026-10-02.md` repair 2 item 11; the search rebase brief
//! `design/handoffs/search-rebase-2026-10-03.md` §4). Each changed screen refuses a
//! hand-built failing case and passes a passing one. The physiology each test needs is set
//! by hand on a clone of the config (never read as a tuned value), and expectations are the
//! formulas over those inputs. Arithmetic, and one introduced body.

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

/// A physiology with the gut set by hand: every food of the diet digests at `y`.
fn hand_gut(c: &mut FaunaConfig, lineage: Founder, s0: f64, k: f64, y: f64) {
    let p = &mut c.founders[lineage.index()];
    p.gut.s0 = s0;
    p.gut.stomach_k = k;
    for f in Food::ALL {
        p.yield_by_food[f.index()] = y;
    }
}

// --- The night guard ------------------------------------------------------------------

/// A stomach of `s0 K` at dusk, digested at the lineage's yield: `s0 x k x y` of its body.
#[test]
fn the_digestible_stomach_is_s0_k_y() {
    let mut c = FaunaConfig::default();
    hand_gut(&mut c, Founder::Browser, 0.15, 0.06, 0.5);
    hand_gut(&mut c, Founder::Chorister, 0.25, 0.2, 0.6);
    let d = digestible_stomach_per_body(c.founder(Founder::Browser));
    assert!((d - 0.15 * 0.06 * 0.5).abs() < 1e-15, "{d}");
    let ch = digestible_stomach_per_body(c.founder(Founder::Chorister));
    assert!((ch - 0.25 * 0.2 * 0.6).abs() < 1e-15, "{ch}");
}

/// A night, hand-built. Reserve cap 1, sleep multiplier 0.5, wake hunger set so the usable
/// reserve at the screen's onset fraction is `R = 0.1`, and a stomach worth `R / 2`
/// (`s0 x k x y`). The upkeep is set so the reserve alone lasts 0.75 of the night guard's
/// need: refused with an empty stomach; with the stomach (1.125 of the need) it passes; with no
/// usable reserve the stomach alone (0.375) is refused.
#[test]
fn the_night_guard_counts_the_digestible_stomach() {
    let w = world();
    let spec = night_spec();
    let night = spec.night.expect("night spec");
    let row = |fauna: &FaunaConfig| {
        let r = roster(&FloraConfig::for_voxel_size(VOXEL_M), fauna, &w, &spec);
        let n = r.night.iter().find(|n| n.lineage == "frondgrazer").expect("a row").clone();
        (n, r.refusals())
    };
    let i = Founder::Browser.index();
    let mut fauna = only(Founder::Browser, 8);
    // The sleep bout is the world's and the body's rhythm: read it off a probe.
    let sleep = row(&fauna).0.sleep_s;
    assert!(sleep > 0.0);
    let need = night.margin * sleep;
    let r = 0.1;
    let (k, y) = (0.2, 0.5);
    {
        let p = &mut fauna.founders[i];
        p.core.reserve_cap = 1.0;
        p.sleep_upkeep = 0.5;
        p.wake_hunger = 1.0 - night.f0 + r;
        p.core.maintenance_per_s = r / (0.75 * need) / p.sleep_upkeep;
    }
    hand_gut(&mut fauna, Founder::Browser, 0.5 * r / (k * y), k, y);
    let burn = fauna.founders[i].core.maintenance_per_s * fauna.founders[i].sleep_upkeep;

    // An empty stomach (s0 0): the reserve alone.
    let mut empty = fauna.clone();
    empty.founders[i].gut.s0 = 0.0;
    let (n, refusals) = row(&empty);
    assert!((n.endurance_s - r / burn).abs() < 1e-6 * need, "{n:?}");
    assert!(!n.passes && refusals.iter().any(|x| x.starts_with("night") && x.contains("frondgrazer")));

    // The stomach carries it through.
    let (n, refusals) = row(&fauna);
    assert!((n.endurance_s - 1.5 * r / burn).abs() < 1e-6 * need, "{n:?}");
    assert!((n.sleep_s - sleep).abs() < 1e-9, "{n:?}");
    assert!(n.passes && refusals.is_empty(), "{n:?} {refusals:?}");

    // No usable reserve: the stomach alone is not a night.
    fauna.founders[i].wake_hunger = 1.0 - night.f0;
    let (n, refusals) = row(&fauna);
    assert!((n.endurance_s - 0.5 * r / burn).abs() < 1e-6 * need, "{n:?}");
    assert!(!n.passes && !refusals.is_empty());
}

// --- Food access: the gut's daily need ------------------------------------------------

/// Demand is the daily need `f` over the rhythm (the design's `E`): a body burns `f` of its
/// adult body a world day, awake at 1 and asleep at the sleep multiplier, and the screen's
/// row is the bodies' sum. Physiology set by hand.
#[test]
fn food_demand_is_the_guts_daily_need() {
    let i = Founder::Browser.index();
    let mut c = only(Founder::Browser, 8);
    c.founders[i].core.maintenance_per_s = 1e-5;
    c.founders[i].core.body_max = 1.5;
    c.founders[i].sleep_upkeep = 0.5;
    // One night bout, so the night row's longest fast is also its whole sleep (a leaning
    // rhythm sleeps in many bouts: `screen::founder_sleep`).
    c.founders[i].active = cubarium_voxel_fauna::Activity::Diurnal;
    let g = c.founder(Founder::Browser);
    let f = daily_need(g, &c.life);
    assert!((f - 1e-5 * c.life.ecological_day_s).abs() < 1e-15);
    let (awake, asleep) = (4000.0, 2000.0);
    let one = basal_demand_per_day(g, &c.life, awake, asleep);
    let want = f * 1.5 * (awake + 0.5 * asleep) / c.life.ecological_day_s;
    assert!((one - want).abs() < 1e-12, "{one} vs {want}");
    // The row: eight bodies over the world's own awake and asleep seconds.
    let spec = ScreenSpec { night: Some(NightSpec::default()), food: Some(FoodSpec::default()), ..ScreenSpec::default() };
    let w = world();
    let r = roster(&FloraConfig::for_voxel_size(VOXEL_M), &c, &w, &spec);
    let sleep = r.night.iter().find(|x| x.lineage == "frondgrazer").expect("a night row").sleep_s;
    let row = r.food.iter().find(|x| x.lineage == "frondgrazer").expect("a row");
    let day = w.day.length_min * 60.0;
    let want = 8.0 * basal_demand_per_day(g, &c.life, day - sleep, sleep);
    assert!((row.need_per_day - want).abs() < 1e-6 * want, "{row:?} vs {want}");
}

/// The seedporter on fruit: the screen's need is the gut's, its ratio is supply over need
/// (so it halves with twice the bodies), a passing ratio passes, and fruit cut until the
/// ratio is half is short (soft, not refused); with none at all, refused.
#[test]
fn the_food_screen_judges_supply_against_the_daily_need() {
    let w = world();
    let fauna = only(Founder::Seedporter, 4);
    let flora = FloraConfig::for_voxel_size(VOXEL_M);
    let seed_row = |flora: &FloraConfig, fauna: &FaunaConfig| {
        let r = roster(flora, fauna, &w, &food_spec());
        let row = r.food.iter().find(|x| x.lineage == "seedporter").expect("a row").clone();
        (row, r)
    };
    let (row, _) = seed_row(&flora, &fauna);
    assert!(row.ratio > 0.0, "the terrarium's fruit must feed something for this test: {row:?}");
    assert_eq!(row.passes, row.ratio >= 1.0, "{row:?}");
    let (twice, _) = seed_row(&flora, &only(Founder::Seedporter, 8));
    assert!((twice.need_per_day - 2.0 * row.need_per_day).abs() < 1e-9 * row.need_per_day, "{twice:?}");
    assert!((twice.ratio - 0.5 * row.ratio).abs() < 1e-9 * row.ratio, "{twice:?}");

    let cut = 0.5 / row.ratio;
    let mut short = flora.clone();
    for sp in Species::ALL {
        short.species_mut(sp).fruit_per_s *= cut;
    }
    short.latticevine.fruit_per_face_s *= cut;
    let (srow, r) = seed_row(&short, &fauna);
    assert!(!srow.passes && srow.ratio > 0.0 && srow.ratio < 1.0, "{srow:?}");
    assert!(r.refusals().is_empty() && r.soft(5.0) > 0.0);

    let mut none = flora.clone();
    for sp in Species::ALL {
        none.species_mut(sp).fruit_per_s = 0.0;
    }
    none.latticevine.fruit_per_face_s = 0.0;
    let (nrow, r) = seed_row(&none, &fauna);
    assert!(!nrow.passes && nrow.ratio == 0.0, "{nrow:?}");
    assert!(r.refusals().iter().any(|x| x.starts_with("food") && x.contains("seedporter")));
}

/// Product mineral limits tissue, while its absorbed sugar can pay running costs.
/// The basal food screen must still refuse absent food or a mineral-free diet with
/// fuel disabled, but it can fund upkeep from mineral-free fruit with fuel enabled.
#[test]
fn vine_fruit_funds_running_costs_with_fuel() {
    let (yield_, mineral, tissue) = (0.8, 0.02, 0.1);
    let funded = mineral_limited_yield(yield_, mineral, tissue, 0.0);
    let fueled = mineral_limited_yield(yield_, mineral, tissue, 0.3);
    assert!(fueled > funded);
    assert_eq!(fueled, yield_);
    assert_eq!(mineral_limited_yield(yield_, 0.0, tissue, 0.0), 0.0);
    assert_eq!(mineral_limited_yield(yield_, 0.0, tissue, 0.3), yield_);
    assert_eq!(mineral_limited_yield(yield_, tissue, tissue, 0.0), yield_);
    let w = world();
    let mut fauna = only(Founder::Seedporter, 1);
    let p = &mut fauna.founders[Founder::Seedporter.index()];
    p.yield_by_food[Food::Fruit.index()] = yield_;
    p.core.n_tissue = tissue;
    p.core.fuel_cap = 0.3;
    let mut vine_only = FloraConfig::for_voxel_size(VOXEL_M);
    for sp in Species::ALL {
        vine_only.species_mut(sp).fruit_per_s = 0.0;
    }
    vine_only.latticevine.mineral_fraction = 0.0;
    let r = roster(&vine_only, &fauna, &w, &food_spec());
    let row = r.food.iter().find(|x| x.lineage == "seedporter").expect("a row");
    let fruit = row.foods.iter().find(|x| x.food == Food::Fruit.name()).expect("fruit supply");
    assert!(fruit.per_day > 0.0);
    assert!((fruit.assimilable_per_day - yield_ * fruit.per_day).abs() < 1e-12 * fruit.per_day);
    assert!(row.ratio > 0.0);
    fauna.founders[Founder::Seedporter.index()].core.fuel_cap = 0.0;
    let r = roster(&vine_only, &fauna, &w, &food_spec());
    assert!(r.refusals().iter().any(|x| x.starts_with("food") && x.contains("seedporter")));
}

// --- Replenishment: reserve plus digestible stomach -----------------------------------

/// A body (B 2.744, hand-set) starts the cycle with a full stomach worth 0.1 B to its reserve,
/// and eats nothing all day. It digests what it burns, 0.0672 B, so its reserve and body stand still. Counting only the
/// reserve, that digestion looks like income and the day replenishes exactly 1: met. The
/// gut's books count the stomach as stock, so the day's income is 0: not met. The same
/// chorister eating one meal that refills its stomach replenishes 1: met.
#[test]
fn replenishment_counts_the_digestible_stomach() {
    let b = 2.744;
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
/// worth `s0 x k x B x y` to its reserve (gut and yield set by hand). Short of mineral, the
/// reserve share is what the mineral can fund; fuel preserves the remaining energy.
#[test]
fn a_bodys_digestible_stomach_is_its_pools_at_their_effective_yield() {
    let world = flat_world();
    let mut config = FaunaConfig::default();
    hand_gut(&mut config, Founder::Chorister, 0.15, 0.2, 0.5);
    let mut fauna = Fauna::new(config.clone());
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
    config.founders[Founder::Chorister.index()].core.fuel_cap = 0.3;
    let p = config.founder(Founder::Chorister);
    let want = 0.15 * 0.2 * a.body * 0.5;
    let got = digestible_stomach(&a, p);
    assert!((got - want).abs() < 1e-12, "{got} vs {want}");
    // Reducing mineral changes the tissue share, while fuel keeps the energy usable.
    a.stomach.pools[Food::Carrion.index()].mineral *= 0.25;
    let short = digestible_stomach(&a, p);
    assert!((short - want).abs() < 1e-12, "{short} vs {want}");
    config.founders[Founder::Chorister.index()].core.fuel_cap = 0.0;
    let unfueled = digestible_stomach(&a, config.founder(Founder::Chorister));
    assert!((unfueled - want / 2.0).abs() < 1e-12);
    let before = super::window::body_energy(&a);
    let stores = super::balance::stores(std::slice::from_ref(&a), &config, Founder::Chorister);
    a.fuel = 0.03 * a.body;
    assert!((super::window::body_energy(&a) - before - a.fuel).abs() < 1e-12);
    assert!(
        (super::balance::stores(std::slice::from_ref(&a), &config, Founder::Chorister)
            - stores
            - a.fuel)
            .abs()
            < 1e-12
    );
}
