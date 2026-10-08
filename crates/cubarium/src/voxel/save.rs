//! Saving the whole world on demand: the console's `save [DIR]`, `SIGUSR1`, and Ctrl+S in
//! the development window all end here.
//!
//! A run started without a state directory still saves the full layered snapshot (the
//! world, the plants, the animals) in the layout `--load DIR` resumes. The signal only
//! raises a flag; the run loop polls it and starts the save off the frame.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

/// Where an on-demand save goes when no directory is named, under the working directory.
pub const SAVES_DIR: &str = "saves";

/// Raised by the `SIGUSR1` handler, taken by the loop.
static SAVE_SIGNAL: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// Install the `SIGUSR1` handler. Before this the signal's default action ends the
/// process, so the run installs it at start-up, ahead of the founding.
pub fn install_save_signal() {
    SAVE_SIGNAL.get_or_init(|| {
        let flag = Arc::new(AtomicBool::new(false));
        if signal_hook::flag::register(signal_hook::consts::SIGUSR1, Arc::clone(&flag)).is_err() {
            eprintln!("cubarium voxel: could not install the SIGUSR1 save handler");
        }
        flag
    });
}

/// Whether `SIGUSR1` arrived since the last call. Several signals before one poll are
/// one save.
pub fn take_save_signal() -> bool {
    SAVE_SIGNAL
        .get()
        .is_some_and(|flag| flag.swap(false, Ordering::SeqCst))
}

/// `saves/<YYYYMMDD-HHMMSS>-t<tick>`, in local time.
pub fn default_save_dir(now: SystemTime, tick: u64) -> PathBuf {
    PathBuf::from(SAVES_DIR).join(format!("{}-t{tick}", stamp(now)))
}

/// `YYYYMMDD-HHMMSS` in local time (UTC if the local zone cannot be read).
fn stamp(now: SystemTime) -> String {
    let secs = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let Some(utc) = chrono::DateTime::from_timestamp(secs, 0) else {
        return format!("{secs}");
    };
    utc.with_timezone(&chrono::Local)
        .format("%Y%m%d-%H%M%S")
        .to_string()
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
    fn the_signal_flag_triggers_exactly_one_save() {
        install_save_signal();
        assert!(!take_save_signal());
        signal_hook::low_level::raise(signal_hook::consts::SIGUSR1).unwrap();
        assert!(take_save_signal(), "the signal raised the flag");
        assert!(!take_save_signal(), "and it was taken once");
    }
}
