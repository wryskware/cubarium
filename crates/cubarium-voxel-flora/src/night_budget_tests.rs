//! The plants' night budget (`design/ecology-timescales-2026-10-02.md` Part 1b; brief
//! `design/handoffs/night-survival-2026-10-02.md`, tests 1–4). A child of `step`, so a case
//! can grow one stand under a climate it sets: the night, the time to dawn, no light.
//!
//! The budget, per second, while the sun is down on the ecological clock:
//! `max(0, Q − maintenance to dawn − night_floor · Q_cap) / max(to dawn, night_min_s)`,
//! spent on the leaf reflush (below `reflush_below`), then nectar, then fruit; the
//! reflush's construction respiration is charged to it.

use super::*;
use crate::{Command, Flora, FloraConfig, Ground, Species, Stand};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

const BC: Species = Species::Bloomcrown;

/// Plant night rule factor at full night (`DayClock::plant_night_factor`'s default).
const NIGHT: f64 = 0.05;

/// The default table with the temperature factor off (`upkeep_q10` 1, so maintenance is
/// the species rate × the night rule × wood), and the bloomcrown hanging fruit as well as
/// making its nectar, so one stand exercises all three draws.
fn config() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.upkeep_q10 = 1.0;
    let sc = c.species_mut(BC);
    sc.fruit_per_s = 1e-4;
    sc.fruit_cap = 0.02;
    sc.fruit_drop_s = 1e9;
    sc.fruit_mineral_fraction = 0.01;
    c
}

/// A soil slab under open sky (`step::tests::slab`'s build) on the day clock `day`.
fn slab(day: DayClock) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 4,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
        day,
        ..VoxelConfig::default()
    });
    for x in 0..4i64 {
        for y in 1..=2u32 {
            let want = 0.6 * w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z: 0,
                volume_m3: want,
            });
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Soil,
            });
        }
    }
    w
}

/// An adult bloomcrown seeded on `world`, browsed (when asked) to `browse_to` of its
/// foliage capacity through the layer's own withdrawal.
fn seeded(config: &FloraConfig, world: &World, browse_to: Option<f64>) -> Flora {
    let mut flora = Flora::new(config.clone());
    let sc = config.species(BC);
    assert!(flora.apply(
        world,
        Command::Seed {
            x: 1,
            z: 0,
            species: BC,
            wood: sc.wood_max,
        }
    ));
    if let Some(f) = browse_to {
        let s = flora.stands[0];
        let want = s.foliage - f * sc.alpha * s.wood;
        assert!(want > 0.0, "fixture: browsing takes something");
        assert!(flora.take_foliage(s.site, want).is_some());
    }
    assert!(flora.stands[0].profile_stage >= 1, "fixture: an adult stand");
    flora
}

/// One adult bloomcrown, cloned out with a ground slot, its reserve set to `reserve_frac`
/// of its cap and its foliage browsed to `browse_to` of its capacity (or left).
fn stand(config: &FloraConfig, reserve_frac: f64, browse_to: Option<f64>) -> (Stand, Ground) {
    let world = slab(DayClock::default());
    let flora = seeded(config, &world, browse_to);
    let mut s = flora.stands[0];
    s.reserve = reserve_frac * config.species(BC).reserve_cap * s.wood;
    s.nectar = 0.0;
    s.fruit = 0.0;
    (s, Ground::new(s.site, 1.0))
}

/// Full night, `to_dawn` seconds before the ecological dawn.
fn night(to_dawn: f64) -> Climate {
    Climate {
        daylight: 0.0,
        upkeep: 1.0,
        night: NIGHT,
        to_dawn_s: Some(to_dawn),
        draw_key: 0,
    }
}

/// One tick of `grow_stand` in the dark; its books.
fn tick(config: &FloraConfig, s: &mut Stand, g: &mut Ground, climate: Climate) -> Booked {
    let mut book = Booked::default();
    let died = grow_stand(
        config,
        s,
        g,
        0.0,
        climate,
        Drink::default(),
        Substrate::default(),
        &mut book,
    );
    assert!(!died, "fixture: the stand lives");
    book
}

/// This tick's maintenance (`upkeep_q10` 1): species rate × night rule × wood × DT.
fn maintenance(config: &FloraConfig, s: &Stand, night: f64) -> f64 {
    config.species(BC).maintenance * night * s.wood * DT
}

/// The budget formula, per tick, for a stand that starts the tick at `s` (maintenance is
/// paid first, then the rest of the night's maintenance is held back with the floor).
fn allowance(config: &FloraConfig, s: &Stand, to_dawn: f64) -> f64 {
    let sc = config.species(BC);
    let m = maintenance(config, s, NIGHT);
    let q = s.reserve - m;
    let hold = m / DT * to_dawn + sc.night_floor * sc.reserve_cap * s.wood;
    (q - hold).max(0.0) / to_dawn.max(sc.night_min_s) * DT
}

/// What one tick spent on the reflush (with its construction respiration), nectar and
/// fruit, read off the stand: the reserve's fall beyond maintenance, the stocks' rise.
fn spent(config: &FloraConfig, before: &Stand, after: &Stand) -> (f64, f64, f64, f64) {
    let total = before.reserve - maintenance(config, before, NIGHT) - after.reserve;
    let nectar = after.nectar - before.nectar;
    let fruit = after.fruit - before.fruit;
    (total, total - nectar - fruit, nectar, fruit)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1e-12)
}

#[test]
fn the_night_knobs_have_their_defaults() {
    let c = FloraConfig::default();
    for s in Species::ALL {
        let sc = c.species(s);
        assert_eq!(sc.night_bloom_scale, 0.3, "{s:?}");
        assert!(sc.night_floor > 0.0 && sc.night_floor < 1.0, "{s:?}: a floor, a fraction of the cap");
        assert!(sc.night_min_s > 0.0, "{s:?}: a positive floor on the time to dawn");
    }
}

/// The ecological dawn: under `cycle` the cycle's sunrise, `None` by day and with the
/// clock off.
#[test]
fn the_time_to_dawn_is_the_ecological_clocks() {
    let day = DayClock::default();
    let at = |h: f64| {
        slab(DayClock {
            start_hour: h,
            ..DayClock::default()
        })
    };
    let (rise, _) = day.cycle_rise_set();
    let midnight = at(0.0).life_to_dawn_s().expect("the sun is down at midnight");
    assert!(close(midnight, rise * day.length_min * 60.0), "{midnight}");
    assert_eq!(at(12.0).life_to_dawn_s(), None, "noon");
    let off = slab(DayClock {
        mode: DayMode::Off,
        start_hour: 0.0,
        ..DayClock::default()
    });
    assert_eq!(off.life_to_dawn_s(), None, "no night with the clock off");
}

// ------------------------------------------------------------------ test 2: bounded spend

/// A stand with its reserve below the dawn floor spends nothing at night: no leaves, no
/// nectar, no fruit; the reserve pays maintenance only.
#[test]
fn below_the_dawn_floor_a_stand_spends_nothing_at_night() {
    let c = config();
    let floor = c.species(BC).night_floor;
    let (mut s, mut g) = stand(&c, 0.5 * floor, Some(0.1));
    let before = s;
    let book = tick(&c, &mut s, &mut g, night(1200.0));
    let (total, reflush, nectar, fruit) = spent(&c, &before, &s);
    assert!(total.abs() < 1e-15, "spent {total}");
    assert_eq!((nectar, fruit), (0.0, 0.0));
    assert!(reflush.abs() < 1e-15);
    assert!(s.foliage <= before.foliage, "no leaves built");
    assert_eq!(book.night, [0.0; 3]);
}

/// A stand with plenty of reserve, browsed, its nectar and fruit empty, stepped tick by
/// tick through a whole night: each tick spends at most the formula's allowance, the
/// books of the three draws add to the reserve's fall, and the reserve never drops below
/// the floor before dawn.
#[test]
fn a_rich_stand_spends_within_the_formula_and_keeps_its_floor_to_dawn() {
    let c = config();
    let sc = c.species(BC).clone();
    // Plenty, and a tight one: the floor and the night's maintenance plus 2 % of the cap,
    // where the budget binds every tick. ("Plenty" means at least the floor and the
    // maintenance to dawn: below that, maintenance alone takes a stand under its floor,
    // with nothing spent.)
    let night_s = 2400.0;
    let maintenance_frac = sc.maintenance * NIGHT * night_s / sc.reserve_cap;
    for start in [1.0, sc.night_floor + maintenance_frac + 0.02] {
        let (mut s, mut g) = stand(&c, start, Some(0.1));
        let ticks = (night_s / DT).round() as u64;
        let mut total_spent = 0.0;
        for k in 0..ticks {
            let to_dawn = night_s - k as f64 * DT;
            let before = s;
            let allowed = allowance(&c, &before, to_dawn);
            let book = tick(&c, &mut s, &mut g, night(to_dawn));
            let (total, reflush, nectar, fruit) = spent(&c, &before, &s);
            assert!(
                total <= allowed * (1.0 + 1e-9) + 1e-18,
                "start {start}, tick {k}: spent {total} over the allowance {allowed}"
            );
            let booked: f64 = book.night.iter().sum();
            assert!(close(booked, total) || (booked - total).abs() < 1e-15, "booked {booked}, spent {total}");
            // Stock differences round at the stock's magnitude (~1e-2), hence 1e-15.
            assert!((book.night[1] - nectar).abs() < 1e-15, "nectar booked {}, made {nectar}", book.night[1]);
            assert!((book.night[2] - fruit).abs() < 1e-15, "fruit booked {}, made {fruit}", book.night[2]);
            assert!(reflush >= -1e-15);
            let floor = sc.night_floor * sc.reserve_cap * s.wood;
            assert!(
                s.reserve >= floor * (1.0 - 1e-9),
                "start {start}, tick {k}: reserve {} under the floor {floor}",
                s.reserve
            );
            total_spent += total;
        }
        assert!(total_spent > 0.0, "start {start}: the budget was spent at all");
    }
}

// ------------------------------------------------------------------ test 3: priority

/// Browsed below `reflush_below`: the reflush is paid first. With the allowance smaller
/// than the tick's reflush the whole allowance goes to leaves and nothing to nectar or
/// fruit; with room for the reflush and half the nectar, the reflush is whole, the nectar
/// gets the rest, and fruit nothing; with plenty, all three at their rates.
#[test]
fn the_reflush_is_paid_before_nectar_and_fruit() {
    // A short night floor, so a reserve within the cap can fund a whole tick's reflush.
    let mut c = config();
    c.species_mut(BC).night_min_s = 50.0;
    let sc = c.species(BC).clone();
    let to_dawn = 2.0 * sc.night_min_s;
    let (probe, _) = stand(&c, 1.0, Some(0.1));
    let p_cap = sc.alpha * probe.wood;
    let reflush_amount = (sc.foliage_rate * probe.wood * DT).min(sc.reflush_below * p_cap - probe.foliage);
    let reflush_cost = (1.0 + sc.build) * reflush_amount;
    let night_nectar = sc.nectar_per_s * sc.night_bloom_scale * DT;
    let night_fruit = sc.fruit_per_s * sc.night_bloom_scale * DT;
    assert!(reflush_amount > 0.0 && night_nectar > 0.0 && night_fruit > 0.0);
    // The reserve fraction whose allowance is `want` per tick.
    let reserve_for = |want: f64| {
        let m = maintenance(&c, &probe, NIGHT);
        let q = want / DT * to_dawn.max(sc.night_min_s)
            + m / DT * to_dawn
            + sc.night_floor * sc.reserve_cap * probe.wood
            + m;
        q / (sc.reserve_cap * probe.wood)
    };
    for (case, want, expect) in [
        ("half the reflush", 0.5 * reflush_cost, (0.5 * reflush_cost, 0.0, 0.0)),
        (
            "the reflush and half the nectar",
            reflush_cost + 0.5 * night_nectar,
            (reflush_cost, 0.5 * night_nectar, 0.0),
        ),
        ("plenty", 1.0, (reflush_cost, night_nectar, night_fruit)),
    ] {
        let frac = if want >= 1.0 { 1.0 } else { reserve_for(want) };
        assert!(frac <= 1.0, "{case}: fixture reserve {frac} within the cap");
        let (mut s, mut g) = stand(&c, frac, Some(0.1));
        let before = s;
        if want < 1.0 {
            assert!(close(allowance(&c, &before, to_dawn), want), "{case}: the fixture's allowance");
        } else {
            assert!(
                allowance(&c, &before, to_dawn) > reflush_cost + night_nectar + night_fruit,
                "{case}: the fixture's allowance covers all three"
            );
        }
        let book = tick(&c, &mut s, &mut g, night(to_dawn));
        let (_, reflush, nectar, fruit) = spent(&c, &before, &s);
        assert!(close(reflush, expect.0), "{case}: reflush {reflush}, want {}", expect.0);
        assert!(close(nectar, expect.1) || (nectar - expect.1).abs() < 1e-15, "{case}: nectar {nectar}, want {}", expect.1);
        assert!(close(fruit, expect.2) || (fruit - expect.2).abs() < 1e-15, "{case}: fruit {fruit}, want {}", expect.2);
        assert!(close(book.night[0], reflush), "{case}: the reflush booked with its respiration");
    }
}

/// Above `reflush_below` no reserve goes to leaves at night, and nectar and fruit draw at
/// `night_bloom_scale` × their day rates, capped by their stocks.
#[test]
fn above_the_reflush_line_nectar_and_fruit_draw_at_the_night_scale_capped_by_stock() {
    let c = config();
    let sc = c.species(BC).clone();
    let (mut s, mut g) = stand(&c, 1.0, None);
    assert!(s.foliage >= sc.reflush_below * sc.alpha * s.wood, "fixture: leafy");
    let before = s;
    tick(&c, &mut s, &mut g, night(1800.0));
    let (_, reflush, nectar, fruit) = spent(&c, &before, &s);
    assert!(reflush.abs() < 1e-15, "no reflush above the line: {reflush}");
    assert!(close(nectar, sc.nectar_per_s * sc.night_bloom_scale * DT), "nectar {nectar}");
    assert!(close(fruit, sc.fruit_per_s * sc.night_bloom_scale * DT), "fruit {fruit}");

    // Nearly full stocks: the room caps the draw.
    let (mut s, mut g) = stand(&c, 1.0, None);
    s.nectar = sc.nectar_cap - 1e-7;
    s.fruit = sc.fruit_cap - 2e-7;
    let before = s;
    tick(&c, &mut s, &mut g, night(1800.0));
    let (_, _, nectar, fruit) = spent(&c, &before, &s);
    assert!((nectar - 1e-7).abs() < 1e-15, "nectar to its cap: {nectar}");
    assert!((fruit - 2e-7).abs() < 1e-15, "fruit to its cap: {fruit}");
}

// ------------------------------------------------------------------ test 1: books close

fn assert_closed(flora: &Flora, when: &str) {
    let v = flora.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() <= 1e-9 * held.abs().max(1.0),
            "{when}: flora {what} holds {held}, the ledger expects {expected}"
        );
    }
}

/// Over one whole (fast) cycle a browsed adult stand, its night budget active, closes
/// its books: organic in = organic out + storage change, and the same for mineral and
/// energy, to the existing conservation tolerance.
#[test]
fn over_one_cycle_with_the_night_budget_the_books_close() {
    let day = DayClock {
        length_min: 2.0,
        start_hour: 0.0,
        ..DayClock::default()
    };
    let mut world = slab(day);
    let mut flora = seeded(&config(), &world, Some(0.1));
    assert!(world.life_to_dawn_s().is_some(), "fixture: starts in the night");
    assert_closed(&flora, "seeded");
    let ticks = (day.length_min * 60.0 / DT).round() as u64;
    for _ in 0..ticks {
        world.step();
        flora.step(&mut world);
    }
    let spent: f64 = flora.view().ledger.night_spend[BC.index()].iter().sum();
    assert!(spent > 0.0, "the night budget was active: {spent}");
    assert_closed(&flora, "after one cycle");
}

// ------------------------------------------------------------------ test 4: clock off

/// With the clock off there is no night: the night knobs reach nothing. Two browsed
/// stands, one on the default knobs and one on extreme ones, step identically to the bit,
/// and neither books a night spend.
#[test]
fn with_the_clock_off_the_plants_are_untouched_by_the_night_budget() {
    let off = DayClock {
        mode: DayMode::Off,
        start_hour: 0.0,
        ..DayClock::default()
    };
    let base = config();
    let mut extreme = config();
    for s in Species::ALL {
        let sc = extreme.species_mut(s);
        sc.night_bloom_scale = 1.0;
        sc.night_floor = 0.0;
        sc.night_min_s = 1.0;
    }
    let (mut wa, mut wb) = (slab(off), slab(off));
    assert_eq!(wa.life_to_dawn_s(), None);
    let mut a = seeded(&base, &wa, Some(0.1));
    let mut b = seeded(&extreme, &wb, Some(0.1));
    for _ in 0..200 {
        wa.step();
        wb.step();
        a.step(&mut wa);
        b.step(&mut wb);
    }
    assert_eq!(a.view().stands, b.view().stands);
    assert_eq!(a.view().ledger, b.view().ledger);
    for s in Species::ALL {
        assert_eq!(a.view().ledger.night_spend[s.index()], [0.0; 3], "{s:?}");
    }
}
