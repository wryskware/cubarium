//! The desktop development window, presented through a Vulkan swapchain on a thread of
//! its own.
//!
//! # Why
//!
//! The window used to be a `minifb` shm buffer: render, wait for the fence, read the
//! whole raster back, upscale it on the CPU and hand it to `minifb` — all on the run
//! loop. At 13 px a voxel the raster is 3328 × 2048 and that was 17 ms a frame on the
//! loop thread (`captures/presentation/P/`), which left the loop busy 100 % of the time
//! with a simulation still to step.
//!
//! # The split
//!
//! It is the panel's presenter (`super::presenter`) with a swapchain where the daemon
//! was, and the same rules: the **recorder** — the run loop — records the world into the
//! raster and posts it, and never waits; a frame that is still waiting is replaced by a
//! fresher one, and a frame with nowhere to go is dropped and counted.
//!
//! The **presenting thread** submits the recorded frame, waits for it, tells the
//! recorder it has retired, and then does everything the window needs on its own:
//! acquires a swapchain image, runs the present pass from the raster onto it — the
//! integer nearest upscale, letterboxed ([`WindowFit`]) — and presents it. Every WSI call
//! is on that thread, because on Wayland with NVIDIA a FIFO present to a window that
//! is on another workspace can block indefinitely: that stops this thread and the
//! picture, and not the world.
//!
//! # Why the present pass is recorded here rather than by the recorder
//!
//! The image to draw into is only known after `vkAcquireNextImageKHR`, and the acquire
//! is one of the calls that can block. So the recorder asks
//! [`FrameSource::record_frame`] for the world alone, and this thread owns a second
//! [`PresentPass`] over the renderer's raster ([`FrameSource::raster_view`]). That is
//! also what lets it rebuild the swapchain when the window is resized without asking the
//! loop for anything.
//!
//! Only one frame is on the GPU at a time, as on the panel: the world frame is waited
//! for before the present pass reads the raster, and the present pass is waited for
//! before the next world frame draws over it.

use std::collections::VecDeque;
use std::ffi::{CStr, c_void};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use ash::vk;

use super::presenter::{
    Frame, FromPresenter, Mailbox, Next, Panel, PresentSample, PresentStats, next_frame,
    present_loop,
};
use crate::present::{FrameSource, PresentPass, WindowFit};
use crate::render::{TargetImage, framebuffer};
use crate::vk::Gpu;

/// The instance extensions a window surface can need. [`Gpu::open`] enables the ones the
/// loader has; a machine with neither surface falls back to the readback window.
pub const SURFACE_EXTENSIONS: [&CStr; 3] = [
    ash::khr::surface::NAME,
    ash::khr::wayland_surface::NAME,
    ash::khr::xlib_surface::NAME,
];

/// The native window to make a surface for: the pointers the windowing library hands
/// out, and nothing that would tie this crate to one.
#[derive(Clone, Copy, Debug)]
pub enum NativeWindow {
    /// A `wl_display *` and the `wl_surface *` on it.
    Wayland {
        display: *mut c_void,
        surface: *mut c_void,
    },
    /// An Xlib `Display *` and a `Window`.
    Xlib { display: *mut c_void, window: u64 },
}

/// Command buffers the recorder records into. Two is the whole pipeline: one on the
/// presenting thread, one waiting or being recorded (a waiting frame is replaced in its
/// own slot).
const SLOTS: usize = 2;

/// How long the presenting thread sleeps on the mailbox between looks at its own state.
/// A posted frame wakes it at once, so this only bounds an idle thread's wake-ups.
const POLL: Duration = Duration::from_millis(20);

/// How long the presenting thread waits for a swapchain image before giving the present
/// up and counting it as never shown. A window on another workspace holds its images,
/// and a thread asleep in the acquire is a thread that cannot be told to stop.
const ACQUIRE_TIMEOUT: Duration = Duration::from_millis(50);

/// How long [`WindowPresenter::destroy`] waits for the thread before leaving it behind.
const JOIN_WAIT: Duration = Duration::from_secs(2);

/// A window size as one word, so the loop can hand it over without a lock.
fn pack(size: (u32, u32)) -> u64 {
    (u64::from(size.0) << 32) | u64::from(size.1)
}

fn unpack(word: u64) -> (u32, u32) {
    ((word >> 32) as u32, word as u32)
}

/// The desktop window as the run loop sees it: a recorder that posts frames to a
/// presenting thread and never waits for one.
pub struct WindowPresenter {
    mail: Arc<Mailbox>,
    back: Receiver<FromPresenter>,
    stats: Arc<PresentStats>,
    /// The window's size in pixels, as the loop last saw it. The thread rebuilds the
    /// swapchain when it changes.
    size: Arc<AtomicU64>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// The recorder's command buffers, by slot, from the renderer's own pool.
    slots: Vec<vk::CommandBuffer>,
    /// Slots the thread has lent and this side has not used yet.
    free: VecDeque<usize>,
    /// Frames with nowhere to go: no slot free, or the renderer's ring already full.
    skipped: u64,
    /// Frames not recorded because the one already waiting showed the same world.
    held: u64,
    /// Why the thread stopped, once it has.
    stopped: Option<String>,
    /// What the swapchain came up as, for the log.
    describe: String,
}

impl WindowPresenter {
    /// Make a surface on `native`, build its swapchain, and start presenting on another
    /// thread. `size` is the window's size in pixels now.
    ///
    /// Any failure here — no surface extension, a queue that cannot present to this
    /// surface, a renderer that does not lend its raster — is returned before anything is
    /// left running, so the caller can fall back to the readback window.
    pub fn open<S: FrameSource>(
        gpu: Arc<Gpu>,
        src: &mut S,
        native: NativeWindow,
        size: (u32, u32),
    ) -> Result<WindowPresenter> {
        let raster_view = src
            .raster_view()
            .ok_or_else(|| anyhow!("this renderer does not lend its raster to a presenter"))?;
        let d = &gpu.device;
        let pool = src.command_pool();
        let slots = unsafe {
            d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(pool)
                    .command_buffer_count(SLOTS as u32),
            )
        }?;
        let stats = Arc::new(PresentStats::default());
        let size_cell = Arc::new(AtomicU64::new(pack(size)));
        let panel = match WindowPanel::open(
            gpu.clone(),
            native,
            raster_view,
            src.raster_size(),
            slots.clone(),
            size_cell.clone(),
            stats.clone(),
        ) {
            Ok(panel) => panel,
            Err(e) => {
                unsafe { d.free_command_buffers(pool, &slots) };
                return Err(e);
            }
        };
        let describe = panel.describe();
        // The recorder starts holding every slot; the thread is told so, and hands each
        // one back after its frame is presented.
        let lent: Vec<usize> = (0..SLOTS).collect();
        let free = lent.iter().copied().collect();
        let mail = Arc::new(Mailbox::new());
        let (tx, back) = channel();
        let (m, s) = (mail.clone(), stats.clone());
        let thread = std::thread::Builder::new()
            .name("cubarium-window".to_string())
            .spawn(move || {
                let mut panel = panel;
                if let Err(e) = present_loop(&mut panel, &m, &tx, &s, lent, POLL) {
                    let _ = tx.send(FromPresenter::Failed(format!("{e:#}")));
                }
                panel.destroy();
            })
            .context("starting the window's presenting thread")?;
        Ok(WindowPresenter {
            mail,
            back,
            stats,
            size: size_cell,
            thread: Some(thread),
            slots,
            free,
            skipped: 0,
            held: 0,
            stopped: None,
            describe,
        })
    }

    /// What the swapchain came up as: size, format, present mode and fit.
    pub fn describe(&self) -> &str {
        &self.describe
    }

    /// Presents that reached the screen.
    pub fn presented(&self) -> u64 {
        self.stats.shown()
    }

    /// Frames the recorder had nowhere to put.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Everything the thread has done so far, with this side's counts folded in.
    pub fn sample(&self) -> PresentSample {
        let mut sample = self.stats.snapshot();
        sample.skipped = self.skipped;
        sample.held = self.held;
        sample
    }

    /// Record one frame of the world and post it; `window` is the window's size in pixels
    /// now. Returns the GPU milliseconds the last **finished** frame's timestamps saw.
    ///
    /// This is the panel's recorder (`ShimPresenter::draw`) with no target: the present
    /// pass is the thread's to record, onto an image only it can acquire.
    pub fn draw<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        src: &mut S,
        frame: S::Frame<'_>,
        window: (u32, u32),
    ) -> Result<f64> {
        self.size.store(pack(window), Ordering::Relaxed);
        self.drain(src)?;
        let version = src.content_version();
        match next_frame(
            self.mail.pending(),
            version,
            self.free.front().copied(),
            src.frames_in_flight(),
            src.frame_capacity(),
        ) {
            Next::Keep => {
                self.held += 1;
                return Ok(src.gpu_ms(gpu));
            }
            Next::Drop => {
                self.skipped += 1;
                return Ok(src.gpu_ms(gpu));
            }
            // Newest wins: the waiting frame comes back, its upload is owed again, and
            // the fresher world is recorded into its slot.
            Next::Replace { .. } => {
                if let Some(frame) = self.mail.reclaim() {
                    src.frame_discarded();
                    self.free.push_front(frame.slot);
                }
            }
            Next::Record { .. } => {}
        }
        // The thread may have taken the waiting frame while that was decided.
        let Some(index) = self.free.pop_front() else {
            self.skipped += 1;
            return Ok(src.gpu_ms(gpu));
        };
        let cb = self.slots[index];
        unsafe {
            gpu.device
                .reset_command_buffer(cb, vk::CommandBufferResetFlags::empty())
        }?;
        src.record_frame(gpu, cb, frame, None)?;
        let frame = Frame {
            slot: index,
            redrew: src.redrew_last(),
            version,
        };
        if let Some(displaced) = self.mail.post(frame) {
            debug_assert!(false, "the recorder reclaims before it records");
            self.free.push_back(displaced.slot);
        }
        Ok(src.gpu_ms(gpu))
    }

    /// Take everything the thread has said since the last frame.
    fn drain<S: FrameSource>(&mut self, src: &mut S) -> Result<()> {
        loop {
            match self.back.try_recv() {
                Ok(FromPresenter::Retired) => src.frame_retired(),
                Ok(FromPresenter::Free(slot)) => self.free.push_back(slot),
                Ok(FromPresenter::Failed(why)) => {
                    self.stopped = Some(why.clone());
                    bail!("the window's presenting thread stopped: {why}");
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    let why = self
                        .stopped
                        .clone()
                        .unwrap_or_else(|| "it ended without saying why".to_string());
                    bail!("the window's presenting thread is gone: {why}");
                }
            }
        }
    }

    /// Stop the thread, which frees its swapchain and surface on the way out.
    ///
    /// Returns **false** when the thread did not stop within [`JOIN_WAIT`] — it is inside
    /// the driver, in a present that is not coming back — and has been left behind with
    /// everything it owns. The caller must then keep the native window alive (the surface
    /// is still on it) and let the process exit take both.
    pub fn destroy(&mut self, _gpu: &Gpu) -> bool {
        self.mail.quit();
        let Some(thread) = self.thread.take() else {
            return true;
        };
        let until = Instant::now() + JOIN_WAIT;
        while !thread.is_finished() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        if !thread.is_finished() {
            eprintln!(
                "cubarium window: the presenting thread did not stop in {:.0} s; leaving it \
                 to the process exit",
                JOIN_WAIT.as_secs_f64()
            );
            return false;
        }
        if thread.join().is_err() {
            eprintln!("cubarium window: the presenting thread panicked");
        }
        // The recorder's command buffers are the renderer's pool's, freed with it.
        true
    }
}

/// One swapchain and what hangs off each of its images.
struct Swapchain {
    handle: vk::SwapchainKHR,
    extent: (u32, u32),
    format: vk::Format,
    /// The window size it was built for; a different one asks for a rebuild.
    built_for: (u32, u32),
    targets: Vec<TargetImage>,
    /// One per image: signalled by that image's present pass, waited on by its present.
    rendered: Vec<vk::Semaphore>,
    pass: vk::RenderPass,
    pipeline: vk::Pipeline,
    fit: WindowFit,
}

/// The presenting thread's side: the swapchain and everything that draws onto it.
struct WindowPanel {
    gpu: Arc<Gpu>,
    surface_fn: ash::khr::surface::Instance,
    swapchain_fn: ash::khr::swapchain::Device,
    surface: vk::SurfaceKHR,
    mode: vk::PresentModeKHR,
    swap: Option<Swapchain>,
    /// An acquire or a present said the swapchain no longer matches the surface.
    stale: bool,
    /// The recorder's command buffers, by slot. This side only submits them.
    slots: Vec<vk::CommandBuffer>,
    world_fence: vk::Fence,
    /// This thread's own pool: a pool is externally synchronised, and the renderer's is
    /// the loop's.
    pool: vk::CommandPool,
    present_cb: vk::CommandBuffer,
    /// Created signalled; unsignalled only while a present pass is on the GPU.
    present_fence: vk::Fence,
    acquired: vk::Semaphore,
    pass: Option<PresentPass>,
    sampler: vk::Sampler,
    raster: (u32, u32),
    size: Arc<AtomicU64>,
    stats: Arc<PresentStats>,
    /// Slots whose world frame has finished, to hand back.
    freed: Vec<usize>,
}

impl WindowPanel {
    fn open(
        gpu: Arc<Gpu>,
        native: NativeWindow,
        raster_view: vk::ImageView,
        raster: (u32, u32),
        slots: Vec<vk::CommandBuffer>,
        size: Arc<AtomicU64>,
        stats: Arc<PresentStats>,
    ) -> Result<WindowPanel> {
        if !gpu.has_instance_extension(ash::khr::surface::NAME) {
            bail!("the Vulkan loader has no VK_KHR_surface");
        }
        let surface = create_surface(&gpu, native)?;
        let mut panel = WindowPanel {
            surface_fn: ash::khr::surface::Instance::new(&gpu.entry, &gpu.instance),
            swapchain_fn: ash::khr::swapchain::Device::new(&gpu.instance, &gpu.device),
            surface,
            mode: vk::PresentModeKHR::FIFO,
            swap: None,
            stale: false,
            slots,
            world_fence: vk::Fence::null(),
            pool: vk::CommandPool::null(),
            present_cb: vk::CommandBuffer::null(),
            present_fence: vk::Fence::null(),
            acquired: vk::Semaphore::null(),
            pass: None,
            sampler: vk::Sampler::null(),
            raster,
            size,
            stats,
            freed: Vec::new(),
            gpu,
        };
        if let Err(e) = panel.init(raster_view) {
            panel.destroy();
            return Err(e);
        }
        Ok(panel)
    }

    /// Everything but the surface, then the first swapchain.
    fn init(&mut self, raster_view: vk::ImageView) -> Result<()> {
        let gpu = self.gpu.clone();
        let d = &gpu.device;
        let supported = unsafe {
            self.surface_fn.get_physical_device_surface_support(
                gpu.pdev,
                gpu.queue_family,
                self.surface,
            )
        }?;
        if !supported {
            bail!(
                "{}'s graphics queue cannot present to this window",
                gpu.name
            );
        }
        let modes = unsafe {
            self.surface_fn
                .get_physical_device_surface_present_modes(gpu.pdev, self.surface)
        }?;
        // Mailbox where the driver offers it: the newest frame replaces a queued one and
        // the present does not wait for a vsync. FIFO otherwise, which every driver has.
        // `CUBARIUM_WINDOW_FIFO=1` forces FIFO where mailbox exists: the case this thread
        // is for (a FIFO present to a hidden window that never returns), on demand.
        let fifo = std::env::var("CUBARIUM_WINDOW_FIFO").as_deref() == Ok("1");
        self.mode = if !fifo && modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else {
            vk::PresentModeKHR::FIFO
        };
        unsafe {
            self.pool = d.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(gpu.queue_family),
                None,
            )?;
            self.present_cb = d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(self.pool)
                    .command_buffer_count(1),
            )?[0];
            self.world_fence = d.create_fence(&vk::FenceCreateInfo::default(), None)?;
            self.present_fence = d.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )?;
            self.acquired = d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
            // The upscale is `texelFetch`, so only the address modes could ever come
            // from this; nearest, like the renderer's own.
            self.sampler = d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )?;
        }
        self.pass = Some(PresentPass::new(&gpu, raster_view, self.sampler)?);
        self.rebuild(unpack(self.size.load(Ordering::Relaxed)))?;
        if self.swap.is_none() {
            bail!("the window has no area to make a swapchain for");
        }
        Ok(())
    }

    fn describe(&self) -> String {
        let Some(swap) = &self.swap else {
            return "no swapchain".to_string();
        };
        describe(swap, self.mode)
    }

    /// Build the swapchain for the window's size now, replacing the old one.
    fn rebuild(&mut self, want: (u32, u32)) -> Result<()> {
        let gpu = self.gpu.clone();
        let d = &gpu.device;
        // Nothing may still use the old images or their semaphores: our last present pass,
        // and the present that waits on it. Rebuilds are rare (a resize), so the whole
        // queue is drained rather than reasoned about.
        gpu.with_queue(|q| unsafe { d.queue_wait_idle(q) })?;
        let caps = unsafe {
            self.surface_fn
                .get_physical_device_surface_capabilities(gpu.pdev, self.surface)
        }?;
        // Wayland's surface has no size of its own: the swapchain's is the window's.
        let extent = if caps.current_extent.width == u32::MAX {
            (
                want.0
                    .clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                want.1
                    .clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            )
        } else {
            (caps.current_extent.width, caps.current_extent.height)
        };
        if extent.0 == 0 || extent.1 == 0 {
            // Minimised: nothing to present to until it has an area again.
            if let Some(old) = self.swap.take() {
                self.drop_swapchain(old);
            }
            self.stale = false;
            return Ok(());
        }
        let formats = unsafe {
            self.surface_fn
                .get_physical_device_surface_formats(gpu.pdev, self.surface)
        }?;
        // An `_SRGB` swapchain makes the encode the hardware's, as on the panel; a UNORM
        // one falls back to the present shader's own.
        let chosen = formats
            .iter()
            .find(|f| {
                matches!(
                    f.format,
                    vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB
                ) && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .or_else(|| {
                formats
                    .iter()
                    .find(|f| f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
            })
            .or_else(|| formats.first())
            .copied()
            .ok_or_else(|| anyhow!("the surface offers no formats"))?;
        let srgb = matches!(
            chosen.format,
            vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB
        );
        let wanted = if self.mode == vk::PresentModeKHR::MAILBOX {
            (caps.min_image_count + 1).max(3)
        } else {
            caps.min_image_count + 1
        };
        let count = match caps.max_image_count {
            0 => wanted,
            max => wanted.min(max),
        };
        let alpha = [
            vk::CompositeAlphaFlagsKHR::OPAQUE,
            vk::CompositeAlphaFlagsKHR::INHERIT,
            vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
        ]
        .into_iter()
        .find(|a| caps.supported_composite_alpha.contains(*a))
        .unwrap_or(vk::CompositeAlphaFlagsKHR::OPAQUE);
        let old = self.swap.take();
        let handle = unsafe {
            self.swapchain_fn.create_swapchain(
                &vk::SwapchainCreateInfoKHR::default()
                    .surface(self.surface)
                    .min_image_count(count)
                    .image_format(chosen.format)
                    .image_color_space(chosen.color_space)
                    .image_extent(vk::Extent2D {
                        width: extent.0,
                        height: extent.1,
                    })
                    .image_array_layers(1)
                    .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .pre_transform(caps.current_transform)
                    .composite_alpha(alpha)
                    .present_mode(self.mode)
                    .clipped(true)
                    .old_swapchain(old.as_ref().map_or(vk::SwapchainKHR::null(), |s| s.handle)),
                None,
            )
        };
        if let Some(old) = old {
            self.drop_swapchain(old);
        }
        let handle = handle.context("vkCreateSwapchainKHR")?;
        let (pass, pipeline) = self
            .pass
            .as_mut()
            .expect("the present pass is built before the first swapchain")
            .entry(&gpu, chosen.format, vk::ImageLayout::PRESENT_SRC_KHR)?;
        let mut swap = Swapchain {
            handle,
            extent,
            format: chosen.format,
            built_for: want,
            targets: Vec::new(),
            rendered: Vec::new(),
            pass,
            pipeline,
            fit: WindowFit::fit(self.raster, extent, !srgb),
        };
        let images = match unsafe { self.swapchain_fn.get_swapchain_images(handle) } {
            Ok(images) => images,
            Err(e) => {
                self.drop_swapchain(swap);
                return Err(anyhow!(e).context("vkGetSwapchainImagesKHR"));
            }
        };
        for image in images {
            let made = (|| -> Result<(TargetImage, vk::Semaphore)> {
                let view = gpu.view(image, chosen.format)?;
                let framebuffer = framebuffer(d, pass, view, extent.0, extent.1)?;
                let semaphore =
                    unsafe { d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }?;
                Ok((
                    TargetImage {
                        image,
                        view,
                        framebuffer,
                    },
                    semaphore,
                ))
            })();
            match made {
                Ok((target, semaphore)) => {
                    swap.targets.push(target);
                    swap.rendered.push(semaphore);
                }
                Err(e) => {
                    self.drop_swapchain(swap);
                    return Err(e);
                }
            }
        }
        eprintln!("cubarium window: {}", describe(&swap, self.mode));
        self.swap = Some(swap);
        self.stale = false;
        Ok(())
    }

    /// Free one swapchain and everything hanging off it. Nothing may still use it.
    fn drop_swapchain(&self, swap: Swapchain) {
        let d = &self.gpu.device;
        unsafe {
            for (target, semaphore) in swap.targets.iter().zip(&swap.rendered) {
                d.destroy_framebuffer(target.framebuffer, None);
                d.destroy_image_view(target.view, None);
                d.destroy_semaphore(*semaphore, None);
            }
            self.swapchain_fn.destroy_swapchain(swap.handle, None);
        }
    }

    /// Put the raster on the window: acquire an image, run the present pass onto it,
    /// present it. False when nothing reached the screen — no image in time, no area, or
    /// a swapchain that has just gone out of date — which is a dropped present and not a
    /// failure.
    fn show(&mut self) -> Result<bool> {
        let want = unpack(self.size.load(Ordering::Relaxed));
        let resized = self.swap.as_ref().is_none_or(|s| s.built_for != want);
        if self.stale || resized {
            self.rebuild(want)?;
        }
        let Some(swap) = &self.swap else {
            return Ok(false);
        };
        let gpu = &self.gpu;
        let d = &gpu.device;
        let index = match unsafe {
            self.swapchain_fn.acquire_next_image(
                swap.handle,
                ACQUIRE_TIMEOUT.as_nanos() as u64,
                self.acquired,
                vk::Fence::null(),
            )
        } {
            Ok((index, suboptimal)) => {
                // Still an image, and its semaphore will be signalled: present it and
                // rebuild before the next one.
                self.stale |= suboptimal;
                index as usize
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.stale = true;
                return Ok(false);
            }
            Err(vk::Result::TIMEOUT | vk::Result::NOT_READY) => return Ok(false),
            Err(e) => return Err(anyhow!(e).context("vkAcquireNextImageKHR")),
        };
        let pass = self
            .pass
            .as_ref()
            .expect("the present pass outlives every swapchain");
        let cb = self.present_cb;
        unsafe {
            d.wait_for_fences(&[self.present_fence], true, u64::MAX)?;
            d.reset_fences(&[self.present_fence])?;
            d.reset_command_buffer(cb, vk::CommandBufferResetFlags::empty())?;
            d.begin_command_buffer(
                cb,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            pass.record_push(
                d,
                cb,
                &swap.targets[index],
                swap.extent,
                swap.fit.push(),
                swap.pass,
                swap.pipeline,
            );
            d.end_command_buffer(cb)?;
        }
        let waits = [self.acquired];
        // The render pass's layout transition happens at the top of the pipe, so the
        // wait covers every stage rather than only colour output.
        let stages = [vk::PipelineStageFlags::ALL_COMMANDS];
        let signals = [swap.rendered[index]];
        let cbs = [cb];
        gpu.submit(
            &[vk::SubmitInfo::default()
                .wait_semaphores(&waits)
                .wait_dst_stage_mask(&stages)
                .command_buffers(&cbs)
                .signal_semaphores(&signals)],
            self.present_fence,
        )?;
        let swapchains = [swap.handle];
        let indices = [index as u32];
        // The present is a queue operation, so it holds the queue's lock. Nothing on the
        // loop submits in a plain window run; a `--gpu-web-rate` or `--gpu-capture`
        // readback waits for it.
        let presented = gpu.with_queue(|q| unsafe {
            self.swapchain_fn.queue_present(
                q,
                &vk::PresentInfoKHR::default()
                    .wait_semaphores(&signals)
                    .swapchains(&swapchains)
                    .image_indices(&indices),
            )
        });
        match presented {
            Ok(suboptimal) => {
                self.stale |= suboptimal;
                Ok(true)
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.stale = true;
                Ok(false)
            }
            Err(e) => Err(anyhow!(e).context("vkQueuePresentKHR")),
        }
    }

    /// Free everything this side made, surface last. Runs on the presenting thread as it
    /// ends, or on the loop when opening failed half-way.
    fn destroy(&mut self) {
        let gpu = self.gpu.clone();
        let d = &gpu.device;
        let _ = gpu.with_queue(|q| unsafe { d.queue_wait_idle(q) });
        if let Some(swap) = self.swap.take() {
            self.drop_swapchain(swap);
        }
        if let Some(pass) = self.pass.take() {
            pass.destroy(&gpu);
        }
        unsafe {
            d.destroy_sampler(self.sampler, None);
            d.destroy_semaphore(self.acquired, None);
            d.destroy_fence(self.present_fence, None);
            d.destroy_fence(self.world_fence, None);
            d.destroy_command_pool(self.pool, None);
            self.surface_fn.destroy_surface(self.surface, None);
        }
        self.sampler = vk::Sampler::null();
        self.acquired = vk::Semaphore::null();
        self.present_fence = vk::Fence::null();
        self.world_fence = vk::Fence::null();
        self.pool = vk::CommandPool::null();
        self.surface = vk::SurfaceKHR::null();
    }
}

impl Panel for WindowPanel {
    fn submit(&mut self, slot: usize) -> Result<()> {
        let d = &self.gpu.device;
        unsafe {
            // The last present pass may still be reading the raster this frame draws over.
            d.wait_for_fences(&[self.present_fence], true, u64::MAX)?;
            d.reset_fences(&[self.world_fence])?;
        }
        let cbs = [self.slots[slot]];
        self.gpu
            .submit(&[vk::SubmitInfo::default().command_buffers(&cbs)], self.world_fence)
    }

    fn wait(&mut self, slot: usize) -> Result<()> {
        unsafe {
            self.gpu
                .device
                .wait_for_fences(&[self.world_fence], true, u64::MAX)
        }?;
        // Its command buffer is the recorder's again once the frame is shown.
        self.freed.push(slot);
        Ok(())
    }

    fn present(&mut self, _slot: usize) -> Result<()> {
        if !self.show()? {
            self.stats.add_unshown();
        }
        Ok(())
    }

    fn released(&mut self, _block: bool) -> Result<Vec<usize>> {
        // A slot is free the moment its world frame has finished, so there is never
        // anything to wait for here.
        Ok(std::mem::take(&mut self.freed))
    }
}

fn describe(swap: &Swapchain, mode: vk::PresentModeKHR) -> String {
    let fit = &swap.fit;
    let scale = if fit.factor >= 1 {
        format!("x{} upscale", fit.factor)
    } else {
        format!("1/{:.2} nearest downscale", fit.step)
    };
    format!(
        "swapchain {}x{} {:?} {:?}, {} images; raster {} to {}x{} at ({}, {}), sRGB by {}",
        swap.extent.0,
        swap.extent.1,
        swap.format,
        mode,
        swap.targets.len(),
        scale,
        fit.size.0,
        fit.size.1,
        fit.origin.0,
        fit.origin.1,
        if fit.encode_srgb {
            "the shader"
        } else {
            "the attachment"
        },
    )
}

/// Whether this device can present to a Wayland surface on `display`, asked of the
/// connection alone — **before** the host makes a surface or a toplevel.
///
/// A window that fails after it exists is replaced by the readback window, and that is a
/// second window from one process. `scripts/hidden.sh`'s per-process rule reaches only
/// the first window a process maps, so a second one lands on the desktop. Everything
/// that can be decided without a window is decided here, first.
pub fn check_wayland(gpu: &Gpu, display: *mut c_void) -> Result<()> {
    if !gpu.has_instance_extension(ash::khr::surface::NAME)
        || !gpu.has_instance_extension(ash::khr::wayland_surface::NAME)
    {
        bail!("the Vulkan loader has no VK_KHR_wayland_surface");
    }
    let f = ash::khr::wayland_surface::Instance::new(&gpu.entry, &gpu.instance);
    // SAFETY of the cast: `display` is the host's live `wl_display *`.
    let supported = unsafe {
        f.get_physical_device_wayland_presentation_support(
            gpu.pdev,
            gpu.queue_family,
            &mut *(display as *mut vk::wl_display),
        )
    };
    if !supported {
        bail!(
            "{}'s graphics queue cannot present to this Wayland display",
            gpu.name
        );
    }
    Ok(())
}

/// A Vulkan surface on the native window.
fn create_surface(gpu: &Gpu, native: NativeWindow) -> Result<vk::SurfaceKHR> {
    match native {
        NativeWindow::Wayland { display, surface } => {
            if !gpu.has_instance_extension(ash::khr::wayland_surface::NAME) {
                bail!("the Vulkan loader has no VK_KHR_wayland_surface");
            }
            let f = ash::khr::wayland_surface::Instance::new(&gpu.entry, &gpu.instance);
            Ok(unsafe {
                f.create_wayland_surface(
                    &vk::WaylandSurfaceCreateInfoKHR::default()
                        .display(display)
                        .surface(surface),
                    None,
                )
            }
            .context("vkCreateWaylandSurfaceKHR")?)
        }
        NativeWindow::Xlib { display, window } => {
            if !gpu.has_instance_extension(ash::khr::xlib_surface::NAME) {
                bail!("the Vulkan loader has no VK_KHR_xlib_surface");
            }
            let f = ash::khr::xlib_surface::Instance::new(&gpu.entry, &gpu.instance);
            Ok(unsafe {
                f.create_xlib_surface(
                    &vk::XlibSurfaceCreateInfoKHR::default()
                        .dpy(display)
                        .window(window as vk::Window),
                    None,
                )
            }
            .context("vkCreateXlibSurfaceKHR")?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The loop hands the window's size over as one word; both halves come back.
    #[test]
    fn a_window_size_survives_the_trip_to_the_thread() {
        for size in [(0, 0), (640, 400), (3328, 2048), (u32::MAX, 1)] {
            assert_eq!(unpack(pack(size)), size);
        }
    }
}
