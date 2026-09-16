#version 450
#include "scene.glsl"
// One instanced quad per stamp. The quad is built in the tile's *own* coordinates and
// rotated by the instance heading, so it is tight around the sprite rather than a
// conservative axis-aligned box. GS-1c narrowed it twice more: around the frames'
// **opaque** box instead of the whole tile, and clipped by the mask's own reveal.
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
layout(location = 10) in vec4 iBox;        // opaque box in source texels: x0, y0, x1, y1

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
    // The quad is built around the union of the live frames' **opaque** boxes, not
    // around the whole tile: 82 % of the pack's atlas texels are clear, and at
    // `--gpu-art-scale 2` on a 640x360 S=2 raster a whole 16x16 tile is a 64x64 quad.
    // A degenerate or missing box (a scratch frame, an instance nothing filled in)
    // falls back to the tile, which is the quad this shader always drew.
    vec2 b0 = iBox.xy;
    vec2 b1 = iBox.zw;
    if (!(b1.x > b0.x && b1.y > b0.y)) { b0 = vec2(0.0); b1 = tile; }

    // The mask cannot paint outside the rows it reveals, so it clips the box too.
    // `sprite.frag`'s cover is `unit(reveal - h + 0.5) * unit(h - floor + 0.5)` with
    // `h = tile.y - src.y`, which is positive only for
    // `src.y` in `[tile.y - reveal - 0.5, tile.y - floor + 0.5]`. `NO_MASK_REVEAL` and
    // `NO_MASK_FLOOR` are +/-1e9, so an unmasked stamp's clamp is a no-op by arithmetic
    // rather than by a branch. A radial mask reveals a disc of `reveal + 0.5` about the
    // pivot instead, on both axes.
    if (iMask.z > 0.5) {
        float r = max(iMask.y, 0.0) + 0.5;
        b0 = max(b0, pivot - vec2(r));
        b1 = min(b1, pivot + vec2(r));
    } else {
        b0.y = max(b0.y, tile.y - iMask.y - 0.5);
        b1.y = min(b1.y, tile.y - iMask.x + 0.5);
    }

    // How far past the box a fragment can still read a texel inside it.
    //
    // Nearest (`u.time.z <= 0.5`): the fragment's own rule is `floor(src)`, so a
    // fragment paints iff `src` is *in* `[b0, b1)` — nothing outside the box can read
    // anything inside it. The row is never displaced, so `y` needs no pad at all; `x`
    // needs the bend, and `round(d)` can overshoot `|amplitude|` by half a texel.
    //
    // EPS is slack for the **rasteriser's sub-pixel grid**, not for float error: Vulkan
    // guarantees only 8 sub-pixel bits, so a quad edge sitting exactly on a pixel centre
    // can snap 1/256 of a pixel the wrong way and lose that whole row of the sprite.
    // (Measured: at 0.001 texels the real ring world's mean |delta| against the CPU
    // moved 15.996 -> 16.003, which is that row.) A twentieth of a source texel is at
    // least 12 sub-pixel units at every stamp scale this renderer draws, and at most a
    // tenth of a raster pixel, so it can add a fragment and can never add a *texel*:
    // anything outside the box still samples clear and discards.
    //
    // Bilinear (`--gpu-filter bilinear`): a sample up to half a texel outside the box
    // still takes part of the edge texel through the filter, so it keeps the whole
    // texel of support on both axes that this shader always had. That mode is the
    // fidelity comparison and its quad is deliberately unchanged.
    const float EPS = 0.05;
    bool filtered = u.time.z > 0.5;
    // The bend's own reach, exactly as the fragment computes it: nothing at all when
    // the stamp carries no bend or no length (which is every ground tile and every rain
    // mark), `|amplitude|` when the displacement lands between texels, and half a texel
    // more when it is rounded to a whole one, because `round` can overshoot.
    float reach = (iBend.x != 0.0 && iBend.w > 0.0)
        ? abs(iBend.x) + (u.knobs.w < 0.5 ? 0.5 : 0.0)
        : 0.0;
    float padX = reach + (filtered ? 1.0 : EPS);
    float padY = filtered ? 1.0 : EPS;
    vec2 lo = vec2(b0.x - pivot.x - padX, b0.y - pivot.y - padY);
    vec2 hi = vec2(b1.x - pivot.x + padX, b1.y - pivot.y + padY);
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
