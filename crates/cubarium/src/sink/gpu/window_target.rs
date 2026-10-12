//! WSI runs on the presenting thread. Recording posts one replaceable frame.
#![forbid(unsafe_code)]

use super::window_host::WindowHost;
use anyhow::{Context, Result, ensure};
use cubarium_gpu::wgpu_backend::window::WindowPresenter;
use cubarium_gpu::wgpu_backend::{FrameSource, Gpu};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

struct Posted {
    slot: usize,
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    size: (u32, u32),
    version: Option<u64>,
}
struct Waiting<T> {
    frame: Option<T>,
    quit: bool,
}
struct Mailbox<T> {
    state: Mutex<Waiting<T>>,
    moved: Condvar,
}
impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Self {
            state: Mutex::new(Waiting {
                frame: None,
                quit: false,
            }),
            moved: Condvar::new(),
        }
    }
}
impl<T> Mailbox<T> {
    fn reclaim(&self) -> Option<T> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .frame
            .take()
    }
    fn post(&self, frame: T) -> Result<()> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        ensure!(
            !state.quit && state.frame.is_none(),
            "window mailbox is unavailable"
        );
        state.frame = Some(frame);
        self.moved.notify_one();
        Ok(())
    }
    fn quit(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).quit = true;
        self.moved.notify_all();
    }
}

enum Completed {
    Retired(usize),
    Failed(String),
    Abandoned(usize),
    Stopped,
}
pub struct WgpuWindowTarget {
    gpu: Arc<Gpu>,
    mailbox: Arc<Mailbox<Posted>>,
    receiver: mpsc::Receiver<Completed>,
    worker: Option<JoinHandle<()>>,
    free: Vec<usize>,
    abandoned: Vec<usize>,
    stopped: bool,
    images: Vec<wgpu::Texture>,
    views: Vec<wgpu::TextureView>,
    size: (u32, u32),
    open: Arc<AtomicBool>,
    save: Arc<AtomicBool>,
    presented: Arc<AtomicU64>,
    skipped: Arc<AtomicU64>,
}
impl WgpuWindowTarget {
    pub fn prepare(title: &str, raster_size: (u32, u32)) -> Result<(Arc<Gpu>, Self)> {
        crate::sink::hidden::window_allowed()?;
        let title = title.to_owned();
        let mailbox = Arc::new(Mailbox::<Posted>::default());
        let open = Arc::new(AtomicBool::new(true));
        let save = Arc::new(AtomicBool::new(false));
        let presented = Arc::new(AtomicU64::new(0));
        let skipped = Arc::new(AtomicU64::new(0));
        let (ready, initial) = mpsc::sync_channel(1);
        let (completed, receiver) = mpsc::channel();
        let (postbox, is_open, save_key, shown, missed) = (
            mailbox.clone(),
            open.clone(),
            save.clone(),
            presented.clone(),
            skipped.clone(),
        );
        let worker = std::thread::Builder::new()
            .name("cubarium-window-present".into())
            .spawn(move || {
                let setup = (|| -> Result<_> {
                    let (rw, rh) = (raster_size.0.max(1), raster_size.1.max(1));
                    let zoom = ((1600 / rw).min(900 / rh)).max(1);
                    let host = WindowHost::open(&title, (rw * zoom, rh * zoom))?;
                    let descriptor = wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                        Box::new(host.display()),
                    );
                    let (gpu, surface) = Gpu::open_for_window(descriptor, host.window())?;
                    let gpu = Arc::new(gpu);
                    let presenter = WindowPresenter::from_surface(
                        gpu.clone(),
                        host.window(),
                        surface,
                        host.size(),
                    )?;
                    Ok((host, gpu, presenter))
                })();
                let (mut host, gpu, mut presenter) = match setup {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = ready.send(Err(e));
                        is_open.store(false, Ordering::Release);
                        return;
                    }
                };
                if ready.send(Ok(gpu.clone())).is_err() {
                    return;
                }
                let window = host.window();
                let mut held: Option<Posted> = None;
                let run = (|| -> Result<()> {
                    loop {
                        {
                            let mut state = postbox.state.lock().unwrap_or_else(|e| e.into_inner());
                            if state.quit {
                                break;
                            }
                            if held.is_none() {
                                held = state.frame.take();
                            }
                        }
                        if !host.pump()? {
                            break;
                        }
                        save_key.fetch_or(host.take_save(), Ordering::Release);
                        if held.is_some() {
                            host.draw(|| {
                                if held.is_none() {
                                    return Ok(());
                                }
                                let size = window.inner_size();
                                presenter.resize((size.width, size.height));
                                let Some(surface) = presenter.acquire()? else {
                                    missed.fetch_add(1, Ordering::Relaxed);
                                    return Ok(());
                                };
                                let mut frame = held.take().unwrap();
                                if let Err(e) = presenter.record_present(
                                    &mut frame.encoder,
                                    &frame.view,
                                    frame.size,
                                    &surface,
                                ) {
                                    let slot = frame.slot;
                                    drop(frame);
                                    let _ = completed.send(Completed::Abandoned(slot));
                                    return Err(e);
                                }
                                if postbox.state.lock().unwrap_or_else(|e| e.into_inner()).quit {
                                    let slot = frame.slot;
                                    drop(frame);
                                    let _ = completed.send(Completed::Abandoned(slot));
                                    return Ok(());
                                }
                                let slot = frame.slot;
                                let on_done = completed.clone();
                                if let Err(error) = presenter.submit_with_callback(
                                    surface,
                                    frame.encoder,
                                    move || {
                                        let _ = on_done.send(Completed::Retired(slot));
                                    },
                                ) {
                                    let _ = completed.send(Completed::Abandoned(slot));
                                    return Err(error);
                                }
                                shown.fetch_add(1, Ordering::Relaxed);
                                Ok(())
                            })?;
                        }
                        if held.is_none() {
                            let state = postbox.state.lock().unwrap_or_else(|e| e.into_inner());
                            if state.frame.is_none() && !state.quit {
                                let _ = postbox
                                    .moved
                                    .wait_timeout(state, Duration::from_millis(10))
                                    .unwrap_or_else(|e| e.into_inner());
                            }
                        }
                        gpu.device.poll(wgpu::PollType::Poll)?;
                    }
                    Ok(())
                })();
                if let Err(e) = run {
                    let _ = completed.send(Completed::Failed(format!("{e:#}")));
                }
                let _ = gpu.device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(30)),
                });
                if let Some(frame) = held.take() {
                    let slot = frame.slot;
                    drop(frame);
                    let _ = completed.send(Completed::Abandoned(slot));
                }
                is_open.store(false, Ordering::Release);
                let _ = completed.send(Completed::Stopped);
            })
            .context("start window presenting thread")?;
        let gpu = initial
            .recv()
            .context("window thread stopped during initialization")??;
        let size = raster_size;
        let images: Vec<_> = (0..2)
            .map(|_| {
                gpu.texture(
                    "window raster slot",
                    wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    wgpu::TextureDimension::D2,
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                )
            })
            .collect();
        let views = images
            .iter()
            .map(|t| t.create_view(&Default::default()))
            .collect();
        let target = Self {
            gpu: gpu.clone(),
            mailbox,
            receiver,
            worker: Some(worker),
            free: vec![1, 0],
            abandoned: vec![],
            stopped: false,
            images,
            views,
            size,
            open,
            save,
            presented,
            skipped,
        };
        Ok((gpu, target))
    }
    pub fn draw<S: FrameSource>(&mut self, src: &mut S, frame: S::Frame<'_>) -> Result<f64> {
        self.poll(src)?;
        if !self.open.load(Ordering::Acquire) {
            return Ok(src.gpu_ms(&self.gpu));
        }
        // Reclaim before recording, so rollback always targets the newest frame.
        if let Some(waiting) = self.mailbox.reclaim() {
            if waiting.version.is_some() && waiting.version == src.content_version() {
                self.mailbox.post(waiting)?;
                self.skipped.fetch_add(1, Ordering::Relaxed);
                return Ok(src.gpu_ms(&self.gpu));
            }
            let slot = waiting.slot;
            drop(waiting);
            src.frame_discarded();
            self.free.push(slot);
        }
        let Some(slot) = self.free.pop() else {
            self.skipped.fetch_add(1, Ordering::Relaxed);
            return Ok(src.gpu_ms(&self.gpu));
        };
        if src.frames_in_flight() >= src.frame_capacity() {
            self.free.push(slot);
            return Ok(src.gpu_ms(&self.gpu));
        }
        ensure!(
            src.raster_size() == self.size,
            "window raster extent changed"
        );
        let mut encoder = self.gpu.encoder("window world snapshot");
        if let Err(e) = src.record_frame(&self.gpu, &mut encoder, frame, None) {
            self.free.push(slot);
            return Err(e);
        }
        encoder.copy_texture_to_texture(
            src.raster_texture().as_image_copy(),
            self.images[slot].as_image_copy(),
            self.images[slot].size(),
        );
        let posted = Posted {
            slot,
            encoder,
            view: self.views[slot].clone(),
            size: self.size,
            version: src.content_version(),
        };
        if let Err(e) = self.mailbox.post(posted) {
            src.frame_discarded();
            self.free.push(slot);
            return Err(e);
        }
        Ok(src.gpu_ms(&self.gpu))
    }
    fn poll<S: FrameSource>(&mut self, src: &mut S) -> Result<()> {
        self.gpu.device.poll(wgpu::PollType::Poll)?;
        while let Ok(done) = self.receiver.try_recv() {
            match done {
                Completed::Retired(slot) => {
                    src.frame_retired();
                    self.free.push(slot);
                }
                Completed::Abandoned(slot) => {
                    self.abandoned.push(slot);
                }
                Completed::Stopped => {
                    self.stopped = true;
                    self.open.store(false, Ordering::Release);
                }
                Completed::Failed(e) => {
                    self.open.store(false, Ordering::Release);
                    anyhow::bail!("window presenter: {e}");
                }
            }
        }
        Ok(())
    }
    pub fn finish<S: FrameSource>(&mut self, src: &mut S) -> Result<()> {
        if let Some(waiting) = self.mailbox.reclaim() {
            let slot = waiting.slot;
            drop(waiting);
            src.frame_discarded();
            self.free.push(slot);
        }
        self.mailbox.quit();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut failure = None;
        while !self.stopped {
            self.gpu.device.poll(wgpu::PollType::Poll)?;
            match self
                .receiver
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            {
                Ok(Completed::Retired(slot)) => {
                    src.frame_retired();
                    self.free.push(slot);
                }
                Ok(Completed::Abandoned(slot)) => self.abandoned.push(slot),
                Ok(Completed::Failed(e)) => failure = Some(e),
                Ok(Completed::Stopped) => self.stopped = true,
                Err(e) => {
                    failure = Some(format!("window shutdown: {e}"));
                    break;
                }
            }
        }
        for slot in self.abandoned.drain(..) {
            src.frame_discarded();
            self.free.push(slot);
        }
        if let Some(error) = failure {
            anyhow::bail!("{error}");
        }
        Ok(())
    }

    pub fn should_quit(&self) -> bool {
        !self.open.load(Ordering::Acquire)
    }
    pub fn take_save(&mut self) -> bool {
        self.save.swap(false, Ordering::AcqRel)
    }
    pub fn presented(&self) -> (u64, u64) {
        (
            self.presented.load(Ordering::Relaxed),
            self.skipped.load(Ordering::Relaxed),
        )
    }
}
impl Drop for WgpuWindowTarget {
    fn drop(&mut self) {
        self.mailbox.quit();
        if let Some(worker) = self.worker.take() {
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mailbox;
    use std::sync::{Arc, mpsc};
    #[test]
    fn a_fresher_window_frame_replaces_the_waiting_frame() {
        let mailbox = Mailbox::default();
        mailbox.post(1u64).unwrap();
        assert_eq!(mailbox.reclaim(), Some(1));
        mailbox.post(2u64).unwrap();
        assert_eq!(mailbox.reclaim(), Some(2));
        assert!(mailbox.reclaim().is_none());
    }
    #[test]
    fn a_recorder_can_post_while_the_presenter_is_blocked_in_wsi() {
        let mailbox = Arc::new(Mailbox::default());
        mailbox.post(1u64).unwrap();
        let (entered, active) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let worker_mailbox = mailbox.clone();
        let worker = std::thread::spawn(move || {
            assert_eq!(worker_mailbox.reclaim(), Some(1));
            entered.send(()).unwrap();
            blocked.recv().unwrap();
        });
        active.recv().unwrap();
        mailbox.post(2u64).unwrap();
        assert_eq!(
            mailbox.reclaim(),
            Some(2),
            "recording does not wait for WSI"
        );
        release.send(()).unwrap();
        worker.join().unwrap();
    }
}
