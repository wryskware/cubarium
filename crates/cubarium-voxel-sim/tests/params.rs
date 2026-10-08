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

/// **`set=` beside `load=`** (`Params::over_loaded`): on a world finer than the reference
/// voxel, a plant set on a loaded layer holds the value the same set holds on a fresh run
/// of that world — for a voxel-scaled row (`hop`) and an unscaled one (`assimilation`) —
/// while every field nobody set keeps the save's value, and the resolved set records the
/// authored value.
#[test]
fn a_set_on_a_loaded_world_means_what_it_means_on_a_fresh_one() {
    let world = Config {
        voxel_m: 0.1,
        ..Config::default()
    };
    let resolved = |sets: &[&str]| {
        Params::resolve_over(
            Params {
                world: world.clone(),
                ..Params::defaults()
            },
            &Layers {
                sets: sets.iter().map(|s| s.to_string()).collect(),
                ..Layers::default()
            },
        )
        .expect("the sets resolve")
    };
    // The save: a run founded under an earlier set of its own, through the plant
    // snapshot's bytes as a loaded layer comes back.
    let earlier = resolved(&["flora.velvetpad.rooting_radius=3", "fauna.founding.arrival_body=0.6"]);
    let founded = cubarium_voxel_flora::Flora::new(earlier.flora_for_world());
    let saved = cubarium_voxel_flora::snapshot::decode(&cubarium_voxel_flora::snapshot::encode(&founded))
        .expect("the snapshot reads back");

    let hop = "flora.springturf.hop";
    let assimilation = "flora.springturf.assimilation";
    let authored_hop = registry_get(&Params::defaults().flora, hop).as_integer().unwrap() + 2;
    let authored_assim = registry_get(&Params::defaults().flora, assimilation).as_float().unwrap() * 1.5;
    let sets = [
        format!("{hop}={authored_hop}"),
        format!("{assimilation}={authored_assim}"),
        "fauna.founding.arrival_stomach=0.5".to_string(),
    ];
    let loaded = Params::over_loaded(world.clone(), saved.config(), earlier.fauna.clone(), &sets)
        .expect("flora and fauna sets resolve over a loaded world");

    let mut all: Vec<&str> = vec!["flora.velvetpad.rooting_radius=3", "fauna.founding.arrival_body=0.6"];
    all.extend(sets.iter().map(String::as_str));
    let fresh = resolved(&all).flora_for_world();
    for path in [hop, assimilation] {
        assert_eq!(registry_get(&loaded.flora, path), registry_get(&fresh, path), "{path}");
    }
    // The scaled row really was rescaled, and the resolved set records it as authored.
    assert_ne!(registry_get(&loaded.flora, hop).as_integer(), Some(authored_hop));
    assert_eq!(registry_get(&loaded.params.flora, hop).as_integer(), Some(authored_hop));
    // What nobody set is the save's, field for field.
    let untouched = "flora.velvetpad.rooting_radius";
    assert_eq!(registry_get(&loaded.flora, untouched), registry_get(saved.config(), untouched));
    assert_eq!(registry_get(&loaded.params.flora, untouched).as_integer(), Some(3));
    let mut expected = saved.config().clone();
    cubarium_voxel_flora::registry()
        .apply(
            &mut expected,
            &[hop, assimilation].map(|p| (p.to_string(), registry_get(&fresh, p))),
        )
        .unwrap();
    assert_eq!(loaded.flora, expected, "only the set paths differ from the save");
    // The animal set lies over the save's animals, not the code defaults.
    assert_eq!(loaded.params.fauna.founding.arrival_stomach, 0.5);
    assert_eq!(loaded.params.fauna.founding.arrival_body, earlier.fauna.founding.arrival_body);
}

/// A `world.*` set beside `load=` is refused by name: the save fixes the world.
#[test]
fn a_world_set_on_a_loaded_world_is_refused() {
    let p = Params::defaults();
    let e = Params::over_loaded(
        p.world.clone(),
        &p.flora_for_world(),
        p.fauna.clone(),
        &["world.climate.dry_gap_min_s=0".to_string()],
    )
    .expect_err("a world set is refused");
    assert!(e.contains("world.climate.dry_gap_min_s"), "{e}");
}

/// A world's plant config taken back to its authored form is the authored one: a rounded
/// voxel distance exactly, a scaled amount to rounding error.
#[test]
fn authored_inverts_rescaled_to() {
    let reg = cubarium_voxel_flora::registry();
    let leaves = |cfg: &FloraConfig| {
        let v = cubarium_voxel::registry::to_value(cfg).unwrap();
        reg.typed_leaves(&v)
    };
    let authored = Params::defaults().flora;
    for voxel_m in [0.05, 0.1, 0.125, 0.2, 0.25, 0.5] {
        let back = authored.clone().rescaled_to(voxel_m).authored();
        let (a, b) = (leaves(&authored), leaves(&back));
        assert_eq!(a.len(), b.len());
        for ((path, x, int), (_, y, _)) in a.iter().zip(&b) {
            if *int || x == y {
                assert_eq!(x, y, "{path} at {voxel_m}");
            } else {
                assert!((x - y).abs() <= 1e-12 * x.abs().max(1e-300), "{path} at {voxel_m}: {x} vs {y}");
            }
        }
    }
}

fn registry_get(cfg: &FloraConfig, path: &str) -> toml::Value {
    cubarium_voxel_flora::registry().get(cfg, path).expect(path)
}
