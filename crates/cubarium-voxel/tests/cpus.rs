//! Cache study B and D (`design/handoffs/voxel-cache-and-pinning-2026-09-24.md`): the
//! CPU and cache topology read from **synthetic sysfs trees** built here — a two-chiplet
//! part with one large and one small L3 and SMT siblings, a one-chiplet part, a tree with
//! no cache files — never from the machine running the test.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use cubarium_voxel::cpus::{
    auto_order, chiplet_plan, format_cpu_list, parse_cpu_list, read_topology,
};

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "cubarium-cpus-{}-{}",
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
fn a_cpu_list_keeps_its_order_and_refuses_duplicates_and_nonsense() {
    assert_eq!(
        parse_cpu_list("0-3,8,10-11").expect("parses"),
        vec![0, 1, 2, 3, 8, 10, 11]
    );
    assert_eq!(
        parse_cpu_list("16-17,0-1\n").expect("parses"),
        vec![16, 17, 0, 1],
        "the order written is the order workers take"
    );
    assert!(parse_cpu_list("3,3").is_err(), "a CPU twice");
    assert!(
        parse_cpu_list("0-3,2").is_err(),
        "a CPU twice through a range"
    );
    assert!(parse_cpu_list("5-2").is_err(), "a backwards range");
    assert!(parse_cpu_list("x").is_err());
    assert!(parse_cpu_list("").is_err());
    assert_eq!(format_cpu_list(&[16, 0, 1, 2, 17, 5]), "0-2,5,16-17");
}

#[test]
fn auto_takes_every_physical_core_before_any_sibling_and_the_large_l3_first() {
    let t = two_chiplets("98304K", "32768K");
    let all = range(0, 32);
    assert_eq!(
        auto_order(&read_topology(&t.0, &all)),
        cat(&[range(0, 8), range(8, 16), range(16, 24), range(24, 32)]),
        "the V-cache chiplet's cores, the other chiplet's cores, then the siblings"
    );
    // The large L3 on the second chiplet: it still goes first.
    let t = two_chiplets("32768K", "98304K");
    assert_eq!(
        auto_order(&read_topology(&t.0, &all)),
        cat(&[range(8, 16), range(0, 8), range(24, 32), range(16, 24)]),
    );
}

#[test]
fn auto_stays_inside_the_affinity_mask() {
    let t = two_chiplets("98304K", "32768K");
    let ccd0 = cat(&[range(0, 8), range(16, 24)]);
    assert_eq!(
        auto_order(&read_topology(&t.0, &ccd0)),
        ccd0,
        "under `taskset -c 0-7,16-23`: the cores, then their siblings"
    );
    let ccd1 = cat(&[range(8, 16), range(24, 32)]);
    assert_eq!(auto_order(&read_topology(&t.0, &ccd1)), ccd1);
    let odd = vec![3, 19, 9];
    assert_eq!(
        auto_order(&read_topology(&t.0, &odd)),
        vec![3, 9, 19],
        "a sibling whose core is outside the mask still counts as a sibling"
    );
}

#[test]
fn a_one_chiplet_part_orders_its_cores_then_their_siblings() {
    let sib: Vec<String> = (0..12)
        .map(|c| format!("{},{}", c % 6, c % 6 + 6))
        .collect();
    let specs: Vec<Spec<'_>> = (0..12)
        .map(|c| Spec {
            cpu: c,
            siblings: &sib[c],
            l3: Some(("0-11", "32768K")),
        })
        .collect();
    let t = tree(&specs);
    let order = auto_order(&read_topology(&t.0, &range(0, 12)));
    assert_eq!(order, cat(&[range(0, 6), range(6, 12)]));
}

#[test]
fn a_tree_without_cache_files_falls_back_to_the_mask_in_order() {
    let specs: Vec<Spec<'_>> = (0..4)
        .map(|c| Spec {
            cpu: c,
            siblings: ["0", "1", "2", "3"][c],
            l3: None,
        })
        .collect();
    let t = tree(&specs);
    assert_eq!(
        auto_order(&read_topology(&t.0, &[2, 0, 3, 1])),
        vec![0, 1, 2, 3]
    );
    // No tree at all.
    let missing = Path::new("/nonexistent/cubarium/cpu");
    assert_eq!(auto_order(&read_topology(missing, &[5, 4])), vec![4, 5]);
}

#[test]
fn the_live_loop_keeps_to_the_chiplet_with_most_of_the_mask_the_larger_l3_on_a_tie() {
    let t = two_chiplets("98304K", "32768K");
    assert_eq!(
        chiplet_plan(&t.0, &range(0, 32)),
        Some(cat(&[range(0, 8), range(16, 24)])),
        "the whole 9950X3D: the V-cache chiplet"
    );
    let t2 = two_chiplets("32768K", "98304K");
    assert_eq!(
        chiplet_plan(&t2.0, &range(0, 32)),
        Some(cat(&[range(8, 16), range(24, 32)])),
        "the larger L3 wherever it is"
    );
    assert_eq!(
        chiplet_plan(&t.0, &cat(&[vec![0, 1], range(8, 16), range(24, 32)])),
        Some(cat(&[range(8, 16), range(24, 32)])),
        "a mask mostly on the small chiplet stays there"
    );
    assert_eq!(
        chiplet_plan(&t.0, &cat(&[range(0, 8), range(16, 24)])),
        None,
        "already one chiplet: nothing to do"
    );
    // One L3, and no L3 at all.
    let sib: Vec<String> = (0..12)
        .map(|c| format!("{},{}", c % 6, c % 6 + 6))
        .collect();
    let one: Vec<Spec<'_>> = (0..12)
        .map(|c| Spec {
            cpu: c,
            siblings: &sib[c],
            l3: Some(("0-11", "32768K")),
        })
        .collect();
    let t = tree(&one);
    assert_eq!(chiplet_plan(&t.0, &range(0, 12)), None);
    assert_eq!(
        chiplet_plan(Path::new("/nonexistent/cubarium/cpu"), &[0, 1]),
        None
    );
}
