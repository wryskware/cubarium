//! **Roster R1, written before the rule** (`design/handoffs/voxel-roster-2026-10-01.md`,
//! Brief R1 "Tests first"): a live capgnawer and a live loftstrider each draw with their
//! baked specimen model and coat (R0's fallback, no rebake).
//!
//! **Roster R2b** (Brief R2b "Tests first"): a live ripple-snail draws with its specimen
//! model, and a film site emits a floor mark whose strength follows its stock.
//!
//! **F1** (the flight brief with the roster brief's "F1 overrides"): a live bellwing draws
//! with its specimen model at its `y_m`.
//!
//! **F3** (the roster brief's Brief F3 "Tests first"): a live seedporter draws with its
//! specimen model at its `y_m`.
//!
//! **R3** (Brief R3 "Tests first"): a live chorister draws with its specimen model, a pup at
//! its body scale.
//!
//! How many of each lineage a preset founds is the preset's business, not a test's: the
//! only preset check here is that every live preset loads and resolves.

use std::path::{Path, PathBuf};

use cubarium_voxel::{Command as WorldCommand, Config as WorldConfig, Material, World};
use cubarium_voxel_fauna::{Command, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

use super::animal::{AnimalPart, Animals};
use super::colours;
use super::model::ModelLibrary;

const NEW: [Founder; 2] = [Founder::Capgnawer, Founder::Loftstrider];

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)
}

/// A flat plain at 0.125 m with room for a 2.2 m-wide, 3.4 m-tall body.
fn plain() -> World {
    let c = WorldConfig {
        width: 48,
        height: 40,
        depth: 32,
        voxel_m: 0.125,
        ..WorldConfig::default()
    };
    let mut w = World::empty(c.clone());
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
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

/// Every live preset loads and resolves to a parameter set the run can build.
#[test]
fn every_live_preset_loads_and_resolves() {
    for file in [
        "config/desktop/terrarium.toml",
        "config/voidrunner/terrarium.toml",
        "config/desktop/voxel.toml",
        "config/tachyon/voxel.toml",
    ] {
        let path = repo(file).canonicalize().expect("the preset is committed");
        let cfg = super::load_config(&path).unwrap_or_else(|e| panic!("{file}: {e:#}"));
        super::params_of(&cfg, &[]).unwrap_or_else(|e| panic!("{file}: {e:#}"));
    }
}

/// The baked library a live 0.125 m world loads (`models_dir`'s default).
#[test]
fn a_live_capgnawer_and_a_live_loftstrider_draw_with_their_specimen_models() {
    let lib = ModelLibrary::load(&repo("assets/voxel-models"), 0.125).expect("the baked library");
    for f in NEW {
        let specimen = lib.specimen(f.name()).expect("the baked specimen of its name");
        let drawn = lib.animal(f).expect("a live body has a model");
        assert!(std::ptr::eq(drawn, specimen), "{f:?} draws with its specimen");
        let coat = colours::founder_coat(f);
        let want = colours::specimen_coat(f.name()).expect("the specimen's coat");
        assert!(std::ptr::eq(coat, want), "{f:?} wears its specimen coat");
    }

    let world = plain();
    let view = world.view();
    for f in NEW {
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(fauna.apply(
            &world,
            Command::IntroduceFounder {
                x: 24,
                z: 16,
                founder: f,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            },
        ));
        let mut animals = Animals::empty(0, 0, 0);
        animals.rebuild_with(&view, Some(fauna.view()), &lib);
        let model_cells = animals
            .cells()
            .filter(|&(x, y, z)| {
                matches!(animals.at(i64::from(x), i64::from(y), z), AnimalPart::Model(_))
            })
            .count();
        assert!(model_cells > 0, "a live {f:?} is stamped from its model, not a glyph");
    }
}

// ------------------------------------------------------------------ roster R2b

/// A live ripple-snail draws with its baked specimen model and coat (R0's fallback, no
/// rebake), stamped from the model and not a glyph.
#[test]
fn a_live_ripple_snail_draws_with_its_specimen_model() {
    let f = Founder::RippleSnail;
    assert_eq!(f.name(), "ripple-snail", "the specimen key");
    let lib = ModelLibrary::load(&repo("assets/voxel-models"), 0.125).expect("the baked library");
    let specimen = lib.specimen(f.name()).expect("the baked specimen of its name");
    let drawn = lib.animal(f).expect("a live body has a model");
    assert!(std::ptr::eq(drawn, specimen), "the snail draws with its specimen");
    let coat = colours::founder_coat(f);
    let want = colours::specimen_coat(f.name()).expect("the specimen's coat");
    assert!(std::ptr::eq(coat, want), "and wears its specimen coat");

    let world = plain();
    let view = world.view();
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        Command::IntroduceFounder {
            x: 24,
            z: 16,
            founder: f,
            stores: StartingStores::FULL,
            heading_rad: 0.0,
        },
    ));
    let mut animals = Animals::empty(0, 0, 0);
    animals.rebuild_with(&view, Some(fauna.view()), &lib);
    let model_cells = animals
        .cells()
        .filter(|&(x, y, z)| {
            matches!(animals.at(i64::from(x), i64::from(y), z), AnimalPart::Model(_))
        })
        .count();
    assert!(model_cells > 0, "a live snail is stamped from its model, not a glyph");
}

/// A 0.25 m soil plain (soil `y = 1..=2`, the support face `y = 2`) with one cell of
/// water standing over columns `2..=5` on every `z`: film-eligible faces there.
fn shallows() -> World {
    let c = WorldConfig {
        width: 16,
        height: 8,
        depth: 4,
        voxel_m: 0.25,
        ..WorldConfig::default()
    };
    let mut w = World::empty(c.clone());
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
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
    let cell = w.config().voxel_volume();
    for z in 0..c.depth {
        for x in 2..=5 {
            w.apply(WorldCommand::AddWater {
                x,
                y: 3,
                z,
                volume_m3: cell,
            });
        }
    }
    w
}

/// **A film site emits a floor mark whose strength follows its stock.** Film seeded on the
/// shallows, then grazed to half on one face and to nothing on another: every film site
/// has one mark, its strength the site's film over its capacity; the half-grazed face's
/// mark is half as strong; a bare face and a dry face have none. The stand presenter puts
/// a floor-mark part ([`cubarium_gpu::voxel::PART_FLOOR_MARK`]) in the cell over a film
/// face, and none over a dry one.
#[test]
fn a_film_site_emits_a_floor_mark_whose_strength_follows_its_stock() {
    let world = shallows();
    // No grazing refuge: this case grazes a face bare.
    let mut fc = FloraConfig::default();
    fc.film_graze_refuge = 0.0;
    let mut flora = Flora::new(fc);
    assert!(flora.seed_film(&world) > 0, "film on the shallows");
    let (full, half, bare, dry) = (
        Site { x: 2, y: 2, z: 1 },
        Site { x: 3, y: 2, z: 1 },
        Site { x: 4, y: 2, z: 1 },
        Site { x: 9, y: 2, z: 1 },
    );
    let seeded = flora.view().film_at(full);
    assert!(seeded > 0.0 && flora.view().film_at(half) == seeded);
    assert!(flora.take_film(half, 0.5 * seeded).is_some(), "graze half of it");
    assert!(flora.take_film(bare, 10.0 * seeded).is_some(), "graze it bare");
    assert_eq!(flora.view().film_at(bare), 0.0);
    assert_eq!(flora.view().film_at(dry), 0.0);

    let c = flora.config();
    let capacity = c.film_capacity_per_m2 * c.voxel_m * c.voxel_m;
    let marks = super::stand::film_marks(flora.view());
    let sites: Vec<Site> = flora.view().film_sites().map(|(s, _)| s).collect();
    assert_eq!(
        marks.iter().map(|m| m.site).collect::<Vec<_>>(),
        sites,
        "one mark per film site, in site order"
    );
    for m in &marks {
        let want = (flora.view().film_at(m.site) / capacity).clamp(0.0, 1.0) as f32;
        assert!(
            (m.strength - want).abs() <= 1e-6,
            "the mark at {:?} is {} for a stock of {want} of capacity",
            m.site,
            m.strength
        );
    }
    let strength = |s: Site| marks.iter().find(|m| m.site == s).map(|m| m.strength);
    let (f, h) = (strength(full).expect("a full mark"), strength(half).expect("a grazed mark"));
    assert!(f > 0.0 && (h - 0.5 * f).abs() <= 1e-6 * f, "the grazed face shows half the tint: {h} of {f}");
    assert_eq!(strength(bare), None, "a grazed-bare face has no mark");
    assert_eq!(strength(dry), None, "nor a dry one");

    let view = world.view();
    let mut stands = super::stand::Stands::empty(16, 8, 4);
    stands.rebuild(&view, flora.view());
    let class = |s: Site| {
        super::appearance::plant_class(stands.at(i64::from(s.x), i64::from(s.y) + 1, s.z))
    };
    assert_eq!(class(full), cubarium_gpu::voxel::PART_FLOOR_MARK, "a film face is marked");
    assert_eq!(class(half), cubarium_gpu::voxel::PART_FLOOR_MARK);
    assert_eq!(class(dry), cubarium_gpu::voxel::PART_NONE, "a dry face is not");
}

// ------------------------------------------------------------------ F1: the bellwing

/// F1 (overrides, presenter): **a live bellwing draws with its baked specimen model, at
/// `y_m`.** One pose, as for every animal (the vane beat and the folded perch pose are
/// owed and need more baked poses). A bellwing put 1 m up over the 0.125 m plain (its base
/// at 1.375 m, cell 11) is stamped from its model, not a glyph, and the model's lowest
/// cell is the cell its base is in (±1), well clear of the ground's air cell 3.
#[test]
fn a_live_bellwing_draws_with_its_specimen_model_at_its_height() {
    let f = Founder::Bellwing;
    assert_eq!(f.name(), "bellwing", "the specimen key");
    let lib = ModelLibrary::load(&repo("assets/voxel-models"), 0.125).expect("the baked library");
    let specimen = lib.specimen(f.name()).expect("the baked specimen of its name");
    let drawn = lib.animal(f).expect("a live body has a model");
    assert!(std::ptr::eq(drawn, specimen), "the bellwing draws with its specimen");
    let coat = colours::founder_coat(f);
    let want = colours::specimen_coat(f.name()).expect("the specimen's coat");
    assert!(std::ptr::eq(coat, want), "and wears its specimen coat");

    let world = plain();
    let view = world.view();
    let v = 0.125;
    let y_m = 3.0 * v + 1.0;
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(
        fauna.apply(
            &world,
            Command::IntroduceAloft {
                pose: cubarium_voxel_fauna::Pose {
                    x: 24.5 * v,
                    z: 16.5 * v,
                    heading_rad: 0.0,
                    y_m,
                },
                founder: f,
                stores: StartingStores::FULL,
            },
        ),
        "a bellwing is put in the air"
    );
    let mut animals = Animals::empty(0, 0, 0);
    animals.rebuild_with(&view, Some(fauna.view()), &lib);
    let rows: Vec<u32> = animals
        .cells()
        .filter(|&(x, y, z)| {
            matches!(animals.at(i64::from(x), i64::from(y), z), AnimalPart::Model(_))
        })
        .map(|(_, y, _)| y)
        .collect();
    assert!(!rows.is_empty(), "a live bellwing is stamped from its model, not a glyph");
    let lowest = *rows.iter().min().expect("cells");
    let base = (y_m / v).floor() as u32;
    assert!(
        lowest.abs_diff(base) <= 1,
        "drawn at its height: lowest model cell {lowest}, its base in cell {base}"
    );
}

// ------------------------------------------------------------------ F3: the seedporter

/// F3 (Brief F3, "Presenter"): **a live seedporter draws with its baked specimen model, at
/// `y_m`, one pose.** A seedporter put gliding 1.5 m over the 0.125 m plain (its base at
/// 1.875 m, cell 15) is stamped from its model, not a glyph, wearing its specimen coat,
/// and the model's lowest cell is the cell its base is in (±1), well clear of the ground's
/// air cell 3.
#[test]
fn a_live_seedporter_draws_with_its_specimen_model_at_its_height() {
    let f = Founder::Seedporter;
    assert_eq!(f.name(), "seedporter", "the specimen key");
    let lib = ModelLibrary::load(&repo("assets/voxel-models"), 0.125).expect("the baked library");
    let specimen = lib.specimen(f.name()).expect("the baked specimen of its name");
    let drawn = lib.animal(f).expect("a live body has a model");
    assert!(std::ptr::eq(drawn, specimen), "the seedporter draws with its specimen");
    let coat = colours::founder_coat(f);
    let want = colours::specimen_coat(f.name()).expect("the specimen's coat");
    assert!(std::ptr::eq(coat, want), "and wears its specimen coat");

    let world = plain();
    let view = world.view();
    let v = 0.125;
    let y_m = 3.0 * v + 1.5;
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(
        fauna.apply(
            &world,
            Command::IntroduceAloft {
                pose: cubarium_voxel_fauna::Pose {
                    x: 24.5 * v,
                    z: 16.5 * v,
                    heading_rad: 0.0,
                    y_m,
                },
                founder: f,
                stores: StartingStores::FULL,
            },
        ),
        "a seedporter is put in the air, gliding"
    );
    let mut animals = Animals::empty(0, 0, 0);
    animals.rebuild_with(&view, Some(fauna.view()), &lib);
    let rows: Vec<u32> = animals
        .cells()
        .filter(|&(x, y, z)| {
            matches!(animals.at(i64::from(x), i64::from(y), z), AnimalPart::Model(_))
        })
        .map(|(_, y, _)| y)
        .collect();
    assert!(!rows.is_empty(), "a live seedporter is stamped from its model, not a glyph");
    let lowest = *rows.iter().min().expect("cells");
    let base = (y_m / v).floor() as u32;
    assert!(
        lowest.abs_diff(base) <= 1,
        "drawn at its height: lowest model cell {lowest}, its base in cell {base}"
    );
}

/// F3 (Fable's addition, the seedporter's poses): **each mode draws its own model** — a
/// climbing seedporter its `seedporter` model, a perched one `seedporter-stand`, a walking
/// one `seedporter-ground`, a gliding one `seedporter-glide` — four different models, and a
/// mode with no pose of its own (`Fly`) draws `seedporter`.
#[test]
fn a_live_seedporter_draws_each_mode_with_its_own_model() {
    use cubarium_voxel_fauna::MobilityMode;
    let f = Founder::Seedporter;
    let lib = ModelLibrary::load(&repo("assets/voxel-models"), 0.125).expect("the baked library");
    let world = plain();
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        Command::IntroduceFounder {
            x: 24,
            z: 16,
            founder: f,
            stores: StartingStores::FULL,
            heading_rad: 0.0,
        },
    ));
    let body = fauna.view().animals[0];
    let config = fauna.config().clone();
    let length = config.founder(f).body_at(body.body).length_m;
    let mut seen = Vec::new();
    for (mode, name) in [
        (MobilityMode::Climb, "seedporter"),
        (MobilityMode::Perched, "seedporter-stand"),
        (MobilityMode::Ground, "seedporter-ground"),
        (MobilityMode::Glide, "seedporter-glide"),
        (MobilityMode::Fly, "seedporter"),
    ] {
        let mut a = body;
        a.mobility.mode = mode;
        let drawn = super::animal::model_cells(&lib, &config, &a).expect("a model");
        let want = lib
            .specimen(name)
            .unwrap_or_else(|| panic!("the baked {name}"))
            .select(length)
            .expect("an adult bin");
        assert!(std::ptr::eq(drawn, want), "{mode:?} draws {name}");
        seen.push(drawn.as_ptr());
    }
    seen.truncate(4);
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 4, "four poses, four models");
}

// ------------------------------------------------------------------ R3: the chorister

/// R3 (Brief R3, "Presenter"): **a live chorister draws with its baked specimen model and
/// coat** (R0's fallback, one pose), stamped from the model and not a glyph, and **a pup
/// is drawn at its body scale**: a body at a tenth of `body_max` selects a shorter size bin
/// than an adult and stamps fewer cells.
#[test]
fn a_live_chorister_draws_with_its_specimen_model_and_a_pup_at_its_body_scale() {
    let f = Founder::Chorister;
    assert_eq!(f.name(), "chorister", "the specimen key");
    let lib = ModelLibrary::load(&repo("assets/voxel-models"), 0.125).expect("the baked library");
    let specimen = lib.specimen(f.name()).expect("the baked specimen of its name");
    let drawn = lib.animal(f).expect("a live body has a model");
    assert!(std::ptr::eq(drawn, specimen), "the chorister draws with its specimen");
    let coat = colours::founder_coat(f);
    let want = colours::specimen_coat(f.name()).expect("the specimen's coat");
    assert!(std::ptr::eq(coat, want), "and wears its specimen coat");

    let world = plain();
    let view = world.view();
    let mut fauna = Fauna::new(FaunaConfig::default());
    for (x, body) in [(12, 1.0), (36, 0.1)] {
        assert!(fauna.apply(
            &world,
            Command::IntroduceFounder {
                x,
                z: 16,
                founder: f,
                stores: StartingStores { body, reserve: 0.5 },
                heading_rad: 0.0,
            },
        ));
    }
    let mut animals = Animals::empty(0, 0, 0);
    animals.rebuild_with(&view, Some(fauna.view()), &lib);
    let model_cells = animals
        .cells()
        .filter(|&(x, y, z)| {
            matches!(animals.at(i64::from(x), i64::from(y), z), AnimalPart::Model(_))
        })
        .count();
    assert!(model_cells > 0, "a live chorister is stamped from its model, not a glyph");

    let config = fauna.config().clone();
    let [adult, pup] = [fauna.view().animals[0], fauna.view().animals[1]];
    let adult_cells = super::animal::model_cells(&lib, &config, &adult).expect("a model");
    let pup_cells = super::animal::model_cells(&lib, &config, &pup).expect("a model");
    assert!(
        pup_cells.len() < adult_cells.len(),
        "the pup is drawn smaller: {} cells against the adult's {}",
        pup_cells.len(),
        adult_cells.len()
    );
}
