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
    Actions, BrowserForager, Controller, Founder, Response, TeacherConfig, resolve_actions,
};

use super::super::tensor;
#[cfg(test)]
use super::voxel_schema_digest;
use super::{BLIND_I, BLIND_O, BROWSER_I, BROWSER_O};

/// How many leading observation channels are the founder's own v2 `Self` block (contract
/// v3's `SelfV3` block sits after the v2 layout and counts as a sense here). The two
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
pub struct GruPolicy<const I: usize, const O: usize> {
    policy: ShapePolicy<I, O>,
    hidden: [f64; HIDDEN],
    ablate_senses: bool,
}

impl<const I: usize, const O: usize> GruPolicy<I, O> {
    /// Build a controller from a flat parameter vector, validated against the founder's
    /// manifest digest — the [`tensor::shape_policy`] boundary with the digest plug-in
    /// ([`super::voxel_schema_digest`]) filled in.
    ///
    /// Refuses a wrong-length vector, a non-finite value, and a digest mismatch by name.
    pub fn new(theta: &[f64], digest: u64) -> Result<GruPolicy<I, O>, String> {
        Ok(GruPolicy {
            policy: tensor::shape_policy::<I, O>(theta, digest)?,
            hidden: [0.0; HIDDEN],
            ablate_senses: false,
        })
    }

    /// The same policy with every sense channel zeroed before it is read
    /// (see [`SELF_CHANNELS`]).
    pub fn new_ablated(theta: &[f64], digest: u64) -> Result<GruPolicy<I, O>, String> {
        let mut p = GruPolicy::new(theta, digest)?;
        p.ablate_senses = true;
        Ok(p)
    }

    /// Whether this body reads its senses.
    pub fn senses_ablated(&self) -> bool {
        self.ablate_senses
    }

    /// The validated policy this controller runs.
    pub fn policy(&self) -> &ShapePolicy<I, O> {
        &self.policy
    }
}

impl<const I: usize, const O: usize> Controller for GruPolicy<I, O> {
    fn drive(&mut self, observation: &[f64]) -> Response {
        let mut x = [0.0; I];
        x.copy_from_slice(observation);
        if self.ablate_senses {
            x[SELF_CHANNELS.min(I)..].fill(0.0);
        }
        let logits = self.policy.weights.forward(&x, &mut self.hidden);
        Response::logits(&logits)
    }

    fn reset(&mut self) {
        self.hidden = [0.0; HIDDEN];
    }
}

/// The blind founder's GRU runtime.
pub type GruBlind = GruPolicy<BLIND_I, BLIND_O>;
/// The browser's GRU runtime.
pub type GruBrowser = GruPolicy<BROWSER_I, BROWSER_O>;

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

/// **The heuristic parker** (`design/handoffs/voxel-audit-round-2026-09-28.md` §2): the
/// fauna's own observation-only forager for the body's founder, except that **whenever its
/// feed gate is open — the mouth tastes food above the forager's own gate — it stands
/// still** (forward and turn zero) and feeds. When what is in reach no longer tastes like
/// food it walks again, exactly as the forager would. The walker and the parker therefore
/// differ in one thing: whether a body keeps moving while it eats. Neither reads a `Self`
/// channel, so both are mass-invariant.
pub struct Parker {
    inner: Box<dyn Controller>,
}

impl Parker {
    /// The parker for `founder`, its forager on `teachers` (the world's
    /// [`cubarium_voxel_fauna::FaunaConfig::teachers`]).
    pub fn new(founder: Founder, teachers: TeacherConfig) -> Parker {
        Parker {
            inner: cubarium_voxel_fauna::heuristic(founder, teachers),
        }
    }
}

impl Controller for Parker {
    fn drive(&mut self, o: &[f64]) -> Response {
        match self.inner.drive(o) {
            Response::Bounded(mut a) => {
                if a.feed > 0.5 {
                    a.forward = 0.0;
                    a.turn = 0.0;
                }
                Response::Bounded(a)
            }
            other => other,
        }
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}

/// **Herding, a prototype arm** (`design/handoffs/voxel-browser-movement-2026-09-28.md`,
/// 3): the fauna's own [`BrowserForager`], plus cohesion toward the bodies its cone
/// already sees and a little separation from any too close. Observation-only — the same
/// `Cone(3, foliage/body)` channels the trained browser reads, no sense or contract
/// change. The cone's `body` class does not tell lineages apart, so a shredder in view
/// draws a herder too.
///
/// Food first: whenever foliage is in view, or the forager is feeding or escaping, its
/// answer stands. Otherwise, a body closer than [`HERD_SEPARATION`] of the cone's range
/// turns the herder away from it, and any other body in view turns it toward the sector
/// that holds the most.
#[derive(Clone, Debug, Default)]
pub struct HerdingBrowser {
    inner: BrowserForager,
    cone: usize,
}

/// A body's cone proximity above this (within a quarter of the 2 m range, 0.5 m) is too
/// close: turn away. A prototype placeholder.
pub const HERD_SEPARATION: f64 = 0.75;
/// A sector must show at least this share of body rays to draw the herder.
pub const HERD_SEEN: f64 = 0.02;
/// The turn effort a herding decision asks for, as the forager's gaze does.
pub const HERD_TURN: f64 = 0.8;

impl HerdingBrowser {
    /// The herder, its forager on `teachers` (the world's
    /// [`cubarium_voxel_fauna::FaunaConfig::teachers`]).
    pub fn new(teachers: TeacherConfig) -> HerdingBrowser {
        let cone = Founder::Browser
            .manifest()
            .modules
            .iter()
            .find(|m| m.name == "Cone(3, foliage/body)")
            .expect("the browser manifest has its cone")
            .offset;
        HerdingBrowser {
            inner: BrowserForager::with_config(teachers),
            cone,
        }
    }
}

impl Controller for HerdingBrowser {
    fn drive(&mut self, o: &[f64]) -> Response {
        let answer = self.inner.drive(o);
        let Response::Bounded(mut a) = answer else {
            return answer;
        };
        let c = self.cone;
        let foliage = (0..3).any(|k| o[c + 6 * k + 2] > 0.02);
        // Feeding, food in view, or an escape (anything short of full-effort walking):
        // the forager's answer stands.
        if a.feed > 0.0 || foliage || a.forward < 0.99 {
            return Response::Bounded(a);
        }
        let body = [o[c + 4], o[c + 10], o[c + 16]];
        let near = [o[c + 5], o[c + 11], o[c + 17]];
        let side = |k: usize| match k {
            0 => -HERD_TURN,
            2 => HERD_TURN,
            _ => 0.0,
        };
        // Separation: the closest body in view, if it is too close.
        if let Some(k) = (0..3)
            .filter(|&k| body[k] > HERD_SEEN && near[k] > HERD_SEPARATION)
            .max_by(|&x, &y| near[x].total_cmp(&near[y]))
        {
            a.turn = match k {
                1 => HERD_TURN,
                k => -side(k),
            };
            return Response::Bounded(a);
        }
        // Cohesion: toward the sector with the most bodies in it.
        if let Some(k) = (0..3)
            .filter(|&k| body[k] > HERD_SEEN)
            .max_by(|&x, &y| body[x].total_cmp(&body[y]))
        {
            a.turn = side(k);
        }
        Response::Bounded(a)
    }

    fn reset(&mut self) {
        self.inner.reset();
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
    tap: Option<Arc<Mutex<Option<cubarium_voxel_fauna::ActionTap>>>>,
}

/// The controller body a driver runs.
#[derive(Clone, Debug)]
pub enum EpisodeKind {
    /// The seeded or perturbed GRU centre, by shape.
    Gru(Arc<EpisodeGru>),
    /// The disclosed diagnostic controls and the observation-only heuristic.
    Control(VoxelControl),
    /// A menu lineage's two-layer brain (TWO-LAYER): its validated weights and the
    /// lineage's holding knobs (`fauna.founders.<lineage>.brain`).
    TwoLayer(Arc<super::two_layer::TwoLayerNet>, super::two_layer::IntentArbitration),
}

/// One lineage's manifest-shaped GRU and two linear heads on the same hidden state.
/// The flat layout stores the action rows first, then the option rows (zero for B).
#[derive(Clone, Debug)]
pub struct EpisodeGru {
    inputs: usize,
    actions: usize,
    options: usize,
    offsets: tensor::ShapeOffsets,
    schema_digest: u64,
    theta: Vec<f64>,
}

impl EpisodeGru {
    pub fn from_manifest(
        theta: &[f64],
        manifest: &cubarium_voxel_fauna::Manifest,
    ) -> Result<Self, String> {
        let offsets =
            tensor::shape_offsets_hidden(manifest.inputs(), manifest.outputs(), manifest.hidden);
        if theta.len() != offsets.params {
            return Err(format!(
                "{}: theta has {} values, expected {} for I={} actions={} options={} H={}",
                manifest.founder.name(),
                theta.len(),
                offsets.params,
                manifest.inputs(),
                manifest.actions.len(),
                manifest.option_head,
                manifest.hidden
            ));
        }
        if manifest.actions.len() > 5 || manifest.outputs() > cubarium_voxel_fauna::MAX_OUTPUTS {
            return Err(format!(
                "{}: too many manifest outputs",
                manifest.founder.name()
            ));
        }
        tensor::gru_view(theta, &offsets)
            .validate(manifest.inputs(), manifest.outputs(), manifest.hidden)
            .map_err(|e| format!("{}: {e}", manifest.founder.name()))?;
        Ok(Self {
            inputs: manifest.inputs(),
            actions: manifest.actions.len(),
            options: manifest.option_head,
            offsets,
            schema_digest: manifest.digest(),
            theta: theta.to_vec(),
        })
    }
    pub fn theta(&self) -> &[f64] {
        &self.theta
    }
    pub fn schema_digest(&self) -> u64 {
        self.schema_digest
    }
    pub fn controller(self: &Arc<Self>, ablate_senses: bool) -> RuntimeGruPolicy {
        RuntimeGruPolicy {
            policy: Arc::clone(self),
            hidden: vec![0.; self.offsets.hidden],
            next: vec![0.; self.offsets.hidden],
            input: vec![0.; self.inputs],
            logits: vec![0.; self.actions + self.options],
            ablate_senses,
        }
    }
}

/// Per-body recurrent state; weights are shared across a live-training variant table.
pub struct RuntimeGruPolicy {
    policy: Arc<EpisodeGru>,
    hidden: Vec<f64>,
    next: Vec<f64>,
    input: Vec<f64>,
    logits: Vec<f64>,
    ablate_senses: bool,
}
impl Controller for RuntimeGruPolicy {
    fn drive(&mut self, observation: &[f64]) -> Response {
        self.input.copy_from_slice(observation);
        if self.ablate_senses {
            self.input[SELF_CHANNELS.min(self.policy.inputs)..].fill(0.);
        }
        tensor::gru_view(&self.policy.theta, &self.policy.offsets).forward(
            &self.input,
            &mut self.hidden,
            &mut self.next,
            &mut self.logits,
        );
        Response::logits(&self.logits)
    }
    fn reset(&mut self) {
        self.hidden.fill(0.);
    }
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
    /// The browser's heuristic plus herding ([`HerdingBrowser`]): a prototype arm of
    /// `design/handoffs/voxel-browser-movement-2026-09-28.md`, browser only.
    Herding,
    /// The heuristic that stands still while it feeds ([`Parker`]), either founder.
    Parker,
}

impl EpisodeDriver {
    /// A GRU driver from a flat parameter vector, for `founder`, validated against that
    /// founder's manifest digest.
    pub fn gru(theta: &[f64], founder: Founder) -> Result<EpisodeDriver, String> {
        let gru = EpisodeGru::from_manifest(theta, &founder.manifest())?;
        Ok(EpisodeDriver {
            founder,
            kind: EpisodeKind::Gru(Arc::new(gru)),
            ablate_senses: false,
            record: None,
            tap: None,
        })
    }

    /// A two-layer brain driver for `founder` (TWO-LAYER): weights validated against its
    /// manifest by the policy file, holding by `arbitration`.
    pub fn two_layer(
        net: Arc<super::two_layer::TwoLayerNet>,
        arbitration: super::two_layer::IntentArbitration,
        founder: Founder,
    ) -> EpisodeDriver {
        EpisodeDriver {
            founder,
            kind: EpisodeKind::TwoLayer(net, arbitration),
            ablate_senses: false,
            record: None,
            tap: None,
        }
    }

    /// The same driver holding by the world's own brain table for its lineage (a no-op for
    /// anything but a two-layer brain): what [`super::live::install_centres`] calls, so a
    /// world's `fauna.founders.<lineage>.brain` knobs reach the brains it installs.
    pub fn with_brain_table(mut self, arbitration: super::two_layer::IntentArbitration) -> EpisodeDriver {
        if let EpisodeKind::TwoLayer(_, a) = &mut self.kind {
            *a = arbitration;
        }
        self
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
            tap: None,
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

    /// Record the body tap, including executed body actions and steering sources.
    pub fn tapping(mut self, tap: Arc<Mutex<Option<cubarium_voxel_fauna::ActionTap>>>) -> Self {
        self.tap = Some(tap);
        self
    }
    pub(crate) fn start_tap(&self, fauna: &mut cubarium_voxel_fauna::Fauna) {
        if self.tap.is_some() {
            fauna.start_tap();
        }
    }
    pub(crate) fn take_tap(&self, fauna: &mut cubarium_voxel_fauna::Fauna) {
        if let Some(sink) = &self.tap {
            *sink.lock().expect("tap sink") = fauna.take_tap();
        }
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
    ///
    /// `teachers` is the reflexes' numbers of the world the controller will drive in (its
    /// [`cubarium_voxel_fauna::FaunaConfig::teachers`]); a GRU or a diagnostic control that
    /// runs no reflex ignores it.
    pub fn fresh(&self, teachers: TeacherConfig) -> Box<dyn Controller> {
        let mut controller: Box<dyn Controller> = match &self.kind {
            EpisodeKind::Gru(policy) => Box::new(policy.controller(self.ablate_senses)),
            EpisodeKind::TwoLayer(net, arbitration) => Box::new(
                net.brain(*arbitration, self.founder.manifest().controller_period_s),
            ),
            EpisodeKind::Control(VoxelControl::NoIntake) => Box::new(NoIntake),
            EpisodeKind::Control(VoxelControl::StationaryFeeding) => Box::new(StationaryFeeding),
            EpisodeKind::Control(VoxelControl::Cruise) => Box::new(Cruise),
            // The heuristic slot is the fauna's own observation-only heuristics: the
            // body the driver runs decides which one fits its founder.
            EpisodeKind::Control(VoxelControl::Heuristic) => {
                cubarium_voxel_fauna::heuristic(self.founder, teachers)
            }
            // The herding variant is the browser's; every other lineage herds as its
            // own heuristic.
            EpisodeKind::Control(VoxelControl::Herding) => match self.founder {
                Founder::Browser => Box::new(HerdingBrowser::new(teachers)),
                other => cubarium_voxel_fauna::heuristic(other, teachers),
            },
            EpisodeKind::Control(VoxelControl::Parker) => {
                Box::new(Parker::new(self.founder, teachers))
            }
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
            EpisodeKind::TwoLayer(..) => "two-layer".into(),
            EpisodeKind::Control(VoxelControl::NoIntake) => "no-intake".into(),
            EpisodeKind::Control(VoxelControl::StationaryFeeding) => "stationary-feeding".into(),
            EpisodeKind::Control(VoxelControl::Cruise) => "cruise".into(),
            EpisodeKind::Control(VoxelControl::Heuristic) => "heuristic".into(),
            EpisodeKind::Control(VoxelControl::Herding) => "herding".into(),
            EpisodeKind::Control(VoxelControl::Parker) => "parker".into(),
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
            EpisodeKind::Gru(policy) => weights_fnv1a(policy.theta()),
            EpisodeKind::TwoLayer(net, _) => weights_fnv1a(net.theta()),
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
        let theta = tensor::initial_center_shape::<BLIND_I, BLIND_O>(11);
        let mut run =
            GruPolicy::<BLIND_I, BLIND_O>::new(&theta, voxel_schema_digest(Founder::Blind))
                .expect("policy");
        let mut fresh =
            GruPolicy::<BLIND_I, BLIND_O>::new(&theta, voxel_schema_digest(Founder::Blind))
                .expect("policy");
        let obs = [0.25; BLIND_I];
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
        let resolved = cubarium_voxel_fauna::resolve_actions(no.drive(&[0.5; BLIND_I]), &manifest);
        assert_eq!(resolved, Actions::REST);
        let mut feed = StationaryFeeding;
        let resolved =
            cubarium_voxel_fauna::resolve_actions(feed.drive(&[0.5; BLIND_I]), &manifest);
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
        let a = cubarium_voxel_fauna::resolve_actions(cruise.drive(&[0.0; BROWSER_I]), &manifest);
        let b = cubarium_voxel_fauna::resolve_actions(cruise.drive(&[0.9; BROWSER_I]), &manifest);
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
        let theta = tensor::initial_center_shape::<BLIND_I, BLIND_O>(21);
        let digest = voxel_schema_digest(Founder::Blind);
        let mut senses_a = [0.3; BLIND_I];
        let mut senses_b = [0.3; BLIND_I];
        for i in SELF_CHANNELS..BLIND_I {
            senses_a[i] = 0.9;
            senses_b[i] = 0.05;
        }

        let mut open = GruPolicy::<BLIND_I, BLIND_O>::new(&theta, digest).expect("policy");
        let mut open2 = GruPolicy::<BLIND_I, BLIND_O>::new(&theta, digest).expect("policy");
        assert_ne!(
            open.drive(&senses_a),
            open2.drive(&senses_b),
            "an unablated policy sees its senses"
        );

        let mut blind_a =
            GruPolicy::<BLIND_I, BLIND_O>::new_ablated(&theta, digest).expect("policy");
        let mut blind_b =
            GruPolicy::<BLIND_I, BLIND_O>::new_ablated(&theta, digest).expect("policy");
        assert!(blind_a.senses_ablated());
        assert_eq!(
            blind_a.drive(&senses_a),
            blind_b.drive(&senses_b),
            "an ablated policy cannot tell two sense vectors apart"
        );

        // Not a lobotomy: the Self block still moves the output, and a zero-sense
        // observation is unchanged by the ablation.
        let mut self_hi = [0.0; BLIND_I];
        self_hi[..SELF_CHANNELS].fill(0.8);
        let mut blind_c =
            GruPolicy::<BLIND_I, BLIND_O>::new_ablated(&theta, digest).expect("policy");
        let mut open3 = GruPolicy::<BLIND_I, BLIND_O>::new(&theta, digest).expect("policy");
        assert_ne!(
            blind_c.drive(&self_hi),
            blind_b.drive(&senses_b),
            "the Self block still reaches an ablated policy"
        );
        let mut blind_d =
            GruPolicy::<BLIND_I, BLIND_O>::new_ablated(&theta, digest).expect("policy");
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
        let theta = tensor::initial_center_shape::<BLIND_I, BLIND_O>(31);
        let plain = EpisodeDriver::gru(&theta, Founder::Blind).expect("policy");
        let ablated = plain.clone().with_ablated_senses();
        assert_eq!(plain.name(), "gru");
        assert_eq!(ablated.name(), "gru-ablated");
        assert!(!plain.senses_ablated() && ablated.senses_ablated());

        let mut obs = [0.2; BLIND_I];
        obs[SELF_CHANNELS..].fill(0.95);
        let mut a = plain.fresh(TeacherConfig::default());
        let mut b = ablated.fresh(TeacherConfig::default());
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
        let driver = EpisodeDriver::gru(
            &tensor::initial_center_shape::<BLIND_I, BLIND_O>(13),
            Founder::Blind,
        )
        .expect("policy");
        let mut a = driver.fresh(TeacherConfig::default());
        let mut b = driver.fresh(TeacherConfig::default());
        let obs = [0.1; BLIND_I];
        // Evolve `a` far from fresh.
        for _ in 0..9 {
            let _ = a.drive(&obs);
        }
        let mut a = driver.fresh(TeacherConfig::default());
        assert_eq!(a.drive(&obs), b.drive(&obs), "fresh is fresh, twice");
    }

    /// Shape acceptance and incompatible-policy rejection at the digest boundary.
    #[test]
    fn the_two_shapes_are_accepted_and_the_mismatched_policy_is_refused() {
        let blind = tensor::initial_center_shape::<BLIND_I, BLIND_O>(7);
        let browser = tensor::initial_center_shape::<BROWSER_I, BROWSER_O>(7);
        let browser_digest = voxel_schema_digest(Founder::Browser);

        let d = EpisodeDriver::gru(&blind, Founder::Blind).expect("blind accepts");
        assert_eq!(d.founder(), Founder::Blind);
        let d = EpisodeDriver::gru(&browser, Founder::Browser).expect("browser accepts");
        assert_eq!(d.founder(), Founder::Browser);

        // A blind-shaped theta refused as a browser policy: wrong length, named.
        let err = EpisodeDriver::gru(&blind, Founder::Browser).expect_err("wrong length");
        assert!(
            err.contains(&Founder::Blind.manifest().parameter_count().to_string()),
            "{err}"
        );

        // A right-length policy stamped with a corrupted digest is refused by the
        // boundary the world uses: `ShapePolicy::new` (the authoring stamp) against
        // `validate` (the expected digest).
        let claimed = ShapePolicy::new(
            tensor::unflatten_shape::<BROWSER_I, BROWSER_O>(&browser).expect("shape"),
            browser_digest ^ 1,
        );
        let err = claimed.validate(browser_digest).expect_err("digest");
        assert!(err.contains("schema_digest"), "{err}");
    }
    #[test]
    fn every_founder_builds_its_manifest_shape_and_resolves_all_body_actions() {
        for founder in Founder::ALL {
            let m = founder.manifest();
            let want = 3 * m.hidden * (m.inputs() + m.hidden + 2) + m.outputs() * (m.hidden + 1);
            assert_eq!(m.parameter_count(), want);
            let mut theta = tensor::initial_center_runtime(m.inputs(), m.outputs(), m.hidden, 7);
            assert_eq!(theta.len(), want);
            eprintln!(
                "shape {}: {} -> {}+{}, H={}, params={}",
                founder.name(),
                m.inputs(),
                m.actions.len(),
                m.option_head,
                m.hidden,
                want
            );
            let offsets = tensor::shape_offsets_hidden(m.inputs(), m.outputs(), m.hidden);
            theta.fill(0.);
            for k in 0..m.outputs() {
                theta[offsets.b_o + k] = if k == 1 { -1. } else { 1. };
            }
            let d = EpisodeDriver::gru(&theta, founder).expect("all nine shapes");
            let EpisodeKind::Gru(g) = d.kind() else {
                panic!("GRU");
            };
            assert_eq!(g.theta().len(), want);
            assert_eq!(g.schema_digest(), m.digest());
            let mut c = d.fresh(TeacherConfig::default());
            let r = c.drive(&vec![0.; m.inputs()]);
            let expected: Vec<_> = (0..m.outputs())
                .map(|k| if k == 1 { -1. } else { 1. })
                .collect();
            assert_eq!(r, Response::logits(&expected));
            let resolved = cubarium_voxel_fauna::resolve(r, &m);
            for action in m.actions {
                let value = match action.name {
                    "forward_effort" => resolved.actions.forward,
                    "turn_effort" => -resolved.actions.turn,
                    "feed_effort" => resolved.actions.feed,
                    "sprint_effort" => resolved.body.sprint,
                    "hunker_effort" => resolved.body.hunker,
                    "retract_effort" => resolved.body.retract,
                    "lure_effort" => resolved.body.lure,
                    "lift_effort" => resolved.body.lift,
                    name => panic!("unhandled {name}"),
                };
                assert!(value > 0., "{} {} is resolved", founder.name(), action.name);
            }
        }
    }

    #[test]
    fn a_manifest_can_change_hidden_width_and_add_an_independent_option_head() {
        for founder in [
            Founder::Loftstrider,
            Founder::Seedporter,
            Founder::Chorister,
        ] {
            let mut m = founder.manifest();
            m.hidden += 7;
            m.option_head = 3;
            let mut theta = tensor::initial_center_runtime(m.inputs(), m.outputs(), m.hidden, 5);
            assert_eq!(theta.len(), m.parameter_count());
            theta.fill(0.);
            let o = tensor::shape_offsets_hidden(m.inputs(), m.outputs(), m.hidden);
            theta[o.b_o + m.actions.len() + 2] = 5.;
            let p = Arc::new(EpisodeGru::from_manifest(&theta, &m).expect("runtime dimensions"));
            let mut c = p.controller(false);
            let r = cubarium_voxel_fauna::resolve(c.drive(&vec![0.; m.inputs()]), &m);
            assert_eq!(r.option, Some(2));
            assert!(EpisodeGru::from_manifest(&theta, &founder.manifest()).is_err());
        }
    }
}
