//! An independent pass over the plant model: tests written from
//! `design/handoffs/voxel-producers-briefs-2026-09-16.md`, the ecology v1 contract and
//! `design/voxel-ecology-sketch-2026-09-16.md` by a worker that did not write `step.rs`.
//! `tests/flora.rs` is the model worker's own pass; nothing it already pins exactly as
//! the brief states it is repeated here. What is here is the decisions the
//! implementation had to make and nobody pinned: the crown geometry the shade model and
//! the presenter must agree on, a withdrawal shared by two *different* species, the
//! propagule contest from the losing donor's side, and the plant-owned half of the
//! light split.
//!
//! `voxel_m` is 1 m in every fixture, so one soil voxel holds `pore_capacity` = 0.35
//! cubic metres and one voxel of standing water is a metre deep. Wet soil is made
//! exactly, by adding free water to an air cell and then turning it to soil. Rates a
//! test sets are set in its **own** config and named in the commit message; no default
//! moves.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species};

// Round 3 replaced the frozen `Stage::Establishing` stand with a per-site seed bank, so
// the three propagule tests below now read `Ground::seeds` where they read a sub-`W_min`
// stand. The claims they make are the same ones: a package is paid for, it arrives only
// where a donor spent its own reserve, and it never reaches past the donor's `hop`.

// ------------------------------------------------------------------- fixtures

/// One slab deep: bedrock floor, soil at `y = 1` and `y = 2` at a chosen pore fraction,
/// air above. Every column's support face is `y = 2`, in open sky.
fn plain(width: u32, height: u32, pore: f64) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth: 1,
        voxel_m: 1.0,
        seed: 23,
        ..VoxelConfig::default()
    });
    for x in 0..width as i64 {
        for y in 1..=2 {
            wet_soil(&mut w, x, y, pore);
        }
    }
    w
}

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

/// The support face of a column of `plain`.
fn site(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    let o = v.organic() - v.ledger.expected_organic();
    let n = v.mineral() - v.ledger.expected_mineral();
    let e = v.energy() - v.ledger.expected_energy();
    assert!(o.abs() <= 1e-9 * v.organic().abs().max(1.0), "{when}: organic residual {o}");
    assert!(n.abs() <= 1e-9 * v.mineral().abs().max(1.0), "{when}: mineral residual {n}");
    assert!(e.abs() <= 1e-9 * v.energy().abs().max(1.0), "{when}: energy residual {e}");
}

fn run(flora: &mut Flora, world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        flora.step(world);
    }
}

// ================================================== the crown that shades is drawn
//
// The brief makes one crown serve two readers: the shade model attenuates by
// `exp(-shade_k · P_j / area_j)` over the stands whose crown *covers* a site, and the
// presenter draws a disc of `crown_radius(W)` voxels at `support.y + crown_height(W)`.
// If the two ever disagree, the picture stops explaining the simulation. These two tests
// pin the boundary of "covers" and the strictness of "above" against the public
// `SpeciesConfig::crown_radius` / `crown_height` the presenter itself calls.

/// A stand exactly `crown_radius` voxels away is covered; one voxel further is not, and
/// its light is open sky to the bit. The attenuation the covered stand receives is the
/// brief's formula in the crown's own published geometry, not an approximation of it.
///
/// `umbrellafrond.crown_radius_voxels` is `[2.0, 2.0]` here against the placeholder
/// `[1.0, 2.5]`, so the boundary falls on a whole voxel and "exactly at the radius" is a
/// thing a fixture can express; `shade_k` is 30 against the placeholder 1.5, so one
/// crown's attenuation is unmistakable in a single tick. Nothing else is changed.
#[test]
fn shade_covers_exactly_the_crown_radius_the_presenter_draws() {
    let mut config = FloraConfig::default();
    config.shade_k = 30.0;
    config.umbrellafrond.crown_radius_voxels = [2.0, 2.0];

    let mut world = plain(16, 12, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(&world, Command::Seed { x: 8, z: 0, species: Species::Umbrellafrond, wood: 0.6 }));
    // At the radius, and one voxel beyond it.
    assert!(flora.apply(&world, Command::Seed { x: 10, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    assert!(flora.apply(&world, Command::Seed { x: 11, z: 0, species: Species::Bloomcrown, wood: 0.1 }));

    let uc = flora.config().species(Species::Umbrellafrond).clone();
    let bc = flora.config().species(Species::Bloomcrown).clone();
    assert_eq!(uc.crown_radius(0.6), 2.0, "the fixture's premise: a whole-voxel radius");
    assert!(
        uc.crown_height(0.6) > bc.crown_height(0.1),
        "the shading crown has to be the taller one: {} against {}",
        uc.crown_height(0.6),
        bc.crown_height(0.1)
    );

    flora.step(&mut world);

    // The exact attenuation the brief asks for, in the crown's published geometry: the
    // shading stand's foliage is `α · W` as `Seed` left it, spread over `π r²`.
    let area = (std::f64::consts::PI * uc.crown_radius(0.6).powi(2)).max(1.0);
    let l = (-30.0 * (uc.alpha * 0.6) / area).exp();
    let expect = l * (1.0 + bc.light_half) / (l + bc.light_half);
    let covered = flora.view().stand_at(site(10)).expect("at the radius").light;
    assert!(
        (covered - expect).abs() < 1e-12,
        "a stand exactly at the crown radius got {covered}, not the crown's own {expect}"
    );
    assert_eq!(
        flora.view().stand_at(site(11)).expect("beyond the radius").light,
        1.0,
        "one voxel past the crown is open sky, exactly"
    );
    assert_eq!(
        flora.view().stand_at(site(8)).expect("the shading stand").light,
        1.0,
        "and nothing shades the tall stand itself"
    );
    assert_residuals(&flora, "after one tick under a crown");
}

/// Only a *strictly* higher crown top shades. Two stands whose tops are equal and whose
/// crowns cover each other both read open sky; the same crown over a lower one does
/// shade it. Same fixture, same geometry, so the difference is the inequality alone.
#[test]
fn a_crown_level_with_another_does_not_shade_it_but_a_higher_one_does() {
    let mut config = FloraConfig::default();
    config.shade_k = 30.0;
    config.umbrellafrond.crown_radius_voxels = [2.0, 2.0];

    let mut world = plain(16, 12, 0.6);
    let mut flora = Flora::new(config);
    // A level pair, one voxel apart, each inside the other's crown.
    for x in [4i64, 5] {
        assert!(flora.apply(&world, Command::Seed { x, z: 0, species: Species::Umbrellafrond, wood: 0.6 }));
    }
    // And an unequal pair, the same species and the same crown radius, one voxel apart.
    assert!(flora.apply(&world, Command::Seed { x: 10, z: 0, species: Species::Umbrellafrond, wood: 0.6 }));
    assert!(flora.apply(&world, Command::Seed { x: 11, z: 0, species: Species::Umbrellafrond, wood: 0.3 }));

    let uc = flora.config().species(Species::Umbrellafrond).clone();
    // The level pair covers itself — one voxel apart against a two-voxel radius — so
    // "not shaded" can only be the strictness of the inequality on the tops.
    assert!(1.0 <= uc.crown_radius(0.6), "the level pair must cover each other");
    assert!(uc.crown_height(0.3) < uc.crown_height(0.6), "and the unequal pair is not level");

    flora.step(&mut world);
    let v = flora.view();
    assert_eq!(v.stand_at(site(4)).unwrap().light, 1.0, "a level crown does not shade");
    assert_eq!(v.stand_at(site(5)).unwrap().light, 1.0, "in either direction");
    assert_eq!(v.stand_at(site(10)).unwrap().light, 1.0, "nor does a lower crown shade a higher");
    // The lower crown is shaded, by exactly the taller crown's own attenuation.
    let area = (std::f64::consts::PI * uc.crown_radius(0.6).powi(2)).max(1.0);
    let l = (-30.0 * (uc.alpha * 0.6) / area).exp();
    let expect = l * (1.0 + uc.light_half) / (l + uc.light_half);
    let lower = v.stand_at(site(11)).unwrap().light;
    assert!(
        (lower - expect).abs() < 1e-12 && expect < 0.4,
        "the lower crown reads {lower}, not the taller crown's {expect}"
    );
    assert_residuals(&flora, "after one tick beside a level crown");
}

// ============================================== one voxel, two species, one withdrawal

/// Five columns, the middle one with bedrock where its lower soil would be, so that the
/// two stands' root boxes overlap in **exactly one** voxel: `(2, 2, 0)`.
///
/// Both species root one voxel sideways, so `x = 1`'s box is columns 0..2 and `x = 3`'s
/// is columns 2..4. Umbrellafrond roots four voxels down against bloomcrown's two, but
/// the support is at `y = 2` and `y = 0` is the bedrock foundation, so both boxes are the
/// soil of their three columns and the difference does not show here.
fn one_shared_voxel(pore: f64) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 5,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 29,
        ..VoxelConfig::default()
    });
    for x in 0..5i64 {
        if x == 2 {
            w.apply(WorldCommand::SetMaterial { x, y: 1, z: 0, material: Material::Bedrock });
            wet_soil(&mut w, x, 2, pore);
        } else {
            for y in 1..=2 {
                wet_soil(&mut w, x, y, pore);
            }
        }
    }
    w
}

/// The brief's hard case: one voxel, two stands of **different** species with different
/// moisture responses, one `WithdrawPore`, and a split that is exactly proportional to
/// demand. The two ledgers must agree to the bit — the flora crate's `transpired_m3` is
/// the core's `transpiration_out`, not a parallel accounting of it.
///
/// `transpiration_m3_per_s` is 500 for both species here against the placeholder 2e-5:
/// scarcity is the point, since with water to spare each stand simply gets what it asked
/// for and the split is never exercised. Nothing else is changed.
#[test]
fn one_voxel_shared_by_two_species_splits_exactly_by_demand() {
    let mut config = FloraConfig::default();
    config.bloomcrown.transpiration_m3_per_s = 500.0;
    config.umbrellafrond.transpiration_m3_per_s = 500.0;

    let pore = 0.6;
    let mut world = one_shared_voxel(pore);
    let mut flora = Flora::new(config);
    assert!(flora.apply(&world, Command::Seed { x: 1, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    assert!(flora.apply(&world, Command::Seed { x: 3, z: 0, species: Species::Umbrellafrond, wood: 0.1 }));

    // One voxel's stock, and the whole strip's: four two-voxel columns and one of one.
    let per_voxel = pore * Material::Soil.pore_capacity() * world.config().voxel_volume();
    let stock0 = pore_stock(&world);
    assert!((stock0 - 9.0 * per_voxel).abs() < 1e-12, "{stock0} is not nine voxels of soil");

    flora.step(&mut world);

    let b = *flora.view().stand_at(site(1)).expect("the bloomcrown");
    let u = *flora.view().stand_at(site(3)).expect("the umbrellafrond");
    // Two species, two moisture responses over the same water: that is what makes the
    // demands differ and the split observable.
    let bs = flora.config().species(Species::Bloomcrown).clone();
    let us = flora.config().species(Species::Umbrellafrond).clone();
    assert_eq!(b.moisture, 1.0, "pore 0.6 is past bloomcrown's sat_pore of {}", bs.sat_pore);
    assert!((u.moisture - 0.6).abs() < 1e-12, "umbrellafrond's ramp: {}", u.moisture);

    let demand_b = bs.transpiration_m3_per_s * (bs.alpha * 0.1) * b.moisture * cubarium_voxel::DT;
    let demand_u = us.transpiration_m3_per_s * (us.alpha * 0.1) * u.moisture * cubarium_voxel::DT;
    assert!(demand_b / 5.0 > per_voxel, "the fixture has to be starved to test a split");

    // Four voxels are the bloomcrown's alone and four the umbrellafrond's; the fifth is
    // shared, and there the two demands share the stock in their own proportion.
    let shared_b = per_voxel * demand_b / (demand_b + demand_u);
    let shared_u = per_voxel * demand_u / (demand_b + demand_u);
    let want_b = 4.0 * per_voxel + shared_b;
    let want_u = 4.0 * per_voxel + shared_u;
    assert!((b.water_m3 - want_b).abs() < 1e-12 * want_b, "{} against {want_b}", b.water_m3);
    assert!((u.water_m3 - want_u).abs() < 1e-12 * want_u, "{} against {want_u}", u.water_m3);
    // The shared voxel alone, as a ratio: exactly the ratio of the demands.
    let (got_b, got_u) = (b.water_m3 - 4.0 * per_voxel, u.water_m3 - 4.0 * per_voxel);
    assert!(
        (got_b / got_u - demand_b / demand_u).abs() < 1e-9,
        "the shared voxel split {got_b} to {got_u}, not as {demand_b} to {demand_u}"
    );

    // One withdrawal per voxel, and the two ledgers are one number.
    assert_eq!(
        world.view().ledger.transpiration_out,
        flora.view().ledger.transpired_m3,
        "the core's transpiration_out and the flora's transpired_m3 must be bit-identical"
    );
    let total = b.water_m3 + u.water_m3;
    assert!(
        (flora.view().ledger.transpired_m3 - total).abs() < 1e-15 * total,
        "the shares do not sum to what was taken: {} against {total}",
        flora.view().ledger.transpired_m3
    );
    assert!(pore_stock(&world) >= 0.0, "the soil went negative");
    assert!(pore_stock(&world) < 1e-15, "every root voxel should be dry: {}", pore_stock(&world));
    assert!(
        (stock0 - total).abs() < 1e-12 * stock0,
        "the soil lost {} for {total} taken",
        stock0 - pore_stock(&world)
    );
    assert!(
        (world.view().stored_m3() - world.view().ledger.expected_stored()).abs() < 1e-9,
        "the core's water residual moved"
    );
    assert_residuals(&flora, "after one shared drink");
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

// =================================================================== propagules
//
// §4.8 is paid: material arrives only where a donor's own reserve paid for it. Three
// claims of the brief's that `tests/flora.rs` does not reach, all from the donor's side.

/// A frozen species: no income, no maintenance, no senescence, so its stocks move only
/// when a propagule debits them. Three rates set to zero in the test's own config; it is
/// an instrument, not a tuning, and it is the only way to read a *debit* out of a stand
/// that is also growing.
fn frozen(sc: &mut cubarium_voxel_flora::SpeciesConfig) {
    sc.assimilation = 0.0;
    sc.maintenance = 0.0;
    sc.senescence = 0.0;
}

/// Two species reaching the same two bare sites. Round 3 has no contest to settle — a
/// site holds a bank per species — so the claim is the one that survives that change:
/// each donor is debited exactly for its **own** cohort, and one species' package never
/// lands in the other species' cohort.
///
/// `bloomcrown.propagule_rate` is 4e-4 here against the placeholder 2e-4, so the two
/// banks are visibly different sizes rather than the same number twice; `bloomcrown.hop`
/// is 1 against the placeholder 2, so both donors reach exactly the same two sites on a
/// four-column ring; both species are frozen (see `frozen`) so a reserve moves only where
/// a propagule debits it. Nothing else is changed.
#[test]
fn two_species_banks_share_one_site_and_each_donor_pays_only_its_own() {
    let mut config = FloraConfig::default();
    config.bloomcrown.propagule_rate = 4e-4;
    config.bloomcrown.hop = 1;
    frozen(&mut config.bloomcrown);
    frozen(&mut config.umbrellafrond);

    let mut world = plain(4, 8, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(&world, Command::Seed { x: 0, z: 0, species: Species::Umbrellafrond, wood: 0.4 }));
    assert!(flora.apply(&world, Command::Seed { x: 2, z: 0, species: Species::Bloomcrown, wood: 0.4 }));

    let bs = flora.config().species(Species::Bloomcrown).clone();
    let us = flora.config().species(Species::Umbrellafrond).clone();
    let seeded_reserve = us.reserve_cap * 0.4;
    let each_b = bs.propagule_rate * cubarium_voxel::DT;
    let each_u = us.propagule_rate * cubarium_voxel::DT;
    assert!(each_b > each_u, "the fixture's premise: {each_b} against {each_u}");

    flora.step(&mut world);

    // Both bare sites hold two cohorts, one per species, each the size its own donor
    // paid for. Sorted by species: bloomcrown first, as `Species::ALL` orders them.
    for x in [1u32, 3] {
        let g = flora.view().ground_at(site(x)).unwrap_or_else(|| panic!("nothing at {x}"));
        assert_eq!(g.seeds.len(), 2, "one cohort per species: {:?}", g.seeds);
        assert_eq!(g.seeds[0].species, Species::Bloomcrown, "sorted by species");
        assert_eq!(g.seeds[1].species, Species::Umbrellafrond);
        for (c, (each, sc)) in g.seeds.iter().zip([(each_b, &bs), (each_u, &us)]) {
            let net = each / (1.0 + sc.build);
            assert!(
                (c.organic - net).abs() < 1e-12 * net,
                "{c:?} for a {net} package"
            );
        }
        assert!(flora.view().stand_at(site(x)).is_none(), "a cohort is not a stand");
    }
    // Each donor paid exactly its two packages and not the other's.
    for (x, each) in [(0u32, each_u), (2, each_b)] {
        let s = *flora.view().stand_at(site(x)).unwrap();
        assert!(
            (seeded_reserve - s.reserve - 2.0 * each).abs() < 1e-15,
            "the donor at {x} spent {} for two {each} packages",
            seeded_reserve - s.reserve
        );
        assert_eq!(s.wood, 0.4, "the frozen donor's wood moved");
    }
    assert_residuals(&flora, "after two species banked on one site");
}

/// A site outside the donor's `hop` never receives, and the patch does not creep: forty
/// ticks of one donor at the placeholder rate is nowhere near a bank that can germinate,
/// so the banked set is still the donor's own hop neighbourhood and the donor is still
/// the only stand.
#[test]
fn nothing_outside_hop_ever_receives_and_the_patch_does_not_creep() {
    let mut world = plain(16, 8, 0.6);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(&world, Command::Seed { x: 7, z: 0, species: Species::Bloomcrown, wood: 0.4 }));
    let sc = flora.config().species(Species::Bloomcrown).clone();
    assert_eq!(sc.hop, 2, "the placeholder hop this test reads");

    run(&mut flora, &mut world, 40);

    let banked: Vec<u32> =
        flora.view().ground.iter().filter(|g| !g.seeds.is_empty()).map(|g| g.site.x).collect();
    assert_eq!(banked, vec![5, 6, 8, 9], "the banks are the donor's hop, and never its own site");
    let stands: Vec<u32> = flora.view().stands.iter().map(|s| s.site.x).collect();
    assert_eq!(stands, vec![7], "something germinated: {stands:?}");
    for x in [5u32, 6, 8, 9] {
        let g = flora.view().ground_at(site(x)).unwrap();
        assert_eq!(g.seeds.len(), 1, "the merge should keep one cohort: {:?}", g.seeds);
        let c = g.seeds[0];
        // One bin, not one cohort per tick: the placeholders' bin is 3,000 ticks wide, so
        // all forty landings joined the bin that opened at tick 0 — and that bin's age is
        // the run's own forty ticks, which is the half of the rule R4.1 corrected.
        assert_eq!(c.bin_start_tick, 0, "not the bin tick 0 opened: {c:?}");
        assert_eq!(c.age_ticks(flora.tick()), 40, "a fed bank stopped ageing: {c:?}");
        assert!(
            sc.propagule_split[0] * c.organic < sc.alive_min,
            "forty ticks is not a germinating bank: {c:?}"
        );
    }
    assert_eq!(flora.view().ledger.establishments, 0);
    assert_residuals(&flora, "after forty ticks of one donor");
}

/// The donor's side of §4.8: what left the donor's reserve is exactly what arrived, as
/// seed cohorts plus the construction respiration. Nothing is created by a propagule and
/// nothing is lost in one.
///
/// Bloomcrown is frozen here (see `frozen`) so that the reserve's whole change over the
/// tick is the debit; with income on, growth moves the same stock and the debit cannot be
/// read off it.
#[test]
fn a_donor_is_debited_exactly_what_arrives_plus_its_construction() {
    let mut config = FloraConfig::default();
    frozen(&mut config.bloomcrown);
    let mut world = plain(8, 8, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(&world, Command::Seed { x: 3, z: 0, species: Species::Bloomcrown, wood: 0.4 }));

    let sc = flora.config().species(Species::Bloomcrown).clone();
    let reserve0 = sc.reserve_cap * 0.4;
    assert_eq!(flora.view().stand_at(site(3)).unwrap().reserve, reserve0);
    let heat0 = flora.view().ledger.heat_out;
    let respired0 = flora.view().ledger.respired_out;

    flora.step(&mut world);

    let spent = reserve0 - flora.view().stand_at(site(3)).unwrap().reserve;
    let targets = [1u32, 2, 4, 5];
    let budget = sc.propagule_rate * cubarium_voxel::DT * targets.len() as f64;
    assert!((spent - budget).abs() < 1e-15, "the donor spent {spent} of a {budget} budget");

    let mut arrived = 0.0;
    for x in targets {
        let g = flora.view().ground_at(site(x)).unwrap_or_else(|| panic!("nothing at {x}"));
        assert_eq!(g.seeds.len(), 1, "{:?}", g.seeds);
        assert_eq!(g.seeds[0].species, Species::Bloomcrown);
        arrived += g.seeds[0].organic;
        // Round 3: the construction respiration is *not* deposited on the recipient. It
        // leaves the system as organic matter, so the site's pool is its starting
        // mineral to the bit.
        assert_eq!(g.mineral, flora.config().initial_mineral, "construction fertilized the site");
        assert!(flora.view().stand_at(site(x)).is_none(), "a package is a cohort, not a stand");
    }
    // What left the donor is what arrived plus what the build respired, and the respired
    // half is a named boundary flow rather than a stock somewhere.
    let net = spent / (1.0 + sc.build);
    let respired = flora.view().ledger.respired_out - respired0;
    assert!(
        (arrived + respired - spent).abs() < 1e-15,
        "{arrived} arrived and {respired} respired for {spent} spent"
    );
    assert!(
        (respired - sc.build * net).abs() < 1e-15,
        "construction respired {respired} for a net {net} package"
    );
    // The construction respiration is booked as heat too, at the species' own density.
    let heat = flora.view().ledger.heat_out - heat0;
    assert!(
        (heat - sc.energy_density * sc.build * net).abs() < 1e-15,
        "construction heat {heat} for a net {net} package"
    );
    assert_residuals(&flora, "after one paid round of propagules");
}

// ====================================================== the light split, plant side
//
// The brief's light is `sky_visibility` from the core times the plant layer's own canopy
// attenuation. The core half is pinned in `cubarium-voxel/tests/boundary.rs`; this is the
// half that needs a canopy to exist: a stand changes no terrain, so it must not move the
// terrain's own answer, and everything a canopy does has to show up in the *plant's*
// light instead.

#[test]
fn a_canopy_changes_no_sky_visibility_and_all_of_the_plant_s_light() {
    let mut world = plain(16, 12, 0.6);
    let before: Vec<f64> =
        (0..16i64).map(|x| world.view().sky_visibility(x, 2, 0)).collect();
    let version = world.terrain_version();
    assert!(before.iter().all(|&v| v == 1.0), "the fixture is an open plain: {before:?}");

    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(&world, Command::Seed { x: 8, z: 0, species: Species::Umbrellafrond, wood: 0.6 }));
    assert!(flora.apply(&world, Command::Seed { x: 9, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    run(&mut flora, &mut world, 5);

    let after: Vec<f64> = (0..16i64).map(|x| world.view().sky_visibility(x, 2, 0)).collect();
    assert_eq!(before, after, "a canopy is not terrain and may not change its geometry");
    assert_eq!(world.terrain_version(), version, "a plant committed a material change");

    // And the canopy is doing something — in the plant layer's number, at the default
    // `shade_k`, with no rate touched.
    let shaded = flora.view().stand_at(site(9)).expect("under the crown").light;
    assert!(
        shaded < after[9],
        "the covered stand's light {shaded} is not below its site's sky visibility {}",
        after[9]
    );
    assert_eq!(flora.view().stand_at(site(8)).unwrap().light, 1.0, "the taller stand is unshaded");
    assert_residuals(&flora, "after five ticks under a canopy");
}
