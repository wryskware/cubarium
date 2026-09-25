// Light shafts (package V): the lit tier's open air scatters light toward the camera.
//
// Included by `voxel.frag` after everything it calls; compiled in only when the pipeline's
// `VOLUMETRIC` specialisation constant is on (`cubarium_gpu::sunvis::Effects`).
//
// The air in front of the slab walk's end scatters two lights toward the camera: the sun,
// in the palette's light colour, where the sun reaches the air (the sun-visibility volume,
// whose occluders are `sunReaches`' own, so the shafts line up with the cast shadows), and
// the emitters' glow (`glowAt`). How much depends on the air's density, thicker low down
// (`airDensity`). Each sample is hazed as a face at its depth is (its light times one less
// the haze there). The light only adds: the scene's own haze already stands for the air's
// extinction.
//
// The camera is orthographic, so every pixel's ray is (0, -RISE/S, 1) a slab; the samples
// sit at fixed places along it, the same every frame, so nothing shimmers: the only things
// that move the picture are the sun volume's fade and a new bake.
//
// Seams for fog (WX2): `airDensity` is the one place the air's thickness is decided, and
// `sunInAir` the one place the sun in the air is read. A height fog lit by the sun adds its
// density to `airDensity` and calls `sunInAir` for its light.

// The sun-visibility volume (`cubarium_gpu::sunvis`): one texel per voxel, red the bake
// fading out and green the bake fading in, sampled trilinearly (x wraps, y and z clamp).
layout(set = 0, binding = 10) uniform sampler3D sunVisTex;

// How much sun reaches the air at world point `p` (voxel units), 0 to 1: the two bakes
// mixed by the frame's fade.
float sunInAir(vec3 p) {
    vec2 v = textureLod(sunVisTex, p / vec3(float(W), float(H), float(D)), 0.0).rg;
    return mix(v.x, v.y, u.volK.w);
}

// The ground fog's density at `p` (WX2 checkpoint 2): its shape over the low ground
// (`fogShape`) at the pixel's drifting noise (`fogPixelNoise`, set once per ray by
// `inScatter`), zero when there is no fog.
float fogPixelNoise = 1.0;

float fogDensity(vec3 p) {
    if (u.fogK.x <= 0.0) { return 0.0; }
    return u.fogK.x * fogPixelNoise * fogShape(p.y);
}

// The air's density at `p`: light scattered toward the camera per voxel of path through
// fully lit air. `volK.x` at the floor, falling by e every `volK.z` voxels up, and the
// ground fog's on top.
float airDensity(vec3 p) {
    return u.volK.x * exp(-max(p.y, 0.0) / u.volK.z) + fogDensity(p);
}

// At most this many samples along a ray, and never more than one a slab.
const int VOL_SAMPLES = 24;

// The light the air scatters toward this pixel (PX, PY) over the first `zEnd` slabs of
// its ray (the air in front of what the walk ended on), already hazed, and (alpha) how much
// of what is behind that air gets through it: 1 but for the ground fog, which the ambient
// light and the sun in the air light, and which hides what is behind it. The fog here is
// the fog the weather pass draws without volumetric light, marched instead of integrated,
// so its sun is shadowed where the shafts are.
vec4 inScatter(int zEnd) {
    if (zEnd <= 0) { return vec4(0.0, 0.0, 0.0, 1.0); }
    float k = float(RISE) / float(S);
    int n = min(VOL_SAMPLES, zEnd);
    float dz = float(zEnd) / float(n);
    // Voxels of path a sample stands for: the ray falls k voxels a slab.
    float ds = dz * sqrt(1.0 + k * k);
    float xw = (float(PX) + 0.5) / float(S);
    // The ray's height on the z = 0 plane.
    float y0 = (float(BASE - PY - 1) + 0.5) / float(S);
    if (u.fogK.x > 0.0) { fogPixelNoise = fogNoiseFor(xw, y0, k, float(zEnd)); }
    vec3 acc = vec3(0.0);
    float through = 1.0;
    for (int i = 0; i < n; ++i) {
        float zs = (float(i) + 0.5) * dz;
        vec3 p = vec3(xw, y0 - zs * k, zs);
        float fog = fogDensity(p);
        float d = (airDensity(p) - fog) * (1.0 - hazeAt(zs));
        // The sun's own colour and strength from the day clock (WX2): at a clear noon
        // `sunLean` is `lightC` and `dayL.y` is 1, so this is the plain sun term; it warms at
        // twilight and falls to the moon's faint cool light at night. A lightning flash
        // lights the air too.
        vec3 sun = u.sunLean.rgb * (u.dayL.y * sunInAir(p));
        vec3 light = sun + 0.5 * u.flashC.rgb;
        vec3 glow = (GLOW && (u.volK.y > 0.0 || fog > 0.0)) ? glowAt(p) : vec3(0.0);
        light += u.volK.y * glow;
        vec3 here = d * light;
        if (fog > 0.0) {
            // The fog: the ambient on it, its share of the sun in the air, the emitters.
            here += fog * (u.fogC.rgb + u.rainC.w * sun + u.fogC.w * glow);
        }
        acc += through * here;
        through *= exp(-fog * ds);
    }
    return vec4(acc * ds, through);
}
