// The model pass's composite (H2 of `design/handoffs/hybrid-renderer-2026-10-07.md`): each
// figure's tile placed at its anchor's exact projection, over the walk's raster.
//
// **Coverage AA.** A raster pixel covers aa*aa tile texels at a whole-pixel anchor, or
// up to (aa+1)*(aa+1) at a fractional anchor. Every face tone is shaded on that finer
// lattice. The box downsample preserves one-pixel axis-aligned lines and integrates
// turned silhouettes, rims and creases rather than resampling a finished low-res tile.
//
// **Depth.** Each texel carries the z of the face it shows; it counts only where that is
// in front of the walk's own hit at this pixel (`voxel_main.wgsl`'s `depth`), so terrain
// and plants hide a figure behind them and a figure hides what is behind it.
//
// **Figures over figures.** Each fragment writes the nearest z of its texels as its depth,
// and the composite keeps the nearest (`figures.rs` draws them back to front as well, so a
// figure's soft edge blends over the one behind it).
//
// **Cross-fades.** A figure may name two tiles, weighted: a pose's model swap, or a turn
// between two of the fine angle steps. Each texel of either counts by its weight, so the
// two blend pixel by pixel. Birth and death fade the whole figure; a highlight leans it
// toward white. Out goes premultiplied colour over the raster, and the emitters' share
// over the emission (its alpha, the weather's depth, kept).

// #import figure.wgsl

@group(2) @binding(0) var atlasColour: texture_2d<f32>;
@group(2) @binding(1) var atlasEmit: texture_2d<f32>;
@group(2) @binding(2) var atlasZ: texture_2d<f32>;
@group(2) @binding(3) var walkDepth: texture_2d<f32>;

struct CompositeOut {
    @location(0) colour: vec4<f32>,
    @location(1) emit: vec4<f32>,
    @location(2) figureWeatherSurface: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

// One tile's coverage of this pixel: premultiplied colour and coverage, and emission.
// `zNear` keeps the nearest z of the texels that count.
fn tileOverOne(t: Tile, tc: vec2<f32>, wz: f32, weight: f32, col: ptr<function, vec4<f32>>,
               em: ptr<function, vec3<f32>>, zNear: ptr<function, f32>) {
    let g = tc - 0.5;
    let i0 = vec2<i32>(floor(g));
    let f = g - vec2<f32>(i0);
    for (var j = 0; j < 4; j++) {
        let o = vec2(j & 1, j >> 1u);
        let ti = i0 + o;
        if (any(ti < vec2(0)) || any(ti >= t.rect.zw)) { continue; }
        let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1) * weight;
        if (w <= 0.0) { continue; }
        let at = t.rect.xy + ti;
        let c = textureLoad(atlasColour, at, 0);
        if (c.a < 0.5) { continue; }
        let tz = textureLoad(atlasZ, at, 0).r;
        if (tz > wz + pc.atlas.z) { continue; }
        *zNear = min(*zNear, tz);
        *col += w * vec4(c.rgb, 1.0);
        *em += w * textureLoad(atlasEmit, at, 0).rgb;
    }
}

fn tileOver(t: Tile, tc: vec2<f32>, wz: f32, weight: f32, col: ptr<function, vec4<f32>>,
            em: ptr<function, vec3<f32>>, zNear: ptr<function, f32>) {
    let aa = t.faceSize.w;
    if (aa == 1) {
        // Preserve the original one-sample arithmetic as well as its sampling phase.
        tileOverOne(t, tc, wz, weight, col, em, zNear);
        return;
    }
    let scale = f32(aa);
    let lo = (tc - 0.5) * scale;
    let hi = lo + scale;
    let i0 = vec2<i32>(floor(lo));
    // The last row/column has zero area at a whole-pixel anchor. At a fractional
    // anchor it accounts for the sample strip entering the shifted footprint.
    for (var y = 0; y <= aa; y++) {
        for (var x = 0; x <= aa; x++) {
            let ti = i0 + vec2(x, y);
            if (any(ti < vec2(0)) || any(ti >= t.rect.zw)) { continue; }
            let area = max(min(hi, vec2<f32>(ti + 1)) - max(lo, vec2<f32>(ti)), vec2(0.0));
            let w = area.x * area.y * weight / (scale * scale);
            if (w <= 0.0) { continue; }
            let at = t.rect.xy + ti;
            let c = textureLoad(atlasColour, at, 0);
            if (c.a <= 0.0) { continue; }
            let tz = textureLoad(atlasZ, at, 0).r;
            if (tz > wz + pc.atlas.z) { continue; }
            *zNear = min(*zNear, tz);
            *col += w * vec4(c.rgb * c.a, c.a);
            *em += w * c.a * textureLoad(atlasEmit, at, 0).rgb;
        }
    }
}

@fragment
fn main(@builtin(position) fragCoord: vec4<f32>, @location(0) @interpolate(flat) vFig: i32) -> CompositeOut {
    let px = vec2<i32>(fragCoord.xy);
    if (px.x < 0 || px.y < 0 || px.x >= pc.k.x || px.y >= pc.k.w) { discard; }
    let wz = textureLoad(walkDepth, px, 0).r;
    let f = figs[vFig];
    let c = fragCoord.xy - f.anchor.xy;
    var col = vec4(0.0);
    var em = vec3(0.0);
    var zNear = 3.0e38;
    let wb = select(0.0, clamp(f.anchor.z, 0.0, 1.0), f.tiles.y >= 0);
    let a = tiles[f.tiles.x];
    tileOver(a, c - vec2<f32>(a.orig.xy), wz, 1.0 - wb, &col, &em, &zNear);
    if (f.tiles.y >= 0) {
        let b = tiles[f.tiles.y];
        tileOver(b, c - vec2<f32>(b.orig.xy), wz, wb, &col, &em, &zNear);
    }
    if (col.a <= 0.0) { discard; }
    var out: CompositeOut;
    // Figures against each other, per pixel: the composite's depth test keeps the nearest
    // figure's texels whatever order the figures are drawn in, by the walk's convention (z,
    // larger is farther; along one pixel's ray z only grows). `pc.atlas.w` is the world's
    // depth; the range has room either side for a body reaching past the strip.
    out.depth = clamp((zNear + 16.0) / (pc.atlas.w + 32.0), 0.0, 1.0);
    let fade = clamp(f.anchor.w, 0.0, 1.0);
    col *= fade;
    em *= fade;
    col = vec4(mix(col.rgb, vec3(col.a), clamp(f.k.x, 0.0, 1.0)), col.a);
    out.colour = col;
    out.figureWeatherSurface = col;
    out.emit = vec4(em, col.a);
    return out;
}
