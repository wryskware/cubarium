//! cubarium-render — a linear-light canvas and seam-aware rasterization.
//!
//! The renderer reads immutable state and writes pixels; it never touches simulation
//! state or randomness. All spatial work goes through `cubarium-surface`: bodies are
//! stamped through [`cubarium_surface::unfold_pixels`], trails are the actual transported
//! [`cubarium_surface::PathSegment`]s, and field filtering uses pixel seam neighbors.
//! Neither the preview nor the physical output gets its own geometry.
//!
//! Output transfer convention (M1 decision, to be verified on the cube): the canvas holds
//! linear light in `[0, 1]`; [`Canvas::encode`] clamps and applies the sRGB transfer
//! function to 8-bit, exactly as an ordinary image would be encoded. The shim's default
//! color correction is identity (`gamma = 1.0`), and its own test patterns and clients
//! send ordinary image bytes, so the LED chain is treated as consuming sRGB-encoded
//! values. The desktop preview shows the same bytes natively, so preview and cube agree
//! by construction.

#![forbid(unsafe_code)]

mod body;
mod canvas;
mod field;
mod multipart;
mod sprite;
mod trail;

pub use body::{BodyShape, Lobe, stamp_body};
pub use canvas::{Canvas, srgb_decode, srgb_encode};
pub use field::draw_field;
pub use multipart::{
    MAX_GRID, MIN_RIG_SCALE, RIG_MARGIN, RigPart, SUPERSAMPLE_REACH, grid_schedule, rig_radius,
    stamp_rig, stamp_rig_scaled, stamp_rig_with_radius,
};
pub use sprite::{
    Bend, Mask, Pose, Sprite, stamp_layers, stamp_layers_bent, stamp_layers_bent_with_radius,
    stamp_pose, stamp_pose_in_chart, stamp_sprite,
};
pub use trail::{Trail, TrailSegment, draw_trail};
