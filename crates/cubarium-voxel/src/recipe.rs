//! What a landscape is made of, in metres: the generation recipe and the choice of
//! generator.
//!
//! A [`Recipe`] holds **generation-only** parameters — wavelengths, relief, persistence,
//! lacunarity, octave counts, layer thicknesses — and names one seed stream per pass.
//! Runtime water and ecology settings stay in [`Config`]; nothing in here is read after
//! the world exists.
//!
//! Every length is a length. Feature size is fixed by the recipe, never by `width` or
//! `voxel_m`: doubling the width adds landforms of the same size, halving `voxel_m`
//! resolves the same landforms more finely, and octaves finer than
//! [`Recipe::min_feature_voxels`] voxels are dropped rather than aliased onto the grid.
//!
//! The named streams are why a pass can be retuned on its own. Each one is xored into
//! [`Config::seed`], so moving the ridged noise leaves the broad relief, the rocky mask
//! and the strata exactly where they were.

use serde::{Deserialize, Serialize};

use crate::Config;
use crate::noise::{Ladder, fbm, resolved_octaves, ridged, ring_cells, ring_noise, smoothstep};

/// Which generator builds the terrain.
///
/// [`Landform::Ridge`] is the original feature-guided generator — one broad ridge, one
/// receiving basin, a weak correlated wobble — and stays the default, so every fixture
/// and harness that never mentions a landform keeps the world it had.
// A `Recipe` is a couple of hundred bytes beside a unit variant. It lives in `Config`,
// which is cloned once per world and never in a hot loop, so the size is not worth an
// indirection that every generator call would then have to follow.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Landform {
    /// The original generator. Unchanged, down to the byte.
    #[default]
    Ridge,
    /// Staged periodic relief in physical units: broad multi-octave relief, ridged noise
    /// warped into rocky regions, and restrained fine detail, voxelised through the same
    /// layering and camera pass.
    Staged(Recipe),
}

/// One deterministic stream per generation pass. Distinct constants, xored into the
/// world seed: retuning one pass does not reshuffle the others.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Streams {
    /// Broad multi-octave relief.
    pub relief: u64,
    /// Ridged noise in the rocky regions.
    pub ridge: u64,
    /// The domain warp applied to the ridged noise.
    pub warp: u64,
    /// Which stretches of the ring are rocky.
    pub rocky: u64,
    /// Rock thickness, strata warp and the soil pockets.
    pub material: u64,
}

impl Default for Streams {
    fn default() -> Streams {
        Streams {
            relief: 0x_5354_4147_5F52_454C,
            ridge: 0x_5354_4147_5F52_4447,
            warp: 0x_5354_4147_5F57_5250,
            rocky: 0x_5354_4147_5F52_4B59,
            material: 0x_5354_4147_5F4D_4154,
        }
    }
}

/// A landscape in metres. [`Recipe::default`] is the `default` preset.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Recipe {
    /// Elevation the front edge is built around, metres above `y = 0`.
    pub base_m: f64,
    /// How much higher the back wall stands than the front. The camera looks slightly
    /// down from the front, so this is what keeps the far terrain readable.
    pub back_rise_m: f64,
    /// Relief below this is compressed rather than cut: a receiving basin with a floor,
    /// not a clipped trough.
    pub basin_floor_m: f64,
    /// Depth anisotropy. The habitat is a few metres deep against tens of metres around,
    /// so features are stretched into `z` by this factor; `1.0` is isotropic.
    pub depth_scale: f64,

    /// Wavelength of the coarsest relief octave, metres. This is the landform size, and
    /// how many fit is `circumference / wavelength`.
    pub relief_wavelength_m: f64,
    /// Amplitude of the whole relief ladder, metres.
    pub relief_m: f64,
    /// Octaves the relief ladder asks for; the resolution limit may drop the finest.
    pub relief_octaves: u32,
    /// Amplitude ratio between successive octaves. Below 0.5 the fine detail is
    /// restrained, which is what a 0.25 m voxel can actually show.
    pub persistence: f64,
    /// Frequency ratio between successive octaves.
    pub lacunarity: f64,

    /// Wavelength of the coarsest ridged octave, metres.
    pub ridge_wavelength_m: f64,
    /// Amplitude of the ridged noise where the rocky mask is full, metres.
    pub ridge_relief_m: f64,
    /// Octaves the ridged ladder asks for.
    pub ridge_octaves: u32,

    /// Wavelength of the field that decides which stretches of ring are rocky, metres.
    pub rocky_wavelength_m: f64,
    /// Roughly what share of the ring the ridged noise reaches, `0..=1`.
    pub rocky_fraction: f64,

    /// Wavelength of the domain warp, metres.
    pub warp_wavelength_m: f64,
    /// How far the domain warp displaces the ridged noise, metres. Modest on purpose:
    /// it is there to bend parallel ridges, not to fold the landscape.
    pub warp_m: f64,

    /// The finest octave the generator will keep, in voxels of wavelength. Two voxels is
    /// the Nyquist limit of the grid the terrain is quantised onto.
    pub min_feature_voxels: f64,

    /// Soil on ground with no slope, metres.
    pub soil_flat_m: f64,
    /// Extra soil in receiving ground, metres. Slice 2 replaces this with transported
    /// sediment; until then soil still comes from slope and deposition, never from the
    /// elevation noise.
    pub soil_deposit_m: f64,
    /// Soil everywhere, even on a bare flank, metres.
    pub soil_floor_m: f64,
    /// Deepest soil a column may carry, metres.
    pub soil_max_m: f64,

    /// Rock between the soil and the bedrock core, metres.
    pub rock_m: f64,
    /// How far that thickness swings around the ring, metres.
    pub rock_swing_m: f64,
    /// Spacing of the hard strata water perches on, metres.
    pub strata_m: f64,
    /// How far the strata are warped around the ring, metres.
    pub strata_warp_m: f64,

    /// Soil pockets inside the rock.
    pub pockets: u32,

    /// The per-pass seed streams.
    pub streams: Streams,
}

impl Default for Recipe {
    fn default() -> Recipe {
        Recipe::DEFAULT
    }
}

impl Recipe {
    /// The `default` ring: 128 x 48 x 24 at 0.25 m, 32 m around, 12 m of headroom.
    /// Six resolved octaves from 16 m down to 0.5 m; two broad landforms.
    pub const DEFAULT: Recipe = Recipe {
        base_m: 3.0,
        back_rise_m: 3.1,
        basin_floor_m: 1.3,
        depth_scale: 0.5,
        relief_wavelength_m: 16.0,
        relief_m: 2.5,
        relief_octaves: 6,
        persistence: 0.5,
        lacunarity: 2.0,
        ridge_wavelength_m: 8.0,
        ridge_relief_m: 1.4,
        ridge_octaves: 3,
        rocky_wavelength_m: 21.0,
        rocky_fraction: 0.45,
        warp_wavelength_m: 18.0,
        warp_m: 1.6,
        min_feature_voxels: 2.0,
        soil_flat_m: 0.8,
        soil_deposit_m: 0.6,
        soil_floor_m: 0.08,
        soil_max_m: 1.5,
        rock_m: 1.5,
        rock_swing_m: 0.75,
        strata_m: 0.5,
        strata_warp_m: 0.4,
        pockets: 3,
        streams: Streams {
            relief: 0x_5354_4147_5F52_454C,
            ridge: 0x_5354_4147_5F52_4447,
            warp: 0x_5354_4147_5F57_5250,
            rocky: 0x_5354_4147_5F52_4B59,
            material: 0x_5354_4147_5F4D_4154,
        },
    };

    /// The `small` ring the Tachyon panel runs: 160 x 48 x 24 at 0.125 m, 20 m around
    /// and only 6 m of headroom. A compact selection of landforms at half the relief,
    /// five resolved octaves from 10 m down to 0.625 m — not the wide landscape squeezed
    /// into a small ring.
    pub const SMALL: Recipe = Recipe {
        base_m: 1.5,
        back_rise_m: 1.5,
        basin_floor_m: 0.7,
        relief_wavelength_m: 10.0,
        relief_m: 1.3,
        relief_octaves: 5,
        ridge_wavelength_m: 5.0,
        ridge_relief_m: 0.7,
        rocky_wavelength_m: 13.0,
        warp_wavelength_m: 11.0,
        warp_m: 0.9,
        soil_flat_m: 0.5,
        soil_deposit_m: 0.35,
        soil_floor_m: 0.05,
        soil_max_m: 0.9,
        rock_m: 0.9,
        rock_swing_m: 0.4,
        strata_m: 0.3,
        strata_warp_m: 0.25,
        ..Recipe::DEFAULT
    };

    /// The `wide` ring: 256 x 48 x 24 at 0.25 m, 64 m around. The same landform sizes as
    /// `default` in a ring twice as long, so it holds twice the geography rather than
    /// the same hill stretched.
    pub const WIDE: Recipe = Recipe {
        relief_m: 2.7,
        rocky_fraction: 0.4,
        ..Recipe::DEFAULT
    };

    /// Refuse a recipe no landscape can be built from. [`Config::validate`] calls this
    /// for a staged config; nothing past it divides by a wavelength or a spacing again.
    pub fn validate(&self) -> anyhow::Result<()> {
        for (name, v) in [
            ("relief_wavelength_m", self.relief_wavelength_m),
            ("ridge_wavelength_m", self.ridge_wavelength_m),
            ("rocky_wavelength_m", self.rocky_wavelength_m),
            ("warp_wavelength_m", self.warp_wavelength_m),
            ("strata_m", self.strata_m),
            ("depth_scale", self.depth_scale),
            ("min_feature_voxels", self.min_feature_voxels),
            ("relief_m", self.relief_m),
            ("persistence", self.persistence),
        ] {
            anyhow::ensure!(
                v.is_finite() && v > 0.0,
                "{name} must be positive and finite, not {v}"
            );
        }
        for (name, v) in [
            ("base_m", self.base_m),
            ("back_rise_m", self.back_rise_m),
            ("basin_floor_m", self.basin_floor_m),
            ("ridge_relief_m", self.ridge_relief_m),
            ("rocky_fraction", self.rocky_fraction),
            ("warp_m", self.warp_m),
            ("soil_flat_m", self.soil_flat_m),
            ("soil_deposit_m", self.soil_deposit_m),
            ("soil_floor_m", self.soil_floor_m),
            ("soil_max_m", self.soil_max_m),
            ("rock_m", self.rock_m),
            ("rock_swing_m", self.rock_swing_m),
            ("strata_warp_m", self.strata_warp_m),
        ] {
            anyhow::ensure!(
                v.is_finite() && v >= 0.0,
                "{name} must be finite and not negative, not {v}"
            );
        }
        anyhow::ensure!(
            self.lacunarity.is_finite() && self.lacunarity > 1.0,
            "lacunarity steps the frequency up, so it must exceed 1, not {}",
            self.lacunarity
        );
        Ok(())
    }

    /// The relief ladder.
    pub fn relief_ladder(&self) -> Ladder {
        Ladder {
            wavelength_m: self.relief_wavelength_m,
            octaves: self.relief_octaves,
            persistence: self.persistence,
            lacunarity: self.lacunarity,
        }
    }

    /// The ridged ladder.
    pub fn ridge_ladder(&self) -> Ladder {
        Ladder {
            wavelength_m: self.ridge_wavelength_m,
            octaves: self.ridge_octaves,
            persistence: self.persistence,
            lacunarity: self.lacunarity,
        }
    }

    /// The finest wavelength this voxel size can carry, metres.
    pub fn min_wavelength_m(&self, voxel_m: f64) -> f64 {
        self.min_feature_voxels * voxel_m
    }

    /// Relief octaves that survive the resolution limit at this voxel size. The number
    /// the recipe *asks* for is [`Recipe::relief_octaves`]; this is what it gets.
    pub fn resolved_octaves(&self, voxel_m: f64) -> u32 {
        resolved_octaves(&self.relief_ladder(), self.min_wavelength_m(voxel_m))
    }

    /// Relief above [`Recipe::base_m`] at one point of the ring, metres. Exactly periodic
    /// in `x_m` at `circumference_m`, including the domain warp: two coordinates one lap
    /// apart return the same float, not two nearly-equal ones. Knows nothing about the
    /// voxel grid except the resolution limit.
    pub fn relief_m_at(
        &self,
        x_m: f64,
        z_m: f64,
        circumference_m: f64,
        voxel_m: f64,
        seed: u64,
    ) -> f64 {
        let min_wl = self.min_wavelength_m(voxel_m);
        // Reduce to the ring's canonical representative *before* the warp: a warp offset
        // added to `x` and to `x + circumference` rounds differently at the two
        // magnitudes, and one ulp of difference is a seam. Reduced first, two coordinates
        // one lap apart are the same float and everything after them is identical.
        let x_m = if circumference_m > 0.0 {
            x_m.rem_euclid(circumference_m)
        } else {
            x_m
        };
        let zs = z_m * self.depth_scale;
        let s = self.streams;

        let broad = self.relief_m
            * fbm(
                x_m,
                zs,
                circumference_m,
                &self.relief_ladder(),
                min_wl,
                seed ^ s.relief,
            );

        // The rocky regions: a slow field, thresholded softly so ridges fade in.
        let rocky_cells = ring_cells(circumference_m, self.rocky_wavelength_m);
        let mask_at = ring_noise(x_m, zs, circumference_m, rocky_cells, seed ^ s.rocky);
        let lo = 1.0 - 2.0 * self.rocky_fraction.clamp(0.0, 1.0);
        let mask = smoothstep(lo, lo + 0.5, mask_at);
        if mask <= 0.0 {
            return broad;
        }

        // A modest periodic domain warp, so the ridges are not parallel and obviously
        // sinusoidal. Both offsets are periodic fields, so the warped field still is.
        let warp_cells = ring_cells(circumference_m, self.warp_wavelength_m);
        let wx = self.warp_m * ring_noise(x_m, zs, circumference_m, warp_cells, seed ^ s.warp);
        let wz = self.warp_m
            * ring_noise(
                x_m + 137.0,
                zs + 61.0,
                circumference_m,
                warp_cells,
                seed ^ s.warp.rotate_left(17),
            );
        let crest = ridged(
            x_m + wx,
            zs + wz,
            circumference_m,
            &self.ridge_ladder(),
            min_wl,
            seed ^ s.ridge,
        );
        broad + mask * self.ridge_relief_m * crest
    }
}

/// A shipped landscape: the recipe and the ring it was written for.
///
/// The extents belong to [`Config`], not to the recipe — the same recipe on a wider ring
/// is more geography, which is the point — but a preset that names no ring is not a
/// landscape anyone can look at, so each one carries the one it was tuned on.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    /// `small`, `default` or `wide`.
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub voxel_m: f64,
    pub recipe: Recipe,
}

/// Every shipped preset, in size order.
pub const PRESETS: [Preset; 3] = [
    Preset {
        name: "small",
        width: 160,
        height: 48,
        depth: 24,
        voxel_m: 0.125,
        recipe: Recipe::SMALL,
    },
    Preset {
        name: "default",
        width: 128,
        height: 48,
        depth: 24,
        voxel_m: 0.25,
        recipe: Recipe::DEFAULT,
    },
    Preset {
        name: "wide",
        width: 256,
        height: 48,
        depth: 24,
        voxel_m: 0.25,
        recipe: Recipe::WIDE,
    },
];

impl Preset {
    /// A shipped preset by name.
    pub fn find(name: &str) -> Option<&'static Preset> {
        PRESETS.iter().find(|p| p.name == name)
    }

    /// The ring this preset was written for, on its own recipe.
    pub fn config(&self) -> Config {
        Config {
            width: self.width,
            height: self.height,
            depth: self.depth,
            voxel_m: self.voxel_m,
            landform: Landform::Staged(self.recipe),
            ..Config::default()
        }
    }

    /// Circumference in metres.
    pub fn circumference_m(&self) -> f64 {
        self.width as f64 * self.voxel_m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The presets differ in resolved octaves and in how many landforms fit, never by
    /// squeezing one landscape into another ring.
    #[test]
    fn the_presets_resolve_the_octaves_their_rings_can_carry() {
        let small = Preset::find("small").unwrap();
        let default = Preset::find("default").unwrap();
        let wide = Preset::find("wide").unwrap();

        assert_eq!(default.recipe.resolved_octaves(default.voxel_m), 6);
        assert_eq!(small.recipe.resolved_octaves(small.voxel_m), 5);
        assert_eq!(wide.recipe.resolved_octaves(wide.voxel_m), 6);

        // Landforms per ring: the coarse octave's lattice.
        let count = |p: &Preset| ring_cells(p.circumference_m(), p.recipe.relief_wavelength_m);
        assert_eq!(count(small), 2);
        assert_eq!(count(default), 2);
        assert_eq!(count(wide), 4, "the wide ring holds twice the geography");
    }

    /// The vertical budget: nothing a preset can build reaches the clamps, or the
    /// landscape would be a plateau and the metre-for-metre invariance would be a lie.
    #[test]
    fn every_preset_fits_between_the_floor_and_the_sky() {
        for p in PRESETS {
            let r = p.recipe;
            let head_m = (p.height as f64 - 5.0) * p.voxel_m;
            let floor_m = 3.0 * p.voxel_m;
            let high = r.base_m + r.relief_m + r.ridge_relief_m + r.back_rise_m;
            let deep = r.base_m - r.relief_m - r.ridge_relief_m;
            let low = r.basin_floor_m - (r.basin_floor_m - deep).max(0.0) * 0.12;
            assert!(
                high < head_m,
                "{}: {high:.2} m reaches the {head_m:.2} m ceiling",
                p.name
            );
            assert!(
                low > floor_m,
                "{}: {low:.2} m reaches the {floor_m:.2} m floor",
                p.name
            );
        }
    }

    /// A staged config survives a snapshot round trip: postcard is not self-describing,
    /// so the enum has to be externally tagged, not internally tagged.
    #[test]
    fn a_staged_config_round_trips_through_postcard() {
        for p in PRESETS {
            let c = p.config();
            let bytes = postcard::to_stdvec(&c).unwrap();
            let back: Config = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(c, back, "{}", p.name);
        }
    }
}
