//! The pack oracle: the change-only pack held to the full pack, tick by tick.
//!
//! Two packers see the same world: one packs in full every tick (today's pack, the
//! oracle), the other packs only what changed, through a staging ring, owed bricks and
//! slow-plane keys emulated on the CPU exactly as `VoxelRenderer` drives them. After every
//! pack the change-only packer's texels must draw what the full packer's draw (each texel
//! with its style resolved, never raw slot numbers), and after every upload the emulated
//! images must hold exactly what that pack held. The unit tests drive it on scripted
//! fixtures; `examples/pack_oracle.rs` drives it on saved worlds with the sim advancing.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use cubarium_gpu::voxel::{
    BrickOwed, Carried, MAX_STYLES, PLANE_CANOPY, PLANE_GLOW, PLANE_GLYPHS, PLANE_ROOF,
    PLANE_SKY, PLANE_STYLES, PoseTable, Recorded, SLOW_PLANES, SlowPlanes, StagingRing,
    VoxelParams, VoxelStaging, VoxelStyle, VoxelTexel, apply_upload,
};
use cubarium_voxel::VoxelView;
use cubarium_voxel_fauna::FaunaView;
use cubarium_voxel_flora::{FaceDraw, FloraView};

use super::{Emission, Packer, params_of};
use crate::voxel::VoxelConfig;
use crate::voxel::model::ModelLibrary;
use crate::voxel::project::Projection;
use crate::voxel::textures::SpeciesFaces;

/// One buffer's planes, or the images'.
#[derive(Clone)]
struct Planes {
    voxels: Vec<VoxelTexel>,
    roof: Vec<u8>,
    styles: Vec<VoxelStyle>,
    glyphs: Vec<u8>,
    sky: Vec<u8>,
    canopy: Vec<u8>,
    glow: Vec<u8>,
    poses: PoseTable,
    emit_curves: [u8; MAX_STYLES],
}

impl Planes {
    /// Filled with junk, so a plane a pack should have written and did not shows.
    fn junk(p: &VoxelParams) -> Planes {
        let lit = |n: usize| if p.lit { vec![0xAB; n] } else { Vec::new() };
        Planes {
            voxels: vec![VoxelTexel([0xAB; 4]); p.voxel_count()],
            roof: vec![0xAB; p.voxel_count()],
            styles: vec![VoxelStyle::new([0.5; 3], [0.5; 3], [0.5; 3]); MAX_STYLES],
            glyphs: vec![0xAB; p.glyph_bytes()],
            sky: lit(p.sky_bytes()),
            canopy: lit(p.canopy_bytes()),
            glow: lit(p.glow_bytes()),
            poses: PoseTable::EMPTY,
            emit_curves: [0; MAX_STYLES],
        }
    }

    /// As `VoxelRenderer::stage` hands a buffer over: the pose table empty and the
    /// emission curves zero; every other plane holds what it held.
    fn staging<'a>(
        &'a mut self,
        write: [bool; SLOW_PLANES],
        owed: &'a mut BrickOwed,
        carried: &'a mut Carried,
        styles_key: &'a mut u64,
    ) -> VoxelStaging<'a> {
        self.poses = PoseTable::EMPTY;
        self.emit_curves = [0; MAX_STYLES];
        *carried = Carried::default();
        VoxelStaging {
            voxels: &mut self.voxels,
            roof: &mut self.roof,
            styles: &mut self.styles,
            glyphs: &mut self.glyphs,
            sky: &mut self.sky,
            canopy: &mut self.canopy,
            glow: &mut self.glow,
            poses: &mut self.poses,
            emit_curves: &mut self.emit_curves,
            write,
            owed,
            carried,
            styles_key,
        }
    }
}

/// What a pack left in its buffer, for the frame that uploads it to be held to.
struct Expect {
    voxels: Vec<VoxelTexel>,
    styles: Vec<VoxelStyle>,
    emit_curves: [u8; MAX_STYLES],
    planes: [Vec<u8>; SLOW_PLANES],
}

/// How the tick's frame goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// Recorded and run at once.
    Now,
    /// Recorded, and still on the GPU while the next tick packs into the other buffer.
    InFlight,
    /// Not recorded: the next pack overwrites this one before any frame takes it.
    Unrecorded,
}

/// What one tick packed and uploaded.
#[derive(Clone, Debug, Default)]
pub struct TickReport {
    /// Bricks the change-only pack carried, or `None` when it carried the whole volume.
    pub bricks: Option<usize>,
    /// Which slow planes the tick's frame copied, if it recorded one.
    pub copied: [bool; SLOW_PLANES],
    /// The two packs' CPU time, ms.
    pub full_ms: f64,
    pub delta_ms: f64,
}

#[doc(hidden)]
pub struct PackOracle {
    params: VoxelParams,
    full: Packer,
    delta: Packer,
    full_planes: Planes,
    full_owed: BrickOwed,
    full_carried: Carried,
    ring: StagingRing,
    slow: SlowPlanes,
    owed: BrickOwed,
    buffers: Vec<Planes>,
    carried: Vec<Carried>,
    /// What the latest pack held, for the images to be held to.
    latest: Option<Expect>,
    pending: VecDeque<(Recorded, [bool; SLOW_PLANES])>,
    images: Planes,
    clock: Instant,
    ticks: u64,
}

impl PackOracle {
    pub(crate) fn new(
        params: VoxelParams,
        models: Option<std::sync::Arc<ModelLibrary>>,
        tiles: bool,
        species: SpeciesFaces,
        emission: Emission,
    ) -> PackOracle {
        let n = cubarium_gpu::voxel::STAGING_RING;
        let (w, h, d) = (params.width, params.height, params.depth);
        PackOracle {
            full: Packer::new(&params, models.clone(), tiles, species.clone(), emission).full_only(),
            delta: Packer::new(&params, models, tiles, species, emission),
            full_planes: Planes::junk(&params),
            full_owed: BrickOwed::new(w, h, d),
            full_carried: Carried::default(),
            ring: StagingRing::new(n),
            slow: SlowPlanes::new(n),
            owed: BrickOwed::new(w, h, d),
            buffers: vec![Planes::junk(&params); n],
            carried: vec![Carried::default(); n],
            latest: None,
            pending: VecDeque::new(),
            images: Planes::junk(&params),
            clock: Instant::now(),
            ticks: 0,
            params,
        }
    }

    /// The oracle a [`super::VoxelGpuSink`] for `cfg` would pack with: its models, its
    /// vine tiles (`textures_under` as the sink's option), its emission.
    pub fn for_config(
        cfg: &VoxelConfig,
        proj: Projection,
        models: Option<std::sync::Arc<ModelLibrary>>,
        textures_under: Option<&std::path::Path>,
    ) -> PackOracle {
        let params = params_of(cfg, proj, false);
        let textures = super::load_textures(cfg, textures_under, &params);
        PackOracle::new(
            params,
            models,
            textures.vine_on,
            SpeciesFaces::of(&textures),
            Emission::of(cfg),
        )
    }

    pub fn params(&self) -> VoxelParams {
        self.params
    }

    /// Take every free style slot of the change-only pack with a style no texel draws
    /// with; returns them for [`PackOracle::empty_palette`]. For the overflow test.
    #[cfg(test)]
    pub(crate) fn fill_palette(&mut self) -> Vec<u8> {
        self.delta.fill_palette()
    }

    /// Give back the slots [`PackOracle::fill_palette`] took.
    #[cfg(test)]
    pub(crate) fn empty_palette(&mut self, held: Vec<u8>) {
        self.delta.empty_palette(held);
    }

    /// Styles the change-only pack has given a slot since it was made.
    pub fn styles_made(&self) -> u64 {
        self.delta.styles_made()
    }

    /// Faces to draw instead of the flora's cover, in both packs.
    pub fn set_cover_draws(&mut self, draws: Option<Vec<FaceDraw>>) {
        for p in [&mut self.full, &mut self.delta] {
            p.stands.set_cover_draws(draws.clone());
            p.cover_override = draws.clone();
        }
    }

    /// Whether either pack's sky plane is still being computed for its terrain.
    pub fn light_pending(&self) -> bool {
        self.full.light_pending() || self.delta.light_pending()
    }

    /// Pack one tick both ways, hold the change-only pack to the full one, and go on with
    /// the frame as `frame` says. The film marks ease on a clock of 50 ms a tick, the same
    /// for both packs.
    pub fn tick(
        &mut self,
        view: &VoxelView<'_>,
        flora: FloraView<'_>,
        fauna: FaunaView<'_>,
        candle_s: f64,
        frame: Frame,
    ) -> Result<TickReport, String> {
        let Some(mut report) = self.pack(view, flora, fauna, candle_s)? else {
            return Err("no staging buffer free".into());
        };
        // The frame still on the GPU from the tick before finishes now.
        while !self.pending.is_empty() {
            self.retire();
        }
        match frame {
            Frame::Now => {
                report.copied = self.record();
                self.retire();
            }
            Frame::InFlight => report.copied = self.record(),
            Frame::Unrecorded => {}
        }
        self.check_images()?;
        Ok(report)
    }

    /// A tick as a presenter can interleave it, drawn from `rng`: a pack (refused when
    /// every buffer is still read), then up to three of record (while a frame slot is
    /// free), retire (oldest first) and displace (the newest, before it ran). Returns the
    /// pack's report, or `None` for a refused pack.
    pub fn tick_random(
        &mut self,
        view: &VoxelView<'_>,
        flora: FloraView<'_>,
        fauna: FaunaView<'_>,
        candle_s: f64,
        rng: &mut u64,
    ) -> Result<Option<TickReport>, String> {
        let mut next = || {
            *rng ^= *rng << 13;
            *rng ^= *rng >> 7;
            *rng ^= *rng << 17;
            *rng
        };
        let report = self.pack(view, flora, fauna, candle_s)?;
        for _ in 0..next() % 4 {
            match next() % 8 {
                0..=3 if self.pending.len() < cubarium_gpu::voxel::STAGING_RING => {
                    self.record();
                }
                4..=6 if !self.pending.is_empty() => self.retire(),
                7 if !self.pending.is_empty() => self.discard(),
                _ => {}
            }
        }
        self.check_images()?;
        Ok(report)
    }

    /// Pack one tick both ways and hold the change-only pack to the full one. `None` when
    /// every staging buffer is still read: the pack is refused, as the renderer refuses it,
    /// and neither packer sees the tick.
    pub(crate) fn pack(
        &mut self,
        view: &VoxelView<'_>,
        flora: FloraView<'_>,
        fauna: FaunaView<'_>,
        candle_s: f64,
    ) -> Result<Option<TickReport>, String> {
        let p = self.params;
        let (w, h, d) = (p.width, p.height, p.depth);
        let Some(b) = self.ring.for_pack() else {
            return Ok(None);
        };
        self.ticks += 1;
        let now = self.clock + Duration::from_millis(50 * self.ticks);
        self.full.set_film_now(Some(now));
        self.delta.set_film_now(Some(now));
        let mut report = TickReport::default();

        let started = Instant::now();
        self.full.prepare(view, flora, fauna, candle_s);
        let mut styles_key = 0;
        self.full.fill(
            view,
            w,
            h,
            d,
            self.full_planes
                .staging([true; SLOW_PLANES], &mut self.full_owed, &mut self.full_carried, &mut styles_key),
        );
        report.full_ms = started.elapsed().as_secs_f64() * 1e3;

        let started = Instant::now();
        let keys = self.delta.prepare(view, flora, fauna, candle_s);
        let write = self.slow.pack(b, keys);
        let mut styles_key = 0;
        self.delta.fill(
            view,
            w,
            h,
            d,
            self.buffers[b].staging(write, &mut self.owed, &mut self.carried[b], &mut styles_key),
        );
        report.delta_ms = started.elapsed().as_secs_f64() * 1e3;
        self.slow.restamp(b, PLANE_STYLES, styles_key);
        self.ring.packed(b);
        report.bricks = (!self.carried[b].full).then_some(self.carried[b].bricks.len());

        self.compare(view.tick)?;
        if self.buffers[b].poses != self.full_planes.poses {
            return Err(format!("tick {}: the pose tables differ", view.tick));
        }
        let mut planes: [Vec<u8>; SLOW_PLANES] = Default::default();
        let (roof, glyphs, sky, canopy, glow) = self.delta.planes();
        planes[PLANE_ROOF] = roof.to_vec();
        planes[PLANE_GLYPHS] = glyphs.to_vec();
        planes[PLANE_SKY] = sky.to_vec();
        planes[PLANE_CANOPY] = canopy.to_vec();
        planes[PLANE_GLOW] = glow.to_vec();
        let mut styles = vec![VoxelStyle::default(); MAX_STYLES];
        let mut emit_curves = [0u8; MAX_STYLES];
        self.delta.style_table(&mut styles, &mut emit_curves);
        if emit_curves != self.buffers[b].emit_curves {
            return Err(format!("tick {}: the buffer's emission curves are not the palette's", view.tick));
        }
        self.latest = Some(Expect {
            voxels: self.delta.master().to_vec(),
            styles,
            emit_curves,
            planes,
        });
        Ok(Some(report))
    }

    /// The newest recorded frame is displaced before it ran: its upload is owed again.
    pub fn discard(&mut self) {
        let Some((rec, _)) = self.pending.pop_back() else { return };
        let _ = self.ring.discard();
        if let Some(i) = rec.staging {
            self.owed.returned(&self.carried[i]);
            self.slow.forget_images();
        }
    }

    pub(crate) fn record(&mut self) -> [bool; SLOW_PLANES] {
        let rec = self.ring.record();
        let copy = match rec.staging {
            Some(i) => {
                self.owed.taken(&self.carried[i]);
                self.slow.upload(i)
            }
            None => [false; SLOW_PLANES],
        };
        self.pending.push_back((rec, copy));
        copy
    }

    /// The oldest recorded frame ran: its upload lands in the images.
    pub(crate) fn retire(&mut self) {
        let Some((rec, copy)) = self.pending.pop_front() else { return };
        self.ring.retire();
        let Some(i) = rec.staging else { return };
        let p = self.params;
        let buffer = &self.buffers[i];
        apply_upload(&buffer.voxels, &self.carried[i], p.width, p.height, p.depth, &mut self.images.voxels);
        if copy[PLANE_STYLES] {
            self.images.styles.clone_from(&buffer.styles);
        }
        self.images.emit_curves = buffer.emit_curves;
        let im = &mut self.images;
        for (k, (from, to)) in [
            (&buffer.roof, &mut im.roof),
            (&buffer.glyphs, &mut im.glyphs),
            (&buffer.sky, &mut im.sky),
            (&buffer.canopy, &mut im.canopy),
            (&buffer.glow, &mut im.glow),
        ]
        .into_iter()
        .enumerate()
        {
            let plane = [PLANE_ROOF, PLANE_GLYPHS, PLANE_SKY, PLANE_CANOPY, PLANE_GLOW][k];
            if copy[plane] {
                to.clone_from(from);
            }
        }
    }

    /// Once every frame recorded has run and no pack waits, the images hold the latest
    /// pack: every texel of every brick not owed, the style table and every plane. An
    /// owed brick goes up with the next pack.
    pub(crate) fn check_images(&self) -> Result<(), String> {
        if !self.pending.is_empty() || self.ring.for_pack_waiting() {
            return Ok(());
        }
        let Some(e) = &self.latest else { return Ok(()) };
        if self.owed.full() {
            return Ok(());
        }
        let p = self.params;
        let (w, h) = (p.width as usize, p.height as usize);
        let owed: std::collections::HashSet<u32> = self.owed.list().iter().copied().collect();
        let im = &self.images;
        for (at, (a, b)) in im.voxels.iter().zip(&e.voxels).enumerate() {
            if a == b {
                continue;
            }
            let (x, y, z) = (at % w, (at / w) % h, at / (w * h));
            let brick = cubarium_gpu::voxel::brick_of(p.width, p.height, x as u32, y as u32, z as u32);
            if !owed.contains(&brick) {
                return Err(format!(
                    "the images hold voxel ({x}, {y}, {z}) as {a:?}, the latest pack {b:?}, and its brick is not owed"
                ));
            }
        }
        if im.styles != e.styles || im.emit_curves != e.emit_curves {
            return Err("the images' style table is not the latest pack's".into());
        }
        let names = ["roof", "glyphs", "sky", "glow", "styles", "canopy"];
        for (plane, got) in [
            (PLANE_ROOF, &im.roof),
            (PLANE_GLYPHS, &im.glyphs),
            (PLANE_SKY, &im.sky),
            (PLANE_CANOPY, &im.canopy),
            (PLANE_GLOW, &im.glow),
        ] {
            if !self.params.lit && matches!(plane, PLANE_SKY | PLANE_CANOPY | PLANE_GLOW) {
                continue;
            }
            if *got != e.planes[plane] {
                return Err(format!("the images' {} plane is not the latest pack's", names[plane]));
            }
        }
        Ok(())
    }

    /// The change-only pack against the full one: every texel as the shader resolves it,
    /// and the planes the full pack writes (the glow to a byte: the curve grids are mixed
    /// in another order of float sums).
    fn compare(&self, tick: u64) -> Result<(), String> {
        let p = self.params;
        let (w, h) = (p.width as usize, p.height as usize);
        for i in 0..p.voxel_count() {
            let (a, b) = (self.delta.resolved(i), self.full.resolved(i));
            if a != b {
                let (x, y, z) = (i % w, (i / w) % h, i / (w * h));
                return Err(format!(
                    "tick {tick}: texel ({x}, {y}, {z}) draws {a:?}, the full pack draws {b:?}"
                ));
            }
        }
        let (roof, glyphs, sky, canopy, glow) = self.delta.planes();
        let (f_roof, f_glyphs, f_sky, f_canopy, f_glow) = self.full.planes();
        if roof != f_roof {
            return Err(format!("tick {tick}: the roof tables differ"));
        }
        if glyphs != f_glyphs {
            return Err(format!("tick {tick}: the glyph atlases differ"));
        }
        // Each pack's sky plane arrives from its own thread: compare once both are in.
        if !self.light_pending() && sky != f_sky {
            return Err(format!("tick {tick}: the sky planes differ"));
        }
        if let Some(at) = canopy.iter().zip(f_canopy).position(|(a, b)| a != b) {
            return Err(format!("tick {tick}: canopy byte {at} is {}, the full pack's {}", canopy[at], f_canopy[at]));
        }
        if let Some(at) = glow.iter().zip(f_glow).position(|(a, b)| a.abs_diff(*b) > 1) {
            return Err(format!("tick {tick}: glow byte {at} is {}, the full pack's {}", glow[at], f_glow[at]));
        }
        Ok(())
    }
}
