//! A small, controlled model/stamp comparison, shared with the mechanism tests.
use anyhow::Result;
use cubarium::voxel::{Lighting, VoxelConfig, appearance, project::Projection};
use cubarium_gpu::{
    figures::{
        FigureFrame, FigureInstance, FigureTile, FigureVoxel, PaletteEntry, crease_masks,
        open_faces,
    },
    target::Headless,
    Gpu,
    voxel::{VoxelRenderer, VoxelStyle, VoxelTexel, VoxelTextures},
};

pub struct Cell {
    pub offset: [i8; 3],
    pub colours: [[f32; 3]; 3],
}

pub struct Picture {
    pub pixels: Vec<u8>,
    pub projection: Projection,
}

/// `glyph == 0` selects baked model cells (the stamp's interim glyph).
/// All light fields come from this supplied config; no tuned colours are asserted.
pub fn draw(
    cells: &[Cell],
    glyph: u8,
    quarter: u8,
    stamped: bool,
    cfg: &VoxelConfig,
) -> Result<Picture> {
    draw_angle(cells, glyph, quarter, 0.0, stamped, cfg)
}

/// Add a model-only angle to a quarter heading; stamps remain on their voxel grid.
pub fn draw_angle(
    cells: &[Cell],
    glyph: u8,
    quarter: u8,
    angle_offset: f64,
    stamped: bool,
    cfg: &VoxelConfig,
) -> Result<Picture> {
    let proj = Projection::new(
        cfg.tilt_degrees,
        cfg.px_per_voxel,
        cfg.raster_height,
        &cfg.sim.world,
    )?;
    let mut params = cubarium::sink::gpu::voxel::params_of(cfg, proj, true);
    params.sky = [0.0; 3];
    params.sky_horizon = [0.0; 3];
    params.day_sky_brightness = 0.0;
    params.effects.shadows = false;
    // One headless device per capture/test process keeps repeated draws cheap and
    // avoids repeated driver teardown. Serialize access to its queue.
    static GPU: std::sync::OnceLock<Result<std::sync::Mutex<Gpu>, String>> = std::sync::OnceLock::new();
    let gpu = GPU.get_or_init(|| Gpu::open().map(std::sync::Mutex::new).map_err(|e| e.to_string()))
        .as_ref().map_err(|e| anyhow::anyhow!("{e}"))?
        .lock().map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut renderer =
        VoxelRenderer::new(&gpu, params, &VoxelTextures::empty(params.s, params.rise))?;
    let mut target = Headless::new(&gpu, &renderer)?;
    let anchor = [
        cfg.sim.world.width as i32 / 2,
        4,
        cfg.sim.world.depth as i32 / 2,
    ];
    let offsets: Vec<_> = cells.iter().map(|c| c.offset).collect();
    let mut paints = Vec::new();
    let entries: Vec<_> = cells
        .iter()
        .map(|c| match paints.iter().position(|p| *p == c.colours) {
            Some(i) => i,
            None => {
                paints.push(c.colours);
                paints.len() - 1
            }
        })
        .collect();
    anyhow::ensure!(paints.len() * 3 <= 255, "fixture palette too large");
    let stamp_glyph = if glyph == 0 { 1 } else { glyph };
    renderer.stage([1; 6], |s| {
        s.voxels.fill(VoxelTexel::default());
        s.carried.full = true;
        s.roof.fill(0);
        s.sky.fill(255);
        s.canopy.fill(0);
        s.glow.fill(0);
        appearance::write_atlas(params.s, params.rise, s.glyphs);
        for (i, c) in cells.iter().enumerate() {
            s.styles[entries[i]] = VoxelStyle::new(c.colours[0], c.colours[1], c.colours[2]);
            if stamped {
                let (dx, dz) =
                    cubarium::voxel::model::turn(quarter, c.offset[0] as i32, c.offset[2] as i32);
                let (x, y, z) = (
                    anchor[0] + dx,
                    anchor[1] + c.offset[1] as i32,
                    anchor[2] + dz,
                );
                let at = ((z as u32 * params.height + y as u32) * params.width + x as u32) as usize;
                s.voxels[at] =
                    VoxelTexel::pack_glyph(0, 5, stamp_glyph, 0.0, true, 0.0, entries[i] as u8);
            }
        }
    });
    if !stamped {
        let open = open_faces(&offsets);
        let creases = crease_masks(&offsets);
        let voxels: Vec<_> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                FigureVoxel::new(c.offset, (entries[i] * 3) as u8, 0, false, open[i], glyph)
                    .with_creases(creases[i])
            })
            .collect();
        renderer.put_figure_models(0, &voxels);
        let angle = std::f64::consts::FRAC_PI_2 * (1.0 + f64::from(quarter)) + angle_offset;
        let axes = cubarium::voxel::figure::Orient::Ground(angle)
            .axes()
            .map(|v| v.map(|x| x as f32));
        let radius = offsets
            .iter()
            .map(|c| c.iter().map(|&x| f32::from(x).abs()).sum::<f32>())
            .fold(1.0, f32::max)
            + 2.0;
        let tile = FigureTile {
            first: 0,
            count: cells.len() as u32,
            pos: [
                anchor[0] as f32 + 0.5,
                anchor[1] as f32,
                anchor[2] as f32 + 0.5,
            ],
            axes,
            centre: [0.0; 3],
            radius,
            pose: [0.0; 4],
            palette: 0,
            bones: None,
        };
        renderer.set_figures(FigureFrame {
            figures: vec![FigureInstance {
                id: 1,
                a: tile,
                b: None,
                weight_b: 0.0,
                alpha: 1.0,
                highlight: 0.0,
            }],
            palette: paints
                .iter()
                .flat_map(|c| c.map(|base| PaletteEntry { base, emit: None }))
                .collect(),
            whole_pixels: true,
            supersample: cfg.figures.supersample,
            ..FigureFrame::default()
        });
    }
    target.draw(&gpu, &mut renderer, ())?;
    let pixels = renderer.read_raster(&gpu)?;
    target.destroy(&gpu);
    renderer.destroy(&gpu);
    Ok(Picture {
        pixels,
        projection: proj,
    })
}

pub fn config(ao: f32) -> VoxelConfig {
    let mut cfg = VoxelConfig::default();
    cfg.sim.world.width = 32;
    cfg.sim.world.height = 16;
    cfg.sim.world.depth = 16;
    cfg.px_per_voxel = 8;
    cfg.tilt_degrees = 30.0;
    cfg.lighting = Lighting::Lit;
    cfg.haze = 0.0;
    cfg.dither = 0.0;
    cfg.light.ao = ao;
    cfg.light.sun = [0.0; 3];
    cfg.light.sun_tint = 0.0;
    cfg.light.levels = 0;
    cfg.light.bloom = 0.0;
    cfg
}
