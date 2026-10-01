//! The animal layer's parameter registry (package R1): coverage, get/set round trip,
//! ranges, and the refusals the hand-written `SpeciesConfig` / `Reproduction` /
//! `FaunaConfig` validators made, now driven by the table and the cross-field rules.

#[path = "../../cubarium-voxel/tests/support/registry_spec.rs"]
mod spec;

use cubarium_voxel_fauna::{BirthMode, FaunaConfig, Founder, registry};

#[test]
fn every_fauna_number_has_a_row_and_every_row_a_number() {
    spec::coverage(registry(), &[&FaunaConfig::default(), &FaunaConfig::placeholder()]);
}

#[test]
fn get_and_set_round_trip_on_every_fauna_row() {
    spec::round_trip(registry(), &FaunaConfig::default());
}

#[test]
fn a_fauna_value_outside_its_range_is_refused_naming_path_and_range() {
    spec::range_refusals(registry(), &FaunaConfig::default());
}

#[test]
fn an_unknown_fauna_path_is_refused() {
    spec::unknown_paths_refused(registry(), &FaunaConfig::default());
}

#[test]
fn founders_and_foods_are_addressed_by_name() {
    let c = FaunaConfig::default();
    let by_name = registry()
        .get(&c, "fauna.founders.lanternjaw.core.maintenance_per_s")
        .expect("a lineage by name");
    let by_index = registry()
        .get(&c, "fauna.founders.2.core.maintenance_per_s")
        .expect("a lineage by index");
    assert_eq!(by_name, by_index);
    assert_eq!(
        by_name.as_float(),
        Some(c.founder(Founder::Lanternjaw).core.maintenance_per_s)
    );
    let foliage = registry()
        .get(&c, "fauna.founders.frondgrazer.yield_by_food.foliage")
        .expect("a food by name");
    assert_eq!(
        foliage.as_float(),
        Some(c.founder(Founder::Browser).yield_by_food[3])
    );
}

/// Every refusal the deleted hand validators made, through `FaunaConfig::validate`, each
/// with the words its message must carry.
#[test]
fn the_old_fauna_validator_refusals_still_refuse() {
    let refuse = |words: &[&str], edit: &dyn Fn(&mut FaunaConfig)| {
        let mut c = FaunaConfig::default();
        edit(&mut c);
        let e = c.validate().expect_err(words[0]);
        for w in words {
            assert!(e.contains(w), "{words:?}: {e}");
        }
    };
    // `SpeciesConfig::validate`: twelve finite, nonnegative numbers, on the placeholder
    // species and on every founder's core.
    type Edit = fn(&mut cubarium_voxel_fauna::SpeciesConfig, f64);
    let fields: [(&str, Edit); 12] = [
        ("maintenance_per_s", |s, v| s.maintenance_per_s = v),
        ("bite_per_s", |s, v| s.bite_per_s = v),
        ("bite_half_stock", |s, v| s.bite_half_stock = v),
        ("growth_max_per_s", |s, v| s.growth_max_per_s = v),
        ("n_tissue", |s, v| s.n_tissue = v),
        ("body_max", |s, v| s.body_max = v),
        ("body_min", |s, v| s.body_min = v),
        ("birth_body", |s, v| s.birth_body = v),
        ("birth_cost", |s, v| s.birth_cost = v),
        ("reserve_cap", |s, v| s.reserve_cap = v),
        ("drown_depth_m", |s, v| s.drown_depth_m = v),
        ("energy_density", |s, v| s.energy_density = v),
    ];
    for (field, edit) in fields {
        for bad in [-1.0, f64::NAN] {
            refuse(&["frondgrazer", field], &|c| edit(&mut c.frondgrazer, bad));
            for f in Founder::ALL {
                refuse(&[f.name(), field], &|c| edit(&mut c.founders[f.index()].core, bad));
            }
        }
    }
    refuse(&["frondgrazer", "body_min"], &|c| c.frondgrazer.body_min = 0.0);
    refuse(&["frondgrazer", "birth_body"], &|c| {
        c.frondgrazer.birth_body = c.frondgrazer.body_max * 2.0
    });
    refuse(&["frondgrazer", "body_min"], &|c| {
        c.frondgrazer.body_min = c.frondgrazer.birth_body * 2.0
    });
    refuse(&["littershredder", "birth_cost"], &|c| {
        let core = &mut c.founders[Founder::Blind.index()].core;
        core.birth_cost = core.body_min / 2.0;
    });
    // `Reproduction::validate`.
    for (field, edit) in [
        ("surplus_floor", (|r: &mut cubarium_voxel_fauna::Reproduction| r.surplus_floor = -1.0) as fn(&mut _)),
        ("surplus_hold_s", |r| r.surplus_hold_s = f64::NAN),
        ("gestation_s", |r| r.gestation_s = -1.0),
        ("gestation_loss_fraction", |r| r.gestation_loss_fraction = -0.1),
        ("gestation_loss_fraction", |r| r.gestation_loss_fraction = 1.5),
        ("birth_interval_s", |r| r.birth_interval_s = -1.0),
        ("incubation_s", |r| r.incubation_s = f64::INFINITY),
    ] {
        refuse(&["frondgrazer", field], &|c| edit(&mut c.frondgrazer.reproduction));
    }
    refuse(&["littershredder", "clutch_size"], &|c| {
        let r = &mut c.founders[Founder::Blind.index()].core.reproduction;
        assert_eq!(r.mode, BirthMode::Eggs);
        r.clutch_size = 0;
    });
    refuse(&["lanternjaw", "egg_organic"], &|c| {
        let core = &mut c.founders[Founder::Lanternjaw.index()].core;
        core.reproduction.egg_organic = core.body_min / 2.0;
    });
    // A gestating body reads neither: a zero clutch is not refused there.
    let mut c = FaunaConfig::default();
    c.frondgrazer.reproduction.clutch_size = 0;
    c.validate().expect("a live-bearer's clutch is unread");
    // `FaunaConfig::validate`'s own.
    refuse(&["parasite_attach_radius_m"], &|c| c.parasite_attach_radius_m = -0.1);
    refuse(&["parasite_attach_radius_m"], &|c| c.parasite_attach_radius_m = f64::NAN);
    refuse(&["parasite_drain_of_host_basal"], &|c| c.parasite_drain_of_host_basal = -0.01);
    refuse(&["parasite_drain_of_host_basal"], &|c| {
        c.parasite_drain_of_host_basal = f64::INFINITY
    });
    refuse(&["parasite_detach_body_fraction"], &|c| c.parasite_detach_body_fraction = 0.0);
    refuse(&["parasite_detach_body_fraction"], &|c| c.parasite_detach_body_fraction = 1.5);
    refuse(&["hatch_chance_per_dusk"], &|c| c.hatch_chance_per_dusk = 1.5);
    refuse(&["hatch_chance_per_dusk"], &|c| c.hatch_chance_per_dusk = -0.1);
    for f in Founder::ALL {
        let i = f.index();
        let n = f.name();
        refuse(&[n, "motor_respiration_per_s"], &|c| c.founders[i].motor_respiration_per_s = -1.0);
        refuse(&[n, "organ_structure_fraction"], &|c| c.founders[i].organ_structure_fraction = 1.0);
        refuse(&[n, "organ_structure_fraction"], &|c| c.founders[i].organ_structure_fraction = -0.1);
        refuse(&[n, "sleep_upkeep"], &|c| c.founders[i].sleep_upkeep = 1.5);
        refuse(&[n, "wake_hunger"], &|c| c.founders[i].wake_hunger = -0.1);
        refuse(&[n, "armour"], &|c| c.founders[i].armour = 1.5);
        refuse(&[n, "agility"], &|c| c.founders[i].agility = f64::NAN);
        refuse(&[n, "mass_scale"], &|c| c.founders[i].mass_scale = 0.0);
        refuse(&[n, "adult_length_m"], &|c| c.founders[i].adult_length_m = 0.0);
        refuse(&[n, "adult_width_m"], &|c| c.founders[i].adult_width_m = -1.0);
        refuse(&[n, "adult_height_m"], &|c| c.founders[i].adult_height_m = f64::NAN);
        refuse(&[n, "eye_height_fraction"], &|c| c.founders[i].eye_height_fraction = -0.1);
        refuse(&[n, "mouth_ceiling_fraction"], &|c| {
            c.founders[i].mouth_ceiling_fraction = f64::NAN
        });
        refuse(&[n, "contact_height_fraction"], &|c| {
            c.founders[i].contact_height_fraction = -1.0
        });
        refuse(&[n, "mouth_reach_length_fraction"], &|c| {
            c.founders[i].mouth_reach_length_fraction = -1.0
        });
    }
}

// ------------------------------------------------------------------ R2-fauna: moved numbers

/// The default config's moved numbers are the values the code ran on before they became
/// knobs, and a default config's live manifest is the founder's fixed one.
#[test]
fn the_moved_numbers_default_to_what_the_code_ran_on() {
    use cubarium_voxel_fauna::{LifeConfig, PredationConfig, TeacherConfig};
    let c = FaunaConfig::default();
    let p = PredationConfig::default();
    assert_eq!((p.still_after_s, p.still_speed_bl_per_s, p.tremor_radius_m), (2.0, 0.1, 1.5));
    assert_eq!((p.footfall_radius_m, p.hatch_suppress_m, p.footfall_tremor_min), (2.0, 4.0, 5.0e-4));
    assert_eq!((p.egg_footfall_min, p.egg_footfall_max, p.egg_viability_median_s), (2, 10, 43_200.0));
    assert_eq!(p.wound_heal_per_s, 0.5 / 1800.0);
    assert_eq!((p.heal_cost, p.egg_upkeep_per_s, p.alert_s), (1.0, 5.0e-6, 60.0));
    assert_eq!(LifeConfig::default().day_line, 0.3);
    assert_eq!(TeacherConfig::default().flat_clear, 9.0 / 15.0);
    for f in Founder::ALL {
        let live = c.manifest(f);
        let fixed = f.manifest();
        assert_eq!(live.digest(), fixed.digest(), "{f:?}");
        assert_eq!(live.cruise_m_per_s, fixed.cruise_m_per_s);
        assert_eq!(live.forward_reference_m, fixed.forward_reference_m);
    }
}

/// Pace is a knob: moving the cruise moves the live manifest (and so its digest).
#[test]
fn a_moved_cruise_reaches_the_live_manifest() {
    let mut c = FaunaConfig::default();
    let before = c.manifest(Founder::Browser);
    c.founders[Founder::Browser.index()].cruise_body_lengths_per_s = 2.0;
    let after = c.manifest(Founder::Browser);
    assert_eq!(after.cruise_m_per_s, 2.0 * c.founder(Founder::Browser).adult_length_m);
    assert_ne!(before.digest(), after.digest());
}

/// A draw's ends the wrong way round is refused by the cross rules.
#[test]
fn inverted_draw_ranges_are_refused() {
    let refuse = |what: &str, edit: &dyn Fn(&mut FaunaConfig)| {
        let mut c = FaunaConfig::default();
        edit(&mut c);
        let e = c.validate().expect_err(what);
        assert!(e.contains(what), "{what}: {e}");
    };
    refuse("egg_footfall_min", &|c| c.predation.egg_footfall_min = 11);
    refuse("twilight_lo", &|c| c.life.twilight_lo = 0.99);
    refuse("wander_hold_min", &|c| c.teachers.wander_hold_min = 25);
    refuse("wander_turn_samples_min", &|c| c.teachers.wander_turn_samples_min = 5);
}

/// The egg draw reads its knobs: a footfall threshold inside `[min, max]`, a viability
/// inside `median / spread ..= median x spread`.
#[test]
fn the_egg_draw_follows_its_knobs() {
    let mut p = cubarium_voxel_fauna::PredationConfig::default();
    p.egg_footfall_min = 7;
    p.egg_footfall_max = 9;
    p.egg_viability_median_s = 1000.0;
    p.egg_viability_spread = 1.5;
    for serial in 0..200 {
        let d = cubarium_voxel_fauna::Dormancy::draw(&p, 1, serial, 0);
        assert!((7..=9).contains(&d.threshold), "{}", d.threshold);
        let secs = d.viable_until_tick as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ);
        assert!((1000.0 / 1.5 - 0.1..=1000.0 * 1.5 + 0.1).contains(&secs), "{secs}");
    }
}
