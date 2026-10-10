//! Taste is measured in food stocks; changing animal mass cannot change its food cue.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species as Plant};

use crate::controller::{resolve_actions, BrowserForager};
use crate::{
    browser_mouth_foliage, Command, Controller, Fauna, FaunaConfig, Founder, StartingStores,
};

fn stand() -> (World, Flora) {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 6,
        depth: 6,
        voxel_m: 0.25,
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    let mut flora = Flora::new(FloraConfig::for_voxel_size(world.config().voxel_m));
    let wood = flora.config().species(Plant::Springturf).wood_max;
    assert!(flora.apply(
        &world,
        FloraCommand::Seed {
            x: 2,
            z: 3,
            species: Plant::Springturf,
            wood
        }
    ));
    (world, flora)
}

fn browser(config: FaunaConfig, world: &World) -> Fauna {
    let mut fauna = Fauna::new(config);
    assert!(fauna.apply(
        world,
        Command::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Browser,
            stores: StartingStores::FULL,
            heading_rad: 0.0,
        }
    ));
    fauna
}

fn taste_and_feed(fauna: &Fauna, world: &World, flora: &Flora) -> (f64, f64) {
    let manifest = fauna.config().manifest(Founder::Browser);
    let taste = manifest
        .modules
        .iter()
        .find(|m| m.name == "Taste(1)")
        .expect("taste");
    let (view, fv) = (world.view(), flora.view());
    let occupancy = crate::senses::cone_occupancy(&view, &fv, &fauna.view());
    let obs = crate::body::observation(
        fauna,
        0,
        &view,
        &fv,
        &manifest,
        None,
        Some((&occupancy).into()),
    );
    assert_eq!(
        obs[taste.offset + 2],
        1.0,
        "the stand is in the mouth's band"
    );
    let mut controller = BrowserForager::with_config(fauna.config().teachers);
    let action = resolve_actions(controller.drive(&obs), &manifest);
    (obs[taste.offset], action.feed)
}

#[test]
fn lineage_food_references_are_independent_of_mass() {
    let config = FaunaConfig::default();
    let flora = FloraConfig::default();
    for founder in Founder::ALL {
        let light = *config.founder(founder);
        let mut heavy = light;
        heavy.mass_scale *= 64.0;
        assert_eq!(
            light.taste_reference_stock(config.cue.emit_reference),
            heavy.taste_reference_stock(config.cue.emit_reference),
            "{founder:?}"
        );
        assert_eq!(
            light.taste_reference_on(config.cue.emit_reference, 0.25, &flora),
            heavy.taste_reference_on(config.cue.emit_reference, 0.25, &flora),
            "{founder:?}"
        );
        if let Some(explicit) = light.taste_reference {
            assert_eq!(
                heavy.taste_reference_on(config.cue.emit_reference, 0.25, &flora),
                explicit,
                "{founder:?}: explicit reference wins"
            );
        }
    }
}

#[test]
fn a_heavy_browser_tastes_and_feeds_on_the_same_in_band_stand() {
    let (world, flora) = stand();
    let mut config = FaunaConfig::default();
    // Put the hand-built stand above the configured gate without pinning a tuned gate.
    let probe = browser(config.clone(), &world);
    config.teachers.feed_gate = taste_and_feed(&probe, &world, &flora).0 * 0.5;
    let light = browser(config.clone(), &world);
    config.founders[Founder::Browser.index()].mass_scale *= 64.0;
    let heavy = browser(config, &world);
    let light_reading = taste_and_feed(&light, &world, &flora);
    let heavy_reading = taste_and_feed(&heavy, &world, &flora);
    assert!(
        light_reading.0 > light.config().teachers.feed_gate,
        "the stand supplies enough food to feed"
    );
    assert!(light_reading.1 > 0.0);
    assert_eq!(
        heavy_reading, light_reading,
        "mass changes neither the sensed stock nor the feed decision"
    );
}

#[test]
fn the_food_reference_knob_scales_the_observation_without_mass() {
    let (world, flora) = stand();
    let stock = {
        let fauna = browser(FaunaConfig::default(), &world);
        browser_mouth_foliage(
            &world.view(),
            &flora.view(),
            fauna.config(),
            &fauna.animals[0],
        )
        .expect("in-band food")
        .1
    };
    assert!(stock > 0.0);
    let mut previous = f64::INFINITY;
    for reference in [stock * 4.0, stock * 8.0] {
        let mut config = FaunaConfig::default();
        config.founders[Founder::Browser.index()].taste_reference = None;
        config.founders[Founder::Browser.index()].taste_reference_food = reference;
        let manifest = config.manifest(Founder::Browser);
        let cue = stock / reference;
        let expected = cue / (cue + manifest.tunings.chem_saturation);
        config.teachers.feed_gate = expected * 0.5;
        let light = browser(config.clone(), &world);
        config.founders[Founder::Browser.index()].mass_scale *= 64.0;
        let heavy = browser(config, &world);
        let (taste, feed) = taste_and_feed(&light, &world, &flora);
        assert!(
            (taste - expected).abs() < 1e-12,
            "the food knob sets the observation's reference"
        );
        assert!(
            taste < previous,
            "a larger food reference gives a smaller cue"
        );
        assert!(feed > 0.0);
        assert_eq!(taste_and_feed(&heavy, &world, &flora), (taste, feed));
        previous = taste;
    }
}

#[test]
fn the_effective_food_reference_is_recorded_in_the_manifest_digest() {
    let mut config = FaunaConfig::default();
    for founder in Founder::ALL {
        assert_eq!(
            config.manifest(founder).digest(),
            founder.manifest().digest()
        );
    }
    let founder = Founder::Browser;
    let before = config.manifest(founder);
    config.founders[founder.index()].taste_reference_food *= 2.0;
    let after = config.manifest(founder);
    assert_eq!(
        after.taste_reference,
        Some(config.founder(founder).taste_reference_food)
    );
    assert_ne!(after.digest(), before.digest());
    config.founders[founder.index()].taste_reference =
        Some(config.founder(founder).taste_reference_food * 0.25);
    let overridden = config.manifest(founder);
    assert_eq!(
        overridden.taste_reference,
        config.founder(founder).taste_reference
    );
    assert_ne!(overridden.digest(), after.digest());
}
