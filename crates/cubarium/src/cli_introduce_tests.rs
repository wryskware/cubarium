//! P1 (predation substrate), tests first: the `--introduce lanternjaw:N` development
//! control (`design/predation-and-flight-2026-09-28.md` §2.9; brief P1 item 8 and case
//! 13). Red by design until the implementation commit adds, in `cli.rs`:
//!
//! - `Voxel::introduce: Vec<String>` behind `--introduce <LINEAGE:N>` (repeatable, the
//!   ambient run's), and `Voxel::introductions(&self) -> anyhow::Result<Vec<(Founder,
//!   usize)>>`, which parses it and refuses an unknown lineage or a count that is not a
//!   number, by name;
//!
//! and in the fauna crate `Founder::Lanternjaw`.

use super::*;

fn voxel(args: &[&str]) -> Voxel {
    let mut line = vec!["cubarium", "voxel"];
    line.extend_from_slice(args);
    match Cli::parse_from(line).command {
        Command::Voxel(v) => v,
        other => panic!("expected a voxel command, got {other:?}"),
    }
}

/// `--introduce lanternjaw:N` asks for N adult lanternjaws; nothing asks for none; a bad
/// lineage or count is refused by name.
#[test]
fn introduce_names_a_lineage_and_a_count() {
    assert!(voxel(&[]).introductions().expect("no flag").is_empty());
    assert_eq!(
        voxel(&["--introduce", "lanternjaw:2"])
            .introductions()
            .expect("two lanternjaws"),
        vec![(Founder::Lanternjaw, 2)]
    );
    for (spec, want) in [("wombat:1", "wombat"), ("lanternjaw:x", "lanternjaw:x")] {
        let err = format!(
            "{:#}",
            voxel(&["--introduce", spec])
                .introductions()
                .expect_err(spec)
        );
        assert!(err.contains(want), "{spec}: {err}");
    }
}
