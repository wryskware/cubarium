//! The plant layer's parameter registry (package R1): coverage, get/set round trip,
//! ranges, the refusals the hand-written `SpeciesConfig` / `VineConfig` / `FloraConfig`
//! validators made, and the voxel rescale by each row's scale tag against the hand-kept
//! list it replaced.

#[path = "../../cubarium-voxel/tests/support/registry_spec.rs"]
mod spec;

use cubarium_voxel_flora::{FloraConfig, Species, VineConfig, registry};

#[test]
fn every_flora_number_has_a_row_and_every_row_a_number() {
    spec::coverage(registry(), &[&FloraConfig::default()]);
}

#[test]
fn get_and_set_round_trip_on_every_flora_row() {
    spec::round_trip(registry(), &FloraConfig::default());
}

#[test]
fn a_flora_value_outside_its_range_is_refused_naming_path_and_range() {
    spec::range_refusals(registry(), &FloraConfig::default());
}

#[test]
fn an_unknown_flora_path_is_refused() {
    spec::unknown_paths_refused(registry(), &FloraConfig::default());
}

/// Every refusal the deleted hand validators made, through `FloraConfig::validate`, each
/// with the words its message must carry.
#[test]
fn the_old_flora_validator_refusals_still_refuse() {
    let refuse = |words: &[&str], edit: &dyn Fn(&mut FloraConfig)| {
        let mut c = FloraConfig::default();
        edit(&mut c);
        let e = c.validate().expect_err(words[0]);
        for w in words {
            assert!(e.contains(w), "{words:?}: {e}");
        }
    };
    // The species' finite, nonnegative rates (`SpeciesConfig::validate`'s list of 28).
    for field in [
        "substrate_uptake_per_s",
        "establish_substrate_min",
        "alpha",
        "reserve_cap",
        "maintenance",
        "senescence",
        "foliage_rate",
        "wood_rate",
        "build",
        "dieback",
        "donor_min",
        "donor_reserve_floor",
        "propagule_rate",
        "energy_density",
        "seed_attrition_per_s",
        "seed_max_age_s",
        "n_tissue",
        "reserve_share",
        "reflush_below",
        "assimilation",
        "nutrient_half",
        "nutrient_draw_max",
        "light_half",
        "transpiration_m3_per_s",
        "stress_rate_per_s",
        "relax_rate_per_s",
        "drown_depth_m",
        "water_depth_min_m",
    ] {
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            let mut c = FloraConfig::default();
            let path = format!("flora.bloomcrown.{field}");
            let e = registry()
                .set(&mut c, &path, toml::Value::Float(bad))
                .expect_err(&path);
            assert!(e.contains(&path), "{e}");
        }
    }
    // The bounds the old validator read only for finiteness.
    for field in [
        "wilt_water",
        "full_water",
        "establish_water_min",
        "establish_light_min",
        "saturated_pore",
        "establish_saturated_max",
        "alive_min",
    ] {
        let mut c = FloraConfig::default();
        let path = format!("flora.umbrellafrond.{field}");
        registry()
            .set(&mut c, &path, toml::Value::Float(f64::NAN))
            .expect_err(&path);
    }
    refuse(&["glowcap", "substrate_yield"], &|c| c.glowcap.substrate_yield = 1.5);
    refuse(&["glowcap", "substrate_yield"], &|c| c.glowcap.substrate_yield = -0.1);
    refuse(&["springturf", "clonal_share"], &|c| c.springturf.clonal_share = 1.5);
    refuse(&["springturf", "overtop_per_check"], &|c| {
        c.springturf.overtop_per_check = -0.1
    });
    refuse(&["vaulttree", "seed_mass"], &|c| c.vaulttree.seed_mass = 0);
    refuse(&["glowcap", "spore_tries"], &|c| c.glowcap.spore_tries = 0);
    refuse(&["bloomcrown", "crown_height_m"], &|c| c.bloomcrown.crown_height_m[1] = -1.0);
    refuse(&["bloomcrown", "crown_radius_m"], &|c| {
        c.bloomcrown.crown_radius_m[0] = f64::NAN
    });
    refuse(&["velvetpad", "graze_refuge"], &|c| c.velvetpad.graze_refuge = 1.0);
    refuse(&["velvetpad", "graze_refuge"], &|c| c.velvetpad.graze_refuge = -0.1);
    refuse(&["springturf", "wood_max"], &|c| c.springturf.wood_max = 0.0);
    // The cross-field rules.
    refuse(&["springturf", "sums to"], &|c| c.springturf.propagule_split = [0.5, 0.4, 0.2]);
    refuse(&["velvetpad", "w_frac"], &|c| c.velvetpad.propagule_split = [0.0, 0.6, 0.4]);
    refuse(&["stonecushion", "propagule_split"], &|c| {
        c.stonecushion.propagule_split = [1.2, -0.1, -0.1]
    });
    refuse(&["stonecushion", "alive_min"], &|c| {
        c.stonecushion.alive_min = c.stonecushion.wood_max * 2.0
    });
    // The anatomy.
    refuse(&["springturf", "profile is empty"], &|c| c.springturf.profile.clear());
    refuse(&["springturf", "no layers"], &|c| c.springturf.profile[0].layers.clear());
    refuse(&["bloomcrown", "threshold"], &|c| {
        let last = c.bloomcrown.profile.len() - 1;
        c.bloomcrown.profile[last].wood_fraction_max = 0.5;
        c.bloomcrown.profile.insert(0, c.bloomcrown.profile[last].clone());
        c.bloomcrown.profile[0].wood_fraction_max = 0.9;
    });
    refuse(&["springturf", "full-grown"], &|c| {
        let last = c.springturf.profile.len() - 1;
        c.springturf.profile[last].wood_fraction_max = 0.9;
    });
    refuse(&["springturf", "band"], &|c| {
        c.springturf.profile[0].layers[0].band = [0.6, 0.4]
    });
    refuse(&["springturf", "band"], &|c| {
        c.springturf.profile[0].layers[0].band = [0.0, 1.5]
    });
    refuse(&["springturf", "radius"], &|c| c.springturf.profile[0].layers[0].radius = 0.0);
    refuse(&["springturf", "radius"], &|c| c.springturf.profile[0].layers[0].radius = 1.5);
    refuse(&["springturf", "porosity"], &|c| {
        c.springturf.profile[0].layers[0].porosity = 1.5
    });
    refuse(&["springturf", "sum to"], &|c| {
        c.springturf.profile[0].layers[0].share += 0.5
    });
    refuse(&["springturf", "height_m_max"], &|c| {
        c.springturf.profile[0].height_m_max = Some(0.0)
    });
    // The shared rates.
    for (field, edit) in [
        ("shade_k_per_m2", (|c: &mut FloraConfig| c.shade_k_per_m2 = f64::NEG_INFINITY) as fn(&mut FloraConfig)),
        ("decomposition", |c| c.decomposition = -1.0),
        ("wood_decomposition", |c| c.wood_decomposition = f64::NAN),
        ("carrion_decomposition", |c| c.carrion_decomposition = -1.0),
        ("litter_energy_cap", |c| c.litter_energy_cap = -1.0),
        ("initial_mineral", |c| c.initial_mineral = -1.0),
        ("seed_check_s", |c| c.seed_check_s = -1.0),
    ] {
        refuse(&[field], &edit);
    }
    // The latticevine (`VineConfig::validate`).
    for field in [
        "transpiration_m3_per_s",
        "wilt_water",
        "full_water",
        "dry_water",
        "dry_spell_s",
        "wet_spell_s",
        "assimilation",
        "underside_light",
        "leaf_upkeep",
        "wood_upkeep",
        "regrow_rate",
        "sister_cost",
        "contest_threshold",
        "spur_reserve_min",
        "spur_window_s",
        "spur_jitter_s",
        "bud_s",
        "flower_s",
        "fruit_s",
        "spent_s",
        "nectar",
        "fruit",
        "energy_density",
        "founder_reserve",
    ] {
        let mut c = FloraConfig::default();
        let path = format!("flora.latticevine.{field}");
        registry()
            .set(&mut c, &path, toml::Value::Float(-1.0))
            .expect_err(&path);
    }
    for field in ["leaf_fall_s", "spread_check_s", "contest_check_s", "leaf_mass"] {
        let mut c = FloraConfig::default();
        let path = format!("flora.latticevine.{field}");
        registry()
            .set(&mut c, &path, toml::Value::Float(0.0))
            .expect_err(&path);
    }
    refuse(&["latticevine", "wet_water"], &|c| {
        c.latticevine.wet_water = c.latticevine.dry_water
    });
    refuse(&["latticevine", "spread_cost"], &|c| {
        c.latticevine.spread_cost = c.latticevine.runner_mass / 2.0
    });
    refuse(&["latticevine", "contest_threshold"], &|c| c.latticevine.contest_threshold = 1.5);

    // What the old validators allowed still passes: a frozen fungus.
    let mut frozen = FloraConfig::default();
    frozen.glowcap.substrate_uptake_per_s = 0.0;
    frozen.glowcap.substrate_yield = 0.0;
    frozen.validate().expect("a frozen fungus is a valid preset");
}

/// The hand-kept voxel rescale this package deleted, kept here as the oracle: finer grids
/// multiply each species' root box, mycelium reach and hop, and the latticevine's root
/// box, by `0.25 / voxel_m` (rounded, never below one), and the latticevine's per-face
/// amounts by the face's area against the reference face's.
fn hand_rescaled(voxel_m: f64) -> FloraConfig {
    const REFERENCE_VOXEL_M: f64 = 0.25;
    fn scaled(distance: u32, scale: f64) -> u32 {
        if distance == 0 {
            return 0;
        }
        (f64::from(distance) * scale).round().clamp(1.0, f64::from(u32::MAX)) as u32
    }
    let mut config = FloraConfig::default();
    if !voxel_m.is_finite() || !(voxel_m > 0.0) {
        return config;
    }
    config.voxel_m = voxel_m;
    if voxel_m >= REFERENCE_VOXEL_M {
        return config;
    }
    let scale = REFERENCE_VOXEL_M / voxel_m;
    for species in Species::ALL {
        let sc = config.species_mut(species);
        sc.rooting_depth = scaled(sc.rooting_depth, scale);
        sc.rooting_radius = scaled(sc.rooting_radius, scale);
        sc.substrate_reach_up_down = scaled(sc.substrate_reach_up_down, scale);
        sc.hop = scaled(sc.hop, scale);
    }
    let vc: &mut VineConfig = &mut config.latticevine;
    vc.rooting_depth = scaled(vc.rooting_depth, scale);
    vc.rooting_radius = scaled(vc.rooting_radius, scale);
    let k = 1.0 / (scale * scale);
    for v in [
        &mut vc.transpiration_m3_per_s,
        &mut vc.assimilation,
        &mut vc.leaf_mass,
        &mut vc.runner_mass,
        &mut vc.leaf_upkeep,
        &mut vc.wood_upkeep,
        &mut vc.spread_cost,
        &mut vc.sister_cost,
        &mut vc.spur_reserve_min,
        &mut vc.nectar,
        &mut vc.fruit,
        &mut vc.founder_reserve,
    ] {
        *v *= k;
    }
    config
}

/// **Test 6**: the table-driven rescale gives the hand list's numbers on every grid the
/// fixtures use (values compared with a tolerance, not bytes).
#[test]
fn the_tag_driven_rescale_matches_the_hand_list() {
    for voxel_m in [0.0625, 0.1, 0.125, 0.2, 0.25, 0.5, 1.0] {
        let table = spec::json_leaves("flora", &FloraConfig::for_voxel_size(voxel_m));
        let hand = spec::json_leaves("flora", &hand_rescaled(voxel_m));
        assert_eq!(table.len(), hand.len(), "{voxel_m} m");
        for ((pa, a), (pb, b)) in table.iter().zip(&hand) {
            assert_eq!(pa, pb);
            assert!(
                (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1e-300),
                "{voxel_m} m: {pa} = {a}, the hand list gives {b}"
            );
        }
        // And everything that is not a number is untouched.
        let mut t = FloraConfig::for_voxel_size(voxel_m);
        let h = hand_rescaled(voxel_m);
        t.voxel_m = h.voxel_m;
        for s in Species::ALL {
            assert_eq!(t.species(s).dispersal, h.species(s).dispersal);
            assert_eq!(t.species(s).trophic, h.species(s).trophic);
        }
    }
}

#[test]
fn every_scaled_row_is_one_the_hand_list_named() {
    // The tags are the hand list: four voxel distances per species, the vine's root box,
    // and its twelve per-face amounts. Nothing else carries a scale.
    let scaled: Vec<String> = registry()
        .rows()
        .iter()
        .filter(|r| r.scale != cubarium_voxel::registry::Scale::None)
        .map(|r| r.path.clone())
        .collect();
    assert_eq!(scaled.len(), 4 * Species::ALL.len() + 2 + 12, "{scaled:#?}");
}
