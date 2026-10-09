//! **Saving the running world from outside the app**: `SIGUSR1` saves it and keeps
//! running; `SIGTERM` and `SIGINT` save it, then end the process as the signal would have.
//!
//! - **Where:** into the world's own slot, the one its session last loaded it from or saved
//!   it into ([`cubarium_session::Session::home`]), so the app's start-up reopens it where
//!   it was. The shell saves into `<root>.staging/<name>` and renames that into
//!   `<root>/<name>` (`game/ui/saves.gd`), so a staging directory maps back to its slot. A
//!   world never loaded or saved (freshly founded) goes to [`FRESH_SLOT`] under the
//!   default root, `user://saves` globalized on Godot's thread when the first session is
//!   made ([`init`]).
//! - **How:** like the shell, into `<root>.staging/<slot>` first, with the slot's
//!   `thumb.png` copied in, then renamed over the slot: an overwritten slot never mixes two
//!   worlds' files, and a failed save leaves it as it was.
//! - **Which world:** the newest session with a world running; with none, nothing is saved.
//! - **Bounded:** a terminating signal gives up after [`QUIT_SAVE_TIMEOUT`] (waiting out a
//!   save or load already in flight counts) and ends the process anyway, so `kill` still
//!   kills; a second `SIGTERM` or `SIGINT` during the save ends it at once; `kill -9`
//!   stays the hard stop.
//!
//! The handlers only queue the signal; a thread of this library takes it and saves, and
//! never calls Godot. One line per signal goes to stderr.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cubarium_session::{SaveMeta, SessionStop};
use signal_hook::consts::{SIGINT, SIGTERM, SIGUSR1};
use signal_hook::iterator::{Handle, Signals};

/// The slot a world never loaded or saved goes to. It passes `saves.gd`'s `name_problem`.
pub const FRESH_SLOT: &str = "Autosave";

/// How long a terminating signal waits for its save before ending the process anyway.
pub const QUIT_SAVE_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a `SIGUSR1` save may take.
const SAVE_TIMEOUT: Duration = Duration::from_secs(120);

/// The shell's thumbnail, kept across a save.
const THUMB: &str = "thumb.png";

/// The default slots' root, `user://saves` globalized.
static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// One save at a time from this module's threads.
static SAVING: Mutex<()> = Mutex::new(());

/// Set once the library is being torn down: the threads stop waiting.
static STOPPING: AtomicBool = AtomicBool::new(false);

/// Raised by the first terminating signal: a second ends the process at once. Also raised at
/// teardown, when nothing takes the signals any more.
static TERM_NOW: OnceLock<Arc<AtomicBool>> = OnceLock::new();
/// Raised at teardown: `SIGUSR1` then has its default action again.
static USR1_DEFAULT: OnceLock<Arc<AtomicBool>> = OnceLock::new();

static SIGNALS: Mutex<Option<(Handle, JoinHandle<()>)>> = Mutex::new(None);

/// Remember the default slots' root and start taking the signals, once. Called on Godot's
/// thread as each session is made.
pub(crate) fn init(root: impl FnOnce() -> PathBuf) {
    if ROOT.get().is_some() {
        return;
    }
    let _ = ROOT.set(root());
    let mut signals = SIGNALS.lock().unwrap_or_else(|e| e.into_inner());
    if signals.is_none() {
        match start_signals() {
            Ok(s) => *signals = Some(s),
            Err(e) => eprintln!("cubarium: could not install the save signal handlers ({e}); signals end the app unsaved"),
        }
    }
}

fn start_signals() -> std::io::Result<(Handle, JoinHandle<()>)> {
    let term_now = TERM_NOW.get_or_init(|| Arc::new(AtomicBool::new(false)));
    let usr1_default = USR1_DEFAULT.get_or_init(|| Arc::new(AtomicBool::new(false)));
    // Handlers run in the order they were registered: the check first, then the flag.
    for sig in [SIGTERM, SIGINT] {
        signal_hook::flag::register_conditional_default(sig, Arc::clone(term_now))?;
        signal_hook::flag::register(sig, Arc::clone(term_now))?;
    }
    signal_hook::flag::register_conditional_default(SIGUSR1, Arc::clone(usr1_default))?;
    let mut signals = Signals::new([SIGUSR1, SIGTERM, SIGINT])?;
    let handle = signals.handle();
    let thread = std::thread::Builder::new()
        .name("cubarium-signals".into())
        .spawn(move || {
            for sig in signals.forever() {
                on_signal(sig);
            }
        })?;
    Ok((handle, thread))
}

fn on_signal(sig: i32) {
    let quit = sig != SIGUSR1;
    let name = match sig {
        SIGUSR1 => "SIGUSR1",
        SIGTERM => "SIGTERM",
        _ => "SIGINT",
    };
    let timeout = if quit { QUIT_SAVE_TIMEOUT } else { SAVE_TIMEOUT };
    let outcome = save_running(&crate::teardown::sessions(), timeout);
    let then = if quit { "; exiting" } else { "" };
    match outcome {
        Outcome::Saved { slot, meta, took } => eprintln!(
            "cubarium: {name}: saved tick {} (day {}) into {} in {:.2} s{then}",
            meta.tick,
            meta.day,
            slot.display(),
            took.as_secs_f64()
        ),
        Outcome::Nothing(why) => eprintln!("cubarium: {name}: nothing saved, {why}{then}"),
        Outcome::Failed { slot, why } => {
            eprintln!("cubarium: {name}: the save into {} failed: {why}{then}", slot.display())
        }
    }
    if quit && !STOPPING.load(Ordering::SeqCst) {
        let _ = signal_hook::low_level::emulate_default_handler(sig);
    }
}

/// What [`save_running`] did.
pub(crate) enum Outcome {
    Saved { slot: PathBuf, meta: SaveMeta, took: Duration },
    Nothing(String),
    Failed { slot: PathBuf, why: String },
}

/// Save the newest session with a world running into its slot, waiting out a save or load
/// already in flight; everything within `timeout`.
pub(crate) fn save_running(sessions: &[SessionStop], timeout: Duration) -> Outcome {
    let started = Instant::now();
    let deadline = started + timeout;
    let Some(session) = sessions.iter().rev().find(|s| s.status().is_some_and(|st| st.running)) else {
        return Outcome::Nothing("no world is running".into());
    };
    let _one = SAVING.lock().unwrap_or_else(|e| e.into_inner());
    // A save or load in flight first: the loop refuses a second save while one writes, and
    // a load replaces the world (and its slot).
    loop {
        match session.status() {
            None => return Outcome::Nothing("the session closed".into()),
            Some(st) if !st.running => return Outcome::Nothing("the world closed".into()),
            Some(st) if !st.saving && !st.loading => break,
            Some(_) if Instant::now() >= deadline || STOPPING.load(Ordering::SeqCst) => {
                return Outcome::Nothing("a save or load in flight did not finish in time".into());
            }
            Some(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let slot = slot_of(session.home(), ROOT.get().map(PathBuf::as_path));
    let Some(slot) = slot else {
        return Outcome::Nothing("the slots' root is unknown".into());
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    match save_into(session, &slot, remaining) {
        Ok(meta) => Outcome::Saved { slot, meta, took: started.elapsed() },
        Err(why) => Outcome::Failed { slot, why },
    }
}

/// The slot a world with `home` saves into: `home` itself, or the slot a staging directory
/// is renamed into; [`FRESH_SLOT`] under `root` for a world with none.
pub(crate) fn slot_of(home: Option<PathBuf>, root: Option<&Path>) -> Option<PathBuf> {
    let Some(home) = home else {
        return root.map(|r| r.join(FRESH_SLOT));
    };
    let name = home.file_name()?;
    let parent = home.parent()?;
    match parent.file_name().and_then(|p| p.to_str()).and_then(|p| p.strip_suffix(".staging")) {
        Some(root_name) if !root_name.is_empty() => Some(parent.with_file_name(root_name).join(name)),
        _ => Some(home.clone()),
    }
}

/// `<root>.staging/<name>` for the slot `<root>/<name>`, as `saves.gd` stages it.
pub(crate) fn staging_of(slot: &Path) -> Option<PathBuf> {
    let name = slot.file_name()?;
    let root = slot.parent()?;
    let mut staging = root.as_os_str().to_os_string();
    staging.push(".staging");
    Some(PathBuf::from(staging).join(name))
}

/// Save `session`'s world into a clean staging directory, copy the slot's thumbnail in, and
/// rename it over `slot`.
fn save_into(session: &SessionStop, slot: &Path, timeout: Duration) -> Result<SaveMeta, String> {
    let staging = staging_of(slot).ok_or("the slot has no name")?;
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| format!("clearing {}: {e}", staging.display()))?;
    }
    std::fs::create_dir_all(&staging).map_err(|e| format!("making {}: {e}", staging.display()))?;
    let meta = session.save_blocking(&staging, timeout)?;
    let thumb = slot.join(THUMB);
    if thumb.is_file() {
        let _ = std::fs::copy(&thumb, staging.join(THUMB));
    }
    if slot.exists() {
        std::fs::remove_dir_all(slot).map_err(|e| format!("clearing the old slot: {e}"))?;
    }
    std::fs::rename(&staging, slot).map_err(|e| format!("moving the save into its slot: {e}"))?;
    Ok(meta)
}

/// The library is being unloaded: a save under way stops waiting. Before the sessions stop.
pub(crate) fn stopping() {
    STOPPING.store(true, Ordering::SeqCst);
}

/// Stop taking the signals and join the thread (the library is being unloaded). From here
/// a `SIGTERM`, `SIGINT` or `SIGUSR1` has its default action.
pub(crate) fn stop() {
    stopping();
    for flag in [TERM_NOW.get(), USR1_DEFAULT.get()].into_iter().flatten() {
        flag.store(true, Ordering::SeqCst);
    }
    let taken = SIGNALS.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some((handle, thread)) = taken {
        handle.close();
        let _ = thread.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_staging_directory_maps_back_to_its_slot() {
        let root = Path::new("/u/saves");
        assert_eq!(slot_of(Some("/u/saves.staging/World 3".into()), Some(root)), Some("/u/saves/World 3".into()));
        assert_eq!(slot_of(Some("/u/saves/World 3".into()), Some(root)), Some("/u/saves/World 3".into()));
        assert_eq!(slot_of(Some("/elsewhere/slot".into()), Some(root)), Some("/elsewhere/slot".into()));
        assert_eq!(slot_of(None, Some(root)), Some("/u/saves/Autosave".into()));
        assert_eq!(slot_of(None, None), None);
        assert_eq!(staging_of(Path::new("/u/saves/World 3")), Some("/u/saves.staging/World 3".into()));
    }
}
