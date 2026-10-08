//! Link the extension so the dynamic loader never unmaps it (`-z nodelete`).
//!
//! Godot unloads (dlcloses) the extension at exit. Every thread the extension starts itself
//! is stopped and joined before that (see `teardown` in src/lib.rs), but the process-wide
//! worker pools its dependencies keep — the water exchange's rayon pool (`water-N`), the
//! schedule's bevy task pool — cannot be joined, and a rayon worker that has just finished
//! a job spins for a moment before it sleeps. One still spinning when the library's pages
//! were unmapped faulted (SIGSEGV in `water-4`, quitting mid-founding, 2026-10-08). With
//! the library kept mapped, such a thread is simply ended with the process. The extension
//! is not reloadable (`game/cubarium.gdextension`), so nothing is lost by never unmapping it.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-cdylib=-Wl,-z,nodelete");
    }
}
