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
    let config =
        VoxelConfig { width, height, depth: 1, voxel_m: 1.0, seed: 7, ..VoxelConfig::default() };
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
        let got = w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: pore * cap });
        assert!((got - pore * cap).abs() < 1e-12, "the void took {got} of {}", pore * cap);
    }
    w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
    assert!((w.view().pore_at(x, y, 0) - pore).abs() < 1e-12, "{}", w.view().pore_at(x, y, 0));
    assert_eq!(w.view().free_at(x, y, 0), 0.0, "nothing may be left standing");
}

fn site(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

fn material_residual(flora: &Flora) -> f64 {
    let v = flora.view();
    v.material() - v.ledger.expected_material()
}

fn energy_residual(flora: &Flora) -> f64 {
    let v = flora.view();
    v.energy() - v.ledger.expected_energy()
}

fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    let m = material_residual(flora);
    let e = energy_residual(flora);
    assert!(m.abs() <= 1e-9 * v.material().abs().max(1.0), "{when}: material residual {m}");
    assert!(e.abs() <= 1e-9 * v.energy().abs().max(1.0), "{when}: energy residual {e}");
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
    assert!(flora.apply(&world, Command::Seed { x: 3, z: 0, species: Species::Bloomcrown, wood: 0.1 }));

    let before = *flora.view().stand_at(site(3)).expect("seeded");
    let nutrient0 = flora.view().ground_at(site(3)).expect("ground").nutrient;
    run(&mut flora, &mut world, 100);

    let after = *flora.view().stand_at(site(3)).expect("still there");
    assert_eq!(after.light, 1.0, "open sky and a saturating light response");
    assert_eq!(after.moisture, 1.0, "pore 0.6 is past sat_pore");
    assert!(after.wood > before.wood, "wood {} -> {}", before.wood, after.wood);
    assert!(after.foliage > 0.9 * before.foliage, "foliage held up: {}", after.foliage);
    assert!(flora.view().ledger.light_in > 0.0, "it fixed no light");
    assert!(flora.view().ledger.transpired_m3 > 0.0, "it drank nothing");
    assert_eq!(
        flora.view().ledger.transpired_m3,
        world.view().ledger.transpiration_out,
        "the two ledgers must agree to the bit"
    );

    // Growth is paid out of the site's own nutrient, and some of it is respired back.
    let nutrient = flora.view().ground_at(site(3)).expect("ground").nutrient;
    assert!(nutrient < nutrient0, "nutrient {nutrient0} -> {nutrient}");
    assert!(flora.view().ground_at(site(3)).unwrap().litter > 0.0, "senescence sheds litter");
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
    assert!(flora.apply(&world, Command::Seed { x: 2, z: 0, species: Species::Bloomcrown, wood: 0.1 }));

    flora.step(&mut world);
    let stand = *flora.view().stand_at(site(2)).expect("still alive after one tick");
    assert_eq!(stand.moisture, 0.0, "dry soil is wilting point");
    assert_eq!(flora.view().ledger.light_in, 0.0, "no water, no income");
    assert_eq!(flora.view().ledger.transpired_m3, 0.0, "and nothing to drink");

    run(&mut flora, &mut world, 200);
    assert!(flora.view().stand_at(site(2)).is_none(), "it should be dead");
    assert_eq!(flora.view().ledger.deaths, 1);
    let g = flora.view().ground_at(site(2)).expect("its remains");
    assert!(g.dead_wood > 0.0, "wood becomes dead wood");
    assert!(g.litter > 0.0, "foliage and reserve become litter");
    assert!(g.nutrient > 0.0, "and decomposition returns some of it");
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
    config.shade_k = 30.0;

    let mut world = plain(16, 12, 0.6);
    let mut flora = Flora::new(config);
    // The shaded pair at x = 4/5, and a lone control at x = 12.
    assert!(flora.apply(&world, Command::Seed { x: 4, z: 0, species: Species::Umbrellafrond, wood: 0.6 }));
    assert!(flora.apply(&world, Command::Seed { x: 5, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    assert!(flora.apply(&world, Command::Seed { x: 12, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    run(&mut flora, &mut world, 100);

    let shaded = *flora.view().stand_at(site(5)).expect("shaded stand");
    let open = *flora.view().stand_at(site(12)).expect("control stand");
    assert!(shaded.light < 0.5 * open.light, "shaded {} vs open {}", shaded.light, open.light);
    assert!(shaded.wood < open.wood, "shaded wood {} vs open {}", shaded.wood, open.wood);
    assert!(shaded.foliage < open.foliage, "shaded foliage {} vs open {}", shaded.foliage, open.foliage);
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
#[test]
fn a_stand_that_cannot_pay_under_a_closed_canopy_diebacks_and_dies() {
    let mut config = FloraConfig::default();
    config.shade_k = 30.0;
    config.bloomcrown.maintenance = 0.0032;
    config.bloomcrown.reserve_cap = 0.001;
    config.bloomcrown.dieback = 3000.0;

    let mut world = plain(16, 12, 0.6);
    let mut flora = Flora::new(config);
    for x in [3i64, 4, 6, 7] {
        assert!(flora.apply(&world, Command::Seed { x, z: 0, species: Species::Umbrellafrond, wood: 0.6 }));
    }
    assert!(flora.apply(&world, Command::Seed { x: 5, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    assert!(flora.apply(&world, Command::Seed { x: 12, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    run(&mut flora, &mut world, 40);

    // The bloomcrown is dead — and the gap under the canopy does not stay empty: the
    // four donors put an establishing umbrellafrond on it within a tick or two, which is
    // the succession this round is for.
    let at5 = flora.view().stand_at(site(5));
    assert!(
        at5.is_none_or(|s| s.species == Species::Umbrellafrond && s.stage == Stage::Establishing),
        "the covered bloomcrown should be dead: {at5:?}"
    );
    let open = flora.view().stand_at(site(12)).expect("the open stand lives");
    assert!(open.wood >= 0.1, "the open stand paid its way: wood {}", open.wood);
    assert_eq!(flora.view().ledger.deaths, 1);
    assert!(flora.view().ground_at(site(5)).unwrap().dead_wood > 0.0);
    // The four crowns are level with each other, so none of them shades another.
    for x in [3, 4, 6, 7] {
        assert_eq!(flora.view().stand_at(site(x)).unwrap().light, 1.0, "crown at {x}");
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
    assert!(flora.apply(&world, Command::Seed { x: 0, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    assert!(flora.apply(&world, Command::Seed { x: 1, z: 0, species: Species::Bloomcrown, wood: 0.2 }));

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
    assert!((a.water_m3 - want_a).abs() < 1e-8 * want_a, "{} vs {want_a}", a.water_m3);
    assert!((b.water_m3 / a.water_m3 - 2.0).abs() < 1e-6, "twice the foliage, twice the water");

    let total = a.water_m3 + b.water_m3;
    assert!((flora.view().ledger.transpired_m3 - total).abs() < 1e-12 * total, "the shares sum");
    assert_eq!(
        world.view().ledger.transpiration_out,
        flora.view().ledger.transpired_m3,
        "the core books exactly the sum of the shares"
    );
    assert!((stock0 - pore_stock(&world) - total).abs() < 1e-15, "the soil lost exactly that");
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
    assert!(flora.apply(&world, Command::Seed { x: 0, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    assert!(flora.apply(&world, Command::Seed { x: 1, z: 0, species: Species::Bloomcrown, wood: 0.2 }));

    // Only the two root rows hold water; the stands' boxes are the whole of them.
    let stock0 = pore_stock(&world);
    flora.step(&mut world);

    let a = flora.view().stand_at(site(0)).unwrap().water_m3;
    let b = flora.view().stand_at(site(1)).unwrap().water_m3;
    assert!((a + b - stock0).abs() < 1e-15, "{a} + {b} is not the {stock0} that was there");
    assert!(pore_stock(&world) < 1e-15, "the box is empty, not negative");
    assert!(pore_stock(&world) >= 0.0);
    assert!((b / a - 2.0).abs() < 1e-9, "the split follows demand: {a} and {b}");
    assert_eq!(world.view().ledger.transpiration_out, flora.view().ledger.transpired_m3);
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
fn standing_water_drowns_bloomcrown_at_once_and_umbrellafrond_only_past_its_limit() {
    let mut world = plain(8, 8, 0.5);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(&world, Command::Seed { x: 2, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    assert!(flora.apply(&world, Command::Seed { x: 5, z: 0, species: Species::Umbrellafrond, wood: 0.1 }));

    // 0.2 m of water over both faces: past bloomcrown's zero tolerance, inside
    // umbrellafrond's 0.5 m.
    for x in [2, 5] {
        world.apply(WorldCommand::AddWater { x, y: 3, z: 0, volume_m3: 0.2 });
    }
    flora.step(&mut world);
    assert!(flora.view().stand_at(site(2)).is_none(), "bloomcrown drowns in a puddle");
    assert!(flora.view().stand_at(site(5)).is_some(), "umbrellafrond stands in it");
    assert_eq!(flora.view().ledger.deaths, 1);
    let g = flora.view().ground_at(site(2)).unwrap();
    assert!(g.dead_wood > 0.0 && g.litter > 0.0, "a drowning is a §4.7 death");

    // Half a metre more and it is past the limit too.
    world.apply(WorldCommand::AddWater { x: 5, y: 3, z: 0, volume_m3: 0.5 });
    assert!(world.view().water_depth_m(5, 2, 0) > 0.5);
    flora.step(&mut world);
    assert!(flora.view().stand_at(site(5)).is_none(), "and now it drowns");
    assert_eq!(flora.view().ledger.deaths, 2);
    assert_residuals(&flora, "after two drownings");
}

// ----------------------------------------------------------------------- burial

#[test]
fn a_terrain_edit_that_buries_a_support_books_the_stand_out() {
    let mut world = plain(8, 8, 0.5);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(&world, Command::Seed { x: 2, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    run(&mut flora, &mut world, 5);

    let stand = *flora.view().stand_at(site(2)).unwrap();
    let ground = *flora.view().ground_at(site(2)).unwrap();
    let material = stand.wood + stand.foliage + stand.reserve;
    let ground_material = ground.nutrient + ground.litter + ground.dead_wood;

    // Rock in the void above the face: it is no longer a support.
    world.apply(WorldCommand::SetMaterial { x: 2, y: 3, z: 0, material: Material::Rock });
    assert!(!world.view().is_support(2, 2, 0));
    flora.step(&mut world);

    assert!(flora.view().stand_at(site(2)).is_none(), "the stand is gone");
    assert!(flora.view().ground_at(site(2)).is_none(), "and so is its ground");
    let l = flora.view().ledger;
    assert!(
        (l.removed_material_out - (material + ground_material)).abs() < 1e-15,
        "booked {} for {material} + {ground_material}",
        l.removed_material_out
    );
    assert_eq!(l.deaths, 0, "a burial is not a death: it is a removal");
    assert_residuals(&flora, "after a burial");
}

// ------------------------------------------------------------------- propagules

#[test]
fn a_donor_with_a_full_reserve_establishes_its_neighbours() {
    let mut world = plain(8, 8, 0.6);
    let mut flora = Flora::new(FloraConfig::default());
    // Wood 0.4 is over bloomcrown's donor_min of 0.3, and `Seed` fills the reserve.
    assert!(flora.apply(&world, Command::Seed { x: 3, z: 0, species: Species::Bloomcrown, wood: 0.4 }));
    let sc = flora.config().species(Species::Bloomcrown).clone();
    let material0 = flora.view().material();

    flora.step(&mut world);

    // hop = 2, one slab: the four columns either side, never the donor's own site.
    let established: Vec<Site> = flora
        .view()
        .stands
        .iter()
        .filter(|s| s.stage == Stage::Establishing)
        .map(|s| s.site)
        .collect();
    assert_eq!(established, vec![site(1), site(2), site(4), site(5)], "{established:?}");

    // The budget is the rate, not the reserve: `k_est · dt · n` shared equally.
    let each = sc.propagule_rate * cubarium_voxel::DT * 4.0 / 4.0;
    let net = each / (1.0 + sc.build);
    for s in established {
        let stand = flora.view().stand_at(s).unwrap();
        assert!((stand.wood - sc.propagule_split[0] * net).abs() < 1e-12 * net, "{stand:?}");
        assert!((stand.foliage - sc.propagule_split[1] * net).abs() < 1e-12 * net, "{stand:?}");
        assert!((stand.reserve - sc.propagule_split[2] * net).abs() < 1e-12 * net, "{stand:?}");
        assert!(stand.wood < sc.alive_min, "a propagule is not born alive");
        let g = flora.view().ground_at(s).unwrap();
        assert!(
            (g.nutrient - (flora.config().initial_nutrient + sc.build * net)).abs() < 1e-15,
            "construction nutrient lands in the recipient's ground: {g:?}"
        );
    }
    assert_eq!(flora.view().ledger.establishments, 0, "none has crossed alive_min yet");
    // Four new sites each brought their own `initial_nutrient`, which is booked in.
    assert!(
        flora.view().material() > material0,
        "the new ground's nutrient is booked, not conjured"
    );
    assert_residuals(&flora, "after one round of propagules");
}

// ---------------------------------------------------------------- conservation

/// Both residuals over 200 ticks of a rained-on world with stands of both species
/// living, drinking, shedding, seeding and dying.
#[test]
fn both_residuals_stay_at_noise_over_two_hundred_ticks_with_rain() {
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
                world.apply(WorldCommand::AddWater { x, y, z, volume_m3: 0.5 * cap });
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
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
            assert!(flora.apply(&world, Command::Seed { x, z, species, wood }));
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
    assert_eq!(flora.view().ledger.transpired_m3, world.view().ledger.transpiration_out);
    assert!(flora.view().ledger.light_in > 0.0, "nothing grew");
}
