//! One episode on one layout, through the **real** core tick loop.
//!
//! There is no second simulator and no copied neural forward pass. An episode is exactly:
//!
//! 1. [`Layout::build`] — `World::new`, stage the fields and the grazer, `World::from_state`.
//! 2. For a candidate, `World::attach_neural_policy(id, policy)` — the single explicit door
//!    into the recurrent extension, which gives the animal fresh zero `hidden`, `held` and
//!    `feedback` at the current tick.
//! 3. `World::set_scripted_intents` with `bud: Some(false)` — the diagnostic seam's only use
//!    in a candidate rollout, applied identically to every arm, so births are disabled without
//!    touching any reproduction rule.
//! 4. `World::step()` until the horizon or until the organism is gone.
//!
//! Nothing carries over between episodes: every candidate gets a fresh isolated world and
//! private state, and no experience, hidden state or field is reused.
//!
//! # Limits are checked *inside* the episode
//!
//! [`Limits`] carries the shared cancellation flag **and** the run's deadline, and both are
//! read every [`CANCEL_CHECK_TICKS`] ticks. Checking a deadline only when a job is dequeued
//! would let every worker's last episode run past the cap — which is exactly what the R2a
//! review measured — so the deadline is a property of the running rollout, not of the queue.
//! A cancelled episode reports the ticks it had already simulated, so a discarded generation's
//! work is still counted against the budget even though it never reaches the optimizer.
//!
//! # Invariants are checked in release too
//!
//! Core's own end-of-step audits are `#[cfg(debug_assertions)]`, and the trainer runs in
//! release. So an episode validates the world itself on a bounded cadence
//! ([`VALIDATE_EVERY_TICKS`]) and once more at the end: `World::check_invariants` for the
//! fields, ledgers and organisms, and `WorldState::validate` for the neural extension's own
//! rules. An invalid world fails the **experiment**, naming the job, rather than producing a
//! score. Ordinary biological death is not invalid: it is a completed episode with a recorded
//! survival time.
//!
//! # What is measured
//!
//! Survival ticks and usable terminal stores are the *score* (§4 of the brief). Everything
//! else here is a **diagnostic**, reported separately and never summed into the ordering, and
//! it is collected on **every** episode — a campaign rollout records the same numbers a
//! control does, because a field serialized as `0.0` that merely was not measured is worse
//! than no field at all.
//!
//! Read the columns for what they are:
//!
//! - `intake_*` is **material** (m), taken from the world's own [`IntakeDiagnostics`]: what
//!   actually left a field through this mouth. It is *not* an energy credit — assimilation,
//!   its efficiency and the reserve's energy density all sit between the two, and none of them
//!   is read here.
//! - `upkeep_billed` and `motion_billed` are **prices**, reconstructed from the body's own
//!   `MotorBill` and from the motion the world actually resolved. They are *not* a reading of
//!   the world's payment ledger, and on a tick where a body dies of starvation the bill is
//!   precisely what it could not pay.
//! - A tick whose **physical turn cannot be measured** contributes its travel but no rotation,
//!   so `turn_sweep_rad` and `motion_billed` are then **lower bounds, not exact**.
//!   `motion_billed_partial` says when that has happened and `turn_unmeasured_ticks` says how
//!   often. Two cases produce it: the tick a body dies on, which has no post-step heading, and
//!   a tick that crosses a seam, where the stored heading changes by a chart transport *and*
//!   possibly by a real turn and this module cannot separate the two. Excluding the transport
//!   is right; excluding the real turn with it is a measurement gap, and it is labelled as one
//!   rather than hidden inside a number called exact.
//! - `store_start` and `terminal_stores` are usable stores at the first and last tick. These
//!   columns do **not** balance into an energy identity, and no claim here says they do: the
//!   oxidation, assimilation and handling terms that would close such a box are not exposed by
//!   the core and are not measured.

use cubarium_surface::{Scale, Topology};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use cubarium_core::diagnostic::ScriptedIntent;
use cubarium_core::motor::MotorBill;
use cubarium_core::neural::Policy;
use cubarium_core::{DT, World};
use cubarium_surface::{CellId, Vec2, cell_of};
use serde::{Deserialize, Serialize};

use super::fixture::Layout;

/// How often the shared cancellation flag and the deadline are read inside an episode. Every
/// 128 ticks is 6.4 s of simulated time and, at the measured 17,400 ticks/s, about 7 ms of
/// wall time — fine enough that a 60 s budget cannot be overrun by a meaningful margin, coarse
/// enough that neither the atomic nor the clock is on the hot path.
pub const CANCEL_CHECK_TICKS: u64 = 128;

/// How often a running episode validates the world. 2,048 ticks is 18 checks over the
/// 36,000-tick horizon.
pub const VALIDATE_EVERY_TICKS: u64 = 2_048;

/// The limits a rollout runs under. Both are checked inside the episode.
#[derive(Clone, Copy)]
pub struct Limits<'a> {
    pub cancel: &'a AtomicBool,
    /// When the whole run must stop. `None` means the caller has imposed no clock; the
    /// cancellation flag still applies.
    pub deadline: Option<Instant>,
}

impl<'a> Limits<'a> {
    /// No clock, only the flag.
    pub fn new(cancel: &'a AtomicBool) -> Limits<'a> {
        Limits {
            cancel,
            deadline: None,
        }
    }

    pub fn until(cancel: &'a AtomicBool, deadline: Instant) -> Limits<'a> {
        Limits {
            cancel,
            deadline: Some(deadline),
        }
    }

    /// True when the run must stop now. A passed deadline *sets* the shared flag, so one
    /// worker noticing the clock stops every other worker's episode at its next check rather
    /// than each discovering it separately.
    pub fn expired(&self) -> bool {
        if self.cancel.load(Ordering::Relaxed) {
            return true;
        }
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            self.cancel.store(true, Ordering::Relaxed);
            return true;
        }
        false
    }
}

/// What drives the body for the whole episode.
#[derive(Clone, Debug)]
pub enum Driver {
    /// A candidate policy, through the world's own recurrent dispatch.
    Policy(Box<Policy>),
    /// A disclosed diagnostic script. **Never** used for a candidate rollout.
    Control(Control),
}

/// The three controls the fixture protocol requires (brief §3), plus the dwell ladder's
/// parametrised fourth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Control {
    /// Stand still, take nothing: what the starting stores alone buy.
    NoIntake,
    /// Stand still on the opening patch and graze continuously: the legacy `feed_min` gate
    /// bypassed, so this is the most favourable stationary strategy that exists.
    StationaryGrazing,
    /// The disclosed mobile script: tour the layout's food cells in the declared route order,
    /// dwelling on a cell until its `P` falls below the world's own `feed_min`, then walking to
    /// the next one on ordinary paid motion through `motor::resolve`.
    MobileScript,
    /// [`Control::MobileScript`] with the **departure rule replaced by a counter**: leave a
    /// route cell after exactly `d` ticks standing on it, whatever is left in it. Everything
    /// else — the route order, the heading request, the full grazing effort, the paid motion
    /// through `motor::resolve` — is the mobile script's, so a ladder of `d` values differs in
    /// residence and in nothing else.
    ///
    /// `d` counts ticks on which the body **finished** the tick on the goal cell, which is
    /// also every tick on which it fed from that cell: the feeding settlement runs after the
    /// move (`ecology-v1-contract.md` §6.2), and the arrival tick already grazes there.
    /// `Dwell(0)` is not a control: it would never leave a cell it never counts, so the ladder
    /// starts at 1.
    Dwell(u32),
}

impl Control {
    pub fn name(self) -> String {
        match self {
            Control::NoIntake => "no-intake".into(),
            Control::StationaryGrazing => "stationary-grazing".into(),
            Control::MobileScript => "mobile-script".into(),
            Control::Dwell(d) => format!("dwell-{d}"),
        }
    }
}

/// The outcome of one episode. Every field is measured on every episode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub layout: String,
    /// Ticks actually simulated. Equal to the horizon when the body survived it; the tick the
    /// body was found gone otherwise. Ordinary death is a completed episode, not an error.
    pub ticks: u64,
    pub alive: bool,
    /// `E + e_r·R` at the end, or 0 for a dead animal.
    pub terminal_stores: f64,
    /// `E_max + e_r·R_max`: the fixed body capacity the normalisation divides by.
    pub store_capacity: f64,
    // --- diagnostics, never part of the ordering; see the module docs for what they are not ---
    /// Material (m) that actually left `P` through this mouth. Not an energy credit.
    pub intake_producer: f64,
    pub intake_fruit: f64,
    pub intake_detritus: f64,
    /// The upkeep price of every simulated tick, from the body's own `MotorBill`. A price, not
    /// a ledger reading: on a starvation tick this is exactly what could not be paid.
    pub upkeep_billed: f64,
    /// The motor price of the motion the world actually resolved.
    pub motion_billed: f64,
    /// `E + e_r·R` at the first tick.
    pub store_start: f64,
    pub travelled_px: f64,
    pub body_lengths: f64,
    pub distinct_cells: usize,
    pub ticks_in_opening: u64,
    pub turn_sweep_rad: f64,
    /// Ticks that crossed a seam. The stored heading changed by a chart transport, and may
    /// *also* have changed by a real turn; this module reconstructs the turn from the two
    /// stored headings and so cannot separate them. Every such tick is therefore counted in
    /// `turn_unmeasured_ticks` as well.
    pub seam_crossing_ticks: u64,
    /// Ticks whose physical turn could not be measured: every seam crossing, plus the tick a
    /// body dies on (no post-step heading). Their rotation is excluded from `turn_sweep_rad`
    /// and from `motion_billed`, which are lower bounds whenever this is nonzero.
    pub turn_unmeasured_ticks: u64,
    /// `turn_unmeasured_ticks > 0`: the sweep and the motor price are **partial**.
    pub motion_billed_partial: bool,
    /// Whether the last simulated tick is the one on which the body died. Its upkeep is
    /// *billed* in `upkeep_billed` and was, by the starvation predicate, not payable.
    pub died_on_last_tick: bool,
    pub route_p_start: f64,
    pub route_p_end: f64,
    pub route_p_grown: f64,
    /// How many times the world was validated during this episode, terminal check included.
    pub validations: u64,
}

impl Episode {
    /// `clip(terminal usable stores / body capacity, 0, 1)`; zero for a dead animal.
    pub fn normalized_stores(&self) -> f64 {
        if !self.alive || self.store_capacity <= 0.0 {
            return 0.0;
        }
        (self.terminal_stores / self.store_capacity).clamp(0.0, 1.0)
    }

    pub fn seconds(&self) -> f64 {
        self.ticks as f64 * DT
    }
}

/// Why an episode produced no score.
///
/// Neither of these is a low fitness. A cancelled episode was stopped by the run's limits; an
/// invalid one is an **experiment error** and names the job it happened in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EpisodeError {
    /// The run's cancellation flag or deadline stopped this rollout. Carries the ticks already
    /// simulated, so discarded work is still counted.
    Cancelled { ticks: u64 },
    /// The world failed its own invariants. The experiment fails here.
    Invalid { ticks: u64, detail: String },
}

impl EpisodeError {
    pub fn ticks(&self) -> u64 {
        match self {
            EpisodeError::Cancelled { ticks } | EpisodeError::Invalid { ticks, .. } => *ticks,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, EpisodeError::Cancelled { .. })
    }
}

impl std::fmt::Display for EpisodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EpisodeError::Cancelled { ticks } => write!(f, "cancelled after {ticks} ticks"),
            EpisodeError::Invalid { ticks, detail } => {
                write!(f, "invalid world after {ticks} ticks: {detail}")
            }
        }
    }
}

impl std::error::Error for EpisodeError {}

/// Validate a running world: the core's own field/ledger/organism audits and the neural
/// extension's own rules. Public so the fault-injection regression exercises the same call the
/// rollout makes, rather than a copy of it.
pub fn validate_runtime(world: &World, job: &str, tick: u64) -> Result<(), String> {
    world
        .check_invariants()
        .map_err(|e| format!("job {job}, tick {tick}: world invariants: {e}"))?;
    world
        .state
        .validate()
        .map_err(|e| format!("job {job}, tick {tick}: world state: {e}"))
}

/// Run one episode. `job` names the rollout for an experiment error.
pub fn run(
    layout: &Layout,
    driver: &Driver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
) -> Result<Episode, EpisodeError> {
    run_with_fault(layout, driver, horizon, limits, job, None)
}

/// A hook that may corrupt a running world, for the fault-injection regression only.
pub type Fault<'a> = &'a (dyn Fn(&mut World, u64) + Sync);

/// A hook run **once**, on the freshly built world, before the driver is attached and before
/// the first tick.
///
/// It exists for measurements that have to be switched on before anything happens — the
/// per-body budget recorder is the one this milestone needed — and deliberately cannot see a
/// tick number, so it can neither script behaviour nor react to the run. Nothing that changes
/// what the world *does* belongs in here; a driver does that, in the open.
pub type Prepare<'a> = &'a (dyn Fn(&mut World) + Sync);

/// [`run`], with a hook that may corrupt the world at a chosen tick.
///
/// This exists for one reason: the invariant checks above must be exercised in **release**,
/// where core's own debug audits are compiled out, and a regression that only calls
/// [`validate_runtime`] directly would not prove that the rollout consults it. Nothing outside
/// the fault-injection test passes a fault.
#[doc(hidden)]
pub fn run_with_fault(
    layout: &Layout,
    driver: &Driver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
    fault: Option<Fault<'_>>,
) -> Result<Episode, EpisodeError> {
    run_prepared(layout, driver, horizon, limits, job, None, fault)
}

/// One trajectory's per-tick measurement, collected **only** when a caller asks for it.
///
/// Every field is a count or a sum over the ticks of one episode. Nothing here is part of the
/// score, of the ordering, or of what the body does: switching it on turns the world's own
/// per-tick intake trace on for the one traced body and reads it, and
/// `a_traced_episode_is_the_same_episode` is the check on that claim.
///
/// The turn columns are measured exactly as [`Episode::turn_sweep_rad`] is, from the two stored
/// headings, and they exclude exactly the same ticks: a seam crossing, where a chart transport
/// and a real turn cannot be separated, and the tick a body dies on, which has no post-step
/// heading. `turn_measured_ticks` is the denominator, so a fraction computed from these columns
/// is a fraction of the ticks the turn could actually be read on.
///
/// `on_food` is **workstream H's** definition, taken from the world's own settlement rather
/// than re-derived: a tick the body finished on a cell holding at least one of the four stocks
/// (`P`, `F`, `D_eff`, `C_eff`) at or above `drives.feed_min`
/// (`cubarium_core::IntakeTick::above_threshold`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Trajectory {
    /// Ticks whose physical turn could be measured.
    pub turn_measured_ticks: u64,
    /// Of those, ticks whose resolved turn is not exactly zero.
    pub turn_active_ticks: u64,
    /// `Σ |turn|` over the measured ticks, rad. Equal to [`Episode::turn_sweep_rad`].
    pub turn_abs_rad: f64,
    /// Ticks the world's intake trace covered: one per tick the body was there to feed.
    pub traced_ticks: u64,
    /// Of those, ticks on food.
    pub on_food_ticks: u64,
    /// Maximal runs of consecutive on-food ticks. A run that is still open at the end of the
    /// episode is counted, and its length is in `dwell_bout_ticks`.
    pub dwell_bouts: u64,
    /// `Σ` of those runs' lengths, which is `on_food_ticks` — kept so the mean bout reads
    /// without a second source.
    pub dwell_bout_ticks: u64,
    /// The longest run.
    pub dwell_bout_max: u64,
    /// Rows the world's recorder dropped. Must be 0; a non-zero value means the drain cadence
    /// below is too slow for `MAX_TRACE_ROWS` and the on-food columns are incomplete.
    pub trace_rows_dropped: u64,
}

impl Trajectory {
    /// Ticks with a non-zero resolved turn, as a fraction of the ticks the turn was measurable
    /// on. `None` when nothing was measurable.
    pub fn turn_active_fraction(&self) -> Option<f64> {
        (self.turn_measured_ticks > 0)
            .then(|| self.turn_active_ticks as f64 / self.turn_measured_ticks as f64)
    }

    /// Mean `|omega|` over the measured ticks, rad/s.
    pub fn mean_abs_omega(&self) -> Option<f64> {
        (self.turn_measured_ticks > 0)
            .then(|| self.turn_abs_rad / (self.turn_measured_ticks as f64 * DT))
    }

    /// On-food ticks as a fraction of the traced ticks.
    pub fn on_food_fraction(&self) -> Option<f64> {
        (self.traced_ticks > 0).then(|| self.on_food_ticks as f64 / self.traced_ticks as f64)
    }

    /// Mean length of a dwell bout, ticks. `None` when the body never stood on food.
    pub fn mean_dwell_bout(&self) -> Option<f64> {
        (self.dwell_bouts > 0).then(|| self.dwell_bout_ticks as f64 / self.dwell_bouts as f64)
    }
}

/// How often a traced episode drains the world's intake recorder. Half
/// [`cubarium_core::MAX_TRACE_ROWS`], so the recorder can never fill between drains and
/// `Trajectory::trace_rows_dropped` stays 0.
pub const TRACE_DRAIN_TICKS: u64 = 2_048;

/// [`run`], with the per-tick trajectory measurement switched on.
pub fn run_traced(
    layout: &Layout,
    driver: &Driver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
) -> Result<(Episode, Trajectory), EpisodeError> {
    let mut trace = Trajectory::default();
    let episode = run_prepared_traced(
        layout,
        driver,
        horizon,
        limits,
        job,
        None,
        None,
        Some(&mut trace),
    )?;
    Ok((episode, trace))
}

/// [`run_with_fault`], with a hook that runs on the built world before the first tick.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn run_prepared(
    layout: &Layout,
    driver: &Driver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
    prepare: Option<Prepare<'_>>,
    fault: Option<Fault<'_>>,
) -> Result<Episode, EpisodeError> {
    run_prepared_traced(layout, driver, horizon, limits, job, prepare, fault, None)
}

/// [`run_prepared`], with the optional per-tick [`Trajectory`].
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn run_prepared_traced(
    layout: &Layout,
    driver: &Driver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
    prepare: Option<Prepare<'_>>,
    fault: Option<Fault<'_>>,
    mut trace: Option<&mut Trajectory>,
) -> Result<Episode, EpisodeError> {
    let (mut world, id) = layout.build().expect("a frozen layout builds");
    if let Some(prepare) = prepare {
        prepare(&mut world);
    }
    let cfg = world.config().clone();
    let e_r = cfg.organism.reserve_energy_density;
    let leave_below = cfg.drives.feed_min;
    let route = layout.route();
    let opening: BTreeSet<u16> = layout.patches[0]
        .cells(layout.face())
        .iter()
        .map(|c| c.0)
        .collect();

    let (extent, capacity) = {
        let o = world.state.organisms.get(id).expect("the grazer");
        (
            o.phenotype.extent,
            o.phenotype.energy_max + e_r * o.phenotype.reserve_max,
        )
    };

    let start_heading = layout.heading_vec();
    match driver {
        Driver::Policy(policy) => {
            world
                .attach_neural_policy(id, (**policy).clone())
                .expect("a validated policy attaches to an ordinary body");
            // The only scripted field a candidate rollout uses: births off, equally, in every
            // arm. `apply` is `d.bud = d.bud && false`, so it can suppress a request and can
            // never create one.
            world.set_scripted_intents(vec![(
                id,
                ScriptedIntent {
                    bud: Some(false),
                    ..ScriptedIntent::default()
                },
            )]);
        }
        Driver::Control(Control::NoIntake) => {
            world.set_scripted_intents(vec![(
                id,
                ScriptedIntent {
                    heading: Some(start_heading),
                    effort: Some(0.0),
                    graze_effort: Some(0.0),
                    fruit_effort: Some(0.0),
                    scavenge_effort: Some(0.0),
                    mode: None,
                    bud: Some(false),
                },
            )]);
        }
        Driver::Control(Control::StationaryGrazing) => {
            world.set_scripted_intents(vec![(
                id,
                ScriptedIntent {
                    heading: Some(start_heading),
                    effort: Some(0.0),
                    graze_effort: Some(1.0),
                    fruit_effort: Some(0.0),
                    scavenge_effort: Some(0.0),
                    mode: None,
                    bud: Some(false),
                },
            )]);
        }
        Driver::Control(Control::MobileScript | Control::Dwell(_)) => {}
    }

    // The world's own per-tick intake trace, for the traced body only. Transient, never read
    // back by the tick, and the episode below is byte-identical with it on or off.
    if trace.is_some() {
        world.trace_intake(Some(id));
    }
    // The open dwell bout, carried across drains.
    let mut bout: u64 = 0;

    let route_p_start: f64 = route.iter().map(|c| world.state.fields.p[c.index()]).sum();
    let mut episode = Episode {
        layout: layout.name.clone(),
        ticks: 0,
        alive: true,
        terminal_stores: 0.0,
        store_capacity: capacity,
        intake_producer: 0.0,
        intake_fruit: 0.0,
        intake_detritus: 0.0,
        upkeep_billed: 0.0,
        motion_billed: 0.0,
        store_start: 0.0,
        travelled_px: 0.0,
        body_lengths: 0.0,
        distinct_cells: 0,
        ticks_in_opening: 0,
        turn_sweep_rad: 0.0,
        seam_crossing_ticks: 0,
        turn_unmeasured_ticks: 0,
        motion_billed_partial: false,
        died_on_last_tick: false,
        route_p_start,
        route_p_end: 0.0,
        route_p_grown: 0.0,
        validations: 0,
    };

    episode.store_start = {
        let o = world.state.organisms.get(id).expect("the grazer");
        o.energy + e_r * o.reserve
    };
    let mut visited: BTreeSet<u16> = BTreeSet::new();
    let mut target = 0usize;
    // Ticks finished on the current goal cell, for `Control::Dwell` only.
    let mut dwelled = 0u64;

    for tick in 0..horizon {
        if tick.is_multiple_of(CANCEL_CHECK_TICKS) && limits.expired() {
            return Err(EpisodeError::Cancelled { ticks: tick });
        }
        if tick > 0 && tick.is_multiple_of(VALIDATE_EVERY_TICKS) {
            episode.validations += 1;
            if let Err(detail) = validate_runtime(&world, job, tick) {
                return Err(EpisodeError::Invalid {
                    ticks: tick,
                    detail,
                });
            }
        }
        let Some(o) = world.state.organisms.get(id) else {
            episode.alive = false;
            episode.ticks = tick;
            break;
        };
        let here = cell_of(Topology::Cube, Scale::ONE, &o.pos);
        let heading_before = o.heading;
        let chart_before = o.pos.chart();
        let face_before = o.pos.face;
        let bill = MotorBill::of(o, &cfg);

        if let Driver::Control(Control::MobileScript | Control::Dwell(_)) = driver {
            // The disclosed rule, in full: dwell and crop until the cell falls below the
            // world's own `feed_min`, then face the next route cell's centre and walk. The
            // heading is a *request*; `motor::resolve` decides how much of the turn and how
            // much of the walk the body can pay for this tick. `Dwell(d)` runs exactly this
            // approach and this intent; only the departure test below differs.
            let goal = route[target % route.len()];
            let intent = if here == goal {
                ScriptedIntent {
                    heading: Some(heading_before),
                    effort: Some(0.0),
                    graze_effort: Some(1.0),
                    fruit_effort: Some(0.0),
                    scavenge_effort: Some(0.0),
                    mode: None,
                    bud: Some(false),
                }
            } else {
                let toward = toward(goal, chart_before, face_before).unwrap_or(heading_before);
                ScriptedIntent {
                    heading: Some(toward),
                    effort: Some(1.0),
                    graze_effort: Some(1.0),
                    fruit_effort: Some(0.0),
                    scavenge_effort: Some(0.0),
                    mode: None,
                    bud: Some(false),
                }
            };
            world.set_scripted_intents(vec![(id, intent)]);
        }

        visited.insert(here.0);
        if opening.contains(&here.0) {
            episode.ticks_in_opening += 1;
        }

        world.step();
        world.drain_events();
        if let Some(inject) = fault {
            inject(&mut world, tick);
        }
        episode.ticks = tick + 1;

        // Every tick is accounted for, including the one a body dies on. `moved_segments` is
        // indexed by slot and still holds the motion this tick resolved, so a death tick's
        // travel is measured; only its *turn* cannot be, because there is no post-step heading.
        let travelled: f64 = world
            .moved_segments(id)
            .iter()
            .map(cubarium_surface::PathSegment::length)
            .sum();
        let after = world.state.organisms.get(id);
        let turn = match after {
            Some(o) if o.pos.face == face_before => signed_turn(heading_before, o.heading).abs(),
            Some(_) => {
                // A seam crossing changes the chart, and may have turned the body as well.
                // `signed_turn` on two headings in different charts would read the transport
                // as rotation, so the tick's real turn is unknown, not zero: count it as
                // unmeasured rather than pretending the exclusion was free. Measuring it would
                // mean transporting one heading into the other's chart, which needs the
                // resolved physical turn the core keeps for neural feedback and does not
                // publish; that is a core accessor, out of this milestone's scope.
                episode.seam_crossing_ticks += 1;
                episode.turn_unmeasured_ticks += 1;
                0.0
            }
            None => {
                episode.turn_unmeasured_ticks += 1;
                0.0
            }
        };
        if let Some(t) = trace.as_deref_mut()
            && after.is_some_and(|o| o.pos.face == face_before)
        {
            t.turn_measured_ticks += 1;
            t.turn_abs_rad += turn;
            if turn != 0.0 {
                t.turn_active_ticks += 1;
            }
        }
        episode.travelled_px += travelled;
        episode.turn_sweep_rad += turn;
        episode.upkeep_billed += bill.upkeep(DT);
        episode.motion_billed += bill.motor_cost(travelled / DT, extent * turn / DT, DT);

        // The organism's position, copied out, so the trace drain below can take the world
        // mutably without holding a borrow into its organism table.
        let Some(after_pos) = after.map(|o| o.pos) else {
            episode.alive = false;
            episode.died_on_last_tick = true;
            break;
        };

        match driver {
            Driver::Control(Control::MobileScript) => {
                let goal = route[target % route.len()];
                if cell_of(Topology::Cube, Scale::ONE, &after_pos) == goal
                    && world.state.fields.p[goal.index()] < leave_below
                {
                    target += 1;
                }
            }
            Driver::Control(Control::Dwell(d)) => {
                // The counter is the whole difference. A tick finished on the goal cell is a
                // tick fed from it, so `dwelled` and "ticks on this food cell" are the same
                // number, and the body leaves on the tick that makes it `d`.
                let goal = route[target % route.len()];
                if cell_of(Topology::Cube, Scale::ONE, &after_pos) == goal {
                    dwelled += 1;
                    if dwelled >= u64::from(*d) {
                        target += 1;
                        dwelled = 0;
                    }
                }
            }
            _ => {}
        }

        if let Some(t) = trace.as_deref_mut()
            && (tick + 1).is_multiple_of(TRACE_DRAIN_TICKS)
        {
            drain_trace_into(&mut world, t, &mut bout);
        }
    }

    if let Some(t) = trace.as_deref_mut() {
        drain_trace_into(&mut world, t, &mut bout);
        if bout > 0 {
            t.dwell_bouts += 1;
            t.dwell_bout_max = t.dwell_bout_max.max(bout);
        }
    }

    episode.validations += 1;
    if let Err(detail) = validate_runtime(&world, job, episode.ticks) {
        return Err(EpisodeError::Invalid {
            ticks: episode.ticks,
            detail,
        });
    }

    if let Some(o) = world.state.organisms.get(id) {
        episode.alive = true;
        episode.terminal_stores = o.energy + e_r * o.reserve;
    } else {
        episode.alive = false;
        episode.terminal_stores = 0.0;
    }

    let diag = world.intake_diagnostics();
    episode.intake_producer = diag.producer_eaten;
    episode.intake_fruit = diag.fruit_eaten;
    // Ecology v1 splits the detrital stock in two; the episode's one scalar is their sum.
    episode.intake_detritus = diag.litter_eaten + diag.carrion_eaten;
    episode.route_p_grown = diag.producer_growth;
    episode.route_p_end = route.iter().map(|c| world.state.fields.p[c.index()]).sum();
    episode.motion_billed_partial = episode.turn_unmeasured_ticks > 0;
    episode.distinct_cells = visited.len();
    episode.body_lengths = if extent > 0.0 {
        episode.travelled_px / extent
    } else {
        0.0
    };
    Ok(episode)
}

/// Fold the world's recorded intake rows into a [`Trajectory`], carrying the open dwell bout.
///
/// A bout is a maximal run of consecutive on-food ticks. It is closed — counted, and its length
/// folded in — on the first row that is not on food, and the caller closes whatever is still
/// open when the episode ends.
fn drain_trace_into(world: &mut cubarium_core::World, t: &mut Trajectory, bout: &mut u64) {
    let (rows, dropped) = world.drain_intake_trace();
    t.trace_rows_dropped += dropped;
    for row in &rows {
        t.traced_ticks += 1;
        if row.above_threshold.iter().any(|b| *b) {
            t.on_food_ticks += 1;
            t.dwell_bout_ticks += 1;
            *bout += 1;
        } else if *bout > 0 {
            t.dwell_bouts += 1;
            t.dwell_bout_max = t.dwell_bout_max.max(*bout);
            *bout = 0;
        }
    }
}

/// A unit heading from the body's chart position toward a goal cell's centre, when both are on
/// the same face. Returns `None` across a seam, where the caller keeps its current heading.
fn toward(goal: CellId, from: Vec2, face: cubarium_surface::Face) -> Option<Vec2> {
    if goal.face(Topology::Cube, Scale::ONE) != face {
        return None;
    }
    (goal.center(Topology::Cube, Scale::ONE).chart() - from).normalized()
}

fn signed_turn(a: Vec2, b: Vec2) -> f64 {
    use std::f64::consts::{PI, TAU};
    let d = b.screen_angle() - a.screen_angle();
    let d = (d + PI).rem_euclid(TAU) - PI;
    if d.is_finite() { d } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::fixture::training_layouts;
    use std::time::Duration;

    fn free(cancel: &AtomicBool) -> Limits<'_> {
        Limits::new(cancel)
    }

    #[test]
    fn a_still_body_with_no_intake_eats_nothing_and_keeps_its_stores_falling() {
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let e = run(
            l,
            &Driver::Control(Control::NoIntake),
            400,
            free(&cancel),
            "t",
        )
        .expect("ok");
        assert_eq!(
            e.intake_producer, 0.0,
            "a closed mouth records exactly zero intake"
        );
        assert_eq!(
            e.travelled_px, 0.0,
            "effort 0 is a genuine request for stillness"
        );
        assert!(e.upkeep_billed > 0.0, "living is not free");
        assert!(e.alive, "400 ticks is well inside the starting stores");
        assert!(e.terminal_stores < e.store_capacity);
    }

    #[test]
    fn a_stationary_grazer_eats_from_its_own_cell_only() {
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let e = run(
            l,
            &Driver::Control(Control::StationaryGrazing),
            400,
            free(&cancel),
            "t",
        )
        .expect("ok");
        assert!(e.intake_producer > 0.0, "an open mouth on a fed cell eats");
        assert_eq!(e.travelled_px, 0.0);
        assert_eq!(e.distinct_cells, 1, "a stationary body visits one cell");
    }

    #[test]
    fn the_mobile_script_travels_and_visits_more_than_one_cell() {
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let e = run(
            l,
            &Driver::Control(Control::MobileScript),
            600,
            free(&cancel),
            "t",
        )
        .expect("ok");
        assert!(e.travelled_px > 0.0, "the script pays for real motion");
        assert!(e.distinct_cells > 1, "the script relocates");
        assert!(e.motion_billed > 0.0, "and pays for it");
        assert!(e.intake_producer > 0.0);
    }

    #[test]
    fn cancellation_stops_an_episode_and_reports_the_ticks_it_had_run() {
        let cancel = AtomicBool::new(true);
        let l = &training_layouts()[0];
        let out = run(
            l,
            &Driver::Control(Control::NoIntake),
            36_000,
            free(&cancel),
            "t",
        );
        assert_eq!(out.unwrap_err(), EpisodeError::Cancelled { ticks: 0 });
    }

    /// Review finding 1, at the episode level: a deadline that passes **while the rollout is
    /// running** must stop it. Before the repair the deadline was only read when a job was
    /// dequeued, so a long episode ran to completion however late it was.
    #[test]
    fn a_deadline_that_passes_mid_episode_stops_the_rollout() {
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let started = Instant::now();
        let limits = Limits::until(&cancel, started + Duration::from_millis(20));
        let out = run(l, &Driver::Control(Control::NoIntake), 36_000, limits, "t");
        let err = out.expect_err("the deadline must stop it");
        assert!(err.is_cancelled());
        assert!(
            err.ticks() > 0,
            "it had already simulated work, and that work is counted"
        );
        assert!(err.ticks() < 36_000, "it did not run to the horizon");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "it stopped promptly"
        );
        assert!(
            cancel.load(Ordering::Relaxed),
            "and it told the other workers"
        );
    }

    /// Review finding 4: an identical episode must report identical diagnostics, whatever the
    /// caller. There is no reduced mode that serializes zeros for what it did not measure.
    #[test]
    fn every_episode_records_the_same_diagnostics_whoever_asked_for_it() {
        use crate::es::tensor;
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let policy = tensor::policy(&tensor::initial_center(20_260_915)).expect("valid");
        let a = run(
            l,
            &Driver::Policy(Box::new(policy.clone())),
            40,
            free(&cancel),
            "a",
        )
        .expect("ok");
        let b = run(l, &Driver::Policy(Box::new(policy)), 40, free(&cancel), "b").expect("ok");
        assert!(a.upkeep_billed > 0.0, "a living body is billed for living");
        assert!(a.motion_billed > 0.0, "and this policy moves");
        assert!(a.distinct_cells > 0);
        assert_eq!(a.upkeep_billed, b.upkeep_billed);
        assert_eq!(a.motion_billed, b.motion_billed);
        assert_eq!(a.terminal_stores, b.terminal_stores);
        assert_eq!(a.distinct_cells, b.distinct_cells);
    }

    #[test]
    fn a_zero_policy_runs_through_the_real_dispatch() {
        use crate::es::tensor;
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let policy = tensor::policy(&vec![0.0; tensor::PARAMS]).expect("valid");
        let e = run(
            l,
            &Driver::Policy(Box::new(policy)),
            200,
            free(&cancel),
            "t",
        )
        .expect("ok");
        assert!(e.alive);
        assert_eq!(e.ticks, 200);
        assert!(e.terminal_stores > 0.0);
        assert!(e.validations >= 1, "the terminal validation always runs");
    }

    #[test]
    fn two_runs_of_the_same_episode_agree_exactly() {
        use crate::es::tensor;
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[1];
        let policy = tensor::policy(&tensor::initial_center(5)).expect("valid");
        let a = run(
            l,
            &Driver::Policy(Box::new(policy.clone())),
            600,
            free(&cancel),
            "t",
        )
        .expect("ok");
        let b = run(
            l,
            &Driver::Policy(Box::new(policy)),
            600,
            free(&cancel),
            "t",
        )
        .expect("ok");
        assert_eq!(a, b);
    }

    /// Review finding 5, exercised in **release**: a world corrupted mid-rollout must fail the
    /// experiment by name, not produce a score. Core's own end-of-step audits are compiled out
    /// of this profile, which is exactly why the episode runs its own.
    #[test]
    fn a_corrupted_world_fails_the_experiment_and_names_the_job() {
        use crate::es::tensor;
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let policy = tensor::policy(&tensor::initial_center(20_260_915)).expect("valid");
        // Corrupt the animal's private hidden state one tick before a validation point, so the
        // rollout's own check is what finds it. `WorldState::validate` requires every neural
        // value to be finite — the R1a extension's own rule, which core's release build never
        // evaluates on its own.
        let corrupt = |w: &mut World, tick: u64| {
            if tick == VALIDATE_EVERY_TICKS - 1 {
                w.state.neural.animals[0].1.hidden[0] = f64::NAN;
            }
        };
        let out = run_with_fault(
            l,
            &Driver::Policy(Box::new(policy.clone())),
            VALIDATE_EVERY_TICKS + 1,
            free(&cancel),
            "gen7/pair3-/t1-corridor",
            Some(&corrupt),
        );
        let err = out.expect_err("an invalid world is an experiment error");
        match err {
            EpisodeError::Invalid { ticks, detail } => {
                assert_eq!(
                    ticks, VALIDATE_EVERY_TICKS,
                    "caught at the next validation point"
                );
                assert!(
                    detail.contains("gen7/pair3-/t1-corridor"),
                    "detail was {detail}"
                );
                assert!(detail.contains("world state"), "detail was {detail}");
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        // The same fault after the last cadence point is still caught by the terminal check.
        let late = |w: &mut World, tick: u64| {
            if tick == 600 {
                w.state.neural.animals[0].1.hidden[1] = f64::INFINITY;
            }
        };
        let out = run_with_fault(
            l,
            &Driver::Policy(Box::new(policy.clone())),
            601,
            free(&cancel),
            "gen0/center/t1-corridor",
            Some(&late),
        );
        assert!(
            matches!(out, Err(EpisodeError::Invalid { .. })),
            "terminal check must catch it"
        );

        // And the same rollout without a fault is a perfectly ordinary episode.
        assert!(
            run(
                l,
                &Driver::Policy(Box::new(policy)),
                1_000,
                free(&cancel),
                "t"
            )
            .is_ok(),
            "the fault, not the fixture, is what fails"
        );
    }

    /// Review repair-1 finding B: a tick that crosses a seam changes the stored heading by a
    /// chart transport *and* possibly by a real body turn, and this module cannot separate the
    /// two. Zeroing the turn silently dropped the real one — the interior arm swept
    /// 0.3431227672041359 rad and the seam arm 0.3345446980240325, a deficit of exactly one
    /// tick's turn — while `turn_unmeasured_ticks` stayed 0 and the price still called itself
    /// exact. The crossing is now counted as unmeasured and the price is labelled partial.
    #[test]
    fn a_seam_crossing_tick_is_counted_as_an_unmeasured_turn_not_as_no_turn() {
        use cubarium_core::neural::{Policy, gru::Gru32};
        let cancel = AtomicBool::new(false);
        let mut layout = training_layouts()[0].clone();
        let mut weights = Gru32::zeros();
        // A constant action: full thrust and a steady turn, so both arms ask for the same
        // thing tick for tick and only the geometry differs.
        weights.b_o[0] = 8.0;
        weights.b_o[1] = 0.12;
        weights.b_o[2] = -8.0;
        weights.b_o[3] = -8.0;
        weights.b_o[4] = -8.0;
        let driver = Driver::Policy(Box::new(Policy::new(weights)));

        layout.start = (8, 15);
        layout.heading = (0.0, 1.0);
        let seam = run(&layout, &driver, 40, free(&cancel), "seam").expect("ok");
        layout.start = (8, 8);
        let flat = run(&layout, &driver, 40, free(&cancel), "flat").expect("ok");

        assert!(seam.alive && flat.alive);
        assert!(
            seam.seam_crossing_ticks > 0,
            "the fixture must actually cross a seam"
        );
        assert!(
            seam.turn_unmeasured_ticks >= seam.seam_crossing_ticks,
            "every crossing tick's turn is unknown, not zero: {} crossings, {} unmeasured",
            seam.seam_crossing_ticks,
            seam.turn_unmeasured_ticks
        );
        assert!(
            seam.motion_billed_partial,
            "so its sweep and price are lower bounds"
        );

        // The interior arm crosses nothing, so its price is not partial and its sweep is the
        // whole rotation. That is what makes the seam arm's label meaningful rather than
        // always-on.
        assert_eq!(flat.seam_crossing_ticks, 0);
        assert_eq!(flat.turn_unmeasured_ticks, 0);
        assert!(!flat.motion_billed_partial);
        assert!(
            flat.turn_sweep_rad > seam.turn_sweep_rad,
            "the deficit is the dropped turn"
        );
    }
}
