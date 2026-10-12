//! Maintained WGSL runtime shaders and native portability checks. Embedded GLSL
//! is retained only as input to the optional one-time conversion tool.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

pub const NEAREST_BINDING: u32 = 32;
pub const LINEAR_BINDING: u32 = 33;

/// The twelve `world.glsl` specialization constants, indexed by their existing ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Specialization(pub [bool; 12]);

impl Default for Specialization {
    fn default() -> Self {
        Self([
            false, true, true, false, true, true, false, false, false, false, false, true,
        ])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shader {
    Background,
    Water,
    SpriteVertex,
    SpriteFragment,
    Present,
    Fullscreen,
    Voxel,
    Weather,
    Bloom,
    BloomSmooth,
    FigureVertex,
    FigureFragment,
    CompositeVertex,
    CompositeFragment,
    FigureSunVertex,
    FigureSunFragment,
    PlantVertex,
    PlantVis,
    PlantShade,
}

pub const ALL: [Shader; 19] = [
    Shader::Background,
    Shader::Water,
    Shader::SpriteVertex,
    Shader::SpriteFragment,
    Shader::Present,
    Shader::Fullscreen,
    Shader::Voxel,
    Shader::Weather,
    Shader::Bloom,
    Shader::BloomSmooth,
    Shader::FigureVertex,
    Shader::FigureFragment,
    Shader::CompositeVertex,
    Shader::CompositeFragment,
    Shader::FigureSunVertex,
    Shader::FigureSunFragment,
    Shader::PlantVertex,
    Shader::PlantVis,
    Shader::PlantShade,
];

impl Shader {
    /// Maintained WGSL sources produced by the one-time conversion probe.
    pub fn runtime_source(self) -> &'static str {
        macro_rules! sources { ($($shader:ident => $name:literal),* $(,)?) => { match self {
            $(Self::$shader => include_str!(concat!("../../shaders/wgpu/", $name, ".wgsl")),)*
        } }; }
        sources!(Background => "background.frag", Water => "water.frag", SpriteVertex => "sprite.vert",
            SpriteFragment => "sprite.frag", Present => "present.frag", Fullscreen => "fullscreen.vert",
            Voxel => "voxel.frag", Weather => "voxel_weather.frag", Bloom => "bloom.frag",
            BloomSmooth => "bloom_smooth.frag", FigureVertex => "figure.vert", FigureFragment => "figure.frag",
            CompositeVertex => "composite.vert", CompositeFragment => "composite.frag",
            FigureSunVertex => "figure_sun.vert", FigureSunFragment => "figure_sun.frag",
            PlantVertex => "plant.vert", PlantVis => "plant_vis.frag", PlantShade => "plant_shade.frag")
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Background => "background.frag",
            Self::Water => "water.frag",
            Self::SpriteVertex => "sprite.vert",
            Self::SpriteFragment => "sprite.frag",
            Self::Present => "present.frag",
            Self::Fullscreen => "fullscreen.vert",
            Self::Voxel => "voxel.frag",
            Self::Weather => "voxel_weather.frag",
            Self::Bloom => "bloom.frag",
            Self::BloomSmooth => "bloom_smooth.frag",
            Self::FigureVertex => "figure.vert",
            Self::FigureFragment => "figure.frag",
            Self::CompositeVertex => "composite.vert",
            Self::CompositeFragment => "composite.frag",
            Self::FigureSunVertex => "figure_sun.vert",
            Self::FigureSunFragment => "figure_sun.frag",
            Self::PlantVertex => "plant.vert",
            Self::PlantVis => "plant_vis.frag",
            Self::PlantShade => "plant_shade.frag",
        }
    }

    pub fn stage(self) -> naga::ShaderStage {
        match self {
            Self::SpriteVertex
            | Self::Fullscreen
            | Self::FigureVertex
            | Self::CompositeVertex
            | Self::FigureSunVertex
            | Self::PlantVertex => naga::ShaderStage::Vertex,
            _ => naga::ShaderStage::Fragment,
        }
    }

    pub fn source(self, specialization: Specialization) -> Result<String> {
        let name = match self {
            Self::Weather | Self::PlantVis | Self::PlantShade => "voxel.frag",
            _ => self.name(),
        };
        let source = expand(name)?;
        let mut output = String::new();
        let mut samplers = BTreeSet::new();
        for line in source.lines() {
            if line.contains("layout(constant_id") {
                let id = layout_number(line, "constant_id")?;
                let tail = line
                    .split_once("const bool ")
                    .context("specialization declaration")?
                    .1;
                let name = tail
                    .split_whitespace()
                    .next()
                    .context("specialization name")?;
                let value = specialization
                    .0
                    .get(id as usize)
                    .context("unknown specialization id")?;
                output.push_str(&format!(
                    "layout(constant_id = {id}) const bool {name} = {value};\n"
                ));
            } else if let Some((before, tail)) = line.split_once("uniform ") {
                let mut words = tail.split_whitespace();
                let ty = words.next().unwrap_or("");
                if ty.contains("sampler") && ty != "sampler" {
                    let name = words
                        .next()
                        .context("combined sampler name")?
                        .trim_end_matches(';');
                    let group = layout_number(before, "set")?;
                    let binding = layout_number(before, "binding")?;
                    let linear = matches!(name, "glowTex" | "sunVisTex")
                        || (self == Self::BloomSmooth && name == "src");
                    let sampler_binding = if linear {
                        LINEAR_BINDING
                    } else {
                        NEAREST_BINDING
                    };
                    samplers.insert((group, sampler_binding));
                    let texture_ty = ty.replace("sampler", "texture");
                    output.push_str(&format!("layout(set = {group}, binding = {binding}) uniform {texture_ty} wgpu_{name};\n"));
                    output.push_str(&format!(
                        "#define {name} {ty}(wgpu_{name}, wgpu_sampler_{group}_{sampler_binding})\n"
                    ));
                } else {
                    output.push_str(line);
                    output.push('\n');
                }
            } else {
                // These helpers pass image handles and perform only texelFetch.
                output.push_str(
                    &line
                        .replace("(sampler2D field", "(texture2D field")
                        // Naga's GLSL frontend exposes these builtins as U32;
                        // Vulkan GLSL exposes them as signed ints. Preserve that ABI.
                        .replace("gl_InstanceIndex", "int(gl_InstanceIndex)")
                        .replace("gl_VertexIndex", "int(gl_VertexIndex)"),
                );
                output.push('\n');
                if self.stage() == naga::ShaderStage::Vertex && line.contains("gl_Position =") {
                    // The source was authored for Vulkan's downward clip-space Y.
                    // Naga IR uses the WebGPU/Metal/D3D convention instead.
                    output.push_str("gl_Position.y = -gl_Position.y;\n");
                }
            }
        }
        if self == Self::BloomSmooth {
            // Clamp-to-border is optional in WebGPU. Reproduce the original
            // bilinear transparent border explicitly, including partial edge taps.
            output = output.replace(
                "vec3 tap(vec2 uv) { return texture(src, uv).rgb; }",
                r#"
vec3 borderTexel(ivec2 p) {
    ivec2 size = textureSize(src, 0);
    if (p.y < 0 || p.y >= size.y) { return vec3(0.0); }
    p.x = p.x >= 0 ? p.x % size.x : size.x - 1 - (-1 - p.x) % size.x;
    return texelFetch(src, p, 0).rgb;
}
vec3 tap(vec2 uv) {
    vec2 p = uv * vec2(textureSize(src, 0)) - 0.5;
    ivec2 lo = ivec2(floor(p));
    vec2 f = fract(p);
    return mix(mix(borderTexel(lo), borderTexel(lo + ivec2(1, 0)), f.x),
               mix(borderTexel(lo + ivec2(0, 1)), borderTexel(lo + ivec2(1, 1)), f.x), f.y);
}
"#,
            );
        }
        let declarations = samplers.into_iter().map(|(group, binding)| {
            format!("layout(set = {group}, binding = {binding}) uniform sampler wgpu_sampler_{group}_{binding};\n")
        }).collect::<String>();
        let (version, rest) = output.split_once('\n').context("shader version")?;
        Ok(format!("{version}\n{declarations}{rest}"))
    }

    pub fn parse(self, specialization: Specialization) -> Result<naga::Module> {
        let source = self.source(specialization)?;
        let mut options = naga::front::glsl::Options::from(self.stage());
        match self {
            Self::Weather => {
                options.defines.insert("FOG_OCCLUDE".into(), "1".into());
            }
            Self::PlantVis => {
                options.defines.insert("PLANT_VIS".into(), "1".into());
            }
            Self::PlantShade => {
                options.defines.insert("PLANT_SHADE".into(), "1".into());
            }
            _ => {}
        }
        naga::front::glsl::Frontend::default()
            .parse(&options, &source)
            .map_err(|e| anyhow::anyhow!("{}: {}", self.name(), e.emit_to_string(&source)))
    }

    /// Parse immutable runtime override metadata once per shader, shared by all
    /// devices. Pipeline constants vary per world; no branch is specialized here.
    pub fn runtime_constants(self, specialization: Specialization) -> Result<Vec<(String, f64)>> {
        type Metadata = std::result::Result<Vec<u16>, String>;
        static METADATA: [std::sync::OnceLock<Metadata>; 19] =
            [const { std::sync::OnceLock::new() }; 19];
        let ids = METADATA[self as usize]
            .get_or_init(|| {
                naga::front::wgsl::parse_str(self.runtime_source())
                    .map(|module| {
                        module
                            .overrides
                            .iter()
                            .filter_map(|(_, item)| item.id)
                            .collect()
                    })
                    .map_err(|error| error.emit_to_string(self.runtime_source()))
            })
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        Ok(ids
            .iter()
            .map(|&id| {
                (
                    id.to_string(),
                    f64::from(u8::from(specialization.0[usize::from(id)])),
                )
            })
            .collect())
    }

    /// One-time conversion: retain overrides and all branches in the emitted
    /// source. Native pipeline specialization happens only after this step.
    pub fn wgsl(self) -> Result<String> {
        let mut module = self.parse(Specialization::default())?;
        depth_inputs(&mut module);
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .context("validate conversion IR")?;
        let source = naga::back::wgsl::write_string(
            &module,
            &info,
            naga::back::wgsl::WriterFlags::EXPLICIT_TYPES,
        )
        .context("emit WGSL without specializing")?;
        let source = compact_uniform_members(&source);
        let roundtrip = naga::front::wgsl::parse_str(&source).context("parse generated WGSL")?;
        ensure!(
            roundtrip.overrides.len() == module.overrides.len(),
            "conversion pruned shader overrides"
        );
        Ok(source)
    }
}

/// Naga's GLSL frontend loads the complete uniform struct before each member
/// access. Keep generated WGSL at member granularity, as the maintained shaders
/// do, to avoid copying the 3984-byte scene (including three large arrays).
/// Only immutable aliases used exclusively through member access are removed.
fn compact_uniform_members(source: &str) -> String {
    if !source.contains("var<uniform> u: VoxelScene;") {
        return source.to_owned();
    }
    let mut chunks = source.split("\nfn ");
    let mut output = chunks.next().unwrap_or_default().to_owned();
    for chunk in chunks {
        let declarations: BTreeMap<_, _> = chunk
            .lines()
            .filter_map(|line| {
                let declaration = line.trim();
                let name = declaration
                    .strip_prefix("let ")?
                    .strip_suffix(": VoxelScene = u;")?;
                Some((name.to_owned(), line))
            })
            .collect();
        let without_declarations = chunk
            .lines()
            .filter(|line| !declarations.values().any(|declaration| declaration == line))
            .collect::<Vec<_>>()
            .join("\n");
        let mut removable: BTreeSet<_> = declarations.keys().cloned().collect();
        // Token boundaries prevent handle names such as _e1 and _e10 colliding.
        for (start, end) in identifiers(&without_declarations) {
            let name = &without_declarations[start..end];
            if removable.contains(name) && !without_declarations[end..].starts_with('.') {
                removable.remove(name);
            }
        }
        let retained = chunk
            .lines()
            .filter(|line| {
                !removable
                    .iter()
                    .any(|name| declarations.get(name) == Some(line))
            })
            .collect::<Vec<_>>()
            .join("\n");
        output.push_str("\nfn ");
        let mut previous = 0;
        for (start, end) in identifiers(&retained) {
            output.push_str(&retained[previous..start]);
            let name = &retained[start..end];
            output.push_str(if removable.contains(name) { "u" } else { name });
            previous = end;
        }
        output.push_str(&retained[previous..]);
    }
    output.push('\n');
    output
}

fn identifiers(source: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (index, ch) in source.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            start.get_or_insert(index);
        } else if let Some(first) = start.take() {
            spans.push((first, index));
        }
    }
    if let Some(first) = start {
        spans.push((first, source.len()));
    }
    spans
}

/// Vulkan allowed sampler2D loads from depth views. WebGPU makes that image
/// class explicit; keep the same scalar depth values and comparisons.
fn depth_inputs(module: &mut naga::Module) {
    let mut globals = std::collections::HashSet::new();
    for (handle, global) in module.global_variables.iter_mut() {
        if matches!(
            global.name.as_deref(),
            Some("wgpu_figSunTex" | "wgpu_plantDepths" | "wgpu_weatherFigureZ")
        ) {
            let mut ty = module.types[global.ty].clone();
            if let naga::TypeInner::Image { class, .. } = &mut ty.inner {
                *class = naga::ImageClass::Depth { multi: false };
                global.ty = module.types.insert(ty, naga::Span::UNDEFINED);
                globals.insert(handle);
            }
        }
    }
    fn fix(
        function: &mut naga::Function,
        globals: &std::collections::HashSet<naga::Handle<naga::GlobalVariable>>,
    ) {
        let loads: std::collections::HashSet<_> = function
            .expressions
            .iter()
            .filter_map(|(handle, expr)| match expr {
                naga::Expression::ImageLoad { image, .. } => match function.expressions[*image] {
                    naga::Expression::GlobalVariable(global) if globals.contains(&global) => {
                        Some(handle)
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect();
        for (_, expr) in function.expressions.iter_mut() {
            if let naga::Expression::AccessIndex { base, index: 0 } = *expr {
                if loads.contains(&base) {
                    *expr = naga::Expression::As {
                        expr: base,
                        kind: naga::ScalarKind::Float,
                        convert: Some(4),
                    };
                }
            }
        }
    }
    for (_, function) in module.functions.iter_mut() {
        fix(function, &globals);
    }
    for entry in &mut module.entry_points {
        fix(&mut entry.function, &globals);
    }
}

pub fn constants(module: &naga::Module, specialization: Specialization) -> Vec<(String, f64)> {
    module
        .overrides
        .iter()
        .filter_map(|(_, item)| {
            item.id.map(|id| {
                (
                    id.to_string(),
                    if specialization.0[usize::from(id)] {
                        1.0
                    } else {
                        0.0
                    },
                )
            })
        })
        .collect()
}

fn layout_number(line: &str, key: &str) -> Result<u32> {
    let tail = line
        .split_once(key)
        .with_context(|| format!("missing layout {key}"))?
        .1;
    let tail = tail
        .trim_start()
        .strip_prefix('=')
        .context("layout assignment")?
        .trim_start();
    let number: String = tail.chars().take_while(char::is_ascii_digit).collect();
    number.parse().context("layout number")
}

fn expand(name: &str) -> Result<String> {
    let source = embedded(name).with_context(|| format!("unknown embedded shader {name}"))?;
    let mut output = String::new();
    for line in source.lines() {
        if let Some(include) = line.trim().strip_prefix("#include \"") {
            let name = include.strip_suffix('"').context("shader include")?;
            output.push_str(&expand(name)?);
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    Ok(output)
}

fn embedded(name: &str) -> Option<&'static str> {
    macro_rules! sources { ($($name:literal),* $(,)?) => { match name {
        $($name => Some(include_str!(concat!("../../shaders/", $name))),)* _ => None,
    } }; }
    sources!(
        "background.frag",
        "water.frag",
        "sprite.vert",
        "sprite.frag",
        "present.frag",
        "fullscreen.vert",
        "voxel.frag",
        "bloom.frag",
        "bloom_smooth.frag",
        "figure.vert",
        "figure.frag",
        "composite.vert",
        "composite.frag",
        "figure_sun.vert",
        "figure_sun.frag",
        "plant.vert",
        "scene.glsl",
        "world.glsl",
        "plant.glsl",
        "figure.glsl",
        "volumetric.glsl",
        "drape_cord.glsl",
        "plant_main.glsl"
    )
}

/// Emit both native shader languages with real resource maps. This checks Naga
/// translation; native compiler/runtime validation still needs those platforms.
pub fn native_translation(
    module: &naga::Module,
    info: &naga::valid::ModuleInfo,
    specialization: Specialization,
) -> Result<(usize, usize)> {
    let pipeline_constants = constants(module, specialization).into_iter().collect();
    let (resolved, resolved_info) =
        naga::back::pipeline_constants::process_overrides(module, info, None, &pipeline_constants)
            .context("resolve native specialization")?;
    let (module, info) = (&*resolved, &*resolved_info);
    let mut resources = naga::back::msl::EntryPointResources::default();
    let mut hlsl = naga::back::hlsl::Options {
        fake_missing_bindings: false,
        ..Default::default()
    };
    let mut slots = BTreeMap::<&str, u8>::new();
    for (_, global) in module.global_variables.iter() {
        if let Some(binding) = global.binding {
            let ty = &module.types[global.ty].inner;
            let class = match ty {
                naga::TypeInner::Image { .. } => "texture",
                naga::TypeInner::Sampler { .. } => "sampler",
                _ => "buffer",
            };
            let slot = slots.entry(class).or_default();
            let mut target = naga::back::msl::BindTarget::default();
            match class {
                "texture" => target.texture = Some(*slot),
                "sampler" => {
                    target.sampler = Some(naga::back::msl::BindSamplerTarget::Resource(*slot))
                }
                _ => target.buffer = Some(*slot),
            }
            *slot += 1;
            resources.resources.insert(binding, target);
            hlsl.binding_map.insert(
                binding,
                naga::back::hlsl::BindTarget {
                    space: binding.group.try_into()?,
                    register: binding.binding,
                    ..Default::default()
                },
            );
            if class == "sampler" {
                hlsl.sampler_buffer_binding_map.insert(
                    naga::back::hlsl::SamplerIndexBufferKey {
                        group: binding.group,
                    },
                    naga::back::hlsl::BindTarget {
                        space: 4,
                        register: binding.group + 2,
                        ..Default::default()
                    },
                );
            }
        }
    }
    let buffer_slot = *slots.get("buffer").unwrap_or(&0);
    resources.immediates_buffer = Some(buffer_slot);
    resources.sizes_buffer = Some(buffer_slot + 1);
    hlsl.immediates_target = Some(naga::back::hlsl::BindTarget {
        space: 3,
        register: 0,
        ..Default::default()
    });
    hlsl.sampler_heap_target = naga::back::hlsl::SamplerHeapBindTargets {
        standard_samplers: naga::back::hlsl::BindTarget {
            space: 4,
            register: 0,
            ..Default::default()
        },
        comparison_samplers: naga::back::hlsl::BindTarget {
            space: 4,
            register: 1,
            ..Default::default()
        },
    };
    let mut msl = naga::back::msl::Options {
        lang_version: (2, 4),
        fake_missing_bindings: false,
        ..Default::default()
    };
    for entry in &module.entry_points {
        msl.per_entry_point_map
            .insert(entry.name.clone(), resources.clone());
    }
    let (metal, reflection) = naga::back::msl::write_string(
        module,
        info,
        &msl,
        &naga::back::msl::PipelineOptions::default(),
    )
    .context("MSL emission")?;
    ensure!(
        reflection.entry_point_names.iter().all(|name| name.is_ok()),
        "MSL entrypoint translation failed: {:?}",
        reflection.entry_point_names
    );
    let mut directx = String::new();
    let pipeline = naga::back::hlsl::PipelineOptions::default();
    let reflection = naga::back::hlsl::Writer::new(&mut directx, &hlsl, &pipeline)
        .write(module, info, None)
        .context("HLSL emission")?;
    ensure!(
        reflection.entry_point_names.iter().all(|name| name.is_ok()),
        "HLSL entrypoint translation failed: {:?}",
        reflection.entry_point_names
    );
    Ok((metal.len(), directx.len()))
}
