//! Light-effect settings and safe sun-volume uploads.
pub use crate::wgpu_backend::sunvis::SunVisVolume;
pub const TEXEL_BYTES: usize = 2;
/// The lit tier's effect switches and the light shafts' numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    /// The sun march. Off: every face turned toward the sun is sunlit (no cast shadows).
    pub shadows: bool,
    /// The water's reflection march. Off: the water reflects nothing.
    pub reflections: bool,
    /// The light shafts: the in-scatter march after the slab walk, and the volume.
    pub volumetric: bool,
    /// The emitters' local light (the glow volume's fetches). Off with emission off.
    pub glow: bool,
    /// The AO crease's neighbour fetches. Off with `[light] ao = 0`. (A uniform test on
    /// the strength instead costs the lit walk 0.47 ms at 13 px on the 5080 even with AO
    /// on: the early return changes the inlined crease's code.)
    pub ao: bool,
    /// Light scattered toward the camera per voxel of view path through fully sunlit air
    /// at the world's floor, as a fraction of the palette's light.
    pub density: f32,
    /// Voxels up over which the air's density falls by e.
    pub falloff: f32,
    /// The glow volume's weight in the air, relative to full sun.
    pub glow_weight: f32,
}

impl Default for Effects {
    /// Today's lit picture: shadows, reflections and the glow on; no shafts.
    fn default() -> Effects {
        Effects {
            shadows: true,
            reflections: true,
            volumetric: false,
            glow: true,
            ao: true,
            density: 0.03,
            falloff: 12.0,
            glow_weight: 1.0,
        }
    }
}

/// `voxel.frag`'s specialisation constants in `constant_id` order: `LIT` (0), `SHADOWS`
/// (1), `REFLECTIONS` (2), `VOLUMETRIC` (3), `GLOW` (4), `AO` (5), each a 32-bit bool.
pub const SPEC_CONSTANTS: u32 = 6;

impl Effects {
    /// The specialisation data for a pipeline of this tier. The flat tier has every lit
    /// effect off: its pipeline is the flat shader code alone, as it always was.
    pub fn spec_constants(&self, lit: bool) -> [u32; SPEC_CONSTANTS as usize] {
        [
            u32::from(lit),
            u32::from(lit && self.shadows),
            u32::from(lit && self.reflections),
            u32::from(lit && self.volumetric),
            u32::from(lit && self.glow),
            u32::from(lit && self.ao),
        ]
    }

    /// The `volK` uniform: density, glow weight, falloff, and the frame's `fade`.
    pub fn uniform(&self, fade: f32) -> [f32; 4] {
        [
            self.density.max(0.0),
            self.glow_weight.max(0.0),
            self.falloff.max(1e-3),
            fade.clamp(0.0, 1.0),
        ]
    }

    /// Whether a renderer of this tier builds the full-size volume.
    pub fn volume_on(&self, lit: bool) -> bool {
        lit && self.volumetric
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switched_off_effect_is_off_in_the_pipeline_and_the_flat_tier_has_none() {
        let all = Effects {
            shadows: true,
            reflections: true,
            volumetric: true,
            glow: true,
            ao: true,
            ..Effects::default()
        };
        let e = Effects {
            shadows: false,
            ..all
        };
        assert_eq!(e.spec_constants(true), [1, 0, 1, 1, 1, 1]);
        assert_eq!(e.spec_constants(false), [0; 6]);
        let no_volume = Effects {
            volumetric: false,
            ..all
        };
        assert_eq!(no_volume.spec_constants(true), [1, 1, 1, 0, 1, 1]);
        assert!(e.volume_on(true) && !e.volume_on(false));
        assert!(!no_volume.volume_on(true));
    }

    #[test]
    fn the_uniform_carries_the_knobs_and_a_clamped_fade() {
        let e = Effects {
            density: 0.02,
            glow_weight: 0.5,
            falloff: 30.0,
            ..Effects::default()
        };
        assert_eq!(e.uniform(0.25), [0.02, 0.5, 30.0, 0.25]);
        assert_eq!(e.uniform(3.0)[3], 1.0);
        let bad = Effects {
            density: -1.0,
            falloff: 0.0,
            ..e
        };
        let u = bad.uniform(-1.0);
        assert_eq!((u[0], u[3]), (0.0, 0.0));
        assert!(u[2] > 0.0);
    }
}
