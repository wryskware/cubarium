//! **The live loop on one chiplet** (cache study D,
//! `design/handoffs/voxel-cache-and-pinning-2026-09-24.md`).
//!
//! On a machine whose CPUs share more than one L3 — the desktop's 9950X3D has two
//! chiplets — the process keeps itself to one of them before the simulation builds its
//! pools, so every pool thread inherits it and `default_threads` counts that chiplet's
//! CPUs. The water phases hand columns from thread to thread at every phase, and across
//! chiplets that costs more than the extra cores return: on the terrarium the water tick
//! took 1.64 ms steady / 2.50 ms in a shower on the V-cache chiplet's 15 threads against
//! 2.02 / 3.04 ms on 31 threads over both, and the live step 4.0 against 4.6 ms/tick for
//! less than half the cycles. The other chiplet stays free.
//!
//! No effect on a one-L3 machine, where the affinity mask is already one chiplet, under
//! `--pin-loop` (the Tachyon's own placement, `placement.rs`, is untouched), or with
//! `--all-chiplets`.

use std::path::Path;

use cubarium_voxel::cpus::{SYSFS_CPU, chiplet_plan, format_cpu_list};

/// Keep this thread — and so every thread it starts from now on — to the chiplet
/// [`chiplet_plan`] names, and say so. Returns the CPUs kept to, if any.
pub fn keep_to_one_chiplet() -> Option<Vec<usize>> {
    let mask = own_affinity().ok()?;
    let cpus = chiplet_plan(Path::new(SYSFS_CPU), &mask)?;
    match set_affinity(&cpus) {
        Ok(()) => {
            eprintln!(
                "cubarium voxel: the simulation keeps to one chiplet, CPUs {} of {} \
                 (`--all-chiplets` spreads it)",
                format_cpu_list(&cpus),
                format_cpu_list(&mask)
            );
            Some(cpus)
        }
        Err(e) => {
            eprintln!("cubarium voxel: could not keep to one chiplet ({e}); running unpinned");
            None
        }
    }
}

/// **The overlapped tick's two legs on cores of their own** (measurement,
/// `design/handoffs/voxel-phase-overlap-2026-09-24.md`): the last `bio_cores` physical
/// cores of this thread's CPUs — every SMT sibling of each — take the compute pool, which
/// runs the plant and animal leg; the rest take the water pool and this thread, which
/// drives the water leg. Builds both pools now, under those affinities, and leaves this
/// thread on the water CPUs. Returns `(water CPUs, bio CPUs)`, or `None` (nothing
/// changed) when the mask has too few cores to split or the affinity calls fail.
pub fn split_overlap_pools(bio_cores: usize) -> Option<(Vec<usize>, Vec<usize>)> {
    use cubarium_voxel::cpus::read_topology;
    let mask = own_affinity().ok()?;
    // Physical cores in mask order, each with its SMT siblings.
    let topology = read_topology(Path::new(SYSFS_CPU), &mask);
    let mut cores: Vec<Vec<usize>> = Vec::new();
    for c in &topology {
        let siblings = std::fs::read_to_string(format!(
            "{SYSFS_CPU}/cpu{}/topology/thread_siblings_list",
            c.cpu
        ))
        .ok()
        .and_then(|s| cubarium_voxel::cpus::parse_cpu_list(&s).ok())
        .unwrap_or_else(|| vec![c.cpu]);
        let first = *siblings.iter().min().unwrap_or(&c.cpu);
        match cores.iter_mut().find(|v| v.contains(&first) || v.contains(&c.cpu)) {
            Some(v) => v.push(c.cpu),
            None => cores.push(vec![c.cpu]),
        }
    }
    cores.sort_by_key(|v| v.iter().copied().min());
    if bio_cores == 0 || cores.len() <= bio_cores {
        return None;
    }
    let split = cores.len() - bio_cores;
    let water: Vec<usize> = cores[..split].iter().flatten().copied().collect();
    let bio: Vec<usize> = cores[split..].iter().flatten().copied().collect();
    set_affinity(&bio).ok()?;
    cubarium_voxel_sim::prepare_compute_pool(bio.len());
    set_affinity(&water).ok()?;
    cubarium_voxel::water::prepare_pool(water.len());
    cubarium_voxel_sim::set_overlap_split(Some((water.len(), bio.len())));
    eprintln!(
        "cubarium voxel: overlapped tick split: water pool on CPUs {}, plants and animals on {}",
        format_cpu_list(&water),
        format_cpu_list(&bio)
    );
    Some((water, bio))
}

/// **The overlapped tick's water yields to the plants and animals** (measurement): every
/// thread of this process named `water-*` — the water pools' workers — gets nice `n`, so
/// when the two legs compete for a CPU the plant and animal leg, the longer one on a grown
/// world, wins it. Returns how many threads it reniced.
pub fn nice_water_pools(n: i32) -> usize {
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else {
        return 0;
    };
    let mut done = 0;
    for task in tasks.flatten() {
        let comm = std::fs::read_to_string(task.path().join("comm")).unwrap_or_default();
        if !comm.starts_with("water-") {
            continue;
        }
        let Some(tid) = task.file_name().to_str().and_then(|t| t.parse::<i32>().ok()) else {
            continue;
        };
        if set_nice(tid, n) {
            done += 1;
        }
    }
    eprintln!("cubarium voxel: {done} water-pool threads at nice {n}");
    done
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)] // Audited: setpriority on one thread of this process.
fn set_nice(tid: i32, n: i32) -> bool {
    // SAFETY: plain syscall on a thread id read from /proc/self/task.
    unsafe { libc::setpriority(libc::PRIO_PROCESS, tid as libc::id_t, n) == 0 }
}

#[cfg(not(target_os = "linux"))]
fn set_nice(_tid: i32, _n: i32) -> bool {
    false
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)] // Audited: sched_getaffinity on the calling thread only.
fn own_affinity() -> std::io::Result<Vec<usize>> {
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

#[cfg(target_os = "linux")]
#[allow(unsafe_code)] // Audited: sched_setaffinity on the calling thread only.
fn set_affinity(cpus: &[usize]) -> std::io::Result<()> {
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
fn own_affinity() -> std::io::Result<Vec<usize>> {
    Err(std::io::Error::other("affinity is Linux-only"))
}

#[cfg(not(target_os = "linux"))]
fn set_affinity(_cpus: &[usize]) -> std::io::Result<()> {
    Err(std::io::Error::other("affinity is Linux-only"))
}
