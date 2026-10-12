//! Portable safe GPU rendering of the ring and voxel worlds.
//! Maintained WGSL runs through wgpu on Vulkan, Metal and Direct3D.
#![forbid(unsafe_code)]

pub mod adapter;
pub mod atlas;
pub mod bloom;
pub mod cord;
#[cfg(test)]
mod cord_tests;
pub mod figures;
#[cfg(test)]
mod figures_order_tests;
pub mod palette;
pub mod plants;
pub mod present;
pub mod render;
pub mod scene;
pub mod sunvis;
pub mod synthetic;
pub mod target;
pub mod voxel;
pub mod weather;
pub mod wgpu_backend;

pub use adapter::{MAX_POSES, PoseRef, ScratchFrame, Stamp, StampMask, StampTone};
pub use atlas::{Atlas, Clip, FrameRect, PlantClip};
pub use present::{FrameSource, PresentPass, TargetSlot};
pub use render::{FrameTiming, PresentTransform, Renderer, TargetImage};
pub use scene::{
    Fields, LAYER_COUNT, LAYERS, Layer, MASK_AXIAL, MASK_RADIAL, NO_MASK_FLOOR, NO_MASK_REVEAL,
    RingLayout, Scene, SpriteInstance,
};
pub use voxel::{VoxelParams, VoxelRenderer, VoxelStyle, VoxelTexel};
pub use wgpu_backend::Gpu;
