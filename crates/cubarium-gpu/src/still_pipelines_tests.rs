//! A renderer built with [`Pipelines::Still`] asked for a walk variant it did not build.
use super::*;
use crate::target::Headless;

fn params() -> VoxelParams {
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
        effects: crate::sunvis::Effects { shadows: false, reflections: false, glow: false, ao: false, ..Default::default() },
        ..super::tests::params()
    }
}

/// A bank of rock across the back; with `pose`, a pack whose table has a body posing
/// (no cell of it drawn, so the picture is the same either way).
fn stage(r: &mut VoxelRenderer, pose: bool) {
    let p = r.params;
    let mut cells = vec![VoxelTexel::default(); p.voxel_count()];
    for x in 0..p.width {
        for y in 0..16 {
            cells[VoxelStaging::index(p.width, p.height, x, y, 12)] = VoxelTexel::pack(2, 0, 0.0, true, 0.0, 0);
        }
    }
    assert!(r.stage([1; SLOW_PLANES], |st| {
        st.owed.mark_full();
        write_upload(&cells, p.width, p.height, p.depth, st.owed, st.voxels, st.carried);
        st.roof.fill(0);
        st.glyphs.fill(0);
        st.sky.fill(255);
        st.canopy.fill(0);
        st.glow.fill(0);
        st.styles[1] = VoxelStyle::new([0.7, 0.1, 0.03], [0.7, 0.1, 0.03], [0.7, 0.1, 0.03]);
        *st.poses = PoseTable::EMPTY;
        if pose {
            st.poses.entries[1] = PoseEntry { sleep: 1.0, drop: 1.0, ..PoseEntry::ZERO };
        }
    }));
}

fn draw(gpu: &Gpu, r: &mut VoxelRenderer, target: &mut Headless, fog: bool) -> Vec<u8> {
    r.look = crate::weather::noon(&r.params);
    r.look.fog = if fog { 1.0 } else { 0.0 };
    r.look.fog_colour = [0.4, 0.25, 0.65];
    r.raster_current = false;
    target.draw(gpu, r, ()).unwrap();
    r.read_raster(gpu).unwrap()
}

/// **A still renderer asked for a variant it did not build draws with the plain walk.**
/// A frame with a posing body and an emitting plant wants the pose and plant-light
/// variants, which [`Pipelines::Still`] leaves out: it is drawn, without a panic, and is
/// the picture the plain walk draws of the same terrain, with and without weather.
#[test]
fn a_still_renderer_draws_a_variant_it_did_not_build_with_the_plain_walk() {
    let Ok(gpu) = Gpu::open(&[]) else {
        eprintln!("skipped: no Vulkan device");
        return;
    };
    let p = params();
    let mut r = VoxelRenderer::with_pipelines(&gpu, p, &VoxelTextures::empty(p.s, p.rise), Pipelines::Still).unwrap();
    assert_eq!(r.pipelines(), Pipelines::Still);
    let none = vk::Pipeline::null();
    assert!(r.pose_pipeline == none && r.depth_pose_pipeline == none, "no pose variant built");
    assert!(r.plant_light_pipelines.iter().all(|&q| q == none), "no plant-light variant built");
    let mut target = Headless::new(&gpu, &r).unwrap();
    stage(&mut r, false);
    let plain = draw(&gpu, &mut r, &mut target, false);
    let plain_fog = draw(&gpu, &mut r, &mut target, true);
    stage(&mut r, true);
    r.set_emit_cells(&[EmitCell { plant: Some(0.5), ..EmitCell::none(4000) }]);
    let wanting = draw(&gpu, &mut r, &mut target, false);
    assert!(r.posing, "the frame drawn wanted a pose variant");
    let cap = emit_table(&r.emit_cells, &mut Vec::new());
    assert_ne!(emit_view(cap, &r.emit_cells) & EMIT_PLANTS, 0, "and the plant-light variant");
    let wanting_fog = draw(&gpu, &mut r, &mut target, true);
    let worst = |a: &[u8], b: &[u8]| a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
    assert!(worst(&wanting, &plain) <= 2, "the plain walk's picture");
    assert!(worst(&wanting_fog, &plain_fog) <= 2, "the plain walk's picture under weather");
    assert!(worst(&plain, &plain_fog) > 2, "the weather was drawn");
    target.destroy(&gpu);
    r.destroy(&gpu);
}
