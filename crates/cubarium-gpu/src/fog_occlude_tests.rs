//! Actual headless raster checks: fog belongs to each visible layer's air path, including
//! fractional figure coverage. The inputs deliberately remove lighting/shadow changes.
use super::*;
use crate::figures::{
    CLASS_NONE, FigureFrame, FigureInstance, FigureTile, FigureVoxel, PaletteEntry,
};
use crate::plants::{PlantMotion, PlantRec, SETTLED, pack_cell};
use crate::target::Headless;

fn params(plants: bool) -> VoxelParams {
    VoxelParams {
        width: 32,
        height: 24,
        depth: 16,
        base: 128,
        raster_w: 128,
        raster_h: 128,
        lit: true,
        haze: 0.0,
        rock: [0.08, 0.12, 0.3],
        ambient_gain: 1.0,
        light_levels: 0,
        ao_strength: 0.0,
        effects: crate::sunvis::Effects {
            shadows: false,
            reflections: false,
            glow: false,
            ao: false,
            ..Default::default()
        },
        plants: plants.then_some(PlantMotion {
            supersample: 4,
            grow_s: 1.0,
            ..Default::default()
        }),
        ..super::tests::params()
    }
}

fn stage(r: &mut VoxelRenderer, plant: bool) {
    let p = r.params.clone();
    let mut cells = vec![VoxelTexel::default(); p.voxel_count()];
    // A fogged bank behind the foreground block, across every test pixel's ray.
    for x in 0..p.width {
        for y in 0..16 {
            cells[VoxelStaging::index(p.width, p.height, x, y, 12)] =
                VoxelTexel::pack(2, 0, 0.0, true, 0.0, 0);
        }
    }
    if plant {
        cells[VoxelStaging::index(p.width, p.height, 10, 6, 0)] =
            VoxelTexel::pack(0, PART_TRUNK, 0.0, true, 0.0, 1);
    }
    assert!(r.stage([1; SLOW_PLANES], |st| {
        write_upload(
            &cells, p.width, p.height, p.depth, st.owed, st.voxels, st.carried,
        );
        st.roof.fill(0);
        st.glyphs.fill(0);
        st.sky.fill(255);
        st.canopy.fill(0);
        st.glow.fill(0);
        st.styles[1] = VoxelStyle::new([0.7, 0.1, 0.03], [0.7, 0.1, 0.03], [0.7, 0.1, 0.03]);
        if let Some(list) = st.plants {
            list.cells = if plant {
                vec![pack_cell([10, 6, 0], 0, 0.0, false, SETTLED, 0)]
            } else {
                vec![]
            };
            list.plants = vec![PlantRec::default()];
        }
    }));
}

fn draw(gpu: &Gpu, r: &mut VoxelRenderer, target: &mut Headless, fog: bool) -> Vec<u8> {
    r.look = crate::weather::noon(&r.params);
    r.look.sun = [0.0; 3];
    r.look.sun_strength = 0.0;
    r.look.fog = if fog { 1.0 } else { 0.0 };
    r.look.fog_colour = [0.4, 0.25, 0.65];
    r.look.fog_glow = 0.0;
    r.set_fog_floor(10.0);
    r.raster_current = false;
    target.draw(gpu, r, ()).unwrap();
    r.read_raster(gpu).unwrap()
}

fn figure(alpha: f32) -> FigureFrame {
    FigureFrame {
        figures: vec![FigureInstance {
            id: 1,
            a: FigureTile {
                first: 0,
                count: 1,
                pos: [10.125, 6.0, -1.0],
                axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                centre: [0.5; 3],
                radius: 1.0,
                pose: [0.0; 4],
                palette: 0,
                bones: None,
            },
            b: None,
            weight_b: 0.0,
            alpha,
            highlight: 0.0,
        }],
        palette: vec![PaletteEntry {
            base: [0.7, 0.1, 0.03],
            emit: None,
        }],
        supersample: 4,
        ..Default::default()
    }
}

#[test]
fn a_foreground_block_and_its_soft_rim_keep_only_the_air_in_front() {
    let Ok(gpu) = Gpu::open(&[]) else {
        eprintln!("skipped: no Vulkan device");
        return;
    };
    let p = params(false);
    let mut r = VoxelRenderer::new(&gpu, p.clone(), &VoxelTextures::empty(p.s, p.rise)).unwrap();
    stage(&mut r, false);
    assert!(r.put_figure_models(0, &[FigureVoxel::new([0; 3], 0, CLASS_NONE, false, 63, 0)]));
    let mut target = Headless::new(&gpu, &r).unwrap();
    let clear_bg = draw(&gpu, &mut r, &mut target, false);
    let fog_bg = draw(&gpu, &mut r, &mut target, true);
    r.set_figures(figure(1.0));
    let clear_block = draw(&gpu, &mut r, &mut target, false);
    let fog_block = draw(&gpu, &mut r, &mut target, true);
    let centre = ((p.base - 6 * p.s as i32 - p.s as i32 / 2 + p.rise as i32) as u32 * p.raster_w
        + 10 * p.s) as usize
        * 4;
    assert_ne!(
        &clear_bg[centre..centre + 3],
        &fog_bg[centre..centre + 3],
        "background has fog"
    );
    assert_ne!(
        &clear_block[centre..centre + 3],
        &clear_bg[centre..centre + 3],
        "block covers the bank"
    );
    assert_eq!(
        &clear_block[centre..centre + 3],
        &fog_block[centre..centre + 3],
        "no fog behind an opaque block at the front"
    );
    for (i, (a, b)) in clear_block
        .chunks_exact(4)
        .zip(clear_bg.chunks_exact(4))
        .enumerate()
    {
        if a == b {
            assert_eq!(
                &fog_block[i * 4..i * 4 + 4],
                &fog_bg[i * 4..i * 4 + 4],
                "surrounding bank stays unchanged"
            );
        }
    }
    r.set_figures(figure(0.5));
    let clear_half = draw(&gpu, &mut r, &mut target, false);
    let fog_half = draw(&gpu, &mut r, &mut target, true);
    // At the interior the only transparency is the explicit 1/2 figure fade. Its covered
    // half sees clear foreground; its uncovered half sees the full fogged bank. Three
    // byte steps allow the raster's two sRGB encodings plus half-float surface rounding.
    for c in 0..3 {
        let linear = |v: u8| crate::palette::srgb_linear(u32::from(v))[2];
        let expected = linear(clear_half[centre + c])
            + 0.5 * (linear(fog_bg[centre + c]) - linear(clear_bg[centre + c]));
        let encoded = if expected <= 0.0031308 {
            expected * 12.92
        } else {
            1.055 * expected.powf(1.0 / 2.4) - 0.055
        };
        assert!(
            (fog_half[centre + c] as f32 - encoded * 255.0).abs() <= 3.0,
            "partial coverage has no fringe: channel {c}"
        );
    }
    r.set_figures(FigureFrame::default());
    assert_eq!(
        draw(&gpu, &mut r, &mut target, true),
        fog_bg,
        "a skipped figure pass never reuses stale depth"
    );
    target.destroy(&gpu);
    r.destroy(&gpu);
}

#[test]
fn the_plant_pass_hides_the_fog_behind_its_front_face() {
    let Ok(gpu) = Gpu::open(&[]) else {
        eprintln!("skipped: no Vulkan device");
        return;
    };
    let p = params(true);
    let mut r = VoxelRenderer::new(&gpu, p.clone(), &VoxelTextures::empty(p.s, p.rise)).unwrap();
    stage(&mut r, false);
    let mut target = Headless::new(&gpu, &r).unwrap();
    let fog_bg = draw(&gpu, &mut r, &mut target, true);
    stage(&mut r, true);
    let clear = draw(&gpu, &mut r, &mut target, false);
    let fog = draw(&gpu, &mut r, &mut target, true);
    let centre = ((p.base - 7 * p.s as i32 + p.s as i32 / 2) as u32 * p.raster_w + 10 * p.s + 2)
        as usize
        * 4;
    assert_ne!(
        &clear[centre..centre + 3],
        &fog_bg[centre..centre + 3],
        "plant covers the bank"
    );
    assert_eq!(
        &clear[centre..centre + 3],
        &fog[centre..centre + 3],
        "plant front at slab zero sees no rear fog"
    );
    stage(&mut r, false);
    assert_eq!(
        draw(&gpu, &mut r, &mut target, true),
        fog_bg,
        "removing the plant restores the bank"
    );
    target.destroy(&gpu);
    r.destroy(&gpu);
}
