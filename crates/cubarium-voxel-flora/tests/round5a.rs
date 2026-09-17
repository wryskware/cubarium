//! Round 5a: the **bounded food transfers** a consumer layer will run on — three
//! withdrawals, one deposit with its new carrion pool, and the reach geometry — each as a
//! short function test of the boundary it states, plus the producer response the existing
//! growth rules already give.
//!
//! No consumer exists: every test here is the plant layer answering a caller that takes
//! material and hands material back. Nothing about populations, viability or carrying
//! capacity is asserted anywhere in this file, because the model says nothing about them.
//!
//! Conventions are `round3.rs`'s and `round4.rs`'s, so a fixture here reads the same way:
//! `voxel_m` is 1 m, so a soil voxel holds `Material::Soil.pore_capacity()` = 0.35 m³ of
//! pore water; soil is wetted by adding free water to an air cell and turning the cell to
//! soil; and the world is **never stepped**, so the only thing that moves water is the
//! plant layer's own bounded withdrawal and a fixture's pore fraction is the condition it
//! says it is.
//!
//! Where a test needs a rate the placeholders do not give, it sets that rate in its **own**
//! config and says why. Three recur:
//!
//! - `maintenance` 1,000 /s on one species, so that a founder diebacks its whole trunk and
//!   dies inside a single tick and both dead pools are filled with something whose mineral
//!   and energy are known. What is being tested is the withdrawal, not how long a stand
//!   takes to starve.
//! - `carrion_decomposition` 5.0 /s against the placeholder, so that a deposit's
//!   decomposition is readable in twenty ticks instead of thousands.
//! - `decomposition` 0, so that what reaches the litter stays there and can be read off.
//!
//! None of those is read back as a placeholder.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Deposit, DepositKind, Flora, FloraConfig, Reach, Site, Species, SpeciesConfig, Taken,
};

// ------------------------------------------------------------------- fixtures

/// Turn one air voxel into `material` holding exactly `pore` of that material's own pore
/// capacity, by adding the water first and converting after. `round3.rs`'s `fill`.
fn fill(w: &mut World, x: i64, y: u32, z: u32, material: Material, pore: f64) {
    let want = pore * material.pore_capacity() * w.config().voxel_volume();
    if want > 0.0 {
        let got = w.apply(WorldCommand::AddWater { x, y, z, volume_m3: want });
        assert!((got - want).abs() < 1e-12, "the void took {got} of {want}");
    }
    w.apply(WorldCommand::SetMaterial { x, y, z, material });
    assert!(
        (w.view().pore_at(x, y, z) - pore).abs() < 1e-12,
        "pore {} at ({x},{y},{z})",
        w.view().pore_at(x, y, z)
    );
}

fn empty_world(width: u32, depth: u32) -> World {
    World::empty(VoxelConfig {
        width,
        height: 10,
        depth,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    })
}

/// Take a column's bedrock away, so it holds nothing solid and therefore no support face.
fn void_column(w: &mut World, x: i64, z: u32) {
    w.apply(WorldCommand::SetMaterial { x, y: 0, z, material: Material::Air });
    assert!(
        cubarium_voxel_flora::highest_support(&w.view(), x, z).is_none(),
        "({x},{z}) must be void"
    );
}

/// `round4.rs`'s strip: only the columns of `keep` are solid at all — bedrock at `y = 0`,
/// soil at `y = 1..=2` holding `pore` of soil's capacity — so every kept column's support
/// face is `y = 2` in open sky and every other column has none.
fn pillars(width: u32, keep: &[i64], pore: f64) -> World {
    let mut w = empty_world(width, 1);
    for x in 0..width as i64 {
        if keep.contains(&x) {
            for y in 1..=2 {
                fill(&mut w, x, y, 0, Material::Soil, pore);
            }
        } else {
            void_column(&mut w, x, 0);
        }
    }
    w
}

/// The support face of a kept column of [`pillars`].
fn at(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

fn run(flora: &mut Flora, world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        flora.step(world);
    }
}

/// The three residuals, as `round3.rs` and `round4.rs` compute them — now including the
/// `consumed_*` and `deposited_*` boundary flows, since they are in the ledger's own
/// expected totals.
fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    let (o, n, e) = (
        v.organic() - v.ledger.expected_organic(),
        v.mineral() - v.ledger.expected_mineral(),
        v.energy() - v.ledger.expected_energy(),
    );
    assert!(o.abs() <= 1e-9 * v.organic().abs().max(1.0), "{when}: organic residual {o}");
    assert!(n.abs() <= 1e-9 * v.mineral().abs().max(1.0), "{when}: mineral residual {n}");
    assert!(e.abs() <= 1e-9 * v.energy().abs().max(1.0), "{when}: energy residual {e}");
}

fn species_mut(config: &mut FloraConfig, species: Species) -> &mut SpeciesConfig {
    match species {
        Species::Bloomcrown => &mut config.bloomcrown,
        Species::Umbrellafrond => &mut config.umbrellafrond,
        Species::Springturf => &mut config.springturf,
        Species::Stonecushion => &mut config.stonecushion,
        Species::Velvetpad => &mut config.velvetpad,
    }
}

/// Every `consumed_*` term of the ledger, so a test can assert the boundary flows against
/// the [`Taken`]s it was handed.
fn consumed(flora: &Flora) -> (f64, f64, f64) {
    let l = flora.view().ledger;
    (l.consumed_organic_out, l.consumed_mineral_out, l.consumed_energy_out)
}

/// A founder of `species` planted on a [`pillars`] column, at half its own `wood_max` —
/// the harness's own founder size, and exactly `donor_min` for all five presets.
fn plant(flora: &mut Flora, world: &World, x: i64, species: Species) -> Site {
    let wood = 0.5 * flora.config().species(species).wood_max;
    assert!(
        flora.apply(world, Command::Seed { x, z: 0, species, wood }),
        "{} could not be planted on column {x}",
        species.name()
    );
    at(x as u32)
}

// ------------------------------------------------------------- 1. withdrawals

/// Asking for more foliage than a stand holds takes **exactly its foliage** and nothing
/// else: the wood and the reserve do not move, the stand's mineral falls by the fraction
/// rule — the foliage's share of the whole material, parcel included — and the ledger's
/// three `consumed_*` terms are the `Taken` that came back, to the bit.
#[test]
fn taking_more_than_a_stand_holds_takes_its_foliage_and_leaves_the_wood_and_the_reserve() {
    let world = pillars(8, &[0], 0.6);
    let mut flora = Flora::new(FloraConfig::default());
    let site = plant(&mut flora, &world, 0, Species::Springturf);

    let before = *flora.view().stand_at(site).expect("just planted");
    assert!(before.foliage > 0.0 && before.reserve > 0.0 && before.mineral > 0.0);
    let share = before.foliage / before.material();
    let e_v = flora.config().species(Species::Springturf).energy_density;

    let taken = flora.take_foliage(site, 10.0 * before.foliage).expect("a stand with foliage");
    let after = *flora.view().stand_at(site).expect("a stripped stand is still standing");

    assert_eq!(taken.organic, before.foliage, "it took {} of {}", taken.organic, before.foliage);
    assert_eq!(after.foliage, 0.0, "{} of foliage left", after.foliage);
    assert_eq!(after.wood, before.wood, "the wood moved");
    assert_eq!(after.reserve, before.reserve, "the reserve moved");
    let want_mineral = before.mineral * share;
    assert!(
        (taken.mineral - want_mineral).abs() <= 1e-18,
        "mineral {} against the fraction rule's {want_mineral}",
        taken.mineral
    );
    assert!(
        (after.mineral - (before.mineral - taken.mineral)).abs() <= 1e-18,
        "the stand kept {} of {}",
        after.mineral,
        before.mineral - taken.mineral
    );
    assert_eq!(taken.energy, e_v * taken.organic, "energy at the tissue's own density");

    assert_eq!(consumed(&flora), (taken.organic, taken.mineral, taken.energy));
    assert_residuals(&flora, "after one foliage withdrawal");
}

/// A withdrawal with nothing to withdraw returns `None` and books **nothing**: a site with
/// no stand, a site that has no ground entry at all, and a stand whose foliage has already
/// been eaten.
#[test]
fn a_withdrawal_with_nothing_to_take_returns_none_and_books_nothing() {
    let world = pillars(8, &[0, 2], 0.6);
    let mut flora = Flora::new(FloraConfig::default());
    let site = plant(&mut flora, &world, 0, Species::Springturf);
    let bare = at(2);

    // A bare support face: no stand, and no ground entry either, so all three refuse.
    assert!(flora.view().stand_at(bare).is_none());
    assert!(flora.view().ground_at(bare).is_none());
    assert_eq!(flora.take_foliage(bare, 1.0), None, "a site with no stand");
    assert_eq!(flora.take_dead_wood(bare, 1.0), None, "a site with no ground");
    assert_eq!(flora.take_litter(bare, 1.0), None, "a site with no ground");
    // The founder's own site has a ground entry — a mineral pool — and two empty dead
    // pools, which is the other way of having nothing to give.
    assert!(flora.view().ground_at(site).is_some_and(|g| g.mineral > 0.0));
    assert_eq!(flora.take_dead_wood(site, 1.0), None, "an empty dead-wood pool");
    assert_eq!(flora.take_litter(site, 1.0), None, "an empty litter pool");
    // And a want that is not a positive finite number takes nothing from a full stand.
    for want in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(flora.take_foliage(site, want), None, "want {want}");
    }
    assert_eq!(consumed(&flora), (0.0, 0.0, 0.0), "nothing may be booked");

    // Now strip it, and ask again: an empty `P` is `None` too, and the second ask books
    // nothing on top of the first.
    let first = flora.take_foliage(site, 1.0).expect("a stand with foliage");
    assert_eq!(flora.take_foliage(site, 1.0), None, "a stand with no foliage left");
    assert_eq!(consumed(&flora), (first.organic, first.mineral, first.energy));
    assert_residuals(&flora, "after a refused withdrawal");
}

/// Dead-wood and litter withdrawals carry their mineral and their retained energy **pro
/// rata**, at the stock's own current density, so the density of what is left does not
/// move — and a withdrawal that empties a pool takes every unit of both.
#[test]
fn dead_wood_and_litter_withdrawals_carry_their_pro_rata_mineral_and_energy() {
    let world = pillars(8, &[0], 0.6);
    // A springturf that diebacks its whole trunk and dies in one tick, so that both dead
    // pools hold something with a known mineral and energy content, and litter that stays
    // where it fell.
    let mut config = FloraConfig::default();
    config.decomposition = 0.0;
    config.wood_decomposition = 0.0;
    species_mut(&mut config, Species::Springturf).maintenance = 1000.0;
    let mut flora = Flora::new(config);
    let site = plant(&mut flora, &world, 0, Species::Springturf);
    run(&mut flora, &mut { world }, 1);

    assert!(flora.view().stand_at(site).is_none(), "the fixture's premise: the stand died");
    let g0 = flora.view().ground_at(site).expect("its own ground").clone();
    assert!(g0.dead_wood > 0.0 && g0.dead_wood_mineral > 0.0 && g0.dead_wood_energy > 0.0);
    assert!(g0.litter > 0.0 && g0.litter_mineral > 0.0 && g0.litter_energy > 0.0);

    // Half the dead wood: half its mineral and half its energy, and the same densities
    // afterwards.
    let half = flora.take_dead_wood(site, g0.dead_wood / 2.0).expect("a full pool");
    let g1 = flora.view().ground_at(site).expect("its own ground").clone();
    assert_eq!(half.organic, g0.dead_wood / 2.0);
    assert!((half.mineral - g0.dead_wood_mineral / 2.0).abs() <= 1e-18, "{half:?}");
    assert!((half.energy - g0.dead_wood_energy / 2.0).abs() <= 1e-18, "{half:?}");
    assert!((g1.dead_wood - g0.dead_wood / 2.0).abs() <= 1e-18);
    assert!(
        (g1.dead_wood_mineral / g1.dead_wood - g0.dead_wood_mineral / g0.dead_wood).abs() <= 1e-15,
        "the mineral density of what is left moved"
    );
    assert!(
        (g1.dead_wood_energy / g1.dead_wood - g0.dead_wood_energy / g0.dead_wood).abs() <= 1e-15,
        "the energy density of what is left moved"
    );

    // More litter than the pool holds: the pool is emptied exactly, mineral and energy
    // included, with no float dust left claiming to be a stock.
    let all = flora.take_litter(site, 10.0 * g0.litter).expect("a full pool");
    let g2 = flora.view().ground_at(site).expect("its own ground").clone();
    assert_eq!(all.organic, g0.litter);
    assert_eq!(all.mineral, g0.litter_mineral);
    assert_eq!(all.energy, g0.litter_energy);
    assert_eq!((g2.litter, g2.litter_mineral, g2.litter_energy), (0.0, 0.0, 0.0));

    let (o, n, e) = consumed(&flora);
    assert!((o - (half.organic + all.organic)).abs() <= 1e-18, "consumed organic {o}");
    assert!((n - (half.mineral + all.mineral)).abs() <= 1e-18, "consumed mineral {n}");
    assert!((e - (half.energy + all.energy)).abs() <= 1e-18, "consumed energy {e}");
    assert_residuals(&flora, "after two dead-pool withdrawals");
}

// ---------------------------------------------------------------- 2. deposits

/// A **carrion** deposit conserves its mineral exactly on the way to the site's pool and
/// respires its organic matter out of the system, which is litter's rule on a pool of its
/// own and at a rate of its own.
///
/// The site has no stand, so nothing draws on the pool while the corpse decomposes and the
/// mineral can be read straight off the ground: what leaves `carrion_mineral` arrives in
/// `mineral`, to the last bit, however many ticks it takes.
#[test]
fn a_carrion_deposit_conserves_its_mineral_into_the_site_pool_and_respires_its_organic_matter() {
    let mut world = pillars(8, &[0], 0.6);
    // Fast enough to read in twenty ticks, and litter decomposition off so that only the
    // carrion can move anything.
    let mut config = FloraConfig::default();
    config.decomposition = 0.0;
    config.wood_decomposition = 0.0;
    config.carrion_decomposition = 5.0;
    let mut flora = Flora::new(config);
    let site = at(0);

    // The site is bare: the deposit is what provisions its ground, and the lazy
    // `initial_mineral` is booked as `seeded_mineral_in` exactly as it is for a founder.
    assert!(flora.view().ground_at(site).is_none());
    let body = Deposit { kind: DepositKind::Carrion, organic: 0.4, mineral: 0.012, energy: 0.9 };
    assert!(flora.deposit(site, body), "the deposit was refused");
    let g0 = flora.view().ground_at(site).expect("the deposit provisions a ground").clone();
    assert_eq!((g0.carrion, g0.carrion_mineral, g0.carrion_energy), (0.4, 0.012, 0.9));
    assert_eq!(g0.mineral, flora.config().initial_mineral, "a new site's own pool");
    let l0 = flora.view().ledger.clone();
    assert_eq!(
        (l0.deposited_organic_in, l0.deposited_mineral_in, l0.deposited_energy_in),
        (0.4, 0.012, 0.9)
    );
    assert_eq!(l0.seeded_mineral_in, flora.config().initial_mineral, "provisioning is seeded");
    assert_residuals(&flora, "after a carrion deposit");

    // The mineral in the ground is conserved to the bit: the pool is the only place the
    // carrion's mineral can go, and nothing on this site draws on it.
    let total_mineral = g0.mineral + g0.carrion_mineral;
    run(&mut flora, &mut world, 20);
    let g1 = flora.view().ground_at(site).expect("its own ground").clone();
    let l1 = flora.view().ledger.clone();
    assert!(g1.carrion < g0.carrion, "the corpse did not decompose at all");
    assert!(g1.mineral > g0.mineral, "no mineral reached the pool");
    assert!(
        (g1.mineral + g1.carrion_mineral - total_mineral).abs() <= 1e-18,
        "mineral {} + {} is not the {total_mineral} that was there",
        g1.mineral,
        g1.carrion_mineral
    );
    // Organic matter respired, energy to heat, and the densities of what is left unmoved.
    assert!(
        (l1.respired_out - l0.respired_out - (g0.carrion - g1.carrion)).abs() <= 1e-15,
        "respired {} for {} of carrion gone",
        l1.respired_out - l0.respired_out,
        g0.carrion - g1.carrion
    );
    assert!(l1.heat_out > l0.heat_out, "no energy left as heat");
    assert!(
        (g1.carrion_mineral / g1.carrion - g0.carrion_mineral / g0.carrion).abs() <= 1e-15,
        "the mineral density of the remains moved"
    );
    assert!(
        (g1.carrion_energy / g1.carrion - g0.carrion_energy / g0.carrion).abs() <= 1e-15,
        "the energy density of the remains moved"
    );
    assert_eq!(l1.deposited_mineral_in, l0.deposited_mineral_in, "decomposition is not a deposit");
    assert_residuals(&flora, "after a corpse decomposed");
}

/// A **litter** deposit joins `Ground::litter` through the existing `e_d_max` cap: the
/// organic matter and the mineral go in whole, the energy only as far as the cap allows,
/// and the refused energy leaves as heat at once — which is what keeps the energy residual
/// closed over a deposit the pool cannot hold.
#[test]
fn a_litter_deposit_obeys_the_energy_cap_and_the_rest_leaves_as_heat() {
    let mut config = FloraConfig::default();
    config.decomposition = 0.0;
    let cap = config.litter_energy_cap;
    let mut flora = Flora::new(config);
    let site = at(0);

    // Twice as much energy as `e_d_max · D` can hold.
    let dung =
        Deposit { kind: DepositKind::Litter, organic: 0.2, mineral: 0.004, energy: 2.0 * cap * 0.2 };
    assert!(flora.deposit(site, dung));
    let g = flora.view().ground_at(site).expect("provisioned").clone();
    let l = flora.view().ledger.clone();
    assert_eq!(g.litter, 0.2);
    assert_eq!(g.litter_mineral, 0.004, "mineral is never capped: it has nowhere else to be");
    assert!((g.litter_energy - cap * 0.2).abs() <= 1e-18, "litter kept {}", g.litter_energy);
    assert!(
        (l.heat_out - cap * 0.2).abs() <= 1e-18,
        "the refused half did not leave as heat: {}",
        l.heat_out
    );
    assert_eq!(l.deposited_energy_in, 2.0 * cap * 0.2, "all of it was booked in");
    assert_eq!(g.carrion, 0.0, "a litter deposit is not carrion");
    assert_residuals(&flora, "after a capped litter deposit");
}

/// A deposit of nothing, and a deposit of nonsense, are refused and book nothing — in
/// particular they do not provision a ground and import its `initial_mineral`.
#[test]
fn a_deposit_of_nothing_or_of_nonsense_is_refused_and_books_nothing() {
    // No world at all: `deposit` reads none, which is the other half of its contract —
    // the site is a place in the layer's own ground, and an unsupported one is booked out
    // by step 1 of the next tick like any other.
    let mut flora = Flora::new(FloraConfig::default());
    let site = at(0);

    for bad in [
        Deposit { kind: DepositKind::Carrion, organic: 0.0, mineral: 0.0, energy: 0.0 },
        Deposit { kind: DepositKind::Litter, organic: 0.0, mineral: 0.0, energy: 0.0 },
        Deposit { kind: DepositKind::Carrion, organic: -1.0, mineral: 0.0, energy: 0.0 },
        Deposit { kind: DepositKind::Carrion, organic: f64::NAN, mineral: 0.0, energy: 0.0 },
        Deposit { kind: DepositKind::Litter, organic: 1.0, mineral: f64::INFINITY, energy: 0.0 },
    ] {
        assert!(!flora.deposit(site, bad), "{bad:?} was accepted");
    }
    assert!(flora.view().ground_at(site).is_none(), "a refused deposit provisioned a ground");
    let l = flora.view().ledger.clone();
    assert_eq!(
        (l.deposited_organic_in, l.deposited_mineral_in, l.deposited_energy_in, l.seeded_mineral_in),
        (0.0, 0.0, 0.0, 0.0)
    );
    assert_residuals(&flora, "after five refused deposits");
}

/// The phase order, as one fixture: a deposit made between ticks is in the **next** tick's
/// decomposition snapshot, and one made during the inter-tick after that is in the one
/// after. Nothing decomposes on the tick it was deposited, because the snapshot is taken
/// before anything moves.
#[test]
fn a_deposit_decomposes_from_the_tick_after_the_inter_tick_it_arrived_in() {
    let mut world = pillars(8, &[0], 0.6);
    let mut config = FloraConfig::default();
    config.decomposition = 0.0;
    config.wood_decomposition = 0.0;
    // Half the pool per tick, so one tick of eligibility is unmistakable.
    config.carrion_decomposition = 10.0;
    let mut flora = Flora::new(config);
    let site = at(0);

    assert!(flora.deposit(
        site,
        Deposit { kind: DepositKind::Carrion, organic: 1.0, mineral: 0.02, energy: 2.0 }
    ));
    let before = flora.view().ground_at(site).expect("provisioned").carrion;
    assert_eq!(before, 1.0, "the deposit is whole before any tick runs");

    flora.step(&mut world);
    let after_one = flora.view().ground_at(site).expect("its own ground").carrion;
    assert!(
        (after_one - 0.5).abs() <= 1e-15,
        "one tick of a 10 /s rate on a 1.0 snapshot should leave 0.5, not {after_one}"
    );
    assert_residuals(&flora, "one tick after a deposit");
}

// ------------------------------------------------------------------- 3. reach

/// Columns of chosen heights and nothing else: for each `(x, top)`, bedrock at `y = 0` and
/// soil at `y = 1..=top`, so that column's support face is `y = top`; every other column is
/// void. These fixtures are about geometry — nothing is stepped, so no stand drinks, grows
/// or dies inside them.
fn ledges(width: u32, columns: &[(i64, u32)]) -> World {
    let mut w = empty_world(width, 1);
    for x in 0..width as i64 {
        match columns.iter().find(|(cx, _)| *cx == x) {
            Some(&(_, top)) => {
                for y in 1..=top {
                    fill(&mut w, x, y, 0, Material::Soil, 0.6);
                }
                assert_eq!(
                    cubarium_voxel_flora::highest_support(&w.view(), x, 0),
                    Some(Site { x: x as u32, y: top, z: 0 }),
                    "column {x} must stand at {top}"
                );
            }
            None => void_column(&mut w, x, 0),
        }
    }
    w
}

/// Plant `species` at exactly `wood` on a column, and return the face it stands on.
fn plant_wood(flora: &mut Flora, world: &World, x: i64, species: Species, wood: f64) -> Site {
    assert!(
        flora.apply(world, Command::Seed { x, z: 0, species, wood }),
        "{} could not be planted on column {x}",
        species.name()
    );
    cubarium_voxel_flora::highest_support(&world.view(), x, 0).expect("a support face")
}

/// The reach box, as three cases on one fixture: a crown one voxel up is in reach of an
/// `up: 1` eater, the same crown **two** voxels up is not, and one across the world's `x`
/// seam is one step away. The results come back sorted by site.
#[test]
fn a_crown_two_voxels_up_is_out_of_reach_and_one_across_the_seam_is_not() {
    // Eight columns: the eater's own face at `y = 2`, a neighbour at `y = 2`, one raised to
    // `y = 3`, one four columns away, and the column on the other side of the seam.
    let world = ledges(8, &[(0, 2), (1, 2), (2, 3), (4, 2), (7, 2)]);
    let mut flora = Flora::new(FloraConfig::default());
    let turf = 0.5 * flora.config().springturf.wood_max;
    // A springturf crown at this wood is one cell, one voxel above its own face.
    assert_eq!(flora.config().springturf.crown_voxels(turf), 1);
    assert!(flora.config().springturf.crown_radius(turf) < 1.0, "one cell wide");

    let near = plant_wood(&mut flora, &world, 1, Species::Springturf, turf);
    let high = plant_wood(&mut flora, &world, 2, Species::Springturf, turf);
    let far = plant_wood(&mut flora, &world, 4, Species::Springturf, turf);
    let seam = plant_wood(&mut flora, &world, 7, Species::Springturf, turf);
    assert_eq!((near.y, high.y, far.y, seam.y), (2, 3, 2, 2), "the fixture's own heights");

    let from = Site { x: 0, y: 2, z: 0 };
    let reach = Reach { horizontal: 2, up: 1 };
    let got = flora.view().reachable_foliage(&world.view(), from, reach);
    let sites: Vec<Site> = got.iter().map(|&(s, _)| s).collect();
    assert_eq!(sites, vec![near, seam], "reached {sites:?}");
    let mut sorted = sites.clone();
    sorted.sort_unstable();
    assert_eq!(sites, sorted, "the list must be sorted by site");
    // Each entry is the foliage that stand actually holds.
    for &(site, foliage) in &got {
        assert_eq!(foliage, flora.view().stand_at(site).expect("a listed stand").foliage);
    }
    // The raised one is out by exactly one voxel of `up`, and the one four columns away is
    // out by two of `horizontal`: both come back in when the box grows.
    let taller = flora.view().reachable_foliage(&world.view(), from, Reach { horizontal: 2, up: 2 });
    assert_eq!(
        taller.iter().map(|&(s, _)| s).collect::<Vec<_>>(),
        vec![near, high, seam],
        "one more voxel of `up` reaches the raised crown"
    );
    let wider = flora.view().reachable_foliage(&world.view(), from, Reach { horizontal: 4, up: 1 });
    assert_eq!(
        wider.iter().map(|&(s, _)| s).collect::<Vec<_>>(),
        vec![near, far, seam],
        "two more voxels sideways reach the far crown"
    );
}

/// What has to be in reach is a **crown cell**, not the stand's own column: a broad crown
/// whose trunk is four columns away still hangs two columns away, and that is what a
/// browser gets at. And a stand with no foliage left is not listed at all — there is
/// nothing there to eat.
#[test]
fn a_broad_crown_is_reached_by_its_cells_and_a_stripped_stand_is_not_listed() {
    let world = ledges(12, &[(0, 2), (4, 2)]);
    let mut flora = Flora::new(FloraConfig::default());
    // Velvetpad at its own `wood_max` is the widest crown of the five: radius 2 voxels, and
    // one voxel tall, so its cells run from column 2 to column 6.
    let pad_wood = flora.config().velvetpad.wood_max;
    assert_eq!(flora.config().velvetpad.crown_radius(pad_wood), 2.0);
    assert_eq!(flora.config().velvetpad.crown_voxels(pad_wood), 1);
    let pad = plant_wood(&mut flora, &world, 4, Species::Velvetpad, pad_wood);

    let from = Site { x: 0, y: 2, z: 0 };
    let reach = Reach { horizontal: 2, up: 1 };
    assert_eq!(
        flora.view().reachable_foliage(&world.view(), from, reach),
        vec![(pad, flora.view().stand_at(pad).expect("planted").foliage)],
        "the crown's nearest cell is two columns away, and the box is two"
    );
    // Its trunk alone would be out of reach: the centre is four columns away.
    assert!(
        flora
            .view()
            .reachable_foliage(&world.view(), from, Reach { horizontal: 1, up: 1 })
            .is_empty(),
        "a one-voxel box cannot reach a crown whose nearest cell is two away"
    );

    // Eat all of it, and it is no longer food.
    assert!(flora.take_foliage(pad, 1.0).is_some());
    assert!(
        flora.view().reachable_foliage(&world.view(), from, reach).is_empty(),
        "a stand with no foliage is not reachable foliage"
    );
    assert_residuals(&flora, "after stripping the only crown in reach");
}

// ----------------------------------------------------- 4. the producer response

/// The producer response, with **no recovery rule in it**: a stand whose whole canopy is
/// taken every tick has no income at all, so it pays maintenance and its reflush out of its
/// reserve and grows no wood; and the same treatment stopped after 50 ticks lets the
/// existing `foliage_rate` and reflush rules put foliage back.
///
/// Direction only, as the brief asks: nothing here asserts that the stand survives, that it
/// reaches full foliage, or how fast it gets anywhere.
#[test]
fn a_stand_stripped_every_tick_burns_reserve_and_grows_no_wood_and_regrows_when_left_alone() {
    let mut world = pillars(8, &[0, 2], 0.6);
    let mut flora = Flora::new(FloraConfig::default());
    let grazed = plant(&mut flora, &world, 0, Species::Springturf);
    let quiet = plant(&mut flora, &world, 2, Species::Springturf);

    let start = *flora.view().stand_at(grazed).expect("just planted");
    // The bite comes **before** the tick, which is what "stripped every tick" means: every
    // tick starts at `P = 0`, so `A ∝ P` is zero in every one of them. Stepping first
    // instead would give the founder one fully funded tick out of its planted canopy, and
    // its wood would grow by that tick's income alone — which is the same rule seen from
    // the other side, and not the treatment this test is about.
    for _ in 0..200 {
        flora.take_foliage(grazed, 1.0);
        flora.step(&mut world);
    }
    flora.take_foliage(grazed, 1.0);
    let stripped = *flora.view().stand_at(grazed).expect("stripping does not kill: wood is wood");
    assert_eq!(stripped.foliage, 0.0, "the treatment is a stripped canopy");
    assert!(
        stripped.reserve < start.reserve,
        "reserve {} did not fall from {}",
        stripped.reserve,
        start.reserve
    );
    assert!(
        stripped.wood <= start.wood,
        "wood grew from {} to {} on no income",
        start.wood,
        stripped.wood
    );
    // The untouched neighbour is the control: the same site, the same water, the same sky.
    let control = *flora.view().stand_at(quiet).expect("the control stands");
    assert!(control.foliage > 0.0 && control.wood >= start.wood, "{control:?}");
    assert_residuals(&flora, "after 200 stripped ticks");

    // Stop, and leave it alone: the foliage comes back through rules that were already
    // there. A direction, not an outcome.
    for _ in 0..200 {
        flora.step(&mut world);
    }
    let regrown = *flora.view().stand_at(grazed).expect("still standing");
    assert!(
        regrown.foliage > stripped.foliage,
        "foliage {} did not rise from {}",
        regrown.foliage,
        stripped.foliage
    );
    assert!(regrown.foliage > 0.0, "nothing regrew at all");
    assert_residuals(&flora, "after the quiet window");
}

/// A `Taken` is three numbers and nothing else: the default is empty, so a consumer layer
/// can accumulate into one without a special case.
#[test]
fn an_empty_taken_is_three_zeroes() {
    assert_eq!(Taken::default(), Taken { organic: 0.0, mineral: 0.0, energy: 0.0 });
}
