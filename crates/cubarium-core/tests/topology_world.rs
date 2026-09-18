//! A ring world through `cubarium-core`: the config surface, the extended `validate`, the two
//! core target resolvers, and the height/`up`/`downhill` the ecology reads.
//!
//! FW-2's own coverage of `design/flat-world-plan-2026-09-16.md` §4 and §5. FW-6 owns the
//! independent battery at `tests/ring_*.rs`; nothing here is written against those files.

use cubarium_core::care::CareTarget;
use cubarium_core::config::CONFIG_VERSION;
use cubarium_core::hunter::HunterTarget;
use cubarium_core::snapshot::{SCHEMA_VERSION, SnapshotError, decode_snapshot, encode_snapshot};
use cubarium_core::{World, WorldConfig};
use cubarium_surface::{CellId, Face, Scale, SurfacePoint, Topology};

const RING: Topology = Topology::Ring { w: 320, h: 180 };

fn ring_config() -> WorldConfig {
    WorldConfig {
        topology: RING,
        ..WorldConfig::default()
    }
}

// ---------------------------------------------------------------- the config surface

#[test]
fn the_config_carries_the_topology_and_the_scale_and_validates_them() {
    let d = WorldConfig::default();
    assert_eq!(
        d.topology,
        Topology::Cube,
        "a default world is still a cube"
    );
    assert_eq!(d.world_scale, Scale::ONE);
    assert_eq!(d.version, CONFIG_VERSION);
    assert_eq!(CONFIG_VERSION, 9);
    d.validate().expect("the default world is valid");

    ring_config().validate().expect("320x180 at S = 1 is valid");
    WorldConfig {
        topology: Topology::Ring { w: 640, h: 360 },
        world_scale: Scale::new(2.0),
        ..WorldConfig::default()
    }
    .validate()
    .expect("640x360 at S = 2 is valid");

    // A cube is pinned to S = 1: its 32-pixel radius and 9-pixel budget are proofs.
    let err = WorldConfig {
        world_scale: Scale::new(2.0),
        ..WorldConfig::default()
    }
    .validate()
    .expect_err("a cube at S = 2 must be refused");
    assert!(err.contains("world_scale"), "{err}");

    // A ring whose sides are not whole cells, and one too narrow for two images.
    for (w, h) in [(321u16, 180u16), (320, 181)] {
        let err = WorldConfig {
            topology: Topology::Ring { w, h },
            ..WorldConfig::default()
        }
        .validate()
        .expect_err("{w}x{h} must be refused");
        assert!(err.contains("whole"), "{err}");
    }
    let err = WorldConfig {
        topology: Topology::Ring { w: 8, h: 8 },
        ..WorldConfig::default()
    }
    .validate()
    .expect_err("an 8x8 ring must be refused");
    assert!(err.contains("third image"), "{err}");
}

#[test]
fn the_world_reports_its_own_topology_scale_and_cell_count() {
    let world = World::new(ring_config()).expect("a ring world");
    assert_eq!(world.topology(), RING);
    assert_eq!(world.scale(), Scale::ONE);
    // 320/4 by 180/4.
    assert_eq!(world.cell_count(), 80 * 45);
    assert_eq!(world.state.fields.n.len(), 3600);
    assert_eq!(world.state.ecology.wood.len(), 3600);
    assert_eq!(world.render_view().topology, RING);
    assert_eq!(world.render_view().scale, Scale::ONE);
    assert_eq!(world.render_view().producer.len(), 3600);
    assert_eq!(world.field_dump().organisms.len(), 3600);
    assert_eq!(world.cell_neighbors().len(), 3600);

    let cube = World::new(WorldConfig::default()).expect("a cube world");
    assert_eq!(cube.topology(), Topology::Cube);
    assert_eq!(cube.cell_count(), 1280);
    assert_eq!(cube.render_view().topology, Topology::Cube);
}

// ---------------------------------------------------------------- persistence, §4

#[test]
fn a_ring_world_round_trips_and_an_older_schema_is_refused_by_name() {
    let mut world = World::new(ring_config()).expect("a ring world");
    for _ in 0..20 {
        world.step();
    }
    let bytes = encode_snapshot(&world.state, "ring");
    let (meta, back) = decode_snapshot(&bytes).expect("a ring world round-trips");
    assert_eq!(meta.schema, SCHEMA_VERSION);
    assert_eq!(back.config.topology, RING);
    assert_eq!(back, world.state);

    for old in 7..SCHEMA_VERSION {
        let mut relabelled = bytes.clone();
        relabelled[4..8].copy_from_slice(&old.to_le_bytes());
        assert_eq!(
            decode_snapshot(&relabelled),
            Err(SnapshotError::UnsupportedSchema(old)),
            "schema {old} must be refused by name"
        );
    }
}

#[test]
fn validate_refuses_a_state_whose_shape_disagrees_with_its_topology() {
    let world = World::new(ring_config()).expect("a ring world");

    // A field vector counted for another surface.
    let mut short = world.state.clone();
    short.fields.n.truncate(1280);
    let err = short.validate().expect_err("a 1,280-cell field on a ring");
    assert!(err.contains("expected 3600"), "{err}");

    // An ecology pool likewise: §4 names them as per-cell serialized vectors too.
    let mut pool = world.state.clone();
    pool.ecology.wood.push(0.0);
    let err = pool.validate().expect_err("an over-long wood vector");
    assert!(err.contains("ecology.wood"), "{err}");

    // An organism on a chart the topology does not have.
    let mut stray = world.state.clone();
    let (id, _) = stray.organisms.iter().next().expect("a founder");
    stray.organisms.get_mut(id).expect("live").pos = SurfacePoint::new(Face::Top, 1.0, 1.0);
    let err = stray
        .validate()
        .expect_err("an organism on Face::Top of a ring");
    assert!(err.contains("chart"), "{err}");

    // And a cube state carrying a ring's config is refused for the same reason.
    let cube = World::new(WorldConfig::default()).expect("a cube world");
    let mut mixed = cube.state.clone();
    mixed.config.topology = RING;
    let err = mixed
        .validate()
        .expect_err("1,280 cells declared as a ring");
    assert!(err.contains("expected 3600"), "{err}");
}

#[test]
fn a_persisted_shower_is_range_checked_against_the_runtime_cell_count() {
    let mut world = World::new(ring_config()).expect("a ring world");
    world.state.care.admitted_seq = 1;
    world
        .state
        .care
        .showers
        .push(cubarium_core::care::ActiveShower {
            seq: 1,
            apply_after_tick: 0,
            // 2,000 is past the cube's 1,280 and inside the ring's 3,600: the check is the
            // world's own count, not a constant.
            cells: vec![2000],
            weights: vec![1.0],
            delivered: 0,
            dose_permille: cubarium_core::care::CareDose::STANDARD_PERMILLE,
        });
    world
        .state
        .validate()
        .expect("cell 2,000 is on a 3,600-cell ring");

    world.state.care.showers[0].cells = vec![3600];
    let err = world
        .state
        .validate()
        .expect_err("cell 3,600 is off the ring");
    assert!(err.contains("out of range"), "{err}");
}

// ---------------------------------------------------------------- §5's two core resolvers

/// Plan §5 / repair 3 finding 2: feed, rain, clean and apex targets beyond pixel 63 resolve on
/// a ring, and anything off the world is refused.
#[test]
fn care_and_hunter_targets_resolve_past_pixel_sixty_three_on_a_ring() {
    let front = Face::Front.index() as u8;
    let top = Face::Top.index() as u8;

    // Past 63 on a ring: a real cell, at the column and row the coordinates name.
    let t = CareTarget {
        face: front,
        u: 200.0,
        v: 170.0,
    };
    let cell = t
        .resolve(RING, Scale::ONE)
        .expect("200,170 is on a 320x180 ring");
    assert_eq!(cell.cx(RING, Scale::ONE), 50);
    assert_eq!(cell.cy(RING, Scale::ONE), 42);
    assert_eq!(cell, CellId::new(RING, Scale::ONE, Face::Front, 50, 42));
    // The same target on a cube is off the chart and refused.
    assert_eq!(t.resolve(Topology::Cube, Scale::ONE), None);

    // At S = 2 the same pixel is half as far into the world, so it lands in cell 25.
    let big = Topology::Ring { w: 640, h: 360 };
    let s2 = Scale::new(2.0);
    let cell = t.resolve(big, s2).expect("200,170 is on a 640x360 ring");
    assert_eq!(cell.cx(big, s2), 25);
    assert_eq!(cell.cy(big, s2), 21);

    // Off the world, in both directions and on a chart the ring does not have.
    for bad in [
        CareTarget {
            face: front,
            u: 320.0,
            v: 10.0,
        },
        CareTarget {
            face: front,
            u: 10.0,
            v: 180.0,
        },
        CareTarget {
            face: front,
            u: -0.5,
            v: 10.0,
        },
        CareTarget {
            face: front,
            u: f64::NAN,
            v: 10.0,
        },
        CareTarget {
            face: top,
            u: 10.0,
            v: 10.0,
        },
        CareTarget {
            face: 9,
            u: 10.0,
            v: 10.0,
        },
    ] {
        assert_eq!(bad.resolve(RING, Scale::ONE), None, "{bad:?} resolved");
    }

    // `HunterTarget` is the apex half of the same rule, and returns a point rather than a cell.
    let h = HunterTarget {
        face: front,
        u: 200.0,
        v: 170.0,
    };
    let p = h.resolve(RING).expect("an apex target past pixel 63");
    assert_eq!(p, SurfacePoint::new(Face::Front, 200.0, 170.0));
    assert_eq!(h.resolve(Topology::Cube), None);
    for bad in [
        HunterTarget {
            face: front,
            u: 320.0,
            v: 10.0,
        },
        HunterTarget {
            face: top,
            u: 10.0,
            v: 10.0,
        },
    ] {
        assert_eq!(bad.resolve(RING), None, "{bad:?} resolved");
    }

    // The cube's own answers are untouched.
    assert_eq!(
        CareTarget {
            face: front,
            u: 63.9,
            v: 63.9
        }
        .resolve(Topology::Cube, Scale::ONE),
        Some(CellId::new(Topology::Cube, Scale::ONE, Face::Front, 15, 15))
    );
    assert_eq!(
        CareTarget {
            face: front,
            u: 64.0,
            v: 1.0
        }
        .resolve(Topology::Cube, Scale::ONE),
        None
    );
}

// ---------------------------------------------------------------- §5's height and gravity

/// Height is a scalar the panel reads as a side view: 1 at the top row, −1 at the bottom
/// edge. `embed` is *position* and is a different function on a ring — that separation is the
/// whole point of §5a. The matching `up` is checked in
/// `world::tests::the_depth_term_points_up_the_side_faces_and_vanishes_on_top`, where the
/// crate-private `up_direction` is reachable.
#[test]
fn height_falls_with_v_and_is_not_the_embedding() {
    assert_eq!(RING.height(&SurfacePoint::new(Face::Front, 0.0, 0.0)), 1.0);
    assert_eq!(RING.height(&SurfacePoint::new(Face::Front, 0.0, 90.0)), 0.0);
    assert_eq!(
        RING.height(&SurfacePoint::new(Face::Front, 0.0, 180.0)),
        -1.0
    );
    // Independent of `u`: the wrap is level.
    for u in [0.0, 80.0, 319.0] {
        assert_eq!(
            RING.height(&SurfacePoint::new(Face::Front, u, 45.0)),
            RING.height(&SurfacePoint::new(Face::Front, 0.0, 45.0))
        );
    }
    // And it is genuinely not the embedding's `y`, which is in embedded units, not [-1, 1].
    let p = SurfacePoint::new(Face::Front, 0.0, 0.0);
    assert!((RING.embed(Scale::ONE, &p)[1] - 2.8125).abs() < 1e-12);

    // The habitat reads the scalar, and it shows in the world: with a positive
    // `light_height_gain` the canopy row grows and the soil row does not.
    let world = World::new(ring_config()).expect("a ring world");
    let row = |r: usize, v: &[f64]| -> f64 { (0..80).map(|c| v[r * 80 + c]).sum() };
    assert!(
        row(0, &world.state.ecology.wood) > row(44, &world.state.ecology.wood),
        "the canopy row must start with more wood than the soil row"
    );
}
