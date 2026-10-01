//! **Knob wiring** (R2 integration): the plant layer reads the world's own configured
//! numbers, not the defaults. `world.standing_support_fill` decides when a vine face is
//! flooded, and `[world.materials]` decides what water a root box offers and how much of
//! it a stand can drink. Before R2's integration the plant layer read a const and the
//! default materials for all three.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Face, FaceDir, Flora, FloraConfig, Site, Species, face_eligible,
};

/// A strip one slab deep: bedrock floor, soil at `y = 1..=2` holding `pore` of **its
/// configured** capacity, air above.
fn plain(config: VoxelConfig, pore: f64) -> World {
    let width = config.width;
    let mut w = World::empty(config);
    let cap = w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for x in 0..width as i64 {
        for y in 1..=2u32 {
            let got = w.apply(WorldCommand::AddWater {
                x,
                y,
                z: 0,
                volume_m3: pore * cap,
            });
            assert!((got - pore * cap).abs() < 1e-12, "the void took {got}");
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

fn strip() -> VoxelConfig {
    VoxelConfig {
        width: 8,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 7,
        ..VoxelConfig::default()
    }
}

fn seed(flora: &mut Flora, world: &World, x: u32, species: Species) {
    let wood = flora.config().species(species).wood_max;
    assert!(flora.apply(
        world,
        Command::Seed {
            x: i64::from(x),
            z: 0,
            species,
            wood,
        }
    ));
}

/// `world.standing_support_fill` reaches the vine layer: a wall face whose front cell
/// holds free water at 0.3 of a cell is dry under the default 0.5 and flooded under 0.2.
#[test]
fn the_worlds_standing_support_fill_decides_a_flooded_vine_face() {
    let eligible = |fill: f64| {
        let mut config = strip();
        config.standing_support_fill = fill;
        let mut w = World::empty(config);
        w.apply(WorldCommand::SetMaterial {
            x: 4,
            y: 3,
            z: 0,
            material: Material::Rock,
        });
        let cell = w.config().voxel_volume();
        w.apply(WorldCommand::AddWater {
            x: 3,
            y: 3,
            z: 0,
            volume_m3: 0.3 * cell,
        });
        face_eligible(&w.view(), Face::new(4, 3, 0, FaceDir::NegX))
    };
    assert!(
        eligible(VoxelConfig::default().standing_support_fill),
        "dry at the default"
    );
    assert!(
        !eligible(0.2),
        "flooded once the configured fill is under the water"
    );
}

/// `[world.materials]` reaches a stand's root-box reading: the same pore fraction is
/// full moisture under the default soil and less under a soil whose wilting point and
/// field capacity are raised around it.
#[test]
fn the_worlds_soil_retention_changes_a_stands_moisture() {
    let moisture = |config: VoxelConfig| {
        let default_fc = VoxelConfig::default().materials.soil.field_capacity;
        let mut world = plain(config, default_fc);
        let mut flora = Flora::new(FloraConfig::default());
        seed(&mut flora, &world, 3, Species::Bloomcrown);
        flora.step(&mut world);
        flora
            .view()
            .stand_at(Site { x: 3, y: 2, z: 0 })
            .expect("still standing")
            .moisture
    };
    let base = moisture(strip());
    let mut wetter = strip();
    wetter.materials.soil.field_capacity = 0.99;
    wetter.materials.soil.wilting_point = 0.5;
    let moved = moisture(wetter);
    assert!(
        (base - 1.0).abs() < 1e-12,
        "default soil at field capacity: μ {base}"
    );
    assert!(
        moved < base - 1e-6,
        "raised retention reads drier: μ {moved}"
    );
}

/// `[world.materials]` reaches what a stand drinks: draining the same root box to its
/// wilting point yields water in proportion to the soil's configured pore capacity.
#[test]
fn the_worlds_soil_pore_capacity_changes_what_a_stand_drinks() {
    let drunk = |config: VoxelConfig| {
        let soil = config.materials.soil;
        let pore = soil.wilting_point + 0.05 * (soil.field_capacity - soil.wilting_point);
        let mut world = plain(config, pore);
        let mut fc = FloraConfig::default();
        fc.stonecushion.transpiration_m3_per_s = 1.0e3;
        let mut flora = Flora::new(fc);
        seed(&mut flora, &world, 3, Species::Stonecushion);
        for _ in 0..10 {
            flora.step(&mut world);
        }
        flora.view().ledger.transpired_m3
    };
    let base = drunk(strip());
    let mut porous = strip();
    porous.materials.soil.pore_capacity *= 2.0;
    let moved = drunk(porous);
    assert!(base > 0.0, "the stand drank something");
    assert!(
        (moved / base - 2.0).abs() < 1e-6,
        "twice the pore space, twice the drinkable water: {base} then {moved}"
    );
}
