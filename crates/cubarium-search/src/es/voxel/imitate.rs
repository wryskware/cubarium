//! **The imitation seed** (package P3-C step 1–2): record what the fauna's own foraging
//! heuristic does on the training layouts, then fit a GRU to it so a search can start
//! from an animal that already wanders instead of one that has to discover wandering.
//!
//! Integration note 5 measured the shape of the problem: warm-starting Stage B from a
//! Stage-A centre is what produced the browser's first reacquisition, and the blind
//! founder's remaining failure is that it never *leaves* after its patch runs out. The
//! heuristics already leave — `BlindForager` and `BrowserForager` fall back to an
//! alternating turn preference when no cue is present — so this module makes that
//! behaviour a starting point for the search rather than something the search must
//! rediscover under a motor penalty.
//!
//! Two halves, both here:
//!
//! - **Recording.** [`record_streams`] runs the heuristic through the ordinary episode
//!   driver with [`super::controller::RecordingController`] attached, so the episode is
//!   the real one and the pairs are the real observations and the real *adapted* actions
//!   (the fauna's own `resolve_actions`). One [`TeacherStream`] per acting body per
//!   fixture ([`record_fixtures`]; a landscape has one body per placed founder).
//! - **Fitting.** [`fit`] minimises the squared error between the GRU's adapted actions
//!   and the teacher's over those streams, **teacher-forced**: the network reads the
//!   recorded observation at every step and never the simulation, so no episode runs per
//!   candidate. The gradient is analytic (truncated BPTT), Adam descends it, and
//!   [`gradient_of_chunk`] is checked against finite differences on a tiny shape in the
//!   tests below.
//!
//! # What is and is not in a stream
//!
//! A stream holds the observation vector the body sampled and the three bounded actions
//! the world then held. There is no site, no distance, no stock, no patch identity and no
//! layout geometry: a clone fitted on these has been told exactly what a policy in that
//! slot is told. Streams come from the **training** layouts only; the held-out seeds are
//! never recorded, never fitted and never selected on.
//!
//! # The objective, exactly
//!
//! The teacher target is the adapted action: `resolve_actions(Bounded(heuristic), …)`,
//! transfer and 0.05 deadband applied, which is what the body held. The network's action
//! is `resolve_actions(Logits(y), …)` — the manifest's sigmoid/tanh transfer, then the
//! same deadband. The **reported** MSE is over those adapted values.
//!
//! The **descended** objective drops the deadband: below it the adapted action is a flat
//! zero with no gradient at all, so a network whose transferred output sits at 0.03 while
//! the teacher asks for 0.6 would receive nothing to learn from. The optimiser therefore
//! differentiates `transfer(y)` (a straight-through deadband), and the finite-difference
//! test checks exactly that function. The two differ only inside a 0.05 band.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use cubarium_core::neural::gru::{GATES, HIDDEN, N, R, Z};
use cubarium_voxel_fauna::{Founder, Manifest, Transfer};
use serde::{Deserialize, Serialize};

use super::controller::{EpisodeDriver, VoxelControl, teacher_sink};
use super::landscape::{LANDSCAPE_HORIZON_TICKS, LANDSCAPE_PROTOCOL};
use super::task::{self, Band, Prepared, Stage};
use super::{driver, voxel_schema_digest};
use crate::es::bits::hex_f64s;
use crate::es::optimizer::Adam;
use crate::es::tensor::{self, ShapeOffsets, shape_offsets};

/// The teacher-stream file's schema token. A file that is not this is refused by name,
/// never reinterpreted.
pub const TEACHER_SCHEMA: &str = "cub-voxel-teacher-1";

/// The provenance a fitted clone records in its centre file.
pub const IMITATION_PROVENANCE: &str = "imitation-of-heuristic-1";

/// The teacher this seed imitates, named in the stream file so a stream cannot be
/// silently re-read as some other controller's.
pub const TEACHER_CONTROLLER: &str = "heuristic";

/// One episode's `(observation, adapted action)` pairs, as a self-contained file.
///
/// Floats are little-endian IEEE-754 hex ([`crate::es::bits`]), so a load is exact. The
/// founder, manifest digest and the three protocol strings are carried so that a stream
/// recorded against another schema or another arena is refused rather than fitted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TeacherStream {
    pub schema: String,
    pub build: String,
    /// The controller that produced the actions ([`TEACHER_CONTROLLER`]).
    pub teacher: String,
    pub founder: String,
    pub digest: u64,
    pub stage: String,
    pub band: String,
    pub arena_protocol: String,
    pub start_heading: String,
    pub starting_stores: String,
    pub layout_seed: u64,
    /// The fixture's label ([`task::Prepared::label`]): an arena's seed with its grid, or
    /// a landscape's `preset/base/water`. Empty in a stream written before P5-C.
    #[serde(default)]
    pub fixture: String,
    /// Which acting body of the episode this stream is, in the driver's body order. An
    /// arena has one; a landscape one per placed founder of the lineage (D7).
    #[serde(default)]
    pub body: usize,
    pub horizon: u64,
    /// Observation width, so a truncated file is refused on arithmetic rather than
    /// silently reshaped.
    pub inputs: usize,
    pub steps: usize,
    /// `steps × inputs`, row-major by step.
    #[serde(with = "hex_f64s")]
    pub observations: Vec<f64>,
    /// `steps × 3`, in manifest action order: forward, turn, feed.
    #[serde(with = "hex_f64s")]
    pub actions: Vec<f64>,
}

impl TeacherStream {
    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    /// Read and validate a stream for `founder` under this build's protocols.
    pub fn load(path: &Path, founder: Founder) -> Result<TeacherStream, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let stream: TeacherStream =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        stream.validate(&path.display().to_string(), founder)?;
        Ok(stream)
    }

    /// Schema token, teacher, founder, digest, the three protocol strings, and the
    /// arithmetic of its own lengths. Every failure names what differs.
    pub fn validate(&self, name: &str, founder: Founder) -> Result<(), String> {
        if self.schema != TEACHER_SCHEMA {
            return Err(format!(
                "{name}: schema `{}` is not {TEACHER_SCHEMA}",
                self.schema
            ));
        }
        if self.teacher != TEACHER_CONTROLLER {
            return Err(format!(
                "{name}: the stream was recorded from `{}`, not the `{TEACHER_CONTROLLER}` \
                 slot this seed imitates",
                self.teacher
            ));
        }
        let named = super::parse_founder(&self.founder).map_err(|e| format!("{name}: {e}"))?;
        if named != founder {
            return Err(format!(
                "{name}: the stream is {}'s and this fit is {}'s: the observation widths \
                 are different interfaces and cannot be reinterpreted",
                named.name(),
                founder.name()
            ));
        }
        let digest = voxel_schema_digest(founder);
        if self.digest != digest {
            return Err(format!(
                "{name}: stream digest {:#018x} is not this build's {} manifest digest \
                 {digest:#018x}: the observation layout differs",
                self.digest,
                founder.name()
            ));
        }
        if self.start_heading != task::START_HEADING_PROTOCOL {
            return Err(format!(
                "{name}: stream start-heading protocol is `{}`, this build uses `{}`",
                self.start_heading,
                task::START_HEADING_PROTOCOL
            ));
        }
        if self.starting_stores != task::STARTING_STORES_PROTOCOL {
            return Err(format!(
                "{name}: stream starting-stores protocol is `{}`, this build uses `{}`",
                self.starting_stores,
                task::STARTING_STORES_PROTOCOL
            ));
        }
        if self.stage == LANDSCAPE_STAGE {
            if self.arena_protocol != LANDSCAPE_PROTOCOL {
                return Err(format!(
                    "{name}: landscape stream protocol is `{}`, this build's landscapes \
                     are `{LANDSCAPE_PROTOCOL}`: the task differs",
                    self.arena_protocol
                ));
            }
        } else {
            let stage = task::parse_stage(&self.stage).map_err(|e| format!("{name}: {e}"))?;
            let band = task::parse_band(&self.band).map_err(|e| format!("{name}: {e}"))?;
            let want = task::arena_protocol(founder, stage, band);
            if self.arena_protocol != want {
                return Err(format!(
                    "{name}: stream arena protocol is `{}`, this build's stage {} band {} \
                     uses `{want}`: the task differs",
                    self.arena_protocol,
                    stage.as_str(),
                    band.as_str()
                ));
            }
        }
        let inputs = founder.manifest().inputs();
        if self.inputs != inputs {
            return Err(format!(
                "{name}: stream declares {} inputs, {} has {inputs}",
                self.inputs,
                founder.name()
            ));
        }
        if self.observations.len() != self.steps * inputs {
            return Err(format!(
                "{name}: {} observation values is not {} steps × {inputs} inputs",
                self.observations.len(),
                self.steps
            ));
        }
        if self.actions.len() != self.steps * 3 {
            return Err(format!(
                "{name}: {} action values is not {} steps × 3",
                self.actions.len(),
                self.steps
            ));
        }
        if self.steps == 0 {
            return Err(format!("{name}: the stream is empty"));
        }
        if let Some(i) = self
            .observations
            .iter()
            .chain(self.actions.iter())
            .position(|x| !x.is_finite())
        {
            return Err(format!("{name}: value {i} is not finite"));
        }
        Ok(())
    }

    /// One step's observation slice.
    pub fn observation(&self, step: usize) -> &[f64] {
        &self.observations[step * self.inputs..(step + 1) * self.inputs]
    }

    /// One step's teacher action, `[forward, turn, feed]`.
    pub fn action(&self, step: usize) -> &[f64] {
        &self.actions[step * 3..step * 3 + 3]
    }
}

/// A digest over a set of streams, for the clone's provenance: FNV-1a 64 over each
/// stream's exact hex bytes in the order given.
pub fn streams_digest(streams: &[TeacherStream]) -> u64 {
    let mut bytes = String::new();
    for s in streams {
        bytes.push_str(&s.stage);
        bytes.push_str(&s.band);
        bytes.push_str(&s.layout_seed.to_string());
        bytes.push_str(&s.fixture);
        bytes.push_str(&s.body.to_string());
        bytes.push_str(&crate::es::bits::encode(&s.observations));
        bytes.push_str(&crate::es::bits::encode(&s.actions));
    }
    crate::es::fixture::fnv1a(bytes.as_bytes())
}

/// The `stage` a landscape stream carries: a landscape is not an arena stage.
pub const LANDSCAPE_STAGE: &str = "landscape";

/// Record the heuristic on the training layouts of one `(stage, band)`, on the
/// standard grid: one stream per layout (an arena has one acting body).
pub fn record_streams(
    founder: Founder,
    stage: Stage,
    band: Band,
    layouts: usize,
    horizon: u64,
    workers: usize,
) -> Result<Vec<TeacherStream>, String> {
    let prepared = task::training_layouts(founder, stage, band);
    let prepared = &prepared[..layouts.min(prepared.len())];
    record_fixtures(founder, prepared, Some(horizon), workers)
}

/// Record the heuristic on any fixtures — arenas of either stage and grid, landscapes —
/// as **one stream per acting body** in fixture order and then body order.
///
/// The episode is the ordinary one: same prepared fixture, same driver, same fauna tick,
/// the body sizes the fixture's own seed draws. The only difference is the transparent
/// recording wrapper, which returns the teacher's response untouched — so a recorded
/// episode is an episode, not a simulation of one. `horizon` overrides every fixture's
/// own (an arena's stage horizon, a landscape's [`LANDSCAPE_HORIZON_TICKS`]). A body
/// that never sampled leaves no stream.
pub fn record_fixtures(
    founder: Founder,
    prepared: &[Prepared],
    horizon: Option<u64>,
    workers: usize,
) -> Result<Vec<TeacherStream>, String> {
    let inputs = founder.manifest().inputs();
    let cancel = AtomicBool::new(false);
    let slots: std::sync::Mutex<Vec<Option<Vec<TeacherStream>>>> =
        std::sync::Mutex::new(vec![None; prepared.len()]);
    let cursor = std::sync::atomic::AtomicUsize::new(0);
    let failure: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let index = cursor.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if index >= prepared.len() {
                        return;
                    }
                    let fixture = &prepared[index];
                    let (stage, band, protocol) = match fixture {
                        Prepared::Arena(a) => (
                            a.stage.as_str().to_string(),
                            a.band.as_str().to_string(),
                            task::arena_protocol(founder, a.stage, a.band),
                        ),
                        Prepared::Landscape(_) => (
                            LANDSCAPE_STAGE.to_string(),
                            "none".to_string(),
                            LANDSCAPE_PROTOCOL.to_string(),
                        ),
                    };
                    let horizon = horizon.unwrap_or_else(|| {
                        fixture.horizon().unwrap_or_else(|| {
                            fixture
                                .arena()
                                .map_or(LANDSCAPE_HORIZON_TICKS, |a| a.stage.horizon())
                        })
                    });
                    let sink = teacher_sink();
                    let d = EpisodeDriver::control(VoxelControl::Heuristic, founder)
                        .recording(sink.clone());
                    let outcome = driver::run_prepared(
                        fixture,
                        &d,
                        horizon,
                        driver::Limits::new(&cancel),
                        &format!("imitate/{stage}/{}", fixture.label()),
                    );
                    if let Err(e) = outcome {
                        *failure.lock().expect("failure") = Some(e.to_string());
                        return;
                    }
                    let buffers = sink.lock().expect("sink").clone();
                    let streams = buffers
                        .iter()
                        .enumerate()
                        .filter(|(_, pairs)| !pairs.is_empty())
                        .map(|(body, pairs)| {
                            let mut observations = Vec::with_capacity(pairs.len() * inputs);
                            let mut actions = Vec::with_capacity(pairs.len() * 3);
                            for p in pairs {
                                observations.extend_from_slice(&p.observation);
                                actions.extend_from_slice(&[
                                    p.action.forward,
                                    p.action.turn,
                                    p.action.feed,
                                ]);
                            }
                            TeacherStream {
                                schema: TEACHER_SCHEMA.into(),
                                build: crate::evaluate::BUILD_ID.into(),
                                teacher: TEACHER_CONTROLLER.into(),
                                founder: founder.name().into(),
                                digest: voxel_schema_digest(founder),
                                stage: stage.clone(),
                                band: band.clone(),
                                arena_protocol: protocol.clone(),
                                start_heading: task::START_HEADING_PROTOCOL.into(),
                                starting_stores: task::STARTING_STORES_PROTOCOL.into(),
                                layout_seed: fixture.layout_seed(),
                                fixture: fixture.label(),
                                body,
                                horizon,
                                inputs,
                                steps: pairs.len(),
                                observations,
                                actions,
                            }
                        })
                        .collect();
                    slots.lock().expect("slots")[index] = Some(streams);
                }
            });
        }
    });
    if let Some(detail) = failure.into_inner().map_err(|e| e.to_string())? {
        return Err(format!("a recording episode failed: {detail}"));
    }
    let slots = slots.into_inner().map_err(|e| e.to_string())?;
    let streams: Vec<TeacherStream> = slots.into_iter().flatten().flatten().collect();
    for (i, s) in streams.iter().enumerate() {
        s.validate(&format!("stream {i}"), founder)?;
    }
    Ok(streams)
}

/// Write one run's streams under `dir`, one file per body per fixture, and return their
/// paths.
pub fn write_streams(dir: &Path, streams: &[TeacherStream]) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for s in streams {
        let fixture = if s.fixture.is_empty() {
            s.layout_seed.to_string()
        } else {
            s.fixture.replace('/', "-")
        };
        let path = dir.join(format!("stage{}-{fixture}-body{}.json", s.stage, s.body));
        s.write(&path)?;
        paths.push(path);
    }
    Ok(paths)
}

// ---------------------------------------------------------------------------
// The fit
// ---------------------------------------------------------------------------

/// How the clone is fitted. Every number here is the fit's, not the arena's.
#[derive(Clone, Copy, Debug)]
pub struct FitSpec {
    /// Adam steps over the full stream set. One update is one pass.
    pub updates: u32,
    pub learning_rate: f64,
    /// Truncated-BPTT window in controller steps. The hidden state carries across a
    /// window boundary; the gradient does not.
    pub chunk: usize,
    /// Global gradient-norm clip, applied to the whole parameter vector.
    pub clip: f64,
    /// Seed for the initial centre the fit starts from.
    pub seed: u64,
    pub workers: usize,
}

impl Default for FitSpec {
    fn default() -> FitSpec {
        FitSpec {
            updates: 1_500,
            learning_rate: 0.01,
            chunk: 120,
            clip: 5.0,
            seed: task::TRAINING_SEED,
            workers: task::episode_worker_limit(),
        }
    }
}

/// What a finished fit reports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FitReport {
    pub updates: u32,
    pub steps: usize,
    /// Mean squared error against the teacher's **adapted** action, per action, at the
    /// end of the fit: forward, turn, feed.
    pub mse: [f64; 3],
    /// Their mean — the number the descent is reported against.
    pub mse_mean: f64,
    /// The same three at the initial centre, so the fit's own movement is visible.
    pub initial_mse: [f64; 3],
    /// Fraction of teacher steps whose turn **sign** the clone matches. A teacher step
    /// with a deadbanded (zero) turn counts as matched when the clone is also zero.
    pub turn_sign_agreement: f64,
    /// The mean loss every `report_every` updates, for the plateau judgement.
    pub loss_history: Vec<f64>,
}

/// Fit a clone of the teacher to `streams`, teacher-forced.
pub fn fit(founder: Founder, streams: &[TeacherStream], spec: &FitSpec) -> (Vec<f64>, FitReport) {
    let manifest = founder.manifest();
    let inputs = manifest.inputs();
    let offs = shape_offsets(inputs, 3);
    let mut theta = if founder == Founder::Blind {
        tensor::initial_center_shape::<23, 3>(spec.seed)
    } else {
        tensor::initial_center_shape::<37, 3>(spec.seed)
    };
    let mut adam = Adam::new(theta.len());
    adam.learning_rate = spec.learning_rate;

    let total_steps: usize = streams.iter().map(|s| s.steps).sum();
    let scale = 1.0 / (total_steps.max(1) * 3) as f64;
    let initial_mse = measure(founder, streams, &theta).0;
    let mut loss_history = Vec::new();

    for update in 0..spec.updates {
        let (loss, gradient) =
            full_gradient(&manifest, inputs, &offs, streams, &theta, spec, scale);
        let norm = gradient.iter().map(|g| g * g).sum::<f64>().sqrt();
        let factor = if norm > spec.clip && norm.is_finite() {
            spec.clip / norm
        } else {
            1.0
        };
        // Adam ascends; the objective is a loss, so the step is against the gradient.
        let descent: Vec<f64> = gradient.iter().map(|g| -g * factor).collect();
        adam.ascend(&mut theta, &descent);
        if update % 25 == 0 || update + 1 == spec.updates {
            loss_history.push(loss);
        }
    }

    let (mse, turn_sign_agreement) = measure(founder, streams, &theta);
    let report = FitReport {
        updates: spec.updates,
        steps: total_steps,
        mse,
        mse_mean: mse.iter().sum::<f64>() / 3.0,
        initial_mse,
        turn_sign_agreement,
        loss_history,
    };
    (theta, report)
}

/// The full-batch gradient of the transferred-action squared error over every stream,
/// accumulated in parallel across streams.
fn full_gradient(
    manifest: &Manifest,
    inputs: usize,
    offs: &ShapeOffsets,
    streams: &[TeacherStream],
    theta: &[f64],
    spec: &FitSpec,
    scale: f64,
) -> (f64, Vec<f64>) {
    let cursor = std::sync::atomic::AtomicUsize::new(0);
    let parts: std::sync::Mutex<Vec<(f64, Vec<f64>)>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..spec.workers.max(1).min(streams.len().max(1)) {
            scope.spawn(|| {
                let mut grad = vec![0.0; theta.len()];
                let mut loss = 0.0;
                loop {
                    let i = cursor.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if i >= streams.len() {
                        break;
                    }
                    loss += stream_gradient(
                        manifest,
                        inputs,
                        offs,
                        &streams[i],
                        theta,
                        spec.chunk.max(1),
                        scale,
                        &mut grad,
                    );
                }
                parts.lock().expect("parts").push((loss, grad));
            });
        }
    });
    let parts = parts.into_inner().expect("parts");
    let mut gradient = vec![0.0; theta.len()];
    let mut loss = 0.0;
    for (l, g) in parts {
        loss += l;
        for (a, b) in gradient.iter_mut().zip(g) {
            *a += b;
        }
    }
    (loss, gradient)
}

/// One stream's contribution: truncated BPTT, hidden state carried across windows.
fn stream_gradient(
    manifest: &Manifest,
    inputs: usize,
    offs: &ShapeOffsets,
    stream: &TeacherStream,
    theta: &[f64],
    chunk: usize,
    scale: f64,
    grad: &mut [f64],
) -> f64 {
    let mut hidden = vec![0.0; HIDDEN];
    let mut loss = 0.0;
    let mut start = 0;
    while start < stream.steps {
        let end = (start + chunk).min(stream.steps);
        let observations = &stream.observations[start * inputs..end * inputs];
        let targets = &stream.actions[start * 3..end * 3];
        loss += gradient_of_chunk(
            manifest,
            inputs,
            offs,
            theta,
            observations,
            targets,
            &mut hidden,
            scale,
            grad,
        );
        start = end;
    }
    loss
}

/// Cached forward values for one window, kept for the backward pass.
struct Cache {
    gh: Vec<f64>,
    r: Vec<f64>,
    z: Vec<f64>,
    n: Vec<f64>,
    h: Vec<f64>,
    y: Vec<f64>,
}

/// One truncated-BPTT window: forward over `observations`, squared error against
/// `targets` on the **transferred** action, backward, accumulated into `grad`.
///
/// `hidden` enters as the window's initial state and leaves as its final one; no
/// gradient flows out through it, which is what makes the window truncated. Returns the
/// window's contribution to the loss, already scaled by `scale`.
///
/// This is the function the finite-difference test checks.
#[allow(clippy::too_many_arguments)]
pub fn gradient_of_chunk(
    manifest: &Manifest,
    inputs: usize,
    offs: &ShapeOffsets,
    theta: &[f64],
    observations: &[f64],
    targets: &[f64],
    hidden: &mut [f64],
    scale: f64,
    grad: &mut [f64],
) -> f64 {
    let steps = targets.len() / 3;
    let h0: Vec<f64> = hidden.to_vec();
    let mut cache = Cache {
        gh: vec![0.0; steps * GATES],
        r: vec![0.0; steps * HIDDEN],
        z: vec![0.0; steps * HIDDEN],
        n: vec![0.0; steps * HIDDEN],
        h: vec![0.0; steps * HIDDEN],
        y: vec![0.0; steps * 3],
    };
    let mut loss = 0.0;
    let mut gi = vec![0.0; GATES];
    let mut hp = h0.clone();
    for t in 0..steps {
        let x = &observations[t * inputs..(t + 1) * inputs];
        if t > 0 {
            hp.copy_from_slice(&cache.h[(t - 1) * HIDDEN..t * HIDDEN]);
        }
        for row in 0..GATES {
            let mut a = theta[offs.b_i + row];
            let w = offs.w_i + row * inputs;
            for c in 0..inputs {
                a += theta[w + c] * x[c];
            }
            gi[row] = a;
            let mut b = theta[offs.b_h + row];
            let w = offs.w_h + row * HIDDEN;
            for c in 0..HIDDEN {
                b += theta[w + c] * hp[c];
            }
            cache.gh[t * GATES + row] = b;
        }
        for j in 0..HIDDEN {
            let gh = &cache.gh[t * GATES..(t + 1) * GATES];
            let r = sigmoid(gi[R + j] + gh[R + j]);
            let z = sigmoid(gi[Z + j] + gh[Z + j]);
            let n = (gi[N + j] + r * gh[N + j]).tanh();
            cache.r[t * HIDDEN + j] = r;
            cache.z[t * HIDDEN + j] = z;
            cache.n[t * HIDDEN + j] = n;
            cache.h[t * HIDDEN + j] = z * hp[j] + (1.0 - z) * n;
        }
        for k in 0..3 {
            let mut a = theta[offs.b_o + k];
            let w = offs.w_o + k * HIDDEN;
            for j in 0..HIDDEN {
                a += theta[w + j] * cache.h[t * HIDDEN + j];
            }
            cache.y[t * 3 + k] = a;
        }
        for k in 0..3 {
            let d = transfer(cache.y[t * 3 + k], manifest.actions[k].transfer) - targets[t * 3 + k];
            loss += scale * d * d;
        }
    }
    hidden.copy_from_slice(&cache.h[(steps - 1) * HIDDEN..steps * HIDDEN]);

    // Backward.
    let mut dh_next = vec![0.0; HIDDEN];
    let mut dh = vec![0.0; HIDDEN];
    let mut dhp = vec![0.0; HIDDEN];
    let mut dgi = vec![0.0; GATES];
    let mut dgh = vec![0.0; GATES];
    for t in (0..steps).rev() {
        let x = &observations[t * inputs..(t + 1) * inputs];
        let hp: &[f64] = if t == 0 {
            &h0
        } else {
            &cache.h[(t - 1) * HIDDEN..t * HIDDEN]
        };
        dh.copy_from_slice(&dh_next);
        for k in 0..3 {
            let y = cache.y[t * 3 + k];
            let a = transfer(y, manifest.actions[k].transfer);
            let dy = 2.0
                * scale
                * (a - targets[t * 3 + k])
                * transfer_derivative(y, a, manifest.actions[k].transfer);
            grad[offs.b_o + k] += dy;
            let w = offs.w_o + k * HIDDEN;
            for j in 0..HIDDEN {
                grad[w + j] += dy * cache.h[t * HIDDEN + j];
                dh[j] += dy * theta[w + j];
            }
        }
        dhp.iter_mut().for_each(|v| *v = 0.0);
        for j in 0..HIDDEN {
            let z = cache.z[t * HIDDEN + j];
            let n = cache.n[t * HIDDEN + j];
            let r = cache.r[t * HIDDEN + j];
            let gh_n = cache.gh[t * GATES + N + j];
            let dz = dh[j] * (hp[j] - n);
            let dn = dh[j] * (1.0 - z);
            dhp[j] += dh[j] * z;
            let da_n = dn * (1.0 - n * n);
            dgi[N + j] = da_n;
            dgh[N + j] = da_n * r;
            let da_z = dz * z * (1.0 - z);
            dgi[Z + j] = da_z;
            dgh[Z + j] = da_z;
            let da_r = da_n * gh_n * r * (1.0 - r);
            dgi[R + j] = da_r;
            dgh[R + j] = da_r;
        }
        for row in 0..GATES {
            let a = dgi[row];
            let b = dgh[row];
            grad[offs.b_i + row] += a;
            grad[offs.b_h + row] += b;
            if a != 0.0 {
                let w = offs.w_i + row * inputs;
                for c in 0..inputs {
                    grad[w + c] += a * x[c];
                }
            }
            let w = offs.w_h + row * HIDDEN;
            for c in 0..HIDDEN {
                grad[w + c] += b * hp[c];
                dhp[c] += b * theta[w + c];
            }
        }
        dh_next.copy_from_slice(&dhp);
    }
    loss
}

/// The adapted-action MSE per action and the turn-sign agreement, teacher-forced.
///
/// This is the **reported** measurement: the deadband is applied, exactly as the world
/// applies it, so the numbers say how close the clone's held actions are to the
/// teacher's held actions.
pub fn measure(founder: Founder, streams: &[TeacherStream], theta: &[f64]) -> ([f64; 3], f64) {
    let manifest = founder.manifest();
    let inputs = manifest.inputs();
    let offs = shape_offsets(inputs, 3);
    let mut sums = [0.0f64; 3];
    let mut steps = 0usize;
    let mut turn_matches = 0usize;
    for stream in streams {
        let mut hidden = vec![0.0; HIDDEN];
        for t in 0..stream.steps {
            let y = forward_step(inputs, &offs, theta, stream.observation(t), &mut hidden);
            let response = cubarium_voxel_fauna::Response::Logits(y);
            let a = cubarium_voxel_fauna::resolve_actions(response, &manifest);
            let mine = [a.forward, a.turn, a.feed];
            let teacher = stream.action(t);
            for k in 0..3 {
                let d = mine[k] - teacher[k];
                sums[k] += d * d;
            }
            if sign(mine[1]) == sign(teacher[1]) {
                turn_matches += 1;
            }
            steps += 1;
        }
    }
    let n = steps.max(1) as f64;
    (
        [sums[0] / n, sums[1] / n, sums[2] / n],
        turn_matches as f64 / n,
    )
}

/// One forward step of the flat-theta GRU. Identical arithmetic to
/// [`cubarium_core::neural::gru::Gru::forward`]; a test asserts they agree value for
/// value on both voxel shapes.
pub fn forward_step(
    inputs: usize,
    offs: &ShapeOffsets,
    theta: &[f64],
    x: &[f64],
    hidden: &mut [f64],
) -> [f64; 3] {
    let mut gi = [0.0f64; GATES];
    let mut gh = [0.0f64; GATES];
    for row in 0..GATES {
        let mut a = theta[offs.b_i + row];
        let w = offs.w_i + row * inputs;
        for c in 0..inputs {
            a += theta[w + c] * x[c];
        }
        gi[row] = a;
        let mut b = theta[offs.b_h + row];
        let w = offs.w_h + row * HIDDEN;
        for c in 0..HIDDEN {
            b += theta[w + c] * hidden[c];
        }
        gh[row] = b;
    }
    let mut next = [0.0f64; HIDDEN];
    for j in 0..HIDDEN {
        let r = sigmoid(gi[R + j] + gh[R + j]);
        let z = sigmoid(gi[Z + j] + gh[Z + j]);
        let n = (gi[N + j] + r * gh[N + j]).tanh();
        next[j] = z * hidden[j] + (1.0 - z) * n;
    }
    hidden.copy_from_slice(&next);
    let mut y = [0.0f64; 3];
    for (k, slot) in y.iter_mut().enumerate() {
        let mut a = theta[offs.b_o + k];
        let w = offs.w_o + k * HIDDEN;
        for j in 0..HIDDEN {
            a += theta[w + j] * hidden[j];
        }
        *slot = a;
    }
    y
}

fn sigmoid(v: f64) -> f64 {
    1.0 / (1.0 + (-v).exp())
}

/// The manifest's transfer, matching `cubarium_voxel_fauna::resolve_actions`'s own.
fn transfer(logit: f64, t: Transfer) -> f64 {
    if !logit.is_finite() {
        return 0.0;
    }
    match t {
        Transfer::Sigmoid => sigmoid(logit),
        Transfer::Tanh => logit.tanh(),
    }
}

/// `d transfer(y) / dy`, given the already-computed value.
fn transfer_derivative(logit: f64, value: f64, t: Transfer) -> f64 {
    if !logit.is_finite() {
        return 0.0;
    }
    match t {
        Transfer::Sigmoid => value * (1.0 - value),
        Transfer::Tanh => 1.0 - value * value,
    }
}

fn sign(v: f64) -> i8 {
    if v > 0.0 {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::neural::gru::Gru;

    /// The flat-theta forward step is the core GRU's, value for value, on both shapes.
    #[test]
    fn the_flat_forward_step_is_the_core_gru() {
        for (inputs, founder) in [(23usize, Founder::Blind), (37, Founder::Browser)] {
            let theta = if founder == Founder::Blind {
                tensor::initial_center_shape::<23, 3>(3)
            } else {
                tensor::initial_center_shape::<37, 3>(3)
            };
            let offs = shape_offsets(inputs, 3);
            let mut mine = vec![0.0; HIDDEN];
            let mut theirs = [0.0; HIDDEN];
            for step in 0..5 {
                let x: Vec<f64> = (0..inputs)
                    .map(|c| ((step * 7 + c) as f64 * 0.11).sin())
                    .collect();
                let a = forward_step(inputs, &offs, &theta, &x, &mut mine);
                let b = if founder == Founder::Blind {
                    let w: Gru<23, 3> = tensor::unflatten_shape(&theta).expect("shape");
                    let mut xa = [0.0; 23];
                    xa.copy_from_slice(&x);
                    w.forward(&xa, &mut theirs)
                } else {
                    let w: Gru<37, 3> = tensor::unflatten_shape(&theta).expect("shape");
                    let mut xa = [0.0; 37];
                    xa.copy_from_slice(&x);
                    w.forward(&xa, &mut theirs)
                };
                assert_eq!(a, b, "step {step} of {}", founder.name());
                assert_eq!(mine.as_slice(), theirs.as_slice());
            }
        }
    }

    /// The analytic gradient against central finite differences on a tiny shape: two
    /// inputs, three steps, one window. Every parameter is checked.
    #[test]
    fn the_analytic_gradient_matches_finite_differences_on_a_tiny_shape() {
        let inputs = 2usize;
        let offs = shape_offsets(inputs, 3);
        let manifest = Founder::Blind.manifest();
        let steps = 3usize;
        let mut theta = tensor::initial_center_shape::<23, 3>(5)[..offs.params].to_vec();
        for (i, v) in theta.iter_mut().enumerate() {
            *v += 0.1 * ((i as f64) * 0.37).sin();
        }
        let observations: Vec<f64> = (0..steps * inputs)
            .map(|i| ((i as f64) * 0.7).cos())
            .collect();
        let targets: Vec<f64> = (0..steps * 3)
            .map(|i| 0.5 + 0.3 * ((i as f64) * 1.3).sin())
            .collect();
        let scale = 1.0 / (steps * 3) as f64;

        let loss_at = |theta: &[f64]| -> f64 {
            let mut hidden = vec![0.0; HIDDEN];
            let mut loss = 0.0;
            for t in 0..steps {
                let y = forward_step(
                    inputs,
                    &offs,
                    theta,
                    &observations[t * inputs..(t + 1) * inputs],
                    &mut hidden,
                );
                for k in 0..3 {
                    let d = transfer(y[k], manifest.actions[k].transfer) - targets[t * 3 + k];
                    loss += scale * d * d;
                }
            }
            loss
        };

        let mut grad = vec![0.0; theta.len()];
        let mut hidden = vec![0.0; HIDDEN];
        let analytic_loss = gradient_of_chunk(
            &manifest,
            inputs,
            &offs,
            &theta,
            &observations,
            &targets,
            &mut hidden,
            scale,
            &mut grad,
        );
        assert!(
            (analytic_loss - loss_at(&theta)).abs() < 1e-12,
            "the backward pass must report the forward pass's own loss"
        );

        let eps = 1e-6;
        let mut worst = 0.0f64;
        for i in 0..theta.len() {
            let keep = theta[i];
            theta[i] = keep + eps;
            let up = loss_at(&theta);
            theta[i] = keep - eps;
            let down = loss_at(&theta);
            theta[i] = keep;
            let fd = (up - down) / (2.0 * eps);
            let err = (fd - grad[i]).abs() / (1.0 + fd.abs().max(grad[i].abs()));
            worst = worst.max(err);
        }
        assert!(
            worst < 1e-7,
            "analytic gradient disagrees with finite differences by {worst:e}"
        );
    }

    /// A synthetic two-step stream: the fit reduces the loss and the turn sign agrees.
    #[test]
    fn a_short_fit_reduces_the_error_against_its_teacher() {
        let founder = Founder::Blind;
        let inputs = founder.manifest().inputs();
        let steps = 8usize;
        let observations: Vec<f64> = (0..steps * inputs)
            .map(|i| 0.5 + 0.4 * ((i as f64) * 0.31).sin())
            .collect();
        // A teacher that alternates its turn and always drives forward.
        let mut actions = Vec::new();
        for t in 0..steps {
            let turn = if t % 2 == 0 { 0.6 } else { -0.6 };
            actions.extend_from_slice(&[0.9, turn, 0.0]);
        }
        let stream = TeacherStream {
            schema: TEACHER_SCHEMA.into(),
            build: "test".into(),
            teacher: TEACHER_CONTROLLER.into(),
            founder: founder.name().into(),
            digest: voxel_schema_digest(founder),
            stage: Stage::A.as_str().into(),
            band: Band::Landed.as_str().into(),
            arena_protocol: task::arena_protocol(founder, Stage::A, Band::Landed),
            start_heading: task::START_HEADING_PROTOCOL.into(),
            starting_stores: task::STARTING_STORES_PROTOCOL.into(),
            layout_seed: 0,
            fixture: String::new(),
            body: 0,
            horizon: 40,
            inputs,
            steps,
            observations,
            actions,
        };
        stream.validate("synthetic", founder).expect("valid");
        let spec = FitSpec {
            updates: 120,
            workers: 1,
            ..FitSpec::default()
        };
        let (theta, report) = fit(founder, std::slice::from_ref(&stream), &spec);
        assert_eq!(theta.len(), founder.manifest().parameter_count());
        assert!(
            report.mse_mean < report.initial_mse.iter().sum::<f64>() / 3.0,
            "the fit must reduce the adapted error: {report:?}"
        );
        assert!(
            report.turn_sign_agreement > 0.5,
            "a clone of an alternating teacher should follow its turn sign: {report:?}"
        );
        assert!(report.loss_history.first() > report.loss_history.last());
    }

    /// A fitted clone is an ordinary centre: it round-trips with its imitation
    /// provenance, its own loader accepts it, `--init-center` accepts it as a warm
    /// start, and the band it declares is the only one it is accepted under.
    #[test]
    fn a_fitted_clone_is_an_ordinary_centre_with_its_provenance() {
        let founder = Founder::Browser;
        let streams = record_streams(founder, Stage::B, Band::Landed, 1, 100, 1).expect("recorded");
        let spec = FitSpec {
            updates: 5,
            workers: 1,
            ..FitSpec::default()
        };
        let (theta, report) = fit(founder, &streams, &spec);
        let file = crate::es::voxel::store::VoxelPolicyFile {
            schema: crate::es::voxel::store::POLICY_SCHEMA.into(),
            build: "test".into(),
            founder: founder.name().into(),
            digest: voxel_schema_digest(founder),
            train_seed: spec.seed,
            generation: None,
            score: None,
            start_heading: task::START_HEADING_PROTOCOL.into(),
            starting_stores: task::STARTING_STORES_PROTOCOL.into(),
            arena_protocol: task::arena_protocol(founder, Stage::B, Band::Landed),
            protocol_hash: None,
            imitation: Some(crate::es::voxel::store::ImitationProvenance {
                provenance: IMITATION_PROVENANCE.into(),
                teacher: TEACHER_CONTROLLER.into(),
                streams_fnv1a: streams_digest(&streams),
                streams: streams.len(),
                steps: report.steps,
                updates: report.updates,
                mse: report.mse,
                mse_mean: report.mse_mean,
                turn_sign_agreement: report.turn_sign_agreement,
            }),
            stage: Stage::B.as_str().into(),
            theta,
        };
        let dir = std::env::temp_dir().join(format!("cubarium-clone-{}", std::process::id()));
        let path = dir.join("clone-center.json");
        file.write(&path).expect("written");

        let back = crate::es::voxel::store::VoxelPolicyFile::load(&path).expect("loaded");
        assert_eq!(back, file, "exact, provenance included");
        let provenance = back.imitation.as_ref().expect("provenance");
        assert_eq!(provenance.provenance, IMITATION_PROVENANCE);
        assert_eq!(provenance.teacher, TEACHER_CONTROLLER);
        assert!(back.driver().is_ok(), "a clone drives an episode");
        back.validate_for_band("clone", Band::Landed)
            .expect("its own band");
        assert!(
            back.validate_for_band("clone", Band::Near).is_err(),
            "a landed clone is not a near-rung policy"
        );

        // The warm-start path takes it, and refuses it for the other lineage.
        let warm = crate::es::voxel::trainer::InitCenter::load(&path, founder).expect("warm");
        assert_eq!(warm.theta, back.theta);
        assert!(
            crate::es::voxel::trainer::InitCenter::load(&path, Founder::Blind).is_err(),
            "another lineage's centre is refused by name"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A stream is refused by name, never reinterpreted: wrong schema token, wrong
    /// teacher, another founder's, another build's digest, another arena's protocol, and
    /// a length that is not `steps × inputs`.
    #[test]
    fn a_teacher_stream_is_refused_by_name() {
        let founder = Founder::Blind;
        let inputs = founder.manifest().inputs();
        let base = TeacherStream {
            schema: TEACHER_SCHEMA.into(),
            build: "test".into(),
            teacher: TEACHER_CONTROLLER.into(),
            founder: founder.name().into(),
            digest: voxel_schema_digest(founder),
            stage: Stage::B.as_str().into(),
            band: Band::Landed.as_str().into(),
            arena_protocol: task::arena_protocol(founder, Stage::B, Band::Landed),
            start_heading: task::START_HEADING_PROTOCOL.into(),
            starting_stores: task::STARTING_STORES_PROTOCOL.into(),
            layout_seed: 7,
            fixture: String::new(),
            body: 0,
            horizon: 100,
            inputs,
            steps: 2,
            observations: vec![0.25; 2 * inputs],
            actions: vec![0.5; 6],
        };
        base.validate("base", founder).expect("valid");

        let mut s = base.clone();
        s.schema = "cub-voxel-teacher-0".into();
        assert!(
            s.validate("s", founder)
                .expect_err("refused")
                .contains(TEACHER_SCHEMA)
        );

        let mut s = base.clone();
        s.teacher = "cruise".into();
        assert!(
            s.validate("s", founder)
                .expect_err("refused")
                .contains("cruise")
        );

        let s = base.clone();
        let err = s.validate("s", Founder::Browser).expect_err("refused");
        assert!(err.contains("frondgrazer"), "{err}");

        let mut s = base.clone();
        s.digest ^= 1;
        assert!(
            s.validate("s", founder)
                .expect_err("refused")
                .contains("observation layout differs")
        );

        let mut s = base.clone();
        s.band = Band::Near.as_str().into();
        let err = s.validate("s", founder).expect_err("refused");
        assert!(
            err.contains(&task::arena_protocol(founder, Stage::B, Band::Landed)),
            "{err}"
        );

        let mut s = base.clone();
        s.start_heading = "aimed-at-food".into();
        assert!(s.validate("s", founder).is_err());

        let mut s = base.clone();
        s.steps = 3;
        assert!(
            s.validate("s", founder)
                .expect_err("refused")
                .contains("observation values")
        );

        let mut s = base.clone();
        s.actions[0] = f64::NAN;
        assert!(
            s.validate("s", founder)
                .expect_err("refused")
                .contains("finite")
        );

        // Round trip through a file, exactly.
        let dir = std::env::temp_dir().join(format!("cubarium-teacher-{}", std::process::id()));
        let path = dir.join("stream.json");
        base.write(&path).expect("written");
        assert_eq!(TeacherStream::load(&path, founder).expect("loaded"), base);
        assert!(TeacherStream::load(&path, Founder::Browser).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The recording wrapper is transparent and captures one pair per controller sample:
    /// 100 ticks at the 5-tick cadence is 20 samples, and the recorded episode's outcome
    /// is the unrecorded one's.
    #[test]
    fn recording_captures_one_pair_per_sample_and_changes_nothing() {
        let founder = Founder::Blind;
        let ticks = 100u64;
        let prepared = task::Prepared::build(founder, task::TRAINING_LAYOUT_SEEDS[0]);
        let cancel = AtomicBool::new(false);
        let plain = EpisodeDriver::control(VoxelControl::Heuristic, founder);
        let bare = driver::run_prepared(
            &prepared,
            &plain,
            ticks,
            driver::Limits::new(&cancel),
            "test/bare",
        )
        .expect("episode");

        let sink = teacher_sink();
        let recorded_driver = plain.clone().recording(sink.clone());
        let recorded = driver::run_prepared(
            &prepared,
            &recorded_driver,
            ticks,
            driver::Limits::new(&cancel),
            "test/recorded",
        )
        .expect("episode");

        assert_eq!(recorded.pose_x, bare.pose_x, "the wrapper is transparent");
        assert_eq!(recorded.pose_z, bare.pose_z);
        assert_eq!(recorded.eaten_organic, bare.eaten_organic);
        assert_eq!(recorded.driver, "heuristic", "the wrapper keeps the name");

        let buffers = sink.lock().expect("sink").clone();
        assert_eq!(
            buffers.len(),
            1,
            "an arena has one acting body, so one buffer"
        );
        let pairs = buffers[0].clone();
        let cadence = founder.manifest().cadence_ticks();
        assert_eq!(pairs.len() as u64, ticks / cadence);
        let inputs = founder.manifest().inputs();
        for p in &pairs {
            assert_eq!(p.observation.len(), inputs);
            assert!(p.observation.iter().all(|v| v.is_finite()));
            assert!((0.0..=1.0).contains(&p.action.forward));
            assert!((-1.0..=1.0).contains(&p.action.turn));
            assert!((0.0..=1.0).contains(&p.action.feed));
        }
        // The teacher wanders: it does not hold one single action for a whole episode.
        assert!(
            pairs.iter().any(|p| p.action.turn != pairs[0].action.turn),
            "the heuristic alternates its turn"
        );
    }

    /// Recording the training layouts yields one stream per layout, each a valid file.
    #[test]
    fn recorded_training_streams_validate_and_digest_stably() {
        let founder = Founder::Browser;
        let streams = record_streams(founder, Stage::A, Band::Landed, 2, 100, 2).expect("recorded");
        assert_eq!(streams.len(), 2);
        for s in &streams {
            s.validate("recorded", founder).expect("valid");
            assert_eq!(s.steps as u64, 100 / founder.manifest().cadence_ticks());
        }
        assert_eq!(streams_digest(&streams), streams_digest(&streams));
        let other = record_streams(founder, Stage::A, Band::Landed, 1, 100, 1).expect("recorded");
        assert_ne!(streams_digest(&streams), streams_digest(&other));
    }
}
