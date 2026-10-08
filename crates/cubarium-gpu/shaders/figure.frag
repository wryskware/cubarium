#version 450
// The model pass's tile raster, shading (H2): one figure's visible faces, lit by the walk's
// own light at the texel's world point and with its real normal.
//
// Each baked model cell samples the stamped cell's glyph (animal part 5, glyph 1):
// an open top rim and plain other rows. Glyph bodies also keep their outline, lit and
// gain tones from the same atlas. Face coordinates and open-plane crease masks are
// local to the model, so they follow its turns. AO uses the walk's crease width and
// strength, before the light ladder. Terrain/plant neighbours are not in these masks.
// The figures' sun map shadows terrain, plants, other figures and their own faces;
// its resolution and low-sun cutoff differ from the stamped volume's sun march.

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
layout(location = 4) flat in ivec2 vFaceSize;
layout(location = 5) in vec3 vRimWorld;
layout(location = 6) in vec2 vRimFace;
layout(location = 7) flat in ivec2 vCapSize;

layout(location = 0) out vec4 outColour;
layout(location = 1) out vec4 outEmit;
layout(location = 2) out float outZ;

// The light a face with normal n gets at world point p (lit tier).
vec3 litFace(vec3 base, vec3 p, vec3 n, float ao) {
    ivec3 c = ivec3(floor(p + n * 0.5));
    float ndl = max(dot(n, u.sunK.xyz), 0.0);
    float sun = 0.0;
    if (ndl > 0.0 && u.sunK.y > 0.0) {
        sun = sunReaches(p + n * 0.01, c);
    }
    float a = skyOpen(c.x, c.y, c.z) * ao * canopyAt(c.x, c.z, int(floor(p.y - n.y * 0.01)));
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
    bool top = (vInfo.w & 7) == 3 || (vInfo.w & 7) == 2;
    bool openUp = vInfo.z != 0;
    int entry = vInfo.x;
    int glyph = vInfo.y;
    float hz = hazeAt(p.z);
    outZ = p.z;

    // Atlas rows are in face space, with the cap's RISE rows after the S side rows.
    int face = vInfo.w & 7;
    bool cap = face == 2 || face == 3;
    int rows = cap ? RISE : S;
    int dx = clamp(int(vFace.x * float(S)), 0, S - 1);
    int dy = clamp(int(vFace.y * float(rows)), 0, rows - 1);
    int stampGlyph = glyph == 0 ? 1 : glyph;
    uint q = glyphTexel(ANIMAL_INTERIM, stampGlyph, dx, (cap ? S : 0) + dy);
    int pigment = glyph == 0 ? 0 : int(q & 3u);
    int tone = int(q >> 2);
    if (tone == 63) { discard; }
    ivec2 aoTexel = clamp(ivec2(vFace * vec2(vFaceSize)), ivec2(0), vFaceSize - 1);
    float ao = modelCrease((uint(vInfo.w) >> 3) & 255u, aoTexel.x, aoTexel.y, vFaceSize);
    ivec2 capTexel = clamp(ivec2(vRimFace * vec2(vCapSize)), ivec2(0), vCapSize - 1);
    float capAo = modelCrease((uint(vInfo.w) >> 11) & 255u, capTexel.x, capTexel.y, vCapSize);
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
        vec3 up = tiles[pc.k.x].ay.xyz;
        if (top) {
            c = litFace(base, p, n, ao);
            if (tone == 1) {
                c *= u.plantA.w;
            } else if (tone >= 32 && tone <= 47) {
                float gain = 0.5 + float(tone & 15) / 16.0;
                c = mix(c, c / u.plantA.x * gain, u.shadeB.y);
            }
        } else if (tone == 3) {
            c = litFace(base, vRimWorld, up, capAo);
        } else {
            if (tone == 1) {
                base *= u.shadeB.x;
            } else if (tone == 4 || tone == 5) {
                base = mix(base, pal[entry].base.rgb, u.plantB.x);
                if (tone == 5) { base *= u.plantA.w; }
            } else if (tone >= 16 && tone <= 31) {
                base *= 0.5 + float(tone & 15) / 16.0;
            }
            c = litFace(base, p, n, ao);
            if (tone == 2 && openUp) {
                c = mix(c, litFace(base, vRimWorld, up, capAo), u.plantA.z);
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
