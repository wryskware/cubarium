//! FW-6: the ring world, written from `design/flat-world-plan-2026-09-16.md` §5 (height,
//! founders, the core resolvers), §5a (the embedding and RNG parity) and §9's FW-2 row
//! ("feed/rain/clean/apex targets beyond pixel 63 resolve on a ring and are refused
//! off-world"; "RNG stream parity (expected exact — no draw changes)").

use cubarium_core::care::CareTarget;
use cubarium_core::config::WorldConfig;
use cubarium_core::hunter::HunterTarget;
use cubarium_core::rng::{Stream, unit};
use cubarium_core::world::World;
use cubarium_surface::{Face, Scale, SurfacePoint, Topology, Vec2, cell_of};

const W: u16 = 320;
const H: u16 = 180;
const SEED: u64 = 20260916;

fn ring() -> Topology {
    Topology::Ring { w: W, h: H }
}

fn ring_config() -> WorldConfig {
    let mut cfg = WorldConfig { seed: SEED, ..WorldConfig::default() };
    cfg.topology = ring();
    cfg.world_scale = Scale::ONE;
    cfg
}

fn cube_config() -> WorldConfig {
    WorldConfig { seed: SEED, ..WorldConfig::default() }
}

// ---------------------------------------------------------------------------
// It runs
// ---------------------------------------------------------------------------

#[test]
fn a_ring_world_is_built_from_its_topology_and_steps() {
    let mut world = World::new(ring_config()).expect("a 320×180 ring at S = 1 is a legal world");
    assert_eq!(world.topology(), ring());
    assert_eq!(world.scale(), Scale::ONE);
    assert_eq!(world.cell_count(), 3600, "80 × 45 cells");
    assert!(world.population() > 0, "founders were placed");

    for _ in 0..20 {
        world.step();
    }
    assert_eq!(world.tick(), 20);
    world.check_invariants().expect("invariants hold on a ring world");

    let view = world.render_view();
    assert_eq!(view.topology, ring(), "the view carries the topology to the presenter");
    assert_eq!(view.scale, Scale::ONE);
    assert_eq!(view.producer.len(), 3600, "every per-cell vector is the world's cell count");
    assert_eq!(view.water.len(), 3600);
    assert_eq!(view.rain.len(), 3600);
}

/// A cube world is unchanged and still refuses a world scale other than 1 (§2).
#[test]
fn a_cube_world_still_pins_the_scale_to_one() {
    let world = World::new(cube_config()).expect("the cube is still a legal world");
    assert_eq!(world.topology(), Topology::Cube);
    assert_eq!(world.cell_count(), 1280);

    let mut bad = cube_config();
    bad.world_scale = Scale::new(2.0);
    let err = match World::new(bad) {
        Err(e) => e,
        Ok(_) => panic!("a cube at S = 2 must be refused"),
    };
    assert!(err.to_lowercase().contains("scale"), "the refusal should name the scale: {err}");
}

// ---------------------------------------------------------------------------
// Founders
// ---------------------------------------------------------------------------

#[test]
fn founders_land_on_the_one_chart_inside_its_extent() {
    let world = World::new(ring_config()).expect("legal ring world");
    let view = world.render_view();
    assert!(!view.organisms.is_empty());
    for o in &view.organisms {
        assert_eq!(o.pos.face, Face::Front, "a ring has one chart");
        assert!((0.0..f64::from(W)).contains(&o.pos.u), "u = {} outside [0, {W})", o.pos.u);
        assert!((0.0..f64::from(H)).contains(&o.pos.v), "v = {} outside [0, {H})", o.pos.v);
        assert!(o.pos.is_canonical(ring()));
    }
}

/// §5: "the face draw must be **kept and discarded** or every seed shifts". Written against
/// the draw definition itself: founder `i` reads counters 0..4 of `Stream::Founders` at key
/// `i` (the v1 path, `kinds` empty), whatever the topology, and `(u, v)` scale to the
/// chart's extent. If a draw were added, removed or reordered for the ring, this fails.
#[test]
fn the_founder_draws_are_the_documented_stream_on_both_topologies() {
    for (topo, ext_u, ext_v) in [(Topology::Cube, 64.0, 64.0), (ring(), f64::from(W), f64::from(H))]
    {
        let mut cfg = if matches!(topo, Topology::Cube) { cube_config() } else { ring_config() };
        cfg.topology = topo;
        // The v1 founder path, where the draw key is the founder index. (With `kinds`
        // configured the key folds the kind index into its high bits instead.)
        cfg.founders.kinds.clear();
        cfg.founders.count = 24;
        let world = World::new(cfg).expect("legal world");
        let view = world.render_view();
        for o in &view.organisms {
            let i = u64::from(o.id.slot);
            let want_u = unit(SEED, Stream::Founders, i, 1) * ext_u;
            let want_v = unit(SEED, Stream::Founders, i, 2) * ext_v;
            let want_heading =
                Vec2::from_screen_angle(unit(SEED, Stream::Founders, i, 3) * std::f64::consts::TAU);
            assert!(
                (o.pos.u - want_u).abs() <= 1e-9,
                "{topo:?} founder {i}: u = {} not counter 1 × {ext_u} = {want_u}",
                o.pos.u
            );
            assert!(
                (o.pos.v - want_v).abs() <= 1e-9,
                "{topo:?} founder {i}: v = {} not counter 2 × {ext_v} = {want_v}",
                o.pos.v
            );
            assert!(
                (o.heading.x - want_heading.x).abs() <= 1e-9
                    && (o.heading.y - want_heading.y).abs() <= 1e-9,
                "{topo:?} founder {i}: heading {:?} not counter 3",
                o.heading
            );
        }
    }
}

/// The same seed on the two topologies differs only where it must: the founders sit at the
/// same *fractions* of their chart and carry the same heading and hue.
#[test]
fn the_same_seed_places_the_same_founders_on_a_ring_and_on_a_cube() {
    let ring_world = World::new(ring_config()).expect("legal ring world");
    let cube_world = World::new(cube_config()).expect("legal cube world");
    let (rv, cv) = (ring_world.render_view(), cube_world.render_view());
    assert_eq!(rv.organisms.len(), cv.organisms.len(), "the same founders were placed");

    for (r, c) in rv.organisms.iter().zip(cv.organisms.iter()) {
        assert_eq!(r.id.slot, c.id.slot, "the same slots in the same order");
        assert!(
            (r.pos.u / f64::from(W) - c.pos.u / 64.0).abs() <= 1e-12,
            "founder {}: u fraction {} vs {}",
            r.id.slot,
            r.pos.u / f64::from(W),
            c.pos.u / 64.0
        );
        assert!(
            (r.pos.v / f64::from(H) - c.pos.v / 64.0).abs() <= 1e-12,
            "founder {}: v fraction",
            r.id.slot
        );
        assert_eq!(r.heading, c.heading, "founder {}: heading", r.id.slot);
        assert_eq!(r.hue, c.hue, "founder {}: hue", r.id.slot);
    }
}

// ---------------------------------------------------------------------------
// Height
// ---------------------------------------------------------------------------

/// §5's design call: `height(p) = 1 − 2v/h` on a ring — canopy at the top row, soil at the
/// bottom edge — and §5a: on the cube height *is* `embed()[1]`, which is why the two
/// coincide there and must be asked for separately on a ring.
#[test]
fn ring_height_is_one_minus_two_v_over_h_and_the_cube_keeps_the_embedding() {
    let topo = ring();
    for v in [0.0, 0.5, 45.0, 90.0, 179.5] {
        let p = SurfacePoint::new(Face::Front, 160.5, v);
        let want = 1.0 - 2.0 * v / f64::from(H);
        assert!((topo.height(&p) - want).abs() <= 1e-12, "height at v = {v}");
        // `embed` is a different function on a ring: the cylinder, not the height.
        assert!(
            (topo.embed(Scale::ONE, &p)[1] - want).abs() > 1e-6 || v == 90.0,
            "embed()[1] should not be the height at v = {v}"
        );
    }
    assert_eq!(topo.height(&SurfacePoint::new(Face::Front, 0.0, 0.0)), 1.0, "canopy row");
    assert!(topo.height(&SurfacePoint::new(Face::Front, 0.0, 179.0)) < -0.98, "soil row");

    // On the cube the two are the same function.
    for face in Face::ALL {
        for (u, v) in [(0.5, 0.5), (32.0, 32.0), (63.5, 63.5)] {
            let p = SurfacePoint::new(face, u, v);
            assert!(
                (Topology::Cube.height(&p) - Topology::Cube.embed(Scale::ONE, &p)[1]).abs()
                    <= 1e-12,
                "cube height at {face:?} ({u}, {v})"
            );
        }
    }
}

/// The controller channel: `Observation70[55]` is the height the depth drive steers on
/// (`controller.rs` `obs.up * (w_depth · (h_pref − obs.height))`). On a ring it must be the
/// ring's height, or "a wrong height silently steers the whole population into a wall".
#[test]
fn the_controller_reads_the_ring_height() {
    let mut world = World::new(ring_config()).expect("legal ring world");
    world.step();
    let view = world.render_view();
    let mut checked = 0;
    for o in &view.organisms {
        let Some(obs) = world.neural_observation(o.id) else { continue };
        let want = 1.0 - 2.0 * o.pos.v / f64::from(H);
        assert!(
            (f64::from(obs.0[55]) - want).abs() <= 1e-6,
            "organism {:?} at v = {}: channel 55 = {} but 1 − 2v/h = {want}",
            o.id,
            o.pos.v,
            obs.0[55]
        );
        checked += 1;
    }
    assert!(checked > 0, "no observation was sampled");
}

// ---------------------------------------------------------------------------
// The core resolvers
// ---------------------------------------------------------------------------

/// §9's FW-2 row: feed/rain/clean and apex targets beyond pixel 63 resolve on a ring.
#[test]
fn care_and_apex_targets_beyond_pixel_63_resolve_on_a_ring() {
    let topo = ring();
    for (u, v) in [(64.0, 64.0), (200.0, 120.0), (319.999, 179.999), (0.0, 0.0)] {
        let target = CareTarget { face: 0, u, v };
        let cell = target
            .resolve(topo, Scale::ONE)
            .unwrap_or_else(|| panic!("({u}, {v}) is inside a 320×180 ring"));
        let want = cell_of(topo, Scale::ONE, &SurfacePoint::new(Face::Front, u, v));
        assert_eq!(cell, want, "({u}, {v}) resolves to its own cell");
        assert!(cell.index() < 3600);

        let apex = HunterTarget { face: 0, u, v };
        let p = apex.resolve(topo).unwrap_or_else(|| panic!("apex at ({u}, {v})"));
        assert!(p.is_canonical(topo), "the apex spawn point is canonical");
        assert_eq!(p.face, Face::Front);
    }
    // The cube is unchanged: 64 is off the chart there.
    assert_eq!(CareTarget { face: 0, u: 64.0, v: 10.0 }.resolve(Topology::Cube, Scale::ONE), None);
}

#[test]
fn off_world_care_and_apex_targets_are_refused() {
    let topo = ring();
    let bad = [
        (0u8, 320.0, 90.0),  // u at the extent
        (0, -0.001, 90.0),   // u below zero
        (0, 160.0, 180.0),   // v at the extent
        (0, 160.0, -1.0),    // v below zero
        (1, 160.0, 90.0),    // Right: not a chart of a ring
        (4, 10.0, 10.0),     // Top: not a chart of a ring
        (9, 10.0, 10.0),     // not a face at all
        (0, f64::NAN, 90.0), // hostile
        (0, 160.0, f64::INFINITY),
    ];
    for (face, u, v) in bad {
        assert_eq!(
            CareTarget { face, u, v }.resolve(topo, Scale::ONE),
            None,
            "care target (face {face}, {u}, {v}) must be refused"
        );
        assert_eq!(
            HunterTarget { face, u, v }.resolve(topo),
            None,
            "apex target (face {face}, {u}, {v}) must be refused"
        );
    }
}

// ---------------------------------------------------------------------------
// Field reactions cover every ring cell (regression for the cube-shaped loops)
// ---------------------------------------------------------------------------

/// FW-5 found three production loops in `fields.rs` iterating the cube's 1,280
/// cells whatever the topology, so on a ring the row holding cell index 1,280
/// (row 16 at 80 cells across) received the detritus of every cell above it and
/// never shed its own: a permanent dam ~3× its neighbours after 3,000 ticks.
/// After the fix every row's detritus is within a modest factor of its
/// neighbours', and the last ring cell sheds like the first.
#[test]
fn detritus_falls_through_every_ring_row_not_only_the_first_1280_cells() {
    let mut world = World::new(ring_config()).expect("ring world");
    for _ in 0..3_000 {
        world.step();
    }
    let view = world.render_view();
    let cols = (W as usize) / 4;
    let rows = (H as usize) / 4;
    assert_eq!(view.detritus.len(), cols * rows);
    let row_mean =
        |r: usize| view.detritus[r * cols..(r + 1) * cols].iter().sum::<f64>() / cols as f64;
    // The cube's loop bound landed on row 1280 / 80 = 16.
    let dam = 1280 / cols;
    let (above, at, below) = (row_mean(dam - 1), row_mean(dam), row_mean(dam + 1));
    let neighbours = 0.5 * (above + below);
    assert!(
        at < 1.5 * neighbours,
        "row {dam} holds {at:.3} against neighbours {above:.3}/{below:.3}: the dam is back"
    );
    // Rows past the old bound shed too: the second-to-last row is not a sink either.
    let (r1, r2) = (row_mean(rows - 3), row_mean(rows - 2));
    assert!(r2 < 3.0 * r1 + 0.05, "row {} holds {r2:.3} against {r1:.3} above it", rows - 2);
}
