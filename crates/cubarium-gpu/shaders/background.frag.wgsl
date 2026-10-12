// #import scene.wgsl

// FW-P passes 1-4 in one full-screen pass: the night floor, the producer ramp faded
// through the horizon, the detritus flecks, and the soil wash below it. Each term is
// `canvas.add`, so the whole pass is a sum and the output is opaque.
@fragment
fn main(@builtin(position) fragCoord: vec4<f32>) -> @location(0) vec4<f32> {
    let px = fragCoord.xy;
    let soil = soilWeight(px.y);
    let above = 1.0 - soil;
    var c = u.floorColour.rgb;

    // 2. The producer ramp: hue linear in density, brightness in its square, so the
    //    ordinary standing crop stays a dim floor and only rich patches turn cyan.
    let saturation = u.grid.w * u.knobs.x;
    if (above > 0.0 && saturation > 0.0) {
        let t = min(filteredAt(fieldA, 0, px) / saturation, 1.0);
        if (t != 0.0) {
            let ramp = mix(u.producerLow.rgb, u.producerHigh.rgb, t);
            let b = u.producerLow.w + (u.producerHigh.w - u.producerLow.w) * t * t;
            c += ramp * (b * above);
        }
        // 3. Detritus flecks: thresholded, unfiltered, above the horizon only. In the
        //    soil, detritus is the ground itself and not a fleck.
        let d = cellValue(fieldA, 2, cellOf(px));
        if (d > u.bands.z) {
            c += u.detritus.rgb * (min(d / u.detritus.w, 1.0) * above);
        }
    }

    // 4. The soil ground: dark plum to violet-mauve by filtered detritus, linear in
    //    brightness (soil is ground, not a highlight) and never fully dark.
    if (soil > 0.0) {
        let t = clamp(filteredAt(fieldA, 2, px) / u.bands.w, 0.0, 1.0);
        let s = mix(u.soilLow.rgb, u.soilHigh.rgb, t);
        let b = (u.soilLow.w + (u.soilHigh.w - u.soilLow.w) * t) * soil;
        c += s * b;
    }
    return vec4<f32>(c, 1.0);
}
