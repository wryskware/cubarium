//! `cubarium voxel`: run a [`cubarium_voxel::World`] at a fixed 20 Hz and draw it as
//! pixel art into a ring raster, through the same sinks the rest of the host uses.
//!
//! The loop is [`crate::run::drive`]'s shape — one [`Clock`], one `encode_raster` per
//! rendered frame, one sink — with two differences: the world is a voxel strip rather
//! than the fixture scenes, and a stdin reader thread lets the run be poked while it is
//! running (pause, single-step, speed, rain, set a cell's material, save, load, inspect,
//! outlet, quit).
//!
//! **`save [DIR]` saves the whole world** — the world, the plants and the animals, in the
//! layered snapshot `--load DIR` resumes — on demand, whether or not the run has a state
//! directory; `SIGUSR1` and Ctrl+S in the development window do the same into
//! `saves/<time>-t<tick>/` ([`save`]).
//!
//! **`save-terrain` and `l` are terrain only** (Astra R9.6). `World::save`/`World::load` carry the
//! voxels, the water and the world's own tick, and **nothing** of the plant or animal
//! layers: there is no atomic world/flora/fauna envelope yet, and the plant layer does not
//! serialize at all. So `l` is refused in place whenever this run holds ecology state — any
//! stand, any animal, any provisioned ground, any seed bank, any nonzero ecological ledger
//! — because restoring another world's terrain under living stands would leave them
//! standing on water and rock they never grew in, with their ledgers still describing the
//! terrain that is gone. A terrain snapshot is resumed with `--load PATH` at startup, which
//! begins the ecology fresh on it, and that is what the refusal says.
//!
//! `--speed` scales world ticks per clock tick through an accumulator, so a fractional
//! speed slows the world down without touching the clock: the picture is still drawn at
//! `--fps` and the simulation still advances in whole 20 Hz ticks.
//!
//! The projection, the palette and the autotiling live in [`project`] and [`present`];
//! the hand-authored fixture in [`scene`]; the stands' own geometry and palettes in
//! [`stand`]. The preview sink is deliberately absent: the minifb window refuses ring
//! rasters and that refusal is correct.
//!
//! The run owns a [`cubarium_voxel_flora::Flora`] beside the world and steps it once
//! after every world tick, in that order: the plant layer reads the water the core has
//! just moved and withdraws from it, never the other way round. Its stands are drawn
//! into the same canvas, in the same traversal, as the terrain and the water.
//!
//! Since voxel round 5c it owns a [`cubarium_voxel_fauna::Fauna`] too, stepped once after
//! the plant layer and for the same reason the plant layer follows the world: the animals
//! eat the foliage the plants have just grown. One coupled tick is `World::step`,
//! `Flora::step`, `Fauna::step`, and the animals are drawn in the same traversal as
//! everything else, as the **interim** glyph of [`animal`].

pub mod animal;
pub mod appearance;
pub mod candle;
pub mod chiplet;
pub mod hunker;
pub mod idle;
pub mod colours;
pub mod model;
pub mod patches;
pub mod placement;
pub mod present;
pub mod project;
pub mod save;
pub mod sleep;
pub mod stand;
pub mod textures;
pub mod vine;

// The seeded habitat, the authored fixture and the founding loop live in
// `cubarium-voxel-sim` since P5-B, so the search crate founds the worlds this host founds;
// every name is re-exported here unchanged.
pub use cubarium_voxel_sim::found::{
    Founded, HABITAT_TRIES, LAKE_SEED_TRIES, LayerConfig, Seeds, found, generate_with_a_lake,
};
pub use cubarium_voxel_sim::{habitat, scene};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use cubarium_render::Canvas;
use cubarium_search::es::voxel::{EpisodeDriver, VoxelPolicyFile};
use cubarium_surface::{Scale, Topology};
use cubarium_voxel::{PoreBand, ViabilitySpec, World};
#[cfg(test)]
use cubarium_voxel::{Command as VoxelCommand, Material};
use cubarium_voxel_fauna::{Controller, Fauna, FaunaConfig, Founder, Response};
#[cfg(test)]
use cubarium_voxel_fauna::Command as FaunaCommand;
use cubarium_voxel_flora::{Flora, FloraConfig, Species};
#[cfg(test)]
use cubarium_voxel_flora::Site;
use cubarium_voxel_sim::params::Params;
use cubarium_voxel_sim::{Arena, ArenaGrid, Sim, SimConfig};
use serde::{Deserialize, Serialize};

use crate::cli::{Voxel, VoxelArenaArg, VoxelControllerArg, VoxelSceneArg, VoxelSinkArg};
use crate::clock::{Clock, MAX_FPS, MIN_FPS, Step};
use crate::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use crate::sink::{FrameSink, GpuTargetKind, Output, PngSink, WebSink, WorldShape};

use present::VoxelPresenter;
use project::Projection;

// The console's commands, speeds and the one function that applies a line live in the
// session crate (`cubarium_session::console`), shared with the Godot session.
use cubarium_session::console::{COMMANDS, Console, MAX_SPEED, MIN_SPEED, Pacer, console_line};
#[cfg(test)]
use cubarium_session::console::{cell_state, ecology_state};
use cubarium_session::create::{Populate, Scene, StartLayers, StartSpec};
#[cfg(test)]
use cubarium_session::create::random_seed;
use cubarium_session::founders::FounderRecipe;
#[cfg(test)]
use cubarium_session::founders::{BUILT_IN_POLICIES, built_in_driver, install_founder_controllers};
use cubarium_session::params::{SimParams, config_table};
use cubarium_session::store::{save_voxel_snapshot, save_whole_world};
#[cfg(test)]
use cubarium_session::store::{layer_files, load_layers, parse_voxel_tick};

// --- The config file -----------------------------------------------------------------

/// `--config`: the presentation, plus the world to build. Missing fields take the
/// defaults below and unknown fields are errors, so a typo is not silently ignored.
///
/// The simulation's half — `[world]`, `[flora]`, `[fauna]`, `threads`, `water_every`,
/// `fast_forward_water_every`, `[observatory]` — is the session crate's
/// [`SimParams`] ([`VoxelConfig::sim`]); the rest is the picture's and stays here.
///
/// ```toml
/// tilt_degrees = 30.0   # the chosen camera
/// px_per_voxel = 4      # the chosen camera
/// raster_height = 0     # 0 derives it from the world
/// haze = 0.55
/// water_alpha = 0.5
///
/// [world]
/// width = 128
/// height = 48
/// # depth is deliberately absent: it comes from `cubarium_voxel::Config::default()`, so
/// # the habitat's chosen depth lives in the core and the presenter never pins its own.
///
/// [world.landform]
/// preset = "default"   # small | default | wide, or `landform = "ridge"`
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", default, deny_unknown_fields)]
pub struct VoxelConfig {
    /// Elevation of the orthographic view, in degrees. Reaches the picture only as
    /// `round(px_per_voxel · tan(tilt))` whole pixels of lift per voxel of depth. The
    /// chosen camera is 30°.
    pub tilt_degrees: f64,
    /// Pixels per voxel edge. The chosen camera is 4: below that the sprite atlas stops
    /// reading. `"auto"` (stored as [`PX_AUTO`]) is resolved once at start-up by
    /// [`auto_px_per_voxel`]: the largest that fits the output — the desktop's screen, or
    /// the panel — with the whole strip in view; for the development window,
    /// [`cover_px_per_voxel`] of the largest screen, scaled down to fill the window.
    #[serde(with = "px_serde")]
    pub px_per_voxel: u32,
    /// Ring raster height in pixels; `0` derives it so the whole strip fits. The width is
    /// always `world.width · px_per_voxel`, so the strip fills it exactly.
    pub raster_height: u16,
    /// How far the back wall fades toward the haze colour, `0..=1`.
    pub haze: f32,
    /// Opacity of one voxel of free water, `0..=1`.
    pub water_alpha: f32,
    /// Micro-dithering strength on solid terrain faces (0.0 to 1.0).
    pub dither: f32,
    /// Whether to render a vertical sky gradient.
    pub sky_gradient: bool,
    /// How organisms are drawn: `"models"`, the voxel models baked from the simulation's
    /// numbers (package V, `assets/voxel-models`), or `"glyphs"`, the dev-mode glyphs that
    /// every species and founder without a model falls back to anyway.
    pub organisms: OrganismLook,
    /// Where the baked models are: `<models_dir>/<voxel mm>/manifest.json`. A relative path
    /// is from the working directory: the repository on the desktop, `/var/lib/cubarium`
    /// under the panel's unit. Missing models are said once and drawn as glyphs.
    pub models_dir: PathBuf,
    /// Draw the GPU renderer's face textures and the latticevine's tile layer.
    /// **Experimental and off by default** (Wrysk, 2026-09-24: voxels stay the main
    /// look): off, every face is a solid voxel as before textures and the latticevine is
    /// plain voxel cells. `--textures` turns it on for one run.
    pub textures: bool,
    /// Draw the latticevine as its tile layer even with [`textures`] off: terrain and every
    /// other organism stay plain voxels, and only the vine's tiles are loaded from
    /// `textures_dir`. Off (the default), the vine is plain voxel cells unless `textures`
    /// is on. Wrysk, 2026-10-02: textures for the vine alone.
    pub vine_tiles: bool,
    /// The GPU renderer's face textures ([`textures`]): `masters/`, `lod/<px>/` and
    /// `override/<px>/`. Relative as `models_dir` is. Missing textures are said once and
    /// the faces drawn solid. The CPU presenter never reads them.
    pub textures_dir: PathBuf,
    /// The GPU renderer's lighting tier. `"flat"`, the default, is the flat tier's
    /// stand-ins for light (the column roof shade, the top-face gain and tint), which the
    /// panel and the cube keep; `"lit"` shades each face texel by the ambient light
    /// (sky × AO × canopy, on a ladder) instead, which `config/desktop/*.toml` ask for.
    /// The CPU presenter ignores it. Package L, `design/handoffs/presentation-plan-2026-09-24.md`.
    pub lighting: Lighting,
    /// The lit tier's numbers ([`LightConfig`]); ignored by the flat tier.
    pub light: LightConfig,
    /// **Seconds a frondgrazer takes to hunker down under its shield, and to come back up**
    /// (HUNKER, [`hunker`]): the pose moves continuously over this, never in one step.
    /// Presentation only: the simulation's hunker is unchanged by it.
    pub hunker_s: f64,
    /// The meal flash's strength, `0..=1`: how far an eating animal's eye or sense patch
    /// leans toward the warm flash colour at the height of the flash. It eases in over
    /// `animal::MEAL_FLASH_FADE_S` once the animal starts cropping and back out after; 0
    /// turns it off. Presentation only.
    pub meal_flash: f32,
    /// **The observatory** (`crates/cubarium-observatory`): serve the running world's
    /// stats and the companion dashboard over HTTP at this address, e.g.
    /// `"127.0.0.1:7878"` (this machine only) or `"0.0.0.0:7878"` (the LAN; no auth).
    /// Absent, the default, serves nothing and the loop pays nothing. `--serve ADDR`
    /// overrides it for one run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serve: Option<String>,
    /// **The simulation's half** of the file ([`SimParams`]): the world to build, the
    /// plant and animal parameters, the tick's threads and water fidelity, the served
    /// history's sampling. Read from the same top-level keys as ever.
    #[serde(skip)]
    pub sim: SimParams,
}

impl Default for VoxelConfig {
    fn default() -> VoxelConfig {
        VoxelConfig {
            // The camera chosen by the storyboard study: 30°, 4 px per voxel.
            tilt_degrees: 30.0,
            px_per_voxel: 4,
            raster_height: 0,
            haze: 0.55,
            water_alpha: 0.5,
            dither: 0.04,
            sky_gradient: true,
            organisms: OrganismLook::Models,
            models_dir: PathBuf::from("assets/voxel-models"),
            textures: false,
            vine_tiles: false,
            textures_dir: PathBuf::from("assets/voxel-textures"),
            lighting: Lighting::Flat,
            light: LightConfig::default(),
            hunker_s: hunker::DEFAULT_S,
            meal_flash: animal::MEAL_FLASH_DEFAULT,
            serve: None,
            sim: SimParams::default(),
        }
    }
}

// The file is one flat table: its simulation keys go to [`SimParams`] (unknown ones there
// refused), everything else to the presentation fields (unknown ones refused).
impl<'de> Deserialize<'de> for VoxelConfig {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<VoxelConfig, D::Error> {
        use serde::de::Error;
        let mut table = toml::Table::deserialize(d)?;
        let sim = SimParams::split_from(&mut table).map_err(D::Error::custom)?;
        let mut cfg = VoxelConfig::deserialize(toml::Value::Table(table)).map_err(D::Error::custom)?;
        cfg.sim = sim;
        Ok(cfg)
    }
}

impl Serialize for VoxelConfig {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::Error;
        struct Picture<'a>(&'a VoxelConfig);
        impl Serialize for Picture<'_> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
                VoxelConfig::serialize(self.0, s)
            }
        }
        let mut table = toml::Table::try_from(Picture(self)).map_err(S::Error::custom)?;
        table.extend(toml::Table::try_from(&self.sim).map_err(S::Error::custom)?);
        table.serialize(s)
    }
}

/// **The run's resolved parameter set** (package R1,
/// `design/ecology-parameter-registry-2026-09-28.md`): the config's `[world]`, then its
/// `[flora]` and `[fauna]` tables, then `--params`, then `--set`, every layer validated
/// against the parameter registry ([`SimParams::resolve`]). A fresh world's plants and
/// animals are built from it.
pub fn resolve_params(args: &Voxel, cfg: &VoxelConfig) -> Result<Params> {
    cfg.sim.resolve(args.params.as_deref(), &args.set)
}

/// [`params_of`] with a co-training run's ecology between the config's tables and `sets`
/// (`--params`, R1's genome layer): a genome is decoded over the tables, a resolved
/// parameter file replaces the plants and animals whole.
pub fn params_with(cfg: &VoxelConfig, file: &std::path::Path, sets: &[String]) -> Result<Params> {
    cfg.sim.resolve_with(file, sets)
}

/// [`resolve_params`] for a caller without a command line: `cfg`'s tables, then `sets`.
pub fn params_of(cfg: &VoxelConfig, sets: &[String]) -> Result<Params> {
    cfg.sim.resolve_sets(sets)
}


/// `lighting` in the config: the GPU renderer's lighting tier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lighting {
    /// The flat tier: the panel's and the cube's, and the code default.
    #[default]
    Flat,
    /// The lit tier: the desktop's.
    Lit,
}

/// `[light]` in the config: the lit tier's numbers. New numbers with stated defaults,
/// knobs rather than decisions (`design/handoffs/presentation-plan-2026-09-24.md`,
/// "No new look"). Colours are not here: the ambient's hue is the palette's sky.
///
/// ```toml
/// [light]
/// levels = 0          # 0: smooth light; 2 or more: rungs on an ambient ladder, floor to full
/// ambient_gain = 1.4  # what full ambient light multiplies a base colour by
/// ambient_floor = 0.2 # the lowest rung, as a fraction of full
/// ao = 0.5            # how far an AO crease line darkens the light
/// ambient_tint = 0.15 # how far the ambient leans toward the sky's hue
/// sun = [-1.0, 2.0, -1.0] # toward the sun: x right, y up, z into the scene
/// sun_tint = 0.18     # how far a sunlit face leans toward the palette's light, at N·L = 1
/// emission = true     # the dossiers' luminous parts glow (lit tier)
/// glow = 0.5          # one emitting voxel's light at its own 4³ cell, in full-ambient units
/// glow_reach = 3      # 4³ cells the light spreads before it is gone
/// bloom = 0.3         # the halo's strength (smooth: times 20 the blurred emission; blocky: at full step)
/// bloom_radius = 1.5  # voxel cells the halo reaches (twice the smooth blur's sigma)
/// bloom_style = "smooth" # or "blocky": L5's halo of whole voxel cells at three strengths
/// water_absorb = 0.1  # water absorption a voxel of path, in units of the deep water colour
/// water_reflect = 6.0  # the surface's Fresnel reflectance times this (physical is ~5 %)
/// water_ripple = 0.2   # how far a ripple tilts the quantised surface normal
/// water_hz = 12.0      # the water's animation speed, steps a second (its steps if not smooth)
/// water_smooth = true  # false: the water moves in water_hz steps instead of every frame
/// water_reflect_cells = 64 # cells a reflected ray is marched before it is sky
/// water_foam = 0.6     # foam along shores and where a fall meets a pool (0: none)
/// water_glint = 0.6    # sun-coloured highlight on sunlit ripple lines tilted sunward (0: none)
/// water_rain_rings = 0.5 # rings on open-sky water while it rains (0: none)
/// water_highlight = 0.5  # glints and rain rings: 0 palette colour .. 1 white
/// shadows = true       # the sun march (false: every face turned to the sun is sunlit)
/// reflections = true   # the water's reflection march (false: no reflection)
/// volumetric = false   # light shafts: sunlit and glowing air scatters light (package V)
/// volumetric_density = 0.03  # lit air's in-scatter a voxel of path, at the floor
/// volumetric_falloff = 12.0  # voxels up over which the air thins by e
/// volumetric_glow = 1.0      # the emitters' light in the air, times the glow volume
/// volumetric_rebake_s = 60.0 # the sun-visibility volume's rebake, at most once this often
/// day_sky = "warm"     # the lit sky by day: "warm", "mint" or "lilac" (interim variants)
/// star_bloom = 0.15    # the brightest stars' share into the bloom (0: none)
/// day_sky_brightness = 1.0  # the day sky and its clouds, times this (dimmer below 1)
/// candle_min = 0.25    # a glowcap lip's dimmest, of full (1: no candle, every lip full)
/// candle_period_s = 8.0  # seconds a glowcap's candle takes to wander, about
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightConfig {
    /// `0` (or `1`): the light is smooth, sky x AO x canopy, the glow and the sun applied as
    /// continuous values (the default; Wrysk, 2026-09-24). `2` or more: the ambient is
    /// snapped to a ladder of this many rungs, the floor and full included, and the sun
    /// adds one rung (package L's look at 4).
    pub levels: u32,
    /// What full ambient light (open sky, no occlusion, no canopy) multiplies a base
    /// colour by. The default puts an open top near the flat tier's lit top and a wall
    /// under open sky (sky visibility about two thirds) near the flat tier's front.
    pub ambient_gain: f32,
    /// The lowest rung, as a fraction of full: what a face that sees no sky still gets.
    pub ambient_floor: f32,
    /// How far an AO crease line (a band along a face edge whose neighbour occludes, or a
    /// corner square where only the diagonal does) darkens the light, `0..=1`.
    pub ao: f32,
    /// How far the ambient light leans toward the sky's hue, `0..=1`, at unit luminance.
    pub ambient_tint: f32,
    /// The direction **toward** the sun, in world axes: x to the right, y up, z into the
    /// scene (away from the camera). Normalised when used. It must sit in the front
    /// hemisphere (z < 0): light travels away from the camera, so the front faces the
    /// picture shows are lit and shadows fall back into the scene. A y at or below 0
    /// turns the sun off (every texel takes the shadow step).
    pub sun: [f32; 3],
    /// How far a sunlit texel leans toward the palette's light colour (`lightC`), scaled
    /// by N·L: the flat tier's top-face lean, gated by the sun. `0` is purely
    /// multiplicative (base × quantised light).
    pub sun_tint: f32,
    /// Whether the parts the species dossiers name as luminous emit
    /// (`crate::voxel::colours::plant_emission`, `vine_emission`): drawn unlit at full
    /// value in their dossier colour, and lighting the open cells around them. Off, the
    /// lit tier is exactly what it was without emission.
    pub emission: bool,
    /// One emitting voxel's light at its own glow cell (4³ voxels), in units of full
    /// ambient light, per channel of its emissive colour. Light from several emitters adds.
    pub glow: f32,
    /// How many glow cells the light spreads from its source through open terrain,
    /// falling off linearly to nothing one cell further.
    pub glow_reach: u32,
    /// The lit tier's bloom (`cubarium_gpu::bloom`): a halo around the emitters, added to
    /// what is behind; the emitters themselves stay crisp. Smooth, this times
    /// `cubarium_gpu::bloom::SMOOTH_GAIN` (20) times the emission blurred by a Gaussian (the
    /// blur spreads a small emitter's light thin, so the same number reads about as strong
    /// in both styles); blocky, whole voxel cells at 1, 2/3 and 1/3 of this times the
    /// emitter's colour. `0` turns it off. Needs `emission`.
    pub bloom: f32,
    /// How far the halo reaches, in voxel cells: smooth, twice the Gaussian's sigma (where
    /// it has fallen to about an eighth); blocky, whole cells from the emitter's own
    /// (Euclidean, rounded).
    pub bloom_radius: f32,
    /// `"smooth"`, blurred down a mip chain and added bilinear, or `"blocky"`, L5's
    /// whole-cell halo added nearest.
    pub bloom_style: BloomStyle,
    /// The lit tier's water (package W). Absorption per voxel of water path, in units of
    /// the palette's deep water colour: after `1 / water_absorb` voxels, what is left of
    /// the light from behind is that colour itself (per channel, Beer–Lambert). The same
    /// number sets how fast the in-scatter runs from the surface colour to the deep one.
    pub water_absorb: f32,
    /// What the surface's Fresnel reflectance is multiplied by (clamped to 1): physical
    /// Fresnel at this camera is about 5-7 %, too weak to read.
    pub water_reflect: f32,
    /// How far a ripple tilts the quantised surface normal (its horizontal part; 0 is a
    /// still mirror).
    pub water_ripple: f32,
    /// The water's animation rate, steps a second of sim time (ripples, streaks).
    pub water_hz: f32,
    /// The water moves every frame instead of in `water_hz` steps, at the same speed (the
    /// clock's step keeps its fraction). On by default: Wrysk, 2026-09-24, after the 12 Hz
    /// and smooth clips: "smooth waterfall is good".
    pub water_smooth: bool,
    /// Cells a reflected ray is marched before it counts as sky.
    pub water_reflect_cells: u32,
    /// How strongly foam covers the water (W checkpoint 2): a band along each shore
    /// (skin texels beside a solid) and a disc where a fall meets a pool, in the palette's
    /// surface colour off the light (as the falls' streaks are), only ever lightening. It
    /// breathes: its width and lace drift smoothly on the water's clock. `0` turns it off.
    pub water_foam: f32,
    /// How strongly a ripple line tilted toward the sun leans toward the palette's light
    /// (`lightC`), times the line's fade and a taper from its middle to its ends, in
    /// sunlit water only. The sun sits on the camera's side, so no true mirror glint
    /// lines up with this view. `0` turns it off.
    pub water_glint: f32,
    /// How strongly rain rings (a drop's ring growing and fading on water under open sky,
    /// while it rains) lean toward the palette's surface colour. `0` turns them off.
    pub water_rain_rings: f32,
    /// How far glints and rain rings lean from their palette colour (the light, the water
    /// surface; both near the lake's own cyan) toward white, 0 to 1. 0.5 by default
    /// (Wrysk, 2026-09-24, after 0 / 0.7 / 0.7-stronger clips: "just under middle. maybe 50%?").
    pub water_highlight: f32,
    /// The sun march (`voxel.frag`'s `sunReaches`). Off, its cost is gone: no cast
    /// shadows, and every face turned toward the sun takes the sunlit term.
    pub shadows: bool,
    /// The water's reflection march. Off, its cost is gone and the water reflects nothing.
    pub reflections: bool,
    /// Light shafts (package V): the lit tier's open air scatters the sun where the sun
    /// reaches it (the sun-visibility volume, `sink::gpu::sunvis`) and the emitters' glow,
    /// toward the camera. Off, the volume is never built and the shader never marches.
    pub volumetric: bool,
    /// How much light a voxel of view path through fully sunlit air at the world's floor
    /// scatters toward the camera, as a fraction of the palette's light colour.
    pub volumetric_density: f32,
    /// Voxels up over which the air's density falls by a factor of e: the air is thicker
    /// low down.
    pub volumetric_falloff: f32,
    /// What the emitters' glow (the lit tier's glow volume) scatters in the air, per unit of
    /// density, relative to full sun. `0` leaves the glow out.
    pub volumetric_glow: f32,
    /// Seconds (on the frame clock) between two bakes of the sun-visibility volume, at
    /// most: a rebake starts only when the sun has moved or the world has changed, and the
    /// new volume fades in over this long.
    pub volumetric_rebake_s: f32,
    /// The lit tier's daytime sky (WX2 checkpoint 1b, interim variants for Wrysk to choose
    /// between): `"warm"` (apricot horizon, rose-lavender zenith), `"mint"` (mint horizon,
    /// aqua-teal zenith) or `"lilac"` (a near-white lavender haze).
    pub day_sky: DaySky,
    /// How much of the brightest stars' light feeds the bloom, very subtly; `0` skips it.
    pub star_bloom: f32,
    /// The day sky's brightness: its gradient and its clouds by day, times this, hues kept
    /// (Wrysk, 2026-09-25: the red-dwarf day is too bright on some screens). `1` is the
    /// look as chosen; the terrain's light is not touched.
    pub day_sky_brightness: f32,
    /// The glowcap candle ([`candle`]): the dimmest a cap's lip gets, as a fraction of its
    /// full emission (unitless, `0 < candle_min <= 1`). At any moment the caps' brightness
    /// is spread reciprocally over `[candle_min, 1]` (density ∝ 1/b), so at 0.25 about
    /// 7.6 % of caps are within 10 % of full. `1` is no candle: every lip at full, the
    /// picture before it. It multiplies both the lip's drawn emission and its glow light.
    pub candle_min: f32,
    /// The glowcap candle's wander, seconds of the presenter's clock: each cap's
    /// brightness drifts on curves of about this period (each cap its own phase and pace).
    pub candle_period_s: f32,
}

/// `[light] day_sky`: [`LightConfig::day_sky`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DaySky {
    #[default]
    Warm,
    Mint,
    Lilac,
}

impl Default for LightConfig {
    fn default() -> LightConfig {
        LightConfig {
            levels: 0,
            ambient_gain: 1.4,
            ambient_floor: 0.2,
            ao: 0.5,
            ambient_tint: 0.15,
            sun: [-1.0, 2.0, -1.0],
            sun_tint: 0.18,
            emission: true,
            glow: 0.5,
            glow_reach: 3,
            bloom: BLOOM_DEFAULT,
            bloom_radius: BLOOM_RADIUS_DEFAULT,
            bloom_style: BloomStyle::Smooth,
            water_absorb: 0.1,
            water_reflect: 6.0,
            water_ripple: 0.2,
            water_hz: 12.0,
            water_smooth: true,
            water_reflect_cells: 64,
            water_foam: 0.6,
            water_glint: 0.6,
            water_rain_rings: 0.5,
            water_highlight: 0.5,
            shadows: true,
            reflections: true,
            volumetric: false,
            volumetric_density: 0.03,
            volumetric_falloff: 12.0,
            volumetric_glow: 1.0,
            volumetric_rebake_s: 60.0,
            day_sky: DaySky::Warm,
            star_bloom: cubarium_gpu::weather::STAR_BLOOM_DEFAULT,
            day_sky_brightness: 1.0,
            candle_min: 0.25,
            candle_period_s: 8.0,
        }
    }
}

/// `[light] bloom_style`: how the emitters' halo is made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BloomStyle {
    /// Blurred down a mip chain and added bilinear: a soft glow.
    #[default]
    Smooth,
    /// Whole voxel cells at three strengths, added nearest (package L5's).
    Blocky,
}

/// `[light] bloom` and `bloom_radius` by default.
pub const BLOOM_DEFAULT: f32 = 0.3;
pub const BLOOM_RADIUS_DEFAULT: f32 = 1.5;

/// `px_per_voxel = "auto"`, as the config holds it until start-up resolves it.
pub const PX_AUTO: u32 = 0;

/// `px_per_voxel`: a whole number, or `"auto"` ([`PX_AUTO`]).
mod px_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(px: &u32, s: S) -> Result<S::Ok, S::Error> {
        if *px == super::PX_AUTO {
            s.serialize_str("auto")
        } else {
            s.serialize_u32(*px)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Px {
            N(u32),
            S(String),
        }
        match Px::deserialize(d)? {
            Px::N(0) => Err(serde::de::Error::custom(
                "px_per_voxel must be at least 1, or \"auto\"",
            )),
            Px::N(n) => Ok(n),
            Px::S(s) if s == "auto" => Ok(super::PX_AUTO),
            Px::S(s) => Err(serde::de::Error::custom(format!(
                "px_per_voxel is a whole number or \"auto\", not {s:?}"
            ))),
        }
    }
}

/// The largest `px_per_voxel` whose strip fits `out` (`width × height` pixels): the ring
/// `width · s` wide, and the whole strip, `height · s + depth · rise`, uncropped — in
/// `out`'s height, or in `raster_height` where that is fixed (and smaller). At least 1.
pub fn auto_px_per_voxel(
    tilt_degrees: f64,
    raster_height: u16,
    world: &cubarium_voxel::Config,
    out: (u32, u32),
) -> u32 {
    let limit = match raster_height {
        0 => out.1,
        h => out.1.min(u32::from(h)),
    };
    let fits = |s: u32| {
        let rise = project::rise_for(tilt_degrees, s);
        let tall =
            u64::from(world.height) * u64::from(s) + u64::from(world.depth) * u64::from(rise);
        u64::from(world.width) * u64::from(s) <= u64::from(out.0) && tall <= u64::from(limit)
    };
    (1..=256).take_while(|&s| fits(s)).last().unwrap_or(1)
}

/// The smallest `px_per_voxel` whose strip **covers** `out` on its limiting axis: the
/// desktop window's, which the present pass then scales down to whatever size the window
/// is (`cubarium_gpu::present::WindowFit`), so the picture is supersampled rather than
/// stretched. It is [`auto_px_per_voxel`]'s fit when that already fills `out` on one
/// axis, else one more.
pub fn cover_px_per_voxel(
    tilt_degrees: f64,
    raster_height: u16,
    world: &cubarium_voxel::Config,
    out: (u32, u32),
) -> u32 {
    let fit = auto_px_per_voxel(tilt_degrees, raster_height, world, out);
    let limit = match raster_height {
        0 => out.1,
        h => out.1.min(u32::from(h)),
    };
    let rise = project::rise_for(tilt_degrees, fit);
    let tall = u64::from(world.height) * u64::from(fit) + u64::from(world.depth) * u64::from(rise);
    let fills = u64::from(world.width) * u64::from(fit) == u64::from(out.0)
        || tall == u64::from(limit);
    if fills { fit } else { fit + 1 }
}

/// The desktop's screen in pixels, for `px_per_voxel = "auto"`: `CUBARIUM_SCREEN=WxH`,
/// else Hyprland's focused monitor, else the first connected output `xrandr` reports.
/// `None` when none of them answers.
pub fn desktop_screen() -> Option<(u32, u32)> {
    desktop_screens(true)
}

/// The desktop's **largest** screen (by area), for the development window's
/// `px_per_voxel = "auto"`: the window can be put on any of them, and the raster is drawn
/// big enough for the largest and scaled down on the others. `CUBARIUM_SCREEN=WxH` still
/// names the one screen outright.
pub fn largest_desktop_screen() -> Option<(u32, u32)> {
    desktop_screens(false)
}

/// The focused screen (`focused`), or the largest, from `CUBARIUM_SCREEN`, Hyprland or
/// `xrandr`, in that order.
fn desktop_screens(focused: bool) -> Option<(u32, u32)> {
    let wxh = |t: &str| -> Option<(u32, u32)> {
        let (w, h) = t.split_once('x')?;
        Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
    };
    if let Ok(v) = std::env::var("CUBARIUM_SCREEN") {
        return wxh(&v);
    }
    let largest = |sizes: Vec<(u32, u32)>| {
        sizes
            .into_iter()
            .max_by_key(|&(w, h)| u64::from(w) * u64::from(h))
    };
    let run = |cmd: &str, args: &[&str]| -> Option<String> {
        let out = std::process::Command::new(cmd).args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    if let Some(json) = run("hyprctl", &["monitors", "-j"])
        && let Ok(serde_json::Value::Array(monitors)) = serde_json::from_str(&json)
    {
        let size = |m: &serde_json::Value| -> Option<(u32, u32)> {
            let scale = m["scale"].as_f64().unwrap_or(1.0).max(0.1);
            Some((
                (m["width"].as_f64()? / scale) as u32,
                (m["height"].as_f64()? / scale) as u32,
            ))
        };
        let found = if focused {
            let f = monitors.iter().find(|m| m["focused"].as_bool() == Some(true));
            f.or(monitors.first()).and_then(size)
        } else {
            largest(monitors.iter().filter_map(size).collect())
        };
        if let Some(size) = found {
            return Some(size);
        }
    }
    let text = run("xrandr", &["--current"])?;
    let mut sizes = text
        .lines()
        .filter(|l| l.contains(" connected"))
        .filter_map(|l| l.split_whitespace().find_map(|w| wxh(w.split('+').next()?)));
    if focused { sizes.next() } else { largest(sizes.collect()) }
}

/// `organisms` in the config: which drawing of plants and animals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrganismLook {
    /// The baked voxel models.
    #[default]
    Models,
    /// The dev-mode glyphs (`stand::parts_of`, `animal::cells_of`).
    Glyphs,
}

/// The model library a run with `cfg` draws with, or `None` for the glyphs: asked for,
/// or asked for models that are not there, which is said on stderr.
pub fn load_models(cfg: &VoxelConfig) -> Option<std::sync::Arc<model::ModelLibrary>> {
    if cfg.organisms == OrganismLook::Glyphs {
        eprintln!("cubarium voxel: organisms = \"glyphs\": drawing the dev-mode glyphs");
        return None;
    }
    match model::ModelLibrary::load(&cfg.models_dir, cfg.sim.world.voxel_m) {
        Ok(lib) => {
            eprintln!(
                "cubarium voxel: {} voxel models from {}/{}",
                lib.len(),
                cfg.models_dir.display(),
                model::voxel_dir(cfg.sim.world.voxel_m)
            );
            Some(std::sync::Arc::new(lib))
        }
        Err(e) => {
            eprintln!("cubarium voxel: no voxel models ({e:#}); drawing the glyphs");
            None
        }
    }
}

/// Load a [`VoxelConfig`] from TOML, its `base` chain merged under it
/// ([`cubarium_session::params::config_table`]).
pub fn load_config(path: &Path) -> Result<VoxelConfig> {
    let table = config_table(path)?;
    table
        .try_into()
        .with_context(|| format!("parsing the voxel config {}", path.display()))
}

// --- The run -------------------------------------------------------------------------


/// Read `[world].seed` from a TOML config file if explicitly declared there.
fn cfg_seed_from_file(config_path: &Option<PathBuf>) -> Option<u64> {
    let path = config_path.as_ref()?;
    let text = std::fs::read_to_string(path).ok()?;
    let val: toml::Value = toml::from_str(&text).ok()?;
    val.get("world")?
        .get("seed")?
        .as_integer()
        .map(|s| s as u64)
}


/// The running snapshot, off the loop.
///
/// **Clone on the loop, encode and write on a thread.** `World::save` clones the world
/// and then postcard-encodes the clone, so the clone is a strict subset of what the loop
/// used to pay: on the desk (release, the panel's 160x72x24 world, 4.7 MB) the clone is
/// 0.1–0.3 ms against 0.6–1.4 ms for the encode and 0.4–0.5 ms for the write, and the
/// board is 8–10x slower than the desk for this loop. One write in flight at a time: a
/// snapshot due while the last is still writing is skipped, not queued, and the exit save
/// ([`SnapshotWriter::save_now`]) waits for the one in flight before it writes its own.
struct SnapshotWriter {
    dir: PathBuf,
    in_flight: Option<JoinHandle<()>>,
    /// Where a thread spawned by the pinned loop goes instead of the loop's own core.
    placement: Option<placement::Placement>,
}

impl SnapshotWriter {
    fn new(dir: PathBuf) -> SnapshotWriter {
        SnapshotWriter {
            dir,
            in_flight: None,
            placement: None,
        }
    }

    /// Start writing `world` in the background, unless the last write is still going.
    /// Returns whether it started.
    fn save_in_background(&mut self, world: &World, flora: &Flora, fauna: &Fauna) -> bool {
        let (world, flora, fauna) = (world.clone(), flora.clone(), fauna.clone());
        let dir = self.dir.clone();
        self.spawn(move || {
            save_voxel_snapshot(&dir, &world, Some((&flora, &fauna)));
        })
    }

    /// Whether a write is still in flight.
    fn busy(&self) -> bool {
        self.in_flight.as_ref().is_some_and(|h| !h.is_finished())
    }

    /// The on-demand save: the whole world into `dir` (made if missing), off the loop.
    /// A request while the last one is still writing is refused and said so, not queued.
    fn save_whole_to(
        &mut self,
        dir: PathBuf,
        world: &World,
        flora: &Flora,
        fauna: &Fauna,
    ) -> bool {
        if self.busy() {
            eprintln!(
                "cubarium voxel: the last save is still writing; not saving to {} — ask again",
                dir.display()
            );
            return false;
        }
        eprintln!(
            "cubarium voxel: saving the whole world at tick {} to {}/",
            world.tick(),
            dir.display()
        );
        let (world, flora, fauna) = (world.clone(), flora.clone(), fauna.clone());
        self.spawn(move || save_whole_world(&dir, &world, &flora, &fauna))
    }

    /// Run `job` as the one write in flight. The seam the tests hold a write open with.
    fn spawn(&mut self, job: impl FnOnce() + Send + 'static) -> bool {
        if self.in_flight.as_ref().is_some_and(|h| !h.is_finished()) {
            eprintln!("cubarium voxel: the last snapshot is still writing; skipping this one");
            return false;
        }
        self.wait();
        let placement = self.placement.clone();
        self.in_flight = Some(std::thread::spawn(move || {
            if let Some(p) = placement {
                p.leave_loop_core();
            }
            job();
        }));
        true
    }

    /// Wait for the write in flight, if there is one.
    fn wait(&mut self) {
        if let Some(h) = self.in_flight.take() {
            let _ = h.join();
        }
    }

    /// The exit save: synchronous, after whatever was already writing.
    fn save_now(&mut self, world: &World, flora: &Flora, fauna: &Fauna) {
        self.wait();
        save_voxel_snapshot(&self.dir, world, Some((flora, fauna)));
    }
}

/// Resume from a file/directory or create a fresh world according to CLI and config
/// ([`cubarium_session::create::load_or_create_world`], the one copy).
///
/// A **fresh generated** world that is going to be seeded goes through the founding loop
/// ([`found`]) and comes back with its layers, already seeded; every other
/// world comes back bare, `None`, for the caller to seed.
fn load_or_create_world(
    args: &Voxel,
    cfg: &VoxelConfig,
) -> Result<(World, String, bool, Option<StartLayers>)> {
    let params = resolve_params(args, cfg)?;
    cubarium_session::create::load_or_create_world(
        &params,
        &StartSpec {
            load: args.load.as_deref(),
            scene: match args.scene {
                VoxelSceneArg::Authored => Scene::Authored,
                VoxelSceneArg::Generated => Scene::Generated,
            },
            seed: args.seed.or_else(|| cfg_seed_from_file(&args.config)),
            empty: args.empty,
            founder_counts: args.founder_counts(&params.fauna),
        },
    )
}

/// Run `cubarium voxel`.
pub fn run_voxel(args: &Voxel, stop: &AtomicBool) -> Result<()> {
    let mut cfg = match &args.config {
        Some(path) => load_config(path)?,
        None => VoxelConfig::default(),
    };
    args.validate()?;
    // Before the founding: until it is installed, SIGUSR1's default action ends the run.
    save::install_save_signal();
    if args.sink == VoxelSinkArg::Png && args.seconds <= 0.0 {
        bail!("`--sink png` needs `--seconds N`, or it would capture until interrupted");
    }
    // `World::new` and `World::empty` panic on a config no world can be built from —
    // building one from nonsense is a programming error. A config *file* is input, so the
    // boundary is here: a bad file is reported, not a backtrace.
    cfg.sim.world.validate().with_context(|| match &args.config {
        Some(path) => format!("the `[world]` in {}", path.display()),
        None => "the built-in world defaults".to_string(),
    })?;
    let params = resolve_params(args, &cfg)?;
    cfg.sim.world = params.world.clone();
    if args.textures {
        cfg.textures = true;
    }
    if cfg.px_per_voxel == PX_AUTO {
        // Once, here: the renderer's projection is fixed for the life of the process.
        let target = (args.sink == VoxelSinkArg::Gpu)
            .then(|| args.gpu_target.unwrap_or_else(GpuTargetKind::detect));
        let panel = target == Some(GpuTargetKind::Shim);
        // The development window fills itself at any scale (`WindowFit`): its raster
        // covers the largest screen and is scaled down to the window, so it is never
        // stretched up. Everything else keeps the largest strip that fits.
        let window = target == Some(GpuTargetKind::Window);
        let (out, what) = if panel {
            let (w, h) = cubarium_gpu::target::SHIM_PANEL;
            ((h, w), "fits the panel")
        } else if window {
            match largest_desktop_screen() {
                Some(size) => (size, "covers the largest screen"),
                None => ((1920, 1080), "covers an assumed 1920x1080 screen (none detected)"),
            }
        } else {
            match desktop_screen() {
                Some(size) => (size, "fits the screen"),
                None => ((1920, 1080), "fits an assumed 1920x1080 screen (none detected)"),
            }
        };
        cfg.px_per_voxel = if window {
            cover_px_per_voxel(cfg.tilt_degrees, cfg.raster_height, &cfg.sim.world, out)
        } else {
            auto_px_per_voxel(cfg.tilt_degrees, cfg.raster_height, &cfg.sim.world, out)
        };
        eprintln!(
            "cubarium voxel: px_per_voxel auto -> {} ({what}, {}x{})",
            cfg.px_per_voxel, out.0, out.1
        );
    }

    // The regular display remains its own coupled world. `--arena` is an explicit
    // development mode which instead owns a frozen P1 sensing layout and a controller
    // installed through fauna's ordinary controller boundary.
    // `0` is the host's "decide for me", and the decision is the voxel crate's:
    // `default_threads` is every core the OS reports but one, so the thread that
    // records, submits and presents the frame is not competing with the tick for the
    // last core. `SimConfig::default()` takes every core instead and is not what the
    // panel wants.
    let sim_config = cfg.sim.sim_config();
    let speed = args.speed.clamp(MIN_SPEED, MAX_SPEED);
    // **The output is opened before the world exists.** Founding one is minutes of work
    // on the board — a seed gate of up to `LAKE_SEED_TRIES` candidates, each settled,
    // then the habitat's own settle — and the shim's daemon blanks the panel three
    // seconds after the last flip, so a process that draws nothing until its world is
    // ready is a process the panel shows as dead. The sink is opened on the geometry the
    // config file already fixes, and a founding frame is presented at the display's rate
    // while the world is built on another thread.
    //
    // The sensing arena is not founded: it builds in milliseconds and has no wait to
    // cover.
    let mut out: Option<Out> = match args.arena {
        Some(_) => None,
        None => {
            let proj = Projection::new(
                cfg.tilt_degrees,
                cfg.px_per_voxel,
                cfg.raster_height,
                &cfg.sim.world,
            )?;
            let mut out = open_out(args, &cfg, proj, speed)?;
            // The GPU draws whatever was last staged, and nothing has been staged yet:
            // an empty world of this config makes the founding frame sky over nothing
            // rather than whatever the staging buffer happened to hold.
            out.stage_empty(&cfg.sim.world);
            Some(out)
        }
    };

    let (mut sim, scene_label) = match args.arena {
        Some(arena) => {
            let (founder, driver) = sensing_driver(args, arena)?;
            // On the run's own parameter set, as a founded world would be.
            let grid = ArenaGrid::Standard;
            let mut arena = Arena::build(
                founder,
                args.arena_seed,
                grid,
                LayerConfig::new(&params, grid.voxel_m()),
            );
            let senses = arena.prepare_senses();
            let animal_id = arena
                .animal_id
                .ok_or_else(|| anyhow::anyhow!("sensing arena did not place its founder"))?;
            let controller = driver.fresh(arena.fauna.config().teachers);
            let controller: Box<dyn Controller> = if args.arena_diagnostics {
                Box::new(DiagnosticController::new(founder, controller))
            } else {
                controller
            };
            anyhow::ensure!(
                arena.fauna.set_controller(animal_id, controller),
                "sensing arena founder {animal_id} disappeared before its controller installed"
            );
            eprintln!(
                "cubarium voxel: sensing arena {} seed {} with {}{}",
                founder.name(),
                args.arena_seed,
                driver.name(),
                if args.arena_diagnostics {
                    " (controller diagnostics on stderr)"
                } else {
                    ""
                },
            );
            (
                arena.into_sim_prepared(SimConfig::with_threads(1), senses),
                format!("sensing {} arena seed {}", founder.name(), args.arena_seed),
            )
        }
        None => found_while_presenting(
            out.as_mut().expect("the ordinary run opens its output first"),
            args.fps,
            stop,
            || {
            let (world, scene_label, resumed, founded) = load_or_create_world(args, &cfg)?;
            let recipes = founder_recipes(args)?;
            let introductions = args.introductions()?;
            // The one copy of seeding, driving and readying a world: the session crate's.
            let (world, flora, fauna) = cubarium_session::create::populate(
                world,
                resumed,
                founded,
                &params,
                &Populate {
                    recipes: &recipes,
                    introductions: &introductions,
                    empty: args.empty,
                    founder_counts: args.founder_counts(&params.fauna),
                    no_egg_bank: args.no_egg_bank,
                },
            )?;
            Ok((
                cubarium_session::create::ready_sim(world, flora, fauna, sim_config),
                scene_label,
            ))
            },
        )?,
    };

    // The display's water fidelity is its config's (`water_every`, live by default); a
    // coarse one is said, since it changes how fast water spreads on screen.
    cubarium_session::create::set_water_every(&mut sim, cfg.sim.water_every);

    // The world that was built is the world the config drew, unless a snapshot was
    // resumed that describes a different one — then the founding output is on the wrong
    // geometry and is reopened on the world that actually exists.
    let proj = Projection::new(
        cfg.tilt_degrees,
        cfg.px_per_voxel,
        cfg.raster_height,
        sim.world().config(),
    )?;
    let mut out = match out {
        Some(out) if same_extent(&cfg.sim.world, sim.world().config()) => out,
        Some(old) => {
            let c = sim.world().config();
            eprintln!(
                "cubarium voxel: the world in hand is {}x{}x{}, not the {}x{}x{} the \
                 config draws; reopening the output on it",
                c.width,
                c.height,
                c.depth,
                cfg.sim.world.width,
                cfg.sim.world.height,
                cfg.sim.world.depth,
            );
            drop(old);
            open_out(args, &cfg, proj, speed)?
        }
        None => open_out(args, &cfg, proj, speed)?,
    };
    // The world exists: the founding sky goes back to the palette's.
    out.founded()?;

    eprintln!(
        "cubarium voxel: tick schedule on {} thread(s)",
        sim.config().threads
    );

    let c = sim.world().config().clone();
    eprintln!(
        "cubarium voxel: {} world {}x{}x{} at {} m/voxel -> ring:{}x{} \
         ({} px/voxel, tilt {:.0} deg, depth step {} px)",
        scene_label,
        c.width,
        c.height,
        c.depth,
        c.voxel_m,
        proj.raster_w,
        proj.raster_h,
        proj.s,
        cfg.tilt_degrees,
        proj.rise,
    );
    // Under a closed budget the outlet is the return flow, so a closed world starts with
    // it open; `o` still toggles it.
    cubarium_session::create::open_closed_outlet(&mut sim);
    // `--weather-force MODE`: a development capture's rain, starting now.
    if let Some(mode) = &args.weather_force {
        use cubarium_voxel::weather::RainMode;
        let mode = match mode.as_str() {
            "drizzle" => RainMode::Drizzle,
            "shower" => RainMode::Shower,
            "downpour" => RainMode::Downpour,
            "clear" => RainMode::Clear,
            other => bail!("--weather-force is drizzle, shower, downpour or clear, not {other:?}"),
        };
        let started = sim.world_mut().force_rain(mode);
        eprintln!("cubarium voxel: --weather-force {mode:?} ({})", if started { "started" } else { "nothing to rain" });
    }
    // Off the loop: the run starts now, and this line arrives when it arrives.
    let _probe = report_water_cycle(sim.world(), sim.flora(), sim.config().threads);
    eprintln!("cubarium voxel: stdin commands — {COMMANDS}");

    let (command_tx, commands) = mpsc::channel();
    spawn_stdin_reader(command_tx);
    // `--serve` / `serve`: off by default, and then nothing below costs anything.
    // A `--serve` that cannot bind ends the run; a config's `serve` that cannot (a second
    // run on the same config, say) is said and the run goes on without one.
    let mut observatory = match (&args.serve, &cfg.serve) {
        (Some(addr), _) => Some(Observatory::start(addr, &cfg, &scene_label)?),
        (None, Some(addr)) => match Observatory::start(addr, &cfg, &scene_label) {
            Ok(o) => Some(o),
            Err(e) => {
                eprintln!("cubarium voxel: no observatory — {e:#}");
                None
            }
        },
        (None, None) => None,
    };

    let limit = (args.seconds > 0.0).then(|| Duration::from_secs_f64(args.seconds));
    let start = Instant::now();
    let mut clock = Clock::with_fps(start, args.fps);
    let (mut ticks, mut frames) = (0u64, 0u64);
    // The running report's own counters: what happened since the last one was printed.
    let (mut since, mut since_ticks, mut since_frames) = (start, 0u64, 0u64);
    // The presented count is the target's own running total, so the interval's share is
    // what it has grown by.
    let mut since_presented = out.presented().map_or(0, |(shown, _)| shown);
    // Where this thread's milliseconds go, printed with the same report. The founding
    // frames are behind us, so the sink's counters start from where they are.
    let mut budget = Budget::default();
    budget.mark(out.draw_split());
    let mut ctl = Control::new(speed, proj);
    let mut pacer = Pacer::default();
    // Whether the world or the plant layer has moved since the last frame drawn. The CPU
    // presenter re-reads the world every frame and does not care; the GPU packs one
    // texture per *tick*, so it needs to be told.
    let mut moved = true;

    let mut snapshots: Option<SnapshotWriter> = args.load.as_ref().and_then(|p| {
        if p.is_dir() || (p.extension().is_none() && !p.is_file()) {
            Some(SnapshotWriter::new(p.clone()))
        } else {
            None
        }
    });

    // Last, so every thread this process has — the presenter, the tick pool, the stdin
    // reader, the start-up probe — already exists to be moved off the loop's core.
    // Opt-in (`--pin-loop`, the Tachyon unit): no other machine's scheduling is touched.
    let placed = if args.pin_loop {
        placement::place_loop()
    } else {
        None
    };
    if let Some(s) = snapshots.as_mut() {
        s.placement = placed.clone();
    }
    // `save [DIR]`, SIGUSR1 and Ctrl+S: the whole world on demand, with or without a
    // state directory. Its own writer, so a running snapshot never refuses it.
    let mut on_demand = SnapshotWriter::new(PathBuf::from(save::SAVES_DIR));
    on_demand.placement = placed;
    // The resolved parameter set, beside what this run writes: its state directory, or
    // the PNG captures. A run that writes nothing has nowhere to put it.
    let params_dir = snapshots
        .as_ref()
        .map(|s| s.dir.clone())
        .or_else(|| (args.sink == VoxelSinkArg::Png).then(|| args.out.clone()));
    if let Some(dir) = params_dir {
        match params.write_beside(&dir) {
            Ok(path) => eprintln!("cubarium voxel: parameters in {}", path.display()),
            Err(e) => eprintln!("cubarium voxel: could not write the parameter set: {e}"),
        }
    }

    while !ctl.quit {
        let now = Instant::now();
        if let Some(l) = limit
            && clock.elapsed(now) >= l
        {
            break;
        }
        if out.should_quit() || stop.load(Ordering::Relaxed) {
            break;
        }
        // The running report, so the panel's rate can be read without stopping it.
        if now.duration_since(since) >= SUMMARY_INTERVAL {
            let interval = now.duration_since(since).as_secs_f64();
            eprintln!(
                "cubarium voxel: since the last report — {}",
                run_line(
                    &sim,
                    since_ticks,
                    since_frames,
                    interval,
                    out.presented().map(|(shown, _)| shown - since_presented),
                )
            );
            // And what a present cost over that same interval, so a reading of the panel's
            // rate says why it is what it is rather than leaving it to be inferred.
            if let Some(line) = out.presenter_line(interval) {
                eprintln!("cubarium voxel: {line}");
            }
            eprintln!(
                "cubarium voxel: {}",
                budget.line(interval, since_ticks, since_frames, out.draw_split())
            );
            since = now;
            since_ticks = 0;
            since_frames = 0;
            since_presented = out.presented().map_or(0, |(shown, _)| shown);
        }

        while let Ok(line) = commands.try_recv() {
            let at = Instant::now();
            sim.with_layers_mut(|world, flora, fauna| ctl.handle(world, flora, fauna, &line));
            budget.commands_ns += at.elapsed().as_nanos() as u64;
            // A command may have moved a cell, seeded a stand or loaded a world; which
            // ones did is the command's business, and one extra pack is cheaper than a
            // rule here that has to be kept in step with `Control::handle`.
            moved = true;
            if let Some(o) = observatory.as_mut() {
                o.observer.request();
            }
        }
        // The observatory's readers: the same console path, at the same point.
        if let Some(o) = observatory.as_mut() {
            while let Ok(command) = o.commands.try_recv() {
                let line = console_line(command);
                sim.with_layers_mut(|world, flora, fauna| ctl.handle(world, flora, fauna, &line));
                o.observer.request();
            }
        }
        if ctl.quit {
            break;
        }
        let asked = ctl.save_to.take().or_else(|| {
            let now = std::time::SystemTime::now();
            (save::take_save_signal() || out.take_save_request())
                .then(|| save::default_save_dir(now, sim.world().tick()))
        });
        if let Some(dir) = asked {
            let (world, flora, fauna) = sim.layers();
            on_demand.save_whole_to(dir, world, flora, fauna);
        }

        match clock.next_step(now) {
            Step::Tick => {
                // The world ticks this clock tick owes: the speed's, or a pending `s`.
                for _ in 0..pacer.due(&mut ctl) {
                    let (at, before) = (Instant::now(), Phases::now());
                    sim.step();
                    let ns = at.elapsed().as_nanos() as u64;
                    budget.tick(ns, before);
                    if let Some(o) = observatory.as_mut() {
                        o.observer.after_tick(sim.world(), sim.fauna(), ns);
                    }
                    ticks += 1;
                    since_ticks += 1;
                    moved = true;
                }
                out.observe_tick(sim.world().tick());
                if let Some(o) = observatory.as_mut() {
                    o.maybe_publish(&sim, &ctl);
                }
                if let Some(s) = snapshots.as_mut()
                    && ticks > 0
                    && ticks % 1200 == 0
                {
                    let (world, flora, fauna) = sim.layers();
                    s.save_in_background(world, flora, fauna);
                }
            }
            Step::Render { f } => {
                let (world, flora, fauna) = sim.layers();
                // A pack the renderer refused is still owed: the flag goes straight back
                // up rather than being lost with the tick that set it.
                let at = Instant::now();
                moved = out.render(world, flora, fauna, std::mem::take(&mut moved), f)?;
                budget.frame(at.elapsed().as_nanos() as u64);
                if let Some(o) = observatory.as_mut() {
                    o.observer.after_frame();
                }
                frames += 1;
                since_frames += 1;
            }
            Step::Sleep(d) => std::thread::sleep(d),
            Step::Lagged { behind, log } => {
                if log {
                    eprintln!(
                        "cubarium voxel: behind by {:.0} ms; dropping render work",
                        behind.as_secs_f64() * 1e3
                    );
                }
            }
            Step::Paused { gap } => {
                eprintln!(
                    "cubarium voxel: clock re-based after a {:.1} s pause",
                    gap.as_secs_f64()
                );
            }
        }
    }

    out.finish()?;
    on_demand.wait();
    if let Some(s) = snapshots.as_mut() {
        let (world, flora, fauna) = sim.layers();
        s.save_now(world, flora, fauna);
    }
    let elapsed = clock.elapsed(Instant::now()).as_secs_f64();
    let shown = out.presented().map(|(shown, _)| shown);
    eprintln!(
        "cubarium voxel: {}",
        run_line(&sim, ticks, frames, elapsed, shown)
    );
    if let Some(line) = out.presenter_line(elapsed) {
        eprintln!("cubarium voxel: {line}");
    }
    eprintln!(
        "cubarium voxel: {}",
        budget.line(elapsed, ticks, frames, out.draw_split())
    );
    let (_, _, fauna) = sim.layers();
    let av = fauna.view();
    eprintln!(
        "cubarium voxel: {} animals ({} born, {} dead, {} bites, {} steps), \
         fauna residual {:.3e} organic, {:.3e} mineral, {:.3e} energy",
        av.animals.len(),
        av.ledger.born,
        av.ledger.deaths,
        av.ledger.bites,
        av.ledger.steps,
        av.organic() - av.ledger.expected_organic(),
        av.mineral() - av.ledger.expected_mineral(),
        av.energy() - av.ledger.expected_energy(),
    );
    // The founder census: who of the sensed lineages is still standing, and what each
    // lineage actually got into its tissue. `assimilated` is the placed figure — what
    // became body and reserve — and not the bite's gross organic matter.
    let census: Vec<String> = Founder::ALL
        .into_iter()
        .map(|f| {
            let alive = av.animals.iter().filter(|a| a.founder == Some(f)).count();
            format!(
                "{} {alive} alive, {} bites, {:.3e} assimilated",
                f.name(),
                av.ledger.bites_by_founder[f.index()],
                av.ledger.assimilated_by_founder[f.index()],
            )
        })
        .collect();
    eprintln!("cubarium voxel: founders — {}", census.join("; "));
    Ok(())
}

// The trained centres, their installation and each lineage's recipe live in the session
// crate (`cubarium_session::founders`), shared with the Godot session; re-exported here
// for the examples.
pub use cubarium_session::founders::{
    install_default_founders, install_founders_with, policy_args, policy_founders,
};

/// Each lineage's driver for the ambient run, from the command line: `--founder-policy`
/// first, then `--founder-heuristic`, otherwise the built-in centre
/// ([`cubarium_session::founders::founder_recipes`]).
fn founder_recipes(args: &Voxel) -> Result<Vec<FounderRecipe>> {
    cubarium_session::founders::founder_recipes(&args.founder_policies()?, &args.founder_heuristics()?)
}

/// Select the exact controller body a sensing-arena run installs. Policy files validate
/// their own founder and digest before handing out an [`EpisodeDriver`], so this host
/// only has to reject a user spelling that asks a blind arena to run a browser policy.
fn sensing_driver(
    args: &Voxel,
    arena: VoxelArenaArg,
) -> Result<(Founder, cubarium_search::es::voxel::EpisodeDriver)> {
    use cubarium_search::es::voxel::VoxelControl;

    let founder = match arena {
        VoxelArenaArg::Blind => Founder::Blind,
        VoxelArenaArg::Browser => Founder::Browser,
    };
    let driver = match args.controller {
        VoxelControllerArg::Heuristic => EpisodeDriver::control(VoxelControl::Heuristic, founder),
        VoxelControllerArg::NoIntake => EpisodeDriver::control(VoxelControl::NoIntake, founder),
        VoxelControllerArg::StationaryFeeding => {
            EpisodeDriver::control(VoxelControl::StationaryFeeding, founder)
        }
        VoxelControllerArg::Gru => {
            let path = args
                .policy
                .as_ref()
                .expect("Voxel::validate required --policy");
            let policy = VoxelPolicyFile::load(path).map_err(anyhow::Error::msg)?;
            let policy_founder = policy.founder().map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                policy_founder == founder,
                "{} policy cannot run the {} arena",
                policy_founder.name(),
                founder.name(),
            );
            policy.driver().map_err(anyhow::Error::msg)?
        }
    };
    Ok((founder, driver))
}

/// An opt-in stderr observer placed directly around the controller seam. It receives
/// exactly the same packet as its wrapped controller, emits one line only when the
/// founder's cadence calls `drive`, and never inspects world state or the target layout.
struct DiagnosticController {
    founder: Founder,
    inner: Box<dyn Controller>,
    samples: u64,
}

impl DiagnosticController {
    fn new(founder: Founder, inner: Box<dyn Controller>) -> DiagnosticController {
        DiagnosticController {
            founder,
            inner,
            samples: 0,
        }
    }
}

impl Controller for DiagnosticController {
    fn drive(&mut self, observation: &[f64]) -> Response {
        let response = self.inner.drive(observation);
        self.samples += 1;
        let nonzero = observation.iter().filter(|v| **v != 0.0).count();
        eprintln!(
            "cubarium sensing: {} sample {} inputs={} nonzero={} response={response:?}",
            self.founder.name(),
            self.samples,
            observation.len(),
            nonzero,
        );
        response
    }

    fn reset(&mut self) {
        self.samples = 0;
        self.inner.reset();
    }
}

/// Where the run's frames come from: the CPU presenter into a [`FrameSink`], or the
/// GPU's slab-walk shader straight onto a target.
///
/// The two cannot be one `dyn FrameSink`, and the reason is the whole point of the GPU
/// path: a `FrameSink` consumes *pixels*, and `--sink gpu` exists so that nobody
/// rasterises the strip on the CPU. `FrameSink`'s `observe_world` hook is no help here
/// either — it carries the ring's `RenderView`, and this world is a voxel strip. So the
/// loop names the two, and everything they share is these four methods.
enum Out {
    Cpu {
        sink: Box<dyn FrameSink>,
        presenter: VoxelPresenter,
        canvas: Canvas,
        raster: cube_proto::Raster,
    },
    Gpu(Box<VoxelGpuSink>),
}

impl Out {
    fn cpu(
        sink: Box<dyn FrameSink>,
        cfg: VoxelConfig,
        proj: Projection,
        models: Option<std::sync::Arc<model::ModelLibrary>>,
    ) -> Out {
        let topology = Topology::Ring {
            w: proj.raster_w,
            h: proj.raster_h,
        };
        Out::Cpu {
            sink,
            presenter: VoxelPresenter::new(cfg, proj).with_models(models),
            canvas: Canvas::new(topology, Scale::ONE),
            raster: cube_proto::Raster::black(proj.raster_w, proj.raster_h),
        }
    }

    /// Draw one frame. `moved` says the world has changed since the last one, which is
    /// what the GPU path packs a new voxel texture on; the CPU path reads the world
    /// afresh every frame and ignores it.
    ///
    /// Returns whether the world is **still** owed a pack: the GPU path refuses one while
    /// it is reading every staging buffer it has, and the caller must keep its flag rather
    /// than lose the tick's world.
    fn render(
        &mut self,
        world: &World,
        flora: &Flora,
        fauna: &Fauna,
        moved: bool,
        fraction: f64,
    ) -> Result<bool> {
        match self {
            Out::Cpu {
                sink,
                presenter,
                canvas,
                raster,
            } => {
                presenter.draw_with_fauna(&world.view(), flora.view(), Some(fauna.view()), canvas);
                canvas.encode_raster(raster);
                sink.submit(Output::Ring(raster))?;
                Ok(false)
            }
            Out::Gpu(gpu) => {
                let owed = moved && !gpu.stage_world(world, flora, fauna);
                gpu.set_clock(world.tick(), fraction);
                gpu.render()?;
                Ok(owed)
            }
        }
    }

    /// What the panel has been shown, where the target counts it separately from what the
    /// loop drew — the presenting thread does.
    fn presented(&self) -> Option<(u64, u64)> {
        match self {
            Out::Cpu { .. } => None,
            Out::Gpu(gpu) => gpu.presented(),
        }
    }

    /// The sink's own cumulative milliseconds: packing a tick, and drawing a frame.
    /// `None` from a sink that does not separate them.
    fn draw_split(&self) -> Option<(f64, f64)> {
        match self {
            Out::Cpu { .. } => None,
            Out::Gpu(gpu) => Some(gpu.draw_split()),
        }
    }

    /// Where the presenting thread's last `seconds` went, for the report. `None` when
    /// nothing presents on another thread.
    fn presenter_line(&mut self, seconds: f64) -> Option<String> {
        match self {
            Out::Cpu { .. } => None,
            Out::Gpu(gpu) => gpu.presenter_line(seconds),
        }
    }

    /// Draw the founding frame — the sky alone, pulsing — at `seconds` into the run.
    ///
    /// This is what the panel shows while the world is being founded. It carries no text
    /// and no reading of any kind: the normal display holds no analytical UI, and what
    /// the wait should look like is Wrysk's to decide. Until he does, this is the
    /// placeholder.
    fn founding(&mut self, seconds: f64) -> Result<()> {
        match self {
            Out::Cpu {
                sink,
                canvas,
                raster,
                ..
            } => {
                present::founding_sky(canvas, seconds);
                canvas.encode_raster(raster);
                sink.submit(Output::Ring(raster))
            }
            Out::Gpu(gpu) => gpu.founding(seconds),
        }
    }

    /// The world exists: undo whatever the founding frame changed.
    fn founded(&mut self) -> Result<()> {
        match self {
            Out::Cpu { .. } => Ok(()),
            Out::Gpu(gpu) => gpu.founded(),
        }
    }

    /// Stage an empty world of `cfg`, so the GPU path has defined contents to draw before
    /// the real world exists. The CPU path reads the world it is handed every frame and
    /// has nothing to stage.
    fn stage_empty(&mut self, cfg: &cubarium_voxel::Config) {
        if let Out::Gpu(gpu) = self {
            let world = World::empty(cfg.clone());
            let flora = Flora::new(FloraConfig::for_voxel_size(world.config().voxel_m));
            let fauna = Fauna::new(FaunaConfig::default());
            // Before the first frame nothing is in flight, so this is never refused.
            gpu.stage_world(&world, &flora, &fauna);
        }
    }

    fn observe_tick(&mut self, tick: u64) {
        if let Out::Cpu { sink, .. } = self {
            sink.observe_tick(tick);
        }
    }

    fn should_quit(&mut self) -> bool {
        match self {
            Out::Cpu { sink, .. } => sink.should_quit(),
            Out::Gpu(gpu) => gpu.should_quit(),
        }
    }

    /// Whether Ctrl+S was pressed in the development window since the last call.
    fn take_save_request(&mut self) -> bool {
        match self {
            Out::Cpu { .. } => false,
            Out::Gpu(gpu) => gpu.take_save_request(),
        }
    }

    fn finish(&mut self) -> Result<()> {
        match self {
            Out::Cpu { sink, .. } => sink.finish(),
            Out::Gpu(gpu) => gpu.finish(),
        }
    }
}

/// Where the main thread's interval went.
///
/// **The panel's rate is this thread's budget.** Once presentation moved to its own
/// thread the loop became the ceiling — the board showed presented frames tracking
/// recorded ones exactly, and the tick rate itself falling as the world grew — so the
/// running report has to say which part of the loop is spending the second. The three
/// layers come from the voxel crate's own phase timers (`cubarium_voxel::profile`, an
/// `Instant` per phase per tick and no rule); the rest is timed at the call sites here.
#[derive(Default)]
struct Budget {
    step_ns: u64,
    step_max_ns: u64,
    render_ns: u64,
    render_max_ns: u64,
    commands_ns: u64,
    /// The phase counters, summed over the windows the loop's own ticks ran in.
    phases: Phases,
    pack_mark: f64,
    record_mark: f64,
}

/// The voxel crate's phase counters, read at one instant.
///
/// **They are global and count every thread**, so they are read around each of the loop's
/// own `Sim::step` calls rather than once a minute: the start-up viability probe
/// simulates a clone of the world on its own thread for the first few minutes and would
/// otherwise land in these numbers. What still overlaps a step's own window is bounded by
/// that window, and the line says so when the layers add up to more than the step did.
#[derive(Default, Clone, Copy)]
struct Phases {
    world: u64,
    flora: u64,
    fauna: u64,
    wet: u64,
    band: u64,
    search: u64,
}

/// The water leaves, in tick order: what the `world` column sums.
///
/// The live schedule runs the water phases as its own chained systems and never calls
/// `World::step`, so `Phase::WorldStep` (and `Substeps`) never open there and read zero;
/// the water's time is the **sum of its leaves**, as `cubarium-voxel-sim`'s bench counts
/// it (`WATER_PHASES` in `examples/bench.rs`).
const WATER_PHASES: [cubarium_voxel::profile::Phase; 9] = {
    use cubarium_voxel::profile::Phase;
    [
        Phase::Rain,
        Phase::Evaporate,
        Phase::Infiltrate,
        Phase::Fall,
        Phase::Exchange,
        Phase::Drain,
        Phase::WaterTable,
        Phase::Spring,
        Phase::Outlet,
    ]
};

impl Phases {
    fn now() -> Phases {
        use cubarium_voxel::profile::{self, Count, Phase};
        Phases {
            world: WATER_PHASES.iter().copied().map(profile::nanos).sum(),
            flora: profile::nanos(Phase::FloraStep),
            fauna: profile::nanos(Phase::FaunaStep),
            wet: profile::count(Count::ExchangeWet),
            band: profile::count(Count::WaterTableCells),
            search: profile::count(Count::FallCells),
        }
    }

    fn add_since(&mut self, before: Phases) {
        let now = Phases::now();
        self.world += now.world.saturating_sub(before.world);
        self.flora += now.flora.saturating_sub(before.flora);
        self.fauna += now.fauna.saturating_sub(before.fauna);
        self.wet += now.wet.saturating_sub(before.wet);
        self.band += now.band.saturating_sub(before.band);
        self.search += now.search.saturating_sub(before.search);
    }
}

impl Budget {
    /// Start an interval from where the sink's own counters are now.
    fn mark(&mut self, draw: Option<(f64, f64)>) {
        if let Some((pack, record)) = draw {
            self.pack_mark = pack;
            self.record_mark = record;
        }
    }

    fn tick(&mut self, ns: u64, before: Phases) {
        self.step_ns += ns;
        self.step_max_ns = self.step_max_ns.max(ns);
        self.phases.add_since(before);
    }

    fn frame(&mut self, ns: u64) {
        self.render_ns += ns;
        self.render_max_ns = self.render_max_ns.max(ns);
    }

    /// One line for the interval, and the interval starts again.
    ///
    /// `draw` is the sink's own cumulative (pack, record) milliseconds, where it keeps
    /// them; `ticks` and `frames` are what the interval did.
    fn line(&mut self, seconds: f64, ticks: u64, frames: u64, draw: Option<(f64, f64)>) -> String {
        let ms = |ns: u64| ns as f64 / 1e6;
        let per = |ns: u64, n: u64| ms(ns) / n.max(1) as f64;
        let p = self.phases;
        let (world, flora, fauna) = (p.world, p.flora, p.fauna);
        let (wet, band, search) = (p.wet, p.band, p.search);
        // Another thread was simulating inside the loop's own step windows — the
        // start-up probe — so the layer split is an over-count until it finishes.
        let shared = if world + flora + fauna > self.step_ns {
            " (+ another thread simulating)"
        } else {
            ""
        };
        // The CPU sink draws the world afresh every frame and separates nothing, so it
        // has no split to report; the whole of it is in `render`.
        let split = draw.map(|(p, r)| {
            let d = (p - self.pack_mark, r - self.record_mark);
            self.pack_mark = p;
            self.record_mark = r;
            format!(
                "; pack {:.2}/tick, record {:.2}/frame",
                d.0 / ticks.max(1) as f64,
                d.1 / frames.max(1) as f64
            )
        });
        let busy = (self.step_ns + self.render_ns + self.commands_ns) as f64 / 1e9;
        let line = format!(
            "tick cost — step {:.1} ms/tick (max {:.1}: world {:.1}, flora {:.1}, \
             fauna {:.1}{shared}), render {:.2} ms/frame (max {:.1}{}), commands {:.2} ms; \
             the loop was busy {:.0} % of {seconds:.1} s; \
             water per tick (summed over its substeps) — {} exchanging, {} band, \
             {} falling cells",
            per(self.step_ns, ticks),
            ms(self.step_max_ns),
            per(world, ticks),
            per(flora, ticks),
            per(fauna, ticks),
            per(self.render_ns, frames),
            ms(self.render_max_ns),
            split.unwrap_or_default(),
            ms(self.commands_ns),
            100.0 * busy / seconds.max(1e-9),
            wet / ticks.max(1),
            band / ticks.max(1),
            search / ticks.max(1),
        );
        self.phases = Phases::default();
        self.step_ns = 0;
        self.step_max_ns = 0;
        self.render_ns = 0;
        self.render_max_ns = 0;
        self.commands_ns = 0;
        line
    }
}

/// How often the running report is printed. The same line the run ends with, for the
/// numbers **since the last one**: a service that only reports at shutdown cannot be
/// asked how fast it is going without being stopped.
const SUMMARY_INTERVAL: Duration = Duration::from_secs(60);

/// What was done in `elapsed` seconds, and what the world holds now — the run's one
/// summary line, printed periodically while it runs and once when it ends.
fn run_line(sim: &Sim, ticks: u64, frames: u64, elapsed: f64, presented: Option<u64>) -> String {
    let (world, flora, _) = sim.layers();
    let view = world.view();
    let fv = flora.view();
    let per_s = |n: u64| n as f64 / elapsed.max(1e-9);
    // The frames the loop drew are not the frames the panel was shown once presentation
    // is on its own thread, and the rate that matters is the one the panel saw.
    let shown = presented.map_or(String::new(), |n| {
        format!(", {n} presented ({:.1} fps)", per_s(n))
    });
    format!(
        "{ticks} ticks, {frames} frames in {elapsed:.2} s ({:.1} fps); \
         {:.1} ticks/s{shown}; \
         stored {:.3} m3, residual {:.3e} m3; \
         {} stands, {} latticevines on {} faces ({} dormant), \
         flora residual {:.3e} organic, {:.3e} mineral, {:.3e} energy",
        per_s(frames),
        per_s(ticks),
        view.stored_m3(),
        view.stored_m3() - view.ledger.expected_stored(),
        fv.stands.len(),
        fv.cover.vines().len(),
        fv.cover.face_count(),
        fv.cover.vines().iter().filter(|v| v.dormant).count(),
        fv.organic() - fv.ledger.expected_organic(),
        fv.mineral() - fv.ledger.expected_mineral(),
        fv.energy() - fv.ledger.expected_energy(),
    )
}

/// Open the output this run draws into, for a world of `proj`'s geometry.
///
/// Its own function because it is called **before** the world exists: the founding frame
/// goes through this same sink (see [`found_while_presenting`]).
fn open_out(args: &Voxel, cfg: &VoxelConfig, proj: Projection, speed: f64) -> Result<Out> {
    let shape = WorldShape::new(
        Topology::Ring {
            w: proj.raster_w,
            h: proj.raster_h,
        },
        Scale::ONE,
    );
    let web = |port| {
        WebSink::with_world(
            port,
            "voxel strip",
            crate::sink::web::Source {
                speed,
                ..Default::default()
            },
            None,
            shape,
        )
    };
    Ok(match args.sink {
        VoxelSinkArg::Png => Out::cpu(
            Box::new(PngSink::new(&args.out, args.every)?),
            cfg.clone(),
            proj,
            load_models(cfg),
        ),
        VoxelSinkArg::Web => Out::cpu(
            Box::new(web(args.web_port)?),
            cfg.clone(),
            proj,
            load_models(cfg),
        ),
        VoxelSinkArg::Gpu => {
            crate::sink::gpu::voxel::check(proj)?;
            let mut gpu = VoxelGpuSink::new(
                cfg,
                proj,
                VoxelGpuSinkOptions {
                    target: args.gpu_target.unwrap_or_else(GpuTargetKind::detect),
                    capture: args.gpu_capture.clone(),
                    roof_from_texture: !args.gpu_roof_walk,
                    models: load_models(cfg),
                    textures_under: None,
                    specimens: Vec::new(),
                },
            )?;
            if args.gpu_web_rate > 0.0 {
                gpu = gpu.with_web(web(args.web_port)?, args.gpu_web_rate);
            }
            gpu.set_weather_preview(args.weather_preview);
            Out::Gpu(Box::new(gpu))
        }
    })
}

/// Whether two world configs project to the same picture — the only thing the output is
/// opened on before the world exists.
fn same_extent(a: &cubarium_voxel::Config, b: &cubarium_voxel::Config) -> bool {
    (a.width, a.height, a.depth) == (b.width, b.height, b.depth)
}

/// Run `found` on another thread and present the founding frame at `fps` until it is
/// done, then return what it built.
///
/// **Nothing blocks the first frame.** Founding a world is minutes of seed gate and
/// settle on the board; the panel's daemon blanks the screen three seconds after the last
/// flip. So the wait is covered rather than waited out, at the display's own rate.
///
/// `stop` ends the presenting early — the build itself cannot be cancelled, so the wait
/// for it is still paid, silently.
fn found_while_presenting<T: Send>(
    out: &mut Out,
    fps: u32,
    stop: &AtomicBool,
    found: impl FnOnce() -> Result<T> + Send,
) -> Result<T> {
    std::thread::scope(|scope| {
        let worker = scope.spawn(found);
        let period = Duration::from_nanos(1_000_000_000 / u64::from(fps.clamp(MIN_FPS, MAX_FPS)));
        let start = Instant::now();
        let mut next = start;
        let mut frames = 0u64;
        while !worker.is_finished() {
            let now = Instant::now();
            if now < next {
                std::thread::sleep((next - now).min(period));
                continue;
            }
            out.founding(now.duration_since(start).as_secs_f64())?;
            frames += 1;
            next += period;
            let now = Instant::now();
            if next <= now {
                // The sink is slower than the frame rate asked for: draw at the rate it
                // can rather than chasing a schedule it will never meet.
                next = now + period;
            }
            if stop.load(Ordering::Relaxed) {
                break;
            }
        }
        let founded = worker
            .join()
            .map_err(|_| anyhow::anyhow!("the thread founding the world panicked"))?;
        if frames > 0 {
            eprintln!(
                "cubarium voxel: founding frame held the panel for {:.1} s ({frames} frames)",
                start.elapsed().as_secs_f64()
            );
        }
        founded
    })
}

/// Simulated seconds the start-up viability probe watches. Short on purpose: it runs on
/// a **clone** of the world before the first frame, so the person waiting to see the
/// habitat pays for it in wall clock.
const VIABILITY_WINDOW_S: u64 = 180;

/// Say whether this world's water cycle is one the seeded species could live in. Reports
/// only — nothing is rejected, that is a later decision
/// (`design/handoffs/voxel-water-cycle-2026-09-20.md`).
///
/// Measured on a clone, so the run itself starts on the world the scene built and not on
/// one this probe has already stepped two minutes forward — and **off the loop**: the
/// probe is `VIABILITY_WINDOW_S` of simulation twice over, which is 216 s of wall clock
/// on the board's cores. Paid on the loop, that was four minutes of black panel before
/// the first frame (Wrysk, 2026-09-22). It now runs on its own thread and prints its one
/// line, unchanged, whenever it gets there. The returned handle is the caller's to keep
/// or drop; dropping it only detaches the thread.
fn report_water_cycle(world: &World, flora: &Flora, threads: usize) -> Option<JoinHandle<()>> {
    if !world.config().closed_water_budget {
        eprintln!(
            "cubarium voxel: open water budget — rain from nowhere, evaporation and the \
             outlet to nowhere; no cycle to report"
        );
        return None;
    }
    // The bands are the plant layer's own establishment gate, read back as a soil pore
    // fraction (the gate itself reads available water, package F). It is a floor, so
    // the band's ceiling is saturation: too wet is drowning, which the plant layer
    // judges from standing water and not from pore.
    let bands: Vec<PoreBand> = Species::ALL
        .iter()
        .map(|&s| {
            let floor = flora
                .config()
                .species(s)
                .establish_pore_min_on(&world.config().materials, cubarium_voxel::Material::Soil);
            PoreBand::new(s.name(), floor, 1.0)
        })
        .collect();
    let spec = ViabilitySpec {
        // The same again as a warm-up: a cycle that has not started yet is not a cycle
        // that cannot start, and a fresh world's first minutes are its transient.
        warmup_ticks: VIABILITY_WINDOW_S * u64::from(cubarium_voxel::TICK_HZ),
        window_ticks: VIABILITY_WINDOW_S * u64::from(cubarium_voxel::TICK_HZ),
        sample_every: u64::from(cubarium_voxel::TICK_HZ),
        bands,
        threads,
        ..ViabilitySpec::default()
    };
    let mut probe = world.clone();
    Some(report_off_the_loop(move || {
        let started = Instant::now();
        let report = cubarium_voxel::viability::measure(&mut probe, &spec);
        format!(
            "{report} (probed {VIABILITY_WINDOW_S} simulated s after a \
             {VIABILITY_WINDOW_S} s warm-up, in {:.1} s)",
            started.elapsed().as_secs_f64()
        )
    }))
}

/// Run `probe` on its own thread and print the line it returns when it is done.
///
/// The one place a start-up diagnostic is taken off the loop, so there is one place to
/// look when a report arrives late and out of order — which it will.
fn report_off_the_loop(probe: impl FnOnce() -> String + Send + 'static) -> JoinHandle<()> {
    std::thread::spawn(move || eprintln!("cubarium voxel: {}", probe()))
}

/// The run state a stdin command may change: the session's [`Console`], which holds the
/// one copy of every command, printed the way this binary always has.
///
/// It owns the scalars and borrows the world per call rather than the other way round,
/// so the loop can read `paused` and `speed` while it still holds the world.
struct Control {
    console: Console,
}

impl std::ops::Deref for Control {
    type Target = Console;
    fn deref(&self) -> &Console {
        &self.console
    }
}

impl std::ops::DerefMut for Control {
    fn deref_mut(&mut self) -> &mut Console {
        &mut self.console
    }
}

impl Control {
    fn new(speed: f64, proj: Projection) -> Control {
        let mut console = Console::new(speed, (proj.width, proj.height, proj.depth));
        console.default_save_dir = save::default_save_dir;
        Control { console }
    }

    /// Apply one stdin line ([`Console::handle`]) and say what it had to say.
    fn handle(&mut self, world: &mut World, flora: &mut Flora, fauna: &mut Fauna, line: &str) {
        self.console.handle(world, flora, fauna, line);
        for said in self.console.take_said() {
            eprintln!("cubarium voxel: {said}");
        }
    }
}

/// Read stdin lines on a thread and hand them to the loop.
///
/// Detached on purpose: a blocking `read_line` cannot be cancelled portably, and the
/// process is about to exit anyway. When stdin closes the thread ends and the loop
/// simply stops getting commands.
fn spawn_stdin_reader(tx: mpsc::Sender<String>) {
    std::thread::Builder::new()
        .name("cubarium-voxel-stdin".into())
        .spawn(move || {
            use std::io::BufRead;
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(line) => {
                        if tx.send(line).is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
        })
        .expect("spawning the voxel stdin reader");
}

/// The observatory, while `--serve` or the config's `serve` asks for one: the loop-side
/// recorder and publisher, the server it hands publications to, and the commands the
/// server's readers send back.
struct Observatory {
    observer: cubarium_observatory::Observer,
    server: cubarium_observatory::server::Server,
    commands: Receiver<cubarium_observatory::Command>,
}

impl Observatory {
    fn start(addr: &str, cfg: &VoxelConfig, label: &str) -> Result<Observatory> {
        let (tx, commands) = mpsc::channel();
        let server = cubarium_observatory::server::Server::start(addr, cfg.sim.observatory, tx)
            .with_context(|| format!("serving the observatory on {addr}"))?;
        eprintln!(
            "cubarium voxel: observatory at {} (bound {}): dashboard, JSON and an event stream",
            server.url(),
            server.addr()
        );
        Ok(Observatory {
            observer: cubarium_observatory::Observer::new(label, cubarium_observatory::Cadence::default()),
            server,
            commands,
        })
    }

    /// Publish if one is due.
    fn maybe_publish(&mut self, sim: &Sim, ctl: &Control) {
        let now = Instant::now();
        if self.observer.due(sim.world().tick(), now, ctl.paused) {
            let (world, flora, fauna) = sim.layers();
            let controls = cubarium_observatory::Controls {
                speed: ctl.speed,
                paused: ctl.paused,
                min_speed: MIN_SPEED,
                max_speed: MAX_SPEED,
            };
            let p = self.observer.publish(world, flora, fauna, controls, now);
            self.server.publish(p);
        }
    }
}

#[cfg(test)]
mod roster_tests;

#[cfg(test)]
mod tests {
    use super::*;

    /// A sink that counts what it is handed and keeps the last frame's brightest pixel.
    struct CountingSink {
        frames: std::sync::Arc<std::sync::atomic::AtomicU64>,
        brightest: std::sync::Arc<std::sync::atomic::AtomicU64>,
    }

    impl crate::sink::FrameSink for CountingSink {
        fn submit(&mut self, out: Output<'_>) -> Result<()> {
            if let Output::Ring(r) = out {
                let mut top = 0u64;
                for y in 0..r.height() {
                    for x in 0..r.width() {
                        let p = r.get(x, y);
                        top = top.max(u64::from(p[0]) + u64::from(p[1]) + u64::from(p[2]));
                    }
                }
                self.brightest.store(top, Ordering::Relaxed);
            }
            self.frames.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    /// **The panel has a picture before the world does.** The frames are presented while
    /// the world is still being founded on the other thread, through the run's own sink,
    /// and they are the sky rather than black.
    #[test]
    fn the_founding_frame_is_presented_before_the_world_exists() {
        let frames = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let brightest = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let cfg = VoxelConfig {
            sim: SimParams {
                world: cubarium_voxel::Config {
                    width: 16,
                    height: 8,
                    depth: 2,
                    ..Default::default()
                },
                ..SimParams::default()
            },
            ..VoxelConfig::default()
        };
        let proj = Projection::new(30.0, 4, 0, &cfg.sim.world).unwrap();
        let mut out = Out::cpu(
            Box::new(CountingSink {
                frames: frames.clone(),
                brightest: brightest.clone(),
            }),
            cfg,
            proj,
            None,
        );

        // The "world" is not built until four founding frames have gone out.
        let seen = frames.clone();
        let stop = AtomicBool::new(false);
        let built = found_while_presenting(&mut out, MAX_FPS, &stop, || {
            while seen.load(Ordering::Relaxed) < 4 {
                std::hint::spin_loop();
            }
            Ok("the world")
        })
        .unwrap();

        assert_eq!(built, "the world");
        assert!(
            frames.load(Ordering::Relaxed) >= 4,
            "frames went out before the world existed"
        );
        assert!(
            brightest.load(Ordering::Relaxed) > 0,
            "the founding frame is the sky, not black"
        );
        // The pulse is a pulse: one breath is dimmer in the middle than at its ends.
        assert!((present::founding_pulse(0.0) - 1.0).abs() < 1e-6);
        assert!(present::founding_pulse(present::FOUNDING_PULSE_S / 2.0) < 0.7);
        assert!((present::founding_pulse(present::FOUNDING_PULSE_S) - 1.0).abs() < 1e-6);
    }

    /// The running report is the line the run ends with, over the interval it names, so
    /// the panel's frame rate can be read without stopping the service.
    #[test]
    fn the_running_report_is_the_summary_line_for_its_own_interval() {
        assert_eq!(SUMMARY_INTERVAL, Duration::from_secs(60));
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let sim = Sim::new(
            World::empty(c.clone()),
            Flora::new(FloraConfig::for_voxel_size(c.voxel_m)),
            Fauna::new(FaunaConfig::placeholder()),
            SimConfig::with_threads(1),
            None,
        );
        let line = run_line(&sim, 1_200, 3_600, 60.0, None);
        assert!(
            line.starts_with("1200 ticks, 3600 frames in 60.00 s (60.0 fps);"),
            "{line}"
        );
        assert!(line.contains("0 stands"), "{line}");
        assert!(line.contains("20.0 ticks/s"), "{line}");
        assert!(
            !line.contains("presented"),
            "a target that shows every frame it is given says nothing: {line}"
        );
    }

    /// With presentation on its own thread the frames the loop drew are not the frames
    /// the panel was shown, and the rate that matters is the second one. Both are in the
    /// line, with the tick rate beside them.
    #[test]
    fn the_running_report_names_the_tick_rate_and_what_the_panel_was_shown() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let sim = Sim::new(
            World::empty(c.clone()),
            Flora::new(FloraConfig::for_voxel_size(c.voxel_m)),
            Fauna::new(FaunaConfig::placeholder()),
            SimConfig::with_threads(1),
            None,
        );
        let line = run_line(&sim, 1_200, 3_600, 60.0, Some(3_580));
        assert!(line.contains("20.0 ticks/s"), "{line}");
        assert!(line.contains("3580 presented (59.7 fps)"), "{line}");
    }

    /// The start-up viability probe is off the loop: the call returns while the work is
    /// still running, and its line arrives later.
    #[test]
    fn the_water_cycle_report_returns_before_it_has_anything_to_say() {
        let (release, wait) = mpsc::channel::<()>();
        let done = std::sync::Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        let handle = report_off_the_loop(move || {
            wait.recv().expect("released");
            flag.store(true, Ordering::Relaxed);
            "the probe's line".to_string()
        });
        // The probe cannot have finished: nothing has released it.
        assert!(!done.load(Ordering::Relaxed), "the call did not return early");
        release.send(()).unwrap();
        handle.join().unwrap();
        assert!(done.load(Ordering::Relaxed), "the line comes later");
    }

    /// The `world` column is the water's leaves, which the live schedule does time —
    /// `Phase::WorldStep` never opens there, which is why it read 0.0 on the board.
    #[test]
    fn the_world_timer_counts_the_live_schedules_water() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let mut sim = Sim::new(
            World::new(c.clone()),
            Flora::new(FloraConfig::for_voxel_size(c.voxel_m)),
            Fauna::new(FaunaConfig::placeholder()),
            SimConfig::with_threads(1),
            None,
        );
        let mut budget = Budget::default();
        for _ in 0..3 {
            let (at, before) = (Instant::now(), Phases::now());
            sim.step();
            budget.tick(at.elapsed().as_nanos() as u64, before);
        }
        assert!(budget.phases.world > 0, "the water leaves were timed");
        let line = budget.line(1.0, 3, 0, None);
        assert!(line.contains("world "), "{line}");
    }

    /// A scratch state directory, removed when dropped.
    struct StateDir(PathBuf);

    impl StateDir {
        fn new(tag: &str) -> StateDir {
            let dir =
                std::env::temp_dir().join(format!("cubarium-snap-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            StateDir(dir)
        }

        fn snapshots(&self) -> Vec<PathBuf> {
            let mut v: Vec<PathBuf> = std::fs::read_dir(&self.0)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .and_then(parse_voxel_tick)
                        .is_some()
                })
                .collect();
            v.sort();
            v
        }
    }

    impl Drop for StateDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A reader polling the directory while the background writer works never finds a
    /// `world-N.voxel` that does not load: the file appears whole or not at all.
    #[test]
    fn a_background_snapshot_is_never_seen_half_written() {
        let dir = StateDir::new("torn");
        let c = cubarium_voxel::Config {
            width: 64,
            height: 32,
            depth: 16,
            ..Default::default()
        };
        let mut world = World::new(c);
        let (flora, fauna) = (
            Flora::new(FloraConfig::default()),
            Fauna::new(FaunaConfig::placeholder()),
        );
        let mut writer = SnapshotWriter::new(dir.0.clone());
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let reader = {
            let (stop, path) = (stop.clone(), dir.0.clone());
            std::thread::spawn(move || {
                let mut loads = 0;
                while !stop.load(Ordering::Relaxed) {
                    for p in std::fs::read_dir(&path)
                        .unwrap()
                        .flatten()
                        .map(|e| e.path())
                    {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if parse_voxel_tick(name).is_some() {
                            // Pruned between the listing and the read is fine; a read that
                            // succeeds must be a whole world.
                            if let Ok(bytes) = std::fs::read(&p) {
                                World::load(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
                                loads += 1;
                            }
                        }
                    }
                }
                loads
            })
        };
        for _ in 0..3 {
            assert!(writer.save_in_background(&world, &flora, &fauna));
            writer.wait();
            world.step();
        }
        writer.save_now(&world, &flora, &fauna);
        stop.store(true, Ordering::Relaxed);
        assert!(reader.join().unwrap() > 0, "the reader saw snapshots");
        assert_eq!(dir.snapshots().len(), 4);
        let leftovers = std::fs::read_dir(&dir.0)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("tmp-"))
            .count();
        assert_eq!(leftovers, 0, "no temporary file outlives its write");
    }

    /// A snapshot due while one is writing is skipped, and the exit save waits for the
    /// write in flight before it writes its own.
    #[test]
    fn the_exit_snapshot_waits_for_the_write_in_flight() {
        let dir = StateDir::new("exit");
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let world = World::new(c);
        let mut writer = SnapshotWriter::new(dir.0.clone());
        let (release, held) = mpsc::channel::<()>();
        let finished = std::sync::Arc::new(AtomicBool::new(false));
        let flag = finished.clone();
        assert!(writer.spawn(move || {
            held.recv().unwrap();
            std::thread::sleep(Duration::from_millis(50));
            flag.store(true, Ordering::SeqCst);
        }));
        let (flora, fauna) = (
            Flora::new(FloraConfig::default()),
            Fauna::new(FaunaConfig::placeholder()),
        );
        assert!(
            !writer.save_in_background(&world, &flora, &fauna),
            "one write in flight at a time"
        );
        release.send(()).unwrap();
        writer.save_now(&world, &flora, &fauna);
        assert!(finished.load(Ordering::SeqCst), "the exit save waited");
        let snaps = dir.snapshots();
        assert_eq!(snaps.len(), 1);
        World::load(&std::fs::read(&snaps[0]).unwrap()).unwrap();
    }

    /// `px_per_voxel = "auto"` is the largest scale whose whole strip fits the output.
    #[test]
    fn px_per_voxel_auto_is_the_largest_whole_strip_that_fits() {
        let cfg: VoxelConfig = toml::from_str(
            "px_per_voxel = \"auto\"\n[world]\nwidth = 256\nheight = 128\ndepth = 48\n",
        )
        .unwrap();
        assert_eq!(cfg.px_per_voxel, PX_AUTO);
        assert!(toml::to_string(&cfg).unwrap().contains("px_per_voxel = \"auto\""));
        assert!(toml::from_str::<VoxelConfig>("px_per_voxel = 0").is_err());
        assert!(toml::from_str::<VoxelConfig>("px_per_voxel = \"big\"").is_err());
        assert_eq!(
            toml::from_str::<VoxelConfig>("px_per_voxel = 6").unwrap().px_per_voxel,
            6
        );
        let w = &cfg.sim.world;
        // 9: 2304 x (1152 + 48 * 5) = 1392. 10 is 1568 rows.
        assert_eq!(auto_px_per_voxel(30.0, 0, w, (2560, 1440)), 9);
        // 13: 3328 x 2048. 14 is 1792 + 48 * 8 = 2176 rows.
        assert_eq!(auto_px_per_voxel(30.0, 0, w, (3840, 2160)), 13);
        assert_eq!(auto_px_per_voxel(30.0, 0, w, (100, 100)), 1);
        // The panel's config: its fixed 540-row raster holds the strip at 6, not the 12
        // the panel's width alone would allow.
        let panel = cubarium_voxel::Config {
            width: 160,
            height: 72,
            depth: 24,
            ..w.clone()
        };
        assert_eq!(auto_px_per_voxel(30.0, 540, &panel, (1920, 1080)), 6);
        assert_eq!(auto_px_per_voxel(30.0, 0, &panel, (1920, 1080)), 12);
    }

    /// The window's cover is the smallest strip at least as big as the screen on one
    /// axis: at it the strip reaches the screen's width or height, and one less would not.
    #[test]
    fn px_per_voxel_cover_is_the_smallest_strip_that_reaches_the_screen() {
        let w = cubarium_voxel::Preset::find("default").unwrap().config();
        let strip = |s: u32| {
            let rise = project::rise_for(30.0, s);
            (w.width * s, w.height * s + w.depth * rise)
        };
        let reaches = |s: u32, out: (u32, u32)| {
            let (sw, sh) = strip(s);
            sw >= out.0 || sh >= out.1
        };
        for out in [(1920, 1200), (2560, 1600), (3840, 2160), (2560, 1440), (1000, 700)] {
            let s = cover_px_per_voxel(30.0, 0, &w, out);
            assert!(reaches(s, out), "{out:?}: {s} does not reach the screen");
            assert!(s == 1 || !reaches(s - 1, out), "{out:?}: {} already reaches it", s - 1);
        }
        // A strip that fills the screen exactly is its own cover.
        let s = auto_px_per_voxel(30.0, 0, &w, (1920, 1080));
        let exact = (strip(s).0, 4000);
        assert_eq!(cover_px_per_voxel(30.0, 0, &w, exact), s);
    }

    /// With no TOML the ambient world is the shipped `default` landscape on the ring it
    /// was written for, and a partial config file that only overrides some fields keeps
    /// the default for everything else rather than being refused.
    #[test]
    fn no_file_is_the_default_preset_and_a_partial_file_loads() {
        let d = VoxelConfig::default();
        // The shipped `default` landscape, whatever that landscape currently is: the
        // preset owns its own recipe, and pinning a copy of it here only makes this test
        // fail every time the generator tunes one.
        let shipped = cubarium_voxel::Preset::find("default").expect("a default preset");
        assert_eq!(
            d.sim.world.landform,
            cubarium_voxel::Landform::Staged(shipped.recipe),
            "`cubarium voxel` with no TOML generates the shipped staged ring"
        );
        assert_eq!(
            (d.sim.world.width, d.sim.world.height, d.sim.world.depth),
            (shipped.width, shipped.height, shipped.depth),
            "and on the ring that landscape was written for"
        );

        let cfg: VoxelConfig =
            toml::from_str("tilt_degrees = 35.0\n[world]\nwidth = 64\ndepth = 8\n").unwrap();
        assert_eq!(cfg.tilt_degrees, 35.0);
        assert_eq!(
            cfg.px_per_voxel, d.px_per_voxel,
            "an unmentioned field keeps its default"
        );
        assert_eq!(cfg.sim.world.width, 64);
        assert_eq!(cfg.sim.world.depth, 8);
        assert_eq!(cfg.sim.world.voxel_m, d.sim.world.voxel_m);
        let explicit: VoxelConfig = toml::from_str("[world]\nvoxel_m = 0.125\n").unwrap();
        assert_eq!(explicit.sim.world.voxel_m, 0.125);
        assert_eq!(cfg.sim.world.height, cubarium_voxel::Config::default().height);

        // A typo is an error, not a silently ignored key.
        assert!(toml::from_str::<VoxelConfig>("tilt_degree = 35.0\n").is_err());
        assert!(toml::from_str::<VoxelConfig>("[world]\nwidht = 64\n").is_err());
    }

    /// `[world.landform]` picks the generator: absent is the ridge generator every world
    /// had, a preset name expands to that shipped recipe, and any field beside the name
    /// overrides it. An unknown preset is an error with the names that do exist.
    #[test]
    fn the_world_table_chooses_a_landform_and_names_a_preset() {
        use cubarium_voxel::Landform;

        let plain: VoxelConfig = toml::from_str("[world]\nwidth = 64\n").unwrap();
        assert_eq!(plain.sim.world.landform, Landform::Ridge);

        let ridge: VoxelConfig = toml::from_str("[world]\nlandform = \"ridge\"\n").unwrap();
        assert_eq!(ridge.sim.world.landform, Landform::Ridge);

        let wide: VoxelConfig =
            toml::from_str("[world]\n[world.landform]\npreset = \"wide\"\n").unwrap();
        let wanted = cubarium_voxel::Preset::find("wide").unwrap().recipe;
        assert_eq!(wide.sim.world.landform, Landform::Staged(wanted));

        let tweaked: VoxelConfig =
            toml::from_str("[world]\n[world.landform]\npreset = \"small\"\nrelief_m = 2.0\n")
                .unwrap();
        let Landform::Staged(recipe) = tweaked.sim.world.landform else {
            panic!("a preset makes a staged landform");
        };
        assert_eq!(recipe.relief_m, 2.0, "the file overrides the preset");
        assert_eq!(
            recipe.relief_wavelength_m,
            cubarium_voxel::Recipe::SMALL.relief_wavelength_m,
            "and leaves the rest of it alone"
        );
        assert!(tweaked.sim.world.validate().is_ok());

        let err = toml::from_str::<VoxelConfig>("[world.landform]\npreset = \"huge\"\n")
            .expect_err("there is no huge preset");
        let msg = format!("{err}");
        assert!(msg.contains("huge") && msg.contains("wide"), "{msg}");
    }

    /// The committed example config parses, is the default picture, and does not pin the
    /// world depth: that follows `cubarium_voxel::Config::default()`, so changing the
    /// habitat's depth in the core changes the example without editing it.
    #[test]
    fn the_example_config_parses_and_leaves_the_depth_to_the_core() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("voxel.example.toml");
        let cfg = load_config(&path).unwrap();
        assert_eq!(cfg, VoxelConfig::default());
        assert_eq!(cfg.sim.world.depth, cubarium_voxel::Config::default().depth);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.lines().any(|l| l.trim_start().starts_with("depth")),
            "the example must not set `depth`; it mirrors the core default"
        );
    }

    /// The panel's own config selects a designed terrarium (Wrysk, 2026-09-23), and
    /// `natural.toml` beside it a staged landscape; both load and validate. Neither
    /// states water keys: the recipe decides those now.
    #[test]
    fn the_tachyon_configs_name_a_terrarium_and_a_staged_landscape() {
        let terrarium = |l: &cubarium_voxel::Landform| {
            matches!(l, cubarium_voxel::Landform::Terrarium(_))
        };
        let staged =
            |l: &cubarium_voxel::Landform| matches!(l, cubarium_voxel::Landform::Staged(_));
        tachyon_config_names("voxel.toml", terrarium);
        tachyon_config_names("natural.toml", staged);
    }

    fn tachyon_config_names(file: &str, landform: impl Fn(&cubarium_voxel::Landform) -> bool) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/tachyon")
            .join(file)
            .canonicalize()
            .expect("the panel config is committed");
        let cfg = load_config(&path).unwrap();
        assert!(landform(&cfg.sim.world.landform), "{file}: {:?}", cfg.sim.world.landform);
        assert!(cfg.sim.world.validate().is_ok(), "{file}");
        let text = std::fs::read_to_string(&path).unwrap();
        for key in [
            "rain_m_per_s",
            "evaporation_m_per_s",
            "initial_aquifer_head_m",
        ] {
            assert!(
                !text.contains(key),
                "{key} is the recipe's business, not the panel's"
            );
        }
    }

    /// **The panel's own case.** Its unit runs `cubarium voxel`, its worlds are
    /// `world-<tick>.voxel`, and it crash-looped holding five snapshots of schema 13
    /// against a binary that speaks 15: "skipping corrupt snapshot … voxel snapshot
    /// schema 13 is not 15" five times over, then "none of them loaded". A world of a
    /// format this build cannot read is **stale**, not damaged — nothing migrates — so it
    /// is discarded and a fresh world founded. Anything else still refuses by name.
    #[test]
    fn stale_voxel_snapshots_are_discarded_and_damaged_ones_still_refuse() {
        let dir = std::env::temp_dir().join(format!("cubarium-voxel-p-{}", std::process::id()));
        let cfg = VoxelConfig::default();
        let args = |state: &Path| crate::cli::Voxel {
            config: None,
            set: Vec::new(),
            params: None,
            sink: crate::cli::VoxelSinkArg::Web,
            scene: crate::cli::VoxelSceneArg::Generated,
            seed: Some(1),
            arena: None,
            controller: crate::cli::VoxelControllerArg::Heuristic,
            policy: None,
            founder_policy: Vec::new(),
            founder_heuristic: Vec::new(),
            introduce: Vec::new(),
            no_egg_bank: false,
            arena_seed: 1,
            arena_diagnostics: false,
            empty: true, // about loading and seeds, not founding a habitat
            seconds: 0.0,
            speed: 1.0,
            load: Some(state.to_path_buf()),
            out: PathBuf::from("captures"),
            every: 30,
            fps: 60,
            pin_loop: false,
            web_port: 7393,
            gpu_target: None,
            gpu_capture: None,
            weather_preview: None,
            gpu_web_rate: 0.0,
            gpu_roof_walk: false,
            textures: false,
            weather_force: None,
            serve: None,
        };
        // A snapshot of a schema this build does not speak: postcard's varint for the tag.
        let stale = |n: u8| vec![n, 0, 0, 0, 0, 0, 0, 0];

        let state = dir.join("stale");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("world-100.voxel"), stale(13)).unwrap();
        std::fs::write(state.join("world-200.voxel"), stale(13)).unwrap();
        let (_, label, resumed, _) =
            load_or_create_world(&args(&state), &cfg).expect("a stale directory founds a world");
        assert!(!resumed, "nothing was resumed: {label}");
        assert!(
            std::fs::read_dir(&state).unwrap().count() == 0,
            "and the stale files are gone"
        );

        // A truncated file is a damaged world: refused by name, and left where it is.
        let damaged = dir.join("damaged");
        std::fs::create_dir_all(&damaged).unwrap();
        std::fs::write(damaged.join("world-100.voxel"), [0xffu8; 3]).unwrap();
        let err = match load_or_create_world(&args(&damaged), &cfg) {
            Ok(_) => panic!("a damaged world must refuse"),
            Err(e) => format!("{e:#}"),
        };
        assert!(err.contains("none of them loaded"), "{err}");
        assert_eq!(std::fs::read_dir(&damaged).unwrap().count(), 1, "kept");

        // One readable snapshot beside a stale one: the readable one is resumed and both
        // files stay. Discarding is only for a directory with nothing left to read.
        let mixed = dir.join("mixed");
        std::fs::create_dir_all(&mixed).unwrap();
        let good = World::new(cubarium_voxel::Config {
            width: 16,
            height: 12,
            depth: 2,
            ..cubarium_voxel::Config::default()
        });
        std::fs::write(mixed.join("world-50.voxel"), good.save()).unwrap();
        std::fs::write(mixed.join("world-100.voxel"), stale(13)).unwrap();
        let (_, label, resumed, _) =
            load_or_create_world(&args(&mixed), &cfg).expect("the readable snapshot resumes");
        assert!(resumed, "{label}");
        assert_eq!(
            std::fs::read_dir(&mixed).unwrap().count(),
            2,
            "and nothing was deleted"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The config's `[flora]` / `[fauna]` tables and `--set` reach the run's parameter set
    /// in that order, an unknown path is refused by name, and the set is what a fresh
    /// world's layers are built from (package R1).
    #[test]
    fn the_config_tables_and_set_reach_the_parameter_set() {
        use clap::Parser;
        let voxel = |args: &[&str]| -> crate::cli::Voxel {
            let mut line = vec!["cubarium", "voxel"];
            line.extend(args.iter().copied());
            match crate::cli::Cli::parse_from(line).command {
                crate::cli::Command::Voxel(v) => v,
                other => panic!("expected a voxel command, got {other:?}"),
            }
        };
        let cfg: VoxelConfig = toml::from_str(
            "[world]\nwidth = 64\n\
             [fauna.founders.frondgrazer.core]\nmaintenance_per_s = 0.002\n\
             [flora.springturf]\nfoliage_rate = 0.0005\n",
        )
        .expect("a config with parameter tables");
        let p = resolve_params(&voxel(&[]), &cfg).expect("the file's tables");
        assert_eq!(p.fauna.founder(Founder::Browser).core.maintenance_per_s, 0.002);
        assert_eq!(p.flora.springturf.foliage_rate, 0.0005);
        assert_eq!(p.world.width, 64);
        let p = resolve_params(
            &voxel(&[
                "--set",
                "fauna.founders.frondgrazer.core.maintenance_per_s=0.003",
                "--set",
                "world.climate.dry_gap_min_s=100",
            ]),
            &cfg,
        )
        .expect("--set over the file");
        assert_eq!(p.fauna.founder(Founder::Browser).core.maintenance_per_s, 0.003);
        assert_eq!(p.world.climate.dry_gap_min_s, 100.0);
        let e = resolve_params(&voxel(&["--set", "fauna.nope=1"]), &cfg)
            .expect_err("an unknown path");
        assert!(format!("{e:#}").contains("fauna.nope"), "{e:#}");
        let bad: VoxelConfig = toml::from_str("[flora.springturf]\nnope = 1\n").unwrap();
        let e = resolve_params(&voxel(&[]), &bad).expect_err("an unknown file path");
        assert!(format!("{e:#}").contains("flora.springturf.nope"), "{e:#}");
    }

    /// `--params <file>` (opt-in; co-training stage 2's "watch the winner"): a search
    /// genome (a run's `mean.toml` / `best.toml`) is decoded over the config's tables, a
    /// resolved parameter file (a run's `mean-params/params.toml`) replaces the plants and
    /// animals whole, `--set` still comes last, and without the flag nothing changes.
    #[test]
    fn params_takes_a_genome_or_a_resolved_file_and_is_opt_in() {
        use clap::Parser;
        let voxel = |args: &[&str]| -> crate::cli::Voxel {
            let mut line = vec!["cubarium", "voxel"];
            line.extend(args.iter().copied());
            match crate::cli::Cli::parse_from(line).command {
                crate::cli::Command::Voxel(v) => v,
                other => panic!("expected a voxel command, got {other:?}"),
            }
        };
        let dir = std::env::temp_dir().join(format!("cub-live-params-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg: VoxelConfig = toml::from_str("[world]
width = 64
").unwrap();
        let shipped = resolve_params(&voxel(&[]), &cfg).expect("no --params");
        let base = shipped.fauna.founder(Founder::Browser).core.maintenance_per_s;

        // A genome: K_bio x2 runs every animal's upkeep twice as fast.
        let mut g = cubarium_search::es::voxel::live::ecology::EcologyParams::cotrain();
        g.set("K_bio", 2.0).unwrap();
        let genome = dir.join("mean.toml");
        g.save(&genome, "a test genome").unwrap();
        let path = genome.display().to_string();
        let p = resolve_params(&voxel(&["--params", &path]), &cfg).expect("a genome");
        assert_eq!(p.fauna.founder(Founder::Browser).core.maintenance_per_s, 2.0 * base);
        assert_eq!(p.world.width, 64, "the world stays the config's");
        let p = resolve_params(
            &voxel(&[
                "--params",
                &path,
                "--set",
                "fauna.founders.frondgrazer.core.maintenance_per_s=0.003",
            ]),
            &cfg,
        )
        .expect("--set over the genome");
        assert_eq!(p.fauna.founder(Founder::Browser).core.maintenance_per_s, 0.003);

        // A resolved parameter file: its plants and animals, the config's world.
        let resolved = dir.join("params.toml");
        std::fs::write(&resolved, g.params().unwrap().to_toml().unwrap()).unwrap();
        let p = resolve_params(&voxel(&["--params", &resolved.display().to_string()]), &cfg)
            .expect("a resolved file");
        assert_eq!(p.fauna.founder(Founder::Browser).core.maintenance_per_s, 2.0 * base);
        assert_eq!(p.world.width, 64);

        // Without the flag, the shipped set.
        assert_eq!(resolve_params(&voxel(&[]), &cfg).unwrap(), shipped);
        assert!(resolve_params(&voxel(&["--params", "/no/such/file.toml"]), &cfg).is_err());
    }

    /// A config file whose `[world]` no world can be built from is an error, not a panic:
    /// `World::new` asserts, so the run validates first.
    #[test]
    fn a_world_no_world_can_be_built_from_is_refused_not_panicked_on() {
        let cfg: VoxelConfig =
            toml::from_str("[world]\ndepth = 0\n").expect("it parses; it is just impossible");
        let err = cfg
            .sim.world
            .validate()
            .expect_err("a zero dimension must be refused");
        assert!(format!("{err}").contains("world.depth"), "{err}");

        let bad: VoxelConfig = toml::from_str("[world]\nvoxel_m = 0.0\n").unwrap();
        assert!(bad.sim.world.validate().is_err());
        assert!(VoxelConfig::default().sim.world.validate().is_ok());
    }

    /// Stdin commands reach the world: pause toggles, speed halves and doubles, a
    /// single step is owed, and a nonsense line changes nothing.
    #[test]
    fn stdin_commands_change_exactly_what_they_name() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let mut world = World::empty(c.clone());
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        let mut ctl = Control::new(1.0, Projection::new(30.0, 4, 0, &c).unwrap());

        ctl.handle(&mut world, &mut flora, &mut fauna, "p");
        assert!(ctl.paused);
        ctl.handle(&mut world, &mut flora, &mut fauna, "p");
        assert!(!ctl.paused);
        ctl.handle(&mut world, &mut flora, &mut fauna, "-");
        assert_eq!(ctl.speed, 0.5);
        ctl.handle(&mut world, &mut flora, &mut fauna, "+");
        ctl.handle(&mut world, &mut flora, &mut fauna, "+");
        assert_eq!(ctl.speed, 2.0);
        ctl.handle(&mut world, &mut flora, &mut fauna, "s");
        assert!(ctl.paused && ctl.pending_steps == 1);
        // `p on` / `p off` say which, whatever the state: what the observatory sends.
        ctl.handle(&mut world, &mut flora, &mut fauna, "p on");
        assert!(ctl.paused);
        ctl.handle(&mut world, &mut flora, &mut fauna, "p off");
        ctl.handle(&mut world, &mut flora, &mut fauna, "p off");
        assert!(!ctl.paused);
        ctl.handle(&mut world, &mut flora, &mut fauna, "p sideways");
        assert!(!ctl.paused, "a bad argument changes nothing");
        // `speed X` sets the multiplier, clamped to the range; nonsense changes nothing.
        ctl.handle(&mut world, &mut flora, &mut fauna, "speed 3");
        assert_eq!(ctl.speed, 3.0);
        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("speed {}", MAX_SPEED * 4.0));
        assert_eq!(ctl.speed, MAX_SPEED);
        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("speed {}", MIN_SPEED / 4.0));
        assert_eq!(ctl.speed, MIN_SPEED);
        for bad in ["speed", "speed 0", "speed -2", "speed fast"] {
            ctl.handle(&mut world, &mut flora, &mut fauna, bad);
            assert_eq!(ctl.speed, MIN_SPEED, "{bad}");
        }
        // Every observatory command is a console line that does the same.
        use cubarium_observatory::Command as Obs;
        ctl.handle(&mut world, &mut flora, &mut fauna, &console_line(Obs::SetSpeed(2.0)));
        assert_eq!(ctl.speed, 2.0);
        ctl.handle(&mut world, &mut flora, &mut fauna, &console_line(Obs::Faster));
        assert_eq!(ctl.speed, 4.0);
        ctl.handle(&mut world, &mut flora, &mut fauna, &console_line(Obs::Slower));
        assert_eq!(ctl.speed, 2.0);
        ctl.handle(&mut world, &mut flora, &mut fauna, &console_line(Obs::Pause));
        assert!(ctl.paused);
        ctl.handle(&mut world, &mut flora, &mut fauna, &console_line(Obs::Resume));
        assert!(!ctl.paused);
        ctl.handle(&mut world, &mut flora, &mut fauna, "o");
        assert!(ctl.outlet);
        // The aquifer takes a charge, gives back only what it holds, and is booked.
        ctl.handle(&mut world, &mut flora, &mut fauna, "a 2.5");
        assert_eq!(world.view().aquifer_m3, 2.5);
        ctl.handle(&mut world, &mut flora, &mut fauna, "a -10");
        assert_eq!(world.view().aquifer_m3, 0.0);
        assert_eq!(world.view().ledger.user_in, 0.0);
        // The atmosphere lever is refused on this open-budget world, and taken on a
        // closed one, where it books as the user's share of the store.
        ctl.handle(&mut world, &mut flora, &mut fauna, "h 3");
        assert_eq!(world.atmosphere_m3(), 0.0);
        let mut closed = World::empty(cubarium_voxel::Config {
            closed_water_budget: true,
            ..c.clone()
        });
        ctl.handle(&mut closed, &mut flora, &mut fauna, "h 3");
        assert_eq!(closed.atmosphere_m3(), 3.0);
        assert_eq!(closed.view().ledger.user_atmosphere_in, 3.0);
        assert!(closed.view().atmosphere_residual().abs() < 1e-12);
        ctl.handle(&mut closed, &mut flora, &mut fauna, "h -1");
        ctl.handle(&mut closed, &mut flora, &mut fauna, "h");
        assert_eq!(closed.atmosphere_m3(), 3.0);
        // Nonsense, and a bad argument, change nothing.
        ctl.handle(&mut world, &mut flora, &mut fauna, "nonsense 1 2 3");
        ctl.handle(&mut world, &mut flora, &mut fauna, "r not-a-volume");
        ctl.handle(&mut world, &mut flora, &mut fauna, "a");
        ctl.handle(&mut world, &mut flora, &mut fauna, "a 0");
        ctl.handle(&mut world, &mut flora, &mut fauna, "a nan");
        assert_eq!(world.view().aquifer_m3, 0.0);
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 0 999 0");
        ctl.handle(&mut world, &mut flora, &mut fauna, "i only-one");
        ctl.handle(&mut world, &mut flora, &mut fauna, "save-terrain");
        assert_eq!((ctl.speed, ctl.pending_steps, ctl.outlet), (2.0, 1, true));
        assert!(!ctl.quit);
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 0 0 0");
        ctl.handle(&mut world, &mut flora, &mut fauna, "q");
        assert!(ctl.quit);
    }

    /// **R9.6: `w` and `l` are terrain only, and an in-place load is refused while
    /// anything of the ecology is standing.** A saved terrain, one edited cell, and then a
    /// stand: `l` refuses and the edit survives, because loading would have put the stand
    /// on another world's terrain with its ledger still describing this one. The same
    /// bytes load into a run that holds no ecology, which is what `--load` does at startup.
    /// **A world that remembers a policy is not quietly put back on the heuristic.**
    ///
    /// `install_founder_controllers` switches a named lineage's birth factory and every
    /// standing body of that lineage onto the driver, and records that the lineage is
    /// policy-driven. A layer that already carries that record and is offered no driver
    /// for it is refused by name. The stand-in driver here is the heuristic control —
    /// the refusal and the install are about *which lineage a driver was supplied for*,
    /// not about what is inside it, and using a control keeps weights out of a unit test.
    #[test]
    fn a_policy_driven_lineage_is_refused_rather_than_demoted_on_load() {
        use cubarium_search::es::voxel::VoxelControl;
        use cubarium_voxel_fauna::StartingStores;

        let world = crate::voxel::scene::authored(cubarium_voxel::Config {
            width: 32,
            height: 16,
            depth: 4,
            ..cubarium_voxel::Config::default()
        });
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        crate::voxel::habitat::install_heuristics(&mut fauna);
        let placed = (0..32i64).any(|x| {
            fauna.apply(
                &world,
                FaunaCommand::IntroduceFounder {
                    x,
                    z: 1,
                    founder: Founder::Blind,
                    stores: StartingStores::HUNGRY,
                    heading_rad: 0.0,
                },
            )
        });
        assert!(placed, "a founder body stands somewhere on the strip");

        // Nothing declared: the heuristics stand and nothing is refused.
        install_founder_controllers(&mut fauna, &[]).expect("the default is the heuristics");
        assert!(!fauna.policy_driven(Founder::Blind));

        // A driver for the blind lineage takes it over and is recorded.
        let driver = EpisodeDriver::control(VoxelControl::Heuristic, Founder::Blind);
        install_founder_controllers(&mut fauna, &[(Founder::Blind, driver)])
            .expect("a supplied driver installs");
        assert!(fauna.policy_driven(Founder::Blind));
        assert!(!fauna.policy_driven(Founder::Browser));

        // That record survives a save and load, and the loaded layer refuses to run
        // without the flag rather than falling back.
        let mut loaded = Fauna::load(&fauna.save()).expect("the layer round-trips");
        crate::voxel::habitat::install_heuristics(&mut loaded);
        //
        // With the trained centres as the default, an empty driver list is what
        // `--founder-heuristic` produces, so this is exactly the demotion the flag could
        // ask for, and the message names both ways back.
        let err = format!(
            "{:#}",
            install_founder_controllers(&mut loaded, &[]).expect_err("a demotion in silence")
        );
        assert!(err.contains("littershredder"), "{err}");
        assert!(err.contains("--founder-heuristic"), "{err}");
        assert!(err.contains("--founder-policy"), "{err}");

        // With the flag back, it loads.
        let driver = EpisodeDriver::control(VoxelControl::Heuristic, Founder::Blind);
        install_founder_controllers(&mut loaded, &[(Founder::Blind, driver)])
            .expect("the policy supplied again");
    }

    /// **A policy-driven lineage loaded under a *different* centre is refused, not
    /// silently run on the wrong weights.**
    ///
    /// The bool alone tells a trained lineage from a heuristic one; it cannot tell one
    /// trained centre from another. [`Fauna::policy_digest`] (`weights_fnv1a`) is
    /// recorded beside it for exactly this: two GRU centres for the same lineage, same
    /// shape, different weights, so different digests, and the loaded layer's recorded
    /// digest disagrees with the second one it is offered.
    #[test]
    fn a_policy_driven_lineage_under_a_different_centre_is_refused_on_load() {
        use cubarium_search::es::tensor::initial_center_shape;
        use cubarium_voxel_fauna::StartingStores;

        let world = crate::voxel::scene::authored(cubarium_voxel::Config {
            width: 32,
            height: 16,
            depth: 4,
            ..cubarium_voxel::Config::default()
        });
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        crate::voxel::habitat::install_heuristics(&mut fauna);
        let placed = (0..32i64).any(|x| {
            fauna.apply(
                &world,
                FaunaCommand::IntroduceFounder {
                    x,
                    z: 1,
                    founder: Founder::Blind,
                    stores: StartingStores::HUNGRY,
                    heading_rad: 0.0,
                },
            )
        });
        assert!(placed, "a founder body stands somewhere on the strip");

        let theta_a = initial_center_shape::<23, 3>(11);
        let driver_a = EpisodeDriver::gru(&theta_a, Founder::Blind).expect("a valid centre");
        install_founder_controllers(&mut fauna, &[(Founder::Blind, driver_a)])
            .expect("the first centre installs");
        assert!(fauna.policy_driven(Founder::Blind));

        let mut loaded = Fauna::load(&fauna.save()).expect("the layer round-trips");
        crate::voxel::habitat::install_heuristics(&mut loaded);

        let theta_b = initial_center_shape::<23, 3>(29);
        assert_ne!(theta_a, theta_b, "the two seeds give two different centres");
        let driver_b = EpisodeDriver::gru(&theta_b, Founder::Blind).expect("a valid centre");
        let err = format!(
            "{:#}",
            install_founder_controllers(&mut loaded, &[(Founder::Blind, driver_b)])
                .expect_err("a different centre for the same lineage is refused")
        );
        assert!(err.contains("littershredder"), "{err}");
        assert!(err.contains("different"), "{err}");
    }

    /// **The same centre loads without complaint.** The digest the loaded layer carries
    /// equals the digest of the driver offered again, so the world is accepted — the
    /// refusal above is about a *different* centre, not every reload.
    #[test]
    fn a_policy_driven_lineage_under_the_same_centre_is_accepted_on_load() {
        use cubarium_search::es::tensor::initial_center_shape;
        use cubarium_voxel_fauna::StartingStores;

        let world = crate::voxel::scene::authored(cubarium_voxel::Config {
            width: 32,
            height: 16,
            depth: 4,
            ..cubarium_voxel::Config::default()
        });
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        crate::voxel::habitat::install_heuristics(&mut fauna);
        let placed = (0..32i64).any(|x| {
            fauna.apply(
                &world,
                FaunaCommand::IntroduceFounder {
                    x,
                    z: 1,
                    founder: Founder::Blind,
                    stores: StartingStores::HUNGRY,
                    heading_rad: 0.0,
                },
            )
        });
        assert!(placed, "a founder body stands somewhere on the strip");

        let theta = initial_center_shape::<23, 3>(7);
        let driver = EpisodeDriver::gru(&theta, Founder::Blind).expect("a valid centre");
        install_founder_controllers(&mut fauna, &[(Founder::Blind, driver)])
            .expect("the centre installs");

        let mut loaded = Fauna::load(&fauna.save()).expect("the layer round-trips");
        crate::voxel::habitat::install_heuristics(&mut loaded);

        let same = EpisodeDriver::gru(&theta, Founder::Blind).expect("the same centre again");
        install_founder_controllers(&mut loaded, &[(Founder::Blind, same)])
            .expect("the same centre loads without complaint");
        assert!(loaded.policy_driven(Founder::Blind));
    }

    /// **The ambient run's default is the trained centres, and the binary carries them —
    /// or, when this build refuses one, the heuristic, loudly.**
    ///
    /// The two embedded files are the trainer's own output and validate themselves
    /// against *this* build — schema token, declared lineage, weight count, finite
    /// weights, and the founder-manifest digest. Contract v2 (P5-A) moved the digests, so
    /// on the retrain branch the schema-1 centres are refused and each lineage falls back
    /// to its heuristic until P5-C copies the new centres in; a centre that validates
    /// drives its own lineage.
    #[test]
    fn policy_arguments_name_one_file_per_lineage() {
        let args = |v: &[&str]| -> Vec<String> { v.iter().map(|s| s.to_string()).collect() };
        let got = policy_args(&args(&[
            "voxel_census",
            "6",
            "policy=browser=a.json",
            "preset=default",
            "policy=littershredder=b.json",
        ]))
        .expect("parsed");
        assert_eq!(
            got,
            vec![
                (Founder::Browser, PathBuf::from("a.json")),
                (Founder::Blind, PathBuf::from("b.json")),
            ]
        );
        assert!(policy_args(&args(&["x", "policy=browser"])).is_err(), "no file");
        assert!(policy_args(&args(&["x", "policy=hunter=a.json"])).is_err(), "no lineage");
        assert!(
            policy_args(&args(&["x", "policy=browser=a", "policy=frondgrazer=b"])).is_err(),
            "one per lineage"
        );
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        let err = install_founders_with(
            &mut fauna,
            &[(Founder::Browser, PathBuf::from("/nonexistent/centre.json"))],
        )
        .expect_err("a missing file is refused, not replaced by the built-in");
        assert!(format!("{err:#}").contains("centre.json"), "{err:#}");
    }

    #[test]
    fn the_built_in_centres_drive_their_own_lineage_by_default() {
        use clap::Parser;

        // A lineage with no built-in centre (the lanternjaw, until P2) runs its heuristic.
        let has_centre =
            |founder: Founder| BUILT_IN_POLICIES.iter().any(|(f, _, _)| *f == founder);
        let validates: Vec<bool> = Founder::ALL
            .iter()
            .map(|&founder| match built_in_driver(founder) {
                _ if !has_centre(founder) => false,
                Ok(driver) => {
                    assert!(
                        driver.name().contains("gru"),
                        "{} runs a trained centre, not a control: {}",
                        founder.name(),
                        driver.name()
                    );
                    true
                }
                Err(e) => {
                    assert!(
                        format!("{e:#}").contains("digest"),
                        "{}: refused for its manifest digest, not something else: {e:#}",
                        founder.name()
                    );
                    false
                }
            })
            .collect();

        let voxel = |args: &[&str]| -> crate::cli::Voxel {
            let mut line = vec!["cubarium", "voxel"];
            line.extend(args.iter().copied());
            match crate::cli::Cli::parse_from(line).command {
                crate::cli::Command::Voxel(v) => v,
                other => panic!("expected a voxel command, got {other:?}"),
            }
        };

        // No flags: every lineage is driven by its built-in centre, or says loudly that
        // it fell back.
        let recipes = founder_recipes(&voxel(&[])).expect("the default run");
        assert_eq!(recipes.len(), Founder::ALL.len());
        for recipe in &recipes {
            if !has_centre(recipe.founder) {
                assert!(recipe.driver.is_none(), "{}", recipe.source);
                continue;
            }
            assert_eq!(
                recipe.driver.is_some(),
                validates[recipe.founder.index()],
                "{}: {}",
                recipe.founder.name(),
                recipe.source
            );
            assert!(recipe.source.contains("built-in"), "{}", recipe.source);
            if recipe.driver.is_none() {
                assert!(recipe.source.contains("FALLBACK"), "{}", recipe.source);
            }
        }

        // `--founder-heuristic all` is the control: no driver for either lineage, so the
        // seeder's heuristic stands and `install_founder_controllers` records nothing.
        let recipes = founder_recipes(&voxel(&["--founder-heuristic", "all"])).expect("control");
        assert!(recipes.iter().all(|r| r.driver.is_none()));
        assert!(
            recipes
                .iter()
                .all(|r| r.source.contains("--founder-heuristic"))
        );

        // One lineage on the control, the other still on its centre.
        let recipes =
            founder_recipes(&voxel(&["--founder-heuristic", "blind"])).expect("one lineage");
        let blind = recipes
            .iter()
            .find(|r| r.founder == Founder::Blind)
            .expect("the blind lineage");
        let browser = recipes
            .iter()
            .find(|r| r.founder == Founder::Browser)
            .expect("the browser lineage");
        assert!(blind.driver.is_none(), "{}", blind.source);
        assert!(
            blind.source.contains("--founder-heuristic"),
            "{}",
            blind.source
        );
        assert_eq!(
            browser.driver.is_some(),
            validates[Founder::Browser.index()],
            "{}",
            browser.source
        );
    }

    #[test]
    fn a_terrain_load_is_refused_while_the_ecology_is_standing() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let proj = Projection::new(30.0, 4, 0, &c).unwrap();
        let path =
            std::env::temp_dir().join(format!("cubarium-voxel-terrain-{}.bin", std::process::id()));
        let at = path.to_str().expect("a utf-8 temp path").to_string();

        let mut world = World::empty(c.clone());
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        let mut ctl = Control::new(1.0, proj);
        assert_eq!(
            ecology_state(&flora, &fauna),
            None,
            "a fresh run holds no ecology"
        );

        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("save-terrain {at}"));
        assert!(path.exists(), "the terrain was saved");

        // A cell edited after the save, so a load that happened would be visible.
        ctl.handle(&mut world, &mut flora, &mut fauna, "m 3 2 1 soil");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Soil);

        // Something alive. `f` seeds a stand on the column's highest support face.
        ctl.handle(&mut world, &mut flora, &mut fauna, "f 3 1 bloomcrown");
        assert_eq!(flora.view().stands.len(), 1, "a stand is standing");
        assert!(ecology_state(&flora, &fauna).unwrap().contains("stand"));

        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("l {at}"));
        assert_eq!(
            world.view().material_at(3, 2, 1),
            Material::Soil,
            "the load was refused and the world was left alone"
        );
        assert_eq!(flora.view().stands.len(), 1, "and so was the stand");

        // Ground and a nonzero ledger are enough on their own: clearing the stand leaves
        // both, and the refusal stands.
        ctl.handle(&mut world, &mut flora, &mut fauna, "c 3 1");
        assert!(flora.view().stands.is_empty());
        let why = ecology_state(&flora, &fauna).expect("the ground and the ledger remain");
        assert!(why.contains("ground") || why.contains("ledger"), "{why}");
        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("l {at}"));
        assert_eq!(
            world.view().material_at(3, 2, 1),
            Material::Soil,
            "still refused"
        );

        // The same bytes into a run with no ecology in it: accepted, and the edit is gone.
        let mut fresh_flora = Flora::new(FloraConfig::default());
        let mut fresh_fauna = Fauna::new(FaunaConfig::placeholder());
        ctl.handle(
            &mut world,
            &mut fresh_flora,
            &mut fresh_fauna,
            &format!("l {at}"),
        );
        assert_eq!(
            world.view().material_at(3, 2, 1),
            Material::Air,
            "the terrain came back as it was saved"
        );

        // And an animal alone is enough to refuse.
        ctl.handle(
            &mut world,
            &mut fresh_flora,
            &mut fresh_fauna,
            "m 3 2 1 soil",
        );
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        assert!(fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder {
                x: 3,
                z: 1,
                founder: Founder::Browser,
                stores: cubarium_voxel_fauna::StartingStores::FULL,
                heading_rad: 0.0,
            }
        ));
        let mut flora = Flora::new(FloraConfig::default());
        assert!(ecology_state(&flora, &fauna).unwrap().contains("animal"));
        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("l {at}"));
        assert_eq!(
            world.view().material_at(3, 2, 1),
            Material::Soil,
            "refused for the animal"
        );

        let _ = std::fs::remove_file(&path);
        // Both help texts say what the two commands carry.
        assert!(COMMANDS.contains("terrain"), "{COMMANDS}");
    }

    /// Terrain editing from stdin: paused, `m` changes one cell, and the `i` that follows
    /// reads the new material off the same view it prints from. The interaction the local
    /// tool was missing — you can now dig a channel and watch the pool find it.
    #[test]
    fn a_paused_terrain_edit_changes_the_cell_the_next_inspect_reports() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let mut world = World::empty(c.clone());
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        let mut ctl = Control::new(1.0, Projection::new(30.0, 4, 0, &c).unwrap());

        ctl.handle(&mut world, &mut flora, &mut fauna, "p");
        assert!(ctl.paused);
        assert_eq!(world.view().material_at(3, 2, 1), Material::Air);

        ctl.handle(&mut world, &mut flora, &mut fauna, "m 3 2 1 soil");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Soil);
        // What the following `i` prints is this, off the edited world.
        let seen = cell_state(&world, 3, 2, 1);
        assert!(seen.contains("(3, 2, 1) Soil"), "{seen}");
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 3 2 1");

        // `x` wraps, as everything in the ring does.
        ctl.handle(&mut world, &mut flora, &mut fauna, "m -13 2 1 rock");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Rock);

        // A `y` or `z` outside the world, an unknown material and a short line are all
        // refused, and leave the cell alone.
        for bad in [
            "m 3 99 1 air",
            "m 3 2 9 air",
            "m 3 2 1 lava",
            "m 3 2 1",
            "m",
            "material",
        ] {
            ctl.handle(&mut world, &mut flora, &mut fauna, bad);
            assert_eq!(
                world.view().material_at(3, 2, 1),
                Material::Rock,
                "`{bad}` edited a cell"
            );
        }

        ctl.handle(&mut world, &mut flora, &mut fauna, "m 3 2 1 air");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Air);
        // Both help texts really do offer the command that was just used.
        assert!(COMMANDS.contains("m X Y Z air|rock|soil|bedrock"));
    }

    /// `f`, `c` and the flora half of `i` parse: a founder lands on the highest support
    /// of the column named, a missing wood means a full-grown one, every bad line is
    /// refused without touching the stands, and `c` takes the stand back off again.
    #[test]
    fn the_flora_commands_seed_clear_and_inspect_exactly_what_they_name() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let mut world = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                for y in 0..=2 {
                    world.apply(VoxelCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        let mut ctl = Control::new(1.0, Projection::new(30.0, 4, 0, &c).unwrap());
        let site = Site { x: 3, y: 2, z: 1 };

        // A founder with no wood named is full-grown: the stand this command exists to
        // put in the picture is one you can see.
        ctl.handle(&mut world, &mut flora, &mut fauna, "f 3 1 bloomcrown");
        let stand = *flora
            .view()
            .stand_at(site)
            .expect("a stand on the column's skyline");
        assert_eq!(stand.species, Species::Bloomcrown);
        assert_eq!(stand.wood, flora.config().bloomcrown.wood_max);
        assert_eq!(flora.view().stands.len(), 1);

        // An explicit wood is taken, and `x` wraps as everything in the ring does.
        ctl.handle(
            &mut world,
            &mut flora,
            &mut fauna,
            "f -12 0 umbrellafrond 0.25",
        );
        let other = *flora
            .view()
            .stand_at(Site { x: 4, y: 2, z: 0 })
            .expect("the wrapped column");
        assert_eq!(other.species, Species::Umbrellafrond);
        assert_eq!(other.wood, 0.25);

        // Everything refusable: a second stand on one site, an unknown species, a `z`
        // outside the world, wood below `alive_min`, a bad number, a short line.
        for bad in [
            "f 3 1 bloomcrown",
            "f 5 1 lichen",
            "f 5 9 bloomcrown",
            "f 5 1 bloomcrown 0.0001",
            "f 5 1 bloomcrown not-a-number",
            "f 5",
            "f",
            "flora",
        ] {
            ctl.handle(&mut world, &mut flora, &mut fauna, bad);
            assert_eq!(flora.view().stands.len(), 2, "`{bad}` changed the stands");
        }

        // `i` on the site reports the stand and its ground; on a bare cell it says so.
        // Neither prints anything the run can act on, so the assertion is that the
        // lookups are the ones the printout claims.
        assert!(
            flora.view().ground_at(site).is_some(),
            "seeding creates the ground"
        );
        assert!(flora.view().stand_at(Site { x: 9, y: 2, z: 1 }).is_none());
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 3 2 1");
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 9 2 1");
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 3 9 1");

        // `c` takes the stand off the highest support of its column, once.
        ctl.handle(&mut world, &mut flora, &mut fauna, "c 3 1");
        assert!(flora.view().stand_at(site).is_none());
        assert_eq!(flora.view().stands.len(), 1);
        for bad in ["c 3 1", "c 3 9", "c 3", "c", "clear"] {
            ctl.handle(&mut world, &mut flora, &mut fauna, bad);
            assert_eq!(flora.view().stands.len(), 1, "`{bad}` changed the stands");
        }

        // The removal is booked, not hidden: the residual line the run prints is the
        // seeded material less what `c` took back.
        let fv = flora.view();
        assert!(fv.ledger.removed_organic_out > 0.0);
        assert!((fv.organic() - fv.ledger.expected_organic()).abs() < 1e-9);

        // Both help texts really do offer the commands that were just used.
        assert!(COMMANDS.contains("f X Z bloomcrown|umbrellafrond [wood]"));
        assert!(COMMANDS.contains("c X Z"));
    }

    /// **`save DIR` writes the whole world**: a soil strip with a stand and a grazer,
    /// saved through the console and the on-demand writer, loads back from `DIR` with the
    /// same tick and the same plants and animals. `save` alone names a default directory
    /// with the tick, and `save-terrain` still writes the terrain alone.
    #[test]
    fn the_console_save_writes_the_whole_world_and_it_loads_back() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let mut world = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                for y in 0..=2 {
                    world.apply(VoxelCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        let mut ctl = Control::new(1.0, Projection::new(30.0, 4, 0, &c).unwrap());
        ctl.handle(&mut world, &mut flora, &mut fauna, "f 3 1 bloomcrown");
        ctl.handle(&mut world, &mut flora, &mut fauna, "f 9 1 umbrellafrond");
        ctl.handle(&mut world, &mut flora, &mut fauna, "g 6 1");
        for _ in 0..3 {
            world.step();
        }
        let (stands, animals) = (flora.view().stands.len(), fauna.view().animals.len());
        assert!(stands == 2 && animals == 1, "{stands} stands, {animals} animals");

        // `save` alone: the default directory, named with the tick.
        ctl.handle(&mut world, &mut flora, &mut fauna, "save");
        let default = ctl.save_to.take().expect("a save was asked for");
        assert!(default.starts_with(save::SAVES_DIR));
        assert!(
            default.to_string_lossy().ends_with(&format!("-t{}", world.tick())),
            "{}",
            default.display()
        );

        let dir = StateDir::new("save-on-demand");
        let target = dir.0.join("nested");
        let line = format!("save {}", target.display());
        ctl.handle(&mut world, &mut flora, &mut fauna, &line);
        let asked = ctl.save_to.take().expect("a save was asked for");
        assert_eq!(asked, target);
        let mut writer = SnapshotWriter::new(PathBuf::from(save::SAVES_DIR));
        assert!(writer.save_whole_to(asked, &world, &flora, &fauna));
        writer.wait();

        let world_file = target.join(format!("world-{}.voxel", world.tick()));
        let loaded = World::load(&std::fs::read(&world_file).unwrap()).unwrap();
        assert_eq!(loaded.tick(), world.tick());
        let (f, a) = load_layers(&target, loaded.tick()).expect("the layers load back");
        assert_eq!(f.view().stands.len(), stands);
        assert_eq!(a.view().animals.len(), animals);

        // `save-terrain PATH` is one terrain file, no layers beside it.
        let terrain = dir.0.join("terrain.bin");
        ctl.handle(
            &mut world,
            &mut flora,
            &mut fauna,
            &format!("save-terrain {}", terrain.display()),
        );
        World::load(&std::fs::read(&terrain).unwrap()).unwrap();
        assert!(ctl.save_to.is_none(), "save-terrain is not a whole-world save");
        let layers = std::fs::read_dir(&dir.0)
            .unwrap()
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with("flora-") || n.starts_with("fauna-")
            })
            .count();
        assert_eq!(layers, 0, "terrain only");
    }

    /// `w` then `l` round-trips the world through the snapshot the core writes, and a
    /// world of a different shape is refused rather than drawn into the wrong raster.
    #[test]
    fn save_and_load_round_trip_and_a_mismatched_world_is_refused() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let mut world = scene::authored(c.clone());
        let proj = Projection::new(30.0, 4, 0, &c).unwrap();
        let dir = std::env::temp_dir().join(format!("cubarium-voxel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("world.bin");

        let before = world.view().stored_m3();
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        let mut ctl = Control::new(1.0, proj);
        ctl.handle(
            &mut world,
            &mut flora,
            &mut fauna,
            &format!("save-terrain {}", path.display()),
        );
        assert!(path.exists());

        let mut fresh = World::empty(c.clone());
        ctl.handle(
            &mut fresh,
            &mut flora,
            &mut fauna,
            &format!("l {}", path.display()),
        );
        assert!(
            (fresh.view().stored_m3() - before).abs() < 1e-9,
            "the pool came back"
        );

        // A world of another shape is named and refused.
        let other = dir.join("other.bin");
        let wide = cubarium_voxel::Config {
            width: 32,
            ..c.clone()
        };
        std::fs::write(&other, World::empty(wide).save()).unwrap();
        let mut keep = World::empty(c.clone());
        ctl.handle(
            &mut keep,
            &mut flora,
            &mut fauna,
            &format!("l {}", other.display()),
        );
        assert_eq!(keep.config().width, 16, "the run kept its own world");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn generated_world_randomizes_by_default_and_respects_explicit_seed() {
        use crate::cli::{Voxel, VoxelControllerArg, VoxelSceneArg, VoxelSinkArg};
        let cfg = VoxelConfig::default();
        let make_args = |seed: Option<u64>| Voxel {
            config: None,
            set: Vec::new(),
            params: None,
            sink: VoxelSinkArg::Web,
            scene: VoxelSceneArg::Generated,
            seed,
            arena: None,
            controller: VoxelControllerArg::Heuristic,
            policy: None,
            founder_policy: Vec::new(),
            founder_heuristic: Vec::new(),
            introduce: Vec::new(),
            no_egg_bank: false,
            arena_seed: 1,
            arena_diagnostics: false,
            empty: true, // about loading and seeds, not founding a habitat
            seconds: 0.0,
            speed: 1.0,
            load: None,
            out: PathBuf::from("captures"),
            every: 30,
            fps: 60,
            pin_loop: false,
            web_port: 7393,
            gpu_target: None,
            gpu_capture: None,
            weather_preview: None,
            gpu_web_rate: 0.0,
            gpu_roof_walk: false,
            textures: false,
            weather_force: None,
            serve: None,
        };

        // Explicit seed is respected
        let args = make_args(Some(42));
        let (world, label, resumed, _) = load_or_create_world(&args, &cfg).unwrap();
        assert!(!resumed);
        assert_eq!(world.config().seed, 42);
        assert_eq!(label, "generated (seed 42)");

        // Default seed is randomized (two calls should produce different seeds)
        let args_default1 = make_args(None);
        let args_default2 = make_args(None);
        let (world1, _, _, _) = load_or_create_world(&args_default1, &cfg).unwrap();
        let (world2, _, _, _) = load_or_create_world(&args_default2, &cfg).unwrap();
        // Probability of random seed collision is 1 in 2^64
        assert_ne!(world1.config().seed, world2.config().seed);
    }

    #[test]
    fn load_or_create_world_resumes_saved_state_and_does_not_regenerate() {
        use crate::cli::{Voxel, VoxelControllerArg, VoxelSceneArg, VoxelSinkArg};
        let dir = std::env::temp_dir().join(format!("cubarium_test_state_{}", random_seed()));
        std::fs::create_dir_all(&dir).unwrap();

        let cfg = VoxelConfig::default();
        let base_args = Voxel {
            config: None,
            set: Vec::new(),
            params: None,
            sink: VoxelSinkArg::Web,
            scene: VoxelSceneArg::Generated,
            seed: None,
            arena: None,
            controller: VoxelControllerArg::Heuristic,
            policy: None,
            founder_policy: Vec::new(),
            founder_heuristic: Vec::new(),
            introduce: Vec::new(),
            no_egg_bank: false,
            arena_seed: 1,
            arena_diagnostics: false,
            empty: true, // about loading and seeds, not founding a habitat
            seconds: 0.0,
            speed: 1.0,
            load: Some(dir.clone()),
            out: PathBuf::from("captures"),
            every: 30,
            fps: 60,
            pin_loop: false,
            web_port: 7393,
            gpu_target: None,
            gpu_capture: None,
            weather_preview: None,
            gpu_web_rate: 0.0,
            gpu_roof_walk: false,
            textures: false,
            weather_force: None,
            serve: None,
        };

        // 1. Initial run into empty state dir creates a fresh world
        let (world1, _, resumed1, _) = load_or_create_world(&base_args, &cfg).unwrap();
        assert!(!resumed1, "fresh directory has no snapshot to resume");

        // Save a snapshot with a unique marker (e.g. edited cell)
        let _tick = 100;
        let mut saved_world = world1.clone();
        saved_world.apply(cubarium_voxel::Command::SetMaterial {
            x: 0,
            y: 1,
            z: 0,
            material: cubarium_voxel::Material::Soil,
        });
        save_voxel_snapshot(&dir, &saved_world, None);

        // Verify snapshot was created
        let snap_file = dir.join(format!("world-{}.voxel", saved_world.tick()));
        assert!(snap_file.exists());

        // 2. Second run into same state dir resumes that snapshot without regenerating
        let (resumed_world, label, resumed2, _) = load_or_create_world(&base_args, &cfg).unwrap();
        assert!(resumed2, "must resume from existing snapshot in directory");
        assert!(
            label.contains("world-"),
            "label reflects resumed snapshot: {label}"
        );
        assert_eq!(resumed_world.config().seed, saved_world.config().seed);
        assert_eq!(resumed_world.tick(), saved_world.tick());
        let (_, _, _, layers) = load_or_create_world(&base_args, &cfg).unwrap();
        assert!(
            layers.is_none(),
            "a world saved alone resumes with nothing to restore"
        );

        // 3. Saved with its plants and animals, a resume restores them instead of seeding.
        let flora = Flora::new(FloraConfig::for_voxel_size(0.125));
        let fauna = Fauna::new(FaunaConfig::placeholder());
        save_voxel_snapshot(&dir, &saved_world, Some((&flora, &fauna)));
        let (_, _, _, layers) = load_or_create_world(&base_args, &cfg).unwrap();
        match layers {
            Some(StartLayers::Restored(f, _)) => {
                assert_eq!(
                    f.config().voxel_m,
                    0.125,
                    "the saved plant layer, not a new one"
                )
            }
            _ => panic!("the saved layers are restored"),
        }

        // 4. A damaged layer file falls back to seeding; it does not refuse the world.
        let (flora_file, _) = layer_files(&dir, saved_world.tick());
        std::fs::write(&flora_file, b"not a flora").unwrap();
        let (_, _, resumed3, layers) = load_or_create_world(&base_args, &cfg).unwrap();
        assert!(resumed3 && layers.is_none());

        // 5. Pruning takes a world's layers with it.
        for _ in 0..6 {
            saved_world.step();
            save_voxel_snapshot(&dir, &saved_world, Some((&flora, &fauna)));
        }
        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        for kind in ["world-", "flora-", "fauna-"] {
            let n = names.iter().filter(|n| n.starts_with(kind)).count();
            assert_eq!(n, 5, "{kind}: {names:?}");
        }

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
