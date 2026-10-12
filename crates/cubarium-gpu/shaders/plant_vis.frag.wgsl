// The plant pass's visibility stage (`plant_main.wgsl`): which plant cell, face and texel
// each layer sample shows.

// #import plant_main.wgsl

struct Varyings {
    @builtin(position) fragCoord: vec4<f32>,
    @location(0) @interpolate(flat) vId: u32,
    @location(1) vUV: vec2<f32>,
}

// A drape cord's sample (`drape_cord.wgsl`), at `uv` across and down the swaying cell's
// front face: the joins move with their faces' weights (`plants::pack_faces`) while the box
// moves with the cell's, so the two cells meeting at a face put the join at one screen
// point (`cord::swayed_join`). A leaving cell carries no face weights: its joins move with
// it.
fn plantCordShows(c: vec4<u32>, p: vec3<i32>, uv: vec2<f32>) -> bool {
    let wc = plantWeight(c);
    var wf = vec2(wc);
    if (!plantDying(c) && (c.w >> 31u) != 0u) {
        wf = vec2(f32((c.w >> 12u) & 4095u), f32(c.w & 4095u)) / 4095.0;
    }
    let sh = tips[plantOf(c)] * (wf - wc) * f32(S);
    // The long wave: the faces' shifts carried linearly down the cell, uv.x past 0..1 where
    // the widened quad (`plant.vert.wgsl`) reaches into the next column.
    if (cordWave()) { return cordWaveCovers(p, uv.y, uv.x * f32(S), cordAmp(false), sh); }
    let e = cordEnds(p.x, p.y, p.z) + sh;
    return cordCovers(e.x, e.y, uv.y, uv.x * f32(S));
}

@fragment
fn main(in: Varyings) -> @location(0) u32 {
    setWorld();
    let inst = i32(in.vId >> 1u);
    let cap = (in.vId & 1u) != 0u;
    let c = cells[inst];
    var v: vec4<u32>;
    if (!plantVoxel(c, &v)) { discard; }
    let p = plantCellAt(c);
    let rows = select(S, RISE, cap);
    var uv = in.vUV;
    if (cordCell(v)) {
        if (cap) { discard; }
        // A cord is asked at the centre of the screen pixel this sample is in, so every
        // sample of a pixel agrees: a cord pixel is whole, as the walk draws it, never a
        // blend of a cord and what is behind it.
        let aa = f32(pc.k.y);
        let centre = (floor(in.fragCoord.xy / aa) + 0.5) * aa;
        uv += (centre - in.fragCoord.xy) / (aa * max(plantScale(c), 1e-3) * f32(S));
        if (!plantCordShows(c, p, uv)) { discard; }
    } else if (uv.x < 0.0 || uv.x >= 1.0) {
        discard;  // a leaving cell's quad is widened in case it is a cord (`plant.vert.wgsl`)
    }
    let dx = clamp(i32(uv.x * f32(S)), 0, S - 1);
    let row = clamp(i32(uv.y * f32(rows)), 0, rows - 1);
    if (!plantFaceShows(p.x, p.y, p.z, v, cap, row, dx)) { discard; }
    return (u32(inst + 1) << 13u) | select(0u, 4096u, cap) | (u32(dx) << 6u) | u32(row);
}
