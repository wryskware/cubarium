//! Headless shader probe: validate every WGSL shader (Naga, MSL and HLSL output, a real
//! pipeline on this adapter) across the override cases, then exercise the safe
//! present/readback path on an asymmetric, non-aligned texture.
#![forbid(unsafe_code)]

use anyhow::{Context, Result, ensure};
use cubarium_gpu::wgpu_backend::shader::{self, Shader, Specialization};
use cubarium_gpu::wgpu_backend::{Gpu, Headless, PresentPass};

fn main() -> Result<()> {
    let gpu = Gpu::open()?;
    println!(
        "adapter: {} ({:?})",
        gpu.name,
        gpu.adapter.get_info().backend
    );
    let mut failed = 0;
    let mut cases: Vec<_> = shader::ALL
        .into_iter()
        .map(|shader| (shader, Specialization::default()))
        .collect();
    let mut lit = Specialization::default();
    lit.0[0] = true;
    lit.0[3] = true;
    lit.0[10] = true;
    for shader in [
        Shader::Voxel,
        Shader::Weather,
        Shader::FigureFragment,
        Shader::PlantVis,
        Shader::PlantShade,
    ] {
        cases.push((shader, lit));
    }
    for id in 0..12 {
        let mut alternate = lit;
        alternate.0[id] = !alternate.0[id];
        cases.push((Shader::Voxel, alternate));
    }
    for (shader, specialization) in &cases {
        match wgsl_probe(&gpu, *shader, *specialization) {
            Ok(()) => println!("PASS WGSL {:?} {:?}", shader, specialization.0),
            Err(error) => {
                failed += 1;
                println!("FAIL WGSL {:?} {:?}: {error:#}", shader, specialization.0);
            }
        }
    }
    println!(
        "WGSL cases: {} passed, {failed} failed",
        cases.len() - failed
    );
    present_smoke(&gpu)?;
    ensure!(
        failed == 0,
        "{failed} WGSL cases failed"
    );
    Ok(())
}

fn wgsl_probe(gpu: &Gpu, shader: Shader, specialization: Specialization) -> Result<()> {
    let source = shader.runtime_source().to_owned();
    let module = naga::front::wgsl::parse_str(&source).context("WGSL runtime parsing")?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .context("Naga WGSL validation")?;
    // The diagnostic must collect Linux results even if an external native
    // emitter panics. Production rendering never calls these emitters directly.
    let native = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        shader::native_translation(&module, &info, specialization)
    }))
    .unwrap_or_else(|panic| {
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or("unknown native-emitter panic");
        Err(anyhow::anyhow!("native emitter panicked: {message}"))
    })
    .context("MSL/HLSL translation (no native runtime)");
    match &native {
        Ok((msl, hlsl)) => println!(
            "  PASS native source {}: MSL {msl} B, HLSL {hlsl} B",
            shader.name()
        ),
        Err(error) => println!("  FAIL native source {}: {error:#}", shader.name()),
    }
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    if shader.stage() == naga::ShaderStage::Fragment {
        pipeline_probe(gpu, shader, specialization, module, source)?;
    } else {
        let _module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(shader.name()),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(source)),
            });
    }
    if let Some(error) = pollster::block_on(scope.pop()) {
        anyhow::bail!("Linux wgpu pipeline/module validation: {error}");
    }
    println!("  PASS Linux wgpu pipeline/module {}", shader.name());
    native?;
    Ok(())
}

fn pipeline_probe(
    gpu: &Gpu,
    shader: Shader,
    specialization: Specialization,
    fragment: naga::Module,
    fragment_source: String,
) -> Result<()> {
    let vertex = match shader {
        Shader::SpriteFragment => Shader::SpriteVertex,
        Shader::FigureFragment => Shader::FigureVertex,
        Shader::CompositeFragment => Shader::CompositeVertex,
        Shader::FigureSunFragment => Shader::FigureSunVertex,
        Shader::PlantVis => Shader::PlantVertex,
        _ => Shader::Fullscreen,
    };
    let vertex_source = vertex.runtime_source().to_owned();
    let vertex_ir = naga::front::wgsl::parse_str(&vertex_source)?;
    let vertex_constants = shader::constants(&vertex_ir, specialization);
    let fragment_constants = shader::constants(&fragment, specialization);
    let vertex_constants: Vec<_> = vertex_constants
        .iter()
        .map(|(key, value)| (key.as_str(), *value))
        .collect();
    let fragment_constants: Vec<_> = fragment_constants
        .iter()
        .map(|(key, value)| (key.as_str(), *value))
        .collect();
    let entry = vertex_ir
        .entry_points
        .first()
        .context("vertex entrypoint")?;
    let mut inputs = Vec::new();
    for argument in &entry.function.arguments {
        locations(
            &vertex_ir,
            argument.ty,
            argument.binding.as_ref(),
            &mut inputs,
        );
    }
    inputs.sort_by_key(|&(location, _)| location);
    let attributes: Vec<_> = inputs
        .iter()
        .enumerate()
        .map(|(i, &(location, ty))| {
            Ok(wgpu::VertexAttribute {
                format: vertex_format(&vertex_ir.types[ty].inner)?,
                offset: i as u64 * 16,
                shader_location: location,
            })
        })
        .collect::<Result<_>>()?;
    let buffers = if attributes.is_empty() {
        vec![]
    } else {
        vec![Some(wgpu::VertexBufferLayout {
            array_stride: attributes.len() as u64 * 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &attributes,
        })]
    };
    let mut outputs = Vec::new();
    if let Some(result) = &fragment
        .entry_points
        .first()
        .context("fragment entrypoint")?
        .function
        .result
    {
        locations(&fragment, result.ty, result.binding.as_ref(), &mut outputs);
    }
    let mut targets = vec![
        None;
        outputs
            .iter()
            .map(|&(location, _)| location as usize + 1)
            .max()
            .unwrap_or(0)
    ];
    for (location, ty) in outputs {
        let format = match fragment.types[ty].inner {
            naga::TypeInner::Scalar(naga::Scalar {
                kind: naga::ScalarKind::Uint,
                ..
            }) => wgpu::TextureFormat::R32Uint,
            naga::TypeInner::Scalar(_) => wgpu::TextureFormat::R32Float,
            _ => wgpu::TextureFormat::Rgba8UnormSrgb,
        };
        targets[location as usize] = Some(wgpu::ColorTargetState {
            format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        });
    }
    let vs = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(vertex.name()),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(vertex_source)),
        });
    let fs = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(shader.name()),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(fragment_source)),
        });
    let _pipeline =
        gpu.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(shader.name()),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &vs,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &vertex_constants,
                        ..Default::default()
                    },
                    buffers: &buffers,
                },
                primitive: Default::default(),
                depth_stencil: (targets.is_empty() || shader == Shader::CompositeFragment)
                    .then_some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: Some(true),
                        depth_compare: Some(if targets.is_empty() {
                            wgpu::CompareFunction::Greater
                        } else {
                            wgpu::CompareFunction::LessEqual
                        }),
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &fs,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &fragment_constants,
                        ..Default::default()
                    },
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            });
    Ok(())
}

fn locations(
    module: &naga::Module,
    ty: naga::Handle<naga::Type>,
    binding: Option<&naga::Binding>,
    out: &mut Vec<(u32, naga::Handle<naga::Type>)>,
) {
    if let Some(naga::Binding::Location { location, .. }) = binding {
        out.push((*location, ty));
    } else if let naga::TypeInner::Struct { members, .. } = &module.types[ty].inner {
        for member in members {
            locations(module, member.ty, member.binding.as_ref(), out);
        }
    }
}

fn vertex_format(ty: &naga::TypeInner) -> Result<wgpu::VertexFormat> {
    let (kind, size) = match ty {
        naga::TypeInner::Scalar(scalar) => (scalar.kind, 1),
        naga::TypeInner::Vector { scalar, size } => (scalar.kind, *size as u8),
        _ => anyhow::bail!("unsupported probe vertex input {ty:?}"),
    };
    Ok(match (kind, size) {
        (naga::ScalarKind::Float, 1) => wgpu::VertexFormat::Float32,
        (naga::ScalarKind::Float, 2) => wgpu::VertexFormat::Float32x2,
        (naga::ScalarKind::Float, 3) => wgpu::VertexFormat::Float32x3,
        (naga::ScalarKind::Float, 4) => wgpu::VertexFormat::Float32x4,
        (naga::ScalarKind::Uint, 1) => wgpu::VertexFormat::Uint32,
        (naga::ScalarKind::Uint, 2) => wgpu::VertexFormat::Uint32x2,
        (naga::ScalarKind::Uint, 3) => wgpu::VertexFormat::Uint32x3,
        (naga::ScalarKind::Uint, 4) => wgpu::VertexFormat::Uint32x4,
        _ => anyhow::bail!("unsupported probe vertex scalar {kind:?} size {size}"),
    })
}

fn present_smoke(gpu: &Gpu) -> Result<()> {
    let module = naga::front::wgsl::parse_str(include_str!("../shaders/present.wgsl"))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)?;
    let (msl, hlsl) = shader::native_translation(&module, &info, Specialization::default())?;
    println!("PASS portable present source: MSL {msl} bytes, HLSL {hlsl} bytes");
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
