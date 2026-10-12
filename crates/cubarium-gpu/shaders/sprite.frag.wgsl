// #import scene.wgsl

// The GPU's `cubarium_render::stamp_layers_bent_toned`, for up to four weighted frames.
//
// Nearest sampling, always: the tile coordinate is floored to a whole source texel, so
// every texel covers an exact S x S block of raster pixels and nothing is filtered. The
// one sub-pixel quantity a stamp carries is the wind's bend, and it is rounded to a whole
// source texel here unless `u.knobs.w` asks for the sub-texel lean (`--gpu-bend-substep`),
// which trades the block rule -- and only for the bend's own displacement -- for a
// smoother breeze at S >= 2.

@group(0) @binding(3) var atlas: texture_2d<f32>;
// The per-frame page the Lanternjaw's freshly rasterised parts are uploaded into,
// already premultiplied and already linear (see `SOURCE_SCRATCH`).
@group(0) @binding(4) var scratch: texture_2d<f32>;

struct Varyings {
    @builtin(position) fragCoord: vec4<f32>,
    @location(0) @interpolate(flat) vPlace: vec4<f32>,
    @location(1) @interpolate(flat) vFrames01: vec4<u32>,
    @location(2) @interpolate(flat) vFrames23: vec4<u32>,
    @location(3) @interpolate(flat) vSizePivot: vec4<u32>,
    @location(4) @interpolate(flat) vWeights: vec4<f32>,
    @location(5) @interpolate(flat) vBend: vec4<f32>,
    @location(6) @interpolate(flat) vMask: vec4<f32>,
    @location(7) @interpolate(flat) vTone: vec4<f32>,
    @location(8) @interpolate(flat) vShade: vec4<f32>,
}

fn unit(v: f32) -> f32 { return clamp(v, 0.0, 1.0); }

// One texel of one frame, premultiplied and linear, transparent outside the tile —
// exactly `Sprite::pixel`'s out-of-range rule. The pack is straight sRGB + alpha, so the
// premultiply happens after the texture's decode -- exactly `Sprite::from_rgba`'s
// `srgb_decode(c) * a`.
fn texelOf(origin: vec2<u32>, texel: vec2<i32>, tile: vec2<f32>, fromScratch: bool) -> vec4<f32> {
    if (texel.x < 0 || texel.y < 0 || texel.x >= i32(tile.x) || texel.y >= i32(tile.y)) {
        return vec4(0.0);
    }
    if (fromScratch) {
        // Already premultiplied linear: the rig's rasteriser produced it that way.
        return textureLoad(scratch, vec2<i32>(origin) + texel, 0);
    }
    let t = textureLoad(atlas, vec2<i32>(origin) + texel, 0);
    return vec4(t.rgb * t.a, t.a);
}

// One frame's contribution at tile coordinate `src`.
//
// Nearest by default: `floor(src)`, which is what puts every source texel on a whole
// S x S block. With `u.time.z` set (`--gpu-filter bilinear`) it is instead
// `Sprite::sample`'s own four taps — `p = src − 0.5`, the four texels around it, the
// four bilinear weights — which is the CPU presenter's sampler, bit for bit, and is
// there so the two renderers can be compared with only the sampler between them.
fn frameAt(origin: vec2<u32>, src: vec2<f32>, tile: vec2<f32>, weight: f32, fromScratch: bool) -> vec4<f32> {
    if (weight <= 0.0) { return vec4(0.0); }
    if (u.time.z > 0.5) {
        let p = src - vec2(0.5);
        let t0 = vec2<i32>(floor(p));
        let fr = p - vec2<f32>(t0);
        let sum = texelOf(origin, t0, tile, fromScratch) * ((1.0 - fr.x) * (1.0 - fr.y))
                + texelOf(origin, t0 + vec2(1, 0), tile, fromScratch) * (fr.x * (1.0 - fr.y))
                + texelOf(origin, t0 + vec2(0, 1), tile, fromScratch) * ((1.0 - fr.x) * fr.y)
                + texelOf(origin, t0 + vec2(1, 1), tile, fromScratch) * (fr.x * fr.y);
        return sum * weight;
    }
    return texelOf(origin, vec2<i32>(floor(src)), tile, fromScratch) * weight;
}

@fragment
fn main(in: Varyings) -> @location(0) vec4<f32> {
    let pivot = vec2<f32>(in.vSizePivot.zw);
    let tile = vec2<f32>(in.vSizePivot.xy);

    // The tile coordinate of *this* pixel's centre, computed from the fragment position
    // and the flat instance data rather than interpolated across the quad. A nearest
    // read turns a half-ulp of interpolation error at a texel boundary into a whole wrong
    // texel; computing it here makes the coordinate a function of the pixel centre and
    // the instance alone, which every conformant implementation agrees on.
    let offset = (in.fragCoord.xy - in.vPlace.xy) / (u.time.w * max(in.vShade.z, 1e-3));
    let heading = in.vPlace.zw;
    let vLocal = vec2(dot(offset, heading), dot(offset, vec2(-heading.y, heading.x)));

    // `Bend::displacement`: H = (tile_height - p_y) + base, D = amplitude * smoothstep.
    // The row is never displaced, only the column, so the material rows are preserved and
    // the mask below still reads the row it was authored against.
    let py = vLocal.y + pivot.y;
    var d = 0.0;
    if (in.vBend.x != 0.0 && in.vBend.w > 0.0) {
        let t = clamp((tile.y - py + in.vBend.y - in.vBend.z) / in.vBend.w, 0.0, 1.0);
        d = in.vBend.x * (t * t * (3.0 - 2.0 * t));
        if (u.knobs.w < 0.5) { d = round(d); }
    }
    let src = vec2(vLocal.x - d, vLocal.y) + pivot;
    // Nearest mode can reject the whole fragment early; the filtered one cannot, because
    // a sample half a texel outside the tile still carries part of the edge texel.
    if (u.time.z <= 0.5
        && (src.x < 0.0 || src.y < 0.0 || src.x >= tile.x || src.y >= tile.y)) {
        discard;
    }

    // `Mask`, in the sprite's own material coordinates so a reveal covers the same
    // material however the wind displaces it. Axial and strip read the row; radial reads
    // the distance from the pivot, which is what a canopy plant opening from its centre
    // needs -- and a ring has canopy cells, unlike the cube where only Top did.
    //
    // It is evaluated **before** the frames, because it depends only on `src` and the
    // instance: a fragment the mask rejects costs no texel read at all. A tall
    // column's trunk strip reveals four rows of a sixteen-row tile, so this is most of
    // the Tall layer's sampling.
    var cover: f32;
    if (in.vMask.z > 0.5) {
        let r = length(src - pivot);
        cover = unit(in.vMask.y - r + 0.5) * unit(in.vMask.y);
    } else {
        let h = tile.y - src.y;
        cover = unit(in.vMask.y - h + 0.5) * unit(h - in.vMask.x + 0.5) * unit(in.vMask.y - max(in.vMask.x, 0.0));
    }
    let opacity = in.vMask.w;
    if (cover <= 0.0 || opacity <= 0.0) { discard; }

    let fromScratch = in.vShade.w > 0.5;
    var rgba = frameAt(in.vFrames01.xy, src, tile, in.vWeights.x, fromScratch)
             + frameAt(in.vFrames01.zw, src, tile, in.vWeights.y, fromScratch)
             + frameAt(in.vFrames23.xy, src, tile, in.vWeights.z, fromScratch)
             + frameAt(in.vFrames23.zw, src, tile, in.vWeights.w, fromScratch);
    rgba *= cover;

    // `Tone`: travel toward one colour at the same alpha, carrying the art's own light
    // through `Shade`, so the coverage of the stamp does not change with the mix.
    if (in.vTone.w > 0.0 && rgba.a > 0.0) {
        let luma = dot(rgba.rgb, vec3(0.2126, 0.7152, 0.0722)) / rgba.a;
        var lit = 1.0;
        if (in.vShade.y > 0.0) { lit = clamp(luma / in.vShade.y, 0.0, 1.0); }
        let s = (in.vShade.x + (1.0 - in.vShade.x) * lit) * rgba.a;
        rgba = vec4(rgba.rgb + (in.vTone.rgb * s - rgba.rgb) * in.vTone.w, rgba.a);
    }

    if (rgba.a * opacity <= 0.0) { discard; }
    return rgba * opacity;
}
