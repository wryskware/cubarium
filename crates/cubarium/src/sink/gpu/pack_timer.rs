//! Per-phase pack times for `examples/pack_profile.rs`, behind the `pack-profile`
//! feature. Without it [`lap!`] compiles to nothing and [`phases`] is always empty.

/// Close the current phase under `$name` (feature `pack-profile`; nothing otherwise).
macro_rules! lap {
    ($name:expr) => {
        #[cfg(feature = "pack-profile")]
        $crate::sink::gpu::pack_timer::mark($name);
    };
}
pub(crate) use lap;

/// Record a count for this pack under `$name` (feature `pack-profile`; nothing otherwise).
macro_rules! count {
    ($name:expr, $n:expr) => {
        #[cfg(feature = "pack-profile")]
        $crate::sink::gpu::pack_timer::tally($name, $n as f64);
    };
}
pub(crate) use count;

#[cfg(feature = "pack-profile")]
thread_local! {
    static T: std::cell::RefCell<(std::time::Instant, Vec<(&'static str, f64)>)> =
        std::cell::RefCell::new((std::time::Instant::now(), Vec::new()));
}

/// Start a pack's phases.
pub fn start() {
    #[cfg(feature = "pack-profile")]
    T.with(|t| {
        let mut t = t.borrow_mut();
        t.0 = std::time::Instant::now();
        t.1.clear();
    });
}

#[cfg(feature = "pack-profile")]
#[doc(hidden)]
pub fn mark(name: &'static str) {
    T.with(|t| {
        let mut t = t.borrow_mut();
        let now = std::time::Instant::now();
        let ms = (now - t.0).as_secs_f64() * 1e3;
        t.0 = now;
        match t.1.iter_mut().find(|(n, _)| *n == name) {
            Some(p) => p.1 += ms,
            None => t.1.push((name, ms)),
        }
    });
}

#[cfg(feature = "pack-profile")]
#[doc(hidden)]
pub fn tally(name: &'static str, n: f64) {
    T.with(|t| t.borrow_mut().1.push((name, n)));
}

/// The last pack's phases, ms, in the order they first closed (and its counts, named `#…`).
pub fn phases() -> Vec<(&'static str, f64)> {
    #[cfg(feature = "pack-profile")]
    return T.with(|t| t.borrow().1.clone());
    #[cfg(not(feature = "pack-profile"))]
    Vec::new()
}
