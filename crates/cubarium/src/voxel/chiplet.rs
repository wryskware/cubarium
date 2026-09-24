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
