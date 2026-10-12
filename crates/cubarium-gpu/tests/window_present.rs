//! The desktop window's present pass on the GPU: `present.frag` with a [`WindowFit`]'s
//! push constants, drawn into an offscreen image and read back.
//!
//! The fit's arithmetic is tested on the CPU in `present.rs`; this is the one check that
//! the shader's window branch agrees with it — bars black, a whole upscale exactly the
//! nearest one, and any other scale the area average of the raster under each pixel. `None` from a machine with no GPU device, which is not a failure of the pass.

#![forbid(unsafe_code)]
use cubarium_gpu::{Gpu, PresentPass, present::WindowFit, wgpu_backend::headless::read_texture};
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A 4 × 2 raster whose every texel is a different colour.
fn raster_pixel(x: u32, y: u32) -> [u8; 4] {
    [
        40 + 50 * x as u8,
        30 + 100 * y as u8,
        7 * (x + 4 * y) as u8 + 1,
        255,
    ]
}

/// Draw the 4 × 2 raster into a `window`-sized image through the present pass at `fit`,
/// and read it back as RGBA rows.
fn present(gpu: &Gpu, window: (u32, u32), fit: WindowFit) -> Vec<u8> {
    let (rw, rh) = (4u32, 2u32);
    let extent = |w, h| wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let raster = gpu.texture(
        "window test raster",
        extent(rw, rh),
        wgpu::TextureDimension::D2,
        FORMAT,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    let raster_view = raster.create_view(&Default::default());
    let target = gpu.texture(
        "window test output",
        extent(window.0, window.1),
        wgpu::TextureDimension::D2,
        FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let view = target.create_view(&Default::default());
    let pixels: Vec<u8> = (0..rh)
        .flat_map(|y| (0..rw).flat_map(move |x| raster_pixel(x, y)))
        .collect();
    let mut encoder = gpu.encoder("window test present");
    gpu.upload_texture(
        &mut encoder,
        &raster,
        wgpu::Origin3d::ZERO,
        extent(rw, rh),
        4,
        &pixels,
    )
    .unwrap();
    PresentPass::new(gpu, FORMAT).record(gpu, &mut encoder, &raster_view, &view, fit.push());
    gpu.queue.submit([encoder.finish()]);
    read_texture(gpu, &target).unwrap()
}

fn open() -> Option<Gpu> {
    match Gpu::open() {
        Ok(gpu) => Some(gpu),
        Err(e) => {
            eprintln!("no GPU device here ({e}); the window's present pass was not checked");
            None
        }
    }
}

fn at(image: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    image[i..i + 4].try_into().unwrap()
}

/// 4 × 2 into 8 × 6: a whole 2× upscale, width filled, 8 × 4 centred at (0, 1) with a
/// black bar above and below; at a whole factor the area filter is the nearest upscale.
#[test]
fn the_window_pass_upscales_whole_and_paints_the_bars_black() {
    let Some(gpu) = open() else { return };
    let window = (8, 6);
    let fit = WindowFit::fit((4, 2), window, false);
    assert_eq!((fit.step, fit.size, fit.origin), (0.5, (8, 4), (0, 1)));
    let image = present(&gpu, window, fit);
    for y in 0..window.1 {
        for x in 0..window.0 {
            let want = if (1..5).contains(&y) {
                raster_pixel(x / 2, (y - 1) / 2)
            } else {
                [0, 0, 0, 255]
            };
            assert_eq!(at(&image, window.0, x, y), want, "pixel ({x}, {y})");
        }
    }
}

/// The area average `present.frag` should draw at window pixel `(x, y)`, from the fit's
/// own footprint arithmetic, or black where the footprint misses the raster.
fn expected(push: &[f32; 8], x: u32, y: u32) -> [u8; 4] {
    let span = |a: usize, p: u32, r: f64| {
        let (step, off) = (f64::from(push[a * 3]), f64::from(push[4 + a]));
        let c = step * f64::from(p) + off;
        ((c - 0.5 * step).max(0.0), (c + 0.5 * step).min(r))
    };
    let (x0, x1) = span(0, x, 4.0);
    let (y0, y1) = span(1, y, 2.0);
    if x1 <= x0 || y1 <= y0 {
        return [0, 0, 0, 255];
    }
    let mut sum = [0.0f64; 3];
    let mut total = 0.0;
    for ty in 0..2u32 {
        for tx in 0..4u32 {
            let wx = (x1.min(f64::from(tx + 1)) - x0.max(f64::from(tx))).max(0.0);
            let wy = (y1.min(f64::from(ty + 1)) - y0.max(f64::from(ty))).max(0.0);
            let c = raster_pixel(tx, ty);
            for i in 0..3 {
                sum[i] += wx * wy * f64::from(c[i]);
            }
            total += wx * wy;
        }
    }
    let ch = |i: usize| (sum[i] / total).round() as u8;
    [ch(0), ch(1), ch(2), 255]
}

/// Scales that are not whole, up and down: every pixel is the area average of the
/// raster under its footprint (in the attachment's values; a UNORM raster here), and
/// the bars are black.
#[test]
fn the_window_pass_draws_the_area_average_at_any_scale() {
    let Some(gpu) = open() else { return };
    for window in [(6, 3), (11, 8), (3, 3), (2, 1), (5, 2)] {
        let fit = WindowFit::fit((4, 2), window, false);
        let push = fit.push();
        let image = present(&gpu, window, fit);
        for y in 0..window.1 {
            for x in 0..window.0 {
                let got = at(&image, window.0, x, y);
                let want = expected(&push, x, y);
                let close = got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 1);
                assert!(close, "{window:?} pixel ({x}, {y}): {got:?}, want {want:?}");
            }
        }
    }
}
