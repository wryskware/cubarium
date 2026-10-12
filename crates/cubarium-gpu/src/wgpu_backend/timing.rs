//! Encoder-scoped timestamp readback; discarded encoders never submit callbacks.
#![forbid(unsafe_code)]
use super::Gpu;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};
pub struct Timing {
    values: Arc<Mutex<BTreeMap<u64, Vec<u64>>>>,
    pending: VecDeque<Option<(u64, usize)>>,
    next: u64,
    done: Option<u64>,
    slots: Vec<Frame>,
}
#[derive(Clone)]
pub struct Frame {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    ticket: u64,
    count: u32,
    ready: Arc<std::sync::atomic::AtomicBool>,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            values: Default::default(),
            pending: Default::default(),
            next: 0,
            done: None,
            slots: Vec::new(),
        }
    }
}
impl Timing {
    pub fn begin(&mut self, gpu: &Gpu, count: u32) -> Option<Frame> {
        if !gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
        {
            self.pending.push_back(None);
            return None;
        }
        if self.slots.is_empty() {
            self.slots = (0..crate::voxel::STAGING_RING)
                .map(|_| Frame {
                    queries: gpu.query_set(&wgpu::QuerySetDescriptor {
                        label: Some("frame timing"),
                        ty: wgpu::QueryType::Timestamp,
                        count,
                    }),
                    resolve: gpu.buffer(&wgpu::BufferDescriptor {
                        label: Some("resolved timestamps"),
                        size: u64::from(count) * 8,
                        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    }),
                    read: gpu.buffer(&wgpu::BufferDescriptor {
                        label: Some("timestamp readback"),
                        size: u64::from(count) * 8,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    ticket: 0,
                    count,
                    ready: Arc::new(std::sync::atomic::AtomicBool::new(true)),
                })
                .collect();
        }
        let ticket = self.next;
        let Some(slot) = self
            .slots
            .iter()
            .position(|slot| slot.ready.load(std::sync::atomic::Ordering::Acquire))
        else {
            self.pending.push_back(None);
            return None;
        };
        self.slots[slot]
            .ready
            .store(false, std::sync::atomic::Ordering::Release);
        self.next += 1;
        self.pending.push_back(Some((ticket, slot)));
        let mut frame = self.slots[slot].clone();
        frame.ticket = ticket;
        Some(frame)
    }

    pub fn finish(&self, encoder: &mut wgpu::CommandEncoder, frame: Frame) {
        encoder.resolve_query_set(&frame.queries, 0..frame.count, &frame.resolve, 0);
        encoder.copy_buffer_to_buffer(
            &frame.resolve,
            0,
            &frame.read,
            0,
            u64::from(frame.count) * 8,
        );
        let buffer = frame.read.clone();
        let values = self.values.clone();
        let ticket = frame.ticket;
        let ready = frame.ready.clone();
        encoder.map_buffer_on_submit(&frame.read, wgpu::MapMode::Read, .., move |result| {
            if result.is_ok() {
                if let Ok(view) = buffer.slice(..).get_mapped_range() {
                    let stamps = view
                        .chunks_exact(8)
                        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
                        .collect();
                    drop(view);
                    buffer.unmap();
                    values
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(ticket, stamps);
                }
            }
            ready.store(true, std::sync::atomic::Ordering::Release);
        });
    }
    pub fn retire(&mut self) {
        if let Some(Some((ticket, _))) = self.pending.pop_front() {
            self.done = Some(ticket);
            self.values
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|&key, _| key >= ticket);
        }
    }
    pub fn discard(&mut self) {
        if let Some(Some((ticket, slot))) = self.pending.pop_back() {
            self.slots[slot]
                .ready
                .store(true, std::sync::atomic::Ordering::Release);
            self.values
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&ticket);
        }
    }
    pub fn stamps(&self) -> Option<Vec<u64>> {
        self.values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&self.done?)
            .cloned()
    }
}
impl Frame {
    pub fn stamp(&self, encoder: &mut wgpu::CommandEncoder, index: u32) {
        encoder.write_timestamp(&self.queries, index);
    }
}
