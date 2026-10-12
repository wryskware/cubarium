#![forbid(unsafe_code)]
use super::*;
#[cfg(feature = "counters")]
use crate::voxel::{self, VoxelStyle, VoxelTexel, VoxelTextures};
use crate::{atlas::Atlas, scene::RingLayout, synthetic::SyntheticWorld};

fn gpu() -> Gpu {
    Gpu::open().expect("GPU acceptance requires a rendering adapter")
}

#[test]
fn discarded_timing_reuses_its_free_slot_while_an_older_frame_is_pending() {
    let gpu = gpu();
    let mut timing = timing::Timing::default();
    let mut first = gpu.encoder("older pending timing");
    let frame = timing.begin(&gpu, 4).unwrap();
    for i in 0..4 {
        frame.stamp(&mut first, i);
    }
    timing.finish(&mut first, frame);
    let mut waiting = gpu.encoder("displaced timing");
    let frame = timing.begin(&gpu, 4).unwrap();
    for i in 0..4 {
        frame.stamp(&mut waiting, i);
    }
    timing.finish(&mut waiting, frame);
    drop(waiting);
    timing.discard();
    let created = gpu.creation_counts();
    assert!(
        timing.begin(&gpu, 4).is_some(),
        "the freed pool can be reused even though the older pool is busy"
    );
    assert_eq!(gpu.creation_counts(), created);
    timing.discard();
    drop(first);
    timing.discard();
    assert!(gpu.take_errors().is_empty());
}
fn atlas() -> Atlas {
    Atlas::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .unwrap()
}
#[cfg(feature = "counters")]
fn lit(gpu: &Gpu) -> voxel::VoxelRenderer {
    let p = voxel::VoxelParams {
        lit: true,
        plants: Some(Default::default()),
        ..voxel::tests::params()
    };
    let mut renderer =
        voxel::VoxelRenderer::new(gpu, p, &VoxelTextures::empty(p.s, p.rise)).unwrap();
    let mut cells = vec![VoxelTexel::default(); p.voxel_count()];
    for x in 0..p.width {
        cells[voxel::VoxelStaging::index(p.width, p.height, x, 0, 2)] =
            VoxelTexel::pack(2, 0, 0., true, 0., 0);
    }
    assert!(renderer.stage([1; voxel::SLOW_PLANES], |st| {
        st.owed.mark_full();
        voxel::write_upload(
            &cells, p.width, p.height, p.depth, st.owed, st.voxels, st.carried,
        );
        st.roof.fill(0);
        st.glyphs.fill(0);
        st.sky.fill(255);
        st.canopy.fill(0);
        st.glow.fill(0);
        st.styles[1] = VoxelStyle::new([0.1, 0.7, 0.3], [0.1, 0.7, 0.3], [0.1, 0.7, 0.3]);
        let plants = st.plants.unwrap();
        plants.cells = vec![crate::plants::pack_cell(
            [8, 2, 2],
            1,
            1.,
            false,
            crate::plants::SETTLED,
            0,
        )];
        plants.plants = vec![
            Default::default(),
            crate::plants::PlantRec {
                x: 8.,
                amp: 0.5,
                lag: 0.,
            },
        ];
    }));
    renderer.set_emit_cells(&[voxel::EmitCell::emitter(3, [0.2, 0.5, 0.8], 1.)]);
    let cell =
        crate::figures::FigureVoxel::new([0, 0, 0], 0, crate::figures::CLASS_NONE, false, 63, 0);
    assert!(renderer.put_figure_models(0, &[cell]));
    renderer
}
#[cfg(feature = "counters")]
fn figure(x: f32) -> crate::figures::FigureFrame {
    use crate::figures::*;
    FigureFrame {
        figures: vec![FigureInstance {
            id: 1,
            a: FigureTile {
                first: 0,
                count: 1,
                pos: [x, 4., 3.],
                axes: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                centre: [0.; 3],
                radius: 1.,
                pose: [0.; 4],
                palette: 0,
                bones: None,
            },
            b: None,
            weight_b: 0.,
            alpha: 1.,
            highlight: 0.,
        }],
        palette: vec![PaletteEntry {
            base: [0.8, 0.2, 0.1],
            emit: None,
        }],
        ..Default::default()
    }
}
#[cfg(feature = "counters")]
fn live_counts(gpu: &Gpu) -> [isize; 6] {
    let c = gpu.device.get_internal_counters().hal;
    [
        c.buffers.read(),
        c.bind_groups.read(),
        c.query_sets.read(),
        c.textures.read(),
        c.texture_views.read(),
        c.samplers.read(),
    ]
}

#[cfg(feature = "counters")]
#[test]
fn steady_lit_and_animal_motion_create_no_gpu_resources_for_120_frames_each() {
    let gpu = gpu();
    let mut renderer = lit(&gpu);
    renderer.set_figures(figure(8.));
    let mut output = headless::Headless::with_size(&gpu, &renderer, (128, 64)).unwrap();
    for _ in 0..4 {
        output.draw(&gpu, &mut renderer, ()).unwrap();
    }
    let created = gpu.creation_counts();
    let live = live_counts(&gpu);
    for _ in 0..120 {
        output.draw(&gpu, &mut renderer, ()).unwrap();
    }
    assert_eq!(
        gpu.creation_counts(),
        created,
        "unchanged lit world creates no objects"
    );
    assert_eq!(live_counts(&gpu), live);
    let before = renderer.read_raster(&gpu).unwrap();
    // The explicit image read allocates its own diagnostic readback buffer.
    let created = gpu.creation_counts();
    for i in 0..120 {
        renderer.set_clock(voxel::FrameClock::at(i, 0.5, 60., 20., true));
        renderer.set_figures(figure(8. + i as f32 / 16.));
        output.draw(&gpu, &mut renderer, ()).unwrap();
    }
    assert_eq!(
        gpu.creation_counts(),
        created,
        "animal motion creates no objects"
    );
    assert_ne!(
        renderer.read_raster(&gpu).unwrap(),
        before,
        "the animal actually moves"
    );
    assert!(gpu.take_errors().is_empty());
    eprintln!(
        "ALLOC lit: unchanged=120 motion=120 buffers=0 bind_groups=0 query_sets=0 textures=0"
    );
}

#[cfg(feature = "counters")]
#[test]
fn steady_flat_and_present_create_no_gpu_resources_for_120_frames_each() {
    let gpu = gpu();
    let atlas = atlas();
    let layout = RingLayout {
        w: 64,
        h: 32,
        scale: 1,
    };
    let mut world = SyntheticWorld::new(layout, 1);
    let mut renderer = flat::Renderer::new(&gpu, &atlas, layout).unwrap();
    let mut target = headless::Headless::with_size(&gpu, &renderer, (128, 64)).unwrap();
    for _ in 0..4 {
        target
            .draw(&gpu, &mut renderer, world.frame(&atlas, 1., 0.))
            .unwrap();
    }
    let created = gpu.creation_counts();
    let live = live_counts(&gpu);
    for _ in 0..120 {
        target
            .draw(&gpu, &mut renderer, world.frame(&atlas, 1., 0.))
            .unwrap();
    }
    assert_eq!(gpu.creation_counts(), created);
    assert_eq!(live_counts(&gpu), live);
    for i in 0..120 {
        target
            .draw(
                &gpu,
                &mut renderer,
                world.frame(&atlas, 1. + i as f64 / 60., 0.5),
            )
            .unwrap();
    }
    assert_eq!(gpu.creation_counts(), created);
    assert!(gpu.take_errors().is_empty());
    eprintln!(
        "ALLOC flat/present: unchanged=120 motion=120 buffers=0 bind_groups=0 query_sets=0 textures=0"
    );
}

#[test]
fn an_injected_validation_error_does_not_panic_or_break_a_shared_device_renderer() {
    let gpu = gpu();
    let atlas = atlas();
    let layout = RingLayout {
        w: 64,
        h: 32,
        scale: 1,
    };
    let mut bad = flat::Renderer::new(&gpu, &atlas, layout).unwrap();
    let mut good = flat::Renderer::new(&gpu, &atlas, layout).unwrap();
    let mut world = SyntheticWorld::new(layout, 1);
    let mut target = headless::Headless::new(&gpu, &good).unwrap();
    target
        .draw(&gpu, &mut good, world.frame(&atlas, 1., 0.))
        .unwrap();
    let before = good.read_raster(&gpu).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut encoder = gpu.encoder("injected validation error on first renderer");
        bad.record(&gpu, &mut encoder, world.frame(&atlas, 1., 0.), None)
            .unwrap();
        // The source deliberately lacks COPY_SRC, so wgpu reports validation.
        let invalid = gpu.buffer(&wgpu::BufferDescriptor {
            label: Some("injected bad source"),
            size: 4,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let destination = gpu.buffer(&wgpu::BufferDescriptor {
            label: Some("injected destination"),
            size: 4,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(&invalid, 0, &destination, 0, 4);
        drop(encoder.finish());
        bad.frame_discarded();
    }));
    assert!(
        result.is_ok(),
        "uncaptured validation logs instead of unwinding"
    );
    assert!(
        !gpu.take_errors().is_empty(),
        "the injected error reached the handler"
    );
    target
        .draw(&gpu, &mut good, world.frame(&atlas, 1., 0.))
        .unwrap();
    assert_eq!(good.read_raster(&gpu).unwrap(), before);
    assert!(gpu.take_errors().is_empty());
}

#[test]
fn a_readback_map_failure_retires_the_source_slot_and_the_ring_is_reusable() {
    let gpu = gpu();
    let atlas = atlas();
    let layout = RingLayout {
        w: 64,
        h: 32,
        scale: 1,
    };
    let mut renderer = flat::Renderer::new(&gpu, &atlas, layout).unwrap();
    let mut world = SyntheticWorld::new(layout, 1);
    let mut ring = ReadbackRing::new(&gpu, renderer.raster_size(), 1).unwrap();
    assert!(
        ring.submit_source(&gpu, &mut renderer, world.frame(&atlas, 1., 0.), 1)
            .unwrap()
    );
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(30)),
        })
        .unwrap();
    let slot = ring.flight[0];
    *ring.slots[slot].ready.lock().unwrap() = Some(Err(wgpu::BufferAsyncError));
    assert!(
        ring.poll_source(&gpu, &mut renderer, |_, _| panic!(
            "failed maps deliver no pixels"
        ))
        .is_err()
    );
    assert_eq!(renderer.frames_in_flight(), 0);
    assert_eq!(ring.in_flight(), 0);
    assert!(
        ring.submit_source(&gpu, &mut renderer, world.frame(&atlas, 1., 0.), 2)
            .unwrap()
    );
    let mut delivered = 0;
    ring.drain_source_rows(&gpu, &mut renderer, |tag, rows| {
        assert_eq!(tag, 2);
        assert_eq!(rows.rows().count(), 32);
        delivered += 1;
    })
    .unwrap();
    assert_eq!(delivered, 1);
    assert_eq!(renderer.frames_in_flight(), 0);
}
