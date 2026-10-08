//! **[`Session`]**: a world and its loop on a thread of its own.
//!
//! Three kinds of thread, and who may wait on whom:
//! - **the loop** (one per session) owns the world, flora and fauna, steps them at 20 Hz
//!   times the speed, applies commands at its command point, and builds the feed and the
//!   publications. It never takes a lock a reader can hold: everything it produces goes
//!   out over a channel. Loads run here too, so a load never races a tick.
//! - **the ingest thread** files what the loop sends into the [`Hub`] under its lock:
//!   chunk changes are unioned, the frame and the snapshot replaced, events appended. A
//!   slow reader holding the lock only delays this thread.
//! - **readers** (Godot's main thread) lock the hub briefly to swap out what they take.
//!
//! Candidates are bare terrains drawn on threads of their own; the picked one is founded
//! on a thread of its own ([`Session::promote`], `crate::founding`) and never touches the
//! loop until it is installed.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cubarium_observatory::{Cadence, Controls, History, HistoryConfig, LifeBook, LifeEvent, Observer, Sample, Snapshot};
use cubarium_voxel::World;
use cubarium_voxel_fauna::{Fauna, Founder};
use cubarium_voxel_flora::Flora;
use cubarium_voxel_sim::Sim;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::console::{Console, MAX_SPEED, MIN_SPEED, Pacer, console_line};
use crate::create::{self, Populate, Scene, StartLayers, StartSpec};
use crate::feed::{ChunkData, FeedConfig, Frame, Tracker, View};
use crate::founding::{Busy, FoundingInfo, Job, Outcome, TERRAIN_TRIES, found_picked, shower_typical, terrain_candidate};
use crate::hook::{HookClock, LoopHook};
use crate::founders::founder_recipes;
use crate::params::{SimParams, WorldSpec};
use crate::store::{self, SaveMeta};

/// How a session runs.
#[derive(Clone, Debug)]
pub struct SessionOptions {
    /// Execution settings (threads, water fidelity, history sampling) for worlds that are
    /// loaded rather than made from a [`WorldSpec`].
    pub exec: SimParams,
    /// Build the view feed. Off, the loop pays nothing for it and [`Session::take_view`]
    /// returns only the world's shape.
    pub feed: bool,
    pub feed_config: FeedConfig,
    /// When snapshots for queries are published.
    pub cadence: Cadence,
    /// The speed a session's first world starts at; later worlds keep the current one.
    pub speed: f64,
    /// Life events held for [`Session::take_events`] before the oldest are dropped.
    pub max_events: usize,
    /// Terrain draws a candidate makes before it reports that none could take the
    /// founders ([`crate::founding::TERRAIN_TRIES`]).
    pub terrain_tries: usize,
    /// Placement tries on a picked terrain before its founding is refused
    /// ([`cubarium_voxel_sim::found::PLACEMENT_TRIES`]).
    pub placement_tries: usize,
    /// Test only: the habitat verdict a founding is judged by, in place of its own.
    #[cfg(test)]
    pub(crate) accept: Option<fn(&cubarium_voxel_sim::habitat::Seeded) -> bool>,
}

impl Default for SessionOptions {
    fn default() -> SessionOptions {
        SessionOptions {
            exec: SimParams::default(),
            feed: true,
            feed_config: FeedConfig::default(),
            cadence: Cadence::default(),
            speed: 1.0,
            max_events: 4096,
            terrain_tries: TERRAIN_TRIES,
            placement_tries: cubarium_voxel_sim::found::PLACEMENT_TRIES,
            #[cfg(test)]
            accept: None,
        }
    }
}

/// A world ready to run.
pub struct Prepared {
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
    pub label: String,
    pub exec: SimParams,
}

/// Something a reader is told: a console reply, a save landing or failing, a load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notice {
    /// `console`, `saved`, `save_failed`, `loaded`, `load_failed`, `closed`.
    pub kind: String,
    pub text: String,
    pub path: Option<String>,
    pub meta: Option<SaveMeta>,
}

impl Notice {
    fn say(kind: &str, text: impl Into<String>) -> Notice {
        Notice { kind: kind.into(), text: text.into(), path: None, meta: None }
    }
}

/// The run as it stands, read without waiting on anything.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// A world is installed (running or paused).
    pub running: bool,
    pub paused: bool,
    pub speed: f64,
    /// The world's tick.
    pub tick: u64,
    /// Ticks this session's loop has stepped, over every world.
    pub stepped: u64,
    /// Counts installed worlds.
    pub generation: u64,
    /// A load is being read.
    pub loading: bool,
    /// A save is being written.
    pub saving: bool,
    pub label: String,
    pub last_error: Option<String>,
    /// Mean milliseconds a tick took to step, and a feed update to build, over this world.
    pub step_ms: f64,
    pub feed_ms: f64,
    /// Feed updates built over this world.
    pub feed_updates: u64,
}

/// What a candidate is doing. A candidate is a **bare terrain**: nothing on it is founded
/// until it is picked ([`Session::promote`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CandidateInfo {
    pub index: usize,
    /// `generating`, `ready`, `failed` or `cancelled`.
    pub state: String,
    /// The seed asked for while generating; the terrain's own once ready.
    pub seed: Option<u64>,
    pub label: Option<String>,
    pub error: Option<String>,
    pub summary: Option<cubarium_observatory::query::WorldSummary>,
    pub populations: Option<cubarium_observatory::query::Populations>,
    /// The terrain's hash ([`World::terrain_hash`], hex), once ready.
    pub terrain: Option<String>,
    /// What the feasibility check said: whether the founders could all be placed on the
    /// bare terrain. `None` for a bare spec, which founds no one. A drawn terrain is only
    /// offered when it is `true`; an asked seed is offered either way.
    pub feasible: Option<bool>,
    /// What was said of this terrain: an asked seed kept despite its lake or its
    /// feasibility, the wettest kept when none had the lake asked for.
    pub notes: Vec<String>,
}

enum Slot {
    Generating,
    /// Shared, so a reader (a renderer staging a thumbnail) holds the candidate's world
    /// without holding the list's lock ([`CandidateSource::get`]).
    Ready(Arc<Prepared>),
    Failed(String),
}

struct Candidate {
    slot: Slot,
    info: CandidateInfo,
}

#[derive(Default)]
struct Candidates {
    /// Bumped on every new batch and every promotion, so a draw that finishes late files
    /// into nothing.
    batch: u64,
    list: Vec<Candidate>,
    /// The batch's stop flag: its draws stop at the next one when it is set.
    cancel: Arc<AtomicBool>,
    /// What the batch was made from, for founding the one picked.
    spec: Option<WorldSpec>,
    params: Option<SimParams>,
}

enum ToSim {
    Install(Box<Prepared>),
    Load(PathBuf, Option<Sender<Result<(), String>>>),
    Line(String),
    Save(PathBuf, Option<Sender<Result<SaveMeta, String>>>),
    Close,
    Shutdown,
    Hook(Option<Box<dyn LoopHook>>),
}

enum ToReader {
    Reset {
        generation: u64,
        label: String,
        dims: [u32; 3],
        chunk_dims: [u32; 3],
        voxel_m: f64,
        history: HistoryConfig,
        chunks: Vec<ChunkData>,
        frame: Option<Arc<Frame>>,
    },
    Closed,
    Feed { chunks: Vec<ChunkData>, frame: Option<Arc<Frame>>, events: Vec<LifeEvent> },
    Publication(Box<Snapshot>),
    Notice(Notice),
    Error(Option<String>),
}

/// What readers read, filled by the ingest thread.
struct Hub {
    generation: u64,
    snapshot: Option<Arc<Snapshot>>,
    book: LifeBook,
    history: History,
    dims: [u32; 3],
    chunk_dims: [u32; 3],
    voxel_m: f64,
    full: bool,
    chunks: FxHashMap<u32, ChunkData>,
    frame: Option<Arc<Frame>>,
    events: VecDeque<LifeEvent>,
    dropped_events: u64,
    notices: VecDeque<Notice>,
}

impl Hub {
    fn new() -> Hub {
        Hub {
            generation: 0,
            snapshot: None,
            book: LifeBook::default(),
            history: History::new(HistoryConfig::default()),
            dims: [0; 3],
            chunk_dims: [0; 3],
            voxel_m: 0.0,
            full: false,
            chunks: FxHashMap::default(),
            frame: None,
            events: VecDeque::new(),
            dropped_events: 0,
            notices: VecDeque::new(),
        }
    }
}

struct Shared {
    hub: Mutex<Hub>,
    /// The run's label and last error: their own lock, so [`Session::status`] never waits
    /// on a reader holding the hub.
    text: Mutex<(String, Option<String>)>,
    running: AtomicBool,
    paused: AtomicBool,
    speed_bits: AtomicU64,
    tick: AtomicU64,
    stepped: AtomicU64,
    generation: AtomicU64,
    loading: AtomicBool,
    saving: AtomicBool,
    step_ns: AtomicU64,
    steps: AtomicU64,
    feed_ns: AtomicU64,
    feed_updates: AtomicU64,
}

/// **A world and its loop on a thread of its own.** Every call returns at once: the slow
/// ones (founding candidates, loading, saving) run off the caller's thread and report
/// through [`Session::status`], [`Session::candidates`] and [`Session::take_notices`].
pub struct Session {
    to_sim: Sender<ToSim>,
    shared: Arc<Shared>,
    threads: Arc<Threads>,
    candidates: Arc<Mutex<Candidates>>,
    /// The picked candidate's founding, the last one started.
    job: Arc<Mutex<Option<Arc<Job>>>>,
    /// Candidate and founding threads still running.
    workers: Arc<AtomicUsize>,
    terrain_tries: usize,
    placement_tries: usize,
    #[cfg(test)]
    accept: Option<fn(&cubarium_voxel_sim::habitat::Seeded) -> bool>,
}

/// Every thread a session starts, and how to stop them ([`Threads::stop`]): the candidate
/// draws and the founding are cancelled (a draw stops between draws, a founding at its next
/// tick), then the loop is asked to shut down and joined (it joins a save's writer on the
/// way out), then the ingest thread, which ends when the loop's channel closes. The Godot
/// extension's library is unloaded at exit, and a thread still running its code then
/// faults; see `stop` for the draws and foundings it does not wait for.
struct Threads {
    to_sim: Sender<ToSim>,
    /// The loop first, then the ingest thread.
    handles: Mutex<Vec<JoinHandle<()>>>,
    /// Candidate draws and foundings not yet joined.
    workers: Mutex<Vec<JoinHandle<()>>>,
    /// Their stop flags: the batch's and the founding's.
    candidates: Arc<Mutex<Candidates>>,
    job: Arc<Mutex<Option<Arc<Job>>>>,
}

impl Threads {
    /// Keep a candidate or founding thread to join; the finished ones are joined here.
    fn track(&self, h: JoinHandle<()>) {
        let mut w = self.workers.lock().unwrap_or_else(|e| e.into_inner());
        let (done, live): (Vec<_>, Vec<_>) = std::mem::take(&mut *w).into_iter().partition(|t| t.is_finished());
        *w = live;
        w.push(h);
        drop(w);
        for t in done {
            let _ = t.join();
        }
    }

    /// Stop and join; returns whether anything was still to stop. Idempotent.
    fn stop(&self) -> bool {
        if let Ok(c) = self.candidates.lock() {
            c.cancel.store(true, Ordering::SeqCst);
        }
        if let Some(j) = self.job.lock().ok().and_then(|j| j.clone()) {
            j.cancel.store(true, Ordering::SeqCst);
        }
        // Draws and foundings that have ended are joined. Ones still running are cancelled
        // above and end at their next check, but are not waited for: a draw checks only
        // between draws (about 10 s on the wide world), and quitting must stay quick. They
        // are pure computation on state they hold by `Arc` and send nothing anyone waits on,
        // so ending with the process is harmless; the Godot extension is linked so its code
        // stays mapped until then (crates/cubarium-godot/build.rs).
        let workers = std::mem::take(&mut *self.workers.lock().unwrap_or_else(|e| e.into_inner()));
        let (done, running): (Vec<_>, Vec<_>) = workers.into_iter().partition(|w| w.is_finished());
        for w in done {
            let _ = w.join();
        }
        let busy = !running.is_empty();
        drop(running);
        let _ = self.to_sim.send(ToSim::Shutdown);
        let handles = std::mem::take(&mut *self.handles.lock().unwrap_or_else(|e| e.into_inner()));
        let live = busy || !handles.is_empty();
        for h in handles {
            let _ = h.join();
        }
        live
    }
}

/// A handle that stops every thread of a [`Session`] without owning it ([`Threads::stop`]):
/// for an owner whose drop may come too late (the Godot extension stops every session still
/// alive when its library is being unloaded). Holds the session only weakly.
#[derive(Clone)]
pub struct SessionStop(std::sync::Weak<Threads>);

impl SessionStop {
    /// Cancel what is being made, stop the loop (and with it the running world) and join
    /// it and the ingest thread, as dropping the session would. Returns whether any thread
    /// was still running.
    pub fn shutdown(&self) -> bool {
        self.0.upgrade().is_some_and(|t| t.stop())
    }

    /// Whether the session has not been dropped yet.
    pub fn alive(&self) -> bool {
        self.0.strong_count() > 0
    }
}

impl Session {
    /// A session with no world yet.
    pub fn new(options: SessionOptions) -> Session {
        let (terrain_tries, placement_tries) = (options.terrain_tries, options.placement_tries);
        #[cfg(test)]
        let accept = options.accept;
        let shared = Arc::new(Shared {
            hub: Mutex::new(Hub::new()),
            text: Mutex::new((String::new(), None)),
            running: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            speed_bits: AtomicU64::new(options.speed.to_bits()),
            tick: AtomicU64::new(0),
            stepped: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            loading: AtomicBool::new(false),
            saving: AtomicBool::new(false),
            step_ns: AtomicU64::new(0),
            steps: AtomicU64::new(0),
            feed_ns: AtomicU64::new(0),
            feed_updates: AtomicU64::new(0),
        });
        let (to_sim, sim_rx) = mpsc::channel();
        let (to_reader, reader_rx) = mpsc::channel();
        let ingest = {
            let shared = shared.clone();
            let max = options.max_events;
            std::thread::Builder::new()
                .name("cubarium-session-ingest".into())
                .spawn(move || ingest_main(reader_rx, &shared, max))
                .expect("spawning the session's ingest thread")
        };
        let sim = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("cubarium-session-loop".into())
                .spawn(move || Loop::new(options, to_reader, shared).main(sim_rx))
                .expect("spawning the session's loop")
        };
        let candidates = Arc::new(Mutex::new(Candidates::default()));
        let job = Arc::new(Mutex::new(None));
        Session {
            threads: Arc::new(Threads {
                to_sim: to_sim.clone(),
                handles: Mutex::new(vec![sim, ingest]),
                workers: Mutex::new(Vec::new()),
                candidates: candidates.clone(),
                job: job.clone(),
            }),
            to_sim,
            shared,
            candidates,
            job,
            workers: Arc::new(AtomicUsize::new(0)),
            terrain_tries,
            placement_tries,
            #[cfg(test)]
            accept,
        }
    }

    /// A handle that stops this session's threads from elsewhere ([`SessionStop`]).
    pub fn stopper(&self) -> SessionStop {
        SessionStop(Arc::downgrade(&self.threads))
    }

    fn send(&self, m: ToSim) {
        let _ = self.to_sim.send(m);
    }

    // --- Lifecycle ------------------------------------------------------------------

    /// Start drawing `count` **terrain** candidates from `spec`, in parallel: bare worlds,
    /// each checked for room for the founders ([`crate::founding::terrain_candidate`]),
    /// nothing founded. Candidate `k` takes `spec.seed + k` when a seed is given, else its
    /// own drawn one. Replaces any earlier candidates, and stops their draws and any
    /// founding in flight. Refused at once only for a spec that does not parse.
    pub fn generate_candidates(&self, spec: &WorldSpec, count: usize) -> Result<(), String> {
        let seeds: Vec<Option<u64>> = (0..count).map(|k| spec.seed.map(|s| s.wrapping_add(k as u64))).collect();
        self.generate_candidates_seeded(spec, &seeds)
    }

    /// [`Session::generate_candidates`] with each candidate's seed named: one candidate per
    /// entry, `Some(seed)` that terrain (kept whatever its lake and feasibility say, as an
    /// asked seed is), `None` a drawn one. What the picker's water slider does to bring the
    /// same terrains back at another water level. `spec.seed` is not read.
    pub fn generate_candidates_seeded(&self, spec: &WorldSpec, seeds: &[Option<u64>]) -> Result<(), String> {
        let count = seeds.len();
        let params = spec.sim_params().map_err(|e| format!("{e:#}"))?;
        self.cancel_founding();
        let mut c = self.candidates.lock().map_err(|_| "the candidate list is poisoned")?;
        c.cancel.store(true, Ordering::SeqCst);
        let cancel = Arc::new(AtomicBool::new(false));
        c.cancel = cancel.clone();
        c.batch += 1;
        let batch = c.batch;
        c.spec = Some(spec.clone());
        c.params = Some(params.clone());
        c.list = (0..count)
            .map(|index| {
                let seed = seeds[index];
                Candidate {
                    slot: Slot::Generating,
                    info: CandidateInfo {
                        index,
                        state: "generating".into(),
                        seed,
                        label: None,
                        error: None,
                        summary: None,
                        populations: None,
                        terrain: None,
                        feasible: None,
                        notes: Vec::new(),
                    },
                }
            })
            .collect();
        drop(c);
        for index in 0..count {
            let mut spec = spec.clone();
            spec.seed = seeds[index];
            let params = params.clone();
            let list = self.candidates.clone();
            let cancel = cancel.clone();
            let busy = Busy::start(&self.workers);
            let tries = self.terrain_tries;
            let h = std::thread::Builder::new()
                .name(format!("cubarium-candidate-{index}"))
                .spawn(move || {
                    let _busy = busy;
                    let go_on = || !cancel.load(Ordering::Relaxed);
                    let made = terrain_candidate(&params, &spec, tries, &go_on);
                    let Ok(mut c) = list.lock() else { return };
                    if c.batch != batch {
                        return;
                    }
                    let Some(cand) = c.list.get_mut(index) else { return };
                    if cand.info.state != "generating" {
                        return;
                    }
                    match made {
                        Ok((p, note)) => {
                            cand.info.state = "ready".into();
                            cand.info.seed = Some(note.seed);
                            cand.info.label = Some(p.label.clone());
                            cand.info.terrain = Some(format!("{:016x}", p.world.terrain_hash()));
                            cand.info.feasible = note.feasibility.as_ref().map(|a| a.accepted);
                            cand.info.notes = note.warnings;
                            cand.info.summary = Some(cubarium_observatory::query::summary(&p.world, &p.label));
                            cand.info.populations = Some(cubarium_observatory::query::populations(&p.flora, &p.fauna));
                            cand.slot = Slot::Ready(Arc::new(p));
                        }
                        Err(_) if !go_on() => {
                            cand.info.state = "cancelled".into();
                            cand.slot = Slot::Failed("cancelled".into());
                        }
                        Err(e) => {
                            cand.info.state = "failed".into();
                            cand.info.error = Some(e.clone());
                            cand.slot = Slot::Failed(e);
                        }
                    }
                })
                .map_err(|e| format!("spawning a candidate: {e}"))?;
            self.threads.track(h);
        }
        Ok(())
    }

    /// The candidates and what each is doing.
    pub fn candidates(&self) -> Vec<CandidateInfo> {
        self.candidates
            .lock()
            .map(|c| c.list.iter().map(|x| x.info.clone()).collect())
            .unwrap_or_default()
    }

    /// A whole view of a ready candidate (every chunk; a bare terrain has no organisms),
    /// for a thumbnail or to pan across before choosing. `None` while it is being drawn.
    ///
    /// Built outside the candidate list's lock, from the candidate's shared world: it walks
    /// every chunk, which on a wide world takes a while. A world view drawn by the renderer
    /// needs none of it ([`Session::candidate_ready`]).
    pub fn candidate_view(&self, index: usize) -> Option<View> {
        let p = self.candidate_source().get(index)?;
        Some(View::whole(&p.world, &p.flora, &p.fauna, 0))
    }

    /// Whether candidate `index` is ready (drawn, and its world held): what a world view
    /// needs before it renders the candidate through [`Session::candidate_source`].
    pub fn candidate_ready(&self, index: usize) -> bool {
        self.candidates
            .lock()
            .is_ok_and(|c| c.list.get(index).is_some_and(|x| matches!(x.slot, Slot::Ready(_))))
    }

    /// **Found candidate `index` on its own terrain and run it** once founded. Returns at
    /// once; the founding runs on a thread of its own and is followed through
    /// [`Session::founding`]. Its terrain is never changed: a refused placement is tried
    /// again on the same terrain ([`cubarium_voxel_sim::found::found_terrain`]), and when
    /// every try is refused the founding fails with a plain message and the candidates
    /// stay. Once founded the world is installed in place of whatever was running and the
    /// candidates are dropped. Refused at once while another founding runs.
    pub fn promote(&self, index: usize) -> Result<(), String> {
        let mut slot = self.job.lock().map_err(|_| "the founding is poisoned")?;
        if slot.as_ref().is_some_and(|j| j.info.lock().is_ok_and(|i| i.state == "founding")) {
            return Err("a world is already being founded".into());
        }
        let c = self.candidates.lock().map_err(|_| "the candidate list is poisoned")?;
        let Some(cand) = c.list.get(index) else {
            return Err(format!("no candidate {index}"));
        };
        let world = match &cand.slot {
            Slot::Generating => return Err(format!("candidate {index} is still being drawn")),
            Slot::Failed(e) => return Err(format!("candidate {index} failed: {e}")),
            Slot::Ready(p) => p.world.clone(),
        };
        let (Some(spec), Some(params), Some(seed)) = (c.spec.clone(), c.params.clone(), cand.info.seed) else {
            return Err(format!("candidate {index} has no spec"));
        };
        let batch = c.batch;
        let terrain = cand.info.terrain.clone();
        drop(c);
        let tries = self.placement_tries.max(1);
        let job = Arc::new(Job {
            info: Mutex::new(FoundingInfo {
                state: "founding".into(),
                candidate: Some(index),
                seed: Some(seed),
                terrain,
                attempts: tries,
                ..FoundingInfo::default()
            }),
            cancel: AtomicBool::new(false),
            started: Instant::now(),
            shower_typical: shower_typical(&params),
            #[cfg(test)]
            accept: self.accept,
            #[cfg(not(test))]
            accept: None,
        });
        *slot = Some(job.clone());
        drop(slot);
        let list = self.candidates.clone();
        let to_sim = self.to_sim.clone();
        let busy = Busy::start(&self.workers);
        let h = std::thread::Builder::new()
            .name(format!("cubarium-founding-{index}"))
            .spawn(move || {
                let _busy = busy;
                let outcome = found_picked(world, seed, &params, &spec, tries, &job);
                // The job's lock decides between this ending and a cancel: whichever takes
                // it first is what happened.
                let Ok(mut info) = job.info.lock() else { return };
                info.elapsed_s = job.started.elapsed().as_secs_f64();
                if info.state != "founding" || job.cancel.load(Ordering::SeqCst) {
                    info.state = "cancelled".into();
                    return;
                }
                match outcome {
                    Outcome::Founded(p) => {
                        let Ok(mut c) = list.lock() else { return };
                        if c.batch != batch {
                            info.state = "cancelled".into();
                            return;
                        }
                        c.list.clear();
                        c.batch += 1;
                        drop(c);
                        info.state = "done".into();
                        info.progress = 1.0;
                        // The founded world's own terrain: the picked one's, which a reader
                        // can check against the candidate's `terrain`.
                        info.terrain = Some(format!("{:016x}", p.world.terrain_hash()));
                        info.label = Some(p.label.clone());
                        let _ = to_sim.send(ToSim::Install(p));
                    }
                    Outcome::Refused(why) | Outcome::Failed(why) => {
                        info.state = "failed".into();
                        info.error = Some(why);
                    }
                    Outcome::Cancelled => info.state = "cancelled".into(),
                }
            })
            .map_err(|e| format!("spawning the founding: {e}"))?;
        self.threads.track(h);
        Ok(())
    }

    /// The picked candidate's founding: `state` `idle` before any, else `founding`,
    /// `done`, `failed` (with `error`) or `cancelled`, and while founding its stage,
    /// progress and placement try ([`FoundingInfo`]).
    pub fn founding(&self) -> FoundingInfo {
        let job = self.job.lock().ok().and_then(|j| j.clone());
        match job {
            None => FoundingInfo { state: "idle".into(), ..FoundingInfo::default() },
            Some(j) => {
                let mut info = j.info.lock().map(|i| i.clone()).unwrap_or_default();
                if info.state == "founding" {
                    info.elapsed_s = j.started.elapsed().as_secs_f64();
                }
                info
            }
        }
    }

    /// **Stop what is being made**: the candidates still being drawn (they become
    /// `cancelled`; ready ones stay) and the founding in flight (`cancelled`, nothing
    /// installed). Their threads stop at the next draw or the next tick.
    pub fn cancel(&self) {
        if let Ok(mut c) = self.candidates.lock() {
            c.cancel.store(true, Ordering::SeqCst);
            for cand in c.list.iter_mut().filter(|x| x.info.state == "generating") {
                cand.info.state = "cancelled".into();
                cand.slot = Slot::Failed("cancelled".into());
            }
        }
        self.cancel_founding();
    }

    fn cancel_founding(&self) {
        let job = self.job.lock().ok().and_then(|j| j.clone());
        if let Some(j) = job {
            j.cancel.store(true, Ordering::SeqCst);
            if let Ok(mut info) = j.info.lock()
                && info.state == "founding"
            {
                info.state = "cancelled".into();
                info.elapsed_s = j.started.elapsed().as_secs_f64();
            }
        }
    }

    /// Candidate and founding threads still running (a cancelled one until it notices).
    pub fn workers(&self) -> usize {
        self.workers.load(Ordering::SeqCst)
    }

    /// Install a world made elsewhere (a tool, a test) in place of whatever was running.
    pub fn install(&self, prepared: Prepared) {
        self.send(ToSim::Install(Box::new(prepared)));
    }

    /// Load the save in `dir` in place of whatever is running. Returns at once; the world
    /// changes when it has loaded ([`Status::generation`]), and a failure — a save from
    /// another version is refused with a plain error — leaves the running world alone and
    /// is told as a `load_failed` notice.
    pub fn load(&self, dir: &Path) {
        self.shared.loading.store(true, Ordering::SeqCst);
        self.send(ToSim::Load(dir.to_path_buf(), None));
    }

    /// [`Session::load`], waiting up to `timeout` for the outcome.
    pub fn load_blocking(&self, dir: &Path, timeout: Duration) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.shared.loading.store(true, Ordering::SeqCst);
        self.send(ToSim::Load(dir.to_path_buf(), Some(tx)));
        rx.recv_timeout(timeout).map_err(|_| "the load did not finish in time".to_string())?
    }

    /// Save the whole world into `dir` (made if missing), with its `meta.json`. Returns at
    /// once; the world is copied at the loop's next command point and written off it, and
    /// the outcome is told as a `saved` or `save_failed` notice.
    pub fn save(&self, dir: &Path) {
        self.send(ToSim::Save(dir.to_path_buf(), None));
    }

    /// [`Session::save`], waiting up to `timeout` for the write.
    pub fn save_blocking(&self, dir: &Path, timeout: Duration) -> Result<SaveMeta, String> {
        let (tx, rx) = mpsc::channel();
        self.send(ToSim::Save(dir.to_path_buf(), Some(tx)));
        rx.recv_timeout(timeout).map_err(|_| "the save did not finish in time".to_string())?
    }

    /// Stop and drop the running world; the session stays, empty.
    pub fn close(&self) {
        self.send(ToSim::Close);
    }

    /// Follow the running world on the loop's thread ([`LoopHook`]); `None` removes the
    /// hook (it is dropped on the loop). A hook set while a world runs is told about it at
    /// once ([`LoopHook::installed`]).
    pub fn set_hook(&self, hook: Option<Box<dyn LoopHook>>) {
        self.send(ToSim::Hook(hook));
    }

    /// A handle on the candidates that outlives this borrow: a renderer reads a candidate
    /// world through it from a thread of its own ([`CandidateSource::with`]).
    pub fn candidate_source(&self) -> CandidateSource {
        CandidateSource(self.candidates.clone())
    }

    // --- Run control ----------------------------------------------------------------

    /// A run command, the dashboard's and the console's own.
    pub fn command(&self, command: cubarium_observatory::Command) {
        self.send(ToSim::Line(console_line(command)));
    }

    /// A console line ([`crate::COMMANDS`]); its reply comes back as a `console` notice.
    pub fn console(&self, line: &str) {
        self.send(ToSim::Line(line.to_string()));
    }

    // --- Reading --------------------------------------------------------------------

    /// The run as it stands.
    pub fn status(&self) -> Status {
        let s = &self.shared;
        let (label, last_error) = s.text.lock().map(|t| t.clone()).unwrap_or_default();
        let steps = s.steps.load(Ordering::Relaxed).max(1);
        let feeds = s.feed_updates.load(Ordering::Relaxed);
        Status {
            running: s.running.load(Ordering::SeqCst),
            paused: s.paused.load(Ordering::SeqCst),
            speed: f64::from_bits(s.speed_bits.load(Ordering::SeqCst)),
            tick: s.tick.load(Ordering::SeqCst),
            stepped: s.stepped.load(Ordering::SeqCst),
            generation: s.generation.load(Ordering::SeqCst),
            loading: s.loading.load(Ordering::SeqCst),
            saving: s.saving.load(Ordering::SeqCst),
            label,
            last_error,
            step_ms: s.step_ns.load(Ordering::Relaxed) as f64 / steps as f64 / 1e6,
            feed_ms: s.feed_ns.load(Ordering::Relaxed) as f64 / feeds.max(1) as f64 / 1e6,
            feed_updates: feeds,
        }
    }

    /// The chunks that changed since the last take — every chunk on the first take after a
    /// world is installed — and the newest frame.
    pub fn take_view(&self) -> View {
        let Ok(mut h) = self.shared.hub.lock() else {
            return View::default();
        };
        let chunks = std::mem::take(&mut h.chunks);
        let full = std::mem::replace(&mut h.full, false);
        let view = View {
            generation: h.generation,
            dims: h.dims,
            chunk_dims: h.chunk_dims,
            voxel_m: h.voxel_m,
            full,
            chunks: Vec::new(),
            frame: h.frame.clone(),
        };
        drop(h);
        let mut list: Vec<(u32, ChunkData)> = chunks.into_iter().collect();
        list.sort_unstable_by_key(|(k, _)| *k);
        View { chunks: list.into_iter().map(|(_, c)| c).collect(), ..view }
    }

    /// The life events (birth, meal, kill, injury, death and the rest) since the last take,
    /// oldest first.
    pub fn take_events(&self) -> Vec<LifeEvent> {
        self.shared
            .hub
            .lock()
            .map(|mut h| h.events.drain(..).collect())
            .unwrap_or_default()
    }

    /// Console replies and save and load outcomes since the last take, oldest first.
    pub fn take_notices(&self) -> Vec<Notice> {
        self.shared
            .hub
            .lock()
            .map(|mut h| h.notices.drain(..).collect())
            .unwrap_or_default()
    }

    /// **A query** over the latest published snapshot, as a serde value:
    /// `summary`, `clock`, `environment`, `populations`, `run`, `snapshot`, `animals`,
    /// `animal` (`{"id": N}`: the body and its life history), `history`
    /// (`{"since": tick}` optional), `species`, `status`, `candidates`, `founding`, `saves`
    /// (`{"dir": path}`: the save slots under it, read without loading any), `presets`
    /// (the landform and climate presets a [`WorldSpec`] can name).
    pub fn query(&self, name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "species" => return Ok(crate::species_table()),
            "presets" => return Ok(crate::params::preset_table()),
            "status" => return Ok(to(&self.status())),
            "candidates" => return Ok(to(&self.candidates())),
            "founding" => return Ok(to(&self.founding())),
            "spec" => return spec_info(args),
            "saves" => {
                let dir = args.get("dir").and_then(Value::as_str).ok_or("`saves` wants {\"dir\": path}")?;
                return Ok(to(&store::list_saves(Path::new(dir))));
            }
            _ => {}
        }
        let h = self.shared.hub.lock().map_err(|_| "the session hub is poisoned")?;
        match name {
            "history" => {
                let since = args.get("since").and_then(Value::as_u64);
                let samples: Vec<&Sample> = h.history.since(since).collect();
                return Ok(json!({ "config": h.history.config(), "samples": to(&samples) }));
            }
            "animal" => {
                let id = args.get("id").and_then(Value::as_u64).ok_or("`animal` wants {\"id\": N}")?;
                let life = h.book.record(id).cloned();
                let snap = h.snapshot.clone();
                drop(h);
                let body = snap.as_ref().and_then(|s| s.animals.iter().find(|a| a.row.id == id).cloned());
                if body.is_none() && life.is_none() {
                    return Err(format!("no animal {id} is alive or remembered"));
                }
                return Ok(json!({ "animal": to(&body), "life": to(&life) }));
            }
            _ => {}
        }
        let snap = h.snapshot.clone().ok_or("nothing has been published yet")?;
        drop(h);
        Ok(match name {
            "summary" => to(&snap.summary),
            "clock" => to(&snap.clock),
            "environment" => to(&snap.environment),
            "populations" => to(&snap.populations),
            "run" => to(&snap.run),
            "animals" => to(&snap.animals),
            "snapshot" => to(&*snap),
            other => {
                return Err(format!(
                    "unknown query `{other}`; known: summary clock environment populations run \
                     snapshot animals animal history species status candidates founding spec saves presets"
                ));
            }
        })
    }

    /// The tick of the frame waiting for the next take, without taking it.
    #[cfg(test)]
    pub(crate) fn pending_frame_tick(&self) -> Option<u64> {
        self.shared.hub.lock().ok()?.frame.as_ref().map(|f| f.tick)
    }

    #[cfg(test)]
    pub(crate) fn hold_hub_for(&self, d: Duration) {
        let _h = self.shared.hub.lock();
        std::thread::sleep(d);
    }
}

/// A handle on a session's candidates ([`Session::candidate_source`]).
#[derive(Clone)]
pub struct CandidateSource(Arc<Mutex<Candidates>>);

impl CandidateSource {
    /// Ready candidate `index`'s world, shared: the list's lock is held only to clone the
    /// handle, so a reader may take as long as it likes over it (open a renderer, pack it)
    /// while the picker polls [`Session::candidates`] and other draws file their results.
    /// `None` while it is being drawn, after it failed, or once the list moved on (a new
    /// batch, a promotion); a world already handed out stays valid for its holder.
    pub fn get(&self, index: usize) -> Option<Arc<Prepared>> {
        let c = self.0.lock().ok()?;
        match &c.list.get(index)?.slot {
            Slot::Ready(p) => Some(p.clone()),
            _ => None,
        }
    }

    /// Run `f` over ready candidate `index`'s layers ([`CandidateSource::get`]), without the
    /// list's lock held.
    pub fn with<R>(&self, index: usize, f: impl FnOnce(&World, &Flora, &Fauna) -> R) -> Option<R> {
        let p = self.get(index)?;
        Some(f(&p.world, &p.flora, &p.fauna))
    }
}

/// Query `spec`: what a [`WorldSpec`] (the query's argument) resolves to, for a picker
/// setting its controls — the landform's generator (`terrarium`, `staged`, `ridge`), its
/// water table when it has one (`water_level_m`), and the water steps a picker offers
/// ([`crate::params::WATER_LEVELS`]).
fn spec_info(args: &Value) -> Result<Value, String> {
    let spec: WorldSpec = serde_json::from_value(args.clone()).map_err(|e| format!("the world spec: {e}"))?;
    let params = spec.sim_params().map_err(|e| format!("{e:#}"))?;
    let (landform, level) = match &params.world.landform {
        cubarium_voxel::Landform::Terrarium(t) => ("terrarium", Some(t.water_level_m)),
        cubarium_voxel::Landform::Staged(_) => ("staged", None),
        cubarium_voxel::Landform::Ridge => ("ridge", None),
    };
    let steps: Vec<Value> = crate::params::WATER_LEVELS
        .iter()
        .map(|(name, m)| json!({ "name": name, "water_level_m": m }))
        .collect();
    Ok(json!({
        "landform": landform,
        "water_level_m": level,
        "water_levels": steps,
        "size": [params.world.width, params.world.height, params.world.depth],
    }))
}

/// A serde value of `v` (`null` for one that does not serialize).
fn to<T: Serialize + ?Sized>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

impl Drop for Session {
    fn drop(&mut self) {
        self.cancel();
        self.threads.stop();
    }
}

/// Found a new world from a spec, seeded and driven, ready to run.
pub fn prepare_new(params: &SimParams, spec: &WorldSpec) -> anyhow::Result<Prepared> {
    let p = params.resolve_sets(&spec.sets)?;
    let recipes = founder_recipes(&[], &[])?;
    let founder_counts = p.fauna.founder_counts();
    let start = StartSpec {
        load: None,
        scene: Scene::Generated,
        seed: spec.seed,
        empty: spec.empty,
        founder_counts,
    };
    let (world, label, resumed, layers) = create::load_or_create_world(&p, &start)?;
    let (world, flora, fauna) = create::populate(
        world,
        resumed,
        layers,
        &p,
        &Populate { recipes: &recipes, introductions: &[], empty: spec.empty, founder_counts, no_egg_bank: false },
    )?;
    Ok(Prepared { world, flora, fauna, label, exec: params.clone() })
}

/// A save read back, seeded with its own layers and its drivers put back.
pub fn prepare_loaded(exec: &SimParams, dir: &Path) -> Result<Prepared, String> {
    let loaded = store::load_save(dir).map_err(|e| e.to_string())?;
    let p = exec.resolve_sets(&[]).map_err(|e| format!("{e:#}"))?;
    let recipes = founder_recipes(&[], &[]).map_err(|e| format!("{e:#}"))?;
    let (world, flora, fauna) = create::populate(
        loaded.world,
        true,
        Some(StartLayers::Restored(loaded.flora, loaded.fauna)),
        &p,
        &Populate {
            recipes: &recipes,
            introductions: &[],
            empty: false,
            founder_counts: [0; Founder::COUNT],
            no_egg_bank: false,
        },
    )
    .map_err(|e| format!("{e:#}"))?;
    Ok(Prepared { world, flora, fauna, label: format!("loaded {}", dir.display()), exec: exec.clone() })
}

// --- The loop ---------------------------------------------------------------------------

struct Run {
    sim: Sim,
    console: Console,
    pacer: Pacer,
    observer: Observer,
    tracker: Option<Tracker>,
    events: Vec<LifeEvent>,
    /// The world or its layers moved since the last feed update.
    moved: bool,
    step_ns: u64,
    steps: u64,
    feed_ns: u64,
    feed_updates: u64,
}

struct Loop {
    options: SessionOptions,
    out: Sender<ToReader>,
    shared: Arc<Shared>,
    run: Option<Run>,
    generation: u64,
    speed: f64,
    /// A world installed over a paused one starts paused (a load from a pause menu stays
    /// paused until the menu resumes it); the first world runs.
    paused: bool,
    writer: Option<JoinHandle<()>>,
    next: Instant,
    /// What follows the world on this thread ([`Session::set_hook`]), and whether the world
    /// moved since it was last called.
    hook: Option<Box<dyn LoopHook>>,
    hook_moved: bool,
    #[cfg(test)]
    after_tick: Option<Box<dyn FnMut(u64) + Send>>,
}

const PERIOD: Duration = Duration::from_millis(1000 / cubarium_voxel::TICK_HZ as u64);

impl Loop {
    fn new(options: SessionOptions, out: Sender<ToReader>, shared: Arc<Shared>) -> Loop {
        let speed = options.speed.clamp(MIN_SPEED, MAX_SPEED);
        Loop {
            options,
            out,
            shared,
            run: None,
            generation: 0,
            speed,
            paused: false,
            writer: None,
            next: Instant::now(),
            hook: None,
            hook_moved: false,
            #[cfg(test)]
            after_tick: None,
        }
    }

    fn main(mut self, rx: Receiver<ToSim>) {
        while self.advance(&rx) {}
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
    }

    /// One pass through the command point and the clock.
    fn advance(&mut self, rx: &Receiver<ToSim>) -> bool {
        let first = if self.run.is_none() {
            match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => return false,
            }
        } else {
            let now = Instant::now();
            if now < self.next {
                match rx.recv_timeout(self.next - now) {
                    Ok(m) => Some(m),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return false,
                }
            } else {
                None
            }
        };
        if !self.commands(rx, first) {
            return false;
        }
        if self.run.is_some() && Instant::now() >= self.next {
            return self.period(rx);
        }
        true
    }

    /// Drain even when no blocking receive was needed (including while catching up).
    fn commands(&mut self, rx: &Receiver<ToSim>, first: Option<ToSim>) -> bool {
        if let Some(m) = first {
            if !self.handle(m) {
                return false;
            }
        }
        while let Ok(m) = rx.try_recv() {
            if !self.handle(m) {
                return false;
            }
        }
        true
    }

    /// One clock period: the ticks it owes, the feed, a publication if one is due.
    fn period(&mut self, rx: &Receiver<ToSim>) -> bool {
        let now = Instant::now();
        self.next += PERIOD;
        if self.next + Duration::from_secs(1) < now {
            // Behind by more than a second (a slow machine at a high speed): re-base rather
            // than chase a schedule that will never be met.
            self.next = now + PERIOD;
        }
        let Some(r) = self.run.as_mut() else {
            return true;
        };
        r.pacer.begin(&mut r.console);
        let mut stepped = 0u64;
        loop {
            if !self.commands(rx, None) {
                return false;
            }
            let Some(r) = self.run.as_mut() else { break };
            if !r.pacer.take_tick() {
                break;
            }
            let at = Instant::now();
            r.sim.step();
            let ns = at.elapsed().as_nanos() as u64;
            r.step_ns += ns;
            r.steps += 1;
            r.observer.after_tick(r.sim.world(), r.sim.fauna(), ns);
            r.events.extend(r.observer.take_events());
            self.shared
                .tick
                .store(r.sim.world().tick(), Ordering::SeqCst);
            self.shared.stepped.fetch_add(1, Ordering::SeqCst);
            r.moved = true;
            self.hook_moved = true;
            stepped += 1;
            #[cfg(test)]
            if let Some(hook) = &mut self.after_tick {
                hook(r.sim.world().tick());
            }
        }
        self.feed();
        self.call_hook(stepped);
        self.publish(false);
        self.store_status();
        true
    }

    /// The hook's period call, with the world borrowed.
    fn call_hook(&mut self, stepped: u64) {
        let (Some(hook), Some(r)) = (self.hook.as_mut(), self.run.as_ref()) else { return };
        let clock = HookClock {
            tick: r.sim.world().tick(),
            stepped,
            speed: r.console.speed,
            paused: r.console.paused,
        };
        let (world, flora, fauna) = r.sim.layers();
        hook.period(world, flora, fauna, std::mem::take(&mut self.hook_moved), clock);
    }

    /// Tell the hook about the world now running.
    fn hook_installed(&mut self) {
        let (Some(hook), Some(r)) = (self.hook.as_mut(), self.run.as_ref()) else { return };
        let clock = HookClock { tick: r.sim.world().tick(), stepped: 0, speed: r.console.speed, paused: r.console.paused };
        let (world, flora, fauna) = r.sim.layers();
        hook.installed(world, flora, fauna, clock);
        self.hook_moved = false;
    }

    fn feed(&mut self) {
        let Some(r) = self.run.as_mut() else { return };
        if !r.moved && r.events.is_empty() {
            return;
        }
        let at = Instant::now();
        let mut chunks = Vec::new();
        let mut frame = None;
        if r.moved
            && let Some(t) = r.tracker.as_mut()
        {
            let (world, flora, fauna) = r.sim.layers();
            t.update(world);
            t.update_surfaces(world, flora);
            chunks = t.take_dirty();
            frame = Some(Arc::new(Frame::of(world, flora, fauna)));
            r.feed_ns += at.elapsed().as_nanos() as u64;
            r.feed_updates += 1;
        }
        r.moved = false;
        let events = std::mem::take(&mut r.events);
        let _ = self.out.send(ToReader::Feed { chunks, frame, events });
    }

    fn publish(&mut self, force: bool) {
        let Some(r) = self.run.as_mut() else { return };
        if force {
            r.observer.request();
        }
        let now = Instant::now();
        if r.observer.due(r.sim.world().tick(), now, r.console.paused) {
            let controls = Controls { speed: r.console.speed, paused: r.console.paused, min_speed: MIN_SPEED, max_speed: MAX_SPEED };
            let (world, flora, fauna) = r.sim.layers();
            let p = r.observer.publish(world, flora, fauna, controls, now);
            // The observer's events are taken tick by tick for the feed, which files them
            // into the life book; what a publication still carries (the bodies standing
            // when the record began) goes the same way, with the next feed.
            r.events.extend(p.events);
            let _ = self.out.send(ToReader::Publication(Box::new(p.snapshot)));
        }
    }

    fn store_status(&self) {
        let s = &self.shared;
        match &self.run {
            Some(r) => {
                s.running.store(true, Ordering::SeqCst);
                s.paused.store(r.console.paused, Ordering::SeqCst);
                s.speed_bits.store(r.console.speed.to_bits(), Ordering::SeqCst);
                s.tick.store(r.sim.world().tick(), Ordering::SeqCst);
                s.step_ns.store(r.step_ns, Ordering::Relaxed);
                s.steps.store(r.steps, Ordering::Relaxed);
                s.feed_ns.store(r.feed_ns, Ordering::Relaxed);
                s.feed_updates.store(r.feed_updates, Ordering::Relaxed);
            }
            None => s.running.store(false, Ordering::SeqCst),
        }
        s.saving.store(self.writer.as_ref().is_some_and(|w| !w.is_finished()), Ordering::SeqCst);
    }

    fn notice(&self, n: Notice) {
        let _ = self.out.send(ToReader::Notice(n));
    }

    /// Apply one message; `false` ends the loop.
    fn handle(&mut self, m: ToSim) -> bool {
        match m {
            ToSim::Shutdown => return false,
            ToSim::Install(p) => self.install(*p),
            ToSim::Load(dir, reply) => {
                let made = prepare_loaded(&self.options.exec, &dir);
                self.shared.loading.store(false, Ordering::SeqCst);
                let result = match made {
                    Ok(p) => {
                        self.install(p);
                        self.notice(Notice { path: Some(dir.display().to_string()), ..Notice::say("loaded", "loaded") });
                        Ok(())
                    }
                    Err(e) => {
                        let _ = self.out.send(ToReader::Error(Some(e.clone())));
                        self.notice(Notice { path: Some(dir.display().to_string()), ..Notice::say("load_failed", e.clone()) });
                        Err(e)
                    }
                };
                if let Some(tx) = reply {
                    let _ = tx.send(result);
                }
            }
            ToSim::Save(dir, reply) => self.save(dir, reply),
            ToSim::Hook(hook) => {
                self.hook = hook;
                self.hook_installed();
            }
            ToSim::Close => {
                self.run = None;
                if let Some(h) = self.hook.as_mut() {
                    h.closed();
                }
                let _ = self.out.send(ToReader::Closed);
                self.notice(Notice::say("closed", "closed"));
                self.store_status();
            }
            ToSim::Line(line) => {
                let Some(r) = self.run.as_mut() else {
                    self.notice(Notice::say("console", "no world is running"));
                    return true;
                };
                let Run { sim, console, pacer, observer, .. } = r;
                sim.with_layers_mut(|world, flora, fauna| console.handle(world, flora, fauna, &line));
                if pacer.sync(console) {
                    self.next = Instant::now() + PERIOD;
                }
                observer.request();
                r.moved = true;
                self.hook_moved = true;
                let said = r.console.take_said();
                let quit = std::mem::take(&mut r.console.quit);
                let save = r.console.save_to.take();
                for text in said {
                    self.notice(Notice::say("console", text));
                }
                if let Some(dir) = save {
                    self.save(dir, None);
                }
                if quit {
                    return self.handle(ToSim::Close);
                }
                self.store_status();
            }
        }
        true
    }

    fn install(&mut self, p: Prepared) {
        if let Some(r) = &self.run {
            self.speed = r.console.speed;
            self.paused = r.console.paused;
        }
        self.run = None;
        let c = p.world.config();
        let dims = (c.width, c.height, c.depth);
        let voxel_m = c.voxel_m;
        let mut sim = create::ready_sim(p.world, p.flora, p.fauna, p.exec.sim_config());
        create::set_water_every(&mut sim, p.exec.water_every);
        create::open_closed_outlet(&mut sim);
        self.generation += 1;
        let mut tracker = self.options.feed.then(|| Tracker::new(sim.world(), self.options.feed_config));
        let chunk_dims = tracker.as_ref().map_or([0; 3], |t| t.dims().1);
        // The first feed travels with the reset, so no reader can take the new world's
        // generation without its chunks.
        let (chunks, frame) = match tracker.as_mut() {
            Some(t) => {
                let (world, flora, fauna) = sim.layers();
                t.update_surfaces(world, flora);
                (t.take_dirty(), Some(Arc::new(Frame::of(world, flora, fauna))))
            }
            None => (Vec::new(), None),
        };
        let _ = self.out.send(ToReader::Reset {
            generation: self.generation,
            label: p.label.clone(),
            dims: [dims.0, dims.1, dims.2],
            chunk_dims,
            voxel_m,
            history: p.exec.observatory,
            chunks,
            frame,
        });
        let mut console = Console::new(self.speed, dims);
        console.paused = self.paused;
        console.default_save_dir = crate::console::tick_save_dir;
        let pacer = Pacer::new(&console);
        let observer = Observer::new(p.label, self.options.cadence);
        self.shared.tick.store(sim.world().tick(), Ordering::SeqCst);
        self.run = Some(Run {
            sim,
            console,
            pacer,
            observer,
            tracker,
            events: Vec::new(),
            moved: false,
            step_ns: 0,
            steps: 0,
            feed_ns: 0,
            feed_updates: 0,
        });
        self.shared.generation.store(self.generation, Ordering::SeqCst);
        self.hook_installed();
        // The first publication at once, whatever the speed or pause.
        self.publish(true);
        self.store_status();
        self.next = Instant::now() + PERIOD;
    }

    fn save(&mut self, dir: PathBuf, reply: Option<Sender<Result<SaveMeta, String>>>) {
        let refuse = |why: String, this: &Loop| {
            this.notice(Notice { path: Some(dir.display().to_string()), ..Notice::say("save_failed", why.clone()) });
            if let Some(tx) = &reply {
                let _ = tx.send(Err(why));
            }
        };
        let Some(r) = &self.run else {
            refuse("no world is running".into(), self);
            return;
        };
        if self.writer.as_ref().is_some_and(|w| !w.is_finished()) {
            refuse("the last save is still writing; ask again".into(), self);
            return;
        }
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
        let (world, flora, fauna) = r.sim.layers();
        let (world, flora, fauna) = (world.clone(), flora.clone(), fauna.clone());
        let out = self.out.clone();
        self.writer = Some(std::thread::spawn(move || {
            let result = store::save_whole(&dir, &world, &flora, &fauna).map_err(|e| format!("{e:#}"));
            let path = Some(dir.display().to_string());
            let notice = match &result {
                Ok(meta) => Notice { path, meta: Some(meta.clone()), ..Notice::say("saved", "saved") },
                Err(e) => Notice { path, ..Notice::say("save_failed", e.clone()) },
            };
            let _ = out.send(ToReader::Notice(notice));
            if let Some(tx) = reply {
                let _ = tx.send(result);
            }
        }));
        self.store_status();
    }
}

fn ingest_main(rx: Receiver<ToReader>, shared: &Shared, max_events: usize) {
    while let Ok(m) = rx.recv() {
        let Ok(mut h) = shared.hub.lock() else { return };
        match m {
            ToReader::Reset { generation, label, dims, chunk_dims, voxel_m, history, chunks, frame } => {
                if let Ok(mut t) = shared.text.lock() {
                    *t = (label, None);
                }
                h.generation = generation;
                h.snapshot = None;
                h.book = LifeBook::default();
                h.history = History::new(history);
                h.dims = dims;
                h.chunk_dims = chunk_dims;
                h.voxel_m = voxel_m;
                h.full = true;
                h.chunks.clear();
                for c in chunks {
                    let k = c.at[0] + chunk_dims[0] * (c.at[1] + chunk_dims[1] * c.at[2]);
                    h.chunks.insert(k, c);
                }
                h.frame = frame;
                h.events.clear();
            }
            ToReader::Closed => {
                h.snapshot = None;
                h.chunks.clear();
                h.frame = None;
                h.full = false;
                h.dims = [0; 3];
                h.chunk_dims = [0; 3];
                if let Ok(mut t) = shared.text.lock() {
                    t.0.clear();
                }
            }
            ToReader::Feed { chunks, frame, events } => {
                for c in chunks {
                    let k = c.at[0] + h.chunk_dims[0] * (c.at[1] + h.chunk_dims[1] * c.at[2]);
                    h.chunks.insert(k, c);
                }
                if frame.is_some() {
                    h.frame = frame;
                }
                h.book.apply(&events);
                h.events.extend(events);
                while h.events.len() > max_events {
                    h.events.pop_front();
                    h.dropped_events += 1;
                }
            }
            ToReader::Publication(s) => {
                let sample = Sample::of(&s.clock, &s.populations, &s.environment);
                h.history.offer(sample);
                h.snapshot = Some(Arc::from(s));
            }
            ToReader::Notice(n) => {
                h.notices.push_back(n);
                while h.notices.len() > 256 {
                    h.notices.pop_front();
                }
            }
            ToReader::Error(e) => {
                if let Ok(mut t) = shared.text.lock() {
                    t.1 = e;
                }
            }
        }
    }
}

#[cfg(test)]
mod command_point_tests {
    use super::*;

    fn tiny_loop() -> (Loop, Sender<ToSim>, Receiver<ToSim>, Receiver<ToReader>) {
        let options = SessionOptions {
            feed: false,
            speed: 8.0,
            ..SessionOptions::default()
        };
        let session = Session::new(options.clone());
        let (out, reader) = mpsc::channel();
        let mut host = Loop::new(options, out, session.shared.clone());
        drop(session);
        host.install(Prepared {
            world: World::empty(cubarium_voxel::Config {
                width: 8,
                height: 4,
                depth: 2,
                ..Default::default()
            }),
            flora: Flora::new(Default::default()),
            fauna: Fauna::new(cubarium_voxel_fauna::FaunaConfig::placeholder()),
            label: "command point test".into(),
            exec: SimParams {
                threads: 1,
                ..Default::default()
            },
        });
        let (tx, rx) = mpsc::channel();
        (host, tx, rx, reader)
    }

    fn behind(host: &mut Loop) {
        // Model the clock after an unsustainable step without sleeping or depending on
        // this machine's throughput. It is still behind after the next clock period.
        host.next = Instant::now() - PERIOD * 4;
    }

    #[test]
    fn catchup_applies_pause_speed_save_and_step_at_the_next_command_point() {
        let (mut host, tx, rx, _reader) = tiny_loop();
        let tick = host.run.as_ref().unwrap().sim.world().tick();
        behind(&mut host);
        tx.send(ToSim::Line(console_line(
            cubarium_observatory::Command::Pause,
        )))
        .unwrap();
        assert!(host.advance(&rx));
        let r = host.run.as_ref().unwrap();
        assert!(r.console.paused, "pause was starved by catch-up");
        assert_eq!(
            r.sim.world().tick(),
            tick,
            "pause stops at the command point"
        );

        behind(&mut host);
        tx.send(ToSim::Line("speed 0.5".into())).unwrap();
        assert!(host.advance(&rx));
        assert_eq!(host.run.as_ref().unwrap().console.speed, 0.5);

        let dir = std::env::temp_dir().join(format!("cubarium-command-point-paused-save-{}", std::process::id()));
        let (reply, saved) = mpsc::channel();
        behind(&mut host);
        tx.send(ToSim::Save(dir.clone(), Some(reply))).unwrap();
        assert!(host.advance(&rx));
        host.writer.take().expect("save follows pause and speed at its command point").join().unwrap();
        assert_eq!(saved.try_recv().unwrap().unwrap().tick, tick);
        std::fs::remove_dir_all(dir).unwrap();

        behind(&mut host);
        tx.send(ToSim::Line(console_line(cubarium_observatory::Command::Resume))).unwrap();
        let command_at = Instant::now();
        assert!(host.advance(&rx));
        assert!(!host.run.as_ref().unwrap().console.paused);
        assert_eq!(host.run.as_ref().unwrap().sim.world().tick(), tick, "resume drops the backlog");
        assert!(host.next > command_at);

        behind(&mut host);
        tx.send(ToSim::Line("speed 2".into())).unwrap();
        let command_at = Instant::now();
        assert!(host.advance(&rx));
        assert_eq!(host.run.as_ref().unwrap().console.speed, 2.0);
        assert_eq!(host.run.as_ref().unwrap().sim.world().tick(), tick, "speed drops the backlog");
        assert!(host.next > command_at);

        behind(&mut host);
        tx.send(ToSim::Line(console_line(
            cubarium_observatory::Command::Step(1),
        )))
        .unwrap();
        assert!(host.advance(&rx));
        assert!(host.run.as_ref().unwrap().console.paused);
        behind(&mut host);
        assert!(host.advance(&rx));
        assert_eq!(host.run.as_ref().unwrap().sim.world().tick(), tick + 1);
    }

    #[test]
    fn catchup_commands_interrupt_a_speed_batch_within_one_tick() {
        for lines in [vec!["p on"], vec!["speed 0.5"], vec!["s 1"], vec!["p on", "p off"]] {
            let (mut host, tx, rx, _reader) = tiny_loop();
            let tick = host.run.as_ref().unwrap().sim.world().tick();
            host.after_tick = Some(Box::new(move |_| {
                for line in &lines {
                    tx.send(ToSim::Line((*line).into())).unwrap();
                }
            }));
            behind(&mut host);
            let command_at = Instant::now();
            assert!(host.advance(&rx));
            assert_eq!(host.run.as_ref().unwrap().sim.world().tick(), tick + 1, "the command interrupts the batch");
            assert!(host.next > command_at, "the clock has no stale backlog");
        }
    }

    #[test]
    fn catchup_save_inside_a_speed_batch_captures_within_one_tick() {
        let (mut host, tx, rx, _reader) = tiny_loop();
        let dir = std::env::temp_dir().join(format!("cubarium-command-point-batch-save-{}", std::process::id()));
        let (reply, saved) = mpsc::channel();
        let tick = host.run.as_ref().unwrap().sim.world().tick();
        let target = dir.clone();
        host.after_tick = Some(Box::new(move |now| {
            if now == tick + 1 {
                tx.send(ToSim::Save(target.clone(), Some(reply.clone()))).unwrap();
            }
        }));
        behind(&mut host);
        assert!(host.advance(&rx));
        host.writer.take().expect("the save reached its command point").join().unwrap();
        assert_eq!(saved.try_recv().unwrap().unwrap().tick, tick + 1);
        assert!(host.run.as_ref().unwrap().sim.world().tick() > tick + 1, "catch-up continues after a save");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn catchup_save_captures_the_next_command_point() {
        let (mut host, tx, rx, _reader) = tiny_loop();
        let dir = std::env::temp_dir().join(format!(
            "cubarium-command-point-save-{}",
            std::process::id()
        ));
        let (reply, saved) = mpsc::channel();
        let tick = host.run.as_ref().unwrap().sim.world().tick();
        behind(&mut host);
        tx.send(ToSim::Save(dir.clone(), Some(reply))).unwrap();
        assert!(host.advance(&rx));
        assert!(host.writer.is_some(), "save was starved by catch-up");
        host.writer.take().unwrap().join().unwrap();
        let meta = saved.try_recv().unwrap().unwrap();
        assert_eq!(meta.tick, tick, "the save precedes the next tick");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod surface_union_tests {
    use super::*;
    use crate::feed_tests::{deposit, flora, world};
    use cubarium_voxel_flora::DepositKind;

    /// Feed messages use exactly the terrain's union, including an empty replacement.
    #[test]
    fn surface_updates_between_takes_union_and_latest_removal_wins() {
        let session = Session::new(SessionOptions::default());
        let w = world();
        let mut f = flora();
        let mut t = Tracker::new(&w, FeedConfig::default());
        let (tx, rx) = mpsc::channel();
        tx.send(ToReader::Reset {
            generation: 1,
            label: "fixture".into(),
            dims: t.dims().0,
            chunk_dims: t.dims().1,
            voxel_m: w.config().voxel_m,
            history: HistoryConfig::default(),
            chunks: t.take_dirty(),
            frame: None,
        })
        .unwrap();
        drop(tx);
        ingest_main(rx, &session.shared, 16);
        assert!(session.take_view().full);

        let (tx, rx) = mpsc::channel();
        for x in [3, 20] {
            deposit(&mut f, x, DepositKind::DeadWood, 1.0);
            t.update_surfaces(&w, &f);
            tx.send(ToReader::Feed { chunks: t.take_dirty(), frame: None, events: Vec::new() }).unwrap();
        }
        // Remove the first log after it was queued, before the reader takes anything.
        let mut state = toml::Value::try_from(&f).unwrap();
        state["ground"][0]["dead_wood"] = toml::Value::Float(0.0);
        f = state.try_into().unwrap();
        t.update_surfaces(&w, &f);
        tx.send(ToReader::Feed { chunks: t.take_dirty(), frame: None, events: Vec::new() }).unwrap();
        drop(tx);
        ingest_main(rx, &session.shared, 16);
        let view = session.take_view();
        assert!(!view.full);
        assert_eq!(view.chunks.len(), 2);
        assert!(view.chunks[0].marks.is_empty(), "empty list clears the earlier mark");
        assert_eq!(view.chunks[1].marks.len(), 1, "the other chunk was retained");
        assert!(session.take_view().chunks.is_empty());
    }
}
