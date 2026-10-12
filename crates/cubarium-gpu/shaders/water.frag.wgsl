// #import scene.wgsl

// FW-P pass 6, source-over the ground and *under* the plants — which is why it is its
// own pass rather than a term in `background.frag.wgsl`: the ground cover lattice is drawn
// between the two, exactly as `art_present::draw_with_fruit` orders them.
@fragment
fn main(@builtin(position) fragCoord: vec4<f32>) -> @location(0) vec4<f32> {
    let px = fragCoord.xy;
    let w = filteredAt(fieldA, 1, px);
    if (!(w > 0.0)) { discard; }
    let a = 1.0 - exp(-w / u.waterLow.w);
    if (a <= 0.0) { discard; }

    // The mat tints the pool mint where the wet cell holds producers; the cell's own
    // value, unfiltered, as `draw_water` reads it.
    let saturation = u.grid.w * u.knobs.x;
    var pt = 0.0;
    if (saturation > 0.0) { pt = cellValue(fieldA, 0, cellOf(px)) / saturation; }
    let base = mix(u.waterLow.rgb, u.waterHigh.rgb, clamp(w, 0.0, 1.0));
    let c = mix(base, u.algae.rgb, clamp(pt, 0.0, 1.0) * u.algae.w);

    // The shimmer: a fixed per-pixel phase that simulated time slides through, so a
    // paused world holds its glints and the pattern repeats every 2.5 s exactly.
    let cycle = fract(u.time.x / u.knobs.z);
    let b = u.waterHigh.w * (1.0 + u.knobs.y * sin(6.28318530718 * cycle + hashPhase(vec2<i32>(floor(px)))));
    return vec4<f32>(c * (b * a), a);
}
