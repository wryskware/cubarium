//! The controller bodies behind the fauna's **own** controller seam.
//!
//! P1-B landed the boundary this crate used to mirror: `cubarium_voxel_fauna::Controller`
//! (`drive(&mut self, observation) -> Response` + `reset()`), one shared adapter
//! (`resolve_actions`: the manifest's transfers and deadband, identical for training and
//! viewing), and a per-body controller table the fauna tick's controller stage drives.
//! This module now only **supplies bodies** for that seam:
//!
//! - [`GruPolicy`]: a validated [`ShapePolicy`] as a `Controller` returning
//!   [`Response::Logits`] — the shape-aware runtime the P1-B module doc reserves for
//!   P1-D. The adapter is *not* duplicated here: the fauna resolves logits through
//!   `resolve_actions`, so training and viewing decode identically by construction.
//! - [`NoIntake`] and [`StationaryFeeding`]: the two disclosed diagnostic controls, as
//!   bounded responses.
//! - The observation-only heuristic slot: the fauna's own `BlindForager` /
//!   `BrowserForager` — real observation-only controllers P1-B shipped; this crate does
//!   not keep a second heuristic.
//!
//! The policy boundary is enforced by the fauna's trait shape: a controller receives the
//! observation vector and its own memory and nothing else. What this module adds is the
//! digest discipline: a [`GruPolicy`] is built only through
//! [`tensor::shape_policy`] against the founder's manifest digest, so a policy authored
//! against one schema is refused before any episode runs.
//!
//! # Who samples, who holds, who resolves
//!
//! The **fauna tick** samples each due founder's observation (its own
//! `body::observation`), drives the installed controller, resolves the response through
//! the shared adapter, and holds the actions in the body's `founder_state.held` until
//! the next due tick. The **driver** installs a fresh controller per episode and calls
//! `reset()` — fresh memory by construction and by contract — then reads outcomes from
//! the world and ledger. It never samples, holds or resolves.

use std::sync::{Arc, Mutex};

use cubarium_core::neural::ShapePolicy;
use cubarium_core::neural::gru::HIDDEN;
use cubarium_voxel_fauna::{
    Actions, BlindForager, BrowserForager, Controller, Founder, Response, resolve_actions,
};

use super::super::tensor;
use super::voxel_schema_digest;

/// How many leading observation channels are the founder's own `Self` block. The two
/// manifests agree: indices 0..8 are energy, reserve, birth readiness, structural loss,
/// assimilated intake, resolved forward, resolved turn, motor delivery. Everything from
/// index 8 on is a **sense** — contact, wet, taste, chem, light, cone — including its
/// validity value.
pub const SELF_CHANNELS: usize = 8;

/// A GRU policy as a fauna controller: validated weights, one hidden state, raw logits
/// out. The fauna's shared adapter transfers and deadbands them.
///
/// `ablate_senses` is the P2-B diagnostic: with it set, every channel from
/// [`SELF_CHANNELS`] on — validity included — is zeroed **here**, at the point the
/// observation is handed to the network, never in the fauna crate. The body still senses
/// normally, the policy's own memory is untouched, and a policy whose score does not move
/// under the ablation was not using its senses.
#[derive(Clone, Debug)]
pub struct GruPolicy<const I: usize> {
    policy: ShapePolicy<I, 3>,
    hidden: [f64; HIDDEN],
    ablate_senses: bool,
}

impl<const I: usize> GruPolicy<I> {
    /// Build a controller from a flat parameter vector, validated against the founder's
    /// manifest digest — the [`tensor::shape_policy`] boundary with the digest plug-in
    /// ([`super::voxel_schema_digest`]) filled in.
    ///
    /// Refuses a wrong-length vector, a non-finite value, and a digest mismatch by name.
    pub fn new(theta: &[f64], digest: u64) -> Result<GruPolicy<I>, String> {
        Ok(GruPolicy {
            policy: tensor::shape_policy::<I, 3>(theta, digest)?,
            hidden: [0.0; HIDDEN],
            ablate_senses: false,
        })
    }

    /// The same policy with every sense channel zeroed before it is read
    /// (see [`SELF_CHANNELS`]).
    pub fn new_ablated(theta: &[f64], digest: u64) -> Result<GruPolicy<I>, String> {
        let mut p = GruPolicy::new(theta, digest)?;
        p.ablate_senses = true;
        Ok(p)
    }

    /// Whether this body reads its senses.
    pub fn senses_ablated(&self) -> bool {
        self.ablate_senses
    }

    /// The validated policy this controller runs.
    pub fn policy(&self) -> &ShapePolicy<I, 3> {
        &self.policy
    }
}

impl<const I: usize> Controller for GruPolicy<I> {
    fn drive(&mut self, observation: &[f64]) -> Response {
        let mut x = [0.0; I];
        x.copy_from_slice(observation);
        if self.ablate_senses {
            x[SELF_CHANNELS.min(I)..].fill(0.0);
        }
        let logits = self.policy.weights.forward(&x, &mut self.hidden);
        Response::Logits(logits)
    }

    fn reset(&mut self) {
        self.hidden = [0.0; HIDDEN];
    }
}

/// The blind founder's GRU runtime.
pub type GruBlind = GruPolicy<23>;
/// The browser's GRU runtime.
pub type GruBrowser = GruPolicy<37>;

/// Stand still, take nothing: what the starting stores alone buy, and the check that no
/// free intake or scoring artifact exists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoIntake;

impl Controller for NoIntake {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        Response::Bounded(cubarium_voxel_fauna::Actions::REST)
    }

    fn reset(&mut self) {}
}

/// Stand still and feed continuously: the most favourable stationary strategy that
/// exists, to show why remaining alive or feeding in place is not foraging from an
/// off-food start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StationaryFeeding;

impl Controller for StationaryFeeding {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        Response::Bounded(cubarium_voxel_fauna::Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        })
    }

    fn reset(&mut self) {}
}

/// Open-loop cruise: half forward effort, no turn, full feed, **without reading the
/// observation at all**. The other half of the P2-B diagnostic pair: if a trained policy
/// cannot beat this on a start that faces its food, the start handed it the answer.
///
/// Half cruise, not full: full cruise costs exactly basal upkeep by design, so a control
/// pinned at 1.0 measures the motor budget rather than the task. 0.5 is the cheapest
/// speed that still crosses the arena inside the horizon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cruise;

/// The forward effort [`Cruise`] holds.
pub const CRUISE_FORWARD: f64 = 0.5;

impl Controller for Cruise {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        Response::Bounded(cubarium_voxel_fauna::Actions {
            forward: CRUISE_FORWARD,
            turn: 0.0,
            feed: 1.0,
        })
    }

    fn reset(&mut self) {}
}

/// One teacher step: the observation a controller was handed, and the action the fauna's
/// own adapter resolved from its answer.
///
/// The observation is the vector the body sampled — exactly what a GRU in that slot would
/// have read — and the action is the *adapted* one (`resolve_actions`), which is what the
/// world went on to hold. Nothing fixture-side is here: there is no site, no distance, no
/// stock. A pair is what one controller saw and what the world then did.
#[derive(Clone, Debug, PartialEq)]
pub struct TeacherStep {
    pub observation: Vec<f64>,
    pub action: Actions,
}

/// Where [`RecordingController`]s put their pairs: **one buffer per controller**, in the
/// order the controllers were made. Shared because each controller is moved into the
/// fauna's table and never handed back.
///
/// Per controller, not per sink: a landscape episode hands one fresh controller to each
/// of its acting bodies (D7), and a single shared buffer interleaved their samples into
/// one incoherent stream — and every body's `reset()` cleared what the others had
/// recorded (P5-B's finding; the P5-C fix). The episode driver hands controllers out in
/// acting-body order, so buffer `k` is body `k`'s stream.
pub type TeacherSink = Arc<Mutex<Vec<Vec<TeacherStep>>>>;

/// A fresh, empty sink.
pub fn teacher_sink() -> TeacherSink {
    Arc::new(Mutex::new(Vec::new()))
}

/// A transparent wrapper around any [`Controller`] that records `(observation, adapted
/// action)` for every sample into its own buffer of a [`TeacherSink`].
///
/// Transparent is the contract: the wrapped controller sees the same observation, and the
/// **unmodified** response goes back to the fauna, so an episode recorded is an episode
/// run. The adapter is the fauna's own [`resolve_actions`], never a second copy, so the
/// recorded action is the one the body held.
pub struct RecordingController {
    inner: Box<dyn Controller>,
    manifest: cubarium_voxel_fauna::Manifest,
    sink: TeacherSink,
    /// This controller's buffer in the sink.
    slot: usize,
}

impl RecordingController {
    /// Wrap `inner`, opening a new buffer at the end of `sink` for it.
    pub fn new(
        inner: Box<dyn Controller>,
        manifest: cubarium_voxel_fauna::Manifest,
        sink: TeacherSink,
    ) -> RecordingController {
        let slot = {
            let mut buffers = sink.lock().expect("teacher sink");
            buffers.push(Vec::new());
            buffers.len() - 1
        };
        RecordingController {
            inner,
            manifest,
            sink,
            slot,
        }
    }

    /// Which buffer of the sink this controller writes.
    pub fn slot(&self) -> usize {
        self.slot
    }
}

impl Controller for RecordingController {
    fn drive(&mut self, observation: &[f64]) -> Response {
        let response = self.inner.drive(observation);
        let action = resolve_actions(response, &self.manifest);
        if let Ok(mut sink) = self.sink.lock() {
            sink[self.slot].push(TeacherStep {
                observation: observation.to_vec(),
                action,
            });
        }
        response
    }

    /// Fresh memory means a fresh recording: one body's episode is one stream, so this
    /// controller's own buffer is emptied with the wrapped controller's memory. Nobody
    /// else's is touched.
    fn reset(&mut self) {
        self.inner.reset();
        if let Ok(mut sink) = self.sink.lock() {
            sink[self.slot].clear();
        }
    }
}

/// What drives the body for one episode. Built per episode through
/// [`EpisodeDriver::fresh`] so the controller's memory — hidden state, turn preference —
/// is never carried between episodes.
#[derive(Clone, Debug)]
pub struct EpisodeDriver {
    founder: Founder,
    kind: EpisodeKind,
    /// Only meaningful for [`EpisodeKind::Gru`]: run the policy with its sense channels
    /// zeroed (see [`SELF_CHANNELS`]).
    ablate_senses: bool,
    /// When set, every body this driver hands out is wrapped in a
    /// [`RecordingController`] writing its `(observation, adapted action)` pairs here.
    /// The episode itself is unchanged: the wrapper returns the inner response untouched.
    record: Option<TeacherSink>,
}

/// The controller body a driver runs.
#[derive(Clone, Debug)]
pub enum EpisodeKind {
    /// The seeded or perturbed GRU centre, by shape.
    Gru(EpisodeGru),
    /// The disclosed diagnostic controls and the observation-only heuristic.
    Control(VoxelControl),
}

/// The shape of an [`EpisodeKind::Gru`]: the two voxel schemas this driver runs.
#[derive(Clone, Debug)]
pub enum EpisodeGru {
    Blind(Box<ShapePolicy<23, 3>>),
    Browser(Box<ShapePolicy<37, 3>>),
}

/// The diagnostic controllers the commands name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoxelControl {
    NoIntake,
    StationaryFeeding,
    /// Open-loop half cruise with full feed, reading nothing ([`Cruise`]).
    Cruise,
    /// The observation-only heuristic: the fauna's own `BlindForager` (blind founder) or
    /// `BrowserForager` (browser) — real chem/gaze heuristics shipped with the seam,
    /// interchangeable here behind the same [`Controller`] trait.
    Heuristic,
}

impl EpisodeDriver {
    /// A GRU driver from a flat parameter vector, for `founder`, validated against that
    /// founder's manifest digest.
    pub fn gru(theta: &[f64], founder: Founder) -> Result<EpisodeDriver, String> {
        let digest = voxel_schema_digest(founder);
        let gru = match founder {
            Founder::Blind => {
                EpisodeGru::Blind(Box::new(tensor::shape_policy::<23, 3>(theta, digest)?))
            }
            Founder::Browser => {
                EpisodeGru::Browser(Box::new(tensor::shape_policy::<37, 3>(theta, digest)?))
            }
        };
        Ok(EpisodeDriver {
            founder,
            kind: EpisodeKind::Gru(gru),
            ablate_senses: false,
            record: None,
        })
    }

    /// The same driver with the policy's sense channels zeroed before every forward
    /// pass. A no-op on a control, which reads no observation in the first place.
    pub fn with_ablated_senses(mut self) -> EpisodeDriver {
        self.ablate_senses = matches!(self.kind, EpisodeKind::Gru(_));
        self
    }

    /// Whether this driver runs a sense-ablated policy.
    pub fn senses_ablated(&self) -> bool {
        self.ablate_senses
    }

    /// A diagnostic driver for `founder`.
    pub fn control(control: VoxelControl, founder: Founder) -> EpisodeDriver {
        EpisodeDriver {
            founder,
            kind: EpisodeKind::Control(control),
            ablate_senses: false,
            record: None,
        }
    }

    /// The same driver, recording every `(observation, adapted action)` pair it produces
    /// into `sink`: every controller [`EpisodeDriver::fresh`] hands out opens its own
    /// buffer there, so an episode with `n` acting bodies leaves `n` buffers, in body
    /// order. Use one sink per episode.
    pub fn recording(mut self, sink: TeacherSink) -> EpisodeDriver {
        self.record = Some(sink);
        self
    }

    /// The founder this driver runs.
    pub fn founder(&self) -> Founder {
        self.founder
    }

    /// The controller body a driver runs, for diagnostics and tests.
    pub fn kind(&self) -> &EpisodeKind {
        &self.kind
    }

    /// A fresh controller for one episode: new memory by construction, then an explicit
    /// `reset()` — the fauna's fresh-episode contract names `Controller::reset`, and
    /// doing both is the point of the check.
    pub fn fresh(&self) -> Box<dyn Controller> {
        let mut controller: Box<dyn Controller> = match &self.kind {
            EpisodeKind::Gru(EpisodeGru::Blind(policy)) => Box::new(GruPolicy::<23> {
                policy: (**policy).clone(),
                hidden: [0.0; HIDDEN],
                ablate_senses: self.ablate_senses,
            }),
            EpisodeKind::Gru(EpisodeGru::Browser(policy)) => Box::new(GruPolicy::<37> {
                policy: (**policy).clone(),
                hidden: [0.0; HIDDEN],
                ablate_senses: self.ablate_senses,
            }),
            EpisodeKind::Control(VoxelControl::NoIntake) => Box::new(NoIntake),
            EpisodeKind::Control(VoxelControl::StationaryFeeding) => Box::new(StationaryFeeding),
            EpisodeKind::Control(VoxelControl::Cruise) => Box::new(Cruise),
            // The heuristic slot is the fauna's own observation-only heuristics: the
            // body the driver runs decides which one fits its founder.
            EpisodeKind::Control(VoxelControl::Heuristic) => match self.founder {
                Founder::Blind => Box::new(BlindForager::new()),
                Founder::Browser => Box::new(BrowserForager::new()),
            },
        };
        if let Some(sink) = &self.record {
            controller = Box::new(RecordingController::new(
                controller,
                self.founder.manifest(),
                Arc::clone(sink),
            ));
        }
        controller.reset();
        controller
    }

    /// The driver's name, for job labels and reports.
    pub fn name(&self) -> String {
        match &self.kind {
            EpisodeKind::Gru(_) if self.ablate_senses => "gru-ablated".into(),
            EpisodeKind::Gru(_) => "gru".into(),
            EpisodeKind::Control(VoxelControl::NoIntake) => "no-intake".into(),
            EpisodeKind::Control(VoxelControl::StationaryFeeding) => "stationary-feeding".into(),
            EpisodeKind::Control(VoxelControl::Cruise) => "cruise".into(),
            EpisodeKind::Control(VoxelControl::Heuristic) => "heuristic".into(),
        }
    }

    /// This driver's `weights_fnv1a` — FNV-1a 64 over its exact weights, the same digest
    /// [`crate::es::voxel::trainer`] stamps a checkpoint's provenance with
    /// ([`crate::es::voxel::store::VoxelPolicyFile`] carries the founder-**manifest**
    /// digest instead, which every centre trained for a lineage shares; this one tells
    /// two different trained centres for the same lineage apart). Zero for a diagnostic
    /// control, which has no weights to name — a fauna layer stores this beside
    /// [`crate::Fauna::policy_driven`] so a loader can refuse a saved policy-driven world
    /// handed a *different* centre for the same lineage, not only a bare demotion.
    pub fn digest(&self) -> u64 {
        match &self.kind {
            EpisodeKind::Gru(EpisodeGru::Blind(policy)) => {
                weights_fnv1a(&tensor::flatten_shape(&policy.weights))
            }
            EpisodeKind::Gru(EpisodeGru::Browser(policy)) => {
                weights_fnv1a(&tensor::flatten_shape(&policy.weights))
            }
            EpisodeKind::Control(_) => 0,
        }
    }
}

/// FNV-1a 64 over the weights' little-endian hex — the same bytes
/// [`crate::es::voxel::trainer`]'s own `fnv1a_hex` and [`crate::es::fixture::fnv1a`] hash,
/// so a driver built from a [`crate::es::voxel::store::VoxelPolicyFile`]'s `theta` and a
/// checkpoint's `weights_fnv1a` provenance name the same centre the same way.
fn weights_fnv1a(theta: &[f64]) -> u64 {
    crate::es::fixture::fnv1a(crate::es::bits::encode(theta).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel_fauna::Actions;

    /// The GRU body answers raw logits — resolution is the fauna's shared adapter, not
    /// this crate's — and reset() clears its memory.
    #[test]
    fn the_gru_returns_logits_and_reset_clears_its_memory() {
        let theta = tensor::initial_center_shape::<23, 3>(11);
        let mut run =
            GruPolicy::<23>::new(&theta, voxel_schema_digest(Founder::Blind)).expect("policy");
        let mut fresh =
            GruPolicy::<23>::new(&theta, voxel_schema_digest(Founder::Blind)).expect("policy");
        let obs = [0.25; 23];
        let Response::Logits(first) = run.drive(&obs) else {
            panic!("the GRU answers logits");
        };
        for v in first {
            assert!(v.is_finite(), "logits are raw and finite");
        }
        // Memory is its own: the same observation twice evolves the hidden state.
        let Response::Logits(second) = run.drive(&obs) else {
            panic!("logits");
        };
        assert_ne!(
            first, second,
            "the GRU is recurrent on its own hidden state"
        );
        // Evolve far from the fresh state, reset, and compare against a fresh body.
        for _ in 0..12 {
            let _ = run.drive(&obs);
        }
        run.reset();
        assert_eq!(
            run.drive(&obs),
            fresh.drive(&obs),
            "a reset controller must act as a fresh one"
        );
    }

    /// The two disclosed controls are bounded responses, deadbanded by the fauna's own
    /// adapter into rest or full feed.
    #[test]
    fn the_controls_answer_bounded_rest_and_full_feed() {
        let manifest = Founder::Blind.manifest();
        let mut no = NoIntake;
        let resolved = cubarium_voxel_fauna::resolve_actions(no.drive(&[0.5; 23]), &manifest);
        assert_eq!(resolved, Actions::REST);
        let mut feed = StationaryFeeding;
        let resolved = cubarium_voxel_fauna::resolve_actions(feed.drive(&[0.5; 23]), &manifest);
        assert_eq!(
            resolved,
            Actions {
                forward: 0.0,
                turn: 0.0,
                feed: 1.0
            }
        );
        no.reset();
        feed.reset();
    }

    /// The open-loop cruise control reads nothing and resolves to half forward, no
    /// turn, full feed — the same bounded response whatever it is shown.
    #[test]
    fn the_cruise_control_is_open_loop() {
        let manifest = Founder::Browser.manifest();
        let mut cruise = Cruise;
        let a = cubarium_voxel_fauna::resolve_actions(cruise.drive(&[0.0; 37]), &manifest);
        let b = cubarium_voxel_fauna::resolve_actions(cruise.drive(&[0.9; 37]), &manifest);
        assert_eq!(a, b, "cruise cannot depend on the observation");
        assert_eq!(
            a,
            Actions {
                forward: CRUISE_FORWARD,
                turn: 0.0,
                feed: 1.0
            }
        );
        cruise.reset();
        assert_eq!(
            EpisodeDriver::control(VoxelControl::Cruise, Founder::Browser).name(),
            "cruise"
        );
    }

    /// The ablation zeroes the sense channels and nothing else: two observations that
    /// differ only from index 8 on are indistinguishable to an ablated policy, while the
    /// same policy unablated tells them apart. The `Self` block still drives it.
    #[test]
    fn ablating_the_senses_hides_only_the_sense_channels() {
        let theta = tensor::initial_center_shape::<23, 3>(21);
        let digest = voxel_schema_digest(Founder::Blind);
        let mut senses_a = [0.3; 23];
        let mut senses_b = [0.3; 23];
        for i in SELF_CHANNELS..23 {
            senses_a[i] = 0.9;
            senses_b[i] = 0.05;
        }

        let mut open = GruPolicy::<23>::new(&theta, digest).expect("policy");
        let mut open2 = GruPolicy::<23>::new(&theta, digest).expect("policy");
        assert_ne!(
            open.drive(&senses_a),
            open2.drive(&senses_b),
            "an unablated policy sees its senses"
        );

        let mut blind_a = GruPolicy::<23>::new_ablated(&theta, digest).expect("policy");
        let mut blind_b = GruPolicy::<23>::new_ablated(&theta, digest).expect("policy");
        assert!(blind_a.senses_ablated());
        assert_eq!(
            blind_a.drive(&senses_a),
            blind_b.drive(&senses_b),
            "an ablated policy cannot tell two sense vectors apart"
        );

        // Not a lobotomy: the Self block still moves the output, and a zero-sense
        // observation is unchanged by the ablation.
        let mut self_hi = [0.0; 23];
        self_hi[..SELF_CHANNELS].fill(0.8);
        let mut blind_c = GruPolicy::<23>::new_ablated(&theta, digest).expect("policy");
        let mut open3 = GruPolicy::<23>::new(&theta, digest).expect("policy");
        assert_ne!(
            blind_c.drive(&self_hi),
            blind_b.drive(&senses_b),
            "the Self block still reaches an ablated policy"
        );
        let mut blind_d = GruPolicy::<23>::new_ablated(&theta, digest).expect("policy");
        assert_eq!(
            blind_d.drive(&self_hi),
            open3.drive(&self_hi),
            "with no sense signal there is nothing to ablate"
        );
    }

    /// The driver-level ablation switch: it renames the driver, it survives `fresh()`,
    /// and it is a no-op on a control that reads no observation.
    #[test]
    fn the_driver_carries_the_ablation_into_every_fresh_body() {
        let theta = tensor::initial_center_shape::<23, 3>(31);
        let plain = EpisodeDriver::gru(&theta, Founder::Blind).expect("policy");
        let ablated = plain.clone().with_ablated_senses();
        assert_eq!(plain.name(), "gru");
        assert_eq!(ablated.name(), "gru-ablated");
        assert!(!plain.senses_ablated() && ablated.senses_ablated());

        let mut obs = [0.2; 23];
        obs[SELF_CHANNELS..].fill(0.95);
        let mut a = plain.fresh();
        let mut b = ablated.fresh();
        assert_ne!(a.drive(&obs), b.drive(&obs), "fresh() carries the ablation");

        let control =
            EpisodeDriver::control(VoxelControl::Heuristic, Founder::Blind).with_ablated_senses();
        assert!(
            !control.senses_ablated(),
            "a control reads no observation, so there is nothing to ablate"
        );
        assert_eq!(control.name(), "heuristic");
    }

    /// A driver hands out fresh memory per episode: two `fresh()` bodies are equal at
    /// their first drive, whatever the prototype has already done.
    #[test]
    fn fresh_hands_out_fresh_memory_per_episode() {
        let driver = EpisodeDriver::gru(&tensor::initial_center_shape::<23, 3>(13), Founder::Blind)
            .expect("policy");
        let mut a = driver.fresh();
        let mut b = driver.fresh();
        let obs = [0.1; 23];
        // Evolve `a` far from fresh.
        for _ in 0..9 {
            let _ = a.drive(&obs);
        }
        let mut a = driver.fresh();
        assert_eq!(a.drive(&obs), b.drive(&obs), "fresh is fresh, twice");
    }

    /// Shape acceptance and incompatible-policy rejection at the digest boundary.
    #[test]
    fn the_two_shapes_are_accepted_and_the_mismatched_policy_is_refused() {
        let blind = tensor::initial_center_shape::<23, 3>(7);
        let browser = tensor::initial_center_shape::<37, 3>(7);
        let browser_digest = voxel_schema_digest(Founder::Browser);

        let d = EpisodeDriver::gru(&blind, Founder::Blind).expect("blind accepts");
        assert_eq!(d.founder(), Founder::Blind);
        let d = EpisodeDriver::gru(&browser, Founder::Browser).expect("browser accepts");
        assert_eq!(d.founder(), Founder::Browser);

        // A blind-shaped theta refused as a browser policy: wrong length, named.
        let err = EpisodeDriver::gru(&blind, Founder::Browser).expect_err("wrong length");
        assert!(err.contains("5571"), "{err}");

        // A right-length policy stamped with a corrupted digest is refused by the
        // boundary the world uses: `ShapePolicy::new` (the authoring stamp) against
        // `validate` (the expected digest).
        let claimed = ShapePolicy::new(
            tensor::unflatten_shape::<37, 3>(&browser).expect("shape"),
            browser_digest ^ 1,
        );
        let err = claimed.validate(browser_digest).expect_err("digest");
        assert!(err.contains("schema_digest"), "{err}");
    }
}
