//! FW-6: the presenter on a ring, written from `design/flat-world-plan-2026-09-16.md` §5
//! (bands, columns, seam-carried sprites, the ray-cast preview not ported) and §9's FW-5
//! row ("`RenderView.topology` consumed, `ArtPresenter` built from the world's cell count …
//! cube capture diffed to zero").
//!
//! **Status.** FW-5 has not landed: `Presenter::new` still allocates its field caches at
//! `Topology::Cube`/`Scale::ONE`, so a ring `RenderView` has nowhere to go. What is written
//! here is the cube behaviour FW-5 must not move, plus the ring test itself — written, and
//! `#[ignore]`d until the presenter can take a topology. The rest is a `pending FW-5` list.

use cubarium::present::Presenter;
use cubarium_core::config::WorldConfig;
use cubarium_core::world::World;
use cubarium_render::Canvas;
use cubarium_surface::{Scale, Topology};

const SEED: u64 = 20260916;

fn ring() -> Topology {
    Topology::Ring { w: 320, h: 180 }
}

fn world_of(topo: Topology, ticks: u32) -> World {
    let mut cfg = WorldConfig { seed: SEED, ..WorldConfig::default() };
    cfg.topology = topo;
    let mut world = World::new(cfg).expect("legal world");
    for _ in 0..ticks {
        world.step();
    }
    world
}

fn fnv1a(pixels: &[[f32; 3]]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for p in pixels {
        for c in p {
            for b in c.to_le_bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    h
}

/// The cube presenter draws a real image and draws the same one every time — the property
/// FW-5's "cube capture diffed to zero" rests on. The digest is printed rather than pinned
/// as a literal: it depends on the whole simulation, so a literal here would be a second,
/// weaker copy of the run's own regression evidence.
#[test]
fn the_cube_presenter_draws_the_same_image_twice() {
    let world = world_of(Topology::Cube, 40);
    let view = world.render_view();

    let mut once = Canvas::cube();
    let mut presenter = Presenter::new();
    presenter.observe(&view);
    presenter.draw(&view, 0.0, &mut once);

    let mut twice = Canvas::cube();
    let mut fresh = Presenter::new();
    fresh.observe(&view);
    fresh.draw(&view, 0.0, &mut twice);

    assert_eq!(once.pixels(), twice.pixels(), "the same view draws the same image");
    assert!(once.pixels().iter().any(|p| *p != [0.0; 3]), "the presenter drew something");
    println!("cube presenter digest at tick {}: {:016x}", view.tick, fnv1a(once.pixels()));
}

/// Interpolating between ticks changes the image but not its shape, and a second frame from
/// the same fraction is identical: the frame clock may not introduce state of its own.
#[test]
fn interpolated_frames_are_deterministic_on_the_cube() {
    let world = world_of(Topology::Cube, 40);
    let view = world.render_view();
    let mut presenter = Presenter::new();
    presenter.observe(&view);

    let mut a = Canvas::cube();
    presenter.draw(&view, 0.5, &mut a);
    let mut b = Canvas::cube();
    presenter.draw(&view, 0.5, &mut b);
    assert_eq!(a.pixels(), b.pixels(), "f = 0.5 twice");

    let mut c = Canvas::cube();
    presenter.draw(&view, 0.0, &mut c);
    assert_eq!(c.pixels().len(), a.pixels().len());
}

/// The view a ring world hands the presenter already carries everything FW-5 needs: the
/// topology, the scale, and per-cell vectors of the world's own length. This part of the
/// contract exists today and is pinned so FW-5 can rely on it.
#[test]
fn a_ring_render_view_carries_the_topology_and_the_world_s_cell_count() {
    let world = world_of(ring(), 10);
    let view = world.render_view();
    assert_eq!(view.topology, ring());
    assert_eq!(view.scale, Scale::ONE);
    assert_eq!(view.producer.len(), world.cell_count());
    assert_eq!(view.detritus.len(), world.cell_count());
    assert_eq!(view.water.len(), world.cell_count());
    assert_eq!(view.rain.len(), world.cell_count());
    for o in &view.organisms {
        assert!(o.pos.is_canonical(view.topology), "{:?} is on the ring", o.pos);
    }

    // And a ring canvas is the surface it has to be drawn onto.
    let canvas = Canvas::new(view.topology, view.scale);
    assert_eq!((canvas.width(), canvas.height()), (320, 180));
    assert_eq!(canvas.pixels().len(), 57_600);
}

/// **pending FW-5.** The presenter itself is still cube-shaped: `Presenter::new` allocates
/// `ScalarField::zeros(Topology::Cube, Scale::ONE)`, so copying a 3,600-cell ring field into
/// it cannot work. This is the test for FW-5's first step; remove the `#[ignore]` when the
/// presenter is built from the world's own cell count.
#[test]
#[ignore = "pending FW-5: Presenter::new allocates its caches at Topology::Cube"]
fn the_presenter_draws_a_ring_world() {
    let world = world_of(ring(), 10);
    let view = world.render_view();
    let mut canvas = Canvas::new(view.topology, view.scale);
    let mut presenter = Presenter::new();
    presenter.observe(&view);
    presenter.draw(&view, 0.0, &mut canvas);
    assert!(canvas.pixels().iter().any(|p| *p != [0.0; 3]), "the ring presenter drew something");
}

// --- pending FW-5 -----------------------------------------------------------------
//
// Not written yet, because the interface does not exist in this worktree:
//
// * `ArtPresenter` built from the world's cell count rather than `CUBE_CELL_COUNT`, and the
//   bands read from `Topology::height` with the new `canopy_top` threshold (§5 leaves the
//   value open for Wrysk; the test will pin whatever is decided, not guess it);
// * the horizon, water and rain against the ring's bottom row — §5 expects standing water
//   along it and asks for `evap_floor` to be re-checked;
// * motifs and tall columns chosen along `u` instead of per side face, in a deterministic
//   order that does not depend on `Face::ALL`;
// * bodies and rigs stamped through `unfold_pixels`, clipping only at the two rims and
//   carrying across the wrap with at most two images;
// * the cube capture diffed to zero against the pre-FW-5 presenter, which is the regression
//   evidence §9 asks for and which needs a fixture from before the change;
// * `raycast.rs` staying cube-only, and `PreviewSink` refusing a ring world (FW-4).
