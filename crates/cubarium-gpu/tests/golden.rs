//! The golden image: the synthetic ring at a fixed instant, rendered to a readback
//! buffer and compared with a stored PNG.
//!
//! This is the regression that says the *picture* did not change — the one thing a
//! renderer's unit tests cannot say. It runs on the desktop; re-generate the reference
//! with
//!
//! ```text
//! cargo run --release -p cubarium-gpu --example synthetic -- \
//!     --art assets/atelier --frames 1 --at 7.5 \
//!     --png crates/cubarium-gpu/tests/golden/synthetic-320x180-s1.png
//! ```
//!
//! **The tolerance is not zero and cannot be, and it is measured rather than guessed.**
//! Rendering this exact scene on the Tachyon's Adreno 643 and on the desktop's RTX 5090
//! gives mean |Δ| **0.116** per channel, with 11 % of channels off by exactly 1 (the
//! sRGB encode's last bit) and **0.05 %** of pixels off by more than 8. The bounds below
//! are four times that, which leaves a wrong colour, a lost pass or a shifted sprite
//! nowhere to hide.

use cubarium_gpu::atlas::Atlas;
use cubarium_gpu::render::Renderer;
use cubarium_gpu::scene::RingLayout;
use cubarium_gpu::synthetic::SyntheticWorld;
use cubarium_gpu::target::{Headless, read_png, write_png};
use cubarium_gpu::vk::Gpu;

/// The instant the golden image is taken at, in presentation seconds.
const AT: f64 = 7.5;
/// The tick rate the presentation clock runs against.
const TICK_HZ: f64 = 20.0;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Render the synthetic ring at [`AT`]. `None` when this machine has no Vulkan device
/// at all, which is not a failure of the renderer.
fn render(layout: RingLayout) -> Option<Vec<u8>> {
    let atlas = Atlas::load(&root().join("../../assets/atelier")).expect("load assets/atelier");
    let gpu = match Gpu::open(&[]) {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("no Vulkan device here ({e}); the golden image was not checked");
            return None;
        }
    };
    let mut renderer = Renderer::new(&gpu, &atlas, layout).expect("build the renderer");
    let mut world = SyntheticWorld::new(layout, 1);
    let mut headless = Headless::new(&gpu, &renderer).expect("headless target");
    let tick = (AT * TICK_HZ).floor() as u64 + 1;
    let f = (AT * TICK_HZ).fract();
    let seconds = |phase: f64| (tick as f64 - 1.0 + phase) / TICK_HZ;
    world.tick(tick, seconds(0.0));
    let scene = world.frame(&atlas, seconds(f), f as f32);
    headless.draw(&gpu, &mut renderer, scene).expect("draw");
    let rgba = headless.read(&gpu, &renderer).expect("read back");
    headless.destroy(&gpu);
    renderer.destroy(&gpu);
    Some(rgba)
}

/// `(mean absolute channel difference, share of channels off by more than 8, worst)`.
fn compare(got: &[u8], want: &[u8]) -> (f64, f64, u8) {
    let mut total = 0u64;
    let mut over_eight = 0usize;
    let mut worst = 0u8;
    for (a, b) in got.iter().zip(want.iter()) {
        let d = a.abs_diff(*b);
        total += u64::from(d);
        worst = worst.max(d);
        if d > 8 {
            over_eight += 1;
        }
    }
    (
        total as f64 / got.len() as f64,
        over_eight as f64 / got.len() as f64,
        worst,
    )
}

#[test]
fn the_synthetic_ring_at_a_fixed_instant_is_the_stored_image() {
    let layout = RingLayout::RING_320;
    let Some(rgba) = render(layout) else { return };
    let reference = root().join("tests/golden/synthetic-320x180-s1.png");
    if !reference.exists() {
        write_png(&reference, layout.w, layout.h, &rgba).expect("write the first reference");
        panic!("wrote a new reference at {}; review it and re-run", reference.display());
    }
    let (w, h, want) = read_png(&reference).expect("read the reference");
    assert_eq!((w, h), (layout.w, layout.h), "the reference is a different size");

    let (mean, over, worst) = compare(&rgba, &want);
    if mean > 0.5 || over > 0.001 {
        let got = root().join("tests/golden/synthetic-320x180-s1.actual.png");
        write_png(&got, w, h, &rgba).ok();
        panic!(
            "the picture changed: mean |Δ| {mean:.3} (≤ 0.5), {:.3}% of channels off by more \
             than 8 (≤ 0.1%), worst {worst}. The render is at {}",
            over * 100.0,
            got.display()
        );
    }
    println!("mean |Δ| {mean:.3}, {:.3}% over 8, worst {worst}", over * 100.0);
}

#[test]
fn the_same_scene_at_scale_two_is_the_stored_image() {
    // The `S = 2` rung of `flat-world-plan` §6: the same 80 x 45 cells, the same art,
    // every source texel on a 2 x 2 block. It is here as a *picture* regression on the
    // scale factor, which is the one thing that has no unit test: a shader that let the
    // two sub-columns of a block sample different texels would still pass everything
    // else and would look soft on the panel.
    let layout = RingLayout::RING_640;
    let Some(rgba) = render(layout) else { return };
    let reference = root().join("tests/golden/synthetic-640x360-s2.png");
    if !reference.exists() {
        write_png(&reference, layout.w, layout.h, &rgba).expect("write the first reference");
        panic!("wrote a new reference at {}; review it and re-run", reference.display());
    }
    let (w, h, want) = read_png(&reference).expect("read the reference");
    assert_eq!((w, h), (layout.w, layout.h));
    let (mean, over, worst) = compare(&rgba, &want);
    assert!(mean <= 0.5 && over <= 0.001, "mean {mean:.3}, {:.3}% over 8, worst {worst}", over * 100.0);
}

#[test]
fn every_sprite_texel_covers_a_whole_scale_by_scale_block() {
    // The pixel-art rule, checked on the image rather than argued from the shader.
    //
    // One stamp of one frame on an empty raster, at S = 1 and again at S = 2 with
    // everything doubled. Nearest sampling makes two claims that filtering would break:
    // the set of colours the two renders paint is *identical* (nothing is interpolated
    // into existence), and every colour covers exactly four times as many pixels at
    // S = 2 as at S = 1 (every source texel is one S x S block, whole).
    //
    // The raster is kept entirely above the horizon and its fields are all zero, so the
    // background is the flat night floor and every other colour is the sprite's.
    let atlas = Atlas::load(&root().join("../../assets/atelier")).expect("load assets/atelier");
    let gpu = match Gpu::open(&[]) {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("no Vulkan device here ({e}); the pixel grid was not checked");
            return;
        }
    };
    let clip = atlas.plant("lanternstalk", cubarium_gpu::atlas::PlantClip::Stage(2)).unwrap();
    let rect = atlas.rect(clip.first);
    let histogram = |scale: u32| {
        let layout = RingLayout { w: 64 * scale, h: 64 * scale, scale };
        let mut renderer = Renderer::new(&gpu, &atlas, layout).expect("renderer");
        let mut headless = Headless::new(&gpu, &renderer).expect("headless");
        let mut scene = cubarium_gpu::scene::Scene::new(layout);
        scene.fields.producer_max = 1.0;
        scene.fields.producer = vec![0.0; layout.cell_count()];
        scene.fields.water = vec![0.0; layout.cell_count()];
        scene.fields.detritus = vec![0.0; layout.cell_count()];
        scene.fields.revision = 1;
        scene.push(
            cubarium_gpu::scene::Layer::Plants,
            cubarium_gpu::scene::SpriteInstance {
                anchor: [32.0 * scale as f32, 20.0 * scale as f32],
                heading: [1.0, 0.0],
                frames: [[rect.x, rect.y], [0, 0], [0, 0], [0, 0]],
                size: [rect.w, rect.h],
                pivot: [rect.w / 2, rect.h / 2],
                weights: [1.0, 0.0, 0.0, 0.0],
                opacity: 1.0,
                ..Default::default()
            },
        );
        headless.draw(&gpu, &mut renderer, &scene).expect("draw");
        let rgba = headless.read(&gpu, &renderer).expect("read back");
        headless.destroy(&gpu);
        renderer.destroy(&gpu);
        // Only the stamp's own 16 x 16 source tile, so the background's horizon
        // gradient — which is a per-pixel ramp at both scales and has no reason to be
        // blocky — stays out of the histogram. The anchor (32 S, 20 S) snaps to
        // 32 S + 0.5, so the tile covers pixels 24 S + 1 ..= 40 S across.
        let mut counts = std::collections::BTreeMap::new();
        for y in (12 * scale + 1)..=(28 * scale) {
            for x in (24 * scale + 1)..=(40 * scale) {
                let i = ((y * layout.w + x) * 4) as usize;
                *counts.entry([rgba[i], rgba[i + 1], rgba[i + 2]]).or_insert(0usize) += 1;
            }
        }
        counts
    };
    let one = histogram(1);
    let two = histogram(2);
    assert!(one.len() >= 5, "the fixture painted almost nothing: {} colours", one.len());
    let colours: Vec<_> = one.keys().copied().collect();
    assert_eq!(
        colours,
        two.keys().copied().collect::<Vec<_>>(),
        "S = 2 paints a different set of colours: something is filtering"
    );
    // The floor covers the rest of a four-times-larger raster, so it is excluded; every
    // colour the sprite paints must be exactly four times as common.
    let floor = *one.iter().max_by_key(|(_, n)| **n).unwrap().0;
    for colour in &colours {
        if *colour == floor {
            continue;
        }
        assert_eq!(
            two[colour],
            one[colour] * 4,
            "{colour:?} covers {} pixels at S = 2 and {} at S = 1, not four times as many",
            two[colour],
            one[colour]
        );
    }
    println!("{} sprite colours, each exactly 4x as large at S = 2", colours.len() - 1);
}

#[test]
fn the_ring_wraps_across_the_seam_rather_than_cutting_at_it() {
    // A stamp that straddles x = 0 is pushed twice by `Scene::push`, so the two rim
    // columns must carry as much light as any other adjacent pair. A renderer that
    // dropped the wrap would leave a dark line exactly one sprite wide at the seam.
    let layout = RingLayout::RING_320;
    let Some(rgba) = render(layout) else { return };
    let luma = |x: u32, y: u32| {
        let i = ((y * layout.w + x) * 4) as usize;
        f64::from(rgba[i]) * 0.2126 + f64::from(rgba[i + 1]) * 0.7152 + f64::from(rgba[i + 2]) * 0.0722
    };
    let column = |x: u32| (0..layout.h).map(|y| luma(x, y)).sum::<f64>() / f64::from(layout.h);
    let seam = (column(0) - column(layout.w - 1)).abs();
    let inner: f64 = (1..layout.w - 1)
        .map(|x| (column(x) - column(x - 1)).abs())
        .fold(0.0f64, f64::max);
    assert!(
        seam <= inner,
        "the seam column steps by {seam:.2}, more than the worst interior step {inner:.2}"
    );
}
