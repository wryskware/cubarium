//! `--sink gpu`: the live ring world drawn by `cubarium-gpu` instead of by the canvas.
//!
//! # What this file is
//!
//! The half of Stage B's adapter that knows about the **world**. `cubarium_gpu::adapter`
//! turns one stamp into one instance; this turns one `RenderView` into the stamps, and it
//! does so by asking the *real* `ArtPresenter` every question rather than by reimplementing
//! it. It keeps a presenter of its own for the state a picture needs and the view does not
//! carry — paced growth, column heights, body cross-fades, hunter phases, meal bouts — and
//! then walks the same passes `ArtPresenter::draw_with_fruit` walks, in the same order,
//! reading `ArtGeometry` for every placement and `habitat`/`wind`/`tall` for every rule.
//!
//! **Nothing here decides what the picture contains.** Every threshold, every phase hash,
//! every opacity ramp and every bend comes from `art_present`, so the CPU and GPU images
//! cannot disagree about *what* to draw. What they can disagree about is rasterisation,
//! which is the thing worth measuring — see `tests/gpu_fidelity.rs`.
//!
//! # Why a second presenter rather than the runner's
//!
//! The runner's `Show` is private and belongs to the CPU path, which `--sink gpu` turns
//! off (`wants_pixels`). A sink may not reach into it. The cost is one extra `observe` per
//! tick over the cells — the growth pacing, no rasterisation — which is 20 Hz work; the
//! alternative is a sink that mutates the host's presenter, which is exactly the one-way
//! rule `FrameSink` is written around.
//!
//! # The ring only
//!
//! `--sink gpu` refuses a cube by name. The renderer draws one raster; a cube is five
//! charts with seams, and `unfold_pixels` — the thing that makes a stamp cross them — has
//! no analogue in an instanced quad. The cube keeps the CPU presenter, which is also
//! where its byte-exact regressions live.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use cubarium_core::hunter::{HunterEvent, HunterView};
use cubarium_core::view::{OrganismView, RenderView};
use cubarium_surface::{CellId, SurfacePoint, Topology, Vec2, cell_of};

use cubarium_gpu::adapter::{MAX_POSES, PoseRef, ScratchFrame, Stamp, StampMask, StampTone};
use cubarium_gpu::atlas::{Atlas, Clip as GpuClip, PlantClip};
use cubarium_gpu::render::Renderer;
use cubarium_gpu::scene::{Layer, RingLayout, Scene};
use cubarium_gpu::vk::Gpu;

use crate::art::{ArtPack, Band};
use crate::art_present::{
    ArtGeometry, ArtPresenter, BodyMemory, DEAD_WOOD_OPACITY, Growth, GrowthStep, PLANT_REVEAL_PX,
    RAIN_OPACITY, RAIN_SRGB, SOIL_SCALE, SOIL_SNAG_PX, TALL_BASE_FADE, TALL_BEND_LENGTH,
    TALL_BEND_ROOT, TALL_FIRST_JOIN, TALL_OPACITY, TALL_PLANTS, TILE_ROWS, VINE_PLANT,
    band_opacity, clip_time, dead_wood_density, dead_wood_tone, foliage_fullness, foliage_ramp_at,
    fruit_stage, ground_opacity, growth_between, growth_step, growth_weights, litter_density,
    living_wood_tone, phase_of, plant_density, plant_phase_of, rain_streaks, rig_of,
    soil_snag_opacity, stage_opacity, stage_thresholds, structural, tall_bend_base, tall_between,
    tall_grown_px, trunk_strip, turn_heading, w_soil, wood_shade,
};
use crate::lanternjaw::Lanternjaw;
use crate::sink::{FrameSink, Output, WebSink, WorldShape};
use cube_proto::Raster;

#[path = "gpu/target.rs"]
mod target;
pub use target::{GpuTarget, GpuTargetKind};

#[path = "gpu/voxel.rs"]
pub mod voxel;
pub use voxel::{VoxelGpuSink, VoxelGpuSinkOptions};

/// `--sink gpu`.
pub struct GpuSink {
    /// The presenter that decides everything; it never draws.
    presenter: ArtPresenter,
    geom: ArtGeometry,
    atlas: Atlas,
    /// Shared so a presenting thread could hold it too; the ring does not use one, but
    /// the device is opened the same way for both renderers.
    gpu: std::sync::Arc<Gpu>,
    renderer: Renderer,
    target: GpuTarget,
    lanternjaw: Lanternjaw,
    parts: Vec<crate::lanternjaw::Part>,
    scene: Scene,
    layout: RingLayout,
    /// The tick the presenter last observed, so the per-frame hook can advance it once.
    observed: Option<u64>,
    /// Every cell's fixed plant slot, laid out once.
    ///
    /// `ArtGeometry::slot_of` is seven `SplitMix64` draws and a `Topology::embed_tangent`;
    /// the CPU presenter builds the table once in `for_world` and this must too, or the
    /// adapter spends more time hashing than the renderer spends drawing.
    slots: Vec<crate::art_present::Slot>,
    /// Each plant family's measured bend budget, by name, so the per-cell lookup is not a
    /// linear scan over the pack with a string compare at every step.
    budgets: std::collections::HashMap<&'static str, f64>,
    /// Each column's height as the previous `observe` left it.
    ///
    /// `ArtPresenter` keeps its own `tall_prev` and interpolates across the tick, but
    /// exposes only the current value; without a copy the GPU would draw a column a whole
    /// tick ahead of the CPU at `f = 0`, which is a visible difference in a young world
    /// where columns are growing fast. Snapshotted here, immediately before the presenter
    /// advances, so it *is* the presenter's own previous value.
    tall_prev: Vec<crate::art_present::TallGrowth>,
    tall_dead_prev: Vec<crate::art_present::TallGrowth>,
    /// How many frames a stamp had to drop because more than four carried weight
    /// (`cubarium_gpu::Stamp::instance`). Reported at `finish`, because a number nobody
    /// prints is a number nobody checks.
    dropped_frames: u64,
    /// Frames drawn, and where their time went: building the scene on the CPU, the GPU's
    /// own timestamps, and everything the target's `draw` spends (submit, the fence wait
    /// and the daemon's vsync pacing).
    frames: u64,
    gpu_ms: f64,
    /// The GPU's own three stages (`Renderer::gpu_split`): uploads, the world raster
    /// pass, the present pass. Summed here and divided at `finish`, so a report can say
    /// which of the two passes `--gpu-art-scale` actually costs.
    gpu_stages: [f64; 3],
    build_ms: f64,
    present_ms: f64,
    /// `Renderer::fill`'s three sums, accumulated: quad pixels, whole-tile pixels,
    /// opaque pixels. Zero unless `--gpu-fill-profile` is on.
    fill: [f64; 3],
    /// The quad pixels of `fill[0]` split by layer, in `LAYERS` order.
    fill_layers: [f64; cubarium_gpu::scene::LAYER_COUNT],
    /// Where to write a PNG of every rendered raster, for captures and the fidelity test.
    capture: Option<PathBuf>,
    /// The operator's viewer, fed from **this renderer's own readback** instead of from a
    /// CPU rasterisation, and the seconds between the frames it is fed.
    ///
    /// W2 measured `--mirror-web` at 59.5 -> 16.5 fps on the board, and all of the loss was
    /// CPU: `FanOutSink::wants_pixels` is true if any child wants pixels, so mirroring made
    /// the host pay for the whole canvas draw and PNG encode that `--sink gpu` exists to
    /// skip. This pays nothing of that. It copies the raster the GPU has already drawn —
    /// 640x360x4 bytes — at a rate an operator chooses, and the panel keeps its own.
    web: Option<(WebSink, f64)>,
    /// The scratch the readback is turned into; one `Raster`, reused.
    web_raster: Option<Raster>,
    /// When the viewer was last fed.
    web_last: Option<std::time::Instant>,
    /// Frames fed to the viewer, and the milliseconds they cost, for `finish`.
    web_frames: u64,
    web_ms: f64,
}

/// The knobs `--sink gpu` takes, as one value rather than seven positional arguments.
///
/// Every `Option` means "the sink decides", and what it decides depends on the world:
/// the substep bend is on by default on a ring at `S >= 2` and the art scale is the CPU
/// presenter's 1. A `Some` is an operator overriding that, either way.
#[derive(Clone, Debug)]
pub struct GpuSinkOptions {
    /// Where the frames go.
    pub target: GpuTargetKind,
    /// `--gpu-bend-substep` / `--no-gpu-bend-substep`; `None` takes the default for the
    /// world (see [`GpuSink::substep_default`]).
    pub bend_substep: Option<bool>,
    /// `--gpu-filter bilinear`: the CPU presenter's sampler, for the comparison.
    pub filter_bilinear: bool,
    /// `--gpu-art-scale`; `None` is the CPU presenter's 1.
    pub art_scale: Option<f32>,
    /// `--gpu-capture`: a PNG of every rendered raster.
    pub capture: Option<PathBuf>,
    /// `--gpu-fill-profile`: measure the sprite fill each frame and report it at exit.
    pub fill_profile: bool,
}

impl Default for GpuSinkOptions {
    fn default() -> GpuSinkOptions {
        GpuSinkOptions {
            target: GpuTargetKind::Headless,
            bend_substep: None,
            filter_bilinear: false,
            art_scale: None,
            capture: None,
            fill_profile: false,
        }
    }
}

impl GpuSink {
    /// Whether the wind's displacement lands between source texels by default on this
    /// world.
    ///
    /// **On at `S >= 2`, off at `S = 1`.** At `S = 1` the two are the same picture — one
    /// source texel is one raster pixel and there is nothing between two of them to land
    /// on — so the question only arises at the ladder's higher rungs, and there Wrysk
    /// asked for the smoother sway (`tachyon-screen-plan`, viewing session 1: "the
    /// higher resolution deserves smoother sway than whole-texel steps"). The `S x S`
    /// block rule is kept for everything except the bend's own displacement.
    pub fn substep_default(scale: u32) -> bool {
        scale >= 2
    }

    /// Open the device, load the pack and build the renderer for this world.
    pub fn new(
        shape: WorldShape,
        art: &std::path::Path,
        options: GpuSinkOptions,
    ) -> Result<GpuSink> {
        let GpuSinkOptions {
            target: kind,
            bend_substep,
            filter_bilinear,
            art_scale,
            capture,
            fill_profile,
        } = options;
        let Topology::Ring { w, h } = shape.topology else {
            bail!(
                "--sink gpu draws one raster and a cube is five charts with seams; \
                 run a ring (--topology ring:WxH) or use --sink shim/web/png for the cube"
            );
        };
        let world_scale = shape.scale.world();
        if world_scale.fract() != 0.0 || !(1.0..=64.0).contains(&world_scale) {
            bail!(
                "--sink gpu needs a whole --world-scale (this world is S = {world_scale}); \
                 one authored source texel must cover an exact S x S block of pixels, and a \
                 fractional S has no such block"
            );
        }
        let layout = RingLayout {
            w: u32::from(w),
            h: u32::from(h),
            scale: world_scale as u32,
        };
        if !layout.is_valid() {
            bail!("a {w}x{h} ring at S = {world_scale} does not divide into whole cells");
        }
        let pack = ArtPack::load(art)
            .with_context(|| format!("loading the art pack {}", art.display()))?;
        let atlas = Atlas::load(art)
            .with_context(|| format!("loading {} for the GPU atlas", art.display()))?;
        let gpu = std::sync::Arc::new(
            Gpu::open(&[]).context("opening the Vulkan device for --sink gpu")?,
        );
        eprintln!("cubarium: --sink gpu on {}", gpu.name);
        let mut renderer = Renderer::new(&gpu, &atlas, layout)?;
        renderer.bend_substep =
            bend_substep.unwrap_or_else(|| GpuSink::substep_default(layout.scale));
        renderer.filter_bilinear = filter_bilinear;
        renderer.fill_profile = fill_profile;
        // Default 1: what `art_present` does, whatever the world's S. `--gpu-art-scale`
        // is how the plan's "the sprite tile scales with S" gets looked at on the panel.
        renderer.art_scale = art_scale.unwrap_or(1.0);
        // Not threaded: this renderer writes its instance buffers every frame and has
        // no ring to keep them off a frame the GPU is still reading.
        let target = GpuTarget::open(kind, &gpu, &mut renderer, "cubarium — ring (GPU)", false)?;
        // The canopy line comes from `ArtGeometry::new`, which is `CANOPY_TOP = 0.67` on
        // a ring — FW-5's constant, so the GPU and the CPU cannot disagree about where
        // the canopy starts.
        //
        // TODO: when a world config carries the threshold, read it here and pass it
        // through `ArtPresenter::with_canopy_top`. `WorldConfig` has no such key today
        // (checked 2026-09-16); the natural name, beside `topology` and `world_scale`,
        // is `canopy_top`, and `ArtGeometry::with_canopy_top` already validates the
        // `0..=1` range a config would have to be checked against.
        let presenter = ArtPresenter::for_world(pack, shape.topology, shape.scale);
        let geom = presenter.geometry();
        let slots: Vec<crate::art_present::Slot> =
            geom.all_cells().map(|c| geom.slot_of(c)).collect();
        let budgets = [
            crate::art_present::SOIL_PLANTS[0],
            crate::art_present::SOIL_PLANTS[1],
            crate::art_present::FOLIAGE_PLANTS[0],
            crate::art_present::FOLIAGE_PLANTS[1],
            crate::art_present::CANOPY_PLANTS[0],
            crate::art_present::CANOPY_PLANTS[1],
            crate::art_present::WATER_PLANT,
        ]
        .into_iter()
        .map(|name| (name, presenter.bend_budget(name)))
        .collect();
        Ok(GpuSink {
            presenter,
            geom,
            atlas,
            gpu,
            renderer,
            target,
            lanternjaw: Lanternjaw::new(),
            parts: Vec::new(),
            scene: Scene::new(layout),
            layout,
            observed: None,
            slots,
            budgets,
            tall_prev: Vec::new(),
            tall_dead_prev: Vec::new(),
            dropped_frames: 0,
            frames: 0,
            gpu_ms: 0.0,
            gpu_stages: [0.0; 3],
            fill: [0.0; 3],
            fill_layers: [0.0; cubarium_gpu::scene::LAYER_COUNT],
            build_ms: 0.0,
            present_ms: 0.0,
            capture,
            web: None,
            web_raster: None,
            web_last: None,
            web_frames: 0,
            web_ms: 0.0,
        })
    }

    /// Feed `web` from this renderer's own readback, at most `rate` times a second.
    ///
    /// A non-positive or non-finite rate attaches nothing, which is the flag's own
    /// default: an operator asks for the viewer, and until they do the panel does not
    /// even read its raster back.
    pub fn with_web(mut self, web: WebSink, rate: f64) -> GpuSink {
        if rate.is_finite() && rate > 0.0 {
            self.web = Some((web, 1.0 / rate));
        }
        self
    }

    /// Hand the viewer this frame if enough time has passed since the last one.
    ///
    /// The readback is `vkCmdCopyImageToBuffer` of the world raster into a host buffer and
    /// a wait — the GPU has finished the frame by the time this runs, so the wait is for
    /// the copy alone — then one pass turning RGBA into the viewer's RGB. At 2 fps on a
    /// 640x360 raster that is 1.8 MB/s of copy against the 60 Hz the panel is getting.
    fn feed_web(&mut self) -> Result<()> {
        let Some((_, period)) = &self.web else {
            return Ok(());
        };
        let period = *period;
        let now = std::time::Instant::now();
        if self
            .web_last
            .is_some_and(|t| (now - t).as_secs_f64() < period)
        {
            return Ok(());
        }
        self.web_last = Some(now);
        let rgba = self.renderer.read_raster(&self.gpu)?;
        let (w, h) = (self.layout.w as u16, self.layout.h as u16);
        let raster = self.web_raster.get_or_insert_with(|| Raster::black(w, h));
        for (rgb, px) in raster
            .as_bytes_mut()
            .chunks_exact_mut(3)
            .zip(rgba.chunks_exact(4))
        {
            rgb.copy_from_slice(&px[..3]);
        }
        if let Some((web, _)) = self.web.as_mut() {
            web.submit(Output::Ring(raster))?;
        }
        self.web_frames += 1;
        self.web_ms += (std::time::Instant::now() - now).as_secs_f64() * 1e3;
        Ok(())
    }

    /// Keep the column heights the presenter is about to advance past.
    fn snapshot_tall(&mut self) {
        let n = self.presenter.columns().len();
        self.tall_prev = (0..n).map(|i| self.presenter.tall_growth_of(i)).collect();
        self.tall_dead_prev = (0..n)
            .map(|i| self.presenter.tall_dead_growth_of(i))
            .collect();
    }

    /// The presenter, for a test that wants to compare the two pictures of one view.
    pub fn presenter_mut(&mut self) -> &mut ArtPresenter {
        &mut self.presenter
    }

    /// The last rendered world raster as `w · h · 4` sRGB RGBA — the same bytes the
    /// `_SRGB` attachment wrote, so it is directly comparable with
    /// `Canvas::encode_raster`'s RGB8.
    pub fn read_raster(&self) -> Result<Vec<u8>> {
        self.renderer.read_raster(&self.gpu)
    }

    /// How many frame slots have been dropped from over-full stamps so far.
    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames
    }

    /// Render one view and present it.
    fn render(&mut self, view: &RenderView, seconds: f64, f: f64) -> Result<()> {
        let started = std::time::Instant::now();
        self.renderer.scratch_begin();
        self.build(view, seconds, f);
        let built = std::time::Instant::now();
        let ms = self
            .target
            .draw(&self.gpu, &mut self.renderer, &self.scene)?;
        let done = std::time::Instant::now();
        self.frames += 1;
        self.gpu_ms += ms;
        if let Some(split) = self.renderer.gpu_split(&self.gpu) {
            for (acc, stage) in self.gpu_stages.iter_mut().zip(split) {
                *acc += stage;
            }
        }
        if self.renderer.fill_profile {
            let f = self.renderer.fill();
            self.fill[0] += f.quad_px;
            self.fill[1] += f.tile_px;
            self.fill[2] += f.opaque_px;
            for (acc, layer) in self.fill_layers.iter_mut().zip(f.per_layer) {
                *acc += layer;
            }
        }
        self.build_ms += (built - started).as_secs_f64() * 1e3;
        self.present_ms += (done - built).as_secs_f64() * 1e3;
        if let Some(dir) = &self.capture {
            let rgba = self.renderer.read_raster(&self.gpu)?;
            let path = dir.join(format!("gpu-{:06}.png", self.frames));
            cubarium_gpu::target::write_png(&path, self.layout.w, self.layout.h, &rgba)?;
        }
        self.feed_web()?;
        Ok(())
    }

    /// Fill `self.scene` from the view, in `ArtPresenter::draw_with_fruit`'s own order.
    fn build(&mut self, view: &RenderView, seconds: f64, f: f64) {
        self.scene.clear_instances();
        self.scene.seconds = seconds;
        self.scene.tick = view.tick;
        self.scene.f = f as f32;
        self.fields(view);
        self.ground_cover(view, seconds);
        self.plants(view, seconds, f);
        self.soil_snags(view, seconds);
        self.tall(view, seconds, f);
        self.rain(view, seconds);
        self.bodies(view, seconds, f);
        self.hunters(view, seconds, f);
    }

    /// Pass 1–4 and 6's inputs: the six cell textures, re-uploaded only when the tick
    /// moves (`Fields::revision`).
    fn fields(&mut self, view: &RenderView) {
        if self.scene.fields.revision == view.tick && !self.scene.fields.producer.is_empty() {
            return;
        }
        let n = self.geom.cell_count();
        let fields = &mut self.scene.fields;
        let copy = |out: &mut Vec<f32>, src: &[f64]| {
            out.clear();
            out.extend((0..n).map(|i| src.get(i).copied().unwrap_or(0.0) as f32));
        };
        copy(&mut fields.producer, &view.producer);
        copy(&mut fields.water, &view.water);
        // The flecks and the soil wash both read `D + C` scaled exactly as the presenter
        // scales it before thresholding (`draw_with_fruit`: `litter_density · SOIL_SCALE`).
        fields.detritus.clear();
        fields
            .detritus
            .extend((0..n).map(|i| (litter_density(view, i) * SOIL_SCALE) as f32));
        fields.rain.clear();
        fields
            .rain
            .extend((0..n).map(|i| view.rain.get(i).copied().unwrap_or(0.0)));
        fields.growth.clear();
        fields.tall.clear();
        fields.growth.resize(n, 0.0);
        fields.tall.resize(n, 0.0);
        fields.producer_max = view.producer_max as f32;
        fields.revision = view.tick;
    }

    /// Pass 5: each band's tileable texture on the 8-texel lattice.
    fn ground_cover(&mut self, view: &RenderView, seconds: f64) {
        let geom = self.geom;
        for &chart in geom.topology().charts() {
            for (face, x, y) in geom.ground_points(chart) {
                let point = SurfacePoint::pixel_center(geom.topology(), face, x, y);
                let cell = cell_of(geom.topology(), geom.scale(), &point);
                let band = geom.band_of(cell);
                let Some(name) = band_name(band) else {
                    continue;
                };
                let Some(clip) = self.atlas.ground(name) else {
                    continue;
                };
                let t = crate::art_present::ground_density(view, cell.index(), band);
                // `environment::band_ground_weight` is `pub(super)`; it is the soil weight
                // for the soil tile and its complement for the others, so the texture
                // cross-fades through the horizon with the ground under it.
                let w = w_soil(geom.topology().height(&point)) as f32;
                let weight = if band == Band::Soil { w } else { 1.0 - w };
                let opacity = ground_opacity(t, band) * weight;
                if opacity <= 0.0 {
                    continue;
                }
                let at = seconds + geom.ground_phase_of(face, x, y, clip.seconds);
                let stamp = Stamp {
                    anchor: raster_of(point),
                    heading: [1.0, 0.0],
                    layers: one(pose(clip, at)),
                    opacity,
                    ..Default::default()
                };
                self.push(Layer::GroundCover, &stamp);
            }
        }
    }

    /// Passes 7 and 8's living half: one plant per cell that warrants one, plus the dead
    /// silhouette under it.
    fn plants(&mut self, view: &RenderView, seconds: f64, f: f64) {
        let geom = self.geom;
        let foliage_full = self.presenter.foliage_full();
        for (index, cell) in geom.all_cells().enumerate() {
            let growth = growth_between(
                self.presenter.growth_prev_of(cell),
                self.presenter.growth_of(cell),
                f,
            );
            let dead = growth_between(
                self.presenter.dead_growth_prev_of(cell),
                self.presenter.dead_growth_of(cell),
                f,
            );
            let bare = growth.from.is_none() && growth.to.is_none();
            let dead_bare = dead.from.is_none() && dead.to.is_none();
            if bare && dead_bare {
                continue;
            }
            let band = self.presenter.band_at(cell);
            let slot = self.slots[index];
            // `habitat::species_of`'s own table, read with the slot this sink already
            // has. Calling `geom.species_of(band, cell)` would re-derive the slot — seven
            // `SplitMix64` draws and an `embed_tangent` — once per cell per frame.
            let name = species_name(band, slot.pick);
            if self.atlas.plant(name, PlantClip::Stage(0)).is_none() {
                continue;
            }
            let t = plant_density(view, index, band);
            let thresholds = stage_thresholds(band);
            let ceiling = band_opacity(band);
            let (bend, heading) = geom.slot_wind(&slot, name, self.budget_of(name), seconds);
            let bend = bend_of(bend);
            let heading = [heading.x as f32, heading.y as f32];
            let anchor = raster_of(slot.at);
            let opacity_of = |stage: u8| stage_opacity(stage, t, &thresholds, ceiling);

            // The dead silhouette, under everything living. `tall::silhouette_layers` is
            // `pub(super)`; its three cases are transcribed here against the same
            // `stage_opacity` and `growth_step` the original calls.
            if !dead_bare
                && let Some((layers, opacity)) = self.silhouette(
                    name,
                    cell,
                    seconds,
                    dead,
                    dead_wood_density(view, index),
                    &thresholds,
                    ceiling * DEAD_WOOD_OPACITY,
                )
            {
                let stamp = Stamp {
                    anchor,
                    heading,
                    layers,
                    bend,
                    opacity,
                    tone: Some(dead_tone()),
                    ..Default::default()
                };
                self.push(Layer::Plants, &stamp);
            }
            if bare {
                continue;
            }
            let tone = StampTone {
                colour: living_wood_tone(),
                shade_floor: wood_shade().floor,
                shade_reference: wood_shade().reference,
                mix: if structural(band) {
                    1.0 - foliage_ramp_at(foliage_fullness(view, index), foliage_full)
                } else {
                    0.0
                },
            };
            let fruit_now = if fruit_stage(view.fruit.get(index).copied()) {
                growth.fruit
            } else {
                0.0
            };
            if growth.from == growth.to {
                let stage = growth.to.expect("an idle bare slot was skipped above");
                let opacity = opacity_of(stage);
                if opacity > 0.0 {
                    let stamp = Stamp {
                        anchor,
                        heading,
                        layers: self.stage_layers(name, stage, cell, seconds, fruit_now),
                        bend,
                        opacity,
                        tone: Some(tone),
                        ..Default::default()
                    };
                    self.push(Layer::Plants, &stamp);
                }
                continue;
            }
            let Some(GrowthStep {
                lower,
                upper,
                t: gu,
            }) = growth_step(growth)
            else {
                continue;
            };
            // An authored growth clip *is* the picture, held between the two idle clips.
            if let Some((low, clip)) = lower.and_then(|low| {
                self.atlas
                    .plant(name, PlantClip::Grow(low, upper))
                    .map(|c| (low, c))
            }) {
                let under = opacity_of(low);
                let opacity = under + (opacity_of(upper) - under) * gu as f32;
                if opacity > 0.0 {
                    let [w_from, w_grow, w_to] = growth_weights(gu);
                    let layers = [
                        self.stage_pose(name, low, cell, seconds, w_from),
                        pose_at(clip, gu * clip.seconds, w_grow),
                        self.stage_pose(name, upper, cell, seconds, w_to),
                    ];
                    let stamp = Stamp {
                        anchor,
                        heading,
                        layers,
                        bend,
                        opacity,
                        tone: Some(tone),
                        ..Default::default()
                    };
                    self.push(Layer::Plants, &stamp);
                }
                continue;
            }
            // The reveal-mask path: the lower stage fades out under the higher one.
            if let Some(stage) = lower {
                let opacity = opacity_of(stage) * (1.0 - gu) as f32;
                if opacity > 0.0 {
                    let stamp = Stamp {
                        anchor,
                        heading,
                        layers: self.stage_layers(name, stage, cell, seconds, fruit_now),
                        bend,
                        opacity,
                        tone: Some(tone),
                        ..Default::default()
                    };
                    self.push(Layer::Plants, &stamp);
                }
            }
            let opacity = opacity_of(upper);
            if opacity > 0.0 {
                let layers = self.stage_layers(name, upper, cell, seconds, fruit_now);
                let mask = if slot.radial {
                    StampMask::Radial {
                        reveal: gu as f32 * (self.layers_extent(&layers) + 0.5),
                    }
                } else {
                    StampMask::Axial {
                        reveal: (gu * PLANT_REVEAL_PX) as f32,
                    }
                };
                let stamp = Stamp {
                    anchor,
                    heading,
                    layers,
                    bend,
                    opacity,
                    mask,
                    tone: Some(tone),
                    ..Default::default()
                };
                self.push(Layer::Plants, &stamp);
            }
        }
    }

    /// The soil band's dead-wood stub, over its scenery.
    fn soil_snags(&mut self, view: &RenderView, seconds: f64) {
        let geom = self.geom;
        for (index, cell) in geom.all_cells().enumerate() {
            if self.presenter.band_at(cell) != Band::Soil {
                continue;
            }
            let opacity = soil_snag_opacity(view, index);
            if opacity <= 0.0 {
                continue;
            }
            let slot = self.slots[index];
            let name = species_name(Band::Soil, slot.pick);
            if self.atlas.plant(name, PlantClip::Stage(0)).is_none() {
                continue;
            }
            let (bend, heading) = geom.slot_wind(&slot, name, self.budget_of(name), seconds);
            let stamp = Stamp {
                anchor: raster_of(slot.at),
                heading: [heading.x as f32, heading.y as f32],
                layers: one(self.stage_pose(name, 0, cell, seconds, 1.0)),
                bend: bend_of(bend),
                opacity,
                mask: StampMask::Axial {
                    reveal: SOIL_SNAG_PX as f32,
                },
                tone: Some(dead_tone()),
                ..Default::default()
            };
            self.push(Layer::Plants, &stamp);
        }
    }

    /// Pass 9: the tall columns, dead wood first and then the living column.
    fn tall(&mut self, view: &RenderView, seconds: f64, f: f64) {
        let geom = self.geom;
        let columns = self.presenter.columns().to_vec();
        let foliage_full = self.presenter.foliage_full();
        for (i, column) in columns.iter().enumerate() {
            let name = TALL_PLANTS[column.pick];
            if self.atlas.tall(name, "trunk").is_none() {
                continue;
            }
            let budget = self.presenter.column_budget(column);
            let amplitude = geom.tall_amplitude(column, budget, seconds);
            let height = tall_between(
                self.tall_prev
                    .get(i)
                    .copied()
                    .unwrap_or(self.presenter.tall_growth_of(i)),
                self.presenter.tall_growth_of(i),
                f,
            )
            .height;
            let dead = tall_between(
                self.tall_dead_prev
                    .get(i)
                    .copied()
                    .unwrap_or(self.presenter.tall_dead_growth_of(i)),
                self.presenter.tall_dead_growth_of(i),
                f,
            )
            .height;
            self.column(name, column, dead, seconds, amplitude, 0.0, true);
            let cap = foliage_ramp_at(
                geom.column_fullness(view, column.face, column.cx),
                foliage_full,
            );
            self.column(name, column, height, seconds, amplitude, cap, false);
        }
    }

    /// One column at a continuous height, every row composited exactly once —
    /// `tall::draw_column_in`'s walk, minus the cube's corner-cap handoff, which has no
    /// near-corner on a ring.
    #[allow(clippy::too_many_arguments)]
    fn column(
        &mut self,
        name: &str,
        column: &crate::art_present::TallColumn,
        height: f64,
        seconds: f64,
        amplitude: f64,
        cap_opacity: f32,
        dead: bool,
    ) {
        if !(height > 0.0) {
            return;
        }
        let geom = self.geom;
        let max_segments = geom.tall_max_segments(column.face);
        let height = height.min(f64::from(max_segments));
        let fade = (height / TALL_BASE_FADE).clamp(0.0, 1.0) as f32;
        let grown = tall_grown_px(height);
        let heading = geom.tall_heading(column.face, column.cx);
        let heading = [heading.x as f32, heading.y as f32];
        let tone = dead.then(dead_tone);
        let strips = trunk_strip_of(&self.presenter, name);
        let local = |i: u8| grown - (4.0 * f64::from(i) - 8.0);

        let stamp_part = |this: &mut Self, clip: GpuClip, i: f64, mask, opacity: f32| {
            if opacity <= 0.0 {
                return;
            }
            let at = geom.tall_anchor_at(column.face, column.cx, i);
            let stamp = Stamp {
                anchor: raster_of(at),
                heading,
                layers: one(pose(
                    clip,
                    seconds + geom.tall_phase_of(column.face, column.cx, clip.seconds),
                )),
                bend: [
                    amplitude as f32,
                    tall_bend_base(i) as f32,
                    TALL_BEND_ROOT as f32,
                    TALL_BEND_LENGTH as f32,
                ],
                opacity,
                mask,
                tone,
                ..Default::default()
            };
            this.push(Layer::Tall, &stamp);
        };

        if let Some(base) = self.atlas.tall(name, "base") {
            stamp_part(self, base, 0.0, StampMask::None, TALL_OPACITY * fade);
        }
        if let Some(trunk) = self.atlas.tall(name, "trunk") {
            for i in 1..=max_segments {
                let floor = if i == 1 { TALL_FIRST_JOIN } else { strips.0 };
                let top = if i == max_segments {
                    TILE_ROWS
                } else {
                    strips.1
                };
                let reveal = local(i).min(top);
                if reveal <= floor {
                    break;
                }
                stamp_part(
                    self,
                    trunk,
                    f64::from(i),
                    StampMask::Strip {
                        floor: floor as f32,
                        reveal: reveal as f32,
                    },
                    TALL_OPACITY,
                );
            }
        }
        if !dead && let Some(crown) = self.atlas.tall(name, "crown") {
            stamp_part(
                self,
                crown,
                height + 1.0,
                StampMask::None,
                TALL_OPACITY * fade * cap_opacity,
            );
        }
        if !dead && column.vine {
            self.vine(column, grown, seconds, amplitude, fade, max_segments);
        }
    }

    /// The climber: its derived strips on every second trunk position, then its endpoint.
    fn vine(
        &mut self,
        column: &crate::art_present::TallColumn,
        grown: f64,
        seconds: f64,
        amplitude: f64,
        fade: f32,
        max_segments: u8,
    ) {
        use crate::art_present::{TALL_VINE_FLOOR, TALL_VINE_TOP};
        let geom = self.geom;
        let Some(trunk) = self.atlas.tall(VINE_PLANT, "trunk_strip") else {
            return;
        };
        let heading = geom.tall_heading(column.face, column.cx);
        let heading = [heading.x as f32, heading.y as f32];
        let bend_at = |i: f64| {
            [
                amplitude as f32,
                tall_bend_base(i) as f32,
                TALL_BEND_ROOT as f32,
                TALL_BEND_LENGTH as f32,
            ]
        };
        let mut i = 1u8;
        while i <= max_segments {
            let reveal = (grown - (4.0 * f64::from(i) - 8.0)).min(TALL_VINE_TOP);
            if reveal <= TALL_VINE_FLOOR {
                break;
            }
            let at = geom.tall_anchor_at(column.face, column.cx, f64::from(i));
            let stamp = Stamp {
                anchor: raster_of(at),
                heading,
                layers: one(pose(
                    trunk,
                    seconds + geom.tall_phase_of(column.face, column.cx, trunk.seconds),
                )),
                bend: bend_at(f64::from(i)),
                opacity: TALL_OPACITY * fade,
                mask: StampMask::Strip {
                    floor: TALL_VINE_FLOOR as f32,
                    reveal: reveal as f32,
                },
                ..Default::default()
            };
            self.push(Layer::Tall, &stamp);
            i += 2;
        }
        if let Some(endpoint) = self.atlas.tall(VINE_PLANT, "endpoint") {
            let i = f64::from(max_segments) + 1.0;
            let reveal = grown - tall_bend_base(i);
            if reveal > 7.0 {
                let at = geom.tall_anchor_at(column.face, column.cx, i);
                let stamp = Stamp {
                    anchor: raster_of(at),
                    heading,
                    layers: one(pose(
                        endpoint,
                        seconds + geom.tall_phase_of(column.face, column.cx, endpoint.seconds),
                    )),
                    bend: bend_at(i),
                    opacity: TALL_OPACITY * fade,
                    mask: StampMask::Axial {
                        reveal: reveal as f32,
                    },
                    ..Default::default()
                };
                self.push(Layer::Tall, &stamp);
            }
        }
    }

    /// Pass 10: the streaks at this frame's own instant.
    fn rain(&mut self, view: &RenderView, seconds: f64) {
        if view.rain.is_empty() {
            return;
        }
        let geom = self.geom;
        let solid = self.atlas.rect(self.atlas.solid());
        let colour = crate::present::srgb_linear(RAIN_SRGB);
        for (index, cell) in geom.all_cells().enumerate() {
            let rate = view.rain.get(index).copied().unwrap_or(0.0);
            let n = rain_streaks(rate);
            if n == 0 {
                continue;
            }
            let scale = RAIN_OPACITY * rate.min(1.0);
            for k in 0..n {
                for ((x, y), weight) in geom.rain_marks(cell, k, seconds) {
                    let alpha = (scale * weight).min(1.0);
                    if alpha <= 0.0 {
                        continue;
                    }
                    let stamp = Stamp {
                        anchor: [f32::from(x) + 0.5, f32::from(y) + 0.5],
                        heading: [1.0, 0.0],
                        scratch: None,
                        opacity: alpha,
                        tone: Some(StampTone {
                            colour,
                            shade_floor: 1.0,
                            shade_reference: 1.0,
                            mix: 1.0,
                        }),
                        ..Default::default()
                    };
                    // The one white texel: rain costs no second pipeline and no second
                    // texture, exactly as in the synthetic scene.
                    let mut instance = stamp.instance(&self.atlas).0;
                    instance.frames[0] = [solid.x, solid.y];
                    instance.size = [1, 1];
                    instance.pivot = [0, 0];
                    instance.weights = [1.0, 0.0, 0.0, 0.0];
                    self.scene.push(Layer::Rain, instance);
                }
            }
        }
    }

    /// Passes 11: the ordinary bodies, hunters excluded.
    fn bodies(&mut self, view: &RenderView, seconds: f64, f: f64) {
        for o in &view.organisms {
            if self.presenter.hunter_of(o.id).is_some() {
                continue;
            }
            let stamp = self.creature(o, seconds, f);
            self.push(Layer::Bodies, &stamp);
        }
    }

    /// `art_present::stamp_creature`, as a stamp.
    fn creature(&self, o: &OrganismView, seconds: f64, f: f64) -> Stamp {
        let pack = self.presenter.pack();
        let form = rig_of(o.form, o.hue, pack.creature_count());
        let state = crate::art_present::state_of(o);
        let names = pack.creature_names();
        let rig = names.get(form).cloned().unwrap_or_default();
        let pose_of = |st: usize, weight: f32| -> Option<PoseRef> {
            let clip = &pack.clips[form * 4 + st];
            let gpu = self.atlas.creature(&rig, st)?;
            let gestation = if st == 3 && st != state {
                Some(1.0)
            } else {
                o.gestation
            };
            let at = clip_time(clip, seconds, phase_of(o.id, clip.seconds), gestation);
            Some(pose_at(gpu, at, weight))
        };
        let (anchor, dir) =
            crate::present::interpolate_on(self.geom.topology(), &o.moved, o.pos, o.heading, f);
        let memory: Option<BodyMemory> = self.presenter.body_of(o.id);
        let heading = turn_heading(dir, memory.as_ref().map_or(0.0, |m| m.turn), f);
        let scale = if o.juvenile {
            crate::present::JUVENILE_SCALE
        } else {
            1.0
        };
        let states = match &memory {
            Some(m) if m.fade_at(seconds) < 1.0 => m.layers_at(seconds),
            _ => vec![(state, 1.0)],
        };
        let meal = self
            .presenter
            .meal_of(o.id)
            .and_then(|m| {
                let weight = m.weight_at(f);
                (weight > 0.0).then(|| m.bout_seconds(seconds).map(|t| (t, weight.min(1.0))))
            })
            .flatten();
        let mode_weight = meal.map_or(1.0, |(_, w)| 1.0 - w);
        let mut layers = [PoseRef::NONE; MAX_POSES];
        let mut n = 0usize;
        if mode_weight > 0.0 {
            for &(st, w) in &states {
                if n == MAX_POSES {
                    break;
                }
                if let Some(p) = pose_of(st, w * mode_weight) {
                    layers[n] = p;
                    n += 1;
                }
            }
        }
        if let Some((bout, w)) = meal
            && let Some(gpu) = self.atlas.creature(&rig, crate::art_present::FEED_STATE)
        {
            // The meal layer is the one that must not be dropped: it is what a feeding
            // body *is*. If three mode layers already filled the array, the lightest of
            // them gives way — the same rule `Stamp::instance` uses one level down.
            if n == MAX_POSES {
                let lightest = (0..MAX_POSES)
                    .min_by(|a, b| layers[*a].weight.total_cmp(&layers[*b].weight))
                    .unwrap_or(0);
                layers[lightest] = PoseRef::NONE;
                n = lightest;
            }
            layers[n] = pose_at(gpu, bout, w);
        }
        Stamp {
            anchor: raster_of(anchor),
            heading: [heading.x as f32, heading.y as f32],
            layers,
            scale: scale as f32,
            ..Default::default()
        }
    }

    /// Pass 12: each hunter's rig, one instance per part, through the scratch page.
    fn hunters(&mut self, view: &RenderView, seconds: f64, f: f64) {
        let _ = seconds;
        for id in self.presenter.hunter_ids() {
            let Some(o) = view.organisms.iter().find(|o| o.id == id) else {
                continue;
            };
            let Some(memory) = self.presenter.hunter_of(id) else {
                continue;
            };
            let (anchor, dir) =
                crate::present::interpolate_on(self.geom.topology(), &o.moved, o.pos, o.heading, f);
            let turn = self.presenter.body_of(id).map_or(0.0, |m| m.turn);
            let heading = turn_heading(dir, turn, f);
            let (pose, scale) = memory.living_pose(view.tick, f, &o.moved);
            self.lanternjaw.parts_living(&pose, &mut self.parts);
            let side = Vec2::new(-heading.y, heading.x);
            let root = raster_of(anchor);
            // One instance per part, at the part's own offset rotated into raster space.
            // `stamp_rig_scaled` composites the whole rig through one query; here each
            // part is its own quad, so parts that overlap *and* are translucent composite
            // in a different order. Every Lanternjaw texel is opaque or clear, so that is
            // a difference the art cannot express — see the report.
            for index in 0..self.parts.len() {
                let (w, h, pivot, texels) = {
                    let part = &self.parts[index];
                    let sprite = &part.sprite;
                    let (w, h) = (sprite.width() as u32, sprite.height() as u32);
                    let mut texels = Vec::with_capacity((w * h) as usize);
                    for y in 0..h {
                        for x in 0..w {
                            texels.push(sprite.texel(x as i32, y as i32));
                        }
                    }
                    (w, h, sprite.pivot(), texels)
                };
                let Some(origin) = self.renderer.scratch_push(w, h, &texels) else {
                    continue;
                };
                let offset = self.parts[index].offset;
                let at = [
                    root[0] + (scale * (offset.x * heading.x + offset.y * side.x)) as f32,
                    root[1] + (scale * (offset.x * heading.y + offset.y * side.y)) as f32,
                ];
                let stamp = Stamp {
                    anchor: at,
                    heading: [heading.x as f32, heading.y as f32],
                    scale: scale as f32,
                    scratch: Some(ScratchFrame {
                        origin,
                        size: [w as u16, h as u16],
                        pivot: [pivot.x.round() as u16, pivot.y.round() as u16],
                    }),
                    ..Default::default()
                };
                self.push(Layer::Bodies, &stamp);
            }
        }
    }

    // --- the small shared rules ------------------------------------------------------

    fn push(&mut self, layer: Layer, stamp: &Stamp) {
        let (instance, dropped) = stamp.instance(&self.atlas);
        self.dropped_frames += dropped as u64;
        self.scene.push(layer, instance);
    }

    /// A plant family's measured bend budget, from the table built at construction.
    fn budget_of(&self, name: &str) -> f64 {
        self.budgets.get(name).copied().unwrap_or(f64::INFINITY)
    }

    /// `tall::stage_pose`: the stage's own looping clip at the slot's phase.
    fn stage_pose(&self, name: &str, stage: u8, cell: CellId, s: f64, weight: f32) -> PoseRef {
        match self.atlas.plant(name, PlantClip::Stage(stage)) {
            Some(clip) => pose_at(clip, s + plant_phase_of(cell, clip.seconds), weight),
            None => PoseRef {
                a: 0,
                b: 0,
                mix: 0.0,
                weight: 0.0,
            },
        }
    }

    /// `tall::stage_layers`: the stage, with the fruit accent blended in where it holds.
    fn stage_layers(
        &self,
        name: &str,
        stage: u8,
        cell: CellId,
        s: f64,
        fruit: f64,
    ) -> [PoseRef; MAX_POSES] {
        let q = if fruit.is_finite() {
            fruit.clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        match self.atlas.plant(name, PlantClip::Fruit) {
            Some(clip) if stage == 2 && q > 0.0 => [
                self.stage_pose(name, stage, cell, s, 1.0 - q),
                pose_at(clip, s + plant_phase_of(cell, clip.seconds), q),
                PoseRef::NONE,
            ],
            _ => one(self.stage_pose(name, stage, cell, s, 1.0)),
        }
    }

    /// `tall::silhouette_layers`, transcribed: the shape a tinted stamp paints for the
    /// structure behind the foliage. `pub(super)` upstream, so it is here rather than
    /// called; the rules it uses — `stage_opacity`, `growth_step`, `stage_pose` — are the
    /// upstream ones.
    #[allow(clippy::too_many_arguments)]
    fn silhouette(
        &self,
        name: &str,
        cell: CellId,
        s: f64,
        growth: Growth,
        t: f64,
        thresholds: &[f64; 3],
        ceiling: f32,
    ) -> Option<([PoseRef; MAX_POSES], f32)> {
        let opacity_of = |stage: u8| stage_opacity(stage, t, thresholds, ceiling);
        if growth.from == growth.to {
            let stage = growth.to?;
            let opacity = opacity_of(stage);
            return (opacity > 0.0)
                .then(|| (one(self.stage_pose(name, stage, cell, s, 1.0)), opacity));
        }
        let GrowthStep {
            lower,
            upper,
            t: tu,
        } = growth_step(growth)?;
        let tu = if tu.is_finite() {
            tu.clamp(0.0, 1.0)
        } else {
            0.0
        } as f32;
        let (layers, opacity) = match lower {
            Some(low) => {
                let under = opacity_of(low);
                (
                    [
                        self.stage_pose(name, low, cell, s, 1.0 - tu),
                        self.stage_pose(name, upper, cell, s, tu),
                        PoseRef::NONE,
                    ],
                    under + (opacity_of(upper) - under) * tu,
                )
            }
            None => (
                one(self.stage_pose(name, upper, cell, s, 1.0)),
                opacity_of(upper) * tu,
            ),
        };
        (opacity > 0.0).then_some((layers, opacity))
    }

    /// `tall::layers_extent`: the largest extent among the layers that will be sampled.
    fn layers_extent(&self, layers: &[PoseRef; MAX_POSES]) -> f32 {
        layers
            .iter()
            .filter(|p| p.weight > 0.0)
            .map(|p| self.atlas.rect(p.a).extent.max(self.atlas.rect(p.b).extent))
            .fold(0.0, f32::max)
    }
}

/// One pose in a stamp's fixed layer array.
fn one(pose: PoseRef) -> [PoseRef; MAX_POSES] {
    [pose, PoseRef::NONE, PoseRef::NONE]
}

/// `habitat::species_of`'s table, read from a slot this caller already has.
fn species_name(band: Band, pick: usize) -> &'static str {
    use crate::art_present::{CANOPY_PLANTS, FOLIAGE_PLANTS, SOIL_PLANTS, WATER_PLANT};
    match band {
        Band::Soil => SOIL_PLANTS[pick],
        Band::Foliage => FOLIAGE_PLANTS[pick],
        Band::Canopy => CANOPY_PLANTS[pick],
        Band::Water => WATER_PLANT,
    }
}

/// On a ring a surface point's `(u, v)` **are** raster pixels, so the mapping is a cast.
fn raster_of(p: SurfacePoint) -> [f32; 2] {
    [p.u as f32, p.v as f32]
}

fn pose(clip: GpuClip, at: f64) -> PoseRef {
    pose_at(clip, at, 1.0)
}

fn pose_at(clip: GpuClip, at: f64, weight: f32) -> PoseRef {
    let (a, b, mix) = clip.pose(at);
    PoseRef { a, b, mix, weight }
}

fn bend_of(b: cubarium_render::Bend) -> [f32; 4] {
    [
        b.amplitude as f32,
        b.base as f32,
        b.root as f32,
        b.length as f32,
    ]
}

fn dead_tone() -> StampTone {
    StampTone {
        colour: dead_wood_tone(),
        shade_floor: wood_shade().floor,
        shade_reference: wood_shade().reference,
        mix: 1.0,
    }
}

fn band_name(band: Band) -> Option<&'static str> {
    match band {
        Band::Soil => Some("soil"),
        Band::Foliage => Some("foliage"),
        Band::Canopy => Some("canopy"),
        Band::Water => None,
    }
}

/// `habitat::trunk_strip` needs the pack's own `TallPlant`, which the presenter owns.
fn trunk_strip_of(presenter: &ArtPresenter, name: &str) -> (f64, f64) {
    match presenter.pack().tall_plant(name) {
        Some(plant) => trunk_strip(plant),
        None => (crate::art_present::TALL_JOIN, TILE_ROWS),
    }
}

impl FrameSink for GpuSink {
    /// `--sink gpu` never looks at the CPU's pixels; [`FrameSink::wants_pixels`] says so
    /// and the runner does not rasterise them at all.
    fn submit(&mut self, _out: Output<'_>) -> Result<()> {
        Ok(())
    }

    fn wants_pixels(&self) -> bool {
        false
    }

    fn observe_world(&mut self, view: &RenderView, hunters: &[HunterView], events: &[HunterEvent]) {
        // The presenter *snaps* on its first observe rather than advancing, and leaves
        // `tall_prev == tall` when it does; a snapshot taken before it would make every
        // column grow from nothing over one tick. So: before an ordinary advance, after a
        // snap — which is exactly what `ArtPresenter::observe` does internally.
        let first = self.observed.is_none();
        if !first {
            self.snapshot_tall();
        }
        self.presenter.observe_with_fruit(view, Some(&view.fruit));
        if first {
            self.snapshot_tall();
        }
        if let Err(e) = self.presenter.observe_hunters(view, hunters, events) {
            eprintln!("cubarium: --sink gpu cannot draw this world's hunters ({e})");
        }
        self.observed = Some(view.tick);
    }

    fn observe_counts(&mut self, population: usize, neural: usize) {
        if let Some((web, _)) = self.web.as_mut() {
            web.observe_counts(population, neural);
        }
    }

    fn observe_tick(&mut self, tick: u64) {
        if let Some((web, _)) = self.web.as_mut() {
            web.observe_tick(tick);
        }
    }

    fn observe_view(&mut self, view: &RenderView, seconds: f64, f: f64) -> Result<()> {
        // A sink attached to a resumed world may see a frame before its first tick; the
        // presenter snaps on its own first observe, exactly as the CPU one does.
        if self.observed.is_none() {
            self.presenter.observe_with_fruit(view, Some(&view.fruit));
            self.snapshot_tall();
            self.observed = Some(view.tick);
        }
        self.render(view, seconds, f)
    }

    fn should_quit(&mut self) -> bool {
        self.target.should_quit()
    }

    fn finish(&mut self) -> Result<()> {
        if self.frames > 0 {
            let n = self.frames as f64;
            eprintln!(
                "cubarium: --sink gpu drew {} frames: scene {:.2} ms, GPU {:.2} ms, \
                 submit+present {:.2} ms, {} instances{}",
                self.frames,
                self.build_ms / n,
                self.gpu_ms / n,
                self.present_ms / n,
                self.scene.instance_count(),
                match self.dropped_frames {
                    0 => String::new(),
                    d => format!(
                        ", {d} frame slot(s) dropped from over-full stamps ({:.4} per frame)",
                        d as f64 / n
                    ),
                }
            );
            eprintln!(
                "cubarium: --sink gpu GPU stages: uploads {:.2} ms, world raster {:.2} ms, \
                 present {:.2} ms",
                self.gpu_stages[0] / n,
                self.gpu_stages[1] / n,
                self.gpu_stages[2] / n,
            );
            if self.renderer.fill_profile {
                let raster = f64::from(self.layout.w) * f64::from(self.layout.h);
                eprintln!(
                    "cubarium: --sink gpu fill: quads {:.2} Mpx/frame ({:.1}x the raster), \
                     whole-tile quads would be {:.2} Mpx ({:.2}x), painted texels {:.2} Mpx \
                     ({:.0}% of the quads)",
                    self.fill[0] / n / 1.0e6,
                    self.fill[0] / n / raster,
                    self.fill[1] / n / 1.0e6,
                    self.fill[1] / self.fill[0].max(1.0),
                    self.fill[2] / n / 1.0e6,
                    100.0 * self.fill[2] / self.fill[0].max(1.0),
                );
                let by_layer: Vec<String> = cubarium_gpu::scene::LAYERS
                    .iter()
                    .map(|l| format!("{l:?} {:.2}", self.fill_layers[*l as usize] / n / 1.0e6))
                    .collect();
                eprintln!(
                    "cubarium: --sink gpu fill by layer, Mpx/frame: {}",
                    by_layer.join(", ")
                );
            }
        }
        if self.web_frames > 0 {
            eprintln!(
                "cubarium: --sink gpu fed the viewer {} frame(s) from its own readback, \
                 {:.2} ms each",
                self.web_frames,
                self.web_ms / self.web_frames as f64
            );
        }
        if let Some((web, _)) = self.web.as_mut() {
            web.finish()?;
        }
        self.target.finish(&self.gpu)
    }
}

impl Drop for GpuSink {
    fn drop(&mut self) {
        self.target.destroy(&self.gpu);
        self.renderer.destroy(&self.gpu);
    }
}
