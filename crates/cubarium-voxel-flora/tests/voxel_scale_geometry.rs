use cubarium_voxel_flora::{FloraConfig, Species};

#[test]
fn fine_voxels_keep_bloomcrown_crown_height_physical() {
    let wood = 0.2;
    let coarse_config = FloraConfig::default();
    let coarse = coarse_config.species(Species::Bloomcrown);
    let fine_config = FloraConfig::for_voxel_size(0.125);
    let fine = fine_config.species(Species::Bloomcrown);

    let coarse_m = coarse.crown_height(wood, 0.25) * 0.25;
    let fine_m = fine.crown_height(wood, 0.125) * 0.125;
    assert!((coarse_m - fine_m).abs() < 1e-12);
    // Package L: wood 0.2 of 0.6 is just past the 0.2 seedling fraction, so the crown
    // is 0.125 + 0.875 · (1/3 − 0.2) / 0.8 = 0.2708 m: one 0.25 m cell, two 0.125 m ones.
    assert!((coarse_m - (0.125 + 0.875 * (1.0 / 3.0 - 0.2) / 0.8)).abs() < 1e-12);
    assert_eq!(coarse.crown_voxels(wood, 0.25), 1);
    assert_eq!(fine.crown_voxels(wood, 0.125), 2);
}

#[test]
fn reference_and_coarser_configs_are_unchanged() {
    assert_eq!(FloraConfig::for_voxel_size(0.25), FloraConfig::default());
    // A coarser grid changes no authored geometry, but the cell size is always
    // recorded: a rule written in metres — decisions §5's 0.125 m ceiling on a woody
    // seedling — has to be converted somewhere, and this is where.
    let mut coarse = FloraConfig::for_voxel_size(0.5);
    assert_eq!(coarse.voxel_m, 0.5);
    coarse.voxel_m = FloraConfig::default().voxel_m;
    assert_eq!(coarse, FloraConfig::default());
}
