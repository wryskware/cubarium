//! The world: checkpointed state plus transient caches, and the tick.

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use cubarium_surface::{
    CELL_COUNT, CellId, ChartImage, FACE_EXTENT, Face, FieldGraph, MAX_SEAMS, ScalarField,
    SurfacePoint, Travel, Vec2, cell_of, chart_images, face_frame, travel_into, unfold_with,
};

use crate::accounting::{self, EnergyCorrection, EnergyLedgers, Ledger};
use crate::care::{
    ActiveShower, CareApplied, CareCommand, CareDose, CareKind, CareOutcome, CareReceipt, CareState,
};
use crate::config::{FounderKind, WorldConfig};
use crate::controller::{Decision, Observation, TurnGate, decide_quiet, turn_toward};
use crate::dormancy::{
    ApexDormancyEvent, ApexDormancyPolicy, ApexDormancyState, EMERGENCE_ENERGY_FRACTION, EMERGENCE_RESERVE_FRACTION,
    MAINTENANCE_PER_STRUCTURE_SECOND, PREY_RADIUS_PX, PREY_REQUIRED, RECHECK_TICKS, SUSTAIN_TICKS,
};
use crate::events::LifeEvent;
use crate::encounter::{
    self, ApexContribution, ApexEncounterEvent, ApexEncounterPolicy, ApexEncounterState, CombatResponse,
    PairedGestation, PairedParentage,
};
use crate::fields::Fields;
use crate::genome::Phenotype;
use crate::motor::{self, MotorBill, MotorLimits, MotorRequest};
use crate::genome::{Genome, MAX_FORMS, decode};
use crate::habitat::{Habitat, Weather};
use crate::hunter::{
    self, AttemptOutcome, FixedHunterProfile, HunterControlReceipt, HunterEvent,
    HunterFounderReceipt, HunterPhase, HunterState, HunterTarget, HunterView,
};
use crate::ids::{OrganismId, Slots};
use crate::organism::{DeathCause, Escrow, Mode, Organism, Origin};
use crate::pairs::{Body, NeighborLists};
use crate::quiet::{QuietEvent, QuietOverride, QuietPause, QuietReason, QuietState};
use crate::rng::{Counter, Stream, draw, normal, unit};
use crate::telemetry::Telemetry;
use crate::view::{FieldDump, OrganismView, RenderView};
use crate::water;
use crate::{DT, care, pairs, snapshot};

/// Everything a checkpoint must capture. Transient caches are rebuilt on load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldState {
    pub config: WorldConfig,
    pub tick: u64,
    pub fields: Fields,
    pub weather: Weather,
    pub organisms: Slots<Organism>,
    /// Cumulative counters since world creation.
    pub births_total: u64,
    pub deaths_total: [u64; 3],
    pub cap_rejections_total: u64,
    /// Material admitted from outside (founders and any future stimuli), for the invariant.
    pub external_material_in: f64,
    /// Running energy audit.
    pub light_in_total: f64,
    pub heat_out_total: f64,
    /// Running water budget (`design/water.md`): `Σw == rain_in_total − evap_out_total`
    /// to rounding at every tick of a world created dry.
    #[serde(default)]
    pub rain_in_total: f64,
    #[serde(default)]
    pub evap_out_total: f64,
    /// Optional care (`design/7_Research/care-contract-2026-09-12.md`): the sequence cursor,
    /// any shower in progress, and the six ledgers the mass, energy and water identities
    /// carry. Schema 8 is schema 7 plus exactly this field, which is what makes
    /// [`crate::snapshot::WorldStateV7`] a byte-exact projection.
    #[serde(default)]
    pub care: CareState,
    /// Persisted signed Neumaier corrections for `light_in_total` and `heat_out_total`
    /// (`crate::accounting`, `design/7_Research/accounting-compensation-handoff-2026-09-13.md`).
    /// The raw counters above are written exactly as before; the corrected cumulative total
    /// of either ledger is `raw + correction`, read through [`WorldState::energy_ledgers`].
    ///
    /// **Appended last**: schema 9 is schema 8 plus exactly this field, which is what makes
    /// [`crate::snapshot::WorldStateV8`] a byte-exact projection. Zero for a world migrated
    /// from schema 7 or 8: the correction begins at migration and claims no repair of
    /// rounding already lost.
    #[serde(default)]
    pub energy_correction: EnergyCorrection,
    /// The opt-in paid hunter extension (`crate::hunter`): the trial profile, its members and
    /// their carried guts, the material and energy it imported, and its own counters.
    ///
    /// **Appended last**: schema 10 is schema 9 plus exactly this field, which is what makes
    /// [`crate::snapshot::WorldStateV9`] a byte-exact projection. Empty and inert for a world
    /// migrated from schema 9, 8 or 7 — a migration never introduces a predator — and empty
    /// for every world that never opts in.
    #[serde(default)]
    pub hunters: HunterState,
    /// The opt-in ordinary quiet extension (`crate::quiet`). Appended in schema 13, outside
    /// every legacy nested organism and config payload. Off with no entries by default, and an
    /// Off world never enters a quiet code path.
    #[serde(default)]
    pub quiet: QuietState,
    /// Opt-in underground dormancy for paid apex offspring. Appended in schema 14 so every
    /// earlier organism, hunter and quiet payload retains its exact wire shape.
    #[serde(default)]
    pub apex_dormancy: ApexDormancyState,
    /// Independently opt-in adult apex pairing and combat. Schema 14 is still unreleased and
    /// carries both milestone extensions; schema 13 migrates each one Off.
    #[serde(default)]
    pub apex_encounters: ApexEncounterState,
}

impl WorldState {
    /// Range checks after decode: finite numbers, nonnegative stocks, canonical positions,
    /// unit headings (renormalize if within 1e-6, else invalid), population ≤ cap, escrows
    /// nonnegative, genome fields in range, config valid.
    ///
    /// `validate` only reports; [`World::from_state`] renormalizes headings that are within
    /// tolerance before calling it, so a heading that still fails here is genuinely invalid.
    pub fn validate(&self) -> Result<(), String> {
        self.config.validate()?;
        let cap = self.config.capacity.max_organisms as usize;
        if self.organisms.len() > cap {
            return Err(format!(
                "population {} exceeds cap {cap}",
                self.organisms.len()
            ));
        }
        for (name, v) in [
            ("external_material_in", self.external_material_in),
            ("light_in_total", self.light_in_total),
            ("heat_out_total", self.heat_out_total),
        ] {
            if !v.is_finite() {
                return Err(format!("{name} is not finite"));
            }
        }
        for (name, v) in [
            ("n", &self.fields.n),
            ("p", &self.fields.p),
            ("d", &self.fields.d),
            ("de", &self.fields.de),
        ] {
            if v.len() != CELL_COUNT {
                return Err(format!(
                    "field {name} has {} cells, expected {CELL_COUNT}",
                    v.len()
                ));
            }
        }
        self.fields.check(self.config.detritus.energy_cap)?;
        self.care.validate(self.tick)?;
        self.hunters
            .validate(self.tick, &self.organisms, &self.config)?;
        self.apex_dormancy
            .validate(self.tick, cap, &self.organisms, &self.hunters)?;
        self.apex_encounters
            .validate(self.tick, cap, &self.organisms, &self.hunters)?;
        // The ordinary quiet extension, against this world's own clock, capacity and live
        // organisms — and against the hunter extension, which this slice refuses to combine
        // with an enabled policy (`crate::quiet`).
        self.quiet.validate(
            self.tick,
            self.config.capacity.max_organisms as usize,
            self.hunters.profile.is_some()
                || self.hunters.founders_placed > 0
                || self.hunters.control_deposited
                || !self.hunters.members.is_empty(),
            |id| self.organisms.get(id).is_some(),
        )?;
        // The corrections are signed, so they are checked as the *combined* totals they are
        // part of: finite corrections, and finite nonnegative corrected cumulative flows.
        // Nothing is clamped or reset — an unusable accounting state fails the load.
        self.energy_ledgers().validate()?;

        for blob in self
            .weather
            .light
            .iter()
            .chain(self.weather.moisture.iter())
        {
            if !blob
                .center
                .iter()
                .chain(blob.axis.iter())
                .all(|c| c.is_finite())
                || !blob.rate.is_finite()
            {
                return Err("weather blob has non-finite geometry".into());
            }
        }

        for (id, o) in self.organisms.iter() {
            let who = format!("organism {}:{}", id.slot, id.generation);
            for (name, v) in [("structure", o.structure), ("reserve", o.reserve)] {
                if !v.is_finite() || v < 0.0 {
                    return Err(format!("{who}: {name} = {v}"));
                }
            }
            if !o.energy.is_finite() || o.energy < 0.0 {
                return Err(format!("{who}: energy = {}", o.energy));
            }
            if !o.hunger_memory.is_finite() {
                return Err(format!("{who}: hunger memory is not finite"));
            }
            if !o.pos.is_canonical() {
                return Err(format!("{who}: position {:?} is not canonical", o.pos));
            }
            if !o.heading.is_finite() || (o.heading.length() - 1.0).abs() > HEADING_TOLERANCE {
                return Err(format!(
                    "{who}: heading {:?} is not a unit vector",
                    o.heading
                ));
            }
            if !o.ou.is_finite() {
                return Err(format!("{who}: OU vector is not finite"));
            }
            if o.born_tick > self.tick {
                return Err(format!(
                    "{who}: born at {} after tick {}",
                    o.born_tick, self.tick
                ));
            }
            if let Some(e) = &o.escrow {
                for (name, v) in [
                    ("structure", e.structure),
                    ("reserve", e.reserve),
                    ("energy", e.energy),
                ] {
                    if !v.is_finite() || v < 0.0 {
                        return Err(format!("{who}: escrow {name} = {v}"));
                    }
                }
                if e.started_tick > self.tick {
                    return Err(format!(
                        "{who}: escrow started at {} after tick {}",
                        e.started_tick, self.tick
                    ));
                }
                check_genome(&e.genome, &format!("{who} escrow"))?;
            }
            check_genome(&o.genome, &who)?;
        }
        Ok(())
    }

    /// Both compensated cumulative energy ledgers: the raw counters paired with their
    /// persisted corrections (`crate::accounting`).
    ///
    /// This is the accurate cumulative representation. `light_in_total` and `heat_out_total`
    /// on their own remain exactly what every earlier build wrote and are kept as explicit
    /// diagnostics and wire values, not as fully accurate totals.
    pub fn energy_ledgers(&self) -> EnergyLedgers {
        EnergyLedgers {
            light_in: Ledger {
                raw: self.light_in_total,
                correction: self.energy_correction.light_in,
            },
            heat_out: Ledger {
                raw: self.heat_out_total,
                correction: self.energy_correction.heat_out,
            },
        }
    }

    /// Corrected cumulative light admitted, `light_in_total + correction`.
    pub fn light_in_corrected(&self) -> f64 {
        self.energy_ledgers().light_in.total()
    }

    /// Corrected cumulative heat dissipated, `heat_out_total + correction`.
    pub fn heat_out_corrected(&self) -> f64 {
        self.energy_ledgers().heat_out.total()
    }

    /// Corrected cumulative `light_in − heat_out`. For an interval, hold an
    /// [`EnergyLedgers`] reading and use [`EnergyLedgers::net_since`] instead: differencing
    /// two corrected totals rounds the interval's flow away again.
    pub fn net_energy_in_corrected(&self) -> f64 {
        self.energy_ledgers().net()
    }
}

/// How far a stored heading may drift from unit length before it is invalid.
const HEADING_TOLERANCE: f64 = 1e-6;

/// Drift below this is ordinary transport rounding and is left exactly as stored.
const HEADING_REPAIR_FLOOR: f64 = 1e-12;

/// The founder inventory a profile and a world config imply, derived once and validated
/// before either hunter initializer changes anything.
struct HunterFounder {
    phenotype: Phenotype,
    structure: f64,
    reserve: f64,
    energy: f64,
    material_in: f64,
    energy_in: f64,
}

/// The gap a paid strike can still close, in pixels: its burst ceiling over its whole
/// duration. A windup is admitted when the prey is within the grasp tolerance **plus** this,
/// so the strike has something to close and the hunter does not cock at a prey it can never
/// reach (`design/7_Research/lanternjaw-ecology-animation-contract-2026-09-13.md`: sensing,
/// stopping distance, windup admission and end-of-strike contact are reconciled together).
fn strike_closing_px(profile: &FixedHunterProfile) -> f64 {
    profile.strike_speed_px_s * profile.strike_seconds
}

/// Radius, in pixels, used to unfold the sensed cell centers. Sensing reaches
/// `SENSE_DEPTH_MAX` graph hops (12 px of `sense_radius` at 4 px per cell), so the farthest
/// sensed center is about 14.5 px from any point of the own cell.
const CELL_UNFOLD_RADIUS: f64 = 20.0;

/// The deepest sensing ring the world precomputes: `ceil(12 / 4)` for the largest `sense`
/// the genome allows.
const SENSE_DEPTH_MAX: usize = 3;

/// Draws per birth on `Stream::Birth`: two for placement (direction, heading), then up to
/// six for mutation. Each birth of a parent gets its own block of counters.
const BIRTH_DRAWS: u64 = 16;

/// Largest energy-audit drift tolerated in one tick (`ΔE_total == light_in − heat_out`).
const AUDIT_TOLERANCE: f64 = 1e-9;

/// Gradient length below which the normalized gradient is reported as zero.
const GRADIENT_EPS: f64 = 1e-9;

/// Bounds from `design/m2-world-spec.md` "Organism representation" and `genome`'s docs.
pub(crate) fn check_genome(g: &Genome, who: &str) -> Result<(), String> {
    let range = |name: &str, v: f32, lo: f32, hi: f32| -> Result<(), String> {
        if !v.is_finite() || v < lo || v > hi {
            Err(format!("{who}: genome {name} = {v}, expected [{lo}, {hi}]"))
        } else {
            Ok(())
        }
    };
    if g.version != Genome::VERSION {
        return Err(format!(
            "{who}: genome version {} is not {}",
            g.version,
            Genome::VERSION
        ));
    }
    range("size", g.size, 0.5, 2.0)?;
    range("metabolism", g.metabolism, 0.5, 2.0)?;
    range("sense", g.sense, 2.0, 12.0)?;
    range("reserve", g.reserve, 0.5, 2.0)?;
    range("mouth", g.mouth, 0.2, 1.0)?;
    range("speed", g.speed, 0.3, 1.0)?;
    range("hue", g.hue, 0.0, 1.0)?;
    range("diet", g.diet, 0.0, 1.0)?;
    range("depth", g.depth, 0.0, 1.0)?;
    range("swim", g.swim, 0.0, 1.0)?;
    if g.form >= MAX_FORMS {
        return Err(format!(
            "{who}: genome form {} is not below {MAX_FORMS}",
            g.form
        ));
    }
    let d = &g.drives;
    for (name, v) in [
        ("w_food", d.w_food),
        ("w_detritus", d.w_detritus),
        ("w_persist", d.w_persist),
        ("w_crowd", d.w_crowd),
        ("w_depth", d.w_depth),
        ("turn_noise", d.turn_noise),
    ] {
        range(name, v, 0.0, 2.0)?;
    }
    for (name, v) in [
        ("seek_on", d.seek_on),
        ("seek_off", d.seek_off),
        ("feed_min", d.feed_min),
        ("rest_effort", d.rest_effort),
        ("feed_effort", d.feed_effort),
        ("bud_reserve", d.bud_reserve),
        ("bud_energy", d.bud_energy),
    ] {
        range(name, v, 0.0, 1.0)?;
    }
    if d.seek_off >= d.seek_on {
        return Err(format!(
            "{who}: seek_off {} is not below seek_on {}",
            d.seek_off, d.seek_on
        ));
    }
    range("tau_hunger_seconds", d.tau_hunger_seconds, 1.0, 60.0)?;
    for (name, v) in [
        ("bud_min_age_seconds", d.bud_min_age_seconds),
        ("turn_rate_max_deg", d.turn_rate_max_deg),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(format!("{who}: genome {name} = {v}"));
        }
    }
    Ok(())
}

/// Per-tick counters exposed to telemetry and reset each sample.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickCounters {
    pub births: u32,
    pub deaths: [u32; 3],
    pub cap_rejections: u32,
    pub light_in: f64,
    pub heat_out: f64,
    pub rain_in: f64,
    pub evap_out: f64,
    pub travel_fallbacks: u32,
    pub travel_ties: u32,
    /// Paid hunter attempts, successful captures and prey consumed this sample
    /// (`crate::hunter`). Zero in every world that never opted in.
    pub hunter_attacks: u32,
    pub hunter_captures: u32,
    pub deaths_predation: u32,
}

/// Read-only stock/flow diagnostics for the R0a food measurement
/// (`design/handoffs/r0a-movement-foundation-2026-09-14.md`, part B): what the world's
/// producers actually made and what its mouths actually took, as opposed to what a reserve
/// delta or a `Feeding` label suggests.
///
/// **Scope.** Totals over every cell and every organism, in material units, accumulated inside
/// the passes that already compute them. Intake is the material that actually left a field,
/// after the per-cell proportional share and after every clamp — never a request, never an
/// assimilated fraction, never a reserve difference.
///
/// **Time.** From the moment this `World` value was constructed to now, exactly as
/// [`ChargingDiagnostics`] documents. Transient: never persisted, never hashed, never read
/// back by the tick, and zero again after a reload.
///
/// **Cost.** Seven running scalars written inside branches the step already takes. No per-tick
/// history, no RNG, nothing read or written that the step did not already touch, so recording
/// this cannot move the simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IntakeDiagnostics {
    /// Gross producer material grown, before mortality, ripening or grazing.
    pub producer_growth: f64,
    /// Material that actually left `P`, `F` and `D` through a mouth.
    pub producer_eaten: f64,
    pub fruit_eaten: f64,
    pub detritus_eaten: f64,
    /// What the mouths asked their cells for, before the proportional share. Larger than the
    /// three totals above exactly when a cell could not serve everyone standing in it, which
    /// is what makes competition visible rather than inferred.
    pub requested: f64,
    /// Organism-ticks that asked for food, and the subset that got none of what they asked
    /// for because their reserve was already full.
    pub request_ticks: u64,
    pub reserve_saturated_ticks: u64,
}

/// Read-only diagnostics for the paid-charging experiment: what the member oxidation policy
/// did that the world's own configured threshold would **not** have done
/// (`design/7_Research/astra-hunter-paid-charging-proposal-2026-09-13.md`).
///
/// **Scope.** A transaction is counted here when, and only when, an authoritative hunter
/// member's oxidation ran at a resolved threshold *above* the world's configured one **and**
/// its energy was at or above the configured threshold — that is, the exact transactions a
/// [`crate::hunter::OxidationPolicy::Configured`] member would not have performed in the same
/// world at the same instant. Oxidation below the configured threshold is ordinary physiology
/// and is not counted, by either policy. Ordinary organisms are never counted.
///
/// **Time.** Totals run from the moment this `World` value was constructed — by
/// [`World::new`] or [`World::from_state`] — to now. They are **transient**: never persisted,
/// never hashed, never read back by the tick. A reloaded world starts them at zero, so a
/// reader must treat them as a property of one process's run, not of the world's history.
///
/// **Cost.** Four running scalars and a counter, written only inside a branch the step already
/// takes. No per-tick history is accumulated, no RNG is consumed and no world state is read or
/// written that the step did not already read or write, so recording this cannot move the
/// simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChargingDiagnostics {
    /// Oxidation transactions performed only because the member's threshold was raised.
    pub extra_transactions: u64,
    /// Reserve material burned in those transactions (m). It went to the local `N` cell, as
    /// every other oxidation's does.
    pub extra_reserve_burned: f64,
    /// Battery charge gained in them (e), after efficiency and the headroom cap.
    pub extra_energy_gained: f64,
    /// Conversion heat released by them (e): `e_r · burned − gained`, as always.
    pub extra_heat: f64,
}

pub struct World {
    pub state: WorldState,
    graph: FieldGraph,
    habitat: Habitat,
    images: [Vec<ChartImage>; 5],
    light: Box<[f64; CELL_COUNT]>,
    moisture: Box<[f64; CELL_COUNT]>,
    /// The moisture weather blob sum per cell, the rain driver (`design/water.md`).
    rain_source: Box<[f64; CELL_COUNT]>,
    /// This tick's rain rate per cell (d/s), for the render view.
    rain: Box<[f32; CELL_COUNT]>,
    /// This tick's *manual* rain rate per cell (d/s), rebuilt from the active shower. Only
    /// handed to `water::step` while a shower is running; with no shower the water stage is
    /// passed `None` and runs the pre-care arithmetic unchanged.
    manual_rain: Box<[f64; CELL_COUNT]>,
    /// The shower envelope, computed once: static for the life of the process.
    rain_envelope: [f64; care::RAIN_SAMPLES],
    scratch: (ScalarField, ScalarField),
    water_scratch: ScalarField,
    /// `sense_rings[cell][d − 1]`: the cells at graph distance exactly `d` from `cell`, for
    /// `d = 1..=SENSE_DEPTH_MAX`. Static for the life of the world.
    sense_rings: Vec<[Vec<CellId>; SENSE_DEPTH_MAX]>,
    neighbors: NeighborLists,
    travel_buf: Travel,
    /// This tick's traveled segments per slot, for the render view.
    moved: Vec<Vec<cubarium_surface::PathSegment>>,
    /// Births and deaths committed since the observer last drained them. Transient: never
    /// checkpointed, never hashed, never read back by the tick.
    events: Vec<LifeEvent>,
    /// Hunter attempts, captures, offspring and deaths since the observer last drained them.
    /// Transient in exactly the same sense (`crate::hunter::HunterEvent`).
    hunter_events: Vec<HunterEvent>,
    /// Ordinary-quiet begin/refuse/end/abort records since the observer last drained them.
    /// Transient in exactly the same sense (`crate::quiet::QuietEvent`).
    quiet_events: Vec<QuietEvent>,
    /// Underground entry/emergence/exhaustion records. Transient, never checkpointed.
    apex_dormancy_events: Vec<ApexDormancyEvent>,
    /// Adult pairing/combat facts, transient and separate from one-parent hunter records.
    apex_encounter_events: Vec<ApexEncounterEvent>,
    counters: TickCounters,
    /// Bounded read-only totals for the paid-charging policy. Transient in exactly the sense
    /// [`ChargingDiagnostics`] documents: never checkpointed, never hashed, never read by the
    /// tick.
    charging: ChargingDiagnostics,
    /// Bounded read-only stock/flow totals; transient in the sense [`IntakeDiagnostics`]
    /// documents.
    intake: IntakeDiagnostics,
    initial_material: f64,
}

// --- ordinary quiet (`crate::quiet`) ------------------------------------------------------
//
// Both helpers are observer-shaped: they read the organism, never charge it, and consume no RNG
// draw. They run only in a world whose policy is enabled.

/// What the controller should do with this organism's decision at boundary `now`.
///
/// Expiry and early abort both remove the entry and return an override that substitutes the
/// **underlying** ordinary mode without imposing anything, so the parent uses the ordinary
/// controller on that same tick and re-enters the hysteresis from where it really was.
fn quiet_hold(
    quiet: &mut QuietState,
    events: &mut Vec<QuietEvent>,
    id: OrganismId,
    o: &Organism,
    cfg: &crate::config::OrganismConfig,
    now: u64,
    dt: f64,
) -> Option<QuietOverride> {
    let index = quiet.pauses.iter().position(|p| p.parent == id)?;
    let p = quiet.pauses[index];
    let released = Some(QuietOverride {
        underlying: p.underlying,
        hold: false,
    });
    if !p.holds(now) {
        // The promised window is over. Release at its exact end, not one tick late.
        quiet.pauses.remove(index);
        events.push(QuietEvent::End {
            tick: now,
            parent: p.parent,
            child: p.child,
            completed_ticks: p.completed(now),
            underlying: p.underlying,
        });
        return released;
    }
    // The conservative budget is re-tested before every held decision, over the decisions that
    // remain plus one ordinary tick. A parent that can no longer cover it stops resting now.
    let abort = |events: &mut Vec<QuietEvent>, quiet: &mut QuietState, reason: QuietReason| {
        quiet.pauses.remove(index);
        events.push(QuietEvent::Abort {
            tick: now,
            parent: p.parent,
            child: p.child,
            completed_ticks: p.completed(now),
            reason,
        });
        Some(QuietOverride {
            underlying: p.underlying,
            hold: false,
        })
    };
    let Some(horizon) = crate::quiet::horizon_seconds(p.remaining(now), dt) else {
        return abort(events, quiet, QuietReason::Overflow);
    };
    let Some(budget) = crate::quiet::Budget::of(o, cfg, horizon) else {
        return abort(events, quiet, QuietReason::InvalidInputs);
    };
    if !budget.affordable(o) {
        return abort(events, quiet, QuietReason::UnaffordableRemaining);
    }
    Some(QuietOverride {
        underlying: p.underlying,
        hold: true,
    })
}

/// Offer a pause to the parent of an actual paid insertion at completed boundary `birth_tick`.
///
/// Every refusal is recorded and the opportunity is then forgotten: a hungry parent is never
/// made to wait for permission to act, and nothing is retried on a later tick.
#[allow(clippy::too_many_arguments)]
fn quiet_admit(
    quiet: &mut QuietState,
    events: &mut Vec<QuietEvent>,
    organisms: &Slots<Organism>,
    cfg: &crate::config::OrganismConfig,
    cap: usize,
    birth_tick: u64,
    dt: f64,
    parent: OrganismId,
    child: OrganismId,
    hunter_parent: bool,
) {
    let mut refuse = |reason: QuietReason| {
        events.push(QuietEvent::Refuse {
            tick: birth_tick,
            parent,
            child,
            reason,
        });
    };
    if hunter_parent {
        // `QuietState::validate` already refuses this combination outright; the record exists so
        // a harness sees why nothing happened rather than inferring it from silence.
        return refuse(QuietReason::HunterMember);
    }
    let Some(o) = organisms.get(parent) else {
        return refuse(QuietReason::ParentGone);
    };
    if quiet.pauses.iter().any(|p| p.parent == parent) {
        return refuse(QuietReason::AlreadyPaused);
    }
    if quiet.pauses.len() >= cap {
        return refuse(QuietReason::Bounded);
    }
    let Some(end_tick) = birth_tick.checked_add(crate::quiet::POST_BIRTH_PAUSE_TICKS) else {
        return refuse(QuietReason::Overflow);
    };
    let Some(horizon) = crate::quiet::horizon_seconds(crate::quiet::POST_BIRTH_PAUSE_TICKS, dt)
    else {
        return refuse(QuietReason::Overflow);
    };
    let Some(budget) = crate::quiet::Budget::of(o, cfg, horizon) else {
        return refuse(QuietReason::InvalidInputs);
    };
    if !budget.affordable(o) {
        return refuse(QuietReason::Unaffordable);
    }
    // The mode the parent is actually in as this tick closes: the ordinary value release will
    // resume from.
    let pause = QuietPause {
        parent,
        child,
        start_tick: birth_tick,
        end_tick,
        underlying: o.mode,
    };
    let at = quiet.pauses.partition_point(|p| {
        (p.parent.slot, p.parent.generation) < (parent.slot, parent.generation)
    });
    quiet.pauses.insert(at, pause);
    events.push(QuietEvent::Begin {
        tick: birth_tick,
        parent,
        child,
        end_tick,
        underlying: pause.underlying,
    });
}

impl World {
    /// Create a new world: validate config, build habitat/weather, initial fields, and
    /// `founders.count` founders placed uniformly by area (rejection sampling on the five
    /// faces from `Stream::Founders`, key = index, counters 0..3 for face/u/v/heading and
    /// 4 for hue), each with the founder genome, `S = S_adult`,
    /// `R = initial_reserve_fraction · R_max`, `E = initial_energy_fraction · E_max`.
    /// Founder material is recorded in `external_material_in`.
    ///
    /// The five faces have equal area, so a uniform face draw followed by uniform `(u, v)`
    /// is already uniform by area; no rejection is needed.
    pub fn new(config: WorldConfig) -> Result<World, String> {
        config.validate()?;
        let habitat = Habitat::new(&config.habitat, config.seed);
        let weather = Weather::new(&config.weather, config.seed);
        let fields = Fields::new(&config, &habitat.light_base, &habitat.moisture_base);

        let cap = config.capacity.max_organisms;
        let mut organisms = Slots::with_capacity(cap as usize);
        let mut external_material_in = 0.0;
        // The founder roster: `(draw key, kind)` per founder. With kinds, the kind index is
        // folded into the high bits of the key so each kind's draws are their own stream and
        // a seed stays reproducible when another kind's count changes; without kinds the
        // v1 path keeps `key = index`.
        let roster: Vec<(u64, Option<&FounderKind>)> = if config.founders.kinds.is_empty() {
            (0..u64::from(config.founders.count))
                .map(|i| (i, None))
                .collect()
        } else {
            config
                .founders
                .kinds
                .iter()
                .enumerate()
                .flat_map(|(k, kind)| {
                    (0..u64::from(kind.count)).map(move |j| (((k as u64) << 32) | j, Some(kind)))
                })
                .collect()
        };
        for (index, kind) in roster.into_iter().take(cap as usize) {
            let seed = config.seed;
            let face_index = (unit(seed, Stream::Founders, index, 0) * 5.0).floor();
            let face = Face::from_index(face_index as u8).unwrap_or(Face::Front);
            let u = unit(seed, Stream::Founders, index, 1) * FACE_EXTENT;
            let v = unit(seed, Stream::Founders, index, 2) * FACE_EXTENT;
            let heading = Vec2::from_screen_angle(unit(seed, Stream::Founders, index, 3) * TAU);
            let hue = unit(seed, Stream::Founders, index, 4) as f32;

            let mut genome =
                Genome::founder(kind.and_then(|k| k.hue).unwrap_or(hue), &config.drives);
            // Founders sense at the configured radius; the genome bounds still apply.
            genome.sense = config.organism.sense_radius as f32;
            if let Some(k) = kind {
                // A kind fixes the loci it names; the rest keep the v1 founder values. The
                // rig follows the kind, or the hue tercile when the kind leaves it open.
                if let Some(x) = k.diet {
                    genome.diet = x;
                }
                if let Some(x) = k.depth {
                    genome.depth = x;
                }
                if let Some(x) = k.speed {
                    genome.speed = x;
                }
                if let Some(x) = k.size {
                    genome.size = x;
                }
                if let Some(x) = k.metabolism {
                    genome.metabolism = x;
                }
                if let Some(x) = k.swim {
                    genome.swim = x;
                }
                if let Some(x) = k.form {
                    genome.form = x;
                } else {
                    genome.form = crate::genome::form_of_hue(genome.hue);
                }
            }
            genome.clamp();
            let phenotype = decode(&genome, &config.organism);
            let structure = phenotype.structure_adult;
            let reserve = config.founders.initial_reserve_fraction * phenotype.reserve_max;
            let energy = config.founders.initial_energy_fraction * phenotype.energy_max;
            external_material_in += structure + reserve;
            organisms.insert(Organism {
                pos: SurfacePoint::new(face, u, v).canonicalize(),
                heading,
                ou: Vec2::ZERO,
                structure,
                reserve,
                energy,
                born_tick: 0,
                hunger_memory: (1.0 - reserve / phenotype.reserve_max).clamp(0.0, 1.0),
                mode: Mode::Resting,
                escrow: None,
                births: 0,
                genome,
                phenotype,
                parent: None,
                origin: Origin::Founder,
                turn_counter: Counter::default(),
                fed_this_tick: false,
            });
        }

        // The residual is measured against the field material present at creation; the
        // founders arrive from outside and are booked in `external_material_in`.
        let initial_material = fields.total_material();
        let state = WorldState {
            config,
            tick: 0,
            fields,
            weather,
            organisms,
            births_total: 0,
            deaths_total: [0; 3],
            cap_rejections_total: 0,
            external_material_in,
            light_in_total: 0.0,
            heat_out_total: 0.0,
            rain_in_total: 0.0,
            evap_out_total: 0.0,
            care: CareState::default(),
            energy_correction: EnergyCorrection::default(),
            hunters: HunterState::default(),
            quiet: QuietState::default(),
            apex_dormancy: ApexDormancyState::default(),
            apex_encounters: ApexEncounterState::default(),
        };
        Ok(World::assemble(state, habitat, initial_material))
    }

    /// Rebuild caches around a validated state (after a snapshot load).
    ///
    /// The residual baseline is re-derived so that it reads zero at load and reports drift
    /// since the load; a snapshot does not carry the pre-load residual.
    pub fn from_state(mut state: WorldState) -> Result<World, String> {
        for (_, o) in state.organisms.iter_mut() {
            // A genome written before fauna v2 is brought to version 2 in place, escrow
            // included; the phenotype is re-decoded so the new loci take effect.
            let upgraded = o.genome.upgrade();
            if let Some(e) = &mut o.escrow {
                e.genome.upgrade();
            }
            if upgraded {
                o.phenotype = decode(&o.genome, &state.config.organism);
            }
            // Repair a heading that drifted, but never touch one that is already unit to
            // rounding: renormalizing it would change the state bit-for-bit and a reloaded
            // world would no longer replay identically to the one that wrote the snapshot.
            let len = o.heading.length();
            if (len - 1.0).abs() > HEADING_REPAIR_FLOOR && (len - 1.0).abs() <= HEADING_TOLERANCE {
                o.heading = o.heading * (1.0 / len);
            }
        }
        state.validate()?;
        let habitat = Habitat::new(&state.config.habitat, state.config.seed);
        let organism_material: f64 = state.organisms.iter().map(|(_, o)| o.material()).sum();
        // The same terms `mass_residual` subtracts, so a loaded world reads zero: care has
        // imported `feed_material_in` and exported `clean_material_out` since creation, the
        // hunter extension has imported its founders (and any budget-matched control deposit),
        // and a carried carcass is material that is still in the world.
        let initial_material =
            state.fields.total_material() + organism_material + state.hunters.gut_material_total()
                - state.external_material_in
                - state.care.feed_material_in
                + state.care.clean_material_out
                - state.hunters.imported_material();
        Ok(World::assemble(state, habitat, initial_material))
    }

    fn assemble(state: WorldState, habitat: Habitat, initial_material: f64) -> World {
        let mut world = World {
            state,
            graph: FieldGraph::new(),
            habitat,
            images: std::array::from_fn(|i| {
                let mut v = Vec::new();
                chart_images(
                    Face::from_index(i as u8).expect("five faces"),
                    MAX_SEAMS,
                    &mut v,
                );
                v
            }),
            light: Box::new([0.0; CELL_COUNT]),
            moisture: Box::new([0.0; CELL_COUNT]),
            rain_source: Box::new([0.0; CELL_COUNT]),
            rain: Box::new([0.0; CELL_COUNT]),
            manual_rain: Box::new([0.0; CELL_COUNT]),
            rain_envelope: care::rain_envelope(),
            scratch: (ScalarField::zeros(), ScalarField::zeros()),
            water_scratch: ScalarField::zeros(),
            sense_rings: Vec::new(),
            neighbors: NeighborLists::default(),
            travel_buf: Travel::default(),
            moved: Vec::new(),
            events: Vec::new(),
            hunter_events: Vec::new(),
            quiet_events: Vec::new(),
            apex_dormancy_events: Vec::new(),
            apex_encounter_events: Vec::new(),
            counters: TickCounters::default(),
            charging: ChargingDiagnostics::default(),
            intake: IntakeDiagnostics::default(),
            initial_material,
        };
        // Make the derived light/moisture readable before the first tick advances weather.
        let cfg = &world.state.config;
        world.state.weather.sample(
            &cfg.weather,
            &world.habitat,
            &mut world.light,
            &mut world.moisture,
            &mut world.rain_source,
            cfg.habitat.moisture_min,
        );
        world
            .moved
            .resize_with(world.state.organisms.slot_count(), Vec::new);
        world.sense_rings = sense_rings(&world.graph);
        world
    }

    /// One tick in the normative order of `design/m2-world-spec.md` "Tick order".
    /// Returns the per-tick counters (also accumulated internally for telemetry).
    pub fn step(&mut self) -> &TickCounters {
        let dt = DT;
        // The audit reads the *corrected* ledgers: the identity it checks is the one the
        // world actually books, and over a long run the raw counters no longer are.
        #[cfg(debug_assertions)]
        let audit = (
            stored_energy(&self.state),
            self.state.energy_ledgers(),
            self.state.care.feed_energy_in,
            self.state.care.clean_energy_out,
            self.state.hunters.imported_energy(),
        );
        #[cfg(debug_assertions)]
        let water_audit = (
            self.state.fields.w.iter().sum::<f64>(),
            self.state.rain_in_total,
            self.state.evap_out_total,
        );
        {
            let World {
                state,
                graph,
                habitat,
                images,
                light,
                moisture,
                rain_source,
                rain,
                manual_rain,
                rain_envelope,
                scratch,
                water_scratch,
                sense_rings,
                neighbors,
                travel_buf,
                moved,
                events,
                hunter_events,
                quiet_events,
                apex_dormancy_events,
                apex_encounter_events,
                counters,
                charging,
                intake,
                initial_material: _,
            } = &mut *self;
            let WorldState {
                config,
                tick,
                fields,
                weather,
                organisms,
                births_total,
                deaths_total,
                cap_rejections_total,
                external_material_in: _,
                light_in_total,
                heat_out_total,
                rain_in_total,
                evap_out_total,
                care,
                // Destructured field by field so the heat closure and the light accumulation
                // borrow disjoint components.
                energy_correction:
                    EnergyCorrection {
                        light_in: light_in_correction,
                        heat_out: heat_out_correction,
                    },
                hunters,
                quiet,
                apex_dormancy,
                apex_encounters,
            } = state;
            let cfg: &WorldConfig = config;
            let org_cfg = &cfg.organism;
            let e_r = org_cfg.reserve_energy_density;
            let k_p = org_cfg.intake_half_saturation;
            let e_f = cfg.fruit.energy_density;
            let gate = TurnGate::from_config(org_cfg);
            // One boolean, read once: an Off world never touches a quiet code path again.
            let quiet_on = quiet.active();
            // Read once. Off remains a branch-free no-op at every lifecycle mutation site.
            let apex_dormancy_on = apex_dormancy.active();
            // Independent from offspring dormancy: either policy can be screened alone.
            let apex_encounters_on = apex_encounters.active();
            let now = *tick;
            let seed = cfg.seed;
            // Every heat payment of the tick goes through here: the transient counter keeps
            // its existing arithmetic and reset semantics, and the persisted raw total gets
            // exactly the addition it always got, with the bits it drops booked into the
            // correction (`crate::accounting`).
            let mut heat = |amount: f64| {
                counters.heat_out += amount;
                accounting::accumulate(heat_out_total, heat_out_correction, amount);
            };

            // 1. Admit due stimuli. The queue is empty in M2; the hook is the journal.

            // 2. Weather advance, then derive light, moisture and the rain source per cell.
            weather.advance(&cfg.weather, seed, now);
            weather.sample(
                &cfg.weather,
                habitat,
                light,
                moisture,
                rain_source,
                cfg.habitat.moisture_min,
            );

            // 2b. Water: rain, downhill flow, evaporation (`design/water.md`). Before the
            //     field reactions, so growth sees this tick's wetness and flooding.
            //
            //     Manual rain (the care contract) joins the natural rate here and nowhere
            //     else, so `rain[c]` publishes what actually falls and the depth is inside
            //     `rain_in_total` once. With no shower running the water stage is handed
            //     `None` and executes the pre-care arithmetic operation for operation.
            let manual: Option<&[f64; CELL_COUNT]> = if care.showers.is_empty() {
                None
            } else {
                manual_rain.fill(0.0);
                for shower in care.showers.iter() {
                    let k = shower.delivered as usize;
                    let Some(&e) = rain_envelope.get(k) else {
                        continue;
                    };
                    // The shower's own persisted dose, not whatever a panel now offers.
                    let depth = shower.dose().scale(care::RAIN_DEPTH_TOTAL);
                    for (c, w) in shower.cells.iter().zip(shower.weights.iter()) {
                        manual_rain[usize::from(*c)] += depth * w * e;
                    }
                }
                Some(&**manual_rain)
            };
            let water_ledger = water::step(
                &mut fields.w,
                &cfg.water,
                water::Drivers {
                    terrain: &habitat.terrain,
                    light,
                    rain_source,
                },
                manual,
                rain,
                graph,
                water_scratch,
            );
            counters.rain_in += water_ledger.rain_in;
            counters.evap_out += water_ledger.evap_out;
            *rain_in_total += water_ledger.rain_in;
            *evap_out_total += water_ledger.evap_out;
            if !care.showers.is_empty() {
                // Book the manual share of the depth that actually landed, then retire the
                // shower once its 120th sample has been delivered.
                care.rain_depth_in += water_ledger.manual_in;
                for shower in care.showers.iter_mut() {
                    shower.delivered += 1;
                }
                care.showers.retain(|s| s.delivered < care::RAIN_TICKS);
            }

            // 3. Field reactions (growth, mortality, decomposition, N diffusion).
            let ledger = fields.react(cfg, light, moisture, graph, scratch);
            intake.producer_growth += ledger.producer_growth;
            counters.light_in += ledger.light_in;
            accounting::accumulate(light_in_total, light_in_correction, ledger.light_in);
            heat(ledger.heat_out);

            // 4. Pair pass.
            let mut bodies: Vec<Body> = Vec::with_capacity(organisms.len());
            for (id, o) in organisms.iter() {
                if apex_dormancy_on && apex_dormancy.contains(id) {
                    continue;
                }
                bodies.push(Body {
                    id,
                    pos: o.pos,
                    sense_radius: o.phenotype.sense_radius,
                    extent: o.phenotype.extent,
                });
            }
            pairs::build(
                &bodies,
                images,
                cfg.capacity.max_neighbors as usize,
                neighbors,
            );

            // 5. Observe and decide (pure per organism, from the pre-movement world).
            let mut decisions: Vec<(OrganismId, Decision)> = Vec::with_capacity(organisms.len());
            for (id, o) in organisms.iter_mut() {
                // A concealed offspring has no surface observation, controller draw, movement,
                // intake or ordinary physiology. Its dedicated paid lifecycle runs below.
                if apex_dormancy_on && apex_dormancy.contains(id) {
                    continue;
                }
                let cell = cell_of(&o.pos);
                let here = cell.index();
                let chart = o.pos.chart();
                let mut obs = Observation {
                    p_here: fields.p[here],
                    f_here: fields.f[here],
                    d_here: edible_detritus(fields.d[here], fields.de[here], e_r),
                    height: o.pos.embed()[1],
                    up: up_direction(o.pos.face),
                    ..Observation::default()
                };
                // Sensing reaches `ceil(r_sense / 4)` graph hops (`design/fauna-v2.md`): each
                // sensed cell contributes its finite-difference slope toward it.
                let depth = sense_depth(o.phenotype.sense_radius);
                for ring in &sense_rings[here][..depth] {
                    for neighbor in ring {
                        let center = neighbor.center();
                        let Some(view) = unfold_with(
                            &images[o.pos.face.index()],
                            o.pos,
                            center,
                            CELL_UNFOLD_RADIUS,
                        ) else {
                            continue;
                        };
                        let Some(dir) = (view.local - chart).normalized() else {
                            continue;
                        };
                        if view.distance <= GRADIENT_EPS || view.distance.is_nan() {
                            continue;
                        }
                        let slope = dir * (1.0 / view.distance);
                        let there = neighbor.index();
                        let edible = edible_detritus(fields.d[there], fields.de[there], e_r);
                        obs.grad_p += slope * (fields.p[there] - obs.p_here);
                        obs.grad_f += slope * (fields.f[there] - obs.f_here);
                        obs.grad_d += slope * (edible - obs.d_here);
                    }
                }
                obs.grad_p = normalize_or_zero(obs.grad_p);
                obs.grad_f = normalize_or_zero(obs.grad_f);
                obs.grad_d = normalize_or_zero(obs.grad_d);

                if let Some(list) = neighbors.lists.get(id.slot as usize) {
                    for n in list {
                        let extent_sum = o.phenotype.extent + n.extent;
                        if n.distance >= extent_sum + 1.0 {
                            continue;
                        }
                        let delta = chart - n.local;
                        let len_sq = delta.length_sq();
                        if len_sq > GRADIENT_EPS {
                            obs.repulsion += delta * (extent_sum / len_sq);
                        }
                    }
                }

                // Two standard normals cost two counters each.
                let cx = o.turn_counter.take();
                let _ = o.turn_counter.take();
                let cy = o.turn_counter.take();
                let _ = o.turn_counter.take();
                let key = u64::from(id.slot);
                obs.noise = Vec2::new(
                    normal(seed, Stream::OrganismTurn, key, cx),
                    normal(seed, Stream::OrganismTurn, key, cy),
                );

                // The ordinary-quiet override (`crate::quiet`). `None` in every Off world,
                // and `decide_quiet(.., None)` is `decide` expression for expression, so the
                // reference trajectory is untouched. The noise pair above is drawn either way.
                let quiet_override = if quiet_on {
                    quiet_hold(quiet, quiet_events, id, o, org_cfg, now, dt)
                } else {
                    None
                };
                let decision = decide_quiet(
                    o,
                    &obs,
                    now,
                    dt,
                    cfg.mechanisms.grazing,
                    cfg.mechanisms.scavenging,
                    gate,
                    quiet_override,
                );
                // Carry the ordinary mode forward, once, so release resumes the real hysteresis
                // rather than the imposed Resting.
                if quiet_override.is_some_and(|q| q.hold)
                    && let Some(p) = quiet.pauses.iter_mut().find(|p| p.parent == id)
                {
                    p.underlying = decision.underlying_mode;
                }
                decisions.push((id, decision));
            }

            // 5b. Hunters (`crate::hunter`), when any member exists: advance each member's
            //     local phase from the pre-movement world, charge a strike in full at its
            //     entry, and turn the phase into a movement intent. Prey that actually senses
            //     a hunter pursuing *it* gets an escape term. Both are skipped whole when the
            //     extension is empty, so a world without hunters runs the pre-hunter tick
            //     operation for operation, with no additional draws.
            //
            //     A boost is an absolute speed ceiling in px/s, still divided by wading and
            //     still capped by the movement energy the creature actually has.
            let mut boosts: Vec<(OrganismId, f64)> = Vec::new();

            // 5a. Adult apex encounters. Work from canonical unordered neighbor pairs and mark
            // both participants used before applying an action: no self-pair, reverse duplicate,
            // distant mating, or second partner reuse can occur in this tick. The ordinary
            // hunter reproduction stream remains strictly one-parent; joint transactions are
            // published only through `ApexEncounterEvent`.
            if apex_encounters_on
                && !hunters.members.is_empty()
                && let Some(profile) = hunters.profile.clone()
            {
                let mut pairs = Vec::new();
                for m in &hunters.members {
                    if m.phase != HunterPhase::Perched
                        || (apex_dormancy_on && apex_dormancy.contains(m.id))
                        || apex_encounters
                            .gestations
                            .iter()
                            .any(|g| g.carrier == m.id || g.partner == m.id)
                    {
                        continue;
                    }
                    let Some(a) = organisms.get(m.id) else {
                        continue;
                    };
                    if a.structure < a.phenotype.structure_adult - hunter::TOLERANCE {
                        continue;
                    }
                    if let Some(sensed) = neighbors.lists.get(m.id.slot as usize) {
                        for n in sensed {
                            if m.id >= n.id || !hunters.contains(n.id) {
                                continue;
                            }
                            let Some(other) = hunters.member(n.id) else {
                                continue;
                            };
                            if other.phase != HunterPhase::Perched
                                || (apex_dormancy_on && apex_dormancy.contains(n.id))
                                || apex_encounters
                                    .gestations
                                    .iter()
                                    .any(|g| g.carrier == n.id || g.partner == n.id)
                            {
                                continue;
                            }
                            let Some(b) = organisms.get(n.id) else {
                                continue;
                            };
                            if b.structure < b.phenotype.structure_adult - hunter::TOLERANCE {
                                continue;
                            }
                            pairs.push((n.distance, m.id, n.id));
                        }
                    }
                }
                pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
                let mut used: Vec<OrganismId> = Vec::new();
                let gestation_ticks = ticks_from_seconds(profile.gestation_seconds, dt);
                let interval_ticks = ticks_from_seconds(profile.reproduce_interval_seconds, dt);
                let encounter_cooldown = gestation_ticks.saturating_add(interval_ticks);
                let recovery_ticks = ticks_from_seconds(profile.recovery_seconds, dt).max(1);

                for (distance, a_id, b_id) in pairs {
                    if used.contains(&a_id) || used.contains(&b_id) {
                        continue;
                    }
                    let (Some(a_index), Some(b_index)) =
                        (hunters.index_of(a_id), hunters.index_of(b_id))
                    else {
                        continue;
                    };
                    let (Some(a), Some(b)) = (organisms.get(a_id), organisms.get(b_id)) else {
                        continue;
                    };
                    let a_ready = hunters.members[a_index].phase == HunterPhase::Perched
                        && hunter::may_reproduce(&profile, a, &hunters.members[a_index], now, dt)
                        && !apex_encounters.gestations.iter().any(|g| g.partner == a_id);
                    let b_ready = hunters.members[b_index].phase == HunterPhase::Perched
                        && hunter::may_reproduce(&profile, b, &hunters.members[b_index], now, dt)
                        && !apex_encounters.gestations.iter().any(|g| g.partner == b_id);

                    if distance <= encounter::MATING_RADIUS_PX
                        && a_ready
                        && b_ready
                        && organisms.len() < cfg.capacity.max_organisms as usize
                    {
                        let genome = encounter::recombine(&a.genome, &b.genome, a_id, b_id);
                        let phenotype = decode(&genome, org_cfg);
                        let structure = org_cfg.child_structure_fraction * phenotype.structure_adult;
                        let reserve = org_cfg.child_reserve_fraction * phenotype.reserve_max;
                        let energy = org_cfg.child_energy_fraction * phenotype.energy_max;
                        let build = org_cfg.build_cost * structure;
                        let paid = ApexContribution {
                            structure: structure * 0.5,
                            reserve: reserve * 0.5,
                            energy: energy * 0.5,
                            build_heat: build * 0.5,
                        };
                        let material_due = paid.material();
                        let energy_due = paid.energy + paid.build_heat;
                        if a.reserve >= material_due
                            && b.reserve >= material_due
                            && a.energy >= energy_due
                            && b.energy >= energy_due
                        {
                            // End the immutable reads before mutating the two distinct slots.
                            let child_genome = genome.digest();
                            let a = organisms.get_mut(a_id).expect("paired adult remained alive");
                            a.reserve -= material_due;
                            a.energy -= energy_due;
                            a.escrow = Some(Escrow {
                                structure,
                                reserve,
                                energy,
                                started_tick: now,
                                genome,
                            });
                            let b = organisms.get_mut(b_id).expect("paired adult remained alive");
                            b.reserve -= material_due;
                            b.energy -= energy_due;
                            hunters.members[a_index].next_reproduction_tick =
                                now.saturating_add(encounter_cooldown);
                            hunters.members[b_index].next_reproduction_tick =
                                now.saturating_add(encounter_cooldown);
                            let record = PairedGestation {
                                carrier: a_id,
                                partner: b_id,
                                started_tick: now,
                                carrier_paid: paid,
                                partner_paid: paid,
                            };
                            assert!(apex_encounters.insert_gestation(record));
                            apex_encounters.matings_total += 1;
                            heat(build);
                            apex_encounter_events.push(ApexEncounterEvent::Mated {
                                tick: now + 1,
                                carrier: a_id,
                                partner: b_id,
                                carrier_paid: paid,
                                partner_paid: paid,
                                child_genome,
                            });
                            used.extend([a_id, b_id]);
                            continue;
                        }
                    }

                    // A failed mutual funding is not a free attack. Combat is an alternative
                    // only when mating itself was unavailable and the profile permits attacks.
                    if distance > encounter::COMBAT_RADIUS_PX
                        || !profile.attacks_enabled
                        || (a_ready && b_ready)
                    {
                        continue;
                    }
                    let a_hungry = a.reserve
                        < profile.seek_reserve_fraction * a.phenotype.reserve_max;
                    let b_hungry = b.reserve
                        < profile.seek_reserve_fraction * b.phenotype.reserve_max;
                    let (attacker, defender, attacker_index, defender_index) =
                        if a_hungry || (!b_hungry && !a_ready) {
                            (a_id, b_id, a_index, b_index)
                        } else {
                            (b_id, a_id, b_index, a_index)
                        };
                    let Some(attacker_o) = organisms.get(attacker) else {
                        continue;
                    };
                    if attacker_o.energy < profile.strike_energy_cost {
                        continue;
                    }
                    let defender_o = organisms.get(defender).expect("paired defender is live");
                    let retreat_cost = profile.strike_energy_cost * 0.5;
                    let retreats = defender_o.structure < attacker_o.structure
                        && defender_o.energy >= retreat_cost;
                    let retaliates = !retreats && defender_o.energy >= profile.strike_energy_cost;
                    let attack_injury = encounter::INJURY_ADULT_FRACTION
                        * attacker_o.phenotype.structure_adult;
                    let defend_injury = encounter::INJURY_ADULT_FRACTION
                        * defender_o.phenotype.structure_adult;
                    let mut attacker_injury = 0.0;
                    let mut defender_injury = 0.0;
                    let defender_energy_paid;
                    let response;

                    {
                        let o = organisms.get_mut(attacker).expect("attacker remained alive");
                        o.energy -= profile.strike_energy_cost;
                    }
                    heat(profile.strike_energy_cost);
                    if retreats {
                        let o = organisms.get_mut(defender).expect("defender remained alive");
                        o.energy -= retreat_cost;
                        heat(retreat_cost);
                        defender_energy_paid = retreat_cost;
                        response = CombatResponse::Retreated;
                        apex_encounters.retreats_total += 1;
                        if let Some(n) = neighbors
                            .lists
                            .get(defender.slot as usize)
                            .and_then(|list| list.iter().find(|n| n.id == attacker))
                            && let Some(d) = decisions.iter_mut().find(|(id, _)| *id == defender)
                        {
                            // A *request* to face away from the attacker. It is resolved,
                            // paid for and bounded by the same envelope as every other
                            // motion in section 6; a retreat is not a free instant turn.
                            d.1.heading = (organisms
                                .get(defender)
                                .expect("defender")
                                .pos
                                .chart()
                                - n.local)
                                .normalized()
                                .unwrap_or(d.1.heading);
                            d.1.turn_rate_max =
                                d.1.turn_rate_max.max(profile.escape_turn_rate_deg.to_radians());
                            boosts.push((
                                defender,
                                profile.escape_speed_multiple
                                    * organisms.get(defender).expect("defender").phenotype.speed_max,
                            ));
                        }
                    } else {
                        let o = organisms.get_mut(defender).expect("defender remained alive");
                        defender_injury = defend_injury.min(o.structure);
                        o.structure -= defender_injury;
                        fields.d[cell_of(&o.pos).index()] += defender_injury;
                        if retaliates {
                            o.energy -= profile.strike_energy_cost;
                            heat(profile.strike_energy_cost);
                            defender_energy_paid = profile.strike_energy_cost;
                            response = CombatResponse::Retaliated;
                            apex_encounters.retaliations_total += 1;
                            let o = organisms.get_mut(attacker).expect("attacker remained alive");
                            attacker_injury = attack_injury.min(o.structure);
                            o.structure -= attacker_injury;
                            fields.d[cell_of(&o.pos).index()] += attacker_injury;
                        } else {
                            defender_energy_paid = 0.0;
                            response = CombatResponse::Injured;
                        }
                    }
                    hunters.members[attacker_index].enter(
                        HunterPhase::Recovering,
                        now,
                        now + recovery_ticks,
                        0,
                    );
                    hunters.members[defender_index].enter(
                        HunterPhase::Recovering,
                        now,
                        now + recovery_ticks,
                        0,
                    );
                    apex_encounters.combats_total += 1;
                    apex_encounters.injury_material_total += attacker_injury + defender_injury;
                    apex_encounter_events.push(ApexEncounterEvent::Combat {
                        tick: now + 1,
                        attacker,
                        defender,
                        response,
                        attacker_energy_paid: profile.strike_energy_cost,
                        defender_energy_paid,
                        attacker_injury,
                        defender_injury,
                    });
                    used.extend([a_id, b_id]);
                }
            }
            if !hunters.members.is_empty()
                && let Some(profile) = hunters.profile.as_ref()
            {
                let windup_ticks = ticks_from_seconds(profile.windup_seconds, dt).max(1);
                let strike_ticks = ticks_from_seconds(profile.strike_seconds, dt).max(1);
                let _recovery_ticks = ticks_from_seconds(profile.recovery_seconds, dt).max(1);
                let meal_ticks = ticks_from_seconds(profile.meal_recovery_seconds, dt).max(1);
                let stalk_ticks = ticks_from_seconds(profile.stalk_timeout_seconds, dt).max(1);
                let mut charges: Vec<(OrganismId, f64)> = Vec::new();
                let mut threats: Vec<(OrganismId, OrganismId)> = Vec::new();

                for index in 0..hunters.members.len() {
                    // The record is `Copy`: the whole phase machine runs on this copy, so the
                    // arena and the member list can be read freely, and only the finished
                    // member is written back.
                    let mut m = hunters.members[index];
                    if apex_dormancy_on && apex_dormancy.contains(m.id) {
                        continue;
                    }
                    let Some(o) = organisms.get(m.id) else {
                        continue;
                    };
                    let headroom = profile.gut_capacity_material - m.gut_material;
                    let empty: &[pairs::Neighbor] = &[];
                    let sensed = neighbors
                        .lists
                        .get(m.id.slot as usize)
                        .map_or(empty, |l| l.as_slice());
                    let eligible = |id: OrganismId| -> bool {
                        !hunters.contains(id)
                            && organisms.get(id).is_some_and(|prey| {
                                hunter::prey_is_eligible(profile, o, prey, headroom, e_r)
                            })
                    };

                    // A target that died, had its slot reused, became a hunter, grew out of
                    // the window or no longer fits the gut is dropped deterministically. A
                    // strike that has already been paid for keeps its (now empty) handle and
                    // resolves as a failed attempt after movement.
                    if let Some(t) = m.target
                        && !eligible(t)
                    {
                        m.target = None;
                        if matches!(m.phase, HunterPhase::Stalking | HunterPhase::Windup) {
                            m.enter(HunterPhase::Perched, now, now, 0);
                        }
                    }
                    // Losing local sensing of the target ends a stalk or a windup too.
                    if matches!(m.phase, HunterPhase::Stalking | HunterPhase::Windup)
                        && m.target.is_some_and(|t| !sensed.iter().any(|n| n.id == t))
                    {
                        m.enter(HunterPhase::Perched, now, now, 0);
                    }

                    // Timed phases expire; a finished meal becomes a pause.
                    match m.phase {
                        HunterPhase::Handling if !m.carrying() => {
                            // The meal is over: the recoil that follows is a *finished meal*,
                            // which `entered_from` now records for the adapter.
                            let episode = m.episode;
                            m.enter(HunterPhase::Recovering, now, now + meal_ticks, episode);
                        }
                        HunterPhase::Recovering if now >= m.phase_ends_tick => {
                            m.enter(HunterPhase::Perched, now, now, 0);
                        }
                        _ => {}
                    }

                    // Satiety and a carried meal both end a hunt: the hysteresis is between
                    // `seek_reserve_fraction` and `perch_reserve_fraction` of `R_max`.
                    let full = o.reserve > profile.perch_reserve_fraction * o.phenotype.reserve_max;
                    if m.phase.hunting() && (m.carrying() || full) {
                        m.enter(HunterPhase::Perched, now, now, 0);
                    }

                    let hungry =
                        o.reserve < profile.seek_reserve_fraction * o.phenotype.reserve_max;
                    let may_hunt = profile.attacks_enabled && !m.carrying() && hungry;
                    // The nearest eligible prey this hunter actually senses; the neighbour
                    // list is already sorted by `(distance, id)`, so this is "nearest, ties by
                    // full ID" and never a global population scan.
                    let nearest = || sensed.iter().find(|n| eligible(n.id)).map(|n| n.id);
                    // The geometry this member hunts with right now, at its own body scale.
                    let geometry = hunter::ContactGeometry::of(profile, o);
                    // How far from the grasp centre a prey may be and still be worth cocking
                    // at: the tolerance plus the gap the paid strike can close.
                    let admission = |prey: &Organism| -> Option<hunter::ContactMeasure> {
                        hunter::measure_contact(
                            images,
                            o.pos,
                            o.heading,
                            &geometry,
                            prey.pos,
                            prey.phenotype.extent,
                        )
                    };
                    let closing = strike_closing_px(profile);

                    match m.phase {
                        HunterPhase::Perched => {
                            if may_hunt && let Some(t) = nearest() {
                                m.enter(HunterPhase::Stalking, now, now, 0);
                                m.target = Some(t);
                            }
                        }
                        HunterPhase::Stalking => {
                            if now.saturating_sub(m.phase_started_tick) >= stalk_ticks {
                                m.enter(HunterPhase::Perched, now, now, 0);
                            } else if m.target.is_none() {
                                match nearest() {
                                    Some(t) => m.target = Some(t),
                                    None => m.enter(HunterPhase::Perched, now, now, 0),
                                }
                            }
                            // The gesture starts only when the prey is inside the grasp the
                            // paid strike can actually close on — measured from the hunter
                            // root in the renderer's own body basis, never from a separately
                            // transported anchor's chart.
                            if m.phase == HunterPhase::Stalking
                                && let Some(t) = m.target
                                && let Some(prey) = organisms.get(t)
                                && admission(prey)
                                    .is_some_and(|c| c.effector_distance <= c.tolerance + closing)
                            {
                                m.enter(HunterPhase::Windup, now, now + windup_ticks, 0);
                                m.target = Some(t);
                            }
                        }
                        HunterPhase::Windup if now >= m.phase_ends_tick => {
                            // The whole strike cost must be available *before* attempting.
                            if let Some(t) = m.target
                                && o.energy >= profile.strike_energy_cost
                            {
                                charges.push((m.id, profile.strike_energy_cost));
                                m.attack_counter += 1;
                                hunters.attacks_total += 1;
                                counters.hunter_attacks += 1;
                                // The episode key: the counter this attempt's draws and its
                                // events all carry, after the increment that opened it.
                                let episode = m.attack_counter;
                                m.enter(HunterPhase::Strike, now, now + strike_ticks, episode);
                                m.target = Some(t);
                            } else {
                                // Refused before payment: no energy, no draw, no attempt.
                                hunter_events.push(HunterEvent::Attempt {
                                    tick: now + 1,
                                    hunter: m.id,
                                    target: m.target,
                                    outcome: hunter::AttemptOutcome::Unaffordable,
                                    energy_paid: 0.0,
                                    attack_counter: None,
                                    evidence: m
                                        .target
                                        .and_then(|t| organisms.get(t).map(|prey| (t, prey)))
                                        .map(|(t, prey)| {
                                            hunter::ContactEvidence::gather(
                                                images, profile, o, t, prey,
                                            )
                                        }),
                                });
                                m.enter(HunterPhase::Perched, now, now, 0);
                            }
                        }
                        _ => {}
                    }

                    // The intent: face the target, close the distance while the grasp is still
                    // ahead of the prey, hold while cocking, and burst during the strike.
                    // Steering uses the neighbour list's own unfolded position.
                    if let Some(t) = m.target
                        && m.phase.hunting()
                        && let Some(n) = sensed.iter().find(|n| n.id == t)
                        && let Some(toward) = (n.local - o.pos.chart()).normalized()
                    {
                        // The pursuit stopping distance, reconciled with the effector rather
                        // than left implicit: a prey already *inside* the reach envelope is not
                        // approached further — walking onto it would put it behind the claws.
                        let inside = organisms.get(t).and_then(admission).is_some_and(|c| {
                            c.body.x < geometry.capture_offset_body.x - c.tolerance - closing
                        });
                        let hold = inside || m.phase == HunterPhase::Windup;
                        if let Some(d) = decisions.iter_mut().find(|(id, _)| *id == m.id) {
                            // The intent to face the target, **not** the heading it ends the
                            // tick with. This late override used to write the heading
                            // directly, so a member could spin to any bearing in one tick
                            // however large its body was; section 6 now turns it by what its
                            // own geometry and energy allow.
                            d.1.heading = toward;
                            d.1.effort = if hold {
                                f64::from(o.phenotype.drives.rest_effort)
                            } else {
                                1.0
                            };
                            if m.phase == HunterPhase::Strike && !inside {
                                boosts.push((m.id, profile.strike_speed_px_s));
                            }
                        }
                        threats.push((t, m.id));
                    }
                    // A member never grazes and never eats fruit; detritus scavenging is the
                    // profile's explicit allocation of its handling capacity, and only with no
                    // meal and no hunt in progress. Budding is the profile's own gate, applied
                    // in the physiology pass, so the controller's request never stands.
                    if let Some(d) = decisions.iter_mut().find(|(id, _)| *id == m.id) {
                        let may_scavenge = profile.scavenge_fraction > 0.0
                            && !m.carrying()
                            && m.target.is_none()
                            && !m.phase.hunting();
                        d.1.fruit_effort = 0.0;
                        d.1.graze_effort = 0.0;
                        d.1.scavenge_effort = if may_scavenge {
                            d.1.scavenge_effort * profile.scavenge_fraction
                        } else {
                            0.0
                        };
                        d.1.bud = false;
                        if !m.phase.hunting() && d.1.scavenge_effort <= 0.0 {
                            // Perched, recovering or handling with nothing to forage: rest in
                            // place rather than wander at seeking effort.
                            d.1.mode = Mode::Resting;
                            d.1.effort = f64::from(o.phenotype.drives.rest_effort);
                        }
                    }
                    hunters.members[index] = m;
                }

                // The strike cost, charged in full at entry, before any outcome is known.
                for (id, cost) in charges {
                    if let Some(o) = organisms.get_mut(id) {
                        let paid = cost.min(o.energy).max(0.0);
                        o.energy -= paid;
                        heat(paid);
                    }
                }

                // 5c. Escape: only prey that is actually being pursued and actually senses
                //     its pursuer turns away, within a documented escape turn limit, and may
                //     briefly ask for more speed than its own maximum.
                let escape_turn = profile.escape_turn_rate_deg.to_radians() * dt;
                for (prey_id, hunter_id) in threats {
                    let Some(prey) = organisms.get(prey_id) else {
                        continue;
                    };
                    let Some(list) = neighbors.lists.get(prey_id.slot as usize) else {
                        continue;
                    };
                    let Some(n) = list.iter().find(|n| n.id == hunter_id) else {
                        continue;
                    };
                    let Some(d) = decisions.iter_mut().find(|(id, _)| *id == prey_id) else {
                        continue;
                    };
                    // Away from the sensed pursuer, at the profile's escape angular ceiling
                    // rather than the prey's ordinary one. Still only a request: section 6
                    // decides how much of it the body and its energy actually deliver.
                    d.1.heading = turn_toward(d.1.heading, prey.pos.chart() - n.local, escape_turn);
                    d.1.turn_rate_max = d.1.turn_rate_max.max(escape_turn / dt);
                    boosts.push((
                        prey_id,
                        profile.escape_speed_multiple * prey.phenotype.speed_max,
                    ));
                }
            }

            // 6. Resolve every motor request, move, transport tangents, and pay for the
            //    motion, the sensing and the maintenance.
            //
            //    This is the single boundary of `crate::motor` (milestone R0a): whatever the
            //    controller, apex pursuit, escape or an encounter retreat asked for above is
            //    an *intent*, and nothing downstream of here may assign a heading. The body's
            //    own radius, its angular ceiling and the movement energy left after upkeep
            //    decide how much of the intent is delivered, under `|v| + r · |ω| ≤ u`.
            //
            //    Payment order matches the physiology the world already used: unavoidable
            //    upkeep (`maintenance · S + sense_cost · r_sense`) is reserved first, and only
            //    the remainder buys motion. Translation and turning are charged exactly once
            //    each, through one term at the world's existing `move_cost`, with rotation
            //    priced at `motor::ROTATION_COST_SCALE` — so a body that only translates pays
            //    the pre-R0a bill to the bit, and a body with no movement energy left now
            //    holds still instead of moving for free.
            moved.resize_with(organisms.slot_count(), Vec::new);
            for segments in moved.iter_mut() {
                segments.clear();
            }
            for (id, d) in &decisions {
                // The apex contact geometry, read before the mutable borrow: a member's claws
                // reach well past its lobes and are the part of it that actually sweeps.
                let apex_geometry = hunters.profile.as_ref().filter(|_| hunters.contains(*id)).and_then(
                    |profile| {
                        organisms
                            .get(*id)
                            .map(|o| hunter::ContactGeometry::of(profile, o))
                    },
                );
                let Some(o) = organisms.get_mut(*id) else {
                    continue;
                };
                o.mode = d.mode;
                o.hunger_memory = d.hunger_memory;
                o.fed_this_tick = false;
                // Wading (`design/water.md`, `design/fauna-v2.md`): speed is divided by
                // `1 + w · (1 − swim)` of the cell the organism stands in before it moves; a
                // swimmer ignores the pool.
                let wading = 1.0 + fields.w[cell_of(&o.pos).index()] * (1.0 - o.phenotype.swim);
                let mut speed_cap = d.effort * o.phenotype.speed_max / wading;
                // A hunter's burst and a threatened prey's dash are the only boosts, and both
                // raise the translation ceiling only; the envelope and the energy still bind.
                // The list is empty in every world without hunters.
                if let Some((_, wanted)) = boosts.iter().find(|(b, _)| *b == *id) {
                    speed_cap = speed_cap.max(wanted / wading);
                }
                let bill = MotorBill::of(o, cfg);
                let limits = MotorLimits {
                    radius_px: motor::turn_radius_px(o, apex_geometry.as_ref()),
                    turn_rate_max: d.turn_rate_max,
                    speed_cap,
                    motor_budget: bill.affordable_motor(o.energy, dt),
                    dt,
                };
                let request = MotorRequest { heading: d.heading, speed: speed_cap };
                let motion = motor::resolve(o.heading, &request, &limits);
                travel_into(o.pos, motion.heading * (motion.speed * dt), travel_buf);
                o.pos = travel_buf.end;
                // Transport is a change of chart, applied to the *resolved* heading: it costs
                // nothing and consumes no turn budget.
                o.heading = travel_buf
                    .map
                    .apply(motion.heading)
                    .normalized()
                    .unwrap_or(motion.heading);
                o.ou = travel_buf.map.apply(d.ou);
                counters.travel_ties += travel_buf.ties;
                counters.travel_fallbacks += u32::from(travel_buf.fallback);
                if let Some(segments) = moved.get_mut(id.slot as usize) {
                    segments.extend_from_slice(&travel_buf.segments);
                }
                // The bill is now affordable by construction; `min` stays as a guard against
                // floating-point overshoot, not as the mechanism that lets a body move broke.
                let cost = bill.total_cost(motion.speed, motion.sweep, dt);
                let paid = cost.min(o.energy).max(0.0);
                o.energy -= paid;
                heat(paid);
            }

            // 6b. Capture settlement, from the common post-movement state and before any
            //     field feeding or physiology: every paid attempt that ends this tick is
            //     resolved once, at most one hunter claims each prey, and a claimed prey is
            //     removed exactly once with exactly one death event. Losing contenders paid
            //     at strike entry and are not refunded.
            if !hunters.members.is_empty()
                && let Some(profile) = hunters.profile.clone()
            {
                let recovery_ticks = ticks_from_seconds(profile.recovery_seconds, dt).max(1);
                // Attempt priority is its own seeded draw, so a contested prey is not decided
                // by slot order; the full ID breaks a tie. Each attempt carries the target it
                // was made against, so a settlement earlier in this loop cannot turn a
                // contender's claim into "the target was lost".
                let mut attempts: Vec<(u64, OrganismId, usize, Option<OrganismId>)> = Vec::new();
                for (index, m) in hunters.members.iter().enumerate() {
                    if m.phase == HunterPhase::Strike && now + 1 >= m.phase_ends_tick {
                        let key = hunter::draw_key(m.id);
                        let counter = hunter::priority_counter(m.attack_counter);
                        attempts.push((
                            draw(seed, Stream::Hunt, key, counter),
                            m.id,
                            index,
                            m.target,
                        ));
                    }
                }
                attempts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

                let mut claimed: Vec<OrganismId> = Vec::new();
                for (_, hunter_id, index, aimed_at) in attempts {
                    let m = hunters.members[index];
                    let Some(hunter_o) = organisms.get(hunter_id) else {
                        continue;
                    };
                    let headroom = profile.gut_capacity_material - m.gut_material;
                    let mut caught: Option<(OrganismId, f64, f64)> = None;
                    // The settlement evidence is gathered here, from the common post-movement
                    // state, **before** anything is removed or any target is cleared: once the
                    // prey is gone no later view can recover where it stood.
                    let mut evidence: Option<hunter::ContactEvidence> = None;
                    let outcome = match aimed_at {
                        None => AttemptOutcome::TargetLost,
                        Some(t) if claimed.contains(&t) => {
                            // A contender that lost the claim still records what it saw.
                            if let Some(prey) = organisms.get(t) {
                                evidence = Some(hunter::ContactEvidence::gather(
                                    images, &profile, hunter_o, t, prey,
                                ));
                            }
                            AttemptOutcome::TargetClaimed
                        }
                        Some(t) => match organisms.get(t) {
                            // A stale handle has no prey position: it is never filled in from
                            // whoever reused the slot.
                            None => AttemptOutcome::TargetLost,
                            Some(prey) => {
                                let seen = hunter::ContactEvidence::gather(
                                    images, &profile, hunter_o, t, prey,
                                );
                                evidence = Some(seen);
                                if hunters.contains(t)
                                    || !hunter::prey_is_eligible(
                                        &profile, hunter_o, prey, headroom, e_r,
                                    )
                                {
                                    AttemptOutcome::Ineligible
                                } else if !seen.measure.is_some_and(|c| c.in_contact()) {
                                    // Contact is re-evaluated here, from the hunter root in the
                                    // renderer's body basis, after *both* creatures moved.
                                    AttemptOutcome::OutOfReach
                                } else if seen.capture_center.is_none() {
                                    // In reach of a grasp that is not on the surface where the
                                    // artwork draws it: off the open rim, or a vertex whose
                                    // images disagree. The strike is paid; no capture is made
                                    // from a point that cannot be drawn (the safe first policy
                                    // of the animation contract).
                                    AttemptOutcome::GraspUnmapped
                                } else {
                                    let chance = hunter::capture_probability(
                                        &profile,
                                        hunter_o.structure,
                                        prey.structure,
                                    );
                                    let roll = unit(
                                        seed,
                                        Stream::Hunt,
                                        hunter::draw_key(hunter_id),
                                        hunter::capture_counter(m.attack_counter),
                                    );
                                    if roll < chance {
                                        let (material, energy) = hunter::prey_inventory(prey, e_r);
                                        caught = Some((t, material, energy));
                                        claimed.push(t);
                                        AttemptOutcome::Captured
                                    } else {
                                        AttemptOutcome::Missed
                                    }
                                }
                            }
                        },
                    };
                    hunter_events.push(HunterEvent::Attempt {
                        tick: now + 1,
                        hunter: hunter_id,
                        target: aimed_at,
                        outcome,
                        energy_paid: profile.strike_energy_cost,
                        // The paid attempt's key: this member's counter after the increment
                        // that opened the attempt, the same one its draws used.
                        attack_counter: Some(m.attack_counter),
                        evidence,
                    });

                    match caught {
                        Some((prey_id, material, energy)) => {
                            // One removal, one death event, and no detritus: the whole body
                            // and its escrow moved into the gut, energy included. This is an
                            // internal transfer with no source ledger and no detritus cap.
                            let prey = organisms.remove(prey_id).expect("the claim was checked");
                            // Every other member invalidates the handle in the same breath, so
                            // no unpaid hunt can chase a body that left the arena. A paid
                            // contender remains in Strike and settles from the attempt snapshot.
                            hunters.forget_target(prey_id, now + 1);
                            let member = &mut hunters.members[index];
                            member.gut_material += material;
                            member.gut_energy += energy;
                            // Settlement happens *after* movement, so the phase it opens starts
                            // at the boundary this tick completes — the same `now + 1` the
                            // capture record is stamped with. Entering at `now` would publish a
                            // recoil that began before the contact it recoils from.
                            member.enter(HunterPhase::Handling, now + 1, now + 1, m.attack_counter);
                            hunters.captures_total += 1;
                            hunters.predation_deaths_total += 1;
                            counters.hunter_captures += 1;
                            counters.deaths_predation += 1;
                            events.push(LifeEvent::Death {
                                tick: now + 1,
                                id: prey_id,
                                age_ticks: prey.age_ticks(now + 1),
                                cause: DeathCause::Predation,
                                births: prey.births,
                                genome: prey.genome.digest(),
                            });
                            hunter_events.push(HunterEvent::Capture {
                                tick: now + 1,
                                hunter: hunter_id,
                                prey: prey_id,
                                material,
                                energy,
                                attack_counter: m.attack_counter,
                                evidence: evidence.expect("a capture always measured its prey"),
                            });
                        }
                        None => {
                            // The recoil that follows came from a fully extended strike, which
                            // `entered_from` and `episode` now say out loud.
                            hunters.members[index].enter(
                                HunterPhase::Recovering,
                                now + 1,
                                now + 1 + recovery_ticks,
                                m.attack_counter,
                            );
                        }
                    }
                }
            }

            // 7. Settle feeding once per cell, proportionally, from the pre-transfer fields.
            let mut fruit = vec![0.0f64; CELL_COUNT];
            let mut graze = vec![0.0f64; CELL_COUNT];
            let mut scavenge = vec![0.0f64; CELL_COUNT];
            let mut detritus_energy_density = vec![0.0f64; CELL_COUNT];
            let mut requests: Vec<(usize, OrganismId, f64, f64, f64)> = Vec::new();
            for (id, d) in &decisions {
                if d.fruit_effort <= 0.0 && d.graze_effort <= 0.0 && d.scavenge_effort <= 0.0 {
                    continue;
                }
                let Some(o) = organisms.get(*id) else {
                    continue;
                };
                let cell = cell_of(&o.pos).index();
                let headroom = (o.phenotype.reserve_max - o.reserve).max(0.0);
                // Type-II intake: what a mouth can take falls off as the cell empties, so a
                // poor cell is poor food even to an organism standing in it. `K_P = 0` gives
                // back the linear law exactly. The rate is the diet's: `graze_rate` on
                // producer and fruit, `scavenge_rate` on detritus (`design/fauna-v2.md`).
                let bite = |rate: f64, effort: f64, room: f64, food: f64| {
                    if effort <= 0.0 {
                        return 0.0;
                    }
                    let total = food + k_p;
                    let saturation = if total > 0.0 { food / total } else { 0.0 };
                    (rate * effort * dt * saturation).clamp(0.0, room.max(0.0))
                };
                // Fruit settles first and takes its headroom, grazing next, scavenging
                // gets the rest, so intake alone can never push the reserve past `R_max`.
                // All read the cell's pre-settlement stock.
                let f = bite(
                    o.phenotype.graze_rate,
                    d.fruit_effort,
                    headroom,
                    fields.f[cell],
                );
                let g = bite(
                    o.phenotype.graze_rate,
                    d.graze_effort,
                    headroom - f,
                    fields.p[cell],
                );
                let s = bite(
                    o.phenotype.scavenge_rate,
                    d.scavenge_effort,
                    headroom - f - g,
                    edible_detritus(fields.d[cell], fields.de[cell], e_r),
                );
                intake.request_ticks += 1;
                if f <= 0.0 && g <= 0.0 && s <= 0.0 {
                    // Asked, got nothing. A full reserve is the one refusal the organism is
                    // carrying rather than the cell: name it, so an empty patch and a full
                    // animal are not reported as the same outcome.
                    intake.reserve_saturated_ticks += u64::from(headroom <= 0.0);
                    continue;
                }
                intake.requested += f + g + s;
                fruit[cell] += f;
                graze[cell] += g;
                scavenge[cell] += s;
                requests.push((cell, *id, f, g, s));
            }
            // Turn the per-cell request sums into proportional shares, and capture the
            // detritus energy density, all before a single transfer is applied.
            let mut contested: Vec<usize> = requests.iter().map(|r| r.0).collect();
            contested.sort_unstable();
            contested.dedup();
            for &cell in &contested {
                let (available_f, available_p, available_d) =
                    (fields.f[cell], fields.p[cell], fields.d[cell]);
                fruit[cell] = share(fruit[cell], available_f);
                graze[cell] = share(graze[cell], available_p);
                scavenge[cell] = share(scavenge[cell], available_d);
                detritus_energy_density[cell] = if available_d > 0.0 {
                    fields.de[cell] / available_d
                } else {
                    0.0
                };
            }

            let eta_m = org_cfg.assimilation_material;
            let eta_e = org_cfg.assimilation_energy;
            let e_p = cfg.producer.energy_density;
            for &(cell, id, f, g, s) in &requests {
                let Some(o) = organisms.get_mut(id) else {
                    continue;
                };
                let mut eaten = 0.0;
                if f > 0.0 {
                    // Frugivory: F -> reserve (η_m) and F -> D (the rest, energy-free). Fruit
                    // carries `e_f` per unit, richer than leaf.
                    let q = (f * fruit[cell]).clamp(0.0, fields.f[cell]);
                    if q > 0.0 {
                        intake.fruit_eaten += q;
                        let to_reserve = eta_m * q;
                        fields.f[cell] -= q;
                        fields.d[cell] += q - to_reserve;
                        o.reserve += to_reserve;
                        let spare = (e_f - e_r * eta_m) * q;
                        let room = (o.phenotype.energy_max - o.energy).max(0.0);
                        let gained = (eta_e * spare).clamp(0.0, room);
                        o.energy += gained;
                        heat(spare - gained);
                        eaten += q;
                    }
                }
                if g > 0.0 {
                    // Grazing: P -> reserve (η_m) and P -> D (the rest, energy-free).
                    let q = (g * graze[cell]).clamp(0.0, fields.p[cell]);
                    if q > 0.0 {
                        intake.producer_eaten += q;
                        let to_reserve = eta_m * q;
                        fields.p[cell] -= q;
                        fields.d[cell] += q - to_reserve;
                        o.reserve += to_reserve;
                        // The food carried `e_p · q`; `e_r · η_m · q` of it is now stored in
                        // the reserve, and `η_e` of the difference is usable energy.
                        let spare = (e_p - e_r * eta_m) * q;
                        let room = (o.phenotype.energy_max - o.energy).max(0.0);
                        let gained = (eta_e * spare).clamp(0.0, room);
                        o.energy += gained;
                        heat(spare - gained);
                        eaten += q;
                    }
                }
                if s > 0.0 {
                    // Scavenging: poor detritus assimilates proportionally less material,
                    // so the reserve is never credited with energy the food did not hold.
                    let rho = detritus_energy_density[cell];
                    let eta = if e_r > 0.0 {
                        eta_m * (rho / e_r).min(1.0)
                    } else {
                        eta_m
                    };
                    let q = (s * scavenge[cell]).clamp(0.0, fields.d[cell]);
                    if q > 0.0 && eta > 0.0 {
                        let to_reserve = eta * q;
                        // Scavenging removes only what it assimilates: the material that
                        // actually left `D` is `to_reserve`, not the bite `q` that was
                        // requested against it. Reporting `q` would overstate the flow out of
                        // a poor-quality patch.
                        intake.detritus_eaten += to_reserve;
                        fields.d[cell] -= to_reserve;
                        o.reserve += to_reserve;
                        let carried = (rho * q).min(fields.de[cell]);
                        fields.de[cell] -= carried;
                        let spare = carried - e_r * to_reserve;
                        let room = (o.phenotype.energy_max - o.energy).max(0.0);
                        let gained = (eta_e * spare).clamp(0.0, room);
                        o.energy += gained;
                        heat(spare - gained);
                        eaten += q;
                    }
                }
                o.fed_this_tick = eaten > 0.0;
            }

            // 7b. Handling and digestion. A carried carcass is homogeneous: a portion `q`
            //     leaves the gut with exactly the energy it was carrying, stores what its
            //     density pays for, rejects the rest as energy-free detritus, and sends the
            //     spare energy to the battery or to heat. Handling is paid first and in full,
            //     or nothing is digested this tick.
            if !hunters.members.is_empty()
                && let Some(profile) = hunters.profile.as_ref()
            {
                let metabolism = hunter::Metabolism::of(cfg);
                let handling = profile.handling_cost_per_second * dt;
                let meal_ticks = ticks_from_seconds(profile.meal_recovery_seconds, dt).max(1);
                for index in 0..hunters.members.len() {
                    let m = hunters.members[index];
                    if apex_dormancy_on && apex_dormancy.contains(m.id) {
                        continue;
                    }
                    if !m.carrying() {
                        continue;
                    }
                    let Some(o) = organisms.get_mut(m.id) else {
                        continue;
                    };
                    let paid = handling.min(o.energy).max(0.0);
                    o.energy -= paid;
                    heat(paid);
                    if paid < handling {
                        // It could not carry its meal this tick; the gut keeps everything and
                        // ordinary oxidation may refill the battery for the next one.
                        continue;
                    }
                    let reserve_room = (o.phenotype.reserve_max - o.reserve).max(0.0);
                    let energy_room = (o.phenotype.energy_max - o.energy).max(0.0);
                    let step = hunter::digest_step(
                        profile,
                        dt,
                        m.gut_material,
                        m.gut_energy,
                        metabolism,
                        reserve_room,
                        energy_room,
                    );
                    if step.material > 0.0 {
                        o.reserve += step.to_reserve;
                        o.energy += step.energy_gain;
                        fields.d[cell_of(&o.pos).index()] += step.to_detritus;
                        heat(step.heat);
                        let member = &mut hunters.members[index];
                        member.gut_material -= step.material;
                        member.gut_energy -= step.carried;
                    }
                    // A finished meal ends exactly empty: the last few ulps of energy leave as
                    // heat rather than sitting in a gut with no material to carry them.
                    let member = &mut hunters.members[index];
                    if member.gut_material <= hunter::GUT_RESIDUE {
                        let residue = member.gut_energy;
                        member.gut_material = 0.0;
                        member.gut_energy = 0.0;
                        if residue > 0.0 {
                            heat(residue);
                        }
                        // The meal ended *this* tick, so the pause starts now: a member is
                        // never left handling nothing, not even for the rest of the tick.
                        if member.phase == HunterPhase::Handling {
                            let episode = member.episode;
                            // Digestion is a post-movement pass too: the pause starts at the
                            // boundary this tick completes, and runs its whole advertised span.
                            member.enter(
                                HunterPhase::Recovering,
                                now + 1,
                                now + 1 + meal_ticks,
                                episode,
                            );
                        }
                    }
                }
            }

            // 8. Physiology: oxidation, growth, gestation, budding, death checks.
            let gestation_ticks = ticks_from_seconds(org_cfg.gestation_seconds, dt);
            // A member gestates on its own, much longer clock and grows on its own, much
            // slower ceiling; every other creature keeps the world config's.
            let hunter_gestation_ticks = hunters
                .profile
                .as_ref()
                .map(|p| ticks_from_seconds(p.gestation_seconds, dt))
                .unwrap_or(gestation_ticks);
            let max_age_ticks = ticks_from_seconds(org_cfg.max_age_seconds, dt);
            let cap = cfg.capacity.max_organisms as usize;
            let population = organisms.len();
            let mut births: Vec<OrganismId> = Vec::new();
            let mut deaths: Vec<(OrganismId, DeathCause)> = Vec::new();

            // Dormancy is a paid lifecycle, not free storage. Count suitable juvenile prey at
            // the concealed organism's persisted surface location, charge the low upkeep first,
            // then recheck both the current abundance and post-payment reserves before waking.
            if apex_dormancy_on {
                let evaluations: Vec<(OrganismId, u32)> = apex_dormancy
                    .dormant
                    .iter()
                    .map(|d| {
                        let suitable = organisms.get(d.id).map_or(0, |buried| {
                            organisms
                                .iter()
                                .filter(|(prey_id, prey)| {
                                    !hunters.contains(*prey_id)
                                        && prey.structure < 0.7 * prey.phenotype.structure_adult
                                        && hunter::prey_is_eligible(
                                            hunters.profile.as_ref().expect(
                                                "validated active dormancy has a hunter profile",
                                            ),
                                            buried,
                                            prey,
                                            hunters
                                                .profile
                                                .as_ref()
                                                .expect("profile")
                                                .gut_capacity_material,
                                            e_r,
                                        )
                                        && hunter::surface_reach(
                                            images,
                                            buried.pos,
                                            prey.pos,
                                            PREY_RADIUS_PX,
                                        )
                                        .is_some()
                                })
                                .count()
                        });
                        (d.id, u32::try_from(suitable).unwrap_or(u32::MAX))
                    })
                    .collect();
                let mut emerged = Vec::new();
                for (id, suitable_prey) in evaluations {
                    let Some(index) = apex_dormancy.index_of(id) else {
                        continue;
                    };
                    let Some(o) = organisms.get_mut(id) else {
                        continue;
                    };
                    let cost = MAINTENANCE_PER_STRUCTURE_SECOND * o.structure * dt;
                    let paid = cost.min(o.energy).max(0.0);
                    o.energy -= paid;
                    heat(paid);
                    apex_dormancy.maintenance_energy_paid_total += paid;
                    let record = &mut apex_dormancy.dormant[index];
                    record.maintenance_energy_paid += paid;
                    record.suitable_prey_ticks = if suitable_prey >= PREY_REQUIRED {
                        record.suitable_prey_ticks.saturating_add(1)
                    } else {
                        0
                    };

                    if o.energy <= 0.0 {
                        deaths.push((id, DeathCause::Starvation));
                        apex_dormancy.exhausted_total += 1;
                        apex_dormancy_events.push(ApexDormancyEvent::Exhausted {
                            tick: now + 1,
                            id,
                            dormant_ticks: (now + 1).saturating_sub(record.entered_tick),
                            maintenance_energy_paid: record.maintenance_energy_paid,
                        });
                        continue;
                    }

                    if now + 1 >= record.next_check_tick {
                        let stocks_ready = o.reserve
                            >= EMERGENCE_RESERVE_FRACTION * o.phenotype.reserve_max
                            && o.energy >= EMERGENCE_ENERGY_FRACTION * o.phenotype.energy_max;
                        if record.suitable_prey_ticks >= SUSTAIN_TICKS && stocks_ready {
                            emerged.push((id, suitable_prey));
                        } else {
                            record.next_check_tick = (now + 1).saturating_add(RECHECK_TICKS);
                        }
                    }
                }
                for (id, suitable_prey) in emerged {
                    let record = apex_dormancy
                        .remove(id)
                        .expect("an emergence candidate is still dormant");
                    let dormant_ticks = (now + 1).saturating_sub(record.entered_tick);
                    if let Some(o) = organisms.get_mut(id) {
                        // Pause the active age clock exactly as long as development was paused.
                        o.born_tick = o.born_tick.saturating_add(dormant_ticks).min(now + 1);
                    }
                    apex_dormancy.emerged_total += 1;
                    apex_dormancy_events.push(ApexDormancyEvent::Emerged {
                        tick: now + 1,
                        id,
                        dormant_ticks,
                        suitable_prey,
                        maintenance_energy_paid: record.maintenance_energy_paid,
                    });
                }
            }
            for (id, d) in &decisions {
                let Some(o) = organisms.get_mut(*id) else {
                    continue;
                };
                // Membership, not `genome.form`, is what makes a predator.
                let member = hunters.index_of(*id);

                // *When* reserve is converted into battery charge is a **member policy**; what
                // that conversion does is not. An authoritative member carrying semantic
                // profile version 4 activates below a fixed fraction of `E_max` at every age
                // and phase; everything else — ordinary organisms, and members carrying
                // version 3 — uses the world's configured threshold, resolved through the same
                // accessor so the two cannot drift apart (`crate::hunter::OxidationPolicy`).
                let reference = org_cfg.oxidation_threshold;
                let threshold = match (member, hunters.profile.as_ref()) {
                    (Some(_), Some(profile)) => profile.oxidation_threshold(org_cfg),
                    _ => reference,
                };
                if o.energy < threshold * o.phenotype.energy_max && o.reserve > 0.0 {
                    // Decided *before* the transaction changes anything, so the test is the
                    // one a configured-threshold member would actually have failed — not a
                    // subtraction reconstructed afterwards from rounded values.
                    let above_reference = o.energy >= reference * o.phenotype.energy_max;
                    let burned = (org_cfg.oxidation_rate * dt).min(o.reserve);
                    o.reserve -= burned;
                    fields.n[cell_of(&o.pos).index()] += burned;
                    // The reserve material carried `e_r` per unit; `η_ox` of it becomes usable.
                    let released = e_r * burned;
                    let room = (o.phenotype.energy_max - o.energy).max(0.0);
                    let gained = (released * org_cfg.oxidation_efficiency).min(room);
                    o.energy += gained;
                    heat(released - gained);
                    // Bounded diagnostics, after the transaction and out of its way: this is
                    // the branch a member under the configured threshold would not have taken.
                    // Reads nothing new, writes no world state, consumes no draw
                    // (`ChargingDiagnostics`).
                    if above_reference && burned > 0.0 {
                        charging.extra_transactions += 1;
                        charging.extra_reserve_burned += burned;
                        charging.extra_energy_gained += gained;
                        charging.extra_heat += released - gained;
                    }
                }

                if o.structure < o.phenotype.structure_adult
                    && o.reserve > org_cfg.growth_reserve_min * o.phenotype.reserve_max
                {
                    let growth_rate = match (member, hunters.profile.as_ref()) {
                        (Some(_), Some(profile)) => profile.juvenile_growth_rate,
                        _ => org_cfg.growth_rate,
                    };
                    let mut grown = (growth_rate * dt)
                        .min(o.phenotype.structure_adult - o.structure)
                        .min(o.reserve);
                    // Building is paid for up front: what the energy cannot cover is not built.
                    if org_cfg.build_cost > 0.0 {
                        grown = grown.min(o.energy / org_cfg.build_cost);
                    }
                    if grown > 0.0 {
                        o.reserve -= grown;
                        o.structure += grown;
                        let cost = (org_cfg.build_cost * grown).min(o.energy);
                        o.energy -= cost;
                        // Structure holds no chemical energy: the reserve's energy is released.
                        heat(cost + e_r * grown);
                    }
                }

                let gestation = if member.is_some() {
                    hunter_gestation_ticks
                } else {
                    gestation_ticks
                };
                let due = o
                    .escrow
                    .as_ref()
                    .is_some_and(|e| now.saturating_sub(e.started_tick) >= gestation);
                // A hunter's one paid offspring is gated by its profile and its own local
                // state; the ordinary controller's `bud` never applies to a member.
                let bud = match (member, hunters.profile.as_ref()) {
                    (Some(index), Some(profile)) if !apex_encounters_on => {
                        hunter::may_reproduce(profile, o, &hunters.members[index], now, dt)
                    }
                    (Some(_), Some(_)) => false,
                    _ => d.bud,
                };
                if due {
                    births.push(*id);
                } else if bud && o.escrow.is_none() {
                    if population + births.len() < cap {
                        let structure =
                            org_cfg.child_structure_fraction * o.phenotype.structure_adult;
                        let reserve = org_cfg.child_reserve_fraction * o.phenotype.reserve_max;
                        let energy = org_cfg.child_energy_fraction * o.phenotype.energy_max;
                        let build = org_cfg.build_cost * structure;
                        if o.reserve >= structure + reserve && o.energy >= build + energy {
                            // Read on either side of the assignment that moves them: this is the
                            // transaction, not a post-step difference (`crate::hunter`).
                            let (reserve_before, energy_before) = (o.reserve, o.energy);
                            o.reserve -= structure + reserve;
                            o.energy -= build + energy;
                            heat(build);
                            let genome = o.genome.clone();
                            o.escrow = Some(Escrow {
                                structure,
                                reserve,
                                energy,
                                started_tick: now,
                                genome,
                            });
                            if member.is_some() {
                                hunter_events.push(HunterEvent::Reproduction {
                                    tick: now + 1,
                                    hunter: *id,
                                    record: hunter::Reproduction::Funded {
                                        key: hunter::EscrowKey {
                                            parent: *id,
                                            started_tick: now,
                                        },
                                        parent_reserve_before: reserve_before,
                                        parent_reserve_after: o.reserve,
                                        parent_energy_before: energy_before,
                                        parent_energy_after: o.energy,
                                        escrow_structure: structure,
                                        escrow_reserve: reserve,
                                        escrow_energy: energy,
                                        build_heat: build,
                                    },
                                });
                            }
                        } else if member.is_some() {
                            // Ready by its own gate, but the stocks could not cover the child:
                            // no escrow exists and nothing moved.
                            hunter_events.push(HunterEvent::Reproduction {
                                tick: now + 1,
                                hunter: *id,
                                record: hunter::Reproduction::NotFunded {
                                    parent: *id,
                                    reason: hunter::FundingBlocked::Stocks,
                                },
                            });
                        }
                    } else {
                        counters.cap_rejections += 1;
                        *cap_rejections_total += 1;
                        if member.is_some() {
                            // The cap refused the gestation before it began: no escrow was ever
                            // created, so there is no transaction to close later.
                            hunter_events.push(HunterEvent::Reproduction {
                                tick: now + 1,
                                hunter: *id,
                                record: hunter::Reproduction::NotFunded {
                                    parent: *id,
                                    reason: hunter::FundingBlocked::Cap,
                                },
                            });
                        }
                    }
                }

                let cause = if o.energy <= 0.0 && o.reserve <= 0.0 {
                    Some(DeathCause::Starvation)
                } else if o.age_ticks(now) >= max_age_ticks {
                    Some(DeathCause::Age)
                } else if o.structure < org_cfg.min_structure {
                    Some(DeathCause::Collapse)
                } else {
                    None
                };
                if let Some(cause) = cause {
                    deaths.push((*id, cause));
                    // A parent that dies this tick miscarries: the escrow goes to detritus.
                    births.retain(|b| b != id);
                }
            }

            // 9. Commit: remove the dead, then place births against the freed capacity.
            let e_d_max = cfg.detritus.energy_cap;
            for (id, cause) in &deaths {
                let Some(o) = organisms.remove(*id) else {
                    continue;
                };
                let cell = cell_of(&o.pos).index();
                // The body: structure carries no energy, the reserve carries `e_r` per unit.
                let material = o.structure + o.reserve;
                let energy = o.energy + e_r * o.reserve;
                fields.d[cell] += material;
                let kept = energy.min(e_d_max * material);
                fields.de[cell] += kept;
                heat(energy - kept);
                // A gestation that never finished decays with its own clamp.
                if let Some(es) = &o.escrow {
                    let paired = if apex_encounters_on {
                        apex_encounters.gestation(*id).copied()
                    } else {
                        None
                    };
                    let material = es.structure + es.reserve;
                    let energy = e_r * material + es.energy;
                    fields.d[cell] += material;
                    let kept = energy.min(e_d_max * material);
                    fields.de[cell] += kept;
                    heat(energy - kept);
                    // The escrow's own terms, kept apart from the body above and the gut below:
                    // a miscarriage is not the whole corpse.
                    if let Some(pair) = paired {
                        apex_encounters.miscarriages_total += 1;
                        apex_encounter_events.push(ApexEncounterEvent::Miscarried {
                            tick: now + 1,
                            carrier: pair.carrier,
                            partner: pair.partner,
                            cause: *cause,
                            material,
                            energy,
                            energy_stored: kept,
                            energy_heat: energy - kept,
                        });
                        let _ = apex_encounters.take_gestation(*id);
                    } else if hunters.contains(*id) {
                        hunter_events.push(HunterEvent::Reproduction {
                            tick: now + 1,
                            hunter: *id,
                            record: hunter::Reproduction::Miscarried {
                                key: hunter::EscrowKey {
                                    parent: *id,
                                    started_tick: es.started_tick,
                                },
                                cause: *cause,
                                material,
                                energy,
                                energy_stored: kept,
                                energy_heat: energy - kept,
                            },
                        });
                    }
                }
                let slot = match cause {
                    DeathCause::Starvation => 0,
                    DeathCause::Age => 1,
                    DeathCause::Collapse => 2,
                    // Predation never reaches this loop: a consumed prey is settled and
                    // removed by the hunter pass, which books it in the extension's own
                    // counter and leaves the three natural counters alone.
                    DeathCause::Predation => {
                        unreachable!("predation is settled by the hunter pass")
                    }
                };
                counters.deaths[slot] += 1;
                deaths_total[slot] += 1;
                // Whether this was prey or another hunter, any unpaid pursuit ends at the
                // removal boundary. A paid strike keeps its phase long enough to settle as a
                // lost target on its own boundary.
                hunters.forget_target(*id, now + 1);
                // A member's carried meal goes where its body went: into the cell's detritus,
                // keeping at most what the detritus cap allows and releasing the rest as heat.
                // Nothing a hunter was holding disappears because its record was removed.
                if let Some(gone) = hunters.remove_member(*id, now + 1) {
                    let mut stored = 0.0;
                    if gone.gut_material > 0.0 || gone.gut_energy > 0.0 {
                        fields.d[cell] += gone.gut_material;
                        stored = gone.gut_energy.min(e_d_max * gone.gut_material);
                        fields.de[cell] += stored;
                        heat(gone.gut_energy - stored);
                    }
                    hunters.hunter_deaths_total += 1;
                    hunter_events.push(HunterEvent::Death {
                        tick: now + 1,
                        id: *id,
                        cause: *cause,
                        gut_material: gone.gut_material,
                        gut_energy: gone.gut_energy,
                        gut_energy_stored: stored,
                    });
                }
                apex_dormancy.remove(*id);
                apex_encounters.remove_parentage(*id);
                events.push(LifeEvent::Death {
                    tick: now + 1,
                    id: *id,
                    age_ticks: o.age_ticks(now + 1),
                    cause: *cause,
                    births: o.births,
                    genome: o.genome.digest(),
                });
            }

            // Every removal path — starvation, age, collapse, a settled capture — has now
            // happened. A pause whose parent is gone is aborted here, once, before any birth
            // can add a new one (`crate::quiet`).
            if quiet_on && !quiet.pauses.is_empty() {
                let mut kept = Vec::with_capacity(quiet.pauses.len());
                for p in std::mem::take(&mut quiet.pauses) {
                    if organisms.get(p.parent).is_some() {
                        kept.push(p);
                    } else {
                        quiet_events.push(QuietEvent::Abort {
                            tick: now + 1,
                            parent: p.parent,
                            child: p.child,
                            completed_ticks: p.completed(now + 1),
                            reason: QuietReason::ParentGone,
                        });
                    }
                }
                quiet.pauses = kept;
            }

            for parent_id in &births {
                let full = organisms.len() >= cap;
                // A member's child inherits the lineage, not the appearance: membership is
                // granted explicitly below, and its genome is copied exactly.
                let hunter_parent = hunters.index_of(*parent_id);
                let paired = if apex_encounters_on {
                    apex_encounters.gestation(*parent_id).copied()
                } else {
                    None
                };
                if full && let Some(pair) = paired {
                    let Some(escrow) = organisms
                        .get_mut(*parent_id)
                        .and_then(|parent| parent.escrow.take())
                    else {
                        continue;
                    };
                    // Each surviving contributor receives its own inventory share. A partner
                    // that died after paying cannot safely be addressed; only that stale share
                    // falls back to the carrier. Build heat is already in the heat ledger.
                    {
                        let carrier = organisms
                            .get_mut(*parent_id)
                            .expect("a due paired carrier is alive");
                        carrier.reserve += pair.carrier_paid.material();
                        carrier.energy += pair.carrier_paid.energy;
                    }
                    let partner_refund_to = if organisms.get(pair.partner).is_some() {
                        let partner = organisms.get_mut(pair.partner).expect("checked live partner");
                        partner.reserve += pair.partner_paid.material();
                        partner.energy += pair.partner_paid.energy;
                        pair.partner
                    } else {
                        let carrier = organisms
                            .get_mut(*parent_id)
                            .expect("a due paired carrier is alive");
                        carrier.reserve += pair.partner_paid.material();
                        carrier.energy += pair.partner_paid.energy;
                        *parent_id
                    };
                    debug_assert!((escrow.structure
                        - pair.carrier_paid.structure
                        - pair.partner_paid.structure)
                        .abs()
                        < hunter::TOLERANCE);
                    debug_assert!((escrow.reserve
                        - pair.carrier_paid.reserve
                        - pair.partner_paid.reserve)
                        .abs()
                        < hunter::TOLERANCE);
                    debug_assert!((escrow.energy
                        - pair.carrier_paid.energy
                        - pair.partner_paid.energy)
                        .abs()
                        < hunter::TOLERANCE);
                    counters.cap_rejections += 1;
                    *cap_rejections_total += 1;
                    apex_encounters.refunds_total += 1;
                    let _ = apex_encounters.take_gestation(*parent_id);
                    apex_encounter_events.push(ApexEncounterEvent::Refunded {
                        tick: now + 1,
                        carrier: pair.carrier,
                        partner: pair.partner,
                        carrier_refund: pair.carrier_paid,
                        partner_refund: pair.partner_paid,
                        partner_refund_to,
                    });
                    continue;
                }
                let placement = {
                    let Some(parent) = organisms.get_mut(*parent_id) else {
                        continue;
                    };
                    let Some(escrow) = parent.escrow.take() else {
                        continue;
                    };
                    if full {
                        // A refused birth returns its escrow to the parent untouched.
                        let (reserve_before, energy_before) = (parent.reserve, parent.energy);
                        parent.reserve += escrow.structure + escrow.reserve;
                        parent.energy += escrow.energy;
                        counters.cap_rejections += 1;
                        *cap_rejections_total += 1;
                        if hunter_parent.is_some() && paired.is_none() {
                            // A refund, not a miscarriage: every unit went back where it came
                            // from, and nothing was burned or dropped.
                            hunter_events.push(HunterEvent::Reproduction {
                                tick: now + 1,
                                hunter: *parent_id,
                                record: hunter::Reproduction::Refunded {
                                    key: hunter::EscrowKey {
                                        parent: *parent_id,
                                        started_tick: escrow.started_tick,
                                    },
                                    refunded_structure: escrow.structure,
                                    refunded_reserve: escrow.reserve,
                                    refunded_energy: escrow.energy,
                                    parent_reserve_before: reserve_before,
                                    parent_reserve_after: parent.reserve,
                                    parent_energy_before: energy_before,
                                    parent_energy_after: parent.energy,
                                },
                            });
                        }
                        // …and the parent waits a gestation before trying again, so a world at
                        // its cap cannot spin a hunter through a free birth attempt per tick.
                        if let Some(index) = hunter_parent {
                            hunters.members[index].next_reproduction_tick =
                                now + 1 + hunter_gestation_ticks;
                        }
                        continue;
                    }
                    let index = u64::from(parent.births);
                    parent.births += 1;
                    let key = u64::from(parent_id.slot);
                    // Each birth owns a block of `BIRTH_DRAWS` counters: placement first,
                    // then mutation, so neither can collide with the next birth's draws.
                    let base = index * BIRTH_DRAWS;
                    let direction =
                        Vec2::from_screen_angle(unit(seed, Stream::Birth, key, base) * TAU);
                    let heading =
                        Vec2::from_screen_angle(unit(seed, Stream::Birth, key, base + 1) * TAU);
                    (
                        parent.pos,
                        direction,
                        heading,
                        escrow,
                        parent.age_ticks(now + 1),
                        parent.births,
                        key,
                        base + 2,
                    )
                };
                let (
                    from,
                    direction,
                    heading,
                    escrow,
                    parent_age_ticks,
                    parent_births,
                    key,
                    mut counter,
                ) = placement;
                travel_into(from, direction * cfg.drives.birth_offset_px, travel_buf);
                counters.travel_ties += travel_buf.ties;
                counters.travel_fallbacks += u32::from(travel_buf.fallback);
                let heading = travel_buf
                    .map
                    .apply(heading)
                    .normalized()
                    .unwrap_or(Vec2::new(1.0, 0.0));
                let mut genome = escrow.genome.clone();
                // Sparse mutation (`design/fauna-v2.md`): the draws follow the placement draws
                // in the parent's birth stream; `form` never changes; an exact copy records
                // nothing.
                // Hunter mutation is outside this experiment: a member's child carries the
                // profile's fixed genome exactly, even where ordinary prey mutate.
                let mutations = if cfg.mechanisms.mutation && hunter_parent.is_none() {
                    let draws = genome.mutate(cfg.mutation.probability, cfg.mutation.step, || {
                        let u = unit(seed, Stream::Birth, key, counter);
                        counter += 1;
                        u
                    });
                    genome.clamp();
                    draws
                } else {
                    Vec::new()
                };
                let mut phenotype = decode(&genome, org_cfg);
                if hunter_parent.is_some()
                    && let Some(profile) = hunters.profile.as_ref()
                {
                    // The assembled body's tested support, exactly as the founder got it.
                    phenotype.extent = profile.body_extent_px;
                }
                // The escrowed material that becomes structure gives up its reserve energy.
                heat(e_r * escrow.structure);
                let child = Organism {
                    pos: travel_buf.end,
                    heading,
                    ou: Vec2::ZERO,
                    structure: escrow.structure,
                    reserve: escrow.reserve,
                    energy: escrow.energy,
                    born_tick: now + 1,
                    hunger_memory: (1.0 - escrow.reserve / phenotype.reserve_max).clamp(0.0, 1.0),
                    mode: Mode::Resting,
                    escrow: None,
                    births: 0,
                    genome,
                    phenotype,
                    parent: Some(*parent_id),
                    origin: Origin::Descendant,
                    turn_counter: Counter::default(),
                    fed_this_tick: false,
                };
                let genome_digest = child.genome.digest();
                let origin = child.origin;
                let child_id = organisms.insert(child);
                if let Some(index) = hunter_parent {
                    // The funded descendant joins the lineage with no target, an empty gut and
                    // a fresh attack counter, and the parent starts its recovery interval.
                    hunters.insert_member(hunter::HunterMember::new(child_id, now + 1));
                    if apex_dormancy_on
                        && let Some(child) = organisms.get(child_id)
                        && let Some(event) =
                            apex_dormancy.admit(*parent_id, child_id, now + 1, child)
                    {
                        apex_dormancy_events.push(event);
                    }
                    hunters.hunter_births_total += 1;
                    let interval = hunters
                        .profile
                        .as_ref()
                        .map(|p| ticks_from_seconds(p.reproduce_interval_seconds, dt))
                        .unwrap_or(0);
                    // `insert_member` may have shifted the parent's row; find it again.
                    let _ = index;
                    if let Some(parent_index) = hunters.index_of(*parent_id) {
                        hunters.members[parent_index].next_reproduction_tick = now + 1 + interval;
                    }
                    if let Some(pair) = paired {
                        // Both identities remain attached to the living child. The ordinary
                        // life record keeps its established primary-parent shape; this event is
                        // the reconciling two-parent identity and inventory record.
                        apex_encounters.insert_parentage(PairedParentage {
                            child: child_id,
                            carrier: pair.carrier,
                            partner: pair.partner,
                        });
                        let _ = apex_encounters.take_gestation(*parent_id);
                        apex_encounters.births_total += 1;
                        if let Some(partner_index) = hunters.index_of(pair.partner) {
                            hunters.members[partner_index].next_reproduction_tick =
                                now + 1 + interval;
                        }
                        apex_encounter_events.push(ApexEncounterEvent::Born {
                            tick: now + 1,
                            carrier: pair.carrier,
                            partner: pair.partner,
                            child: child_id,
                            structure: escrow.structure,
                            reserve: escrow.reserve,
                            energy: escrow.energy,
                            birth_heat: e_r * escrow.structure,
                        });
                    } else {
                        hunter_events.push(HunterEvent::Offspring {
                            tick: now + 1,
                            parent: *parent_id,
                            child: child_id,
                        });
                        // The transaction beside the identity link: the child's actual opening
                        // inventory is the escrow's, and the structural material gave up its
                        // reserve energy as heat on the way.
                        hunter_events.push(HunterEvent::Reproduction {
                            tick: now + 1,
                            hunter: *parent_id,
                            record: hunter::Reproduction::Born {
                                key: hunter::EscrowKey {
                                    parent: *parent_id,
                                    started_tick: escrow.started_tick,
                                },
                                child: child_id,
                                child_structure: escrow.structure,
                                child_reserve: escrow.reserve,
                                child_energy: escrow.energy,
                                birth_heat: e_r * escrow.structure,
                            },
                        });
                    }
                }
                // The trigger is this successful core commit, not an observer's post-step
                // inference: the child is in the arena and the parent survived to see it.
                if quiet_on {
                    quiet_admit(
                        quiet,
                        quiet_events,
                        organisms,
                        org_cfg,
                        cap,
                        now + 1,
                        dt,
                        *parent_id,
                        child_id,
                        hunter_parent.is_some(),
                    );
                }
                events.push(LifeEvent::Birth {
                    tick: now + 1,
                    id: child_id,
                    parent: *parent_id,
                    parent_age_ticks,
                    parent_births,
                    genome: genome_digest,
                    origin,
                    mutations,
                });
                let slot = child_id.slot as usize;
                if moved.len() <= slot {
                    moved.resize_with(slot + 1, Vec::new);
                }
                moved[slot].clear();
                counters.births += 1;
                *births_total += 1;
            }

            *tick += 1;
        }

        // 10. Invariants; the render view and telemetry are pulled by the host.
        #[cfg(debug_assertions)]
        {
            if let Err(e) = self.check_invariants() {
                panic!("invariant violated after tick {}: {e}", self.state.tick);
            }
            // The energy audit is an exact identity: every joule is light, heat, stored, or
            // (with care) fed in or cleaned out. Feed and clean commit at a boundary, never
            // inside a step, so their terms are zero here; they are written out anyway so
            // the identity the contract states is the identity the code checks.
            //
            // The hunter founder and control imports are boundary commits for the same reason;
            // a capture, a digestion and a hunter's death are all internal transfers and move
            // no term of this identity except heat.
            let (before, opening, fed, cleaned, imported) = audit;
            let booked = self.state.energy_ledgers().net_since(opening)
                + (self.state.care.feed_energy_in - fed)
                - (self.state.care.clean_energy_out - cleaned)
                + (self.state.hunters.imported_energy() - imported);
            let drift = (stored_energy(&self.state) - before) - booked;
            assert!(
                drift.abs() < AUDIT_TOLERANCE,
                "energy audit drifted by {drift:e} in tick {}",
                self.state.tick
            );
            // The water budget is the same kind of identity: `Δ Σw == rain_in − evap_out`.
            let (w_before, rain, evap) = water_audit;
            let w_booked = (self.state.rain_in_total - rain) - (self.state.evap_out_total - evap);
            let w_drift = (self.state.fields.w.iter().sum::<f64>() - w_before) - w_booked;
            assert!(
                w_drift.abs() < AUDIT_TOLERANCE,
                "water budget drifted by {w_drift:e} in tick {}",
                self.state.tick
            );
        }
        &self.counters
    }

    /// Water budget residual (`design/water.md`): `Σw − (rain_in_total − evap_out_total)`.
    /// Zero to rounding for a world created dry; telemetry consumers check it like the
    /// mass residual.
    pub fn water_residual(&self) -> f64 {
        self.state.fields.w.iter().sum::<f64>()
            - (self.state.rain_in_total - self.state.evap_out_total)
    }

    /// Mass invariant: `Σ fields + Σ organisms (incl. escrow) − external_material_in
    /// − feed_material_in + clean_material_out − initial`. Should stay within
    /// `1e-9 · initial` per hour of simulated time; telemetry reports it. Fed crumbs are
    /// material admitted from outside and cleaned litter is material exported, exactly like
    /// the founders in `external_material_in`.
    ///
    /// With the hunter extension the same rule applies to it: a carried carcass
    /// (`Σ gut_material`) is material still inside the world, and the hunter founders and any
    /// budget-matched control deposit are material admitted from outside
    /// (`HunterState::imported_material`), booked once in the extension and never again in
    /// `external_material_in`.
    pub fn mass_residual(&self) -> f64 {
        let organisms: f64 = self.state.organisms.iter().map(|(_, o)| o.material()).sum();
        self.state.fields.total_material() + organisms + self.state.hunters.gut_material_total()
            - self.state.external_material_in
            - self.state.care.feed_material_in
            + self.state.care.clean_material_out
            - self.state.hunters.imported_material()
            - self.initial_material
    }

    /// The care ledgers, the sequence cursor, and any shower in progress.
    pub fn care(&self) -> &CareState {
        &self.state.care
    }

    /// The compensated cumulative energy ledgers
    /// ([`WorldState::energy_ledgers`], `crate::accounting`). Hold a reading before an
    /// interval and call [`EnergyLedgers::net_since`] to get the energy booked over it
    /// without re-rounding the two large totals.
    pub fn energy_ledgers(&self) -> EnergyLedgers {
        self.state.energy_ledgers()
    }

    /// Apply one care command at a held boundary
    /// (`design/7_Research/care-contract-2026-09-12.md`).
    ///
    /// Admission is strict and total:
    /// - only `seq == care.admitted_seq + 1` is accepted; any other `seq` is
    ///   `Rejected("out of order")` **without changing a single value**;
    /// - `world.tick()` must equal `cmd.apply_after_tick`, else `Rejected("wrong boundary")`,
    ///   again without changing anything (the host treats it as a recovery failure);
    /// - a command with the expected `seq` at the right boundary **always** consumes that
    ///   seq, including when its own validation rejects it, so replay is a pure function of
    ///   the journal.
    ///
    /// Feed and Clean change the fields immediately. Rain registers a shower whose first
    /// sample falls in the step `B → B + 1`.
    pub fn apply_care(&mut self, cmd: &CareCommand) -> CareReceipt {
        let tick = self.state.tick;
        let receipt = |outcome| CareReceipt {
            seq: cmd.seq,
            tick,
            outcome,
        };
        if cmd.seq != self.state.care.admitted_seq.wrapping_add(1) {
            return receipt(CareOutcome::Rejected("out of order".into()));
        }
        if tick != cmd.apply_after_tick {
            return receipt(CareOutcome::Rejected("wrong boundary".into()));
        }
        // From here the seq is spent whatever happens next.
        self.state.care.admitted_seq = cmd.seq;
        // A dose outside the documented range is refused, not clamped — and it still spends
        // its sequence, like every other admitted-then-rejected command.
        if let Err(reason) = cmd.dose.validate("care dose") {
            return receipt(CareOutcome::Rejected(reason));
        }
        let Some(center) = cmd.target.resolve() else {
            return receipt(CareOutcome::Rejected("invalid target".into()));
        };
        let outcome = match cmd.kind {
            CareKind::Feed => self.feed(center, cmd.dose),
            CareKind::Rain => self.shower(cmd.seq, tick, center, cmd.dose),
            CareKind::Clean => self.clean(center, cmd.dose),
        };
        receipt(outcome)
    }

    /// Consume `seq` with no effect. The host calls this for a record it durably aborted, so
    /// a replay of the journal reaches the same cursor as the run that wrote it. Returns
    /// false (and changes nothing) when `seq` is not the next one.
    pub fn void_care(&mut self, seq: u64) -> bool {
        if seq != self.state.care.admitted_seq.wrapping_add(1) {
            return false;
        }
        self.state.care.admitted_seq = seq;
        true
    }

    // ------------------------------------------------------------------ hunters

    /// What the member oxidation policy did that this world's configured threshold would not
    /// have done, since this `World` value was built. Read-only, transient and process-scoped;
    /// see [`ChargingDiagnostics`] for the exact transaction scope and time window.
    pub fn charging_diagnostics(&self) -> ChargingDiagnostics {
        self.charging
    }

    /// What this world's producers actually made and its mouths actually took, since this
    /// `World` value was built. Read-only, transient and process-scoped; see
    /// [`IntakeDiagnostics`] for the exact scope and time window.
    pub fn intake_diagnostics(&self) -> IntakeDiagnostics {
        self.intake
    }

    /// The oxidation activation threshold, as a fraction of `E_max`, that an **authoritative
    /// member** of this world runs under — the world's configured one when there is no
    /// profile, or no raised policy on it. Published so a recorded experiment can state the
    /// resolved number it actually ran, rather than a reader inferring it from a version.
    pub fn member_oxidation_threshold(&self) -> f64 {
        let org = &self.state.config.organism;
        match self.state.hunters.profile.as_ref() {
            Some(profile) => profile.oxidation_threshold(org),
            None => org.oxidation_threshold,
        }
    }

    /// The hunter extension: profile, members, guts, imports and counters (`crate::hunter`).
    pub fn hunters(&self) -> &HunterState {
        &self.state.hunters
    }

    /// Start the opt-in hunter trial: validate everything, then place **exactly one** founder
    /// of the fixed lineage at `target` and book what it imported.
    ///
    /// Refused, without changing a single value, when the extension is already initialized,
    /// when this world is a budget-matched control, when the profile or the target is invalid,
    /// when the body does not fit the world's own `body_extent_max`, or when there is no
    /// organism capacity left. The founder inventory is **derived from this world's config**
    /// (`S = S_adult`, `R = fraction · R_max`, `E = fraction · E_max`), never hardcoded, and
    /// its material and energy are booked once in the extension's import ledgers — never again
    /// in `external_material_in`, and never by resetting the world's opening history.
    ///
    /// One `Stream::Hunt` draw (key [`hunter::FOUNDER_DRAW_KEY`], counter 0) picks the founder
    /// heading; nothing else in the world consumes a draw here.
    pub fn start_hunter_trial(
        &mut self,
        profile: FixedHunterProfile,
        target: HunterTarget,
    ) -> Result<HunterFounderReceipt, String> {
        if self.state.quiet.active() {
            return Err(
                "this world runs an ordinary quiet policy; the hunter extension is not combined \
                 with it in this slice, because threat and escape interactions need their own \
                 explicit contract"
                    .into(),
            );
        }
        if self.state.hunters.control_deposited {
            return Err("this world is a budget-matched control and must not gain a hunter".into());
        }
        if self.state.hunters.profile.is_some() || self.state.hunters.founders_placed > 0 {
            return Err("the hunter extension is already initialized".into());
        }
        let founder = self.derive_hunter_founder(&profile)?;
        let Some(pos) = target.resolve() else {
            return Err(format!(
                "hunter founder target {target:?} is not on the surface"
            ));
        };
        let cap = self.state.config.capacity.max_organisms as usize;
        if self.state.organisms.len() >= cap {
            return Err(format!(
                "no organism capacity for a hunter founder (population {cap})"
            ));
        }

        // Everything above validates; only now does anything change.
        let tick = self.state.tick;
        let heading = Vec2::from_screen_angle(
            unit(
                self.state.config.seed,
                Stream::Hunt,
                hunter::FOUNDER_DRAW_KEY,
                0,
            ) * TAU,
        );
        let hunger_memory = (1.0 - founder.reserve / founder.phenotype.reserve_max).clamp(0.0, 1.0);
        let id = self.state.organisms.insert(Organism {
            pos,
            heading,
            ou: Vec2::ZERO,
            structure: founder.structure,
            reserve: founder.reserve,
            energy: founder.energy,
            born_tick: tick,
            hunger_memory,
            mode: Mode::Resting,
            escrow: None,
            births: 0,
            genome: profile.genome.clone(),
            phenotype: founder.phenotype.clone(),
            parent: None,
            origin: Origin::Founder,
            turn_counter: Counter::default(),
            fed_this_tick: false,
        });
        let receipt = HunterFounderReceipt {
            id,
            tick,
            pos,
            structure: founder.structure,
            reserve: founder.reserve,
            energy: founder.energy,
            material_in: founder.material_in,
            energy_in: founder.energy_in,
            extent: founder.phenotype.extent,
            sense_radius: founder.phenotype.sense_radius,
            geometry: hunter::ContactGeometry {
                // The founder is an adult, so this is the profile's own geometry at scale 1;
                // it is published here so the art adapter and the harness read the same
                // numbers the world will test contact with.
                scale: 1.0,
                capture_offset_body: profile.capture_offset_body,
                capture_reach_px: profile.capture_reach_px,
                ingestion_offset_body: profile.ingestion_offset_body,
                visual_query_extent_px: profile.visual_query_extent_px,
            },
        };
        self.state.hunters.profile = Some(profile);
        self.state
            .hunters
            .insert_member(hunter::HunterMember::new(id, tick));
        self.state.hunters.founder_material_in += founder.material_in;
        self.state.hunters.founder_energy_in += founder.energy_in;
        self.state.hunters.founders_placed += 1;
        // The import is booked, so the closed box has not moved: a control run and a hunter
        // run are comparable on the same residual.
        debug_assert!(
            self.mass_residual().abs() < 1e-9,
            "founding moved the mass residual"
        );
        Ok(receipt)
    }

    /// Add one or two mature founders to an existing or new hunter lineage.
    ///
    /// Unlike the singleton trial initializer, this permits repeated introductions. All
    /// targets, profile compatibility and capacity are validated before any state changes,
    /// so a two-founder request is never partially applied. Successful introductions enable
    /// paired encounters and underground offspring; the new adults remain active on surface.
    pub fn introduce_hunters(
        &mut self,
        profile: FixedHunterProfile,
        targets: &[HunterTarget],
    ) -> Result<Vec<HunterFounderReceipt>, String> {
        if !(1..=2).contains(&targets.len()) {
            return Err("an interactive hunter introduction must contain one or two founders".into());
        }
        if self.state.hunters.control_deposited {
            return Err("this world is a budget-matched control and must not gain a hunter".into());
        }
        if let Some(installed) = self.state.hunters.profile.as_ref()
            && installed != &profile
        {
            return Err("the requested hunter profile does not match the installed lineage".into());
        }
        let founder = self.derive_hunter_founder(&profile)?;
        let positions = targets.iter().map(|target| {
            target.resolve().ok_or_else(|| format!("hunter founder target {target:?} is not on the surface"))
        }).collect::<Result<Vec<_>, _>>()?;
        let cap = self.state.config.capacity.max_organisms as usize;
        if self.state.organisms.len().saturating_add(positions.len()) > cap {
            return Err(format!(
                "no organism capacity for {} hunter founder(s) (population {}, capacity {cap})",
                positions.len(), self.state.organisms.len()
            ));
        }

        let tick = self.state.tick;
        let first_draw = u64::from(self.state.hunters.founders_placed);
        let mut receipts = Vec::with_capacity(positions.len());
        for (offset, pos) in positions.into_iter().enumerate() {
            let heading = Vec2::from_screen_angle(unit(
                self.state.config.seed, Stream::Hunt, hunter::FOUNDER_DRAW_KEY,
                first_draw + offset as u64,
            ) * TAU);
            let hunger_memory = (1.0 - founder.reserve / founder.phenotype.reserve_max).clamp(0.0, 1.0);
            let id = self.state.organisms.insert(Organism {
                pos,
                heading,
                ou: Vec2::ZERO,
                structure: founder.structure,
                reserve: founder.reserve,
                energy: founder.energy,
                born_tick: tick,
                hunger_memory,
                mode: Mode::Resting,
                escrow: None,
                births: 0,
                genome: profile.genome.clone(),
                phenotype: founder.phenotype.clone(),
                parent: None,
                origin: Origin::Founder,
                turn_counter: Counter::default(),
                fed_this_tick: false,
            });
            self.state.hunters.insert_member(hunter::HunterMember::new(id, tick));
            receipts.push(HunterFounderReceipt {
                id,
                tick,
                pos,
                structure: founder.structure,
                reserve: founder.reserve,
                energy: founder.energy,
                material_in: founder.material_in,
                energy_in: founder.energy_in,
                extent: founder.phenotype.extent,
                sense_radius: founder.phenotype.sense_radius,
                geometry: hunter::ContactGeometry {
                    scale: 1.0,
                    capture_offset_body: profile.capture_offset_body,
                    capture_reach_px: profile.capture_reach_px,
                    ingestion_offset_body: profile.ingestion_offset_body,
                    visual_query_extent_px: profile.visual_query_extent_px,
                },
            });
        }
        let count = receipts.len() as u32;
        self.state.hunters.profile = Some(profile);
        self.state.hunters.founder_material_in += founder.material_in * f64::from(count);
        self.state.hunters.founder_energy_in += founder.energy_in * f64::from(count);
        self.state.hunters.founders_placed += count;
        // Adding founders enables behavior without resetting existing offspring or escrows.
        self.state.apex_dormancy.policy = ApexDormancyPolicy::UndergroundV1;
        self.state.apex_encounters.policy = ApexEncounterPolicy::PairedV1;
        debug_assert!(self.mass_residual().abs() < 1e-9, "founding moved the mass residual");
        Ok(receipts)
    }

    /// The budget-matched control: deposit the **same derived founder inventory** as local
    /// detritus at `target` and place no hunter, so a paired arm can separate "a predator was
    /// added" from "this much material and energy was added".
    ///
    /// The deposit respects the per-cell `De ≤ energy_cap · D` bound; energy the cap cannot
    /// hold leaves as real heat, compensated like every other heat payment, rather than
    /// silently vanishing. Refused, without changing anything, for a world that already has a
    /// profile or a previous deposit.
    pub fn deposit_hunter_budget_control(
        &mut self,
        profile: FixedHunterProfile,
        target: HunterTarget,
    ) -> Result<HunterControlReceipt, String> {
        if self.state.quiet.active() {
            return Err(
                "this world runs an ordinary quiet policy; the hunter extension is not combined \
                 with it in this slice, because threat and escape interactions need their own \
                 explicit contract"
                    .into(),
            );
        }
        if self.state.hunters.profile.is_some() || self.state.hunters.founders_placed > 0 {
            return Err("the hunter extension is already initialized".into());
        }
        if self.state.hunters.control_deposited {
            return Err("this world already holds a budget-matched control deposit".into());
        }
        let founder = self.derive_hunter_founder(&profile)?;
        let Some(pos) = target.resolve() else {
            return Err(format!(
                "hunter control target {target:?} is not on the surface"
            ));
        };

        let cell = cell_of(&pos);
        let at = cell.index();
        let cap = self.state.config.detritus.energy_cap;
        let material = founder.material_in;
        let energy = founder.energy_in;
        // `De ≤ energy_cap · D` per cell, measured against the cell as it will be.
        let room = (cap * (self.state.fields.d[at] + material) - self.state.fields.de[at]).max(0.0);
        let stored = energy.min(room);
        self.state.fields.d[at] += material;
        self.state.fields.de[at] += stored;
        let spilled = energy - stored;
        if spilled > 0.0 {
            accounting::accumulate(
                &mut self.state.heat_out_total,
                &mut self.state.energy_correction.heat_out,
                spilled,
            );
        }
        self.state.hunters.profile = Some(profile);
        self.state.hunters.control_material_in += material;
        self.state.hunters.control_energy_in += energy;
        self.state.hunters.control_deposited = true;
        debug_assert!(
            self.mass_residual().abs() < 1e-9,
            "the control deposit moved the mass residual"
        );
        Ok(HunterControlReceipt {
            tick: self.state.tick,
            cell: cell.0,
            material_in: material,
            energy_in: energy,
            energy_stored: stored,
            energy_heat: spilled,
        })
    }

    /// The founder inventory this world's own config gives the profile, with every derived
    /// number validated before anything is placed or deposited.
    fn derive_hunter_founder(&self, profile: &FixedHunterProfile) -> Result<HunterFounder, String> {
        profile.validate()?;
        let org_cfg = &self.state.config.organism;
        if profile.body_extent_px > org_cfg.body_extent_max {
            return Err(format!(
                "hunter body extent {} exceeds this world's body_extent_max {}",
                profile.body_extent_px, org_cfg.body_extent_max
            ));
        }
        let mut phenotype = decode(&profile.genome, org_cfg);
        // The assembled body is longer than the decoded lobes: members carry the profile's
        // tested support, and no other creature's radius moves.
        phenotype.extent = profile.body_extent_px;
        let structure = phenotype.structure_adult;
        let reserve = profile.founder_reserve_fraction * phenotype.reserve_max;
        let energy = profile.founder_energy_fraction * phenotype.energy_max;
        let material_in = structure + reserve;
        let energy_in = energy + org_cfg.reserve_energy_density * reserve;
        for (name, v) in [
            ("structure", structure),
            ("reserve", reserve),
            ("energy", energy),
            ("material", material_in),
            ("energy_in", energy_in),
        ] {
            if !v.is_finite() || v < 0.0 {
                return Err(format!("derived hunter founder {name} = {v}"));
            }
        }
        if structure < org_cfg.min_structure {
            return Err(format!(
                "derived hunter founder structure {structure} would collapse at once"
            ));
        }
        Ok(HunterFounder {
            phenotype,
            structure,
            reserve,
            energy,
            material_in,
            energy_in,
        })
    }

    /// One hunter per live member, keyed by full ID, with the transported jaw anchor contact
    /// is actually tested at. Empty for every world without hunters, and published separately
    /// from [`World::render_view`] so the existing view structs keep their fields.
    pub fn hunter_view(&self) -> Vec<HunterView> {
        let Some(profile) = self.state.hunters.profile() else {
            return Vec::new();
        };
        let gestation_ticks = ticks_from_seconds(profile.gestation_seconds, DT);
        let tick = self.state.tick;
        self.state
            .hunters
            .members
            .iter()
            .filter_map(|m| {
                if self.state.apex_dormancy.contains(m.id) {
                    return None;
                }
                let o = self.state.organisms.get(m.id)?;
                let geometry = hunter::ContactGeometry::of(profile, o);
                Some(HunterView {
                    id: m.id,
                    role: profile.role,
                    phase: m.phase,
                    phase_progress: m.progress(tick),
                    phase_started_tick: m.phase_started_tick,
                    phase_ends_tick: m.phase_ends_tick,
                    entered_from: m.entered_from,
                    episode: m.episode,
                    attack_counter: m.attack_counter,
                    pos: o.pos,
                    heading: o.heading,
                    geometry,
                    body_scale: geometry.scale,
                    capture_center: hunter::body_point(
                        &self.images,
                        o.pos,
                        o.heading,
                        geometry.capture_offset_body,
                    ),
                    ingestion_center: hunter::body_point(
                        &self.images,
                        o.pos,
                        o.heading,
                        geometry.ingestion_offset_body,
                    ),
                    target: m.target.filter(|t| self.state.organisms.get(*t).is_some()),
                    structure: o.structure,
                    structure_adult: o.phenotype.structure_adult,
                    extent: o.phenotype.extent,
                    juvenile: o.structure < 0.7 * o.phenotype.structure_adult,
                    gut_material: m.gut_material,
                    gut_energy: m.gut_energy,
                    gut_fraction: (m.gut_material / profile.gut_capacity_material).clamp(0.0, 1.0)
                        as f32,
                    gut_capacity: profile.gut_capacity_material,
                    gestation: o.escrow.as_ref().map(|e| {
                        if gestation_ticks == 0 {
                            1.0
                        } else {
                            (tick.saturating_sub(e.started_tick) as f64 / gestation_ticks as f64)
                                .clamp(0.0, 1.0) as f32
                        }
                    }),
                })
            })
            .collect()
    }

    /// Take the hunter records committed since the last drain, in commit order. Transient:
    /// nothing in the world reads them back, and a host that never drains simply lets the
    /// buffer grow. Each consumed prey also produced one ordinary `LifeEvent::Death` with
    /// [`DeathCause::Predation`].
    pub fn drain_hunter_events(&mut self) -> Vec<HunterEvent> {
        std::mem::take(&mut self.hunter_events)
    }

    /// Underground entry, emergence and exhaustion records since the last drain.
    pub fn drain_apex_dormancy_events(&mut self) -> Vec<ApexDormancyEvent> {
        std::mem::take(&mut self.apex_dormancy_events)
    }

    /// The persisted opt-in policy, dormant identities and accounting totals.
    pub fn apex_dormancy(&self) -> &ApexDormancyState {
        &self.state.apex_dormancy
    }

    /// Paid pairing, birth/refund/loss, and combat records since the last drain.
    pub fn drain_apex_encounter_events(&mut self) -> Vec<ApexEncounterEvent> {
        std::mem::take(&mut self.apex_encounter_events)
    }

    /// The independently opt-in adult apex encounter state.
    pub fn apex_encounters(&self) -> &ApexEncounterState {
        &self.state.apex_encounters
    }

    /// Ordinary-quiet begin/refuse/end/abort records since the last drain (`crate::quiet`).
    ///
    /// Transient measurement output for a harness: never checkpointed, never hashed, never read
    /// back by the tick, and never routed to the ambient display. Always empty in an Off world.
    pub fn drain_quiet_events(&mut self) -> Vec<QuietEvent> {
        std::mem::take(&mut self.quiet_events)
    }

    /// The opt-in ordinary quiet extension: policy and any held pauses (`crate::quiet`).
    pub fn quiet(&self) -> &QuietState {
        &self.state.quiet
    }

    /// "Scatter food": charged organic crumbs into `D` and `De`. Per cell `D += m·w_c` and
    /// `De += rho·(m·w_c)` with `rho = detritus.energy_cap`, so `De ≤ energy_cap · D` is
    /// preserved cell by cell and existing scavenging diets can eat all of it. No organism
    /// state is touched: they find it by the sensing they already have.
    fn feed(&mut self, center: CellId, dose: CareDose) -> CareOutcome {
        // The dose scales the nominal total this command moves, and nothing else: the
        // footprint, the weights, the energy density and the allowance rule are unchanged.
        let m = dose.scale(care::FEED_MATERIAL);
        // The bound is the nominal dose. The ledger books the actual f64 sums, which trail
        // the nominal total by a few ulps (a rim footprint sums `3 · w_c` to
        // `3.0000000000000004`), so without the tolerance the allowance would silently buy
        // one fewer feed at the rim than in the interior.
        //
        // A dose the remaining allowance cannot cover is **refused**, not quietly served
        // smaller: the viewer asked for an amount, and a smaller one is a different answer.
        if self.state.care.allowance_used + m > care::FEED_ALLOWANCE + care::ALLOWANCE_TOLERANCE {
            return CareOutcome::Rejected("allowance exhausted".into());
        }
        let rho = care::feed_energy_density(&self.state.config);
        let footprint = care::footprint(&self.graph, center, care::FEED_HOPS);
        let (mut material, mut energy) = (0.0, 0.0);
        for (cell, w) in &footprint {
            let add = m * w;
            let add_energy = rho * add;
            self.state.fields.d[cell.index()] += add;
            self.state.fields.de[cell.index()] += add_energy;
            material += add;
            energy += add_energy;
        }
        self.state.care.feed_material_in += material;
        self.state.care.feed_energy_in += energy;
        self.state.care.allowance_used += material;
        CareOutcome::Applied(CareApplied {
            material_in: material,
            energy_in: energy,
            cells: footprint.len() as u32,
            ..CareApplied::default()
        })
    }

    /// "Shower": register the one active shower. Cells and weights are resolved now, so a
    /// snapshot taken mid-shower resumes at the next undelivered sample over the same
    /// footprint. The weather source is never mutated.
    fn shower(&mut self, seq: u64, tick: u64, center: CellId, dose: CareDose) -> CareOutcome {
        if !self.state.care.showers.is_empty() {
            return CareOutcome::Rejected("shower active".into());
        }
        let footprint = care::footprint(&self.graph, center, care::RAIN_HOPS);
        let cells = footprint.len() as u32;
        self.state.care.showers.push(ActiveShower {
            seq,
            apply_after_tick: tick,
            cells: footprint.iter().map(|(c, _)| c.0).collect(),
            weights: footprint.iter().map(|(_, w)| *w).collect(),
            delivered: 0,
            // Persisted with the shower: the amount that was asked for is the amount its
            // remaining samples deliver, across any number of restarts.
            dose_permille: dose.permille(),
        });
        CareOutcome::Applied(CareApplied {
            // The scheduled total; what lands is booked tick by tick in `rain_depth_in`.
            water_depth: dose.scale(care::RAIN_DEPTH_TOTAL),
            cells,
            ends_tick: Some(tick + u64::from(care::RAIN_TICKS)),
            ..CareApplied::default()
        })
    }

    /// "Clean up litter": export `D` and the chemical energy it carried, at the cell's own
    /// energy density, so `De ≤ energy_cap · D` still holds. `N`, `P`, `F`, `w` and every
    /// organism are untouched, and the exported energy is an export, not heat dissipated
    /// inside the world.
    fn clean(&mut self, center: CellId, dose: CareDose) -> CareOutcome {
        // The dose scales the maximum export. The per-cell half-of-what-is-there limit, the
        // footprint and the proportional energy export are untouched, so a larger dose still
        // cannot sterilize a cell.
        let cap = dose.scale(care::CLEAN_MATERIAL);
        let footprint = care::footprint(&self.graph, center, care::CLEAN_HOPS);
        let (mut material, mut energy) = (0.0, 0.0);
        for (cell, w) in &footprint {
            let i = cell.index();
            let d = self.state.fields.d[i];
            if d <= 0.0 {
                continue;
            }
            let take = (care::CLEAN_FRACTION * d).min(cap * w);
            if take <= 0.0 {
                continue;
            }
            let de = self.state.fields.de[i];
            let removed = (de * (take / d)).min(de).max(0.0);
            self.state.fields.d[i] = d - take;
            self.state.fields.de[i] = de - removed;
            material += take;
            energy += removed;
        }
        if material <= 0.0 {
            return CareOutcome::Rejected("nothing to remove".into());
        }
        self.state.care.clean_material_out += material;
        self.state.care.clean_energy_out += energy;
        // A clean gives the allowance back, but never turns into credit.
        self.state.care.allowance_used = (self.state.care.allowance_used - material).max(0.0);
        let q = CareApplied {
            material_out: material,
            energy_out: energy,
            cells: footprint.len() as u32,
            ..CareApplied::default()
        };
        if material + care::WEIGHT_TOLERANCE < cap {
            CareOutcome::Partial(q)
        } else {
            CareOutcome::Applied(q)
        }
    }

    /// Full invariant check (fields finite/nonnegative, organisms finite, positions canonical,
    /// escrows consistent, population ≤ cap). Called every tick in debug builds and every
    /// telemetry sample in release; a failure is a fatal implementation error.
    pub fn check_invariants(&self) -> Result<(), String> {
        let cfg = &self.state.config;
        self.state.fields.check(cfg.detritus.energy_cap)?;
        // The care ledgers and any shower's progress are runtime invariants too: catching a
        // defect here stops a checkpoint that would not load back. So are the compensated
        // energy ledgers.
        self.state.care.validate(self.state.tick)?;
        self.state.energy_ledgers().validate()?;
        self.state
            .hunters
            .validate(self.state.tick, &self.state.organisms, &self.state.config)?;
        let cap = cfg.capacity.max_organisms as usize;
        if self.state.organisms.len() > cap {
            return Err(format!(
                "population {} exceeds cap {cap}",
                self.state.organisms.len()
            ));
        }
        for (id, o) in self.state.organisms.iter() {
            let who = format!("organism {}:{}", id.slot, id.generation);
            if !o.structure.is_finite() || o.structure < 0.0 {
                return Err(format!("{who}: structure = {}", o.structure));
            }
            if !o.reserve.is_finite() || o.reserve < 0.0 {
                return Err(format!("{who}: reserve = {}", o.reserve));
            }
            if !o.energy.is_finite() || o.energy < 0.0 {
                return Err(format!("{who}: energy = {}", o.energy));
            }
            if !o.hunger_memory.is_finite() {
                return Err(format!("{who}: hunger memory is not finite"));
            }
            if !o.pos.is_canonical() {
                return Err(format!("{who}: position {:?} is not canonical", o.pos));
            }
            if !o.heading.is_finite() || (o.heading.length() - 1.0).abs() > HEADING_TOLERANCE {
                return Err(format!(
                    "{who}: heading {:?} is not a unit vector",
                    o.heading
                ));
            }
            if !o.ou.is_finite() {
                return Err(format!("{who}: OU vector is not finite"));
            }
            if let Some(e) = &o.escrow {
                if !(e.structure.is_finite() && e.reserve.is_finite() && e.energy.is_finite()) {
                    return Err(format!("{who}: escrow is not finite"));
                }
                if e.structure < 0.0 || e.reserve < 0.0 || e.energy < 0.0 {
                    return Err(format!("{who}: escrow is negative"));
                }
                if e.started_tick > self.state.tick {
                    return Err(format!("{who}: escrow starts after the current tick"));
                }
            }
        }
        Ok(())
    }

    /// The segments this organism was **actually transported along** during the last step,
    /// borrowed rather than cloned.
    ///
    /// This is the same data [`World::render_view`] publishes in
    /// [`crate::view::OrganismView::moved`], reachable without building a whole view — that call
    /// clones four full fields and every organism's lobes, which is right for a renderer and
    /// wrong for an observer that wants one number every tick. Read-only: it borrows what the
    /// tick already computed, consumes no draw, and changes nothing.
    ///
    /// A seam crossing appears here as the several straight pieces it really was, which is why
    /// an endpoint difference in chart coordinates is not a substitute: at a seam it is
    /// meaningless, and at a reflective rim it is not even the distance travelled.
    ///
    /// Indexed by **slot**, exactly as the view is: a handle whose generation has been retired
    /// resolves to whatever now occupies that slot. Callers iterate live organisms.
    pub fn moved_segments(&self, id: OrganismId) -> &[cubarium_surface::PathSegment] {
        self.moved.get(id.slot as usize).map_or(&[], Vec::as_slice)
    }

    pub fn render_view(&self) -> RenderView {
        // The same derivation the tick uses for the escrow's due date, so what a renderer
        // shows as "nearly born" is the tick the world will actually commit the birth on.
        let gestation_ticks = ticks_from_seconds(self.state.config.organism.gestation_seconds, DT);
        let tick = self.state.tick;
        let organisms = self
            .state
            .organisms
            .iter()
            .filter(|(id, _)| !self.state.apex_dormancy.contains(*id))
            .map(|(id, o)| OrganismView {
                id,
                pos: o.pos,
                heading: o.heading,
                lobes: o.phenotype.lobes.clone(),
                hue: o.phenotype.hue,
                mode: o.mode,
                fed: o.fed_this_tick,
                juvenile: o.structure < 0.7 * o.phenotype.structure_adult,
                form: o.phenotype.form,
                gestation: o.escrow.as_ref().map(|e| {
                    // A zero-tick gestation (a config that rounds below one tick) is due
                    // the moment it starts, so it reads as complete rather than as NaN.
                    if gestation_ticks == 0 {
                        1.0
                    } else {
                        (tick.saturating_sub(e.started_tick) as f64 / gestation_ticks as f64)
                            .clamp(0.0, 1.0) as f32
                    }
                }),
                moved: self
                    .moved
                    .get(id.slot as usize)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();
        RenderView {
            tick: self.state.tick,
            producer: self.state.fields.p.clone(),
            detritus: self.state.fields.d.clone(),
            fruit: self.state.fields.f.clone(),
            water: self.state.fields.w.clone(),
            rain: self.rain.to_vec(),
            producer_max: self.state.config.producer.max,
            organisms,
        }
    }

    /// Take the births and deaths committed since the last drain, in commit order (a tick's
    /// deaths before its births, each in slot order). The buffer is transient: nothing in the
    /// world reads it back, and a host that never drains it simply lets it grow.
    pub fn drain_events(&mut self) -> Vec<LifeEvent> {
        std::mem::take(&mut self.events)
    }

    /// Every cell's material fields plus its live organism count, for the observer's
    /// field dump. Cells are in `CellId` index order, 1,280 entries each.
    pub fn field_dump(&self) -> FieldDump {
        let mut organisms = vec![0u16; CELL_COUNT];
        for (_, o) in self.state.organisms.iter() {
            organisms[cell_of(&o.pos).index()] += 1;
        }
        let fields = &self.state.fields;
        FieldDump {
            tick: self.state.tick,
            n: fields.n.clone(),
            p: fields.p.clone(),
            d: fields.d.clone(),
            de: fields.de.clone(),
            w: fields.w.clone(),
            f: fields.f.clone(),
            organisms,
        }
    }

    /// Each cell's four graph neighbors in `Edge` order (`Top, Right, Bottom, Left`), as raw
    /// cell indices with `None` at the open rim. Static for the life of the world: an
    /// analyzer reads it once and computes graph distances without this crate.
    pub fn cell_neighbors(&self) -> Vec<[Option<u16>; 4]> {
        CellId::all()
            .map(|cell| {
                let n = self.graph.neighbors(cell);
                std::array::from_fn(|e| n[e].map(|c| c.0))
            })
            .collect()
    }

    /// Produce a telemetry sample and reset the per-sample counters.
    pub fn telemetry(&mut self) -> Telemetry {
        let mass_residual = self.mass_residual();
        let (mut hunters, mut hunter_juveniles) = (0u32, 0u32);
        for m in &self.state.hunters.members {
            if let Some(o) = self.state.organisms.get(m.id) {
                hunters += 1;
                if o.structure < 0.7 * o.phenotype.structure_adult {
                    hunter_juveniles += 1;
                }
            }
        }
        let mut population_by_face = [0u32; 5];
        let mut occupied = vec![false; CELL_COUNT];
        let mut occupied_cells = 0;
        let mut escrows = 0;
        let (mut organism_material, mut organism_energy) = (0.0, 0.0);
        let (mut mode_resting, mut mode_seeking, mut mode_feeding) = (0, 0, 0);
        let mut population_by_form = [0u32; 8];
        let mut height_by_form = [0.0f64; 8];
        for (_, o) in self.state.organisms.iter() {
            population_by_face[o.pos.face.index()] += 1;
            let form = (o.phenotype.form as usize).min(7);
            population_by_form[form] += 1;
            height_by_form[form] += o.pos.embed()[1];
            let cell = cell_of(&o.pos).index();
            if !occupied[cell] {
                occupied[cell] = true;
                occupied_cells += 1;
            }
            if o.escrow.is_some() {
                escrows += 1;
            }
            organism_material += o.material();
            organism_energy += o.energy;
            match o.mode {
                Mode::Resting => mode_resting += 1,
                Mode::Seeking => mode_seeking += 1,
                Mode::Feeding => mode_feeding += 1,
            }
        }
        let fields = &self.state.fields;
        let mut producer_by_face = [0.0f64; 5];
        let mut detritus_by_face = [0.0f64; 5];
        let mut water_by_face = [0.0f64; 5];
        for cell in CellId::all() {
            let face = cell.face().index();
            producer_by_face[face] += fields.p[cell.index()];
            detritus_by_face[face] += fields.d[cell.index()];
            water_by_face[face] += fields.w[cell.index()];
        }
        let mean_height_by_form = std::array::from_fn(|i| {
            if population_by_form[i] > 0 {
                height_by_form[i] / f64::from(population_by_form[i])
            } else {
                0.0
            }
        });
        let sample = Telemetry {
            tick: self.state.tick,
            population: self.state.organisms.len() as u32,
            births: self.counters.births,
            deaths_starvation: self.counters.deaths[0],
            deaths_age: self.counters.deaths[1],
            deaths_collapse: self.counters.deaths[2],
            escrows,
            cap_rejections: self.counters.cap_rejections,
            nutrient: fields.n.iter().sum(),
            producer: fields.p.iter().sum(),
            detritus: fields.d.iter().sum(),
            detritus_energy: fields.de.iter().sum(),
            fruit: fields.f.iter().sum(),
            organism_material,
            organism_energy,
            light_in: self.counters.light_in,
            heat_out: self.counters.heat_out,
            mass_residual,
            population_by_face,
            producer_by_face,
            detritus_by_face,
            water: fields.w.iter().sum(),
            water_by_face,
            rain_in: self.counters.rain_in,
            evap_out: self.counters.evap_out,
            occupied_cells,
            travel_fallbacks: self.counters.travel_fallbacks,
            travel_ties: self.counters.travel_ties,
            pairs_considered: self.neighbors.pairs_considered,
            pairs_unfolded: self.neighbors.pairs_unfolded,
            neighbor_truncations: self.neighbors.lists_truncated,
            mode_resting,
            mode_seeking,
            mode_feeding,
            population_by_form,
            mean_height_by_form,
            state_hash: snapshot::state_hash(&self.state),
            ecology_hash: snapshot::ecology_hash(&self.state),
            care_admitted_seq: self.state.care.admitted_seq,
            care_feed_material_in: self.state.care.feed_material_in,
            care_feed_energy_in: self.state.care.feed_energy_in,
            care_rain_depth_in: self.state.care.rain_depth_in,
            care_clean_material_out: self.state.care.clean_material_out,
            care_clean_energy_out: self.state.care.clean_energy_out,
            care_allowance_used: self.state.care.allowance_used,
            hunters,
            hunter_juveniles,
            hunter_attacks: self.counters.hunter_attacks,
            hunter_captures: self.counters.hunter_captures,
            deaths_predation: self.counters.deaths_predation,
            hunter_gut_material: self.state.hunters.gut_material_total(),
            hunter_gut_energy: self.state.hunters.gut_energy_total(),
            hunter_attacks_total: self.state.hunters.attacks_total,
            hunter_captures_total: self.state.hunters.captures_total,
            predation_deaths_total: self.state.hunters.predation_deaths_total,
            hunter_births_total: self.state.hunters.hunter_births_total,
            hunter_deaths_total: self.state.hunters.hunter_deaths_total,
            hunter_material_in: self.state.hunters.imported_material(),
            hunter_energy_in: self.state.hunters.imported_energy(),
        };
        self.counters = TickCounters::default();
        self.neighbors.pairs_considered = 0;
        self.neighbors.pairs_unfolded = 0;
        self.neighbors.lists_truncated = 0;
        sample
    }

    pub fn config(&self) -> &WorldConfig {
        &self.state.config
    }

    pub fn tick(&self) -> u64 {
        self.state.tick
    }

    pub fn population(&self) -> usize {
        self.state.organisms.len()
    }

    /// Death causes in `deaths_total` order.
    pub const DEATH_CAUSES: [DeathCause; 3] = [
        DeathCause::Starvation,
        DeathCause::Age,
        DeathCause::Collapse,
    ];
}

/// The audited energy total of `design/m2-world-spec.md` "Units and quantities":
/// `Σ_cells (e_p·P + e_f·F + De) + Σ_organisms (E + e_r·R) + Σ_escrow (e_r·(S_c + R_c) + E_c)`.
/// Reserve material carries chemical energy; structure does not; fruit carries `e_f`.
///
/// A hunter's carried carcass is stored energy too: `Σ gut_energy` is exactly the energy that
/// was removed from the prey, held until it is digested, rejected or released by the hunter's
/// own death (`crate::hunter`).
#[cfg(any(debug_assertions, test))]
fn stored_energy(state: &WorldState) -> f64 {
    let e_p = state.config.producer.energy_density;
    let e_f = state.config.fruit.energy_density;
    let e_r = state.config.organism.reserve_energy_density;
    let cells: f64 = state.fields.p.iter().map(|p| e_p * p).sum::<f64>()
        + state.fields.f.iter().map(|f| e_f * f).sum::<f64>()
        + state.fields.de.iter().sum::<f64>();
    let organisms: f64 = state
        .organisms
        .iter()
        .map(|(_, o)| {
            o.energy
                + e_r * o.reserve
                + o.escrow
                    .as_ref()
                    .map_or(0.0, |e| e_r * (e.structure + e.reserve) + e.energy)
        })
        .sum();
    cells + organisms + state.hunters.gut_energy_total()
}

/// Edible detritus `D_eff = D · min(1, ρ / e_r)` with `ρ = De / D` (zero when `D == 0`),
/// from `design/m2-world-spec.md` "Controller". Detritus too energy-poor to pay for its own
/// reserve storage is not food: this is the same `min(1, ρ / e_r)` factor that scales
/// scavenging assimilation, so what an organism sees and what it can digest agree.
fn edible_detritus(detritus: f64, energy: f64, e_r: f64) -> f64 {
    if detritus <= 0.0 {
        return 0.0;
    }
    let rho = energy / detritus;
    if e_r > 0.0 {
        detritus * (rho / e_r).min(1.0)
    } else {
        detritus
    }
}

/// The unit chart direction of increasing embedded height at a point of `face`: the chart
/// gradient of `y`, `(tangent_u.y, tangent_v.y)` normalized. Zero on the level top face,
/// `−v` on the four side faces.
fn up_direction(face: Face) -> Vec2 {
    let frame = face_frame(face);
    normalize_or_zero(Vec2::new(frame.tangent_u[1], frame.tangent_v[1]))
}

/// Sensing depth in graph hops for a sensing radius: `ceil(r_sense / 4)`, at least 1 and at
/// most `SENSE_DEPTH_MAX` (`design/fauna-v2.md` "Controller v2").
fn sense_depth(sense_radius: f64) -> usize {
    let hops = (sense_radius / cubarium_surface::CELL_PIXELS).ceil();
    if hops.is_finite() {
        (hops as usize).clamp(1, SENSE_DEPTH_MAX)
    } else {
        1
    }
}

/// For every cell, the cells at graph distance exactly 1, 2 and 3 (breadth-first over the
/// field graph, seams included, never across the open rim).
fn sense_rings(graph: &FieldGraph) -> Vec<[Vec<CellId>; SENSE_DEPTH_MAX]> {
    CellId::all()
        .map(|origin| {
            let mut seen = vec![false; CELL_COUNT];
            seen[origin.index()] = true;
            let mut rings: [Vec<CellId>; SENSE_DEPTH_MAX] = Default::default();
            let mut frontier = vec![origin];
            for ring in rings.iter_mut() {
                let mut next = Vec::new();
                for cell in &frontier {
                    for n in graph.neighbors(*cell).iter().flatten() {
                        if !seen[n.index()] {
                            seen[n.index()] = true;
                            next.push(*n);
                        }
                    }
                }
                next.sort_unstable_by_key(|c| c.index());
                *ring = next.clone();
                frontier = next;
            }
            rings
        })
        .collect()
}

/// Unit gradient, or zero when the gradient carries no direction.
fn normalize_or_zero(v: Vec2) -> Vec2 {
    if v.length() > GRADIENT_EPS {
        v.normalized().unwrap_or(Vec2::ZERO)
    } else {
        Vec2::ZERO
    }
}

/// The proportional share each request receives when the cell cannot serve them all.
fn share(requested: f64, available: f64) -> f64 {
    if requested > available && requested > 0.0 {
        available / requested
    } else {
        1.0
    }
}

pub(crate) fn ticks_from_seconds(seconds: f64, dt: f64) -> u64 {
    let ticks = (seconds / dt).round();
    if ticks.is_finite() && ticks > 0.0 {
        ticks as u64
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cubarium_surface::CellId;

    use crate::events::LifeEvent;

    /// The M2 fixture: v1 founders (one omnivore genotype, `diet` 0.7, drawn hues), so the
    /// intake, gestation and accounting tests below read as they were written. The fauna v2
    /// kinds have their own tests, which set `founders.kinds` explicitly.
    fn config() -> WorldConfig {
        let mut cfg = WorldConfig::default();
        cfg.founders.kinds.clear();
        cfg
    }

    /// The founder diet as the world widens it from the `f32` genome.
    fn founder_diet() -> f64 {
        f64::from(Genome::founder(0.5, &config().drives).diet)
    }

    /// Move `count` founders onto one cell and empty them out, so they contest its producer.
    fn crowd_onto_cell(world: &mut World, cell: CellId, producer: f64) -> Vec<OrganismId> {
        let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
        let base = cell.center();
        for (k, id) in ids.iter().enumerate() {
            let o = world.state.organisms.get_mut(*id).expect("live founder");
            o.pos = SurfacePoint::new(base.face, base.u + k as f64 * 0.5, base.v);
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
            assert_eq!(cell_of(&o.pos), cell, "test bodies must share the cell");
        }
        world.state.fields.p[cell.index()] = producer;
        ids
    }

    #[test]
    fn founders_are_created_with_recorded_external_material() {
        let world = World::new(config()).expect("default config is valid");
        assert_eq!(world.population(), config().founders.count as usize);
        let expected: f64 = world
            .state
            .organisms
            .iter()
            .map(|(_, o)| o.structure + o.reserve)
            .sum();
        assert!((world.state.external_material_in - expected).abs() < 1e-12);
        assert!(world.mass_residual().abs() < 1e-12);
        world
            .check_invariants()
            .expect("a fresh world is consistent");
        // Founders land on every face: 72 draws over five faces effectively never miss one.
        let mut faces = [0u32; 5];
        for (_, o) in world.state.organisms.iter() {
            faces[o.pos.face.index()] += 1;
        }
        assert!(faces.iter().all(|&n| n > 0), "{faces:?}");
    }

    #[test]
    fn six_hundred_ticks_conserve_material_and_keep_the_population() {
        let mut world = World::new(config()).expect("default config is valid");
        for _ in 0..600 {
            world.step();
        }
        world
            .check_invariants()
            .expect("invariants hold after 600 ticks");
        assert!(world.population() > 0, "the world died out");
        assert!(
            world.mass_residual().abs() < 1e-9,
            "mass residual {}",
            world.mass_residual()
        );
        let sample = world.telemetry();
        assert_eq!(sample.tick, 600);
        assert!(sample.light_in > 0.0 && sample.heat_out > 0.0);
        assert!(sample.occupied_cells > 0);
    }

    #[test]
    fn replay_is_deterministic_and_seed_sensitive() {
        let mut a = World::new(config()).expect("valid");
        let mut b = World::new(config()).expect("valid");
        let mut c = World::new(WorldConfig {
            seed: config().seed + 1,
            ..config()
        })
        .expect("valid");
        for _ in 0..300 {
            a.step();
            b.step();
            c.step();
        }
        assert_eq!(a.telemetry().state_hash, b.telemetry().state_hash);
        assert_ne!(a.telemetry().state_hash, c.telemetry().state_hash);
    }

    #[test]
    fn contested_feeding_splits_the_cell_and_conserves_material() {
        let mut cfg = config();
        cfg.founders.count = 3;
        // Low enough that three full mouthfuls (3 x 0.0025 m) cannot all be served.
        cfg.drives.feed_min = 0.001;
        // Linear requests: the spec's proportional allocation is unchanged by the intake
        // saturation, and a saturating mouth on a cell this poor asks for far too little to
        // contest it (three organisms would need `K_P` near zero or a crowd of ~180).
        cfg.organism.intake_half_saturation = 0.0;
        let mut world = World::new(cfg).expect("valid");
        let cell = CellId::new(Face::Front, 0, 0);
        let ids = crowd_onto_cell(&mut world, cell, 0.004);

        let before = world.mass_residual();
        let available = world.state.fields.p[cell.index()];
        world.step();

        let gains: Vec<f64> = ids
            .iter()
            .map(|id| world.state.organisms.get(*id).expect("alive").reserve)
            .collect();
        assert!(gains.iter().all(|&g| g > 0.0), "{gains:?}");
        for pair in gains.windows(2) {
            assert!(
                (pair[0] - pair[1]).abs() < 1e-15,
                "unequal shares {gains:?}"
            );
        }
        // The cell is emptied: the requests exceeded what it held.
        assert!(world.state.fields.p[cell.index()] >= 0.0);
        assert!(
            world.state.fields.p[cell.index()] < 1e-12,
            "{}",
            world.state.fields.p[cell.index()]
        );
        // Each organism assimilated η_m of its share; the rest became detritus in the cell.
        let taken: f64 = gains.iter().sum();
        let eta = world.config().organism.assimilation_material;
        assert!(
            (taken - eta * available).abs() < 1e-6,
            "{taken} vs {}",
            eta * available
        );
        assert!(
            (world.mass_residual() - before).abs() < 1e-12,
            "{} -> {}",
            before,
            world.mass_residual()
        );
        for (_, o) in world.state.organisms.iter() {
            assert!(o.fed_this_tick);
        }
    }

    #[test]
    fn the_render_view_reports_gestation_only_while_an_escrow_is_held() {
        let mut cfg = config();
        cfg.founders.count = 1;
        let gestation_ticks = ticks_from_seconds(cfg.organism.gestation_seconds, DT);
        assert!(
            gestation_ticks > 1,
            "this test needs a multi-tick gestation"
        );
        let mut world = World::new(cfg).expect("valid");
        let id = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("one founder");

        // No escrow: nothing to show.
        assert_eq!(world.render_view().organisms[0].gestation, None);

        // Started this tick: no progress yet.
        world.state.tick = 100;
        let genome = world.state.organisms.get(id).expect("alive").genome.clone();
        world.state.organisms.get_mut(id).expect("alive").escrow = Some(Escrow {
            structure: 0.4,
            reserve: 0.2,
            energy: 0.5,
            started_tick: 100,
            genome,
        });
        assert_eq!(world.render_view().organisms[0].gestation, Some(0.0));

        // Halfway through, to within a tick of rounding.
        world.state.tick = 100 + gestation_ticks / 2;
        let half = world.render_view().organisms[0]
            .gestation
            .expect("gestating");
        assert!((half - 0.5).abs() < 1.0 / gestation_ticks as f32, "{half}");

        // Exactly due, and then well past it: clamped at 1, never above.
        world.state.tick = 100 + gestation_ticks;
        assert_eq!(world.render_view().organisms[0].gestation, Some(1.0));
        world.state.tick = 100 + gestation_ticks * 9;
        assert_eq!(world.render_view().organisms[0].gestation, Some(1.0));

        // And it goes away with the escrow.
        world.state.organisms.get_mut(id).expect("alive").escrow = None;
        assert_eq!(world.render_view().organisms[0].gestation, None);
    }

    #[test]
    fn a_birth_at_the_cap_refunds_the_escrow_to_the_parent() {
        let mut cfg = config();
        cfg.capacity.max_organisms = 1;
        cfg.founders.count = 1;
        // Keep the parent's stocks still so the refund is the only change to its reserve.
        cfg.mechanisms.grazing = false;
        cfg.mechanisms.scavenging = false;
        let mut world = World::new(cfg).expect("valid");
        let id = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("one founder");

        // A gestation that finished long ago, so this tick is the birth tick.
        world.state.tick = 700;
        let (structure, reserve, energy, before_reserve, before_energy) = {
            let o = world.state.organisms.get_mut(id).expect("alive");
            let escrow = Escrow {
                structure: 0.4,
                reserve: 0.2,
                energy: 0.5,
                started_tick: 0,
                genome: o.genome.clone(),
            };
            let snapshot = (
                escrow.structure,
                escrow.reserve,
                escrow.energy,
                o.reserve,
                o.energy,
            );
            o.escrow = Some(escrow);
            snapshot
        };

        world.step();

        let o = world.state.organisms.get(id).expect("alive");
        assert!(o.escrow.is_none(), "the escrow must be released");
        assert_eq!(o.reserve, before_reserve + (structure + reserve));
        assert!(
            o.energy > before_energy,
            "energy {} vs {before_energy}",
            o.energy
        );
        assert!(
            o.energy < before_energy + energy,
            "movement still costs energy"
        );
        assert_eq!(world.population(), 1);
        assert_eq!(world.state.births_total, 0);
        assert_eq!(world.state.cap_rejections_total, 1);
    }

    #[test]
    fn a_completed_gestation_places_a_child() {
        let mut cfg = config();
        cfg.founders.count = 1;
        cfg.mechanisms.grazing = false;
        cfg.mechanisms.scavenging = false;
        let mut world = World::new(cfg).expect("valid");
        let parent = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("one founder");
        world.state.tick = 700;
        {
            let o = world.state.organisms.get_mut(parent).expect("alive");
            o.escrow = Some(Escrow {
                structure: 0.4,
                reserve: 0.2,
                energy: 0.5,
                started_tick: 0,
                genome: o.genome.clone(),
            });
        }
        let residual = world.mass_residual();
        world.step();

        assert_eq!(world.population(), 2);
        assert_eq!(world.state.births_total, 1);
        let child = world
            .state
            .organisms
            .iter()
            .find(|(_, o)| o.origin == Origin::Descendant)
            .map(|(id, o)| (id, o.clone()))
            .expect("a child");
        assert_eq!(child.1.parent, Some(parent));
        assert_eq!(child.1.born_tick, 701);
        assert_eq!(child.1.structure, 0.4);
        assert_eq!(child.1.reserve, 0.2);
        assert!((child.1.heading.length() - 1.0).abs() < 1e-12);
        let parent_pos = world.state.organisms.get(parent).expect("alive").pos;
        let offset =
            cubarium_surface::surface_distance(parent_pos, child.1.pos, 16.0).expect("nearby");
        assert!((offset - 2.5).abs() < 0.1, "child placed {offset} px away");
        assert!((world.mass_residual() - residual).abs() < 1e-12);
    }

    #[test]
    fn render_view_and_telemetry_describe_the_world() {
        let mut world = World::new(config()).expect("valid");
        world.step();
        let view = world.render_view();
        assert_eq!(view.tick, 1);
        assert_eq!(view.producer.len(), CELL_COUNT);
        assert_eq!(view.detritus.len(), CELL_COUNT);
        assert_eq!(view.organisms.len(), world.population());
        assert!(view.organisms.iter().all(|o| !o.lobes.is_empty()));
        assert!(
            view.organisms.iter().any(|o| !o.moved.is_empty()),
            "resting still drifts a little"
        );

        let sample = world.telemetry();
        assert_eq!(sample.population, world.population() as u32);
        assert_eq!(
            sample.population_by_face.iter().sum::<u32>(),
            sample.population
        );
        assert!((sample.producer_by_face.iter().sum::<f64>() - sample.producer).abs() < 1e-9);
        assert!((sample.detritus_by_face.iter().sum::<f64>() - sample.detritus).abs() < 1e-9);
        assert!(sample.producer_by_face.iter().all(|&p| p > 0.0));
        assert_eq!(
            sample.mode_resting + sample.mode_seeking + sample.mode_feeding,
            sample.population
        );
        assert!(sample.pairs_considered > 0);
        // The sample resets the per-sample counters.
        let empty = world.telemetry();
        assert_eq!(empty.pairs_considered, 0);
        assert_eq!(empty.light_in, 0.0);
    }

    /// `moved_segments` is the same data the render view publishes, borrowed instead of cloned.
    /// An observer that wants one number per tick must not have to build a whole view, and must
    /// not get a different answer for taking the cheaper door.
    #[test]
    fn moved_segments_is_exactly_what_the_render_view_publishes() {
        let mut world = World::new(config()).expect("valid");
        for _ in 0..40 {
            world.step();
            let view = world.render_view();
            assert_eq!(view.organisms.len(), world.population());
            for o in &view.organisms {
                assert_eq!(
                    world.moved_segments(o.id),
                    o.moved.as_slice(),
                    "the borrowed segments differ from the published ones for {:?}",
                    o.id
                );
            }
        }
        // Some organism really did move, so the equality above is not two empty slices.
        let view = world.render_view();
        assert!(view.organisms.iter().any(|o| !o.moved.is_empty()));
        // A handle the arena never issued has no segments rather than panicking.
        assert!(
            world
                .moved_segments(OrganismId {
                    slot: u32::MAX,
                    generation: 1
                })
                .is_empty()
        );
        // And reading them changes nothing at all.
        let before = crate::snapshot::state_hash(&world.state);
        for (id, _) in world.state.organisms.iter() {
            let _ = world.moved_segments(id);
        }
        assert_eq!(crate::snapshot::state_hash(&world.state), before);
    }

    #[test]
    fn the_energy_audit_closes_every_tick_and_cumulatively() {
        let mut world = World::new(config()).expect("valid");
        let opening = stored_energy(&world.state);
        let mut worst: f64 = 0.0;
        for _ in 0..6000 {
            let before = stored_energy(&world.state);
            let ledgers = world.state.energy_ledgers();
            world.step();
            let booked = world.state.energy_ledgers().net_since(ledgers);
            let drift = (stored_energy(&world.state) - before) - booked;
            worst = worst.max(drift.abs());
        }
        assert!(worst < 1e-9, "worst per-tick energy drift {worst:e}");
        // The per-tick identity is exact to rounding; the cumulative sum of 6000 ticks of
        // rounding is bounded relative to the energy being differenced, not absolutely. The
        // cumulative side reads the compensated ledgers (`crate::accounting`), which for a
        // world created here (corrections open at zero) is the whole history.
        let booked = world.state.net_energy_in_corrected();
        let total = stored_energy(&world.state);
        let overall = (total - opening) - booked;
        assert!(
            overall.abs() < 1e-9 * total.max(1.0),
            "cumulative energy drift {overall:e} over 6000 ticks"
        );
        // A real leak would be many orders larger than accumulated rounding.
        assert!(
            overall.abs() / 6000.0 < 1e-10,
            "systematic energy drift {:e} per tick",
            overall.abs() / 6000.0
        );
        assert!(world.state.light_in_total > 0.0 && world.state.heat_out_total > 0.0);
    }

    #[test]
    fn the_intake_request_saturates_at_half_at_k_p() {
        // One organism alone on a frozen cell: its reserve gain is exactly `η_m · q`.
        fn gain(half_saturation: f64, producer: f64) -> f64 {
            let mut cfg = config();
            cfg.founders.count = 1;
            cfg.mechanisms.scavenging = false;
            cfg.producer.growth = 0.0;
            cfg.producer.mortality = 0.0;
            cfg.detritus.decomposition = 0.0;
            cfg.nutrient.diffusion = 0.0;
            // A rich cell would ripen a trace of fruit before the bite and shift `P`.
            cfg.fruit.ripen = 0.0;
            cfg.organism.intake_half_saturation = half_saturation;
            let mut world = World::new(cfg).expect("valid");
            let id = world
                .state
                .organisms
                .iter()
                .map(|(id, _)| id)
                .next()
                .expect("one founder");
            let cell = CellId::new(Face::Front, 0, 0);
            {
                let o = world.state.organisms.get_mut(id).expect("alive");
                o.pos = cell.center();
                o.reserve = 0.0;
                o.hunger_memory = 1.0;
                o.mode = Mode::Seeking;
            }
            world.state.fields.p[cell.index()] = producer;
            world.step();
            world.state.organisms.get(id).expect("alive").reserve
        }

        let org = config().organism;
        let k_p = org.intake_half_saturation;
        assert!(k_p > 0.0, "the default is a saturating mouth");

        // `K_P = 0` is the linear law: a full mouthful of `graze_rate · dt = k_mouth · diet ·
        // dt` (`design/fauna-v2.md`), assimilated at η_m (to rounding: the world multiplies
        // the same factors in its own order).
        let linear = gain(0.0, k_p);
        let mouthful = org.assimilation_material * (org.mouth_rate * founder_diet()) * DT;
        assert!(
            (linear - mouthful).abs() < 1e-15 * mouthful,
            "{linear} vs {mouthful}"
        );

        // At `P = K_P` the type-II term is exactly one half.
        let saturating = gain(k_p, k_p);
        assert_eq!(saturating, 0.5 * linear);

        // And it is monotone in the cell's stock: more food, bigger bite, never more than one.
        let richer = gain(k_p, 3.0 * k_p);
        assert!(saturating < richer && richer < linear);
        assert!(
            (richer - 0.75 * linear).abs() < 1e-15 * linear,
            "{richer} vs {}",
            0.75 * linear
        );
    }

    #[test]
    fn poor_detritus_assimilates_less_and_still_closes() {
        let mut cfg = config();
        cfg.founders.count = 2;
        cfg.mechanisms.grazing = false;
        // Freeze the fields so the detritus the organisms bite into is exactly what is set
        // here: the type-II request reads the cell's pre-settlement stock, and both test
        // cells are on a side face, so the fall step would slide a slice of it away first.
        cfg.producer.growth = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.detritus.fall = 0.0;
        cfg.nutrient.diffusion = 0.0;
        let e_r = cfg.organism.reserve_energy_density;
        let mut world = World::new(cfg).expect("valid");
        let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
        // Two cells with the same detritus but energy densities of e_r/8 and e_r/2.
        let cells = [
            CellId::new(Face::Front, 0, 0),
            CellId::new(Face::Front, 4, 4),
        ];
        let densities = [e_r / 8.0, e_r / 2.0];
        // Enough detritus that even the poor cell's edible share clears `feed_min`.
        let detritus = 2.0;
        for (k, id) in ids.iter().enumerate() {
            let o = world.state.organisms.get_mut(*id).expect("alive");
            o.pos = cells[k].center();
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
            let c = cells[k].index();
            world.state.fields.p[c] = 0.0;
            world.state.fields.d[c] = detritus;
            world.state.fields.de[c] = densities[k] * detritus;
        }
        let before = stored_energy(&world.state);
        let (light, heat) = (world.state.light_in_total, world.state.heat_out_total);
        world.step();

        let gains: Vec<f64> = ids
            .iter()
            .map(|id| world.state.organisms.get(*id).expect("alive").reserve)
            .collect();
        assert!(gains[0] > 0.0 && gains[1] > 0.0, "{gains:?}");
        // Poor detritus loses twice: `η` scales with `ρ/e_r` (a factor of four here) and the
        // type-II request scales with `D_eff/(D_eff + K_P)` on top of it.
        let k_p = world.config().organism.intake_half_saturation;
        let edible: Vec<f64> = densities
            .iter()
            .map(|rho| detritus * (rho / e_r).min(1.0))
            .collect();
        let bite = |food: f64| food / (food + k_p);
        let expected = 4.0 * bite(edible[1]) / bite(edible[0]);
        assert!(
            expected > 4.0,
            "the saturating request must widen the gap, not close it"
        );
        assert!(
            (gains[1] / gains[0] - expected).abs() < 1e-6,
            "ratio {} vs {expected}",
            gains[1] / gains[0]
        );
        // The poor detritus never credits more reserve energy than the food carried.
        for (k, id) in ids.iter().enumerate() {
            let o = world.state.organisms.get(*id).expect("alive");
            let c = cells[k].index();
            assert!(world.state.fields.de[c] >= 0.0);
            assert!(o.reserve * e_r <= densities[k] * detritus + 1e-12);
        }
        let booked = (world.state.light_in_total - light) - (world.state.heat_out_total - heat);
        assert!(((stored_energy(&world.state) - before) - booked).abs() < 1e-9);
    }

    #[test]
    fn energy_free_detritus_is_not_food() {
        let mut cfg = config();
        cfg.founders.count = 2;
        cfg.mechanisms.grazing = false;
        // `De = 2.0` on `D = 1.0` needs a cap that admits it; the point is `ρ ≥ e_r`.
        cfg.detritus.energy_cap = 2.0;
        let mut world = World::new(cfg).expect("valid");
        let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
        let cells = [
            CellId::new(Face::Front, 0, 0),
            CellId::new(Face::Front, 8, 8),
        ];
        // Same detritus, no energy versus fully charged.
        let energies = [0.0, 2.0];
        for (k, id) in ids.iter().enumerate() {
            let o = world.state.organisms.get_mut(*id).expect("alive");
            o.pos = cells[k].center();
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
            let c = cells[k].index();
            world.state.fields.p[c] = 0.0;
            world.state.fields.d[c] = 1.0;
            world.state.fields.de[c] = energies[k];
        }
        world.step();

        let spent = world.state.organisms.get(ids[0]).expect("alive");
        assert_eq!(
            spent.mode,
            Mode::Seeking,
            "energy-free detritus must not read as food"
        );
        assert_eq!(spent.reserve, 0.0);
        assert!(!spent.fed_this_tick);

        let rich = world.state.organisms.get(ids[1]).expect("alive");
        assert_eq!(rich.mode, Mode::Feeding, "charged detritus is food");
        assert!(rich.reserve > 0.0);
        assert!(rich.fed_this_tick);
    }

    #[test]
    fn both_intake_channels_respect_the_reserve_ceiling() {
        let mut cfg = config();
        cfg.founders.count = 1;
        let mut world = World::new(cfg).expect("valid");
        let id = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("one founder");
        let cell = CellId::new(Face::Front, 0, 0);
        // Headroom of 0.0014 m: more than grazing's saturated bite alone (about 0.0012 m at
        // `diet` 0.7), less than the two channels' bites together
        // (`graze_rate + scavenge_rate = mouth_rate`, a full `k_mouth · dt = 0.0025 m`).
        let (reserve_max, headroom) = {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.pos = cell.center();
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
            o.reserve = o.phenotype.reserve_max - 0.0014;
            (o.phenotype.reserve_max, 0.0014)
        };
        let c = cell.index();
        world.state.fields.p[c] = 1.0;
        world.state.fields.d[c] = 1.0;
        world.state.fields.de[c] = 1.0;
        let before = world.state.organisms.get(id).expect("alive").reserve;
        world.step();

        let o = world.state.organisms.get(id).expect("alive");
        assert!(
            o.reserve <= reserve_max,
            "reserve {} exceeds {reserve_max}",
            o.reserve
        );
        assert!(o.reserve - before <= headroom + 1e-12);
        // Grazing alone could add at most η_m times its saturated request; more than that
        // proves the scavenging channel ran too.
        let org = &world.config().organism;
        let k_p = org.intake_half_saturation;
        let grazing_only =
            org.assimilation_material * (0.0025 * founder_diet()) * (1.0 / (1.0 + k_p));
        assert!(
            o.reserve - before > grazing_only,
            "{} vs {grazing_only}",
            o.reserve - before
        );
        assert!(o.fed_this_tick);
    }

    #[test]
    fn energy_never_goes_negative_while_starving() {
        let mut cfg = config();
        cfg.producer.growth = 0.0;
        cfg.producer.initial_fraction = 0.0;
        // No initial litter either (`detritus.initial_dark`): a foodless world has nothing
        // to scavenge.
        cfg.detritus.initial_dark = 0.0;
        // Dry: standing in a pool slows a body (`design/water.md` "Wading") and so cuts its
        // movement cost, which stretches starvation past this test's five minutes. The test
        // is about the audit while starving, not about pools.
        cfg.water.rain_rate = 0.0;
        let mut world = World::new(cfg).expect("valid");
        let opening = stored_energy(&world.state);
        for _ in 0..6000 {
            world.step();
            for (_, o) in world.state.organisms.iter() {
                assert!(o.energy >= 0.0, "energy {} went negative", o.energy);
            }
            if world.population() == 0 {
                break;
            }
        }
        assert_eq!(world.population(), 0, "a foodless world must empty out");
        assert!(
            world.state.deaths_total[0] > 0,
            "starvation is the cause: {:?}",
            world.state.deaths_total
        );
        assert!(world.mass_residual().abs() < 1e-9);
        let closing = stored_energy(&world.state);
        let booked = world.state.net_energy_in_corrected();
        let drift = (closing - opening) - booked;
        println!(
            "starvation audit: opening {opening:e} closing {closing:e} booked {booked:e} drift {drift:e}"
        );
        assert!(drift.abs() < 1e-9 * opening.max(1.0), "drift {drift:e}");
    }

    #[test]
    fn life_events_account_for_every_birth_and_death() {
        let cfg = config();
        // Budding needs the age gate, then a full gestation, before a child appears.
        let earliest_parent_age =
            ((cfg.drives.bud_min_age_seconds + cfg.organism.gestation_seconds) / DT).floor() as u64;
        let mut world = World::new(cfg).expect("valid");

        let mut births = 0u64;
        let mut deaths = 0u64;
        let mut seen: Vec<OrganismId> = Vec::new();
        for _ in 0..12_000 {
            world.step();
            for event in world.drain_events() {
                assert_eq!(
                    event.tick(),
                    world.tick(),
                    "an event is dated off its commit tick"
                );
                match event {
                    LifeEvent::Birth {
                        id,
                        parent,
                        parent_age_ticks,
                        parent_births,
                        origin,
                        genome,
                        ..
                    } => {
                        assert_ne!(id, parent);
                        assert_eq!(origin, Origin::Descendant, "founders emit no birth event");
                        assert!(parent_births >= 1, "the parent's own birth is counted");
                        assert!(
                            parent_age_ticks >= earliest_parent_age,
                            "parent aged {parent_age_ticks} ticks cannot have gestated yet"
                        );
                        assert_ne!(genome, 0);
                        assert_eq!(
                            world.state.organisms.get(id).map(|o| o.parent),
                            Some(Some(parent))
                        );
                        assert!(!seen.contains(&id), "organism id {id:?} was born twice");
                        seen.push(id);
                        births += 1;
                    }
                    LifeEvent::Death {
                        id,
                        age_ticks,
                        births: had,
                        ..
                    } => {
                        assert!(age_ticks > 0);
                        assert!(
                            world.state.organisms.get(id).is_none(),
                            "a dead organism is gone"
                        );
                        let _ = had;
                        deaths += 1;
                    }
                }
            }
            // A second drain in the same tick yields nothing.
            assert!(world.drain_events().is_empty());
        }

        assert!(
            births > 0 && deaths > 0,
            "the run produced {births} births and {deaths} deaths"
        );
        assert_eq!(
            births, world.state.births_total,
            "birth events do not match births_total"
        );
        assert_eq!(
            deaths,
            world.state.deaths_total.iter().sum::<u64>(),
            "death events do not match deaths_total"
        );
        assert!(world.drain_events().is_empty());
    }

    #[test]
    fn the_field_dump_and_cell_graph_describe_every_cell() {
        let mut world = World::new(config()).expect("valid");
        world.step();
        let dump = world.field_dump();
        assert_eq!(dump.tick, 1);
        assert_eq!(dump.n, world.state.fields.n);
        assert_eq!(dump.p, world.state.fields.p);
        assert_eq!(dump.d, world.state.fields.d);
        assert_eq!(dump.de, world.state.fields.de);
        assert_eq!(dump.organisms.len(), CELL_COUNT);
        let counted: u32 = dump.organisms.iter().map(|&c| u32::from(c)).sum();
        assert_eq!(
            counted,
            world.population() as u32,
            "every organism is counted once"
        );
        for (_, o) in world.state.organisms.iter() {
            assert!(dump.organisms[cell_of(&o.pos).index()] > 0);
        }

        let neighbors = world.cell_neighbors();
        assert_eq!(neighbors.len(), CELL_COUNT);
        // The open rim leaves 64 cells with three neighbors; everyone else has four.
        let rim = neighbors
            .iter()
            .filter(|n| n.iter().any(Option::is_none))
            .count();
        assert_eq!(rim, 64);
        for (i, n) in neighbors.iter().enumerate() {
            for &there in n.iter().flatten() {
                assert!(
                    neighbors[usize::from(there)]
                        .iter()
                        .flatten()
                        .any(|&back| usize::from(back) == i),
                    "cell {i} -> {there} is not reciprocal"
                );
            }
        }
    }

    #[test]
    fn from_state_rebuilds_and_zeroes_the_residual() {
        let mut world = World::new(config()).expect("valid");
        for _ in 0..20 {
            world.step();
        }
        let state = world.state.clone();
        let reloaded = World::from_state(state).expect("a stepped state is valid");
        assert!(reloaded.mass_residual().abs() < 1e-12);
        assert_eq!(reloaded.tick(), world.tick());
        assert_eq!(reloaded.population(), world.population());
    }

    /// `design/water.md`: the water budget is an exact identity for a world created dry,
    /// `Σw == rain_in_total − evap_out_total`, checked every tick and cumulatively.
    #[test]
    fn the_water_budget_closes_every_tick_and_cumulatively() {
        let mut world = World::new(config()).expect("valid");
        assert_eq!(
            world.state.fields.w.iter().sum::<f64>(),
            0.0,
            "a new world is dry"
        );
        let mut worst: f64 = 0.0;
        for _ in 0..2000 {
            let before: f64 = world.state.fields.w.iter().sum();
            let (rain, evap) = (world.state.rain_in_total, world.state.evap_out_total);
            world.step();
            let booked = (world.state.rain_in_total - rain) - (world.state.evap_out_total - evap);
            let drift = (world.state.fields.w.iter().sum::<f64>() - before) - booked;
            worst = worst.max(drift.abs());
        }
        assert!(worst < 1e-9, "worst per-tick water drift {worst:e}");
        let total: f64 = world.state.fields.w.iter().sum();
        assert!(
            world.water_residual().abs() < 1e-9 * total.max(1.0),
            "residual {:e}",
            world.water_residual()
        );
        assert!(
            world.state.rain_in_total > 0.0,
            "it must have rained somewhere in 100 s"
        );
        assert!(world.state.evap_out_total > 0.0);
        assert!(total > 0.0);
        // The view and telemetry carry the same water.
        let view = world.render_view();
        assert_eq!(view.water, world.state.fields.w);
        assert_eq!(view.rain.len(), CELL_COUNT);
        let sample = world.telemetry();
        assert!((sample.water - total).abs() < 1e-12);
        assert!((sample.water_by_face.iter().sum::<f64>() - total).abs() < 1e-9);
    }

    /// `design/water.md` "Wading": an organism's speed is divided by `1 + w` of its cell.
    #[test]
    fn wading_halves_the_speed_at_unit_depth() {
        fn traveled(depth: f64) -> f64 {
            let mut cfg = config();
            cfg.founders.count = 1;
            // Freeze the water so the depth the organism wades through is exactly `depth`.
            cfg.water.rain_rate = 0.0;
            cfg.water.flow = 0.0;
            cfg.water.evap = 0.0;
            let mut world = World::new(cfg).expect("valid");
            let id = world
                .state
                .organisms
                .iter()
                .map(|(id, _)| id)
                .next()
                .expect("founder");
            let cell = {
                let o = world.state.organisms.get_mut(id).expect("alive");
                o.mode = Mode::Seeking;
                o.hunger_memory = 1.0;
                o.reserve = 0.0;
                cell_of(&o.pos)
            };
            world.state.fields.w[cell.index()] = depth;
            // Keep the cell's food below the feeding threshold so the mode stays Seeking.
            world.state.fields.p[cell.index()] = 0.0;
            world.state.fields.d[cell.index()] = 0.0;
            world.step();
            let view = world.render_view();
            view.organisms[0]
                .moved
                .iter()
                .map(|s| s.length())
                .sum::<f64>()
        }
        let dry = traveled(0.0);
        let wading = traveled(1.0);
        let deep = traveled(3.0);
        assert!(dry > 0.0, "a seeking organism moves");
        assert!(
            (dry / wading - 2.0).abs() < 1e-9,
            "dry {dry} wading {wading}"
        );
        assert!((dry / deep - 4.0).abs() < 1e-9, "dry {dry} deep {deep}");
    }

    /// Not a test of anything: prints where the water stands after two simulated hours of a
    /// default world (soil / foliage / canopy by cell height, the soil floor row, the top
    /// face's wettest cells), for the short-run report in `design/water.md`'s slice.
    /// `cargo test -p cubarium-core --release --lib water_by_band -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn report_water_by_band_after_two_hours() {
        let seed: u64 = std::env::var("CUBARIUM_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        let mut cfg = config();
        cfg.seed = seed;
        let mut world = World::new(cfg).expect("valid");
        let mut pop_min = world.population();
        // Ponds on the level top face come and go with the showers, so sample them over
        // time as well as at the end: how often at least one interior top cell holds more
        // than 0.5, and the most such cells seen at once.
        let (mut top_pond_ticks, mut top_pond_peak, mut samples) = (0u64, 0usize, 0u64);
        for tick in 0..(7200.0 / DT) as u64 {
            world.step();
            pop_min = pop_min.min(world.population());
            if tick % 20 == 0 {
                samples += 1;
                let ponds = CellId::all()
                    .filter(|c| {
                        c.face() == Face::Top
                            && (1..15).contains(&c.cx())
                            && (1..15).contains(&c.cy())
                    })
                    .filter(|c| world.state.fields.w[c.index()] > 0.5)
                    .count();
                if ponds > 0 {
                    top_pond_ticks += 1;
                }
                top_pond_peak = top_pond_peak.max(ponds);
            }
        }
        println!(
            "top-face interior ponds (>0.5) over the run: present in {:.0}% of once-a-second samples, peak {top_pond_peak} cells at once",
            100.0 * top_pond_ticks as f64 / samples as f64
        );
        let w = &world.state.fields.w;
        let (mut soil, mut foliage, mut canopy, mut floor) =
            ((0.0, 0), (0.0, 0), (0.0, 0), (0.0, 0));
        let mut top_cells: Vec<(f64, CellId)> = Vec::new();
        for cell in CellId::all() {
            let h = cell.center().embed()[1];
            let i = cell.index();
            if cell.face() == Face::Top {
                canopy.0 += w[i];
                canopy.1 += 1;
                top_cells.push((w[i], cell));
            } else if h < -0.33 {
                soil.0 += w[i];
                soil.1 += 1;
                if cell.cy() == 15 {
                    floor.0 += w[i];
                    floor.1 += 1;
                }
            } else {
                foliage.0 += w[i];
                foliage.1 += 1;
            }
        }
        let total: f64 = w.iter().sum();
        top_cells.sort_by(|a, b| b.0.total_cmp(&a.0));
        let pools_floor = (0..64).filter(|_| true).count();
        let _ = pools_floor;
        let floor_cells: Vec<f64> = CellId::all()
            .filter(|c| c.face() != Face::Top && c.cy() == 15)
            .map(|c| w[c.index()])
            .collect();
        let floor_pools = floor_cells.iter().filter(|&&x| x > 0.5).count();
        let floor_dry = floor_cells.iter().filter(|&&x| x < 0.3).count();
        let floor_max = floor_cells.iter().cloned().fold(0.0, f64::max);
        let floor_min = floor_cells.iter().cloned().fold(f64::MAX, f64::min);
        let top_pools = top_cells.iter().filter(|(x, _)| *x > 0.5).count();
        println!(
            "water after 2 h (seed {seed}): total {total:.2}; soil {:.2} ({:.0}%), foliage {:.2} ({:.0}%), canopy {:.2} ({:.0}%)",
            soil.0,
            100.0 * soil.0 / total,
            foliage.0,
            100.0 * foliage.0 / total,
            canopy.0,
            100.0 * canopy.0 / total
        );
        println!(
            "soil floor row (64 cells): {:.2} total, mean {:.3}, min {floor_min:.3}, max {floor_max:.3}, {floor_pools} cells deeper than 0.5, {floor_dry} cells under 0.3 (dry gaps)",
            floor.0,
            floor.0 / floor.1 as f64
        );
        println!("population: min {pop_min}, end {}", world.population());
        println!(
            "top face: mean {:.3}, {top_pools} cells deeper than 0.5, wettest {:?}",
            canopy.0 / canopy.1 as f64,
            top_cells
                .iter()
                .take(3)
                .map(|(x, c)| (format!("{x:.3}"), c.cx(), c.cy()))
                .collect::<Vec<_>>()
        );
        println!(
            "budget: rain_in {:.2} evap_out {:.2} residual {:e}; population {}",
            world.state.rain_in_total,
            world.state.evap_out_total,
            world.water_residual(),
            world.population()
        );
    }

    // --- Fauna v2 (`design/fauna-v2.md`) -------------------------------------------------

    /// One default-kind founder of each kind, by `form`.
    fn one_of_each_kind(world: &World) -> Vec<Organism> {
        let mut out = Vec::new();
        for form in [2u8, 0, 1, 3] {
            out.push(
                world
                    .state
                    .organisms
                    .iter()
                    .find(|(_, o)| o.phenotype.form == form)
                    .map(|(_, o)| o.clone())
                    .expect("a founder of each kind"),
            );
        }
        out
    }

    #[test]
    fn the_default_kinds_place_twenty_four_founders_with_the_tables_genomes() {
        let world = World::new(WorldConfig::default()).expect("valid");
        assert_eq!(world.population(), 24, "4 + 10 + 5 + 5 founders");
        let mut by_form = [0usize; 8];
        for (_, o) in world.state.organisms.iter() {
            by_form[o.phenotype.form as usize] += 1;
            assert_eq!(o.genome.version, Genome::VERSION);
        }
        assert_eq!(
            by_form[..4],
            [10, 5, 4, 5],
            "lantern 0 = grazer, sail 1 = glider, mossback 2 = burrower, skimmer 3"
        );
        let Ok([burrower, grazer, glider, skimmer]) =
            <[Organism; 4]>::try_from(one_of_each_kind(&world))
        else {
            panic!("four kinds");
        };
        let g = |o: &Organism| {
            (
                o.genome.diet,
                o.genome.depth,
                o.genome.speed,
                o.genome.size,
                o.genome.swim,
                o.genome.hue,
            )
        };
        assert_eq!(g(&burrower), (0.10, 0.10, 0.6, 1.0, 0.0, 0.15));
        assert_eq!(g(&grazer), (0.85, 0.55, 1.0, 1.0, 0.0, 0.50));
        assert_eq!(g(&glider), (0.90, 1.00, 1.0, 1.0, 0.0, 0.85));
        assert_eq!(g(&skimmer), (0.60, 0.10, 0.9, 0.9, 1.0, 0.65));
        // Unnamed loci keep the v1 founder values, and the phenotype carries the kind.
        assert_eq!(
            (
                burrower.genome.metabolism,
                burrower.genome.mouth,
                burrower.genome.reserve
            ),
            (0.7, 1.0, 1.0),
            "the burrower kind fixes metabolism; the rest stay v1"
        );
        assert_eq!(grazer.genome.metabolism, 1.0);
        assert_eq!(
            burrower.genome.sense,
            WorldConfig::default().organism.sense_radius as f32
        );
        assert!((glider.phenotype.h_pref - 1.0).abs() < 1e-12);
        assert!((burrower.phenotype.h_pref + 0.8).abs() < 1e-6);
        assert_eq!(skimmer.phenotype.swim, 1.0);
        assert!(
            (grazer.phenotype.graze_rate - 0.85f32 as f64 * grazer.phenotype.mouth_rate).abs()
                < 1e-12
        );
        // The founders' material is booked and the world is consistent.
        world
            .check_invariants()
            .expect("a fresh kinds world is consistent");
        assert!(world.mass_residual().abs() < 1e-12);
    }

    #[test]
    fn an_empty_kind_list_falls_back_to_v1_founders_and_kinds_are_deterministic() {
        let v1 = World::new(config()).expect("valid");
        assert_eq!(v1.population(), config().founders.count as usize);
        for (_, o) in v1.state.organisms.iter() {
            assert_eq!(
                (o.genome.diet, o.genome.depth, o.genome.swim),
                (0.7, 0.5, 0.0)
            );
            assert_eq!(
                o.genome.form,
                crate::genome::form_of_hue(o.genome.hue),
                "v1 founders take the hue tercile"
            );
        }
        let a = World::new(WorldConfig::default()).expect("valid");
        let b = World::new(WorldConfig::default()).expect("valid");
        assert_eq!(
            a.state, b.state,
            "two kinds worlds from one seed are identical"
        );
        // A kind's own draws do not move when another kind's count changes.
        let mut fewer = WorldConfig::default();
        fewer.founders.kinds[0].count = 2;
        let c = World::new(fewer).expect("valid");
        let gliders = |w: &World| {
            let mut v: Vec<(u32, SurfacePoint)> = w
                .state
                .organisms
                .iter()
                .filter(|(_, o)| o.phenotype.form == 1)
                .map(|(id, o)| (id.slot, o.pos))
                .collect();
            v.sort_by_key(|x| x.0);
            v.into_iter().map(|x| x.1).collect::<Vec<_>>()
        };
        assert_eq!(
            gliders(&a),
            gliders(&c),
            "glider placements are their own stream"
        );
    }

    #[test]
    fn a_v1_genome_is_upgraded_in_place_on_load() {
        let mut world = World::new(config()).expect("valid");
        for _ in 0..5 {
            world.step();
        }
        let mut state = world.state.clone();
        for (_, o) in state.organisms.iter_mut() {
            o.genome.version = 1;
            o.genome.form = crate::genome::FORM_UNSET;
        }
        let reloaded =
            World::from_state(state).expect("a v1-genome state is upgraded, not refused");
        for (_, o) in reloaded.state.organisms.iter() {
            assert_eq!(o.genome.version, Genome::VERSION);
            assert_eq!(o.genome.form, crate::genome::form_of_hue(o.genome.hue));
            assert_eq!(
                o.phenotype.form, o.genome.form,
                "the phenotype is re-decoded"
            );
        }
    }

    /// A single organism on a frozen cell of a kinds-free world, with the given genome
    /// loci, its cell stocked as asked. Returns the world, the id and the cell index.
    fn frozen_feeder(diet: f32, p: f64, f: f64, d: f64, de: f64) -> (World, OrganismId, usize) {
        let mut cfg = config();
        cfg.founders.count = 1;
        cfg.producer.growth = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.detritus.fall = 0.0;
        cfg.nutrient.diffusion = 0.0;
        cfg.fruit.ripen = 0.0;
        cfg.fruit.drop = 0.0;
        cfg.water.rain_rate = 0.0;
        cfg.mechanisms.mutation = false;
        let mut world = World::new(cfg).expect("valid");
        let id = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("one founder");
        let cell = CellId::new(Face::Front, 4, 4);
        {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.genome.diet = diet;
            o.phenotype = decode(&o.genome, &world.state.config.organism);
            o.pos = cell.center();
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
        }
        let c = cell.index();
        world.state.fields.p[c] = p;
        world.state.fields.f[c] = f;
        world.state.fields.d[c] = d;
        world.state.fields.de[c] = de;
        (world, id, c)
    }

    #[test]
    fn frugivory_comes_first_and_the_diet_gates_hold() {
        // A pure grazer on a cell with fruit and producer eats the fruit first: with
        // linear intake and headroom for exactly one mouthful, the fruit bite fills it and
        // the producer, whose request comes second, gets nothing.
        let (mut world, id, c) = frozen_feeder(1.0, 1.0, 1.0, 1.0, 1.0);
        world.state.config.organism.intake_half_saturation = 0.0;
        {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.reserve = o.phenotype.reserve_max - o.phenotype.graze_rate * DT;
        }
        let (p0, f0, d0) = (
            world.state.fields.p[c],
            world.state.fields.f[c],
            world.state.fields.d[c],
        );
        // The hand-stocked cell moved the residual once; the step must not move it again.
        let residual = world.mass_residual();
        let before = stored_energy(&world.state);
        let (light, heat) = (world.state.light_in_total, world.state.heat_out_total);
        world.step();
        let o = world.state.organisms.get(id).expect("alive");
        assert_eq!(o.mode, Mode::Feeding);
        assert!(o.fed_this_tick);
        let q = f0 - world.state.fields.f[c];
        assert!(q > 0.0, "fruit was eaten");
        assert!(
            (q - o.phenotype.graze_rate * DT).abs() < 1e-12,
            "a full fruit mouthful {q}"
        );
        assert_eq!(
            world.state.fields.p[c], p0,
            "the producer waited its turn and got nothing"
        );
        // Frugivory: η_m of the bite to reserve, the rest to detritus; scavenging is gated off
        // for a pure grazer, so detritus only grew.
        let eta_m = world.config().organism.assimilation_material;
        let r0 = o.phenotype.reserve_max - o.phenotype.graze_rate * DT;
        assert!((o.reserve - (r0 + eta_m * q)).abs() < 1e-12);
        assert!((world.state.fields.d[c] - (d0 + q - eta_m * q)).abs() < 1e-12);
        let booked = (world.state.light_in_total - light) - (world.state.heat_out_total - heat);
        assert!(
            ((stored_energy(&world.state) - before) - booked).abs() < 1e-9,
            "fruit energy is audited"
        );
        assert!(
            (world.mass_residual() - residual).abs() < 1e-12,
            "frugivory conserves material"
        );

        // A pure scavenger never grazes or eats fruit, however rich the cell.
        let (mut world, id, c) = frozen_feeder(0.0, 1.0, 1.0, 0.0, 0.0);
        world.step();
        let o = world.state.organisms.get(id).expect("alive");
        assert_eq!(
            o.mode,
            Mode::Seeking,
            "no detritus, and leaf is not its food"
        );
        assert_eq!(
            (world.state.fields.p[c], world.state.fields.f[c]),
            (1.0, 1.0)
        );
        assert_eq!(o.reserve, 0.0);

        // A pure grazer never scavenges.
        let (mut world, id, c) = frozen_feeder(1.0, 0.0, 0.0, 1.0, 1.0);
        world.step();
        let o = world.state.organisms.get(id).expect("alive");
        assert_eq!(o.mode, Mode::Seeking);
        assert_eq!(world.state.fields.d[c], 1.0);
        assert_eq!(o.reserve, 0.0);

        // An omnivore below the fruit diet leaves fruit alone but grazes.
        let (mut world, id, c) = frozen_feeder(0.4, 1.0, 1.0, 0.0, 0.0);
        world.step();
        let o = world.state.organisms.get(id).expect("alive");
        assert_eq!(o.mode, Mode::Feeding);
        assert_eq!(world.state.fields.f[c], 1.0, "fruit needs diet ≥ 0.5");
        assert!(world.state.fields.p[c] < 1.0);
    }

    #[test]
    fn fruit_is_conserved_material_over_a_default_run() {
        let mut world = World::new(WorldConfig::default()).expect("valid");
        let opening = stored_energy(&world.state);
        let mut worst: f64 = 0.0;
        for _ in 0..2000 {
            let before = stored_energy(&world.state);
            let ledgers = world.state.energy_ledgers();
            world.step();
            let booked = world.state.energy_ledgers().net_since(ledgers);
            worst = worst.max(((stored_energy(&world.state) - before) - booked).abs());
            assert!(
                world.mass_residual().abs() < 1e-9,
                "mass residual {}",
                world.mass_residual()
            );
        }
        assert!(
            worst < 1e-9,
            "worst per-tick energy drift {worst:e} with fruit in the sum"
        );
        let total_fruit: f64 = world.state.fields.f.iter().sum();
        assert!(
            total_fruit > 0.0,
            "a lit default world ripens some fruit in 100 s"
        );
        let booked = world.state.net_energy_in_corrected();
        let overall = (stored_energy(&world.state) - opening) - booked;
        assert!(
            overall.abs() < 1e-9 * stored_energy(&world.state).max(1.0),
            "cumulative {overall:e}"
        );
        // The view, the dump and telemetry all carry the same fruit.
        let view = world.render_view();
        assert_eq!(view.fruit, world.state.fields.f);
        assert_eq!(world.field_dump().f, world.state.fields.f);
        let sample = world.telemetry();
        assert!((sample.fruit - total_fruit).abs() < 1e-12);
    }

    #[test]
    fn the_depth_term_points_up_the_side_faces_and_vanishes_on_top() {
        assert_eq!(up_direction(Face::Top), Vec2::ZERO);
        for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
            let up = up_direction(face);
            assert!(
                (up - Vec2::new(0.0, -1.0)).length() < 1e-12,
                "{face:?}: {up:?}"
            );
            // Moving along `up` really raises the embedded height.
            let low = SurfacePoint::new(face, 32.0, 40.0);
            let higher = SurfacePoint::new(face, 32.0 + up.x * 4.0, 40.0 + up.y * 4.0);
            assert!(higher.embed()[1] > low.embed()[1]);
        }
        // A canopy-bound organism low on a wall heads up; a soil-bound one high up heads down.
        // No food anywhere (no producers, no litter), so nothing stops it to feed on the way.
        let mut cfg = config();
        cfg.founders.count = 1;
        cfg.producer.initial_fraction = 0.0;
        cfg.producer.growth = 0.0;
        cfg.detritus.initial_dark = 0.0;
        cfg.water.rain_rate = 0.0;
        cfg.drives.w_persist = 0.0;
        cfg.drives.turn_noise = 0.0;
        for (depth, expect_dy_sign) in [(1.0f32, -1.0f64), (0.0, 1.0)] {
            let mut world = World::new(cfg.clone()).expect("valid");
            let id = world
                .state
                .organisms
                .iter()
                .map(|(id, _)| id)
                .next()
                .expect("founder");
            let start = SurfacePoint::new(Face::Front, 32.0, 32.0);
            {
                let o = world.state.organisms.get_mut(id).expect("alive");
                o.genome.depth = depth;
                o.genome.drives.w_persist = 0.0;
                o.genome.drives.turn_noise = 0.0;
                o.phenotype = decode(&o.genome, &world.state.config.organism);
                o.pos = start;
                o.heading = Vec2::new(1.0, 0.0);
                o.reserve = 0.0;
                o.hunger_memory = 1.0;
                o.mode = Mode::Seeking;
            }
            for _ in 0..200 {
                world.step();
            }
            let o = world.state.organisms.get(id).expect("alive");
            let dv = o.pos.v - start.v;
            assert!(
                dv * expect_dy_sign > 0.5,
                "depth {depth}: moved {dv} in v on Front (expected sign {expect_dy_sign})"
            );
            assert!(
                (o.heading.y * expect_dy_sign) > 0.9,
                "heading {:?} settled toward the band",
                o.heading
            );
        }
    }

    #[test]
    fn sensing_reaches_the_configured_depth_and_finds_food_two_cells_out() {
        assert_eq!(sense_depth(6.0), 2);
        assert_eq!(sense_depth(4.0), 1);
        assert_eq!(sense_depth(12.0), 3);
        assert_eq!(sense_depth(0.0), 1);
        let rings = sense_rings(&FieldGraph::new());
        let origin = CellId::new(Face::Front, 8, 8);
        assert_eq!(rings[origin.index()][0].len(), 4);
        assert_eq!(rings[origin.index()][1].len(), 8);
        assert_eq!(rings[origin.index()][2].len(), 12);
        for (d, ring) in rings[origin.index()].iter().enumerate() {
            for c in ring {
                let dist = (i32::from(c.cx()) - 8).abs() + (i32::from(c.cy()) - 8).abs();
                assert_eq!(
                    dist as usize,
                    d + 1,
                    "{c:?} is not at graph distance {}",
                    d + 1
                );
            }
        }
        // A seam-adjacent cell's rings cross onto the neighbouring face and never the rim.
        let corner = CellId::new(Face::Front, 15, 15);
        assert!(
            rings[corner.index()][0]
                .iter()
                .any(|c| c.face() == Face::Right)
        );
        assert!(
            rings[corner.index()]
                .iter()
                .flatten()
                .all(|c| c.face() != Face::Top)
        );

        // Food two cells away, none adjacent: a 6 px sensor turns toward it; a 4 px one
        // sees a flat neighbourhood and holds its heading.
        let mut cfg = config();
        cfg.founders.count = 1;
        cfg.producer.initial_fraction = 0.0;
        cfg.producer.growth = 0.0;
        // Freeze everything that could leak a trace of the rich cell into the one-hop
        // neighbourhood before the observation (mortality → detritus → fall; ripening):
        // gradients are normalized, so any nonzero difference steers at full strength.
        cfg.producer.mortality = 0.0;
        cfg.detritus.fall = 0.0;
        cfg.detritus.initial_dark = 0.0;
        cfg.fruit.ripen = 0.0;
        cfg.water.rain_rate = 0.0;
        for (sense, turns) in [(6.0f32, true), (4.0, false)] {
            let mut world = World::new(cfg.clone()).expect("valid");
            let id = world
                .state
                .organisms
                .iter()
                .map(|(id, _)| id)
                .next()
                .expect("founder");
            let here = CellId::new(Face::Front, 8, 8);
            {
                let o = world.state.organisms.get_mut(id).expect("alive");
                o.genome.sense = sense;
                o.genome.depth = 0.5;
                o.genome.drives.w_persist = 0.0;
                o.genome.drives.turn_noise = 0.0;
                o.genome.drives.w_depth = 0.0;
                o.phenotype = decode(&o.genome, &world.state.config.organism);
                o.pos = here.center();
                o.heading = Vec2::new(1.0, 0.0);
                o.reserve = 0.0;
                o.hunger_memory = 1.0;
                o.mode = Mode::Seeking;
            }
            // Rich cells straight "up" the chart, two hops away, nothing at one hop.
            world.state.fields.p[CellId::new(Face::Front, 8, 6).index()] = 1.5;
            world.step();
            let o = world.state.organisms.get(id).expect("alive");
            if turns {
                assert!(
                    o.heading.y < -0.05,
                    "a 6 px sensor turned toward food two cells up: {:?}",
                    o.heading
                );
            } else {
                assert!(
                    (o.heading - Vec2::new(1.0, 0.0)).length() < 1e-9,
                    "a 4 px sensor saw nothing: {:?}",
                    o.heading
                );
            }
        }
    }

    #[test]
    fn a_swimmer_ignores_pools_while_a_wader_is_slowed() {
        fn traveled(swim: f32, depth: f64) -> f64 {
            let mut cfg = config();
            cfg.founders.count = 1;
            cfg.water.rain_rate = 0.0;
            cfg.water.flow = 0.0;
            cfg.water.evap = 0.0;
            let mut world = World::new(cfg).expect("valid");
            let id = world
                .state
                .organisms
                .iter()
                .map(|(id, _)| id)
                .next()
                .expect("founder");
            let cell = {
                let o = world.state.organisms.get_mut(id).expect("alive");
                o.genome.swim = swim;
                o.phenotype = decode(&o.genome, &world.state.config.organism);
                o.mode = Mode::Seeking;
                o.hunger_memory = 1.0;
                o.reserve = 0.0;
                cell_of(&o.pos)
            };
            world.state.fields.w[cell.index()] = depth;
            world.state.fields.p[cell.index()] = 0.0;
            world.state.fields.f[cell.index()] = 0.0;
            world.state.fields.d[cell.index()] = 0.0;
            world.step();
            world.render_view().organisms[0]
                .moved
                .iter()
                .map(|s| s.length())
                .sum::<f64>()
        }
        let dry = traveled(0.0, 0.0);
        assert!(
            (dry / traveled(0.0, 1.0) - 2.0).abs() < 1e-9,
            "a wader halves at unit depth"
        );
        assert!(
            (traveled(1.0, 1.0) - dry).abs() < 1e-12,
            "a swimmer moves as if dry"
        );
        assert!((traveled(1.0, 3.0) - dry).abs() < 1e-12);
        assert!(
            (dry / traveled(0.5, 1.0) - 1.5).abs() < 1e-9,
            "half a swimmer wades at 1 + w/2"
        );
    }

    #[test]
    fn mutation_records_its_loci_and_never_touches_form() {
        let mut cfg = WorldConfig::default();
        cfg.mutation.probability = 1.0;
        let mut world = World::new(cfg).expect("valid");
        let (mut births, mut mutated, mut loci) = (0u64, 0u64, std::collections::HashSet::new());
        for _ in 0..24_000 {
            world.step();
            for event in world.drain_events() {
                if let LifeEvent::Birth {
                    id,
                    parent,
                    mutations,
                    ..
                } = event
                {
                    births += 1;
                    let child = world.state.organisms.get(id).expect("newborn").clone();
                    // Copies are exact where no mutation is recorded, and the parent (if it
                    // still lives) shares the child's rig whatever else changed.
                    if let Some(p) = world.state.organisms.get(parent) {
                        assert_eq!(child.genome.form, p.genome.form, "form must never mutate");
                        assert_eq!(child.phenotype.form, p.phenotype.form);
                    }
                    if !mutations.is_empty() {
                        mutated += 1;
                    }
                    for m in &mutations {
                        assert!(
                            crate::genome::MUTABLE_LOCI.contains(&m.locus),
                            "{} is not mutable",
                            m.locus
                        );
                        assert_ne!(m.from, m.to);
                        loci.insert(m.locus);
                    }
                    assert!(
                        mutations.len() <= 2,
                        "at most two loci per birth: {mutations:?}"
                    );
                    let mut g = child.genome.clone();
                    assert!(!g.clamp(), "a mutated child is always in range");
                }
            }
        }
        assert!(births >= 20, "the run produced {births} births");
        assert!(
            mutated as f64 >= 0.8 * births as f64,
            "with p_mut = 1 nearly every child differs ({mutated}/{births})"
        );
        assert!(
            loci.len() >= 5,
            "many different loci were touched: {loci:?}"
        );
        world
            .check_invariants()
            .expect("mutated worlds stay consistent");

        // With mutation off every child is an exact copy of its parent's escrowed genome.
        let mut cfg = WorldConfig::default();
        cfg.mechanisms.mutation = false;
        let mut world = World::new(cfg).expect("valid");
        let mut seen = 0;
        for _ in 0..24_000 {
            world.step();
            for event in world.drain_events() {
                if let LifeEvent::Birth {
                    id,
                    parent,
                    mutations,
                    ..
                } = event
                {
                    assert!(mutations.is_empty());
                    if let Some(p) = world.state.organisms.get(parent) {
                        assert_eq!(
                            world.state.organisms.get(id).expect("newborn").genome,
                            p.genome
                        );
                        seen += 1;
                    }
                }
            }
        }
        assert!(seen > 0, "some exact copies were compared");
    }

    #[test]
    fn telemetry_counts_each_form_and_its_mean_height() {
        let mut world = World::new(WorldConfig::default()).expect("valid");
        world.step();
        let sample = world.telemetry();
        assert_eq!(
            sample.population_by_form.iter().sum::<u32>(),
            sample.population
        );
        assert_eq!(sample.population_by_form[..4], [10, 5, 4, 5]);
        for form in 0..4 {
            let expected: f64 = world
                .state
                .organisms
                .iter()
                .filter(|(_, o)| o.phenotype.form == form as u8)
                .map(|(_, o)| o.pos.embed()[1])
                .sum::<f64>()
                / f64::from(sample.population_by_form[form]);
            assert!((sample.mean_height_by_form[form] - expected).abs() < 1e-12);
            assert!(sample.mean_height_by_form[form].abs() <= 1.0);
        }
        assert_eq!(
            sample.mean_height_by_form[7], 0.0,
            "an empty form reports zero"
        );
        let view = world.render_view();
        assert!(view.organisms.iter().all(|o| o.form < 4));
    }

    /// Not a test of anything: the fauna v2 short-run reporter for the slice report.
    /// `CUBARIUM_SEED=1 CUBARIUM_HOURS=2 cargo test -p cubarium-core --release --lib
    /// report_fauna_by_form -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn report_fauna_by_form_after_a_short_run() {
        let seed: u64 = std::env::var("CUBARIUM_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        let hours: f64 = std::env::var("CUBARIUM_HOURS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2.0);
        let mut cfg = WorldConfig {
            seed,
            ..WorldConfig::default()
        };
        // Diagnostic counterfactual only (never a default): `CUBARIUM_ENERGY_CAP` overrides
        // `detritus.energy_cap`, which sets how edible detritus can be.
        if let Some(cap) = std::env::var("CUBARIUM_ENERGY_CAP")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
        {
            cfg.detritus.energy_cap = cap;
            println!("counterfactual: detritus.energy_cap = {cap}");
        }
        let e_r = cfg.organism.reserve_energy_density;
        let mut world = World::new(cfg).expect("valid");
        let names = [
            "lantern/grazer",
            "sail/glider",
            "mossback/burrower",
            "skimmer",
        ];
        // Edible litter on the soil floor at tick 0, against the feeding threshold.
        {
            let f = &world.state.fields;
            let floor: Vec<f64> = CellId::all()
                .filter(|c| c.face() != Face::Top && c.cy() == 15)
                .map(|c| edible_detritus(f.d[c.index()], f.de[c.index()], e_r))
                .collect();
            let mean = floor.iter().sum::<f64>() / floor.len() as f64;
            let max = floor.iter().cloned().fold(0.0, f64::max);
            let feed_min = world.config().drives.feed_min;
            println!(
                "tick 0 soil floor row: mean D_eff {mean:.3}, max {max:.3}, {} of 64 cells at or above feed_min {feed_min}",
                floor.iter().filter(|&&x| x >= feed_min).count()
            );
        }
        let (mut pop_min, mut pop_max) = (world.population(), world.population());
        let mut min_by_form = [u32::MAX; 4];
        let mut form_of: std::collections::HashMap<OrganismId, u8> = world
            .state
            .organisms
            .iter()
            .map(|(id, o)| (id, o.phenotype.form))
            .collect();
        let mut deaths_by_form = [[0u32; 3]; 4];
        let mut death_time_by_form = [0.0f64; 4];
        let mut death_age_by_form = [0.0f64; 4];
        // `CUBARIUM_TRACE_FORM=<form>` prints that kind's state once a simulated minute for
        // the first half hour: where it is, what it holds, what it stands on.
        let trace: Option<u8> = std::env::var("CUBARIUM_TRACE_FORM")
            .ok()
            .and_then(|s| s.parse().ok());
        let ticks = (hours * 3600.0 / DT) as u64;
        for tick in 0..ticks {
            if let Some(form) = trace
                && tick % 1200 == 0
                && tick <= 36_000
            {
                let f = &world.state.fields;
                let members: Vec<&Organism> = world
                    .state
                    .organisms
                    .iter()
                    .filter(|(_, o)| o.phenotype.form == form)
                    .map(|(_, o)| o)
                    .collect();
                if !members.is_empty() {
                    let n = members.len() as f64;
                    let mean = |g: &dyn Fn(&Organism) -> f64| {
                        members.iter().map(|o| g(o)).sum::<f64>() / n
                    };
                    let modes = members.iter().fold([0; 3], |mut m, o| {
                        m[match o.mode {
                            Mode::Resting => 0,
                            Mode::Seeking => 1,
                            Mode::Feeding => 2,
                        }] += 1;
                        m
                    });
                    println!(
                        "    t {:>4.0}s form {form}: n {} h {:+.2} R/Rmax {:.2} E/Emax {:.2} m_h {:.2} modes rest/seek/feed {:?} D_eff here {:.3} P here {:.3} fed {}",
                        tick as f64 * DT,
                        members.len(),
                        mean(&|o| o.pos.embed()[1]),
                        mean(&|o| o.reserve / o.phenotype.reserve_max),
                        mean(&|o| o.energy / o.phenotype.energy_max),
                        mean(&|o| o.hunger_memory),
                        modes,
                        mean(&|o| {
                            let c = cell_of(&o.pos).index();
                            edible_detritus(f.d[c], f.de[c], e_r)
                        }),
                        mean(&|o| f.p[cell_of(&o.pos).index()]),
                        members.iter().filter(|o| o.fed_this_tick).count(),
                    );
                }
            }
            world.step();
            for event in world.drain_events() {
                match event {
                    LifeEvent::Birth { id, .. } => {
                        if let Some(o) = world.state.organisms.get(id) {
                            form_of.insert(id, o.phenotype.form);
                        }
                    }
                    LifeEvent::Death {
                        id,
                        cause,
                        age_ticks,
                        ..
                    } => {
                        if let Some(&form) = form_of.get(&id)
                            && (form as usize) < 4
                        {
                            let slot = match cause {
                                DeathCause::Starvation => 0,
                                DeathCause::Age => 1,
                                DeathCause::Collapse => 2,
                                // This fixture runs no hunters.
                                DeathCause::Predation => continue,
                            };
                            deaths_by_form[form as usize][slot] += 1;
                            death_time_by_form[form as usize] += (tick + 1) as f64 * DT;
                            death_age_by_form[form as usize] += age_ticks as f64 * DT;
                        }
                    }
                }
            }
            pop_min = pop_min.min(world.population());
            pop_max = pop_max.max(world.population());
            if tick % 100 == 0 {
                let mut by_form = [0u32; 4];
                for (_, o) in world.state.organisms.iter() {
                    if (o.phenotype.form as usize) < 4 {
                        by_form[o.phenotype.form as usize] += 1;
                    }
                }
                for f in 0..4 {
                    min_by_form[f] = min_by_form[f].min(by_form[f]);
                }
            }
        }
        let sample = world.telemetry();
        println!(
            "seed {seed}, {hours} h: population end {} min {pop_min} max {pop_max}; residual {:e}",
            sample.population, sample.mass_residual
        );
        for f in 0..4 {
            let deaths: u32 = deaths_by_form[f].iter().sum();
            let mean_death = if deaths > 0 {
                death_time_by_form[f] / f64::from(deaths) / 60.0
            } else {
                0.0
            };
            let mean_age = if deaths > 0 {
                death_age_by_form[f] / f64::from(deaths) / 60.0
            } else {
                0.0
            };
            println!(
                "  {:<18} end {:>3}  min {:>3}  mean height {:+.3}  starved {:>3} (age/collapse {} / {}), mean age at death {mean_age:.1} min, mean death time {mean_death:.1} min",
                names[f],
                sample.population_by_form[f],
                min_by_form[f],
                sample.mean_height_by_form[f],
                deaths_by_form[f][0],
                deaths_by_form[f][1],
                deaths_by_form[f][2]
            );
        }
        let fields = &world.state.fields;
        let (mut soil, mut foliage, mut canopy) = (0.0, 0.0, 0.0);
        for cell in CellId::all() {
            let f = fields.f[cell.index()];
            if cell.face() == Face::Top {
                canopy += f;
            } else if cell.center().embed()[1] < -0.33 {
                soil += f;
            } else {
                foliage += f;
            }
        }
        let fruiting = fields.f.iter().filter(|&&x| x > 0.15).count();
        let ripe_cells = fields
            .p
            .iter()
            .filter(|&&p| p > world.config().fruit.fruit_min * world.config().producer.max)
            .count();
        let max_f = fields.f.iter().cloned().fold(0.0, f64::max);
        let max_p = fields.p.iter().cloned().fold(0.0, f64::max);
        println!(
            "  fruit total {:.3} ({fruiting} cells above 0.15, max F {max_f:.3}; {ripe_cells} cells with P above fruit_min·P_max, max P {max_p:.3}): soil {:.3} foliage {:.3} canopy {:.3}; P {:.1} D {:.1} N {:.1} water {:.1}",
            sample.fruit,
            soil,
            foliage,
            canopy,
            sample.producer,
            sample.detritus,
            sample.nutrient,
            sample.water
        );
    }
}
