//! Repeated safe device/resource lifecycle diagnostic; no windows or renderer caches.
#![forbid(unsafe_code)]
use anyhow::{Result, ensure};
use cubarium_gpu::wgpu_backend::{Gpu, ReadbackRing};
fn main() -> Result<()> {
    let cycles: u32 = std::env::args().nth(1).as_deref().unwrap_or("16").parse()?;
    ensure!(cycles > 0, "positive cycle count required");
    memory("before");
    for cycle in 0..cycles {
        let start = std::time::Instant::now();
        {
            let gpu = Gpu::open()?;
            if cycle == 0 {
                memory("first-device-open");
            }
            if cycle == 0 {
                println!(
                    "adapter={:?} requested={:?}",
                    gpu.adapter.get_info(),
                    gpu.device.limits()
                );
            }
            let texture = gpu.texture(
                "lifecycle pixel",
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                wgpu::TextureDimension::D2,
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
            );
            let mut encoder = gpu.encoder("lifecycle upload");
            let pixel = [cycle as u8, 37, 129, 255];
            gpu.upload_texture(
                &mut encoder,
                &texture,
                wgpu::Origin3d::ZERO,
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                4,
                &pixel,
            )?;
            let mut ring = ReadbackRing::new(&gpu, (1, 1), 1)?;
            ring.submit(&gpu, encoder, &texture, cycle as u64)?;
            let mut seen = false;
            ring.drain(&gpu, |tag, bytes| {
                assert_eq!(tag, cycle as u64);
                assert_eq!(bytes, pixel);
                seen = true;
            })?;
            ensure!(seen, "pixel never completed");
            if cycle == 0 {
                memory("first-resource-complete");
            }
        }
        println!(
            "cycle={} open/upload/readback/drop_ms={:.3}",
            cycle + 1,
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
    memory("after-drop");
    println!("PASS {cycles} device lifecycles");
    Ok(())
}

// Optional diagnostic telemetry, never a production/runtime dependency.
fn memory(stage: &str) {
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args([
            "--query-compute-apps=pid,used_gpu_memory,gpu_uuid",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        let prefix = format!("{},", std::process::id());
        let rows = String::from_utf8_lossy(&output.stdout);
        let own: Vec<_> = rows
            .lines()
            .filter(|row| row.starts_with(&prefix))
            .collect();
        println!(
            "memory {stage} own_pid={} allocations={own:?}",
            std::process::id()
        );
    }
}
