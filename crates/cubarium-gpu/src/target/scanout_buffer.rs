//! Safe, CPU-writable DRM buffers for the legacy panel handoff.
//!
//! wgpu renders the same panel-sized image into a texture. Its readback is copied
//! into this buffer before presenting; the shim still owns mapping and page flips.

use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use drm::buffer::{Buffer, DrmFourcc};
use drm::control::{Device as ControlDevice, dumbbuffer::DumbBuffer};

struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl drm::Device for Card {}
impl ControlDevice for Card {}

/// A linear XR24 allocation, exported once and written only while its slot is free.
pub struct ScanoutBuffer {
    card: Card,
    buffer: DumbBuffer,
}

impl ScanoutBuffer {
    /// Opens a DRM allocation client without taking DRM master from the shim.
    pub fn new(path: &Path, width: u32, height: u32) -> Result<Self> {
        ensure!(
            width > 0 && height > 0,
            "scanout dimensions must be nonzero"
        );
        let card = Card(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .with_context(|| format!("open DRM allocation client {}", path.display()))?,
        );
        let buffer = card
            .create_dumb_buffer((width, height), DrmFourcc::Xrgb8888, 32)
            .context("allocate linear DRM scanout buffer")?;
        Ok(Self { card, buffer })
    }

    pub fn pitch(&self) -> u32 {
        self.buffer.pitch()
    }

    pub fn export(&self) -> Result<OwnedFd> {
        let flags = rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::RDWR;
        self.card
            .buffer_to_prime_fd(self.buffer.handle(), flags.bits())
            .context("export DRM scanout allocation as dma-buf")
    }

    /// Copies tightly packed sRGB RGBA bytes into the buffer's padded BGRA rows.
    pub fn write_rgba(&mut self, rgba: &[u8]) -> Result<()> {
        let (width, height) = self.buffer.size();
        let row_bytes = width as usize * 4;
        ensure!(
            rgba.len() == row_bytes * height as usize,
            "wrong scanout readback size"
        );
        let pitch = self.buffer.pitch() as usize;
        ensure!(
            pitch >= row_bytes,
            "DRM pitch is smaller than a scanout row"
        );
        let mut mapped = self
            .card
            .map_dumb_buffer(&mut self.buffer)
            .context("map DRM scanout allocation")?;
        copy_rows(rgba, &mut mapped, row_bytes, pitch, height as usize)
    }
}

fn copy_rows(
    rgba: &[u8],
    mapped: &mut [u8],
    row_bytes: usize,
    pitch: usize,
    rows: usize,
) -> Result<()> {
    ensure!(
        row_bytes > 0 && row_bytes % 4 == 0 && pitch >= row_bytes,
        "invalid scanout stride"
    );
    ensure!(
        rgba.len() == row_bytes * rows && mapped.len() >= pitch * rows,
        "scanout allocation does not cover the frame"
    );
    for (source, destination) in rgba
        .chunks_exact(row_bytes)
        .zip(mapped.chunks_mut(pitch).take(rows))
    {
        for (pixel, target) in source
            .chunks_exact(4)
            .zip(destination[..row_bytes].chunks_exact_mut(4))
        {
            target.copy_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
        }
    }
    Ok(())
}

impl Drop for ScanoutBuffer {
    fn drop(&mut self) {
        let _ = self.card.destroy_dumb_buffer(self.buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::copy_rows;

    #[test]
    fn padded_scanout_preserves_rows_and_swaps_red_blue() {
        let rgba = [10, 20, 30, 40, 50, 60, 70, 80];
        let mut mapped = [99; 16];
        copy_rows(&rgba, &mut mapped, 4, 8, 2).unwrap();
        assert_eq!(&mapped[0..4], &[30, 20, 10, 255]);
        assert_eq!(&mapped[8..12], &[70, 60, 50, 255]);
        assert_eq!(&mapped[4..8], &[99; 4]);
        assert_eq!(&mapped[12..16], &[99; 4]);
    }

    #[test]
    fn a_short_mapping_is_rejected_before_writing() {
        let mut mapped = [99; 4];
        assert!(copy_rows(&[10, 20, 30, 40], &mut mapped, 4, 8, 1).is_err());
        assert_eq!(mapped, [99; 4]);
    }
}
