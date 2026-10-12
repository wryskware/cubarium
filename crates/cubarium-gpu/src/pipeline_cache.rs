//! Safe, device-resident compiled pipelines. Reuse the same `Arc<Gpu>` when
//! warming and rendering. Entries live until that device is dropped; no native
//! binary cache is imported or persisted (wgpu's native cache API is unsafe).
#![forbid(unsafe_code)]
use anyhow::Result;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

static DIRECTORY: Mutex<Option<PathBuf>> = Mutex::new(None);
/// Retain application cache configuration; compiled GPU objects stay in memory.
pub fn set_dir(dir: Option<PathBuf>) {
    *DIRECTORY.lock().unwrap_or_else(|e| e.into_inner()) = dir;
}
pub fn use_dir(default: Option<PathBuf>) {
    set_dir(match std::env::var_os("CUBARIUM_PIPELINE_CACHE") {
        Some(v) if v.is_empty() => None,
        Some(v) => Some(PathBuf::from(v)),
        None => default,
    });
}
pub fn dir() -> Option<PathBuf> {
    DIRECTORY.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub pipelines: usize,
    pub shader_modules: usize,
    pub layouts: usize,
    pub hits: usize,
    pub misses: usize,
}
/// Compatibility report. `bytes` counts resident WGSL source, never native blobs.
#[derive(Clone, Copy, Debug, Default)]
pub struct Saved {
    pub grew: bool,
    pub bytes: usize,
    pub written: bool,
}
pub fn save(gpu: &crate::Gpu) -> Result<Saved> {
    Ok(gpu.resident.saved())
}

type Entries<K, V> = Mutex<HashMap<K, Arc<OnceLock<V>>>>;
fn entry<K: std::hash::Hash + Eq, V>(map: &Entries<K, V>, key: K) -> Arc<OnceLock<V>> {
    map.lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(key)
        .or_default()
        .clone()
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct LayoutKey {
    groups: Vec<Option<wgpu::BindGroupLayout>>,
    immediate_size: u32,
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct StageKey {
    module: wgpu::ShaderModule,
    entry: Option<String>,
    constants: Vec<(String, u64)>,
    zero: bool,
}
impl StageKey {
    fn new(
        module: &wgpu::ShaderModule,
        name: Option<&str>,
        options: &wgpu::PipelineCompilationOptions<'_>,
    ) -> Self {
        Self {
            module: module.clone(),
            entry: name.map(str::to_owned),
            constants: options
                .constants
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_bits()))
                .collect(),
            zero: options.zero_initialize_workgroup_memory,
        }
    }
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct VertexBufferKey {
    stride: u64,
    step: wgpu::VertexStepMode,
    attributes: Vec<wgpu::VertexAttribute>,
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct RenderKey {
    layout: Option<wgpu::PipelineLayout>,
    vertex: StageKey,
    buffers: Vec<Option<VertexBufferKey>>,
    fragment: Option<(StageKey, Vec<Option<wgpu::ColorTargetState>>)>,
    primitive: wgpu::PrimitiveState,
    depth: Option<wgpu::DepthStencilState>,
    multisample: wgpu::MultisampleState,
    multiview: Option<NonZeroU32>,
}
impl RenderKey {
    fn new(d: &wgpu::RenderPipelineDescriptor<'_>) -> Self {
        Self {
            layout: d.layout.cloned(),
            vertex: StageKey::new(
                d.vertex.module,
                d.vertex.entry_point,
                &d.vertex.compilation_options,
            ),
            buffers: d
                .vertex
                .buffers
                .iter()
                .map(|b| {
                    b.as_ref().map(|b| VertexBufferKey {
                        stride: b.array_stride,
                        step: b.step_mode,
                        attributes: b.attributes.to_vec(),
                    })
                })
                .collect(),
            fragment: d.fragment.as_ref().map(|f| {
                (
                    StageKey::new(f.module, f.entry_point, &f.compilation_options),
                    f.targets.to_vec(),
                )
            }),
            primitive: d.primitive,
            depth: d.depth_stencil.clone(),
            multisample: d.multisample,
            multiview: d.multiview_mask,
        }
    }
}
#[derive(Default)]
pub(crate) struct Resident {
    shaders: Entries<String, wgpu::ShaderModule>,
    groups: Entries<Vec<wgpu::BindGroupLayoutEntry>, wgpu::BindGroupLayout>,
    layouts: Entries<LayoutKey, wgpu::PipelineLayout>,
    pipelines: Entries<RenderKey, wgpu::RenderPipeline>,
    hits: AtomicUsize,
    misses: AtomicUsize,
    saved_count: AtomicUsize,
}
impl Resident {
    pub fn shader(
        &self,
        device: &wgpu::Device,
        descriptor: wgpu::ShaderModuleDescriptor<'_>,
    ) -> wgpu::ShaderModule {
        let wgpu::ShaderSource::Wgsl(ref source) = descriptor.source else {
            return device.create_shader_module(descriptor);
        };
        let slot = entry(&self.shaders, source.to_string());
        slot.get_or_init(|| device.create_shader_module(descriptor))
            .clone()
    }
    pub fn group(
        &self,
        device: &wgpu::Device,
        d: &wgpu::BindGroupLayoutDescriptor<'_>,
    ) -> wgpu::BindGroupLayout {
        let slot = entry(&self.groups, d.entries.to_vec());
        slot.get_or_init(|| device.create_bind_group_layout(d))
            .clone()
    }
    pub fn layout(
        &self,
        device: &wgpu::Device,
        d: &wgpu::PipelineLayoutDescriptor<'_>,
    ) -> wgpu::PipelineLayout {
        let slot = entry(
            &self.layouts,
            LayoutKey {
                groups: d.bind_group_layouts.iter().map(|l| l.cloned()).collect(),
                immediate_size: d.immediate_size,
            },
        );
        slot.get_or_init(|| device.create_pipeline_layout(d))
            .clone()
    }
    pub fn pipeline(
        &self,
        device: &wgpu::Device,
        d: &wgpu::RenderPipelineDescriptor<'_>,
    ) -> wgpu::RenderPipeline {
        assert!(
            d.cache.is_none(),
            "native pipeline caches are outside the safe renderer"
        );
        let slot = entry(&self.pipelines, RenderKey::new(d));
        let mut built = false;
        let pipeline = slot
            .get_or_init(|| {
                built = true;
                compile_turn(|| device.create_render_pipeline(d))
            })
            .clone();
        if built {
            self.misses.fetch_add(1, Ordering::Relaxed);
        } else {
            self.hits.fetch_add(1, Ordering::Relaxed);
        }
        pipeline
    }
    pub fn stats(&self) -> Stats {
        Stats {
            pipelines: self.misses.load(Ordering::Relaxed),
            shader_modules: self.shaders.lock().unwrap_or_else(|e| e.into_inner()).len(),
            layouts: self.layouts.lock().unwrap_or_else(|e| e.into_inner()).len()
                + self.groups.lock().unwrap_or_else(|e| e.into_inner()).len(),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }
    fn saved(&self) -> Saved {
        let count = self.misses.load(Ordering::Relaxed);
        let previous = self.saved_count.swap(count, Ordering::Relaxed);
        Saved {
            grew: count > previous,
            bytes: self
                .shaders
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .keys()
                .map(String::len)
                .sum(),
            written: false,
        }
    }
}

/// NVIDIA's first cache miss must load its shader compiler on one thread.
/// Keep main's guard: release parallel creation once that compiler is mapped.
fn compile_turn<R>(create: impl FnOnce() -> R) -> R {
    use std::sync::atomic::AtomicBool;
    static TURN: Mutex<()> = Mutex::new(());
    static FREE: AtomicBool = AtomicBool::new(false);
    if FREE.load(Ordering::Acquire) {
        return create();
    }
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    if FREE.load(Ordering::Acquire) {
        drop(turn);
        return create();
    }
    let result = create();
    let pending = std::fs::read_to_string("/proc/self/maps").is_ok_and(|m| {
        (m.contains("libnvidia-glcore") || m.contains("libnvidia-eglcore"))
            && !m.contains("libnvidia-rtcore")
    });
    if !pending {
        FREE.store(true, Ordering::Release);
    }
    result
}
