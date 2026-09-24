//! **Face textures** for the GPU voxel renderer, at every level of detail
//! (`design/handoffs/voxel-textures-lod-2026-09-23.md`).
//!
//! The CPU presenter never draws these: it is the PNG diagnostics path and keeps its
//! solid faces.
//!
//! # Files
//!
//! ```text
//! assets/voxel-textures/
//!   masters/<stem>-<variant>.png      48 x 48, authored (interim: scripts/voxel-textures/)
//!   lod/<px>/<stem>-<variant>.png     derived by `examples/voxel_texture_levels.rs`
//!   override/<px>/<stem>-<variant>.png   hand-fixed levels; these win
//! ```
//!
//! `<stem>` is one of [`TEXTURE_SLOTS`] (`rock-side`, `soil-top`, `leaf-side`, …) and
//! `<variant>` is `0..TEX_VARIANTS`.
//!
//! The latticevine tiles live one directory down, `masters/vine/<stem>.png` (and
//! `lod/<px>/vine/`, `override/<px>/vine/`), one file per tile with no variants: the stems
//! are [`vine_tiles`]'s (`crate::voxel::vine`). They are direct colour with a cutout
//! alpha, derived the same way.
//!
//! # Levels
//!
//! A level is `px_per_voxel` itself: the renderer shows one texel per screen pixel, never
//! filtered. A side face at level `s` is `s × s`; a **top face is `s × rise`**, because
//! the elevated camera foreshortens it (`project.rs`, `rise = round(s · tan tilt)`). The
//! top master is authored square — the top of the voxel seen from straight above, row 0
//! at the back edge — and each level squeezes it vertically by area, so the shader
//! addresses a top texel exactly as it addresses a side texel and nothing is smeared.
//!
//! 48 divides by 4, 6, 8, 12, 16 and 24, so a side face at those levels is an exact
//! block average. A top face is not (48 / rise is fractional), nor is a level `auto` picks
//! outside that list; those are the same area average with fractional coverage.
//!
//! Every derived texel is then **snapped to the nearest master colour under its
//! footprint**, so a level never invents a shade, and its alpha is cut to 0 or 255 at one
//! half, so a cutout stays a cutout. At load, a level is taken from `override/`, else from `lod/` if it has
//! this projection's size, else derived from the master in memory — so any `s` draws, and
//! the files are there to be looked at and fixed.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
pub use cubarium_gpu::voxel::{TEX_VARIANTS, TEXTURE_SLOTS, TexFace, VoxelTextures};

/// The side of a master face image.
pub const MASTER_PX: u32 = 48;

/// The levels `voxel_texture_levels` writes: every divisor of [`MASTER_PX`] from 4 up.
pub const STANDARD_LEVELS: [u32; 6] = [4, 6, 8, 12, 16, 24];

/// An RGBA8 image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
}

/// Read a PNG of any colour type and depth as RGBA8.
pub fn read_png(path: &Path) -> Result<Image> {
    let file = std::io::BufReader::new(
        std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?,
    );
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().context("PNG too large")?];
    let info = reader
        .next_frame(&mut buf)
        .with_context(|| format!("decode {}", path.display()))?;
    let (w, h) = (info.width, info.height);
    let n = (w * h) as usize;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..n * 4].to_vec(),
        png::ColorType::Rgb => buf[..n * 3]
            .chunks_exact(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf[..n * 2]
            .chunks_exact(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => buf[..n].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => bail!("{}: palette not expanded", path.display()),
    };
    Ok(Image { w, h, rgba })
}

/// Write an RGBA8 PNG, creating its directory.
pub fn write_png(path: &Path, img: &Image) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    let file = std::fs::File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.w, img.h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&img.rgba)?;
    Ok(())
}

/// The size of a face of `face` at level `s` with this `rise`.
pub fn face_size(face: TexFace, s: u32, rise: u32) -> (u32, u32) {
    match face {
        TexFace::Side => (s, s),
        TexFace::Top => (s, rise),
    }
}

/// One level of a master: area-average it down to `w × h` (fractional coverage where the
/// ratio is not whole), cut the alpha at one half, and snap every opaque texel to the
/// nearest of the master's colours **under that texel's footprint**.
///
/// Snapping to the whole master's palette instead invents colours at a boundary: blue
/// turf over violet soil averages to something nearest the rock pebbles' blue-violet, and
/// a band of rock appears under every fringe. A colour that is under the footprint is
/// always one the artist put there.
pub fn derive(master: &Image, w: u32, h: u32) -> Image {
    let (sx, sy) = (
        f64::from(master.w) / f64::from(w),
        f64::from(master.h) / f64::from(h),
    );
    // The source span `[a, b)` of destination cell `i` along one axis, as whole source
    // indices and how much of each one the cell covers.
    let span = |i: u32, k: f64, n: u32| -> Vec<(u32, f64)> {
        let (a, b) = (f64::from(i) * k, f64::from(i + 1) * k);
        let mut out = Vec::new();
        let mut j = a.floor() as u32;
        while f64::from(j) < b && j < n {
            let cover = (f64::from(j + 1)).min(b) - (f64::from(j)).max(a);
            if cover > 1e-9 {
                out.push((j, cover));
            }
            j += 1;
        }
        out
    };
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let ys = span(y, sy, master.h);
        for x in 0..w {
            let xs = span(x, sx, master.w);
            let (mut c, mut a, mut area) = ([0.0f64; 3], 0.0f64, 0.0f64);
            let mut palette: Vec<[u8; 3]> = Vec::new();
            for &(yy, wy) in &ys {
                for &(xx, wx) in &xs {
                    let p = master.px(xx, yy);
                    if p[3] >= 128 && !palette.contains(&[p[0], p[1], p[2]]) {
                        palette.push([p[0], p[1], p[2]]);
                    }
                    let cover = wx * wy;
                    let alpha = f64::from(p[3]) / 255.0;
                    for k in 0..3 {
                        c[k] += f64::from(p[k]) * cover * alpha;
                    }
                    a += cover * alpha;
                    area += cover;
                }
            }
            if area <= 0.0 || a / area < 0.5 || palette.is_empty() {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let mean = [c[0] / a, c[1] / a, c[2] / a];
            let snap = palette
                .iter()
                .min_by(|p, q| {
                    let d = |p: &[u8; 3]| {
                        (0..3)
                            .map(|k| (f64::from(p[k]) - mean[k]).powi(2))
                            .sum::<f64>()
                    };
                    d(p).total_cmp(&d(q))
                })
                .expect("a palette with a colour in it");
            rgba.extend_from_slice(&[snap[0], snap[1], snap[2], 255]);
        }
    }
    Image { w, h, rgba }
}

/// Every latticevine tile the renderer's vine atlas holds, as `(row, column, stem,
/// fallback)`: rows [`super::vine::tile_row`] (plain, climbing root, hanging root; bare,
/// thin, full), columns [`super::vine::tile_column`] (mask, and the exits inside it), and
/// row [`super::vine::ACCENT_ROW`] the three accents. `fallback` is the older stem with no
/// exits digit, drawn while a per-exit file is missing.
pub fn vine_tiles() -> Vec<(u32, u32, String, Option<String>)> {
    use super::vine::{
        ACCENT_ROW, Accent, Density, TileSet, accent_name, tile_column, tile_name, tile_row,
    };
    let mut out = Vec::new();
    for set in TileSet::ALL {
        for d in Density::ALL {
            for m in 0..16u8 {
                for e in (0..16u8).filter(|e| e & !m == 0) {
                    out.push((
                        tile_row(set, d),
                        tile_column(m, e),
                        tile_name(set, d, m, Some(e)),
                        Some(tile_name(set, d, m, None)),
                    ));
                }
            }
        }
    }
    for (i, a) in Accent::DRAWN.into_iter().enumerate() {
        out.push((ACCENT_ROW, i as u32, accent_name(a), None));
    }
    out
}

/// A hanging-root tile with no file of its own: the climbing tile of the flipped mask,
/// upside down.
fn hanging_fallback(stem: &str) -> Option<String> {
    let body = stem.strip_suffix("-hang")?;
    let (head, mask) = body.rsplit_once('-')?;
    let m = u8::from_str_radix(mask, 16).ok()?;
    Some(format!("{head}-{:x}", super::vine::flip_mask(m)))
}

fn flip_rows(img: &Image) -> Image {
    let row = (img.w * 4) as usize;
    let rgba = img.rgba.chunks_exact(row).rev().flatten().copied().collect();
    Image { rgba, ..*img }
}

/// Where each face of a loaded atlas came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Provenance {
    pub overrides: usize,
    pub levels: usize,
    pub derived: usize,
    /// Variants with no file at all, filled from the slot's other variants.
    pub repeated: usize,
    /// Slots with nothing, drawn untextured.
    pub absent: usize,
    /// Vine tiles loaded, and vine tiles with no file at all (drawn as holes).
    pub vine_tiles: usize,
    pub vine_missing: usize,
}

fn file(root: &Path, dir: &str, s: Option<u32>, stem: &str, v: u32) -> PathBuf {
    let mut p = root.join(dir);
    if let Some(s) = s {
        p = p.join(s.to_string());
    }
    p.join(format!("{stem}-{v}.png"))
}

/// Load every face at level `s` (with this `rise`) from `root`. A missing directory is
/// not an error: every slot is absent and the picture is the untextured one.
pub fn load(root: &Path, s: u32, rise: u32) -> Result<(VoxelTextures, Provenance)> {
    let mut atlas = VoxelTextures::empty(s, rise);
    let mut prov = Provenance::default();
    for (slot, &(stem, face)) in TEXTURE_SLOTS.iter().enumerate() {
        let (w, h) = face_size(face, s, rise);
        let mut found: Vec<Option<Image>> = Vec::new();
        for v in 0..TEX_VARIANTS {
            let exact = |path: PathBuf| -> Result<Option<Image>> {
                if !path.exists() {
                    return Ok(None);
                }
                let img = read_png(&path)?;
                Ok(((img.w, img.h) == (w, h)).then_some(img))
            };
            let over = file(root, "override", Some(s), stem, v);
            let img = if let Some(img) = exact(over.clone())? {
                prov.overrides += 1;
                Some(img)
            } else {
                if over.exists() {
                    eprintln!(
                        "cubarium voxel: {} is not {w}x{h}; ignoring the override",
                        over.display()
                    );
                }
                if let Some(img) = exact(file(root, "lod", Some(s), stem, v))? {
                    prov.levels += 1;
                    Some(img)
                } else {
                    let master = file(root, "masters", None, stem, v);
                    if master.exists() {
                        prov.derived += 1;
                        Some(derive(&read_png(&master)?, w, h))
                    } else {
                        None
                    }
                }
            };
            found.push(img);
        }
        let have: Vec<Image> = found.iter().flatten().cloned().collect();
        if have.is_empty() {
            prov.absent += 1;
            continue;
        }
        for (v, img) in found.iter().enumerate() {
            let img = match img {
                Some(img) => img,
                None => {
                    prov.repeated += 1;
                    &have[v % have.len()]
                }
            };
            atlas.put(slot, v as u32, &img.rgba)?;
        }
    }
    let vine_level = |stem: &str| -> Result<Option<Image>> {
        let exact = |path: PathBuf| -> Result<Option<Image>> {
            if !path.exists() {
                return Ok(None);
            }
            let img = read_png(&path)?;
            Ok(((img.w, img.h) == (s, s)).then_some(img))
        };
        if let Some(img) = exact(vine_file(root, "override", Some(s), stem))? {
            return Ok(Some(img));
        }
        if let Some(img) = exact(vine_file(root, "lod", Some(s), stem))? {
            return Ok(Some(img));
        }
        let master = vine_file(root, "masters", None, stem);
        Ok(match master.exists() {
            true => Some(derive(&read_png(&master)?, s, s)),
            false => None,
        })
    };
    for (row, col, stem, legacy) in vine_tiles() {
        let mut img = vine_level(&stem)?;
        if img.is_none()
            && let Some(legacy) = &legacy
        {
            img = match vine_level(legacy)? {
                Some(img) => Some(img),
                None => match hanging_fallback(legacy) {
                    Some(climb) => vine_level(&climb)?.map(|img| flip_rows(&img)),
                    None => None,
                },
            };
        }
        match img {
            Some(img) => {
                atlas.put_vine(row, col, &img.rgba)?;
                prov.vine_tiles += 1;
            }
            None => prov.vine_missing += 1,
        }
    }
    Ok((atlas, prov))
}

fn vine_file(root: &Path, dir: &str, s: Option<u32>, stem: &str) -> PathBuf {
    let mut p = root.join(dir);
    if let Some(s) = s {
        p = p.join(s.to_string());
    }
    p.join("vine").join(format!("{stem}.png"))
}

/// Write `lod/<s>/` for every level in `levels` from the masters under `root`, at this
/// tilt. Returns the number of files written.
pub fn write_levels(root: &Path, levels: &[u32], tilt_degrees: f64) -> Result<usize> {
    let mut n = 0;
    for &s in levels {
        let rise = super::project::rise_for(tilt_degrees, s);
        for &(stem, face) in TEXTURE_SLOTS.iter() {
            let (w, h) = face_size(face, s, rise);
            for v in 0..TEX_VARIANTS {
                let master = file(root, "masters", None, stem, v);
                if !master.exists() {
                    continue;
                }
                let img = derive(&read_png(&master)?, w, h);
                write_png(&file(root, "lod", Some(s), stem, v), &img)?;
                n += 1;
            }
        }
        let mut stems: Vec<String> = vine_tiles()
            .into_iter()
            .flat_map(|(_, _, stem, legacy)| std::iter::once(stem).chain(legacy))
            .collect();
        stems.sort_unstable();
        stems.dedup();
        for stem in stems {
            let master = vine_file(root, "masters", None, &stem);
            if !master.exists() {
                continue;
            }
            let img = derive(&read_png(&master)?, s, s);
            write_png(&vine_file(root, "lod", Some(s), &stem), &img)?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Image {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        Image { w, h, rgba }
    }

    /// An exact divisor is a block average snapped to the master's colours: a 48 px
    /// checker of 12 px squares at level 4 is the same checker, one texel a square.
    #[test]
    fn a_level_is_the_block_average_snapped_to_the_masters_colours() {
        let (a, b) = ([200, 10, 10, 255], [10, 10, 200, 255]);
        let m = solid(48, 48, |x, y| if (x / 12 + y / 12) % 2 == 0 { a } else { b });
        let l = derive(&m, 4, 4);
        assert_eq!(l.px(0, 0), a);
        assert_eq!(l.px(1, 0), b);
        assert_eq!(l.px(3, 3), a);
        // A texel straddling both colours still comes out as one of them, never a blend.
        let fine = solid(48, 48, |x, _| if x % 2 == 0 { a } else { b });
        let l = derive(&fine, 6, 6);
        assert!(l.rgba.chunks(4).all(|p| p == a || p == b));
    }

    /// A top face squeezes the square master vertically: the back half of the master is
    /// the back half of the level, whatever `rise` is.
    #[test]
    fn a_top_face_keeps_its_back_and_front_rows_apart() {
        let (back, front) = ([90, 90, 90, 255], [30, 200, 30, 255]);
        let m = solid(48, 48, |_, y| if y < 24 { back } else { front });
        for (s, rise) in [(4, 2), (6, 3), (8, 5), (12, 7), (24, 14)] {
            let l = derive(&m, s, rise);
            assert_eq!((l.w, l.h), (s, rise));
            assert_eq!(l.px(0, 0), back, "s {s}");
            assert_eq!(l.px(s - 1, rise - 1), front, "s {s}");
        }
    }

    /// A cutout stays a cutout: a texel is a hole iff less than half of what it covers
    /// is opaque.
    #[test]
    fn cutout_alpha_is_cut_at_one_half() {
        let leaf = [40, 120, 200, 255];
        let m = solid(48, 48, |x, _| if x < 32 { leaf } else { [0, 0, 0, 0] });
        let l = derive(&m, 4, 4);
        assert_eq!(l.px(0, 0), leaf);
        assert_eq!(l.px(2, 0), leaf, "8 of its 12 columns opaque");
        assert_eq!(l.px(3, 0)[3], 0);
        let m = solid(48, 48, |x, _| if x < 28 { leaf } else { [0, 0, 0, 0] });
        assert_eq!(derive(&m, 4, 4).px(2, 0)[3], 0, "4 of its 12 columns opaque");
    }

    /// The vine atlas holds each set and density by `(mask, exits)` — 81 exit patterns
    /// inside the 16 masks — and the three accents. A per-exit file wins; without one the
    /// older mask-only file is drawn in every exits column of its mask.
    #[test]
    fn a_vine_tile_lands_in_its_row_and_mask_column() {
        let tiles = vine_tiles();
        assert_eq!(tiles.len(), 9 * 81 + 3);
        let stems: Vec<&str> = tiles.iter().map(|t| t.2.as_str()).collect();
        assert!(stems.contains(&"vine-root-thin-a-8"));
        assert!(stems.contains(&"vine-root-full-c-4-hang"));
        assert!(!stems.contains(&"vine-full-1-2"), "an exit outside the mask is never drawn");
        assert!(tiles.contains(&(9, 1, "vine-accent-flower".to_string(), None)));
        assert_eq!(hanging_fallback("vine-root-full-1-hang").unwrap(), "vine-root-full-4");
        let root = std::env::temp_dir().join(format!("cubarium-vine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let green = solid(48, 48, |_, _| [0, 200, 0, 255]);
        let red = solid(48, 48, |_, _| [200, 0, 0, 255]);
        write_png(&root.join("masters/vine/vine-full-5.png"), &green).unwrap();
        write_png(&root.join("masters/vine/vine-full-5-4.png"), &red).unwrap();
        let (atlas, prov) = load(&root, 4, 2).unwrap();
        assert!(atlas.vine_on);
        // Mask 5 has four exit patterns: one per-exit file, three from the older file.
        assert_eq!(prov.vine_tiles, 4);
        let aw = VoxelTextures::vine_size(4).0;
        let px = |x: u32, y: u32| {
            let i = ((y * aw + x) * 4) as usize;
            atlas.vine_rgba[i..i + 4].to_vec()
        };
        assert_eq!(px((5 * 16 + 4) * 4, 2 * 4), [200, 0, 0, 255], "the per-exit file");
        assert_eq!(px((5 * 16 + 1) * 4, 2 * 4), [0, 200, 0, 255], "the older file");
        assert_eq!(px((4 * 16) * 4, 2 * 4)[3], 0, "mask 4 has no file");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// An override of the right size wins over the derived level; one of the wrong size
    /// is ignored; a missing variant repeats one that exists; a slot with no files is
    /// absent from the mask.
    #[test]
    fn an_override_wins_and_missing_variants_repeat() {
        let root = std::env::temp_dir().join(format!("cubarium-tex-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let grey = solid(48, 48, |_, _| [100, 100, 100, 255]);
        write_png(&root.join("masters/rock-side-0.png"), &grey).unwrap();
        write_png(&root.join("masters/rock-side-1.png"), &grey).unwrap();
        let red = solid(6, 6, |_, _| [255, 0, 0, 255]);
        write_png(&root.join("override/6/rock-side-1.png"), &red).unwrap();
        write_png(&root.join("override/6/rock-side-0.png"), &solid(5, 5, |_, _| [0; 4])).unwrap();

        let (atlas, prov) = load(&root, 6, 3).unwrap();
        let rock = TEXTURE_SLOTS.iter().position(|s| s.0 == "rock-side").unwrap();
        assert_eq!(atlas.mask(), 1 << rock);
        assert_eq!(prov.overrides, 1);
        assert_eq!(prov.derived, 1);
        assert_eq!(prov.repeated, 2);
        let aw = VoxelTextures::size(6).0;
        let at = |v: u32| {
            let i = (((rock as u32 * 6) * aw + v * 6) * 4) as usize;
            [atlas.rgba[i], atlas.rgba[i + 1], atlas.rgba[i + 2]]
        };
        assert_eq!(at(0), [100, 100, 100]);
        assert_eq!(at(1), [255, 0, 0]);
        assert_eq!(at(2), [100, 100, 100], "variant 2 repeats variant 0");
        assert_eq!(at(3), [255, 0, 0], "variant 3 repeats variant 1");
        assert!(!atlas.vine_on, "no vine tiles, no vine layer");
        let _ = std::fs::remove_dir_all(&root);
    }
}
