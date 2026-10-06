//! **The census file** (`flora::census::write_census`, `load`): a census written to the
//! plain file reads back field for field, and `load` prefers the file beside a save over
//! the save itself, picking the one read under the nearest plant config.

use std::path::{Path, PathBuf};

use cubarium_voxel_flora::Species;
use cubarium_webmodel::flora::census::{self, Census, Face, StandRec};
use cubarium_webmodel::flora::config;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wm-census-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// Faces that exercise every field: awkward floats, `None`s, a plant, a fungus, seeds and
/// sparse optical depths.
fn awkward() -> Vec<Face> {
    let mut a = Face::bare(3, 17, 9, 0.1 + 0.2, 1.0 / 3.0);
    a.soil = false;
    a.soil_depth = 2;
    a.highest = false;
    a.water_ok = [true, false, true, true, false, false, true, false];
    a.light_ok = [false, true, false, false, true, true, false, true];
    a.turf_water = None;
    a.turf_saturated = 1e-17;
    a.pad_saturated = 0.999_999_999_999;
    a.depth_m = 7.25e-3;
    a.darkness = std::f64::consts::PI;
    a.glow_ok = false;
    a.canopy = (-0.731_f64).exp();
    a.plant = Some((Species::Lanternberry, StandRec { wood: 0.123_456_789_012_345_67, foliage: 2.0 / 7.0, reserve: 1e-9, light: 0.61, moisture: 0.3, aeration_stress: 0.01, cured: true, edible: 0.05, mineral: 0.002 }));
    a.fungus = Some(StandRec { wood: 0.4, foliage: 0.0, reserve: 0.25, ..Default::default() });
    a.litter = 12.5;
    a.dead_wood = 1e300;
    a.mineral = 0.92;
    a.film = 0.0;
    a.litter_mineral = 0.1;
    a.dead_wood_mineral = 0.2;
    a.seed_mineral = 0.3;
    a.water = [Some(0.25), None, Some(1.0 / 3.0), None, Some(0.0), Some(1.0), None, Some(0.7)];
    a.saturated = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7];
    a.seeds = [0, 1, 2, 3, 4, 5, 6, 7, 400];
    a.nb_tau[0][6] = 0.333_333_34;
    a.nb_tau[7][2] = 1e-7;
    a.pl_tau[5] = 0.5;
    let b = Face::bare(0, 0, 0, 1.0, 0.5);
    vec![a, b]
}

#[test]
fn a_census_file_reads_back_field_for_field() {
    let faces = awkward();
    for f in &faces {
        let back = census::face_of_row(&census::face_row(f)).expect("the row reads");
        assert_eq!(&back, f);
    }
    let dir = scratch("roundtrip");
    let c = Census { width: 256, depth: 128, minute: 370.0, faces: faces.clone() };
    census::write_census(&dir, &c, "x = 1\n", "test").unwrap();
    let r = census::read_census(&dir).unwrap();
    assert_eq!((r.width, r.depth, r.minute), (256, 128, 370.0));
    assert_eq!(r.faces, faces);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_bad_row_is_refused() {
    let row = census::face_row(&awkward()[0]);
    assert!(census::face_of_row(&format!("{row}\textra")).is_err());
    let cut: Vec<&str> = row.split('\t').collect();
    assert!(census::face_of_row(&cut[..cut.len() - 1].join("\t")).is_err());
}

#[test]
fn load_takes_the_census_beside_a_save_nearest_the_run_config() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/scenarios/terrarium.toml");
    let fc = config::flora_of(&config::resolve(&root, &[]).unwrap());
    let want = toml::to_string(&fc).unwrap();
    let mut other = fc.clone();
    other.initial_mineral += 0.5;
    other.decomposition *= 2.0;
    let save = scratch("save");
    let near = Census { width: 4, depth: 4, minute: 10.0, faces: vec![Face::bare(1, 1, 1, 0.5, 0.5)] };
    let far = Census { width: 4, depth: 4, minute: 20.0, faces: vec![Face::bare(2, 2, 2, 0.5, 0.5)] };
    census::write_census(&save.join("census-a"), &far, &toml::to_string(&other).unwrap(), "far").unwrap();
    census::write_census(&save.join("census-b"), &near, &want, "near").unwrap();
    let got = census::load(&save, &fc).unwrap();
    assert_eq!(got.minute, 10.0);
    assert_eq!(got.faces, near.faces);
    // The differences are reported by key.
    let d = census::config_differences(&toml::to_string(&other).unwrap(), &want).unwrap();
    assert!(d.contains(&"initial_mineral".to_string()) && d.contains(&"decomposition".to_string()), "{d:?}");
    // A directory with neither a census nor a save is an error, not a guess.
    assert!(census::load(&scratch("empty"), &fc).is_err());
    let _ = std::fs::remove_dir_all(&save);
}
