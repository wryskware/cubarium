//! Opt-in underground dormancy for paid apex offspring.
//!
//! The organism and hunter member remain authoritative. This extension stores only the
//! lifecycle state that makes a newly inserted paid descendant inactive and concealed until
//! local juvenile prey and its own stocks permit emergence.

use serde::{Deserialize, Serialize};

use crate::hunter::HunterState;
use crate::ids::{OrganismId, Slots};
use crate::organism::Organism;

pub const APEX_DORMANCY_VERSION: u32 = 1;
pub const SUSTAIN_TICKS: u64 = 400;
pub const STAGGER_TICKS: u64 = 100;
pub const RECHECK_TICKS: u64 = 40;
pub const PREY_RADIUS_PX: f64 = 12.0;
pub const PREY_REQUIRED: u32 = 4;
pub const MAINTENANCE_PER_STRUCTURE_SECOND: f64 = 0.0005;
pub const EMERGENCE_RESERVE_FRACTION: f64 = 0.20;
pub const EMERGENCE_ENERGY_FRACTION: f64 = 0.10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApexDormancyPolicy {
    #[default]
    Off,
    UndergroundV1,
}

impl ApexDormancyPolicy {
    pub fn enabled(self) -> bool {
        matches!(self, Self::UndergroundV1)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::UndergroundV1 => "underground_v1",
        }
    }
}

/// Persisted state for one concealed paid descendant.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DormantApex {
    pub id: OrganismId,
    pub parent: OrganismId,
    pub entered_tick: u64,
    pub suitable_prey_ticks: u64,
    pub next_check_tick: u64,
    pub maintenance_energy_paid: f64,
}

/// Transient lifecycle facts for tests and headless observers; never rendered or persisted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ApexDormancyEvent {
    Entered {
        tick: u64,
        parent: OrganismId,
        child: OrganismId,
        next_check_tick: u64,
        structure: f64,
        reserve: f64,
        energy: f64,
    },
    Emerged {
        tick: u64,
        id: OrganismId,
        dormant_ticks: u64,
        suitable_prey: u32,
        maintenance_energy_paid: f64,
    },
    Exhausted {
        tick: u64,
        id: OrganismId,
        dormant_ticks: u64,
        maintenance_energy_paid: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApexDormancyState {
    pub version: u32,
    pub policy: ApexDormancyPolicy,
    /// Strictly sorted by full generational ID.
    pub dormant: Vec<DormantApex>,
    pub entered_total: u64,
    pub emerged_total: u64,
    pub exhausted_total: u64,
    pub maintenance_energy_paid_total: f64,
}

impl Default for ApexDormancyState {
    fn default() -> Self {
        Self {
            version: APEX_DORMANCY_VERSION,
            policy: ApexDormancyPolicy::Off,
            dormant: Vec::new(),
            entered_total: 0,
            emerged_total: 0,
            exhausted_total: 0,
            maintenance_energy_paid_total: 0.0,
        }
    }
}

impl ApexDormancyState {
    /// The explicit milestone-1 candidate setting. The default remains [`ApexDormancyPolicy::Off`].
    pub fn underground_v1() -> Self {
        Self {
            policy: ApexDormancyPolicy::UndergroundV1,
            ..Self::default()
        }
    }

    pub fn active(&self) -> bool {
        self.policy.enabled()
    }

    pub fn contains(&self, id: OrganismId) -> bool {
        self.index_of(id).is_some()
    }

    pub fn get(&self, id: OrganismId) -> Option<&DormantApex> {
        self.index_of(id).map(|i| &self.dormant[i])
    }

    pub(crate) fn index_of(&self, id: OrganismId) -> Option<usize> {
        self.dormant.binary_search_by_key(&id, |d| d.id).ok()
    }

    pub(crate) fn admit(
        &mut self,
        parent: OrganismId,
        child: OrganismId,
        tick: u64,
        organism: &Organism,
    ) -> Option<ApexDormancyEvent> {
        if !self.active() || self.contains(child) {
            return None;
        }
        let stagger = deterministic_stagger(child);
        let next_check_tick = tick.saturating_add(SUSTAIN_TICKS).saturating_add(stagger);
        let record = DormantApex {
            id: child,
            parent,
            entered_tick: tick,
            suitable_prey_ticks: 0,
            next_check_tick,
            maintenance_energy_paid: 0.0,
        };
        let at = self.dormant.partition_point(|d| d.id < child);
        self.dormant.insert(at, record);
        self.entered_total += 1;
        Some(ApexDormancyEvent::Entered {
            tick,
            parent,
            child,
            next_check_tick,
            structure: organism.structure,
            reserve: organism.reserve,
            energy: organism.energy,
        })
    }

    pub(crate) fn remove(&mut self, id: OrganismId) -> Option<DormantApex> {
        self.index_of(id).map(|i| self.dormant.remove(i))
    }

    pub fn validate(
        &self,
        tick: u64,
        cap: usize,
        organisms: &Slots<Organism>,
        hunters: &HunterState,
    ) -> Result<(), String> {
        if self.version != APEX_DORMANCY_VERSION {
            return Err(format!(
                "apex dormancy extension version {} is not {APEX_DORMANCY_VERSION}",
                self.version
            ));
        }
        if !self.maintenance_energy_paid_total.is_finite()
            || self.maintenance_energy_paid_total < 0.0
        {
            return Err("apex dormancy maintenance total is not finite and nonnegative".into());
        }
        if !self.active() {
            if !self.dormant.is_empty()
                || self.entered_total != 0
                || self.emerged_total != 0
                || self.exhausted_total != 0
                || self.maintenance_energy_paid_total != 0.0
            {
                return Err("apex dormancy policy is off but persisted activity exists".into());
            }
            return Ok(());
        }
        if self.dormant.len() > cap {
            return Err("apex dormancy entries exceed organism capacity".into());
        }
        if self.entered_total
            != self.emerged_total + self.exhausted_total + self.dormant.len() as u64
        {
            return Err("apex dormancy lifecycle counters do not reconcile".into());
        }
        let mut previous = None;
        for d in &self.dormant {
            if previous.is_some_and(|id| id >= d.id) {
                return Err("apex dormancy entries are not strictly sorted".into());
            }
            previous = Some(d.id);
            let Some(o) = organisms.get(d.id) else {
                return Err(format!("dormant apex {:?} is not a live organism", d.id));
            };
            let Some(member) = hunters.member(d.id) else {
                return Err(format!("dormant apex {:?} is not a hunter member", d.id));
            };
            if o.parent != Some(d.parent) {
                return Err(format!("dormant apex {:?} has the wrong parent", d.id));
            }
            if member.target.is_some() || member.carrying() || member.phase.hunting() {
                return Err(format!("dormant apex {:?} has active hunter state", d.id));
            }
            if d.entered_tick > tick || d.next_check_tick < d.entered_tick {
                return Err(format!("dormant apex {:?} has invalid tick state", d.id));
            }
            if !d.maintenance_energy_paid.is_finite() || d.maintenance_energy_paid < 0.0 {
                return Err(format!("dormant apex {:?} has invalid maintenance", d.id));
            }
        }
        Ok(())
    }
}

/// Stable full-ID staggering. Slot reuse changes `generation`, so a replacement does not inherit
/// the old organism's wake phase.
pub fn deterministic_stagger(id: OrganismId) -> u64 {
    if STAGGER_TICKS == 0 {
        return 0;
    }
    let mut x = (u64::from(id.slot) << 32) | u64::from(id.generation);
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    (x ^ (x >> 31)) % STAGGER_TICKS
}
