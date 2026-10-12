//! Drape cords on the device (DRAPE-CORD, [`crate::cord`]): the walk draws a drape cell's
//! cord where the CPU mirror says, and off the cord it reaches what is behind the cell.
//! Skips with a printed reason when there is no GPU device.

use crate::Gpu;
use crate::cord::{CordShape, CordWave, DrapeCords, WaveRatios};
use crate::target::Headless;
use crate::voxel::{
    PART_CROWN, ROLE_DRAPE, VoxelParams, VoxelRenderer, VoxelStaging, VoxelStyle, VoxelTexel,
    VoxelTextures, write_upload,
};

/// The drape column: cells (X, Y0..Y1, Z) in front of a rock wall at Z + 1.
const X: u32 = 5;
const Z: u32 = 1;
const Y0: u32 = 8;
const Y1: u32 = 13;

/// One headless frame of the wall, with the drape column if `drape`; `None` without a device.
fn draw(cords: DrapeCords, drape: bool) -> Option<(Vec<u8>, VoxelParams)> {
    let gpu = match Gpu::open() {
        Ok(gpu) => gpu,
        Err(e) => {
            println!("skipped: no GPU device ({e:#})");
            return None;
        }
    };
    let params = VoxelParams {
        rock: [0.0, 0.0, 0.6],
        drape_cords: cords,
        ..crate::voxel::tests::params()
    };
    let (w, h, d) = (params.width, params.height, params.depth);
    let mut master = vec![VoxelTexel::default(); params.voxel_count()];
    for y in 0..20 {
        for x in 0..20 {
            master[VoxelStaging::index(w, h, x, y, Z + 1)] =
                VoxelTexel::pack(2, 0, 0.0, true, 0.0, 0);
        }
    }
    if drape {
        for y in Y0..Y1 {
            master[VoxelStaging::index(w, h, X, y, Z)] =
                VoxelTexel::pack(0, PART_CROWN, 0.0, true, 0.0, 1);
        }
    }
    let mut r = VoxelRenderer::new(
        &gpu,
        params.clone(),
        &VoxelTextures::empty(params.s, params.rise),
    )
    .unwrap();
    assert!(r.stage([1; crate::voxel::SLOW_PLANES], |st| {
        write_upload(&master, w, h, d, st.owed, st.voxels, st.carried);
        st.roof.fill(0);
        st.glyphs.fill(0);
        st.sky.fill(255);
        st.canopy.fill(0);
        st.glow.fill(0);
        let green = [0.0, 0.9, 0.0];
        st.styles[1] = VoxelStyle::new(green, green, green).with_role(ROLE_DRAPE);
    }));
    let mut target = Headless::new(&gpu, &r).unwrap();
    target.draw(&gpu, &mut r, ()).unwrap();
    let rgba = r.read_raster(&gpu).unwrap();
    target.destroy(&gpu);
    r.destroy(&gpu);
    Some((rgba, params))
}

/// Every front-face pixel of the drape column: (cell y, dx, dy, raster pixel index).
fn face_pixels(p: &VoxelParams) -> Vec<(u32, u32, u32, usize)> {
    let mut out = Vec::new();
    for y in Y0..Y1 {
        let top = p.base - ((y + 1) * p.s) as i32 - (Z * p.rise) as i32;
        for dy in 0..p.s {
            for dx in 0..p.s {
                let (px, py) = (X * p.s + dx, (top + dy as i32) as u32);
                out.push((y, dx, dy, (py * p.raster_w + px) as usize * 4));
            }
        }
    }
    out
}

#[test]
fn off_the_cord_the_walk_reaches_the_wall_and_on_it_draws_the_cord_where_the_mirror_says() {
    for cords in [
        DrapeCords {
            width_px: 1,
            shape: CordShape::Straight,
            wander: 1.0,
            wave: CordWave::NONE,
        },
        DrapeCords {
            width_px: 2,
            shape: CordShape::Eased,
            wander: 1.0,
            wave: CordWave::NONE,
        },
        // The long wave, its swing past the column held inside it in the walk.
        DrapeCords {
            width_px: 1,
            shape: CordShape::Eased,
            wander: 1.0,
            wave: CordWave {
                period: 5.0,
                amplitude: 1.5,
                octaves: 1,
                ratios: WaveRatios::Octave,
            },
        },
        // With octaves (round 3), both ratios.
        DrapeCords {
            width_px: 2,
            shape: CordShape::Eased,
            wander: 0.5,
            wave: CordWave {
                period: 12.0,
                amplitude: 0.5,
                octaves: 3,
                ratios: WaveRatios::Octave,
            },
        },
        DrapeCords {
            width_px: 2,
            shape: CordShape::Eased,
            wander: 0.5,
            wave: CordWave {
                period: 24.0,
                amplitude: 0.5,
                octaves: 3,
                ratios: WaveRatios::Jittered,
            },
        },
    ] {
        let Some((with, p)) = draw(cords, true) else {
            return;
        };
        let Some((without, _)) = draw(cords, false) else {
            return;
        };
        let (mut cord, mut through) = (0, 0);
        for (y, dx, dy, i) in face_pixels(&p) {
            let px = &with[i..i + 4];
            if cords.texel(p.s, X as i32, y as i32, Z as i32, dx, dy) {
                assert!(
                    px[1] > 100 && px[2] < 60,
                    "{cords:?}: cord pixel ({y}, {dx}, {dy}) is {px:?}"
                );
                cord += 1;
            } else {
                assert_eq!(
                    px,
                    &without[i..i + 4],
                    "{cords:?}: ({y}, {dx}, {dy}) shows what is behind"
                );
                through += 1;
            }
        }
        assert!(cord > 0 && through > 0);
    }
}

#[test]
fn with_the_cords_off_a_drape_cell_draws_as_a_whole_cube() {
    let Some((with, p)) = draw(DrapeCords::OFF, true) else {
        return;
    };
    for (y, dx, dy, i) in face_pixels(&p) {
        let px = &with[i..i + 4];
        assert!(px[1] > 100 && px[2] < 60, "({y}, {dx}, {dy}) is {px:?}");
    }
}
