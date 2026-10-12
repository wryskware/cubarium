//! Safe voxel passes with the existing CPU packing and frame-state algorithms.
#![forbid(unsafe_code)]
use super::{
    FrameSource, Gpu, PresentPass, TargetSlot,
    bloom::Bloom,
    geometry::{Figures, Plants},
    shader::{Shader, Specialization, constants},
    sunvis::SunVisVolume,
};
use crate::voxel::*;
use anyhow::{Result, bail, ensure};
use std::collections::HashMap;
use wgpu::util::DeviceExt;
struct Plane {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}
struct Staging {
    voxels: Vec<VoxelTexel>,
    roof: Vec<u8>,
    styles: Vec<VoxelStyle>,
    glyphs: Vec<u8>,
    sky: Vec<u8>,
    canopy: Vec<u8>,
    glow: Vec<u8>,
}
pub struct VoxelRenderer {
    closed: bool,
    timing: super::timing::Timing,
    params: VoxelParams,
    raster: Plane,
    backdrop: Plane,
    dummy_colour: Plane,
    dummy_depth: Plane,
    world: Vec<Plane>,
    tex_mask: u32,
    vine_on: bool,
    sunvis: SunVisVolume,
    bloom: Option<Bloom>,
    figures: Figures,
    plants: Option<Plants>,
    world_layout: wgpu::BindGroupLayout,
    walk: Vec<wgpu::RenderPipeline>,
    sky_pipeline: Option<wgpu::RenderPipeline>,
    weather_pipeline: Option<wgpu::RenderPipeline>,
    nearest: wgpu::Sampler,
    linear: wgpu::Sampler,
    present: HashMap<wgpu::TextureFormat, PresentPass>,
    staging: Vec<Staging>,
    ring: StagingRing,
    slow: SlowPlanes,
    owed: BrickOwed,
    carried: Vec<Carried>,
    last_upload: usize,
    upload_totals: (u64, u64),
    frame_uploads: Vec<(u64, u64)>,
    dirty: bool,
    raster_current: bool,
    staged: bool,
    pose_tables: Vec<PoseTable>,
    drawn_poses: PoseTable,
    posing: bool,
    emit_curve_tables: Vec<[u8; MAX_STYLES]>,
    drawn_emit_curves: [u8; MAX_STYLES],
    curving: bool,
    curve_gain: Vec<f32>,
    emit_cells: Vec<EmitCell>,
    emit_scratch: Vec<[u32; 4]>,
    clock: FrameClock,
    weather: crate::weather::Weather,
    weather_ease: crate::weather::WeatherEase,
    weather_at: f64,
    look: crate::weather::Look,
    fog_floor: f32,
    water_visible: bool,
    version: u64,
    redrew: bool,
    view_x: i32,
    view_y: i32,
    projection_base: i32,
    projection_height: u32,
    highlight: Option<Highlight>,
    last_done: Option<usize>,
    figure_frame: crate::figures::FigureFrame,
    figures_dropped: u64,
}
impl VoxelRenderer {
    pub fn set_params(&mut self, params: VoxelParams) -> Result<()> {
        let fixed = |p: &VoxelParams| {
            (
                p.s,
                p.rise,
                p.base,
                p.width,
                p.height,
                p.depth,
                p.raster_w,
                p.raster_h,
                p.lit,
                p.effects.spec_constants(p.lit),
            )
        };
        if fixed(&params) != fixed(&self.params) {
            bail!(
                "the projection, the world's extent, the lighting tier and its effect \
                 switches are fixed for a VoxelRenderer"
            );
        }
        self.params = params;
        // The look reads the palette (the founding pulse dims the sky through here).
        self.look = match self.weather_ease.current() {
            Some(e) => crate::weather::look(&e, &self.params),
            None => crate::weather::noon(&self.params),
        };
        self.raster_current = false;
        self.version += 1;
        Ok(())
    }

    /// Update dynamic atmosphere moisture and rain animation tick uniforms.
    /// **Nothing is written to the GPU here.** The uniform block a frame reads is written
    /// when that frame is recorded, into that frame's own slot; writing it now would write
    /// under whatever frame the GPU is reading.
    pub fn update_weather(&mut self, atmosphere: f32, rain_tick: f32) {
        self.params.atmosphere = atmosphere;
        self.params.rain_tick = rain_tick;
        self.raster_current = false;
        self.version += 1;
    }

    /// Move the frame's clock. **Animated water means redrawing**: in the lit tier, while
    /// the last pack held water, a clock whose animation step has moved clears the redraw
    /// skip, so the water animates at its step rate between ticks (the picture changes
    /// only when the step does, so a frame at the same step is still skipped). The flat
    /// tier draws nothing from the clock and keeps its redraw skip.
    ///
    /// **The weather eases on it too** ([`VoxelRenderer::set_weather`]). The lit tier
    /// takes the eased look every frame and redraws while it changes or while the weather
    /// animates (drifting clouds, twinkling stars, rain, fog, a flash). The flat tier takes
    /// it only when the clock crosses a tick, so its redraw rate stays the tick rate
    /// whatever the sky does (its rain and flash step at the tick).
    pub fn set_clock(&mut self, clock: FrameClock) {
        let moved = clock.step != self.clock.step;
        let new_tick = clock.time.floor() != self.clock.time.floor();
        let first = self.weather_ease.current().is_none();
        let ticked = clock.time != self.clock.time;
        self.clock = clock;
        let mut redraw = moved && self.params.lit && self.water_visible;
        // A posing animal moves every frame in the lit tier; the flat tier keeps its tick
        // rate and draws the pose at the clock of the frame that redraws.
        redraw |= self.posing && self.params.lit && ticked;
        if self.params.lit || new_tick || first {
            let dt = (clock.seconds - self.weather_at) as f32;
            self.weather_at = clock.seconds;
            let eased =
                self.weather_ease
                    .advance(&self.weather, dt, self.params.width, clock.seconds);
            let look = crate::weather::look(&eased, &self.params);
            redraw |= !look.same_picture(&self.look);
            self.look = look;
            // The plants sway on the frame clock in the lit tier, once a tick in the flat
            // tier (its redraw rate), and redraw while a tip moves or a cell changes size.
            if let Some(p) = &mut self.plants {
                redraw |= p.set_clock(clock.seconds, self.look.rain);
            }
        }
        if redraw {
            self.raster_current = false;
            self.version += 1;
        }
    }

    /// The weather to draw from now on: the world's (`WeatherView`, once a tick) or a
    /// preview's (every frame). Nothing changes until the next [`VoxelRenderer::set_clock`]
    /// eases toward it.
    pub fn set_weather(&mut self, weather: crate::weather::Weather) {
        self.weather = weather;
    }

    /// Where the ground fog lies: its floor, voxels up (the low ground: the sink takes a
    /// low percentile of the terrain's column tops). The picture moves only while there
    /// is fog.
    pub fn set_fog_floor(&mut self, y: f32) {
        if y != self.fog_floor {
            self.fog_floor = y;
            if self.look.fog > 0.0 {
                self.raster_current = false;
                self.version += 1;
            }
        }
    }

    /// The look the next frame draws.
    pub fn look(&self) -> crate::weather::Look {
        self.look
    }

    /// The ground fog's floor, voxels up ([`VoxelRenderer::set_fog_floor`]).
    pub fn fog_floor(&self) -> f32 {
        self.fog_floor
    }

    /// The unit direction toward the sun (the moon at night) this frame draws with, zero
    /// for none: `sunK`, moved by the day clock through [`VoxelRenderer::set_clock`]. The
    /// one sun path: anything keyed on the sun (shadows, volumetric light) reads this.
    pub fn sun_direction(&self) -> [f32; 3] {
        self.look.sun
    }

    /// Each emission curve's gain from this frame on, by curve index (`gains[0]` is
    /// ignored: curve 0 is always 1): what a style slot following that curve
    /// ([`VoxelStaging::emit_curves`]) draws its emissive colour times — the glowcap
    /// candle. A change redraws the lit tier while the drawn pack has a slot on a curve.
    pub fn set_emit_gains(&mut self, gains: &[f32]) {
        if gains != self.curve_gain.as_slice() {
            self.curve_gain.clear();
            self.curve_gain.extend_from_slice(gains);
            if self.params.lit && self.curving {
                self.raster_current = false;
                self.version += 1;
            }
        }
    }

    /// This frame's emitting cells (GLOW-EASE, [`EmitCell`]): each draws its own colour and
    /// on share instead of its style's emission, from this frame on, until the next call.
    /// A change redraws the lit tier. An empty list draws every emitter from its style.
    pub fn set_emit_cells(&mut self, cells: &[EmitCell]) {
        if cells != self.emit_cells.as_slice() {
            self.emit_cells.clear();
            self.emit_cells.extend_from_slice(cells);
            if self.params.lit {
                self.raster_current = false;
                self.version += 1;
            }
        }
    }

    /// Turn the picture round the ring by `x` raster columns: what was drawn at column `x`
    /// is drawn at column 0. The camera's pan, in whole pixels; 0 is the strip as it always
    /// was. A change redraws.
    pub fn set_view_x(&mut self, x: i32) {
        let rw = (self.params.width * self.params.s) as i32;
        let x = if rw > 0 { x.rem_euclid(rw) } else { 0 };
        if x != self.view_x {
            self.view_x = x;
            self.raster_current = false;
            self.version += 1;
        }
    }

    /// Retain the current atmosphere while replacing projection-dependent resources.
    pub fn inherit_atmosphere(&mut self, old: &Self) {
        self.weather = old.weather;
        self.weather_ease = old.weather_ease.clone();
        self.weather_at = old.weather_at;
        self.clock = old.clock;
        self.look = old.look;
        self.fog_floor = old.fog_floor;
        self.curve_gain.clone_from(&old.curve_gain);
        self.set_emit_cells(&old.emit_cells);
    }

    /// Vertical camera origin. Whole-voxel steps preserve bloom grid alignment;
    /// the viewer supplies the remaining smooth pixel offset.
    pub fn set_view_y(&mut self, y: i32, full_height: u32) {
        let y = y.div_euclid(self.params.s as i32) * self.params.s as i32;
        self.projection_height = full_height;
        if y != self.view_y {
            self.view_y = y;
            self.params.base = self.projection_base - y;
            self.raster_current = false;
            self.version += 1;
        }
    }

    pub fn view_y(&self) -> i32 {
        self.view_y
    }

    /// The view's turn round the ring ([`VoxelRenderer::set_view_x`]).
    pub fn view_x(&self) -> i32 {
        self.view_x
    }

    /// Draw one animal's cells leaning toward white (`None`: no highlight). A change
    /// redraws.
    pub fn set_highlight(&mut self, highlight: Option<Highlight>) {
        if highlight != self.highlight {
            self.highlight = highlight;
            self.raster_current = false;
            self.version += 1;
        }
    }

    /// Whether the world last staged holds any free water: the lit tier redraws on its
    /// clock only while it does.
    pub fn set_water_visible(&mut self, visible: bool) {
        self.water_visible = visible;
    }

    /// The light shafts' sun-visibility volume ([`crate::sunvis::SunVisVolume`]): whether
    /// it takes new data now, and how many bytes.
    pub fn sunvis_accepts(&self) -> Option<usize> {
        self.sunvis.accepts().then(|| self.sunvis.bytes())
    }

    /// New sun-visibility data (the bake fading out in red, in green the one fading in),
    /// starting at `fade`. False when refused ([`VoxelRenderer::sunvis_accepts`]).
    pub fn put_sunvis(&mut self, rg: &[u8], fade: f32) -> bool {
        let put = self.sunvis.put(rg, fade);
        if put {
            self.raster_current = false;
            self.version += 1;
        }
        put
    }

    /// Move the volume's fade; the picture redraws when the step shows.
    pub fn set_sunvis_fade(&mut self, fade: f32) {
        if self.sunvis.set_fade(fade) {
            self.raster_current = false;
            self.version += 1;
        }
    }

    /// Whether a pack has a staging buffer to go into. False while every one of them is
    /// being read by a frame that has not retired — the caller keeps the world owed and
    /// packs it next tick rather than doing the work and throwing it away.
    pub fn can_stage(&self) -> bool {
        self.ring.for_pack().is_some()
    }

    pub fn retire_frame(&mut self) {
        self.timing.retire();
        if let Some(frame) = self.ring.retire() {
            self.last_done = Some(frame.slot);
            self.sunvis.retired(frame.slot);
        }
    }

    /// The newest recorded frame was displaced before it was ever submitted. Its buffer
    /// comes back and its upload is owed again — without this the world it carried would
    /// never reach the GPU, because recording cleared the dirty flag.
    pub fn discard_frame(&mut self) {
        self.timing.discard();
        self.raster_current = false;
        if let Some(pl) = &mut self.plants {
            pl.frame_discarded();
        }
        let frame = self.ring.discard();
        if let Some(f) = frame {
            self.sunvis.discarded(f.slot);
            let (bytes, batches) = std::mem::take(&mut self.frame_uploads[f.slot]);
            self.upload_totals.0 -= bytes;
            self.upload_totals.1 -= batches;
        }
        if let Some(frame) = frame
            && frame.staging.is_some()
        {
            self.dirty = true;
            // Its slow planes never reached the images either, nor its bricks.
            self.slow.forget_images();
            self.owed
                .returned(&self.carried[frame.staging.expect("checked above")]);
            // Its upload never ran, so the raster is not what that frame would have made
            // it: the next frame must draw the world again.
            self.raster_current = false;
        }
    }

    /// What the last pack will upload when its frame runs, bytes.
    pub fn last_upload_bytes(&self) -> usize {
        self.last_upload
    }

    /// Cumulative image-upload bytes and staged batches recorded for submission.
    /// Overwritten packs and discarded records contribute nothing. Uniforms and model
    /// buffers are separate, as in [`Self::last_upload_bytes`].
    pub fn upload_totals(&self) -> (u64, u64) {
        self.upload_totals
    }

    /// Whether the last recorded frame redrew the world raster.
    pub fn redrew_last(&self) -> bool {
        self.redrew
    }

    /// What a frame recorded now would show. See [`VoxelRenderer::version`].
    pub fn content_version(&self) -> u64 {
        self.version
    }

    /// Frames recorded and not yet retired.
    pub fn frames_in_flight(&self) -> usize {
        self.ring.in_flight()
    }

    pub fn plant_stats(&self) -> Option<crate::plants::PlantStats> {
        self.plants.as_ref().map(|p| p.stats())
    }

    /// The plant pass's drawn list and the clock it was last drawn at ([`crate::plants::PlantPass::drawn`]).
    pub fn plant_drawn(&self) -> Option<(&crate::plants::PlantList, f64)> {
        self.plants.as_ref().map(|p| p.drawn())
    }

    /// The figures the next frame draws (H2's model pass). A change redraws.
    pub fn set_figures(&mut self, frame: crate::figures::FigureFrame) {
        if frame != self.figure_frame {
            self.figure_frame = frame;
            self.raster_current = false;
            self.version += 1;
        }
    }

    /// Write model cells for the figures at `first` ([`crate::figures::FigurePass::put_models`]).
    pub fn put_figure_models(
        &mut self,
        first: usize,
        cells: &[crate::figures::FigureVoxel],
    ) -> bool {
        self.figures.put_models(first, cells)
    }

    /// Tiles the atlas had no room for, over the run.
    pub fn figures_dropped(&self) -> u64 {
        self.figures_dropped
    }

    /// The latest model pass's atlas occupancy and any quality fallback.
    pub fn figure_atlas_stats(&self) -> crate::figures::FigureAtlasStats {
        self.figures.atlas_stats()
    }
}
impl VoxelRenderer {
    pub fn new(gpu: &Gpu, params: VoxelParams, textures: &VoxelTextures) -> Result<Self> {
        params.validate()?;
        ensure!(
            (textures.s, textures.rise) == (params.s, params.rise),
            "face texture projection differs"
        );
        let p = &params;
        let raster_size = (p.raster_w, p.raster_h);
        let volume = (p.width, p.height, p.depth);
        let raster_format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let raster = plane(
            gpu,
            "voxel raster",
            (p.raster_w, p.raster_h, 1),
            wgpu::TextureDimension::D2,
            raster_format,
            true,
        );
        let backdrop = plane(
            gpu,
            "sky backdrop",
            if p.lit {
                (p.raster_w, p.raster_h, 1)
            } else {
                (1, 1, 1)
            },
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba16Float,
            true,
        );
        let dummy_colour = plane(
            gpu,
            "unused float input",
            (1, 1, 1),
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba16Float,
            false,
        );
        let dummy_depth = plane(
            gpu,
            "unused depth input",
            (1, 1, 1),
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Depth32Float,
            false,
        );
        let mut world = vec![];
        let mut add = |name, size, dimension, format| {
            world.push(plane(gpu, name, size, dimension, format, false))
        };
        add(
            "voxel cells",
            volume,
            wgpu::TextureDimension::D3,
            wgpu::TextureFormat::Rgba8Uint,
        );
        add(
            "roof",
            volume,
            wgpu::TextureDimension::D3,
            wgpu::TextureFormat::R8Uint,
        );
        add(
            "styles",
            (4, MAX_STYLES as u32, 1),
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba32Float,
        );
        add(
            "glyphs",
            (p.s, (p.s + p.rise) * MAX_GLYPHS as u32, 1),
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::R8Uint,
        );
        let (tw, th) = textures.atlas_size();
        add(
            "face atlas",
            (tw, th, 1),
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let (vw, vh) = VoxelTextures::vine_size(p.s);
        add(
            "vine atlas",
            (vw, vh, 1),
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        add(
            "sky visibility",
            if p.lit { volume } else { (1, 1, 1) },
            wgpu::TextureDimension::D3,
            wgpu::TextureFormat::R8Uint,
        );
        let (cw, ch) = p.canopy_extent();
        add(
            "canopy",
            (cw, ch, 1),
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba8Uint,
        );
        add(
            "glow",
            p.glow_extent(),
            wgpu::TextureDimension::D3,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let mut upload = gpu.encoder("world atlases");
        gpu.upload_texture(
            &mut upload,
            &world[4].texture,
            wgpu::Origin3d::ZERO,
            extent((tw, th, 1)),
            4,
            &textures.rgba,
        )?;
        gpu.upload_texture(
            &mut upload,
            &world[5].texture,
            wgpu::Origin3d::ZERO,
            extent((vw, vh, 1)),
            4,
            &textures.vine_rgba,
        )?;
        gpu.queue.submit([upload.finish()]);
        let sunvis = SunVisVolume::new(gpu, p.effects.volume_on(p.lit), p.width, p.height, p.depth);
        let mut entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<VoxelUniforms>() as u64),
            },
            count: None,
        }];
        for binding in 1..=18 {
            if binding == 14 {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                continue;
            }
            let sample_type = match binding {
                1 | 2 | 4 | 7 | 8 => wgpu::TextureSampleType::Uint,
                13 | 16 => wgpu::TextureSampleType::Depth,
                9 | 10 => wgpu::TextureSampleType::Float { filterable: true },
                _ => wgpu::TextureSampleType::Float { filterable: false },
            };
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type,
                    view_dimension: if matches!(binding, 1 | 2 | 7 | 9 | 10) {
                        wgpu::TextureViewDimension::D3
                    } else {
                        wgpu::TextureViewDimension::D2
                    },
                    multisampled: false,
                },
                count: None,
            });
        }
        for (binding, kind) in [
            (32, wgpu::SamplerBindingType::NonFiltering),
            (33, wgpu::SamplerBindingType::Filtering),
        ] {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(kind),
                count: None,
            });
        }
        let world_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("world ABI"),
                entries: &entries,
            });
        let nearest = gpu
            .device
            .create_sampler(&wgpu::SamplerDescriptor::default());
        let linear = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mut spec = Specialization([false; 12]);
        for (i, &v) in p.effects.spec_constants(p.lit).iter().enumerate() {
            spec.0[i] = v != 0;
        }
        spec.0[10] = p.plants.is_some();
        spec.0[11] = true;
        let bloom = p.lit.then(|| Bloom::new(gpu, p)).transpose()?;
        let mut figures = Figures::new(
            gpu,
            &world_layout,
            raster_size,
            volume,
            raster_format,
            bloom.as_ref().map(|_| raster_format),
            p.lit,
            spec,
        )?;
        let plants = match p.plants {
            Some(motion) => {
                let plants = Plants::new(
                    gpu,
                    &world_layout,
                    raster_size,
                    volume,
                    raster_format,
                    bloom.as_ref().map(|_| raster_format),
                    p.lit,
                    spec,
                    motion,
                    &figures.walk_depth.view,
                    STAGING_RING,
                )?;
                figures.set_occluder(gpu, &plants.occluder.view);
                Some(plants)
            }
            None => None,
        };
        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("world passes"),
                bind_group_layouts: &[Some(&world_layout)],
                immediate_size: 0,
            });
        let targets = if p.lit {
            vec![
                Some(raster_format),
                Some(raster_format),
                Some(wgpu::TextureFormat::R32Float),
            ]
        } else {
            vec![
                Some(raster_format),
                None,
                Some(wgpu::TextureFormat::R32Float),
            ]
        };
        let mut walk = vec![];
        for plant_light in [false, true] {
            for weather in [false, true] {
                for pose in [false, true] {
                    let mut flags = spec;
                    flags.0[8] = weather;
                    flags.0[9] = pose;
                    flags.0[11] = plant_light;
                    walk.push(fullscreen(
                        gpu,
                        &layout,
                        Shader::Voxel,
                        flags,
                        &targets,
                        false,
                    )?);
                }
            }
        }
        let sky_pipeline = if p.lit {
            let mut flags = spec;
            flags.0[6] = true;
            Some(fullscreen(
                gpu,
                &layout,
                Shader::Voxel,
                flags,
                &[Some(wgpu::TextureFormat::Rgba16Float)],
                false,
            )?)
        } else {
            None
        };
        let weather_pipeline = if p.lit {
            let mut flags = spec;
            flags.0[7] = true;
            Some(fullscreen(
                gpu,
                &layout,
                Shader::Weather,
                flags,
                &[Some(raster_format)],
                true,
            )?)
        } else {
            None
        };
        let staging = (0..STAGING_RING)
            .map(|_| Staging {
                voxels: vec![VoxelTexel::default(); p.voxel_count()],
                roof: vec![0; p.voxel_count()],
                styles: vec![VoxelStyle::default(); MAX_STYLES],
                glyphs: vec![0; p.glyph_bytes()],
                sky: vec![0; p.sky_bytes()],
                canopy: vec![0; p.canopy_bytes()],
                glow: vec![0; p.glow_bytes()],
            })
            .collect();
        Ok(Self {
            closed: false,
            timing: Default::default(),
            params,
            raster,
            backdrop,
            dummy_colour,
            dummy_depth,
            world,
            tex_mask: textures.mask(),
            vine_on: textures.vine_on,
            sunvis,
            bloom,
            figures,
            plants,
            world_layout,
            walk,
            sky_pipeline,
            weather_pipeline,
            nearest,
            linear,
            present: HashMap::new(),
            staging,
            ring: StagingRing::new(STAGING_RING),
            slow: SlowPlanes::new(STAGING_RING),
            owed: BrickOwed::new(p.width, p.height, p.depth),
            carried: vec![Carried::default(); STAGING_RING],
            last_upload: 0,
            upload_totals: (0, 0),
            frame_uploads: vec![(0, 0); STAGING_RING],
            dirty: false,
            raster_current: false,
            staged: false,
            pose_tables: vec![PoseTable::EMPTY; STAGING_RING],
            drawn_poses: PoseTable::EMPTY,
            posing: false,
            emit_curve_tables: vec![[0; MAX_STYLES]; STAGING_RING],
            drawn_emit_curves: [0; MAX_STYLES],
            curving: false,
            curve_gain: vec![],
            emit_cells: vec![],
            emit_scratch: vec![],
            clock: FrameClock::default(),
            weather: crate::weather::Weather::CLEAR_NOON,
            weather_ease: Default::default(),
            weather_at: 0.0,
            look: crate::weather::noon(p),
            fog_floor: p.height as f32 * 0.25,
            water_visible: false,
            version: 0,
            redrew: true,
            view_x: 0,
            view_y: 0,
            projection_base: p.base,
            projection_height: p.raster_h,
            highlight: None,
            last_done: None,
            figure_frame: Default::default(),
            figures_dropped: 0,
        })
    }
    pub fn params(&self) -> VoxelParams {
        self.params
    }
    pub fn stage(&mut self, keys: [u64; SLOW_PLANES], fill: impl FnOnce(VoxelStaging<'_>)) -> bool {
        let Some(into) = self.ring.for_pack() else {
            return false;
        };
        let write = self.slow.pack(into, keys);
        let b = &mut self.staging[into];
        self.pose_tables[into] = PoseTable::EMPTY;
        self.emit_curve_tables[into].fill(0);
        self.carried[into] = Carried::default();
        let mut styles_key = 0;
        fill(VoxelStaging {
            plants: self.plants.as_mut().map(|p| p.staging(into)),
            voxels: &mut b.voxels,
            roof: &mut b.roof,
            styles: &mut b.styles,
            glyphs: &mut b.glyphs,
            sky: &mut b.sky,
            canopy: &mut b.canopy,
            glow: &mut b.glow,
            poses: &mut self.pose_tables[into],
            emit_curves: &mut self.emit_curve_tables[into],
            write,
            owed: &mut self.owed,
            carried: &mut self.carried[into],
            styles_key: &mut styles_key,
        });
        self.slow.restamp(into, PLANE_STYLES, styles_key);
        self.last_upload = image_upload_bytes(
            &self.params,
            &self.carried[into],
            self.slow.would_upload(into),
        );
        self.ring.packed(into);
        self.dirty = true;
        self.staged = true;
        self.raster_current = false;
        self.version += 1;
        true
    }
}
fn extent(size: (u32, u32, u32)) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: size.0,
        height: size.1,
        depth_or_array_layers: size.2,
    }
}
fn plane(
    gpu: &Gpu,
    name: &str,
    size: (u32, u32, u32),
    dimension: wgpu::TextureDimension,
    format: wgpu::TextureFormat,
    render: bool,
) -> Plane {
    let texture = gpu.texture(
        name,
        extent(size),
        dimension,
        format,
        wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | if render {
                wgpu::TextureUsages::RENDER_ATTACHMENT
            } else {
                wgpu::TextureUsages::empty()
            },
    );
    let view = texture.create_view(&Default::default());
    Plane { texture, view }
}
fn fullscreen(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    shader: Shader,
    spec: Specialization,
    formats: &[Option<wgpu::TextureFormat>],
    blend: bool,
) -> Result<wgpu::RenderPipeline> {
    let vertex = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fullscreen"),
            source: wgpu::ShaderSource::Wgsl(Shader::Fullscreen.runtime_source().into()),
        });
    let source = shader.runtime_source();
    let module = naga::front::wgsl::parse_str(source)?;
    let owned_constants = constants(&module, spec);
    let constants: Vec<_> = owned_constants
        .iter()
        .map(|(k, v)| (k.as_str(), *v))
        .collect();
    let fragment = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(shader.name()),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let targets: Vec<_> = formats
        .iter()
        .map(|&format| {
            format.map(|format| wgpu::ColorTargetState {
                format,
                blend: blend.then_some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })
        })
        .collect();
    Ok(gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(shader.name()),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &vertex,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &constants,
                    ..Default::default()
                },
                targets: &targets,
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        }))
}

impl VoxelRenderer {
    pub fn record(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: Option<TargetSlot<'_>>,
    ) -> Result<()> {
        let before = self.ring.in_flight();
        let result = self.record_inner(gpu, encoder, target);
        if result.is_err() && self.ring.in_flight() > before {
            self.discard_frame();
        }
        result
    }
    fn record_inner(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: Option<TargetSlot<'_>>,
    ) -> Result<()> {
        ensure!(!self.closed, "voxel renderer is destroyed");
        ensure!(self.staged, "VoxelRenderer::record before first stage");
        let timing = self.timing.begin(gpu, 12);
        if let Some(t) = &timing {
            t.stamp(encoder, 0);
        }
        let (w, h, dd) = (self.params.width, self.params.height, self.params.depth);
        // Whatever was packed is this frame's to upload, and its buffer is this frame's
        // until it retires. A frame that uploads nothing still takes a place in the
        // ring's order, so retirements and frames stay one to one.
        let frame = self.ring.record();
        let upload = self.dirty && frame.staging.is_some();
        self.dirty = false;
        // Which of the roof and the atlas this upload carries: only the ones the images
        // do not already hold.
        let slow = match frame.staging {
            Some(i) if upload => self.slow.upload(i),
            _ => [false; SLOW_PLANES],
        };
        if upload && let Some(i) = frame.staging {
            self.owed.taken(&self.carried[i]);
        }
        let bytes = frame.staging.filter(|_| upload).map_or(0, |i| {
            image_upload_bytes(&self.params, &self.carried[i], slow) as u64
        });
        let batches = u64::from(upload);
        self.frame_uploads[frame.slot] = (bytes, batches);
        self.upload_totals.0 += bytes;
        self.upload_totals.1 += batches;
        // The raster already holds this picture unless something changed it, and the
        // upload only ever comes with a change.
        let redraw = !self.raster_current || self.sunvis.owed();
        self.raster_current = true;
        self.redrew = redraw;
        // This frame's own uniform block, written now and read by the GPU when it runs:
        // `set_params` and `update_weather` only moved `self.params`.
        if upload && let Some(i) = frame.staging {
            if let Some(p) = &mut self.plants {
                p.adopt(i);
            }
            self.drawn_poses = self.pose_tables[i];
            self.posing = self.drawn_poses.posing();
            self.drawn_emit_curves = self.emit_curve_tables[i];
            self.curving = self.drawn_emit_curves.iter().any(|&c| c != 0);
        }
        let mut block = self
            .params
            .uniforms(self.tex_mask, self.vine_on, self.clock, &self.look);
        if self.posing {
            block.poses = pose_rows(&self.drawn_poses, self.clock.time, self.params.s);
        }
        block.vol[3] = self.sunvis.fade();
        block.view[0] = self.view_x;
        block.camera = [self.view_y, self.projection_height as i32, 0, 0];
        if let Some(h) = &self.highlight {
            let (lo, hi, count, cells) = h.block(self.params.width);
            block.hi_min = lo;
            block.hi_max = hi;
            block.view[1] = count;
            block.hi_k = [h.mix, 0.0, 0.0, 0.0];
            block.hi_cells = cells;
        }
        block.fog_k[1] = self.fog_floor;
        // The emitter table into this frame's own region (GLOW-EASE).
        let cap = emit_table(&self.emit_cells, &mut self.emit_scratch);
        if cap > 0 {}
        block.view[2] = 0;
        block.view[3] = emit_view(cap, &self.emit_cells) as i32;
        // The figures go into this slot's buffer now: the sun map is drawn before the walk.
        let p = &self.params;
        let sun_map = if redraw {
            self.figures_dropped += self.figures.prepare(
                gpu,
                &self.figure_frame,
                p.s,
                p.rise,
                p.base,
                self.view_x,
                p.width * p.s,
            ) as u64;
            (p.lit && self.figures.has_tiles())
                .then(|| crate::figures::sun_map(p.width, p.height, p.depth, self.look.sun))
                .flatten()
        } else {
            None
        };
        if let Some((fs, fm)) = sun_map {
            block.fig_sun = fs;
            block.fig_map = fm;
        }
        if self.curving {
            let gain = |c: u8| match c {
                0 => 1.0,
                c => self.curve_gain.get(usize::from(c)).copied().unwrap_or(1.0),
            };
            for (row, curves) in block
                .emit_k
                .iter_mut()
                .zip(self.drawn_emit_curves.chunks_exact(4))
            {
                for (g, &c) in row.iter_mut().zip(curves) {
                    *g = gain(c);
                }
            }
        }
        // Never sample a skipped figure pass's stale depth/coverage (including the first
        // frame), or the flat tier's dummy descriptors. Plant shade writes every pixel.
        block.camera[2] = i32::from(self.params.lit && self.plants.is_some())
            | (i32::from(self.params.lit && self.figures.drawing()) << 1);

        if let Some(pl) = &mut self.plants {
            pl.frame_recorded(redraw);
        }
        if upload && redraw {
            let i = frame.staging.unwrap();
            let b = &self.staging[i];
            let carried = &self.carried[i];
            if carried.full {
                gpu.upload_texture(
                    encoder,
                    &self.world[0].texture,
                    wgpu::Origin3d::ZERO,
                    extent((w, h, dd)),
                    4,
                    bytemuck::cast_slice(&b.voxels[..self.params.voxel_count()]),
                )?;
            } else {
                let (bw, bh, _) = brick_dims(w, h, dd);
                for (k, &brick) in carried.bricks.iter().enumerate() {
                    let (x, y, z) = (
                        (brick % bw) * BRICK,
                        ((brick / bw) % bh) * BRICK,
                        (brick / (bw * bh)) * BRICK,
                    );
                    let size = ((w - x).min(BRICK), (h - y).min(BRICK), (dd - z).min(BRICK));
                    let mut texels = Vec::with_capacity((size.0 * size.1 * size.2) as usize);
                    let slot = &b.voxels[k * BRICK_TEXELS..(k + 1) * BRICK_TEXELS];
                    for dz in 0..size.2 {
                        for dy in 0..size.1 {
                            let start = ((dz * BRICK + dy) * BRICK) as usize;
                            texels.extend_from_slice(&slot[start..start + size.0 as usize]);
                        }
                    }
                    gpu.upload_texture(
                        encoder,
                        &self.world[0].texture,
                        wgpu::Origin3d { x, y, z },
                        extent(size),
                        4,
                        bytemuck::cast_slice(&texels),
                    )?;
                }
            }
            let (cw, ch) = self.params.canopy_extent();
            let planes: [(bool, usize, (u32, u32, u32), u32, &[u8]); 6] = [
                (slow[PLANE_ROOF], 1, (w, h, dd), 1, &b.roof),
                (
                    slow[PLANE_STYLES],
                    2,
                    (4, MAX_STYLES as u32, 1),
                    16,
                    bytemuck::cast_slice(&b.styles),
                ),
                (
                    slow[PLANE_GLYPHS],
                    3,
                    (
                        self.params.s,
                        (self.params.s + self.params.rise) * MAX_GLYPHS as u32,
                        1,
                    ),
                    1,
                    &b.glyphs,
                ),
                (self.params.lit && slow[PLANE_SKY], 6, (w, h, dd), 1, &b.sky),
                (
                    self.params.lit && slow[PLANE_CANOPY],
                    7,
                    (cw, ch, 1),
                    4,
                    &b.canopy,
                ),
                (
                    self.params.lit && slow[PLANE_GLOW],
                    8,
                    self.params.glow_extent(),
                    4,
                    &b.glow,
                ),
            ];
            for (on, i, size, bytes, data) in planes {
                if on {
                    gpu.upload_texture(
                        encoder,
                        &self.world[i].texture,
                        wgpu::Origin3d::ZERO,
                        extent(size),
                        bytes,
                        data,
                    )?;
                }
            }
        }
        if redraw {
            self.sunvis.record_upload(gpu, encoder, frame.slot)?;
            if let Some(p) = &mut self.plants {
                p.prepare(gpu);
            }
        }
        if let Some(t) = &timing {
            t.stamp(encoder, 1);
        }
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("voxel frame uniforms"),
                contents: bytemuck::bytes_of(&block),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let empty = [[0u32; 4]];
        let emit = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("voxel frame emitter cells"),
                contents: bytemuck::cast_slice(if self.emit_scratch.is_empty() {
                    &empty
                } else {
                    &self.emit_scratch
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }];
        for (i, plane) in self.world.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: i as u32 + 1,
                resource: wgpu::BindingResource::TextureView(&plane.view),
            });
        }
        let plant_z = self
            .plants
            .as_ref()
            .map_or(&self.figures.walk_depth.view, |p| &p.occluder.view);
        let plant_surface = self
            .plants
            .as_ref()
            .and_then(|p| p.weather_surface.as_ref())
            .map_or(&self.backdrop.view, |p| &p.view);
        let figure_surface = self
            .figures
            .weather_surface
            .as_ref()
            .map_or(&self.backdrop.view, |p| &p.view);
        for (binding, view) in [
            (10, &self.sunvis.view),
            (11, &self.backdrop.view),
            (12, &self.figures.walk_depth.view),
            (13, &self.figures.sun.view),
            (15, plant_z),
            (16, &self.figures.comp_depth.view),
            (17, plant_surface),
            (18, figure_surface),
        ] {
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(view),
            });
        }
        entries.extend([
            wgpu::BindGroupEntry {
                binding: 14,
                resource: emit.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 32,
                resource: wgpu::BindingResource::Sampler(&self.nearest),
            },
            wgpu::BindGroupEntry {
                binding: 33,
                resource: wgpu::BindingResource::Sampler(&self.linear),
            },
        ]);
        let world_group = |excluded: &[u32]| {
            let entries: Vec<_> = entries
                .iter()
                .map(|entry| wgpu::BindGroupEntry {
                    binding: entry.binding,
                    resource: if excluded.contains(&entry.binding) {
                        wgpu::BindingResource::TextureView(if matches!(entry.binding, 13 | 16) {
                            &self.dummy_depth.view
                        } else {
                            &self.dummy_colour.view
                        })
                    } else {
                        entry.resource.clone()
                    },
                })
                .collect();
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pass world inputs"),
                layout: &self.world_layout,
                entries: &entries,
            })
        };
        let world = world_group(&[]);
        let sky_world = world_group(&[11, 17, 18]);
        let sun_world = world_group(&[13]);
        // Without model plants, binding 15 falls back to walk_depth too. Both
        // descriptors must be replaced while that texture is a colour attachment.
        let walk_world = world_group(&[12, 15]);
        let plant_world = world_group(&[15, 17]);
        let figure_world = world_group(&[16, 18]);
        if redraw {
            if let Some(pipeline) = &self.sky_pipeline {
                draw_fullscreen(
                    encoder,
                    pipeline,
                    &sky_world,
                    &[colour(&self.backdrop.view, true)],
                );
            }
            self.figures.record_sun(encoder, &sun_world, 0, sun_map);
            if let Some(t) = &timing {
                t.stamp(encoder, 8);
            }
            let weather =
                self.weather_pipeline.is_some() && weather_drawn(&self.look, &self.params);
            let plant_light = block.view[3] as u32 & EMIT_PLANTS != 0;
            let index =
                4 * usize::from(plant_light) + 2 * usize::from(weather) + usize::from(self.posing);
            let emission = self.bloom.as_ref().map(|b| &b.emission);
            let mut attachments = vec![colour(&self.raster.view, true)];
            attachments.push(emission.and_then(|v| colour(v, true)));
            attachments.push(colour(&self.figures.walk_depth.view, true));
            draw_fullscreen(encoder, &self.walk[index], &walk_world, &attachments);
            if let Some(t) = &timing {
                t.stamp(encoder, 4);
            }
            let p = &self.params;
            let geom = (p.s, p.rise, p.base);
            if let Some(pl) = &self.plants {
                pl.record_vis(
                    encoder,
                    &world,
                    0,
                    geom,
                    self.view_x,
                    p.drape_cords.reach(p.s),
                );
                if let Some(t) = &timing {
                    t.stamp(encoder, 6);
                    t.stamp(encoder, 9);
                }
                pl.record_shade(
                    encoder,
                    &plant_world,
                    0,
                    &self.raster.view,
                    emission,
                    geom,
                    self.view_x,
                );
            }
            if let Some(t) = &timing {
                if self.plants.is_none() {
                    t.stamp(encoder, 6);
                    t.stamp(encoder, 9);
                }
                t.stamp(encoder, 7);
            }
            self.figures.record(
                encoder,
                &figure_world,
                0,
                &self.raster.view,
                emission,
                p.s,
                p.rise,
            );
            if let Some(t) = &timing {
                t.stamp(encoder, 5);
                t.stamp(encoder, 10);
            }
            if weather {
                draw_fullscreen(
                    encoder,
                    self.weather_pipeline.as_ref().unwrap(),
                    &world,
                    &[colour(&self.raster.view, false)],
                );
            }
            if let Some(t) = &timing {
                t.stamp(encoder, 11);
            }
            if let Some(b) = &self.bloom {
                if p.bloom > 0.0 {
                    b.record(encoder, p, &self.raster.view);
                }
            }
        }
        if let Some(t) = &timing {
            if !redraw {
                for i in 4..12 {
                    t.stamp(encoder, i);
                }
            }
            t.stamp(encoder, 2);
        }
        if let Some(target) = target {
            self.present
                .entry(target.format)
                .or_insert_with(|| PresentPass::new(gpu, target.format))
                .record(
                    gpu,
                    encoder,
                    &self.raster.view,
                    target.view,
                    target.transform,
                );
        }
        if let Some(t) = timing {
            t.stamp(encoder, 3);
            self.timing.finish(encoder, t);
        }
        Ok(())
    }
    pub fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>> {
        super::headless::read_texture(gpu, &self.raster.texture)
    }
    pub fn destroy(&mut self, _gpu: &Gpu) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.raster.texture.destroy();
        self.backdrop.texture.destroy();
        self.dummy_colour.texture.destroy();
        self.dummy_depth.texture.destroy();
        for plane in &self.world {
            plane.texture.destroy();
        }
        self.sunvis.texture.destroy();
        if let Some(b) = &mut self.bloom {
            b.destroy();
        }
        self.figures.destroy();
        if let Some(p) = &mut self.plants {
            p.destroy();
        }
        self.present.clear();
        self.walk.clear();
        self.sky_pipeline = None;
        self.weather_pipeline = None;
        self.staging.clear();
    }
}
fn colour(view: &wgpu::TextureView, clear: bool) -> Option<wgpu::RenderPassColorAttachment<'_>> {
    Some(wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: if clear {
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
            } else {
                wgpu::LoadOp::Load
            },
            store: wgpu::StoreOp::Store,
        },
    })
}
fn draw_fullscreen(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    world: &wgpu::BindGroup,
    attachments: &[Option<wgpu::RenderPassColorAttachment<'_>>],
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("voxel fullscreen"),
        color_attachments: attachments,
        depth_stencil_attachment: None,
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, world, &[0]);
    pass.draw(0..3, 0..1);
}
impl FrameSource for VoxelRenderer {
    type Frame<'a> = ();
    fn raster_size(&self) -> (u32, u32) {
        (self.params.raster_w, self.params.raster_h)
    }
    fn raster_texture(&self) -> &wgpu::Texture {
        &self.raster.texture
    }
    fn raster_view(&self) -> &wgpu::TextureView {
        &self.raster.view
    }
    fn record_frame(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        _frame: (),
        target: Option<TargetSlot<'_>>,
    ) -> Result<()> {
        self.record(gpu, encoder, target)
    }
    fn frame_retired(&mut self) {
        self.retire_frame();
        if let Some(pl) = &mut self.plants {
            pl.frame_retired();
        }
    }
    fn frame_discarded(&mut self) {
        self.discard_frame();
    }
    fn frames_in_flight(&self) -> usize {
        self.frames_in_flight()
    }
    fn frame_capacity(&self) -> usize {
        STAGING_RING
    }
    fn gpu_ms(&self, gpu: &Gpu) -> f64 {
        self.gpu_split(gpu).map_or(f64::NAN, |t| t.iter().sum())
    }
    fn redrew_last(&self) -> bool {
        self.redrew
    }
    fn content_version(&self) -> Option<u64> {
        Some(self.version)
    }
}

impl VoxelRenderer {
    pub fn gpu_split(&self, gpu: &Gpu) -> Option<[f64; 3]> {
        let t = self.timing.stamps()?;
        let ns = f64::from(gpu.queue.get_timestamp_period()) / 1e6;
        Some([
            t[1].wrapping_sub(t[0]) as f64 * ns,
            t[2].wrapping_sub(t[1]) as f64 * ns,
            t[3].wrapping_sub(t[2]) as f64 * ns,
        ])
    }
    pub fn figure_gpu_ms(&self, gpu: &Gpu) -> Option<f64> {
        let t = self.timing.stamps()?;
        Some(t[5].wrapping_sub(t[4]) as f64 * f64::from(gpu.queue.get_timestamp_period()) / 1e6)
    }
    pub fn weather_gpu_ms(&self, gpu: &Gpu) -> Option<f64> {
        let t = self.timing.stamps()?;
        Some(t[11].wrapping_sub(t[10]) as f64 * f64::from(gpu.queue.get_timestamp_period()) / 1e6)
    }
    pub fn walk_plant_gpu_ms(&self, gpu: &Gpu) -> Option<(f64, f64)> {
        let t = self.timing.stamps()?;
        let ns = f64::from(gpu.queue.get_timestamp_period()) / 1e6;
        Some((
            t[4].wrapping_sub(t[8]) as f64 * ns,
            if self.plants.is_some() {
                (t[6].wrapping_sub(t[4]) + t[7].wrapping_sub(t[9])) as f64 * ns
            } else {
                0.0
            },
        ))
    }
}
