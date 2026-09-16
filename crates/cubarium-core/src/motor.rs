//! The motor boundary: a *request* for physical motion, and the motion the body, its
//! surroundings and its energy actually allow.
//!
//! Milestone R0a (`design/handoffs/r0a-movement-foundation-2026-09-14.md`). Every writer of
//! heading and movement effort — the ordinary controller, apex pursuit, escape and encounter
//! retreat — states an *intent* in [`MotorRequest`], and exactly one call to [`resolve`] at
//! the end of the step turns that intent into the motion the world applies. Nothing else may
//! assign a heading: a direct assignment is a request, never a result.
//!
//! # The envelope
//!
//! `|v| + r · |ω| ≤ u` — the candidate bound of `design/recurrent-organism-plan.md` §2.
//! `v` is the centre speed in px/s, `ω` the turn rate in rad/s and `r` the body's outer
//! radius in px, so `r · |ω|` is the speed of the outermost point of the body as it sweeps.
//! Translation and rotation therefore share one capability, and **pure pivoting is legal**:
//! at `v = 0` the whole budget goes to `ω`. The bound is never a comparison between the
//! angular sweep and the centre speed, which would forbid exactly that.
//!
//! A body twice as wide sweeps twice as fast at the same `ω`, so it turns at half the rate
//! for the same budget. That is the whole point: "larger physical bodies cannot rotate like
//! points".
//!
//! # Where the budget comes from
//!
//! `u = min(capability, affordable)`:
//!
//! - `capability = speed_cap` — the translation ceiling the caller already computed from
//!   effort, morphology, wading and any legitimate burst. **That single number is the whole
//!   budget: going and turning spend the same pixels per second.** `turn_rate_max` (the
//!   genome's `turn_rate_max_deg`, or an override such as a threatened prey's escape rate) is
//!   retained as an *additional* ceiling on `|ω|` and never adds to the budget.
//! - `affordable` is the motor magnitude the creature's remaining energy pays for *after*
//!   unavoidable upkeep is reserved ([`MotorBill`]).
//!
//! **R0b correction.** Until R0b the capability was `speed_cap + `[`REFERENCE_RADIUS_PX`]` ·
//! turn_rate_max`, a *union of two ceilings* rather than a shared budget: a unit adult had
//! 0.3 px/s of travel plus 3.93 px/s of rim sweep it could spend on nothing else, so
//! `|v| + r·|ω| ≤ u` never bound a body narrower than [`REFERENCE_RADIUS_PX`] and a Resting
//! body could still spin at its genome's full rate. Removing the addend is what makes the
//! bound a budget: a unit adult at full effort now pivots at `0.3 / 2.5` = 0.12 rad/s (6.9°/s)
//! and gives up travel one-for-one to do it. See
//! `design/7_Research/r0b-motor-foraging-result-2026-09-14.md`.
//!
//! # Simultaneous requests
//!
//! When `v_req + r · |ω_req| > u`, **both** are multiplied by the single factor
//! `s = u / (v_req + r · |ω_req|)`. One common factor, so the caller's chosen split between
//! going and turning survives the scaling; neither channel is starved to feed the other. A
//! request that already fits is granted untouched, and the resolved heading is then the
//! requested heading exactly — bit for bit, not a round trip through an angle.
//!
//! Zero requests hold the body still: no drift, no residual turn, no charge.
//!
//! # What is *not* paid for here
//!
//! Transporting a heading across a seam or a rim is a change of chart, not a physical turn.
//! [`resolve`] works entirely in the pre-transport chart; `crate::world` applies the
//! [`cubarium_surface::TangentMap`] to the *resolved* heading afterwards, so a chart jump
//! costs nothing and consumes no turn budget.
//!
//! Acceleration state, a contact solver and a neural action ABI are deliberately out of this
//! slice; the resolver is memoryless and a request is satisfied within the tick or scaled
//! down, never queued.

use cubarium_surface::Vec2;

use crate::config::WorldConfig;
use crate::organism::Organism;

/// The decoded outer radius of a unit-size adult founder, in pixels: the body the pace of this
/// world was calibrated against.
///
/// 2.5 px is the [`crate::genome::decode`] extent of a unit-size adult founder (core lobe
/// `(0, 0, 1.4)`, head `(1.6, 0, 0.9)`, tail `(−1.4, 0, 0.7)`; the head lobe's `1.6 + 0.9` is
/// the maximum). `reference_radius_is_the_unit_adult_extent` re-derives it from the decoder
/// and fails if the body plan moves.
///
/// **Since R0b this constant enters no envelope arithmetic.** It was the addend that made
/// [`MotorLimits::capability`] a union of two ceilings — a body at exactly this radius could
/// attain its translation and rotation ceilings *at the same time* — and removing that addend
/// is the R0b correction. What remains is a reference scale: the radius that divides the
/// shared budget for an ordinary adult (`u / 2.5` rad/s of pivot), used by tests and prose to
/// say how hard the shared budget bites at a familiar body size. A body wider than this turns
/// proportionally slower, and a narrower one proportionally faster, with no discontinuity here.
pub const REFERENCE_RADIUS_PX: f64 = 2.5;

/// What one pixel of outer-body sweep costs, relative to one pixel of centre travel.
///
/// The envelope and the bill answer different questions and need not use the same radius.
/// The *constraint* is on the fastest-moving part of the body, so [`turn_radius_px`] is the
/// outermost point. The *cost* is work done by the whole body, and an extended body's mean
/// sweep radius is well under its outer one — half, for a uniform rod about its centre; two
/// thirds, for a uniform disc. This scale carries that difference, at the world's one
/// `move_cost`, so translation and rotation are still billed once each through one term.
///
/// **This is the knob for the price of turning; how fast a body may turn is set by the shared
/// budget in [`MotorLimits::capability`].** Before R0a rotation was free. 0.5 is the rod
/// figure: honest, conservative against the mean, and still a real price.
///
/// **R0b changes what this knob can do.** Under the old union-of-ceilings envelope a unit adult
/// could sweep 3.93 px/s while travelling 0.3 px/s, so `k` priced thirteen times as much sweep
/// as travel and was the dominant ecological lever — that is what
/// `design/7_Research/r0a-motor-cost-ecology-2026-09-14.md` measured (population 93 at `k = 0`
/// down to 39 at `k = 1`). With the shared budget, `|v| + r·|ω| ≤ speed_cap`, so sweep can
/// never exceed `speed_cap` and the most `k` can ever cost is
/// `move_cost · S · k · speed_max · dt` — at most half of what pure travel already costs.
/// `k` is no longer an ecological lever, and R0b keeps it at 0.5 as shipped. Those population
/// figures describe the legacy controller's turning habit under the inflated envelope and do
/// not transfer to the corrected one.
pub const ROTATION_COST_SCALE: f64 = 0.5;

/// Free rotation is the defect R0a fixes, so a zero scale is not a tuning option.
const _: () = assert!(ROTATION_COST_SCALE > 0.0);

/// Which **motor contract** a world is running: the shipped outer-point sweep, or the paired
/// inertial disc (`design/handoffs/ecology-v1-motor-inertial-opus-2026-09-16.md`).
///
/// This is a `World`-level transient — it is never persisted, never in [`WorldConfig`], and
/// never part of a state hash. [`MotorModel::Sweep`] is the default and is byte-identical to
/// the build that never heard of the switch; every entry point below without an `_in` suffix
/// *is* the sweep model, unchanged, so a caller that does not name a model gets today's world.
///
/// # The two models
///
/// Both express rotation as a **speed in px/s** — a radius times `|ω|` — so that translation
/// and turning can share one envelope and one `move_cost`. They differ in which radius, how
/// the two speeds combine in the envelope, and what a px/s of rotation costs.
///
/// | | radius in the term | envelope | bill |
/// |---|---|---|---|
/// | [`Sweep`](MotorModel::Sweep) | `max(lobes, grasp)` — the outermost *contacting* point | `\|v\| + r·\|ω\| ≤ cap` | `move_cost·S·(\|v\| + k·r·\|ω\|)·dt` |
/// | [`Inertial`](MotorModel::Inertial) | `lobes/√2` — a uniform disc's radius of gyration | `√(v² + v_rot²) ≤ cap` | `move_cost·S·(\|v\| + v_rot)·dt` |
///
/// Under `Inertial` every organism is a uniform disc of mass ∝ `structure` and radius
/// `phenotype.extent`, and `v_rot = r·|ω|/√2` is the translation speed that carries the same
/// kinetic energy as spinning that disc at `ω`. Rotation is therefore priced and bounded as an
/// *energy*, which is why it combines with `v` in quadrature rather than by addition, and why
/// the separate [`ROTATION_COST_SCALE`] — a stylized price for the rod figure — disappears
/// into the radius. The apex's grasp (`capture_offset + capture_reach`) is contact geometry and
/// carries no mass worth turning, so it leaves the radius entirely; this is Wrysk's direction
/// of 2026-09-16, "we don't need to model 'are claws outstretched' when turning".
///
/// **A body that only translates is identical under both models** — the same bill, bit for
/// bit, and the same delivered speed at every energy. The whole difference is rotation.
///
/// Not modelled here: a cost of *acceleration*. Both models are memoryless and price the
/// motion held during the tick, not the change in it.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum MotorModel {
    /// The shipped contract: the outermost contacting point's sweep, summed with travel.
    #[default]
    Sweep,
    /// The paired inertial disc: an energy-equivalent rotation speed, combined in quadrature.
    Inertial,
}

impl MotorModel {
    /// The name this model is spelled with on a command line, in a protocol and in a policy
    /// file. Stable: it is provenance, not a label.
    pub const fn name(self) -> &'static str {
        match self {
            MotorModel::Sweep => "sweep",
            MotorModel::Inertial => "inertial",
        }
    }

    /// Read a model by name. An unknown name is an error, never a silent default: a run that
    /// misspells its model must not quietly produce the shipped one.
    pub fn parse(s: &str) -> Result<MotorModel, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sweep" => Ok(MotorModel::Sweep),
            "inertial" => Ok(MotorModel::Inertial),
            other => Err(format!(
                "unknown motor model {other:?}: expected \"sweep\" (the shipped contract) or \
                 \"inertial\""
            )),
        }
    }

    /// Is this the shipped contract? Used to keep the default out of serialized provenance, so
    /// a `Sweep` protocol keeps the hash it has always had.
    pub const fn is_sweep(&self) -> bool {
        matches!(self, MotorModel::Sweep)
    }

    /// The radius that enters the rotation term, given the body's physical radius `r`.
    ///
    /// `r` under `Sweep` — the outermost point sweeps at `r·ω`. `r/√2` under `Inertial` — a
    /// uniform disc's radius of gyration, so `r_g·ω` is the translation speed of equal kinetic
    /// energy.
    pub fn rotation_radius_px(self, radius_px: f64) -> f64 {
        let r = finite_non_negative(radius_px);
        match self {
            MotorModel::Sweep => r,
            MotorModel::Inertial => r / std::f64::consts::SQRT_2,
        }
    }

    /// The rotation speed this model bounds and bills, px/s, from a **physical** radius and an
    /// angular rate: `r·|ω|` under `Sweep`, `r·|ω|/√2` under `Inertial`.
    pub fn rotation_speed(self, radius_px: f64, omega_abs: f64) -> f64 {
        self.rotation_radius_px(radius_px) * finite_non_negative(omega_abs)
    }

    /// How translation and rotation combine in the **envelope**: added under `Sweep` (one
    /// body cannot have two fastest points), in quadrature under `Inertial` (two kinetic
    /// energies drawn from one budget).
    pub fn envelope_magnitude(self, speed: f64, rotation: f64) -> f64 {
        let v = finite_non_negative(speed);
        let w = finite_non_negative(rotation);
        match self {
            MotorModel::Sweep => v + w,
            MotorModel::Inertial => v.hypot(w),
        }
    }

    /// What one px/s of rotation speed costs relative to one px/s of travel:
    /// [`ROTATION_COST_SCALE`] under `Sweep`, and **one** under `Inertial`, where the
    /// mean-radius discount is already inside the radius.
    pub const fn rotation_price(self) -> f64 {
        match self {
            MotorModel::Sweep => ROTATION_COST_SCALE,
            MotorModel::Inertial => 1.0,
        }
    }
}

/// What a body asks the world for this tick, in its own (pre-transport) chart.
///
/// `heading` is a *target orientation*, not a result: the resolver turns toward it by as much
/// as the envelope permits. A caller that wants no turn asks for the body's current heading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotorRequest {
    /// Desired orientation, normally a unit vector. A zero or non-finite vector requests no
    /// turn at all; a non-unit one is normalized before use.
    pub heading: Vec2,
    /// Desired centre speed along the *resolved* heading, px/s. Negative reads as zero.
    pub speed: f64,
}

impl MotorRequest {
    /// The request that holds a body exactly where and as it is.
    pub fn still(heading: Vec2) -> MotorRequest {
        MotorRequest { heading, speed: 0.0 }
    }
}

/// The physical envelope one body faces this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotorLimits {
    /// `r`: **the radius the model in force puts in the rotation term**, px
    /// ([`turn_radius_px_in`]). Under [`MotorModel::Sweep`] that is the outer radius of the
    /// physical body — the outermost contacting point, the apex's grasp included. Under
    /// [`MotorModel::Inertial`] it is already the disc's radius of gyration, `lobes/√2`, with
    /// the grasp excluded, so the resolver's `radius · |ω|` is `v_rot` with no further
    /// arithmetic and the neural adapter's `Envelope` reads the same number the envelope uses.
    pub radius_px: f64,
    /// `ω_max`: an *additional* ceiling on `|ω|`, rad/s, applied before the shared budget.
    /// It never enlarges the budget; it can only clip a request further.
    pub turn_rate_max: f64,
    /// The translation ceiling after effort, morphology, wading and any burst, px/s. **This
    /// is the whole capability**: `|v| + r · |ω| ≤ speed_cap` (and `≤ motor_budget`).
    pub speed_cap: f64,
    /// The motor magnitude this tick's energy pays for after upkeep, px/s
    /// ([`MotorBill::affordable_motor`]). `f64::INFINITY` means movement is free.
    pub motor_budget: f64,
    /// Seconds in the tick.
    pub dt: f64,
}

impl MotorLimits {
    /// `speed_cap`: the whole `|v| + r · |ω|` this body could spend if energy were free.
    ///
    /// **R0b.** This used to be `speed_cap + `[`REFERENCE_RADIUS_PX`]` · turn_rate_max`, which
    /// handed every body an independent rotation allowance the envelope then never bound.
    /// `turn_rate_max` is now an extra ceiling applied in [`resolve`] and nothing more, so
    /// effort, morphology, wading and the burst list are the only things that set the budget —
    /// and a body throttled by any of them is throttled in *both* channels at once.
    pub fn capability(&self) -> f64 {
        finite_non_negative(self.speed_cap)
    }

    /// The purse: [`MotorLimits::motor_budget`] sanitized to a non-negative number (NaN is no
    /// budget at all), `f64::INFINITY` when motion is free.
    ///
    /// Under [`MotorModel::Sweep`] this bounds the same quantity the capability does; under
    /// [`MotorModel::Inertial`] it bounds the *billed* motion while the capability bounds the
    /// envelope magnitude, which is why the two are available separately.
    pub fn budget(&self) -> f64 {
        if self.motor_budget.is_nan() {
            0.0
        } else {
            self.motor_budget.max(0.0)
        }
    }

    /// `u`: the capability, capped by what the energy after upkeep actually buys. This is the
    /// [`MotorModel::Sweep`] envelope's single bound.
    pub fn available(&self) -> f64 {
        self.capability().min(self.budget())
    }
}

/// The motion the world applies, all in the pre-transport chart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedMotion {
    /// The unit heading after the bounded turn.
    pub heading: Vec2,
    /// Centre speed actually granted, px/s.
    pub speed: f64,
    /// The signed physical turn actually performed this tick, radians in `(−π, π]`. This —
    /// never the request, never a chart jump — is what the angular cost is charged on.
    pub turn: f64,
    /// The **rotation speed** the model in force resolved, px/s: `r · |ω|` — the outermost
    /// point's sweep — under [`MotorModel::Sweep`], and `v_rot = r·|ω|/√2` under
    /// [`MotorModel::Inertial`]. It is always `MotorLimits::radius_px · |ω|`, because the
    /// limits already carry the model's radius.
    ///
    /// The bill prices it at [`MotorModel::rotation_price`]; the envelope combines it with the
    /// speed at [`MotorModel::envelope_magnitude`].
    pub sweep: f64,
}

impl ResolvedMotion {
    /// The resolved turn rate, rad/s.
    pub fn omega(&self, dt: f64) -> f64 {
        if dt > 0.0 { self.turn / dt } else { 0.0 }
    }

    /// `|v| + rotation`: the **linear** motor magnitude.
    ///
    /// Under [`MotorModel::Sweep`] this is the quantity the envelope bounds, and it is always
    /// `≤ MotorLimits::available()`. Under [`MotorModel::Inertial`] the envelope bounds
    /// `√(v² + v_rot²)` instead — see [`ResolvedMotion::envelope_magnitude`] — and this sum is
    /// the *billed* motion, always within [`MotorLimits::motor_budget`] and at most `√2` times
    /// the capability. It is kept as the one linear measure so that the recurrent feedback
    /// channel, whose requested half is the adapter's `v_req + r·|ω_req|`, compares like with
    /// like under both models.
    pub fn motor_magnitude(&self) -> f64 {
        self.speed + self.sweep
    }

    /// The quantity the envelope actually bounds under `model`: `≤ MotorLimits::capability()`.
    pub fn envelope_magnitude(&self, model: MotorModel) -> f64 {
        model.envelope_magnitude(self.speed, self.sweep)
    }

    /// A body that neither moved nor turned.
    pub fn still(heading: Vec2) -> ResolvedMotion {
        ResolvedMotion { heading, speed: 0.0, turn: 0.0, sweep: 0.0 }
    }
}

/// Turn a request into motion. Pure: no draws, no state, no world access.
///
/// Order of operations, all documented above: clamp the requested turn to the angular
/// ceiling, clamp the requested speed to the translation ceiling, then scale **both** by the
/// single factor that brings `|v| + r · |ω|` inside `u`.
///
/// Since R0b the translation ceiling *is* `u` (when energy is not the binder), so a body that
/// asks for full speed and any turn at all is always scaled: every radian is paid for in
/// pixels of travel given up, at exactly `r` px per radian. That is the shared budget, and it
/// is why a caller that wants to keep travelling asks for a small turn rather than a large one.
pub fn resolve(
    current_heading: Vec2,
    request: &MotorRequest,
    limits: &MotorLimits,
) -> ResolvedMotion {
    resolve_in(current_heading, request, limits, MotorModel::Sweep)
}

/// [`resolve`], under a named [`MotorModel`]. `MotorModel::Sweep` is [`resolve`] itself, arithmetic
/// for arithmetic.
///
/// The two models share everything up to the scaling factor: the requested turn is clamped to
/// the angular ceiling, the requested speed to the translation ceiling, the rotation speed is
/// `limits.radius_px · |ω|` (the limits already carry the model's radius), and both channels
/// are then multiplied by **one** common factor so the caller's split survives.
///
/// What differs is how that factor is found.
///
/// - `Sweep` collapses the capability and the purse onto one number,
///   `u = min(speed_cap, motor_budget)`, and bounds `|v| + r·|ω|` by it. `motor_budget` is
///   sized by [`MotorBill::affordable_motor`], which prices the whole magnitude at the dearer
///   of its two halves; a turning body is therefore throttled a little more than its own bill
///   would require. That conservatism is shipped behaviour and is kept exactly.
/// - `Inertial` keeps the two constraints apart, because under it they bound *different*
///   quantities: the capability bounds the envelope magnitude `√(v² + v_rot²)`, and the purse
///   bounds the billed motion `|v| + v_rot`. Both are positively homogeneous in the request,
///   so each yields its own ratio and the factor is the smaller. Nothing is approximated: the
///   charge is exactly what the energy pays for, and a body that only translates sees the two
///   constraints coincide and moves exactly as it does under `Sweep`.
pub fn resolve_in(
    current_heading: Vec2,
    request: &MotorRequest,
    limits: &MotorLimits,
    model: MotorModel,
) -> ResolvedMotion {
    let Some(current) = current_heading.normalized() else {
        // A body without a usable heading cannot be turned relative to one; adopt the
        // request's own direction if it has one and stand still this tick.
        let heading = request.heading.normalized().unwrap_or(current_heading);
        return ResolvedMotion::still(heading);
    };
    let dt = limits.dt;
    if dt <= 0.0 || !dt.is_finite() {
        return ResolvedMotion::still(current);
    }

    // The signed shortest physical turn the caller asked for, then the morphological ceiling.
    let target = request.heading.normalized();
    let requested_turn = target.map_or(0.0, |t| wrap_pi(t.screen_angle() - current.screen_angle()));
    let ceiling = finite_non_negative(limits.turn_rate_max) * dt;
    let turn_req = requested_turn.clamp(-ceiling, ceiling);
    let speed_req = finite_non_negative(request.speed).min(finite_non_negative(limits.speed_cap));

    let radius = finite_non_negative(limits.radius_px);
    let rotation_req = radius * (turn_req / dt).abs();
    let scale = match model {
        MotorModel::Sweep => {
            let demand = speed_req + rotation_req;
            let available = limits.available();
            if demand > available {
                if demand > 0.0 { available / demand } else { 0.0 }
            } else {
                1.0
            }
        }
        MotorModel::Inertial => {
            let envelope_demand = MotorModel::Inertial.envelope_magnitude(speed_req, rotation_req);
            let billed_demand = speed_req + rotation_req;
            let by_capability = bound(envelope_demand, limits.capability());
            let by_purse = bound(billed_demand, limits.budget());
            by_capability.min(by_purse)
        }
    };

    let speed = speed_req * scale;
    let mut turn = turn_req * scale;
    // A request the envelope did not materially reduce is handed back exactly as it came in,
    // rather than rebuilt from an angle: `screen_angle` and `from_screen_angle` are inverses
    // only to within rounding, and a trajectory must not drift by ulps a tick just because it
    // passed through the resolver. See [`TURN_SLACK_RAD`].
    let heading = if (requested_turn - turn).abs() <= TURN_SLACK_RAD {
        turn = requested_turn;
        match target {
            Some(t) => {
                if is_unit(request.heading) {
                    request.heading
                } else {
                    t
                }
            }
            None => current,
        }
    } else if turn == 0.0 {
        current
    } else {
        Vec2::from_screen_angle(current.screen_angle() + turn)
            .normalized()
            .unwrap_or(current)
    };
    let sweep = radius * (turn / dt).abs();
    ResolvedMotion { heading, speed, turn, sweep }
}

/// How far the resolved turn may sit from the requested one and still count as *unbound*.
///
/// This is a numerical no-op guard, not a motor deadband. Reading a requested turn back out of
/// a heading costs an `atan2`/`sin`-`cos` round trip, which lands within a few ulps of the
/// angle that produced it — occasionally a few ulps *past* the angular ceiling. Without this,
/// every ordinary tick would rebuild the heading from an angle and drift the world by
/// rounding alone. At 1e-12 rad (6 × 10⁻¹¹ degrees, under 10⁻¹⁰ px of sweep on the widest
/// body in the world) it is far below anything physical or visible, and a request that binds
/// by more than this is scaled exactly as documented.
const TURN_SLACK_RAD: f64 = 1e-12;

/// Is this vector already unit length to the tolerance `crate::world` validates headings at?
fn is_unit(v: Vec2) -> bool {
    v.is_finite() && (v.length() - 1.0).abs() <= 1e-6
}

/// `r` for one organism: the radius of the outermost *physical* point that sweeps when the
/// body pivots about its position, in pixels.
///
/// For ordinary fauna this is [`crate::genome::Phenotype::extent`], the decoded lobe extent —
/// real geometry, at the body's own current scale, never a hardcoded radius and never a
/// rendered pixel coordinate.
///
/// An apex member reaches further than its lobes: its claws close about `|capture_offset| +
/// capture_reach` ahead of the root, already multiplied by the member's own body scale
/// ([`crate::hunter::ContactGeometry`]), and that grasp is the part of it that actually
/// contacts the world. `apex` supplies that geometry, and the radius is the larger of the
/// two. The renderer's `visual_query_extent_px` is deliberately excluded: it is the support
/// the art needs around the root, not a physical part of the animal.
///
/// **Conservative approximation.** This charges the *outermost* point's sweep, not an
/// area- or mass-weighted mean over the body. For a long thin animal that overstates the
/// effort of turning, so the budget errs toward slower rotation rather than free rotation.
/// It is a stylized budget, not a moment of inertia.
pub fn turn_radius_px(organism: &Organism, apex: Option<&crate::hunter::ContactGeometry>) -> f64 {
    turn_radius_px_in(organism, apex, MotorModel::Sweep)
}

/// [`turn_radius_px`] under a named [`MotorModel`]: **the radius that model puts in the
/// rotation term**, which is what [`MotorLimits::radius_px`] wants and what the neural
/// adapter's `Envelope` and the observation must be told.
///
/// - [`MotorModel::Sweep`] is [`turn_radius_px`] itself: the outermost *contacting* point, so
///   `max(lobes, |capture_offset| + capture_reach)` for an apex member.
/// - [`MotorModel::Inertial`] models the body as a uniform disc of radius
///   [`crate::genome::Phenotype::extent`] and returns its **radius of gyration**, `lobes/√2`,
///   so that `radius · |ω|` is the translation speed of equal kinetic energy. `apex` is
///   deliberately ignored: a grasp is contact geometry, it carries no mass worth turning, and
///   pricing 14.8 px of claw reach as a turn radius is what
///   `design/7_Research/ecology-v1-apex-predicate-2026-09-16.md` measured taking 64 % of the
///   apex's boosted budget.
///
/// One consequence worth naming: `crate::world::view::neural_observation` and
/// `crate::world::step::neural_decision` already read this with `apex = None`, so under
/// `Inertial` the radius a body is *told* and the radius its envelope *uses* are the same
/// number for every body in the world, apex included, for the first time.
pub fn turn_radius_px_in(
    organism: &Organism,
    apex: Option<&crate::hunter::ContactGeometry>,
    model: MotorModel,
) -> f64 {
    let lobes = finite_non_negative(organism.phenotype.extent);
    match model {
        MotorModel::Sweep => match apex {
            Some(g) => lobes.max(finite_non_negative(
                g.capture_offset_body.length() + g.capture_reach_px,
            )),
            None => lobes,
        },
        MotorModel::Inertial => MotorModel::Inertial.rotation_radius_px(lobes),
    }
}

/// What one tick of living and moving costs, and therefore how much motion the remaining
/// energy can buy.
///
/// The split is the physiology ordering the world already used: `maintenance · S` and
/// `sense_cost · r_sense` are **unavoidable upkeep** and are reserved first, and only what is
/// left pays for motion. Before this milestone the whole bill was computed *after* the move
/// and then truncated by `min(cost, E)` — an exhausted body moved anyway and simply paid what
/// it had. Now the affordable motor magnitude is computed *before* the move and caps the
/// envelope, so a body with no movement energy holds still.
///
/// Neither `maintenance` nor the metabolism behind it is changed here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotorBill {
    pub structure: f64,
    pub maintenance: f64,
    pub sense_radius: f64,
    pub move_cost: f64,
    pub sense_cost: f64,
}

impl MotorBill {
    pub fn of(o: &Organism, config: &WorldConfig) -> MotorBill {
        MotorBill {
            structure: o.structure,
            maintenance: o.phenotype.maintenance,
            sense_radius: o.phenotype.sense_radius,
            move_cost: config.organism.move_cost,
            sense_cost: config.organism.sense_cost,
        }
    }

    /// `(maintenance · S + sense_cost · r_sense) · dt`: what this tick costs even standing
    /// perfectly still.
    pub fn upkeep(&self, dt: f64) -> f64 {
        (self.maintenance * self.structure + self.sense_cost * self.sense_radius) * dt
    }

    /// `move_cost · S · dt`: the energy one px/s of motor magnitude costs for a whole tick.
    fn per_motor(&self, dt: f64) -> f64 {
        self.move_cost * self.structure * dt
    }

    /// The largest `|v| + r · |ω|` this `energy` can pay for once upkeep is reserved, px/s.
    /// `f64::INFINITY` when motion is free (`move_cost` or `S` is zero), which is the only
    /// case the envelope's `capability` alone decides.
    ///
    /// The envelope bounds one magnitude while the bill prices its two halves differently, so
    /// this prices the whole magnitude at whichever half is dearer. The budget is then never
    /// larger than the body can actually pay for, whatever split the resolver lands on.
    ///
    /// **The same number under both [`MotorModel`]s, and deliberately so.** Under `Sweep` the
    /// dearer half is translation (`max(k, 1) = 1`); under `Inertial` this bounds the *billed*
    /// motion `|v| + v_rot`, whose price is exactly one `move_cost · S · dt` per px/s. The two
    /// arrive at `(E − upkeep) / (move_cost · S · dt)` by different routes, which is what makes
    /// a purely translating body identical under both models at every energy.
    pub fn affordable_motor(&self, energy: f64, dt: f64) -> f64 {
        let per_motor = self.per_motor(dt) * ROTATION_COST_SCALE.max(1.0);
        if per_motor <= 0.0 || !per_motor.is_finite() {
            return f64::INFINITY;
        }
        (energy - self.upkeep(dt)).max(0.0) / per_motor
    }

    /// `move_cost · S · (|v| + k · r|ω|) · dt`, with `k` = [`ROTATION_COST_SCALE`]: the cost of
    /// the motion that was actually resolved.
    ///
    /// Translation and turning are billed **once each**, through one term, at the world's
    /// existing `move_cost`. A body that only translates pays exactly what it paid before this
    /// milestone.
    pub fn motor_cost(&self, speed: f64, sweep: f64, dt: f64) -> f64 {
        self.motor_cost_in(speed, sweep, dt, MotorModel::Sweep)
    }

    /// [`MotorBill::motor_cost`] under a named [`MotorModel`].
    pub fn motor_cost_in(&self, speed: f64, sweep: f64, dt: f64, model: MotorModel) -> f64 {
        self.per_motor(dt) * billed_motion(speed, sweep, model)
    }

    /// The whole tick's bill: upkeep plus the resolved motion, in the world's own
    /// association `(maintenance · S + move_cost · S · swept + sense_cost · r_sense) · dt`.
    ///
    /// This is the single charge the world books. It equals `upkeep + motor_cost` to within
    /// floating-point association only — those two exist to *size* the budget before the
    /// move, this to bill it afterwards — which is why `crate::world` still clamps the charge
    /// to the energy on hand. Keeping the original association means a body that only
    /// translates pays the pre-R0a bill bit for bit.
    pub fn total_cost(&self, speed: f64, sweep: f64, dt: f64) -> f64 {
        self.total_cost_in(speed, sweep, dt, MotorModel::Sweep)
    }

    /// [`MotorBill::total_cost`] under a named [`MotorModel`], in the world's own association.
    ///
    /// The only thing the model changes here is the price of the rotation term: `k` under
    /// `Sweep`, one under `Inertial` — where the mean-radius discount already lives in the
    /// radius that produced `sweep`. **A body that only translates therefore pays exactly the
    /// same number under both models, bit for bit**, which is the invariant the paired
    /// experiment rests on.
    pub fn total_cost_in(&self, speed: f64, sweep: f64, dt: f64, model: MotorModel) -> f64 {
        (self.maintenance * self.structure
            + self.move_cost * self.structure * billed_motion(speed, sweep, model)
            + self.sense_cost * self.sense_radius)
            * dt
    }
}

/// `|v| + price · rotation`: the motion the bill actually prices, as opposed to the motion the
/// envelope bounds. Under [`MotorModel::Sweep`] `price` is [`ROTATION_COST_SCALE`] and the
/// rotation is the outer point's sweep; under [`MotorModel::Inertial`] it is one and the
/// rotation is `v_rot`.
fn billed_motion(speed: f64, rotation: f64, model: MotorModel) -> f64 {
    finite_non_negative(speed) + model.rotation_price() * finite_non_negative(rotation)
}

/// The common factor that brings `demand` inside `limit`: 1 when it already fits, `limit /
/// demand` when it does not, and 0 for a degenerate demand there is no direction to scale.
fn bound(demand: f64, limit: f64) -> f64 {
    if demand > limit {
        if demand > 0.0 { limit / demand } else { 0.0 }
    } else {
        1.0
    }
}

/// `x` if it is finite and positive, else 0.
fn finite_non_negative(x: f64) -> f64 {
    if x.is_finite() && x > 0.0 { x } else { 0.0 }
}

/// Wrap an angle into `(−π, π]`.
fn wrap_pi(a: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    if !a.is_finite() {
        return 0.0;
    }
    let mut a = a % TAU;
    if a > PI {
        a -= TAU;
    } else if a < -PI {
        a += TAU;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::DT;
    use crate::config::{DriveConfig, OrganismConfig};
    use crate::genome::{Genome, decode};

    const FREE: f64 = f64::INFINITY;

    /// An adult apex body from the trial profile's own genome, decoded by the world's rule.
    fn apex_body(
        profile: &crate::hunter::FixedHunterProfile,
        cfg: &WorldConfig,
    ) -> Organism {
        use cubarium_surface::{Face, SurfacePoint};

        use crate::organism::Mode;
        use crate::rng::Counter;

        let phenotype = decode(&profile.genome, &cfg.organism);
        Organism {
            pos: SurfacePoint::new(Face::Front, 10.0, 10.0),
            heading: Vec2::new(1.0, 0.0),
            ou: Vec2::ZERO,
            structure: phenotype.structure_adult,
            reserve: phenotype.reserve_max * 0.5,
            energy: phenotype.energy_max * 0.5,
            born_tick: 0,
            hunger_memory: 0.0,
            mode: Mode::Seeking,
            escrow: None,
            births: 0,
            genome: profile.genome.clone(),
            phenotype,
            parent: None,
            origin: crate::organism::Origin::Founder,
            turn_counter: Counter::default(),
            fed_this_tick: false,
        }
    }

    fn limits(radius_px: f64, turn_rate_max: f64, speed_cap: f64) -> MotorLimits {
        MotorLimits { radius_px, turn_rate_max, speed_cap, motor_budget: FREE, dt: DT }
    }

    /// The one calibration constant is the world's own unit adult, not a free parameter.
    #[test]
    fn reference_radius_is_the_unit_adult_extent() {
        let cfg = OrganismConfig::default();
        let unit = Genome::founder(0.5, &DriveConfig::default());
        assert_eq!(unit.size, 1.0, "the founder genome is the unit body");
        assert_eq!(decode(&unit, &cfg).extent, REFERENCE_RADIUS_PX);
    }

    #[test]
    fn zero_requests_hold_the_body_still() {
        let h = Vec2::new(1.0, 0.0);
        let m = resolve(h, &MotorRequest::still(h), &limits(2.5, 1.5, 0.3));
        assert_eq!(m, ResolvedMotion::still(h));
        assert_eq!(m.motor_magnitude(), 0.0);
    }

    /// Translation alone is never reduced: `speed_cap ≤ capability` by construction.
    #[test]
    fn pure_translation_is_granted_whole() {
        let h = Vec2::new(1.0, 0.0);
        for &(r, cap) in &[(2.5, 0.3), (9.0, 0.3), (0.5, 4.0)] {
            let l = limits(r, 90.0f64.to_radians(), cap);
            let m = resolve(h, &MotorRequest { heading: h, speed: cap }, &l);
            assert_eq!(m.speed, cap, "radius {r}");
            assert_eq!(m.turn, 0.0);
            assert_eq!(m.sweep, 0.0);
            assert_eq!(m.motor_magnitude(), cap);
            assert_eq!(m.heading, h);
        }
    }

    /// A stationary body spends the whole budget on rotation — the behaviour Wrysk asked for
    /// explicitly. A comparison of sweep against centre speed would give zero here.
    ///
    /// **R0b.** The rate is now `u / r` with `u = speed_cap`, not the genome's `turn_rate_max`:
    /// before the correction a unit adult pivoted at its full 90°/s because the capability
    /// carried a rotation allowance of its own. It pivots at 6.9°/s now, and the budget splits
    /// at the body's *own* radius, so twice as wide is exactly half as fast.
    #[test]
    fn pure_pivot_is_legal_and_bounded_by_the_budget_over_the_radius() {
        let h = Vec2::new(1.0, 0.0);
        let omega_max = 90.0f64.to_radians();
        let cap = 0.3;
        let back = Vec2::new(-1.0, 0.0);

        // A unit adult: the whole translation budget, spent on turning.
        let l = limits(REFERENCE_RADIUS_PX, omega_max, cap);
        let m = resolve(h, &MotorRequest { heading: back, speed: 0.0 }, &l);
        assert_eq!(m.speed, 0.0, "a pivot needs no forward motion");
        assert!(m.turn.abs() > 0.0, "a pure pivot is still legal");
        assert!(
            (m.turn.abs() / DT - cap / REFERENCE_RADIUS_PX).abs() < 1e-15,
            "{} rad/s is not u/r",
            m.turn.abs() / DT
        );
        assert!((m.motor_magnitude() - cap).abs() < 1e-15, "the whole budget went to the turn");
        assert!(m.turn.abs() < omega_max * DT, "6.9°/s, not the genome's 90°/s");

        // Twice the radius, exactly half the sweep rate the same budget buys.
        let wide = limits(2.0 * REFERENCE_RADIUS_PX, omega_max, cap);
        let m2 = resolve(h, &MotorRequest { heading: back, speed: 0.0 }, &wide);
        assert!(
            (m2.turn.abs() / DT - cap / (2.0 * REFERENCE_RADIUS_PX)).abs() < 1e-15,
            "{}",
            m2.turn.abs() / DT
        );
        assert!((m2.turn.abs() * 2.0 - m.turn.abs()).abs() < 1e-15, "half, exactly");
        assert!((m2.motor_magnitude() - cap).abs() < 1e-15, "the same budget, either way");

        // A body with no translation budget has no turning budget either: that is the whole
        // point of a shared one.
        let broke = limits(REFERENCE_RADIUS_PX, omega_max, 0.0);
        assert_eq!(resolve(h, &MotorRequest { heading: back, speed: 0.0 }, &broke), ResolvedMotion::still(h));
    }

    /// **R0b regression.** The angular ceiling is a ceiling and never an addend: raising it,
    /// by a genome or by a threatened prey's escape override, cannot buy one extra pixel per
    /// second of anything. Before R0b each of these limits had a different capability.
    #[test]
    fn an_angular_ceiling_never_enlarges_the_budget() {
        let cap = 0.3;
        for &omega_max in &[0.0, 90.0f64.to_radians(), 240.0f64.to_radians(), 1e6] {
            let l = limits(REFERENCE_RADIUS_PX, omega_max, cap);
            assert_eq!(l.capability(), cap, "ceiling {omega_max} moved the capability");
            assert_eq!(l.available(), cap, "ceiling {omega_max} moved the budget");
        }
        // It still clips: a genome that cannot turn fast does not turn fast.
        let h = Vec2::new(1.0, 0.0);
        let back = Vec2::new(-1.0, 0.0);
        let slow = limits(REFERENCE_RADIUS_PX, 0.01, cap);
        let m = resolve(h, &MotorRequest { heading: back, speed: 0.0 }, &slow);
        assert!((m.turn.abs() / DT - 0.01).abs() < 1e-15, "{}", m.turn.abs() / DT);
        assert!(m.motor_magnitude() < cap, "the ceiling bound before the budget did");
    }

    /// Wading and a burst reach the envelope through one number — `speed_cap` — so they now
    /// throttle and lift *turning* exactly as they throttle and lift travel.
    #[test]
    fn wading_and_bursts_move_the_whole_budget() {
        let h = Vec2::new(1.0, 0.0);
        let back = Vec2::new(-1.0, 0.0);
        let omega_max = 90.0f64.to_radians();
        let dry = limits(REFERENCE_RADIUS_PX, omega_max, 0.3);
        // `1 + w · (1 − swim)` with a full pool and a non-swimmer: half speed.
        let waded = limits(REFERENCE_RADIUS_PX, omega_max, 0.3 / 2.0);
        // A threatened prey's `escape_speed_multiple` of 2.
        let burst = limits(REFERENCE_RADIUS_PX, omega_max, 0.3 * 2.0);

        let pivot = |l: &MotorLimits| resolve(h, &MotorRequest { heading: back, speed: 0.0 }, l).turn.abs() / DT;
        assert!((pivot(&waded) * 2.0 - pivot(&dry)).abs() < 1e-15, "wading did not slow the turn");
        assert!((pivot(&burst) - pivot(&dry) * 2.0).abs() < 1e-15, "the burst did not lift the turn");
        // Still the same one bound in every case.
        for l in [&dry, &waded, &burst] {
            let m = resolve(h, &MotorRequest { heading: back, speed: 0.15 }, l);
            assert!(m.motor_magnitude() <= l.available() * (1.0 + 1e-12));
        }
    }

    /// Legacy resting effort is small, not zero, and the shared budget makes a resting body's
    /// sweep small with it. Under the old capability a Resting body still swept 3.9 px/s.
    #[test]
    fn a_resting_body_barely_sweeps() {
        let h = Vec2::new(1.0, 0.0);
        let rest_effort = DriveConfig::default().rest_effort;
        assert!(rest_effort > 0.0, "resting effort is not literally zero");
        let cap = rest_effort * OrganismConfig::default().speed_max;
        let l = limits(REFERENCE_RADIUS_PX, 90.0f64.to_radians(), cap);
        let m = resolve(h, &MotorRequest { heading: Vec2::new(-1.0, 0.0), speed: cap }, &l);
        assert!(m.sweep <= cap, "{} px/s of sweep on a {cap} px/s budget", m.sweep);
        // R0d pace calibration: the same 5% resting share of a 16.667x larger cruise.
        // Measured 0.2350 px/s = 0.047 BL/s of the unit adult — a half turn in 33 s.
        assert!(m.sweep < 0.24, "a resting body swept {} px/s", m.sweep);
        assert!(m.sweep > 0.0, "resting is not paralysis");
    }

    /// Both channels shrink by one common factor, so the requested split survives.
    #[test]
    fn simultaneous_requests_scale_by_one_common_factor() {
        let h = Vec2::new(1.0, 0.0);
        let omega_max = 90.0f64.to_radians();
        let r = 6.0;
        let l = limits(r, omega_max, 0.3);
        let m = resolve(h, &MotorRequest { heading: Vec2::new(-1.0, 0.0), speed: 0.3 }, &l);
        let demand = 0.3 + r * omega_max;
        let u = l.capability();
        assert!(demand > u, "this fixture must actually bind");
        let s = u / demand;
        assert!((m.speed - 0.3 * s).abs() < 1e-12, "{}", m.speed);
        assert!((m.turn.abs() - omega_max * DT * s).abs() < 1e-15, "{}", m.turn);
        assert!(
            (m.motor_magnitude() - u).abs() < 1e-12,
            "the envelope is met exactly: {}",
            m.motor_magnitude()
        );
        // The ratio the caller asked for is what it got.
        assert!(((m.speed / m.turn.abs()) - (0.3 / (omega_max * DT))).abs() < 1e-9);
    }

    /// A target the budget can actually reach within the tick is reached exactly.
    ///
    /// **R0b.** "Reachable" is now `u · dt / r` — 0.006 rad for a unit adult at full effort,
    /// not the 0.0785 rad the genome's angular ceiling alone would allow — so this fixture
    /// asks for an angle inside the *shared* budget. The bit-for-bit pass-through it pins is
    /// unchanged; only what counts as unbound moved.
    #[test]
    fn a_reachable_target_is_reached_exactly_and_the_short_way() {
        let h = Vec2::new(1.0, 0.0);
        let l = limits(REFERENCE_RADIUS_PX, 90.0f64.to_radians(), 0.3);
        let reachable = l.available() * DT / REFERENCE_RADIUS_PX;
        assert!(reachable > 0.0 && reachable < 90.0f64.to_radians() * DT);
        let target = Vec2::from_screen_angle(-reachable * 0.5).normalized().expect("unit");
        let m = resolve(h, &MotorRequest { heading: target, speed: 0.0 }, &l);
        assert_eq!(m.heading, target, "no round trip through an angle");

        // Just past 180°: the short way is negative, never a full sweep the other way.
        let behind = Vec2::from_screen_angle(std::f64::consts::PI + 0.1);
        let m = resolve(h, &MotorRequest { heading: behind, speed: 0.0 }, &l);
        assert!(m.turn < 0.0, "{}", m.turn);
    }

    /// Low and zero movement energy: upkeep first, and then nothing is granted.
    #[test]
    fn an_exhausted_body_neither_moves_nor_turns() {
        let bill = MotorBill {
            structure: 1.0,
            maintenance: 0.005,
            sense_radius: 6.0,
            move_cost: 0.006,
            sense_cost: 0.0002,
        };
        let upkeep = bill.upkeep(DT);
        assert!(upkeep > 0.0);
        assert_eq!(bill.affordable_motor(0.0, DT), 0.0);
        assert_eq!(bill.affordable_motor(upkeep, DT), 0.0);

        let h = Vec2::new(1.0, 0.0);
        let mut l = limits(REFERENCE_RADIUS_PX, 90.0f64.to_radians(), 0.3);
        l.motor_budget = bill.affordable_motor(upkeep, DT);
        let m = resolve(h, &MotorRequest { heading: Vec2::new(0.0, 1.0), speed: 0.3 }, &l);
        assert_eq!(m, ResolvedMotion::still(h), "no free movement once energy is gone");

        // A sliver of movement energy buys a proportional sliver of motion.
        l.motor_budget = bill.affordable_motor(upkeep + bill.motor_cost(0.1, 0.0, DT), DT);
        assert!((l.motor_budget - 0.1).abs() < 1e-12, "{}", l.motor_budget);
        let m = resolve(h, &MotorRequest { heading: Vec2::new(0.0, 1.0), speed: 0.3 }, &l);
        assert!(
            (m.motor_magnitude() - 0.1).abs() < 1e-12,
            "{}",
            m.motor_magnitude()
        );
    }

    /// Charging a pure translation is arithmetically what the world charged before R0a.
    #[test]
    fn a_translating_body_pays_exactly_the_pre_r0a_bill() {
        let bill = MotorBill {
            structure: 1.3,
            maintenance: 0.005,
            sense_radius: 6.0,
            move_cost: 0.006,
            sense_cost: 0.0002,
        };
        let speed = 0.21;
        let legacy = (bill.maintenance * bill.structure
            + bill.move_cost * bill.structure * speed
            + bill.sense_cost * bill.sense_radius)
            * DT;
        assert_eq!(bill.total_cost(speed, 0.0, DT), legacy);
        // The budget split reconciles with the charge to within association.
        let split = bill.upkeep(DT) + bill.motor_cost(speed, 0.0, DT);
        assert!((split - legacy).abs() <= 4.0 * f64::EPSILON * legacy, "{split} vs {legacy}");
    }

    /// The price of turning is a separate, named number from the radius that bounds it, and
    /// the bill uses it while the envelope does not.
    #[test]
    fn rotation_is_priced_by_its_own_scale_and_the_envelope_is_not() {
        let bill = MotorBill {
            structure: 1.0,
            maintenance: 0.005,
            sense_radius: 6.0,
            move_cost: 0.006,
            sense_cost: 0.0002,
        };
        let (speed, sweep) = (0.2, 1.4);
        let expected = bill.upkeep(DT)
            + bill.move_cost * bill.structure * (speed + ROTATION_COST_SCALE * sweep) * DT;
        assert!((bill.total_cost(speed, sweep, DT) - expected).abs() < 1e-15);

        // The envelope still bounds the unpriced magnitude, so lowering the price cannot let a
        // body turn faster than its geometry allows.
        let l = limits(6.0, 90.0f64.to_radians(), 0.3);
        let m = resolve(
            Vec2::new(1.0, 0.0),
            &MotorRequest { heading: Vec2::new(-1.0, 0.0), speed: 0.3 },
            &l,
        );
        assert!((m.motor_magnitude() - l.capability()).abs() < 1e-12);

        // And the budget is never generous: whatever split lands, the charge fits the energy.
        let energy = bill.upkeep(DT) + bill.motor_cost(0.0, 1.0, DT);
        let budget = bill.affordable_motor(energy, DT);
        for split in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let (v, w) = (budget * split, budget * (1.0 - split));
            assert!(
                bill.total_cost(v, w, DT) <= energy + 1e-15,
                "split {split} billed {} against {energy}",
                bill.total_cost(v, w, DT)
            );
        }
    }

    /// Juvenile and adult apex bodies read their radius from real geometry, and the claws
    /// count while the renderer's query support does not.
    #[test]
    fn the_apex_radius_is_the_grasp_not_the_visual_support() {
        use crate::hunter::{ContactGeometry, FixedHunterProfile};

        let cfg = WorldConfig::default();
        let profile = FixedHunterProfile::lanternjaw_trial(&cfg);
        let mut o = apex_body(&profile, &cfg);
        let adult = ContactGeometry::of(&profile, &o);
        let r = turn_radius_px(&o, Some(&adult));
        let grasp = adult.capture_offset_body.length() + adult.capture_reach_px;
        assert!((r - grasp).abs() < 1e-12, "{r} vs {grasp}");
        assert!(r > o.phenotype.extent, "the claws reach past the lobes");
        assert!(r < adult.visual_query_extent_px, "the art's query support is not the body");

        // A juvenile is the same rig, smaller — and its radius shrinks with it.
        o.structure = o.phenotype.structure_adult * 0.25;
        let young = ContactGeometry::of(&profile, &o);
        let r_young = turn_radius_px(&o, Some(&young));
        assert!(r_young < r, "{r_young} is not smaller than the adult {r}");
        assert!((r_young / r - young.scale / adult.scale).abs() < 1e-12);

        // Ordinary fauna read the lobes and nothing else.
        assert_eq!(turn_radius_px(&o, None), o.phenotype.extent);
    }

    /// Degenerate inputs cannot produce motion or a non-unit heading.
    #[test]
    fn degenerate_inputs_hold_still() {
        let h = Vec2::new(1.0, 0.0);
        let nan = MotorRequest { heading: Vec2::new(f64::NAN, 0.0), speed: f64::NAN };
        let m = resolve(h, &nan, &limits(2.5, 1.5, 0.3));
        assert_eq!(m, ResolvedMotion::still(h));

        let zero_dt = MotorLimits { dt: 0.0, ..limits(2.5, 1.5, 0.3) };
        let m = resolve(h, &MotorRequest { heading: Vec2::new(0.0, 1.0), speed: 1.0 }, &zero_dt);
        assert_eq!(m, ResolvedMotion::still(h));

        let m = resolve(Vec2::ZERO, &MotorRequest { heading: h, speed: 1.0 }, &limits(2.5, 1.5, 0.3));
        assert_eq!(m, ResolvedMotion::still(h));
    }
}
