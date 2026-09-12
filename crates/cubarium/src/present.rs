//! The M2 ambient image, exactly as `design/m2-world-spec.md` "Presentation" describes it:
//! producer substrate, detritus flecks, short trails, bodies. Nothing else — no overlays,
//! no labels, no diagnostics.
//!
//! The presenter owns only renderer-side memory (one [`Trail`] per live organism). It
//! never reads the world: everything comes from the immutable [`RenderView`] published
//! after each completed tick.

use std::collections::HashMap;

use cubarium_core::view::{OrganismView, RenderView};
use cubarium_core::organism::Mode;
use cubarium_core::OrganismId;
use cubarium_render::{BodyShape, Canvas, Lobe, Trail, draw_field, draw_trail, stamp_body};
use cubarium_surface::{PixelImage, ScalarField, Travel, Vec2};

// --- Appearance constants ----------------------------------------------------------

/// Producer substrate, dim green, drawn with the seam-aware filter.
pub const SUBSTRATE_COLOR: [f32; 3] = [0.10, 0.40, 0.16];
/// Detritus flecks, warm brown, at cell level with no filter.
pub const DETRITUS_COLOR: [f32; 3] = [0.30, 0.18, 0.08];
/// Detritus below this (m per cell) shows nothing at all: flecks, not a wash.
pub const DETRITUS_THRESHOLD: f64 = 0.05;
/// Detritus saturates at 1 m per cell.
pub const DETRITUS_SCALE: f64 = 1.0;
/// Genome hue 0: warm, low saturation.
pub const HUE_WARM: [f32; 3] = [0.95, 0.70, 0.45];
/// Genome hue 1: cool, low saturation.
pub const HUE_COOL: [f32; 3] = [0.55, 0.75, 0.95];
/// Body brightness while Resting.
pub const BRIGHT_RESTING: f32 = 0.55;
/// Body brightness while Seeking or Feeding.
pub const BRIGHT_ACTIVE: f32 = 0.8;
/// The extra core-lobe stamp while an organism fed this tick.
pub const BRIGHT_FED_CORE: f32 = 1.0;
/// Juveniles are the same body at 0.7 scale (offsets and radii alike).
pub const JUVENILE_SCALE: f64 = 0.7;
/// Trail length in segments.
pub const TRAIL_SEGMENTS: usize = 12;
/// Trail fade length: 3 simulated seconds at 20 Hz.
pub const TRAIL_MAX_AGE_TICKS: u64 = 60;
/// Trails are the body color at this brightness.
pub const TRAIL_BRIGHTNESS: f32 = 0.25;

/// The warm-to-cool ramp: a linear blend in linear light.
pub fn hue_color(hue: f32) -> [f32; 3] {
    let t = hue.clamp(0.0, 1.0);
    [
        HUE_WARM[0] + (HUE_COOL[0] - HUE_WARM[0]) * t,
        HUE_WARM[1] + (HUE_COOL[1] - HUE_WARM[1]) * t,
        HUE_WARM[2] + (HUE_COOL[2] - HUE_WARM[2]) * t,
    ]
}

/// Brightness by mode: resting bodies sit back, active bodies come forward.
pub fn mode_brightness(mode: Mode) -> f32 {
    match mode {
        Mode::Resting => BRIGHT_RESTING,
        Mode::Seeking | Mode::Feeding => BRIGHT_ACTIVE,
    }
}

/// The body stamp for one organism view: the phenotype's lobes, scaled for juveniles.
pub fn body_shape(view: &OrganismView) -> BodyShape {
    let s = if view.juvenile { JUVENILE_SCALE } else { 1.0 };
    BodyShape {
        lobes: view
            .lobes
            .iter()
            .map(|&(x, y, r)| Lobe { offset: Vec2::new(x * s, y * s), radius: r * s })
            .collect(),
    }
}

fn scale(color: [f32; 3], k: f32) -> [f32; 3] {
    [color[0] * k, color[1] * k, color[2] * k]
}

/// Presentation state that outlives a single frame: one trail per live organism.
pub struct Presenter {
    trails: HashMap<OrganismId, Trail>,
    /// Reused so pushing a view's segments into a trail allocates nothing per organism.
    travel: Travel,
    producer: ScalarField,
    detritus: ScalarField,
    scratch: Vec<PixelImage>,
    live: Vec<OrganismId>,
}

impl Default for Presenter {
    fn default() -> Self {
        Presenter::new()
    }
}

impl Presenter {
    pub fn new() -> Presenter {
        Presenter {
            trails: HashMap::new(),
            travel: Travel::default(),
            producer: ScalarField::zeros(),
            detritus: ScalarField::zeros(),
            scratch: Vec::new(),
            live: Vec::new(),
        }
    }

    /// Live trails currently held (one per organism seen last tick).
    pub fn trail_count(&self) -> usize {
        self.trails.len()
    }

    /// Record one completed tick: append each organism's traveled segments to its trail
    /// and drop the trails of organisms that are gone. Call this once per simulation
    /// tick, not once per frame, so trail length is simulated time, not frame rate.
    pub fn observe(&mut self, view: &RenderView) {
        self.live.clear();
        for o in &view.organisms {
            self.live.push(o.id);
            let trail = self.trails.entry(o.id).or_insert_with(|| Trail::new(TRAIL_SEGMENTS));
            if !o.moved.is_empty() {
                // `Trail` takes a whole travel; the view already carries this tick's
                // per-chart segments, so the other travel fields are irrelevant here.
                self.travel.segments.clear();
                self.travel.segments.extend_from_slice(&o.moved);
                trail.push_travel(&self.travel, view.tick);
            }
            trail.prune(view.tick, TRAIL_MAX_AGE_TICKS);
        }
        if self.trails.len() != self.live.len() {
            // An ID that disappeared is gone for good: slots bump their generation on
            // reuse, so a recycled slot never inherits the dead organism's trail.
            let live: std::collections::HashSet<OrganismId> = self.live.iter().copied().collect();
            self.trails.retain(|id, _| live.contains(id));
        }
    }

    /// Draw the ambient image: substrate, detritus, trails, bodies, in that order.
    pub fn draw(&mut self, view: &RenderView, canvas: &mut Canvas) {
        canvas.clear();

        // Substrate: `P` against the configured carrying capacity, seam-aware filter on.
        copy_field(&mut self.producer, &view.producer);
        draw_field(canvas, &self.producer, view.producer_max, SUBSTRATE_COLOR, true);

        // Detritus: flecks only, at cell level, nearest, nothing below the threshold.
        threshold_field(&mut self.detritus, &view.detritus, DETRITUS_THRESHOLD);
        draw_field(canvas, &self.detritus, DETRITUS_SCALE, DETRITUS_COLOR, false);

        for o in &view.organisms {
            let color = hue_color(o.hue);
            if let Some(trail) = self.trails.get(&o.id) {
                draw_trail(
                    canvas,
                    trail,
                    view.tick,
                    TRAIL_MAX_AGE_TICKS,
                    scale(color, TRAIL_BRIGHTNESS),
                );
            }
        }

        for o in &view.organisms {
            let color = hue_color(o.hue);
            let shape = body_shape(o);
            stamp_body(
                canvas,
                o.pos,
                o.heading,
                &shape,
                scale(color, mode_brightness(o.mode)),
                &mut self.scratch,
            );
            if o.fed && let Some(core) = shape.lobes.first() {
                // A feeding organism's core brightens; only the core lobe is restamped.
                let core_only = BodyShape { lobes: vec![*core] };
                stamp_body(
                    canvas,
                    o.pos,
                    o.heading,
                    &core_only,
                    scale(color, BRIGHT_FED_CORE),
                    &mut self.scratch,
                );
            }
        }
    }
}

/// Copy a per-cell vector into a `ScalarField` (cell order is the field's own index order).
fn copy_field(out: &mut ScalarField, values: &[f64]) {
    let n = out.values.len().min(values.len());
    out.values[..n].copy_from_slice(&values[..n]);
    for v in out.values[n..].iter_mut() {
        *v = 0.0;
    }
}

/// Copy with everything at or below `threshold` zeroed, so `draw_field` paints flecks.
fn threshold_field(out: &mut ScalarField, values: &[f64], threshold: f64) {
    for (i, slot) in out.values.iter_mut().enumerate() {
        let v = values.get(i).copied().unwrap_or(0.0);
        *slot = if v > threshold { v } else { 0.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_proto::Face;
    use cubarium_surface::{CellId, PathSegment, SurfacePoint, cell_of};

    fn organism(id: u32, hue: f32, mode: Mode) -> OrganismView {
        OrganismView {
            id: OrganismId { slot: id, generation: 1 },
            pos: SurfacePoint::new(Face::Front, 32.0, 32.0),
            heading: Vec2::new(1.0, 0.0),
            lobes: vec![(0.0, 0.0, 1.4), (2.0, 0.0, 0.9)],
            hue,
            mode,
            fed: false,
            juvenile: false,
            moved: Vec::new(),
        }
    }

    fn empty_view() -> RenderView {
        RenderView {
            tick: 0,
            producer: vec![0.0; cubarium_surface::CELL_COUNT],
            detritus: vec![0.0; cubarium_surface::CELL_COUNT],
            producer_max: 2.0,
            organisms: Vec::new(),
        }
    }

    fn total(canvas: &Canvas) -> f64 {
        let mut t = 0.0;
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let p = canvas.get(face, x, y);
                    t += f64::from(p[0]) + f64::from(p[1]) + f64::from(p[2]);
                }
            }
        }
        t
    }

    #[test]
    fn the_hue_ramp_runs_warm_to_cool_and_clamps() {
        assert_eq!(hue_color(0.0), HUE_WARM);
        assert_eq!(hue_color(1.0), HUE_COOL);
        assert_eq!(hue_color(-5.0), HUE_WARM);
        assert_eq!(hue_color(5.0), HUE_COOL);
        let mid = hue_color(0.5);
        for i in 0..3 {
            assert!((mid[i] - (HUE_WARM[i] + HUE_COOL[i]) / 2.0).abs() < 1e-6);
        }
        // Warm is redder than blue, cool is bluer than red: the ramp has a direction.
        const { assert!(HUE_WARM[0] > HUE_WARM[2] && HUE_COOL[2] > HUE_COOL[0]) };
    }

    #[test]
    fn brightness_follows_mode() {
        assert_eq!(mode_brightness(Mode::Resting), 0.55);
        assert_eq!(mode_brightness(Mode::Seeking), 0.8);
        assert_eq!(mode_brightness(Mode::Feeding), 0.8);
    }

    #[test]
    fn juveniles_scale_offsets_and_radii_together() {
        let mut o = organism(0, 0.0, Mode::Resting);
        let adult = body_shape(&o);
        o.juvenile = true;
        let juvenile = body_shape(&o);
        for (a, j) in adult.lobes.iter().zip(juvenile.lobes.iter()) {
            assert!((j.radius - a.radius * 0.7).abs() < 1e-12);
            assert!((j.offset.x - a.offset.x * 0.7).abs() < 1e-12);
            assert!((j.offset.y - a.offset.y * 0.7).abs() < 1e-12);
        }
        assert!((juvenile.extent() - adult.extent() * 0.7).abs() < 1e-12);
    }

    #[test]
    fn detritus_below_the_threshold_paints_nothing() {
        let mut view = empty_view();
        let cell = CellId::new(Face::Front, 4, 4).index();
        view.detritus[cell] = DETRITUS_THRESHOLD;
        let mut p = Presenter::new();
        let mut canvas = Canvas::new();
        p.draw(&view, &mut canvas);
        assert_eq!(total(&canvas), 0.0, "a cell exactly at the threshold is not a fleck");

        view.detritus[cell] = 0.5;
        p.draw(&view, &mut canvas);
        // Exactly the 4x4 pixels of that cell, at half the fleck color, and nothing else.
        let mut lit = 0;
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    if canvas.get(face, x, y) != [0.0; 3] {
                        lit += 1;
                        assert_eq!(face, Face::Front);
                        assert_eq!(cell_of(&SurfacePoint::pixel_center(face, x, y)).index(), cell);
                        let p = canvas.get(face, x, y);
                        assert!((p[0] - DETRITUS_COLOR[0] * 0.5).abs() < 1e-6, "{p:?}");
                    }
                }
            }
        }
        assert_eq!(lit, 16);
    }

    #[test]
    fn the_substrate_scales_against_the_views_producer_max() {
        let mut view = empty_view();
        for v in view.producer.iter_mut() {
            *v = view.producer_max;
        }
        let mut p = Presenter::new();
        let mut canvas = Canvas::new();
        p.draw(&view, &mut canvas);
        // A saturated field paints the full substrate color everywhere.
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let px = canvas.get(face, x, y);
                    assert!((px[1] - SUBSTRATE_COLOR[1]).abs() < 1e-6, "{face:?} {x},{y}: {px:?}");
                }
            }
        }
    }

    #[test]
    fn feeding_adds_a_brighter_core_on_top_of_the_body() {
        let mut view = empty_view();
        let mut o = organism(0, 0.0, Mode::Feeding);
        o.fed = false;
        view.organisms = vec![o.clone()];
        let mut p = Presenter::new();
        let mut canvas = Canvas::new();
        p.draw(&view, &mut canvas);
        let unfed = total(&canvas);

        o.fed = true;
        view.organisms = vec![o];
        p.draw(&view, &mut canvas);
        let fed = total(&canvas);
        assert!(fed > unfed, "fed {fed} must be brighter than unfed {unfed}");
    }

    #[test]
    fn trails_follow_live_ids_and_vanish_with_them() {
        let mut view = empty_view();
        let mut a = organism(0, 0.0, Mode::Seeking);
        a.moved = vec![PathSegment {
            face: Face::Front,
            from: Vec2::new(30.0, 32.0),
            to: Vec2::new(32.0, 32.0),
        }];
        let b = organism(1, 1.0, Mode::Resting);
        view.organisms = vec![a, b.clone()];
        let mut p = Presenter::new();
        p.observe(&view);
        assert_eq!(p.trail_count(), 2);

        view.tick = 1;
        view.organisms = vec![b];
        p.observe(&view);
        assert_eq!(p.trail_count(), 1, "a vanished ID drops its trail");

        view.tick = 2;
        view.organisms.clear();
        p.observe(&view);
        assert_eq!(p.trail_count(), 0);
    }

    #[test]
    fn a_trail_is_bounded_by_segments_and_by_age() {
        let mut view = empty_view();
        let mut o = organism(0, 0.0, Mode::Seeking);
        o.moved = vec![PathSegment {
            face: Face::Front,
            from: Vec2::new(10.0, 32.0),
            to: Vec2::new(10.5, 32.0),
        }];
        let mut p = Presenter::new();
        for tick in 0..200u64 {
            view.tick = tick;
            view.organisms = vec![o.clone()];
            p.observe(&view);
        }
        let id = OrganismId { slot: 0, generation: 1 };
        assert_eq!(p.trails[&id].len(), TRAIL_SEGMENTS);
        // Everything held is inside the fade window.
        for s in p.trails[&id].segments() {
            assert!(199u64.saturating_sub(s.tick) <= TRAIL_MAX_AGE_TICKS);
        }
    }

    #[test]
    fn an_empty_world_paints_nothing_at_all() {
        let mut p = Presenter::new();
        let mut canvas = Canvas::new();
        p.draw(&empty_view(), &mut canvas);
        assert_eq!(total(&canvas), 0.0);
    }
}
