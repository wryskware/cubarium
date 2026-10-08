//! `cubarium voxel --sink gpu`: the voxel strip drawn by `cubarium-gpu`'s slab-walk
//! shader instead of by the CPU presenter.
//!
//! # What this file is
//!
//! The half of the voxel GPU path that knows about the **world**. `cubarium_gpu::voxel`
//! owns the texel layout, the shader and the pass; this fills one texel per voxel from a
//! [`VoxelView`] and a [`FloraView`] once per tick, and hands
//! [`cubarium_gpu::VoxelParams`] the palette and every shading constant straight out of
//! [`crate::voxel::present`]. **Nothing here decides what the picture looks like**: the
//! CPU presenter's constants are the definition and this is a second reader of them, so
//! the two renderers cannot disagree about a colour — only about rasterisation, which is
//! what `examples/voxel_fidelity.rs` measures.
//!
//! # Why the plants come through `Stands`
//!
//! `Stands::rebuild` is the CPU presenter's own decomposition of a stand into voxels,
//! with its own placement rules (wood outranks canopy, canopy outranks a sprout, a cell
//! inside rock is dropped) and its own per-stand style. Reimplementing that here to pack
//! it would be a second geometry with the same name. So the packer runs the real one and
//! copies the grid it produces.
//!
//! The one thing it must change is the **style index**: `Stands` numbers styles per
//! stand, up to 65 535 of them, and a texel carries eight bits. Styles are therefore
//! deduplicated by value — every seed bank of one species is one style, and identical
//! stands collapse — and the remapping is memoised per `Stands` index so the dedup is
//! paid once per stand rather than once per voxel. A frame that still needs more than
//! [`MAX_STYLES`] distinct styles paints the excess with style 0 and says how many times.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cubarium_gpu::target::presenter::PresentSample;

use anyhow::{Context, Result, bail};

use cubarium_gpu::vk::Gpu;
use cubarium_gpu::voxel::{
    MAX_GLYPHS, MAX_STYLES, PART_ANIMAL_INTERIM, PART_NONE, POSE_ENTRIES, PLANE_GLOW, PLANE_GLYPHS, PLANE_ROOF,
    PLANE_CANOPY, PLANE_SKY, PLANE_STYLES, SLOW_PLANES, FrameClock, VoxelParams, VoxelRenderer, VoxelStaging, VoxelStyle, VoxelTexel, VoxelTextures,
};
use cubarium_voxel::{Material, Materials, VoxelView, World};
use cubarium_voxel_flora::{FaceDraw, Flora, FloraView};

use crate::present::srgb_linear;
use crate::sink::{FrameSink, Output, WebSink};
use crate::voxel::{Lighting, VoxelConfig};
use crate::voxel::animal::{AnimalPart, Animals, Specimen};
use crate::voxel::candle::{self, Candle};
use crate::voxel::sleep;
use crate::voxel::{appearance, colours};
use crate::voxel::figure::{self, AnimalDraw, FigureModels, FigureMotion, FigurePick, FigureTrack, Spec};
use crate::voxel::model::{ModelLibrary, Tag};
use crate::voxel::present as cpu;
use crate::voxel::project::Projection;
use crate::voxel::stand::{Part, Stands};
use crate::voxel::textures::SpeciesFaces;
use crate::voxel::vine::{self, VineCell};
use cubarium_voxel_fauna::{Fauna, FaunaView};

use super::light::{Canopy, Glow, GlowSet, SkyWorker};
use super::palette::{self, Palette, StyleKey};
use super::pack_timer::{count, lap};
use super::sunvis::{Snapshot, SunVisDriver};
use super::target::{GpuTarget, GpuTargetKind};

// The texel's six entry bits, the renderer's uniform array and the pose's own table agree.
const _: () = assert!(sleep::POSE_ENTRIES == POSE_ENTRIES && POSE_ENTRIES <= 64);

/// The pack oracle: the change-only pack held to the full one (tests and
/// `examples/pack_oracle.rs`).
#[path = "oracle.rs"]
#[doc(hidden)]
pub mod oracle;

/// The renderer hosted in an app: the Godot world view's (H1, D-0005).
#[path = "hosted.rs"]
pub mod hosted;

/// What the pack reads, copied off the live world (H3).
#[path = "snapshot.rs"]
pub mod snapshot;
use snapshot::{Hint, Mirror, PackSnapshot};

/// The knobs `cubarium voxel --sink gpu` takes.
#[derive(Clone, Debug)]
pub struct VoxelGpuSinkOptions {
    /// Where the frames go.
    pub target: GpuTargetKind,
    /// `--gpu-capture`: a PNG of every rendered raster.
    pub capture: Option<PathBuf>,
    /// Read the roof gap from the uploaded table rather than walking the column in the
    /// shader. Both draw the same picture; `--gpu-roof-walk` turns the table off so the
    /// two can be timed against each other.
    pub roof_from_texture: bool,
    /// The baked voxel models organisms are drawn with (package V), or `None` for the
    /// dev-mode glyphs.
    pub models: Option<Arc<ModelLibrary>>,
    /// A face-texture directory under `cfg.textures_dir`, filling whatever it lacks
    /// ([`crate::voxel::textures::load_layers`]): a preview of a few candidate species over
    /// the repository's full set.
    pub textures_under: Option<PathBuf>,
    /// Unsimulated animals drawn from the models' `specimens` (`voxel_specimens` only).
    pub specimens: Vec<Specimen>,
}

impl Default for VoxelGpuSinkOptions {
    fn default() -> VoxelGpuSinkOptions {
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            capture: None,
            roof_from_texture: true,
            models: None,
            textures_under: None,
            specimens: Vec::new(),
        }
    }
}

/// Where a sink's frames go: a [`GpuTarget`] (a window, the panel, a headless image), or
/// [`Offscreen`]: nowhere, for a host that records the renderer's frames itself (the
/// hosted renderer, which reads its raster back on its own ring).
pub trait SinkTarget {
    fn destroy(&mut self, gpu: &Gpu);
}

impl SinkTarget for GpuTarget {
    fn destroy(&mut self, gpu: &Gpu) {
        GpuTarget::destroy(self, gpu);
    }
}

/// No target: the renderer's raster alone. Unlike a [`GpuTarget`] (whose window is tied to
/// its thread) it is `Send`, so a sink with it can move to a presenter thread.
#[derive(Clone, Copy, Debug, Default)]
pub struct Offscreen;

impl SinkTarget for Offscreen {
    fn destroy(&mut self, _gpu: &Gpu) {}
}

/// `cubarium voxel --sink gpu`.
pub struct VoxelGpuSink<T: SinkTarget = GpuTarget> {
    /// Shared with the presenting thread, which submits on the same queue.
    gpu: std::sync::Arc<Gpu>,
    renderer: VoxelRenderer,
    target: T,
    /// Turns a tick's world into texels.
    packer: Packer,
    capture: Option<PathBuf>,
    /// The lit tier's water animation rate (`[light] water_hz`).
    water_hz: f32,
    /// Whether the water moves every frame rather than in steps (`[light] water_smooth`).
    water_smooth: bool,
    /// Capture-only: draw every staged world as if it were raining ([`Self::set_force_rain`]).
    force_rain: bool,
    /// Capture-only: a scripted weather loop over the world's own
    /// ([`Self::set_weather_preview`]), and the sim time and tick its first frame drew.
    weather_preview: Option<(super::weather::WeatherPreview, Option<(f64, u64)>)>,
    /// Every column's top (highest solid voxel, +1; 0 for an all-air column), `x + z·width`,
    /// for the terrain of this `terrain_version`: where a lightning strike lands, and the
    /// fog's floor.
    tops: (Option<u64>, Vec<u16>),
    /// The sky this world is drawn under, kept while the founding frame dims it.
    founding_sky: Option<([f32; 3], [f32; 3])>,
    /// When the founding frame's pulse started, so a held founding frame keeps its phase.
    founding_since: Option<Instant>,
    /// The lit tier's first world frame waits for its sky plane: the world is founded,
    /// but until the sky plane for its terrain arrives every pack is refused (the world
    /// stays owed) and the founding frame goes on pulsing.
    awaiting_light: bool,
    /// The light shafts' sun-visibility bakes (`[light] volumetric`), or `None`.
    sunvis: Option<SunVisDriver>,
    /// The frame clock in seconds of sim time ([`VoxelGpuSink::set_clock`]).
    clock_s: f64,
    /// The operator's viewer, fed from this renderer's own readback.
    web: Option<(WebSink, f64)>,
    web_raster: Option<cube_proto::Raster>,
    web_last: Option<Instant>,
    web_frames: u64,
    web_ms: f64,
    // --- measurements ---
    frames: u64,
    ticks_staged: u64,
    /// Ticks whose pack was refused because the GPU still held every staging buffer.
    packs_skipped: u64,
    /// Frames that put an already-drawn raster on a target instead of drawing the world
    /// again. A frame between two ticks is the same picture; this counts how many of
    /// them the renderer was spared.
    reshown: u64,
    /// The presenter's totals as of the last report, so each report is about its own
    /// interval rather than about the whole run.
    presenter_mark: PresentSample,
    /// When the sink opened, for the last report's own rate.
    opened: Instant,
    gpu_ms: f64,
    gpu_stages: [f64; 3],
    present_ms: f64,
    pack_ms: f64,
    capture_ms: f64,
    // --- the model pass (H2) ---
    /// The figures between ticks, when animals are drawn as models (`[figures] animals`).
    figure_track: Option<FigureTrack>,
    /// The animal drawn highlighted, and how far toward white.
    figure_highlight: Option<(u64, f32)>,
    /// The last frame's figures, for picking.
    figure_pick: Arc<FigurePick>,
    /// The tick the flat tier last moved its figures at (it moves them at the tick rate).
    figure_tick: Option<u64>,
    /// GPU milliseconds of the model pass over the frames that drew it, and those frames.
    figure_ms: f64,
    figure_frames: u64,
    // --- the pack off the loop (H3) ---
    /// The snapshots' dense water grids ([`snapshot`]).
    mirror: Mirror,
    /// The tick of the world last staged.
    staged_tick: Option<u64>,
    last_pack_ms: f64,
    last_phases: Vec<(&'static str, f64)>,
    /// The run loop's snapshot packed beside its next step ([`Self::pack_beside`]).
    behind: Option<Box<Behind>>,
}

/// The run loop's pack beside the step ([`VoxelGpuSink::pack_beside`]): the copier, the
/// one snapshot buffer, and whether it holds a tick not yet packed.
struct Behind {
    copier: snapshot::SnapshotCopier,
    snap: PackSnapshot,
    pending: bool,
}

impl VoxelGpuSink<Offscreen> {
    /// A sink with no target, for a host that records and reads back the renderer's frames
    /// itself ([`hosted`]).
    pub fn offscreen(cfg: &VoxelConfig, proj: Projection, models: Option<Arc<ModelLibrary>>) -> Result<Self> {
        let options = VoxelGpuSinkOptions { target: GpuTargetKind::Headless, models, ..Default::default() };
        Self::build(cfg, proj, options, |_, _| Ok(Offscreen))
    }
}

impl<T: SinkTarget> VoxelGpuSink<T> {
    /// Open the device and build the renderer for this world and projection, its frames
    /// going to the target `open` makes.
    fn build(
        cfg: &VoxelConfig,
        proj: Projection,
        options: VoxelGpuSinkOptions,
        open: impl FnOnce(&std::sync::Arc<Gpu>, &mut VoxelRenderer) -> Result<T>,
    ) -> Result<Self> {
        let params = params_of(cfg, proj, options.roof_from_texture);
        let gpu = std::sync::Arc::new(
            Gpu::open(options.target.instance_extensions())
                .context("opening the Vulkan device for --sink gpu")?,
        );
        eprintln!(
            "cubarium voxel: --sink gpu on {} ({:.0} KiB per tick)",
            gpu.name,
            params.upload_bytes() as f64 / 1024.0
        );
        let textures = load_textures(cfg, options.textures_under.as_deref(), &params);
        let species = SpeciesFaces::of(&textures);
        let mut renderer = VoxelRenderer::new(&gpu, params, &textures)?;
        let target = open(&gpu, &mut renderer)?;
        let mut packer = Packer {
            specimens: options.specimens.clone(),
            ..Packer::new(
                &params,
                options.models.clone(),
                textures.vine_on,
                species,
                Emission::of(cfg),
            )
        };
        packer.animals.set_hunker_s(cfg.hunker_s);
        packer.animals.set_meal_flash(cfg.meal_flash);
        let models_on = cfg.figures.animals == AnimalDraw::Models;
        packer.figures = models_on;
        packer.stands.set_foliage_fade_s(cfg.foliage_fade_s);
        Ok(VoxelGpuSink {
            figure_track: models_on.then(|| FigureTrack::new(cfg.figures.clone(), params.width)),
            figure_highlight: None,
            figure_pick: Arc::new(FigurePick::default()),
            figure_tick: None,
            figure_ms: 0.0,
            figure_frames: 0,
            gpu,
            renderer,
            target,
            packer,
            capture: options.capture,
            water_hz: cfg.light.water_hz,
            water_smooth: cfg.light.water_smooth,
            force_rain: false,
            weather_preview: None,
            tops: (None, Vec::new()),
            founding_sky: None,
            founding_since: None,
            awaiting_light: false,
            sunvis: params
                .effects
                .volume_on(params.lit)
                .then(|| SunVisDriver::new(f64::from(cfg.light.volumetric_rebake_s))),
            clock_s: 0.0,
            web: None,
            web_raster: None,
            web_last: None,
            web_frames: 0,
            web_ms: 0.0,
            frames: 0,
            ticks_staged: 0,
            packs_skipped: 0,
            reshown: 0,
            presenter_mark: PresentSample::default(),
            opened: Instant::now(),
            gpu_ms: 0.0,
            gpu_stages: [0.0; 3],
            present_ms: 0.0,
            pack_ms: 0.0,
            capture_ms: 0.0,
            mirror: Mirror::default(),
            staged_tick: None,
            last_pack_ms: 0.0,
            last_phases: Vec::new(),
            behind: None,
        })
    }

    /// Feed `web` from this renderer's own readback, at most `rate` times a second.
    pub fn with_web(mut self, web: WebSink, rate: f64) -> Self {
        if rate.is_finite() && rate > 0.0 {
            self.web = Some((web, 1.0 / rate));
        }
        self
    }

    pub fn params(&self) -> VoxelParams {
        self.renderer.params()
    }

    /// The last rendered world raster as `w · h · 4` sRGB RGBA — the same bytes the
    /// `_SRGB` attachment wrote, directly comparable with `Canvas::encode_raster`'s RGB8.
    /// Run the presenter's frame clock (the film's easing and the foliage cross-fades) on
    /// `now` instead of the wall clock; `None` goes back to it. A capture that stages its
    /// own frames passes each frame's time, so the fades play at real speed in the video.
    pub fn set_presenter_clock(&mut self, now: Option<Instant>) {
        self.packer.set_film_now(now);
    }

    pub fn read_raster(&self) -> Result<Vec<u8>> {
        self.renderer.read_raster(&self.gpu)
    }

    /// Turn the picture `x` raster columns round the ring (the hosted view's pan).
    pub fn set_view_x(&mut self, x: i32) {
        self.renderer.set_view_x(x);
    }



    /// Pack one tick's world into the renderer's staging buffer. Call it whenever the
    /// world or the plant layer has moved — a tick, or a stdin command that changed a
    /// cell — and not per frame: a frame is one draw over whatever was last staged.
    ///
    /// Returns false when the renderer had no staging buffer free — the GPU is still
    /// reading every one of them. **The world is then still owed a pack**: the caller
    /// keeps its `moved` flag and tries again next frame, which is a tick-stale picture
    /// rather than a wrong one. Nothing is rebuilt in that case, so a refused pack costs
    /// nothing but the check. In the lit tier a pack is also refused between
    /// [`VoxelGpuSink::founded`] and the arrival of the world's sky plane, while the
    /// founding frame goes on.
    pub fn stage_world(&mut self, world: &World, flora: &Flora, fauna: &Fauna) -> bool {
        self.stage_view(&world.view(), flora.view(), fauna.view())
    }

    /// [`VoxelGpuSink::stage_world`] from views: what a tool drawing a posed flora (its
    /// stands' moisture or parcel set by hand) stages.
    pub fn stage_view(
        &mut self,
        view: &VoxelView<'_>,
        flora: FloraView<'_>,
        fauna: FaunaView<'_>,
    ) -> bool {
        let (stager, _) = self.stager();
        stager.stage(view, flora, fauna, Hint::default())
    }

    /// [`VoxelGpuSink::stage_world`] from a [`PackSnapshot`] of the world (H3): the same
    /// pack, of the tick the snapshot was taken at, with the snapshot's pore-water window
    /// ([`snapshot`]). Taking the same snapshot again after a refusal is safe.
    pub fn stage_snapshot(&mut self, snap: &mut PackSnapshot) -> bool {
        let (stager, mirror) = self.stager();
        stage_snapshot_with(stager, mirror, snap)
    }

    /// [`VoxelGpuSink::stage_snapshot`] on a thread of its own while `beside` runs on this
    /// one, returning both results: the run loop steps the next tick beside the pack of
    /// the last, so a tick costs the longer of the two rather than their sum. The target
    /// (a window, tied to this thread) is never touched by the pack.
    pub fn stage_snapshot_beside<R>(&mut self, snap: &mut PackSnapshot, beside: impl FnOnce() -> R) -> (bool, R) {
        let (stager, mirror) = self.stager();
        let (staged, phases, r) = std::thread::scope(|scope| {
            let pack = std::thread::Builder::new()
                .name("cubarium-pack".into())
                .spawn_scoped(scope, move || {
                    let staged = stage_snapshot_with(stager, mirror, snap);
                    (staged, super::pack_timer::phases())
                })
                .expect("spawning the pack's thread");
            let r = beside();
            match pack.join() {
                Ok((staged, phases)) => (staged, phases, r),
                Err(e) => std::panic::resume_unwind(e),
            }
        });
        self.last_phases = phases;
        (staged, r)
    }

    /// The parts of this sink a pack writes, and the snapshot mirror: everything but the
    /// target, so a pack can run on another thread.
    fn stager(&mut self) -> (Stager<'_>, &mut Mirror) {
        (
            Stager {
                renderer: &mut self.renderer,
                packer: &mut self.packer,
                sunvis: &mut self.sunvis,
                figure_track: &mut self.figure_track,
                tops: &mut self.tops,
                founding_sky: &mut self.founding_sky,
                awaiting_light: &mut self.awaiting_light,
                figure_tick: &mut self.figure_tick,
                packs_skipped: &mut self.packs_skipped,
                ticks_staged: &mut self.ticks_staged,
                pack_ms: &mut self.pack_ms,
                last_pack_ms: &mut self.last_pack_ms,
                staged_tick: &mut self.staged_tick,
                clock_s: self.clock_s,
                weather_preview: self.weather_preview.is_some(),
                force_rain: self.force_rain,
            },
            &mut self.mirror,
        )
    }

    /// **Pack beside the step** (H3, `cubarium voxel --sink gpu`): from now on the run loop
    /// takes a snapshot after its ticks ([`Self::snapshot`]) and the snapshot is packed on
    /// its own thread while the next tick steps ([`Self::step_beside`]). The picture then
    /// trails the simulation by one tick, and the frame clock runs from the tick it shows
    /// ([`Self::staged_tick`]). `pore_sweep`: [`crate::voxel::PackConfig::pore_sweep`].
    pub fn pack_beside(&mut self, pore_sweep: u32) {
        self.behind = Some(Box::new(Behind {
            copier: snapshot::SnapshotCopier::new(pore_sweep),
            snap: PackSnapshot::default(),
            pending: false,
        }));
    }

    /// Whether [`Self::pack_beside`] is on.
    pub fn packs_beside(&self) -> bool {
        self.behind.is_some()
    }

    /// Copy what the pack reads of the world now (a tick not packed yet is replaced, its
    /// pore windows kept). Returns the copy's milliseconds, or `None` when
    /// [`Self::pack_beside`] is off.
    pub fn snapshot(&mut self, world: &World, flora: &Flora, fauna: &Fauna) -> Option<f64> {
        let b = self.behind.as_mut()?;
        b.copier.fill(&world.view(), flora.view(), fauna.view(), &mut b.snap);
        b.pending = true;
        Some(b.snap.copy_ms())
    }

    /// Whether a snapshot waits to be packed.
    pub fn snapshot_pending(&self) -> bool {
        self.behind.as_ref().is_some_and(|b| b.pending)
    }

    /// Run `step` (the next tick) on this thread while the waiting snapshot, if any, is packed
    /// on another. A refused pack leaves the snapshot waiting.
    pub fn step_beside<R>(&mut self, step: impl FnOnce() -> R) -> R {
        let Some(mut b) = self.behind.take().filter(|b| b.pending) else { return step() };
        let (staged, r) = self.stage_snapshot_beside(&mut b.snap, step);
        b.pending = !staged;
        self.behind = Some(b);
        r
    }

    /// Pack the waiting snapshot here and now (no tick is coming to pack it beside: the run
    /// is paused). False when it was refused, or none waits.
    pub fn stage_snapshot_now(&mut self) -> bool {
        let Some(mut b) = self.behind.take() else { return false };
        let staged = b.pending && self.stage_snapshot(&mut b.snap);
        if staged {
            b.pending = false;
        }
        self.behind = Some(b);
        staged
    }

    /// The tick of the world last staged, if any: what the frame clock should run from
    /// when the pack trails the simulation ([`VoxelGpuSink::stage_snapshot_beside`]).
    pub fn staged_tick(&self) -> Option<u64> {
        self.staged_tick
    }

    /// Milliseconds the last pack took (refused or not), and its phases on the thread it
    /// ran on (`pack-profile`; a pack run beside the step, [`Self::stage_snapshot_beside`]).
    pub fn last_pack(&self) -> (f64, &[(&'static str, f64)]) {
        (self.last_pack_ms, &self.last_phases)
    }

    /// The frame's time: the sim's `tick` plus the `fraction` of the next tick elapsed.
    /// The lit tier's water animates on it (`[light] water_hz` steps a second of sim
    /// time), and redraws a frame whose step has moved while there is water; the flat
    /// tier ignores it.
    ///
    /// The weather eases on it (`cubarium_gpu::weather`); a preview
    /// ([`Self::set_weather_preview`]) sets the weather here, every frame, from its script.
    pub fn set_clock(&mut self, tick: u64, fraction: f64) {
        self.clock_s = (tick as f64 + fraction) / f64::from(cubarium_voxel::TICK_HZ);
        if let Some((preview, start)) = &mut self.weather_preview {
            let hz = f64::from(cubarium_voxel::TICK_HZ);
            let now = (tick as f64 + fraction) / hz;
            let (t0, tick0) = *start.get_or_insert((now, tick));
            let view = preview.at(now - t0, tick0, hz);
            let (tops, width) = (&self.tops.1, self.renderer.params().width);
            self.renderer.set_weather(super::weather::weather_of(&view, |x, z| {
                super::weather::top_of(tops, width, x, z)
            }));
        }
        self.renderer.set_clock(FrameClock::at(
            tick,
            fraction,
            f64::from(self.water_hz),
            f64::from(cubarium_voxel::TICK_HZ),
            self.water_smooth,
        ));
        // The glowcaps' candles move on the frame clock; the light they cast follows at the
        // tick (`Packer::prepare`).
        if let Some(c) = self.packer.candle {
            self.renderer.set_emit_gains(&candle_gains(Some(c), self.clock_s));
        }
        self.move_figures(tick as f64 + fraction.clamp(0.0, 1.0));
    }

    /// The figures at clock `t` (ticks) to the renderer, and to picking. The lit tier moves
    /// them every frame; the flat tier, which redraws at the tick rate, once a tick.
    fn move_figures(&mut self, t: f64) {
        let Some(track) = &mut self.figure_track else { return };
        let p = self.renderer.params();
        if !p.lit {
            let tick = t.floor() as u64;
            if self.figure_tick == Some(tick) {
                return;
            }
            self.figure_tick = Some(tick);
        }
        let (frame, figures) = track.frame(t, self.figure_highlight, &self.packer.figure_models);
        self.renderer.set_figures(frame);
        self.figure_pick = Arc::new(FigurePick {
            s: p.s,
            rise: p.rise,
            base: p.base,
            width: p.width,
            figures,
            voxels: self.packer.figure_models.voxels(),
        });
    }

    /// Whether animals are drawn as models (`[figures] animals = "models"`).
    pub fn figures_on(&self) -> bool {
        self.figure_track.is_some()
    }

    /// Draw animal `id` leaning `mix` toward white (the model pass's highlight), from the
    /// next clock on.
    pub fn set_figure_highlight(&mut self, highlight: Option<(u64, f32)>) {
        if highlight != self.figure_highlight {
            self.figure_highlight = highlight;
            self.figure_tick = None;
        }
    }

    /// Draw animals with the live settings `m` (movement, subpixel placement: the app's
    /// Settings page) from the next clock on. Nothing with stamped animals.
    pub fn set_figure_motion(&mut self, m: FigureMotion) {
        if let Some(track) = &mut self.figure_track
            && track.set_motion(m)
        {
            // The flat tier moves its figures once a tick: this tick's move again.
            self.figure_tick = None;
        }
    }

    /// The live settings the animals are drawn with; `None` with stamped animals.
    pub fn figure_motion(&self) -> Option<FigureMotion> {
        self.figure_track.as_ref().map(FigureTrack::motion)
    }

    /// The last frame's figures, for picking.
    pub fn figure_pick(&self) -> Arc<FigurePick> {
        self.figure_pick.clone()
    }

    /// The model pass's mean GPU milliseconds over the frames that drew it, and how many.
    pub fn figure_gpu(&self) -> (f64, u64) {
        (self.figure_ms / self.figure_frames.max(1) as f64, self.figure_frames)
    }

    /// Capture-only: draw the lit tier's derived water flow field over the water instead
    /// of the water (`VoxelParams::debug_flow`). Never set by the live display.
    pub fn set_debug_flow(&mut self, on: bool) -> Result<()> {
        let mut params = self.renderer.params();
        params.debug_flow = on;
        self.renderer.set_params(params)
    }

    /// Capture-only: draw every world staged from now on as if it were raining (the rain
    /// streaks, and the lit tier's rain rings), whatever its weather. Never set by the
    /// live display.
    pub fn set_force_rain(&mut self, on: bool) {
        self.force_rain = on;
    }

    /// Capture-only (`--weather-preview`): draw the weather from a scripted loop starting
    /// at the next frame, whatever the world's own weather is. `None` goes back to the
    /// world's. Never set by the live display.
    pub fn set_weather_preview(&mut self, preview: Option<super::weather::WeatherPreview>) {
        self.weather_preview = preview.map(|p| (p, None));
    }

    /// Whether the lit tier's sky plane for the terrain last staged is still being
    /// computed off the loop thread; until it arrives the picture keeps the sky it had.
    /// Always false in the flat tier. A capture that must show the finished light stages
    /// again until this is false.
    pub fn light_pending(&self) -> bool {
        self.packer.light_pending() || self.sunvis.as_ref().is_some_and(|s| s.pending())
    }

    /// Wait up to `timeout` for the lit tier's sky plane being computed off this thread,
    /// without packing: true once it has finished (the next [`Self::stage_world`] takes it
    /// in), false if it has not by then. True at once when no sky plane is awaited (the flat
    /// tier, or it is in), so a caller staging until [`Self::light_pending`] clears stages
    /// again, as it would have. A still world waits here instead of packing to poll.
    pub fn wait_sky(&mut self, timeout: Duration) -> bool {
        match self.packer.light.as_mut() {
            Some(l) if !l.current => l.worker.wait(timeout),
            _ => true,
        }
    }

    /// Capture-only: point the lit tier's sun somewhere else (normalised; at or below the
    /// horizon, no sun), as a moving sun would. The light shafts' volume follows it by
    /// rebaking (`[light] volumetric_rebake_s`).
    pub fn set_sun(&mut self, sun: [f32; 3]) -> Result<()> {
        let mut params = self.renderer.params();
        params.sun = sun_direction(sun);
        self.renderer.set_params(params)
    }

    /// The light shafts' bakes so far: how many, their mean wall time in milliseconds, how
    /// many were skipped as unchanged, and the threads a bake runs on.
    pub fn sunvis_bakes(&self) -> Option<(u64, f64, u64, usize)> {
        self.sunvis.as_ref().map(|s| {
            (s.baked, s.bake_ms / s.baked.max(1) as f64, s.unchanged, s.threads)
        })
    }

    /// Pack the next staged world in full, as a first tick does. For
    /// `examples/pack_profile.rs`'s check of the change-only uploads.
    #[doc(hidden)]
    pub fn pack_next_in_full(&mut self) {
        self.packer.delta.full = true;
    }

    /// What the last staged pack will upload, KiB: its voxel bricks (or volume) and the
    /// slow planes the images lack. For `examples/pack_profile.rs`.
    pub fn last_upload_kib(&self) -> f64 {
        self.renderer.last_upload_bytes() as f64 / 1024.0
    }

    /// Style slots the last pack used, of `MAX_STYLES`, and how many follow a candle curve.
    /// For captures.
    pub fn styles_used(&self) -> (usize, usize) {
        let p = &self.packer.palette;
        let keys = (0..MAX_STYLES).filter_map(|s| p.key(s as u8));
        (p.live(), keys.filter(|k| palette::curve_of(k) != 0).count())
    }

    /// The lit tier's emitting voxels in the last pack, with their emissive colours in
    /// linear light: empty in the flat tier and with emission off. For captures.
    pub fn emitters(&self) -> &[((u32, u32, u32), [f32; 3])] {
        self.packer.light.as_ref().map_or(&[], |l| &l.emitters)
    }

    /// Draw these covered faces instead of the flora's own (`None` goes back to the
    /// flora's): a fixture that poses spur phases and dormancy the simulation would take
    /// hours to reach.
    pub fn set_cover_draws(&mut self, draws: Option<Vec<FaceDraw>>) {
        self.packer.stands.set_cover_draws(draws.clone());
        self.packer.cover_override = draws;
    }


    /// What this sink has spent, cumulatively: packing ticks, and drawing frames. The
    /// run loop prints the difference between two of these.
    pub fn draw_split(&self) -> (f64, f64) {
        (self.pack_ms, self.present_ms)
    }

    /// Frames drawn, and the GPU's milliseconds over them, cumulatively: uploads, the
    /// slab walk, the present pass. A tool prints the difference between two of these.
    pub fn gpu_stage_totals(&self) -> (u64, [f64; 3]) {
        (self.frames, self.gpu_stages)
    }



    /// Dim the sky to the founding pulse at `seconds`.
    fn pulse(&mut self, seconds: f64) -> Result<()> {
        let mut params = self.renderer.params();
        let base = *self
            .founding_sky
            .get_or_insert((params.sky, params.sky_horizon));
        let k = crate::voxel::present::founding_pulse(seconds);
        let dim = |c: [f32; 3]| [c[0] * k, c[1] * k, c[2] * k];
        params.sky = dim(base.0);
        params.sky_horizon = dim(base.1);
        self.renderer.set_params(params)
    }

    /// The world exists: put the sky back where the palette had it. In the lit tier that
    /// waits for the world's sky plane (the first pack that has it), so the first world
    /// frame is drawn in its finished light and the founding pulse covers the wait.
    pub fn founded(&mut self) -> Result<()> {
        if self.founding_sky.is_some() && self.renderer.params().lit {
            self.awaiting_light = true;
            return Ok(());
        }
        self.restore_sky()
    }

    fn restore_sky(&mut self) -> Result<()> {
        restore_sky(&mut self.founding_sky, &mut self.renderer)
    }


    /// Hand the viewer this frame if enough time has passed since the last one.
    fn feed_web(&mut self) -> Result<()> {
        let Some((_, period)) = &self.web else {
            return Ok(());
        };
        let period = *period;
        let now = Instant::now();
        if self
            .web_last
            .is_some_and(|t| (now - t).as_secs_f64() < period)
        {
            return Ok(());
        }
        self.web_last = Some(now);
        let p = self.renderer.params();
        let rgba = self.renderer.read_raster(&self.gpu)?;
        let raster = self
            .web_raster
            .get_or_insert_with(|| cube_proto::Raster::black(p.raster_w as u16, p.raster_h as u16));
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
        self.web_ms += (Instant::now() - now).as_secs_f64() * 1e3;
        Ok(())
    }



}

impl VoxelGpuSink<GpuTarget> {
    /// Open the device and build the renderer for this world and projection.
    pub fn new(cfg: &VoxelConfig, proj: Projection, options: VoxelGpuSinkOptions) -> Result<VoxelGpuSink> {
        let kind = options.target;
        // The panel is presented on its own thread: the queue submit and the fence wait
        // are 8 ms the run loop has better things to do with. Every other target is
        // unchanged (`sink/gpu/target.rs`).
        Self::build(cfg, proj, options, |gpu, renderer| {
            GpuTarget::open(kind, gpu, renderer, "cubarium — voxel strip (GPU)", true)
        })
    }

    /// A measurement/capture target at an exact output size, pixels. Set before the
    /// first frame; its normal presentation pass is included in the render wall time.
    pub fn set_headless_size(&mut self, size: (u32, u32)) -> Result<()> {
        anyhow::ensure!(self.frames == 0 && matches!(self.target, GpuTarget::Headless(_)),
            "set_headless_size needs a headless sink before its first frame");
        let sized = cubarium_gpu::target::Headless::with_size(&self.gpu, &self.renderer, size)?;
        self.target.destroy(&self.gpu);
        self.target = GpuTarget::Headless(Box::new(sized));
        Ok(())
    }

    /// The sized headless attachment, including its letterbox, as sRGB RGBA8.
    pub fn read_headless_output(&self) -> Result<Vec<u8>> {
        match &self.target {
            GpuTarget::Headless(target) => target.read_output(&self.gpu, &self.renderer),
            _ => anyhow::bail!("read_headless_output needs a headless sink"),
        }
    }

    /// What the panel has actually been shown, where the target knows: presented frames
    /// and frames the loop had nowhere to put. `None` when every frame drawn is
    /// presented, which is every target but the panel's presenting thread.
    pub fn presented(&self) -> Option<(u64, u64)> {
        self.target.presented()
    }

    /// Where a present's time went over the last `seconds`, and what never became one.
    ///
    /// **This is the line that says what the panel's ceiling is.** Each phase is per
    /// present, so they sum to the interval the thread had per frame: mostly idle means
    /// this thread is not the ceiling; mostly fence means the GPU; mostly submit means a
    /// driver that renders on the calling thread; mostly slots means the daemon's pacing.
    /// The redraw-versus-re-present split says whether it is the world pass or the
    /// upscale onto the panel.
    pub fn presenter_line(&mut self, seconds: f64) -> Option<String> {
        let mut now = self.target.sample()?;
        now.refused_packs = self.packs_skipped;
        let since = now.since(&self.presenter_mark);
        self.presenter_mark = now;
        Some(since.line(seconds, &self.gpu.name))
    }

    /// Draw one frame of whatever was last staged, and present it.
    /// Draw the founding frame: whatever is staged — an empty world, before the real one
    /// exists — under a sky at [`crate::voxel::present::founding_pulse`]'s brightness.
    ///
    /// The uniform block is the only thing that moves, so this costs one buffer write and
    /// the same draw every other frame pays.
    pub fn founding(&mut self, seconds: f64) -> Result<()> {
        self.founding_since
            .get_or_insert_with(|| Instant::now() - Duration::from_secs_f64(seconds.max(0.0)));
        self.pulse(seconds)?;
        self.render()
    }

    pub fn render(&mut self) -> Result<()> {
        if self.awaiting_light
            && let Some(since) = self.founding_since
        {
            self.pulse(since.elapsed().as_secs_f64())?;
        }
        if let Some(sv) = &mut self.sunvis {
            sv.frame(self.clock_s, self.renderer.params().sun, &mut self.renderer);
        }
        let started = Instant::now();
        let ms = self.target.draw(&self.gpu, &mut self.renderer, ())?;
        self.frames += 1;
        if !self.renderer.redrew_last() {
            self.reshown += 1;
        }
        self.gpu_ms += ms;
        if let Some(split) = self.renderer.gpu_split(&self.gpu) {
            for (acc, stage) in self.gpu_stages.iter_mut().zip(split) {
                *acc += stage;
            }
        }
        if self.figure_track.is_some()
            && self.renderer.redrew_last()
            && let Some(ms) = self.renderer.figure_gpu_ms(&self.gpu)
        {
            self.figure_ms += ms;
            self.figure_frames += 1;
        }
        self.present_ms += (Instant::now() - started).as_secs_f64() * 1e3;
        if let Some(dir) = &self.capture {
            let at = Instant::now();
            let p = self.renderer.params();
            let rgba = self.renderer.read_raster(&self.gpu)?;
            let path = dir.join(format!("voxel-gpu-{:06}.png", self.frames));
            cubarium_gpu::target::write_png(&path, p.raster_w, p.raster_h, &rgba)?;
            self.capture_ms += (Instant::now() - at).as_secs_f64() * 1e3;
        }
        self.feed_web()?;
        Ok(())
    }

    /// True once the target wants the run to stop (the development window was closed).
    pub fn should_quit(&mut self) -> bool {
        self.target.should_quit()
    }

    /// Whether Ctrl+S was pressed in the development window since the last call.
    pub fn take_save_request(&mut self) -> bool {
        self.target.take_save_request()
    }

    /// Report what the frames cost and stop.
    pub fn finish(&mut self) -> Result<()> {
        let n = self.frames.max(1) as f64;
        let t = self.ticks_staged.max(1) as f64;
        let p = self.renderer.params();
        eprintln!(
            "cubarium voxel --sink gpu: {} frames at {}x{} ({} px/voxel, depth step {}); \
             GPU {:.3} ms/frame (upload {:.3}, slab walk {:.3}, present {:.3}); \
             target draw {:.3} ms/frame; pack {:.3} ms/tick over {} ticks, \
             {:.0} KiB per tick; {} frames re-presented an unchanged raster; \
             roof from {}",
            self.frames,
            p.raster_w,
            p.raster_h,
            p.s,
            p.rise,
            self.gpu_ms / n,
            self.gpu_stages[0] / n,
            self.gpu_stages[1] / n,
            self.gpu_stages[2] / n,
            self.present_ms / n,
            self.pack_ms / t,
            self.ticks_staged,
            p.upload_bytes() as f64 / 1024.0,
            self.reshown,
            if p.roof_from_texture {
                "the uploaded table"
            } else {
                "a column walk in the shader"
            },
        );
        if self.figure_track.is_some() {
            let (ms, n) = self.figure_gpu();
            eprintln!(
                "cubarium voxel --sink gpu: model pass {ms:.3} GPU ms/frame over {n} frames; \
                 {} tiles had no room in the atlas",
                self.renderer.figures_dropped()
            );
        }
        if self.capture.is_some() {
            eprintln!(
                "cubarium voxel --sink gpu: captures cost {:.2} ms/frame (readback + PNG)",
                self.capture_ms / n
            );
        }
        if self.target.sample().is_some() {
            let run = self.opened.elapsed().as_secs_f64();
            let mut all = self.target.sample().expect("the target has a presenter");
            all.refused_packs = self.packs_skipped;
            eprintln!(
                "cubarium voxel --sink gpu: over {:.0} s the loop drew {} frames; {}",
                run,
                self.frames,
                all.line(run, &self.gpu.name),
            );
        }
        if let Some((n, ms, same, threads)) = self.sunvis_bakes() {
            eprintln!(
                "cubarium voxel --sink gpu: sun-visibility volume baked {n} times, \
                 {ms:.1} ms each on {threads} threads ({same} rebakes skipped as unchanged)"
            );
        }
        if self.packer.style_overflow > 0 {
            eprintln!(
                "cubarium voxel --sink gpu: {} plant voxels fell back to style 0 \
                 (more than {MAX_STYLES} distinct plant styles in one frame)",
                self.packer.style_overflow
            );
        }
        if let Some((web, _)) = self.web.as_mut() {
            eprintln!(
                "cubarium voxel --sink gpu: fed the viewer {} frames, {:.2} ms each",
                self.web_frames,
                self.web_ms / self.web_frames.max(1) as f64
            );
            web.finish()?;
        }
        self.target.finish(&self.gpu)
    }
}

impl<T: SinkTarget> Drop for VoxelGpuSink<T> {
    fn drop(&mut self) {
        self.target.destroy(&self.gpu);
        self.renderer.destroy(&self.gpu);
    }
}

/// Take `snap` into `mirror` and pack it.
fn stage_snapshot_with(stager: Stager<'_>, mirror: &mut Mirror, snap: &mut PackSnapshot) -> bool {
    mirror.apply(snap);
    let staged = {
        let (view, flora, fauna) = mirror.views(snap);
        stager.stage(&view, flora, fauna, mirror.hint(snap))
    };
    if staged {
        mirror.packed();
    }
    staged
}

/// Put the sky back where the palette had it, after the founding pulse.
fn restore_sky(founding_sky: &mut Option<([f32; 3], [f32; 3])>, renderer: &mut VoxelRenderer) -> Result<()> {
    if let Some((sky, horizon)) = founding_sky.take() {
        let mut params = renderer.params();
        params.sky = sky;
        params.sky_horizon = horizon;
        renderer.set_params(params)?;
    }
    Ok(())
}

/// What a pack writes of a [`VoxelGpuSink`]: every part but the target, borrowed, so the
/// pack can run on another thread ([`VoxelGpuSink::stage_snapshot_beside`]).
struct Stager<'a> {
    renderer: &'a mut VoxelRenderer,
    packer: &'a mut Packer,
    sunvis: &'a mut Option<SunVisDriver>,
    figure_track: &'a mut Option<FigureTrack>,
    tops: &'a mut (Option<u64>, Vec<u16>),
    founding_sky: &'a mut Option<([f32; 3], [f32; 3])>,
    awaiting_light: &'a mut bool,
    figure_tick: &'a mut Option<u64>,
    packs_skipped: &'a mut u64,
    ticks_staged: &'a mut u64,
    pack_ms: &'a mut f64,
    last_pack_ms: &'a mut f64,
    staged_tick: &'a mut Option<u64>,
    clock_s: f64,
    weather_preview: bool,
    force_rain: bool,
}

impl Stager<'_> {
    /// [`VoxelGpuSink::stage_world`]'s pack: see there.
    fn stage(self, view: &VoxelView<'_>, flora: FloraView<'_>, fauna: FaunaView<'_>, hint: Hint<'_>) -> bool {
        let started = Instant::now();
        let done = |pack_ms: &mut f64, last: &mut f64| {
            let ms = (Instant::now() - started).as_secs_f64() * 1e3;
            *pack_ms += ms;
            *last = ms;
        };
        if !self.renderer.can_stage() {
            *self.packs_skipped += 1;
            *self.last_pack_ms = 0.0;
            return false;
        }
        super::pack_timer::start();
        let p = self.renderer.params();
        // The candles' light at the frame clock's time (the tick's, before a clock is set).
        let candle_s = if self.clock_s > 0.0 {
            self.clock_s
        } else {
            view.tick as f64 / f64::from(cubarium_voxel::TICK_HZ)
        };
        let slow = self.packer.prepare(view, flora, fauna, candle_s);
        if let Some(sv) = self.sunvis.as_mut() {
            let packer = &*self.packer;
            let snapshot = || {
                let canopy = packer.light.as_ref().map(|l| &l.canopy);
                Snapshot::of(view, &packer.stands, &packer.animals, canopy)
            };
            sv.after_pack(self.clock_s, p.sun, snapshot, self.renderer);
        }
        if *self.awaiting_light {
            let pending = self.packer.light_pending() || self.sunvis.as_ref().is_some_and(|s| s.pending());
            if pending {
                done(self.pack_ms, self.last_pack_ms);
                return false;
            }
            *self.awaiting_light = false;
            if let Err(e) = restore_sky(self.founding_sky, self.renderer) {
                eprintln!("cubarium voxel: restoring the sky after founding: {e:#}");
            }
        }
        if let Some(track) = self.figure_track.as_mut() {
            let (first, cells) = self.packer.figure_models.pending();
            if !cells.is_empty() {
                if !self.renderer.put_figure_models(first, cells) {
                    eprintln!("cubarium voxel: the model pass is full; new animal models are not drawn");
                }
                self.packer.figure_models.uploaded();
            }
            track.push(view.tick, &self.packer.figure_specs, self.clock_s);
            // The flat tier moves its figures at the next tick's clock.
            *self.figure_tick = None;
        }
        let packer = &mut *self.packer;
        lap!("sunvis");
        self.renderer.stage(slow, |out| {
            packer.fill_with(view, p.width, p.height, p.depth, out, hint)
        });
        lap!("stage");
        let atmosphere = if p.sky_gradient && view.atmosphere_m3 > 0.0 {
            (view.atmosphere_m3 as f32 / 1.5).clamp(0.1, 1.0)
        } else {
            0.0
        };
        // A weather preview draws its own weather, not the world's shower.
        let raining = view.is_raining() && !self.weather_preview;
        let rain_tick = if p.sky_gradient && (raining || self.force_rain) {
            view.tick as f32
        } else {
            0.0
        };
        self.renderer.update_weather(atmosphere, rain_tick);
        if self.tops.0 != Some(view.terrain_version) {
            let (tops, floor) = super::weather::column_tops(view);
            *self.tops = (Some(view.terrain_version), tops);
            self.renderer.set_fog_floor(floor);
        }
        if !self.weather_preview {
            let mut w = view.weather;
            if self.force_rain && w.mode == cubarium_voxel::weather::RainMode::Clear {
                w.mode = cubarium_voxel::weather::RainMode::Shower;
                w.rain_m_per_s = cubarium_gpu::weather::R0;
            }
            let tops = &self.tops.1;
            self.renderer.set_weather(super::weather::weather_of(&w, |x, z| {
                super::weather::top_of(tops, p.width, x, z)
            }));
        }
        self.renderer.set_water_visible(self.packer.wet);
        *self.ticks_staged += 1;
        *self.staged_tick = Some(view.tick);
        done(self.pack_ms, self.last_pack_ms);
        true
    }
}

/// One tick's world as texels: the stage loop, and what it keeps between ticks.
///
/// **The full pack.** Every voxel once, in the world's own order, for material and water;
/// then only the cells a stand or an animal stamped, in texture order; then the vine's
/// tile cells. It is what a first tick, a terrain edit or another world packs, and what
/// the oracle packer ([`Packer::full_only`]) packs every tick.
///
/// **The change-only pack** (the live one). The packer keeps the texels it last packed
/// (`master`, texture order) and the style table as a persistent [`Palette`], so a texel
/// whose drawing did not change keeps its bytes and its slot. Each tick it gathers the
/// cells something may have changed in — free water (the world's wet set and the cells
/// that were wet), pore water (a diff over the porous runs), the stands' dirty cells and
/// their crown neighbours, the plant cells under canopy columns that moved, the animals'
/// cells before and after, the vine's cells when its key moved — composes each one
/// exactly as the full pack would, and marks the brick of every texel that changed. Only
/// owed bricks go into the staging buffer ([`cubarium_gpu::voxel::write_upload`]).
///
/// **What it keeps for both.** The roof table is a function of the terrain alone, so it is
/// built when the terrain moves and copied into a staging buffer only when that buffer
/// does not hold it yet (`cubarium_gpu::voxel::SlowPlanes`); the glyph atlas is a
/// function of the projection and is built once; the style table, the canopy plane and
/// the glow volume are slow planes keyed by their own versions.
struct Packer {
    /// The CPU presenter's own stand decomposition, rebuilt once per staged tick.
    stands: Stands,
    /// The frame's animals on the same grid, rebuilt per staged tick (round 5c).
    animals: Animals,
    /// The baked voxel models, or `None` for the dev-mode glyphs.
    models: Option<Arc<ModelLibrary>>,
    /// Animals drawn as models by the model pass (H2) rather than stamped here: the grid
    /// then holds carcasses and specimens alone, and the animals become `figure_specs`.
    figures: bool,
    figure_models: FigureModels,
    figure_specs: Vec<Spec>,
    /// This tick's emitting figure cells, for the glow volume.
    figure_emitters: Vec<((u32, u32, u32), [f32; 3])>,
    /// Unsimulated animals drawn from the models' `specimens` (`voxel_specimens` only).
    specimens: Vec<Specimen>,
    /// The GPU style table: one slot per style some texel draws with.
    palette: Palette,
    /// Which species face slots each model cell draws with ([`SpeciesFaces`]).
    species: SpeciesFaces,
    /// This tick's style key per `Stands` style index, and per `Animals` style index
    /// (stamped with the tick, `keys_tick`), so a style is resolved once a tick.
    plant_keys: Vec<(u64, StyleKey)>,
    animal_keys: Vec<(u64, StyleKey)>,
    keys_tick: u64,
    /// Texels that had to draw with slot 0 because the frame wanted more than
    /// [`MAX_STYLES`] distinct styles at once.
    style_overflow: u64,
    /// The roof table in texture order, for the terrain in [`Packer::roof_materials`].
    roof: Vec<u8>,
    /// The materials `roof` was built from, and their `terrain_version`. The version says
    /// when the terrain moved in one world, but two worlds can share a version (a fresh
    /// world starts at zero, as the empty founding world does), so the materials
    /// themselves are compared too, as bytes.
    roof_materials: Vec<Material>,
    roof_version: Option<u64>,
    /// Bumped every time `roof` is rebuilt: the key its plane is uploaded under.
    roof_key: u64,
    /// The whole glyph plane, atlas and zero tail, built once.
    glyphs: Vec<u8>,
    /// Scratch: the overlay's texture indices.
    overlay: Vec<u32>,
    /// This tick's latticevine cells (`crate::voxel::vine`), for the tile layer, and the
    /// tiled ones by texture index.
    vines: Vec<VineCell>,
    vine_at: rustc_hash::FxHashMap<u32, VineCell>,
    /// The vine key the tile cells were built under ([`crate::voxel::stand::vine_key`]),
    /// and whether this tick rebuilt them.
    vine_key: Option<u64>,
    vines_moved: bool,
    /// Whether the latticevine is the tile layer (textures on) rather than plain cells.
    vine_tiles: bool,
    /// Faces to draw instead of the flora's own cover: a fixture posing spur phases and
    /// dormancy by hand.
    cover_override: Option<Vec<FaceDraw>>,
    /// The lit tier's planes (`super::light`), or `None` in the flat tier, which packs
    /// neither.
    light: Option<LightPlanes>,
    /// Whether the last fill left any free water (the lit tier animates it).
    wet: bool,
    /// The glowcap candle, when it runs (lit tier, emission on, `candle_min` below 1).
    candle: Option<Candle>,
    /// This tick's candle curve for each flora stand (its index in `FloraView::stands`):
    /// a glowcap's ([`Candle::class_of`]), `0` for every other stand.
    candle_class: Vec<u8>,
    /// Whether a tick packs only what changed (the live pack) or everything (the oracle
    /// the change-only pack is held to).
    incremental: bool,
    /// The change-only pack's state.
    delta: Delta,
}

/// What the change-only pack keeps between ticks.
#[derive(Default)]
struct Delta {
    /// The texels last packed, texture order, and which of them draw with a style slot.
    master: Vec<VoxelTexel>,
    keyed: Vec<u64>,
    /// The next fill packs in full (first tick, terrain edit, another world).
    full: bool,
    /// The texels whose free water was not zero, texture order; the wet set's cells.
    wet: Vec<u32>,
    wet_cells: Vec<usize>,
    /// The porous runs of the terrain, `(world index, texture index, length)`, and the
    /// pore byte each of their cells was packed with.
    porous: Vec<(u32, u32, u32)>,
    pore: Vec<u8>,
    /// Where each porous run's bytes start in `pore`.
    porous_at: Vec<u32>,
    /// Last tick's animal cells, texture order.
    animals: Vec<u32>,
    /// This tick's cells to compose, and a bit per texel so each is listed once.
    dirty: Vec<u32>,
    dirty_bits: Vec<u64>,
    /// Columns (`z · width + x`) whose plant cells this tick's canopy moved.
    canopy_cols: Vec<u32>,
    /// Scratch for the two-phase palette update.
    changed: Vec<(u32, VoxelTexel, Option<StyleKey>)>,
    /// Texels that wanted a style when every slot was live, and draw with slot 0 (or a
    /// candle copy's steady style) meanwhile: composed again every tick a slot is free.
    overflowed: Vec<u32>,
}

impl Delta {
    #[inline]
    fn mark(&mut self, i: u32) {
        let (w, b) = ((i / 64) as usize, 1u64 << (i % 64));
        if self.dirty_bits[w] & b == 0 {
            self.dirty_bits[w] |= b;
            self.dirty.push(i);
        }
    }

    /// Compare porous run `j`'s cells `lo..hi` (world indices, inside the run) against the
    /// bytes they were packed with, and mark every texel whose byte moved.
    fn pore_run(&mut self, pore: &[f64], j: usize, lo: usize, hi: usize) {
        let (src, dst, _) = self.porous[j];
        let (src, dst) = (src as usize, dst as usize);
        let k = self.porous_at[j] as usize;
        let mut run = [0u8; 512];
        let mut at = lo;
        while at < hi {
            let n = (hi - at).min(run.len());
            for (b, &p) in run[..n].iter_mut().zip(&pore[at..at + n]) {
                *b = pore_byte(p);
            }
            let off = at - src;
            let held = &mut self.pore[k + off..k + off + n];
            if held != &run[..n] {
                for j in 0..n {
                    if held[j] != run[j] {
                        held[j] = run[j];
                        let i = (dst + off + j) as u32;
                        let (wd, bit) = ((i / 64) as usize, 1u64 << (i % 64));
                        if self.dirty_bits[wd] & bit == 0 {
                            self.dirty_bits[wd] |= bit;
                            self.dirty.push(i);
                        }
                    }
                }
            }
            at += n;
        }
    }

    #[inline]
    fn is_keyed(&self, i: usize) -> bool {
        self.keyed[i / 64] & (1 << (i % 64)) != 0
    }

    #[inline]
    fn set_keyed(&mut self, i: usize, on: bool) {
        let b = 1u64 << (i % 64);
        if on {
            self.keyed[i / 64] |= b;
        } else {
            self.keyed[i / 64] &= !b;
        }
    }
}

/// One texel as the shader resolves it: its material, part, glyph and water bytes, and
/// what its fourth byte means — a style slot's contents and candle curve, or a raw byte.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Resolved {
    Styled([u8; 3], VoxelStyle, u8),
    Raw([u8; 4]),
}

/// The lit tier's emitters ([`crate::voxel::LightConfig`]'s `emission`, `glow`,
/// `glow_reach`). Off in the flat tier whatever the config says.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Emission {
    on: bool,
    glow: f32,
    reach: u32,
    /// The glowcap candle (`[light] candle_min`, `candle_period_s`).
    candle: Candle,
}

impl Emission {
    #[cfg(test)]
    pub(crate) const OFF: Emission = Emission {
        on: false,
        glow: 0.0,
        reach: 0,
        candle: Candle {
            min: 1.0,
            period_s: 0.0,
        },
    };

    fn of(cfg: &VoxelConfig) -> Emission {
        Emission {
            on: cfg.lighting == Lighting::Lit && cfg.light.emission,
            glow: cfg.light.glow.max(0.0),
            reach: cfg.light.glow_reach,
            candle: Candle::of(&cfg.light),
        }
    }
}

/// What the packer keeps for the lit tier.
struct LightPlanes {
    /// The sky plane the images should hold, in texture order: open sky everywhere until
    /// the worker's first plane arrives.
    sky: Vec<u8>,
    /// Bumped whenever `sky` changes: the key its plane is uploaded under.
    sky_key: u64,
    /// The id of the newest terrain submitted to the worker; a plane under any other id is
    /// for a terrain that has since moved.
    submitted: u64,
    /// Whether the plane for the newest terrain has arrived.
    current: bool,
    worker: SkyWorker,
    canopy: Canopy,
    /// The canopy plane, built in `prepare` from the flora and copied in `fill` when its
    /// key moves.
    canopy_plane: Vec<u8>,
    canopy_key: u64,
    /// The full pack's glow: one volume of every emitter at its candle's brightness.
    glow: Glow,
    /// The change-only pack's glow: one grid per candle curve, mixed each tick.
    glow_set: GlowSet,
    emission: Emission,
    /// This tick's emitting voxels and their colours (the full pack: at the candle's
    /// brightness; the change-only pack: at full, with their curve).
    emitters: Vec<((u32, u32, u32), [f32; 3])>,
    /// The change-only pack's emitters by source, kept until that source moves.
    stand_emitters: Vec<((u32, u32, u32), u8, [f32; 3])>,
    stand_emitter_cells: rustc_hash::FxHashSet<u32>,
    animal_emitters: Vec<((u32, u32, u32), u8, [f32; 3])>,
    vine_emitters: Vec<((u32, u32, u32), u8, [f32; 3])>,
}

/// The glyph plane never changes after construction: one key for the life of a renderer.
const GLYPHS_KEY: u64 = 0;

/// What [`Packer::overlay`] makes of a cell: its texel (slot byte not yet set) and the
/// style it draws with, if any.
type Composed = (VoxelTexel, Option<StyleKey>);

impl Packer {
    /// `tiles`: the vine tile atlas is loaded, so the latticevine is drawn as the tile
    /// layer; otherwise it is plain voxel cells in the stands' grid.
    fn new(
        p: &VoxelParams,
        models: Option<Arc<ModelLibrary>>,
        tiles: bool,
        species: SpeciesFaces,
        emission: Emission,
    ) -> Packer {
        let mut glyphs = vec![0u8; p.glyph_bytes()];
        let used = appearance::atlas_len(p.s, p.rise);
        debug_assert!(used <= glyphs.len());
        debug_assert!(appearance::ATLAS_GLYPHS <= MAX_GLYPHS);
        appearance::write_atlas(p.s, p.rise, &mut glyphs[..used]);
        let mut stands = Stands::empty(p.width, p.height, p.depth);
        stands.set_vine_cells(!tiles);
        stands.set_incremental(true);
        if p.lit {
            // The lit tier divides a stand's own shade out of its own cells.
            stands.track_owners();
            stands.set_emission(emission.on);
        }
        let mut animals = Animals::empty(p.width, p.height, p.depth);
        animals.set_emission(p.lit && emission.on);
        let n = p.voxel_count();
        Packer {
            stands,
            vine_tiles: tiles,
            candle: (p.lit && emission.on && emission.candle.on()).then_some(emission.candle),
            candle_class: Vec::new(),
            incremental: true,
            animals,
            models,
            figures: false,
            figure_models: FigureModels::default(),
            figure_specs: Vec::new(),
            figure_emitters: Vec::new(),
            specimens: Vec::new(),
            palette: Palette::new(),
            species,
            plant_keys: Vec::new(),
            animal_keys: Vec::new(),
            keys_tick: 0,
            style_overflow: 0,
            roof: vec![0; n],
            roof_materials: Vec::new(),
            roof_version: None,
            roof_key: 0,
            glyphs,
            overlay: Vec::new(),
            vines: Vec::new(),
            vine_at: Default::default(),
            vine_key: None,
            vines_moved: false,
            cover_override: None,
            wet: false,
            light: p.lit.then(|| LightPlanes {
                sky: vec![255; n],
                sky_key: 0,
                submitted: 0,
                current: false,
                worker: SkyWorker::spawn(),
                canopy: Canopy::new(p.width, p.depth),
                canopy_plane: vec![0; p.canopy_bytes()],
                canopy_key: 0,
                glow: Glow::new(p.width, p.height, p.depth),
                glow_set: GlowSet::new(p.width, p.height, p.depth),
                emission,
                emitters: Vec::new(),
                stand_emitters: Vec::new(),
                stand_emitter_cells: Default::default(),
                animal_emitters: Vec::new(),
                vine_emitters: Vec::new(),
            }),
            delta: Delta {
                master: vec![VoxelTexel::default(); n],
                keyed: vec![0; n.div_ceil(64)],
                dirty_bits: vec![0; n.div_ceil(64)],
                full: true,
                ..Delta::default()
            },
        }
    }

    /// This packer packs every tick in full: the oracle.
    fn full_only(mut self) -> Packer {
        self.incremental = false;
        self.stands.set_incremental(false);
        self
    }

    /// Ease the film marks on this clock instead of the wall clock (`None`: the wall
    /// clock), so two packers fed the same ticks draw the same film.
    fn set_film_now(&mut self, now: Option<Instant>) {
        self.stands.set_film_now(now);
    }

    /// The packed texels, texture order: what the images hold once every owed brick is up.
    fn master(&self) -> &[VoxelTexel] {
        &self.delta.master
    }

    /// Texel `i` of [`Packer::master`] as the shader resolves it.
    fn resolved(&self, i: usize) -> Resolved {
        let t = self.delta.master[i];
        let bytes = [t.0[0], t.0[1], t.0[2]];
        match self.delta.is_keyed(i).then(|| self.palette.key(t.style())).flatten() {
            Some(k) => Resolved::Styled(bytes, palette::voxel_style(k), palette::curve_of(k)),
            None => Resolved::Raw(t.0),
        }
    }

    /// The style table and emission curves as the GPU should hold them.
    fn style_table(&self, styles: &mut [VoxelStyle], curves: &mut [u8; MAX_STYLES]) {
        self.palette.write(styles, curves);
    }

    /// Fill every free slot with a style of its own (tests: an overflow).
    #[cfg(test)]
    fn fill_palette(&mut self) -> Vec<u8> {
        let mut held = Vec::new();
        for n in 0.. {
            let c = [1e-3 * n as f32, 0.123, 0.456];
            let key = (crate::voxel::stand::Style { wood: c, crown: c, heart: c }, 0, [None; 2], None);
            match self.palette.acquire(&key) {
                Some(s) => held.push(s),
                None => break,
            }
        }
        held
    }

    /// Give back what [`Packer::fill_palette`] took.
    #[cfg(test)]
    fn empty_palette(&mut self, held: Vec<u8>) {
        for s in held {
            self.palette.release(s);
        }
    }

    /// Styles given a slot since the packer was made.
    fn styles_made(&self) -> u64 {
        self.palette.made()
    }

    /// The planes as this packer holds them: roof, glyphs, sky, canopy, glow (the last
    /// three empty in the flat tier).
    fn planes(&self) -> (&[u8], &[u8], &[u8], &[u8], &[u8]) {
        match &self.light {
            Some(l) => (
                &self.roof,
                &self.glyphs,
                &l.sky,
                &l.canopy_plane,
                if self.incremental { &l.glow_set.bytes } else { &l.glow.bytes },
            ),
            None => (&self.roof, &self.glyphs, &[], &[], &[]),
        }
    }

    /// Whether the lit tier's sky plane is still being computed for the terrain last
    /// packed. Always false in the flat tier.
    fn light_pending(&self) -> bool {
        self.light.as_ref().is_some_and(|l| !l.current)
    }

    /// Everything that is not a write into the staging buffer: the plant and animal
    /// grids, the roof if the terrain moved, and in the lit tier the canopy, the terrain
    /// handed to the sky's thread if it moved, the sky plane if one has arrived, and the
    /// glow. Returns the slow planes' keys. `candle_s` is the presenter's clock, seconds:
    /// the glowcaps' light is built at their candles' brightness then.
    fn prepare(
        &mut self,
        view: &VoxelView<'_>,
        flora: FloraView<'_>,
        fauna: FaunaView<'_>,
        candle_s: f64,
    ) -> [u64; SLOW_PLANES] {
        // The terrain first: a moved terrain (or another world) packs in full.
        let terrain_moved = self.roof_version != Some(view.terrain_version)
            || material_bytes(&self.roof_materials) != material_bytes(view.material);
        if terrain_moved {
            let c = view.config;
            roof_table(view.material, c.width, c.height, c.depth, &mut self.roof);
            self.roof_materials.clear();
            self.roof_materials.extend_from_slice(view.material);
            self.roof_version = Some(view.terrain_version);
            self.roof_key += 1;
            self.delta.full = true;
            self.stands.reset_incremental();
            if let Some(l) = &mut self.light {
                l.submitted += 1;
                l.current = false;
                l.worker.submit(l.submitted, view.config, view.material);
                l.glow.set_terrain(view.config, view.material);
                l.glow_set.set_terrain(view.config, view.material);
            }
        }
        lap!("terrain");
        match self.models.as_deref() {
            Some(lib) => {
                self.stands.rebuild_with(view, flora, lib);
                lap!("stands");
                if self.figures {
                    self.animals.rebuild_without_living(view, fauna);
                } else {
                    self.animals.rebuild_with(view, Some(fauna), lib);
                }
                self.animals.stamp_specimens(view, lib, &self.specimens);
            }
            None => {
                self.stands.rebuild(view, flora);
                if self.figures {
                    self.animals.rebuild_without_living(view, fauna);
                } else {
                    self.animals.rebuild(view, Some(fauna));
                }
            }
        }
        self.animals.stamp_carcasses(view, &flora, fauna.config);
        lap!("animals");
        if self.figures {
            figure::specs(&self.animals, view, fauna, self.models.as_deref(), &mut self.figure_models, &mut self.figure_specs);
            self.figure_emitters.clear();
            if self.light.as_ref().is_some_and(|l| l.emission.on) {
                let c = view.config;
                self.figure_emitters =
                    figure::emitters(&self.figure_models, &self.figure_specs, c.width, c.height, c.depth);
            }
            lap!("figures");
        }
        self.candle_class.clear();
        if let Some(c) = self.candle {
            self.candle_class.extend(flora.stands.iter().map(|s| {
                if s.species == cubarium_voxel_flora::Species::Glowcap {
                    c.class_of(s.id)
                } else {
                    0
                }
            }));
        }
        self.vines_moved = false;
        if self.vine_tiles {
            let root = |id| flora.cover.vine(id).map(|v| v.root);
            let key = match &self.cover_override {
                Some(d) => crate::voxel::stand::vine_key(d.iter().copied(), root),
                None => crate::voxel::stand::vine_key(flora.cover.face_draws(), root),
            };
            if !self.incremental || self.vine_key != Some(key) {
                let draws = match &self.cover_override {
                    Some(d) => d.clone(),
                    None => flora.cover.draw(),
                };
                self.vines = vine::cells(&draws, view.config, root);
                self.vine_key = Some(key);
                self.vines_moved = true;
            }
        }
        lap!("vines");
        let mut keys = [0; SLOW_PLANES];
        let incremental = self.incremental && !self.delta.full;
        if let Some(l) = &mut self.light {
            // The sky keeps the plane it has until the one for this terrain arrives: a
            // tick or two of the last terrain's sky after an edit, never a blank one.
            if let Some((id, plane)) = l.worker.poll()
                && id == l.submitted
            {
                l.sky = plane;
                l.sky_key += 1;
                l.current = true;
            }
            keys[PLANE_SKY] = l.sky_key;
            let stands = &self.stands;
            let crowns = |x, z| stands.crowns_in_column(x, z);
            self.delta.canopy_cols.clear();
            if self.incremental {
                let before = l.canopy_plane.clone();
                let moved = l.canopy.update(flora, &mut l.canopy_plane).to_vec();
                // Columns whose crown count may have moved: those under a dirty stand cell.
                let (w, h) = (view.config.width as usize, view.config.height as usize);
                let mut cols = moved;
                cols.extend(stands.dirty().iter().map(|&i| (i as usize / h) as u32));
                if !incremental {
                    cols.clear();
                    cols.extend(0..(w * view.config.depth as usize) as u32);
                }
                cols.sort_unstable();
                cols.dedup();
                l.canopy.recount(cols.iter().copied(), crowns);
                if l.canopy_plane != before {
                    l.canopy_key += 1;
                }
                self.delta.canopy_cols = cols;
            } else {
                l.canopy.build(flora, &mut l.canopy_plane);
                l.canopy.count_crowns(
                    stands
                        .cells()
                        .filter(|&(x, y, z)| {
                            matches!(stands.at(i64::from(x), i64::from(y), z), Part::Crown { .. })
                        })
                        .map(|(x, _, z)| (x, z)),
                );
                l.canopy_key += 1;
            }
            keys[PLANE_CANOPY] = l.canopy_key;
            lap!("canopy");
            if l.emission.on {
                // A glowcap's lip lights its cave at its candle's brightness this tick.
                let gains = candle_gains(self.candle, candle_s);
                if self.incremental {
                    keys[PLANE_GLOW] = Packer::glow_delta(
                        l,
                        stands,
                        &self.animals,
                        &self.figure_emitters,
                        &self.vines,
                        &self.candle_class,
                        incremental,
                        self.vines_moved,
                        &gains,
                    );
                } else {
                    // The emitters: every stamped cell whose style emits, and the tile
                    // layer's flowering vine cells. A cell an animal stands in still
                    // counts: the glow volume is coarse.
                    l.emitters.clear();
                    let class = &self.candle_class;
                    l.emitters.extend(stands.cells().filter_map(|(x, y, z)| {
                        let rgb = stands.emission(stands.at(i64::from(x), i64::from(y), z))?;
                        let c = stands
                            .owner(i64::from(x), y, z)
                            .and_then(|o| class.get(o).copied())
                            .unwrap_or(0);
                        Some(((x, y, z), rgb.map(|k| k * gains[usize::from(c)])))
                    }));
                    let animals = &self.animals;
                    l.emitters.extend(animals.cells().filter_map(|(x, y, z)| {
                        let rgb = animals.emission(animals.at(i64::from(x), i64::from(y), z))?;
                        Some(((x, y, z), rgb))
                    }));
                    l.emitters.extend_from_slice(&self.figure_emitters);
                    for c in self.vines.iter().filter(|c| c.kind.tiled()) {
                        if let Some(rgb) = colours::vine_emission(c.accent) {
                            l.emitters.push(((c.x, c.y, c.z), srgb_linear(rgb)));
                        }
                    }
                    l.glow.build(l.emitters.iter().copied(), l.emission.glow, l.emission.reach);
                    keys[PLANE_GLOW] = l.glow.key;
                }
            } else {
                keys[PLANE_GLOW] = if self.incremental { l.glow_set.key } else { l.glow.key };
            }
        }
        lap!("glow");
        keys[PLANE_ROOF] = self.roof_key;
        keys[PLANE_GLYPHS] = GLYPHS_KEY;
        keys[PLANE_STYLES] = 0;
        keys
    }

    /// The change-only pack's glow: the emitter lists rebuilt only from a source that
    /// moved (the stands' when any stand cell is dirty, the vine's when its cells were
    /// rebuilt, the animals' every tick, being few), the grids spread only where their
    /// emitters moved, and the curves mixed at this tick's gains. Returns the glow key.
    #[allow(clippy::too_many_arguments)]
    fn glow_delta(
        l: &mut LightPlanes,
        stands: &Stands,
        animals: &Animals,
        figures: &[((u32, u32, u32), [f32; 3])],
        vines: &[VineCell],
        class: &[u8],
        incremental: bool,
        vines_moved: bool,
        gains: &[f32],
    ) -> u64 {
        // A dirty stand cell moves the emitters only if it emits now or did.
        let moved = !incremental
            || stands.dirty().iter().any(|&g| {
                l.stand_emitter_cells.contains(&g) || stands.emission(stands.at_index(g)).is_some()
            });
        if moved {
            l.stand_emitters.clear();
            l.stand_emitter_cells.clear();
            for g in stands.stamped() {
                let Some(rgb) = stands.emission(stands.at_index(g)) else { continue };
                let (x, y, z) = stands.xyz(g);
                let c = stands
                    .owner(i64::from(x), y, z)
                    .and_then(|o| class.get(o).copied())
                    .unwrap_or(0);
                l.stand_emitters.push(((x, y, z), c, rgb));
                l.stand_emitter_cells.insert(g);
            }
        }
        if !incremental || vines_moved {
            l.vine_emitters.clear();
            for c in vines.iter().filter(|c| c.kind.tiled()) {
                if let Some(rgb) = colours::vine_emission(c.accent) {
                    l.vine_emitters.push(((c.x, c.y, c.z), 0, srgb_linear(rgb)));
                }
            }
        }
        let beasts: Vec<_> = animals
            .cells()
            .filter_map(|(x, y, z)| {
                let rgb = animals.emission(animals.at(i64::from(x), i64::from(y), z))?;
                Some(((x, y, z), 0u8, rgb))
            })
            .chain(figures.iter().map(|&(v, rgb)| (v, 0u8, rgb)))
            .collect();
        // The grids see the emitters again only when some source moved.
        if moved || !incremental || vines_moved || beasts != l.animal_emitters {
            l.animal_emitters = beasts;
            let all: Vec<_> = l
                .stand_emitters
                .iter()
                .chain(&l.animal_emitters)
                .chain(&l.vine_emitters)
                .copied()
                .collect();
            l.emitters.clear();
            l.emitters.extend(all.iter().map(|&(v, _, rgb)| (v, rgb)));
            l.glow_set.update(all.into_iter(), l.emission.glow, l.emission.reach);
        }
        lap!("glow emitters");
        l.glow_set.mix(gains);
        lap!("glow mix");
        l.glow_set.key
    }

    /// The style key a plant part draws with, resolved once a tick.
    fn plant_key(&mut self, part: Part) -> Option<StyleKey> {
        let index = usize::from(part.style()?);
        if let Some((tick, key)) = self.plant_keys.get(index)
            && *tick == self.keys_tick
        {
            return Some(*key);
        }
        let stands = &self.stands;
        let style = stands.style(part)?;
        let cell = stands.model_cell(part);
        let faces = cell.map_or([None; 2], |(sp, tag)| self.species.faces(sp, tag));
        let emit = stands.emission(part).map(|e| emit_key(e, stands.emits_whole(part)));
        let key = (style, role_of(cell.map(|(_, tag)| tag)), faces, emit);
        if self.plant_keys.len() <= index {
            self.plant_keys.resize(index + 1, (0, key));
        }
        self.plant_keys[index] = (self.keys_tick, key);
        Some(key)
    }

    /// The style key an animal part draws with, resolved once a tick.
    fn animal_key(&mut self, beast: AnimalPart) -> Option<StyleKey> {
        let index = usize::from(beast.style()?);
        if let Some((tick, key)) = self.animal_keys.get(index)
            && *tick == self.keys_tick
        {
            return Some(*key);
        }
        let style = self.animals.style(beast)?;
        let emit = self.animals.emission(beast);
        let key = (style, 0, [None; 2], emit.map(|e| emit_key(e, false)));
        if self.animal_keys.len() <= index {
            self.animal_keys.resize(index + 1, (0, key));
        }
        self.animal_keys[index] = (self.keys_tick, key);
        Some(key)
    }

    /// A plant or animal cell's texel over its material and water (texture index `i`), or
    /// `None` where nothing draws over them: the full pack's per-cell rule, shared by the
    /// change-only pack so the two cannot drift.
    fn overlay(&mut self, view: &VoxelView<'_>, i: usize, w: u32, h: u32) -> Option<Composed> {
        let (wu, hu) = (w as usize, h as usize);
        let (x, y, z) = ((i % wu) as u32, ((i / wu) % hu) as u32, (i / (wu * hu)) as u32);
        let du = view.config.depth as usize;
        let src = (y as usize * du + z as usize) * wu + x as usize;
        let m = view.material[src];
        if m.is_solid() {
            return None;
        }
        let xi = i64::from(x);
        let mats = &view.config.materials;
        let beasts = !self.animals.is_empty();
        // The animal after the plant in its own cell, as the CPU presenter stamps it: a
        // body standing in a turf covers the turf.
        let beast = if beasts {
            self.animals.at(xi, i64::from(y), z)
        } else {
            AnimalPart::None
        };
        if let Some(key) = self.animal_key(beast) {
            // An emitting part (lit tier, `colours::animal_emission`) draws the emissive
            // animal glyph in its emissive colour.
            let glyph = match key.3 {
                Some(_) => appearance::animal_emissive_glyph(beast.glyph()),
                None => beast.glyph(),
            };
            let mut t = texel_of(mats, m, view.free[src], view.pore[src], PART_ANIMAL_INTERIM, glyph.0, 0);
            // The sleep pose's class and entry (0: not posing; `voxel::sleep`).
            t.0[2] = self.animals.pose_at(xi, i64::from(y), z);
            return Some((t, Some(key)));
        }
        let p = self.stands.at(xi, i64::from(y), z);
        // Under a settling torso (WX4): a faceless leg cell, over whatever plant is there,
        // so the walk looks for the torso from here. Under a dipping head: a faceless torso
        // cell, where no plant is (`sleep::is_soft_mark`).
        let under = if beasts { self.animals.pose_at(xi, i64::from(y), z) } else { 0 };
        if under != 0 && (!sleep::is_soft_mark(under) || appearance::plant_class(p) == PART_NONE) {
            let mut t = texel_of(mats, m, view.free[src], view.pore[src], PART_ANIMAL_INTERIM, 0, 0);
            t.0[2] = under;
            return Some((t, None));
        }
        let class = appearance::plant_class(p);
        if class == PART_NONE {
            return None;
        }
        let stands = &self.stands;
        let mut glyph = appearance::plant_glyph(
            p,
            !stands.crown_continues(p, xi - 1, y, z),
            !stands.crown_continues(p, xi + 1, y, z),
        );
        // An emitting crown or heart cell draws the atlas's emissive variant (only the lit
        // tier with emission gives a style one).
        let emits = stands.emission(p).is_some();
        if matches!(p, Part::Crown { .. }) && emits {
            glyph = appearance::emissive_glyph(glyph);
        }
        // A glowcap's lip takes its candle curve's copy of the style.
        let curve = if emits && !self.candle_class.is_empty() {
            stands
                .owner(xi, y, z)
                .and_then(|o| self.candle_class.get(o).copied())
                .unwrap_or(0)
        } else {
            0
        };
        let owner = stands.owner(xi, y, z);
        let mut key = self.plant_key(p);
        if curve != 0
            && let Some(k) = &mut key
            && let Some(emit) = &mut k.3
        {
            emit[3] = (emit[3] & 1) | u32::from(curve) << 1;
        }
        let mut t = texel_of(mats, m, view.free[src], view.pore[src], class, glyph.0, 0);
        if let Some(l) = &self.light {
            let crown = matches!(p, Part::Crown { .. });
            t.0[2] = l.canopy.plant_byte(owner, crown, x, y, z);
        }
        // A part without a style draws with slot 0, as it always has.
        Some((t, key))
    }

    /// The vine's tile texel over `t`, if the vine has a cell there that nothing else
    /// claimed: an organism, a stand or a ground mark in the same voxel wins.
    #[inline]
    fn with_vine(t: VoxelTexel, c: &VineCell) -> Option<VoxelTexel> {
        // A head's faceless cell gives way to a vine as it does to a plant.
        let soft = t.part() == PART_ANIMAL_INTERIM && t.glyph() == 0 && sleep::is_soft_mark(t.0[2]);
        if t.material() == 0 && (t.part() == PART_NONE || soft) {
            let (glyph, b, a) = c.texel_bytes();
            return Some(t.with_vine(glyph, b, a));
        }
        None
    }

    /// The slot a composed texel (texture index `i`) draws with: its key's, given one if
    /// new; out of slots, a candle copy falls back to its steady style and anything else
    /// to slot 0, and the texel is listed to try again. Counts the texel in the palette.
    /// Returns the slot and whether it is keyed.
    fn acquire(&mut self, i: u32, key: Option<StyleKey>) -> (u8, bool) {
        let Some(key) = key else { return (0, false) };
        if let Some(s) = self.palette.acquire(&key) {
            return (s, true);
        }
        self.delta.overflowed.push(i);
        if let Some(emit) = key.3
            && emit[3] >> 1 != 0
        {
            let steady = (key.0, key.1, key.2, Some([emit[0], emit[1], emit[2], emit[3] & 1]));
            if let Some(s) = self.palette.acquire(&steady) {
                return (s, true);
            }
        }
        self.style_overflow += 1;
        (0, false)
    }

    /// Write the prepared tick into `out`.
    fn fill(&mut self, view: &VoxelView<'_>, w: u32, h: u32, d: u32, out: VoxelStaging<'_>) {
        self.fill_with(view, w, h, d, out, Hint::default());
    }

    /// [`Packer::fill`] of a snapshot's views ([`snapshot`]): `hint` names the wet set the
    /// view itself cannot give, and the only pore ranges that can have moved.
    fn fill_with(&mut self, view: &VoxelView<'_>, w: u32, h: u32, d: u32, out: VoxelStaging<'_>, hint: Hint<'_>) {
        self.keys_tick += 1;
        if self.incremental && !self.delta.full {
            self.fill_delta(view, w, h, d, hint);
            for &(i, _, _) in &self.delta.changed {
                let i = i as usize;
                let (x, y, z) = (i % w as usize, (i / w as usize) % h as usize, i / (w as usize * h as usize));
                out.owed.mark(cubarium_gpu::voxel::brick_of(w, h, x as u32, y as u32, z as u32));
            }
        } else {
            self.fill_full(view, w, h, d);
            out.owed.mark_full();
            lap!("full pack");
        }
        self.delta.full = false;
        // The pose table: the staged tick and its breaths, then this pack's poses.
        let entries = self.animals.pose_entries();
        if entries.len() > 1 {
            *out.poses = sleep::table(view.tick, FrameClock::TICK_WRAP, cubarium_voxel_fauna::DT);
            out.poses.entries[1..entries.len()].copy_from_slice(&entries[1..]);
        }
        // Every buffer carries the whole style table (16 KiB); the images take it when its
        // version moved.
        self.palette.write(out.styles, out.emit_curves);
        *out.styles_key = self.palette.version();
        lap!("tables");
        if out.write[PLANE_ROOF] {
            out.roof.copy_from_slice(&self.roof);
        }
        if out.write[PLANE_GLYPHS] {
            out.glyphs.copy_from_slice(&self.glyphs);
        }
        if let Some(l) = &self.light {
            if out.write[PLANE_SKY] {
                out.sky.copy_from_slice(&l.sky);
            }
            if out.write[PLANE_CANOPY] {
                out.canopy.copy_from_slice(&l.canopy_plane);
            }
            if out.write[PLANE_GLOW] {
                let glow = if self.incremental { &l.glow_set.bytes } else { &l.glow.bytes };
                out.glow.copy_from_slice(glow);
            }
        }
        cubarium_gpu::voxel::write_upload(&self.delta.master, w, h, d, out.owed, out.voxels, out.carried);
        lap!("upload");
    }

    /// The full pack into `master`, and everything the change-only pack keeps reset to it.
    fn fill_full(&mut self, view: &VoxelView<'_>, w: u32, h: u32, d: u32) {
        self.palette.reset();
        self.delta.overflowed.clear();
        let (wu, hu, du) = (w as usize, h as usize, d as usize);
        let mats = &view.config.materials;
        let master = &mut self.delta.master;
        // Material and water, every voxel, walking the world's `(y · depth + z) · width
        // + x` rows into the texture's `(z · height + y) · width + x` rows: both run x
        // fastest, so each row is one contiguous read and one contiguous write.
        for y in 0..hu {
            for z in 0..du {
                let src = (y * du + z) * wu;
                let dst = (z * hu + y) * wu;
                let material = &view.material[src..src + wu];
                let free = &view.free[src..src + wu];
                let pore = &view.pore[src..src + wu];
                for (x, texel) in master[dst..dst + wu].iter_mut().enumerate() {
                    *texel = texel_of(mats, material[x], free[x], pore[x], PART_NONE, 0, 0);
                }
            }
        }
        self.delta.keyed.fill(0);
        // Then the plants and animals, only where they stand, in texture order.
        self.overlay.clear();
        let index = |(x, y, z): (u32, u32, u32)| VoxelStaging::index(w, h, x, y, z) as u32;
        self.overlay.extend(self.stands.cells().map(index));
        self.overlay.extend(self.animals.cells().map(index));
        self.overlay.sort_unstable();
        self.overlay.dedup();
        let overlay = std::mem::take(&mut self.overlay);
        for &i in &overlay {
            let i = i as usize;
            if let Some((mut t, key)) = self.overlay(view, i, w, h) {
                let (slot, keyed) = self.acquire(i as u32, key);
                t.0[3] = slot;
                self.delta.master[i] = t;
                self.delta.set_keyed(i, keyed);
            }
        }
        self.overlay = overlay;
        // The vines last, into the air cells nothing else claimed.
        self.vine_at.clear();
        for c in self.vines.iter().filter(|c| c.kind.tiled()) {
            let i = VoxelStaging::index(w, h, c.x, c.y, c.z);
            self.vine_at.insert(i as u32, *c);
            if let Some(t) = Packer::with_vine(self.delta.master[i], c) {
                self.delta.master[i] = t;
            }
        }
        // What the change-only pack diffs against from here.
        let delta = &mut self.delta;
        delta.wet.clear();
        delta
            .wet
            .extend(delta.master.iter().enumerate().filter(|(_, t)| t.0[1] != 0).map(|(i, _)| i as u32));
        self.wet = !delta.wet.is_empty();
        delta.porous.clear();
        delta.porous_at.clear();
        delta.pore.clear();
        for y in 0..hu {
            for z in 0..du {
                let row = (y * du + z) * wu;
                let mut x = 0;
                while x < wu {
                    if mats.pore_capacity(view.material[row + x]) > 0.0 {
                        let start = x;
                        while x < wu && mats.pore_capacity(view.material[row + x]) > 0.0 {
                            x += 1;
                        }
                        let dst = (z * hu + y) * wu + start;
                        delta.porous.push(((row + start) as u32, dst as u32, (x - start) as u32));
                        delta.porous_at.push(delta.pore.len() as u32);
                        delta.pore.extend((start..x).map(|x| pore_byte(view.pore[row + x])));
                    } else {
                        x += 1;
                    }
                }
            }
        }
        delta.animals.clear();
        delta.animals.extend(self.animals.cells().map(index));
    }

    /// The change-only pack: compose the cells something may have changed in, and mark
    /// the brick of every texel that did.
    fn fill_delta(&mut self, view: &VoxelView<'_>, w: u32, h: u32, d: u32, hint: Hint<'_>) {
        let (wu, hu, du) = (w as usize, h as usize, d as usize);
        let mats = &view.config.materials;
        let tex = |src: usize| {
            let (x, rest) = (src % wu, src / wu);
            let (z, y) = (rest % du, rest / du);
            ((z * hu + y) * wu + x) as u32
        };
        let delta = &mut self.delta;
        delta.dirty.clear();
        // The texels that overflowed the style table, while it has room again.
        if !delta.overflowed.is_empty() && self.palette.live() < MAX_STYLES {
            for i in std::mem::take(&mut delta.overflowed) {
                delta.mark(i);
            }
        }
        // Free water: the cells that hold it now and the cells that held it.
        let wet = std::mem::take(&mut delta.wet);
        let mut cells = std::mem::take(&mut delta.wet_cells);
        let trusted = match hint.wet {
            Some(wet) => {
                cells.clear();
                cells.extend_from_slice(wet);
                true
            }
            None => view.wet_cells(&mut cells),
        };
        if trusted {
            for &src in &cells {
                let i = tex(src);
                if water_byte(view.free[src]) != delta.master[i as usize].0[1] {
                    delta.mark(i);
                }
            }
        } else {
            // No trusted wet set (a world loaded and not yet stepped): look at every cell.
            for src in 0..view.free.len() {
                let i = tex(src);
                if water_byte(view.free[src]) != delta.master[i as usize].0[1] {
                    delta.mark(i);
                }
                if view.free[src] > 0.0 {
                    cells.push(src);
                }
            }
        }
        for &i in &wet {
            let i = i as usize;
            let (x, rest) = (i % wu, i / wu);
            let (y, z) = (rest % hu, rest / hu);
            let src = (y * du + z) * wu + x;
            if water_byte(view.free[src]) != delta.master[i].0[1] {
                delta.mark(i as u32);
            }
        }
        delta.wet_cells = cells;
        lap!("free water");
        // Pore water: the porous runs against the bytes they were packed with, a run's bytes
        // worked out together and compared whole first. A snapshot's pack compares only the
        // ranges its sweep moved ([`snapshot`]).
        match hint.pore {
            None => {
                for j in 0..delta.porous.len() {
                    let (src, _, len) = delta.porous[j];
                    delta.pore_run(view.pore, j, src as usize, src as usize + len as usize);
                }
            }
            Some(ranges) => {
                for &(start, len) in ranges {
                    let end = start + len;
                    // The first run that ends past `start`: runs are in world order.
                    let first = delta.porous.partition_point(|&(src, _, l)| (src + l) as usize <= start);
                    for j in first..delta.porous.len() {
                        let (src, _, l) = delta.porous[j];
                        let (src, l) = (src as usize, l as usize);
                        if src >= end {
                            break;
                        }
                        delta.pore_run(view.pore, j, src.max(start), (src + l).min(end));
                    }
                }
            }
        }
        lap!("pore water");
        // The stands' dirty cells and their neighbours along x, whose silhouette glyph
        // reads them.
        for &g in self.stands.dirty() {
            let (x, y, z) = self.stands.xyz(g);
            for dx in [w - 1, 0, 1] {
                delta.mark(VoxelStaging::index(w, h, (x + dx) % w, y, z) as u32);
            }
        }
        // The plant cells of every column the canopy or the crown counts moved.
        let cols = std::mem::take(&mut delta.canopy_cols);
        for &col in &cols {
            let (x, z) = (col % w, col / w);
            for y in 0..h {
                if self.stands.at(i64::from(x), i64::from(y), z) != Part::None {
                    delta.mark(VoxelStaging::index(w, h, x, y, z) as u32);
                }
            }
        }
        delta.canopy_cols = cols;
        lap!("plant cells");
        // The animals: where they were and where they are.
        let index = |(x, y, z): (u32, u32, u32)| VoxelStaging::index(w, h, x, y, z) as u32;
        let before = std::mem::take(&mut delta.animals);
        for &i in &before {
            delta.mark(i);
        }
        let mut now = before;
        now.clear();
        now.extend(self.animals.cells().map(index));
        for &i in &now {
            delta.mark(i);
        }
        delta.animals = now;
        // The vine's tile cells, when they were rebuilt.
        if self.vines_moved {
            let old: Vec<u32> = self.vine_at.keys().copied().collect();
            for i in old {
                delta.mark(i);
            }
            self.vine_at.clear();
            for c in self.vines.iter().filter(|c| c.kind.tiled()) {
                let i = VoxelStaging::index(w, h, c.x, c.y, c.z) as u32;
                self.vine_at.insert(i, *c);
                delta.mark(i);
            }
        }

        lap!("animal+vine cells");
        // Compose each, and keep what changed.
        let dirty = std::mem::take(&mut self.delta.dirty);
        let mut changed = std::mem::take(&mut self.delta.changed);
        changed.clear();
        for &i in &dirty {
            let iu = i as usize;
            self.delta.dirty_bits[iu / 64] = 0;
            let (x, rest) = (iu % wu, iu / wu);
            let (y, z) = (rest % hu, rest / hu);
            let src = (y * du + z) * wu + x;
            let base = texel_of(mats, view.material[src], view.free[src], view.pore[src], PART_NONE, 0, 0);
            let (mut t, key) = self.overlay(view, iu, w, h).unwrap_or((base, None));
            if let Some(c) = self.vine_at.get(&i)
                && let Some(v) = Packer::with_vine(t, c)
            {
                t = v;
            }
            let held = self.delta.master[iu];
            let same = held.0[..3] == t.0[..3]
                && match (&key, self.delta.is_keyed(iu)) {
                    (Some(k), true) => {
                        self.palette.key(held.style()).is_some_and(|hk| palette::key_bits(hk) == palette::key_bits(k))
                    }
                    (None, false) => held.0[3] == t.0[3],
                    _ => false,
                };
            if !same {
                changed.push((i, t, key));
            }
        }
        lap!("compose");
        count!("#dirty", dirty.len());
        count!("#changed", changed.len());
        count!("#stand dirty", self.stands.dirty().len());
        count!("#canopy cols", self.delta.canopy_cols.len());
        // Free what the changed texels drew with before taking what they draw with now, so
        // a frame needs no more slots than the full pack of it would.
        for &(i, _, _) in &changed {
            let iu = i as usize;
            if self.delta.is_keyed(iu) {
                self.palette.release(self.delta.master[iu].style());
            }
        }
        for k in 0..changed.len() {
            let (i, mut t, key) = changed[k];
            let iu = i as usize;
            if key.is_some() {
                let (slot, keyed) = self.acquire(i, key);
                t.0[3] = slot;
                self.delta.set_keyed(iu, keyed);
            } else {
                self.delta.set_keyed(iu, false);
            }
            self.delta.master[iu] = t;
        }
        self.delta.dirty = dirty;
        self.delta.dirty.clear();
        self.delta.changed = changed;
        // A texel that overflowed twice while the table stayed full is listed once.
        if self.delta.overflowed.len() > 1 {
            self.delta.overflowed.sort_unstable();
            self.delta.overflowed.dedup();
        }
        lap!("palette");
        // The wet list for next tick: every texel with free water is in the wet set (or in
        // the full scan's list when the set was not trusted).
        let delta = &mut self.delta;
        let mut still_wet = wet;
        still_wet.clear();
        for &src in &delta.wet_cells {
            let i = tex(src);
            if delta.master[i as usize].0[1] != 0 {
                still_wet.push(i);
            }
        }
        delta.wet = still_wet;
        self.wet = !delta.wet.is_empty();
        lap!("wet list");
    }
}

/// The free-water byte a texel holding `free` packs ([`texel_of`]'s `g`).
#[inline]
fn water_byte(free: f64) -> u8 {
    let free = free as f32;
    VoxelTexel::pack_glyph(0, PART_NONE, 0, free, free <= cpu::WATER_EPSILON, 0.0, 0).0[1]
}

/// The pore-water byte a porous texel holding `pore` packs ([`texel_of`]'s `b`).
#[inline]
fn pore_byte(pore: f64) -> u8 {
    // `quantise(pore.clamp(0, 1) as f32)` without its branches, so a run vectorises: a NaN
    // clamps to NaN and casts to 0, as `quantise` answers it.
    let v = pore.clamp(0.0, 1.0) as f32;
    (v * 255.0 + 0.5) as u8
}

/// A slice of materials as its bytes, for a fast whole-terrain compare (`Material` is a
/// one-byte fieldless enum: `bytemuck::NoUninit`).
fn material_bytes(m: &[Material]) -> &[u8] {
    bytemuck::cast_slice(m)
}

/// One voxel's texel: its material and water, and whatever part stands in it. `mats` is
/// the world's own `config.materials`: a material it gives no pore space shows no pore
/// water.
#[inline]
fn texel_of(
    mats: &Materials,
    m: Material,
    free: f64,
    pore: f64,
    part: u8,
    glyph: u8,
    slot: u8,
) -> VoxelTexel {
    let free = free as f32;
    let pore = if mats.pore_capacity(m) > 0.0 {
        pore.clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    VoxelTexel::pack_glyph(
        m as u8,
        part,
        glyph,
        free,
        free <= cpu::WATER_EPSILON,
        pore,
        slot,
    )
}

/// `VoxelPresenter::build_roof` in the roof texture's own index order: voxels from each
/// one up to the nearest solid above it in its own column, `0` where the column is open
/// to the sky, clamped into a byte. Walked a whole row of columns at a time, top down, so
/// every read and write runs along x.
///
/// The clamp is lossless where it matters: `roof_shade` is `ROOF_LIGHT + (1 −
/// ROOF_LIGHT)(1 − e^−(gap−1)/4)`, which is within 1e-5 of full light by a gap of 50 and
/// so within a thousandth of an 8-bit code long before 255.
fn roof_table(material: &[Material], w: u32, h: u32, d: u32, out: &mut [u8]) {
    let (w, h, d) = (w as usize, h as usize, d as usize);
    let mut nearest: Vec<Option<usize>> = vec![None; w];
    for z in 0..d {
        nearest.fill(None);
        for y in (0..h).rev() {
            let src = (y * d + z) * w;
            let dst = (z * h + y) * w;
            for x in 0..w {
                out[dst + x] = nearest[x].map_or(0, |r| (r - y).min(255) as u8);
                if material[src + x].is_solid() {
                    nearest[x] = Some(y);
                }
            }
        }
    }
}

/// The gain of each candle curve at `seconds` (index 0 and every curve with the candle off
/// at 1): what an emitter on that curve is drawn and casts light times.
fn candle_gains(candle: Option<Candle>, seconds: f64) -> [f32; candle::CLASSES as usize + 1] {
    let mut gains = [1.0; candle::CLASSES as usize + 1];
    if let Some(c) = candle {
        for (k, g) in gains.iter_mut().enumerate().skip(1) {
            *g = c.class_brightness(k as u8, seconds);
        }
    }
    gains
}

/// An emissive colour and its whole-cell flag as a style key: the colour's bits, then 1
/// for a whole-cell emitter (`VoxelStyle::emit_whole`); a glowcap lip's candle curve
/// ([`candle_slot`]) rides above that bit.
fn emit_key(rgb: [f32; 3], whole: bool) -> [u32; 4] {
    [rgb[0].to_bits(), rgb[1].to_bits(), rgb[2].to_bits(), u32::from(whole)]
}

/// The texture role of a baked model cell (`cubarium_gpu::voxel::ROLE_*`); anything that
/// is not a model cell draws untextured.
fn role_of(tag: Option<Tag>) -> u8 {
    use cubarium_gpu::voxel::{ROLE_ACCENT, ROLE_BARK, ROLE_DRAPE, ROLE_LEAF, ROLE_NONE};
    match tag {
        None => ROLE_NONE,
        Some(Tag::Trunk) => ROLE_BARK,
        Some(Tag::Foliage(_) | Tag::Fallen(_)) => ROLE_LEAF,
        Some(Tag::Drape(_)) => ROLE_DRAPE,
        Some(Tag::Accent) => ROLE_ACCENT,
    }
}

/// The pack before the cheap one, kept as the oracle `Packer` is tested against.
/// `VoxelPresenter::build_roof`, written into the roof texture's own index order:
/// voxels from each one up to the nearest solid above it in its own column, `0` where
/// the column is open to the sky, clamped into a byte.
///
/// The clamp is lossless where it matters: `roof_shade` is `ROOF_LIGHT + (1 −
/// ROOF_LIGHT)(1 − e^−(gap−1)/4)`, which is within 1e-5 of full light by a gap of 50 and
/// so within a thousandth of an 8-bit code long before 255.
#[cfg(test)]
fn roof_into(view: &VoxelView<'_>, w: u32, h: u32, d: u32, out: &mut [u8]) {
    for z in 0..d {
        for x in 0..w {
            let mut nearest: Option<u32> = None;
            for y in (0..h).rev() {
                out[VoxelStaging::index(w, h, x, y, z)] =
                    nearest.map_or(0, |r| (r - y).min(255) as u8);
                if view.material_at(i64::from(x), y, z).is_solid() {
                    nearest = Some(y);
                }
            }
        }
    }
}

/// [`VoxelParams`] from the presenter's own projection and constants.
///
/// Every colour and every coefficient is read out of [`crate::voxel::present`]. That
/// module is the definition of the picture; this function is the only place the GPU path
/// learns any of it, which is what keeps the two renderers from drifting.
pub fn params_of(cfg: &VoxelConfig, proj: Projection, roof_from_texture: bool) -> VoxelParams {
    use crate::present::srgb_linear;
    VoxelParams {
        s: proj.s,
        rise: proj.rise,
        base: proj.base,
        width: proj.width,
        height: proj.height,
        depth: proj.depth,
        raster_w: u32::from(proj.raster_w),
        raster_h: u32::from(proj.raster_h),
        haze: cfg.haze,
        water_alpha: cfg.water_alpha,
        roof_from_texture,
        sky: cpu::sky(),
        sky_horizon: cpu::sky_horizon(),
        atmosphere: 0.0,
        rain_tick: 0.0,
        dither: cfg.dither,
        sky_gradient: cfg.sky_gradient,
        bedrock: srgb_linear(cpu::BEDROCK_SRGB),
        rock: srgb_linear(cpu::ROCK_SRGB),
        soil: srgb_linear(cpu::SOIL_SRGB),
        water_deep: srgb_linear(cpu::WATER_DEEP_SRGB),
        water_surface: srgb_linear(cpu::WATER_SURFACE_SRGB),
        light: srgb_linear(cpu::LIGHT_SRGB),
        haze_colour: srgb_linear(cpu::HAZE_SRGB),
        top_gain: cpu::TOP_GAIN,
        top_tint: cpu::TOP_TINT,
        top_back: cpu::TOP_BACK,
        rim: cpu::RIM,
        edge_dark: cpu::EDGE_DARK,
        top_edge: cpu::TOP_EDGE,
        riser_lean: cpu::RISER_LEAN,
        wet: cpu::WET,
        roof_light: cpu::ROOF_LIGHT,
        roof_falloff: cpu::ROOF_FALLOFF_VOXELS,
        skin_alpha_gain: cpu::SKIN_ALPHA_GAIN,
        water_top_alpha: cpu::WATER_TOP_ALPHA,
        plant_top_gain: cpu::PLANT_TOP_GAIN,
        plant_top_tint: cpu::PLANT_TOP_TINT,
        plant_rim: cpu::PLANT_RIM,
        crown_edge: cpu::CROWN_EDGE,
        crown_under: cpu::CROWN_UNDER,
        trunk_shade: cpu::TRUNK_SHADE,
        trunk_light_at: cpu::TRUNK_LIGHT_AT,
        lit: cfg.lighting == Lighting::Lit,
        ambient_gain: cfg.light.ambient_gain,
        ambient_floor: cfg.light.ambient_floor.clamp(0.0, 1.0),
        light_levels: cfg.light.levels,
        ao_strength: cfg.light.ao.clamp(0.0, 1.0),
        ambient_colour: ambient_colour(cpu::sky(), cfg.light.ambient_tint),
        sun: sun_direction(cfg.light.sun),
        sun_tint: cfg.light.sun_tint.clamp(0.0, 1.0),
        water_absorb: cfg.light.water_absorb.max(0.0),
        reflect_gain: cfg.light.water_reflect.max(0.0),
        ripple: cfg.light.water_ripple.max(0.0),
        reflect_cells: cfg.light.water_reflect_cells,
        foam: cfg.light.water_foam.max(0.0),
        glint: cfg.light.water_glint.max(0.0),
        rain_rings: cfg.light.water_rain_rings.max(0.0),
        highlight: cfg.light.water_highlight,
        // Nothing to bloom without emitters: the passes are skipped.
        bloom: if cfg.lighting == Lighting::Lit && cfg.light.emission {
            cfg.light.bloom.max(0.0)
        } else {
            0.0
        },
        bloom_radius: cfg.light.bloom_radius,
        bloom_style: match cfg.light.bloom_style {
            crate::voxel::BloomStyle::Smooth => cubarium_gpu::bloom::BloomStyle::Smooth,
            crate::voxel::BloomStyle::Blocky => cubarium_gpu::bloom::BloomStyle::Blocky,
        },
        debug_flow: false,
        effects: cubarium_gpu::sunvis::Effects {
            shadows: cfg.light.shadows,
            reflections: cfg.light.reflections,
            volumetric: cfg.light.volumetric,
            glow: cfg.light.emission,
            ao: cfg.light.ao > 0.0,
            density: cfg.light.volumetric_density,
            falloff: cfg.light.volumetric_falloff,
            glow_weight: cfg.light.volumetric_glow,
        },
        day_sky: match cfg.light.day_sky {
            crate::voxel::DaySky::Warm => cubarium_gpu::weather::DaySky::Warm,
            crate::voxel::DaySky::Mint => cubarium_gpu::weather::DaySky::Mint,
            crate::voxel::DaySky::Lilac => cubarium_gpu::weather::DaySky::Lilac,
        },
        star_bloom: cfg.light.star_bloom.max(0.0),
        day_sky_brightness: cfg.light.day_sky_brightness.clamp(0.0, 2.0),
    }
}

/// The `sun` knob as the unit direction toward the sun, or zero (no sun) when it does
/// not point above the horizon or is not a direction at all.
fn sun_direction(sun: [f32; 3]) -> [f32; 3] {
    let len = (sun[0] * sun[0] + sun[1] * sun[1] + sun[2] * sun[2]).sqrt();
    if !len.is_finite() || len <= 0.0 || sun[1] <= 0.0 {
        return [0.0; 3];
    }
    sun.map(|k| k / len)
}

/// The lit tier's ambient colour: white leaning `tint` of the way toward the sky's hue
/// (its colour over its brightest channel), rescaled to unit luminance so the tint moves
/// the hue and not the brightness. The palette's sky is the only colour it reads.
fn ambient_colour(sky: [f32; 3], tint: f32) -> [f32; 3] {
    let peak = sky[0].max(sky[1]).max(sky[2]);
    if peak.is_nan() || peak <= 0.0 {
        return [1.0; 3];
    }
    let t = tint.clamp(0.0, 1.0);
    let c = sky.map(|k| 1.0 + (k / peak - 1.0) * t);
    let luma = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    if luma > 0.0 { c.map(|k| k / luma) } else { [1.0; 3] }
}

/// The face textures at this projection's level, from `cfg.textures_dir`, said once on
/// stderr. Anything wrong with them draws the faces solid rather than refusing to run.
pub fn load_textures(
    cfg: &VoxelConfig,
    under: Option<&std::path::Path>,
    params: &VoxelParams,
) -> VoxelTextures {
    if !cfg.textures && !cfg.vine_tiles {
        return VoxelTextures::empty(params.s, params.rise);
    }
    let dir = &cfg.textures_dir;
    if !dir.exists() {
        eprintln!(
            "cubarium voxel: no face textures at {}; drawing solid faces",
            dir.display()
        );
        return VoxelTextures::empty(params.s, params.rise);
    }
    let roots: Vec<&std::path::Path> = std::iter::once(dir.as_path()).chain(under).collect();
    if !cfg.textures {
        // `vine_tiles` alone: the vine's tile layer, every face solid.
        return match crate::voxel::textures::load_vine_layer(&roots, params.s, params.rise) {
            Ok((atlas, p)) => {
                eprintln!(
                    "cubarium voxel: vine tiles at {} px from {}: {} ({} missing); faces solid",
                    params.s,
                    dir.display(),
                    p.vine_tiles,
                    p.vine_missing,
                );
                atlas
            }
            Err(e) => {
                eprintln!(
                    "cubarium voxel: the vine tiles in {} did not load ({e:#}); drawing plain vine cells",
                    dir.display()
                );
                VoxelTextures::empty(params.s, params.rise)
            }
        };
    }
    match crate::voxel::textures::load_layers(&roots, params.s, params.rise) {
        Ok((atlas, p)) => {
            let slots = atlas.mask().count_ones();
            eprintln!(
                "cubarium voxel: face textures at {} px from {}: {slots} of {} faces \
                 ({} override, {} level, {} derived here, {} repeated variants); \
                 {} species faces; {} vine tiles ({} missing)",
                params.s,
                roots
                    .iter()
                    .map(|r| r.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" over "),
                cubarium_gpu::voxel::TEXTURE_SLOTS.len(),
                p.overrides,
                p.levels,
                p.derived,
                p.repeated,
                p.species_faces,
                p.vine_tiles,
                p.vine_missing,
            );
            atlas
        }
        Err(e) => {
            eprintln!(
                "cubarium voxel: the face textures in {} did not load ({e:#}); drawing solid faces",
                dir.display()
            );
            VoxelTextures::empty(params.s, params.rise)
        }
    }
}

/// Refuse a world the renderer cannot hold, before the device is opened.
pub fn check(proj: Projection) -> Result<()> {
    if proj.cropped {
        // Not a refusal in the CPU presenter either — it says so and carries on — but a
        // cropped strip on the GPU costs the same as an uncropped one and hides rows the
        // operator asked for, so it is worth saying twice.
        eprintln!(
            "cubarium voxel: --sink gpu is drawing a cropped strip ({} rows of {})",
            proj.raster_h,
            proj.full_height()
        );
    }
    if u32::from(proj.raster_w) != proj.width * proj.s {
        bail!("the projection's raster is not the strip's own width");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! The cheap pack against the one it replaced. The bar is equal texels in every plane
    //! the GPU reads, through the two alternating staging buffers and the slow planes'
    //! keys, across the edits that move what a pack keeps between ticks.

    use super::oracle::{Frame, PackOracle};
    use super::*;
    use crate::voxel::stand::Style;
    use cubarium_gpu::voxel::{BrickOwed, Carried, SlowPlanes, apply_upload};
    use cubarium_voxel::{Command, Config};
    use cubarium_voxel_fauna::{Command as FaunaCommand, FaunaConfig};
    use cubarium_voxel_flora::{Command as FloraCommand, FloraConfig, Species};

    // The old pack's per-tick style table, for the reference pack.
    type StyleKey = (Style, u8, [Option<u16>; 2], Option<[u32; 4]>);

    /// The GPU slot for a style that has no plant part index to cache under (an animal's),
    /// deduplicated by value against the same table the plants fill.
    fn slot_for_style(style: StyleKey, styles: &mut Vec<StyleKey>, overflow: &mut u64) -> u8 {
        match styles.iter().position(|s| *s == style) {
            Some(at) => at as u8,
            None if styles.len() < MAX_STYLES => {
                styles.push(style);
                (styles.len() - 1) as u8
            }
            None => {
                *overflow += 1;
                0
            }
        }
    }

    /// The GPU slot for one plant part's style, deduplicated by value.
    fn slot_for(
        stands: &Stands,
        species: &SpeciesFaces,
        part: Part,
        styles: &mut Vec<StyleKey>,
        slot_of: &mut Vec<Option<u8>>,
        overflow: &mut u64,
    ) -> u8 {
        let Some(index) = part.style() else { return 0 };
        let index = usize::from(index);
        if slot_of.len() <= index {
            slot_of.resize(index + 1, None);
        }
        if let Some(slot) = slot_of[index] {
            return slot;
        }
        let Some(style) = stands.style(part) else {
            return 0;
        };
        let cell = stands.model_cell(part);
        let faces = cell.map_or([None; 2], |(sp, tag)| species.faces(sp, tag));
        let emit = stands.emission(part).map(|e| emit_key(e, stands.emits_whole(part)));
        let style = (style, role_of(cell.map(|(_, tag)| tag)), faces, emit);
        let slot = match styles.iter().position(|s| *s == style) {
            Some(at) => at as u8,
            None if styles.len() < MAX_STYLES => {
                styles.push(style);
                (styles.len() - 1) as u8
            }
            None => {
                *overflow += 1;
                0
            }
        };
        slot_of[index] = Some(slot);
        slot
    }


    /// The pack as it was before the cheap one: every voxel through the wrapping
    /// accessors, fresh plant and animal grids, the roof walked per column and the atlas
    /// rebuilt, every plane written.
    fn reference(
        view: &VoxelView<'_>,
        flora: &Flora,
        fauna: &Fauna,
        p: &VoxelParams,
        out: VoxelStaging<'_>,
    ) {
        let (w, h, d) = (p.width, p.height, p.depth);
        let mut stands = Stands::empty(w, h, d);
        let mut animals = Animals::empty(w, h, d);
        stands.rebuild(view, flora.view());
        animals.rebuild(view, Some(fauna.view()));
        let (mut styles, mut slot_of, mut overflow) = (Vec::new(), Vec::new(), 0u64);
        let (stands, animals) = (&stands, &animals);
        let beasts = !animals.is_empty();
        let (styles, slot_of, overflow) = (&mut styles, &mut slot_of, &mut overflow);
        roof_into(view, w, h, d, out.roof);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let xi = i64::from(x);
                    let m = view.material_at(xi, y, z);
                    let beast = if beasts && !m.is_solid() {
                        animals.at(xi, i64::from(y), z)
                    } else {
                        AnimalPart::None
                    };
                    let (part, glyph, slot) = if m.is_solid() {
                        (PART_NONE, 0, 0)
                    } else if let Some(style) = animals.style(beast) {
                        (
                            PART_ANIMAL_INTERIM,
                            beast.glyph().0,
                            slot_for_style((style, 0, [None; 2], None), styles, overflow),
                        )
                    } else {
                        let p = stands.at(xi, i64::from(y), z);
                        let under = animals.pose_at(xi, i64::from(y), z);
                        match appearance::plant_class(p) {
                            _ if under != 0
                                && (!sleep::is_soft_mark(under)
                                    || appearance::plant_class(p) == PART_NONE) =>
                            {
                                (PART_ANIMAL_INTERIM, 0, 0)
                            }
                            PART_NONE => (PART_NONE, 0, 0),
                            class => {
                                let s = slot_for(
                                    stands,
                                    &SpeciesFaces::default(),
                                    p,
                                    styles,
                                    slot_of,
                                    overflow,
                                );
                                let glyph = appearance::plant_glyph(
                                    p,
                                    !stands.crown_continues(p, xi - 1, y, z),
                                    !stands.crown_continues(p, xi + 1, y, z),
                                );
                                (class, glyph.0, s)
                            }
                        }
                    };
                    let free = view.free_at(xi, y, z) as f32;
                    let pore = if view.config.materials.pore_capacity(m) > 0.0 {
                        view.pore_at(xi, y, z).clamp(0.0, 1.0) as f32
                    } else {
                        0.0
                    };
                    let mut t = VoxelTexel::pack_glyph(
                        m as u8,
                        part,
                        glyph,
                        free,
                        free <= cpu::WATER_EPSILON,
                        pore,
                        slot,
                    );
                    if part == PART_ANIMAL_INTERIM {
                        t.0[2] = animals.pose_at(xi, i64::from(y), z);
                    }
                    out.voxels[VoxelStaging::index(w, h, x, y, z)] = t;
                }
            }
        }
        for (slot, (style, role, faces, _)) in styles.iter().enumerate() {
            out.styles[slot] = VoxelStyle::new(style.wood, style.crown, style.heart)
                .with_role(*role)
                .with_faces(*faces);
        }
        out.glyphs.fill(0);
        let used = appearance::atlas_len(p.s, p.rise);
        appearance::write_atlas(p.s, p.rise, &mut out.glyphs[..used]);
    }

    /// One staging buffer's planes, or the images'.
    #[derive(Clone, PartialEq)]
    struct Planes {
        voxels: Vec<VoxelTexel>,
        roof: Vec<u8>,
        styles: Vec<VoxelStyle>,
        glyphs: Vec<u8>,
        poses: cubarium_gpu::voxel::PoseTable,
        emit_curves: [u8; MAX_STYLES],
    }

    impl Planes {
        /// Filled with junk, so a plane a pack should have written and did not shows.
        fn junk(p: &VoxelParams) -> Planes {
            Planes {
                voxels: vec![VoxelTexel([0xAB; 4]); p.voxel_count()],
                roof: vec![0xAB; p.voxel_count()],
                styles: vec![VoxelStyle::new([0.5; 3], [0.5; 3], [0.5; 3]); MAX_STYLES],
                glyphs: vec![0xAB; p.glyph_bytes()],
                poses: cubarium_gpu::voxel::PoseTable::EMPTY,
                emit_curves: [0; MAX_STYLES],
            }
        }

        /// As `VoxelRenderer::stage` hands it over. The flat tier's pack has no sky,
        /// canopy or glow plane.
        fn staging<'a>(
            &'a mut self,
            write: [bool; SLOW_PLANES],
            owed: &'a mut BrickOwed,
            carried: &'a mut Carried,
            styles_key: &'a mut u64,
        ) -> VoxelStaging<'a> {
            *carried = Carried::default();
            VoxelStaging {
                voxels: &mut self.voxels,
                roof: &mut self.roof,
                styles: &mut self.styles,
                glyphs: &mut self.glyphs,
                sky: &mut [],
                canopy: &mut [],
                glow: &mut [],
                poses: &mut self.poses,
                emit_curves: &mut self.emit_curves,
                write,
                owed,
                carried,
                styles_key,
            }
        }
    }

    /// The renderer's side of a pack and an upload, without a device: two buffers
    /// alternating, the owed bricks, and the images holding whatever was copied.
    struct Rig {
        params: VoxelParams,
        packer: Packer,
        slow: SlowPlanes,
        owed: BrickOwed,
        carried: [Carried; 2],
        buffers: [Planes; 2],
        images: Planes,
        ticks: usize,
    }

    impl Rig {
        fn new(c: &Config) -> Rig {
            let cfg = VoxelConfig {
                sim: cubarium_session::SimParams { world: c.clone(), ..Default::default() },
                ..VoxelConfig::default()
            };
            let proj =
                Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, c).unwrap();
            let params = params_of(&cfg, proj, true);
            Rig {
                packer: Packer::new(
                    &params,
                    None,
                    false,
                    SpeciesFaces::default(),
                    Emission::OFF,
                ),
                slow: SlowPlanes::new(2),
                owed: BrickOwed::new(c.width, c.height, c.depth),
                carried: Default::default(),
                buffers: [Planes::junk(&params), Planes::junk(&params)],
                images: Planes::junk(&params),
                params,
                ticks: 0,
            }
        }

        /// Pack and upload one tick, and hold the images to the reference pack, each
        /// texel with its style resolved through its own table.
        fn tick(&mut self, world: &World, flora: &Flora, fauna: &Fauna, what: &str) {
            let p = self.params;
            let view = world.view();
            let b = self.ticks % 2;
            self.ticks += 1;
            let keys = self.packer.prepare(&view, flora.view(), fauna.view(), 0.0);
            let write = self.slow.pack(b, keys);
            let mut styles_key = 0;
            self.packer.fill(
                &view,
                p.width,
                p.height,
                p.depth,
                self.buffers[b].staging(write, &mut self.owed, &mut self.carried[b], &mut styles_key),
            );
            self.slow.restamp(b, PLANE_STYLES, styles_key);
            let copy = self.slow.upload(b);
            self.owed.taken(&self.carried[b]);
            let buffer = &self.buffers[b];
            apply_upload(
                &buffer.voxels,
                &self.carried[b],
                p.width,
                p.height,
                p.depth,
                &mut self.images.voxels,
            );
            if copy[PLANE_STYLES] {
                self.images.styles.clone_from(&buffer.styles);
            }
            if copy[PLANE_ROOF] {
                self.images.roof.clone_from(&buffer.roof);
            }
            if copy[PLANE_GLYPHS] {
                self.images.glyphs.clone_from(&buffer.glyphs);
            }

            let mut want = Planes::junk(&p);
            let (mut owed, mut carried, mut key) = (BrickOwed::new(p.width, p.height, p.depth), Carried::default(), 0);
            reference(&view, flora, fauna, &p, want.staging([true; SLOW_PLANES], &mut owed, &mut carried, &mut key));
            // The style a texel draws with, when it draws with one: a faceless animal cell
            // (glyph 0) and a plain voxel read none.
            let style = |t: VoxelTexel, styles: &[VoxelStyle]| {
                let styled = t.part() != PART_NONE && !(t.part() == PART_ANIMAL_INTERIM && t.glyph() == 0);
                styled.then(|| styles[usize::from(t.style())])
            };
            for (i, (got, want_t)) in self.images.voxels.iter().zip(&want.voxels).enumerate() {
                assert!(
                    got.0[..3] == want_t.0[..3]
                        && style(*got, &self.images.styles) == style(*want_t, &want.styles),
                    "{what}: voxel texel {i} is {got:?}, the old pack wrote {want_t:?}"
                );
            }
            assert!(
                self.images.roof == want.roof,
                "{what}: the roof table differs"
            );
            assert!(
                self.images.glyphs == want.glyphs,
                "{what}: the glyph atlas differs"
            );
        }
    }

    fn config() -> Config {
        Config {
            width: 32,
            height: 14,
            depth: 5,
            ..Config::default()
        }
    }

    fn set(world: &mut World, x: i64, y: u32, z: u32, material: Material) {
        world.apply(Command::SetMaterial { x, y, z, material });
    }

    /// Terrain, a roof, standing water, pore water, two stands (one across the seam) and
    /// a grazer — the small world `tests/voxel_gpu.rs` draws on both renderers.
    fn world(c: &Config) -> World {
        let mut world = World::empty(c.clone());
        let v = c.voxel_volume();
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                set(&mut world, x, 0, z, Material::Bedrock);
                set(&mut world, x, 1, z, Material::Rock);
                set(&mut world, x, 2, z, Material::Soil);
                if (8..20).contains(&x) {
                    for y in 3..=3 + z {
                        set(&mut world, x, y, z, Material::Soil);
                    }
                }
            }
        }
        for x in 22..28i64 {
            set(&mut world, x, 9, 0, Material::Rock);
            set(&mut world, x, 9, 1, Material::Rock);
        }
        for z in 0..c.depth {
            for x in -2..6i64 {
                for y in 3..5u32 {
                    world.apply(Command::AddWater {
                        x,
                        y,
                        z,
                        volume_m3: v,
                    });
                }
            }
        }
        world.apply(Command::AddWater {
            x: 24,
            y: 8,
            z: 1,
            volume_m3: v * 0.25,
        });
        for x in 8..20i64 {
            world.apply(Command::AddWater {
                x,
                y: 2,
                z: 2,
                volume_m3: v * 0.2,
            });
        }
        world
    }

    fn flora(world: &World, stands: &[(i64, u32, Species, f64)]) -> Flora {
        let mut flora = Flora::new(FloraConfig::default());
        for &(x, z, species, of_max) in stands {
            let wood = flora.config().species(species).wood_max * of_max;
            assert!(
                flora.apply(
                    world,
                    FloraCommand::Seed {
                        x,
                        z,
                        species,
                        wood
                    }
                ),
                "a support face for the stand at ({x}, {z})"
            );
        }
        flora
    }

    fn grazer(world: &World, x: i64, z: u32) -> Fauna {
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(
            fauna.apply(
                world,
                FaunaCommand::IntroduceFounder {
                    x,
                    z,
                    founder: cubarium_voxel_fauna::Founder::Browser,
                    stores: cubarium_voxel_fauna::StartingStores::FULL,
                    heading_rad: 0.0,
                }
            ),
            "a support face for the grazer at ({x}, {z})"
        );
        fauna
    }

    /// The same packer, tick after tick, against the old pack run fresh each time: a
    /// terrain edit, a moved animal, a stand that grows and one that dies all have to
    /// leave nothing of the tick before behind.
    #[test]
    fn the_cheap_pack_writes_the_texels_the_old_pack_wrote() {
        let c = config();
        let mut rig = Rig::new(&c);
        let mut world = world(&c);
        let stands = [
            (14i64, 2u32, Species::Bloomcrown, 1.0),
            (1, 1, Species::Umbrellafrond, 1.0),
            (26, 3, Species::Umbrellafrond, 0.4),
        ];
        let mut plants = flora(&world, &stands);
        let mut beast = grazer(&world, 4, 1);
        rig.tick(&world, &plants, &beast, "the fixture");
        rig.tick(
            &world,
            &plants,
            &beast,
            "the fixture again, into the other buffer",
        );
        rig.tick(&world, &plants, &beast, "the fixture a third time");

        // A terrain edit: a shelf over the pool, which moves the roof under it.
        for x in 0..4 {
            set(&mut world, x, 11, 2, Material::Rock);
        }
        rig.tick(&world, &plants, &beast, "after a shelf is added");
        rig.tick(&world, &plants, &beast, "after a shelf, the other buffer");
        set(&mut world, 24, 9, 0, Material::Air);
        rig.tick(&world, &plants, &beast, "after a roof cell is dug out");

        beast = grazer(&world, 29, 3);
        rig.tick(&world, &plants, &beast, "after the grazer moves");
        beast = grazer(&world, 31, 0);
        rig.tick(
            &world,
            &plants,
            &beast,
            "after the grazer moves onto the seam",
        );

        let mut grown = stands;
        grown[2].3 = 1.0;
        plants = flora(&world, &grown);
        rig.tick(&world, &plants, &beast, "after a stand grows");
        assert!(plants.apply(&world, FloraCommand::Clear { x: 1, z: 1 }));
        rig.tick(&world, &plants, &beast, "after the stand on the seam dies");
        let none = Fauna::new(FaunaConfig::default());
        rig.tick(&world, &plants, &none, "after the grazer is gone");
        rig.tick(
            &world,
            &Flora::new(FloraConfig::default()),
            &none,
            "with nothing living",
        );
    }

    /// Two worlds can share a terrain version — the empty founding world and the first
    /// real one both start at zero — so the roof cache cannot trust the version alone.
    #[test]
    fn a_different_world_at_the_same_terrain_version_gets_its_own_roof() {
        let c = config();
        let mut rig = Rig::new(&c);
        let (mut a, mut b) = (World::empty(c.clone()), World::empty(c.clone()));
        set(&mut a, 3, 8, 1, Material::Rock);
        set(&mut b, 9, 5, 2, Material::Rock);
        assert_eq!(a.terrain_version(), b.terrain_version());
        let (flora, fauna) = (
            Flora::new(FloraConfig::default()),
            Fauna::new(FaunaConfig::default()),
        );
        rig.tick(&a, &flora, &fauna, "world a");
        rig.tick(&b, &flora, &fauna, "world b at a's terrain version");
        rig.tick(&b, &flora, &fauna, "world b, the other buffer");
    }

    /// `vine_tiles` is off by default. With `textures` off and `vine_tiles` on, only the
    /// vine's tiles load: the vine draws as the tile layer (the packer leaves it out of the
    /// stands' cells) while no face slot is filled, so terrain draws as the plain path.
    #[test]
    fn vine_tiles_load_the_vine_alone() {
        use crate::voxel::textures::{Image, write_png};
        assert!(!VoxelConfig::default().vine_tiles);
        let root = std::env::temp_dir().join(format!("cubarium-vine-only-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let solid = |c: [u8; 4]| Image {
            w: 48,
            h: 48,
            rgba: c.repeat(48 * 48),
        };
        write_png(&root.join("masters/rock-side-0.png"), &solid([100, 100, 100, 255])).unwrap();
        write_png(&root.join("masters/vine/vine-full-5.png"), &solid([0, 200, 0, 255])).unwrap();
        let c = Config::default();
        let cfg = VoxelConfig {
            sim: cubarium_session::SimParams { world: c.clone(), ..Default::default() },
            textures_dir: root.clone(),
            vine_tiles: true,
            ..VoxelConfig::default()
        };
        let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &c).unwrap();
        let params = params_of(&cfg, proj, true);
        let atlas = load_textures(&cfg, None, &params);
        assert!(atlas.vine_on, "the vine is the tile layer");
        assert_eq!(atlas.mask(), 0, "no face texture: terrain draws plain");
        let packer = Packer::new(&params, None, atlas.vine_on, SpeciesFaces::of(&atlas), Emission::OFF);
        assert!(packer.vine_tiles);
        // Both off: nothing loads; textures on: the faces load as before.
        let plain = load_textures(&VoxelConfig { vine_tiles: false, ..cfg.clone() }, None, &params);
        assert!(!plain.vine_on && plain.mask() == 0);
        let full = load_textures(&VoxelConfig { textures: true, ..cfg.clone() }, None, &params);
        assert!(full.vine_on && full.mask() != 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    // ---- the change-only pack against the full pack ----

    /// An oracle on the fixture world: the flat tier with the stands' plain vine cells, or
    /// the lit tier with the vine's tile layer, emission and a running candle.
    fn oracle(c: &Config, lit: bool) -> PackOracle {
        oracle_with(c, lit, None)
    }

    /// The repository's baked plant and animal models.
    fn models(c: &Config) -> Arc<ModelLibrary> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/voxel-models");
        Arc::new(ModelLibrary::load(&root, c.voxel_m).expect("the baked models"))
    }

    fn oracle_with(c: &Config, lit: bool, models: Option<Arc<ModelLibrary>>) -> PackOracle {
        let mut cfg = VoxelConfig {
            sim: cubarium_session::SimParams { world: c.clone(), ..Default::default() },
            lighting: if lit { Lighting::Lit } else { Lighting::Flat },
            ..VoxelConfig::default()
        };
        cfg.light.candle_min = 0.25;
        cfg.light.candle_period_s = 2.0;
        let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, c).unwrap();
        let params = params_of(&cfg, proj, true);
        let emission = if lit { Emission::of(&cfg) } else { Emission::OFF };
        PackOracle::new(params, models, lit, SpeciesFaces::default(), emission)
    }

    /// Wait out both packs' first sky planes, so the lit tier's comparison is whole.
    fn settle(o: &mut PackOracle, world: &World, flora: &Flora, fauna: &Fauna) {
        for _ in 0..400 {
            o.tick(&world.view(), flora.view(), fauna.view(), 0.0, Frame::Now)
                .unwrap_or_else(|e| panic!("settling: {e}"));
            if !o.light_pending() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the sky planes never arrived");
    }

    fn site(world: &World, x: i64, z: u32) -> cubarium_voxel_flora::Site {
        let v = world.view();
        let y = (0..v.config.height)
            .rev()
            .find(|&y| v.material_at(x, y, z).is_solid())
            .expect("a column with ground");
        cubarium_voxel_flora::Site {
            x: x.rem_euclid(i64::from(v.config.width)) as u32,
            y,
            z,
        }
    }

    fn seed(flora: &mut Flora, world: &World, x: i64, z: u32, species: Species, of_max: f64) {
        let wood = flora.config().species(species).wood_max * of_max;
        assert!(
            flora.apply(world, FloraCommand::Seed { x, z, species, wood }),
            "a support face for the {species:?} at ({x}, {z})"
        );
    }

    fn vine_face(x: u32, y: u32, z: u32, spur: cubarium_voxel_flora::SpurPhase) -> FaceDraw {
        FaceDraw {
            face: cubarium_voxel_flora::Face::new(x, y, z, cubarium_voxel_flora::FaceDir::NegX),
            owner: 1,
            leafiness: 0.9,
            rooted: false,
            dormant: false,
            spur,
        }
    }

    /// The scripted sequence, both tiers: after **every** tick the change-only pack draws
    /// what the full pack draws. An animal walks; a stand is born, grows a stage and dies;
    /// a carcass appears and decays to remains; the vine spreads a face and flowers; a
    /// shower falls and the water flows; the terrain is edited; a glowcap appears and dies;
    /// and the plants live their own lives (`Flora::step`) throughout.
    fn scripted(lit: bool, baked: bool) {
        use cubarium_voxel_flora::{Carcass, Deposit, DepositKind, SpurPhase};
        let c = config();
        let mut o = oracle_with(&c, lit, baked.then(|| models(&c)));
        let mut world = world(&c);
        // A turf under the frond's crown, whose canopy moves when the frond goes.
        let mut plants = flora(
            &world,
            &[
                (1, 1, Species::Umbrellafrond, 1.0),
                (2, 1, Species::Springturf, 1.0),
                (26, 3, Species::Umbrellafrond, 0.4),
            ],
        );
        let mut beast = grazer(&world, 3, 1);
        settle(&mut o, &world, &plants, &beast);
        let tier = if lit { "lit" } else { "flat" };
        let v = c.voxel_volume();
        for t in 0..72u32 {
            match t {
                4 => seed(&mut plants, &world, 14, 2, Species::Bloomcrown, 0.3),
                8 => o.set_cover_draws(Some(vec![vine_face(8, 3, 1, SpurPhase::Bare)])),
                12 => seed(&mut plants, &world, 22, 4, Species::Glowcap, 0.6),
                14 => {
                    let at = site(&world, 18, 4);
                    plants.lay_carcass(at, Carcass { lineage: 0, heading_rad: 0.0, organic: 1.0 });
                    plants.deposit(at, Deposit { kind: DepositKind::Carrion, organic: 0.8, mineral: 0.0, energy: 0.0 });
                }
                18 => o.set_cover_draws(Some(vec![
                    vine_face(8, 3, 1, SpurPhase::Bare),
                    vine_face(8, 4, 1, SpurPhase::Bare),
                ])),
                // A second frond born beside the first: one canopy, if their styles agree.
                20 => seed(&mut plants, &world, 29, 3, Species::Umbrellafrond, 0.9),
                24 => {
                    assert!(plants.apply(&world, FloraCommand::Clear { x: 14, z: 2 }));
                    seed(&mut plants, &world, 14, 2, Species::Bloomcrown, 1.0);
                }
                28 => o.set_cover_draws(Some(vec![
                    vine_face(8, 3, 1, SpurPhase::Bare),
                    vine_face(8, 4, 1, SpurPhase::Flower),
                ])),
                30 => {
                    for x in 0..4 {
                        set(&mut world, x, 11, 2, Material::Rock);
                    }
                }
                // The vine loses its first face.
                38 => o.set_cover_draws(Some(vec![vine_face(8, 4, 1, SpurPhase::Flower)])),
                // The frond over the turf dies.
                56 => assert!(plants.apply(&world, FloraCommand::Clear { x: 1, z: 1 })),
                34 => {
                    // Decayed: the same pool now under the remains fraction of the body.
                    let at = site(&world, 18, 4);
                    plants.lay_carcass(at, Carcass { lineage: 0, heading_rad: 0.0, organic: 40.0 });
                }
                44 => assert!(plants.apply(&world, FloraCommand::Clear { x: 14, z: 2 })),
                50 => {
                    // The glowcap dies now, if its own life has not ended it already.
                    let _ = plants.apply(&world, FloraCommand::Clear { x: 22, z: 4 });
                }
                _ => {}
            }
            // A shower over the pool and the bump, then the water's own step.
            if (10..40).contains(&t) {
                for x in [2i64, 10, 16] {
                    world.apply(Command::AddWater { x, y: 12, z: t % c.depth, volume_m3: v * 0.3 });
                }
            }
            world.step_with(1);
            plants.step(&mut world);
            // The grazer walks a voxel every other tick.
            beast = grazer(&world, 3 + i64::from(t / 2), 1);
            let candle_s = f64::from(t) * 0.05;
            o.tick(&world.view(), plants.view(), beast.view(), candle_s, Frame::Now)
                .unwrap_or_else(|e| panic!("{tier}, scripted tick {t}: {e}"));
        }
    }

    #[test]
    fn the_change_only_pack_draws_what_the_full_pack_draws_flat() {
        scripted(false, false);
    }

    #[test]
    fn the_change_only_pack_draws_what_the_full_pack_draws_lit() {
        scripted(true, false);
    }

    #[test]
    fn the_change_only_pack_draws_what_the_full_pack_draws_flat_with_models() {
        scripted(false, true);
    }

    #[test]
    fn the_change_only_pack_draws_what_the_full_pack_draws_lit_with_models() {
        scripted(true, true);
    }

    /// The staging ring: a pack overwritten before any frame took it, a frame still on the
    /// GPU while the next tick packs into the other buffer, and a frame displaced before
    /// it ran — after each upload the images hold exactly that pack, and it draws what the
    /// full pack draws.
    #[test]
    fn every_buffer_uploads_what_it_owes_through_skips_and_displacements() {
        let c = config();
        let mut o = oracle(&c, false);
        let mut world = world(&c);
        let mut plants = flora(&world, &[(14, 2, Species::Bloomcrown, 1.0), (1, 1, Species::Umbrellafrond, 1.0)]);
        let frames = [
            Frame::Now,
            Frame::Unrecorded,
            Frame::Unrecorded,
            Frame::Now,
            Frame::InFlight,
            Frame::InFlight,
            Frame::Now,
            Frame::InFlight,
            Frame::Now,
            Frame::InFlight,
            Frame::Unrecorded,
            Frame::Now,
        ];
        let v = c.voxel_volume();
        for (t, &frame) in frames.iter().enumerate() {
            world.apply(Command::AddWater { x: 3 + t as i64, y: 10, z: 2, volume_m3: v * 0.5 });
            world.step_with(1);
            plants.step(&mut world);
            let beast = grazer(&world, 2 + t as i64, 1);
            o.tick(&world.view(), plants.view(), beast.view(), 0.0, frame)
                .unwrap_or_else(|e| panic!("tick {t} ({frame:?}): {e}"));
            if t == 7 || t == 9 {
                // The frame just recorded is displaced before it runs.
                o.discard();
            }
        }
    }

    /// Plants living their lives churn styles every tick: hundreds more over time than
    /// the table holds, a few at once. No slot runs out, and a slot freed and taken by a
    /// new style leaves no texel drawing the old one (the oracle sees every texel).
    #[test]
    fn styles_churning_over_time_never_exhaust_the_table_or_leave_a_stale_texel() {
        let c = config();
        let mut o = oracle(&c, false);
        let mut world = world(&c);
        let stands = [
            (1i64, 1u32, Species::Umbrellafrond, 0.5),
            (12, 1, Species::Bloomcrown, 0.5),
            (16, 4, Species::Bloomcrown, 0.8),
            (24, 3, Species::Umbrellafrond, 0.4),
            (29, 0, Species::Umbrellafrond, 0.7),
        ];
        let mut plants = flora(&world, &stands);
        let none = Fauna::new(FaunaConfig::default());
        for t in 0..400 {
            world.step_with(1);
            plants.step(&mut world);
            o.tick(&world.view(), plants.view(), none.view(), 0.0, Frame::Now)
                .unwrap_or_else(|e| panic!("tick {t}: {e}"));
        }
        assert!(
            o.styles_made() > MAX_STYLES as u64,
            "the plants churned only {} styles",
            o.styles_made()
        );
    }

    /// A tick where nothing changed uploads nothing: no brick and no plane.
    #[test]
    fn a_tick_where_nothing_changed_uploads_nothing() {
        let c = config();
        let mut o = oracle(&c, false);
        let world = world(&c);
        let plants = flora(&world, &[(14, 2, Species::Bloomcrown, 1.0)]);
        let beast = grazer(&world, 4, 1);
        let tick = |o: &mut PackOracle| {
            o.tick(&world.view(), plants.view(), beast.view(), 0.0, Frame::Now).unwrap()
        };
        let first = tick(&mut o);
        assert_eq!(first.bricks, None, "the first pack carries the whole volume");
        tick(&mut o);
        let still = tick(&mut o);
        assert_eq!(still.bricks, Some(0), "nothing changed, no brick");
        assert_eq!(still.copied, [false; SLOW_PLANES], "and no plane");
    }

    /// A frame displaced after a newer pack was made, in the order the presenters make it:
    /// record A, pack B, displace A, record, run. Nothing either pack changed is lost —
    /// first with A carrying bricks, then with A the first pack, carrying the whole volume.
    #[test]
    fn a_displaced_frame_and_a_newer_pack_lose_no_change() {
        let c = config();
        let v = c.voxel_volume();
        for a_whole in [true, false] {
            let mut o = oracle(&c, false);
            let mut world = world(&c);
            let plants = flora(&world, &[(14, 2, Species::Bloomcrown, 1.0)]);
            let mut tick = |o: &mut PackOracle, world: &mut World, x: i64| {
                world.apply(Command::AddWater { x, y: 10, z: 2, volume_m3: v * 0.5 });
                world.step_with(1);
                let beast = grazer(world, x, 1);
                o.pack(&world.view(), plants.view(), beast.view(), 0.0).unwrap().expect("a buffer");
            };
            if !a_whole {
                tick(&mut o, &mut world, 2);
                o.record();
                o.retire();
            }
            tick(&mut o, &mut world, 4); // A
            o.record();
            tick(&mut o, &mut world, 6); // B, into the other buffer
            o.discard(); // A never runs
            o.record();
            o.retire();
            o.check_images().unwrap_or_else(|e| panic!("A whole: {a_whole}: {e}"));
            // The next pack carries what A owed.
            tick(&mut o, &mut world, 8);
            o.record();
            o.retire();
            o.check_images().unwrap_or_else(|e| panic!("A whole: {a_whole}, the pack after: {e}"));
        }
    }

    /// Every interleaving the presenters can make — packs refused while both buffers are
    /// read, frames recorded, run and displaced in any legal order — through a scripted
    /// world's life: the images always hold the latest pack outside the owed bricks.
    #[test]
    fn random_frame_orders_never_lose_a_change() {
        let c = config();
        let v = c.voxel_volume();
        for seed in [0x9e37_79b9_7f4a_7c15u64, 0x2545_f491_4f6c_dd1d, 7] {
            let mut rng = seed;
            let mut o = oracle(&c, false);
            let mut world = world(&c);
            let mut plants = flora(&world, &[(14, 2, Species::Bloomcrown, 1.0), (1, 1, Species::Umbrellafrond, 1.0)]);
            for t in 0..300u32 {
                if t % 5 == 0 {
                    world.apply(Command::AddWater { x: i64::from(t % 30), y: 10, z: t % c.depth, volume_m3: v * 0.5 });
                }
                world.step_with(1);
                plants.step(&mut world);
                let beast = grazer(&world, 2 + i64::from(t / 3 % 28), 1);
                o.tick_random(&world.view(), plants.view(), beast.view(), 0.0, &mut rng)
                    .unwrap_or_else(|e| panic!("seed {seed:#x}, tick {t}: {e}"));
            }
        }
    }

    /// More styles at once than the table holds: the texels that overflowed draw with slot
    /// 0 for now, and once slots are free again they take their own style at the next
    /// tick, though nothing about their cells changed.
    #[test]
    fn overflowed_texels_take_their_own_style_once_slots_free() {
        let c = config();
        let mut o = oracle(&c, false);
        let world = world(&c);
        let none = Fauna::new(FaunaConfig::default());
        let first = flora(&world, &[(14, 2, Species::Bloomcrown, 1.0)]);
        o.tick(&world.view(), first.view(), none.view(), 0.0, Frame::Now).unwrap();
        // Every free slot taken by a style no texel draws with.
        let held = o.fill_palette();
        let both = flora(&world, &[(14, 2, Species::Bloomcrown, 1.0), (1, 1, Species::Umbrellafrond, 1.0)]);
        assert!(
            o.tick(&world.view(), both.view(), none.view(), 0.0, Frame::Now).is_err(),
            "the new stand overflowed the table"
        );
        o.empty_palette(held);
        // Nothing moves; the overflowed texels resolve to their own style all the same.
        o.tick(&world.view(), both.view(), none.view(), 0.0, Frame::Now)
            .unwrap_or_else(|e| panic!("after slots freed: {e}"));
        o.tick(&world.view(), both.view(), none.view(), 0.0, Frame::Now)
            .unwrap_or_else(|e| panic!("a tick later: {e}"));
    }

    // ---- H3: a pack of a snapshot against a pack of the live world ----

    /// One packer's side of the comparison: its buffer, owed bricks and carry.
    struct Side {
        packer: Packer,
        planes: Planes,
        owed: BrickOwed,
        carried: Carried,
    }

    impl Side {
        fn new(c: &Config, models: Option<Arc<ModelLibrary>>, figures: bool) -> Side {
            let cfg = VoxelConfig {
                sim: cubarium_session::SimParams { world: c.clone(), ..Default::default() },
                ..VoxelConfig::default()
            };
            let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, c).unwrap();
            let params = params_of(&cfg, proj, true);
            let mut packer = Packer::new(&params, models, false, SpeciesFaces::default(), Emission::OFF);
            packer.figures = figures;
            Side {
                packer,
                planes: Planes::junk(&params),
                owed: BrickOwed::new(c.width, c.height, c.depth),
                carried: Carried::default(),
            }
        }

        fn pack(&mut self, view: &VoxelView<'_>, flora: FloraView<'_>, fauna: FaunaView<'_>, hint: Hint<'_>) {
            let c = view.config;
            self.packer.prepare(view, flora, fauna, 0.0);
            let mut key = 0;
            let out = self.planes.staging([true; SLOW_PLANES], &mut self.owed, &mut self.carried, &mut key);
            self.packer.fill_with(view, c.width, c.height, c.depth, out, hint);
        }
    }

    /// Every texel of `a` and `b` as the shader resolves it; `water_only` compares only
    /// the free-water byte (a sweep's pore water may trail).
    fn same_texels(a: &Packer, b: &Packer, what: &str, water_only: bool) {
        for i in 0..a.master().len() {
            if water_only {
                assert_eq!(a.master()[i].0[1], b.master()[i].0[1], "{what}: texel {i}'s free water");
            } else {
                assert_eq!(a.resolved(i), b.resolved(i), "{what}: texel {i}");
            }
        }
    }

    /// The scripted world, packed live by one packer and through snapshots (copied, taken
    /// into a mirror, packed from its views) by another: with the sweep at 1 the two draw
    /// the same texels after every tick; with a longer sweep the free water still agrees
    /// every tick, and once the sweep has gone round with the world standing still,
    /// everything does.
    fn snapshot_against_live(sweep: u32, models: bool, figures: bool) {
        let c = config();
        let lib = models.then(|| super::tests::models(&c));
        let (mut live, mut snapped) = (Side::new(&c, lib.clone(), figures), Side::new(&c, lib, figures));
        let mut copier = snapshot::SnapshotCopier::new(sweep);
        let mut mirror = Mirror::default();
        let mut snap = PackSnapshot::default();
        let mut world = world(&c);
        let mut plants = flora(&world, &[(1, 1, Species::Umbrellafrond, 1.0), (26, 3, Species::Umbrellafrond, 0.4)]);
        let mut beast = grazer(&world, 3, 1);
        let v = c.voxel_volume();
        let mut pack_both = |world: &World, plants: &Flora, beast: &Fauna, what: &str, water_only: bool| {
            live.pack(&world.view(), plants.view(), beast.view(), Hint::default());
            copier.fill(&world.view(), plants.view(), beast.view(), &mut snap);
            mirror.apply(&mut snap);
            {
                let (view, flora, fauna) = mirror.views(&snap);
                snapped.pack(&view, flora, fauna, mirror.hint(&snap));
            }
            mirror.packed();
            same_texels(&live.packer, &snapped.packer, what, water_only);
        };
        for t in 0..48u32 {
            match t {
                6 => seed(&mut plants, &world, 14, 2, Species::Bloomcrown, 0.6),
                20 => {
                    for x in 0..4 {
                        set(&mut world, x, 11, 2, Material::Rock);
                    }
                }
                30 => assert!(plants.apply(&world, FloraCommand::Clear { x: 1, z: 1 })),
                _ => {}
            }
            if (8..30).contains(&t) {
                for x in [2i64, 10, 16] {
                    world.apply(Command::AddWater { x, y: 12, z: t % c.depth, volume_m3: v * 0.3 });
                }
            }
            world.step_with(1);
            plants.step(&mut world);
            beast = grazer(&world, 3 + i64::from(t / 2), 1);
            let what = format!("sweep {sweep}, tick {t}");
            pack_both(&world, &plants, &beast, &what, sweep > 1);
        }
        // The world stands still while the sweep goes round once more.
        for k in 0..sweep {
            pack_both(&world, &plants, &beast, &format!("sweep {sweep}, standing {k}"), k + 1 < sweep);
        }
    }

    #[test]
    fn a_pack_of_a_snapshot_draws_what_a_pack_of_the_world_draws() {
        snapshot_against_live(1, false, false);
    }

    #[test]
    fn a_pack_of_a_snapshot_draws_what_a_pack_of_the_world_draws_with_stamped_models() {
        snapshot_against_live(1, true, false);
    }

    #[test]
    fn a_pack_of_a_snapshot_draws_what_a_pack_of_the_world_draws_with_model_figures() {
        snapshot_against_live(1, true, true);
    }

    #[test]
    fn a_swept_snapshot_keeps_free_water_current_and_catches_up_on_pore_water() {
        snapshot_against_live(4, false, false);
        snapshot_against_live(7, true, false);
    }

    /// A snapshot filled again before any mirror took it in keeps the pore windows it
    /// carried: none of the grid is skipped.
    #[test]
    fn a_snapshot_refilled_before_it_is_taken_loses_no_pore_window() {
        let c = config();
        let world = world(&c);
        let (plants, beast) = (Flora::new(FloraConfig::default()), Fauna::new(FaunaConfig::default()));
        let mut copier = snapshot::SnapshotCopier::new(5);
        let mut snap = PackSnapshot::default();
        let mut mirror = Mirror::default();
        // The first fill is whole; take it in.
        copier.fill(&world.view(), plants.view(), beast.view(), &mut snap);
        mirror.apply(&mut snap);
        mirror.packed();
        // Five fills, only the last taken in: the windows of all five arrive.
        for _ in 0..5 {
            copier.fill(&world.view(), plants.view(), beast.view(), &mut snap);
        }
        mirror.apply(&mut snap);
        let owed = mirror.hint(&snap).pore.expect("windows, not the whole grid").to_vec();
        let n = world.view().pore.len();
        let mut seen = vec![false; n];
        for (start, len) in owed {
            seen[start..start + len].iter_mut().for_each(|s| *s = true);
        }
        assert!(seen.iter().all(|&s| s), "five windows of a five-tick sweep cover the grid");
    }
}
