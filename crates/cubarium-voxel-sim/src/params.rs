//! **The resolved parameter set** (package R1, `design/ecology-parameter-registry-2026-09-28.md`
//! §2): the world, plant and animal configs a run uses, built in layers — code defaults,
//! the preset or world TOML, the search genome, `--set path=value` — validated against the
//! three registries, dumped, and written beside a run's output.
//!
//! A path is the registry's: `world.climate.dry_gap_min_s`, `flora.springturf.hop`,
//! `fauna.founders.frondgrazer.core.maintenance_per_s`. An unknown one is an error that
//! names it, in every layer.

use std::path::{Path, PathBuf};

use cubarium_voxel::Config;
use cubarium_voxel::registry::{self, DumpLine, Registry};
use cubarium_voxel_fauna::FaunaConfig;
use cubarium_voxel_flora::FloraConfig;

/// The file a run writes its resolved set to, in its output directory.
pub const PARAMS_FILE: &str = "params.toml";

/// Every number a run uses.
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub world: Config,
    /// The plant layer as authored, on the reference 0.25 m voxel;
    /// [`Params::flora_for_world`] is it rescaled to the world's.
    pub flora: FloraConfig,
    pub fauna: FaunaConfig,
}

/// What lies over the code defaults, applied in field order.
#[derive(Default)]
pub struct Layers<'a> {
    /// A TOML document: its `[world]`, `[flora]` and `[fauna]` tables; any other key is
    /// someone else's (the presenter's) and ignored. `[world]` reads as the app reads it
    /// (`cubarium_voxel::world_table`): an omitted `voxel_m` is the `default` landscape's
    /// and a `[world.landform] preset` is expanded.
    pub file: Option<toml::Table>,
    /// The search genome's edit.
    pub genome: Option<&'a dyn Fn(&mut Params)>,
    /// `path=value` assignments, last.
    pub sets: Vec<String>,
}

/// The three tables, in dump order.
fn tables() -> [&'static Registry; 3] {
    [registry::world(), cubarium_voxel_flora::registry(), cubarium_voxel_fauna::registry()]
}

impl Params {
    /// The code defaults: today's struct defaults, field for field.
    pub fn defaults() -> Params {
        Params {
            world: Config::default(),
            flora: FloraConfig::default(),
            fauna: FaunaConfig::default(),
        }
    }

    /// [`Params::resolve_over`] the code defaults.
    pub fn resolve(layers: &Layers<'_>) -> Result<Params, String> {
        Params::resolve_over(Params::defaults(), layers)
    }

    /// `base` with each layer applied in order, then validated whole.
    pub fn resolve_over(mut p: Params, layers: &Layers<'_>) -> Result<Params, String> {
        if let Some(doc) = &layers.file {
            p.apply_file(doc)?;
        }
        if let Some(genome) = layers.genome {
            genome(&mut p);
        }
        let mut sets: [Vec<(String, toml::Value)>; 3] = Default::default();
        for s in &layers.sets {
            let (path, value) = registry::parse_set(s)?;
            let root = path.split('.').next().unwrap_or_default();
            let i = tables()
                .iter()
                .position(|t| t.root() == root)
                .ok_or_else(|| format!("unknown parameter `{path}`: no table is called `{root}`"))?;
            sets[i].push((path, value));
        }
        let [world, flora, fauna] = sets;
        with_seeds_masked(&mut p.world, |w| registry::world().apply(w, &world))
            .map_err(|e| format!("--set: {e}"))?;
        cubarium_voxel_flora::registry()
            .apply(&mut p.flora, &flora)
            .map_err(|e| format!("--set: {e}"))?;
        cubarium_voxel_fauna::registry()
            .apply(&mut p.fauna, &fauna)
            .map_err(|e| format!("--set: {e}"))?;
        p.validate()?;
        Ok(p)
    }

    fn apply_file(&mut self, doc: &toml::Table) -> Result<(), String> {
        let table = |key: &str| -> Result<Option<toml::Table>, String> {
            match doc.get(key) {
                None => Ok(None),
                Some(toml::Value::Table(t)) => Ok(Some(t.clone())),
                Some(_) => Err(format!("[{key}] is not a table")),
            }
        };
        if let Some(mut world) = table("world")? {
            let cell = cubarium_voxel::Preset::find("default")
                .expect("the shipped presets include `default`")
                .voxel_m;
            world.entry("voxel_m").or_insert(toml::Value::Float(cell));
            cubarium_voxel::expand_landform_preset(&mut world)?;
            with_seeds_masked(&mut self.world, |w| registry::world().apply_table(w, &world))
                .map_err(|e| format!("[world]: {e}"))?;
        }
        if let Some(flora) = table("flora")? {
            cubarium_voxel_flora::registry()
                .apply_table(&mut self.flora, &flora)
                .map_err(|e| format!("[flora]: {e}"))?;
        }
        if let Some(fauna) = table("fauna")? {
            cubarium_voxel_fauna::registry()
                .apply_table(&mut self.fauna, &fauna)
                .map_err(|e| format!("[fauna]: {e}"))?;
        }
        Ok(())
    }

    /// Every number inside its row's range, and each crate's cross-field rules.
    pub fn validate(&self) -> Result<(), String> {
        self.world.validate().map_err(|e| format!("{e:#}"))?;
        self.flora.validate()?;
        self.fauna.validate()
    }

    /// The plant layer a world of this cell size runs: [`Params::flora`] rescaled by the
    /// registry's scale tags ([`FloraConfig::rescaled_to`]).
    pub fn flora_for_world(&self) -> FloraConfig {
        self.flora.clone().rescaled_to(self.world.voxel_m)
    }

    /// Every numeric leaf of the three configs with its value, unit and range.
    pub fn dump(&self) -> Result<Vec<DumpLine>, String> {
        let mut out = registry::world().dump(&self.world)?;
        out.extend(cubarium_voxel_flora::registry().dump(&self.flora)?);
        out.extend(cubarium_voxel_fauna::registry().dump(&self.fauna)?);
        Ok(out)
    }

    /// The dump as markdown, one table per crate and one line per row, its values listed
    /// beside it: what `design/backlog.md` §1 carries.
    pub fn markdown(&self) -> Result<String, String> {
        let dump = self.dump()?;
        let mut md = String::new();
        for t in tables() {
            md.push_str(&format!(
                "\n**`{}.*`** ({} rows)\n\n| path | value | unit | range | search | grade | provenance | what |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n",
                t.root(),
                t.rows().len()
            ));
            for row in t.rows() {
                let values: Vec<&str> = dump
                    .iter()
                    .filter(|l| l.row.path == row.path)
                    .map(|l| l.value.as_str())
                    .collect();
                let value = match values.len() {
                    0 => "(absent)".to_string(),
                    1..=6 => values.join(", "),
                    n => format!("{} … ({n} values)", values[..4].join(", ")),
                };
                let search = match row.search {
                    Some((lo, hi)) => format!("{lo}–{hi}{}", if row.log { " log" } else { "" }),
                    None if row.evolve => "yes".into(),
                    None => String::new(),
                };
                md.push_str(&format!(
                    "| `{}` | {} | {} | {} | {} | {} | {}{} | {} |\n",
                    row.path,
                    value,
                    row.unit,
                    row.range,
                    search,
                    row.grade,
                    row.provenance,
                    if row.digest { ", digest" } else { "" },
                    row.what
                ));
            }
        }
        Ok(md)
    }

    /// The resolved set as a TOML document — `[world]`, `[flora]` (as authored) and
    /// `[fauna]` — which reads back as a [`Layers::file`].
    pub fn to_toml(&self) -> Result<String, String> {
        let mut doc = toml::Table::new();
        doc.insert("world".into(), registry::to_value(&self.world)?);
        doc.insert("flora".into(), registry::to_value(&self.flora)?);
        doc.insert("fauna".into(), registry::to_value(&self.fauna)?);
        let body = toml::to_string(&doc).map_err(|e| format!("cannot write the parameter set: {e}"))?;
        Ok(format!(
            "# The resolved parameter set of one run (package R1,\n\
             # design/ecology-parameter-registry-2026-09-28.md): every number the world, the\n\
             # plants and the animals ran on. [flora] is as authored on the reference 0.25 m\n\
             # voxel; the run rescales it to world.voxel_m. Passed back as a parameter file it\n\
             # resolves to the same set.\n\n{body}"
        ))
    }

    /// Write [`Params::to_toml`] to `dir/params.toml`, creating `dir`.
    pub fn write_beside(&self, dir: &Path) -> Result<PathBuf, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let path = dir.join(PARAMS_FILE);
        std::fs::write(&path, self.to_toml()?)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        Ok(path)
    }
}

/// Run `f` on a world config whose seeds read 0 (TOML's integers stop at `i64::MAX` and a
/// drawn seed is any `u64`), then put the seeds back unless `f` set them.
fn with_seeds_masked(
    world: &mut Config,
    f: impl FnOnce(&mut Config) -> Result<(), String>,
) -> Result<(), String> {
    let (seed, noise) = (world.seed, world.noise_seed);
    let big = |s: u64| s > i64::MAX as u64;
    if big(seed) {
        world.seed = 0;
    }
    if big(noise) {
        world.noise_seed = 0;
    }
    let result = f(world);
    if big(seed) && world.seed == 0 {
        world.seed = seed;
    }
    if big(noise) && world.noise_seed == 0 {
        world.noise_seed = noise;
    }
    result
}
