//! **A lineage's brain table** (TWO-LAYER, `design/behaviour-ai-2026-10-11.md` §2.2, §10.1,
//! §11.3): the knobs a two-layer brain holds its intent by, mounted per lineage at
//! `fauna.founders.<lineage>.brain`. The brain itself lives in `cubarium-search`
//! (`es::voxel::two_layer`), which reads this table as its `IntentArbitration`; it sits
//! here so the per-lineage config and its registry rows are the fauna's, like every other
//! per-lineage table.
//!
//! These are the **brain's** hold and margin. The rules' own records hold their proposals
//! by [`crate::IntentKnobs`] (`fauna.founders.<lineage>.intent`); both start at the same
//! values (§11.3: the brain runs the same hold and margin as the teacher, once).

use serde::{Deserialize, Serialize};

use cubarium_voxel::registry::{Range, Registry, Row};

/// A lineage's intent-holding knobs for its two-layer brain. Per-lineage, not trained
/// weights. A plain lineage's brain has no top and reads none of them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrainArbitration {
    /// The top layer's rate, decisions per second (Hz). The bottom runs at every controller
    /// sample; the top decides on a credit that accrues `sample period × intent_hz` per
    /// sample (§2.2: 4 Hz).
    pub intent_hz: f64,
    /// Minimum hold, seconds: a held intent or subject is not switched on the top's own
    /// account before it has been held this long. An early end ignores it.
    pub hold_s: f64,
    /// Switch margin, probability (`[0, 1]`): a proposal replaces the held choice only when
    /// its probability exceeds the held choice's by at least this, both at `temperature`.
    pub switch_margin: f64,
    /// The fixed softmax temperature the margin is read at (dimensionless, > 0). Fixed, so
    /// a change of logit scale is read as a change of confidence at a known calibration.
    pub temperature: f64,
    /// Time scale of the bottom's and top's time-in-intent input, seconds: the input is
    /// `1 − exp(−t / scale)` for `t` seconds in the current intent.
    pub intent_time_scale_s: f64,
}

impl Default for BrainArbitration {
    fn default() -> BrainArbitration {
        BrainArbitration {
            intent_hz: 4.0,
            hold_s: 1.0,
            switch_margin: 0.1,
            temperature: 1.0,
            intent_time_scale_s: 10.0,
        }
    }
}

impl BrainArbitration {
    /// Refuse an out-of-range set by name ([`registry`]'s ranges).
    pub fn validate(&self) -> Result<(), String> {
        registry().check(self)
    }
}

/// The knobs' registry rows, under `prefix` (`"brain"` standalone; the fauna table mounts
/// them per lineage at `fauna.founders.<lineage>.brain`).
pub fn arbitration_rows(prefix: &str) -> Vec<Row> {
    vec![
        Row::new("intent_hz", "Hz", Range::positive(), "TWO-LAYER: top-layer decision rate; the bottom runs every controller sample"),
        Row::new("hold_s", "s", Range::nonneg(), "TWO-LAYER: minimum hold before the brain's top may switch intent or subject on its own account (an early end ignores it)"),
        Row::new("switch_margin", "probability", Range::fraction(), "TWO-LAYER: a brain's proposal must beat the held choice's probability by this, both at `temperature`"),
        Row::new("temperature", "-", Range::positive(), "TWO-LAYER: fixed softmax temperature the brain's hold and margin read probabilities at"),
        Row::new("intent_time_scale_s", "s", Range::positive(), "TWO-LAYER: the brain's time-in-intent input is 1 - exp(-t / this)"),
    ]
    .into_iter()
    .map(|r| r.under(prefix))
    .collect()
}

/// The standalone table, root `brain`, for checking one set on its own.
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| Registry::new("brain", arbitration_rows("brain"), Vec::new(), Vec::new()))
}
