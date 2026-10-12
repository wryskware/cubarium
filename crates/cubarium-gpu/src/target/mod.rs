//! Safe headless/window presentation and Linux panel transports.
pub use crate::wgpu_backend::headless::Headless;
use anyhow::Result;
#[cfg(all(feature = "scanout", target_os = "linux"))]
pub mod kms;
#[cfg(all(feature = "scanout", target_os = "linux"))]
mod panel_readback;
pub mod presenter;
pub mod readback;
#[cfg(all(feature = "scanout", target_os = "linux"))]
mod scanout;
#[cfg(all(feature = "scanout", target_os = "linux"))]
mod scanout_buffer;
#[cfg(all(feature = "scanout", target_os = "linux"))]
mod shim;
pub mod window;
#[cfg(all(feature = "scanout", target_os = "linux"))]
pub use scanout::Scanout;
#[cfg(all(feature = "scanout", target_os = "linux"))]
pub use shim::{PANEL as SHIM_PANEL, SOCKET as SHIM_SOCKET, ShimPresenter, ShimScanout};
pub fn write_png(path: &std::path::Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

/// Read an RGBA8 PNG back.
pub fn read_png(path: &std::path::Path) -> Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buffer)?;
    buffer.truncate(info.buffer_size());
    Ok((info.width, info.height, buffer))
}
