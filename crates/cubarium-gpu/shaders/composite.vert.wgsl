// The model pass's composite (H2): one quad a figure, covering its tiles where its anchor
// puts them in the raster.

// #import figure.wgsl

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) vFig: i32,
}

@vertex
fn main(@builtin(instance_index) instanceIndex: u32, @builtin(vertex_index) vertexIndex: u32) -> Varyings {
    let instance = i32(instanceIndex);
    let vertex = i32(vertexIndex);
    let f = figs[instance];
    let a = tiles[f.tiles.x];
    var lo = vec2<f32>(a.orig.xy);
    var hi = lo + vec2<f32>(a.rect.zw) / f32(a.faceSize.w);
    if (f.tiles.y >= 0) {
        let b = tiles[f.tiles.y];
        lo = min(lo, vec2<f32>(b.orig.xy));
        hi = max(hi, vec2<f32>(b.orig.xy) + vec2<f32>(b.rect.zw) / f32(b.faceSize.w));
    }
    // A texel's footprint reaches one raster pixel past its edge at a fractional anchor.
    lo = floor(f.anchor.xy + lo) - 1.0;
    hi = ceil(f.anchor.xy + hi) + 1.0;
    let corner = vec2(f32(vertex & 1), f32((vertex >> 1u) & 1));
    let px = mix(lo, hi, corner);
    let position = vec4(px / vec2(f32(pc.k.x), f32(pc.k.w)) * 2.0 - 1.0, 0.0, 1.0);
    var out: Varyings;
    // Written for Vulkan's downward clip-space y; WebGPU's points up.
    out.position = vec4(position.x, -position.y, position.zw);
    out.vFig = instance;
    return out;
}
