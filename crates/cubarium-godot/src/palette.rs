//! **The view's colours**: `game/world/data/palette.json`, exported from the Rust
//! presenter by `crates/cubarium/examples/godot_palette.rs`. Nothing here chooses a
//! colour; it decodes the file's sRGB hex to the linear light the meshes carry.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Linear-light RGB.
pub type Linear = [f32; 3];

/// The strata palette of the presenter (`voxel::present`), as the file holds it.
#[derive(Clone, Debug, Deserialize)]
pub struct TerrainHex {
    pub bedrock: String,
    pub rock: String,
    pub soil: String,
    pub water_deep: String,
    pub water_surface: String,
    pub haze: String,
    /// How far fully saturated soil darkens toward deep water.
    pub wet: f32,
    /// How far the back wall fades toward the haze colour.
    pub haze_amount: f32,
    /// The water body's alpha.
    pub water_alpha: f32,
    /// A surface blends this much harder than the body.
    pub skin_alpha_gain: f32,
    /// A surface's top face blends this much of the skin alpha.
    pub water_top_alpha: f32,
}

/// The file.
#[derive(Clone, Debug, Deserialize)]
pub struct PaletteFile {
    pub terrain: TerrainHex,
    /// The model palette's material names, in index order.
    pub model_palette: Vec<String>,
    /// Per species: `"tag:material:band" → #rrggbb`.
    #[serde(default)]
    pub plants: BTreeMap<String, BTreeMap<String, String>>,
    /// Per animal model: `"material:band" → #rrggbb`.
    #[serde(default)]
    pub animals: BTreeMap<String, BTreeMap<String, String>>,
}

impl PaletteFile {
    pub fn load(path: &str) -> Result<PaletteFile, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("parsing {path}: {e}"))
    }
}

/// The terrain and water colours the mesher paints with, in linear light.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    /// By material code: air (unused), bedrock, rock, soil.
    pub material: [Linear; 4],
    pub water_deep: Linear,
    pub water_surface: Linear,
    pub haze: Linear,
    pub wet: f32,
    pub haze_amount: f32,
    /// Alpha of the water body's faces and of its surface's top face.
    pub body_alpha: f32,
    pub top_alpha: f32,
}

impl Look {
    pub fn from_file(p: &PaletteFile) -> Result<Look, String> {
        let t = &p.terrain;
        let c = |s: &str| hex_linear(s).ok_or_else(|| format!("not a colour: {s:?}"));
        let alpha = t.water_alpha.clamp(0.0, 1.0);
        Ok(Look {
            material: [[0.0; 3], c(&t.bedrock)?, c(&t.rock)?, c(&t.soil)?],
            water_deep: c(&t.water_deep)?,
            water_surface: c(&t.water_surface)?,
            haze: c(&t.haze)?,
            wet: t.wet,
            haze_amount: t.haze_amount,
            body_alpha: alpha,
            top_alpha: (alpha * t.skin_alpha_gain).min(0.95) * t.water_top_alpha,
        })
    }

    /// A plain look for tests: distinct greys, no haze.
    #[cfg(test)]
    pub fn plain() -> Look {
        Look {
            material: [[0.0; 3], [0.1; 3], [0.2; 3], [0.3; 3]],
            water_deep: [0.0, 0.0, 0.5],
            water_surface: [0.0, 0.5, 1.0],
            haze: [0.5; 3],
            wet: 0.5,
            haze_amount: 0.0,
            body_alpha: 0.5,
            top_alpha: 0.7,
        }
    }
}

/// `#rrggbb` to linear light (the sRGB transfer function `cubarium_gpu::palette` uses).
pub fn hex_linear(s: &str) -> Option<Linear> {
    let h = u32::from_str_radix(s.trim_start_matches('#'), 16).ok()?;
    let ch = |shift: u32| srgb_to_linear(((h >> shift) & 0xFF) as f32 / 255.0);
    Some([ch(16), ch(8), ch(0)])
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// `a` toward `b` by `t`.
#[inline]
pub fn mix(a: Linear, b: Linear, t: f32) -> Linear {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
