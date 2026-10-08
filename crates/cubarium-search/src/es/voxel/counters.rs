//! **Hardware counters per episode worker** (`design/handoffs/voxel-cache-and-pinning-2026-09-24.md`,
//! A): cycles, instructions and where the L1d's demand misses were filled from, read
//! through `perf_event_open` on each worker thread, user space only (what
//! `perf_event_paranoid` 2 allows for one's own threads). Off unless `CUBARIUM_COUNTERS`
//! is set; then every worker pool that opens a [`Probe`] adds its thread's counts to a
//! process tally the command prints ([`take`]) — per generation, per held-out check, per
//! bench pool.
//!
//! The fill-source events are AMD Zen 5's (`ls_dmnd_fills_from_sys`, event 0x43): on
//! another CPU only cycles and instructions are opened. The miss rates are of the
//! **demand** fills — the loads that waited — not of prefetches:
//! - L2 miss rate: of the L1d's demand fills, those not served by the core's own L2;
//! - L3 miss rate: of those, the ones not served by the chiplet's own L3 (DRAM, or
//!   another chiplet's cache).
//!
//! Development tooling: nothing here reaches an episode.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[cfg(target_os = "linux")]
use perf_event::Counter;
#[cfg(not(target_os = "linux"))]
type Counter = std::fs::File;

/// `(name, perf type, config)`: PERF_TYPE_HARDWARE = 0, PERF_TYPE_RAW = 4.
const EVENTS: [(&str, u32, u64); 6] = [
    ("cycles", 0, 0),
    ("instructions", 0, 1),
    // ls_dmnd_fills_from_sys: all / local_l2 / local_ccx / (dram_io_all | far_cache |
    // near_cache): the L1d's demand fills by where they came from.
    ("l1d_fills", 4, 0xff43),
    ("l2_hits", 4, 0x0143),
    ("l3_hits", 4, 0x0243),
    ("beyond_l3", 4, 0x5c43),
];

/// Whether `CUBARIUM_COUNTERS` asked for counters (read once).
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("CUBARIUM_COUNTERS").is_ok_and(|v| !v.is_empty() && v != "0"))
}

/// Whether this CPU is a Zen 5 (family 0x1A), whose fill-source encodings [`EVENTS`] uses.
fn zen5() -> bool {
    static ZEN5: OnceLock<bool> = OnceLock::new();
    *ZEN5.get_or_init(|| {
        let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let field = |k: &str| {
            info.lines()
                .find(|l| l.starts_with(k))
                .and_then(|l| l.split(':').nth(1))
                .map(|v| v.trim().to_string())
        };
        field("vendor_id").as_deref() == Some("AuthenticAMD")
            && field("cpu family").as_deref() == Some("26")
    })
}

/// One thread's (or a tally's) counts. Counts are scaled for multiplexing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub threads: usize,
    pub cycles: f64,
    pub instructions: f64,
    pub l1d_fills: f64,
    pub l2_hits: f64,
    pub l3_hits: f64,
    pub beyond_l3: f64,
    /// Ticks the thread's episodes ran.
    pub ticks: u64,
    /// Seconds the thread spent inside episodes.
    pub busy_s: f64,
}

impl Sample {
    fn add(&mut self, o: &Sample) {
        self.threads += o.threads;
        self.cycles += o.cycles;
        self.instructions += o.instructions;
        self.l1d_fills += o.l1d_fills;
        self.l2_hits += o.l2_hits;
        self.l3_hits += o.l3_hits;
        self.beyond_l3 += o.beyond_l3;
        self.ticks += o.ticks;
        self.busy_s += o.busy_s;
    }

    /// The CSV header of [`Sample::line`].
    pub const HEADER: &'static str = "counters,label,threads,ticks,wall_s,ticks_per_s,\
         ticks_per_s_per_worker,ipc,cycles_per_tick,l1d_mpki,l2_mpki,l3_mpki,l2_miss_rate,\
         l3_miss_rate";

    /// One CSV row: `wall_s` is the pool's wall time (total rate); the per-worker rate is
    /// over each thread's busy time.
    pub fn line(&self, label: &str, wall_s: f64) -> String {
        let per_k = |x: f64| 1000.0 * x / self.instructions.max(1.0);
        let l2_miss = self.l1d_fills - self.l2_hits;
        format!(
            "counters,{label},{},{},{wall_s:.2},{:.0},{:.0},{:.3},{:.0},{:.2},{:.3},{:.4},{:.3},{:.3}",
            self.threads,
            self.ticks,
            self.ticks as f64 / wall_s.max(1e-9),
            self.ticks as f64 / self.busy_s.max(1e-9),
            self.instructions / self.cycles.max(1.0),
            self.cycles / (self.ticks.max(1) as f64),
            per_k(self.l1d_fills),
            per_k(l2_miss),
            per_k(self.beyond_l3),
            l2_miss / self.l1d_fills.max(1.0),
            self.beyond_l3 / l2_miss.max(1.0),
        )
    }
}

static TALLY: Mutex<Sample> = Mutex::new(Sample {
    threads: 0,
    cycles: 0.0,
    instructions: 0.0,
    l1d_fills: 0.0,
    l2_hits: 0.0,
    l3_hits: 0.0,
    beyond_l3: 0.0,
    ticks: 0,
    busy_s: 0.0,
});

/// The tally since the last `take`, reset.
pub fn take() -> Sample {
    std::mem::take(&mut *TALLY.lock().expect("counter tally"))
}

/// One worker thread's open counters. Dropping it adds the thread's counts to the tally.
pub struct Probe {
    fds: Vec<(usize, Counter)>,
    pub ticks: u64,
    pub busy_s: f64,
}

impl Probe {
    /// Open and start the counters on the calling thread, when [`enabled`].
    pub fn start() -> Option<Probe> {
        if !enabled() {
            return None;
        }
        let raw = zen5();
        let fds = EVENTS
            .iter()
            .enumerate()
            .filter(|(_, (_, ty, _))| raw || *ty != 4)
            .filter_map(|(k, &(_, ty, config))| open(ty, config).map(|f| (k, f)))
            .collect();
        Some(Probe {
            fds,
            ticks: 0,
            busy_s: 0.0,
        })
    }

    /// Count one finished episode.
    pub fn episode(&mut self, ticks: u64, since: Instant) {
        self.ticks += ticks;
        self.busy_s += since.elapsed().as_secs_f64();
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let mut s = Sample {
            threads: 1,
            ticks: self.ticks,
            busy_s: self.busy_s,
            ..Sample::default()
        };
        for (k, f) in &mut self.fds {
            let v = read(f);
            match EVENTS[*k].0 {
                "cycles" => s.cycles = v,
                "instructions" => s.instructions = v,
                "l1d_fills" => s.l1d_fills = v,
                "l2_hits" => s.l2_hits = v,
                "l3_hits" => s.l3_hits = v,
                _ => s.beyond_l3 = v,
            }
        }
        TALLY.lock().expect("counter tally").add(&s);
    }
}

/// Open one counter on the calling thread, any CPU, user space only.
#[cfg(target_os = "linux")]
fn open(kind: u32, config: u64) -> Option<Counter> {
    use perf_event::events::{Hardware, Raw};
    use perf_event::{Builder, ReadFormat};
    let mut builder = match kind {
        0 => Builder::new(Hardware(config)),
        4 => Builder::new(Raw::new(config)),
        _ => return None,
    };
    builder
        .observe_self()
        .any_cpu()
        .enabled(true)
        .exclude_kernel(true)
        .exclude_hv(true)
        .read_format(ReadFormat::TOTAL_TIME_ENABLED | ReadFormat::TOTAL_TIME_RUNNING)
        .build()
        .ok()
}

#[cfg(not(target_os = "linux"))]
fn open(_kind: u32, _config: u64) -> Option<Counter> {
    None
}

/// The counter's value scaled by enabled / running time.
#[cfg(target_os = "linux")]
fn read(counter: &mut Counter) -> f64 {
    let Ok(sample) = counter.read_count_and_time() else {
        return 0.0;
    };
    if sample.time_running == 0 {
        return 0.0;
    }
    sample.count as f64 * sample.time_enabled as f64 / sample.time_running as f64
}

#[cfg(not(target_os = "linux"))]
fn read(_counter: &mut Counter) -> f64 {
    0.0
}

/// Per-CPU `(busy, total)` jiffies from `/proc/stat`, indexed by CPU number: the machine's
/// load over an interval is the difference of two readings ([`busy_share`]).
pub fn cpu_times() -> Vec<(u64, u64)> {
    let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let mut out: Vec<(u64, u64)> = Vec::new();
    for l in stat.lines() {
        let Some(rest) = l.strip_prefix("cpu") else {
            continue;
        };
        let mut f = rest.split_whitespace();
        let Some(Ok(cpu)) = f.next().map(str::parse::<usize>) else {
            continue; // the aggregate line
        };
        let v: Vec<u64> = f.filter_map(|x| x.parse().ok()).collect();
        let total: u64 = v.iter().take(8).sum();
        let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
        if out.len() <= cpu {
            out.resize(cpu + 1, (0, 0));
        }
        out[cpu] = (total - idle, total);
    }
    out
}

/// The busy share of `cpus` between two [`cpu_times`] readings.
pub fn busy_share(before: &[(u64, u64)], after: &[(u64, u64)], cpus: &[usize]) -> f64 {
    let (mut busy, mut total) = (0u64, 0u64);
    for &c in cpus {
        if let (Some(a), Some(b)) = (before.get(c), after.get(c)) {
            busy += b.0.saturating_sub(a.0);
            total += b.1.saturating_sub(a.1);
        }
    }
    busy as f64 / total.max(1) as f64
}

/// This process's `(user s, system s, minor faults)` so far, from `/proc/self/stat`.
pub fn process_times() -> (f64, f64, u64) {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // Fields after the parenthesised command name: state is field 3, minflt 10, utime 14,
    // stime 15 (1-based over the whole line).
    let rest = stat.rsplit_once(')').map_or("", |(_, r)| r);
    let f: Vec<&str> = rest.split_whitespace().collect();
    let num = |i: usize| {
        f.get(i - 3)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let hz = 100.0;
    (num(14) as f64 / hz, num(15) as f64 / hz, num(10))
}
