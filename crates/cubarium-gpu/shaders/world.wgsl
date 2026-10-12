// The world as the slab walk reads it, and its light: the uniform block, the walk's
// pipeline-overridable switches, the world's textures, and the functions that read them.
//
// Shared by the slab walk (`voxel.wgsl`) and the model pass (`figure.frag.wgsl`, H2 of
// `design/handoffs/hybrid-renderer-2026-10-07.md`): an animal's model is lit by the same
// sun march, sky plane, canopy, glow volume, ladder and haze as the terrain behind it.
// Whoever imports this sets S, RISE, BASE, W, H and D from the block first thing in
// `main` (`setWorld`).

// #import drape_cord.wgsl

struct VoxelScene {
    geom: vec4<i32>,          // s, rise, base, roof from the table (1) or a column walk (0)
    extent: vec4<i32>,        // width, height, depth, raster_h
    knobs: vec4<f32>,         // haze, water alpha, atmosphere, rain_tick
    skyC: vec4<f32>,          // the sky zenith, already at SKY_BRIGHTNESS
    skyHorizon: vec4<f32>,    // the sky horizon
    bedrockC: vec4<f32>,
    rockC: vec4<f32>,
    soilC: vec4<f32>,
    waterDeepC: vec4<f32>,
    waterSurfaceC: vec4<f32>,
    lightC: vec4<f32>,
    hazeC: vec4<f32>,
    shadeA: vec4<f32>,        // TOP_GAIN, TOP_TINT, TOP_BACK, RIM
    shadeB: vec4<f32>,        // EDGE_DARK, TOP_EDGE, RISER_LEAN, WET
    roofK: vec4<f32>,         // ROOF_LIGHT, ROOF_FALLOFF_VOXELS, -, -
    waterK: vec4<f32>,        // SKIN_ALPHA_GAIN, WATER_TOP_ALPHA, -, -
    plantA: vec4<f32>,        // PLANT_TOP_GAIN, PLANT_TOP_TINT, PLANT_RIM, CROWN_EDGE
    plantB: vec4<f32>,        // CROWN_UNDER, TRUNK_SHADE[0], TRUNK_SHADE[1], TRUNK_LIGHT_AT
    tex: vec4<i32>,           // the face textures present, one bit per slot; vine tiles present;
                              // the drape cords' width (px, 0 off) and shape (`drape_cord.wgsl`)
    lightK: vec4<f32>,        // lit: ambient gain, ambient floor, ladder rungs, AO strength
    ambientC: vec4<f32>,      // lit: the ambient light's colour, the sky's hue at unit luminance
    sunK: vec4<f32>,          // lit: the unit direction toward the sun (zero: none), sun tint
    waterL: vec4<f32>,        // lit: absorption per voxel of path, reflection gain, ripple tilt, reflection cells
    clock: vec4<f32>,         // sim time in ticks (tick + the fraction elapsed), the water's animation step, flow debug, sky lift
    waterM: vec4<f32>,        // lit: foam, glint, rain rings, highlight (toward white)
    volK: vec4<f32>,          // lit: light shafts' density, glow weight, falloff (voxels), fade
    // The weather and the time of day (`cubarium_gpu::weather::Look`, package WX2). At a
    // clear noon every one of these leaves today's picture as it was.
    dayK: vec4<f32>,          // lit: the sun's share of a rung (ladder, smooth) over the ambient
                              // level, that level (ambientC is already times it); the twilight band
    dayL: vec4<f32>,          // lit: the unlit water's level (1 by day), the sun's strength, -, -
    sunLean: vec4<f32>,       // lit: what a sunlit texel leans toward; which side the sun is on (-1..1)
    cloudK: vec4<f32>,        // lit sky: cloud cover, drift (voxels), storm, twinkle clock (seconds)
    cloudBody: vec4<f32>,     // lit sky: the clouds' body colour; the stars' brightness
    cloudLit: vec4<f32>,      // lit sky: the clouds' sunlit edge colour, and its strength
    bandA: vec4<f32>,         // the twilight band's colours, top to bottom
    bandB: vec4<f32>,
    bandC: vec4<f32>,
    tint: vec4<f32>,          // flat: the whole picture's day/night tint (1 by day)
    dayA: vec4<f32>,          // lit sky: the day sky's zenith; how much of the day sky shows
    dayB: vec4<f32>,          // lit sky: its middle; the stars' share into the bloom
    dayC: vec4<f32>,          // lit sky: its horizon
    cloudDay: vec4<f32>,      // lit sky: the clouds' body by day
    cloudShade: vec4<f32>,    // lit sky: the clouds' shadowed underside by day
    // Rain, fog and lightning (WX2 checkpoint 2, `cubarium_gpu::weather::Look`). All zero at
    // a clear noon.
    rainK: vec4<f32>,         // how much drizzle, shower and downpour is falling (0..1 each); the
                              // rain rings' density on open water (0..1, from the rate)
    rainC: vec4<f32>,         // the streaks' colour (linear); how much of the sun the fog sends back
    fogK: vec4<f32>,          // the fog's density a voxel of path at its floor (0: none), the floor
                              // (voxels up), its full-density band above that, its falloff (voxels)
    fogC: vec4<f32>,          // the ambient light on the fog (linear); the emitters' glow it scatters
    flashC: vec4<f32>,        // a lightning flash's fill light (linear, already times the flash); the
                              // flash (0..1)
    strikeK: vec4<f32>,       // the bolt: its column's x centre, top y and z (voxels), its brightness
    strikeL: vec4<f32>,       // the bolt's seed, as two 16-bit halves
    // The pose this frame (WX4, `cubarium_gpu::voxel::pose_rows`): entry k's settle and
    // breath dip, in pixel rows, evaluated on the frame's clock.
    poseK: array<vec4<f32>, 64>,
    // Each style slot's emission gain this frame (`VoxelRenderer::set_emit_gains`, the
    // glowcap candle), four slots a row; 1 draws the emissive colour as it is.
    emitK: array<vec4<f32>, 64>,
    // The view (the hosted renderer's, H1): raster columns the picture is turned round the
    // ring (the camera's pan, whole pixels; 0 draws the strip as it always was), and the
    // highlighted animal's cell count (0 none, -1 its box alone).
    view: vec4<i32>,
    hiMin: vec4<i32>,         // the highlighted animal's box: first x (wrapped), lowest y, nearest z
    hiMax: vec4<i32>,         // its x span in cells, highest y, farthest z
    hiK: vec4<f32>,           // how far its cells lean toward white
    hiCells: array<vec4<u32>, 64>,  // its cells, x | y << 12 | z << 22, four a row
    // The figures' sun map (H2, `cubarium_gpu::figures`): a figure's highest point along
    // each sun ray, by where the ray meets y = 0. figSun: Lx/Ly, Lz/Ly, the map's first z,
    // on (1) or off; figMap: texels a voxel across and along z, the map's size.
    figSun: vec4<f32>,
    figMap: vec4<f32>,
    // How the ground fog moves (FOG-MOTION, `cubarium_gpu::weather::FogMotion`): the mode
    // (0 fixed, 1 breathing, 2 flowing), the breath's swing (a share of the density), its
    // period (seconds), the flow (voxels a second along +x).
    fogM: vec4<f32>,
    // How much of each rain mode's haze hangs in the air (drizzle, shower, downpour; eased
    // far slower than `rainK`, so the veil gathers and thins rather than switching).
    rainH: vec4<f32>,
    camera: vec4<i32>,        // vertical origin, full projected height, weather planes (plant=1, figure=2), -
}

@group(0) @binding(0) var<uniform> u: VoxelScene;

// The lighting tier (`lighting = "flat" | "lit"`), fixed when the pipeline is built. The
// flat tier's pipeline has it off: every `if (LIT)` below is dead code there, and what
// runs is the flat shader as it was. The lit tier replaces the flat tier's stand-ins for
// light (the column roof shade, and the TOP_* / PLANT_TOP_* gain-and-tint mixes) with a
// light value that multiplies each texel's base colour; the edge treatments are the
// flat tier's, leaning between the lit tones instead of the flat ones.
@id(0) override LIT: bool = false;
// The lit tier's effect switches (`cubarium_gpu::sunvis::Effects`), all off in the flat
// tier: an effect switched off is dead code and costs nothing.
@id(1) override SHADOWS: bool = true;      // the sun march
@id(2) override REFLECTIONS: bool = true;  // the water's reflection march
@id(3) override VOLUMETRIC: bool = false;  // the light shafts (volumetric.wgsl)
@id(4) override GLOW: bool = true;         // the emitters' local light
@id(5) override AO: bool = true;           // the AO crease (`[light] ao` > 0)
// The lit tier's sky pass (package WX2): this shader again, overridden to draw only the
// sky (`litSky`: the gradient, the twilight band, the stars, the clouds) into the backdrop
// image, just before the walk reads it. Kept out of the walk's own pipeline on purpose: the
// walk sits at an occupancy cliff, and the clouds' noise compiled into it cost 0.4 ms at
// 13 px before a single cloud was drawn.
@id(6) override SKY: bool = false;
// The lit tier's weather pass (WX2 checkpoint 2): this shader again, overridden to draw
// only the weather (`weatherOver`: fog, rain, splashes, the lightning bolt) over the raster
// after the walk, premultiplied, before the bloom. Out of the walk for the same reason as
// the sky; its pipeline keeps the walk's effect switches, so it knows whether the fog is
// already in the volumetric march.
@id(7) override WEATHER: bool = false;
// The lit walk writes its end depth for the weather pass (in the emission's alpha). A
// second walk pipeline, used only on frames that draw weather: keeping the walk's end
// slab to the end of the shader costs the walk about 0.1 ms at 13 px, which a clear sky
// does not pay.
@id(8) override WEATHER_DEPTH: bool = false;
// The sleep and idle poses (WX4, `posedFind`): a second walk pipeline, bound only on
// frames whose pack has an animal posing. The search costs the walk registers (a step of
// occupancy on the desktop's RDNA) even where no animal is, so the rest of the time it is
// dead code.
@id(9) override POSE: bool = false;
// **Plants as models** (`[figures] plants = "models"`, `crate::plants`): the walk's ray passes
// through every dry block plant cell (trunk, crown, heart), which the plant pass draws
// instead, swaying and growing in. The cells stay in the volume for everything else (the sun
// march, the sky plane, the canopy, the AO), so the plants light and shade as they did.
// Fixed for a renderer's life; off, the walk is exactly what it was.
@id(10) override PLANT_MODELS: bool = false;
// CELL-SHRINK: the walk binds the weighted variant only while EMIT_PLANTS is set.
// Off, the original whole-cell crease and sun march compile without size lookups or
// transmission state. Other passes keep the weighted path (the default).
@id(11) override PLANT_LIGHT: bool = true;

@group(0) @binding(1) var voxels: texture_3d<u32>;   // rgba8ui, one texel per voxel
@group(0) @binding(2) var roofTex: texture_3d<u32>;  // r8ui, voxels to the solid above
@group(0) @binding(3) var styleTex: texture_2d<f32>; // 4 x MAX_STYLES: wood, crown, heart, emit
@group(0) @binding(4) var glyphTex: texture_2d<u32>; // shared organism face texels
// The face textures at this px_per_voxel (`cubarium_gpu::voxel::VoxelTextures`): slot k,
// variant v is the S x S cell at (v*S, k*S), a top face in its first RISE rows. Texels
// are sRGB-encoded.
@group(0) @binding(5) var faceTex: texture_2d<f32>;
// The latticevine tiles at this px_per_voxel (`VoxelTextures::vine_rgba`): row
// 3*set + density (plain, climbing, hanging), column 16*mask + exits; row 9 the accents.
// Direct colour, sRGB.
@group(0) @binding(6) var vineTex: texture_2d<f32>;
// Lit tier only. The sky plane: at each open cell, the core's `sky_visibility` of the
// cell under it (the fan from the open cell's foot), 255 for open sky. Keyed on the
// terrain and computed off the loop thread.
@group(0) @binding(7) var skyTex: texture_3d<u32>;
// Lit tier only. The model's canopy per column: texels (x, 2z) and (x, 2z+1) hold four
// (cell, cumulative transmission) steps, highest cell first; a cell of 0 ends the list.
@group(0) @binding(8) var canopyTex: texture_2d<u32>;
// Lit tier only. The local light of the frame's emitters, one texel per 4x4x4 voxels
// (`GLOW_CELL`), in units of full ambient light: built on the CPU from the emitter list,
// spread a few cells through open terrain, and sampled trilinearly here.
@group(0) @binding(9) var glowTex: texture_3d<f32>;
// Lit tier only. The sky behind the world, from the sky pass (`SKY`), in linear light, one
// texel a raster pixel; the flat tier's is a single texel it never reads.
@group(0) @binding(11) var backdropTex: texture_2d<f32>;
// Lit tier only, read by the weather pass alone: the emission attachment, whose alpha the
// walk sets to the slab its ray ended in (over 255; the water surface where there is one),
// the weather's depth. The flat tier binds its one backdrop texel here.
@group(0) @binding(12) var depthTex: texture_2d<f32>;
// Lit tier: the figures' sun map (depth: a figure's height over H), `figSun` above.
@group(0) @binding(13) var figSunTex: texture_depth_2d;
// Lit tier: the emitter table (GLOW-EASE, `cubarium_gpu::voxel::emit_table`). Each emitting
// cell's colour this frame and its on share, open-addressed by texture index plus one:
// entries (key, rg, b on as half floats, -); this frame's `view.w` of them from entry
// `view.z`, none when `view.w` is 0.
@group(0) @binding(14) var<storage, read> emitCells: array<vec4<u32>>;
// The trilinear sampler of the glow and sun volumes (x repeats round the ring). Every
// other read is a texel load.
@group(0) @binding(33) var linearSampler: sampler;

// The plant part classes, as `crate::voxel`'s PART_* constants number them.
const TRUNK = 1;
const CROWN = 2;
const CROWN_HEART = 3;
const SPROUT = 4;
// The fauna range of the same field. The interim glyph is a flat block in the style's
// `wood`.
const ANIMAL_INTERIM = 5;
const LOG = 6;
const FLOOR_MARK = 7;

var<private> S: i32;
var<private> RISE: i32;
var<private> BASE: i32;
var<private> W: i32;
var<private> H: i32;
var<private> D: i32;

// The projection and the world's size, from the block.
fn setWorld() {
    S = u.geom.x;
    RISE = u.geom.y;
    BASE = u.geom.z;
    W = u.extent.x;
    H = u.extent.y;
    D = u.extent.z;
}

// --- the world, as the packing stores it ---------------------------------------------

// `a` round a ring of `m`, in 0..m. NVIDIA's driver once took the unsigned remainder of a
// negative operand: -1 % 640 came out 255, so on a ring whose width is not a power of two
// every lookup one cell left of the seam read a column 384 away (the sun march's shadows,
// the AO, the sky plane and the canopy at the seam). Only non-negative operands reach `%`
// here.
fn wrapMod(a: i32, m: i32) -> i32 {
    return select(m - 1 - (-1 - a) % m, a % m, a >= 0);
}

fn wrapX(x: i32) -> i32 { return wrapMod(x, W); }

fn inY(y: i32) -> bool { return y >= 0 && y < H; }

fn at(x: i32, y: i32, z: i32) -> vec4<u32> { return textureLoad(voxels, vec3(wrapX(x), y, z), 0); }

fn matOf(v: vec4<u32>) -> i32 { return i32(v.r & 3u); }
fn partOf(v: vec4<u32>) -> i32 { return i32((v.r >> 2u) & 7u); }
fn glyphOf(v: vec4<u32>) -> i32 { return i32(v.r >> 5u); }
fn isBlockPart(p: i32) -> bool { return p == TRUNK || p == CROWN || p == CROWN_HEART || p == ANIMAL_INTERIM || p == LOG; }
// A plant's own block cell: what the plant pass draws when plants are models.
fn plantBlockPart(p: i32) -> bool { return p == TRUNK || p == CROWN || p == CROWN_HEART; }
// A faceless animal cell of class torso (WX4 idle): an empty voxel under a dipping head,
// there only so the walk looks for the head from it. It is air to everything else (no
// crease, no shadow, no cover), so it can come and go with a head move unseen. The sleep's
// belly cells (class leg) keep counting as the settled body.
fn softMark(v: vec4<u32>) -> bool {
    return POSE && partOf(v) == ANIMAL_INTERIM && glyphOf(v) == 0 && v.b != 0u && (v.b >> 6u) < 2u;
}
fn solidV(v: vec4<u32>) -> bool { return matOf(v) != 0; }

// Out-of-range y reads as air, which is what makes the top of the world an open sky and
// the autotiling work at y = 0 (`present::solid`).
fn solidAt(x: i32, y: i32, z: i32) -> bool { return inY(y) && solidV(at(x, y, z)); }

// The free-water byte: 0 is dry, and any water at all is at least 1 (see VoxelTexel).
fn freeQ(x: i32, y: i32, z: i32) -> i32 {
    if (!inY(y)) { return 0; }
    return i32(at(x, y, z).g);
}

// `present::fill_px`: rows of a front rectangle a fraction fills, at least one.
fn fillPxQ(q: i32) -> i32 {
    if (q == 0) { return 0; }
    return clamp(i32(floor(f32(q) / 255.0 * f32(S) + 0.5)), 1, S);
}
fn fillPxAt(x: i32, y: i32, z: i32) -> i32 { return fillPxQ(freeQ(x, y, z)); }

// `present::water_open_up`, in drawn pixels and not in the raw fraction.
fn waterOpenUp(x: i32, y: i32, z: i32) -> bool {
    return fillPxAt(x, y, z) < S
        || (!solidAt(x, y + 1, z) && freeQ(x, y + 1, z) == 0);
}

fn frontRow(y: i32, z: i32) -> i32 { return BASE - (y + 1) * S - z * RISE; }

// --- face textures --------------------------------------------------------------------

// `TEXTURE_SLOTS`: terrain material m has its side at 2(m-1) and its top at 2(m-1)+1.
const TEX_TURF_SIDE = 6;
// A style's texture role (`ROLE_*`, the alpha of its wood column): bark, leaf and drape
// have a side slot and a top slot from TEX_BARK_SIDE on; the accent is untextured.
const ROLE_BARK = 1;
const ROLE_DRAPE = 3;
const TEX_BARK_SIDE = 7;

fn texOn(slot: i32) -> bool { return slot >= 0 && (u.tex.x & (1 << u32(slot))) != 0; }

// Which of a face's four variants a voxel shows: a hash of its position, so a wall is not
// one tile repeating, and the same voxel shows the same variant every frame.
fn cellHash(x: i32, y: i32, z: i32, salt: i32) -> u32 {
    var h = (u32(x) * 0x9E3779B1u) ^ (u32(y) * 0x85EBCA77u) ^ (u32(z) * 0xC2B2AE3Du)
        ^ (u32(salt) * 0x27D4EB2Fu);
    h ^= h >> 15u;
    h *= 0x2C1B3C6Du;
    h ^= h >> 12u;
    return h;
}

fn texVariant(x: i32, y: i32, z: i32, salt: i32) -> i32 {
    return i32(cellHash(x, y, z, salt) & 3u);
}

// Face texel (dx, dy) of `slot`: dx across the face, dy down a side face from its top row
// or across a top face from its back edge. The level is px_per_voxel itself, so this is
// one texel per screen pixel and never filtered.
fn faceTexel(slot: i32, x: i32, y: i32, z: i32, salt: i32, dx: i32, dy: i32) -> vec4<f32> {
    let v = texVariant(x, y, z, salt);
    return textureLoad(faceTex, vec2(v * S + dx, slot * S + dy), 0);
}

fn srgbToLinear(c: vec3<f32>) -> vec3<f32> {
    return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
}

// --- the latticevine tile layer (`cubarium::voxel::vine`) --------------------------------

// How a vine cell shows its face: flush on the rock behind it (a face looking at the
// camera), a curtain on its own front (an underside), or a quarter-width sliver against
// the wall on its left or right (a +x or -x face, seen edge-on).
const VINE_FLUSH = 0;
const VINE_CURTAIN = 1;
const VINE_SLIVER_L = 2;
const VINE_SLIVER_R = 3;

// An air voxel with no part and a nonzero glyph id carries a vine cell; the glyph id is
// its kind plus one.
fn isVine(v: vec4<u32>) -> bool {
    return u.tex.y != 0 && (v.r & 31u) == 0u && (v.r >> 5u) != 0u;
}

fn vineKind(v: vec4<u32>) -> i32 { return i32(v.r >> 5u) - 1; }

// The cell's tile at face pixel (dx, dy), row 0 at the top, in linear light; alpha is the
// cutout. The host resolved which tile (`VineCell::tile`): row in a's low nibble, column
// 16*mask + exits from b. The accent (bud, flower, fruit) goes over it, jittered a little
// per cell.
fn vineTexel(v: vec4<u32>, x: i32, y: i32, z: i32, dx: i32, dy: i32) -> vec4<f32> {
    let row = i32(v.a & 15u);
    let col = i32(v.b & 15u) * 16 + i32(v.b >> 4u);
    var t = textureLoad(vineTex, vec2(col * S + dx, row * S + dy), 0);
    let accent = i32((v.a >> 4u) & 3u);
    if (accent != 0) {
        let j = S / 8;
        let h = cellHash(x, y, z, 9);
        let ox = select(0, i32(h % u32(2 * j + 1)) - j, j > 0);
        let oy = select(0, i32((h >> 8u) % u32(2 * j + 1)) - j, j > 0);
        let ax = dx - ox;
        let ay = dy - oy;
        if (ax >= 0 && ax < S && ay >= 0 && ay < S) {
            let a = textureLoad(vineTex, vec2((accent - 1) * S + ax, 9 * S + ay), 0);
            if (a.a >= 0.5) { t = a; }
        }
    }
    return vec4(srgbToLinear(t.rgb), t.a);
}

// --- colour ---------------------------------------------------------------------------

fn strataOf(m: i32) -> vec3<f32> {
    if (m == 0) { return u.skyC.rgb; }
    if (m == 1) { return u.bedrockC.rgb; }
    if (m == 2) { return u.rockC.rgb; }
    return u.soilC.rgb;
}

// `Material::pore_capacity() > 0`: rock and soil hold pore water, air and bedrock do not.
fn holdsPore(m: i32) -> bool { return m == 2 || m == 3; }

fn hazeAt(zf: f32) -> f32 {
    if (D <= 1) { return 0.0; }
    return clamp(u.knobs.x, 0.0, 1.0) * clamp(zf / f32(D - 1), 0.0, 1.0);
}

fn hazed(c: vec3<f32>, t: f32) -> vec3<f32> { return mix(c, u.hazeC.rgb, clamp(t, 0.0, 1.0)); }

// `present::roof_shade`: how much of the top-face light a surface `gap` voxels below its
// roof keeps. 0 is an open column.
fn roofShade(gap: i32) -> f32 {
    if (gap == 0) { return 1.0; }
    let t = 1.0 - exp(-f32(gap - 1) / u.roofK.y);
    return u.roofK.x + (1.0 - u.roofK.x) * t;
}

// `VoxelPresenter::build_roof`: voxels from this one up to the nearest solid above it in
// its own column, 0 where the column is open to the sky. Either read from the table the
// CPU built or walked here; the two give the same number.
fn roofGap(x: i32, y: i32, z: i32) -> i32 {
    if (u.geom.w != 0) { return i32(textureLoad(roofTex, vec3(wrapX(x), y, z), 0).r); }
    for (var yy = y + 1; yy < H; yy++) {
        if (solidV(at(x, yy, z))) { return yy - y; }
    }
    return 0;
}

fn styleAt(style: i32, part: i32) -> vec3<f32> { return textureLoad(styleTex, vec2(part, style), 0).rgb; }

// A style's emissive colour is its fourth column (`VoxelStyle::with_emit`) in linear
// light, zero for none. A **whole-cell** emitter (`VoxelStyle::emit_whole`), whose every
// visible texel emits whatever its glyph's tones, carries a quarter in the fractional part
// of its three pigment columns' alphas, so the pigment fetch every texel makes already
// says so.
fn styleEmit(style: i32) -> vec3<f32> {
    return textureLoad(styleTex, vec2(3, style), 0).rgb * u.emitK[style >> 2u][style & 3];
}
fn emitsWhole(pigmentAlpha: f32) -> bool { return fract(pigmentAlpha) > 0.125; }

// GLOW-EASE: voxel (x, y, z)'s entry in the emitter table, or -1. `view.w` is the table's
// size (a power of two, 0: none) with `EMIT_VINES` set when some entry carries a vine
// cell's old tile.
const EMIT_VINES = 1u << 20u;
fn cellEntry(x: i32, y: i32, z: i32) -> i32 {
    let cap = u32(u.view.w) & (EMIT_VINES - 1u);
    if (cap == 0u) { return -1; }
    let key = u32((z * H + y) * W + wrapX(x)) + 1u;
    var h = ((key * 0x9E3779B1u) >> u32(32 - min(i32(firstTrailingBit(cap)), 31))) & (cap - 1u);
    for (var i = 0; i < 32; i++) {
        let c = emitCells[u.view.z + i32(h)];
        if (c.x == key) { return u.view.z + i32(h); }
        if (c.x == 0u) { return -1; }
        h = (h + 1u) & (cap - 1u);
    }
    return -1;
}

// GLOW-EASE: the colour and on share the host gave voxel (x, y, z)'s emitting texels this
// frame (`cubarium_gpu::voxel::EmitCell`), if it gave one; `e` and `on` are left alone if
// not. The texels then draw `e + (1 - on) x` their lit pigment, so an emitter fading in or
// out never steps, and one at nothing is its pigment.
fn entryEmit(i: i32, e: ptr<function, vec3<f32>>, on: ptr<function, f32>) -> bool {
    if (!LIT || i < 0) { return false; }
    let c = emitCells[i];
    if ((c.w >> 31u) != 0u) { return false; }
    let rg = unpack2x16float(c.y);
    let bo = unpack2x16float(c.z);
    *e = vec3(rg, bo.x);
    *on = clamp(bo.y, 0.0, 1.0);
    return true;
}
fn cellEmit(x: i32, y: i32, z: i32, e: ptr<function, vec3<f32>>, on: ptr<function, f32>) -> bool {
    if (!LIT) { return false; }
    return entryEmit(cellEntry(x, y, z), e, on);
}

// GLOW-EASE round 2: a vine cell part way through a change of tile (density, accent,
// shape), from its table entry `i` (`cellEntry`, -1: none): `v` with its old tile's bytes,
// and the share of the old tile still shown (0: no change under way, `v` left alone). Both
// tiers.
fn entryVineFrom(i: i32, v: ptr<function, vec4<u32>>) -> f32 {
    if (i < 0 || (u32(u.view.w) & EMIT_VINES) == 0u) { return 0.0; }
    let w = emitCells[i].w;
    let s = (w >> 16u) & 255u;
    if (s == 0u) { return 0.0; }
    (*v).b = w & 255u;
    (*v).a = (w >> 8u) & 255u;
    return f32(s) / 255.0;
}

// CELL-SHRINK: how big plant cell (x, y, z) is drawn while it arrives or leaves through the
// plant pass's grow-in (`cubarium_gpu::voxel::EmitCell::plant`), 1 for a whole cell: the
// weight of the light it casts on the rest of the world (its AO crease, its sun shadow). A
// leaving cell stays in the volume until it has shrunk away. `view.w`'s `EMIT_PLANTS` says
// the table carries any, so the walk looks plant cells up only then.
const EMIT_PLANTS = 1u << 21u;
fn plantSize(x: i32, y: i32, z: i32) -> f32 {
    if (!PLANT_LIGHT || (u32(u.view.w) & EMIT_PLANTS) == 0u) { return 1.0; }
    let i = cellEntry(x, y, z);
    if (i < 0) { return 1.0; }
    let p = (emitCells[i].w >> 24u) & 127u;
    if (p == 0u) { return 1.0; }
    return f32(p - 1u) / 125.0;
}

// CELL-SHRINK: how much of crown cell (x, y, z)'s outline edge at face column `dx` is drawn
// (`EmitCell::edge`): beside a cell of its crown arriving or leaving, one less that cell's
// size, so the edge comes and goes with it; 1 (the glyph's own edge) elsewhere.
fn crownEdge(x: i32, y: i32, z: i32, dx: i32) -> f32 {
    if (!PLANT_LIGHT || (u32(u.view.w) & EMIT_PLANTS) == 0u || (dx != 0 && dx != S - 1)) { return 1.0; }
    let i = cellEntry(x, y, z);
    if (i < 0) { return 1.0; }
    let w = emitCells[i].w;
    if (((w >> 16u) & 255u) != 0u) { return 1.0; }
    let e = select((w >> 8u) & 255u, w & 255u, dx == 0);
    if (e == 0u) { return 1.0; }
    return f32(e - 1u) / 254.0;
}

// The walk's `trans` once it has ended on an emitter: the light the emitter sends to the
// camera through what is in front of it (`trans` times its colour less the haze), negated
// and less one, so that every channel is at most -1. Nothing else makes `trans` negative,
// and after the walk this is where the emission for the bloom is read from: carrying it
// in a variable of its own through the walk costs the slab walk's occupancy.
fn emitterTrans(trans: vec3<f32>, drawn: vec3<f32>, haze: f32) -> vec3<f32> {
    return -(trans * max(drawn - clamp(haze, 0.0, 1.0) * u.hazeC.rgb, vec3(0.0))) - 1.0;
}

// The glyph atlas's emissive tone (`appearance::TONE_EMIT`): a texel that draws its
// style's emissive colour, unlit and at full value, in the lit tier. A style with no
// emissive colour, and the flat tier, draw it as a plain texel.
const TONE_EMIT = 6;

// A vine tile texel the host flagged as emitting (`textures::flag_vine_emitters`): alpha
// 254 instead of 255. Drawn at its own colour, unlit, in the lit tier.
fn vineEmits(t: vec4<f32>) -> bool { return t.a >= 0.5 && t.a < 0.998; }

fn plantLit(c: vec3<f32>, shade: f32) -> vec3<f32> {
    let lit = mix(c * u.plantA.x, u.lightC.rgb, u.plantA.y);
    return select(lit, mix(c, lit, shade), shade < 1.0);
}

// A solid face's colour before wetness and light: its texture's texel where the slot is
// present, the material's strata colour where it is not.
fn faceBase(m: i32, slot: i32, x: i32, y: i32, z: i32, salt: i32, dx: i32, dy: i32) -> vec3<f32> {
    if (!texOn(slot)) { return strataOf(m); }
    return srgbToLinear(faceTexel(slot, x, y, z, salt, dx, dy).rgb);
}

fn blockBody(v: vec4<u32>, base: vec3<f32>) -> vec3<f32> {
    let m = matOf(v);
    let wet = select(0.0, clamp(f32(v.b) / 255.0, 0.0, 1.0), holdsPore(m));
    return mix(base, u.waterDeepC.rgb, wet * u.shadeB.w);
}

fn blockLit(body: vec3<f32>, shade: f32) -> vec3<f32> {
    let lit = mix(body * u.shadeA.x, u.lightC.rgb, u.shadeA.y);
    return select(lit, mix(body, lit, shade), shade < 1.0);
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
// world in y or z is open. A plant cell part way through arriving or leaving darkens them
// by its size (`plantSize`): 1 is a whole cell, 0 none.
fn occluder(x: i32, y: i32, z: i32) -> f32 {
    if (!inY(y) || z < 0 || z >= D) { return 0.0; }
    let v = at(x, y, z);
    if (!PLANT_LIGHT) {
        return select(0.0, 1.0, solidV(v) || (isBlockPart(partOf(v)) && !softMark(v) && !cordCell(v)));
    }
    if (solidV(v)) { return 1.0; }
    let p = partOf(v);
    if (!isBlockPart(p) || softMark(v) || cordCell(v)) { return 0.0; }
    if (plantBlockPart(p)) { return plantSize(x, y, z); }
    return 1.0;
}
fn occluderAt(c: vec3<i32>) -> f32 { return occluder(c.x, c.y, c.z); }

// The AO is a **crease line** (Wrysk, checkpoint 2): a face stays flat, and only a thin
// band along an edge whose side neighbour in the face's open plane occludes, and a small
// square in a corner where only the diagonal neighbour does, take the AO step: the light
// times 1 - strength, before the ladder snaps it (one rung down at the default 0.5 on a
// 4-rung ladder). The band is about S/8 wide, at least 1 px (1 px at 6 px a voxel, 2 at 13),
// on a top face at most half its rows.
fn creaseWidth(rows: i32) -> i32 { return min(max(1, (S + 4) / 8), max(1, rows / 2)); }

// The shared appearance atlas: front rows followed by the receding cap rows.
fn glyphTexel(part: i32, glyph: i32, dx: i32, row: i32) -> u32 {
    return textureLoad(glyphTex, vec2(dx, (part * 8 + glyph) * (S + RISE) + row), 0).r;
}

// The crease factor at texel (du, dv) of a face nu x nv texels, dv = 0 on its `hi` side,
// from its four side neighbours and four diagonals (lo/hi x left/right), each an occluder's
// weight (0..1): the strongest of those whose band or corner square holds the texel takes
// the AO step that share of the way.
fn creaseBands(l: f32, r: f32, lo: f32, hi: f32,
               cLL: f32, cRL: f32, cLH: f32, cRH: f32, du: i32, dv: i32, nu: i32, nv: i32, b: i32) -> f32 {
    let inL = du < b;
    let inR = du >= nu - b;
    let inH = dv < b;
    let inLo = dv >= nv - b;
    if (!PLANT_LIGHT) {
        let band = (inL && l > 0.5) || (inR && r > 0.5) || (inH && hi > 0.5) || (inLo && lo > 0.5)
            || (inL && inLo && cLL > 0.5) || (inR && inLo && cRL > 0.5)
            || (inL && inH && cLH > 0.5) || (inR && inH && cRH > 0.5);
        return select(1.0, 1.0 - u.lightK.w, band);
    }
    var k = max(max(select(0.0, l, inL), select(0.0, r, inR)), max(select(0.0, hi, inH), select(0.0, lo, inLo)));
    k = max(k, max(max(select(0.0, cLL, inL && inLo), select(0.0, cRL, inR && inLo)),
                   max(select(0.0, cLH, inL && inH), select(0.0, cRH, inR && inH))));
    return 1.0 - u.lightK.w * k;
}

fn crease(l: f32, r: f32, lo: f32, hi: f32,
          cLL: f32, cRL: f32, cLH: f32, cRH: f32, du: i32, dv: i32, nu: i32, nv: i32) -> f32 {
    return creaseBands(l, r, lo, hi, cLL, cRL, cLH, cRH, du, dv, nu, nv, creaseWidth(nv));
}

// `creaseBands` of the face whose open plane holds cell `o`, `U` along its texels' du and
// `V` toward its `hi` side, reading only the occluders whose band or corner holds the texel
// (an inner texel reads none).
fn creaseOpen(o: vec3<i32>, U: vec3<i32>, V: vec3<i32>, du: i32, dv: i32, nu: i32, nv: i32) -> f32 {
    let b = creaseWidth(nv);
    let inL = du < b;
    let inR = du >= nu - b;
    let inH = dv < b;
    let inLo = dv >= nv - b;
    var k = 0.0;
    if (inL) { k = max(k, occluderAt(o - U)); }
    if (inR) { k = max(k, occluderAt(o + U)); }
    if (inH) { k = max(k, occluderAt(o + V)); }
    if (inLo) { k = max(k, occluderAt(o - V)); }
    if (inL && inLo) { k = max(k, occluderAt(o - U - V)); }
    if (inR && inLo) { k = max(k, occluderAt(o + U - V)); }
    if (inL && inH) { k = max(k, occluderAt(o - U + V)); }
    if (inR && inH) { k = max(k, occluderAt(o + U + V)); }
    return 1.0 - u.lightK.w * k;
}

// A figure face's baked open-plane occluders, in the same order as `crease`.
fn modelCrease(mask: u32, dx: i32, dy: i32, size: vec2<i32>) -> f32 {
    if (!AO) { return 1.0; }
    return creaseBands(f32(mask & 1u), f32((mask >> 1u) & 1u),
        f32((mask >> 2u) & 1u), f32((mask >> 3u) & 1u),
        f32((mask >> 4u) & 1u), f32((mask >> 5u) & 1u),
        f32((mask >> 6u) & 1u), f32((mask >> 7u) & 1u), dx, dy, size.x, size.y, creaseWidth(min(size.x, size.y)));
}

// AO of a front face of (x, y, z) at texel (dx, dy), dy = 0 at the top: the open plane is
// z-1.
fn aoFront(x: i32, y: i32, z: i32, dx: i32, dy: i32) -> f32 {
    if (!AO) { return 1.0; }
    if (!PLANT_LIGHT) {
        let p = z - 1;
        return crease(
            occluder(x - 1, y, p), occluder(x + 1, y, p), occluder(x, y - 1, p), occluder(x, y + 1, p),
            occluder(x - 1, y - 1, p), occluder(x + 1, y - 1, p),
            occluder(x - 1, y + 1, p), occluder(x + 1, y + 1, p),
            dx, dy, S, S);
    }
    return creaseOpen(vec3(x, y, z - 1), vec3(1, 0, 0), vec3(0, 1, 0), dx, dy, S, S);
}

// AO of a top face of (x, y, z) at texel (dx, dy), dy = 0 at the back: the open plane is
// y+1; its near side (z-1) is the face's bottom rows.
fn aoTop(x: i32, y: i32, z: i32, dx: i32, dy: i32) -> f32 {
    if (!AO) { return 1.0; }
    if (!PLANT_LIGHT) {
        let p = y + 1;
        return crease(
            occluder(x - 1, p, z), occluder(x + 1, p, z), occluder(x, p, z - 1), occluder(x, p, z + 1),
            occluder(x - 1, p, z - 1), occluder(x + 1, p, z - 1),
            occluder(x - 1, p, z + 1), occluder(x + 1, p, z + 1),
            dx, dy, S, RISE);
    }
    return creaseOpen(vec3(x, y + 1, z), vec3(1, 0, 0), vec3(0, 0, 1), dx, dy, S, RISE);
}

// The sky plane at open cell (x, y, z). Above the world, and in front of it (a front
// face at z = 0, whose fan leaves through the world's near side at once), is open sky.
// The plane is lifted toward open sky by clock.w (`VoxelRenderer::set_sky_lift`): a
// picture first drawn under open sky eases its plane in rather than switching to it.
fn skyOpen(x: i32, y: i32, z: i32) -> f32 {
    if (y >= H || z < 0) { return 1.0; }
    if (y < 0 || z >= D) { return 0.0; }
    return mix(f32(textureLoad(skyTex, vec3(wrapX(x), y, z), 0).r) / 255.0, 1.0, u.clock.w);
}

// The model's canopy over column (x, z) for a face of a voxel at height yv: the product
// of every foliage layer whose disc sits in a cell above yv (`shade_layers_into`'s
// attenuation, straight down).
fn canopyAt(x: i32, z: i32, yv: i32) -> f32 {
    if (z < 0 || z >= D) { return 1.0; }
    let a = textureLoad(canopyTex, vec2(wrapX(x), 2 * z), 0);
    if (i32(a.r) <= yv) { return 1.0; }
    if (i32(a.b) <= yv) { return f32(a.g) / 255.0; }
    let b = textureLoad(canopyTex, vec2(wrapX(x), 2 * z + 1), 0);
    if (i32(b.r) <= yv) { return f32(a.a) / 255.0; }
    if (i32(b.b) <= yv) { return f32(b.g) / 255.0; }
    return f32(b.a) / 255.0;
}

// The model's canopy over a plant cell's own faces, from the packer: the foliage layers
// above it in its own column **but its own stand's**, since the model never lets a stand
// shade itself (v.b's low nibble; `Packer::fill`). An animal's cell carries no such byte
// and reads the column plane like the terrain.
fn plantCanopy(v: vec4<u32>) -> f32 { return f32(v.b & 15u) / 15.0; }

// A crown cell's chance of letting a shadow ray through, from the packer: the model's
// transmission down its column spread evenly over the column's crown cells (v.b's high
// nibble), so a ray straight down through all of them keeps what the model lets through.
fn crownPass(v: vec4<u32>) -> f32 { return f32(v.b >> 4u) / 15.0; }

// What the sun adds to the ambient product when the light is smooth: the default 4-rung
// ladder's one rung, so a shadow darkens as much as it did (continuously, not by a rung).
const SMOOTH_SUN = 1.0 / 3.0;

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
fn ladderLight(ambient: f32, sun: f32, local: vec3<f32>) -> vec3<f32> {
    let smoothLight = u.lightK.z < 1.5;
    let n = max(u.lightK.z - 1.0, 1.0);
    var hue = u.ambientC.rgb;
    var a = ambient;
    let l = dot(local, vec3(0.2126, 0.7152, 0.0722));
    if (l > 1.0 / 512.0) {
        let sum = clamp(a, 0.0, 1.0) + l;
        let share = select(floor(l / sum * n + 0.5) / n, l / sum, smoothLight);
        hue = mix(hue, local / l, share);
        a = sum;
    }
    let t = select((floor(clamp(a, 0.0, 1.0) * n + 0.5) + sun * u.dayK.x) / n,
                   clamp(a, 0.0, 1.0) + sun * u.dayK.y, smoothLight);
    return hue * (u.lightK.x * (u.lightK.y + (1.0 - u.lightK.y) * t));
}

// The ladder's light on open, sunlit ground at a clear noon, in this light's hue (the
// ambient colour over its level): what the water's in-scatter is taken as a ratio to, so
// water darkens at night as the ground does.
fn ladderRef(sunOn: f32) -> vec3<f32> {
    let smoothLight = u.lightK.z < 1.5;
    let n = max(u.lightK.z - 1.0, 1.0);
    let t = select((floor(n + 0.5) + sunOn) / n, 1.0 + sunOn * SMOOTH_SUN, smoothLight);
    return u.ambientC.rgb / u.dayK.z * (u.lightK.x * (u.lightK.y + (1.0 - u.lightK.y) * t));
}

// The emitters' light at world point `p` (voxel units), trilinear over the glow volume
// whose texel i covers voxels 4i .. 4i + 4.
fn glowAt(p: vec3<f32>) -> vec3<f32> {
    if (!GLOW) { return vec3(0.0); }
    let n = vec3<f32>(textureDimensions(glowTex, 0)) * 4.0;
    // Explicit level: the walk is divergent per pixel, and an implicit-derivative sample
    // there costs as much as the rest of the lit shading together.
    return textureSampleLevel(glowTex, linearSampler, p / n, 0.0).rgb;
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
const SUN_MARCH = 128;

// Does a ray get through crown cell `c`? The same answer for every ray through the cell.
fn crownLets(v: vec4<u32>, c: vec3<i32>) -> bool {
    let h = cellHash(wrapX(c.x), c.y, c.z, 32);
    return f32(h & 0xFFFFu) / 65536.0 < crownPass(v);
}

// Whether a figure stands between world point `p` and the sun: on the sun ray through `p`
// the map holds a figure higher than `p` (by a quarter voxel, so a top face does not shade
// itself).
fn figureShades(p: vec3<f32>) -> bool {
    if (u.figSun.w < 0.5) { return false; }
    let fx = p.x - p.y * u.figSun.x;
    let fz = p.z - p.y * u.figSun.y - u.figSun.z;
    let t = vec2((fx - f32(W) * floor(fx / f32(W))) * u.figMap.x, fz * u.figMap.y);
    if (t.y < 0.0 || t.y >= u.figMap.w) { return false; }
    let top = textureLoad(figSunTex, vec2<i32>(min(t, u.figMap.zw - 1.0)), 0) * f32(H);
    return top > p.y + 0.25;
}

// 1 where the sun reaches world point `p`, which lies on the boundary of open cell `cell`
// (the cell in front of the face it is on); 0 where anything opaque is in the way. A plant
// cell part way through arriving or leaving lets through what its size does not cover
// (CELL-SHRINK, `plantSize`): the ray goes on past it with that share.
fn sunReaches(p: vec3<f32>, cell: vec3<i32>) -> f32 {
    if (!SHADOWS) { return 1.0; }
    if (figureShades(p)) { return 0.0; }
    let L = u.sunK.xyz;
    let dir = vec3<i32>(sign(L));
    let inv = vec3(
        select(1e30, 1.0 / abs(L.x), abs(L.x) > 1e-6),
        select(1e30, 1.0 / abs(L.y), abs(L.y) > 1e-6),
        select(1e30, 1.0 / abs(L.z), abs(L.z) > 1e-6));
    var c = cell;
    // How far along the ray each axis's next cell boundary is.
    var next = vec3(
        select(p.x - f32(c.x), f32(c.x + 1) - p.x, dir.x > 0) * inv.x,
        select(p.y - f32(c.y), f32(c.y + 1) - p.y, dir.y > 0) * inv.y,
        select(p.z - f32(c.z), f32(c.z + 1) - p.z, dir.z > 0) * inv.z);
    var through = 1.0;
    for (var i = 0; i < SUN_MARCH; i++) {
        if (c.y >= H || c.z < 0 || c.z >= D) { return through; }
        if (c.y < 0) { return 0.0; }
        let axis = select(select(2, 1, next.y < next.z), 0, next.x < next.y && next.x < next.z);
        let v = at(c.x, c.y, c.z);
        if (solidV(v)) { return 0.0; }
        let part = partOf(v);
        if (!PLANT_LIGHT) {
            if (part == TRUNK || part == LOG || (part == ANIMAL_INTERIM && !softMark(v))) { return 0.0; }
            if ((part == CROWN || part == CROWN_HEART) && !crownLets(v, c) && !cordCell(v)) { return 0.0; }
        } else {
            if (part == LOG || (part == ANIMAL_INTERIM && !softMark(v))) { return 0.0; }
            if (part == TRUNK
                || ((part == CROWN || part == CROWN_HEART) && !crownLets(v, c) && !cordCell(v))) {
                through *= 1.0 - plantSize(c.x, c.y, c.z);
                if (through <= 0.0) { return 0.0; }
            }
        }
        if (axis == 0) { c.x += dir.x; next.x += inv.x; }
        else if (axis == 1) { c.y += dir.y; next.y += inv.y; }
        else { c.z += dir.z; next.z += inv.z; }
    }
    return through;
}

// A lit colour: the base under the quantised light, and where the sun reaches, leaning
// toward the sun's colour by the sun tint times N·L (the flat tier's lean of a top face
// toward `lightC`, now gated by the sun). The sun's colour (`sunLean`) is the palette's
// light by day, warm low down, the moon's at night; the lean's strength (`sunK.w`) is the
// tint times the sun's strength, raised for a low sun (`cubarium_gpu::weather::Look`).
fn litBy(base: vec3<f32>, ambient: f32, sun: f32, ndl: f32, local: vec3<f32>) -> vec3<f32> {
    let c = base * ladderLight(ambient, sun, local);
    return mix(c, u.sunLean.rgb, clamp(u.sunK.w * sun * ndl, 0.0, 1.0));
}

// The front face of (x, y, z) at texel (dx, dy), lit, with `canopy` over it.
fn shadeFront(base: vec3<f32>, x: i32, y: i32, z: i32, dx: i32, dy: i32, canopy: f32) -> vec3<f32> {
    let ndl = max(-u.sunK.z, 0.0);
    var sun = 0.0;
    let p = vec3(f32(x) + (f32(dx) + 0.5) / f32(S),
                 f32(y + 1) - (f32(dy) + 0.5) / f32(S), f32(z));
    if (ndl > 0.0 && u.sunK.y > 0.0) {
        sun = sunReaches(p, vec3(x, y, z - 1));
    }
    // The local light half a voxel into the open cell in front of the face.
    let local = glowAt(p - vec3(0.0, 0.0, 0.5));
    return litBy(base, skyOpen(x, y, z - 1) * aoFront(x, y, z, dx, dy) * canopy, sun, ndl,
                 local);
}

// The top face of (x, y, z) at texel (dx, dy), dy = 0 at the back, lit, with `canopy`
// over it.
fn shadeTop(base: vec3<f32>, x: i32, y: i32, z: i32, dx: i32, dy: i32, canopy: f32) -> vec3<f32> {
    let ndl = max(u.sunK.y, 0.0);
    var sun = 0.0;
    let p = vec3(f32(x) + (f32(dx) + 0.5) / f32(S), f32(y + 1),
                 f32(z + 1) - (f32(dy) + 0.5) / f32(RISE));
    if (ndl > 0.0) {
        sun = sunReaches(p, vec3(x, y + 1, z));
    }
    // The local light half a voxel into the open cell above the face.
    let local = glowAt(p + vec3(0.0, 0.5, 0.0));
    return litBy(base, skyOpen(x, y + 1, z) * aoTop(x, y, z, dx, dy) * canopy, sun, ndl, local);
}
