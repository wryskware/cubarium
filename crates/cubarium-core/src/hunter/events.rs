use serde::Serialize;

use cubarium_surface::{SurfacePoint, Vec2};

use crate::ids::OrganismId;
use crate::organism::DeathCause;

use super::*;

/// What [`crate::World::start_hunter_trial`] actually did: the full founder ID, the inventory
/// derived from this world's own config, the amounts booked, and the jaw geometry the art
/// worker has to confirm.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct HunterFounderReceipt {
    pub id: OrganismId,
    pub tick: u64,
    pub pos: SurfacePoint,
    pub structure: f64,
    pub reserve: f64,
    pub energy: f64,
    /// `S + R`, booked in [`HunterState::founder_material_in`].
    pub material_in: f64,
    /// `E + e_r · R`, booked in [`HunterState::founder_energy_in`].
    pub energy_in: f64,
    pub extent: f64,
    pub sense_radius: f64,
    /// The founder's contact geometry at its own scale: what the art adapter must draw and
    /// what the world will test contact against.
    pub geometry: ContactGeometry,
}

/// What [`crate::World::deposit_hunter_budget_control`] actually did: the same derived
/// inventory, placed as local detritus instead of a hunter.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct HunterControlReceipt {
    pub tick: u64,
    pub cell: u16,
    /// The founder-equivalent material added to `D`.
    pub material_in: f64,
    /// The founder-equivalent energy admitted; `energy_stored` of it fits the detritus cap
    /// and `energy_heat` left as heat immediately.
    pub energy_in: f64,
    pub energy_stored: f64,
    pub energy_heat: f64,
}

/// Why a paid attempt ended the way it did. `Unaffordable` is the one refusal that happens
/// *before* payment: it consumes no energy, no draw and no attack counter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum AttemptOutcome {
    Captured,
    /// In contact, paid, and the capture roll failed.
    Missed,
    /// The jaw was not in reach of the target after both creatures moved.
    OutOfReach,
    /// The target died or its slot was reused before the attempt resolved.
    TargetLost,
    /// Another hunter's claim on the same prey won.
    TargetClaimed,
    /// The target no longer fits the eligibility window or the remaining gut.
    Ineligible,
    /// The grasp centre was off the surface or could not be mapped consistently — an off-rim
    /// reach the static artwork clips, or a vertex whose images disagree. The strike is still
    /// paid for; no capture is made from a point the renderer cannot draw.
    GraspUnmapped,
    /// Refused before payment: the full strike cost was not available.
    Unaffordable,
}

impl AttemptOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            AttemptOutcome::Captured => "captured",
            AttemptOutcome::Missed => "missed",
            AttemptOutcome::OutOfReach => "out_of_reach",
            AttemptOutcome::TargetLost => "target_lost",
            AttemptOutcome::TargetClaimed => "target_claimed",
            AttemptOutcome::Ineligible => "ineligible",
            AttemptOutcome::GraspUnmapped => "grasp_unmapped",
            AttemptOutcome::Unaffordable => "unaffordable",
        }
    }
}

/// The stable identity of one funded gestation: the parent's full ID and the tick its escrow
/// was started, which the world already persists in `Escrow::started_tick`.
///
/// A parent holds at most one escrow at a time, so this names exactly one transaction from its
/// funding to whichever way it ends. It is not a new counter and nothing new is persisted to
/// carry it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct EscrowKey {
    pub parent: OrganismId,
    pub started_tick: u64,
}

/// Why a member that wanted to fund an offspring never got an escrow at all. This is a
/// **non-transaction**: nothing moved, and there is no [`EscrowKey`] to name.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FundingBlocked {
    /// The world was at its organism cap when the gestation would have started.
    Cap,
    /// The parent's own reserve or usable energy could not cover the child's inventory and its
    /// build cost.
    Stocks,
}

/// One reproduction transaction, recorded **at the mutation itself**: the numbers below are the
/// values the world actually moved, read on either side of the assignment that moved them, not
/// a post-step difference an observer could have taken for itself.
///
/// A gestation is funded once and ends exactly once — as a birth, a refund at the cap, or an
/// export to the litter when its parent dies. All four name the same [`EscrowKey`], so a
/// reader can close every transaction it opens. A funding and its loss can both happen in the
/// same tick, and both are emitted.
///
/// Material and energy identities the records satisfy exactly, by construction:
///
/// ```text
/// Funded:      reserve_before − reserve_after = escrow_structure + escrow_reserve
///              (e_r·reserve + energy)_before − (e_r·reserve + energy)_after
///                  = e_r·(escrow_structure + escrow_reserve) + escrow_energy + build_heat
/// Born:        child_structure/reserve/energy = the escrow's own S/R/E
///              birth_heat = e_r · escrow_structure      (structure holds no chemical energy)
/// Refunded:    reserve_after − reserve_before = refunded_structure + refunded_reserve
///              energy_after − energy_before  = refunded_energy      (no heat: nothing burned)
/// Miscarried:  material = escrow S + R;  energy = e_r·material + escrow E
///              energy_stored + energy_heat = energy;  energy_stored ≤ energy_cap · material
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "transaction", rename_all = "snake_case")]
pub enum Reproduction {
    /// The parent paid for a gestation: reserve and usable energy debited, the build cost
    /// released as heat, and an escrow now exists.
    Funded {
        key: EscrowKey,
        /// The parent's stocks immediately before and after the debit.
        parent_reserve_before: f64,
        parent_reserve_after: f64,
        parent_energy_before: f64,
        parent_energy_after: f64,
        /// What the escrow holds.
        escrow_structure: f64,
        escrow_reserve: f64,
        escrow_energy: f64,
        /// `build_cost · escrow_structure`, heated at funding time.
        build_heat: f64,
    },
    /// The escrow became a child. Emitted with — and reconciling one-for-one against — the
    /// [`HunterEvent::Offspring`] record and the ordinary `LifeEvent::Birth` of the same tick.
    Born {
        key: EscrowKey,
        child: OrganismId,
        /// The child's actual initial inventory, which is the escrow's.
        child_structure: f64,
        child_reserve: f64,
        child_energy: f64,
        /// `e_r · escrow_structure`: the reserve energy the structural material gives up when
        /// it becomes structure.
        birth_heat: f64,
    },
    /// A due birth the organism cap refused. The escrow went back to the parent exactly as it
    /// left: no heat, no discard.
    Refunded {
        key: EscrowKey,
        refunded_structure: f64,
        refunded_reserve: f64,
        refunded_energy: f64,
        parent_reserve_before: f64,
        parent_reserve_after: f64,
        parent_energy_before: f64,
        parent_energy_after: f64,
    },
    /// The parent died holding it. The escrow's own material and energy are exported to the
    /// cell's litter under the detritus cap — **only** the escrow: the body and any carried gut
    /// are separate terms of the same death, reported separately.
    Miscarried {
        key: EscrowKey,
        cause: DeathCause,
        /// `escrow S + R` added to `D`.
        material: f64,
        /// `e_r · material + escrow E` removed with it, of which …
        energy: f64,
        /// … the cap let `De` keep this much …
        energy_stored: f64,
        /// … and this much left as heat.
        energy_heat: f64,
    },
    /// A member that was ready to fund and did not: **no escrow was ever created**, nothing
    /// moved, and no later record closes this one.
    NotFunded {
        parent: OrganismId,
        reason: FundingBlocked,
    },
}

impl Reproduction {
    /// The gestation this record belongs to, or `None` for a non-transaction.
    pub fn key(&self) -> Option<EscrowKey> {
        match self {
            Reproduction::Funded { key, .. }
            | Reproduction::Born { key, .. }
            | Reproduction::Refunded { key, .. }
            | Reproduction::Miscarried { key, .. } => Some(*key),
            Reproduction::NotFunded { .. } => None,
        }
    }

    /// The member the record is about, transaction or not.
    pub fn parent(&self) -> OrganismId {
        match self {
            Reproduction::NotFunded { parent, .. } => *parent,
            other => {
                other
                    .key()
                    .expect("every transaction names its parent")
                    .parent
            }
        }
    }
}

/// Transient hunter records, drained by the observer like [`crate::LifeEvent`]. Never
/// checkpointed, never hashed, never read back by the tick. Each consumed prey also produces
/// exactly one ordinary `LifeEvent::Death` with [`DeathCause::Predation`].
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HunterEvent {
    /// One resolved attempt, paid or refused.
    Attempt {
        tick: u64,
        hunter: OrganismId,
        target: Option<OrganismId>,
        outcome: AttemptOutcome,
        /// Strike cost actually charged (zero for `Unaffordable`).
        energy_paid: f64,
        /// The paid attempt's stable key: the member's `attack_counter` **after** the
        /// increment that opened this attempt, unique with the full hunter ID. `None` for an
        /// unpaid refusal, which consumes no counter and can never alias a paid attempt.
        attack_counter: Option<u64>,
        /// The post-movement contact evidence, when the target still resolved. A missing or
        /// stale target has none; it is never filled in from a reused slot.
        evidence: Option<ContactEvidence>,
    },
    /// A capture, with the material and energy actually transferred into the gut.
    Capture {
        tick: u64,
        hunter: OrganismId,
        prey: OrganismId,
        material: f64,
        energy: f64,
        /// The same key as this attempt's `Attempt` record.
        attack_counter: u64,
        /// The pre-removal settlement geometry: where the prey was, where the hunter was, and
        /// the grasp that took it.
        evidence: ContactEvidence,
    },
    /// A funded descendant was placed.
    Offspring {
        tick: u64,
        parent: OrganismId,
        child: OrganismId,
    },
    /// One reproduction transaction, recorded at the mutation that moved it
    /// ([`Reproduction`]). A birth emits this **and** the [`HunterEvent::Offspring`] record
    /// above, so the identity link and the transaction reconcile one-for-one.
    Reproduction {
        tick: u64,
        hunter: OrganismId,
        record: Reproduction,
    },
    /// A member died; its carried gut went to the local fields.
    Death {
        tick: u64,
        id: OrganismId,
        cause: DeathCause,
        gut_material: f64,
        gut_energy: f64,
        /// Energy the detritus cap allowed the gut to keep; the rest became heat.
        gut_energy_stored: f64,
    },
}

impl HunterEvent {
    /// The completed-tick boundary this record belongs to, so an observer can order or bucket
    /// events without repeating the variant list.
    pub fn tick(&self) -> u64 {
        match self {
            HunterEvent::Attempt { tick, .. }
            | HunterEvent::Capture { tick, .. }
            | HunterEvent::Offspring { tick, .. }
            | HunterEvent::Reproduction { tick, .. }
            | HunterEvent::Death { tick, .. } => *tick,
        }
    }

    /// The member the record is about: the attacker, the captor, the parent, or the deceased.
    pub fn hunter(&self) -> OrganismId {
        match self {
            HunterEvent::Attempt { hunter, .. }
            | HunterEvent::Capture { hunter, .. }
            | HunterEvent::Reproduction { hunter, .. } => *hunter,
            HunterEvent::Offspring { parent, .. } => *parent,
            HunterEvent::Death { id, .. } => *id,
        }
    }
}

/// One hunter as the renderer sees it, keyed by full ID. Transient and published separately
/// from `RenderView` so the existing view structs keep their fields while the art package
/// lands (`fixed-hunter-core-handoff-2026-09-13.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HunterView {
    pub id: OrganismId,
    /// The art direction's semantic role, not `genome.form`.
    pub role: HunterRole,
    pub phase: HunterPhase,
    /// Progress through a timed phase; `None` while perched, stalking or handling.
    ///
    /// A convenience only. Fractional playback belongs to `phase_started_tick` /
    /// `phase_ends_tick` and the presenter's own `present_seconds`.
    pub phase_progress: Option<f32>,
    /// The persisted phase boundary, in completed ticks: the phase was entered at the boundary
    /// `phase_started_tick` (the world had exactly that many completed ticks) and a timed
    /// phase ends at `phase_ends_tick`. Untimed phases store `ends == started`.
    pub phase_started_tick: u64,
    pub phase_ends_tick: u64,
    /// The phase this one was entered from, persisted, so a restart during the recoil knows
    /// whether it followed a fully extended strike, an aborted windup or a finished meal.
    pub entered_from: HunterPhase,
    /// The attack episode this phase belongs to: the `attack_counter` of the paid attempt that
    /// produced it, or 0 when the phase belongs to no paid attempt.
    pub episode: u64,
    /// The member's paid-attempt counter, the key its events carry.
    pub attack_counter: u64,
    pub pos: SurfacePoint,
    pub heading: Vec2,
    /// The authoritative scaled contact geometry — the same numbers the world tests contact
    /// with, and the scale the whole rig must be drawn at.
    pub geometry: ContactGeometry,
    /// `geometry.scale`, published on its own because the rig needs exactly this number.
    pub body_scale: f64,
    /// The physical grasp centre, **`None` when it cannot be drawn there** (off the open rim,
    /// a vertex whose images disagree, a fallback sweep). Never a reflected point.
    pub capture_center: Option<SurfacePoint>,
    /// The physical ingestion mouth, under the same rule and named separately.
    pub ingestion_center: Option<SurfacePoint>,
    /// The prey being pursued, when the handle still resolves.
    pub target: Option<OrganismId>,
    pub structure: f64,
    pub structure_adult: f64,
    pub extent: f64,
    pub juvenile: bool,
    pub gut_material: f64,
    pub gut_energy: f64,
    /// `gut_material / gut_capacity_material`.
    pub gut_fraction: f32,
    pub gut_capacity: f64,
    /// Gestation progress while an escrow is held, on the hunter's own gestation time.
    pub gestation: Option<f32>,
}
