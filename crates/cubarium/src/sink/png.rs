//! PNG capture sink: the unfolded net at scale 1, every `--every` rendered frames.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cube_proto::Frame;

use crate::net::{PNG_HEIGHT, PNG_WIDTH, net_rgb8};

use super::FrameSink;

/// Writes `frame_NNNNNN.png` captures of the net layout, plus `final.png` on exit.
pub struct PngSink {
    dir: PathBuf,
    every: u64,
    seen: u64,
    saved: u64,
    last: Option<Frame>,
    buf: Vec<u8>,
}

impl PngSink {
    pub fn new(dir: impl Into<PathBuf>, every: u64) -> Result<PngSink> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating capture directory {}", dir.display()))?;
        Ok(PngSink { dir, every: every.max(1), seen: 0, saved: 0, last: None, buf: Vec::new() })
    }

    pub fn saved(&self) -> u64 {
        self.saved
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn write(&mut self, frame: &Frame, name: &str) -> Result<PathBuf> {
        net_rgb8(frame, &mut self.buf);
        let path = self.dir.join(name);
        write_net_png(&path, &self.buf)?;
        Ok(path)
    }
}

impl FrameSink for PngSink {
    fn submit(&mut self, frame: &Frame) -> Result<()> {
        if self.seen.is_multiple_of(self.every) {
            let name = format!("frame_{:06}.png", self.saved);
            self.write(frame, &name)?;
            self.saved += 1;
        }
        self.seen += 1;
        self.last = Some(frame.clone());
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        if let Some(frame) = self.last.take() {
            self.write(&frame, "final.png")?;
        }
        Ok(())
    }
}

/// Write one `PNG_WIDTH` × `PNG_HEIGHT` RGB8 buffer as a PNG.
pub fn write_net_png(path: &Path, rgb: &[u8]) -> Result<()> {
    anyhow::ensure!(
        rgb.len() == PNG_WIDTH * PNG_HEIGHT * 3,
        "net buffer is {} bytes, expected {}",
        rgb.len(),
        PNG_WIDTH * PNG_HEIGHT * 3
    );
    let file =
        File::create(path).with_context(|| format!("creating capture {}", path.display()))?;
    let mut enc = ::png::Encoder::new(BufWriter::new(file), PNG_WIDTH as u32, PNG_HEIGHT as u32);
    enc.set_color(::png::ColorType::Rgb);
    enc.set_depth(::png::BitDepth::Eight);
    let mut writer = enc.write_header().context("writing PNG header")?;
    writer.write_image_data(rgb).context("writing PNG data")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_proto::Face;

    #[test]
    fn captures_land_on_the_documented_cadence_and_a_final_frame_is_written() {
        let dir = std::env::temp_dir().join(format!("cubarium-png-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut sink = PngSink::new(&dir, 3).unwrap();
        let mut frame = Frame::black();
        for i in 0..10u8 {
            frame.set(Face::Front, 0, 0, [i, i, i]);
            sink.submit(&frame).unwrap();
        }
        sink.finish().unwrap();
        assert_eq!(sink.saved(), 4, "frames 0, 3, 6, 9");
        for i in 0..4 {
            assert!(dir.join(format!("frame_{i:06}.png")).exists());
        }
        assert!(dir.join("final.png").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
