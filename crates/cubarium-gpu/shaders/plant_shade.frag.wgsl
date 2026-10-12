// The plant pass's shade stage (`plant_main.wgsl`): each raster pixel's plant samples,
// lit and laid over the raster, and the occluder the figures test against.

// #import plant_main.wgsl

@group(2) @binding(0) var plantIds: texture_2d<u32>;
@group(2) @binding(1) var plantDepths: texture_depth_2d;
@group(2) @binding(2) var walkDepth: texture_2d<f32>;

struct ShadeOut {
    @location(0) colour: vec4<f32>,
    @location(1) emit: vec4<f32>,
    @location(2) plantOccluder: f32,
    @location(3) plantWeatherSurface: vec4<f32>,
}

// The most layer samples a pixel takes (`plants::MAX_SUPERSAMPLE` squared).
const PLANT_SAMPLES = 16;

@fragment
fn main(@builtin(position) fragCoord: vec4<f32>) -> ShadeOut {
    setWorld();
    let px = vec2<i32>(fragCoord.xy);
    let wz = textureLoad(walkDepth, px, 0).r;
    var out: ShadeOut;
    out.plantOccluder = wz;
    out.colour = vec4(0.0);
    out.emit = vec4(0.0);
    out.plantWeatherSurface = vec4(0.0);
    let aa = clamp(pc.k.y, 1, 4);
    var ids: array<u32, PLANT_SAMPLES>;
    var counts: array<f32, PLANT_SAMPLES>;
    var m = 0;
    for (var j = 0; j < aa; j++) {
        for (var i = 0; i < aa; i++) {
            let at2 = px * aa + vec2(i, j);
            let id = textureLoad(plantIds, at2, 0).r;
            if (id == 0u) { continue; }
            let z = plantZ(textureLoad(plantDepths, at2, 0));
            // Terrain (and whatever the walk still draws) in front hides the sample.
            if (z > wz + pc.t.z) { continue; }
            var k = 0;
            for (; k < m; k++) {
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
    if (m == 0) { return out; }
    var col = vec3(0.0);
    var em = vec3(0.0);
    var cover = 0.0;
    // The occluder takes each shown face's depth at the pixel's centre, as the walk's own
    // depth would have been with the cell in it, not a sample's.
    var zNear = 3.0e38;
    let py = fragCoord.y;
    for (var k = 0; k < m; k++) {
        let id = ids[k];
        let c = cells[i32(id >> 13u) - 1];
        let cap = (id & 4096u) != 0u;
        let dx = i32((id >> 6u) & 63u);
        let row = i32(id & 63u);
        var v: vec4<u32>;
        plantVoxel(c, &v);
        let p = plantCellAt(c);
        var rgb = vec3(0.0);
        var glow = vec3(0.0);
        var shown: bool;
        if (cap) {
            shown = glyphCapAt(p.x, p.y, p.z, v, row, dx, &rgb, &glow);
        } else {
            shown = glyphFrontAt(p.x, p.y, p.z, v, row, dx, &rgb, &glow);
        }
        if (!shown) { continue; }
        zNear = min(zNear, plantFaceZ(c, cap, py));
        // The flat tier's day and night, as the walk tints what it gathered.
        if (!LIT) { rgb *= u.tint.rgb; }
        col += counts[k] * rgb;
        cover += counts[k];
        if (LIT && any(glow > vec3(0.0))) {
            let hz = hazeAt(f32(p.z) + select(0.0, f32(RISE - 1 - row) / f32(RISE), cap));
            em += counts[k] * max(glow - clamp(hz, 0.0, 1.0) * u.hazeC.rgb, vec3(0.0));
        }
    }
    if (cover <= 0.0) { return out; }
    let inv = 1.0 / f32(aa * aa);
    out.colour = vec4(col * inv, cover * inv);
    out.plantWeatherSurface = out.colour;
    out.emit = vec4(em * inv, cover * inv);
    out.plantOccluder = min(wz, zNear);
    return out;
}
