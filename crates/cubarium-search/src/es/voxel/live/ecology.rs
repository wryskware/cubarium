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
    // Added by the ecology search (2026-09-24): with no mortality but starvation and
    // drowning, the five above cannot bring a world's carrying capacity near the band;
    // an animal's upkeep sets it directly (capacity ~ production · yield / upkeep).
    ParamSpec {
        name: "founder.maintenance_per_s",
        kind: Kind::Scale,
        min: 0.25,
        max: 4.0,
        what: "animal upkeep: every founder's maintenance_per_s",
    },
    // Added with the lifespan and the grazing defence (2026-09-25).
    ParamSpec {
        name: "founder.lifespan_median_s",
        kind: Kind::Scale,
        min: 0.25,
        max: 4.0,
        what: "median lifespan: every body kind's lifespan_median_s (4 h shipped)",
    },
    ParamSpec {
        name: "flora.defence_per_bite",
        kind: Kind::Scale,
        min: 0.25,
        max: 4.0,
        what: "grazing defence rise per bite, every species",
    },
    ParamSpec {
        name: "flora.defence_half_life_s",
        kind: Kind::Scale,
        min: 0.25,
        max: 4.0,
        what: "grazing defence half-life, every species (1.5 h shipped)",
    },
    // The audit round (2026-09-28, audit §8.1): walking is priced as a multiple of
    // upkeep, so an upkeep multiplier no longer silently changes what a metre costs.
    ParamSpec {
        name: "founder.motor_factor",
        kind: Kind::Scale,
        min: 0.5,
        max: 4.0,
        what: "walking cost as a multiple of upkeep, on the shipped ratio (1)",
    },
    // The audit round (2026-09-28, audit §1.4, §8.2): the animal's size in org-units
    // against the plants'. It multiplies the nine absolute animal quantities
    // (`MASS_SCALED`) and leaves every per-unit rate, clock and metre alone, so one spot
    // can run out while walking still pays.
    ParamSpec {
        name: "founder.mass_scale",
        kind: Kind::Scale,
        min: 1.0,
        max: 1000.0,
        what: "animal org-unit mass: the nine absolute animal quantities (x1 shipped)",
    },
];

/// The absolute animal quantities `founder.mass_scale` multiplies (audit §1.4), by name.
pub const MASS_SCALED: [&str; 9] = [
    "body_max",
    "body_min",
    "birth_body",
    "birth_cost",
    "surplus_floor",
    "egg_organic",
    "bite_per_s",
    "bite_half_stock",
    "growth_max_per_s",
];

/// The founder parameters that also take a **per-lineage** multiplier
/// (`name@<lineage>`, or an `[ecology.<lineage>]` table in a file), on top of the shared
/// one (`design/handoffs/voxel-browser-movement-2026-09-28.md`, 2). The two multiply.
pub const LINEAGE_PARAMS: [&str; 5] = [
    "founder.yield_by_food",
    "founder.maintenance_per_s",
    "founder.lifespan_median_s",
    "founder.motor_factor",
    "founder.mass_scale",
];

/// One world's parameter vector, aligned with [`PARAMS`], plus the per-lineage
/// multipliers of [`LINEAGE_PARAMS`] (every one `1.0` by default).
#[derive(Clone, Debug, PartialEq)]
pub struct EcologyParams {
    values: Vec<f64>,
    per_lineage: Vec<[f64; LINEAGE_PARAMS.len()]>,
}

/// `name@lineage` split into the shared name, the lineage and its slot in
/// [`LINEAGE_PARAMS`].
fn split_lineage(name: &str) -> Result<Option<(&str, Founder, usize)>, String> {
    let Some((base, lineage)) = name.split_once('@') else {
        return Ok(None);
    };
    let f = Founder::ALL
        .into_iter()
        .find(|f| f.name() == lineage)
        .ok_or_else(|| format!("{name}: no lineage is called {lineage:?}"))?;
    let slot = LINEAGE_PARAMS
        .iter()
        .position(|p| *p == base)
        .ok_or_else(|| format!("{name}: {base} has no per-lineage setting"))?;
    Ok(Some((base, f, slot)))
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
            per_lineage: vec![[1.0; LINEAGE_PARAMS.len()]; Founder::COUNT],
        }
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        if let Ok(Some((_, f, slot))) = split_lineage(name) {
            return Some(self.per_lineage[f.index()][slot]);
        }
        PARAMS
            .iter()
            .position(|p| p.name == name)
            .map(|i| self.values[i])
    }

    /// Set one parameter, refusing an unknown name or a value outside its range.
    pub fn set(&mut self, name: &str, value: f64) -> Result<(), String> {
        if let Some((base, f, slot)) = split_lineage(name)? {
            let p = PARAMS
                .iter()
                .find(|p| p.name == base)
                .expect("a listed parameter");
            if !(value.is_finite() && (p.min..=p.max).contains(&value)) {
                return Err(format!(
                    "{name} = {value} is outside [{}, {}]",
                    p.min, p.max
                ));
            }
            self.per_lineage[f.index()][slot] = value;
            return Ok(());
        }
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
            per_lineage: vec![[1.0; LINEAGE_PARAMS.len()]; Founder::COUNT],
        }
    }

    /// The search's dimensions: every shared parameter except the [`LINEAGE_PARAMS`],
    /// then each of those once per lineage (`name@lineage`).
    pub fn search_names() -> Vec<String> {
        let mut out: Vec<String> = PARAMS
            .iter()
            .filter(|p| !LINEAGE_PARAMS.contains(&p.name))
            .map(|p| p.name.to_string())
            .collect();
        for f in Founder::ALL {
            for p in LINEAGE_PARAMS {
                out.push(format!("{p}@{}", f.name()));
            }
        }
        out
    }

    /// `(ln min, ln max)` per [`EcologyParams::search_names`] entry.
    pub fn search_bounds() -> Vec<(f64, f64)> {
        EcologyParams::search_names()
            .iter()
            .map(|n| {
                let base = n.split('@').next().expect("a name");
                let p = PARAMS.iter().find(|p| p.name == base).expect("listed");
                (p.min.ln(), p.max.ln())
            })
            .collect()
    }

    /// A vector from natural-log multipliers over [`EcologyParams::search_names`], each
    /// clamped to its range; zeros are the defaults.
    pub fn from_search_log(log: &[f64]) -> EcologyParams {
        let names = EcologyParams::search_names();
        assert_eq!(log.len(), names.len(), "one value per search dimension");
        let mut e = EcologyParams::defaults();
        for ((n, l), (lo, hi)) in names.iter().zip(log).zip(EcologyParams::search_bounds()) {
            e.set(n, l.clamp(lo, hi).exp().clamp(lo.exp(), hi.exp()))
                .expect("clamped into range");
        }
        e
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
        for f in Founder::ALL {
            text.push_str(&format!("[ecology.{}]\n", f.name()));
            for (p, v) in LINEAGE_PARAMS.iter().zip(&self.per_lineage[f.index()]) {
                text.push_str(&format!("\"{p}\" = {v}\n"));
            }
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
            // An `[ecology.<lineage>]` table: that lineage's own multipliers.
            if let Some(sub) = value.as_table() {
                for (k, v) in sub {
                    let v = v
                        .as_float()
                        .or_else(|| v.as_integer().map(|i| i as f64))
                        .ok_or_else(|| format!("{name}.{k} is not a number"))?;
                    e.set(&format!("{k}@{name}"), v)?;
                }
                continue;
            }
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
        let upkeep = self.v("founder.maintenance_per_s");
        let life = self.v("founder.lifespan_median_s");
        c.frondgrazer.lifespan_median_s *= life;
        // The placeholder species is the browser's body.
        c.frondgrazer.lifespan_median_s *= self.per_lineage[Founder::Browser.index()][2];
        let mass = self.v("founder.mass_scale");
        c.frondgrazer
            .scale_mass(mass * self.per_lineage[Founder::Browser.index()][4]);
        for f in Founder::ALL {
            let own = self.per_lineage[f.index()];
            let phys = &mut c.founders[f.index()];
            for v in phys.yield_by_food.iter_mut() {
                *v *= y * own[0];
            }
            phys.core.reproduction.surplus_floor *= floor;
            phys.core.maintenance_per_s *= upkeep * own[1];
            // Walking is priced against upkeep: it keeps its shipped ratio to it, times
            // the motor factor.
            phys.motor_respiration_per_s *=
                upkeep * own[1] * self.v("founder.motor_factor") * own[3];
            phys.core.lifespan_median_s *= life * own[2];
            phys.scale_mass(mass * own[4]);
        }
        c
    }

    /// The flora config this vector builds on top of `base` (the preset's own, scaled
    /// for its voxel size).
    pub fn flora_config(&self, mut base: FloraConfig) -> FloraConfig {
        let regrow = self.v("flora.foliage_rate");
        let fall = self.v("flora.senescence");
        base.decomposition *= self.v("flora.decomposition");
        let rise = self.v("flora.defence_per_bite");
        let half = self.v("flora.defence_half_life_s");
        for s in Species::ALL {
            let sc = base.species_mut(s);
            sc.defence_per_bite *= rise;
            sc.defence_half_life_s *= half;
        }
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
