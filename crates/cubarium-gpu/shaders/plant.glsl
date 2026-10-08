// The plant pass's shared declarations (`crates/cubarium-gpu/src/plants.rs`): the push block,
// the frame's plant cells and each plant's tip. `plants::PlantPush` field for field; a cell is
// `plants::pack_cell`'s four words.

layout(push_constant) uniform PlantPush {
    ivec4 geom;  // s, rise, base, the world's width (voxels)
    ivec4 k;     // cells drawn, layer samples per raster pixel axis, the view's turn (raster
                 // columns), the world's depth (voxels)
    vec4 t;      // seconds since the cell list's epoch, a voxel's grow time (s), the depth
                 // test's bias (voxels), -
    vec4 layer;  // the layer's width and height (texels), the raster's width and height
} pc;

layout(std430, set = 1, binding = 0) readonly buffer PlantCells { uvec4 cells[]; };
layout(std430, set = 1, binding = 1) readonly buffer PlantTips { float tips[]; };

// A cell's voxel: x | y << 12 | z << 22.
ivec3 plantCellAt(uvec4 c) {
    return ivec3(int(c.x & 4095u), int((c.x >> 12) & 1023u), int(c.x >> 22));
}
// Its plant (an index into `tips`), whether it is leaving, and how much of its plant's tip
// it moves by (0 at the root, 1 at the top).
uint plantOf(uvec4 c) { return c.y & 0x7FFFFu; }
bool plantDying(uvec4 c) { return (c.y & 0x80000u) != 0u; }
float plantWeight(uvec4 c) { return float(c.y >> 20) / 4095.0; }
// A leaving cell's texel as it was stamped (it is no longer in the volume).
uvec4 plantCarried(uvec4 c) {
    return uvec4(c.w & 255u, (c.w >> 8) & 255u, (c.w >> 16) & 255u, c.w >> 24);
}

// How big the cell is drawn, 0..1, about its own centre: an arriving cell grows from its
// time (`c.z`, seconds since the epoch) over the grow time, a leaving one shrinks from its
// time; smoothstepped, so a voxel starts and ends its change at rest.
float plantScale(uvec4 c) {
    float since = pc.t.x - uintBitsToFloat(c.z);
    float r = pc.t.y > 0.0 ? clamp(since / pc.t.y, 0.0, 1.0) : (since >= 0.0 ? 1.0 : 0.0);
    float s = r * r * (3.0 - 2.0 * r);
    return plantDying(c) ? 1.0 - s : s;
}

// The cell's box this frame: its centre (moved sideways by its plant's tip times its
// weight) and half its edge (its scale).
void plantBox(uvec4 c, out vec3 ctr, out float h) {
    h = 0.5 * plantScale(c);
    ctr = vec3(plantCellAt(c)) + 0.5;
    ctr.x += tips[plantOf(c)] * plantWeight(c);
}

// Where raster row `py` (a pixel's centre) meets the cell's front face or cap, voxels into
// the strip: the front face's plane, or the cap's z at that row, as the walk's `zHit`.
float plantFaceZ(uvec4 c, bool cap, float py) {
    vec3 ctr;
    float h;
    plantBox(c, ctr, h);
    float zf = ctr.z - h;
    if (!cap) { return zf; }
    float R = float(pc.geom.y);
    float bottom = float(pc.geom.z) - (ctr.y + h) * float(pc.geom.x) - zf * R;
    return zf + clamp((bottom - py) / R, 0.0, 2.0 * h);
}

// Depth as the figures' composite keeps it (`composite.frag`): z voxels into the strip.
float plantDepth(float z) { return clamp((z + 16.0) / (float(pc.k.w) + 32.0), 0.0, 1.0); }
float plantZ(float depth) { return depth * (float(pc.k.w) + 32.0) - 16.0; }
