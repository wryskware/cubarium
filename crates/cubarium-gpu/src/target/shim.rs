//! The board, through `cube-screen-shim`'s dma-buf socket (GS-2).
//!
//! This is the target the panel gets in production. The renderer exports the same
//! `VK_IMAGE_TILING_LINEAR` scanout images as the direct path ([`super::dmabuf`]) and
//! hands their fds to the daemon over `SOCK_SEQPACKET` at
//! `/run/cube-screen-shim/frames.sock`; the daemon keeps DRM master and does the page
//! flips. Nothing here takes DRM master, so the service stays up and two workers can be
//! on the device at once.
//!
//! # The wire, exactly
//!
//! Little-endian, one datagram per message (`SOCK_SEQPACKET`, so a short read is a
//! protocol error rather than a resync problem).
//!
//! ```text
//! request  32 B  tag:u8 slot:u8 rsv:u16 seq:u32 width:u32 height:u32
//!                fourcc:u32 pitch:u32 offset:u32 rsv:u32
//!   tag 1 Attach  — carries exactly one fd in SCM_RIGHTS
//!   tag 2 Present — no fd
//!   tag 3 Detach  — no fd
//!
//! reply    12 B + optional text
//!                tag:u8 slot:u8 released:u8 code:u8 seq:u32 len:u32 message[len]
//!   tag 1 Attached  2 Presented  3 Detached  4 Error
//! ```
//!
//! **`released` is the only signal a buffer is free.** It is a bitmask of the slots the
//! daemon stopped using since the last `Presented`, so a client that re-renders into a
//! slot before seeing its bit has torn the frame the panel is still reading. This client
//! therefore keeps a free mask, clears a slot's bit when it presents it, and blocks on
//! the reply until the slot it wants next comes back.
//!
//! The daemon takes at most four slots, one client, `XR24`/`AR24` only, and the image
//! must be exactly 1080x1920 — **the client rotates**, which is what the present pass's
//! quarter turn already does.

use std::io::{IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::unix::io::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use ash::vk;

use super::dmabuf::{self, LinearImage};
use super::presenter::{
    Frame, FromPresenter, Mailbox, Next, Panel, PresentSample, PresentStats, next_frame,
    present_loop,
};
use crate::present::FrameSource;
use crate::render::{PresentTransform, TargetImage};
use crate::vk::Gpu;

/// The daemon's socket.
pub const SOCKET: &str = "/run/cube-screen-shim/frames.sock";
/// How long the presenter will sit on the mailbox before going to look at the socket.
///
/// A freed slot arrives in a reply and nowhere else, so this is the worst case between a
/// flip completing and the recorder being told it may draw again — a fraction of the
/// 16.7 ms the panel gives a frame, for one `poll` every two milliseconds.
const SOCKET_POLL: Duration = Duration::from_millis(2);
/// The panel, which the daemon fixes: the client rotates into it.
pub const PANEL: (u32, u32) = (1080, 1920);
/// `XR24`, the only format the spike's modifier-free `AddFB2` accepted.
const FOURCC_XR24: u32 = u32::from_le_bytes(*b"XR24");
/// Slots to attach — **every one the daemon takes** (`MAX_SLOTS` is 4 in
/// `led-cube-shim`'s `handoff/wire.rs`, and `Slots::attach` refuses the fifth).
///
/// Four is the number the pipeline needs: one on the panel, one queued for the next
/// vblank, one the GPU is drawing and one the recorder can record into. With three the
/// recorder holds exactly one at a time once the daemon is a frame ahead, so every frame
/// waits for a flip to give a slot back before it can even be recorded — which is what
/// the board showed: every present starved of a slot and the panel at a third of its
/// rate.
const SLOTS: usize = 4;

const TAG_ATTACH: u8 = 1;
const TAG_PRESENT: u8 = 2;
const TAG_DETACH: u8 = 3;
const REPLY_ATTACHED: u8 = 1;
const REPLY_PRESENTED: u8 = 2;
const REPLY_DETACHED: u8 = 3;
const REPLY_ERROR: u8 = 4;
/// `Error { Busy }`: another client is attached.
const ERROR_BUSY: u8 = 6;
/// How long to wait out a busy daemon, and how often.
const BUSY_RETRIES: u32 = 40;
const BUSY_WAIT: Duration = Duration::from_millis(100);
/// The same four seconds, for waiting out a daemon that has just handed the panel over:
/// it closed this client's socket, and the slots went with the device it reopened.
const HANDOFF_RETRIES: u32 = 40;
const HANDOFF_WAIT: Duration = Duration::from_millis(100);

struct Slot {
    /// The slot number the **daemon** assigned in its `Attached` reply. It is not the
    /// index in this vector: `Attach` carries slot 0 and the daemon picks, and the
    /// `released` bitmask is in the daemon's numbering.
    id: u8,
    /// Our end of the dma-buf, **kept** rather than dropped after the attach: when the
    /// daemon hands the panel over it forgets every slot, and re-attaching the same
    /// memory is what puts this client back on the screen without re-exporting it.
    fd: OwnedFd,
    image: LinearImage,
    target: TargetImage,
    command_buffer: vk::CommandBuffer,
    fence: vk::Fence,
}

/// A client of the shim's frame socket.
pub struct ShimScanout {
    socket: OwnedFd,
    slots: Vec<Slot>,
    /// Bitmask of slots the daemon is not using.
    free: u8,
    transform: PresentTransform,
    view_format: vk::Format,
    seq: u32,
    next: usize,
}

impl ShimScanout {
    /// Connect, export `SLOTS` scanout images and attach them all.
    pub fn open<S: FrameSource>(gpu: &Gpu, src: &mut S, quarter_turns: u32) -> Result<ShimScanout> {
        if !gpu.has_dma_buf {
            bail!("this device has no VK_EXT_external_memory_dma_buf; the shim cannot be fed");
        }
        dmabuf::linear_export_supported(gpu)?;
        let shader_encode = !dmabuf::srgb_view_supported(gpu);
        let raster = src.raster_size();
        let transform = PresentTransform::fit(raster, PANEL, quarter_turns, shader_encode)
            .ok_or_else(|| {
                anyhow!(
                    "a {}x{} raster does not fit {}x{} at {quarter_turns} quarter turn(s)",
                    raster.0,
                    raster.1,
                    PANEL.0,
                    PANEL.1
                )
            })?;

        let view_format = if shader_encode {
            dmabuf::FORMAT
        } else {
            vk::Format::B8G8R8A8_SRGB
        };
        let pass = src.present_pass(gpu, view_format, vk::ImageLayout::GENERAL)?;
        let d = &gpu.device;
        let command_buffers = unsafe {
            d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(src.command_pool())
                    .command_buffer_count(SLOTS as u32),
            )
        }?;
        // **Exactly one socket, and it is the retrying one.** The daemon decides
        // one-client-at-a-time at `accept`, not at `Attach` (`handoff/server.rs`:
        // `if client.is_some()` on the newly accepted connection), so any second
        // socket this process holds open -- including a placeholder created only to
        // fill in a struct field -- *is* the attached client and makes the real
        // connection refuse itself. That cost half an hour of blaming a neighbour.
        let mut first = dmabuf::export_linear(gpu, PANEL.0, PANEL.1, shader_encode)?;
        let first_fd = first
            .fd
            .take()
            .expect("a freshly exported image has its fd");
        let (socket, attached) = connect_when_free(&first, first_fd.as_fd())?;
        let mut client = ShimScanout {
            socket,
            slots: Vec::with_capacity(SLOTS),
            free: attached.released | (1 << attached.slot),
            transform,
            view_format,
            seq: 0,
            next: 0,
        };
        let mut pending = vec![(attached.slot, first, first_fd)];
        for _ in 1..SLOTS {
            let mut image = dmabuf::export_linear(gpu, PANEL.0, PANEL.1, shader_encode)?;
            let fd = image
                .fd
                .take()
                .expect("a freshly exported image has its fd");
            let id = client.attach(&image, fd.as_fd())?;
            client.free |= 1 << id;
            pending.push((id, image, fd));
        }
        for (i, (id, image, fd)) in pending.into_iter().enumerate() {
            let target = TargetImage {
                image: image.image,
                view: image.view,
                framebuffer: crate::render::framebuffer(d, pass, image.view, PANEL.0, PANEL.1)?,
            };
            client.slots.push(Slot {
                id,
                fd,
                image,
                target,
                command_buffer: command_buffers[i],
                fence: unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?,
            });
        }
        println!(
            "shim socket: {SLOTS} slots attached at {}x{} XR24 pitch {}, sRGB encode by {}",
            PANEL.0,
            PANEL.1,
            client.slots[0].image.pitch,
            if shader_encode {
                "the present shader"
            } else {
                "the _SRGB attachment"
            }
        );
        Ok(client)
    }

    pub fn transform(&self) -> PresentTransform {
        self.transform
    }

    /// Render one frame into a free slot and present it.
    ///
    /// Returns `(GPU ms, submit..fence ms, pacing ms)`. The third is what this frame
    /// spent waiting for the daemon to give a slot back — the panel's own pacing, which
    /// with three slots shows up only once the renderer is a frame ahead of it.
    pub fn draw<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        src: &mut S,
        frame: S::Frame<'_>,
    ) -> Result<(f64, f64, f64)> {
        let waited = Instant::now();
        let index = self.take_free_slot()?;
        let d = &gpu.device;
        let start = Instant::now();
        {
            let slot = &self.slots[index];
            unsafe {
                d.reset_command_buffer(slot.command_buffer, vk::CommandBufferResetFlags::empty())
            }?;
            src.record_frame(
                gpu,
                slot.command_buffer,
                frame,
                Some((
                    &slot.target,
                    PANEL,
                    self.view_format,
                    vk::ImageLayout::GENERAL,
                    self.transform,
                )),
            )?;
            let one = [slot.command_buffer];
            unsafe { d.reset_fences(&[slot.fence]) }?;
            gpu.submit(
                &[vk::SubmitInfo::default().command_buffers(&one)],
                slot.fence,
            )?;
            unsafe { d.wait_for_fences(&[slot.fence], true, u64::MAX) }?;
        }
        let submitted = Instant::now();
        src.frame_retired();
        self.present(self.slots[index].id)?;
        Ok((
            src.gpu_ms(gpu),
            (submitted - start).as_secs_f64() * 1e3,
            (start - waited).as_secs_f64() * 1e3,
        ))
    }

    /// The last presented slot's contents, read back through the GPU as RGBA8: proof
    /// that what the daemon is scanning out is the frame that was drawn.
    pub fn read_presented<S: FrameSource>(
        &self,
        gpu: &Gpu,
        src: &S,
    ) -> Result<(u32, u32, Vec<u8>)> {
        let index = (self.next + SLOTS - 1) % SLOTS;
        let rgba = dmabuf::read_back(
            gpu,
            src.command_pool(),
            &self.slots[index].image,
            PANEL.0,
            PANEL.1,
            vk::ImageLayout::GENERAL,
        )?;
        Ok((PANEL.0, PANEL.1, rgba))
    }

    /// Every slot's command buffer and attachment, for a recorder that is not this
    /// thread. They are fixed when the slot is attached — a re-attach changes the
    /// daemon's slot *id*, never the image or the buffer — so the recorder can keep them
    /// while the client itself lives on the presenter thread.
    pub fn record_slots(&self) -> Vec<SlotRecord> {
        self.slots
            .iter()
            .map(|s| SlotRecord {
                command_buffer: s.command_buffer,
                image: s.target,
            })
            .collect()
    }

    /// The attachment format every slot's view has, which the recorder needs to ask for
    /// the right present pass.
    pub fn view_format(&self) -> vk::Format {
        self.view_format
    }

    /// Submit slot `i`'s recording.
    pub fn submit_slot(&mut self, gpu: &Gpu, i: usize) -> Result<()> {
        let slot = &self.slots[i];
        let one = [slot.command_buffer];
        unsafe { gpu.device.reset_fences(&[slot.fence]) }?;
        gpu.submit(
            &[vk::SubmitInfo::default().command_buffers(&one)],
            slot.fence,
        )
    }

    /// Wait for slot `i`'s fence.
    ///
    /// **The wait stays.** The wire has no fence field, so a `Present` is a promise that
    /// the image is finished; the only thing that moved is which thread pays for it. It
    /// is separate from the submit so that a report can say which of the two the panel's
    /// rate is spent in — a driver that renders on the submitting thread and a GPU that
    /// is simply slow look the same from the outside.
    pub fn wait_slot(&mut self, gpu: &Gpu, i: usize) -> Result<()> {
        let fence = [self.slots[i].fence];
        unsafe { gpu.device.wait_for_fences(&fence, true, u64::MAX) }?;
        Ok(())
    }

    /// Show slot `i`.
    ///
    /// Presenting faster than the panel refreshes is safe and is not waste: the daemon's
    /// `Present` is latest-wins, and the frame it overtakes is freed at the next flip
    /// along with the one leaving the screen (`handoff/slots.rs`).
    pub fn present_slot(&mut self, i: usize) -> Result<()> {
        self.present(self.slots[i].id)
    }

    /// The slots the daemon has released and this client has not handed on yet.
    ///
    /// With `block` it reads replies until one comes back — `released` is the only signal
    /// a buffer is free — and fails as the synchronous path does if the daemon releases
    /// nothing at all. Without it, it reports what is already known and reads nothing, so
    /// a presenter that still has slots to lend never waits on a vsync.
    pub fn take_released(&mut self, block: bool) -> Result<Vec<usize>> {
        // **Always take what has already arrived.** A slot the panel has finished with
        // does not exist until the reply carrying it is read, so every one of these that
        // is left in the socket is a slot the recorder could have been drawing into. This
        // costs one `poll` and no wait.
        while ready_to_read(&self.socket)? {
            self.read_reply(REPLY_PRESENTED)?;
        }
        if block && self.free == 0 {
            // Nothing to draw into at all: now a reply is worth waiting for, because it
            // is the only thing that can free one.
            for _ in 0..(SLOTS + 2) {
                self.read_reply(REPLY_PRESENTED)?;
                if self.free != 0 {
                    break;
                }
            }
            if self.free == 0 {
                bail!("the daemon released no slot after {SLOTS} replies");
            }
        }
        let free: Vec<usize> = (0..self.slots.len())
            .filter(|i| self.free & (1 << self.slots[*i].id) != 0)
            .collect();
        for i in &free {
            self.free &= !(1 << self.slots[*i].id);
        }
        Ok(free)
    }

    /// The next slot the daemon has given back, blocking on replies until one arrives.
    fn take_free_slot(&mut self) -> Result<usize> {
        for _ in 0..(SLOTS + 2) {
            let candidate = self.next;
            let id = self.slots[candidate].id;
            if self.free & (1 << id) != 0 {
                self.next = (self.next + 1) % SLOTS;
                self.free &= !(1 << id);
                return Ok(candidate);
            }
            // Nothing free: the only thing that frees a slot is a reply.
            self.read_reply(REPLY_PRESENTED)?;
        }
        bail!("the daemon released no slot after {SLOTS} replies")
    }

    /// Hand one exported dma-buf to the daemon and return the slot it chose.
    ///
    /// `slot` is **reserved on the way in** — the daemon picks and says so in its
    /// `Attached` reply — so the request carries 0 and the answer is authoritative.
    fn attach(&mut self, image: &LinearImage, fd: BorrowedFd<'_>) -> Result<u8> {
        let reply = attach_on(&self.socket, image.pitch, image.offset, fd)?
            .ok_or_else(|| anyhow!("the daemon closed the connection during the attach"))?;
        self.free |= reply.released;
        reply.expect(REPLY_ATTACHED)?;
        Ok(reply.slot)
    }

    /// The daemon handed the panel over: re-open the socket and put every slot back.
    ///
    /// The old socket is dropped first — the daemon refuses a second `accept` while this
    /// process holds one, so keeping it would make the reconnection refuse itself. The
    /// images and their memory are untouched; only the daemon's framebuffers over them
    /// are new, so nothing is re-exported and nothing is re-rendered. Past the retries it
    /// fails as it always did, and the unit restarts the client.
    fn reattach(&mut self) -> Result<()> {
        eprintln!("shim socket: the daemon closed the connection; re-attaching");
        // Nothing may be in flight on a socket that is gone.
        let images: Vec<(u32, u32, BorrowedFd<'_>)> = self
            .slots
            .iter()
            .map(|s| (s.image.pitch, s.image.offset, s.fd.as_fd()))
            .collect();
        let (socket, ids, free) = {
            let dead = std::mem::replace(&mut self.socket, placeholder_socket()?);
            drop(dead);
            reattach_all(SOCKET, &images, HANDOFF_RETRIES, HANDOFF_WAIT, |n, what| {
                eprintln!("shim socket: re-attach attempt {} — {what}", n + 1);
            })?
        };
        self.socket = socket;
        for (slot, id) in self.slots.iter_mut().zip(ids) {
            slot.id = id;
        }
        self.free = free;
        self.next = 0;
        self.seq = 0;
        Ok(())
    }

    /// `Present` describes nothing: every geometry field is reserved and must be zero,
    /// because the daemon already has the slot's description from `Attach`.
    ///
    /// **It does not wait for the answer.** The daemon replies when the flip it queued has
    /// completed, so waiting here would put a whole vsync period on the critical path and
    /// leave the host blocked while the GPU and the panel both had nothing to do. The
    /// replies are drained in [`ShimScanout::take_free_slot`] instead, which needs one
    /// anyway — `released` is the only signal a buffer is free — so with three slots the
    /// wait happens only when the renderer has genuinely got a frame ahead of the panel.
    fn present(&mut self, slot: u8) -> Result<()> {
        self.seq = self.seq.wrapping_add(1);
        send(
            &self.socket,
            &request(TAG_PRESENT, slot, self.seq, 0, 0, 0, 0, 0),
            None,
        )
        .context("sendmsg(Present)")
    }

    /// Read one reply and fold its `released` mask into the free set.
    ///
    /// A closed connection is the panel changing hands, not a failure: the client
    /// re-attaches and every slot is free again, which is what the caller was waiting to
    /// hear.
    fn read_reply(&mut self, expect: u8) -> Result<()> {
        match recv(&self.socket)? {
            Some(reply) => {
                // `released` is the only signal a buffer is free.
                self.free |= reply.released;
                reply.expect(expect)
            }
            None => self.reattach(),
        }
    }

    pub fn destroy(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
        }
        for i in 0..self.slots.len() {
            let request = request(TAG_DETACH, self.slots[i].id, 0, 0, 0, 0, 0, 0);
            // On the way out a closed socket is simply the end of the conversation:
            // the reply is read if there is one, and nothing is re-attached.
            if send(&self.socket, &request, None).is_ok()
                && let Ok(Some(reply)) = recv(&self.socket)
            {
                let _ = reply.expect(REPLY_DETACHED);
            }
        }
        unsafe {
            for slot in &self.slots {
                d.destroy_fence(slot.fence, None);
                d.destroy_framebuffer(slot.target.framebuffer, None);
                d.destroy_image_view(slot.image.view, None);
                d.destroy_image(slot.image.image, None);
                d.free_memory(slot.image.memory, None);
            }
        }
        self.slots.clear();
    }
}

/// What a recorder needs to draw into one slot: the command buffer it records into and
/// the attachment the present pass draws onto.
///
/// Both are fixed when the slot is attached and neither is owned here — the client on the
/// presenter thread owns them and destroys them — so this is a handle a second thread may
/// hold, not a second owner.
#[derive(Clone, Copy, Debug)]
pub struct SlotRecord {
    pub command_buffer: vk::CommandBuffer,
    pub image: TargetImage,
}

/// The shim client as the presenter thread uses it.
struct ShimPanel {
    gpu: Arc<Gpu>,
    shim: ShimScanout,
}

impl Panel for ShimPanel {
    fn submit(&mut self, slot: usize) -> Result<()> {
        self.shim.submit_slot(&self.gpu, slot)
    }
    fn wait(&mut self, slot: usize) -> Result<()> {
        self.shim.wait_slot(&self.gpu, slot)
    }
    fn present(&mut self, slot: usize) -> Result<()> {
        self.shim.present_slot(slot)
    }
    fn released(&mut self, block: bool) -> Result<Vec<usize>> {
        self.shim.take_released(block)
    }
}

/// The panel, presented from its own thread.
///
/// The recorder keeps this: it records a frame into a slot the presenter lent it and
/// posts it, and that is all it does. The queue submit, the fence wait, the socket, the
/// free mask and the re-attach are on the other side of the channel — which is the whole
/// point, since together they were 8 of the 15 ms that held the board to 20 fps.
///
/// **It never blocks.** No free slot, or the renderer already holding as many frames as
/// it has per-frame resources for, is a dropped frame and a counter, never a wait.
pub struct ShimPresenter {
    mail: Arc<Mailbox>,
    back: Receiver<FromPresenter>,
    stats: Arc<PresentStats>,
    thread: Option<std::thread::JoinHandle<ShimScanout>>,
    /// The client, once the thread has given it back, so that `destroy` can free it.
    client: Option<ShimScanout>,
    slots: Vec<SlotRecord>,
    view_format: vk::Format,
    transform: PresentTransform,
    /// Slots the presenter has lent and this side has not used yet, newest first.
    free: std::collections::VecDeque<usize>,
    /// Frames that had nowhere to go: no free slot, or the renderer already holding as
    /// many frames as it has per-frame resources for.
    skipped: u64,
    /// Frames not recorded because the one already waiting showed the same world.
    held: u64,
    /// The renderer's content version when the waiting frame was recorded.
    posted_version: Option<u64>,
    /// Why the presenter stopped, once it has.
    stopped: Option<String>,
}

impl ShimPresenter {
    /// Attach to the daemon and start presenting on another thread.
    pub fn open<S: FrameSource>(
        gpu: Arc<Gpu>,
        src: &mut S,
        quarter_turns: u32,
    ) -> Result<ShimPresenter> {
        let shim = ShimScanout::open(&gpu, src, quarter_turns)?;
        let slots = shim.record_slots();
        let view_format = shim.view_format();
        let transform = shim.transform();
        // Nothing has been presented, so the daemon is using no slot and the recorder
        // starts holding all of them. The presenter is told so, and therefore does not
        // offer them again — its own free list is what a re-attach refills.
        let lent: Vec<usize> = (0..slots.len()).collect();
        let free = lent.iter().copied().collect();
        let mail = Arc::new(Mailbox::new());
        let stats = Arc::new(PresentStats::default());
        let (tx, back) = channel();
        let (m, s) = (mail.clone(), stats.clone());
        let thread = std::thread::Builder::new()
            .name("cubarium-present".to_string())
            .spawn(move || {
                let mut panel = ShimPanel { gpu, shim };
                if let Err(e) = present_loop(&mut panel, &m, &tx, &s, lent, SOCKET_POLL) {
                    let _ = tx.send(FromPresenter::Failed(format!("{e:#}")));
                }
                panel.shim
            })
            .context("starting the presenting thread")?;
        Ok(ShimPresenter {
            mail,
            back,
            stats,
            thread: Some(thread),
            client: None,
            slots,
            view_format,
            transform,
            free,
            skipped: 0,
            held: 0,
            posted_version: None,
            stopped: None,
        })
    }

    pub fn transform(&self) -> PresentTransform {
        self.transform
    }

    /// Frames the daemon has been shown.
    pub fn presented(&self) -> u64 {
        self.stats.presented()
    }

    /// Everything the presenter has done so far, with this side's dropped-frame count
    /// folded in. The report prints the difference between two of these, so each one is
    /// about its own interval.
    pub fn sample(&self) -> PresentSample {
        let mut sample = self.stats.snapshot();
        sample.skipped = self.skipped;
        sample.held = self.held;
        sample
    }

    /// Frames the recorder had nowhere to put.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Record one frame and post it. Returns the GPU milliseconds the last **finished**
    /// frame's timestamps saw; this one has not run yet.
    pub fn draw<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        src: &mut S,
        frame: S::Frame<'_>,
    ) -> Result<f64> {
        self.drain(src)?;
        // What to do is decided before any of it is done, so that the rules can be
        // tested rather than read out of a board log (`presenter::next_frame`).
        let version = src.content_version();
        match next_frame(
            self.mail.pending(),
            version,
            self.free.front().copied(),
            src.frames_in_flight(),
            src.frame_capacity(),
        ) {
            // The frame already waiting shows this same world: recording it again would
            // build the same commands out of the same texture, and the thread that would
            // do it has a simulation to run.
            Next::Keep => {
                self.held += 1;
                return Ok(src.gpu_ms(gpu));
            }
            Next::Drop => {
                self.skipped += 1;
                return Ok(src.gpu_ms(gpu));
            }
            // **Newest wins.** The waiting frame is stale: its slot comes back, the
            // upload it carried is owed again, and the fresher world takes its place.
            // Reclaiming *before* recording is what keeps the frame given up the newest
            // one recorded, which is the only one the renderer's ring can give back.
            Next::Replace { .. } => {
                if let Some(frame) = self.mail.reclaim() {
                    src.frame_discarded();
                    self.free.push_front(frame.slot);
                }
            }
            Next::Record { .. } => {}
        }
        // The presenter may have taken the waiting frame while that was decided, which
        // only means there is one fewer slot in hand.
        let Some(index) = self.free.pop_front() else {
            self.skipped += 1;
            return Ok(src.gpu_ms(gpu));
        };
        let slot = self.slots[index];
        unsafe {
            gpu.device
                .reset_command_buffer(slot.command_buffer, vk::CommandBufferResetFlags::empty())
        }?;
        src.record_frame(
            gpu,
            slot.command_buffer,
            frame,
            Some((
                &slot.image,
                PANEL,
                self.view_format,
                vk::ImageLayout::GENERAL,
                self.transform,
            )),
        )?;
        // What the frame did is the presenter's to report: a frame that redrew the world
        // and one that only put an already-drawn raster on a new slot cost very different
        // amounts, and which of the two the panel's rate is made of is the whole question.
        let frame = Frame {
            slot: index,
            redrew: src.redrew_last(),
            version,
        };
        self.posted_version = version;
        if let Some(displaced) = self.mail.post(frame) {
            debug_assert!(false, "the recorder reclaims before it records");
            self.free.push_back(displaced.slot);
        }
        Ok(src.gpu_ms(gpu))
    }

    /// Take everything the presenter has said since the last frame: which frames have
    /// retired, which slots are free, and whether it has stopped.
    fn drain<S: FrameSource>(&mut self, src: &mut S) -> Result<()> {
        loop {
            match self.back.try_recv() {
                Ok(FromPresenter::Retired) => src.frame_retired(),
                Ok(FromPresenter::Free(slot)) => self.free.push_back(slot),
                Ok(FromPresenter::Failed(why)) => {
                    self.stopped = Some(why.clone());
                    bail!("the presenting thread stopped: {why}");
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    let why = self
                        .stopped
                        .clone()
                        .unwrap_or_else(|| "it ended without saying why".to_string());
                    bail!("the presenting thread is gone: {why}");
                }
            }
        }
    }

    /// Stop the thread, take the client back and free everything.
    pub fn destroy(&mut self, gpu: &Gpu) {
        self.mail.quit();
        if let Some(thread) = self.thread.take() {
            match thread.join() {
                Ok(shim) => self.client = Some(shim),
                Err(_) => eprintln!("shim socket: the presenting thread panicked"),
            }
        }
        if let Some(mut shim) = self.client.take() {
            shim.destroy(gpu);
        }
    }
}

/// One decoded reply.
struct Reply {
    tag: u8,
    slot: u8,
    released: u8,
    code: u8,
    message: String,
}

impl Reply {
    /// `Ok` when this is the reply that was asked for; the daemon's own text otherwise.
    fn expect(&self, tag: u8) -> Result<()> {
        if self.tag == REPLY_ERROR {
            bail!(
                "the daemon refused the request with code {}: {}",
                self.code,
                self.message
            );
        }
        if self.tag != tag {
            bail!("expected reply tag {tag}, got {}", self.tag);
        }
        Ok(())
    }

    /// Whether this is `Error { Busy }`: another client is still attached, which is
    /// worth waiting out rather than failing on.
    fn is_busy(&self) -> bool {
        self.tag == REPLY_ERROR && self.code == ERROR_BUSY
    }
}

/// Whether a reply is already waiting, so that it can be read without blocking.
///
/// A closed connection polls as readable and `recv` reports it as the handoff it is, so
/// this does not hide one.
fn ready_to_read(socket: &OwnedFd) -> Result<bool> {
    use rustix::event::{PollFd, PollFlags, poll};
    let now = rustix::event::Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let mut fds = [PollFd::new(socket, PollFlags::IN)];
    loop {
        match poll(&mut fds, Some(&now)) {
            Ok(0) => return Ok(false),
            Ok(_) => return Ok(true),
            Err(rustix::io::Errno::INTR) => continue,
            Err(e) => return Err(e.into()),
        }
    }
}

/// An unconnected socket, held in the client's field only while the dead one is dropped
/// and the new one opened. The daemon's one-client-at-a-time rule is about *connections*,
/// and this is not connected to anything.
fn placeholder_socket() -> Result<OwnedFd> {
    rustix::net::socket(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::SEQPACKET,
        None,
    )
    .context("socket(AF_UNIX, SOCK_SEQPACKET)")
}

/// A connected, unattached client socket.
fn connect() -> Result<OwnedFd> {
    connect_to(SOCKET)
}

/// The same, on a named socket: the tests bring their own daemon.
fn connect_to(path: &str) -> Result<OwnedFd> {
    let socket = rustix::net::socket(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::SEQPACKET,
        None,
    )
    .context("socket(AF_UNIX, SOCK_SEQPACKET)")?;
    rustix::net::connect(&socket, &rustix::net::SocketAddrUnix::new(path)?)
        .with_context(|| format!("connect {path} (is cube-screen-shim running?)"))?;
    Ok(socket)
}

/// Connect and attach the first image, waiting out a daemon that still has another
/// client registered. Returns the socket the attach succeeded on and its `Attached`
/// reply, so the caller does **not** attach the same image a second time.
///
/// The caller must hold **no other** connection to the daemon while this runs: it
/// refuses a second `accept` outright, whether or not that connection ever speaks.
fn connect_when_free(image: &LinearImage, fd: BorrowedFd<'_>) -> Result<(OwnedFd, Reply)> {
    let mut last = String::new();
    for attempt in 0..BUSY_RETRIES {
        let socket = connect()?;
        match attach_on(&socket, image.pitch, image.offset, fd) {
            Ok(Some(reply)) if !reply.is_busy() => {
                reply.expect(REPLY_ATTACHED)?;
                return Ok((socket, reply));
            }
            Ok(Some(reply)) => last = format!("the daemon is busy: {}", reply.message),
            Ok(None) => last = "the daemon closed the connection".to_string(),
            // The close that follows `Error { Busy }` can break the write itself.
            Err(e) => last = format!("{e:#}"),
        }
        if attempt + 1 < BUSY_RETRIES {
            std::thread::sleep(BUSY_WAIT);
        }
    }
    bail!("no free client slot on {SOCKET} after {BUSY_RETRIES} tries: {last}")
}

/// Send `Attach` with the one descriptor it must carry, and read the answer.
fn attach_on(
    socket: &OwnedFd,
    pitch: u32,
    offset: u32,
    fd: BorrowedFd<'_>,
) -> Result<Option<Reply>> {
    let request = request(
        TAG_ATTACH,
        0,
        0,
        PANEL.0,
        PANEL.1,
        FOURCC_XR24,
        pitch,
        offset,
    );
    send(socket, &request, Some(fd)).context("sendmsg(Attach)")?;
    recv(socket)
}

/// Re-open `path` and attach every one of `images` — `(pitch, offset, fd)` — again,
/// waiting out a daemon that is not ready yet.
///
/// This is [`connect_when_free`]'s idiom for the other case it has to survive: not a
/// daemon that still has another client, but one that has just dropped *this* client
/// because it reopened the panel. Every attempt opens a fresh socket, because the daemon
/// decides one-client-at-a-time at `accept` and a connection this process still holds is
/// that client. `log` is called once per attempt.
///
/// Returns the socket, the slot id the daemon gave each image in order, and the free
/// mask to start from — everything is free, since nothing has been presented yet.
fn reattach_all(
    path: &str,
    images: &[(u32, u32, BorrowedFd<'_>)],
    retries: u32,
    wait: Duration,
    mut log: impl FnMut(u32, &str),
) -> Result<(OwnedFd, Vec<u8>, u8)> {
    let mut last = String::new();
    for attempt in 0..retries.max(1) {
        match reattach_once(path, images) {
            Ok(got) => {
                log(attempt, "attached");
                return Ok(got);
            }
            Err(e) => last = format!("{e:#}"),
        }
        log(attempt, &last);
        if attempt + 1 < retries {
            std::thread::sleep(wait);
        }
    }
    bail!("could not re-attach to {path} after {retries} tries: {last}")
}

/// One attempt of [`reattach_all`]: a fresh socket and every image attached on it.
fn reattach_once(
    path: &str,
    images: &[(u32, u32, BorrowedFd<'_>)],
) -> Result<(OwnedFd, Vec<u8>, u8)> {
    let socket = connect_to(path)?;
    let mut ids = Vec::with_capacity(images.len());
    let mut free = 0u8;
    for (pitch, offset, fd) in images {
        let reply = attach_on(&socket, *pitch, *offset, *fd)?
            .ok_or_else(|| anyhow!("the daemon closed the connection during the attach"))?;
        reply.expect(REPLY_ATTACHED)?;
        free |= 1 << reply.slot;
        ids.push(reply.slot);
    }
    Ok((socket, ids, free))
}

/// One datagram, with at most one descriptor.
fn send(socket: &OwnedFd, request: &[u8; 32], fd: Option<BorrowedFd<'_>>) -> Result<()> {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut control = rustix::net::SendAncillaryBuffer::new(&mut space);
    let fds = fd.map(|f| [f]);
    if let Some(fds) = &fds
        && !control.push(rustix::net::SendAncillaryMessage::ScmRights(fds))
    {
        bail!("the ancillary buffer would not take the dma-buf fd");
    }
    rustix::net::sendmsg(
        socket,
        &[IoSlice::new(request)],
        &mut control,
        rustix::net::SendFlags::empty(),
    )?;
    Ok(())
}

/// One reply datagram, or `None` when the daemon has closed the connection.
///
/// **A zero-byte read is a handoff, not a protocol error.** The daemon closes its client
/// when it reopens the panel, and the slots go with the device that went away; the client
/// used to bail here, exit, and be restarted by systemd into another founding. It now
/// re-attaches instead ([`ShimScanout::reattach`]). A short *non-empty* datagram is still
/// a protocol error: `SOCK_SEQPACKET` does not fragment.
fn recv(socket: &OwnedFd) -> Result<Option<Reply>> {
    let mut buffer = [0u8; 12 + 512];
    let mut control = rustix::net::RecvAncillaryBuffer::default();
    let received = rustix::net::recvmsg(
        socket,
        &mut [IoSliceMut::new(&mut buffer)],
        &mut control,
        rustix::net::RecvFlags::empty(),
    )
    .context("recvmsg")?;
    if received.bytes == 0 {
        return Ok(None);
    }
    if received.bytes < 12 {
        bail!(
            "the daemon sent a {}-byte reply; the header is 12",
            received.bytes
        );
    }
    let len = u32::from_le_bytes(buffer[8..12].try_into().unwrap()) as usize;
    let end = (12 + len).min(received.bytes);
    Ok(Some(Reply {
        tag: buffer[0],
        slot: buffer[1],
        released: buffer[2],
        code: buffer[3],
        message: String::from_utf8_lossy(&buffer[12..end]).into_owned(),
    }))
}

/// The 32-byte request header.
#[allow(clippy::too_many_arguments)]
fn request(
    tag: u8,
    slot: u8,
    seq: u32,
    width: u32,
    height: u32,
    fourcc: u32,
    pitch: u32,
    offset: u32,
) -> [u8; 32] {
    let mut r = [0u8; 32];
    r[0] = tag;
    r[1] = slot;
    r[4..8].copy_from_slice(&seq.to_le_bytes());
    r[8..12].copy_from_slice(&width.to_le_bytes());
    r[12..16].copy_from_slice(&height.to_le_bytes());
    r[16..20].copy_from_slice(&fourcc.to_le_bytes());
    r[20..24].copy_from_slice(&pitch.to_le_bytes());
    r[24..28].copy_from_slice(&offset.to_le_bytes());
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake daemon on `path`: it accepts `closes` clients and drops each one without
    /// answering — the handoff — and then answers every `Attach` on the next client.
    /// Returns its thread, which ends once that client has had `answers` attaches.
    fn fake_daemon(path: String, closes: u32, answers: usize) -> std::thread::JoinHandle<()> {
        let listener = rustix::net::socket(
            rustix::net::AddressFamily::UNIX,
            rustix::net::SocketType::SEQPACKET,
            None,
        )
        .unwrap();
        rustix::net::bind(&listener, &rustix::net::SocketAddrUnix::new(&path).unwrap()).unwrap();
        rustix::net::listen(&listener, 8).unwrap();
        std::thread::spawn(move || {
            for _ in 0..closes {
                // Accepted and dropped: the client's next read is zero bytes.
                drop(rustix::net::accept(&listener).unwrap());
            }
            if answers == 0 {
                return;
            }
            let client = rustix::net::accept(&listener).unwrap();
            for slot in 0..answers {
                let mut buffer = [0u8; 64];
                let mut control = rustix::net::RecvAncillaryBuffer::default();
                let got = rustix::net::recvmsg(
                    &client,
                    &mut [IoSliceMut::new(&mut buffer)],
                    &mut control,
                    rustix::net::RecvFlags::empty(),
                )
                .unwrap();
                assert_eq!(got.bytes, 32, "the request is 32 bytes");
                assert_eq!(buffer[0], TAG_ATTACH);
                let mut reply = [0u8; 12];
                reply[0] = REPLY_ATTACHED;
                reply[1] = slot as u8;
                rustix::net::sendmsg(
                    &client,
                    &[IoSlice::new(&reply)],
                    &mut rustix::net::SendAncillaryBuffer::default(),
                    rustix::net::SendFlags::empty(),
                )
                .unwrap();
            }
        })
    }

    fn socket_path(name: &str) -> String {
        let dir =
            std::env::temp_dir().join(format!("cubarium-shim-test-{name}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join("frames.sock").to_string_lossy().into_owned()
    }

    /// **A handoff is not a failure.** The daemon closed this client when it reopened the
    /// panel; the client re-opens the socket, attaches the same dma-bufs again and carries
    /// on, instead of exiting into another four minutes of founding.
    #[test]
    fn a_closed_connection_is_re_attached_on_the_next_attempt() {
        let path = socket_path("handoff");
        let _ = std::fs::remove_file(&path);
        let daemon = fake_daemon(path.clone(), 1, 3);
        let fd = std::fs::File::open("/dev/null").unwrap();
        let images: Vec<(u32, u32, BorrowedFd<'_>)> =
            (0..3).map(|_| (4352u32, 0u32, fd.as_fd())).collect();
        let mut attempts = Vec::new();
        let (_socket, ids, free) = reattach_all(&path, &images, 4, Duration::ZERO, |n, what| {
            attempts.push(format!("{n}:{what}"))
        })
        .expect("the second attempt attaches");

        assert_eq!(ids, vec![0, 1, 2], "every slot is attached again");
        assert_eq!(free, 0b111, "and every one of them is free to draw into");
        assert_eq!(attempts.len(), 2, "one line per attempt: {attempts:?}");
        assert!(!attempts[0].ends_with("attached"), "{attempts:?}");
        assert!(attempts[1].ends_with("attached"), "{attempts:?}");
        daemon.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    /// The 0-byte reply itself: the daemon went away after reading, and `recv` says so
    /// rather than calling a 0-byte datagram a broken header.
    #[test]
    fn a_daemon_that_goes_away_reads_as_no_reply_and_not_as_a_short_one() {
        let (ours, theirs) = rustix::net::socketpair(
            rustix::net::AddressFamily::UNIX,
            rustix::net::SocketType::SEQPACKET,
            rustix::net::SocketFlags::empty(),
            None,
        )
        .unwrap();
        drop(theirs);
        assert!(recv(&ours).unwrap().is_none(), "a closed peer is a handoff");
    }

    /// Past the bound it fails as it always did, and says how many tries it had.
    #[test]
    fn a_daemon_that_never_comes_back_fails_after_the_bound() {
        let path = socket_path("gone");
        let _ = std::fs::remove_file(&path);
        let daemon = fake_daemon(path.clone(), 3, 0);
        let fd = std::fs::File::open("/dev/null").unwrap();
        let images = [(4352u32, 0u32, fd.as_fd())];
        let mut attempts = 0;
        let e = reattach_all(&path, &images, 3, Duration::ZERO, |_, _| attempts += 1)
            .expect_err("nothing ever answers");
        assert_eq!(attempts, 3, "one line per attempt");
        assert!(format!("{e:#}").contains("after 3 tries"), "{e:#}");
        daemon.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    /// The presenting thread reads the replies that have arrived and does not wait for
    /// one: waiting would put the daemon's whole vsync between one frame and the next.
    #[test]
    fn a_reply_that_has_not_arrived_is_not_waited_for() {
        let (ours, theirs) = rustix::net::socketpair(
            rustix::net::AddressFamily::UNIX,
            rustix::net::SocketType::SEQPACKET,
            rustix::net::SocketFlags::empty(),
            None,
        )
        .unwrap();
        assert!(!ready_to_read(&ours).unwrap(), "nothing has been sent");

        let mut reply = [0u8; 12];
        reply[0] = REPLY_PRESENTED;
        reply[2] = 0b010;
        rustix::net::sendmsg(
            &theirs,
            &[IoSlice::new(&reply)],
            &mut rustix::net::SendAncillaryBuffer::default(),
            rustix::net::SendFlags::empty(),
        )
        .unwrap();
        assert!(ready_to_read(&ours).unwrap(), "this one is waiting");
        assert_eq!(recv(&ours).unwrap().expect("a reply").released, 0b010);
        assert!(!ready_to_read(&ours).unwrap(), "and it was the only one");

        // A daemon that has gone away is readable, so the handoff is never missed.
        drop(theirs);
        assert!(
            ready_to_read(&ours).unwrap(),
            "the close is a readable event"
        );
    }

    #[test]
    fn an_attach_request_is_the_thirty_two_bytes_the_daemon_documents() {
        // `slot` and `seq` are reserved on an Attach — the daemon picks the slot — and
        // the daemon refuses the message outright if either is non-zero.
        let r = request(TAG_ATTACH, 0, 0, 1080, 1920, FOURCC_XR24, 4352, 0);
        assert_eq!(r.len(), 32);
        assert_eq!(&r[0..8], &[1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&r[8..12], &1080u32.to_le_bytes());
        assert_eq!(&r[12..16], &1920u32.to_le_bytes());
        assert_eq!(&r[16..20], b"XR24", "the fourcc reads as four ASCII bytes");
        assert_eq!(&r[20..24], &4352u32.to_le_bytes());
        assert_eq!(&r[24..32], &[0u8; 8]);
    }

    #[test]
    fn a_present_describes_nothing_but_its_slot_and_sequence() {
        // Every geometry field is reserved on a Present; sending the width again is
        // refused with BadMessage, which is how this was found.
        let r = request(TAG_PRESENT, 2, 7, 0, 0, 0, 0, 0);
        assert_eq!(&r[0..4], &[2, 2, 0, 0]);
        assert_eq!(&r[4..8], &7u32.to_le_bytes());
        assert_eq!(&r[8..32], &[0u8; 24]);
    }

    #[test]
    fn a_detach_carries_only_its_slot() {
        let r = request(TAG_DETACH, 1, 0, 0, 0, 0, 0, 0);
        assert_eq!(&r[0..4], &[3, 1, 0, 0]);
        assert_eq!(&r[4..32], &[0u8; 28]);
    }
}
