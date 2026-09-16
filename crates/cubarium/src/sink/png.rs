//! PNG capture sink: every `--every` rendered frames, plus `final.png` on exit.
//!
//! A cube world is written as the unfolded net at scale 1 (256×128), exactly as before.
//! A ring world is written as its own raster, `w×h`, one PNG pixel per world pixel — no
//! layout, no upscale: the capture *is* the image the shim and the viewer get.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cube_proto::{Frame, Raster};

use crate::net::{PNG_HEIGHT, PNG_WIDTH, net_rgb8};

use super::{FrameSink, Output};

/// The newest image seen, kept so `finish` can write `final.png`.
enum Last {
    Cube(Frame),
    Ring(Raster),
}

/// Writes `frame_NNNNNN.png` captures, plus `final.png` on exit.
pub struct PngSink {
    dir: PathBuf,
    every: u64,
    seen: u64,
    saved: u64,
    last: Option<Last>,
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

    fn write_cube(&mut self, frame: &Frame, name: &str) -> Result<PathBuf> {
        net_rgb8(frame, &mut self.buf);
        let path = self.dir.join(name);
        write_net_png(&path, &self.buf)?;
        Ok(path)
    }

    fn write_ring(&mut self, raster: &Raster, name: &str) -> Result<PathBuf> {
        let path = self.dir.join(name);
        let (w, h) = raster.size();
        write_rgb8_png(&path, u32::from(w), u32::from(h), raster.as_bytes())?;
        Ok(path)
    }

    fn write_last(&mut self, name: &str) -> Result<()> {
        match self.last.take() {
            Some(Last::Cube(frame)) => self.write_cube(&frame, name).map(|_| ()),
            Some(Last::Ring(raster)) => self.write_ring(&raster, name).map(|_| ()),
            None => Ok(()),
        }
    }
}

impl FrameSink for PngSink {
    fn submit(&mut self, out: Output<'_>) -> Result<()> {
        if self.seen.is_multiple_of(self.every) {
            let name = format!("frame_{:06}.png", self.saved);
            match out {
                Output::Cube(frame) => self.write_cube(frame, &name)?,
                Output::Ring(raster) => self.write_ring(raster, &name)?,
            };
            self.saved += 1;
        }
        self.seen += 1;
        self.last = Some(match out {
            Output::Cube(frame) => Last::Cube(frame.clone()),
            Output::Ring(raster) => Last::Ring(raster.clone()),
        });
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.write_last("final.png")
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
    write_rgb8_png(path, PNG_WIDTH as u32, PNG_HEIGHT as u32, rgb)
}

/// Write a tightly packed `width × height` RGB8 buffer as a PNG. The one encoder both
/// topologies go through, so a ring capture and a cube capture differ only in their size.
pub fn write_rgb8_png(path: &Path, width: u32, height: u32, rgb: &[u8]) -> Result<()> {
    anyhow::ensure!(
        rgb.len() as u64 == u64::from(width) * u64::from(height) * 3,
        "image buffer is {} bytes, expected {} for {width}x{height}",
        rgb.len(),
        u64::from(width) * u64::from(height) * 3
    );
    let file =
        File::create(path).with_context(|| format!("creating capture {}", path.display()))?;
    let mut enc = ::png::Encoder::new(BufWriter::new(file), width, height);
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

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cubarium-png-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn captures_land_on_the_documented_cadence_and_a_final_frame_is_written() {
        let dir = scratch("cube");
        let mut sink = PngSink::new(&dir, 3).unwrap();
        let mut frame = Frame::black();
        for i in 0..10u8 {
            frame.set(Face::Front, 0, 0, [i, i, i]);
            sink.submit(Output::Cube(&frame)).unwrap();
        }
        sink.finish().unwrap();
        assert_eq!(sink.saved(), 4, "frames 0, 3, 6, 9");
        for i in 0..4 {
            assert!(dir.join(format!("frame_{i:06}.png")).exists());
        }
        assert!(dir.join("final.png").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A ring capture is the raster itself: same cadence, same names, and the PNG's own
    /// header carries the world's size rather than the net's 256×128.
    #[test]
    fn a_ring_capture_is_the_raster_at_its_own_size() {
        for (w, h) in [(320u16, 180u16), (640, 360)] {
            let dir = scratch(&format!("ring-{w}x{h}"));
            let mut sink = PngSink::new(&dir, 2).unwrap();
            let mut raster = Raster::black(w, h);
            for i in 0..4u8 {
                raster.set(i.into(), 1, [i, 2 * i, 3 * i]);
                sink.submit(Output::Ring(&raster)).unwrap();
            }
            sink.finish().unwrap();
            assert_eq!(sink.saved(), 2, "frames 0 and 2");

            let file = std::io::BufReader::new(
                std::fs::File::open(dir.join("final.png")).unwrap(),
            );
            let decoder = ::png::Decoder::new(file);
            let mut reader = decoder.read_info().unwrap();
            let info = reader.info();
            assert_eq!((info.width, info.height), (u32::from(w), u32::from(h)));
            assert_eq!(info.color_type, ::png::ColorType::Rgb);
            let mut back = vec![0u8; reader.output_buffer_size().expect("a bounded image")];
            let out = reader.next_frame(&mut back).unwrap();
            assert_eq!(&back[..out.buffer_size()], raster.as_bytes(), "byte for byte");
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}
