//! The resolved parameter set (package R1, `design/ecology-parameter-registry-2026-09-28.md`
//! §2): layers in order — code defaults, the preset or world TOML, the search genome,
//! `--set path=value` — an unknown path refused by name, the defaults unchanged, the dump,
//! and the set a run writes beside its output.

use cubarium_voxel::Config;
use cubarium_voxel_fauna::{FaunaConfig, Founder};
use cubarium_voxel_flora::FloraConfig;
use cubarium_voxel_sim::params::{Layers, PARAMS_FILE, Params};

fn doc(text: &str) -> toml::Table {
    text.parse().expect("test TOML")
}

fn browser_upkeep(p: &Params) -> f64 {
    p.fauna.founder(Founder::Browser).core.maintenance_per_s
}

/// **Test 3**: an unknown path in the TOML layer and in `--set` is an error naming the
/// path.
#[test]
fn an_unknown_path_is_refused_by_name_in_every_layer() {
    for (text, path) in [
        ("[fauna]\nbogus = 1.0\n", "fauna.bogus"),
        (
            "[fauna.founders.frondgrazer.core]\nmaintenence_per_s = 0.002\n",
            "fauna.founders.frondgrazer.core.maintenence_per_s",
        ),
        ("[flora.springturf]\nnope = 1\n", "flora.springturf.nope"),
        ("[world]\nbogus = 1\n", "world.bogus"),
        ("[world.climate]\nnot_a_key = 2.0\n", "world.climate.not_a_key"),
    ] {
        let e = Params::resolve(&Layers {
            file: Some(doc(text)),
            ..Layers::default()
        })
        .expect_err(text);
        assert!(e.contains(path), "{path}: {e}");
    }
    for set in [
        "fauna.founders.frondgrazer.core.nope=1",
        "flora.springturf.nope=1",
        "world.nope=1",
        "fauna.founders.nobody.core.maintenance_per_s=1",
        "nothing.at.all=1",
    ] {
        let e = Params::resolve(&Layers {
            sets: vec![set.to_string()],
            ..Layers::default()
        })
        .expect_err(set);
        let path = set.split('=').next().unwrap();
        assert!(e.contains(path), "{set}: {e}");
    }
}

/// **Test 4**: a `--set` beats the genome, which beats the world TOML, which beats the
/// defaults — on an animal number and on a world number.
#[test]
fn later_layers_win() {
    let file = doc(
        "[fauna.founders.frondgrazer.core]\nmaintenance_per_s = 0.002\n\
         [world.climate]\ndry_gap_min_s = 100.0\n",
    );
    let shipped = Params::defaults();
    let none = Params::resolve(&Layers::default()).expect("defaults");
    assert_eq!(browser_upkeep(&none), browser_upkeep(&shipped));

    let toml_only = Params::resolve(&Layers {
        file: Some(file.clone()),
        ..Layers::default()
    })
    .expect("the TOML layer");
    assert_eq!(browser_upkeep(&toml_only), 0.002);
    assert_eq!(toml_only.world.climate.dry_gap_min_s, 100.0);

    let double = |p: &mut Params| {
        p.fauna.founders[Founder::Browser.index()].core.maintenance_per_s *= 2.0;
    };
    let genome = Params::resolve(&Layers {
        file: Some(file.clone()),
        genome: Some(&double),
        ..Layers::default()
    })
    .expect("the genome layer");
    assert_eq!(browser_upkeep(&genome), 0.004);

    let set = Params::resolve(&Layers {
        file: Some(file),
        genome: Some(&double),
        sets: vec![
            "fauna.founders.frondgrazer.core.maintenance_per_s=0.003".into(),
            "world.climate.dry_gap_min_s=200".into(),
        ],
    })
    .expect("the --set layer");
    assert_eq!(browser_upkeep(&set), 0.003);
    assert_eq!(set.world.climate.dry_gap_min_s, 200.0);
    // Nothing else moved.
    assert_eq!(set.fauna.founder(Founder::Blind), shipped.fauna.founder(Founder::Blind));
    assert_eq!(set.flora, shipped.flora);
}

/// **Test 5**, the layered half: a `--set` outside the row's range is refused with the
/// path and the range; a TOML value too; and a cross-field rule refuses the resolved set.
#[test]
fn an_out_of_range_layer_is_refused() {
    let e = Params::resolve(&Layers {
        sets: vec!["fauna.parasite_detach_body_fraction=1.5".into()],
        ..Layers::default()
    })
    .expect_err("1.5 accepted");
    assert!(e.contains("fauna.parasite_detach_body_fraction") && e.contains("(0, 1]"), "{e}");
    let e = Params::resolve(&Layers {
        file: Some(doc("[world.day]\ndaylight = 0.99\n")),
        ..Layers::default()
    })
    .expect_err("daylight 0.99 accepted");
    assert!(e.contains("world.day.daylight") && e.contains("[0.05, 0.95]"), "{e}");
    let e = Params::resolve(&Layers {
        sets: vec!["world.climate.dry_gap_min_s=1000".into()],
        ..Layers::default()
    })
    .expect_err("a backwards gap accepted");
    assert!(e.contains("dry_gap"), "{e}");
}

/// **Test 7**: with no layers the resolved set is today's struct defaults, field for
/// field.
#[test]
fn the_resolved_defaults_are_todays_defaults() {
    let p = Params::resolve(&Layers::default()).expect("defaults");
    assert_eq!(p.world, Config::default());
    assert_eq!(p.flora, FloraConfig::default());
    assert_eq!(p.fauna, FaunaConfig::default());
    assert_eq!(p.flora_for_world(), FloraConfig::for_voxel_size(Config::default().voxel_m));
    // And on a finer world, the plant layer is the rescaled one.
    let fine = Params::resolve(&Layers {
        sets: vec!["world.voxel_m=0.125".into()],
        ..Layers::default()
    })
    .expect("a fine world");
    assert_eq!(fine.flora, FloraConfig::default(), "authored values are not rescaled");
    assert_eq!(fine.flora_for_world(), FloraConfig::for_voxel_size(0.125));
}

/// **Test 8**: the dump lists every row of every table, and a run's output directory
/// holds the resolved set, which read back as a layer gives the same set.
#[test]
fn the_dump_lists_every_row_and_a_run_writes_its_set() {
    let p = Params::resolve(&Layers {
        sets: vec!["fauna.founders.lanternjaw.core.maintenance_per_s=0.0003".into()],
        ..Layers::default()
    })
    .expect("a set");
    let dump = p.dump().expect("the dump");
    for reg in [
        cubarium_voxel::registry::world(),
        cubarium_voxel_flora::registry(),
        cubarium_voxel_fauna::registry(),
    ] {
        for row in reg.rows().iter().filter(|r| !r.optional) {
            assert!(
                dump.iter().any(|l| row.matches(&l.path)),
                "the dump has no line for `{}`",
                row.path
            );
        }
    }
    let line = dump
        .iter()
        .find(|l| l.path == "fauna.founders.lanternjaw.core.maintenance_per_s")
        .expect("the line");
    assert_eq!(line.value, "0.0003");
    assert!(!line.row.unit.is_empty());

    let md = p.markdown().expect("the markdown");
    for reg in [
        cubarium_voxel::registry::world(),
        cubarium_voxel_flora::registry(),
        cubarium_voxel_fauna::registry(),
    ] {
        for row in reg.rows() {
            assert!(md.contains(&format!("`{}`", row.path)), "{} not in the markdown", row.path);
        }
    }

    let dir = std::env::temp_dir().join(format!("cubarium-params-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let written = p.write_beside(&dir).expect("written");
    assert_eq!(written, dir.join(PARAMS_FILE));
    let text = std::fs::read_to_string(&written).expect("readable");
    assert!(text.contains("[fauna"), "{text}");
    let back = Params::resolve(&Layers {
        file: Some(doc(&text)),
        ..Layers::default()
    })
    .expect("the written set is a layer");
    assert_eq!(back, p);
    let _ = std::fs::remove_dir_all(&dir);
}
