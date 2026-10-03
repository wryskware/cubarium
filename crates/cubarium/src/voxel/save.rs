//! Saving the whole world on demand: the console's `save [DIR]`, `SIGUSR1`, and Ctrl+S in
//! the development window all end here.
//!
//! A run started without a state directory still saves the full layered snapshot (the
//! world, the plants, the animals) in the layout `--load DIR` resumes. The signal only
//! raises a flag; the run loop polls it and starts the save off the frame.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

/// Where an on-demand save goes when no directory is named, under the working directory.
pub const SAVES_DIR: &str = "saves";

/// Raised by the `SIGUSR1` handler, taken by the loop.
static SAVE_SIGNAL: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigusr1(_: libc::c_int) {
    // Async-signal-safe: one atomic store and nothing else.
    SAVE_SIGNAL.store(true, Ordering::SeqCst);
}

/// Install the `SIGUSR1` handler. Before this the signal's default action ends the
/// process, so the run installs it at start-up, ahead of the founding.
#[allow(unsafe_code)] // Audited: installs a handler that only stores an atomic flag.
pub fn install_save_signal() {
    // SAFETY: `on_sigusr1` is async-signal-safe (one atomic store), and `sigaction` is
    // given a zeroed, fully initialised struct with an empty mask.
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = on_sigusr1 as *const () as libc::sighandler_t;
        sa.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut sa.sa_mask);
        if libc::sigaction(libc::SIGUSR1, &sa, std::ptr::null_mut()) != 0 {
            eprintln!("cubarium voxel: could not install the SIGUSR1 save handler");
        }
    }
}

/// Whether `SIGUSR1` arrived since the last call. Several signals before one poll are
/// one save.
pub fn take_save_signal() -> bool {
    SAVE_SIGNAL.swap(false, Ordering::SeqCst)
}

/// `saves/<YYYYMMDD-HHMMSS>-t<tick>`, in local time.
pub fn default_save_dir(now: SystemTime, tick: u64) -> PathBuf {
    PathBuf::from(SAVES_DIR).join(format!("{}-t{tick}", stamp(now)))
}

/// `YYYYMMDD-HHMMSS` in local time (UTC if the local zone cannot be read).
#[allow(unsafe_code)] // Audited: localtime_r into a stack struct this function owns.
fn stamp(now: SystemTime) -> String {
    let secs = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as libc::time_t;
    // SAFETY: `localtime_r` writes only into `tm`, which outlives the call.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&secs, &mut tm).is_null() }
        || unsafe { !libc::gmtime_r(&secs, &mut tm).is_null() };
    if !ok {
        return format!("{secs}");
    }
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_directory_names_the_tick() {
        let dir = default_save_dir(SystemTime::now(), 4321);
        assert_eq!(dir.parent(), Some(std::path::Path::new(SAVES_DIR)));
        let name = dir.file_name().unwrap().to_str().unwrap();
        assert!(name.ends_with("-t4321"), "{name}");
        // YYYYMMDD-HHMMSS-t4321
        let (date, rest) = name.split_once('-').unwrap();
        assert_eq!(date.len(), 8);
        assert!(date.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(&rest[6..], "-t4321");
    }

    /// One `SIGUSR1` is one save request: taken once, then gone.
    #[test]
    #[allow(unsafe_code)] // Raises a signal at this test process, whose handler is ours.
    fn the_signal_flag_triggers_exactly_one_save() {
        install_save_signal();
        assert!(!take_save_signal());
        // SAFETY: the handler for SIGUSR1 is installed just above.
        assert_eq!(unsafe { libc::raise(libc::SIGUSR1) }, 0);
        assert!(take_save_signal(), "the signal raised the flag");
        assert!(!take_save_signal(), "and it was taken once");
    }
}
