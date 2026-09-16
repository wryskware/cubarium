#version 450
#include "scene.glsl"
// FW-P passes 1-4 in one full-screen pass: the night floor, the producer ramp faded
// through the horizon, the detritus flecks, and the soil wash below it. Each term is
// `canvas.add`, so the whole pass is a sum and the output is opaque.
layout(location = 0) out vec4 outColour;

void main() {
    vec2 px = gl_FragCoord.xy;
    float soil = soilWeight(px.y);
    float above = 1.0 - soil;
    vec3 c = u.floorColour.rgb;

    // 2. The producer ramp: hue linear in density, brightness in its square, so the
    //    ordinary standing crop stays a dim floor and only rich patches turn cyan.
    float saturation = u.grid.w * u.knobs.x;
    if (above > 0.0 && saturation > 0.0) {
        float t = min(filteredAt(fieldA, 0, px) / saturation, 1.0);
        if (t != 0.0) {
            vec3 ramp = mix(u.producerLow.rgb, u.producerHigh.rgb, t);
            float b = u.producerLow.w + (u.producerHigh.w - u.producerLow.w) * t * t;
            c += ramp * (b * above);
        }
        // 3. Detritus flecks: thresholded, unfiltered, above the horizon only. In the
        //    soil, detritus is the ground itself and not a fleck.
        float d = cellValue(fieldA, 2, cellOf(px));
        if (d > u.bands.z) {
            c += u.detritus.rgb * (min(d / u.detritus.w, 1.0) * above);
        }
    }

    // 4. The soil ground: dark plum to violet-mauve by filtered detritus, linear in
    //    brightness (soil is ground, not a highlight) and never fully dark.
    if (soil > 0.0) {
        float t = clamp(filteredAt(fieldA, 2, px) / u.bands.w, 0.0, 1.0);
        vec3 s = mix(u.soilLow.rgb, u.soilHigh.rgb, t);
        float b = (u.soilLow.w + (u.soilHigh.w - u.soilLow.w) * t) * soil;
        c += s * b;
    }
    outColour = vec4(c, 1.0);
}
