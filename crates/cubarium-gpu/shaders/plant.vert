#version 450
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

#include "plant.glsl"

layout(location = 0) flat out uint vId;
// Across the face 0..1, and down it from its top edge (a front) or its far edge (a cap).
layout(location = 1) out vec2 vUV;

const vec2 CORNER[6] = vec2[](vec2(0, 0), vec2(1, 0), vec2(1, 1), vec2(0, 0), vec2(1, 1), vec2(0, 1));

void cull() {
    gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
    vId = 0u;
    vUV = vec2(0.0);
}

void main() {
    int n = pc.k.x;
    int inst = gl_InstanceIndex;
    bool copy = inst >= n;
    if (copy) { inst -= n; }
    uvec4 c = cells[inst];
    float k = plantScale(c);
    if (k <= 0.0) { cull(); return; }
    int face = gl_VertexIndex / 6;
    vec2 ab = CORNER[gl_VertexIndex % 6];
    float S = float(pc.geom.x);
    float R = float(pc.geom.y);
    float B = float(pc.geom.z);
    vec3 ctr;
    float h;
    plantBox(c, ctr, h);
    float zf = ctr.z - h;
    float top;
    float bottom;
    float d0;
    float d1;
    if (face == 0) {
        top = B - (ctr.y + h) * S - zf * R;
        bottom = B - (ctr.y - h) * S - zf * R;
        d0 = zf;
        d1 = zf;
    } else {
        float yt = ctr.y + h;
        top = B - yt * S - (zf + 2.0 * h) * R;
        bottom = B - yt * S - zf * R;
        d0 = zf + 2.0 * h;
        d1 = zf;
    }
    float width = 2.0 * h * S;
    // A drape cord's long wave (`drape_cord.glsl`): a drape cell's front quad (host flag, bit
    // 30 of its faces word; a leaving cell's in case) reaches `pc.t.w` voxels either side of
    // its centre, so the cord shows where it swings into the next columns.
    vec2 span = vec2(0.0, 1.0);
    if (face == 0 && pc.t.w > 0.5 && (plantDying(c) || (c.w & 0xC0000000u) == 0xC0000000u)) {
        span = vec2(0.5 - pc.t.w, 0.5 + pc.t.w);
    }
    float rw = pc.layer.z;
    float rel = (ctr.x - h) * S - float(pc.k.z);
    float left = rel - rw * floor(rel / rw);
    if (copy) {
        if (left + span.y * width > rw) {
            left -= rw;
        } else if (left + span.x * width < 0.0) {
            left += rw;
        } else {
            cull();
            return;
        }
    }
    ab.x = mix(span.x, span.y, ab.x);
    vec2 px = vec2(left + ab.x * width, mix(top, bottom, ab.y));
    float aa = float(pc.k.y);
    gl_Position = vec4(px * aa / pc.layer.xy * 2.0 - 1.0, plantDepth(mix(d0, d1, ab.y)), 1.0);
    vId = uint(inst) << 1 | uint(face);
    vUV = ab;
}
