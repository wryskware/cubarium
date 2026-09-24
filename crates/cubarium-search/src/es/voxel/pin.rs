//! **Worker pinning** (`design/handoffs/voxel-cache-and-pinning-2026-09-24.md`, B): episode
//! worker `i` takes one CPU, chosen from the machine's cache topology.
//!
//! `--pin auto` reads sysfs within the process's current affinity mask (so `taskset` and
//! a cgroup cpuset are respected) and orders the CPUs **physical cores first, the largest
//! L3 first** — on the 9950X3D the V-cache CCD's eight cores, then the other CCD's eight,
//! then the SMT siblings in the same order. `--pin 0-7,16-23` is an explicit list, in the
//! order given. `--pin off`, the default ([`DEFAULT_PIN`]), leaves the scheduler to it:
//! on this desktop the scheduler did as well or better. The topology itself is
//! `cubarium_voxel::cpus`, shared with the live loop.
//!
//! The setting is process-wide ([`set_process`]): one process runs on one machine, and
//! every worker pool in it (a generation's workers, the held-out evaluation, a remote
//! worker's threads, the landscape bench) asks [`pin_worker`] for its `i`-th CPU. Without
//! a setting nothing is pinned, so tests and library callers are unchanged. Execution
//! only: which CPU runs an episode cannot reach its result.

use std::path::Path;
use std::sync::OnceLock;

pub use cubarium_voxel::cpus::{Cpu, auto_order, format_cpu_list, parse_cpu_list, read_topology};

/// What `--pin` asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PinSpec {
    /// No pinning: the scheduler places the workers.
    Off,
    /// The cache-topology order within the current affinity mask ([`auto_order`]).
    Auto,
    /// These CPUs, in this order: worker `i` takes `cpus[i % len]`.
    List(Vec<usize>),
}

/// The default: **off**. Measured on the 9950X3D (cache study A), pinning never beat
/// the scheduler: 16 workers on one chiplet's 16 threads ran the same pinned or not
/// (landscape bench and 2-update `voxel-train` smokes, within 1 %); on the whole machine
/// the scheduler already spread 16 unpinned workers over both chiplets' physical cores
/// (the same 260 k ticks/s as `auto`), and at 32 workers pinning lost 2 %; on eidolon
/// under another job's load it lost 2-4 %, because a pinned worker cannot leave a busy
/// CPU. `auto` and a list stay for placing a run deliberately.
pub const DEFAULT_PIN: &str = "off";

impl PinSpec {
    /// `auto`, `off`, or a CPU list such as `0-7,16-23`.
    pub fn parse(s: &str) -> Result<PinSpec, String> {
        match s.trim() {
            "auto" => Ok(PinSpec::Auto),
            "off" | "none" => Ok(PinSpec::Off),
            list => parse_cpu_list(list)
                .map(PinSpec::List)
                .map_err(|e| format!("--pin wants auto, off or a CPU list: {e}")),
        }
    }

    /// What a remote worker is told: the policy, not this machine's CPU numbers. An
    /// explicit list names CPUs of this machine, so a remote reads its own topology.
    pub fn remote_arg(&self) -> &'static str {
        match self {
            PinSpec::Off => "off",
            PinSpec::Auto | PinSpec::List(_) => "auto",
        }
    }
}

/// A resolved plan: worker `i` runs on `cpus[i % cpus.len()]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pinning {
    pub spec: PinSpec,
    pub cpus: Vec<usize>,
}

impl Pinning {
    /// Resolve `spec` against `mask` (the current affinity) and `cpu_root`'s topology.
    /// `Off` resolves to `None`. An explicit CPU outside the mask is refused by name:
    /// the mask is the operator's or the cgroup's limit, and pinning never widens it.
    pub fn resolve(
        spec: &PinSpec,
        cpu_root: &Path,
        mask: &[usize],
    ) -> Result<Option<Pinning>, String> {
        let cpus = match spec {
            PinSpec::Off => return Ok(None),
            PinSpec::Auto => auto_order(&read_topology(cpu_root, mask)),
            PinSpec::List(list) => {
                if let Some(c) = list.iter().find(|c| !mask.contains(c)) {
                    return Err(format!(
                        "--pin names CPU {c}, outside this process's affinity {}",
                        format_cpu_list(mask)
                    ));
                }
                list.clone()
            }
        };
        if cpus.is_empty() {
            return Err("--pin found no CPU to place workers on".into());
        }
        Ok(Some(Pinning {
            spec: spec.clone(),
            cpus,
        }))
    }

    /// Worker `i`'s CPU.
    pub fn cpu_for(&self, worker: usize) -> usize {
        self.cpus[worker % self.cpus.len()]
    }
}

static PROCESS: OnceLock<Option<Pinning>> = OnceLock::new();

/// Set this process's worker pinning from `--pin`, once, and say what it chose. A second
/// call is ignored (the first setting stands).
pub fn set_process(spec: &PinSpec) -> Result<(), String> {
    let mask = own_affinity().map_err(|e| format!("reading the affinity mask: {e}"))?;
    let pinning = Pinning::resolve(spec, Path::new(cubarium_voxel::cpus::SYSFS_CPU), &mask)?;
    match &pinning {
        None => eprintln!("# pin: off (the scheduler places the workers)"),
        Some(p) => eprintln!(
            "# pin: {} within affinity {} → worker i on [{}]",
            match spec {
                PinSpec::Auto => "auto",
                PinSpec::List(_) => "list",
                PinSpec::Off => "off",
            },
            format_cpu_list(&mask),
            p.cpus
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
    let _ = PROCESS.set(pinning);
    Ok(())
}

/// This process's pinning, if one was set.
pub fn process() -> Option<&'static Pinning> {
    PROCESS.get().and_then(Option::as_ref)
}

/// Place the calling thread as worker `i` of `pinning`, or anywhere in `mask` when
/// there is none (a bench arm that runs unpinned after a pinned one).
pub fn place_worker(pinning: Option<&Pinning>, i: usize, mask: &[usize]) {
    let cpus = match pinning {
        Some(p) => vec![p.cpu_for(i)],
        None => mask.to_vec(),
    };
    if let Err(e) = set_affinity(&cpus) {
        eprintln!(
            "# pin: worker {i} could not take {}: {e}",
            format_cpu_list(&cpus)
        );
    }
}

/// Pin the calling thread as worker `i`, if the process has a pinning. A refusal from
/// the kernel is reported once per call and the worker runs unpinned.
pub fn pin_worker(i: usize) {
    if let Some(p) = process() {
        let cpu = p.cpu_for(i);
        if let Err(e) = set_affinity(&[cpu]) {
            eprintln!("# pin: worker {i} could not take CPU {cpu}: {e}");
        }
    }
}

/// The calling thread's affinity mask.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)] // Audited: sched_getaffinity on the calling thread only.
pub fn own_affinity() -> std::io::Result<Vec<usize>> {
    // SAFETY: a zeroed `cpu_set_t` is a valid empty set, and the size passed is its own.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        if libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok((0..libc::CPU_SETSIZE as usize)
            .filter(|&c| libc::CPU_ISSET(c, &set))
            .collect())
    }
}

/// Restrict the calling thread to `cpus`.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)] // Audited: sched_setaffinity on the calling thread only.
pub fn set_affinity(cpus: &[usize]) -> std::io::Result<()> {
    // SAFETY: as in `own_affinity`; `CPU_SET` is bounded by `CPU_SETSIZE` below.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        for &c in cpus.iter().filter(|&&c| c < libc::CPU_SETSIZE as usize) {
            libc::CPU_SET(c, &mut set);
        }
        if libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn own_affinity() -> std::io::Result<Vec<usize>> {
    Ok((0..std::thread::available_parallelism().map_or(1, std::num::NonZero::get)).collect())
}

#[cfg(not(target_os = "linux"))]
pub fn set_affinity(_cpus: &[usize]) -> std::io::Result<()> {
    Err(std::io::Error::other("pinning is Linux-only"))
}
