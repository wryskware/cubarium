//! Surface presentation with an owned window handle. The host owns its event
//! loop and supplies physical pixel sizes; it never lends raw window pointers.
#![forbid(unsafe_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, bail, ensure};

use super::{Gpu, PresentPass};

pub struct WindowPresenter {
    gpu: Arc<Gpu>,
    owner: Arc<dyn wgpu::DisplayAndWindowHandle>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    present: PresentPass,
    size: (u32, u32),
    configure_needed: bool,
    acquired: Arc<AtomicBool>,
    presented: u64,
    skipped: u64,
}

/// An acquired surface image. Dropping it discards the image and releases the
/// presenter's lease. No resize/configure is allowed while this lease is live.
pub struct WindowFrame {
    texture: Option<wgpu::SurfaceTexture>,
    view: wgpu::TextureView,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    acquired: Arc<AtomicBool>,
}

impl WindowFrame {
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

impl Drop for WindowFrame {
    fn drop(&mut self) {
        // Release the actual image before another acquire can reconfigure.
        self.texture.take();
        self.acquired.store(false, Ordering::Release);
    }
}

impl WindowPresenter {
    /// Construct this on the platform's window thread (required by Metal).
    /// `Arc<winit::Window>` is a supported owner; wgpu retains it in the surface.
    pub fn open<W: wgpu::DisplayAndWindowHandle + 'static>(
        gpu: Arc<Gpu>,
        owner: Arc<W>,
        physical_size: (u32, u32),
    ) -> Result<Self> {
        let surface = gpu
            .instance
            .create_surface(owner.clone())
            .context("create owned window surface")?;
        Self::from_surface(gpu, owner, surface, physical_size)
    }

    /// Reuse the surface that constrained adapter selection in `Gpu::open_for_window`.
    pub fn from_surface<W: wgpu::DisplayAndWindowHandle + 'static>(
        gpu: Arc<Gpu>,
        owner: Arc<W>,
        surface: wgpu::Surface<'static>,
        physical_size: (u32, u32),
    ) -> Result<Self> {
        let owner: Arc<dyn wgpu::DisplayAndWindowHandle> = owner;
        let config = configuration(&gpu, &surface, physical_size)?;
        let present = PresentPass::new(&gpu, config.format);
        Ok(Self {
            gpu,
            owner,
            surface,
            config,
            present,
            size: physical_size,
            configure_needed: true,
            acquired: Arc::new(AtomicBool::new(false)),
            presented: 0,
            skipped: 0,
        })
    }

    /// Store the physical extent; configuration is deferred until no image is
    /// acquired. A minimized window remains unconfigured until its area returns.
    pub fn resize(&mut self, physical_size: (u32, u32)) {
        if self.size != physical_size {
            self.size = physical_size;
            self.configure_needed = true;
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }
    pub fn presented(&self) -> u64 {
        self.presented
    }
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Acquire before recording the world. None means a zero-area, occluded or
    /// timed-out window, so the caller need not claim a renderer staging slot.
    /// An outdated/lost surface gets one repair and one acquisition retry.
    pub fn acquire(&mut self) -> Result<Option<WindowFrame>> {
        ensure!(
            !self.acquired.load(Ordering::Acquire),
            "a window frame is still acquired"
        );
        if self.size.0 == 0 || self.size.1 == 0 {
            self.skipped += 1;
            return Ok(None);
        }
        self.configure_if_needed();
        for attempt in 0..2 {
            let texture = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture) => texture,
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                    self.configure_needed = true;
                    texture
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    self.skipped += 1;
                    return Ok(None);
                }
                wgpu::CurrentSurfaceTexture::Outdated if attempt == 0 => {
                    self.configure_needed = true;
                    self.configure_if_needed();
                    continue;
                }
                wgpu::CurrentSurfaceTexture::Lost if attempt == 0 => {
                    self.surface = self
                        .gpu
                        .instance
                        .create_surface(self.owner.clone())
                        .context("recreate lost window surface")?;
                    let config = configuration(&self.gpu, &self.surface, self.size)?;
                    if config.format != self.config.format {
                        self.present = PresentPass::new(&self.gpu, config.format);
                    }
                    self.config = config;
                    self.configure_needed = true;
                    self.configure_if_needed();
                    continue;
                }
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                    self.configure_needed = true;
                    self.skipped += 1;
                    return Ok(None);
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    bail!("wgpu rejected window surface acquisition")
                }
            };
            let view = texture.texture.create_view(&Default::default());
            self.acquired.store(true, Ordering::Release);
            return Ok(Some(WindowFrame {
                texture: Some(texture),
                view,
                size: self.size,
                format: self.config.format,
                acquired: self.acquired.clone(),
            }));
        }
        unreachable!("both acquisition attempts return or retry once")
    }

    pub fn record_present(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        raster: &wgpu::TextureView,
        raster_size: (u32, u32),
        frame: &WindowFrame,
    ) -> Result<()> {
        ensure!(
            Arc::ptr_eq(&self.acquired, &frame.acquired),
            "frame belongs to another presenter"
        );
        ensure!(
            raster_size.0 > 0 && raster_size.1 > 0,
            "window raster is empty"
        );
        let transform =
            crate::present::WindowFit::fit(raster_size, frame.size, !frame.format.is_srgb()).push();
        self.present
            .record(&self.gpu, encoder, raster, &frame.view, transform);
        Ok(())
    }

    /// Submit the world and presentation together, then present that image.
    /// The caller uses the returned index/callback for renderer retirement.
    pub fn submit(
        &mut self,
        mut frame: WindowFrame,
        encoder: wgpu::CommandEncoder,
    ) -> Result<wgpu::SubmissionIndex> {
        ensure!(
            Arc::ptr_eq(&self.acquired, &frame.acquired),
            "frame belongs to another presenter"
        );
        let submission = self.gpu.queue.submit([encoder.finish()]);
        self.gpu
            .queue
            .present(frame.texture.take().expect("acquired window image"));
        self.presented += 1;
        Ok(submission)
    }

    fn configure_if_needed(&mut self) {
        if self.configure_needed {
            self.config.width = self.size.0;
            self.config.height = self.size.1;
            self.surface.configure(&self.gpu.device, &self.config);
            self.configure_needed = false;
        }
    }
}

fn configuration(
    gpu: &Gpu,
    surface: &wgpu::Surface<'_>,
    size: (u32, u32),
) -> Result<wgpu::SurfaceConfiguration> {
    let mut config = surface
        .get_default_config(&gpu.adapter, size.0.max(1), size.1.max(1))
        .context("selected adapter cannot present to this window")?;
    let caps = surface.get_capabilities(&gpu.adapter);
    config.format = caps
        .formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .unwrap_or(config.format);
    config.present_mode = wgpu::PresentMode::Fifo;
    if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
        config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
    }
    Ok(config)
}
