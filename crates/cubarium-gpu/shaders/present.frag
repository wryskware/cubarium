#version 450
// The final pass, and the only one that runs at panel resolution: a nearest-neighbour
// integer upscale of the world raster, plus the panel's 90-degree rotation, in one
// `texelFetch`. `xform` maps a panel pixel to a raster pixel; the Rust side builds it
// from the quarter-turn count and the integer factor k, so the shader has no idea the
// panel is portrait.
layout(set = 0, binding = 0) uniform sampler2D raster;

layout(push_constant) uniform Present {
    vec4 col0;   // xy = the matrix's first column, zw = its second
    vec4 offset; // xy = the translation, z = 1 to encode sRGB here, w = 1 to letterbox
} p;

layout(location = 0) out vec4 outColour;

void main() {
    vec2 panel = floor(gl_FragCoord.xy);
    vec2 src = floor(mat2(p.col0.xy, p.col0.zw) * panel + p.offset.xy);
    if (p.offset.w > 0.5) {
        // A desktop window (`WindowFit`): the picture is centred in whatever size the
        // window is, and a pixel that maps outside the raster is a black bar. The panel
        // never sets this; its transform always lands inside the raster.
        vec2 size = vec2(textureSize(raster, 0));
        if (any(lessThan(src, vec2(0.0))) || any(greaterThanEqual(src, size))) {
            outColour = vec4(0.0, 0.0, 0.0, 1.0);
            return;
        }
    }
    vec4 c = texelFetch(raster, ivec2(src), 0);
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
