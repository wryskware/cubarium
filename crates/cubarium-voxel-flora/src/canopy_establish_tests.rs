//! **The canopy gate** (turf-establish, 2026-10-05): a photosynthetic seed comes up, and a
//! runner roots, only where the light reaching its newborn through the taller foliage
//! around it pays the newborn's upkeep ([`crate::SpeciesConfig::establish_light_margin`]).
//! A seed refused stays in the bank for a gap; a runner refused tries another face or
//! stays in its parent's parcel. A runner daughter is tied to its parent for support.
//!
//! Hand-built at 0.125 m voxels (the patch's), so a crown spans neighbouring columns: a
//! soil slab at field capacity, one row, open sky. The shade is made total with the test's
//! own `light_extinction_k`, and each fixture sets the margin it tests at half the cover the
//! open site gives, so no tuned number decides a verdict: the fixture asserts the covers
//! it relies on before it expects anything.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

use crate::seeds::{check_period, check_phase, newborn_stand, package_of};
use crate::step::{break_even_cover, canopy_light, establishment_gates, monod};
use crate::{Command, Flora, FloraConfig, Site, Species, Stand};

const TURF: Species = Species::Springturf;
const PAD: Species = Species::Velvetpad;
const TALL: Species = Species::Bloomcrown;
const VOXEL_M: f64 = 0.125;
const WIDTH: u32 = 10;

fn face(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

/// `WIDTH` soil columns at field capacity, faces at `y = 2`, open sky.
fn slab() -> World {
    let mut w = World::empty(VoxelConfig {
        width: WIDTH,
        height: 24,
        depth: 1,
        voxel_m: VOXEL_M,
        seed: 5,
        rain_m_per_s: 0.0,
        ..VoxelConfig::default()
    });
    let pore = cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil);
    let cap = w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for x in 0..i64::from(WIDTH) {
        for y in 1..=2u32 {
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: pore * cap });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    w
}

/// Nothing reproduces, no seed dies, nothing cures; the arming check off, so a passing
/// check sprouts; and any foliage over a column takes all its light.
fn quiet() -> FloraConfig {
    let mut c = FloraConfig::for_voxel_size(VOXEL_M);
    for sp in Species::ALL {
        let s = c.species_mut(sp);
        s.seed_attrition_per_s = 0.0;
        s.propagule_rate = 0.0;
        s.burr_pickup = 0.0;
        s.cure_after_s = 0.0;
    }
    c.sprout_after_arming = false;
    c.light_extinction_k = 1e6;
    c
}

/// A full-grown founder on `face(x)`.
fn plant(flora: &mut Flora, world: &World, x: u32, species: Species) {
    let wood = flora.config.species(species).wood_max;
    assert!(flora.apply(world, Command::SeedOnFace { site: face(x), species, wood }));
}

fn on(flora: &Flora, x: u32, species: Species) -> Option<Stand> {
    flora.view().stands.iter().find(|s| s.site == face(x) && s.species == species).copied()
}

/// One seed of `species` landed on `face(x)`, booked as seeded material.
fn land(flora: &mut Flora, x: u32, species: Species) {
    let sc = flora.config.species(species).clone();
    let organic = package_of(&sc);
    assert!(flora.deposit_seeds(face(x), species, organic, sc.n_tissue * organic, sc.energy_density * organic));
}

fn banked(flora: &Flora, x: u32, species: Species) -> f64 {
    let package = package_of(flora.config.species(species));
    flora.view().ground_at(face(x)).map_or(0.0, |g| g.seed_organic(species) / package)
}

/// Step to `face(x)`'s next seed-bank check.
fn check(flora: &mut Flora, world: &mut World, x: u32) {
    let period = check_period(&flora.config);
    let phase = check_phase(face(x), period);
    let after = flora.tick;
    let t = after - after % period + phase;
    let t = if t > after { t } else { t + period };
    while flora.tick < t {
        flora.step(world);
    }
}

/// The cover a newborn of `species` on `face(x)` would have now: its income at full
/// daylight over its upkeep, under the light through the stands as they stand.
fn cover(flora: &Flora, world: &World, x: u32, species: Species) -> f64 {
    let view = world.view();
    let site = face(x);
    let sc = flora.config.species(species);
    let nb = newborn_stand(&flora.config, site, species);
    let open = view.sky_visibility(i64::from(site.x), site.y, site.z);
    let l = canopy_light(&flora.config, &flora.stands, &view, open, &nb);
    let g = establishment_gates(&view, site, sc);
    assert!(g.passes(), "fixture: every site gate passes on face {x}");
    let mu = g.mean_water.map_or(0.0, |m| sc.moisture_at(m));
    let n = flora.view().ground_at(site).map_or(flora.config.initial_mineral, |g| g.mineral);
    break_even_cover(sc, nb.wood, nb.foliage, l, mu, monod(sc, n))
}

fn assert_conserved(flora: &Flora, when: &str) {
    let v = flora.view();
    let (o, n, e) = (
        v.organic() - v.ledger.expected_organic(),
        v.mineral() - v.ledger.expected_mineral(),
        v.energy() - v.ledger.expected_energy(),
    );
    assert!(o.abs() <= 1e-9 * v.organic().max(1.0), "{when}: organic {o}");
    assert!(n.abs() <= 1e-9 * v.mineral().max(1.0), "{when}: mineral {n}");
    assert!(e.abs() <= 1e-9 * v.energy().max(1.0), "{when}: energy {e}");
}

/// A seed under a taller stand whose shade leaves its newborn below the margin stays in
/// the bank, check after check; the same seed in the open comes up at its first check;
/// and once the shade is gone the banked seed comes up at its next check — waiting for a
/// gap.
#[test]
fn a_seed_under_shade_below_its_break_even_stays_banked_until_a_gap() {
    let mut world = slab();
    let mut flora = Flora::new(quiet());
    let (shaded, open) = (1, 7);
    let margin = 0.5 * cover(&flora, &world, open, TURF);
    assert!(margin > 0.0, "fixture: the open face pays something");
    flora.config.species_mut(TURF).establish_light_margin = margin;
    plant(&mut flora, &world, 2, TALL);
    assert!(cover(&flora, &world, shaded, TURF) < margin, "fixture: the tall crown shades face {shaded}");
    assert!(cover(&flora, &world, open, TURF) >= margin, "fixture: face {open} is in the open");
    land(&mut flora, shaded, TURF);
    land(&mut flora, open, TURF);

    check(&mut flora, &mut world, open);
    assert!(on(&flora, open, TURF).is_some(), "the open seed came up");
    for _ in 0..3 {
        check(&mut flora, &mut world, shaded);
        assert!(on(&flora, shaded, TURF).is_none(), "nothing comes up in the shade");
        assert_eq!(banked(&flora, shaded, TURF).round(), 1.0, "the seed waits in the bank");
    }
    assert!(flora.view().ledger.canopy_refused[TURF.index()] >= 3, "refused by the canopy gate");
    assert_conserved(&flora, "under the shade");

    // The gap: the tall stand is gone.
    assert!(flora.apply(&world, Command::Clear { x: 2, z: 0 }));
    check(&mut flora, &mut world, shaded);
    assert!(on(&flora, shaded, TURF).is_some(), "the banked seed came up in the gap");
    assert_eq!(banked(&flora, shaded, TURF), 0.0);
    assert_conserved(&flora, "after the gap");
}

/// A margin of zero opens the gate: the same seed comes up in the same shade.
#[test]
fn a_zero_margin_opens_the_canopy_gate() {
    let mut world = slab();
    let mut flora = Flora::new(quiet());
    flora.config.species_mut(TURF).establish_light_margin = 0.0;
    plant(&mut flora, &world, 2, TALL);
    land(&mut flora, 1, TURF);
    check(&mut flora, &mut world, 1);
    assert!(on(&flora, 1, TURF).is_some(), "no gate, so the shade does not hold it");
    assert_eq!(flora.view().ledger.canopy_refused[TURF.index()], 0);
}

/// Velvetpad runners from a pad on face 5 (reach 1: faces 4 and 6). The pad's crown is
/// narrowed to its own column, so a parent never shades its own daughter here (two mats
/// of one height that overlap do shade each other since SHADE-REAL); the shade is a tall
/// stand.
fn runner_world(tall_on: Option<u32>) -> (World, Flora) {
    let world = slab();
    let mut c = quiet();
    {
        let pad = c.species_mut(PAD);
        pad.crown_radius_m = [0.4 * VOXEL_M, 0.4 * VOXEL_M];
        pad.clonal_share = 0.99;
        pad.propagule_rate = 1.0;
        pad.donor_reserve_floor = 0.0;
        pad.runner_reach = 1;
    }
    let mut flora = Flora::new(c);
    let margin = 0.5 * cover(&flora, &world, 6, PAD);
    assert!(margin > 0.0, "fixture: the open face pays something");
    flora.config.species_mut(PAD).establish_light_margin = margin;
    plant(&mut flora, &world, 5, PAD);
    if let Some(x) = tall_on {
        plant(&mut flora, &world, x, TALL);
    }
    (world, flora)
}

/// In the open a runner roots beside its parent, tied to it for support.
#[test]
fn in_the_open_a_runner_roots_beside_its_parent() {
    let (mut world, mut flora) = runner_world(None);
    let parent = on(&flora, 5, PAD).unwrap();
    for _ in 0..20 {
        flora.step(&mut world);
    }
    let v = flora.view();
    assert!(v.ledger.clonal_births[PAD.index()] >= 1, "a runner rooted");
    let daughter = [4, 6].into_iter().find_map(|x| on(&flora, x, PAD)).expect("beside the parent");
    let tie = daughter.support.expect("tied to its parent");
    assert_eq!((tie.parent, tie.parent_site), (parent.id, parent.site));
    assert_eq!(v.ledger.runner_canopy_refused[PAD.index()], 0);
    assert_conserved(&flora, "after the runner");
}

/// Under a tall stand's shade (face 4 held by the tall stand itself, face 6 under its
/// crown) no runner roots: the face is refused by the canopy gate, and the package stays
/// in the parent's parcel.
#[test]
fn under_shade_below_its_break_even_a_runner_does_not_root() {
    let (mut world, mut flora) = runner_world(Some(4));
    assert!(cover(&flora, &world, 6, PAD) < flora.config.species(PAD).establish_light_margin, "fixture: face 6 is shaded");
    for _ in 0..20 {
        flora.step(&mut world);
    }
    let v = flora.view();
    assert_eq!(v.ledger.clonal_births[PAD.index()], 0, "no runner rooted");
    assert!(on(&flora, 6, PAD).is_none());
    assert!(v.ledger.runner_canopy_refused[PAD.index()] > 0, "refused by the canopy gate");
    assert!(v.ledger.runner_kept[PAD.index()] > 0, "the package stayed with the parent");
    let parent = on(&flora, 5, PAD).unwrap();
    assert!(parent.parcel >= package_of(flora.config.species(PAD)), "its matter is in its parcel");
    assert_conserved(&flora, "with the runner kept");
}
