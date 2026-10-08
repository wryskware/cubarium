//! The animals' live settings (movement, subpixel placement) on a running GPU sink: a
//! change reaches the very next frame's figures, on the flat tier too, which otherwise
//! moves its figures only once a tick. Mechanisms only, no tuned numbers. Needs a Vulkan
//! device and skips with a printed reason when there is none.

use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::VoxelConfig;
use cubarium::voxel::figure::{FigureMotion, Movement};
use cubarium::voxel::project::Projection;
use cubarium_voxel::{Command, Config, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel_flora::{Flora, FloraConfig};

fn on(x: f32, phase: f32) -> bool {
    let d = x - phase;
    (d - d.round()).abs() < 1e-4
}

#[test]
fn a_motion_change_moves_the_next_frames_figures_on_a_running_sink() {
    let c = Config { width: 32, height: 8, depth: 4, ..Config::default() };
    let cfg = VoxelConfig {
        sim: cubarium_session::SimParams { world: c.clone(), ..Default::default() },
        ..VoxelConfig::default()
    };
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &c).unwrap();
    let mut sink = match VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions { target: GpuTargetKind::Headless, ..VoxelGpuSinkOptions::default() },
    ) {
        Ok(sink) => sink,
        Err(e) => {
            println!("skipped: no Vulkan device for the voxel GPU renderer ({e:#})");
            return;
        }
    };
    assert!(sink.figures_on(), "the default config draws animals as models");
    assert_eq!(cfg.lighting, cubarium::voxel::Lighting::Flat, "the flat tier: figures move once a tick");

    let mut world = World::empty(c.clone());
    for x in 0..i64::from(c.width) {
        for z in 0..c.depth {
            world.apply(Command::SetMaterial { x, y: 0, z, material: Material::Rock });
        }
    }
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 8,
            z: 1,
            founder: Founder::Browser,
            stores: StartingStores::FULL,
            heading_rad: 0.0,
        }
    ));
    // Two ticks of the one animal, off the voxel grid and across voxels between them.
    let v = c.voxel_m;
    let base = fauna.view();
    let mut a = base.animals.to_vec();
    let mut b = base.animals.to_vec();
    a[0].pose.x = 8.3 * v;
    a[0].pose.z = 1.6 * v;
    b[0].pose.x = 10.2 * v;
    b[0].pose.z = 1.7 * v;
    for (tick, animals) in [(1u64, &a), (2, &b)] {
        let mut view = world.view();
        view.tick = tick;
        let fv = cubarium_voxel_fauna::FaunaView { animals, tick, ..fauna.view() };
        assert!(sink.stage_view(&view, flora.view(), fv), "tick {tick} staged");
    }
    let x_drawn = |sink: &mut VoxelGpuSink<_>| {
        sink.set_clock(2, 0.5);
        let pick = sink.figure_pick();
        assert_eq!(pick.figures.len(), 1, "one figure drawn");
        pick.figures[0].pos
    };
    let smooth = x_drawn(&mut sink);
    assert!(!on(smooth[0], 0.5), "smooth: between voxels mid-tick ({smooth:?})");
    sink.set_figure_motion(FigureMotion { movement: Movement::Stepped, subpixel: true });
    assert_eq!(sink.figure_motion().map(|m| m.movement), Some(Movement::Stepped));
    let stepped = x_drawn(&mut sink);
    assert!(
        on(stepped[0], 0.5) && on(stepped[2], 0.5) && on(stepped[1], 0.0),
        "stepped, the same tick: on the grid ({stepped:?})"
    );
    assert!((stepped[0] - smooth[0]).abs() <= 0.5, "the voxel it is in, not somewhere else");
    sink.set_figure_motion(FigureMotion { movement: Movement::Smooth, subpixel: true });
    assert_eq!(x_drawn(&mut sink), smooth, "and back to the glide, the same tick");
}
