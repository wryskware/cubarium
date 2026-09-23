//! **Grazing** (`design/handoffs/voxel-plant-viability-2026-09-23.md` §G), written
//! before the rule: satiety, diminishing bites, the refuge as a mouth sees it, and a
//! life history on plant time.
//!
//! Each case is a browser founder on a flat 0.25 m plain beside springturf, driven by a
//! scripted controller that holds `feed = 1`, for at most a couple of hundred ticks.
//! Cases that need a rule to bind inside that budget set their own numbers in their own
//! [`FaunaConfig`] and say so; the shipped numbers are checked against the brief's
//! targets arithmetically, never by stepping an hour.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Fauna, FaunaConfig, Food, Founder, Reproduction, Scripted,
    StartingStores, browser_mouth_foliage,
};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};

// ------------------------------------------------------------------- fixtures

/// 8 × 6 × 6 voxels at 0.25 m, soil 1..=2, the support face at y = 2 everywhere.
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
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    world
}

fn site(x: u32, z: u32) -> Site {
    Site { x, y: 2, z }
}

/// A full-grown springturf at `(x, z)`.
fn turf(flora: &mut Flora, world: &World, x: i64, z: u32) {
    let wood = flora.config().species(Plant::Springturf).wood_max;
    assert!(flora.apply(
        world,
        FloraCommand::Seed {
            x,
            z,
            species: Plant::Springturf,
            wood,
        },
    ));
}

fn foliage_at(flora: &Flora, s: Site) -> f64 {
    flora.view().stand_at(s).map_or(0.0, |st| st.foliage)
}

/// A browser founder at `(2, 2)` facing `+z`, holding `feed = 1` from its first
/// sampling. Returns its id.
fn feeding_browser(fauna: &mut Fauna, world: &World, stores: StartingStores) -> u64 {
    let id = fauna.view().ledger.births;
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Browser,
            stores,
            heading_rad: 0.0,
        },
    ));
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        }])),
    ));
    id
}

/// The browser's own numbers with its upkeep switched off, so a reserve stays exactly
/// where a case put it and every change in it is a bite.
fn still_browser(edit: impl FnOnce(&mut cubarium_voxel_fauna::SpeciesConfig)) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    let core = &mut c.founders[Founder::Browser.index()].core;
    core.maintenance_per_s = 0.0;
    edit(core);
    c
}

/// One controller period: the first sampling is at age 5, and the bite with it.
fn one_period(fauna: &mut Fauna, world: &World, flora: &mut Flora) {
    for _ in 0..5 {
        fauna.step(world, flora);
    }
}

// ------------------------------------------------------------------ 1. satiety

/// Test 1: a full animal — body at `body_max`, reserve at its cap — takes nothing, and
/// the plant keeps its foliage.
#[test]
fn a_full_animal_takes_nothing_and_the_plant_keeps_its_foliage() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 2, 2);
    turf(&mut flora, &world, 2, 3);
    let before = (
        foliage_at(&flora, site(2, 2)),
        foliage_at(&flora, site(2, 3)),
    );

    let config = still_browser(|_| {});
    let sc = config.founder(Founder::Browser).core;
    let mut fauna = Fauna::new(config);
    let id = feeding_browser(&mut fauna, &world, StartingStores::FULL);
    assert!(
        browser_mouth_foliage(
            &world.view(),
            &flora.view(),
            &config,
            fauna.view().animal(id).unwrap()
        )
        .is_some(),
        "the fixture puts food at the mouth"
    );
    one_period(&mut fauna, &world, &mut flora);

    let ledger = &fauna.view().ledger;
    assert_eq!(ledger.bites, 0, "a full animal does not bite");
    assert_eq!(ledger.eaten_organic_in, 0.0);
    assert_eq!(
        (
            foliage_at(&flora, site(2, 2)),
            foliage_at(&flora, site(2, 3))
        ),
        before,
        "the plants keep their foliage"
    );
    let a = fauna.view().animal(id).expect("alive");
    assert_eq!(a.body, sc.body_max);
    assert_eq!(a.reserve, sc.reserve_of(sc.body_max));
}

/// Test 2: a hungry animal's bite is capped by the room it has left after yield, and
/// none of it is respired as surplus.
///
/// This case's mouth is five hundred times the shipped one — `bite_per_s` 1.0 — so that
/// the room and not the mouth is what binds inside one bite. The body is at `body_max`
/// (no growth to move the reserve) with its reserve half full: the room is half a
/// reserve, and the bite may take no more than `room / yield`.
#[test]
fn a_hungry_bite_is_capped_by_its_room_and_none_of_it_is_respired_as_surplus() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 2, 2);
    turf(&mut flora, &world, 2, 3);

    let config = still_browser(|core| core.bite_per_s = 1.0);
    let phys = *config.founder(Founder::Browser);
    let sc = phys.core;
    let mut fauna = Fauna::new(config);
    let id = feeding_browser(
        &mut fauna,
        &world,
        StartingStores {
            body: 1.0,
            reserve: 0.5,
        },
    );
    let reserve_before = fauna.view().animal(id).unwrap().reserve;
    let cap = sc.reserve_of(sc.body_max);
    let room = cap - reserve_before;
    let yield_fraction = phys.yield_for(Food::Foliage);
    one_period(&mut fauna, &world, &mut flora);

    let ledger = *fauna.view().ledger;
    assert_eq!(ledger.bites, 1, "one bite");
    let eaten = ledger.eaten_organic_in;
    let limit = room / yield_fraction;
    assert!(
        eaten <= limit * (1.0 + 1e-12) && eaten >= limit * (1.0 - 1e-9),
        "the bite is the room after yield, {limit}, not {eaten}"
    );
    // What the bite built, by the assimilation rule: the yield, or what its mineral pays
    // for. All of it is placed; the rest of the bite is the undigested share and nothing
    // more.
    let assimilated = (yield_fraction * eaten).min(ledger.eaten_mineral_in / sc.n_tissue);
    let a = fauna.view().animal(id).expect("alive");
    assert!(
        (a.reserve - (reserve_before + assimilated)).abs() < 1e-15,
        "every assimilated unit was placed: {} vs {}",
        a.reserve,
        reserve_before + assimilated
    );
    assert!(
        a.reserve <= cap * (1.0 + 1e-12),
        "the reserve is within its cap"
    );
    assert!(
        (ledger.respired_digestion_out - (eaten - assimilated)).abs() < 1e-15,
        "digestion respired {} of a bite whose undigested share is {}: no surplus",
        ledger.respired_digestion_out,
        eaten - assimilated
    );
}

// ---------------------------------------------------------- 2. diminishing bites

/// The edible foliage a fresh hungry browser's mouth reaches at the fixture, `E`.
fn edible_at_the_mouth(world: &World, flora: &Flora) -> f64 {
    let config = still_browser(|_| {});
    let mut fauna = Fauna::new(config);
    let id = feeding_browser(
        &mut fauna,
        world,
        StartingStores {
            body: 1.0,
            reserve: 0.0,
        },
    );
    browser_mouth_foliage(
        &world.view(),
        &flora.view(),
        &config,
        fauna.view().animal(id).unwrap(),
    )
    .map(|(_, e)| e)
    .expect("the fixture puts food at the mouth")
}

/// Test 3: the bite is `want · E / (E + K)`, and at `E = K` it is half of `want`.
///
/// `want` is the shipped mouth over one controller period at full effort and full
/// hunger (an empty reserve): `bite_per_s · 0.25 s`. `K` is set to the `E` this mouth
/// actually reads, and then to a third of it.
#[test]
fn the_bite_is_want_times_e_over_e_plus_k_and_half_of_want_at_k() {
    let world = flat_world();
    let mut seed = Flora::new(FloraConfig::default());
    turf(&mut seed, &world, 2, 2);
    let e = edible_at_the_mouth(&world, &seed);
    assert!(e > 0.0);

    for (k, factor) in [(e, 0.5), (e / 3.0, 0.75)] {
        let mut flora = seed.clone();
        let config = still_browser(|core| core.bite_half_stock = k);
        let sc = config.founder(Founder::Browser).core;
        let want = sc.bite_per_s * 0.25;
        let mut fauna = Fauna::new(config);
        feeding_browser(
            &mut fauna,
            &world,
            StartingStores {
                body: 1.0,
                reserve: 0.0,
            },
        );
        one_period(&mut fauna, &world, &mut flora);
        let eaten = fauna.view().ledger.eaten_organic_in;
        assert!(
            (eaten - factor * want).abs() < 1e-12 * want.max(1.0),
            "K = {k}: the bite is {factor} of want {want}, not {eaten}"
        );
    }
}

// ------------------------------------------------------------------- 3. refuge

/// The refuge as the mouth sees it: a turf grazed to its floor offers the mouth
/// nothing, and a hungry browser at it takes nothing.
#[test]
fn a_turf_at_its_floor_is_not_food_to_a_mouth() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 2, 2);
    let _ = flora.take_foliage(site(2, 2), 1e9);
    let floor = foliage_at(&flora, site(2, 2));
    assert!(floor > 0.0, "the refuge stays on the plant");

    let config = still_browser(|_| {});
    let mut fauna = Fauna::new(config);
    let id = feeding_browser(
        &mut fauna,
        &world,
        StartingStores {
            body: 1.0,
            reserve: 0.0,
        },
    );
    assert_eq!(
        browser_mouth_foliage(
            &world.view(),
            &flora.view(),
            &config,
            fauna.view().animal(id).unwrap()
        ),
        None,
        "nothing above the floor is nothing at the mouth"
    );
    one_period(&mut fauna, &world, &mut flora);
    assert_eq!(fauna.view().ledger.bites, 0);
    assert_eq!(foliage_at(&flora, site(2, 2)), floor);
}

// ------------------------------------------------------------- 4. life history

/// Test 5: a newborn fed at the most it can eat reaches `birth_body` no sooner than
/// the growth cap allows.
///
/// The case's own numbers: a growth cap that would take a newborn from `body_min` to
/// `birth_body` in 100 ticks, a mouth twenty-five times the shipped one, no upkeep, and
/// births off. Three full turfs are more than it can eat in 200 ticks. However much it
/// eats, its structure never runs ahead of `body_min + g · t`; and it does get there,
/// so the cap and not the food is what held it.
#[test]
fn a_newborn_fed_at_the_most_it_can_eat_grows_no_faster_than_the_cap() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    for z in 2..=4 {
        turf(&mut flora, &world, 2, z);
    }
    let base = FaunaConfig::default().founder(Founder::Browser).core;
    let g = (base.birth_body - base.body_min) / (100.0 * DT);
    let config = still_browser(|core| {
        core.growth_max_per_s = g;
        core.bite_per_s = 0.05;
    });
    let sc = config.founder(Founder::Browser).core;
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let id = feeding_browser(
        &mut fauna,
        &world,
        StartingStores {
            body: sc.body_min / sc.body_max,
            reserve: 1.0,
        },
    );
    let start = fauna.view().animal(id).unwrap().body;
    assert!((start - sc.body_min).abs() < 1e-15, "a newborn's structure");

    for t in 1..=200u32 {
        fauna.step(&world, &mut flora);
        let body = fauna.view().animal(id).expect("alive").body;
        assert!(
            body <= start + g * f64::from(t) * DT + 1e-12,
            "tick {t}: structure {body} ran ahead of the cap {}",
            start + g * f64::from(t) * DT
        );
    }
    let body = fauna.view().animal(id).unwrap().body;
    assert!(
        body >= sc.birth_body,
        "fed at the most it can eat, it reached birth_body: {body}"
    );
}

/// Test 6: two births by one parent are at least the interval apart — refractory, then
/// a fresh hold, then a whole gestation. The escrow opens and pays its first instalment
/// on the tick the hold completes, so the least gap the rule allows is one tick under
/// the three durations summed.
///
/// The case's own table (10-tick hold, 10-tick gestation, 40-tick refractory), a full
/// parent with no upkeep, and turf to refill from.
#[test]
fn two_births_by_one_parent_are_at_least_the_interval_apart() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    for z in 2..=4 {
        turf(&mut flora, &world, 2, z);
    }
    let rule = Reproduction {
        surplus_hold_s: 10.0 * DT,
        gestation_s: 10.0 * DT,
        birth_interval_s: 40.0 * DT,
        ..Reproduction::LIVE_BIRTH_PLACEHOLDER
    };
    let config = still_browser(|core| core.reproduction = rule);
    let mut fauna = Fauna::new(config);
    feeding_browser(&mut fauna, &world, StartingStores::FULL);

    let mut births: Vec<u32> = Vec::new();
    let mut born = fauna.view().ledger.born;
    for t in 1..=200u32 {
        fauna.step(&world, &mut flora);
        let now = fauna.view().ledger.born;
        if now > born {
            births.push(t);
            born = now;
        }
    }
    assert!(births.len() >= 2, "two births in the window: {births:?}");
    let gap = u64::from(births[1] - births[0]);
    let least = rule.interval_ticks() + rule.hold_ticks() + rule.gestation_ticks() - 1;
    assert!(
        gap >= least,
        "births {births:?} are {gap} ticks apart, under {least}"
    );
}

/// The shipped life history against the brief's targets, as arithmetic on the shipped
/// numbers: a browser's first birth no sooner than an hour after its own, and births at
/// least half an hour apart; a shredder's first clutch no sooner than an hour after it
/// hatched, and clutches at least half an hour apart. "No sooner" is at the growth cap,
/// which is the fastest any body can go.
#[test]
fn the_shipped_life_history_runs_on_plant_time() {
    let c = FaunaConfig::default();
    let b = c.founder(Founder::Browser).core;
    let r = b.reproduction;
    let grow = (b.birth_body - b.body_min) / b.growth_max_per_s;
    assert!(
        grow + r.surplus_hold_s + r.gestation_s >= 3600.0,
        "a browser's first birth: {} s",
        grow + r.surplus_hold_s + r.gestation_s
    );
    assert!(
        r.birth_interval_s + r.surplus_hold_s + r.gestation_s >= 1800.0,
        "a browser's birth interval: {} s",
        r.birth_interval_s + r.surplus_hold_s + r.gestation_s
    );

    let s = c.founder(Founder::Blind).core;
    let r = s.reproduction;
    let grow = (s.birth_body - s.body_min) / s.growth_max_per_s;
    assert!(
        grow + r.surplus_hold_s >= 3600.0,
        "a shredder's first clutch: {} s",
        grow + r.surplus_hold_s
    );
    assert!(
        r.birth_interval_s + r.surplus_hold_s >= 1800.0,
        "a shredder's clutch interval: {} s",
        r.birth_interval_s + r.surplus_hold_s
    );
}
