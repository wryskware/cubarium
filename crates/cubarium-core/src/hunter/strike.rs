//! The **per-attempt strike record**: what one paid attempt looked like at the three instants
//! that decide it, and the four-way reading of why it ended where it did.
//!
//! [`crate::hunter::HunterEvent::Attempt`] already says *what* an attempt resolved to and,
//! through [`ContactEvidence`], where everything stood **at settlement**. It cannot say what
//! the attempt was aimed at: by the time the outcome is known both bodies have moved twice,
//! and the geometry the hunter actually committed to — at the start of the windup gesture, and
//! again at the start of the paid burst — is gone. The apex eligibility audit
//! (`design/7_Research/ecology-v1-apex-eligibility-2026-09-16.md`) localised an introduced
//! apex's death to intake and intake to reach — 402 of 449 paid attempts ended
//! [`AttemptOutcome::OutOfReach`] — and left exactly that question open. This module is that
//! question's measurement (`design/handoffs/ecology-v1-apex-reach-opus-2026-09-16.md`,
//! deliverable 1).
//!
//! **Three instants, one attempt.** A [`StrikeRecord`] carries one [`StrikeFrame`] per instant:
//!
//! | frame | when | what has happened since the previous frame |
//! | --- | --- | --- |
//! | `intent` | the boundary the member entered [`HunterPhase::Windup`] on | — |
//! | `strike` | the boundary it entered [`HunterPhase::Strike`] on | `windup_seconds` of the hunter holding and the prey fleeing |
//! | `resolution` | the boundary the settlement pass resolved on | `strike_seconds` of the paid burst |
//!
//! Each frame is gathered from the **same** [`ContactEvidence::gather`] the settlement itself
//! uses, so `intent` and `strike` are measured by the authority that decides `resolution` and
//! not by a second, looser geometry. The frames' ticks are exactly
//! `windup_seconds / DT` and `strike_seconds / DT` apart, so a realised speed is a
//! displacement divided by a duration this module knows rather than one it infers.
//!
//! **Inert, opt-in, transient.** Recording is off by default and is turned on per `World` by
//! [`crate::World::record_strike_attempts`]. Every site is one `bool` test when it is off.
//! Gathering a frame is pure: it reads two bodies and the surface charts, consumes no draw,
//! writes nothing the tick reads back, and appears in no snapshot and no state hash — the same
//! contract [`crate::BodyBudget`] holds itself to, and it is measured the same way, by running
//! a two-apex world 9,000 ticks with the recorder on and off and comparing the hashes
//! (`crates/cubarium-core/tests/hunter_strike_record.rs`).
//!
//! **This module changes no constant.** It names them: [`StrikeFrame::advertised_reach`] is
//! `(|capture_offset_body| + capture_reach_px) · scale` and [`StrikeFrame::tolerance`] is
//! `capture_reach_px · scale + prey_extent`, both read from the profile the world is running.

use serde::{Deserialize, Serialize};

use cubarium_surface::{ChartImage, SurfacePoint, Vec2};

use crate::ids::OrganismId;
use crate::organism::Organism;

use super::{AttemptOutcome, ContactEvidence, FixedHunterProfile};

/// How many closed records an undrained recorder keeps before it starts counting drops.
///
/// The death run this module was written for made 449 paid attempts over 16 lives and
/// 180,000 ticks, and the audit drains every tick. A recorder nobody drains is still bounded,
/// like [`crate::world::budget::MAX_CLOSED_RECORDS`], and never silently loses a record.
pub const MAX_STRIKE_RECORDS: usize = 16_384;

/// How many attempts may be open at once before the oldest is dropped. One per living member;
/// a member that dies mid-windup leaves one behind, so the cap is generous rather than tight.
pub const MAX_OPEN_ATTEMPTS: usize = 256;

/// Everything one instant of one attempt looked like, from the hunter's own contact authority.
///
/// A frame with no target — the handle went stale, or the prey was already claimed and removed
/// by an earlier settlement in the same tick — still carries the hunter's own state, so a
/// record is never silently missing its hunter side.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrikeFrame {
    /// The completed-tick boundary this frame was read at.
    pub tick: u64,
    pub hunter_pos: SurfacePoint,
    pub hunter_heading: Vec2,
    /// The body scale the geometry below was built at: `max(min, (S / S_adult)^exponent)`.
    pub scale: f64,
    /// `(|capture_offset_body| + capture_reach_px) · scale`: how far from its own root this
    /// member's grasp closes. The reach the profile advertises, at this member's size.
    pub advertised_reach: f64,
    /// `capture_offset_body.x · scale`: how far **forward** of the root the grasp centre sits.
    /// Recorded separately from the reach because the pursuit's own stopping rule is written
    /// against this coordinate and not against the distance
    /// (`crate::world::step`, "the pursuit stopping distance").
    pub capture_forward: f64,
    /// The target this attempt was aimed at, by full ID, at this instant.
    pub target: Option<OrganismId>,
    pub target_pos: Option<SurfacePoint>,
    pub target_heading: Option<Vec2>,
    pub target_extent: Option<f64>,
    /// Surface distance from the hunter **root** to the prey, through the shortest valid
    /// unfolding. `None` when the prey is gone or is not reachable inside the local window.
    pub root_distance: Option<f64>,
    /// Surface distance from the **scaled grasp centre** to the prey: the quantity the capture
    /// test compares, and the one "separation" means everywhere in this module.
    pub effector_distance: Option<f64>,
    /// `capture_reach_px · scale + prey_extent`: what `effector_distance` must not exceed.
    pub tolerance: Option<f64>,
    /// `effector_distance <= tolerance`: the contact test, evaluated at this instant. This is
    /// [`crate::hunter::ContactMeasure::in_contact`] and nothing looser.
    pub in_reach: bool,
    /// The prey's position **in the hunter's own body basis**, unfolded from the root: `+x`
    /// along the heading, `+y` its clockwise side. This is the coordinate the contact test and
    /// the pursuit stopping rule are both written in, so a record can say which of them a
    /// given instant satisfied without re-deriving either.
    pub body_forward: Option<f64>,
    pub body_side: Option<f64>,
    /// Whether a grasp centre exists on the surface here at all, i.e. whether a capture at
    /// this instant could have been drawn where the artwork puts it.
    pub grasp_mapped: bool,
}

impl StrikeFrame {
    /// Read one instant. Pure: no draws, no mutation, no world access beyond the two bodies
    /// and the charts, and the same [`ContactEvidence::gather`] the settlement uses.
    pub fn gather(
        images: &[Vec<ChartImage>; 5],
        profile: &FixedHunterProfile,
        tick: u64,
        hunter: &Organism,
        target: Option<(OrganismId, &Organism)>,
    ) -> StrikeFrame {
        let geometry = super::ContactGeometry::of(profile, hunter);
        let advertised_reach = geometry.capture_offset_body.length() + geometry.capture_reach_px;
        let mut frame = StrikeFrame {
            tick,
            hunter_pos: hunter.pos,
            hunter_heading: hunter.heading,
            scale: geometry.scale,
            advertised_reach,
            capture_forward: geometry.capture_offset_body.x,
            target: target.map(|(id, _)| id),
            target_pos: None,
            target_heading: None,
            target_extent: None,
            root_distance: None,
            effector_distance: None,
            tolerance: None,
            in_reach: false,
            body_forward: None,
            body_side: None,
            grasp_mapped: false,
        };
        let Some((id, prey)) = target else {
            return frame;
        };
        let evidence = ContactEvidence::gather(images, profile, hunter, id, prey);
        frame.target_pos = Some(evidence.prey_pos);
        frame.target_heading = Some(prey.heading);
        frame.target_extent = Some(evidence.prey_extent);
        frame.grasp_mapped = evidence.capture_center.is_some();
        if let Some(m) = evidence.measure {
            frame.root_distance = Some(m.root_distance);
            frame.effector_distance = Some(m.effector_distance);
            frame.tolerance = Some(m.tolerance);
            frame.in_reach = m.in_contact();
            frame.body_forward = Some(m.body.x);
            frame.body_side = Some(m.body.y);
        }
        frame
    }

    /// Whether the pursuit's own stopping rule held this member still at this instant, under
    /// `stop`. A member for which this is true asks for its resting effort and is pushed **no
    /// strike burst**, whatever phase it is in.
    ///
    /// This does not re-derive either rule: it rebuilds the [`super::ContactMeasure`] this
    /// frame recorded and asks [`super::ContactMeasure::pursuit_holds`], the one place both
    /// rules are written and the same call `crate::world::step`'s hunt-intent pass makes. Read
    /// it with [`StrikeRecord::stop`] and it says what the world actually did; read it with the
    /// other rule and it says what the other rule would have done at the same instant.
    pub fn pursuit_holds(&self, stop: super::PursuitStop) -> Option<bool> {
        let measure = super::ContactMeasure {
            body: Vec2::new(self.body_forward?, self.body_side?),
            root_distance: self.root_distance?,
            effector_distance: self.effector_distance?,
            tolerance: self.tolerance?,
        };
        Some(measure.pursuit_holds(stop, self.capture_forward))
    }

    /// How far past the grasp the prey stood: `effector_distance − tolerance`. Positive is out
    /// of reach, and is the number a reach verdict is about.
    pub fn overshoot(&self) -> Option<f64> {
        Some(self.effector_distance? - self.tolerance?)
    }

    fn face(pos: Option<SurfacePoint>) -> Option<usize> {
        pos.map(|p| p.face.index())
    }
}

/// Which of the four mechanisms an attempt's own geometry implicates, evaluated in a fixed
/// priority so every attempt gets exactly one class and the priority is auditable.
///
/// The thresholds are this workstream's, stated rather than tuned: `in_reach` is the contact
/// test itself (`effector_distance <= tolerance`), and "the separation grew" is a strict
/// increase in `effector_distance` between the strike frame and the resolution frame. There is
/// no epsilon: a tie counts as "did not grow", which is the conservative reading for the
/// escape hypothesis.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrikeClass {
    /// The target's identity changed, it died, or its handle went stale before the attempt
    /// resolved. There is no resolution geometry to read, so no other class can be decided.
    TargetLost,
    /// The strike arrived: the prey was inside the grasp at resolution. Whether it was then
    /// captured is the capture roll's business, not reach's.
    ResolvedInReach,
    /// In reach when the gesture began and out of it when the attempt resolved. The hunter
    /// committed to a geometry that stopped being true — **cadence or resolution**.
    BeganInReachResolvedOut,
    /// Out of reach when the gesture began, and the separation **grew** across the paid burst:
    /// the prey pulled away faster than the hunter closed — **the escape envelope**.
    PreyOutran,
    /// Out of reach when the gesture began and the gap was never closed, although it did not
    /// grow — **target selection or the pursuit controller**.
    BeganOutOfReach,
    /// No intent frame was recorded, so the attempt cannot be read. Only possible for an
    /// attempt that was already in flight when recording was turned on.
    Unreadable,
}

impl StrikeClass {
    pub fn as_str(self) -> &'static str {
        match self {
            StrikeClass::TargetLost => "target_lost",
            StrikeClass::ResolvedInReach => "resolved_in_reach",
            StrikeClass::BeganInReachResolvedOut => "began_in_reach_resolved_out",
            StrikeClass::PreyOutran => "prey_outran",
            StrikeClass::BeganOutOfReach => "began_out_of_reach",
            StrikeClass::Unreadable => "unreadable",
        }
    }
}

/// One paid attempt, from the gesture that opened it to the settlement that closed it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrikeRecord {
    pub hunter: OrganismId,
    /// The member's `attack_counter` after the increment that opened this attempt: the same
    /// key [`crate::hunter::HunterEvent::Attempt::attack_counter`] carries, so a record and an
    /// event reconcile one for one.
    pub attack_counter: u64,
    /// The boundary the windup began on. `None` only for an attempt already in flight when
    /// recording was turned on.
    pub intent: Option<StrikeFrame>,
    /// The boundary the paid burst began on.
    pub strike: Option<StrikeFrame>,
    /// The boundary the settlement resolved on.
    pub resolution: StrikeFrame,
    pub outcome: AttemptOutcome,
    /// What the attempt actually cost, as the settlement charged it.
    pub energy_paid: f64,
    /// The target named at the intent frame is not the one named at resolution.
    pub target_changed: bool,
    /// The target's handle no longer resolved at settlement: it died, was claimed and removed,
    /// or its slot was reused.
    pub target_missing_at_resolution: bool,
    /// The target stood on a different cube face at resolution than at intent. Recorded
    /// because a seam crossing is the one way a separation can be measured through a different
    /// unfolding than the one the hunter committed to.
    pub target_crossed_face: bool,
    /// The same for the hunter itself.
    pub hunter_crossed_face: bool,
    /// The prey's realised speed over the windup and over the strike, px/s: surface
    /// displacement divided by the phase's own duration. `None` when either end is missing or
    /// the two points are not reachable within one local unfolding.
    pub target_speed_windup: Option<f64>,
    pub target_speed_strike: Option<f64>,
    /// The hunter's realised speed over the same two phases, px/s — what the paid burst
    /// actually delivered against `strike_speed_px_s`.
    pub hunter_speed_windup: Option<f64>,
    pub hunter_speed_strike: Option<f64>,
    /// The prey's heading change over the windup and over the strike, radians in `[0, π]`.
    /// `None` across a face change, where two chart headings are not comparable.
    pub target_turn_windup: Option<f64>,
    pub target_turn_strike: Option<f64>,
    /// The hunter's own heading change over the same two phases, radians in `[0, π]`. A
    /// member's turn is priced against the *same* motor budget as its travel
    /// (`|v| + r · |ω| ≤ u`) at the radius its claws imply, so this is not decoration: it is
    /// the other half of what the burst could have spent.
    pub hunter_turn_windup: Option<f64>,
    pub hunter_turn_strike: Option<f64>,
    /// The class this record's own geometry puts it in.
    pub class: StrikeClass,
    /// **Which pursuit stopping rule the world was running when this attempt was made.**
    /// Carried on the record so a held/delivered reading is never taken under a rule the world
    /// did not run, and so two arms of a paired experiment are distinguishable in one file.
    ///
    /// The serde default is [`rule_before_the_adoption`] — the forward half-space — and **not**
    /// [`super::PursuitStop::default`], which became the reach envelope when the envelope was
    /// adopted as the shipped rule on 2026-09-16. Every record written before this field
    /// existed ran under the half-space, so that is what a record with no `stop` says; reading
    /// those artifacts under today's default would relabel them as something they never ran.
    /// The function is named rather than derived exactly so that the two defaults cannot drift
    /// together again.
    #[serde(default = "rule_before_the_adoption")]
    pub stop: super::PursuitStop,
}

/// The pursuit stopping rule a [`StrikeRecord`] written before the `stop` field existed ran
/// under: the forward half-space, which was the only rule this workspace had until
/// 2026-09-16. Deliberately independent of [`super::PursuitStop::default`].
fn rule_before_the_adoption() -> super::PursuitStop {
    super::PursuitStop::ForwardHalfSpace
}

impl StrikeRecord {
    /// How much the separation moved across the paid burst: `resolution − strike`, in px of
    /// `effector_distance`. Positive means the prey pulled away.
    pub fn separation_change_over_strike(&self) -> Option<f64> {
        Some(self.resolution.effector_distance? - self.strike?.effector_distance?)
    }

    /// How much it moved across the whole attempt: `resolution − intent`.
    pub fn separation_change_total(&self) -> Option<f64> {
        Some(self.resolution.effector_distance? - self.intent?.effector_distance?)
    }

    /// Whether the pursuit held this member at the gesture, **under the rule this attempt was
    /// actually made under**. `None` when the frame carried no measure.
    pub fn held_at_intent(&self) -> Option<bool> {
        self.intent?.pursuit_holds(self.stop)
    }

    /// The same at the boundary the paid burst began on. This is the reading that decides
    /// whether the burst the member had just paid 0.08 e for was requested or suppressed.
    pub fn held_at_strike(&self) -> Option<bool> {
        self.strike?.pursuit_holds(self.stop)
    }

    fn classify(&self) -> StrikeClass {
        if self.target_missing_at_resolution || self.target_changed || self.resolution.target.is_none()
        {
            return StrikeClass::TargetLost;
        }
        if self.resolution.in_reach {
            return StrikeClass::ResolvedInReach;
        }
        let Some(intent) = self.intent else {
            return StrikeClass::Unreadable;
        };
        if intent.in_reach {
            return StrikeClass::BeganInReachResolvedOut;
        }
        match self.separation_change_over_strike() {
            Some(d) if d > 0.0 => StrikeClass::PreyOutran,
            _ => StrikeClass::BeganOutOfReach,
        }
    }
}

/// One attempt still in flight: what the recorder is holding until the settlement closes it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct OpenAttempt {
    hunter: OrganismId,
    intent: StrikeFrame,
    strike: Option<StrikeFrame>,
    attack_counter: Option<u64>,
}

/// The apex strike path's transient, opt-in state: the per-attempt recorder, and the pursuit
/// stopping rule this world runs. Never persisted, never hashed, never checkpointed.
///
/// The two live together because they are the same instrument: the rule is what the record
/// exists to measure, an arm that changes one always reads the other, and a record is only
/// legible beside the rule it was made under — which is why [`StrikeRecord::stop`] is copied
/// from here at close.
///
/// The rule is **not** a [`crate::WorldConfig`] field, and must not become one: a config field
/// would change `calibrate::config_hash` for every existing TOML and break the provenance of
/// every retained row. It is carried here instead, which is why a world's saved bytes do not
/// state it — and why adopting a new default needed a schema bump
/// ([`crate::snapshot::SCHEMA_VERSION`] 17) rather than a silent change of behaviour on resume.
#[derive(Clone, Debug, Default)]
pub struct StrikeRecorder {
    on: bool,
    /// The rule `crate::world::step`'s hunt-intent pass evaluates. It follows
    /// [`super::PursuitStop::default`] **by construction** — the shipped rule, the reach
    /// envelope, since 2026-09-16 — and is **independent of `on`**: turning recording on or off
    /// never moves it, and running either rule does not require recording.
    stop: super::PursuitStop,
    open: Vec<OpenAttempt>,
    closed: Vec<StrikeRecord>,
    /// Closed records discarded because nobody drained them. Never silently zero.
    dropped: u64,
    /// Attempts that resolved with no intent frame — only possible across a mid-attempt
    /// switch-on — and open attempts evicted by [`MAX_OPEN_ATTEMPTS`].
    unreadable: u64,
    evicted: u64,
}

impl StrikeRecorder {
    pub fn enabled(&self) -> bool {
        self.on
    }

    /// The pursuit stopping rule this world runs.
    pub fn pursuit_stop(&self) -> super::PursuitStop {
        self.stop
    }

    /// Run the pursuit's stopping predicate under `stop` from the next tick on — which an
    /// ordinary world never needs to do, because the default is the shipped rule. Records already
    /// closed keep the rule they were made under; an attempt still in flight is closed under
    /// the rule in force when it resolves, so a mid-attempt change is visible rather than
    /// silently retconned.
    pub fn set_pursuit_stop(&mut self, stop: super::PursuitStop) {
        self.stop = stop;
    }

    /// Turning it off drops everything in flight: a half-recorded attempt is not a record.
    pub fn set_enabled(&mut self, on: bool) {
        if self.on == on {
            return;
        }
        self.on = on;
        self.open.clear();
        if !on {
            self.closed.clear();
            self.dropped = 0;
            self.unreadable = 0;
            self.evicted = 0;
        }
    }

    /// The records closed since the last drain, oldest first, with how many were dropped
    /// undrained and how many resolved without an intent frame.
    pub fn drain(&mut self) -> (Vec<StrikeRecord>, u64, u64) {
        let dropped = std::mem::take(&mut self.dropped);
        let unreadable = std::mem::take(&mut self.unreadable);
        (std::mem::take(&mut self.closed), dropped, unreadable)
    }

    /// Open attempts evicted by the cap, since the recorder was turned on.
    pub fn evicted(&self) -> u64 {
        self.evicted
    }

    /// The gesture began: this member is cocking at this target, from here.
    pub fn open_intent(&mut self, hunter: OrganismId, intent: StrikeFrame) {
        if !self.on {
            return;
        }
        self.open.retain(|a| a.hunter != hunter);
        if self.open.len() >= MAX_OPEN_ATTEMPTS {
            self.open.remove(0);
            self.evicted += 1;
        }
        self.open.push(OpenAttempt { hunter, intent, strike: None, attack_counter: None });
    }

    /// The burst was paid for and began: this is where it started from.
    pub fn begin_strike(&mut self, hunter: OrganismId, attack_counter: u64, strike: StrikeFrame) {
        if !self.on {
            return;
        }
        if let Some(a) = self.open.iter_mut().find(|a| a.hunter == hunter) {
            a.strike = Some(strike);
            a.attack_counter = Some(attack_counter);
        }
    }

    /// The member left the gesture without paying for a burst: nothing to record.
    pub fn abandon(&mut self, hunter: OrganismId) {
        if !self.on {
            return;
        }
        self.open.retain(|a| a.hunter != hunter);
    }

    /// The settlement resolved this attempt. Builds the record, derives its class, and files
    /// it. Everything derived here is arithmetic on the three frames; nothing is re-measured.
    pub fn close(
        &mut self,
        hunter: OrganismId,
        attack_counter: u64,
        resolution: StrikeFrame,
        outcome: AttemptOutcome,
        energy_paid: f64,
        windup_seconds: f64,
        strike_seconds: f64,
        surface_distance: impl Fn(SurfacePoint, SurfacePoint) -> Option<f64>,
    ) {
        if !self.on {
            return;
        }
        let open = self
            .open
            .iter()
            .position(|a| a.hunter == hunter)
            .map(|i| self.open.remove(i));
        let (intent, strike) = match open {
            Some(a) => (Some(a.intent), a.strike),
            None => {
                self.unreadable += 1;
                (None, None)
            }
        };
        let speed = |from: Option<SurfacePoint>, to: Option<SurfacePoint>, seconds: f64| {
            if !(seconds > 0.0) {
                return None;
            }
            surface_distance(from?, to?).map(|d| d / seconds)
        };
        let turn = |from: Option<(SurfacePoint, Vec2)>, to: Option<(SurfacePoint, Vec2)>| {
            let (a_pos, a) = from?;
            let (b_pos, b) = to?;
            if a_pos.face.index() != b_pos.face.index() {
                return None;
            }
            let (a, b) = (a.normalized()?, b.normalized()?);
            Some(a.dot(b).clamp(-1.0, 1.0).acos())
        };
        let target_at = |f: Option<StrikeFrame>| f.and_then(|f| Some((f.target_pos?, f.target_heading?)));
        let mut record = StrikeRecord {
            hunter,
            attack_counter,
            intent,
            strike,
            resolution,
            outcome,
            energy_paid,
            target_changed: intent
                .and_then(|f| f.target)
                .is_some_and(|t| resolution.target.is_some_and(|r| r != t)),
            target_missing_at_resolution: resolution.target_pos.is_none(),
            target_crossed_face: match (
                StrikeFrame::face(intent.and_then(|f| f.target_pos)),
                StrikeFrame::face(resolution.target_pos),
            ) {
                (Some(a), Some(b)) => a != b,
                _ => false,
            },
            stop: self.stop,
            hunter_crossed_face: intent
                .is_some_and(|f| f.hunter_pos.face.index() != resolution.hunter_pos.face.index()),
            target_speed_windup: speed(
                intent.and_then(|f| f.target_pos),
                strike.and_then(|f| f.target_pos),
                windup_seconds,
            ),
            target_speed_strike: speed(
                strike.and_then(|f| f.target_pos),
                resolution.target_pos,
                strike_seconds,
            ),
            hunter_speed_windup: speed(
                intent.map(|f| f.hunter_pos),
                strike.map(|f| f.hunter_pos),
                windup_seconds,
            ),
            hunter_speed_strike: speed(
                strike.map(|f| f.hunter_pos),
                Some(resolution.hunter_pos),
                strike_seconds,
            ),
            target_turn_windup: turn(target_at(intent), target_at(strike)),
            target_turn_strike: turn(target_at(strike), target_at(Some(resolution))),
            hunter_turn_windup: turn(
                intent.map(|f| (f.hunter_pos, f.hunter_heading)),
                strike.map(|f| (f.hunter_pos, f.hunter_heading)),
            ),
            hunter_turn_strike: turn(
                strike.map(|f| (f.hunter_pos, f.hunter_heading)),
                Some((resolution.hunter_pos, resolution.hunter_heading)),
            ),
            class: StrikeClass::Unreadable,
        };
        record.class = record.classify();
        if self.closed.len() >= MAX_STRIKE_RECORDS {
            self.dropped += 1;
            return;
        }
        self.closed.push(record);
    }
}
