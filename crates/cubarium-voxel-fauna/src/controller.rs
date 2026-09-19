//! **The phase-one controller boundary** — the one seam between a body and whatever
//! drives it (`design/voxel-senses-phase1-plan.md`, "Phase-one body and action contract").
//!
//! A [`Controller`] receives **only the observation vector and its own private memory**
//! and answers with the three local actions. It sees no world, no flora stock, no site,
//! no heading and no clock: every world-derived quantity reaches it inside the vector, in
//! the order the founder's [`Manifest`](crate::Manifest) declares, and nothing in the
//! resolve path below can hand a policy a privileged target. That is the policy boundary
//! the tests plan's §1 "Policy boundary" row names, and it is enforced by shape: there is
//! no other input.
//!
//! Two kinds of controller are interchangeable behind the trait:
//!
//! - **Diagnostic controllers** (the P1-C heuristics; [`Scripted`] here for tests)
//!   return [`Response::Bounded`] — actions already inside the manifest's bounds.
//! - **A GRU policy** returns [`Response::Logits`] — the raw network outputs. The
//!   adapter ([`resolve_actions`]) applies the manifest's transfers (sigmoid for
//!   forward/feed, tanh for turn) and the fixed 0.05 deadband to logits, and the same
//!   deadband to bounded actions, **identically in training and viewing**: the arena
//!   tick, the ES episode driver and the viewer all resolve through this one function.
//!
//! The shape-aware GRU runtime itself is P1-D (`crates/cubarium-search`): it plugs in as
//! a `Controller` whose `drive` runs one GRU forward step over the observation and wraps
//! the three outputs in [`Response::Logits`], keeping its hidden state as its private
//! memory and clearing it in `reset` when a fresh episode starts.

use serde::{Deserialize, Serialize};

use crate::manifest::{Manifest, Transfer};

/// The three bounded local actions, in manifest order: forward effort, signed turn
/// effort, feed effort. Zero movement is rest; turning while stopped is permitted and
/// paid; a feed above the deadband attempts one local bite for the interval it is held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Actions {
    pub forward: f64,
    pub turn: f64,
    pub feed: f64,
}

impl Actions {
    /// No action at all: what a founder holds before its controller is ever sampled.
    pub const REST: Actions = Actions {
        forward: 0.0,
        turn: 0.0,
        feed: 0.0,
    };
}

/// What a controller answers with. The distinction is the whole adapter: bounded
/// responses are deadbanded as they are, logits are transferred first, and both paths
/// are the same code in training and viewing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Response {
    /// Actions already inside the manifest's bounds, from a diagnostic controller.
    Bounded(Actions),
    /// Unbounded logits in manifest action order, from a network controller. The
    /// adapter applies the manifest's transfer per action before the deadband.
    Logits([f64; 3]),
}

/// One controller: a pure function of the observation and its own private state. The
/// observation slice is guaranteed finite by the sampler that builds it; a controller
/// must not need more than `manifest.inputs()` of it, and must not get anything else.
///
/// Controllers are `Send + Sync` because the layer they sit in is a bevy resource and
/// an episode worker's payload: one body's controller is driven from one tick at a
/// time, but the table itself has to move between threads.
pub trait Controller: Send + Sync {
    /// Answer one observation sample, once per controller period. The only inputs are
    /// the observation vector and this controller's own private memory (`&mut self`).
    fn drive(&mut self, observation: &[f64]) -> Response;

    /// Drop all private memory. A fresh episode starts from here: hidden state is
    /// episode-private and never carried across one.
    fn reset(&mut self);
}

/// The shared action adapter: transfer, clamp, deadband — in that order, identically
/// for training and viewing. Non-finite logits and out-of-range bounded actions are
/// rejected to the deadband's zero rather than trained through.
pub fn resolve_actions(response: Response, manifest: &Manifest) -> Actions {
    let deadband = manifest.deadband;
    match response {
        Response::Bounded(a) => Actions {
            forward: deadband_low(clean(a.forward, manifest.actions[0].low, manifest.actions[0].high), deadband),
            turn: deadband_sym(clean(a.turn, manifest.actions[1].low, manifest.actions[1].high), deadband),
            feed: deadband_low(clean(a.feed, manifest.actions[2].low, manifest.actions[2].high), deadband),
        },
        Response::Logits(l) => Actions {
            forward: deadband_low(transfer(l[0], manifest.actions[0].transfer), deadband),
            turn: deadband_sym(transfer(l[1], manifest.actions[1].transfer), deadband),
            feed: deadband_low(transfer(l[2], manifest.actions[2].transfer), deadband),
        },
    }
}

/// A finite value clamped into the action's declared bounds; anything non-finite is 0.
fn clean(v: f64, low: f64, high: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(low, high)
}

/// One manifest transfer applied to a logit; non-finite in, zero out.
fn transfer(logit: f64, t: Transfer) -> f64 {
    if !logit.is_finite() {
        return 0.0;
    }
    match t {
        Transfer::Sigmoid => 1.0 / (1.0 + (-logit).exp()),
        Transfer::Tanh => logit.tanh(),
    }
}

/// Forward and feed deadband: anything below it is no action.
fn deadband_low(v: f64, deadband: f64) -> f64 {
    if v < deadband {
        0.0
    } else {
        v
    }
}

/// Turn deadband, on the absolute value, keeping the sign.
fn deadband_sym(v: f64, deadband: f64) -> f64 {
    if v.abs() < deadband {
        0.0
    } else {
        v
    }
}

/// The controller table: one controller per founder body that has one, keyed by animal
/// id in id order. Controller memory is **driver-owned session state, not world
/// state**: it is skipped by the fauna snapshot (a loaded layer's founders rest until
/// their driver installs controllers again), and cloning a fauna starts the copy
/// without it — cloning a body does not clone a mind.
#[derive(Default)]
pub struct FounderControllers {
    entries: Vec<(u64, Box<dyn Controller>)>,
}

impl FounderControllers {
    /// Install or replace one body's controller, keeping the id order.
    pub fn set(&mut self, id: u64, controller: Box<dyn Controller>) {
        match self.entries.binary_search_by_key(&id, |e| e.0) {
            Ok(at) => self.entries[at].1 = controller,
            Err(at) => self.entries.insert(at, (id, controller)),
        }
    }

    /// Take a body's controller back, if it has one.
    pub fn take(&mut self, id: u64) -> Option<Box<dyn Controller>> {
        let at = self.entries.binary_search_by_key(&id, |e| e.0).ok()?;
        Some(self.entries.remove(at).1)
    }

    /// Drive one body's controller with an observation, if it has one.
    pub fn drive(&mut self, id: u64, observation: &[f64]) -> Option<Response> {
        let at = self.entries.binary_search_by_key(&id, |e| e.0).ok()?;
        Some(self.entries[at].1.drive(observation))
    }

    /// How many bodies currently have a controller.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Clone for FounderControllers {
    /// Controller memory is not world state and is not cloned: the copy starts
    /// mindless, the same way a snapshot does. Installing fresh controllers is what a
    /// fresh episode does.
    fn clone(&self) -> Self {
        FounderControllers::default()
    }
}

impl std::fmt::Debug for FounderControllers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<u64> = self.entries.iter().map(|e| e.0).collect();
        f.debug_tuple("FounderControllers").field(&ids).finish()
    }
}

/// A diagnostic controller that cycles a fixed script of already-bounded actions, one
/// per drive. It is the plumbing test's controller — motion, contact and feeding
/// behaviour are driven with it — and **not** the P1-C foraging heuristic, which reads
/// the senses this phase does not have yet.
#[derive(Clone, Debug, Default)]
pub struct Scripted {
    script: Vec<Actions>,
    next: usize,
}

impl Scripted {
    /// Cycle `script` forever; an empty script rests.
    pub fn new(script: Vec<Actions>) -> Scripted {
        Scripted { script, next: 0 }
    }
}

impl Controller for Scripted {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        if self.script.is_empty() {
            return Response::Bounded(Actions::REST);
        }
        let action = self.script[self.next % self.script.len()];
        self.next += 1;
        Response::Bounded(action)
    }

    fn reset(&mut self) {
        self.next = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Founder;

    /// The adapter is the policy boundary's other half: logits go through the
    /// manifest's transfers and then the deadband, bounded actions only through the
    /// deadband, and neither path can touch a world — the function takes no world.
    #[test]
    fn the_adapter_transfers_logits_and_deadbands_both_kinds() {
        let manifest = Founder::Blind.manifest();

        // Sigmoid logits, then the 0.05 deadband: a small logit is zeroed, a large one
        // saturates to 1.
        // σ(0) = 0.5 for forward and feed (above the deadband), and tanh(0) = 0 is
        // deadbanded to no turn.
        let a = resolve_actions(Response::Logits([0.0, 0.0, 100.0]), &manifest);
        assert_eq!((a.forward, a.turn, a.feed), (0.5, 0.0, 1.0));
        // σ(0) = 0.5 exactly, so forward and feed are 0.5; turn tanh(5) ≈ 1.
        let a = resolve_actions(Response::Logits([0.0, 5.0, 0.0]), &manifest);
        assert_eq!(a.forward, 0.5);
        assert_eq!(a.feed, 0.5);
        assert!(a.turn > 0.99);
        // tanh of a small logit is inside the deadband: no turn.
        let a = resolve_actions(Response::Logits([10.0, 0.01, 10.0]), &manifest);
        assert_eq!(a.turn, 0.0);
        assert!(a.forward > 0.9 && a.feed > 0.9);

        // Bounded responses are not transferred again: the heuristic's 0.5 stays 0.5.
        let a = resolve_actions(
            Response::Bounded(Actions {
                forward: 0.5,
                turn: 0.5,
                feed: 0.5,
            }),
            &manifest,
        );
        assert_eq!(
            (a.forward, a.turn, a.feed),
            (0.5, 0.5, 0.5),
            "sigmoid must not be applied to already-bounded actions"
        );

        // Out-of-range and non-finite responses are rejected, not trained through.
        let a = resolve_actions(
            Response::Bounded(Actions {
                forward: f64::NAN,
                turn: 7.0,
                feed: -3.0,
            }),
            &manifest,
        );
        assert_eq!(
            (a.forward, a.turn, a.feed),
            (0.0, 1.0, 0.0),
            "NAN forward is zero, turn 7 clamps to 1, feed -3 clamps to 0"
        );
        let a = resolve_actions(Response::Logits([f64::NAN, f64::INFINITY, f64::NEG_INFINITY]), &manifest);
        assert_eq!(a, Actions::REST, "non-finite logits are zeroed");
    }

    /// The scripted diagnostic emits its bounded script in order, cycles, and `reset`
    /// rewinds it — the fresh-episode contract for controller memory.
    #[test]
    fn the_scripted_diagnostic_cycles_and_resets() {
        let mut c = Scripted::new(vec![
            Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0,
            },
            Actions {
                forward: 0.0,
                turn: -1.0,
                feed: 0.0,
            },
        ]);
        for round in 0..2 {
            for expected in [1.0, 0.0] {
                let Response::Bounded(a) = c.drive(&[0.0; 23]) else {
                    panic!("a scripted controller answers bounded");
                };
                assert_eq!(a.forward, expected, "round {round}");
            }
        }
        c.reset();
        let Response::Bounded(a) = c.drive(&[0.0; 23]) else {
            panic!("a scripted controller answers bounded");
        };
        assert_eq!(a.forward, 1.0, "reset rewound the script");
    }
}
