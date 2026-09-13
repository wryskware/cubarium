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

use cubarium_core::hunter::{HunterPhase, HunterView};
use cubarium_core::view::OrganismView;
use cubarium_surface::PathSegment;

use crate::art_present::present_seconds;
use crate::clock::DT;
use crate::lanternjaw::{
    AttackEpisode, AttackPhase, LivingPose, RECOIL_SECONDS, Reach, SCALE_MAX, SCALE_MIN,
    attack_channels,
};

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
/// and within [`SCALE_MIN`]`..=`[`SCALE_MAX`]. A profile outside it is not clamped and not
/// drawn as a Lanternjaw: the presenter reports it ([`crate::art_present::ArtPresenter::unsupported_hunters`])
/// and draws that organism with its ordinary rig, so a saved world never panics mid-frame
/// and never shows claws that disagree with the core's contact geometry.
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
            target_view: None,
        }
    }

    /// Record the next tick's frame. **Normative**: the current frame becomes `prev`; if the
    /// phase key changed, the new `from` is the reach the previous phase displayed at the new
    /// phase's entry boundary (`reach_at(prev, prev_from, started · DT)`), so every phase
    /// continues from the picture the last one left — an interrupted windup recoils from its
    /// partial cock, a settled strike from full extension.
    pub fn observe(&mut self, frame: HunterFrame) {
        let previous = self.cur;
        let previous_from = self.from;
        if frame.key() != previous.key() {
            self.from = reach_at(&previous, previous_from, frame.started as f64 * DT);
        }
        self.prev = Some(previous);
        self.prev_from = previous_from;
        self.cur = frame;
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

    /// The living pose and whole-rig scale at fraction `f` of `tick`, for a root that
    /// travelled `moved` during the last tick.
    ///
    /// **Normative**: `ambient = present_seconds(tick, f)`, `movement = movement_of(moved)`,
    /// `attack = episode_of(frame_at(tick, f))` at that same instant, `gut = cur.gut`,
    /// `cocoon = cur.gestation` (only a funded escrow has one), `scale = cur.scale`. Pure: the
    /// same inputs give the same pose at any frame rate, and nothing here advances anything.
    pub fn living_pose(&self, tick: u64, f: f64, moved: &[PathSegment]) -> (LivingPose, f64) {
        let seconds = present_seconds(tick, f);
        let (frame, from) = self.frame_at(tick, f);
        let pose = LivingPose {
            ambient: seconds,
            movement: movement_of(moved),
            attack: episode_of(frame, from, seconds),
            gut: f64::from(self.cur.gut),
            cocoon: self.cur.gestation.map(f64::from),
        };
        (pose, self.cur.scale)
    }
}
