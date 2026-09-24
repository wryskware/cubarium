//! **The machine's CPU and cache topology, read from sysfs** — execution only, like every
//! thread count in this crate (`design/handoffs/voxel-cache-and-pinning-2026-09-24.md`).
//!
//! Two callers: the voxel trainer's worker pinning (`cubarium-search`'s `es::voxel::pin`,
//! `--pin auto`), and the live loop, which keeps its pools on one chiplet
//! ([`chiplet_plan`]). Nothing here makes a system call: the callers own the affinity
//! syscalls, and every function takes the sysfs root so it can be tested on a synthetic
//! tree rather than on the machine running the test.

use std::path::Path;

/// Where Linux describes the CPUs.
pub const SYSFS_CPU: &str = "/sys/devices/system/cpu";

/// A CPU list in sysfs / `taskset -c` syntax (`0-3,8,10-11`), **in the order written**,
/// duplicates refused.
pub fn parse_cpu_list(s: &str) -> Result<Vec<usize>, String> {
    let mut out: Vec<usize> = Vec::new();
    for part in s.trim().split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (lo, hi) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let lo: usize = lo
            .parse()
            .map_err(|_| format!("`{part}` is not a CPU or a range"))?;
        let hi: usize = hi
            .parse()
            .map_err(|_| format!("`{part}` is not a CPU or a range"))?;
        if hi < lo {
            return Err(format!("`{part}` runs backwards"));
        }
        for c in lo..=hi {
            if out.contains(&c) {
                return Err(format!("CPU {c} is listed twice"));
            }
            out.push(c);
        }
    }
    if out.is_empty() {
        return Err(format!("`{s}` names no CPU"));
    }
    Ok(out)
}

/// `0-7,16-23` for a set in any order.
pub fn format_cpu_list(cpus: &[usize]) -> String {
    let mut v = cpus.to_vec();
    v.sort_unstable();
    v.dedup();
    let mut parts = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j + 1 < v.len() && v[j + 1] == v[j] + 1 {
            j += 1;
        }
        parts.push(if i == j {
            v[i].to_string()
        } else {
            format!("{}-{}", v[i], v[j])
        });
        i = j + 1;
    }
    parts.join(",")
}

/// One CPU as sysfs describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cpu {
    pub cpu: usize,
    /// The lowest CPU sharing this CPU's L3: the group's name. `usize::MAX` when sysfs
    /// shows no L3 (every such CPU in one group).
    pub l3_group: usize,
    /// That L3's size in bytes (0 when unknown).
    pub l3_bytes: u64,
    /// 0 for the first thread of its physical core, 1 for its SMT sibling, ...
    pub smt_rank: usize,
}

/// Read `cpu_root` (normally [`SYSFS_CPU`]) for the CPUs in `mask`, in `mask`'s order.
/// A CPU whose files are missing is still listed, alone in its core and in the unknown
/// L3 group.
pub fn read_topology(cpu_root: &Path, mask: &[usize]) -> Vec<Cpu> {
    let read = |p: std::path::PathBuf| std::fs::read_to_string(p).ok();
    mask.iter()
        .map(|&cpu| {
            let dir = cpu_root.join(format!("cpu{cpu}"));
            let siblings = read(dir.join("topology/thread_siblings_list"))
                .or_else(|| read(dir.join("topology/core_cpus_list")))
                .and_then(|s| parse_cpu_list(&s).ok())
                .map(|mut v| {
                    v.sort_unstable();
                    v
                })
                .unwrap_or_else(|| vec![cpu]);
            let smt_rank = siblings.iter().position(|&c| c == cpu).unwrap_or(0);
            let mut l3_group = usize::MAX;
            let mut l3_bytes = 0;
            for index in 0..8 {
                let cache = dir.join(format!("cache/index{index}"));
                let Some(level) = read(cache.join("level")) else {
                    continue;
                };
                if level.trim() != "3" {
                    continue;
                }
                if let Some(shared) =
                    read(cache.join("shared_cpu_list")).and_then(|s| parse_cpu_list(&s).ok())
                {
                    l3_group = shared.iter().copied().min().unwrap_or(cpu);
                }
                l3_bytes = read(cache.join("size"))
                    .and_then(|s| parse_size(&s))
                    .unwrap_or(0);
                break;
            }
            Cpu {
                cpu,
                l3_group,
                l3_bytes,
                smt_rank,
            }
        })
        .collect()
}

/// `98304K`, `32M`, `512` → bytes.
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let (digits, scale) = match s.chars().last()? {
        'K' | 'k' => (&s[..s.len() - 1], 1u64 << 10),
        'M' | 'm' => (&s[..s.len() - 1], 1 << 20),
        'G' | 'g' => (&s[..s.len() - 1], 1 << 30),
        _ => (s, 1),
    };
    digits.trim().parse::<u64>().ok().map(|n| n * scale)
}

/// Every physical core's first thread before any SMT sibling; within one SMT tier, the
/// largest L3 first (ties: the group with the lowest CPU), and ascending CPU inside a
/// group. On the 9950X3D with the whole machine that is 0-7 (V-cache), 8-15, 16-23,
/// 24-31; under `taskset -c 0-7,16-23` it is 0-7, 16-23.
pub fn auto_order(topology: &[Cpu]) -> Vec<usize> {
    let mut cpus: Vec<&Cpu> = topology.iter().collect();
    cpus.sort_by(|a, b| {
        a.smt_rank
            .cmp(&b.smt_rank)
            .then(b.l3_bytes.cmp(&a.l3_bytes))
            .then(a.l3_group.cmp(&b.l3_group))
            .then(a.cpu.cmp(&b.cpu))
    });
    cpus.into_iter().map(|c| c.cpu).collect()
}

/// The live loop's chiplet: when `mask` spans **more than one** L3 group, the CPUs of the
/// group with the most of the mask's CPUs, ascending — ties to the larger L3, then to
/// the group with the lowest CPU, so the whole 9950X3D gives the V-cache chiplet; `None`
/// when the mask is already one group or sysfs shows no L3.
///
/// Measured on the 9950X3D (cache study D): the terrarium's water tick on the V-cache
/// chiplet's 15 threads took 1.64 ms steady / 2.50 ms in a shower, against 2.02 / 3.04 ms
/// on 31 threads over both chiplets and 2.08 / 3.33 ms on 8 cores of each — the water
/// phases hand columns between threads every phase, and across chiplets that costs more
/// than the extra cores return.
pub fn chiplet_plan(cpu_root: &Path, mask: &[usize]) -> Option<Vec<usize>> {
    let topology = read_topology(cpu_root, mask);
    let mut groups: Vec<(usize, u64, Vec<usize>)> = Vec::new();
    for c in &topology {
        if c.l3_group == usize::MAX {
            return None;
        }
        match groups.iter_mut().find(|(g, _, _)| *g == c.l3_group) {
            Some((_, _, v)) => v.push(c.cpu),
            None => groups.push((c.l3_group, c.l3_bytes, vec![c.cpu])),
        }
    }
    if groups.len() < 2 {
        return None;
    }
    groups.sort_by(|a, b| {
        b.2.len()
            .cmp(&a.2.len())
            .then(b.1.cmp(&a.1))
            .then(a.0.cmp(&b.0))
    });
    let mut cpus = groups.swap_remove(0).2;
    cpus.sort_unstable();
    Some(cpus)
}
