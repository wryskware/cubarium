#version 450
// The model pass's composite (H2): one quad a figure, covering its tiles where its anchor
// puts them in the raster.

#include "figure.glsl"

layout(location = 0) flat out int vFig;

void main() {
    Fig f = figs[gl_InstanceIndex];
    Tile a = tiles[f.tiles.x];
    vec2 lo = vec2(a.orig.xy);
    vec2 hi = lo + vec2(a.rect.zw);
    if (f.tiles.y >= 0) {
        Tile b = tiles[f.tiles.y];
        lo = min(lo, vec2(b.orig.xy));
        hi = max(hi, vec2(b.orig.xy) + vec2(b.rect.zw));
    }
    // A texel's footprint reaches one raster pixel past its edge at a fractional anchor.
    lo = floor(f.anchor.xy + lo) - 1.0;
    hi = ceil(f.anchor.xy + hi) + 1.0;
    vec2 corner = vec2(float(gl_VertexIndex & 1), float((gl_VertexIndex >> 1) & 1));
    vec2 px = mix(lo, hi, corner);
    gl_Position = vec4(px / vec2(float(pc.k.x), float(pc.k.w)) * 2.0 - 1.0, 0.0, 1.0);
    vFig = gl_InstanceIndex;
}
