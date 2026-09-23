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
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species, SpeciesConfig};

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

/// The support face of a column of `plain`.
fn site(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    let o = v.organic() - v.ledger.expected_organic();
    let n = v.mineral() - v.ledger.expected_mineral();
    let e = v.energy() - v.ledger.expected_energy();
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
/// `umbrellafrond.crown_radius_m` is `[2.0, 2.0]` (two 1 m voxels) here against the placeholder
/// `[0.375, 0.75]`, so the boundary falls on a whole voxel and "exactly at the radius" is a
/// thing a fixture can express; `shade_k` is 30 against the placeholder 1.5, so one
/// crown's attenuation is unmistakable in a single tick. Nothing else is changed.
#[test]
fn shade_covers_exactly_the_crown_radius_the_presenter_draws() {
    let mut config = FloraConfig::default();
    config.shade_k_per_m2 = 30.0;
    config.umbrellafrond.crown_radius_m = [2.0, 2.0];
    // These two tests predate layers and are about the shade rule's **geometry** and
    // the strictness of its inequality, both of which are stated over one disc. The
    // shading species is therefore given the one-layer, zero-porosity profile the rule
    // was written against; what a *tiered* frond does to the light under it is
    // `tests/layers.rs`'s business, not this fixture's.
    config.umbrellafrond.profile = SpeciesConfig::one_stage(vec![SpeciesConfig::foliage_layer(
        [0.0, 1.0],
        1.0,
        1.0,
        0.0,
    )]);
    config.bloomcrown.profile = SpeciesConfig::one_stage(vec![SpeciesConfig::foliage_layer(
        [0.0, 1.0],
        1.0,
        1.0,
        0.0,
    )]);

    let mut world = plain(16, 12, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 8,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.6
        }
    ));
    // At the radius, and one voxel beyond it.
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 10,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 11,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));

    let uc = flora.config().species(Species::Umbrellafrond).clone();
    let bc = flora.config().species(Species::Bloomcrown).clone();
    assert_eq!(
        uc.crown_radius(0.6, 1.0),
        2.0,
        "the fixture's premise: a whole-voxel radius"
    );
    assert!(
        uc.crown_height(0.6, 1.0) > bc.crown_height(0.1, 1.0),
        "the shading crown has to be the taller one: {} against {}",
        uc.crown_height(0.6, 1.0),
        bc.crown_height(0.1, 1.0)
    );

    flora.step(&mut world);

    // The exact attenuation the brief asks for, in the crown's published geometry: the
    // shading stand's foliage is `α · W` as `Seed` left it, spread over `π r²`.
    let area = (std::f64::consts::PI * uc.crown_radius(0.6, 1.0).powi(2)).max(1.0);
    let l = (-30.0 * (uc.alpha * 0.6) / area).exp();
    let expect = l * (1.0 + bc.light_half) / (l + bc.light_half);
    let covered = flora
        .view()
        .stand_at(site(10))
        .expect("at the radius")
        .light;
    assert!(
        (covered - expect).abs() < 1e-12,
        "a stand exactly at the crown radius got {covered}, not the crown's own {expect}"
    );
    assert_eq!(
        flora
            .view()
            .stand_at(site(11))
            .expect("beyond the radius")
            .light,
        1.0,
        "one voxel past the crown is open sky, exactly"
    );
    assert_eq!(
        flora
            .view()
            .stand_at(site(8))
            .expect("the shading stand")
            .light,
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
    config.shade_k_per_m2 = 30.0;
    config.umbrellafrond.crown_radius_m = [2.0, 2.0];
    // These two tests predate layers and are about the shade rule's **geometry** and
    // the strictness of its inequality, both of which are stated over one disc. The
    // shading species is therefore given the one-layer, zero-porosity profile the rule
    // was written against; what a *tiered* frond does to the light under it is
    // `tests/layers.rs`'s business, not this fixture's.
    config.umbrellafrond.profile = SpeciesConfig::one_stage(vec![SpeciesConfig::foliage_layer(
        [0.0, 1.0],
        1.0,
        1.0,
        0.0,
    )]);
    config.bloomcrown.profile = SpeciesConfig::one_stage(vec![SpeciesConfig::foliage_layer(
        [0.0, 1.0],
        1.0,
        1.0,
        0.0,
    )]);

    let mut world = plain(16, 12, 0.6);
    let mut flora = Flora::new(config);
    // A level pair, one voxel apart, each inside the other's crown.
    for x in [4i64, 5] {
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
    // And an unequal pair, the same species and the same crown radius, one voxel apart.
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 10,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.6
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 11,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.3
        }
    ));

    let uc = flora.config().species(Species::Umbrellafrond).clone();
    // The level pair covers itself — one voxel apart against a two-voxel radius — so
    // "not shaded" can only be the strictness of the inequality on the tops.
    assert!(
        1.0 <= uc.crown_radius(0.6, 1.0),
        "the level pair must cover each other"
    );
    assert!(
        uc.crown_height(0.3, 1.0) < uc.crown_height(0.6, 1.0),
        "and the unequal pair is not level"
    );

    flora.step(&mut world);
    let v = flora.view();
    assert_eq!(
        v.stand_at(site(4)).unwrap().light,
        1.0,
        "a level crown does not shade"
    );
    assert_eq!(
        v.stand_at(site(5)).unwrap().light,
        1.0,
        "in either direction"
    );
    assert_eq!(
        v.stand_at(site(10)).unwrap().light,
        1.0,
        "nor does a lower crown shade a higher"
    );
    // The lower crown is shaded, by exactly the taller crown's own attenuation.
    let area = (std::f64::consts::PI * uc.crown_radius(0.6, 1.0).powi(2)).max(1.0);
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
            w.apply(WorldCommand::SetMaterial {
                x,
                y: 1,
                z: 0,
                material: Material::Bedrock,
            });
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
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 3,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.1
        }
    ));

    // One voxel's stock, and the whole strip's: four two-voxel columns and one of one.
    let per_voxel = pore * Material::Soil.pore_capacity() * world.config().voxel_volume();
    let stock0 = pore_stock(&world);
    assert!(
        (stock0 - 9.0 * per_voxel).abs() < 1e-12,
        "{stock0} is not nine voxels of soil"
    );

    flora.step(&mut world);

    let b = *flora.view().stand_at(site(1)).expect("the bloomcrown");
    let u = *flora.view().stand_at(site(3)).expect("the umbrellafrond");
    // Two species, two moisture responses over the same water: that is what makes the
    // demands differ and the split observable.
    let bs = flora.config().species(Species::Bloomcrown).clone();
    let us = flora.config().species(Species::Umbrellafrond).clone();
    assert_eq!(
        b.moisture, 1.0,
        "pore 0.6 is past bloomcrown's sat_pore of {}",
        bs.sat_pore
    );
    assert!(
        (u.moisture - 0.6).abs() < 1e-12,
        "umbrellafrond's ramp: {}",
        u.moisture
    );

    let demand_b = bs.transpiration_m3_per_s * (bs.alpha * 0.1) * b.moisture * cubarium_voxel::DT;
    let demand_u = us.transpiration_m3_per_s * (us.alpha * 0.1) * u.moisture * cubarium_voxel::DT;
    assert!(
        demand_b / 5.0 > per_voxel,
        "the fixture has to be starved to test a split"
    );

    // Four voxels are the bloomcrown's alone and four the umbrellafrond's; the fifth is
    // shared, and there the two demands share the stock in their own proportion.
    let shared_b = per_voxel * demand_b / (demand_b + demand_u);
    let shared_u = per_voxel * demand_u / (demand_b + demand_u);
    let want_b = 4.0 * per_voxel + shared_b;
    let want_u = 4.0 * per_voxel + shared_u;
    assert!(
        (b.water_m3 - want_b).abs() < 1e-12 * want_b,
        "{} against {want_b}",
        b.water_m3
    );
    assert!(
        (u.water_m3 - want_u).abs() < 1e-12 * want_u,
        "{} against {want_u}",
        u.water_m3
    );
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
    assert!(
        pore_stock(&world) < 1e-15,
        "every root voxel should be dry: {}",
        pore_stock(&world)
    );
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

/// Two species reaching the same two bare sites. Round 3b's donors save for one recipient
/// at a time, so the claim that survives is the one about **cadence and books**: a package
/// is the same size for both species (`alive_min / w_frac`, 0.05 at the placeholders), so
/// a donor funded at twice the rate sends twice as many of them, each donor is debited
/// only for its own parcels, and one species' package never lands in the other species'
/// cohort.
///
/// `bloomcrown.propagule_rate` is 0.36 /s and `umbrellafrond.propagule_rate` 0.18 /s here
/// (placeholder 2e-4 for both), which is 0.015 and 0.0075 of net parcel a tick — so in 21
/// ticks the first funds 0.315 of parcel and sends **six** packages and the second funds
/// 0.1575 and sends **three**. `reserve_cap` is 4.0 for both (placeholder 0.5) so the
/// reserve is never the binding constraint and the rate is; `bloomcrown.hop` is 1 against
/// the placeholder 2, so both donors reach exactly the same two sites on a four-column
/// ring; both species are frozen (see `frozen`) so a reserve moves only where a propagule
/// debits it. Both `establish_light_min` are 2.0 (placeholders 0.6 and 0.1), a predicate
/// nothing can pass, so the banks accumulate instead of germinating and the books can be
/// read against what landed — nine packages on two sites would otherwise recruit.
#[test]
fn two_species_banks_share_one_site_and_each_donor_pays_only_its_own() {
    let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
    config.bloomcrown.propagule_rate = 0.36;
    config.bloomcrown.hop = 1;
    config.bloomcrown.reserve_cap = 4.0;
    config.bloomcrown.establish_light_min = 2.0;
    config.umbrellafrond.propagule_rate = 0.18;
    config.umbrellafrond.reserve_cap = 4.0;
    config.umbrellafrond.establish_light_min = 2.0;
    frozen(&mut config.bloomcrown);
    frozen(&mut config.umbrellafrond);

    let mut world = plain(4, 8, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.4
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.4
        }
    ));

    let bs = flora.config().species(Species::Bloomcrown).clone();
    let us = flora.config().species(Species::Umbrellafrond).clone();
    let package = bs.alive_min / bs.propagule_split[0];
    assert!(
        (package - us.alive_min / us.propagule_split[0]).abs() < 1e-15,
        "the fixture's premise: one package is the same size for both species"
    );
    let net_b = bs.propagule_rate * cubarium_voxel::DT / (1.0 + bs.build);
    let net_u = us.propagule_rate * cubarium_voxel::DT / (1.0 + us.build);
    let seeded_reserve_b = bs.reserve_cap * 0.4;
    let seeded_reserve_u = us.reserve_cap * 0.4;

    let ticks = 21u32;
    run(&mut flora, &mut world, ticks);

    // Six packages against three: the cadence is the rate, and the ledger says so without
    // anyone having to guess which site a draw picked.
    let l = flora.view().ledger;
    let (ib, iu) = (Species::Bloomcrown.index(), Species::Umbrellafrond.index());
    assert!(
        (l.propagule_landed[ib] - 6.0 * package).abs() <= 1e-15,
        "bloomcrown landed {} of six {package} packages",
        l.propagule_landed[ib]
    );
    assert!(
        (l.propagule_landed[iu] - 3.0 * package).abs() <= 1e-15,
        "umbrellafrond landed {} of three {package} packages",
        l.propagule_landed[iu]
    );
    // Each donor asked for, was funded, and is holding exactly its own arithmetic.
    for (species, net, landed, seeded) in [
        (Species::Bloomcrown, net_b, 6.0 * package, seeded_reserve_b),
        (
            Species::Umbrellafrond,
            net_u,
            3.0 * package,
            seeded_reserve_u,
        ),
    ] {
        let i = species.index();
        let accrued = ticks as f64 * net;
        assert!(
            (l.propagule_requested[i] - accrued).abs() <= 1e-15,
            "{} requested {}",
            species.name(),
            l.propagule_requested[i]
        );
        assert!(
            (l.propagule_funded[i] - accrued).abs() <= 1e-15,
            "{} was funded {} of {accrued} it asked for",
            species.name(),
            l.propagule_funded[i]
        );
        let x = if species == Species::Bloomcrown { 2 } else { 0 };
        let donor = *flora.view().stand_at(site(x)).expect("the donor");
        assert_eq!(donor.wood, 0.4, "the frozen donor's wood moved");
        assert!(
            (donor.parcel - (accrued - landed)).abs() <= 1e-15,
            "{}'s parcel holds {} of {accrued} funded less {landed} landed",
            species.name(),
            donor.parcel
        );
        // The debit is the gross of everything it saved, sent or not: `(1 + c_g)` times.
        let build = 1.0 + flora.config().species(species).build;
        let spent = seeded - donor.reserve;
        assert!(
            (spent - build * accrued).abs() <= 1e-12 * spent,
            "{} spent {spent} of reserve for {accrued} of parcel",
            species.name()
        );
    }

    // Nine packages over the two sites both donors reach, and nowhere else. Where a site
    // holds both species they are two cohorts, bloomcrown first, and neither donor's
    // material is in the other's.
    let mut shared = 0;
    for g in flora.view().ground.iter().filter(|g| !g.seeds.is_empty()) {
        assert!(
            [site(1), site(3)].contains(&g.site),
            "a package landed outside both donors' hop: {:?}",
            g.site
        );
        let species: Vec<Species> = g.seeds.iter().map(|c| c.species).collect();
        let mut sorted = species.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            species, sorted,
            "not one cohort per species, sorted: {:?}",
            g.seeds
        );
        if species.len() == 2 {
            shared += 1;
        }
    }
    assert!(
        shared >= 1,
        "the two species never shared a site in {ticks} ticks"
    );
    // And what is banked is what landed, less the attrition each bin has paid since.
    for (species, landed) in [
        (Species::Bloomcrown, 6.0 * package),
        (Species::Umbrellafrond, 3.0 * package),
    ] {
        let banked: f64 = flora
            .view()
            .ground
            .iter()
            .map(|g| g.seed_organic(species))
            .sum();
        assert!(
            banked > 0.99 * landed && banked <= landed,
            "{} banked {banked} of {landed} landed",
            species.name()
        );
    }
    assert_residuals(&flora, "after two species banked on one site");
}

/// A site outside the donor's `hop` never receives, and the patch does not creep. Two arms,
/// because round 3b's rule changed what "forty ticks at the placeholder rate" means:
///
///   - at the **placeholder** `propagule_rate` of 2e-4 /s, forty ticks fund 3.33e-4 of
///     parcel, which is 0.67 % of one 0.05 package, so **nothing lands at all**. That is
///     Astra's R4.4 arithmetic from the other end: one package is 300 s of a donor's whole
///     funded reproductive output, and the old rule's forty-tick cohorts were the same
///     material spread over four sites at once and counted four times.
///   - at 3.0 /s with `reserve_cap` 4.0 (placeholder 0.5), the donor sends a package every
///     tick, and every one of them lands on one of the four columns within `hop` 2 —
///     never outside, never its own site, and never a stand.
#[test]
fn nothing_outside_hop_ever_receives_and_the_patch_does_not_creep() {
    let mut world = plain(16, 8, 0.6);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 7,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.4
        }
    ));
    let sc = flora.config().species(Species::Bloomcrown).clone();
    assert_eq!(sc.hop, 2, "the placeholder hop this test reads");
    let package = sc.alive_min / sc.propagule_split[0];
    let net = sc.propagule_rate * cubarium_voxel::DT / (1.0 + sc.build);

    run(&mut flora, &mut world, 40);

    let donor = *flora.view().stand_at(site(7)).expect("the donor");
    assert!(
        (donor.parcel - 40.0 * net).abs() <= 1e-15,
        "the parcel holds {} after forty ticks of {net}",
        donor.parcel
    );
    assert!(
        donor.parcel < 0.01 * package,
        "forty placeholder ticks is {} of a {package} package",
        donor.parcel / package
    );
    assert!(
        flora.view().ground.iter().all(|g| g.seeds.is_empty()),
        "something landed on a parcel that is not a package: {:?}",
        flora.view().ground
    );
    assert_eq!(
        flora.view().ledger.propagule_landed[Species::Bloomcrown.index()],
        0.0
    );
    assert_eq!(flora.view().ledger.establishments, 0);
    assert_residuals(&flora, "after forty ticks of one placeholder donor");

    // The second arm: a funded donor, whose packages stay inside its hop.
    let mut config = FloraConfig::default();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.reserve_cap = 4.0;
    let mut world = plain(16, 8, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 7,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.4
        }
    ));

    run(&mut flora, &mut world, 40);

    let banked: Vec<u32> = flora
        .view()
        .ground
        .iter()
        .filter(|g| !g.seeds.is_empty())
        .map(|g| g.site.x)
        .collect();
    assert!(!banked.is_empty(), "a funded donor landed nothing");
    assert!(
        banked.iter().all(|x| [5u32, 6, 8, 9].contains(x)),
        "a package landed outside the donor's hop, or on its own site: {banked:?}"
    );
    // Forty packages over four sites is about ten each, so this arm does germinate — and
    // the patch still does not creep: every stand is the donor or one of its own hop
    // neighbours, and the newborns are far under `donor_min` so none of them can spread
    // further.
    let stands: Vec<u32> = flora.view().stands.iter().map(|s| s.site.x).collect();
    assert!(
        stands.iter().all(|x| [5u32, 6, 7, 8, 9].contains(x)),
        "a stand appeared outside the donor's hop: {stands:?}"
    );
    assert!(stands.contains(&7), "the donor died: {stands:?}");
    let donor_min = flora.config().species(Species::Bloomcrown).donor_min;
    for s in flora.view().stands.iter().filter(|s| s.site.x != 7) {
        assert!(s.wood < donor_min, "a newborn can already donate: {s:?}");
    }
    assert!(
        flora.view().ledger.propagule_landed[Species::Bloomcrown.index()] > 0.0,
        "the funded arm landed nothing either"
    );
    assert_residuals(&flora, "after forty ticks of one funded donor");
}

/// The donor's side of §4.8 under round 3b: what left the donor's reserve is exactly what
/// it saved, `1 + c_g` times over; what it saved is either standing in its parcel or has
/// landed as a package; and the three reproductive fluxes in the ledger say which.
/// Nothing is created by a propagule and nothing is lost in one.
///
/// Bloomcrown is frozen here (see `frozen`) so that the reserve's whole change over the
/// run is the debit; with income on, growth moves the same stock and the debit cannot be
/// read off it. `propagule_rate` is 0.18 /s (placeholder 2e-4), which is 0.0075 of parcel
/// a tick, so the first package is full on the seventh tick and this ten-tick run has one
/// package away and 0.025 still saving. `establish_light_min` is 2.0 (placeholder 0.6), a
/// predicate nothing can pass: since K7 a single package on a *passing* site is a recruit
/// on the next step, and this test is about the donor's books, so the package has to stay
/// in the bank to be read.
#[test]
fn a_donor_is_debited_exactly_what_arrives_plus_its_construction() {
    let mut config = FloraConfig::default();
    config.bloomcrown.propagule_rate = 0.18;
    config.bloomcrown.establish_light_min = 2.0;
    frozen(&mut config.bloomcrown);
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

    let sc = flora.config().species(Species::Bloomcrown).clone();
    let reserve0 = sc.reserve_cap * 0.4;
    assert_eq!(flora.view().stand_at(site(3)).unwrap().reserve, reserve0);
    let heat0 = flora.view().ledger.heat_out;
    let respired0 = flora.view().ledger.respired_out;
    let build = 1.0 + sc.build;
    let net = sc.propagule_rate * cubarium_voxel::DT / build;
    let package = sc.alive_min / sc.propagule_split[0];

    let ticks = 10u32;
    run(&mut flora, &mut world, ticks);

    let accrued = ticks as f64 * net;
    let donor = *flora.view().stand_at(site(3)).expect("the donor");
    let spent = reserve0 - donor.reserve;
    assert!(
        (spent - build * accrued).abs() <= 1e-15,
        "the donor spent {spent} of reserve for {accrued} of parcel"
    );

    // One package away, on one site, and the rest of what it saved is in the parcel.
    let mut arrived = 0.0;
    let mut landed_on = 0;
    for g in flora.view().ground {
        if g.seeds.is_empty() {
            continue;
        }
        assert!(
            [site(1), site(2), site(4), site(5)].contains(&g.site),
            "a package landed outside the donor's hop 2: {:?}",
            g.site
        );
        landed_on += 1;
        assert_eq!(g.seeds.len(), 1, "{:?}", g.seeds);
        assert_eq!(g.seeds[0].species, Species::Bloomcrown);
        arrived += g.seeds[0].organic;
        // Round 3: the construction respiration is *not* deposited on the recipient. It
        // leaves the system as organic matter, so the site's pool is still its starting
        // mineral — to 1e-11, which is the cohort's own attrition decomposing in the three
        // ticks since it landed, and not the 0.01 of construction that was respired.
        let pool = g.mineral - flora.config().initial_mineral;
        assert!(
            pool >= 0.0 && pool < 1e-10,
            "construction fertilized the site by {pool}"
        );
        assert!(
            flora.view().stand_at(g.site).is_none(),
            "a package is a cohort, not a stand"
        );
    }
    assert_eq!(landed_on, 1, "one package, one recipient");
    // What is there is the package less the attrition of the three ticks since it landed:
    // 0.001/s of it a second, so 0.015 % over three ticks.
    let kept = 1.0 - 3.0 * sc.seed_attrition_per_s * cubarium_voxel::DT;
    assert!(
        arrived <= package && arrived > kept * package,
        "{arrived} arrived for a {package} package, {} expected after three ticks of attrition",
        kept * package
    );
    assert!(
        (donor.parcel - (accrued - package)).abs() <= 1e-15,
        "the parcel holds {} of {accrued} saved less one {package} package",
        donor.parcel
    );

    // What left the donor is what it saved plus what the build respired, and the respired
    // half is a named boundary flow rather than a stock somewhere.
    // To 1e-7 and not to the bit: the landed package's attrition is litter on its site,
    // and the site's litter decomposes, which is the other thing in `respired_out` here.
    let respired = flora.view().ledger.respired_out - respired0;
    assert!(
        (respired - sc.build * accrued).abs() <= 1e-7 * respired,
        "construction respired {respired} for {accrued} saved"
    );
    assert!(
        (arrived + donor.parcel + respired - spent).abs() <= 1e-4 * spent,
        "{arrived} arrived, {} is saving and {respired} respired for {spent} spent (the \
         difference is the landed package's own attrition, which is litter on its site)",
        donor.parcel
    );
    // The construction respiration is booked as heat too, at the species' own density.
    let heat = flora.view().ledger.heat_out - heat0;
    assert!(
        (heat - sc.energy_density * sc.build * accrued).abs() <= 1e-7 * heat,
        "construction heat {heat} for {accrued} saved"
    );
    // The three fluxes: an unlimited donor is funded everything it asks for, and what it
    // has not landed is exactly what it is holding.
    let l = flora.view().ledger;
    let i = Species::Bloomcrown.index();
    assert!(
        (l.propagule_requested[i] - accrued).abs() <= 1e-15,
        "{:?}",
        l.propagule_requested
    );
    assert!(
        (l.propagule_funded[i] - accrued).abs() <= 1e-15,
        "{:?}",
        l.propagule_funded
    );
    assert!(
        (l.propagule_landed[i] - package).abs() <= 1e-15,
        "{:?}",
        l.propagule_landed
    );
    assert!(
        (l.propagule_funded[i] - l.propagule_landed[i] - donor.parcel).abs() <= 1e-15,
        "funded minus landed is the parcel: {:?} against {}",
        l,
        donor.parcel
    );
    assert_residuals(&flora, "after ten ticks of one saving donor");
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
    let before: Vec<f64> = (0..16i64)
        .map(|x| world.view().sky_visibility(x, 2, 0))
        .collect();
    let version = world.terrain_version();
    assert!(
        before.iter().all(|&v| v == 1.0),
        "the fixture is an open plain: {before:?}"
    );

    // Package L: crowns are metres; this 1 m-cell fixture keeps the crowns it was
    // written against, four times the 0.25 m-cell ladder (`FloraConfig::crowns_scaled`).
    let mut flora = Flora::new(FloraConfig::default().crowns_scaled(4.0));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 8,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.6
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 9,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    run(&mut flora, &mut world, 5);

    let after: Vec<f64> = (0..16i64)
        .map(|x| world.view().sky_visibility(x, 2, 0))
        .collect();
    assert_eq!(
        before, after,
        "a canopy is not terrain and may not change its geometry"
    );
    assert_eq!(
        world.terrain_version(),
        version,
        "a plant committed a material change"
    );

    // And the canopy is doing something — in the plant layer's number, at the default
    // `shade_k`, with no rate touched.
    let shaded = flora
        .view()
        .stand_at(site(9))
        .expect("under the crown")
        .light;
    assert!(
        shaded < after[9],
        "the covered stand's light {shaded} is not below its site's sky visibility {}",
        after[9]
    );
    assert_eq!(
        flora.view().stand_at(site(8)).unwrap().light,
        1.0,
        "the taller stand is unshaded"
    );
    assert_residuals(&flora, "after five ticks under a canopy");
}
