// --- drape cords (DRAPE-CORD, `cubarium_gpu::cord`, its CPU mirror and its tests) ---------
//
// A baked model's drape cell (ROLE_DRAPE) drawn as a thin cord down its front face instead
// of a whole cube, when `u.tex.z` (the cord's width in screen pixels) is not 0. The cord
// crosses each horizontal face at a hash of that face's position, so the cells above and
// below a face join there; it is `u.tex.z` pixels wide at every px_per_voxel (a face texel
// is a screen pixel); off it the cell is see-through. A drape cell's cap is never drawn (a
// cord seen from above is a point), and it neither casts the sun march's shadow nor creases
// the AO of the faces beside it. `u.tex.w`: the path inside the cell (low byte: 0 straight,
// 1 eased) and how far a join wanders across the face (second byte, hundredths).
//
// Included by `world.glsl` (needs `at`, `cellHash`, `styleTex`, `ROLE_DRAPE`, `S`).

uint glyphTexel(int part, int glyph, int dx, int row);  // world.glsl, further down

// Whether cell v draws as a cord: a crown cell of a drape style, with the cords on.
bool cordCell(uvec4 v) {
    return u.tex.z > 0 && partOf(v) == CROWN
        && int(texelFetch(styleTex, ivec2(0, int(v.a)), 0).a + 0.5) == ROLE_DRAPE;
}

// `DrapeCords::join`: the cord's centre, face pixels from the left edge, where it crosses
// the horizontal face at height yf of column (x, z).
float cordJoin(int x, int yf, int z) {
    int w = u.tex.z;
    int free = max(S - w, 0);
    int span = int(float(free) * float((u.tex.w >> 8) & 255) / 100.0 + 0.5);
    int lo = (free - span) / 2;
    uint h = cellHash(wrapX(x), yf, z, 77);
    return float(lo + int(h % uint(span + 1))) + 0.5 * float(w);
}

// `DrapeCords::ends`: cell (x, y, z)'s top join and bottom join.
vec2 cordEnds(int x, int y, int z) { return vec2(cordJoin(x, y + 1, z), cordJoin(x, y, z)); }

// `DrapeCords::centre`: the cord's centre at t down the face, between joins a and b.
float cordAt(float a, float b, float t) {
    float e = (u.tex.w & 255) == 1 ? t * t * (3.0 - 2.0 * t) : t;
    return mix(a, b, e);
}

// `DrapeCords::covers`: face point (xp face pixels across, t down) is on the cord.
bool cordCovers(float a, float b, float t, float xp) {
    float w = float(u.tex.z);
    float c = cordAt(a, b, t);
    float dt = 1.0 / float(S);
    float up = abs(c - cordAt(a, b, max(t - dt, 0.0)));
    float down = abs(cordAt(a, b, min(t + dt, 1.0)) - c);
    float ext = max(0.5 * max(up, down) - 0.5, 0.0);
    float r = xp - c;
    return r >= -0.5 * w - ext && r < 0.5 * w + ext;
}

// `DrapeCords::texel`, the walk's: front-face texel (dx, dy) of drape cell (x, y, z) is the
// cord's (dy 0 at the top).
bool cordTexel(int x, int y, int z, int dx, int dy) {
    vec2 e = cordEnds(x, y, z);
    return cordCovers(e.x, e.y, (float(dy) + 0.5) / float(S), float(dx) + 0.5);
}

// The glyph texel a cord pixel takes its tone from: the face's centre, whatever pixel the
// cord crosses, so the cord keeps one tone down its length (no edge tone where it passes
// a face's edge column).
uint cordGlyphTexel(uvec4 v, int dx, int row) {
    return cordCell(v) ? glyphTexel(partOf(v), glyphOf(v), S / 2, S / 2)
                       : glyphTexel(partOf(v), glyphOf(v), dx, row);
}
