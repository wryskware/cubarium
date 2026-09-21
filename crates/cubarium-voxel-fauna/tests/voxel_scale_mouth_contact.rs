use cubarium_voxel::{Command as WorldCommand, Config, Material, World};
use cubarium_voxel_fauna::{
    Command, Fauna, FaunaConfig, Founder, StartingStores, browser_mouth_foliage,
};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species};

fn world(voxel_m: f64) -> World {
    let mut w = World::empty(Config {
        width: 6,
        height: 8,
        depth: 4,
        voxel_m,
        ..Config::default()
    });
    for z in 0..4 {
        for x in 0..6i64 {
            for y in 1..=2 {
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

#[test]
fn browser_contacts_the_scaled_crown_at_both_grid_sizes() {
    let wood = 0.2;
    for voxel_m in [0.25, 0.125] {
        let w = world(voxel_m);
        let mut flora = Flora::new(if voxel_m == 0.125 {
            FloraConfig::for_voxel_size(voxel_m)
        } else {
            FloraConfig::default()
        });
        let site = Site { x: 2, y: 2, z: 2 };
        assert!(flora.apply(
            &w,
            FloraCommand::Seed {
                x: 2,
                z: 2,
                species: Species::Bloomcrown,
                wood
            }
        ));
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(fauna.apply(
            &w,
            Command::IntroduceFounder {
                x: 2,
                z: 2,
                founder: Founder::Browser,
                stores: StartingStores {
                    body: 0.8,
                    reserve: 1.0
                },
                heading_rad: 0.0
            }
        ));
        let id = fauna.view().ledger.births - 1;
        let animal = fauna.view().animal(id).expect("browser");
        assert_eq!(
            browser_mouth_foliage(&w.view(), &flora.view(), animal).map(|(s, _)| s),
            Some(site)
        );
    }
}
