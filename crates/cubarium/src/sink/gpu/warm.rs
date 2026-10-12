//! **Shader pipelines compiled ahead of need**, on a thread of their own, from when the app asks
//! ([`super::hosted::Hosted::warm_pipelines`]; the Godot app does as its first renderer starts).
//!
//! Hosted renderers and this warm-up share one GPU in the app process. Compiled
//! pipelines stay resident on that device, so previews and live worlds reuse
//! them safely. A new process warms its pipelines again.
//!
//! So [`start`] compiles once, in the background: first the pipelines a still terrain draws
//! with ([`Pipelines::Still`], every new-world preview), then the rest ([`Pipelines::All`],
//! every world that opens). A renderer about to open asks [`wait`] for the phase it needs: it
//! returns at once when that phase is done (or no warm-up runs for its config), and otherwise
//! blocks only until it is, so the renderer then finds its pipelines cached instead of
//! compiling a second copy beside the warm-up. Nothing here runs on, or locks anything the
//! presenter or the app's main thread takes.
//!
//! **The backend seam** is [`compile`]: build renderers of the given [`Pipelines`] in turn for
//! a small world with the app's config, on one device, and destroy them. Everything else here
//! is backend-neutral.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cubarium_gpu::Gpu;
use cubarium_gpu::voxel::{Pipelines, VoxelRenderer};

use super::{load_textures, params_of};
use crate::voxel::project::Projection;
use crate::voxel::{PX_AUTO, VoxelConfig, cover_px_per_voxel};

/// The world the warm-up compiles for. Pipelines do not depend on the world's size (their
/// specialisation is the lighting tier and effect switches, their attachments the raster
/// formats), so a small one keeps the warm-up's allocations small.
const WARM_WORLD: (u32, u32, u32) = (64, 32, 16);
/// How often a waiter looks at its give-up flag.
const WAIT_SLICE: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Running,
    /// Compiled, or failed (a waiter then compiles for itself).
    Done,
}

struct State {
    /// What the running or finished warm-up compiled for: the presentation config.
    key: Option<String>,
    still: Phase,
    all: Phase,
    started: Option<Instant>,
}

struct Warm {
    state: Mutex<State>,
    moved: Condvar,
}

static WARM: Warm = Warm {
    state: Mutex::new(State { key: None, still: Phase::Done, all: Phase::Done, started: None }),
    moved: Condvar::new(),
};

/// The app's headless/hosted device. Window surfaces select their own device.
static GPU: Mutex<Option<Arc<Gpu>>> = Mutex::new(None);

pub(crate) fn shared_gpu() -> Result<Arc<Gpu>> {
    let mut stored = GPU.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(gpu) = stored.as_ref() {
        return Ok(gpu.clone());
    }
    let gpu = Arc::new(Gpu::open().context("opening the shared GPU device")?);
    *stored = Some(gpu.clone());
    Ok(gpu)
}

/// The warm-up's thread, for [`stop`].
static THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
/// [`stop`] was called: no further phase starts.
static STOPPING: AtomicBool = AtomicBool::new(false);

/// Stop the warm-up and join its thread: a phase being compiled is finished first (a
/// pipeline compile cannot be interrupted, and a process that exits with one in the driver
/// can crash on the way out), none is started after. For the app's unload.
pub fn stop() {
    STOPPING.store(true, Ordering::SeqCst);
    let thread = THREAD.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(t) = thread {
        let _ = t.join();
    }
    GPU.lock().unwrap_or_else(|e| e.into_inner()).take();
}

/// The key a config's warm-up goes by.
pub fn key_of(config: &Path, root: &Path) -> String {
    format!("{}|{}", root.display(), config.display())
}

fn timing(what: std::fmt::Arguments<'_>) {
    super::super::pack_timer::render_timing(what);
}

/// Start compiling `cfg`'s pipelines in the background, Still first, unless a warm-up has
/// already started in this process (a second config's renderers compile for themselves).
/// `screen` sizes the projection as a hosted renderer's would be. Returns at once.
pub fn start(key: String, cfg: &VoxelConfig, screen: (u32, u32)) -> bool {
    // Serialize publication with stop, which must join even a just-started compile.
    let mut thread = THREAD.lock().unwrap_or_else(|e| e.into_inner());
    {
        let mut s = WARM.state.lock().unwrap_or_else(|e| e.into_inner());
        if s.key.is_some() || STOPPING.load(Ordering::SeqCst) {
            return false;
        }
        *s = State { key: Some(key.clone()), still: Phase::Running, all: Phase::Running, started: Some(Instant::now()) };
    }
    let cfg = cfg.clone();
    let spawned = std::thread::Builder::new().name("cubarium-pipeline-warm".into()).spawn(move || {
        let t0 = Instant::now();
        let mut lap = Instant::now();
        let finish = |kind: Pipelines| {
            let mut s = WARM.state.lock().unwrap_or_else(|e| e.into_inner());
            // Every pipeline Still builds is among All's.
            s.still = Phase::Done;
            if kind == Pipelines::All {
                s.all = Phase::Done;
            }
            WARM.moved.notify_all();
        };
        let run = compile(&cfg, screen, &[Pipelines::Still, Pipelines::All], |kind, made| {
            match made {
                Ok(()) => timing(format_args!(
                    "warm-up {kind:?} compiled in {:.0} ms ({:.0} ms since start)",
                    lap.elapsed().as_secs_f64() * 1e3,
                    t0.elapsed().as_secs_f64() * 1e3
                )),
                Err(e) => eprintln!("cubarium: the pipeline warm-up ({kind:?}) failed: {e:#}"),
            }
            // Still at the app's own priority: a preview may be waiting on it, behind a
            // terrain generation that fills every core. The rest has until a world opens.
            lower_priority();
            lap = Instant::now();
            finish(kind);
        });
        if let Err(e) = run {
            eprintln!("cubarium: no pipeline warm-up: {e:#}");
        }
        // Whatever happened, nobody waits on it any more.
        finish(Pipelines::All);
    });
    let spawned = spawned.map(|t| *thread = Some(t));
    if let Err(e) = spawned {
        eprintln!("cubarium: no pipeline warm-up thread: {e}");
        let mut s = WARM.state.lock().unwrap_or_else(|e| e.into_inner());
        s.still = Phase::Done;
        s.all = Phase::Done;
        WARM.moved.notify_all();
    }
    true
}

/// The warm-up's thread, and the compile threads and driver threads it starts from now on
/// (they inherit it), at the lowest CPU priority, so the app's own threads run first.
fn lower_priority() {
    #[cfg(target_os = "linux")]
    {
        let _ = rustix::process::setpriority_process(Some(rustix::thread::gettid()), 19);
    }
}

/// Block until the warm-up for `key` has compiled what a renderer building `want` needs, or
/// `give_up` says so. Returns at once when no warm-up runs for `key`. How long it waited.
pub fn wait(key: &str, want: Pipelines, give_up: impl Fn() -> bool) -> Duration {
    let started = Instant::now();
    let mut s = WARM.state.lock().unwrap_or_else(|e| e.into_inner());
    if s.key.as_deref() != Some(key) {
        return Duration::ZERO;
    }
    let pending = |s: &State| match want {
        Pipelines::Still => s.still == Phase::Running,
        Pipelines::All => s.all == Phase::Running,
    };
    if !pending(&s) {
        return Duration::ZERO;
    }
    let since = s.started.map_or(Duration::ZERO, |t| t.elapsed());
    timing(format_args!("waiting for the warm-up's {want:?} pipelines ({:.0} ms into it)", since.as_secs_f64() * 1e3));
    while pending(&s) && !give_up() {
        s = WARM.moved.wait_timeout(s, WAIT_SLICE).unwrap_or_else(|e| e.into_inner()).0;
    }
    let waited = started.elapsed();
    timing(format_args!("waited {:.0} ms for the warm-up's {want:?} pipelines", waited.as_secs_f64() * 1e3));
    waited
}

/// **The backend seam**: compile each of `kinds`' pipelines in turn by building a renderer
/// for a small world with `cfg` and destroying it, calling `done` after each. One device for
/// all of them: opening a device stalls the app's own frame for about as long as it takes
/// (~60 ms on the desktop's NVIDIA driver, 2026-10-11), so the warm-up opens one. Each
/// renderer leaves its compiled pipelines resident on the shared device.
fn compile(cfg: &VoxelConfig, screen: (u32, u32), kinds: &[Pipelines], mut done: impl FnMut(Pipelines, Result<()>)) -> Result<()> {
    let mut cfg = cfg.clone();
    // The real world's pixels per voxel, so the small world's raster is small too.
    if cfg.px_per_voxel == PX_AUTO {
        cfg.px_per_voxel = cover_px_per_voxel(cfg.tilt_degrees, cfg.raster_height, &cfg.sim.world, screen);
    }
    let (w, h, d) = WARM_WORLD;
    cfg.sim.world.width = w;
    cfg.sim.world.height = h;
    cfg.sim.world.depth = d;
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &cfg.sim.world)?;
    let params = params_of(&cfg, proj, true);
    let gpu = shared_gpu()?;
    let textures = load_textures(&cfg, None, &params);
    for &kind in kinds {
        if STOPPING.load(Ordering::SeqCst) {
            break;
        }
        let made = VoxelRenderer::with_pipelines(&gpu, params, &textures, kind).map(|mut r| r.destroy(&gpu));
        done(kind, made);
    }
    Ok(())
}
