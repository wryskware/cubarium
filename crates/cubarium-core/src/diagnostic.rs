//! A transient diagnostic seam: substitute part of one organism's per-tick *intent*.
//!
//! Milestone R0b (`design/handoffs/r0b-opus-2026-09-14.md`) asks for a scripted-probe fixture
//! that can exercise physical travel and a continuous feeding request without the controller's
//! own `feed_min` gate. Nothing in the world could state such an intent, so this is the
//! smallest seam that makes one expressible.
//!
//! # What it is, and what it deliberately is not
//!
//! - **It is an intent, never a result.** Every field lands in [`crate::controller::Decision`]
//!   immediately after the ordinary controller has decided, and is then answered by exactly
//!   the same machinery as any other decision: the heading and effort go through
//!   [`crate::motor::resolve`] under `|v| + r · |ω| ≤ u`, the intake efforts go through the
//!   world's own per-cell share, type-II term and reserve headroom, and `bud` still faces the
//!   capacity and funding checks. A script cannot teleport a body, align it for free, take
//!   more than a cell holds, or fund a birth the world refuses.
//! - **It is transient.** The overrides live on [`crate::World`], never in
//!   [`crate::WorldState`], so they are not serialized, not migrated, not resumed and not
//!   visible to a snapshot. An empty list — the default, and the only state any ordinary
//!   world is ever in — is a single `is_empty()` test per tick and changes no draw, no
//!   counter and no trajectory.
//! - **It is not live policy.** Nothing in the world sets one. It exists for
//!   `examples/mobile_grazing.rs` and for tests, and the ordinary controller, the display and
//!   the persisted world are unaware of it.
//!
//! An override applies *before* the hunter, escape and encounter passes, so a legitimate
//! override still wins over a script exactly as it wins over the controller.

use cubarium_surface::Vec2;

use crate::controller::Decision;
use crate::organism::Mode;

/// A partial substitution of one organism's decided intent for one tick. Every field is
/// `None` by default, which leaves the controller's own answer in place.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScriptedIntent {
    /// Target orientation to request. The resolver turns toward it by as much as the shared
    /// budget allows, so a script states where a body *wants* to face, not where it ends up.
    pub heading: Option<Vec2>,
    /// Movement effort in `[0, 1]`. Zero is a genuine request for stillness.
    pub effort: Option<f64>,
    /// Grazing intake effort in `[0, 1]`, bypassing the controller's `feed_min` gate but not
    /// the world's per-cell availability, share or headroom.
    pub graze_effort: Option<f64>,
    /// Fruit intake effort in `[0, 1]`, same terms.
    pub fruit_effort: Option<f64>,
    /// Detritus intake effort in `[0, 1]`, same terms.
    pub scavenge_effort: Option<f64>,
    /// The published mode label. A label alone grants nothing: effort and the intake efforts
    /// above are what the world reads.
    pub mode: Option<Mode>,
    /// Whether to request gestation. `Some(false)` is how a capability fixture disables
    /// births without touching the world's reproduction rules.
    pub bud: Option<bool>,
}

impl ScriptedIntent {
    /// Stand still, ask for nothing, start nothing: the intent a stationary arm states.
    pub fn still(heading: Vec2) -> ScriptedIntent {
        ScriptedIntent {
            heading: Some(heading),
            effort: Some(0.0),
            bud: Some(false),
            ..ScriptedIntent::default()
        }
    }

    /// Apply the substitution to a decided intent. Efforts are clamped to `[0, 1]` and a
    /// non-finite value is ignored, so a script cannot inject a degenerate request.
    pub(crate) fn apply(&self, d: &mut Decision) {
        if let Some(h) = self.heading
            && h.is_finite()
        {
            d.heading = h;
        }
        if let Some(e) = self.effort {
            d.effort = clamp_unit(e, d.effort);
        }
        if let Some(e) = self.graze_effort {
            d.graze_effort = clamp_unit(e, d.graze_effort);
        }
        if let Some(e) = self.fruit_effort {
            d.fruit_effort = clamp_unit(e, d.fruit_effort);
        }
        if let Some(e) = self.scavenge_effort {
            d.scavenge_effort = clamp_unit(e, d.scavenge_effort);
        }
        if let Some(m) = self.mode {
            d.mode = m;
        }
        if let Some(b) = self.bud {
            d.bud = d.bud && b;
        }
    }
}

fn clamp_unit(x: f64, fallback: f64) -> f64 {
    if x.is_finite() { x.clamp(0.0, 1.0) } else { fallback }
}
