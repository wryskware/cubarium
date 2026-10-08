//! **Frames read back without a stall** (the Godot app's renderer, H1): a ring of slots,
//! each a command buffer pair, a fence and a host-cached buffer. A frame is recorded and
//! submitted into a free slot and the caller goes on; a later [`ReadbackRing::poll`] hands
//! back, oldest first, every slot whose fence has signalled. Nothing here waits on the GPU,
//! so the GPU renders the next frame while the host copies the last one out.
//!
//! The renderer is told a frame retired ([`FrameSource::frame_retired`]) in submit order,
//! the order it recorded them, which is the contract its staging ring keeps.

use std::collections::VecDeque;
use std::sync::Arc;

use anyhow::{Result, ensure};
use ash::vk;

use crate::present::FrameSource;
use crate::vk::{Gpu, HostBuffer, barrier};

struct Slot {
    world: vk::CommandBuffer,
    copy: vk::CommandBuffer,
    fence: vk::Fence,
    host: HostBuffer,
    tag: u64,
}

/// A ring of read-back frames for one renderer's raster ([`ReadbackRing::new`]).
pub struct ReadbackRing {
    gpu: Arc<Gpu>,
    slots: Vec<Slot>,
    /// Slots in flight, oldest first.
    flight: VecDeque<usize>,
    raster: vk::Image,
    size: (u32, u32),
}

// The host buffers' mapped pointers are only read by whoever holds the ring, after their
// fence signalled.
unsafe impl Send for ReadbackRing {}

impl ReadbackRing {
    /// `slots` frames for `src`'s raster image `raster`, at most what the renderer has
    /// per-frame resources for ([`FrameSource::frame_capacity`]).
    pub fn new<S: FrameSource>(gpu: Arc<Gpu>, src: &S, raster: vk::Image, slots: usize) -> Result<ReadbackRing> {
        let n = slots.min(src.frame_capacity()).max(1);
        let size = src.raster_size();
        let bytes = u64::from(size.0) * u64::from(size.1) * 4;
        let d = &gpu.device;
        let mut list = Vec::with_capacity(n);
        for _ in 0..n {
            let cbs = unsafe {
                d.allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(src.command_pool())
                        .command_buffer_count(2),
                )
            }?;
            let fence = unsafe {
                d.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None)
            }?;
            list.push(Slot { world: cbs[0], copy: cbs[1], fence, host: gpu.readback_buffer(bytes)?, tag: 0 });
        }
        Ok(ReadbackRing { gpu, slots: list, flight: VecDeque::new(), raster, size })
    }

    /// The raster's size, pixels.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Frames submitted and not yet handed back.
    pub fn in_flight(&self) -> usize {
        self.flight.len()
    }

    /// Whether a frame can be submitted now: a slot is free and the renderer has room.
    pub fn can_submit<S: FrameSource>(&self, src: &S) -> bool {
        self.flight.len() < self.slots.len() && src.frames_in_flight() < src.frame_capacity()
    }

    /// Record one frame of `src` and the copy of its raster, and submit both. `tag` comes
    /// back with the frame. False (nothing recorded) when no slot is free.
    pub fn submit<S: FrameSource>(&mut self, src: &mut S, frame: S::Frame<'_>, tag: u64) -> Result<bool> {
        if !self.can_submit(src) {
            return Ok(false);
        }
        let i = (0..self.slots.len())
            .find(|i| !self.flight.contains(i))
            .expect("a free slot: fewer in flight than slots");
        let gpu = self.gpu.clone();
        let d = &gpu.device;
        let slot = &mut self.slots[i];
        unsafe {
            d.reset_command_buffer(slot.world, vk::CommandBufferResetFlags::empty())?;
            d.reset_command_buffer(slot.copy, vk::CommandBufferResetFlags::empty())?;
        }
        src.record_frame(&gpu, slot.world, frame, None)?;
        let (w, h) = self.size;
        unsafe {
            d.begin_command_buffer(
                slot.copy,
                &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            barrier(d, slot.copy, self.raster, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
            d.cmd_copy_image_to_buffer(
                slot.copy,
                self.raster,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                slot.host.buffer,
                &[vk::BufferImageCopy::default()
                    .buffer_row_length(w)
                    .buffer_image_height(h)
                    .image_subresource(
                        vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1),
                    )
                    .image_extent(vk::Extent3D { width: w, height: h, depth: 1 })],
            );
            barrier(d, slot.copy, self.raster, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            // The host reads the buffer once the fence signals.
            d.cmd_pipeline_barrier(
                slot.copy,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::HOST,
                vk::DependencyFlags::empty(),
                &[vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::HOST_READ)],
                &[],
                &[],
            );
            d.end_command_buffer(slot.copy)?;
            d.reset_fences(&[slot.fence])?;
        }
        let commands = [slot.world, slot.copy];
        gpu.submit(&[vk::SubmitInfo::default().command_buffers(&commands)], slot.fence)?;
        slot.tag = tag;
        self.flight.push_back(i);
        Ok(true)
    }

    /// Hand back every finished frame, oldest first and without waiting: `done(tag, rgba)`
    /// with the raster as `w · h · 4` sRGB bytes. Stops at the first unfinished frame, so
    /// frames come back (and retire in `src`) in submit order. Returns how many.
    pub fn poll<S: FrameSource>(&mut self, src: &mut S, mut done: impl FnMut(u64, &[u8])) -> Result<usize> {
        let mut n = 0;
        while let Some(&i) = self.flight.front() {
            let slot = &self.slots[i];
            if !unsafe { self.gpu.device.get_fence_status(slot.fence) }? {
                break;
            }
            self.flight.pop_front();
            src.frame_retired();
            let (w, h) = self.size;
            let len = w as usize * h as usize * 4;
            // The fence signalled after the copy and its host barrier.
            let bytes = unsafe { slot.host.bytes() };
            ensure!(bytes.len() >= len, "a readback buffer smaller than its raster");
            done(slot.tag, &bytes[..len]);
            n += 1;
        }
        Ok(n)
    }

    /// Wait for every frame in flight and hand them back ([`ReadbackRing::poll`]).
    pub fn drain<S: FrameSource>(&mut self, src: &mut S, done: impl FnMut(u64, &[u8])) -> Result<usize> {
        let fences: Vec<vk::Fence> = self.flight.iter().map(|&i| self.slots[i].fence).collect();
        if !fences.is_empty() {
            unsafe { self.gpu.device.wait_for_fences(&fences, true, u64::MAX) }?;
        }
        self.poll(src, done)
    }
}

impl Drop for ReadbackRing {
    /// Waits for whatever is in flight, then frees the fences and buffers. The command
    /// buffers go with their renderer's pool, which must outlive this ring.
    fn drop(&mut self) {
        let d = &self.gpu.device;
        let fences: Vec<vk::Fence> = self.slots.iter().map(|s| s.fence).collect();
        unsafe {
            let _ = d.wait_for_fences(&fences, true, u64::MAX);
            for s in &self.slots {
                d.destroy_fence(s.fence, None);
            }
        }
        for s in &self.slots {
            s.host.destroy(&self.gpu);
        }
    }
}
