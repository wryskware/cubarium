//! The lit tier's effect switches and the sun-visibility volume the light shafts read
//! (package V, `design/handoffs/presentation-plan-2026-09-24.md`).
//!
//! # Effect switches
//!
//! [`Effects`] names what each lit effect costs a pipeline: the sun march (`shadows`), the
//! water's reflection march (`reflections`), the light shafts (`volumetric`), the
//! emitters' local light (`glow`) and the AO crease (`ao`). Each is a `voxel.frag` specialisation constant beside
//! `LIT` ([`Effects::spec_constants`]), so an effect switched off is dead code in the
//! pipeline and costs nothing, not even registers. They are fixed for a renderer's life,
//! as the tier is.
//!
//! # The sun-visibility volume
//!
//! One texel per voxel, `R8G8_UNORM`, sampled trilinearly (`volumetric.glsl`'s
//! `sunInAir`): how much sun reaches each cell's air. The **red** channel is the bake
//! being faded out and the **green** one the bake being faded in; the shader mixes them by
//! the frame's fade. The bake itself is the caller's (`cubarium`'s
//! `sink::gpu::sunvis`, on the CPU off the loop thread); this holds the image, one staging
//! buffer, and the upload's place in the frame ring:
//!
//! * [`SunVisVolume::accepts`] only while no recorded frame copies from it, so the
//!   staging buffer is never written under a frame that copies from it;
//! * [`SunVisVolume::put`] writes a new pair of channels and owes an upload;
//! * the next redrawing frame records the copy ([`SunVisVolume::record_upload`]) and
//!   its uniform block carries the fade the caller set with the new data, in the same
//!   frame: a new bake and the fade that starts it can never be a frame apart;
//! * the frame retiring frees the buffer ([`SunVisVolume::retired`]); the frame being
//!   displaced before it ran owes the upload again ([`SunVisVolume::discarded`]).
//!
//! In the flat tier, and with `volumetric` off, the image is one texel the shader never
//! reads and nothing is ever uploaded.

use anyhow::Result;
use ash::vk;

use crate::vk::{Gpu, HostBuffer, barrier};

/// The lit tier's effect switches and the light shafts' numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    /// The sun march. Off: every face turned toward the sun is sunlit (no cast shadows).
    pub shadows: bool,
    /// The water's reflection march. Off: the water reflects nothing.
    pub reflections: bool,
    /// The light shafts: the in-scatter march after the slab walk, and the volume.
    pub volumetric: bool,
    /// The emitters' local light (the glow volume's fetches). Off with emission off.
    pub glow: bool,
    /// The AO crease's neighbour fetches. Off with `[light] ao = 0`. (A uniform test on
    /// the strength instead costs the lit walk 0.47 ms at 13 px on the 5080 even with AO
    /// on: the early return changes the inlined crease's code.)
    pub ao: bool,
    /// Light scattered toward the camera per voxel of view path through fully sunlit air
    /// at the world's floor, as a fraction of the palette's light.
    pub density: f32,
    /// Voxels up over which the air's density falls by e.
    pub falloff: f32,
    /// The glow volume's weight in the air, relative to full sun.
    pub glow_weight: f32,
}

impl Default for Effects {
    /// Today's lit picture: shadows, reflections and the glow on; no shafts.
    fn default() -> Effects {
        Effects {
            shadows: true,
            reflections: true,
            volumetric: false,
            glow: true,
            ao: true,
            density: 0.03,
            falloff: 12.0,
            glow_weight: 1.0,
        }
    }
}

/// `voxel.frag`'s specialisation constants in `constant_id` order: `LIT` (0), `SHADOWS`
/// (1), `REFLECTIONS` (2), `VOLUMETRIC` (3), `GLOW` (4), `AO` (5), each a 32-bit bool.
pub const SPEC_CONSTANTS: u32 = 6;

impl Effects {
    /// The specialisation data for a pipeline of this tier. The flat tier has every lit
    /// effect off: its pipeline is the flat shader code alone, as it always was.
    pub fn spec_constants(&self, lit: bool) -> [u32; SPEC_CONSTANTS as usize] {
        [
            u32::from(lit),
            u32::from(lit && self.shadows),
            u32::from(lit && self.reflections),
            u32::from(lit && self.volumetric),
            u32::from(lit && self.glow),
            u32::from(lit && self.ao),
        ]
    }

    /// The `volK` uniform: density, glow weight, falloff, and the frame's `fade`.
    pub fn uniform(&self, fade: f32) -> [f32; 4] {
        [
            self.density.max(0.0),
            self.glow_weight.max(0.0),
            self.falloff.max(1e-3),
            fade.clamp(0.0, 1.0),
        ]
    }

    /// Whether a renderer of this tier builds the full-size volume.
    pub fn volume_on(&self, lit: bool) -> bool {
        lit && self.volumetric
    }
}

/// Where the volume's upload stands in the frame ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Upload {
    /// The staging buffer is free.
    Idle,
    /// New data is in the staging buffer; the next redrawing frame copies it.
    Owed,
    /// The frame in this ring slot copies it and has not retired.
    Recorded(usize),
}

/// The sun-visibility volume's image and its upload ([`crate::sunvis`]).
pub struct SunVisVolume {
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    extent: (u32, u32, u32),
    /// `None` when the volume is a one-texel stand-in.
    staging: Option<HostBuffer>,
    upload: Upload,
    /// The fade the frames use: the red channel at 0, the green at 1.
    fade: f32,
}

/// Bytes a texel: the bake fading out, the bake fading in.
pub const TEXEL_BYTES: usize = 2;

impl SunVisVolume {
    /// The volume for a `width × height × depth` world, full size when `on`. A full-size
    /// volume starts owing an upload of zeros, so the first frame finds it defined: no sun
    /// in the air until the first bake arrives.
    pub fn new(gpu: &Gpu, on: bool, width: u32, height: u32, depth: u32) -> Result<SunVisVolume> {
        let extent = if on { (width, height, depth) } else { (1, 1, 1) };
        let format = vk::Format::R8G8_UNORM;
        let (image, memory) = crate::voxel::image_3d(gpu, extent.0, extent.1, extent.2, format)?;
        let view = crate::voxel::view_3d(gpu, image, format)?;
        let staging = if on {
            let bytes = extent.0 as u64 * extent.1 as u64 * extent.2 as u64 * TEXEL_BYTES as u64;
            let buffer = gpu.host_buffer(bytes, vk::BufferUsageFlags::TRANSFER_SRC)?;
            unsafe { std::ptr::write_bytes(buffer.ptr, 0, bytes as usize) };
            Some(buffer)
        } else {
            None
        };
        Ok(SunVisVolume {
            image,
            memory,
            view,
            extent,
            upload: if staging.is_some() { Upload::Owed } else { Upload::Idle },
            staging,
            fade: 1.0,
        })
    }

    /// Whether this is the full-size volume.
    pub fn on(&self) -> bool {
        self.staging.is_some()
    }

    /// Bytes [`SunVisVolume::put`] takes: [`TEXEL_BYTES`] a voxel, in the voxels' texture
    /// order `(z · height + y) · width + x`.
    pub fn bytes(&self) -> usize {
        self.extent.0 as usize * self.extent.1 as usize * self.extent.2 as usize * TEXEL_BYTES
    }

    /// Whether [`SunVisVolume::put`] may write now: the volume is full size and no recorded
    /// frame still copies from the staging buffer. An upload owed but not yet recorded (the
    /// zeros a new volume starts with, before any frame) is simply replaced.
    pub fn accepts(&self) -> bool {
        self.on() && !matches!(self.upload, Upload::Recorded(_))
    }

    /// A new pair of bakes, faded in from `fade` (usually 0). Refused (false) unless
    /// [`SunVisVolume::accepts`], or when `rg` is not [`SunVisVolume::bytes`] long.
    pub fn put(&mut self, rg: &[u8], fade: f32) -> bool {
        if !self.accepts() || rg.len() != self.bytes() {
            return false;
        }
        let staging = self.staging.as_ref().expect("a full-size volume has staging");
        staging.write(rg);
        self.upload = Upload::Owed;
        self.fade = fade.clamp(0.0, 1.0);
        true
    }

    /// Move the fade. Returns whether the picture would change (a step of 1/1024 or more,
    /// or reaching an end), so a frame between two such steps may keep its raster. Held
    /// while an upload is owed: the fade that goes with new data goes up with it.
    pub fn set_fade(&mut self, fade: f32) -> bool {
        let fade = fade.clamp(0.0, 1.0);
        if self.upload == Upload::Owed {
            return false;
        }
        let q = |f: f32| (f * 1024.0).round() as i32;
        if q(fade) == q(self.fade) {
            return false;
        }
        self.fade = fade;
        true
    }

    /// The fade a frame recorded now draws with.
    pub fn fade(&self) -> f32 {
        self.fade
    }

    /// Whether a frame recorded now must copy new data (and so redraw).
    pub fn owed(&self) -> bool {
        self.upload == Upload::Owed
    }

    /// Record the copy into the frame in ring slot `slot`, if one is owed.
    ///
    /// # Safety
    /// `cb` must be recording, ahead of the pass that samples the volume.
    pub unsafe fn record_upload(&mut self, d: &ash::Device, cb: vk::CommandBuffer, slot: usize) {
        if self.upload != Upload::Owed {
            return;
        }
        let Some(staging) = &self.staging else {
            return;
        };
        let (w, h, depth) = self.extent;
        unsafe {
            barrier(
                d,
                cb,
                self.image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );
            d.cmd_copy_buffer_to_image(
                cb,
                staging.buffer,
                self.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy::default()
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D {
                        width: w,
                        height: h,
                        depth,
                    })],
            );
            barrier(
                d,
                cb,
                self.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        }
        self.upload = Upload::Recorded(slot);
    }

    /// The frame in ring slot `slot` has completed.
    pub fn retired(&mut self, slot: usize) {
        if self.upload == Upload::Recorded(slot) {
            self.upload = Upload::Idle;
        }
    }

    /// The frame in ring slot `slot` was displaced before it ran: its copy is owed again.
    pub fn discarded(&mut self, slot: usize) {
        if self.upload == Upload::Recorded(slot) {
            self.upload = Upload::Owed;
        }
    }

    pub fn destroy(&self, gpu: &Gpu) {
        if let Some(s) = &self.staging {
            s.destroy(gpu);
        }
        let d = &gpu.device;
        unsafe {
            d.destroy_image_view(self.view, None);
            d.destroy_image(self.image, None);
            d.free_memory(self.memory, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switched_off_effect_is_off_in_the_pipeline_and_the_flat_tier_has_none() {
        let e = Effects {
            shadows: false,
            volumetric: true,
            ..Effects::default()
        };
        assert_eq!(e.spec_constants(true), [1, 0, 1, 1, 1, 1]);
        assert_eq!(e.spec_constants(false), [0; 6]);
        assert_eq!(Effects::default().spec_constants(true), [1, 1, 1, 0, 1, 1]);
        assert!(e.volume_on(true) && !e.volume_on(false));
        assert!(!Effects::default().volume_on(true));
    }

    #[test]
    fn the_uniform_carries_the_knobs_and_a_clamped_fade() {
        let e = Effects {
            density: 0.02,
            glow_weight: 0.5,
            falloff: 30.0,
            ..Effects::default()
        };
        assert_eq!(e.uniform(0.25), [0.02, 0.5, 30.0, 0.25]);
        assert_eq!(e.uniform(3.0)[3], 1.0);
        let bad = Effects {
            density: -1.0,
            falloff: 0.0,
            ..e
        };
        let u = bad.uniform(-1.0);
        assert_eq!((u[0], u[3]), (0.0, 0.0));
        assert!(u[2] > 0.0);
    }
}
