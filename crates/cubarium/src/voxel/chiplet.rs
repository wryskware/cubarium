//! **Per-thread CPU placement for the live loop's pools** (overlap measurement,
//! `design/handoffs/voxel-phase-overlap-2026-09-24.md`). Which CPUs the process keeps to
//! at all is the host's choice, made outside the program (`taskset` in
//! `scripts/run-live.sh`, per `config/hosts/<host>.env`; `CPUAffinity=` in the Tachyon's
//! unit): the pools are sized to that mask. The desktop's measurement for keeping to one
//! chiplet (cache study D, `design/handoffs/voxel-cache-and-pinning-2026-09-24.md`): the
//! terrarium's water tick took 1.64 / 2.50 ms (steady / shower) on the V-cache chiplet's
//! 15 threads against 2.02 / 3.04 ms on 31 threads over both.

use std::path::Path;

use cubarium_voxel::cpus::{SYSFS_CPU, format_cpu_list};

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
        match cores
            .iter_mut()
            .find(|v| v.contains(&first) || v.contains(&c.cpu))
        {
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
        let Some(tid) = task
            .file_name()
            .to_str()
            .and_then(|t| t.parse::<i32>().ok())
        else {
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
fn set_nice(tid: i32, n: i32) -> bool {
    rustix::process::setpriority_process(rustix::process::Pid::from_raw(tid), n).is_ok()
}

#[cfg(not(target_os = "linux"))]
fn set_nice(_tid: i32, _n: i32) -> bool {
    false
}

#[cfg(target_os = "linux")]
fn own_affinity() -> std::io::Result<Vec<usize>> {
    let set = rustix::thread::sched_getaffinity(None)?;
    Ok((0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|&c| set.is_set(c))
        .collect())
}

#[cfg(target_os = "linux")]
fn set_affinity(cpus: &[usize]) -> std::io::Result<()> {
    let mut set = rustix::thread::CpuSet::new();
    for &c in cpus
        .iter()
        .filter(|&&c| c < rustix::thread::CpuSet::MAX_CPU)
    {
        set.set(c);
    }
    rustix::thread::sched_setaffinity(None, &set).map_err(Into::into)
}

#[cfg(not(target_os = "linux"))]
fn own_affinity() -> std::io::Result<Vec<usize>> {
    Err(std::io::Error::other("affinity is Linux-only"))
}

#[cfg(not(target_os = "linux"))]
fn set_affinity(_cpus: &[usize]) -> std::io::Result<()> {
    Err(std::io::Error::other("affinity is Linux-only"))
}
