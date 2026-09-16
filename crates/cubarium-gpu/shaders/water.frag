#version 450
#include "scene.glsl"
// FW-P pass 6, source-over the ground and *under* the plants — which is why it is its
// own pass rather than a term in `background.frag`: the ground cover lattice is drawn
// between the two, exactly as `art_present::draw_with_fruit` orders them.
layout(location = 0) out vec4 outColour;

void main() {
    vec2 px = gl_FragCoord.xy;
    float w = filteredAt(fieldA, 1, px);
    if (!(w > 0.0)) { discard; }
    float a = 1.0 - exp(-w / u.waterLow.w);
    if (a <= 0.0) { discard; }

    // The mat tints the pool mint where the wet cell holds producers; the cell's own
    // value, unfiltered, as `draw_water` reads it.
    float saturation = u.grid.w * u.knobs.x;
    float pt = saturation > 0.0 ? cellValue(fieldA, 0, cellOf(px)) / saturation : 0.0;
    vec3 base = mix(u.waterLow.rgb, u.waterHigh.rgb, clamp(w, 0.0, 1.0));
    vec3 c = mix(base, u.algae.rgb, clamp(pt, 0.0, 1.0) * u.algae.w);

    // The shimmer: a fixed per-pixel phase that simulated time slides through, so a
    // paused world holds its glints and the pattern repeats every 2.5 s exactly.
    float cycle = fract(u.time.x / u.knobs.z);
    float b = u.waterHigh.w * (1.0 + u.knobs.y * sin(6.28318530718 * cycle + hashPhase(ivec2(floor(px)))));
    outColour = vec4(c * (b * a), a);
}
