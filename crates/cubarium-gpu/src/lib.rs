//! `cubarium-gpu`: the ring world drawn on the GPU, with the CPU presenter's look.
//!
//! Built for the Particle Tachyon's Adreno 643 through Qualcomm's Vulkan blob — raw
//! `ash`, no Mesa, no wgpu — as `design/7_Research/gpu-scanout-spike-2026-09-16.md`
//! recommended, and for a desktop window while that board is busy.
//!
//! # What this crate is, and is not
//!
//! **Stage A** (this) renders a [`Scene`] you build. The synthetic example builds one
//! by hand; **Stage B** will add the adapter from the world's `RenderView` after FW-2
//! lands. Nothing here depends on `cubarium-core` or `cubarium-surface`, which is what
//! lets it be built and measured while those crates are still moving.
//!
//! The CPU presenter (`crates/cubarium/src/art_present`) is untouched and stays: it
//! draws the LED cube and it is what the byte-exact regression tests are written
//! against.
//!
//! # The shape of a frame
//!
//! ```text
//!   per tick (20 Hz)   Fields  ──► two 80×45 RGBA16F cell textures   57.6 KB
//!   per frame (60 Hz)  layers  ──► one instance buffer               88 B each
//!                                       │
//!            background ─ ground cover ─ water ─ plants ─ tall ─ rain ─ bodies
//!                                       │
//!                              world raster, w×h, R8G8B8A8_SRGB
//!                                       │
//!                       present: nearest ×k upscale + quarter turn
//!                                       │
//!                    window swapchain  ·or·  linear dma-buf → KMS page flip
//! ```
//!
//! Only the last pass runs at panel resolution. Everything above it runs at the world
//! raster's 57,600 (S = 1) or 230,400 (S = 2) pixels.

pub mod adapter;
pub mod atlas;
pub mod bloom;
pub mod palette;
pub mod present;
pub mod render;
pub mod scene;
pub mod sunvis;
pub mod synthetic;
pub mod target;
pub mod vk;
pub mod voxel;

pub use adapter::{MAX_POSES, PoseRef, ScratchFrame, Stamp, StampMask, StampTone};
pub use atlas::{Atlas, Clip, FrameRect, PlantClip};
pub use present::{FrameSource, PresentPass, TargetSlot};
pub use render::{FrameTiming, PresentTransform, Renderer, TargetImage};
pub use scene::{
    Fields, LAYER_COUNT, LAYERS, Layer, MASK_AXIAL, MASK_RADIAL, NO_MASK_FLOOR, NO_MASK_REVEAL,
    RingLayout, Scene, SpriteInstance,
};
pub use vk::Gpu;
pub use voxel::{VoxelParams, VoxelRenderer, VoxelStyle, VoxelTexel};
