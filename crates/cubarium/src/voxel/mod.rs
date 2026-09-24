//! `cubarium voxel`: run a [`cubarium_voxel::World`] at a fixed 20 Hz and draw it as
//! pixel art into a ring raster, through the same sinks the rest of the host uses.
//!
//! The loop is [`crate::run::drive`]'s shape — one [`Clock`], one `encode_raster` per
//! rendered frame, one sink — with two differences: the world is a voxel strip rather
//! than the fixture scenes, and a stdin reader thread lets the run be poked while it is
//! running (pause, single-step, speed, rain, set a cell's material, save, load, inspect,
//! outlet, quit).
//!
//! **`w` and `l` are terrain only** (Astra R9.6). `World::save`/`World::load` carry the
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
pub mod chiplet;
pub mod colours;
pub mod model;
pub mod placement;
pub mod present;
pub mod project;
pub mod stand;
pub mod textures;
pub mod vine;

// The seeded habitat, the authored fixture and the founding loop live in
// `cubarium-voxel-sim` since P5-B, so the search crate founds the worlds this host founds;
// every name is re-exported here unchanged.
pub use cubarium_voxel_sim::found::{
    Founded, HABITAT_TRIES, LAKE_SEED_TRIES, ambient_habitat, found_a_habitat,
    generate_with_a_lake,
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
use cubarium_voxel::{Command as VoxelCommand, Material, PoreBand, ViabilitySpec, World};
use cubarium_voxel_fauna::{
    Command as FaunaCommand, Controller, Fauna, FaunaConfig, FaunaLedger, Founder, Response,
    Senses, Species as Beast,
};
use cubarium_voxel_flora::{
    Command as FloraCommand, Flora, FloraConfig, FloraLedger, Site, Species,
};
use cubarium_voxel_sim::{Arena, Sim, SimConfig};
use serde::{Deserialize, Serialize};

use crate::cli::{Voxel, VoxelArenaArg, VoxelControllerArg, VoxelSceneArg, VoxelSinkArg};
use crate::clock::{Clock, MAX_FPS, MIN_FPS, Step};
use crate::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use crate::sink::{FrameSink, GpuTargetKind, Output, PngSink, WebSink, WorldShape};

use present::VoxelPresenter;
use project::Projection;

/// The stdin commands, in the one place both the banner and the usage line read them
/// from, so a new command cannot be added to only one of the two.
const COMMANDS: &str = "p pause/resume, s step, +/- speed, r [m3] rain, a M3 charge the aquifer (negative withdraws), \
                        h M3 add water to the atmosphere (closed budget only), \
                        m X Y Z air|rock|soil|bedrock set material, \
                        f X Z bloomcrown|umbrellafrond [wood] seed a stand, \
                        c X Z clear a stand, \
                        g X Z frondgrazer [body] introduce an animal, \
                        w PATH save the terrain only, \
                        l PATH load a terrain only (refused once anything is alive), \
                        i X Y Z inspect, o outlet, q quit";

/// Default rain volume for the `r` command, in cubic metres.
const DEFAULT_RAIN_M3: f64 = 1.0;
/// `--speed` is clamped to this range; zero is `p` (pause), not a speed.
const MIN_SPEED: f64 = 1.0 / 64.0;
const MAX_SPEED: f64 = 64.0;

// --- The config file -----------------------------------------------------------------

/// `--config`: the presentation, plus the world to build. Missing fields take the
/// defaults below and unknown fields are errors, so a typo is not silently ignored.
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
#[serde(default, deny_unknown_fields)]
pub struct VoxelConfig {
    /// Elevation of the orthographic view, in degrees. Reaches the picture only as
    /// `round(px_per_voxel · tan(tilt))` whole pixels of lift per voxel of depth. The
    /// chosen camera is 30°.
    pub tilt_degrees: f64,
    /// Pixels per voxel edge. The chosen camera is 4: below that the sprite atlas stops
    /// reading. `"auto"` (stored as [`PX_AUTO`]) is resolved once at start-up by
    /// [`auto_px_per_voxel`]: the largest that fits the output — the desktop's screen, or
    /// the panel — with the whole strip in view.
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
    /// Worker threads for the in-phase splits of the tick
    /// (`cubarium_voxel_sim::SimConfig::threads`); `0` means
    /// [`std::thread::available_parallelism`]. Execution only — it reaches no rule, no
    /// number and no picture. **Placeholder** (`design/backlog.md` §1).
    pub threads: usize,
    /// The world to build. Extents follow the core; the ambient display uses 0.125 m
    /// cells so organisms retain their physical size with more cells of visual detail.
    #[serde(deserialize_with = "ambient_world_config")]
    pub world: cubarium_voxel::Config,
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
            textures_dir: PathBuf::from("assets/voxel-textures"),
            lighting: Lighting::Flat,
            light: LightConfig::default(),
            threads: 0,
            // The shipped `default` landscape, ring and all: a `cubarium voxel` with no
            // TOML generates a staged world, not the ridge. `Config::default()` stays
            // `Landform::Ridge` for fixtures and frozen arenas, and `landform = "ridge"`
            // asks for it here.
            world: cubarium_voxel::Preset::find("default")
                .expect("the shipped presets include `default`")
                .config(),
        }
    }
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
/// levels = 4          # rungs on the ambient ladder, floor to full
/// ambient_gain = 1.4  # what full ambient light multiplies a base colour by
/// ambient_floor = 0.2 # the lowest rung, as a fraction of full
/// ao = 0.5            # how far an AO crease line darkens the light
/// ambient_tint = 0.15 # how far the ambient leans toward the sky's hue
/// sun = [-1.0, 2.0, -1.0] # toward the sun: x right, y up, z into the scene
/// sun_tint = 0.18     # how far a sunlit face leans toward the palette's light, at N·L = 1
/// emission = true     # the dossiers' luminous parts glow (lit tier)
/// glow = 0.5          # one emitting voxel's light at its own 4³ cell, in full-ambient units
/// glow_reach = 3      # 4³ cells the light spreads before it is gone
/// bloom = 0.3         # the emitters' blocky halo: how much of their colour it adds at full step
/// bloom_radius = 2    # voxel cells the halo reaches from the emitter's own cell
/// water_absorb = 0.1  # water absorption a voxel of path, in units of the deep water colour
/// water_reflect = 6.0  # the surface's Fresnel reflectance times this (physical is ~5 %)
/// water_ripple = 0.2   # how far a ripple tilts the quantised surface normal
/// water_hz = 12.0      # the water's animation steps a second
/// water_reflect_cells = 64 # cells a reflected ray is marched before it is sky
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightConfig {
    /// Rungs on the ambient ladder, the floor and full included; at least 2.
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
    /// The lit tier's bloom (package L5, `cubarium_gpu::bloom`): a pixel-art halo around
    /// the emitters, made of whole voxel cells at three strengths (1, 2/3, 1/3 of this
    /// times the emitter's colour, falling off with distance), added to what is behind;
    /// the emitters themselves stay crisp. `0` turns it off. Needs `emission`.
    pub bloom: f32,
    /// How many voxel cells the halo reaches from the emitter's own cell (Euclidean).
    pub bloom_radius: u32,
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
    /// Cells a reflected ray is marched before it counts as sky.
    pub water_reflect_cells: u32,
}

impl Default for LightConfig {
    fn default() -> LightConfig {
        LightConfig {
            levels: 4,
            ambient_gain: 1.4,
            ambient_floor: 0.2,
            ao: 0.5,
            ambient_tint: 0.15,
            sun: [-1.0, 2.0, -1.0],
            sun_tint: 0.18,
            emission: true,
            glow: 0.5,
            glow_reach: 3,
            bloom: 0.3,
            bloom_radius: 2,
            water_absorb: 0.1,
            water_reflect: 6.0,
            water_ripple: 0.2,
            water_hz: 12.0,
            water_reflect_cells: 64,
        }
    }
}

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

/// The desktop's screen in pixels, for `px_per_voxel = "auto"`: `CUBARIUM_SCREEN=WxH`,
/// else Hyprland's focused monitor, else the first connected output `xrandr` reports.
/// `None` when none of them answers.
pub fn desktop_screen() -> Option<(u32, u32)> {
    let wxh = |t: &str| -> Option<(u32, u32)> {
        let (w, h) = t.split_once('x')?;
        Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
    };
    if let Ok(v) = std::env::var("CUBARIUM_SCREEN") {
        return wxh(&v);
    }
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
        let focused = monitors.iter().find(|m| m["focused"].as_bool() == Some(true));
        if let Some(size) = focused.or(monitors.first()).and_then(size) {
            return Some(size);
        }
    }
    let text = run("xrandr", &["--current"])?;
    text.lines()
        .filter(|l| l.contains(" connected"))
        .find_map(|l| l.split_whitespace().find_map(|w| wxh(w.split('+').next()?)))
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
    match model::ModelLibrary::load(&cfg.models_dir, cfg.world.voxel_m) {
        Ok(lib) => {
            eprintln!(
                "cubarium voxel: {} voxel models from {}/{}",
                lib.len(),
                cfg.models_dir.display(),
                model::voxel_dir(cfg.world.voxel_m)
            );
            Some(std::sync::Arc::new(lib))
        }
        Err(e) => {
            eprintln!("cubarium voxel: no voxel models ({e:#}); drawing the glyphs");
            None
        }
    }
}

// A partial [world] table must use the same cell size as an omitted table, which since
// the ambient default became the shipped `default` landscape is that landscape's own
// 0.25 m. `cubarium_voxel::Config::default()` is untouched, so frozen arenas and the
// simulation fixtures keep theirs.
fn ambient_world_config<'de, D>(
    deserializer: D,
) -> std::result::Result<cubarium_voxel::Config, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut table = toml::Table::deserialize(deserializer)?;
    let cell = toml::Value::Float(
        cubarium_voxel::Preset::find("default")
            .expect("the shipped presets include `default`")
            .voxel_m,
    );
    table.entry("voxel_m").or_insert(cell);
    expand_landform_preset(&mut table).map_err(serde::de::Error::custom)?;
    toml::Value::Table(table)
        .try_into()
        .map_err(serde::de::Error::custom)
}

/// Let `[world.landform]` name one of the shipped landscapes:
///
/// ```toml
/// [world.landform]
/// preset = "wide"     # small | default | wide
/// relief_m = 3.0      # any recipe field, overriding the preset
/// ```
///
/// The preset is expanded here, into the plain `landform = { staged = { .. } }` the
/// core's serde already reads, so `cubarium_voxel::Recipe` stays an ordinary struct that
/// postcard can read straight back out of a world snapshot. The extents stay `[world]`'s
/// business: a preset names a landscape, not a ring size.
///
/// Anything else — no `landform` key, `landform = "ridge"`, or a `[world.landform.staged]`
/// table written out in full — passes through untouched.
fn expand_landform_preset(world: &mut toml::Table) -> std::result::Result<(), String> {
    let Some(toml::Value::Table(landform)) = world.get("landform") else {
        return Ok(());
    };
    if !landform.contains_key("preset") {
        return Ok(());
    }
    let mut landform = landform.clone();
    let name = match landform.remove("preset") {
        Some(toml::Value::String(name)) => name,
        Some(other) => {
            return Err(format!(
                "[world.landform] preset must be a name, not {other}"
            ));
        }
        None => unreachable!("the key is there"),
    };
    // A terrarium preset names the other generator: `landform = { terrarium = { .. } }`.
    if let Some(terrarium) = cubarium_voxel::Terrarium::preset(&name) {
        let mut table = match toml::Value::try_from(terrarium) {
            Ok(toml::Value::Table(t)) => t,
            _ => return Err("a terrarium is a table".into()),
        };
        merge_over(&mut table, landform);
        let mut wrapped = toml::Table::new();
        wrapped.insert("terrarium".into(), toml::Value::Table(table));
        world.insert("landform".into(), toml::Value::Table(wrapped));
        return Ok(());
    }
    let preset = cubarium_voxel::Preset::find(&name).ok_or_else(|| {
        let known: Vec<&str> = cubarium_voxel::PRESETS
            .iter()
            .map(|p| p.name)
            .chain(cubarium_voxel::Terrarium::PRESETS.iter().map(|(n, _)| *n))
            .collect();
        format!("no landform preset is called {name:?}; the shipped ones are {known:?}")
    })?;
    let mut staged = match toml::Value::try_from(preset.recipe) {
        Ok(toml::Value::Table(t)) => t,
        _ => return Err("a recipe is a table".into()),
    };
    // Whatever else the table said overrides the preset, field by field.
    merge_over(&mut staged, landform);
    let mut wrapped = toml::Table::new();
    wrapped.insert("staged".into(), toml::Value::Table(staged));
    world.insert("landform".into(), toml::Value::Table(wrapped));
    Ok(())
}

/// Lay `over` onto `base` field by field, into nested tables too: a config that says
/// `[world.landform.water] inventory_m = 2.0` changes that one field of the preset's
/// water and keeps the rest, rather than replacing the whole table with a dry default.
fn merge_over(base: &mut toml::Table, over: toml::Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(inner)), toml::Value::Table(value)) => merge_over(inner, value),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

/// `[world]`: [`cubarium_voxel::Config`] with every field optional.
///
/// Load a [`VoxelConfig`] from TOML.
pub fn load_config(path: &Path) -> Result<VoxelConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the voxel config {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing the voxel config {}", path.display()))
}

// --- The run -------------------------------------------------------------------------

/// Generate a random 64-bit seed from `/dev/urandom` or fallback system time entropy.
fn random_seed() -> u64 {
    let mut buf = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        if f.read_exact(&mut buf).is_ok() {
            let s = u64::from_le_bytes(buf);
            if s != 0 {
                return s;
            }
        }
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EED_0001)
}

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

/// Parse tick from a snapshot filename like `world-1730.voxel`.
fn parse_voxel_tick(name: &str) -> Option<u64> {
    let name = name.strip_prefix("world-")?;
    let name = name.strip_suffix(".voxel")?;
    name.parse().ok()
}

/// Prune older voxel snapshots in `dir`, keeping the newest `keep` snapshots.
fn prune_voxel_snapshots(dir: &Path, keep: usize) {
    let keep = keep.max(1);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut candidates: Vec<(u64, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Some(tick) = parse_voxel_tick(name) {
                candidates.push((tick, p));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, old_path) in candidates.into_iter().skip(keep) {
        let _ = std::fs::remove_file(old_path);
    }
}

/// Atomically write a snapshot to `dir` and prune older snapshots.
///
/// The bytes reach the disk before the rename makes them the snapshot: a
/// `world-N.voxel` is either whole or absent, even across a power cut, and a crash
/// mid-write leaves only a `tmp-` file nothing reads.
fn save_voxel_snapshot(dir: &Path, world: &World) {
    use std::io::Write;
    let tick = world.tick();
    let snap_file = dir.join(format!("world-{tick}.voxel"));
    let tmp_file = dir.join(format!("tmp-{tick}.voxel"));
    let bytes = world.save();
    let written = std::fs::File::create(&tmp_file).and_then(|mut f| {
        f.write_all(&bytes)?;
        f.sync_all()
    });
    if written.is_ok() && std::fs::rename(&tmp_file, &snap_file).is_ok() {
        eprintln!("cubarium voxel: saved snapshot {}", snap_file.display());
        prune_voxel_snapshots(dir, 5);
    } else {
        let _ = std::fs::remove_file(&tmp_file);
    }
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
    fn save_in_background(&mut self, world: &World) -> bool {
        let world = world.clone();
        let dir = self.dir.clone();
        self.spawn(move || save_voxel_snapshot(&dir, &world))
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
    fn save_now(&mut self, world: &World) {
        self.wait();
        save_voxel_snapshot(&self.dir, world);
    }
}

/// A fresh generated world's founding: the layers the founding loop seeded it with.
type Seeding = (Flora, Fauna, habitat::Seeded);

/// Resume from a file/directory or create a fresh world according to CLI and config.
///
/// A **fresh generated** world that is going to be seeded goes through the founding loop
/// ([`found_a_habitat`]) and comes back with its layers, already seeded; every other
/// world comes back bare, `None`, for the caller to seed.
fn load_or_create_world(
    args: &Voxel,
    cfg: &VoxelConfig,
) -> Result<(World, String, bool, Option<Seeding>)> {
    let (world, label, resumed) = match load_world(args)? {
        Some(found) => found,
        None => {
            let world_cfg = cfg.world.clone();
            match args.scene {
                VoxelSceneArg::Authored => {
                    (scene::authored(world_cfg), "authored".to_string(), false)
                }
                VoxelSceneArg::Generated => {
                    let asked = args.seed.or_else(|| cfg_seed_from_file(&args.config));
                    if args.empty {
                        let (world, seed, rejected) =
                            generate_with_a_lake(&world_cfg, asked, LAKE_SEED_TRIES, random_seed);
                        let label = if rejected > 0 {
                            format!("generated (seed {seed}, {rejected} rejected)")
                        } else {
                            format!("generated (seed {seed})")
                        };
                        (world, label, false)
                    } else {
                        let founded = found_a_habitat(
                            &world_cfg,
                            asked,
                            LAKE_SEED_TRIES,
                            HABITAT_TRIES,
                            random_seed,
                            |w: &World| {
                                (
                                    Flora::new(FloraConfig::for_voxel_size(w.config().voxel_m)),
                                    Fauna::new(FaunaConfig::default()),
                                )
                            },
                            habitat::FOUNDER_COUNTS,
                            |s: &habitat::Seeded| s.acceptance.accepted,
                        );
                        let rejected = founded.lake_rejected + founded.habitat_rejected;
                        let label = if rejected > 0 {
                            format!("generated (seed {}, {rejected} rejected)", founded.seed)
                        } else {
                            format!("generated (seed {})", founded.seed)
                        };
                        return Ok((
                            founded.world,
                            label,
                            false,
                            Some((founded.flora, founded.fauna, founded.seeded)),
                        ));
                    }
                }
            }
        }
    };
    Ok((world, label, resumed, None))
}

/// Resume a world from `--load`, if there is one to resume: `None` means found a new one.
fn load_world(args: &Voxel) -> Result<Option<(World, String, bool)>> {
    if let Some(path) = &args.load {
        if path.is_file() {
            let bytes = std::fs::read(path)
                .with_context(|| format!("reading the world {}", path.display()))?;
            let world =
                World::load(&bytes).with_context(|| format!("loading {}", path.display()))?;
            let label = path.display().to_string();
            eprintln!("cubarium voxel: resumed world from {}", path.display());
            return Ok(Some((world, label, true)));
        } else if path.is_dir() {
            let mut candidates: Vec<(u64, std::time::SystemTime, PathBuf)> = Vec::new();
            for entry in std::fs::read_dir(path)
                .with_context(|| format!("reading state directory {}", path.display()))?
            {
                let entry = entry?;
                let p = entry.path();
                if p.is_file() {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.ends_with(".voxel") {
                        let tick = parse_voxel_tick(name).unwrap_or(0);
                        let mtime = entry
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        candidates.push((tick, mtime, p));
                    }
                }
            }
            candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
            if !candidates.is_empty() {
                // A refusal is *stale* when the file is a perfectly good snapshot of a
                // format this build no longer speaks, and *damaged* when it is anything
                // else. Told apart by the error's own type, never by its text.
                let mut stale: Vec<(PathBuf, u32)> = Vec::new();
                for (tick, _, cand_path) in &candidates {
                    match std::fs::read(cand_path) {
                        Ok(bytes) => match World::load(&bytes) {
                            Ok(world) => {
                                eprintln!(
                                    "cubarium voxel: resuming {} at tick {tick}",
                                    cand_path.display()
                                );
                                let label = format!("resumed from {}", cand_path.display());
                                return Ok(Some((world, label, true)));
                            }
                            Err(e) => {
                                eprintln!(
                                    "cubarium voxel: skipping corrupt snapshot {}: {e}",
                                    cand_path.display()
                                );
                                if let Some(m) = e.downcast_ref::<cubarium_voxel::SchemaMismatch>()
                                {
                                    stale.push((cand_path.clone(), m.found));
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!(
                                "cubarium voxel: skipping unreadable snapshot {}: {e}",
                                cand_path.display()
                            );
                        }
                    }
                }
                // **Stale is not damaged.** A directory whose every snapshot is simply of
                // an older format holds nothing that can be recovered — the standing rule
                // is fresh, never migrate — and refusing to start leaves the panel showing
                // the last frame it drew until somebody ssh's in. That is exactly what it
                // did: five worlds of schema 13 against a build that speaks 15, and a
                // crash loop (Wrysk, 2026-09-21, who does not want them kept). So they are
                // discarded, loudly, and a fresh world founded. Anything else still
                // refuses by name below.
                if stale.len() == candidates.len() {
                    let mut had: Vec<u32> = stale.iter().map(|(_, n)| *n).collect();
                    had.sort_unstable();
                    had.dedup();
                    let names: Vec<String> = had.iter().map(|n| n.to_string()).collect();
                    for (p, _) in &stale {
                        std::fs::remove_file(p).with_context(|| {
                            format!("discarding the stale snapshot {}", p.display())
                        })?;
                    }
                    eprintln!(
                        "cubarium voxel: discarded {} snapshot(s) of schema {} in {}; \
                         founding a fresh world (schema {})",
                        stale.len(),
                        names.join(", "),
                        path.display(),
                        cubarium_voxel::snapshot::SCHEMA,
                    );
                } else {
                    bail!(
                        "{} snapshot file(s) are present in {} and none of them loaded",
                        candidates.len(),
                        path.display()
                    );
                }
            }
            eprintln!(
                "cubarium voxel: no loadable snapshot in {}; creating a new world",
                path.display()
            );
        } else {
            // Path does not exist yet
            if path.extension().is_some() {
                bail!("cannot load world from {}: file not found", path.display());
            } else {
                std::fs::create_dir_all(path)
                    .with_context(|| format!("creating state directory {}", path.display()))?;
                eprintln!(
                    "cubarium voxel: created state directory {}; creating a new world",
                    path.display()
                );
            }
        }
    }

    Ok(None)
}

/// Run `cubarium voxel`.
pub fn run_voxel(args: &Voxel, stop: &AtomicBool) -> Result<()> {
    let mut cfg = match &args.config {
        Some(path) => load_config(path)?,
        None => VoxelConfig::default(),
    };
    args.validate()?;
    if args.sink == VoxelSinkArg::Png && args.seconds <= 0.0 {
        bail!("`--sink png` needs `--seconds N`, or it would capture until interrupted");
    }
    // `World::new` and `World::empty` panic on a config no world can be built from —
    // building one from nonsense is a programming error. A config *file* is input, so the
    // boundary is here: a bad file is reported, not a backtrace.
    cfg.world.validate().with_context(|| match &args.config {
        Some(path) => format!("the `[world]` in {}", path.display()),
        None => "the built-in world defaults".to_string(),
    })?;
    // Before any pool exists and before the thread count is read, so both follow it.
    if !args.pin_loop && !args.all_chiplets {
        chiplet::keep_to_one_chiplet();
    }
    if args.textures {
        cfg.textures = true;
    }
    if cfg.px_per_voxel == PX_AUTO {
        // Once, here: the renderer's projection is fixed for the life of the process.
        let panel = args.sink == VoxelSinkArg::Gpu
            && args.gpu_target.unwrap_or_else(GpuTargetKind::detect) == GpuTargetKind::Shim;
        let (out, what) = if panel {
            let (w, h) = cubarium_gpu::target::SHIM_PANEL;
            ((h, w), "the panel")
        } else {
            match desktop_screen() {
                Some(size) => (size, "the screen"),
                None => ((1920, 1080), "an assumed 1920x1080 screen (none detected)"),
            }
        };
        cfg.px_per_voxel = auto_px_per_voxel(cfg.tilt_degrees, cfg.raster_height, &cfg.world, out);
        eprintln!(
            "cubarium voxel: px_per_voxel auto -> {} (fits {what}, {}x{})",
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
    let sim_config = SimConfig {
        threads: if cfg.threads == 0 {
            cubarium_voxel::default_threads()
        } else {
            cfg.threads
        },
    };
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
                &cfg.world,
            )?;
            let mut out = open_out(args, &cfg, proj, speed)?;
            // The GPU draws whatever was last staged, and nothing has been staged yet:
            // an empty world of this config makes the founding frame sky over nothing
            // rather than whatever the staging buffer happened to hold.
            out.stage_empty(&cfg.world);
            Some(out)
        }
    };

    let (mut sim, scene_label) = match args.arena {
        Some(arena) => {
            let (founder, driver) = sensing_driver(args, arena)?;
            let mut arena = Arena::build(founder, args.arena_seed);
            let senses = arena.prepare_senses();
            let animal_id = arena
                .animal_id
                .ok_or_else(|| anyhow::anyhow!("sensing arena did not place its founder"))?;
            let controller = driver.fresh();
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
                arena.into_sim_prepared(SimConfig { threads: 1 }, senses),
                format!("sensing {} arena seed {}", founder.name(), args.arena_seed),
            )
        }
        None => found_while_presenting(
            out.as_mut().expect("the ordinary run opens its output first"),
            args.fps,
            stop,
            || {
            let (world, scene_label, resumed, founded) = load_or_create_world(args, &cfg)?;
            let mut world = world;
            let recipes = founder_recipes(args)?;
            let drivers: Vec<(Founder, EpisodeDriver)> = recipes
                .iter()
                .filter_map(|r| r.driver.clone().map(|d| (r.founder, d)))
                .collect();
            // A fresh generated world comes out of the founding loop **already seeded**
            // and judged; an authored or resumed one is pre-rolled and seeded here — a
            // resumed one without the opening shower, which it has had, and never redrawn.
            let (flora, mut fauna, seeded) = match founded {
                Some((flora, fauna, seeded)) => (flora, fauna, Some(seeded)),
                None => {
                    let mut flora =
                        Flora::new(FloraConfig::for_voxel_size(world.config().voxel_m));
                    let mut fauna = Fauna::new(FaunaConfig::default());
                    let seeded = (!args.empty).then(|| {
                        let pre = habitat::pre_roll(&mut world, !resumed);
                        habitat::seed_pre_rolled(
                            &mut world,
                            &mut flora,
                            &mut fauna,
                            &pre,
                            habitat::FOUNDER_COUNTS,
                        )
                    });
                    (flora, fauna, seeded)
                }
            };
            if let Some(seeded) = seeded {
                // The settle report, once, at startup: how long the water took to stop
                // moving and whether it did. The ordinary display shows none of this.
                let st = seeded.settle;
                let wc = world.config().clone();
                let lake = cubarium_voxel::hydrate::lake(&world);
                let above = cubarium_voxel::hydrate::tier_pools(
                    &cubarium_voxel::hydrate::pools(&world),
                    lake.level_y,
                );
                eprintln!(
                    "cubarium voxel: lake {:.2} m³ over {:.1} m², {:.1} m² visible, \
                     {above} pool(s) above it, stream {:.4} m³/s",
                    lake.volume_m3,
                    lake.surface_cells.len() as f64 * world.config().cell_area(),
                    lake.visible_m2,
                    world.config().reentry_m3_per_s,
                );
                eprintln!(
                    "cubarium voxel: {} — water settled in {} ticks ({}), {:.2} m³ pooled \
                     in {} cells, {:.2} m³ pore, drift {:.2e} m³/100 ticks{}",
                    if wc.closed_water_budget {
                        format!(
                            "closed cycle, {:.2} m³ aloft, {}, {:.0} mm/h rain, \
                             {:.0} mm/h evaporation",
                            world.atmosphere_m3(),
                            if wc.shower_interval_max_s > 0.0 {
                                // Scheduled: the calendar says when, and the fraction is
                                // only the floor the store has to clear to pay for it.
                                format!(
                                    "showers every {:.0}-{:.0} min above a {:.0} % floor",
                                    wc.shower_interval_min_s / 60.0,
                                    wc.shower_interval_max_s / 60.0,
                                    wc.shower_trigger_fraction * 100.0,
                                )
                            } else {
                                format!(
                                    "showers at {:.0} % of total",
                                    wc.shower_trigger_fraction * 100.0
                                )
                            },
                            wc.rain_m_per_s * 3600.0 * 1000.0,
                            wc.evaporation_m_per_s * 3600.0 * 1000.0,
                        )
                    } else {
                        "open budget, no cycle to report".to_string()
                    },
                    st.ticks,
                    if st.converged {
                        "converged"
                    } else {
                        "still moving at the cap"
                    },
                    st.pooled_m3,
                    st.free_cells,
                    st.pore_m3,
                    st.drift_m3_per_100,
                    if st.dry_locked {
                        " — DRY LOCKED: water aloft and no pool on the ground"
                    } else {
                        ""
                    },
                );
                eprintln!(
                    "cubarium voxel: seeded the example habitat — {} stands, {} logs, \
                     {} litter tiles, {} littershredders, {} frondgrazer founders \
                     (--empty for a bare world)",
                    seeded.stands,
                    seeded.logs,
                    seeded.litter_tiles,
                    seeded.founders[Founder::Blind.index()],
                    seeded.founders[Founder::Browser.index()],
                );
                for founder in Founder::ALL {
                    let short = seeded.shortfall[founder.index()];
                    if short > 0 {
                        eprintln!(
                            "cubarium voxel: {} shortfall — {short} bodies not placed; \
                             this habitat holds no site the lineage can live on",
                            founder.name()
                        );
                    }
                }
                let pr = seeded.pre_roll;
                eprintln!(
                    "cubarium voxel: pre-roll {} ticks — {}; habitat {}",
                    pr.ticks,
                    if pr.opening_shower {
                        format!(
                            "opened with a {}-tick shower, drained in {} ticks ({})",
                            pr.shower_ticks,
                            pr.drain.ticks,
                            if pr.drain.converged { "converged" } else { "at the cap" },
                        )
                    } else {
                        "no opening shower".to_string()
                    },
                    if seeded.acceptance.accepted {
                        "accepted".to_string()
                    } else {
                        format!("NOT accepted: {}", seeded.acceptance.reasons())
                    },
                );
            } else {
                // An empty world grows its founders by hand; the lineages still need
                // their recipes registered so anything born is driven.
                habitat::install_heuristics(&mut fauna);
            }
            install_founder_controllers(&mut fauna, &drivers)?;
            // Say what is actually driving each lineage, every run: the default is now a
            // trained centre, and a trained animal and a reflex animal are told apart
            // from outside only by being told apart here.
            for recipe in &recipes {
                eprintln!(
                    "cubarium voxel: {} founders are driven by {} ({})",
                    recipe.founder.name(),
                    match &recipe.driver {
                        Some(driver) => driver.name(),
                        None => "the observation-only heuristic".to_string(),
                    },
                    recipe.source,
                );
            }
            // The live schedule's own sensory state, settled against the layers as they
            // stand before the first tick: the ambient world's founders smell the litter
            // this flora actually drops and has eaten, at the field's own cadence. An
            // `--empty` world has no litter, so the field is empty and its update is a
            // walk over nothing.
            let mut senses = Senses::new();
            senses.settle(&world.view(), &flora.view());
            Ok((
                Sim::new(world, flora, fauna, sim_config, Some(senses)),
                scene_label,
            ))
            },
        )?,
    };

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
        Some(out) if same_extent(&cfg.world, sim.world().config()) => out,
        Some(old) => {
            let c = sim.world().config();
            eprintln!(
                "cubarium voxel: the world in hand is {}x{}x{}, not the {}x{}x{} the \
                 config draws; reopening the output on it",
                c.width,
                c.height,
                c.depth,
                cfg.world.width,
                cfg.world.height,
                cfg.world.depth,
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
    // Under a closed budget the outlet is not an export: what it takes goes into the
    // atmosphere store and comes back as a shower. It is the strongest of the two return
    // flows, so a closed world starts with it open — otherwise the cycle never starts and
    // the run reports a world that is only standing still. `o` still toggles it.
    if sim.world().config().closed_water_budget && !sim.world().outlet_open() {
        sim.world_mut()
            .apply(VoxelCommand::SetOutlet { open: true });
        eprintln!(
            "cubarium voxel: closed water budget — the outlet is open as the return flow \
             into the atmosphere, not an export"
        );
    }
    // Off the loop: the run starts now, and this line arrives when it arrives.
    let _probe = report_water_cycle(sim.world(), sim.flora(), sim.config().threads);
    eprintln!("cubarium voxel: stdin commands — {COMMANDS}");

    let commands = spawn_stdin_reader();

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
    let mut debt = 0.0f64;
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
        s.placement = placed;
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
        }
        if ctl.quit {
            break;
        }

        match clock.next_step(now) {
            Step::Tick => {
                if ctl.paused {
                    // A single `s` still advances exactly one world tick.
                    if ctl.pending_steps > 0 {
                        ctl.pending_steps -= 1;
                        let (at, before) = (Instant::now(), Phases::now());
                        sim.step();
                        budget.tick(at.elapsed().as_nanos() as u64, before);
                        ticks += 1;
                        since_ticks += 1;
                        moved = true;
                    }
                } else {
                    debt += ctl.speed;
                    while debt >= 1.0 {
                        debt -= 1.0;
                        let (at, before) = (Instant::now(), Phases::now());
                        sim.step();
                        budget.tick(at.elapsed().as_nanos() as u64, before);
                        ticks += 1;
                        since_ticks += 1;
                        moved = true;
                    }
                }
                out.observe_tick(sim.world().tick());
                if let Some(s) = snapshots.as_mut()
                    && ticks > 0
                    && ticks % 1200 == 0
                {
                    let (world, _, _) = sim.layers();
                    s.save_in_background(world);
                }
            }
            Step::Render { f } => {
                let (world, flora, fauna) = sim.layers();
                // A pack the renderer refused is still owed: the flag goes straight back
                // up rather than being lost with the tick that set it.
                let at = Instant::now();
                moved = out.render(world, flora, fauna, std::mem::take(&mut moved), f)?;
                budget.frame(at.elapsed().as_nanos() as u64);
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
    if let Some(s) = snapshots.as_mut() {
        let (world, _, _) = sim.layers();
        s.save_now(world);
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

/// The trained centres the ambient run installs by default, carried inside the binary so
/// the live world needs no files on disk. Provenance, and the rule that a centre is
/// replaced and never edited, are in `crates/cubarium/assets/policies/README.md`.
const BUILT_IN_POLICIES: [(Founder, &str, &str); 2] = [
    (
        Founder::Blind,
        "littershredder-p3c-wander-gen441.json",
        include_str!("../../assets/policies/littershredder-p3c-wander-gen441.json"),
    ),
    (
        Founder::Browser,
        "frondgrazer-p3d-reach-gen390.json",
        include_str!("../../assets/policies/frondgrazer-p3d-reach-gen390.json"),
    ),
];

/// Parse one embedded centre into a driver, with the same checks [`VoxelPolicyFile::load`]
/// makes of a file on disk: the schema token first, then the file's own validation of
/// lineage, weight count, finite weights and this build's founder-manifest digest.
///
/// A centre that no longer matches this build's manifest fails here rather than being
/// reinterpreted, which is the point of shipping the trainer's own file untouched: the
/// binary refuses to pretend that weights trained against another schema are its default.
fn built_in_driver(founder: Founder) -> Result<EpisodeDriver> {
    let (_, name, json) = BUILT_IN_POLICIES
        .iter()
        .find(|(f, _, _)| *f == founder)
        .ok_or_else(|| anyhow::anyhow!("no built-in centre for the {} lineage", founder.name()))?;
    let file: VoxelPolicyFile = serde_json::from_str(json)
        .with_context(|| format!("the built-in centre {name} is not a policy file"))?;
    anyhow::ensure!(
        file.schema == cubarium_search::es::voxel::store::POLICY_SCHEMA,
        "the built-in centre {name}: schema `{}` is not {}",
        file.schema,
        cubarium_search::es::voxel::store::POLICY_SCHEMA,
    );
    let declared = file
        .founder()
        .map_err(|e| anyhow::anyhow!("the built-in centre {name}: {e}"))?;
    anyhow::ensure!(
        declared == founder,
        "the built-in centre {name} declares the {} lineage, not {}",
        declared.name(),
        founder.name(),
    );
    file.driver()
        .map_err(|e| anyhow::anyhow!("the built-in centre {name}: {e}"))
}

/// The built-in centre for `founder` if it validates against this build, else `None`
/// with a **loud** line on stderr: the lineage falls back to the observation-only
/// heuristic the seeder registered.
///
/// Contract v2 (`design/handoffs/voxel-retrain-2026-09-22.md`, P5-A item 10) moved both
/// founder manifests' digests, so the centres shipped under schema 1 are refused until the
/// retrain (P5-C) replaces them. A refused centre is never reinterpreted and never
/// silently swapped: the run says which lineage is on the fallback and why.
fn built_in_or_fallback(founder: Founder) -> Option<EpisodeDriver> {
    match built_in_driver(founder) {
        Ok(driver) => Some(driver),
        Err(e) => {
            eprintln!(
                "WARNING: {e:#}\nWARNING: the {} founders fall back to the observation-only \
                 heuristic: the built-in centre was trained against another founder manifest \
                 and is refused, not reinterpreted.",
                founder.name()
            );
            None
        }
    }
}

/// Install the ambient run's built-in trained centres on every founder lineage of an
/// already-seeded layer, and say so on stderr — one line per lineage naming its driver,
/// the same announcement `cubarium voxel`'s ambient run makes by default. An example that
/// wants to measure the shipped world's founders, and not the seeder's bare
/// observation-only heuristics, calls this once after `habitat::seed` (or
/// `habitat::install_heuristics` on an empty world), before stepping the sim.
///
/// A lineage whose built-in centre this build refuses keeps the seeder's heuristic, with
/// a loud line saying so ([`built_in_or_fallback`]).
pub fn install_default_founders(fauna: &mut Fauna) -> Result<()> {
    install_founders_with(fauna, &[])
}

/// [`install_default_founders`], with each lineage named in `policies` driven by that
/// saved policy file instead of the built-in centre (P5-C: `policy=<lineage>=<file>` on
/// the examples, so a candidate runs in the live tools). The file validates itself
/// against this build exactly as `--founder-policy` does, and a file declaring another
/// lineage than the one it is named for is refused.
pub fn install_founders_with(fauna: &mut Fauna, policies: &[(Founder, PathBuf)]) -> Result<()> {
    let mut drivers = Vec::with_capacity(Founder::ALL.len());
    let mut sources = Vec::with_capacity(Founder::ALL.len());
    for founder in Founder::ALL {
        if let Some((_, path)) = policies.iter().find(|(f, _)| *f == founder) {
            let file = VoxelPolicyFile::load(path).map_err(anyhow::Error::msg)?;
            let declared = file.founder().map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                declared == founder,
                "policy={}={}: the file declares the {} lineage",
                founder.name(),
                path.display(),
                declared.name(),
            );
            drivers.push((founder, file.driver().map_err(anyhow::Error::msg)?));
            sources.push(format!("the policy file {}", path.display()));
        } else if let Some(driver) = built_in_or_fallback(founder) {
            drivers.push((founder, driver));
            sources.push("the built-in centre".to_string());
        }
    }
    install_founder_controllers(fauna, &drivers)?;
    for ((founder, driver), source) in drivers.iter().zip(&sources) {
        eprintln!(
            "{} founders are driven by {} ({source})",
            founder.name(),
            driver.name(),
        );
    }
    Ok(())
}

/// The examples' `policy=<lineage>=<file>` arguments, in order: one per lineage at most.
pub fn policy_args(args: &[String]) -> Result<Vec<(Founder, PathBuf)>> {
    let mut out: Vec<(Founder, PathBuf)> = Vec::new();
    for spec in args.iter().filter_map(|a| a.strip_prefix("policy=")) {
        let Some((name, path)) = spec.split_once('=') else {
            bail!("policy= takes `<lineage>=<file>`, not `{spec}`");
        };
        let founder = cubarium_search::es::voxel::parse_founder(name)
            .map_err(|e| anyhow::anyhow!("policy={spec}: {e}"))?;
        anyhow::ensure!(!path.trim().is_empty(), "policy={spec} names no file");
        anyhow::ensure!(
            !out.iter().any(|(f, _)| *f == founder),
            "policy= names {} twice; one policy per lineage",
            founder.name()
        );
        out.push((founder, PathBuf::from(path.trim())));
    }
    Ok(out)
}

/// What one founder lineage of the ambient run is driven by, and how it was chosen. The
/// source is carried so the run can say it out loud: a world whose animals are a trained
/// policy and a world whose animals are a reflex look alike from outside.
struct FounderRecipe {
    founder: Founder,
    /// `None` is the observation-only heuristic the seeder registered.
    driver: Option<EpisodeDriver>,
    source: String,
}

/// Choose each lineage's driver for the ambient run: `--founder-policy` first, then
/// `--founder-heuristic` (the disclosed control), and otherwise the trained centre built
/// into the binary, which is the default since Wrysk's 2026-09-20 decision — founders on
/// these centres survived the generated closed world where the heuristics died.
///
/// A `--founder-policy` file is the trainer's own and validates itself — schema token,
/// lineage, vector length, finite weights, and a digest that is that founder manifest's
/// — so all this adds is the cross-check the user's spelling makes possible: a browser
/// centre asked to drive the blind lineage is refused by name rather than run against a
/// schema it was never trained on.
fn founder_recipes(args: &Voxel) -> Result<Vec<FounderRecipe>> {
    let policies = args.founder_policies()?;
    let heuristics = args.founder_heuristics()?;
    let mut out = Vec::new();
    for founder in Founder::ALL {
        if let Some((_, path)) = policies.iter().find(|(f, _)| *f == founder) {
            let file = VoxelPolicyFile::load(path).map_err(anyhow::Error::msg)?;
            let declared = file.founder().map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                declared == founder,
                "--founder-policy {}={}: the file declares the {} lineage",
                founder.name(),
                path.display(),
                declared.name(),
            );
            out.push(FounderRecipe {
                founder,
                driver: Some(file.driver().map_err(anyhow::Error::msg)?),
                source: format!("--founder-policy {}", path.display()),
            });
        } else if heuristics.contains(&founder) {
            out.push(FounderRecipe {
                founder,
                driver: None,
                source: "--founder-heuristic, the disclosed control".to_string(),
            });
        } else {
            let name = BUILT_IN_POLICIES
                .iter()
                .find(|(f, _, _)| *f == founder)
                .map(|(_, n, _)| *n)
                .unwrap_or("none");
            let driver = built_in_or_fallback(founder);
            let source = if driver.is_some() {
                format!("the built-in centre {name}")
            } else {
                format!(
                    "the observation-only heuristic — FALLBACK: the built-in centre {name} \
                     is refused by this build's founder manifest"
                )
            };
            out.push(FounderRecipe {
                founder,
                driver,
                source,
            });
        }
    }
    Ok(out)
}

/// Install the ambient run's founder controllers over a whole layer: a lineage given a
/// driver has its **birth factory and every standing body of that lineage** switched to
/// it, and a lineage given none keeps the heuristic the seeder registered. One factory
/// per lineage, and `EpisodeDriver::fresh` per body, so no two bodies share a hidden
/// state. Since the trained centres became the default, "given none" means the run asked
/// for the control with `--founder-heuristic`.
///
/// A layer that *remembers* being policy-driven — [`Fauna::policy_driven`], which a
/// snapshot carries — and is given no driver for that lineage is **refused here**, not
/// quietly demoted to a heuristic: a world whose animals were a trained policy is not
/// the same world with the reflex put back, and silently substituting one is exactly
/// the re-anchoring `always-fresh-never-migrate` forbids.
///
/// The record the layer carries is per lineage and a [`Fauna::policy_digest`]
/// (`weights_fnv1a`, `0` for a heuristic), so what is refused is not only the demotion
/// but a swap of one centre for another: a loaded world whose recorded digest is nonzero
/// and does not match the digest of the driver offered for that lineage is refused by
/// name, the same re-anchoring the bare-demotion refusal above forbids.
fn install_founder_controllers(
    fauna: &mut Fauna,
    drivers: &[(Founder, EpisodeDriver)],
) -> Result<()> {
    for founder in Founder::ALL {
        if fauna.policy_driven(founder) && !drivers.iter().any(|(f, _)| *f == founder) {
            bail!(
                "this world's {} founders were driven by a saved policy; drop \
                 `--founder-heuristic {}` to run them on the built-in centre, or pass \
                 `--founder-policy {}=<centre.json>` to name one, or start a fresh \
                 world. They are not silently put back on the heuristic.",
                founder.name(),
                founder.name(),
                founder.name(),
            );
        }
    }
    for (founder, driver) in drivers {
        let digest = driver.digest();
        let recorded = fauna.policy_digest(*founder);
        if recorded != 0 && recorded != digest {
            bail!(
                "this world's {} founders were driven by a saved policy, centre digest \
                 {recorded:#018x}; the centre supplied here is a different one (digest \
                 {digest:#018x}). That is not the same world under a new centre, so the \
                 policy is refused rather than swapped in silence — supply the same \
                 centre, or start a fresh world.",
                founder.name(),
            );
        }
    }
    for (founder, driver) in drivers {
        let digest = driver.digest();
        let driver = driver.clone();
        fauna.set_founder_factory(
            *founder,
            std::sync::Arc::new(move || -> Box<dyn Controller> { driver.fresh() }),
        );
        fauna.set_policy_driven(*founder, true);
        fauna.set_policy_digest(*founder, digest);
    }
    // Re-install over every standing founder body, so a loaded layer's bodies and the
    // bodies a policy flag took over are both driven by the lineage's current recipe.
    let bodies: Vec<(u64, Founder)> = fauna
        .view()
        .animals
        .iter()
        .filter_map(|a| a.founder.map(|f| (a.id, f)))
        .collect();
    for (id, founder) in bodies {
        if drivers.iter().any(|(f, _)| *f == founder) {
            anyhow::ensure!(
                fauna.install_founder_controller(id, founder),
                "the {} policy could not be installed on body {id}",
                founder.name()
            );
        }
    }
    Ok(())
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
                },
            )?;
            if args.gpu_web_rate > 0.0 {
                gpu = gpu.with_web(web(args.web_port)?, args.gpu_web_rate);
            }
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
                .establish_pore_min_on(cubarium_voxel::Material::Soil);
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

/// The run state a stdin command may change, and the one function that changes it.
///
/// It owns the scalars and borrows the world per call rather than the other way round,
/// so the loop can read `paused` and `speed` while it still holds the world.
struct Control {
    speed: f64,
    paused: bool,
    pending_steps: u32,
    outlet: bool,
    quit: bool,
    proj: Projection,
}

impl Control {
    fn new(speed: f64, proj: Projection) -> Control {
        Control {
            speed,
            paused: false,
            pending_steps: 0,
            outlet: false,
            quit: false,
            proj,
        }
    }

    /// Apply one stdin line. Anything unrecognised prints the usage and changes nothing;
    /// a bad argument is reported and the world is left alone.
    ///
    /// The plant and animal layers come in as further borrows rather than living in
    /// `Control`, for the same reason the world does: the loop reads `paused` and `speed`
    /// while it still holds all three.
    fn handle(&mut self, world: &mut World, flora: &mut Flora, fauna: &mut Fauna, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let mut parts = line.split_whitespace();
        let verb = parts.next().unwrap_or_default();
        let rest: Vec<&str> = parts.collect();

        match verb {
            "q" | "quit" => self.quit = true,
            "p" | "pause" => {
                self.paused = !self.paused;
                eprintln!(
                    "cubarium voxel: {}",
                    if self.paused { "paused" } else { "running" }
                );
            }
            "s" | "step" => {
                self.paused = true;
                self.pending_steps += 1;
            }
            "+" => {
                self.speed = (self.speed * 2.0).min(MAX_SPEED);
                eprintln!("cubarium voxel: speed {}", self.speed);
            }
            "-" => {
                self.speed = (self.speed / 2.0).max(MIN_SPEED);
                eprintln!("cubarium voxel: speed {}", self.speed);
            }
            "r" | "rain" => {
                let volume = match rest.first() {
                    None => DEFAULT_RAIN_M3,
                    Some(text) => match text.parse::<f64>() {
                        Ok(v) if v.is_finite() && v > 0.0 => v,
                        _ => {
                            eprintln!("cubarium voxel: `r [m3]` wants a positive volume");
                            return;
                        }
                    },
                };
                let took = world.apply(VoxelCommand::RainPulse { volume_m3: volume });
                eprintln!("cubarium voxel: rain {volume} m3, accepted {took} m3");
            }
            "a" | "aquifer" => {
                let volume = match rest.first().map(|t| t.parse::<f64>()) {
                    Some(Ok(v)) if v.is_finite() && v != 0.0 => v,
                    _ => {
                        eprintln!(
                            "cubarium voxel: `a M3` wants a nonzero volume (negative withdraws)"
                        );
                        return;
                    }
                };
                let took = world.apply(VoxelCommand::ChargeAquifer { volume_m3: volume });
                eprintln!(
                    "cubarium voxel: aquifer {volume:+} m3, accepted {took:+} m3, head now {:.3} m",
                    world.aquifer_head_m()
                );
            }
            // The closed budget's "make it rain": water goes **aloft**, and the shower
            // scheduler brings it down when the store passes its trigger. `r` still
            // drops a pulse on the world now; this is the lever that feeds the cycle.
            "h" | "atmosphere" => {
                let volume = match rest.first().map(|t| t.parse::<f64>()) {
                    Some(Ok(v)) if v.is_finite() && v > 0.0 => v,
                    _ => {
                        eprintln!("cubarium voxel: `h M3` wants a positive volume");
                        return;
                    }
                };
                let took = world.apply(VoxelCommand::AddAtmosphere { volume_m3: volume });
                if took > 0.0 {
                    eprintln!(
                        "cubarium voxel: atmosphere +{took} m3, store now {:.3} m3",
                        world.atmosphere_m3()
                    );
                } else {
                    eprintln!(
                        "cubarium voxel: refused — this world runs the open water budget \
                         and has no atmosphere store"
                    );
                }
            }
            "o" | "outlet" => {
                self.outlet = !world.outlet_open();
                world.apply(VoxelCommand::SetOutlet { open: self.outlet });
                eprintln!(
                    "cubarium voxel: outlet {}",
                    if self.outlet { "open" } else { "closed" }
                );
            }
            "w" | "save" => match rest.first() {
                // Terrain only, and the message says so: `World::save` holds the voxels,
                // the water and the world's tick, and neither layer of the ecology.
                Some(path) => match std::fs::write(path, world.save()) {
                    Ok(()) => eprintln!(
                        "cubarium voxel: saved {path} — terrain and water only, no plants \
                         and no animals"
                    ),
                    Err(e) => eprintln!("cubarium voxel: saving {path}: {e}"),
                },
                None => eprintln!("cubarium voxel: `w PATH` needs a path (terrain only)"),
            },
            "l" | "load" => match rest.first() {
                Some(path) => {
                    // A terrain load is not an ecosystem restore (Astra R9.6): refuse it
                    // in place while anything of the ecology is standing, and say what.
                    if let Some(why) = ecology_state(flora, fauna) {
                        eprintln!(
                            "cubarium voxel: `l` loads the **terrain only** and this run has \
                             {why}; an in-place load would leave them on another world's \
                             terrain with ledgers describing this one. Restart with \
                             `--load {path}`, which starts the ecology fresh on that terrain."
                        );
                        return;
                    }
                    let read = std::fs::read(path)
                        .map_err(anyhow::Error::from)
                        .and_then(|b| World::load(&b));
                    match read {
                        Ok(loaded) => {
                            // The projection is fixed for the life of the run, so a world
                            // of a different shape would draw into the wrong raster.
                            let c = loaded.config();
                            if (c.width, c.height, c.depth)
                                != (self.proj.width, self.proj.height, self.proj.depth)
                            {
                                eprintln!(
                                    "cubarium voxel: {path} is {}x{}x{}, this run draws \
                                     {}x{}x{}; restart with a matching config",
                                    c.width,
                                    c.height,
                                    c.depth,
                                    self.proj.width,
                                    self.proj.height,
                                    self.proj.depth
                                );
                            } else {
                                *world = loaded;
                                self.outlet = world.outlet_open();
                                eprintln!(
                                    "cubarium voxel: loaded the terrain of {path} at tick \
                                     {} — the plants and the animals are this run's own",
                                    world.tick()
                                );
                            }
                        }
                        Err(e) => eprintln!("cubarium voxel: loading {path}: {e:#}"),
                    }
                }
                None => eprintln!("cubarium voxel: `l PATH` needs a path (terrain only)"),
            },
            "i" | "inspect" => {
                let Some((x, y, z)) = coords(world, "i X Y Z", &rest) else {
                    return;
                };
                eprintln!("cubarium voxel: {}", cell_state(world, x, y, z));
                // The site is the cell itself, not the column's skyline: `i` on a
                // support face reports what grows there and what the ground holds.
                let site = site_of(world, x, y, z);
                let fv = flora.view();
                match fv.stand_at(site) {
                    Some(s) => eprintln!(
                        "cubarium voxel:   {} {:?} W {:.4} P {:.4} Q {:.4} mineral {:.5} \
                         light {:.3} moisture {:.3} aeration stress {:.3}",
                        s.species.name(),
                        s.stage,
                        s.wood,
                        s.foliage,
                        s.reserve,
                        s.mineral,
                        s.light,
                        s.moisture,
                        s.aeration_stress,
                    ),
                    None => eprintln!("cubarium voxel:   no stand"),
                }
                // The animals standing on this face, in id order. More than one is
                // ordinary: a newborn appears on its parent's face.
                for a in fauna.view().animals_at(site) {
                    eprintln!(
                        "cubarium voxel:   {} #{} body {:.4} reserve {:.4} mineral {:.5} \
                         energy {:.4} age {} ticks {:?}",
                        a.species.name(),
                        a.id,
                        a.body,
                        a.reserve,
                        a.mineral,
                        a.energy,
                        a.age_ticks,
                        a.state,
                    );
                }
                if let Some(g) = fv.ground_at(site) {
                    eprintln!(
                        "cubarium voxel:   ground N {:.4} litter {:.4} ({:.4} energy, \
                         {:.5} mineral) dead wood {:.4} ({:.5} mineral)",
                        g.mineral,
                        g.litter,
                        g.litter_energy,
                        g.litter_mineral,
                        g.dead_wood,
                        g.dead_wood_mineral,
                    );
                    // The seed bank, in the order the ground holds it: species, then
                    // arrival bin, oldest first. The age is the bin's, measured from the
                    // tick its window opened.
                    for c in &g.seeds {
                        eprintln!(
                            "cubarium voxel:   seed {} organic {:.6} mineral {:.7} age {} ticks \
                             (bin from tick {})",
                            c.species.name(),
                            c.organic,
                            c.mineral,
                            c.age_ticks(fv.tick),
                            c.bin_start_tick,
                        );
                    }
                    if g.seeds.is_empty() {
                        eprintln!("cubarium voxel:   no seed cohort");
                    }
                }
            }
            "f" | "flora" => {
                let usage = "f X Z bloomcrown|umbrellafrond [wood]";
                let Some((x, z)) = column(world, usage, &rest) else {
                    return;
                };
                let Some(species) = rest.get(2).and_then(|n| Species::parse(n)) else {
                    eprintln!(
                        "cubarium voxel: `{usage}` — `{}` is not a species",
                        rest.get(2).copied().unwrap_or(""),
                    );
                    return;
                };
                // A founder with no wood named is a full-grown one: the picture this
                // command exists for is a stand you can see.
                let wood = match rest.get(3) {
                    None => flora.config().species(species).wood_max,
                    Some(text) => match text.parse::<f64>() {
                        Ok(v) if v.is_finite() && v > 0.0 => v,
                        _ => {
                            eprintln!("cubarium voxel: `{usage}` wants a positive wood");
                            return;
                        }
                    },
                };
                if flora.apply(
                    world,
                    FloraCommand::Seed {
                        x,
                        z,
                        species,
                        wood,
                    },
                ) {
                    let site = highest_site(world, x, z);
                    eprintln!(
                        "cubarium voxel: seeded {} at {:?} with W {wood}",
                        species.name(),
                        site,
                    );
                } else {
                    eprintln!(
                        "cubarium voxel: no stand at ({x}, {z}): either the column has no \
                         support face, one already grows there, or W {wood} is below \
                         {}'s alive_min {}",
                        species.name(),
                        flora.config().species(species).alive_min,
                    );
                }
            }
            "c" | "clear" => {
                let Some((x, z)) = column(world, "c X Z", &rest) else {
                    return;
                };
                if flora.apply(world, FloraCommand::Clear { x, z }) {
                    eprintln!("cubarium voxel: cleared the stand at ({x}, {z})");
                } else {
                    eprintln!("cubarium voxel: no stand to clear at ({x}, {z})");
                }
            }
            "g" | "grazer" => {
                let usage = "g X Z frondgrazer [body]";
                let Some((x, z)) = column(world, usage, &rest) else {
                    return;
                };
                let Some(species) = rest.get(2).and_then(|n| Beast::parse(n)) else {
                    eprintln!(
                        "cubarium voxel: `{usage}` — `{}` is not an animal",
                        rest.get(2).copied().unwrap_or(""),
                    );
                    return;
                };
                // No body named is a grown one: the picture this command exists for is an
                // animal you can see, as `f`'s is a stand you can see.
                let body = match rest.get(3) {
                    None => fauna.config().species(species).body_max,
                    Some(text) => match text.parse::<f64>() {
                        Ok(v) if v.is_finite() && v > 0.0 => v,
                        _ => {
                            eprintln!("cubarium voxel: `{usage}` wants a positive body");
                            return;
                        }
                    },
                };
                if fauna.apply(
                    world,
                    FaunaCommand::Introduce {
                        x,
                        z,
                        species,
                        body,
                    },
                ) {
                    eprintln!(
                        "cubarium voxel: introduced {} at {:?} with body {body}",
                        species.name(),
                        highest_site(world, x, z),
                    );
                } else {
                    eprintln!(
                        "cubarium voxel: no animal at ({x}, {z}): either the column has no \
                         support face, or body {body} is below {}'s body_min {}",
                        species.name(),
                        fauna.config().species(species).body_min,
                    );
                }
            }
            "m" | "material" => {
                let usage = "m X Y Z air|rock|soil|bedrock";
                let Some((x, y, z)) = coords(world, usage, &rest) else {
                    return;
                };
                let material = match rest.get(3).map(|w| w.to_ascii_lowercase()).as_deref() {
                    Some("air") => Material::Air,
                    Some("rock") => Material::Rock,
                    Some("soil") => Material::Soil,
                    Some("bedrock") => Material::Bedrock,
                    other => {
                        eprintln!(
                            "cubarium voxel: `{usage}` — `{}` is not a material",
                            other.unwrap_or("")
                        );
                        return;
                    }
                };
                // The core moves the water the old material held; the new state is the
                // receipt, so a displaced cell says so itself.
                world.apply(VoxelCommand::SetMaterial { x, y, z, material });
                eprintln!(
                    "cubarium voxel: set {material:?}, now {}",
                    cell_state(world, x, y, z)
                );
            }
            other => eprintln!("cubarium voxel: `{other}`? — {COMMANDS}"),
        }
    }
}

/// `X Y Z` from a command's arguments, or `None` with the reason printed.
///
/// `x` wraps — the strip is a ring and has no end to fall off — so any integer is a
/// column. `y` and `z` have real ends, so one outside the world is refused and named
/// rather than folded into a cell the caller did not mean.
/// What a terrain-only load would leave standing, in words, or `None` when this run holds
/// no ecology at all and `World::load` can safely replace the terrain under it.
///
/// Astra R9.6: `World::save`/`World::load` carry the voxels, the water and the world's
/// tick, and neither layer of the ecology — so an in-place load against living stands is
/// not a restore of anything. The checks are the review's list: any stand, any animal, any
/// seed bank, any provisioned ground, and a nonzero ledger on either layer (compared
/// against `Default`, so a term added later is covered without touching this).
fn ecology_state(flora: &Flora, fauna: &Fauna) -> Option<String> {
    let fv = flora.view();
    let av = fauna.view();
    if !fv.stands.is_empty() {
        return Some(format!("{} stand(s) growing", fv.stands.len()));
    }
    if !av.animals.is_empty() {
        return Some(format!("{} animal(s) alive", av.animals.len()));
    }
    let banks = fv.ground.iter().filter(|g| !g.seeds.is_empty()).count();
    if banks > 0 {
        return Some(format!("{banks} site(s) holding a seed bank"));
    }
    if !fv.ground.is_empty() {
        return Some(format!("{} provisioned ground site(s)", fv.ground.len()));
    }
    if *fv.ledger != FloraLedger::default() {
        return Some("a nonzero plant ledger".to_string());
    }
    if *av.ledger != FaunaLedger::default() {
        return Some("a nonzero animal ledger".to_string());
    }
    None
}

fn coords(world: &World, usage: &str, rest: &[&str]) -> Option<(i64, u32, u32)> {
    let triple = (
        rest.first().and_then(|t| t.parse::<i64>().ok()),
        rest.get(1).and_then(|t| t.parse::<u32>().ok()),
        rest.get(2).and_then(|t| t.parse::<u32>().ok()),
    );
    let (Some(x), Some(y), Some(z)) = triple else {
        eprintln!("cubarium voxel: `{usage}` wants three integers for X Y Z");
        return None;
    };
    let c = world.config();
    if y >= c.height || z >= c.depth {
        eprintln!(
            "cubarium voxel: ({x}, {y}, {z}) is outside a {}x{}x{} world",
            c.width, c.height, c.depth
        );
        return None;
    }
    Some((x, y, z))
}

/// `X Z` from a command's arguments: a column, for the commands that act on whichever
/// support face is the highest one in it. `x` wraps; `z` is refused if it is outside.
fn column(world: &World, usage: &str, rest: &[&str]) -> Option<(i64, u32)> {
    let pair = (
        rest.first().and_then(|t| t.parse::<i64>().ok()),
        rest.get(1).and_then(|t| t.parse::<u32>().ok()),
    );
    let (Some(x), Some(z)) = pair else {
        eprintln!("cubarium voxel: `{usage}` wants two integers for X Z");
        return None;
    };
    let c = world.config();
    if z >= c.depth {
        eprintln!(
            "cubarium voxel: z = {z} is outside a {}-deep world",
            c.depth
        );
        return None;
    }
    Some((x, z))
}

/// The flora site a cell names: the cell itself, with `x` wrapped as a site stores it.
fn site_of(world: &World, x: i64, y: u32, z: u32) -> Site {
    Site {
        x: x.rem_euclid(i64::from(world.config().width)) as u32,
        y,
        z,
    }
}

/// The site `f` and `c` act on: the highest support face of a column, which is what
/// `cubarium_voxel_flora::highest_support` picks, so the echo names the cell the command
/// really touched.
fn highest_site(world: &World, x: i64, z: u32) -> Option<Site> {
    cubarium_voxel_flora::highest_support(&world.view(), x, z)
}

/// One cell's state as `i` reports it, and as `m` echoes back after an edit.
fn cell_state(world: &World, x: i64, y: u32, z: u32) -> String {
    let view = world.view();
    format!(
        "({}, {y}, {z}) {:?} free {:.4} pore {:.4}",
        x.rem_euclid(i64::from(view.config.width)),
        view.material_at(x, y, z),
        view.free_at(x, y, z),
        view.pore_at(x, y, z),
    )
}

/// Read stdin lines on a thread and hand them to the loop.
///
/// Detached on purpose: a blocking `read_line` cannot be cancelled portably, and the
/// process is about to exit anyway. When stdin closes the thread ends and the loop
/// simply stops getting commands.
fn spawn_stdin_reader() -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
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
    rx
}

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
            world: cubarium_voxel::Config {
                width: 16,
                height: 8,
                depth: 2,
                ..Default::default()
            },
            ..VoxelConfig::default()
        };
        let proj = Projection::new(30.0, 4, 0, &cfg.world).unwrap();
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
            Fauna::new(FaunaConfig::default()),
            SimConfig { threads: 1 },
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
            Fauna::new(FaunaConfig::default()),
            SimConfig { threads: 1 },
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
            Fauna::new(FaunaConfig::default()),
            SimConfig { threads: 1 },
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
            assert!(writer.save_in_background(&world));
            writer.wait();
            world.step();
        }
        writer.save_now(&world);
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
        assert!(
            !writer.save_in_background(&world),
            "one write in flight at a time"
        );
        release.send(()).unwrap();
        writer.save_now(&world);
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
        let w = &cfg.world;
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

    /// The documented defaults, and a partial config file that only overrides some of
    /// them: everything else keeps the default rather than being refused.
    #[test]
    fn the_config_defaults_are_the_documented_ones_and_a_partial_file_loads() {
        let d = VoxelConfig::default();
        assert_eq!(d.tilt_degrees, 30.0);
        assert_eq!(d.px_per_voxel, 4);
        assert_eq!(d.raster_height, 0);
        assert_eq!(d.haze, 0.55);
        assert_eq!(d.water_alpha, 0.5);
        assert_eq!(
            d.world.voxel_m, 0.25,
            "the ambient default is the shipped `default` landscape, cell size and all"
        );
        // The shipped `default` landscape, whatever that landscape currently is: the
        // preset owns its own recipe, and pinning a copy of it here only makes this test
        // fail every time the generator tunes one.
        let shipped = cubarium_voxel::Preset::find("default").expect("a default preset");
        assert_eq!(
            d.world.landform,
            cubarium_voxel::Landform::Staged(shipped.recipe),
            "`cubarium voxel` with no TOML generates the shipped staged ring"
        );
        assert_eq!(
            (d.world.width, d.world.height, d.world.depth),
            (shipped.width, shipped.height, shipped.depth),
            "and on the ring that landscape was written for"
        );

        let cfg: VoxelConfig =
            toml::from_str("tilt_degrees = 35.0\n[world]\nwidth = 64\ndepth = 8\n").unwrap();
        assert_eq!(cfg.tilt_degrees, 35.0);
        assert_eq!(
            cfg.px_per_voxel, 4,
            "an unmentioned field keeps its default"
        );
        assert_eq!(cfg.world.width, 64);
        assert_eq!(cfg.world.depth, 8);
        assert_eq!(cfg.world.voxel_m, 0.25);
        let explicit: VoxelConfig = toml::from_str("[world]\nvoxel_m = 0.125\n").unwrap();
        assert_eq!(explicit.world.voxel_m, 0.125);
        assert_eq!(cfg.world.height, cubarium_voxel::Config::default().height);

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
        assert_eq!(plain.world.landform, Landform::Ridge);

        let ridge: VoxelConfig = toml::from_str("[world]\nlandform = \"ridge\"\n").unwrap();
        assert_eq!(ridge.world.landform, Landform::Ridge);

        let wide: VoxelConfig =
            toml::from_str("[world]\n[world.landform]\npreset = \"wide\"\n").unwrap();
        let wanted = cubarium_voxel::Preset::find("wide").unwrap().recipe;
        assert_eq!(wide.world.landform, Landform::Staged(wanted));

        let tweaked: VoxelConfig =
            toml::from_str("[world]\n[world.landform]\npreset = \"small\"\nrelief_m = 2.0\n")
                .unwrap();
        let Landform::Staged(recipe) = tweaked.world.landform else {
            panic!("a preset makes a staged landform");
        };
        assert_eq!(recipe.relief_m, 2.0, "the file overrides the preset");
        assert_eq!(
            recipe.relief_wavelength_m,
            cubarium_voxel::Recipe::SMALL.relief_wavelength_m,
            "and leaves the rest of it alone"
        );
        assert!(tweaked.world.validate().is_ok());

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
        assert_eq!(cfg.world.depth, cubarium_voxel::Config::default().depth);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.lines().any(|l| l.trim_start().starts_with("depth")),
            "the example must not set `depth`; it mirrors the core default"
        );
        // The chosen camera, written down where a reader of the file will find it.
        assert!(text.contains("tilt_degrees = 30.0") && text.contains("px_per_voxel = 4"));
    }

    /// The panel's own config selects the designed terrarium (Wrysk, 2026-09-23), and
    /// `natural.toml` beside it keeps the shipped `small` landscape, both on the ring they
    /// were written for. Neither states water keys: the recipe decides those now.
    #[test]
    fn the_tachyon_configs_name_the_terrarium_and_the_small_landscape() {
        for (file, landform) in [
            (
                "voxel.toml",
                cubarium_voxel::Landform::Terrarium(cubarium_voxel::Terrarium::SMALL),
            ),
            (
                "natural.toml",
                cubarium_voxel::Landform::Staged(cubarium_voxel::Recipe::SMALL),
            ),
        ] {
            tachyon_config_names(file, landform);
        }
    }

    fn tachyon_config_names(file: &str, landform: cubarium_voxel::Landform) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/tachyon")
            .join(file)
            .canonicalize()
            .expect("the panel config is committed");
        let cfg = load_config(&path).unwrap();
        assert_eq!(cfg.world.landform, landform, "{file}");
        assert_eq!(
            (
                cfg.world.width,
                cfg.world.height,
                cfg.world.depth,
                cfg.world.voxel_m
            ),
            (160, 72, 24, 0.125),
            "the Tachyon ring: 20 m around, 9 m tall, 3 m deep"
        );
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
            sink: crate::cli::VoxelSinkArg::Web,
            scene: crate::cli::VoxelSceneArg::Generated,
            seed: Some(1),
            arena: None,
            controller: crate::cli::VoxelControllerArg::Heuristic,
            policy: None,
            founder_policy: Vec::new(),
            founder_heuristic: Vec::new(),
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
            all_chiplets: false,
            web_port: 7393,
            gpu_target: None,
            gpu_capture: None,
            gpu_web_rate: 0.0,
            gpu_roof_walk: false,
            textures: false,
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

    /// A config file whose `[world]` no world can be built from is an error, not a panic:
    /// `World::new` asserts, so the run validates first.
    #[test]
    fn a_world_no_world_can_be_built_from_is_refused_not_panicked_on() {
        let cfg: VoxelConfig =
            toml::from_str("[world]\ndepth = 0\n").expect("it parses; it is just impossible");
        let err = cfg
            .world
            .validate()
            .expect_err("a zero dimension must be refused");
        assert!(format!("{err}").contains("nonzero dimensions"), "{err}");

        let bad: VoxelConfig = toml::from_str("[world]\nvoxel_m = 0.0\n").unwrap();
        assert!(bad.world.validate().is_err());
        assert!(VoxelConfig::default().world.validate().is_ok());
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
        let mut fauna = Fauna::new(FaunaConfig::default());
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
        ctl.handle(&mut world, &mut flora, &mut fauna, "w");
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
        let mut fauna = Fauna::new(FaunaConfig::default());
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
        let mut fauna = Fauna::new(FaunaConfig::default());
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
        let mut fauna = Fauna::new(FaunaConfig::default());
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
        let mut fauna = Fauna::new(FaunaConfig::default());
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

        let validates: Vec<bool> = Founder::ALL
            .iter()
            .map(|&founder| match built_in_driver(founder) {
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
        let mut fauna = Fauna::new(FaunaConfig::default());
        let mut ctl = Control::new(1.0, proj);
        assert_eq!(
            ecology_state(&flora, &fauna),
            None,
            "a fresh run holds no ecology"
        );

        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("w {at}"));
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
        let mut fresh_fauna = Fauna::new(FaunaConfig::default());
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
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(fauna.apply(
            &world,
            FaunaCommand::Introduce {
                x: 3,
                z: 1,
                species: Beast::Frondgrazer,
                body: 0.02,
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
        let mut fauna = Fauna::new(FaunaConfig::default());
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
        let mut fauna = Fauna::new(FaunaConfig::default());
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
        let mut fauna = Fauna::new(FaunaConfig::default());
        let mut ctl = Control::new(1.0, proj);
        ctl.handle(
            &mut world,
            &mut flora,
            &mut fauna,
            &format!("w {}", path.display()),
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
            sink: VoxelSinkArg::Web,
            scene: VoxelSceneArg::Generated,
            seed,
            arena: None,
            controller: VoxelControllerArg::Heuristic,
            policy: None,
            founder_policy: Vec::new(),
            founder_heuristic: Vec::new(),
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
            all_chiplets: false,
            web_port: 7393,
            gpu_target: None,
            gpu_capture: None,
            gpu_web_rate: 0.0,
            gpu_roof_walk: false,
            textures: false,
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
            sink: VoxelSinkArg::Web,
            scene: VoxelSceneArg::Generated,
            seed: None,
            arena: None,
            controller: VoxelControllerArg::Heuristic,
            policy: None,
            founder_policy: Vec::new(),
            founder_heuristic: Vec::new(),
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
            all_chiplets: false,
            web_port: 7393,
            gpu_target: None,
            gpu_capture: None,
            gpu_web_rate: 0.0,
            gpu_roof_walk: false,
            textures: false,
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
        save_voxel_snapshot(&dir, &saved_world);

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

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
