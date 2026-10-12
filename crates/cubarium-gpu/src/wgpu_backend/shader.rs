//! The shaders, as hand-written WGSL under `crates/cubarium-gpu/shaders/`: the one shader
//! source, maintained by hand. WGSL has no includes, so a shader's file names the shared
//! modules it uses with `// #import <file>.wgsl` lines, and [`Shader::runtime_source`]
//! splices them in once, the first time the shader is asked for. Being a comment, the line
//! keeps every file valid WGSL as it stands, for an editor.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use anyhow::{Context, Result, bail, ensure};

pub const NEAREST_BINDING: u32 = 32;
pub const LINEAR_BINDING: u32 = 33;

/// The twelve `world.wgsl` pipeline-overridable switches, indexed by their `@id`s.
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

pub const ALL: [Shader; 18] = [
    Shader::Background,
    Shader::Water,
    Shader::SpriteVertex,
    Shader::SpriteFragment,
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

/// Every WGSL file a shader is made of, by the name an import uses.
const FILES: &[(&str, &str)] = {
    macro_rules! files { ($($name:literal),* $(,)?) => {
        &[$(($name, include_str!(concat!("../../shaders/", $name))),)*]
    }; }
    files!(
        "background.frag.wgsl",
        "water.frag.wgsl",
        "sprite.vert.wgsl",
        "sprite.frag.wgsl",
        "fullscreen.vert.wgsl",
        "voxel.frag.wgsl",
        "voxel_weather.frag.wgsl",
        "bloom.frag.wgsl",
        "bloom_smooth.frag.wgsl",
        "figure.vert.wgsl",
        "figure.frag.wgsl",
        "composite.vert.wgsl",
        "composite.frag.wgsl",
        "figure_sun.vert.wgsl",
        "figure_sun.frag.wgsl",
        "plant.vert.wgsl",
        "plant_vis.frag.wgsl",
        "plant_shade.frag.wgsl",
        // The shared modules.
        "scene.wgsl",
        "world.wgsl",
        "drape_cord.wgsl",
        "voxel.wgsl",
        "voxel_main.wgsl",
        "volumetric.wgsl",
        "figure.wgsl",
        "plant.wgsl",
        "plant_main.wgsl",
    )
};

const IMPORT: &str = "// #import ";

/// `name`'s source with its imports spliced in where they are named, each file once.
pub fn compose(name: &str) -> Result<String> {
    fn splice(name: &str, seen: &mut BTreeSet<String>, out: &mut String) -> Result<()> {
        if !seen.insert(name.to_owned()) {
            return Ok(());
        }
        let Some(&(_, source)) = FILES.iter().find(|(file, _)| *file == name) else {
            bail!("no shader file {name}");
        };
        for line in source.lines() {
            match line.trim().strip_prefix(IMPORT) {
                Some(import) => splice(import.trim(), seen, out)
                    .with_context(|| format!("imported by {name}"))?,
                None => {
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        Ok(())
    }
    let mut out = String::new();
    splice(name, &mut BTreeSet::new(), &mut out)?;
    Ok(out)
}

impl Shader {
    /// The shader's WGSL, imports spliced in: composed once, then shared.
    pub fn runtime_source(self) -> &'static str {
        static SOURCES: [OnceLock<String>; ALL.len()] = [const { OnceLock::new() }; ALL.len()];
        SOURCES[self as usize].get_or_init(|| {
            compose(self.file()).unwrap_or_else(|error| panic!("{}: {error:#}", self.name()))
        })
    }

    /// The file under `shaders/` holding the shader's entry point.
    pub fn file(self) -> &'static str {
        match self {
            Self::Background => "background.frag.wgsl",
            Self::Water => "water.frag.wgsl",
            Self::SpriteVertex => "sprite.vert.wgsl",
            Self::SpriteFragment => "sprite.frag.wgsl",
            Self::Fullscreen => "fullscreen.vert.wgsl",
            Self::Voxel => "voxel.frag.wgsl",
            Self::Weather => "voxel_weather.frag.wgsl",
            Self::Bloom => "bloom.frag.wgsl",
            Self::BloomSmooth => "bloom_smooth.frag.wgsl",
            Self::FigureVertex => "figure.vert.wgsl",
            Self::FigureFragment => "figure.frag.wgsl",
            Self::CompositeVertex => "composite.vert.wgsl",
            Self::CompositeFragment => "composite.frag.wgsl",
            Self::FigureSunVertex => "figure_sun.vert.wgsl",
            Self::FigureSunFragment => "figure_sun.frag.wgsl",
            Self::PlantVertex => "plant.vert.wgsl",
            Self::PlantVis => "plant_vis.frag.wgsl",
            Self::PlantShade => "plant_shade.frag.wgsl",
        }
    }

    pub fn name(self) -> &'static str {
        self.file().trim_end_matches(".wgsl")
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

    /// Parse immutable runtime override metadata once per shader, shared by all
    /// devices. Pipeline constants vary per world; no branch is specialized here.
    pub fn runtime_constants(self, specialization: Specialization) -> Result<Vec<(String, f64)>> {
        type Metadata = std::result::Result<Vec<u16>, String>;
        static METADATA: [OnceLock<Metadata>; ALL.len()] = [const { OnceLock::new() }; ALL.len()];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shader_composes_and_validates() {
        for shader in ALL {
            let source = shader.runtime_source();
            assert!(!source.contains(IMPORT), "{}: an import was left in", shader.name());
            let module = naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|e| panic!("{}: {}", shader.name(), e.emit_to_string(source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{}: {:?}", shader.name(), e));
            let entry = &module.entry_points[..];
            assert_eq!(entry.len(), 1, "{}", shader.name());
            assert_eq!(entry[0].name, "main", "{}", shader.name());
            assert_eq!(entry[0].stage, shader.stage(), "{}", shader.name());
        }
    }

    #[test]
    fn a_module_imported_twice_is_spliced_once() {
        // world.wgsl imports drape_cord.wgsl, and so does nothing else; voxel.frag.wgsl
        // reaches world.wgsl through several modules.
        let source = compose("voxel.frag.wgsl").unwrap();
        assert_eq!(source.matches("struct VoxelScene {").count(), 1);
        assert_eq!(source.matches("fn cordCell(").count(), 1);
        assert!(compose("no-such.wgsl").is_err());
    }
}
