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
//! - **The leash loosens** ([`leash`]). A young body that knows its mother (a recorded mother,
//!   sensed through [`crate::nurse::mother_track`]) and is younger than `independence_s` keeps
//!   within a comfort distance of her that grows with its age, from `follow_m` to `roam_m` at
//!   independence: past it the herd pull ([`crate::herding::group_pull`]) leans its walk back
//!   toward her at `follow_pull`, instead of toward the troop's or family's mean; within it,
//!   nothing pulls it. A nursing lineage's young before its `weaning_age_s` keeps its nursing
//!   follow ([`crate::nurse::mother_pull`]) and does not stray; its roam grows from weaning to
//!   independence. It weighs its mother on every remote sensing, as a dependent young does
//!   ([`crate::nurse::weighs_as_mother`]).

use serde::{Deserialize, Serialize};

use cubarium_voxel::DT;

use crate::{Fauna, Founder};

/// **A lineage's young** (module note): its knobs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Young {
    /// A young body's cruise (and sprint) is its own length's, never under this fraction of
    /// the adult's; 1: the adult's pace at any size (before YOUNG-ROAM).
    pub pace_floor: f64,
    /// A young body keeps to its mother, and roams, until this age, seconds; 0: off.
    pub independence_s: f64,
    /// Its comfort distance from its mother at birth (a nursing lineage's: at weaning),
    /// metres; 0: off (the leash and excursions).
    pub follow_m: f64,
    /// ...growing with its age to this at `independence_s`, metres.
    pub roam_m: f64,
    /// The leash's share of its walking turn past the comfort (1: straight at its mother).
    pub follow_pull: f64,
}

impl Default for Young {
    /// The adult's pace at any size; no roaming.
    fn default() -> Young {
        Young { pace_floor: 1.0, independence_s: 0.0, follow_m: 0.0, roam_m: 0.0, follow_pull: 0.0 }
    }
}

impl Young {
    /// Whether the leash (and with it the excursions) is on.
    pub fn roams(&self) -> bool {
        self.independence_s > 0.0 && self.follow_m > 0.0
    }
}

/// **Each lineage's young** (module note), placeholders, untuned:
/// - `pace_floor` 0.5 for every lineage: a newborn is about 0.55 of a breeding adult's length
///   (its `body_min` over `birth_body`, cube root), so the floor seldom binds; it keeps a body
///   born small walking at half the adult's pace (hoofed calves run with the herd within a day
///   of birth, at about half the adults' speed).
/// - **The loftstrider** (Wrysk's addition on topic 7): a calf keeps within 3 m of its mother at
///   birth (at heel, under two of her lengths) and within 10 m at independence (beyond the
///   family's 6 m comfort, inside its 12 m day sight); independence at 15,000 s, about 0.7 of
///   its 21,600 s birth interval (giraffe calves stay with the mother 12-16 months of a
///   ~20-month interbirth); the leash's pull is the family pull's half turn.
/// - **The seedporter**: from its weaning (its 3 m nursing leash) to 8 m at independence (twice
///   the troop's 4 m comfort, half its 16 m sight); independence at 18,000 s, half its 10 h
///   year (marmoset juveniles forage on their own by five to six months); the troop's half
///   turn.
pub fn young_of(founder: Founder) -> Young {
    let d = Young { pace_floor: 0.5, ..Young::default() };
    match founder {
        Founder::Loftstrider => Young { independence_s: 15_000.0, follow_m: 3.0, roam_m: 10.0, follow_pull: 0.5, ..d },
        Founder::Seedporter => Young { independence_s: 18_000.0, follow_m: 3.0, roam_m: 8.0, follow_pull: 0.5, ..d },
        _ => d,
    }
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

/// The age at which body `i`'s roam begins: its lineage's weaning age when it nurses (an
/// infant does not stray), else birth.
fn roam_start_s(fauna: &Fauna, i: usize) -> f64 {
    let Some(f) = fauna.animals.get(i).and_then(|a| a.founder) else { return 0.0 };
    let n = fauna.config.founder(f).nursing;
    if n.on() { n.weaning_age_s.max(0.0) } else { 0.0 }
}

/// **Body `i`'s roam**, 0 at its start (birth, or weaning) to 1 at `independence_s`: `Some`
/// for a young body of a roaming lineage that knows its mother, past the start and short of
/// independence; `None` else (an infant before weaning included).
pub fn progress(fauna: &Fauna, i: usize) -> Option<f64> {
    let a = fauna.animals.get(i)?;
    let y = fauna.config.founder(a.founder?).young;
    if !y.roams() || crate::mother_body(a).is_none() {
        return None;
    }
    let age = a.age_ticks as f64 * DT;
    let start = roam_start_s(fauna, i);
    if age < start || age >= y.independence_s {
        return None;
    }
    let span = y.independence_s - start;
    Some(if span > 0.0 { ((age - start) / span).clamp(0.0, 1.0) } else { 1.0 })
}

/// **The leash** (module note) of body `i`: its comfort distance from its mother at its age
/// and the pull's weight, or `None` when it has none ([`progress`]).
pub fn leash(fauna: &Fauna, i: usize) -> Option<(f64, f64)> {
    let k = progress(fauna, i)?;
    let y = fauna.config.founder(fauna.animals[i].founder?).young;
    let roam = y.roam_m.max(y.follow_m);
    Some((y.follow_m + (roam - y.follow_m) * k, y.follow_pull.clamp(0.0, 1.0)))
}

#[cfg(test)]
#[path = "roam_tests.rs"]
mod roam_tests;
