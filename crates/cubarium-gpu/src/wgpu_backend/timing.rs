//! Encoder-scoped timestamp readback; discarded encoders never submit callbacks.
#![forbid(unsafe_code)]
use super::Gpu;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};
pub struct Timing {
    values: Arc<Mutex<BTreeMap<u64, Vec<u64>>>>,
    pending: VecDeque<u64>,
    next: u64,
    done: Option<u64>,
}
pub struct Frame {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    ticket: u64,
    count: u32,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            values: Default::default(),
            pending: Default::default(),
            next: 0,
            done: None,
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
            return None;
        }
        let ticket = self.next;
        self.next += 1;
        self.pending.push_back(ticket);
        Some(Frame {
            queries: gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("frame timing"),
                ty: wgpu::QueryType::Timestamp,
                count,
            }),
            resolve: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("resolved timestamps"),
                size: u64::from(count) * 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamp readback"),
                size: u64::from(count) * 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            ticket,
            count,
        })
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
        });
    }
    pub fn retire(&mut self) {
        if let Some(ticket) = self.pending.pop_front() {
            self.done = Some(ticket);
            self.values
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|&key, _| key >= ticket);
        }
    }
    pub fn discard(&mut self) {
        if let Some(ticket) = self.pending.pop_back() {
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
