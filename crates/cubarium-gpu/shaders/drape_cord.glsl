// --- drape cords (DRAPE-CORD, `cubarium_gpu::cord`, its CPU mirror and its tests) ---------
//
// A baked model's drape cell (ROLE_DRAPE) drawn as a thin cord down its front face instead
// of a whole cube, when `u.tex.z` (the cord's width in screen pixels) is not 0. The cord
// crosses each horizontal face at a hash of that face's position, so the cells above and
// below a face join there; it is `u.tex.z` pixels wide at every px_per_voxel (a face texel
// is a screen pixel); off it the cell is see-through. A drape cell's cap is never drawn (a
// cord seen from above is a point), and it neither casts the sun march's shadow nor creases
// the AO of the faces beside it. `u.tex.w`: the path inside the cell (low byte: 0 straight,
// 1 eased) and how far a join wanders across the face (second byte, hundredths); the long
// wave's period (third byte, quarter voxels; 0 off) and amplitude (fourth, 32nds of a voxel).
//
// **The long wave** (round 2): with a period, the cord's centre is one sine down its whole
// strand (`cordWaveAt`), each column on its own phase, so the cells either side of a face
// read the same curve there. The plant pass widens a drape cell's front quad by the swing
// (`plant.vert`, `PlantPush.t.w`), so a cord past half a voxel shows in the next column;
// the walk sees a cell only by entering it, so there the swing stays inside the column.
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

// `DrapeCords::amp`, `CordWave`: the long wave's period and amplitude (voxels).
float cordPeriod() { return float((u.tex.w >> 16) & 255) / 4.0; }
bool cordWave() { return ((u.tex.w >> 16) & 255) != 0; }
float cordAmp(bool walk) {
    float a = float((u.tex.w >> 24) & 255) / 32.0;
    return walk ? min(a, max(float(S - u.tex.z) / (2.0 * float(S)), 0.0)) : a;
}

// `cord::phase`: strand (column) (x, z)'s place in its wave at height 0, 0..1.
float cordPhase(int x, int z) { return float(cellHash(wrapX(x), 0, z, 78) >> 8u) / 16777216.0; }

// `DrapeCords::wave_at`: the centre on strand (x, z) at height yw (voxels), face pixels from
// the column's left edge.
float cordWaveAt(int x, int z, float yw, float amp) {
    float a = 6.28318530718 * (yw / max(cordPeriod(), 0.25) + cordPhase(x, z));
    return float(S) * (0.5 + amp * sin(a));
}

// `DrapeCords::wave_centre`: at t down cell p's front face, shifted by sh (top, bottom) px.
float cordWaveCentre(ivec3 p, float t, float amp, vec2 sh) {
    return cordWaveAt(p.x, p.z, float(p.y) + 1.0 - t, amp) + mix(sh.x, sh.y, t);
}

// `cordSpan` (`cord::span`): xp is on a row centred at c, its neighbours up and down px off.
bool cordSpan(float c, float up, float down, float xp) {
    float w = float(u.tex.z);
    float ext = max(0.5 * max(up, down) - 0.5, 0.0);
    float r = xp - c;
    return r >= -0.5 * w - ext && r < 0.5 * w + ext;
}

// `DrapeCords::wave_covers`: face point (xp face pixels from the cell's left edge, any sign;
// t down) of cell p is on the waving cord.
bool cordWaveCovers(ivec3 p, float t, float xp, float amp, vec2 sh) {
    float dt = 1.0 / float(S);
    float c = cordWaveCentre(p, t, amp, sh);
    float up = abs(c - cordWaveCentre(p, t - dt, amp, sh));
    float down = abs(cordWaveCentre(p, t + dt, amp, sh) - c);
    return cordSpan(c, up, down, xp);
}

// `DrapeCords::texel`, the walk's: front-face texel (dx, dy) of drape cell (x, y, z) is the
// cord's (dy 0 at the top).
bool cordTexel(int x, int y, int z, int dx, int dy) {
    if (cordWave()) {
        float t = (float(dy) + 0.5) / float(S);
        return cordWaveCovers(ivec3(x, y, z), t, float(dx) + 0.5, cordAmp(true), vec2(0.0));
    }
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
