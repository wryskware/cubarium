//! A config file's `[world]` table read as the ambient app reads it — a named landform
//! preset expanded, the cell size defaulting to the `default` landscape's — so the
//! training harness can found the very world a host profile runs. No ticks.

use cubarium_voxel::{Landform, Preset, world_config_from_table};

fn table(text: &str) -> toml::Table {
    let doc: toml::Table = text.parse().expect("toml");
    doc.get("world")
        .and_then(|w| w.as_table())
        .cloned()
        .expect("a [world] table")
}

#[test]
fn a_terrarium_preset_expands_and_the_extents_are_the_files() {
    let c = world_config_from_table(table(
        "[world]\nwidth = 256\nheight = 128\ndepth = 48\nvoxel_m = 0.125\n\
         [world.landform]\npreset = \"terrarium\"\n[world.day]\nstart_hour = 9.0\n",
    ))
    .expect("the desktop terrarium reads");
    assert_eq!((c.width, c.height, c.depth, c.voxel_m), (256, 128, 48, 0.125));
    assert!(matches!(c.landform, Landform::Terrarium(_)), "{:?}", c.landform);
    assert_eq!(c.day.start_hour, 9.0);
}

#[test]
fn a_staged_preset_expands_and_a_field_overrides_it() {
    let c = world_config_from_table(table(
        "[world]\nwidth = 64\ndepth = 8\n[world.landform]\npreset = \"small\"\nrelief_m = 3.0\n",
    ))
    .expect("reads");
    let Landform::Staged(r) = c.landform else {
        panic!("a staged landform");
    };
    let small = Preset::find("small").expect("ships").recipe;
    assert_eq!(r.relief_m, 3.0);
    assert_eq!(r.water, small.water, "untouched fields keep the preset's");
    assert_eq!(
        c.voxel_m,
        Preset::find("default").expect("ships").voxel_m,
        "an omitted cell size is the default landscape's"
    );
    assert!(world_config_from_table(table("[world]\n[world.landform]\npreset = \"nope\"\n")).is_err());
}
