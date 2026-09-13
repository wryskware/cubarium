//! The host clock: fixed 20 Hz simulation ticks and configurable rendering (60 Hz by
//! default, `--fps`) from one monotonic source, bounded catch-up, and suspend handling.
//!
//! The clock is the only place wall time enters the host. Scenes never see it. Because
//! rendering runs faster than the simulation, every [`Step::Render`] carries the
//! interpolation fraction `f` of the tick in progress — the presenter walks each
//! organism's last-tick path by that fraction, so a 20 Hz world reads as continuous
//! motion (one tick of latency, by design).

use std::time::{Duration, Instant};

/// Simulation rate.
pub const TICK_HZ: u32 = 20;
/// Simulation step in seconds (`dt`).
pub const DT: f64 = 1.0 / TICK_HZ as f64;
/// Default render rate, overridden by `--fps`.
pub const RENDER_HZ: u32 = 60;
/// The range `--fps` accepts.
pub const MIN_FPS: u32 = 1;
pub const MAX_FPS: u32 = 240;
/// Catch-up ticks allowed in one pass before render work is dropped.
pub const MAX_CATCHUP_TICKS: u32 = 4;
/// Being further behind than this is a suspend, not a stall: the clock re-bases rather
/// than fast-forwarding the simulation.
pub const PAUSE_THRESHOLD: Duration = Duration::from_secs(2);
/// The lag report is rate-limited to this interval.
pub const LAG_LOG_INTERVAL: Duration = Duration::from_secs(1);

fn tick_period() -> Duration {
    Duration::from_nanos(1_000_000_000 / u64::from(TICK_HZ))
}

/// The nominal period between frames at `fps`. Frame *instants* are not accumulated
/// from this (see [`Clock::render_at`]): at 60 fps the period is 16.666 ms, and adding a
/// truncated period three times would land a whole 2 ns before every tick boundary and
/// hand the presenter a fraction of 0.99999996 where it should see 0.
fn render_period(fps: u32) -> Duration {
    Duration::from_nanos(1_000_000_000 / u64::from(clamp_fps(fps)))
}

fn clamp_fps(fps: u32) -> u32 {
    fps.clamp(MIN_FPS, MAX_FPS)
}

/// What the loop should do next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Advance the simulation by one tick.
    Tick,
    /// Render and submit one frame. It shows the last completed tick, with the
    /// presenter interpolating `f ∈ [0, 1)` of the way along the path each organism
    /// traveled *during* that tick: `f = 0` is where the tick started, `f → 1`
    /// approaches where it ended.
    Render { f: f64 },
    /// Nothing is due: sleep this long (never a busy-wait).
    Sleep(Duration),
    /// More than [`MAX_CATCHUP_TICKS`] behind: render work for this pass is dropped.
    /// `log` is true at most once per [`LAG_LOG_INTERVAL`].
    Lagged { behind: Duration, log: bool },
    /// The process was suspended (or stalled past [`PAUSE_THRESHOLD`]). The clock has
    /// re-based; no ticks are fast-forwarded across the gap.
    Paused { gap: Duration },
}

/// Fixed-step clock driving simulation and rendering off one monotonic instant.
#[derive(Debug)]
pub struct Clock {
    start: Instant,
    fps: u32,
    tick_period: Duration,
    render_period: Duration,
    next_tick: Instant,
    /// Frames are scheduled as exact multiples of `1 / fps` from `render_base` so the
    /// schedule never drifts against the tick boundaries.
    render_base: Instant,
    render_index: u64,
    tick: u64,
    catchup: u32,
    last_lag_log: Option<Instant>,
}

impl Clock {
    /// A clock rendering at the default [`RENDER_HZ`].
    pub fn new(now: Instant) -> Clock {
        Clock::with_fps(now, RENDER_HZ)
    }

    /// A clock rendering at `fps` (clamped to [`MIN_FPS`]..=[`MAX_FPS`]); the simulation
    /// rate is fixed at [`TICK_HZ`] whatever the render rate is.
    pub fn with_fps(now: Instant, fps: u32) -> Clock {
        let tick_period = tick_period();
        Clock {
            start: now,
            fps: clamp_fps(fps),
            tick_period,
            render_period: render_period(fps),
            next_tick: now + tick_period,
            render_base: now,
            render_index: 0,
            tick: 0,
            catchup: 0,
            last_lag_log: None,
        }
    }

    /// The instant frame `index` is due, measured exactly from `render_base`.
    fn render_at(&self, index: u64) -> Instant {
        self.render_base + Duration::from_nanos(index * 1_000_000_000 / u64::from(self.fps))
    }

    /// Restart the frame schedule from `now`, so the next frame is one period away.
    fn reschedule_renders(&mut self, now: Instant) {
        self.render_base = now;
        self.render_index = 1;
    }

    pub fn start(&self) -> Instant {
        self.start
    }

    /// The number of completed simulation ticks.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.start)
    }

    /// The render rate in use, after clamping.
    pub fn fps(&self) -> u32 {
        self.fps
    }

    /// The nominal render period in use, i.e. `1 / fps`.
    pub fn render_period(&self) -> Duration {
        self.render_period
    }

    /// The interpolation fraction of the tick in progress: the time since the last
    /// completed tick over `dt`, clamped into `[0, 1)`. A caller that renders exactly
    /// when a tick is due sees 0, not 1, so the presenter never draws past the path it
    /// was given.
    pub fn fraction(&self, now: Instant) -> f64 {
        let last_tick = self.next_tick - self.tick_period;
        let since = now.saturating_duration_since(last_tick).as_secs_f64();
        let f = since / self.tick_period.as_secs_f64();
        if !f.is_finite() || f < 0.0 {
            return 0.0;
        }
        f.min(1.0f64.next_down())
    }

    /// Decide the next action. The caller performs it and calls again.
    pub fn next_step(&mut self, now: Instant) -> Step {
        // A suspend shows up as a gap far larger than any plausible stall. Re-base
        // instead of replaying the missing ticks.
        let behind = now.saturating_duration_since(self.next_tick);
        if behind > PAUSE_THRESHOLD {
            let gap = behind + self.tick_period;
            self.rebase(now);
            return Step::Paused { gap };
        }

        if now >= self.next_tick {
            if self.catchup < MAX_CATCHUP_TICKS {
                self.catchup += 1;
                self.tick += 1;
                self.next_tick += self.tick_period;
                return Step::Tick;
            }
            // Still behind after a full run of catch-up ticks: give up the frame, report
            // the lag at most once a second, and keep ticking on the next pass. Ticks are
            // never skipped here, only render work.
            self.catchup = 0;
            let log = match self.last_lag_log {
                Some(t) if now.duration_since(t) < LAG_LOG_INTERVAL => false,
                _ => {
                    self.last_lag_log = Some(now);
                    true
                }
            };
            // The dropped frame must not pile up either.
            if now >= self.render_at(self.render_index) {
                self.reschedule_renders(now);
            }
            return Step::Lagged { behind, log };
        }

        self.catchup = 0;

        if now >= self.render_at(self.render_index) {
            self.render_index += 1;
            if self.render_at(self.render_index) <= now {
                // A single late frame does not turn into a burst of catch-up frames.
                self.reschedule_renders(now);
            }
            return Step::Render { f: self.fraction(now) };
        }

        let due = self.next_tick.min(self.render_at(self.render_index));
        Step::Sleep(due.saturating_duration_since(now))
    }

    /// Re-base the clock on `now`, as if a pause had just ended.
    ///
    /// The care hold uses this: a boundary held for the length of an `fsync` is a pause the
    /// world genuinely spent not stepping, and fast-forwarding afterwards would compress
    /// that time into a burst of catch-up ticks nobody asked for. The world resumes at its
    /// normal rate from wherever the hold left it.
    pub fn rebase_now(&mut self, now: Instant) {
        self.rebase(now);
    }

    fn rebase(&mut self, now: Instant) {
        self.next_tick = now + self.tick_period;
        self.render_base = now;
        self.render_index = 0;
        self.catchup = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_simulation_is_twenty_hertz_and_rendering_defaults_to_sixty() {
        assert_eq!(TICK_HZ, 20);
        assert_eq!(RENDER_HZ, 60);
        assert!((DT - 0.05).abs() < 1e-12);
        assert_eq!(tick_period(), Duration::from_millis(50));
        // Three render frames per tick at the default rate.
        assert_eq!(tick_period().as_nanos() / render_period(RENDER_HZ).as_nanos(), 3);
        assert_eq!(clamp_fps(0), MIN_FPS);
        assert_eq!(clamp_fps(10_000), MAX_FPS);
    }

    /// Over ten seconds of simulated real time the clock must produce 200 ticks and
    /// about 600 renders at the default 60 fps, and never busy-wait.
    #[test]
    fn ten_seconds_yields_the_nominal_tick_and_render_counts() {
        let t0 = Instant::now();
        let mut c = Clock::new(t0);
        let mut now = t0;
        let (mut ticks, mut renders, mut sleeps) = (0u32, 0u32, 0u32);
        let end = t0 + Duration::from_secs(10);
        while now < end {
            match c.next_step(now) {
                Step::Tick => ticks += 1,
                Step::Render { f } => {
                    assert!((0.0..1.0).contains(&f), "fraction {f} is not in [0, 1)");
                    renders += 1;
                }
                Step::Sleep(d) => {
                    assert!(d > Duration::ZERO, "a zero sleep is a busy-wait");
                    sleeps += 1;
                    now += d;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!((199..=200).contains(&ticks), "ticks {ticks}");
        assert!((599..=602).contains(&renders), "renders {renders}");
        // Every tick boundary is also a frame boundary at 60 fps, so there are as many
        // sleeps as there are distinct instants: one per frame.
        assert!(sleeps >= 500, "the loop must sleep between events, got {sleeps}");
        assert_eq!(c.tick(), u64::from(ticks));
    }

    /// `--fps` changes the render rate only: the simulation stays at 20 Hz, and the
    /// fractions handed to the presenter cover the tick evenly.
    #[test]
    fn the_render_rate_follows_the_requested_fps() {
        for fps in [1u32, 24, 30, 60, 90, 120, 240] {
            let t0 = Instant::now();
            let mut c = Clock::with_fps(t0, fps);
            let mut now = t0;
            let (mut ticks, mut renders) = (0u32, 0u32);
            let end = t0 + Duration::from_secs(10);
            while now < end {
                match c.next_step(now) {
                    Step::Tick => ticks += 1,
                    Step::Render { f } => {
                        assert!((0.0..1.0).contains(&f), "fps {fps}: fraction {f}");
                        renders += 1;
                    }
                    Step::Sleep(d) => now += d,
                    other => panic!("unexpected {other:?}"),
                }
            }
            assert!((199..=200).contains(&ticks), "fps {fps}: ticks {ticks}");
            let want = 10 * fps;
            assert!(
                renders.abs_diff(want) <= 2,
                "fps {fps}: {renders} renders, expected about {want}"
            );
        }
    }

    /// At 60 fps there are three frames per tick, and their fractions are 0, 1/3, 2/3 —
    /// the frame that lands exactly on a tick boundary starts the next tick at 0.
    #[test]
    fn sixty_fps_gives_three_frames_per_tick_at_thirds() {
        let t0 = Instant::now();
        let mut c = Clock::with_fps(t0, 60);
        let mut now = t0;
        let mut fractions = Vec::new();
        while now < t0 + Duration::from_millis(150) {
            match c.next_step(now) {
                Step::Render { f } => fractions.push(f),
                Step::Sleep(d) => now += d,
                Step::Tick => {}
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(fractions.len(), 9, "{fractions:?}");
        for (i, f) in fractions.iter().enumerate() {
            let want = (i % 3) as f64 / 3.0;
            assert!((f - want).abs() < 1e-6, "frame {i}: {f} expected {want}");
        }
    }

    #[test]
    fn the_fraction_is_clamped_below_one_while_a_tick_is_owed() {
        let t0 = Instant::now();
        let c = Clock::new(t0);
        assert_eq!(c.fraction(t0), 0.0);
        assert!((c.fraction(t0 + Duration::from_millis(25)) - 0.5).abs() < 1e-9);
        // Past the due tick the fraction saturates just below 1 rather than wrapping.
        let late = c.fraction(t0 + Duration::from_millis(500));
        assert!(late < 1.0 && late > 1.0 - 1e-12, "late fraction {late}");
    }

    #[test]
    fn a_stall_runs_at_most_four_catch_up_ticks_then_drops_render_work() {
        let t0 = Instant::now();
        let mut c = Clock::new(t0);
        // One second of stall: twenty ticks are owed.
        let now = t0 + Duration::from_millis(1_000);
        let mut ticks = 0;
        let mut lagged = 0;
        let mut logged = 0;
        for _ in 0..40 {
            match c.next_step(now) {
                Step::Tick => ticks += 1,
                Step::Lagged { log, .. } => {
                    lagged += 1;
                    if log {
                        logged += 1;
                    }
                }
                Step::Render { .. } => panic!("render work must be dropped while behind"),
                Step::Sleep(_) => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(ticks, 20, "every owed tick still runs");
        assert!(lagged >= 4, "the lag must be reported, got {lagged}");
        assert_eq!(logged, 1, "the lag logs at most once per second");
    }

    #[test]
    fn a_suspend_rebases_and_reports_the_gap_without_fast_forwarding() {
        let t0 = Instant::now();
        let mut c = Clock::new(t0);
        assert_eq!(c.next_step(t0 + Duration::from_millis(50)), Step::Tick);
        let resume = t0 + Duration::from_secs(600);
        match c.next_step(resume) {
            Step::Paused { gap } => {
                assert!(gap > Duration::from_secs(599), "gap {gap:?}");
            }
            other => panic!("expected a pause report, got {other:?}"),
        }
        // Exactly one tick was ever run: the ten missing minutes are not replayed.
        assert_eq!(c.tick(), 1);
        // The clock now runs normally again from the resume instant.
        assert_eq!(c.next_step(resume), Step::Render { f: 0.0 });
        let mut ticks = 0;
        let mut now = resume;
        while now < resume + Duration::from_secs(1) {
            match c.next_step(now) {
                Step::Tick => ticks += 1,
                Step::Render { .. } => {}
                Step::Sleep(d) => now += d,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!((19..=20).contains(&ticks), "ticks {ticks}");
    }

    #[test]
    fn renders_do_not_burst_after_a_single_late_frame() {
        let t0 = Instant::now();
        let mut c = Clock::new(t0);
        // 100 ms late, but under the tick backlog limit after four catch-up ticks.
        let now = t0 + Duration::from_millis(100);
        let mut renders = 0;
        for _ in 0..20 {
            match c.next_step(now) {
                Step::Render { .. } => renders += 1,
                Step::Sleep(_) => break,
                Step::Tick | Step::Lagged { .. } => {}
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(renders <= 1, "a late frame must not replay missed frames, got {renders}");
    }
}
