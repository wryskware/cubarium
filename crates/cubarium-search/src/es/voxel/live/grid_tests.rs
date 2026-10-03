//! The lever grid's decode, resume and summary.

use cubarium_voxel_sim::params::Params;

use super::grid::*;

/// `glob` (`*` any one segment) matches `path`.
fn matches(glob: &str, path: &str) -> bool {
    let (g, p): (Vec<&str>, Vec<&str>) = (glob.split('.').collect(), path.split('.').collect());
    g.len() == p.len() && g.iter().zip(&p).all(|(a, b)| *a == "*" || a == b)
}

fn dump(p: &Params) -> Vec<(String, f64)> {
    p.dump()
        .expect("dump")
        .into_iter()
        .filter_map(|l| l.value.parse::<f64>().ok().map(|v| (l.path, v)))
        .collect()
}

/// `m` and `p` scale exactly their listed paths (and motor through its tie), every
/// nonzero leaf there by the factor, and nothing else.
#[test]
fn m_and_p_scale_exactly_their_paths() {
    let axes = [Axis::named("m", &[2.0]).unwrap(), Axis::named("p", &[0.5]).unwrap()];
    let cell = &cells(&axes)[0];
    assert_eq!(cell.label, "m=2 p=0.5");
    let scaled = cell_ecology(cell, None).expect("decodes").params().unwrap();
    let base = Params::defaults();
    let (before, after) = (dump(&base), dump(&scaled));
    assert_eq!(before.len(), after.len());
    let m_globs: Vec<&str> = M_PATHS.iter().chain(&M_FOLLOWS).copied().collect();
    let (mut m_hits, mut p_hits) = (0, 0);
    let mut changed_paths = Vec::new();
    for ((path, b), (path2, a)) in before.iter().zip(&after) {
        assert_eq!(path, path2);
        let factor = if m_globs.iter().any(|g| matches(g, path)) {
            if *b != 0.0 {
                m_hits += 1;
            }
            2.0
        } else if P_PATHS.iter().any(|g| matches(g, path)) {
            if *b != 0.0 {
                p_hits += 1;
            }
            0.5
        } else {
            1.0
        };
        let want = b * factor;
        assert!(
            *a == want || (a - want).abs() <= 1e-12 * want.abs(),
            "{path}: {b} -> {a}, want x{factor}"
        );
        if a != b {
            changed_paths.push(path.clone());
        }
    }
    assert!(m_hits >= 2, "m moved {m_hits} leaves");
    assert!(p_hits >= 8, "p moved {p_hits} leaves");
    // Every listed path names something that moved.
    for g in M_PATHS.iter().chain(&M_FOLLOWS).chain(&P_PATHS) {
        assert!(changed_paths.iter().any(|p| matches(g, p)), "{g} moved nothing");
    }
}

/// A generic axis's overrides set and multiply exactly the paths they name.
#[test]
fn a_generic_axis_sets_and_scales_its_paths() {
    let a = Axis::parse("lit:flora.decomposition=x3;fauna.founding.starter_litter_organic=0.7").unwrap();
    let cell = &cells(&[a])[0];
    let p = cell_ecology(cell, None).expect("decodes").params().unwrap();
    let base = Params::defaults();
    let changed: Vec<String> = dump(&base)
        .into_iter()
        .zip(dump(&p))
        .filter(|((_, b), (_, a))| a != b)
        .map(|((path, _), _)| path)
        .collect();
    for c in &changed {
        // Dead wood and carrion rot follow the litter by the flora's ties.
        assert!(
            [
                "flora.decomposition",
                "flora.wood_decomposition",
                "flora.carrion_decomposition",
                "fauna.founding.starter_litter_organic"
            ]
            .contains(&c.as_str()),
            "{c} moved"
        );
    }
    assert!(changed.iter().any(|c| c == "flora.decomposition"));
    assert_eq!(p.fauna.founding.starter_litter_organic, 0.7);
    assert!(Axis::parse("q:2").is_err(), "a bare number needs a named scale");
}

fn record(cell: &str, seed: u64, b: Option<f64>, r: Option<f64>, alive: usize) -> CellRecord {
    CellRecord {
        cell: cell.into(),
        world_seed: seed,
        hours: 2.0,
        accepted: true,
        lineages: vec![LineageRecord {
            name: "frondgrazer".into(),
            founded: 6,
            alive_120: Some(alive),
            alive_end: alive,
            b,
            r_hat: r,
            ..Default::default()
        }],
        species: vec![SpeciesRecord {
            name: "springturf".into(),
            stands_start: 10,
            stands_end: alive,
            foliage_start: 1.0,
            foliage_end: alive as f64,
        }],
        ..Default::default()
    }
}

/// A rerun skips every cell x seed booked at the same hours, through the file.
#[test]
fn resume_skips_finished_cells() {
    let axes = [Axis::named("m", &[0.5, 1.0]).unwrap(), Axis::named("p", &[1.0]).unwrap()];
    let all = cells(&axes);
    let dir = std::env::temp_dir().join(format!("grid-resume-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("cells.jsonl");
    let mut text = String::new();
    for r in [record("m=0.5 p=1", 3, Some(1.0), None, 4), record("m=1 p=1", 3, Some(1.0), None, 4)] {
        text.push_str(&serde_json::to_string(&r).unwrap());
        text.push('\n');
    }
    let mut other_hours = record("m=1 p=1", 5, None, None, 0);
    other_hours.hours = 1.0;
    text.push_str(&serde_json::to_string(&other_hours).unwrap());
    text.push_str("\n{\"cell\": \"torn");
    std::fs::write(&path, text).unwrap();
    let done = read_records(&path);
    assert_eq!(done.len(), 3, "the torn line is dropped");
    let jobs = pending(&all, &[3, 5], 2.0, &done);
    assert_eq!(jobs, vec![(0, 5), (1, 5)]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The summary of hand-built records: medians and ranges over seeds, missing values left
/// out, and the matrix shows them.
#[test]
fn the_summary_of_hand_built_records() {
    let axes = [Axis::named("m", &[0.5, 1.0]).unwrap(), Axis::named("p", &[1.0, 2.0]).unwrap()];
    let all = cells(&axes);
    let records = vec![
        record("m=0.5 p=1", 1, Some(0.8), Some(0.1), 2),
        record("m=0.5 p=1", 2, Some(1.2), None, 6),
        record("m=0.5 p=1", 3, Some(1.0), Some(0.3), 4),
        record("m=1 p=2", 1, Some(0.5), Some(0.2), 1),
        record("m=1 p=2", 2, None, Some(0.4), 3),
    ];
    let rows = summarise(&all, &records);
    let get = |cell: &str, kind: &str, metric: &str| {
        rows.iter().find(|r| r.cell == cell && r.kind == kind && r.metric == metric).cloned().unwrap()
    };
    let b = get("m=0.5 p=1", "lineage", "B");
    assert_eq!((b.n, b.median, b.min, b.max), (3, 1.0, 0.8, 1.2));
    let r = get("m=0.5 p=1", "lineage", "r_hat");
    assert_eq!((r.n, r.median), (2, 0.2));
    let a = get("m=1 p=2", "lineage", "alive_120");
    assert_eq!((a.n, a.median, a.min, a.max), (2, 2.0, 1.0, 3.0));
    let b2 = get("m=1 p=2", "lineage", "B");
    assert_eq!((b2.n, b2.median), (1, 0.5));
    let f = get("m=0.5 p=1", "species", "foliage_end");
    assert_eq!(f.median, 4.0);
    assert!(rows.iter().all(|r| r.cell != "m=1 p=1"), "an unrun cell has no rows");
    let tsv = summary_tsv(&axes, &rows);
    assert!(tsv.starts_with("cell\tm\tp\tkind\tname\tmetric\tn\tmedian\tmin\tmax\n"));
    assert!(tsv.contains("m=0.5 p=1\t0.5\t1\tlineage\tfrondgrazer\tB\t3\t1.000000\t0.800000\t1.200000\n"));
    let md = summary_md(&axes, &rows, "h");
    assert!(md.contains("| m \\ p | 1 | 2 |"), "{md}");
    assert!(md.contains("| 0.5 | 1.00 / 0.20 | · |"), "{md}");
    assert!(md.contains("| 1 | · | 0.50 / 0.30 |"), "{md}");
}
