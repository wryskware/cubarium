//! **Wall slide** (`design/handoffs/browser-stuck-in-crags-2026-10-01.md`, fix 4 as
//! Wrysk approved it, 2026-10-02).
//!
//! Written from the spec before the implementation. A forward request refused by a wall
//! keeps its component along the wall, scaled by `1 − wall_friction`:
//! - a body walking into a straight wall at 45° slides along it, at about `cos 45°` of
//!   its request with friction 0, and does not slide with friction 1 (the old motor);
//! - a body walking into a wall head-on does not move sideways;
//! - a body sliding along a wall reaches an opening in it and walks through.
//!
//! Hand-built worlds on the 0.125 m grid (a browser's disc is 1.5 voxels in radius, its
//! step 3 voxels): ground at 3 and a wall topped at 10, out of any climb.

use crate::body::{StepLimits, resolve_motion};
use crate::controller::Actions;
use crate::manifest::Founder;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

const V: f64 = 0.125;
const WALL_X: u32 = 16;

/// A `32 × 16 × 16` world at 0.125 m, ground at 3 and a wall over `x` 16..18 topped at
/// 10, except where `open(z)`.
fn walled(open: impl Fn(u32) -> bool) -> World {
    let (width, depth) = (32, 16);
    let mut w = World::empty(VoxelConfig {
        width,
        height: 16,
        depth,
        voxel_m: V,
        ..VoxelConfig::default()
    });
    for x in 0..width {
        for z in 0..depth {
            let wall = (WALL_X..WALL_X + 2).contains(&x) && !open(z);
            for y in 1..=(if wall { 10 } else { 3 }) {
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

/// Walk forward at full effort for `ticks` ticks from `(x, z)` on layer 3 along
/// `heading`, with `wall_friction`; the final pose and the forward distance requested.
fn walk(world: &World, x: f64, z: f64, heading: f64, friction: f64, ticks: u32) -> (crate::Pose, u32, f64) {
    let phys = crate::FounderPhysiology::placeholder(Founder::Browser);
    let body = phys.adult_body();
    let manifest = Founder::Browser.manifest();
    let view = world.view();
    let limits: StepLimits = phys.step_limits(V);
    let mut pose = crate::Pose {
        x,
        z,
        heading_rad: heading,
    };
    let mut y = 3;
    let forward = Actions {
        forward: 1.0,
        turn: 0.0,
        feed: 0.0,
    };
    for _ in 0..ticks {
        resolve_motion(
            &view,
            &mut pose,
            &mut y,
            &manifest,
            &body,
            phys.wade_depth_m(&body),
            limits,
            friction,
            forward,
        );
    }
    (pose, y, manifest.cruise_m_per_s * cubarium_voxel::DT * f64::from(ticks))
}

fn radius() -> f64 {
    crate::FounderPhysiology::placeholder(Founder::Browser)
        .adult_body()
        .footprint_radius()
}

/// Ticks for about `metres` of requested travel.
fn ticks_for(metres: f64) -> u32 {
    let cruise = Founder::Browser.manifest().cruise_m_per_s;
    (metres / (cruise * cubarium_voxel::DT)).ceil() as u32
}

#[test]
fn a_body_walking_into_a_wall_at_45_degrees_slides_along_it() {
    let w = walled(|_| false);
    let (x0, z0) = (14.0 * V, 2.0 * V);
    let n = ticks_for(1.0);
    let wall_face = f64::from(WALL_X) * V;

    let (free, _, asked) = walk(&w, x0, z0, FRAC_PI_4, 0.0, n);
    assert!(free.x + radius() <= wall_face + 1e-9, "never into the wall: {}", free.x);
    let slid = free.z - z0;
    let ideal = asked * FRAC_PI_4.cos();
    assert!(
        slid > 0.9 * ideal && slid < 1.0 * ideal + 1e-9,
        "friction 0 keeps the along-wall component: slid {slid}, ideal {ideal}"
    );

    let (stuck, _, _) = walk(&w, x0, z0, FRAC_PI_4, 1.0, n);
    let pre_contact = (wall_face - radius() - x0) * 1.0; // 45°: Δz = Δx before contact
    assert!(
        stuck.z - z0 < pre_contact + 0.02,
        "friction 1 is the old motor, no slide: Δz {} (contact after {pre_contact})",
        stuck.z - z0
    );

    let (half, _, _) = walk(&w, x0, z0, FRAC_PI_4, 0.5, n);
    assert!(
        half.z - z0 > stuck.z - z0 + 0.1 && half.z - z0 < slid - 0.1,
        "friction 0.5 slides part of the way: {} between {} and {slid}",
        half.z - z0,
        stuck.z - z0
    );
}

#[test]
fn a_body_walking_into_a_wall_head_on_does_not_slide() {
    let w = walled(|_| false);
    let (x0, z0) = (12.0 * V, 8.0 * V);
    let (p, y, _) = walk(&w, x0, z0, FRAC_PI_2, 0.0, ticks_for(1.0));
    assert_eq!(y, 3);
    assert!((p.z - z0).abs() < 1e-9, "no sideways drift: {} -> {}", z0, p.z);
    let wall_face = f64::from(WALL_X) * V;
    assert!(
        p.x + radius() <= wall_face + 1e-9 && p.x + radius() > wall_face - 0.01,
        "pressed up to the wall: x {}",
        p.x
    );
}

#[test]
fn a_body_sliding_along_a_wall_reaches_an_opening_and_walks_through() {
    // The wall is open over z 10..14 (0.5 m, wider than the 0.375 m disc).
    let w = walled(|z| (10..14).contains(&z));
    let (x0, z0) = (14.0 * V, 3.0 * V);
    let n = ticks_for(2.5);
    let (p, y, _) = walk(&w, x0, z0, FRAC_PI_4, 0.0, n);
    assert_eq!(y, 3, "level ground all the way");
    let beyond = f64::from(WALL_X + 2) * V;
    assert!(p.x > beyond, "through the opening: x {} z {}", p.x, p.z);

    let (old, _, _) = walk(&w, x0, z0, FRAC_PI_4, 1.0, n);
    assert!(old.x < f64::from(WALL_X) * V, "the old motor stays at the wall: {}", old.x);
}
