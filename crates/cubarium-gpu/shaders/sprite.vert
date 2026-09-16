#version 450
#include "scene.glsl"
// One instanced quad per stamp. The quad is built in the tile's *own* coordinates and
// rotated by the instance heading, so it is tight around the sprite (plus the bend's
// reach) rather than a conservative axis-aligned box: ~1.2x overdraw instead of 2.25x.

layout(location = 0) in vec2  iAnchor;
layout(location = 1) in vec2  iHeading;
layout(location = 2) in uvec4 iFrame0;      // x, y, w, h in atlas texels
layout(location = 3) in uvec4 iFrame1Pivot; // frame1.x, frame1.y, pivot.x, pivot.y
layout(location = 4) in vec2  iBlend;       // mix, opacity
layout(location = 5) in vec4  iBend;        // amplitude, base, root, length
layout(location = 6) in vec2  iMask;        // floor, reveal
layout(location = 7) in vec3  iToneColour;
layout(location = 8) in vec3  iToneShade;   // shade floor, shade reference, tone mix

layout(location = 0) out vec2      vLocal;    // tile coords, relative to the pivot
layout(location = 1) flat out uvec4 vFrame0;
layout(location = 2) flat out uvec4 vFrame1Pivot;
layout(location = 3) flat out vec2  vBlend;
layout(location = 4) flat out vec4  vBend;
layout(location = 5) flat out vec2  vMask;
layout(location = 6) flat out vec3  vToneColour;
layout(location = 7) flat out vec3  vToneShade;

void main() {
    vec2 pivot = vec2(iFrame1Pivot.zw);
    vec2 tile  = vec2(iFrame0.zw);
    // The bend displaces along tile +x only, so only x needs the headroom. One extra
    // texel each way covers the half-open edge of the nearest-neighbour footprint.
    float pad = abs(iBend.x) + 1.0;
    vec2 lo = vec2(-pivot.x - pad, -pivot.y - 1.0);
    vec2 hi = vec2(tile.x - pivot.x + pad, tile.y - pivot.y + 1.0);
    vec2 corner = vec2((gl_VertexIndex & 1) == 0 ? lo.x : hi.x,
                       (gl_VertexIndex & 2) == 0 ? lo.y : hi.y);

    // The anchor lands on a whole raster pixel: pixel art has no sub-pixel positions.
    // Everything else follows from that, including the S x S blocks in the fragment.
    vec2 anchor = floor(iAnchor) + 0.5;
    vec2 side = vec2(-iHeading.y, iHeading.x);
    vec2 px = anchor + u.grid.z * (corner.x * iHeading + corner.y * side);

    vLocal = corner;
    vFrame0 = iFrame0;
    vFrame1Pivot = iFrame1Pivot;
    vBlend = iBlend;
    vBend = iBend;
    vMask = iMask;
    vToneColour = iToneColour;
    vToneShade = iToneShade;
    gl_Position = vec4(px * u.raster.zw * 2.0 - 1.0, 0.0, 1.0);
}
