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
use std::time::Instant;

use anyhow::{Context, Result, bail};

use cubarium_gpu::vk::Gpu;
use cubarium_gpu::voxel::{
    MAX_GLYPHS, MAX_STYLES, PART_ANIMAL_INTERIM, PART_NONE, VoxelParams, VoxelRenderer,
    VoxelStaging, VoxelStyle, VoxelTexel,
};
use cubarium_voxel::{VoxelView, World};
use cubarium_voxel_flora::Flora;

use crate::sink::{FrameSink, Output, WebSink};
use crate::voxel::VoxelConfig;
use crate::voxel::animal::{AnimalPart, Animals};
use crate::voxel::appearance;
use crate::voxel::present as cpu;
use crate::voxel::project::Projection;
use crate::voxel::stand::{Part, Stands, Style};
use cubarium_voxel_fauna::Fauna;

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
}

impl Default for VoxelGpuSinkOptions {
    fn default() -> VoxelGpuSinkOptions {
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            capture: None,
            roof_from_texture: true,
        }
    }
}

/// `cubarium voxel --sink gpu`.
pub struct VoxelGpuSink {
    gpu: Gpu,
    renderer: VoxelRenderer,
    target: GpuTarget,
    /// The CPU presenter's own stand decomposition, rebuilt once per staged tick.
    stands: Stands,
    /// The frame's animals on the same grid, rebuilt per staged tick (round 5c).
    animals: Animals,
    /// GPU style slots, in the order they were first needed this tick.
    styles: Vec<Style>,
    /// `Stands` style index → GPU slot for this tick, so the dedup costs one linear scan
    /// per stand and not one per voxel.
    slot_of: Vec<Option<u8>>,
    /// Plant voxels that had to reuse style 0 because the frame wanted more than
    /// [`MAX_STYLES`] distinct styles.
    style_overflow: u64,
    capture: Option<PathBuf>,
    /// The operator's viewer, fed from this renderer's own readback.
    web: Option<(WebSink, f64)>,
    web_raster: Option<cube_proto::Raster>,
    web_last: Option<Instant>,
    web_frames: u64,
    web_ms: f64,
    // --- measurements ---
    frames: u64,
    ticks_staged: u64,
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
        let gpu = Gpu::open(&[]).context("opening the Vulkan device for --sink gpu")?;
        eprintln!(
            "cubarium voxel: --sink gpu on {} ({:.0} KiB per tick)",
            gpu.name,
            params.upload_bytes() as f64 / 1024.0
        );
        let mut renderer = VoxelRenderer::new(&gpu, params)?;
        let target = GpuTarget::open(
            options.target,
            &gpu,
            &mut renderer,
            "cubarium — voxel strip (GPU)",
        )?;
        Ok(VoxelGpuSink {
            gpu,
            renderer,
            target,
            stands: Stands::empty(params.width, params.height, params.depth),
            animals: Animals::empty(params.width, params.height, params.depth),
            styles: Vec::new(),
            slot_of: Vec::new(),
            style_overflow: 0,
            capture: options.capture,
            web: None,
            web_raster: None,
            web_last: None,
            web_frames: 0,
            web_ms: 0.0,
            frames: 0,
            ticks_staged: 0,
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
    pub fn stage_world(&mut self, world: &World, flora: &Flora, fauna: &Fauna) {
        let started = Instant::now();
        let view = world.view();
        self.stands.rebuild(&view, flora.view());
        self.animals.rebuild(&view, Some(fauna.view()));
        self.styles.clear();
        self.slot_of.clear();

        let p = self.renderer.params();
        let (w, h, d) = (p.width, p.height, p.depth);
        // `Stands` and the style dedup are borrowed inside the closure, so take what it
        // needs out of `self` first; the renderer owns the staging buffer.
        let stands = &self.stands;
        let animals = &self.animals;
        let beasts = !animals.is_empty();
        let styles = &mut self.styles;
        let slot_of = &mut self.slot_of;
        let overflow = &mut self.style_overflow;
        self.renderer.stage(|out| {
            roof_into(&view, w, h, d, out.roof);
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let xi = i64::from(x);
                        let m = view.material_at(xi, y, z);
                        // The animal after the plant in its own cell, as the CPU presenter
                        // stamps it: a body standing in a turf covers the turf.
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
                                slot_for_style(style, styles, overflow),
                            )
                        } else {
                            let p = stands.at(xi, i64::from(y), z);
                            match appearance::plant_class(p) {
                                PART_NONE => (PART_NONE, 0, 0),
                                class => {
                                    let s = slot_for(stands, p, styles, slot_of, overflow);
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
            for (slot, style) in styles.iter().enumerate() {
                out.styles[slot] = VoxelStyle::new(style.wood, style.crown, style.heart);
            }
            out.glyphs.fill(0);
            let used = appearance::atlas_len(p.s, p.rise);
            debug_assert!(used <= out.glyphs.len());
            debug_assert!(appearance::ATLAS_GLYPHS <= MAX_GLYPHS);
            appearance::write_atlas(p.s, p.rise, &mut out.glyphs[..used]);
        });
        let atmosphere = if p.sky_gradient && view.atmosphere_m3 > 0.0 {
            (view.atmosphere_m3 as f32 / 1.5).clamp(0.1, 1.0)
        } else {
            0.0
        };
        let rain_tick = if p.sky_gradient && view.is_raining() {
            view.tick as f32
        } else {
            0.0
        };
        self.renderer.update_weather(atmosphere, rain_tick);
        self.ticks_staged += 1;
        self.pack_ms += (Instant::now() - started).as_secs_f64() * 1e3;
    }

    /// Draw one frame of whatever was last staged, and present it.
    pub fn render(&mut self) -> Result<()> {
        let started = Instant::now();
        let ms = self.target.draw(&self.gpu, &mut self.renderer, ())?;
        self.frames += 1;
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
             {:.0} KiB per tick; roof from {}",
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
        if self.style_overflow > 0 {
            eprintln!(
                "cubarium voxel --sink gpu: {} plant voxels fell back to style 0 \
                 (more than {MAX_STYLES} distinct plant styles in one frame)",
                self.style_overflow
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

/// The GPU slot for a style that has no plant part index to cache under (an animal's),
/// deduplicated by value against the same table the plants fill.
fn slot_for_style(style: Style, styles: &mut Vec<Style>, overflow: &mut u64) -> u8 {
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
    part: Part,
    styles: &mut Vec<Style>,
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

/// `VoxelPresenter::build_roof`, written into the roof texture's own index order:
/// voxels from each one up to the nearest solid above it in its own column, `0` where
/// the column is open to the sky, clamped into a byte.
///
/// The clamp is lossless where it matters: `roof_shade` is `ROOF_LIGHT + (1 −
/// ROOF_LIGHT)(1 − e^−(gap−1)/4)`, which is within 1e-5 of full light by a gap of 50 and
/// so within a thousandth of an 8-bit code long before 255.
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
