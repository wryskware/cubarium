//! The M2 ambient image, exactly as `design/m2-world-spec.md` "Presentation" describes it:
//! producer substrate, detritus flecks, short trails, bodies. Nothing else — no overlays,
//! no labels, no diagnostics. The colors are the Outrun palette decided in
//! `design/appearance.md` "Palette".
//!
//! The presenter owns only renderer-side memory (one [`Trail`] per live organism). It
//! never reads the world: everything comes from the immutable [`RenderView`] published
//! after each completed tick.
//!
//! Rendering runs faster than the simulation (`--fps`, default 60, against a 20 Hz
//! world), so [`Presenter::draw`] takes the clock's interpolation fraction `f` and draws
//! each body a fraction `f` along the path it traveled *during the last tick*
//! ([`OrganismView::moved`]). At `f = 0` a body sits where the tick began and at
//! `f → 1` it approaches `pos`: the motion shown is one tick old, which is the price of
//! showing it smoothly at all. Fields and trails are not interpolated.

use std::sync::LazyLock;

use cubarium_core::view::{OrganismView, RenderView};
use cubarium_core::organism::Mode;
use cubarium_core::OrganismId;
use cubarium_render::{
    BodyShape, Canvas, Lobe, Trail, draw_field, draw_trail, srgb_decode, stamp_body,
};
use cubarium_surface::{
    Edge, FACE_EXTENT, PathSegment, PixelImage, ScalarField, SurfacePoint, Travel, Vec2, cell_of,
    pixel_neighbor,
};
use cube_proto::{FACE_SIZE, Face};
use std::collections::HashMap;

// --- Appearance constants ----------------------------------------------------------
//
// Every color below is an sRGB hex value from `design/appearance.md` "Palette",
// decoded to linear light once into [`PALETTE`]. Hex is what the palette was chosen and
// reviewed in; the canvas is linear light, and mixing the two is how a ramp ends up the
// wrong color in the middle.

/// Uniform night floor, added to every pixel before anything else so "empty" reads as
/// dark rather than off.
pub const FLOOR_SRGB: u32 = 0x0012_093A;
/// The floor is that color at this brightness.
pub const FLOOR_BRIGHTNESS: f32 = 0.12;
/// Producer substrate at zero density: deep indigo.
pub const PRODUCER_LOW_SRGB: u32 = 0x001E_2798;
/// The ramp saturates at this fraction of `P_max`: the measured standing crop peaks near
/// 0.3-0.5 of capacity, so the logistic ceiling itself is never a useful white point.
pub const PRODUCER_SATURATION: f64 = 0.6;
/// Brightness multipliers of the ramp color at zero and full density.
pub const RAMP_MIN_BRIGHTNESS: f32 = 0.06;
pub const RAMP_MAX_BRIGHTNESS: f32 = 0.55;
/// Producer substrate at `P_max`: electric blue-cyan. Rich patches turn cyan.
pub const PRODUCER_HIGH_SRGB: u32 = 0x0042_C5F8;
/// Detritus flecks: dim violet.
pub const DETRITUS_SRGB: u32 = 0x0051_0B6D;
/// Genome hue 0.
pub const HUE_MAGENTA_SRGB: u32 = 0x00FF_2AFC;
/// Genome hue 1.
pub const HUE_CYAN_SRGB: u32 = 0x0042_C6FF;
/// The feeding flash on the core lobe — the only warm color in the image.
pub const FEEDING_SRGB: u32 = 0x00FF_9B50;

/// Detritus below this (m per cell) shows nothing at all: flecks, not a wash.
pub const DETRITUS_THRESHOLD: f64 = 0.05;
/// Detritus saturates at 1.5 m per cell.
pub const DETRITUS_SCALE: f64 = 1.5;
/// Body brightness while Resting.
pub const BRIGHT_RESTING: f32 = 0.55;
/// Body brightness while Seeking or Feeding.
pub const BRIGHT_ACTIVE: f32 = 0.8;
/// The feeding flash is drawn on the core lobe at full brightness.
pub const BRIGHT_FED_CORE: f32 = 1.0;
/// Juveniles are the same body at 0.7 scale (offsets and radii alike).
pub const JUVENILE_SCALE: f64 = 0.7;
/// Trail length in segments.
pub const TRAIL_SEGMENTS: usize = 12;
/// Trail fade length: 3 simulated seconds at 20 Hz.
pub const TRAIL_MAX_AGE_TICKS: u64 = 60;
/// Trails are the body color at this brightness.
pub const TRAIL_BRIGHTNESS: f32 = 0.25;

/// `0xRRGGBB` to linear light, through the canvas's own sRGB transfer function.
pub fn srgb_linear(hex: u32) -> [f32; 3] {
    [
        srgb_decode(((hex >> 16) & 0xFF) as u8),
        srgb_decode(((hex >> 8) & 0xFF) as u8),
        srgb_decode((hex & 0xFF) as u8),
    ]
}

/// The palette in linear light, decoded once on first use.
pub struct Palette {
    pub floor: [f32; 3],
    pub producer_low: [f32; 3],
    pub producer_high: [f32; 3],
    pub detritus: [f32; 3],
    pub hue_magenta: [f32; 3],
    pub hue_cyan: [f32; 3],
    pub feeding: [f32; 3],
}

/// The decoded palette. `srgb_decode` is not a `const fn` (it needs `powf`), so the
/// tables are built once at first use instead of at compile time.
pub static PALETTE: LazyLock<Palette> = LazyLock::new(|| Palette {
    floor: scale(srgb_linear(FLOOR_SRGB), FLOOR_BRIGHTNESS),
    producer_low: srgb_linear(PRODUCER_LOW_SRGB),
    producer_high: srgb_linear(PRODUCER_HIGH_SRGB),
    detritus: srgb_linear(DETRITUS_SRGB),
    hue_magenta: srgb_linear(HUE_MAGENTA_SRGB),
    hue_cyan: srgb_linear(HUE_CYAN_SRGB),
    feeding: srgb_linear(FEEDING_SRGB),
});

/// Linear blend between two linear-light colors. The ends are exact: `t <= 0` is `a` and
/// `t >= 1` is `b`, rather than `a + (b - a)` rounding to something a shade off.
pub fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    // `NaN` and every non-positive blend land on `a`.
    if t.is_nan() || t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// The body hue ramp: magenta at hue 0, cyan at hue 1, blended in linear light.
pub fn hue_color(hue: f32) -> [f32; 3] {
    mix(PALETTE.hue_magenta, PALETTE.hue_cyan, hue)
}

/// Brightness by mode: resting bodies sit back, active bodies come forward.
pub fn mode_brightness(mode: Mode) -> f32 {
    match mode {
        Mode::Resting => BRIGHT_RESTING,
        Mode::Seeking | Mode::Feeding => BRIGHT_ACTIVE,
    }
}

/// Add the uniform floor to every pixel. Called before the substrate, so a cell with no
/// producer still reads as night rather than as a dead panel.
pub fn draw_floor(canvas: &mut Canvas) {
    let floor = PALETTE.floor;
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                canvas.add(face, x, y, floor);
            }
        }
    }
}

/// `draw_field` with a color *ramp*: each pixel takes `t = min(value / scale, 1)`, the
/// color `mix(low, high, t)`, and the intensity `t` — so density moves the substrate
/// from indigo to cyan while an empty cell still paints nothing.
///
/// This lives here rather than in `cubarium-render` because the ramp is a presentation
/// decision, not a rasterization one; the seam-aware filter is exactly `draw_field`'s,
/// including the rim normalization that keeps the open bottom edge from darkening.
pub fn draw_ramp_field(
    canvas: &mut Canvas,
    field: &ScalarField,
    scale_to: f64,
    low: [f32; 3],
    high: [f32; 3],
    filter: bool,
) {
    if scale_to.is_nan() || scale_to <= 0.0 {
        return;
    }
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let own = field.get(cell_of(&SurfacePoint::pixel_center(face, x, y)));
                let value = if filter {
                    let mut sum = own * 4.0;
                    let mut weight = 4.0;
                    for edge in Edge::ALL {
                        if let Some((nf, nx, ny)) = pixel_neighbor(face, x, y, edge) {
                            sum += field.get(cell_of(&SurfacePoint::pixel_center(nf, nx, ny)));
                            weight += 1.0;
                        }
                    }
                    sum / weight
                } else {
                    own
                };
                let t = (value / scale_to).min(1.0);
                if t != 0.0 {
                    let t = t as f32;
                    // Hue follows density linearly; brightness follows its square so the
                    // ordinary standing crop stays a dim floor and only rich patches glow.
                    let c = mix(low, high, t);
                    let b = RAMP_MIN_BRIGHTNESS + (RAMP_MAX_BRIGHTNESS - RAMP_MIN_BRIGHTNESS) * t * t;
                    canvas.add(face, x, y, [c[0] * b, c[1] * b, c[2] * b]);
                }
            }
        }
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

/// Where to draw a body between ticks.
///
/// `moved` is the path the organism traveled during the last completed tick, already
/// split per chart, and `pos`/`heading` are where that tick left it. For a render
/// fraction `f ∈ [0, 1)` this returns the point a fraction `f` along the concatenated
/// path *by arc length*, on whichever chart that point falls, and the unit direction of
/// that segment. `f = 0` is the start of the path — so the image lags the simulation by
/// one tick and never extrapolates ahead of state that exists.
///
/// Degenerate inputs fall back to the end-of-tick state: an empty path, a path of zero
/// total length, or a segment whose direction cannot be normalized keeps the view's own
/// `pos` or `heading`.
pub fn interpolate(
    moved: &[PathSegment],
    pos: SurfacePoint,
    heading: Vec2,
    f: f64,
) -> (SurfacePoint, Vec2) {
    if moved.is_empty() {
        return (pos, heading);
    }
    let f = if f.is_finite() { f.clamp(0.0, 1.0) } else { 0.0 };
    let total: f64 = moved.iter().map(PathSegment::length).sum();
    if total.is_nan() || total <= 0.0 {
        return (anchor_on(moved[0].face, moved[0].from), heading);
    }
    let target = f * total;
    let mut walked = 0.0;
    for (i, seg) in moved.iter().enumerate() {
        let len = seg.length();
        let last = i + 1 == moved.len();
        if walked + len >= target || last {
            let t = if len > 0.0 { ((target - walked) / len).clamp(0.0, 1.0) } else { 0.0 };
            let delta = seg.to - seg.from;
            let point = seg.from + delta * t;
            let dir = delta.normalized().unwrap_or(heading);
            return (anchor_on(seg.face, point), dir);
        }
        walked += len;
    }
    unreachable!("the last segment always returns")
}

/// A chart point as a canonical [`SurfacePoint`]. Interpolated points can land exactly on
/// a chart boundary (a segment ends there whenever the path crossed a seam), which is a
/// transient coordinate, not a canonical one.
fn anchor_on(face: Face, p: Vec2) -> SurfacePoint {
    let clamp = |c: f64| if c.is_finite() { c.clamp(0.0, FACE_EXTENT) } else { 0.0 };
    SurfacePoint::new(face, clamp(p.x), clamp(p.y)).canonicalize()
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

    /// Draw the ambient image: floor, substrate, detritus, trails, bodies, in that
    /// order. `f` is the clock's interpolation fraction for this frame (see
    /// [`interpolate`]); pass 0 to draw the state of the last completed tick exactly.
    pub fn draw(&mut self, view: &RenderView, f: f64, canvas: &mut Canvas) {
        canvas.clear();

        // The night floor, under everything.
        draw_floor(canvas);

        // Substrate: `P` against the configured carrying capacity, indigo to cyan by
        // density, seam-aware filter on.
        copy_field(&mut self.producer, &view.producer);
        draw_ramp_field(
            canvas,
            &self.producer,
            view.producer_max * PRODUCER_SATURATION,
            PALETTE.producer_low,
            PALETTE.producer_high,
            true,
        );

        // Detritus: flecks only, at cell level, nearest, nothing below the threshold.
        threshold_field(&mut self.detritus, &view.detritus, DETRITUS_THRESHOLD);
        draw_field(canvas, &self.detritus, DETRITUS_SCALE, PALETTE.detritus, false);

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
            // Between ticks the body walks the path it took during the last tick.
            let (anchor, heading) = interpolate(&o.moved, o.pos, o.heading, f);
            stamp_body(
                canvas,
                anchor,
                heading,
                &shape,
                scale(color, mode_brightness(o.mode)),
                &mut self.scratch,
            );
            if o.fed && let Some(core) = shape.lobes.first() {
                // A feeding organism's core flashes warm; only the core lobe is restamped.
                let core_only = BodyShape { lobes: vec![*core] };
                stamp_body(
                    canvas,
                    anchor,
                    heading,
                    &core_only,
                    scale(PALETTE.feeding, BRIGHT_FED_CORE),
                    &mut self.scratch,
                );
            }
        }
    }
}

/// Copy a per-cell vector into a `ScalarField` (cell order is the field's own index order).
pub(crate) fn copy_field(out: &mut ScalarField, values: &[f64]) {
    let n = out.values.len().min(values.len());
    out.values[..n].copy_from_slice(&values[..n]);
    for v in out.values[n..].iter_mut() {
        *v = 0.0;
    }
}

/// Copy with everything at or below `threshold` zeroed, so `draw_field` paints flecks.
pub(crate) fn threshold_field(out: &mut ScalarField, values: &[f64], threshold: f64) {
    for (i, slot) in out.values.iter_mut().enumerate() {
        let v = values.get(i).copied().unwrap_or(0.0);
        *slot = if v > threshold { v } else { 0.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_surface::{CellId, cell_of, travel};

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
            gestation: None,
            form: u8::MAX,
            moved: Vec::new(),
        }
    }

    fn empty_view() -> RenderView {
        RenderView {
            tick: 0,
            producer: vec![0.0; cubarium_surface::CELL_COUNT],
            detritus: vec![0.0; cubarium_surface::CELL_COUNT],
            fruit: vec![0.0; cubarium_surface::CELL_COUNT],
            water: vec![0.0; cubarium_surface::CELL_COUNT],
            rain: vec![0.0; cubarium_surface::CELL_COUNT],
            producer_max: 2.0,
            organisms: Vec::new(),
        }
    }

    /// The floor is on every pixel, so tests that care about what was *drawn* subtract
    /// it before looking.
    fn above_floor(canvas: &Canvas, face: Face, x: u8, y: u8) -> [f32; 3] {
        let px = canvas.get(face, x, y);
        [px[0] - PALETTE.floor[0], px[1] - PALETTE.floor[1], px[2] - PALETTE.floor[2]]
    }

    fn floor_total() -> f64 {
        5.0 * 64.0 * 64.0 * PALETTE.floor.iter().map(|&c| f64::from(c)).sum::<f64>()
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
    fn the_hue_ramp_runs_magenta_to_cyan_and_clamps() {
        assert_eq!(hue_color(0.0), PALETTE.hue_magenta);
        assert_eq!(hue_color(1.0), PALETTE.hue_cyan);
        assert_eq!(hue_color(-5.0), PALETTE.hue_magenta);
        assert_eq!(hue_color(5.0), PALETTE.hue_cyan);
        let mid = hue_color(0.5);
        for (i, c) in mid.iter().enumerate() {
            assert!((c - (PALETTE.hue_magenta[i] + PALETTE.hue_cyan[i]) / 2.0).abs() < 1e-6);
        }
        // Magenta is redder than green, cyan is greener than red: the ramp has a
        // direction, and both ends are blue-heavy.
        assert!(PALETTE.hue_magenta[0] > PALETTE.hue_magenta[1]);
        assert!(PALETTE.hue_cyan[1] > PALETTE.hue_cyan[0]);
    }

    #[test]
    fn the_palette_decodes_the_appearance_hex_values() {
        // `#FF2AFC`: the decoded value is the sRGB transfer function applied to each
        // byte, so full-scale channels stay 1.0 and mid ones drop well below 0.5.
        assert_eq!(srgb_linear(0x00FF_FFFF), [1.0, 1.0, 1.0]);
        assert_eq!(srgb_linear(0x0000_0000), [0.0, 0.0, 0.0]);
        assert_eq!(PALETTE.hue_magenta[0], 1.0);
        assert!((PALETTE.hue_magenta[1] - srgb_decode(0x2A)).abs() < 1e-9);
        // The floor is dim: 12 % of an already dark indigo.
        for c in PALETTE.floor {
            assert!(c < 0.01, "the floor must stay a night floor: {:?}", PALETTE.floor);
        }
        // The producer ramp gets brighter and bluer-to-cyan with density.
        assert!(PALETTE.producer_high[1] > PALETTE.producer_low[1]);
        assert!(PALETTE.producer_high[2] > PALETTE.producer_low[2]);
        // The feeding flash is the only warm color: red-dominant.
        assert!(PALETTE.feeding[0] > PALETTE.feeding[1] && PALETTE.feeding[1] > PALETTE.feeding[2]);
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
        p.draw(&view, 0.0, &mut canvas);
        assert!(
            (total(&canvas) - floor_total()).abs() < 1e-3,
            "a cell exactly at the threshold is not a fleck"
        );

        view.detritus[cell] = 0.5;
        p.draw(&view, 0.0, &mut canvas);
        // Exactly the 4x4 pixels of that cell, at 0.5 / DETRITUS_SCALE of the fleck color
        // above the floor, and nothing else.
        let mut lit = 0;
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let px = above_floor(&canvas, face, x, y);
                    if px.iter().any(|&c| c.abs() > 1e-9) {
                        lit += 1;
                        assert_eq!(face, Face::Front);
                        assert_eq!(cell_of(&SurfacePoint::pixel_center(face, x, y)).index(), cell);
                        let want = PALETTE.detritus[0] * (0.5 / DETRITUS_SCALE) as f32;
                        assert!((px[0] - want).abs() < 1e-6, "{px:?}");
                    }
                }
            }
        }
        assert_eq!(lit, 16);
    }

    #[test]
    fn the_substrate_ramps_from_indigo_to_cyan_against_the_views_producer_max() {
        let mut view = empty_view();
        for v in view.producer.iter_mut() {
            *v = view.producer_max;
        }
        let mut p = Presenter::new();
        let mut canvas = Canvas::new();
        p.draw(&view, 0.0, &mut canvas);
        // A saturated field paints the cyan end at the ramp's maximum brightness everywhere,
        // over the floor.
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let px = above_floor(&canvas, face, x, y);
                    assert!(
                        (px[1] - PALETTE.producer_high[1] * RAMP_MAX_BRIGHTNESS).abs() < 1e-6,
                        "{face:?} {x},{y}: {px:?}"
                    );
                }
            }
        }

        // Half the saturation point is halfway along the hue ramp, at the squared
        // brightness: a dimmer, bluer substrate, never the cyan end at reduced brightness.
        let t = 0.5f32;
        for v in view.producer.iter_mut() {
            *v = view.producer_max * PRODUCER_SATURATION * f64::from(t);
        }
        p.draw(&view, 0.0, &mut canvas);
        let px = above_floor(&canvas, Face::Front, 32, 32);
        let want = mix(PALETTE.producer_low, PALETTE.producer_high, t);
        let b = RAMP_MIN_BRIGHTNESS + (RAMP_MAX_BRIGHTNESS - RAMP_MIN_BRIGHTNESS) * t * t;
        for i in 0..3 {
            assert!((px[i] - want[i] * b).abs() < 1e-6, "{px:?} vs {want:?}");
        }
        assert!(px[2] > px[1], "at half density the substrate is still blue: {px:?}");
    }

    #[test]
    fn the_floor_is_under_every_pixel_and_nothing_else_is() {
        let mut p = Presenter::new();
        let mut canvas = Canvas::new();
        p.draw(&empty_view(), 0.0, &mut canvas);
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    assert_eq!(canvas.get(face, x, y), PALETTE.floor, "{face:?} {x},{y}");
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
        p.draw(&view, 0.0, &mut canvas);
        let unfed = total(&canvas);

        o.fed = true;
        view.organisms = vec![o];
        p.draw(&view, 0.0, &mut canvas);
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
    fn an_empty_world_paints_only_the_floor() {
        let mut p = Presenter::new();
        let mut canvas = Canvas::new();
        p.draw(&empty_view(), 0.0, &mut canvas);
        assert!((total(&canvas) - floor_total()).abs() < 1e-3, "{}", total(&canvas));
    }

    // --- Interpolation along the transported path ----------------------------------

    fn seg(face: Face, from: (f64, f64), to: (f64, f64)) -> PathSegment {
        PathSegment { face, from: Vec2::new(from.0, from.1), to: Vec2::new(to.0, to.1) }
    }

    #[test]
    fn an_empty_path_draws_at_the_end_of_tick_state() {
        let o = organism(0, 0.0, Mode::Resting);
        let (a, h) = interpolate(&o.moved, o.pos, o.heading, 0.0);
        assert_eq!(a, o.pos);
        assert_eq!(h, o.heading);
        let (a, h) = interpolate(&o.moved, o.pos, o.heading, 0.75);
        assert_eq!(a, o.pos);
        assert_eq!(h, o.heading);
    }

    #[test]
    fn fraction_zero_is_the_start_of_the_tick_and_one_is_its_end() {
        let mut o = organism(0, 0.0, Mode::Seeking);
        o.moved = vec![seg(Face::Front, (30.0, 32.0), (32.0, 32.0))];
        o.pos = SurfacePoint::new(Face::Front, 32.0, 32.0);

        let (start, h) = interpolate(&o.moved, o.pos, o.heading, 0.0);
        assert_eq!((start.face, start.u, start.v), (Face::Front, 30.0, 32.0));
        assert!((h.x - 1.0).abs() < 1e-12 && h.y.abs() < 1e-12);

        let (mid, _) = interpolate(&o.moved, o.pos, o.heading, 0.5);
        assert!((mid.u - 31.0).abs() < 1e-12, "{mid:?}");

        // Just below 1 lands within 1e-9 of the end of the path, which is `pos`.
        let (end, _) = interpolate(&o.moved, o.pos, o.heading, 1.0f64.next_down());
        assert!((end.u - o.pos.u).abs() < 1e-9 && (end.v - o.pos.v).abs() < 1e-9, "{end:?}");
        assert_eq!(end.face, o.pos.face);
    }

    #[test]
    fn the_walk_is_by_arc_length_across_segments() {
        // Three pixels on Front then one on Right: at f = 0.75 the point is exactly the
        // seam, and past it the walk continues on the second chart.
        let moved = vec![
            seg(Face::Front, (61.0, 10.0), (64.0, 10.0)),
            seg(Face::Right, (0.0, 10.0), (1.0, 10.0)),
        ];
        let pos = SurfacePoint::new(Face::Right, 1.0, 10.0);
        let heading = Vec2::new(1.0, 0.0);

        let (a, _) = interpolate(&moved, pos, heading, 0.5);
        assert_eq!(a.face, Face::Front);
        assert!((a.u - 63.0).abs() < 1e-12, "{a:?}");

        let (a, _) = interpolate(&moved, pos, heading, 0.9);
        assert_eq!(a.face, Face::Right, "past the seam the anchor is on the next chart");
        assert!((a.u - 0.6).abs() < 1e-9, "{a:?}");
    }

    #[test]
    fn a_path_across_a_seam_yields_anchors_on_both_faces() {
        // A real transported path, not a hand-built one: sweep off the Front/Right seam.
        let start = SurfacePoint::new(Face::Front, 63.0, 20.0);
        let t = travel(start, Vec2::new(3.0, 0.0));
        assert!(t.crossings >= 1, "this sweep must cross a seam: {t:?}");
        let heading = t.map.apply(Vec2::new(1.0, 0.0));
        let mut faces = std::collections::HashSet::new();
        for i in 0..=100 {
            let f = f64::from(i) / 101.0;
            let (a, h) = interpolate(&t.segments, t.end, heading, f);
            assert!(a.is_canonical(), "f {f}: non-canonical anchor {a:?}");
            assert!((h.length() - 1.0).abs() < 1e-9, "f {f}: heading {h:?} is not a unit vector");
            faces.insert(a.face);
        }
        assert!(faces.len() >= 2, "a seam-crossing path must anchor on both charts: {faces:?}");
        assert!(faces.contains(&Face::Front) && faces.contains(&Face::Right), "{faces:?}");

        // The end of the walk is the end of the path, which is where the tick left the
        // organism: `f → 1` converges on `pos` and never past it.
        let (end, _) = interpolate(&t.segments, t.end, heading, 1.0f64.next_down());
        assert_eq!(end.face, t.end.face);
        assert!((end.u - t.end.u).abs() < 1e-9 && (end.v - t.end.v).abs() < 1e-9, "{end:?}");
        let last = t.segments.last().unwrap();
        assert!((end.u - last.to.x).abs() < 1e-9 || (last.to.x - FACE_EXTENT).abs() < 1e-9);
    }

    #[test]
    fn interpolation_never_produces_a_non_canonical_point() {
        // Every direction out of a few awkward places, over a whole tick's worth of
        // fractions, including sweeps that reflect off the rim and cross the Top seams.
        let starts = [
            SurfacePoint::new(Face::Front, 63.9, 63.9),
            SurfacePoint::new(Face::Top, 0.1, 0.1),
            SurfacePoint::new(Face::Right, 63.99, 0.01),
            SurfacePoint::new(Face::Back, 32.0, 63.5),
        ];
        for start in starts {
            for step in 0..24 {
                let angle = f64::from(step) * std::f64::consts::TAU / 24.0;
                let t = travel(start, Vec2::from_screen_angle(angle) * 5.0);
                let heading = t.map.apply(Vec2::from_screen_angle(angle));
                for i in 0..=32 {
                    let f = f64::from(i) / 33.0;
                    let (a, _) = interpolate(&t.segments, t.end, heading, f);
                    assert!(a.is_canonical(), "{start:?} angle {angle} f {f}: {a:?}");
                }
                // Out-of-range fractions are clamped, never extrapolated.
                for f in [-1.0, 1.0, 2.0, f64::NAN] {
                    let (a, _) = interpolate(&t.segments, t.end, heading, f);
                    assert!(a.is_canonical(), "{start:?} f {f}: {a:?}");
                }
            }
        }
    }

    #[test]
    fn a_body_drawn_between_ticks_moves_with_the_fraction() {
        let mut view = empty_view();
        let mut o = organism(0, 0.0, Mode::Seeking);
        o.pos = SurfacePoint::new(Face::Front, 40.0, 32.0);
        o.moved = vec![seg(Face::Front, (20.0, 32.0), (40.0, 32.0))];
        view.organisms = vec![o];
        let mut p = Presenter::new();
        let mut canvas = Canvas::new();

        let centroid = |canvas: &Canvas| {
            let (mut sum, mut weight) = (0.0f64, 0.0f64);
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let px = above_floor(canvas, Face::Front, x, y);
                    let w = f64::from(px[0].max(0.0) + px[1].max(0.0) + px[2].max(0.0));
                    sum += w * f64::from(x);
                    weight += w;
                }
            }
            assert!(weight > 0.0, "the body must be visible");
            sum / weight
        };

        p.draw(&view, 0.0, &mut canvas);
        let at_start = centroid(&canvas);
        p.draw(&view, 0.5, &mut canvas);
        let halfway = centroid(&canvas);
        p.draw(&view, 1.0f64.next_down(), &mut canvas);
        let at_end = centroid(&canvas);

        assert!((at_start - 20.0).abs() < 1.0, "f=0 draws at the start of the tick: {at_start}");
        assert!((halfway - 30.0).abs() < 1.0, "f=0.5 draws halfway: {halfway}");
        assert!((at_end - 40.0).abs() < 1.0, "f→1 approaches the tick's end: {at_end}");
    }
}
