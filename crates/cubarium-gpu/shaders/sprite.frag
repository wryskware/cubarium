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

layout(location = 0) flat in vec4  vPlace;
layout(location = 1) flat in uvec4 vFrames01;
layout(location = 2) flat in uvec4 vFrames23;
layout(location = 3) flat in uvec4 vSizePivot;
layout(location = 4) flat in vec4  vWeights;
layout(location = 5) flat in vec4  vBend;
layout(location = 6) flat in vec4  vMask;
layout(location = 7) flat in vec4  vTone;
layout(location = 8) flat in vec2  vShade;

layout(location = 0) out vec4 outColour;

float unit(float v) { return clamp(v, 0.0, 1.0); }

// One frame's premultiplied linear sample. The pack is straight sRGB + alpha, so the
// premultiply happens after the sampler's decode -- exactly `Sprite::from_rgba`'s
// `srgb_decode(c) * a`.
vec4 frameAt(uvec2 origin, ivec2 texel, float weight) {
    if (weight <= 0.0) { return vec4(0.0); }
    vec4 t = texelFetch(atlas, ivec2(origin) + texel, 0);
    return vec4(t.rgb * t.a, t.a) * weight;
}

void main() {
    vec2 pivot = vec2(vSizePivot.zw);
    vec2 tile = vec2(vSizePivot.xy);

    // The tile coordinate of *this* pixel's centre, computed from `gl_FragCoord` and the
    // flat instance data rather than interpolated across the quad. A nearest sampler
    // turns a half-ulp of interpolation error at a texel boundary into a whole wrong
    // texel; computing it here makes the coordinate a function of the pixel centre and
    // the instance alone, which every conformant implementation agrees on.
    vec2 offset = (gl_FragCoord.xy - vPlace.xy) / u.grid.z;
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
    ivec2 texel = ivec2(floor(src));
    if (texel.x < 0 || texel.y < 0 || texel.x >= int(tile.x) || texel.y >= int(tile.y)) {
        discard;
    }

    vec4 rgba = frameAt(vFrames01.xy, texel, vWeights.x)
              + frameAt(vFrames01.zw, texel, vWeights.y)
              + frameAt(vFrames23.xy, texel, vWeights.z)
              + frameAt(vFrames23.zw, texel, vWeights.w);

    // `Mask`, in the sprite's own material coordinates so a reveal covers the same
    // material however the wind displaces it. Axial and strip read the row; radial reads
    // the distance from the pivot, which is what a canopy plant opening from its centre
    // needs -- and a ring has canopy cells, unlike the cube where only Top did.
    float cover;
    if (vMask.z > 0.5) {
        float r = length(src - pivot);
        cover = unit(vMask.y - r + 0.5) * unit(vMask.y);
    } else {
        float h = tile.y - src.y;
        cover = unit(vMask.y - h + 0.5) * unit(h - vMask.x + 0.5) * unit(vMask.y - max(vMask.x, 0.0));
    }
    if (cover <= 0.0) { discard; }
    rgba *= cover;

    // `Tone`: travel toward one colour at the same alpha, carrying the art's own light
    // through `Shade`, so the coverage of the stamp does not change with the mix.
    if (vTone.w > 0.0 && rgba.a > 0.0) {
        float luma = dot(rgba.rgb, vec3(0.2126, 0.7152, 0.0722)) / rgba.a;
        float lit = vShade.y > 0.0 ? clamp(luma / vShade.y, 0.0, 1.0) : 1.0;
        float s = (vShade.x + (1.0 - vShade.x) * lit) * rgba.a;
        rgba.rgb += (vTone.rgb * s - rgba.rgb) * vTone.w;
    }

    float opacity = vMask.w;
    if (rgba.a * opacity <= 0.0) { discard; }
    outColour = rgba * opacity;
}
