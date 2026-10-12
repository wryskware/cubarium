//! WSI runs on the presenting thread. Recording posts one replaceable frame.
#![forbid(unsafe_code)]

use super::window_host::WindowHost;
use anyhow::{Context, Result, ensure};
use cubarium_gpu::wgpu_backend::window::WindowPresenter;
use cubarium_gpu::wgpu_backend::{FrameSource, Gpu};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const RETRY_WAIT: Duration = Duration::from_millis(10);
const JOIN_WAIT: Duration = Duration::from_secs(2);

struct Posted<T> {
    slot: usize,
    version: Option<u64>,
    commands: T,
}
struct Commands {
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    size: (u32, u32),
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
    fn wait_after_attempt(&self, presented: bool) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.quit || (presented && state.frame.is_some()) {
            return false;
        }
        // An unavailable surface can leave a held frame and a newer posted frame.
        // Neither makes another immediate acquire useful; wait before retrying.
        self.moved
            .wait_timeout(state, RETRY_WAIT)
            .unwrap_or_else(|e| e.into_inner())
            .1
            .timed_out()
    }
}

enum Completed {
    Retired(usize),
    Failed(String),
    Abandoned(usize),
    Stopped,
}

// The production draw/poll/finish bookkeeping does not need a device or window.
// Keeping it here lets the headless tests exercise actual slot ownership and
// FrameSource retirement/rollback rather than a separate mailbox model.
trait FrameLifecycle {
    fn retired(&mut self);
    fn discarded(&mut self);
    fn in_flight(&self) -> usize;
    fn capacity(&self) -> usize;
    fn version(&self) -> Option<u64>;
}
impl<S: FrameSource> FrameLifecycle for S {
    fn retired(&mut self) {
        self.frame_retired();
    }
    fn discarded(&mut self) {
        self.frame_discarded();
    }
    fn in_flight(&self) -> usize {
        self.frames_in_flight()
    }
    fn capacity(&self) -> usize {
        self.frame_capacity()
    }
    fn version(&self) -> Option<u64> {
        self.content_version()
    }
}
struct WindowFrames<T> {
    mailbox: Arc<Mailbox<Posted<T>>>,
    receiver: mpsc::Receiver<Completed>,
    free: Vec<usize>,
    abandoned: Vec<usize>,
    stopped: bool,
    open: Arc<AtomicBool>,
    skipped: Arc<AtomicU64>,
}
impl<T> WindowFrames<T> {
    fn draw<S: FrameLifecycle>(
        &mut self,
        src: &mut S,
        record: impl FnOnce(&mut S) -> Result<T>,
    ) -> Result<()> {
        self.poll(src)?;
        if !self.open.load(Ordering::Acquire) {
            return Ok(());
        }
        // Reclaim before recording, so rollback always targets the newest frame.
        if let Some(waiting) = self.mailbox.reclaim() {
            if waiting.version.is_some() && waiting.version == src.version() {
                self.mailbox.post(waiting)?;
                self.skipped.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            let slot = waiting.slot;
            drop(waiting);
            src.discarded();
            self.free.push(slot);
        }
        let Some(slot) = self.free.pop() else {
            self.skipped.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        };
        if src.in_flight() >= src.capacity() {
            self.free.push(slot);
            self.skipped.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        let commands = match record(src) {
            Ok(commands) => commands,
            Err(e) => {
                self.free.push(slot);
                return Err(e);
            }
        };
        let posted = Posted {
            slot,
            commands,
            version: src.version(),
        };
        if let Err(e) = self.mailbox.post(posted) {
            src.discarded();
            self.free.push(slot);
            return Err(e);
        }
        Ok(())
    }
    fn completed<S: FrameLifecycle>(&mut self, src: &mut S, done: Completed) -> Result<()> {
        match done {
            Completed::Retired(slot) => {
                src.retired();
                self.free.push(slot);
            }
            Completed::Abandoned(slot) => self.abandoned.push(slot),
            Completed::Stopped => {
                self.stopped = true;
                self.open.store(false, Ordering::Release);
            }
            Completed::Failed(e) => {
                self.open.store(false, Ordering::Release);
                anyhow::bail!("window presenter: {e}");
            }
        }
        Ok(())
    }
    fn poll<S: FrameLifecycle>(&mut self, src: &mut S) -> Result<()> {
        while let Ok(done) = self.receiver.try_recv() {
            self.completed(src, done)?;
        }
        Ok(())
    }
    fn finish<S: FrameLifecycle>(
        &mut self,
        src: &mut S,
        mut poll_device: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        if let Some(waiting) = self.mailbox.reclaim() {
            let slot = waiting.slot;
            drop(waiting);
            src.discarded();
            self.free.push(slot);
        }
        self.mailbox.quit();
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut failure = None;
        while !self.stopped {
            if let Err(e) = poll_device() {
                failure = Some(e);
                break;
            }
            match self
                .receiver
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(done) => {
                    if let Err(e) = self.completed(src, done) {
                        failure = Some(e);
                    }
                }
                Err(e) => {
                    failure = Some(anyhow::anyhow!("window shutdown: {e}"));
                    break;
                }
            }
        }
        for slot in self.abandoned.drain(..) {
            src.discarded();
            self.free.push(slot);
        }
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(())
    }
    fn finish_and_join<S: FrameLifecycle>(
        &mut self,
        src: &mut S,
        worker: &mut Option<Worker>,
        poll_device: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        let finished = self.finish(src, poll_device);
        // Stopped accounts for frames; joining also waits for the surface,
        // WindowHost and thread's GPU reference to be dropped.
        let joined = join_worker(worker);
        finished.and(joined)
    }
}

struct Worker {
    handle: JoinHandle<()>,
    exited: mpsc::Receiver<()>,
}
struct ExitNotice(mpsc::Sender<()>);
impl Drop for ExitNotice {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}
fn join_worker(worker: &mut Option<Worker>) -> Result<()> {
    join_worker_until(worker, Instant::now() + JOIN_WAIT)
}
fn join_worker_until(worker: &mut Option<Worker>, deadline: Instant) -> Result<()> {
    let Some(worker) = worker.take() else {
        return Ok(());
    };
    if !worker.handle.is_finished() {
        let _ = worker
            .exited
            .recv_timeout(deadline.saturating_duration_since(Instant::now()));
    }
    // The resource-drop notice can arrive before the closure's epilogue. Keep
    // the handle's completion check inside the same bounded join deadline.
    while !worker.handle.is_finished() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            anyhow::bail!("window presenting thread did not exit before join deadline");
        }
        std::thread::park_timeout(remaining.min(Duration::from_millis(5)));
    }
    worker
        .handle
        .join()
        .map_err(|_| anyhow::anyhow!("window presenting thread panicked"))
}

pub struct WgpuWindowTarget {
    gpu: Arc<Gpu>,
    frames: WindowFrames<Commands>,
    worker: Option<Worker>,
    size: (u32, u32),
    save: Arc<AtomicBool>,
    presented: Arc<AtomicU64>,
}
impl WgpuWindowTarget {
    pub fn prepare(title: &str, raster_size: (u32, u32)) -> Result<(Arc<Gpu>, Self)> {
        crate::sink::hidden::window_allowed()?;
        let title = title.to_owned();
        let mailbox = Arc::new(Mailbox::<Posted<Commands>>::default());
        let open = Arc::new(AtomicBool::new(true));
        let save = Arc::new(AtomicBool::new(false));
        let presented = Arc::new(AtomicU64::new(0));
        let skipped = Arc::new(AtomicU64::new(0));
        let (ready, initial) = mpsc::sync_channel(1);
        let (completed, receiver) = mpsc::channel();
        let (exit_notice, exited) = mpsc::channel();
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
                // Declared before window/GPU resources so it drops after them,
                // including on an initialization error or thread panic.
                let _exit_notice = ExitNotice(exit_notice);
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
                let mut held: Option<Posted<Commands>> = None;
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
                        let mut presented = false;
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
                                    &mut frame.commands.encoder,
                                    &frame.commands.view,
                                    frame.commands.size,
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
                                    frame.commands.encoder,
                                    move || {
                                        let _ = on_done.send(Completed::Retired(slot));
                                    },
                                ) {
                                    let _ = completed.send(Completed::Abandoned(slot));
                                    return Err(error);
                                }
                                shown.fetch_add(1, Ordering::Relaxed);
                                presented = true;
                                Ok(())
                            })?;
                        }
                        postbox.wait_after_attempt(presented);
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
        let target = Self {
            gpu: gpu.clone(),
            frames: WindowFrames {
                mailbox,
                receiver,
                free: vec![1, 0],
                abandoned: vec![],
                stopped: false,
                open,
                skipped,
            },
            worker: Some(Worker {
                handle: worker,
                exited,
            }),
            size,
            save,
            presented,
        };
        Ok((gpu, target))
    }
    pub fn draw<S: FrameSource>(&mut self, src: &mut S, frame: S::Frame<'_>) -> Result<f64> {
        self.gpu.device.poll(wgpu::PollType::Poll)?;
        let gpu = &self.gpu;
        let size = self.size;
        self.frames.draw(src, |src| {
            ensure!(src.raster_size() == size, "window raster extent changed");
            let mut encoder = gpu.encoder("window world and present");
            src.record_frame(gpu, &mut encoder, frame, None)?;
            // World and present are in one command buffer. The worker submits
            // frames in record order, so each present reads its own world pass
            // before the next submission can overwrite this shared raster.
            Ok(Commands {
                encoder,
                view: src.raster_view().clone(),
                size,
            })
        })?;
        Ok(src.gpu_ms(&self.gpu))
    }
    pub fn finish<S: FrameSource>(&mut self, src: &mut S) -> Result<()> {
        let gpu = &self.gpu;
        self.frames.finish_and_join(src, &mut self.worker, || {
            Ok(gpu.device.poll(wgpu::PollType::Poll).map(|_| ())?)
        })
    }

    pub fn should_quit(&self) -> bool {
        !self.frames.open.load(Ordering::Acquire)
    }
    pub fn take_save(&mut self) -> bool {
        self.save.swap(false, Ordering::AcqRel)
    }
    pub fn presented(&self) -> (u64, u64) {
        (
            self.presented.load(Ordering::Relaxed),
            self.frames.skipped.load(Ordering::Relaxed),
        )
    }
}
impl Drop for WgpuWindowTarget {
    fn drop(&mut self) {
        self.frames.mailbox.quit();
        let _ = join_worker(&mut self.worker);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct Source {
        version: u64,
        capacity: usize,
        flight: VecDeque<u64>,
        recorded: Vec<u64>,
        retired: Vec<u64>,
        discarded: Vec<u64>,
    }
    impl Default for Source {
        fn default() -> Self {
            Self {
                version: 1,
                capacity: 2,
                flight: VecDeque::new(),
                recorded: vec![],
                retired: vec![],
                discarded: vec![],
            }
        }
    }
    impl FrameLifecycle for Source {
        fn retired(&mut self) {
            self.retired
                .push(self.flight.pop_front().expect("retire oldest record"));
        }
        fn discarded(&mut self) {
            self.discarded
                .push(self.flight.pop_back().expect("discard newest record"));
        }
        fn in_flight(&self) -> usize {
            self.flight.len()
        }
        fn capacity(&self) -> usize {
            self.capacity
        }
        fn version(&self) -> Option<u64> {
            Some(self.version)
        }
    }
    fn record(src: &mut Source) -> Result<u64> {
        src.flight.push_back(src.version);
        src.recorded.push(src.version);
        Ok(src.version)
    }
    fn window_frames() -> (WindowFrames<u64>, mpsc::Sender<Completed>) {
        let (completed, receiver) = mpsc::channel();
        (
            WindowFrames {
                mailbox: Arc::new(Mailbox::default()),
                receiver,
                free: vec![1, 0],
                abandoned: vec![],
                stopped: false,
                open: Arc::new(AtomicBool::new(true)),
                skipped: Arc::new(AtomicU64::new(0)),
            },
            completed,
        )
    }
    fn assert_all_slots_free(frames: &WindowFrames<u64>) {
        let mut slots = frames.free.clone();
        slots.sort_unstable();
        assert_eq!(slots, [0, 1]);
    }
    #[test]
    fn window_draw_replaces_only_the_newest_waiting_frame() {
        let (mut frames, completed) = window_frames();
        let mut src = Source::default();
        frames.draw(&mut src, record).unwrap();
        let held = frames.mailbox.reclaim().unwrap();
        src.version = 2;
        frames.draw(&mut src, record).unwrap();
        src.version = 3;
        frames.draw(&mut src, record).unwrap();
        let newest = frames.mailbox.reclaim().unwrap();
        assert_eq!((held.commands, newest.commands), (1, 3));
        assert_eq!(src.recorded, [1, 2, 3]);
        assert_eq!(src.discarded, [2]);
        assert!(frames.free.is_empty());

        completed.send(Completed::Retired(held.slot)).unwrap();
        frames.poll(&mut src).unwrap();
        assert_eq!(src.retired, [1]);
        assert_eq!(src.flight, [3]);
        completed.send(Completed::Retired(newest.slot)).unwrap();
        frames.poll(&mut src).unwrap();
        assert_eq!(src.retired, [1, 3]);
        assert!(src.flight.is_empty());
        assert_all_slots_free(&frames);
    }
    #[test]
    fn window_draw_preserves_matching_waiting_frames_and_backpressure() {
        let (mut frames, completed) = window_frames();
        let mut src = Source::default();
        frames.draw(&mut src, record).unwrap();
        frames
            .draw(&mut src, |_| panic!("matching version must stay posted"))
            .unwrap();
        let first = frames.mailbox.reclaim().unwrap();
        assert_eq!(first.commands, 1);
        assert_eq!(src.recorded, [1]);
        assert!(src.discarded.is_empty());
        src.version = 2;
        frames.draw(&mut src, record).unwrap();
        let second = frames.mailbox.reclaim().unwrap();
        src.version = 3;
        frames
            .draw(&mut src, |_| panic!("no target slot available"))
            .unwrap();
        completed.send(Completed::Retired(first.slot)).unwrap();
        src.capacity = 1;
        frames
            .draw(&mut src, |_| panic!("source ring is full"))
            .unwrap();
        assert_eq!(frames.free, [first.slot]);
        src.capacity = 2;
        frames.draw(&mut src, record).unwrap();
        let third = frames.mailbox.reclaim().unwrap();
        assert_eq!(third.slot, first.slot);
        assert_ne!(third.slot, second.slot);
        assert_eq!(src.recorded, [1, 2, 3]);
        assert_eq!(frames.skipped.load(Ordering::Relaxed), 3);
    }
    #[test]
    fn window_record_and_post_errors_return_slots_without_extra_discard() {
        let (mut frames, _completed) = window_frames();
        let mut src = Source::default();
        assert!(
            frames
                .draw(&mut src, |_| anyhow::bail!("record failed"))
                .is_err()
        );
        assert_all_slots_free(&frames);
        assert!(src.discarded.is_empty());
        let mailbox = frames.mailbox.clone();
        assert!(
            frames
                .draw(&mut src, |src| {
                    let commands = record(src)?;
                    mailbox.quit();
                    Ok(commands)
                })
                .is_err()
        );
        assert_eq!(src.recorded, [1]);
        assert_eq!(src.discarded, [1]);
        assert!(src.flight.is_empty());
        assert_all_slots_free(&frames);
    }
    #[test]
    fn window_finish_discards_waiting_before_abandoned_frames() {
        let (mut frames, completed) = window_frames();
        let mut src = Source::default();
        frames.draw(&mut src, record).unwrap();
        let submitted = frames.mailbox.reclaim().unwrap();
        src.version = 2;
        frames.draw(&mut src, record).unwrap();
        let abandoned = frames.mailbox.reclaim().unwrap();
        completed.send(Completed::Retired(submitted.slot)).unwrap();
        completed
            .send(Completed::Abandoned(abandoned.slot))
            .unwrap();
        src.version = 3;
        frames.draw(&mut src, record).unwrap();
        assert_eq!(src.retired, [1]);
        assert!(src.discarded.is_empty(), "abandoned is older than waiting");
        completed.send(Completed::Stopped).unwrap();
        frames
            .finish_and_join(&mut src, &mut None, || Ok(()))
            .unwrap();
        assert_eq!(src.discarded, [3, 2]);
        assert!(src.flight.is_empty());
        assert_all_slots_free(&frames);
        assert!(frames.stopped);
        frames
            .draw(&mut src, |_| panic!("closed window must not record"))
            .unwrap();
    }
    #[test]
    fn window_finish_drains_retirement_and_reports_presenter_failure() {
        let (mut frames, completed) = window_frames();
        let mut src = Source::default();
        frames.draw(&mut src, record).unwrap();
        let submitted = frames.mailbox.reclaim().unwrap();
        src.version = 2;
        frames.draw(&mut src, record).unwrap();
        completed.send(Completed::Retired(submitted.slot)).unwrap();
        completed
            .send(Completed::Failed("injected failure".into()))
            .unwrap();
        completed.send(Completed::Stopped).unwrap();
        let error = frames
            .finish_and_join(&mut src, &mut None, || Ok(()))
            .unwrap_err();
        assert!(error.to_string().contains("injected failure"));
        assert_eq!(src.retired, [1]);
        assert_eq!(src.discarded, [2]);
        assert!(src.flight.is_empty());
        assert_all_slots_free(&frames);
    }
    #[test]
    fn window_surface_retry_waits_even_with_another_frame_posted() {
        let mailbox = Mailbox::default();
        mailbox.post(1u64).unwrap();
        assert!(
            mailbox.wait_after_attempt(false),
            "unavailable surface must wait"
        );
        assert!(
            !mailbox.wait_after_attempt(true),
            "next successful frame can run at once"
        );
        assert_eq!(mailbox.reclaim(), Some(1));
        mailbox.quit();
        assert!(!mailbox.wait_after_attempt(false), "shutdown must not wait");
    }
    #[test]
    fn window_finish_joins_after_stopped_and_thread_resources_drop() {
        struct Resource {
            dropping: mpsc::Sender<()>,
            release: mpsc::Receiver<()>,
            dropped: Arc<AtomicBool>,
        }
        impl Drop for Resource {
            fn drop(&mut self) {
                self.dropping.send(()).unwrap();
                self.release.recv_timeout(Duration::from_secs(1)).unwrap();
                self.dropped.store(true, Ordering::Release);
            }
        }
        let (mut frames, completed) = window_frames();
        let (notice, exited) = mpsc::channel();
        let (dropping, started_drop) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let dropped = Arc::new(AtomicBool::new(false));
        let resource = Resource {
            dropping,
            release: released,
            dropped: dropped.clone(),
        };
        let handle = std::thread::spawn(move || {
            let _notice = ExitNotice(notice);
            let _resource = resource;
            completed.send(Completed::Stopped).unwrap();
        });
        let (finished, result) = mpsc::channel();
        let finisher = std::thread::spawn(move || {
            let mut worker = Some(Worker { handle, exited });
            let outcome = frames.finish_and_join(&mut Source::default(), &mut worker, || Ok(()));
            finished.send((outcome, worker.is_none())).unwrap();
        });
        started_drop.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(
            matches!(
                result.recv_timeout(Duration::from_millis(20)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ),
            "Stopped must not let finish return while thread resources remain alive"
        );
        release.send(()).unwrap();
        let (outcome, joined) = result.recv_timeout(Duration::from_secs(1)).unwrap();
        outcome.unwrap();
        assert!(joined);
        assert!(dropped.load(Ordering::Acquire));
        finisher.join().unwrap();
    }
    #[test]
    fn window_join_deadline_checks_thread_completion_after_exit_notice() {
        let (notice, exited) = mpsc::channel();
        let (entered, ready) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let (finished, ended) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            drop(ExitNotice(notice));
            entered.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(1)).unwrap();
            finished.send(()).unwrap();
        });
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        let mut worker = Some(Worker { handle, exited });
        assert!(
            join_worker_until(&mut worker, Instant::now() + Duration::from_millis(20)).is_err(),
            "an early exit notice must not bypass the bounded completion check"
        );
        assert!(worker.is_none(), "timed-out worker must be detached");
        assert!(
            ended.try_recv().is_err(),
            "worker remains inside its epilogue"
        );
        release.send(()).unwrap();
        ended.recv_timeout(Duration::from_secs(1)).unwrap();
    }
}
