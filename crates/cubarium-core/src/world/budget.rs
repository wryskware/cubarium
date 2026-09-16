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

// --- the per-tick intake trace (workstream H) ---------------------------------------------
//
// `BodyBudget` answers "what did this body take in over its life". It cannot answer "why did
// it take in so little", because a life total cannot tell an unfed mouth from a shut one.
// The trace below is that question, for **one** body, one row per tick, recorded at the same
// sites the ledger uses (`design/handoffs/ecology-v1-intake-opus-2026-09-16.md`, deliverable
// 1).

/// How many mouth channels one decision carries. The three efforts of
/// `crate::controller::Decision`, which contract §6.3 shares one mouth between.
pub const MOUTHS: usize = 3;
/// Mouth index: `graze_effort`, which bites the foliage stock `P`.
pub const MOUTH_GRAZE: usize = 0;
/// Mouth index: `fruit_effort`, which bites the fruit stock `F`.
pub const MOUTH_FRUIT: usize = 1;
/// Mouth index: `scavenge_effort`, which bites `D_eff + C_eff` as one food and is served out
/// of the two stocks in proportion to their edible shares (§6.2, §6.4).
pub const MOUTH_SCAVENGE: usize = 2;
/// The mouth names, in index order.
pub const MOUTH_NAMES: [&str; MOUTHS] = ["graze", "fruit", "scavenge"];

/// The largest number of trace rows a recorder holds before it starts dropping them. A
/// consumer that drains every tick never reaches it; one that forgets is still bounded.
pub const MAX_TRACE_ROWS: usize = 4_096;

/// What decided one mouth's bite this tick, at the site that decided it.
///
/// The terms are evaluated in the order below and the **first** that applies is the one
/// recorded, because each earlier term makes every later one vacuous: a body with no gut for a
/// food never asks, a mouth that is shut cannot be starved of stock, and a cell the world's own
/// `feed_min` calls bare cannot clamp anything worth naming.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntakeLimit {
    /// The phenotype has no machinery for this food (`cap_foliage` or `cap_detrital` is zero),
    /// so the world refuses the channel whatever asked for it (§6.2).
    CapabilityZero,
    /// The controller asked for nothing: the decoded effort is zero. For a neural body this is
    /// the action adapter's deadband or its capability mask; for a scripted one it is the
    /// script.
    EffortZero,
    /// The mouth was open and the cell was bare: the stock this mouth serves is below the
    /// world's own `drives.feed_min`, the threshold its legacy controller and the disclosed
    /// mobile script both use to call a cell not worth cropping.
    StockBelowThreshold,
    /// The reserve had no room left: `R_max − R` (less whatever an earlier-settling mouth
    /// already took of it) clamped the bite below what the mouth rate would have taken.
    ReserveHeadroom,
    /// The cell could not serve what was asked: the proportional share of a contested or
    /// nearly empty stock delivered less than the request.
    StockShare,
    /// Nothing else bound it: the bite is exactly `mouth_rate · effort · dt · S(food)`, the
    /// type-II mouth rate on this cell's stock. `saturation` says how much of that ceiling the
    /// cell's own thinness cost.
    MouthRate,
}

/// One tick of one body's intake, from the cell it stood on to the credit it banked.
///
/// Every number is read from the values the world used, in the pass that used them; nothing is
/// reconstructed afterwards. The row is opened before the tick's first mutation, filled by the
/// motor settlement (§7) and the feeding settlement (§6.4), and closed after the tick commits.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntakeTick {
    /// The world tick this row covers.
    pub tick: u64,
    /// Whether the feeding settlement saw this body at all. False only for a tick the traced
    /// body was not there to feed; such a row is never kept.
    pub observed: bool,
    /// The cell the body occupied at the feeding settlement, i.e. **after** this tick's move.
    pub cell: u16,
    /// The four stocks' edible portions in that cell, before a single transfer:
    /// `P`, `F`, `D_eff`, `C_eff` (§3.1, §6.2).
    pub stock: [f64; CHANNELS],
    /// Whether each stock is at or above `drives.feed_min`.
    pub above_threshold: [bool; CHANNELS],
    /// The threshold the flags above were taken against, carried so a row reads alone.
    pub feed_threshold: f64,
    /// The three efforts as the controller's `Decision` carried them into the tick.
    pub effort: [f64; MOUTHS],
    /// The same three after the world's one-mouth normalisation (`/Σ` when `Σ > 1`, §6.3).
    /// Equal to `effort` for a neural body, whose adapter already normalised them.
    pub effort_normalised: [f64; MOUTHS],
    /// The raw linear head **before** squashing, deadband, masking and normalisation, for a
    /// neural body on a tick its controller actually ran. `None` on a tick that reused the
    /// held action, and for every scripted or legacy body.
    pub raw_head: Option<[f64; crate::neural::action::ACT_LEN]>,
    /// The type-II factor `food/(food + K_P)` each mouth saw on its own food.
    pub saturation: [f64; MOUTHS],
    /// `mouth_rate · effort_normalised · dt · saturation`: what the mouth would have taken
    /// with unlimited room.
    pub mouth_bite: [f64; MOUTHS],
    /// The same after the reserve-headroom clamp: the bite the cell was actually asked for.
    pub requested: [f64; MOUTHS],
    /// What decided each mouth's bite.
    pub limit: [IntakeLimit; MOUTHS],
    /// `q`: what actually left each stock (m).
    pub served: [f64; CHANNELS],
    /// `q_d = cap · q` (m).
    pub digestible: [f64; CHANNELS],
    /// `η_m′ · q_d` into the reserve (m).
    pub reserve_credit: [f64; CHANNELS],
    /// The charge the same bite delivered into the battery, after `η_e` and the `E_max` clamp.
    pub battery_credit: [f64; CHANNELS],
    /// The charge the `E_max` clamp sent to heat instead of the battery (e). Non-zero only
    /// when the battery was already full, which is a limit on the *credit*, never on the bite.
    pub battery_rejected: [f64; CHANNELS],
    /// `R_max − R` and `E_max − E` as the feeding settlement found them.
    pub reserve_headroom: f64,
    pub energy_headroom: f64,
    /// The stores the feeding settlement started from.
    pub reserve: f64,
    pub energy: f64,
    /// This tick's `MotorBill::total_cost` and what the body raised against it (e).
    pub bill_total: f64,
    pub bill_paid: f64,
}

impl IntakeTick {
    fn open(tick: u64) -> IntakeTick {
        IntakeTick {
            tick,
            observed: false,
            cell: 0,
            stock: [0.0; CHANNELS],
            above_threshold: [false; CHANNELS],
            feed_threshold: 0.0,
            effort: [0.0; MOUTHS],
            effort_normalised: [0.0; MOUTHS],
            raw_head: None,
            saturation: [0.0; MOUTHS],
            mouth_bite: [0.0; MOUTHS],
            requested: [0.0; MOUTHS],
            limit: [IntakeLimit::EffortZero; MOUTHS],
            served: [0.0; CHANNELS],
            digestible: [0.0; CHANNELS],
            reserve_credit: [0.0; CHANNELS],
            battery_credit: [0.0; CHANNELS],
            battery_rejected: [0.0; CHANNELS],
            reserve_headroom: 0.0,
            energy_headroom: 0.0,
            reserve: 0.0,
            energy: 0.0,
            bill_total: 0.0,
            bill_paid: 0.0,
        }
    }

    /// Which mouth serves a stock channel.
    pub fn mouth_of(channel: usize) -> usize {
        match channel {
            FOLIAGE => MOUTH_GRAZE,
            FRUIT => MOUTH_FRUIT,
            _ => MOUTH_SCAVENGE,
        }
    }

    /// The bite one mouth was actually served, summed over the stocks it draws from.
    pub fn served_by(&self, mouth: usize) -> f64 {
        (0..CHANNELS)
            .filter(|c| IntakeTick::mouth_of(*c) == mouth)
            .map(|c| self.served[c])
            .sum()
    }

    /// Whether any stock in this cell is at or above the feed threshold.
    pub fn on_food(&self) -> bool {
        self.above_threshold.iter().any(|f| *f)
    }

    /// Whether the mouth that serves `channel` was open above `floor`.
    pub fn mouth_open(&self, channel: usize, floor: f64) -> bool {
        self.effort[IntakeTick::mouth_of(channel)] > floor
    }

    /// One channel's §6.4 transaction, recorded where the world performs it.
    pub(crate) fn credit(
        &mut self,
        channel: usize,
        served: f64,
        digestible: f64,
        to_reserve: f64,
        to_battery: f64,
        rejected: f64,
    ) {
        self.served[channel] += served;
        self.digestible[channel] += digestible;
        self.reserve_credit[channel] += to_reserve;
        self.battery_credit[channel] += to_battery;
        self.battery_rejected[channel] += rejected.max(0.0);
    }
}

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
    /// The one body whose per-tick intake is traced, when tracing is on at all. One body,
    /// because the trace is a diagnostic for a named experiment and not a second ledger.
    target: Option<OrganismId>,
    /// This tick's row, open from before the tick's first mutation until after it commits.
    row: Option<IntakeTick>,
    rows: Vec<IntakeTick>,
    /// Rows discarded because nobody drained them.
    rows_dropped: u64,
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

    /// The body whose per-tick intake is traced, if any.
    #[inline]
    pub fn traced(&self) -> Option<OrganismId> {
        self.target
    }

    /// Trace one body's intake, or nobody. Changing the target discards every row, so a
    /// reader never sees two bodies' ticks interleaved in one trace.
    pub(crate) fn set_trace(&mut self, target: Option<OrganismId>) {
        if self.target == target {
            return;
        }
        self.target = target;
        self.row = None;
        self.rows.clear();
        self.rows_dropped = 0;
    }

    /// Open this tick's row. Called once per tick, before anything moves.
    #[inline]
    pub(crate) fn open_row(&mut self, tick: u64) {
        if self.target.is_some() {
            self.row = Some(IntakeTick::open(tick));
        }
    }

    /// This tick's row, for the traced body only. `None` for every other body and in every
    /// world that is not tracing, which is the one branch an ordinary world takes.
    #[inline]
    pub(crate) fn row_of(&mut self, id: OrganismId) -> Option<&mut IntakeTick> {
        if self.target != Some(id) {
            return None;
        }
        self.row.as_mut()
    }

    /// Close this tick's row: settle the one limit that needs the served bite, then keep it.
    pub(crate) fn close_row(&mut self) {
        let Some(mut row) = self.row.take() else {
            return;
        };
        if !row.observed {
            return;
        }
        for mouth in 0..MOUTHS {
            if row.limit[mouth] != IntakeLimit::MouthRate {
                continue;
            }
            let asked = row.requested[mouth];
            let got = row.served_by(mouth);
            // Relative, because a bite is of order 1e-4 m and an exact float equality here
            // would report float association as a clamp.
            if got < asked - (1e-12 + 1e-9 * asked) {
                row.limit[mouth] = IntakeLimit::StockShare;
            }
        }
        if self.rows.len() >= MAX_TRACE_ROWS {
            self.rows_dropped += 1;
            return;
        }
        self.rows.push(row);
    }

    /// The rows recorded so far, oldest first, and how many were dropped undrained.
    pub fn trace(&self) -> (&[IntakeTick], u64) {
        (&self.rows, self.rows_dropped)
    }

    /// Take the rows and reset the dropped counter.
    pub(crate) fn drain_trace(&mut self) -> (Vec<IntakeTick>, u64) {
        let dropped = std::mem::take(&mut self.rows_dropped);
        (std::mem::take(&mut self.rows), dropped)
    }

    /// Fill this tick's row from the feeding settlement's own inputs, before a single
    /// transfer is applied.
    ///
    /// Read-only: it takes the values the settlement is about to use and re-derives the same
    /// three bites with the same closure the settlement uses, in the same order — fruit takes
    /// its headroom first, then grazing, then scavenging (§6.4). It writes nothing a body or a
    /// field can read, draws no randomness, and is called only for the one traced body.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn observe_intake(
        &mut self,
        id: OrganismId,
        cell: u16,
        stock: [f64; CHANNELS],
        feed_threshold: f64,
        effort: [f64; MOUTHS],
        cap_foliage: f64,
        cap_detrital: f64,
        mouth_rate: f64,
        k_p: f64,
        dt: f64,
        reserve: f64,
        reserve_max: f64,
        energy: f64,
        energy_max: f64,
        raw_head: Option<[f64; crate::neural::action::ACT_LEN]>,
    ) {
        if self.target != Some(id) {
            return;
        }
        let Some(row) = self.row.as_mut() else {
            return;
        };
        row.observed = true;
        row.cell = cell;
        row.stock = stock;
        row.feed_threshold = feed_threshold;
        for (flag, s) in row.above_threshold.iter_mut().zip(stock) {
            *flag = s >= feed_threshold;
        }
        row.effort = effort;
        row.raw_head = raw_head;
        row.reserve = reserve;
        row.energy = energy;
        let headroom = (reserve_max - reserve).max(0.0);
        row.reserve_headroom = headroom;
        row.energy_headroom = (energy_max - energy).max(0.0);

        // §6.2: a channel whose machinery does not exist is refused in the world, whatever
        // asked for it. This is the settlement's own test, not a second rule.
        let open = [cap_foliage > 0.0, cap_foliage > 0.0, cap_detrital > 0.0];
        let want: [f64; MOUTHS] =
            std::array::from_fn(|m| if open[m] { effort[m].max(0.0) } else { 0.0 });
        let asked: f64 = want.iter().sum();
        // §6.3: one mouth. Normalise only when the efforts overcommit it.
        let norm = if asked > 1.0 { 1.0 / asked } else { 1.0 };
        row.effort_normalised = std::array::from_fn(|m| want[m] * norm);
        // The food each mouth bites: foliage, fruit, and the two detrital stocks together.
        let food = [stock[FOLIAGE], stock[FRUIT], stock[LITTER] + stock[CARRION]];
        for (m, f) in food.iter().enumerate() {
            let total = f + k_p;
            row.saturation[m] = if total > 0.0 { f / total } else { 0.0 };
            row.mouth_bite[m] = if row.effort_normalised[m] > 0.0 {
                mouth_rate * row.effort_normalised[m] * dt * row.saturation[m]
            } else {
                0.0
            };
        }
        // The settlement's own order: fruit, then grazing, then scavenging, each against what
        // the reserve has left.
        let mut room = headroom;
        for m in [MOUTH_FRUIT, MOUTH_GRAZE, MOUTH_SCAVENGE] {
            let bite = row.mouth_bite[m].clamp(0.0, room.max(0.0));
            row.requested[m] = bite;
            room -= bite;
        }
        let (bite, asked_for) = (row.mouth_bite, row.requested);
        row.limit = std::array::from_fn(|m| {
            if !open[m] {
                IntakeLimit::CapabilityZero
            } else if want[m] <= 0.0 {
                IntakeLimit::EffortZero
            } else if food[m] < feed_threshold {
                IntakeLimit::StockBelowThreshold
            } else if asked_for[m] < bite[m] - 1e-15 {
                IntakeLimit::ReserveHeadroom
            } else {
                // Refined to `StockShare` on close, when the served bite is known.
                IntakeLimit::MouthRate
            }
        });
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

    /// Turn workstream M's per-cell **plant** budget on or off for this world.
    ///
    /// The per-cell counterpart of [`World::record_body_budgets`], and inert in exactly the
    /// same sense: it is read only at sites that add to a counter, consumes no draw, moves no
    /// value the simulation reads, and is never persisted, hashed or snapshotted. Turning it
    /// on opens the record on the stocks as they stand *now*, so the identity
    /// `P − P₀ = in − out − withdrawal` is anchored at this tick; turning it off frees it.
    ///
    /// It lives in the plant step's own scratch ([`crate::fields::EcoScratch`]) because that
    /// is where §4.1–4.7 are computed, and the §6.4 withdrawal site in `world::step` books
    /// each bite into the same record. Nothing else in the world can see it.
    pub fn record_plant_budgets(&mut self, on: bool) {
        let tick = self.state.tick;
        let (p, q, w) = (
            &self.state.fields.p,
            &self.state.ecology.plant_reserve,
            &self.state.ecology.wood,
        );
        self.eco_scratch.record_plant_budget(on, p, q, w, tick);
    }

    /// Whether this world is recording the per-cell plant budget.
    pub fn records_plant_budgets(&self) -> bool {
        self.eco_scratch.plant_budget().is_some()
    }

    /// The per-cell plant budget so far, or `None` when recording is off.
    pub fn plant_budget(&self) -> Option<&crate::fields::PlantBudgetRecord> {
        self.eco_scratch.plant_budget()
    }

    /// The largest absolute residual of the record's three §4 identities against the world's
    /// own current stocks, over every cell — the record's self-check. `0.0` when off.
    pub fn plant_budget_residual(&self) -> f64 {
        match self.eco_scratch.plant_budget() {
            None => 0.0,
            Some(rec) => rec.max_residual(
                &self.state.fields.p,
                &self.state.ecology.plant_reserve,
                &self.state.ecology.wood,
                self.state.config.plant.build,
            ),
        }
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

    /// Trace one body's per-tick intake, or `None` to stop.
    ///
    /// Independent of [`World::record_body_budgets`] and inert in exactly the same sense: the
    /// flag is read only at sites that fill a counter, consumes no draw, moves no value the
    /// simulation reads, and is never persisted or hashed. One body at a time, because this is
    /// a diagnostic for a named experiment and not a second ledger.
    pub fn trace_intake(&mut self, target: Option<crate::ids::OrganismId>) {
        self.budgets.set_trace(target);
    }

    /// The body whose per-tick intake this world traces, if any.
    pub fn traced_intake(&self) -> Option<crate::ids::OrganismId> {
        self.budgets.traced()
    }

    /// The intake rows recorded so far, oldest first, and how many were dropped undrained.
    pub fn intake_trace(&self) -> (&[IntakeTick], u64) {
        self.budgets.trace()
    }

    /// Take the intake rows and reset the dropped counter.
    pub fn drain_intake_trace(&mut self) -> (Vec<IntakeTick>, u64) {
        self.budgets.drain_trace()
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
