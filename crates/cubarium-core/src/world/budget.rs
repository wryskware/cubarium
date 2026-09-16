//! The **per-organism store budget**: every credit into and every debit out of one body's
//! three stores, accumulated from the moment the body was first seen to the moment it died.
//!
//! [`crate::world::IntakeDiagnostics`] answers "what did this *world's* mouths take and what
//! did its bodies owe". It cannot answer "could *this* body have paid for itself", because it
//! is a world total with no owner. This module is the per-body ledger that question needs
//! (`design/7_Research/ecology-v1-next-review-2026-09-15.md`, finding 1 and next step 1).
//!
//! **What a channel records.** Ecology v1 §6.4 turns one served bite `q` on a food of density
//! `ρ` into: `q_d = cap·q` digestible, `η_m′·q_d` into the reserve, `min(η_e·(ρ·q_d −
//! e_r·η_m′·q_d), E_max − E)` into the battery, and feces and heat for the rest. All four of
//! those are recorded per channel, at the site that performs them, from the values the world
//! actually used — never reconstructed and never divided by anything.
//!
//! **What the two identities are.** Recording every write to `S`, `R` and `E` makes the ledger
//! closed, so it can be checked rather than believed:
//!
//! ```text
//! material:  Σ reserve_credit + gut_reserve_credit
//!              − oxidation_reserve_burned − reproduction_material − injury_structure
//!            = Δ(structure + reserve)
//!
//! energy:    Σ battery_credit + gut_battery_credit + oxidation_battery_credit
//!              − bill_paid − other_energy_paid − growth_energy − reproduction_energy
//!            = Δ(energy)
//! ```
//!
//! Growth moves `growth_material` from `R` to `S` and so cancels inside `Δ(S + R)`; it is
//! recorded anyway because "what did it spend on its own body" is a question the campaign
//! asks. [`BodyBudget::material_residual`] and [`BodyBudget::energy_residual`] are those two
//! lines rearranged to zero.
//!
//! **Prices are not payments.** `upkeep_billed`, `motor_translation_billed` and
//! `motor_turn_billed` are the three terms of `MotorBill::total_cost`, split out at the site
//! that levies them. They are what the body *owed*; `bill_paid` is what it could actually
//! raise, and only `bill_paid` appears in the energy identity. The three billed terms sum to
//! `bill_total` to floating-point association only, exactly as `MotorBill` documents, so a
//! reader must not difference them against each other.
//!
//! **Cost and scope.** Recording is **off** by default and is turned on per `World` by
//! [`crate::World::record_body_budgets`]. When it is off, every site here is one `bool` test
//! and nothing is allocated. It is off by default for two reasons that have nothing to do
//! with the arithmetic: a long-lived display world would otherwise accumulate one closed
//! record per death forever, and the trainer pays for throughput it does not use. A recorder
//! that is never drained keeps at most [`MAX_CLOSED_RECORDS`] closed records and counts what
//! it dropped, so even a leaked recorder is bounded.
//!
//! **Transient.** Like every other diagnostic on `World`, these records are never persisted,
//! never hashed, never read back by the tick and zero again after a reload. The snapshot
//! schema is unchanged: [`BodyBudget`] appears nowhere in [`crate::world::WorldState`].

use serde::{Deserialize, Serialize};

use crate::ids::OrganismId;
use crate::organism::{DeathCause, Organism};

/// How many food channels a body's intake is split into.
pub const CHANNELS: usize = 4;

/// Channel index: leaf material out of `P`, digested at `cap_foliage`.
pub const FOLIAGE: usize = 0;
/// Channel index: fruit out of `F`, digested at `cap_foliage`.
pub const FRUIT: usize = 1;
/// Channel index: litter out of `D`, digested at `cap_detrital · min(1, ρ_D/e_r)`.
pub const LITTER: usize = 2;
/// Channel index: remains out of `C`, digested at `cap_detrital · min(1, ρ_C/e_r)`.
pub const CARRION: usize = 3;

/// The channel names, in index order, for a report that prints them.
pub const CHANNEL_NAMES: [&str; CHANNELS] = ["foliage", "fruit", "litter", "carrion"];

/// The largest number of closed records a recorder holds before it starts dropping them.
pub const MAX_CLOSED_RECORDS: usize = 4_096;

/// One body's complete store budget, from the first tick it was recorded to now.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BodyBudget {
    pub id: OrganismId,
    /// The tick this record opened: the body's birth when recording was already on, otherwise
    /// the first tick after recording was switched on. Every figure below is from that tick.
    pub opened_tick: u64,
    /// The body's own `born_tick`, so a record opened later says how much of the life it
    /// covers.
    pub born_tick: u64,
    /// The tick the body was removed, and why. `None` while it is alive.
    pub closed_tick: Option<u64>,
    pub death_cause: Option<DeathCause>,

    /// Stores at `opened_tick`, before the tick's first mutation.
    pub start_structure: f64,
    pub start_reserve: f64,
    pub start_energy: f64,
    /// Stores now, or at removal for a closed record.
    pub end_structure: f64,
    pub end_reserve: f64,
    pub end_energy: f64,

    /// Served material `q` that actually left the field, per channel (m).
    pub served: [f64; CHANNELS],
    /// `q_d = cap · q`, the portion this body's machinery could work on (m).
    pub digestible: [f64; CHANNELS],
    /// `η_m′ · q_d`, the material that entered the reserve (m).
    pub reserve_credit: [f64; CHANNELS],
    /// The battery charge the same bite delivered directly, after `η_e` and the headroom clamp
    /// (e). What the clamp rejected went to heat and is not here.
    pub battery_credit: [f64; CHANNELS],

    /// Reserve and battery credited by digesting gut contents. A hunter member only; ecology
    /// v1 has no predation channel for an ordinary body.
    pub gut_reserve_credit: f64,
    pub gut_battery_credit: f64,

    /// Reserve material oxidised to `N` (m), and the battery charge that released (e), summed
    /// over both passes that can oxidise: the shortfall burn inside the motor settlement and
    /// the ordinary physiology transaction.
    pub oxidation_reserve_burned: f64,
    pub oxidation_battery_credit: f64,

    /// The three terms of the tick's `MotorBill::total_cost`, at the site that levies it (e).
    /// Prices, not payments; they sum to `bill_total` only to floating-point association.
    pub upkeep_billed: f64,
    pub motor_translation_billed: f64,
    pub motor_turn_billed: f64,
    /// `MotorBill::total_cost` itself, the one number the world books.
    pub bill_total: f64,
    /// What the body actually raised against that bill (e): stored energy plus whatever the
    /// shortfall burn converted. Less than `bill_total` only on a tick the body dies.
    pub bill_paid: f64,
    /// Energy paid outside the motor bill (e): an apex member's strike and retreat costs and
    /// its meal handling. Zero for every ordinary body.
    pub other_energy_paid: f64,

    /// Reserve turned into structure (m) and the `build_cost` energy that paid for it (e).
    pub growth_material: f64,
    pub growth_energy: f64,
    /// Material escrowed into a gestation (m) and the energy paid with it (e), for both the
    /// one-parent bud and an apex pair's half contribution.
    pub reproduction_material: f64,
    pub reproduction_energy: f64,
    /// Structure lost to apex combat injury (m). Zero for every ordinary body.
    pub injury_structure: f64,

    /// Ticks on which this body was billed by the motor settlement.
    pub billed_ticks: u64,
}

impl BodyBudget {
    fn open(id: OrganismId, o: &Organism, tick: u64) -> BodyBudget {
        BodyBudget {
            id,
            opened_tick: tick,
            born_tick: o.born_tick,
            closed_tick: None,
            death_cause: None,
            start_structure: o.structure,
            start_reserve: o.reserve,
            start_energy: o.energy,
            end_structure: o.structure,
            end_reserve: o.reserve,
            end_energy: o.energy,
            served: [0.0; CHANNELS],
            digestible: [0.0; CHANNELS],
            reserve_credit: [0.0; CHANNELS],
            battery_credit: [0.0; CHANNELS],
            gut_reserve_credit: 0.0,
            gut_battery_credit: 0.0,
            oxidation_reserve_burned: 0.0,
            oxidation_battery_credit: 0.0,
            upkeep_billed: 0.0,
            motor_translation_billed: 0.0,
            motor_turn_billed: 0.0,
            bill_total: 0.0,
            bill_paid: 0.0,
            other_energy_paid: 0.0,
            growth_material: 0.0,
            growth_energy: 0.0,
            reproduction_material: 0.0,
            reproduction_energy: 0.0,
            injury_structure: 0.0,
            billed_ticks: 0,
        }
    }

    /// Total served material over every channel (m).
    pub fn served_total(&self) -> f64 {
        self.served.iter().sum()
    }

    /// Total material that entered the reserve from food (m).
    pub fn reserve_credit_total(&self) -> f64 {
        self.reserve_credit.iter().sum()
    }

    /// Total battery charge delivered directly by food (e).
    pub fn battery_credit_total(&self) -> f64 {
        self.battery_credit.iter().sum()
    }

    /// `Σ credits − Σ debits − Δ(structure + reserve)`: zero for a correctly recorded life.
    pub fn material_residual(&self) -> f64 {
        let credits = self.reserve_credit_total() + self.gut_reserve_credit;
        let debits = self.oxidation_reserve_burned
            + self.reproduction_material
            + self.injury_structure;
        let delta = (self.end_structure + self.end_reserve)
            - (self.start_structure + self.start_reserve);
        credits - debits - delta
    }

    /// `Σ credits − Σ debits − Δ(energy)`: zero for a correctly recorded life.
    pub fn energy_residual(&self) -> f64 {
        let credits =
            self.battery_credit_total() + self.gut_battery_credit + self.oxidation_battery_credit;
        let debits = self.bill_paid
            + self.other_energy_paid
            + self.growth_energy
            + self.reproduction_energy;
        let delta = self.end_energy - self.start_energy;
        credits - debits - delta
    }

    /// Usable stores `E + e_r·R` at the record's start and end, the same combination the ES
    /// fixture scores.
    pub fn usable_start(&self, reserve_energy_density: f64) -> f64 {
        self.start_energy + reserve_energy_density * self.start_reserve
    }

    pub fn usable_end(&self, reserve_energy_density: f64) -> f64 {
        self.end_energy + reserve_energy_density * self.end_reserve
    }
}

/// The per-`World` store of [`BodyBudget`] records. Transient; never persisted or hashed.
#[derive(Clone, Debug, Default)]
pub struct BudgetRecorder {
    on: bool,
    /// Indexed by organism slot. A slot's record is replaced when the slot is reused.
    live: Vec<Option<BodyBudget>>,
    closed: Vec<BodyBudget>,
    /// Closed records discarded because nobody drained them. Never silently zero.
    dropped: u64,
}

impl BudgetRecorder {
    /// Whether this world records per-body budgets at all.
    #[inline]
    pub fn enabled(&self) -> bool {
        self.on
    }

    /// Turn recording on or off. Turning it on starts every currently living body's record at
    /// the next tick's opening stores; turning it off discards every open and closed record,
    /// so a reader never sees a ledger with a hole in the middle of it.
    pub(crate) fn set_enabled(&mut self, on: bool) {
        if self.on == on {
            return;
        }
        self.on = on;
        self.live.clear();
        self.closed.clear();
        self.dropped = 0;
    }

    /// The record for one live body, if recording is on and the id still resolves.
    pub fn get(&self, id: OrganismId) -> Option<&BodyBudget> {
        if !self.on {
            return None;
        }
        self.live
            .get(id.slot as usize)
            .and_then(|r| r.as_ref())
            .filter(|b| b.id == id)
    }

    /// The closed records, oldest first, and how many were dropped because nobody drained.
    pub fn closed(&self) -> (&[BodyBudget], u64) {
        (&self.closed, self.dropped)
    }

    /// Take the closed records and reset the dropped counter.
    pub(crate) fn drain_closed(&mut self) -> (Vec<BodyBudget>, u64) {
        let dropped = std::mem::take(&mut self.dropped);
        (std::mem::take(&mut self.closed), dropped)
    }

    /// Open a record for a body that has none, or replace a stale one whose slot was reused.
    /// Called once per live body per tick, before the tick mutates anything.
    #[inline]
    pub(crate) fn ensure(&mut self, id: OrganismId, o: &Organism, tick: u64) {
        if !self.on {
            return;
        }
        let slot = id.slot as usize;
        if self.live.len() <= slot {
            self.live.resize(slot + 1, None);
        }
        let entry = &mut self.live[slot];
        match entry {
            Some(b) if b.id == id => {}
            _ => *entry = Some(BodyBudget::open(id, o, tick)),
        }
    }

    /// The mutable record for one live body. `None` when recording is off, which is the one
    /// branch every instrumented site takes in an ordinary world.
    #[inline]
    pub(crate) fn at(&mut self, id: OrganismId) -> Option<&mut BodyBudget> {
        if !self.on {
            return None;
        }
        self.live
            .get_mut(id.slot as usize)
            .and_then(|r| r.as_mut())
            .filter(|b| b.id == id)
    }

    /// Bring one live record's `end_*` stores up to date. Called once per body per tick, after
    /// the tick's mutations, so a reader mid-run sees a closed identity rather than a stale
    /// one.
    #[inline]
    pub(crate) fn mark(&mut self, id: OrganismId, o: &Organism) {
        if let Some(b) = self.at(id) {
            b.end_structure = o.structure;
            b.end_reserve = o.reserve;
            b.end_energy = o.energy;
        }
    }

    /// Close a record: the body has been removed. Its terminal stores are the ones it held at
    /// removal, and the cause is the one the world booked.
    pub(crate) fn close(&mut self, id: OrganismId, o: &Organism, tick: u64, cause: DeathCause) {
        if !self.on {
            return;
        }
        let Some(entry) = self.live.get_mut(id.slot as usize) else {
            return;
        };
        let Some(mut b) = entry.take_if(|b| b.id == id) else {
            return;
        };
        b.end_structure = o.structure;
        b.end_reserve = o.reserve;
        b.end_energy = o.energy;
        b.closed_tick = Some(tick);
        b.death_cause = Some(cause);
        if self.closed.len() >= MAX_CLOSED_RECORDS {
            self.dropped += 1;
            return;
        }
        self.closed.push(b);
    }
}

impl super::World {
    /// Turn per-organism budget recording on or off for this world.
    ///
    /// Off is the default and is what every ordinary path — the display host, the trainer, the
    /// calibration runner — leaves it at. Turning it on opens a record for every living body
    /// at the next tick, with that tick's opening stores as the start; turning it off discards
    /// every record. The flag changes no dynamics: it is read only at sites that add to a
    /// counter, consumes no draw, and moves no value the simulation reads.
    pub fn record_body_budgets(&mut self, on: bool) {
        self.budgets.set_enabled(on);
    }

    /// Whether this world is recording per-organism budgets.
    pub fn records_body_budgets(&self) -> bool {
        self.budgets.enabled()
    }

    /// One living body's budget so far, or `None` when recording is off or the id is stale.
    pub fn body_budget(&self, id: crate::ids::OrganismId) -> Option<&BodyBudget> {
        self.budgets.get(id)
    }

    /// The records of every body that died since the last drain, oldest first, and how many
    /// were dropped because more than [`MAX_CLOSED_RECORDS`] accumulated undrained.
    pub fn drain_body_budgets(&mut self) -> (Vec<BodyBudget>, u64) {
        self.budgets.drain_closed()
    }

    /// The apex mating-opportunity counters so far
    /// (`crate::encounter::ApexOpportunity`).
    pub fn apex_opportunity(&self) -> crate::encounter::ApexOpportunity {
        self.apex_opportunity
    }

    /// The apex mating-opportunity counters, reset to zero.
    pub fn drain_apex_opportunity(&mut self) -> crate::encounter::ApexOpportunity {
        std::mem::take(&mut self.apex_opportunity)
    }
}
