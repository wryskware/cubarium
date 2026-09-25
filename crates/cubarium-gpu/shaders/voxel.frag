#version 450
// The voxel strip, drawn per pixel by a slab walk.
//
// `crates/cubarium/src/voxel/present.rs` is the definition of this picture and draws it
// on the CPU far to near, rectangle by rectangle. This walks the depth slabs **near to
// far** for one raster pixel and asks which voxel and which face owns it in each slab.
// The first opaque face ends the walk; water blends front to back on the way.
//
// The projection inversion is `crate::voxel::slab_hit`, which has the derivation. For
// slab z, writing q = base - z*rise - sy - 1, the pixel is in the band of voxel
// level = q/s at r = q - level*s rows up from that band's bottom, and two faces can own
// it: the front face of `level` (at face row s-1-r), and — only when r < rise — the top
// face of `level-1` (at cap row rise-1-r), because a cap occupies the bottom `rise` rows
// of the band above it. Within a slab the CPU paints y ascending, so front-to-back means
// the `level` faces first and the `level-1` cap second.
//
// Everything below that is the CPU presenter's colour rules, ported one for one. Two of
// its ownership tests are genuinely local functions of a cell and its nearer neighbour
// and are ported as well: the water body's `stop` row and `nearerOwns` for a water top.
// The rest of the CPU's bookkeeping — `front_hidden`, `top_hidden`, the y-descending
// clipping — is what walking front to back replaces.

layout(set = 0, binding = 0, std140) uniform VoxelScene {
    ivec4 geom;         // s, rise, base, roof from the table (1) or a column walk (0)
    ivec4 extent;       // width, height, depth, raster_h
    vec4 knobs;         // haze, water alpha, atmosphere, rain_tick
    vec4 skyC;          // the sky zenith, already at SKY_BRIGHTNESS
    vec4 skyHorizon;    // the sky horizon
    vec4 bedrockC;
    vec4 rockC;
    vec4 soilC;
    vec4 waterDeepC;
    vec4 waterSurfaceC;
    vec4 lightC;
    vec4 hazeC;
    vec4 shadeA;        // TOP_GAIN, TOP_TINT, TOP_BACK, RIM
    vec4 shadeB;        // EDGE_DARK, TOP_EDGE, RISER_LEAN, WET
    vec4 roofK;         // ROOF_LIGHT, ROOF_FALLOFF_VOXELS, -, -
    vec4 waterK;        // SKIN_ALPHA_GAIN, WATER_TOP_ALPHA, -, -
    vec4 plantA;        // PLANT_TOP_GAIN, PLANT_TOP_TINT, PLANT_RIM, CROWN_EDGE
    vec4 plantB;        // CROWN_UNDER, TRUNK_SHADE[0], TRUNK_SHADE[1], TRUNK_LIGHT_AT
    ivec4 tex;          // the face textures present, one bit per slot; vine tiles present; -, -
    vec4 lightK;        // lit: ambient gain, ambient floor, ladder rungs, AO strength
    vec4 ambientC;      // lit: the ambient light's colour, the sky's hue at unit luminance
    vec4 sunK;          // lit: the unit direction toward the sun (zero: none), sun tint
    vec4 waterL;        // lit: absorption per voxel of path, reflection gain, ripple tilt, reflection cells
    vec4 clock;         // sim time in ticks (tick + the fraction elapsed), the water's animation step, -, -
} u;

// The lighting tier (`lighting = "flat" | "lit"`), fixed when the pipeline is built. The
// flat tier's pipeline has it off: every `if (LIT)` below is dead code there, and what
// runs is the flat shader as it was. The lit tier replaces the flat tier's stand-ins for
// light (the column roof shade, and the TOP_* / PLANT_TOP_* gain-and-tint mixes) with a
// light value that multiplies each texel's base colour; the edge treatments are the
// flat tier's, leaning between the lit tones instead of the flat ones.
layout(constant_id = 0) const bool LIT = false;

layout(set = 0, binding = 1) uniform usampler3D voxels;  // rgba8ui, one texel per voxel
layout(set = 0, binding = 2) uniform usampler3D roofTex; // r8ui, voxels to the solid above
layout(set = 0, binding = 3) uniform sampler2D styleTex; // 4 x MAX_STYLES: wood, crown, heart, emit
layout(set = 0, binding = 4) uniform usampler2D glyphTex; // shared organism face texels
// The face textures at this px_per_voxel (`cubarium_gpu::voxel::VoxelTextures`): slot k,
// variant v is the S x S cell at (v*S, k*S), a top face in its first RISE rows. Texels
// are sRGB-encoded.
layout(set = 0, binding = 5) uniform sampler2D faceTex;
// The latticevine tiles at this px_per_voxel (`VoxelTextures::vine_rgba`): row
// 3*set + density (plain, climbing, hanging), column 16*mask + exits; row 9 the accents.
// Direct colour, sRGB.
layout(set = 0, binding = 6) uniform sampler2D vineTex;
// Lit tier only. The sky plane: at each open cell, the core's `sky_visibility` of the
// cell under it (the fan from the open cell's foot), 255 for open sky. Keyed on the
// terrain and computed off the loop thread.
layout(set = 0, binding = 7) uniform usampler3D skyTex;
// Lit tier only. The model's canopy per column: texels (x, 2z) and (x, 2z+1) hold four
// (cell, cumulative transmission) steps, highest cell first; a cell of 0 ends the list.
layout(set = 0, binding = 8) uniform usampler2D canopyTex;
// Lit tier only. The local light of the frame's emitters, one texel per 4x4x4 voxels
// (`GLOW_CELL`), in units of full ambient light: built on the CPU from the emitter list,
// spread a few cells through open terrain, and sampled trilinearly here.
layout(set = 0, binding = 9) uniform sampler3D glowTex;

layout(location = 0) out vec4 outColour;
// Lit tier only (the flat tier's pass has no attachment here, so the writes are dropped):
// the light this pixel emits, for the bloom (`cubarium_gpu::bloom`). An emitting texel's
// emissive colour, less the haze in front of it and times the water's transmission in
// front of it; zero for every pixel that is not an emitter.
layout(location = 1) out vec4 emitOut;

// The plant part classes, as `crate::voxel`'s PART_* constants number them.
const int TRUNK = 1;
const int CROWN = 2;
const int CROWN_HEART = 3;
const int SPROUT = 4;
// The fauna range of the same field. The interim glyph is a flat block in the style's
// `wood`.
const int ANIMAL_INTERIM = 5;
const int LOG = 6;
const int FLOOR_MARK = 7;

int S, RISE, BASE, W, H, D;

// --- the world, as the packing stores it ---------------------------------------------

int wrapX(int x) {
    int m = x % W;
    return m < 0 ? m + W : m;
}

bool inY(int y) { return y >= 0 && y < H; }

uvec4 at(int x, int y, int z) { return texelFetch(voxels, ivec3(wrapX(x), y, z), 0); }

int matOf(uvec4 v) { return int(v.r & 3u); }
int partOf(uvec4 v) { return int((v.r >> 2) & 7u); }
int glyphOf(uvec4 v) { return int(v.r >> 5); }
bool isBlockPart(int p) { return p == TRUNK || p == CROWN || p == CROWN_HEART || p == ANIMAL_INTERIM || p == LOG; }
bool solidV(uvec4 v) { return matOf(v) != 0; }

// Out-of-range y reads as air, which is what makes the top of the world an open sky and
// the autotiling work at y = 0 (`present::solid`).
bool solidAt(int x, int y, int z) { return inY(y) && solidV(at(x, y, z)); }

// The free-water byte: 0 is dry, and any water at all is at least 1 (see VoxelTexel).
int freeQ(int x, int y, int z) { return inY(y) ? int(at(x, y, z).g) : 0; }

// `present::fill_px`: rows of a front rectangle a fraction fills, at least one.
int fillPxQ(int q) {
    return q == 0 ? 0 : clamp(int(floor(float(q) / 255.0 * float(S) + 0.5)), 1, S);
}
int fillPxAt(int x, int y, int z) { return fillPxQ(freeQ(x, y, z)); }

// `present::water_open_up`, in drawn pixels and not in the raw fraction.
bool waterOpenUp(int x, int y, int z) {
    return fillPxAt(x, y, z) < S
        || (!solidAt(x, y + 1, z) && freeQ(x, y + 1, z) == 0);
}

int frontRow(int y, int z) { return BASE - (y + 1) * S - z * RISE; }

// --- face textures --------------------------------------------------------------------

// `TEXTURE_SLOTS`: terrain material m has its side at 2(m-1) and its top at 2(m-1)+1.
const int TEX_TURF_SIDE = 6;
// A style's texture role (`ROLE_*`, the alpha of its wood column): bark, leaf and drape
// have a side slot and a top slot from TEX_BARK_SIDE on; the accent is untextured.
const int ROLE_BARK = 1;
const int ROLE_DRAPE = 3;
const int TEX_BARK_SIDE = 7;

bool texOn(int slot) { return slot >= 0 && (u.tex.x & (1 << slot)) != 0; }

// Which of a face's four variants a voxel shows: a hash of its position, so a wall is not
// one tile repeating, and the same voxel shows the same variant every frame.
uint cellHash(int x, int y, int z, int salt) {
    uint h = uint(x) * 0x9E3779B1u ^ uint(y) * 0x85EBCA77u ^ uint(z) * 0xC2B2AE3Du
        ^ uint(salt) * 0x27D4EB2Fu;
    h ^= h >> 15;
    h *= 0x2C1B3C6Du;
    h ^= h >> 12;
    return h;
}

int texVariant(int x, int y, int z, int salt) {
    return int(cellHash(x, y, z, salt) & 3u);
}

// Face texel (dx, dy) of `slot`: dx across the face, dy down a side face from its top row
// or across a top face from its back edge. The level is px_per_voxel itself, so this is
// one texel per screen pixel and never filtered.
vec4 faceTexel(int slot, int x, int y, int z, int salt, int dx, int dy) {
    int v = texVariant(x, y, z, salt);
    return texelFetch(faceTex, ivec2(v * S + dx, slot * S + dy), 0);
}

vec3 srgbToLinear(vec3 c) {
    return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
}

// --- the latticevine tile layer (`cubarium::voxel::vine`) --------------------------------

// How a vine cell shows its face: flush on the rock behind it (a face looking at the
// camera), a curtain on its own front (an underside), or a quarter-width sliver against
// the wall on its left or right (a +x or -x face, seen edge-on).
const int VINE_FLUSH = 0;
const int VINE_CURTAIN = 1;
const int VINE_SLIVER_L = 2;
const int VINE_SLIVER_R = 3;

// An air voxel with no part and a nonzero glyph id carries a vine cell; the glyph id is
// its kind plus one.
bool isVine(uvec4 v) {
    return u.tex.y != 0 && (v.r & 31u) == 0u && (v.r >> 5) != 0u;
}

int vineKind(uvec4 v) { return int(v.r >> 5) - 1; }

// The cell's tile at face pixel (dx, dy), row 0 at the top, in linear light; alpha is the
// cutout. The host resolved which tile (`VineCell::tile`): row in a's low nibble, column
// 16*mask + exits from b. The accent (bud, flower, fruit) goes over it, jittered a little
// per cell.
vec4 vineTexel(uvec4 v, int x, int y, int z, int dx, int dy) {
    int row = int(v.a & 15u);
    int col = int(v.b & 15u) * 16 + int(v.b >> 4);
    vec4 t = texelFetch(vineTex, ivec2(col * S + dx, row * S + dy), 0);
    int accent = int((v.a >> 4) & 3u);
    if (accent != 0) {
        int j = S / 8;
        uint h = cellHash(x, y, z, 9);
        int ox = j > 0 ? int(h % uint(2 * j + 1)) - j : 0;
        int oy = j > 0 ? int((h >> 8) % uint(2 * j + 1)) - j : 0;
        int ax = dx - ox;
        int ay = dy - oy;
        if (ax >= 0 && ax < S && ay >= 0 && ay < S) {
            vec4 a = texelFetch(vineTex, ivec2((accent - 1) * S + ax, 9 * S + ay), 0);
            if (a.a >= 0.5) { t = a; }
        }
    }
    return vec4(srgbToLinear(t.rgb), t.a);
}

// --- colour ---------------------------------------------------------------------------

vec3 strataOf(int m) {
    if (m == 0) { return u.skyC.rgb; }
    if (m == 1) { return u.bedrockC.rgb; }
    if (m == 2) { return u.rockC.rgb; }
    return u.soilC.rgb;
}

// `Material::pore_capacity() > 0`: rock and soil hold pore water, air and bedrock do not.
bool holdsPore(int m) { return m == 2 || m == 3; }

float hazeAt(float zf) {
    if (D <= 1) { return 0.0; }
    return clamp(u.knobs.x, 0.0, 1.0) * clamp(zf / float(D - 1), 0.0, 1.0);
}

vec3 hazed(vec3 c, float t) { return mix(c, u.hazeC.rgb, clamp(t, 0.0, 1.0)); }

// `present::roof_shade`: how much of the top-face light a surface `gap` voxels below its
// roof keeps. 0 is an open column.
float roofShade(int gap) {
    if (gap == 0) { return 1.0; }
    float t = 1.0 - exp(-float(gap - 1) / u.roofK.y);
    return u.roofK.x + (1.0 - u.roofK.x) * t;
}

// `VoxelPresenter::build_roof`: voxels from this one up to the nearest solid above it in
// its own column, 0 where the column is open to the sky. Either read from the table the
// CPU built or walked here; the two give the same number.
int roofGap(int x, int y, int z) {
    if (u.geom.w != 0) { return int(texelFetch(roofTex, ivec3(wrapX(x), y, z), 0).r); }
    for (int yy = y + 1; yy < H; ++yy) {
        if (solidV(at(x, yy, z))) { return yy - y; }
    }
    return 0;
}

vec3 styleAt(int style, int part) { return texelFetch(styleTex, ivec2(part, style), 0).rgb; }

// A style's emissive colour is its fourth column (`VoxelStyle::with_emit`) in linear
// light, zero for none. A **whole-cell** emitter (`VoxelStyle::emit_whole`), whose every
// visible texel emits whatever its glyph's tones, carries a quarter in the fractional part
// of its three pigment columns' alphas, so the pigment fetch every texel makes already
// says so.
vec3 styleEmit(int style) { return texelFetch(styleTex, ivec2(3, style), 0).rgb; }
bool emitsWhole(float pigmentAlpha) { return fract(pigmentAlpha) > 0.125; }

// The walk's `trans` once it has ended on an emitter: the light the emitter sends to the
// camera through what is in front of it (`trans` times its colour less the haze), negated
// and less one, so that every channel is at most -1. Nothing else makes `trans` negative,
// and after the walk this is where the emission for the bloom is read from: carrying it
// in a variable of its own through the walk costs the slab walk's occupancy.
vec3 emitterTrans(vec3 trans, vec3 drawn, float haze) {
    return -(trans * max(drawn - clamp(haze, 0.0, 1.0) * u.hazeC.rgb, vec3(0.0))) - 1.0;
}

// The glyph atlas's emissive tone (`appearance::TONE_EMIT`): a texel that draws its
// style's emissive colour, unlit and at full value, in the lit tier. A style with no
// emissive colour, and the flat tier, draw it as a plain texel.
const int TONE_EMIT = 6;

// A vine tile texel the host flagged as emitting (`textures::flag_vine_emitters`): alpha
// 254 instead of 255. Drawn at its own colour, unlit, in the lit tier.
bool vineEmits(vec4 t) { return t.a >= 0.5 && t.a < 0.998; }

vec3 plantLit(vec3 c, float shade) {
    vec3 lit = mix(c * u.plantA.x, u.lightC.rgb, u.plantA.y);
    return shade < 1.0 ? mix(c, lit, shade) : lit;
}

// A solid face's colour before wetness and light: its texture's texel where the slot is
// present, the material's strata colour where it is not.
vec3 faceBase(int m, int slot, int x, int y, int z, int salt, int dx, int dy) {
    if (!texOn(slot)) { return strataOf(m); }
    return srgbToLinear(faceTexel(slot, x, y, z, salt, dx, dy).rgb);
}

vec3 blockBody(uvec4 v, vec3 base) {
    int m = matOf(v);
    float wet = holdsPore(m) ? clamp(float(v.b) / 255.0, 0.0, 1.0) : 0.0;
    return mix(base, u.waterDeepC.rgb, wet * u.shadeB.w);
}

vec3 blockLit(vec3 body, float shade) {
    vec3 lit = mix(body * u.shadeA.x, u.lightC.rgb, u.shadeA.y);
    return shade < 1.0 ? mix(body, lit, shade) : lit;
}

// --- light (the lit tier) ---------------------------------------------------------------
//
// Only two faces of a voxel are ever drawn: the front (normal -z) and the top (+y). A
// front face's open cell is (x, y, z-1) and a top face's is (x, y+1, z); its texel (dx, dy)
// lies at (x + (dx+1/2)/S, y+1 - (dy+1/2)/S, z) on a front face and at
// (x + (dx+1/2)/S, y+1, z+1 - (dy+1/2)/RISE) on a top face.
//
// The ambient is sky x AO x canopy, between the floor `lightK.y` and one, times the gain
// `lightK.x`, in the ambient colour: continuous by default (`lightK.z` 0, Wrysk: "lighting
// yes"), or snapped to a ladder of `lightK.z` rungs. The base colour it multiplies is never
// quantised.

// A cell that darkens the corners of a face beside it: terrain and block parts (trunk,
// crown, heart, log, animal). Sprouts, floor marks and vine cells do not. Outside the
// world in y or z is open.
float occluder(int x, int y, int z) {
    if (!inY(y) || z < 0 || z >= D) { return 0.0; }
    uvec4 v = at(x, y, z);
    return (solidV(v) || isBlockPart(partOf(v))) ? 1.0 : 0.0;
}

// The AO is a **crease line** (Wrysk, checkpoint 2): a face stays flat, and only a thin
// band along an edge whose side neighbour in the face's open plane occludes, and a small
// square in a corner where only the diagonal neighbour does, take the AO step: the light
// times 1 - strength, before the ladder snaps it (one rung down at the default 0.5 on a
// 4-rung ladder). The band is about S/8 wide, at least 1 px (1 px at 6 px a voxel, 2 at 13),
// on a top face at most half its rows.
int creaseWidth(int rows) { return min(max(1, (S + 4) / 8), max(1, rows / 2)); }

// The crease factor at texel (du, dv) of a face nu x nv texels, dv = 0 on its `hi` side,
// from its four side neighbours and four diagonals (lo/hi x left/right).
float crease(float l, float r, float lo, float hi,
             float cLL, float cRL, float cLH, float cRH, int du, int dv, int nu, int nv) {
    int b = creaseWidth(nv);
    bool inL = du < b, inR = du >= nu - b, inH = dv < b, inLo = dv >= nv - b;
    bool band = (inL && l > 0.5) || (inR && r > 0.5) || (inH && hi > 0.5) || (inLo && lo > 0.5)
        || (inL && inLo && cLL > 0.5) || (inR && inLo && cRL > 0.5)
        || (inL && inH && cLH > 0.5) || (inR && inH && cRH > 0.5);
    return band ? 1.0 - u.lightK.w : 1.0;
}

// AO of a front face of (x, y, z) at texel (dx, dy), dy = 0 at the top: the open plane is
// z-1.
float aoFront(int x, int y, int z, int dx, int dy) {
    int p = z - 1;
    return crease(
        occluder(x - 1, y, p), occluder(x + 1, y, p), occluder(x, y - 1, p), occluder(x, y + 1, p),
        occluder(x - 1, y - 1, p), occluder(x + 1, y - 1, p),
        occluder(x - 1, y + 1, p), occluder(x + 1, y + 1, p),
        dx, dy, S, S);
}

// AO of a top face of (x, y, z) at texel (dx, dy), dy = 0 at the back: the open plane is
// y+1; its near side (z-1) is the face's bottom rows.
float aoTop(int x, int y, int z, int dx, int dy) {
    int p = y + 1;
    return crease(
        occluder(x - 1, p, z), occluder(x + 1, p, z), occluder(x, p, z - 1), occluder(x, p, z + 1),
        occluder(x - 1, p, z - 1), occluder(x + 1, p, z - 1),
        occluder(x - 1, p, z + 1), occluder(x + 1, p, z + 1),
        dx, dy, S, RISE);
}

// The sky plane at open cell (x, y, z). Above the world, and in front of it (a front
// face at z = 0, whose fan leaves through the world's near side at once), is open sky.
float skyOpen(int x, int y, int z) {
    if (y >= H || z < 0) { return 1.0; }
    if (y < 0 || z >= D) { return 0.0; }
    return float(texelFetch(skyTex, ivec3(wrapX(x), y, z), 0).r) / 255.0;
}

// The model's canopy over column (x, z) for a face of a voxel at height yv: the product
// of every foliage layer whose disc sits in a cell above yv (`shade_layers_into`'s
// attenuation, straight down).
float canopyAt(int x, int z, int yv) {
    if (z < 0 || z >= D) { return 1.0; }
    uvec4 a = texelFetch(canopyTex, ivec2(wrapX(x), 2 * z), 0);
    if (int(a.r) <= yv) { return 1.0; }
    if (int(a.b) <= yv) { return float(a.g) / 255.0; }
    uvec4 b = texelFetch(canopyTex, ivec2(wrapX(x), 2 * z + 1), 0);
    if (int(b.r) <= yv) { return float(a.a) / 255.0; }
    if (int(b.b) <= yv) { return float(b.g) / 255.0; }
    return float(b.a) / 255.0;
}

// The model's canopy over a plant cell's own faces, from the packer: the foliage layers
// above it in its own column **but its own stand's**, since the model never lets a stand
// shade itself (v.b's low nibble; `Packer::fill`). An animal's cell carries no such byte
// and reads the column plane like the terrain.
float plantCanopy(uvec4 v) { return float(v.b & 15u) / 15.0; }

// A crown cell's chance of letting a shadow ray through, from the packer: the model's
// transmission down its column spread evenly over the column's crown cells (v.b's high
// nibble), so a ray straight down through all of them keeps what the model lets through.
float crownPass(uvec4 v) { return float(v.b >> 4) / 15.0; }

// What the sun adds to the ambient product when the light is smooth: the default 4-rung
// ladder's one rung, so a shadow darkens as much as it did (continuously, not by a rung).
const float SMOOTH_SUN = 1.0 / 3.0;

// The light: the ambient product, plus the sun's share where the sun reaches (so a shadow
// is a hard-edged texel step darker than the sunlit texel beside it, and never below the
// floor), times the gain, in the ambient colour. Smooth (`lightK.z` under 2), the product
// is used as it is and the sun adds SMOOTH_SUN; on a ladder of `lightK.z` rungs the product
// is snapped to its rung and the sun adds one rung.
//
// `local` is the emitters' light at the texel (`glowAt`), added to the ambient product
// before any snapping: its luminance raises the product, and the light's hue leans from
// the ambient colour toward the emitters' by their share of the sum (on a ladder, itself
// snapped to the ladder's steps). With no local light this is exactly the ambient term.
vec3 ladderLight(float a, float sun, vec3 local) {
    bool smoothLight = u.lightK.z < 1.5;
    float n = max(u.lightK.z - 1.0, 1.0);
    vec3 hue = u.ambientC.rgb;
    float l = dot(local, vec3(0.2126, 0.7152, 0.0722));
    if (l > 1.0 / 512.0) {
        float sum = clamp(a, 0.0, 1.0) + l;
        float share = smoothLight ? l / sum : floor(l / sum * n + 0.5) / n;
        hue = mix(hue, local / l, share);
        a = sum;
    }
    float t = smoothLight ? clamp(a, 0.0, 1.0) + sun * SMOOTH_SUN
                          : (floor(clamp(a, 0.0, 1.0) * n + 0.5) + sun) / n;
    return hue * (u.lightK.x * (u.lightK.y + (1.0 - u.lightK.y) * t));
}

// The emitters' light at world point `p` (voxel units), trilinear over the glow volume
// whose texel i covers voxels 4i .. 4i + 4.
vec3 glowAt(vec3 p) {
    vec3 n = vec3(textureSize(glowTex, 0)) * 4.0;
    // Explicit level: the walk is divergent per pixel, and an implicit-derivative sample
    // there costs as much as the rest of the lit shading together.
    return textureLod(glowTex, p / n, 0.0).rgb;
}

// --- the sun (the lit tier) --------------------------------------------------------------
//
// The sun term is binary per texel: a ray from the texel's world point toward the sun
// (`sunK.xyz`), walked cell by cell, either leaves the world (lit) or meets something in
// the way (one rung darker). Terrain and block parts (trunk, log, animal) stop it. A crown
// cell passes or blocks it **as a whole cell** (Wrysk, checkpoint 2): a hash of the cell
// under the cell's pass chance, so crown shade falls in whole-block spots with the model's
// transmission as its mean. Sprouts, floor marks, vine cells and water do not stop it.

// Cells a shadow ray crosses before it gives up and counts as lit.
const int SUN_MARCH = 128;

// Does a ray get through crown cell `c`? The same answer for every ray through the cell.
bool crownLets(uvec4 v, ivec3 c) {
    uint h = cellHash(wrapX(c.x), c.y, c.z, 32);
    return float(h & 0xFFFFu) / 65536.0 < crownPass(v);
}

// 1 where the sun reaches world point `p`, which lies on the boundary of open cell `c`
// (the cell in front of the face it is on); 0 where anything opaque is in the way.
float sunReaches(vec3 p, ivec3 c) {
    vec3 L = u.sunK.xyz;
    ivec3 dir = ivec3(sign(L));
    vec3 inv = vec3(
        abs(L.x) > 1e-6 ? 1.0 / abs(L.x) : 1e30,
        abs(L.y) > 1e-6 ? 1.0 / abs(L.y) : 1e30,
        abs(L.z) > 1e-6 ? 1.0 / abs(L.z) : 1e30);
    // How far along the ray each axis's next cell boundary is.
    vec3 next = vec3(
        (dir.x > 0 ? float(c.x + 1) - p.x : p.x - float(c.x)) * inv.x,
        (dir.y > 0 ? float(c.y + 1) - p.y : p.y - float(c.y)) * inv.y,
        (dir.z > 0 ? float(c.z + 1) - p.z : p.z - float(c.z)) * inv.z);
    for (int i = 0; i < SUN_MARCH; ++i) {
        if (c.y >= H || c.z < 0 || c.z >= D) { return 1.0; }
        if (c.y < 0) { return 0.0; }
        int axis = (next.x < next.y && next.x < next.z) ? 0 : (next.y < next.z ? 1 : 2);
        uvec4 v = at(c.x, c.y, c.z);
        if (solidV(v)) { return 0.0; }
        int part = partOf(v);
        if (part == TRUNK || part == LOG || part == ANIMAL_INTERIM) { return 0.0; }
        if ((part == CROWN || part == CROWN_HEART) && !crownLets(v, c)) {
            return 0.0;
        }
        if (axis == 0) { c.x += dir.x; next.x += inv.x; }
        else if (axis == 1) { c.y += dir.y; next.y += inv.y; }
        else { c.z += dir.z; next.z += inv.z; }
    }
    return 1.0;
}

// A lit colour: the base under the quantised light, and where the sun reaches, leaning
// toward the palette's light by the sun tint times N·L (the flat tier's lean of a top
// face toward `lightC`, now gated by the sun).
vec3 litBy(vec3 base, float ambient, float sun, float ndl, vec3 local) {
    vec3 c = base * ladderLight(ambient, sun, local);
    return mix(c, u.lightC.rgb, clamp(u.sunK.w * sun * ndl, 0.0, 1.0));
}

// The front face of (x, y, z) at texel (dx, dy), lit, with `canopy` over it.
vec3 shadeFront(vec3 base, int x, int y, int z, int dx, int dy, float canopy) {
    float ndl = max(-u.sunK.z, 0.0);
    float sun = 0.0;
    vec3 p = vec3(float(x) + (float(dx) + 0.5) / float(S),
                  float(y + 1) - (float(dy) + 0.5) / float(S), float(z));
    if (ndl > 0.0 && u.sunK.y > 0.0) {
        sun = sunReaches(p, ivec3(x, y, z - 1));
    }
    // The local light half a voxel into the open cell in front of the face.
    vec3 local = glowAt(p - vec3(0.0, 0.0, 0.5));
    return litBy(base, skyOpen(x, y, z - 1) * aoFront(x, y, z, dx, dy) * canopy, sun, ndl,
                 local);
}

// The top face of (x, y, z) at texel (dx, dy), dy = 0 at the back, lit, with `canopy`
// over it.
vec3 shadeTop(vec3 base, int x, int y, int z, int dx, int dy, float canopy) {
    float ndl = max(u.sunK.y, 0.0);
    float sun = 0.0;
    vec3 p = vec3(float(x) + (float(dx) + 0.5) / float(S), float(y + 1),
                  float(z + 1) - (float(dy) + 0.5) / float(RISE));
    if (ndl > 0.0) {
        sun = sunReaches(p, ivec3(x, y + 1, z));
    }
    // The local light half a voxel into the open cell above the face.
    vec3 local = glowAt(p + vec3(0.0, 0.5, 0.0));
    return litBy(base, skyOpen(x, y + 1, z) * aoTop(x, y, z, dx, dy) * canopy, sun, ndl, local);
}

// --- the faces ------------------------------------------------------------------------

// Micro-dithering grain across solid voxel faces to break up flat surfaces
float faceGrain(int x, int y, int z, int dx, int dy) {
    uint h = uint(x * 73856093 ^ y * 19349663 ^ z * 83492791 ^ dx * 2654435761u ^ dy * 38291);
    h = (h ^ (h >> 13)) * 1274126177u;
    return float(int(h & 15u) - 7) * 0.005;
}

// The lit tier's front rectangle: `blockFront`'s rules with its two tones lit. The face's
// own tone is the base under this face's light; where the flat tier leans toward the top
// face's light (the rim row, the chamfer, the riser's head) this leans toward the base
// under the top face's light at its front edge.
vec3 blockFrontLit(int x, int y, int z, uvec4 v, int r, int dx) {
    int dy = S - 1 - r;
    bool openUp = !solidAt(x, y + 1, z);
    bool openLeft = !solidAt(x - 1, y, z);
    bool openRight = !solidAt(x + 1, y, z);
    bool riser = openUp && z > 0 && !solidAt(x, y, z - 1) && solidAt(x, y - 1, z - 1);
    bool onSide = (dx == 0 && openLeft) || (dx + 1 == S && openRight);

    int m = matOf(v);
    int slot = (m == 3 && openUp && texOn(TEX_TURF_SIDE)) ? TEX_TURF_SIDE : 2 * (m - 1);
    vec3 body = blockBody(v, faceBase(m, slot, x, y, z, 0, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy) * (u.roofK.z / 0.04));
    }
    vec3 own = shadeFront(body, x, y, z, dx, dy, canopyAt(x, z - 1, y));
    if (riser) {
        vec3 lit = shadeTop(body, x, y, z, dx, RISE - 1, canopyAt(x, z, y));
        float t = S > 1 ? float(dy) / float(S - 1) : 0.0;
        vec3 slope = mix(lit, own, u.shadeB.z * t);
        float hz = hazeAt(float(z) - t);
        return hazed(onSide ? slope * u.shadeB.x : slope, hz);
    }
    float haze = hazeAt(float(z));
    vec3 c;
    if (dy == 0 && openUp) {
        vec3 lit = shadeTop(body, x, y, z, dx, RISE - 1, canopyAt(x, z, y));
        c = onSide ? lit : mix(own, lit, u.shadeA.w);
    } else {
        c = onSide ? own * u.shadeB.x : own;
    }
    return hazed(c, haze);
}

// `VoxelPresenter::block`'s front rectangle: rim row, side bevel, chamfered corner, or
// — where the ground steps one voxel into depth — the riser's lean with no rim at all.
vec3 blockFront(int x, int y, int z, uvec4 v, int r, int dx) {
    if (LIT) { return blockFrontLit(x, y, z, v, r, dx); }
    int dy = S - 1 - r;
    bool openUp = !solidAt(x, y + 1, z);
    bool openLeft = !solidAt(x - 1, y, z);
    bool openRight = !solidAt(x + 1, y, z);
    bool riser = openUp && z > 0 && !solidAt(x, y, z - 1) && solidAt(x, y - 1, z - 1);
    bool onSide = (dx == 0 && openLeft) || (dx + 1 == S && openRight);

    int m = matOf(v);
    // Soil under open sky wears the turf: the same soil, with a fringe over its top rows.
    int slot = (m == 3 && openUp && texOn(TEX_TURF_SIDE)) ? TEX_TURF_SIDE : 2 * (m - 1);
    vec3 body = blockBody(v, faceBase(m, slot, x, y, z, 0, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy) * (u.roofK.z / 0.04));
    }
    vec3 lit = blockLit(body, roofShade(roofGap(x, y, z)));
    if (riser) {
        float t = S > 1 ? float(dy) / float(S - 1) : 0.0;
        vec3 slope = mix(lit, body, u.shadeB.z * t);
        float hz = hazeAt(float(z) - t);
        return hazed(onSide ? slope * u.shadeB.x : slope, hz);
    }
    float haze = hazeAt(float(z));
    bool onCap = dy == 0 && openUp;
    vec3 c;
    if (onCap) {
        c = onSide ? lit : mix(body, lit, u.shadeA.w);   // the chamfer, then the rim
    } else {
        c = onSide ? body * u.shadeB.x : body;
    }
    return hazed(c, haze);
}

// The lit tier's top rectangle: `blockTop`'s rules with its two tones lit. The face's own
// tone is the base under this face's light; where the flat tier leans back toward the
// unlit body (the contour row, the bevel at a drop) this leans toward the own tone over
// TOP_GAIN, the flat tier's own ratio of a top to the front it caps, so both keep their
// flat-tier contrast at every rung of the ladder.
vec3 blockTopLit(int x, int y, int z, uvec4 v, int r, int dx) {
    int dy = RISE - 1 - r;
    bool openLeft = !solidAt(x - 1, y, z);
    bool openRight = !solidAt(x + 1, y, z);
    bool dropLeft = openLeft && !solidAt(x - 1, y - 1, z);
    bool dropRight = openRight && !solidAt(x + 1, y - 1, z);
    bool backContinues = z + 1 < D && solidAt(x, y, z + 1) && !solidAt(x, y + 1, z + 1);

    int m = matOf(v);
    vec3 body = blockBody(v, faceBase(m, 2 * (m - 1) + 1, x, y, z, 1, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy + 100) * (u.roofK.z / 0.04));
    }
    vec3 own = shadeTop(body, x, y, z, dx, dy, canopyAt(x, z, y));
    vec3 dark = own / u.shadeA.x;
    float hz = hazeAt(float(z) + float(r) / float(RISE));
    vec3 plane = (dy == 0 && RISE > 2 && !backContinues) ? mix(own, dark, u.shadeA.z) : own;
    bool onDrop = (dx == 0 && dropLeft) || (dx + 1 == S && dropRight);
    return hazed(onDrop ? mix(plane, dark, u.shadeB.y) : plane, hz);
}

// `VoxelPresenter::block`'s top rectangle: the contour row only where the ground really
// ends going back, and a bevel only at a real drop.
vec3 blockTop(int x, int y, int z, uvec4 v, int r, int dx) {
    if (LIT) { return blockTopLit(x, y, z, v, r, dx); }
    int dy = RISE - 1 - r;
    bool openLeft = !solidAt(x - 1, y, z);
    bool openRight = !solidAt(x + 1, y, z);
    bool dropLeft = openLeft && !solidAt(x - 1, y - 1, z);
    bool dropRight = openRight && !solidAt(x + 1, y - 1, z);
    bool backContinues = z + 1 < D && solidAt(x, y, z + 1) && !solidAt(x, y + 1, z + 1);

    int m = matOf(v);
    vec3 body = blockBody(v, faceBase(m, 2 * (m - 1) + 1, x, y, z, 1, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy + 100) * (u.roofK.z / 0.04));
    }
    vec3 lit = blockLit(body, roofShade(roofGap(x, y, z)));
    float hz = hazeAt(float(z) + float(r) / float(RISE));
    vec3 plane = (dy == 0 && RISE > 2 && !backContinues) ? mix(lit, body, u.shadeA.z) : lit;
    bool onDrop = (dx == 0 && dropLeft) || (dx + 1 == S && dropRight);
    return hazed(onDrop ? mix(plane, body, u.shadeB.y) : plane, hz);
}

vec3 glyphPigment(uvec4 v, uint q) {
    return styleAt(int(v.a), int(q & 3u));
}

// The texture slot a style's cells draw with on this face, or -1: only a baked model's
// trunk, foliage and drape have one.
int roleSlot(uvec4 v, bool top) {
    int role = int(texelFetch(styleTex, ivec2(0, int(v.a)), 0).a + 0.5);
    if (role < ROLE_BARK || role > ROLE_DRAPE) { return -1; }
    return TEX_BARK_SIDE + 2 * (role - ROLE_BARK) + (top ? 1 : 0);
}

// A style's species face slot on this face (`VoxelStyle::with_faces`: the alpha of its
// crown column for the side, of its heart column for the top, slot + 1), or -1.
int speciesSlot(uvec4 v, bool top) {
    return int(texelFetch(styleTex, ivec2(top ? 2 : 1, int(v.a)), 0).a + 0.5) - 1;
}

// A model cell's texture over its pigment. A species face (`species/<species>/`) is
// direct colour and replaces the pigment; a generic one's `r / 128` multiplies the style
// colour, so the colour pass still decides the hue. False where a leaf or drape cutout
// has a hole: that texel is not this cell's, and the walk goes on to whatever is behind
// it.
bool plantTexel(int x, int y, int z, uvec4 v, bool top, int dx, int dy, inout vec3 base) {
    int own = speciesSlot(v, top);
    if (own >= 0) {
        vec4 t = faceTexel(own, x, y, z, top ? 1 : 0, dx, dy);
        int role = int(texelFetch(styleTex, ivec2(0, int(v.a)), 0).a + 0.5);
        if (role >= ROLE_BARK + 1 && role <= ROLE_DRAPE && t.a < 0.5) { return false; }
        base = srgbToLinear(t.rgb);
        return true;
    }
    int slot = roleSlot(v, top);
    if (!texOn(slot)) { return true; }
    vec4 t = faceTexel(slot, x, y, z, top ? 1 : 0, dx, dy);
    if (slot > TEX_BARK_SIDE + 1 && t.a < 0.5) { return false; }
    base *= t.r * (255.0 / 128.0);
    return true;
}

// Organism anatomy and markings are already resolved in glyphTex by the shared
// appearance layer. This is deliberately generic: the shader knows only pigment slots
// and treatments, never bodies, heads, eyes or facing.
bool glyphFront(int x, int y, int z, uvec4 v, int r, int dx, out vec3 rgb, out bool lum) {
    lum = false;
    int localY = S - 1 - r;
    int atlasY = (partOf(v) * 8 + glyphOf(v)) * (S + RISE) + localY;
    uint q = texelFetch(glyphTex, ivec2(dx, atlasY), 0).r;
    vec4 pigment = texelFetch(styleTex, ivec2(int(q & 3u), int(v.a)), 0);
    vec3 base = pigment.rgb;
    int tone = int(q >> 2);
    if (tone == 63) { return false; }
    if (LIT && (tone == TONE_EMIT || emitsWhole(pigment.a))) {
        // An emitter: its style's emissive colour, unshadowed and at full value.
        vec3 e = styleEmit(int(v.a));
        if (any(greaterThan(e, vec3(0.0)))) {
            rgb = hazed(e, hazeAt(float(z)));
            lum = true;
            return true;
        }
    }
    if (!plantTexel(x, y, z, v, false, dx, localY, base)) { return false; }
    bool coveredUp = solidAt(x, y + 1, z)
        || (inY(y + 1) && isBlockPart(partOf(at(x, y + 1, z))));
    if (LIT) {
        // The atlas's tones on the lit base: the rim and the lit column lean toward the
        // base under the cell's top light, every other tone keeps its pigment rule under
        // this face's light.
        // A plant cell's own stand does not shade it; an animal reads the column.
        bool animal = partOf(v) == ANIMAL_INTERIM;
        float cf = animal ? canopyAt(x, z - 1, y) : plantCanopy(v);
        float ct = animal ? canopyAt(x, z, y) : plantCanopy(v);
        vec3 c;
        if (tone == 2 && !coveredUp) {
            c = mix(shadeFront(base, x, y, z, dx, localY, cf),
                    shadeTop(base, x, y, z, dx, RISE - 1, ct), u.plantA.z);
        } else if (tone == 3) {
            c = shadeTop(base, x, y, z, dx, RISE - 1, ct);
        } else {
            if (tone == 1) {
                base *= u.shadeB.x;
            } else if (tone == 4 || tone == 5) {
                base = mix(base, styleAt(int(v.a), 0), u.plantB.x);
                if (tone == 5) { base *= u.plantA.w; }
            } else if (tone >= 16 && tone <= 31) {
                base *= 0.5 + float(tone & 15) / 16.0;
            }
            c = shadeFront(base, x, y, z, dx, localY, cf);
        }
        rgb = hazed(c, hazeAt(float(z)));
        return true;
    }
    if (tone == 1) {
        base *= u.shadeB.x;
    } else if (tone == 2 && !coveredUp) {
        base = mix(base, plantLit(base, roofShade(roofGap(x, y, z))), u.plantA.z);
    } else if (tone == 3) {
        base = plantLit(base, roofShade(roofGap(x, y, z)));
    } else if (tone == 4 || tone == 5) {
        base = mix(base, styleAt(int(v.a), 0), u.plantB.x);
        if (tone == 5) { base *= u.plantA.w; }
    } else if (tone >= 16 && tone <= 31) {
        base *= 0.5 + float(tone & 15) / 16.0;
    }
    rgb = hazed(base, hazeAt(float(z)));
    return true;
}

bool glyphCap(int x, int y, int z, uvec4 v, int r, int dx, out vec3 rgb, out bool lum) {
    lum = false;
    int localY = RISE - 1 - r;
    int atlasY = (partOf(v) * 8 + glyphOf(v)) * (S + RISE) + S + localY;
    uint q = texelFetch(glyphTex, ivec2(dx, atlasY), 0).r;
    vec4 pigment = texelFetch(styleTex, ivec2(int(q & 3u), int(v.a)), 0);
    vec3 base = pigment.rgb;
    int tone = int(q >> 2);
    if (LIT && (tone == TONE_EMIT || emitsWhole(pigment.a))) {
        vec3 e = styleEmit(int(v.a));
        if (any(greaterThan(e, vec3(0.0)))) {
            rgb = hazed(e, hazeAt(float(z) + float(r) / float(RISE)));
            lum = true;
            return true;
        }
    }
    if (!plantTexel(x, y, z, v, true, dx, localY, base)) { return false; }
    if (LIT) {
        // The cap's own tone is the base under its light; the edge tones lean toward it
        // over PLANT_TOP_GAIN, as the flat tier leans toward the unlit base.
        float ct = partOf(v) == ANIMAL_INTERIM ? canopyAt(x, z, y) : plantCanopy(v);
        vec3 own = shadeTop(base, x, y, z, dx, localY, ct);
        vec3 lc = own;
        if (tone == 1) {
            lc *= u.plantA.w;
        } else if (tone >= 32 && tone <= 47) {
            float gain = 0.5 + float(tone & 15) / 16.0;
            lc = mix(lc, own / u.plantA.x * gain, u.shadeB.y);
        }
        rgb = hazed(lc, hazeAt(float(z) + float(r) / float(RISE)));
        return true;
    }
    float shade = roofShade(roofGap(x, y, z));
    vec3 cap = plantLit(base, shade);
    if (tone == 1) {
        cap *= u.plantA.w;
    } else if (tone >= 32 && tone <= 47) {
        float gain = 0.5 + float(tone & 15) / 16.0;
        cap = mix(cap, base * gain, u.shadeB.y);
    }
    rgb = hazed(cap, hazeAt(float(z) + float(r) / float(RISE)));
    return true;
}

// --- water ----------------------------------------------------------------------------

// `VoxelPresenter::nearer_owns`: does the slab one step nearer already own this screen
// row, in the pixel columns of voxel (x, y, z)? The bands are the unclipped ones on
// purpose — where the nearer slab's own face is itself culled, whatever culls it is
// nearer still.
bool nearerOwns(int x, int y, int z, int row) {
    if (z == 0) { return false; }
    int R = frontRow(y, z);
    for (int dy = 0; dy < 3; ++dy) {
        int yn = y + dy;
        int f = R + RISE - dy * S;
        int top;
        if (solidAt(x, yn, z - 1)) {
            top = f - RISE;
        } else {
            int fill = fillPxAt(x, yn, z - 1);
            if (fill == 0) { continue; }
            int skin = f + S - fill;
            top = waterOpenUp(x, yn, z - 1) ? skin - RISE : skin;
        }
        if (row >= top && row < f + S) { return true; }
    }
    return false;
}

// --- water, the lit tier -------------------------------------------------------------------
//
// The boundary is the flat tier's, exactly: `waterAtLit` runs the same row tests
// (`fillPxQ`, the skin rows, `nearerOwns`, `stop`), and a pixel is water in the lit tier
// if and only if one of them marks it. Only the colour inside changes:
//
// - Depth absorption: from the slab that first marks the pixel on, the walk's ray is
//   followed through the water it really crosses, cell by cell (the part of each slab's
//   segment, √(1 + (RISE/S)²) voxels long, under the cell's water line), and each piece
//   absorbs per channel by Beer–Lambert with σ = `absorb` × −ln(deep colour): after
//   1 / `absorb` voxels of water what is left of the light behind is the palette's deep
//   colour itself. What the water scatters in runs from the palette's surface colour at
//   the surface to its deep colour at depth under it (the same `absorb` sets that ramp,
//   over the view ray's path to the depth). The
//   in-scatter is lit by the ladder at the water's entry, as a ratio to open sunlit sky,
//   so open water keeps the palette's colours and water under cover darkens with it.
// - The surface (a pixel whose first mark is a skin top-face row): an animated, quantised
//   normal and a Fresnel mix with a reflection marched through the volume.
// - Falling water (a water cell over a cell that is neither solid nor full) has no ripple
//   and no reflection, but streaks that scroll down.
//
// Everything that needs more than a few texel fetches (the light, the flow, the normal,
// the reflection, the streaks) runs once per pixel after the walk, from what the entry
// recorded: front to back is linear, so the surface can be put in front afterwards.
// Nothing reaches the pixel before its first water (every other contribution ends the
// walk), so in front of the entry the walk has gathered nothing and let everything
// through. **The walk carries as little as it can**: one int for the entry, from which
// everything else about it is recomputed afterwards (state carried through the loop
// costs every pixel occupancy, water or not: +1.7 ms at 13 px with the entry point, its
// open cell and its canopy carried).

int wEntry;         // the first water: y | z << 12 | top-row << 24 | flat << 25 | falling << 26,
                    // or -1 for none
vec3 wScatter;      // in-scatter so far, unlit and unhazed, weighted by transmission
float wHaze;        // the haze share of that in-scatter (its weight, averaged over the channels)
float wSurf;        // the height of the entered water's surface
int PX;             // this pixel's raster column
int PY;             // and row

void waterLitReset() {
    wEntry = -1;
    wScatter = vec3(0.0);
    wHaze = 0.0;
    wSurf = 0.0;
}

// Per-channel absorption per voxel of water path.
vec3 waterSigma() { return -log(max(u.waterDeepC.rgb, vec3(1.0 / 4096.0))) * u.waterL.x; }

// Entry flags: the pixel's water is drawn by the flat tier's rule (`waterAt`), and its
// first water is falling.
const int ENTRY_FLAT = 1 << 25;
const int ENTRY_FALLING = 1 << 26;

// Whether the water in cell (x, y, z) is falling: the cell under it is neither solid nor
// full (in drawn pixels). `flowAt` reads the same.
bool fallingAt(int x, int y, int z) {
    return y > 0 && !solidAt(x, y - 1, z) && fillPxAt(x, y - 1, z) < S;
}

// The lit water for one screen row. A pixel whose first water is the lake's cut face at
// the world's front edge, or falling water, takes the flat tier's rule for all of its
// water (`waterAt`'s colours and opacities, off the ladder), carried in the lit tier's
// own accumulators: running the flat code itself in the walk costs +0.8 ms at 13 px in
// occupancy.
void waterAtLit(int x, int y, int z, uvec4 v, int row, inout vec3 acc, inout vec3 trans) {
    int fill = fillPxQ(int(v.g));
    int R = frontRow(y, z);
    int bottom = R + S;
    int skinRow = bottom - fill;
    bool openUp = waterOpenUp(x, y, z);
    bool topRow = openUp && row >= skinRow - RISE && row < skinRow && !nearerOwns(x, y, z, row);
    bool nearSolid = z > 0 && solidAt(x, y, z - 1);
    int nearFill = z > 0 ? fillPxAt(x, y, z - 1) : 0;
    int nearSkin = R + RISE + S - nearFill;
    int stop = bottom;
    if (nearSolid) {
        stop = skinRow;
    } else if (nearFill > 0) {
        bool nearOpenUp = waterOpenUp(x, y, z - 1);
        stop = min(bottom, nearOpenUp ? nearSkin - RISE : nearSkin);
    }
    bool bodyRow = row >= skinRow && row < stop;

    float k = float(RISE) / float(S);
    // The pixel's height on this slab's front plane; the ray falls k voxels a slab.
    float yf = (float(BASE - z * RISE - row - 1) + 0.5) / float(S);
    float h = hazeAt(float(z) + 0.5);
    if (wEntry < 0) {
        if (!topRow && !bodyRow) { return; }
        wEntry = y | (z << 12) | (topRow ? 1 << 24 : 0);
        // The lake's cut face at the world's front edge keeps the flat tier's deep indigo,
        // and falling water its bright surface colour: both by the flat rule, off the
        // ladder (the finish adds the fall's streaks).
        bool falling = fallingAt(x, y, z);
        if ((z == 0 && !topRow) || falling) {
            wEntry |= ENTRY_FLAT | (falling ? ENTRY_FALLING : 0);
        } else {
            // The surface over the entry: its own top face, or up the column a front face
            // belongs to.
            int ys = y;
            for (int i = 0; i < 8 && !topRow; ++i) {
                if (solidAt(x, ys + 1, z) || freeQ(x, ys + 1, z) == 0) { break; }
                ++ys;
            }
            wSurf = float(ys) + float(topRow ? fill : fillPxAt(x, ys, z)) / float(S);
            if (bodyRow && row == skinRow && openUp) {
                // The front's top row: the flat tier's skin line, in the surface colour.
                float a = min(clamp(u.knobs.y, 0.0, 1.0) * u.waterK.x, 0.95);
                wScatter += trans * a * (1.0 - h) * u.waterSurfaceC.rgb;
                wHaze += dot(trans, vec3(a * h / 3.0));
                trans *= 1.0 - a;
            }
        }
    }
    if ((wEntry & ENTRY_FLAT) != 0) {
        float alpha = clamp(u.knobs.y, 0.0, 1.0);
        float skinAlpha = min(alpha * u.waterK.x, 0.95);
        float a;
        vec3 c;
        float hf;
        if (topRow) {
            a = skinAlpha * u.waterK.y;
            c = u.waterSurfaceC.rgb;
            hf = hazeAt(float(z) + float(RISE - 1 - (row - (skinRow - RISE))) / float(RISE));
        } else if (bodyRow) {
            bool isSkin = row == skinRow && openUp;
            a = isSkin ? skinAlpha : alpha;
            c = isSkin ? u.waterSurfaceC.rgb : u.waterDeepC.rgb;
            hf = hazeAt(float(z));
        } else {
            return;
        }
        wScatter += trans * a * (1.0 - hf) * c;
        wHaze += dot(trans, vec3(a * hf / 3.0));
        trans *= 1.0 - a;
        return;
    }
    // The ray's piece in this cell's water, in this slab.
    float top = float(y) + float(fill) / float(S);
    float hi = min(yf, top);
    float lo = max(yf - k, float(y));
    float l = max(hi - lo, 0.0) / k * sqrt(1.0 + k * k);
    if (l <= 0.0) { return; }
    vec3 T = exp(-waterSigma() * l);
    // The in-scatter ramp runs on depth under the surface, as the view ray's path to
    // that depth (l / (hi - lo) voxels of path a voxel of depth).
    float depth = max(wSurf - 0.5 * (hi + lo), 0.0) * sqrt(1.0 + k * k) / k;
    vec3 cin = mix(u.waterSurfaceC.rgb, u.waterDeepC.rgb, 1.0 - exp(-u.waterL.x * depth));
    wScatter += trans * (1.0 - T) * (1.0 - h) * cin;
    wHaze += dot(trans * (1.0 - T), vec3(h / 3.0));
    trans *= T;
}

// `VoxelPresenter::water`, for one screen row: the surface's own receding top face and
// the body below it, each blended at most once.
void waterAt(int x, int y, int z, uvec4 v, int row, inout vec3 acc, inout vec3 trans) {
    if (LIT) { waterAtLit(x, y, z, v, row, acc, trans); return; }
    int fill = fillPxQ(int(v.g));
    int R = frontRow(y, z);
    int bottom = R + S;
    int skinRow = bottom - fill;
    float alpha = clamp(u.knobs.y, 0.0, 1.0);
    float skinAlpha = min(alpha * u.waterK.x, 0.95);
    bool openUp = waterOpenUp(x, y, z);

    if (openUp && row >= skinRow - RISE && row < skinRow && !nearerOwns(x, y, z, row)) {
        int dy = row - (skinRow - RISE);
        vec3 c = hazed(u.waterSurfaceC.rgb, hazeAt(float(z) + float(RISE - 1 - dy) / float(RISE)));
        float a = skinAlpha * u.waterK.y;
        acc += trans * a * c;
        trans *= 1.0 - a;
    }

    bool nearSolid = z > 0 && solidAt(x, y, z - 1);
    int nearFill = z > 0 ? fillPxAt(x, y, z - 1) : 0;
    int nearSkin = R + RISE + S - nearFill;
    int stop = bottom;
    if (nearSolid) {
        stop = skinRow;
    } else if (nearFill > 0) {
        bool nearOpenUp = waterOpenUp(x, y, z - 1);
        stop = min(bottom, nearOpenUp ? nearSkin - RISE : nearSkin);
    }
    if (row >= skinRow && row < stop) {
        bool isSkin = row == skinRow && openUp;
        float haze = hazeAt(float(z));
        vec3 c = hazed(isSkin ? u.waterSurfaceC.rgb : u.waterDeepC.rgb, haze);
        float a = isSkin ? skinAlpha : alpha;
        acc += trans * a * c;
        trans *= 1.0 - a;
    }
}

// --- the walk -------------------------------------------------------------------------

// Vertical sky gradient from zenith down to horizon
vec3 skyAt(int py) {
    if (u.roofK.w > 0.5) {
        float t = clamp(float(py) / float(max(u.extent.w, 1)), 0.0, 1.0);
        return mix(u.skyC.rgb, u.skyHorizon.rgb, t);
    }
    return u.skyC.rgb;
}

// Atmospheric sky with drifting moisture cloud wisps
vec3 skyColor(int px_x, int px_y) {
    vec3 sky = skyAt(px_y);
    float moisture = clamp(u.knobs.z, 0.0, 1.0);
    int mistRows = min(u.extent.w / 3, 56);
    if (u.roofK.w > 0.5 && moisture > 0.0 && px_y < mistRows) {
        float verticalT = 1.0 - (float(px_y) / float(mistRows));
        float rowFactor = verticalT * verticalT * moisture;
        int driftX = int(u.knobs.w) / 2;
        float wave1 = sin(float(px_x + driftX + px_y * 4) * 0.045) * 0.5 + 0.5;
        float wave2 = cos(float(px_x * 2 - driftX + 37) * 0.025) * 0.5 + 0.5;
        float density = wave1 * wave2 * rowFactor;
        if (density > 0.10) {
            float a = min((density - 0.10) * 1.8, 0.70);
            vec3 cloudC = vec3(0.038, 0.020, 0.102);
            vec3 cloudEdge = vec3(0.113, 0.171, 0.354);
            vec3 col = mix(cloudC, cloudEdge, wave1 * 0.5);
            sky = mix(sky, col, a);
        }
    }
    return sky;
}

// --- water after the walk (the lit tier) --------------------------------------------------

// The flow field, derived from the water state alone (the solver hands the renderer no
// velocity): lateral flow runs down the gradient of the free-surface height y + free
// against the four lateral neighbours' surfaces, and a water cell over a cell that is
// neither solid nor full is falling. Computed here, per water pixel, from the free bytes
// the walk already reads: at most 17 texel fetches, once per pixel.

// A neighbour that is a wall (solid, or outside the world in z): no flow across it.
const float WALL = -1.0e6;
// The surface drop per voxel that is full speed, and the steps the speed is quantised to
// (0, 1/3, 2/3, 1: a drop under a sixth of FLOW_FULL per voxel is still water).
const float FLOW_FULL = 0.5;
const float FLOW_STEPS = 3.0;

// The free surface near level y in column (x, z): the water over it, in it, or the water
// or ground at most two cells under it.
float surfaceNear(int x, int y, int z) {
    if (z < 0 || z >= D || solidAt(x, y, z)) { return WALL; }
    int ga = solidAt(x, y + 1, z) ? 0 : freeQ(x, y + 1, z);
    if (ga > 0) { return float(y + 1) + float(ga) / 255.0; }
    int g = freeQ(x, y, z);
    if (g > 0) { return float(y) + float(g) / 255.0; }
    for (int yy = y - 1; yy >= max(y - 2, 0); --yy) {
        if (solidAt(x, yy, z)) { return float(yy + 1); }
        int gg = freeQ(x, yy, z);
        if (gg > 0) { return float(yy) + float(gg) / 255.0; }
    }
    return float(max(y - 2, 0));
}

// The surface's slope along one axis from its two neighbours: central where both are
// water or open, one-sided against a wall, flat between two walls.
float slope1(float a, float h0, float b) {
    bool wa = a < WALL * 0.5;
    bool wb = b < WALL * 0.5;
    if (wa && wb) { return 0.0; }
    if (wa) { return b - h0; }
    if (wb) { return h0 - a; }
    return 0.5 * (b - a);
}

// xy: the flow's direction (world x, z) times its quantised speed; z: 1 where falling.
vec3 flowAt(ivec3 c) {
    int x = c.x;
    int y = c.y;
    int z = c.z;
    if (y > 0 && !solidAt(x, y - 1, z) && fillPxAt(x, y - 1, z) < S) {
        return vec3(0.0, 0.0, 1.0);
    }
    // The cell's own column by the same rule as its neighbours': a submerged cell (a
    // front face's lower rows) reads the surface over it, as they do.
    float h0 = surfaceNear(x, y, z);
    float gx = slope1(surfaceNear(x - 1, y, z), h0, surfaceNear(x + 1, y, z));
    float gz = slope1(surfaceNear(x, y, z - 1), h0, surfaceNear(x, y, z + 1));
    vec2 f = -vec2(gx, gz);
    float m = length(f);
    float q = floor(clamp(m / FLOW_FULL, 0.0, 1.0) * FLOW_STEPS + 0.5) / FLOW_STEPS;
    return vec3(m > 0.0 ? f / m * q : vec2(0.0), 0.0);
}

int wrapI(int a, int m) {
    int r = a % m;
    return r < 0 ? r + m : r;
}

// Steps in one flow-map cycle, and voxels the ripples move a step at full speed (one
// voxel a second at 12 Hz). Two copies half a cycle apart take turns: a ripple line
// appears, drifts along the flow and fades within its copy's cycle, so the advection never
// runs away and a restart is never seen.
const float FLOW_CYCLE = 64.0;
const float FLOW_ADVECT = 1.0 / 12.0;
// The ripple lines: segments this many voxels long along x (a line takes part of one),
// and the share of (row, segment) places that carry a line some time in its cycle.
const float LINE_SEG = 2.0;
const float LINE_DENSITY = 0.16;
// A line lives this share of its copy's cycle (between the two), starting at a random
// point in it; it fades in and out over its life (sin²) and grows from its middle.
const float LINE_LIFE_MIN = 0.3;
const float LINE_LIFE_MAX = 0.6;
// A breeze: each row of lines slides along +x at its own speed, between these voxels a
// second at full water speed, on still and moving water alike.
const float BREEZE_MIN = 0.1;
const float BREEZE_MAX = 0.3;

// One copy of the ripple lines at world (x, z), `phase` of the way through cycle `cycle`:
// 0 for none, else the tilt along z, signed, times the line's fade (0 to 1). A line is one
// raster row of a top face (RISE rows a voxel of z) and part of a LINE_SEG-voxel segment.
float rippleLine(vec2 w, vec2 flow, float phase, int cycle, int salt) {
    float t = phase * FLOW_CYCLE * FLOW_ADVECT;
    vec2 a = w - flow * t;
    int row = int(floor(a.y * float(RISE)));
    uint hr = cellHash(row, 0, cycle, salt);
    a.x -= mix(BREEZE_MIN, BREEZE_MAX, float((hr >> 8) & 0xFFu) / 255.0) * t;
    // Segments across the ring: a whole number, so the pattern meets itself at the seam.
    int segs = max(1, int(float(W) / LINE_SEG + 0.5));
    float sx = a.x * float(segs) / float(W) + float(hr & 0xFFu) / 256.0;
    int seg = int(floor(sx));
    uint h = cellHash(wrapI(seg, segs), row, cycle, salt + 1);
    if (float(h & 0xFFFFu) / 65535.0 >= LINE_DENSITY) { return 0.0; }
    uint hl = cellHash(wrapI(seg, segs), row, cycle, salt + 2);
    float life = mix(LINE_LIFE_MIN, LINE_LIFE_MAX, float(hl & 0xFFu) / 255.0);
    float age = (phase - float((hl >> 8) & 0xFFu) / 255.0 * (1.0 - life)) / life;
    if (age <= 0.0 || age >= 1.0) { return 0.0; }
    float fade = sin(3.14159265 * age);
    fade *= fade;
    float f = sx - float(seg);
    float mid = 0.25 + float((h >> 16) & 0xFFu) / 255.0 * 0.5;
    float halfLen = (0.1 + float((h >> 24) & 0x7Fu) / 127.0 * 0.15) * sqrt(fade);
    if (abs(f - mid) >= halfLen) { return 0.0; }
    return ((h & 0x80000000u) != 0u ? 1.0 : -1.0) * fade;
}

// The surface normal at world (x, z) under flow `flow`: straight up, or, on a ripple line,
// tilted along z (toward or away from the camera) by up to the ripple knob. Lines are
// sparse, thin and horizontal; they fade in, glide with the breeze (and the flow, on
// moving water) and fade out.
vec3 rippleNormal(vec2 w, vec2 flow) {
    float st = u.clock.y;
    float c1 = st / FLOW_CYCLE;
    float c2 = c1 + 0.5;
    // The step wraps at 2048 (`VoxelRenderer::set_clock`): 32 cycles.
    float s = rippleLine(w, flow, fract(c1), wrapI(int(floor(c1)), 32), 60);
    if (s == 0.0) {
        s = rippleLine(w + vec2(0.37, 0.0), flow, fract(c2), wrapI(int(floor(c2)), 32), 62);
    }
    if (s == 0.0) { return vec3(0.0, 1.0, 0.0); }
    return normalize(vec3(0.0, 1.0, -s * u.waterL.z));
}

// A ceiling seen in a reflection: the underside of a solid, which the picture never
// draws; the ambient ladder's floor rung and whatever local light reaches it.
vec3 undersideLit(ivec3 c, uvec4 v, vec3 hp) {
    vec3 body = blockBody(v, strataOf(matOf(v)));
    return hazed(body * ladderLight(0.0, 0.0, glowAt(hp - vec3(0.0, 0.5, 0.0))),
                 hazeAt(float(c.z)));
}

// The front-face texel (r rows up from its bottom, dx across) a reflected ray entering
// cell `c` through `axis` at `hp` lands on: a side face (axis 0) is drawn as the front's
// edge column on that side, an underside (axis 1) as its bottom row.
ivec2 reflectTexel(vec3 hp, ivec3 c, int axis, float dirX) {
    int dx = clamp(int(floor(fract(hp.x) * float(S))), 0, S - 1);
    int dy = clamp(int(floor((float(c.y + 1) - hp.y) * float(S))), 0, S - 1);
    if (axis == 0) { dx = dirX > 0.0 ? 0 : S - 1; }
    if (axis == 1) { dy = S - 1; }
    return ivec2(S - 1 - dy, dx);
}

bool glyphFilled(int base, int col, int row) {
    if (col < 0 || col >= S || row < 0 || row >= S) { return true; }
    return (texelFetch(glyphTex, ivec2(col, base + row), 0).r >> 2) != 63u;
}

// Whether a reflected ray stops at a block part's face texel: the texel and its four
// neighbours on the face all drawn (past the face's edge counts as drawn, so a body of
// several cells keeps its edges). Thin parts (stems, sprigs, a berry's pixel) are passed
// through: in a rippled mirror they are specks that blink.
bool glyphTexelAt(uvec4 v, vec3 hp, ivec3 c, int axis, float dirX) {
    ivec2 t = reflectTexel(hp, c, axis, dirX);
    int base = (partOf(v) * 8 + glyphOf(v)) * (S + RISE);
    int row = S - 1 - t.x;
    int col = t.y;
    return glyphFilled(base, col, row) && glyphFilled(base, col - 1, row)
        && glyphFilled(base, col + 1, row) && glyphFilled(base, col, row - 1)
        && glyphFilled(base, col, row + 1);
}

// What the surface at `p` mirrors along `r`: a DDA through the volume, at most
// `reflect_cells` cells, to the first terrain or block part (`hit`, entered through
// `axis` at `hp`); false when the ray leaves the world or is still going at the cap
// (`hp` is then where it went). The march only finds the hit; `reflectionShade` shades
// it once the march's own state is dead, which keeps this pass's register count (and so
// the whole shader's occupancy) near the walk's own.
bool reflectMarch(vec3 p, vec3 r, out ivec3 hit, out vec3 hp, out int axis) {
    ivec3 c = ivec3(floor(p + r * 1e-3));
    ivec3 dir = ivec3(sign(r));
    vec3 inv = vec3(
        abs(r.x) > 1e-6 ? 1.0 / abs(r.x) : 1e30,
        abs(r.y) > 1e-6 ? 1.0 / abs(r.y) : 1e30,
        abs(r.z) > 1e-6 ? 1.0 / abs(r.z) : 1e30);
    vec3 next = vec3(
        (dir.x > 0 ? float(c.x + 1) - p.x : p.x - float(c.x)) * inv.x,
        (dir.y > 0 ? float(c.y + 1) - p.y : p.y - float(c.y)) * inv.y,
        (dir.z > 0 ? float(c.z + 1) - p.z : p.z - float(c.z)) * inv.z);
    int cap = int(u.waterL.w);
    float tIn = 0.0;
    axis = -1;
    hit = c;
    for (int i = 0; i < cap; ++i) {
        if (c.y >= H || c.y < 0 || c.z < 0 || c.z >= D) { break; }
        if (i > 0) {
            uvec4 v = at(c.x, c.y, c.z);
            // A block part counts where its glyph has a texel (the atlas's own holes let
            // the ray on); a texture's leaf cutout does not (`reflectionShade`).
            if (solidV(v) || (isBlockPart(partOf(v)) && glyphTexelAt(v, p + r * tIn, c, axis, r.x))) {
                hit = c;
                hp = p + r * tIn;
                return true;
            }
        }
        axis = (next.x < next.y && next.x < next.z) ? 0 : (next.y < next.z ? 1 : 2);
        if (axis == 0) { tIn = next.x; c.x += dir.x; next.x += inv.x; }
        else if (axis == 1) { tIn = next.y; c.y += dir.y; next.y += inv.y; }
        else { tIn = next.z; c.z += dir.z; next.z += inv.z; }
    }
    hp = p + r * tIn;
    return false;
}

// The reflection's colour: the hit's face under L's light (`shadeFront`: the ladder, AO,
// the sun and its shadows, the glow), from its base colour (terrain: the material or its
// texture, wet; a block part: its glyph texel's pigment or texture, and an emitter at
// full value), without the picture's edge treatments (rims, bevels, tones), which a
// ripple-broken mirror image at 40 % would not show and which cost the whole shader
// occupancy to inline a second time; a solid's underside at the ladder's floor; or the
// sky gradient at the raster row where the ray left.
vec3 reflectionShade(bool found, ivec3 c, vec3 hp, int axis, vec3 r) {
    if (!found) {
        int row = int(floor(float(BASE) - hp.y * float(S) - hp.z * float(RISE)));
        return skyAt(max(row, 0));
    }
    uvec4 v = at(c.x, c.y, c.z);
    ivec2 t = reflectTexel(hp, c, axis, r.x);
    int dy = S - 1 - t.x;
    bool solid = solidV(v);
    if (solid && axis == 1) { return undersideLit(c, v, hp); }
    vec3 base;
    float canopy;
    if (solid) {
        int m = matOf(v);
        base = blockBody(v, faceBase(m, 2 * (m - 1), c.x, c.y, c.z, 0, t.y, dy));
        canopy = canopyAt(c.x, c.z - 1, c.y);
    } else {
        int atlasY = (partOf(v) * 8 + glyphOf(v)) * (S + RISE) + dy;
        uint q = texelFetch(glyphTex, ivec2(t.y, atlasY), 0).r;
        vec4 pigment = texelFetch(styleTex, ivec2(int(q & 3u), int(v.a)), 0);
        vec3 e = styleEmit(int(v.a));
        if ((int(q >> 2) == TONE_EMIT || emitsWhole(pigment.a)) && any(greaterThan(e, vec3(0.0)))) {
            return hazed(e, hazeAt(float(c.z)));
        }
        base = pigment.rgb;
        // A texture's leaf hole: the cell's crown pigment.
        if (!plantTexel(c.x, c.y, c.z, v, false, t.y, dy, base)) { base = styleAt(int(v.a), 1); }
        canopy = partOf(v) == ANIMAL_INTERIM ? canopyAt(c.x, c.z - 1, c.y) : plantCanopy(v);
    }
    return hazed(shadeFront(base, c.x, c.y, c.z, t.y, dy, canopy), hazeAt(float(c.z)));
}

// A falling-water streak at raster pixel (px, py): a third of the pixel columns carry
// one, S/3 rows long (at least 2) once every 16 rows (32 from 8 px a voxel up), moving
// down S/4 rows (at least 1) an animation step. The periods divide the step's wrap.
bool fallStreak(int px, int py, float st) {
    uint h = cellHash(wrapI(px, W * S), 0, 0, 50);
    if (h % 3u != 0u) { return false; }
    int period = S >= 8 ? 32 : 16;
    int len = max(2, S / 3) + int((h >> 8) & 1u);
    int speed = max(1, S / 4);
    int ph = wrapI(py - int(floor(st * float(speed))) + int(h >> 16), period);
    return ph < len;
}

// The capture-only flow overlay (`VoxelParams::debug_flow`): still water dark blue,
// moving water hued by its direction (+x red, +z green, -x cyan, -z violet) and brighter
// with speed, falling water white-on-violet stripes; from 8 px a voxel, a white line (its
// head yellow) from each top face's centre along its flow.
vec3 flowDebug(vec3 fl, bool top, vec3 p) {
    if (fl.z > 0.5) {
        return fallStreak(PX, PY, u.clock.y) ? vec3(1.0) : vec3(0.35, 0.05, 0.5);
    }
    float q = length(fl.xy);
    if (q < 0.01) { return vec3(0.01, 0.02, 0.2); }
    float a = atan(fl.y, fl.x);
    vec3 hue = 0.5 + 0.5 * vec3(cos(a), cos(a - 2.0944), cos(a + 2.0944));
    vec3 c = hue * (0.25 + 0.75 * q);
    if (top && S >= 8) {
        vec2 lp = vec2(fract(p.x) * float(S), (1.0 - fract(p.z)) * float(RISE));
        vec2 o = vec2(0.5 * float(S), 0.5 * float(RISE));
        vec2 d = normalize(vec2(fl.x * float(S), -fl.y * float(RISE)));
        vec2 e = o + d * 0.45 * float(S) * vec2(1.0, float(RISE) / float(S));
        vec2 pa = lp - o;
        vec2 ba = e - o;
        float t = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-4), 0.0, 1.0);
        if (length(pa - ba * t) < 0.75) { c = t > 0.6 ? vec3(1.0, 1.0, 0.3) : vec3(1.0); }
    }
    return c;
}

// The lit water, put together once the walk is over: `colour` is what the walk made of
// the pixel with the water's absorption in it and its in-scatter still out. The entry is
// recomputed from `wEntry`: its cell, whether it is the surface, the point where the ray
// entered, and the open cell whose light it reads.
vec3 litWaterFinish(vec3 colour) {
    if ((wEntry & ENTRY_FLAT) != 0) {
        colour += wScatter + wHaze * u.hazeC.rgb;
        if ((wEntry & ENTRY_FALLING) == 0) { return colour; }
        if (u.clock.z > 0.5) { return fallStreak(PX, PY, u.clock.y) ? vec3(1.0) : vec3(0.35, 0.05, 0.5); }
        // A fall's streaks over the flat tier's sheet: the surface colour itself (opaque,
        // hazed), a third of the columns, moving down. The palette has nothing brighter,
        // and the sheet between them keeps the flat tier's brightness.
        int zf = (wEntry >> 12) & 0xFFF;
        return fallStreak(PX, PY, u.clock.y) ? hazed(u.waterSurfaceC.rgb, hazeAt(float(zf))) : colour;
    }
    int y = wEntry & 0xFFF;
    int z = (wEntry >> 12) & 0xFFF;
    bool top = (wEntry >> 24) != 0;
    int x = PX / S;
    int fill = fillPxAt(x, y, z);
    float k = float(RISE) / float(S);
    float yf = (float(BASE - z * RISE - PY - 1) + 0.5) / float(S);
    float xw = (float(PX) + 0.5) / float(S);
    float ys = float(y) + float(fill) / float(S);
    vec3 p = top ? vec3(xw, ys, float(z) + clamp((yf - ys) / k, 0.0, 1.0)) : vec3(xw, yf, float(z));
    ivec3 open = top ? ivec3(x, fill < S ? y : y + 1, z) : ivec3(x, y, z - 1);
    float canopy = top ? canopyAt(x, z, y) : canopyAt(x, z - 1, y);

    vec3 fl = flowAt(ivec3(x, y, z));
    if (u.clock.z > 0.5) { return flowDebug(fl, top, p); }
    // The ladder's light at the entry, as a ratio to open sunlit sky.
    float sunOn = u.sunK.y > 0.0 ? 1.0 : 0.0;
    float ndl = top ? u.sunK.y : -u.sunK.z;
    float sun = (sunOn > 0.0 && ndl > 0.0) ? sunReaches(p, open) : 0.0;
    vec3 glow = glowAt(top ? p + vec3(0.0, 0.5, 0.0) : p - vec3(0.0, 0.0, 0.5));
    vec3 here = ladderLight(skyOpen(open.x, open.y, open.z) * canopy, sun, glow);
    vec3 ratio = here / max(ladderLight(1.0, sunOn, vec3(0.0)), vec3(1e-4));
    colour += wScatter * ratio + wHaze * u.hazeC.rgb;
    if (!top) { return colour; }
    vec3 n = rippleNormal(p.xz, fl.xy);
    vec3 d = normalize(vec3(0.0, -k, 1.0));
    float cosi = clamp(-dot(d, n), 0.0, 1.0);
    float f = clamp(u.waterL.y * (0.02 + 0.98 * pow(1.0 - cosi, 5.0)), 0.0, 1.0);
    vec3 r = reflect(d, n);
    r.y = max(r.y, 0.05);
    r = normalize(r);
    // What is behind the surface first, so the march and the shading carry little state.
    vec3 under = (1.0 - f) * colour;
    ivec3 hc;
    vec3 hp;
    int axis;
    bool found = reflectMarch(p, r, hc, hp, axis);
    return under + f * reflectionShade(found, hc, hp, axis, r);
}

void main() {
    S = u.geom.x;
    RISE = u.geom.y;
    BASE = u.geom.z;
    W = u.extent.x;
    H = u.extent.y;
    D = u.extent.z;

    ivec2 px = ivec2(gl_FragCoord.xy);
    int x = px.x / S;
    int dx = px.x - x * S;

    vec3 acc = vec3(0.0);
    vec3 trans = vec3(1.0);
    if (LIT) {
        PX = px.x;
        PY = px.y;
        waterLitReset();
    }

    for (int z = 0; z < D; ++z) {
        int q = BASE - z * RISE - px.y - 1;
        // q falls by `rise` a slab: once the pixel is below this slab's y = 0 front face
        // it is below every deeper slab's too.
        if (q < 0) { break; }
        int level = q / S;
        int r = q - level * S;

        // The band's own voxel, which the CPU paints last of the two and which therefore
        // comes first walking front to back.
        if (level < H) {
            uvec4 v = at(x, level, z);
            if (solidV(v)) {
                // A covered rock face looking at the camera: its vine, carried by the air
                // cell in front, is drawn flush on the rock, which shows through its holes.
                if (z > 0 && u.tex.y != 0) {
                    uvec4 f = at(x, level, z - 1);
                    if (isVine(f) && vineKind(f) == VINE_FLUSH) {
                        vec4 t = vineTexel(f, x, level, z - 1, dx, S - 1 - r);
                        if (t.a >= 0.5) {
                            if (LIT && !vineEmits(t)) {
                                t.rgb = shadeFront(t.rgb, x, level, z, dx, S - 1 - r,
                                                   canopyAt(x, z - 1, level));
                            }
                            float h = hazeAt(float(z));
                            acc += trans * hazed(t.rgb, h);
                            trans = LIT && vineEmits(t) ? emitterTrans(trans, hazed(t.rgb, h), h)
                                                        : vec3(0.0);
                            break;
                        }
                    }
                }
                acc += trans * blockFront(x, level, z, v, r, dx);
                trans = vec3(0.0);
                break;
            }
            // A plant stands in the void and the water of its own cell blends over it: a
            // trunk in a pool is submerged, so the water is nearer than the plant.
            if (v.g != 0u) { waterAt(x, level, z, v, px.y, acc, trans); }
            if (isVine(v)) {
                int k = vineKind(v);
                int q = max(1, S / 4);
                if (k == VINE_CURTAIN || (k == VINE_SLIVER_L && dx < q)
                    || (k == VINE_SLIVER_R && dx >= S - q)) {
                    vec4 t = vineTexel(v, x, level, z, dx, S - 1 - r);
                    if (t.a >= 0.5) {
                        if (LIT && !vineEmits(t)) {
                            t.rgb = shadeFront(t.rgb, x, level, z, dx, S - 1 - r,
                                               canopyAt(x, z - 1, level));
                        }
                        float h = hazeAt(float(z));
                        acc += trans * hazed(t.rgb, h);
                        trans = LIT && vineEmits(t) ? emitterTrans(trans, hazed(t.rgb, h), h)
                                                    : vec3(0.0);
                        break;
                    }
                }
            }
            int p = partOf(v);
            if (p != 0) {
                vec3 art;
                bool lum;
                if (glyphFront(x, level, z, v, r, dx, art, lum)) {
                    acc += trans * art;
                    trans = LIT && lum ? emitterTrans(trans, art, hazeAt(float(z))) : vec3(0.0);
                    break;
                }
            }
        }

        // The cap of the voxel below the band, which reaches its bottom `rise` rows.
        // Getting here means the band's own voxel is air, holds no block plant, or holds
        // a model's leaf or drape with a hole at this pixel - so the cap is what shows
        // through, which for the terrain is the CPU's `open_up` and `!covered_up`.
        int below = level - 1;
        if (r < RISE && below >= 0 && below < H) {
            uvec4 v = at(x, below, z);
            if (solidV(v)) {
                acc += trans * blockTop(x, below, z, v, r, dx);
                trans = vec3(0.0);
                break;
            }
            // The cell's own water before its own plant, here as at the level above: the
            // CPU stamps a plant and then blends the water of its cell over it, and a
            // full-to-the-brim cell with air over it has its water *top* in the band
            // above — which is this band, exactly where the plant's cap is. A submerged
            // crown is seen through that surface, not instead of it.
            if (v.g != 0u) { waterAt(x, below, z, v, px.y, acc, trans); }
            int p = partOf(v);
            vec3 cap;
            bool lum;
            if (isBlockPart(p) && glyphCap(x, below, z, v, r, dx, cap, lum)) {
                acc += trans * cap;
                trans = LIT && lum
                    ? emitterTrans(trans, cap, hazeAt(float(z) + float(r) / float(RISE)))
                    : vec3(0.0);
                break;
            }
        }

        if (all(lessThanEqual(trans, vec3(0.0)))) { break; }
    }

    // An emitter ended the walk: what it sends the camera is the bloom's.
    vec3 emitted = vec3(0.0);
    if (LIT && trans.x < -0.5) {
        emitted = -trans - 1.0;
        trans = vec3(0.0);
    }
    emitOut = vec4(emitted, 1.0);

    // The presenter clears to the sky and paints over it; front to back, the sky is
    // whatever light is left.
    vec3 sky = skyColor(px.x, px.y);
    vec3 colour = acc + trans * sky;
    if (LIT && wEntry >= 0) { colour = litWaterFinish(colour); }
    outColour = vec4(colour, 1.0);

    // Falling rain animation streaks when active
    if (u.knobs.w > 0.0) {
        int tick = int(u.knobs.w);
        int speed = 4;
        vec3 rainC = u.waterSurfaceC.rgb;
        for (int len = 0; len < 4; ++len) {
            int c = px.x - (len / 2);
            uint hash = ((uint(c) * 1664525u + 1013904223u) >> 16);
            if ((hash % 5u) == 0u) {
                int streakLen = 3 + int(hash & 1u);
                if (len < streakLen) {
                    int yOffset = int((uint(tick * speed) + hash) % 32u);
                    int period = 32 + int(hash % 16u);
                    int r = (px.y - len - yOffset) % period;
                    if (r < 0) { r += period; }
                    if (r == 0) {
                        float a = (len == streakLen - 1) ? 0.40 : 0.20;
                        outColour.rgb = mix(outColour.rgb, rainC, a);
                        break;
                    }
                }
            }
        }
    }
}
