//! Mapped frame uploads. Every copy belongs to its encoder, including rollback.
#![forbid(unsafe_code)]

use super::{Gpu, MapReply};
use anyhow::{Context, Result, ensure};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub fn texture_bytes(size: wgpu::Extent3d, texel: u32) -> u64 {
    u64::from((size.width * texel).div_ceil(256) * 256)
        * u64::from(size.height)
        * u64::from(size.depth_or_array_layers)
}

pub fn buffer(gpu: &Gpu, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    gpu.buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.max(4).div_ceil(4) * 4,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

struct Slot {
    buffer: wgpu::Buffer,
    ready: MapReply,
}
pub struct UploadRing {
    slots: Vec<Slot>,
    flight: VecDeque<usize>,
}

impl UploadRing {
    pub fn new(gpu: &Gpu, size: u64, count: usize) -> Self {
        Self {
            slots: (0..count)
                .map(|_| Slot {
                    buffer: gpu.buffer(&wgpu::BufferDescriptor {
                        label: Some("frame upload ring"),
                        size: size.max(256).div_ceil(256) * 256,
                        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: true,
                    }),
                    ready: Arc::new(Mutex::new(Some(Ok(())))),
                })
                .collect(),
            flight: VecDeque::with_capacity(count),
        }
    }
    pub fn begin(&mut self, gpu: &Gpu, slot: usize) -> Result<Upload<'_>> {
        gpu.device.poll(wgpu::PollType::Poll)?;
        ensure!(
            !self.flight.contains(&slot),
            "upload slot is still in flight"
        );
        let s = &self.slots[slot];
        let mut ready = s.ready.lock().unwrap_or_else(|e| e.into_inner());
        let result = ready.take().context("upload slot mapping is pending")?;
        result.context("map upload slot")?;
        let view = s.buffer.slice(..).get_mapped_range_mut()?;
        drop(ready);
        self.flight.push_back(slot);
        Ok(Upload {
            buffer: &s.buffer,
            view: Some(view),
            ready: s.ready.clone(),
            cursor: 0,
        })
    }
    pub fn reclaim_completed(&mut self) {
        while self.flight.front().is_some_and(|&i| {
            self.slots[i]
                .ready
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_some()
        }) {
            self.flight.pop_front();
        }
    }
    pub fn available(&self) -> Option<usize> {
        (0..self.slots.len()).find(|i| {
            !self.flight.contains(i)
                && self.slots[*i]
                    .ready
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_some()
        })
    }
    pub fn discard_last(&mut self) {
        if let Some(&slot) = self.flight.back() {
            self.discard_slot(slot);
        }
    }
    pub fn retire(&mut self) {
        self.flight.pop_front();
    }
    pub fn discard_slot(&mut self, slot: usize) {
        if self.flight.back() != Some(&slot) {
            return;
        }
        if let Some(i) = self.flight.pop_back() {
            let s = &self.slots[i];
            // No submitted command reads this buffer. A dropped encoder never
            // runs its map-on-submit callback, so request the mapping ourselves.
            if s.ready.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
                let ready = s.ready.clone();
                s.buffer
                    .slice(..)
                    .map_async(wgpu::MapMode::Write, move |r| {
                        *ready.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
                    });
            }
        }
    }
}

pub struct Upload<'a> {
    buffer: &'a wgpu::Buffer,
    view: Option<wgpu::BufferViewMut>,
    ready: MapReply,
    cursor: u64,
}
impl Upload<'_> {
    fn reserve(&mut self, bytes: u64, alignment: u64) -> Result<u64> {
        let offset = self.cursor.div_ceil(alignment) * alignment;
        self.cursor = offset.checked_add(bytes).context("upload size overflow")?;
        ensure!(
            self.cursor <= self.buffer.size(),
            "frame upload ring capacity exceeded"
        );
        Ok(offset)
    }
    pub fn bytes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Buffer,
        offset: u64,
        bytes: &[u8],
    ) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        ensure!(
            bytes.len() % 4 == 0 && offset % 4 == 0,
            "buffer upload must be word aligned"
        );
        ensure!(
            offset
                .checked_add(bytes.len() as u64)
                .is_some_and(|end| end <= target.size()),
            "buffer upload exceeds destination"
        );
        let at = self.reserve(bytes.len() as u64, 4)?;
        self.view
            .as_mut()
            .unwrap()
            .slice(at as usize..at as usize + bytes.len())
            .copy_from_slice(bytes);
        encoder.copy_buffer_to_buffer(self.buffer, at, target, offset, bytes.len() as u64);
        Ok(())
    }
    pub fn texture(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        origin: wgpu::Origin3d,
        size: wgpu::Extent3d,
        texel: u32,
        bytes: &[u8],
    ) -> Result<()> {
        self.texture_rows(
            encoder,
            target,
            origin,
            size,
            texel,
            bytes,
            size.width * texel,
            size.height,
        )
    }
    /// Source rows can be the full plane or fixed 8³ packed brick rows. Neither
    /// a temporary brick vector nor padding initialization is necessary.
    pub fn texture_rows(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        origin: wgpu::Origin3d,
        size: wgpu::Extent3d,
        texel: u32,
        bytes: &[u8],
        source_stride: u32,
        source_height: u32,
    ) -> Result<()> {
        let row = size.width * texel;
        let stride = row.div_ceil(256) * 256;
        let at = self.reserve(texture_bytes(size, texel), 256)?;
        let view = self.view.as_mut().unwrap();
        for z in 0..size.depth_or_array_layers {
            for y in 0..size.height {
                let src = ((z * source_height + y) * source_stride) as usize;
                let dst = at as usize + ((z * size.height + y) * stride) as usize;
                view.slice(dst..dst + row as usize)
                    .copy_from_slice(&bytes[src..src + row as usize]);
            }
        }
        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: at,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(size.height),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture: target,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            size,
        );
        Ok(())
    }
    pub fn finish(mut self, encoder: &mut wgpu::CommandEncoder) {
        self.view.take();
        self.buffer.unmap();
        let ready = self.ready.clone();
        encoder.map_buffer_on_submit(self.buffer, wgpu::MapMode::Write, .., move |r| {
            *ready.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
        });
    }
}
impl Drop for Upload<'_> {
    fn drop(&mut self) {
        if self.view.take().is_some() {
            self.buffer.unmap();
        }
    }
}
