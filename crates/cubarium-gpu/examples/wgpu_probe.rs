//! Headless migration probe: validate every existing shader, then exercise the
//! safe present/readback path on an asymmetric, non-aligned texture.
#![forbid(unsafe_code)]

use anyhow::{Context, Result, ensure};
use cubarium_gpu::wgpu_backend::{Gpu, Headless, PresentPass};

fn main() -> Result<()> {
    let gpu = Gpu::open()?;
    println!(
        "adapter: {} ({:?})",
        gpu.name,
        gpu.adapter.get_info().backend
    );
    let shaders = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders");
    let mut paths = std::fs::read_dir(shaders)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.retain(|p| p.extension().is_some_and(|e| e == "spv"));
    paths.sort();
    let mut failed = 0;
    for path in &paths {
        let bytes = std::fs::read(path)?;
        let name = path
            .file_name()
            .context("shader filename")?
            .to_string_lossy();
        let result = probe_shader(&gpu, &name, &bytes);
        match result {
            Ok(()) => println!("PASS shader {name}"),
            Err(error) => {
                failed += 1;
                println!("FAIL shader {name}: {error:#}");
            }
        }
    }
    println!(
        "shader modules: {} passed, {failed} failed",
        paths.len() - failed
    );
    present_smoke(&gpu)?;
    ensure!(
        failed == 0,
        "{failed} shader modules require translation before the full renderer port"
    );
    Ok(())
}

fn probe_shader(gpu: &Gpu, name: &str, bytes: &[u8]) -> Result<()> {
    let module = wgpu::naga::front::spv::parse_u8_slice(bytes, &Default::default())
        .context("Naga SPIR-V parsing")?;
    wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .context("Naga validation")?;
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let _shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::util::make_spirv(bytes),
        });
    if let Some(error) = pollster::block_on(scope.pop()) {
        anyhow::bail!("wgpu validation: {error}");
    }
    Ok(())
}

fn present_smoke(gpu: &Gpu) -> Result<()> {
    let size = (13, 7);
    let extent = wgpu::Extent3d {
        width: size.0,
        height: size.1,
        depth_or_array_layers: 1,
    };
    let source = gpu.texture(
        "probe source",
        extent,
        wgpu::TextureDimension::D2,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let source_view = source.create_view(&Default::default());
    let mut output = Headless::new(gpu, size, 2)?;
    let present = PresentPass::new(gpu, wgpu::TextureFormat::Rgba8UnormSrgb);
    let pixels: Vec<u8> = (0..size.1)
        .flat_map(|y| {
            (0..size.0)
                .flat_map(move |x| [(x * 19) as u8, (y * 31) as u8, ((x + y) * 11) as u8, 255])
        })
        .collect();
    let identity = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    // Recording a different upload and dropping it must not change the submitted
    // picture. Both upload and transform data are owned by their command recordings.
    for tag in [11, 12] {
        let mut encoder = gpu.encoder("present probe");
        gpu.upload_texture(
            &mut encoder,
            &source,
            wgpu::Origin3d::ZERO,
            extent,
            4,
            &pixels,
        )?;
        present.record(gpu, &mut encoder, &source_view, &output.view, identity);
        ensure!(
            output.readback.submit(gpu, encoder, &output.texture, tag)?,
            "probe readback slot unavailable"
        );
    }
    ensure!(
        !output.readback.can_submit(),
        "full ring offered another slot"
    );
    let mut tags = Vec::new();
    let mut mismatch = None;
    output.readback.drain(gpu, |tag, bytes| {
        tags.push(tag);
        if bytes != pixels {
            mismatch = Some(tag);
        }
    })?;
    ensure!(tags == [11, 12], "readback tags reordered: {tags:?}");
    ensure!(
        mismatch.is_none(),
        "present/readback changed source texels in frame {mismatch:?}"
    );
    ensure!(
        output.readback.can_submit(),
        "retired readback slot not reusable"
    );
    let mut discarded = gpu.encoder("discarded probe");
    gpu.upload_texture(
        &mut discarded,
        &source,
        wgpu::Origin3d::ZERO,
        extent,
        4,
        &vec![0; pixels.len()],
    )?;
    drop(discarded);
    let mut encoder = gpu.encoder("reused readback slot");
    present.record(gpu, &mut encoder, &source_view, &output.view, identity);
    ensure!(
        output.readback.submit(gpu, encoder, &output.texture, 13)?,
        "retired slot unavailable"
    );
    output.readback.drain(gpu, |tag, bytes| {
        tags.push(tag);
        if bytes != pixels {
            mismatch = Some(tag);
        }
    })?;
    ensure!(
        tags == [11, 12, 13] && mismatch.is_none(),
        "reused readback slot corrupted frame"
    );
    println!(
        "PASS present/readback: 13x7 RGBA, padded rows, ordered tags, ring reuse, discarded upload"
    );
    Ok(())
}
