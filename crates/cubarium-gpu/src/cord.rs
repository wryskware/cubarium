//! **Drape cords** (DRAPE-CORD, `design/handoffs/drape-cord-2026-10-10.md`): a baked model's
//! drape cell (the vaulttree's teal filaments, [`crate::voxel::ROLE_DRAPE`]) drawn as a thin
//! cord through its front face instead of a whole cube. This is the CPU mirror of
//! `shaders/drape_cord.glsl`, function for function, so the shader's rules are tested here.
//!
//! - **One cord a column, joined.** The cord enters a cell at its top edge and leaves at its
//!   bottom edge. Where it crosses a horizontal face is a hash of that face's position
//!   ([`DrapeCords::join`]), so the cell above and the cell below, each asking for the face
//!   they share, put it at the same point. A column's lowest cell ends the cord at its
//!   bottom edge, its highest starts it at its top edge (under the shelf it hangs from).
//!   The bake's strands are vertical cylinders, so a strand steps straight down from cell
//!   to cell; a cell with nothing below it is the end of its strand, even where a longer
//!   strand of the same cluster carries on in the next column.
//! - **Pixel-constant width.** The cord is `width_px` screen pixels wide at every
//!   `px_per_voxel`: a face texel is one screen pixel, so the width is counted in texels,
//!   never as a share of the voxel. Where it runs steeper than a pixel a row it widens
//!   just enough to stay joined row to row ([`DrapeCords::covers`]).
//! - **See-through.** A texel off the cord is not the cell's: the walk carries on to what
//!   is behind it, as through a leaf cutout, and the plant pass draws nothing there.
//! - **Swaying.** The plant pass moves each cell whole by its plant's tip times its weight
//!   ([`crate::plants`]); a cord's joins move with the *face's* weight instead
//!   ([`crate::plants::pack_faces`]), so the two cells meeting at a face put the join at
//!   the same screen point however the plant sways ([`swayed_join`]).

/// The path a cord takes inside its cell between its two joins: a look choice (Wrysk's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CordShape {
    /// A straight line from the top join to the bottom join.
    #[default]
    Straight,
    /// Smoothstepped: plumb where it meets each face, the sideways step in the middle.
    Eased,
}

/// The cords' settings as the shader reads them (`u.tex.z`, `u.tex.w`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrapeCords {
    /// The cord's width in screen pixels; 0 draws drape cells as whole cubes, as before.
    pub width_px: u32,
    pub shape: CordShape,
    /// How much of the face's free width (its width less the cord's) a join may wander
    /// across, centred, 0..1: 0 hangs every cord plumb down the middle.
    pub wander: f32,
}

impl Default for DrapeCords {
    fn default() -> DrapeCords {
        DrapeCords::OFF
    }
}

impl DrapeCords {
    /// Drape cells drawn as whole cubes.
    pub const OFF: DrapeCords = DrapeCords { width_px: 0, shape: CordShape::Straight, wander: 1.0 };

    pub fn on(&self) -> bool {
        self.width_px > 0
    }

    /// `u.tex.z` and `u.tex.w`: the width, and the shape (low byte) with the wander in
    /// hundredths (second byte).
    pub fn uniform(&self) -> [i32; 2] {
        let shape = match self.shape {
            CordShape::Straight => 0,
            CordShape::Eased => 1,
        };
        [self.width_px.min(255) as i32, shape | (self.wander_pct() << 8)]
    }

    fn wander_pct(&self) -> i32 {
        (self.wander.clamp(0.0, 1.0) * 100.0).round() as i32
    }

    /// `cordJoin`: the cord's centre, in face pixels from the left edge, where it crosses
    /// the horizontal face at height `y_face` of column `(x, z)` (`x` already round the
    /// ring) on a face `s` pixels wide. The join's left edge is on a whole pixel, so a cord
    /// at rest is crisp.
    pub fn join(&self, s: u32, x: i32, y_face: i32, z: i32) -> f32 {
        let w = self.width_px as i32;
        let free = (s as i32 - w).max(0);
        let span = (free as f32 * self.wander_pct() as f32 / 100.0 + 0.5) as i32;
        let lo = (free - span) / 2;
        let h = cell_hash(x, y_face, z, JOIN_SALT);
        (lo + (h % (span as u32 + 1)) as i32) as f32 + 0.5 * w as f32
    }

    /// `cordEnds`: cell `(x, y, z)`'s two joins, its top face's and its bottom face's.
    pub fn ends(&self, s: u32, x: i32, y: i32, z: i32) -> (f32, f32) {
        (self.join(s, x, y + 1, z), self.join(s, x, y, z))
    }

    /// `cordAt`: the cord's centre at `t` down the face (0 its top edge, 1 its bottom),
    /// between the joins `a` (top) and `b` (bottom).
    pub fn centre(&self, a: f32, b: f32, t: f32) -> f32 {
        let e = match self.shape {
            CordShape::Straight => t,
            CordShape::Eased => t * t * (3.0 - 2.0 * t),
        };
        a + (b - a) * e
    }

    /// `cordCovers`: whether face point `(x, t)` (`x` in face pixels, `t` down the face) is
    /// on the cord between joins `a` and `b`. Half-open, so a cord `w` pixels wide covers
    /// exactly `w` pixel centres of a row wherever it lies; a row it crosses more than a
    /// pixel of takes the excess as well, so consecutive rows always touch.
    pub fn covers(&self, s: u32, a: f32, b: f32, t: f32, x: f32) -> bool {
        let w = self.width_px as f32;
        let c = self.centre(a, b, t);
        // Half the larger step to the rows above and below (a face's edge stands in for a
        // row past it): each row reaches half way to its neighbours, so they meet.
        let dt = 1.0 / s as f32;
        let up = (c - self.centre(a, b, (t - dt).max(0.0))).abs();
        let down = (self.centre(a, b, (t + dt).min(1.0)) - c).abs();
        let ext = (0.5 * up.max(down) - 0.5).max(0.0);
        let r = x - c;
        r >= -0.5 * w - ext && r < 0.5 * w + ext
    }

    /// The walk's test (`plantTexel` on a drape cell): whether front-face texel `(dx, dy)`
    /// (`dy` 0 at the top) of cell `(x, y, z)` is the cord's. Every texel is, with the cords
    /// off: the cube as before.
    pub fn texel(&self, s: u32, x: i32, y: i32, z: i32, dx: u32, dy: u32) -> bool {
        if !self.on() {
            return true;
        }
        let (a, b) = self.ends(s, x, y, z);
        self.covers(s, a, b, (dy as f32 + 0.5) / s as f32, dx as f32 + 0.5)
    }
}

/// The salt of a join's hash.
const JOIN_SALT: i32 = 77;

/// `world.glsl`'s `cellHash`, bit for bit.
pub fn cell_hash(x: i32, y: i32, z: i32, salt: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA77)
        ^ (z as u32).wrapping_mul(0xC2B2_AE3D)
        ^ (salt as u32).wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h
}

/// `plantCordShows`'s join, in the cell's own swayed box: the face's join `join` moved by
/// the plant's `tip` times the face's weight less the cell's (the box already moved by the
/// cell's), in face pixels from the box's left edge.
pub fn swayed_join(s: u32, join: f32, tip: f32, w_face: f32, w_cell: f32) -> f32 {
    join + tip * (w_face - w_cell) * s as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALES: [u32; 7] = [2, 3, 4, 6, 8, 13, 16];

    fn variants() -> Vec<DrapeCords> {
        let mut v = Vec::new();
        for width_px in [1, 2] {
            for shape in [CordShape::Straight, CordShape::Eased] {
                for wander in [0.0, 0.5, 1.0] {
                    v.push(DrapeCords { width_px, shape, wander });
                }
            }
        }
        v
    }

    fn positions() -> impl Iterator<Item = (i32, i32, i32)> {
        (0..40).flat_map(|x| (0..12).map(move |y| (x * 7 + 3, y * 3 + 1, (x + y) % 9)))
    }

    /// The pixels of row `dy` of cell `(x, y, z)` on the cord.
    fn row(c: &DrapeCords, s: u32, x: i32, y: i32, z: i32, dy: u32) -> Vec<u32> {
        (0..s).filter(|&dx| c.texel(s, x, y, z, dx, dy)).collect()
    }

    #[test]
    fn two_stacked_drape_cells_put_the_cord_at_the_same_point_on_their_shared_face() {
        for c in variants() {
            for s in SCALES {
                for (x, y, z) in positions() {
                    let (_, upper_bottom) = c.ends(s, x, y + 1, z);
                    let (lower_top, _) = c.ends(s, x, y, z);
                    assert_eq!(upper_bottom, lower_top, "{c:?} at {s} px, ({x}, {y}, {z})");
                    // And the drawn pixels join: the upper cell's last row and the lower
                    // cell's first row touch (8-connected).
                    let up = row(&c, s, x, y + 1, z, s - 1);
                    let down = row(&c, s, x, y, z, 0);
                    assert!(!up.is_empty() && !down.is_empty());
                    let touch = up.iter().any(|&a| down.iter().any(|&b| a.abs_diff(b) <= 1));
                    assert!(touch, "{c:?} at {s} px, ({x}, {y}, {z}): {up:?} over {down:?}");
                }
            }
        }
    }

    #[test]
    fn every_row_of_a_cord_touches_the_next_inside_a_cell() {
        for c in variants() {
            for s in SCALES {
                for (x, y, z) in positions() {
                    for dy in 0..s - 1 {
                        let a = row(&c, s, x, y, z, dy);
                        let b = row(&c, s, x, y, z, dy + 1);
                        let touch = a.iter().any(|&p| b.iter().any(|&q| p.abs_diff(q) <= 1));
                        assert!(touch, "{c:?} at {s} px, ({x}, {y}, {z}) rows {dy}: {a:?} / {b:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_cord_is_the_same_number_of_screen_pixels_wide_at_every_scale() {
        for width_px in [1u32, 2] {
            for shape in [CordShape::Straight, CordShape::Eased] {
                // Plumb (no wander): every row is exactly the width, whatever the scale.
                let plumb = DrapeCords { width_px, shape, wander: 0.0 };
                for s in SCALES.into_iter().filter(|&s| s >= width_px) {
                    for (x, y, z) in positions() {
                        for dy in 0..s {
                            let n = row(&plumb, s, x, y, z, dy).len() as u32;
                            assert_eq!(n, width_px, "plumb {width_px} px at {s} px a voxel");
                        }
                    }
                }
                // Wandering: never narrower than the width, and a row less than a pixel from
                // its neighbours is exactly the width.
                let c = DrapeCords { width_px, shape, wander: 1.0 };
                for s in SCALES.into_iter().filter(|&s| s >= width_px) {
                    for (x, y, z) in positions() {
                        let (a, b) = c.ends(s, x, y, z);
                        for dy in 0..s {
                            let n = row(&c, s, x, y, z, dy).len() as u32;
                            // The larger step to a neighbouring row's centre.
                            let t = (dy as f32 + 0.5) / s as f32;
                            let at = |t: f32| c.centre(a, b, t.clamp(0.0, 1.0));
                            let dt = 1.0 / s as f32;
                            let step = (at(t) - at(t - dt)).abs().max((at(t + dt) - at(t)).abs());
                            assert!(n >= width_px, "{c:?} at {s} px: row {dy} has {n}");
                            if step <= 1.0 {
                                assert_eq!(n, width_px, "{c:?} at {s} px: row {dy}, step {step}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn off_the_cord_a_drape_texel_is_not_the_cells_and_with_the_switch_off_every_texel_is() {
        for s in SCALES {
            for (x, y, z) in positions().take(50) {
                let on = DrapeCords { width_px: 1, shape: CordShape::Straight, wander: 1.0 };
                let shown = (0..s).flat_map(|dy| (0..s).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| on.texel(s, x, y, z, dx, dy))
                    .count() as u32;
                assert!(shown < s * s, "a cord leaves holes at {s} px");
                assert!(shown >= s, "a cord crosses every row");
                assert!((0..s).all(|dy| (0..s).all(|dx| DrapeCords::OFF.texel(s, x, y, z, dx, dy))));
            }
        }
    }

    #[test]
    fn a_swaying_join_is_at_one_screen_point_from_both_of_its_cells() {
        let s = 6;
        let c = DrapeCords { width_px: 1, shape: CordShape::Eased, wander: 1.0 };
        let (x, y, z) = (11, 5, 3);
        let join = c.ends(s, x, y + 1, z).1;
        // The face's weight lies between the two cells' weights; the tip is anything.
        let (w_upper, w_face, w_lower) = (0.42, 0.37, 0.31);
        for tip in [-0.75f32, -0.2, 0.0, 0.33, 0.75] {
            // Each box's left edge is where its cell's sway put it.
            let screen = |w_cell: f32| (x as f32 + tip * w_cell) * s as f32 + swayed_join(s, join, tip, w_face, w_cell);
            let (a, b) = (screen(w_upper), screen(w_lower));
            assert!((a - b).abs() < 1e-4, "tip {tip}: {a} vs {b}");
        }
    }

    #[test]
    fn the_uniform_carries_the_width_shape_and_wander() {
        let c = DrapeCords { width_px: 2, shape: CordShape::Eased, wander: 0.5 };
        assert_eq!(c.uniform(), [2, 1 | (50 << 8)]);
        assert_eq!(DrapeCords::OFF.uniform()[0], 0);
        assert!(!DrapeCords::OFF.on());
    }
}
