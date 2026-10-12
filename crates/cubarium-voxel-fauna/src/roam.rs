//! **YOUNG-ROAM: young animals stray more as they grow** (`design/behaviour-ai-2026-10-11.md`
//! package 6: §0 row 7, §3.5; the brief `design/handoffs/young-roam-2026-10-12.md`). Wrysk:
//! "young loftstriders and non-infant seedporters should have some level of curiosity to stray
//! from their parent and explore though, increasing the older they get".
//!
//! - **Young pace** ([`pace_scale`]). Pace is calibrated in body lengths per second, so a young
//!   body's cruise, and its sprint with it, is its own length's: the lineage's cruise times its
//!   length over an adult's (`(body / birth_body)^(1/3)`, lengths going as the cube root of the
//!   body), never under `pace_floor` of it. A body at or past `birth_body` (an adult) keeps its
//!   lineage's cruise. Before this a calf at a fifth of the adult body walked and fled at the
//!   adult's metres per second.

use serde::{Deserialize, Serialize};

use crate::Founder;

/// **A lineage's young** (module note): its knobs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Young {
    /// A young body's cruise (and sprint) is its own length's, never under this fraction of
    /// the adult's; 1: the adult's pace at any size (before YOUNG-ROAM).
    pub pace_floor: f64,
}

impl Default for Young {
    /// The adult's pace at any size; no roaming.
    fn default() -> Young {
        Young { pace_floor: 1.0 }
    }
}

/// **Each lineage's young** (module note), placeholders, untuned:
/// - `pace_floor` 0.5 for every lineage: a newborn is about 0.55 of a breeding adult's length
///   (its `body_min` over `birth_body`, cube root), so the floor seldom binds; it keeps a body
///   born small walking at half the adult's pace (hoofed calves run with the herd within a day
///   of birth, at about half the adults' speed).
pub fn young_of(_founder: Founder) -> Young {
    Young { pace_floor: 0.5 }
}

/// **The pace scale** of a body of `body` on lineage table `phys` (module note): its length
/// over an adult's (`(body / birth_body)^(1/3)`), at least `pace_floor`, at most 1; 1 at or
/// past `birth_body`.
pub fn pace_scale(phys: &crate::FounderPhysiology, body: f64) -> f64 {
    let adult = phys.core.birth_body;
    if !(adult > 0.0) || !body.is_finite() || body >= adult {
        return 1.0;
    }
    let own = (body.max(0.0) / adult).cbrt();
    let floor = if phys.young.pace_floor.is_finite() { phys.young.pace_floor.clamp(0.0, 1.0) } else { 1.0 };
    own.max(floor).min(1.0)
}

#[cfg(test)]
#[path = "roam_tests.rs"]
mod roam_tests;
