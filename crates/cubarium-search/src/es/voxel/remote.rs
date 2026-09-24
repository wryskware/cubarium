//! **Remote episode workers** (package S item 3,
//! `design/handoffs/voxel-training-speed-2026-09-23.md`): a second machine takes part
//! of every generation over an ssh pipe.
//!
//! `voxel-train ... --remote eidolon.local:12` starts
//! `ssh eidolon.local ~/cubarium-train/cubarium-search voxel-eval-worker --threads 12`
//! and speaks newline-delimited JSON over the pipe: no open ports, and ssh owns the
//! authentication. Protocol lines carry the [`PREFIX`]; anything else the worker prints
//! is passed through to this process's stderr under the remote's name.
//!
//! # What makes a remote result the same as a local one
//!
//! - **The same build.** The worker's first line names its git build and its build
//!   flags (profile, rustflags, target features); a remote whose build or flags are not
//!   the coordinator's is refused at connect. The binary is shipped, not rebuilt
//!   (`scripts/voxel-remote-ship.sh`): both machines are Zen 5, and one
//!   `-C target-cpu=znver5` build running on both executes the same instructions.
//! - **The same worlds.** The worker founds its fixtures itself from the run's
//!   [`FixtureSpec`] — the arena half and the landscape pool, the same functions this
//!   machine calls — and answers with a per-fixture summary ([`fixture_summary`]:
//!   the frozen world's cells, the plants, the founders, the start faces and the
//!   recorded production totals, hashed exactly). One differing fixture refuses that
//!   remote, loudly. Founding there rather than shipping the frozen worlds: the pool is
//!   gigabytes in memory and has no file format, while founding is a pure function of
//!   the seeds that runs on the remote at the same time as it runs here.
//! - **Pairs stay whole.** The unit of work is both signs of one pair on one fixture
//!   ([`UnitJob`]), so whatever one machine's floats do, they do to both halves of the
//!   difference the gradient is made of. The worker rebuilds the pair's weights from
//!   the generation's centre with the coordinator's own
//!   [`super::trainer::candidate_theta`].
//! - **Held-out evaluation stays local**, and so does the reduction: a result lands at
//!   its fixed index whoever ran it.
//!
//! # Losing a remote
//!
//! A remote whose pipe closes, that sends nothing for [`RemoteOptions::stall`] (the
//! worker sends a heartbeat every two seconds), that holds a unit past
//! [`RemoteOptions::unit_timeout`], or that reports a failure, is dropped for the rest
//! of the run with one loud line, and its outstanding units are re-queued on the local
//! workers ([`super::trainer::run_generation`]). The generation completes.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use cubarium_voxel_fauna::Founder;
use serde::{Deserialize, Serialize};

use super::controller::EpisodeDriver;
use super::driver::{self, Episode, Limits};
use super::task::{self, Prepared};
use super::trainer::{Candidate, VoxelProtocol, candidate_theta};

/// The pipe protocol's name: a worker speaking another is refused.
pub const REMOTE_PROTOCOL: &str = "cub-voxel-remote-1";

/// Every protocol line starts with this; any other line is the worker's own chatter.
pub const PREFIX: &str = "@cvr ";

/// Where `--remote host:threads` finds the worker binary on the remote machine.
pub const DEFAULT_REMOTE_BIN: &str = "~/cubarium-train/cubarium-search";

/// How this binary was built: profile, optimisation level, target, the rustflags and a
/// digest of the enabled target features (`build.rs`). Part of the connect check.
pub const BUILD_FLAGS: &str = env!("CUBARIUM_SEARCH_FLAGS");

/// The fixtures a worker founds for itself: the run's arena half and, on a `--p5` run,
/// the whole training landscape pool, in the order the coordinator holds them. A unit
/// names its fixture by index into this list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureSpec {
    pub founder: String,
    pub stage: String,
    pub band: String,
    /// How many training arenas, from the front ([`task::arena_half`]).
    pub layouts: usize,
    /// P5-C's arena grids and landscape pool ([`super::landscape::training_pool`]).
    pub p5: bool,
}

impl FixtureSpec {
    /// The arena half, exactly as the trainer builds it.
    pub fn arenas(&self) -> Result<Vec<Prepared>, String> {
        let founder = super::parse_founder(&self.founder)?;
        let stage = task::parse_stage(&self.stage)?;
        let band = task::parse_band(&self.band)?;
        Ok(task::arena_half(
            founder,
            stage,
            band,
            self.p5,
            self.layouts,
        ))
    }

    /// Every fixture: the arena half, then the landscape pool, founded on up to
    /// `workers` threads.
    pub fn build(&self, workers: usize) -> Result<Vec<Prepared>, String> {
        let mut fixtures = self.arenas()?;
        if self.p5 {
            let founder = super::parse_founder(&self.founder)?;
            fixtures.extend(super::landscape::training_pool(founder, workers)?);
        }
        Ok(fixtures)
    }
}

/// A streaming 64-bit digest: FNV-1a's constants over whole words. Each step is a
/// bijection of the running state, so a single differing word always shows.
struct Digest(u64);

impl Digest {
    fn new() -> Digest {
        Digest(0xcbf2_9ce4_8422_2325)
    }
    fn word(&mut self, w: u64) {
        self.0 = (self.0 ^ w).wrapping_mul(0x0000_0100_0000_01b3);
    }
    fn bytes(&mut self, b: &[u8]) {
        for chunk in b.chunks(8) {
            let mut w = [0u8; 8];
            w[..chunk.len()].copy_from_slice(chunk);
            self.word(u64::from_le_bytes(w));
        }
        self.word(b.len() as u64);
    }
    fn text(&mut self, t: &str) {
        self.bytes(t.as_bytes());
    }
}

fn world_digest(world: &cubarium_voxel::World) -> u64 {
    let v = world.view();
    let mut d = Digest::new();
    let material: Vec<u8> = v.material.iter().map(|m| *m as u8).collect();
    d.bytes(&material);
    for f in v.free.iter().chain(v.pore) {
        d.word(f.to_bits());
    }
    d.word(v.aquifer_m3.to_bits());
    d.word(v.atmosphere_m3.to_bits());
    d.word(v.tick);
    d.0
}

fn flora_digest(flora: &cubarium_voxel_flora::Flora) -> u64 {
    let mut d = Digest::new();
    d.bytes(&serde_json::to_vec(flora).expect("the plant layer serializes"));
    d.0
}

fn debug_digest(value: &impl std::fmt::Debug) -> u64 {
    let mut d = Digest::new();
    d.text(&format!("{value:?}"));
    d.0
}

/// One fixture, summarised exactly enough that two machines agreeing on it are running
/// the same world: the frozen cells (material, free and pore water), the plant layer,
/// who is placed where and where the acting bodies may start, and — on a landscape that
/// carries it — the recorded production totals, to the bit.
pub fn fixture_summary(fixture: &Prepared) -> String {
    match fixture {
        Prepared::Arena(a) => {
            let arena = a.fixture_arena();
            format!(
                "arena {} grid {} stage {} band {} | world {:016x} | plants {:016x} | \
                 resources {} {:016x} | start {:016x} | bystanders {} | animal {:?}",
                a.layout_seed,
                a.grid.as_str(),
                a.stage.as_str(),
                a.band.as_str(),
                world_digest(&arena.world),
                flora_digest(&arena.flora),
                arena.resources.len(),
                debug_digest(&(&arena.resources, &arena.resource_kinds)),
                debug_digest(&arena.start),
                arena.bystanders.len(),
                arena.animal_id,
            )
        }
        Prepared::Landscape(l) => {
            let per_lineage: Vec<usize> = Founder::ALL
                .iter()
                .map(|f| l.placements().iter().filter(|p| p.founder == *f).count())
                .collect();
            let production = l.production().map_or("none".to_string(), |p| {
                format!(
                    "litter {:016x} cap {:016x} buckets {}",
                    p.litter_total().to_bits(),
                    p.cap_growth.to_bits(),
                    p.buckets.len()
                )
            });
            format!(
                "landscape {} world seed {} | world {:016x} | plants {:016x} | founders \
                 {per_lineage:?} {:016x} | start faces {} {:016x} | production {production}",
                l.label(),
                l.world_seed,
                world_digest(l.world()),
                flora_digest(l.flora()),
                debug_digest(&l.placements()),
                l.start_faces().len(),
                debug_digest(&l.start_faces()),
            )
        }
    }
}

// ------------------------------------------------------------------- the messages

/// Coordinator → worker.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToWorker {
    /// Found these fixtures and answer [`FromWorker::Ready`] with their summaries.
    Setup {
        fixtures: FixtureSpec,
    },
    /// The centre every following unit of `generation` perturbs.
    Generation {
        generation: u32,
        founder: String,
        train_seed: u64,
        sigma: f64,
        #[serde(with = "crate::es::bits::hex_f64s")]
        theta: Vec<f64>,
    },
    Unit(UnitJob),
    Shutdown,
}

/// One unit: both signs of pair `pair` on fixture `fixture`, or the centre on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitJob {
    pub id: u64,
    pub generation: u32,
    /// The pair, or `None` for the unperturbed centre.
    pub pair: Option<usize>,
    /// Index into the worker's [`FixtureSpec::build`] list.
    pub fixture: usize,
    pub horizon: u64,
    pub episode_seed: u64,
    /// The job names, one per candidate in [`Candidate`] order (`+`, `−`; or the centre).
    pub names: Vec<String>,
}

/// Who a worker is: the connect check reads this.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: String,
    pub build: String,
    pub flags: String,
    pub threads: usize,
    pub host: String,
    pub exe: String,
}

/// Worker → coordinator.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum FromWorker {
    Hello(Hello),
    /// The fixtures are founded: one [`fixture_summary`] each, in order.
    Ready {
        summaries: Vec<String>,
        seconds: f64,
    },
    SetupFailed {
        detail: String,
    },
    Result {
        id: u64,
        episodes: Vec<Episode>,
        /// The thread-seconds the unit took on the worker.
        busy_seconds: f64,
    },
    Failed {
        id: u64,
        detail: String,
    },
    Heartbeat,
}

/// What the reader thread hands the coordinator.
#[derive(Debug)]
pub enum Incoming {
    Message(FromWorker),
    /// A protocol line that did not parse — a non-finite float in an episode would do
    /// it — with the unit id when one could be read out of it.
    Unreadable {
        id: Option<u64>,
        detail: String,
    },
    /// The worker's output ended.
    Closed(String),
}

fn line(msg: &impl Serialize) -> String {
    format!(
        "{PREFIX}{}\n",
        serde_json::to_string(msg).expect("a protocol message serializes")
    )
}

// ------------------------------------------------------------------- the coordinator

/// How to start one remote: a name for the log, the command, and the units it may
/// hold at once (its thread count).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteSpec {
    pub name: String,
    pub argv: Vec<String>,
    pub threads: usize,
}

impl RemoteSpec {
    /// Any command that ends in a `voxel-eval-worker` on its stdin and stdout: the tests'
    /// loopback runs this build through `sh -c`.
    pub fn command(name: &str, argv: Vec<String>, threads: usize) -> RemoteSpec {
        RemoteSpec {
            name: name.to_string(),
            argv,
            threads,
        }
    }

    /// `ssh host '<bin> voxel-eval-worker --threads N'`: batch mode (a password prompt
    /// would hang the run), and keep-alives so a dead link closes the pipe.
    pub fn ssh(host: &str, threads: usize, bin: &str) -> RemoteSpec {
        RemoteSpec {
            name: host.to_string(),
            argv: vec![
                "ssh".into(),
                "-o".into(),
                "BatchMode=yes".into(),
                "-o".into(),
                "ServerAliveInterval=10".into(),
                "-o".into(),
                "ServerAliveCountMax=3".into(),
                host.into(),
                format!("{bin} voxel-eval-worker --threads {threads}"),
            ],
            threads,
        }
    }

    /// `--remote host:threads`.
    pub fn parse(arg: &str, bin: &str) -> Result<RemoteSpec, String> {
        let (host, threads) = arg
            .rsplit_once(':')
            .ok_or_else(|| format!("--remote wants `host:threads`, not `{arg}`"))?;
        let threads: usize = threads
            .parse()
            .ok()
            .filter(|t| *t > 0)
            .ok_or_else(|| format!("--remote `{arg}`: `{threads}` is not a thread count"))?;
        if host.is_empty() {
            return Err(format!("--remote `{arg}` names no host"));
        }
        Ok(RemoteSpec::ssh(host, threads, bin))
    }
}

/// The remote's clocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RemoteOptions {
    /// How long a remote has to say who it is.
    pub hello_timeout: Duration,
    /// A remote holding units that sends nothing — no result, no heartbeat — for this
    /// long is lost.
    pub stall: Duration,
    /// A remote holding one unit for this long is lost.
    pub unit_timeout: Duration,
}

impl Default for RemoteOptions {
    fn default() -> RemoteOptions {
        RemoteOptions {
            hello_timeout: Duration::from_secs(60),
            stall: Duration::from_secs(30),
            unit_timeout: Duration::from_secs(600),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Status {
    /// Said hello; founding its fixtures.
    Founding,
    Active,
    Refused(String),
    Dead(String),
}

/// One connected remote worker.
pub struct Remote {
    name: String,
    capacity: usize,
    options: RemoteOptions,
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    inbox: Mutex<Receiver<Incoming>>,
    status: Mutex<Status>,
    last_seen: Arc<Mutex<Instant>>,
    ids: Arc<AtomicU64>,
    hello: Option<Hello>,
}

impl Remote {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Units it may hold at once: its thread count.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn options(&self) -> RemoteOptions {
        self.options
    }

    /// A unit id unique across the run and every remote, so a late answer from an
    /// earlier generation is recognised and dropped.
    pub fn next_id(&self) -> u64 {
        self.ids.fetch_add(1, Ordering::SeqCst)
    }

    fn send(&self, msg: &ToWorker) -> Result<(), String> {
        let mut stdin = self.stdin.lock().expect("stdin");
        let pipe = stdin
            .as_mut()
            .ok_or_else(|| "the pipe is closed".to_string())?;
        pipe.write_all(line(msg).as_bytes())
            .and_then(|()| pipe.flush())
            .map_err(|e| format!("the pipe refused a write: {e}"))
    }

    /// Tell the worker the generation's centre.
    pub fn send_generation(
        &self,
        generation: u32,
        protocol: &VoxelProtocol,
        theta: &[f64],
    ) -> Result<(), String> {
        self.send(&ToWorker::Generation {
            generation,
            founder: protocol.founder.clone(),
            train_seed: protocol.train_seed,
            sigma: protocol.sigma,
            theta: theta.to_vec(),
        })
    }

    pub fn send_unit(&self, unit: &UnitJob) -> Result<(), String> {
        self.send(&ToWorker::Unit(unit.clone()))
    }

    /// The next thing the worker said, if it said anything within `timeout`.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Incoming> {
        match self.inbox.lock().expect("inbox").recv_timeout(timeout) {
            Ok(m) => Some(m),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                Some(Incoming::Closed("the reader thread is gone".into()))
            }
        }
    }

    /// Time since the worker last said anything at all.
    pub fn silence(&self) -> Duration {
        self.last_seen.lock().expect("last seen").elapsed()
    }

    /// Drop this remote for the rest of the run and end its process.
    pub fn lose(&self, reason: &str) {
        *self.status.lock().expect("status") = Status::Dead(reason.to_string());
        self.stdin.lock().expect("stdin").take();
        let _ = self.child.lock().expect("child").kill();
    }

    fn refuse(&self, reason: String) {
        println!("# REMOTE {} REFUSED: {reason}", self.name);
        *self.status.lock().expect("status") = Status::Refused(reason);
        self.stdin.lock().expect("stdin").take();
        let _ = self.child.lock().expect("child").kill();
    }

    fn status(&self) -> Status {
        self.status.lock().expect("status").clone()
    }

    /// Compare the worker's fixtures with this machine's; join or be refused.
    fn judge(&self, summaries: &[String], seconds: f64, local: &[String]) {
        if summaries.len() != local.len() {
            return self.refuse(format!(
                "it founded {} fixtures, this machine holds {}",
                summaries.len(),
                local.len()
            ));
        }
        if let Some(i) = (0..local.len()).find(|&i| summaries[i] != local[i]) {
            return self.refuse(format!(
                "fixture {i} differs — here: {} — there: {}",
                local[i], summaries[i]
            ));
        }
        *self.status.lock().expect("status") = Status::Active;
        let h = self.hello.as_ref();
        println!(
            "# remote {}: joined with {} threads (host {}, build {}, {}); its {} fixtures \
             match this machine's (founded there in {seconds:.1} s)",
            self.name,
            self.capacity,
            h.map_or("?", |h| h.host.as_str()),
            h.map_or("?", |h| h.build.as_str()),
            h.map_or("?", |h| h.flags.as_str()),
            local.len(),
        );
    }

    /// Read what has arrived while founding, without blocking past `until`.
    fn await_ready(&self, local: &[String], until: Instant) {
        while self.status() == Status::Founding {
            let wait = until.saturating_duration_since(Instant::now());
            let Some(msg) = self.recv_timeout(wait.max(Duration::from_millis(1))) else {
                if Instant::now() >= until {
                    return;
                }
                continue;
            };
            match msg {
                Incoming::Message(FromWorker::Ready { summaries, seconds }) => {
                    self.judge(&summaries, seconds, local)
                }
                Incoming::Message(FromWorker::SetupFailed { detail }) => {
                    self.refuse(format!("it could not found the fixtures: {detail}"))
                }
                Incoming::Closed(reason) => {
                    println!("# REMOTE {} LOST while founding: {reason}", self.name);
                    self.lose(&reason);
                }
                _ => {}
            }
            if Instant::now() >= until {
                return;
            }
        }
    }
}

/// The run's remote workers.
pub struct RemotePool {
    remotes: Vec<Remote>,
    local: Mutex<Option<Vec<String>>>,
    ready_wait: Duration,
}

impl RemotePool {
    /// Start every remote, check that it is this build, and set it founding `fixtures`.
    /// Returns at once after the hello: the founding runs there while this machine
    /// founds its own. A remote that does not start, does not say hello in time, or is
    /// another build is refused with a loud line and takes no part.
    pub fn connect(
        specs: &[RemoteSpec],
        fixtures: &FixtureSpec,
        options: RemoteOptions,
    ) -> RemotePool {
        let ids = Arc::new(AtomicU64::new(1));
        let remotes = specs
            .iter()
            .filter_map(|spec| match start(spec, options, ids.clone()) {
                Ok(r) => Some(r),
                Err(e) => {
                    println!("# REMOTE {} REFUSED: {e}", spec.name);
                    None
                }
            })
            .collect::<Vec<_>>();
        for r in &remotes {
            if r.status() != Status::Founding {
                continue;
            }
            if let Err(e) = r.send(&ToWorker::Setup {
                fixtures: fixtures.clone(),
            }) {
                r.refuse(format!("the setup could not be sent: {e}"));
            }
        }
        RemotePool {
            remotes,
            local: Mutex::new(None),
            ready_wait: Duration::from_secs(900),
        }
    }

    /// How long [`super::trainer::train`] waits, once its own fixtures are founded, for a
    /// remote still founding before it starts without it (15 minutes by default).
    pub fn ready_wait(&self) -> Duration {
        self.ready_wait
    }

    pub fn set_ready_wait(&mut self, wait: Duration) {
        self.ready_wait = wait;
    }

    /// Summarise this machine's fixtures — in the order the remotes founded theirs —
    /// and admit every remote whose summaries match, waiting up to `wait` for those
    /// still founding. A remote not ready by then may still join at a later generation
    /// ([`RemotePool::admit`]).
    pub fn verify<'a>(&self, local: impl IntoIterator<Item = &'a Prepared>, wait: Duration) {
        let summaries: Vec<String> = local.into_iter().map(fixture_summary).collect();
        let until = Instant::now() + wait;
        for r in &self.remotes {
            if r.status() == Status::Founding {
                if Instant::now() < until {
                    println!(
                        "# remote {}: waiting up to {:.0} s for its fixtures",
                        r.name,
                        until
                            .saturating_duration_since(Instant::now())
                            .as_secs_f64()
                    );
                }
                r.await_ready(&summaries, until);
                if r.status() == Status::Founding {
                    println!(
                        "# remote {}: still founding; it joins at a later generation when \
                         its fixtures are ready",
                        r.name
                    );
                }
            }
        }
        *self.local.lock().expect("local") = Some(summaries);
    }

    /// Admit any remote that has finished founding since [`RemotePool::verify`],
    /// without waiting.
    pub fn admit(&self) {
        let Some(local) = self.local.lock().expect("local").clone() else {
            return;
        };
        for r in &self.remotes {
            if r.status() == Status::Founding {
                r.await_ready(&local, Instant::now());
            }
        }
    }

    /// The remotes taking units now.
    pub fn active_remotes(&self) -> Vec<&Remote> {
        self.remotes
            .iter()
            .filter(|r| r.status() == Status::Active)
            .collect()
    }

    pub fn active(&self) -> usize {
        self.active_remotes().len()
    }

    /// Every remote and where it stands: `founding`, `active`, `refused: …`, `dead: …`.
    pub fn status(&self) -> Vec<(String, String)> {
        self.remotes
            .iter()
            .map(|r| {
                let s = match r.status() {
                    Status::Founding => "founding".to_string(),
                    Status::Active => "active".to_string(),
                    Status::Refused(why) => format!("refused: {why}"),
                    Status::Dead(why) => format!("dead: {why}"),
                };
                (r.name.clone(), s)
            })
            .collect()
    }
}

impl Drop for RemotePool {
    /// Say goodbye, give each worker a moment to go, then end it.
    fn drop(&mut self) {
        for r in &self.remotes {
            let _ = r.send(&ToWorker::Shutdown);
            r.stdin.lock().expect("stdin").take();
        }
        let until = Instant::now() + Duration::from_secs(2);
        for r in &self.remotes {
            let mut child = r.child.lock().expect("child");
            while Instant::now() < until {
                if let Ok(Some(_)) = child.try_wait() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Spawn one remote, wire its pipes, and read its hello.
fn start(spec: &RemoteSpec, options: RemoteOptions, ids: Arc<AtomicU64>) -> Result<Remote, String> {
    let (program, args) = spec
        .argv
        .split_first()
        .ok_or_else(|| "an empty command".to_string())?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("`{}` did not start: {e}", spec.argv.join(" ")))?;
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let last_seen = Arc::new(Mutex::new(Instant::now()));
    let (tx, rx) = mpsc::channel();
    {
        let name = spec.name.clone();
        let last_seen = last_seen.clone();
        std::thread::spawn(move || read_worker(stdout, &name, &last_seen, &tx));
    }
    {
        let name = spec.name.clone();
        std::thread::spawn(move || {
            for l in BufReader::new(stderr).lines() {
                match l {
                    Ok(l) => eprintln!("[{name}] {l}"),
                    Err(_) => return,
                }
            }
        });
    }
    let mut remote = Remote {
        name: spec.name.clone(),
        capacity: spec.threads.max(1),
        options,
        child: Mutex::new(child),
        stdin: Mutex::new(Some(stdin)),
        inbox: Mutex::new(rx),
        status: Mutex::new(Status::Founding),
        last_seen,
        ids,
        hello: None,
    };
    let until = Instant::now() + options.hello_timeout;
    let hello = loop {
        let wait = until.saturating_duration_since(Instant::now());
        match remote.recv_timeout(wait) {
            Some(Incoming::Message(FromWorker::Hello(h))) => break h,
            Some(Incoming::Closed(reason)) => {
                remote.lose(&reason);
                return Err(format!("it closed before saying hello: {reason}"));
            }
            Some(_) => {}
            None => {
                remote.lose("no hello");
                return Err(format!(
                    "no hello within {:.0} s",
                    options.hello_timeout.as_secs_f64()
                ));
            }
        }
    };
    let mismatch = if hello.protocol != REMOTE_PROTOCOL {
        Some(format!(
            "it speaks {}, this coordinator {REMOTE_PROTOCOL}",
            hello.protocol
        ))
    } else if hello.build != crate::evaluate::BUILD_ID {
        Some(format!(
            "its build is {}, this coordinator's is {}: ship this build",
            hello.build,
            crate::evaluate::BUILD_ID
        ))
    } else if hello.flags != BUILD_FLAGS {
        Some(format!(
            "its build flags are `{}`, this coordinator's are `{BUILD_FLAGS}`: ship this build",
            hello.flags
        ))
    } else {
        None
    };
    remote.capacity = remote.capacity.min(hello.threads.max(1));
    remote.hello = Some(hello);
    if let Some(why) = mismatch {
        remote.lose(&why);
        return Err(why);
    }
    Ok(remote)
}

/// The reader: protocol lines become [`Incoming`]s; anything else goes to stderr.
fn read_worker(
    stdout: std::process::ChildStdout,
    name: &str,
    last_seen: &Mutex<Instant>,
    tx: &Sender<Incoming>,
) {
    for l in BufReader::new(stdout).lines() {
        *last_seen.lock().expect("last seen") = Instant::now();
        let l = match l {
            Ok(l) => l,
            Err(e) => {
                let _ = tx.send(Incoming::Closed(format!("the pipe failed: {e}")));
                return;
            }
        };
        let Some(body) = l.strip_prefix(PREFIX) else {
            eprintln!("[{name}] {l}");
            continue;
        };
        let msg = match serde_json::from_str::<FromWorker>(body) {
            Ok(m) => Incoming::Message(m),
            Err(e) => Incoming::Unreadable {
                id: serde_json::from_str::<serde_json::Value>(body)
                    .ok()
                    .and_then(|v| v.get("id").and_then(serde_json::Value::as_u64)),
                detail: e.to_string(),
            },
        };
        if tx.send(msg).is_err() {
            return;
        }
    }
    let _ = tx.send(Incoming::Closed("the worker closed its output".into()));
}

// ------------------------------------------------------------------- the worker

/// `voxel-eval-worker`'s settings. The two `*_after` hooks exist for the tests: a
/// worker that dies, or goes silent, part-way through a generation.
#[derive(Clone, Copy, Debug)]
pub struct WorkerOptions {
    pub threads: usize,
    pub heartbeat: Duration,
    /// Exit, abruptly, right after sending this many results.
    pub die_after: Option<u64>,
    /// Stop saying anything at all — results and heartbeats — after this many results,
    /// with the pipe left open.
    pub hang_after: Option<u64>,
}

/// The generation a worker is evaluating.
struct Centre {
    generation: u32,
    founder: Founder,
    train_seed: u64,
    sigma: f64,
    theta: Vec<f64>,
}

/// The worker's stdout: one writer, whole lines, and the two test hooks.
struct Outbox {
    out: Mutex<std::io::Stdout>,
    results: AtomicU64,
    hung: AtomicBool,
    options: WorkerOptions,
}

impl Outbox {
    fn send(&self, msg: &FromWorker) {
        let mut out = self.out.lock().expect("stdout");
        while self.hung.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(3600));
        }
        if out
            .write_all(line(msg).as_bytes())
            .and_then(|()| out.flush())
            .is_err()
        {
            // The coordinator is gone: nobody to work for.
            std::process::exit(0);
        }
        if matches!(msg, FromWorker::Result { .. }) {
            let sent = self.results.fetch_add(1, Ordering::SeqCst) + 1;
            if self.options.die_after == Some(sent) {
                std::process::exit(3);
            }
            if self.options.hang_after == Some(sent) {
                self.hung.store(true, Ordering::SeqCst);
            }
        }
    }
}

/// Serve units on stdin/stdout until the coordinator says goodbye or goes away.
pub fn serve(options: WorkerOptions) -> Result<(), String> {
    let threads = options.threads.max(1);
    let outbox = Arc::new(Outbox {
        out: Mutex::new(std::io::stdout()),
        results: AtomicU64::new(0),
        hung: AtomicBool::new(false),
        options,
    });
    outbox.send(&FromWorker::Hello(Hello {
        protocol: REMOTE_PROTOCOL.into(),
        build: crate::evaluate::BUILD_ID.into(),
        flags: BUILD_FLAGS.into(),
        threads,
        host: std::fs::read_to_string("/etc/hostname")
            .map(|h| h.trim().to_string())
            .unwrap_or_else(|_| "unknown".into()),
        exe: std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    }));
    {
        let outbox = outbox.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(options.heartbeat);
                outbox.send(&FromWorker::Heartbeat);
            }
        });
    }
    let fixtures: Arc<OnceLock<Vec<Prepared>>> = Arc::new(OnceLock::new());
    let centre: Arc<Mutex<Option<Arc<Centre>>>> = Arc::new(Mutex::new(None));
    let (tx, rx) = mpsc::channel::<UnitJob>();
    let rx = Arc::new(Mutex::new(rx));
    for _ in 0..threads {
        let (outbox, fixtures, centre, rx) =
            (outbox.clone(), fixtures.clone(), centre.clone(), rx.clone());
        std::thread::spawn(move || {
            let never = AtomicBool::new(false);
            loop {
                let Ok(job) = rx.lock().expect("jobs").recv() else {
                    return;
                };
                let Some(c) = centre.lock().expect("centre").clone() else {
                    outbox.send(&FromWorker::Failed {
                        id: job.id,
                        detail: "a unit before any generation".into(),
                    });
                    continue;
                };
                if c.generation != job.generation {
                    continue; // an earlier generation's: the coordinator has moved on
                }
                let started = Instant::now();
                let answer = match run_unit(&job, &c, fixtures.get(), &never) {
                    Ok(episodes) => FromWorker::Result {
                        id: job.id,
                        episodes,
                        busy_seconds: started.elapsed().as_secs_f64(),
                    },
                    Err(detail) => FromWorker::Failed { id: job.id, detail },
                };
                outbox.send(&answer);
            }
        });
    }
    for l in std::io::stdin().lock().lines() {
        let l = l.map_err(|e| format!("stdin: {e}"))?;
        let body = l.strip_prefix(PREFIX).unwrap_or(&l).trim();
        if body.is_empty() {
            continue;
        }
        let msg: ToWorker =
            serde_json::from_str(body).map_err(|e| format!("an unreadable request: {e}"))?;
        match msg {
            ToWorker::Setup { fixtures: spec } => {
                let t = Instant::now();
                match spec.build(threads) {
                    Ok(built) => {
                        let summaries = built.iter().map(fixture_summary).collect();
                        let _ = fixtures.set(built);
                        outbox.send(&FromWorker::Ready {
                            summaries,
                            seconds: t.elapsed().as_secs_f64(),
                        });
                    }
                    Err(detail) => outbox.send(&FromWorker::SetupFailed { detail }),
                }
            }
            ToWorker::Generation {
                generation,
                founder,
                train_seed,
                sigma,
                theta,
            } => {
                let founder = super::parse_founder(&founder)?;
                *centre.lock().expect("centre") = Some(Arc::new(Centre {
                    generation,
                    founder,
                    train_seed,
                    sigma,
                    theta,
                }));
            }
            ToWorker::Unit(job) => {
                let _ = tx.send(job);
            }
            ToWorker::Shutdown => break,
        }
    }
    // Goodbye or end of input: units still running are the coordinator's to forget.
    std::process::exit(0)
}

/// Both signs of the unit's pair (or the centre) on its fixture, in candidate order.
fn run_unit(
    job: &UnitJob,
    centre: &Centre,
    fixtures: Option<&Vec<Prepared>>,
    never: &AtomicBool,
) -> Result<Vec<Episode>, String> {
    let fixture = fixtures
        .ok_or("a unit before the fixtures were founded")?
        .get(job.fixture)
        .ok_or_else(|| format!("no fixture {}", job.fixture))?;
    let candidates = match job.pair {
        Some(p) => vec![Candidate::Plus(p), Candidate::Minus(p)],
        None => vec![Candidate::Center],
    };
    if job.names.len() != candidates.len() {
        return Err(format!(
            "{} names for {} candidates",
            job.names.len(),
            candidates.len()
        ));
    }
    let mut eps = vec![0.0; centre.theta.len()];
    let mut episodes = Vec::with_capacity(candidates.len());
    for (c, name) in candidates.into_iter().zip(&job.names) {
        let theta = candidate_theta(
            &centre.theta,
            centre.train_seed,
            centre.sigma,
            centre.generation,
            c,
            &mut eps,
        );
        let driver = EpisodeDriver::gru(&theta, centre.founder)?;
        let e = driver::run_prepared_seeded(
            fixture,
            &driver,
            job.horizon,
            Limits::new(never),
            name,
            job.episode_seed,
        )
        .map_err(|e| format!("{name}: {e}"))?;
        episodes.push(e);
    }
    Ok(episodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remote_argument_names_a_host_and_a_thread_count() {
        let r = RemoteSpec::parse("eidolon.local:12", DEFAULT_REMOTE_BIN).expect("parses");
        assert_eq!(r.name, "eidolon.local");
        assert_eq!(r.threads, 12);
        assert_eq!(r.argv[0], "ssh");
        assert!(r.argv.contains(&"BatchMode=yes".to_string()));
        assert_eq!(
            r.argv.last().expect("the remote command"),
            "~/cubarium-train/cubarium-search voxel-eval-worker --threads 12"
        );
        assert!(RemoteSpec::parse("eidolon.local", DEFAULT_REMOTE_BIN).is_err());
        assert!(RemoteSpec::parse("eidolon.local:0", DEFAULT_REMOTE_BIN).is_err());
        assert!(RemoteSpec::parse(":4", DEFAULT_REMOTE_BIN).is_err());
    }

    #[test]
    fn the_messages_round_trip_through_a_protocol_line() {
        let unit = ToWorker::Unit(UnitJob {
            id: 7,
            generation: 3,
            pair: Some(2),
            fixture: 17,
            horizon: 2_400,
            episode_seed: u64::MAX,
            names: vec!["a".into(), "b".into()],
        });
        let text = line(&unit);
        assert!(text.starts_with(PREFIX) && text.ends_with('\n'));
        let back: ToWorker =
            serde_json::from_str(text[PREFIX.len()..].trim()).expect("a unit parses");
        let ToWorker::Unit(job) = back else {
            panic!("a unit");
        };
        assert_eq!(job.episode_seed, u64::MAX, "u64 seeds survive exactly");
        let theta = vec![0.1, -1.0e-300, f64::MIN_POSITIVE, 3.0];
        let generation = ToWorker::Generation {
            generation: 1,
            founder: "littershredder".into(),
            train_seed: 5,
            sigma: 0.02,
            theta: theta.clone(),
        };
        let text = line(&generation);
        let ToWorker::Generation { theta: back, .. } =
            serde_json::from_str(text[PREFIX.len()..].trim()).expect("parses")
        else {
            panic!("a generation");
        };
        assert_eq!(back, theta, "the centre crosses the pipe bit for bit");
    }

    /// A fixture's summary is a pure function of the fixture, and two arenas of
    /// different seeds or grids never share one.
    #[test]
    fn a_fixture_summary_names_its_world() {
        let spec = FixtureSpec {
            founder: "littershredder".into(),
            stage: "a".into(),
            band: "landed".into(),
            layouts: 2,
            p5: false,
        };
        let a = spec.arenas().expect("builds");
        let b = spec.arenas().expect("builds again");
        assert_eq!(fixture_summary(&a[0]), fixture_summary(&b[0]));
        assert_ne!(fixture_summary(&a[0]), fixture_summary(&a[1]));
        assert!(fixture_summary(&a[0]).starts_with("arena "));
    }
}
