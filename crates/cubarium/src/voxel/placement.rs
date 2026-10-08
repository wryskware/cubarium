//! Where the loop's thread runs on a big.LITTLE board.
//!
//! **The panel's rate is the main thread's budget, and on the Tachyon that budget depends
//! on which core it lands on.** The QCM6490 has four A55s (capacity 381), three A78s (889)
//! and one prime A78 (1024, 2.7 GHz). Left to the scheduler the loop spent ~30 % of its
//! samples on A55s, migrated 646 k times in a run and averaged 1.35 GHz; pinned to the
//! prime by hand its worst step fell from 62–66 ms to 17–29 ms (the 2026-09-22 review in
//! `design/handoffs/terrain-generation-briefs-2026-09-21.md`).
//!
//! So at the top of the loop the host picks the highest-`cpu_capacity` CPU in its own
//! mask, pins the calling thread there, and moves every other thread of the process — the
//! presenter, the tick pool, the web encoder — onto the remaining big cores, off the one
//! the loop owns. The tick pool is fork-join over equal column chunks, so a chunk on an A55
//! is the straggler the whole scope waits for; the remaining big cores are where it waits
//! least. Threads the loop spawns afterwards (the snapshot writer) call
//! [`Placement::leave_loop_core`] first.
//!
//! On a machine whose CPUs all report one capacity — the desk — there is nothing to pick
//! and nothing happens. A CPU the kernel has parked (Qualcomm `core_ctl` isolates idle big
//! cores) refuses an affinity of only itself with `EINVAL`; the pick then falls to the next
//! big core, and to nothing at all if every big core refuses.
//!
//! Then the loop asks for a **utilisation floor** (`uclamp.min`, `sched_setattr`) so that
//! schedutil runs its core at speed while it is runnable, rather than at the ~1 GHz the
//! board showed with the loop pinned but the governor reading a half-idle thread. A kernel
//! without `CONFIG_UCLAMP_TASK` says `EOPNOTSUPP` and the line says so.

use std::path::Path;

/// The loop thread's `uclamp.min`: the whole scale, so its core runs at full clock
/// whenever the loop is runnable. The loop sleeps between ticks and frames (it is < 50 %
/// busy by target), and the floor applies only while it is runnable, so this is "fast
/// while working", not "hot all the time".
pub const LOOP_UCLAMP_MIN: u32 = 1024;

/// One CPU of the process mask and the capacity the kernel reports for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cpu {
    pub id: usize,
    pub capacity: u32,
}

/// The CPUs a loop thread could be pinned to, best first, and the capacities of the rest.
///
/// Built from the sysfs tree alone ([`Plan::read`]), so it is tested on fixture trees; the
/// syscalls are in [`Plan::apply_with`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Every CPU of the mask, highest capacity first (ties by id).
    cpus: Vec<Cpu>,
    /// The lowest capacity in the mask: a CPU above it is a big core.
    little: u32,
}

/// What was done: the loop's core and where every other thread went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub core: Cpu,
    /// The other threads' mask: the big cores of the process mask bar `core`, or every
    /// other CPU of the mask when `core` is the only big one.
    pub rest: Vec<usize>,
}

impl Plan {
    /// Read `cpu_capacity` for every CPU of `mask` under `cpu_root` (normally
    /// `/sys/devices/system/cpu`). `None` when there is nothing to choose: a missing or
    /// unreadable capacity, or every CPU of the mask the same.
    pub fn read(cpu_root: &Path, mask: &[usize]) -> Option<Plan> {
        let mut cpus = Vec::with_capacity(mask.len());
        for &id in mask {
            let text =
                std::fs::read_to_string(cpu_root.join(format!("cpu{id}/cpu_capacity"))).ok()?;
            let capacity = text.trim().parse().ok()?;
            cpus.push(Cpu { id, capacity });
        }
        let little = cpus.iter().map(|c| c.capacity).min()?;
        let big = cpus.iter().map(|c| c.capacity).max()?;
        if big == little {
            return None;
        }
        cpus.sort_by(|a, b| b.capacity.cmp(&a.capacity).then(a.id.cmp(&b.id)));
        Some(Plan { cpus, little })
    }

    /// The loop's candidates in the order they are tried: every big core, best first.
    pub fn candidates(&self) -> impl Iterator<Item = Cpu> + '_ {
        self.cpus
            .iter()
            .copied()
            .filter(|c| c.capacity > self.little)
    }

    /// The other threads' mask when the loop holds `core`.
    pub fn rest_for(&self, core: usize) -> Vec<usize> {
        let mut rest: Vec<usize> = self
            .candidates()
            .map(|c| c.id)
            .filter(|&id| id != core)
            .collect();
        if rest.is_empty() {
            rest = self
                .cpus
                .iter()
                .map(|c| c.id)
                .filter(|&id| id != core)
                .collect();
        }
        rest.sort_unstable();
        rest
    }

    /// Pin the loop with `pin_loop` (the calling thread, one CPU at a time, best first),
    /// and on the first that takes, move every other thread with `pin_other(rest)`.
    /// `None` when every big core refused: nothing else is moved either.
    pub fn apply_with(
        &self,
        mut pin_loop: impl FnMut(usize) -> std::io::Result<()>,
        pin_others: impl FnOnce(&[usize]),
    ) -> Option<Placement> {
        let core = self.candidates().find(|c| pin_loop(c.id).is_ok())?;
        let rest = self.rest_for(core.id);
        pin_others(&rest);
        Some(Placement { core, rest })
    }

    /// The capacity range of the mask, for the log line.
    fn span(&self) -> (u32, u32) {
        (self.little, self.cpus.first().map_or(0, |c| c.capacity))
    }
}

impl Placement {
    /// Take the calling thread off the loop's core: the first thing a thread the loop
    /// spawns after [`place_loop`] does, since it inherits the loop's one-CPU mask.
    pub fn leave_loop_core(&self) {
        let _ = set_affinity(0, &self.rest);
    }
}

/// Pick, pin, clamp, and say so in one line. Returns the placement for threads spawned
/// later, or `None` where there was nothing to pick or nothing would take.
pub fn place_loop() -> Option<Placement> {
    let mask = own_affinity().ok()?;
    let plan = Plan::read(Path::new("/sys/devices/system/cpu"), &mask)?;
    let me = current_tid();
    let placed = plan.apply_with(
        |cpu| set_affinity(0, &[cpu]),
        |rest| {
            for tid in other_tids(me) {
                // A thread that exited between the listing and the call is not an error.
                let _ = set_affinity(tid, rest);
            }
        },
    );
    let (lo, hi) = plan.span();
    match &placed {
        Some(p) => {
            let clamp = match set_uclamp_min(LOOP_UCLAMP_MIN) {
                Ok(()) => format!("uclamp.min {LOOP_UCLAMP_MIN}"),
                Err(e) => format!("no uclamp.min ({e})"),
            };
            eprintln!(
                "cubarium voxel: the loop is on cpu{} (capacity {} of {lo}..{hi}), {} other \
                 thread(s) on cpu {}; {clamp}",
                p.core.id,
                p.core.capacity,
                other_tids(me).len(),
                list(&p.rest),
            );
        }
        None => eprintln!(
            "cubarium voxel: no big core would take the loop (capacities {lo}..{hi}, mask \
             {}); left to the scheduler",
            list(&mask)
        ),
    }
    placed
}

fn list(cpus: &[usize]) -> String {
    cpus.iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(target_os = "linux")]
fn current_tid() -> i32 {
    rustix::thread::gettid().as_raw_nonzero().get()
}

/// The process's other threads, from `/proc/self/task`.
#[cfg(target_os = "linux")]
fn other_tids(me: i32) -> Vec<i32> {
    let Ok(dir) = std::fs::read_dir("/proc/self/task") else {
        return Vec::new();
    };
    dir.flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .filter(|&tid| tid != me)
        .collect()
}

#[cfg(target_os = "linux")]
fn own_affinity() -> std::io::Result<Vec<usize>> {
    let set = rustix::thread::sched_getaffinity(None)?;
    Ok((0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|&c| set.is_set(c))
        .collect())
}

/// `sched_setaffinity` for one thread (`0` is the caller).
#[cfg(target_os = "linux")]
fn set_affinity(tid: i32, cpus: &[usize]) -> std::io::Result<()> {
    let mut set = rustix::thread::CpuSet::new();
    for &c in cpus
        .iter()
        .filter(|&&c| c < rustix::thread::CpuSet::MAX_CPU)
    {
        set.set(c);
    }
    rustix::thread::sched_setaffinity(rustix::process::Pid::from_raw(tid), &set).map_err(Into::into)
}

/// Raise the calling thread's `uclamp.min`, keeping its policy, nice and `uclamp.max`.
/// Uses util-linux's safe process interface; `chrt` and `uclampset` must be installed.
#[cfg(target_os = "linux")]
pub fn set_uclamp_min(value: u32) -> std::io::Result<()> {
    use std::process::Command;

    if value > 1024 && value != u32::MAX {
        return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
    }
    let tid = current_tid().to_string();
    let policy = Command::new("chrt")
        .env("LC_ALL", "C")
        .args(["--pid", &tid])
        .output()?;
    if !policy.status.success() {
        return Err(std::io::Error::other(format!(
            "chrt failed: {}",
            String::from_utf8_lossy(&policy.stderr).trim()
        )));
    }
    let policy = String::from_utf8_lossy(&policy.stdout);
    let policy = policy
        .lines()
        .find(|line| line.contains("current scheduling policy:"))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "chrt did not report the current scheduling policy",
            )
        })?;
    let minimum = if value == u32::MAX {
        "-1".to_owned()
    } else {
        value.to_string()
    };
    let mut command = Command::new("uclampset");
    command
        .env("LC_ALL", "C")
        .args(["--pid", &tid, "-m", &minimum]);
    if policy.contains("|SCHED_RESET_ON_FORK") {
        command.arg("--reset-on-fork");
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "uclampset failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn current_tid() -> i32 {
    0
}
#[cfg(not(target_os = "linux"))]
fn other_tids(_me: i32) -> Vec<i32> {
    Vec::new()
}
#[cfg(not(target_os = "linux"))]
fn own_affinity() -> std::io::Result<Vec<usize>> {
    Err(std::io::ErrorKind::Unsupported.into())
}
#[cfg(not(target_os = "linux"))]
fn set_affinity(_tid: i32, _cpus: &[usize]) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
#[cfg(not(target_os = "linux"))]
pub fn set_uclamp_min(_value: u32) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory, removed when dropped.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new() -> Scratch {
            use std::sync::atomic::{AtomicU32, Ordering};
            static N: AtomicU32 = AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "cubarium-placement-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A sysfs `cpu` tree with these capacities, one directory per CPU.
    fn tree(capacities: &[u32]) -> Scratch {
        let dir = Scratch::new();
        for (id, cap) in capacities.iter().enumerate() {
            let cpu = dir.path().join(format!("cpu{id}"));
            std::fs::create_dir_all(&cpu).unwrap();
            std::fs::write(cpu.join("cpu_capacity"), format!("{cap}\n")).unwrap();
        }
        dir
    }

    const TACHYON: [u32; 8] = [381, 381, 381, 381, 889, 889, 889, 1024];

    #[test]
    fn the_prime_takes_the_loop_and_the_other_big_cores_take_the_rest() {
        let sys = tree(&TACHYON);
        let plan = Plan::read(sys.path(), &[1, 2, 3, 4, 5, 6, 7]).unwrap();
        let mut others = None;
        let placed = plan
            .apply_with(|_| Ok(()), |rest| others = Some(rest.to_vec()))
            .unwrap();
        assert_eq!(
            placed.core,
            Cpu {
                id: 7,
                capacity: 1024
            }
        );
        assert_eq!(placed.rest, vec![4, 5, 6]);
        assert_eq!(others, Some(vec![4, 5, 6]));
    }

    #[test]
    fn equal_capacities_choose_nothing() {
        let sys = tree(&[1024; 8]);
        assert_eq!(Plan::read(sys.path(), &(0..8).collect::<Vec<_>>()), None);
        // Nor does a tree without capacities (a kernel that does not report them).
        let empty = Scratch::new();
        assert_eq!(Plan::read(empty.path(), &[0, 1]), None);
    }

    #[test]
    fn a_prime_outside_the_mask_leaves_the_best_big_core_in_it() {
        let sys = tree(&TACHYON);
        let plan = Plan::read(sys.path(), &[0, 1, 2, 3, 4, 5, 6]).unwrap();
        let placed = plan.apply_with(|_| Ok(()), |_| {}).unwrap();
        assert_eq!(
            placed.core,
            Cpu {
                id: 4,
                capacity: 889
            }
        );
        assert_eq!(placed.rest, vec![5, 6]);
    }

    #[test]
    fn a_parked_prime_refuses_and_the_next_big_core_takes_the_loop() {
        let sys = tree(&TACHYON);
        let plan = Plan::read(sys.path(), &[1, 2, 3, 4, 5, 6, 7]).unwrap();
        let mut tried = Vec::new();
        let placed = plan
            .apply_with(
                |cpu| {
                    tried.push(cpu);
                    if cpu == 7 {
                        Err(std::io::Error::from_raw_os_error(libc::EINVAL))
                    } else {
                        Ok(())
                    }
                },
                |_| {},
            )
            .unwrap();
        assert_eq!(tried, vec![7, 4]);
        assert_eq!(placed.core.id, 4);
        assert_eq!(placed.rest, vec![5, 6, 7]);

        // Every big core parked: nothing is pinned and nothing else is moved.
        let mut moved = false;
        let none = plan.apply_with(
            |_| Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
            |_| moved = true,
        );
        assert_eq!(none, None);
        assert!(!moved);
    }

    #[test]
    fn a_lone_big_core_leaves_the_others_on_the_little_ones() {
        let sys = tree(&[381, 381, 381, 889]);
        let plan = Plan::read(sys.path(), &[0, 1, 2, 3]).unwrap();
        let placed = plan.apply_with(|_| Ok(()), |_| {}).unwrap();
        assert_eq!(placed.core.id, 3);
        assert_eq!(placed.rest, vec![0, 1, 2]);
    }

    /// The affinity wrappers on this kernel: a thread pinned to one CPU of its mask
    /// reads back exactly that CPU, and `leave_loop_core` gives it the rest.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_thread_pinned_by_the_wrappers_reads_back_its_cpus() {
        std::thread::spawn(|| {
            let mask = own_affinity().unwrap();
            let one = *mask.last().unwrap();
            set_affinity(0, &[one]).unwrap();
            assert_eq!(own_affinity().unwrap(), vec![one]);
            let rest: Vec<usize> = mask.iter().copied().filter(|&c| c != one).collect();
            if !rest.is_empty() {
                let p = Placement {
                    core: Cpu {
                        id: one,
                        capacity: 0,
                    },
                    rest: rest.clone(),
                };
                p.leave_loop_core();
                assert_eq!(own_affinity().unwrap(), rest);
            }
        })
        .join()
        .unwrap();
    }

    /// The syscall itself, on this kernel: the clamp reads back as set. A kernel without
    /// `CONFIG_UCLAMP_TASK`, or one that wants `CAP_SYS_NICE` for it, is skipped with its
    /// errno rather than failed — the board's unit grants the capability.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_uclamp_floor_reads_back_on_the_thread_that_set_it() {
        std::thread::spawn(|| match set_uclamp_min(512) {
            Ok(()) => {
                let sched =
                    std::fs::read_to_string(format!("/proc/self/task/{}/sched", current_tid()))
                        .unwrap();
                let minimum = sched
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        (name.trim() == "uclamp.min").then(|| value.trim().parse::<u32>().unwrap())
                    })
                    .expect("kernel reports the clamp minimum");
                assert_eq!(minimum, 512);
            }
            Err(e) => eprintln!("uclamp not settable here: {e}"),
        })
        .join()
        .unwrap();
    }
}
