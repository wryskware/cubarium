// The existing present.frag algorithm, with transform data in a uniform buffer.
struct Present { col0: vec4<f32>, offset: vec4<f32> }
@group(0) @binding(0) var raster: texture_2d<f32>;
@group(0) @binding(1) var<uniform> p: Present;

@vertex fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn area_average(low: vec2<f32>, high: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    let over = max(high - low - 7.0, vec2<f32>(0.0));
    let lo = max(low + 0.5 * over, vec2<f32>(0.0));
    let hi = min(high - 0.5 * over, size);
    let i0 = vec2<i32>(floor(lo));
    let i1 = min(vec2<i32>(ceil(hi)) - vec2<i32>(1), vec2<i32>(size) - vec2<i32>(1));
    var sum = vec3<f32>(0.0);
    var total = 0.0;
    for (var y = i0.y; y <= i1.y && y < i0.y + 8; y += 1) {
        let wy = min(hi.y, f32(y + 1)) - max(lo.y, f32(y));
        if (wy <= 1e-4) { continue; }
        for (var x = i0.x; x <= i1.x && x < i0.x + 8; x += 1) {
            let wx = min(hi.x, f32(x + 1)) - max(lo.x, f32(x));
            if (wx <= 1e-4) { continue; }
            sum += wx * wy * textureLoad(raster, vec2<i32>(x, y), 0).rgb;
            total += wx * wy;
        }
    }
    return vec4<f32>(sum / max(total, 1e-6), total);
}

@fragment fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let panel = floor(position.xy);
    var c: vec4<f32>;
    if (p.offset.w > 0.5) {
        let size = vec2<f32>(textureDimensions(raster));
        let step_size = abs(vec2<f32>(p.col0.x, p.col0.w));
        let centre = step_size * panel + p.offset.xy;
        let lo = centre - 0.5 * step_size;
        let hi = centre + 0.5 * step_size;
        if (any(hi <= vec2<f32>(0.0)) || any(lo >= size)) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
        c = area_average(lo, hi, size);
        if (c.a <= 1e-4) { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
    } else {
        let src = floor(p.col0.xy * panel.x + p.col0.zw * panel.y + p.offset.xy);
        c = textureLoad(raster, vec2<i32>(src), 0);
    }
    if (p.offset.z > 0.5) {
        let lo = c.rgb * 12.92;
        let hi = 1.055 * pow(max(c.rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
        c = vec4<f32>(clamp(select(hi, lo, c.rgb <= vec3<f32>(0.0031308)), vec3<f32>(0.0), vec3<f32>(1.0)), c.a);
    }
    return vec4<f32>(c.rgb, 1.0);
}
