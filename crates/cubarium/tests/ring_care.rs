//! FW-6: the care chain on a ring, written from `design/flat-world-plan-2026-09-16.md` §3
//! (`CareTarget { face: u8, u, v }`, `face > 4` rejected → "validate `face == 0` and
//! `u < w`, `v < h` for a ring"), §4 (the host `CareTarget` is FW-4's, not the core's) and
//! §9's FW-4 row (the widened target, the journal, the web request).
//!
//! **Status.** FW-4 has not landed: `crates/cubarium/src/care/mod.rs` still declares
//! `CareTarget { face: u8, u: u8, v: u8 }` and validates it against the cube's 64. What is
//! written here is that current shape — which FW-4 must keep working for cube worlds — and
//! the ring behaviour of the **core** chain the host will drive. The rest is a
//! `pending FW-4` list at the foot of the file.

use cubarium::care::CareTarget as HostCareTarget;
use cubarium_core::care::{CareCommand, CareKind, CareOutcome, CareTarget};
use cubarium_core::config::WorldConfig;
use cubarium_core::world::World;
use cubarium_surface::{Scale, Topology};

const W: u16 = 320;
const H: u16 = 180;
const SEED: u64 = 20260916;

fn ring() -> Topology {
    Topology::Ring { w: W, h: H }
}

fn ring_world() -> World {
    let mut cfg = WorldConfig {
        seed: SEED,
        ..WorldConfig::default()
    };
    cfg.topology = ring();
    let mut world = World::new(cfg).expect("legal ring world");
    // One step so the fields are settled and the tick is a real boundary.
    world.step();
    world
}

// ---------------------------------------------------------------------------
// What FW-4 inherits
// ---------------------------------------------------------------------------

/// Today's host target, pinned: the cube's five charts and 64 pixels. FW-4 widens `u`/`v` to
/// `u16` and validates against the topology's extent; every assertion here about a **cube**
/// target must still hold afterwards.
#[test]
fn the_host_care_target_is_still_the_cube_chart() {
    assert_eq!(
        HostCareTarget {
            face: 0,
            u: 0,
            v: 0
        }
        .validate(),
        Ok(())
    );
    assert_eq!(
        HostCareTarget {
            face: 4,
            u: 63,
            v: 63
        }
        .validate(),
        Ok(())
    );
    assert!(
        HostCareTarget {
            face: 5,
            u: 0,
            v: 0
        }
        .validate()
        .is_err(),
        "there is no face 5"
    );
    assert!(
        HostCareTarget {
            face: 0,
            u: 64,
            v: 0
        }
        .validate()
        .is_err(),
        "u is 0..63 today"
    );
    assert!(
        HostCareTarget {
            face: 0,
            u: 0,
            v: 64
        }
        .validate()
        .is_err(),
        "v is 0..63 today"
    );
    // The type itself is the thing FW-4 has to widen: `u8` cannot even name pixel 200 of a
    // 320-pixel ring row, let alone 319.
    assert_eq!(
        std::mem::size_of::<HostCareTarget>(),
        6,
        "face + u16 + u16, widened by FW-4"
    );
}

// ---------------------------------------------------------------------------
// The core chain on a ring
// ---------------------------------------------------------------------------

fn apply(world: &mut World, kind: CareKind, u: f64, v: f64) -> CareOutcome {
    let seq = world.care().admitted_seq + 1;
    let cmd = CareCommand::standard(seq, world.tick(), kind, CareTarget { face: 0, u, v });
    world.apply_care(&cmd).outcome
}

/// Feed, rain and clean all land on a ring cell well beyond the cube's pixel 63, and each
/// moves its own ledger.
#[test]
fn feed_rain_and_clean_land_beyond_pixel_63_on_a_ring() {
    let mut world = ring_world();

    let before = world.care().feed_material_in;
    let fed = apply(&mut world, CareKind::Feed, 200.0, 120.0);
    assert!(fed.applied().is_some(), "feed at (200, 120): {fed:?}");
    assert!(
        world.care().feed_material_in > before,
        "the feed ledger moved"
    );

    let before = world.care().rain_depth_in;
    let rained = apply(&mut world, CareKind::Rain, 300.5, 20.5);
    assert!(
        rained.applied().is_some(),
        "rain at (300.5, 20.5): {rained:?}"
    );
    world.step();
    assert!(
        world.care().rain_depth_in > before,
        "the rain ledger moved once a sample fell"
    );

    // Clean takes back what feeding put down, at the same place.
    let before = world.care().clean_material_out;
    let cleaned = apply(&mut world, CareKind::Clean, 200.0, 120.0);
    assert!(
        cleaned.applied().is_some(),
        "clean at (200, 120): {cleaned:?}"
    );
    assert!(
        world.care().clean_material_out > before,
        "the clean ledger moved"
    );

    world
        .check_invariants()
        .expect("care on a ring keeps the world's identities");
}

/// A persisted shower's footprint is raw cell indices, and on a ring those may exceed the
/// cube's 1,280.
#[test]
fn a_ring_shower_may_name_cells_beyond_the_cubes_1280() {
    let mut world = ring_world();
    let outcome = apply(&mut world, CareKind::Rain, 300.5, 120.5);
    assert!(outcome.applied().is_some(), "{outcome:?}");
    let shower = world.care().showers.first().expect("a shower is in flight");
    assert!(!shower.cells.is_empty());
    for c in &shower.cells {
        assert!(
            usize::from(*c) < world.cell_count(),
            "cell {c} is inside the world"
        );
    }
    // (300.5, 120.5) is column 75 of row 30, cell 2,475 — an index the cube's 1,280 cells
    // could never name, and one a leftover `CELL_COUNT` bound would have refused.
    assert!(
        shower.cells.iter().any(|c| usize::from(*c) >= 1280),
        "a shower at v = 120.5 must touch cells past the cube's 1,280: {:?}",
        shower.cells
    );
    world
        .state
        .validate()
        .expect("the persisted shower validates against the ring");
}

/// Off-world targets are refused, not clamped onto some other cell.
#[test]
fn off_world_care_is_refused_on_a_ring() {
    let mut world = ring_world();
    for (u, v) in [
        (320.0, 90.0),
        (-1.0, 90.0),
        (160.0, 180.0),
        (160.0, -0.5),
        (f64::NAN, 1.0),
    ] {
        let outcome = apply(&mut world, CareKind::Feed, u, v);
        assert!(
            matches!(outcome, CareOutcome::Rejected(_)),
            "({u}, {v}) must be refused, got {outcome:?}"
        );
    }
    // A chart the ring does not have is refused too, whatever the coordinates.
    let seq = world.care().admitted_seq + 1;
    let cmd = CareCommand::standard(
        seq,
        world.tick(),
        CareKind::Feed,
        CareTarget {
            face: 4,
            u: 10.0,
            v: 10.0,
        },
    );
    assert!(
        matches!(world.apply_care(&cmd).outcome, CareOutcome::Rejected(_)),
        "Face::Top"
    );
}

/// The cube is unchanged: the same commands still work there, and the same bounds still
/// refuse.
#[test]
fn the_cube_care_chain_is_unchanged() {
    let mut world = World::new(WorldConfig {
        seed: SEED,
        ..WorldConfig::default()
    })
    .expect("legal cube world");
    world.step();
    assert_eq!(world.topology(), Topology::Cube);
    assert_eq!(world.scale(), Scale::ONE);

    let outcome = apply(&mut world, CareKind::Feed, 32.0, 32.0);
    assert!(
        outcome.applied().is_some(),
        "feed at the middle of Front: {outcome:?}"
    );
    let outcome = apply(&mut world, CareKind::Feed, 64.0, 32.0);
    assert!(
        matches!(outcome, CareOutcome::Rejected(_)),
        "u = 64 is off a cube chart"
    );
}

// --- pending FW-4 -----------------------------------------------------------------
//
// Not written yet, because the interface does not exist in this worktree:
//
// * `cubarium::care::CareTarget.{u, v}` widened to `u16` and validated against the
//   topology's extent — `face == 0`, `u < w`, `v < h` for a ring; the five cube charts and
//   64 pixels unchanged for a cube;
// * `PlannedCommand` journal compatibility: a journal written before the widening still
//   replays, and a replayed command lands on the same cell it landed on before;
// * the web request shape for a target that needs more than a byte per coordinate;
// * the canvas flourish for a ring world;
// * `--sink preview` refused for a ring at argument validation, and `open_world` refusing a
//   resume whose config topology differs from the snapshot's.
