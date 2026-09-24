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

// A style's emissive colour in linear light (`VoxelStyle::with_emit`), zero for none.
vec3 styleEmit(int style) { return texelFetch(styleTex, ivec2(3, style), 0).rgb; }

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
// The ambient is sky x AO x canopy, snapped to a ladder of `lightK.z` rungs between the
// floor `lightK.y` and one, times the gain `lightK.x`, in the ambient colour. The light
// is quantised; the base colour it multiplies is not.

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

// The quantised light: the ambient product's rung on the ladder, one rung higher where
// the sun reaches (so a shadow is exactly one rung darker than the sunlit texel beside
// it, and never below the floor), times the gain, in the ambient colour.
//
// `local` is the emitters' light at the texel (`glowAt`), added to the ambient product
// before it is snapped: its luminance raises the product, and the light's hue leans from
// the ambient colour toward the emitters' by their share of the sum, itself snapped to
// the ladder's steps. With no local light this is exactly the ambient ladder.
vec3 ladderLight(float a, float sun, vec3 local) {
    float n = max(u.lightK.z - 1.0, 1.0);
    vec3 hue = u.ambientC.rgb;
    float l = dot(local, vec3(0.2126, 0.7152, 0.0722));
    if (l > 1.0 / 512.0) {
        float sum = clamp(a, 0.0, 1.0) + l;
        float share = floor(l / sum * n + 0.5) / n;
        hue = mix(hue, local / l, share);
        a = sum;
    }
    float rung = floor(clamp(a, 0.0, 1.0) * n + 0.5) + sun;
    return hue * (u.lightK.x * (u.lightK.y + (1.0 - u.lightK.y) * rung / n));
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
bool glyphFront(int x, int y, int z, uvec4 v, int r, int dx, out vec3 rgb) {
    int localY = S - 1 - r;
    int atlasY = (partOf(v) * 8 + glyphOf(v)) * (S + RISE) + localY;
    uint q = texelFetch(glyphTex, ivec2(dx, atlasY), 0).r;
    vec3 base = glyphPigment(v, q);
    int tone = int(q >> 2);
    if (tone == 63) { return false; }
    if (LIT && tone == TONE_EMIT) {
        // An emitter: its style's emissive colour, unshadowed and at full value.
        vec3 e = styleEmit(int(v.a));
        if (any(greaterThan(e, vec3(0.0)))) {
            rgb = hazed(e, hazeAt(float(z)));
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

bool glyphCap(int x, int y, int z, uvec4 v, int r, int dx, out vec3 rgb) {
    int localY = RISE - 1 - r;
    int atlasY = (partOf(v) * 8 + glyphOf(v)) * (S + RISE) + S + localY;
    uint q = texelFetch(glyphTex, ivec2(dx, atlasY), 0).r;
    vec3 base = glyphPigment(v, q);
    int tone = int(q >> 2);
    if (LIT && tone == TONE_EMIT) {
        vec3 e = styleEmit(int(v.a));
        if (any(greaterThan(e, vec3(0.0)))) {
            rgb = hazed(e, hazeAt(float(z) + float(r) / float(RISE)));
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

// `VoxelPresenter::water`, for one screen row: the surface's own receding top face and
// the body below it, each blended at most once.
void waterAt(int x, int y, int z, uvec4 v, int row, inout vec3 acc, inout float trans) {
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
    float trans = 1.0;

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
                            acc += trans * hazed(t.rgb, hazeAt(float(z)));
                            trans = 0.0;
                            break;
                        }
                    }
                }
                acc += trans * blockFront(x, level, z, v, r, dx);
                trans = 0.0;
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
                        acc += trans * hazed(t.rgb, hazeAt(float(z)));
                        trans = 0.0;
                        break;
                    }
                }
            }
            int p = partOf(v);
            if (p != 0) {
                vec3 art;
                if (glyphFront(x, level, z, v, r, dx, art)) {
                    acc += trans * art;
                    trans = 0.0;
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
                trans = 0.0;
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
            if (isBlockPart(p) && glyphCap(x, below, z, v, r, dx, cap)) {
                acc += trans * cap;
                trans = 0.0;
                break;
            }
        }

        if (trans <= 0.0) { break; }
    }

    // The presenter clears to the sky and paints over it; front to back, the sky is
    // whatever light is left.
    vec3 sky = skyColor(px.x, px.y);
    outColour = vec4(acc + trans * sky, 1.0);

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
