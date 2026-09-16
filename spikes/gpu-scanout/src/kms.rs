//! The KMS half of the spike: become DRM master on /dev/dri/card0, drive one
//! connector, and flip framebuffers at it. Modelled on `cube-kms` in
//! led-cube-shim, cut down to what the spike needs.
use std::fs::{File, OpenOptions};
use std::os::unix::io::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use drm::buffer::{DrmFourcc, DrmModifier, Handle as BufHandle, PlanarBuffer};
use drm::control::{connector, crtc, framebuffer, Device as ControlDevice, Event, FbCmd2Flags, Mode, PageFlipFlags};
use drm::Device as _;

pub struct Card {
    file: File,
}
impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}
impl drm::Device for Card {}
impl ControlDevice for Card {}

/// A dma-buf imported into DRM, described well enough for AddFB2 with modifiers.
pub struct ImportedBuffer {
    pub handle: BufHandle,
    pub width: u32,
    pub height: u32,
    pub fourcc: DrmFourcc,
    pub modifier: DrmModifier,
    pub pitch: u32,
    pub offset: u32,
    /// Kept so the exporter's fd outlives the import.
    pub _fd: OwnedFd,
}
impl PlanarBuffer for ImportedBuffer {
    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    fn format(&self) -> DrmFourcc {
        self.fourcc
    }
    fn modifier(&self) -> Option<DrmModifier> {
        Some(self.modifier)
    }
    fn pitches(&self) -> [u32; 4] {
        [self.pitch, 0, 0, 0]
    }
    fn handles(&self) -> [Option<BufHandle>; 4] {
        [Some(self.handle), None, None, None]
    }
    fn offsets(&self) -> [u32; 4] {
        [self.offset, 0, 0, 0]
    }
}

pub struct Output {
    pub card: Card,
    pub connector: connector::Handle,
    pub crtc: crtc::Handle,
    pub mode: Mode,
    pub width: u32,
    pub height: u32,
}

impl Output {
    /// Open `card` and take mastership, resolving `name` (e.g. "DP-1").
    pub fn open(card_path: &Path, name: &str) -> Result<Output> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(card_path)
            .with_context(|| format!("open {}", card_path.display()))?;
        let card = Card { file };
        card.acquire_master_lock()
            .context("acquire DRM master (is cube-screen-shim still running?)")?;

        let res = card.resource_handles().context("resource_handles")?;
        let mut found = None;
        for &c in res.connectors() {
            let info = card.get_connector(c, false)?;
            let n = format!("{}-{}", info.interface().as_str(), info.interface_id());
            if n == name {
                found = Some((c, info));
                break;
            }
        }
        let (conn, info) = found.ok_or_else(|| anyhow!("no connector named {name} on {}", card_path.display()))?;
        let mode = *info
            .modes()
            .first()
            .ok_or_else(|| anyhow!("connector {name} reports no modes"))?;

        // Follow the connector's current encoder if it has one, else the first
        // CRTC its encoders allow.
        let mut crtc = info.current_encoder().and_then(|e| card.get_encoder(e).ok()).and_then(|e| e.crtc());
        if crtc.is_none() {
            for &e in info.encoders() {
                let Ok(enc) = card.get_encoder(e) else { continue };
                if let Some(&c) = res.filter_crtcs(enc.possible_crtcs()).first() {
                    crtc = Some(c);
                    break;
                }
            }
        }
        let crtc = crtc.or_else(|| res.crtcs().first().copied()).ok_or_else(|| anyhow!("no CRTC for {name}"))?;

        let (w, h) = mode.size();
        Ok(Output { card, connector: conn, crtc, mode, width: u32::from(w), height: u32::from(h) })
    }

    /// Import a dma-buf fd and register it as a framebuffer with its modifier.
    pub fn import_dmabuf(
        &self,
        fd: OwnedFd,
        width: u32,
        height: u32,
        fourcc: DrmFourcc,
        modifier: DrmModifier,
        pitch: u32,
        offset: u32,
    ) -> Result<(ImportedBuffer, framebuffer::Handle)> {
        let handle = self
            .card
            .prime_fd_to_buffer(fd.as_fd())
            .context("drmPrimeFDToHandle")?;
        let modifier = if std::env::var("GPUS_FB_NOMOD").is_ok() { DrmModifier::Invalid } else { modifier };
        let buf = ImportedBuffer { handle, width, height, fourcc, modifier, pitch, offset, _fd: fd };
        // GPUS_FB_NOMOD=1 asks for a plain AddFB2 (no modifier), which is what a
        // kernel without IN_FORMATS understands.
        let nomod = std::env::var("GPUS_FB_NOMOD").is_ok() || modifier == DrmModifier::Invalid;
        let flags = if nomod { FbCmd2Flags::empty() } else { FbCmd2Flags::MODIFIERS };
        let fb = self
            .card
            .add_planar_framebuffer(&buf, flags)
            .with_context(|| format!("drmModeAddFB2WithModifiers({width}x{height} {fourcc:?} mod={modifier:?} pitch={pitch})"))?;
        Ok((buf, fb))
    }

    /// Export a DRM buffer handle (e.g. a dumb buffer's) as a dma-buf fd.
    pub fn export_prime(&self, handle: BufHandle) -> Result<OwnedFd> {
        // O_CLOEXEC | O_RDWR: the GPU needs to write into it.
        self.card.buffer_to_prime_fd(handle, 0o2000000 | 0o2).context("drmPrimeHandleToFD")
    }

    pub fn set_crtc(&self, fb: framebuffer::Handle) -> Result<()> {
        self.card
            .set_crtc(self.crtc, Some(fb), (0, 0), &[self.connector], Some(self.mode))
            .context("drmModeSetCrtc")
    }

    /// Queue a flip and block until the flip-complete event arrives.
    pub fn flip(&self, fb: framebuffer::Handle) -> Result<()> {
        self.card
            .page_flip(self.crtc, fb, PageFlipFlags::EVENT, None)
            .context("drmModePageFlip")?;
        self.wait_flip()
    }

    fn wait_flip(&self) -> Result<()> {
        use rustix::event::{poll, PollFd, PollFlags};
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(anyhow!("timed out waiting for flip-complete"));
            }
            let ts = rustix::event::Timespec { tv_sec: left.as_secs() as _, tv_nsec: left.subsec_nanos() as _ };
            let mut fds = [PollFd::new(&self.card, PollFlags::IN)];
            match poll(&mut fds, Some(&ts)) {
                Ok(0) => continue,
                Ok(_) => {}
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.into()),
            }
            for ev in self.card.receive_events()? {
                if let Event::PageFlip(f) = ev {
                    if f.crtc == self.crtc {
                        return Ok(());
                    }
                }
            }
        }
    }
}
