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
use cubarium_core::{FixedHunterProfile, HunterTarget};
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

/// A ring world with the Lanternjaw trial started at its centre, matured for `ticks`.
///
/// There is no command line for this: `World::start_hunter_trial` is the only way a
/// hunter enters a world (`world/hunter.rs`, "place **exactly one** founder of the fixed
/// lineage"), and `cubarium run` has no flag that reaches it — `--neural` seeds ordinary
/// trained animals, not predators. The trial is started **before** the ticks so the
/// founder is a settled adult by the time the frame is drawn, and the profile is the
/// world's own `lanternjaw_trial`, which is the profile `ArtPresenter::validate_hunter_
/// profile` accepts.
fn hunter_ring_world(w: u16, h: u16, scale: f64, ticks: u32) -> Option<World> {
    let mut cfg = WorldConfig { seed: SEED, ..WorldConfig::default() };
    cfg.topology = Topology::Ring { w, h };
    cfg.world_scale = Scale::new(scale);
    let mut world = World::new(cfg).expect("a legal ring world");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let target = HunterTarget { face: 0, u: f64::from(w) / 2.0, v: f64::from(h) * 0.55 };
    if let Err(e) = world.start_hunter_trial(profile, target) {
        eprintln!("no hunter trial on this world ({e}); the comparison was not run");
        return None;
    }
    for _ in 0..ticks {
        world.step();
    }
    Some(world)
}

/// The CPU presenter's picture of one view, as `w · h · 3` sRGB bytes.
fn cpu_raster(world: &World, f: f64) -> (u16, u16, Vec<u8>) {
    let view = world.render_view();
    let Topology::Ring { w, h } = view.topology else { unreachable!() };
    let pack = ArtPack::load(&art()).expect("the art pack");
    let mut presenter = ArtPresenter::for_world(pack, view.topology, view.scale);
    presenter.observe(&view);
    presenter
        .observe_hunters(&view, &world.hunter_view(), &[])
        .expect("the art draws this world's hunters");
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
    compare_world(&ring_world(w, h, scale, ticks), w, h, scale, f, bilinear, "")
}

/// Render one world's current view both ways and compare. `tag` names the dump files
/// `CUBARIUM_GPU_FIDELITY_DUMP` writes.
fn compare_world(
    world: &World,
    w: u16,
    h: u16,
    scale: f64,
    f: f64,
    bilinear: bool,
    tag: &str,
) -> Option<(f64, f64, u8, u64)> {
    let view = world.render_view();
    let shape = WorldShape::new(view.topology, view.scale);
    // `bend_substep: Some(false)` explicitly, not the sink's default. GS-1c made the
    // sub-texel bend the default on a ring at S >= 2 because Wrysk asked for a smoother
    // sway on the panel; this comparison is the *adapter's* evidence and its numbers are
    // quoted across four reports, so it keeps naming the whole-texel bend it was
    // measured with. The substep's own cost against the CPU is measured separately.
    let options = cubarium::sink::GpuSinkOptions {
        target: GpuTargetKind::Headless,
        bend_substep: Some(false),
        filter_bilinear: bilinear,
        ..Default::default()
    };
    let mut sink = match GpuSink::new(shape, &art(), options) {
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
    let (cw, ch, cpu) = cpu_raster(world, f);
    assert_eq!((cw, ch), (w, h));
    // `CUBARIUM_GPU_FIDELITY_DUMP=<dir>` writes both pictures out, which is how a
    // difference gets diagnosed rather than argued about.
    if let Ok(dir) = std::env::var("CUBARIUM_GPU_FIDELITY_DUMP") {
        let dir = std::path::Path::new(&dir);
        let tag = format!("{tag}{w}x{h}-s{scale}-f{f}{}", if bilinear { "-bilinear" } else { "" });
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

/// One picture of `world`'s current view, from whichever renderer, with the world's
/// hunters either handed to it or withheld.
///
/// Withholding them is how the rig is isolated: the *same* view, the same plants, the
/// same everything, drawn once with the Lanternjaw and once without. The difference is
/// the rig and nothing else, on either renderer.
fn raster_of(world: &World, gpu: bool, bilinear: bool, with_hunters: bool) -> Vec<u8> {
    let view = world.render_view();
    let none: Vec<cubarium_core::HunterView> = Vec::new();
    let hunters = if with_hunters { world.hunter_view() } else { none };
    if gpu {
        let options = cubarium::sink::GpuSinkOptions {
            target: GpuTargetKind::Headless,
            bend_substep: Some(false),
            filter_bilinear: bilinear,
            ..Default::default()
        };
        let shape = WorldShape::new(view.topology, view.scale);
        let mut sink = GpuSink::new(shape, &art(), options).expect("a GPU");
        sink.observe_world(&view, &hunters, &[]);
        sink.observe_view(&view, present_seconds(view.tick, 0.0), 0.0).expect("render");
        let rgba = sink.read_raster().expect("read the raster back");
        return rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    }
    let Topology::Ring { w, h } = view.topology else { unreachable!() };
    let pack = ArtPack::load(&art()).expect("the art pack");
    let mut presenter = ArtPresenter::for_world(pack, view.topology, view.scale);
    presenter.observe(&view);
    presenter.observe_hunters(&view, &hunters, &[]).expect("the art draws them");
    let mut canvas = Canvas::new(view.topology, view.scale);
    presenter.draw(&view, 0.0, &mut canvas);
    let mut raster = Raster::black(w, h);
    canvas.encode_raster(&mut raster);
    raster.as_bytes().to_vec()
}

/// Which pixels two `w · h · 3` rasters differ in at all, and their bounding box.
fn painted(a: &[u8], b: &[u8], w: u16) -> (usize, (u16, u16, u16, u16)) {
    let (mut n, mut x0, mut y0, mut x1, mut y1) = (0usize, u16::MAX, u16::MAX, 0u16, 0u16);
    for i in 0..a.len() / 3 {
        if a[i * 3..i * 3 + 3] == b[i * 3..i * 3 + 3] {
            continue;
        }
        let (x, y) = ((i % usize::from(w)) as u16, (i / usize::from(w)) as u16);
        n += 1;
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    (n, (x0, y0, x1, y1))
}

/// `compare`, restricted to a disc of `r` pixels about `(cx, cy)` in the raster — the
/// wrap is *not* followed, because the fixture puts the hunter well away from it.
fn compare_near(
    world: &World,
    w: u16,
    h: u16,
    cx: f64,
    cy: f64,
    r: f64,
    bilinear: bool,
) -> (f64, f64, u8) {
    let view = world.render_view();
    let shape = WorldShape::new(view.topology, view.scale);
    let options = cubarium::sink::GpuSinkOptions {
        target: GpuTargetKind::Headless,
        bend_substep: Some(false),
        filter_bilinear: bilinear,
        ..Default::default()
    };
    let mut sink = GpuSink::new(shape, &art(), options).expect("a GPU");
    sink.observe_world(&view, &world.hunter_view(), &[]);
    sink.observe_view(&view, present_seconds(view.tick, 0.0), 0.0).expect("render");
    let gpu = sink.read_raster().expect("read the raster back");
    let (_, _, cpu) = cpu_raster(world, 0.0);
    let (mut total, mut n, mut over, mut worst) = (0u64, 0usize, 0usize, 0u8);
    for y in 0..u32::from(h) {
        for x in 0..u32::from(w) {
            if (f64::from(x) + 0.5 - cx).hypot(f64::from(y) + 0.5 - cy) > r {
                continue;
            }
            let i = (y * u32::from(w) + x) as usize;
            let mut pixel_worst = 0u8;
            for c in 0..3 {
                let d = gpu[i * 4 + c].abs_diff(cpu[i * 3 + c]);
                total += u64::from(d);
                pixel_worst = pixel_worst.max(d);
            }
            worst = worst.max(pixel_worst);
            n += 1;
            if pixel_worst > 8 {
                over += 1;
            }
        }
    }
    (total as f64 / (n * 3) as f64, over as f64 / n as f64, worst)
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

/// **A hunter on the GPU.** GS-1b wrote the rig path and never drew one: no world in that
/// package had a hunter profile, so `Lanternjaw::parts_living` → one instance per part
/// through the scratch page had compiled and never rasterised.
///
/// The one known difference between the two is the compositing: `stamp_rig_scaled` puts
/// the whole rig through a **single** query, so a texel two parts both reach is resolved
/// once, while the GPU draws part over part and lets the blender resolve it. GS-1b argued
/// the art cannot express the difference because every Lanternjaw texel is opaque or
/// clear. This is the measurement that argument was missing: the same frame, with a
/// hunter in it, at `--gpu-filter bilinear` — where *nothing* else is left between the
/// two renderers — compared against the same world's number without one.
#[test]
fn the_rig_composites_part_over_part_as_the_cpus_single_query_does() {
    let Some(world) = hunter_ring_world(320, 180, 1.0, TICKS) else { return };
    let hunters = world.hunter_view();
    assert!(!hunters.is_empty(), "the trial placed no hunter");
    // Where it is and how big, so the comparison below can be restricted to it: a mean
    // over the whole 320x180 raster would drown one body in 57,600 pixels.
    let (hx, hy, hr) = {
        let h = &hunters[0];
        let o = world
            .render_view()
            .organisms
            .iter()
            .find(|o| o.id == h.id)
            .expect("the hunter is in the view")
            .clone();
        // The rig's own reach: the furthest lobe centre plus its radius, with the
        // stamp's own footprint on top.
        let reach = o
            .lobes
            .iter()
            .map(|(x, y, r)| x.hypot(*y) + r)
            .fold(0.0f64, f64::max);
        (o.pos.u, o.pos.v, reach + 4.0)
    };
    println!("the hunter is at ({hx:.1}, {hy:.1}) with a {hr:.1} px neighbourhood");
    let Some((mean, over, worst, dropped)) =
        compare_world(&world, 320, 180, 1.0, 0.0, true, "hunter-")
    else {
        return;
    };
    println!(
        "bilinear, {TICKS} ticks, {} hunter(s): mean |Δ| {mean:.3} per channel, \
         {:.2}% of pixels off by more than 8, worst {worst}, {dropped} slot(s) dropped",
        hunters.len(),
        over * 100.0
    );
    // The same thresholds the hunterless comparison is held to. A rig that composited
    // differently, landed at a different anchor, or lost a part would not clear them:
    // the Lanternjaw is the largest single thing in the picture.
    assert!(mean <= 1.5, "mean |Δ| {mean:.3} per channel with a hunter");
    assert!(over <= 0.04, "{:.2}% of pixels off by more than 8", over * 100.0);

    // Restricted to the hunter itself. This is the number the compositing argument is
    // actually about: whatever the rest of the picture does, the pixels the rig paints
    // have to be the rig's.
    let (near, near_over, near_worst) = compare_near(&world, 320, 180, hx, hy, hr, true);
    println!(
        "bilinear, the hunter's own {hr:.0} px neighbourhood: mean |Δ| {near:.3} per channel, \
         {:.2}% off by more than 8, worst {near_worst}",
        near_over * 100.0
    );
    assert!(near <= 2.0, "mean |Δ| {near:.3} on the rig itself");

    // **The rig is drawn at all**, which is the gap GS-1b left: the path compiled and had
    // never rasterised. Withhold the hunters from the same view on each renderer and see
    // what changes. Both must paint, in the same place, and a comparable number of pixels
    // — the GPU's count differs a little because a nearest sampler paints whole texels
    // where the CPU's bilinear one also tints their neighbours.
    for (name, gpu) in [("CPU", false), ("GPU", true)] {
        let with = raster_of(&world, gpu, true, true);
        let without = raster_of(&world, gpu, true, false);
        let (n, (x0, y0, x1, y1)) = painted(&with, &without, 320);
        println!("{name}: the rig changes {n} pixels, in [{x0}..{x1}] x [{y0}..{y1}]");
        assert!(n > 100, "{name} drew no rig: {n} pixels changed");
        assert!(
            (f64::from(x0 + x1) / 2.0 - hx).abs() < 8.0
                && (f64::from(y0 + y1) / 2.0 - hy).abs() < 8.0,
            "{name} drew the rig away from the hunter: [{x0}..{x1}] x [{y0}..{y1}]"
        );
    }

    // And in the shipped pixel-art mode, which is what the panel draws.
    let Some((mean, over, worst, _)) = compare_world(&world, 320, 180, 1.0, 0.0, false, "hunter-")
    else {
        return;
    };
    println!(
        "nearest, {TICKS} ticks, {} hunter(s): mean |Δ| {mean:.3} per channel, \
         {:.2}% of pixels off by more than 8, worst {worst}",
        hunters.len(),
        over * 100.0
    );
    assert!(mean <= 25.0 && over <= 0.80);
}
