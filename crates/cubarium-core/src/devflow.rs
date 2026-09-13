//! A transient, write-only per-member developmental flow ledger, recorded at the exact
//! mutation sites in [`crate::world::World::step`].
//!
//! This is the pre-hunter sibling of the hunter-era `flow` module on
//! `diagnostic/hunter-juvenile-flow-2026-09-13`. It is deliberately a separate module with a
//! separate name: at this revision the four ordinary fauna forms are the entire fauna, there
//! is no `hunter.rs`, and none of the hunter-only channels (strikes, handling, captures, prey
//! digestion) exist. Importing hunter membership assumptions here would be importing a claim
//! this revision cannot support.
//!
//! Four rules keep it a measurement rather than an attribution:
//!
//! 1. **Every amount is read at the assignment that moves it**, in the same expression the
//!    core uses, and is never reconstructed from a post-step stock difference. A post-step
//!    delta cannot tell oxidation from growth; both move reserve in the same tick.
//! 2. **Gate state is read before the branch it gates.** [`GrowthGate`] records the growth
//!    predicate's two sides as the core evaluates them, so a closed gate is an *observation*,
//!    not the absence of a growth record.
//! 3. **Potential and actual are never mixed.** What a member *requested* from its own cell
//!    ([`Channel::requested_amount`]) and what actually transferred after contention
//!    ([`Channel::actual_amount`]) are separate fields, and the stock in the member's own cell
//!    ([`LocalCell`]) is availability, not intake.
//! 4. **Nothing here is ever read back by the tick.** The ledger is write-only from the
//!    world's point of view: no decision, draw, clamp, branch or state transition consults it.
//!    When it is absent (the default) every recording site is a null check.
//!
//! [`Reconciliation`] then proves the recording is complete rather than merely plausible: for
//! every member and every tick of its life, the stocks the world actually holds must equal the
//! stocks it held at the start of the tick plus exactly the flows recorded here, to
//! [`RESIDUAL_TOLERANCE`].
//!
//! Output is bounded by construction: one record per member with fixed-width aggregates.
//! There is no per-tick series — 144000 ticks times ~100 members is ~14 M rows and would say
//! nothing a per-member aggregate does not.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer};

use crate::ids::OrganismId;

/// Reconciliation tolerance, absolute. Flows are summed here in a different association order
/// than the world applies them, so exact bit equality is not available; this is a rounding
/// envelope, not slack that could hide a missing mutation site. A single missed site moves a
/// stock by ~1e-6 or more — four orders of magnitude above this.
pub const RESIDUAL_TOLERANCE: f64 = 1e-12;

/// The three stocks this ledger reconciles, read together at one point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Stocks {
    pub structure: f64,
    pub reserve: f64,
    pub energy: f64,
}

/// Escrowed offspring amounts, as the core holds them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct EscrowAmounts {
    pub structure: f64,
    pub reserve: f64,
    pub energy: f64,
}

/// What a body (or an unfinished gestation) hands to the detritus fields at the removal site.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct ToDetritus {
    pub material: f64,
    pub energy_kept: f64,
    pub heat: f64,
}

/// Maintenance, movement and sensing. The core pays these as one lumped debit clamped to the
/// battery, so the three components are recorded as *demanded* — each the core's own term —
/// beside the single amount actually paid. Splitting `paid` across the three would be
/// attribution, not measurement, and is deliberately not done.
///
/// `demand_total` is the core's own `cost`, i.e. `(m + v + s) * dt` associated exactly as the
/// core associates it. The three `demand_*` terms are each formed as `term * dt` for
/// reporting, so their sum need not be bit-identical to `demand_total`; only `demand_total`
/// and `paid_total` participate in reconciliation.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Upkeep {
    pub demand_maintenance: f64,
    pub demand_movement: f64,
    pub demand_sensing: f64,
    pub demand_total: f64,
    pub paid_total: f64,
    pub shortfall_total: f64,
    /// Ticks the member reached the upkeep payment at all.
    pub ticks: u64,
    /// Ticks where the battery could not cover the full demand.
    pub ticks_underpaid: u64,
    pub speed_sum: f64,
    pub speed_max: f64,
    pub wading_sum: f64,
    pub wading_max: f64,
    pub effort_sum: f64,
    /// Mean over `ticks`; filled by [`DevFlowLedger::finalize`].
    pub mean_speed: f64,
    pub mean_wading: f64,
    pub mean_effort: f64,
}

/// The oxidation branch and its predicate, read at the branch.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Oxidation {
    /// Ticks the branch body ran.
    pub ticks_fired: u64,
    /// Ticks the energy side of the predicate held (`E < oxidation_threshold * E_max`).
    pub ticks_threshold_open: u64,
    /// Ticks the energy side held but the reserve was empty, so nothing burned.
    pub ticks_blocked_by_empty_reserve: u64,
    pub reserve_burned: f64,
    pub energy_gained: f64,
    pub heat: f64,
}

/// The growth predicate, sampled at the mutation site *before* the branch, plus what the
/// branch did when it ran and which of its four caps bound the step.
///
/// `bound_by_*` counts every cap whose value equals the realized step, so a tie increments
/// more than one counter and the four need not sum to `positive_steps`.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct GrowthGate {
    /// Ticks the predicate was evaluated at all (the member reached the physiology pass).
    pub observations: u64,
    /// Ticks `structure < structure_adult`.
    pub structure_side_open: u64,
    /// Ticks `reserve > growth_reserve_min * reserve_max`.
    pub reserve_side_open: u64,
    pub both_open: u64,
    /// Ticks the branch body was entered (identical to `both_open`; recorded separately so a
    /// disagreement would be visible rather than assumed away).
    pub entered: u64,
    /// The constant gate `growth_reserve_min * reserve_max` for this member.
    pub threshold: f64,
    /// The largest reserve ever seen while the reserve side was *closed* — how near the member
    /// came to the gate from below. `0.0` with `closest_deficit` infinite if never closed.
    pub closest_approach_reserve: f64,
    /// `threshold - closest_approach_reserve`. `f64::INFINITY` when the reserve side was never
    /// closed at an observation.
    pub closest_deficit: f64,
    #[serde(skip)]
    pub reserve_sum: f64,
    /// `reserve_sum / observations`; filled by [`DevFlowLedger::finalize`].
    pub mean_pre_growth_reserve: f64,
    /// Observations with the reserve at exactly zero.
    pub ticks_reserve_zero: u64,
    /// Ticks the branch produced a strictly positive step.
    pub positive_steps: u64,
    pub structure_gained: f64,
    pub reserve_spent: f64,
    pub energy_spent: f64,
    pub heat: f64,
    pub bound_by_rate: u64,
    pub bound_by_headroom: u64,
    pub bound_by_reserve: u64,
    pub bound_by_energy: u64,
}

/// Gestation and budding on the member's own side.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Reproduction {
    /// Ticks the controller asked to bud.
    pub bud_decisions: u64,
    /// Bud requests refused because an escrow was already held.
    pub blocked_by_existing_escrow: u64,
    /// Bud requests refused at the population cap (the core's `cap_rejections` path).
    pub blocked_by_cap: u64,
    /// Affordability refusals; both can be counted on the same tick.
    pub blocked_by_reserve: u64,
    pub blocked_by_energy: u64,
    pub funded: u64,
    pub births_delivered: u64,
    /// Escrows refused at the cap on the delivery boundary and returned to the parent.
    pub refunds: u64,
    /// Gestations due on the tick the parent died, dropped by the core's `births.retain`.
    pub miscarriages: u64,
    pub reserve_debit: f64,
    pub energy_debit: f64,
    pub build_heat: f64,
    pub refund_reserve: f64,
    pub refund_energy: f64,
}

/// One field-feeding channel, with potential and actual kept strictly apart.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Channel {
    /// Ticks a strictly positive bite was *requested* from the member's own cell.
    pub request_ticks: u64,
    /// POTENTIAL: the sum of requested bites, before per-cell contention.
    pub requested_amount: f64,
    /// Ticks a strictly positive quantity actually transferred.
    pub actual_ticks: u64,
    /// ACTUAL: the sum of the quantities that actually transferred.
    pub actual_amount: f64,
    /// The material that actually reached the reserve (assimilation is below `actual_amount`).
    pub to_reserve: f64,
    pub energy: f64,
    pub heat: f64,
    /// Sum of the per-cell share multipliers on ticks this channel settled; 1.0 uncontested.
    pub share_sum: f64,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Intake {
    /// Ticks any channel actually transferred a positive quantity.
    pub ticks_any_actual_intake: u64,
    /// Ticks a request was formed at all.
    pub request_ticks: u64,
    pub frugivory: Channel,
    pub grazing: Channel,
    pub scavenging: Channel,
    /// Ticks a share multiplier below 1 applied to any channel this member settled.
    pub contested_ticks: u64,
    /// Ticks the member's reserve headroom was exhausted by its own requests
    /// (`f + g + s >= headroom` at the request site, which includes a full reserve).
    pub headroom_limited_ticks: u64,
    pub headroom_sum: f64,
}

/// How each sampled tick's own cell classified for access. Occupancy of a cell holding stock
/// is not an encounter and is certainly not intake; see the artifact's `notes`.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct AccessClassTicks {
    pub none: u64,
    pub producer_only: u64,
    pub detritus_only: u64,
    pub both: u64,
}

/// POTENTIAL access: the member's own cell, sampled once per stepped tick after it moved and
/// before any settlement touches the fields — exactly the stocks the intake request reads.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct LocalCell {
    pub sampled_ticks: u64,
    pub water_sum: f64,
    pub water_max: f64,
    pub ticks_water_positive: u64,
    pub producer_sum: f64,
    pub fruit_sum: f64,
    pub edible_detritus_sum: f64,
    pub ticks_producer_zero: u64,
    pub ticks_fruit_zero: u64,
    pub ticks_edible_detritus_zero: u64,
    /// Tracked beside the classification rather than folded into it: fruit is a third stock.
    pub ticks_fruit_present: u64,
    pub access_class_ticks: AccessClassTicks,
}

/// Per-tick reconciliation of recorded flows against the stocks the world actually holds.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Reconciliation {
    pub checks: u64,
    pub violations: u64,
    pub worst_residual: Stocks,
    pub first_violation_tick: Option<u64>,
}

impl Reconciliation {
    fn observe(&mut self, tick: u64, structure: f64, reserve: f64, energy: f64) {
        self.checks += 1;
        self.worst_residual.structure = self.worst_residual.structure.max(structure.abs());
        self.worst_residual.reserve = self.worst_residual.reserve.max(reserve.abs());
        self.worst_residual.energy = self.worst_residual.energy.max(energy.abs());
        if structure.abs() > RESIDUAL_TOLERANCE
            || reserve.abs() > RESIDUAL_TOLERANCE
            || energy.abs() > RESIDUAL_TOLERANCE
        {
            self.violations += 1;
            self.first_violation_tick.get_or_insert(tick);
        }
    }
}

/// The structural trajectory: where the member started, what it was aiming at, where it ended.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct StructureTrack {
    pub at_birth: f64,
    pub adult_target: f64,
    #[serde(rename = "final")]
    pub final_structure: f64,
    /// The first tick the member closed with `structure >= adult_target`; `None` is a member
    /// that never reached its target, not a zero.
    pub adult_recruitment_tick: Option<u64>,
    /// Classified by the structure the member *started* the tick with.
    pub ticks_juvenile: u64,
    pub ticks_adult: u64,
}

/// The payment that produced a descendant, copied from its parent's funding record at the
/// delivery boundary.
///
/// `refunded` is always `false` on a member that exists: a refused escrow is returned to the
/// parent and produces no member at all, and is counted in the parent's
/// [`Reproduction::refunds`]. The field is kept so the two paths are named in one place.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct BirthPayment {
    pub funded_tick: u64,
    pub escrow: EscrowAmounts,
    pub parent_reserve_debit: f64,
    pub parent_energy_debit: f64,
    pub build_heat: f64,
    pub refunded: bool,
    pub refund_tick: Option<u64>,
    /// The member whose death at this same boundary freed the slot this member was placed in.
    pub slot_freed_same_boundary_by: Option<OrganismId>,
}

/// The removal site.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct DeathRecord {
    /// The tick whose physiology pass chose the cause: the core's `now`.
    pub decision_tick: u64,
    /// The boundary the core stamps on the event: `now + 1`.
    pub event_tick: u64,
    pub cause: &'static str,
    /// As the core computes it for the event: `age_ticks(now + 1)`.
    pub age_ticks: u64,
    pub stocks_at_death: Stocks,
    pub escrow_at_death: Option<EscrowAmounts>,
    pub to_detritus: ToDetritus,
    /// A gestation that never finished, dispersed with its own clamp.
    pub escrow_to_detritus: Option<ToDetritus>,
    /// The member placed into this member's freed slot at the same boundary, if any.
    pub slot_reused_same_boundary_by: Option<OrganismId>,
    /// The last tick the member was stepped. Equal to `decision_tick`; recorded so the
    /// interval to `event_tick` is explicit rather than assumed.
    pub last_stepped_tick: u64,
    pub ticks_between_last_step_and_event: u64,
}

/// Everything recorded for one member over its whole life.
#[derive(Clone, Debug, Serialize)]
pub struct MemberRecord {
    pub id: OrganismId,
    pub form: u8,
    pub origin: &'static str,
    pub parent: Option<OrganismId>,
    /// The boundary the core stamped: `0` for a founder, `now + 1` for a descendant.
    pub born_tick: u64,
    pub first_observed_tick: u64,
    pub last_observed_tick: u64,
    pub reserve_max: f64,
    pub energy_max: f64,
    pub opening_stocks: Stocks,
    pub closing_stocks: Stocks,
    pub structure: StructureTrack,
    pub upkeep: Upkeep,
    pub oxidation: Oxidation,
    pub growth_gate: GrowthGate,
    pub reproduction: Reproduction,
    pub intake: Intake,
    pub local_cell: LocalCell,
    pub birth_payment: Option<BirthPayment>,
    pub death: Option<DeathRecord>,
    pub reconciliation: Reconciliation,
    /// The tick's working reconciliation state. Reset every tick; not a result.
    #[serde(skip)]
    pending: Pending,
    /// The funding this member has paid for and not yet delivered, so the child can be handed
    /// its own `birth_payment` at the delivery boundary.
    #[serde(skip)]
    open_funding: Option<BirthPayment>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    open: bool,
    start: Stocks,
    d_structure: f64,
    d_reserve: f64,
    d_energy: f64,
}

impl MemberRecord {
    fn new(id: OrganismId, form: u8, origin: &'static str, parent: Option<OrganismId>, born_tick: u64) -> MemberRecord {
        MemberRecord {
            id,
            form,
            origin,
            parent,
            born_tick,
            first_observed_tick: u64::MAX,
            last_observed_tick: 0,
            reserve_max: 0.0,
            energy_max: 0.0,
            opening_stocks: Stocks::default(),
            closing_stocks: Stocks::default(),
            structure: StructureTrack::default(),
            upkeep: Upkeep::default(),
            oxidation: Oxidation::default(),
            growth_gate: GrowthGate { closest_deficit: f64::INFINITY, ..GrowthGate::default() },
            reproduction: Reproduction::default(),
            intake: Intake::default(),
            local_cell: LocalCell::default(),
            birth_payment: None,
            death: None,
            reconciliation: Reconciliation::default(),
            pending: Pending::default(),
            open_funding: None,
        }
    }

    fn reconcile(&mut self, tick: u64, end: Stocks) {
        if !self.pending.open {
            return;
        }
        let start = self.pending.start;
        self.reconciliation.observe(
            tick,
            (end.structure - start.structure) - self.pending.d_structure,
            (end.reserve - start.reserve) - self.pending.d_reserve,
            (end.energy - start.energy) - self.pending.d_energy,
        );
        self.pending = Pending::default();
    }
}

/// Which field-feeding channel a record came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedingChannel {
    Frugivory,
    Grazing,
    Scavenging,
}

/// The ledger. Absent by default; every recording site in the world is a null check when it
/// is. The world never reads it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DevFlowLedger {
    pub opened_tick: u64,
    /// Keyed by the full slot/generation ID, serialized as a list: a JSON object cannot key on
    /// a composite ID, and flattening one to a string would lose the generation.
    #[serde(serialize_with = "members_as_list")]
    pub members: BTreeMap<OrganismId, MemberRecord>,
    /// Records arriving for a member the ledger never opened a tick for. Always zero in a
    /// sound run; nonzero means a lifecycle edge was missed.
    pub unregistered_records: u64,
    /// Slot -> the member removed from it at the boundary currently being committed.
    #[serde(skip)]
    freed_this_boundary: BTreeMap<u32, OrganismId>,
}

impl DevFlowLedger {
    pub fn new(opened_tick: u64) -> DevFlowLedger {
        DevFlowLedger { opened_tick, ..DevFlowLedger::default() }
    }

    fn at(&mut self, id: OrganismId) -> Option<&mut MemberRecord> {
        match self.members.get_mut(&id) {
            Some(m) => Some(m),
            None => {
                self.unregistered_records += 1;
                None
            }
        }
    }

    /// Open the member's tick: the stocks it starts with, before upkeep is paid. Registers the
    /// member on first sight, which is how founders enter the ledger.
    #[allow(clippy::too_many_arguments)]
    pub fn open_tick(
        &mut self,
        id: OrganismId,
        tick: u64,
        form: u8,
        origin: &'static str,
        parent: Option<OrganismId>,
        born_tick: u64,
        stocks: Stocks,
        structure_adult: f64,
        reserve_max: f64,
        energy_max: f64,
    ) {
        let m = self
            .members
            .entry(id)
            .or_insert_with(|| MemberRecord::new(id, form, origin, parent, born_tick));
        m.reserve_max = reserve_max;
        m.energy_max = energy_max;
        m.structure.adult_target = structure_adult;
        if m.first_observed_tick == u64::MAX {
            m.first_observed_tick = tick;
            m.opening_stocks = stocks;
            // A descendant's opening structure is its escrow; a founder's is what it was
            // placed with. Both are the structure at the first tick it was ever stepped.
            m.structure.at_birth = stocks.structure;
        }
        if stocks.structure < structure_adult {
            m.structure.ticks_juvenile += 1;
        } else {
            m.structure.ticks_adult += 1;
        }
        m.pending = Pending { open: true, start: stocks, ..Pending::default() };
    }

    /// Step 6, the lumped upkeep debit. `demand_total` is the core's own `cost`.
    #[allow(clippy::too_many_arguments)]
    pub fn record_upkeep(
        &mut self,
        id: OrganismId,
        maintenance: f64,
        movement: f64,
        sensing: f64,
        demand_total: f64,
        paid: f64,
        speed: f64,
        wading: f64,
        effort: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        let u = &mut m.upkeep;
        u.ticks += 1;
        u.demand_maintenance += maintenance;
        u.demand_movement += movement;
        u.demand_sensing += sensing;
        u.demand_total += demand_total;
        u.paid_total += paid;
        let shortfall = (demand_total - paid).max(0.0);
        u.shortfall_total += shortfall;
        if paid < demand_total {
            u.ticks_underpaid += 1;
        }
        u.speed_sum += speed;
        u.speed_max = u.speed_max.max(speed);
        u.wading_sum += wading;
        u.wading_max = u.wading_max.max(wading);
        u.effort_sum += effort;
        m.pending.d_energy -= paid;
    }

    /// Step 6, after the move: the stocks standing in the member's own cell. POTENTIAL access.
    #[allow(clippy::too_many_arguments)]
    pub fn record_local_cell(
        &mut self,
        id: OrganismId,
        water: f64,
        producer: f64,
        fruit: f64,
        edible_detritus: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        let c = &mut m.local_cell;
        c.sampled_ticks += 1;
        c.water_sum += water;
        c.water_max = c.water_max.max(water);
        if water > 0.0 {
            c.ticks_water_positive += 1;
        }
        c.producer_sum += producer;
        c.fruit_sum += fruit;
        c.edible_detritus_sum += edible_detritus;
        if producer <= 0.0 {
            c.ticks_producer_zero += 1;
        }
        if fruit <= 0.0 {
            c.ticks_fruit_zero += 1;
        } else {
            c.ticks_fruit_present += 1;
        }
        if edible_detritus <= 0.0 {
            c.ticks_edible_detritus_zero += 1;
        }
        match (producer > 0.0, edible_detritus > 0.0) {
            (false, false) => c.access_class_ticks.none += 1,
            (true, false) => c.access_class_ticks.producer_only += 1,
            (false, true) => c.access_class_ticks.detritus_only += 1,
            (true, true) => c.access_class_ticks.both += 1,
        }
    }

    /// Step 7, the request: POTENTIAL bites, formed from the member's own cell before any
    /// contention and before a single transfer.
    #[allow(clippy::too_many_arguments)]
    pub fn record_intake_request(
        &mut self,
        id: OrganismId,
        headroom: f64,
        fruit: f64,
        graze: f64,
        scavenge: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        let i = &mut m.intake;
        i.request_ticks += 1;
        i.headroom_sum += headroom;
        if fruit + graze + scavenge >= headroom {
            i.headroom_limited_ticks += 1;
        }
        for (amount, channel) in [(fruit, 0usize), (graze, 1), (scavenge, 2)] {
            if amount <= 0.0 {
                continue;
            }
            let slot = match channel {
                0 => &mut i.frugivory,
                1 => &mut i.grazing,
                _ => &mut i.scavenging,
            };
            slot.request_ticks += 1;
            slot.requested_amount += amount;
        }
    }

    /// Step 7, the settlement: ACTUAL transfer after the per-cell proportional share.
    #[allow(clippy::too_many_arguments)]
    pub fn record_intake_settlement(
        &mut self,
        id: OrganismId,
        channel: FeedingChannel,
        share: f64,
        quantity: f64,
        to_reserve: f64,
        energy: f64,
        heat: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        let slot = match channel {
            FeedingChannel::Frugivory => &mut m.intake.frugivory,
            FeedingChannel::Grazing => &mut m.intake.grazing,
            FeedingChannel::Scavenging => &mut m.intake.scavenging,
        };
        slot.actual_ticks += 1;
        slot.actual_amount += quantity;
        slot.to_reserve += to_reserve;
        slot.energy += energy;
        slot.heat += heat;
        slot.share_sum += share;
        m.pending.d_reserve += to_reserve;
        m.pending.d_energy += energy;
    }

    /// Step 7, once per member per settled tick: whether anything actually transferred, and
    /// whether any channel it settled was contended.
    pub fn record_intake_close(&mut self, id: OrganismId, any_intake: bool, contested: bool) {
        let Some(m) = self.at(id) else { return };
        if any_intake {
            m.intake.ticks_any_actual_intake += 1;
        }
        if contested {
            m.intake.contested_ticks += 1;
        }
    }

    /// Step 8, the oxidation predicate and, when it fired, the branch.
    #[allow(clippy::too_many_arguments)]
    pub fn record_oxidation(
        &mut self,
        id: OrganismId,
        threshold_open: bool,
        fired: bool,
        burned: f64,
        gained: f64,
        heat: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        let o = &mut m.oxidation;
        if threshold_open {
            o.ticks_threshold_open += 1;
            if !fired {
                o.ticks_blocked_by_empty_reserve += 1;
            }
        }
        if fired {
            o.ticks_fired += 1;
            o.reserve_burned += burned;
            o.energy_gained += gained;
            o.heat += heat;
            m.pending.d_reserve -= burned;
            m.pending.d_energy += gained;
        }
    }

    /// Step 8, the growth predicate, read before the branch it gates.
    #[allow(clippy::too_many_arguments)]
    pub fn record_growth_gate(
        &mut self,
        id: OrganismId,
        structure: f64,
        structure_adult: f64,
        reserve: f64,
        threshold: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        let g = &mut m.growth_gate;
        g.observations += 1;
        g.threshold = threshold;
        g.reserve_sum += reserve;
        let structure_open = structure < structure_adult;
        let reserve_open = reserve > threshold;
        if structure_open {
            g.structure_side_open += 1;
        }
        if reserve_open {
            g.reserve_side_open += 1;
        } else {
            // Closed: how near it came from below.
            if g.closest_deficit.is_infinite() || reserve > g.closest_approach_reserve {
                g.closest_approach_reserve = reserve;
                g.closest_deficit = threshold - reserve;
            }
        }
        if structure_open && reserve_open {
            g.both_open += 1;
        }
        if reserve == 0.0 {
            g.ticks_reserve_zero += 1;
        }
    }

    /// Step 8, the growth branch was entered (the predicate held). Recorded even when the
    /// step it produces is zero.
    pub fn record_growth_entered(&mut self, id: OrganismId) {
        let Some(m) = self.at(id) else { return };
        m.growth_gate.entered += 1;
    }

    /// Step 8, a realized growth step and which of the four caps bound it. `cap_energy` is
    /// `None` exactly when `build_cost == 0` and the core never formed that cap.
    #[allow(clippy::too_many_arguments)]
    pub fn record_growth(
        &mut self,
        id: OrganismId,
        grown: f64,
        cost: f64,
        heat: f64,
        cap_rate: f64,
        cap_headroom: f64,
        cap_reserve: f64,
        cap_energy: Option<f64>,
    ) {
        let Some(m) = self.at(id) else { return };
        let g = &mut m.growth_gate;
        g.positive_steps += 1;
        g.structure_gained += grown;
        g.reserve_spent += grown;
        g.energy_spent += cost;
        g.heat += heat;
        if cap_rate == grown {
            g.bound_by_rate += 1;
        }
        if cap_headroom == grown {
            g.bound_by_headroom += 1;
        }
        if cap_reserve == grown {
            g.bound_by_reserve += 1;
        }
        if cap_energy == Some(grown) {
            g.bound_by_energy += 1;
        }
        m.pending.d_structure += grown;
        m.pending.d_reserve -= grown;
        m.pending.d_energy -= cost;
    }

    /// Step 8, the gestation state read before the budding branch.
    pub fn record_gestation_observation(&mut self, id: OrganismId, bud: bool, escrow_held: bool) {
        let Some(m) = self.at(id) else { return };
        if bud {
            m.reproduction.bud_decisions += 1;
            if escrow_held {
                m.reproduction.blocked_by_existing_escrow += 1;
            }
        }
    }

    /// Step 8, a bud request refused at the population cap.
    pub fn record_bud_cap_rejection(&mut self, id: OrganismId) {
        let Some(m) = self.at(id) else { return };
        m.reproduction.blocked_by_cap += 1;
    }

    /// Step 8, a bud request that could not be afforded. Both sides are recorded, so a tick
    /// blocked on both increments both.
    pub fn record_bud_unaffordable(&mut self, id: OrganismId, reserve_ok: bool, energy_ok: bool) {
        let Some(m) = self.at(id) else { return };
        if !reserve_ok {
            m.reproduction.blocked_by_reserve += 1;
        }
        if !energy_ok {
            m.reproduction.blocked_by_energy += 1;
        }
    }

    /// Step 8, an escrow funded: the parent's exact debits and the build heat.
    #[allow(clippy::too_many_arguments)]
    pub fn record_funding(
        &mut self,
        id: OrganismId,
        tick: u64,
        structure: f64,
        reserve: f64,
        energy: f64,
        build: f64,
    ) {
        let Some(m) = self.at(id) else { return };
        let reserve_debit = structure + reserve;
        let energy_debit = build + energy;
        let r = &mut m.reproduction;
        r.funded += 1;
        r.reserve_debit += reserve_debit;
        r.energy_debit += energy_debit;
        r.build_heat += build;
        m.pending.d_reserve -= reserve_debit;
        m.pending.d_energy -= energy_debit;
        m.open_funding = Some(BirthPayment {
            funded_tick: tick,
            escrow: EscrowAmounts { structure, reserve, energy },
            parent_reserve_debit: reserve_debit,
            parent_energy_debit: energy_debit,
            build_heat: build,
            refunded: false,
            refund_tick: None,
            slot_freed_same_boundary_by: None,
        });
    }

    /// Step 8, a gestation due on the tick its parent died: dropped by `births.retain`.
    pub fn record_miscarriage(&mut self, id: OrganismId) {
        let Some(m) = self.at(id) else { return };
        m.reproduction.miscarriages += 1;
    }

    /// Step 9, the boundary begins: nothing has been removed yet.
    pub fn open_boundary(&mut self) {
        self.freed_this_boundary.clear();
    }

    /// Step 9, the removal site: the member's last stocks, read before the body is dispersed.
    /// No end-of-tick sweep reaches the tick a member dies on, so this closes its
    /// reconciliation.
    #[allow(clippy::too_many_arguments)]
    pub fn record_death(
        &mut self,
        id: OrganismId,
        decision_tick: u64,
        cause: &'static str,
        age_ticks: u64,
        stocks: Stocks,
        escrow: Option<EscrowAmounts>,
        body: ToDetritus,
        escrow_to_detritus: Option<ToDetritus>,
    ) {
        self.freed_this_boundary.insert(id.slot, id);
        let Some(m) = self.at(id) else { return };
        m.reconcile(decision_tick, stocks);
        m.last_observed_tick = decision_tick;
        m.closing_stocks = stocks;
        m.structure.final_structure = stocks.structure;
        m.death = Some(DeathRecord {
            decision_tick,
            event_tick: decision_tick + 1,
            cause,
            age_ticks,
            stocks_at_death: stocks,
            escrow_at_death: escrow,
            to_detritus: body,
            escrow_to_detritus,
            slot_reused_same_boundary_by: None,
            last_stepped_tick: decision_tick,
            ticks_between_last_step_and_event: 1,
        });
    }

    /// Step 9, an escrow refused at the cap and returned to the parent untouched.
    pub fn record_refund(&mut self, id: OrganismId, tick: u64, structure: f64, reserve: f64, energy: f64) {
        let Some(m) = self.at(id) else { return };
        let r = &mut m.reproduction;
        r.refunds += 1;
        r.refund_reserve += structure + reserve;
        r.refund_energy += energy;
        m.pending.d_reserve += structure + reserve;
        m.pending.d_energy += energy;
        if let Some(f) = &mut m.open_funding {
            f.refunded = true;
            f.refund_tick = Some(tick);
        }
        m.open_funding = None;
    }

    /// Step 9, a child placed. Registers it with the payment its parent made, and links the
    /// slot-reuse pair when the slot was freed by a death at this same boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn record_birth(
        &mut self,
        child: OrganismId,
        parent: OrganismId,
        born_tick: u64,
        form: u8,
        stocks: Stocks,
        structure_adult: f64,
        reserve_max: f64,
        energy_max: f64,
    ) {
        let freed_by = self.freed_this_boundary.get(&child.slot).copied();
        let mut payment = match self.members.get_mut(&parent) {
            Some(p) => {
                p.reproduction.births_delivered += 1;
                p.open_funding.take()
            }
            None => {
                self.unregistered_records += 1;
                None
            }
        };
        if let Some(p) = &mut payment {
            p.slot_freed_same_boundary_by = freed_by;
        }
        if let Some(dead) = freed_by {
            if let Some(d) = self.members.get_mut(&dead) {
                if let Some(record) = &mut d.death {
                    record.slot_reused_same_boundary_by = Some(child);
                }
            }
        }
        let m = self
            .members
            .entry(child)
            .or_insert_with(|| MemberRecord::new(child, form, "descendant", Some(parent), born_tick));
        m.reserve_max = reserve_max;
        m.energy_max = energy_max;
        m.structure.adult_target = structure_adult;
        m.structure.at_birth = stocks.structure;
        m.opening_stocks = stocks;
        m.closing_stocks = stocks;
        m.structure.final_structure = stocks.structure;
        m.birth_payment = payment;
    }

    /// End of tick: close every member still alive, reconciling the tick's flows against the
    /// stocks the world actually holds. Members removed this tick were already closed at their
    /// removal site.
    pub fn close_tick(&mut self, id: OrganismId, tick: u64, stocks: Stocks) {
        let Some(m) = self.at(id) else { return };
        if !m.pending.open {
            // A member born at this boundary was never stepped this tick.
            return;
        }
        m.reconcile(tick, stocks);
        m.last_observed_tick = tick;
        m.closing_stocks = stocks;
        m.structure.final_structure = stocks.structure;
        if m.structure.adult_recruitment_tick.is_none() && stocks.structure >= m.structure.adult_target {
            m.structure.adult_recruitment_tick = Some(tick);
        }
    }

    /// Derived means, computed once at the end. Nothing here is a new measurement.
    pub fn finalize(&mut self) {
        for m in self.members.values_mut() {
            if m.upkeep.ticks > 0 {
                let n = m.upkeep.ticks as f64;
                m.upkeep.mean_speed = m.upkeep.speed_sum / n;
                m.upkeep.mean_wading = m.upkeep.wading_sum / n;
                m.upkeep.mean_effort = m.upkeep.effort_sum / n;
            }
            if m.growth_gate.observations > 0 {
                m.growth_gate.mean_pre_growth_reserve =
                    m.growth_gate.reserve_sum / m.growth_gate.observations as f64;
            }
            if m.first_observed_tick == u64::MAX {
                m.first_observed_tick = m.born_tick;
            }
        }
    }

    /// Total reconciliation checks, violations, and the worst residual per stock over every
    /// member. Zero violations is the gate.
    pub fn totals(&self) -> Reconciliation {
        let mut total = Reconciliation::default();
        for m in self.members.values() {
            total.checks += m.reconciliation.checks;
            total.violations += m.reconciliation.violations;
            total.worst_residual.structure =
                total.worst_residual.structure.max(m.reconciliation.worst_residual.structure);
            total.worst_residual.reserve =
                total.worst_residual.reserve.max(m.reconciliation.worst_residual.reserve);
            total.worst_residual.energy =
                total.worst_residual.energy.max(m.reconciliation.worst_residual.energy);
            if let Some(t) = m.reconciliation.first_violation_tick {
                total.first_violation_tick = Some(total.first_violation_tick.map_or(t, |o: u64| o.min(t)));
            }
        }
        total
    }
}

fn members_as_list<S: Serializer>(
    members: &BTreeMap<OrganismId, MemberRecord>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(members.values())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(slot: u32, generation: u32) -> OrganismId {
        OrganismId { slot, generation }
    }

    fn open(l: &mut DevFlowLedger, who: OrganismId, tick: u64, stocks: Stocks) {
        l.open_tick(who, tick, 0, "founder", None, 0, stocks, 2.0, 4.0, 3.0);
    }

    #[test]
    fn a_tick_whose_flows_match_the_stocks_leaves_no_residual() {
        let mut l = DevFlowLedger::new(0);
        let a = id(0, 1);
        open(&mut l, a, 0, Stocks { structure: 1.0, reserve: 2.0, energy: 1.0 });
        // Upkeep pays 0.25; grazing moves 0.5 into reserve and 0.125 into the battery;
        // oxidation burns 0.25 of reserve into 0.375 of energy.
        l.record_upkeep(a, 0.1, 0.1, 0.05, 0.25, 0.25, 1.0, 1.0, 1.0);
        l.record_intake_settlement(a, FeedingChannel::Grazing, 1.0, 0.6, 0.5, 0.125, 0.01);
        l.record_oxidation(a, true, true, 0.25, 0.375, 0.02);
        // reserve: 2.0 + 0.5 grazed - 0.25 oxidised = 2.25.
        // energy:  1.0 + 0.125 grazed + 0.375 oxidised - 0.25 upkeep = 1.25.
        l.close_tick(a, 0, Stocks { structure: 1.0, reserve: 2.25, energy: 1.25 });
        let t = l.totals();
        assert_eq!((t.checks, t.violations), (1, 0));
        assert_eq!(t.worst_residual, Stocks::default());
    }

    #[test]
    fn the_terminal_transaction_closes_the_member_and_disperses_both_bodies() {
        let mut l = DevFlowLedger::new(0);
        let a = id(3, 2);
        open(&mut l, a, 42, Stocks { structure: 1.0, reserve: 0.5, energy: 0.3 });
        l.record_upkeep(a, 0.06, 0.02, 0.02, 0.1, 0.1, 1.0, 1.0, 1.0);
        l.open_boundary();
        // The removal site closes the tick: no end-of-tick sweep reaches the tick a member
        // dies on, so this is the only reconciliation the member's last tick ever gets.
        l.record_death(
            a,
            42,
            "starvation",
            900,
            Stocks { structure: 1.0, reserve: 0.5, energy: 0.2 },
            Some(EscrowAmounts { structure: 0.4, reserve: 0.3, energy: 0.2 }),
            ToDetritus { material: 1.5, energy_kept: 0.45, heat: 0.05 },
            Some(ToDetritus { material: 0.7, energy_kept: 0.3, heat: 0.02 }),
        );
        let t = l.totals();
        assert_eq!((t.checks, t.violations), (1, 0), "the terminal site must reconcile, not skip");
        let d = l.members[&a].death.expect("a death record");
        assert_eq!((d.decision_tick, d.event_tick), (42, 43), "the core stamps the event at now + 1");
        assert_eq!(d.last_stepped_tick, 42);
        assert_eq!(d.ticks_between_last_step_and_event, 1, "the final interval is explicit");
        assert_eq!(d.cause, "starvation");
        assert_eq!(d.to_detritus.material, 1.5);
        let e = d.escrow_to_detritus.expect("a miscarried escrow disperses separately");
        assert_eq!(e.material, 0.7, "the body and the escrow are never merged");
    }

    #[test]
    fn the_funding_a_parent_paid_is_the_escrow_its_child_opens_with() {
        let mut l = DevFlowLedger::new(0);
        let (parent, child) = (id(6, 1), id(9, 1));
        open(&mut l, parent, 10, Stocks { structure: 1.0, reserve: 2.0, energy: 1.0 });
        l.record_funding(parent, 10, 0.4, 0.3, 0.2, 0.05);
        // reserve debit 0.4 + 0.3, energy debit 0.05 + 0.2.
        l.close_tick(parent, 10, Stocks { structure: 1.0, reserve: 1.3, energy: 0.75 });
        l.open_boundary();
        l.record_birth(
            child,
            parent,
            11,
            3,
            Stocks { structure: 0.4, reserve: 0.3, energy: 0.2 },
            0.9,
            0.85,
            1.8,
        );
        let t = l.totals();
        assert_eq!((t.checks, t.violations), (1, 0), "funding must reconcile against the parent's stocks");
        let p = &l.members[&parent];
        assert_eq!((p.reproduction.funded, p.reproduction.births_delivered), (1, 1));
        assert_eq!(p.reproduction.reserve_debit, 0.7);
        assert_eq!(p.reproduction.energy_debit, 0.25);
        let c = &l.members[&child];
        assert_eq!(c.origin, "descendant");
        assert_eq!(c.parent, Some(parent));
        assert_eq!(c.born_tick, 11);
        let pay = c.birth_payment.expect("the child carries the payment its parent made");
        assert_eq!(pay.funded_tick, 10, "the child is priced at the tick the parent paid");
        assert_eq!((pay.escrow.structure, pay.escrow.reserve, pay.escrow.energy), (0.4, 0.3, 0.2));
        assert_eq!(pay.parent_reserve_debit, 0.7);
        assert_eq!(pay.parent_energy_debit, 0.25);
        assert_eq!(pay.build_heat, 0.05);
        assert!(!pay.refunded);
        assert_eq!(pay.slot_freed_same_boundary_by, None, "slot 9 was not freed this boundary");
    }

    #[test]
    fn each_feeding_channel_keeps_its_own_source_and_they_reconcile_together() {
        let mut l = DevFlowLedger::new(0);
        let a = id(1, 1);
        open(&mut l, a, 5, Stocks { structure: 1.0, reserve: 0.0, energy: 0.0 });
        l.record_intake_settlement(a, FeedingChannel::Frugivory, 1.0, 0.2, 0.10, 0.01, 0.0);
        l.record_intake_settlement(a, FeedingChannel::Grazing, 1.0, 0.5, 0.25, 0.02, 0.0);
        l.record_intake_settlement(a, FeedingChannel::Scavenging, 1.0, 0.8, 0.40, 0.04, 0.0);
        l.close_tick(a, 5, Stocks { structure: 1.0, reserve: 0.75, energy: 0.07 });
        let t = l.totals();
        assert_eq!((t.checks, t.violations), (1, 0));
        let i = &l.members[&a].intake;
        assert_eq!(i.frugivory.to_reserve, 0.10);
        assert_eq!(i.grazing.to_reserve, 0.25);
        assert_eq!(i.scavenging.to_reserve, 0.40);
        assert_eq!(i.frugivory.energy, 0.01);
        assert_eq!(i.grazing.energy, 0.02);
        assert_eq!(i.scavenging.energy, 0.04);
        assert_eq!(i.ticks_any_actual_intake, 0, "the tick is only counted when the close says so");
    }

    #[test]
    fn a_missing_mutation_site_shows_up_as_a_violation() {
        let mut l = DevFlowLedger::new(0);
        let a = id(0, 1);
        open(&mut l, a, 7, Stocks { structure: 1.0, reserve: 2.0, energy: 1.0 });
        // The world burned 0.25 of reserve and the ledger was never told.
        l.close_tick(a, 7, Stocks { structure: 1.0, reserve: 1.75, energy: 1.0 });
        let t = l.totals();
        assert_eq!((t.checks, t.violations), (1, 1));
        assert_eq!(t.first_violation_tick, Some(7));
        // `worst_residual` is a magnitude: the gate compares it against the tolerance, so the
        // direction of the miss is deliberately not carried here.
        assert!((t.worst_residual.reserve - 0.25).abs() < 1e-15);
    }

    #[test]
    fn growth_moves_all_three_stocks_and_attributes_its_cap() {
        let mut l = DevFlowLedger::new(0);
        let a = id(0, 1);
        open(&mut l, a, 0, Stocks { structure: 1.0, reserve: 2.0, energy: 1.0 });
        l.record_growth_gate(a, 1.0, 2.0, 2.0, 1.2);
        l.record_growth_entered(a);
        // rate 0.01, headroom 1.0, reserve 2.0, energy cap 0.5: the rate binds.
        l.record_growth(a, 0.01, 0.02, 0.05, 0.01, 1.0, 2.0, Some(0.5));
        l.close_tick(a, 0, Stocks { structure: 1.01, reserve: 1.99, energy: 0.98 });
        let m = &l.members[&a];
        assert_eq!(m.growth_gate.observations, 1);
        assert_eq!(m.growth_gate.structure_side_open, 1);
        assert_eq!(m.growth_gate.reserve_side_open, 1);
        assert_eq!(m.growth_gate.both_open, 1);
        assert_eq!(m.growth_gate.entered, 1);
        assert_eq!(
            (
                m.growth_gate.bound_by_rate,
                m.growth_gate.bound_by_headroom,
                m.growth_gate.bound_by_reserve,
                m.growth_gate.bound_by_energy
            ),
            (1, 0, 0, 0)
        );
        assert_eq!(l.totals().violations, 0);
    }

    #[test]
    fn each_of_the_four_growth_caps_is_attributed_when_it_binds() {
        // The replay may never exercise three of these, so they are pinned here.
        let cases: [(f64, f64, f64, Option<f64>, (u64, u64, u64, u64)); 4] = [
            (0.01, 1.0, 2.0, Some(0.5), (1, 0, 0, 0)),
            (1.0, 0.01, 2.0, Some(0.5), (0, 1, 0, 0)),
            (1.0, 2.0, 0.01, Some(0.5), (0, 0, 1, 0)),
            (1.0, 2.0, 3.0, Some(0.01), (0, 0, 0, 1)),
        ];
        for (rate, headroom, reserve, energy, want) in cases {
            let mut l = DevFlowLedger::new(0);
            let a = id(0, 1);
            open(&mut l, a, 0, Stocks { structure: 1.0, reserve: 2.0, energy: 1.0 });
            let grown = rate.min(headroom).min(reserve).min(energy.unwrap_or(f64::INFINITY));
            l.record_growth(a, grown, 0.0, 0.0, rate, headroom, reserve, energy);
            let g = &l.members[&a].growth_gate;
            assert_eq!(
                (g.bound_by_rate, g.bound_by_headroom, g.bound_by_reserve, g.bound_by_energy),
                want,
                "caps {rate} {headroom} {reserve} {energy:?}"
            );
        }
        // With `build_cost == 0` the core never forms an energy cap, and `None` never binds.
        let mut l = DevFlowLedger::new(0);
        let a = id(0, 1);
        open(&mut l, a, 0, Stocks { structure: 1.0, reserve: 2.0, energy: 1.0 });
        l.record_growth(a, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, None);
        assert_eq!(l.members[&a].growth_gate.bound_by_energy, 0);
    }

    #[test]
    fn a_closed_reserve_gate_is_an_observation_with_a_deficit() {
        let mut l = DevFlowLedger::new(0);
        let a = id(0, 1);
        open(&mut l, a, 0, Stocks { structure: 1.0, reserve: 0.0, energy: 1.0 });
        l.record_growth_gate(a, 1.0, 2.0, 0.0, 1.2);
        l.record_growth_gate(a, 1.0, 2.0, 0.9, 1.2);
        l.record_growth_gate(a, 1.0, 2.0, 0.4, 1.2);
        let g = &l.members[&a].growth_gate;
        assert_eq!((g.observations, g.reserve_side_open, g.both_open, g.entered), (3, 0, 0, 0));
        assert_eq!(g.ticks_reserve_zero, 1);
        assert!((g.closest_approach_reserve - 0.9).abs() < 1e-15);
        assert!((g.closest_deficit - 0.3).abs() < 1e-15);
        l.finalize();
        let g = &l.members[&a].growth_gate;
        assert!((g.mean_pre_growth_reserve - (0.0 + 0.9 + 0.4) / 3.0).abs() < 1e-15);
    }

    #[test]
    fn a_slot_reused_at_the_same_boundary_is_recorded_on_both_sides() {
        let mut l = DevFlowLedger::new(0);
        let dead = id(4, 1);
        let parent = id(9, 1);
        open(&mut l, dead, 100, Stocks { structure: 1.0, reserve: 0.0, energy: 0.0 });
        open(&mut l, parent, 100, Stocks { structure: 2.0, reserve: 3.0, energy: 3.0 });
        l.record_funding(parent, 100, 0.4, 0.8, 0.6, 0.2);
        l.open_boundary();
        l.record_death(
            dead,
            100,
            "Starvation",
            100,
            Stocks { structure: 1.0, reserve: 0.0, energy: 0.0 },
            None,
            ToDetritus { material: 1.0, energy_kept: 0.0, heat: 0.0 },
            None,
        );
        // The core hands the freed slot 4 to the next insert, with a bumped generation.
        let child = id(4, 2);
        l.record_birth(
            child,
            parent,
            101,
            0,
            Stocks { structure: 0.4, reserve: 0.8, energy: 0.6 },
            2.0,
            4.0,
            3.0,
        );
        let d = l.members[&dead].death.expect("death recorded");
        assert_eq!(d.slot_reused_same_boundary_by, Some(child));
        assert_eq!(d.decision_tick, 100);
        assert_eq!(d.event_tick, 101);
        assert_eq!(d.ticks_between_last_step_and_event, 1);
        let payment = l.members[&child].birth_payment.expect("payment carried to the child");
        assert_eq!(payment.slot_freed_same_boundary_by, Some(dead));
        assert_eq!(payment.funded_tick, 100);
        assert!((payment.parent_reserve_debit - 1.2).abs() < 1e-15);
        assert!((payment.parent_energy_debit - 0.8).abs() < 1e-15);
        // The stale ID never resolves to the new occupant.
        assert_eq!(l.members[&dead].id.generation, 1);
        assert_eq!(l.members[&child].id.generation, 2);
        assert_eq!(l.members[&parent].reproduction.births_delivered, 1);
        assert_eq!(l.unregistered_records, 0);
    }

    #[test]
    fn a_refund_returns_the_escrow_and_reconciles() {
        let mut l = DevFlowLedger::new(0);
        let p = id(1, 1);
        open(&mut l, p, 50, Stocks { structure: 2.0, reserve: 3.0, energy: 3.0 });
        l.record_refund(p, 50, 0.4, 0.8, 0.6);
        l.close_tick(p, 50, Stocks { structure: 2.0, reserve: 4.2, energy: 3.6 });
        assert_eq!(l.totals().violations, 0);
        let r = l.members[&p].reproduction;
        assert_eq!(r.refunds, 1);
        assert!((r.refund_reserve - 1.2).abs() < 1e-15);
        assert!((r.refund_energy - 0.6).abs() < 1e-15);
    }

    #[test]
    fn potential_and_actual_intake_never_merge() {
        let mut l = DevFlowLedger::new(0);
        let a = id(0, 1);
        open(&mut l, a, 0, Stocks { structure: 1.0, reserve: 1.0, energy: 1.0 });
        l.record_intake_request(a, 3.0, 0.0, 0.5, 0.25);
        // Half the cell's grazing request is served; scavenging is served in full.
        l.record_intake_settlement(a, FeedingChannel::Grazing, 0.5, 0.25, 0.2, 0.05, 0.0);
        l.record_intake_settlement(a, FeedingChannel::Scavenging, 1.0, 0.25, 0.2, 0.05, 0.0);
        l.record_intake_close(a, true, true);
        l.close_tick(a, 0, Stocks { structure: 1.0, reserve: 1.4, energy: 1.1 });
        let i = l.members[&a].intake;
        assert_eq!(i.frugivory.request_ticks, 0);
        assert!((i.grazing.requested_amount - 0.5).abs() < 1e-15);
        assert!((i.grazing.actual_amount - 0.25).abs() < 1e-15);
        assert!((i.scavenging.requested_amount - 0.25).abs() < 1e-15);
        assert!((i.scavenging.actual_amount - 0.25).abs() < 1e-15);
        assert_eq!((i.ticks_any_actual_intake, i.contested_ticks), (1, 1));
        assert_eq!(i.headroom_limited_ticks, 0);
        assert_eq!(l.totals().violations, 0);
    }

    #[test]
    fn an_unregistered_record_is_counted_not_silently_dropped() {
        let mut l = DevFlowLedger::new(0);
        l.record_oxidation(id(3, 1), true, true, 0.1, 0.1, 0.0);
        assert_eq!(l.unregistered_records, 1);
        assert!(l.members.is_empty());
    }

    #[test]
    fn juvenile_and_adult_ticks_split_on_the_structure_the_tick_started_with() {
        let mut l = DevFlowLedger::new(0);
        let a = id(0, 1);
        open(&mut l, a, 0, Stocks { structure: 1.0, reserve: 1.0, energy: 1.0 });
        l.close_tick(a, 0, Stocks { structure: 2.0, reserve: 1.0, energy: 1.0 });
        open(&mut l, a, 1, Stocks { structure: 2.0, reserve: 1.0, energy: 1.0 });
        l.close_tick(a, 1, Stocks { structure: 2.0, reserve: 1.0, energy: 1.0 });
        let s = l.members[&a].structure;
        assert_eq!((s.ticks_juvenile, s.ticks_adult), (1, 1));
        assert_eq!(s.adult_recruitment_tick, Some(0));
        assert!((s.at_birth - 1.0).abs() < 1e-15);
        assert!((s.final_structure - 2.0).abs() < 1e-15);
    }
}
