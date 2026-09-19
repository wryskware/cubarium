//! The seven-channel action adapter (contract §3–4).
//!
//! The network head is linear; squashing, the deadband, the capability masks and the shared
//! mouth are *actuator* semantics, identical for every controller that speaks this contract.
//! Nothing here assigns a heading or grants motion: it produces a request, and
//! [`crate::motor::resolve`] decides how much of it the envelope and the energy deliver.

use cubarium_surface::Vec2;

/// Number of action channels.
pub const ACT_LEN: usize = 7;
/// The three intake channels that share one mouth: graze, fruit, scavenge.
pub const MOUTH_CHANNELS: [usize; 3] = [2, 3, 4];
/// Deadband on the two motor and three intake channels.
pub const DEADBAND: f64 = 0.05;
/// Level trigger on attack and reproduce: these are standing requests, not per-tick attempts.
pub const LEVEL: f64 = 0.5;

/// The distinct three-channel voxel contract: forward, signed turn, local feed.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VoxelAction3(pub [f64; 3]);

/// Decodes a voxel GRU head. This is deliberately not [`ActionAdapter`].
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct VoxelActionAdapter;

impl VoxelActionAdapter {
    pub const DEADBAND: f64 = 0.05;
    pub const FORWARD: usize = 0;
    pub const TURN: usize = 1;
    pub const FEED: usize = 2;

    /// Squash a voxel head. Non-finite logits are **refused**, not coerced: a NaN head is an
    /// invalid controller outcome the caller records, never apparently valid effort.
    pub fn squash(self, logits: &[f64; 3]) -> Result<VoxelAction3, String> {
        if let Some(i) = logits.iter().position(|x| !x.is_finite()) {
            return Err(format!("voxel head channel {i} is not finite"));
        }
        let forward = band01(voxel_sigmoid(logits[Self::FORWARD]));
        let turn = band_signed(logits[Self::TURN].tanh());
        let feed = band01(voxel_sigmoid(logits[Self::FEED]));
        Ok(VoxelAction3([forward, turn, feed]))
    }
}

fn band01(x: f64) -> f64 {
    if x.is_finite() && x >= VoxelActionAdapter::DEADBAND {
        x
    } else {
        0.0
    }
}
fn band_signed(x: f64) -> f64 {
    if x.is_finite() && x.abs() >= VoxelActionAdapter::DEADBAND {
        x
    } else {
        0.0
    }
}
fn voxel_sigmoid(x: f64) -> f64 {
    debug_assert!(x.is_finite(), "callers reject non-finite logits first");
    1.0 / (1.0 + (-x).exp())
}

/// Which action adapter decodes a raw head into the action in force.
///
/// The two differ in **exactly one constant**: the deadband applied to [`TURN`]. Everything
/// else — the squash functions, the thrust and intake bands, the `LEVEL` triggers, the
/// capability masks, the shared-mouth normalisation and the order they run in — is one
/// implementation, shared, so no second adapter can drift from the first by accident.
///
/// Why it exists: workstream Q measured the centre's raw turn head at mean `|head|` 0.0600
/// against a band edge of 0.0500, so the turn channel spends ~42 % of a trajectory and
/// essentially all of a newborn candidate's ticks clipped to exactly zero
/// (`design/7_Research/ecology-v1-es-antithetic-2026-09-16.md`). Releasing that band changes
/// what a policy can *express*, so it is a different interface, not a tuning: the profile text
/// carries the adapter's name, the schema digest changes with it, and a policy trained under
/// one is refused **by name** under the other rather than reinterpreted.
///
/// [`ActionAdapter::CubAct1`] is the shipped adapter, the [`Default`], and the one the display
/// host runs. A world that never names an adapter is byte-identical to the build before this
/// type existed.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ActionAdapter {
    /// `DEADBAND` on both motor channels: the shipped contract.
    #[default]
    #[serde(rename = "cub-act-1")]
    CubAct1,
    /// The same adapter with the [`TURN`] band at 0.0. The [`THRUST`] band stays [`DEADBAND`].
    #[serde(rename = "cub-act-2")]
    CubAct2,
}

impl ActionAdapter {
    /// The token this adapter occupies in [`super::PROFILE_TEXT`], and its name everywhere
    /// else: a protocol, a policy file, a command line.
    pub fn name(self) -> &'static str {
        match self {
            ActionAdapter::CubAct1 => "cub-act-1",
            ActionAdapter::CubAct2 => "cub-act-2",
        }
    }

    /// Parse a name. An unknown name is refused rather than defaulted: silently running the
    /// shipped adapter for a misspelled `--adapter` would compare two tasks.
    pub fn parse(s: &str) -> Result<ActionAdapter, String> {
        match s {
            "cub-act-1" => Ok(ActionAdapter::CubAct1),
            "cub-act-2" => Ok(ActionAdapter::CubAct2),
            other => Err(format!(
                "adapter must be `cub-act-1` or `cub-act-2`, not `{other}`"
            )),
        }
    }

    /// Whether this is the shipped adapter. Used to keep it out of a serialized protocol, so
    /// every hash written before the switch existed keeps the value it has always had.
    pub fn is_default(&self) -> bool {
        matches!(self, ActionAdapter::CubAct1)
    }

    /// The deadband this adapter applies to [`TURN`]. **The one constant that differs.**
    pub fn turn_band(self) -> f64 {
        match self {
            ActionAdapter::CubAct1 => DEADBAND,
            ActionAdapter::CubAct2 => 0.0,
        }
    }

    /// Every adapter this build knows, for a caller that has to check a digest against all of
    /// them.
    pub const ALL: [ActionAdapter; 2] = [ActionAdapter::CubAct1, ActionAdapter::CubAct2];
}

/// Channel indices, for readers.
pub const THRUST: usize = 0;
pub const TURN: usize = 1;
pub const GRAZE: usize = 2;
pub const FRUIT: usize = 3;
pub const SCAVENGE: usize = 4;
pub const ATTACK: usize = 5;
pub const REPRODUCE: usize = 6;

/// What this body is physically able to ask for. A mask is a property of the *body*, not of
/// the policy: a masked channel never reaches the `Decision`, so a policy that learns to push
/// it simply wastes the channel rather than acquiring an ability.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capability {
    /// The phenotype's two digestive capabilities. A channel is open exactly when its
    /// machinery exists — `cap_foliage > 0` for leaf and fruit, `cap_detrital > 0` for litter
    /// and remains (`design/ecology-v1-contract.md` §6.1–6.2). The masks are world-side and
    /// identical for legacy and neural control.
    pub cap_foliage: f64,
    pub cap_detrital: f64,
    /// Whether the world's grazing and scavenging mechanisms are switched on at all.
    pub grazing: bool,
    pub scavenging: bool,
    /// Apex member with `attacks_enabled`. **False for every ordinary body**: attack keeps its
    /// channel index for versioning and is masked to zero until the §7 apex extension exists.
    pub attacks: bool,
    /// Whether this body's lifecycle reproduces at all.
    pub reproduces: bool,
}

impl Capability {
    /// The masks an ordinary (non-member) body carries.
    pub fn ordinary(
        cap_foliage: f64,
        cap_detrital: f64,
        grazing: bool,
        scavenging: bool,
    ) -> Capability {
        Capability {
            cap_foliage,
            cap_detrital,
            grazing,
            scavenging,
            attacks: false,
            reproduces: true,
        }
    }
}

/// The post-squash, post-deadband, post-mask, mouth-normalised action in force.
///
/// This is what the world holds between controller ticks and what it persists: reproducing it
/// from the raw head would need the network, so the held action is state, not a cache.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Action7(pub [f64; ACT_LEN]);

impl Default for Action7 {
    fn default() -> Self {
        Action7([0.0; ACT_LEN])
    }
}

impl Action7 {
    /// Squash, deadband, mask and normalise a raw linear head into the action in force.
    ///
    /// Order matters and is fixed: squash, then deadband, then the capability mask, then the
    /// shared-mouth normalisation. Masking after the deadband means a masked channel is zero
    /// whatever it held; normalising last means the mouth budget is shared only among the
    /// channels that survived their masks.
    pub fn squash(y: &[f64; ACT_LEN], cap: &Capability) -> Action7 {
        Action7::squash_in(y, cap, ActionAdapter::CubAct1)
    }

    /// [`Action7::squash`], under a named [`ActionAdapter`].
    ///
    /// [`ActionAdapter::CubAct1`] is [`Action7::squash`] itself, arithmetic for arithmetic.
    /// The **only** value the adapter reaches is the deadband width on [`TURN`]; there is one
    /// body of code here, not two, so the claim "they differ in exactly one constant" is a
    /// property of the implementation rather than of a comment.
    pub fn squash_in(y: &[f64; ACT_LEN], cap: &Capability, adapter: ActionAdapter) -> Action7 {
        let mut a = [0.0f64; ACT_LEN];
        a[THRUST] = band(sigmoid(y[THRUST]), DEADBAND);
        a[TURN] = band(tanh(y[TURN]), adapter.turn_band());
        a[GRAZE] = band(sigmoid(y[GRAZE]), DEADBAND);
        a[FRUIT] = band(sigmoid(y[FRUIT]), DEADBAND);
        a[SCAVENGE] = band(sigmoid(y[SCAVENGE]), DEADBAND);
        a[ATTACK] = band(sigmoid(y[ATTACK]), LEVEL);
        a[REPRODUCE] = band(sigmoid(y[REPRODUCE]), LEVEL);

        if !(cap.grazing && cap.cap_foliage > 0.0) {
            a[GRAZE] = 0.0;
        }
        if !(cap.grazing && cap.cap_foliage > 0.0) {
            a[FRUIT] = 0.0;
        }
        if !(cap.scavenging && cap.cap_detrital > 0.0) {
            a[SCAVENGE] = 0.0;
        }
        if !cap.attacks {
            a[ATTACK] = 0.0;
        }
        if !cap.reproduces {
            a[REPRODUCE] = 0.0;
        }

        // One mouth (`design/ecology-v1-contract.md` §6.3): every channel bites at the same
        // `mouth_rate`, so normalising the three efforts to sum to at most one is exactly
        // what keeps total handling inside one mouth-tick. The world applies the same rule to
        // a legacy decision, which can set all three efforts to 1 at once.
        let sum: f64 = MOUTH_CHANNELS.iter().map(|i| a[*i]).sum();
        if sum > 1.0 {
            for i in MOUTH_CHANNELS {
                a[i] /= sum;
            }
        }
        Action7(a)
    }

    /// `active`: any motor channel above its deadband. This — not the size of the request —
    /// is what gives the body its full translational capability to *split*.
    pub fn active(&self) -> bool {
        self.0[THRUST] > 0.0 || self.0[TURN] != 0.0
    }

    /// `Decision.effort`: 1 when active, 0 otherwise. Capability is a property of the body,
    /// not of how much of it the policy uses.
    pub fn effort(&self) -> f64 {
        f64::from(u8::from(self.active()))
    }

    pub fn thrust(&self) -> f64 {
        self.0[THRUST]
    }

    pub fn turn(&self) -> f64 {
        self.0[TURN]
    }

    /// Ordinary budding is a *level*: the world samples it each tick under its own funding,
    /// capacity, maturity and gestation rules.
    pub fn reproduce(&self) -> bool {
        self.0[REPRODUCE] >= LEVEL
    }
}

/// The physical envelope the adapter needs to turn a held action into this tick's request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Envelope {
    /// `v_max`, px/s.
    pub speed_max: f64,
    /// `1 + w·(1 − swim)` for the cell the body stands in.
    pub wading: f64,
    /// `r`: the outer radius that sweeps when pivoting, px.
    pub radius_px: f64,
    /// `ω_max`, rad/s: the genome's angular ceiling.
    pub turn_rate_max: f64,
    /// `u_full = min(v_max / wading, affordable_motor)`: the whole magnitude this body could
    /// spend this tick at full activation, from the world's own bill.
    pub u_full: f64,
    /// Seconds in the tick.
    pub dt: f64,
}

impl Envelope {
    /// `ω_attain = min(ω_max, u_full / r)`: the fastest pivot this body can actually perform
    /// this tick.
    ///
    /// A zero or non-finite radius has no sweep to pay for, so the genome's ceiling is the
    /// only limit; a zero `u_full` attains nothing. Both are finite encodings on purpose: the
    /// channel must never produce a NaN request.
    pub fn omega_attain(&self) -> f64 {
        let omega_max = non_negative(self.turn_rate_max);
        let u = non_negative(self.u_full);
        let r = non_negative(self.radius_px);
        if r <= 0.0 {
            omega_max
        } else {
            omega_max.min(u / r)
        }
    }

    /// `v_req = a₀ · v_max / wading`, px/s: the requested centre speed along the heading.
    pub fn requested_speed(&self, thrust: f64) -> f64 {
        let wading = if self.wading.is_finite() && self.wading > 0.0 {
            self.wading
        } else {
            1.0
        };
        non_negative(thrust) * non_negative(self.speed_max) / wading
    }

    /// `ω_req = a₁ · ω_attain`, rad/s, positive clockwise in the body frame.
    pub fn requested_omega(&self, turn: f64) -> f64 {
        let turn = if turn.is_finite() {
            turn.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        turn * self.omega_attain()
    }

    /// The magnitude the policy actually asked the envelope for this tick, px/s:
    /// `a₀ · v_max/wading + r · |ω_req|`.
    ///
    /// This is the contract's true requested magnitude. It is **not** `(a₀ + |a₁|)·u_full`:
    /// the angular ceiling binds whenever `ω_max < u_full/r` (which is the ordinary case at
    /// the R0d pace for a small body), and a low energy budget caps `u_full` below the
    /// translational capability so the two halves are scaled by different quantities.
    pub fn requested_magnitude(&self, action: &Action7) -> f64 {
        self.requested_speed(action.thrust())
            + non_negative(self.radius_px) * self.requested_omega(action.turn()).abs()
    }

    /// This tick's `MotorRequest`, built from the *current* transported heading.
    ///
    /// `MotorRequest` takes a target orientation, not a signed rate, so the adapter rotates
    /// the heading the body has **right now** by `ω_req · dt`. A held turn therefore keeps
    /// turning at the requested rate across both ticks of the controller interval and across a
    /// seam, rather than chasing a fixed bearing in a chart that no longer exists.
    pub fn request(&self, heading: Vec2, action: &Action7) -> crate::motor::MotorRequest {
        let current = heading.normalized().unwrap_or(heading);
        let omega = self.requested_omega(action.turn());
        let dt = if self.dt.is_finite() && self.dt > 0.0 {
            self.dt
        } else {
            0.0
        };
        let target = if omega == 0.0 || dt == 0.0 {
            current
        } else {
            rotate(current, omega * dt)
        };
        crate::motor::MotorRequest {
            heading: target,
            speed: self.requested_speed(action.thrust()),
        }
    }
}

/// Rotate a screen-space vector **clockwise** by `a` radians.
///
/// Screen `y` runs downward, so this is `(x·cos − y·sin, x·sin + y·cos)`: the turn the
/// contract calls positive, toward the body frame's `+y` (its clockwise side).
pub fn rotate(v: Vec2, a: f64) -> Vec2 {
    let (s, c) = a.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// The signed physical turn the world resolved, in the **body frame's** sign convention.
///
/// `ResolvedMotion.turn` is measured by `Vec2::screen_angle`, which increases
/// counter-clockwise on screen. The action channel and the sector numbering both run
/// clockwise, so the two differ by a sign and the adapter converts once, here, rather than
/// letting the mismatch reach the policy through the `turned` feedback channel.
pub fn resolved_turn(motion: &crate::motor::ResolvedMotion) -> f64 {
    -motion.turn
}

/// The resolved turn *rate* in the body frame's sign convention, rad/s.
pub fn resolved_omega(motion: &crate::motor::ResolvedMotion, dt: f64) -> f64 {
    -motion.omega(dt)
}

fn sigmoid(x: f64) -> f64 {
    if !x.is_finite() {
        return if x > 0.0 { 1.0 } else { 0.0 };
    }
    1.0 / (1.0 + (-x).exp())
}

fn tanh(x: f64) -> f64 {
    if !x.is_finite() {
        return if x > 0.0 { 1.0 } else { -1.0 };
    }
    x.tanh()
}

fn band(x: f64, width: f64) -> f64 {
    if x.abs() < width {
        0.0
    } else {
        x
    }
}

fn non_negative(x: f64) -> f64 {
    if x.is_finite() && x > 0.0 {
        x
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motor::{self, MotorLimits};

    const DT: f64 = 0.05;

    fn grazer() -> Capability {
        Capability::ordinary(1.0, 0.0, true, true)
    }

    /// A unit adult at the R0d pace with ample energy.
    fn adult() -> Envelope {
        Envelope {
            speed_max: 5.0,
            wading: 1.0,
            radius_px: 2.5,
            turn_rate_max: std::f64::consts::FRAC_PI_2,
            u_full: 5.0,
            dt: DT,
        }
    }

    fn limits(e: &Envelope, effort: f64) -> MotorLimits {
        MotorLimits {
            radius_px: e.radius_px,
            turn_rate_max: e.turn_rate_max,
            speed_cap: effort * e.speed_max / e.wading,
            motor_budget: e.u_full,
            dt: e.dt,
        }
    }

    fn held(a: [f64; ACT_LEN]) -> Action7 {
        Action7(a)
    }

    /// A NaN head is refused, never decoded into apparent effort; a finite head still works.
    #[test]
    fn the_voxel_squash_refuses_non_finite_heads() {
        for bad in [
            [f64::NAN, 0.0, 0.0],
            [0.0, f64::NAN, 0.0],
            [0.0, 0.0, f64::NAN],
            [f64::INFINITY, 0.0, 0.0],
        ] {
            let err = VoxelActionAdapter
                .squash(&bad)
                .expect_err("non-finite logits are an invalid controller");
            assert!(err.contains("not finite"), "{err}");
        }
        let a = VoxelActionAdapter
            .squash(&[0.4, 0.4, 0.4])
            .expect("finite head");
        assert!(a.0[VoxelActionAdapter::FORWARD] > 0.0);
        assert_eq!(a.0[VoxelActionAdapter::TURN], 0.4f64.tanh());
        assert!(a.0[VoxelActionAdapter::FEED] > 0.0);
    }

    #[test]
    fn the_deadband_gives_exact_stillness_and_no_bill_beyond_upkeep() {
        let a = Action7::squash(&[-10.0, 0.0, -10.0, -10.0, -10.0, -10.0, -10.0], &grazer());
        assert_eq!(a.0[THRUST], 0.0, "σ(−10) is inside the deadband");
        assert_eq!(a.0[TURN], 0.0, "tanh(0) is inside the deadband");
        assert!(!a.active());
        assert_eq!(a.effort(), 0.0);
        let e = adult();
        let motion = motor::resolve(
            Vec2::new(1.0, 0.0),
            &e.request(Vec2::new(1.0, 0.0), &a),
            &limits(&e, a.effort()),
        );
        assert_eq!(motion.speed, 0.0);
        assert_eq!(motion.turn, 0.0);
        assert_eq!(motion.sweep, 0.0);
        assert_eq!(e.requested_magnitude(&a), 0.0);
    }

    #[test]
    fn a_pure_pivot_with_zero_thrust_resolves_to_paid_rotation() {
        let e = adult();
        let a = held([0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!(a.active(), "turning alone activates the body");
        // ω_attain = min(90°/s, 5.0/2.5 = 2 rad/s) = 90°/s: at the R0d pace the genome binds.
        assert!((e.omega_attain() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        let motion = motor::resolve(
            Vec2::new(1.0, 0.0),
            &e.request(Vec2::new(1.0, 0.0), &a),
            &limits(&e, a.effort()),
        );
        assert_eq!(motion.speed, 0.0, "no thrust was requested");
        assert!(
            resolved_turn(&motion) > 0.0,
            "the body turned clockwise: {}",
            resolved_turn(&motion)
        );
        assert!(
            motion.sweep > 0.0,
            "and paid for the sweep: {}",
            motion.sweep
        );
        assert!(
            (resolved_omega(&motion, DT) - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "ω = {} rad/s",
            resolved_omega(&motion, DT)
        );
    }

    #[test]
    fn the_requested_magnitude_is_not_the_old_product_when_the_angular_ceiling_binds() {
        let e = adult();
        let a = held([1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        // Naive: (a₀ + |a₁|)·u_full = 10.0. True: 5.0 + 2.5·(π/2) = 8.927.
        let truth = 5.0 + 2.5 * std::f64::consts::FRAC_PI_2;
        assert!((e.requested_magnitude(&a) - truth).abs() < 1e-9);
        assert!(e.requested_magnitude(&a) < 2.0 * e.u_full);
        let motion = motor::resolve(
            Vec2::new(1.0, 0.0),
            &e.request(Vec2::new(1.0, 0.0), &a),
            &limits(&e, a.effort()),
        );
        assert!(
            motion.motor_magnitude() <= e.u_full + 1e-12,
            "the envelope still binds: {} > {}",
            motion.motor_magnitude(),
            e.u_full
        );
    }

    #[test]
    fn a_low_energy_budget_caps_the_turn_as_well_as_the_travel() {
        // The other regime: `u_full` below the translational capability, so `ω_attain` is the
        // budget's, not the genome's.
        let mut e = adult();
        e.u_full = 1.0;
        assert!((e.omega_attain() - 0.4).abs() < 1e-12, "1.0 / 2.5 rad/s");
        let a = held([0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!(
            (e.requested_magnitude(&a) - 1.0).abs() < 1e-12,
            "exactly the budget"
        );
        let motion = motor::resolve(
            Vec2::new(1.0, 0.0),
            &e.request(Vec2::new(1.0, 0.0), &a),
            &limits(&e, a.effort()),
        );
        assert!((motion.motor_magnitude() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn wading_throttles_the_requested_speed() {
        let mut e = adult();
        e.wading = 2.5;
        let a = held([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!((e.requested_speed(a.thrust()) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn a_zero_radius_or_zero_budget_still_gives_a_finite_turn_request() {
        let mut e = adult();
        e.radius_px = 0.0;
        assert_eq!(e.omega_attain(), std::f64::consts::FRAC_PI_2);
        e.radius_px = 2.5;
        e.u_full = 0.0;
        assert_eq!(e.omega_attain(), 0.0);
        e.turn_rate_max = f64::NAN;
        assert_eq!(e.omega_attain(), 0.0);
        let a = held([0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!(e.requested_omega(a.turn()).is_finite());
    }

    #[test]
    fn a_held_turn_keeps_turning_by_the_same_signed_amount_every_tick() {
        let e = adult();
        let a = held([0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let mut heading = Vec2::new(1.0, 0.0);
        let mut turns = Vec::new();
        for _ in 0..4 {
            let motion = motor::resolve(heading, &e.request(heading, &a), &limits(&e, a.effort()));
            turns.push(resolved_turn(&motion));
            heading = motion.heading;
        }
        for t in &turns {
            assert!(
                (t - turns[0]).abs() < 1e-12,
                "a held rate turns by the same amount each tick: {turns:?}"
            );
        }
        assert!(turns[0] > 0.0, "positive `a₁` is a clockwise turn");
    }

    #[test]
    fn the_mouth_is_shared_and_never_exceeds_one_mouth_tick() {
        let a = Action7::squash(&[0.0, 0.0, 4.0, 4.0, 4.0, 0.0, 0.0], &grazer());
        let sum: f64 = MOUTH_CHANNELS.iter().map(|i| a.0[*i]).sum();
        assert!(
            (sum - 1.0).abs() < 1e-12,
            "three saturated mouths sum to {sum}"
        );
    }

    #[test]
    fn masked_channels_never_reach_the_decision() {
        // A pure scavenger: below the grazing gate, above the fruit gate's complement.
        let scavenger = Capability::ordinary(0.0, 1.0, true, true);
        let a = Action7::squash(&[0.0, 0.0, 6.0, 6.0, 6.0, 6.0, 6.0], &scavenger);
        assert_eq!(a.0[GRAZE], 0.0);
        assert_eq!(a.0[FRUIT], 0.0);
        assert!(a.0[SCAVENGE] > 0.0);
        assert_eq!(a.0[ATTACK], 0.0, "attack is masked for every ordinary body");
        assert!(a.reproduce());

        // A pure grazer cannot scavenge.
        let grazer = Capability::ordinary(1.0, 0.0, true, true);
        let b = Action7::squash(&[0.0, 0.0, 6.0, 6.0, 6.0, 6.0, 6.0], &grazer);
        assert_eq!(b.0[SCAVENGE], 0.0);
        assert!(b.0[GRAZE] > 0.0 && b.0[FRUIT] > 0.0);

        // A disabled world mechanism masks the mouth too.
        let off = Capability::ordinary(1.0, 1.0, false, false);
        let c = Action7::squash(&[0.0, 0.0, 6.0, 6.0, 6.0, 6.0, 6.0], &off);
        assert_eq!([c.0[GRAZE], c.0[FRUIT], c.0[SCAVENGE]], [0.0, 0.0, 0.0]);
    }

    #[test]
    fn the_attack_and_reproduce_channels_are_levels_not_pulses() {
        let cap = grazer();
        let low = Action7::squash(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -0.5], &cap);
        assert!(!low.reproduce(), "σ(−0.5) = 0.378 is below the level");
        let high = Action7::squash(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5], &cap);
        assert!(high.reproduce(), "σ(0.5) = 0.622 is above it");
    }

    /// The contract's §4 table, at R0b's persisted pace, straight through `motor::resolve`.
    #[test]
    fn the_worked_examples_hold_at_the_old_persisted_pace() {
        let e = Envelope {
            speed_max: 0.3,
            wading: 1.0,
            radius_px: 2.5,
            turn_rate_max: std::f64::consts::FRAC_PI_2,
            u_full: 0.30,
            dt: DT,
        };
        // At 0.3 px/s the *budget* binds instead of the genome: ω_attain = 0.30/2.5 = 0.12.
        assert!((e.omega_attain() - 0.12).abs() < 1e-12);

        let cases: [([f64; ACT_LEN], f64, f64); 5] = [
            ([0.0; ACT_LEN], 0.0, 0.0),
            ([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 0.30, 0.0),
            ([0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 0.0, 0.12),
            ([0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0], 0.15, 0.06),
            ([1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 0.15, 0.06),
        ];
        for (raw, want_v, want_omega) in cases {
            let a = held(raw);
            let motion = motor::resolve(
                Vec2::new(1.0, 0.0),
                &e.request(Vec2::new(1.0, 0.0), &a),
                &limits(&e, a.effort()),
            );
            assert!(
                (motion.speed - want_v).abs() < 1e-9,
                "{raw:?}: v = {} not {want_v}",
                motion.speed
            );
            assert!(
                (resolved_omega(&motion, DT) - want_omega).abs() < 1e-9,
                "{raw:?}: ω = {} not {want_omega}",
                resolved_omega(&motion, DT)
            );
        }

        // The two energy-capped rows.
        let mut poor = e;
        poor.u_full = 0.10;
        for (raw, want_v, want_omega) in [
            ([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 0.10, 0.0),
            ([0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 0.0, 0.04),
        ] {
            let a = held(raw);
            let motion = motor::resolve(
                Vec2::new(1.0, 0.0),
                &poor.request(Vec2::new(1.0, 0.0), &a),
                &limits(&poor, a.effort()),
            );
            assert!(
                (motion.speed - want_v).abs() < 1e-9,
                "{raw:?}: v = {}",
                motion.speed
            );
            assert!(
                (resolved_omega(&motion, DT) - want_omega).abs() < 1e-9,
                "{raw:?}: ω = {}",
                resolved_omega(&motion, DT)
            );
        }

        // Energy below upkeep: still, and unpaid.
        let mut broke = e;
        broke.u_full = 0.0;
        let a = held([0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let motion = motor::resolve(
            Vec2::new(1.0, 0.0),
            &broke.request(Vec2::new(1.0, 0.0), &a),
            &MotorLimits {
                motor_budget: 0.0,
                ..limits(&broke, a.effort())
            },
        );
        assert_eq!(motion.speed, 0.0);
        assert_eq!(motion.turn, 0.0);
    }
}
