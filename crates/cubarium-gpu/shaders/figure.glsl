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
};

struct Fig {
    vec4 anchor; // P projected into the raster (view applied); tile B's weight; the fade
    ivec4 tiles; // tile A, tile B (-1: none), -, -
    vec4 k;      // how far it leans toward white (the highlight), -, -, -
};

layout(std430, set = 1, binding = 0) readonly buffer FigureVoxels { uvec2 vox[]; };
layout(std430, set = 1, binding = 1) readonly buffer FigureTiles { Tile tiles[]; };
layout(std430, set = 1, binding = 2) readonly buffer FigureFigs { Fig figs[]; };
