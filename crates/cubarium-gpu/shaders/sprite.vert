#version 450
#include "scene.glsl"
// One instanced quad per stamp. The quad is built in the tile's *own* coordinates and
// rotated by the instance heading, so it is tight around the sprite (plus the bend's
// reach) rather than a conservative axis-aligned box: ~1.2x overdraw instead of 2.25x.
//
// Nothing is interpolated. Every varying is `flat`, and the fragment recomputes its own
// tile coordinate from `gl_FragCoord` — see `sprite.frag`.

layout(location = 0) in vec2  iAnchor;
layout(location = 1) in vec2  iHeading;
layout(location = 2) in uvec4 iFrames01;   // frame0.xy, frame1.xy
layout(location = 3) in uvec4 iFrames23;   // frame2.xy, frame3.xy
layout(location = 4) in uvec4 iSizePivot;  // size.xy, pivot.xy
layout(location = 5) in vec4  iWeights;
layout(location = 6) in vec4  iBend;       // amplitude, base, root, length
layout(location = 7) in vec4  iMask;       // floor, reveal, flags, opacity
layout(location = 8) in vec4  iTone;       // colour.rgb, mix
layout(location = 9) in vec4  iShade;      // shade floor, reference, scale, source

layout(location = 0) flat out vec4  vPlace;      // the snapped anchor, then the heading
layout(location = 1) flat out uvec4 vFrames01;
layout(location = 2) flat out uvec4 vFrames23;
layout(location = 3) flat out uvec4 vSizePivot;
layout(location = 4) flat out vec4  vWeights;
layout(location = 5) flat out vec4  vBend;
layout(location = 6) flat out vec4  vMask;
layout(location = 7) flat out vec4  vTone;
layout(location = 8) flat out vec4  vShade;

void main() {
    vec2 pivot = vec2(iSizePivot.zw);
    vec2 tile  = vec2(iSizePivot.xy);
    // The bend displaces along tile +x only, so only x needs the headroom. One extra
    // texel each way covers the half-open edge of the nearest-neighbour footprint.
    float pad = abs(iBend.x) + 1.0;
    vec2 lo = vec2(-pivot.x - pad, -pivot.y - 1.0);
    vec2 hi = vec2(tile.x - pivot.x + pad, tile.y - pivot.y + 1.0);
    // The stamp's own scale (1, or 0.7 for a juvenile) multiplies the world's S.
    float stampScale = u.time.w * max(iShade.z, 1e-3);
    vec2 corner = vec2((gl_VertexIndex & 1) == 0 ? lo.x : hi.x,
                       (gl_VertexIndex & 2) == 0 ? lo.y : hi.y);

    // The anchor lands on a whole raster pixel: pixel art has no sub-pixel positions.
    // Everything else follows from that, including the S x S blocks in the fragment.
    // `--gpu-filter bilinear` keeps the sub-pixel anchor instead, because the point of
    // that mode is to be the CPU presenter's stamp and the CPU does not snap.
    vec2 anchor = u.time.z > 0.5 ? iAnchor : floor(iAnchor) + 0.5;
    vec2 side = vec2(-iHeading.y, iHeading.x);
    vec2 px = anchor + stampScale * (corner.x * iHeading + corner.y * side);

    vPlace = vec4(anchor, iHeading);
    vFrames01 = iFrames01;
    vFrames23 = iFrames23;
    vSizePivot = iSizePivot;
    vWeights = iWeights;
    vBend = iBend;
    vMask = iMask;
    vTone = iTone;
    vShade = iShade;
    gl_Position = vec4(px * u.raster.zw * 2.0 - 1.0, 0.0, 1.0);
}
