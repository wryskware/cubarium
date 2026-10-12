//! Safe development-window adapter, independent of the renderer's frame trait.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use cubarium_gpu::wgpu_backend::{FrameSource, Gpu};
use cubarium_gpu::wgpu_backend::window::WindowPresenter;

use super::window_host::WindowHost;

pub struct WgpuWindowTarget {
    // Drop the presenter/surface before its event loop and window host.
    presenter: WindowPresenter,
    host: WindowHost,
    gpu: Arc<Gpu>,
    open: bool,
    save: bool,
    flight: VecDeque<(wgpu::SubmissionIndex, Arc<AtomicBool>)>,
}

impl WgpuWindowTarget {
    pub fn prepare(title: &str, raster_size: (u32, u32)) -> Result<(Arc<Gpu>, Self)> {
        let (rw, rh) = (raster_size.0.max(1), raster_size.1.max(1));
        let zoom = ((1600 / rw).min(900 / rh)).max(1);
        let host = WindowHost::open(title, (rw * zoom, rh * zoom))?;
        let descriptor = wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(host.display()));
        let (gpu, surface) = Gpu::open_for_window(descriptor, host.window())?;
        let gpu = Arc::new(gpu);
        let presenter = WindowPresenter::from_surface(gpu.clone(), host.window(), surface, host.size())?;
        let target = Self { presenter, host, gpu: gpu.clone(), open: true, save: false, flight: VecDeque::new() };
        Ok((gpu, target))
    }

    /// Acquire, record, submit and present inside Winit's redraw callback. If no
    /// image is available, the record closure is never called, so no world-frame
    /// staging slot or upload is claimed. Submitted frames retire FIFO after
    /// queue completion; a discarded recording returns its renderer slot.
    pub fn draw<S: FrameSource>(
        &mut self,
        src: &mut S,
        world: S::Frame<'_>,
    ) -> Result<f64> {
        self.poll(src)?;
        self.open = self.host.is_open();
        if !self.open {
            return Ok(src.gpu_ms(&self.gpu));
        }
        let window = self.host.window();
        let presenter = &mut self.presenter;
        let gpu = &self.gpu;
        let flight = &mut self.flight;
        let mut world = Some(world);
        let result = self.host.draw(|| {
            let size = window.inner_size();
            presenter.resize((size.width, size.height));
            let Some(frame) = presenter.acquire()? else {
                return Ok(());
            };
            let mut encoder = gpu.encoder("development window world and presentation");
            // Additional redraws (including live resize) reuse the raster. Keep
            // pumping events even while all world-frame slots are occupied.
            let recorded = world.is_some() && src.frames_in_flight() < src.frame_capacity();
            if recorded {
                src.record_frame(gpu, &mut encoder, world.take().expect("available world frame"), None)?;
            }
            if let Err(error) = presenter.record_present(&mut encoder, src.raster_view(), src.raster_size(), &frame) {
                if recorded { src.frame_discarded(); }
                return Err(error);
            }
            let index = match presenter.submit(frame, encoder) {
                Ok(index) => index,
                Err(error) => {
                    if recorded { src.frame_discarded(); }
                    return Err(error);
                }
            };
            if recorded {
                // Register immediately: a later redraw error in this same pump
                // must not lose retirement of an already submitted world frame.
                let ready = Arc::new(AtomicBool::new(false));
                let completed = ready.clone();
                gpu.queue.on_submitted_work_done(move || completed.store(true, Ordering::Release));
                flight.push_back((index, ready));
            }
            Ok(())
        });
        self.save |= self.host.take_save();
        self.open = self.host.is_open();
        result?;
        Ok(src.gpu_ms(&self.gpu))
    }

    fn poll<S: FrameSource>(&mut self, src: &mut S) -> Result<()> {
        self.gpu.device.poll(wgpu::PollType::Poll)?;
        while self.flight.front().is_some_and(|(_, ready)| ready.load(Ordering::Acquire)) {
            self.flight.pop_front();
            src.frame_retired();
        }
        Ok(())
    }

    pub fn finish<S: FrameSource>(&mut self, src: &mut S) -> Result<()> {
        if let Some((index, _)) = self.flight.back() {
            self.gpu.device.poll(wgpu::PollType::Wait {
                submission_index: Some(index.clone()),
                timeout: Some(Duration::from_secs(30)),
            })?;
        }
        self.poll(src)
    }

    pub fn should_quit(&self) -> bool {
        !self.open
    }

    pub fn take_save(&mut self) -> bool {
        std::mem::take(&mut self.save)
    }

    pub fn presented(&self) -> (u64, u64) {
        (self.presenter.presented(), self.presenter.skipped())
    }
}
