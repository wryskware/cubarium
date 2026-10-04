//! **F1, written before the rule** (`design/handoffs/voxel-flight-2026-09-28.md` Brief F1,
//! test 7; `design/predation-and-flight-2026-09-28.md` §3.7, decision 6): bloomcrown gains
//! a nectar stock of its own — on adult stands, paid from the reserve in daylight up to a
//! cap; unvisited nectar stays — and a consumer drinks it through
//! [`Flora::take_stand_nectar`].
//!
//! Fixtures: one full-grown bloomcrown on a wet soil plain at 0.25 m, the day clock held
//! at noon or at midnight (a world that is never stepped keeps its hour; the plant layer
//! borrows it per call, so a noon twin and a midnight twin of the same terrain switch the
//! hour under one plant layer). The nectar knobs are set explicitly, so the implementer's
//! defaults change none of these results. No run is longer than 200 ticks.

use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species};

const RATE: f64 = 1e-3;
const CAP: f64 = 2e-3;

/// A wet soil plain 12 × 12 × 6 at 0.25 m with the day clock at `hour`.
fn plain(hour: f64) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 12,
        height: 12,
        depth: 6,
        voxel_m: 0.25,
        seed: 3,
        day: DayClock {
            start_hour: hour,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    let m = cubarium_voxel::Materials::DEFAULT;
    let cap = m.pore_capacity(Material::Soil) * w.config().voxel_volume();
    let fc = m.field_capacity(Material::Soil);
    for z in 0..6 {
        for x in 0..12 {
            for y in 1..=2 {
                w.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: fc * cap,
                });
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

fn noon() -> World {
    let w = plain(12.0);
    assert!(w.life_daylight() > 0.9, "fixture: noon, {}", w.life_daylight());
    w
}

fn midnight() -> World {
    let w = plain(0.0);
    assert_eq!(w.life_daylight(), 0.0, "fixture: night");
    w
}

fn config() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.bloomcrown.nectar_per_s = RATE;
    c.bloomcrown.nectar_cap = CAP;
    c
}

/// A bloomcrown on `(5, 3)` with `wood`; its site.
fn bloomcrown(flora: &mut Flora, world: &World, wood: f64) -> Site {
    assert!(flora.apply(
        world,
        Command::Seed {
            x: 5,
            z: 3,
            species: Species::Bloomcrown,
            wood,
        },
    ));
    Site { x: 5, y: 2, z: 3 }
}

fn nectar(flora: &Flora, site: Site) -> f64 {
    flora.view().stand_at(site).expect("the stand").nectar
}

fn closes(flora: &Flora, when: &str) {
    let v = flora.view();
    for (name, got, want) in [
        ("organic", v.organic(), v.ledger.expected_organic()),
        ("mineral", v.mineral(), v.ledger.expected_mineral()),
        ("energy", v.energy(), v.ledger.expected_energy()),
    ] {
        assert!(
            (got - want).abs() <= 1e-9 * got.abs().max(1.0),
            "{when}: the {name} ledger must close: {got} against {want}"
        );
    }
}

/// **7. Bloomcrown nectar refills in daylight up to its cap and not at night.**
/// - At noon a full-grown stand's nectar rises from nothing at `nectar_per_s` (after 1 s
///   it holds `RATE`, ±10 %), never falls, reaches the cap within 3 s and never passes it;
///   the books close every tick.
/// - The same stand, filled, then stepped 40 ticks under the midnight twin keeps its
///   nectar exactly: unvisited nectar stays, and night adds none.
/// - A fresh stand at midnight makes at most `night_bloom_scale` of the rate in 80 ticks
///   (night survival 1b), and none at a scale of 0.
/// - A seedling (a twentieth of `wood_max`, still on its rosette profile) makes none at
///   noon: nectar is an adult's.
#[test]
fn bloomcrown_nectar_refills_in_daylight_up_to_its_cap_and_not_at_night() {
    let mut day = noon();
    let mut night = midnight();
    let wood = config().bloomcrown.wood_max;

    let mut flora = Flora::new(config());
    let site = bloomcrown(&mut flora, &day, wood);
    assert_eq!(nectar(&flora, site), 0.0, "a founder starts with none");
    let mut last = 0.0;
    let mut full_at = None;
    for t in 1..=80 {
        flora.step(&mut day);
        let n = nectar(&flora, site);
        assert!(n >= last - 1e-15, "tick {t}: nectar never falls by day: {n} after {last}");
        assert!(n <= CAP + 1e-12, "tick {t}: {n} over the cap {CAP}");
        if t == 20 {
            assert!((n - RATE).abs() <= 0.1 * RATE, "after 1 s it holds the rate's worth: {n}");
        }
        if full_at.is_none() && (n - CAP).abs() <= 1e-12 {
            full_at = Some(t);
        }
        closes(&flora, &format!("noon tick {t}"));
        last = n;
    }
    let full_at = full_at.expect("the stock fills to its cap");
    assert!(full_at <= 60, "full within 3 s: tick {full_at}");

    let filled = nectar(&flora, site);
    for t in 1..=40 {
        flora.step(&mut night);
        assert_eq!(nectar(&flora, site), filled, "night tick {t}: unvisited nectar stays");
        closes(&flora, &format!("night tick {t}"));
    }

    // Night survival 1b: at night a stand makes nectar at `night_bloom_scale` of the day
    // rate out of its night budget, never more; at a scale of 0, none (the old rule).
    let mut none = config();
    none.bloomcrown.night_bloom_scale = 0.0;
    let mut flora = Flora::new(none);
    let site = bloomcrown(&mut flora, &night, wood);
    for _ in 0..80 {
        flora.step(&mut night);
    }
    assert_eq!(nectar(&flora, site), 0.0, "no nectar is made at night at scale 0");
    let mut flora = Flora::new(config());
    let scale = flora.config().bloomcrown.night_bloom_scale;
    let site = bloomcrown(&mut flora, &night, wood);
    for _ in 0..80 {
        flora.step(&mut night);
    }
    let n = nectar(&flora, site);
    assert!(
        n <= scale * RATE * 80.0 * cubarium_voxel_flora::DT + 1e-12,
        "at night at most the night scale of the rate: {n}"
    );

    let mut flora = Flora::new(config());
    let site = bloomcrown(&mut flora, &day, 0.05 * wood);
    for _ in 0..80 {
        flora.step(&mut day);
    }
    assert!(flora.view().stand_at(site).is_some(), "fixture: the seedling lives");
    assert_eq!(nectar(&flora, site), 0.0, "a seedling makes no nectar");
}

/// **A stand's nectar is drunk through `take_stand_nectar`** (supports test 6): a take is
/// bounded by the ask and by the stock, the stock falls by exactly what was handed over,
/// the plant layer books it consumed, the nectar carries a little mineral (pollen lumped
/// in, §3.7) and its energy, and an empty stand — or a site with no stand — gives `None`.
#[test]
fn stand_nectar_is_taken_booked_and_runs_out() {
    let mut day = noon();
    let mut flora = Flora::new(config());
    let site = bloomcrown(&mut flora, &day, config().bloomcrown.wood_max);
    for _ in 0..60 {
        flora.step(&mut day);
    }
    let have = nectar(&flora, site);
    assert!(have > 0.0, "fixture: nectar made at noon");
    let out = flora.view().ledger.consumed_organic_out;

    let first = flora.take_stand_nectar(site, 0.5 * have).expect("a first sip");
    assert!(first.organic > 0.0 && first.organic <= 0.5 * have + 1e-15);
    assert!(first.mineral > 0.0, "nectar is mineral-poor, not mineral-free");
    assert!(first.energy > 0.0);
    assert!((nectar(&flora, site) - (have - first.organic)).abs() <= 1e-15);
    let rest = flora.take_stand_nectar(site, 1e9).expect("the rest");
    assert!((rest.organic - (have - first.organic)).abs() <= 1e-15, "no more than is left");
    assert_eq!(nectar(&flora, site), 0.0);
    assert!(flora.take_stand_nectar(site, 1e9).is_none(), "an empty flower gives nothing");
    assert!(
        flora.take_stand_nectar(Site { x: 1, y: 2, z: 1 }, 1e9).is_none(),
        "no stand, no nectar"
    );
    let booked = flora.view().ledger.consumed_organic_out - out;
    assert!((booked - (first.organic + rest.organic)).abs() <= 1e-15, "booked consumed");
    closes(&flora, "after the sips");
}
