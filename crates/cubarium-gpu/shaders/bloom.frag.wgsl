// The lit tier's bloom (package L5): a blocky, pixel-art halo around the emitters.
//
// `voxel.frag.wgsl` writes each pixel's emitted light (an emitter's colour, zero elsewhere)
// to a second attachment. Three passes over it, all nearest, all at the voxel cell's size
// (an S x S block of raster pixels, rows aligned with the voxel bands):
//   0 gather: one texel per cell, the brightest emitted colour in the cell, per channel;
//   1 spread: each cell takes, from every gathered cell within `radius` cells (Euclidean),
//     that cell's colour times a falloff quantised to STEPS steps (1, 2/3, 1/3 at 3), the
//     largest per channel, so neighbouring emitters do not stack;
//   2 add:    every raster pixel adds its cell's halo times `strength`, except an emitting
//     pixel, which stays exactly what `voxel.frag.wgsl` drew (the emitter stays crisp).

struct Bloom {
    a: vec4<i32>,  // mode, S, radius (cells), -
    b: vec4<i32>,  // cells wide, cells high, row offset, raster height
    k: vec4<f32>,  // strength, -, -, -
}
var<immediate> pc: Bloom;

@group(0) @binding(0) var src: texture_2d<f32>;      // the pass's input: emission or cells
@group(0) @binding(1) var emitTex: texture_2d<f32>;  // the emission attachment

// Steps the halo's falloff is quantised to.
const STEPS = 3.0;

@fragment
fn main(@builtin(position) fragCoord: vec4<f32>) -> @location(0) vec4<f32> {
    let mode = pc.a.x;
    let S = pc.a.y;
    let R = pc.a.z;
    let CW = pc.b.x;
    let CH = pc.b.y;
    let OFF = pc.b.z;
    let RH = pc.b.w;
    let p = vec2<i32>(fragCoord.xy);

    if (mode == 0) {
        var e = vec3(0.0);
        let y0 = p.y * S - OFF;
        for (var j = 0; j < S; j++) {
            let y = y0 + j;
            if (y < 0 || y >= RH) { continue; }
            for (var i = 0; i < S; i++) {
                e = max(e, textureLoad(src, vec2(p.x * S + i, y), 0).rgb);
            }
        }
        return vec4(e, 1.0);
    }

    if (mode == 1) {
        var best = vec3(0.0);
        let reach = f32(R) + 0.5;
        for (var dy = -R; dy <= R; dy++) {
            let y = p.y + dy;
            if (y < 0 || y >= CH) { continue; }
            for (var dx = -R; dx <= R; dx++) {
                let d = length(vec2<f32>(vec2(dx, dy)));
                if (d > reach) { continue; }
                // Round the ring without a `%` of a negative (`world.wgsl`'s `wrapMod`).
                var x = p.x + dx;
                x = select(CW - 1 - (-1 - x) % CW, x % CW, x >= 0);
                let e = textureLoad(src, vec2(x, y), 0).rgb;
                let w = 1.0 - d / f32(R + 1);
                let q = ceil(w * STEPS - 1e-3) / STEPS;
                best = max(best, e * q);
            }
        }
        return vec4(best, 1.0);
    }

    // mode 2: add the halo onto the raster (additive blend, alpha 0 keeps the raster's).
    if (any(textureLoad(emitTex, p, 0).rgb > vec3(0.0))) {
        return vec4(0.0);
    }
    let c = vec2(p.x / S, (p.y + OFF) / S);
    let h = textureLoad(src, c, 0).rgb;
    return vec4(h * pc.k.x, 0.0);
}
