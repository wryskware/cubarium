//! The shape of the world: which charts exist, how large they are, how they join, and
//! how they embed in 3D.
//!
//! Everything else in this crate is written against [`Topology`] and [`Scale`], never
//! against the cube's 64-pixel charts directly. [`Topology::Cube`] reproduces the
//! five-chart cube exactly — every cube arm below is the body the function had before
//! the topology parameter existed — and [`Topology::Ring`] is one chart whose left and
//! right edges join and whose top and bottom edges are solid rims.

use crate::{Edge, FACE_EXTENT, Face, Seam, SurfacePoint, Vec2, cross_seam};

/// Pixels along one cell edge at world scale 1. A ring at scale `S` uses `4·S`
/// (see [`Scale::cell_pixels`]); the cube is pinned to `S = 1` and always uses 4.
pub const CELL_PIXELS: f64 = 4.0;

/// Body extent in pixels at world scale 1: the stamp budget every renderer shares with
/// [`crate::unfold_pixels`] (see [`Scale::footprint_radius`]).
pub const FOOTPRINT_PIXELS: f64 = 9.0;

/// Embedded units per pixel at world scale 1: the cube's `[-1, 1]^3` chart is 64 pixels
/// across and 2 units across, so one pixel is `1/32` of a unit, and the ring's cylinder
/// is built to match that feature scale exactly (`design/flat-world-plan-2026-09-16.md` §5a).
pub const EMBED_PIXELS: f64 = 32.0;

/// World scale `S`: the multiplier on every length in a ring world.
///
/// A ring at `S = 2` is twice as wide, twice as tall and drawn with twice-as-large art,
/// and it has **the same cells, the same noise scale and the same ecology** as the same
/// ring at `S = 1`, because cells are `4·S` pixels and the cylinder embedding divides by
/// `32·S`. `Topology::Cube` is pinned to `S = 1`: the cube's 32-pixel local-radius proof
/// and its 9-pixel stamp budget are completeness arguments, not tunables, so
/// [`Topology::validate`] refuses a cube world at any other scale
/// (`design/flat-world-plan-2026-09-16.md` §2).
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
// A scale *is* its multiplier on the wire, so a config can write `world_scale = 2.0`.
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Scale {
    world: f64,
}

impl Scale {
    /// `S = 1`: the cube's scale, and the ring's first shipped value.
    pub const ONE: Scale = Scale { world: 1.0 };

    /// A scale from its multiplier. Nothing is checked here; [`Topology::validate`] is
    /// where a world's scale is accepted or refused.
    #[inline]
    pub const fn new(world: f64) -> Scale {
        Scale { world }
    }

    /// The multiplier `S`.
    #[inline]
    pub const fn world(self) -> f64 {
        self.world
    }

    /// Pixels along one cell edge: `4·S`.
    #[inline]
    pub fn cell_pixels(self) -> f64 {
        CELL_PIXELS * self.world
    }

    /// The stamp budget in pixels: `9·S`. Bodies, care flourishes and every other
    /// [`crate::unfold_pixels`] footprint are bounded by this, and it must stay within
    /// [`Topology::max_local_radius`] for the unfolding to be complete.
    #[inline]
    pub fn footprint_radius(self) -> f64 {
        FOOTPRINT_PIXELS * self.world
    }

    /// Embedded units per pixel: `1/(32·S)`, the divisor both cylinder axes use.
    #[inline]
    pub fn embed_divisor(self) -> f64 {
        EMBED_PIXELS * self.world
    }
}

impl Default for Scale {
    fn default() -> Scale {
        Scale::ONE
    }
}

/// The surface's shape.
///
/// * [`Topology::Cube`] — the five charts `Front, Right, Back, Left, Top`, each 64×64
///   pixels, joined by `cube_proto`'s seam contract, with the four lower edges an open
///   rim. This is the world every existing test describes.
/// * [`Topology::Ring`] — one chart, [`Face::Front`], `w` by `h` pixels. **The left and
///   right edges join** (a seam of the chart to itself: identity transport, translation
///   by `∓w`, zero quarter turns) and **the top and bottom edges are solid rims** that
///   reflect exactly as the cube's open bottom does.
///
/// `w` and `h` are pixel counts, so a ring is at most 65,535 pixels on a side; the real
/// bound is much tighter, because [`Topology::validate`] refuses more than
/// `u16::MAX` cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Topology {
    #[default]
    Cube,
    Ring {
        w: u16,
        h: u16,
    },
}

/// The charts of a cube, in `Face` index order.
const CUBE_CHARTS: [Face; 5] = Face::ALL;
/// The single chart of a ring.
const RING_CHARTS: [Face; 1] = [Face::Front];

impl Topology {
    /// The charts this topology has, in index order: all five faces for a cube, just
    /// [`Face::Front`] for a ring. Replaces `Face::ALL` wherever the code must work on
    /// either shape.
    #[inline]
    pub fn charts(self) -> &'static [Face] {
        match self {
            Topology::Cube => &CUBE_CHARTS,
            Topology::Ring { .. } => &RING_CHARTS,
        }
    }

    /// Position of `face` in [`Topology::charts`], for indexing a per-chart array.
    /// A cube's charts are in `Face` index order, so this is `face.index()` there.
    #[inline]
    pub fn chart_index(self, face: Face) -> usize {
        match self {
            Topology::Cube => face.index(),
            Topology::Ring { .. } => {
                debug_assert_eq!(face, Face::Front, "a ring has only Face::Front");
                0
            }
        }
    }

    /// True when `face` is one of this topology's charts.
    #[inline]
    pub fn has_chart(self, face: Face) -> bool {
        match self {
            Topology::Cube => true,
            Topology::Ring { .. } => face == Face::Front,
        }
    }

    /// The chart's size in pixels: coordinates span `[0, w) × [0, h)`.
    #[inline]
    pub fn extent(self, face: Face) -> (f64, f64) {
        match self {
            Topology::Cube => (FACE_EXTENT, FACE_EXTENT),
            Topology::Ring { w, h } => {
                debug_assert_eq!(face, Face::Front, "a ring has only Face::Front");
                (f64::from(w), f64::from(h))
            }
        }
    }

    /// The chart across `edge`, or `None` at an open rim.
    ///
    /// Cube: `cube_proto`'s table verbatim, `None` on a side face's `Edge::Bottom`.
    /// Ring: `Edge::Right` and `Edge::Left` name **this same chart's** opposite edge,
    /// unreversed — the wrap — and `Edge::Top`/`Edge::Bottom` are the two solid rims.
    #[inline]
    pub fn neighbor(self, face: Face, edge: Edge) -> Option<Seam> {
        match self {
            Topology::Cube => face.neighbor(edge),
            Topology::Ring { .. } => match edge {
                Edge::Right => Some(Seam { face: Face::Front, edge: Edge::Left, reversed: false }),
                Edge::Left => Some(Seam { face: Face::Front, edge: Edge::Right, reversed: false }),
                Edge::Top | Edge::Bottom => None,
            },
        }
    }

    /// Quarter turns a tangent vector rotates when crossing out of `face` through `edge`,
    /// or `None` at an open rim. The ring's wrap is a pure translation: zero turns.
    #[inline]
    pub fn seam_turns(self, face: Face, edge: Edge) -> Option<u8> {
        match self {
            Topology::Cube => cross_seam(face, edge, 0).map(|(_, _, _, turns)| turns),
            Topology::Ring { .. } => self.neighbor(face, edge).map(|_| 0),
        }
    }

    /// Largest query radius [`crate::unfold`] and [`crate::unfold_pixels`] accept.
    ///
    /// On the cube this is [`crate::MAX_LOCAL_RADIUS`] — a *completeness proof*, not a
    /// tunable: within 32 pixels every shortest path crosses at most
    /// [`crate::MAX_SEAMS`] seams. Nothing about `S` may touch it.
    ///
    /// On a ring it is a **chosen cap**, `min(h, w − 2·CELL_PIXELS) / 2`. No radius is
    /// meaningless on a ring; the cap bounds `unfold_pixels` cost (which grows with the
    /// square of the radius) and is what keeps the wrap at two images in range, since
    /// [`Topology::validate`] requires `w >= 2·max_local_radius() + 2·CELL_PIXELS` and
    /// this formula satisfies it by construction.
    #[inline]
    pub fn max_local_radius(self) -> f64 {
        match self {
            Topology::Cube => crate::MAX_LOCAL_RADIUS,
            Topology::Ring { w, h } => {
                (f64::from(h)).min(f64::from(w) - 2.0 * CELL_PIXELS) / 2.0
            }
        }
    }

    /// Cells across and down one chart.
    #[inline]
    pub fn cells(self, scale: Scale, face: Face) -> (u16, u16) {
        match self {
            Topology::Cube => (crate::CELLS_PER_FACE_EDGE as u16, crate::CELLS_PER_FACE_EDGE as u16),
            Topology::Ring { .. } => {
                let (w, h) = self.extent(face);
                let cp = scale.cell_pixels();
                ((w / cp) as u16, (h / cp) as u16)
            }
        }
    }

    /// Cells over the whole surface. Runtime, not a constant: `1,280 = 2^8·5` cannot be
    /// factored 16:9 with square cells, so no ring raster reproduces it.
    #[inline]
    pub fn cell_count(self, scale: Scale) -> usize {
        match self {
            Topology::Cube => crate::CUBE_CELL_COUNT,
            Topology::Ring { .. } => {
                let (cx, cy) = self.cells(scale, Face::Front);
                usize::from(cx) * usize::from(cy)
            }
        }
    }

    /// Position in 3D, for sampling spatially coherent environmental functions.
    ///
    /// **Cube:** the unit cube `[-1, 1]^3`, matching `cube_proto::geometry::pixel_direction`
    /// at pixel centres — with `a = u/32 − 1` and `b = v/32 − 1`, the face frame's
    /// `center + a·tangent_u + b·tangent_v`. The scale is ignored (the cube is pinned to
    /// `S = 1`).
    ///
    /// **Ring:** the isotropic cylinder of `design/flat-world-plan-2026-09-16.md` §5a,
    ///
    /// > `θ = 2π·u/w`, `r = w/(2π·32·S)`, `y_e = (h/2 − v)/(32·S)`,
    /// > `embed(p) = [r·cos θ, y_e, r·sin θ]`
    ///
    /// with the vertical in slot 1 because that is where the cube puts height. Arc length
    /// per pixel is `1/(32·S)` on **both** axes, so the habitat wave sum samples at the
    /// cube's frequency in every direction, habitat patches are round, and the field is
    /// seamless across the wrap by construction: `u = 0` and `u = w` are the same point.
    ///
    /// This is *position*, not height — see [`Topology::height`]. They coincide on the
    /// cube and differ on a ring, which is the whole point of having both.
    pub fn embed(self, scale: Scale, p: &SurfacePoint) -> [f64; 3] {
        match self {
            Topology::Cube => {
                let f = crate::face_frame(p.face);
                let a = p.u / EMBED_PIXELS - 1.0;
                let b = p.v / EMBED_PIXELS - 1.0;
                [
                    f.center[0] + a * f.tangent_u[0] + b * f.tangent_v[0],
                    f.center[1] + a * f.tangent_u[1] + b * f.tangent_v[1],
                    f.center[2] + a * f.tangent_u[2] + b * f.tangent_v[2],
                ]
            }
            Topology::Ring { w, h } => {
                let (w, h) = (f64::from(w), f64::from(h));
                let theta = std::f64::consts::TAU * p.u / w;
                let r = w / (std::f64::consts::TAU * scale.embed_divisor());
                let y = (h / 2.0 - p.v) / scale.embed_divisor();
                [r * theta.cos(), y, r * theta.sin()]
            }
        }
    }

    /// A chart tangent vector in the same 3D coordinates as [`Topology::embed`], with
    /// pixel lengths scaled by `1/(32·S)` (the cube's `1/32`).
    pub fn embed_tangent(self, scale: Scale, p: &SurfacePoint, t: Vec2) -> [f64; 3] {
        match self {
            Topology::Cube => {
                let f = crate::face_frame(p.face);
                let (a, b) = (t.x / EMBED_PIXELS, t.y / EMBED_PIXELS);
                [
                    a * f.tangent_u[0] + b * f.tangent_v[0],
                    a * f.tangent_u[1] + b * f.tangent_v[1],
                    a * f.tangent_u[2] + b * f.tangent_v[2],
                ]
            }
            Topology::Ring { w, .. } => {
                // d/du is the azimuthal unit vector times `1/(32S)`; d/dv is `-y` times
                // the same, so the map is a local isometry up to that one divisor.
                let theta = std::f64::consts::TAU * p.u / f64::from(w);
                let k = 1.0 / scale.embed_divisor();
                [-t.x * k * theta.sin(), -t.y * k, t.x * k * theta.cos()]
            }
        }
    }

    /// Height in `[-1, 1]`: 1 at the top of the world, −1 at the bottom.
    ///
    /// This is the scalar light, moisture, the bands, `downhill` and both controllers'
    /// `height` channel read. On the cube it is `embed()[1]` — Top is exactly 1 — and on
    /// a ring it is `1 − 2v/h`, the side view the panel wants: canopy at the top row,
    /// soil at the bottom edge (`design/flat-world-plan-2026-09-16.md` §5).
    #[inline]
    pub fn height(self, p: &SurfacePoint) -> f64 {
        match self {
            Topology::Cube => self.embed(Scale::ONE, p)[1],
            Topology::Ring { h, .. } => 1.0 - 2.0 * p.v / f64::from(h),
        }
    }

    /// Squared distance between two points **in squared pixels**, used to reject far
    /// pairs before unfolding them.
    ///
    /// Cube: the squared 3D chord times `32²`. A chord may pass through the cube, so this
    /// is a lower bound on surface distance and never a metric on its own.
    ///
    /// Ring: `min(|Δu|, w − |Δu|)² + Δv²` straight from the chart coordinates — exact,
    /// wrap-aware and independent of the embedding, so pair rejection on a ring is the
    /// true distance rather than a conservative bound.
    pub fn chord_sq(self, a: &SurfacePoint, b: &SurfacePoint) -> f64 {
        match self {
            Topology::Cube => {
                let pa = self.embed(Scale::ONE, a);
                let pb = self.embed(Scale::ONE, b);
                let d = [pa[0] - pb[0], pa[1] - pb[1], pa[2] - pb[2]];
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) * (EMBED_PIXELS * EMBED_PIXELS)
            }
            Topology::Ring { w, .. } => {
                let w = f64::from(w);
                let du = (a.u - b.u).abs();
                let du = du.min(w - du);
                let dv = a.v - b.v;
                du * du + dv * dv
            }
        }
    }

    /// Why a `(topology, scale)` pair is not a world this build can run.
    pub fn validate(self, scale: Scale) -> Result<(), TopologyError> {
        let s = scale.world();
        if !s.is_finite() || s <= 0.0 {
            return Err(TopologyError::BadScale { world: s });
        }
        match self {
            Topology::Cube => {
                // The 32-pixel local radius and the 9-pixel stamp budget are proofs about
                // a 64-pixel chart. Rescaling the cube would invalidate both.
                if s != 1.0 {
                    return Err(TopologyError::CubeScaleNotOne { world: s });
                }
                Ok(())
            }
            Topology::Ring { w, h } => {
                if w == 0 || h == 0 {
                    return Err(TopologyError::EmptyRing { w, h });
                }
                let cp = scale.cell_pixels();
                let (wf, hf) = (f64::from(w), f64::from(h));
                if wf % cp != 0.0 || hf % cp != 0.0 {
                    return Err(TopologyError::ExtentNotCellMultiple { w, h, cell_pixels: cp });
                }
                // Counted in f64 before any narrowing cast, so a world too large to index
                // cannot hide behind a saturating `as u16`.
                let wanted = (wf / cp) * (hf / cp);
                if wanted.is_nan() || wanted > f64::from(u16::MAX) {
                    let cells = if wanted.is_finite() { wanted as usize } else { usize::MAX };
                    return Err(TopologyError::TooManyCells { cells, w, h, cell_pixels: cp });
                }
                // Two images of the chart, `2w` apart, must be the only ones that can ever
                // lie within a query radius of an observer.
                let r = self.max_local_radius();
                if r <= 0.0 || wf < 2.0 * r + 2.0 * CELL_PIXELS {
                    return Err(TopologyError::RingTooNarrow {
                        w,
                        needed: 2.0 * r.max(0.0) + 2.0 * CELL_PIXELS,
                    });
                }
                // A stamp wider than the query radius cannot be unfolded at all.
                if scale.footprint_radius() > r {
                    return Err(TopologyError::FootprintExceedsLocalRadius {
                        footprint: scale.footprint_radius(),
                        radius: r,
                    });
                }
                Ok(())
            }
        }
    }
}

/// Why [`Topology::validate`] refused a world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TopologyError {
    /// `world_scale` is not a positive finite number.
    BadScale { world: f64 },
    /// A cube world asked for a scale other than 1; its bounds are proofs, not tunables.
    CubeScaleNotOne { world: f64 },
    /// A ring with a zero side.
    EmptyRing { w: u16, h: u16 },
    /// A ring whose sides are not whole numbers of cells.
    ExtentNotCellMultiple { w: u16, h: u16, cell_pixels: f64 },
    /// More cells than a `CellId`'s `u16` can index.
    TooManyCells { cells: usize, w: u16, h: u16, cell_pixels: f64 },
    /// A ring narrow enough that a third image of the chart could be the nearest one.
    RingTooNarrow { w: u16, needed: f64 },
    /// A stamp budget larger than the largest query radius the topology accepts.
    FootprintExceedsLocalRadius { footprint: f64, radius: f64 },
}

impl std::fmt::Display for TopologyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopologyError::BadScale { world } => {
                write!(f, "world_scale {world} is not a positive finite number")
            }
            TopologyError::CubeScaleNotOne { world } => write!(
                f,
                "a cube world must have world_scale 1, not {world}: its 32-pixel local radius \
                 and 9-pixel stamp budget are completeness proofs about a 64-pixel chart"
            ),
            TopologyError::EmptyRing { w, h } => write!(f, "ring {w}x{h} has a zero side"),
            TopologyError::ExtentNotCellMultiple { w, h, cell_pixels } => write!(
                f,
                "ring {w}x{h} is not a whole number of {cell_pixels}-pixel cells"
            ),
            TopologyError::TooManyCells { cells, w, h, cell_pixels } => write!(
                f,
                "ring {w}x{h} at {cell_pixels} pixels per cell wants {cells} cells, over the \
                 {} a CellId can index",
                u16::MAX
            ),
            TopologyError::RingTooNarrow { w, needed } => write!(
                f,
                "ring width {w} is below {needed}, so a third image of the chart could be the \
                 nearest one"
            ),
            TopologyError::FootprintExceedsLocalRadius { footprint, radius } => write!(
                f,
                "a {footprint}-pixel stamp exceeds the {radius}-pixel local radius"
            ),
        }
    }
}

impl std::error::Error for TopologyError {}

#[cfg(test)]
mod tests {
    use super::*;

    const RING1: Topology = Topology::Ring { w: 320, h: 180 };
    const RING2: Topology = Topology::Ring { w: 640, h: 360 };
    const S2: Scale = Scale::new(2.0);

    #[test]
    fn the_cube_arms_are_todays_values() {
        let c = Topology::Cube;
        assert_eq!(c.charts(), &Face::ALL[..]);
        for face in Face::ALL {
            assert_eq!(c.extent(face), (64.0, 64.0));
            assert_eq!(c.cells(Scale::ONE, face), (16, 16));
        }
        assert_eq!(c.cell_count(Scale::ONE), 1280);
        assert_eq!(c.max_local_radius(), crate::MAX_LOCAL_RADIUS);
        assert_eq!(c.validate(Scale::ONE), Ok(()));
        assert_eq!(
            c.validate(S2),
            Err(TopologyError::CubeScaleNotOne { world: 2.0 })
        );
    }

    #[test]
    fn the_ladder_is_the_same_world_at_both_scales() {
        // Same cells, same embedding, same ecology: only the raster changes.
        assert_eq!(RING1.cells(Scale::ONE, Face::Front), (80, 45));
        assert_eq!(RING2.cells(S2, Face::Front), (80, 45));
        assert_eq!(RING1.cell_count(Scale::ONE), 3600);
        assert_eq!(RING2.cell_count(S2), 3600);
        assert_eq!(RING1.validate(Scale::ONE), Ok(()));
        assert_eq!(RING2.validate(S2), Ok(()));
        // Corresponding points embed identically.
        for &(u, v) in &[(0.0, 0.0), (10.0, 20.0), (319.0, 179.0), (160.0, 90.0)] {
            let a = SurfacePoint::new(Face::Front, u, v);
            let b = SurfacePoint::new(Face::Front, u * 2.0, v * 2.0);
            let ea = RING1.embed(Scale::ONE, &a);
            let eb = RING2.embed(S2, &b);
            for k in 0..3 {
                assert!((ea[k] - eb[k]).abs() < 1e-12, "axis {k}: {ea:?} vs {eb:?}");
            }
            assert!((RING1.height(&a) - RING2.height(&b)).abs() < 1e-12);
        }
    }

    #[test]
    fn the_cylinder_is_isotropic_and_seamless() {
        // One pixel is 1/(32S) embedded units along both axes.
        let k = 1.0 / Scale::ONE.embed_divisor();
        for &u in &[0.0, 37.0, 160.0, 319.0] {
            let a = SurfacePoint::new(Face::Front, u, 90.0);
            let b = SurfacePoint::new(Face::Front, u + 0.001, 90.0);
            let c = SurfacePoint::new(Face::Front, u, 90.001);
            let (ea, eb, ec) = (
                RING1.embed(Scale::ONE, &a),
                RING1.embed(Scale::ONE, &b),
                RING1.embed(Scale::ONE, &c),
            );
            let du = dist(ea, eb) / 0.001;
            let dv = dist(ea, ec) / 0.001;
            assert!((du - k).abs() < 1e-6, "{du} != {k}");
            assert!((dv - k).abs() < 1e-12, "{dv} != {k}");
        }
        // u = 0 and u = w are the same 3D point, so noise cannot jump at the wrap.
        let a = RING1.embed(Scale::ONE, &SurfacePoint::new(Face::Front, 0.0, 33.0));
        let b = RING1.embed(Scale::ONE, &SurfacePoint::new(Face::Front, 320.0, 33.0));
        assert!(dist(a, b) < 1e-12, "{a:?} {b:?}");
    }

    fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    #[test]
    fn ring_height_is_the_side_view_not_the_embedding() {
        let top = SurfacePoint::new(Face::Front, 17.0, 0.0);
        let bottom = SurfacePoint::new(Face::Front, 17.0, 180.0);
        assert_eq!(RING1.height(&top), 1.0);
        assert_eq!(RING1.height(&bottom), -1.0);
        assert_eq!(RING1.height(&SurfacePoint::new(Face::Front, 17.0, 90.0)), 0.0);
        // height does not vary along the wrap; embed does.
        for u in [0.0, 80.0, 160.0, 240.0] {
            assert_eq!(RING1.height(&SurfacePoint::new(Face::Front, u, 45.0)), 0.5);
        }
    }

    #[test]
    fn ring_chord_sq_is_the_exact_wrapped_distance() {
        let a = SurfacePoint::new(Face::Front, 2.0, 10.0);
        let b = SurfacePoint::new(Face::Front, 318.0, 13.0);
        // Four pixels apart across the wrap, three down.
        assert_eq!(RING1.chord_sq(&a, &b), 25.0);
        assert_eq!(RING1.chord_sq(&b, &a), 25.0);
        let c = SurfacePoint::new(Face::Front, 6.0, 10.0);
        assert_eq!(RING1.chord_sq(&a, &c), 16.0);
    }

    #[test]
    fn validate_refuses_the_three_named_shapes() {
        // Not a whole number of cells.
        assert!(matches!(
            Topology::Ring { w: 321, h: 180 }.validate(Scale::ONE),
            Err(TopologyError::ExtentNotCellMultiple { .. })
        ));
        assert!(matches!(
            Topology::Ring { w: 320, h: 180 }.validate(S2),
            Err(TopologyError::ExtentNotCellMultiple { .. })
        ));
        // More cells than a CellId can index: 1920x1080 at 4 px per cell wants 129,600.
        assert!(matches!(
            Topology::Ring { w: 1920, h: 1080 }.validate(Scale::ONE),
            Err(TopologyError::TooManyCells { cells: 129_600, .. })
        ));
        // At S = 6 the same raster is 320x180 cells = 57,600, which fits.
        assert_eq!(Topology::Ring { w: 1920, h: 1080 }.validate(Scale::new(6.0)), Ok(()));
        // Too narrow for two images.
        assert!(matches!(
            Topology::Ring { w: 8, h: 180 }.validate(Scale::ONE),
            Err(TopologyError::RingTooNarrow { .. })
        ));
        assert!(matches!(
            Topology::Ring { w: 0, h: 180 }.validate(Scale::ONE),
            Err(TopologyError::EmptyRing { .. })
        ));
        assert!(matches!(
            Topology::Ring { w: 320, h: 180 }.validate(Scale::new(0.0)),
            Err(TopologyError::BadScale { .. })
        ));
    }

    #[test]
    fn the_local_radius_leaves_room_for_the_stamp_and_for_two_images() {
        assert_eq!(RING1.max_local_radius(), 90.0);
        assert_eq!(RING2.max_local_radius(), 180.0);
        for (topo, scale) in [(RING1, Scale::ONE), (RING2, S2)] {
            let Topology::Ring { w, .. } = topo else { unreachable!() };
            let r = topo.max_local_radius();
            assert!(scale.footprint_radius() <= r, "{topo:?}");
            assert!(f64::from(w) >= 2.0 * r + 2.0 * CELL_PIXELS, "{topo:?}");
        }
    }

    #[test]
    fn the_ring_wrap_is_a_self_seam_and_the_rims_are_open() {
        for topo in [RING1, RING2] {
            assert_eq!(
                topo.neighbor(Face::Front, Edge::Right),
                Some(Seam { face: Face::Front, edge: Edge::Left, reversed: false })
            );
            assert_eq!(
                topo.neighbor(Face::Front, Edge::Left),
                Some(Seam { face: Face::Front, edge: Edge::Right, reversed: false })
            );
            assert_eq!(topo.neighbor(Face::Front, Edge::Top), None);
            assert_eq!(topo.neighbor(Face::Front, Edge::Bottom), None);
            assert_eq!(topo.seam_turns(Face::Front, Edge::Right), Some(0));
            assert_eq!(topo.seam_turns(Face::Front, Edge::Top), None);
            assert_eq!(topo.charts(), &[Face::Front]);
        }
    }
}
