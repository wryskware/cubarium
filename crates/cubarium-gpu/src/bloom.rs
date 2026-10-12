//! Bloom styles and unchanged CPU pass sizing.
use crate::voxel::VoxelParams;
pub use crate::wgpu_backend::bloom::Bloom;
/// What the smooth style multiplies `bloom` by: the blur spreads a small emitter's light
/// over many pixels, so at the same `bloom` the smooth halo reads about as strong as the
/// blocky one (whose cells take the emitter's full colour).
pub const SMOOTH_GAIN: f32 = 20.0;

/// How the halo is made (`[light] bloom_style`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BloomStyle {
    /// Blurred down a mip chain and added bilinear: a soft glow.
    #[default]
    Smooth,
    /// L5's: whole voxel cells at three strengths, added nearest.
    Blocky,
}

// Blocky passes.
// Smooth passes.

/// The voxel cell grid over a raster: how many cells across and down, and how many rows
/// the grid is shifted by so a cell's rows are one voxel band's (`base` is a band edge).
pub fn cell_grid(params: &VoxelParams) -> (u32, u32, u32) {
    let s = params.s.max(1);
    let off = (-params.base).rem_euclid(s as i32) as u32;
    (
        params.raster_w.div_ceil(s),
        (params.raster_h + off).div_ceil(s),
        off,
    )
}

/// The smooth bloom's Gaussian sigma in raster pixels: half of `bloom_radius` voxel cells.
pub fn smooth_sigma_px(params: &VoxelParams) -> f32 {
    params.bloom_radius.max(0.0) * params.s.max(1) as f32 * 0.5
}

/// How many times the smooth bloom halves the emission before it blurs: as many as keep
/// the Gaussian at least two texels wide there, one at least, six at most.
pub fn smooth_levels(params: &VoxelParams) -> u32 {
    let sigma = smooth_sigma_px(params);
    if sigma < 4.0 {
        return 1;
    }
    ((sigma / 2.0).log2().floor() as u32).clamp(1, 6)
}

/// Each mip level's size: half the one above, rounded up.
pub fn level_sizes(params: &VoxelParams, levels: u32) -> Vec<(u32, u32)> {
    let mut size = (params.raster_w, params.raster_h);
    (0..levels)
        .map(|_| {
            size = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
            size
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cell_grid_covers_the_raster_with_rows_on_the_voxel_bands() {
        let mut p = crate::voxel::tests::params();
        p.s = 6;
        p.raster_w = 600;
        p.raster_h = 100;
        p.base = 97;
        let (w, h, off) = cell_grid(&p);
        assert_eq!(w, 100);
        // Row `base - s` (the top of the y = 0, z = 0 front face) starts a cell.
        assert_eq!((91 + off) % 6, 0);
        assert!(h * 6 >= 100 + off && (h - 1) * 6 < 100 + off);
    }

    #[test]
    fn the_smooth_chain_keeps_the_blur_a_few_texels_wide_at_every_scale() {
        let mut p = crate::voxel::tests::params();
        p.bloom_radius = 2.0;
        for s in [4, 6, 9, 13, 20] {
            p.s = s;
            p.raster_w = 256 * s;
            p.raster_h = 1024;
            let n = smooth_levels(&p);
            let sizes = level_sizes(&p, n);
            assert_eq!(sizes.len() as u32, n);
            assert_eq!(sizes[0], (128 * s, 512));
            let sigma = smooth_sigma_px(&p) / (1u32 << n) as f32;
            assert!(
                (1.5..=4.0).contains(&sigma),
                "{s} px: sigma {sigma} texels at level {n}"
            );
        }
    }
}
