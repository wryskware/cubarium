//! Founding shade selection on a hand-built wet floor, without a pre-roll simulation.
use cubarium_voxel::{Command, Config, Material, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig, Founder};
use cubarium_voxel_flora::{Flora, FloraConfig, Species};
use cubarium_voxel_sim::habitat::{self, PreRoll};

#[test]
fn founding_glowcaps_use_shaded_faces_and_report_their_actual_light() {
    let mut world = World::empty(Config {
        width: 9,
        height: 8,
        depth: 9,
        voxel_m: 1.0,
        seed: 41,
        ..Config::default()
    });
    let water = world.config().materials.field_capacity(Material::Soil)
        * world.config().materials.pore_capacity(Material::Soil)
        * world.config().voxel_volume();
    for z in 0..9 {
        for x in 0..9 {
            for y in 1..=2 {
                world.apply(Command::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: water,
                });
                world.apply(Command::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
            if x < 5 {
                world.apply(Command::SetMaterial {
                    x,
                    y: 4,
                    z,
                    material: Material::Rock,
                });
            }
        }
    }
    let mut config = FloraConfig::for_voxel_size(1.0);
    for species in Species::ALL {
        config.species_mut(species).founder_per_m2 = 0.0;
    }
    config.glowcap.founder_per_m2 = 1.0;
    config.glowcap.founder_cap = 81;
    config.glowcap.founder_colony = 1;
    config.glowcap.sun_shade_max = 0.5;
    config.glowcap.dark_founder_share = 0.0;
    let threshold = config.glowcap.sun_shade_max;
    let sites = habitat::support_sites(&world);
    let view = world.view();
    assert!(
        sites
            .iter()
            .any(|s| view.sky_visibility(i64::from(s.x), s.y, s.z) <= threshold),
        "fixture has shaded faces"
    );
    assert!(
        sites
            .iter()
            .any(|s| view.sky_visibility(i64::from(s.x), s.y, s.z) > threshold),
        "fixture has bright faces"
    );
    let pre = PreRoll {
        wettest: vec![0.0; sites.len()],
        sites,
        ..PreRoll::default()
    };
    let mut flora = Flora::new(config);
    let mut fauna_config = FaunaConfig::placeholder();
    fauna_config.founding.reference_land_m2 = 0.0;
    let mut fauna = Fauna::new(fauna_config);
    habitat::seed_pre_rolled(
        &mut world,
        &mut flora,
        &mut fauna,
        &pre,
        [0; Founder::COUNT],
    );
    let v = flora.view();
    let fungi: Vec<_> = v
        .stands
        .iter()
        .filter(|s| s.species == Species::Glowcap)
        .collect();
    assert!(
        !fungi.is_empty(),
        "the eligible shaded ground must get founders"
    );
    for fungus in fungi {
        let sky =
            world
                .view()
                .sky_visibility(i64::from(fungus.site.x), fungus.site.y, fungus.site.z);
        assert!(
            (fungus.light - sky).abs() < 1e-12,
            "with no plants the founded light equals terrain sky: {} vs {sky}",
            fungus.light
        );
        assert!(
            fungus.light <= threshold,
            "a founder on a lit face: {fungus:?}"
        );
    }
    for (held, expected) in [
        (v.organic(), v.ledger.expected_organic()),
        (v.mineral(), v.ledger.expected_mineral()),
        (v.energy(), v.ledger.expected_energy()),
    ] {
        assert!((held - expected).abs() < 1e-9 * held.abs().max(1.0));
    }
    assert_eq!(
        world.tick(),
        0,
        "the founding test never advances the world"
    );
}
