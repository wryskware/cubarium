//! The **ecology parameter vector**: the seam for the ecology half of live training.
//!
//! Each training world holds one [`EcologyParams`]: a named, ranged list of continuous
//! settings. Today every world runs the defaults, which change nothing: a scale of `1.0`
//! multiplies a shipped value by one, and an absolute parameter's default is the shipped
//! value read off the layer's own default config. Later a world-level ES perturbs each
//! world's vector and scores it against targets chosen from the readout
//! ([`super::readout`]).
//!
//! The names follow `design/backlog.md` §1 (the user-configurable parameter list) where one
//! exists: `foliage_rate`, `senescence` and `decomposition` are `FloraConfig` /
//! `SpeciesConfig` fields listed there, `yield_by_food` is `FounderPhysiology`'s, and
//! `surplus_floor` is the reproduction placeholder's. Adding a parameter is one
//! [`PARAMS`] row plus one line in [`EcologyParams::fauna_config`] or
//! [`EcologyParams::flora_config`].
//!
//! Parameters apply when a world's layers are built ([`super::world::found_world`]), so a
//! perturbation takes effect in the world founded (or refounded) with it.

use cubarium_voxel_fauna::{FaunaConfig, Founder};
use cubarium_voxel_flora::{FloraConfig, Species, Trophic};

/// How a parameter's value is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A multiplier on a shipped value (or on every value of a family, e.g. every
    /// photosynthetic species' foliage rate). Default `1.0`.
    Scale,
    /// An absolute value that replaces the shipped one. Default: the shipped value.
    Absolute,
}

/// One named, ranged parameter.
#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    pub name: &'static str,
    pub kind: Kind,
    pub min: f64,
    pub max: f64,
    /// What it means, in a line.
    pub what: &'static str,
}

/// Every parameter the ecology half may move. Order is the vector's order.
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "flora.foliage_rate",
        kind: Kind::Scale,
        min: 0.25,
        max: 4.0,
        what: "regrowth: every photosynthetic species' foliage_rate",
    },
    ParamSpec {
        name: "flora.senescence",
        kind: Kind::Scale,
        min: 0.25,
        max: 4.0,
        what: "litterfall: every photosynthetic species' senescence",
    },
    ParamSpec {
        name: "flora.decomposition",
        kind: Kind::Scale,
        min: 0.25,
        max: 4.0,
        what: "litter decomposition rate (FloraConfig::decomposition)",
    },
    ParamSpec {
        name: "founder.yield_by_food",
        kind: Kind::Scale,
        min: 0.25,
        // The shipped yields are 0.5; twice that is a body keeping every bite, past which
        // assimilation would create matter.
        max: 2.0,
        what: "bite yield: every founder's assimilation yield, every food class",
    },
    ParamSpec {
        name: "founder.reproduction.surplus_floor",
        kind: Kind::Scale,
        min: 0.25,
        max: 4.0,
        what: "breeding reserve threshold: every founder's surplus floor",
    },
];

/// One world's parameter vector, aligned with [`PARAMS`].
#[derive(Clone, Debug, PartialEq)]
pub struct EcologyParams {
    values: Vec<f64>,
}

impl Default for EcologyParams {
    fn default() -> EcologyParams {
        EcologyParams::defaults()
    }
}

impl EcologyParams {
    /// The shipped world: every scale `1.0`, every absolute value the layer default.
    pub fn defaults() -> EcologyParams {
        EcologyParams {
            values: PARAMS
                .iter()
                .map(|p| match p.kind {
                    Kind::Scale => 1.0,
                    Kind::Absolute => unreachable!("{} needs its shipped default read", p.name),
                })
                .collect(),
        }
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        PARAMS
            .iter()
            .position(|p| p.name == name)
            .map(|i| self.values[i])
    }

    /// Set one parameter, refusing an unknown name or a value outside its range.
    pub fn set(&mut self, name: &str, value: f64) -> Result<(), String> {
        let i = PARAMS
            .iter()
            .position(|p| p.name == name)
            .ok_or_else(|| format!("no ecology parameter is called {name:?}"))?;
        let p = PARAMS[i];
        if !(value.is_finite() && (p.min..=p.max).contains(&value)) {
            return Err(format!(
                "{name} = {value} is outside [{}, {}]",
                p.min, p.max
            ));
        }
        self.values[i] = value;
        Ok(())
    }

    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// A vector from natural-log multipliers, in [`PARAMS`] order, each clamped to its
    /// parameter's range. Only [`Kind::Scale`] parameters exist today, so `0` is the
    /// shipped value.
    pub fn from_log(log: &[f64]) -> EcologyParams {
        assert_eq!(log.len(), PARAMS.len(), "one value per parameter");
        EcologyParams {
            values: PARAMS
                .iter()
                .zip(log)
                .map(|(p, l)| {
                    assert_eq!(p.kind, Kind::Scale, "{} is not a multiplier", p.name);
                    l.exp().clamp(p.min, p.max)
                })
                .collect(),
        }
    }

    /// The natural-log multipliers of this vector.
    pub fn to_log(&self) -> Vec<f64> {
        self.values.iter().map(|v| v.ln()).collect()
    }

    /// The search box in log space: `(ln min, ln max)` per parameter.
    pub fn log_bounds() -> Vec<(f64, f64)> {
        PARAMS.iter().map(|p| (p.min.ln(), p.max.ln())).collect()
    }

    /// Write this vector as a training-only TOML file: `[ecology]` with one
    /// `"name" = multiplier` line per parameter.
    pub fn save(&self, path: &std::path::Path, note: &str) -> Result<(), String> {
        let mut text = String::new();
        for line in note.lines() {
            text.push_str(&format!("# {line}\n"));
        }
        text.push_str("[ecology]\n");
        for (p, v) in PARAMS.iter().zip(&self.values) {
            text.push_str(&format!("\"{}\" = {v}\n", p.name));
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    /// Read a file [`EcologyParams::save`] wrote. Parameters it does not name keep their
    /// defaults; an unknown name or an out-of-range value is refused.
    pub fn load(path: &std::path::Path) -> Result<EcologyParams, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let doc: toml::Table = text
            .parse()
            .map_err(|e| format!("{} is not TOML: {e}", path.display()))?;
        let table = doc
            .get("ecology")
            .and_then(|t| t.as_table())
            .ok_or_else(|| format!("{} has no [ecology] table", path.display()))?;
        let mut e = EcologyParams::defaults();
        for (name, value) in table {
            let v = value
                .as_float()
                .or_else(|| value.as_integer().map(|i| i as f64))
                .ok_or_else(|| format!("{name} is not a number"))?;
            e.set(name, v)?;
        }
        Ok(e)
    }

    /// `name=value` pairs, for a log line.
    pub fn describe(&self) -> String {
        PARAMS
            .iter()
            .zip(&self.values)
            .map(|(p, v)| format!("{}={v}", p.name))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn v(&self, name: &str) -> f64 {
        self.get(name).expect("a listed parameter")
    }

    /// The fauna config this vector builds, from the shipped default.
    pub fn fauna_config(&self) -> FaunaConfig {
        let mut c = FaunaConfig::default();
        let y = self.v("founder.yield_by_food");
        let floor = self.v("founder.reproduction.surplus_floor");
        for f in Founder::ALL {
            let phys = &mut c.founders[f.index()];
            for v in phys.yield_by_food.iter_mut() {
                *v *= y;
            }
            phys.core.reproduction.surplus_floor *= floor;
        }
        c
    }

    /// The flora config this vector builds on top of `base` (the preset's own, scaled
    /// for its voxel size).
    pub fn flora_config(&self, mut base: FloraConfig) -> FloraConfig {
        let regrow = self.v("flora.foliage_rate");
        let fall = self.v("flora.senescence");
        base.decomposition *= self.v("flora.decomposition");
        for s in Species::ALL {
            let sc = base.species_mut(s);
            if sc.trophic == Trophic::Photo {
                sc.foliage_rate *= regrow;
                sc.senescence *= fall;
            }
        }
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_build_the_shipped_configs() {
        let e = EcologyParams::defaults();
        assert_eq!(e.values().len(), PARAMS.len());
        let fauna = e.fauna_config();
        let shipped = FaunaConfig::default();
        for f in Founder::ALL {
            assert_eq!(
                fauna.founder(f).yield_by_food,
                shipped.founder(f).yield_by_food
            );
            assert_eq!(
                fauna.founder(f).core.reproduction.surplus_floor,
                shipped.founder(f).core.reproduction.surplus_floor
            );
        }
        let base = FloraConfig::for_voxel_size(0.125);
        let flora = e.flora_config(base.clone());
        assert_eq!(flora.decomposition, base.decomposition);
        for s in Species::ALL {
            assert_eq!(flora.species(s).foliage_rate, base.species(s).foliage_rate);
        }
    }

    #[test]
    fn a_routed_parameter_reaches_its_layer_and_a_bad_one_is_refused() {
        let mut e = EcologyParams::defaults();
        e.set("flora.foliage_rate", 2.0).expect("in range");
        let base = FloraConfig::for_voxel_size(0.125);
        let flora = e.flora_config(base.clone());
        let s = Species::ALL
            .into_iter()
            .find(|s| base.species(*s).trophic == Trophic::Photo)
            .expect("a plant");
        assert_eq!(
            flora.species(s).foliage_rate,
            2.0 * base.species(s).foliage_rate
        );
        assert!(e.set("flora.foliage_rate", 99.0).is_err());
        assert!(e.set("no.such", 1.0).is_err());
    }
}
