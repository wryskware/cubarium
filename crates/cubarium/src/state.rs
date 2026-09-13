//! The state directory: snapshot discovery, loading, the checkpoint worker, and pruning.
//!
//! Nothing here knows about the world's contents. It moves opaque encoded bytes onto
//! disk durably (temp file, `sync_all`, rename, directory fsync) and keeps the newest
//! [`SNAPSHOT_KEEP`] files. Disk errors are logged with the path and the reason; the
//! world never stops for them.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use cubarium_core::world::WorldState;
use cubarium_core::{decode_snapshot, SnapshotError};

/// Snapshot files kept in the state directory. The newest is never removed.
pub const SNAPSHOT_KEEP: usize = 8;
/// How long shutdown waits for the checkpoint worker to drain.
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

const PREFIX: &str = "world-";
const SUFFIX: &str = ".cubw";
/// Temp files [`write_snapshot`] renames from; an interrupted write leaves one behind.
const TMP_PREFIX: &str = "tmp-";
/// The care journal, named here because the occupancy guard has to recognize it.
pub const JOURNAL_NAME: &str = "care.jsonl";
/// The advisory lock file. Opened and locked, never unlinked and never replaced.
pub const LOCK_NAME: &str = ".lock";

/// The build identity written into every snapshot header: the crate version plus the
/// git short hash captured by `build.rs` (`unknown` when git was unavailable).
pub fn build_id() -> String {
    format!("{}+{}", env!("CARGO_PKG_VERSION"), env!("CUBARIUM_GIT_HASH"))
}

/// `world-<tick>.cubw` inside `dir`.
pub fn snapshot_path(dir: &Path, tick: u64) -> PathBuf {
    dir.join(format!("{PREFIX}{tick}{SUFFIX}"))
}

/// The tick encoded in a snapshot file name, or `None` when the name is not one.
/// Names that do not parse are ignored entirely rather than guessed at.
pub fn parse_tick(name: &str) -> Option<u64> {
    name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?.parse().ok()
}

/// Every `world-<tick>.cubw` in `dir`, newest tick first. Unreadable directories and
/// unparseable names yield an empty list rather than an error: a missing state directory
/// simply has no snapshots.
pub fn list_snapshots(dir: &Path) -> Vec<(u64, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(u64, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let tick = parse_tick(path.file_name()?.to_str()?)?;
            path.is_file().then_some((tick, path))
        })
        .collect();
    // Descending tick, then descending path so the order is total and stable.
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    found
}

// --- Exclusive ownership of the state directory ------------------------------------

/// An exclusive, non-blocking advisory lock (`flock(2)`) on `<state>/.lock`, held by an
/// open handle for the whole run.
///
/// Why an OS lock and not a pid file: a `LOCK` file holding a pid that the launcher
/// checks against `/proc` has a check-then-create race — two launchers can both see an
/// absent or dead owner and both start writing — and pid reuse makes liveness no proof
/// of identity either. `flock` is acquired atomically by the kernel and released by the
/// kernel when the owning process exits, however it exits, so a leftover `.lock` file is
/// harmless and never needs deleting.
///
/// The file's *content* is diagnostic only (pid and start stamp): nothing reads it to
/// decide ownership. The inode is never unlinked or replaced, because another process may
/// be holding a lock on it right now and replacing the inode would silently hand the
/// directory to two owners.
#[derive(Debug)]
pub struct StateLock {
    path: PathBuf,
    /// Dropped last: closing this descriptor is what releases the lock.
    file: File,
}

impl StateLock {
    /// Lock `dir`, or return an error naming the directory. The directory must exist.
    pub fn acquire(dir: &Path) -> Result<StateLock> {
        let path = dir.join(LOCK_NAME);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("opening the state lock {}", path.display()))?;
        match flock_exclusive_nonblocking(&file) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                anyhow::bail!(
                    "the state directory {} is already in use by another cubarium process \
                     (it holds the lock {}); stop that process or choose a different --state \
                     directory",
                    dir.display(),
                    path.display()
                );
            }
            Err(e) => {
                return Err(anyhow::Error::new(e))
                    .with_context(|| format!("locking the state directory {}", dir.display()));
            }
        }
        let mut lock = StateLock { path, file };
        lock.write_diagnostics();
        Ok(lock)
    }

    /// The lock file's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Pid and start stamp, for a human reading the directory. Best effort: a failure to
    /// write the note does not cost the run its ownership, which the kernel already granted.
    fn write_diagnostics(&mut self) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let note = format!("pid {} started {stamp}\n", std::process::id());
        let _ = self.file.set_len(0);
        let _ = self.file.write_all(note.as_bytes());
        let _ = self.file.flush();
    }
}

/// `flock(fd, LOCK_EX | LOCK_NB)`. Contention arrives as [`io::ErrorKind::WouldBlock`].
///
/// The only `unsafe` in this crate. It is sound: `as_raw_fd` yields a descriptor the
/// `File` keeps open for the duration of the call (and for the life of the [`StateLock`]),
/// `flock` takes only that integer and a flag constant, and the return value is fully
/// described by `errno`, which is read through `io::Error::last_os_error`.
#[allow(unsafe_code)]
fn flock_exclusive_nonblocking(file: &File) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

// --- What a state directory already holds -------------------------------------------

/// The narrow "this directory already holds a world" policy the `--fresh` guard uses.
/// Deliberately narrow: snapshots, interrupted snapshot writes, and a non-empty care
/// journal. Arbitrary unrelated files (telemetry, a README, the lock itself) are not
/// occupancy — an operator may keep notes beside a world.
#[derive(Debug, Default, Clone)]
pub struct Occupancy {
    /// `world-<tick>.cubw` files, newest tick first.
    pub snapshots: Vec<(u64, PathBuf)>,
    /// `tmp-*.cubw` files left by an interrupted write.
    pub temporaries: Vec<PathBuf>,
    /// A `care.jsonl` that exists and is not empty.
    pub journal: Option<PathBuf>,
}

impl Occupancy {
    /// True when a `--fresh` launch here would mix two worlds' histories.
    pub fn is_occupied(&self) -> bool {
        !self.snapshots.is_empty() || !self.temporaries.is_empty() || self.journal.is_some()
    }

    /// What was found, for the refusal message.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some((tick, _)) = self.snapshots.first() {
            parts.push(format!(
                "{} snapshot{} (newest tick {tick})",
                self.snapshots.len(),
                if self.snapshots.len() == 1 { "" } else { "s" }
            ));
        }
        if !self.temporaries.is_empty() {
            parts.push(format!("{} interrupted snapshot write(s)", self.temporaries.len()));
        }
        if self.journal.is_some() {
            parts.push(format!("a non-empty {JOURNAL_NAME}"));
        }
        parts.join(", ")
    }
}

/// Enumerate `dir` for the `--fresh` guard. Unlike [`list_snapshots`], every I/O error is
/// propagated: a protective check that reads an unreadable directory as empty would wave
/// the very launch it exists to refuse straight through. A directory that does not exist
/// yet is genuinely empty, and only that is reported as unoccupied.
pub fn occupancy(dir: &Path) -> io::Result<Occupancy> {
    let mut found = Occupancy::default();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(found),
        Err(e) => return Err(io::Error::new(e.kind(), format!("{}: {e}", dir.display()))),
    };
    for entry in entries {
        let entry = entry.map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", dir.display())))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let file_type = entry
            .file_type()
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
        if !file_type.is_file() {
            continue;
        }
        if let Some(tick) = parse_tick(name) {
            found.snapshots.push((tick, path));
        } else if name.starts_with(TMP_PREFIX) && name.ends_with(SUFFIX) {
            found.temporaries.push(path);
        } else if name == JOURNAL_NAME {
            let len = entry
                .metadata()
                .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?
                .len();
            if len > 0 {
                found.journal = Some(path);
            }
        }
    }
    found.snapshots.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    found.temporaries.sort();
    Ok(found)
}

/// A snapshot that decoded, with where it came from.
#[derive(Debug)]
pub struct Loaded {
    pub path: PathBuf,
    pub tick: u64,
    pub state: WorldState,
}

/// Why one candidate snapshot was skipped.
#[derive(Debug)]
pub enum LoadFailure {
    Io(String),
    Snapshot(SnapshotError),
}

impl std::fmt::Display for LoadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadFailure::Io(e) => write!(f, "{e}"),
            LoadFailure::Snapshot(e) => write!(f, "{e}"),
        }
    }
}

/// Try every snapshot newest-first and return the first that decodes and validates.
/// Every failure is reported to `report` with its path so the operator learns why an
/// older world was resumed.
pub fn load_newest(
    dir: &Path,
    report: &mut dyn FnMut(&Path, &LoadFailure),
) -> Option<Loaded> {
    for (tick, path) in list_snapshots(dir) {
        match fs::read(&path) {
            Err(e) => report(&path, &LoadFailure::Io(e.to_string())),
            Ok(bytes) => match decode_snapshot(&bytes) {
                Err(e) => report(&path, &LoadFailure::Snapshot(e)),
                Ok((_meta, state)) => return Some(Loaded { path, tick, state }),
            },
        }
    }
    None
}

/// Remove all but the newest `keep` snapshots. The newest is never a candidate: `keep`
/// is clamped to at least one.
pub fn prune_snapshots(dir: &Path, keep: usize) -> Vec<(PathBuf, std::io::Error)> {
    let keep = keep.max(1);
    let mut errors = Vec::new();
    for (_, path) in list_snapshots(dir).into_iter().skip(keep) {
        if let Err(e) = fs::remove_file(&path) {
            errors.push((path, e));
        }
    }
    errors
}

/// Write one snapshot durably: temp file, `sync_all`, rename, directory fsync.
///
/// The rename is atomic within the directory, so a reader never sees a half-written
/// `world-<tick>.cubw`; the directory fsync makes the rename itself durable.
pub fn write_snapshot(dir: &Path, tick: u64, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let tmp = dir.join(format!("tmp-{tick}{SUFFIX}"));
    let final_path = snapshot_path(dir, tick);
    {
        let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, &final_path)?;
    // Fsync the directory so the rename survives a crash, not just the file contents.
    File::open(dir)?.sync_all()?;
    Ok(final_path)
}

// --- The checkpoint worker ---------------------------------------------------------

/// The newest-pending mailbox shared with the worker. Capacity one: a checkpoint queued
/// while another is still pending replaces it, because an older snapshot of the same
/// world is strictly less useful than a newer one.
#[derive(Default)]
struct Mailbox {
    pending: Option<(u64, Vec<u8>)>,
    stop: bool,
    finished: bool,
    /// Checkpoints replaced before the worker could write them.
    dropped: u64,
}

/// A worker thread that writes snapshots off the simulation loop.
pub struct Checkpointer {
    dir: PathBuf,
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    handle: Option<JoinHandle<()>>,
    queued: u64,
    warned_drop: bool,
}

impl Checkpointer {
    /// Spawn the worker for `dir`. The directory must already exist.
    pub fn spawn(dir: impl Into<PathBuf>) -> Checkpointer {
        Checkpointer::spawn_holding(dir, None)
    }

    /// [`Checkpointer::spawn`], with the worker thread holding a share of the state lock.
    ///
    /// This is what makes the one-writer guarantee true rather than merely intended.
    /// [`Checkpointer::shutdown`] *detaches* a worker still writing after
    /// [`SHUTDOWN_TIMEOUT`], and a library caller can then return and start another owner
    /// while that thread is still renaming a snapshot into the directory. Holding a clone of
    /// the same `Arc<StateLock>` — the same open file description, never a second `open` or
    /// a second `flock` — keeps the kernel's lock held for exactly as long as a writer
    /// actually exists, timeout or no timeout.
    pub fn spawn_holding(dir: impl Into<PathBuf>, lock: Option<Arc<StateLock>>) -> Checkpointer {
        let dir = dir.into();
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let worker_dir = dir.clone();
        let worker_shared = Arc::clone(&shared);
        let handle = std::thread::Builder::new()
            .name("cubarium-checkpoint".into())
            .spawn(move || {
                // Moved into the thread and dropped when it returns, however it returns.
                let _ownership = lock;
                checkpoint_loop(&worker_dir, &worker_shared);
            })
            .expect("spawning the checkpoint worker");
        Checkpointer { dir, shared, handle: Some(handle), queued: 0, warned_drop: false }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Number of checkpoints handed to the worker (including any it later replaced).
    pub fn queued(&self) -> u64 {
        self.queued
    }

    /// Checkpoints the worker never wrote because a newer one arrived first.
    pub fn dropped(&self) -> u64 {
        let (lock, _) = &*self.shared;
        lock.lock().map(|m| m.dropped).unwrap_or(0)
    }

    /// Hand the worker a snapshot. Never blocks on the disk; replaces a still-pending
    /// older checkpoint and logs that at most once per run.
    pub fn queue(&mut self, tick: u64, bytes: Vec<u8>) {
        let (lock, cv) = &*self.shared;
        let mut mailbox = lock.lock().expect("checkpoint mailbox");
        let replaced = mailbox.pending.replace((tick, bytes));
        if let Some((old, _)) = replaced {
            mailbox.dropped += 1;
            if !self.warned_drop {
                self.warned_drop = true;
                eprintln!(
                    "cubarium: checkpoint worker is behind; dropping the pending snapshot at tick {old} (logged once)"
                );
            }
        }
        cv.notify_all();
        drop(mailbox);
        self.queued += 1;
    }

    /// Queue a final snapshot, then stop and join the worker, waiting at most
    /// [`SHUTDOWN_TIMEOUT`]. A worker still writing after that is left detached and
    /// reported rather than blocking the exit forever.
    pub fn shutdown(mut self, final_snapshot: Option<(u64, Vec<u8>)>) {
        if let Some((tick, bytes)) = final_snapshot {
            self.queue(tick, bytes);
        }
        let (lock, cv) = &*self.shared;
        {
            let mut mailbox = lock.lock().expect("checkpoint mailbox");
            mailbox.stop = true;
            cv.notify_all();
        }
        let deadline_reached = {
            let mailbox = lock.lock().expect("checkpoint mailbox");
            let (_mailbox, timeout) = cv
                .wait_timeout_while(mailbox, SHUTDOWN_TIMEOUT, |m| !m.finished)
                .expect("checkpoint mailbox");
            timeout.timed_out()
        };
        if deadline_reached {
            eprintln!(
                "cubarium: checkpoint worker did not finish within {} s; leaving it running",
                SHUTDOWN_TIMEOUT.as_secs()
            );
            // Drop the handle without joining: the process is exiting anyway, and the
            // worker's own write is atomic.
            self.handle = None;
            return;
        }
        if let Some(handle) = self.handle.take()
            && handle.join().is_err()
        {
            eprintln!("cubarium: the checkpoint worker panicked");
        }
    }
}

impl Drop for Checkpointer {
    /// A checkpointer dropped without [`Checkpointer::shutdown`] (an error path) still
    /// releases its worker rather than leaving the process with a parked thread.
    fn drop(&mut self) {
        let (lock, cv) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.stop = true;
            cv.notify_all();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn checkpoint_loop(dir: &Path, shared: &Arc<(Mutex<Mailbox>, Condvar)>) {
    let (lock, cv) = &**shared;
    loop {
        let item = {
            let mut mailbox = lock.lock().expect("checkpoint mailbox");
            while mailbox.pending.is_none() && !mailbox.stop {
                mailbox = cv.wait(mailbox).expect("checkpoint mailbox");
            }
            match mailbox.pending.take() {
                Some(item) => item,
                // Nothing pending and asked to stop: announce the exit and leave.
                None => {
                    mailbox.finished = true;
                    cv.notify_all();
                    return;
                }
            }
        };
        let (tick, bytes) = item;
        match write_snapshot(dir, tick, &bytes) {
            Ok(_) => {
                for (path, err) in prune_snapshots(dir, SNAPSHOT_KEEP) {
                    eprintln!("cubarium: cannot prune {}: {err}", path.display());
                }
            }
            Err(err) => eprintln!(
                "cubarium: cannot write {}: {err}",
                snapshot_path(dir, tick).display()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cubarium-state-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn snapshot_names_round_trip_and_foreign_names_are_ignored() {
        assert_eq!(parse_tick("world-0.cubw"), Some(0));
        assert_eq!(parse_tick("world-4800.cubw"), Some(4800));
        assert_eq!(parse_tick("tmp-4800.cubw"), None);
        assert_eq!(parse_tick("world-.cubw"), None);
        assert_eq!(parse_tick("world-abc.cubw"), None);
        assert_eq!(parse_tick("world-12.cubw.bak"), None);
        assert_eq!(parse_tick("telemetry.jsonl"), None);
        let dir = Path::new("/state");
        assert_eq!(snapshot_path(dir, 77), PathBuf::from("/state/world-77.cubw"));
    }

    #[test]
    fn listing_is_newest_tick_first_and_skips_unparseable_names() {
        let dir = temp_dir("listing");
        for tick in [5u64, 1200, 77, 0] {
            fs::write(snapshot_path(&dir, tick), b"x").unwrap();
        }
        fs::write(dir.join("telemetry.jsonl"), b"x").unwrap();
        fs::write(dir.join("tmp-99.cubw"), b"x").unwrap();
        fs::write(dir.join("world-nope.cubw"), b"x").unwrap();
        let ticks: Vec<u64> = list_snapshots(&dir).into_iter().map(|(t, _)| t).collect();
        assert_eq!(ticks, vec![1200, 77, 5, 0]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_directory_simply_has_no_snapshots() {
        assert!(list_snapshots(Path::new("/definitely/not/here")).is_empty());
    }

    #[test]
    fn pruning_keeps_the_newest_and_never_the_last_one() {
        let dir = temp_dir("prune");
        for i in 0..12u64 {
            fs::write(snapshot_path(&dir, i * 100), b"x").unwrap();
        }
        assert!(prune_snapshots(&dir, SNAPSHOT_KEEP).is_empty());
        let ticks: Vec<u64> = list_snapshots(&dir).into_iter().map(|(t, _)| t).collect();
        assert_eq!(ticks, vec![1100, 1000, 900, 800, 700, 600, 500, 400]);

        // `keep = 0` still leaves the newest in place.
        prune_snapshots(&dir, 0);
        let ticks: Vec<u64> = list_snapshots(&dir).into_iter().map(|(t, _)| t).collect();
        assert_eq!(ticks, vec![1100]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn writing_leaves_no_temp_file_behind() {
        let dir = temp_dir("write");
        let path = write_snapshot(&dir, 42, b"payload").unwrap();
        assert_eq!(path, snapshot_path(&dir, 42));
        assert_eq!(fs::read(&path).unwrap(), b"payload");
        let names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["world-42.cubw".to_string()]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_worker_writes_prunes_and_stops() {
        let dir = temp_dir("worker");
        let mut cp = Checkpointer::spawn(&dir);
        // One at a time so nothing is dropped: each write is observed before the next.
        for i in 0..12u64 {
            cp.queue(i * 10, vec![i as u8; 32]);
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while std::time::Instant::now() < deadline
                && !snapshot_path(&dir, i * 10).exists()
            {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        cp.shutdown(None);
        let ticks: Vec<u64> = list_snapshots(&dir).into_iter().map(|(t, _)| t).collect();
        assert_eq!(ticks, vec![110, 100, 90, 80, 70, 60, 50, 40]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_final_snapshot_is_written_during_shutdown() {
        let dir = temp_dir("final");
        let cp = Checkpointer::spawn(&dir);
        cp.shutdown(Some((7, b"final".to_vec())));
        assert_eq!(fs::read(snapshot_path(&dir, 7)).unwrap(), b"final");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn occupancy_sees_snapshots_temp_files_and_a_nonempty_journal_only() {
        let dir = temp_dir("occupancy");
        assert!(!occupancy(&dir).unwrap().is_occupied(), "an empty directory is not occupied");

        // Files an operator may keep beside a world are not occupancy.
        fs::write(dir.join("telemetry.jsonl"), b"{}\n").unwrap();
        fs::write(dir.join("README"), b"notes").unwrap();
        fs::write(dir.join(LOCK_NAME), b"pid 1\n").unwrap();
        fs::write(dir.join("world-oops.cubw"), b"x").unwrap();
        // An *empty* journal is not occupancy either: care that never accepted anything
        // left no history to mix.
        fs::write(dir.join(JOURNAL_NAME), b"").unwrap();
        assert!(!occupancy(&dir).unwrap().is_occupied(), "{:?}", occupancy(&dir).unwrap());

        fs::write(dir.join(JOURNAL_NAME), b"{\"rec\":\"epoch\"}\n").unwrap();
        let seen = occupancy(&dir).unwrap();
        assert!(seen.is_occupied());
        assert!(seen.journal.is_some());
        assert!(seen.describe().contains(JOURNAL_NAME), "{}", seen.describe());

        fs::write(snapshot_path(&dir, 5), b"x").unwrap();
        fs::write(snapshot_path(&dir, 446_277), b"x").unwrap();
        fs::write(dir.join("tmp-99.cubw"), b"x").unwrap();
        let seen = occupancy(&dir).unwrap();
        assert_eq!(seen.snapshots.len(), 2);
        assert_eq!(seen.snapshots[0].0, 446_277, "newest tick first");
        assert_eq!(seen.temporaries.len(), 1);
        assert!(seen.describe().contains("446277"), "{}", seen.describe());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn occupancy_propagates_a_read_error_and_reports_a_missing_directory_as_empty() {
        // `list_snapshots` treats both as empty; the protective check must not.
        assert!(!occupancy(Path::new("/definitely/not/here")).unwrap().is_occupied());
        // A path that is a file, not a directory, is an error rather than "nothing here".
        let dir = temp_dir("occupancy-error");
        let file = dir.join("not-a-directory");
        fs::write(&file, b"x").unwrap();
        let err = occupancy(&file).unwrap_err();
        assert!(err.to_string().contains("not-a-directory"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_state_lock_is_exclusive_and_reusable_without_deleting_the_file() {
        let dir = temp_dir("lock");
        let held = StateLock::acquire(&dir).expect("the first launcher owns the directory");
        assert!(held.path().exists());

        let err = StateLock::acquire(&dir).expect_err("a second launcher must be refused");
        let text = format!("{err:#}");
        assert!(text.contains(&dir.display().to_string()), "the refusal names the directory: {text}");

        // The diagnostic content is content, not the mechanism.
        let note = fs::read_to_string(held.path()).unwrap();
        assert!(note.contains(&format!("pid {}", std::process::id())), "{note}");

        drop(held);
        assert!(dir.join(LOCK_NAME).exists(), "the lock file is never unlinked");
        let again = StateLock::acquire(&dir).expect("the lock is free once the owner drops it");
        drop(again);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The one-writer guarantee has to survive a *detached* writer. `shutdown` gives up
    /// after [`SHUTDOWN_TIMEOUT`] and leaves the worker running; if ownership lived only in
    /// the runner's local, a second launcher could start while that thread was still
    /// renaming a snapshot into the directory.
    #[test]
    fn a_checkpoint_worker_keeps_the_directory_owned_after_the_runner_drops_its_lock() {
        let dir = temp_dir("lock-through-worker");
        let lock = Arc::new(StateLock::acquire(&dir).unwrap());
        let cp = Checkpointer::spawn_holding(&dir, Some(Arc::clone(&lock)));

        // The runner returns and drops its own handle. The worker thread still holds a
        // clone of the same open file description, so the kernel's lock is still held.
        drop(lock);
        assert!(
            StateLock::acquire(&dir).is_err(),
            "a second launcher must not start while a writer thread is alive"
        );

        // Only when the worker has actually stopped is the directory free — and the lock
        // file is still there, never unlinked.
        cp.shutdown(Some((1, b"final".to_vec())));
        assert!(dir.join(LOCK_NAME).exists());
        let again = StateLock::acquire(&dir).expect("free once every writer has exited");
        drop(again);
        assert!(snapshot_path(&dir, 1).exists(), "the detached writer's work still landed");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unwritable_directory_is_logged_and_not_fatal() {
        // The worker must survive a write failure and keep serving later checkpoints.
        let dir = temp_dir("unwritable");
        let missing = dir.join("gone");
        let mut cp = Checkpointer::spawn(&missing);
        cp.queue(1, b"x".to_vec());
        // Give the worker time to fail on the missing directory before it reappears.
        std::thread::sleep(Duration::from_millis(50));
        fs::create_dir_all(&missing).unwrap();
        cp.shutdown(Some((2, b"y".to_vec())));
        assert!(snapshot_path(&missing, 2).exists(), "the worker must survive a write failure");
        fs::remove_dir_all(&dir).unwrap();
    }
}
