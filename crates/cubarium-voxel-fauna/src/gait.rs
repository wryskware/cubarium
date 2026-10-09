//! **Gaits above cruise** (package GAIT, Wrysk 2026-10-06): the Veilhound's rush and the
//! seedporter's ground dash.
//!
//! A lineage's [`Sprint`] is the top speed a rush or a ground flight asks for, limited by
//! stamina. The motion rule delivers it through the motor budget: a forward above 1 (cruise)
//! is paid as walking is, pro rata to the speed asked, so a rush at 2.5 × cruise costs 2.5 ×
//! a second's walking. A sprint spends stamina at `1 / endurance_s` a second at top speed
//! (pro rata below it); a body that spends it all is **winded** and held to cruise until it
//! has rested back to `resume_fraction`, resting at `1 / recovery_s` a second off the
//! sprint. A lineage with no sprint is held to its cruise whatever its forward asks.
//!
//! The sprint machinery is S2b's unmerged work (`sprint-wip.patch`), moved here out of
//! `vigil` and used only for the Veilhound's rush and the seedporter's ground flight.

use serde::{Deserialize, Serialize};

use crate::{Actions, Fauna, FaunaConfig, Founder, FounderState};

/// **A lineage's sprint**: the gait above cruise its rush or ground flight asks for, and the
/// stamina that limits it. A top speed at or below the cruise, or no endurance, is no sprint.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sprint {
    /// Top speed, body lengths per second (the cruise's units, on the adult length).
    pub body_lengths_per_s: f64,
    /// Seconds at top speed from rested to spent.
    pub endurance_s: f64,
    /// Seconds off the sprint from spent to rested.
    pub recovery_s: f64,
    /// A winded body sprints again once its stamina is back to this fraction.
    pub resume_fraction: f64,
}

impl Default for Sprint {
    /// No sprint.
    fn default() -> Sprint {
        Sprint { body_lengths_per_s: 0.0, endurance_s: 0.0, recovery_s: 0.0, resume_fraction: 0.5 }
    }
}

impl Sprint {
    /// The forward effort of a full sprint for a lineage cruising at `cruise_bl_per_s`: its
    /// top speed over its cruise, and 1 (cruise) when it has no sprint.
    pub fn effort(&self, cruise_bl_per_s: f64) -> f64 {
        if self.endurance_s > 0.0 && cruise_bl_per_s > 0.0 && self.body_lengths_per_s > cruise_bl_per_s {
            self.body_lengths_per_s / cruise_bl_per_s
        } else {
            1.0
        }
    }

    /// **One tick of the gate** on the stamina in `fs`: the forward the body may have of the
    /// `asked`, over `dt` seconds. A forward above 1 is a sprint, capped at [`Sprint::effort`]
    /// and allowed while `may` (on its feet, not on a wall) and not winded; anything else is
    /// held to cruise and rests. Returns the forward and whether this tick winded it.
    pub fn gate(&self, cruise_bl_per_s: f64, fs: &mut FounderState, asked: f64, may: bool, dt: f64) -> (f64, bool) {
        let top = self.effort(cruise_bl_per_s);
        if asked > 1.0 && top > 1.0 && may && !fs.winded {
            let forward = asked.min(top);
            fs.sprint_spent += dt / self.endurance_s * (forward - 1.0) / (top - 1.0);
            let winded = fs.sprint_spent >= 1.0;
            if winded {
                fs.sprint_spent = 1.0;
                fs.winded = true;
            }
            return (forward, winded);
        }
        self.rest(fs, dt);
        (asked.min(1.0), false)
    }

    /// A tick off the sprint: stamina rests at `1 / recovery_s` a second, and a winded body
    /// may sprint again once rested to `resume_fraction`.
    pub fn rest(&self, fs: &mut FounderState, dt: f64) {
        if fs.sprint_spent > 0.0 {
            fs.sprint_spent = if self.recovery_s > 0.0 { (fs.sprint_spent - dt / self.recovery_s).max(0.0) } else { 0.0 };
        }
        if fs.winded && 1.0 - fs.sprint_spent >= self.resume_fraction {
            fs.winded = false;
        }
    }
}

/// **Each lineage's sprint** (GAIT, Wrysk 2026-10-06: the Veilhound "should move faster", the
/// seedporter "a bit below veilhound when on the ground"). Placeholders at the display's
/// scale; the numbers are knobs (`fauna.founders.*.sprint`).
///
/// - **The Veilhound** rushes at 2.5 × its 1 BL/s cruise: 2.5 BL/s, 3.5 m/s on its 1.4 m.
///   Real pack hunters sprint far faster in body lengths but only briefly: the lion, about
///   2 m head-body, charges at about 80 km/h (≈ 11 BL/s) over some 100–200 m, a few seconds
///   (Schaller 1972, *The Serengeti Lion*; Elliott, Cowan & Holling 1977, Can. J. Zool.
///   55:1811); the grey wolf, about 1.3 m, reaches 55–65 km/h (≈ 12–14 BL/s) in a dash and
///   chases for minutes at a lope of about 40 km/h (Mech 1970, *The Wolf*; Mech, Smith &
///   MacNulty 2015, *Wolves on the Hunt*). Against their trot (≈ 2 BL/s) the dash is some
///   5–7 × the cruise. The display keeps the order (a rush is a few × the cruise, short,
///   then a long rest) and compresses the multiple to 2.5 × so a rush reads on screen and a
///   grazer's flight at its walk still matters. **Endurance 20 s** (between the lion's few
///   seconds and the wolf's minutes), **recovery 120 s** (a cat lies panting a minute or
///   two after a failed charge), sprinting again at half rested.
/// - **The seedporter's ground dash** is 0.85 × the Veilhound's rush in m/s (the parent's
///   design: caught in the open far from a tree it can be run down; near a tree it is up
///   it): 0.85 × 3.5 = 2.975 m/s, 5.95 BL/s on its 0.5 m. Small arboreal mammals dash
///   briefly between trees (a tree squirrel's ground sprint is some 5 m/s, ≈ 20 BL/s, held
///   for seconds): **endurance 10 s, recovery 60 s**.
/// - Every other lineage has none: their gaits do not change (the frondgrazer's defence is
///   its armour, the lanternjaw ambushes from still).
pub fn sprint_of(founder: Founder) -> Sprint {
    let none = Sprint::default();
    match founder {
        Founder::Chorister => Sprint { body_lengths_per_s: 2.5, endurance_s: 20.0, recovery_s: 120.0, ..none },
        Founder::Seedporter => Sprint { body_lengths_per_s: 0.85 * 3.5 / 0.5, endurance_s: 10.0, recovery_s: 60.0, ..none },
        _ => none,
    }
}

/// The forward a rush or a ground flight asks for: its lineage's full sprint
/// ([`Sprint::effort`]), 1 (cruise) for a lineage with none.
pub(crate) fn sprint_forward(config: &FaunaConfig, founder: Founder) -> f64 {
    let p = config.founder(founder);
    p.sprint.effort(p.cruise_body_lengths_per_s)
}

/// A ground flight's actions at its sprint while the threat is `close`, else as asked.
pub(crate) fn dash(config: &FaunaConfig, founder: Founder, close: bool, act: Actions) -> Actions {
    if close { Actions { forward: act.forward * sprint_forward(config, founder), ..act } } else { act }
}

/// **The sprint gate** on the body at `i` this tick (founder tick): `held.forward` becomes
/// what the gate allows, and the ledger counts sprint ticks and windings by lineage.
pub(crate) fn gate(fauna: &mut Fauna, i: usize, founder: Founder, may: bool, held: &mut Actions) {
    let p = *fauna.config.founder(founder);
    let fs = &mut fauna.animals[i].founder_state;
    let (forward, winded) = p.sprint.gate(p.cruise_body_lengths_per_s, fs, held.forward, may, cubarium_voxel::DT);
    if forward > 1.0 {
        fauna.ledger.sprint_ticks_by_founder[founder.index()] += 1;
    }
    if winded {
        fauna.ledger.winded_by_founder[founder.index()] += 1;
    }
    held.forward = forward;
    if let Some(tap) = fauna.tap.as_mut() {
        tap.gait(fauna.animals[i].id, fauna.tick, forward, p.sprint.effort(p.cruise_body_lengths_per_s));
    }
}

/// A tick off its feet (a glider climbing, perched or gliding): stamina rests.
pub(crate) fn rest(fauna: &mut Fauna, i: usize, founder: Founder) {
    let sp = fauna.config.founder(founder).sprint;
    sp.rest(&mut fauna.animals[i].founder_state, cubarium_voxel::DT);
    if let Some(tap) = fauna.tap.as_mut() {
        let a = &fauna.animals[i];
        tap.gait(a.id, fauna.tick, a.founder_state.held.forward.min(1.0), 1.0);
    }
}
