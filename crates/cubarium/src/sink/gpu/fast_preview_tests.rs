//! The fast preview tier on a headless renderer: the sky plane's ease over a picture
//! already drawn, and the pictures the fast and refined tiers end at. Each test returns
//! early, saying so, where no GPU device opens.

use super::*;
use crate::voxel::project::Projection;
use cubarium_gpu::target::readback::ReadbackRing;
use cubarium_gpu::FrameSource;
use cubarium_voxel::{Command, Config};
use cubarium_voxel_fauna::FaunaConfig;
use cubarium_voxel_flora::FloraConfig;

/// A floor with a slab over its middle third: the sky plane shades the ground under the
/// slab, so a picture under the plane differs from one under open sky. `mark` puts one
/// rock somewhere of its own, so each test's terrain is its own sky plane (planes are
/// kept by terrain across the process).
pub(super) fn sheltered(c: &Config, mark: u32) -> World {
    let mut world = World::empty(c.clone());
    let set = |world: &mut World, x: u32, y: u32, z: u32, material: Material| {
        world.apply(Command::SetMaterial { x: i64::from(x), y, z, material });
    };
    for z in 0..c.depth {
        for x in 0..c.width {
            set(&mut world, x, 0, z, Material::Bedrock);
            set(&mut world, x, 1, z, Material::Soil);
        }
    }
    for z in 0..c.depth {
        for x in c.width / 3..2 * c.width / 3 {
            set(&mut world, x, 5, z, Material::Rock);
        }
    }
    set(&mut world, mark % c.width, 2, c.depth - 1, Material::Rock);
    world
}

fn lit(c: &Config) -> (VoxelConfig, Projection) {
    let mut cfg = VoxelConfig {
        px_per_voxel: 4,
        textures: false,
        vine_tiles: false,
        lighting: Lighting::Lit,
        sim: cubarium_session::SimParams { world: c.clone(), ..Default::default() },
        ..VoxelConfig::default()
    };
    // The light shafts bake on their own clock; these tests are about the sky plane.
    cfg.light.volumetric = false;
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, c).unwrap();
    (cfg, proj)
}

fn open(c: &Config, o: OffscreenOptions) -> Option<VoxelGpuSink<Offscreen>> {
    let (cfg, proj) = lit(c);
    match VoxelGpuSink::offscreen_with(&cfg, proj, None, o) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("no GPU device: {e:#}");
            None
        }
    }
}

/// Draw one frame and read it back, as the hosted presenter does.
fn draw(sink: &mut VoxelGpuSink<Offscreen>) -> Vec<u8> {
    let mut ring = ReadbackRing::new(&sink.gpu, sink.renderer.raster_size(), 2).unwrap();
    let mut out = Vec::new();
    sink.renderer.set_view_x(0);
    assert!(ring.submit_source(&sink.gpu, &mut sink.renderer, (), sink.frames).unwrap(), "a frame was submitted");
    sink.frames += 1;
    ring.drain_source(&sink.gpu, &mut sink.renderer, |_, bytes| out.extend_from_slice(bytes)).unwrap();
    assert!(!out.is_empty(), "a frame was read back");
    out
}

/// Wait for the sky plane being baked (the next stage takes it in).
fn await_plane(sink: &mut VoxelGpuSink<Offscreen>) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !sink.wait_sky(Duration::from_millis(20)) {
        assert!(Instant::now() < deadline, "the sky plane never came");
    }
}

/// The largest difference of one colour channel between two pictures.
fn worst(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len(), "pictures of one size");
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

fn bare() -> (Flora, Fauna) {
    (Flora::new(FloraConfig::default()), Fauna::new(FaunaConfig::default()))
}

/// **The sky plane eases in over a picture already drawn, and shows at once before
/// one.** Landing over a drawn picture, the lift starts at open sky (1) and comes down to
/// the plane (0) over the configured seconds of the presenter's clock, never rising, and
/// no frame's step larger than a pop would make (twice the frame's share of the ease).
/// Landing before any frame, the plane is there at once and nothing eases.
#[test]
fn the_sky_plane_eases_in_over_a_drawn_picture_and_at_once_before_one() {
    let _queues = super::super::light::preview_queues();
    let c = Config { width: 24, height: 10, depth: 6, ..Config::default() };
    let world = sheltered(&c, 3);
    let (flora, fauna) = bare();
    // Any duration: the ease is measured against the one asked for.
    let fade = 0.37_f32;
    let o = OffscreenOptions { pipelines: Pipelines::Still, sky: SkyBake::Queued, sky_fade_s: fade };
    let Some(mut sink) = open(&c, o) else { return };
    let t0 = Instant::now();
    sink.set_presenter_clock(Some(t0));
    sink.set_clock(0, 0.0);
    // The plane waits behind another preview's until the first picture is drawn.
    let held = super::super::light::hold_plane_turns();
    assert!(sink.stage_world(&world, &flora, &fauna));
    assert!(sink.light_pending(), "the plane is still being baked");
    draw(&mut sink);
    assert_eq!(sink.renderer.sky_lift(), 0.0, "nothing to ease before the plane");
    drop(held);
    await_plane(&mut sink);
    assert!(sink.stage_world(&world, &flora, &fauna));
    assert!(!sink.light_pending(), "the plane is in");
    assert!(sink.light_fading(), "a plane over a drawn picture eases in");
    assert_eq!(sink.renderer.sky_lift(), 1.0, "the ease starts from the open sky drawn");
    let frames = 30;
    let dt = fade / frames as f32;
    let mut last = 1.0_f32;
    let mut seen = vec![last];
    for i in 1..=frames + 10 {
        let at = Duration::from_secs_f32(dt * i as f32);
        sink.set_presenter_clock(Some(t0 + at));
        sink.set_clock(0, 0.0);
        let lift = sink.renderer.sky_lift();
        seen.push(lift);
        assert!(lift <= last, "the lift rose: {seen:?}");
        assert!(last - lift <= 2.0 * dt / fade + 1e-4, "a step of {} in one frame: {seen:?}", last - lift);
        if at.as_secs_f32() < fade * 0.999 {
            assert!(lift > 0.0, "the plane was full before its {fade} s: {seen:?}");
            assert!(sink.light_fading());
        } else {
            assert_eq!(lift, 0.0, "the plane is whole once its {fade} s are up: {seen:?}");
            assert!(!sink.light_fading());
        }
        last = lift;
    }
    drop(sink);

    // The same terrain's plane landing before the first frame: no ease.
    let Some(mut sink) = open(&c, o) else { return };
    sink.set_presenter_clock(Some(t0));
    sink.set_clock(0, 0.0);
    assert!(sink.stage_world(&world, &flora, &fauna));
    await_plane(&mut sink);
    assert!(sink.stage_world(&world, &flora, &fauna));
    assert!(!sink.light_pending());
    assert_eq!(sink.renderer.sky_lift(), 0.0, "a plane before the first frame shows at once");
    assert!(!sink.light_fading(), "and does not ease");
}

/// **Both tiers start at the same picture, and the refined one ends at the lit one.**
/// The fast tier's only picture and the refined tier's first are the same (open sky);
/// once the refined tier's plane has landed and eased in, its picture is the one a
/// renderer that had its plane from the start draws. Tolerances, not bits.
#[test]
fn fast_and_refined_start_alike_and_refined_ends_at_the_lit_picture() {
    let _queues = super::super::light::preview_queues();
    let c = Config { width: 24, height: 10, depth: 6, ..Config::default() };
    let world = sheltered(&c, 11);
    let (flora, fauna) = bare();
    let fade = 0.25_f32;
    let t0 = Instant::now();

    let fast_o = OffscreenOptions { pipelines: Pipelines::Still, sky: SkyBake::Never, sky_fade_s: 0.0 };
    let Some(mut fast) = open(&c, fast_o) else { return };
    fast.set_presenter_clock(Some(t0));
    fast.set_clock(0, 0.0);
    assert!(fast.stage_world(&world, &flora, &fauna));
    assert!(!fast.light_pending(), "the fast tier waits for nothing");
    let fast_picture = draw(&mut fast);
    drop(fast);

    let refined_o = OffscreenOptions { pipelines: Pipelines::Still, sky: SkyBake::Queued, sky_fade_s: fade };
    let Some(mut refined) = open(&c, refined_o) else { return };
    refined.set_presenter_clock(Some(t0));
    refined.set_clock(0, 0.0);
    assert!(refined.stage_world(&world, &flora, &fauna));
    let first = draw(&mut refined);
    assert!(worst(&first, &fast_picture) <= 2, "the refined tier's first picture is the fast one's");
    await_plane(&mut refined);
    assert!(refined.stage_world(&world, &flora, &fauna));
    let mut eased = Vec::new();
    for i in 1..=12 {
        refined.set_presenter_clock(Some(t0 + Duration::from_secs_f32(fade * i as f32 / 8.0)));
        refined.set_clock(0, 0.0);
        eased = draw(&mut refined);
    }
    assert!(!refined.light_pending() && !refined.light_fading(), "the refined tier's lighting is whole");
    drop(refined);

    let lit_o = OffscreenOptions { pipelines: Pipelines::Still, sky: SkyBake::Now, sky_fade_s: 0.0 };
    let Some(mut reference) = open(&c, lit_o) else { return };
    reference.set_presenter_clock(Some(t0));
    reference.set_clock(0, 0.0);
    assert!(reference.stage_world(&world, &flora, &fauna));
    await_plane(&mut reference);
    assert!(reference.stage_world(&world, &flora, &fauna));
    assert!(!reference.light_pending());
    let lit_picture = draw(&mut reference);

    assert!(worst(&fast_picture, &lit_picture) > 8, "the fixture's plane shades the picture visibly");
    let off = worst(&eased, &lit_picture);
    assert!(off <= 2, "the refined tier ends {off} colour codes off the lit picture");
}
