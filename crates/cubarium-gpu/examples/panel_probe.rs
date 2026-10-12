//! A few headless frames verify panel rotation, integer pixels and sRGB encoding.
#![forbid(unsafe_code)]

use anyhow::{Result, ensure};
use cubarium_gpu::render::PresentTransform;
use cubarium_gpu::wgpu_backend::{Gpu, PresentPass, ReadbackRing};

fn main() -> Result<()> {
    let gpu = Gpu::open()?;
    let raster = (3, 2);
    let target = (8, 8);
    let extent = |size: (u32, u32)| wgpu::Extent3d {
        width: size.0, height: size.1, depth_or_array_layers: 1,
    };
    let source = gpu.texture("panel probe source", extent(raster), wgpu::TextureDimension::D2,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING);
    let pixels = [
        10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255,
        100, 110, 120, 255, 130, 140, 150, 255, 160, 170, 180, 255,
    ];
    let mut upload = gpu.encoder("panel probe source upload");
    gpu.upload_texture(&mut upload, &source, wgpu::Origin3d::ZERO, extent(raster), 4, &pixels)?;
    gpu.queue.submit([upload.finish()]);
    let source_view = source.create_view(&Default::default());
    let mut checked = 0;
    for encode in [false, true] {
        let format = if encode { wgpu::TextureFormat::Rgba8Unorm } else { wgpu::TextureFormat::Rgba8UnormSrgb };
        let output = gpu.texture("panel probe output", extent(target), wgpu::TextureDimension::D2,
            format, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let view = output.create_view(&Default::default());
        let pass = PresentPass::new(&gpu, format);
        let mut readback = ReadbackRing::new(&gpu, target, 2)?;
        for rotation in 0..4 {
            let transform = PresentTransform::fit(raster, target, rotation, encode).unwrap();
            let mut encoder = gpu.encoder("panel probe frame");
            pass.record(&gpu, &mut encoder, &source_view, &view,
                PresentPass::panel_transform(transform, target));
            ensure!(readback.submit(&gpu, encoder, &output, rotation as u64)?, "probe ring full");
            let mut mismatch = None;
            readback.drain(&gpu, |_, bytes| {
                for y in 0..target.1 {
                    for x in 0..target.0 {
                        let (sx, sy) = match rotation {
                            0 => (x, y),
                            1 => (y, target.0 - 1 - x),
                            2 => (target.0 - 1 - x, target.1 - 1 - y),
                            _ => (target.1 - 1 - y, x),
                        };
                        let (sx, sy) = (sx / transform.factor, sy / transform.factor);
                        let expected = if sx < raster.0 && sy < raster.1 {
                            let i = ((sy * raster.0 + sx) * 4) as usize;
                            &pixels[i..i + 4]
                        } else { &[0, 0, 0, 255] };
                        let i = ((y * target.0 + x) * 4) as usize;
                        if &bytes[i..i + 4] != expected && mismatch.is_none() {
                            mismatch = Some((x, y, bytes[i..i + 4].to_vec(), expected.to_vec()));
                        }
                    }
                }
            })?;
            ensure!(mismatch.is_none(), "rotation {rotation}, shader sRGB {encode}: {mismatch:?}");
            checked += 1;
        }
    }
    println!("PASS panel mapping: {checked} frames, four quarter-turns, integer upscaling, black margins, attachment/shader sRGB");
    Ok(())
}
