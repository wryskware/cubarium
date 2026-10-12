//! **Holding an intent** (§2.2, §11.3): the per-lineage knobs and the pure rule that
//! settles the held intent and subject from the top's proposal.
//!
//! The rule reads **probabilities at a fixed temperature**, never raw logits (Astra review
//! #11): `hold_s` is a minimum hold before the top may switch on its own account, and
//! `switch_margin` is how much more probability the proposal must carry than the held
//! choice. A hold ends early, whatever its age, when the fauna says so (subject lost or
//! dead, intent masked, a physical emergency) or when the brain itself finds its subject
//! gone or invalid or its intent masked; the top then chooses afresh.

use serde::{Deserialize, Serialize};

use cubarium_voxel::registry::{Range, Registry, Row};

/// A lineage's intent-holding knobs. Per-lineage, not trained weights.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IntentArbitration {
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

impl Default for IntentArbitration {
    fn default() -> IntentArbitration {
        IntentArbitration {
            intent_hz: 4.0,
            hold_s: 1.0,
            switch_margin: 0.1,
            temperature: 1.0,
            intent_time_scale_s: 10.0,
        }
    }
}

impl IntentArbitration {
    /// Refuse an out-of-range set by name ([`registry`]'s ranges).
    pub fn validate(&self) -> Result<(), String> {
        registry().check(self)
    }
}

/// The knobs' registry rows, under `prefix` (`"brain"` standalone; the adapter mounts
/// them per lineage, e.g. `fauna.founders.*.brain`).
pub fn arbitration_rows(prefix: &str) -> Vec<Row> {
    vec![
        Row::new("intent_hz", "Hz", Range::positive(), "TWO-LAYER: top-layer decision rate; the bottom runs every controller sample"),
        Row::new("hold_s", "s", Range::nonneg(), "TWO-LAYER: minimum hold before the top may switch intent or subject on its own account (an early end ignores it)"),
        Row::new("switch_margin", "probability", Range::fraction(), "TWO-LAYER: a proposal must beat the held choice's probability by this, both at `temperature`"),
        Row::new("temperature", "-", Range::positive(), "TWO-LAYER: fixed softmax temperature the hold and margin read probabilities at"),
        Row::new("intent_time_scale_s", "s", Range::positive(), "TWO-LAYER: time-in-intent input is 1 - exp(-t / this)"),
    ]
    .into_iter()
    .map(|r| r.under(prefix))
    .collect()
}

/// The standalone table, root `brain`. The adapter (second half) mounts the same rows per
/// lineage in the fauna's table; until then this is where the rows live.
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| Registry::new("brain", arbitration_rows("brain"), Vec::new(), Vec::new()))
}

/// Why the fauna ends a hold early (the interface's early-end flag).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EarlyEnd {
    /// The subject was lost (unseen past `subject_lost_s`) or died.
    SubjectLost,
    /// The held intent became masked.
    IntentMasked,
    /// A physical emergency: DROP, wade-out, a hit.
    Emergency,
}

/// Why the held intent or subject changed at a decision, for the readouts (§10.4: hold
/// durations, switch rates and why holds ended).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HoldChange {
    /// The first choice of a fresh brain.
    Start,
    /// The fauna's early-end flag.
    Told(EarlyEnd),
    /// The held subject is not among this sample's candidates.
    SubjectGone,
    /// The held subject's row is not valid for the held intent.
    SubjectInvalid,
    /// The held intent is masked in this sample.
    IntentMasked,
    /// After `hold_s`, the proposal's intent beat the held one by the margin.
    Outscored,
    /// After `hold_s`, under the same intent, the proposal's subject beat the held one by
    /// the margin.
    SubjectOutscored,
}

impl HoldChange {
    /// Whether this change ended the hold early (forced a fresh choice).
    pub fn is_early(self) -> bool {
        !matches!(
            self,
            HoldChange::Start | HoldChange::Outscored | HoldChange::SubjectOutscored
        )
    }
}

/// The top's proposal at one decision, as probabilities at the fixed temperature.
#[derive(Clone, Copy, Debug)]
pub struct ProposalView<'a> {
    /// Intent probabilities, `M'` long, masked entries 0.
    pub intent_probs: &'a [f64],
    /// The proposed intent (the masked argmax).
    pub intent: usize,
    /// The pointer's probabilities for the proposed intent: `K` rows then "none" last;
    /// empty when the lineage has no candidates.
    pub subject_probs: &'a [f64],
    /// The proposed subject's row (`None`: no subject).
    pub subject: Option<usize>,
}

impl ProposalView<'_> {
    fn subject_prob(&self, subject: Option<usize>) -> f64 {
        if self.subject_probs.is_empty() {
            return 0.0;
        }
        let none = self.subject_probs.len() - 1;
        self.subject_probs[subject.unwrap_or(none)]
    }
}

/// The hold in force before a decision: its intent, its subject's row in this sample,
/// and how long it has been held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoldView {
    pub intent: usize,
    pub subject: Option<usize>,
    pub hold_age_s: f64,
}

/// What a decision does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settle {
    /// Keep the held intent and subject.
    Keep,
    /// Take the proposal's intent and subject, for this reason.
    Take(HoldChange),
}

/// The rule. `hold` is `None` for a fresh brain; `forced` is an early end found by the
/// fauna or the brain. Deterministic, and it reads only probabilities.
pub fn settle(
    cfg: &IntentArbitration,
    hold: Option<HoldView>,
    forced: Option<HoldChange>,
    p: &ProposalView<'_>,
) -> Settle {
    let Some(h) = hold else {
        return Settle::Take(HoldChange::Start);
    };
    if let Some(reason) = forced {
        return Settle::Take(reason);
    }
    // A small tolerance so a hold of exactly `hold_s` (accumulated sample periods) counts.
    if h.hold_age_s + 1e-9 < cfg.hold_s {
        return Settle::Keep;
    }
    if p.intent != h.intent {
        return if p.intent_probs[p.intent] >= p.intent_probs[h.intent] + cfg.switch_margin {
            Settle::Take(HoldChange::Outscored)
        } else {
            Settle::Keep
        };
    }
    if p.subject != h.subject && p.subject_prob(p.subject) >= p.subject_prob(h.subject) + cfg.switch_margin
    {
        return Settle::Take(HoldChange::SubjectOutscored);
    }
    Settle::Keep
}
