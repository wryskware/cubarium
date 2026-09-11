//! The host clock: fixed 20 Hz simulation ticks and 30 Hz rendering from one monotonic
//! source, bounded catch-up, and suspend handling.
//!
//! The clock is the only place wall time enters the host. Scenes never see it.

use std::time::{Duration, Instant};

/// Simulation rate.
pub const TICK_HZ: u32 = 20;
/// Simulation step in seconds (`dt`).
pub const DT: f64 = 1.0 / TICK_HZ as f64;
/// Render rate.
pub const RENDER_HZ: u32 = 30;
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

fn render_period() -> Duration {
    Duration::from_nanos(1_000_000_000 / u64::from(RENDER_HZ))
}

/// What the loop should do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Advance the simulation by one tick.
    Tick,
    /// Render and submit one frame; it shows the state of the last completed tick.
    Render,
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
    tick_period: Duration,
    render_period: Duration,
    next_tick: Instant,
    next_render: Instant,
    tick: u64,
    catchup: u32,
    last_lag_log: Option<Instant>,
}

impl Clock {
    pub fn new(now: Instant) -> Clock {
        let tick_period = tick_period();
        let render_period = render_period();
        Clock {
            start: now,
            tick_period,
            render_period,
            next_tick: now + tick_period,
            next_render: now,
            tick: 0,
            catchup: 0,
            last_lag_log: None,
        }
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
            if now >= self.next_render {
                self.next_render = now + self.render_period;
            }
            return Step::Lagged { behind, log };
        }

        self.catchup = 0;

        if now >= self.next_render {
            self.next_render += self.render_period;
            if self.next_render <= now {
                // A single late frame does not turn into a burst of catch-up frames.
                self.next_render = now + self.render_period;
            }
            return Step::Render;
        }

        let due = self.next_tick.min(self.next_render);
        Step::Sleep(due.saturating_duration_since(now))
    }

    fn rebase(&mut self, now: Instant) {
        self.next_tick = now + self.tick_period;
        self.next_render = now;
        self.catchup = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_are_twenty_and_thirty_hertz() {
        assert_eq!(TICK_HZ, 20);
        assert_eq!(RENDER_HZ, 30);
        assert!((DT - 0.05).abs() < 1e-12);
        assert_eq!(tick_period(), Duration::from_millis(50));
    }

    /// Over ten seconds of simulated real time the clock must produce 200 ticks and
    /// about 300 renders, and never busy-wait.
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
                Step::Render => renders += 1,
                Step::Sleep(d) => {
                    assert!(d > Duration::ZERO, "a zero sleep is a busy-wait");
                    sleeps += 1;
                    now += d;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!((199..=200).contains(&ticks), "ticks {ticks}");
        assert!((299..=302).contains(&renders), "renders {renders}");
        assert!(sleeps > 400, "the loop must sleep between events, got {sleeps}");
        assert_eq!(c.tick(), u64::from(ticks));
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
                Step::Render => panic!("render work must be dropped while behind"),
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
        assert_eq!(c.next_step(resume), Step::Render);
        let mut ticks = 0;
        let mut now = resume;
        while now < resume + Duration::from_secs(1) {
            match c.next_step(now) {
                Step::Tick => ticks += 1,
                Step::Render => {}
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
                Step::Render => renders += 1,
                Step::Sleep(_) => break,
                Step::Tick | Step::Lagged { .. } => {}
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(renders <= 1, "a late frame must not replay missed frames, got {renders}");
    }
}
