#version 450
// The model pass's composite (H2 of `design/handoffs/hybrid-renderer-2026-10-07.md`): each
// figure's tile placed at its anchor's exact projection, over the walk's raster.
//
// **Sub-pixel placement.** A raster pixel's footprint covers up to four tile texels at a
// fractional anchor; each counts by the share of the footprint it covers (the box filter
// the window's present pass uses), so the figure glides between pixels with the tile's own
// pixels intact, as the camera's sub-pixel pan does.
//
// **Depth.** Each texel carries the z of the face it shows; it counts only where that is
// in front of the walk's own hit at this pixel (`voxel.frag`'s `depthOut`), so terrain and
// plants hide a figure behind them and a figure hides what is behind it.
//
// **Figures over figures.** Each fragment writes the nearest z of its texels as its depth,
// and the composite keeps the nearest (`figures.rs` draws them back to front as well, so a
// figure's soft edge blends over the one behind it).
//
// **Cross-fades.** A figure may name two tiles, weighted: a pose's model swap, or a turn
// between two of the fine angle steps. Each texel of either counts by its weight, so the
// two blend pixel by pixel. Birth and death fade the whole figure; a highlight leans it
// toward white. Out goes premultiplied colour over the raster, and the emitters' share
// over the emission (its alpha, the weather's depth, kept).

#include "figure.glsl"

layout(set = 2, binding = 0) uniform sampler2D atlasColour;
layout(set = 2, binding = 1) uniform sampler2D atlasEmit;
layout(set = 2, binding = 2) uniform sampler2D atlasZ;
layout(set = 2, binding = 3) uniform sampler2D walkDepth;

layout(location = 0) flat in int vFig;

layout(location = 0) out vec4 outColour;
layout(location = 1) out vec4 outEmit;

// One tile's coverage of this pixel: premultiplied colour and coverage, and emission.
// `zNear` keeps the nearest z of the texels that count.
void tileOver(Tile t, vec2 tc, float wz, float weight, inout vec4 col, inout vec3 em, inout float zNear) {
    vec2 g = tc - 0.5;
    ivec2 i0 = ivec2(floor(g));
    vec2 f = g - vec2(i0);
    for (int j = 0; j < 4; ++j) {
        ivec2 o = ivec2(j & 1, j >> 1);
        ivec2 ti = i0 + o;
        if (any(lessThan(ti, ivec2(0))) || any(greaterThanEqual(ti, t.rect.zw))) { continue; }
        float w = (o.x == 1 ? f.x : 1.0 - f.x) * (o.y == 1 ? f.y : 1.0 - f.y) * weight;
        if (w <= 0.0) { continue; }
        ivec2 at = t.rect.xy + ti;
        vec4 c = texelFetch(atlasColour, at, 0);
        if (c.a < 0.5) { continue; }
        float tz = texelFetch(atlasZ, at, 0).r;
        if (tz > wz + pc.atlas.z) { continue; }
        zNear = min(zNear, tz);
        col += w * vec4(c.rgb, 1.0);
        em += w * texelFetch(atlasEmit, at, 0).rgb;
    }
}

void main() {
    ivec2 px = ivec2(gl_FragCoord.xy);
    if (px.x < 0 || px.y < 0 || px.x >= pc.k.x || px.y >= pc.k.w) { discard; }
    float wz = texelFetch(walkDepth, px, 0).r;
    Fig f = figs[vFig];
    vec2 c = gl_FragCoord.xy - f.anchor.xy;
    vec4 col = vec4(0.0);
    vec3 em = vec3(0.0);
    float zNear = 3.0e38;
    float wb = f.tiles.y >= 0 ? clamp(f.anchor.z, 0.0, 1.0) : 0.0;
    Tile a = tiles[f.tiles.x];
    tileOver(a, c - vec2(a.orig.xy), wz, 1.0 - wb, col, em, zNear);
    if (f.tiles.y >= 0) {
        Tile b = tiles[f.tiles.y];
        tileOver(b, c - vec2(b.orig.xy), wz, wb, col, em, zNear);
    }
    if (col.a <= 0.0) { discard; }
    // Figures against each other, per pixel: the composite's depth test keeps the nearest
    // figure's texels whatever order the figures are drawn in, by the walk's convention (z,
    // larger is farther; along one pixel's ray z only grows). `pc.atlas.w` is the world's
    // depth; the range has room either side for a body reaching past the strip.
    gl_FragDepth = clamp((zNear + 16.0) / (pc.atlas.w + 32.0), 0.0, 1.0);
    float fade = clamp(f.anchor.w, 0.0, 1.0);
    col *= fade;
    em *= fade;
    col.rgb = mix(col.rgb, vec3(col.a), clamp(f.k.x, 0.0, 1.0));
    outColour = col;
    outEmit = vec4(em, col.a);
}
