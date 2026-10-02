//! **Roster R1, written before the rule** (`design/handoffs/voxel-roster-2026-10-01.md`,
//! Brief R1 "Tests first"): a live capgnawer and a live loftstrider each draw with their
//! baked specimen model and coat (R0's fallback, no rebake), and the live presets found
//! them (desktop and voidrunner terrariums and the desktop voxel preset 6 and 2, the
//! Tachyon panel 4 and 1).

use std::path::{Path, PathBuf};

use cubarium_voxel::{Command as WorldCommand, Config as WorldConfig, Material, World};
use cubarium_voxel_fauna::{Command, Fauna, FaunaConfig, Founder, StartingStores};

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
