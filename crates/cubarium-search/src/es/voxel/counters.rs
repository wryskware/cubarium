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
    fds: Vec<(usize, std::fs::File)>,
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
        for (k, f) in &self.fds {
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

/// `perf_event_attr` up to `config1` (`PERF_ATTR_SIZE_VER0`, 64 bytes).
#[repr(C)]
#[derive(Default)]
struct Attr {
    kind: u32,
    size: u32,
    config: u64,
    sample_period: u64,
    sample_type: u64,
    read_format: u64,
    flags: u64,
    wakeup_events: u32,
    bp_type: u32,
    config1: u64,
}

/// `read_format`: the value, then time enabled and time running (for multiplex scaling).
const FORMAT_TIMES: u64 = 1 | 2;
/// `flags`: disabled = bit 0 is left clear (counting starts at open), exclude_kernel =
/// bit 5, exclude_hv = bit 6.
const FLAGS: u64 = (1 << 5) | (1 << 6);

/// Open one counter on the calling thread, any CPU, user space only.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)] // Audited: perf_event_open on the calling thread; the fd is owned.
fn open(kind: u32, config: u64) -> Option<std::fs::File> {
    use std::os::fd::FromRawFd;
    let attr = Attr {
        kind,
        size: std::mem::size_of::<Attr>() as u32,
        config,
        read_format: FORMAT_TIMES,
        flags: FLAGS,
        ..Attr::default()
    };
    // SAFETY: `attr` is a valid PERF_ATTR_SIZE_VER0 attribute that outlives the call;
    // pid 0 / cpu -1 count the calling thread on any CPU; no group, no flags.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_perf_event_open,
            &attr as *const Attr,
            0 as libc::pid_t,
            -1 as libc::c_int,
            -1 as libc::c_int,
            0 as libc::c_ulong,
        )
    };
    if fd < 0 {
        return None;
    }
    // SAFETY: the kernel just returned this fd to us and nothing else owns it.
    Some(unsafe { std::fs::File::from_raw_fd(fd as i32) })
}

#[cfg(not(target_os = "linux"))]
fn open(_kind: u32, _config: u64) -> Option<std::fs::File> {
    None
}

/// The counter's value scaled by enabled / running time.
fn read(f: &std::fs::File) -> f64 {
    use std::io::Read;
    let mut buf = [0u8; 24];
    let mut f = f;
    if f.read_exact(&mut buf).is_err() {
        return 0.0;
    }
    let word = |i: usize| u64::from_ne_bytes(buf[i * 8..i * 8 + 8].try_into().expect("8 bytes"));
    let (value, enabled, running) = (word(0), word(1), word(2));
    if running == 0 {
        return 0.0;
    }
    value as f64 * enabled as f64 / running as f64
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
