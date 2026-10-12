//! Plant lists, motion and packing with safe geometry passes.
pub use crate::wgpu_backend::geometry::Plants as PlantPass;
pub const MAX_PLANT_CELLS: usize = 1 << 18;
/// The most plants a cell list names (a cell's plant field has 19 bits).
pub const MAX_PLANTS: usize = 1 << 16;
/// The most layer samples a raster pixel axis (`plant_main.glsl`'s `PLANT_SAMPLES`).
pub const MAX_SUPERSAMPLE: u32 = 4;
/// The largest voxel a layer id can name a texel of (six bits each for column and row).
pub const MAX_PX_PER_VOXEL: u32 = 64;

/// The breeze plants sway in: one gust packet a period travelling round the ring, a
/// flutter inside it, and a slow change in how strong each packet peaks. Presentation
/// only. Modelled on the old renderer's shared breeze (`art_present::wind`): the packet
/// eases in with zero slope, holds, eases out, and then is **exactly** calm for the rest of
/// the period, so a plant rests and then stirs.
///
/// **The ring**: a point's place round it is `ring = x / width` in `[0, 1)`. The packet
/// reaches it `sweeps · period_s · ring` seconds late and the flutter runs `flutter_waves`
/// whole waves round it, both whole numbers, so the breeze at `ring = 1` is the breeze at
/// `ring = 0` and nothing jumps at the seam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breeze {
    /// Seconds from one packet's start to the next.
    pub period_s: f64,
    /// Seconds a packet eases in over, holds, and eases out over; the rest is calm.
    pub rise_s: f64,
    pub hold_s: f64,
    pub fall_s: f64,
    /// How deep the flutter dips a packet (its factor runs over `[1 − flutter, 1]`), and
    /// its period in seconds.
    pub flutter: f64,
    pub flutter_s: f64,
    /// How far a packet's peak may fall below 1 (`[1 − peak_vary, 1]`), on this period.
    pub peak_vary: f64,
    pub peak_s: f64,
    /// Packet periods the gust front takes to go once round the ring (0: everywhere at
    /// once); its sign is the way it travels, +x for positive.
    pub sweeps: i32,
    /// Flutter waves round the ring.
    pub flutter_waves: i32,
    /// What a shower and a downpour add to the strength (each times how much of it is
    /// falling, eased with the weather).
    pub shower_gain: f64,
    pub storm_gain: f64,
}

impl Default for Breeze {
    fn default() -> Breeze {
        Breeze {
            period_s: 30.0,
            rise_s: 5.0,
            hold_s: 8.0,
            fall_s: 5.0,
            flutter: 0.3,
            flutter_s: 2.3,
            peak_vary: 0.15,
            peak_s: 97.0,
            sweeps: 1,
            flutter_waves: 8,
            shower_gain: 0.25,
            storm_gain: 0.75,
        }
    }
}

/// The clamped Hermite smoothstep `t²(3 − 2t)`.
fn hermite(t: f64) -> f64 {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    t * t * (3.0 - 2.0 * t)
}

impl Breeze {
    /// The packet's envelope at `when`: 0..1, exactly 0 in the calm.
    pub fn envelope(&self, when: f64) -> f64 {
        if !(when.is_finite() && self.period_s > 0.0) {
            return 0.0;
        }
        let u = when.rem_euclid(self.period_s);
        let (rise, hold, fall) = (
            self.rise_s.max(0.0),
            self.hold_s.max(0.0),
            self.fall_s.max(0.0),
        );
        if u < rise {
            hermite(u / rise)
        } else if u < rise + hold {
            1.0
        } else if u < rise + hold + fall {
            1.0 - hermite((u - rise - hold) / fall)
        } else {
            0.0
        }
    }

    /// How hard the breeze blows at `ring` (`x / width`, any real: it is taken round the
    /// ring) at `seconds`, with `rain` (drizzle, shower, downpour) falling: 0 in the calm,
    /// 1 at a dry packet's highest peak.
    pub fn strength(&self, ring: f64, seconds: f64, rain: [f32; 3]) -> f64 {
        if !(ring.is_finite() && seconds.is_finite()) {
            return 0.0;
        }
        let ring = ring.rem_euclid(1.0);
        let envelope = self.envelope(seconds - f64::from(self.sweeps) * self.period_s * ring);
        if envelope <= 0.0 {
            return 0.0;
        }
        let tau = std::f64::consts::TAU;
        let wave = |phase: f64, depth: f64| 1.0 - depth + depth * 0.5 * (1.0 + (tau * phase).sin());
        let flutter = if self.flutter_s > 0.0 {
            wave(
                seconds / self.flutter_s - f64::from(self.flutter_waves) * ring,
                self.flutter.clamp(0.0, 1.0),
            )
        } else {
            1.0
        };
        let peak = if self.peak_s > 0.0 {
            wave(seconds / self.peak_s, self.peak_vary.clamp(0.0, 1.0))
        } else {
            1.0
        };
        let rain =
            1.0 + self.shower_gain * f64::from(rain[1]) + self.storm_gain * f64::from(rain[2]);
        let s = envelope * flutter * peak * rain.max(0.0);
        if s.is_finite() { s.max(0.0) } else { 0.0 }
    }
}

/// The plant pass's settings, fixed for a renderer's life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlantMotion {
    /// Layer samples per raster pixel axis, `1..=`[`MAX_SUPERSAMPLE`].
    pub supersample: u32,
    /// Seconds one voxel takes to grow in from nothing, or shrink out.
    pub grow_s: f32,
    pub breeze: Breeze,
}

impl Default for PlantMotion {
    fn default() -> PlantMotion {
        PlantMotion {
            supersample: 2,
            grow_s: 0.35,
            breeze: Breeze::default(),
        }
    }
}

/// One plant of a cell list: where it stands round the ring (its root's column, voxels),
/// how far its top moves at full breeze (voxels, signed: + is +x) and how many seconds it
/// answers the breeze late. Plant 0 of every list is the still plant (no amplitude): the
/// cells no stand owns.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlantRec {
    pub x: f32,
    pub amp: f32,
    pub lag: f32,
}

/// A tick's plant cells, as the pack writes them ([`crate::voxel::VoxelStaging::plants`]):
/// each one [`pack_cell`]'s words, the plants they name, and the clock second their times
/// count from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlantList {
    pub cells: Vec<[u32; 4]>,
    pub plants: Vec<PlantRec>,
    /// Seconds of the frame clock (sim seconds) that a cell's time 0 is.
    pub epoch: f64,
    /// Frame-clock seconds at which every cell of the list has finished arriving or
    /// leaving: the picture moves on its own until then.
    pub settled_at: f64,
}

/// A cell's time while it is neither arriving nor leaving: long grown.
pub const SETTLED: f32 = -1.0e30;

/// One plant cell as `plant.glsl` reads it: voxel `(x, y, z)` (x round the ring), its
/// plant, how much of its plant's tip it moves by (`weight`, 0..1), whether it is leaving,
/// the moment it starts arriving or leaving (seconds from the list's epoch; [`SETTLED`]
/// for neither), and, leaving, the texel it was stamped with (`r | g << 8 | b << 16 | a << 24`:
/// it is no longer in the volume).
pub fn pack_cell(
    cell: [u32; 3],
    plant: u32,
    weight: f32,
    dying: bool,
    time: f32,
    texel: u32,
) -> [u32; 4] {
    let w = (weight.clamp(0.0, 1.0) * 4095.0).round() as u32;
    [
        (cell[0] & 4095) | (cell[1] & 1023) << 12 | (cell[2] & 1023) << 22,
        (plant & 0x7FFFF) | u32::from(dying) << 19 | w << 20,
        time.to_bits(),
        texel,
    ]
}

/// A settled or arriving cell's fourth word (in place of a leaving cell's texel): the
/// weights of its top and bottom faces, 12 bits each, and the top bit set to say so. The
/// plant pass moves a drape cord's joins by these ([`crate::cord`]), so two cells meeting at
/// a face, given the same weight for it, put the join at the same screen point. Bit 30 says
/// the cell is a drape cell, whose front quad `plant.vert` widens for the cord's long wave
/// ([`crate::cord::DrapeCords::reach`]).
pub fn pack_faces(top: f32, bottom: f32, drape: bool) -> u32 {
    let q = |w: f32| (w.clamp(0.0, 1.0) * 4095.0).round() as u32;
    1 << 31 | u32::from(drape) << 30 | q(top) << 12 | q(bottom)
}

/// The top and bottom face weights of [`pack_faces`], or `None` for a word without them.
pub fn unpack_faces(w: u32) -> Option<(f32, f32)> {
    (w >> 31 != 0).then(|| {
        (
            ((w >> 12) & 4095) as f32 / 4095.0,
            (w & 4095) as f32 / 4095.0,
        )
    })
}

/// The voxel, plant, weight, leaving flag and time of a packed cell ([`pack_cell`]).
pub fn unpack_cell(c: [u32; 4]) -> ([u32; 3], u32, f32, bool, f32) {
    (
        [c[0] & 4095, (c[0] >> 12) & 1023, c[0] >> 22],
        c[1] & 0x7FFFF,
        (c[1] >> 20) as f32 / 4095.0,
        c[1] & 0x80000 != 0,
        f32::from_bits(c[2]),
    )
}

/// How big a cell is drawn at `t` (seconds from the list's epoch), 0..1 about its centre:
/// `plant.glsl`'s `plantScale`. Arriving from `time`, it grows over `grow_s`; leaving from
/// `time`, it shrinks; both smoothstepped.
pub fn cell_scale(t: f32, time: f32, dying: bool, grow_s: f32) -> f32 {
    let since = t - time;
    let r = if grow_s > 0.0 {
        (since / grow_s).clamp(0.0, 1.0)
    } else if since >= 0.0 {
        1.0
    } else {
        0.0
    };
    let s = r * r * (3.0 - 2.0 * r);
    if dying { 1.0 - s } else { s }
}

/// The ramp position `r` in 0..1 whose smoothstep is `s`: what a reversed change starts
/// from so the scale goes on from where it was.
pub fn unsmooth(s: f32) -> f32 {
    let s = s.clamp(0.0, 1.0);
    0.5 - ((1.0 - 2.0 * s).asin() / 3.0).sin()
}

/// Each plant's tip this instant: `amp` times the breeze at its column, `lag` seconds late.
pub fn tips_into(
    plants: &[PlantRec],
    breeze: &Breeze,
    width: u32,
    seconds: f64,
    rain: [f32; 3],
    out: &mut Vec<f32>,
) {
    out.clear();
    let w = f64::from(width.max(1));
    out.extend(plants.iter().map(|p| {
        if p.amp == 0.0 {
            0.0
        } else {
            (f64::from(p.amp)
                * breeze.strength(f64::from(p.x) / w, seconds - f64::from(p.lag), rain))
                as f32
        }
    }));
}

/// The push block (`plant.glsl`'s `PlantPush`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlantStats {
    pub cells: usize,
    pub plants: usize,
    /// Cells arriving or leaving at the list's epoch.
    pub changing: usize,
    /// Cells a list had beyond [`MAX_PLANT_CELLS`] (not drawn).
    pub dropped: usize,
}
#[cfg(test)]
mod tests {
    use super::*;

    const DRY: [f32; 3] = [0.0; 3];

    #[test]
    fn the_breeze_is_the_same_on_both_sides_of_the_seam() {
        // Whole sweeps and flutter waves round the ring: ring 1 is ring 0, and the last
        // column before the seam is as close to the first as any two neighbours are.
        let b = Breeze::default();
        let width = 640.0;
        let mut worst_seam: f64 = 0.0;
        let mut worst_step: f64 = 0.0;
        for k in 0..2000 {
            let t = 0.037 * f64::from(k);
            assert_eq!(
                b.strength(0.0, t, DRY),
                b.strength(1.0, t, DRY),
                "ring 1 is ring 0 at t {t}"
            );
            worst_seam =
                worst_seam.max((b.strength(639.0 / width, t, DRY) - b.strength(0.0, t, DRY)).abs());
            worst_step = worst_step
                .max((b.strength(320.0 / width, t, DRY) - b.strength(321.0 / width, t, DRY)).abs());
        }
        assert!(
            worst_seam <= worst_step + 1e-12,
            "across the seam {worst_seam} vs a column step {worst_step}"
        );
        assert!(worst_step > 0.0, "the breeze does vary along the ring");
    }

    #[test]
    fn a_packet_rests_then_stirs_and_never_jumps() {
        let b = Breeze::default();
        // Exactly calm after the fall, at every place.
        let calm = b.rise_s + b.hold_s + b.fall_s + 0.5;
        assert_eq!(b.strength(0.0, calm, DRY), 0.0);
        assert!(
            b.strength(0.0, b.rise_s + 1.0, DRY) > 0.0,
            "a gust in the hold"
        );
        // Continuous on the frame clock: no step bigger than a small slope allows.
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while t < 2.0 * b.period_s {
            let (a, c) = (b.strength(0.3, t, DRY), b.strength(0.3, t + dt, DRY));
            assert!((a - c).abs() < 0.05, "a jump of {} at t {t}", (a - c).abs());
            t += dt;
        }
    }

    #[test]
    fn rain_strengthens_the_breeze_and_only_where_it_blows() {
        let b = Breeze::default();
        let t = b.rise_s + 2.0;
        assert!(b.strength(0.0, t, [0.0, 0.0, 1.0]) > b.strength(0.0, t, DRY));
        let calm = b.rise_s + b.hold_s + b.fall_s + 1.0;
        assert_eq!(
            b.strength(0.0, calm, [0.0, 0.0, 1.0]),
            0.0,
            "a calm stays calm in the rain"
        );
    }

    #[test]
    fn face_weights_round_trip_and_a_shared_face_packs_alike_from_both_cells() {
        assert_eq!(unpack_faces(0), None, "a word without face weights");
        let (t, b) = unpack_faces(pack_faces(0.75, 0.25, true)).unwrap();
        assert!((t - 0.75).abs() < 1e-3 && (b - 0.25).abs() < 1e-3);
        // The face between a lower and an upper cell: the lower's top, the upper's bottom.
        let face = 0.4321;
        let lower = unpack_faces(pack_faces(face, 0.3, false)).unwrap().0;
        let upper = unpack_faces(pack_faces(0.6, face, true)).unwrap().1;
        assert_eq!(lower, upper);
    }

    #[test]
    fn a_cell_round_trips_its_packing() {
        let c = pack_cell([639, 127, 95], 4321, 0.5, true, 1.25, 0xAABBCCDD);
        let (cell, plant, w, dying, t) = unpack_cell(c);
        assert_eq!((cell, plant, dying, t), ([639, 127, 95], 4321, true, 1.25));
        assert!((w - 0.5).abs() < 1.0 / 4095.0);
        assert_eq!(c[3], 0xAABBCCDD);
    }

    #[test]
    fn a_cell_scales_in_and_out_continuously_and_a_reversal_starts_where_it_was() {
        let g = 0.4;
        assert_eq!(cell_scale(-0.1, 0.0, false, g), 0.0, "not yet arrived");
        assert_eq!(cell_scale(1.0, 0.0, false, g), 1.0, "grown");
        assert_eq!(cell_scale(-0.1, 0.0, true, g), 1.0, "not yet leaving");
        assert_eq!(cell_scale(1.0, 0.0, true, g), 0.0, "gone");
        let mid = cell_scale(0.1, 0.0, false, g);
        assert!(mid > 0.0 && mid < 1.0);
        // Reversed at t = 0.1: leaving from the moment that gives the same scale now.
        let r = unsmooth(1.0 - mid);
        let t_off = 0.1 - r * g;
        assert!(
            (cell_scale(0.1, t_off, true, g) - mid).abs() < 1e-5,
            "no jump at the reversal"
        );
    }

    #[test]
    fn a_still_plant_has_no_tip_and_a_plant_follows_the_breeze_late() {
        let b = Breeze {
            sweeps: 0,
            flutter: 0.0,
            peak_vary: 0.0,
            ..Breeze::default()
        };
        let plants = [
            PlantRec::default(),
            PlantRec {
                x: 10.0,
                amp: 2.0,
                lag: 0.0,
            },
            PlantRec {
                x: 10.0,
                amp: 2.0,
                lag: 1.0,
            },
        ];
        let mut tips = Vec::new();
        tips_into(&plants, &b, 64, b.rise_s * 0.5, DRY, &mut tips);
        assert_eq!(tips[0], 0.0);
        assert!(
            tips[1] > tips[2] && tips[2] > 0.0,
            "the lagging plant is behind on the rise: {tips:?}"
        );
    }
}
