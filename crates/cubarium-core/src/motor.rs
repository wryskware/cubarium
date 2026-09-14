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
//! - `capability = speed_cap + `[`REFERENCE_RADIUS_PX`]` · turn_rate_max`. `speed_cap` is the
//!   translation ceiling the caller already computed from effort, morphology, wading and any
//!   burst; `turn_rate_max` is the body's angular ceiling (the genome's `turn_rate_max_deg`,
//!   or an override such as a threatened prey's escape rate). Calibrating the budget this way
//!   means a body of exactly [`REFERENCE_RADIUS_PX`] can still do both at once, as it could
//!   before this milestone existed, and every larger body must trade.
//! - `affordable` is the motor magnitude the creature's remaining energy pays for *after*
//!   unavoidable upkeep is reserved ([`MotorBill`]).
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

/// The body radius at which the translation and rotation ceilings are *jointly* attainable,
/// in pixels.
///
/// This is the one knob that sets how hard the size penalty bites, and it is anchored to the
/// world's own decode rule rather than chosen freely: 2.5 px is the [`crate::genome::decode`]
/// extent of a unit-size adult founder (core lobe `(0, 0, 1.4)`, head `(1.6, 0, 0.9)`, tail
/// `(−1.4, 0, 0.7)`; the head lobe's `1.6 + 0.9` is the maximum). `reference_radius_is_the_unit_adult_extent`
/// re-derives it from the decoder and fails if the body plan moves.
///
/// So an ordinary adult keeps exactly the turn rate its genome asks for while translating at
/// full effort, and a body larger than that — a grown organism, or an apex whose claws reach
/// far ahead of its root — trades speed against turning.
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
/// **This is the knob for the price of turning, and it is deliberately separate from
/// [`REFERENCE_RADIUS_PX`], which is the knob for how fast a body may turn.** Before R0a
/// rotation was free; at 1.0 a unit adult turning at its genome's full rate sweeps 3.9 px/s
/// against a top centre speed of 0.3 px/s, so turning would cost an order of magnitude more
/// than going — a change to the world's energy economy far larger than the kinematic one this
/// milestone is about. 0.5 is the rod figure: honest, conservative against the mean, and still
/// a real price. The measured ecological consequence of each setting is in
/// `design/7_Research/r0a-motor-cost-ecology-2026-09-14.md`; the balance itself is Fable's.
pub const ROTATION_COST_SCALE: f64 = 0.5;

/// Free rotation is the defect R0a fixes, so a zero scale is not a tuning option.
const _: () = assert!(ROTATION_COST_SCALE > 0.0);

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
    /// `r`: the outer radius of the physical body, px ([`turn_radius_px`]).
    pub radius_px: f64,
    /// `ω_max`: the angular ceiling, rad/s, before the shared budget is applied.
    pub turn_rate_max: f64,
    /// The translation ceiling after effort, morphology, wading and any burst, px/s.
    pub speed_cap: f64,
    /// The motor magnitude this tick's energy pays for after upkeep, px/s
    /// ([`MotorBill::affordable_motor`]). `f64::INFINITY` means movement is free.
    pub motor_budget: f64,
    /// Seconds in the tick.
    pub dt: f64,
}

impl MotorLimits {
    /// `speed_cap + `[`REFERENCE_RADIUS_PX`]` · turn_rate_max`: what the body could do if
    /// energy were free.
    pub fn capability(&self) -> f64 {
        let speed = finite_non_negative(self.speed_cap);
        let turn = finite_non_negative(self.turn_rate_max);
        speed + REFERENCE_RADIUS_PX * turn
    }

    /// `u`: the capability, capped by what the energy after upkeep actually buys.
    pub fn available(&self) -> f64 {
        let budget = if self.motor_budget.is_nan() {
            0.0
        } else {
            self.motor_budget.max(0.0)
        };
        self.capability().min(budget)
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
    /// `r · |ω|`, px/s: how fast the outermost point of the body swept. The envelope bounds
    /// `speed + sweep`; the bill prices `sweep` at [`ROTATION_COST_SCALE`].
    pub sweep: f64,
}

impl ResolvedMotion {
    /// The resolved turn rate, rad/s.
    pub fn omega(&self, dt: f64) -> f64 {
        if dt > 0.0 { self.turn / dt } else { 0.0 }
    }

    /// `|v| + r · |ω|`: the quantity the envelope bounds. Always `≤ MotorLimits::available()`.
    pub fn motor_magnitude(&self) -> f64 {
        self.speed + self.sweep
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
pub fn resolve(
    current_heading: Vec2,
    request: &MotorRequest,
    limits: &MotorLimits,
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
    let demand = speed_req + radius * (turn_req / dt).abs();
    let available = limits.available();
    let scale = if demand > available {
        if demand > 0.0 { available / demand } else { 0.0 }
    } else {
        1.0
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
    let lobes = finite_non_negative(organism.phenotype.extent);
    match apex {
        Some(g) => lobes.max(finite_non_negative(
            g.capture_offset_body.length() + g.capture_reach_px,
        )),
        None => lobes,
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
        self.per_motor(dt) * self.billed_motion(speed, sweep)
    }

    /// `|v| + k · r|ω|`: the motion the bill actually prices, as opposed to the motion the
    /// envelope bounds.
    fn billed_motion(&self, speed: f64, sweep: f64) -> f64 {
        finite_non_negative(speed) + ROTATION_COST_SCALE * finite_non_negative(sweep)
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
        (self.maintenance * self.structure
            + self.move_cost * self.structure * self.billed_motion(speed, sweep)
            + self.sense_cost * self.sense_radius)
            * dt
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
    #[test]
    fn pure_pivot_is_legal_and_bounded_by_the_budget_over_the_radius() {
        let h = Vec2::new(1.0, 0.0);
        let omega_max = 90.0f64.to_radians();
        // A unit adult: the reference radius, so the full rate survives.
        let l = limits(REFERENCE_RADIUS_PX, omega_max, 0.3);
        let back = Vec2::new(-1.0, 0.0);
        let m = resolve(h, &MotorRequest { heading: back, speed: 0.0 }, &l);
        assert_eq!(m.speed, 0.0, "a pivot needs no forward motion");
        assert!((m.turn.abs() - omega_max * DT).abs() < 1e-15, "{}", m.turn);

        // Twice the radius, exactly half the sweep rate the budget buys.
        let wide = limits(2.0 * REFERENCE_RADIUS_PX, omega_max, 0.0);
        let m = resolve(h, &MotorRequest { heading: back, speed: 0.0 }, &wide);
        let expected = (0.0 + REFERENCE_RADIUS_PX * omega_max) / (2.0 * REFERENCE_RADIUS_PX);
        assert!((m.turn.abs() / DT - expected).abs() < 1e-12, "{}", m.turn / DT);
        assert!(m.turn.abs() < omega_max * DT, "a wide body turns slower than the ceiling");
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

    #[test]
    fn a_reachable_target_is_reached_exactly_and_the_short_way() {
        let h = Vec2::new(1.0, 0.0);
        let target = Vec2::new(1.0, -0.01).normalized().expect("unit");
        let l = limits(REFERENCE_RADIUS_PX, 90.0f64.to_radians(), 0.3);
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
