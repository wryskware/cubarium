//! Renderer-only history trails made of actual transported path segments.

use std::collections::VecDeque;

use crate::Canvas;
use cubarium_surface::{PathSegment, Travel};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrailSegment {
    pub segment: PathSegment,
    /// Tick at which this segment was traveled.
    pub tick: u64,
}

/// A bounded queue of traveled segments. Presentation only: never checkpointed, never
/// sensed, never a source of simulation randomness.
#[derive(Clone, Debug)]
pub struct Trail {
    segments: VecDeque<TrailSegment>,
    max_segments: usize,
}

impl Trail {
    pub fn new(max_segments: usize) -> Trail {
        Trail { segments: VecDeque::with_capacity(max_segments), max_segments }
    }

    /// Append every segment of a travel; drops the oldest beyond `max_segments`.
    pub fn push_travel(&mut self, travel: &Travel, tick: u64) {
        for s in &travel.segments {
            if self.segments.len() == self.max_segments {
                self.segments.pop_front();
            }
            self.segments.push_back(TrailSegment { segment: *s, tick });
        }
    }

    /// Drop segments older than `max_age_ticks` before `now`.
    pub fn prune(&mut self, now: u64, max_age_ticks: u64) {
        while let Some(front) = self.segments.front() {
            if now.saturating_sub(front.tick) > max_age_ticks {
                self.segments.pop_front();
            } else {
                break;
            }
        }
    }

    /// A gap: the next segment will not join the previous one (dropped view, restart).
    pub fn clear(&mut self) {
        self.segments.clear();
    }

    pub fn segments(&self) -> impl Iterator<Item = &TrailSegment> {
        self.segments.iter()
    }

    pub fn len(&self) -> usize {
        self.segments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

/// Draw each segment as a one-pixel line inside its own chart (segments are already
/// split per chart by `travel`, so no seam logic is needed here), with brightness
/// `color · (1 - age/max_age_ticks)` clamped to `[0, 1]`. A pixel touched by several
/// segments keeps the maximum brightness rather than accumulating.
///
/// Normative rasterization: walk the segment in steps of at most 0.5 pixels, lighting
/// the pixel containing each sample; this keeps slow sub-pixel motion from shimmering
/// and never marks a pixel outside `[0, 64)` (a coordinate exactly 64 belongs to pixel 63).
pub fn draw_trail(canvas: &mut Canvas, trail: &Trail, now: u64, max_age_ticks: u64, color: [f32; 3]) {
    let _ = (canvas, trail, now, max_age_ticks, color);
    todo!("draw_trail")
}
