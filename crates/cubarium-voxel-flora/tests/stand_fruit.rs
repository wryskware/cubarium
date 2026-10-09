//! **F3, written before the rule** (`design/handoffs/voxel-roster-2026-10-01.md` Brief F3,
//! "Food"; `design/predation-and-flight-2026-09-28.md` §3.7): lanternberry gains a `fruit`
//! stock on adult stands — paid from the reserve in daylight up to `fruit_cap`, none at
//! night — whose uneaten fruit falls to the litter after `fruit_drop_s`, and a consumer
//! eats it through [`Flora::take_stand_fruit`]. Fruit is energy-rich with a small, non-zero
//! `fruit_mineral_fraction`.
//!
//! Fixtures: one full-grown lanternberry on a wet soil plain at 0.25 m, the day clock held
//! at noon or at midnight (a world that is never stepped keeps its hour; the plant layer
//! borrows it per call). The fruit knobs are set explicitly, so the implementer's defaults
//! change none of these results, and the shrub's own seed parcel is switched off
//! (`propagule_rate` 0) so nothing but fruit leaves its reserve for a propagule. No run is
//! longer than 200 ticks.

use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species};

const RATE: f64 = 1e-3;
const CAP: f64 = 2e-3;
/// "Never" for a fixture: no fruit falls inside a test.
const NEVER_S: f64 = 1e9;

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

/// Lanternberry fruiting at [`RATE`] up to [`CAP`], dropping uneaten fruit after
/// `drop_s`; no seed parcel.
fn config(rate: f64, drop_s: f64) -> FloraConfig {
    let mut c = FloraConfig::default();
    c.lanternberry.fruit_per_s = rate;
    c.lanternberry.fruit_cap = CAP;
    c.lanternberry.fruit_drop_s = drop_s;
    c.lanternberry.fruit_mineral_fraction = 0.01;
    c.lanternberry.propagule_rate = 0.0;
    // F3's mechanism alone (FLOWER-FRUIT): no flowering stage, fruit at par.
    c.lanternberry.flower_s = 0.0;
    c.lanternberry.fruit_cost = 1.0;
    c
}

/// A lanternberry on `(5, 3)` with `wood`; its site.
fn lanternberry(flora: &mut Flora, world: &World, wood: f64) -> Site {
    assert!(flora.apply(
        world,
        Command::Seed {
            x: 5,
            z: 3,
            species: Species::Lanternberry,
            wood,
        },
    ));
    Site { x: 5, y: 2, z: 3 }
}

fn fruit(flora: &Flora, site: Site) -> f64 {
    flora.view().stand_at(site).expect("the stand").fruit
}

fn reserve(flora: &Flora, site: Site) -> f64 {
    flora.view().stand_at(site).expect("the stand").reserve
}

fn litter(flora: &Flora, site: Site) -> f64 {
    flora.view().ground_at(site).map_or(0.0, |g| g.litter)
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

/// **A lanternberry grows fruit in daylight up to its cap, from its reserve, and none at
/// night.**
/// - At noon a full-grown stand's fruit rises from nothing at `fruit_per_s` (after 1 s it
///   holds `RATE`, ±10 %), never falls while none drops, reaches the cap within 3 s and
///   never passes it; the books close every tick.
/// - It is paid from the reserve: against a twin that makes no fruit its reserve is never
///   higher, and never lower by more than the fruit it holds; the books close, so the
///   fruit is the stand's own matter, not new.
/// - The same stand, filled, then stepped 40 ticks under the midnight twin keeps its fruit
///   exactly: night adds none.
/// - A fresh stand at midnight makes at most `night_bloom_scale` of the rate in 80 ticks
///   (night survival 1b), none at a scale of 0; a seedling (a twentieth of
///   `wood_max`) makes none at noon: fruit is an adult's.
#[test]
fn lanternberry_fruit_grows_in_daylight_up_to_its_cap_from_its_reserve_and_none_at_night() {
    let mut day = noon();
    let mut night = midnight();
    let wood = FloraConfig::default().lanternberry.wood_max;

    let mut flora = Flora::new(config(RATE, NEVER_S));
    let mut barren = Flora::new(config(0.0, NEVER_S));
    let site = lanternberry(&mut flora, &day, wood);
    let _ = lanternberry(&mut barren, &day, wood);
    assert_eq!(fruit(&flora, site), 0.0, "a founder starts with none");
    let mut last = 0.0;
    let mut full_at = None;
    for t in 1..=80 {
        flora.step(&mut day);
        barren.step(&mut day);
        let f = fruit(&flora, site);
        assert!(f >= last - 1e-15, "tick {t}: fruit never falls when none drops: {f} after {last}");
        assert!(f <= CAP + 1e-12, "tick {t}: {f} over the cap {CAP}");
        assert_eq!(fruit(&barren, site), 0.0, "the twin makes none");
        // Paid from the reserve: the fruiting stand never holds more reserve than its
        // barren twin, and never less by more than the fruit it hangs (income may refill
        // the reserve, so the gap is not pinned to the fruit).
        let paid = reserve(&barren, site) - reserve(&flora, site);
        assert!(paid >= -1e-15, "tick {t}: fruit adds no reserve: {paid}");
        assert!(paid <= f + 1e-12, "tick {t}: it pays no more than the fruit: {paid} for {f}");
        if t == 20 {
            assert!((f - RATE).abs() <= 0.1 * RATE, "after 1 s it holds the rate's worth: {f}");
        }
        if full_at.is_none() && (f - CAP).abs() <= 1e-12 {
            full_at = Some(t);
        }
        closes(&flora, &format!("noon tick {t}"));
        last = f;
    }
    let full_at = full_at.expect("the stock fills to its cap");
    assert!(full_at <= 60, "full within 3 s: tick {full_at}");

    let filled = fruit(&flora, site);
    for t in 1..=40 {
        flora.step(&mut night);
        assert_eq!(fruit(&flora, site), filled, "night tick {t}: night adds none, and it hangs");
        closes(&flora, &format!("night tick {t}"));
    }

    // Night survival 1b: at night a stand hangs fruit at `night_bloom_scale` of the day
    // rate out of its night budget, never more; at a scale of 0, none (the old rule).
    let mut none = config(RATE, NEVER_S);
    none.lanternberry.night_bloom_scale = 0.0;
    let mut flora = Flora::new(none);
    let site = lanternberry(&mut flora, &night, wood);
    for _ in 0..80 {
        flora.step(&mut night);
    }
    assert_eq!(fruit(&flora, site), 0.0, "no fruit is made at night at scale 0");
    let mut flora = Flora::new(config(RATE, NEVER_S));
    let scale = flora.config().lanternberry.night_bloom_scale;
    let site = lanternberry(&mut flora, &night, wood);
    for _ in 0..80 {
        flora.step(&mut night);
    }
    let f = fruit(&flora, site);
    assert!(
        f <= scale * RATE * 80.0 * cubarium_voxel_flora::DT + 1e-12,
        "at night at most the night scale of the rate: {f}"
    );

    let mut flora = Flora::new(config(RATE, NEVER_S));
    let site = lanternberry(&mut flora, &day, 0.05 * wood);
    for _ in 0..80 {
        flora.step(&mut day);
    }
    assert!(flora.view().stand_at(site).is_some(), "fixture: the seedling lives");
    assert_eq!(fruit(&flora, site), 0.0, "a seedling makes no fruit");
}

/// **Uneaten fruit falls to the litter after `fruit_drop_s`.** Two noon twins of the same
/// stand, one dropping after 2 s and one never. Until 2 s have passed the twins are
/// identical (no fruit is old enough to fall); within 160 ticks (2 s to fill the cap, 2 s
/// to ripen past the drop time, and room) the dropping twin's site holds at least half a
/// crop more litter than the other's. Its fruit never passes the cap and the books close
/// every tick: fallen fruit is litter, not lost.
#[test]
fn uneaten_lanternberry_fruit_falls_to_litter_after_fruit_drop_s() {
    const DROP_S: f64 = 2.0;
    let mut day = noon();
    let wood = FloraConfig::default().lanternberry.wood_max;
    let mut drops = Flora::new(config(RATE, DROP_S));
    let mut hangs = Flora::new(config(RATE, NEVER_S));
    let site = lanternberry(&mut drops, &day, wood);
    let _ = lanternberry(&mut hangs, &day, wood);
    let drop_ticks = (DROP_S / DT).round() as usize;
    for t in 1..=160 {
        drops.step(&mut day);
        hangs.step(&mut day);
        assert!(fruit(&drops, site) <= CAP + 1e-12, "tick {t}: over the cap");
        closes(&drops, &format!("tick {t}"));
        if t < drop_ticks {
            assert_eq!(
                litter(&drops, site),
                litter(&hangs, site),
                "tick {t}: nothing is old enough to fall before {DROP_S} s"
            );
        }
    }
    let fell = litter(&drops, site) - litter(&hangs, site);
    assert!(fell >= 0.5 * CAP, "uneaten fruit fell to the litter: {fell} (a crop is {CAP})");
    assert!((fruit(&hangs, site) - CAP).abs() <= 1e-12, "the twin's fruit still hangs");
}

/// **A stand's fruit is eaten through `take_stand_fruit`**: a take is bounded by the ask
/// and by the stock, the stock falls by exactly what was handed over, the plant layer
/// books it consumed, the fruit carries a little mineral and its energy, and an empty
/// stand — or a site with no stand — gives `None`.
#[test]
fn stand_fruit_is_taken_booked_and_runs_out() {
    let mut day = noon();
    let mut flora = Flora::new(config(RATE, NEVER_S));
    let site = lanternberry(&mut flora, &day, FloraConfig::default().lanternberry.wood_max);
    for _ in 0..60 {
        flora.step(&mut day);
    }
    let have = fruit(&flora, site);
    assert!(have > 0.0, "fixture: fruit made at noon");
    let out = flora.view().ledger.consumed_organic_out;

    let first = flora.take_stand_fruit(site, 0.5 * have).expect("a first bite");
    assert!(first.organic > 0.0 && first.organic <= 0.5 * have + 1e-15);
    assert!(first.mineral > 0.0, "fruit carries a little mineral");
    assert!(first.mineral <= 0.05 * first.organic, "a little: {} of {}", first.mineral, first.organic);
    assert!(first.energy > 0.0);
    assert!((fruit(&flora, site) - (have - first.organic)).abs() <= 1e-15);
    let rest = flora.take_stand_fruit(site, 1e9).expect("the rest");
    assert!((rest.organic - (have - first.organic)).abs() <= 1e-15, "no more than is left");
    assert_eq!(fruit(&flora, site), 0.0);
    assert!(flora.take_stand_fruit(site, 1e9).is_none(), "an empty stand gives nothing");
    assert!(
        flora.take_stand_fruit(Site { x: 1, y: 2, z: 1 }, 1e9).is_none(),
        "no stand, no fruit"
    );
    let booked = flora.view().ledger.consumed_organic_out - out;
    assert!((booked - (first.organic + rest.organic)).abs() <= 1e-15, "booked consumed");
    closes(&flora, "after the bites");
}
