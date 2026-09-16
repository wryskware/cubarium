//! GS-1b deliverable 3: the GPU picture of one `RenderView` against the CPU presenter's.
//!
//! The CPU presenter is the reference. This does **not** assert the two are identical —
//! they cannot be, and a test that claimed it would be a test of nothing. It renders one
//! view both ways and reports the distance, with a threshold loose enough to survive the
//! differences named below and tight enough that a lost pass, a wrong colour or a shifted
//! sprite fails it.
//!
//! # The differences that are meant to be there
//!
//! * **Bilinear against nearest.** `Sprite::sample` reads four texels and four weights;
//!   the GPU floors to one. At `S = 1` with an integer anchor and an axis-aligned heading
//!   they agree exactly, so what is left is the jittered plant headings (±12°) and the
//!   bodies, whose rotated samples the CPU softens and the GPU does not. **The GPU is
//!   right for this art**: a bilinear sample of a pixel-art tile invents colours the
//!   palette does not contain, which is the thing `design/appearance.md` asks the
//!   renderer not to do.
//! * **The bend.** The CPU displaces by a real number inside the bilinear sample; the GPU
//!   rounds the displacement to a whole source texel per row. With the shipped pack's
//!   measured budgets (0.3–1.3 texels) that is mostly no displacement at all against the
//!   CPU's fraction of one.
//! * **Column height between ticks.** `ArtPresenter` interpolates a column's height
//!   across the tick; it exposes the current height but not the previous one, so the GPU
//!   driver uses the tick's own value. Columns grow at 1.5 px/s, so the gap is under a
//!   tenth of a pixel and only on the frames between ticks.
//! * **Three-layer body fades.** Four frame slots hold two whole poses. A body that
//!   changed state twice inside `BODY_FADE_SECONDS` wants three; the lightest is dropped
//!   and the rest renormalised. The test counts how often that happened.
//! * **Rig compositing.** A hunter's parts are one instance each, so translucent overlaps
//!   composite part-over-part instead of through the rig's single query. Every Lanternjaw
//!   texel is opaque or clear, so the art cannot express the difference — and the fixture
//!   below has no hunters anyway.

use cubarium::art_present::{ArtPresenter, present_seconds};
use cubarium::art::ArtPack;
use cubarium::sink::gpu::{GpuSink, GpuTargetKind};
use cubarium::sink::{FrameSink, WorldShape};
use cubarium_core::config::WorldConfig;
use cubarium_core::world::World;
use cubarium_render::Canvas;
use cubarium_surface::{Scale, Topology};
use cube_proto::Raster;

const SEED: u64 = 20260916;
const TICKS: u32 = 3_000;

fn art() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}

fn ring_world(w: u16, h: u16, scale: f64, ticks: u32) -> World {
    let mut cfg = WorldConfig { seed: SEED, ..WorldConfig::default() };
    cfg.topology = Topology::Ring { w, h };
    cfg.world_scale = Scale::new(scale);
    let mut world = World::new(cfg).expect("a legal ring world");
    for _ in 0..ticks {
        world.step();
    }
    world
}

/// The CPU presenter's picture of one view, as `w · h · 3` sRGB bytes.
fn cpu_raster(world: &World, f: f64) -> (u16, u16, Vec<u8>) {
    let view = world.render_view();
    let Topology::Ring { w, h } = view.topology else { unreachable!() };
    let pack = ArtPack::load(&art()).expect("the art pack");
    let mut presenter = ArtPresenter::for_world(pack, view.topology, view.scale);
    presenter.observe(&view);
    presenter.observe_hunters(&view, &world.hunter_view(), &[]).expect("no hunters");
    let mut canvas = Canvas::new(view.topology, view.scale);
    presenter.draw(&view, f, &mut canvas);
    let mut raster = Raster::black(w, h);
    canvas.encode_raster(&mut raster);
    (w, h, raster.as_bytes().to_vec())
}

/// `(mean absolute channel difference, share of pixels whose worst channel is off by more
/// than 8, the worst channel difference)`.
fn compare(gpu: &[u8], cpu: &[u8], w: u32, h: u32) -> (f64, f64, u8) {
    let mut total = 0u64;
    let mut over = 0usize;
    let mut worst = 0u8;
    for i in 0..(w * h) as usize {
        let mut pixel_worst = 0u8;
        for c in 0..3 {
            let d = gpu[i * 4 + c].abs_diff(cpu[i * 3 + c]);
            total += u64::from(d);
            pixel_worst = pixel_worst.max(d);
        }
        worst = worst.max(pixel_worst);
        if pixel_worst > 8 {
            over += 1;
        }
    }
    (
        total as f64 / (w * h * 3) as f64,
        over as f64 / (w * h) as f64,
        worst,
    )
}

/// Render one view both ways. `None` when this machine has no Vulkan device, which is not
/// a failure of the renderer.
fn both(w: u16, h: u16, scale: f64, ticks: u32, f: f64) -> Option<(f64, f64, u8, u64)> {
    both_with(w, h, scale, ticks, f, false)
}

fn both_with(
    w: u16,
    h: u16,
    scale: f64,
    ticks: u32,
    f: f64,
    bilinear: bool,
) -> Option<(f64, f64, u8, u64)> {
    let world = ring_world(w, h, scale, ticks);
    let view = world.render_view();
    let shape = WorldShape::new(view.topology, view.scale);
    let mut sink = match GpuSink::new(shape, &art(), GpuTargetKind::Headless, false, bilinear, None, None) {
        Ok(sink) => sink,
        Err(e) => {
            eprintln!("no GPU here ({e:#}); the fidelity comparison was not run");
            return None;
        }
    };
    sink.observe_world(&view, &world.hunter_view(), &[]);
    sink.observe_view(&view, present_seconds(view.tick, f), f).expect("render");
    let gpu = sink.read_raster().expect("read the raster back");
    let dropped = sink.dropped_frames();
    let (cw, ch, cpu) = cpu_raster(&world, f);
    assert_eq!((cw, ch), (w, h));
    // `CUBARIUM_GPU_FIDELITY_DUMP=<dir>` writes both pictures out, which is how a
    // difference gets diagnosed rather than argued about.
    if let Ok(dir) = std::env::var("CUBARIUM_GPU_FIDELITY_DUMP") {
        let dir = std::path::Path::new(&dir);
        let tag = format!("{w}x{h}-s{scale}-f{f}{}", if bilinear { "-bilinear" } else { "" });
        cubarium_gpu::target::write_png(&dir.join(format!("gpu-{tag}.png")), u32::from(w), u32::from(h), &gpu).ok();
        let rgba: Vec<u8> = cpu
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        cubarium_gpu::target::write_png(&dir.join(format!("cpu-{tag}.png")), u32::from(w), u32::from(h), &rgba).ok();
    }
    let (mean, over, worst) = compare(&gpu, &cpu, u32::from(w), u32::from(h));
    Some((mean, over, worst, dropped))
}

/// A sweep over the world's age, printed rather than asserted: it is how a difference is
/// attributed to a pass rather than guessed at, since a young world has no plants, a
/// middle-aged one no tall columns, and a mature one everything.
#[test]
#[ignore = "diagnostic; run with --ignored"]
fn the_difference_by_world_age() {
    for ticks in [0, 40, 600, 3000] {
        for bilinear in [false, true] {
            match both_with(320, 180, 1.0, ticks, 0.0, bilinear) {
                Some((mean, over, worst, _)) => println!(
                    "{ticks:5} ticks {:8}: mean |Δ| {mean:6.3}, {:5.2}% over 8, worst {worst}",
                    if bilinear { "bilinear" } else { "nearest" },
                    over * 100.0
                ),
                None => return,
            }
        }
    }
}

#[test]
fn the_gpu_draws_the_cpus_picture_when_it_samples_the_cpus_way() {
    // **The regression that matters.** With `--gpu-filter bilinear` the GPU uses
    // `Sprite::sample`'s own four taps at the CPU's own un-snapped sub-pixel anchor, so
    // the only thing left between the two renderers is... nothing. If the adapter picked
    // a different stamp, a different clip phase, a different opacity, mask, bend or tone,
    // or drew the passes in a different order, this is where it would show.
    for (ticks, f) in [(TICKS, 0.0), (TICKS, 0.5), (40, 0.0)] {
        let Some((mean, over, worst, dropped)) = both_with(320, 180, 1.0, ticks, f, true) else {
            return;
        };
        println!(
            "bilinear, {ticks} ticks, f = {f}: mean |Δ| {mean:.3} per channel, \
             {:.2}% of pixels off by more than 8, worst {worst}, {dropped} slot(s) dropped",
            over * 100.0
        );
        assert!(mean <= 1.5, "mean |Δ| {mean:.3} per channel at {ticks} ticks, f = {f}");
        assert!(
            over <= 0.04,
            "{:.2}% of pixels off by more than 8 at {ticks} ticks, f = {f}",
            over * 100.0
        );
    }
}

#[test]
fn the_pixel_art_sampler_differs_from_the_cpu_only_as_much_as_the_sampler_does() {
    // The **default** mode, and the number the report quotes. It is large — around 17 per
    // channel — and that is the sampler, not a fault: the CPU bilinearly resamples every
    // tile at a random sub-pixel anchor (`slot_of` jitters the anchor by ±1 px), which
    // manufactures colours the palette does not contain; the GPU keeps each source texel
    // whole. The bound is there to catch a *collapse*, not to pin the sampler.
    let Some((mean, over, worst, _)) = both(320, 180, 1.0, TICKS, 0.0) else { return };
    println!(
        "nearest, {TICKS} ticks, f = 0: mean |Δ| {mean:.3} per channel, \
         {:.2}% of pixels off by more than 8, worst {worst}",
        over * 100.0
    );
    assert!(mean <= 25.0, "mean |Δ| {mean:.3} per channel");
    assert!(over <= 0.80, "{:.2}% of pixels off by more than 8", over * 100.0);
}

#[test]
fn the_two_renderers_agree_at_scale_two_as_well() {
    let Some((mean, over, worst, _)) = both_with(640, 360, 2.0, TICKS, 0.0, true) else { return };
    println!(
        "bilinear, 640x360 S=2: mean |Δ| {mean:.3}, {:.2}% over 8, worst {worst}",
        over * 100.0
    );
    assert!(mean <= 1.5 && over <= 0.04);
}
