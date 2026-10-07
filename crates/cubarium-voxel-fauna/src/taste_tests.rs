//! **TASTE** (Wrysk 2026-10-06: "why does empty face still taste like algae? shouldnt that
//! be fixed?"): an animal perceives only the food it can eat. Plant tissue keeps a grazing
//! refuge (`graze_refuge` of each layer's capacity) that no bite takes, so every reading of
//! "food here" — the fungal scent a glowcap's caps give off, a hunter's cue for its prey's
//! habitat — reads the tissue above it, through the one accessor
//! ([`cubarium_voxel_flora::FloraView::edible_foliage`]) that agrees with the take.
//!
//! A child of `step`, for the predation fixture. Every case checks a mechanism against the
//! config it sets; no default, ratio or population is asserted.

use super::predation_fixture::*;
use crate::{Fauna, FaunaConfig, Food, Founder, StartingStores};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Command as FloraCommand, FaceLayer, Flora, FloraConfig, Site, Species};

const GNAWER: Founder = Founder::Capgnawer;

/// Every layer of a stand, whatever its height.
const ALL_LAYERS: std::ops::RangeInclusive<i64> = i64::MIN..=i64::MAX;

/// A full-grown stand of `species` on column `(x, z)` of the plain, in its own place.
fn grown(flora: &mut Flora, world: &World, species: Species, x: i64, z: u32) -> (Site, FaceLayer) {
    let wood = flora.config().species(species).wood_max;
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species, wood }), "fixture: {species:?} seeded");
    let site = Site { x: x as u32, y: SURFACE_Y, z };
    let st = flora.view().stands_on(site).iter().find(|s| s.species == species).expect("fixture: the stand").clone();
    assert!(st.foliage > 0.0, "fixture: {species:?} holds tissue");
    (site, st.place)
}

fn stand_of(flora: &Flora, site: Site, place: FaceLayer) -> cubarium_voxel_flora::Stand {
    flora.view().stand_in(site, place).expect("the stand").clone()
}

/// **The edible tissue a view reads is what a whole take gives.** A glowcap's caps and a
/// bloomcrown's foliage, each full and then cropped to its grazing floor: the view's
/// `edible_foliage` equals what `take_foliage_of` withdraws from every layer when asked for
/// everything, and once cropped it reads 0 while the refuge is still on the stand.
#[test]
fn the_edible_tissue_a_view_reads_is_what_a_whole_take_gives() {
    for species in [Species::Glowcap, Species::Bloomcrown] {
        let world = clock_off(53);
        let mut flora = Flora::new(FloraConfig::default());
        assert!(flora.config().species(species).graze_refuge > 0.0, "fixture: {species:?} keeps a refuge");
        let (site, place) = grown(&mut flora, &world, species, 20, 6);
        for round in ["full", "cropped"] {
            let st = stand_of(&flora, site, place);
            let seen = flora.view().edible_foliage(&st);
            let took = flora.take_foliage_of(site, place, 1.0e9, &ALL_LAYERS).map_or(0.0, |t| t.taken.organic);
            assert!(
                (seen - took).abs() <= 1e-12 * took.abs().max(1.0),
                "{species:?} {round}: the view reads {seen}, a whole take gives {took}"
            );
            if round == "cropped" {
                assert_eq!(seen, 0.0, "{species:?} cropped to its floor reads no food");
                assert!(stand_of(&flora, site, place).foliage > 0.0, "fixture: the refuge stays on the stand");
            } else {
                assert!(seen > 0.0, "{species:?} full holds food");
            }
        }
    }
}

/// The `Chem(fungal)` a young capgnawer 1 m west of `cap_x` reads after the field has stood
/// under a long half-life.
fn fungal_read(world: &World, flora: &Flora, cap_x: f64) -> f64 {
    let mut c = FaunaConfig::default();
    c.cue.fungal_half_life_s = 300.0;
    let mut fauna = Fauna::new(c);
    let core = fauna.config().founders[GNAWER.index()].core;
    let id = introduce(&mut fauna, world, GNAWER, StartingStores { body: core.body_min / core.body_max, reserve: 0.3 });
    place(&mut fauna, id, cap_x - 1.0, 6.5 * VOXEL_M, EAST);
    let mut senses = crate::Senses::with_cue(fauna.config().cue);
    for _ in 0..(8.0 * 300.0 / crate::senses::UPDATE_S).ceil() as usize {
        senses.update(&world.view(), &flora.view());
    }
    let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
    let manifest = fauna.config().manifest(GNAWER);
    let m = *manifest.modules.iter().find(|m| m.name == "Chem(fungal)").expect("the fungal chem");
    let obs = crate::body::observation(&fauna, i, &world.view(), &flora.view(), &manifest, Some(&mut senses), None);
    assert_eq!(obs[m.offset + 2], 1.0, "the chem receptor is valid");
    obs[m.offset]
}

/// **A glowcap cropped to its floor smells of nothing.** The same young capgnawer 1 m from
/// one glowcap reads `Chem(fungal)` from its full caps, and nothing once the caps are cropped
/// to the refuge no mouth can take (the refuge still on the stand).
#[test]
fn a_glowcap_cropped_to_its_floor_gives_off_no_fungal_scent() {
    let world = clock_off(59);
    let mut config = FloraConfig::default();
    config.glowcap.mycelium_graze_fraction = 0.0;
    let mut flora = Flora::new(config);
    let (site, place) = grown(&mut flora, &world, Species::Glowcap, 30, 6);
    let cap_x = (f64::from(site.x) + 0.5) * VOXEL_M;
    let full = fungal_read(&world, &flora, cap_x);
    assert!(full > 0.0, "fixture: full caps are smelled 1 m off ({full})");
    assert!(flora.take_foliage_of(site, place, 1.0e9, &ALL_LAYERS).is_some(), "fixture: caps to crop");
    assert!(stand_of(&flora, site, place).foliage > 0.0, "fixture: the refuge stays on the stand");
    let cropped = fungal_read(&world, &flora, cap_x);
    assert_eq!(cropped, 0.0, "caps at their floor give off no scent (full read {full})");
}

/// **A hunter's cue for its prey's food reads only what the prey can eat.** A stand's cue
/// for cap tissue (a glowcap) and for foliage (a bloomcrown) is 1 while it holds tissue above
/// its floor and 0 once cropped to it; a film face's cue is its edible film against the
/// film's capacity, 0 at the refuge.
#[test]
fn a_hunters_habitat_cue_reads_only_edible_food() {
    for (species, food) in [(Species::Glowcap, Food::CapTissue), (Species::Bloomcrown, Food::Foliage)] {
        let world = clock_off(61);
        let mut flora = Flora::new(FloraConfig::default());
        let (site, place) = grown(&mut flora, &world, species, 20, 6);
        let cue = |flora: &Flora| crate::hunt::stand_cue(&flora.view(), &stand_of(flora, site, place), food);
        assert_eq!(cue(&flora), 1.0, "{species:?} full is {food:?}");
        assert!(flora.take_foliage_of(site, place, 1.0e9, &ALL_LAYERS).is_some(), "fixture: tissue to crop");
        assert_eq!(cue(&flora), 0.0, "{species:?} at its floor is no {food:?}");
    }
    // Film: a pond face full, then grazed to the refuge.
    let world = film_pond();
    let fauna = Fauna::new(FaunaConfig::default());
    let fc = FloraConfig { film_seed_per_m2: FloraConfig::default().film_capacity_per_m2, ..FloraConfig::default() };
    let mut flora = Flora::new(fc.clone());
    assert!(flora.seed_film(&world) > 0, "fixture: film on the pond");
    let s = Site { x: 4, y: 2, z: 2 };
    let cell_m2 = VOXEL_M * VOXEL_M;
    let cue = |flora: &Flora| {
        let fv = flora.view();
        let g = fv.ground_at(s).expect("a film face");
        crate::hunt::ground_cue(&fv, &fauna, g, Food::Film, cell_m2)
    };
    let k = fc.film_capacity_per_m2 * cell_m2;
    let want = flora.view().edible_film_at(s) / k;
    assert!((cue(&flora) - want).abs() <= 1e-12, "full: the cue is the edible share {want}, read {}", cue(&flora));
    assert!(flora.take_film(s, 1.0).is_some(), "fixture: film to graze");
    assert!(flora.view().film_at(s) > 0.0, "fixture: the refuge stays");
    assert_eq!(cue(&flora), 0.0, "a face at its refuge is no film");
}

/// A flat bedrock-and-soil strip with one cell of water standing over columns `0..16`
/// (the film's wet-eligible faces at `y = 2`), at 0.25 m.
fn film_pond() -> World {
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material};
    let mut world = World::empty(VoxelConfig {
        width: 24,
        height: 8,
        depth: 6,
        voxel_m: VOXEL_M,
        day: cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() },
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..24 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    let cell = world.config().voxel_volume();
    for z in 0..6 {
        for x in 0..16 {
            world.apply(WorldCommand::AddWater { x, y: 3, z, volume_m3: cell });
        }
    }
    world
}
