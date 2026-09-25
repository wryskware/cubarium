#version 450
// The lit tier's smooth bloom (the default `bloom_style`): a soft halo around the emitters.
//
// `voxel.frag` writes each pixel's emitted light (an emitter's colour, zero elsewhere) to
// a second attachment. A mip chain over it, every read bilinear:
//   0 down:  half the size, a dual-Kawase 5-tap (the centre four times, the four diagonal
//            corners once), from the emission and then from each level;
//   1 blurH, 2 blurV: a separable Gaussian at the last level, `sigma` texels;
//   3 up:    twice the size, a dual-Kawase 8-tap tent, back up the chain;
//   4 add:   every raster pixel adds the first level, bilinear, times `strength`, except
//            an emitting pixel, which stays exactly what `voxel.frag` drew (the emitter
//            stays crisp).
// The world is a ring: every read wraps in x (the sampler repeats, the blur wraps by
// hand) and is black past the top and bottom.

layout(push_constant) uniform Bloom {
    ivec4 a; // mode, source width, source height, taps (blur)
    ivec4 b; // target width, target height, -, -
    vec4 k;  // strength (add), sigma (blur), -, -
} pc;

layout(set = 0, binding = 0) uniform sampler2D src;     // the pass's input, bilinear
layout(set = 0, binding = 1) uniform sampler2D emitTex; // the emission attachment

layout(location = 0) out vec4 outColour;

vec3 tap(vec2 uv) { return texture(src, uv).rgb; }

void main() {
    int mode = pc.a.x;
    vec2 srcSize = vec2(pc.a.yz);
    vec2 dstSize = vec2(pc.b.xy);
    vec2 uv = gl_FragCoord.xy / dstSize;
    vec2 t = 1.0 / srcSize;

    if (mode == 0) {
        vec3 s = tap(uv) * 4.0;
        s += tap(uv - t);
        s += tap(uv + t);
        s += tap(uv + vec2(t.x, -t.y));
        s += tap(uv + vec2(-t.x, t.y));
        outColour = vec4(s / 8.0, 1.0);
        return;
    }

    if (mode == 1 || mode == 2) {
        ivec2 p = ivec2(gl_FragCoord.xy);
        ivec2 size = ivec2(pc.a.yz);
        ivec2 dir = mode == 1 ? ivec2(1, 0) : ivec2(0, 1);
        float sigma = max(pc.k.y, 1e-3);
        int n = pc.a.w;
        vec3 s = vec3(0.0);
        float wsum = 0.0;
        for (int i = -n; i <= n; ++i) {
            float w = exp(-float(i * i) / (2.0 * sigma * sigma));
            wsum += w;
            ivec2 q = p + dir * i;
            if (q.y < 0 || q.y >= size.y) { continue; }
            q.x = q.x % size.x;
            if (q.x < 0) { q.x += size.x; }
            s += texelFetch(src, q, 0).rgb * w;
        }
        outColour = vec4(s / wsum, 1.0);
        return;
    }

    if (mode == 3) {
        vec2 h = 0.5 * t;
        vec3 s = tap(uv + vec2(-2.0 * h.x, 0.0));
        s += tap(uv + vec2(-h.x, h.y)) * 2.0;
        s += tap(uv + vec2(0.0, 2.0 * h.y));
        s += tap(uv + vec2(h.x, h.y)) * 2.0;
        s += tap(uv + vec2(2.0 * h.x, 0.0));
        s += tap(uv + vec2(h.x, -h.y)) * 2.0;
        s += tap(uv + vec2(0.0, -2.0 * h.y));
        s += tap(uv + vec2(-h.x, -h.y)) * 2.0;
        outColour = vec4(s / 12.0, 1.0);
        return;
    }

    // mode 4: add the halo onto the raster (additive blend, alpha 0 keeps the raster's).
    if (any(greaterThan(texelFetch(emitTex, ivec2(gl_FragCoord.xy), 0).rgb, vec3(0.0)))) {
        outColour = vec4(0.0);
        return;
    }
    outColour = vec4(tap(uv) * pc.k.x, 0.0);
}
