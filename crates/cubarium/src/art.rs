//! Versioned output of the Godot art project. File I/O stays in the host.

use anyhow::{Context, Result, ensure};
use cubarium_render::Sprite;
use cubarium_surface::Vec2;
use std::path::Path;

pub const NAMES: [&str; 3] = ["lantern", "sail", "mossback"];
pub const STATES: [&str; 4] = ["rest", "move", "feed", "bud"];

pub struct Clip {
    pub frames: Vec<Sprite>,
    pub seconds: f64,
    pub looping: bool,
}

impl Clip {
    pub fn at(&self, seconds: f64) -> &Sprite {
        let phase = if !seconds.is_finite() {
            0.0
        } else if self.looping {
            seconds.rem_euclid(self.seconds) / self.seconds
        } else {
            (seconds / self.seconds).clamp(0.0, 1.0)
        };
        let index = if self.looping {
            (phase * self.frames.len() as f64).floor() as usize
        } else {
            (phase * (self.frames.len() - 1) as f64).round() as usize
        };
        &self.frames[index.min(self.frames.len() - 1)]
    }
}

pub struct ArtPack {
    /// Species-major, then rest/move/feed/bud. These are visual studies, not diets.
    pub clips: Vec<Clip>,
    pub habitat: Vec<Sprite>,
}

impl ArtPack {
    pub fn load(directory: &Path) -> Result<Self> {
        let path = directory.join("pack.json");
        let file = std::fs::File::open(&path)
            .with_context(|| format!("open {} (bake art/project.godot first)", path.display()))?;
        let meta: serde_json::Value = serde_json::from_reader(file)?;
        ensure!(
            meta["version"] == 1
                && meta["tile"] == 16
                && meta["frames"] == 8
                && meta["pivot"] == serde_json::json!([8, 8])
                && meta["facing"] == "+x",
            "unsupported art pack layout"
        );
        ensure!(
            meta["creatures"] == "creatures.png" && meta["habitat"] == "habitat.png",
            "unsupported atlas filenames"
        );
        ensure!(
            meta["habitat_names"] == serde_json::json!(["rosette", "fern", "lichen"]),
            "unsupported habitat order"
        );
        let (width, height, rgba) = read_rgba(&directory.join("creatures.png"))?;
        ensure!(
            (width, height) == (128, 192),
            "creature atlas must be 128×192 RGBA8"
        );
        let rows = meta["clips"].as_array().context("missing clips")?;
        ensure!(rows.len() == 12, "art pack must contain twelve clips");
        let mut clips = Vec::new();
        for (row, entry) in rows.iter().enumerate() {
            ensure!(
                entry["name"] == NAMES[row / 4]
                    && entry["state"] == STATES[row % 4]
                    && entry["row"].as_u64() == Some(row as u64),
                "unexpected clip order at row {row}"
            );
            let seconds = entry["seconds"].as_f64().context("clip needs seconds")?;
            ensure!(
                seconds.is_finite() && seconds > 0.0,
                "invalid clip duration"
            );
            let looping = entry["loop"].as_bool().context("clip needs loop flag")?;
            ensure!(
                looping == (row % 4 != 3),
                "bud must be nonlooping; rest/move/feed must loop"
            );
            let frames = (0..8)
                .map(|column| tile(&rgba, width, column * 16, row * 16))
                .collect::<Result<Vec<_>>>()?;
            clips.push(Clip {
                frames,
                seconds,
                looping,
            });
        }
        let (width, height, rgba) = read_rgba(&directory.join("habitat.png"))?;
        ensure!(
            (width, height) == (48, 16),
            "habitat atlas must be 48×16 RGBA8"
        );
        let habitat = (0..3)
            .map(|column| tile(&rgba, width, column * 16, 0))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { clips, habitat })
    }

    pub fn creature(&self, form: usize, state: usize, seconds: f64) -> &Sprite {
        self.clips[form * 4 + state].at(seconds)
    }
}

fn tile(bytes: &[u8], width: usize, x: usize, y: usize) -> Result<Sprite> {
    let mut rgba = Vec::with_capacity(16 * 16 * 4);
    for row in y..y + 16 {
        rgba.extend_from_slice(&bytes[(row * width + x) * 4..(row * width + x + 16) * 4]);
    }
    Sprite::from_rgba(16, 16, Vec2::new(8.0, 8.0), &rgba).map_err(anyhow::Error::msg)
}

fn read_rgba(path: &Path) -> Result<(usize, usize, Vec<u8>)> {
    let file = std::io::BufReader::new(
        std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?,
    );
    let decoder = png::Decoder::new(file);
    let mut reader = decoder.read_info()?;
    let info = reader.info();
    ensure!(
        info.width <= 512
            && info.height <= 512
            && info.color_type == png::ColorType::Rgba
            && info.bit_depth == png::BitDepth::Eight,
        "atlas must be small RGBA8 PNG"
    );
    let mut bytes = vec![
        0;
        reader
            .output_buffer_size()
            .context("atlas allocation overflow")?
    ];
    let info = reader.next_frame(&mut bytes)?;
    bytes.truncate(info.buffer_size());
    Ok((info.width as usize, info.height as usize, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_baked_art_loads_and_move_feed_and_bud_have_visible_animation() {
        let art =
            ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
                .unwrap();
        assert_eq!(art.clips.len(), 12);
        for (form, name) in NAMES.iter().enumerate() {
            for (state, state_name) in STATES.iter().enumerate().skip(1) {
                let clip = &art.clips[form * 4 + state];
                let images = clip
                    .frames
                    .iter()
                    .map(|s| format!("{s:?}"))
                    .collect::<std::collections::HashSet<_>>();
                assert!(images.len() > 1, "{name} {state_name} is frozen");
            }
        }
        let bud = &art.clips[3];
        assert!(std::ptr::eq(bud.at(bud.seconds * 4.0), &bud.frames[7]));
        let movement = &art.clips[1];
        assert!(std::ptr::eq(
            movement.at(movement.seconds),
            &movement.frames[0]
        ));
    }
}
