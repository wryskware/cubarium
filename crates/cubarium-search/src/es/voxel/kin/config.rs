//! **The kin knobs**, per lineage: the shares `r` (§10.3), the comparison switch (§11.7,
//! topic 10) and the brain stage (§10.4). Training knobs, like curiosity's: a TOML file of
//! one `[<lineage>]` table per lineage (`--kin FILE`), nothing in the sim or a save.
//!
//! **The defaults are today's training exactly**: every share 0 and the `per_body`
//! comparison. The design's "0.5 to start" is the value the kin file sets for the lineages
//! that train with kin (`config/training/kin-2026-10-12.toml`), not a code default, so a
//! lineage nobody configured trains as it does today.

use std::path::Path;

use cubarium_voxel::registry::{Range, Registry, Row};
use cubarium_voxel_fauna::Founder;
use serde::{Deserialize, Serialize};

use super::roster::Relation;

/// The share `r` of each relative's own base score added to a body's score, by how the
/// relative is related to it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KinShares {
    /// Share of each of its calves' base scores (fraction, `[0, 1]`).
    pub calf: f64,
    /// Share of its mother's base score (fraction, `[0, 1]`).
    pub mother: f64,
    /// Share of each sibling's base score (fraction, `[0, 1]`).
    pub sibling: f64,
    /// Share of each packmate's base score (fraction, `[0, 1]`).
    pub packmate: f64,
}

impl KinShares {
    /// The share for `relation`.
    pub fn of(&self, relation: Relation) -> f64 {
        match relation {
            Relation::Calf => self.calf,
            Relation::Mother => self.mother,
            Relation::Sibling => self.sibling,
            Relation::Packmate => self.packmate,
        }
    }

    /// Every share 0: the score is the body's own, as today.
    pub fn is_zero(&self) -> bool {
        Relation::ALL.iter().all(|r| self.of(*r) == 0.0)
    }

    /// The same share for every relation.
    pub fn uniform(r: f64) -> KinShares {
        KinShares { calf: r, mother: r, sibling: r, packmate: r }
    }
}

/// How a lineage's variants are compared (§11.7, topic 10: a switch, not a commitment).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    /// Each body its own variant, scores z-scored within each world and lineage and pooled
    /// as today ([`crate::es::voxel::live::trainer::pooled_scores`]). The live windows'
    /// default; with shares above 0 and one pack per world the shared kin term cancels.
    #[default]
    PerBody,
    /// One shared variant per family or pack ([`super::Roster::families`]), compared raw
    /// across worlds (topic 2's fallback).
    Family,
    /// Matched copies from one saved start: one focal body on `+ε` in one copy and `−ε`
    /// in the other, everything else identical ([`super::matched`]). The staged
    /// encounters' default.
    Matched,
}

/// Which layers of a two-layer brain the variants perturb (§10.4). A plain brain has no
/// top and ignores it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrainStage {
    /// Stage 1: only the top; the bottom stays at its copy.
    #[default]
    Top,
    /// Stage 2: both layers.
    Both,
}

/// One lineage's kin knobs: a `[<lineage>]` table of the kin file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LineageKin {
    pub shares: KinShares,
    pub comparison: Comparison,
    pub stage: BrainStage,
}

impl LineageKin {
    /// Whether this lineage trains exactly as it did before KIN-TRAIN.
    pub fn is_today(&self) -> bool {
        self.shares.is_zero() && self.comparison == Comparison::PerBody
    }
}

/// Every lineage's kin knobs, in [`Founder::ALL`] order. The default is today's training.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KinConfig {
    pub lineages: [LineageKin; Founder::COUNT],
}

impl KinConfig {
    pub fn of(&self, f: Founder) -> &LineageKin {
        &self.lineages[f.index()]
    }

    /// Read a kin file's text: one `[<lineage>]` table per lineage by its code name; a
    /// lineage left out keeps the default. Every share is range-checked by [`registry`].
    pub fn parse(text: &str) -> Result<KinConfig, String> {
        let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut c = KinConfig::default();
        for (name, value) in table {
            let f = Founder::ALL
                .into_iter()
                .find(|f| f.name() == name)
                .ok_or_else(|| {
                    format!(
                        "unknown lineage [{name}]; the lineages are {}",
                        Founder::ALL.map(|f| f.name()).join(", ")
                    )
                })?;
            let l: LineageKin = value
                .try_into()
                .map_err(|e: toml::de::Error| format!("[{name}]: {e}"))?;
            c.lineages[f.index()] = l;
        }
        registry().check(&c)?;
        Ok(c)
    }

    pub fn load(path: &Path) -> Result<KinConfig, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        KinConfig::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// `--kin FILE`, or today's training without one.
    pub fn from_flag(path: Option<&Path>) -> Result<KinConfig, String> {
        path.map_or_else(|| Ok(KinConfig::default()), KinConfig::load)
    }
}

/// The kin knobs' registry rows (root `kin`; `lineages.*` is the lineage by index, in
/// [`Founder::ALL`] order). Training-side, like the two-layer `brain` table.
pub fn kin_rows() -> Vec<Row> {
    let share = |name: &'static str, what: &'static str| {
        Row::new(format!("kin.lineages.*.shares.{name}"), "fraction", Range::fraction(), what)
    };
    vec![
        share("calf", "KIN-TRAIN: share of each calf's own base score (E_end - E_start + E_given)/body_ref added to a body's training score; 0 = today"),
        share("mother", "KIN-TRAIN: share of its mother's own base score added to a body's training score; 0 = today"),
        share("sibling", "KIN-TRAIN: share of each sibling's (shared recorded mother id) own base score added to a body's training score; 0 = today"),
        share("packmate", "KIN-TRAIN: share of each packmate's own base score added to a body's training score; 0 = today"),
    ]
}

/// The standalone table, root `kin`.
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| Registry::new("kin", kin_rows(), Vec::new(), Vec::new()))
}
