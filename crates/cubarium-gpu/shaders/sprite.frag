#version 450
#include "scene.glsl"
// The GPU's `cubarium_render::stamp_layers_bent_toned`, for up to four weighted frames.
//
// Nearest sampling, always: the tile coordinate is floored to a whole source texel, so
// every texel covers an exact S x S block of raster pixels and nothing is filtered. The
// one sub-pixel quantity a stamp carries is the wind's bend, and it is rounded to a whole
// source texel here unless `u.knobs.w` asks for the sub-texel lean (`--gpu-bend-substep`),
// which trades the block rule -- and only for the bend's own displacement -- for a
// smoother breeze at S >= 2.

layout(set = 0, binding = 3) uniform sampler2D atlas;
// The per-frame page the Lanternjaw's freshly rasterised parts are uploaded into,
// already premultiplied and already linear (see `SOURCE_SCRATCH`).
layout(set = 0, binding = 4) uniform sampler2D scratch;

layout(location = 0) flat in vec4  vPlace;
layout(location = 1) flat in uvec4 vFrames01;
layout(location = 2) flat in uvec4 vFrames23;
layout(location = 3) flat in uvec4 vSizePivot;
layout(location = 4) flat in vec4  vWeights;
layout(location = 5) flat in vec4  vBend;
layout(location = 6) flat in vec4  vMask;
layout(location = 7) flat in vec4  vTone;
layout(location = 8) flat in vec4  vShade;

layout(location = 0) out vec4 outColour;

float unit(float v) { return clamp(v, 0.0, 1.0); }

// One frame's premultiplied linear sample. The pack is straight sRGB + alpha, so the
// premultiply happens after the sampler's decode -- exactly `Sprite::from_rgba`'s
// `srgb_decode(c) * a`.
// One texel of one frame, premultiplied and linear, transparent outside the tile —
// exactly `Sprite::pixel`'s out-of-range rule.
vec4 texelOf(uvec2 origin, ivec2 texel, vec2 tile, bool fromScratch) {
    if (texel.x < 0 || texel.y < 0 || texel.x >= int(tile.x) || texel.y >= int(tile.y)) {
        return vec4(0.0);
    }
    if (fromScratch) {
        // Already premultiplied linear: the rig's rasteriser produced it that way.
        return texelFetch(scratch, ivec2(origin) + texel, 0);
    }
    vec4 t = texelFetch(atlas, ivec2(origin) + texel, 0);
    return vec4(t.rgb * t.a, t.a);
}

// One frame's contribution at tile coordinate `src`.
//
// Nearest by default: `floor(src)`, which is what puts every source texel on a whole
// S x S block. With `u.time.z` set (`--gpu-filter bilinear`) it is instead
// `Sprite::sample`'s own four taps — `p = src − 0.5`, the four texels around it, the
// four bilinear weights — which is the CPU presenter's sampler, bit for bit, and is
// there so the two renderers can be compared with only the sampler between them.
vec4 frameAt(uvec2 origin, vec2 src, vec2 tile, float weight, bool fromScratch) {
    if (weight <= 0.0) { return vec4(0.0); }
    if (u.time.z > 0.5) {
        vec2 p = src - vec2(0.5);
        ivec2 t0 = ivec2(floor(p));
        vec2 fr = p - vec2(t0);
        vec4 sum = texelOf(origin, t0, tile, fromScratch) * ((1.0 - fr.x) * (1.0 - fr.y))
                 + texelOf(origin, t0 + ivec2(1, 0), tile, fromScratch) * (fr.x * (1.0 - fr.y))
                 + texelOf(origin, t0 + ivec2(0, 1), tile, fromScratch) * ((1.0 - fr.x) * fr.y)
                 + texelOf(origin, t0 + ivec2(1, 1), tile, fromScratch) * (fr.x * fr.y);
        return sum * weight;
    }
    return texelOf(origin, ivec2(floor(src)), tile, fromScratch) * weight;
}

void main() {
    vec2 pivot = vec2(vSizePivot.zw);
    vec2 tile = vec2(vSizePivot.xy);

    // The tile coordinate of *this* pixel's centre, computed from `gl_FragCoord` and the
    // flat instance data rather than interpolated across the quad. A nearest sampler
    // turns a half-ulp of interpolation error at a texel boundary into a whole wrong
    // texel; computing it here makes the coordinate a function of the pixel centre and
    // the instance alone, which every conformant implementation agrees on.
    vec2 offset = (gl_FragCoord.xy - vPlace.xy) / (u.time.w * max(vShade.z, 1e-3));
    vec2 heading = vPlace.zw;
    vec2 vLocal = vec2(dot(offset, heading), dot(offset, vec2(-heading.y, heading.x)));

    // `Bend::displacement`: H = (tile_height - p_y) + base, D = amplitude * smoothstep.
    // The row is never displaced, only the column, so the material rows are preserved and
    // the mask below still reads the row it was authored against.
    float py = vLocal.y + pivot.y;
    float d = 0.0;
    if (vBend.x != 0.0 && vBend.w > 0.0) {
        float t = clamp((tile.y - py + vBend.y - vBend.z) / vBend.w, 0.0, 1.0);
        d = vBend.x * (t * t * (3.0 - 2.0 * t));
        if (u.knobs.w < 0.5) { d = round(d); }
    }
    vec2 src = vec2(vLocal.x - d, vLocal.y) + pivot;
    // Nearest mode can reject the whole fragment early; the filtered one cannot, because
    // a sample half a texel outside the tile still carries part of the edge texel.
    if (u.time.z <= 0.5
        && (src.x < 0.0 || src.y < 0.0 || src.x >= tile.x || src.y >= tile.y)) {
        discard;
    }

    // `Mask`, in the sprite's own material coordinates so a reveal covers the same
    // material however the wind displaces it. Axial and strip read the row; radial reads
    // the distance from the pivot, which is what a canopy plant opening from its centre
    // needs -- and a ring has canopy cells, unlike the cube where only Top did.
    //
    // It is evaluated **before** the frames, because it depends only on `src` and the
    // instance: a fragment the mask rejects costs no `texelFetch` at all. A tall
    // column's trunk strip reveals four rows of a sixteen-row tile, so this is most of
    // the Tall layer's sampling.
    float cover;
    if (vMask.z > 0.5) {
        float r = length(src - pivot);
        cover = unit(vMask.y - r + 0.5) * unit(vMask.y);
    } else {
        float h = tile.y - src.y;
        cover = unit(vMask.y - h + 0.5) * unit(h - vMask.x + 0.5) * unit(vMask.y - max(vMask.x, 0.0));
    }
    float opacity = vMask.w;
    if (cover <= 0.0 || opacity <= 0.0) { discard; }

    bool fromScratch = vShade.w > 0.5;
    vec4 rgba = frameAt(vFrames01.xy, src, tile, vWeights.x, fromScratch)
              + frameAt(vFrames01.zw, src, tile, vWeights.y, fromScratch)
              + frameAt(vFrames23.xy, src, tile, vWeights.z, fromScratch)
              + frameAt(vFrames23.zw, src, tile, vWeights.w, fromScratch);
    rgba *= cover;

    // `Tone`: travel toward one colour at the same alpha, carrying the art's own light
    // through `Shade`, so the coverage of the stamp does not change with the mix.
    if (vTone.w > 0.0 && rgba.a > 0.0) {
        float luma = dot(rgba.rgb, vec3(0.2126, 0.7152, 0.0722)) / rgba.a;
        float lit = vShade.y > 0.0 ? clamp(luma / vShade.y, 0.0, 1.0) : 1.0;
        float s = (vShade.x + (1.0 - vShade.x) * lit) * rgba.a;
        rgba.rgb += (vTone.rgb * s - rgba.rgb) * vTone.w;
    }

    if (rgba.a * opacity <= 0.0) { discard; }
    outColour = rgba * opacity;
}
