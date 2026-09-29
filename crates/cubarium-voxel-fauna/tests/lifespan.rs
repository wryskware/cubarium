//! **Animal lifespan** (`design/handoffs/voxel-lifespan-defence-2026-09-25.md`, A): a body
//! past its own drawn lifespan departs as [`Departure::Aged`] and leaves carrion like any
//! other death. Short test lifespans; every test under 200 ticks; no pinned hashes.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_fauna::{
    Command, Departure, Fauna, FaunaConfig, Founder, Reproduction, StartingStores,
    draw_lifespan_ticks,
};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site};

fn plain(width: u32, top: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 10,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    });
    for x in 0..width as i64 {
        for y in 1..=top {
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

/// Every body kind's median lifespan set to `median_s`.
fn short_lived(median_s: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.frondgrazer.lifespan_median_s = median_s;
    for f in Founder::ALL {
        c.founders[f.index()].core.lifespan_median_s = median_s;
    }
    c
}

fn founder(fauna: &mut Fauna, world: &World, x: i64, f: Founder) -> u64 {
    let id = fauna.view().ledger.births;
    assert!(fauna.apply(
        world,
        Command::IntroduceFounder {
            x,
            z: 0,
            founder: f,
            stores: StartingStores::FULL,
            heading_rad: 0.0,
        }
    ));
    id
}

fn residuals(fauna: &Fauna, flora: &Flora) {
    let v = fauna.view();
    let fv = flora.view();
    for (got, what) in [
        (v.organic() - v.ledger.expected_organic(), "fauna organic"),
        (v.mineral() - v.ledger.expected_mineral(), "fauna mineral"),
        (v.energy() - v.ledger.expected_energy(), "fauna energy"),
        (fv.organic() - fv.ledger.expected_organic(), "flora organic"),
        (fv.mineral() - fv.ledger.expected_mineral(), "flora mineral"),
        (fv.energy() - fv.ledger.expected_energy(), "flora energy"),
    ] {
        assert!(got.abs() <= 1e-9, "{what} residual {got:e}");
    }
}

/// The shipped median is four simulated hours for every body kind.
#[test]
fn the_shipped_median_is_four_hours() {
    let c = FaunaConfig::default();
    assert_eq!(c.frondgrazer.lifespan_median_s, 4.0 * 3600.0);
    for f in Founder::ALL {
        assert_eq!(c.founder(f).core.lifespan_median_s, 4.0 * 3600.0);
    }
}

/// A body past its lifespan departs as `Aged`, leaves its whole body as carrion, and both
/// ledgers close.
#[test]
fn a_body_past_its_lifespan_departs_aged_and_leaves_carrion() {
    let world = plain(6, 2);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(short_lived(4.0));
    let id = founder(&mut fauna, &world, 2, Founder::Browser);
    let a = *fauna.view().animal(id).expect("placed");
    assert!(a.lifespan_ticks > 0, "a lifespan was drawn");
    let organic = a.organic();
    let mut ticks = 0;
    while !fauna.view().animals.is_empty() && ticks < 200 {
        fauna.step(&world, &mut flora);
        ticks += 1;
    }
    let l = fauna.view().ledger;
    assert!(fauna.view().animals.is_empty(), "gone within 200 ticks");
    assert_eq!(l.departed(Departure::Aged), 1);
    assert_eq!(l.departed_founder(Founder::Browser, Departure::Aged), 1);
    assert_eq!(
        l.departed(Departure::Starved) + l.departed(Departure::Drowned),
        0
    );
    assert_eq!(l.deaths, 1);
    assert_eq!(l.deaths_accounted(), 1, "aged is a death cause");
    let carrion: f64 = flora.view().ground.iter().map(|g| g.carrion).sum();
    assert!(
        carrion > 0.5 * organic,
        "the body became carrion: {carrion} of {organic}"
    );
    residuals(&fauna, &flora);
}

/// A lifespan of zero is no lifespan: the body stays.
#[test]
fn a_zero_median_is_immortal() {
    let world = plain(6, 2);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(short_lived(0.0));
    founder(&mut fauna, &world, 2, Founder::Blind);
    for _ in 0..200 {
        fauna.step(&world, &mut flora);
    }
    assert_eq!(fauna.view().animals.len(), 1);
}

/// Founders arrive at staggered ages: eight introduced together die at many different
/// ticks, spread over a good part of the lifespan, not as one cohort.
#[test]
fn founders_ages_at_death_are_spread_out() {
    let world = plain(10, 2);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(short_lived(6.0)); // 120 ticks
    for x in 1..9 {
        founder(&mut fauna, &world, x, Founder::Browser);
    }
    let mut deaths: Vec<u64> = Vec::new();
    for t in 1..=200u64 {
        let before = fauna.view().animals.len();
        fauna.step(&world, &mut flora);
        let after = fauna.view().animals.len();
        deaths.extend(std::iter::repeat_n(t, before - after));
    }
    assert_eq!(deaths.len(), 8, "all eight aged out: {deaths:?}");
    let mut distinct = deaths.clone();
    distinct.dedup();
    assert!(distinct.len() >= 5, "death ticks {deaths:?}");
    assert!(
        deaths.last().unwrap() - deaths.first().unwrap() >= 30,
        "spread over >= 30 of 120 ticks: {deaths:?}"
    );
}

/// The draw: a newborn's lifespan is within ±20 % of the median and differs body to body;
/// an arriving founder's is a staggered share of one.
#[test]
fn the_draw_is_within_its_spread_and_differs_by_body() {
    let median = 1000.0 * DT; // 1000 ticks
    let newborn: Vec<u64> = (0..200)
        .map(|id| draw_lifespan_ticks(median, id, 17, false))
        .collect();
    assert!(
        newborn.iter().all(|&t| (800..=1200).contains(&t)),
        "{newborn:?}"
    );
    let mean = newborn.iter().sum::<u64>() as f64 / newborn.len() as f64;
    assert!((mean - 1000.0).abs() < 40.0, "mean {mean}");
    let mut d = newborn.clone();
    d.sort_unstable();
    d.dedup();
    assert!(d.len() > 150, "bodies draw their own");
    let arriving: Vec<u64> = (0..200)
        .map(|id| draw_lifespan_ticks(median, id, 17, true))
        .collect();
    assert!(
        arriving.iter().all(|&t| (400..=1200).contains(&t)),
        "{arriving:?}"
    );
    assert!(arriving.iter().any(|&t| t < 700) && arriving.iter().any(|&t| t > 900));
    assert_eq!(
        draw_lifespan_ticks(0.0, 3, 17, false),
        0,
        "no median, no lifespan"
    );
}

/// A newborn gets its own lifespan at birth: hatchlings of one clutch each carry one, in
/// the newborn spread, and not the same one.
#[test]
fn a_newborn_gets_its_own_lifespan() {
    let (count, egg, incubation) = (3u32, 0.0035, 10u64);
    let world = plain(4, 2);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.deposit(
        Site { x: 1, y: 2, z: 0 },
        Deposit {
            kind: DepositKind::Litter,
            organic: 0.2,
            mineral: 0.004,
            energy: 0.4,
        }
    ));
    let mut c = FaunaConfig::default();
    {
        let core = &mut c.founders[Founder::Blind.index()].core;
        core.reserve_cap = 1.0;
        core.reproduction = Reproduction {
            surplus_floor: 0.0005,
            surplus_hold_s: DT,
            birth_interval_s: 200.0 * DT,
            clutch_size: count,
            egg_organic: egg,
            incubation_s: incubation as f64 * DT,
            ..Reproduction::EGGS_PLACEHOLDER
        };
    }
    let median_ticks = (c.founder(Founder::Blind).core.lifespan_median_s / DT).round() as u64;
    let mut fauna = Fauna::new(c);
    let parent = founder(&mut fauna, &world, 1, Founder::Blind);
    for _ in 0..=incubation + 1 {
        fauna.step(&world, &mut flora);
    }
    let young: Vec<_> = fauna
        .view()
        .animals
        .iter()
        .filter(|a| a.id != parent)
        .copied()
        .collect();
    assert_eq!(young.len(), count as usize, "the clutch hatched");
    for a in &young {
        let t = a.lifespan_ticks as f64 / median_ticks as f64;
        assert!(
            (0.8..=1.2).contains(&t),
            "newborn lifespan {t} of the median"
        );
    }
    assert!(
        young
            .windows(2)
            .any(|w| w[0].lifespan_ticks != w[1].lifespan_ticks),
        "each drew its own"
    );
}
