//! Renderer-only history trails made of actual transported path segments.

use std::collections::VecDeque;

use crate::Canvas;
use cube_proto::{FACE_SIZE, Face, NUM_FACES};
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
    // Brightness is resolved per pixel as a maximum before anything reaches the canvas,
    // so overlapping segments never accumulate.
    let mut bright = vec![0.0f32; NUM_FACES * FACE_SIZE * FACE_SIZE];
    for ts in trail.segments() {
        let age = now.saturating_sub(ts.tick);
        if max_age_ticks > 0 && age > max_age_ticks {
            continue;
        }
        let fade = if max_age_ticks > 0 {
            1.0 - (age as f64) / (max_age_ticks as f64)
        } else {
            1.0
        };
        let fade = fade.clamp(0.0, 1.0) as f32;
        if fade <= 0.0 {
            continue;
        }
        let seg = ts.segment;
        let d = seg.to - seg.from;
        // At most half a pixel per step, so slow sub-pixel motion never skips or shimmers.
        let steps = ((d.length() / 0.5).ceil() as u32).max(1);
        let base = seg.face.index() * FACE_SIZE * FACE_SIZE;
        for i in 0..=steps {
            let p = seg.from + d * (f64::from(i) / f64::from(steps));
            let (x, y) = pixel_of(p.x, p.y);
            let cell = &mut bright[base + y * FACE_SIZE + x];
            if fade > *cell {
                *cell = fade;
            }
        }
    }
    for face in Face::ALL {
        let base = face.index() * FACE_SIZE * FACE_SIZE;
        for y in 0..FACE_SIZE {
            for x in 0..FACE_SIZE {
                let f = bright[base + y * FACE_SIZE + x];
                if f > 0.0 {
                    canvas.add(
                        face,
                        x as u8,
                        y as u8,
                        [
                            (color[0] * f).clamp(0.0, 1.0),
                            (color[1] * f).clamp(0.0, 1.0),
                            (color[2] * f).clamp(0.0, 1.0),
                        ],
                    );
                }
            }
        }
    }
}

/// The pixel containing a chart coordinate; a coordinate exactly 64 (or any transient
/// boundary value) belongs to pixel 63, and nothing ever lands outside `[0, 64)`.
#[inline]
fn pixel_of(u: f64, v: f64) -> (usize, usize) {
    let clamp = |c: f64| c.floor().clamp(0.0, (FACE_SIZE - 1) as f64) as usize;
    (clamp(u), clamp(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_surface::{SurfacePoint, Vec2, travel};

    fn lit(canvas: &Canvas) -> Vec<(Face, u8, u8, f32)> {
        let mut v = Vec::new();
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let p = canvas.get(face, x, y);
                    if p[0] != 0.0 || p[1] != 0.0 || p[2] != 0.0 {
                        v.push((face, x, y, p[0]));
                    }
                }
            }
        }
        v
    }

    /// Push a real swept path that crosses seams and reflects off the rim.
    fn wandering_trail(steps: u64) -> (Trail, u64) {
        let mut trail = Trail::new(1024);
        let mut p = SurfacePoint::new(cube_proto::Face::Front, 3.0, 3.0);
        let mut h = Vec2::new(0.93, 0.37).normalized().unwrap();
        for t in 0..steps {
            let tr = travel(p, h * 3.7);
            trail.push_travel(&tr, t);
            p = tr.end;
            h = tr.map.apply(h);
        }
        (trail, steps)
    }

    #[test]
    fn trail_pixels_stay_inside_their_charts() {
        let (trail, now) = wandering_trail(400);
        assert!(trail.len() > 400, "a wandering path should split at seams");
        let mut canvas = Canvas::new();
        draw_trail(&mut canvas, &trail, now, 160, [0.1, 0.35, 0.3]);
        let pixels = lit(&canvas);
        assert!(!pixels.is_empty());
        for (_, x, y, _) in &pixels {
            assert!(*x < 64 && *y < 64);
        }
        // Only segments within the age window contributed.
        let faces: std::collections::HashSet<Face> = pixels.iter().map(|p| p.0).collect();
        assert!(faces.len() > 1, "the path should cross seams: {faces:?}");
    }

    #[test]
    fn a_segment_running_into_the_corner_lands_on_pixel_63() {
        let mut trail = Trail::new(8);
        let tr = travel(SurfacePoint::new(cube_proto::Face::Front, 60.0, 60.0), Vec2::new(3.5, 3.5));
        trail.push_travel(&tr, 0);
        let mut canvas = Canvas::new();
        draw_trail(&mut canvas, &trail, 0, 160, [1.0, 1.0, 1.0]);
        for (_, x, y, _) in lit(&canvas) {
            assert!(x < 64 && y < 64);
        }
        assert!(canvas.get(cube_proto::Face::Front, 63, 63)[0] > 0.0);
    }

    #[test]
    fn overlapping_segments_keep_the_maximum_not_the_sum() {
        let mut trail = Trail::new(16);
        // The same short segment traveled twice, at different ages.
        for tick in [0u64, 100] {
            let tr = travel(SurfacePoint::new(cube_proto::Face::Left, 20.0, 20.0), Vec2::new(2.0, 0.0));
            trail.push_travel(&tr, tick);
        }
        let mut canvas = Canvas::new();
        draw_trail(&mut canvas, &trail, 100, 200, [1.0, 1.0, 1.0]);
        // Ages 100 and 0 give fades 0.5 and 1.0; the maximum wins, nothing sums to 1.5.
        let peak = lit(&canvas).iter().map(|p| p.3).fold(0.0f32, f32::max);
        assert!((peak - 1.0).abs() < 1e-6, "peak {peak}");
        for (_, _, _, v) in lit(&canvas) {
            assert!(v <= 1.0 + 1e-6);
        }
    }

    #[test]
    fn segments_older_than_the_window_are_not_drawn() {
        let mut trail = Trail::new(16);
        let tr = travel(SurfacePoint::new(cube_proto::Face::Back, 10.0, 10.0), Vec2::new(4.0, 0.0));
        trail.push_travel(&tr, 0);
        let mut canvas = Canvas::new();
        draw_trail(&mut canvas, &trail, 500, 160, [1.0, 1.0, 1.0]);
        assert!(lit(&canvas).is_empty());
    }

    #[test]
    fn prune_drops_only_the_old_front() {
        let mut trail = Trail::new(64);
        for t in 0..10u64 {
            let tr = travel(SurfacePoint::new(cube_proto::Face::Front, 10.0, 10.0), Vec2::new(1.0, 0.0));
            trail.push_travel(&tr, t);
        }
        assert_eq!(trail.len(), 10);
        trail.prune(9, 4);
        assert_eq!(trail.len(), 5);
        trail.clear();
        assert!(trail.is_empty());
    }
}
