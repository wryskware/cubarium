#version 450
// The lit tier's bloom (package L5): a blocky, pixel-art halo around the emitters.
//
// `voxel.frag` writes each pixel's emitted light (an emitter's colour, zero elsewhere) to
// a second attachment. Three passes over it, all nearest, all at the voxel cell's size
// (an S x S block of raster pixels, rows aligned with the voxel bands):
//   0 gather: one texel per cell, the brightest emitted colour in the cell, per channel;
//   1 spread: each cell takes, from every gathered cell within `radius` cells (Euclidean),
//     that cell's colour times a falloff quantised to STEPS steps (1, 2/3, 1/3 at 3), the
//     largest per channel, so neighbouring emitters do not stack;
//   2 add:    every raster pixel adds its cell's halo times `strength`, except an emitting
//     pixel, which stays exactly what `voxel.frag` drew (the emitter stays crisp).

layout(push_constant) uniform Bloom {
    ivec4 a; // mode, S, radius (cells), -
    ivec4 b; // cells wide, cells high, row offset, raster height
    vec4 k;  // strength, -, -, -
} pc;

layout(set = 0, binding = 0) uniform sampler2D src;     // the pass's input: emission or cells
layout(set = 0, binding = 1) uniform sampler2D emitTex; // the emission attachment

layout(location = 0) out vec4 outColour;

// Steps the halo's falloff is quantised to.
const float STEPS = 3.0;

void main() {
    int mode = pc.a.x;
    int S = pc.a.y;
    int R = pc.a.z;
    int CW = pc.b.x;
    int CH = pc.b.y;
    int OFF = pc.b.z;
    int RH = pc.b.w;
    ivec2 p = ivec2(gl_FragCoord.xy);

    if (mode == 0) {
        vec3 e = vec3(0.0);
        int y0 = p.y * S - OFF;
        for (int j = 0; j < S; ++j) {
            int y = y0 + j;
            if (y < 0 || y >= RH) { continue; }
            for (int i = 0; i < S; ++i) {
                e = max(e, texelFetch(src, ivec2(p.x * S + i, y), 0).rgb);
            }
        }
        outColour = vec4(e, 1.0);
        return;
    }

    if (mode == 1) {
        vec3 best = vec3(0.0);
        float reach = float(R) + 0.5;
        for (int dy = -R; dy <= R; ++dy) {
            int y = p.y + dy;
            if (y < 0 || y >= CH) { continue; }
            for (int dx = -R; dx <= R; ++dx) {
                float d = length(vec2(dx, dy));
                if (d > reach) { continue; }
                int x = (p.x + dx) % CW;
                if (x < 0) { x += CW; }
                vec3 e = texelFetch(src, ivec2(x, y), 0).rgb;
                float w = 1.0 - d / float(R + 1);
                float q = ceil(w * STEPS - 1e-3) / STEPS;
                best = max(best, e * q);
            }
        }
        outColour = vec4(best, 1.0);
        return;
    }

    // mode 2: add the halo onto the raster (additive blend, alpha 0 keeps the raster's).
    if (any(greaterThan(texelFetch(emitTex, p, 0).rgb, vec3(0.0)))) {
        outColour = vec4(0.0);
        return;
    }
    ivec2 c = ivec2(p.x / S, (p.y + OFF) / S);
    vec3 h = texelFetch(src, c, 0).rgb;
    outColour = vec4(h * pc.k.x, 0.0);
}
