//! **Figures** (H2 of `design/handoffs/hybrid-renderer-2026-10-07.md`): the animals the GPU
//! renderer draws as models in its model pass (`cubarium_gpu::figures`) instead of stamping
//! them into the voxel texture.
//!
//! # From a tick to a frame
//!
//! - **Per tick** ([`specs`]): each animal becomes a [`Spec`] — the model it draws with
//!   (its founder's baked model for its mode and size, as the stamp chose; or the dev-mode
//!   glyph body), its place P in voxels (continuous: the pose's metres, not the cell it is
//!   in), its orientation (its heading, or pitched flat on a wall), its colours (the coat at
//!   each cell's material and band, the meal flash, starving and wounded tints, as the stamp
//!   painted), and its sleep, idle and hunker pose entries. A held lanternjaw lure is a
//!   figure of its own beside its body. Models are interned once ([`FigureModels`]) and go
//!   up to the GPU once.
//! - **Per frame** ([`FigureTrack::frame`]): positions and orientations interpolated between
//!   the two newest ticks on the frame clock, one pack interval behind it (the clock at
//!   `t` draws the figures of `t − (Tc − Tp)`, so a frame is always between two ticks that
//!   exist); the poses evaluated at that time; a birth fades in and a death fades out over
//!   `[figures] fade_s`; a model swap (a pose mode, a size bin) cross-fades over `swap_s`;
//!   in `turn = "steps"` the heading moves in `turn_steps` steps a turn, each step blended
//!   over `turn_blend_s`. In `movement = "stepped"` ([`Movement`]) the place drawn is that
//!   glide's, snapped to the world's voxel grid ([`on_grid`]: it hops a voxel at a time,
//!   as the stamp did), and the heading always moves in `turn_steps` steps, blended over
//!   `stepped_turn_blend_s` (0: a snap). The movement and subpixel placement change live
//!   ([`FigureTrack::set_motion`], the app's Settings page).
//! - **Picking** ([`FigurePick`]): the frame's figures, kept with the frame, are ray-cast
//!   per pick: the ray through a raster pixel into each figure's model frame, against its
//!   cells; the nearest cell in front of the terrain's own hit wins.

use std::sync::Arc;

use cubarium_gpu::figures::{
    CLASS_BREATHING, CLASS_LEG1, CLASS_LEG2, CLASS_NONE, CLASS_TORSO, FigureFrame, FigureInstance,
    FigureTile, FigureVoxel, PaletteEntry, crease_masks, open_faces,
};
use cubarium_gpu::voxel::{Breath, HeadMove, PoseEntry, pose_with};
use cubarium_voxel::VoxelView;
use cubarium_voxel_fauna::{Animal, DT, FaunaView, Founder, State};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::present::srgb_linear;

use super::animal::{self, AnimalPart, Animals};
use super::appearance;
use super::colours;
use super::hunker;
use super::idle;
use super::model::{ModelCell, ModelLibrary, Tag};
use super::sleep;

// --- the config ---------------------------------------------------------------------------

/// How the GPU renderer draws animals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimalDraw {
    /// The model pass (H2): each animal its model, moving and turning smoothly.
    #[default]
    Models,
    /// Voxels of the world's texture, as before H2: a step a tick, four headings.
    Stamped,
}

/// How the GPU renderer draws plants (`[figures] plants`, `CUBARIUM_PLANTS`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantDraw {
    /// Voxels of the world's texture, still, as ever (the default until Wrysk picks).
    #[default]
    Stamped,
    /// The plant pass (`cubarium_gpu::plants`): every plant cell a model voxel, swaying in
    /// the breeze and growing in voxel by voxel (`[plants]`).
    Models,
}

impl PlantDraw {
    pub fn name(self) -> &'static str {
        match self {
            PlantDraw::Stamped => "stamped",
            PlantDraw::Models => "models",
        }
    }

    pub fn parse(s: &str) -> Option<PlantDraw> {
        match s {
            "stamped" => Some(PlantDraw::Stamped),
            "models" => Some(PlantDraw::Models),
            _ => None,
        }
    }
}

/// How a figure's heading turns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Turn {
    /// Every frame at its own angle: a turn re-rasterises the model a few pixels at a time.
    #[default]
    Free,
    /// In fine steps (`turn_steps` a turn), each step blended into the next over
    /// `turn_blend_s`.
    Steps,
}

/// How a figure moves between ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Movement {
    /// Glides between ticks: its place interpolated, turning freely or in `turn_steps` per
    /// `turn`, placed between pixels or at whole ones per `subpixel`.
    #[default]
    Smooth,
    /// Hops a voxel at a time, as the stamped animals did: its place snapped to the world's
    /// voxel grid, its heading to `turn_steps` headings a turn (each change blended over
    /// `stepped_turn_blend_s`, 0: a snap), at whole pixels. `turn` and `subpixel` do not
    /// apply.
    Stepped,
}

impl Movement {
    pub fn name(self) -> &'static str {
        match self {
            Movement::Smooth => "smooth",
            Movement::Stepped => "stepped",
        }
    }

    pub fn parse(s: &str) -> Option<Movement> {
        match s {
            "smooth" => Some(Movement::Smooth),
            "stepped" => Some(Movement::Stepped),
            _ => None,
        }
    }
}

/// The two figure settings a viewer changes live (the app's Settings page): what
/// [`FigureTrack::set_motion`] takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FigureMotion {
    pub movement: Movement,
    pub subpixel: bool,
}

/// Which of the live settings the process environment named at startup
/// ([`FigureConfig::apply_env_overrides`]): `CUBARIUM_ANIMALS` names both,
/// `CUBARIUM_ANIMAL_SUBPIXEL` the placement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnvPins {
    pub movement: bool,
    pub subpixel: bool,
}

impl EnvPins {
    /// The settings named by `CUBARIUM_ANIMALS` = `animals` and `CUBARIUM_ANIMAL_SUBPIXEL`
    /// = `subpixel` (either unset: `None`): any preset names the movement; `crisp` and
    /// `smooth` name the placement too, `stepped` and `classic` leave it.
    pub fn of(animals: Option<&str>, subpixel: Option<&str>) -> EnvPins {
        EnvPins {
            movement: animals.is_some(),
            subpixel: subpixel.is_some() || matches!(animals, Some("crisp" | "smooth")),
        }
    }
}

impl FigureMotion {
    /// A viewer's stored settings as they apply when a renderer starts: each one the
    /// environment named keeps the environment's value (`env`), the rest are the viewer's.
    /// Settings changed while it runs apply whole ([`FigureTrack::set_motion`]).
    pub fn at_startup(stored: FigureMotion, env: FigureMotion, pins: EnvPins) -> FigureMotion {
        FigureMotion {
            movement: if pins.movement { env.movement } else { stored.movement },
            subpixel: if pins.subpixel { env.subpixel } else { stored.subpixel },
        }
    }
}

/// `[figures]`: the model pass's presentation (H2). Presentation only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FigureConfig {
    /// `"models"` (the default) or `"stamped"` (the pre-H2 look, for comparison).
    pub animals: AnimalDraw,
    /// `"smooth"` (the default) or `"stepped"` ([`Movement`]).
    pub movement: Movement,
    /// `"free"` (the default) or `"steps"`, in smooth movement.
    pub turn: Turn,
    /// Headings a full turn in `turn = "steps"`, and in stepped movement.
    pub turn_steps: u32,
    /// Seconds of sim time one step's blend takes in `turn = "steps"`.
    pub turn_blend_s: f64,
    /// Seconds of sim time a heading step's blend takes in stepped movement: 0 (the
    /// default) snaps.
    pub stepped_turn_blend_s: f64,
    /// Seconds of sim time a born animal fades in over, and a dead one out.
    pub fade_s: f64,
    /// Seconds of sim time a model swap (a pose mode, a size bin) cross-fades over.
    pub swap_s: f64,
    /// `true` (the default): a figure is placed between raster pixels, gliding, its edge
    /// pixels blended by how much of them it covers. `false`: at the nearest whole pixel,
    /// crisp, moving a pixel at a time.
    pub subpixel: bool,
    /// Tile samples per raster-pixel axis (positive; 1 disables AA). Startup only.
    #[serde(deserialize_with = "positive_supersample")]
    pub supersample: u32,
    /// `"models"` (the default, Wrysk 2026-10-08) or `"stamped"` ([`PlantDraw`]): plants drawn
    /// by the plant pass, swaying and growing in (`[plants]`), or in the world's voxels, still.
    /// Startup only.
    pub plants: PlantDraw,
}

fn positive_supersample<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let n = u32::deserialize(d)?;
    if n == 0 {
        return Err(serde::de::Error::custom("figures.supersample must be a positive integer"));
    }
    Ok(n)
}

impl Default for FigureConfig {
    fn default() -> FigureConfig {
        FigureConfig {
            animals: AnimalDraw::Models,
            movement: Movement::Smooth,
            turn: Turn::Free,
            turn_steps: 16,
            turn_blend_s: 0.1,
            stepped_turn_blend_s: 0.0,
            fade_s: 0.5,
            swap_s: 0.3,
            subpixel: true,
            supersample: 2,
            plants: PlantDraw::Models,
        }
    }
}

/// The `CUBARIUM_ANIMALS` presets.
pub const PRESETS: &str = "classic, crisp, smooth, stepped";

impl FigureConfig {
    /// The live settings ([`FigureMotion`]) this config draws with.
    pub fn motion(&self) -> FigureMotion {
        FigureMotion { movement: self.movement, subpixel: self.subpixel }
    }

    /// This config with the live settings `m`.
    pub fn with_motion(&self, m: FigureMotion) -> FigureConfig {
        FigureConfig { movement: m.movement, subpixel: m.subpixel, ..self.clone() }
    }

    /// Apply a per-run preset, then the turn, placement and AA overrides. Missing values
    /// preserve the presentation config; this is pure so tools and tests need not edit
    /// the process environment.
    pub fn with_overrides(
        &self,
        animals: Option<&str>,
        turn: Option<&str>,
        subpixel: Option<&str>,
        supersample: Option<&str>,
    ) -> anyhow::Result<Self> {
        let mut cfg = self.clone();
        match animals {
            None => {}
            Some("classic") => cfg.animals = AnimalDraw::Stamped,
            Some("crisp") => {
                cfg.animals = AnimalDraw::Models;
                cfg.movement = Movement::Smooth;
                cfg.subpixel = false;
                cfg.turn = Turn::Steps;
                cfg.turn_steps = 8;
            }
            Some("smooth") => {
                cfg.animals = AnimalDraw::Models;
                cfg.movement = Movement::Smooth;
                cfg.subpixel = true;
                cfg.turn = Turn::Free;
            }
            Some("stepped") => {
                cfg.animals = AnimalDraw::Models;
                cfg.movement = Movement::Stepped;
            }
            Some(value) => anyhow::bail!(
                "unknown CUBARIUM_ANIMALS value {value:?}; valid values: {PRESETS}"
            ),
        }
        if let Some(value) = turn {
            if value == "free" {
                cfg.turn = Turn::Free;
            } else {
                let steps = value
                    .parse::<u32>()
                    .ok()
                    .filter(|&n| n > 0 && value.bytes().all(|b| b.is_ascii_digit()));
                cfg.turn_steps = steps.ok_or_else(|| anyhow::anyhow!(
                    "unknown CUBARIUM_ANIMAL_TURN value {value:?}; valid values: free or a positive integer (1..={})",
                    u32::MAX
                ))?;
                cfg.turn = Turn::Steps;
            }
        }
        match subpixel {
            None => {}
            Some("0") => cfg.subpixel = false,
            Some("1") => cfg.subpixel = true,
            Some(value) => anyhow::bail!(
                "unknown CUBARIUM_ANIMAL_SUBPIXEL value {value:?}; valid values: 0, 1"
            ),
        }
        if let Some(value) = supersample {
            cfg.supersample = value.parse::<u32>().ok()
                .filter(|&n| n > 0 && value.bytes().all(|b| b.is_ascii_digit()))
                .ok_or_else(|| anyhow::anyhow!(
                    "unknown CUBARIUM_ANIMAL_AA value {value:?}; valid values: a positive integer (1..={})",
                    u32::MAX
                ))?;
        }
        Ok(cfg)
    }

    /// `CUBARIUM_PLANTS` applied: `models` or `stamped`, or anything else refused with the
    /// valid values named. Pure, like [`FigureConfig::with_overrides`].
    pub fn with_plants(&self, plants: Option<&str>) -> anyhow::Result<Self> {
        let mut cfg = self.clone();
        if let Some(value) = plants {
            cfg.plants = PlantDraw::parse(value).ok_or_else(|| {
                anyhow::anyhow!("unknown CUBARIUM_PLANTS value {value:?}; valid values: models, stamped")
            })?;
        }
        Ok(cfg)
    }

    /// Read the process overrides once at renderer startup, and report the effective
    /// settings. Low-level sinks do not read them: explicit captures and CPU/GPU parity
    /// checks retain their own presentation config. Returns which live settings the
    /// environment named ([`EnvPins`]).
    pub fn apply_env_overrides(&mut self) -> anyhow::Result<EnvPins> {
        let read = |name: &str, valid: &str| match std::env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(value)) => Err(anyhow::anyhow!(
                "invalid {name} value {value:?}; valid values: {valid}"
            )),
        };
        let animals = read("CUBARIUM_ANIMALS", PRESETS)?;
        let turn = read(
            "CUBARIUM_ANIMAL_TURN",
            "free or a positive integer (1..=4294967295)",
        )?;
        let subpixel = read("CUBARIUM_ANIMAL_SUBPIXEL", "0, 1")?;
        let supersample = read("CUBARIUM_ANIMAL_AA", "a positive integer (1..=4294967295)")?;
        let plants = read("CUBARIUM_PLANTS", "models, stamped")?;
        *self = self.with_overrides(animals.as_deref(), turn.as_deref(), subpixel.as_deref(), supersample.as_deref())?;
        *self = self.with_plants(plants.as_deref())?;
        eprintln!(
            "cubarium animals: animals={} movement={} turn={} turn_steps={} turn_blend_s={} stepped_turn_blend_s={} subpixel={} supersample={}; plants={}",
            match self.animals {
                AnimalDraw::Models => "models",
                AnimalDraw::Stamped => "stamped",
            },
            self.movement.name(),
            match self.turn {
                Turn::Free => "free",
                Turn::Steps => "steps",
            },
            self.turn_steps,
            self.turn_blend_s,
            self.stepped_turn_blend_s,
            self.subpixel,
            self.supersample,
            self.plants.name(),
        );
        Ok(EnvPins::of(animals.as_deref(), subpixel.as_deref()))
    }
}

#[cfg(test)]
mod preset_tests {
    use super::{AnimalDraw, EnvPins, FigureConfig, FigureMotion, Movement, PlantDraw, Turn};

    fn config() -> FigureConfig {
        FigureConfig {
            animals: AnimalDraw::Stamped,
            movement: Movement::Stepped,
            turn: Turn::Steps,
            turn_steps: 17,
            turn_blend_s: 0.75,
            stepped_turn_blend_s: 0.4,
            fade_s: 1.25,
            swap_s: 2.5,
            subpixel: false,
            supersample: 3,
            plants: PlantDraw::Models,
        }
    }

    #[test]
    fn the_plants_override_names_models_or_stamped_and_nothing_else() {
        let cfg = config();
        assert_eq!(cfg.with_plants(Some("stamped")).unwrap(), FigureConfig { plants: PlantDraw::Stamped, ..cfg.clone() });
        assert_eq!(cfg.with_plants(Some("models")).unwrap(), cfg);
        assert_eq!(cfg.with_plants(None).unwrap(), cfg);
        let err = cfg.with_plants(Some("sway")).unwrap_err().to_string();
        assert!(err.contains("CUBARIUM_PLANTS") && err.contains("models, stamped"), "{err}");
    }

    #[test]
    fn animal_presets_change_only_their_named_fields() {
        let base = config();
        assert_eq!(base.with_overrides(None, None, None, None).unwrap(), base);
        let models = FigureConfig {
            animals: AnimalDraw::Models,
            ..base.clone()
        };
        assert_eq!(
            models.with_overrides(Some("classic"), None, None, None).unwrap(),
            base
        );
        assert_eq!(
            base.with_overrides(Some("crisp"), None, None, None).unwrap(),
            FigureConfig {
                animals: AnimalDraw::Models,
                movement: Movement::Smooth,
                turn: Turn::Steps,
                turn_steps: 8,
                subpixel: false,
                ..base.clone()
            }
        );
        assert_eq!(
            base.with_overrides(Some("smooth"), None, None, None).unwrap(),
            FigureConfig {
                animals: AnimalDraw::Models,
                movement: Movement::Smooth,
                turn: Turn::Free,
                subpixel: true,
                ..base.clone()
            }
        );
        let smooth = FigureConfig { movement: Movement::Smooth, ..base.clone() };
        assert_eq!(
            smooth.with_overrides(Some("stepped"), None, None, None).unwrap(),
            FigureConfig {
                animals: AnimalDraw::Models,
                movement: Movement::Stepped,
                ..base
            },
            "stepped names the movement and the model pass, nothing else"
        );
    }

    #[test]
    fn the_live_settings_round_trip_through_a_config() {
        let base = config();
        for movement in [Movement::Smooth, Movement::Stepped] {
            for subpixel in [false, true] {
                let m = FigureMotion { movement, subpixel };
                let cfg = base.with_motion(m);
                assert_eq!(cfg.motion(), m);
                assert_eq!(FigureConfig { movement: base.movement, subpixel: base.subpixel, ..cfg }, base);
            }
        }
        for name in ["smooth", "stepped"] {
            assert_eq!(Movement::parse(name).map(Movement::name), Some(name));
        }
        assert_eq!(Movement::parse("Stepped"), None);
    }

    #[test]
    fn at_startup_the_environment_keeps_what_it_named_and_the_viewer_has_the_rest() {
        let env = FigureMotion { movement: Movement::Stepped, subpixel: false };
        let stored = FigureMotion { movement: Movement::Smooth, subpixel: true };
        let at = |movement, subpixel| FigureMotion::at_startup(stored, env, EnvPins { movement, subpixel });
        assert_eq!(at(false, false), stored, "nothing named: the viewer's settings");
        assert_eq!(at(true, true), env, "both named: the environment's");
        assert_eq!(at(true, false), FigureMotion { movement: env.movement, subpixel: stored.subpixel });
        assert_eq!(at(false, true), FigureMotion { movement: stored.movement, subpixel: env.subpixel });
        let pins = |a, s| { let p = EnvPins::of(a, s); (p.movement, p.subpixel) };
        assert_eq!(pins(None, None), (false, false));
        assert_eq!(pins(None, Some("0")), (false, true));
        assert_eq!(pins(Some("stepped"), None), (true, false));
        assert_eq!(pins(Some("crisp"), None), (true, true));
        assert_eq!(pins(Some("smooth"), None), (true, true));
    }

    #[test]
    fn turn_and_subpixel_overrides_take_precedence_over_presets() {
        let base = config();
        assert_eq!(
            base.with_overrides(Some("crisp"), Some("free"), Some("1"), None)
                .unwrap(),
            FigureConfig {
                animals: AnimalDraw::Models,
                movement: Movement::Smooth,
                turn: Turn::Free,
                turn_steps: 8,
                subpixel: true,
                ..base.clone()
            }
        );
        for steps in [1, 7, u32::MAX] {
            for preset in [None, Some("classic"), Some("crisp"), Some("smooth"), Some("stepped")] {
                let selected = base.with_overrides(preset, None, None, None).unwrap();
                let cfg = base
                    .with_overrides(preset, Some(&steps.to_string()), Some("0"), None)
                    .unwrap();
                assert_eq!(
                    cfg,
                    FigureConfig {
                        turn: Turn::Steps,
                        turn_steps: steps,
                        subpixel: false,
                        ..selected
                    }
                );
            }
        }
        assert_eq!(
            base.with_overrides(None, Some("free"), Some("1"), None).unwrap(),
            FigureConfig {
                turn: Turn::Free,
                subpixel: true,
                ..base
            }
        );
    }

    #[test]
    fn aa_override_is_positive_and_preserves_other_settings() {
        let base = config();
        for factor in [1, 2, 7, u32::MAX] {
            let cfg = base
                .with_overrides(None, None, None, Some(&factor.to_string()))
                .unwrap();
            assert_eq!(
                cfg,
                FigureConfig {
                    supersample: factor,
                    ..base.clone()
                }
            );
            let encoded = toml::to_string(&cfg).unwrap();
            assert_eq!(toml::from_str::<FigureConfig>(&encoded).unwrap(), cfg);
        }
        for value in ["", "0", "-1", "+1", "1.5", " 2", "4294967296"] {
            let error = base
                .with_overrides(None, None, None, Some(value))
                .unwrap_err()
                .to_string();
            assert!(error.contains("CUBARIUM_ANIMAL_AA"), "{error}");
            assert!(error.contains("positive integer"), "{error}");
        }
        assert!(toml::from_str::<FigureConfig>("supersample = 0").is_err());
        assert_eq!(
            toml::from_str::<FigureConfig>("").unwrap().supersample,
            FigureConfig::default().supersample
        );
    }

    #[test]
    fn bad_animal_overrides_name_the_variable_and_valid_values() {
        let base = config();
        for value in ["", "models", "Classic", " smooth"] {
            let error = base
                .with_overrides(Some(value), None, None, None)
                .unwrap_err()
                .to_string();
            assert!(error.contains("CUBARIUM_ANIMALS"), "{error}");
            assert!(error.contains("classic, crisp, smooth, stepped"), "{error}");
        }
        for value in ["", "steps", "0", "-1", "+1", "1.5", " 8", "4294967296"] {
            let error = base
                .with_overrides(None, Some(value), None, None)
                .unwrap_err()
                .to_string();
            assert!(error.contains("CUBARIUM_ANIMAL_TURN"), "{error}");
            assert!(error.contains("free or a positive integer"), "{error}");
        }
        for value in ["", "true", "false", "2", "01"] {
            let error = base
                .with_overrides(None, None, Some(value), None)
                .unwrap_err()
                .to_string();
            assert!(error.contains("CUBARIUM_ANIMAL_SUBPIXEL"), "{error}");
            assert!(error.contains("0, 1"), "{error}");
        }
        assert_eq!(
            base,
            config(),
            "an error leaves the caller's config untouched"
        );
    }
}

// --- models -------------------------------------------------------------------------------

/// A model the figures draw with ([`FigureModels`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModelId(pub u32);

/// What a model cell paints with, before a figure's colours are known.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Paint {
    /// A baked cell: its founder's coat at this material and band, an eye or sense patch
    /// when `accent`.
    Coat { material: u8, band: u8, accent: bool },
    /// A glyph body's style: wood, crown and heart, three entries from here.
    Style,
    /// One flat colour (the lure).
    Flat,
}

/// One cell of a model as it is built.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cell {
    offset: [i32; 3],
    paint: Paint,
    class: u8,
    head: bool,
    glyph: u8,
}

#[derive(Clone, Debug)]
struct ModelInfo {
    first: u32,
    count: u32,
    centre: [f32; 3],
    radius: f32,
    /// Its palette: one entry a distinct paint (three for `Style`).
    paints: Vec<Paint>,
    /// The rows its torso settles when asleep or hunkered (`sleep::Classified::drop`).
    drop: i32,
}

/// Every model the figures have drawn with, interned: its cells in the GPU's layout, once.
#[derive(Clone, Debug, Default)]
pub struct FigureModels {
    voxels: Arc<Vec<FigureVoxel>>,
    uploaded: usize,
    models: Vec<ModelInfo>,
    by_key: FxHashMap<u64, ModelId>,
}

impl FigureModels {
    /// The model under `key`, built by `cells` the first time. `None` for a model with no
    /// cells, or one whose offsets do not fit a byte.
    fn intern(&mut self, key: u64, cells: impl FnOnce() -> (Vec<Cell>, i32)) -> Option<ModelId> {
        if let Some(&id) = self.by_key.get(&key) {
            return Some(id);
        }
        let (cells, drop) = cells();
        if cells.is_empty()
            || cells.iter().any(|c| c.offset.iter().any(|&o| !(-120..=120).contains(&o)))
        {
            return None;
        }
        let mut paints: Vec<Paint> = Vec::new();
        let mut entry = Vec::with_capacity(cells.len());
        for c in &cells {
            let at = match paints.iter().position(|p| *p == c.paint) {
                Some(i) => i,
                None => {
                    paints.push(c.paint);
                    paints.len() - 1
                }
            };
            entry.push(at);
        }
        // The palette's place of each paint: `Style` takes three entries.
        let mut place = Vec::with_capacity(paints.len());
        let mut n = 0usize;
        for p in &paints {
            place.push(n);
            n += if *p == Paint::Style { 3 } else { 1 };
        }
        if n > 255 {
            return None;
        }
        let offsets: Vec<[i8; 3]> = cells.iter().map(|c| c.offset.map(|o| o as i8)).collect();
        let open = open_faces(&offsets);
        let creases = crease_masks(&offsets);
        let voxels = Arc::make_mut(&mut self.voxels);
        let first = voxels.len() as u32;
        for ((((c, o), &e), &op), &crease) in cells
            .iter()
            .zip(&offsets)
            .zip(&entry)
            .zip(&open)
            .zip(&creases)
        {
            voxels.push(
                FigureVoxel::new(*o, place[e] as u8, c.class, c.head, op, c.glyph)
                    .with_creases(crease),
            );
        }
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for c in &cells {
            let b = [c.offset[0] as f32 - 0.5, c.offset[1] as f32, c.offset[2] as f32 - 0.5];
            for i in 0..3 {
                lo[i] = lo[i].min(b[i]);
                hi[i] = hi[i].max(b[i] + 1.0);
            }
        }
        let centre = [0, 1, 2].map(|i| 0.5 * (lo[i] + hi[i]));
        let half = [0, 1, 2].map(|i| 0.5 * (hi[i] - lo[i]));
        // A margin of a voxel: a settling head dips below the body's box.
        let radius = (half[0] * half[0] + half[1] * half[1] + half[2] * half[2]).sqrt() + 1.0;
        let id = ModelId(self.models.len() as u32);
        self.models.push(ModelInfo {
            first,
            count: cells.len() as u32,
            centre,
            radius,
            paints,
            drop,
        });
        self.by_key.insert(key, id);
        Some(id)
    }

    /// Cells not yet on the GPU: where they start and what they are.
    pub fn pending(&self) -> (usize, &[FigureVoxel]) {
        (self.uploaded, &self.voxels[self.uploaded..])
    }

    /// The GPU holds every cell so far.
    pub fn uploaded(&mut self) {
        self.uploaded = self.voxels.len();
    }

    /// Every cell, for picking.
    pub fn voxels(&self) -> Arc<Vec<FigureVoxel>> {
        self.voxels.clone()
    }

    fn info(&self, id: ModelId) -> &ModelInfo {
        &self.models[id.0 as usize]
    }
}

fn hash_of<T: std::hash::Hash>(t: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = rustc_hash::FxHasher::default();
    t.hash(&mut h);
    h.finish()
}

fn class_code(c: Option<sleep::Class>) -> u8 {
    match c {
        None => CLASS_NONE,
        Some(sleep::Class::Torso) => CLASS_TORSO,
        Some(sleep::Class::Breathing) => CLASS_BREATHING,
        Some(sleep::Class::Leg1) => CLASS_LEG1,
        Some(sleep::Class::Leg2) => CLASS_LEG2,
    }
}

/// A baked model's cells for a figure: its coat paints, and its pose classes — the
/// sleep's, or toward its baked hunker (`toward`) — and its head, as the stamp classed
/// them (`Animals::stamp_model`).
fn baked(cells: &[ModelCell], toward: Option<&[ModelCell]>) -> (Vec<Cell>, i32) {
    let top = cells.iter().map(|m| m.offset[1]).max().unwrap_or(0);
    let local: Vec<sleep::Local> = cells
        .iter()
        .map(|m| sleep::Local {
            a: m.offset[0],
            c: m.offset[2],
            row: m.offset[1],
            bump: m.tag != Tag::Accent,
        })
        .collect();
    let k = match toward {
        Some(target) => hunker::classify(&local, target),
        None => sleep::classify(&local),
    };
    let torso = sleep::classify(&local).torso;
    let head = idle::head_of_model(&local, torso);
    let out = cells
        .iter()
        .zip(&k.class)
        .zip(&head)
        .map(|((m, &class), &head)| Cell {
            offset: m.offset,
            paint: Paint::Coat {
                material: m.material,
                band: colours::band(m.offset[1], top),
                accent: m.tag == Tag::Accent,
            },
            class: class_code(class),
            head,
            glyph: 0,
        })
        .collect();
    (out, k.drop)
}

// --- a tick's figures ---------------------------------------------------------------------

/// How a figure is turned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Orient {
    /// Standing, heading `h` (radians from +z toward +x; the model's forward +x faces it).
    Ground(f64),
    /// Pitched flat on a wall: the model's x, y and z axes in the world.
    Axes([[f64; 3]; 3]),
}

impl Orient {
    /// The model's axes in the world: forward `(sin h, 0, cos h)`, up, side
    /// `(−cos h, 0, sin h)`, the stamp's quarter turns at the quarter headings
    /// (`model::turn`).
    pub fn axes(self) -> [[f64; 3]; 3] {
        match self {
            Orient::Ground(h) => {
                let (s, c) = h.sin_cos();
                [[s, 0.0, c], [0.0, 1.0, 0.0], [-c, 0.0, s]]
            }
            Orient::Axes(a) => a,
        }
    }
}

/// A figure's pose this tick: its body's entry and, while its head moves, its head's, and
/// the breaths at the tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoseSpec {
    pub tick: u64,
    pub body: PoseEntry,
    pub head: Option<PoseEntry>,
    pub sleep: Breath,
    pub idle: Breath,
}

impl PoseSpec {
    /// Settle and breath of body and head, voxels, `ticks` after the staged tick.
    fn at(&self, ticks: f64) -> [f32; 4] {
        let b = pose_with(self.sleep, self.idle, &self.body, ticks as f32);
        let h = self.head.map_or(b, |h| pose_with(self.sleep, self.idle, &h, ticks as f32));
        [b.settle.min(2.0), b.breath, h.settle.min(2.0), h.breath]
    }
}

/// One figure in one tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    /// The animal, and which of its figures (0 its body, 1 its lure).
    pub key: (u64, u8),
    pub model: ModelId,
    /// P: where the model's anchor is, voxels.
    pub pos: [f64; 3],
    pub orient: Orient,
    pub palette: Vec<PaletteEntry>,
    pub pose: Option<PoseSpec>,
}

/// The model a founder draws its lure with: one emitting cell at the tail, over the back.
fn lure_spec(animal: &Animal, fauna: &FaunaView<'_>, voxel_m: f64, body: &Spec, models: &mut FigureModels, emission: bool) -> Option<Spec> {
    if !(animal.founder_state.lure > 0.0) || animal.state == State::Asleep {
        return None;
    }
    let founder = animal.founder?;
    let b = fauna.config.founder(founder).body_at(animal.body);
    let back = (0.5 * b.length_m / voxel_m).round() as i32;
    let rows = ((b.height_m / voxel_m).ceil() as i32).max(1);
    let model = models.intern(hash_of(&("lure", back, rows)), || {
        (vec![Cell { offset: [-back, rows, 0], paint: Paint::Flat, class: CLASS_NONE, head: false, glyph: 0 }], 0)
    })?;
    let c = srgb_linear(animal::LANTERNJAW_LURE_SRGB);
    Some(Spec {
        key: (animal.id, 1),
        model,
        pos: body.pos,
        orient: body.orient,
        palette: vec![PaletteEntry { base: c, emit: emission.then_some(c) }],
        pose: None,
    })
}

/// Where a body's anchor is, voxels: its pose's `x` and `z`, and its feet — on the ground
/// the top of its support face, on a wall the layer its feet have reached (continuous), a
/// flyer its base height.
fn feet(animal: &Animal, v: f64) -> [f64; 3] {
    let y = if let Some(c) = animal.mobility.climb {
        f64::from(animal.site.y) + 1.0 + c.offset_m() / v
    } else if animal.pose.y_m.is_finite() && animal.pose.y_m > 0.0 {
        animal.pose.y_m / v
    } else {
        f64::from(animal.site.y) + 1.0
    };
    [animal.pose.x / v, y, animal.pose.z / v]
}

/// A glyph body (no baked model: the dev-mode bodies, a rider): its cells as
/// [`animal::cells_of_local`] lays them, in its own frame where it has one.
fn glyph_spec(animal: &Animal, fauna: &FaunaView<'_>, v: f64, at: [f64; 3], models: &mut FigureModels, flash: f32) -> Option<Spec> {
    let placed = animal::cells_of_local(animal, fauna.config, 0, v);
    if placed.is_empty() {
        return None;
    }
    let framed = placed.iter().all(|(_, _, l)| l.is_some());
    let anchor = (
        (animal.pose.x / v).floor() as i64,
        i64::from(animal.site.y) + 1,
        (animal.pose.z / v).floor() as i64,
    );
    let cells: Vec<Cell> = placed
        .iter()
        .map(|(cell, part, l)| Cell {
            offset: match l {
                Some(l) if framed => [l.a, l.row, l.c],
                _ => [
                    (cell.x - anchor.0) as i32,
                    (i64::from(cell.y) - anchor.1) as i32,
                    (i64::from(cell.z) - anchor.2) as i32,
                ],
            },
            paint: Paint::Style,
            class: CLASS_NONE,
            head: matches!(part, AnimalPart::Head { .. }),
            glyph: appearance::animal_glyph(*part).0,
        })
        .collect();
    let key = hash_of(&cells.iter().map(|c| (c.offset, c.glyph)).collect::<Vec<_>>());
    let model = models.intern(key, || (cells, 0))?;
    let style = animal::animal_style(animal, flash);
    let (pos, orient) = if framed {
        (at, Orient::Ground(animal.pose.heading_rad))
    } else {
        // An interim block or a crawler stood on a wall: drawn where the stamp drew it.
        (
            [anchor.0 as f64 + 0.5, anchor.1 as f64, anchor.2 as f64 + 0.5],
            Orient::Axes([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        )
    };
    Some(Spec {
        key: (animal.id, 0),
        model,
        pos,
        orient,
        palette: [style.wood, style.crown, style.heart].map(|base| PaletteEntry { base, emit: None }).to_vec(),
        pose: None,
    })
}

/// Every animal of `fauna` as figures, after [`Animals::rebuild_without_living`] moved the
/// pose and flash memory on to this tick. `lib` is the baked models (or `None` for the
/// dev-mode glyphs).
pub fn specs(
    animals: &Animals,
    view: &VoxelView<'_>,
    fauna: FaunaView<'_>,
    lib: Option<&ModelLibrary>,
    models: &mut FigureModels,
    out: &mut Vec<Spec>,
) {
    out.clear();
    let v = view.config.voxel_m;
    if !(v > 0.0) {
        return;
    }
    let lib = lib.filter(|l| l.serves(v));
    let emission = animals.emission_on();
    for a in fauna.animals {
        if !a.pose.is_finite() {
            continue;
        }
        let flash_level = animals.flash_level(a);
        let flash = animals.flash_amount(flash_level);
        // A rider on its host's back: its own box, at the host's place, lifted.
        if let Some(hold) = a.attached {
            let (Some(host), Some(lift)) = (fauna.animal(hold.host), animal::rider_lift(&fauna, hold.host, v)) else {
                continue;
            };
            let mut at = feet(host, v);
            at[1] += f64::from(lift);
            if let Some(mut s) = glyph_spec(a, &fauna, v, at, models, flash) {
                if let Orient::Ground(_) = s.orient {
                    s.orient = Orient::Ground(host.pose.heading_rad);
                    s.pos = at;
                }
                out.push(s);
            }
            continue;
        }
        let at = feet(a, v);
        let baked_cells = lib.and_then(|lib| animal::model_cells(lib, fauna.config, a));
        let (Some(cells), Some(founder)) = (baked_cells, a.founder) else {
            if let Some(s) = glyph_spec(a, &fauna, v, at, models, flash) {
                out.push(s);
            }
            continue;
        };
        let ground = a.mobility.climb.is_none();
        let asleep = animals.sleep_weight(a).filter(|_| ground);
        let idling = animals.idle_weight(a).filter(|_| ground);
        let hunkering = animals.hunker_weight(a).filter(|_| ground);
        let toward = hunkering.and(lib).and_then(|lib| animal::hunker_cells(lib, fauna.config, a));
        let key = hash_of(&(
            cells.as_ptr() as usize,
            cells.len(),
            toward.map(|t| (t.as_ptr() as usize, t.len())),
        ));
        let Some(model) = models.intern(key, || baked(cells, toward)) else { continue };
        let info = models.info(model);
        let starving = animal::is_starving(a);
        let wounded = animal::is_wounded(a) && !starving;
        let palette = info
            .paints
            .iter()
            .map(|p| match *p {
                Paint::Coat { material, band, accent } => {
                    let (style, emit) = animal::model_colours(founder, material, band, accent, starving, wounded, flash);
                    PaletteEntry {
                        base: style.wood,
                        emit: emit.filter(|_| emission).map(srgb_linear),
                    }
                }
                Paint::Style | Paint::Flat => PaletteEntry::default(),
            })
            .collect();
        let pose = (asleep.is_some() || idling.is_some() || hunkering.is_some()).then(|| {
            let (sw, sr) = asleep.unwrap_or((0.0, 0.0));
            let (iw, ir) = idling.unwrap_or((0.0, 0.0));
            let (hw, hr) = hunkering.unwrap_or((0.0, 0.0));
            let settles = asleep.is_some() || hunkering.is_some();
            let body = PoseEntry {
                sleep: sw,
                sleep_rate: sr,
                drop: if settles { info.drop as f32 } else { 0.0 },
                sleep_phase: if asleep.is_some() { sleep::phase(a.id) } else { 0.0 },
                idle: iw,
                idle_rate: ir,
                idle_phase: if idling.is_some() { idle::phase(a.id) } else { 0.0 },
                head: HeadMove::NONE,
                hunker: hw,
                hunker_rate: hr,
            };
            let head = idling
                .filter(|_| idle::moves_head(founder))
                .and_then(|_| idle::move_near(a.id, view.tick, DT))
                .map(|m| PoseEntry { head: m.relative_to(view.tick), ..body });
            PoseSpec {
                tick: view.tick,
                body,
                head,
                sleep: sleep::breath(view.tick, DT),
                idle: idle::breath(view.tick, DT),
            }
        });
        let orient = match a.mobility.climb {
            Some(climb) => {
                let (tx, tz) = super::animal::wall_toward(&climb);
                let fy = if climb.ascending { 1.0 } else { -1.0 };
                let (ux, uz) = (-f64::from(tx), -f64::from(tz));
                Orient::Axes([[0.0, fy, 0.0], [ux, 0.0, uz], [fy * uz, 0.0, -fy * ux]])
            }
            None => Orient::Ground(a.pose.heading_rad),
        };
        // On a wall the anchor is the face's plane at the feet: the stamp's anchor cell, its
        // back half a voxel out of the face.
        let pos = match orient {
            Orient::Axes(ax) if a.mobility.climb.is_some() => {
                [at[0] - 0.5 * ax[1][0], at[1] + 0.5, at[2] - 0.5 * ax[1][2]]
            }
            _ => at,
        };
        let body = Spec { key: (a.id, 0), model, pos, orient, palette, pose };
        if founder == Founder::Lanternjaw
            && let Some(l) = lure_spec(a, &fauna, v, &body, models, emission)
        {
            out.push(l);
        }
        out.push(body);
    }
}

/// The emitting cells of a tick's figures and their colours (linear): the glow volume's
/// share of the animals.
pub fn emitters(models: &FigureModels, specs: &[Spec], width: u32, height: u32, depth: u32) -> Vec<((u32, u32, u32), [f32; 3])> {
    let mut out = Vec::new();
    for s in specs {
        if s.palette.iter().all(|p| p.emit.is_none()) {
            continue;
        }
        let info = models.info(s.model);
        let ax = s.orient.axes();
        for v in &models.voxels[info.first as usize..(info.first + info.count) as usize] {
            let Some(e) = s.palette.get(usize::from(v.palette())).and_then(|p| p.emit) else { continue };
            let o = v.offset().map(f64::from);
            let local = [o[0], o[1] + 0.5, o[2]];
            let w = [0, 1, 2].map(|i| s.pos[i] + ax[0][i] * local[0] + ax[1][i] * local[1] + ax[2][i] * local[2]);
            let (y, z) = (w[1].floor(), w[2].floor());
            if y < 0.0 || z < 0.0 || y >= f64::from(height) || z >= f64::from(depth) {
                continue;
            }
            let x = (w[0].floor() as i64).rem_euclid(i64::from(width)) as u32;
            out.push(((x, y as u32, z as u32), e));
        }
    }
    out
}

// --- between ticks ------------------------------------------------------------------------

/// A rotation as a unit quaternion `[w, x, y, z]`.
type Quat = [f64; 4];

fn quat_of(m: [[f64; 3]; 3]) -> Quat {
    // m[c] is column c (the model's axis c in the world): element (row r, col c) = m[c][r].
    let e = |r: usize, c: usize| m[c][r];
    let tr = e(0, 0) + e(1, 1) + e(2, 2);
    let q = if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        [0.25 * s, (e(2, 1) - e(1, 2)) / s, (e(0, 2) - e(2, 0)) / s, (e(1, 0) - e(0, 1)) / s]
    } else if e(0, 0) > e(1, 1) && e(0, 0) > e(2, 2) {
        let s = (1.0 + e(0, 0) - e(1, 1) - e(2, 2)).sqrt() * 2.0;
        [(e(2, 1) - e(1, 2)) / s, 0.25 * s, (e(0, 1) + e(1, 0)) / s, (e(0, 2) + e(2, 0)) / s]
    } else if e(1, 1) > e(2, 2) {
        let s = (1.0 + e(1, 1) - e(0, 0) - e(2, 2)).sqrt() * 2.0;
        [(e(0, 2) - e(2, 0)) / s, (e(0, 1) + e(1, 0)) / s, 0.25 * s, (e(1, 2) + e(2, 1)) / s]
    } else {
        let s = (1.0 + e(2, 2) - e(0, 0) - e(1, 1)).sqrt() * 2.0;
        [(e(1, 0) - e(0, 1)) / s, (e(0, 2) + e(2, 0)) / s, (e(1, 2) + e(2, 1)) / s, 0.25 * s]
    };
    let n = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    q.map(|v| v / n)
}

fn axes_of(q: Quat) -> [[f64; 3]; 3] {
    let [w, x, y, z] = q;
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)],
        [2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)],
        [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)],
    ]
}

fn slerp(a: Quat, b: Quat, t: f64) -> Quat {
    let mut d: f64 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    let b = if d < 0.0 {
        d = -d;
        b.map(|v| -v)
    } else {
        b
    };
    if d > 0.9995 {
        let q: Quat = [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t);
        let n = q.iter().map(|v| v * v).sum::<f64>().sqrt();
        return q.map(|v| v / n);
    }
    let th = d.clamp(-1.0, 1.0).acos();
    let (sa, sb) = (((1.0 - t) * th).sin() / th.sin(), (t * th).sin() / th.sin());
    [0, 1, 2, 3].map(|i| a[i] * sa + b[i] * sb)
}

/// The shortest way from `a` to `b` round a ring of `width`, voxels.
fn ring_delta(a: f64, b: f64, width: f64) -> f64 {
    let d = b - a;
    if width > 0.0 { d - width * (d / width).round() } else { d }
}

/// A move between two ticks longer than this, voxels, is a jump (a world reloaded, a
/// rider let go) and is drawn where it lands, not glided.
pub const JUMP_VOXELS: f64 = 6.0;

/// Where a figure is at `u` of the way from its last tick's spec `a` to this tick's `b`:
/// position along the short way round the ring, orientation along the shorter arc.
pub fn between(a: &Spec, b: &Spec, u: f64, width: f64) -> ([f64; 3], [[f64; 3]; 3]) {
    let dx = ring_delta(a.pos[0], b.pos[0], width);
    let d = [dx, b.pos[1] - a.pos[1], b.pos[2] - a.pos[2]];
    if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > JUMP_VOXELS {
        return (b.pos, b.orient.axes());
    }
    let pos = [0, 1, 2].map(|i| a.pos[i] + d[i] * u);
    let axes = match (a.orient, b.orient) {
        // A heading turns by the shorter way; so does a quaternion, but the angle keeps
        // the stamp's axes exact at quarter turns.
        (Orient::Ground(ha), Orient::Ground(hb)) => {
            let dh = ring_delta(ha, hb, std::f64::consts::TAU);
            Orient::Ground(ha + dh * u).axes()
        }
        (oa, ob) => axes_of(slerp(quat_of(oa.axes()), quat_of(ob.axes()), u)),
    };
    (pos, axes)
}

/// The cross-fade weight `t` seconds into a blend of `span` seconds: a smoothstep, so it
/// starts and ends with no speed; 1 for a span of 0.
pub fn blend_weight(t: f64, span: f64) -> f32 {
    if !(span > 0.0) {
        return 1.0;
    }
    let x = (t / span).clamp(0.0, 1.0);
    (x * x * (3.0 - 2.0 * x)) as f32
}

/// A heading's step in `steps` a turn.
pub fn heading_step(h: f64, steps: u32) -> i64 {
    let n = f64::from(steps.max(1));
    (h / std::f64::consts::TAU * n).round() as i64
}

fn step_heading(k: i64, steps: u32) -> f64 {
    k as f64 * std::f64::consts::TAU / f64::from(steps.max(1))
}

/// Where a figure turned `orient` stands on the world's voxel grid nearest `pos`: its
/// cells' boxes (each `[o − ½, o + ½]` across and `[o, o + 1]` up, in its own frame) at
/// whole voxels, as the stamp put them. Standing, the anchor goes to a voxel's middle
/// across and its floor up, whatever the heading (it turns about that point); on a wall
/// (a quarter-turned frame) the box of its anchor cell goes to whole voxels.
pub fn on_grid(pos: [f64; 3], orient: Orient) -> [f64; 3] {
    let corner = match orient {
        Orient::Ground(_) => [-0.5, 0.0, -0.5],
        Orient::Axes(ax) => {
            let lo = [-0.5, 0.0, -0.5];
            [0, 1, 2].map(|i| {
                let c: f64 = (0..3).map(|k| ax[k][i] * lo[k]).sum();
                (c * 2.0).round() * 0.5
            })
        }
    };
    [0, 1, 2].map(|i| (pos[i] + corner[i]).round() - corner[i])
}

#[derive(Clone, Debug)]
struct Live {
    /// When it was first drawn, sim seconds; `None` for one there from the start.
    born: Option<f64>,
    model: ModelId,
    /// A swap under way: the model and colours it left, and when.
    swap: Option<(ModelId, Vec<PaletteEntry>, f64)>,
    /// Turn steps: the step drawn, the one it left and when.
    step: Option<(i64, i64, f64)>,
    /// Gone from the newest tick since then: it fades out from `last`.
    gone: Option<f64>,
    last: Spec,
}

#[derive(Clone, Debug)]
struct Snap {
    tick: u64,
    specs: Vec<Spec>,
    index: FxHashMap<(u64, u8), usize>,
}

/// The figures between ticks: the two newest ticks' specs, and each figure's fades.
#[derive(Clone, Debug, Default)]
pub struct FigureTrack {
    cfg: FigureConfig,
    width: f64,
    prev: Option<Snap>,
    cur: Option<Snap>,
    live: FxHashMap<(u64, u8), Live>,
}

/// One figure as a frame draws it, for picking.
#[derive(Clone, Debug, PartialEq)]
pub struct PickFigure {
    pub id: u64,
    pub first: u32,
    pub count: u32,
    pub pos: [f32; 3],
    pub axes: [[f32; 3]; 3],
}

impl FigureTrack {
    pub fn new(cfg: FigureConfig, width: u32) -> FigureTrack {
        FigureTrack { cfg, width: f64::from(width), ..FigureTrack::default() }
    }

    /// The live settings it draws with.
    pub fn motion(&self) -> FigureMotion {
        self.cfg.motion()
    }

    /// Draw with the live settings `m` from the next frame on. Returns whether they
    /// changed. Heading steps under way are dropped, so a figure turns at once to where
    /// the new mode puts it rather than finishing a blend of the old mode's.
    pub fn set_motion(&mut self, m: FigureMotion) -> bool {
        if self.cfg.motion() == m {
            return false;
        }
        self.cfg = self.cfg.with_motion(m);
        for l in self.live.values_mut() {
            l.step = None;
        }
        true
    }

    /// A new tick's figures, seen at `now_s` (sim seconds of the frame clock).
    pub fn push(&mut self, tick: u64, specs: &[Spec], now_s: f64) {
        if self.cur.as_ref().is_some_and(|c| tick < c.tick) {
            // Another world, or this one reloaded: nothing carries over.
            self.prev = None;
            self.cur = None;
            self.live.clear();
        }
        if self.cur.as_ref().is_some_and(|c| c.tick == tick) {
            // The same tick again (a pack retried): its specs replace it.
            self.cur = None;
        }
        let started = self.cur.is_some() || self.prev.is_some();
        let index = specs.iter().enumerate().map(|(i, s)| (s.key, i)).collect();
        let snap = Snap { tick, specs: specs.to_vec(), index };
        let mut seen = rustc_hash::FxHashSet::default();
        for s in specs {
            seen.insert(s.key);
            match self.live.get_mut(&s.key) {
                Some(l) => {
                    if l.gone.is_some() {
                        l.gone = None;
                    }
                    if l.model != s.model {
                        l.swap = Some((l.model, l.last.palette.clone(), now_s));
                        l.model = s.model;
                    }
                    l.last = s.clone();
                }
                None => {
                    self.live.insert(
                        s.key,
                        Live {
                            born: started.then_some(now_s),
                            model: s.model,
                            swap: None,
                            step: None,
                            gone: None,
                            last: s.clone(),
                        },
                    );
                }
            }
        }
        let fade = self.cfg.fade_s;
        self.live.retain(|k, l| {
            if !seen.contains(k) && l.gone.is_none() {
                l.gone = Some(now_s);
            }
            l.gone.is_none_or(|g| now_s - g < fade)
        });
        self.prev = self.cur.take();
        self.cur = Some(snap);
    }

    /// The frame at clock `t` (ticks), `highlight` leaning that animal toward white by
    /// `mix`, from the models in `models`: what the model pass draws, and what a pick reads.
    pub fn frame(&mut self, t: f64, highlight: Option<(u64, f32)>, models: &FigureModels) -> (FigureFrame, Vec<PickFigure>) {
        let stepped = self.cfg.movement == Movement::Stepped;
        let mut frame = FigureFrame { whole_pixels: stepped || !self.cfg.subpixel, supersample: self.cfg.supersample, ..FigureFrame::default() };
        let mut picks = Vec::new();
        let Some(cur) = &self.cur else { return (frame, picks) };
        let now_s = t / f64::from(cubarium_voxel::TICK_HZ);
        let tc = cur.tick as f64;
        let (tp, u) = match &self.prev {
            Some(p) if cur.tick > p.tick => {
                let span = tc - p.tick as f64;
                (p.tick as f64, ((t - tc) / span).clamp(0.0, 1.0))
            }
            _ => (tc, 1.0),
        };
        let td = tp + (tc - tp) * u;
        let cfg = self.cfg.clone();
        let width = self.width;
        let mut keys: Vec<(u64, u8)> = self.live.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            let Some(l) = self.live.get_mut(&key) else { continue };
            let now = cur.index.get(&key).map(|&i| &cur.specs[i]);
            let before = self.prev.as_ref().and_then(|p| p.index.get(&key).map(|&i| &p.specs[i]));
            let (spec, pos, axes, alpha) = match (now, l.gone) {
                (Some(b), None) => {
                    let (pos, axes) = match before {
                        Some(a) => between(a, b, u, width),
                        None => (b.pos, b.orient.axes()),
                    };
                    let alpha = l.born.map_or(1.0, |born| blend_weight(now_s - born, cfg.fade_s));
                    (b, pos, axes, alpha)
                }
                _ => {
                    let g = l.gone.unwrap_or(now_s);
                    let alpha = 1.0 - blend_weight(now_s - g, cfg.fade_s);
                    if alpha <= 0.0 {
                        continue;
                    }
                    (&l.last, l.last.pos, l.last.orient.axes(), alpha)
                }
            };
            // Stepped: the glide's place, on the voxel grid; it hops a voxel each time the
            // glide crosses into the next.
            let pos = if stepped { on_grid(pos, spec.orient) } else { pos };
            // The pose: the older tick's entries run on to the drawn time (they end where
            // the newer ones start), else the newer's from its own tick.
            let pose = match (before.and_then(|a| a.pose), spec.pose) {
                (Some(p), _) => p.at(td - p.tick as f64),
                (None, Some(p)) => p.at((td - p.tick as f64).max(0.0)),
                (None, None) => [0.0; 4],
            };
            let heading = match spec.orient {
                Orient::Ground(_) => Some(axes[0][0].atan2(axes[0][2])),
                Orient::Axes(_) => None,
            };
            let swap = l.swap.as_ref().filter(|(_, _, at)| now_s - at < cfg.swap_s).cloned();
            if swap.is_none() {
                l.swap = None;
            }
            // Turn steps: the step the heading is nearest, blended from the last one.
            let mut step_pair: Option<([[f64; 3]; 3], [[f64; 3]; 3], f32)> = None;
            let mut axes = axes;
            let blend_s = if stepped { cfg.stepped_turn_blend_s } else { cfg.turn_blend_s };
            if (stepped || cfg.turn == Turn::Steps)
                && let Some(h) = heading
            {
                let k = heading_step(h, cfg.turn_steps);
                let (shown, from, at) = match l.step {
                    None => (k, k, now_s - blend_s),
                    Some((shown, from, at)) if shown != k => {
                        // Mid-blend, it leaves whichever of the two shows more.
                        let w = blend_weight(now_s - at, blend_s);
                        let left = if w >= 0.5 { shown } else { from };
                        (k, left, now_s)
                    }
                    Some(s) => s,
                };
                l.step = Some((shown, from, at));
                let on = Orient::Ground(step_heading(shown, cfg.turn_steps)).axes();
                axes = on;
                let w = blend_weight(now_s - at, blend_s);
                if from != shown && w < 1.0 {
                    step_pair = Some((Orient::Ground(step_heading(from, cfg.turn_steps)).axes(), on, w));
                }
            }
            let f32s = |a: [[f64; 3]; 3]| a.map(|c| c.map(|v| v as f32));
            let tile_of = |model: ModelId, palette: &[PaletteEntry], axes: [[f64; 3]; 3], frame: &mut FigureFrame| {
                let info = models.info(model);
                let at = frame.palette.len() as u32;
                frame.palette.extend_from_slice(palette);
                FigureTile {
                    first: info.first,
                    count: info.count,
                    pos: pos.map(|v| v as f32),
                    axes: f32s(axes),
                    centre: info.centre,
                    radius: info.radius,
                    pose,
                    palette: at,
                }
            };
            let (a, b, weight_b) = match (&swap, step_pair) {
                (Some((old, old_pal, at)), _) => {
                    let ta = tile_of(*old, old_pal, axes, &mut frame);
                    let tb = tile_of(spec.model, &spec.palette, axes, &mut frame);
                    (ta, Some(tb), blend_weight(now_s - at, cfg.swap_s))
                }
                (None, Some((from, on, w))) => {
                    let ta = tile_of(spec.model, &spec.palette, from, &mut frame);
                    let tb = tile_of(spec.model, &spec.palette, on, &mut frame);
                    (ta, Some(tb), w)
                }
                (None, None) => (tile_of(spec.model, &spec.palette, axes, &mut frame), None, 0.0),
            };
            let shown = b.unwrap_or(a);
            picks.push(PickFigure { id: key.0, first: shown.first, count: shown.count, pos: shown.pos, axes: shown.axes });
            frame.figures.push(FigureInstance {
                id: key.0,
                a,
                b,
                weight_b,
                alpha,
                highlight: highlight.filter(|h| h.0 == key.0).map_or(0.0, |h| h.1),
            });
        }
        (frame, picks)
    }
}

// --- picking ------------------------------------------------------------------------------

/// A frame's figures for picking: the ray through a raster pixel against their cells.
#[derive(Clone, Debug, Default)]
pub struct FigurePick {
    pub s: u32,
    pub rise: u32,
    pub base: i32,
    pub width: u32,
    pub figures: Vec<PickFigure>,
    pub voxels: Arc<Vec<FigureVoxel>>,
}

impl FigurePick {
    /// Where the ray through world raster pixel `(px, py)` (`px` round the ring) first meets
    /// a cell of a figure: `(z, id)`, z in voxels along the ray.
    pub fn hit(&self, px: i32, py: i32) -> Option<(f32, u64)> {
        if self.s == 0 {
            return None;
        }
        let (s, r) = (self.s as f32, self.rise as f32 / self.s as f32);
        let rw = (self.width * self.s) as i32;
        let x = (px.rem_euclid(rw.max(1)) as f32 + 0.5) / s;
        let y0 = (self.base as f32 - py as f32 - 0.5) / s;
        // The ray: (x, y0 − r·t, t) for t (= z) from 0 on.
        let dir = [0.0, -r, 1.0];
        let mut best: Option<(f32, u64)> = None;
        for f in &self.figures {
            let w = self.width as f32;
            let dx = {
                let d = x - f.pos[0];
                d - w * (d / w).round()
            };
            let o = [dx, y0 - f.pos[1], -f.pos[2]];
            let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            // Into the model's frame: the axes are orthonormal.
            let lo = [dot(o, f.axes[0]), dot(o, f.axes[1]), dot(o, f.axes[2])];
            let ld = [dot(dir, f.axes[0]), dot(dir, f.axes[1]), dot(dir, f.axes[2])];
            let cells = self.voxels.get(f.first as usize..(f.first + f.count) as usize).unwrap_or(&[]);
            for v in cells {
                let c = v.offset().map(f32::from);
                let bmin = [c[0] - 0.5, c[1], c[2] - 0.5];
                let bmax = [c[0] + 0.5, c[1] + 1.0, c[2] + 0.5];
                let (mut t0, mut t1) = (0.0f32, f32::MAX);
                let mut hit = true;
                for i in 0..3 {
                    if ld[i].abs() < 1e-9 {
                        if lo[i] < bmin[i] || lo[i] > bmax[i] {
                            hit = false;
                            break;
                        }
                    } else {
                        let (a, b) = ((bmin[i] - lo[i]) / ld[i], (bmax[i] - lo[i]) / ld[i]);
                        t0 = t0.max(a.min(b));
                        t1 = t1.min(a.max(b));
                    }
                }
                if hit && t0 <= t1 && best.is_none_or(|(bt, _)| t0 < bt) {
                    best = Some((t0, f.id));
                }
            }
        }
        best
    }

    /// The figure with a cell nearest `(px, py)` within `radius` raster pixels.
    pub fn nearest(&self, px: i32, py: i32, radius: f32) -> Option<u64> {
        let rw = (self.width * self.s) as f32;
        let mut best: Option<(f32, u64)> = None;
        for f in &self.figures {
            for (x, y) in self.cell_points(f) {
                let d = (px as f32 + 0.5 - x).rem_euclid(rw);
                let dx = d.min(rw - d);
                let dy = py as f32 + 0.5 - y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= radius && best.is_none_or(|(b, _)| dist < b) {
                    best = Some((dist, f.id));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// Each cell's centre of figure `f` in world raster pixels.
    fn cell_points<'a>(&'a self, f: &'a PickFigure) -> impl Iterator<Item = (f32, f32)> + 'a {
        let cells = self.voxels.get(f.first as usize..(f.first + f.count) as usize).unwrap_or(&[]);
        cells.iter().map(move |v| {
            let c = v.offset().map(f32::from);
            let l = [c[0], c[1] + 0.5, c[2]];
            let w = [0, 1, 2].map(|i| f.pos[i] + f.axes[0][i] * l[0] + f.axes[1][i] * l[1] + f.axes[2][i] * l[2]);
            let p = cubarium_gpu::figures::project(self.s, self.rise, self.base, w);
            (p[0], p[1])
        })
    }

    /// Where figure `id` is drawn, world raster pixels (`x` in `0..raster_w`): the middle of
    /// its cells.
    pub fn animal_at(&self, id: u64) -> Option<(f32, f32)> {
        let f = self.figures.iter().find(|f| f.id == id)?;
        let rw = (self.width * self.s) as f32;
        let ref_x = cubarium_gpu::figures::project(self.s, self.rise, self.base, f.pos)[0];
        let (mut sx, mut sy, mut n) = (0.0f32, 0.0f32, 0.0f32);
        for (x, y) in self.cell_points(f) {
            let d = (x - ref_x + rw / 2.0).rem_euclid(rw) - rw / 2.0;
            sx += ref_x + d;
            sy += y;
            n += 1.0;
        }
        (n > 0.0).then(|| ((sx / n).rem_euclid(rw), sy / n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: u64, model: ModelId, pos: [f64; 3], h: f64) -> Spec {
        Spec { key: (id, 0), model, pos, orient: Orient::Ground(h), palette: vec![PaletteEntry::default()], pose: None }
    }

    fn one_cell(models: &mut FigureModels, key: u64) -> ModelId {
        models
            .intern(key, || {
                (vec![Cell { offset: [0, 0, 0], paint: Paint::Flat, class: CLASS_NONE, head: false, glyph: 0 }], 0)
            })
            .unwrap()
    }

    #[test]
    fn a_figure_moves_between_two_ticks_on_the_frame_clock() {
        let a = spec(1, ModelId(0), [10.0, 4.0, 5.0], 0.0);
        let b = spec(1, ModelId(0), [11.0, 4.0, 7.0], std::f64::consts::FRAC_PI_2);
        let (p, axes) = between(&a, &b, 0.25, 64.0);
        assert_eq!(p, [10.25, 4.0, 5.5]);
        // A quarter of the way through a quarter turn: forward at π/8 from +z.
        let h = axes[0][0].atan2(axes[0][2]);
        assert!((h - std::f64::consts::FRAC_PI_8).abs() < 1e-9, "{h}");
        // Round the ring the short way: from x 63.5 to 0.5 crosses the seam.
        let c = spec(1, ModelId(0), [63.5, 4.0, 5.0], 0.0);
        let d = spec(1, ModelId(0), [0.5, 4.0, 5.0], 0.0);
        assert_eq!(between(&c, &d, 0.5, 64.0).0[0], 64.0);
        // A jump is drawn where it lands.
        let far = spec(1, ModelId(0), [30.0, 4.0, 5.0], 0.0);
        assert_eq!(between(&a, &far, 0.5, 64.0).0, far.pos);
    }

    #[test]
    fn the_track_draws_one_pack_interval_behind_the_clock() {
        let mut models = FigureModels::default();
        let m = one_cell(&mut models, 1);
        let mut track = FigureTrack::new(FigureConfig::default(), 64);
        track.push(100, &[spec(7, m, [10.0, 4.0, 5.0], 0.0)], 5.0);
        track.push(101, &[spec(7, m, [12.0, 4.0, 5.0], 0.0)], 5.05);
        let x_at = |track: &mut FigureTrack, t: f64| track.frame(t, None, &models).0.figures[0].a.pos[0];
        assert_eq!(x_at(&mut track, 101.0), 10.0, "at the newest tick: the one before");
        assert_eq!(x_at(&mut track, 101.5), 11.0);
        assert_eq!(x_at(&mut track, 102.5), 12.0, "past a whole interval it holds the newest");
    }

    #[test]
    fn a_birth_fades_in_a_death_fades_out_and_the_first_tick_shows_whole() {
        let mut models = FigureModels::default();
        let m = one_cell(&mut models, 1);
        let cfg = FigureConfig { fade_s: 1.0, ..FigureConfig::default() };
        let mut track = FigureTrack::new(cfg, 64);
        let tick_s = |t: u64| t as f64 / f64::from(cubarium_voxel::TICK_HZ);
        track.push(0, &[spec(1, m, [1.0, 1.0, 1.0], 0.0)], tick_s(0));
        let alpha = |track: &mut FigureTrack, t: f64, id: u64| {
            track.frame(t, None, &models).0.figures.iter().find(|f| f.id == id).map(|f| f.alpha)
        };
        assert_eq!(alpha(&mut track, 0.0, 1), Some(1.0), "there from the start");
        track.push(1, &[spec(1, m, [1.0, 1.0, 1.0], 0.0), spec(2, m, [5.0, 1.0, 1.0], 0.0)], tick_s(1));
        assert_eq!(alpha(&mut track, 1.0, 2), Some(0.0), "born: from nothing");
        let half = alpha(&mut track, 11.0, 2).unwrap();
        assert!((half - 0.5).abs() < 1e-6, "{half}");
        track.push(2, &[spec(2, m, [5.0, 1.0, 1.0], 0.0)], tick_s(2));
        let leaving = alpha(&mut track, 12.0, 1).unwrap();
        assert!((leaving - 0.5).abs() < 1e-6, "dead: half gone halfway: {leaving}");
        track.push(25, &[spec(2, m, [5.0, 1.0, 1.0], 0.0)], tick_s(25));
        assert_eq!(alpha(&mut track, 25.0, 1), None, "gone after the fade");
    }

    #[test]
    fn a_model_swap_cross_fades_from_the_old_model_to_the_new() {
        let mut models = FigureModels::default();
        let (m1, m2) = (one_cell(&mut models, 1), one_cell(&mut models, 2));
        let cfg = FigureConfig { swap_s: 1.0, ..FigureConfig::default() };
        let mut track = FigureTrack::new(cfg, 64);
        track.push(0, &[spec(1, m1, [1.0, 1.0, 1.0], 0.0)], 0.0);
        track.push(1, &[spec(1, m2, [1.0, 1.0, 1.0], 0.0)], 0.05);
        let at = |track: &mut FigureTrack, t: f64| {
            let f = track.frame(t, None, &models).0.figures[0];
            (f.a.first, f.b.map(|b| b.first), f.weight_b)
        };
        let (a, b, w) = at(&mut track, 1.0);
        assert_eq!((a, b), (models.info(m1).first, Some(models.info(m2).first)));
        assert_eq!(w, 0.0);
        let (_, _, w) = at(&mut track, 11.0);
        assert!((w - 0.5).abs() < 1e-6, "{w}");
        let (a, b, _) = at(&mut track, 30.0);
        assert_eq!((a, b), (models.info(m2).first, None), "done: the new model alone");
    }

    #[test]
    fn turn_steps_blend_from_the_last_step_to_the_next() {
        let mut models = FigureModels::default();
        let m = one_cell(&mut models, 1);
        let cfg = FigureConfig { turn: Turn::Steps, turn_steps: 4, turn_blend_s: 1.0, ..FigureConfig::default() };
        let mut track = FigureTrack::new(cfg, 64);
        track.push(0, &[spec(1, m, [1.0, 1.0, 1.0], 0.0)], 0.0);
        let f = track.frame(0.0, None, &models).0.figures[0];
        assert!(f.b.is_none(), "settled on its step");
        track.push(1, &[spec(1, m, [1.0, 1.0, 1.0], std::f64::consts::FRAC_PI_2)], 0.05);
        // Halfway through the tick the heading is π/4: still step 0 (a tie rounds away).
        let f = track.frame(1.4, None, &models).0.figures[0];
        assert!(f.b.is_none(), "{f:?}");
        let f = track.frame(1.8, None, &models).0.figures[0];
        let b = f.b.expect("blending into step 1");
        assert!((f.a.axes[0][2] - 1.0).abs() < 1e-6 && (b.axes[0][0] - 1.0).abs() < 1e-6);
        assert_eq!(f.weight_b, 0.0);
        let f = track.frame(1.8 + 10.0, None, &models).0.figures[0];
        assert!((f.weight_b - 0.5).abs() < 1e-6, "{f:?}");
    }

    fn stepped(steps: u32) -> FigureConfig {
        FigureConfig { movement: Movement::Stepped, turn_steps: steps, ..FigureConfig::default() }
    }

    /// Whether `x` is a whole number of voxels plus `phase`.
    fn on(x: f32, phase: f32) -> bool {
        let d = x - phase;
        (d - d.round()).abs() < 1e-4
    }

    /// The forward heading of a tile's axes, in steps of `steps` a turn.
    fn steps_of(axes: [[f32; 3]; 3], steps: u32) -> f64 {
        let h = f64::from(axes[0][0]).atan2(f64::from(axes[0][2]));
        h / std::f64::consts::TAU * f64::from(steps)
    }

    #[test]
    fn stepped_movement_draws_every_frame_on_the_voxel_grid_and_a_step_heading() {
        let mut models = FigureModels::default();
        let m = one_cell(&mut models, 1);
        for (movement, expect_grid) in [(Movement::Stepped, true), (Movement::Smooth, false)] {
            let cfg = FigureConfig { movement, turn_steps: 16, ..FigureConfig::default() };
            let mut track = FigureTrack::new(cfg, 64);
            // Off the grid at both ends, across voxels, turning by an odd angle; a flyer's
            // height in between rows.
            track.push(10, &[spec(1, m, [10.3, 4.2, 5.7], 0.37)], 0.5);
            track.push(11, &[spec(1, m, [13.1, 4.9, 3.4], 1.91)], 0.55);
            let (mut all_on, mut xs, mut headings_on) = (true, Vec::new(), true);
            for i in 0..=40 {
                let (frame, picks) = track.frame(11.0 + f64::from(i) / 40.0, None, &models);
                let f = frame.figures[0];
                for t in std::iter::once(f.a).chain(f.b) {
                    all_on &= on(t.pos[0], 0.5) && on(t.pos[1], 0.0) && on(t.pos[2], 0.5);
                    let k = steps_of(t.axes, 16);
                    headings_on &= (k - k.round()).abs() < 1e-4;
                }
                assert_eq!(picks[0].pos, f.b.unwrap_or(f.a).pos, "picking reads the place drawn");
                if expect_grid {
                    assert!(frame.whole_pixels, "stepped places at whole pixels whatever subpixel says");
                    assert!(f.b.is_none(), "a heading step snaps (stepped_turn_blend_s 0)");
                }
                xs.push(f.a.pos[0]);
            }
            assert_eq!(all_on, expect_grid, "{movement:?}: every frame on the grid");
            assert_eq!(headings_on, expect_grid, "{movement:?}: every heading on a step");
            if expect_grid {
                xs.dedup();
                assert_eq!(xs, vec![10.5, 11.5, 12.5, 13.5], "it hops a voxel at a time, every voxel on the way");
            }
        }
    }

    #[test]
    fn on_a_wall_the_anchor_cells_box_lands_on_whole_voxels() {
        // A climber facing up a wall whose outward normal is −x: the axes `specs` builds.
        let (ux, uz, fy) = (-1.0f64, 0.0f64, 1.0f64);
        let orient = Orient::Axes([[0.0, fy, 0.0], [ux, 0.0, uz], [fy * uz, 0.0, -fy * ux]]);
        let p = on_grid([7.81, 3.37, 2.62], orient);
        let ax = orient.axes();
        // The anchor cell's box in the world: corner + each axis × [lo, hi].
        let (lo, hi) = ([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5]);
        for i in 0..3 {
            let a: f64 = p[i] + (0..3).map(|k| ax[k][i] * lo[k]).sum::<f64>();
            let b: f64 = p[i] + (0..3).map(|k| ax[k][i] * hi[k]).sum::<f64>();
            for v in [a, b] {
                assert!((v - v.round()).abs() < 1e-9, "axis {i}: {a}..{b} from {p:?}");
            }
        }
    }

    #[test]
    fn toggling_the_motion_changes_the_next_frames_placement_and_back() {
        let mut models = FigureModels::default();
        let m = one_cell(&mut models, 1);
        let cfg = FigureConfig { turn: Turn::Steps, turn_steps: 16, turn_blend_s: 1.0, ..FigureConfig::default() };
        let mut track = FigureTrack::new(cfg, 64);
        track.push(10, &[spec(1, m, [10.3, 4.0, 5.7], 0.0)], 0.5);
        track.push(11, &[spec(1, m, [11.4, 4.0, 5.9], 1.0)], 0.55);
        let smooth = FigureMotion { movement: Movement::Smooth, subpixel: true };
        let step = FigureMotion { movement: Movement::Stepped, subpixel: true };
        assert_eq!(track.motion(), smooth);
        let t = 11.6;
        track.frame(11.0, None, &models);
        let (before, _) = track.frame(t, None, &models);
        let f = before.figures[0];
        assert!(!on(f.a.pos[0], 0.5) && !before.whole_pixels);
        assert!(f.b.is_some(), "mid heading blend in smooth steps");
        assert!(track.set_motion(step));
        assert!(!track.set_motion(step), "the same settings again change nothing");
        let (now, _) = track.frame(t, None, &models);
        let g = now.figures[0];
        assert!(on(g.a.pos[0], 0.5) && on(g.a.pos[2], 0.5) && now.whole_pixels, "{g:?}");
        assert!(g.b.is_none(), "the smooth blend under way is dropped, not finished");
        let k = steps_of(g.a.axes, 16);
        assert!((k - k.round()).abs() < 1e-4, "on a step: {k}");
        // Stepped turns snap: a step taken just now (from step 0 at the tick to the one at
        // `t`) leaves nothing for smooth's blend to pick up when it comes back.
        track.frame(11.0, None, &models);
        let (snapped, _) = track.frame(t, None, &models);
        assert!(snapped.figures[0].b.is_none());
        // Subpixel off in smooth: the same glide, at whole pixels, on the step it is on.
        assert!(track.set_motion(FigureMotion { subpixel: false, ..smooth }));
        let (crisp, _) = track.frame(t, None, &models);
        assert!(crisp.whole_pixels && crisp.figures[0].a.pos == f.a.pos);
        assert!(crisp.figures[0].b.is_none(), "no blend back from the step stepped left: {:?}", crisp.figures[0]);
        assert_eq!(crisp.figures[0].a.axes, snapped.figures[0].a.axes, "the step stepped showed");
        assert!(track.set_motion(smooth));
        let (back, _) = track.frame(t + 30.0 / 1000.0, None, &models);
        assert!(!back.whole_pixels && !on(back.figures[0].a.pos[0], 0.5), "back to the glide");
    }

    #[test]
    fn a_pick_finds_the_figure_whose_cell_the_pixel_shows() {
        let mut models = FigureModels::default();
        let m = one_cell(&mut models, 1);
        let mut track = FigureTrack::new(FigureConfig::default(), 16);
        // One cell at x 4.5 (spanning 4..5), y 2..3, z 3..4.
        track.push(0, &[spec(9, m, [4.5, 2.0, 3.5], std::f64::consts::FRAC_PI_2)], 0.0);
        let (_, figures) = track.frame(0.0, None, &models);
        let pick = FigurePick { s: 4, rise: 2, base: 40, width: 16, figures, voxels: models.voxels() };
        // Its front face: columns 16..20, rows base − 3·4 − 3·2 = 22 .. 26.
        assert_eq!(pick.hit(17, 23).map(|h| h.1), Some(9));
        let (z, _) = pick.hit(17, 23).unwrap();
        assert!((z - 3.0).abs() < 1e-4, "it meets the front face at z 3: {z}");
        // Its cap: the rise rows above the front face.
        assert_eq!(pick.hit(17, 21).map(|h| h.1), Some(9));
        assert_eq!(pick.hit(21, 23), None, "the next column");
        assert_eq!(pick.hit(17 + 64, 23).map(|h| h.1), Some(9), "round the ring");
        assert_eq!(pick.nearest(22, 24, 5.0), Some(9));
        assert_eq!(pick.nearest(30, 24, 5.0), None);
    }
}
