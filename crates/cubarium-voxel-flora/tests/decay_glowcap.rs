//! Short mechanism checks for DECAY-GLOWCAP, with deliberately chosen fixture rates.
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, CropPhase, DT, Deposit, DepositKind, Flora, FloraConfig, Site, Species, SpeciesConfig,
};

fn soil(roof: bool) -> World {
    let mut world = World::empty(VoxelConfig {
        width: 9,
        height: 8,
        depth: 9,
        voxel_m: 1.0,
        seed: 41,
        ..VoxelConfig::default()
    });
    let water = world.config().materials.field_capacity(Material::Soil)
        * world.config().materials.pore_capacity(Material::Soil)
        * world.config().voxel_volume();
    for z in 0..9 {
        for x in 0..9 {
            for y in 1..=2 {
                world.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: water,
                });
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
            if roof {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y: 4,
                    z,
                    material: Material::Rock,
                });
            }
        }
    }
    world
}

fn site(x: u32, z: u32) -> Site {
    Site { x, y: 2, z }
}

fn litter(flora: &mut Flora, site: Site, organic: f64) {
    let sc = flora.config().glowcap.clone();
    assert!(flora.deposit(
        site,
        Deposit {
            kind: DepositKind::Litter,
            organic,
            mineral: organic * sc.n_tissue,
            energy: organic * sc.energy_density,
        }
    ));
}

fn closes(flora: &Flora) {
    let v = flora.view();
    for (name, held, expected) in [
        ("organic", v.organic(), v.ledger.expected_organic()),
        ("mineral", v.mineral(), v.ledger.expected_mineral()),
        ("energy", v.energy(), v.ledger.expected_energy()),
    ] {
        assert!(
            (held - expected).abs() < 1e-9 * held.abs().max(1.0),
            "{name}: held {held}, expected {expected}"
        );
    }
}

#[test]
fn hypothetical_founder_cap_light_matches_full_caps_under_a_shared_face_plant() {
    let world = soil(false);
    let s = site(4, 4);
    let mut config = FloraConfig::for_voxel_size(1.0);
    // Unequal cap layers straddle a low canopy, so capacity weighting matters.
    config.glowcap.crown_height_m = [2.0, 2.0];
    config.glowcap.profile = SpeciesConfig::one_stage(vec![
        SpeciesConfig::foliage_layer([0.0, 0.5], 1.0, 0.2, 0.5),
        SpeciesConfig::foliage_layer([0.5, 1.0], 1.0, 0.8, 0.5),
    ]);
    config.bloomcrown.crown_height_m = [3.0, 3.0];
    config.bloomcrown.profile = SpeciesConfig::one_stage(vec![SpeciesConfig::foliage_layer(
        [0.2, 0.4],
        1.0,
        1.0,
        0.5,
    )]);
    let wood = config.glowcap.wood_max;
    let plant_wood = config.bloomcrown.wood_max;
    let mut flora = Flora::new(config);
    let bare = flora.glowcap_founder_light(&world, &[(s, wood)])[0];
    assert!(flora.apply(
        &world,
        Command::SeedOnFace {
            site: s,
            species: Species::Bloomcrown,
            wood: plant_wood,
        }
    ));
    let before = {
        let v = flora.view();
        (
            v.stands.to_vec(),
            format!("{:?}", v.ledger),
            v.organic(),
            v.mineral(),
            v.energy(),
        )
    };
    let predicted = flora.glowcap_founder_light(&world, &[(s, wood)])[0];
    assert!(
        predicted < bare,
        "the shared face's plant shades caps: {predicted} vs {bare}"
    );
    {
        let v = flora.view();
        assert_eq!(
            v.stands,
            before.0.as_slice(),
            "hypothetical caps never change existing stands"
        );
        assert_eq!(
            format!("{:?}", v.ledger),
            before.1,
            "no hypothetical imports or flows"
        );
        assert_eq!(
            (v.organic(), v.mineral(), v.energy()),
            (before.2, before.3, before.4)
        );
        assert!(
            v.fungus_at(s).is_none(),
            "hypothetical caps are not planted"
        );
    }
    assert!(flora.apply(
        &world,
        Command::SeedOnFace {
            site: s,
            species: Species::Glowcap,
            wood,
        }
    ));
    flora.refresh_founder_light(&world);
    let actual = flora.view().fungus_at(s).unwrap().light;
    assert!(
        (predicted - actual).abs() < 1e-12,
        "capacity-weighted hypothetical exposure matches full cap stocks: {predicted} vs {actual}"
    );
    closes(&flora);
}

#[test]
fn feeding_fragmentation_speeds_only_nearby_litter_and_then_fades() {
    let world = soil(false);
    let mut c = FloraConfig::for_voxel_size(1.0);
    c.decomposition = 0.1;
    c.litter_fragment_boost = 0.8;
    c.litter_fragment_radius_m = 1.1;
    c.litter_fragment_duration_s = 4.0 * DT;
    let mut flora = Flora::new(c);
    let (fed, near, far) = (site(3, 3), site(4, 3), site(7, 3));
    for s in [fed, near, far] {
        litter(&mut flora, s, 2.0);
    }
    let bite = flora.take_litter(fed, 0.01).expect("a litter bite");
    assert!(bite.organic > 0.0);
    // Restore the bite as a booked deposit to keep the decay pools identical.
    assert!(flora.deposit(
        fed,
        Deposit {
            kind: DepositKind::Litter,
            organic: bite.organic,
            mineral: bite.mineral,
            energy: bite.energy
        }
    ));
    flora.fragment_litter(&world, fed);
    let mut previous_excess = f64::INFINITY;
    for _ in 0..5 {
        let before: Vec<_> = [fed, near, far]
            .into_iter()
            .map(|s| flora.view().ground_at(s).unwrap().litter)
            .collect();
        flora.decompose_dead_pools();
        let loss: Vec<_> = [fed, near, far]
            .into_iter()
            .enumerate()
            .map(|(i, s)| (before[i] - flora.view().ground_at(s).unwrap().litter) / before[i])
            .collect();
        assert!(
            (loss[0] - loss[1]).abs() < 1e-12,
            "the footprint treats both faces alike"
        );
        let excess = loss[0] - loss[2];
        assert!(
            excess >= -1e-12 && excess < previous_excess,
            "the local boost fades: {excess} after {previous_excess}"
        );
        previous_excess = excess + 1e-13;
        closes(&flora);
    }
    assert!(
        previous_excess.abs() < 1e-12,
        "the expired boost leaves ordinary decay"
    );
    assert_eq!(flora.view().ground_at(fed).unwrap().fragmented_s, 0.0);
}

fn fungus_config() -> FloraConfig {
    let mut c = FloraConfig::for_voxel_size(1.0);
    c.step_every = 1;
    c.decomposition = 0.0;
    c.wood_decomposition = 0.0;
    c.initial_mineral = 1.0;
    c.upkeep_q10 = 1.0;
    let s = &mut c.glowcap;
    s.wood_max = 1.0;
    s.alive_min = 0.1;
    s.alpha = 0.5;
    s.reserve_cap = 0.0;
    s.maintenance = 0.0;
    s.senescence = 0.0;
    s.foliage_rate = 2.0 / DT;
    s.wood_rate = 2.0 / DT;
    s.substrate_uptake_per_s = 20.0 / DT;
    s.substrate_budgeted = true;
    s.substrate_required = true;
    s.substrate_yield = 0.5;
    s.rooting_radius = 0;
    s.rooting_depth = 1;
    s.dark_uptake_per_s = 0.0;
    s.propagule_rate = 0.0;
    s.transpiration_m3_per_s = 0.0;
    s.wilt_water = 0.0;
    s.full_water = 0.5;
    s.sun_shade_max = 0.5;
    s.sun_dark_shelter = 0.0;
    s.sun_cap_loss_per_s = 0.5 / DT;
    s.sun_mycelium_loss_per_s = 0.2 / DT;
    c
}

#[test]
fn a_lit_glowcap_dies_despite_litter_while_a_shaded_one_grows() {
    let s = site(4, 4);
    for roof in [false, true] {
        let mut world = soil(roof);
        let mut flora = Flora::new(fungus_config());
        assert!(flora.apply(
            &world,
            Command::SeedOnFace {
                site: s,
                species: Species::Glowcap,
                wood: 0.5,
            }
        ));
        litter(&mut flora, s, 100.0);
        let initial = *flora.view().fungus_at(s).unwrap();
        flora.step(&mut world);
        let first = *flora
            .view()
            .fungus_at(s)
            .expect("one sun tick leaves living wood");
        if roof {
            assert!(
                first.light <= flora.config().glowcap.sun_shade_max,
                "roof gives shade"
            );
            assert!(
                first.wood > initial.wood,
                "shade permits saprotroph growth on litter"
            );
        } else {
            assert!(
                first.light > 0.99,
                "fixture is fully exposed: {}",
                first.light
            );
            assert!(
                first.wood < initial.wood,
                "abundant substrate cannot outgrow full sun"
            );
        }
        for _ in 0..10 {
            flora.step(&mut world);
        }
        assert_eq!(
            flora.view().fungus_at(s).is_some(),
            roof,
            "full sun kills the stock; shade keeps it alive"
        );
        closes(&flora);
    }
}

#[test]
fn founding_crop_phases_are_spread_and_all_stocks_are_booked() {
    let world = soil(false);
    let mut c = FloraConfig::for_voxel_size(1.0);
    c.lanternberry.flower_s = 10.0;
    c.lanternberry.fruit_drop_s = 10.0;
    c.lanternberry.fruit_rest_s = 10.0;
    c.lanternberry.fruit_per_s = 0.01;
    c.lanternberry.fruit_cap = 0.01;
    let mut flora = Flora::new(c);
    let wood = flora.config().lanternberry.wood_max;
    for z in 0..8 {
        for x in 0..8 {
            assert!(flora.apply(
                &world,
                Command::SeedOnFace {
                    site: site(x, z),
                    species: Species::Lanternberry,
                    wood,
                }
            ));
        }
    }
    let (organic, mineral, energy) = {
        let v = flora.view();
        (v.organic(), v.mineral(), v.energy())
    };
    let imported = {
        let l = flora.view().ledger;
        (l.seeded_organic_in, l.seeded_mineral_in, l.seeded_energy_in)
    };
    flora.stagger_founder_crops(&world);
    let v = flora.view();
    assert!(
        v.stands.iter().any(|s| s.crop.phase == CropPhase::Flower),
        "some founders flower"
    );
    assert!(
        v.stands.iter().any(|s| s.crop.phase == CropPhase::Fruit),
        "some founders bear fruit"
    );
    assert!(
        v.stands.iter().any(|s| s.fruit_rest_s > 0.0),
        "some founders rest"
    );
    // Hanging founder fruit arrives as imported tissue, like the founder's wood.
    for (held, before, delta) in [
        (
            v.organic(),
            organic,
            v.ledger.seeded_organic_in - imported.0,
        ),
        (
            v.mineral(),
            mineral,
            v.ledger.seeded_mineral_in - imported.1,
        ),
        (v.energy(), energy, v.ledger.seeded_energy_in - imported.2),
    ] {
        assert!(
            (held - before - delta).abs() < 1e-9 * held.abs().max(1.0),
            "founding crop stocks match their booked imports: {held} - {before} vs {delta}"
        );
    }
    closes(&flora);
}
