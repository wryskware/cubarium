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

// The air's density at `p`: light scattered toward the camera per voxel of path through
// fully lit air. `volK.x` at the floor, falling by e every `volK.z` voxels up.
float airDensity(vec3 p) {
    return u.volK.x * exp(-max(p.y, 0.0) / u.volK.z);
}

// At most this many samples along a ray, and never more than one a slab.
const int VOL_SAMPLES = 24;

// The light the air scatters toward this pixel (PX, PY) over the first `zEnd` slabs of
// its ray (the air in front of what the walk ended on), already hazed.
vec3 inScatter(int zEnd) {
    if (zEnd <= 0) { return vec3(0.0); }
    float k = float(RISE) / float(S);
    int n = min(VOL_SAMPLES, zEnd);
    float dz = float(zEnd) / float(n);
    // Voxels of path a sample stands for: the ray falls k voxels a slab.
    float ds = dz * sqrt(1.0 + k * k);
    float xw = (float(PX) + 0.5) / float(S);
    // The ray's height on the z = 0 plane.
    float y0 = (float(BASE - PY - 1) + 0.5) / float(S);
    vec3 acc = vec3(0.0);
    for (int i = 0; i < n; ++i) {
        float zs = (float(i) + 0.5) * dz;
        vec3 p = vec3(xw, y0 - zs * k, zs);
        float d = airDensity(p) * (1.0 - hazeAt(zs));
        vec3 light = u.lightC.rgb * sunInAir(p);
        if (GLOW && u.volK.y > 0.0) { light += u.volK.y * glowAt(p); }
        acc += d * light;
    }
    return acc * ds;
}
