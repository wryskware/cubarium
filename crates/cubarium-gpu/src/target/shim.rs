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
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use ash::vk;

use super::dmabuf::{self, LinearImage};
use crate::render::{PresentTransform, Renderer, TargetImage};
use crate::scene::Scene;
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
const BUSY_WAIT: std::time::Duration = std::time::Duration::from_millis(100);

struct Slot {
    /// The slot number the **daemon** assigned in its `Attached` reply. It is not the
    /// index in this vector: `Attach` carries slot 0 and the daemon picks, and the
    /// `released` bitmask is in the daemon's numbering.
    id: u8,
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
    pub fn open(gpu: &Gpu, renderer: &mut Renderer, quarter_turns: u32) -> Result<ShimScanout> {
        if !gpu.has_dma_buf {
            bail!("this device has no VK_EXT_external_memory_dma_buf; the shim cannot be fed");
        }
        dmabuf::linear_export_supported(gpu)?;
        let shader_encode = !dmabuf::srgb_view_supported(gpu);
        let transform = PresentTransform::fit(
            (renderer.layout.w, renderer.layout.h),
            PANEL,
            quarter_turns,
            shader_encode,
        )
        .ok_or_else(|| {
            anyhow!(
                "a {}x{} raster does not fit {}x{} at {quarter_turns} quarter turn(s)",
                renderer.layout.w,
                renderer.layout.h,
                PANEL.0,
                PANEL.1
            )
        })?;

        let view_format = if shader_encode {
            dmabuf::FORMAT
        } else {
            vk::Format::B8G8R8A8_SRGB
        };
        let pass = renderer.present_pass(gpu, view_format, vk::ImageLayout::GENERAL)?;
        let d = &gpu.device;
        let command_buffers = unsafe {
            d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(renderer.command_pool)
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
        let first_fd = first.fd.take().expect("a freshly exported image has its fd");
        let (socket, attached) = connect_when_free(&first, first_fd.as_fd())?;
        drop(first_fd);
        let mut client = ShimScanout {
            socket,
            slots: Vec::with_capacity(SLOTS),
            free: attached.released | (1 << attached.slot),
            transform,
            view_format,
            seq: 0,
            next: 0,
        };
        let mut pending = vec![(attached.slot, first)];
        for _ in 1..SLOTS {
            let mut image = dmabuf::export_linear(gpu, PANEL.0, PANEL.1, shader_encode)?;
            let fd = image.fd.take().expect("a freshly exported image has its fd");
            let id = client.attach(&image, fd.as_fd())?;
            // The daemon has imported the fd, so ours is dropped here rather than
            // kept: the framebuffer it made is what lives on.
            drop(fd);
            client.free |= 1 << id;
            pending.push((id, image));
        }
        for (i, (id, image)) in pending.into_iter().enumerate() {
            let target = TargetImage {
                image: image.image,
                view: image.view,
                framebuffer: crate::render::framebuffer(d, pass, image.view, PANEL.0, PANEL.1)?,
            };
            client.slots.push(Slot {
                id,
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
            if shader_encode { "the present shader" } else { "the _SRGB attachment" }
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
    pub fn draw(&mut self, gpu: &Gpu, renderer: &mut Renderer, scene: &Scene) -> Result<(f64, f64, f64)> {
        let waited = Instant::now();
        let index = self.take_free_slot()?;
        let d = &gpu.device;
        let start = Instant::now();
        {
            let slot = &self.slots[index];
            unsafe { d.reset_command_buffer(slot.command_buffer, vk::CommandBufferResetFlags::empty()) }?;
            renderer.record(
                gpu,
                slot.command_buffer,
                scene,
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
                d.queue_submit(gpu.queue, &[vk::SubmitInfo::default().command_buffers(&one)], slot.fence)?;
                d.wait_for_fences(&[slot.fence], true, u64::MAX)?;
            }
        }
        let submitted = Instant::now();
        self.present(self.slots[index].id)?;
        Ok((
            renderer.gpu_ms(gpu),
            (submitted - start).as_secs_f64() * 1e3,
            (start - waited).as_secs_f64() * 1e3,
        ))
    }

    /// The last presented slot's contents, read back through the GPU as RGBA8: proof
    /// that what the daemon is scanning out is the frame that was drawn.
    pub fn read_presented(&self, gpu: &Gpu, renderer: &Renderer) -> Result<(u32, u32, Vec<u8>)> {
        let index = (self.next + SLOTS - 1) % SLOTS;
        let rgba = dmabuf::read_back(
            gpu,
            renderer.command_pool,
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
        let reply = attach_on(&self.socket, image, fd)?;
        self.free |= reply.released;
        reply.expect(REPLY_ATTACHED)?;
        Ok(reply.slot)
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
        send(&self.socket, &request(TAG_PRESENT, slot, self.seq, 0, 0, 0, 0, 0), None)
            .context("sendmsg(Present)")
    }

    /// Read one reply and fold its `released` mask into the free set.
    fn read_reply(&mut self, expect: u8) -> Result<()> {
        let reply = recv(&self.socket)?;
        // `released` is the only signal a buffer is free.
        self.free |= reply.released;
        reply.expect(expect)
    }

    pub fn destroy(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe { let _ = d.device_wait_idle(); }
        for i in 0..self.slots.len() {
            let request = request(TAG_DETACH, self.slots[i].id, 0, 0, 0, 0, 0, 0);
            if send(&self.socket, &request, None).is_ok() {
                let _ = self.read_reply(REPLY_DETACHED);
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
            bail!("the daemon refused the request with code {}: {}", self.code, self.message);
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

/// A connected, unattached client socket.
fn connect() -> Result<OwnedFd> {
    let socket = rustix::net::socket(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::SEQPACKET,
        None,
    )
    .context("socket(AF_UNIX, SOCK_SEQPACKET)")?;
    rustix::net::connect(&socket, &rustix::net::SocketAddrUnix::new(SOCKET)?)
        .with_context(|| format!("connect {SOCKET} (is cube-screen-shim running?)"))?;
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
        match attach_on(&socket, image, fd) {
            Ok(reply) if !reply.is_busy() => {
                reply.expect(REPLY_ATTACHED)?;
                return Ok((socket, reply));
            }
            Ok(reply) => last = format!("the daemon is busy: {}", reply.message),
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
fn attach_on(socket: &OwnedFd, image: &LinearImage, fd: BorrowedFd<'_>) -> Result<Reply> {
    let request = request(TAG_ATTACH, 0, 0, PANEL.0, PANEL.1, FOURCC_XR24, image.pitch, image.offset);
    send(socket, &request, Some(fd)).context("sendmsg(Attach)")?;
    recv(socket)
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

/// One reply datagram.
fn recv(socket: &OwnedFd) -> Result<Reply> {
    let mut buffer = [0u8; 12 + 512];
    let mut control = rustix::net::RecvAncillaryBuffer::default();
    let received = rustix::net::recvmsg(
        socket,
        &mut [IoSliceMut::new(&mut buffer)],
        &mut control,
        rustix::net::RecvFlags::empty(),
    )
    .context("recvmsg")?;
    if received.bytes < 12 {
        bail!("the daemon sent a {}-byte reply; the header is 12", received.bytes);
    }
    let len = u32::from_le_bytes(buffer[8..12].try_into().unwrap()) as usize;
    let end = (12 + len).min(received.bytes);
    Ok(Reply {
        tag: buffer[0],
        slot: buffer[1],
        released: buffer[2],
        code: buffer[3],
        message: String::from_utf8_lossy(&buffer[12..end]).into_owned(),
    })
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
