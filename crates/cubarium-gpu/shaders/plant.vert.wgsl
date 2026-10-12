// The plant pass's visibility raster (`crates/cubarium-gpu/src/plants.rs`): one instance a
// plant cell, its front face and its cap as two quads, into the layer (the world raster at
// `pc.k.y` samples a pixel axis). The walk's projection, exactly: a voxel spans `s` columns
// from x·s, its front face the `s` rows above `base − y·s − z·rise`, its cap the `rise` rows
// above that. So a cell at rest covers the very pixels the walk drew it on.
//
// **Sway.** The cell moves whole, sideways, by its plant's tip (`tips`, this frame's) times
// its weight (0 at the root, 1 at the top): a rooted bend, the root never moving.
//
// **Grow-in.** It is drawn scaled about its own centre by `plantScale`: an arriving cell
// grows from nothing, a leaving one shrinks to nothing, each over the grow time from its own
// moment, which the host staggers along the plant.
//
// **The ring.** The view turns the raster `pc.k.z` columns; a cell is placed by its left
// edge round the ring, and the second instance range draws the cells that cross the
// raster's right edge again one ring to the left.

// #import plant.wgsl

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) vId: u32,
    // Across the face 0..1, and down it from its top edge (a front) or its far edge (a cap).
    @location(1) vUV: vec2<f32>,
}

const CORNER = array(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0));

// Off the clip volume.
fn cull() -> Varyings {
    var out: Varyings;
    out.position = vec4(2.0, -2.0, 2.0, 1.0);
    out.vId = 0u;
    out.vUV = vec2(0.0);
    return out;
}

@vertex
fn main(@builtin(instance_index) instanceIndex: u32, @builtin(vertex_index) vertexIndex: u32) -> Varyings {
    let n = pc.k.x;
    var inst = i32(instanceIndex);
    let vertex = i32(vertexIndex);
    let copy = inst >= n;
    if (copy) { inst -= n; }
    let c = cells[inst];
    let k = plantScale(c);
    if (k <= 0.0) { return cull(); }
    let face = vertex / 6;
    var ab = CORNER[vertex % 6];
    let S = f32(pc.geom.x);
    let R = f32(pc.geom.y);
    let B = f32(pc.geom.z);
    var ctr: vec3<f32>;
    var h: f32;
    plantBox(c, &ctr, &h);
    let zf = ctr.z - h;
    var top: f32;
    var bottom: f32;
    var d0: f32;
    var d1: f32;
    if (face == 0) {
        top = B - (ctr.y + h) * S - zf * R;
        bottom = B - (ctr.y - h) * S - zf * R;
        d0 = zf;
        d1 = zf;
    } else {
        let yt = ctr.y + h;
        top = B - yt * S - (zf + 2.0 * h) * R;
        bottom = B - yt * S - zf * R;
        d0 = zf + 2.0 * h;
        d1 = zf;
    }
    let width = 2.0 * h * S;
    // A drape cord's long wave (`drape_cord.wgsl`): a drape cell's front quad (host flag, bit
    // 30 of its faces word; a leaving cell's in case) reaches `pc.t.w` voxels either side of
    // its centre, so the cord shows where it swings into the next columns.
    var span = vec2(0.0, 1.0);
    if (face == 0 && pc.t.w > 0.5 && (plantDying(c) || (c.w & 0xC0000000u) == 0xC0000000u)) {
        span = vec2(0.5 - pc.t.w, 0.5 + pc.t.w);
    }
    let rw = f32(pc.geom.w) * S;
    let rel = (ctr.x - h) * S - f32(pc.k.z);
    var left = rel - rw * floor(rel / rw);
    if (copy) {
        if (left + span.y * width > rw) {
            left -= rw;
        } else if (left + span.x * width < 0.0) {
            left += rw;
        } else {
            return cull();
        }
    }
    ab.x = mix(span.x, span.y, ab.x);
    let px = vec2(left + ab.x * width, mix(top, bottom, ab.y));
    let aa = f32(pc.k.y);
    let position = vec4(px * aa / pc.layer.xy * 2.0 - 1.0, plantDepth(mix(d0, d1, ab.y)), 1.0);
    var out: Varyings;
    // Written for Vulkan's downward clip-space y; WebGPU's points up.
    out.position = vec4(position.x, -position.y, position.zw);
    out.vId = (u32(inst) << 1u) | u32(face);
    out.vUV = ab;
    return out;
}
