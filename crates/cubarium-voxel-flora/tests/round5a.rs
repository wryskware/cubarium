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
    Command, Flora, FloraConfig, Site, Species, SpeciesConfig, Taken,
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
