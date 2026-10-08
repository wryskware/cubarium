#version 450
// The final pass, and the only one that runs at panel resolution: a nearest-neighbour
// integer upscale of the world raster, plus the panel's 90-degree rotation, in one
// `texelFetch`. `xform` maps a panel pixel to a raster pixel; the Rust side builds it
// from the quarter-turn count and the integer factor k, so the shader has no idea the
// panel is portrait.
//
// A desktop window (`WindowFit`, `offset.w = 1`) is drawn differently: at any scale,
// each window pixel the **area average** of the raster it covers (a box filter, in
// linear light). At a whole upscale that is exactly the nearest upscale; at any other
// scale a texel edge is one blended pixel, and a downscale is supersampled.
layout(set = 0, binding = 0) uniform sampler2D raster;

layout(push_constant) uniform Present {
    vec4 col0;   // xy = the matrix's first column, zw = its second
    vec4 offset; // xy = the translation, z = 1 to encode sRGB here, w = 1 to letterbox
} p;

layout(location = 0) out vec4 outColour;

// Raster texels a window pixel's footprint may span along one axis: a downscale by up to
// MAX_TAPS - 1 is exact; a smaller window averages the middle MAX_TAPS of its footprint.
const int MAX_TAPS = 8;

// The area-weighted average of the raster over `lo..hi` (raster pixels), clamped to the
// raster: a footprint half outside it is the inside half's colour, so the picture's edge
// stays sharp against the bar.
vec4 areaAverage(vec2 lo, vec2 hi, vec2 size) {
    vec2 span = hi - lo;
    vec2 over = max(span - float(MAX_TAPS - 1), vec2(0.0));
    lo += 0.5 * over;
    hi -= 0.5 * over;
    lo = max(lo, vec2(0.0));
    hi = min(hi, size);
    ivec2 i0 = ivec2(floor(lo));
    ivec2 i1 = min(ivec2(ceil(hi)) - 1, ivec2(size) - 1);
    vec3 sum = vec3(0.0);
    float total = 0.0;
    for (int y = i0.y; y <= i1.y && y < i0.y + MAX_TAPS; y++) {
        float wy = min(hi.y, float(y + 1)) - max(lo.y, float(y));
        if (wy <= 1e-4) { continue; }
        for (int x = i0.x; x <= i1.x && x < i0.x + MAX_TAPS; x++) {
            float wx = min(hi.x, float(x + 1)) - max(lo.x, float(x));
            if (wx <= 1e-4) { continue; }
            sum += wx * wy * texelFetch(raster, ivec2(x, y), 0).rgb;
            total += wx * wy;
        }
    }
    return vec4(sum / max(total, 1e-6), total);
}

void main() {
    vec2 panel = floor(gl_FragCoord.xy);
    vec4 c;
    if (p.offset.w > 0.5) {
        // A desktop window (`WindowFit`): the picture is centred in whatever size the
        // window is, and a pixel whose footprint misses the raster is a black bar. The
        // panel never sets this; its transform always lands inside the raster. The
        // window's matrix is a plain scale, `step` raster pixels a window pixel.
        vec2 size = vec2(textureSize(raster, 0));
        vec2 step = abs(vec2(p.col0.x, p.col0.w));
        vec2 centre = step * panel + p.offset.xy;
        vec2 lo = centre - 0.5 * step;
        vec2 hi = centre + 0.5 * step;
        if (any(lessThanEqual(hi, vec2(0.0))) || any(greaterThanEqual(lo, size))) {
            outColour = vec4(0.0, 0.0, 0.0, 1.0);
            return;
        }
        c = areaAverage(lo, hi, size);
        if (c.a <= 1e-4) {
            outColour = vec4(0.0, 0.0, 0.0, 1.0);
            return;
        }
    } else {
        vec2 src = floor(mat2(p.col0.xy, p.col0.zw) * panel + p.offset.xy);
        c = texelFetch(raster, ivec2(src), 0);
    }
    if (p.offset.z > 0.5) {
        // A UNORM attachment: encode here. An _SRGB attachment does it in hardware and
        // this branch is off, which is `presenter-budget` W7 disappearing rather than
        // shrinking.
        vec3 lo = c.rgb * 12.92;
        vec3 hi = 1.055 * pow(max(c.rgb, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055;
        c.rgb = clamp(mix(hi, lo, step(c.rgb, vec3(0.0031308))), 0.0, 1.0);
    }
    outColour = vec4(c.rgb, 1.0);
}
