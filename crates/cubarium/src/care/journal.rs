//! `<state>/care.jsonl`: the append-only record that makes care survive a crash.
//!
//! Everything the world will be asked to do is durable *before* it is done. The runner
//! holds the simulation at boundary `B` while a record is written and `fsync`ed, so a
//! replay after any crash applies exactly the commands the original history applied, at
//! exactly the boundaries it applied them at. That is the whole reason this file exists;
//! see `design/7_Research/care-contract-2026-09-12.md` "Host: admission at a held
//! boundary" and the review that corrected the earlier asynchronous design.
//!
//! Three rules the reviews insisted on, implemented here:
//!
//! * **A torn final line is truncated back to the verified prefix, at open and only at
//!   open.** Appending after a torn tail would turn that tail into *interior* corruption,
//!   which no later reader can distinguish from a real record. An invalid interior line is
//!   a recovery failure and refuses to start: it is not ours to guess at.
//! * **A failed append or `fsync` is not proof the record is absent, and nothing is ever
//!   appended after one.** [`Journal::append`] reports the error and the runner holds the
//!   boundary with care failed until a clean stop. There is deliberately no `abort` record
//!   and no resume-after-failure path: the follow-up review showed that writing an abort
//!   on top of an uncertain suffix can produce one malformed concatenated line, and that a
//!   reserved sequence whose `accepted` record never reached the file cannot be recovered
//!   from an abort at all (an abort does not name the boundary, and replay would find a
//!   hole in the sequence). Holding until a restart is an explicit availability trade: a
//!   disk failure pauses the world instead of risking a history nobody can reconstruct.
//!   Sequence numbers reserved for records that did not survive were never applied and are
//!   simply never consumed — the restarted process allocates after the surviving maximum.
//! * **The file is bounded.** An append-only journal grows forever, and dropping old
//!   records would lose the per-client high-water marks that stop an old request from
//!   being applied twice. So there is a hard 4 MiB bound with a reserve for the records
//!   already-accepted commands still owe, and care is refused (`503`) before it is hit
//!   while autonomous life continues. No compaction in this slice.
//!
//! ## Two accepted records, and why the amount gets its own discriminator
//!
//! A standard-dose command is still written as `{"rec":"accepted", …}`, exactly as before: it
//! carries no amount because the amount it carries is the only one that build could mean, and
//! an older binary reading it applies precisely the right thing. A **nonstandard** command is
//! written as `{"rec":"accepted_dose_v1", …,"dose_permille":N}`. The new discriminator is the
//! point: an older binary does not know the record kind, so it refuses to start rather than
//! reading the line as an ordinary command and quietly applying the wrong amount. An extra
//! field on the old record would have been ignored, which is exactly the silent failure this
//! avoids.
//!
//! Parsing is deliberately asymmetric, for the same reason:
//!
//! * `accepted` must have **no** `dose_permille` — its presence, even at the standard 1000, is
//!   refused. A record that carried an amount under the old name would mean a writer somewhere
//!   believed old readers would honour it.
//! * `accepted_dose_v1` must have an explicit, valid, integral `dose_permille`. Missing never
//!   defaults to standard: a dropped field would silently turn a Generous command into an
//!   ordinary one.
//!
//! Anything else — `accepted_dose_v2`, a fractional or out-of-range amount, a malformed line —
//! fails closed at open, including at the final newline-terminated record. Only an actually
//! unterminated tail is truncatable.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::{CareDose, CareKind, CareTarget, OutcomeRecord, PlannedCommand};
use crate::state::JOURNAL_NAME;

/// The journal's hard byte bound.
pub const JOURNAL_LIMIT: u64 = 4 * 1024 * 1024;
/// Read at open before rejecting an oversized file: the bound plus enough to *prove* the
/// file is over it without ever allocating the whole of an arbitrarily large file.
const READ_SENTINEL: u64 = 4096;
/// Reserved for each accepted record that does not yet have an outcome record, so a
/// command already promised durability can always record what it did.
pub const OUTCOME_RESERVE: u64 = 512;
/// Budgeted for one `accepted` or `accepted_dose_v1` record. Client ids, targets and the
/// four-digit amount are fixed-width, so a real line is well under this.
pub const ACCEPTED_ESTIMATE: u64 = 512;

/// Why a journal write did not happen.
///
/// The distinction is load-bearing, and it is a distinction of *origin*, never of error
/// kind. [`JournalError::Full`] can only come from the advertised-capacity comparison,
/// which runs before a single byte is seeked, written or synced: nothing was attempted, so
/// nothing is uncertain, the sequence numbers go back and the world keeps stepping.
///
/// Everything that happens once I/O has begun is [`JournalError::Uncertain`] — including
/// `ENOSPC`. A write can partially succeed before the disk fills, and a `sync_all` can fail
/// after complete record bytes reached the file; the OS error kind says nothing about which
/// happened. Reading `StorageFull` as "the file is unchanged" would reclaim a sequence
/// number for a record that survived, and a later restart would then apply it at a boundary
/// the original world walked past. So the rule is simply: do not infer from the error kind
/// whether bytes changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalError {
    /// The advertised bound would be exceeded. No write was attempted.
    Full(String),
    /// I/O had begun and failed. The bytes may or may not be there.
    Uncertain(String),
}

impl JournalError {
    /// True for a capacity refusal: care is refused, the world is not held.
    pub fn is_full(&self) -> bool {
        matches!(self, JournalError::Full(_))
    }

    /// The message, for stderr and `/care/status`.
    pub fn message(&self) -> &str {
        match self {
            JournalError::Full(m) | JournalError::Uncertain(m) => m,
        }
    }
}

impl std::fmt::Display for JournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JournalError::Full(m) => write!(f, "the care journal is full: {m}"),
            JournalError::Uncertain(m) => write!(f, "uncertain care journal write: {m}"),
        }
    }
}

impl std::error::Error for JournalError {}

/// The journal's size and debt, published for `/care/status` and for the intake check on
/// the HTTP thread, which must never touch the disk.
#[derive(Debug, Default)]
pub struct JournalStatus {
    bytes: AtomicU64,
    outstanding: AtomicU64,
}

impl JournalStatus {
    /// Current file size in bytes.
    pub fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    /// Accepted records that still owe an outcome record.
    pub fn outstanding(&self) -> u64 {
        self.outstanding.load(Ordering::Relaxed)
    }

    /// The bound itself, so `/care/status` can report `{bytes, limit}`.
    pub fn limit(&self) -> u64 {
        JOURNAL_LIMIT
    }

    /// Force the counters, for tests that need the bound reached without writing 4 MiB.
    pub fn set_for_test(&self, bytes: u64, outstanding: u64) {
        self.bytes.store(bytes, Ordering::Relaxed);
        self.outstanding.store(outstanding, Ordering::Relaxed);
    }

    /// True when accepting `new_records` more commands could not be completed inside the
    /// bound — counting both the accepted records themselves and the outcome records every
    /// outstanding command is still owed.
    pub fn would_overflow(&self, new_records: u64) -> bool {
        let needed = self.bytes()
            + new_records * ACCEPTED_ESTIMATE
            + (self.outstanding() + new_records) * OUTCOME_RESERVE;
        needed > JOURNAL_LIMIT
    }
}

/// Test hooks on the durable path. Public because the evidence the contract asks for —
/// "the world provably stays at `B` under a delayed acknowledgement" and "a complete
/// record visible on reopen after an uncertain sync failure" — cannot be produced from
/// outside the process any other way. All three default to off and cost one relaxed
/// atomic load per append.
#[derive(Clone, Debug, Default)]
pub struct JournalHooks {
    /// Milliseconds to sleep before acknowledging an append.
    pub delay_ms: Arc<AtomicU64>,
    /// Fail this many appends *after* the bytes are durably on disk: the ambiguous case,
    /// where the record survives but the writer reported an error.
    pub fail_after_sync: Arc<AtomicU64>,
    /// Fail this many appends before writing anything: the unambiguous case.
    pub fail_before_write: Arc<AtomicU64>,
    /// When nonzero, the next append writes only this many bytes, `fsync`s them, and then
    /// reports a failure. Set it inside a JSON record to reproduce a torn line; set it to
    /// exactly one record's length to reproduce a batch whose first record survived and
    /// whose second never reached the file. Consumed by the append that uses it.
    pub short_write_bytes: Arc<AtomicU64>,
    /// Fail this many appends with `ENOSPC` *after* the seek has begun: a real disk filling
    /// up, which must be treated as uncertain and not as a capacity refusal.
    pub enospc_after_write: Arc<AtomicU64>,
    /// Incremented every time the parent directory is synced at open. A test reads it to
    /// establish that the barrier happened before any accepted append could.
    pub dir_syncs: Arc<AtomicU64>,
    /// Fail the next `n` parent-directory syncs at open.
    pub fail_dir_sync: Arc<AtomicU64>,
}

impl JournalHooks {
    /// Delay every append by `ms`.
    pub fn delay(&self, ms: u64) {
        self.delay_ms.store(ms, Ordering::Relaxed);
    }

    /// Make the next `n` appends fail with the record already durable.
    pub fn fail_after_sync(&self, n: u64) {
        self.fail_after_sync.store(n, Ordering::Relaxed);
    }

    /// Make the next `n` appends fail before touching the file.
    pub fn fail_before_write(&self, n: u64) {
        self.fail_before_write.store(n, Ordering::Relaxed);
    }

    /// Make the next append write only `bytes` of its payload, then fail.
    pub fn short_write(&self, bytes: u64) {
        self.short_write_bytes.store(bytes, Ordering::Relaxed);
    }

    /// Make the next `n` appends fail with `ENOSPC` once I/O has begun.
    pub fn enospc(&self, n: u64) {
        self.enospc_after_write.store(n, Ordering::Relaxed);
    }

    /// How many times a journal open has synced the parent directory.
    pub fn dir_sync_count(&self) -> u64 {
        self.dir_syncs.load(Ordering::Relaxed)
    }

    /// Make the next `n` parent-directory syncs fail.
    pub fn fail_dir_sync(&self, n: u64) {
        self.fail_dir_sync.store(n, Ordering::Relaxed);
    }

    /// Attach these hooks to the next journal [`Journal::open`] creates, wherever it is
    /// opened from — including inside `run_world`, which builds its own.
    ///
    /// The evidence the contract asks for ("the world provably stays at `B` under a delayed
    /// acknowledgement") is about the *runner's* journal, which an integration test has no
    /// other handle on. Process-global and therefore one test at a time: the integration
    /// tests that use it serialize on their own mutex. [`JournalHooks::uninstall`] puts it
    /// back.
    pub fn install(&self) {
        *installed().lock().unwrap_or_else(|e| e.into_inner()) = Some(self.clone());
    }

    /// Remove any installed hooks.
    pub fn uninstall() {
        *installed().lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// The process-global slot [`JournalHooks::install`] writes to.
fn installed() -> &'static std::sync::Mutex<Option<JournalHooks>> {
    static INSTALLED: std::sync::OnceLock<std::sync::Mutex<Option<JournalHooks>>> =
        std::sync::OnceLock::new();
    INSTALLED.get_or_init(|| std::sync::Mutex::new(None))
}

/// The open journal. One per run, owned by the journal worker thread.
#[derive(Debug)]
pub struct Journal {
    path: PathBuf,
    file: File,
    status: Arc<JournalStatus>,
    hooks: JournalHooks,
    epoch: String,
    /// Every `accepted` record read at open, in file order.
    accepted: Vec<PlannedCommand>,
    outcomes: HashSet<u64>,
    /// Bytes dropped from a torn final line, for the report and the test.
    truncated: u64,
    /// False when the file was already at its bound at open: history replays, nothing new
    /// is ever appended, and the service refuses new care for the whole run.
    care_available: bool,
    /// Set by the *first* uncertain write. Every later append is refused here, in the
    /// writer, without touching the file.
    ///
    /// The runner also stops submitting once it sees the failed acknowledgement, but that
    /// is one poll too late: jobs queued before the failure — a diagnostic outcome, a
    /// second accepted batch — are already in the worker's channel and would be appended
    /// on top of a torn suffix, turning it into interior corruption that no later open can
    /// repair. The writer has to be the thing that stops.
    poisoned: Option<String>,
}

impl Journal {
    /// Open (or create) `<dir>/care.jsonl`, verify every line, truncate a torn final line
    /// back to the verified prefix, and append this run's `epoch` record.
    ///
    /// The caller must already hold the state lock: this both reads and rewrites the file.
    pub fn open(dir: &Path, epoch: &str, build: &str) -> Result<Journal> {
        let hooks = installed()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or_default();
        Journal::open_with_hooks(dir, epoch, build, hooks)
    }

    /// [`Journal::open`] with the durable-path test hooks attached.
    pub fn open_with_hooks(
        dir: &Path,
        epoch: &str,
        build: &str,
        hooks: JournalHooks,
    ) -> Result<Journal> {
        let path = dir.join(JOURNAL_NAME);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("opening the care journal {}", path.display()))?;
        // Bounded before allocation: an oversized file is rejected on the strength of the
        // first `JOURNAL_LIMIT + READ_SENTINEL` bytes rather than by reading all of it.
        let mut raw = Vec::new();
        (&file)
            .take(JOURNAL_LIMIT + READ_SENTINEL)
            .read_to_end(&mut raw)
            .with_context(|| format!("reading the care journal {}", path.display()))?;
        if raw.len() as u64 > JOURNAL_LIMIT {
            anyhow::bail!(
                "{}: the care journal is larger than its {JOURNAL_LIMIT}-byte bound. \
                 This host never writes past that bound, so the file was grown by something \
                 else; it is not truncated automatically. Move it aside deliberately.",
                path.display()
            );
        }

        let verified = verify(&raw).map_err(|e| {
            anyhow::anyhow!(
                "{}: {e}\nthe care journal cannot be trusted; move it aside deliberately \
                 rather than letting a world resume from a history it cannot reconstruct",
                path.display()
            )
        })?;

        let truncated = raw.len() as u64 - verified.prefix as u64;
        if truncated > 0 {
            file.set_len(verified.prefix as u64).with_context(|| {
                format!("truncating the torn tail of {}", path.display())
            })?;
            file.sync_all().with_context(|| format!("syncing {}", path.display()))?;
            eprintln!(
                "cubarium: {}: a torn final line of {truncated} byte(s) was truncated back to the \
                 last complete record; the verified prefix is unchanged",
                path.display()
            );
        }

        // `create(true)` makes a *directory entry*, and syncing the file's own contents
        // says nothing about whether that entry survives a power loss — the same reason
        // `state::write_snapshot` fsyncs the directory after its rename. A resumed world
        // receiving its first ever care has no opening checkpoint to do this for it, so it
        // could otherwise acknowledge an accepted command whose journal pathname is not yet
        // durable, and lose the whole history while keeping the old checkpoint. Done on
        // every open; it costs one `fsync` per run.
        if take_one(&hooks.fail_dir_sync) {
            anyhow::bail!(
                "{}: injected failure syncing the care journal's directory (test hook); no \
                 command may be accepted until the journal's own pathname is durable",
                dir.display()
            );
        }
        File::open(dir)
            .and_then(|d| d.sync_all())
            .with_context(|| {
                format!(
                    "syncing {} so the care journal's directory entry is durable before any \
                     command is accepted",
                    dir.display()
                )
            })?;
        hooks.dir_syncs.fetch_add(1, Ordering::Relaxed);

        let status = Arc::new(JournalStatus::default());
        status.bytes.store(verified.prefix as u64, Ordering::Relaxed);
        let owing = verified
            .accepted
            .iter()
            .filter(|c| !verified.outcomes.contains(&c.seq))
            .count() as u64;
        status.outstanding.store(owing, Ordering::Relaxed);

        let mut journal = Journal {
            path,
            file,
            status,
            hooks,
            epoch: epoch.to_string(),
            accepted: verified.accepted,
            outcomes: verified.outcomes,
            truncated,
            care_available: true,
            poisoned: None,
        };
        // The epoch record is what makes the client identities of this run distinct from
        // every earlier run's, so it goes down durably before any of them are issued.
        // Hand-built for the contract's key order, and deliberately outside the test
        // hooks: those exist to make *care* writes fail, not to fake a broken open.
        let line = format!(
            "{{\"rec\":\"epoch\",\"epoch\":{},\"build\":{}}}\n",
            serde_json::Value::from(journal.epoch.as_str()),
            serde_json::Value::from(build),
        );
        // A journal already at its bound must still be *opened*: its history has to be
        // validated and replayed, and a new world or a new journal beside it would lose
        // exactly the history the bound exists to protect. What it cannot do is grow, so
        // when even the epoch record does not fit, care is disabled for this run and the
        // file is left exactly as it was.
        if journal.fits(line.len() as u64, 0) {
            journal.write_durably(line.as_bytes()).map_err(|e| {
                anyhow::anyhow!(
                    "writing the epoch record to {}: {e}",
                    journal.path.display()
                )
            })?;
        } else {
            journal.care_available = false;
            eprintln!(
                "cubarium: {}: the care journal is at its {JOURNAL_LIMIT}-byte bound, so this \
                 run writes no epoch record and accepts no new care. Existing history is still \
                 validated and replayed, and autonomous life is unaffected.",
                journal.path.display()
            );
        }
        Ok(journal)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    /// The shared size/debt view the HTTP intake and `/care/status` read.
    pub fn status(&self) -> Arc<JournalStatus> {
        Arc::clone(&self.status)
    }

    /// Bytes dropped from a torn final line at open (0 when the file was intact).
    pub fn truncated_bytes(&self) -> u64 {
        self.truncated
    }

    /// False when the journal was already at its bound at open: this run replays what is
    /// there and accepts nothing new, without growing the file by so much as an epoch line.
    pub fn care_available(&self) -> bool {
        self.care_available
    }

    /// Why this journal stopped accepting writes, if an uncertain one poisoned it.
    pub fn poisoned(&self) -> Option<&str> {
        self.poisoned.as_deref()
    }

    /// Would `len` more bytes fit, with `reserve_for` commands still owing an outcome
    /// record afterwards?
    fn fits(&self, len: u64, reserve_for: u64) -> bool {
        self.status.bytes() + len + reserve_for * OUTCOME_RESERVE <= JOURNAL_LIMIT
    }

    /// Every `accepted` record in the file, in file order, as read at open.
    pub fn accepted_records(&self) -> &[PlannedCommand] {
        &self.accepted
    }

    /// The highest sequence number any surviving record claims, or `None` for an empty
    /// journal. New commands allocate *after* this, never after the snapshot's cursor: a
    /// record in the journal has already reserved its sequence, whether or not the world
    /// has reached its boundary yet.
    pub fn max_seq(&self) -> Option<u64> {
        self.accepted.iter().map(|c| c.seq).max()
    }

    /// Append the accepted records for one held boundary and `fsync`.
    ///
    /// [`JournalError::Uncertain`] is *ambiguous*: some or all of the records may be on
    /// disk, and the caller must not treat it as "the command did not happen".
    /// [`JournalError::Full`] is not ambiguous at all — nothing was attempted — so the
    /// caller refuses the care and keeps the world running.
    pub fn append_accepted(&mut self, records: &[PlannedCommand]) -> Result<(), JournalError> {
        if records.is_empty() {
            return Ok(());
        }
        let mut text = String::new();
        for record in records {
            text.push_str(&accepted_line(record));
            text.push('\n');
        }
        // Every one of these will owe an outcome record, and so does everything already
        // outstanding; the bound has to hold for all of them.
        let reserve_for = self.status.outstanding() + records.len() as u64;
        if !self.fits(text.len() as u64, reserve_for) {
            return Err(JournalError::Full(format!(
                "{} is within {OUTCOME_RESERVE} bytes per outstanding command of its \
                 {JOURNAL_LIMIT}-byte bound",
                self.path.display()
            )));
        }
        let result = self.append(&text);
        // Counted as owed whatever the writer said: an ambiguous failure may still have
        // left the record on disk, and the reserve must cover it either way.
        self.status.outstanding.fetch_add(records.len() as u64, Ordering::Relaxed);
        for record in records {
            self.accepted.push(record.clone());
        }
        result
    }

    /// Append the diagnostic `outcome` records. These report what happened; replay never
    /// needs them to reconstruct timing, so a failure is logged and the world goes on.
    ///
    /// These always fit: [`OUTCOME_RESERVE`] bytes were held back for each of them when
    /// their command was accepted, and writing one releases its own reserve. The guard is
    /// still here, because "always" is a claim about arithmetic that should be checked.
    pub fn append_outcomes(&mut self, records: &[OutcomeRecord]) -> Result<(), JournalError> {
        if records.is_empty() {
            return Ok(());
        }
        let mut text = String::new();
        for record in records {
            text.push_str(&outcome_line(record));
            text.push('\n');
        }
        let reserve_for = self.status.outstanding().saturating_sub(records.len() as u64);
        if !self.fits(text.len() as u64, reserve_for) {
            return Err(JournalError::Full(format!(
                "{}: no room even for the outcome records the bound reserved space for",
                self.path.display()
            )));
        }
        let result = self.append(&text);
        for record in records {
            self.outcomes.insert(record.seq);
        }
        self.settle(records.len() as u64);
        result
    }

    /// What this run still has to do, from the journal alone: every surviving `accepted`
    /// record the snapshot has not already admitted, in sequence order.
    ///
    /// The *whole* schedule is validated here, before a single tick is stepped, because
    /// sorting malformed input by boundary establishes nothing. Three properties must
    /// hold, and any of them failing stops the process rather than being skipped past:
    ///
    /// 1. sequence numbers are contiguous from `admitted_seq + 1` — the core admits only
    ///    `admitted_seq + 1`, so a hole can never be applied;
    /// 2. boundaries are nondecreasing in sequence order — a later command cannot belong
    ///    to an earlier boundary;
    /// 3. no boundary is before `snapshot_tick` — ecology has already run past it, and
    ///    applying there would rewrite executed history.
    ///
    /// Sequence numbers reserved by the previous process whose records did not survive
    /// simply are not here. They were never applied, so they are never consumed; the
    /// restarted process allocates after [`Journal::max_seq`].
    pub fn replay_plan(&self, admitted_seq: u64, snapshot_tick: u64) -> Result<Vec<PlannedCommand>> {
        let mut plan: Vec<PlannedCommand> =
            self.accepted.iter().filter(|c| c.seq > admitted_seq).cloned().collect();
        plan.sort_by_key(|c| c.seq);

        let mut previous_boundary = 0u64;
        for (i, command) in plan.iter().enumerate() {
            let expected = admitted_seq + 1 + i as u64;
            if command.seq != expected {
                bail!(
                    "{}: the replay schedule is not contiguous: expected seq {expected}, found \
                     {}. The snapshot and the journal disagree about what was already applied; \
                     this is a recovery error, not something to skip past.",
                    self.path.display(),
                    command.seq
                );
            }
            if command.apply_after_tick < snapshot_tick {
                bail!(
                    "{}: seq {} was accepted for boundary {} but the snapshot is already at tick \
                     {snapshot_tick}. Applying it now would rewrite ecology that has already run; \
                     this is an inconsistent recovery state and the run refuses to start.",
                    self.path.display(),
                    command.seq,
                    command.apply_after_tick
                );
            }
            if i > 0 && command.apply_after_tick < previous_boundary {
                bail!(
                    "{}: seq {} names boundary {} after seq {} named {previous_boundary}. \
                     Boundaries must not go backwards in sequence order.",
                    self.path.display(),
                    command.seq,
                    command.apply_after_tick,
                    command.seq - 1,
                );
            }
            previous_boundary = command.apply_after_tick;
        }
        Ok(plan)
    }

    /// One durable append: the poison gate, the capacity gate, the test hooks, then write
    /// and `fsync`.
    fn append(&mut self, text: &str) -> Result<(), JournalError> {
        if let Some(reason) = &self.poisoned {
            // Refused here, with the file untouched. Whatever the earlier write left on
            // disk stays exactly as it is, so the next open sees either a complete prefix
            // or one torn suffix it can repair — never a suffix with records written after
            // it.
            return Err(JournalError::Uncertain(format!(
                "{}: refused; an earlier write left an uncertain result ({reason}) and this \
                 journal accepts nothing further in this process",
                self.path.display()
            )));
        }
        if !self.care_available {
            return Err(JournalError::Full(format!(
                "{}: the journal was already at its bound when this run opened it",
                self.path.display()
            )));
        }
        let delay = self.hooks.delay_ms.load(Ordering::Relaxed);
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
        if take_one(&self.hooks.fail_before_write) {
            let e = JournalError::Uncertain(
                "injected journal write failure before the write (test hook)".to_string(),
            );
            self.poison(&e);
            return Err(e);
        }
        let short = self.hooks.short_write_bytes.swap(0, Ordering::Relaxed) as usize;
        if short > 0 {
            // Sliced as bytes, not as `str`: a short write lands wherever the disk stopped,
            // which is not obliged to be a character boundary.
            let partial = &text.as_bytes()[..short.min(text.len())];
            self.write_durably(partial).inspect_err(|e| self.poison(e))?;
            let e = JournalError::Uncertain(
                "injected short journal write (test hook); the partial record is durable"
                    .to_string(),
            );
            self.poison(&e);
            return Err(e);
        }
        self.write_durably(text.as_bytes()).inspect_err(|e| self.poison(e))?;
        if take_one(&self.hooks.fail_after_sync) {
            // The bytes above are on disk. This is the case the review named: the writer
            // reported a failure and the record survives anyway.
            let e = JournalError::Uncertain(
                "injected journal write failure after the sync (test hook); the record is durable"
                    .to_string(),
            );
            self.poison(&e);
            return Err(e);
        }
        Ok(())
    }

    /// Remember an uncertain failure so nothing is ever appended after it. A capacity
    /// refusal is not a poison: nothing was attempted and the file is intact.
    fn poison(&mut self, e: &JournalError) {
        if let JournalError::Uncertain(reason) = e
            && self.poisoned.is_none()
        {
            self.poisoned = Some(reason.clone());
        }
    }

    /// Append at the end of the file and `fsync`.
    ///
    /// The one place bytes reach the file, and therefore the one place the advertised bound
    /// is enforced against *actual* lengths rather than estimates — epoch records and
    /// outcome records included. The capacity comparison happens first, before any seek, so
    /// a caller that got its arithmetic wrong is refused with a typed
    /// [`JournalError::Full`] and nothing has been attempted.
    ///
    /// Past that point every failure is [`JournalError::Uncertain`], `ENOSPC` included.
    fn write_durably(&mut self, bytes: &[u8]) -> Result<(), JournalError> {
        if self.status.bytes() + bytes.len() as u64 > JOURNAL_LIMIT {
            return Err(JournalError::Full(format!(
                "{}: {} more bytes would take the care journal past its {JOURNAL_LIMIT}-byte \
                 bound",
                self.path.display(),
                bytes.len()
            )));
        }
        // --- nothing below here can report `Full` ---------------------------------
        let uncertain = |e: io::Error| JournalError::Uncertain(e.to_string());
        self.file.seek(SeekFrom::End(0)).map_err(uncertain)?;
        if take_one(&self.hooks.enospc_after_write) {
            // A real disk filling up mid-append. Whether any of these bytes landed is
            // exactly what nobody can know, so it is uncertain — never a capacity refusal.
            let _ = self.file.write_all(&bytes[..bytes.len() / 2]);
            let _ = self.file.sync_all();
            self.status.bytes.fetch_add((bytes.len() / 2) as u64, Ordering::Relaxed);
            return Err(uncertain(io::Error::new(
                io::ErrorKind::StorageFull,
                "injected ENOSPC part-way through a journal append (test hook)",
            )));
        }
        self.file.write_all(bytes).map_err(uncertain)?;
        self.file.sync_all().map_err(uncertain)?;
        self.status.bytes.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        Ok(())
    }

    /// A record that owed an outcome no longer does.
    fn settle(&self, n: u64) {
        let _ = self.status.outstanding.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |owed| Some(owed.saturating_sub(n)),
        );
    }
}

/// Decrement a hook counter, returning whether this call was one of the injected failures.
fn take_one(counter: &AtomicU64) -> bool {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            (n > 0).then(|| n - 1)
        })
        .is_ok()
}

/// One accepted line, hand-built so the key order is the contract's.
///
/// A standard dose writes the legacy `accepted` record, with **no** amount field: an older
/// binary reads it and applies exactly the right thing. A nonstandard dose writes
/// `accepted_dose_v1` with an explicit `dose_permille`, which an older binary refuses by name
/// rather than misreading. See the module documentation.
fn accepted_line(c: &PlannedCommand) -> String {
    if c.dose.is_standard() {
        return format!(
            r#"{{"rec":"accepted","seq":{},"apply_after_tick":{},"client":{},"request":{},"kind":"{}","target":{{"face":{},"u":{},"v":{}}}}}"#,
            c.seq,
            c.apply_after_tick,
            serde_json::Value::from(c.client.as_str()),
            c.request,
            c.kind.as_str(),
            c.target.face,
            c.target.u,
            c.target.v,
        );
    }
    format!(
        r#"{{"rec":"accepted_dose_v1","seq":{},"apply_after_tick":{},"client":{},"request":{},"kind":"{}","target":{{"face":{},"u":{},"v":{}}},"dose_permille":{}}}"#,
        c.seq,
        c.apply_after_tick,
        serde_json::Value::from(c.client.as_str()),
        c.request,
        c.kind.as_str(),
        c.target.face,
        c.target.u,
        c.target.v,
        c.dose.permille(),
    )
}

/// One `outcome` line. `applied` carries the quantities the world reported.
fn outcome_line(r: &OutcomeRecord) -> String {
    format!(
        r#"{{"rec":"outcome","seq":{},"tick":{},"outcome":"{}","reason":{},"applied":{}}}"#,
        r.seq,
        r.tick,
        r.outcome,
        serde_json::Value::from(r.reason.as_str()),
        r.applied,
    )
}

/// What reading the file established.
struct Verified {
    /// Byte length of the verified prefix: everything at or past this is a torn tail.
    prefix: usize,
    accepted: Vec<PlannedCommand>,
    outcomes: HashSet<u64>,
}

/// Verify every complete line.
///
/// Only an **unterminated trailing byte suffix** — bytes after the last newline — is
/// treated as a torn tail and truncated away. A newline-terminated record that fails to
/// parse, or parses as JSON but is not a record this host understands, is *not*
/// demonstrably an interrupted append: it could be a scheduled command whose bytes were
/// mangled, or a record from a future version. Dropping it while claiming successful
/// recovery would destroy exactly the evidence someone would need. So it refuses, the
/// bytes are preserved, and a human decides — the same treatment as interior corruption,
/// wherever in the file it sits.
fn verify(raw: &[u8]) -> std::result::Result<Verified, String> {
    let mut complete: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    for (i, &b) in raw.iter().enumerate() {
        if b == b'\n' {
            complete.push((start, i));
            start = i + 1;
        }
    }
    // Bytes after the last newline were never terminated: a torn tail by definition.
    let prefix = start;

    let mut out = Verified { prefix, accepted: Vec::new(), outcomes: HashSet::new() };
    for (index, &(from, to)) in complete.iter().enumerate() {
        let line = &raw[from..to];
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        match parse_record(line) {
            Ok(Record::Epoch) => {}
            Ok(Record::Accepted(command)) => out.accepted.push(command),
            Ok(Record::Outcome { seq }) => {
                out.outcomes.insert(seq);
            }
            Err(reason) => {
                return Err(format!(
                    "line {} is not a valid care record ({reason}); a complete, \
                     newline-terminated record is not an interrupted write, so it is preserved \
                     rather than discarded",
                    index + 1
                ));
            }
        }
    }
    out.prefix = prefix;
    Ok(out)
}

enum Record {
    Epoch,
    Accepted(PlannedCommand),
    Outcome { seq: u64 },
}

fn parse_record(line: &[u8]) -> std::result::Result<Record, String> {
    let value: serde_json::Value =
        serde_json::from_slice(line).map_err(|e| format!("not JSON: {e}"))?;
    let object = value.as_object().ok_or_else(|| "not a JSON object".to_string())?;
    let rec = object.get("rec").and_then(|v| v.as_str()).ok_or("no `rec` string")?;
    let u64_field = |key: &str| -> std::result::Result<u64, String> {
        object
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("no `{key}` number"))
    };
    match rec {
        "epoch" => {
            object.get("epoch").and_then(|v| v.as_str()).ok_or("no `epoch` string")?;
            Ok(Record::Epoch)
        }
        "accepted" | "accepted_dose_v1" => {
            let dose = match rec {
                // The legacy record has no amount, and must not: an `accepted` line carrying
                // `dose_permille` would mean somebody wrote an amount under a name older
                // readers ignore. Refused even when the amount is the standard one, because
                // the writer's intent is what is wrong, not the number.
                "accepted" => {
                    if object.contains_key("dose_permille") {
                        return Err(
                            "an `accepted` record carries no `dose_permille`; a dosed command \
                             must be written as `accepted_dose_v1`, which older readers refuse \
                             by name rather than silently ignoring the amount"
                                .to_string(),
                        );
                    }
                    CareDose::STANDARD
                }
                // The new record must state its amount. Missing never defaults to standard: a
                // dropped field would silently turn a Generous command into an ordinary one.
                _ => {
                    let raw = object
                        .get("dose_permille")
                        .ok_or("an `accepted_dose_v1` record must carry `dose_permille`")?;
                    let n = raw.as_u64().ok_or_else(|| {
                        format!("`dose_permille` must be a non-negative integer, not {raw}")
                    })?;
                    let n = u16::try_from(n)
                        .map_err(|_| format!("`dose_permille` is out of range: {n}"))?;
                    CareDose::new(n)?
                }
            };
            let kind = object.get("kind").and_then(|v| v.as_str()).ok_or("no `kind` string")?;
            let kind = CareKind::parse(kind).ok_or_else(|| format!("unknown kind `{kind}`"))?;
            let target = object.get("target").ok_or("no `target`")?;
            let component = |key: &str| -> std::result::Result<u8, String> {
                let n = target
                    .get(key)
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| format!("target has no `{key}`"))?;
                u8::try_from(n).map_err(|_| format!("target `{key}` is out of range: {n}"))
            };
            let target = CareTarget { face: component("face")?, u: component("u")?, v: component("v")? };
            target.validate().map_err(|e| e.to_string())?;
            Ok(Record::Accepted(PlannedCommand {
                seq: u64_field("seq")?,
                apply_after_tick: u64_field("apply_after_tick")?,
                kind,
                target,
                dose,
                client: object
                    .get("client")
                    .and_then(|v| v.as_str())
                    .ok_or("no `client` string")?
                    .to_string(),
                request: u64_field("request")?,
            }))
        }
        "outcome" => Ok(Record::Outcome { seq: u64_field("seq")? }),
        other => Err(format!("unknown record kind `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("cubarium-journal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn command(seq: u64, boundary: u64, kind: CareKind) -> PlannedCommand {
        PlannedCommand::standard(
            seq,
            boundary,
            kind,
            CareTarget { face: 1, u: 12, v: 34 },
            "epoch-1.7",
            seq,
        )
    }

    /// The same command at a nonstandard amount, which is written as `accepted_dose_v1`.
    fn dosed(seq: u64, boundary: u64, kind: CareKind, permille: u16) -> PlannedCommand {
        PlannedCommand {
            dose: CareDose::new(permille).expect("the test's dose is in range"),
            ..command(seq, boundary, kind)
        }
    }

    fn contents(dir: &Path) -> String {
        std::fs::read_to_string(dir.join(JOURNAL_NAME)).unwrap()
    }

    #[test]
    fn opening_writes_an_epoch_record_and_reopening_keeps_the_history() {
        let dir = scratch("epoch");
        {
            let mut j = Journal::open(&dir, "stamp-1", "0.1.0+abc").unwrap();
            j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap();
        }
        let text = contents(&dir);
        assert!(text.starts_with(r#"{"rec":"epoch""#), "{text}");
        assert!(text.contains(r#""epoch":"stamp-1""#), "{text}");
        assert!(text.contains(r#""build":"0.1.0+abc""#), "{text}");
        assert!(text.contains(r#""rec":"accepted","seq":1,"apply_after_tick":100"#), "{text}");
        assert!(text.contains(r#""kind":"feed""#), "{text}");

        let j = Journal::open(&dir, "stamp-2", "0.1.0+abc").unwrap();
        assert_eq!(j.accepted_records().len(), 1, "the earlier run's record survives");
        assert_eq!(j.truncated_bytes(), 0);
        assert_eq!(contents(&dir).lines().count(), 3, "epoch, accepted, epoch");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_torn_final_line_is_truncated_back_to_the_verified_prefix() {
        let dir = scratch("torn");
        {
            let mut j = Journal::open(&dir, "stamp-1", "b").unwrap();
            j.append_accepted(&[command(1, 100, CareKind::Feed), command(2, 100, CareKind::Rain)])
                .unwrap();
        }
        let intact = contents(&dir);
        let verified_prefix = intact.clone();
        // A crash mid-append: half a record, no newline.
        std::fs::write(
            dir.join(JOURNAL_NAME),
            format!("{intact}{{\"rec\":\"accepted\",\"seq\":3,\"apply"),
        )
        .unwrap();

        let j = Journal::open(&dir, "stamp-2", "b").unwrap();
        assert!(j.truncated_bytes() > 0, "the torn tail must be reported");
        assert_eq!(j.accepted_records().len(), 2, "seq 3 was never a record");
        let after = contents(&dir);
        assert!(
            after.starts_with(&verified_prefix),
            "the verified prefix must be preserved byte for byte:\n{after}"
        );
        // And the new epoch record went after the prefix, not after the torn tail.
        assert!(!after.contains("\"apply\n"), "{after}");
        assert_eq!(after.lines().count(), 4, "epoch, two accepted, epoch: {after}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_invalid_interior_line_refuses_to_start() {
        let dir = scratch("interior");
        {
            let mut j = Journal::open(&dir, "s", "b").unwrap();
            j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap();
        }
        let mut lines: Vec<String> = contents(&dir).lines().map(str::to_string).collect();
        lines.insert(1, "{\"rec\":\"accepted\",\"seq\":".to_string());
        std::fs::write(dir.join(JOURNAL_NAME), format!("{}\n", lines.join("\n"))).unwrap();

        let err = Journal::open(&dir, "s2", "b").unwrap_err();
        let text = format!("{err:#}");
        assert!(text.contains("line 2"), "{text}");
        assert!(text.contains(JOURNAL_NAME), "the refusal names the file: {text}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unknown_record_kind_in_the_middle_is_also_a_refusal() {
        let dir = scratch("unknown-kind");
        {
            let mut j = Journal::open(&dir, "s", "b").unwrap();
            j.append_accepted(&[command(1, 10, CareKind::Clean)]).unwrap();
            j.append_outcomes(&[OutcomeRecord {
                seq: 1,
                tick: 10,
                outcome: "applied",
                reason: String::new(),
                applied: serde_json::json!({ "cells": 5 }),
            }])
            .unwrap();
        }
        let mut lines: Vec<String> = contents(&dir).lines().map(str::to_string).collect();
        lines.insert(2, r#"{"rec":"nonsense","seq":9}"#.to_string());
        std::fs::write(dir.join(JOURNAL_NAME), format!("{}\n", lines.join("\n"))).unwrap();
        assert!(Journal::open(&dir, "s2", "b").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_replay_schedule_is_validated_whole_before_a_single_tick_is_stepped() {
        let dir = scratch("replay");
        let mut j = Journal::open(&dir, "s", "b").unwrap();
        j.append_accepted(&[
            command(1, 100, CareKind::Feed),
            command(2, 100, CareKind::Clean),
            command(3, 140, CareKind::Rain),
        ])
        .unwrap();

        // Nothing admitted yet, snapshot at 100.
        let plan = j.replay_plan(0, 100).unwrap();
        assert_eq!(
            plan.iter().map(|c| (c.seq, c.apply_after_tick)).collect::<Vec<_>>(),
            vec![(1, 100), (2, 100), (3, 140)]
        );
        assert_eq!(j.max_seq(), Some(3), "new commands allocate after the journal maximum");

        // A snapshot that already admitted the first two leaves only the third.
        let plan = j.replay_plan(2, 100).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].seq, 3);

        // A snapshot past a pending boundary is an inconsistent recovery state.
        let err = j.replay_plan(0, 120).unwrap_err().to_string();
        assert!(err.contains("inconsistent recovery state"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_hole_or_a_backwards_boundary_in_the_schedule_refuses_to_start() {
        let dir = scratch("replay-holes");
        let mut j = Journal::open(&dir, "s", "b").unwrap();
        // seq 1 then seq 3: the reserved seq 2 never reached the file.
        j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap();
        j.append_accepted(&[command(3, 140, CareKind::Rain)]).unwrap();
        let err = j.replay_plan(0, 100).unwrap_err().to_string();
        assert!(err.contains("not contiguous"), "{err}");
        assert!(err.contains("recovery error"), "{err}");
        drop(j);

        let dir2 = scratch("replay-backwards");
        let mut j = Journal::open(&dir2, "s", "b").unwrap();
        j.append_accepted(&[command(1, 140, CareKind::Feed), command(2, 100, CareKind::Clean)])
            .unwrap();
        let err = j.replay_plan(0, 100).unwrap_err().to_string();
        assert!(err.contains("must not go backwards"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&dir2).unwrap();
    }

    /// The fault the follow-up review named: an append that stops inside a JSON record.
    /// The partial bytes are durable; nothing may be appended after them in this process;
    /// and the next open truncates exactly that suffix and keeps every complete record.
    #[test]
    fn a_short_write_inside_a_record_survives_as_a_torn_tail_and_nothing_else() {
        let dir = scratch("short-mid-json");
        let hooks = JournalHooks::default();
        {
            let mut j = Journal::open_with_hooks(&dir, "s", "b", hooks.clone()).unwrap();
            j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap();
            hooks.short_write(40);
            let err = j.append_accepted(&[command(2, 100, CareKind::Rain)]).unwrap_err();
            assert!(err.to_string().contains("short journal write"), "{err}");
        }
        let raw = std::fs::read_to_string(dir.join(JOURNAL_NAME)).unwrap();
        assert!(!raw.ends_with('\n'), "the injected write stopped mid-record: {raw:?}");

        let j = Journal::open(&dir, "s2", "b").unwrap();
        assert_eq!(j.truncated_bytes(), 40, "exactly the torn suffix is dropped");
        assert_eq!(j.accepted_records().len(), 1, "only the complete record survives");
        assert_eq!(j.accepted_records()[0].seq, 1);
        assert_eq!(j.max_seq(), Some(1), "seq 2 was reserved but never applied, so it is free");
        // The surviving schedule is exactly the one complete record.
        let plan = j.replay_plan(0, 100).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].seq, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The other fault: a batch of two whose first record landed whole and whose second
    /// never reached the file at all.
    #[test]
    fn a_short_write_between_two_batch_records_keeps_only_the_first() {
        let dir = scratch("short-between");
        let hooks = JournalHooks::default();
        let batch = [command(1, 100, CareKind::Feed), command(2, 100, CareKind::Clean)];
        let first_len = accepted_line(&batch[0]).len() + 1;
        {
            let mut j = Journal::open_with_hooks(&dir, "s", "b", hooks.clone()).unwrap();
            hooks.short_write(first_len as u64);
            assert!(j.append_accepted(&batch).is_err());
        }
        let raw = std::fs::read_to_string(dir.join(JOURNAL_NAME)).unwrap();
        assert!(raw.ends_with('\n'), "the write stopped exactly on a record boundary");

        let j = Journal::open(&dir, "s2", "b").unwrap();
        assert_eq!(j.truncated_bytes(), 0, "there was no torn suffix to drop");
        assert_eq!(j.accepted_records().len(), 1);
        assert_eq!(j.accepted_records()[0].seq, 1);
        // seq 2 was reserved and never written: it is simply not in the schedule, and the
        // restarted process allocates after 1 rather than consuming it.
        let plan = j.replay_plan(0, 100).unwrap();
        assert_eq!(plan.iter().map(|c| c.seq).collect::<Vec<_>>(), vec![1]);
        assert_eq!(j.max_seq(), Some(1));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_bound_reserves_room_for_the_outcomes_already_accepted_commands_owe() {
        let dir = scratch("bound");
        let mut j = Journal::open(&dir, "s", "b").unwrap();
        let status = j.status();
        assert_eq!(status.limit(), JOURNAL_LIMIT);
        assert!(!status.would_overflow(1), "an empty journal has room");
        assert_eq!(status.outstanding(), 0);

        j.append_accepted(&[command(1, 10, CareKind::Feed)]).unwrap();
        assert_eq!(status.outstanding(), 1, "an accepted record owes an outcome");
        assert!(status.bytes() > 0);
        j.append_outcomes(&[OutcomeRecord {
            seq: 1,
            tick: 10,
            outcome: "applied",
            reason: String::new(),
            applied: serde_json::json!({}),
        }])
        .unwrap();
        assert_eq!(status.outstanding(), 0, "the debt is settled");

        // Right at the bound, accepting more is refused while the file is still writable.
        let debt = 300u64;
        status.outstanding.store(debt, Ordering::Relaxed);
        status.bytes.store(JOURNAL_LIMIT - debt * OUTCOME_RESERVE, Ordering::Relaxed);
        assert!(status.would_overflow(1), "the reserve for outstanding outcomes must bind");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_injected_failure_after_the_sync_still_leaves_the_record_on_disk() {
        let dir = scratch("uncertain");
        let hooks = JournalHooks::default();
        hooks.fail_after_sync(1);
        {
            let mut j = Journal::open_with_hooks(&dir, "s", "b", hooks.clone()).unwrap();
            let err = j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap_err();
            assert!(err.to_string().contains("durable"), "{err}");
            assert_eq!(j.status().outstanding(), 1, "an ambiguous write still owes an outcome");
        }
        // The whole point: reopening finds the record the writer reported a failure for.
        let j = Journal::open(&dir, "s2", "b").unwrap();
        assert_eq!(j.accepted_records().len(), 1);
        assert_eq!(j.accepted_records()[0].seq, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_injected_failure_before_the_write_leaves_nothing_behind() {
        let dir = scratch("clean-failure");
        let hooks = JournalHooks::default();
        hooks.fail_before_write(1);
        {
            let mut j = Journal::open_with_hooks(&dir, "s", "b", hooks).unwrap();
            assert!(j.append_accepted(&[command(1, 100, CareKind::Rain)]).is_err());
        }
        let j = Journal::open(&dir, "s2", "b").unwrap();
        assert!(j.accepted_records().is_empty(), "nothing was written");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Root's correction: only an *unterminated* suffix is an interrupted append. Three
    /// final lines, three different verdicts.
    #[test]
    fn only_an_unterminated_suffix_is_repaired_and_complete_records_are_preserved() {
        let base = |name: &str| {
            let dir = scratch(name);
            {
                let mut j = Journal::open(&dir, "s", "b").unwrap();
                j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap();
            }
            dir
        };

        // (a) Unterminated partial JSON: an interrupted append. Repaired, recovery proceeds.
        let dir = base("final-unterminated");
        let intact = contents(&dir);
        let mut f = std::fs::OpenOptions::new().append(true).open(dir.join(JOURNAL_NAME)).unwrap();
        let suffix = br#"{"rec":"accepted","seq":2,"app"#;
        f.write_all(suffix).unwrap();
        drop(f);
        let j = Journal::open(&dir, "s2", "b").unwrap();
        assert_eq!(j.truncated_bytes(), suffix.len() as u64);
        assert_eq!(j.accepted_records().len(), 1);
        assert!(contents(&dir).starts_with(&intact));
        std::fs::remove_dir_all(&dir).unwrap();

        // (b) Newline-terminated invalid JSON: a complete record that is not an interrupted
        // write. Refused, and the bytes are still there afterwards.
        let dir = base("final-invalid-json");
        let before = contents(&dir);
        let damaged = format!("{before}{{\"rec\":\"accepted\",\"seq\":\n");
        std::fs::write(dir.join(JOURNAL_NAME), &damaged).unwrap();
        let err = format!("{:#}", Journal::open(&dir, "s2", "b").unwrap_err());
        assert!(err.contains("not an interrupted write"), "{err}");
        assert_eq!(contents(&dir), damaged, "the evidence must be preserved, not repaired");
        std::fs::remove_dir_all(&dir).unwrap();

        // (c) Valid JSON, unknown record type, last line. Could be a future version's
        // record; refused rather than silently dropped.
        let dir = base("final-unknown-kind");
        let before = contents(&dir);
        let damaged = format!("{before}{{\"rec\":\"scheduled\",\"seq\":2}}\n");
        std::fs::write(dir.join(JOURNAL_NAME), &damaged).unwrap();
        let err = format!("{:#}", Journal::open(&dir, "s2", "b").unwrap_err());
        assert!(err.contains("unknown record kind"), "{err}");
        assert_eq!(contents(&dir), damaged);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A journal already at its bound still opens, still replays, and does not grow — not
    /// even by an epoch record. Autonomous life is unaffected; only new care is refused.
    #[test]
    fn a_journal_at_capacity_reopens_without_growing_and_still_replays() {
        let dir = scratch("at-capacity");
        {
            let mut j = Journal::open(&dir, "s", "b").unwrap();
            j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap();
        }
        // Pad to exactly the bound with a comment-free filler line that still parses.
        let text = contents(&dir);
        let filler_line = format!("{}\n", outcome_line(&OutcomeRecord {
            seq: 1,
            tick: 100,
            outcome: "applied",
            reason: "x".repeat(200),
            applied: serde_json::json!({}),
        }));
        let mut padded = text.clone();
        while (padded.len() + filler_line.len()) as u64 <= JOURNAL_LIMIT {
            padded.push_str(&filler_line);
        }
        padded.push_str(&"\n".repeat(JOURNAL_LIMIT as usize - padded.len()));
        assert_eq!(padded.len() as u64, JOURNAL_LIMIT);
        std::fs::write(dir.join(JOURNAL_NAME), &padded).unwrap();

        let mut j = Journal::open(&dir, "s2", "b").unwrap();
        assert!(!j.care_available(), "a full journal accepts no new care");
        assert_eq!(
            std::fs::metadata(dir.join(JOURNAL_NAME)).unwrap().len(),
            JOURNAL_LIMIT,
            "not one byte was appended, epoch record included"
        );
        // History is still there and still replays.
        assert_eq!(j.accepted_records().len(), 1);
        let plan = j.replay_plan(0, 100).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].seq, 1);
        // And a new command is refused as *full*, never as uncertain: nothing was attempted,
        // so the runner keeps stepping.
        let err = j.append_accepted(&[command(2, 100, CareKind::Rain)]).unwrap_err();
        assert!(err.is_full(), "{err:?}");
        assert!(!matches!(err, JournalError::Uncertain(_)));
        assert_eq!(
            std::fs::metadata(dir.join(JOURNAL_NAME)).unwrap().len(),
            JOURNAL_LIMIT,
            "a refused command writes nothing"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_larger_than_the_bound_is_rejected_without_reading_all_of_it() {
        let dir = scratch("oversized");
        // Ten times the bound. The open must refuse on the strength of a bounded read.
        let mut file = std::fs::File::create(dir.join(JOURNAL_NAME)).unwrap();
        let chunk = vec![b'\n'; 1 << 20];
        for _ in 0..(10 * JOURNAL_LIMIT / chunk.len() as u64) {
            file.write_all(&chunk).unwrap();
        }
        drop(file);
        let err = format!("{:#}", Journal::open(&dir, "s", "b").unwrap_err());
        assert!(err.contains("larger than its"), "{err}");
        assert!(err.contains("Move it aside deliberately"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The central guard in `write_durably` refuses even a record whose caller thought it
    /// had room, so the advertised bound holds against actual byte lengths.
    #[test]
    fn the_central_write_guard_refuses_a_record_that_would_cross_the_bound() {
        let dir = scratch("central-guard");
        let mut j = Journal::open(&dir, "s", "b").unwrap();
        // Claim the file is one byte short of the bound without touching the disk.
        j.status().set_for_test(JOURNAL_LIMIT - 1, 0);
        let err = j.append_outcomes(&[OutcomeRecord {
            seq: 1,
            tick: 1,
            outcome: "applied",
            reason: String::new(),
            applied: serde_json::json!({}),
        }])
        .unwrap_err();
        assert!(err.is_full(), "{err:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Astra's final blocker: a real disk filling up is never a capacity refusal. Both
    /// injected `ENOSPC` shapes — part-way through a record, and after a complete record
    /// reached the file — are uncertain, poison the writer, and leave the bytes alone.
    #[test]
    fn enospc_once_io_has_begun_is_uncertain_and_never_a_capacity_refusal() {
        // (a) ENOSPC part-way through: a torn suffix survives, repaired at the next open.
        let dir = scratch("enospc-partial");
        let hooks = JournalHooks::default();
        {
            let mut j = Journal::open_with_hooks(&dir, "s", "b", hooks.clone()).unwrap();
            // Armed after the open, so the epoch record lands normally and the injected
            // failure is squarely on the care write.
            hooks.enospc(1);
            let err = j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap_err();
            assert!(!err.is_full(), "ENOSPC from a write in progress is not a capacity refusal");
            assert!(matches!(err, JournalError::Uncertain(_)), "{err:?}");
            assert!(j.poisoned().is_some(), "the writer must be poisoned");

            // Every later queued job is refused with the file untouched.
            let after_failure = std::fs::read(dir.join(JOURNAL_NAME)).unwrap();
            let e = j
                .append_outcomes(&[OutcomeRecord {
                    seq: 1,
                    tick: 100,
                    outcome: "applied",
                    reason: String::new(),
                    applied: serde_json::json!({}),
                }])
                .unwrap_err();
            assert!(matches!(e, JournalError::Uncertain(_)), "{e:?}");
            let e = j.append_accepted(&[command(2, 100, CareKind::Clean)]).unwrap_err();
            assert!(matches!(e, JournalError::Uncertain(_)), "{e:?}");
            assert_eq!(
                std::fs::read(dir.join(JOURNAL_NAME)).unwrap(),
                after_failure,
                "a poisoned writer must not touch the file again, in any job"
            );
        }
        let j = Journal::open(&dir, "s2", "b").unwrap();
        assert!(j.truncated_bytes() > 0, "the half record is repaired away");
        assert!(j.accepted_records().is_empty(), "no complete record survived");
        assert_eq!(j.max_seq(), None, "seq 1 was never applied, and is not consumed");
        std::fs::remove_dir_all(&dir).unwrap();

        // (b) The complete record reached the file and the failure came at the sync. The
        // record replays once at its boundary.
        let dir = scratch("enospc-complete");
        let hooks = JournalHooks::default();
        {
            let mut j = Journal::open_with_hooks(&dir, "s", "b", hooks.clone()).unwrap();
            hooks.fail_after_sync(1);
            let err = j.append_accepted(&[command(7, 100, CareKind::Rain)]).unwrap_err();
            assert!(!err.is_full(), "{err:?}");
            assert!(j.poisoned().is_some());
        }
        let j = Journal::open(&dir, "s2", "b").unwrap();
        assert_eq!(j.accepted_records().len(), 1);
        let plan = j.replay_plan(6, 100).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].seq, 7);
        assert_eq!(plan[0].apply_after_tick, 100);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The other side of the same rule, kept explicit: the *configured* limit is checked
    /// before any write is attempted, so it is typed `Full`, the file is untouched, and the
    /// writer is not poisoned — autonomous life carries on with care refused.
    #[test]
    fn the_configured_limit_is_refused_without_attempting_a_write_or_poisoning() {
        let dir = scratch("configured-limit");
        let mut j = Journal::open(&dir, "s", "b").unwrap();
        let before = std::fs::read(dir.join(JOURNAL_NAME)).unwrap();
        j.status().set_for_test(JOURNAL_LIMIT - 1, 0);

        let err = j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap_err();
        assert!(err.is_full(), "{err:?}");
        assert!(j.poisoned().is_none(), "a capacity refusal must not poison the writer");
        assert_eq!(std::fs::read(dir.join(JOURNAL_NAME)).unwrap(), before);

        // And once there is room again the same writer still works: nothing was broken.
        j.status().set_for_test(before.len() as u64, 0);
        j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap();
        assert_eq!(j.accepted_records().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Creating `care.jsonl` makes a directory entry, and syncing the file's own contents
    /// says nothing about whether that entry survives a power loss. The barrier has to be
    /// crossed at open, before any command can be accepted.
    #[test]
    fn the_journals_directory_entry_is_made_durable_before_any_command_is_accepted() {
        let dir = scratch("dir-barrier");
        let hooks = JournalHooks::default();
        assert_eq!(hooks.dir_sync_count(), 0);
        let mut j = Journal::open_with_hooks(&dir, "s", "b", hooks.clone()).unwrap();
        assert_eq!(
            hooks.dir_sync_count(),
            1,
            "the directory must be synced at open, which is before any append is possible"
        );
        j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap();
        assert_eq!(hooks.dir_sync_count(), 1, "one barrier per run is enough");
        drop(j);

        // A failure at that barrier refuses to open at all: no service is published on the
        // strength of a file whose own pathname may not survive.
        let dir2 = scratch("dir-barrier-fails");
        let hooks = JournalHooks::default();
        hooks.fail_dir_sync(1);
        let err = format!("{:#}", Journal::open_with_hooks(&dir2, "s", "b", hooks).unwrap_err());
        assert!(err.contains("no \ncommand may be accepted") || err.contains("command may be accepted"), "{err}");
        assert!(
            !dir2.join(JOURNAL_NAME).exists()
                || std::fs::read(dir2.join(JOURNAL_NAME)).unwrap().is_empty(),
            "a journal that failed its barrier must carry no records"
        );
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&dir2).unwrap();
    }

    #[test]
    fn an_empty_and_a_newline_only_journal_are_both_fine() {
        let dir = scratch("empty");
        std::fs::write(dir.join(JOURNAL_NAME), b"").unwrap();
        let j = Journal::open(&dir, "s", "b").unwrap();
        assert!(j.accepted_records().is_empty());
        assert_eq!(j.truncated_bytes(), 0);
        drop(j);
        std::fs::write(dir.join(JOURNAL_NAME), b"\n\n").unwrap();
        let j = Journal::open(&dir, "s", "b").unwrap();
        assert!(j.accepted_records().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ------------------------------------------------------------------ the dose records

    /// A standard dose is still written as the legacy `accepted` record, with **no** amount
    /// field: an older binary reads it and applies exactly the right thing. A nonstandard dose
    /// is written as `accepted_dose_v1`, which that binary refuses by name.
    #[test]
    fn a_standard_dose_stays_a_legacy_record_and_a_nonstandard_one_gets_its_own_kind() {
        let dir = scratch("dose-records");
        {
            let mut j = Journal::open(&dir, "s", "b").unwrap();
            j.append_accepted(&[
                command(1, 100, CareKind::Feed),
                dosed(2, 100, CareKind::Rain, 1500),
                dosed(3, 100, CareKind::Clean, 250),
            ])
            .unwrap();
        }
        let text = contents(&dir);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[1].contains(r#""rec":"accepted","seq":1"#), "{}", lines[1]);
        assert!(
            !lines[1].contains("dose_permille"),
            "a standard command must carry no amount at all: {}",
            lines[1]
        );
        assert!(lines[2].contains(r#""rec":"accepted_dose_v1","seq":2"#), "{}", lines[2]);
        assert!(lines[2].contains(r#""dose_permille":1500"#), "{}", lines[2]);
        assert!(lines[3].contains(r#""dose_permille":250"#), "{}", lines[3]);

        // And every one of them reads back as the amount it was written with — a mixed
        // old/new journal, which is what any upgraded world's journal actually is.
        let j = Journal::open(&dir, "s2", "b").unwrap();
        let plan = j.replay_plan(0, 100).unwrap();
        assert_eq!(
            plan.iter().map(|c| (c.seq, c.dose.permille())).collect::<Vec<_>>(),
            vec![(1, 1000), (2, 1500), (3, 250)]
        );
        assert!(plan[0].dose.is_standard(), "a legacy record means the standard dose");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Both bounds survive the round trip exactly, with no clamping anywhere in the path.
    #[test]
    fn the_documented_bounds_round_trip_through_the_journal() {
        let dir = scratch("dose-bounds");
        {
            let mut j = Journal::open(&dir, "s", "b").unwrap();
            j.append_accepted(&[
                dosed(1, 10, CareKind::Feed, CareDose::MIN_PERMILLE),
                dosed(2, 10, CareKind::Feed, CareDose::MAX_PERMILLE),
            ])
            .unwrap();
        }
        let j = Journal::open(&dir, "s2", "b").unwrap();
        assert_eq!(
            j.accepted_records().iter().map(|c| c.dose.permille()).collect::<Vec<_>>(),
            vec![CareDose::MIN_PERMILLE, CareDose::MAX_PERMILLE]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The asymmetry, case by case, and **at the final newline-terminated line** — the one
    /// position where a reader might be tempted to call a complete record a torn tail. Every
    /// one of these fails closed, and the bytes are preserved for a human to look at.
    #[test]
    fn a_malformed_or_mislabelled_dose_record_fails_closed_even_as_the_last_line() {
        let cases: [(&str, &str); 8] = [
            // A legacy record carrying an amount: somebody wrote a dose under a name older
            // readers ignore. Refused even at the standard value — the intent is what is wrong.
            (
                "legacy-with-standard-dose",
                r#"{"rec":"accepted","seq":2,"apply_after_tick":100,"client":"c","request":2,"kind":"feed","target":{"face":1,"u":12,"v":34},"dose_permille":1000}"#,
            ),
            (
                "legacy-with-nonstandard-dose",
                r#"{"rec":"accepted","seq":2,"apply_after_tick":100,"client":"c","request":2,"kind":"feed","target":{"face":1,"u":12,"v":34},"dose_permille":1500}"#,
            ),
            // The new record without its amount: missing never defaults to standard.
            (
                "dosed-without-amount",
                r#"{"rec":"accepted_dose_v1","seq":2,"apply_after_tick":100,"client":"c","request":2,"kind":"feed","target":{"face":1,"u":12,"v":34}}"#,
            ),
            (
                "dosed-null-amount",
                r#"{"rec":"accepted_dose_v1","seq":2,"apply_after_tick":100,"client":"c","request":2,"kind":"feed","target":{"face":1,"u":12,"v":34},"dose_permille":null}"#,
            ),
            (
                "dosed-fractional-amount",
                r#"{"rec":"accepted_dose_v1","seq":2,"apply_after_tick":100,"client":"c","request":2,"kind":"feed","target":{"face":1,"u":12,"v":34},"dose_permille":1500.5}"#,
            ),
            (
                "dosed-negative-amount",
                r#"{"rec":"accepted_dose_v1","seq":2,"apply_after_tick":100,"client":"c","request":2,"kind":"feed","target":{"face":1,"u":12,"v":34},"dose_permille":-500}"#,
            ),
            (
                "dosed-out-of-range-amount",
                r#"{"rec":"accepted_dose_v1","seq":2,"apply_after_tick":100,"client":"c","request":2,"kind":"feed","target":{"face":1,"u":12,"v":34},"dose_permille":5000}"#,
            ),
            // A version this build does not know. Fails closed exactly like any other
            // unknown record kind, rather than being read as its nearest relative.
            (
                "future-version",
                r#"{"rec":"accepted_dose_v2","seq":2,"apply_after_tick":100,"client":"c","request":2,"kind":"feed","target":{"face":1,"u":12,"v":34},"dose_permille":1500}"#,
            ),
        ];
        for (name, line) in cases {
            let dir = scratch(name);
            {
                let mut j = Journal::open(&dir, "s", "b").unwrap();
                j.append_accepted(&[command(1, 100, CareKind::Feed)]).unwrap();
            }
            let damaged = format!("{}{line}\n", contents(&dir));
            std::fs::write(dir.join(JOURNAL_NAME), &damaged).unwrap();
            let err = format!("{:#}", Journal::open(&dir, "s2", "b").unwrap_err());
            assert!(err.contains("cannot be trusted"), "{name}: {err}");
            assert_eq!(
                contents(&dir),
                damaged,
                "{name}: a complete record is evidence, not a torn tail to repair"
            );
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// The one thing that *is* still repairable: an unterminated suffix, even when it is a
    /// half-written dosed record. Nothing here changes what "torn" means.
    #[test]
    fn a_half_written_dosed_record_is_still_just_a_torn_tail() {
        let dir = scratch("dose-torn");
        {
            let mut j = Journal::open(&dir, "s", "b").unwrap();
            j.append_accepted(&[dosed(1, 100, CareKind::Rain, 1500)]).unwrap();
        }
        let intact = contents(&dir);
        let suffix = br#"{"rec":"accepted_dose_v1","seq":2,"apply_after_tick":100,"dose_per"#;
        let mut f = std::fs::OpenOptions::new().append(true).open(dir.join(JOURNAL_NAME)).unwrap();
        f.write_all(suffix).unwrap();
        drop(f);

        let j = Journal::open(&dir, "s2", "b").unwrap();
        assert_eq!(j.truncated_bytes(), suffix.len() as u64);
        assert_eq!(j.accepted_records().len(), 1);
        assert_eq!(j.accepted_records()[0].dose.permille(), 1500, "the complete record is intact");
        assert!(contents(&dir).starts_with(&intact));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// What an **older** binary does with a dosed record, established from that binary's own
    /// reader rather than asserted: the pre-dose build knew exactly three record kinds, and
    /// `accepted_dose_v1` is not one of them, so it takes the `unknown record kind` path — a
    /// refusal to start, not a silently ignored amount. (The executable evidence is in
    /// `design/7_Research/adjustable-care-dose-progress-2026-09-13.md`; this is the in-repo
    /// guard that the discriminator stays outside the old set.)
    #[test]
    fn the_new_discriminator_is_outside_the_pre_dose_readers_vocabulary() {
        const PRE_DOSE_RECORD_KINDS: [&str; 3] = ["epoch", "accepted", "outcome"];
        let line = accepted_line(&dosed(1, 100, CareKind::Rain, 1500));
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        let rec = value["rec"].as_str().unwrap();
        assert_eq!(rec, "accepted_dose_v1");
        assert!(
            !PRE_DOSE_RECORD_KINDS.contains(&rec),
            "a dosed command must not reuse a record kind the old reader accepts"
        );
        // And the standard one deliberately *is* in that vocabulary, so an old binary can go
        // on reading a standard-only history correctly.
        let legacy: serde_json::Value =
            serde_json::from_str(&accepted_line(&command(1, 100, CareKind::Feed))).unwrap();
        assert!(PRE_DOSE_RECORD_KINDS.contains(&legacy["rec"].as_str().unwrap()));
        assert!(legacy.get("dose_permille").is_none());
    }
}
