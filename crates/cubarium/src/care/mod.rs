//! Optional care: the host half of feed / rain / clean.
//!
//! The host never touches ecology. It validates a request, journals it durably at a held
//! simulation boundary, and hands it to the world; the world alone decides what it does
//! and reports back a receipt. Everything in this module is either validation, durability,
//! or bookkeeping for the answers a waiting browser tab is owed.
//!
//! The shape comes from `design/7_Research/care-contract-2026-09-12.md` (revision 2) and
//! the review that produced it. Three structural decisions worth stating up front, because
//! they are what the rest of the code is arranged around:
//!
//! * **HTTP handlers never touch the disk.** A request is validated on the connection
//!   thread and `try_send` into a bounded FIFO of eight. If the FIFO is full the answer is
//!   `503 intake full` — a browser tab can never make the simulation wait on it.
//! * **Admission commits at a held boundary.** The runner drains that FIFO at a boundary
//!   with exactly `B` completed ticks, assigns contiguous sequence numbers and
//!   `apply_after_tick = B`, and stops advancing until the journal worker says the records
//!   are on disk. A slow `fsync` therefore stalls the world instead of silently moving the
//!   command to a later tick that a replay could never reconstruct.
//! * **Identity is server-issued and epoch-scoped.** `POST /care/register` mints an id
//!   embedding this run's epoch. After a restart every earlier id is retired, so a request
//!   that was in flight across the restart can never be re-admitted as a new command under
//!   a remembered identity; it is either already in the journal (and replays) or was never
//!   accepted.

pub mod journal;

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::clock::TICK_HZ;

pub use journal::{
    ACCEPTED_ESTIMATE, JOURNAL_LIMIT, Journal, JournalError, JournalHooks, JournalStatus,
    OUTCOME_RESERVE,
};

/// Commands waiting to be drained at the next boundary. Small on purpose: a burst beyond
/// this is refused with `503 intake full` rather than queued into a long, stale backlog.
pub const PREPARED_CAPACITY: usize = 8;
/// Commands admitted but not yet reported back.
pub const MAX_OUTSTANDING: usize = 4;
/// Registered client identities per process.
pub const MAX_CLIENTS: usize = 64;
/// Request rows kept for duplicate detection and `/care/status`.
pub const RETAINED_RECEIPTS: usize = 64;
/// Largest accepted request body.
pub const MAX_BODY_BYTES: usize = 4 * 1024;
/// How long `POST /care` waits for the durable acknowledgement before answering
/// `202 {pending: true}` and sending the client to `/care/status` for its result.
pub const ACCEPT_WAIT: Duration = Duration::from_secs(5);

/// Per-kind cooldown in simulated seconds: feed 30 s, rain 60 s, clean 30 s.
const COOLDOWN_SECONDS: [u64; 3] = [30, 60, 30];

/// What a viewer can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CareKind {
    Feed,
    Rain,
    Clean,
}

impl CareKind {
    /// The wire name, in the journal and in HTTP bodies.
    pub fn as_str(self) -> &'static str {
        match self {
            CareKind::Feed => "feed",
            CareKind::Rain => "rain",
            CareKind::Clean => "clean",
        }
    }

    /// The wire name back, or `None` for anything else. Unknown kinds are `400`, never a
    /// guess at the nearest one.
    pub fn parse(name: &str) -> Option<CareKind> {
        match name {
            "feed" => Some(CareKind::Feed),
            "rain" => Some(CareKind::Rain),
            "clean" => Some(CareKind::Clean),
            _ => None,
        }
    }

    /// Index into the per-kind cooldown table.
    pub fn index(self) -> usize {
        match self {
            CareKind::Feed => 0,
            CareKind::Rain => 1,
            CareKind::Clean => 2,
        }
    }

    /// This kind's cooldown, in simulation ticks.
    pub fn cooldown_ticks(self) -> u64 {
        COOLDOWN_SECONDS[self.index()] * u64::from(TICK_HZ)
    }

    /// All three, in wire order.
    pub const ALL: [CareKind; 3] = [CareKind::Feed, CareKind::Rain, CareKind::Clean];
}

/// A canonical surface point: which face and which pixel of its 64×64 chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CareTarget {
    pub face: u8,
    pub u: u8,
    pub v: u8,
}

impl CareTarget {
    /// `face` in 0..5 and both chart coordinates in 0..64. A target outside the charts has
    /// no cell to land on, so it is refused rather than clamped onto some other cell.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.face > 4 {
            return Err("face must be 0..4");
        }
        if self.u >= 64 || self.v >= 64 {
            return Err("u and v must be 0..63");
        }
        Ok(())
    }
}

/// One command, after the host has given it a sequence number and a boundary. This is
/// exactly what the journal records and what replay reconstructs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedCommand {
    /// Contiguous from the world's `admitted_seq`.
    pub seq: u64,
    /// The boundary with exactly this many completed ticks; the command applies there.
    pub apply_after_tick: u64,
    pub kind: CareKind,
    pub target: CareTarget,
    /// The server-issued identity that asked for it.
    pub client: String,
    /// That client's monotonic request number.
    pub request: u64,
}

/// The diagnostic record written after the world has answered.
#[derive(Clone, Debug)]
pub struct OutcomeRecord {
    pub seq: u64,
    pub tick: u64,
    /// `applied`, `partial` or `rejected`.
    pub outcome: &'static str,
    pub reason: String,
    /// The receipt's quantities, as JSON.
    pub applied: serde_json::Value,
}

/// What the runner is doing about care right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CareState {
    /// A newly created world that has not yet written a durable opening checkpoint.
    /// Accepting care first would leave journaled commands with no persisted world to
    /// replay them against, so intake stays shut until the first checkpoint lands.
    Opening,
    /// A recovered schedule is not yet exhausted. New commands must not allocate sequence
    /// numbers from the snapshot's cursor while the journal has already reserved higher
    /// ones, so intake stays shut until the last replayed command has been applied.
    Replaying,
    /// Normal: intake open, the world stepping.
    Ready,
    /// Holding tick advancement at a boundary while a record is committed.
    Holding,
    /// An append or `fsync` had an uncertain result. Intake is closed for good and the
    /// world is held at the boundary until a clean stop. There is no resume path in this
    /// process: see `journal`'s module documentation for why an abort record cannot
    /// safely provide one.
    Failed(String),
}

impl CareState {
    fn as_str(&self) -> &'static str {
        match self {
            CareState::Opening => "opening",
            CareState::Replaying => "replaying",
            CareState::Ready => "ready",
            CareState::Holding => "holding",
            CareState::Failed(_) => "failed",
        }
    }
}

/// How far one request has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    /// In the prepared FIFO or being committed; the client is still waiting.
    Queued,
    /// Durably journaled at a boundary; the world has not answered yet.
    Accepted,
    /// The world answered.
    Applied,
    Partial,
    Rejected,
    /// Care failed before this request reached the disk. Whether the record survived is
    /// genuinely unknown; the page is told to check the receipts after a restart.
    Failed,
}

impl RowState {
    fn as_str(self) -> &'static str {
        match self {
            RowState::Queued => "queued",
            RowState::Accepted => "accepted",
            RowState::Applied => "applied",
            RowState::Partial => "partial",
            RowState::Rejected => "rejected",
            RowState::Failed => "failed",
        }
    }

    fn is_terminal(self) -> bool {
        matches!(self, RowState::Applied | RowState::Partial | RowState::Rejected | RowState::Failed)
    }
}

/// A request the host is tracking, from intake to receipt.
#[derive(Clone, Debug)]
struct Row {
    client: String,
    request: u64,
    kind: CareKind,
    target: CareTarget,
    seq: Option<u64>,
    apply_after_tick: Option<u64>,
    state: RowState,
    reason: String,
    applied: serde_json::Value,
}

impl Row {
    fn to_json(&self, duplicate: bool) -> String {
        let number = |v: Option<u64>| {
            v.map_or("null".to_string(), |n| n.to_string())
        };
        format!(
            r#"{{"client":{},"request":{},"kind":"{}","target":{{"face":{},"u":{},"v":{}}},"seq":{},"apply_after_tick":{},"state":"{}","reason":{},"applied":{},"duplicate":{duplicate}}}"#,
            serde_json::Value::from(self.client.as_str()),
            self.request,
            self.kind.as_str(),
            self.target.face,
            self.target.u,
            self.target.v,
            number(self.seq),
            number(self.apply_after_tick),
            self.state.as_str(),
            serde_json::Value::from(self.reason.as_str()),
            self.applied,
        )
    }
}

/// A validated request on its way to the next boundary.
#[derive(Clone, Debug)]
struct Prepared {
    client: String,
    request: u64,
    kind: CareKind,
    target: CareTarget,
}

/// What `POST /care/register` answers.
#[derive(Clone, Debug)]
pub enum RegisterOutcome {
    Registered { client: String, epoch: String },
    Limited(&'static str),
    Unavailable(&'static str),
}

impl RegisterOutcome {
    /// The HTTP status line this outcome is served as.
    pub fn http_status(&self) -> &'static str {
        match self {
            RegisterOutcome::Registered { .. } => "200 OK",
            RegisterOutcome::Limited(_) => "429 Too Many Requests",
            RegisterOutcome::Unavailable(_) => "503 Service Unavailable",
        }
    }

    /// The JSON body.
    pub fn body(&self) -> String {
        match self {
            RegisterOutcome::Registered { client, epoch } => format!(
                r#"{{"client":{},"epoch":{}}}"#,
                serde_json::Value::from(client.as_str()),
                serde_json::Value::from(epoch.as_str())
            ),
            RegisterOutcome::Limited(reason) | RegisterOutcome::Unavailable(reason) => {
                format!(r#"{{"error":{}}}"#, serde_json::Value::from(*reason))
            }
        }
    }
}

/// What `POST /care` answers. The status codes are the contract's, in one place, so the
/// HTTP layer has no policy of its own.
#[derive(Clone, Debug)]
pub enum SubmitOutcome {
    /// Durably journaled at a boundary.
    Accepted { seq: u64, apply_after_tick: u64 },
    /// Still committing after [`ACCEPT_WAIT`]; the client reads its result from
    /// `/care/status`. Not a rejection and not an acceptance — the honest answer.
    Pending,
    /// Same identity, same payload: the retained receipt, verbatim.
    Duplicate(String),
    Invalid(&'static str),
    /// `conflict`, `stale`, or `retired`.
    Conflict(&'static str),
    /// A cooldown or one of the bounded limits.
    Limited(&'static str),
    /// Intake full, journal full, care failed, or care disabled.
    Unavailable(&'static str),
    /// Not open yet: the world is still writing its opening checkpoint, or still replaying
    /// a recovered schedule. `503 {"care": "opening" | "replaying"}` — a state, not an
    /// error, and the page retries rather than reporting a failure.
    Gated(&'static str),
}

impl SubmitOutcome {
    /// The HTTP status line this outcome is served as.
    pub fn http_status(&self) -> &'static str {
        match self {
            SubmitOutcome::Accepted { .. } | SubmitOutcome::Pending => "202 Accepted",
            SubmitOutcome::Duplicate(_) => "200 OK",
            SubmitOutcome::Invalid(_) => "400 Bad Request",
            SubmitOutcome::Conflict(_) => "409 Conflict",
            SubmitOutcome::Limited(_) => "429 Too Many Requests",
            SubmitOutcome::Unavailable(_) | SubmitOutcome::Gated(_) => "503 Service Unavailable",
        }
    }

    /// The JSON body.
    pub fn body(&self) -> String {
        match self {
            SubmitOutcome::Accepted { seq, apply_after_tick } => {
                format!(r#"{{"seq":{seq},"apply_after_tick":{apply_after_tick}}}"#)
            }
            SubmitOutcome::Pending => r#"{"pending":true}"#.to_string(),
            SubmitOutcome::Duplicate(receipt) => receipt.clone(),
            SubmitOutcome::Invalid(reason)
            | SubmitOutcome::Conflict(reason)
            | SubmitOutcome::Limited(reason)
            | SubmitOutcome::Unavailable(reason) => {
                format!(r#"{{"error":{}}}"#, serde_json::Value::from(*reason))
            }
            SubmitOutcome::Gated(state) => {
                format!(r#"{{"care":{}}}"#, serde_json::Value::from(*state))
            }
        }
    }
}

/// One registered identity's dedup state.
#[derive(Clone, Debug, Default)]
struct ClientEntry {
    /// The highest request number this client has had accepted.
    high_water: u64,
}

/// Everything two threads share: HTTP handlers, and the runner.
#[derive(Debug)]
struct Inner {
    care: CareState,
    holding_at: Option<u64>,
    clients: HashMap<String, ClientEntry>,
    issued: u64,
    rows: VecDeque<Row>,
    last_kind_tick: [Option<u64>; 3],
    outstanding: usize,
}

impl Inner {
    fn find(&mut self, client: &str, request: u64) -> Option<&mut Row> {
        self.rows.iter_mut().find(|r| r.client == client && r.request == request)
    }

    fn find_seq(&mut self, seq: u64) -> Option<&mut Row> {
        self.rows.iter_mut().find(|r| r.seq == Some(seq))
    }
}

/// The care service as the HTTP server sees it. Cheap to clone (`Arc`), and every method
/// is safe to call from a connection thread: none of them touches the disk or blocks on
/// the simulation, except `submit`'s bounded wait for the durable acknowledgement.
#[derive(Debug)]
pub struct CareShared {
    epoch: String,
    tx: SyncSender<Prepared>,
    state: Mutex<Inner>,
    cv: Condvar,
    /// The world's completed-tick count, published by the loop; rate limits are measured
    /// in these, not in wall time, so an 8× world's cooldowns are 8× shorter in wall time
    /// and exactly right in the world's own time.
    world_tick: AtomicU64,
    journal: Arc<JournalStatus>,
}

impl CareShared {
    /// This run's epoch: the process start stamp in the journal's first record.
    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    /// The world's last completed tick, as the loop last published it.
    pub fn world_tick(&self) -> u64 {
        self.world_tick.load(Ordering::Relaxed)
    }

    /// Issue a new client identity. The id embeds the epoch, so a page holding an id from
    /// before a restart is recognizably retired rather than silently re-registered.
    pub fn register(&self) -> RegisterOutcome {
        let mut inner = self.lock();
        if let CareState::Failed(_) = inner.care {
            return RegisterOutcome::Unavailable("care failed");
        }
        // Registering while the world is still opening or replaying is allowed: the page
        // wants its identity ready, and `POST /care` is what the gate refuses.
        if inner.clients.len() >= MAX_CLIENTS {
            return RegisterOutcome::Limited("too many clients");
        }
        inner.issued += 1;
        let client = format!("{}.{}", self.epoch, inner.issued);
        inner.clients.insert(client.clone(), ClientEntry::default());
        RegisterOutcome::Registered { client, epoch: self.epoch.clone() }
    }

    /// Validate one request, put it in the prepared FIFO, and wait up to [`ACCEPT_WAIT`]
    /// for the durable acknowledgement.
    pub fn submit(
        &self,
        client: &str,
        request: u64,
        kind: CareKind,
        target: CareTarget,
    ) -> SubmitOutcome {
        if let Err(reason) = target.validate() {
            return SubmitOutcome::Invalid(reason);
        }
        let mut inner = self.lock();

        match inner.care {
            CareState::Failed(_) => return SubmitOutcome::Unavailable("care failed"),
            CareState::Opening => return SubmitOutcome::Gated("opening"),
            CareState::Replaying => return SubmitOutcome::Gated("replaying"),
            CareState::Ready | CareState::Holding => {}
        }
        // An identity this process never issued — or issued before a restart — is refused.
        // A `POST /care` never registers one implicitly: that is what would let a request
        // from before the epoch turn into a brand new command.
        if !inner.clients.contains_key(client) {
            return SubmitOutcome::Conflict("retired");
        }
        // The retained receipt answers a retry; a different payload under the same request
        // number is a genuine conflict and must not silently become a second command.
        if let Some(row) = inner.find(client, request) {
            if row.kind == kind && row.target == target {
                let receipt = row.to_json(true);
                return SubmitOutcome::Duplicate(receipt);
            }
            return SubmitOutcome::Conflict("conflict");
        }
        let high_water = inner.clients.get(client).map(|c| c.high_water).unwrap_or(0);
        if request <= high_water {
            // Above the high-water mark but no retained row: the receipt has aged out of
            // the bounded window. Refusing is the only answer that cannot apply it twice.
            return SubmitOutcome::Conflict("stale");
        }
        if self.journal.would_overflow(1) {
            return SubmitOutcome::Unavailable("journal full");
        }
        if inner.outstanding >= MAX_OUTSTANDING {
            return SubmitOutcome::Limited("too many outstanding commands");
        }
        let now = self.world_tick();
        if let Some(last) = inner.last_kind_tick[kind.index()]
            && now < last.saturating_add(kind.cooldown_ticks())
        {
            return SubmitOutcome::Limited("cooldown");
        }

        let prepared =
            Prepared { client: client.to_string(), request, kind, target };
        match self.tx.try_send(prepared) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => return SubmitOutcome::Unavailable("intake full"),
            Err(TrySendError::Disconnected(_)) => {
                return SubmitOutcome::Unavailable("care disabled");
            }
        }
        if let Some(entry) = inner.clients.get_mut(client) {
            entry.high_water = request;
        }
        inner.last_kind_tick[kind.index()] = Some(now);
        inner.outstanding += 1;
        while inner.rows.len() >= RETAINED_RECEIPTS {
            inner.rows.pop_front();
        }
        inner.rows.push_back(Row {
            client: client.to_string(),
            request,
            kind,
            target,
            seq: None,
            apply_after_tick: None,
            state: RowState::Queued,
            reason: String::new(),
            applied: serde_json::Value::Null,
        });

        // Wait for the runner and the journal worker, but never forever: a client that has
        // waited five seconds is told the truth — still committing — and reads the result
        // from `/care/status` rather than holding a connection thread open.
        let deadline = Instant::now() + ACCEPT_WAIT;
        loop {
            match inner.find(client, request) {
                None => return SubmitOutcome::Unavailable("the request was dropped"),
                Some(row) if row.state != RowState::Queued => {
                    return match (row.state, row.seq, row.apply_after_tick) {
                        (RowState::Failed, _, _) => SubmitOutcome::Unavailable("care failed"),
                        (_, Some(seq), Some(boundary)) => {
                            SubmitOutcome::Accepted { seq, apply_after_tick: boundary }
                        }
                        _ => SubmitOutcome::Pending,
                    };
                }
                Some(_) => {}
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return SubmitOutcome::Pending;
            }
            let (guard, _) = self
                .cv
                .wait_timeout(inner, remaining)
                .unwrap_or_else(|e| e.into_inner());
            inner = guard;
        }
    }

    /// `GET /care/status`.
    pub fn status_json(&self) -> String {
        let inner = self.lock();
        let now = self.world_tick();
        let cooldowns = CareKind::ALL
            .iter()
            .map(|kind| {
                let left = inner.last_kind_tick[kind.index()]
                    .map(|last| last.saturating_add(kind.cooldown_ticks()).saturating_sub(now))
                    .unwrap_or(0);
                format!(r#""{}":{left}"#, kind.as_str())
            })
            .collect::<Vec<_>>()
            .join(",");
        let receipts =
            inner.rows.iter().map(|r| r.to_json(false)).collect::<Vec<_>>().join(",");
        let reason = match &inner.care {
            CareState::Failed(reason) => serde_json::Value::from(reason.as_str()).to_string(),
            _ => "null".to_string(),
        };
        format!(
            r#"{{"enabled":true,"care":"{}","reason":{reason},"holding_at":{},"epoch":{},"world_tick":{now},"outstanding":{},"cooldowns":{{{cooldowns}}},"journal":{{"bytes":{},"limit":{},"outstanding":{}}},"receipts":[{receipts}]}}"#,
            inner.care.as_str(),
            inner.holding_at.map_or("null".to_string(), |b| b.to_string()),
            serde_json::Value::from(self.epoch.as_str()),
            inner.outstanding,
            self.journal.bytes(),
            self.journal.limit(),
            self.journal.outstanding(),
        )
    }

    /// The `/care/status` body a host without `--care` serves, so the page can tell
    /// "this world does not offer care" from "this host is too old to know the route".
    pub fn disabled_status_json() -> String {
        r#"{"enabled":false,"care":"disabled","reason":null,"holding_at":null,"epoch":null,"world_tick":0,"outstanding":0,"cooldowns":{},"journal":{"bytes":0,"limit":0,"outstanding":0},"receipts":[]}"#
            .to_string()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The care service the runner owns. It holds the receiving end of the prepared FIFO and
/// every method the simulation loop calls at a tick boundary.
#[derive(Debug)]
pub struct CareService {
    shared: Arc<CareShared>,
    prepared: Receiver<Prepared>,
}

impl CareService {
    /// A service for one run. `epoch` is the stamp the journal's first record carries.
    pub fn new(epoch: impl Into<String>, journal: Arc<JournalStatus>) -> CareService {
        let (tx, prepared) = sync_channel(PREPARED_CAPACITY);
        let shared = Arc::new(CareShared {
            epoch: epoch.into(),
            tx,
            state: Mutex::new(Inner {
                care: CareState::Ready,
                holding_at: None,
                clients: HashMap::new(),
                issued: 0,
                rows: VecDeque::new(),
                last_kind_tick: [None; 3],
                outstanding: 0,
            }),
            cv: Condvar::new(),
            world_tick: AtomicU64::new(0),
            journal,
        });
        CareService { shared, prepared }
    }

    /// The handle the web sink serves care from.
    pub fn shared(&self) -> Arc<CareShared> {
        Arc::clone(&self.shared)
    }

    /// Publish the world's completed-tick count. Called once per tick by the loop; the
    /// cooldowns and `/care/status` read it.
    pub fn publish_tick(&self, tick: u64) {
        self.shared.world_tick.store(tick, Ordering::Relaxed);
    }

    /// Take everything waiting in the prepared FIFO, in arrival order, and turn it into
    /// commands for boundary `boundary` with sequence numbers from `next_seq`.
    ///
    /// Called only at a boundary, by the simulation owner. The returned commands are not
    /// yet durable and not yet acknowledged to anyone.
    pub fn drain_prepared(&self, next_seq: u64, boundary: u64) -> Vec<PlannedCommand> {
        let mut planned = Vec::new();
        while let Ok(prepared) = self.prepared.try_recv() {
            let seq = next_seq + planned.len() as u64;
            planned.push(PlannedCommand {
                seq,
                apply_after_tick: boundary,
                kind: prepared.kind,
                target: prepared.target,
                client: prepared.client,
                request: prepared.request,
            });
        }
        if !planned.is_empty() {
            let mut inner = self.shared.lock();
            for command in &planned {
                if let Some(row) = inner.find(&command.client, command.request) {
                    row.seq = Some(command.seq);
                    row.apply_after_tick = Some(command.apply_after_tick);
                }
            }
        }
        planned
    }

    /// Shut intake until the opening checkpoint is durable. Called for a world this run
    /// created: a crash before the first checkpoint would otherwise leave accepted commands
    /// with no persisted world to replay them against.
    pub fn gate_until_opened(&self) {
        let mut inner = self.shared.lock();
        if !matches!(inner.care, CareState::Failed(_)) {
            inner.care = CareState::Opening;
        }
    }

    /// Shut intake until the recovered schedule is exhausted.
    pub fn gate_until_replayed(&self) {
        let mut inner = self.shared.lock();
        if !matches!(inner.care, CareState::Failed(_)) {
            inner.care = CareState::Replaying;
        }
    }

    /// Open intake. Only ever moves out of `Opening` or `Replaying`; a failed service stays
    /// failed and a holding one stays holding.
    pub fn open_intake(&self) {
        let mut inner = self.shared.lock();
        if matches!(inner.care, CareState::Opening | CareState::Replaying) {
            inner.care = CareState::Ready;
        }
        drop(inner);
        self.shared.cv.notify_all();
    }

    /// The service's current state, for the runner's own decisions.
    pub fn state(&self) -> CareState {
        self.shared.lock().care.clone()
    }

    /// Report that the world is held at `boundary` while a record commits.
    pub fn hold_at(&self, boundary: u64) {
        let mut inner = self.shared.lock();
        inner.holding_at = Some(boundary);
        if inner.care == CareState::Ready {
            inner.care = CareState::Holding;
        }
        drop(inner);
        self.shared.cv.notify_all();
    }

    /// Report that the hold is over and the world is stepping again.
    pub fn release_hold(&self) {
        let mut inner = self.shared.lock();
        inner.holding_at = None;
        if inner.care == CareState::Holding {
            inner.care = CareState::Ready;
        }
        drop(inner);
        self.shared.cv.notify_all();
    }

    /// The records are durably on disk: wake every client waiting on one of them with its
    /// `202 {seq, apply_after_tick}`.
    pub fn commit_accepted(&self, commands: &[PlannedCommand]) {
        let mut inner = self.shared.lock();
        for command in commands {
            if let Some(row) = inner.find(&command.client, command.request) {
                row.state = RowState::Accepted;
            }
        }
        drop(inner);
        self.shared.cv.notify_all();
    }

    /// The world answered. `outcome` is `applied`, `partial` or `rejected`.
    pub fn record_outcome(
        &self,
        seq: u64,
        outcome: &str,
        reason: &str,
        applied: serde_json::Value,
    ) {
        let state = match outcome {
            "applied" => RowState::Applied,
            "partial" => RowState::Partial,
            _ => RowState::Rejected,
        };
        self.settle(seq, state, reason, applied);
    }

    fn settle(&self, seq: u64, state: RowState, reason: &str, applied: serde_json::Value) {
        let mut inner = self.shared.lock();
        let was_open = match inner.find_seq(seq) {
            Some(row) => {
                let open = !row.state.is_terminal();
                row.state = state;
                row.reason = reason.to_string();
                row.applied = applied;
                open
            }
            None => false,
        };
        if was_open {
            inner.outstanding = inner.outstanding.saturating_sub(1);
        }
        drop(inner);
        self.shared.cv.notify_all();
    }

    /// An append or `fsync` had an uncertain result. Intake closes permanently, the runner
    /// holds the boundary, and the only way out is a clean stop (which still writes the
    /// final snapshot at that boundary). Every request that had not reached the disk is
    /// answered `503 care failed` rather than left waiting for an acknowledgement that will
    /// never come; whether its record survived is genuinely unknown until the next open.
    pub fn fail(&self, reason: impl Into<String>) {
        let reason = reason.into();
        let mut inner = self.shared.lock();
        inner.care = CareState::Failed(reason.clone());
        for row in inner.rows.iter_mut() {
            if row.state == RowState::Queued {
                row.state = RowState::Failed;
                row.reason = reason.clone();
            }
        }
        inner.outstanding = 0;
        drop(inner);
        self.shared.cv.notify_all();
    }

    /// Whether care has failed, and why.
    pub fn failure(&self) -> Option<String> {
        match &self.shared.lock().care {
            CareState::Failed(reason) => Some(reason.clone()),
            _ => None,
        }
    }

    /// Commands admitted but not yet reported back.
    pub fn outstanding(&self) -> usize {
        self.shared.lock().outstanding
    }
}

// --- The journal worker --------------------------------------------------------------

/// Work handed to the journal thread.
#[derive(Debug)]
pub enum JournalJob {
    /// Make these commands durable. The runner holds the boundary until the ack arrives.
    Accept(Vec<PlannedCommand>),
    /// Diagnostic outcome records; nothing waits on these.
    Outcome(Vec<OutcomeRecord>),
}

/// What the journal thread reports back.
///
/// [`JournalError::Uncertain`] means exactly that: the record may be on disk, and nothing
/// here may be read as "it did not happen". [`JournalError::Full`] is the opposite — the
/// capacity gate refused before anything was attempted — and is the one failure that lets
/// the runner answer the client and keep stepping.
#[derive(Debug)]
pub enum JournalAck {
    Accepted { commands: Vec<PlannedCommand>, result: Result<(), JournalError> },
    Outcome { result: Result<(), JournalError> },
}

/// The journal on its own thread, so a slow disk never runs inside the simulation loop.
/// The loop submits work and polls [`JournalWorker::poll_ack`] once per iteration; it
/// never blocks on the channel.
#[derive(Debug)]
pub struct JournalWorker {
    jobs: SyncSender<JournalJob>,
    acks: Receiver<JournalAck>,
    handle: std::thread::JoinHandle<Journal>,
    status: Arc<JournalStatus>,
}

impl JournalWorker {
    /// Take ownership of an open journal and start the thread.
    pub fn spawn(journal: Journal) -> JournalWorker {
        JournalWorker::spawn_holding(journal, None)
    }

    /// [`JournalWorker::spawn`], with the worker thread holding a share of the state lock
    /// for the same reason the checkpoint worker does: a thread that may still be writing
    /// into the directory must keep the directory owned, on every exit path including the
    /// runner's early-error returns.
    pub fn spawn_holding(
        journal: Journal,
        lock: Option<Arc<crate::state::StateLock>>,
    ) -> JournalWorker {
        let status = journal.status();
        // Small and bounded: the runner submits one batch per held boundary and waits.
        let (jobs, job_rx) = sync_channel::<JournalJob>(PREPARED_CAPACITY);
        // Unbounded the other way: the worker must never block trying to report a result,
        // or a full ack queue would stall the very thread the simulation is waiting on.
        let (ack_tx, acks) = std::sync::mpsc::channel::<JournalAck>();
        let handle = std::thread::Builder::new()
            .name("cubarium-care-journal".into())
            .spawn(move || {
                let _ownership = lock;
                let mut journal = journal;
                while let Ok(job) = job_rx.recv() {
                    let ack = match job {
                        JournalJob::Accept(commands) => {
                            let result = journal.append_accepted(&commands);
                            JournalAck::Accepted { commands, result }
                        }
                        JournalJob::Outcome(records) => {
                            JournalAck::Outcome { result: journal.append_outcomes(&records) }
                        }
                    };
                    if ack_tx.send(ack).is_err() {
                        break;
                    }
                }
                journal
            })
            .expect("spawning the care journal worker");
        JournalWorker { jobs, acks, handle, status }
    }

    /// The size/debt view `/care/status` and the HTTP intake read.
    pub fn status(&self) -> Arc<JournalStatus> {
        Arc::clone(&self.status)
    }

    /// Hand the worker a job.
    ///
    /// This is a *blocking* `SyncSender::send` on a channel of [`PREPARED_CAPACITY`]. It is
    /// safe only because of the runner's discipline, which is the same discipline the
    /// held-boundary protocol already requires: the runner submits one accept batch at a
    /// time and does not step, or submit a second batch, until that one is acknowledged.
    /// Outcome jobs are submitted only after an acknowledgement, so at most a couple of
    /// jobs are ever in flight and the channel cannot fill. `false` means the worker is
    /// gone, which is itself an uncertain result for anything already submitted.
    pub fn submit(&self, job: JournalJob) -> bool {
        self.jobs.send(job).is_ok()
    }

    /// One acknowledgement if there is one, without ever blocking the simulation loop.
    pub fn poll_ack(&self) -> Option<JournalAck> {
        self.acks.try_recv().ok()
    }

    /// Block for one acknowledgement, at most `timeout`. Used on the shutdown path and in
    /// tests; the loop itself polls.
    pub fn wait_ack(&self, timeout: Duration) -> Option<JournalAck> {
        self.acks.recv_timeout(timeout).ok()
    }

    /// Stop the thread and take the journal back. Dropping the job channel is what asks
    /// the worker to finish; it drains whatever it was already given first.
    pub fn shutdown(self) -> Option<Journal> {
        let JournalWorker { jobs, acks, handle, status: _ } = self;
        drop(jobs);
        let journal = handle.join().ok();
        drop(acks);
        journal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> CareService {
        CareService::new("epoch-test", Arc::new(JournalStatus::default()))
    }

    fn target() -> CareTarget {
        CareTarget { face: 0, u: 10, v: 20 }
    }

    #[test]
    fn a_target_outside_the_charts_is_refused_rather_than_clamped() {
        assert!(CareTarget { face: 0, u: 0, v: 0 }.validate().is_ok());
        assert!(CareTarget { face: 4, u: 63, v: 63 }.validate().is_ok());
        assert!(CareTarget { face: 5, u: 0, v: 0 }.validate().is_err());
        assert!(CareTarget { face: 0, u: 64, v: 0 }.validate().is_err());
        assert!(CareTarget { face: 0, u: 0, v: 200 }.validate().is_err());
    }

    #[test]
    fn the_cooldowns_are_the_contracts_seconds_in_simulation_ticks() {
        assert_eq!(CareKind::Feed.cooldown_ticks(), 600);
        assert_eq!(CareKind::Rain.cooldown_ticks(), 1200);
        assert_eq!(CareKind::Clean.cooldown_ticks(), 600);
    }

    #[test]
    fn an_unregistered_identity_is_retired_and_never_registered_by_a_post() {
        let care = service();
        let out = care.shared().submit("someone-elses-id", 1, CareKind::Feed, target());
        assert!(matches!(out, SubmitOutcome::Conflict("retired")), "{out:?}");
        assert_eq!(out.http_status(), "409 Conflict");
        // And nothing was registered as a side effect.
        assert_eq!(care.shared().lock().clients.len(), 0);
    }

    #[test]
    fn identities_embed_the_epoch_and_are_capped() {
        let care = service();
        let shared = care.shared();
        let mut ids = Vec::new();
        for _ in 0..MAX_CLIENTS {
            match shared.register() {
                RegisterOutcome::Registered { client, epoch } => {
                    assert!(client.starts_with("epoch-test."), "{client}");
                    assert_eq!(epoch, "epoch-test");
                    ids.push(client);
                }
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(ids.len(), MAX_CLIENTS);
        let full = shared.register();
        assert!(matches!(full, RegisterOutcome::Limited("too many clients")), "{full:?}");
        assert_eq!(full.http_status(), "429 Too Many Requests");
    }

    /// A submit that never gets an acknowledgement: the client's answer is `pending`, not
    /// a lie in either direction. The wait is shortened by settling the row instead.
    #[test]
    fn a_duplicate_returns_the_retained_receipt_and_a_conflicting_payload_is_409() {
        let care = service();
        let shared = care.shared();
        let RegisterOutcome::Registered { client, .. } = shared.register() else {
            panic!("registration must succeed")
        };

        // Commit the first request out of band so `submit` does not wait five seconds.
        let waiter = {
            let shared = Arc::clone(&shared);
            let client = client.clone();
            std::thread::spawn(move || shared.submit(&client, 1, CareKind::Feed, target()))
        };
        let planned = wait_for_planned(&care, 1);
        care.commit_accepted(&planned);
        let first = waiter.join().unwrap();
        assert!(matches!(first, SubmitOutcome::Accepted { seq: 1, apply_after_tick: 40 }), "{first:?}");

        // The same identity and payload gets the retained receipt, with no second command.
        let again = shared.submit(&client, 1, CareKind::Feed, target());
        let SubmitOutcome::Duplicate(receipt) = &again else { panic!("{again:?}") };
        assert_eq!(again.http_status(), "200 OK");
        assert!(receipt.contains(r#""duplicate":true"#), "{receipt}");
        assert!(receipt.contains(r#""seq":1"#), "{receipt}");

        // A different payload under the same request number is a conflict.
        let other = CareTarget { face: 2, u: 1, v: 1 };
        let clash = shared.submit(&client, 1, CareKind::Clean, other);
        assert!(matches!(clash, SubmitOutcome::Conflict("conflict")), "{clash:?}");

        // A request number at or below the high-water mark with no retained row is stale.
        care.record_outcome(1, "applied", "", serde_json::json!({}));
        let mut inner = shared.lock();
        inner.rows.clear();
        drop(inner);
        let stale = shared.submit(&client, 1, CareKind::Feed, target());
        assert!(matches!(stale, SubmitOutcome::Conflict("stale")), "{stale:?}");
    }

    /// Drain the FIFO from the test thread, standing in for the runner's boundary.
    fn wait_for_planned(care: &CareService, next_seq: u64) -> Vec<PlannedCommand> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let planned = care.drain_prepared(next_seq, 40);
            if !planned.is_empty() {
                return planned;
            }
            assert!(Instant::now() < deadline, "the prepared FIFO stayed empty");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// The outstanding limit (4) binds before the prepared FIFO (8) ever can, so `503
    /// intake full` is defence in depth rather than a path a client can reach. What a
    /// burst actually gets is `429`.
    #[test]
    fn a_burst_past_the_outstanding_limit_is_429() {
        let care = service();
        let shared = care.shared();
        let RegisterOutcome::Registered { client, .. } = shared.register() else { panic!() };
        // Four commands of different kinds would hit the per-kind cooldowns, so the burst
        // uses one kind and the cooldown is disabled by advancing the published tick.
        let mut request = 0u64;
        let mut submit = |tick: u64| {
            request += 1;
            care.publish_tick(tick);
            let shared = Arc::clone(&shared);
            let client = client.clone();
            let request = request;
            std::thread::spawn(move || shared.submit(&client, request, CareKind::Feed, target()))
        };
        let mut waiters = Vec::new();
        for i in 0..MAX_OUTSTANDING as u64 {
            waiters.push(submit(i * 1000));
            // Serialize intake so the outstanding count is deterministic.
            let deadline = Instant::now() + Duration::from_secs(5);
            while care.outstanding() as u64 != i + 1 {
                assert!(Instant::now() < deadline, "intake stalled at {i}");
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        assert_eq!(care.outstanding(), MAX_OUTSTANDING);
        care.publish_tick(100_000);
        let refused = shared.submit(&client, 99, CareKind::Feed, target());
        assert!(matches!(refused, SubmitOutcome::Limited(_)), "{refused:?}");
        assert_eq!(refused.http_status(), "429 Too Many Requests");

        // Release everyone so the test threads finish.
        let planned = care.drain_prepared(1, 40);
        care.commit_accepted(&planned);
        for w in waiters {
            let _ = w.join();
        }
    }

    #[test]
    fn a_cooldown_inside_the_window_is_429_and_outside_it_is_not() {
        let care = service();
        let shared = care.shared();
        let RegisterOutcome::Registered { client, .. } = shared.register() else { panic!() };
        care.publish_tick(1_000);
        let waiter = {
            let shared = Arc::clone(&shared);
            let client = client.clone();
            std::thread::spawn(move || shared.submit(&client, 1, CareKind::Rain, target()))
        };
        let planned = wait_for_planned(&care, 1);
        care.commit_accepted(&planned);
        let _ = waiter.join();
        care.record_outcome(1, "applied", "", serde_json::json!({}));

        // One tick later: still inside rain's 1200-tick cooldown.
        care.publish_tick(1_001);
        let soon = shared.submit(&client, 2, CareKind::Rain, target());
        assert!(matches!(soon, SubmitOutcome::Limited("cooldown")), "{soon:?}");
        // And `/care/status` shows how much of the cooldown is left, in ticks.
        let status: serde_json::Value = serde_json::from_str(&shared.status_json()).unwrap();
        assert_eq!(status["cooldowns"]["rain"], CareKind::Rain.cooldown_ticks() - 1);
        assert_eq!(status["cooldowns"]["feed"], 0, "another kind is unaffected");

        care.publish_tick(1_000 + CareKind::Rain.cooldown_ticks());
        let planned_after = {
            let shared = Arc::clone(&shared);
            let client = client.clone();
            std::thread::spawn(move || shared.submit(&client, 3, CareKind::Rain, target()))
        };
        let planned = wait_for_planned(&care, 2);
        assert_eq!(planned.len(), 1, "the cooldown has expired");
        care.commit_accepted(&planned);
        let _ = planned_after.join();
    }

    #[test]
    fn a_journal_at_its_bound_refuses_care_while_the_world_keeps_running() {
        let status = Arc::new(JournalStatus::default());
        let care = CareService::new("epoch-test", Arc::clone(&status));
        let shared = care.shared();
        let RegisterOutcome::Registered { client, .. } = shared.register() else { panic!() };
        status.set_for_test(JOURNAL_LIMIT, 0);
        let refused = shared.submit(&client, 1, CareKind::Feed, target());
        assert!(matches!(refused, SubmitOutcome::Unavailable("journal full")), "{refused:?}");
        assert_eq!(refused.http_status(), "503 Service Unavailable");
        assert!(refused.body().contains("journal full"), "{}", refused.body());
    }

    #[test]
    fn a_failed_journal_closes_intake_and_answers_every_waiting_client() {
        let care = service();
        let shared = care.shared();
        let RegisterOutcome::Registered { client, .. } = shared.register() else { panic!() };
        let waiter = {
            let shared = Arc::clone(&shared);
            let client = client.clone();
            std::thread::spawn(move || shared.submit(&client, 1, CareKind::Feed, target()))
        };
        // Wait for intake, then fail before anything is committed.
        let deadline = Instant::now() + Duration::from_secs(5);
        while care.outstanding() == 0 {
            assert!(Instant::now() < deadline, "intake stalled");
            std::thread::sleep(Duration::from_millis(1));
        }
        care.fail("injected: fsync failed with an uncertain result");
        let answer = waiter.join().unwrap();
        assert!(matches!(answer, SubmitOutcome::Unavailable("care failed")), "{answer:?}");
        assert_eq!(care.failure().as_deref(), Some("injected: fsync failed with an uncertain result"));

        let after = shared.submit(&client, 2, CareKind::Clean, target());
        assert!(matches!(after, SubmitOutcome::Unavailable("care failed")), "{after:?}");
        let status = shared.status_json();
        assert!(status.contains(r#""care":"failed""#), "{status}");
        assert!(status.contains("uncertain result"), "{status}");
    }

    #[test]
    fn the_status_reports_the_hold_the_epoch_and_the_journal_bound() {
        let status = Arc::new(JournalStatus::default());
        let care = CareService::new("epoch-abc", Arc::clone(&status));
        care.publish_tick(4242);
        care.hold_at(4242);
        let body = care.shared().status_json();
        let v: serde_json::Value = serde_json::from_str(&body).expect(&body);
        assert_eq!(v["enabled"], true);
        assert_eq!(v["care"], "holding");
        assert_eq!(v["holding_at"], 4242);
        assert_eq!(v["epoch"], "epoch-abc");
        assert_eq!(v["world_tick"], 4242);
        assert_eq!(v["journal"]["limit"], JOURNAL_LIMIT);
        assert_eq!(v["cooldowns"]["feed"], 0);
        assert!(v["receipts"].as_array().unwrap().is_empty());

        care.release_hold();
        let v: serde_json::Value = serde_json::from_str(&care.shared().status_json()).unwrap();
        assert_eq!(v["care"], "ready");
        assert!(v["holding_at"].is_null());

        let disabled: serde_json::Value =
            serde_json::from_str(&CareShared::disabled_status_json()).unwrap();
        assert_eq!(disabled["enabled"], false);
        assert_eq!(disabled["care"], "disabled");
    }

    #[test]
    fn the_retained_receipt_window_is_bounded() {
        let care = service();
        let mut inner = care.shared.lock();
        for i in 0..(RETAINED_RECEIPTS as u64 * 2) {
            while inner.rows.len() >= RETAINED_RECEIPTS {
                inner.rows.pop_front();
            }
            inner.rows.push_back(Row {
                client: "c".into(),
                request: i,
                kind: CareKind::Feed,
                target: target(),
                seq: Some(i),
                apply_after_tick: Some(0),
                state: RowState::Applied,
                reason: String::new(),
                applied: serde_json::Value::Null,
            });
        }
        assert_eq!(inner.rows.len(), RETAINED_RECEIPTS);
        assert_eq!(inner.rows.front().unwrap().request, RETAINED_RECEIPTS as u64);
    }

    #[test]
    fn the_worker_makes_records_durable_and_acknowledges_them() {
        let dir = std::env::temp_dir()
            .join(format!("cubarium-care-worker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let journal = Journal::open(&dir, "e", "b").unwrap();
        let worker = JournalWorker::spawn(journal);
        let command = PlannedCommand {
            seq: 1,
            apply_after_tick: 40,
            kind: CareKind::Feed,
            target: target(),
            client: "e.1".into(),
            request: 1,
        };
        assert!(worker.submit(JournalJob::Accept(vec![command.clone()])));
        match worker.wait_ack(Duration::from_secs(5)).expect("an acknowledgement") {
            JournalAck::Accepted { commands, result } => {
                result.expect("the append must succeed");
                assert_eq!(commands, vec![command]);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(worker.status().outstanding(), 1);
        let journal = worker.shutdown().expect("the worker returns the journal");
        assert_eq!(journal.accepted_records().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
