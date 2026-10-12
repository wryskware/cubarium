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
use cubarium_voxel::genome::{Genome, SearchFile};
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

/// [`Params::over_loaded`]'s result.
#[derive(Clone, Debug)]
pub struct LoadedParams {
    /// The resolved set, its plants as authored: what `params.toml` records.
    pub params: Params,
    /// The plant config to install on the loaded layer, at the world's cell size.
    pub flora: FloraConfig,
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

    /// **`set=` over a loaded run**: `sets` resolved over the configs a saved world carries
    /// — its world config, its plant config **as it runs** (at the world's cell size) and
    /// its animal config — rather than over the code defaults.
    ///
    /// A `flora.*` value means what it means on a fresh run of the same world: the
    /// authored unit on the reference 0.25 m voxel, rescaled to the world's
    /// ([`Params::flora_for_world`]). The loaded plant config is taken back to its authored
    /// form ([`FloraConfig::authored`]) for the resolved set, and the plant config to
    /// install is the loaded one with only the set paths replaced by their rescaled
    /// values, so every field nobody set stays exactly as saved.
    ///
    /// A `world.*` set is refused: the save fixes the terrain and water, and the world has
    /// no setter for any of its config at run time, the climate and day clock included.
    pub fn over_loaded(
        world: Config,
        flora: &FloraConfig,
        fauna: FaunaConfig,
        sets: &[String],
    ) -> Result<LoadedParams, String> {
        let mut flora_paths = Vec::new();
        for s in sets {
            let (path, _) = registry::parse_set(s)?;
            match path.split('.').next().unwrap_or_default() {
                "world" => {
                    return Err(format!(
                        "set={s}: a loaded world keeps its saved world config (terrain, water, \
                         climate and day clock alike); only flora.* and fauna.* can be set beside load="
                    ));
                }
                "flora" => flora_paths.push(path),
                _ => {}
            }
        }
        let params = Params::resolve_over(
            Params {
                world,
                flora: flora.clone().authored(),
                fauna,
            },
            &Layers {
                sets: sets.to_vec(),
                ..Layers::default()
            },
        )?;
        let rescaled = params.flora_for_world();
        let reg = cubarium_voxel_flora::registry();
        let mut edits = Vec::with_capacity(flora_paths.len());
        for path in flora_paths {
            let value = reg.get(&rescaled, &path)?;
            edits.push((path, value));
        }
        let mut installed = flora.clone();
        reg.apply(&mut installed, &edits).map_err(|e| format!("--set: {e}"))?;
        installed.validate()?;
        Ok(LoadedParams {
            params,
            flora: installed,
        })
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

    /// Expand a search file over this set's plants and animals ([`Genome::expand`]):
    /// `tie = "each"` split, every path, group and rule checked.
    ///
    /// **A lineage this set founds none of** (`founding_count` 0, a new founder's code
    /// default) **has no gene**: its `fauna.founders.<lineage>` subtree is hidden from the
    /// expansion, so a `tie = "each"` over `fauna.founders.*` splits only over the
    /// lineages a world is founded with.
    pub fn expand_genome(&self, file: SearchFile) -> Result<Genome, String> {
        let flora = registry::to_value(&self.flora)?;
        let mut fauna = registry::to_value(&self.fauna)?;
        if let Some(founders) = fauna
            .as_table_mut()
            .and_then(|t| t.get_mut("founders"))
            .and_then(|f| f.as_array_mut())
        {
            for (slot, count) in founders.iter_mut().zip(self.fauna.founder_counts()) {
                if count == 0 {
                    *slot = toml::Value::Table(toml::Table::new());
                }
            }
        }
        Genome::expand(
            file,
            &[
                (cubarium_voxel_flora::registry(), &flora),
                (cubarium_voxel_fauna::registry(), &fauna),
            ],
        )
    }

    /// Decode `genome` over this set's plants (as authored) and animals, then validate.
    /// The world is not searched: a gene names a `flora.*` or `fauna.*` path.
    pub fn apply_genome(&mut self, genome: &Genome) -> Result<(), String> {
        let flora = registry::to_value(&self.flora)?;
        let fauna = registry::to_value(&self.fauna)?;
        let edits = genome.resolve(&[
            (cubarium_voxel_flora::registry(), &flora),
            (cubarium_voxel_fauna::registry(), &fauna),
        ])?;
        cubarium_voxel_flora::registry()
            .apply(&mut self.flora, &edits[0])
            .map_err(|e| format!("genome: {e}"))?;
        cubarium_voxel_fauna::registry()
            .apply(&mut self.fauna, &edits[1])
            .map_err(|e| format!("genome: {e}"))?;
        self.validate()
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
        let table = crate::engagement::registry();
        let dump = table.dump(&crate::engagement::Config::default())?;
        md.push_str("\n**`engagement.*`** (isolated harness; `config/engagement.toml`)\n\n| path | value | unit | range | what |\n| --- | --- | --- | --- | --- |\n");
        for row in table.rows() {
            let values: Vec<_> = dump.iter().filter(|d| d.row.path == row.path).map(|d| d.value.as_str()).collect();
            md.push_str(&format!("| `{}` | {} | {} | {} | {} |\n", row.path, values.join(", "), row.unit, row.range, row.what));
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

/// A search (or genome) file's text, expanded over the code defaults.
pub fn load_genome(text: &str) -> Result<Genome, String> {
    Params::defaults().expand_genome(SearchFile::parse(text)?)
}

/// **A genome as `set=` assignments** (`voxel_census genome=FILE gene=NAME=VALUE`): `text`
/// expanded over `base` ([`Params::expand_genome`]), each named gene (its expanded name,
/// `upkeep@frondgrazer`) set inside its box, then decoded over `base` with the ties
/// followed ([`Genome::resolve`]); every leaf that changes comes back as `path=value`.
///
/// The assignments go through the ordinary `set=` layer, so a fresh run and a loaded one
/// (whose `set=` resolves over the save's own configs) take a genome the same way. `base`
/// carries the plants as authored, as [`Params`] always does.
pub fn genome_sets(
    base: &Params,
    text: &str,
    genes: &[(String, f64)],
) -> Result<(Genome, Vec<String>), String> {
    let mut genome = base.expand_genome(SearchFile::parse(text)?)?;
    for (name, value) in genes {
        genome.set(name, *value)?;
    }
    let flora = registry::to_value(&base.flora)?;
    let fauna = registry::to_value(&base.fauna)?;
    let edits = genome.resolve(&[
        (cubarium_voxel_flora::registry(), &flora),
        (cubarium_voxel_fauna::registry(), &fauna),
    ])?;
    let sets = edits
        .into_iter()
        .flatten()
        .map(|(path, value)| format!("{path}={value}"))
        .collect();
    Ok((genome, sets))
}

/// `gene=NAME=VALUE`'s `(NAME, VALUE)`.
pub fn parse_gene_arg(s: &str) -> Result<(String, f64), String> {
    let (name, value) = s
        .rsplit_once('=')
        .ok_or_else(|| format!("gene={s}: expected NAME=VALUE"))?;
    let value: f64 = value
        .parse()
        .map_err(|_| format!("gene={s}: {value} is not a number"))?;
    if name.is_empty() || !value.is_finite() {
        return Err(format!("gene={s}: expected NAME=finite value"));
    }
    Ok((name.to_string(), value))
}
