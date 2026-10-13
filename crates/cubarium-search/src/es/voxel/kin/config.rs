//! **The kin knobs**, per lineage: the shares `r` (§10.3), the comparison switch (§11.7,
//! topic 10), the `twins` comparison's layout and the brain stage (§10.4). Training knobs, like curiosity's: a TOML file of
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
    /// **All-sign twins** (LIVE-TWINS; LJ-TWIN's measured comparison): at each world's
    /// window boundary the world is captured as one saved start; from it, two copies run a
    /// short twin window ([`TwinKnobs::window_s`]) with **every body of the lineage** on
    /// `+ε_p` in one copy and on `−ε_p` in the other (its newborns too), every other body on
    /// its lineage's centre or heuristic. A copy's score is the lineage's **summed**
    /// kin-weighted score over the window (every body of the lineage the copy metered,
    /// newborns included, relatives counted as the kin shares say) **over the number of
    /// bodies put on the variant at the start**: summed, so a birth or a death in one twin
    /// moves the score by what it did to the lineage rather than by how it changed a head
    /// count (a mean over the bodies alive would score a birth as a lower average); over the
    /// start's head count, a constant shared by both twins, so a start with many bodies does
    /// not outweigh one with few. The twin's measurement is `+` minus `−`
    /// ([`super::twins`]). The live world itself then runs its window with this lineage on
    /// its centre, so the worlds live on as before. Distinct from the focal `matched`.
    ///
    /// **Half-and-half by default** (TWIN-MIX): with [`TwinKnobs::carrier_share`] below 1
    /// only a drawn subset carries `±ε_p` and the twin is scored by
    /// [`super::twins::estimate`] (the carrier's direct effect, its kin-weighted effect on
    /// relatives, and [`TwinKnobs::stranger_weight`] times its effect on the rest); the
    /// all-sign twin above is `carrier_share = 1, stranger_weight = 1`.
    Twins,
}

/// **The twins comparison's layout** (LIVE-TWINS): how many twin measurements a
/// generation makes and how long each runs. Read only by a lineage on
/// [`Comparison::Twins`]; every other lineage ignores it. The defaults are the layout chosen
/// from LJ-TWIN's measured ICCs (`design/handoffs/live-twins-2026-10-13.md`; the run's
/// handback says why).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TwinKnobs {
    /// One twin window, simulated seconds (each copy runs this long from its start).
    pub window_s: f64,
    /// Antithetic pairs drawn per generation (the lineage's table has `2·pairs` variants).
    pub pairs: usize,
    /// Starts each pair is measured on (distinct worlds' boundary starts; at most the
    /// run's worlds). A pair's score is the mean of its measurements.
    pub starts: usize,
    /// Null twins per generation: both copies on candidate 0 from one start, the
    /// determinism floor (their gap must be 0); readout only, never in the update.
    pub nulls: usize,
    /// **Carriers** (TWIN-MIX): the share of the lineage's bodies at the start that carry
    /// the variant in a twin (fraction, `(0, 1]`); the rest run the centre in both copies.
    /// A fresh subset is drawn for every twin from its seed, the same in both copies
    /// ([`super::twins::carriers`]). `1`: every body, all-sign twins exactly (then
    /// [`TwinKnobs::stranger_weight`] must be 1).
    pub carrier_share: f64,
    /// **The stranger dial** (TWIN-MIX): the weight on the carriers' social effect on the
    /// lineage's other bodies (fraction, `[0, 1]`). `0`: a twin scores the carrier's direct
    /// effect plus its kin-weighted effect on relatives, topic 2 exactly; `1`: the effect on
    /// every other body counts in full, relatives at `1 + r` (all-sign twins' weighting).
    /// The estimator is [`super::twins::estimate`].
    pub stranger_weight: f64,
}

impl Default for TwinKnobs {
    fn default() -> TwinKnobs {
        TwinKnobs { window_s: 120.0, pairs: 1024, starts: 1, nulls: 1, carrier_share: 0.5, stranger_weight: 0.0 }
    }
}

impl TwinKnobs {
    pub fn is_default(&self) -> bool {
        *self == TwinKnobs::default()
    }

    /// Every body carries the variant: the all-sign twins of LIVE-TWINS, scored as before.
    pub fn all_sign(&self) -> bool {
        self.carrier_share >= 1.0
    }

    /// The knobs that the registry's ranges cannot say: at `carrier_share = 1` no body is
    /// left to tell a carrier's direct effect from its social one, so the score is the
    /// all-sign sum and the dial must say so (`stranger_weight = 1`).
    pub fn check(&self) -> Result<(), String> {
        if self.all_sign() && self.stranger_weight != 1.0 {
            return Err(format!(
                "twins: carrier_share = 1 scores every body (all-sign twins), which is stranger_weight = 1; \
                 stranger_weight = {} needs carrier_share below 1",
                self.stranger_weight
            ));
        }
        Ok(())
    }
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
    /// The `twins` comparison's layout (`[<lineage>.twins]`); left out of a written
    /// config at its default, so a run that never names it writes what it wrote before.
    #[serde(skip_serializing_if = "TwinKnobs::is_default")]
    pub twins: TwinKnobs,
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
        for f in Founder::ALL {
            c.of(f).twins.check().map_err(|why| format!("[{}]: {why}", f.name()))?;
        }
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
        Row::new("kin.lineages.*.twins.window_s", "s", Range::closed(1.0, 3600.0),
            "LIVE-TWINS: one twin window on the `twins` comparison, simulated seconds per copy"),
        Row::new("kin.lineages.*.twins.pairs", "count", Range::closed(1.0, 65536.0),
            "LIVE-TWINS: antithetic pairs per generation on the `twins` comparison"),
        Row::new("kin.lineages.*.twins.starts", "count", Range::closed(1.0, 4096.0),
            "LIVE-TWINS: distinct boundary starts each pair is measured on (at most the run's worlds)"),
        Row::new("kin.lineages.*.twins.nulls", "count", Range::closed(0.0, 4096.0),
            "LIVE-TWINS: null twins (both copies on one candidate) per generation, the determinism readout"),
        Row::new("kin.lineages.*.twins.carrier_share", "fraction", Range::fraction().open_lo(),
            "TWIN-MIX: share of the lineage's bodies at a twin's start drawn to carry the variant (the rest run the centre in both copies); 1 = all-sign twins"),
        Row::new("kin.lineages.*.twins.stranger_weight", "fraction", Range::fraction(),
            "TWIN-MIX: weight on the carriers' social effect on the lineage's other bodies; 0 = direct + kin-weighted relatives (topic 2), 1 = every body in full (all-sign)"),
    ]
}

/// The standalone table, root `kin`.
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| Registry::new("kin", kin_rows(), Vec::new(), Vec::new()))
}
