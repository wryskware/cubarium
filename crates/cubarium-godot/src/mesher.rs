//! **The terrain and water mesher**: the view feed's chunks (`game/README.md`, "The view")
//! as plain coloured voxel meshes, one terrain and one water mesh per 16³ chunk. Godot-side
//! presentation code (D-0004): the sim crates do not know it exists.
//!
//! - **What it keeps**: a copy of the whole grid's three planes (material, free water, pore
//!   water), written chunk by chunk as the feed sends them.
//! - **What it meshes**: only chunks whose cells changed in a way the picture shows (a
//!   material, a free-water byte, or a pore-water level), and a neighbour when a change
//!   sits on the face they share. A remeshed chunk is reported only when its geometry
//!   actually changed (a hash of the arrays), so a neighbour remeshed for nothing costs no
//!   upload.
//! - **Faces**: a solid cell gets a face toward every non-solid neighbour; cells past the
//!   world's edge are air, except across the ring's seam in `x` when `wrap_x` is on. The
//!   view meshes only what its camera can see ([`VIEW_FACES`]: tops and fronts; an
//!   unrotated orthographic camera sees every side face edge-on).
//! - **Greedy merging**: coplanar neighbouring faces of one colour merge into one quad.
//!   The depth haze is linear in `z`, so it is applied per vertex and merged faces stay
//!   exact.
//! - **Water**: free water is a translucent box per cell whose top sits at the fill level
//!   (`free / 255` of the cell, or the whole cell when water stands above it). Its top face
//!   is drawn where no water or rock lies above, its sides where the neighbour's water is
//!   lower. Body faces in the deep-water colour, the surface in the surface colour, at the
//!   presenter's alphas.
//! - **Pore water** darkens a solid cell toward deep water (`wet`), in eight levels so pore
//!   water's slow creep does not remesh a chunk every update.
//! - **Colours** are the presenter's own (`palette.rs`), haze toward the back as
//!   `haze_amount · z / (depth − 1)`.
//!
//! Coordinates: the world's `x`, `y` up and `z` into the depth map to Godot's `x`, `y` and
//! `−z`, so the front of the world faces a camera looking down Godot's `−z`. A chunk's
//! mesh is in voxels, relative to its own origin corner `(16·cx, 16·cy, −16·cz)`.

use std::hash::{Hash, Hasher};

use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::{ArrayMesh, IRefCounted};
use godot::prelude::*;

use crate::palette::{Linear, Look, PaletteFile, mix};

/// Voxels along a chunk's edge (the feed's `chunk_size`).
pub const CHUNK: usize = 16;
const CELLS: usize = CHUNK * CHUNK * CHUNK;
/// Pore water is drawn in this many levels above dry.
pub const WET_LEVELS: u32 = 7;

/// A face direction in the world's axes (`y` up, `z` into the depth).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    PosX = 0,
    NegX = 1,
    PosY = 2,
    NegY = 3,
    PosZ = 4,
    NegZ = 5,
}

pub const DIRS: [Dir; 6] = [
    Dir::PosX,
    Dir::NegX,
    Dir::PosY,
    Dir::NegY,
    Dir::PosZ,
    Dir::NegZ,
];

impl Dir {
    pub const fn bit(self) -> u8 {
        1 << self as u8
    }
    /// The axis the face's normal runs along (0 x, 1 y, 2 z).
    pub const fn axis(self) -> usize {
        self as usize / 2
    }
    pub const fn positive(self) -> bool {
        self as usize % 2 == 0
    }
    /// The step to the neighbour this face looks at.
    pub const fn step(self) -> [i64; 3] {
        match self {
            Dir::PosX => [1, 0, 0],
            Dir::NegX => [-1, 0, 0],
            Dir::PosY => [0, 1, 0],
            Dir::NegY => [0, -1, 0],
            Dir::PosZ => [0, 0, 1],
            Dir::NegZ => [0, 0, -1],
        }
    }
    /// The face's in-plane axes `(u, v)`; `v` is `y` for every side face.
    const fn plane(self) -> (usize, usize) {
        match self.axis() {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        }
    }
    /// The normal in Godot's axes (`z` flipped).
    fn godot_normal(self) -> [f32; 3] {
        let s = self.step();
        [s[0] as f32, s[1] as f32, -s[2] as f32]
    }
}

/// Every face.
pub const ALL_FACES: u8 = 0b11_1111;
/// What an unrotated elevated orthographic camera at the front sees: tops, and fronts
/// (the faces toward `−z`, the viewer).
pub const VIEW_FACES: u8 = Dir::PosY.bit() | Dir::NegZ.bit();

/// One mesh's arrays, in Godot's axes, relative to its chunk's origin corner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    pub pos: Vec<[f32; 3]>,
    pub normal: Vec<[f32; 3]>,
    pub color: Vec<[f32; 4]>,
    pub index: Vec<i32>,
}

impl MeshData {
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub fn quads(&self) -> usize {
        self.index.len() / 6
    }

    fn hash_into(&self, h: &mut impl Hasher) {
        for p in self.pos.iter().chain(&self.normal) {
            p.map(f32::to_bits).hash(h);
        }
        for c in &self.color {
            c.map(f32::to_bits).hash(h);
        }
        self.index.hash(h);
    }

    /// The face toward `dir` of the unit cell at `offset` from a model's anchor, the cell
    /// centred on its column (`x` and `z` ± ½, `y` from `offset.y` up), in Godot's axes.
    pub fn quad_box_face(&mut self, offset: [i32; 3], dir: Dir, rgba: [f32; 4]) {
        let lo = [
            offset[0] as f32 - 0.5,
            offset[1] as f32,
            offset[2] as f32 - 0.5,
        ];
        let hi = [lo[0] + 1.0, lo[1] + 1.0, lo[2] + 1.0];
        let (ua, va) = dir.plane();
        let a = dir.axis();
        let corner = |u: bool, v: bool| {
            let mut p = [0f32; 3];
            p[a] = if dir.positive() { hi[a] } else { lo[a] };
            p[ua] = if u { hi[ua] } else { lo[ua] };
            p[va] = if v { hi[va] } else { lo[va] };
            [p[0], p[1], -p[2]]
        };
        let corners = [
            corner(false, false),
            corner(true, false),
            corner(true, true),
            corner(false, true),
        ];
        self.quad(corners, dir.godot_normal(), [rgba; 4]);
    }

    /// A quad from four corners in order round its edge, wound so its front faces
    /// `normal` (Godot's front faces are clockwise).
    fn quad(&mut self, corners: [[f32; 3]; 4], normal: [f32; 3], colors: [[f32; 4]; 4]) {
        let base = self.pos.len() as i32;
        let (a, b, c) = (corners[0], corners[1], corners[2]);
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let cross = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let ccw = cross[0] * normal[0] + cross[1] * normal[1] + cross[2] * normal[2] > 0.0;
        self.pos.extend(corners);
        self.normal.extend([normal; 4]);
        self.color.extend(colors);
        if ccw {
            self.index
                .extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        } else {
            self.index
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}

/// A chunk's meshes.
#[derive(Clone, Debug, PartialEq)]
pub struct ChunkMesh {
    pub at: [usize; 3],
    pub terrain: MeshData,
    pub water: MeshData,
}

/// The whole grid's three planes, `x` fastest, then `y`, then `z`.
#[derive(Clone, Debug)]
pub struct Grid {
    pub dims: [usize; 3],
    material: Vec<u8>,
    free: Vec<u8>,
    pore: Vec<u8>,
    wrap_x: bool,
}

impl Grid {
    pub fn new(dims: [usize; 3], wrap_x: bool) -> Grid {
        let n = dims[0] * dims[1] * dims[2];
        Grid {
            dims,
            material: vec![0; n],
            free: vec![0; n],
            pore: vec![0; n],
            wrap_x,
        }
    }

    #[inline]
    fn index(&self, x: usize, y: usize, z: usize) -> usize {
        (z * self.dims[1] + y) * self.dims[0] + x
    }

    /// The index of `(x, y, z)`, wrapping `x` round the ring when on; `None` past an edge.
    #[inline]
    fn at(&self, x: i64, y: i64, z: i64) -> Option<usize> {
        let [w, h, d] = self.dims.map(|v| v as i64);
        let x = if self.wrap_x { x.rem_euclid(w) } else { x };
        if x < 0 || y < 0 || z < 0 || x >= w || y >= h || z >= d {
            return None;
        }
        Some(self.index(x as usize, y as usize, z as usize))
    }

    #[inline]
    fn solid(&self, i: Option<usize>) -> bool {
        i.is_some_and(|i| self.material[i] != 0)
    }

    /// How high free water stands in a cell, `0..=1` of it: the whole cell when water
    /// stands in the open cell above, else its fill. 0 for a solid or dry cell, or past an
    /// edge.
    #[inline]
    fn water_height(&self, x: i64, y: i64, z: i64) -> f32 {
        let Some(i) = self.at(x, y, z) else {
            return 0.0;
        };
        if self.material[i] != 0 || self.free[i] == 0 {
            return 0.0;
        }
        match self.at(x, y + 1, z) {
            Some(a) if self.material[a] == 0 && self.free[a] > 0 => 1.0,
            _ => f32::from(self.free[i]) / 255.0,
        }
    }

    /// Set one cell's planes (for tests and hand-built worlds).
    pub fn set(&mut self, x: usize, y: usize, z: usize, material: u8, free: u8, pore: u8) {
        let i = self.index(x, y, z);
        self.material[i] = material;
        self.free[i] = free;
        self.pore[i] = pore;
    }
}

/// The wet level a pore byte draws at, `0..=WET_LEVELS`.
#[inline]
fn wet_level(pore: u8) -> u32 {
    (u32::from(pore) * WET_LEVELS + 127) / 255
}

/// What changed in one chunk write.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Touched {
    /// Anything the picture shows.
    pub any: bool,
    /// A material or free-water change on the chunk's face toward each [`Dir`].
    pub faces: [bool; 6],
}

/// **The mesher**: the grid, which chunks are owed a mesh, and the hash of each chunk's
/// last reported meshes.
pub struct Mesher {
    pub grid: Grid,
    pub look: Look,
    /// Which faces to emit ([`Dir::bit`]s).
    pub faces: u8,
    /// Merge coplanar faces of one colour.
    pub greedy: bool,
    cdims: [usize; 3],
    dirty: Vec<bool>,
    reported: Vec<Option<u64>>,
}

impl Mesher {
    pub fn new(dims: [usize; 3], look: Look, wrap_x: bool) -> Mesher {
        let cdims = dims.map(|d| d.div_ceil(CHUNK));
        let n = cdims[0] * cdims[1] * cdims[2];
        Mesher {
            grid: Grid::new(dims, wrap_x),
            look,
            faces: ALL_FACES,
            greedy: true,
            cdims,
            dirty: vec![false; n],
            reported: vec![None; n],
        }
    }

    pub fn chunk_dims(&self) -> [usize; 3] {
        self.cdims
    }

    fn chunk_index(&self, at: [usize; 3]) -> usize {
        (at[2] * self.cdims[1] + at[1]) * self.cdims[0] + at[0]
    }

    fn chunk_at(&self, i: usize) -> [usize; 3] {
        let cx = i % self.cdims[0];
        let cy = (i / self.cdims[0]) % self.cdims[1];
        [cx, cy, i / (self.cdims[0] * self.cdims[1])]
    }

    /// Owe chunk `at` a mesh.
    pub fn mark(&mut self, at: [usize; 3]) {
        let i = self.chunk_index(at);
        self.dirty[i] = true;
    }

    /// Owe every chunk a mesh.
    pub fn mark_all(&mut self) {
        self.dirty.fill(true);
    }

    /// Write one chunk's feed bytes (`3 · 16³`: material, free water, pore water) and owe
    /// it — and each neighbour across a face where a change sits — a mesh.
    pub fn write_chunk(&mut self, at: [usize; 3], bytes: &[u8]) -> Touched {
        let mut t = Touched::default();
        if bytes.len() < 3 * CELLS || (0..3).any(|a| at[a] >= self.cdims[a]) {
            return t;
        }
        let [w, h, d] = self.grid.dims;
        let o = at.map(|c| c * CHUNK);
        for lz in 0..CHUNK {
            let z = o[2] + lz;
            if z >= d {
                break;
            }
            for ly in 0..CHUNK {
                let y = o[1] + ly;
                if y >= h {
                    break;
                }
                for lx in 0..CHUNK {
                    let x = o[0] + lx;
                    if x >= w {
                        break;
                    }
                    let l = (lz * CHUNK + ly) * CHUNK + lx;
                    let (m, f, p) = (bytes[l], bytes[CELLS + l], bytes[2 * CELLS + l]);
                    let i = self.grid.index(x, y, z);
                    let shape = self.grid.material[i] != m || self.grid.free[i] != f;
                    let wet = wet_level(self.grid.pore[i]) != wet_level(p);
                    if !(shape || wet || self.grid.pore[i] != p) {
                        continue;
                    }
                    self.grid.material[i] = m;
                    self.grid.free[i] = f;
                    self.grid.pore[i] = p;
                    t.any |= shape || wet;
                    if shape {
                        let local = [lx, ly, lz];
                        for dir in DIRS {
                            let a = dir.axis();
                            let edge = if dir.positive() { CHUNK - 1 } else { 0 };
                            if local[a] == edge {
                                t.faces[dir as usize] = true;
                            }
                        }
                    }
                }
            }
        }
        if t.any {
            self.mark(at);
        }
        for dir in DIRS {
            if t.faces[dir as usize] {
                if let Some(n) = self.neighbour(at, dir) {
                    self.mark(n);
                }
            }
        }
        t
    }

    /// The chunk across `at`'s face toward `dir`, wrapping in `x` round the ring when on.
    fn neighbour(&self, at: [usize; 3], dir: Dir) -> Option<[usize; 3]> {
        let s = dir.step();
        let mut n = [0usize; 3];
        for a in 0..3 {
            let v = at[a] as i64 + s[a];
            let lim = self.cdims[a] as i64;
            let v = if a == 0 && self.grid.wrap_x {
                v.rem_euclid(lim)
            } else {
                v
            };
            if v < 0 || v >= lim {
                return None;
            }
            n[a] = v as usize;
        }
        Some(n)
    }

    /// Mesh every owed chunk (in parallel over `threads`) and return those whose meshes
    /// differ from what was last reported for them.
    pub fn flush(&mut self, threads: usize) -> Vec<ChunkMesh> {
        let owed: Vec<usize> = (0..self.dirty.len()).filter(|&i| self.dirty[i]).collect();
        self.dirty.fill(false);
        if owed.is_empty() {
            return Vec::new();
        }
        let threads = threads.clamp(1, owed.len().div_ceil(4).max(1));
        let me: &Mesher = self;
        let meshed: Vec<ChunkMesh> = if threads <= 1 {
            owed.iter()
                .map(|&i| me.mesh_chunk(me.chunk_at(i)))
                .collect()
        } else {
            let per = owed.len().div_ceil(threads);
            std::thread::scope(|s| {
                let handles: Vec<_> = owed
                    .chunks(per)
                    .map(|part| {
                        s.spawn(move || {
                            part.iter()
                                .map(|&i| me.mesh_chunk(me.chunk_at(i)))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .flat_map(|h| h.join().expect("a meshing thread"))
                    .collect()
            })
        };
        let mut changed = Vec::new();
        for m in meshed {
            let mut h = rustc_hash::FxHasher::default();
            m.terrain.hash_into(&mut h);
            0xA7u8.hash(&mut h);
            m.water.hash_into(&mut h);
            let hash = h.finish();
            let i = self.chunk_index(m.at);
            if self.reported[i] != Some(hash) {
                self.reported[i] = Some(hash);
                changed.push(m);
            }
        }
        changed
    }

    /// One chunk's terrain and water meshes from the grid as it stands.
    pub fn mesh_chunk(&self, at: [usize; 3]) -> ChunkMesh {
        let mut terrain = MeshData::default();
        let mut water = MeshData::default();
        for dir in DIRS {
            if self.faces & dir.bit() != 0 {
                self.terrain_faces(at, dir, &mut terrain);
                self.water_faces(at, dir, &mut water);
            }
        }
        ChunkMesh { at, terrain, water }
    }

    /// The world cell at a chunk's local `(layer along the normal, u, v)`.
    #[inline]
    fn cell(o: [usize; 3], dir: Dir, layer: usize, u: usize, v: usize) -> [i64; 3] {
        let (ua, va) = dir.plane();
        let mut c = [0i64; 3];
        c[dir.axis()] = (o[dir.axis()] + layer) as i64;
        c[ua] = (o[ua] + u) as i64;
        c[va] = (o[va] + v) as i64;
        c
    }

    fn terrain_faces(&self, at: [usize; 3], dir: Dir, out: &mut MeshData) {
        let g = &self.grid;
        let o = at.map(|c| c * CHUNK);
        let s = dir.step();
        let mut mask = [0u32; CHUNK * CHUNK];
        for layer in 0..CHUNK {
            let mut any = false;
            for v in 0..CHUNK {
                for u in 0..CHUNK {
                    let [x, y, z] = Self::cell(o, dir, layer, u, v);
                    let key = match g.at(x, y, z) {
                        Some(i)
                            if g.material[i] != 0
                                && !g.solid(g.at(x + s[0], y + s[1], z + s[2])) =>
                        {
                            u32::from(g.material[i]) * (WET_LEVELS + 1) + wet_level(g.pore[i]) + 1
                        }
                        _ => 0,
                    };
                    mask[v * CHUNK + u] = key;
                    any |= key != 0;
                }
            }
            if !any {
                continue;
            }
            let plane = layer as f32 + if dir.positive() { 1.0 } else { 0.0 };
            self.greedy_rects(
                &mut mask,
                |_| true,
                |u, v, du, dv, key| {
                    let k = key - 1;
                    let material = (k / (WET_LEVELS + 1)) as usize;
                    let level = k % (WET_LEVELS + 1);
                    let base = mix(
                        self.look.material[material.min(3)],
                        self.look.water_deep,
                        self.look.wet * level as f32 / WET_LEVELS as f32,
                    );
                    self.emit(out, at, dir, plane, [u, v, du, dv], (0.0, 1.0), base, 1.0);
                },
            );
        }
    }

    fn water_faces(&self, at: [usize; 3], dir: Dir, out: &mut MeshData) {
        let g = &self.grid;
        let o = at.map(|c| c * CHUNK);
        let s = dir.step();
        // Keys: a top face by its height, a side face by its low and high ends, a bottom 1.
        const STEPS: f32 = 255.0;
        let q = |h: f32| (h * STEPS).round() as u32;
        let mut mask = [0u32; CHUNK * CHUNK];
        for layer in 0..CHUNK {
            let mut any = false;
            for v in 0..CHUNK {
                for u in 0..CHUNK {
                    let [x, y, z] = Self::cell(o, dir, layer, u, v);
                    let h = g.water_height(x, y, z);
                    let mut key = 0;
                    if h > 0.0 {
                        let n = g.at(x + s[0], y + s[1], z + s[2]);
                        key = match dir {
                            Dir::PosY => {
                                let open = !g.solid(n) && n.is_none_or(|n| g.free[n] == 0);
                                if open { q(h) + 1 } else { 0 }
                            }
                            Dir::NegY => {
                                let open = !g.solid(n) && n.is_none_or(|n| g.free[n] == 0);
                                if open { 1 } else { 0 }
                            }
                            _ => {
                                if g.solid(n) {
                                    0
                                } else {
                                    let hn = g.water_height(x + s[0], y + s[1], z + s[2]);
                                    if hn < h { ((q(hn) << 9) | q(h)) + 1 } else { 0 }
                                }
                            }
                        };
                    }
                    mask[v * CHUNK + u] = key;
                    any |= key != 0;
                }
            }
            if !any {
                continue;
            }
            let full = ((0u32 << 9) | q(1.0)) + 1;
            match dir {
                Dir::PosY => self.greedy_rects(
                    &mut mask,
                    |_| true,
                    |u, v, du, dv, key| {
                        let h = (key - 1) as f32 / STEPS;
                        let c = self.look.water_surface;
                        self.emit(
                            out,
                            at,
                            dir,
                            layer as f32 + h,
                            [u, v, du, dv],
                            (0.0, 1.0),
                            c,
                            self.look.top_alpha,
                        );
                    },
                ),
                Dir::NegY => self.greedy_rects(
                    &mut mask,
                    |_| true,
                    |u, v, du, dv, _| {
                        let c = self.look.water_deep;
                        self.emit(
                            out,
                            at,
                            dir,
                            layer as f32,
                            [u, v, du, dv],
                            (0.0, 1.0),
                            c,
                            self.look.body_alpha,
                        );
                    },
                ),
                _ => {
                    let plane = layer as f32 + if dir.positive() { 1.0 } else { 0.0 };
                    // A side face stacks in `y` only between whole-cell faces.
                    self.greedy_rects(
                        &mut mask,
                        |k| k == full,
                        |u, v, du, dv, key| {
                            let k = key - 1;
                            let (lo, hi) = ((k >> 9) as f32 / STEPS, (k & 0x1FF) as f32 / STEPS);
                            let c = self.look.water_deep;
                            self.emit(
                                out,
                                at,
                                dir,
                                plane,
                                [u, v, du, dv],
                                (lo, hi),
                                c,
                                self.look.body_alpha,
                            );
                        },
                    );
                }
            }
        }
    }

    /// Cover the mask's non-zero keys with rectangles of one key each (one per cell when
    /// greedy merging is off), calling `emit(u, v, du, dv, key)`. A rectangle grows in `v`
    /// only for keys `stack` allows.
    fn greedy_rects(
        &self,
        mask: &mut [u32; CHUNK * CHUNK],
        stack: impl Fn(u32) -> bool,
        mut emit: impl FnMut(usize, usize, usize, usize, u32),
    ) {
        for v in 0..CHUNK {
            let mut u = 0;
            while u < CHUNK {
                let key = mask[v * CHUNK + u];
                if key == 0 {
                    u += 1;
                    continue;
                }
                let mut du = 1;
                let mut dv = 1;
                if self.greedy {
                    while u + du < CHUNK && mask[v * CHUNK + u + du] == key {
                        du += 1;
                    }
                    if stack(key) {
                        'grow: while v + dv < CHUNK {
                            for i in 0..du {
                                if mask[(v + dv) * CHUNK + u + i] != key {
                                    break 'grow;
                                }
                            }
                            dv += 1;
                        }
                    }
                }
                for j in 0..dv {
                    for i in 0..du {
                        mask[(v + j) * CHUNK + u + i] = 0;
                    }
                }
                emit(u, v, du, dv, key);
                u += du;
            }
        }
    }

    /// One quad: on the plane `plane` (chunk-local, along the face's normal axis), over
    /// `[u, u + du]` and in `v` from `v + lo` to `v + dv − 1 + hi`, hazed per vertex.
    #[allow(clippy::too_many_arguments)]
    fn emit(
        &self,
        out: &mut MeshData,
        at: [usize; 3],
        dir: Dir,
        plane: f32,
        [u, v, du, dv]: [usize; 4],
        (lo, hi): (f32, f32),
        base: Linear,
        alpha: f32,
    ) {
        let (ua, va) = dir.plane();
        let (u0, u1) = (u as f32, (u + du) as f32);
        let (v0, v1) = (v as f32 + lo, (v + dv - 1) as f32 + hi);
        let corner = |cu: f32, cv: f32| {
            let mut p = [0f32; 3];
            p[dir.axis()] = plane;
            p[ua] = cu;
            p[va] = cv;
            p
        };
        let local = [
            corner(u0, v0),
            corner(u1, v0),
            corner(u1, v1),
            corner(u0, v1),
        ];
        let depth = self.grid.dims[2].max(2) as f32 - 1.0;
        let z0 = (at[2] * CHUNK) as f32;
        let colors = local.map(|p| {
            let k = self.look.haze_amount * ((z0 + p[2]) / depth).clamp(0.0, 1.0);
            let c = mix(base, self.look.haze, k);
            [c[0], c[1], c[2], alpha]
        });
        let godot = local.map(|p| [p[0], p[1], -p[2]]);
        out.quad(godot, dir.godot_normal(), colors);
    }
}

// --- the Godot class -------------------------------------------------------------------

/// **`VoxelMesher`**: the view feed's terrain and water chunks as `ArrayMesh`es, remeshing
/// only what changed. `load_palette` once, then `apply_view(view)` with each view (from
/// `CubariumSession.take_view()` or `candidate_view(i)`): it returns the chunks whose
/// meshes changed, each `{at: Vector3i, terrain: ArrayMesh or null, water: ArrayMesh or
/// null}`. A view with `full` set, a new `generation` or new `dims` starts over and
/// reports every chunk.
#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct VoxelMesher {
    look: Option<Look>,
    mesher: Option<Mesher>,
    generation: i64,
    view_faces_only: bool,
    wrap_x: bool,
    last_ms: f64,
    last_meshed: i64,
}

#[godot_api]
impl IRefCounted for VoxelMesher {
    fn init(_base: Base<RefCounted>) -> Self {
        VoxelMesher {
            look: None,
            mesher: None,
            generation: -1,
            view_faces_only: true,
            wrap_x: true,
            last_ms: 0.0,
            last_meshed: 0,
        }
    }
}

#[godot_api]
impl VoxelMesher {
    /// Read the colours (`game/world/data/palette.json`, a filesystem path). "" or why not.
    #[func]
    fn load_palette(&mut self, path: GString) -> GString {
        match PaletteFile::load(&path.to_string()).and_then(|p| Look::from_file(&p)) {
            Ok(look) => {
                self.look = Some(look);
                self.mesher = None;
                GString::new()
            }
            Err(e) => GString::from(e.as_str()),
        }
    }

    /// Mesh only the faces the view's camera sees (tops and fronts; the default), or all six.
    #[func]
    fn set_view_faces_only(&mut self, on: bool) {
        self.view_faces_only = on;
        self.mesher = None;
    }

    /// Whether `x` wraps round the ring at the seam (the default), so faces there are culled
    /// against the other side.
    #[func]
    fn set_wrap_x(&mut self, on: bool) {
        self.wrap_x = on;
        self.mesher = None;
    }

    /// Forget everything: the next view reports every chunk.
    #[func]
    fn reset(&mut self) {
        self.mesher = None;
        self.generation = -1;
    }

    /// Milliseconds the last `apply_view` spent writing, meshing and building meshes.
    #[func]
    fn last_mesh_ms(&self) -> f64 {
        self.last_ms
    }

    /// Chunks the last `apply_view` reported.
    #[func]
    fn last_meshed(&self) -> i64 {
        self.last_meshed
    }

    /// Take one view; return the chunks whose meshes changed.
    #[func]
    fn apply_view(&mut self, view: VarDictionary) -> VarArray {
        let started = std::time::Instant::now();
        let mut out = VarArray::new();
        let Some(look) = self.look.clone() else {
            godot_error!("VoxelMesher: load_palette first");
            return out;
        };
        let Some(dims) = view.get("dims").and_then(|v| v.try_to::<Vector3i>().ok()) else {
            return out;
        };
        let dims = [
            dims.x.max(0) as usize,
            dims.y.max(0) as usize,
            dims.z.max(0) as usize,
        ];
        let generation = view
            .get("generation")
            .and_then(|v| v.try_to::<i64>().ok())
            .unwrap_or(0);
        let full = view
            .get("full")
            .and_then(|v| v.try_to::<bool>().ok())
            .unwrap_or(false);
        let stale = self.mesher.as_ref().is_none_or(|m| m.grid.dims != dims);
        if stale || full || generation != self.generation {
            let mut m = Mesher::new(dims, look, self.wrap_x);
            m.faces = if self.view_faces_only {
                VIEW_FACES
            } else {
                ALL_FACES
            };
            m.mark_all();
            self.mesher = Some(m);
            self.generation = generation;
        }
        let mesher = self.mesher.as_mut().expect("a mesher");
        let coords = view
            .get("chunk_coords")
            .and_then(|v| v.try_to::<PackedInt32Array>().ok())
            .unwrap_or_default();
        let bytes = view
            .get("voxels")
            .and_then(|v| v.try_to::<PackedByteArray>().ok())
            .unwrap_or_default();
        let (coords, bytes) = (coords.as_slice(), bytes.as_slice());
        for (k, c) in coords.chunks_exact(3).enumerate() {
            let Some(b) = bytes.get(k * 3 * CELLS..(k + 1) * 3 * CELLS) else {
                break;
            };
            if c.iter().all(|&v| v >= 0) {
                mesher.write_chunk([c[0] as usize, c[1] as usize, c[2] as usize], b);
            }
        }
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(8);
        let changed = mesher.flush(threads);
        self.last_meshed = changed.len() as i64;
        for m in &changed {
            let mut d = VarDictionary::new();
            d.set(
                "at",
                Vector3i::new(m.at[0] as i32, m.at[1] as i32, m.at[2] as i32),
            );
            d.set(
                "terrain",
                &array_mesh(&m.terrain).map_or(Variant::nil(), |g| g.to_variant()),
            );
            d.set(
                "water",
                &array_mesh(&m.water).map_or(Variant::nil(), |g| g.to_variant()),
            );
            out.push(&d.to_variant());
        }
        self.last_ms = started.elapsed().as_secs_f64() * 1000.0;
        out
    }
}

/// An `ArrayMesh` of one surface, or `None` for empty arrays.
pub fn array_mesh(m: &MeshData) -> Option<Gd<ArrayMesh>> {
    if m.is_empty() {
        return None;
    }
    let v3 = |p: &[f32; 3]| Vector3::new(p[0], p[1], p[2]);
    let pos: Vec<Vector3> = m.pos.iter().map(v3).collect();
    let normal: Vec<Vector3> = m.normal.iter().map(v3).collect();
    let color: Vec<Color> = m
        .color
        .iter()
        .map(|c| Color::from_rgba(c[0], c[1], c[2], c[3]))
        .collect();
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    arrays.set(
        ArrayType::VERTEX.ord() as usize,
        &PackedVector3Array::from(pos.as_slice()).to_variant(),
    );
    arrays.set(
        ArrayType::NORMAL.ord() as usize,
        &PackedVector3Array::from(normal.as_slice()).to_variant(),
    );
    arrays.set(
        ArrayType::COLOR.ord() as usize,
        &PackedColorArray::from(color.as_slice()).to_variant(),
    );
    arrays.set(
        ArrayType::INDEX.ord() as usize,
        &PackedInt32Array::from(m.index.as_slice()).to_variant(),
    );
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
    Some(mesh)
}

#[cfg(test)]
#[path = "mesher_tests.rs"]
mod tests;
