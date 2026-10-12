// --- drape cords (DRAPE-CORD, `cubarium_gpu::cord`, its CPU mirror and its tests) ---------
//
// A baked model's drape cell (ROLE_DRAPE) drawn as a thin cord down its front face instead
// of a whole cube, when `u.tex.z` (the cord's width in screen pixels) is not 0. The cord
// crosses each horizontal face at a hash of that face's position, so the cells above and
// below a face join there; it is `u.tex.z` pixels wide at every px_per_voxel (a face texel
// is a screen pixel); off it the cell is see-through. A drape cell's cap is never drawn (a
// cord seen from above is a point), and it neither casts the sun march's shadow nor creases
// the AO of the faces beside it. `u.tex.w`: the path inside the cell (bit 0: 0 straight,
// 1 eased), the long wave's ratios (bit 1: jittered) and octave count (bits 4..8), how far a join wanders across the face (second byte, hundredths); the long
// wave's period (third byte, quarter voxels; 0 off) and amplitude (fourth, 32nds of a voxel).
//
// **The long wave** (round 2): with a period, the cord's centre is one sine down its whole
// strand (`cordWaveAt`), each column on its own phase, so the cells either side of a face
// read the same curve there. The plant pass widens a drape cell's front quad by the swing
// (`plant.vert.wgsl`, `PlantPush.t.w`), so a cord past half a voxel shows in the next
// column; the walk sees a cell only by entering it, so there the swing stays inside the
// column.
//
// **Octaves** (round 3): the curve sums `cordOctaves()` sines, octave k at about 2^k the
// base frequency (`cordFreq`: exactly, or jittered per strand round 1.15 * 2^k) and half the
// amplitude of the one below, each on its own phase per strand, over the sum of the gains
// so the swing stays within the amplitude (`cord::WaveRatios`, `GAIN`).
//
// Imported by `world.wgsl` (uses `at`, `cellHash`, `glyphTexel`, `styleTex`, `ROLE_DRAPE`,
// `S`).

// Whether cell v draws as a cord: a crown cell of a drape style, with the cords on.
fn cordCell(v: vec4<u32>) -> bool {
    return u.tex.z > 0 && partOf(v) == CROWN
        && i32(textureLoad(styleTex, vec2(0, i32(v.a)), 0).a + 0.5) == ROLE_DRAPE;
}

// `DrapeCords::join`: the cord's centre, face pixels from the left edge, where it crosses
// the horizontal face at height yf of column (x, z).
fn cordJoin(x: i32, yf: i32, z: i32) -> f32 {
    let w = u.tex.z;
    let free = max(S - w, 0);
    let span = i32(f32(free) * f32((u.tex.w >> 8u) & 255) / 100.0 + 0.5);
    let lo = (free - span) / 2;
    let h = cellHash(wrapX(x), yf, z, 77);
    return f32(lo + i32(h % u32(span + 1))) + 0.5 * f32(w);
}

// `DrapeCords::ends`: cell (x, y, z)'s top join and bottom join.
fn cordEnds(x: i32, y: i32, z: i32) -> vec2<f32> { return vec2(cordJoin(x, y + 1, z), cordJoin(x, y, z)); }

// `DrapeCords::centre`: the cord's centre at t down the face, between joins a and b.
fn cordAt(a: f32, b: f32, t: f32) -> f32 {
    let e = select(t, t * t * (3.0 - 2.0 * t), (u.tex.w & 1) == 1);
    return mix(a, b, e);
}

// `DrapeCords::covers`: face point (xp face pixels across, t down) is on the cord.
fn cordCovers(a: f32, b: f32, t: f32, xp: f32) -> bool {
    let w = f32(u.tex.z);
    let c = cordAt(a, b, t);
    let dt = 1.0 / f32(S);
    let up = abs(c - cordAt(a, b, max(t - dt, 0.0)));
    let down = abs(cordAt(a, b, min(t + dt, 1.0)) - c);
    let ext = max(0.5 * max(up, down) - 0.5, 0.0);
    let r = xp - c;
    return r >= -0.5 * w - ext && r < 0.5 * w + ext;
}

// `DrapeCords::amp`, `CordWave`: the long wave's period and amplitude (voxels).
fn cordPeriod() -> f32 { return f32((u.tex.w >> 16u) & 255) / 4.0; }
fn cordWave() -> bool { return ((u.tex.w >> 16u) & 255) != 0; }
fn cordAmp(walk: bool) -> f32 {
    let a = f32((u.tex.w >> 24u) & 255) / 32.0;
    return select(a, min(a, max(f32(S - u.tex.z) / (2.0 * f32(S)), 0.0)), walk);
}

// `cord::phase`: strand (column) (x, z)'s place in its wave at height 0, 0..1.
// `cord::octave_phase`: strand (x, z)'s place in octave k's wave at height 0, 0..1.
fn cordPhase(x: i32, z: i32, k: i32) -> f32 { return f32(cellHash(wrapX(x), k, z, 78) >> 8u) / 16777216.0; }

// `DrapeCords::freq`: octave k's frequency on strand (x, z) over the base's.
fn cordFreq(x: i32, z: i32, k: i32) -> f32 {
    let o = f32(1 << u32(k));
    if (k == 0 || (u.tex.w & 2) == 0) { return o; }
    let h = f32(cellHash(wrapX(x), k, z, 79) >> 8u) / 16777216.0;
    return o * (1.15 + 0.05 * (2.0 * h - 1.0));
}

// `CordWave::octave_count`: the octaves the curve sums, 1..15.
fn cordOctaves() -> i32 { return max((u.tex.w >> 4u) & 15, 1); }

// `DrapeCords::wave_at`: the centre on strand (x, z) at height yw (voxels), face pixels from
// the column's left edge.
fn cordWaveAt(x: i32, z: i32, yw: f32, amp: f32) -> f32 {
    let p = max(cordPeriod(), 0.25);
    var sum = 0.0;
    var g = 1.0;
    var gs = 0.0;
    let n = cordOctaves();
    for (var k = 0; k < n; k++) {
        sum += g * sin(6.28318530718 * (yw * cordFreq(x, z, k) / p + cordPhase(x, z, k)));
        gs += g;
        g *= 0.5;
    }
    return f32(S) * (0.5 + amp * sum / gs);
}

// `DrapeCords::wave_centre`: at t down cell p's front face, shifted by sh (top, bottom) px.
fn cordWaveCentre(p: vec3<i32>, t: f32, amp: f32, sh: vec2<f32>) -> f32 {
    return cordWaveAt(p.x, p.z, f32(p.y) + 1.0 - t, amp) + mix(sh.x, sh.y, t);
}

// `cordSpan` (`cord::span`): xp is on a row centred at c, its neighbours up and down px off.
fn cordSpan(c: f32, up: f32, down: f32, xp: f32) -> bool {
    let w = f32(u.tex.z);
    let ext = max(0.5 * max(up, down) - 0.5, 0.0);
    let r = xp - c;
    return r >= -0.5 * w - ext && r < 0.5 * w + ext;
}

// `DrapeCords::wave_covers`: face point (xp face pixels from the cell's left edge, any sign;
// t down) of cell p is on the waving cord.
fn cordWaveCovers(p: vec3<i32>, t: f32, xp: f32, amp: f32, sh: vec2<f32>) -> bool {
    let dt = 1.0 / f32(S);
    let c = cordWaveCentre(p, t, amp, sh);
    let up = abs(c - cordWaveCentre(p, t - dt, amp, sh));
    let down = abs(cordWaveCentre(p, t + dt, amp, sh) - c);
    return cordSpan(c, up, down, xp);
}

// `DrapeCords::texel`, the walk's: front-face texel (dx, dy) of drape cell (x, y, z) is the
// cord's (dy 0 at the top).
fn cordTexel(x: i32, y: i32, z: i32, dx: i32, dy: i32) -> bool {
    if (cordWave()) {
        let t = (f32(dy) + 0.5) / f32(S);
        return cordWaveCovers(vec3(x, y, z), t, f32(dx) + 0.5, cordAmp(true), vec2(0.0));
    }
    let e = cordEnds(x, y, z);
    return cordCovers(e.x, e.y, (f32(dy) + 0.5) / f32(S), f32(dx) + 0.5);
}

// The glyph texel a cord pixel takes its tone from: the face's centre, whatever pixel the
// cord crosses, so the cord keeps one tone down its length (no edge tone where it passes
// a face's edge column).
fn cordGlyphTexel(v: vec4<u32>, dx: i32, row: i32) -> u32 {
    if (cordCell(v)) { return glyphTexel(partOf(v), glyphOf(v), S / 2, S / 2); }
    return glyphTexel(partOf(v), glyphOf(v), dx, row);
}
