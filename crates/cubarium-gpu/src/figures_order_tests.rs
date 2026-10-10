//! The model pass's figures against each other, on the device: overlapping figures resolve
//! per pixel by depth (the walk's convention, larger z farther), not by the order they are
//! drawn in. Skips with a printed reason when there is no Vulkan device.

use crate::figures::{
    CLASS_NONE, FigureFrame, FigureInstance, FigureTile, FigureVoxel, PaletteEntry, open_faces,
};
use crate::target::Headless;
use crate::vk::Gpu;
use crate::voxel::{VoxelRenderer, VoxelTextures, VoxelTexel, write_upload};

const UPRIGHT: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// A 2×2×2 block of cells offset by `dz` along the model's z: its tile's anchor and its
/// cells' depth then disagree, which a per-figure sort cannot see.
fn block(dz: i8) -> Vec<FigureVoxel> {
    let mut cells = Vec::new();
    for x in 0..2i8 {
        for y in 0..2i8 {
            for z in 0..2i8 {
                cells.push([x, y, z + dz]);
            }
        }
    }
    let open = open_faces(&cells);
    cells.iter().zip(open).map(|(&c, o)| FigureVoxel::new(c, 0, CLASS_NONE, false, o, 0)).collect()
}

fn tile(first: u32, pos: [f32; 3], palette: u32, dz: f32) -> FigureTile {
    FigureTile {
        first,
        count: 8,
        pos,
        axes: UPRIGHT,
        centre: [0.5, 1.0, 0.5 + dz],
        radius: 2.5,
        pose: [0.0; 4],
        palette,
        bones: None,
    }
}

fn figure(id: u64, a: FigureTile) -> FigureInstance {
    FigureInstance { id, a, b: None, weight_b: 0.0, alpha: 1.0, highlight: 0.0 }
}

/// One headless frame of an empty world with `figures` drawn; `None` without a device.
fn draw(figures: Vec<FigureInstance>) -> Option<(Vec<u8>, u32)> {
    draw_at(figures, 0)
}

fn draw_at(mut figures: Vec<FigureInstance>, first: usize) -> Option<(Vec<u8>, u32)> {
    let gpu = match Gpu::open(&[]) {
        Ok(gpu) => gpu,
        Err(e) => {
            println!("skipped: no Vulkan device ({e:#})");
            return None;
        }
    };
    let params = crate::voxel::tests::params();
    let mut r = VoxelRenderer::new(&gpu, params.clone(), &VoxelTextures::empty(params.s, params.rise)).unwrap();
    let master = vec![VoxelTexel::default(); params.voxel_count()];
    assert!(r.stage([1; crate::voxel::SLOW_PLANES], |st| {
        write_upload(&master, params.width, params.height, params.depth, st.owed, st.voxels, st.carried);
        st.roof.fill(0);
        st.glyphs.fill(0);
        st.sky.fill(255);
        st.canopy.fill(0);
        st.glow.fill(0);
    }));
    let mut cells = block(0);
    cells.extend(block(-4));
    assert!(r.put_figure_models(first, &cells));
    for figure in &mut figures {
        figure.a.first += first as u32;
        if let Some(b) = &mut figure.b {
            b.first += first as u32;
        }
    }
    r.set_figures(FigureFrame {
        figures,
        palette: vec![
            PaletteEntry { base: [0.0, 0.8, 0.0], emit: None },
            PaletteEntry { base: [0.8, 0.0, 0.0], emit: None },
        ],
        whole_pixels: true,
        supersample: 1,
        ..FigureFrame::default()
    });
    let mut target = Headless::new(&gpu, &r).unwrap();
    target.draw(&gpu, &mut r, ()).unwrap();
    let rgba = r.read_raster(&gpu).unwrap();
    target.destroy(&gpu);
    r.destroy(&gpu);
    Some((rgba, params.raster_w))
}

#[test]
fn cells_at_the_end_of_the_model_buffer_render_like_cells_at_the_start() {
    let figure = figure(1, tile(0, [20.0, 10.0, 8.0], 0, 0.0));
    let Some((start, _)) = draw(vec![figure]) else {
        return;
    };
    let first = crate::figures::MAX_VOXELS - block(0).len() - block(-4).len();
    let Some((end, _)) = draw_at(vec![figure], first) else {
        return;
    };
    assert!(
        start.chunks_exact(4).any(|px| px[1] > px[0]),
        "the uploaded cells visibly render"
    );
    assert_eq!(start, end, "model buffer capacity includes its final cells");
}

#[test]
fn overlapping_figures_resolve_per_pixel_by_depth_whichever_is_drawn_first() {
    // Far (green): anchored at z 8, its cells at z 8..10, a voxel to the right. Near (red): anchored farther, at
    // z 10, but its cells are offset to z 6..8, nearer than every green cell. One voxel
    // higher, it lands on the same screen rows (rise 2, s 4: a voxel up is two of depth).
    // Anchors put the near figure first back to front, so only a per-pixel test shows it.
    let far = figure(1, tile(0, [21.0, 10.0, 8.0], 0, 0.0));
    let near = figure(2, tile(8, [20.0, 11.0, 10.0], 1, -4.0));
    let Some((ab, w)) = draw(vec![far, near]) else { return };
    let Some((ba, _)) = draw(vec![near, far]) else { return };
    assert_eq!(ab, ba, "the order the figures come in changes nothing");
    let (mut red, mut green) = (0, 0);
    for px in ab.chunks_exact(4) {
        if px[0] > 40 && px[1] < 20 {
            red += 1;
        } else if px[1] > 40 && px[0] < 20 {
            green += 1;
        }
    }
    // Both draw (the far block shows round the near one), and the near block is whole:
    // a 2×2×2 block at s 4, rise 2 is 8 px wide and 8 + 4 px tall.
    assert!(green > 0, "the far figure shows where the near one does not cover it");
    assert!(red >= 8 * 12, "the near figure is drawn whole over the far one: {red} px");
    let _ = w;
}
