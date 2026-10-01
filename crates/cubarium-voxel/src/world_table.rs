//! A config file's **`[world]` table**, read as the ambient app reads it: a
//! `[world.landform] preset = "…"` expanded into the landform the core's serde reads, and
//! an omitted `voxel_m` defaulting to the shipped `default` landscape's. Moved here from
//! the `cubarium` binary so the training harness founds the world a host profile runs.

use crate::Config;

/// A `[world]` table as a [`Config`]: the cell size defaults to the `default` landscape's,
/// a named landform preset is expanded, and every other key is [`Config`]'s own.
pub fn world_config_from_table(mut table: toml::Table) -> Result<Config, String> {
    let cell = toml::Value::Float(
        crate::Preset::find("default")
            .expect("the shipped presets include `default`")
            .voxel_m,
    );
    table.entry("voxel_m").or_insert(cell);
    expand_landform_preset(&mut table)?;
    toml::Value::Table(table)
        .try_into()
        .map_err(|e| format!("[world]: {e}"))
}

/// The `[world]` table of the config file at `path`, as a [`Config`]
/// ([`world_config_from_table`]); an absent table is the defaults'.
pub fn world_config_from_file(path: &std::path::Path) -> Result<Config, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let doc: toml::Table = text
        .parse()
        .map_err(|e| format!("{} is not TOML: {e}", path.display()))?;
    let world = match doc.get("world") {
        Some(toml::Value::Table(t)) => t.clone(),
        Some(_) => return Err(format!("{}: [world] is not a table", path.display())),
        None => toml::Table::new(),
    };
    world_config_from_table(world)
}

/// Let `[world.landform]` name one of the shipped landscapes:
///
/// ```toml
/// [world.landform]
/// preset = "wide"     # small | default | wide
/// relief_m = 3.0      # any recipe field, overriding the preset
/// ```
///
/// The preset is expanded here, into the plain `landform = { staged = { .. } }` the
/// core's serde already reads, so `crate::Recipe` stays an ordinary struct that
/// postcard can read straight back out of a world snapshot. The extents stay `[world]`'s
/// business: a preset names a landscape, not a ring size.
///
/// Anything else — no `landform` key, `landform = "ridge"`, or a `[world.landform.staged]`
/// table written out in full — passes through untouched.
pub fn expand_landform_preset(world: &mut toml::Table) -> std::result::Result<(), String> {
    let Some(toml::Value::Table(landform)) = world.get("landform") else {
        return Ok(());
    };
    if !landform.contains_key("preset") {
        return Ok(());
    }
    let mut landform = landform.clone();
    let name = match landform.remove("preset") {
        Some(toml::Value::String(name)) => name,
        Some(other) => {
            return Err(format!(
                "[world.landform] preset must be a name, not {other}"
            ));
        }
        None => unreachable!("the key is there"),
    };
    // A terrarium preset names the other generator: `landform = { terrarium = { .. } }`.
    if let Some(terrarium) = crate::Terrarium::preset(&name) {
        let mut table = match toml::Value::try_from(terrarium) {
            Ok(toml::Value::Table(t)) => t,
            _ => return Err("a terrarium is a table".into()),
        };
        merge_over(&mut table, landform);
        let mut wrapped = toml::Table::new();
        wrapped.insert("terrarium".into(), toml::Value::Table(table));
        world.insert("landform".into(), toml::Value::Table(wrapped));
        return Ok(());
    }
    let preset = crate::Preset::find(&name).ok_or_else(|| {
        let known: Vec<&str> = crate::PRESETS
            .iter()
            .map(|p| p.name)
            .chain(crate::Terrarium::PRESETS.iter().map(|(n, _)| *n))
            .collect();
        format!("no landform preset is called {name:?}; the shipped ones are {known:?}")
    })?;
    let mut staged = match toml::Value::try_from(preset.recipe) {
        Ok(toml::Value::Table(t)) => t,
        _ => return Err("a recipe is a table".into()),
    };
    // Whatever else the table said overrides the preset, field by field.
    merge_over(&mut staged, landform);
    let mut wrapped = toml::Table::new();
    wrapped.insert("staged".into(), toml::Value::Table(staged));
    world.insert("landform".into(), toml::Value::Table(wrapped));
    Ok(())
}

/// Lay `over` onto `base` field by field, into nested tables too: a config that says
/// `[world.landform.water] inventory_m = 2.0` changes that one field of the preset's
/// water and keeps the rest, rather than replacing the whole table with a dry default.
fn merge_over(base: &mut toml::Table, over: toml::Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(inner)), toml::Value::Table(value)) => merge_over(inner, value),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

