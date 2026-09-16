//! The KMS half of the scanout target: take DRM master on `/dev/dri/card0`, drive one
//! connector, import dma-bufs as framebuffers and flip them.
//!
//! Derived from `spikes/gpu-scanout/src/kms.rs`, which is in turn cut down from
//! `cube-kms` in led-cube-shim. **The modifier-free `AddFB2` is not an
//! oversimplification**: the 5.4 downstream KMS driver on this board exposes no
//! `IN_FORMATS` blob on its planes and rejects the one modifier the Vulkan blob
//! advertises, so a plain `AddFB2` over a `VK_IMAGE_TILING_LINEAR` image is the only
//! accepted route (`gpu-scanout-spike-2026-09-16.md` Q2).

use std::fs::{File, OpenOptions};
use std::os::unix::io::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use drm::Device as _;
use drm::buffer::{DrmFourcc, DrmModifier, Handle as BufHandle, PlanarBuffer};
use drm::control::{
    Device as ControlDevice, Event, FbCmd2Flags, Mode, PageFlipFlags, connector, crtc, framebuffer,
};

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

/// A dma-buf imported into DRM, described well enough for `AddFB2`.
pub struct ImportedBuffer {
    pub handle: BufHandle,
    pub width: u32,
    pub height: u32,
    pub fourcc: DrmFourcc,
    pub pitch: u32,
    pub offset: u32,
    /// Kept so the exporter's fd outlives the import.
    _fd: OwnedFd,
}

impl PlanarBuffer for ImportedBuffer {
    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    fn format(&self) -> DrmFourcc {
        self.fourcc
    }
    fn modifier(&self) -> Option<DrmModifier> {
        None
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

/// One connector, its CRTC and its mode.
pub struct Output {
    pub card: Card,
    pub connector: connector::Handle,
    pub crtc: crtc::Handle,
    pub mode: Mode,
    pub width: u32,
    pub height: u32,
    /// Framebuffers are kept alive for the life of the output.
    buffers: Vec<ImportedBuffer>,
}

impl Output {
    /// Open `card_path` and take mastership, resolving `name` (e.g. "DP-1").
    ///
    /// Taking DRM master fails while `cube-screen-shim` (or another worker's test) holds
    /// it. The caller is expected to retry rather than to kill anything.
    pub fn open(card_path: &Path, name: &str) -> Result<Output> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(card_path)
            .with_context(|| format!("open {}", card_path.display()))?;
        let card = Card { file };
        card.acquire_master_lock()
            .context("acquire DRM master (is cube-screen-shim, or another test, holding it?)")?;

        let resources = card.resource_handles().context("resource_handles")?;
        let mut found = None;
        for &c in resources.connectors() {
            let info = card.get_connector(c, false)?;
            if format!("{}-{}", info.interface().as_str(), info.interface_id()) == name {
                found = Some((c, info));
                break;
            }
        }
        let (connector, info) =
            found.ok_or_else(|| anyhow!("no connector named {name} on {}", card_path.display()))?;
        let mode = *info
            .modes()
            .first()
            .ok_or_else(|| anyhow!("connector {name} reports no modes"))?;

        let mut crtc = info
            .current_encoder()
            .and_then(|e| card.get_encoder(e).ok())
            .and_then(|e| e.crtc());
        if crtc.is_none() {
            for &e in info.encoders() {
                let Ok(encoder) = card.get_encoder(e) else { continue };
                if let Some(&c) = resources.filter_crtcs(encoder.possible_crtcs()).first() {
                    crtc = Some(c);
                    break;
                }
            }
        }
        let crtc = crtc
            .or_else(|| resources.crtcs().first().copied())
            .ok_or_else(|| anyhow!("no CRTC for {name}"))?;
        let (w, h) = mode.size();
        Ok(Output {
            card,
            connector,
            crtc,
            mode,
            width: u32::from(w),
            height: u32::from(h),
            buffers: Vec::new(),
        })
    }

    /// The mode's refresh, in Hz.
    pub fn refresh_hz(&self) -> f64 {
        let (w, h) = (self.mode.hsync().2, self.mode.vsync().2);
        if w == 0 || h == 0 {
            return f64::from(self.mode.vrefresh());
        }
        f64::from(self.mode.clock()) * 1000.0 / (f64::from(w) * f64::from(h))
    }

    /// Import a dma-buf fd and register it as a framebuffer, modifier-free.
    pub fn import_dmabuf(
        &mut self,
        fd: OwnedFd,
        width: u32,
        height: u32,
        fourcc: DrmFourcc,
        pitch: u32,
        offset: u32,
    ) -> Result<framebuffer::Handle> {
        let handle = self.card.prime_fd_to_buffer(fd.as_fd()).context("drmPrimeFDToHandle")?;
        let buffer = ImportedBuffer { handle, width, height, fourcc, pitch, offset, _fd: fd };
        let fb = self
            .card
            .add_planar_framebuffer(&buffer, FbCmd2Flags::empty())
            .with_context(|| format!("drmModeAddFB2({width}x{height} {fourcc:?} pitch={pitch})"))?;
        self.buffers.push(buffer);
        Ok(fb)
    }

    pub fn set_crtc(&self, fb: framebuffer::Handle) -> Result<()> {
        self.card
            .set_crtc(self.crtc, Some(fb), (0, 0), &[self.connector], Some(self.mode))
            .context("drmModeSetCrtc")
    }

    /// Queue a flip and block until flip-complete.
    pub fn flip(&self, fb: framebuffer::Handle) -> Result<()> {
        self.card
            .page_flip(self.crtc, fb, PageFlipFlags::EVENT, None)
            .context("drmModePageFlip")?;
        self.wait_flip()
    }

    fn wait_flip(&self) -> Result<()> {
        use rustix::event::{PollFd, PollFlags, poll};
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(anyhow!("timed out waiting for flip-complete"));
            }
            let timespec = rustix::event::Timespec {
                tv_sec: left.as_secs() as _,
                tv_nsec: left.subsec_nanos() as _,
            };
            let mut fds = [PollFd::new(&self.card, PollFlags::IN)];
            match poll(&mut fds, Some(&timespec)) {
                Ok(0) => continue,
                Ok(_) => {}
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.into()),
            }
            for event in self.card.receive_events()? {
                if let Event::PageFlip(f) = event
                    && f.crtc == self.crtc
                {
                    return Ok(());
                }
            }
        }
    }
}
