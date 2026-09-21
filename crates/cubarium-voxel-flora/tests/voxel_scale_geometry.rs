use cubarium_voxel_flora::{FloraConfig, Species};

#[test]
fn fine_voxels_keep_bloomcrown_crown_height_physical() {
    let wood = 0.2;
    let coarse_config = FloraConfig::default();
    let coarse = coarse_config.species(Species::Bloomcrown);
    let fine_config = FloraConfig::for_voxel_size(0.125);
    let fine = fine_config.species(Species::Bloomcrown);

    let coarse_m = coarse.crown_height(wood) * 0.25;
    let fine_m = fine.crown_height(wood) * 0.125;
    assert!((coarse_m - fine_m).abs() < 1e-12);
    assert_eq!(coarse.crown_voxels(wood), 2);
    assert_eq!(fine.crown_voxels(wood), 3);
}

#[test]
fn reference_and_coarser_configs_are_unchanged() {
    assert_eq!(FloraConfig::for_voxel_size(0.25), FloraConfig::default());
    assert_eq!(FloraConfig::for_voxel_size(0.5), FloraConfig::default());
}
