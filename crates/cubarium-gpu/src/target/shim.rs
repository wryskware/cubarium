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
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use ash::vk;

use super::dmabuf::{self, LinearImage};
use crate::present::FrameSource;
use crate::render::{PresentTransform, TargetImage};
use crate::vk::Gpu;

/// The daemon's socket.
pub const SOCKET: &str = "/run/cube-screen-shim/frames.sock";
/// The panel, which the daemon fixes: the client rotates into it.
pub const PANEL: (u32, u32) = (1080, 1920);
/// `XR24`, the only format the spike's modifier-free `AddFB2` accepted.
const FOURCC_XR24: u32 = u32::from_le_bytes(*b"XR24");
/// Slots to attach. Three lets one be scanned out, one be rendered into and one be in
/// flight, which is what keeps the render off the flip's critical path.
const SLOTS: usize = 3;

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
            unsafe {
                d.reset_fences(&[slot.fence])?;
                d.queue_submit(
                    gpu.queue,
                    &[vk::SubmitInfo::default().command_buffers(&one)],
                    slot.fence,
                )?;
                d.wait_for_fences(&[slot.fence], true, u64::MAX)?;
            }
        }
        let submitted = Instant::now();
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
    let request = request(TAG_ATTACH, 0, 0, PANEL.0, PANEL.1, FOURCC_XR24, pitch, offset);
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
        let dir = std::env::temp_dir().join(format!("cubarium-shim-test-{name}-{}", std::process::id()));
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
        let (_socket, ids, free) = reattach_all(
            &path,
            &images,
            4,
            Duration::ZERO,
            |n, what| attempts.push(format!("{n}:{what}")),
        )
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
