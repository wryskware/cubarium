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
//! This step is the **recording** half. [`record_streams`] runs the heuristic through
//! the ordinary episode driver with [`super::controller::RecordingController`] attached,
//! so the episode is the real one and the pairs are the real observations and the real
//! *adapted* actions (the fauna's own `resolve_actions`). One [`TeacherStream`] per
//! layout per stage. The fit that consumes them is the next step.
//!
//! # What is and is not in a stream
//!
//! A stream holds the observation vector the body sampled and the three bounded actions
//! the world then held. There is no site, no distance, no stock, no patch identity and no
//! layout geometry: a clone fitted on these has been told exactly what a policy in that
//! slot is told. Streams come from the **training** layouts only; the held-out seeds are
//! never recorded, never fitted and never selected on.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use cubarium_voxel_fauna::Founder;
use serde::{Deserialize, Serialize};

use super::controller::{EpisodeDriver, VoxelControl, teacher_sink};
use super::task::{self, Band, Stage};
use super::{driver, voxel_schema_digest};
use crate::es::bits::hex_f64s;

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
        let stage = task::parse_stage(&self.stage).map_err(|e| format!("{name}: {e}"))?;
        let band = task::parse_band(&self.band).map_err(|e| format!("{name}: {e}"))?;
        let want = task::arena_protocol(stage, band);
        if self.arena_protocol != want {
            return Err(format!(
                "{name}: stream arena protocol is `{}`, this build's stage {} band {} uses \
                 `{want}`: the task differs",
                self.arena_protocol,
                stage.as_str(),
                band.as_str()
            ));
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
        bytes.push_str(&crate::es::bits::encode(&s.observations));
        bytes.push_str(&crate::es::bits::encode(&s.actions));
    }
    crate::es::fixture::fnv1a(bytes.as_bytes())
}

/// Record the heuristic on the training layouts of one `(stage, band)`.
///
/// The episode is the ordinary one: same prepared layout, same driver, same fauna tick.
/// The only difference is the transparent recording wrapper, which returns the teacher's
/// response untouched — so a recorded episode is an episode, not a simulation of one.
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
    let inputs = founder.manifest().inputs();
    let cancel = AtomicBool::new(false);
    let slots: std::sync::Mutex<Vec<Option<TeacherStream>>> =
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
                    let sink = teacher_sink();
                    let d = EpisodeDriver::control(VoxelControl::Heuristic, founder)
                        .recording(sink.clone());
                    let outcome = driver::run_prepared(
                        &prepared[index],
                        &d,
                        horizon,
                        driver::Limits::new(&cancel),
                        &format!("imitate/{}/{index}", stage.as_str()),
                    );
                    match outcome {
                        Ok(_) => {}
                        Err(e) => {
                            *failure.lock().expect("failure") = Some(e.to_string());
                            return;
                        }
                    }
                    let pairs = sink.lock().expect("sink").clone();
                    let mut observations = Vec::with_capacity(pairs.len() * inputs);
                    let mut actions = Vec::with_capacity(pairs.len() * 3);
                    for p in &pairs {
                        observations.extend_from_slice(&p.observation);
                        actions.extend_from_slice(&[
                            p.action.forward,
                            p.action.turn,
                            p.action.feed,
                        ]);
                    }
                    slots.lock().expect("slots")[index] = Some(TeacherStream {
                        schema: TEACHER_SCHEMA.into(),
                        build: crate::evaluate::BUILD_ID.into(),
                        teacher: TEACHER_CONTROLLER.into(),
                        founder: founder.name().into(),
                        digest: voxel_schema_digest(founder),
                        stage: stage.as_str().into(),
                        band: band.as_str().into(),
                        arena_protocol: task::arena_protocol(stage, band).into(),
                        start_heading: task::START_HEADING_PROTOCOL.into(),
                        starting_stores: task::STARTING_STORES_PROTOCOL.into(),
                        layout_seed: prepared[index].layout_seed,
                        horizon,
                        inputs,
                        steps: pairs.len(),
                        observations,
                        actions,
                    });
                }
            });
        }
    });
    if let Some(detail) = failure.into_inner().map_err(|e| e.to_string())? {
        return Err(format!("a recording episode failed: {detail}"));
    }
    let slots = slots.into_inner().map_err(|e| e.to_string())?;
    let streams: Vec<TeacherStream> = slots.into_iter().flatten().collect();
    for (i, s) in streams.iter().enumerate() {
        s.validate(&format!("stream {i}"), founder)?;
    }
    Ok(streams)
}

/// Write one run's streams under `dir`, one file per layout, and return their paths.
pub fn write_streams(dir: &Path, streams: &[TeacherStream]) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for s in streams {
        let path = dir.join(format!("stage{}-seed{}.json", s.stage, s.layout_seed));
        s.write(&path)?;
        paths.push(path);
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            arena_protocol: task::arena_protocol(Stage::B, Band::Landed).into(),
            start_heading: task::START_HEADING_PROTOCOL.into(),
            starting_stores: task::STARTING_STORES_PROTOCOL.into(),
            layout_seed: 7,
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
            err.contains(task::arena_protocol(Stage::B, Band::Landed)),
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

        let pairs = sink.lock().expect("sink").clone();
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
