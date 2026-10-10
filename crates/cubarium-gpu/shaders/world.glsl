// The world as the slab walk reads it, and its light: the uniform block, the walk's
// specialisation constants, the world's textures, and the functions that read them.
//
// Shared by `voxel.frag` (the slab walk) and `figure.frag` (the model pass, H2 of
// `design/handoffs/hybrid-renderer-2026-10-07.md`): an animal's model is lit by the same
// sun march, sky plane, canopy, glow volume, ladder and haze as the terrain behind it.
// Whoever includes this sets S, RISE, BASE, W, H and D from the block first thing in
// `main`.

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
    vec4 waterM;        // lit: foam, glint, rain rings, highlight (toward white)
    vec4 volK;          // lit: light shafts' density, glow weight, falloff (voxels), fade
    // The weather and the time of day (`cubarium_gpu::weather::Look`, package WX2). At a
    // clear noon every one of these leaves today's picture as it was.
    vec4 dayK;          // lit: the sun's share of a rung (ladder, smooth) over the ambient
                        // level, that level (ambientC is already times it); the twilight band
    vec4 dayL;          // lit: the unlit water's level (1 by day), the sun's strength, -, -
    vec4 sunLean;       // lit: what a sunlit texel leans toward; which side the sun is on (-1..1)
    vec4 cloudK;        // lit sky: cloud cover, drift (voxels), storm, twinkle clock (seconds)
    vec4 cloudBody;     // lit sky: the clouds' body colour; the stars' brightness
    vec4 cloudLit;      // lit sky: the clouds' sunlit edge colour, and its strength
    vec4 bandA;         // the twilight band's colours, top to bottom
    vec4 bandB;
    vec4 bandC;
    vec4 tint;          // flat: the whole picture's day/night tint (1 by day)
    vec4 dayA;          // lit sky: the day sky's zenith; how much of the day sky shows
    vec4 dayB;          // lit sky: its middle; the stars' share into the bloom
    vec4 dayC;          // lit sky: its horizon
    vec4 cloudDay;      // lit sky: the clouds' body by day
    vec4 cloudShade;    // lit sky: the clouds' shadowed underside by day
    // Rain, fog and lightning (WX2 checkpoint 2, `cubarium_gpu::weather::Look`). All zero at
    // a clear noon.
    vec4 rainK;         // how much drizzle, shower and downpour is falling (0..1 each); the
                        // rain rings' density on open water (0..1, from the rate)
    vec4 rainC;         // the streaks' colour (linear); how much of the sun the fog sends back
    vec4 fogK;          // the fog's density a voxel of path at its floor (0: none), the floor
                        // (voxels up), its full-density band above that, its falloff (voxels)
    vec4 fogC;          // the ambient light on the fog (linear); the emitters' glow it scatters
    vec4 flashC;        // a lightning flash's fill light (linear, already times the flash); the
                        // flash (0..1)
    vec4 strikeK;       // the bolt: its column's x centre, top y and z (voxels), its brightness
    vec4 strikeL;       // the bolt's seed, as two 16-bit halves
    // The pose this frame (WX4, `cubarium_gpu::voxel::pose_rows`): entry k's settle and
    // breath dip, in pixel rows, evaluated on the frame's clock.
    vec4 poseK[64];
    // Each style slot's emission gain this frame (`VoxelRenderer::set_emit_gains`, the
    // glowcap candle), four slots a row; 1 draws the emissive colour as it is.
    vec4 emitK[64];
    // The view (the hosted renderer's, H1): raster columns the picture is turned round the
    // ring (the camera's pan, whole pixels; 0 draws the strip as it always was), and the
    // highlighted animal's cell count (0 none, -1 its box alone).
    ivec4 view;
    ivec4 hiMin;        // the highlighted animal's box: first x (wrapped), lowest y, nearest z
    ivec4 hiMax;        // its x span in cells, highest y, farthest z
    vec4 hiK;           // how far its cells lean toward white
    uvec4 hiCells[64];  // its cells, x | y << 12 | z << 22, four a row
    // The figures' sun map (H2, `cubarium_gpu::figures`): a figure's highest point along
    // each sun ray, by where the ray meets y = 0. figSun: Lx/Ly, Lz/Ly, the map's first z,
    // on (1) or off; figMap: texels a voxel across and along z, the map's size.
    vec4 figSun;
    vec4 figMap;
    // How the ground fog moves (FOG-MOTION, `cubarium_gpu::weather::FogMotion`): the mode
    // (0 fixed, 1 breathing, 2 flowing), the breath's swing (a share of the density), its
    // period (seconds), the flow (voxels a second along +x).
    vec4 fogM;
    // How much of each rain mode's haze hangs in the air (drizzle, shower, downpour; eased
    // far slower than `rainK`, so the veil gathers and thins rather than switching).
    vec4 rainH;
} u;

// The lighting tier (`lighting = "flat" | "lit"`), fixed when the pipeline is built. The
// flat tier's pipeline has it off: every `if (LIT)` below is dead code there, and what
// runs is the flat shader as it was. The lit tier replaces the flat tier's stand-ins for
// light (the column roof shade, and the TOP_* / PLANT_TOP_* gain-and-tint mixes) with a
// light value that multiplies each texel's base colour; the edge treatments are the
// flat tier's, leaning between the lit tones instead of the flat ones.
layout(constant_id = 0) const bool LIT = false;
// The lit tier's effect switches (`cubarium_gpu::sunvis::Effects`), all off in the flat
// tier: an effect switched off is dead code and costs nothing.
layout(constant_id = 1) const bool SHADOWS = true;      // the sun march
layout(constant_id = 2) const bool REFLECTIONS = true;  // the water's reflection march
layout(constant_id = 3) const bool VOLUMETRIC = false;  // the light shafts (volumetric.glsl)
layout(constant_id = 4) const bool GLOW = true;         // the emitters' local light
layout(constant_id = 5) const bool AO = true;           // the AO crease (`[light] ao` > 0)
// The lit tier's sky pass (package WX2): this shader again, specialised to draw only the
// sky (`litSky`: the gradient, the twilight band, the stars, the clouds) into the backdrop
// image, just before the walk reads it. Kept out of the walk's own pipeline on purpose: the
// walk sits at an occupancy cliff, and the clouds' noise compiled into it cost 0.4 ms at
// 13 px before a single cloud was drawn.
layout(constant_id = 6) const bool SKY = false;
// The lit tier's weather pass (WX2 checkpoint 2): this shader again, specialised to draw
// only the weather (`weatherOver`: fog, rain, splashes, the lightning bolt) over the raster
// after the walk, premultiplied, before the bloom. Out of the walk for the same reason as
// the sky; its pipeline keeps the walk's effect switches, so it knows whether the fog is
// already in the volumetric march.
layout(constant_id = 7) const bool WEATHER = false;
// The lit walk writes its end depth for the weather pass (in the emission's alpha). A
// second walk pipeline, used only on frames that draw weather: keeping the walk's end
// slab to the end of the shader costs the walk about 0.1 ms at 13 px, which a clear sky
// does not pay.
layout(constant_id = 8) const bool WEATHER_DEPTH = false;
// The sleep and idle poses (WX4, `posedFind`): a second walk pipeline, bound only on
// frames whose pack has an animal posing. The search costs the walk registers (a step of
// occupancy on the desktop's RDNA) even where no animal is, so the rest of the time it is
// dead code.
layout(constant_id = 9) const bool POSE = false;
// **Plants as models** (`[figures] plants = "models"`, `crate::plants`): the walk's ray passes
// through every dry block plant cell (trunk, crown, heart), which the plant pass draws
// instead, swaying and growing in. The cells stay in the volume for everything else (the sun
// march, the sky plane, the canopy, the AO), so the plants light and shade as they did.
// Fixed for a renderer's life; off, the walk is exactly what it was.
layout(constant_id = 10) const bool PLANT_MODELS = false;

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
// Lit tier only. The sky behind the world, from the sky pass (`SKY`), in linear light, one
// texel a raster pixel; the flat tier's is a single texel it never reads.
layout(set = 0, binding = 11) uniform sampler2D backdropTex;
// Lit tier only, read by the weather pass alone: the emission attachment, whose alpha the
// walk sets to the slab its ray ended in (over 255; the water surface where there is one),
// the weather's depth. The flat tier binds its one backdrop texel here.
layout(set = 0, binding = 12) uniform sampler2D depthTex;
// Lit tier: the figures' sun map (depth: a figure's height over H), `figSun` above.
layout(set = 0, binding = 13) uniform sampler2D figSunTex;

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

// `a` round a ring of `m`, in 0..m. GLSL leaves `%` undefined for a negative operand, and
// NVIDIA's driver takes the unsigned remainder there: -1 % 640 came out 255, so on a ring
// whose width is not a power of two every lookup one cell left of the seam read a column
// 384 away (the sun march's shadows, the AO, the sky plane and the canopy at the seam).
// Only non-negative operands reach `%` here.
int wrapMod(int a, int m) {
    return a >= 0 ? a % m : m - 1 - (-1 - a) % m;
}

int wrapX(int x) { return wrapMod(x, W); }

bool inY(int y) { return y >= 0 && y < H; }

uvec4 at(int x, int y, int z) { return texelFetch(voxels, ivec3(wrapX(x), y, z), 0); }

int matOf(uvec4 v) { return int(v.r & 3u); }
int partOf(uvec4 v) { return int((v.r >> 2) & 7u); }
int glyphOf(uvec4 v) { return int(v.r >> 5); }
bool isBlockPart(int p) { return p == TRUNK || p == CROWN || p == CROWN_HEART || p == ANIMAL_INTERIM || p == LOG; }
// A plant's own block cell: what the plant pass draws when plants are models.
bool plantBlockPart(int p) { return p == TRUNK || p == CROWN || p == CROWN_HEART; }
// A faceless animal cell of class torso (WX4 idle): an empty voxel under a dipping head,
// there only so the walk looks for the head from it. It is air to everything else (no
// crease, no shadow, no cover), so it can come and go with a head move unseen. The sleep's
// belly cells (class leg) keep counting as the settled body.
bool softMark(uvec4 v) {
    return POSE && partOf(v) == ANIMAL_INTERIM && glyphOf(v) == 0 && v.b != 0u && (v.b >> 6) < 2u;
}
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
vec3 styleEmit(int style) {
    return texelFetch(styleTex, ivec2(3, style), 0).rgb * u.emitK[style >> 2][style & 3];
}
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
    return (solidV(v) || (isBlockPart(partOf(v)) && !softMark(v))) ? 1.0 : 0.0;
}

// The AO is a **crease line** (Wrysk, checkpoint 2): a face stays flat, and only a thin
// band along an edge whose side neighbour in the face's open plane occludes, and a small
// square in a corner where only the diagonal neighbour does, take the AO step: the light
// times 1 - strength, before the ladder snaps it (one rung down at the default 0.5 on a
// 4-rung ladder). The band is about S/8 wide, at least 1 px (1 px at 6 px a voxel, 2 at 13),
// on a top face at most half its rows.
int creaseWidth(int rows) { return min(max(1, (S + 4) / 8), max(1, rows / 2)); }

// The shared appearance atlas: front rows followed by the receding cap rows.
uint glyphTexel(int part, int glyph, int dx, int row) {
    return texelFetch(glyphTex, ivec2(dx, (part * 8 + glyph) * (S + RISE) + row), 0).r;
}

// The crease factor at texel (du, dv) of a face nu x nv texels, dv = 0 on its `hi` side,
// from its four side neighbours and four diagonals (lo/hi x left/right).
float creaseBands(float l, float r, float lo, float hi,
             float cLL, float cRL, float cLH, float cRH, int du, int dv, int nu, int nv, int b) {
    bool inL = du < b, inR = du >= nu - b, inH = dv < b, inLo = dv >= nv - b;
    bool band = (inL && l > 0.5) || (inR && r > 0.5) || (inH && hi > 0.5) || (inLo && lo > 0.5)
        || (inL && inLo && cLL > 0.5) || (inR && inLo && cRL > 0.5)
        || (inL && inH && cLH > 0.5) || (inR && inH && cRH > 0.5);
    return band ? 1.0 - u.lightK.w : 1.0;
}

float crease(float l, float r, float lo, float hi,
             float cLL, float cRL, float cLH, float cRH, int du, int dv, int nu, int nv) {
    return creaseBands(l, r, lo, hi, cLL, cRL, cLH, cRH, du, dv, nu, nv, creaseWidth(nv));
}

// A figure face's baked open-plane occluders, in the same order as `crease`.
float modelCrease(uint mask, int dx, int dy, ivec2 size) {
    if (!AO) { return 1.0; }
    return creaseBands(float(mask & 1u), float((mask >> 1) & 1u),
        float((mask >> 2) & 1u), float((mask >> 3) & 1u),
        float((mask >> 4) & 1u), float((mask >> 5) & 1u),
        float((mask >> 6) & 1u), float((mask >> 7) & 1u), dx, dy, size.x, size.y, creaseWidth(min(size.x, size.y)));
}

// AO of a front face of (x, y, z) at texel (dx, dy), dy = 0 at the top: the open plane is
// z-1.
float aoFront(int x, int y, int z, int dx, int dy) {
    if (!AO) { return 1.0; }
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
    if (!AO) { return 1.0; }
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
//
// The time of day adds nothing to this function's work (every extra live value in the walk
// costs occupancy: 0.3-0.6 ms at 13 px, measured): the ambient colour the CPU sets is
// already times the ambient level (1 by day), and the sun's share of a rung (`dayK.x` on a
// ladder, `dayK.y` smooth: exactly 1 and SMOOTH_SUN at a clear noon) is already times the
// sun's strength over that level. So the ambient and the floor dim at night, the sun or
// the moon keeps its own strength, and the emitters' light, mixed in by its share of the
// sum, comes to carry the dark.
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
    float t = smoothLight ? clamp(a, 0.0, 1.0) + sun * u.dayK.y
                          : (floor(clamp(a, 0.0, 1.0) * n + 0.5) + sun * u.dayK.x) / n;
    return hue * (u.lightK.x * (u.lightK.y + (1.0 - u.lightK.y) * t));
}

// The ladder's light on open, sunlit ground at a clear noon, in this light's hue (the
// ambient colour over its level): what the water's in-scatter is taken as a ratio to, so
// water darkens at night as the ground does.
vec3 ladderRef(float sunOn) {
    bool smoothLight = u.lightK.z < 1.5;
    float n = max(u.lightK.z - 1.0, 1.0);
    float t = smoothLight ? 1.0 + sunOn * SMOOTH_SUN : (floor(n + 0.5) + sunOn) / n;
    return u.ambientC.rgb / u.dayK.z * (u.lightK.x * (u.lightK.y + (1.0 - u.lightK.y) * t));
}

// The emitters' light at world point `p` (voxel units), trilinear over the glow volume
// whose texel i covers voxels 4i .. 4i + 4.
vec3 glowAt(vec3 p) {
    if (!GLOW) { return vec3(0.0); }
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
// Whether a figure stands between world point `p` and the sun: on the sun ray through `p`
// the map holds a figure higher than `p` (by a quarter voxel, so a top face does not shade
// itself).
bool figureShades(vec3 p) {
    if (u.figSun.w < 0.5) { return false; }
    float fx = p.x - p.y * u.figSun.x;
    float fz = p.z - p.y * u.figSun.y - u.figSun.z;
    vec2 t = vec2((fx - float(W) * floor(fx / float(W))) * u.figMap.x, fz * u.figMap.y);
    if (t.y < 0.0 || t.y >= u.figMap.w) { return false; }
    float top = texelFetch(figSunTex, ivec2(min(t, u.figMap.zw - 1.0)), 0).r * float(H);
    return top > p.y + 0.25;
}

float sunReaches(vec3 p, ivec3 c) {
    if (!SHADOWS) { return 1.0; }
    if (figureShades(p)) { return 0.0; }
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
        if (part == TRUNK || part == LOG || (part == ANIMAL_INTERIM && !softMark(v))) { return 0.0; }
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
// toward the sun's colour by the sun tint times N·L (the flat tier's lean of a top face
// toward `lightC`, now gated by the sun). The sun's colour (`sunLean`) is the palette's
// light by day, warm low down, the moon's at night; the lean's strength (`sunK.w`) is the
// tint times the sun's strength, raised for a low sun (`cubarium_gpu::weather::Look`).
vec3 litBy(vec3 base, float ambient, float sun, float ndl, vec3 local) {
    vec3 c = base * ladderLight(ambient, sun, local);
    return mix(c, u.sunLean.rgb, clamp(u.sunK.w * sun * ndl, 0.0, 1.0));
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

