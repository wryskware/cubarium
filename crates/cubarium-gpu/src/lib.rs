//! Portable safe GPU rendering of the ring and voxel worlds.
//! Maintained WGSL runs through wgpu on Vulkan, Metal and Direct3D.
#![deny(unsafe_code)]

#[forbid(unsafe_code)]
pub mod adapter;
#[forbid(unsafe_code)]
pub mod atlas;
#[forbid(unsafe_code)]
pub mod bloom;
#[forbid(unsafe_code)]
pub mod cord;
#[cfg(test)]
#[forbid(unsafe_code)]
mod cord_tests;
#[forbid(unsafe_code)]
pub mod figures;
#[cfg(test)]
#[forbid(unsafe_code)]
mod figures_order_tests;
#[forbid(unsafe_code)]
pub mod palette;
pub mod pipeline_cache;
#[forbid(unsafe_code)]
pub mod plants;
#[forbid(unsafe_code)]
pub mod present;
#[forbid(unsafe_code)]
pub mod render;
#[forbid(unsafe_code)]
pub mod scene;
#[forbid(unsafe_code)]
pub mod sunvis;
#[forbid(unsafe_code)]
pub mod synthetic;
#[forbid(unsafe_code)]
pub mod target;
#[forbid(unsafe_code)]
pub mod voxel;
#[forbid(unsafe_code)]
pub mod weather;
#[forbid(unsafe_code)]
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
