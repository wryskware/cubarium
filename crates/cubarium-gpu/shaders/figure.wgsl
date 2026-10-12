// The model pass's shared declarations (H2, `crates/cubarium-gpu/src/figures.rs`): one
// tile per drawn model, the figures that composite them, and the push block. `Tile` and
// `Fig` are `figures::TileGpu` and `figures::FigGpu`, field for field.

struct FigurePush {
    k: vec4<i32>,      // tile index (tile pass) | raster width (composite), s, rise, raster height
    atlas: vec4<f32>,  // atlas width, height, the depth test's bias (voxels), -
}
var<immediate> pc: FigurePush;

struct Tile {
    pos: vec4<f32>,       // P, the model's anchor in the world (voxels); w: unused
    ax: vec4<f32>,        // the model's x axis in the world; w: the body's settle (voxels)
    ay: vec4<f32>,        // its y axis; w: the breath's dip (voxels)
    az: vec4<f32>,        // its z axis; w: the head's settle (voxels)
    rect: vec4<i32>,      // its place in the atlas: x, y, width, height
    orig: vec4<i32>,      // the screen offset from P's projection to its texel (0, 0); its first
                          // palette entry; -
    depth: vec4<f32>,     // the in-tile depth: z at depth 0.5 (P's), 1 / span; the head's breath; -
    faceSize: vec4<i32>,  // projected lengths of local x/y/z axes in raster pixels; samples per axis
    rig: vec4<i32>,       // FEED-RIG: its first bone in `bones` (-1: none); -, -, -
}

struct Fig {
    anchor: vec4<f32>,  // P projected into the raster (view applied); tile B's weight; the fade
    tiles: vec4<i32>,   // tile A, tile B (-1: none), -, -
    k: vec4<f32>,       // how far it leans toward white (the highlight), -, -, -
}

@group(1) @binding(0) var<storage, read> vox: array<vec4<u32>>;
@group(1) @binding(1) var<storage, read> tiles: array<Tile>;
@group(1) @binding(2) var<storage, read> figs: array<Fig>;
// FEED-RIG: the rigged tiles' bone transforms, three rows `[m0, m1, m2, t]` a bone, in the
// model's own frame (voxels): a rest point p goes to M p + t.
@group(1) @binding(4) var<storage, read> bones: array<vec4<f32>>;

// The bone a rigged tile's cell (`FigureVoxel.bits` 14..19) moves by, as an index of its
// first row in `bones`; -1 for a tile drawn as baked.
fn boneRow(t: Tile, bits: u32) -> i32 {
    return select(3 * (t.rig.x + i32((bits >> 14u) & 63u)), -1, t.rig.x < 0);
}

fn boneMove(row: i32, p: vec3<f32>) -> vec3<f32> {
    let q = vec4(p, 1.0);
    return vec3(dot(bones[row], q), dot(bones[row + 1], q), dot(bones[row + 2], q));
}

fn boneTurn(row: i32, v: vec3<f32>) -> vec3<f32> {
    return vec3(dot(bones[row].xyz, v), dot(bones[row + 1].xyz, v), dot(bones[row + 2].xyz, v));
}

// A point p of a rigged cell, posed: the whole bone transform, so the cube turns with its
// part; unmoved for a tile drawn as baked.
fn bonePose(row: i32, p: vec3<f32>) -> vec3<f32> {
    if (row < 0) { return p; }
    return boneMove(row, p);
}

// Local face axes, also used when baking each face's open-plane crease neighbours.
// Faces: -x, +x, -y, +y, -z, +z (the open mask's bits, in this order).
const FACE_O = array(vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 0.0), vec3(1.0, 0.0, 1.0));
const FACE_U = array(vec3(0.0, 0.0, -1.0), vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(-1.0, 0.0, 0.0));
const FACE_V = array(vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, -1.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0));
const FACE_N = array(vec3(-1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, -1.0), vec3(0.0, 0.0, 1.0));

// Two triangles of the unit square.
const CORNER = array(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0));

// Byte `at` of `v`, sign-extended.
fn sbyte(v: u32, at: u32) -> i32 { return extractBits(i32(v), at, 8u); }
