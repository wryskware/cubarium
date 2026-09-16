//! Q1: headless wgpu — no surface, render 1920x1080 (or any size) to a texture,
//! copy to a buffer, read it back. Reports per-frame submit+wait and map+copy.
use std::time::Instant;

use anyhow::{anyhow, Result};
use wgpu::util::DeviceExt as _;

const SHADER: &str = r#"
struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32(i & 1u), f32((i >> 1u) & 1u));
    var o: VsOut;
    o.uv = uv;
    o.pos = vec4<f32>(uv * 1.6 - vec2<f32>(0.8, 0.8), 0.0, 1.0);
    return o;
}
@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var s: sampler;
@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> { return textureSample(t, s, in.uv); }
"#;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let w: u32 = args.next().unwrap_or_else(|| "1920".into()).parse()?;
    let h: u32 = args.next().unwrap_or_else(|| "1080".into()).parse()?;
    let frames: usize = args.next().unwrap_or_else(|| "60".into()).parse()?;

    let instance = wgpu::Instance::default();
    for a in instance.enumerate_adapters(wgpu::Backends::all()) {
        let i = a.get_info();
        println!("adapter: {:?} {} backend={:?} driver={} {}", i.device_type, i.name, i.backend, i.driver, i.driver_info);
    }
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .map_err(|e| anyhow!("no adapter: {e}"))?;
    println!("chosen: {:?}", adapter.get_info());

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

    // 256x256 checker texture for the quad.
    let tw = 256u32;
    let mut texels = vec![0u8; (tw * tw * 4) as usize];
    for y in 0..tw {
        for x in 0..tw {
            let i = ((y * tw + x) * 4) as usize;
            let c = if ((x / 16) + (y / 16)) % 2 == 0 { 255 } else { 40 };
            texels[i] = c;
            texels[i + 1] = (x * 255 / tw) as u8;
            texels[i + 2] = (y * 255 / tw) as u8;
            texels[i + 3] = 255;
        }
    }
    let tex = device.create_texture_with_data(
        &queue,
        &wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: tw, height: tw, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &texels,
    );
    let tex_view = tex.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
            compilation_options: Default::default(),
        }),
        multiview: None,
        cache: None,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&tex_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
        ],
    });

    let bpr = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (bpr * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut render_ms = Vec::new();
    let mut read_ms = Vec::new();
    let mut out = vec![0u8; (w * h * 4) as usize];

    for f in 0..frames {
        let t0 = Instant::now();
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.02, b: 0.12, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&pipeline);
            rp.set_bind_group(0, &bind, &[]);
            rp.draw(0..4, 0..1);
        }
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit([enc.finish()]);
        device.poll(wgpu::PollType::wait_indefinitely())?;
        let t1 = Instant::now();

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
        device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        {
            let view = slice.get_mapped_range();
            for y in 0..h as usize {
                let src = y * bpr as usize;
                let dst = y * (w * 4) as usize;
                out[dst..dst + (w * 4) as usize].copy_from_slice(&view[src..src + (w * 4) as usize]);
            }
        }
        readback.unmap();
        let t2 = Instant::now();

        if f >= 5 {
            render_ms.push((t1 - t0).as_secs_f64() * 1e3);
            read_ms.push((t2 - t1).as_secs_f64() * 1e3);
        }
    }

    // Prove it drew something: sample the centre and a corner.
    let px = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        (out[i], out[i + 1], out[i + 2], out[i + 3])
    };
    println!("corner(4,4)={:?} centre={:?}", px(4, 4), px(w / 2, h / 2));
    stat("gpu render+copy (submit..fence)", &render_ms);
    stat("map + memcpy readback", &read_ms);
    Ok(())
}

fn stat(name: &str, v: &[f64]) {
    if v.is_empty() { return; }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = s.iter().sum::<f64>() / s.len() as f64;
    println!("{name}: n={} min={:.2} p50={:.2} p95={:.2} max={:.2} mean={:.2} ms",
        s.len(), s[0], s[s.len() / 2], s[(s.len() * 95 / 100).min(s.len() - 1)], s[s.len() - 1], mean);
}
