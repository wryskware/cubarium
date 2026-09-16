#version 450
#include "scene.glsl"
// The GPU's `cubarium_render::stamp_layers_bent_toned`, for one two-frame pose.
//
// Nearest sampling, always: the tile coordinate is floored to a whole source texel, so
// every texel covers an exact S x S block of raster pixels and nothing is filtered.
// The one sub-pixel quantity a stamp carries is the wind's bend, and it is rounded to
// a whole source texel here, so a breeze steps the art rather than smearing it.

layout(set = 0, binding = 3) uniform sampler2D atlas;

layout(location = 0) flat in vec4  vPlace;
layout(location = 1) flat in uvec4 vFrame0;
layout(location = 2) flat in uvec4 vFrame1Pivot;
layout(location = 3) flat in vec2  vBlend;
layout(location = 4) flat in vec4  vBend;
layout(location = 5) flat in vec2  vMask;
layout(location = 6) flat in vec3  vToneColour;
layout(location = 7) flat in vec3  vToneShade;

layout(location = 0) out vec4 outColour;

float unit(float v) { return clamp(v, 0.0, 1.0); }

void main() {
    vec2 pivot = vec2(vFrame1Pivot.zw);
    vec2 tile = vec2(vFrame0.zw);

    // The tile coordinate of *this* pixel's centre, computed from `gl_FragCoord` and
    // the flat instance data rather than interpolated across the quad.
    //
    // This is not a micro-optimisation. An interpolated varying is only as good as the
    // interpolator's precision, and a nearest-neighbour sampler turns a half-ulp of
    // interpolation error at a texel boundary into a whole wrong texel. Computing it
    // here makes the coordinate a function of the pixel centre and the instance alone,
    // which every conformant implementation agrees on: the Adreno 643 and the desktop
    // went from 3.5 % of pixels differing by more than 8 to a fraction of that.
    vec2 offset = (gl_FragCoord.xy - vPlace.xy) / u.grid.z;
    vec2 heading = vPlace.zw;
    vec2 vLocal = vec2(dot(offset, heading), dot(offset, vec2(-heading.y, heading.x)));

    // `Bend::displacement`: H = (tile_height - p_y) + base, D = amplitude * smoothstep.
    // The row is never displaced, only the column, so the material rows are preserved
    // and the mask below still reads the row it was authored against.
    float py = vLocal.y + pivot.y;
    float d = 0.0;
    if (vBend.x != 0.0 && vBend.w > 0.0) {
        float t = clamp((tile.y - py + vBend.y - vBend.z) / vBend.w, 0.0, 1.0);
        d = vBend.x * (t * t * (3.0 - 2.0 * t));
    }
    vec2 src = vec2(vLocal.x - round(d), vLocal.y) + pivot;
    ivec2 texel = ivec2(floor(src));
    if (texel.x < 0 || texel.y < 0 || texel.x >= int(tile.x) || texel.y >= int(tile.y)) {
        discard;
    }

    // The pack is straight sRGB + alpha; premultiply after the sampler's decode, which
    // is exactly `Sprite::from_rgba`'s `srgb_decode(c) * a`.
    vec4 a0 = texelFetch(atlas, ivec2(vFrame0.xy) + texel, 0);
    vec4 a1 = texelFetch(atlas, ivec2(vFrame1Pivot.xy) + texel, 0);
    vec4 rgba = mix(vec4(a0.rgb * a0.a, a0.a), vec4(a1.rgb * a1.a, a1.a), vBlend.x);

    // `Mask::Axial` / `Mask::Strip`, in the sprite's own material coordinates so a
    // reveal covers the same rows however the wind displaces them.
    float h = tile.y - src.y;
    float cover = unit(vMask.y - h + 0.5) * unit(h - vMask.x + 0.5) * unit(vMask.y - max(vMask.x, 0.0));
    if (cover <= 0.0) { discard; }
    rgba *= cover;

    // `Tone`: travel toward one colour at the same alpha, carrying the art's own light
    // through `Shade`, so the coverage of the stamp does not change with the mix.
    if (vToneShade.z > 0.0 && rgba.a > 0.0) {
        float luma = dot(rgba.rgb, vec3(0.2126, 0.7152, 0.0722)) / rgba.a;
        float lit = vToneShade.y > 0.0 ? clamp(luma / vToneShade.y, 0.0, 1.0) : 1.0;
        float s = (vToneShade.x + (1.0 - vToneShade.x) * lit) * rgba.a;
        rgba.rgb += (vToneColour * s - rgba.rgb) * vToneShade.z;
    }

    float opacity = vBlend.y;
    if (rgba.a * opacity <= 0.0) { discard; }
    outColour = rgba * opacity;
}
