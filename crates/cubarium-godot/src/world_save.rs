//! **Saving the running world from outside the app**: `SIGUSR1` saves it and keeps
//! running; `SIGTERM` and `SIGINT` save it, then end the process as the signal would have.
//!
//! - **Where:** into the world's autosave slot: `<name> (autosave)` beside its home
//!   ([`cubarium_session::Session::home`]). An autosave home reuses that same slot; the
//!   player's explicitly saved snapshot stays untouched. A shell staging home maps back
//!   to its slot before choosing the autosave. A world never loaded or saved goes to
//!   [`FRESH_SLOT`] under the default root, `user://saves` globalized on Godot's thread
//!   when the first session is made ([`init`]). Automatic saves keep the session's home.
//! - **How:** like the shell, into `<root>.staging/<slot>` first, with the autosave slot's
//!   own `thumb.png` copied in (or the home's if it has none), then renamed over the slot:
//!   an overwritten slot never mixes two worlds' files, and a failed save leaves it as it was.
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

/// The suffix shared with the shell's Continue selection (`game/main.gd`).
const AUTOSAVE_SUFFIX: &str = " (autosave)";

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

/// The app's explicit initial/quit save, joined during teardown.
static REQUEST: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static NOTICES: Mutex<Vec<SaveNotice>> = Mutex::new(Vec::new());

#[derive(serde::Serialize)]
pub(crate) struct SaveNotice {
    pub kind: &'static str,
    pub text: String,
    pub path: Option<String>,
    pub requested: bool,
}

pub(crate) fn take_notices() -> Vec<SaveNotice> {
    std::mem::take(&mut *NOTICES.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Ask off Godot's thread; the result appears through `take_notices` after disk commit.
///
/// A request made while an earlier one is still writing (or has reported but not yet
/// exited) is queued behind it, never refused: each request reports exactly once, in order.
pub(crate) fn request(session: SessionStop) -> Result<(), String> {
    let mut request = REQUEST.lock().unwrap_or_else(|e| e.into_inner());
    let previous = request.take();
    let spawned = std::thread::Builder::new()
        .name("cubarium-save-request".into())
        .spawn(move || {
            if let Some(previous) = previous {
                let _ = previous.join();
            }
            let outcome = save_running(&[session], SAVE_TIMEOUT);
            report(&outcome, true);
        });
    match spawned {
        Ok(handle) => {
            *request = Some(handle);
            Ok(())
        }
        // The closure (and the earlier handle in it) is gone; that thread still runs and
        // reports on its own, it is just no longer joined at teardown.
        Err(e) => Err(format!("starting the autosave: {e}")),
    }
}

pub(crate) fn report(outcome: &Outcome, requested: bool) {
    let (kind, text, path) = match outcome {
        Outcome::Saved { slot, .. } => {
            ("autosaved", String::new(), Some(slot.display().to_string()))
        }
        Outcome::Nothing(why) => ("autosave_failed", why.clone(), None),
        Outcome::Failed { slot, why } => (
            "autosave_failed",
            why.clone(),
            Some(slot.display().to_string()),
        ),
    };
    NOTICES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(SaveNotice {
            kind,
            text,
            path,
            requested,
        });
}

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

/// Save the newest session with a world running into its autosave slot, waiting out a save or load
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
    let home = session.home();
    let slot = autosave_slot_of(home.clone(), ROOT.get().map(PathBuf::as_path));
    let Some(slot) = slot else {
        return Outcome::Nothing("the slots' root is unknown".into());
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    let home = slot_of(home, None);
    match save_into(session, &slot, home.as_deref(), remaining) {
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
    match parent
        .file_name()
        .and_then(|p| p.to_str())
        .and_then(|p| p.strip_suffix(".staging"))
    {
        Some(root_name) if !root_name.is_empty() => {
            Some(parent.with_file_name(root_name).join(name))
        }
        _ => Some(home.clone()),
    }
}

/// One automatic snapshot per world, beside its home; an autosave home reuses itself.
/// Fresh worlds keep [`FRESH_SLOT`]. Normalize the shell's staging home first.
pub(crate) fn autosave_slot_of(home: Option<PathBuf>, root: Option<&Path>) -> Option<PathBuf> {
    let fresh = home.is_none();
    let slot = slot_of(home, root)?;
    if fresh {
        return Some(slot);
    }
    let mut name = slot.file_name()?.to_os_string();
    if !name.to_string_lossy().ends_with(AUTOSAVE_SUFFIX) {
        name.push(AUTOSAVE_SUFFIX);
    }
    Some(slot.with_file_name(name))
}

/// `<root>.staging/<name>` for the slot `<root>/<name>`, as `saves.gd` stages it.
pub(crate) fn staging_of(slot: &Path) -> Option<PathBuf> {
    let name = slot.file_name()?;
    let root = slot.parent()?;
    let mut staging = root.as_os_str().to_os_string();
    staging.push(".staging");
    Some(PathBuf::from(staging).join(name))
}

/// Save without changing the session's home, keeping the slot's thumbnail or falling back
/// to the home's, then rename the clean staging directory over `slot`.
fn save_into(session: &SessionStop, slot: &Path, home: Option<&Path>, timeout: Duration) -> Result<SaveMeta, String> {
    let generation = session.status().ok_or("the session closed")?.generation;
    let had_slot = slot.exists();
    let staging = staging_of(slot).ok_or("the slot has no name")?;
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| format!("clearing {}: {e}", staging.display()))?;
    }
    std::fs::create_dir_all(&staging).map_err(|e| format!("making {}: {e}", staging.display()))?;
    let meta = session.save_snapshot_blocking(&staging, timeout)?;
    let thumb = slot.join(THUMB);
    let thumb = if thumb.is_file() { Some(thumb) } else { home.map(|h| h.join(THUMB)).filter(|p| p.is_file()) };
    if let Some(thumb) = thumb {
        std::fs::copy(&thumb, staging.join(THUMB)).map_err(|e| format!("copying {}: {e}", thumb.display()))?;
    }
    if !session.status().is_some_and(|st| st.running && st.generation == generation) {
        return Err("the world closed or changed while saving".into());
    }
    if autosave_slot_of(session.home(), slot.parent()).as_deref() != Some(slot) {
        return Err("the world's autosave home changed while saving".into());
    }
    replace_slot(&staging, slot, had_slot)?;
    Ok(meta)
}

/// Keep the previous completed save until its replacement has moved successfully.
fn replace_slot(staging: &Path, slot: &Path, expected_existing: bool) -> Result<(), String> {
    let name = slot
        .file_name()
        .ok_or("the slot has no name")?
        .to_string_lossy();
    let backup = staging
        .parent()
        .ok_or("the staging directory has no parent")?
        .join(format!(".previous-{name}"));
    if backup.exists() {
        std::fs::remove_dir_all(&backup).map_err(|e| format!("clearing the previous save: {e}"))?;
    }
    let had_slot = slot.exists();
    if expected_existing && !had_slot {
        return Err("the autosave was removed or renamed while saving".into());
    }
    if had_slot {
        std::fs::rename(slot, &backup).map_err(|e| format!("keeping the previous save: {e}"))?;
    }
    if let Err(e) = std::fs::rename(staging, slot) {
        if had_slot {
            std::fs::rename(&backup, slot).map_err(|restore| {
                format!("moving the save: {e}; restoring the previous save: {restore}")
            })?;
        }
        return Err(format!("moving the save into its slot: {e}"));
    }
    // Make both renames durable before the previous save goes: a crash from here on finds
    // the new slot, and a crash before it finds `.previous-<name>`, which `saves.gd`
    // restores at start-up when the slot is missing.
    for dir in [slot.parent(), backup.parent()].into_iter().flatten() {
        if let Err(e) = sync_dir(dir) {
            eprintln!("cubarium: saved, but syncing {} failed: {e}", dir.display());
        }
    }
    if had_slot {
        // A cleanup failure must not report the completed new save as a failed commit.
        if let Err(e) = std::fs::remove_dir_all(&backup) {
            eprintln!(
                "cubarium: saved, but clearing {} failed: {e}",
                backup.display()
            );
        }
    }
    Ok(())
}

/// Flush a directory's entries (the renames in it) to disk.
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::File::open(dir)?.sync_all()
}

/// The library is being unloaded: a save under way stops waiting. Before the sessions stop.
pub(crate) fn stopping() {
    STOPPING.store(true, Ordering::SeqCst);
}

/// Stop taking the signals and join the thread (the library is being unloaded). From here
/// a `SIGTERM`, `SIGINT` or `SIGUSR1` has its default action.
pub(crate) fn stop() {
    stopping();
    if let Some(request) = REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = request.join();
    }
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

    #[test]
    fn autosaves_use_a_sibling_of_the_players_slot() {
        assert_eq!(autosave_slot_of(Some("/r/World 2".into()), None), Some("/r/World 2 (autosave)".into()));
    }

    #[test]
    fn an_autosave_home_reuses_the_same_slot() {
        assert_eq!(autosave_slot_of(Some("/r/World 2 (autosave)".into()), None), Some("/r/World 2 (autosave)".into()));
    }

    #[test]
    fn autosaves_map_staging_homes_back_first() {
        assert_eq!(autosave_slot_of(Some("/r.staging/World 2".into()), None), Some("/r/World 2 (autosave)".into()));
        assert_eq!(autosave_slot_of(Some("/r.staging/World 2 (autosave)".into()), None), Some("/r/World 2 (autosave)".into()));
    }

    #[test]
    fn a_fresh_world_uses_the_fresh_slot() {
        assert_eq!(autosave_slot_of(None, Some(Path::new("/r"))), Some("/r/Autosave".into()));
        assert_eq!(autosave_slot_of(None, None), None);
    }

    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_failed_slot_replacement_restores_the_previous_completed_save() {
        let scratch = Scratch(std::env::temp_dir().join(format!(
                "cubarium-replace-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        let slot = scratch.0.join("saves/World (autosave)");
        let staging = staging_of(&slot).unwrap();
        std::fs::create_dir_all(&slot).unwrap();
        std::fs::create_dir_all(staging.parent().unwrap()).unwrap();
        std::fs::write(slot.join("completed"), b"previous world").unwrap();
        assert!(
            replace_slot(&staging, &slot, true).is_err(),
            "missing replacement must fail"
        );
        assert_eq!(
            std::fs::read(slot.join("completed")).unwrap(),
            b"previous world"
        );
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("completed"), b"new world").unwrap();
        replace_slot(&staging, &slot, true).unwrap();
        assert_eq!(std::fs::read(slot.join("completed")).unwrap(), b"new world");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("completed"), b"late autosave").unwrap();
        std::fs::remove_dir_all(&slot).unwrap();
        assert!(replace_slot(&staging, &slot, true).is_err());
        assert!(!slot.exists(), "a late autosave must not recreate a deleted home");
    }

    fn files(dir: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
        let mut out = std::collections::BTreeMap::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                for (name, bytes) in files(&path) {
                    out.insert(PathBuf::from(path.file_name().unwrap()).join(name), bytes);
                }
            } else {
                out.insert(path.file_name().unwrap().into(), std::fs::read(&path).unwrap());
            }
        }
        out
    }

    fn idle(session: &cubarium_session::Session) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !session.status().running || !session.status().paused || session.status().saving {
            assert!(Instant::now() < deadline, "the tiny world did not become idle");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A paused tiny world in a fresh scratch directory.
    fn tiny_world(tag: &str) -> (Scratch, cubarium_session::Session) {
        use cubarium_session::{Session, SessionOptions, SimParams, WorldSpec, prepare_new};
        let scratch = Scratch(std::env::temp_dir().join(format!(
            "cubarium-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        std::fs::create_dir_all(&scratch.0).unwrap();
        // The same tiny founding fixture as cubarium-session's tests.
        let config = scratch.0.join("tiny.toml");
        std::fs::write(&config,
            "threads = 2\n[world]\nwidth = 32\nheight = 24\ndepth = 4\nvoxel_m = 0.25\n\
             [world.landform]\npreset = \"default\"\n\
             [world.landform.water]\nmin_lake_m2 = 0.0\nmin_tier_pools = 0\n\
             [world.climate]\nopening_min_s = 1.0\n"
        ).unwrap();
        let spec = WorldSpec { config: Some(config), seed: Some(5), ..WorldSpec::default() };
        let session = Session::new(SessionOptions { exec: SimParams { threads: 2, ..SimParams::default() }, ..SessionOptions::default() });
        session.install(prepare_new(&spec.sim_params().unwrap(), &spec).unwrap());
        session.command(cubarium_observatory::Command::Pause);
        idle(&session);
        (scratch, session)
    }

    /// (d) A save asked for while the previous requested save is still writing (a Quit right
    /// after a switch or load save) is queued behind it and completes, never refused.
    #[test]
    fn a_request_while_another_is_writing_is_queued_not_refused() {
        let (scratch, session) = tiny_world("request-queue");
        let root = scratch.0.join("saves");
        std::fs::create_dir_all(&root).unwrap();
        let home = root.join("World 1 (autosave)");
        session.attach_home(&home);
        let stop = session.stopper();
        assert_eq!(request(stop.clone()), Ok(()), "the first save is accepted");
        assert_eq!(request(stop.clone()), Ok(()), "a second save while the first writes is accepted");
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut mine = Vec::new();
        while mine.len() < 2 {
            assert!(Instant::now() < deadline, "both requested saves report");
            let home_text = home.display().to_string();
            mine.extend(take_notices().into_iter().filter(|n| n.path.as_deref() == Some(home_text.as_str())));
            std::thread::sleep(Duration::from_millis(10));
        }
        for n in &mine {
            assert_eq!((n.kind, n.requested), ("autosaved", true), "{}", n.text);
        }
        assert!(cubarium_session::store::read_meta(&home).unwrap().loadable());
        if let Some(h) = REQUEST.lock().unwrap().take() {
            h.join().unwrap();
        }
        stop.shutdown();
    }

    #[test]
    fn automatic_saves_preserve_player_files_and_keep_the_right_thumbnail() {
        use cubarium_session::store;
        let (scratch, session) = tiny_world("autosave");
        let stop = session.stopper();
        let timeout = Duration::from_secs(10);
        let root = scratch.0.join("saves");
        std::fs::create_dir_all(&root).unwrap();

        // A fresh world's repeated automatic snapshots must stay in Autosave.
        for _ in 0..2 {
            let target = autosave_slot_of(session.home(), Some(&root)).unwrap();
            assert_eq!(target, root.join(FRESH_SLOT));
            save_into(&stop, &target, None, timeout).unwrap();
            idle(&session);
            assert_eq!(session.home(), None);
            assert!(store::read_meta(&target).unwrap().loadable());
        }

        // Mimic an explicit shell save: the session remembers its staging path.
        let home = root.join("World 2");
        let staging = staging_of(&home).unwrap();
        session.save_blocking(&staging, timeout).unwrap();
        std::fs::write(staging.join(THUMB), b"player thumbnail").unwrap();
        std::fs::rename(&staging, &home).unwrap();
        idle(&session);
        let before = files(&home);
        let target = root.join("World 2 (autosave)");
        for own_thumbnail in [false, true] {
            if own_thumbnail {
                std::fs::write(target.join(THUMB), b"autosave thumbnail").unwrap();
            }
            let outcome = save_running(std::slice::from_ref(&stop), timeout);
            assert!(matches!(outcome, Outcome::Saved { slot, .. } if slot == target));
            idle(&session);
            assert_eq!(files(&home), before, "all player files stay byte for byte unchanged");
            assert_eq!(session.home(), Some(staging.clone()));
            assert!(store::read_meta(&target).unwrap().loadable());
            let want: &[u8] = if own_thumbnail { b"autosave thumbnail" } else { b"player thumbnail" };
            assert_eq!(std::fs::read(target.join(THUMB)).unwrap(), want);
        }
        assert!(!root.join("World 2 (autosave) (autosave)").exists());
        stop.shutdown();
    }

}
