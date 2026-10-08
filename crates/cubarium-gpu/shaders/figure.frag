#version 450
// The model pass's tile raster, shading (H2): one figure's visible faces, lit by the walk's
// own light at the texel's world point and with its real normal.
//
// The stamped animal drew its front faces under the front light and its caps under the
// top light, its front's top row (the open rim) leaning toward the top light. Here a face's
// light is the same `litBy` of the same terms — the sky plane and the canopy over the open
// cell in front of the face, the sun march from the texel toward the sun times N·L, the
// emitters' glow half a voxel out, the haze at its depth — with N the face's own normal, so
// a model facing the default way draws what the stamp drew, a turned one is lit as it now
// faces, and a sunlit flank is the side toward the sun. The rim stays: a side face's top
// pixel row under a cell open above leans toward the light an up-facing face there would
// have, by PLANT_RIM. The animal is not in the voxel volume any more, so it shadows neither
// itself nor the ground (`figures.rs` names the hook); its faces' AO crease is not drawn.
//
// A glyph cell (the dev-mode bodies, `cubarium::voxel::appearance`) reads its face texels
// from the walk's glyph atlas as the walk did: pigment and tone, markings and eyes as
// authored, on every side face and the cap.

#include "world.glsl"
#include "figure.glsl"

struct Pal {
    vec4 base; // linear colour; w > 0.5: the cell emits
    vec4 emit; // its emissive colour, linear
};
layout(std430, set = 1, binding = 3) readonly buffer FigurePalette { Pal pal[]; };

layout(location = 0) in vec3 vWorld;
layout(location = 1) in vec3 vNormal;
layout(location = 2) in vec2 vFace;
layout(location = 3) flat in ivec4 vInfo;

layout(location = 0) out vec4 outColour;
layout(location = 1) out vec4 outEmit;
layout(location = 2) out float outZ;

// The light a face with normal n gets at world point p (lit tier).
vec3 litFace(vec3 base, vec3 p, vec3 n) {
    ivec3 c = ivec3(floor(p + n * 0.5));
    float ndl = max(dot(n, u.sunK.xyz), 0.0);
    float sun = 0.0;
    if (ndl > 0.0 && u.sunK.y > 0.0) {
        sun = sunReaches(p + n * 0.01, c);
    }
    float a = skyOpen(c.x, c.y, c.z) * canopyAt(c.x, c.z, int(floor(p.y)));
    return litBy(base, a, sun, ndl, glowAt(p + n * 0.5));
}

// The flat tier's top light over the cell at p (`plantLit` under its column's roof).
vec3 flatTop(vec3 base, vec3 p) {
    ivec3 c = ivec3(floor(p - vec3(0.0, 0.01, 0.0)));
    int gap = inY(c.y) ? roofGap(c.x, c.y, c.z) : 0;
    return plantLit(base, roofShade(gap));
}

void main() {
    S = u.geom.x;
    RISE = u.geom.y;
    BASE = u.geom.z;
    W = u.extent.x;
    H = u.extent.y;
    D = u.extent.z;

    vec3 n = normalize(vNormal);
    vec3 p = vWorld;
    bool top = n.y > 0.7;
    bool side = abs(n.y) <= 0.7;
    bool openUp = vInfo.z != 0;
    int entry = vInfo.x;
    int glyph = vInfo.y;
    float hz = hazeAt(p.z);
    outZ = p.z;

    // The texel's pigment and tone: a model cell is its palette colour, plain, its side
    // faces' top row the open rim; a glyph cell reads its atlas.
    int tone = 0;
    int pigment = 0;
    // The first pixel row under a face's top edge (the rim row of the stamp).
    bool topRow = side && vFace.y < max(fwidth(vFace.y), 1e-3);
    if (glyph != 0) {
        int dx = clamp(int(vFace.x * float(S)), 0, S - 1);
        int row = top ? S + clamp(int(vFace.y * float(RISE)), 0, RISE - 1)
                      : clamp(int(vFace.y * float(S)), 0, S - 1);
        uint q = texelFetch(glyphTex, ivec2(dx, (ANIMAL_INTERIM * 8 + glyph) * (S + RISE) + row), 0).r;
        pigment = int(q & 3u);
        tone = int(q >> 2);
        if (tone == 63) { discard; }
    } else if (topRow && openUp) {
        tone = 2;
    }
    Pal pe = pal[entry + pigment];
    vec3 base = pe.base.rgb;

    if (LIT && (pe.base.w > 0.5 || tone == TONE_EMIT)) {
        if (any(greaterThan(pe.emit.rgb, vec3(0.0)))) {
            // An emitter: its emissive colour, unshadowed and at full value.
            vec3 drawn = hazed(pe.emit.rgb, hz);
            outColour = vec4(drawn, 1.0);
            outEmit = vec4(max(drawn - clamp(hz, 0.0, 1.0) * u.hazeC.rgb, vec3(0.0)), 1.0);
            return;
        }
    }
    outEmit = vec4(0.0, 0.0, 0.0, 1.0);

    vec3 c;
    if (LIT) {
        vec3 up = vec3(0.0, 1.0, 0.0);
        if (top) {
            c = litFace(base, p, n);
            if (tone == 1) {
                c *= u.plantA.w;
            } else if (tone >= 32 && tone <= 47) {
                float gain = 0.5 + float(tone & 15) / 16.0;
                c = mix(c, c / u.plantA.x * gain, u.shadeB.y);
            }
        } else if (tone == 3) {
            c = litFace(base, p, up);
        } else {
            if (tone == 1) {
                base *= u.shadeB.x;
            } else if (tone == 4 || tone == 5) {
                base = mix(base, pal[entry].base.rgb, u.plantB.x);
                if (tone == 5) { base *= u.plantA.w; }
            } else if (tone >= 16 && tone <= 31) {
                base *= 0.5 + float(tone & 15) / 16.0;
            }
            c = litFace(base, p, n);
            if (tone == 2 && openUp) {
                c = mix(c, litFace(base, p, up), u.plantA.z);
            }
        }
    } else {
        if (top) {
            c = flatTop(base, p);
            if (tone == 1) {
                c *= u.plantA.w;
            } else if (tone >= 32 && tone <= 47) {
                float gain = 0.5 + float(tone & 15) / 16.0;
                c = mix(c, base * gain, u.shadeB.y);
            }
        } else if (tone == 3) {
            c = flatTop(base, p);
        } else {
            if (tone == 1) {
                base *= u.shadeB.x;
            } else if (tone == 2 && openUp) {
                base = mix(base, flatTop(base, p), u.plantA.z);
            } else if (tone == 4 || tone == 5) {
                base = mix(base, pal[entry].base.rgb, u.plantB.x);
                if (tone == 5) { base *= u.plantA.w; }
            } else if (tone >= 16 && tone <= 31) {
                base *= 0.5 + float(tone & 15) / 16.0;
            }
            c = base;
        }
    }
    vec3 o = hazed(c, hz);
    // The flat tier's day and night: a tint over the hazed world (1 by day), as the walk's.
    if (!LIT) { o *= u.tint.rgb; }
    outColour = vec4(o, 1.0);
}
