//! **The app's own compiled-pipeline cache** (`VkPipelineCache`), shared by every device the
//! process opens and kept on disk between runs.
//!
//! Every renderer opens a Vulkan device of its own (each preview card, each live world), and
//! a pipeline compiled on one device is not reused by another: on the desktop's NVIDIA driver
//! only its own disk shader cache carried a compile from device to device (2026-10-11,
//! `examples/pipeline_probe.rs`: with that cache off, a second device recompiled every
//! pipeline). That cache is the driver's, with the driver's size limit and eviction.
//!
//! So each [`crate::Gpu`] gets a `VkPipelineCache` seeded from this process's newest blob for
//! its physical device, every pipeline in the crate is created through it, and
//! [`save`] merges what a device compiled back into the blob. With a directory set
//! ([`set_dir`]) the blob is read from and written to `pipelines-VVVV-DDDD.bin` there. A
//! blob whose header names another device or driver build (its `pipelineCacheUUID` changes
//! with the driver) is ignored and replaced at the next save.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use ash::vk;
use ash::vk::Handle as _;

/// `VkPipelineCacheHeaderVersionOne`'s length.
const HEADER_LEN: usize = 16 + 16;

/// Which physical device and driver build a blob is for: vendor, device and the driver's
/// `pipelineCacheUUID`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CacheKey {
    pub vendor: u32,
    pub device: u32,
    pub uuid: [u8; vk::UUID_SIZE],
}

impl CacheKey {
    pub fn of(props: &vk::PhysicalDeviceProperties) -> CacheKey {
        CacheKey { vendor: props.vendor_id, device: props.device_id, uuid: props.pipeline_cache_uuid }
    }

    fn file_name(&self) -> String {
        format!("pipelines-{:04x}-{:04x}.bin", self.vendor, self.device)
    }

    /// Whether `blob` starts with a version-one header for this device and driver build.
    pub fn accepts(&self, blob: &[u8]) -> bool {
        if blob.len() < HEADER_LEN {
            return false;
        }
        let word = |i: usize| u32::from_le_bytes([blob[i], blob[i + 1], blob[i + 2], blob[i + 3]]);
        word(0) as usize >= HEADER_LEN
            && word(4) == vk::PipelineCacheHeaderVersion::ONE.as_raw() as u32
            && word(8) == self.vendor
            && word(12) == self.device
            && blob[16..32] == self.uuid
    }
}

struct Entry {
    /// The newest merged blob (empty: none yet).
    blob: Vec<u8>,
    /// The directory was looked in for this key.
    loaded: bool,
}

struct Store {
    dir: Option<PathBuf>,
    /// One per physical device and driver build met (a handful at most).
    entries: Vec<(CacheKey, Entry)>,
}

static STORE: Mutex<Store> = Mutex::new(Store { dir: None, entries: Vec::new() });
/// Serialises the file writes (outside [`STORE`]'s lock).
static WRITE: Mutex<()> = Mutex::new(());
/// Each device's cache, by the device's raw handle: how the pipeline builders, which are
/// handed an `ash::Device`, find their device's cache.
static BY_DEVICE: Mutex<Vec<(u64, u64)>> = Mutex::new(Vec::new());

/// Keep the cache in `dir` (created when first written): read at a device's first open,
/// written by [`save`]. `None` keeps it in this process only. Set it before the first device
/// opens; a directory set later is read for keys not yet looked up.
pub fn set_dir(dir: Option<PathBuf>) {
    let mut s = STORE.lock().unwrap_or_else(|e| e.into_inner());
    if s.dir != dir {
        s.dir = dir;
        for (_, e) in s.entries.iter_mut() {
            e.loaded = e.loaded && !e.blob.is_empty();
        }
    }
}

/// [`set_dir`] with `CUBARIUM_PIPELINE_CACHE` when it is set (empty: in this process only),
/// else `default`.
pub fn use_dir(default: Option<PathBuf>) {
    set_dir(match std::env::var_os("CUBARIUM_PIPELINE_CACHE") {
        Some(v) if v.is_empty() => None,
        Some(v) => Some(PathBuf::from(v)),
        None => default,
    });
}

/// The directory [`set_dir`] named.
pub fn dir() -> Option<PathBuf> {
    STORE.lock().unwrap_or_else(|e| e.into_inner()).dir.clone()
}

/// `key`'s entry, made (as `loaded`) when it has none.
fn entry(s: &mut Store, key: CacheKey, loaded: bool) -> &mut Entry {
    let i = match s.entries.iter().position(|(k, _)| *k == key) {
        Some(i) => i,
        None => {
            s.entries.push((key, Entry { blob: Vec::new(), loaded }));
            s.entries.len() - 1
        }
    };
    &mut s.entries[i].1
}

/// The newest blob for `key`, read from the directory the first time it is asked for.
fn seed(key: CacheKey) -> Vec<u8> {
    let mut s = STORE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = s.dir.clone();
    let e = entry(&mut s, key, false);
    if !e.loaded {
        e.loaded = true;
        if let Some(dir) = dir
            && e.blob.is_empty()
            && let Ok(blob) = std::fs::read(dir.join(key.file_name()))
            && key.accepts(&blob)
        {
            e.blob = blob;
        }
    }
    e.blob.clone()
}

/// A pipeline cache for a device just made on the physical device `key` names, seeded with
/// this process's newest blob for it, and that blob's length. Registered for
/// [`of`]. A seed the driver refuses is dropped and the cache starts empty.
pub(crate) fn create(device: &ash::Device, key: CacheKey) -> Result<(vk::PipelineCache, usize)> {
    let blob = seed(key);
    let made = unsafe {
        device.create_pipeline_cache(&vk::PipelineCacheCreateInfo::default().initial_data(&blob), None)
    };
    let (cache, seeded) = match made {
        Ok(c) => (c, blob.len()),
        Err(_) => (unsafe { device.create_pipeline_cache(&vk::PipelineCacheCreateInfo::default(), None) }
            .context("vkCreatePipelineCache")?, 0),
    };
    BY_DEVICE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((device.handle().as_raw(), cache.as_raw()));
    Ok((cache, seeded))
}

/// The pipeline cache of the device `d` ([`create`]); null for a device opened elsewhere.
pub(crate) fn of(d: &ash::Device) -> vk::PipelineCache {
    let raw = d.handle().as_raw();
    BY_DEVICE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .rev()
        .find(|(dev, _)| *dev == raw)
        .map_or(vk::PipelineCache::null(), |(_, c)| vk::PipelineCache::from_raw(*c))
}

/// How [`save`] went.
#[derive(Clone, Copy, Debug, Default)]
pub struct Saved {
    /// The blob grew: this device compiled something the process had not.
    pub grew: bool,
    /// Its length now.
    pub bytes: usize,
    /// It was written to the directory.
    pub written: bool,
}

static TEMP_IDS: AtomicU64 = AtomicU64::new(0);

/// Merge what `gpu`'s device has compiled into the process's blob for its physical device,
/// and write the blob to the directory when it grew. Call it on a thread that may wait a
/// little (a merge and, when it grew, a file write), never the presenter's, and while no
/// pipeline is being created on `gpu`.
pub fn save(gpu: &crate::Gpu) -> Result<Saved> {
    let d = &gpu.device;
    let mine = unsafe { d.get_pipeline_cache_data(gpu.pipeline_cache) }.context("vkGetPipelineCacheData")?;
    if mine.len() <= gpu.pipeline_seed_len {
        return Ok(Saved { grew: false, bytes: mine.len(), written: false });
    }
    let (grew, bytes, dir) = {
        let mut s = STORE.lock().unwrap_or_else(|e| e.into_inner());
        let dir = s.dir.clone();
        let e = entry(&mut s, gpu.cache_key, true);
        // The process's blob may hold what other devices compiled since this one was
        // seeded: merge the two on a cache of our own (a merge's target must not be in use).
        let merged = if e.blob.is_empty() {
            mine
        } else {
            let tmp = unsafe {
                d.create_pipeline_cache(&vk::PipelineCacheCreateInfo::default().initial_data(&e.blob), None)
            }
            .context("vkCreatePipelineCache (merge)")?;
            let out = unsafe { d.merge_pipeline_caches(tmp, &[gpu.pipeline_cache]) }
                .context("vkMergePipelineCaches")
                .and_then(|()| unsafe { d.get_pipeline_cache_data(tmp) }.context("vkGetPipelineCacheData"));
            unsafe { d.destroy_pipeline_cache(tmp, None) };
            out?
        };
        let grew = merged.len() > e.blob.len();
        if grew {
            e.blob = merged;
        }
        (grew, e.blob.len(), dir)
    };
    let mut written = false;
    if grew && let Some(dir) = dir {
        written = write(&dir, gpu.cache_key)?;
    }
    Ok(Saved { grew, bytes, written })
}

/// Write the process's newest blob for `key` to `dir`, by rename so a reader never sees a
/// half-written file.
fn write(dir: &Path, key: CacheKey) -> Result<bool> {
    let _w = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    let blob = {
        let s = STORE.lock().unwrap_or_else(|e| e.into_inner());
        match s.entries.iter().find(|(k, _)| *k == key) {
            Some((_, e)) if !e.blob.is_empty() => e.blob.clone(),
            _ => return Ok(false),
        }
    };
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(key.file_name());
    let tmp = dir.join(format!(
        ".{}.{}.{}",
        key.file_name(),
        std::process::id(),
        TEMP_IDS.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&tmp, &blob).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("renaming to {}", path.display()))?;
    Ok(true)
}
