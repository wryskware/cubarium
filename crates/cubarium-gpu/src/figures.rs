//! **The model pass** (H2 of `design/handoffs/hybrid-renderer-2026-10-07.md`): animals
//! drawn as their baked voxel models, after the slab walk, at positions and headings the
//! host interpolates between ticks on the frame clock.
//!
//! # Pixel look without pixel stepping
//!
//! - **Tiles.** Each figure's model is rasterised, as cubes with their real normals, into a
//!   small tile of the atlas at the raster's own scale (`s` pixels a voxel, `rise` a voxel
//!   of depth). The tile's pixel grid is anchored at the figure's place P: a world point q
//!   lands on texel `proj(q − P) − origin` ([`project`], [`tile_box`]). So the tile holds
//!   the same pixels wherever the figure stands; only a turn or a pose changes them.
//! - **Composite.** Each tile is placed at P's exact projection in the raster, fractional:
//!   a raster pixel takes the texels its footprint covers by area (the window's present
//!   pass's box filter), so the figure glides between pixels with its pixels intact.
//! - **Depth.** The walk writes the z of its first opaque face per pixel (`voxel.frag`,
//!   `depthOut`, into [`WALK_DEPTH_FORMAT`]); every tile texel carries its own face's z and
//!   counts only in front of it. Terrain and plants hide animals behind them and are hidden
//!   by animals in front.
//! - **Light.** `figure.frag` lights each texel with the walk's own functions
//!   (`world.glsl`) at the texel's world point with its face's normal.
//! - **Cross-fades.** A figure names one tile or two with a weight (a pose's model swap; a
//!   turn between fine angle steps), and a fade (birth, death) and a highlight.
//!
//! # Shadows of animals
//!
//! An animal is no longer a voxel of the volume the sun march walks, so its shadow comes
//! from a **sun map** (lit tier): before the walk, every figure cell is drawn along the
//! sun, orthographic, into a depth map that keeps each sun ray's highest figure point
//! (`figure_sun.vert`), a texel being where the ray meets y = 0 (two texels a voxel across,
//! [`SUN_MAP_H`] rows over the rays' reach in z). `world.glsl`'s `sunReaches` reads it
//! first: a point with a figure higher than itself on its sun ray is in shadow. So animals
//! shade the ground, the plants and each other, and their own lower faces, as the stamped
//! voxels did. A sun lower than [`SUN_MAP_MIN_Y`] casts none (the rays' reach grows
//! without bound).
//!
//! Every `unsafe` block below holds one `ash` call and nothing else (WORKING_POLICY,
//! "No unsafe code").

use anyhow::{Context, Result, anyhow, bail};
use ash::vk;

use crate::vk::{Gpu, HostBuffer};

const FIGURE_VERT: &[u8] = include_bytes!("../shaders/figure.vert.spv");
const FIGURE_SUN_VERT: &[u8] = include_bytes!("../shaders/figure_sun.vert.spv");
const FIGURE_SUN_FRAG: &[u8] = include_bytes!("../shaders/figure_sun.frag.spv");
const FIGURE_FRAG: &[u8] = include_bytes!("../shaders/figure.frag.spv");
const COMPOSITE_VERT: &[u8] = include_bytes!("../shaders/composite.vert.spv");
const COMPOSITE_FRAG: &[u8] = include_bytes!("../shaders/composite.frag.spv");

/// The walk's end depth per raster pixel, voxels of z (`voxel.frag`'s `depthOut`).
pub const WALK_DEPTH_FORMAT: vk::Format = vk::Format::R32_SFLOAT;
/// What the walk writes where its ray met nothing.
pub const FAR: f32 = 65504.0;
/// A tile texel counts when its z is at most this far behind the walk's hit: a body's
/// feet stand on the cap their texels are level with.
pub const DEPTH_BIAS: f32 = 0.05;

/// Rows of the figures' sun map.
pub const SUN_MAP_H: u32 = 512;
/// A sun lower than this (the y of the unit vector toward it) casts no figure shadow.
pub const SUN_MAP_MIN_Y: f32 = 0.2;

/// The figures' sun map for a world `w × h × d` under the sun `toward` (unit, toward the
/// sun), as `world.glsl`'s `figSun` and `figMap` carry it: `None` for a sun too low.
pub fn sun_map(w: u32, h: u32, d: u32, toward: [f32; 3]) -> Option<([f32; 4], [f32; 4])> {
    if !(toward[1] >= SUN_MAP_MIN_Y) {
        return None;
    }
    let (a, b) = (toward[0] / toward[1], toward[2] / toward[1]);
    // Where the rays through the world's box meet y = 0: z − y·b over z in 0..d, y in 0..h.
    let (h, d) = (h as f32, d as f32);
    let z0 = (0.0f32).min(-h * b);
    let z1 = d.max(d - h * b);
    let kz = SUN_MAP_H as f32 / (z1 - z0).max(1.0);
    Some(([a, b, z0, 1.0], [2.0, kz, (2 * w) as f32, SUN_MAP_H as f32]))
}

/// The atlas the frame's tiles are packed into, texels.
pub const ATLAS_W: u32 = 2048;
pub const ATLAS_H: u32 = 1024;
/// The most tiles, figures (seam copies included) and palette entries one frame draws.
pub const MAX_TILES: usize = 2048;
pub const MAX_FIGS: usize = 2048;
pub const MAX_PALETTE: usize = 16384;
/// The most model voxels the pass holds, every model of a run.
pub const MAX_VOXELS: usize = 1 << 18;

const ATLAS_COLOUR: vk::Format = vk::Format::R8G8B8A8_SRGB;
const ATLAS_Z: vk::Format = vk::Format::R32_SFLOAT;

/// How a model cell moves with the sleep and idle poses (`cubarium::voxel::sleep::Class`,
/// one up: 0 is a cell that never moves).
pub const CLASS_NONE: u8 = 0;
pub const CLASS_TORSO: u8 = 1;
pub const CLASS_BREATHING: u8 = 2;
pub const CLASS_LEG1: u8 = 3;
pub const CLASS_LEG2: u8 = 4;

/// One cell of a figure model, as `figure.vert` reads it: its offset in the model's frame,
/// its palette entry (from the figure's first), its pose class, whether it is the head, the
/// faces with no cell beside them, its glyph (0: the stamped baked-model glyph), and
/// each face's open-plane crease neighbours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct FigureVoxel {
    pub cell: u32,
    pub bits: u32,
    /// Eight occluders per face in its open plane: left, right, low, high, LL, RL, LH, RH.
    /// Four faces in the first word, two in the second, as `figure.vert` reads them.
    pub creases: [u32; 2],
}

impl FigureVoxel {
    /// `open`: one bit a face, `-x +x -y +y -z +z` from bit 0.
    pub fn new(offset: [i8; 3], palette: u8, class: u8, head: bool, open: u8, glyph: u8) -> FigureVoxel {
        FigureVoxel {
            cell: u32::from(offset[0] as u8)
                | u32::from(offset[1] as u8) << 8
                | u32::from(offset[2] as u8) << 16
                | u32::from(palette) << 24,
            bits: u32::from(class & 7)
                | u32::from(head) << 3
                | u32::from(open & 63) << 4
                | u32::from(glyph & 15) << 10,
            creases: [0; 2],
        }
    }

    pub fn with_creases(mut self, faces: [u8; 6]) -> Self {
        for (face, mask) in faces.into_iter().enumerate() {
            self.creases[face / 4] |= u32::from(mask) << (8 * (face % 4));
        }
        self
    }

    pub fn offset(self) -> [i8; 3] {
        [self.cell as u8 as i8, (self.cell >> 8) as u8 as i8, (self.cell >> 16) as u8 as i8]
    }

    pub fn palette(self) -> u8 {
        (self.cell >> 24) as u8
    }

    pub fn class(self) -> u8 {
        (self.bits & 7) as u8
    }

    pub fn head(self) -> bool {
        self.bits & 8 != 0
    }

    pub fn open(self) -> u8 {
        ((self.bits >> 4) & 63) as u8
    }
}

/// Occluding neighbours in each face's open plane, in the model's own frame. An
/// adjacent coplanar cell does not make a crease; a cell one step outward does.
/// These axes match `FACE_U/V/N` in `figure.glsl` (V points toward the high edge).
pub fn crease_masks(cells: &[[i8; 3]]) -> Vec<[u8; 6]> {
    const N: [[i16; 3]; 6] = [
        [-1, 0, 0],
        [1, 0, 0],
        [0, -1, 0],
        [0, 1, 0],
        [0, 0, -1],
        [0, 0, 1],
    ];
    const U: [[i16; 3]; 6] = [
        [0, 0, -1],
        [0, 0, 1],
        [1, 0, 0],
        [1, 0, 0],
        [1, 0, 0],
        [-1, 0, 0],
    ];
    const V: [[i16; 3]; 6] = [
        [0, 1, 0],
        [0, 1, 0],
        [0, 0, -1],
        [0, 0, 1],
        [0, 1, 0],
        [0, 1, 0],
    ];
    const EDGES: [[i16; 2]; 8] = [
        [-1, 0],
        [1, 0],
        [0, -1],
        [0, 1],
        [-1, -1],
        [1, -1],
        [-1, 1],
        [1, 1],
    ];
    let set: std::collections::HashSet<[i16; 3]> = cells.iter().map(|c| c.map(i16::from)).collect();
    cells
        .iter()
        .map(|c| {
            std::array::from_fn(|f| {
                EDGES.iter().enumerate().fold(0, |mask, (bit, &[du, dv])| {
                    let neighbour = std::array::from_fn(|i| {
                        i16::from(c[i]) + N[f][i] + du * U[f][i] + dv * V[f][i]
                    });
                    mask | if set.contains(&neighbour) {
                        1 << bit
                    } else {
                        0
                    }
                })
            })
        })
        .collect()
}

/// The open-face mask of every cell of a model: the faces with no other cell beside them.
pub fn open_faces(cells: &[[i8; 3]]) -> Vec<u8> {
    let set: std::collections::HashSet<[i8; 3]> = cells.iter().copied().collect();
    const N: [[i8; 3]; 6] = [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]];
    cells
        .iter()
        .map(|c| {
            N.iter().enumerate().fold(0u8, |m, (i, d)| {
                let n = [c[0].wrapping_add(d[0]), c[1].wrapping_add(d[1]), c[2].wrapping_add(d[2])];
                if set.contains(&n) { m } else { m | 1 << i }
            })
        })
        .collect()
}

/// One colour a figure's cells paint with, linear light, and its emissive colour if it
/// glows (lit tier).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaletteEntry {
    pub base: [f32; 3],
    pub emit: Option<[f32; 3]>,
}

impl PaletteEntry {
    fn gpu(self) -> [f32; 8] {
        let e = self.emit.unwrap_or([0.0; 3]);
        [
            self.base[0],
            self.base[1],
            self.base[2],
            if self.emit.is_some() { 1.0 } else { 0.0 },
            e[0],
            e[1],
            e[2],
            0.0,
        ]
    }
}

/// One model drawn into one tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FigureTile {
    /// Its cells: `first .. first + count` of the pass's models ([`FigurePass::put_models`]).
    pub first: u32,
    pub count: u32,
    /// P, the model's anchor in the world, voxels (`x` may lie anywhere round the ring).
    pub pos: [f32; 3],
    /// The model's x (forward), y (up) and z (side) axes in the world.
    pub axes: [[f32; 3]; 3],
    /// A sphere round every cell, in the model's frame: what the tile must hold.
    pub centre: [f32; 3],
    pub radius: f32,
    /// The pose: the body's settle and breath, the head's settle and breath (voxels).
    pub pose: [f32; 4],
    /// Its first entry in the frame's palette.
    pub palette: u32,
}

/// One figure this frame: its tile, or two cross-faded by `weight_b`, at tile A's P.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FigureInstance {
    pub id: u64,
    pub a: FigureTile,
    pub b: Option<FigureTile>,
    pub weight_b: f32,
    /// The birth or death fade, 0..1.
    pub alpha: f32,
    /// How far it leans toward white.
    pub highlight: f32,
}

/// Everything the model pass draws in a frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FigureFrame {
    pub figures: Vec<FigureInstance>,
    pub palette: Vec<PaletteEntry>,
    /// Place each tile at the whole raster pixel nearest its anchor instead of between
    /// pixels: crisp at rest, moving a pixel at a time.
    pub whole_pixels: bool,
}

/// World point `p` (voxels) in raster pixels, before the view turns it: the walk's
/// projection, `x · s` across and `base − y · s − z · rise` down.
pub fn project(s: u32, rise: u32, base: i32, p: [f32; 3]) -> [f32; 2] {
    [p[0] * s as f32, base as f32 - p[1] * s as f32 - p[2] * rise as f32]
}

/// A figure's tile: the screen offset from P's projection to its texel (0, 0), and its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileBox {
    pub ox: i32,
    pub oy: i32,
    pub w: u32,
    pub h: u32,
}

/// The tile a model with this bounding sphere needs, turned by `axes`. Whole texels, so the
/// grid stays anchored at P.
pub fn tile_box(s: u32, rise: u32, axes: [[f32; 3]; 3], centre: [f32; 3], radius: f32) -> TileBox {
    let d = [0, 1, 2].map(|i| axes[0][i] * centre[0] + axes[1][i] * centre[1] + axes[2][i] * centre[2]);
    let (s, r) = (s as f32, rise as f32);
    let (cx, cy) = (d[0] * s, -(d[1] * s + d[2] * r));
    // A sphere's projection along y and z reaches at most radius · (s + rise) up or down.
    let (hx, hy) = (radius * s + 1.0, radius * (s + r) + 1.0);
    let ox = (cx - hx).floor() as i32;
    let oy = (cy - hy).floor() as i32;
    TileBox {
        ox,
        oy,
        w: ((cx + hx).ceil() as i32 - ox).max(1) as u32,
        h: ((cy + hy).ceil() as i32 - oy).max(1) as u32,
    }
}

/// Rows of tiles packed left to right, a new row when one is full.
#[derive(Clone, Debug, Default)]
pub struct ShelfPacker {
    x: u32,
    y: u32,
    row: u32,
}

impl ShelfPacker {
    /// Where a `w × h` tile goes in a `width × height` atlas, if it fits.
    pub fn place(&mut self, w: u32, h: u32, width: u32, height: u32) -> Option<(u32, u32)> {
        if w > width || h > height {
            return None;
        }
        if self.x + w > width {
            self.y += self.row;
            self.x = 0;
            self.row = 0;
        }
        if self.y + h > height {
            return None;
        }
        let at = (self.x, self.y);
        self.x += w;
        self.row = self.row.max(h);
        Some(at)
    }

    /// The rows used so far.
    pub fn used_height(&self) -> u32 {
        self.y + self.row
    }
}

#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct TileGpu {
    pos: [f32; 4],
    ax: [f32; 4],
    ay: [f32; 4],
    az: [f32; 4],
    rect: [i32; 4],
    orig: [i32; 4],
    depth: [f32; 4],
    face_size: [i32; 4],
}

#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct FigGpu {
    anchor: [f32; 4],
    tiles: [i32; 4],
    k: [f32; 4],
}

/// One frame's tiles and composites, packed: what [`FigurePass::prepare`] writes.
#[derive(Clone, Debug, Default)]
pub struct Packed {
    tiles: Vec<TileGpu>,
    /// Each tile's cell count, beside it.
    counts: Vec<u32>,
    /// Whether each tile casts a shadow: its figure's shown tile, at least half faded in.
    casts: Vec<bool>,
    figs: Vec<FigGpu>,
    palette: Vec<[f32; 8]>,
    used_h: u32,
    /// Tiles the atlas had no room for this frame (their figures are not drawn).
    pub dropped: usize,
}

impl Packed {
    pub fn tiles(&self) -> usize {
        self.tiles.len()
    }

    pub fn figures(&self) -> usize {
        self.figs.len()
    }

    /// Each composite's anchor in the raster (view applied, wrapped) and tile A's rect:
    /// `(anchor, (ox, oy, w, h))`. For tests and tools.
    pub fn placements(&self) -> Vec<([f32; 2], (i32, i32, i32, i32))> {
        self.figs
            .iter()
            .map(|f| {
                let t = &self.tiles[f.tiles[0] as usize];
                ([f.anchor[0], f.anchor[1]], (t.orig[0], t.orig[1], t.rect[2], t.rect[3]))
            })
            .collect()
    }
}

/// Pack a frame for a world raster `raster_w` wide, turned `view_x` columns round the
/// ring: each tile's box and atlas place, each figure's anchor (and a second copy of it a
/// ring away where its tiles cross the raster's edge).
pub fn pack(frame: &FigureFrame, s: u32, rise: u32, base: i32, raster_w: u32, view_x: i32) -> Packed {
    let mut out = Packed {
        palette: frame.palette.iter().take(MAX_PALETTE).map(|p| p.gpu()).collect(),
        ..Packed::default()
    };
    let mut shelf = ShelfPacker::default();
    let rw = raster_w as f32;
    let mut tile = |t: &FigureTile, out: &mut Packed| -> Option<i32> {
        if out.tiles.len() >= MAX_TILES {
            return None;
        }
        let b = tile_box(s, rise, t.axes, t.centre, t.radius);
        let (x, y) = shelf.place(b.w, b.h, ATLAS_W, ATLAS_H)?;
        let span = 2.0 * t.radius + 6.0;
        let v = |a: [f32; 3], w: f32| [a[0], a[1], a[2], w];
        out.tiles.push(TileGpu {
            pos: v(t.pos, 0.0),
            ax: v(t.axes[0], t.pose[0]),
            ay: v(t.axes[1], t.pose[1]),
            az: v(t.axes[2], t.pose[2]),
            rect: [x as i32, y as i32, b.w as i32, b.h as i32],
            orig: [b.ox, b.oy, t.palette as i32, t.first as i32],
            depth: [t.pos[2], 1.0 / span, t.pose[3], 0.0],
            // Projected lengths of the model's axes, once per tile instead of per
            // vertex. A quarter turn exchanges the cap's S and RISE dimensions.
            face_size: {
                let sizes = t.axes.map(|a| {
                    let x = a[0] * s as f32;
                    let y = a[1] * s as f32 + a[2] * rise as f32;
                    x.hypot(y).round().max(1.0) as i32
                });
                [sizes[0], sizes[1], sizes[2], 0]
            },
        });
        out.counts.push(t.count);
        out.casts.push(false);
        Some(out.tiles.len() as i32 - 1)
    };
    for f in &frame.figures {
        let Some(a) = tile(&f.a, &mut out) else {
            out.dropped += 1;
            continue;
        };
        let b = f.b.and_then(|b| tile(&b, &mut out)).unwrap_or(-1);
        // The shadow is the dominant tile's, while the figure is at least half there.
        let shown = if b >= 0 && f.weight_b >= 0.5 { b } else { a };
        out.casts[shown as usize] = f.alpha >= 0.5;
        let mut p = project(s, rise, base, f.a.pos);
        // Centre a half-voxel box on the world's texel grid at odd scales. The
        // vertex pass subtracts this phase; the composite adds it back before
        // snapping, so quarter-heading samples land at the stamped texel centres.
        p[0] += (s % 2) as f32 * 0.5;
        p[1] += (rise % 2) as f32 * 0.5;
        if frame.whole_pixels {
            p = p.map(f32::round);
        }
        let x = (p[0] - view_x as f32).rem_euclid(rw);
        let fig = FigGpu {
            anchor: [x, p[1], if b >= 0 { f.weight_b } else { 0.0 }, f.alpha],
            tiles: [a, b, 0, 0],
            k: [f.highlight, 0.0, 0.0, 0.0],
        };
        // Its widest reach either side, over both tiles.
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in [a, b].into_iter().filter(|&i| i >= 0) {
            let t = &out.tiles[i as usize];
            lo = lo.min(t.orig[0] as f32);
            hi = hi.max((t.orig[0] + t.rect[2]) as f32);
        }
        for shift in [0.0, -rw, rw] {
            let (l, h) = (x + shift + lo - 1.0, x + shift + hi + 1.0);
            if (shift == 0.0 || (h > 0.0 && l < rw)) && out.figs.len() < MAX_FIGS {
                out.figs.push(FigGpu {
                    anchor: [x + shift, fig.anchor[1], fig.anchor[2], fig.anchor[3]],
                    ..fig
                });
            }
        }
    }
    out.used_h = shelf.used_height();
    out
}

/// The walk's render pass with its end depth beside the raster (and the emission, in the
/// lit tier): `[raster, emit, depth]`, or `[raster, -, depth]` in the flat tier, whose
/// location 1 writes go nowhere. Every attachment is drawn whole and left for sampling.
pub(crate) fn walk_pass(d: &ash::Device, raster: vk::Format, emit: Option<vk::Format>) -> Result<vk::RenderPass> {
    let mut formats = vec![raster];
    formats.extend(emit);
    formats.push(WALK_DEPTH_FORMAT);
    let attachments: Vec<_> = formats
        .iter()
        .map(|&format| {
            vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        })
        .collect();
    let at = |i: u32| vk::AttachmentReference::default().attachment(i).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let refs = match emit {
        Some(_) => vec![at(0), at(1), at(2)],
        None => vec![
            at(0),
            vk::AttachmentReference::default().attachment(vk::ATTACHMENT_UNUSED),
            at(1),
        ],
    };
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&refs)];
    let r = unsafe {
        d.create_render_pass(
            &vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpasses),
            None,
        )
    };
    Ok(r?)
}

/// An image, its memory and a view.
struct Plane {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
}

impl Plane {
    fn new(gpu: &Gpu, w: u32, h: u32, format: vk::Format, usage: vk::ImageUsageFlags, aspect: vk::ImageAspectFlags) -> Result<Plane> {
        let (image, memory) = gpu.image(w, h, format, vk::ImageTiling::OPTIMAL, usage)?;
        let info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(aspect).level_count(1).layer_count(1));
        let view = unsafe { gpu.device.create_image_view(&info, None) }?;
        Ok(Plane { image, memory, view })
    }

    fn destroy(&self, d: &ash::Device) {
        unsafe { d.destroy_image_view(self.view, None) };
        unsafe { d.destroy_image(self.image, None) };
        unsafe { d.free_memory(self.memory, None) };
    }
}

/// Bytes of one ring slot's frame buffer: tiles, then figures, then the palette.
const AT_FIGS: u64 = (MAX_TILES * std::mem::size_of::<TileGpu>()) as u64;
const AT_PALETTE: u64 = AT_FIGS + (MAX_FIGS * std::mem::size_of::<FigGpu>()) as u64;
const SLOT_BYTES: u64 = AT_PALETTE + (MAX_PALETTE * 32) as u64;

/// The model pass's GPU side: the walk's depth plane, the atlas, the two passes.
pub struct FigurePass {
    /// The walk's end depth, an attachment of the walk's pass.
    pub walk_depth: vk::ImageView,
    walk_plane: Plane,
    colour: Plane,
    emit: Plane,
    z: Plane,
    depth: Plane,
    depth_format: vk::Format,
    tile_pass: vk::RenderPass,
    tile_fb: vk::Framebuffer,
    composite_pass: vk::RenderPass,
    composite_fb: vk::Framebuffer,
    models: HostBuffer,
    voxels: usize,
    frames: Vec<HostBuffer>,
    packed: Vec<Packed>,
    set_layouts: [vk::DescriptorSetLayout; 2],
    pool: vk::DescriptorPool,
    data_sets: Vec<vk::DescriptorSet>,
    image_set: vk::DescriptorSet,
    sampler: vk::Sampler,
    layout: vk::PipelineLayout,
    tile_pipeline: vk::Pipeline,
    composite_pipeline: vk::Pipeline,
    raster: (u32, u32),
    lit: bool,
    /// The figures' sun map: its depth plane (sampled by the walk), pass and pipeline.
    pub sun_view: vk::ImageView,
    sun_plane: Plane,
    sun_pass: vk::RenderPass,
    sun_fb: vk::Framebuffer,
    sun_pipeline: vk::Pipeline,
    world: (u32, u32, u32),
}

/// The push block both passes read (`figure.glsl`'s `FigurePush`).
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Push {
    k: [i32; 4],
    atlas: [f32; 4],
}

impl FigurePass {
    /// The pass for a raster `raster` big, compositing onto `raster_view` (and the lit
    /// tier's `emit_view`), lit through the walk's `walk_layout` (set 0) with the walk's
    /// specialisation constants `constants` (`LIT` and the effect switches).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        gpu: &Gpu,
        raster: (u32, u32),
        raster_format: vk::Format,
        raster_view: vk::ImageView,
        emit: Option<(vk::Format, vk::ImageView)>,
        walk_layout: vk::DescriptorSetLayout,
        constants: &[u32],
        slots: usize,
        world: (u32, u32, u32),
    ) -> Result<FigurePass> {
        let d = &gpu.device;
        let colour_usage = vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED;
        let colour_aspect = vk::ImageAspectFlags::COLOR;
        let walk_plane = Plane::new(gpu, raster.0, raster.1, WALK_DEPTH_FORMAT, colour_usage, colour_aspect)?;
        let colour = Plane::new(gpu, ATLAS_W, ATLAS_H, ATLAS_COLOUR, colour_usage, colour_aspect)?;
        let emit_plane = Plane::new(gpu, ATLAS_W, ATLAS_H, ATLAS_COLOUR, colour_usage, colour_aspect)?;
        let z = Plane::new(gpu, ATLAS_W, ATLAS_H, ATLAS_Z, colour_usage, colour_aspect)?;
        let depth_format = [vk::Format::D32_SFLOAT, vk::Format::D16_UNORM]
            .into_iter()
            .find(|&f| {
                let p = unsafe { gpu.instance.get_physical_device_format_properties(gpu.pdev, f) };
                p.optimal_tiling_features
                    .contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT | vk::FormatFeatureFlags::SAMPLED_IMAGE)
            })
            .ok_or_else(|| anyhow!("no depth format for the model pass"))?;
        let depth = Plane::new(
            gpu,
            ATLAS_W,
            ATLAS_H,
            depth_format,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
        )?;

        // The tile pass: three colour planes and a depth, cleared, left for sampling.
        let colour_att = |format: vk::Format| {
            vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        };
        let tile_atts = [
            colour_att(ATLAS_COLOUR),
            colour_att(ATLAS_COLOUR),
            colour_att(ATLAS_Z),
            vk::AttachmentDescription::default()
                .format(depth_format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
        ];
        let at = |i: u32| vk::AttachmentReference::default().attachment(i).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        let tile_refs = [at(0), at(1), at(2)];
        let depth_ref = vk::AttachmentReference::default()
            .attachment(3)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let tile_sub = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&tile_refs)
            .depth_stencil_attachment(&depth_ref)];
        let tile_pass = unsafe {
            d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&tile_atts).subpasses(&tile_sub), None)
        }?;
        let tile_views = [colour.view, emit_plane.view, z.view, depth.view];
        let tile_fb = unsafe {
            d.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(tile_pass)
                    .attachments(&tile_views)
                    .width(ATLAS_W)
                    .height(ATLAS_H)
                    .layers(1),
                None,
            )
        }?;

        // The composite: over the raster (and the emission), loaded and left for sampling.
        let mut comp_formats = vec![raster_format];
        let mut comp_views = vec![raster_view];
        if let Some((f, v)) = emit {
            comp_formats.push(f);
            comp_views.push(v);
        }
        let comp_atts: Vec<_> = comp_formats
            .iter()
            .map(|&f| {
                vk::AttachmentDescription::default()
                    .format(f)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::LOAD)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            })
            .collect();
        let comp_refs: Vec<_> = (0..comp_formats.len() as u32).map(at).collect();
        let comp_sub = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&comp_refs)];
        let composite_pass = unsafe {
            d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&comp_atts).subpasses(&comp_sub), None)
        }?;
        let composite_fb = unsafe {
            d.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(composite_pass)
                    .attachments(&comp_views)
                    .width(raster.0)
                    .height(raster.1)
                    .layers(1),
                None,
            )
        }?;

        // Buffers: every model's cells, and one frame buffer a ring slot.
        let storage = vk::BufferUsageFlags::STORAGE_BUFFER;
        let models = gpu.host_buffer((MAX_VOXELS * 8) as u64, storage)?;
        let frames = (0..slots).map(|_| gpu.host_buffer(SLOT_BYTES, storage)).collect::<Result<Vec<_>>>()?;

        // Set 1: the cells, tiles, figures, palette (one set a slot); set 2: the planes.
        let both = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let ssbo = |b: u32| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(b)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(both)
        };
        let img = |b: u32| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(b)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        };
        let data_bindings = [ssbo(0), ssbo(1), ssbo(2), ssbo(3)];
        let image_bindings = [img(0), img(1), img(2), img(3)];
        let data_layout = unsafe {
            d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&data_bindings), None)
        }?;
        let image_layout = unsafe {
            d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&image_bindings), None)
        }?;
        let sizes = [
            vk::DescriptorPoolSize::default().ty(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(4 * slots as u32),
            vk::DescriptorPoolSize::default().ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(4),
        ];
        let pool = unsafe {
            d.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default().max_sets(slots as u32 + 1).pool_sizes(&sizes),
                None,
            )
        }?;
        let mut layouts = vec![data_layout; slots];
        layouts.push(image_layout);
        let mut sets = unsafe {
            d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))
        }?;
        let image_set = sets.pop().context("the model pass's image set")?;
        let data_sets = sets;
        let sampler = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )
        }?;
        for (set, frame) in data_sets.iter().zip(&frames) {
            let infos = [
                [vk::DescriptorBufferInfo::default().buffer(models.buffer).offset(0).range(vk::WHOLE_SIZE)],
                [vk::DescriptorBufferInfo::default().buffer(frame.buffer).offset(0).range(AT_FIGS)],
                [vk::DescriptorBufferInfo::default().buffer(frame.buffer).offset(AT_FIGS).range(AT_PALETTE - AT_FIGS)],
                [vk::DescriptorBufferInfo::default().buffer(frame.buffer).offset(AT_PALETTE).range(SLOT_BYTES - AT_PALETTE)],
            ];
            let writes: Vec<_> = infos
                .iter()
                .enumerate()
                .map(|(b, info)| {
                    vk::WriteDescriptorSet::default()
                        .dst_set(*set)
                        .dst_binding(b as u32)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(info)
                })
                .collect();
            unsafe { d.update_descriptor_sets(&writes, &[]) };
        }
        let image_infos = [colour.view, emit_plane.view, z.view, walk_plane.view].map(|v| {
            [vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(v)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]
        });
        let writes: Vec<_> = image_infos
            .iter()
            .enumerate()
            .map(|(b, info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(image_set)
                    .dst_binding(b as u32)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(info)
            })
            .collect();
        unsafe { d.update_descriptor_sets(&writes, &[]) };

        let set_layouts = [walk_layout, data_layout, image_layout];
        let push = [vk::PushConstantRange::default()
            .stage_flags(both)
            .offset(0)
            .size(std::mem::size_of::<Push>() as u32)];
        let layout = unsafe {
            d.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push),
                None,
            )
        }?;

        let mut flags: Vec<u32> = constants.to_vec();
        flags.extend([0, 0, 0, 0]);
        let entries: Vec<_> = (0..flags.len() as u32)
            .map(|i| vk::SpecializationMapEntry::default().constant_id(i).offset(i * 4).size(4))
            .collect();
        let spec = vk::SpecializationInfo::default().map_entries(&entries).data(bytemuck::cast_slice(&flags));
        let tile_pipeline = {
            let vs = gpu.shader(FIGURE_VERT)?;
            let fs = gpu.shader(FIGURE_FRAG).context("the model pass's fragment shader")?;
            let blend = [plain(); 3];
            let p = pipeline(d, tile_pass, layout, vs, fs, Some(&spec), vk::PrimitiveTopology::TRIANGLE_LIST, &blend, true);
            unsafe { d.destroy_shader_module(vs, None) };
            unsafe { d.destroy_shader_module(fs, None) };
            p?
        };
        let composite_pipeline = {
            let vs = gpu.shader(COMPOSITE_VERT)?;
            let fs = gpu.shader(COMPOSITE_FRAG)?;
            let blend = vec![over(); comp_formats.len()];
            let p = pipeline(d, composite_pass, layout, vs, fs, None, vk::PrimitiveTopology::TRIANGLE_STRIP, &blend, false);
            unsafe { d.destroy_shader_module(vs, None) };
            unsafe { d.destroy_shader_module(fs, None) };
            p?
        };
        // The sun map: a depth plane alone, cleared to the floor, left for sampling.
        let sun_plane = Plane::new(
            gpu,
            2 * world.0,
            SUN_MAP_H,
            depth_format,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::DEPTH,
        )?;
        let sun_att = [vk::AttachmentDescription::default()
            .format(depth_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let sun_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let sun_sub = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .depth_stencil_attachment(&sun_ref)];
        let sun_pass = unsafe {
            d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&sun_att).subpasses(&sun_sub), None)
        }?;
        let sun_views = [sun_plane.view];
        let sun_fb = unsafe {
            d.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(sun_pass)
                    .attachments(&sun_views)
                    .width(2 * world.0)
                    .height(SUN_MAP_H)
                    .layers(1),
                None,
            )
        }?;
        let sun_pipeline = {
            let vs = gpu.shader(FIGURE_SUN_VERT)?;
            let fs = gpu.shader(FIGURE_SUN_FRAG)?;
            let p = pipeline(d, sun_pass, layout, vs, fs, None, vk::PrimitiveTopology::TRIANGLE_LIST, &[], true);
            unsafe { d.destroy_shader_module(vs, None) };
            unsafe { d.destroy_shader_module(fs, None) };
            p?
        };
        let lit = emit.is_some();
        Ok(FigurePass {
            sun_view: sun_plane.view,
            sun_plane,
            sun_pass,
            sun_fb,
            sun_pipeline,
            world,
            walk_depth: walk_plane.view,
            walk_plane,
            colour,
            emit: emit_plane,
            z,
            depth,
            depth_format,
            tile_pass,
            tile_fb,
            composite_pass,
            composite_fb,
            models,
            voxels: 0,
            packed: vec![Packed::default(); slots],
            frames,
            set_layouts: [data_layout, image_layout],
            pool,
            data_sets,
            image_set,
            sampler,
            layout,
            tile_pipeline,
            composite_pipeline,
            raster,
            lit,
        })
    }

    /// Model cells written so far.
    pub fn voxels(&self) -> usize {
        self.voxels
    }

    /// Write model cells at `first` (cells a frame in flight reads are never rewritten:
    /// the host only appends). False when they do not fit.
    pub fn put_models(&mut self, first: usize, cells: &[FigureVoxel]) -> bool {
        if first + cells.len() > MAX_VOXELS {
            return false;
        }
        self.models.write_at(first, cells);
        self.voxels = self.voxels.max(first + cells.len());
        true
    }

    /// Pack this frame into ring slot `slot`'s buffer. Returns how many tiles the atlas
    /// had no room for.
    pub fn prepare(&mut self, slot: usize, frame: &FigureFrame, s: u32, rise: u32, base: i32, view_x: i32) -> usize {
        let packed = pack(frame, s, rise, base, self.raster.0, view_x);
        let buf = &self.frames[slot];
        buf.write_bytes_at(0, &packed.tiles);
        buf.write_bytes_at(AT_FIGS, &packed.figs);
        buf.write_bytes_at(AT_PALETTE, &packed.palette);
        let dropped = packed.dropped;
        self.packed[slot] = packed;
        dropped
    }

    /// Record the figures' sun map for slot `slot` (packed by [`FigurePass::prepare`]) under
    /// `sun` (`figSun`, [`sun_map`]): cleared, then every figure cell, before the walk.
    /// Every frame that draws the world records it, so the walk always samples a defined
    /// map; with no sun or no figures it stays clear.
    pub(crate) fn record_sun(&self, d: &ash::Device, cb: vk::CommandBuffer, slot: usize, sun: Option<([f32; 4], [f32; 4])>) {
        let (w, h, _) = self.world;
        let clears = [vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 } }];
        let extent = vk::Extent2D { width: 2 * w, height: SUN_MAP_H };
        let begin = vk::RenderPassBeginInfo::default()
            .render_pass(self.sun_pass)
            .framebuffer(self.sun_fb)
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent })
            .clear_values(&clears);
        unsafe { d.cmd_begin_render_pass(cb, &begin, vk::SubpassContents::INLINE) };
        let packed = &self.packed[slot];
        if let Some((fs, fm)) = sun
            && !packed.tiles.is_empty()
        {
            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: (2 * w) as f32,
                height: SUN_MAP_H as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            unsafe { d.cmd_set_viewport(cb, 0, &[viewport]) };
            unsafe { d.cmd_set_scissor(cb, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }]) };
            unsafe { d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.sun_pipeline) };
            let sets = [self.data_sets[slot]];
            unsafe { d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 1, &sets, &[]) };
            for (i, t) in packed.tiles.iter().enumerate() {
                if !packed.casts[i] {
                    continue;
                }
                let push = Push {
                    k: [i as i32, w as i32, h as i32, SUN_MAP_H as i32],
                    atlas: [fs[0], fs[1], fs[2], fm[1]],
                };
                unsafe { d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&push)) };
                unsafe { d.cmd_draw(cb, 36, packed.counts[i], 0, t.orig[3] as u32) };
            }
        }
        unsafe { d.cmd_end_render_pass(cb) };
        let m = [vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)];
        unsafe {
            d.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &m,
                &[],
                &[],
            )
        };
    }

    /// Whether this slot's frame has any figure (it has a sun map worth reading).
    pub fn has_tiles(&self, slot: usize) -> bool {
        !self.packed[slot].tiles.is_empty()
    }

    /// Whether slot `slot`'s frame draws anything.
    pub fn drawing(&self, slot: usize) -> bool {
        !self.packed[slot].figs.is_empty()
    }

    /// Record the tile pass and the composite for slot `slot`, after the walk: `walk_set`
    /// is the walk's set 0 at its dynamic offset `offset`.
    pub(crate) fn record(&self, d: &ash::Device, cb: vk::CommandBuffer, slot: usize, walk_set: vk::DescriptorSet, offset: u32, s: u32, rise: u32) {
        let packed = &self.packed[slot];
        if packed.figs.is_empty() {
            return;
        }
        let used_h = packed.used_h.clamp(1, ATLAS_H);
        let clears = [
            vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
            vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
            vk::ClearValue { color: vk::ClearColorValue { float32: [FAR, 0.0, 0.0, 0.0] } },
            vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
        ];
        let area = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: ATLAS_W, height: used_h } };
        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: ATLAS_W as f32,
            height: ATLAS_H as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let sets = [walk_set, self.data_sets[slot]];
        // What the walk wrote is read below; what the tiles write is read by the composite.
        let fence_writes = |d: &ash::Device| {
            let m = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            unsafe {
                d.cmd_pipeline_barrier(
                    cb,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &m,
                    &[],
                    &[],
                )
            };
        };
        fence_writes(d);
        let begin = vk::RenderPassBeginInfo::default()
            .render_pass(self.tile_pass)
            .framebuffer(self.tile_fb)
            .render_area(area)
            .clear_values(&clears);
        unsafe { d.cmd_begin_render_pass(cb, &begin, vk::SubpassContents::INLINE) };
        unsafe { d.cmd_set_viewport(cb, 0, &[viewport]) };
        unsafe { d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.tile_pipeline) };
        unsafe { d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &sets, &[offset]) };
        for (i, t) in packed.tiles.iter().enumerate() {
            let push = Push {
                k: [i as i32, s as i32, rise as i32, 0],
                atlas: [ATLAS_W as f32, ATLAS_H as f32, DEPTH_BIAS, 0.0],
            };
            let scissor = vk::Rect2D {
                offset: vk::Offset2D { x: t.rect[0], y: t.rect[1] },
                extent: vk::Extent2D { width: t.rect[2] as u32, height: t.rect[3] as u32 },
            };
            let count = packed.counts[i];
            unsafe { d.cmd_set_scissor(cb, 0, &[scissor]) };
            unsafe { d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&push)) };
            unsafe { d.cmd_draw(cb, 36, count, 0, t.orig[3] as u32) };
        }
        unsafe { d.cmd_end_render_pass(cb) };
        fence_writes(d);

        let (rw, rh) = self.raster;
        let begin = vk::RenderPassBeginInfo::default()
            .render_pass(self.composite_pass)
            .framebuffer(self.composite_fb)
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: rw, height: rh } });
        unsafe { d.cmd_begin_render_pass(cb, &begin, vk::SubpassContents::INLINE) };
        let full = vk::Viewport { x: 0.0, y: 0.0, width: rw as f32, height: rh as f32, min_depth: 0.0, max_depth: 1.0 };
        unsafe { d.cmd_set_viewport(cb, 0, &[full]) };
        let scissor = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: rw, height: rh } };
        unsafe { d.cmd_set_scissor(cb, 0, &[scissor]) };
        unsafe { d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.composite_pipeline) };
        let sets = [walk_set, self.data_sets[slot], self.image_set];
        unsafe { d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &sets, &[offset]) };
        let push = Push {
            k: [rw as i32, s as i32, rise as i32, rh as i32],
            atlas: [ATLAS_W as f32, ATLAS_H as f32, DEPTH_BIAS, 0.0],
        };
        unsafe { d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&push)) };
        unsafe { d.cmd_draw(cb, 4, packed.figs.len() as u32, 0, 0) };
        unsafe { d.cmd_end_render_pass(cb) };
    }

    /// Whether the lit tier's emission is composited too.
    pub fn lit(&self) -> bool {
        self.lit
    }

    /// The depth format the tiles are tested with.
    pub fn depth_format(&self) -> vk::Format {
        self.depth_format
    }

    pub(crate) fn destroy(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe { d.destroy_pipeline(self.tile_pipeline, None) };
        unsafe { d.destroy_pipeline(self.composite_pipeline, None) };
        unsafe { d.destroy_pipeline(self.sun_pipeline, None) };
        unsafe { d.destroy_framebuffer(self.sun_fb, None) };
        unsafe { d.destroy_render_pass(self.sun_pass, None) };
        self.sun_plane.destroy(d);
        unsafe { d.destroy_pipeline_layout(self.layout, None) };
        unsafe { d.destroy_descriptor_pool(self.pool, None) };
        for l in self.set_layouts {
            unsafe { d.destroy_descriptor_set_layout(l, None) };
        }
        unsafe { d.destroy_sampler(self.sampler, None) };
        unsafe { d.destroy_framebuffer(self.tile_fb, None) };
        unsafe { d.destroy_framebuffer(self.composite_fb, None) };
        unsafe { d.destroy_render_pass(self.tile_pass, None) };
        unsafe { d.destroy_render_pass(self.composite_pass, None) };
        self.models.destroy(gpu);
        for f in &self.frames {
            f.destroy(gpu);
        }
        for p in [&self.walk_plane, &self.colour, &self.emit, &self.z, &self.depth] {
            p.destroy(d);
        }
    }
}

fn plain() -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)
}

/// Premultiplied colour over what is there; the destination's alpha kept (the emission's
/// alpha is the weather's depth).
fn over() -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::ONE)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ZERO)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE)
        .alpha_blend_op(vk::BlendOp::ADD)
}

#[allow(clippy::too_many_arguments)]
fn pipeline(
    d: &ash::Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    vs: vk::ShaderModule,
    fs: vk::ShaderModule,
    spec: Option<&vk::SpecializationInfo>,
    topology: vk::PrimitiveTopology,
    blend: &[vk::PipelineColorBlendAttachmentState],
    depth: bool,
) -> Result<vk::Pipeline> {
    let mut fs_stage = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::FRAGMENT)
        .module(fs)
        .name(c"main");
    if let Some(spec) = spec {
        fs_stage = fs_stage.specialization_info(spec);
    }
    let stages = [
        vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
        fs_stage,
    ];
    let vi = vk::PipelineVertexInputStateCreateInfo::default();
    let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(topology);
    let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
    let rs = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0);
    let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let ds = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(depth)
        .depth_write_enable(depth)
        // The sun map keeps each ray's highest point; a tile its nearest face.
        .depth_compare_op(if blend.is_empty() { vk::CompareOp::GREATER } else { vk::CompareOp::LESS_OR_EQUAL });
    let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(blend);
    let dynamic = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic);
    let info = [vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vi)
        .input_assembly_state(&ia)
        .viewport_state(&vp)
        .rasterization_state(&rs)
        .multisample_state(&ms)
        .depth_stencil_state(&ds)
        .color_blend_state(&cb)
        .dynamic_state(&dy)
        .layout(layout)
        .render_pass(pass)
        .subpass(0)];
    let made = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &info, None) };
    match made {
        Ok(p) => Ok(p[0]),
        Err((_, e)) => bail!("vkCreateGraphicsPipelines (model pass): {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPRIGHT: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    fn tile(pos: [f32; 3]) -> FigureTile {
        FigureTile {
            first: 0,
            count: 1,
            pos,
            axes: UPRIGHT,
            centre: [0.0, 0.5, 0.0],
            radius: 1.0,
            pose: [0.0; 4],
            palette: 0,
        }
    }

    #[test]
    fn a_world_point_lands_on_the_walks_pixel() {
        // s 4, rise 2, base 100: the top front edge of voxel (3, 5, 7) is the walk's
        // `frontRow(5, 7)` = base − 6·4 − 7·2 = 62, at column 3·4 = 12.
        assert_eq!(project(4, 2, 100, [3.0, 6.0, 7.0]), [12.0, 62.0]);
        // Half a voxel on in x is half a voxel's pixels on.
        assert_eq!(project(4, 2, 100, [3.5, 6.0, 7.0]), [14.0, 62.0]);
    }

    #[test]
    fn a_tile_holds_its_model_and_its_grid_is_anchored_at_p() {
        let b = tile_box(4, 2, UPRIGHT, [0.0, 0.5, 0.0], 1.0);
        // The model's centre projects to (0, −2) from P; a radius of 1 reaches 4 + 1 px
        // across and 6 + 1 px down and up.
        assert_eq!(b, TileBox { ox: -5, oy: -9, w: 10, h: 14 });
        // The box is whole texels whatever P is: it depends on the turn alone.
        let turned = [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]];
        let c = tile_box(4, 2, turned, [1.0, 0.5, 0.0], 1.0);
        // Turned a quarter, the model's +x is the world's −z: its centre rises 2 px.
        assert_eq!(c.oy, b.oy + 2);
    }

    #[test]
    fn a_figure_is_anchored_at_its_projection_turned_by_the_view() {
        let frame = FigureFrame {
            figures: vec![FigureInstance {
                id: 1,
                a: tile([10.25, 3.0, 4.0]),
                b: None,
                weight_b: 0.0,
                alpha: 1.0,
                highlight: 0.0,
            }],
            palette: vec![PaletteEntry::default()],
            whole_pixels: false,
        };
        let p = pack(&frame, 4, 2, 60, 256, 8);
        let placed = p.placements();
        assert_eq!(placed.len(), 1);
        // x 10.25 · 4 − 8 = 33 across; 60 − 3 · 4 − 4 · 2 = 40 down.
        assert_eq!(placed[0].0, [33.0, 60.0 - 12.0 - 8.0]);
    }

    #[test]
    fn a_figure_across_the_seam_is_drawn_at_both_edges() {
        let frame = FigureFrame {
            figures: vec![FigureInstance {
                id: 1,
                a: tile([0.1, 3.0, 4.0]),
                b: None,
                weight_b: 0.0,
                alpha: 1.0,
                highlight: 0.0,
            }],
            palette: vec![PaletteEntry::default()],
            whole_pixels: false,
        };
        let p = pack(&frame, 4, 2, 60, 256, 0);
        let xs: Vec<f32> = p.placements().iter().map(|q| q.0[0]).collect();
        assert_eq!(xs.len(), 2, "{xs:?}");
        assert!(xs.contains(&0.4) && xs.contains(&256.4), "{xs:?}");
    }

    #[test]
    fn the_sun_map_reaches_every_ray_through_the_world_and_a_low_sun_casts_none() {
        assert_eq!(sun_map(64, 32, 16, [0.0, 0.1, 0.995]), None);
        let (fs, fm) = sun_map(64, 32, 16, [0.6, 0.8, 0.0]).unwrap();
        assert_eq!((fs[0], fs[1]), (0.75, 0.0));
        // A sun straight across x: the rays' z is the world's own, 0..16.
        assert_eq!((fs[2], fm[1] * 16.0), (0.0, SUN_MAP_H as f32));
        // Leaning toward +z, a ray from the floor at the back reaches z 16, one from the
        // top at the front z −32·b.
        let (fs, fm) = sun_map(64, 32, 16, [0.0, 0.8, 0.6]).unwrap();
        assert_eq!(fs[2], -32.0 * 0.75);
        assert!((fm[1] * (16.0 + 24.0) - SUN_MAP_H as f32).abs() < 1e-3);
        assert_eq!(fm[2], 128.0, "two texels a voxel across");
    }

    #[test]
    fn the_shelf_starts_a_row_when_one_is_full_and_refuses_what_cannot_fit() {
        let mut s = ShelfPacker::default();
        assert_eq!(s.place(6, 3, 10, 8), Some((0, 0)));
        assert_eq!(s.place(4, 5, 10, 8), Some((6, 0)));
        assert_eq!(s.place(3, 2, 10, 8), Some((0, 5)));
        assert_eq!(s.place(3, 2, 10, 8), Some((3, 5)));
        assert_eq!(s.place(5, 2, 10, 8), None, "no row of 2 left under 7");
        assert_eq!(s.used_height(), 7);
    }

    #[test]
    fn a_cell_round_trips_its_packing_and_knows_its_open_faces() {
        let v = FigureVoxel::new([-3, 7, -128], 200, CLASS_LEG2, true, 0b101010, 5);
        assert_eq!(v.offset(), [-3, 7, -128]);
        assert_eq!((v.palette(), v.class(), v.head(), v.open()), (200, CLASS_LEG2, true, 0b101010));
        let open = open_faces(&[[0, 0, 0], [1, 0, 0], [0, 1, 0]]);
        // The first cell has neighbours at +x (bit 1) and +y (bit 3).
        assert_eq!(open[0], 0b111111 & !(1 << 1) & !(1 << 3));
        assert_eq!(open[1], 0b111111 & !1);
    }
    #[test]
    fn creases_are_only_occluders_in_the_faces_open_plane() {
        // The front face (-z) stays flat beside a coplanar cell. A cell left and in
        // front of it makes a left crease; above and in front makes a high crease.
        let cells = [[0, 0, 0], [-1, 0, 0], [-1, 0, -1], [0, 1, -1]];
        let masks = crease_masks(&cells);
        assert_eq!(masks[0][4], 1 | 8);
        assert_eq!(crease_masks(&cells[..2])[0][4], 0);
        // A diagonal alone shades only its corner, not both entire edges.
        assert_eq!(crease_masks(&[[0, 0, 0], [1, 1, -1]])[0][4], 1 << 7);
        // The cap (+y) sees the same geometry rotated into its own x/z plane.
        assert_eq!(
            crease_masks(&[[0, 0, 0], [-1, 1, 0], [0, 1, 1]])[0][3],
            1 | 8
        );
        // The packed mask survives all six face slots, including the second word.
        let faces = [1, 2, 4, 8, 16, 32];
        let voxel = FigureVoxel::default().with_creases(faces);
        for (i, want) in faces.into_iter().enumerate() {
            assert_eq!(((voxel.creases[i / 4] >> (8 * (i % 4))) & 255) as u8, want);
        }
        // Offset extremes must not wrap around and invent neighbours.
        assert_eq!(crease_masks(&[[-128, 0, 0], [127, 0, -1]])[0][4], 0);
    }
}
