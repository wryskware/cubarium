// The plant pass's fragment stages (`crates/cubarium-gpu/src/plants.rs`), compiled from
// `voxel.frag` so they call the walk's own face functions: `-DPLANT_VIS` and `-DPLANT_SHADE`.
//
// **Visibility** (`PLANT_VIS`): each layer sample a plant cell's face covers records which
// cell, which face and which of the stamped face's texels it shows, or nothing where the
// walk would have drawn nothing (a glyph's empty texel, a leaf cutout's hole). The depth
// test keeps the nearest. Nothing is lit here, so a canopy's hidden cells cost a few fetches.
//
// **Shade** (`PLANT_SHADE`), a full-screen pass before the figures: each raster pixel takes
// its layer samples in front of the walk's own end depth, lights every distinct texel among
// them once with `glyphFrontAt` / `glyphCapAt` at the cell's resting place (so a cell shows
// exactly the stamped cell's colours, light and shadow), and lays their mean over the
// raster by coverage. It writes the **occluder** for every pixel: the walk's depth, or the
// nearest plant sample in front of it, which the figures test their texels against as
// they tested them against the walk's depth when the plants were in it.

// Which voxel a cell draws: a leaving cell its carried texel; an arriving or settled one the
// volume's own, while it is still a dry block plant cell there (a cell flooded, or taken by
// an animal, is the walk's: `plantHidden`).
bool plantVoxel(uvec4 c, out uvec4 v) {
    if (plantDying(c)) {
        v = plantCarried(c);
        return true;
    }
    ivec3 p = plantCellAt(c);
    v = at(p.x, p.y, p.z);
    return plantHidden(v);
}

// Whether the walk draws texel (dx, row) of this face of cell (x, y, z) holding v: the
// glyph has a texel there (a front face's tone 63 is empty) and no cutout takes it, as
// `glyphFrontAt` and `glyphCapAt` decide. A cap's row counts from its back edge.
bool plantFaceShows(int x, int y, int z, uvec4 v, bool cap, int row, int dx) {
    uint q = glyphTexel(partOf(v), glyphOf(v), dx, cap ? S + row : row);
    vec4 pigment = texelFetch(styleTex, ivec2(int(q & 3u), int(v.a)), 0);
    int tone = int(q >> 2);
    if (!cap && tone == 63) { return false; }
    if (LIT && (tone == TONE_EMIT || emitsWhole(pigment.a))
        && any(greaterThan(styleEmit(int(v.a)), vec3(0.0)))) {
        return true;
    }
    vec3 base = pigment.rgb;
    return plantTexel(x, y, z, v, cap, dx, row, base);
}

#ifdef PLANT_VIS
layout(location = 0) flat in uint vId;
layout(location = 1) in vec2 vUV;

void main() {
    S = u.geom.x;
    RISE = u.geom.y;
    BASE = u.geom.z;
    W = u.extent.x;
    H = u.extent.y;
    D = u.extent.z;
    int inst = int(vId >> 1);
    bool cap = (vId & 1u) != 0u;
    uvec4 c = cells[inst];
    uvec4 v;
    if (!plantVoxel(c, v)) { discard; }
    ivec3 p = plantCellAt(c);
    int rows = cap ? RISE : S;
    int dx = clamp(int(vUV.x * float(S)), 0, S - 1);
    int row = clamp(int(vUV.y * float(rows)), 0, rows - 1);
    if (!plantFaceShows(p.x, p.y, p.z, v, cap, row, dx)) { discard; }
    outId = (uint(inst + 1) << 9) | (cap ? 256u : 0u) | (uint(dx) << 4) | uint(row);
}
#endif

#ifdef PLANT_SHADE
layout(set = 2, binding = 0) uniform usampler2D plantIds;
layout(set = 2, binding = 1) uniform sampler2D plantDepths;
layout(set = 2, binding = 2) uniform sampler2D walkDepth;

layout(location = 2) out float plantOccluder;

// The most layer samples a pixel takes (`plants::MAX_SUPERSAMPLE` squared).
const int PLANT_SAMPLES = 16;

void main() {
    S = u.geom.x;
    RISE = u.geom.y;
    BASE = u.geom.z;
    W = u.extent.x;
    H = u.extent.y;
    D = u.extent.z;
    ivec2 px = ivec2(gl_FragCoord.xy);
    float wz = texelFetch(walkDepth, px, 0).r;
    plantOccluder = wz;
    outColour = vec4(0.0);
    emitOut = vec4(0.0);
    int aa = clamp(pc.k.y, 1, 4);
    uint ids[PLANT_SAMPLES];
    float counts[PLANT_SAMPLES];
    int m = 0;
    for (int j = 0; j < aa; ++j) {
        for (int i = 0; i < aa; ++i) {
            ivec2 at2 = px * aa + ivec2(i, j);
            uint id = texelFetch(plantIds, at2, 0).r;
            if (id == 0u) { continue; }
            float z = plantZ(texelFetch(plantDepths, at2, 0).r);
            // Terrain (and whatever the walk still draws) in front hides the sample.
            if (z > wz + pc.t.z) { continue; }
            int k = 0;
            for (; k < m; ++k) {
                if (ids[k] == id) { break; }
            }
            if (k == m) {
                ids[m] = id;
                counts[m] = 0.0;
                m += 1;
            }
            counts[k] += 1.0;
        }
    }
    if (m == 0) { return; }
    vec3 col = vec3(0.0);
    vec3 em = vec3(0.0);
    float cover = 0.0;
    // The occluder takes each shown face's depth at the pixel's centre, as the walk's own
    // depth would have been with the cell in it, not a sample's.
    float zNear = 3.0e38;
    float py = gl_FragCoord.y;
    for (int k = 0; k < m; ++k) {
        uint id = ids[k];
        uvec4 c = cells[int(id >> 9) - 1];
        bool cap = (id & 256u) != 0u;
        int dx = int((id >> 4) & 15u);
        int row = int(id & 15u);
        uvec4 v;
        plantVoxel(c, v);
        ivec3 p = plantCellAt(c);
        vec3 rgb;
        bool lum;
        bool shown = cap ? glyphCapAt(p.x, p.y, p.z, v, row, dx, rgb, lum)
                         : glyphFrontAt(p.x, p.y, p.z, v, row, dx, rgb, lum);
        if (!shown) { continue; }
        zNear = min(zNear, plantFaceZ(c, cap, py));
        // The flat tier's day and night, as the walk tints what it gathered.
        if (!LIT) { rgb *= u.tint.rgb; }
        col += counts[k] * rgb;
        cover += counts[k];
        if (LIT && lum) {
            float hz = hazeAt(float(p.z) + (cap ? float(RISE - 1 - row) / float(RISE) : 0.0));
            em += counts[k] * max(rgb - clamp(hz, 0.0, 1.0) * u.hazeC.rgb, vec3(0.0));
        }
    }
    if (cover <= 0.0) { return; }
    float inv = 1.0 / float(aa * aa);
    outColour = vec4(col * inv, cover * inv);
    emitOut = vec4(em * inv, cover * inv);
    plantOccluder = min(wz, zNear);
}
#endif
