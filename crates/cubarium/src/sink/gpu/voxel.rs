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
    MAX_GLYPHS, MAX_STYLES, PART_ANIMAL_INTERIM, PART_NONE, PLANE_GLOW, PLANE_GLYPHS, PLANE_ROOF,
    PLANE_SKY, SLOW_PLANES, FrameClock, VoxelParams, VoxelRenderer, VoxelStaging, VoxelStyle, VoxelTexel, VoxelTextures,
};
use cubarium_voxel::{Material, VoxelView, World};
use cubarium_voxel_flora::{FaceDraw, Flora, FloraView};

use crate::present::srgb_linear;
use crate::sink::{FrameSink, Output, WebSink};
use crate::voxel::{Lighting, VoxelConfig};
use crate::voxel::animal::{AnimalPart, Animals};
use crate::voxel::{appearance, colours};
use crate::voxel::model::{ModelLibrary, Tag};
use crate::voxel::present as cpu;
use crate::voxel::project::Projection;
use crate::voxel::stand::{Part, Stands, Style};
use crate::voxel::textures::SpeciesFaces;
use crate::voxel::vine::{self, VineCell};
use cubarium_voxel_fauna::{Fauna, FaunaView};

use super::light::{Canopy, Glow, SkyWorker};
use super::target::{GpuTarget, GpuTargetKind};

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
}

impl Default for VoxelGpuSinkOptions {
    fn default() -> VoxelGpuSinkOptions {
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            capture: None,
            roof_from_texture: true,
            models: None,
            textures_under: None,
        }
    }
}

/// `cubarium voxel --sink gpu`.
pub struct VoxelGpuSink {
    /// Shared with the presenting thread, which submits on the same queue.
    gpu: std::sync::Arc<Gpu>,
    renderer: VoxelRenderer,
    target: GpuTarget,
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
    /// The sky this world is drawn under, kept while the founding frame dims it.
    founding_sky: Option<([f32; 3], [f32; 3])>,
    /// When the founding frame's pulse started, so a held founding frame keeps its phase.
    founding_since: Option<Instant>,
    /// The lit tier's first world frame waits for its sky plane: the world is founded,
    /// but until the sky plane for its terrain arrives every pack is refused (the world
    /// stays owed) and the founding frame goes on pulsing.
    awaiting_light: bool,
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
}

impl VoxelGpuSink {
    /// Open the device and build the renderer for this world and projection.
    pub fn new(
        cfg: &VoxelConfig,
        proj: Projection,
        options: VoxelGpuSinkOptions,
    ) -> Result<VoxelGpuSink> {
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
        // The panel is presented on its own thread: the queue submit and the fence wait
        // are 8 ms the run loop has better things to do with. Every other target is
        // unchanged (`sink/gpu/target.rs`).
        let target = GpuTarget::open(
            options.target,
            &gpu,
            &mut renderer,
            "cubarium — voxel strip (GPU)",
            true,
        )?;
        Ok(VoxelGpuSink {
            gpu,
            renderer,
            target,
            packer: Packer::new(
                &params,
                options.models.clone(),
                textures.vine_on,
                species,
                Emission::of(cfg),
            ),
            capture: options.capture,
            water_hz: cfg.light.water_hz,
            water_smooth: cfg.light.water_smooth,
            force_rain: false,
            weather_preview: None,
            founding_sky: None,
            founding_since: None,
            awaiting_light: false,
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
        })
    }

    /// Feed `web` from this renderer's own readback, at most `rate` times a second.
    pub fn with_web(mut self, web: WebSink, rate: f64) -> VoxelGpuSink {
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
    pub fn read_raster(&self) -> Result<Vec<u8>> {
        self.renderer.read_raster(&self.gpu)
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
        if !self.renderer.can_stage() {
            self.packs_skipped += 1;
            return false;
        }
        let started = Instant::now();
        let p = self.renderer.params();
        let slow = self.packer.prepare(view, flora, fauna);
        if self.awaiting_light {
            if self.packer.light_pending() {
                self.pack_ms += (Instant::now() - started).as_secs_f64() * 1e3;
                return false;
            }
            self.awaiting_light = false;
            if let Err(e) = self.restore_sky() {
                eprintln!("cubarium voxel: restoring the sky after founding: {e:#}");
            }
        }
        let packer = &mut self.packer;
        self.renderer.stage(slow, |out| {
            packer.fill(view, p.width, p.height, p.depth, out)
        });
        let atmosphere = if p.sky_gradient && view.atmosphere_m3 > 0.0 {
            (view.atmosphere_m3 as f32 / 1.5).clamp(0.1, 1.0)
        } else {
            0.0
        };
        // A weather preview draws its own weather, not the world's shower.
        let raining = view.is_raining() && self.weather_preview.is_none();
        let rain_tick = if p.sky_gradient && (raining || self.force_rain) {
            view.tick as f32
        } else {
            0.0
        };
        self.renderer.update_weather(atmosphere, rain_tick);
        if self.weather_preview.is_none() {
            self.renderer.set_weather(super::weather::weather_of(&view.weather));
        }
        self.renderer.set_water_visible(self.packer.wet);
        self.ticks_staged += 1;
        self.pack_ms += (Instant::now() - started).as_secs_f64() * 1e3;
        true
    }

    /// The frame's time: the sim's `tick` plus the `fraction` of the next tick elapsed.
    /// The lit tier's water animates on it (`[light] water_hz` steps a second of sim
    /// time), and redraws a frame whose step has moved while there is water; the flat
    /// tier ignores it.
    ///
    /// The weather eases on it (`cubarium_gpu::weather`); a preview
    /// ([`Self::set_weather_preview`]) sets the weather here, every frame, from its script.
    pub fn set_clock(&mut self, tick: u64, fraction: f64) {
        if let Some((preview, start)) = &mut self.weather_preview {
            let hz = f64::from(cubarium_voxel::TICK_HZ);
            let now = (tick as f64 + fraction) / hz;
            let (t0, tick0) = *start.get_or_insert((now, tick));
            let view = preview.at(now - t0, tick0, hz);
            self.renderer.set_weather(super::weather::weather_of(&view));
        }
        self.renderer.set_clock(FrameClock::at(
            tick,
            fraction,
            f64::from(self.water_hz),
            f64::from(cubarium_voxel::TICK_HZ),
            self.water_smooth,
        ));
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
        self.packer.light_pending()
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

    /// What the panel has actually been shown, where the target knows: presented frames
    /// and frames the loop had nowhere to put. `None` when every frame drawn is
    /// presented, which is every target but the panel's presenting thread.
    pub fn presented(&self) -> Option<(u64, u64)> {
        self.target.presented()
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
        if let Some((sky, horizon)) = self.founding_sky.take() {
            let mut params = self.renderer.params();
            params.sky = sky;
            params.sky_horizon = horizon;
            self.renderer.set_params(params)?;
        }
        Ok(())
    }

    pub fn render(&mut self) -> Result<()> {
        if self.awaiting_light
            && let Some(since) = self.founding_since
        {
            self.pulse(since.elapsed().as_secs_f64())?;
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

    /// True once the target wants the run to stop (the development window was closed).
    pub fn should_quit(&mut self) -> bool {
        self.target.should_quit()
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

impl Drop for VoxelGpuSink {
    fn drop(&mut self) {
        self.target.destroy(&self.gpu);
        self.renderer.destroy(&self.gpu);
    }
}

/// One tick's world as texels: the stage loop, and what it keeps between ticks.
///
/// **What it keeps.** The roof table is a function of the terrain alone, so it is built
/// when the terrain moves and copied into a staging buffer only when that buffer does not
/// hold it yet (`cubarium_gpu::voxel::SlowPlanes`); the glyph atlas is a function of the
/// projection and is built once. The plant and animal grids clear only what they stamped.
///
/// **What it walks.** Every voxel once, in the world's own order, for material and water;
/// then only the cells a stand or an animal stamped, in texture order — the order the old
/// per-voxel loop met them in, so the style slots come out numbered the same.
struct Packer {
    /// The CPU presenter's own stand decomposition, rebuilt once per staged tick.
    stands: Stands,
    /// The frame's animals on the same grid, rebuilt per staged tick (round 5c).
    animals: Animals,
    /// The baked voxel models, or `None` for the dev-mode glyphs.
    models: Option<Arc<ModelLibrary>>,
    /// GPU style slots, in the order they were first needed this tick, with the texture
    /// role each draws with (`ROLE_*`) and its species face slots.
    styles: Vec<StyleKey>,
    /// Which species face slots each model cell draws with ([`SpeciesFaces`]).
    species: SpeciesFaces,
    /// `Stands` style index → GPU slot for this tick, so the dedup costs one linear scan
    /// per stand and not one per voxel.
    slot_of: Vec<Option<u8>>,
    /// Plant voxels that had to reuse style 0 because the frame wanted more than
    /// [`MAX_STYLES`] distinct styles.
    style_overflow: u64,
    /// The roof table in texture order, for the terrain in [`Packer::roof_materials`].
    roof: Vec<u8>,
    /// The materials `roof` was built from, and their `terrain_version`. The version says
    /// when the terrain moved in one world, but two worlds can share a version (a fresh
    /// world starts at zero, as the empty founding world does), so the materials
    /// themselves are compared too — a few hundred microseconds, not a rebuild.
    roof_materials: Vec<Material>,
    roof_version: Option<u64>,
    /// Bumped every time `roof` is rebuilt: the key its plane is uploaded under.
    roof_key: u64,
    /// The whole glyph plane, atlas and zero tail, built once.
    glyphs: Vec<u8>,
    /// Scratch: the overlay's texture indices.
    overlay: Vec<u32>,
    /// This tick's latticevine cells (`crate::voxel::vine`), for the tile layer.
    vines: Vec<VineCell>,
    /// Whether the latticevine is the tile layer (textures on) rather than plain cells.
    vine_tiles: bool,
    /// Faces to draw instead of the flora's own cover: a fixture posing spur phases and
    /// dormancy by hand.
    cover_override: Option<Vec<FaceDraw>>,
    /// The lit tier's planes (`super::light`), or `None` in the flat tier, which packs
    /// neither.
    light: Option<LightPlanes>,
    /// Whether the last fill wrote any free water (the lit tier animates it).
    wet: bool,
}

/// The lit tier's emitters ([`crate::voxel::LightConfig`]'s `emission`, `glow`,
/// `glow_reach`). Off in the flat tier whatever the config says.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Emission {
    on: bool,
    glow: f32,
    reach: u32,
}

impl Emission {
    #[cfg(test)]
    pub(crate) const OFF: Emission = Emission {
        on: false,
        glow: 0.0,
        reach: 0,
    };

    fn of(cfg: &VoxelConfig) -> Emission {
        Emission {
            on: cfg.lighting == Lighting::Lit && cfg.light.emission,
            glow: cfg.light.glow.max(0.0),
            reach: cfg.light.glow_reach,
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
    /// This tick's canopy plane, built in `prepare` from the flora and copied in `fill`.
    canopy_plane: Vec<u8>,
    /// The emitters' local light, rebuilt in `prepare` when the emitters or the terrain
    /// moved, and its settings.
    glow: Glow,
    emission: Emission,
    /// Scratch: this tick's emitting voxels and their colours.
    emitters: Vec<((u32, u32, u32), [f32; 3])>,
}

/// The glyph plane never changes after construction: one key for the life of a renderer.
const GLYPHS_KEY: u64 = 0;

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
        if p.lit {
            // The lit tier divides a stand's own shade out of its own cells.
            stands.track_owners();
            stands.set_emission(emission.on);
        }
        let mut animals = Animals::empty(p.width, p.height, p.depth);
        animals.set_emission(p.lit && emission.on);
        Packer {
            stands,
            vine_tiles: tiles,
            animals,
            models,
            styles: Vec::new(),
            species,
            slot_of: Vec::new(),
            style_overflow: 0,
            roof: vec![0; p.voxel_count()],
            roof_materials: Vec::new(),
            roof_version: None,
            roof_key: 0,
            glyphs,
            overlay: Vec::new(),
            vines: Vec::new(),
            cover_override: None,
            wet: false,
            light: p.lit.then(|| LightPlanes {
                sky: vec![255; p.voxel_count()],
                sky_key: 0,
                submitted: 0,
                current: false,
                worker: SkyWorker::spawn(),
                canopy: Canopy::new(p.width, p.depth),
                canopy_plane: vec![0; p.canopy_bytes()],
                glow: Glow::new(p.width, p.height, p.depth),
                emission,
                emitters: Vec::new(),
            }),
        }
    }

    /// Whether the lit tier's sky plane is still being computed for the terrain last
    /// packed. Always false in the flat tier.
    fn light_pending(&self) -> bool {
        self.light.as_ref().is_some_and(|l| !l.current)
    }

    /// Everything that is not a write into the staging buffer: the plant and animal
    /// grids, the roof if the terrain moved, and in the lit tier the canopy, the terrain
    /// handed to the sky's thread if it moved, and the sky plane if one has arrived.
    /// Returns the slow planes' keys.
    fn prepare(
        &mut self,
        view: &VoxelView<'_>,
        flora: FloraView<'_>,
        fauna: FaunaView<'_>,
    ) -> [u64; SLOW_PLANES] {
        match self.models.as_deref() {
            Some(lib) => {
                self.stands.rebuild_with(view, flora, lib);
                self.animals.rebuild_with(view, Some(fauna), lib);
            }
            None => {
                self.stands.rebuild(view, flora);
                self.animals.rebuild(view, Some(fauna));
            }
        }
        self.vines.clear();
        if self.vine_tiles {
            let draws = match &self.cover_override {
                Some(d) => d.clone(),
                None => flora.cover.draw(),
            };
            self.vines = vine::cells(&draws, view.config, |id| {
                flora.cover.vine(id).map(|v| v.root)
            });
        }
        if self.roof_version != Some(view.terrain_version)
            || self.roof_materials.as_slice() != view.material
        {
            let c = view.config;
            roof_table(view.material, c.width, c.height, c.depth, &mut self.roof);
            self.roof_materials.clear();
            self.roof_materials.extend_from_slice(view.material);
            self.roof_version = Some(view.terrain_version);
            self.roof_key += 1;
            if let Some(l) = &mut self.light {
                l.submitted += 1;
                l.current = false;
                l.worker.submit(l.submitted, view.config, view.material);
                l.glow.set_terrain(view.config, view.material);
            }
        }
        let mut keys = [0; SLOW_PLANES];
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
            l.canopy.build(flora, &mut l.canopy_plane);
            let stands = &self.stands;
            l.canopy.count_crowns(
                stands
                    .cells()
                    .filter(|&(x, y, z)| {
                        matches!(stands.at(i64::from(x), i64::from(y), z), Part::Crown { .. })
                    })
                    .map(|(x, _, z)| (x, z)),
            );
            keys[PLANE_SKY] = l.sky_key;
            // The emitters: every stamped cell whose style emits, and the tile layer's
            // flowering vine cells. A cell an animal stands in still counts: the glow
            // volume is coarse.
            l.emitters.clear();
            if l.emission.on {
                l.emitters.extend(stands.cells().filter_map(|(x, y, z)| {
                    let rgb = stands.emission(stands.at(i64::from(x), i64::from(y), z))?;
                    Some(((x, y, z), rgb))
                }));
                let animals = &self.animals;
                l.emitters.extend(animals.cells().filter_map(|(x, y, z)| {
                    let rgb = animals.emission(animals.at(i64::from(x), i64::from(y), z))?;
                    Some(((x, y, z), rgb))
                }));
                for c in self.vines.iter().filter(|c| c.kind.tiled()) {
                    if let Some(rgb) = colours::vine_emission(c.accent) {
                        l.emitters.push(((c.x, c.y, c.z), srgb_linear(rgb)));
                    }
                }
            }
            l.glow.build(l.emitters.iter().copied(), l.emission.glow, l.emission.reach);
            keys[PLANE_GLOW] = l.glow.key;
        }
        keys[PLANE_ROOF] = self.roof_key;
        keys[PLANE_GLYPHS] = GLYPHS_KEY;
        keys
    }

    /// Write the prepared tick into `out`.
    fn fill(&mut self, view: &VoxelView<'_>, w: u32, h: u32, d: u32, out: VoxelStaging<'_>) {
        self.styles.clear();
        self.slot_of.clear();
        let (wu, hu, du) = (w as usize, h as usize, d as usize);
        let mut wet = 0u8;
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
                for (x, texel) in out.voxels[dst..dst + wu].iter_mut().enumerate() {
                    *texel = texel_of(material[x], free[x], pore[x], PART_NONE, 0, 0);
                    wet |= texel.0[1];
                }
            }
        }
        self.wet = wet != 0;
        // Then the plants and animals, only where they stand, in texture order.
        let stands = &self.stands;
        let animals = &self.animals;
        let beasts = !animals.is_empty();
        self.overlay.clear();
        let index = |(x, y, z): (u32, u32, u32)| VoxelStaging::index(w, h, x, y, z) as u32;
        self.overlay.extend(stands.cells().map(index));
        if beasts {
            self.overlay.extend(animals.cells().map(index));
        }
        self.overlay.sort_unstable();
        self.overlay.dedup();
        for &i in &self.overlay {
            let i = i as usize;
            let (x, y, z) = (
                (i % wu) as u32,
                ((i / wu) % hu) as u32,
                (i / (wu * hu)) as u32,
            );
            let src = (y as usize * du + z as usize) * wu + x as usize;
            let m = view.material[src];
            if m.is_solid() {
                continue;
            }
            let xi = i64::from(x);
            // The animal after the plant in its own cell, as the CPU presenter stamps it:
            // a body standing in a turf covers the turf.
            let beast = if beasts {
                animals.at(xi, i64::from(y), z)
            } else {
                AnimalPart::None
            };
            // In the lit tier a stand's cell carries its canopy and crown pass in `b`.
            let mut plant = None;
            let (part, glyph, slot) = if let Some(style) = animals.style(beast) {
                // An emitting part (lit tier, `colours::animal_emission`) draws the
                // emissive animal glyph in its emissive colour.
                let emit = animals.emission(beast);
                let glyph = match emit {
                    Some(_) => appearance::animal_emissive_glyph(beast.glyph()),
                    None => beast.glyph(),
                };
                (
                    PART_ANIMAL_INTERIM,
                    glyph.0,
                    slot_for_style(
                        (style, 0, [None; 2], emit.map(|e| emit_key(e, false))),
                        &mut self.styles,
                        &mut self.style_overflow,
                    ),
                )
            } else {
                let p = stands.at(xi, i64::from(y), z);
                match appearance::plant_class(p) {
                    PART_NONE => continue,
                    class => {
                        let s = slot_for(
                            stands,
                            &self.species,
                            p,
                            &mut self.styles,
                            &mut self.slot_of,
                            &mut self.style_overflow,
                        );
                        let mut glyph = appearance::plant_glyph(
                            p,
                            !stands.crown_continues(p, xi - 1, y, z),
                            !stands.crown_continues(p, xi + 1, y, z),
                        );
                        // An emitting crown or heart cell draws the atlas's emissive
                        // variant (only the lit tier with emission gives a style one).
                        if matches!(p, Part::Crown { .. }) && stands.emission(p).is_some() {
                            glyph = appearance::emissive_glyph(glyph);
                        }
                        plant = Some(p);
                        (class, glyph.0, s)
                    }
                }
            };
            out.voxels[i] = texel_of(m, view.free[src], view.pore[src], part, glyph, slot);
            if let (Some(p), Some(l)) = (plant, &self.light) {
                let crown = matches!(p, Part::Crown { .. });
                out.voxels[i].0[2] = l.canopy.plant_byte(stands.owner(xi, y, z), crown, x, y, z);
            }
        }
        // The vines last, into the air cells nothing else claimed: an organism, a stand or
        // a ground mark in the same voxel wins.
        for c in self.vines.iter().filter(|c| c.kind.tiled()) {
            let i = VoxelStaging::index(w, h, c.x, c.y, c.z);
            let t = out.voxels[i];
            if t.material() == 0 && t.part() == PART_NONE {
                let (glyph, b, a) = c.texel_bytes();
                out.voxels[i] = t.with_vine(glyph, b, a);
            }
        }
        for (slot, (style, role, faces, emit)) in self.styles.iter().enumerate() {
            out.styles[slot] = VoxelStyle::new(style.wood, style.crown, style.heart)
                .with_role(*role)
                .with_faces(*faces)
                .with_emit(emit.map(|e| [e[0], e[1], e[2]].map(f32::from_bits)))
                .emit_whole(emit.is_some_and(|e| e[3] != 0));
        }
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
            out.canopy.copy_from_slice(&l.canopy_plane);
            if out.write[PLANE_GLOW] {
                out.glow.copy_from_slice(&l.glow.bytes);
            }
        }
    }
}

/// One voxel's texel: its material and water, and whatever part stands in it.
#[inline]
fn texel_of(m: Material, free: f64, pore: f64, part: u8, glyph: u8, slot: u8) -> VoxelTexel {
    let free = free as f32;
    let pore = if m.pore_capacity() > 0.0 {
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

/// One GPU style slot's contents: the colours, the texture role (`ROLE_*`), the species
/// face slots (`VoxelStyle::with_faces`) and the emission ([`emit_key`]).
type StyleKey = (Style, u8, [Option<u16>; 2], Option<[u32; 4]>);

/// An emissive colour and its whole-cell flag as a style key: the colour's bits, then 1
/// for a whole-cell emitter (`VoxelStyle::emit_whole`).
fn emit_key(rgb: [f32; 3], whole: bool) -> [u32; 4] {
    [rgb[0].to_bits(), rgb[1].to_bits(), rgb[2].to_bits(), u32::from(whole)]
}

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

/// The texture role of a baked model cell (`cubarium_gpu::voxel::ROLE_*`); anything that
/// is not a model cell draws untextured.
fn role_of(tag: Option<Tag>) -> u8 {
    use cubarium_gpu::voxel::{ROLE_ACCENT, ROLE_BARK, ROLE_DRAPE, ROLE_LEAF, ROLE_NONE};
    match tag {
        None => ROLE_NONE,
        Some(Tag::Trunk) => ROLE_BARK,
        Some(Tag::Foliage(_)) => ROLE_LEAF,
        Some(Tag::Drape(_)) => ROLE_DRAPE,
        Some(Tag::Accent) => ROLE_ACCENT,
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
    if !cfg.textures {
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

    use super::*;
    use cubarium_gpu::voxel::SlowPlanes;
    use cubarium_voxel::{Command, Config};
    use cubarium_voxel_fauna::{Command as FaunaCommand, FaunaConfig, Species as Beast};
    use cubarium_voxel_flora::{Command as FloraCommand, FloraConfig, Species};

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
                        match appearance::plant_class(p) {
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
                    let pore = if m.pore_capacity() > 0.0 {
                        view.pore_at(xi, y, z).clamp(0.0, 1.0) as f32
                    } else {
                        0.0
                    };
                    out.voxels[VoxelStaging::index(w, h, x, y, z)] = VoxelTexel::pack_glyph(
                        m as u8,
                        part,
                        glyph,
                        free,
                        free <= cpu::WATER_EPSILON,
                        pore,
                        slot,
                    );
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
    }

    impl Planes {
        /// Filled with junk, so a plane a pack should have written and did not shows.
        fn junk(p: &VoxelParams) -> Planes {
            Planes {
                voxels: vec![VoxelTexel([0xAB; 4]); p.voxel_count()],
                roof: vec![0xAB; p.voxel_count()],
                styles: vec![VoxelStyle::new([0.5; 3], [0.5; 3], [0.5; 3]); MAX_STYLES],
                glyphs: vec![0xAB; p.glyph_bytes()],
            }
        }

        /// As `VoxelRenderer::stage` hands it over: the styles zeroed first. The flat
        /// tier's pack has no sky or canopy plane.
        fn staging(&mut self, write: [bool; SLOW_PLANES]) -> VoxelStaging<'_> {
            self.styles.fill(VoxelStyle::default());
            VoxelStaging {
                voxels: &mut self.voxels,
                roof: &mut self.roof,
                styles: &mut self.styles,
                glyphs: &mut self.glyphs,
                sky: &mut [],
                canopy: &mut [],
                glow: &mut [],
                write,
            }
        }
    }

    /// The renderer's side of a pack and an upload, without a device: two buffers
    /// alternating, and the images holding whatever the slow planes' keys say was copied.
    struct Rig {
        params: VoxelParams,
        packer: Packer,
        slow: SlowPlanes,
        buffers: [Planes; 2],
        images: Planes,
        ticks: usize,
    }

    impl Rig {
        fn new(c: &Config) -> Rig {
            let cfg = VoxelConfig {
                world: c.clone(),
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
                buffers: [Planes::junk(&params), Planes::junk(&params)],
                images: Planes::junk(&params),
                params,
                ticks: 0,
            }
        }

        /// Pack and upload one tick, and hold the images to the reference pack.
        fn tick(&mut self, world: &World, flora: &Flora, fauna: &Fauna, what: &str) {
            let p = self.params;
            let view = world.view();
            let b = self.ticks % 2;
            self.ticks += 1;
            let keys = self.packer.prepare(&view, flora.view(), fauna.view());
            let write = self.slow.pack(b, keys);
            self.packer.fill(
                &view,
                p.width,
                p.height,
                p.depth,
                self.buffers[b].staging(write),
            );
            let copy = self.slow.upload(b);
            let buffer = &self.buffers[b];
            self.images.voxels.clone_from(&buffer.voxels);
            self.images.styles.clone_from(&buffer.styles);
            if copy[PLANE_ROOF] {
                self.images.roof.clone_from(&buffer.roof);
            }
            if copy[PLANE_GLYPHS] {
                self.images.glyphs.clone_from(&buffer.glyphs);
            }

            let mut want = Planes::junk(&p);
            reference(&view, flora, fauna, &p, want.staging([true; SLOW_PLANES]));
            let differ =
                |a: &[VoxelTexel], b: &[VoxelTexel]| a.iter().zip(b).position(|(a, b)| a != b);
            if let Some(i) = differ(&self.images.voxels, &want.voxels) {
                panic!(
                    "{what}: voxel texel {i} is {:?}, the old pack wrote {:?}",
                    self.images.voxels[i], want.voxels[i]
                );
            }
            assert!(
                self.images.roof == want.roof,
                "{what}: the roof table differs"
            );
            assert!(
                self.images.styles == want.styles,
                "{what}: the style table differs"
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
        let body = fauna.config().species(Beast::Frondgrazer).body_max;
        let species = Beast::Frondgrazer;
        assert!(
            fauna.apply(
                world,
                FaunaCommand::Introduce {
                    x,
                    z,
                    species,
                    body
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
}
