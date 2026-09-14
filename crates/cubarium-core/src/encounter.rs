//! Explicitly opt-in encounters between active adult apex predators.
//!
//! The ordinary hunter extension remains a one-parent experiment. This sibling extension owns
//! the additional parent, contribution split, and combat facts needed by the paired policy.

use serde::{Deserialize, Serialize};

use crate::genome::{Drives, Genome};
use crate::hunter::HunterState;
use crate::ids::{OrganismId, Slots};
use crate::organism::DeathCause;
use crate::organism::Organism;

pub const APEX_ENCOUNTER_VERSION: u32 = 1;
pub const MATING_RADIUS_PX: f64 = 10.0;
pub const COMBAT_RADIUS_PX: f64 = 8.0;
pub const INJURY_ADULT_FRACTION: f64 = 0.05;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApexEncounterPolicy {
    #[default]
    Off,
    PairedV1,
}

impl ApexEncounterPolicy {
    pub fn enabled(self) -> bool {
        matches!(self, Self::PairedV1)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::PairedV1 => "paired_v1",
        }
    }
}

/// The inventory one adult actually transferred into a joint gestation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ApexContribution {
    pub structure: f64,
    pub reserve: f64,
    pub energy: f64,
    /// Energy paid as heat while this contributor's structural share was built.
    pub build_heat: f64,
}

impl ApexContribution {
    pub fn material(self) -> f64 {
        self.structure + self.reserve
    }
}

/// The second-parent metadata beside an ordinary escrow carried by `carrier`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairedGestation {
    pub carrier: OrganismId,
    pub partner: OrganismId,
    pub started_tick: u64,
    pub carrier_paid: ApexContribution,
    pub partner_paid: ApexContribution,
}

/// Two-parent ancestry for a currently living paid child, persisted with the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedParentage {
    pub child: OrganismId,
    pub carrier: OrganismId,
    pub partner: OrganismId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatResponse {
    Retreated,
    Retaliated,
    Injured,
}

/// Dedicated paired facts. These do not masquerade as one-parent hunter transactions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ApexEncounterEvent {
    Mated {
        tick: u64,
        carrier: OrganismId,
        partner: OrganismId,
        carrier_paid: ApexContribution,
        partner_paid: ApexContribution,
        child_genome: u64,
    },
    Born {
        tick: u64,
        carrier: OrganismId,
        partner: OrganismId,
        child: OrganismId,
        structure: f64,
        reserve: f64,
        energy: f64,
        birth_heat: f64,
    },
    /// A capacity refusal returns each share to its surviving contributor. If `partner` has
    /// died since funding, `partner_refund_to` is the carrier: the prepaid inventory remains
    /// conserved rather than being addressed through a stale generational ID. Build heat was
    /// already paid at mating and is never refunded.
    Refunded {
        tick: u64,
        carrier: OrganismId,
        partner: OrganismId,
        carrier_refund: ApexContribution,
        partner_refund: ApexContribution,
        partner_refund_to: OrganismId,
    },
    Miscarried {
        tick: u64,
        carrier: OrganismId,
        partner: OrganismId,
        cause: DeathCause,
        material: f64,
        energy: f64,
        energy_stored: f64,
        energy_heat: f64,
    },
    Combat {
        tick: u64,
        attacker: OrganismId,
        defender: OrganismId,
        response: CombatResponse,
        attacker_energy_paid: f64,
        defender_energy_paid: f64,
        attacker_injury: f64,
        defender_injury: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApexEncounterState {
    pub version: u32,
    pub policy: ApexEncounterPolicy,
    /// Strictly sorted by carrier ID. The authoritative inventory remains `carrier.escrow`.
    pub gestations: Vec<PairedGestation>,
    /// Strictly sorted by living child ID.
    pub parentage: Vec<PairedParentage>,
    pub matings_total: u64,
    pub births_total: u64,
    pub refunds_total: u64,
    pub miscarriages_total: u64,
    pub combats_total: u64,
    pub retreats_total: u64,
    pub retaliations_total: u64,
    pub injury_material_total: f64,
}

impl Default for ApexEncounterState {
    fn default() -> Self {
        Self {
            version: APEX_ENCOUNTER_VERSION,
            policy: ApexEncounterPolicy::Off,
            gestations: Vec::new(),
            parentage: Vec::new(),
            matings_total: 0,
            births_total: 0,
            refunds_total: 0,
            miscarriages_total: 0,
            combats_total: 0,
            retreats_total: 0,
            retaliations_total: 0,
            injury_material_total: 0.0,
        }
    }
}

impl ApexEncounterState {
    pub fn paired_v1() -> Self {
        Self {
            policy: ApexEncounterPolicy::PairedV1,
            ..Self::default()
        }
    }

    pub fn active(&self) -> bool {
        self.policy.enabled()
    }

    pub fn gestation(&self, carrier: OrganismId) -> Option<&PairedGestation> {
        self.gestations
            .binary_search_by_key(&carrier, |g| g.carrier)
            .ok()
            .map(|i| &self.gestations[i])
    }

    pub(crate) fn insert_gestation(&mut self, record: PairedGestation) -> bool {
        match self
            .gestations
            .binary_search_by_key(&record.carrier, |g| g.carrier)
        {
            Ok(_) => false,
            Err(at) => {
                self.gestations.insert(at, record);
                true
            }
        }
    }

    pub(crate) fn take_gestation(&mut self, carrier: OrganismId) -> Option<PairedGestation> {
        self.gestations
            .binary_search_by_key(&carrier, |g| g.carrier)
            .ok()
            .map(|i| self.gestations.remove(i))
    }

    pub fn parents_of(&self, child: OrganismId) -> Option<(OrganismId, OrganismId)> {
        self.parentage
            .binary_search_by_key(&child, |p| p.child)
            .ok()
            .map(|i| (self.parentage[i].carrier, self.parentage[i].partner))
    }

    pub(crate) fn insert_parentage(&mut self, record: PairedParentage) {
        let at = self.parentage.partition_point(|p| p.child < record.child);
        self.parentage.insert(at, record);
    }

    pub(crate) fn remove_parentage(&mut self, child: OrganismId) {
        if let Ok(at) = self.parentage.binary_search_by_key(&child, |p| p.child) {
            self.parentage.remove(at);
        }
    }

    pub fn validate(
        &self,
        tick: u64,
        cap: usize,
        organisms: &Slots<Organism>,
        hunters: &HunterState,
    ) -> Result<(), String> {
        if self.version != APEX_ENCOUNTER_VERSION {
            return Err(format!(
                "apex encounter extension version {} is not {APEX_ENCOUNTER_VERSION}",
                self.version
            ));
        }
        if !self.injury_material_total.is_finite() || self.injury_material_total < 0.0 {
            return Err("apex encounter injury total is not finite and nonnegative".into());
        }
        if !self.active() {
            if self != &Self::default() {
                return Err("apex encounter policy is off but persisted activity exists".into());
            }
            return Ok(());
        }
        if self.gestations.len() > cap || self.parentage.len() > cap {
            return Err("apex encounter records exceed organism capacity".into());
        }
        if self.matings_total
            != self.births_total
                + self.refunds_total
                + self.miscarriages_total
                + self.gestations.len() as u64
        {
            return Err("apex encounter mating outcomes do not reconcile".into());
        }
        if self.retreats_total + self.retaliations_total > self.combats_total {
            return Err("apex encounter combat outcomes do not reconcile".into());
        }
        let mut previous = None;
        let mut committed = Vec::with_capacity(self.gestations.len() * 2);
        for g in &self.gestations {
            if previous.is_some_and(|id| id >= g.carrier) {
                return Err("paired gestations are not strictly sorted".into());
            }
            previous = Some(g.carrier);
            if g.carrier == g.partner || g.started_tick > tick {
                return Err(format!(
                    "paired gestation {:?} has invalid identity or tick",
                    g.carrier
                ));
            }
            if committed.contains(&g.carrier) || committed.contains(&g.partner) {
                return Err("an apex adult is committed to more than one gestation".into());
            }
            committed.extend([g.carrier, g.partner]);
            let Some(carrier) = organisms.get(g.carrier) else {
                return Err(format!(
                    "paired gestation carrier {:?} is not alive",
                    g.carrier
                ));
            };
            if !hunters.contains(g.carrier) {
                return Err(format!(
                    "paired gestation carrier {:?} is not a hunter",
                    g.carrier
                ));
            }
            if carrier.escrow.as_ref().map(|e| e.started_tick) != Some(g.started_tick) {
                return Err(format!(
                    "paired gestation {:?} does not match its escrow",
                    g.carrier
                ));
            }
            if organisms.get(g.partner).is_some() && !hunters.contains(g.partner) {
                return Err(format!(
                    "live paired partner {:?} is not a hunter",
                    g.partner
                ));
            }
            for paid in [g.carrier_paid, g.partner_paid] {
                for amount in [paid.structure, paid.reserve, paid.energy, paid.build_heat] {
                    if !amount.is_finite() || amount < 0.0 {
                        return Err("paired contribution is not finite and nonnegative".into());
                    }
                }
            }
            let escrow = carrier.escrow.as_ref().expect("checked above");
            if (escrow.structure - g.carrier_paid.structure - g.partner_paid.structure).abs()
                > crate::hunter::TOLERANCE
                || (escrow.reserve - g.carrier_paid.reserve - g.partner_paid.reserve).abs()
                    > crate::hunter::TOLERANCE
                || (escrow.energy - g.carrier_paid.energy - g.partner_paid.energy).abs()
                    > crate::hunter::TOLERANCE
            {
                return Err(format!(
                    "paired gestation {:?} contributions do not match its escrow",
                    g.carrier
                ));
            }
        }
        previous = None;
        for p in &self.parentage {
            if previous.is_some_and(|id| id >= p.child) {
                return Err("paired parentage is not strictly sorted".into());
            }
            previous = Some(p.child);
            let Some(child) = organisms.get(p.child) else {
                return Err(format!("paired child {:?} is not alive", p.child));
            };
            if child.parent != Some(p.carrier) || p.carrier == p.partner {
                return Err(format!(
                    "paired child {:?} has inconsistent parentage",
                    p.child
                ));
            }
        }
        Ok(())
    }
}

/// Midpoint recombination makes every continuous inherited locus depend on both adults. The
/// discrete rig is selected deterministically from the canonical pair and is never mutated.
pub fn recombine(a: &Genome, b: &Genome, carrier: OrganismId, partner: OrganismId) -> Genome {
    let mean = |x: f32, y: f32| x + (y - x) * 0.5;
    let drives = Drives {
        w_food: mean(a.drives.w_food, b.drives.w_food),
        w_detritus: mean(a.drives.w_detritus, b.drives.w_detritus),
        w_persist: mean(a.drives.w_persist, b.drives.w_persist),
        w_crowd: mean(a.drives.w_crowd, b.drives.w_crowd),
        seek_on: mean(a.drives.seek_on, b.drives.seek_on),
        seek_off: mean(a.drives.seek_off, b.drives.seek_off),
        feed_min: mean(a.drives.feed_min, b.drives.feed_min),
        rest_effort: mean(a.drives.rest_effort, b.drives.rest_effort),
        feed_effort: mean(a.drives.feed_effort, b.drives.feed_effort),
        bud_reserve: mean(a.drives.bud_reserve, b.drives.bud_reserve),
        bud_energy: mean(a.drives.bud_energy, b.drives.bud_energy),
        bud_min_age_seconds: mean(a.drives.bud_min_age_seconds, b.drives.bud_min_age_seconds),
        tau_hunger_seconds: mean(a.drives.tau_hunger_seconds, b.drives.tau_hunger_seconds),
        turn_rate_max_deg: mean(a.drives.turn_rate_max_deg, b.drives.turn_rate_max_deg),
        turn_noise: mean(a.drives.turn_noise, b.drives.turn_noise),
        w_depth: mean(a.drives.w_depth, b.drives.w_depth),
    };
    let choose_a = (u64::from(carrier.slot)
        ^ u64::from(carrier.generation)
        ^ u64::from(partner.slot)
        ^ u64::from(partner.generation))
        & 1
        == 0;
    let mut child = Genome {
        version: Genome::VERSION,
        size: mean(a.size, b.size),
        metabolism: mean(a.metabolism, b.metabolism),
        sense: mean(a.sense, b.sense),
        reserve: mean(a.reserve, b.reserve),
        mouth: mean(a.mouth, b.mouth),
        speed: mean(a.speed, b.speed),
        hue: mean(a.hue, b.hue),
        diet: mean(a.diet, b.diet),
        depth: mean(a.depth, b.depth),
        swim: mean(a.swim, b.swim),
        form: if choose_a { a.form } else { b.form },
        drives,
    };
    child.clamp();
    child
}
