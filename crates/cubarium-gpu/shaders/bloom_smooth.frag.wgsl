// The lit tier's smooth bloom (the default `bloom_style`): a soft halo around the emitters.
//
// `voxel.frag.wgsl` writes each pixel's emitted light (an emitter's colour, zero elsewhere)
// to a second attachment. A mip chain over it, every read bilinear:
//   0 down:  half the size, a dual-Kawase 5-tap (the centre four times, the four diagonal
//            corners once), from the emission and then from each level;
//   1 blurH, 2 blurV: a separable Gaussian at the last level, `sigma` texels;
//   3 up:    twice the size, a dual-Kawase 8-tap tent, back up the chain;
//   4 add:   every raster pixel adds the first level, bilinear, times `strength`, except
//            an emitting pixel, which stays exactly what `voxel.frag.wgsl` drew (the emitter
//            stays crisp).
// The world is a ring: every read wraps in x and is black past the top and bottom.

struct Bloom {
    a: vec4<i32>,  // mode, source width, source height, taps (blur)
    b: vec4<i32>,  // target width, target height, -, -
    k: vec4<f32>,  // strength (add), sigma (blur), -, -
}
var<immediate> pc: Bloom;

@group(0) @binding(0) var src: texture_2d<f32>;      // the pass's input, bilinear
@group(0) @binding(1) var emitTex: texture_2d<f32>;  // the emission attachment

// Texel p of `src`, wrapping round the ring in x and black past the top and bottom.
fn borderTexel(pIn: vec2<i32>) -> vec3<f32> {
    var p = pIn;
    let size = vec2<i32>(textureDimensions(src, 0));
    if (p.y < 0 || p.y >= size.y) { return vec3(0.0); }
    p.x = select(size.x - 1 - (-1 - p.x) % size.x, p.x % size.x, p.x >= 0);
    return textureLoad(src, p, 0).rgb;
}

// The bilinear read. Clamp-to-border is optional in WebGPU, so the four texels and their
// weights are taken by hand, transparent past the edge, partial edge taps included.
fn tap(uv: vec2<f32>) -> vec3<f32> {
    let p = uv * vec2<f32>(vec2<i32>(textureDimensions(src, 0))) - 0.5;
    let lo = vec2<i32>(floor(p));
    let f = fract(p);
    return mix(mix(borderTexel(lo), borderTexel(lo + vec2(1, 0)), f.x),
               mix(borderTexel(lo + vec2(0, 1)), borderTexel(lo + vec2(1, 1)), f.x), f.y);
}

@fragment
fn main(@builtin(position) fragCoord: vec4<f32>) -> @location(0) vec4<f32> {
    let mode = pc.a.x;
    let srcSize = vec2<f32>(pc.a.yz);
    let dstSize = vec2<f32>(pc.b.xy);
    let uv = fragCoord.xy / dstSize;
    let t = 1.0 / srcSize;

    if (mode == 0) {
        var s = tap(uv) * 4.0;
        s += tap(uv - t);
        s += tap(uv + t);
        s += tap(uv + vec2(t.x, -t.y));
        s += tap(uv + vec2(-t.x, t.y));
        return vec4(s / 8.0, 1.0);
    }

    if (mode == 1 || mode == 2) {
        let p = vec2<i32>(fragCoord.xy);
        let size = pc.a.yz;
        let dir = select(vec2(0, 1), vec2(1, 0), mode == 1);
        let sigma = max(pc.k.y, 1e-3);
        let n = pc.a.w;
        var s = vec3(0.0);
        var wsum = 0.0;
        for (var i = -n; i <= n; i++) {
            let w = exp(-f32(i * i) / (2.0 * sigma * sigma));
            wsum += w;
            var q = p + dir * i;
            if (q.y < 0 || q.y >= size.y) { continue; }
            // Round the ring without a `%` of a negative (`world.wgsl`'s `wrapMod`).
            q.x = select(size.x - 1 - (-1 - q.x) % size.x, q.x % size.x, q.x >= 0);
            s += textureLoad(src, q, 0).rgb * w;
        }
        return vec4(s / wsum, 1.0);
    }

    if (mode == 3) {
        let h = 0.5 * t;
        var s = tap(uv + vec2(-2.0 * h.x, 0.0));
        s += tap(uv + vec2(-h.x, h.y)) * 2.0;
        s += tap(uv + vec2(0.0, 2.0 * h.y));
        s += tap(uv + vec2(h.x, h.y)) * 2.0;
        s += tap(uv + vec2(2.0 * h.x, 0.0));
        s += tap(uv + vec2(h.x, -h.y)) * 2.0;
        s += tap(uv + vec2(0.0, -2.0 * h.y));
        s += tap(uv + vec2(-h.x, -h.y)) * 2.0;
        return vec4(s / 12.0, 1.0);
    }

    // mode 4: add the halo onto the raster (additive blend, alpha 0 keeps the raster's).
    if (any(textureLoad(emitTex, vec2<i32>(fragCoord.xy), 0).rgb > vec3(0.0))) {
        return vec4(0.0);
    }
    return vec4(tap(uv) * pc.k.x, 0.0);
}
