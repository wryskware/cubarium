use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species};

#[test]
fn slower_senescence_retains_more_foliage_and_sheds_less_litter() {
    let after_one_step = |senescence: f64| {
        let mut world_config = VoxelConfig {
            width: 1,
            height: 4,
            depth: 1,
            ..VoxelConfig::default()
        };
        world_config.day.plant_night_factor = 1.0;
        let mut world = World::empty(world_config);
        world.apply(WorldCommand::SetMaterial {
            x: 0,
            y: 1,
            z: 0,
            material: Material::Soil,
        });

        let mut config = FloraConfig::default().one_layer_species();
        let sc = config.species_mut(Species::Springturf);
        sc.alpha = 1.0;
        sc.wood_max = 1.0;
        sc.alive_min = 0.01;
        sc.senescence = senescence;
        // Hand-built rates isolate leaf loss from growth, starvation, reproduction,
        // and drought dormancy. The soil and world clock never advance.
        sc.assimilation = 0.0;
        sc.maintenance = 0.0;
        sc.foliage_rate = 0.0;
        sc.wood_rate = 0.0;
        sc.propagule_rate = 0.0;
        sc.cure_after_s = 0.0;
        let mut flora = Flora::new(config);
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 0,
                z: 0,
                species: Species::Springturf,
                wood: 0.5,
            },
        ));
        let site = Site { x: 0, y: 1, z: 0 };
        let initial = flora.view().stand_at(site).unwrap().foliage;
        flora.step(&mut world);
        let view = flora.view();
        let retained = view
            .stand_at(site)
            .expect("senescence leaves a live stand")
            .foliage;
        let litter = view.ground_at(site).unwrap().litter;
        assert!(retained > 0.0 && retained < initial);
        assert!(litter > 0.0, "shed leaves reach the litter pool");
        (retained, litter)
    };

    let (slow_foliage, slow_litter) = after_one_step(0.05);
    let (fast_foliage, fast_litter) = after_one_step(0.1);
    assert!(slow_foliage > fast_foliage);
    assert!(slow_litter < fast_litter);
}
