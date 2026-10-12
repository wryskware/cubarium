//! **Autosave**: every [`DEFAULT_MINUTES`] of wall time (`CUBARIUM_AUTOSAVE_MIN` minutes,
//! fractions allowed; 0 turns it off), the running world is saved the way the signals save
//! it ([`crate::world_save`]): into its autosave slot, staged and renamed, thumbnail kept.
//! The player's explicitly saved snapshot is untouched.
//!
//! Skipped while no world is running, and while a save or load is already in flight (the
//! next interval tries again). One line per autosave goes to stderr.

use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::world_save::{self, Outcome};

/// The interval when `CUBARIUM_AUTOSAVE_MIN` is not set.
pub const DEFAULT_MINUTES: f64 = 10.0;

/// The variable that sets the interval, in minutes; 0 turns autosave off.
pub const ENV: &str = "CUBARIUM_AUTOSAVE_MIN";

/// How long one autosave may take.
const TIMEOUT: Duration = Duration::from_secs(120);

/// Dropping the sender wakes and ends the thread.
static AUTOSAVE: Mutex<Option<(Sender<()>, JoinHandle<()>)>> = Mutex::new(None);

/// The interval `value` (the variable's) asks for: `None` is off.
pub(crate) fn interval(value: Option<&str>) -> Option<Duration> {
    let minutes = match value.map(str::trim) {
        None | Some("") => DEFAULT_MINUTES,
        Some(v) => match v.parse::<f64>() {
            Ok(m) if m.is_finite() && m >= 0.0 => m,
            _ => {
                eprintln!("cubarium: {ENV}={v} is not a number of minutes; autosaving every {DEFAULT_MINUTES}");
                DEFAULT_MINUTES
            }
        },
    };
    (minutes > 0.0).then(|| Duration::from_secs_f64(minutes * 60.0))
}

/// Start the autosave thread, once.
pub(crate) fn start() {
    let mut slot = AUTOSAVE.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_some() {
        return;
    }
    let Some(every) = interval(std::env::var(ENV).ok().as_deref()) else {
        eprintln!("cubarium: autosave is off ({ENV}=0)");
        return;
    };
    let (tx, rx) = mpsc::channel::<()>();
    let spawned = std::thread::Builder::new().name("cubarium-autosave".into()).spawn(move || {
        while let Err(RecvTimeoutError::Timeout) = rx.recv_timeout(every) {
            autosave();
        }
    });
    match spawned {
        Ok(thread) => *slot = Some((tx, thread)),
        Err(e) => eprintln!("cubarium: could not start the autosave ({e})"),
    }
}

fn autosave() {
    let sessions = crate::teardown::sessions();
    let Some(st) = sessions.iter().rev().find_map(|s| s.status().filter(|st| st.running)) else {
        return;
    };
    if st.saving || st.loading {
        eprintln!("cubarium: autosave skipped, a save or load is in flight");
        return;
    }
    let outcome = world_save::save_running(&sessions, TIMEOUT);
    world_save::report(&outcome, false);
    match outcome {
        Outcome::Saved { slot, meta, took } => eprintln!(
            "cubarium: autosave: saved tick {} (day {}) into {} in {:.2} s",
            meta.tick,
            meta.day,
            slot.display(),
            took.as_secs_f64()
        ),
        Outcome::Nothing(why) => eprintln!("cubarium: autosave: nothing saved, {why}"),
        Outcome::Failed { slot, why } => {
            eprintln!("cubarium: autosave into {} failed: {why}", slot.display())
        }
    }
}

/// End and join the thread (the library is being unloaded).
pub(crate) fn stop() {
    let taken = AUTOSAVE.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some((tx, thread)) = taken {
        drop(tx);
        let _ = thread.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variable_sets_the_interval_and_zero_turns_it_off() {
        assert_eq!(interval(None), Some(Duration::from_secs_f64(DEFAULT_MINUTES * 60.0)));
        assert_eq!(interval(Some("0.5")), Some(Duration::from_secs(30)));
        assert_eq!(interval(Some("0")), None);
        assert_eq!(interval(Some("soon")), interval(None));
        assert_eq!(interval(Some("-3")), interval(None));
    }
}
