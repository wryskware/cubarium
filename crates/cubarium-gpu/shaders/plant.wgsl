// The plant pass's shared declarations (`crates/cubarium-gpu/src/plants.rs`): the push block,
// the frame's plant cells and each plant's tip. `plants::PlantPush` field for field; a cell is
// `plants::pack_cell`'s four words.

struct PlantPush {
    geom: vec4<i32>,   // s, rise, base, the world's width (voxels)
    k: vec4<i32>,      // cells drawn, layer samples per raster pixel axis, the view's turn (raster
                       // columns), the world's depth (voxels)
    t: vec4<f32>,      // seconds since the cell list's epoch, a voxel's grow time (s), the depth
                       // test's bias (voxels), a drape cell's front quad's reach (voxels, vis only)
    layer: vec4<f32>,  // the layer's width and height (texels), the raster's width and height
}
var<immediate> pc: PlantPush;

@group(1) @binding(0) var<storage, read> cells: array<vec4<u32>>;
@group(1) @binding(1) var<storage, read> tips: array<f32>;

// A cell's voxel: x | y << 12 | z << 22.
fn plantCellAt(c: vec4<u32>) -> vec3<i32> {
    return vec3(i32(c.x & 4095u), i32((c.x >> 12u) & 1023u), i32(c.x >> 22u));
}
// Its plant (an index into `tips`), whether it is leaving, and how much of its plant's tip
// it moves by (0 at the root, 1 at the top).
fn plantOf(c: vec4<u32>) -> u32 { return c.y & 0x7FFFFu; }
fn plantDying(c: vec4<u32>) -> bool { return (c.y & 0x80000u) != 0u; }
fn plantWeight(c: vec4<u32>) -> f32 { return f32(c.y >> 20u) / 4095.0; }
// A leaving cell's texel as it was stamped (it is no longer in the volume).
fn plantCarried(c: vec4<u32>) -> vec4<u32> {
    return vec4(c.w & 255u, (c.w >> 8u) & 255u, (c.w >> 16u) & 255u, c.w >> 24u);
}

// How big the cell is drawn, 0..1, about its own centre: an arriving cell grows from its
// time (`c.z`, seconds since the epoch) over the grow time, a leaving one shrinks from its
// time; smoothstepped, so a voxel starts and ends its change at rest.
fn plantScale(c: vec4<u32>) -> f32 {
    let since = pc.t.x - bitcast<f32>(c.z);
    var r: f32;
    if (pc.t.y > 0.0) {
        r = clamp(since / pc.t.y, 0.0, 1.0);
    } else {
        r = select(0.0, 1.0, since >= 0.0);
    }
    let s = r * r * (3.0 - 2.0 * r);
    return select(s, 1.0 - s, plantDying(c));
}

// The cell's box this frame: its centre (moved sideways by its plant's tip times its
// weight) and half its edge (its scale).
fn plantBox(c: vec4<u32>, ctr: ptr<function, vec3<f32>>, h: ptr<function, f32>) {
    *h = 0.5 * plantScale(c);
    *ctr = vec3<f32>(plantCellAt(c)) + 0.5;
    (*ctr).x += tips[plantOf(c)] * plantWeight(c);
}

// Where raster row `py` (a pixel's centre) meets the cell's front face or cap, voxels into
// the strip: the front face's plane, or the cap's z at that row, as the walk's `zHit`.
fn plantFaceZ(c: vec4<u32>, cap: bool, py: f32) -> f32 {
    var ctr: vec3<f32>;
    var h: f32;
    plantBox(c, &ctr, &h);
    let zf = ctr.z - h;
    if (!cap) { return zf; }
    let R = f32(pc.geom.y);
    let bottom = f32(pc.geom.z) - (ctr.y + h) * f32(pc.geom.x) - zf * R;
    return zf + clamp((bottom - py) / R, 0.0, 2.0 * h);
}

// Depth as the figures' composite keeps it (`composite.frag.wgsl`): z voxels into the strip.
fn plantDepth(z: f32) -> f32 { return clamp((z + 16.0) / (f32(pc.k.w) + 32.0), 0.0, 1.0); }
fn plantZ(depth: f32) -> f32 { return depth * (f32(pc.k.w) + 32.0) - 16.0; }
