//! Figure model data, exact CPU atlas packing and safe geometry passes.
pub use crate::wgpu_backend::geometry::Figures as FigurePass;
/// The walk records its linear world depth in a sampled colour plane.
pub const WALK_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
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
pub const ATLAS_W: u32 = 4096;
pub const ATLAS_H: u32 = 2048;
const LEGACY_ATLAS: (u32, u32) = (2048, 1024);
/// The most tiles, figures (seam copies included) and palette entries one frame draws.
pub const MAX_TILES: usize = 2048;
pub const MAX_FIGS: usize = 2048;
pub const MAX_PALETTE: usize = 16384;
/// Bone transforms a frame can carry (FEED-RIG): eight for each of [`MAX_TILES`].
pub const MAX_BONES: usize = 8 * MAX_TILES;
/// The bones a model's cells can name: [`FigureVoxel::bone`]'s bits.
pub const BONE_LIMIT: u8 = 64;
/// The most model voxels the pass holds, every model of a run.
pub const MAX_VOXELS: usize = 1 << 18;

/// How a model cell moves with the sleep and idle poses (`cubarium::voxel::sleep::Class`,
/// one up: 0 is a cell that never moves).
pub const CLASS_NONE: u8 = 0;
pub const CLASS_TORSO: u8 = 1;
pub const CLASS_BREATHING: u8 = 2;
pub const CLASS_LEG1: u8 = 3;
pub const CLASS_LEG2: u8 = 4;

/// One cell of a figure model, as `figure.vert` reads it: its offset in the model's frame,
/// its palette entry (from the figure's first), its pose class, whether it is the head, the
/// faces with no cell beside them, its glyph (0: the stamped baked-model glyph), its rig
/// bone (bits 14..19; 0 the root, FEED-RIG), whether its top reads open for the lip tone
/// (bit 20, [`FigureVoxel::with_lip`]), and each face's open-plane crease neighbours.
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
    pub fn new(
        offset: [i8; 3],
        palette: u8,
        class: u8,
        head: bool,
        open: u8,
        glyph: u8,
    ) -> FigureVoxel {
        FigureVoxel {
            cell: u32::from(offset[0] as u8)
                | u32::from(offset[1] as u8) << 8
                | u32::from(offset[2] as u8) << 16
                | u32::from(palette) << 24,
            bits: u32::from(class & 7)
                | u32::from(head) << 3
                | u32::from(open & 63) << 4
                | u32::from(glyph & 15) << 10
                | u32::from((open >> 3) & 1) << 20,
            creases: [0; 2],
        }
    }

    /// Whether its top reads open for the lip tone, the top row of a side face drawn as the
    /// top's edge (`figure.frag`); by default its open `+y` face. A rigged model draws a top
    /// face against another bone's cell (so a moving bone leaves no hole), but its lip keeps
    /// the body at rest where the two bones only part in a state's transition (RIG-STATES).
    pub fn with_lip(mut self, open: bool) -> Self {
        self.bits = (self.bits & !(1 << 20)) | u32::from(open) << 20;
        self
    }

    pub fn lip(self) -> bool {
        (self.bits >> 20) & 1 != 0
    }

    /// This cell on rig bone `bone` (below [`BONE_LIMIT`]): a rigged figure moves it by the
    /// frame's transform of that bone ([`FigureTile::bones`]).
    pub fn with_bone(mut self, bone: u8) -> Self {
        self.bits = (self.bits & !(63 << 14)) | u32::from(bone.min(BONE_LIMIT - 1)) << 14;
        self
    }

    pub fn bone(self) -> u8 {
        ((self.bits >> 14) & 63) as u8
    }

    pub fn with_creases(mut self, faces: [u8; 6]) -> Self {
        for (face, mask) in faces.into_iter().enumerate() {
            self.creases[face / 4] |= u32::from(mask) << (8 * (face % 4));
        }
        self
    }

    pub fn offset(self) -> [i8; 3] {
        [
            self.cell as u8 as i8,
            (self.cell >> 8) as u8 as i8,
            (self.cell >> 16) as u8 as i8,
        ]
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

/// The open-face mask of every cell of a rigged model (FEED-RIG): the faces with no cell
/// **of the same bone** beside them. A face between two bones is drawn, so a bending joint
/// shows the cube behind it rather than a hole; at rest it is inside the body, hidden.
pub fn open_faces_by_bone(cells: &[[i8; 3]], bones: &[u8]) -> Vec<u8> {
    let set: std::collections::HashMap<[i8; 3], u8> =
        cells.iter().copied().zip(bones.iter().copied()).collect();
    const N: [[i8; 3]; 6] = [
        [-1, 0, 0],
        [1, 0, 0],
        [0, -1, 0],
        [0, 1, 0],
        [0, 0, -1],
        [0, 0, 1],
    ];
    cells
        .iter()
        .zip(bones)
        .map(|(c, b)| {
            N.iter().enumerate().fold(0u8, |m, (i, d)| {
                let n = [
                    c[0].wrapping_add(d[0]),
                    c[1].wrapping_add(d[1]),
                    c[2].wrapping_add(d[2]),
                ];
                if set.get(&n) == Some(b) {
                    m
                } else {
                    m | 1 << i
                }
            })
        })
        .collect()
}

/// The open-face mask of every cell of a model: the faces with no other cell beside them.
pub fn open_faces(cells: &[[i8; 3]]) -> Vec<u8> {
    let set: std::collections::HashSet<[i8; 3]> = cells.iter().copied().collect();
    const N: [[i8; 3]; 6] = [
        [-1, 0, 0],
        [1, 0, 0],
        [0, -1, 0],
        [0, 1, 0],
        [0, 0, -1],
        [0, 0, 1],
    ];
    cells
        .iter()
        .map(|c| {
            N.iter().enumerate().fold(0u8, |m, (i, d)| {
                let n = [
                    c[0].wrapping_add(d[0]),
                    c[1].wrapping_add(d[1]),
                    c[2].wrapping_add(d[2]),
                ];
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
    /// A rigged figure's bones this frame (FEED-RIG): the first of its transforms in
    /// [`FigureFrame::bones`] and how many; a cell on bone `k` moves by transform
    /// `first + k`. `None`: the model as baked.
    pub bones: Option<(u32, u32)>,
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
#[derive(Clone, Debug, PartialEq)]
pub struct FigureFrame {
    pub figures: Vec<FigureInstance>,
    pub palette: Vec<PaletteEntry>,
    /// Place each tile at the whole raster pixel nearest its anchor instead of between
    /// pixels: crisp at rest, moving a pixel at a time.
    pub whole_pixels: bool,
    /// Tile samples per raster pixel on each axis. One preserves the original raster.
    pub supersample: u32,
    /// Every rigged tile's bone transforms (FEED-RIG), in the model's own frame (voxels):
    /// three rows of `[m0, m1, m2, t]`, a rest point `p` going to `M p + t`.
    pub bones: Vec<[[f32; 4]; 3]>,
}

impl Default for FigureFrame {
    fn default() -> Self {
        Self {
            figures: Vec::new(),
            palette: Vec::new(),
            whole_pixels: false,
            supersample: 1,
            bones: Vec::new(),
        }
    }
}

/// World point `p` (voxels) in raster pixels, before the view turns it: the walk's
/// projection, `x · s` across and `base − y · s − z · rise` down.
pub fn project(s: u32, rise: u32, base: i32, p: [f32; 3]) -> [f32; 2] {
    [
        p[0] * s as f32,
        base as f32 - p[1] * s as f32 - p[2] * rise as f32,
    ]
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
    let d =
        [0, 1, 2].map(|i| axes[0][i] * centre[0] + axes[1][i] * centre[1] + axes[2][i] * centre[2]);
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
pub(crate) struct TileGpu {
    pos: [f32; 4],
    ax: [f32; 4],
    ay: [f32; 4],
    az: [f32; 4],
    pub(crate) rect: [i32; 4],
    pub(crate) orig: [i32; 4],
    depth: [f32; 4],
    face_size: [i32; 4],
    /// FEED-RIG: the first of its bones in the slot's bone buffer (-1: none), -, -, -.
    rig: [i32; 4],
}

#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(crate) struct FigGpu {
    anchor: [f32; 4],
    tiles: [i32; 4],
    k: [f32; 4],
}

/// One frame's tiles and composites, packed: what [`FigurePass::prepare`] writes.
#[derive(Clone, Debug, Default)]
pub struct Packed {
    pub(crate) tiles: Vec<TileGpu>,
    /// Each tile's cell count, beside it.
    pub(crate) counts: Vec<u32>,
    /// Whether each tile casts a shadow: its figure's shown tile, at least half faded in.
    pub(crate) casts: Vec<bool>,
    pub(crate) figs: Vec<FigGpu>,
    pub(crate) palette: Vec<[f32; 8]>,
    /// The rigged tiles' bone transforms, rows of `[m0, m1, m2, t]`.
    pub(crate) bones: Vec<[[f32; 4]; 3]>,
    pub(crate) used_h: u32,
    /// The viewport and clip-space scale, possibly smaller than the physical planes.
    pub logical_atlas: (u32, u32),
    /// Tiles the atlas had no room for this frame (their figures are not drawn).
    pub dropped: usize,
    /// The requested and actual samples per axis; packing lowers quality before omitting
    /// figures when the atlas fills.
    pub requested_supersample: u32,
    pub supersample: u32,
    /// Swap tiles or seam copies that exceeded their buffers.
    pub omitted_extras: usize,
}

/// The latest frame's atlas budget, for headless captures and startup diagnostics.
#[derive(Clone, Copy, Debug, Default)]
pub struct FigureAtlasStats {
    pub width: u32,
    pub height: u32,
    pub logical_width: u32,
    pub logical_height: u32,
    pub tile_capacity: usize,
    pub composite_capacity: usize,
    pub tiles: usize,
    pub figures: usize,
    pub composites: usize,
    pub requested_supersample: u32,
    pub supersample: u32,
    pub dropped: usize,
    pub omitted_extras: usize,
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
                let aa = t.face_size[3];
                (
                    [f.anchor[0], f.anchor[1]],
                    (t.orig[0], t.orig[1], t.rect[2] / aa, t.rect[3] / aa),
                )
            })
            .collect()
    }
}

/// Pack a frame for a world raster `raster_w` wide, turned `view_x` columns round the
/// ring: each tile's box and atlas place, each figure's anchor (and a second copy of it a
/// ring away where its tiles cross the raster's edge).
pub fn pack(
    frame: &FigureFrame,
    s: u32,
    rise: u32,
    base: i32,
    raster_w: u32,
    view_x: i32,
) -> Packed {
    if frame.supersample.max(1) == 1 {
        // Keep the original tile positions and clip-space precision at S=1. Merely
        // doubling the viewport changes a few borderline samples of turned models.
        let legacy = pack_into(frame, s, rise, base, raster_w, view_x, LEGACY_ATLAS);
        if legacy.dropped == 0 && legacy.omitted_extras == 0 {
            return legacy;
        }
    }
    pack_into(frame, s, rise, base, raster_w, view_x, (ATLAS_W, ATLAS_H))
}

#[allow(clippy::too_many_arguments)]
fn pack_into(
    frame: &FigureFrame,
    s: u32,
    rise: u32,
    base: i32,
    raster_w: u32,
    view_x: i32,
    atlas: (u32, u32),
) -> Packed {
    let requested = frame.supersample.max(1);
    // Clamp even an enormous override before multiplying tile dimensions or entering
    // the shader's sample loops. A factor larger than a tile can fit cannot be used.
    let mut aa = frame
        .figures
        .iter()
        .flat_map(|f| [Some(f.a), f.b])
        .flatten()
        .fold(requested, |aa, t| {
            let b = tile_box(s, rise, t.axes, t.centre, t.radius);
            aa.min(atlas.0 / b.w).min(atlas.1 / b.h)
        })
        .max(1);
    loop {
        let mut out = pack_at(frame, s, rise, base, raster_w, view_x, atlas, aa);
        out.requested_supersample = requested;
        if (out.dropped == 0 && out.omitted_extras == 0) || aa == 1 {
            return out;
        }
        // Repack the entire frame so a large early tile cannot starve later animals.
        aa = (aa / 2).max(1);
    }
}

#[allow(clippy::too_many_arguments)]
fn pack_at(
    frame: &FigureFrame,
    s: u32,
    rise: u32,
    base: i32,
    raster_w: u32,
    view_x: i32,
    atlas: (u32, u32),
    aa: u32,
) -> Packed {
    let mut out = Packed {
        palette: frame
            .palette
            .iter()
            .take(MAX_PALETTE)
            .map(|p| p.gpu())
            .collect(),
        supersample: aa,
        logical_atlas: atlas,
        ..Packed::default()
    };
    let mut shelf = ShelfPacker::default();
    let rw = raster_w as f32;
    let mut tile = |t: &FigureTile, out: &mut Packed| -> Option<i32> {
        if out.tiles.len() >= MAX_TILES
            || t.palette as usize >= out.palette.len()
            || t.first
                .checked_add(t.count)
                .is_none_or(|end| end as usize > MAX_VOXELS)
        {
            return None;
        }
        let b = tile_box(s, rise, t.axes, t.centre, t.radius);
        let (w, h) = (b.w.checked_mul(aa)?, b.h.checked_mul(aa)?);
        let (x, y) = shelf.place(w, h, atlas.0, atlas.1)?;
        let span = 2.0 * t.radius + 6.0;
        let v = |a: [f32; 3], w: f32| [a[0], a[1], a[2], w];
        // Its bones, while the frame has them and the buffer has room; else drawn at rest.
        let rig = match t.bones {
            Some((first, n))
                if out.bones.len() + n as usize <= MAX_BONES
                    && (first as usize)
                        .checked_add(n as usize)
                        .is_some_and(|e| e <= frame.bones.len()) =>
            {
                let at = out.bones.len() as i32;
                out.bones
                    .extend_from_slice(&frame.bones[first as usize..(first + n) as usize]);
                [at, 0, 0, 0]
            }
            _ => [-1, 0, 0, 0],
        };
        out.tiles.push(TileGpu {
            pos: v(t.pos, 0.0),
            ax: v(t.axes[0], t.pose[0]),
            ay: v(t.axes[1], t.pose[1]),
            az: v(t.axes[2], t.pose[2]),
            rect: [x as i32, y as i32, w as i32, h as i32],
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
                [sizes[0], sizes[1], sizes[2], aa as i32]
            },
            rig,
        });
        out.counts.push(t.count);
        out.casts.push(false);
        Some(out.tiles.len() as i32 - 1)
    };
    for (index, f) in frame.figures.iter().enumerate() {
        let remaining = frame.figures.len() - index - 1;
        let Some(a) = tile(&f.a, &mut out) else {
            out.dropped += 1;
            continue;
        };
        // Reserve one tile and composite for every remaining animal before adding
        // swaps and seam copies. Optional extras must never displace whole figures.
        let b =
            f.b.filter(|_| out.tiles.len() + remaining < MAX_TILES)
                .and_then(|b| tile(&b, &mut out))
                .unwrap_or(-1);
        out.omitted_extras += usize::from(f.b.is_some() && b < 0);
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
            hi = hi.max(t.orig[0] as f32 + t.rect[2] as f32 / aa as f32);
        }
        for shift in [0.0, -rw, rw] {
            let (l, h) = (x + shift + lo - 1.0, x + shift + hi + 1.0);
            if shift == 0.0 || (h > 0.0 && l < rw) {
                if out.figs.len() + if shift == 0.0 { 0 } else { remaining } < MAX_FIGS {
                    out.figs.push(FigGpu {
                        anchor: [x + shift, fig.anchor[1], fig.anchor[2], fig.anchor[3]],
                        ..fig
                    });
                } else if shift == 0.0 {
                    out.dropped += 1;
                } else {
                    out.omitted_extras += 1;
                }
            }
        }
    }
    // Back to front (larger z first), so a nearer figure's soft edge blends over the one
    // behind it; the composite's depth test settles every overlap per pixel either way.
    let tiles = &out.tiles;
    out.figs.sort_by(|a, b| {
        tiles[b.tiles[0] as usize].pos[2].total_cmp(&tiles[a.tiles[0] as usize].pos[2])
    });
    out.used_h = shelf.used_height();
    out
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
            bones: None,
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
        assert_eq!(
            b,
            TileBox {
                ox: -5,
                oy: -9,
                w: 10,
                h: 14
            }
        );
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
            supersample: 1,
            ..FigureFrame::default()
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
            supersample: 1,
            ..FigureFrame::default()
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

    fn frame_for_packing(n: usize, supersample: u32) -> FigureFrame {
        FigureFrame {
            figures: (0..n)
                .map(|id| FigureInstance {
                    id: id as u64,
                    a: tile([10.25, 3.0, 4.0]),
                    b: None,
                    weight_b: 0.0,
                    alpha: 1.0,
                    highlight: 0.0,
                })
                .collect(),
            palette: vec![PaletteEntry::default()],
            supersample,
            ..FigureFrame::default()
        }
    }

    #[test]
    fn supersampling_only_refines_the_tile_lattice() {
        for whole_pixels in [false, true] {
            let mut frame = frame_for_packing(1, 1);
            frame.whole_pixels = whole_pixels;
            let one = pack(&frame, 7, 3, 60, 256, 8);
            frame.supersample = 2;
            let two = pack(&frame, 7, 3, 60, 256, 8);
            assert_eq!(two.supersample, frame.supersample);
            assert_eq!(
                one.placements(),
                two.placements(),
                "output-pixel placement is unchanged"
            );
            for (one, two) in one.tiles.iter().zip(&two.tiles) {
                assert_eq!(two.rect[2], one.rect[2] * frame.supersample as i32);
                assert_eq!(two.rect[3], one.rect[3] * frame.supersample as i32);
                assert_eq!(one.orig, two.orig, "origins stay in output pixels");
                assert_eq!(
                    &one.face_size[..3],
                    &two.face_size[..3],
                    "face lines retain their output width"
                );
            }
        }
    }

    #[test]
    fn a_full_atlas_reduces_sampling_before_omitting_figures() {
        let frame = frame_for_packing(2, 2);
        let b = tile_box(
            4,
            2,
            frame.figures[0].a.axes,
            frame.figures[0].a.centre,
            frame.figures[0].a.radius,
        );
        // One S=2 tile fills this atlas; both S=1 tiles fit side by side.
        let packed = pack_into(&frame, 4, 2, 60, 256, 0, (2 * b.w, 2 * b.h));
        assert!(packed.supersample < frame.supersample);
        assert_eq!(packed.tiles(), frame.figures.len());
        assert_eq!(packed.dropped, 0);
        assert_eq!(packed.omitted_extras, 0);
    }

    #[test]
    fn one_sample_keeps_legacy_clip_space_and_grows_when_needed() {
        let mut frame = frame_for_packing(1, 1);
        let legacy = pack(&frame, 4, 1, 60, 20000, 0);
        assert_eq!(legacy.logical_atlas, LEGACY_ATLAS);
        frame.figures[0].a.pos[0] = 1000.0;
        // A tile taller than the old atlas, derived from its height and projection.
        frame.figures[0].a.radius = LEGACY_ATLAS.1 as f32 / (2.0 * (4.0 + 1.0)) + 1.0;
        let expanded = pack(&frame, 4, 1, 60, 20000, 0);
        assert_eq!(expanded.logical_atlas, (ATLAS_W, ATLAS_H));
        assert_eq!(expanded.dropped, 0);
        assert_eq!(expanded.figures(), frame.figures.len());
        assert_eq!(expanded.supersample, frame.supersample);
    }

    #[test]
    fn oversized_factors_are_bounded_by_the_atlas() {
        let frame = frame_for_packing(1, u32::MAX);
        let b = tile_box(
            4,
            2,
            frame.figures[0].a.axes,
            frame.figures[0].a.centre,
            frame.figures[0].a.radius,
        );
        let packed = pack_into(&frame, 4, 2, 60, 256, 0, (2 * b.w, 2 * b.h));
        assert!(packed.supersample < frame.supersample);
        assert_eq!(packed.dropped, 0);
        assert!(
            packed
                .tiles
                .iter()
                .all(|t| t.rect[2] as u32 <= 2 * b.w && t.rect[3] as u32 <= 2 * b.h)
        );
    }

    #[test]
    fn optional_swaps_and_seam_copies_never_displace_primary_figures() {
        let mut frame = frame_for_packing(MAX_TILES.min(MAX_FIGS), 1);
        for figure in &mut frame.figures {
            figure.a.pos[0] = 0.0;
            figure.a.radius = 0.01;
            figure.b = Some(figure.a);
        }
        let packed = pack(&frame, 1, 1, 60, 64, 0);
        assert_eq!(
            packed.dropped, 0,
            "every animal keeps its primary tile and composite"
        );
        assert_eq!(packed.figures(), frame.figures.len());
        assert_eq!(packed.tiles(), frame.figures.len());
        assert!(
            packed.omitted_extras > 0,
            "swaps and seam copies yield to primary figures"
        );
        let primary: std::collections::HashSet<_> =
            packed.figs.iter().map(|f| f.tiles[0]).collect();
        assert_eq!(primary.len(), frame.figures.len());
    }

    #[test]
    fn a_cell_round_trips_its_packing_and_knows_its_open_faces() {
        let v = FigureVoxel::new([-3, 7, -128], 200, CLASS_LEG2, true, 0b101010, 5);
        assert_eq!(v.offset(), [-3, 7, -128]);
        assert_eq!(
            (v.palette(), v.class(), v.head(), v.open()),
            (200, CLASS_LEG2, true, 0b101010)
        );
        assert!(v.lip(), "its lip is its open top by default");
        assert!(!v.with_lip(false).lip() && v.with_lip(false).open() == 0b101010);
        let open = open_faces(&[[0, 0, 0], [1, 0, 0], [0, 1, 0]]);
        // The first cell has neighbours at +x (bit 1) and +y (bit 3).
        assert_eq!(open[0], 0b111111 & !(1 << 1) & !(1 << 3));
        assert_eq!(open[1], 0b111111 & !1);
    }
    #[test]
    fn a_cell_keeps_its_bone_and_a_face_between_two_bones_is_drawn() {
        let v = FigureVoxel::new([1, 2, -3], 4, CLASS_TORSO, true, 0b10_1010, 3).with_bone(5);
        assert_eq!(
            (v.bone(), v.class(), v.head(), v.open(), v.offset()),
            (5, CLASS_TORSO, true, 0b10_1010, [1, 2, -3])
        );
        let cells = [[0, 0, 0], [1, 0, 0], [2, 0, 0]];
        let open = open_faces_by_bone(&cells, &[0, 0, 1]);
        // the first two share a bone: the face between them is inside; the third's
        // neighbour is another bone's, so both faces between them are drawn
        assert_eq!(open[0] & 0b10, 0);
        assert_eq!(open[1] & 0b10, 0b10);
        assert_eq!(open[2] & 0b01, 0b01);
        assert_eq!(open_faces_by_bone(&cells, &[0, 0, 0]), open_faces(&cells));
    }

    #[test]
    fn a_rigged_tile_carries_its_bones_to_the_buffer() {
        let rows = |k: f32| {
            [
                [1.0, 0.0, 0.0, k],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ]
        };
        let mut frame = FigureFrame {
            figures: vec![
                FigureInstance {
                    id: 1,
                    a: tile([4.0, 3.0, 4.0]),
                    b: None,
                    weight_b: 0.0,
                    alpha: 1.0,
                    highlight: 0.0,
                },
                FigureInstance {
                    id: 2,
                    a: FigureTile {
                        bones: Some((1, 2)),
                        ..tile([9.0, 3.0, 4.0])
                    },
                    b: None,
                    weight_b: 0.0,
                    alpha: 1.0,
                    highlight: 0.0,
                },
            ],
            palette: vec![PaletteEntry::default()],
            bones: vec![rows(0.0), rows(1.0), rows(2.0)],
            ..FigureFrame::default()
        };
        let p = pack(&frame, 4, 2, 60, 256, 0);
        assert_eq!(
            p.bones,
            vec![rows(1.0), rows(2.0)],
            "only the rigged tile's bones, from its first"
        );
        let rigs: Vec<[i32; 4]> = p.tiles.iter().map(|t| t.rig).collect();
        assert!(rigs.contains(&[-1, 0, 0, 0]) && rigs.contains(&[0, 0, 0, 0]));
        // bones the frame does not hold: drawn as baked
        frame.figures[1].a.bones = Some((2, 5));
        assert!(
            pack(&frame, 4, 2, 60, 256, 0)
                .tiles
                .iter()
                .all(|t| t.rig[0] == -1)
        );
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
