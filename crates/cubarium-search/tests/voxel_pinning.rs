//! Cache study B: episode-worker pinning from the cache topology
//! (`design/handoffs/voxel-cache-and-pinning-2026-09-24.md`).
//!
//! The topology itself is `cubarium-voxel`'s (`tests/cpus.rs`); here the pin argument,
//! the plan it resolves to on a **synthetic** two-chiplet sysfs tree, what a remote is
//! told, and — the one test that touches the real scheduler — a worker pinning its own
//! thread and reading the mask back.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use cubarium_search::es::voxel::pin::{self, PinSpec, Pinning};
use cubarium_search::es::voxel::remote::{DEFAULT_REMOTE_BIN, RemoteSpec};

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "cubarium-pin-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

/// One CPU of a synthetic tree: its SMT siblings, and its L3 (shared list, size) if any.
struct Spec<'a> {
    cpu: usize,
    siblings: &'a str,
    l3: Option<(&'a str, &'a str)>,
}

/// `cpuN/topology/thread_siblings_list` and `cpuN/cache/index{0,1,2,3}` as Linux writes
/// them (L1d, L1i, L2 per core; L3 per chiplet).
fn tree(cpus: &[Spec<'_>]) -> Scratch {
    let s = Scratch::new();
    for c in cpus {
        let dir = s.0.join(format!("cpu{}", c.cpu));
        write(
            &dir.join("topology/thread_siblings_list"),
            &format!("{}\n", c.siblings),
        );
        for (i, (level, kind, size)) in [
            (1, "Data", "48K"),
            (1, "Instruction", "32K"),
            (2, "Unified", "1024K"),
        ]
        .iter()
        .enumerate()
        {
            let idx = dir.join(format!("cache/index{i}"));
            write(&idx.join("level"), &format!("{level}\n"));
            write(&idx.join("type"), &format!("{kind}\n"));
            write(&idx.join("size"), &format!("{size}\n"));
            write(&idx.join("shared_cpu_list"), &format!("{}\n", c.siblings));
        }
        if let Some((shared, size)) = c.l3 {
            let idx = dir.join("cache/index3");
            write(&idx.join("level"), "3\n");
            write(&idx.join("type"), "Unified\n");
            write(&idx.join("size"), &format!("{size}\n"));
            write(&idx.join("shared_cpu_list"), &format!("{shared}\n"));
        }
    }
    s
}

/// A 16-core, 32-thread, two-chiplet part: CPU `c` and `c + 16` are one core; cores 0-7
/// share the `first` L3, cores 8-15 the `second`.
fn two_chiplets(first: &str, second: &str) -> Scratch {
    let sib: Vec<String> = (0..32)
        .map(|c| format!("{},{}", c % 16, c % 16 + 16))
        .collect();
    let specs: Vec<Spec<'_>> = (0..32)
        .map(|c| Spec {
            cpu: c,
            siblings: &sib[c],
            l3: Some(if c % 16 < 8 {
                ("0-7,16-23", first)
            } else {
                ("8-15,24-31", second)
            }),
        })
        .collect();
    tree(&specs)
}

fn range(a: usize, b: usize) -> Vec<usize> {
    (a..b).collect()
}

fn cat(parts: &[Vec<usize>]) -> Vec<usize> {
    parts.concat()
}

#[test]
fn a_pin_argument_is_auto_off_or_a_list() {
    assert_eq!(PinSpec::parse("auto").expect("auto"), PinSpec::Auto);
    assert_eq!(PinSpec::parse("off").expect("off"), PinSpec::Off);
    assert_eq!(
        PinSpec::parse("0-1,16").expect("a list"),
        PinSpec::List(vec![0, 1, 16])
    );
    assert!(PinSpec::parse("fast").is_err());
    assert_eq!(
        PinSpec::parse(pin::DEFAULT_PIN).expect("the default parses"),
        PinSpec::Off,
        "the measured default (A): the scheduler places the workers"
    );
}

#[test]
fn a_plan_wraps_refuses_cpus_outside_the_mask_and_off_is_no_plan() {
    let t = two_chiplets("98304K", "32768K");
    let mask = cat(&[range(0, 8), range(16, 24)]);
    assert_eq!(
        Pinning::resolve(&PinSpec::Off, &t.0, &mask).expect("off"),
        None
    );
    let p = Pinning::resolve(&PinSpec::Auto, &t.0, &mask)
        .expect("auto")
        .expect("a plan");
    assert_eq!(p.cpu_for(0), 0);
    assert_eq!(p.cpu_for(8), 16, "worker 8 takes core 0's sibling");
    assert_eq!(p.cpu_for(16), 0, "more workers than CPUs wrap");
    let err = Pinning::resolve(&PinSpec::List(vec![0, 8]), &t.0, &mask)
        .expect_err("CPU 8 is outside the mask");
    assert!(err.contains("CPU 8"), "{err}");
    let listed = Pinning::resolve(&PinSpec::List(vec![17, 1]), &t.0, &mask)
        .expect("inside")
        .expect("a plan");
    assert_eq!(
        listed.cpus,
        vec![17, 1],
        "an explicit list is taken as written"
    );
}

#[test]
fn the_coordinator_tells_a_remote_its_pin_policy_not_its_cpu_numbers() {
    let spec = |p: PinSpec| {
        RemoteSpec::parse("eidolon.local:12", DEFAULT_REMOTE_BIN)
            .expect("parses")
            .with_pin(&p)
            .argv
            .last()
            .cloned()
            .expect("the remote command")
    };
    assert_eq!(
        spec(PinSpec::Auto),
        "~/cubarium-train/cubarium-search voxel-eval-worker --threads 12 --pin auto"
    );
    assert!(spec(PinSpec::Off).ends_with("--threads 12 --pin off"));
    assert!(
        spec(PinSpec::List(vec![0, 1])).ends_with("--pin auto"),
        "this machine's CPU numbers mean nothing there"
    );
}

#[test]
fn a_placed_worker_runs_on_its_one_cpu_and_an_unplaced_one_anywhere_in_the_mask() {
    let mask = pin::own_affinity().expect("the mask");
    let cpu = *mask.last().expect("some CPU");
    let plan = Pinning {
        spec: PinSpec::List(vec![cpu]),
        cpus: vec![cpu],
    };
    let (pinned, unpinned) = std::thread::scope(|s| {
        let a = s.spawn(|| {
            pin::place_worker(Some(&plan), 3, &mask);
            pin::own_affinity().expect("reads back")
        });
        let b = s.spawn(|| {
            pin::place_worker(Some(&plan), 0, &mask);
            pin::place_worker(None, 0, &mask);
            pin::own_affinity().expect("reads back")
        });
        (a.join().expect("a"), b.join().expect("b"))
    });
    assert_eq!(pinned, vec![cpu]);
    assert_eq!(unpinned, mask, "unplacing widens back to the whole mask");
}
