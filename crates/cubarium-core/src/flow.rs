//! A read-only per-member reserve/energy/structure flow ledger, recorded at the exact
//! mutation sites in [`crate::world::World::step`].
//!
//! This exists to answer one question the retained charging artifacts cannot: where a paid
//! juvenile's reserve actually goes, tick by tick, between the meal that supplies it and the
//! growth branch that would spend it. The 200-tick census in those artifacts observes stocks;
//! it never observes a flow, so every "oxidation drained it" statement made from that census
//! is a budget inference. This module measures the flows themselves.
//!
//! Three rules keep it a measurement rather than an attribution:
//!
//! 1. **Every amount is read at the assignment that moves it**, in the same expression the
//!    core uses, and is never reconstructed from a post-step stock difference. A post-step
//!    delta cannot tell oxidation from growth; both move reserve in the same tick.
//! 2. **Gate state is read before the branch it gates**, not after. `growth_gate` records the
//!    predicate's two sides as the core evaluates them, so a closed gate is observed, not
//!    inferred from the absence of growth.
//! 3. **Nothing here is ever read back by the tick.** The ledger is write-only from the
//!    world's point of view: no decision, draw, clamp or branch consults it. When it is
//!    absent (the default) every recording site is a null check. [`FlowLedger::residual`]
//!    then proves the recording is complete rather than merely plausible: for every member
//!    and every tick of its life, the stocks the world actually holds must equal the previous
//!    tick's stocks plus exactly the flows recorded here.
//!
//! Output is bounded by construction: one record per hunter member plus fixed-width time
//! bins, never a per-tick dump.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer};

use crate::ids::OrganismId;

/// Width of one aggregation bin, in ticks: 60 s at the core's 20 Hz step.
pub const BIN_TICKS: u64 = 1200;
/// Bins retained per member before the last one absorbs the tail (2 h 40 m of life).
pub const MAX_BINS: usize = 160;
/// Reconciliation tolerance. Flows are summed in a different order than the world applies
/// them, so exact bit equality is not available; this is a rounding envelope, not a slack
/// that could hide a missing mutation site (one missed site moves a stock by ~1e-4 or more).
pub const RESIDUAL_TOLERANCE: f64 = 1e-9;

/// The three stocks this ledger reconciles, read together at one point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Stocks {
    pub tick: u64,
    pub structure: f64,
    pub reserve: f64,
    pub energy: f64,
}

/// Gut digestion: `hunter::digest_step` applied at `world.rs` "Handling and digestion".
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Digestion {
    pub ticks: u64,
    /// Gut material consumed by the step (`step.material`).
    pub material: f64,
    pub to_reserve: f64,
    pub energy_gain: f64,
    pub to_detritus: f64,
    pub heat: f64,
}

/// One of the three field-feeding channels. A Lanternjaw's diet is 0.0, so a nonzero
/// frugivory or grazing total here would itself be a finding.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct FieldFeeding {
    pub ticks: u64,
    pub material: f64,
    pub to_reserve: f64,
    pub energy_gain: f64,
    pub heat: f64,
}

/// The oxidation branch: reserve burned into battery charge.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Oxidation {
    pub ticks: u64,
    pub reserve_burned: f64,
    pub energy_gained: f64,
    pub heat: f64,
    /// Ticks whose burn happened only because the member's activation threshold was raised
    /// above the world's configured one — the same `above_reference` test the charging
    /// diagnostics use, recorded per member instead of per process.
    pub above_reference_ticks: u64,
    pub above_reference_reserve_burned: f64,
}

/// The growth branch, and which of its four caps actually bound the step.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Growth {
    /// Ticks on which the branch was entered *and* produced a positive step.
    pub ticks: u64,
    pub reserve_spent: f64,
    pub structure_gained: f64,
    pub energy_cost: f64,
    pub heat: f64,
    pub bound_by_rate: u64,
    pub bound_by_remaining_structure: u64,
    pub bound_by_reserve: u64,
    pub bound_by_energy: u64,
}

/// Maintenance, movement and sensing. The core pays these as one lumped debit clamped to the
/// battery, so the three components are recorded as *demanded* — their exact terms in the
/// core's own expression — beside the single amount actually paid. Splitting `paid` across
/// them would be attribution, not measurement, and is deliberately not done.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Upkeep {
    pub ticks: u64,
    pub demanded_maintenance: f64,
    pub demanded_move: f64,
    pub demanded_sense: f64,
    pub demanded_total: f64,
    pub paid: f64,
    pub shortfall: f64,
    /// Ticks where the battery could not cover the full demand.
    pub shortfall_ticks: u64,
}

/// A paid strike, charged in full at entry before any outcome is known.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Strike {
    pub ticks: u64,
    pub demanded: f64,
    pub paid: f64,
    pub shortfall: f64,
}

/// Handling: paid first and in full, or nothing is digested this tick.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Handling {
    pub ticks: u64,
    pub demanded: f64,
    pub paid: f64,
    /// Ticks where the meal could not be carried, so digestion was skipped entirely.
    pub unaffordable_ticks: u64,
}

/// Reproduction transfers on the parent's side.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Funding {
    pub funded_count: u64,
    pub reserve_debit: f64,
    pub energy_debit: f64,
    pub build_heat: f64,
    pub escrow_structure: f64,
    pub escrow_reserve: f64,
    pub escrow_energy: f64,
    /// A birth refused at the population cap returns the escrow to the parent untouched.
    pub refunded_count: u64,
    pub refunded_reserve: f64,
    pub refunded_energy: f64,
}

/// The growth predicate, sampled at the mutation site before the branch runs.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct GrowthGate {
    /// Ticks on which the predicate was evaluated at all (the member was alive and reached
    /// the physiology pass).
    pub observed_ticks: u64,
    pub structure_below_adult_ticks: u64,
    pub reserve_above_min_ticks: u64,
    /// Ticks where both sides held, i.e. the branch body ran (it may still have stepped zero).
    pub branch_entered_ticks: u64,
    /// The constant threshold `growth_reserve_min · reserve_max` for this member.
    pub gate_reserve: f64,
    /// Closest the member ever came to the gate from below: `min(gate − reserve)` over ticks
    /// where it was below. `f64::INFINITY` if it was never below.
    pub min_reserve_deficit: f64,
    pub max_reserve: f64,
    /// Sum of pre-growth reserve over `observed_ticks`, for a mean.
    pub reserve_sum: f64,
    pub max_energy: f64,
    pub energy_sum: f64,
    /// Ticks with the reserve stock at exactly zero when the predicate was evaluated.
    pub reserve_zero_ticks: u64,
}

/// Per-tick reconciliation of recorded flows against the stocks the world actually holds.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Residual {
    pub checked_ticks: u64,
    pub max_structure: f64,
    pub max_reserve: f64,
    pub max_energy: f64,
    pub violations: u64,
    pub first_violation_tick: Option<u64>,
}

impl Residual {
    fn observe(&mut self, tick: u64, structure: f64, reserve: f64, energy: f64) {
        self.checked_ticks += 1;
        self.max_structure = self.max_structure.max(structure.abs());
        self.max_reserve = self.max_reserve.max(reserve.abs());
        self.max_energy = self.max_energy.max(energy.abs());
        if structure.abs() > RESIDUAL_TOLERANCE
            || reserve.abs() > RESIDUAL_TOLERANCE
            || energy.abs() > RESIDUAL_TOLERANCE
        {
            self.violations += 1;
            self.first_violation_tick.get_or_insert(tick);
        }
    }
}

/// One fixed-width slice of a member's life.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Bin {
    pub start_tick: u64,
    pub ticks: u64,
    pub reserve_in_digestion: f64,
    pub reserve_in_field: f64,
    pub reserve_out_oxidation: f64,
    pub reserve_out_growth: f64,
    pub reserve_out_funding: f64,
    pub energy_in: f64,
    pub energy_out: f64,
    pub growth_ticks: u64,
    pub gate_open_ticks: u64,
    pub oxidation_ticks: u64,
    /// Stocks at the last end-of-tick probe inside this bin.
    pub end_stocks: Stocks,
}

/// Everything recorded for one hunter member over its whole life.
#[derive(Clone, Debug, Serialize)]
pub struct MemberFlow {
    pub id: OrganismId,
    pub born_tick: u64,
    pub origin: &'static str,
    pub parent: Option<OrganismId>,
    pub first_observed_tick: u64,
    pub last_observed_tick: u64,
    pub end_tick: Option<u64>,
    pub end_cause: Option<&'static str>,
    pub adult_structure: f64,
    pub reserve_max: f64,
    pub energy_max: f64,
    /// Stocks the member started life with. For a paid descendant this is its escrow.
    pub open_stocks: Stocks,
    /// Last end-of-tick probe.
    pub close_stocks: Stocks,
    /// Stocks read at the removal site, on the tick the member died. `None` if it survived to
    /// the horizon — a censoring, not a zero.
    pub death_stocks: Option<Stocks>,
    pub juvenile_ticks: u64,
    pub adult_ticks: u64,
    pub digestion: Digestion,
    pub frugivory: FieldFeeding,
    pub grazing: FieldFeeding,
    pub scavenging: FieldFeeding,
    pub oxidation: Oxidation,
    pub growth: Growth,
    pub upkeep: Upkeep,
    pub strike: Strike,
    pub handling: Handling,
    pub funding: Funding,
    pub gate: GrowthGate,
    pub residual: Residual,
    pub bins: Vec<Bin>,
    /// Flows recorded since the last probe, and the stocks at that probe. Reset every tick;
    /// this is the reconciliation's working state, not a result.
    #[serde(skip)]
    pending: Pending,
}

#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    structure: f64,
    reserve: f64,
    energy: f64,
    have_previous: bool,
    previous: Stocks,
}

impl MemberFlow {
    fn new(
        id: OrganismId,
        born_tick: u64,
        origin: &'static str,
        parent: Option<OrganismId>,
    ) -> Self {
        MemberFlow {
            id,
            born_tick,
            origin,
            parent,
            first_observed_tick: u64::MAX,
            last_observed_tick: 0,
            end_tick: None,
            end_cause: None,
            adult_structure: 0.0,
            reserve_max: 0.0,
            energy_max: 0.0,
            open_stocks: Stocks::default(),
            close_stocks: Stocks::default(),
            death_stocks: None,
            juvenile_ticks: 0,
            adult_ticks: 0,
            digestion: Digestion::default(),
            frugivory: FieldFeeding::default(),
            grazing: FieldFeeding::default(),
            scavenging: FieldFeeding::default(),
            oxidation: Oxidation::default(),
            growth: Growth::default(),
            upkeep: Upkeep::default(),
            strike: Strike::default(),
            handling: Handling::default(),
            funding: Funding::default(),
            gate: GrowthGate {
                min_reserve_deficit: f64::INFINITY,
                ..GrowthGate::default()
            },
            residual: Residual::default(),
            bins: Vec::new(),
            pending: Pending::default(),
        }
    }

    fn bin(&mut self, tick: u64) -> &mut Bin {
        let first = if self.first_observed_tick == u64::MAX {
            tick
        } else {
            self.first_observed_tick.min(tick)
        };
        let index = ((tick.saturating_sub(first)) / BIN_TICKS) as usize;
        let index = index.min(MAX_BINS - 1);
        while self.bins.len() <= index {
            let start = first + (self.bins.len() as u64) * BIN_TICKS;
            self.bins.push(Bin {
                start_tick: start,
                ..Bin::default()
            });
        }
        &mut self.bins[index]
    }
}

/// The ledger itself. Absent by default; every recording site is a null check when it is.
#[derive(Clone, Debug, Default, Serialize)]
pub struct FlowLedger {
    pub opened_tick: u64,
    /// Keyed by full slot/generation ID, but serialized as a list: a JSON object cannot key
    /// on a composite ID, and flattening one to a string would lose the generation.
    #[serde(serialize_with = "members_as_list")]
    pub members: BTreeMap<OrganismId, MemberFlow>,
    /// Members observed at a mutation site that were never registered. Always zero in a sound
    /// run; a nonzero value means the ledger missed a lifecycle edge.
    pub unregistered_records: u64,
}

impl FlowLedger {
    pub fn new(opened_tick: u64) -> Self {
        FlowLedger {
            opened_tick,
            members: BTreeMap::new(),
            unregistered_records: 0,
        }
    }

    /// Register a member the ledger has not seen. Called for the founders present when the
    /// ledger is enabled and for each paid descendant at its birth commit.
    pub fn register(
        &mut self,
        id: OrganismId,
        born_tick: u64,
        origin: &'static str,
        parent: Option<OrganismId>,
        stocks: Stocks,
        adult_structure: f64,
        reserve_max: f64,
        energy_max: f64,
    ) {
        let entry = self
            .members
            .entry(id)
            .or_insert_with(|| MemberFlow::new(id, born_tick, origin, parent));
        entry.adult_structure = adult_structure;
        entry.reserve_max = reserve_max;
        entry.energy_max = energy_max;
        entry.open_stocks = stocks;
        entry.pending = Pending {
            have_previous: true,
            previous: stocks,
            ..Pending::default()
        };
    }

    fn at(&mut self, id: OrganismId) -> Option<&mut MemberFlow> {
        if self.members.contains_key(&id) {
            self.members.get_mut(&id)
        } else {
            self.unregistered_records += 1;
            None
        }
    }

    pub fn record_digestion(
        &mut self,
        id: OrganismId,
        tick: u64,
        material: f64,
        to_reserve: f64,
        energy_gain: f64,
        to_detritus: f64,
        heat: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        m.digestion.ticks += 1;
        m.digestion.material += material;
        m.digestion.to_reserve += to_reserve;
        m.digestion.energy_gain += energy_gain;
        m.digestion.to_detritus += to_detritus;
        m.digestion.heat += heat;
        m.pending.reserve += to_reserve;
        m.pending.energy += energy_gain;
        let bin = m.bin(tick);
        bin.reserve_in_digestion += to_reserve;
        bin.energy_in += energy_gain;
    }

    pub fn record_field_feeding(
        &mut self,
        id: OrganismId,
        tick: u64,
        channel: FeedingChannel,
        material: f64,
        to_reserve: f64,
        energy_gain: f64,
        heat: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        let slot = match channel {
            FeedingChannel::Frugivory => &mut m.frugivory,
            FeedingChannel::Grazing => &mut m.grazing,
            FeedingChannel::Scavenging => &mut m.scavenging,
        };
        slot.ticks += 1;
        slot.material += material;
        slot.to_reserve += to_reserve;
        slot.energy_gain += energy_gain;
        slot.heat += heat;
        m.pending.reserve += to_reserve;
        m.pending.energy += energy_gain;
        let bin = m.bin(tick);
        bin.reserve_in_field += to_reserve;
        bin.energy_in += energy_gain;
    }

    pub fn record_oxidation(
        &mut self,
        id: OrganismId,
        tick: u64,
        burned: f64,
        gained: f64,
        heat: f64,
        above_reference: bool,
    ) {
        let Some(m) = self.at(id) else { return };
        m.oxidation.ticks += 1;
        m.oxidation.reserve_burned += burned;
        m.oxidation.energy_gained += gained;
        m.oxidation.heat += heat;
        if above_reference {
            m.oxidation.above_reference_ticks += 1;
            m.oxidation.above_reference_reserve_burned += burned;
        }
        m.pending.reserve -= burned;
        m.pending.energy += gained;
        let bin = m.bin(tick);
        bin.reserve_out_oxidation += burned;
        bin.energy_in += gained;
        bin.oxidation_ticks += 1;
    }

    /// The growth predicate, read before the branch. `reserve` and `energy` are the stocks the
    /// core is about to test, not values recovered afterwards.
    pub fn record_growth_gate(
        &mut self,
        id: OrganismId,
        tick: u64,
        structure: f64,
        adult_structure: f64,
        reserve: f64,
        gate_reserve: f64,
        energy: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        let g = &mut m.gate;
        g.observed_ticks += 1;
        g.gate_reserve = gate_reserve;
        let structure_open = structure < adult_structure;
        let reserve_open = reserve > gate_reserve;
        if structure_open {
            g.structure_below_adult_ticks += 1;
        }
        if reserve_open {
            g.reserve_above_min_ticks += 1;
        } else {
            g.min_reserve_deficit = g.min_reserve_deficit.min(gate_reserve - reserve);
        }
        if structure_open && reserve_open {
            g.branch_entered_ticks += 1;
        }
        g.max_reserve = g.max_reserve.max(reserve);
        g.reserve_sum += reserve;
        g.max_energy = g.max_energy.max(energy);
        g.energy_sum += energy;
        if reserve == 0.0 {
            g.reserve_zero_ticks += 1;
        }
        if structure + f64::EPSILON < adult_structure {
            m.juvenile_ticks += 1;
        } else {
            m.adult_ticks += 1;
        }
        let entered = structure_open && reserve_open;
        let bin = m.bin(tick);
        if entered {
            bin.gate_open_ticks += 1;
        }
    }

    /// A growth step that actually moved structure, with the four caps the core minimised over.
    #[allow(clippy::too_many_arguments)]
    pub fn record_growth(
        &mut self,
        id: OrganismId,
        tick: u64,
        grown: f64,
        energy_cost: f64,
        heat: f64,
        rate_term: f64,
        remaining_structure_term: f64,
        reserve_term: f64,
        energy_term: Option<f64>,
    ) {
        let Some(m) = self.at(id) else { return };
        m.growth.ticks += 1;
        m.growth.reserve_spent += grown;
        m.growth.structure_gained += grown;
        m.growth.energy_cost += energy_cost;
        m.growth.heat += heat;
        // Which cap bound the step. Ties are attributed once, in the core's own `.min` order.
        let mut best = rate_term;
        let mut which = 0u8;
        for (value, tag) in [
            (remaining_structure_term, 1u8),
            (reserve_term, 2),
            (energy_term.unwrap_or(f64::INFINITY), 3),
        ] {
            if value < best {
                best = value;
                which = tag;
            }
        }
        match which {
            0 => m.growth.bound_by_rate += 1,
            1 => m.growth.bound_by_remaining_structure += 1,
            2 => m.growth.bound_by_reserve += 1,
            _ => m.growth.bound_by_energy += 1,
        }
        m.pending.reserve -= grown;
        m.pending.structure += grown;
        m.pending.energy -= energy_cost;
        let bin = m.bin(tick);
        bin.reserve_out_growth += grown;
        bin.energy_out += energy_cost;
        bin.growth_ticks += 1;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_upkeep(
        &mut self,
        id: OrganismId,
        tick: u64,
        maintenance: f64,
        movement: f64,
        sensing: f64,
        demanded: f64,
        paid: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        m.upkeep.ticks += 1;
        m.upkeep.demanded_maintenance += maintenance;
        m.upkeep.demanded_move += movement;
        m.upkeep.demanded_sense += sensing;
        m.upkeep.demanded_total += demanded;
        m.upkeep.paid += paid;
        let shortfall = (demanded - paid).max(0.0);
        m.upkeep.shortfall += shortfall;
        if shortfall > 0.0 {
            m.upkeep.shortfall_ticks += 1;
        }
        m.pending.energy -= paid;
        m.bin(tick).energy_out += paid;
    }

    pub fn record_strike(&mut self, id: OrganismId, tick: u64, demanded: f64, paid: f64) {
        let Some(m) = self.at(id) else { return };
        m.strike.ticks += 1;
        m.strike.demanded += demanded;
        m.strike.paid += paid;
        m.strike.shortfall += (demanded - paid).max(0.0);
        m.pending.energy -= paid;
        m.bin(tick).energy_out += paid;
    }

    pub fn record_handling(
        &mut self,
        id: OrganismId,
        tick: u64,
        demanded: f64,
        paid: f64,
        affordable: bool,
    ) {
        let Some(m) = self.at(id) else { return };
        m.handling.ticks += 1;
        m.handling.demanded += demanded;
        m.handling.paid += paid;
        if !affordable {
            m.handling.unaffordable_ticks += 1;
        }
        m.pending.energy -= paid;
        m.bin(tick).energy_out += paid;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_funding(
        &mut self,
        id: OrganismId,
        tick: u64,
        structure: f64,
        reserve: f64,
        energy: f64,
        build_heat: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        let reserve_debit = structure + reserve;
        let energy_debit = build_heat + energy;
        m.funding.funded_count += 1;
        m.funding.reserve_debit += reserve_debit;
        m.funding.energy_debit += energy_debit;
        m.funding.build_heat += build_heat;
        m.funding.escrow_structure += structure;
        m.funding.escrow_reserve += reserve;
        m.funding.escrow_energy += energy;
        m.pending.reserve -= reserve_debit;
        m.pending.energy -= energy_debit;
        let bin = m.bin(tick);
        bin.reserve_out_funding += reserve_debit;
        bin.energy_out += energy_debit;
    }

    pub fn record_refund(
        &mut self,
        id: OrganismId,
        tick: u64,
        structure: f64,
        reserve: f64,
        energy: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        m.funding.refunded_count += 1;
        m.funding.refunded_reserve += structure + reserve;
        m.funding.refunded_energy += energy;
        m.pending.reserve += structure + reserve;
        m.pending.energy += energy;
        // A refund is a returned escrow, not an intake channel: it moves the stocks back and
        // is deliberately not folded into any feeding total.
        m.bin(tick).energy_in += energy;
    }

    /// End-of-tick stock probe. Reconciles the tick's recorded flows against the stocks the
    /// world actually holds, then arms the next tick.
    pub fn probe(&mut self, id: OrganismId, tick: u64, structure: f64, reserve: f64, energy: f64) {
        let Some(m) = self.at(id) else { return };
        let now = Stocks {
            tick,
            structure,
            reserve,
            energy,
        };
        if m.pending.have_previous {
            let p = m.pending.previous;
            m.residual.observe(
                tick,
                (structure - p.structure) - m.pending.structure,
                (reserve - p.reserve) - m.pending.reserve,
                (energy - p.energy) - m.pending.energy,
            );
        }
        m.pending = Pending {
            have_previous: true,
            previous: now,
            ..Pending::default()
        };
        if m.first_observed_tick == u64::MAX {
            m.first_observed_tick = tick;
        }
        m.last_observed_tick = tick;
        m.close_stocks = now;
        let bin = m.bin(tick);
        bin.ticks += 1;
        bin.end_stocks = now;
    }

    /// The removal site: the member's last stocks, read before the body is dispersed. This
    /// closes the reconciliation for the death tick, which no end-of-tick probe reaches.
    pub fn record_death(
        &mut self,
        id: OrganismId,
        tick: u64,
        cause: &'static str,
        structure: f64,
        reserve: f64,
        energy: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        if m.pending.have_previous {
            let p = m.pending.previous;
            m.residual.observe(
                tick,
                (structure - p.structure) - m.pending.structure,
                (reserve - p.reserve) - m.pending.reserve,
                (energy - p.energy) - m.pending.energy,
            );
        }
        m.end_tick = Some(tick);
        m.end_cause = Some(cause);
        m.death_stocks = Some(Stocks {
            tick,
            structure,
            reserve,
            energy,
        });
        m.pending.have_previous = false;
    }

    /// Total reconciliation violations over every member. Zero is the gate.
    pub fn total_violations(&self) -> u64 {
        self.members
            .values()
            .map(|m| m.residual.violations)
            .sum::<u64>()
            + self.unregistered_records
    }
}

fn members_as_list<S: Serializer>(
    members: &BTreeMap<OrganismId, MemberFlow>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(members.values())
}

/// Which field-feeding channel a record came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedingChannel {
    Frugivory,
    Grazing,
    Scavenging,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(slot: u32) -> OrganismId {
        OrganismId {
            slot,
            generation: 1,
        }
    }

    fn ledger() -> FlowLedger {
        let mut l = FlowLedger::new(100);
        l.register(
            id(1),
            100,
            "Descendant",
            Some(id(2)),
            Stocks {
                tick: 100,
                structure: 0.8,
                reserve: 0.8,
                energy: 0.6,
            },
            2.0,
            4.0,
            4.0,
        );
        l
    }

    #[test]
    fn a_tick_whose_flows_match_the_stocks_leaves_no_residual() {
        let mut l = ledger();
        // Oxidation burns 0.0005 reserve into 0.0008 energy; upkeep then pays 0.0002.
        l.record_oxidation(id(1), 101, 0.0005, 0.0008, 0.0002, true);
        l.record_upkeep(id(1), 101, 0.0001, 0.00005, 0.00005, 0.0002, 0.0002);
        l.probe(id(1), 101, 0.8, 0.8 - 0.0005, 0.6 + 0.0008 - 0.0002);
        let m = &l.members[&id(1)];
        assert_eq!(m.residual.violations, 0);
        assert_eq!(m.residual.checked_ticks, 1);
        assert!(m.residual.max_reserve < RESIDUAL_TOLERANCE);
        assert_eq!(l.total_violations(), 0);
    }

    #[test]
    fn an_unrecorded_stock_movement_is_reported_as_a_residual() {
        let mut l = ledger();
        l.record_oxidation(id(1), 101, 0.0005, 0.0008, 0.0002, false);
        // The world also moved 0.01 of reserve through a site the ledger does not cover.
        l.probe(id(1), 101, 0.8, 0.8 - 0.0005 - 0.01, 0.6 + 0.0008);
        let m = &l.members[&id(1)];
        assert_eq!(m.residual.violations, 1);
        assert_eq!(m.residual.first_violation_tick, Some(101));
        assert!((m.residual.max_reserve - 0.01).abs() < 1e-12);
        assert_eq!(l.total_violations(), 1);
    }

    #[test]
    fn the_death_tick_reconciles_against_the_removal_site_not_a_probe() {
        let mut l = ledger();
        l.probe(id(1), 101, 0.8, 0.8, 0.6);
        l.record_upkeep(id(1), 102, 0.4, 0.1, 0.1, 0.6, 0.6);
        l.record_oxidation(id(1), 102, 0.8, 0.0, 1.6, true);
        l.record_death(id(1), 102, "Starvation", 0.8, 0.0, 0.0);
        let m = &l.members[&id(1)];
        assert_eq!(m.residual.violations, 0);
        assert_eq!(m.residual.checked_ticks, 2);
        assert_eq!(m.end_cause, Some("Starvation"));
        assert_eq!(m.death_stocks.unwrap().reserve, 0.0);
    }

    #[test]
    fn the_gate_records_both_sides_before_the_branch() {
        let mut l = ledger();
        // Below the gate by 0.4, then by 0.1, then above it.
        l.record_growth_gate(id(1), 101, 0.8, 2.0, 0.8, 1.2, 0.6);
        l.record_growth_gate(id(1), 102, 0.8, 2.0, 1.1, 1.2, 0.6);
        l.record_growth_gate(id(1), 103, 0.8, 2.0, 1.3, 1.2, 0.6);
        let g = l.members[&id(1)].gate;
        assert_eq!(g.observed_ticks, 3);
        assert_eq!(g.structure_below_adult_ticks, 3);
        assert_eq!(g.reserve_above_min_ticks, 1);
        assert_eq!(g.branch_entered_ticks, 1);
        assert!((g.min_reserve_deficit - 0.1).abs() < 1e-12);
        assert!((g.max_reserve - 1.3).abs() < 1e-12);
        assert_eq!(l.members[&id(1)].juvenile_ticks, 3);
        assert_eq!(l.members[&id(1)].adult_ticks, 0);
    }

    #[test]
    fn a_growth_step_names_the_cap_that_bound_it() {
        let mut l = ledger();
        // rate 0.0001, headroom 1.2, reserve 1.3, energy/build 5.0 -> the rate binds.
        l.record_growth(
            id(1),
            101,
            0.0001,
            0.00005,
            0.0003,
            0.0001,
            1.2,
            1.3,
            Some(5.0),
        );
        // reserve 0.00002 is now the smallest term.
        l.record_growth(
            id(1),
            102,
            0.00002,
            0.00001,
            0.0001,
            0.0001,
            1.2,
            0.00002,
            Some(5.0),
        );
        // the affordable-energy term binds.
        l.record_growth(
            id(1),
            103,
            0.000005,
            0.0000025,
            0.00002,
            0.0001,
            1.2,
            1.3,
            Some(0.000005),
        );
        let g = l.members[&id(1)].growth;
        assert_eq!(g.ticks, 3);
        assert_eq!(g.bound_by_rate, 1);
        assert_eq!(g.bound_by_reserve, 1);
        assert_eq!(g.bound_by_energy, 1);
        assert_eq!(g.bound_by_remaining_structure, 0);
        assert!((g.structure_gained - 0.000125).abs() < 1e-12);
    }

    #[test]
    fn upkeep_keeps_demand_and_payment_apart_instead_of_splitting_the_debit() {
        let mut l = ledger();
        // Demanded 0.6, only 0.25 in the battery.
        l.record_upkeep(id(1), 101, 0.4, 0.1, 0.1, 0.6, 0.25);
        let u = l.members[&id(1)].upkeep;
        assert!((u.demanded_maintenance - 0.4).abs() < 1e-12);
        assert!((u.demanded_move - 0.1).abs() < 1e-12);
        assert!((u.demanded_sense - 0.1).abs() < 1e-12);
        assert!((u.paid - 0.25).abs() < 1e-12);
        assert!((u.shortfall - 0.35).abs() < 1e-12);
        assert_eq!(u.shortfall_ticks, 1);
    }

    #[test]
    fn bins_are_fixed_width_and_bounded() {
        let mut l = ledger();
        for tick in 0..5 {
            l.probe(id(1), 100 + tick * BIN_TICKS, 0.8, 0.5, 0.5);
        }
        let m = &l.members[&id(1)];
        assert_eq!(m.bins.len(), 5);
        assert_eq!(m.bins[0].start_tick, 100);
        assert_eq!(m.bins[4].start_tick, 100 + 4 * BIN_TICKS);
        assert!(m.bins.iter().all(|b| b.ticks == 1));
        // Far beyond the retained span, the last bin absorbs the tail rather than growing.
        l.probe(id(1), 100 + 5000 * BIN_TICKS, 0.8, 0.5, 0.5);
        assert_eq!(l.members[&id(1)].bins.len(), MAX_BINS);
    }

    #[test]
    fn a_record_for_an_unregistered_member_is_counted_not_silently_dropped() {
        let mut l = ledger();
        l.record_oxidation(id(9), 101, 0.1, 0.1, 0.0, false);
        assert_eq!(l.unregistered_records, 1);
        assert_eq!(l.total_violations(), 1);
        assert!(!l.members.contains_key(&id(9)));
    }
}
