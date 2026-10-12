//! Device-resident pipelines and an adapter-keyed, atomically saved driver cache.
#![deny(unsafe_code)]
use anyhow::Result;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

static DIRECTORY: Mutex<Option<PathBuf>> = Mutex::new(None);
static CONFIGURED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Set the application-owned disk cache before opening the first device.
pub fn set_dir(dir: Option<PathBuf>) {
    *DIRECTORY.lock().unwrap_or_else(|e| e.into_inner()) = dir;
    CONFIGURED.store(true, Ordering::Release);
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
/// Native cache bytes saved to disk, when the backend supports them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Saved {
    pub grew: bool,
    pub bytes: usize,
    pub written: bool,
}
pub fn save(gpu: &crate::Gpu) -> Result<Saved> {
    match &gpu.disk_cache {
        Some(cache) => cache.save(),
        None => Ok(Saved::default()),
    }
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
    pipelines: Mutex<HashMap<RenderKey, Arc<Mutex<Option<wgpu::RenderPipeline>>>>>,
    hits: AtomicUsize,
    misses: AtomicUsize,
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
        disk: Option<&Disk>,
    ) -> Result<wgpu::RenderPipeline> {
        let slot = self
            .pipelines
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(RenderKey::new(d))
            .or_default()
            .clone();
        let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pipeline) = &*slot {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(pipeline.clone());
        }
        let pipeline = validated(device, d.label.unwrap_or("render pipeline"), || {
            Ok(compile_turn(|| device.create_render_pipeline(d)))
        })?;
        // Count the descriptor actually submitted to creation, including its
        // exact cache handle. Resident hits do not submit any descriptor.
        if let Some(disk) = disk.filter(|disk| d.cache == Some(&disk.cache)) {
            disk.uses.fetch_add(1, Ordering::Relaxed);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        *slot = Some(pipeline.clone());
        Ok(pipeline)
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

/// Construction scopes are thread-local in wgpu, including warm workers.
/// Pop even when the constructor returns an ordinary application error.
pub(crate) fn validated<T>(
    device: &wgpu::Device,
    label: &str,
    create: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let result = create();
    if let Some(error) = pollster::block_on(scope.pop()) {
        anyhow::bail!("GPU validation during {label}: {error}");
    }
    result
}

#[cfg(not(test))]
pub(crate) fn init_default_dir() {
    if CONFIGURED.load(Ordering::Acquire) {
        return;
    }
    let mut directory = DIRECTORY.lock().unwrap_or_else(|e| e.into_inner());
    if CONFIGURED.load(Ordering::Acquire) {
        return;
    }
    *directory = match std::env::var_os("CUBARIUM_PIPELINE_CACHE") {
        Some(value) if value.is_empty() => None,
        Some(value) => Some(value.into()),
        None => std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .map(|p| p.join("cubarium/pipeline-cache")),
    };
    CONFIGURED.store(true, Ordering::Release);
}

/// The envelope rejects accidental corruption and foreign-adapter files before
/// their contents reach the driver. The payload is only PipelineCache::get_data.
const MAGIC: &[u8; 8] = b"CUBPC001";
fn envelope(key: &str, blob: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut out = Vec::with_capacity(52 + key.len() + blob.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(key.len() as u32).to_le_bytes());
    out.extend_from_slice(&(blob.len() as u64).to_le_bytes());
    out.extend_from_slice(&Sha256::digest(blob));
    out.extend_from_slice(key.as_bytes());
    out.extend_from_slice(blob);
    out
}
fn payload<'a>(key: &str, bytes: &'a [u8]) -> Option<&'a [u8]> {
    use sha2::{Digest, Sha256};
    if bytes.get(..8)? != MAGIC {
        return None;
    }
    let key_len = u32::from_le_bytes(bytes.get(8..12)?.try_into().ok()?) as usize;
    let length = usize::try_from(u64::from_le_bytes(bytes.get(12..20)?.try_into().ok()?)).ok()?;
    let start = 52usize.checked_add(key_len)?;
    if bytes.get(52..start)? != key.as_bytes() || start.checked_add(length)? != bytes.len() {
        return None;
    }
    let blob = bytes.get(start..)?;
    (bytes.get(20..52)? == Sha256::digest(blob).as_slice()).then_some(blob)
}

pub(crate) struct Disk {
    pub cache: wgpu::PipelineCache,
    path: Option<PathBuf>,
    key: String,
    seed_bytes: AtomicUsize,
    save_turn: Mutex<()>,
    pub loaded: bool,
    pub uses: AtomicUsize,
}
impl Disk {
    pub fn open(
        device: &wgpu::Device,
        info: &wgpu::AdapterInfo,
        directory: Option<PathBuf>,
    ) -> Option<Self> {
        if !device.features().contains(wgpu::Features::PIPELINE_CACHE) {
            return None;
        }
        let key = wgpu::util::pipeline_cache_key(info)?;
        if let Some(directory) = &directory {
            remove_stale_temps(directory, std::time::SystemTime::now());
        }
        let path = directory.map(|d| d.join(&key));
        let bytes = path.as_ref().and_then(|p| std::fs::read(p).ok());
        let data = bytes.as_deref().and_then(|b| payload(&key, b));
        let descriptor = wgpu::PipelineCacheDescriptor {
            label: Some("Cubarium disk cache"),
            data,
            fallback: true,
        };
        let cache = import_cache(device, &descriptor);
        Some(Self {
            cache,
            path,
            key,
            seed_bytes: AtomicUsize::new(data.map_or(0, <[u8]>::len)),
            save_turn: Mutex::new(()),
            loaded: data.is_some(),
            uses: AtomicUsize::new(0),
        })
    }
    pub fn save(&self) -> Result<Saved> {
        let _turn = self.save_turn.lock().unwrap_or_else(|e| e.into_inner());
        let Some(data) = self.cache.get_data() else {
            return Ok(Saved::default());
        };
        let previous = self.seed_bytes.load(Ordering::Relaxed);
        if data.len() <= previous {
            return Ok(Saved {
                grew: false,
                bytes: data.len(),
                written: false,
            });
        }
        let written = if let Some(path) = &self.path {
            atomic_write(path, &envelope(&self.key, &data))?;
            true
        } else {
            false
        };
        self.seed_bytes.store(data.len(), Ordering::Relaxed);
        Ok(Saved {
            grew: data.len() > previous,
            bytes: data.len(),
            written,
        })
    }
}

#[allow(unsafe_code)]
fn import_cache(
    device: &wgpu::Device,
    descriptor: &wgpu::PipelineCacheDescriptor<'_>,
) -> wgpu::PipelineCache {
    // SAFETY: only intact, checksummed get_data blobs bearing this adapter's
    // pipeline_cache_key are imported from the app-owned cache directory.
    // wgpu validates its driver/version header; fallback rejects obsolete data.
    // Wrysk approved this one call on 2026-10-12.
    unsafe { device.create_pipeline_cache(descriptor) }
}

fn remove_stale_temps(directory: &std::path::Path, now: std::time::SystemTime) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "tmp") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_file()
            && metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age > std::time::Duration::from_secs(24 * 60 * 60))
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    atomic_write_before_rename(path, bytes, || Ok(()))
}
fn atomic_write_before_rename(
    path: &std::path::Path,
    bytes: &[u8],
    before_rename: impl FnOnce() -> Result<()>,
) -> Result<()> {
    use std::io::Write;
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cache path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let temp = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        before_rename()?;
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
#[forbid(unsafe_code)]
mod tests {
    use super::*;
    use crate::{Gpu, PresentPass};
    struct CacheDir {
        path: PathBuf,
    }
    impl CacheDir {
        fn new() -> Self {
            static SERIAL: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "cubarium-perf-cache-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }
    impl Drop for CacheDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
    fn render(gpu: &Gpu) -> Vec<u8> {
        let size = wgpu::Extent3d {
            width: 5,
            height: 3,
            depth_or_array_layers: 1,
        };
        let source = gpu.texture(
            "cache test source",
            size,
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let target = gpu.texture(
            "cache test target",
            size,
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let bytes: Vec<u8> = (0..15u8).flat_map(|i| [i * 7, i * 3, i, 255]).collect();
        let mut encoder = gpu.encoder("cache test");
        gpu.upload_texture(&mut encoder, &source, wgpu::Origin3d::ZERO, size, 4, &bytes)
            .unwrap();
        let present = PresentPass::new(gpu, target.format()).unwrap();
        present.record(
            gpu,
            &mut encoder,
            &source.create_view(&Default::default()),
            &target.create_view(&Default::default()),
            crate::present::WindowFit::fit((5, 3), (5, 3), false).push(),
        );
        gpu.queue.submit([encoder.finish()]);
        let image = crate::wgpu_backend::headless::read_texture(gpu, &target).unwrap();
        assert_eq!(image, bytes);
        assert!(gpu.take_errors().is_empty());
        image
    }
    #[test]
    fn saved_cache_reloads_and_is_used_and_corrupt_or_foreign_files_render_identically() {
        let directory = CacheDir::new();
        let gpu = Gpu::open_with_cache_dir(Some(directory.path.clone())).unwrap();
        let expected = render(&gpu);
        let saved = save(&gpu).unwrap();
        assert!(
            saved.written && saved.bytes > 0,
            "acceptance requires a disk-cache adapter"
        );
        let key = wgpu::util::pipeline_cache_key(&gpu.adapter.get_info()).unwrap();
        let path = directory.path.join(&key);
        drop(gpu);
        let valid = std::fs::read(&path).unwrap();
        let gpu = Gpu::open_with_cache_dir(Some(directory.path.clone())).unwrap();
        assert!(
            gpu.disk_cache_stats().unwrap().0,
            "the saved payload was loaded"
        );
        assert_eq!(render(&gpu), expected);
        assert_eq!(
            gpu.disk_cache_stats().unwrap().1,
            gpu.pipeline_cache_stats().misses,
            "every created pipeline received the reloaded cache in its descriptor"
        );
        assert!(gpu.disk_cache_stats().unwrap().1 > 0);
        let unchanged = save(&gpu).unwrap();
        assert!(
            !unchanged.grew && !unchanged.written,
            "unchanged caches are not rewritten"
        );
        assert_eq!(std::fs::read(&path).unwrap(), valid);
        drop(gpu);
        let foreign = envelope(
            "wgpu_pipeline_cache_vulkan_other_adapter",
            payload(&key, &valid).unwrap(),
        );
        let mut seed = 0x1234_abcd_u32;
        let random: Vec<u8> = (0..valid.len())
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed as u8
            })
            .collect();
        // Keep our envelope valid while corrupting wgpu's native header. This
        // reaches create_pipeline_cache(data=Some(..), fallback=true), rather
        // than stopping at the app's key or SHA checks.
        let mut damaged_payload = payload(&key, &valid).unwrap().to_vec();
        damaged_payload[0] ^= 0xff;
        let damaged = envelope(&key, &damaged_payload);
        assert!(payload(&key, &damaged).is_some());
        std::fs::write(&path, &damaged).unwrap();
        let gpu = Gpu::open_with_cache_dir(Some(directory.path.clone())).unwrap();
        assert!(
            gpu.disk_cache_stats().unwrap().0,
            "valid envelope reached wgpu import"
        );
        assert_eq!(render(&gpu), expected, "wgpu native payload fallback image");
        assert!(gpu.disk_cache_stats().unwrap().1 > 0);
        drop(gpu);
        for (name, bytes) in [
            ("truncated", valid[..valid.len() / 2].to_vec()),
            ("random", random),
            ("foreign", foreign),
        ] {
            std::fs::write(&path, bytes).unwrap();
            let gpu = Gpu::open_with_cache_dir(Some(directory.path.clone())).unwrap();
            assert!(
                !gpu.disk_cache_stats().unwrap().0,
                "{name} file fell back before driver import"
            );
            assert_eq!(render(&gpu), expected, "{name} fallback image");
            drop(gpu);
        }
    }
    #[test]
    fn construction_validation_errors_are_returned_and_never_cached() {
        let gpu = Gpu::open_with_cache_dir(None).unwrap();
        let before = gpu.pipeline_cache_stats();
        for _ in 0..2 {
            let result = PresentPass::new(&gpu, wgpu::TextureFormat::Depth32Float);
            assert!(
                result.is_err(),
                "invalid color format must fail construction"
            );
            assert_eq!(gpu.pipeline_cache_stats().misses, before.misses);
            assert!(
                gpu.resident
                    .pipelines
                    .lock()
                    .unwrap()
                    .values()
                    .all(|slot| slot.lock().unwrap().is_none())
            );
        }
        assert!(
            gpu.take_errors().is_empty(),
            "scopes capture construction errors"
        );
        render(&gpu);
        assert_eq!(gpu.pipeline_cache_stats().misses, before.misses + 1);
    }

    #[test]
    fn an_interrupted_atomic_write_preserves_the_real_file() {
        let directory = CacheDir::new();
        let gpu = Gpu::open_with_cache_dir(Some(directory.path.clone())).unwrap();
        let expected = render(&gpu);
        assert!(save(&gpu).unwrap().written);
        let key = wgpu::util::pipeline_cache_key(&gpu.adapter.get_info()).unwrap();
        let path = directory.path.join(&key);
        drop(gpu);
        let previous = std::fs::read(&path).unwrap();
        let interrupted = atomic_write_before_rename(&path, b"replacement cache", || {
            anyhow::bail!("injected interruption before rename")
        });
        assert!(interrupted.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), previous);
        assert_eq!(std::fs::read_dir(&directory.path).unwrap().count(), 1);
        // A process killed before cleanup leaves a separate partial temp file.
        // Reopen through the production loader to prove it selects the complete
        // file and supplies a usable cache, despite that interrupted write.
        std::fs::write(path.with_extension("interrupted.tmp"), b"partial").unwrap();
        let gpu = Gpu::open_with_cache_dir(Some(directory.path.clone())).unwrap();
        assert!(gpu.disk_cache_stats().unwrap().0);
        assert_eq!(render(&gpu), expected);
        assert_eq!(std::fs::read(&path).unwrap(), previous);
    }

    #[test]
    fn opening_a_cache_removes_only_day_old_temp_files() {
        let directory = CacheDir::new();
        let old = directory.path.join("old.tmp");
        let recent = directory.path.join("recent.tmp");
        let complete = directory.path.join("complete");
        for path in [&old, &recent, &complete] {
            std::fs::write(path, b"cache bytes").unwrap();
        }
        let previous_day =
            std::time::SystemTime::now() - std::time::Duration::from_secs(48 * 60 * 60);
        for path in [&old, &complete] {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(previous_day))
                .unwrap();
        }
        let _gpu = Gpu::open_with_cache_dir(Some(directory.path.clone())).unwrap();
        assert!(!old.exists());
        assert_eq!(std::fs::read(recent).unwrap(), b"cache bytes");
        assert_eq!(std::fs::read(complete).unwrap(), b"cache bytes");
    }
}
