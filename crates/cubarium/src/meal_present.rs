//! Meal onset for ordinary fauna: a presentation-only reading of the world's published
//! per-organism `fed` flag (any actual field intake this tick), so that the moment an animal
//! *starts* eating is legible and a meal that goes on is one continuous loop.
//!
//! What the recorded worlds show (the paired captures of 2026-09-13): an ordinary animal's
//! meal is a run of **one-tick nibbles** — `Feeding` for a tick with intake, `Seeking` for
//! a tick or two while it shuffles a pixel, `Feeding` again — so the presenter's mode-driven
//! choice of clip cross-fades between `move` and `feed` every tick or two, and the `feed`
//! loop (a two-second probe-and-bite cycle) never establishes; a new meal also joins that
//! loop at an arbitrary point of its cycle, since it plays on world time plus a per-organism
//! phase. Here an actual-intake **bout** — intake ticks no further apart than
//! [`MEAL_SETTLE_SECONDS`] — is presented as one continuous `feed` loop read from its own
//! onset (*bout time*, seconds since the first observed intake tick), held through the
//! nibble gaps, and handed back to the actual mode's clip only when intake has really
//! stopped. The body's path, heading, turn and scale are exactly what the world published
//! (a nibbling animal still shuffles along its real path), gestation immediately ends the
//! bout and fades its outgoing pose into the bud, the hunter adapter's members are its own,
//! and a `Feeding` organism that takes nothing shows exactly what it showed before — mode is
//! not an ingestion counter.
//!
//! The bout is faded in and out, never cut: a per-organism **weight** rises from 0 to 1 over
//! [`MEAL_FADE_SECONDS`] after the onset and falls back after the bout is over, and the body
//! is drawn as the mode-driven layers at `1 − weight` plus the bout-time feed reading at
//! `weight`. A gap of at most the settle window neither ends the bout nor restarts it, and
//! an intake that returns while the weight is still falling resumes the same origin, so the
//! loop never jumps. Memory is one small record per living ordinary organism keyed by its
//! full generation-bearing [`OrganismId`], dropped with the organism; a snap (first
//! observation, rewind, reload) carries no invented history — an organism already eating when
//! first seen is drawn as before until its next real onset.

use std::collections::BTreeMap;

use cubarium_core::ids::OrganismId;
use cubarium_core::view::RenderView;

use crate::art_present::{BODY_FADE_SECONDS, MAX_STEP_SECONDS, present_seconds};
use crate::clock::DT;

/// Seconds without observed intake after which a bout is over. A gap of at most this many
/// seconds is the pause between two nibbles, not a new meal: bridged, the loop continues.
/// Chosen at the elbow of the intake-gap distribution of the recorded worlds (gaps of 1–3
/// ticks make up two thirds of all gaps and fall off sharply by 5; beyond that the
/// distribution is flat — real pauses), so the feed gesture is never shown more than this
/// plus one fade after the last mouthful. Presentation only, never a biological timer.
pub const MEAL_SETTLE_SECONDS: f64 = 0.25;

/// Seconds over which the feed clip's origin fades between the shared phase and the bout's
/// own: the body cross-fade's own length, so a state change and an onset that arrive
/// together finish together.
pub const MEAL_FADE_SECONDS: f64 = BODY_FADE_SECONDS;

/// What the presenter remembers about one ordinary organism's eating.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MealMemory {
    /// Presentation seconds at which the current bout's first observed intake tick began
    /// ([`present_seconds`]`(tick, 0)`), or `None` while the organism's eating has no known
    /// onset — it was already fed when first seen, or has not eaten since it was seen.
    pub onset: Option<f64>,
    /// The last tick intake was observed, if any since the organism was first seen.
    pub last_fed: Option<u64>,
    /// The weight of the bout-time reading of the feed clip as the previous observed tick
    /// left it, and as this one did; a frame at fraction `f` draws their mix.
    pub weight_prev: f32,
    pub weight: f32,
}

impl MealMemory {
    /// The weight of the bout-time reading at fraction `f` of the current tick.
    pub fn weight_at(&self, f: f64) -> f32 {
        let f = if f.is_finite() {
            f.clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        self.weight_prev + (self.weight - self.weight_prev) * f
    }

    /// Seconds into the bout at presentation `seconds`: where the feed clip is read while
    /// the bout-time reading has weight. `None` without a known onset.
    pub fn bout_seconds(&self, seconds: f64) -> Option<f64> {
        self.onset.map(|onset| (seconds - onset).max(0.0))
    }

    /// Whether intake has been seen within the settle window as of `tick`.
    pub fn active_at(&self, tick: u64) -> bool {
        self.last_fed
            .is_some_and(|last| tick >= last && (tick - last) as f64 * DT <= MEAL_SETTLE_SECONDS)
    }
}

/// The meal memory of every living ordinary organism.
#[derive(Debug, Default)]
pub struct Meals {
    memories: BTreeMap<OrganismId, MealMemory>,
    last_tick: Option<u64>,
}

impl Meals {
    pub fn new() -> Meals {
        Meals::default()
    }

    /// Observe a published tick. `snap` is the presenter's own snap (first view, rewind or
    /// a replaced world): everything starts over with no history. `skip` names the ids
    /// another presentation owns (the hunter adapter's members), which are never tracked.
    ///
    /// **Normative.** A repeated observation of the same tick changes nothing. Otherwise,
    /// with `now = present_seconds(tick, 0)` and `dt` the seconds since the last observed
    /// tick (clamped to [`MAX_STEP_SECONDS`]), for every organism the view carries:
    ///
    /// * an id not seen before enters with no onset and `last_fed = tick` if it is fed
    ///   (its eating has no known beginning) — never a bout in flight;
    /// * a seen id that is fed this tick and holds no escrow (gestation beats the meal, so a
    ///   budding organism's intake is not a bout) begins a bout at `now` if its eating is not
    ///   *active* ([`MealMemory::active_at`]) and its weight has fallen to 0; otherwise it
    ///   continues the bout it is in (same onset), whether the intake never paused, paused
    ///   for at most the settle window, or returned while the weight was still falling;
    /// * gestation clears the intake clock and immediately fades out the outgoing bout;
    /// * the weight moves toward 1 while the bout is active and has an onset, toward 0
    ///   otherwise, by `dt / MEAL_FADE_SECONDS`, and the previous tick's value is kept for
    ///   the frames in between;
    /// * an id the view no longer carries is forgotten, so the memory is bounded by the
    ///   living population.
    pub fn observe(&mut self, view: &RenderView, snap: bool, skip: &dyn Fn(OrganismId) -> bool) {
        if snap {
            self.memories.clear();
            self.last_tick = None;
        }
        if self.last_tick == Some(view.tick) {
            return;
        }
        let dt = match self.last_tick {
            Some(last) if view.tick >= last => {
                (((view.tick - last) as f64) * DT).clamp(0.0, MAX_STEP_SECONDS)
            }
            _ => 0.0,
        };
        let step = (dt / MEAL_FADE_SECONDS) as f32;
        let now = present_seconds(view.tick, 0.0);
        for o in &view.organisms {
            if skip(o.id) {
                continue;
            }
            let intake = o.fed && o.gestation.is_none();
            match self.memories.get_mut(&o.id) {
                Some(m) => {
                    m.weight_prev = m.weight;
                    if intake {
                        if !m.active_at(view.tick) && m.weight <= 0.0 {
                            m.onset = Some(now);
                        }
                        m.last_fed = Some(view.tick);
                    }
                    if o.gestation.is_some() {
                        // Do not abruptly replace an established bout with the unrelated
                        // shared-phase feed pose when the ordinary body begins its bud fade.
                        // End now (no settle hold), retaining only the outgoing pose origin.
                        m.last_fed = None;
                    }
                    let target = if m.onset.is_some() && m.active_at(view.tick) {
                        1.0
                    } else {
                        0.0
                    };
                    m.weight = if m.weight < target {
                        (m.weight + step).min(target)
                    } else {
                        (m.weight - step).max(target)
                    };
                }
                None => {
                    self.memories.insert(
                        o.id,
                        MealMemory {
                            onset: None,
                            last_fed: intake.then_some(view.tick),
                            weight_prev: 0.0,
                            weight: 0.0,
                        },
                    );
                }
            }
        }
        // Bounded by the living: an id the view no longer carries is forgotten (the skipped
        // ids are never in here, so the counts cannot be compared to decide this cheaply).
        let live: std::collections::HashSet<OrganismId> = view
            .organisms
            .iter()
            .filter(|o| !skip(o.id))
            .map(|o| o.id)
            .collect();
        self.memories.retain(|id, _| live.contains(id));
        self.last_tick = Some(view.tick);
    }

    /// The memory of an organism, if it is tracked.
    pub fn memory_of(&self, id: OrganismId) -> Option<MealMemory> {
        self.memories.get(&id).copied()
    }

    /// Transfer ownership at the hunter adapter's boundary, which may follow `observe`
    /// for this same tick. No ordinary meal history may survive that transfer.
    pub fn forget(&mut self, id: OrganismId) {
        self.memories.remove(&id);
    }

    /// How many organisms are tracked.
    pub fn len(&self) -> usize {
        self.memories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }

    /// The feed clip's bout-time reading for a body drawn at presentation `seconds`,
    /// fraction `f` of the current tick: `(bout seconds, weight)`, or `None` when the body
    /// has no bout with weight — in which case it is drawn exactly as before.
    pub fn bout(&self, id: OrganismId, seconds: f64, f: f64) -> Option<(f64, f32)> {
        let m = self.memories.get(&id)?;
        let weight = m.weight_at(f);
        if weight <= 0.0 {
            return None;
        }
        m.bout_seconds(seconds).map(|t| (t, weight))
    }
}
