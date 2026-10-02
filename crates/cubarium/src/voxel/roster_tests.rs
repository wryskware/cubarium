//! **Roster R1, written before the rule** (`design/handoffs/voxel-roster-2026-10-01.md`,
//! Brief R1 "Tests first"): a live capgnawer and a live loftstrider each draw with their
//! baked specimen model and coat (R0's fallback, no rebake), and the live presets found
//! them (desktop and voidrunner terrariums and the desktop voxel preset 6 and 2, the
//! Tachyon panel 4 and 1).
//!
//! **Roster R2b** (Brief R2b "Tests first"): a live ripple-snail draws with its specimen
//! model, the live presets seed glassfilm at founding and found snails on it (8 on the
//! desktop and voidrunner terrariums and the desktop voxel preset, 6 on the Tachyon
//! panel), and a film site emits a floor mark whose strength follows its stock.

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

/// The live presets found the two new lineages; the code default founds neither.
#[test]
fn the_live_presets_found_capgnawers_and_loftstriders() {
    let code = FaunaConfig::default().founder_counts();
    assert_eq!(
        (code[Founder::Capgnawer.index()], code[Founder::Loftstrider.index()]),
        (0, 0),
        "training worlds are unchanged"
    );
    for (file, want) in [
        ("config/desktop/terrarium.toml", (6, 2)),
        ("config/voidrunner/terrarium.toml", (6, 2)),
        ("config/desktop/voxel.toml", (6, 2)),
        ("config/tachyon/voxel.toml", (4, 1)),
    ] {
        let path = repo(file).canonicalize().expect("the preset is committed");
        let cfg = super::load_config(&path).unwrap_or_else(|e| panic!("{file}: {e:#}"));
        let params = super::params_of(&cfg, &[]).unwrap_or_else(|e| panic!("{file}: {e:#}"));
        let counts = params.fauna.founder_counts();
        assert_eq!(
            (counts[Founder::Capgnawer.index()], counts[Founder::Loftstrider.index()]),
            want,
            "{file}: capgnawers and loftstriders"
        );
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

/// The live presets seed glassfilm at founding and found ripple-snails on it; the code
/// defaults (co-training worlds) do neither.
#[test]
fn the_live_presets_seed_film_and_found_ripple_snails() {
    let snail = Founder::RippleSnail.index();
    assert_eq!(FaunaConfig::default().founder_counts()[snail], 0, "training worlds are unchanged");
    assert!(!FloraConfig::default().seed_film_at_founding, "and seed no film");
    for (file, want) in [
        ("config/desktop/terrarium.toml", 8),
        ("config/voidrunner/terrarium.toml", 8),
        ("config/desktop/voxel.toml", 8),
        ("config/tachyon/voxel.toml", 6),
    ] {
        let path = repo(file).canonicalize().expect("the preset is committed");
        let cfg = super::load_config(&path).unwrap_or_else(|e| panic!("{file}: {e:#}"));
        let params = super::params_of(&cfg, &[]).unwrap_or_else(|e| panic!("{file}: {e:#}"));
        assert!(params.flora.seed_film_at_founding, "{file}: seeds film at founding");
        assert_eq!(params.fauna.founder_counts()[snail], want, "{file}: ripple-snails");
    }
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
    let mut flora = Flora::new(FloraConfig::default());
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
