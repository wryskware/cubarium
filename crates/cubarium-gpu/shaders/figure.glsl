// The model pass's shared declarations (H2, `crates/cubarium-gpu/src/figures.rs`): one
// tile per drawn model, the figures that composite them, and the push block. `Tile` and
// `Fig` are `figures::TileGpu` and `figures::FigGpu`, field for field.

layout(push_constant) uniform FigurePush {
    ivec4 k;    // tile index (tile pass) | raster width (composite), s, rise, raster height
    vec4 atlas; // atlas width, height, the depth test's bias (voxels), -
} pc;

struct Tile {
    vec4 pos;    // P, the model's anchor in the world (voxels); w: unused
    vec4 ax;     // the model's x axis in the world; w: the body's settle (voxels)
    vec4 ay;     // its y axis; w: the breath's dip (voxels)
    vec4 az;     // its z axis; w: the head's settle (voxels)
    ivec4 rect;  // its place in the atlas: x, y, width, height
    ivec4 orig;  // the screen offset from P's projection to its texel (0, 0); its first
                 // palette entry; -
    vec4 depth;  // the in-tile depth: z at depth 0.5 (P's), 1 / span; the head's breath; -
    ivec4 faceSize; // projected lengths of local x/y/z axes in raster pixels; samples per axis
    ivec4 rig;   // FEED-RIG: its first bone in `bones` (-1: none); -, -, -
};

struct Fig {
    vec4 anchor; // P projected into the raster (view applied); tile B's weight; the fade
    ivec4 tiles; // tile A, tile B (-1: none), -, -
    vec4 k;      // how far it leans toward white (the highlight), -, -, -
};

layout(std430, set = 1, binding = 0) readonly buffer FigureVoxels { uvec4 vox[]; };
layout(std430, set = 1, binding = 1) readonly buffer FigureTiles { Tile tiles[]; };
layout(std430, set = 1, binding = 2) readonly buffer FigureFigs { Fig figs[]; };
// FEED-RIG: the rigged tiles' bone transforms, three rows `[m0, m1, m2, t]` a bone, in the
// model's own frame (voxels): a rest point p goes to M p + t.
layout(std430, set = 1, binding = 4) readonly buffer FigureBones { vec4 bones[]; };

// The bone a rigged tile's cell (`FigureVoxel.bits` 14..19) moves by, as an index of its
// first row in `bones`; -1 for a tile drawn as baked.
int boneRow(Tile t, uint bits) {
    return t.rig.x < 0 ? -1 : 3 * (t.rig.x + int((bits >> 14) & 63u));
}

vec3 boneMove(int row, vec3 p) {
    vec4 q = vec4(p, 1.0);
    return vec3(dot(bones[row], q), dot(bones[row + 1], q), dot(bones[row + 2], q));
}

vec3 boneTurn(int row, vec3 v) {
    return vec3(dot(bones[row].xyz, v), dot(bones[row + 1].xyz, v), dot(bones[row + 2].xyz, v));
}

// A point p of a rigged cell, posed: the whole bone transform, so the cube turns with its
// part; unmoved for a tile drawn as baked.
vec3 bonePose(int row, vec3 p) {
    return row < 0 ? p : boneMove(row, p);
}

// Local face axes, also used when baking each face's open-plane crease neighbours.
const vec3 FACE_O[6] = vec3[](vec3(0, 0, 1), vec3(1, 0, 0), vec3(0, 0, 1), vec3(0, 1, 0), vec3(0, 0, 0), vec3(1, 0, 1));
const vec3 FACE_U[6] = vec3[](vec3(0, 0, -1), vec3(0, 0, 1), vec3(1, 0, 0), vec3(1, 0, 0), vec3(1, 0, 0), vec3(-1, 0, 0));
const vec3 FACE_V[6] = vec3[](vec3(0, 1, 0), vec3(0, 1, 0), vec3(0, 0, -1), vec3(0, 0, 1), vec3(0, 1, 0), vec3(0, 1, 0));
const vec3 FACE_N[6] = vec3[](vec3(-1, 0, 0), vec3(1, 0, 0), vec3(0, -1, 0), vec3(0, 1, 0), vec3(0, 0, -1), vec3(0, 0, 1));
