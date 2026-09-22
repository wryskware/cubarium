//! Short function tests of the plant layer on tiny hand-built strips.
//!
//! `voxel_m` is 1 m everywhere here, so one voxel of soil holds `pore_capacity` = 0.35
//! cubic metres of pore water and one voxel of standing water is one metre deep. The
//! fixtures wet the soil **exactly**: free water is added to an air cell and the cell is
//! then turned to soil, which keeps the water as pore water and, while the volume fits
//! the soil's capacity, displaces nothing and leaves no free water standing.
//!
//! Where a test needs a rate the placeholders do not give — a maintenance a stand cannot
//! pay inside a hundred ticks, a shade strong enough to read — it sets that rate in its
//! **own** config and says so. Nothing here changes a default.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species, Stage};

// ------------------------------------------------------------------- fixtures

/// A strip one slab deep: bedrock floor, two rows of soil at a chosen pore fraction,
/// air above. Every column's top soil voxel is a support face in open sky.
fn plain(width: u32, height: u32, pore: f64) -> World {
    let config = VoxelConfig {
        width,
        height,
        depth: 1,
        voxel_m: 1.0,
        seed: 7,
        ..VoxelConfig::default()
    };
    let mut w = World::empty(config);
    for x in 0..width as i64 {
        for y in 1..=2 {
            wet_soil(&mut w, x, y, pore);
        }
    }
    w
}

/// Turn one air voxel into soil holding exactly `pore` of its capacity.
fn wet_soil(w: &mut World, x: i64, y: u32, pore: f64) {
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    if pore > 0.0 {
        let got = w.apply(WorldCommand::AddWater {
            x,
            y,
            z: 0,
            volume_m3: pore * cap,
        });
        assert!(
            (got - pore * cap).abs() < 1e-12,
            "the void took {got} of {}",
            pore * cap
        );
    }
    w.apply(WorldCommand::SetMaterial {
        x,
        y,
        z: 0,
        material: Material::Soil,
    });
    assert!(
        (w.view().pore_at(x, y, 0) - pore).abs() < 1e-12,
        "{}",
        w.view().pore_at(x, y, 0)
    );
    assert_eq!(
        w.view().free_at(x, y, 0),
        0.0,
        "nothing may be left standing"
    );
}

fn site(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

fn organic_residual(flora: &Flora) -> f64 {
    let v = flora.view();
    v.organic() - v.ledger.expected_organic()
}

fn mineral_residual(flora: &Flora) -> f64 {
    let v = flora.view();
    v.mineral() - v.ledger.expected_mineral()
}

fn energy_residual(flora: &Flora) -> f64 {
    let v = flora.view();
    v.energy() - v.ledger.expected_energy()
}

/// The three residuals of the round-3 ledger: organic matter against its two named
/// boundary flows, mineral against nothing but seeding and removal, energy as before.
fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    let o = organic_residual(flora);
    let n = mineral_residual(flora);
    let e = energy_residual(flora);
    assert!(
        o.abs() <= 1e-9 * v.organic().abs().max(1.0),
        "{when}: organic residual {o}"
    );
    assert!(
        n.abs() <= 1e-9 * v.mineral().abs().max(1.0),
        "{when}: mineral residual {n}"
    );
    assert!(
        e.abs() <= 1e-9 * v.energy().abs().max(1.0),
        "{when}: energy residual {e}"
    );
}

fn run(flora: &mut Flora, world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        flora.step(world);
    }
}

// --------------------------------------------------------------- income and growth

/// Wet enough that `μ` saturates (pore 0.6 is past bloomcrown's `sat_pore` of 0.5) and
/// in open sky, so the only limits are the model's own caps.
#[test]
fn a_seeded_stand_in_open_sky_earns_income_and_grows() {
    let mut world = plain(8, 8, 0.6);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 3,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));

    let before = *flora.view().stand_at(site(3)).expect("seeded");
    let mineral0 = flora.view().ground_at(site(3)).expect("ground").mineral;
    run(&mut flora, &mut world, 100);

    let after = *flora.view().stand_at(site(3)).expect("still there");
    assert_eq!(after.light, 1.0, "open sky and a saturating light response");
    assert_eq!(after.moisture, 1.0, "pore 0.6 is past sat_pore");
    assert!(
        after.wood > before.wood,
        "wood {} -> {}",
        before.wood,
        after.wood
    );
    assert!(
        after.foliage > 0.9 * before.foliage,
        "foliage held up: {}",
        after.foliage
    );
    assert!(flora.view().ledger.light_in > 0.0, "it fixed no light");
    assert!(flora.view().ledger.transpired_m3 > 0.0, "it drank nothing");
    assert_eq!(
        flora.view().ledger.transpired_m3,
        world.view().ledger.transpiration_out,
        "the two ledgers must agree to the bit"
    );

    // Growth is paid for out of the site's own mineral pool, at `n_tissue` per unit of
    // tissue built — and the mineral it drew is standing in the plant, not respired.
    let g = flora.view().ground_at(site(3)).expect("ground");
    assert!(g.mineral < mineral0, "mineral {mineral0} -> {}", g.mineral);
    assert!(
        after.mineral > before.mineral,
        "the plant holds the mineral it drew"
    );
    assert!(
        flora.view().ground_at(site(3)).unwrap().litter > 0.0,
        "senescence sheds litter"
    );
    assert_residuals(&flora, "after 100 ticks of growth");
}

/// The wilt test: dry soil means `μ = 0`, no income at all, and a stand that has to pay
/// maintenance out of its reserve until there is none and the wood diebacks away.
///
/// `maintenance` is 0.4 /s here against the placeholder 0.0002: at the placeholder this
/// stand would take about eight hours of simulated time to die, and this is a short
/// test. Nothing else is changed.
#[test]
fn a_stand_at_wilting_point_earns_nothing_and_dies_of_unpaid_maintenance() {
    let mut world = plain(6, 8, 0.0);
    let mut config = FloraConfig::default();
    config.bloomcrown.maintenance = 0.4;
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));

    flora.step(&mut world);
    let stand = *flora
        .view()
        .stand_at(site(2))
        .expect("still alive after one tick");
    assert_eq!(stand.moisture, 0.0, "dry soil is wilting point");
    assert_eq!(flora.view().ledger.light_in, 0.0, "no water, no income");
    assert_eq!(
        flora.view().ledger.transpired_m3,
        0.0,
        "and nothing to drink"
    );

    run(&mut flora, &mut world, 200);
    assert!(
        flora.view().stand_at(site(2)).is_none(),
        "it should be dead"
    );
    assert_eq!(flora.view().ledger.deaths, 1);
    let g = flora.view().ground_at(site(2)).expect("its remains");
    assert!(g.dead_wood > 0.0, "wood becomes dead wood");
    assert!(g.litter > 0.0, "foliage and reserve become litter");
    assert!(
        g.litter_mineral > 0.0 && g.dead_wood_mineral > 0.0,
        "carrying their mineral"
    );
    // The pool gained mineral only from decomposing the remains: nothing was ever
    // assimilated here, so no mineral was drawn out of it either.
    assert!(
        g.mineral > flora.config().initial_mineral,
        "decomposition returns the dead tissue's mineral: {}",
        g.mineral
    );
    assert_residuals(&flora, "after a death by drought");
}

// ------------------------------------------------------------------------- shade

/// An umbrellafrond at full wood beside a young bloomcrown: crown top 5 voxels against
/// 1, crown radius 2.5, so the smaller stand's column is under it.
///
/// `shade_k` is 30 here against the placeholder 1.5. At the placeholder a full
/// umbrellafrond crown (`P = 1.2` over `π·2.5² = 19.6` voxels) attenuates by 9 %, which
/// is real but not something a hundred-tick test can separate from noise. Nothing else
/// is changed.
#[test]
fn a_taller_crown_lowers_the_light_and_the_income_under_it() {
    let mut config = FloraConfig::default();
    config.shade_k_per_m2 = 30.0;

    let mut world = plain(16, 12, 0.6);
    let mut flora = Flora::new(config);
    // The shaded pair at x = 4/5, and a lone control at x = 12.
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 4,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.6
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 5,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 12,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    run(&mut flora, &mut world, 100);

    let shaded = *flora.view().stand_at(site(5)).expect("shaded stand");
    let open = *flora.view().stand_at(site(12)).expect("control stand");
    assert!(
        shaded.light < 0.5 * open.light,
        "shaded {} vs open {}",
        shaded.light,
        open.light
    );
    assert!(
        shaded.wood < open.wood,
        "shaded wood {} vs open {}",
        shaded.wood,
        open.wood
    );
    assert!(
        shaded.foliage < open.foliage,
        "shaded foliage {} vs open {}",
        shaded.foliage,
        open.foliage
    );
    // The tall stand is not shaded by the short one: only a strictly higher crown top
    // shades, and the umbrellafrond's is 5 voxels up against the bloomcrown's 1.
    assert_eq!(flora.view().stand_at(site(4)).unwrap().light, 1.0);
    assert_residuals(&flora, "after 100 ticks in the shade");
}

/// A **closed** canopy: four full umbrellafronds around one young bloomcrown, their
/// crowns stacked over its column, with the maintenance set between what an open stand
/// earns and what a covered one does. The covered stand cannot pay, diebacks and dies;
/// the one in the open pays and grows.
///
/// One crown is not enough to do this and that is a result, not a fixture problem: see
/// the commit message. Three rates are the test's own — `maintenance` 0.0032 /s
/// (placeholder 0.0002) sits between the two incomes, which is the point of the
/// fixture; `reserve_cap` 0.001 (placeholder 0.5) makes the reserve run out in ten ticks
/// instead of five thousand; `dieback` 3000 (placeholder 1) turns the resulting
/// shortfall into wood loss inside the same test. The mechanism under test is the order
/// of §4.3, §4.6 and §4.7, not the constants.
///
/// Round 3b added `umbrellafrond.propagule_rate` 2.0 /s (placeholder 2e-4) to the same
/// fixture: a donor now saves a whole package before anything lands at all, and at the
/// placeholder rate that is 300 s — so without it the gap under the canopy holds no bank
/// inside forty ticks and the succession half of this test has nothing to read. Which of a
/// donor's neighbours receives is its own draw, so the assertion is that the gap is one of
/// the sites the four donors fed, which it is.
#[test]
fn a_stand_that_cannot_pay_under_a_closed_canopy_diebacks_and_dies() {
    let mut config = FloraConfig::default();
    config.shade_k_per_m2 = 30.0;
    config.umbrellafrond.propagule_rate = 2.0;
    config.bloomcrown.maintenance = 0.0032;
    config.bloomcrown.reserve_cap = 0.001;
    config.bloomcrown.dieback = 3000.0;

    let mut world = plain(16, 12, 0.6);
    let mut flora = Flora::new(config);
    for x in [3i64, 4, 6, 7] {
        assert!(flora.apply(
            &world,
            Command::Seed {
                x,
                z: 0,
                species: Species::Umbrellafrond,
                wood: 0.6
            }
        ));
    }
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 5,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 12,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    run(&mut flora, &mut world, 40);

    // The bloomcrown is dead — and the gap under the canopy does not stay empty: the four
    // donors feed an umbrellafrond bank onto it, which either waits there for enough
    // material or has already become the stand standing on it. That is the succession this
    // round is for, and which of the two it is depends on how many of the four donors'
    // draws landed here.
    let took_the_gap = flora.view().stand_at(site(5)).map(|s| s.species);
    assert_ne!(
        took_the_gap,
        Some(Species::Bloomcrown),
        "the covered bloomcrown should be dead"
    );
    let banked = flora.view().ground_at(site(5)).unwrap().seed_species();
    assert!(
        banked == Some(Species::Umbrellafrond) || took_the_gap == Some(Species::Umbrellafrond),
        "the gap holds neither an umbrellafrond bank nor an umbrellafrond born of one: \
         stand {took_the_gap:?}, bank {banked:?}"
    );
    let open = flora
        .view()
        .stand_at(site(12))
        .expect("the open stand lives");
    assert!(
        open.wood >= 0.1,
        "the open stand paid its way: wood {}",
        open.wood
    );
    assert_eq!(flora.view().ledger.deaths, 1);
    assert!(flora.view().ground_at(site(5)).unwrap().dead_wood > 0.0);
    // The four crowns are level with each other, so none of them shades another.
    for x in [3, 4, 6, 7] {
        assert_eq!(
            flora.view().stand_at(site(x)).unwrap().light,
            1.0,
            "crown at {x}"
        );
    }
    assert_residuals(&flora, "after a death under a canopy");
}

// ------------------------------------------------------------- shared root water

/// A three-column ring with `rooting_radius = 1` gives two stands the **same** root box:
/// every voxel of it is shared, so what each one gets is decided entirely by the split.
fn shared_root_world(pore: f64) -> World {
    plain(3, 8, pore)
}

#[test]
fn two_stands_sharing_a_root_box_take_their_demand_and_the_core_books_the_sum() {
    let mut world = shared_root_world(0.2);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.2
        }
    ));

    let stock0: f64 = pore_stock(&world);
    flora.step(&mut world);

    let a = *flora.view().stand_at(site(0)).unwrap();
    let b = *flora.view().stand_at(site(1)).unwrap();
    assert_eq!(a.moisture, b.moisture, "the same box is the same moisture");
    // Ample water: each stand's own demand, so the ratio is the foliage ratio.
    // The demand is read before income and senescence move the foliage, so it is the
    // seeded foliage, alpha times the seeded wood.
    let sc = flora.config().species(Species::Bloomcrown);
    let want_a = sc.transpiration_m3_per_s * (sc.alpha * 0.1) * a.moisture * cubarium_voxel::DT;
    // Only to about 1e-9 of itself: the core keeps pore water as a *fraction* of the
    // voxel's capacity, and a withdrawal this small (1e-7 of the cell's stock) is
    // quantized by the ulp of that fraction. The ledger still books exactly what the
    // store lost, which is what the volume checks below are for.
    assert!(
        (a.water_m3 - want_a).abs() < 1e-8 * want_a,
        "{} vs {want_a}",
        a.water_m3
    );
    assert!(
        (b.water_m3 / a.water_m3 - 2.0).abs() < 1e-6,
        "twice the foliage, twice the water"
    );

    let total = a.water_m3 + b.water_m3;
    assert!(
        (flora.view().ledger.transpired_m3 - total).abs() < 1e-12 * total,
        "the shares sum"
    );
    assert_eq!(
        world.view().ledger.transpiration_out,
        flora.view().ledger.transpired_m3,
        "the core books exactly the sum of the shares"
    );
    assert!(
        (stock0 - pore_stock(&world) - total).abs() < 1e-15,
        "the soil lost exactly that"
    );
    assert_residuals(&flora, "after one shared drink");
}

/// Starved: `transpiration_m3_per_s` is 500 here against the placeholder 2e-5, so the
/// two stands between them ask for more than the whole strip's pore water in one tick.
/// One withdrawal per voxel, capped by the stock, split proportional to demand.
#[test]
fn a_starved_shared_root_box_is_emptied_once_and_split_by_demand() {
    let mut world = shared_root_world(0.1);
    let mut config = FloraConfig::default();
    config.bloomcrown.transpiration_m3_per_s = 500.0;
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.2
        }
    ));

    // Only the two root rows hold water; the stands' boxes are the whole of them.
    let stock0 = pore_stock(&world);
    flora.step(&mut world);

    let a = flora.view().stand_at(site(0)).unwrap().water_m3;
    let b = flora.view().stand_at(site(1)).unwrap().water_m3;
    assert!(
        (a + b - stock0).abs() < 1e-15,
        "{a} + {b} is not the {stock0} that was there"
    );
    assert!(pore_stock(&world) < 1e-15, "the box is empty, not negative");
    assert!(pore_stock(&world) >= 0.0);
    assert!(
        (b / a - 2.0).abs() < 1e-9,
        "the split follows demand: {a} and {b}"
    );
    assert_eq!(
        world.view().ledger.transpiration_out,
        flora.view().ledger.transpired_m3
    );
}

fn pore_stock(world: &World) -> f64 {
    let v = world.view();
    let mut total = 0.0;
    for z in 0..v.config.depth {
        for y in 0..v.config.height {
            for x in 0..v.config.width as i64 {
                total += v.pore_water_m3(x, y, z);
            }
        }
    }
    total
}

// --------------------------------------------------------------------- drowning

#[test]
fn a_pool_drowns_bloomcrown_and_a_film_does_not_while_umbrellafrond_stands_in_both() {
    let mut world = plain(8, 8, 0.5);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 5,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.1
        }
    ));

    // A 2 cm film first: that is rain on its way somewhere, and it kills nothing. Both
    // species' thresholds mean a pool.
    for x in [2, 5] {
        world.apply(WorldCommand::AddWater {
            x,
            y: 3,
            z: 0,
            volume_m3: 0.02,
        });
    }
    flora.step(&mut world);
    assert!(
        flora.view().stand_at(site(2)).is_some(),
        "bloomcrown does not drown in a shower"
    );
    assert_eq!(flora.view().ledger.deaths, 0);

    // Now 0.2 m: past bloomcrown's 0.05 m, inside umbrellafrond's 0.5 m.
    for x in [2, 5] {
        world.apply(WorldCommand::AddWater {
            x,
            y: 3,
            z: 0,
            volume_m3: 0.18,
        });
    }
    flora.step(&mut world);
    assert!(
        flora.view().stand_at(site(2)).is_none(),
        "bloomcrown drowns in a puddle"
    );
    assert!(
        flora.view().stand_at(site(5)).is_some(),
        "umbrellafrond stands in it"
    );
    assert_eq!(flora.view().ledger.deaths, 1);
    let g = flora.view().ground_at(site(2)).unwrap();
    assert!(
        g.dead_wood > 0.0 && g.litter > 0.0,
        "a drowning is a §4.7 death"
    );

    // Half a metre more and it is past the limit too.
    world.apply(WorldCommand::AddWater {
        x: 5,
        y: 3,
        z: 0,
        volume_m3: 0.5,
    });
    assert!(world.view().water_depth_m(5, 2, 0) > 0.5);
    flora.step(&mut world);
    assert!(
        flora.view().stand_at(site(5)).is_none(),
        "and now it drowns"
    );
    assert_eq!(flora.view().ledger.deaths, 2);
    assert_residuals(&flora, "after two drownings");
}

/// Dead wood is a stock with its own energy, not respired remains: §5's `Wd` row. Its
/// density never moves, because decomposition withdraws energy at the stock's current
/// density, and what leaves the stock arrives in `heat_out`.
#[test]
fn dead_wood_keeps_its_energy_until_it_decomposes() {
    let mut world = plain(8, 8, 0.5);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    let e_v = flora.config().species(Species::Bloomcrown).energy_density;

    // Drown it: a §4.7 death, wood to dead wood.
    world.apply(WorldCommand::AddWater {
        x: 2,
        y: 3,
        z: 0,
        volume_m3: 0.2,
    });
    flora.step(&mut world);
    let g = flora
        .view()
        .ground_at(site(2))
        .expect("its remains")
        .clone();
    assert!(
        (g.dead_wood - 0.1).abs() < 1e-12,
        "the wood it had: {}",
        g.dead_wood
    );
    assert!(
        (g.dead_wood_energy - e_v * g.dead_wood).abs() < 1e-12,
        "dead wood holds e_v per unit, not zero: {} for {}",
        g.dead_wood_energy,
        g.dead_wood
    );

    // Clear the water so nothing else dies, and let it rot for a hundred ticks.
    world.apply(WorldCommand::SetMaterial {
        x: 2,
        y: 3,
        z: 0,
        material: Material::Rock,
    });
    world.apply(WorldCommand::SetMaterial {
        x: 2,
        y: 3,
        z: 0,
        material: Material::Air,
    });
    let heat0 = flora.view().ledger.heat_out;
    run(&mut flora, &mut world, 100);

    let after = flora
        .view()
        .ground_at(site(2))
        .expect("still rotting")
        .clone();
    assert!(
        after.dead_wood < g.dead_wood,
        "it did not decompose: {}",
        after.dead_wood
    );
    assert!(after.dead_wood_energy < g.dead_wood_energy);
    assert!(
        (after.dead_wood_energy - e_v * after.dead_wood).abs() < 1e-12,
        "the density moved: {} for {}",
        after.dead_wood_energy,
        after.dead_wood
    );
    let released = g.dead_wood_energy - after.dead_wood_energy;
    let heat = flora.view().ledger.heat_out - heat0;
    assert!(
        heat >= released - 1e-12,
        "the released energy did not reach heat: {heat} vs {released}"
    );
    // The organic matter it lost is respired out of the system, not turned into
    // fertilizer — but the mineral that was in it lands in the site's pool, at the same
    // fraction as the organic matter that left.
    assert!(
        after.mineral > flora.config().initial_mineral,
        "{}",
        after.mineral
    );
    assert!(
        flora.view().ledger.respired_out > 0.0,
        "decomposition respires nothing"
    );
    // Exactly what the two decomposing stocks gave up, and not a unit more: the site's
    // mineral is conserved across the transfer.
    let released = after.mineral - flora.config().initial_mineral;
    let gave_up =
        (g.litter_mineral + g.dead_wood_mineral) - (after.litter_mineral + after.dead_wood_mineral);
    assert!(
        (released - gave_up).abs() < 1e-9 * gave_up,
        "{released} reached the pool for {gave_up} released"
    );
    assert_residuals(&flora, "after a hundred ticks of rot");
}

// ----------------------------------------------------------------------- burial

#[test]
fn a_terrain_edit_that_buries_a_support_books_the_stand_out() {
    let mut world = plain(8, 8, 0.5);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    run(&mut flora, &mut world, 5);

    let stand = *flora.view().stand_at(site(2)).unwrap();
    let ground = flora.view().ground_at(site(2)).unwrap().clone();
    let organic = stand.organic() + ground.litter + ground.dead_wood;
    let mineral = stand.mineral + ground.mineral + ground.litter_mineral + ground.dead_wood_mineral;

    // Rock in the void above the face: it is no longer a support.
    world.apply(WorldCommand::SetMaterial {
        x: 2,
        y: 3,
        z: 0,
        material: Material::Rock,
    });
    assert!(!world.view().is_support(2, 2, 0));
    flora.step(&mut world);

    assert!(
        flora.view().stand_at(site(2)).is_none(),
        "the stand is gone"
    );
    assert!(
        flora.view().ground_at(site(2)).is_none(),
        "and so is its ground"
    );
    let l = flora.view().ledger;
    assert!(
        (l.removed_organic_out - organic).abs() < 1e-15,
        "booked {} organic for {organic}",
        l.removed_organic_out
    );
    assert!(
        (l.removed_mineral_out - mineral).abs() < 1e-15,
        "booked {} mineral for {mineral}",
        l.removed_mineral_out
    );
    assert_eq!(l.deaths, 0, "a burial is not a death: it is a removal");
    assert_residuals(&flora, "after a burial");
}

// ------------------------------------------------------------------- propagules

/// A donor **saves** for one recipient and then sends it a whole package: nothing lands
/// while the parcel is short, one package lands on one site when it is full, and the
/// donor's debit is exactly what it saved plus the construction that was respired.
///
/// Round 3b's rule (Astra R4.4). The old one paid `propagule_rate · dt` to *every*
/// recipient in `hop` out of one reserve, so this fixture's four neighbours each got a
/// cohort on the first tick and the donor paid four times the rate; the rate was
/// advertised as per recipient and funded as if the donor had four times its income.
///
/// `senescence`, `maintenance` and `assimilation` are 0 in this test's own config
/// (placeholders 0.001, 0.0002, 0.004) so the donor's reserve moves only where a
/// propagule debits it and the parcel is the whole of the difference. `propagule_rate` is
/// 0.18 /s against the placeholder 2e-4, which is 0.0075 of net parcel a tick: a package
/// of `alive_min / w_frac` = 0.05 is therefore full on the **seventh** tick (0.045 after
/// six, 0.0525 after seven) and the test can name the tick without standing on a float
/// knife-edge. `hop` is the placeholder 2, so there are four candidate recipients and
/// exactly one of them receives.
#[test]
fn a_donor_saves_a_parcel_and_lands_one_whole_package_on_one_site() {
    let mut config = FloraConfig::default();
    config.bloomcrown.senescence = 0.0;
    config.bloomcrown.maintenance = 0.0;
    config.bloomcrown.assimilation = 0.0;
    config.bloomcrown.propagule_rate = 0.18;
    let mut world = plain(8, 8, 0.6);
    let mut flora = Flora::new(config);
    // Wood 0.4 is over bloomcrown's donor_min of 0.3, and `Seed` fills the reserve.
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 3,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.4
        }
    ));
    let sc = flora.config().species(Species::Bloomcrown).clone();
    let build = 1.0 + sc.build;
    let gross = sc.propagule_rate * cubarium_voxel::DT;
    let net = gross / build;
    let package = sc.alive_min / sc.propagule_split[0];
    assert!(
        (net - 0.0075).abs() < 1e-15 && (package - 0.05).abs() < 1e-15,
        "{net} {package}"
    );
    let mineral0 = flora.view().mineral();
    let donor0 = *flora.view().stand_at(site(3)).expect("seeded");

    // Six ticks of saving: the parcel grows by `net` a tick, the reserve falls by `gross`,
    // the difference is construction respiration, and **nothing** has landed anywhere.
    for tick in 1..=6u32 {
        flora.step(&mut world);
        let donor = *flora.view().stand_at(site(3)).expect("the donor");
        assert!(
            (donor.parcel - tick as f64 * net).abs() <= 1e-15 * package,
            "tick {tick}: parcel {} for {tick} ticks of {net}",
            donor.parcel
        );
        assert!(
            (donor0.reserve - donor.reserve - tick as f64 * gross).abs() <= 1e-15,
            "tick {tick}: the donor spent {}",
            donor0.reserve - donor.reserve
        );
        assert!(
            flora.view().ground.iter().all(|g| g.seeds.is_empty()),
            "tick {tick}: a partial parcel landed: {:?}",
            flora.view().ground
        );
        assert_eq!(
            flora.view().stands.len(),
            1,
            "tick {tick}: a cohort became a stand"
        );
    }

    // The seventh: one package leaves, on exactly one of the four candidate sites, and the
    // remainder keeps saving.
    flora.step(&mut world);
    let banked: Vec<Site> = flora
        .view()
        .ground
        .iter()
        .filter(|g| !g.seeds.is_empty())
        .map(|g| g.site)
        .collect();
    assert_eq!(banked.len(), 1, "one package, one recipient: {banked:?}");
    let at = banked[0];
    assert!(
        [site(1), site(2), site(4), site(5)].contains(&at),
        "{at:?} is not one of the donor's hop-2 neighbours, or is its own site"
    );
    assert_eq!(flora.view().stands.len(), 1, "a cohort is not a stand");
    assert_eq!(
        flora.view().ledger.establishments,
        0,
        "nothing germinated on its landing tick"
    );

    let g = flora.view().ground_at(at).expect("the recipient");
    assert_eq!(g.seeds.len(), 1, "one cohort, not {:?}", g.seeds);
    let c = g.seeds[0];
    assert_eq!(c.species, Species::Bloomcrown);
    assert_eq!(
        c.bin_start_tick, 0,
        "the bin tick 0 opened: the placeholders' bin is 3,000 ticks"
    );
    assert!(
        (c.organic - package).abs() <= 1e-15 * package,
        "{c:?} for a {package} package"
    );
    assert!(
        c.mineral > 0.0,
        "a cohort carries the donor's mineral: {c:?}"
    );
    assert_eq!(
        g.mineral,
        flora.config().initial_mineral,
        "construction respiration is not fertilizer: the recipient's pool is untouched"
    );

    // The donor's books. It saved seven ticks and sent one package, so its debit is
    // `7 · gross` and what it holds in the parcel is `7 · net − package`; the respired
    // half is the construction on everything it saved, sent or not.
    let donor = *flora.view().stand_at(site(3)).expect("the donor");
    let spent = donor0.reserve - donor.reserve;
    assert!(
        (spent - 7.0 * gross).abs() <= 1e-15,
        "the donor spent {spent} of {}",
        7.0 * gross
    );
    assert!(
        (donor.parcel - (7.0 * net - package)).abs() <= 1e-15,
        "the parcel holds {} after sending one {package} package",
        donor.parcel
    );
    // The package itself cost `(1 + c_g)` times its own size out of the reserve, which is
    // the arithmetic Astra's R4.4 asked to be made explicit: 0.06 of reserve for 0.05 net.
    assert!(
        (spent - build * (package + donor.parcel)).abs() <= 1e-15,
        "{spent} of reserve for a {package} package and a {} parcel",
        donor.parcel
    );
    let respired = flora.view().ledger.respired_out;
    assert!(
        (respired - sc.build * 7.0 * net).abs() <= 1e-15,
        "respired_out {respired} for seven ticks of {net} saved"
    );
    let debited = donor0.mineral - donor.mineral;
    // Every unit the donor was debited arrived: construction respiration moves no mineral.
    // The tolerance is relative — the two numbers are a paired subtract and add on stocks
    // three orders of magnitude larger, so they agree to about two ulps and not to the bit.
    assert!(
        (c.mineral - debited).abs() <= 1e-12 * debited,
        "{} of mineral arrived for {debited} debited",
        c.mineral
    );

    // The three reproductive fluxes, which are the diagnosis R4.4 asked for: this donor
    // could pay for everything it asked for, and one package of it has landed.
    let l = flora.view().ledger;
    let i = Species::Bloomcrown.index();
    assert!(
        (l.propagule_requested[i] - 7.0 * net).abs() <= 1e-15,
        "{:?}",
        l.propagule_requested
    );
    assert!(
        (l.propagule_funded[i] - 7.0 * net).abs() <= 1e-15,
        "{:?}",
        l.propagule_funded
    );
    assert!(
        (l.propagule_landed[i] - package).abs() <= 1e-15,
        "{:?}",
        l.propagule_landed
    );
    assert_eq!(
        l.propagule_requested[Species::Umbrellafrond.index()],
        0.0,
        "no frond asked"
    );

    // One new site brought its own `initial_mineral`, which is booked in.
    assert!(
        (flora.view().mineral() - (mineral0 + flora.config().initial_mineral)).abs() < 1e-15,
        "the new ground's mineral is booked, not conjured"
    );
    assert_residuals(&flora, "after one saved package landed");
}

/// A cohort on a site its species cannot establish on ages, pays its attrition into the
/// site's litter, and finally falls to litter whole. It never becomes a stand.
///
/// `bloomcrown.establish_pore_min` is 0.9 here against the placeholder 0.1, which no site
/// in this fixture reaches, so the predicate fails everywhere and germination is the one
/// thing that cannot happen; `seed_max_age_s` is 1 s (placeholder 600) so the age limit
/// fires inside a short test, and its bin is a fifth of that, five ticks.
/// `propagule_rate` is 0.18 /s (placeholder 2e-4) so the donor's parcel is a whole package
/// on the seventh tick; the donor is cleared as soon as it has sent one, so the bank stops
/// being fed and can actually age. Both decomposition rates are 0 (placeholders 0.001 and
/// 0.0001) so that what reaches the litter stays there and can be read off.
///
/// Round 3b: which site receives is the donor's own draw among its four `hop`-2
/// neighbours, so the test finds the bank rather than naming the column.
#[test]
fn a_cohort_on_a_site_that_fails_the_predicate_decays_to_litter_and_never_stands() {
    let mut config = FloraConfig::default();
    config.bloomcrown.establish_pore_min = 0.9;
    config.bloomcrown.seed_max_age_s = 1.0;
    config.bloomcrown.propagule_rate = 0.18;
    config.decomposition = 0.0;
    config.wood_decomposition = 0.0;
    let mut world = plain(8, 8, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 3,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.4
        }
    ));

    run(&mut flora, &mut world, 7);
    assert!(
        flora.apply(&world, Command::Clear { x: 3, z: 0 }),
        "the donor is taken away"
    );
    let banked = flora
        .view()
        .ground
        .iter()
        .find(|g| !g.seeds.is_empty())
        .expect("nothing landed at all")
        .site;
    let landed = flora.view().ground_at(banked).unwrap().seeds[0];
    assert!(landed.organic > 0.0, "nothing landed: {landed:?}");

    // Ten ticks of attrition: the cohort shrinks and the site's litter grows by what it
    // lost, mineral included. Paid decay, not deletion.
    run(&mut flora, &mut world, 10);
    let g = flora.view().ground_at(banked).unwrap().clone();
    let c = g.seeds[0];
    // Seventeen ticks since the bin opened: the package landed on tick 7, in the bin whose
    // five-tick window opened at tick 5, and the run is at tick 17.
    assert_eq!(
        c.bin_start_tick, 5,
        "not the bin the landing tick belonged to: {c:?}"
    );
    assert_eq!(c.age_ticks(flora.tick()), 12, "the bin did not age");
    // Attrition from its **first banked tick**, not from the tick it landed on: the
    // package arrives in step 9, after that tick's decay has been charged, so ten ticks in
    // the bank is ten factors of `1 − seed_attrition_per_s · dt` and no more. K7 moved
    // germination in front of the decay; it did not move the decay itself.
    let attrition = flora
        .config()
        .species(Species::Bloomcrown)
        .seed_attrition_per_s;
    let want = landed.organic * (1.0 - attrition * cubarium_voxel::DT).powi(10);
    assert!(
        (c.organic - want).abs() <= 1e-15,
        "{} banked after ten ticks of attrition, not {want}",
        c.organic
    );
    assert!(c.organic < landed.organic, "it did not decay: {c:?}");
    assert!(
        (g.litter - (landed.organic - c.organic)).abs() < 1e-15,
        "the litter is {} for {} lost",
        g.litter,
        landed.organic - c.organic
    );
    let lost_mineral = landed.mineral - c.mineral;
    assert!(
        (g.litter_mineral - lost_mineral).abs() < 1e-9 * lost_mineral,
        "the litter's mineral is {} for {lost_mineral} lost",
        g.litter_mineral
    );
    assert!(
        flora.view().stands.is_empty(),
        "something germinated on a failing site"
    );

    // Past `seed_max_age_s` — 1 s is 30 ticks — the rest of it falls whole.
    run(&mut flora, &mut world, 25);
    let g = flora.view().ground_at(banked).unwrap().clone();
    assert!(
        g.seeds.is_empty(),
        "the over-age cohort is still there: {:?}",
        g.seeds
    );
    assert!(
        (g.litter - landed.organic).abs() < 1e-15,
        "{} of the cohort's {} reached the litter",
        g.litter,
        landed.organic
    );
    assert!(
        (g.litter_mineral - landed.mineral).abs() < 1e-9 * landed.mineral,
        "{} of the cohort's {} of mineral reached the litter",
        g.litter_mineral,
        landed.mineral
    );
    assert!(
        flora.view().stands.is_empty(),
        "something germinated on a failing site"
    );
    assert_eq!(flora.view().ledger.establishments, 0);
    assert_residuals(&flora, "after a cohort decayed away");
}

/// A bank on a passing site germinates once it can build a living stand, and the stand it
/// builds is its own banked material: alive, over `alive_min`, and carrying the bank's own
/// mineral density rather than `n_tissue`.
///
/// It also pins the arithmetic of round 3b's package, which K7 made whole: a landed
/// package is **exactly** the minimum viable stand's material, `alive_min / w_frac`, so one
/// package on a passing site is **one recruit**, born on the next step out of the whole
/// package at exactly `alive_min` of wood. Germination reads the bank before that tick's
/// attrition is charged for precisely this reason; when attrition went first, one package
/// was 0.1 % short of the threshold for ever and every recruit cost two deliveries.
///
/// `propagule_rate` is 3.0 /s against the placeholder 2e-4 and `reserve_cap` 4.0 against
/// 0.5, so the donor can fund a package every tick for eight ticks and the test is short;
/// `hop` is 1 against 2, so there are two candidate recipients and the draws pile up on
/// them quickly. `assimilation`, `maintenance` and `senescence` are 0 so the donor's
/// reserve is exactly what `Seed` gave it minus what reproduction cost.
#[test]
fn a_bank_over_the_threshold_germinates_into_a_stand_of_its_own_pooled_cohorts() {
    let mut config = FloraConfig::default();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    config.bloomcrown.reserve_cap = 4.0;
    config.bloomcrown.assimilation = 0.0;
    config.bloomcrown.maintenance = 0.0;
    config.bloomcrown.senescence = 0.0;
    let mut world = plain(6, 8, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));
    let sc = flora.config().species(Species::Bloomcrown).clone();
    let package = sc.alive_min / sc.propagule_split[0];
    let build = 1.0 + sc.build;

    // Tick one: one package lands, on one of the two neighbours, and nothing is born.
    flora.step(&mut world);
    let banked: Vec<Site> = flora
        .view()
        .ground
        .iter()
        .filter(|g| !g.seeds.is_empty())
        .map(|g| g.site)
        .collect();
    assert_eq!(banked.len(), 1, "one package a tick: {banked:?}");
    assert!(
        [site(1), site(3)].contains(&banked[0]),
        "{banked:?} is not a hop-1 neighbour"
    );
    let first = flora
        .view()
        .ground_at(banked[0])
        .unwrap()
        .seed_organic(Species::Bloomcrown);
    assert!(
        (first - package).abs() <= 1e-15 * package,
        "a {first} package, not {package}"
    );
    assert_eq!(
        flora.view().ledger.establishments,
        0,
        "born on its landing tick"
    );

    // Tick two: that one package is a recruit. It germinates out of the whole package,
    // before any attrition is charged against it, on the site it landed on.
    flora.step(&mut world);
    assert_eq!(
        flora.view().ledger.establishments,
        1,
        "one package on a passing site is one recruit"
    );
    let at = banked[0];
    let s = *flora.view().stand_at(at).expect("the newborn");
    // The bin it came out of was emptied whole, so the bank on that site is whatever the
    // donor has landed since and nothing older.
    assert!(
        flora
            .view()
            .ground_at(at)
            .unwrap()
            .seed_organic(Species::Bloomcrown)
            <= package,
        "the bin was not spent whole: {:?}",
        flora.view().ground_at(at).unwrap().seeds
    );
    assert_eq!(
        s.stage,
        Stage::Alive,
        "a germinated stand is alive, not establishing"
    );
    assert_eq!(s.species, Species::Bloomcrown);
    assert!(s.wood >= sc.alive_min, "born below alive_min: {}", s.wood);
    // Exactly one package, whatever the bank had grown to by then, and exactly `alive_min`
    // of wood out of it: round 3b's germination spends one package and leaves the rest.
    assert!(
        (s.organic() - package).abs() <= 1e-12 * package,
        "born with {} for a {package} package",
        s.organic()
    );
    assert!(
        (s.wood - sc.alive_min).abs() <= 1e-15,
        "born with {} of wood",
        s.wood
    );
    assert_eq!(s.parcel, 0.0, "a newborn saves nothing yet");
    // Its mineral came with its cohorts, at the **donor's** own density and not `n_tissue`
    // exactly: the mineral is pulled when the package leaves, by the fraction rule over the
    // donor's whole material, so the construction respiration leaves its share of the
    // mineral in the donor exactly as maintenance respiration does. Round 3's rule debited
    // the mineral of the gross `1 + c_g` and sent all of it on, which made every cohort —
    // and every stand born of one — `1 + c_g` times as mineral-rich as its parent. That
    // factor is gone; what is left is the slow enrichment a saving donor accumulates
    // between deliveries.
    let density = s.mineral / s.organic();
    assert!(
        density >= sc.n_tissue && density < build * sc.n_tissue,
        "the newborn's density {density} is outside [n_tissue {}, (1 + c_g) n_tissue {}]",
        sc.n_tissue,
        build * sc.n_tissue
    );
    assert!(
        (density - sc.n_tissue).abs() < 0.1 * sc.n_tissue,
        "the newborn's density {density} is nowhere near its donor's {}",
        sc.n_tissue
    );
    assert_residuals(&flora, "after a bank germinated");
}

/// The gap the seed bank exists for: a site with a living stand still receives cohorts,
/// holds them while the stand lives, and germinates the tick after the stand **dies**.
///
/// The victim is an umbrellafrond with `assimilation` 0 and `maintenance` 0.4 /s
/// (placeholders 0.004 and 0.0002): no income and a bill it cannot pay, so it spends its
/// reserve and diebacks below `alive_min` inside forty ticks. The bloomcrown donor's
/// `propagule_rate` is 3.0 /s and its `hop` 1 (placeholders 2e-4 and 2), so one tick
/// fills the victim's bank past the germination threshold while the victim is still
/// standing on it.
#[test]
fn a_bank_waits_under_a_living_stand_and_germinates_when_it_dies() {
    let mut config = FloraConfig::default();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    config.umbrellafrond.assimilation = 0.0;
    config.umbrellafrond.maintenance = 0.4;
    let mut world = plain(5, 8, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));
    let victim = site(1);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.02
        }
    ));

    // Twenty ticks: the bank on the occupied site is over the threshold and waiting.
    run(&mut flora, &mut world, 20);
    let bank = flora
        .view()
        .ground_at(victim)
        .unwrap()
        .seed_organic(Species::Bloomcrown);
    let w_frac = flora.config().species(Species::Bloomcrown).propagule_split[0];
    let alive_min = flora.config().species(Species::Bloomcrown).alive_min;
    assert!(
        w_frac * bank >= alive_min,
        "the bank is not over the threshold: {bank}"
    );
    assert_eq!(
        flora.view().stand_at(victim).map(|s| s.species),
        Some(Species::Umbrellafrond),
        "the victim should still be standing on its successor's bank"
    );
    assert_eq!(flora.view().ledger.deaths, 0);

    // Forty more: the umbrellafrond has spent its reserve, diebacked and died, and the
    // bank that was waiting under it is a bloomcrown stand.
    run(&mut flora, &mut world, 40);
    assert_eq!(flora.view().ledger.deaths, 1, "the victim did not die");
    let born = *flora.view().stand_at(victim).expect("the gap stayed empty");
    assert_eq!(
        born.species,
        Species::Bloomcrown,
        "the wrong species took the gap"
    );
    assert!(
        born.wood >= alive_min,
        "born below alive_min: {}",
        born.wood
    );
    assert!(flora.view().ledger.establishments >= 1);
    // The dead umbrellafrond's remains are on the same site, under its successor.
    let g = flora.view().ground_at(victim).unwrap();
    assert!(
        g.dead_wood > 0.0 && g.litter > 0.0,
        "the victim left no remains"
    );
    assert_residuals(&flora, "after a gap was filled by its bank");
}

// ------------------------------------------- organic matter and mineral (round 3)

/// Respiration is not fertilizer. A stand with no income pays its maintenance out of its
/// reserve, tick after tick: organic matter leaves the system as `respired_out` and heat,
/// and the site's mineral pool does not move by one bit. What the burned reserve was
/// holding stays in the plant, so the standing tissue ends up richer per unit than
/// `n_tissue`.
///
/// Two rates are the test's own: `maintenance` 0.4 /s (placeholder 0.0002) so the
/// reserve is visibly spent inside twenty ticks, and `senescence` 0 (placeholder 0.001)
/// so litterfall does not also move mineral and the reading is respiration alone.
#[test]
fn respiration_loses_organic_matter_and_releases_no_mineral() {
    let mut world = plain(6, 8, 0.0);
    let mut config = FloraConfig::default();
    config.bloomcrown.maintenance = 0.4;
    config.bloomcrown.senescence = 0.0;
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));

    let n_tissue = flora.config().species(Species::Bloomcrown).n_tissue;
    let stand0 = *flora.view().stand_at(site(2)).expect("seeded");
    let pool0 = flora.view().ground_at(site(2)).expect("ground").mineral;
    let organic0 = flora.view().organic();
    assert_eq!(
        stand0.mineral,
        n_tissue * stand0.organic(),
        "a founder arrives at n_tissue"
    );

    run(&mut flora, &mut world, 20);

    let stand = *flora.view().stand_at(site(2)).expect("still alive");
    let l = flora.view().ledger;
    assert_eq!(l.fixed_in, 0.0, "dry soil is no income");
    assert!(l.respired_out > 0.0, "it respired nothing");
    assert!(
        stand.organic() < stand0.organic(),
        "it lost no organic matter"
    );
    let lost = organic0 - flora.view().organic();
    assert!(
        (lost - l.respired_out).abs() < 1e-12 * l.respired_out,
        "the organic matter that left the system is {lost}, respired {}",
        l.respired_out
    );
    // The two mineral claims: the pool did not move, and the plant kept every unit.
    assert_eq!(
        flora.view().ground_at(site(2)).unwrap().mineral,
        pool0,
        "respiration fertilized the site"
    );
    assert_eq!(
        stand.mineral, stand0.mineral,
        "respiration took mineral out of the plant"
    );
    assert!(
        stand.mineral > n_tissue * stand.organic(),
        "what is left should be mineral-rich: {} for {} of tissue",
        stand.mineral,
        stand.organic()
    );
    assert_residuals(&flora, "after twenty ticks of unpaid maintenance");
}

/// A litter cohort hands the site's pool **exactly** the mineral it held, while its
/// organic matter is respired out of the system rather than becoming pool.
///
/// `decomposition` is 60 /s here against the placeholder 0.001: at `k_d · dt > 1` the
/// whole cohort is eligible in one tick, which is what makes "exactly its mineral" a
/// thing one assertion can say. `wood_decomposition` is 0 (placeholder 0.0001) so the
/// dead wood the same death left behind stays out of the reading.
#[test]
fn a_decomposing_litter_cohort_releases_exactly_its_mineral() {
    let mut config = FloraConfig::default();
    config.decomposition = 60.0;
    config.wood_decomposition = 0.0;

    let mut world = plain(8, 8, 0.5);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    // Drown it: a §4.7 death puts foliage and reserve into litter, with their mineral.
    world.apply(WorldCommand::AddWater {
        x: 2,
        y: 3,
        z: 0,
        volume_m3: 0.2,
    });
    flora.step(&mut world);
    // Take the water away so the litter can rot without anything else happening.
    world.apply(WorldCommand::SetMaterial {
        x: 2,
        y: 3,
        z: 0,
        material: Material::Rock,
    });
    world.apply(WorldCommand::SetMaterial {
        x: 2,
        y: 3,
        z: 0,
        material: Material::Air,
    });

    let before = flora
        .view()
        .ground_at(site(2))
        .expect("its remains")
        .clone();
    assert!(
        before.litter > 0.0 && before.litter_mineral > 0.0,
        "{before:?}"
    );
    let respired0 = flora.view().ledger.respired_out;

    flora.step(&mut world);

    let after = flora
        .view()
        .ground_at(site(2))
        .expect("still there")
        .clone();
    assert_eq!(
        after.litter, 0.0,
        "the whole cohort was eligible: {}",
        after.litter
    );
    assert_eq!(after.litter_mineral, 0.0, "and it kept none of its mineral");
    let released = after.mineral - before.mineral;
    assert!(
        (released - before.litter_mineral).abs() < 1e-12 * before.litter_mineral,
        "the pool got {released} for the cohort's {}",
        before.litter_mineral
    );
    // Its organic matter left the system instead: no pool, no stock, a named flow.
    let respired = flora.view().ledger.respired_out - respired0;
    assert!(
        (respired - before.litter).abs() < 1e-12 * before.litter,
        "the cohort's {} of organic matter left as {respired} of respiration",
        before.litter
    );
    assert_eq!(
        after.dead_wood, before.dead_wood,
        "the dead wood was not in play"
    );
    assert_residuals(&flora, "after one litter cohort decomposed");
}

/// The mineral cap is real: with light and water both saturating, a stand can build no
/// more tissue than `mineral / n_tissue` of the pool it stands on, and when the pool is
/// spent growth stops even though nothing else is limiting. The ample arm is the same
/// fixture with a pool a hundred times larger.
///
/// Four values are the test's own, all to make the *mineral* cap the binding one rather
/// than the Michaelis–Menten that already throttles a small pool: `n_tissue` 1.0
/// (placeholder 0.02), `nutrient_half` 0 and `nutrient_draw_max` 1e9 (placeholders 0.5
/// and 0.01) to take the other two terms out of the way, and `initial_mineral` 1e-4 in
/// the starved arm against 1e-2 in the ample one (placeholder 1.0).
#[test]
fn growth_stops_when_the_site_s_mineral_is_spent_though_light_and_water_are_ample() {
    let arm = |initial_mineral: f64| -> (Flora, World) {
        let mut config = FloraConfig::default();
        config.initial_mineral = initial_mineral;
        config.bloomcrown.n_tissue = 1.0;
        config.bloomcrown.nutrient_half = 0.0;
        config.bloomcrown.nutrient_draw_max = 1e9;
        let mut world = plain(6, 8, 0.6);
        let mut flora = Flora::new(config);
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 2,
                z: 0,
                species: Species::Bloomcrown,
                wood: 0.1
            }
        ));
        run(&mut flora, &mut world, 50);
        (flora, world)
    };

    let (mut starved, mut starved_world) = arm(1e-4);
    let (mut ample, mut ample_world) = arm(1e-2);
    let seeded_mineral = starved.view().stand_at(site(2)).unwrap().mineral;
    let mid = *starved.view().stand_at(site(2)).expect("alive");
    run(&mut starved, &mut starved_world, 150);
    run(&mut ample, &mut ample_world, 150);

    let stand = *starved.view().stand_at(site(2)).expect("still alive");
    assert_eq!(
        stand.light, 1.0,
        "open sky, so light is not what stopped it"
    );
    assert_eq!(
        stand.moisture, 1.0,
        "pore 0.6 is past sat_pore: nor is water"
    );
    // The pool does not reach zero and should not: senescence keeps shedding foliage
    // into litter and decomposition keeps handing that litter's mineral back, so the
    // site settles at the small stock that recycling supports — a fiftieth of what it
    // started with here. That trickle is all the growth there is.
    let g = starved.view().ground_at(site(2)).unwrap();
    assert!(
        g.mineral < 0.02 * 1e-4,
        "the pool is not spent: {}",
        g.mineral
    );
    // Everything the plant drew came out of that one pool, and nowhere else.
    assert!(
        stand.mineral - seeded_mineral <= 1e-4 + 1e-18,
        "it drew {} from a 1e-4 pool",
        stand.mineral - seeded_mineral
    );
    // And growth has stopped, not merely slowed: over the next 150 ticks the wood does
    // not move by one bit, while the ample arm — same light, same water, same rates —
    // keeps growing.
    assert_eq!(
        stand.wood, mid.wood,
        "the starved stand grew after its pool was spent"
    );
    assert!(
        ample.view().stand_at(site(2)).unwrap().wood > stand.wood,
        "the ample arm did not outgrow the starved one"
    );
    assert_residuals(&starved, "after a pool ran out of mineral");
    assert_residuals(&ample, "after 200 ticks on an ample pool");
}

// ------------------------------------------------- root-zone aeration (round 3)

/// A strip whose soil rows are saturated and held there by the water table: pore 0.98 to
/// begin with, which is already past both species' `saturated_pore` of 0.95, and an
/// aquifer charged to 3 m — above the support face at `y = 2` — so the core tops the
/// rows to capacity and `drain` leaves them alone while the table is up. Dropping the
/// table with a negative `ChargeAquifer` is then the one thing that unsaturates them.
///
/// Nothing stands in free water here: the table fills pore space up to capacity and no
/// further, so `water_depth_m` at the support face stays 0 and `drown_depth_m` never
/// fires. This is waterlogged *soil*, which is the thing correction 2 is about.
fn saturated_basin() -> World {
    let config = VoxelConfig {
        width: 6,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 11,
        initial_aquifer_head_m: 3.0,
        ..VoxelConfig::default()
    };
    let mut w = World::empty(config);
    for x in 0..6i64 {
        for y in 1..=2 {
            wet_soil(&mut w, x, y, 0.98);
        }
    }
    w
}

fn coupled(flora: &mut Flora, world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        world.step();
        flora.step(world);
    }
}

/// The three claims of correction 2's income half, on one fixture: a bloomcrown over a
/// saturated root box reaches stress 1 and earns nothing measurable; an umbrellafrond over
/// the same box does not stress at all and keeps earning; and the bloomcrown relaxes once
/// the water table is dropped out from under it.
///
/// No rate is the test's own. Package J's target rule is what sets the two levels: a
/// wholly saturated box is `f` = 1, so bloomcrown's target is 1 (its tolerance is 0.25)
/// and umbrellafrond's is 0 (its tolerance is 1.0, which is no saturation stress at all).
/// The rates only say how fast. Bloomcrown closes `stress_rate_per_s` = 0.2 of the
/// remaining gap per second, so 100 s is 20 time constants and the stress is 1 to within
/// 2e-9 — asymptotically, never exactly, which is why the income below is read against a
/// tolerance and not against zero.
#[test]
fn a_saturated_root_box_stresses_bloomcrown_to_nothing_and_leaves_umbrellafrond_earning() {
    let mut bloom_world = saturated_basin();
    let mut bloom = Flora::new(FloraConfig::default());
    assert!(bloom.apply(
        &bloom_world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    let mut frond_world = saturated_basin();
    let mut frond = Flora::new(FloraConfig::default());
    assert!(frond.apply(
        &frond_world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.1
        }
    ));

    coupled(&mut bloom, &mut bloom_world, 2000);
    coupled(&mut frond, &mut frond_world, 2000);

    let b = *bloom.view().stand_at(site(2)).expect("alive, not drowned");
    let u = *frond.view().stand_at(site(2)).expect("alive, not drowned");
    assert_eq!(
        bloom_world.view().water_depth_m(2, 2, 0),
        0.0,
        "this is wet soil, not a pool"
    );
    assert_eq!(
        bloom.view().ledger.deaths,
        0,
        "drowning is not what is being tested"
    );
    assert_eq!(b.light, 1.0, "open sky: light is not what stopped it");
    assert_eq!(b.moisture, 1.0, "pore 1.0 is past sat_pore: nor is water");
    assert!(
        1.0 - b.aeration_stress < 1e-6,
        "bloomcrown should be fully stressed: {b:?}"
    );
    assert_eq!(
        u.aeration_stress, 0.0,
        "umbrellafrond stressed to {}",
        u.aeration_stress
    );

    // Earning nothing measurable: over the next thirty ticks the bloomcrown fixes about
    // 1.5e-12 of a unit — `1 − stress` is 2e-9 there and its maintenance alone is 1e-6 a
    // tick — its wood does not move by one bit, and its foliage and reserve only fall,
    // where with any income to speak of the shed foliage would be reflushed.
    let fixed = bloom.view().ledger.fixed_in;
    coupled(&mut bloom, &mut bloom_world, 30);
    let after = *bloom.view().stand_at(site(2)).expect("still alive");
    let earned = bloom.view().ledger.fixed_in - fixed;
    assert!(earned < 1e-10, "a fully stressed stand fixed {earned}");
    assert_eq!(after.wood, b.wood, "and it grew");
    assert!(
        after.foliage < b.foliage && after.reserve < b.reserve,
        "{after:?}"
    );
    // The umbrellafrond on the same box is still earning over the same span.
    let frond_fixed = frond.view().ledger.fixed_in;
    coupled(&mut frond, &mut frond_world, 30);
    assert!(
        frond.view().ledger.fixed_in > frond_fixed,
        "umbrellafrond stopped earning"
    );

    // Drop the table out from under it: `drain` takes the root box down to the soil's
    // field capacity, well under `saturated_pore`, and the stress relaxes and the income
    // comes back. `ChargeAquifer` with a negative volume is clamped by what is there.
    let drained = bloom_world.apply(WorldCommand::ChargeAquifer { volume_m3: -1e9 });
    assert!(drained < 0.0, "nothing was drained: {drained}");
    assert_eq!(bloom_world.aquifer_head_m(), 0.0);
    let fixed = bloom.view().ledger.fixed_in;
    coupled(&mut bloom, &mut bloom_world, 600);

    let dry = *bloom.view().stand_at(site(2)).expect("still alive");
    assert!(
        bloom_world.view().pore_at(2, 1, 0) < 0.95,
        "the box is still saturated"
    );
    assert!(
        dry.aeration_stress < 1.0,
        "the stress did not relax: {}",
        dry.aeration_stress
    );
    assert!(
        bloom.view().ledger.fixed_in > fixed,
        "the income did not come back"
    );
    assert_residuals(&bloom, "after a bloomcrown stressed and relaxed");
    assert_residuals(
        &frond,
        "after an umbrellafrond shrugged off a saturated box",
    );
}

/// The establishment half: on a wholly saturated site bloomcrown's bank never germinates
/// and umbrellafrond's does. Both banks are paid for by a donor of their own species and
/// both are over the germination threshold, so the aeration bound is the only difference.
///
/// `propagule_rate` is 3.0 /s for both species here against the placeholder 2e-4, `hop` 1
/// for bloomcrown against the placeholder 2, and `reserve_cap` 4.0 for both against 0.5:
/// together they let each donor fund a whole package every tick for long enough that both
/// of its neighbours hold several, which is what a germination needs now. Round 3b's
/// donors save for **one** recipient at a time and a bloomcrown in a saturated basin earns
/// almost nothing, so at the placeholder `reserve_cap` this fixture's donor could pay for
/// exactly two packages in its whole life and one neighbour ended up with one. The
/// saturation is the fixture's, not a rate.
#[test]
fn bloomcrown_cannot_germinate_on_a_saturated_site_and_umbrellafrond_can() {
    let mut config = FloraConfig::default();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    config.bloomcrown.reserve_cap = 4.0;
    config.umbrellafrond.propagule_rate = 3.0;
    config.umbrellafrond.reserve_cap = 4.0;
    // The fixture is saturated to the bit, so the two ceilings are the whole difference.
    assert_eq!(config.bloomcrown.establish_saturated_max, 0.25);
    assert_eq!(config.umbrellafrond.establish_saturated_max, 1.0);

    // No `world.step`: pore 0.98 is already past both species' `saturated_pore`, so the
    // saturated fraction of every root box is 1 and nothing has to move to keep it there.
    let mut world = saturated_basin();
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 3,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.6
        }
    ));

    // Round 3b: a package lands on one drawn neighbour a tick, and a site needs two of
    // them before a birth is possible at all (one package is exactly `alive_min / w_frac`
    // and attrition shaves it), so this is 60 ticks rather than 21 — long enough for both
    // donors to have fed each of their neighbours several packages.
    run(&mut flora, &mut world, 60);

    let w_frac = flora.config().species(Species::Bloomcrown).propagule_split[0];
    let alive_min = flora.config().species(Species::Bloomcrown).alive_min;
    // Bloomcrown's own hop neighbours hold banks well over the threshold, and no stand.
    for x in [1u32, 5] {
        let g = flora.view().ground_at(site(x)).unwrap();
        let bank = g.seed_organic(Species::Bloomcrown);
        assert!(
            w_frac * bank >= alive_min,
            "site {x} is not over the threshold: {bank}"
        );
        assert!(
            flora.view().stand_at(site(x)).is_none(),
            "bloomcrown germinated on a saturated site: {:?}",
            flora.view().stand_at(site(x))
        );
    }
    // Umbrellafrond's germinated on exactly the same saturation.
    for x in [2u32, 4] {
        let s = flora
            .view()
            .stand_at(site(x))
            .unwrap_or_else(|| panic!("umbrellafrond did not germinate at {x}"));
        assert_eq!(s.species, Species::Umbrellafrond);
        assert!(s.wood >= flora.config().species(Species::Umbrellafrond).alive_min);
    }
    assert_eq!(
        flora.view().ledger.establishments,
        2,
        "one per umbrellafrond bank"
    );
    assert_residuals(&flora, "after a saturated site turned one species away");
}

// ---------------------------------------------------------------- conservation

/// All three residuals over 200 ticks of a rained-on world with stands of both species
/// living, drinking, shedding, seeding and dying.
#[test]
fn the_three_residuals_stay_at_noise_over_two_hundred_ticks_with_rain() {
    let config = VoxelConfig {
        width: 12,
        height: 10,
        depth: 2,
        voxel_m: 1.0,
        seed: 3,
        rain_m_per_s: 0.0005,
        ..VoxelConfig::default()
    };
    let mut world = World::empty(config.clone());
    for z in 0..config.depth {
        for x in 0..config.width as i64 {
            for y in 1..=2 {
                let cap = Material::Soil.pore_capacity() * world.config().voxel_volume();
                world.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: 0.5 * cap,
                });
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    let mut flora = Flora::new(FloraConfig::default());
    for (x, species, wood) in [
        (1i64, Species::Bloomcrown, 0.4),
        (4, Species::Umbrellafrond, 0.5),
        (7, Species::Bloomcrown, 0.05),
        (9, Species::Umbrellafrond, 0.1),
    ] {
        for z in 0..config.depth {
            assert!(flora.apply(
                &world,
                Command::Seed {
                    x,
                    z,
                    species,
                    wood
                }
            ));
        }
    }

    for tick in 0..200 {
        world.step();
        flora.step(&mut world);
        if tick % 25 == 0 {
            assert_residuals(&flora, &format!("at tick {tick}"));
        }
    }
    assert_residuals(&flora, "after 200 ticks");

    // The core's own water residual is untouched by the withdrawals.
    let water = world.view().stored_m3() - world.view().ledger.expected_stored();
    assert!(water.abs() < 1e-9, "water residual {water}");
    assert!(flora.view().ledger.transpired_m3 > 0.0, "nothing drank");
    assert_eq!(
        flora.view().ledger.transpired_m3,
        world.view().ledger.transpiration_out
    );
    assert!(flora.view().ledger.light_in > 0.0, "nothing grew");
}
