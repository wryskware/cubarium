//! The sprite pack, loaded into one RGBA8 atlas and a frame table.
//!
//! **No re-bake.** This reads `assets/atelier` exactly as `crates/cubarium/src/art.rs`
//! does — same `pack.json`, same PNGs, same 16×16 tiles on the `pivot` the pack
//! declares — and lays the five sheets out side by side in one texture. A pack tile
//! is authored at `S = 1`; the renderer draws it at an integer `scale` factor, so
//! `S = 2` is the same art with every source texel on a 2×2 block of raster pixels.
//!
//! The atlas is uploaded as `R8G8B8A8_SRGB`, so the sampler returns **straight**
//! linear RGB with a linear alpha, and the shader premultiplies. That is exactly
//! `Sprite::from_rgba`: `srgb_decode(c) · a`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};

/// One frame's rectangle in the atlas, in atlas texels, and how far it paints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameRect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    /// `cubarium_render::Sprite::extent`: the distance from the pivot to the furthest
    /// painted texel's far corner, plus the half-texel of bilinear support the CPU
    /// sprite carries. A radial growth mask reveals out to `extent + 0.5`
    /// (`art_present::draw_with_fruit`), so it has to be measured from the art rather
    /// than assumed to be half the tile.
    pub extent: f32,
    /// The frame's **opaque bounding box** in its own source texels: the half-open
    /// `[x0, x1) × [y0, y1)` of columns and rows that carry any texel with `α > 0`,
    /// `(0, 0, 0, 0)` for a frame that paints nothing at all.
    ///
    /// This is the fill lever. A pack tile is 16×16 and 82 % of the atlas is clear, so
    /// a quad built around the whole tile rasterises several fragments for every one
    /// that can paint; `sprite.vert` builds its quad around this box instead, plus the
    /// bend's reach and one texel of filter support on each side — which is what makes
    /// the tighter quad a *superset* of the fragments that ever painted.
    pub bbox: [u16; 4],
    /// How many texels of the frame carry `α > 0`. Reported, not drawn: it is the
    /// numerator of the fill profile in `sink/gpu.rs` — the art that can actually land
    /// in a quad, against the quad's own area.
    pub opaque: u32,
}

/// A baked animation: a contiguous run of frames plus the clock `pack.json` gives it.
///
/// [`Clip::pose`] is `cubarium::art::Clip::sample`'s rule, transcribed: the two
/// bracketing frames and the blend between them, wrapping for a looping clip and
/// clamping to the last frame otherwise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clip {
    /// Index of the clip's first frame in [`Atlas::frames`].
    pub first: u32,
    /// How many frames it has.
    pub count: u32,
    /// Its length in simulated seconds.
    pub seconds: f64,
    /// Whether it wraps.
    pub looping: bool,
}

impl Clip {
    /// The pose at a presentation time: `(frame_a, frame_b, mix)` as frame **indices**
    /// into [`Atlas::frames`].
    ///
    /// **Normative**, and identical to `art.rs`'s `Clip::sample`: with `n` frames and
    /// `u = (seconds mod length)/length · n` for a looping clip, the frames
    /// `floor(u)` and `(floor(u)+1) mod n` at `fract(u)`; a non-looping clip clamps
    /// `u` to `n − 1` and never wraps; one frame, a nonsense length or a non-finite
    /// time holds the first frame.
    pub fn pose(&self, seconds: f64) -> (u32, u32, f32) {
        let n = self.count;
        if n < 2 || !seconds.is_finite() || !(self.seconds.is_finite() && self.seconds > 0.0) {
            return (self.first, self.first, 0.0);
        }
        let u = if self.looping {
            seconds.rem_euclid(self.seconds) / self.seconds * f64::from(n)
        } else {
            (seconds / self.seconds * f64::from(n)).clamp(0.0, f64::from(n) - 1.0)
        };
        let i = (u.floor() as u32).min(n - 1);
        let j = if self.looping {
            (i + 1) % n
        } else {
            (i + 1).min(n - 1)
        };
        (self.first + i, self.first + j, (u - f64::from(i)) as f32)
    }
}

/// Which of a plant's clips: the three growth stages, the fruit accent, or the
/// authored `from → to` growth transition (pack v5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlantClip {
    Stage(u8),
    Fruit,
    Grow(u8, u8),
}

/// The whole pack, as one texture and a frame table.
pub struct Atlas {
    /// Atlas width in texels.
    pub width: u32,
    /// Atlas height in texels.
    pub height: u32,
    /// Straight sRGB RGBA8, row-major, `width · height · 4` bytes.
    pub rgba: Vec<u8>,
    /// Every frame of every clip, in load order.
    pub frames: Vec<FrameRect>,
    /// `(creature name, state)` → clip. States are `art::STATES` order.
    creatures: HashMap<(String, usize), Clip>,
    /// Creature rig names in atlas order.
    creature_names: Vec<String>,
    /// `(plant name, which)` → clip.
    plants: HashMap<(String, PlantClip), Clip>,
    /// A plant's band name, as `pack.json` declares it.
    plant_bands: HashMap<String, String>,
    /// `(tall plant name, part)` → clip; parts are `base`, `trunk`, `crown`.
    tall: HashMap<(String, String), Clip>,
    /// Band name → the band's 8×8 ground tile clip.
    ground: HashMap<String, Clip>,
    /// The single opaque white texel every solid-colour quad (rain) samples.
    solid: u32,
    /// The pivot `pack.json` declares for its 16×16 tiles.
    pub pivot: [u16; 2],
    /// The tile side, `pack.json`'s `tile`.
    pub tile: u16,
}

impl Atlas {
    /// Load `assets/atelier` (or any pack directory of the same version).
    pub fn load(directory: &Path) -> Result<Atlas> {
        let meta: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.join("pack.json"))
                .with_context(|| format!("read {}", directory.join("pack.json").display()))?,
        )
        .context("parse pack.json")?;
        let version = meta["version"].as_u64().unwrap_or(1);
        if version < 5 {
            bail!("pack version {version}: this renderer wants v5 (authored growth clips)");
        }
        let tile = u16::try_from(meta["tile"].as_u64().unwrap_or(16))?;
        let pivot = {
            let p = meta["pivot"]
                .as_array()
                .ok_or_else(|| anyhow!("pack.json has no pivot"))?;
            [
                u16::try_from(p[0].as_u64().unwrap_or(8))?,
                u16::try_from(p[1].as_u64().unwrap_or(8))?,
            ]
        };

        // Five sheets, stacked: the atlas is as wide as the widest and as tall as the
        // sum. No packing cleverness — 384 × 1,016 × 4 = 1.56 MB, uploaded once.
        let sheets = [
            "creatures",
            "plant_atlas",
            "tall_atlas",
            "ground_atlas",
            "habitat",
        ];
        let mut loaded = Vec::new();
        for key in sheets {
            let Some(name) = meta[key].as_str() else {
                continue;
            };
            let (w, h, bytes) = read_rgba(&directory.join(name))?;
            loaded.push((key, w, h, bytes));
        }
        let width = loaded.iter().map(|s| s.1).max().unwrap_or(1).max(1);
        // Two extra tile rows for the derived vine strips (see `derive_vine_strips`),
        // and one more for the solid white texel the rain quads sample.
        let height = loaded.iter().map(|s| s.2).sum::<u32>() + 2 * u32::from(tile) + 1;
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        let mut origin = HashMap::new();
        let mut y0 = 0u32;
        for (key, w, h, bytes) in &loaded {
            for row in 0..*h {
                let src = (row * w * 4) as usize;
                let dst = (((y0 + row) * width) * 4) as usize;
                rgba[dst..dst + (*w * 4) as usize]
                    .copy_from_slice(&bytes[src..src + (*w * 4) as usize]);
            }
            origin.insert(*key, (0u32, y0));
            y0 += h;
        }
        let derived_y = y0;
        y0 += 2 * u32::from(tile);
        // The solid texel: opaque white, so `colour · 1` is the instance's tone.
        let solid_y = y0;
        let s = ((solid_y * width) * 4) as usize;
        rgba[s..s + 4].copy_from_slice(&[255, 255, 255, 255]);

        let mut atlas = Atlas {
            width,
            height,
            rgba,
            frames: Vec::new(),
            creatures: HashMap::new(),
            creature_names: Vec::new(),
            plants: HashMap::new(),
            plant_bands: HashMap::new(),
            tall: HashMap::new(),
            ground: HashMap::new(),
            solid: 0,
            pivot,
            tile,
        };
        atlas.solid = atlas.push_frames(0, solid_y, 1, 1, 1);
        atlas.frames.truncate(atlas.solid as usize + 1);

        // Creatures: one row per (rig, state), `frames` samples across.
        let frames = meta["frames"].as_u64().unwrap_or(16) as u32;
        atlas.creature_names = meta["creature_names"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let (cx, cy) = *origin.get("creatures").unwrap_or(&(0, 0));
        for clip in meta["clips"].as_array().into_iter().flatten() {
            let (Some(name), Some(state), Some(row)) = (
                clip["name"].as_str(),
                clip["state"].as_str(),
                clip["row"].as_u64(),
            ) else {
                continue;
            };
            let Some(index) = STATES.iter().position(|s| *s == state) else {
                continue;
            };
            let first =
                atlas.push_frames(cx, cy + row as u32 * u32::from(tile), tile, frames, tile);
            atlas.creatures.insert(
                (name.to_owned(), index),
                Clip {
                    first,
                    count: frames,
                    seconds: clip["seconds"].as_f64().unwrap_or(1.0),
                    looping: clip["loop"].as_bool().unwrap_or(true),
                },
            );
        }

        // Plants: three stages, an optional fruit clip, and the v5 growth transitions.
        let (px, py) = *origin.get("plant_atlas").unwrap_or(&(0, 0));
        for row in meta["plants"].as_array().into_iter().flatten() {
            let (Some(name), Some(r)) = (row["name"].as_str(), row["row"].as_u64()) else {
                continue;
            };
            let count = row["frames"].as_u64().unwrap_or(u64::from(frames)) as u32;
            let first = atlas.push_frames(px, py + r as u32 * u32::from(tile), tile, count, tile);
            let clip = Clip {
                first,
                count,
                seconds: row["seconds"].as_f64().unwrap_or(3.0),
                looping: row["loop"].as_bool().unwrap_or(true),
            };
            if let Some(band) = row["band"].as_str() {
                atlas.plant_bands.insert(name.to_owned(), band.to_owned());
            }
            let which = match &row["stage"] {
                serde_json::Value::Number(n) => PlantClip::Stage(n.as_u64().unwrap_or(0) as u8),
                serde_json::Value::String(s) if s == "fruit" => PlantClip::Fruit,
                serde_json::Value::String(s) if s == "grow" => PlantClip::Grow(
                    row["from"].as_u64().unwrap_or(0) as u8,
                    row["to"].as_u64().unwrap_or(1) as u8,
                ),
                _ => continue,
            };
            atlas.plants.insert((name.to_owned(), which), clip);
        }

        // Tall plants: base, trunk and crown rows.
        let (tx, ty) = *origin.get("tall_atlas").unwrap_or(&(0, 0));
        for row in meta["tall"].as_array().into_iter().flatten() {
            let (Some(name), Some(part), Some(r)) = (
                row["name"].as_str(),
                row["part"].as_str(),
                row["row"].as_u64(),
            ) else {
                continue;
            };
            let count = row["frames"].as_u64().unwrap_or(u64::from(frames)) as u32;
            let first = atlas.push_frames(tx, ty + r as u32 * u32::from(tile), tile, count, tile);
            atlas.tall.insert(
                (name.to_owned(), part.to_owned()),
                Clip {
                    first,
                    count,
                    seconds: row["seconds"].as_f64().unwrap_or(3.0),
                    looping: true,
                },
            );
        }

        // Ground cover: 8×8 tiles, four breathing frames per band.
        let ground_tile = meta["ground_tile"].as_u64().unwrap_or(8) as u16;
        let (gx, gy) = *origin.get("ground_atlas").unwrap_or(&(0, 0));
        for row in meta["ground"].as_array().into_iter().flatten() {
            let (Some(band), Some(r)) = (row["band"].as_str(), row["row"].as_u64()) else {
                continue;
            };
            let count = row["frames"].as_u64().unwrap_or(4) as u32;
            let first = atlas.push_frames(
                gx,
                gy + r as u32 * u32::from(ground_tile),
                ground_tile,
                count,
                ground_tile,
            );
            atlas.ground.insert(
                band.to_owned(),
                Clip {
                    first,
                    count,
                    seconds: row["seconds"].as_f64().unwrap_or(6.0),
                    looping: true,
                },
            );
        }

        // The derived vine strips, `art.rs::derive_vine_strips` transcribed onto the
        // atlas bytes: a four-row-periodic vine trunk paints the column through a tile
        // whose rows 0 and 15 are cleared, and its top is a separate clip keeping only
        // rows 4..=7. Both inherit the original's clock. Without them a vine draws its
        // raw tile and paints the rows its neighbours already own — the one place the
        // GPU could not simply read the pack.
        for row in meta["tall"].as_array().into_iter().flatten() {
            let (Some(name), Some(part)) = (row["name"].as_str(), row["part"].as_str()) else {
                continue;
            };
            if part != "trunk" || row["vine_strips"].as_str() != Some(VINE_STRIPS_V1) {
                continue;
            }
            let Some(source) = atlas.tall(name, "trunk") else {
                continue;
            };
            for (i, (clear, keep)) in [
                (vec![0usize, usize::from(tile) - 1], None),
                (Vec::new(), Some(4..8)),
            ]
            .into_iter()
            .enumerate()
            {
                let y = derived_y + i as u32 * u32::from(tile);
                let first = atlas.copy_derived(source, 0, y, tile, &clear, keep);
                atlas.tall.insert(
                    (
                        name.to_owned(),
                        if i == 0 { "trunk_strip" } else { "endpoint" }.to_owned(),
                    ),
                    Clip {
                        first,
                        count: source.count,
                        seconds: source.seconds,
                        looping: true,
                    },
                );
            }
        }

        if atlas.plants.is_empty() || atlas.creatures.is_empty() {
            bail!(
                "pack at {} carries no plants or no creatures",
                directory.display()
            );
        }
        Ok(atlas)
    }

    /// Append `count` frames of `w × h` laid left to right from `(x, y)`, returning
    /// the first one's index. Each frame's extent, opaque bounding box and opaque texel
    /// count are measured from its own alpha, once, here — `copy_derived` calls this
    /// after it has written its bytes, so a derived vine strip is measured from what it
    /// actually paints rather than from the clip it was cut out of.
    fn push_frames(&mut self, x: u32, y: u32, w: u16, count: u32, h: u16) -> u32 {
        let first = self.frames.len() as u32;
        for i in 0..count {
            let rect = FrameRect {
                x: (x + i * u32::from(w)) as u16,
                y: y as u16,
                w,
                h,
                extent: 0.0,
                bbox: [0; 4],
                opaque: 0,
            };
            let (extent, bbox, opaque) = self.measure(rect);
            self.frames.push(FrameRect {
                extent,
                bbox,
                opaque,
                ..rect
            });
        }
        first
    }

    /// One pass over a frame's alpha: its extent, its opaque bounding box and how many
    /// texels it paints.
    ///
    /// The extent is `Sprite::from_rgba`'s: over every texel with `α > 0`, the largest
    /// `hypot(x + 0.5 − pivot.x, y + 0.5 − pivot.y) + TEXEL_SUPPORT`, with the pivot at
    /// the tile's centre and `TEXEL_SUPPORT = 0.5 · √2` as `cubarium_render::sprite`
    /// defines it. The box is the half-open span of painted columns and rows.
    fn measure(&self, rect: FrameRect) -> (f32, [u16; 4], u32) {
        const TEXEL_SUPPORT: f64 = std::f64::consts::SQRT_2 / 2.0;
        let (px, py) = (f64::from(rect.w) / 2.0, f64::from(rect.h) / 2.0);
        let mut extent = 0.0f64;
        let (mut x0, mut y0, mut x1, mut y1) = (u32::from(rect.w), u32::from(rect.h), 0u32, 0u32);
        let mut opaque = 0u32;
        for ty in 0..u32::from(rect.h) {
            for tx in 0..u32::from(rect.w) {
                let i = (((u32::from(rect.y) + ty) * self.width + u32::from(rect.x) + tx) * 4 + 3)
                    as usize;
                if self.rgba.get(i).copied().unwrap_or(0) == 0 {
                    continue;
                }
                opaque += 1;
                x0 = x0.min(tx);
                y0 = y0.min(ty);
                x1 = x1.max(tx + 1);
                y1 = y1.max(ty + 1);
                let dx = f64::from(tx) + 0.5 - px;
                let dy = f64::from(ty) + 0.5 - py;
                extent = extent.max(dx.hypot(dy) + TEXEL_SUPPORT);
            }
        }
        let bbox = if opaque == 0 {
            [0; 4]
        } else {
            [x0 as u16, y0 as u16, x1 as u16, y1 as u16]
        };
        (extent as f32, bbox, opaque)
    }

    /// Copy a clip's frames to `(x, y)`, clearing whole rows or keeping only a range of
    /// them, and register the copies as new frames. Returns the first one's index.
    fn copy_derived(
        &mut self,
        source: Clip,
        x: u32,
        y: u32,
        tile: u16,
        clear: &[usize],
        keep: Option<std::ops::Range<usize>>,
    ) -> u32 {
        for i in 0..source.count {
            let from = self.frames[(source.first + i) as usize];
            let to_x = x + i * u32::from(tile);
            for row in 0..u32::from(tile) {
                let blank = clear.contains(&(row as usize))
                    || keep.as_ref().is_some_and(|k| !k.contains(&(row as usize)));
                for col in 0..u32::from(tile) {
                    let src = (((u32::from(from.y) + row) * self.width + u32::from(from.x) + col)
                        * 4) as usize;
                    let dst = (((y + row) * self.width + to_x + col) * 4) as usize;
                    let texel = if blank {
                        [0u8; 4]
                    } else {
                        [
                            self.rgba[src],
                            self.rgba[src + 1],
                            self.rgba[src + 2],
                            self.rgba[src + 3],
                        ]
                    };
                    self.rgba[dst..dst + 4].copy_from_slice(&texel);
                }
            }
        }
        self.push_frames(x, y, tile, source.count, tile)
    }

    /// The rect of a frame index.
    pub fn rect(&self, frame: u32) -> FrameRect {
        self.frames[frame as usize]
    }

    /// The one opaque white texel: the frame a solid-colour quad (a rain streak)
    /// samples, so rain costs no second pipeline.
    pub fn solid(&self) -> u32 {
        self.solid
    }

    /// A creature rig's clip for a state index (`0..4`, `art::STATES` order).
    pub fn creature(&self, name: &str, state: usize) -> Option<Clip> {
        self.creatures.get(&(name.to_owned(), state)).copied()
    }

    /// The rig names, in the pack's own order.
    pub fn creature_names(&self) -> &[String] {
        &self.creature_names
    }

    /// One of a plant's clips.
    pub fn plant(&self, name: &str, which: PlantClip) -> Option<Clip> {
        self.plants.get(&(name.to_owned(), which)).copied()
    }

    /// The band `pack.json` files a plant under.
    pub fn plant_band(&self, name: &str) -> Option<&str> {
        self.plant_bands.get(name).map(String::as_str)
    }

    /// One part of a tall plant: `base`, `trunk` or `crown`.
    pub fn tall(&self, name: &str, part: &str) -> Option<Clip> {
        self.tall.get(&(name.to_owned(), part.to_owned())).copied()
    }

    /// A band's ground-cover tile.
    pub fn ground(&self, band: &str) -> Option<Clip> {
        self.ground.get(band).copied()
    }
}

/// The four clip rows every creature carries, in atlas order — `art::STATES`.
pub const STATES: [&str; 4] = ["rest", "move", "feed", "bud"];
/// `art::VINE_STRIPS_V1`: the selector that opts a vine trunk into the derived strips.
pub const VINE_STRIPS_V1: &str = "period4_endpoint_v1";

fn read_rgba(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?,
    ));
    let mut reader = decoder
        .read_info()
        .with_context(|| format!("{}: png header", path.display()))?;
    let mut buffer = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader
        .next_frame(&mut buffer)
        .with_context(|| format!("{}: png data", path.display()))?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        bail!(
            "{}: expected 8-bit RGBA, got {:?}/{:?}",
            path.display(),
            info.color_type,
            info.bit_depth
        );
    }
    buffer.truncate(info.buffer_size());
    Ok((info.width, info.height, buffer))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atelier() -> Atlas {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier");
        Atlas::load(&root).expect("load assets/atelier")
    }

    #[test]
    fn the_shipped_pack_loads_with_every_clip_the_presenter_draws() {
        let a = atelier();
        assert_eq!(a.tile, 16);
        assert_eq!(a.pivot, [8, 8]);
        assert_eq!(a.creature_names().len(), 4);
        for name in a.creature_names() {
            for state in 0..STATES.len() {
                assert!(a.creature(name, state).is_some(), "{name}/{state}");
            }
        }
        for name in [
            "glowcap",
            "rootveil",
            "lanternstalk",
            "tendrilfan",
            "umbrellafrond",
            "bloomcrown",
            "reedspire",
        ] {
            for stage in 0..3 {
                assert!(
                    a.plant(name, PlantClip::Stage(stage)).is_some(),
                    "{name} stage {stage}"
                );
            }
            assert!(
                a.plant(name, PlantClip::Grow(0, 1)).is_some(),
                "{name} grow 0->1"
            );
            assert!(
                a.plant(name, PlantClip::Grow(1, 2)).is_some(),
                "{name} grow 1->2"
            );
        }
        for band in ["soil", "foliage", "canopy"] {
            assert!(a.ground(band).is_some(), "ground {band}");
        }
        for part in ["base", "trunk", "crown"] {
            assert!(a.tall("spiretree", part).is_some(), "spiretree {part}");
        }
        assert!(a.tall("vinecoil", "trunk").is_some());
    }

    #[test]
    fn the_vine_strips_are_derived_from_the_raw_trunk() {
        let a = atelier();
        let raw = a
            .tall("vinecoil", "trunk")
            .expect("the pack carries a vine");
        let strip = a
            .tall("vinecoil", "trunk_strip")
            .expect("the strip is derived");
        let end = a
            .tall("vinecoil", "endpoint")
            .expect("the endpoint is derived");
        assert_eq!((strip.count, strip.seconds), (raw.count, raw.seconds));
        assert_eq!((end.count, end.seconds), (raw.count, raw.seconds));
        let alpha = |clip: Clip, row: u16, col: u16| {
            let r = a.rect(clip.first);
            let i =
                (((u32::from(r.y) + u32::from(row)) * a.width + u32::from(r.x) + u32::from(col))
                    * 4
                    + 3) as usize;
            a.rgba[i]
        };
        // The strip clears rows 0 and 15 and keeps the rest; the endpoint keeps 4..=7.
        for col in 0..16 {
            assert_eq!(alpha(strip, 0, col), 0, "strip row 0 col {col}");
            assert_eq!(alpha(strip, 15, col), 0, "strip row 15 col {col}");
            for row in [0u16, 1, 2, 3, 8, 15] {
                assert_eq!(alpha(end, row, col), 0, "endpoint row {row} col {col}");
            }
        }
        let painted = (0..16).any(|col| alpha(end, 5, col) > 0);
        assert!(painted, "the endpoint kept nothing at all");
    }

    #[test]
    fn the_solid_texel_is_opaque_white_so_a_rain_quad_paints_its_own_colour() {
        let a = atelier();
        let r = a.rect(a.solid());
        let i = ((u32::from(r.y) * a.width + u32::from(r.x)) * 4) as usize;
        assert_eq!(&a.rgba[i..i + 4], &[255, 255, 255, 255]);
    }

    #[test]
    fn a_looping_clip_wraps_last_into_first_and_a_growth_clip_clamps() {
        let a = atelier();
        let stage = a.plant("lanternstalk", PlantClip::Stage(1)).unwrap();
        assert!(stage.looping);
        let (i, j, m) = stage.pose(stage.seconds - 1e-9);
        assert_eq!(i, stage.first + stage.count - 1);
        assert_eq!(
            j, stage.first,
            "the last frame must blend back into the first"
        );
        assert!(m > 0.99, "{m}");
        let grow = a.plant("lanternstalk", PlantClip::Grow(0, 1)).unwrap();
        assert!(!grow.looping);
        let (i, j, _) = grow.pose(grow.seconds * 4.0);
        assert_eq!(
            (i, j),
            (grow.first + grow.count - 1, grow.first + grow.count - 1)
        );
    }
}
