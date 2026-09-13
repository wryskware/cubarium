//! The adapter from the core's hunter observer to the Lanternjaw's living pose.
//!
//! Pure renderer-side memory per hunter, keyed by full generation-bearing [`OrganismId`],
//! built from [`HunterView`] and nothing else: no `genome.form`, no guessed durations, no
//! second world. It turns the persisted phase boundaries (`phase_started_tick`,
//! `phase_ends_tick`, `entered_from`, `episode`) and the presenter's own fractional clock
//! ([`present_seconds`]) into the [`AttackEpisode`] the rig draws, keeps the previous tick's
//! phase through the one-tick interpolation interval so a strike is still fully extended at
//! its settlement boundary, and reconstructs the displayed reach a phase was entered from —
//! from the previous frame when one was observed, and conservatively from `entered_from`
//! after a restart. Contract: `design/7_Research/lanternjaw-ecology-animation-contract-2026-09-13.md`.

use cubarium_core::hunter::{FixedHunterProfile, HunterPhase, HunterRole, HunterView};
use cubarium_core::view::OrganismView;
use cubarium_surface::{MAX_LOCAL_RADIUS, PathSegment, SurfacePoint, Vec2, travel, unfold};

use crate::art_present::present_seconds;
use crate::clock::DT;
use crate::lanternjaw::{
    AttackEpisode, AttackPhase, LivingPose, RECOIL_SECONDS, Reach, SCALE_MAX, SCALE_MIN,
    attack_channels, effectors,
};

/// Tolerance, in body pixels, within which a profile's contact geometry must equal the art's
/// named effectors for the rig to be allowed to draw that profile.
pub const GEOMETRY_TOLERANCE: f64 = 1e-6;

/// Whether the Lanternjaw art can draw hunters of this profile: the renderer stating its
/// capability **before** a world with this profile is stepped or drawn, so a saved world
/// whose profile the art cannot honour fails by name at load, and nothing is clamped or
/// substituted later.
///
/// **Normative.** `Ok` exactly when: the role is `Lanternjaw`; `capture_offset_body` equals
/// [`effectors`]`(1).near_claw` and `ingestion_offset_body` equals `effectors(1).mouth`, each
/// component within [`GEOMETRY_TOLERANCE`] (the core's contact geometry is *the art's* claw and
/// mouth, scaled — a profile that tests contact somewhere else would draw claws where the
/// world does not bite); `body_scale_min` is finite and within [`SCALE_MIN`]`..=`[`SCALE_MAX`]
/// and `body_scale_exponent` is finite and non-negative, so every `body_scale = max(min,
/// (S / S_adult)^exponent)` of a member with `S ≤ S_adult` lies in the admitted range. The
/// error names the offending field and both values. A world without a hunter profile needs
/// no capability and is never asked.
pub fn validate_profile(profile: &FixedHunterProfile) -> Result<(), String> {
    if profile.role != HunterRole::Lanternjaw {
        return Err(format!(
            "hunter role {:?} has no art; only Lanternjaw is drawn",
            profile.role
        ));
    }
    let adult = effectors(1.0);
    let close = |a: Vec2, b: Vec2| {
        (a.x - b.x).abs() <= GEOMETRY_TOLERANCE && (a.y - b.y).abs() <= GEOMETRY_TOLERANCE
    };
    if !close(profile.capture_offset_body, adult.near_claw) {
        return Err(format!(
            "capture_offset_body ({}, {}) is not the Lanternjaw's near claw ({}, {})",
            profile.capture_offset_body.x,
            profile.capture_offset_body.y,
            adult.near_claw.x,
            adult.near_claw.y
        ));
    }
    if !close(profile.ingestion_offset_body, adult.mouth) {
        return Err(format!(
            "ingestion_offset_body ({}, {}) is not the Lanternjaw's mouth ({}, {})",
            profile.ingestion_offset_body.x,
            profile.ingestion_offset_body.y,
            adult.mouth.x,
            adult.mouth.y
        ));
    }
    if !hunter_scale_supported(profile.body_scale_min) {
        return Err(format!(
            "body_scale_min {} is outside the art's admitted {SCALE_MIN}..={SCALE_MAX}",
            profile.body_scale_min
        ));
    }
    if !(profile.body_scale_exponent.is_finite() && profile.body_scale_exponent >= 0.0) {
        return Err(format!(
            "body_scale_exponent {} could scale a body past the adult",
            profile.body_scale_exponent
        ));
    }
    Ok(())
}

/// Whether one member's published geometry is the art's at its own scale: the same test as
/// [`validate_profile`], applied to the scaled `ContactGeometry` a view carries.
pub fn validate_view(view: &HunterView) -> Result<(), String> {
    if !hunter_scale_supported(view.body_scale) {
        return Err(format!(
            "hunter {:?}: body scale {} is outside the art's admitted {SCALE_MIN}..={SCALE_MAX}",
            view.id, view.body_scale
        ));
    }
    let e = effectors(view.body_scale);
    let tolerance = GEOMETRY_TOLERANCE * view.body_scale.max(1.0);
    let close = |a: Vec2, b: Vec2| (a.x - b.x).abs() <= tolerance && (a.y - b.y).abs() <= tolerance;
    if !close(view.geometry.capture_offset_body, e.near_claw) {
        return Err(format!(
            "hunter {:?}: capture offset ({}, {}) is not the drawn near claw ({}, {}) at scale {}",
            view.id,
            view.geometry.capture_offset_body.x,
            view.geometry.capture_offset_body.y,
            e.near_claw.x,
            e.near_claw.y,
            view.body_scale
        ));
    }
    if !close(view.geometry.ingestion_offset_body, e.mouth) {
        return Err(format!(
            "hunter {:?}: ingestion offset ({}, {}) is not the drawn mouth ({}, {}) at scale {}",
            view.id,
            view.geometry.ingestion_offset_body.x,
            view.geometry.ingestion_offset_body.y,
            e.mouth.x,
            e.mouth.y,
            view.body_scale
        ));
    }
    Ok(())
}

/// The root speed, in chart pixels per second, at which the hunter's ambient body is in full
/// locomotion (`LivingPose::movement = 1`): the trial profile's adult maximum, about
/// 0.2523 px/s. A strike burst (up to 1 px/s) saturates it. Review-tunable presentation;
/// it does not read the profile so a profile change cannot move the legs by itself.
pub const HUNTER_FULL_SPEED_PX_S: f64 = 0.25;

/// The cocked reach a `Windup` leaves behind: the study's coil, fully wound.
pub const COCKED: Reach = Reach {
    near: -0.35,
    far: -0.35,
    compress: 1.7,
    lunge: 0.0,
    charge: 1.0,
};

/// Whether the rig can draw a hunter at this authoritative `body_scale`. **Normative**: finite
/// and within [`SCALE_MIN`]`..=`[`SCALE_MAX`]. The runner checks the profile's supported
/// range with [`validate_profile`] before stepping; [`validate_view`] rejects an unsupported
/// published member with a named error. No scale is clamped and no ordinary rig is substituted.
pub fn hunter_scale_supported(scale: f64) -> bool {
    scale.is_finite() && (SCALE_MIN..=SCALE_MAX).contains(&scale)
}

/// What one tick's [`HunterView`] says about a member, as the adapter keeps it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HunterFrame {
    /// The view's tick: the completed-tick boundary this frame describes.
    pub tick: u64,
    pub phase: HunterPhase,
    /// The completed-tick boundary the phase was entered at.
    pub started: u64,
    /// The boundary a timed phase ends at (`== started` for an untimed one).
    pub ends: u64,
    pub entered_from: HunterPhase,
    pub episode: u64,
    /// The authoritative whole-rig scale.
    pub scale: f64,
    /// `gut_material / gut_capacity`.
    pub gut: f32,
    /// Gestation progress of a funded escrow, or `None`.
    pub gestation: Option<f32>,
    pub target: Option<cubarium_core::OrganismId>,
}

impl HunterFrame {
    pub fn of(view: &HunterView, tick: u64) -> HunterFrame {
        HunterFrame {
            tick,
            phase: view.phase,
            started: view.phase_started_tick,
            ends: view.phase_ends_tick,
            entered_from: view.entered_from,
            episode: view.episode,
            scale: view.body_scale,
            gut: view.gut_fraction,
            gestation: view.gestation,
            target: view.target,
        }
    }

    /// The identity of the phase this frame is in: a new phase means a new key.
    pub fn key(&self) -> (HunterPhase, u64, u64) {
        (self.phase, self.started, self.episode)
    }
}

/// The rig phase a core phase drives; `None` for the phase-free `Perched` and `Stalking`.
pub fn attack_phase_of(phase: HunterPhase) -> Option<AttackPhase> {
    match phase {
        HunterPhase::Perched | HunterPhase::Stalking => None,
        HunterPhase::Windup => Some(AttackPhase::Windup),
        HunterPhase::Strike => Some(AttackPhase::Strike),
        HunterPhase::Recovering => Some(AttackPhase::Recovering),
        HunterPhase::Handling => Some(AttackPhase::Handling),
    }
}

/// The reach a complete strike leaves at its settlement: the schedule's own end state for a
/// strike entered fully cocked — full extension, with the chain's charge already bled for
/// the extension's 120 ms and the far claw 45 ms behind. **Normative**:
/// `attack_channels(Strike { elapsed = duration = 1, from: COCKED }).reach()`.
pub fn settled_reach() -> Reach {
    attack_channels(Some(&AttackEpisode {
        phase: AttackPhase::Strike,
        elapsed: 1.0,
        duration: 1.0,
        from: COCKED,
    }))
    .reach()
}

/// The reach a phase was entered from when no previous frame was observed (a viewer joining
/// mid-hunt, a restart from a snapshot): a conservative reconstruction from the persisted
/// `entered_from`, never an invented attack. **Normative**: entered from `Strike` ⇒
/// [`settled_reach`] (a strike always runs to its settlement); from `Windup` ⇒ [`COCKED`];
/// anything else ⇒ [`Reach::FOLDED`]. A windup interrupted part-way, or one whose far claw
/// had not quite caught up, is reconstructed as fully cocked: the difference is a fraction
/// of a pixel of limb reach for the recoil's first 200 ms and no attack is invented.
pub fn entry_reach(entered_from: HunterPhase) -> Reach {
    match entered_from {
        HunterPhase::Strike => settled_reach(),
        HunterPhase::Windup => COCKED,
        _ => Reach::FOLDED,
    }
}

/// The attack episode a frame drives at presentation `seconds`, entered from `from`.
///
/// **Normative**: `elapsed = seconds − started · DT` (attack time from the persisted entry
/// boundary, never a modulo of anything), `duration = (ends − started) · DT` for a timed
/// phase and at least [`RECOIL_SECONDS`] for `Recovering`/`Handling`, whose own constants
/// are what matter. `None` for a phase-free frame.
pub fn episode_of(frame: &HunterFrame, from: Reach, seconds: f64) -> Option<AttackEpisode> {
    let phase = attack_phase_of(frame.phase)?;
    let elapsed = seconds - frame.started as f64 * DT;
    let timed = frame.ends.saturating_sub(frame.started) as f64 * DT;
    let duration = match phase {
        AttackPhase::Windup | AttackPhase::Strike => timed,
        AttackPhase::Recovering | AttackPhase::Handling => timed.max(RECOIL_SECONDS),
    };
    Some(AttackEpisode {
        phase,
        elapsed,
        duration,
        from,
    })
}

/// The reach a frame displays at presentation `seconds`.
pub fn reach_at(frame: &HunterFrame, from: Reach, seconds: f64) -> Reach {
    attack_channels(episode_of(frame, from, seconds).as_ref()).reach()
}

/// `LivingPose::movement` for a root that travelled `moved` during the last tick: the tick's
/// path length over `DT`, as a fraction of [`HUNTER_FULL_SPEED_PX_S`], clamped to `[0, 1]`.
pub fn movement_of(moved: &[PathSegment]) -> f64 {
    let length: f64 = moved.iter().map(PathSegment::length).sum();
    if !length.is_finite() {
        return 0.0;
    }
    (length / DT / HUNTER_FULL_SPEED_PX_S).clamp(0.0, 1.0)
}

/// The adapter's memory of one hunter across ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct HunterMemory {
    /// This tick's frame.
    pub cur: HunterFrame,
    /// The previous tick's frame, kept for exactly one interpolation interval.
    pub prev: Option<HunterFrame>,
    /// The reach displayed when `cur`'s phase was entered.
    pub from: Reach,
    /// The reach displayed when `prev`'s phase was entered.
    pub prev_from: Reach,
    /// The last view of the hunter's target, kept so that a prey the world removed at this
    /// tick's boundary is still drawn for the frames before that boundary. `Some` only on
    /// the tick the target left the view while the hunter entered `Handling`; cleared the
    /// tick after. Bounded: one per hunter.
    pub prey: Option<OrganismView>,
    /// Where the world says the retained prey was at settlement (the `Capture` event's
    /// post-movement `prey_pos`), when the event was seen; `None` otherwise.
    pub prey_at: Option<SurfacePoint>,
    /// The target's view as of the last tick, so the tick it disappears still has it.
    pub target_view: Option<OrganismView>,
}

impl HunterMemory {
    /// A hunter first seen in `frame`: its entry reach is reconstructed from `entered_from`.
    pub fn enter(frame: HunterFrame) -> HunterMemory {
        let from = entry_reach(frame.entered_from);
        HunterMemory {
            cur: frame,
            prev: None,
            from,
            prev_from: from,
            prey: None,
            prey_at: None,
            target_view: None,
        }
    }

    /// Record the next tick's frame. **Normative**: the current frame becomes `prev`; if the
    /// phase key changed, the new `from` is the reach the previous phase displayed at the new
    /// phase's entry boundary (`reach_at(prev, prev_from, started · DT)`), so every phase
    /// continues from the picture the last one left — an interrupted windup recoils from its
    /// partial cock, a settled strike from full extension. **Idempotent**: a frame of the tick
    /// already recorded changes nothing (the same completed view observed twice is one
    /// observation), and a frame of an *earlier* tick starts the memory over as if first seen
    /// (a rewind never carries a later phase's reach back in time).
    pub fn observe(&mut self, frame: HunterFrame) {
        if frame.tick == self.cur.tick {
            return;
        }
        if frame.tick < self.cur.tick {
            *self = HunterMemory::enter(frame);
            return;
        }
        let previous = self.cur;
        let previous_from = self.from;
        if frame.key() != previous.key() {
            self.from = reach_at(&previous, previous_from, frame.started as f64 * DT);
        }
        self.prev = Some(previous);
        self.prev_from = previous_from;
        self.cur = frame;
    }

    /// Record where the world says the retained prey was taken: the `Capture` event's
    /// post-movement position. Ignored unless it names the retained prey.
    pub fn note_capture(&mut self, prey: cubarium_core::OrganismId, at: SurfacePoint) {
        if self.prey.as_ref().is_some_and(|p| p.id == prey) {
            self.prey_at = Some(at);
        }
    }

    /// Where the retained prey is drawn at fraction `f` of the capture tick, and with which
    /// heading.
    ///
    /// **Normative.** With `p` its last published view and `q` the settlement position from
    /// the `Capture` event: no `q` ⇒ `(p.pos, p.heading)` throughout. Otherwise the prey
    /// walks the **shortest valid surface chord** from `p.pos` to `q`: `u = unfold(p.pos, q,
    /// MAX_LOCAL_RADIUS)` gives `q`'s image in `p`'s chart, `d = u.local − p.pos.chart()`, and
    /// the pose at `f` is `t = travel(p.pos, f · d)` — `t.end` on whichever chart the point
    /// falls, crossing seams exactly as a moving body does (chart transport, never a
    /// reflection: a valid unfolding never crosses the open rim), with the heading
    /// `t.map.apply(p.heading)`, the last published heading carried through the seams the
    /// chord crosses. The endpoints are exact: `f = 0` is `p.pos` with `p.heading`, `f = 1`
    /// is `q` itself with the heading carried the whole way. This is an **approximation**
    /// and is labelled one: the event carries neither the capture-tick path nor a final
    /// heading, so a straight surface chord and the old heading stand in for both. Fallback,
    /// also documented: when no valid unfolding exists within `MAX_LOCAL_RADIUS`, or the
    /// sweep reflects or falls back (neither can happen for a chord the unfolding validated),
    /// the prey is drawn at `q` with `p.heading` for the whole interval. `None` without a
    /// retained prey.
    pub fn retained_prey_pose(&self, f: f64) -> Option<(SurfacePoint, Vec2)> {
        let p = self.prey.as_ref()?;
        let f = if f.is_finite() {
            f.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let Some(q) = self.prey_at else {
            return Some((p.pos, p.heading));
        };
        let Some(u) = unfold(p.pos, q, MAX_LOCAL_RADIUS) else {
            return Some((q, p.heading));
        };
        if f <= 0.0 {
            return Some((p.pos, p.heading));
        }
        if f >= 1.0 {
            // The recorded endpoint owns its tangent chart. A sweep ending exactly on
            // its edge can cross onward; that next chart's heading does not belong to q.
            return Some((q, u.map.inverse().apply(p.heading)));
        }
        let d = u.local - p.pos.chart();
        let t = travel(p.pos, d * f);
        if t.reflections > 0 || t.fallback {
            return Some((q, p.heading));
        }
        let heading = t.map.apply(p.heading);
        Some((t.end, heading))
    }

    /// The frame and entry reach in effect at fraction `f` of `tick`'s presentation interval.
    /// **Normative**: while the current phase was entered exactly at `tick`'s boundary and
    /// `f < 1`, the previous frame is still in effect (the presentation instant lies before
    /// that boundary); at `f = 1`, or for a phase entered earlier, the current one.
    pub fn frame_at(&self, tick: u64, f: f64) -> (&HunterFrame, Reach) {
        match &self.prev {
            Some(prev) if self.cur.started == tick && self.cur.key() != prev.key() && f < 1.0 => {
                (prev, self.prev_from)
            }
            _ => (&self.cur, self.from),
        }
    }

    /// The body state — gut fraction, gestation, whole-rig scale — in effect at fraction `f`
    /// of `tick`'s interval, between the previous published frame and the current one.
    ///
    /// **Normative.** With `p = prev` (when it exists and is an earlier tick) and `c = cur`,
    /// and `f` clamped to `[0, 1]` (NaN ⇒ 0):
    ///
    /// * **scale** is continuous biology and interpolates linearly, `p.scale + (c.scale −
    ///   p.scale) · f`, exactly `p.scale` at 0 and `c.scale` at 1;
    /// * **gut** is a tick-boundary fact when the phase changed at this boundary (a capture
    ///   fills it, a finished meal empties it): while `c.started == tick` and `c.key() ≠
    ///   p.key()` it is `p.gut` for `f < 1` and `c.gut` at `f = 1`; otherwise (digestion
    ///   inside one phase) it interpolates linearly;
    /// * **gestation** interpolates linearly while both frames carry one; a start or an end
    ///   (`None ↔ Some`) is a boundary fact, `p.gestation` for `f < 1` and `c.gestation` at 1.
    ///
    /// Without a previous frame the current values hold throughout. Nothing here reads the
    /// attack phase's own timing; the `Capture` that fills the gut therefore shows on the
    /// abdomen exactly at the settlement boundary the strike is held to, not one interval early.
    pub fn state_at(&self, tick: u64, f: f64) -> (f64, Option<f64>, f64) {
        let f = if f.is_finite() {
            f.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let c = &self.cur;
        let Some(p) = self.prev.as_ref().filter(|p| p.tick < c.tick) else {
            return (f64::from(c.gut), c.gestation.map(f64::from), c.scale);
        };
        let lerp = |a: f64, b: f64| if f >= 1.0 { b } else { a + (b - a) * f };
        let boundary_event = c.started == tick && c.key() != p.key();
        let gut = if boundary_event {
            if f >= 1.0 {
                f64::from(c.gut)
            } else {
                f64::from(p.gut)
            }
        } else {
            lerp(f64::from(p.gut), f64::from(c.gut))
        };
        let cocoon = match (p.gestation, c.gestation) {
            (Some(a), Some(b)) => Some(lerp(f64::from(a), f64::from(b))),
            (a, b) => {
                if f >= 1.0 {
                    b.map(f64::from)
                } else {
                    a.map(f64::from)
                }
            }
        };
        (gut, cocoon, lerp(p.scale, c.scale))
    }

    /// The living pose and whole-rig scale at fraction `f` of `tick`, for a root that
    /// travelled `moved` during the last tick.
    ///
    /// **Normative**: `ambient = present_seconds(tick, f)`, `movement = movement_of(moved)`,
    /// `attack = episode_of(frame_at(tick, f))` at that same instant, and `(gut, cocoon,
    /// scale) = state_at(tick, f)` — the published body state interpolated across the same
    /// interval the root is, with boundary events (a meal taken, a meal finished, an escrow
    /// begun or ended) stepping exactly at the boundary. Pure: the same inputs give the same
    /// pose at any frame rate, and nothing here advances anything.
    pub fn living_pose(&self, tick: u64, f: f64, moved: &[PathSegment]) -> (LivingPose, f64) {
        let seconds = present_seconds(tick, f);
        let (frame, from) = self.frame_at(tick, f);
        let (gut, cocoon, scale) = self.state_at(tick, f);
        let pose = LivingPose {
            ambient: seconds,
            movement: movement_of(moved),
            attack: episode_of(frame, from, seconds),
            gut,
            cocoon,
        };
        (pose, scale)
    }
}
