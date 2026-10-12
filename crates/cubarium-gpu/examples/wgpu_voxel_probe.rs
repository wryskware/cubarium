//! Headless validation of the complete safe voxel pass chain.
#![forbid(unsafe_code)]
use anyhow::{Result, ensure};
use cubarium_gpu::{
    figures::{CLASS_NONE, FigureFrame, FigureInstance, FigureTile, FigureVoxel, PaletteEntry},
    plants::{PlantRec, SETTLED, pack_cell},
    voxel::{
        PART_CROWN, SLOW_PLANES, VoxelParams, VoxelStyle, VoxelTexel, VoxelTextures, write_upload,
    },
    wgpu_backend::{FrameSource, Gpu, headless::Headless, voxel::VoxelRenderer},
};
fn main() -> Result<()> {
    let gpu = Gpu::open()?;
    for (lit, plants) in [(false, false), (true, false), (true, true)] {
        let mut p = params();
        p.width = 8;
        p.height = 4;
        p.depth = 4;
        p.raster_w = 32;
        p.raster_h = 32;
        p.base = 24;
        p.lit = lit;
        p.sky = [0.05, 0.1, 0.2];
        p.soil = [0.2, 0.08, 0.04];
        p.bloom = if lit { 0.2 } else { 0.0 };
        p.plants = plants.then(Default::default);
        let mut r = VoxelRenderer::new(&gpu, p, &VoxelTextures::empty(p.s, p.rise))?;
        ensure!(
            r.put_figure_models(0, &[FigureVoxel::new([0; 3], 0, CLASS_NONE, false, 63, 0)]),
            "model rejected"
        );
        r.set_figures(FigureFrame {
            figures: vec![FigureInstance {
                id: 1,
                a: FigureTile {
                    first: 0,
                    count: 1,
                    pos: [4.0, 2.0, 1.0],
                    axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    centre: [0.0; 3],
                    radius: 1.0,
                    pose: [0.0; 4],
                    palette: 0,
                    bones: None,
                },
                b: None,
                weight_b: 0.0,
                alpha: 1.0,
                highlight: 0.0,
            }],
            palette: vec![PaletteEntry {
                base: [0.8, 0.1, 0.05],
                emit: Some([0.1, 0.02, 0.0]),
            }],
            ..Default::default()
        });
        let mut master = vec![VoxelTexel::default(); p.voxel_count()];
        for z in 0..p.depth {
            for x in 0..p.width {
                master[(z * p.height * p.width + x) as usize] = VoxelTexel([3, 0, 0, 0]);
            }
        }
        if plants {
            master[((2 * p.height + 1) * p.width + 2) as usize] =
                VoxelTexel::pack(0, PART_CROWN, 0.0, true, 0.0, 1);
        }
        ensure!(
            r.stage([1; SLOW_PLANES], |s| {
                write_upload(
                    &master, p.width, p.height, p.depth, s.owed, s.voxels, s.carried,
                );
                s.roof.fill(0);
                s.glyphs.fill(0);
                s.sky.fill(255);
                s.canopy.fill(0);
                s.glow.fill(0);
                s.styles[1] =
                    VoxelStyle::new([0.1, 0.05, 0.02], [0.05, 0.5, 0.1], [0.1, 0.2, 0.05]);
                if let Some(list) = s.plants {
                    list.plants = vec![PlantRec::default()];
                    list.cells = vec![pack_cell([2, 1, 2], 0, 0.0, false, SETTLED, 0)];
                }
            }),
            "stage refused"
        );
        if lit {
            r.set_weather(cubarium_gpu::weather::Weather {
                fog: 0.25,
                ..cubarium_gpu::weather::Weather::CLEAR_NOON
            });
            r.set_clock(cubarium_gpu::voxel::FrameClock::default());
        }
        let mut dropped = gpu.encoder("discarded voxel frame");
        r.record_frame(&gpu, &mut dropped, (), None)?;
        drop(dropped);
        r.frame_discarded();
        let mut target = Headless::new(&gpu, &r)?;
        target.draw(&gpu, &mut r, ())?;
        let pixels = target.read(&gpu, &r)?;
        ensure!(
            pixels.len() == (p.raster_w * p.raster_h * 4) as usize,
            "wrong readback size"
        );
        ensure!(
            pixels.chunks_exact(4).any(|p| p[..3] != [0, 0, 0]),
            "black fixture"
        );
        target.draw(&gpu, &mut r, ())?;
        ensure!(!r.redrew_last(), "unchanged voxel frame redrew");
        ensure!(
            target.read(&gpu, &r)? == pixels,
            "unchanged frame changed pixels"
        );
        println!(
            "voxel pass chain lit={lit} plants={plants}: PASS {} bytes",
            pixels.len()
        );
    }
    Ok(())
}
fn params() -> VoxelParams {
    VoxelParams {
        s: 4,
        rise: 2,
        base: 240,
        width: 128,
        height: 48,
        depth: 24,
        raster_w: 512,
        raster_h: 240,
        haze: 0.55,
        water_alpha: 0.5,
        roof_from_texture: true,
        sky: [0.0; 3],
        sky_horizon: [0.0; 3],
        atmosphere: 0.0,
        rain_tick: 0.0,
        dither: 0.0,
        sky_gradient: false,
        bedrock: [0.0; 3],
        rock: [0.0; 3],
        soil: [0.0; 3],
        water_deep: [0.0; 3],
        water_surface: [0.0; 3],
        light: [0.0; 3],
        haze_colour: [0.0; 3],
        top_gain: 2.4,
        top_tint: 0.2,
        top_back: 0.3,
        rim: 0.38,
        edge_dark: 0.72,
        top_edge: 0.18,
        riser_lean: 0.45,
        wet: 0.55,
        roof_light: 0.22,
        roof_falloff: 4.0,
        skin_alpha_gain: 1.7,
        water_top_alpha: 0.8,
        plant_top_gain: 1.5,
        plant_top_tint: 0.14,
        plant_rim: 0.42,
        crown_edge: 0.7,
        crown_under: 0.22,
        trunk_shade: [0.62, 1.22],
        trunk_light_at: 0.35,
        lit: false,
        ambient_gain: 1.4,
        ambient_floor: 0.2,
        light_levels: 4,
        ao_strength: 0.5,
        ambient_colour: [1.0; 3],
        sun: [0.0; 3],
        sun_tint: 0.0,
        water_absorb: 0.1,
        reflect_gain: 6.0,
        ripple: 0.2,
        reflect_cells: 64,
        foam: 0.0,
        glint: 0.0,
        rain_rings: 0.0,
        highlight: 0.0,
        bloom: 0.0,
        bloom_radius: 2.0,
        bloom_style: cubarium_gpu::bloom::BloomStyle::Smooth,
        debug_flow: false,
        effects: cubarium_gpu::sunvis::Effects::default(),
        day_sky: cubarium_gpu::weather::DaySky::Warm,
        star_bloom: 0.0,
        day_sky_brightness: 1.0,
        fog_motion: cubarium_gpu::weather::FogMotion::default(),
        plants: None,
        drape_cords: cubarium_gpu::cord::DrapeCords::OFF,
    }
}
