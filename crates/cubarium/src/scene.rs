//! The M1 geometry fixtures.
//!
//! Scenes own their state and their PRNG; they never see wall time, and the renderer
//! never sees them — it is handed a cloned [`SceneView`] snapshot of the last completed
//! tick. Every spatial step goes through `cubarium-surface`.

use cube_proto::Face;
use cubarium_render::{BodyShape, Canvas, Lobe, Trail, draw_trail, stamp_body};
use cubarium_surface::{
    FieldGraph, PathSegment, PixelImage, ScalarField, SurfacePoint, Vec2, deposit, diffuse,
    travel_into,
};

use crate::clock::DT;
use crate::present::{PALETTE, TRAIL_BRIGHTNESS, draw_floor, draw_ramp_field, interpolate};
use crate::rng::SplitMix64;

/// Which fixture(s) to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneKind {
    Body,
    Vertex,
    Patch,
    All,
}

// --- Appearance constants from `crates/cubarium/README.md` -------------------------
//
// The fixtures share the M2 palette (`design/appearance.md` "Palette", decoded in
// [`crate::present`]) so that what a capture teaches about color holds for the world
// too: the same night floor, the same indigo-to-cyan substrate ramp, bodies at the
// magenta end of the hue ramp, and trails at the body color times
// [`TRAIL_BRIGHTNESS`].

/// The fixture bodies sit at hue 0 of the M2 body ramp: magenta.
pub fn body_color() -> [f32; 3] {
    PALETTE.hue_magenta
}

/// The history trail is the body color at the M2 trail brightness.
pub fn trail_color() -> [f32; 3] {
    let c = body_color();
    [c[0] * TRAIL_BRIGHTNESS, c[1] * TRAIL_BRIGHTNESS, c[2] * TRAIL_BRIGHTNESS]
}

/// At most 160 trail segments.
pub const TRAIL_MAX_SEGMENTS: usize = 160;
/// Eight seconds of trail at 20 Hz.
pub const TRAIL_MAX_AGE_TICKS: u64 = 8 * crate::clock::TICK_HZ as u64;
/// The patch field saturates the substrate ramp at this value per cell.
pub const FIELD_SCALE: f64 = 6.0;

/// The asymmetric fixture body: core, head, and one side lobe.
pub fn fixture_body() -> BodyShape {
    BodyShape {
        lobes: vec![
            Lobe { offset: Vec2::new(0.0, 0.0), radius: 1.6 },
            Lobe { offset: Vec2::new(2.4, 0.0), radius: 1.0 },
            Lobe { offset: Vec2::new(-1.4, 1.3), radius: 0.9 },
        ],
    }
}

/// Rotate a chart tangent vector counter-clockwise on screen by `a` radians. With `+v`
/// image-down this is `(x cos a + y sin a, -x sin a + y cos a)`, agreeing with
/// `Vec2::rotate_quarter_turns` at `a = π/2`.
fn rotate_screen_ccw(v: Vec2, a: f64) -> Vec2 {
    let (s, c) = a.sin_cos();
    Vec2::new(v.x * c + v.y * s, -v.x * s + v.y * c)
}

// --- Render views ------------------------------------------------------------------

/// One body as the renderer sees it.
#[derive(Clone, Debug)]
pub struct BodyView {
    /// Where the last completed tick left the body.
    pub anchor: SurfacePoint,
    pub heading: Vec2,
    pub shape: BodyShape,
    pub color: [f32; 3],
    /// The path traveled *during* that tick, split per chart. Empty for a body that does
    /// not move; the renderer interpolates along it between frames exactly as the M2
    /// presenter does (see [`crate::present::interpolate`]).
    pub moved: Vec<PathSegment>,
}

/// A cloned snapshot of the last completed tick. Never a live reference into a scene.
#[derive(Clone, Debug, Default)]
pub struct SceneView {
    pub tick: u64,
    pub field: Option<ScalarField>,
    pub trail: Option<Trail>,
    pub bodies: Vec<BodyView>,
}

/// Draw a snapshot: floor, substrate, then the body trail, then the bodies. `f` is the
/// clock's interpolation fraction for this frame; bodies are drawn that fraction along
/// the path they traveled during the last tick (0 draws the tick's own start).
pub fn render(view: &SceneView, f: f64, canvas: &mut Canvas, scratch: &mut Vec<PixelImage>) {
    canvas.clear();
    draw_floor(canvas);
    if let Some(field) = &view.field {
        draw_ramp_field(
            canvas,
            field,
            FIELD_SCALE,
            PALETTE.producer_low,
            PALETTE.producer_high,
            true,
        );
    }
    if let Some(trail) = &view.trail {
        draw_trail(canvas, trail, view.tick, TRAIL_MAX_AGE_TICKS, trail_color());
    }
    for b in &view.bodies {
        let (anchor, heading) = interpolate(&b.moved, b.anchor, b.heading, f);
        stamp_body(canvas, anchor, heading, &b.shape, b.color, scratch);
    }
}

// --- The wandering body ------------------------------------------------------------

/// Speed in pixels per second.
const BODY_SPEED: f64 = 4.0;
/// Ornstein–Uhlenbeck mean reversion on the turn rate, per second.
const OU_THETA: f64 = 0.2;
/// Ornstein–Uhlenbeck volatility, radians per second per √second. The stationary
/// standard deviation is `sigma / sqrt(2 theta)` ≈ 0.13 rad/s, a turning radius of about
/// 30 pixels at 4 px/s: tight enough to wander, loose enough to cross every seam.
const OU_SIGMA: f64 = 0.08;

/// One asymmetric body wandering the whole surface, with a history trail.
#[derive(Clone, Debug)]
pub struct BodyScene {
    pos: SurfacePoint,
    heading: Vec2,
    /// Radians per second. A scalar: it is body-relative and does *not* transport.
    turn_rate: f64,
    rng: SplitMix64,
    trail: Trail,
    shape: BodyShape,
    travel_buf: cubarium_surface::Travel,
}

impl BodyScene {
    pub fn new(seed: u64) -> BodyScene {
        let mut rng = SplitMix64::new(seed ^ 0x_B0D1_5EED_0000_0001);
        let angle = rng.range(0.0, std::f64::consts::TAU);
        BodyScene {
            pos: SurfacePoint::new(Face::Front, 32.0, 32.0),
            heading: Vec2::from_screen_angle(angle),
            turn_rate: 0.0,
            rng,
            trail: Trail::new(TRAIL_MAX_SEGMENTS),
            shape: fixture_body(),
            travel_buf: cubarium_surface::Travel::default(),
        }
    }

    pub fn tick(&mut self, tick: u64) {
        // Ornstein–Uhlenbeck on the turn rate, in the body's own frame.
        let noise = self.rng.normal();
        self.turn_rate += -OU_THETA * self.turn_rate * DT + OU_SIGMA * DT.sqrt() * noise;

        // Steer, then sweep, then transport the heading through the travel's tangent map.
        self.heading = rotate_screen_ccw(self.heading, self.turn_rate * DT);
        travel_into(self.pos, self.heading * (BODY_SPEED * DT), &mut self.travel_buf);
        self.trail.push_travel(&self.travel_buf, tick);
        self.pos = self.travel_buf.end;
        self.heading = self.travel_buf.map.apply(self.heading);
        // Keep the heading a unit vector against accumulated rounding.
        if let Some(h) = self.heading.normalized() {
            self.heading = h;
        }
        self.trail.prune(tick, TRAIL_MAX_AGE_TICKS);
    }

    pub fn body(&self) -> BodyView {
        BodyView {
            anchor: self.pos,
            heading: self.heading,
            shape: self.shape.clone(),
            color: body_color(),
            // `travel_buf` still holds the last tick's sweep: the same segments the
            // trail was fed, which is exactly the path to interpolate along.
            moved: self.travel_buf.segments.clone(),
        }
    }

    pub fn trail(&self) -> Trail {
        self.trail.clone()
    }
}

// --- The vertex fixture ------------------------------------------------------------

/// Quarter turn per minute, in radians per second.
const VERTEX_TURN_RATE: f64 = std::f64::consts::TAU * 0.25 / 60.0;

/// The ownership-discontinuity fixture: four bodies tucked into the Top face's vertices
/// with slowly rotating headings, plus one straddling the Front/Right seam.
#[derive(Clone, Debug)]
pub struct VertexScene {
    angle: f64,
    shape: BodyShape,
}

/// The four Top-face anchors, 2.5 px diagonally inside each vertex.
pub const VERTEX_ANCHORS: [(f64, f64); 4] =
    [(2.5, 2.5), (61.5, 2.5), (61.5, 61.5), (2.5, 61.5)];
/// The Front/Right seam straddler.
pub const SEAM_ANCHOR: (f64, f64) = (63.2, 40.0);

impl VertexScene {
    pub fn new(_seed: u64) -> VertexScene {
        VertexScene { angle: 0.0, shape: fixture_body() }
    }

    pub fn tick(&mut self, _tick: u64) {
        self.angle += VERTEX_TURN_RATE * DT;
        if self.angle >= std::f64::consts::TAU {
            self.angle -= std::f64::consts::TAU;
        }
    }

    pub fn bodies(&self) -> Vec<BodyView> {
        let heading = Vec2::from_screen_angle(self.angle);
        let mut v: Vec<BodyView> = VERTEX_ANCHORS
            .iter()
            .map(|&(u, p)| BodyView {
                anchor: SurfacePoint::new(Face::Top, u, p),
                heading,
                shape: self.shape.clone(),
                color: body_color(),
                // The vertex fixture's bodies never move: nothing to interpolate.
                moved: Vec::new(),
            })
            .collect();
        // The seam straddler holds a fixed heading straight across the Front/Right seam.
        v.push(BodyView {
            anchor: SurfacePoint::new(Face::Front, SEAM_ANCHOR.0, SEAM_ANCHOR.1),
            heading: Vec2::new(1.0, 0.0),
            shape: self.shape.clone(),
            color: body_color(),
            moved: Vec::new(),
        });
        v
    }
}

// --- The substrate patch fixture ---------------------------------------------------

/// Diffusion exchange coefficient per tick.
const PATCH_DIFFUSION: f64 = 0.15;
/// Multiplicative decay per tick (0.5 %).
const PATCH_DECAY: f64 = 0.995;
/// Deposit amount.
const PATCH_AMOUNT: f64 = 40.0;
/// Deposit surface radius in pixels.
const PATCH_RADIUS: f64 = 10.0;
/// Twelve seconds between deposits, at 20 Hz.
pub const PATCH_PERIOD_TICKS: u64 = 12 * crate::clock::TICK_HZ as u64;

/// The four deposit centers, in order: Front/Right seam, the Right/Top twisted seam,
/// the Back/Top/Left vertex region, and a rim-clipped footprint.
pub const PATCH_CENTERS: [(Face, f64, f64); 4] = [
    (Face::Front, 60.0, 32.0),
    (Face::Right, 50.0, 3.0),
    (Face::Back, 61.0, 3.0),
    (Face::Front, 20.0, 61.0),
];

/// A diffusing, decaying scalar field fed by periodic deposits at awkward geometry.
/// The cell adjacency graph is derived once from `cross_seam` and never copied.
pub struct PatchScene {
    field: ScalarField,
    scratch: ScalarField,
    graph: FieldGraph,
}

impl PatchScene {
    pub fn new(_seed: u64) -> PatchScene {
        PatchScene {
            field: ScalarField::zeros(),
            scratch: ScalarField::zeros(),
            graph: FieldGraph::new(),
        }
    }

    pub fn tick(&mut self, tick: u64) {
        if tick.is_multiple_of(PATCH_PERIOD_TICKS) {
            let (face, u, v) = PATCH_CENTERS[((tick / PATCH_PERIOD_TICKS) % 4) as usize];
            deposit(&mut self.field, SurfacePoint::new(face, u, v), PATCH_RADIUS, PATCH_AMOUNT);
        }
        diffuse(&mut self.field, &mut self.scratch, &self.graph, PATCH_DIFFUSION);
        for c in cubarium_surface::CellId::all() {
            let x = self.field.get(c);
            self.field.set(c, x * PATCH_DECAY);
        }
    }

    pub fn field(&self) -> ScalarField {
        self.field.clone()
    }

    pub fn total(&self) -> f64 {
        self.field.total()
    }
}

// --- The composite ------------------------------------------------------------------

/// The scene set selected by `--scene`, driven by one tick counter.
pub struct Scenes {
    kind: SceneKind,
    tick: u64,
    body: Option<BodyScene>,
    vertex: Option<VertexScene>,
    patch: Option<PatchScene>,
}

impl Scenes {
    pub fn new(kind: SceneKind, seed: u64) -> Scenes {
        let want_body = matches!(kind, SceneKind::Body | SceneKind::All);
        let want_vertex = matches!(kind, SceneKind::Vertex | SceneKind::All);
        let want_patch = matches!(kind, SceneKind::Patch | SceneKind::All);
        Scenes {
            kind,
            tick: 0,
            body: want_body.then(|| BodyScene::new(seed)),
            vertex: want_vertex.then(|| VertexScene::new(seed)),
            patch: want_patch.then(|| PatchScene::new(seed)),
        }
    }

    pub fn kind(&self) -> SceneKind {
        self.kind
    }

    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    /// Advance one simulation tick. The tick index handed to the scenes is the index of
    /// the tick being completed, starting at 0.
    pub fn tick(&mut self) {
        let t = self.tick;
        if let Some(s) = &mut self.patch {
            s.tick(t);
        }
        if let Some(s) = &mut self.body {
            s.tick(t);
        }
        if let Some(s) = &mut self.vertex {
            s.tick(t);
        }
        self.tick += 1;
    }

    /// An owned snapshot of the last completed tick.
    pub fn view(&self) -> SceneView {
        let mut bodies = Vec::new();
        if let Some(s) = &self.body {
            bodies.push(s.body());
        }
        if let Some(s) = &self.vertex {
            bodies.extend(s.bodies());
        }
        SceneView {
            tick: self.tick.saturating_sub(1),
            field: self.patch.as_ref().map(|s| s.field()),
            trail: self.body.as_ref().map(|s| s.trail()),
            bodies,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Faces carrying anything above the uniform night floor.
    fn lit_faces(canvas: &Canvas) -> HashSet<Face> {
        let mut f = HashSet::new();
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let px = canvas.get(face, x, y);
                    if (0..3).any(|i| px[i] - PALETTE.floor[i] > 1e-9) {
                        f.insert(face);
                    }
                }
            }
        }
        f
    }

    fn run(kind: SceneKind, seed: u64, ticks: u64) -> Scenes {
        let mut s = Scenes::new(kind, seed);
        for _ in 0..ticks {
            s.tick();
        }
        s
    }

    #[test]
    fn rotate_screen_ccw_agrees_with_the_quarter_turn_convention() {
        let v = Vec2::new(1.0, 0.0);
        let q = rotate_screen_ccw(v, std::f64::consts::FRAC_PI_2);
        let want = v.rotate_quarter_turns(1);
        assert!((q.x - want.x).abs() < 1e-12 && (q.y - want.y).abs() < 1e-12, "{q:?} vs {want:?}");
        // Length is preserved.
        let r = rotate_screen_ccw(Vec2::new(0.3, -0.7), 1.234);
        assert!((r.length() - Vec2::new(0.3, -0.7).length()).abs() < 1e-12);
    }

    #[test]
    fn scenes_are_deterministic_in_the_seed() {
        let a = run(SceneKind::All, 42, 200).view();
        let b = run(SceneKind::All, 42, 200).view();
        let c = run(SceneKind::All, 43, 200).view();
        assert_eq!(a.bodies[0].anchor, b.bodies[0].anchor);
        assert_eq!(a.field, b.field);
        assert_ne!(a.bodies[0].anchor, c.bodies[0].anchor, "a different seed must differ");
    }

    #[test]
    fn the_wandering_body_crosses_seams_and_stays_canonical() {
        // Six minutes of simulation: enough to leave the starting face many times.
        let mut s = Scenes::new(SceneKind::Body, 7);
        let mut faces = HashSet::new();
        for _ in 0..(20 * 60 * 6) {
            s.tick();
            let v = s.view();
            let a = v.bodies[0].anchor;
            assert!(a.is_canonical(), "non-canonical anchor {a:?}");
            assert!((v.bodies[0].heading.length() - 1.0).abs() < 1e-9, "heading drifted");
            faces.insert(a.face);
        }
        assert!(faces.len() >= 3, "the body should reach several faces, got {faces:?}");
    }

    #[test]
    fn the_trail_is_bounded_in_length_and_age() {
        let s = run(SceneKind::Body, 3, 2_000);
        let trail = s.view().trail.expect("the body scene has a trail");
        assert!(trail.len() <= TRAIL_MAX_SEGMENTS, "trail {} segments", trail.len());
        let now = s.view().tick;
        for seg in trail.segments() {
            assert!(now - seg.tick <= TRAIL_MAX_AGE_TICKS, "segment older than the window");
        }
    }

    #[test]
    fn the_patch_field_stays_finite_nonnegative_and_bounded() {
        let s = run(SceneKind::Patch, 1, 4_000);
        let f = s.view().field.expect("the patch scene has a field");
        assert!(f.is_finite() && f.is_nonnegative());
        // Deposits add 40 every 12 s and 0.5 %/tick decay bounds the steady state.
        assert!(f.total() > 1.0, "the field should hold mass: {}", f.total());
        assert!(f.total() < 40.0 * 12.0, "decay should bound the total: {}", f.total());
    }

    #[test]
    fn every_scene_lights_more_than_one_face() {
        let mut canvas = Canvas::new();
        let mut scratch = Vec::new();
        for kind in [SceneKind::Body, SceneKind::Vertex, SceneKind::Patch, SceneKind::All] {
            // 20 s of simulation, the capture length the contract asks for.
            let s = run(kind, 1, 400);
            render(&s.view(), 0.0, &mut canvas, &mut scratch);
            let faces = lit_faces(&canvas);
            assert!(faces.len() > 1, "{kind:?} lit only {faces:?}");
        }
    }

    #[test]
    fn the_vertex_fixture_puts_bodies_on_top_and_across_the_front_right_seam() {
        let s = run(SceneKind::Vertex, 1, 100);
        let v = s.view();
        assert_eq!(v.bodies.len(), 5);
        assert_eq!(v.bodies.iter().filter(|b| b.anchor.face == Face::Top).count(), 4);
        let seam = v.bodies.last().unwrap();
        assert_eq!(seam.anchor.face, Face::Front);
        assert!((seam.anchor.u - 63.2).abs() < 1e-12);

        let mut canvas = Canvas::new();
        let mut scratch = Vec::new();
        render(&v, 0.0, &mut canvas, &mut scratch);
        let faces = lit_faces(&canvas);
        // The Top vertices reach the four side faces and the seam body reaches Right.
        assert!(faces.contains(&Face::Top) && faces.contains(&Face::Right), "{faces:?}");
    }

    /// The fixture body carries its last tick's path, so the demo interpolates between
    /// ticks exactly as the world does: the stamp at `f = 0` sits where the tick began
    /// and the stamp at `f → 1` sits where it ended.
    #[test]
    fn the_demo_body_is_interpolated_along_its_travel() {
        let s = run(SceneKind::Body, 11, 100);
        let v = s.view();
        let body = &v.bodies[0];
        assert!(!body.moved.is_empty(), "the wandering body travels every tick");

        let (start, _) = interpolate(&body.moved, body.anchor, body.heading, 0.0);
        let first = body.moved.first().unwrap();
        assert_eq!(start.face, first.face);
        assert!((start.u - first.from.x).abs() < 1e-9 && (start.v - first.from.y).abs() < 1e-9);

        let (end, _) = interpolate(&body.moved, body.anchor, body.heading, 1.0f64.next_down());
        assert_eq!(end.face, body.anchor.face);
        assert!((end.u - body.anchor.u).abs() < 1e-9 && (end.v - body.anchor.v).abs() < 1e-9);

        // A whole tick of motion is 4 px/s * 0.05 s = 0.2 px, so the two stamps differ
        // by a fraction of a pixel: compare the images, not just the anchors.
        let mut canvas = Canvas::new();
        let mut scratch = Vec::new();
        render(&v, 0.0, &mut canvas, &mut scratch);
        let at_zero: Vec<f32> = Face::ALL
            .into_iter()
            .flat_map(|f| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (f, x, y))))
            .map(|(f, x, y)| canvas.get(f, x, y)[0])
            .collect();
        render(&v, 1.0f64.next_down(), &mut canvas, &mut scratch);
        let at_one: Vec<f32> = Face::ALL
            .into_iter()
            .flat_map(|f| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (f, x, y))))
            .map(|(f, x, y)| canvas.get(f, x, y)[0])
            .collect();
        assert_ne!(at_zero, at_one, "the interpolated body must move within the tick");
    }

    #[test]
    fn the_view_is_a_snapshot_not_a_live_reference() {
        let mut s = Scenes::new(SceneKind::All, 5);
        for _ in 0..50 {
            s.tick();
        }
        let snap = s.view();
        let pos = snap.bodies[0].anchor;
        let field = snap.field.clone();
        for _ in 0..50 {
            s.tick();
        }
        assert_eq!(snap.bodies[0].anchor, pos, "the snapshot moved with the scene");
        assert_eq!(snap.field, field);
        assert_ne!(s.view().bodies[0].anchor, pos, "the scene did not advance");
    }

    #[test]
    fn the_composite_scene_holds_every_fixture() {
        let s = run(SceneKind::All, 1, 10);
        let v = s.view();
        assert!(v.field.is_some() && v.trail.is_some());
        assert_eq!(v.bodies.len(), 6, "one wanderer plus five vertex-fixture bodies");
        for kind in [SceneKind::Body, SceneKind::Vertex, SceneKind::Patch] {
            let v = run(kind, 1, 10).view();
            match kind {
                SceneKind::Body => assert!(v.field.is_none() && v.bodies.len() == 1),
                SceneKind::Vertex => assert!(v.field.is_none() && v.trail.is_none()),
                SceneKind::Patch => assert!(v.field.is_some() && v.bodies.is_empty()),
                SceneKind::All => unreachable!(),
            }
        }
    }
}
