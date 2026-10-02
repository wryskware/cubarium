//! **Descents that freeze, and founding sites a body cannot leave**
//! (`design/handoffs/browser-stuck-in-crags-2026-10-01.md`, fixes 1 and 2 as Wrysk
//! approved them, 2026-10-01).
//!
//! Written from the spec before the implementation:
//! - (a) a step down beside a column that is within the climb from the upper layer but
//!   out of it from the lower one is refused: the body stays on the upper layer, and can
//!   still turn and walk away;
//! - (b) a plain one-riser descent and a plain ascent still succeed;
//! - (c) founding refuses a face where the body's disc overlaps a wall, and a face in a
//!   small pocket ([`crate::founding_faces`]).
//!
//! Hand-built worlds on the 0.125 m grid, where a browser's disc is 1.5 voxels in
//! radius, its headroom 3 voxels and its step 3 voxels both ways. A few ticks each.

use crate::body::{StepLimits, resolve_motion};
use crate::controller::Actions;
use crate::manifest::Founder;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::Site;

const V: f64 = 0.125;

/// An empty `width × 16 × depth` world at 0.125 m with soil from layer 1 up to
/// `top(x, z)` in every column: that column's support face is `top`.
fn world(width: u32, depth: u32, top: impl Fn(u32, u32) -> u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 16,
        depth,
        voxel_m: V,
        ..VoxelConfig::default()
    });
    for x in 0..width {
        for z in 0..depth {
            for y in 1..=top(x, z) {
                w.apply(WorldCommand::SetMaterial {
                    x: i64::from(x),
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

fn browser() -> (crate::FounderPhysiology, crate::Body, crate::Manifest) {
    let phys = crate::FounderPhysiology::placeholder(Founder::Browser);
    (phys, phys.adult_body(), Founder::Browser.manifest())
}

/// Hold `actions` for `ticks` ticks; the standing layer after each, in order.
fn drive(world: &World, pose: &mut crate::Pose, y: &mut u32, actions: Actions, ticks: u32) -> Vec<u32> {
    let (phys, body, manifest) = browser();
    let view = world.view();
    let limits: StepLimits = phys.step_limits(V);
    let mut layers = Vec::new();
    for _ in 0..ticks {
        resolve_motion(&view, pose, y, &manifest, &body, phys.wade_depth_m(&body), limits, actions);
        layers.push(*y);
    }
    layers
}

const FORWARD: Actions = Actions {
    forward: 1.0,
    turn: 0.0,
    feed: 0.0,
};
const TURN: Actions = Actions {
    forward: 0.0,
    turn: 1.0,
    feed: 0.0,
};

/// The fixture's geometry is what the test means: a 3-voxel step, a pillar 2 above the
/// upper layer (5 above the lower one, so out of the climb from there).
#[test]
fn the_fixture_is_the_one_the_spec_describes() {
    let (phys, body, _) = browser();
    let s = phys.step_limits(V);
    assert_eq!((s.up, s.down), (3, 3));
    assert_eq!(body.headroom_voxels(V), 3);
    assert!((body.footprint_radius() / V - 1.5).abs() < 1e-9);
}

/// (a) Ground at 3, a ledge at 6 over `x < 8` (and `x >= 16`, so the strip's wrap on
/// `x` continues the ledge westward), and beside the way down (`x` 8..11,
/// `z` 0..=1) a pillar topped at 8: within the climb of the ledge, out of it from the
/// ground below. A browser walking east off the ledge with its disc over the pillar's
/// edge would land on 3 with the pillar's solid inside its headroom and nowhere to go.
/// The step down is refused; it stays on 6, turns, and walks back west.
#[test]
fn a_step_down_that_would_leave_the_disc_in_a_wall_is_refused() {
    let w = world(24, 8, |x, z| {
        if x < 8 || x >= 16 {
            6
        } else if x < 11 && z <= 1 {
            8
        } else {
            3
        }
    });
    let mut pose = crate::Pose {
        x: 5.5 * V,
        z: 3.0 * V,
        heading_rad: std::f64::consts::FRAC_PI_2,
    };
    let mut y = 6;
    let east = drive(&w, &mut pose, &mut y, FORWARD, 60);
    assert!(
        east.iter().all(|&l| l == 6),
        "the body stepped down beside the pillar: {east:?}"
    );
    let stopped_x = pose.x;
    assert!(stopped_x > 6.5 * V, "it did walk up to the edge: {stopped_x}");

    // It can turn about ...
    let h0 = pose.heading_rad;
    drive(&w, &mut pose, &mut y, TURN, 32);
    assert!(
        (pose.heading_rad - h0).abs() > 3.0,
        "it turned: {h0} -> {}",
        pose.heading_rad
    );
    // ... and walk away on the upper layer.
    let west = drive(&w, &mut pose, &mut y, FORWARD, 30);
    assert!(west.iter().all(|&l| l == 6), "{west:?}");
    // West on the wrapped strip: the distance from where it stopped, modulo the width.
    let walked = (stopped_x - pose.x).rem_euclid(24.0 * V);
    assert!(
        walked > 4.0 * V && walked < 12.0 * V,
        "it walked back west from {stopped_x} to {}",
        pose.x
    );
}

/// (b) A plain 3-voxel riser, nothing beside it: a browser walks east off the ledge onto
/// the ground and on, and back west up onto the ledge.
#[test]
fn a_plain_riser_is_still_a_step_down_and_a_step_up() {
    let w = world(24, 8, |x, _| if x < 8 { 6 } else { 3 });
    let mut pose = crate::Pose {
        x: 4.5 * V,
        z: 4.0 * V,
        heading_rad: std::f64::consts::FRAC_PI_2,
    };
    let mut y = 6;
    let east = drive(&w, &mut pose, &mut y, FORWARD, 40);
    assert_eq!(*east.last().unwrap(), 3, "it got down: {east:?}");
    assert!(pose.x > 13.0 * V, "and walked on: {}", pose.x);

    pose.heading_rad = -std::f64::consts::FRAC_PI_2;
    let west = drive(&w, &mut pose, &mut y, FORWARD, 40);
    assert_eq!(*west.last().unwrap(), 6, "it got back up: {west:?}");
    assert!(pose.x < 6.0 * V, "and walked on: {}", pose.x);
}

/// (c) Ground at 3 over a 32 × 16 strip, and a 4 × 4 mesa topped at 8 (5 above the
/// ground: a wall from below, and its top a pocket nothing walks onto or off). Founding
/// takes a face out on the open ground, refuses one beside the mesa where the browser's
/// disc would overlap its side, and refuses the mesa's top.
#[test]
fn founding_refuses_a_disc_in_a_wall_and_a_small_pocket() {
    let w = world(32, 16, |x, z| if (10..14).contains(&x) && (6..10).contains(&z) { 8 } else { 3 });
    let (phys, _, _) = browser();
    let faces = crate::founding_faces(&w.view(), &phys);
    let has = |x, y, z| faces.contains(&Site { x, y, z });
    assert!(has(25, 3, 8), "an open face is a founding face");
    assert!(!has(9, 3, 8), "beside the mesa the disc overlaps its side");
    assert!(!has(12, 8, 8), "the mesa's top is a pocket");
    assert!(!has(11, 8, 7), "anywhere on it");
}
