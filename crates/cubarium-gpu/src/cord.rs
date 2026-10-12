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
//!
//! **The long wave** (round 2, [`CordWave`]): with a period, a cord no longer joins hashed
//! points face by face. Its centre follows one sine down its whole strand, `amplitude`
//! voxels either side of its column's centre, `period` voxels a wave, each strand (column)
//! on its own phase ([`phase`]). The cells above and below a face read the same curve at
//! the face's height, so the joins agree by construction. An amplitude past half a voxel
//! carries the cord into the neighbouring columns: the plant pass (the default draw) widens
//! a drape cell's front face quad to [`DrapeCords::reach`] either side, so the cell draws
//! its cord wherever it swings, at the cell's own depth. The walk (stamped plants) only
//! reaches a cell by entering it, so there the swing is held inside the column
//! ([`DrapeCords::amp`]).
//!
//! **Octaves** (round 3): the curve is a sum of `octaves` sines, octave `k` at about `2^k`
//! times the base frequency ([`WaveRatios`]) and [`GAIN`]`^k` of its amplitude, each on its
//! own phase per strand ([`octave_phase`]), the sum divided by the sum of the gains so the
//! centre never swings past `amplitude` either way. One octave is round 2's single sine.
//! [`WaveRatios::Octave`]: exactly 1 : 2 : 4, so every strand is one shape shifted and
//! repeats every period; [`WaveRatios::Jittered`]: about 1 : 2.3 : 4.6, each strand's
//! ratios moved a little by its own hash ([`DrapeCords::freq`]), so no two strands share a
//! shape and nothing repeats.

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
    /// The long wave (round 2); with it on, `shape` and `wander` are not read.
    pub wave: CordWave,
}

/// A cord's long wave: its centre a sine down the whole strand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CordWave {
    /// Voxels from crest to crest down the strand; 0: no wave, the hashed joins of round 1.
    pub period: f32,
    /// Voxels the cord's centre swings either side of its column's centre (past 0.5 it
    /// crosses into the next column).
    pub amplitude: f32,
    /// How many octaves the curve sums (round 3), 1..=15: 1 is a single sine.
    pub octaves: u32,
    /// The octaves' frequency ratios (a look choice, Wrysk's).
    pub ratios: WaveRatios,
}

/// The octaves' frequencies over the base's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WaveRatios {
    /// Literal octaves, exactly `2^k`: every strand's curve the same shape, shifted by its
    /// phases, repeating every base period.
    Octave,
    /// Off-ratio: `2^k` times [`JITTER_MEAN`], moved up to [`JITTER_SPREAD`] either way by
    /// a hash of the strand and octave (about 1 : 2.3 : 4.6): no two strands alike, no repeat.
    #[default]
    Jittered,
}

/// [`WaveRatios::Jittered`]: an octave's frequency over `2^k`, on average.
pub const JITTER_MEAN: f32 = 1.15;

/// [`WaveRatios::Jittered`]: how far a strand's hash moves that factor either way.
pub const JITTER_SPREAD: f32 = 0.05;

/// The salt of an octave's jitter hash.
const JITTER_SALT: i32 = 79;

/// Each octave's amplitude over the one below.
pub const GAIN: f32 = 0.5;

impl CordWave {
    pub const NONE: CordWave = CordWave {
        period: 0.0,
        amplitude: 0.0,
        octaves: 1,
        ratios: WaveRatios::Jittered,
    };

    /// The octave count as the uniform carries it (1..=15).
    pub fn octave_count(&self) -> u32 {
        self.octaves.clamp(1, 15)
    }

    /// The sum of the octaves' gains: the divisor that holds the swing to `amplitude`.
    pub fn gain_sum(&self) -> f32 {
        (0..self.octave_count()).map(|k| GAIN.powi(k as i32)).sum()
    }

    /// The curve's steepest slope over a single sine's of the same period and amplitude
    /// (the octaves' slopes all lined up, each at its highest frequency): 1 for one octave.
    pub fn slope_factor(&self) -> f32 {
        let top = match self.ratios {
            WaveRatios::Octave => 1.0,
            WaveRatios::Jittered => JITTER_MEAN + JITTER_SPREAD,
        };
        let f = |k: u32| {
            if k == 0 {
                1.0
            } else {
                top * 2f32.powi(k as i32)
            }
        };
        (0..self.octave_count())
            .map(|k| GAIN.powi(k as i32) * f(k))
            .sum::<f32>()
            / self.gain_sum()
    }

    /// As the uniform carries them: the period in quarter voxels, the amplitude in 32nds.
    fn quantised(&self) -> (f32, f32) {
        let p = (self.period.max(0.0) * 4.0).round().min(255.0);
        let a = (self.amplitude.max(0.0) * 32.0).round().min(255.0);
        (p / 4.0, a / 32.0)
    }

    pub fn on(&self) -> bool {
        self.quantised().0 > 0.0
    }
}

impl Default for DrapeCords {
    fn default() -> DrapeCords {
        DrapeCords::OFF
    }
}

impl DrapeCords {
    /// Drape cells drawn as whole cubes.
    pub const OFF: DrapeCords = DrapeCords {
        width_px: 0,
        shape: CordShape::Straight,
        wander: 1.0,
        wave: CordWave::NONE,
    };

    pub fn on(&self) -> bool {
        self.width_px > 0
    }

    /// `u.tex.z` and `u.tex.w`: the width, and the shape (bit 0) with the wave's ratios
    /// (bit 1: jittered) and octave count (bits 4..8), the wander in hundredths (second byte), the wave's period in quarter voxels (third) and its
    /// amplitude in 32nds of a voxel (fourth).
    pub fn uniform(&self) -> [i32; 2] {
        let shape = match self.shape {
            CordShape::Straight => 0,
            CordShape::Eased => 1,
        };
        let (p, a) = self.wave.quantised();
        let wave = ((p * 4.0) as u32) << 16 | ((a * 32.0) as u32) << 24;
        let jit = (self.wave.ratios == WaveRatios::Jittered) as u32;
        let low = shape as u32 | jit << 1 | self.wave.octave_count() << 4;
        [
            self.width_px.min(255) as i32,
            (low | (self.wander_pct() as u32) << 8 | wave) as i32,
        ]
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
        span(w, c, up, down, x)
    }

    /// The walk's test (`plantTexel` on a drape cell): whether front-face texel `(dx, dy)`
    /// (`dy` 0 at the top) of cell `(x, y, z)` is the cord's. Every texel is, with the cords
    /// off: the cube as before.
    pub fn texel(&self, s: u32, x: i32, y: i32, z: i32, dx: u32, dy: u32) -> bool {
        if !self.on() {
            return true;
        }
        let t = (dy as f32 + 0.5) / s as f32;
        if self.wave.on() {
            return self.wave_covers(
                s,
                (x, y, z),
                t,
                dx as f32 + 0.5,
                self.amp(s, true),
                (0.0, 0.0),
            );
        }
        let (a, b) = self.ends(s, x, y, z);
        self.covers(s, a, b, t, dx as f32 + 0.5)
    }

    /// `cordAmp`: the wave's amplitude, voxels. In the walk (`walk`) it is held inside the
    /// column, the cord's width short of its edges: the walk sees a cell only by entering it.
    pub fn amp(&self, s: u32, walk: bool) -> f32 {
        let a = self.wave.quantised().1;
        if !walk {
            return a;
        }
        let w = self.width_px as f32;
        a.min(((s as f32 - w) / (2.0 * s as f32)).max(0.0))
    }

    /// `cordWaveAt`: the cord's centre on strand (column) `(x, z)` at height `yw` (voxels,
    /// continuous: a cell's top face is at its `y + 1`), face pixels from the column's left
    /// edge, swinging at most `amp` voxels either side: the octaves summed
    /// (`cordWaveAt`'s loop), the sum over the gains' sum.
    pub fn wave_at(&self, s: u32, x: i32, z: i32, yw: f32, amp: f32) -> f32 {
        let p = self.wave.quantised().0.max(0.25);
        let (mut sum, mut g) = (0.0f32, 1.0f32);
        for k in 0..self.wave.octave_count() {
            let a = std::f32::consts::TAU * (yw * self.freq(x, z, k) / p + octave_phase(x, z, k));
            sum += g * a.sin();
            g *= GAIN;
        }
        s as f32 * (0.5 + amp * sum / self.wave.gain_sum())
    }

    /// `cordFreq`: octave `k`'s frequency on strand `(x, z)` over the base's: `2^k`, or for
    /// [`WaveRatios::Jittered`] (past the base) `2^k` times [`JITTER_MEAN`] give or take
    /// [`JITTER_SPREAD`] by the strand's hash.
    pub fn freq(&self, x: i32, z: i32, k: u32) -> f32 {
        let o = (1u32 << k) as f32;
        if k == 0 || self.wave.ratios == WaveRatios::Octave {
            return o;
        }
        let u = (cell_hash(x, k as i32, z, JITTER_SALT) >> 8) as f32 / 16_777_216.0;
        o * (JITTER_MEAN + JITTER_SPREAD * (2.0 * u - 1.0))
    }

    /// `cordWaveCentre`: the centre at `t` down cell `(x, y, z)`'s front face, moved by
    /// `shift` pixels at its top face and at its bottom face (the sway of each face relative
    /// to the cell's, [`swayed_join`]), linearly between.
    pub fn wave_centre(
        &self,
        s: u32,
        (x, y, z): (i32, i32, i32),
        t: f32,
        amp: f32,
        shift: (f32, f32),
    ) -> f32 {
        self.wave_at(s, x, z, y as f32 + 1.0 - t, amp) + shift.0 + (shift.1 - shift.0) * t
    }

    /// `cordWaveCovers`: whether face point `(xp, t)` of the cell (`xp` face pixels from its
    /// left edge, any sign: the cord may be in the next column) is on the waving cord. As
    /// [`Self::covers`]: `width_px` pixels a row, widened where a row steps more than a
    /// pixel so the rows stay joined; the rows past a face are the next cell's, read from
    /// the same curve.
    pub fn wave_covers(
        &self,
        s: u32,
        cell: (i32, i32, i32),
        t: f32,
        xp: f32,
        amp: f32,
        shift: (f32, f32),
    ) -> bool {
        let dt = 1.0 / s as f32;
        let c = self.wave_centre(s, cell, t, amp, shift);
        let up = (c - self.wave_centre(s, cell, t - dt, amp, shift)).abs();
        let down = (self.wave_centre(s, cell, t + dt, amp, shift) - c).abs();
        span(self.width_px as f32, c, up, down, xp)
    }

    /// `PlantPush.t.w`: how far the plant pass's front face quad of a drape cell reaches
    /// either side of its column's centre, voxels (0.5: the cell's own face). It holds the
    /// whole swing, the cord's half width, the widening of its steepest rows and the sway of
    /// its joins against its box, with a margin.
    pub fn reach(&self, s: u32) -> f32 {
        if !self.on() || !self.wave.on() {
            return 0.5;
        }
        let (p, a) = self.wave.quantised();
        let px = 0.5 * self.width_px as f32
            + std::f32::consts::PI * a * self.wave.slope_factor() / p
            + 3.0;
        (a + px / s as f32).max(0.5)
    }
}

/// `cordSpan`: whether `x` is on a cord row centred at `c`, `w` pixels wide, its centre
/// `up` and `down` pixels from the rows above and below. Half-open, so a cord covers
/// exactly `w` pixel centres of a row wherever it lies; a row more than a pixel from a
/// neighbour takes half the excess as well, so consecutive rows always touch.
fn span(w: f32, c: f32, up: f32, down: f32, x: f32) -> bool {
    let ext = (0.5 * up.max(down) - 0.5).max(0.0);
    let r = x - c;
    r >= -0.5 * w - ext && r < 0.5 * w + ext
}

/// `cordPhase`: strand (column) `(x, z)`'s place in its wave at height 0, 0..1 of a period
/// (`x` already round the ring).
pub fn phase(x: i32, z: i32) -> f32 {
    octave_phase(x, z, 0)
}

/// `cordPhase`: strand `(x, z)`'s place in octave `k`'s wave at height 0, 0..1 (octave 0 is
/// [`phase`]).
pub fn octave_phase(x: i32, z: i32, k: u32) -> f32 {
    (cell_hash(x, k as i32, z, PHASE_SALT) >> 8) as f32 / 16_777_216.0
}

/// The salt of a strand's phase hash.
const PHASE_SALT: i32 = 78;

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
                    v.push(DrapeCords {
                        width_px,
                        shape,
                        wander,
                        wave: CordWave::NONE,
                    });
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
                    assert!(
                        touch,
                        "{c:?} at {s} px, ({x}, {y}, {z}): {up:?} over {down:?}"
                    );
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
                        assert!(
                            touch,
                            "{c:?} at {s} px, ({x}, {y}, {z}) rows {dy}: {a:?} / {b:?}"
                        );
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
                let plumb = DrapeCords {
                    width_px,
                    shape,
                    wander: 0.0,
                    wave: CordWave::NONE,
                };
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
                let c = DrapeCords {
                    width_px,
                    shape,
                    wander: 1.0,
                    wave: CordWave::NONE,
                };
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
                let on = DrapeCords {
                    width_px: 1,
                    shape: CordShape::Straight,
                    wander: 1.0,
                    wave: CordWave::NONE,
                };
                let shown = (0..s)
                    .flat_map(|dy| (0..s).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| on.texel(s, x, y, z, dx, dy))
                    .count() as u32;
                assert!(shown < s * s, "a cord leaves holes at {s} px");
                assert!(shown >= s, "a cord crosses every row");
                assert!(
                    (0..s).all(|dy| (0..s).all(|dx| DrapeCords::OFF.texel(s, x, y, z, dx, dy)))
                );
            }
        }
    }

    #[test]
    fn a_swaying_join_is_at_one_screen_point_from_both_of_its_cells() {
        let s = 6;
        let c = DrapeCords {
            width_px: 1,
            shape: CordShape::Eased,
            wander: 1.0,
            wave: CordWave::NONE,
        };
        let (x, y, z) = (11, 5, 3);
        let join = c.ends(s, x, y + 1, z).1;
        // The face's weight lies between the two cells' weights; the tip is anything.
        let (w_upper, w_face, w_lower) = (0.42, 0.37, 0.31);
        for tip in [-0.75f32, -0.2, 0.0, 0.33, 0.75] {
            // Each box's left edge is where its cell's sway put it.
            let screen = |w_cell: f32| {
                (x as f32 + tip * w_cell) * s as f32 + swayed_join(s, join, tip, w_face, w_cell)
            };
            let (a, b) = (screen(w_upper), screen(w_lower));
            assert!((a - b).abs() < 1e-4, "tip {tip}: {a} vs {b}");
        }
    }

    #[test]
    fn the_uniform_carries_the_width_shape_and_wander() {
        let mut c = DrapeCords {
            width_px: 2,
            shape: CordShape::Eased,
            wander: 0.5,
            wave: CordWave::NONE,
        };
        assert_eq!(
            c.uniform(),
            [2, 1 | 2 | (1 << 4) | (50 << 8)],
            "one octave without the wave"
        );
        c.wave = CordWave {
            period: 6.25,
            amplitude: 1.5,
            octaves: 3,
            ratios: WaveRatios::Octave,
        };
        let w = c.uniform()[1] as u32;
        assert_eq!(
            w & 0xFF0D,
            1 | (50 << 8),
            "the wave leaves the shape and wander"
        );
        assert_eq!((w >> 4) & 15, 3, "the octave count");
        assert_eq!(w & 2, 0, "literal octaves");
        c.wave.ratios = WaveRatios::Jittered;
        assert_eq!(c.uniform()[1] as u32 & 2, 2, "jittered ratios");
        assert_eq!(((w >> 16) & 255) as f32 / 4.0, 6.25);
        assert_eq!((w >> 24) as f32 / 32.0, 1.5);
        assert_eq!(DrapeCords::OFF.uniform()[0], 0);
        assert!(!DrapeCords::OFF.on());
    }

    fn waves() -> Vec<DrapeCords> {
        let mut v = Vec::new();
        for width_px in [1, 2] {
            for (period, amplitude) in [(3.0, 0.5), (6.0, 1.0), (12.0, 2.0), (3.0, 2.0), (7.5, 0.2)]
            {
                for octaves in [1, 3, 5] {
                    for ratios in [WaveRatios::Octave, WaveRatios::Jittered] {
                        let wave = CordWave {
                            period,
                            amplitude,
                            octaves,
                            ratios,
                        };
                        v.push(DrapeCords {
                            width_px,
                            shape: CordShape::Eased,
                            wander: 0.5,
                            wave,
                        });
                    }
                }
            }
            // The defaults' neighbourhood: long periods with octaves.
            for period in [12.0, 24.0] {
                for ratios in [WaveRatios::Octave, WaveRatios::Jittered] {
                    let wave = CordWave {
                        period,
                        amplitude: 0.5,
                        octaves: 3,
                        ratios,
                    };
                    v.push(DrapeCords {
                        width_px,
                        shape: CordShape::Eased,
                        wander: 0.5,
                        wave,
                    });
                }
            }
        }
        v
    }

    /// The pixels of row `dy` of cell `(x, y, z)` on the waving cord as the plant pass draws
    /// it (no walk limit), over the reach either side, with each face's sway `shift`.
    fn wave_row(
        c: &DrapeCords,
        s: u32,
        cell: (i32, i32, i32),
        dy: u32,
        shift: (f32, f32),
    ) -> Vec<i32> {
        let r = (c.reach(s) * s as f32).ceil() as i32 + 2 * s as i32;
        let t = (dy as f32 + 0.5) / s as f32;
        (-r..s as i32 + r)
            .filter(|&dx| c.wave_covers(s, cell, t, dx as f32 + 0.5, c.amp(s, false), shift))
            .collect()
    }

    #[test]
    fn under_the_long_wave_two_stacked_cells_meet_at_one_point_of_their_shared_face() {
        for c in waves() {
            for s in SCALES {
                for (x, y, z) in positions() {
                    let (upper, lower) = ((x, y + 1, z), (x, y, z));
                    for walk in [false, true] {
                        let a = c.amp(s, walk);
                        let up_end = c.wave_centre(s, upper, 1.0, a, (0.0, 0.0));
                        let low_top = c.wave_centre(s, lower, 0.0, a, (0.0, 0.0));
                        assert!(
                            (up_end - low_top).abs() < 1e-3,
                            "{c:?} at {s} px: {up_end} vs {low_top}"
                        );
                    }
                    // Swaying: the face's shift is the upper's bottom and the lower's top.
                    let face = 0.7;
                    let a = c.amp(s, false);
                    let up_end = c.wave_centre(s, upper, 1.0, a, (-0.3, face));
                    let low_top = c.wave_centre(s, lower, 0.0, a, (face, 1.9));
                    assert!((up_end - low_top).abs() < 1e-3);
                    // And the drawn pixels touch across the face.
                    let up = wave_row(&c, s, upper, s - 1, (0.0, 0.0));
                    let down = wave_row(&c, s, lower, 0, (0.0, 0.0));
                    let touch = up.iter().any(|&p| down.iter().any(|&q| p.abs_diff(q) <= 1));
                    assert!(
                        touch,
                        "{c:?} at {s} px, ({x}, {y}, {z}): {up:?} over {down:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn under_the_long_wave_a_row_less_than_a_pixel_from_its_neighbours_is_the_width_at_every_scale()
    {
        for c in waves() {
            for s in SCALES.into_iter().filter(|&s| s >= c.width_px) {
                let a = c.amp(s, false);
                for (x, y, z) in positions().take(60) {
                    for dy in 0..s {
                        let n = wave_row(&c, s, (x, y, z), dy, (0.0, 0.0)).len() as u32;
                        let t = (dy as f32 + 0.5) / s as f32;
                        let dt = 1.0 / s as f32;
                        let at = |t: f32| c.wave_centre(s, (x, y, z), t, a, (0.0, 0.0));
                        let step = (at(t) - at(t - dt)).abs().max((at(t + dt) - at(t)).abs());
                        assert!(n >= c.width_px, "{c:?} at {s} px: row {dy} has {n}");
                        if step <= 1.0 {
                            assert_eq!(n, c.width_px, "{c:?} at {s} px: row {dy}, step {step}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn past_half_a_voxel_the_cord_shows_in_the_next_column_and_the_plant_quad_holds_all_of_it() {
        for c in waves() {
            for s in SCALES.into_iter().filter(|&s| s >= c.width_px) {
                let mut outside = 0;
                for (x, y, z) in positions().take(80) {
                    for dy in 0..s {
                        for sh in [(0.0, 0.0), (-1.5, 1.5)] {
                            for dx in wave_row(&c, s, (x, y, z), dy, sh) {
                                let off = (dx as f32 + 0.5) / s as f32 - 0.5;
                                assert!(
                                    off.abs() <= c.reach(s),
                                    "{c:?} at {s} px: {dx} past the quad"
                                );
                                if dx < 0 || dx >= s as i32 {
                                    outside += 1;
                                }
                            }
                        }
                    }
                }
                if c.amp(s, false) > 1.0 {
                    assert!(outside > 0, "{c:?} at {s} px never leaves its column");
                }
            }
        }
    }

    #[test]
    fn in_the_walk_the_wave_stays_in_its_column_and_still_crosses_every_row() {
        for c in waves() {
            for s in SCALES.into_iter().filter(|&s| s > c.width_px) {
                for (x, y, z) in positions().take(40) {
                    for dy in 0..s {
                        let n = (0..s).filter(|&dx| c.texel(s, x, y, z, dx, dy)).count();
                        assert!(n >= 1, "{c:?} at {s} px: row {dy} empty");
                        assert!(n < s as usize || s <= 2, "{c:?} at {s} px: row {dy} full");
                    }
                }
            }
        }
    }

    #[test]
    fn with_octaves_the_centre_never_swings_past_the_amplitude_and_uses_its_room() {
        for c in waves() {
            for s in SCALES {
                let a = c.amp(s, false);
                let mut most = 0.0f32;
                for (x, _, z) in positions().take(40) {
                    for i in 0..400 {
                        let yw = i as f32 * 0.173 - 20.0;
                        let off = (c.wave_at(s, x, z, yw, a) - 0.5 * s as f32) / s as f32;
                        assert!(off.abs() <= a + 1e-4, "{c:?} at {s} px: {off} past {a}");
                        most = most.max(off.abs());
                    }
                }
                // Scaled by the gains' sum, not by more: the swing comes near the amplitude.
                assert!(
                    most >= 0.5 * a,
                    "{c:?} at {s} px: swings only {most} of {a}"
                );
            }
        }
    }

    #[test]
    fn one_octave_is_the_single_sine_and_more_octaves_add_finer_waves() {
        let s = 6;
        let wave = CordWave {
            period: 12.0,
            amplitude: 0.5,
            octaves: 1,
            ratios: WaveRatios::Jittered,
        };
        let one = DrapeCords {
            width_px: 2,
            shape: CordShape::Eased,
            wander: 0.5,
            wave,
        };
        let three = DrapeCords {
            wave: CordWave {
                octaves: 3,
                ..one.wave
            },
            ..one
        };
        let (x, z) = (17, 4);
        let mut differs = false;
        for i in 0..200 {
            let yw = i as f32 * 0.25;
            let sine =
                s as f32 * (0.5 + 0.5 * (std::f32::consts::TAU * (yw / 12.0 + phase(x, z))).sin());
            assert!((one.wave_at(s, x, z, yw, 0.5) - sine).abs() < 1e-3);
            differs |= (three.wave_at(s, x, z, yw, 0.5) - sine).abs() > 0.1;
        }
        assert!(differs, "the octaves change the curve");
        // Each strand has its own phases in every octave.
        assert_ne!(octave_phase(x, z, 1), octave_phase(x + 1, z, 1));
        assert_ne!(octave_phase(x, z, 1), octave_phase(x, z, 2));
    }

    #[test]
    fn literal_octaves_repeat_every_period_and_jittered_ones_differ_strand_to_strand() {
        let s = 6;
        for period in [12.0, 24.0] {
            let wave = CordWave {
                period,
                amplitude: 0.5,
                octaves: 3,
                ratios: WaveRatios::Octave,
            };
            let lit = DrapeCords {
                width_px: 2,
                shape: CordShape::Eased,
                wander: 0.5,
                wave,
            };
            let jit = DrapeCords {
                wave: CordWave {
                    ratios: WaveRatios::Jittered,
                    ..wave
                },
                ..lit
            };
            for (x, _, z) in positions().take(20) {
                for i in 0..40 {
                    let yw = i as f32 * 0.37;
                    let (a, b) = (
                        lit.wave_at(s, x, z, yw, 0.5),
                        lit.wave_at(s, x, z, yw + period, 0.5),
                    );
                    assert!((a - b).abs() < 1e-2, "literal octaves repeat: {a} vs {b}");
                }
                for k in 0..3 {
                    assert_eq!(lit.freq(x, z, k), (1u32 << k) as f32);
                    let f = jit.freq(x, z, k) / (1u32 << k) as f32;
                    let ok = if k == 0 {
                        f == 1.0
                    } else {
                        (f - JITTER_MEAN).abs() <= JITTER_SPREAD + 1e-6
                    };
                    assert!(ok, "octave {k}: {f}");
                }
            }
            // Jittered: two strands' octave ratios differ.
            assert_ne!(jit.freq(3, 1, 1), jit.freq(4, 1, 1));
        }
    }
}
