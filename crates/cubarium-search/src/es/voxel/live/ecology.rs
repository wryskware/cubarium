//! The **ecology a training world is founded under**: a search genome
//! ([`cubarium_voxel::genome`], R3) decoded over the shipped configs, or a resolved
//! parameter file read as it stands.
//!
//! Every searched number is a registry path, so there is no routing here: a search is a
//! file under `config/training/search/` (`cotrain.toml` is stage 1's, `legacy.toml` the
//! vector this module used to hand-route), a genome is the same file with each gene's
//! value, and the one generic decode
//! ([`cubarium_voxel_sim::params::Params::apply_genome`]) moves every path it names.
//!
//! [`EcologyParams::load`] reads three kinds of file:
//! - a **genome or search file** (`[[gene]]` tables);
//! - a **resolved parameter file** (`[flora]` and `[fauna]` tables, as a run's
//!   `params.toml` writes them; a `[world]` there is ignored, the world is the caller's);
//! - an **old `[ecology]` vector** (`config/training/ecology-tuned-*.toml`), read through
//!   `legacy.toml`'s gene names.

use cubarium_voxel::genome::Genome;
use cubarium_voxel_fauna::FaunaConfig;
use cubarium_voxel_flora::FloraConfig;
use cubarium_voxel_sim::LayerConfig;
use cubarium_voxel_sim::params::{Params, load_genome};

/// The pre-R3 routed vector as a search file.
pub const LEGACY: &str = include_str!("../../../../../../config/training/search/legacy.toml");
/// Stage 1's search (`design/handoffs/cotrain-stage0-2026-10-01.md` §2).
pub const COTRAIN: &str = include_str!("../../../../../../config/training/search/cotrain.toml");
/// Where the search files live, from the repository root.
pub const SEARCH_DIR: &str = "config/training/search";

/// One training world's ecology.
#[derive(Clone, Debug, PartialEq)]
pub enum EcologyParams {
    /// A genome decoded over the shipped plants and animals. No genes is the shipped world.
    Genome(Genome),
    /// Plants (as authored, on the reference voxel) and animals, read whole.
    Resolved {
        flora: Box<FloraConfig>,
        fauna: Box<FaunaConfig>,
    },
}

impl Default for EcologyParams {
    fn default() -> EcologyParams {
        EcologyParams::defaults()
    }
}

impl EcologyParams {
    /// The shipped world: no genes.
    pub fn defaults() -> EcologyParams {
        EcologyParams::Genome(Genome::default())
    }

    /// `legacy.toml` at x1 (the shipped world, with the old vector's names to set).
    pub fn legacy() -> EcologyParams {
        EcologyParams::Genome(load_genome(LEGACY).expect("legacy.toml is a valid search file"))
    }

    /// `cotrain.toml` at x1.
    pub fn cotrain() -> EcologyParams {
        EcologyParams::Genome(load_genome(COTRAIN).expect("cotrain.toml is a valid search file"))
    }

    pub fn genome(&self) -> Option<&Genome> {
        match self {
            EcologyParams::Genome(g) => Some(g),
            EcologyParams::Resolved { .. } => None,
        }
    }

    /// A gene's value, by name.
    pub fn get(&self, name: &str) -> Option<f64> {
        self.genome().and_then(|g| g.get(name))
    }

    /// Set one gene, refusing an unknown name, an out-of-range value, or a resolved file.
    pub fn set(&mut self, name: &str, value: f64) -> Result<(), String> {
        match self {
            EcologyParams::Genome(g) => g.set(name, value),
            EcologyParams::Resolved { .. } => {
                Err(format!("{name}: a resolved parameter file has no genes"))
            }
        }
    }

    /// The plants (as authored) and animals this ecology runs; the world is the default.
    /// Refuses a genome that does not decode (unreachable for one [`load`] or a search
    /// expanded).
    pub fn params(&self) -> Result<Params, String> {
        match self {
            EcologyParams::Genome(g) => {
                let mut p = Params::defaults();
                p.apply_genome(g)?;
                Ok(p)
            }
            EcologyParams::Resolved { flora, fauna } => Ok(Params {
                flora: (**flora).clone(),
                fauna: **fauna,
                ..Params::defaults()
            }),
        }
    }

    fn decoded(&self) -> Params {
        self.params().expect("a loaded ecology decodes")
    }

    /// The animal layer's config.
    pub fn fauna(&self) -> FaunaConfig {
        self.decoded().fauna
    }

    /// The plant layer's config at cell size `voxel_m`.
    pub fn flora(&self, voxel_m: f64) -> FloraConfig {
        self.decoded().flora.rescaled_to(voxel_m)
    }

    /// Both layers at cell size `voxel_m`.
    pub fn layers(&self, voxel_m: f64) -> LayerConfig {
        let p = self.decoded();
        LayerConfig {
            flora: p.flora.rescaled_to(voxel_m),
            fauna: p.fauna,
        }
    }

    /// Whether this is the shipped world, whatever its form.
    pub fn is_shipped(&self) -> bool {
        self.params().is_ok_and(|p| {
            let d = Params::defaults();
            p.flora == d.flora && p.fauna == d.fauna
        })
    }

    /// Read a genome or search file, a resolved parameter file, or an old `[ecology]`
    /// vector (module docs).
    pub fn load(path: &std::path::Path) -> Result<EcologyParams, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        EcologyParams::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// [`EcologyParams::load`] from text.
    pub fn parse(text: &str) -> Result<EcologyParams, String> {
        let doc: toml::Table = text.parse().map_err(|e| format!("not TOML: {e}"))?;
        if doc.contains_key("gene") {
            return Ok(EcologyParams::Genome(load_genome(text)?));
        }
        if let Some(table) = doc.get("ecology").and_then(|t| t.as_table()) {
            return EcologyParams::from_old(table);
        }
        if doc.contains_key("flora") || doc.contains_key("fauna") {
            let mut only = toml::Table::new();
            for key in ["flora", "fauna"] {
                if let Some(v) = doc.get(key) {
                    only.insert(key.into(), v.clone());
                }
            }
            let p = Params::resolve(&cubarium_voxel_sim::params::Layers {
                file: Some(only),
                ..Default::default()
            })?;
            return Ok(EcologyParams::Resolved {
                flora: Box::new(p.flora),
                fauna: Box::new(p.fauna),
            });
        }
        Err("neither a genome ([[gene]]), a parameter file ([flora]/[fauna]) nor an \
             [ecology] vector"
            .into())
    }

    /// An old `[ecology]` table: shared names, and `[ecology.<lineage>]` tables of
    /// per-lineage ones, set on `legacy.toml`'s genes.
    fn from_old(table: &toml::Table) -> Result<EcologyParams, String> {
        let mut e = EcologyParams::legacy();
        let num = |name: &str, v: &toml::Value| {
            v.as_float()
                .or_else(|| v.as_integer().map(|i| i as f64))
                .ok_or_else(|| format!("{name} is not a number"))
        };
        for (name, value) in table {
            if let Some(sub) = value.as_table() {
                for (k, v) in sub {
                    e.set(&format!("{k}@{name}"), num(k, v)?)?;
                }
                continue;
            }
            e.set(name, num(name, value)?)?;
        }
        Ok(e)
    }

    /// Write this ecology as a training-only file: a genome as a genome file, a resolved
    /// set as a parameter file.
    pub fn save(&self, path: &std::path::Path, note: &str) -> Result<(), String> {
        let text = match self {
            EcologyParams::Genome(g) => g.to_toml(note),
            EcologyParams::Resolved { .. } => {
                let mut t = String::new();
                for line in note.lines() {
                    t.push_str(&format!("# {line}\n"));
                }
                t + &self.params()?.to_toml()?
            }
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    /// `name=value` pairs, for a log line.
    pub fn describe(&self) -> String {
        match self {
            EcologyParams::Genome(g) => g.describe(),
            EcologyParams::Resolved { .. } => "(resolved parameter file)".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel_flora::{Species, Trophic};

    #[test]
    fn the_defaults_build_the_shipped_configs() {
        for e in [EcologyParams::defaults(), EcologyParams::legacy(), EcologyParams::cotrain()] {
            assert_eq!(e.fauna(), FaunaConfig::default());
            assert_eq!(e.flora(0.125), FloraConfig::for_voxel_size(0.125));
            assert!(e.is_shipped());
        }
    }

    #[test]
    fn a_routed_parameter_reaches_its_layer_and_a_bad_one_is_refused() {
        let mut e = EcologyParams::legacy();
        e.set("flora.foliage_rate", 2.0).expect("in range");
        let base = FloraConfig::for_voxel_size(0.125);
        let flora = e.flora(0.125);
        let s = Species::ALL
            .into_iter()
            .find(|s| base.species(*s).trophic == Trophic::Photo)
            .expect("a plant");
        assert_eq!(flora.species(s).foliage_rate, 2.0 * base.species(s).foliage_rate);
        assert!(e.set("flora.foliage_rate", 99.0).is_err());
        assert!(e.set("no.such", 1.0).is_err());
    }

    /// The three file kinds load; a resolved file round-trips.
    #[test]
    fn genome_resolved_and_old_files_load() {
        let mut g = EcologyParams::cotrain();
        g.set("K_bio", 1.5).expect("in range");
        let back = EcologyParams::parse(&g.genome().unwrap().to_toml("t")).expect("a genome");
        assert_eq!(back.fauna(), g.fauna());
        let resolved = EcologyParams::parse(&g.params().unwrap().to_toml().unwrap())
            .expect("a parameter file");
        assert!(matches!(resolved, EcologyParams::Resolved { .. }));
        assert_eq!(resolved.fauna(), g.fauna());
        assert_eq!(resolved.flora(0.125), g.flora(0.125));
        let old = EcologyParams::parse(
            "[ecology]\n\"flora.decomposition\" = 2.0\n[ecology.frondgrazer]\n\"founder.lifespan_median_s\" = 0.5\n",
        )
        .expect("an old vector");
        assert_eq!(old.get("flora.decomposition"), Some(2.0));
        assert_eq!(old.get("founder.lifespan_median_s@frondgrazer"), Some(0.5));
    }
}
