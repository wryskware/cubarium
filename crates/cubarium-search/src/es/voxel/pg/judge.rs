//! **The judge's and the teacher's readings of a body** (SCORE-V2,
//! `design/handoffs/score-v2-2026-10-15.md`): its life stage, for the judge's life values,
//! and its three drives, for the teacher. Bookkeeping for the reward and the checks only:
//! no behaviour, sense or heuristic reads anything here.
//!
//! - **Life stage** ([`life_stage`]). The fauna's own word for breeding is *eligible*: a body
//!   may begin a gestation or a clutch once its structure is at `birth_body` (with a reserve
//!   that pays for the package; `cubarium_voxel_fauna::body::birth_readiness`). There is no
//!   breeding age: maturity is size. So a **young** is below `birth_body` (or riding a host);
//!   a **breeding adult** is a grown body that is pregnant, its escrow filling
//!   (`ReproductionState::escrow`), or, for an egg-layer, holding a brood earmarked for its
//!   next clutch (`ReproductionState::brood_held`); every other grown body is an **adult**.
//! - **Drives** ([`drives`]), each dimensionless:
//!   - hunger: `1 − S/K`, the stomach's raw organic matter `S` (`Animal::stomach`) against
//!     its capacity `K = stomach_k · body` (`Gut::capacity`), clamped to `[0, 1]`; 0 for a
//!     lineage without a stomach;
//!   - wound load: the wound stock `Animal::wound`, `[0, 1)` (1 kills);
//!   - sleep debt: the rest physiology's time since its last deep bout
//!     (`Doze::last_deep`; from its birth before its first rest) over its sleep-need ceiling
//!     `RestTable::need_s`, 0 while in deep sleep. A lineage without light rest or without a
//!     ceiling (every rest deep, nothing owed: the lanternjaw today) carries none.

use std::collections::BTreeMap;

use cubarium_voxel_fauna::{Animal, Founder, FounderPhysiology};
use cubarium_voxel_sim::Sim;
use serde::Serialize;

use super::knobs::JudgeKnobs;

/// A body's life stage, the judge's life value's key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeStage {
    /// Below breeding size (`birth_body`), or riding a host.
    Young,
    /// At breeding size, carrying no offspring.
    #[default]
    Adult,
    /// At breeding size and gestating, or holding a brood for its next clutch.
    Breeding,
}

/// Body `a`'s life stage under its lineage's physiology `p` (module note).
pub fn life_stage(a: &Animal, p: &FounderPhysiology) -> LifeStage {
    if a.attached.is_some() || a.body < p.core.birth_body {
        LifeStage::Young
    } else if a.reproduction.escrow.is_some() || a.reproduction.brood_held(a.reserve) > 0.0 {
        LifeStage::Breeding
    } else {
        LifeStage::Adult
    }
}

/// The drives, in this order.
pub const DRIVES: usize = 3;
pub const HUNGER: usize = 0;
pub const WOUND: usize = 1;
pub const SLEEP: usize = 2;
/// Each drive's reward-term name.
pub const DRIVE_NAMES: [&str; DRIVES] = ["hunger", "wound", "sleep"];

/// Body `a`'s drives at fauna tick `now` (module note): hunger, wound load, sleep debt.
pub fn drives(a: &Animal, p: &FounderPhysiology, now: u64) -> [f64; DRIVES] {
    let k = p.gut.capacity(a.body);
    let hunger = if k > 0.0 { (1.0 - a.stomach.organic() / k).clamp(0.0, 1.0) } else { 0.0 };
    let wound = a.wound.clamp(0.0, 1.0);
    let rest = p.rest;
    let sleep = if !rest.light || !(rest.need_s > 0.0) || cubarium_voxel_fauna::sleep::deep_asleep(a) {
        0.0
    } else {
        let since = if a.doze.last_deep > 0 { a.doze.last_deep } else { now.saturating_sub(a.age_ticks) };
        now.saturating_sub(since) as f64 * cubarium_voxel::DT / rest.need_s
    };
    [hunger, wound, sleep]
}

/// **The judge's life events over a check arm** (lineage-level): every body of the lineage
/// that dies costs its life value at the stage it was last seen in; every newborn whose
/// mother is a body of the lineage adds a young's. Read each tick after the step.
#[derive(Clone, Debug, Default)]
pub struct LifeTally {
    /// Live bodies of the lineage after the last observation and their stage then.
    seen: BTreeMap<u64, LifeStage>,
    started: bool,
    /// Summed life values lost at deaths and added at births, body references.
    pub lost: f64,
    pub born: f64,
    pub deaths: usize,
    pub births: usize,
}

impl LifeTally {
    /// Read lineage `f`'s bodies now: the first call only notes who stands.
    pub fn observe(&mut self, sim: &Sim, f: Founder, judge: &JudgeKnobs) {
        let p = sim.fauna().config().founder(f);
        let mut now = BTreeMap::new();
        for a in sim.fauna().view().animals.iter().filter(|a| a.founder == Some(f)) {
            now.insert(a.id, life_stage(a, p));
            if self.started && !self.seen.contains_key(&a.id) && a.mother.is_some_and(|m| self.seen.contains_key(&m) || now.contains_key(&m)) {
                self.births += 1;
                self.born += judge.life(LifeStage::Young);
            }
        }
        if self.started {
            for (id, stage) in &self.seen {
                if !now.contains_key(id) {
                    self.deaths += 1;
                    self.lost += judge.life(*stage);
                }
            }
        }
        self.seen = now;
        self.started = true;
    }

    /// Births' life values minus deaths', body references.
    pub fn net(&self) -> f64 {
        self.born - self.lost
    }
}
